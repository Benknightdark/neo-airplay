use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, Notify, watch};
use tokio::time::timeout;

use crate::{AppLogger, EngineStatus, LogLevel, ProcessSpawner, ProcessSpec, ReceiverState};

const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const SUPERVISOR_STOP_TIMEOUT: Duration = Duration::from_secs(7);

struct ActiveProcess {
    generation: u64,
    stop_sender: watch::Sender<bool>,
    completion: Arc<Notify>,
    stop_requested: Arc<AtomicBool>,
}

#[derive(Default)]
struct SupervisorState {
    generation: u64,
    active: Option<ActiveProcess>,
}

struct SharedState {
    lifecycle: Mutex<()>,
    process: Mutex<SupervisorState>,
    status: watch::Sender<EngineStatus>,
}

#[derive(Clone)]
pub struct ProcessSupervisor {
    shared: Arc<SharedState>,
    logger: Arc<dyn AppLogger>,
    spawner: Arc<dyn ProcessSpawner>,
}

impl ProcessSupervisor {
    pub fn new(
        initial_message: impl Into<String>,
        logger: Arc<dyn AppLogger>,
        spawner: Arc<dyn ProcessSpawner>,
    ) -> Self {
        let initial_status = EngineStatus {
            state: ReceiverState::Stopped,
            message: initial_message.into(),
        };
        let (status, _) = watch::channel(initial_status);
        Self {
            shared: Arc::new(SharedState {
                lifecycle: Mutex::new(()),
                process: Mutex::new(SupervisorState::default()),
                status,
            }),
            logger,
            spawner,
        }
    }

    pub fn status(&self) -> EngineStatus {
        self.shared.status.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<EngineStatus> {
        self.shared.status.subscribe()
    }

    pub async fn is_running(&self) -> bool {
        self.shared.process.lock().await.active.is_some()
    }

    pub fn set_status(&self, state: ReceiverState, message: impl Into<String>) {
        self.shared.status.send_replace(EngineStatus {
            state,
            message: message.into(),
        });
    }

    pub async fn start_process(
        &self,
        spec: ProcessSpec,
        process_name: &'static str,
        running_message: impl Into<String>,
    ) -> EngineStatus {
        let _lifecycle = self.shared.lifecycle.lock().await;
        {
            let process = self.shared.process.lock().await;
            if process.active.is_some() {
                return self.status();
            }
        }

        let mut child = match self.spawner.spawn(spec).await {
            Ok(child) => child,
            Err(error) => {
                let message = format!("{process_name} 啟動失敗：{error}");
                self.set_status(ReceiverState::Faulted, message.clone());
                self.logger.write(LogLevel::Error, message).await;
                return self.status();
            }
        };

        let stdout = child.take_stdout();
        let stderr = child.take_stderr();
        let (stop_sender, stop_receiver) = watch::channel(false);
        let completion = Arc::new(Notify::new());
        let stop_requested = Arc::new(AtomicBool::new(false));
        let generation = {
            let mut process = self.shared.process.lock().await;
            process.generation = process.generation.wrapping_add(1);
            let generation = process.generation;
            process.active = Some(ActiveProcess {
                generation,
                stop_sender,
                completion: completion.clone(),
                stop_requested: stop_requested.clone(),
            });
            generation
        };

        self.set_status(ReceiverState::Running, running_message);
        spawn_log_reader(
            stdout,
            Arc::clone(&self.logger),
            LogLevel::Information,
            process_name,
        );
        spawn_log_reader(
            stderr,
            Arc::clone(&self.logger),
            LogLevel::Warning,
            process_name,
        );

        let shared = Arc::clone(&self.shared);
        let logger = Arc::clone(&self.logger);
        tokio::spawn(async move {
            let result = monitor_child(&mut *child, stop_receiver, &logger, process_name).await;
            let requested = stop_requested.load(Ordering::Acquire);
            complete_process(
                &shared,
                generation,
                process_name,
                requested,
                result,
                &logger,
            )
            .await;
            completion.notify_one();
        });

        self.status()
    }

    pub async fn stop(&self, stopped_message: impl Into<String>) -> EngineStatus {
        let _lifecycle = self.shared.lifecycle.lock().await;
        let control = {
            let process = self.shared.process.lock().await;
            process.active.as_ref().map(|active| {
                active.stop_requested.store(true, Ordering::Release);
                ActiveProcess {
                    generation: active.generation,
                    stop_sender: active.stop_sender.clone(),
                    completion: Arc::clone(&active.completion),
                    stop_requested: Arc::clone(&active.stop_requested),
                }
            })
        };

        let Some(control) = control else {
            if self.status().state != ReceiverState::DependencyMissing {
                self.set_status(ReceiverState::Stopped, stopped_message);
            }
            return self.status();
        };

        self.set_status(ReceiverState::Stopping, "正在停止接收器。");
        let completion = control.completion.notified();
        tokio::pin!(completion);
        let _ = control.stop_sender.send(true);
        let completed = timeout(SUPERVISOR_STOP_TIMEOUT, &mut completion)
            .await
            .is_ok();
        if !completed {
            let message = format!("停止程序逾時：{}", control.generation);
            self.logger.write(LogLevel::Error, message).await;
            return self.status();
        }

        if self.status().state != ReceiverState::Faulted {
            self.set_status(ReceiverState::Stopped, stopped_message);
        }
        self.status()
    }
}

fn spawn_log_reader<R>(
    reader: Option<R>,
    logger: Arc<dyn AppLogger>,
    level: LogLevel,
    prefix: &'static str,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let Some(reader) = reader else {
        return;
    };

    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                logger.write(level, format!("{prefix}: {line}")).await;
            }
        }
    });
}

async fn monitor_child(
    child: &mut dyn crate::ChildHandle,
    mut stop_receiver: watch::Receiver<bool>,
    logger: &Arc<dyn AppLogger>,
    process_name: &'static str,
) -> std::io::Result<ExitStatus> {
    loop {
        tokio::select! {
            result = child.wait() => return result,
            changed = stop_receiver.changed() => {
                if changed.is_err() || *stop_receiver.borrow() {
                    if let Err(error) = child.request_stop().await {
                        logger
                            .write(
                                LogLevel::Warning,
                                format!("無法正常停止 {process_name}，稍後將終止程序：{error}"),
                            )
                            .await;
                    }

                    return match timeout(GRACEFUL_STOP_TIMEOUT, child.wait()).await {
                        Ok(result) => result,
                        Err(_) => {
                            if let Err(error) = child.kill_tree().await {
                                logger
                                    .write(
                                        LogLevel::Error,
                                        format!("無法終止 {process_name} 程序樹：{error}"),
                                    )
                                    .await;
                            }
                            child.wait().await
                        }
                    };
                }
            }
        }
    }
}

async fn complete_process(
    shared: &SharedState,
    generation: u64,
    process_name: &str,
    requested_stop: bool,
    result: std::io::Result<ExitStatus>,
    logger: &Arc<dyn AppLogger>,
) {
    let (state, level, message) = if requested_stop {
        (
            ReceiverState::Stopped,
            LogLevel::Information,
            format!("{process_name} 已停止。"),
        )
    } else {
        match result {
            Ok(status) if status.code() == Some(0) => (
                ReceiverState::Stopped,
                LogLevel::Information,
                format!("{process_name} 已結束。"),
            ),
            Ok(status) => {
                let code = status
                    .code()
                    .map_or_else(|| "unknown".to_owned(), |code| code.to_string());
                (
                    ReceiverState::Faulted,
                    LogLevel::Error,
                    format!("{process_name} 已結束，結束代碼：{code}。"),
                )
            }
            Err(error) => (
                ReceiverState::Faulted,
                LogLevel::Error,
                format!("等待 {process_name} 結束失敗：{error}"),
            ),
        }
    };

    let completed = {
        let mut process = shared.process.lock().await;
        if process.generation == generation {
            process.active = None;
            shared.status.send_replace(EngineStatus {
                state,
                message: message.clone(),
            });
            true
        } else {
            false
        }
    };
    if completed {
        logger.write(level, message).await;
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::Arc;

    use crate::{FileAppLogger, ProcessSpec, ReceiverState, TokioProcessSpawner};

    use super::ProcessSupervisor;

    #[tokio::test]
    async fn launch_and_stop_tracks_process_lifetime() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = Arc::new(FileAppLogger::new(temp.path().join("logs")));
        let supervisor = ProcessSupervisor::new("stopped", logger, Arc::new(TokioProcessSpawner));
        let spec = ProcessSpec {
            program: std::env::current_exe().expect("test executable should be available"),
            args: ["--exact", "process::tests::child_helper", "--nocapture"]
                .into_iter()
                .map(OsString::from)
                .collect(),
            cwd: std::env::current_dir().expect("current directory should be available"),
            env: vec![(
                OsString::from("NEO_PROCESS_CHILD_HELPER"),
                OsString::from("hold"),
            )],
        };

        supervisor.set_status(ReceiverState::Starting, "starting");
        assert_eq!(
            supervisor
                .start_process(spec, "test process", "running")
                .await
                .state,
            ReceiverState::Running
        );

        assert_eq!(
            supervisor.stop("stopped").await.state,
            ReceiverState::Stopped
        );
        assert!(!supervisor.is_running().await);
    }
}
