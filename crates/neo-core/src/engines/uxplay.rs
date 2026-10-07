use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    AppLogger, EngineStatus, LogLevel, MdnsInspector, Platform, ProcessSpawner, ProcessSpec,
    ReceiverSettings, ReceiverState, SystemMdnsInspector,
};

use super::ProcessSupervisor;

pub struct UxPlayEngine {
    platform: Platform,
    bundled_engine_directory: Option<PathBuf>,
    user_engine_directory: PathBuf,
    gstreamer_registry_path: Option<PathBuf>,
    mdns: Arc<dyn MdnsInspector>,
    supervisor: ProcessSupervisor,
    logger: Arc<dyn AppLogger>,
    lifecycle: Mutex<()>,
}

impl UxPlayEngine {
    pub fn new(
        platform: Platform,
        bundled_engine_directory: Option<PathBuf>,
        user_engine_directory: PathBuf,
        gstreamer_registry_path: Option<PathBuf>,
        logger: Arc<dyn AppLogger>,
        spawner: Arc<dyn ProcessSpawner>,
    ) -> Self {
        Self {
            platform,
            bundled_engine_directory,
            user_engine_directory,
            gstreamer_registry_path,
            mdns: Arc::new(SystemMdnsInspector),
            supervisor: ProcessSupervisor::new("接收器尚未啟動。", Arc::clone(&logger), spawner),
            logger,
            lifecycle: Mutex::new(()),
        }
    }

    pub fn status(&self) -> EngineStatus {
        self.supervisor.status()
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineStatus> {
        self.supervisor.subscribe()
    }

    pub async fn start(&self, settings: &ReceiverSettings) -> EngineStatus {
        let _lifecycle = self.lifecycle.lock().await;
        if self.supervisor.is_running().await {
            return self.status();
        }

        self.supervisor
            .set_status(ReceiverState::Starting, "正在啟動 UxPlay。");
        let mdns_runtime = match self.mdns.runtime() {
            Ok(runtime) => runtime,
            Err(problem) => {
                self.supervisor
                    .set_status(ReceiverState::DependencyMissing, problem.message.clone());
                self.logger.write(LogLevel::Error, problem.message).await;
                return self.status();
            }
        };
        let Some(executable) = self.locate_executable(settings.uxplay_path.as_deref()) else {
            let message = self.missing_engine_message();
            self.supervisor
                .set_status(ReceiverState::DependencyMissing, message.clone());
            self.logger.write(LogLevel::Error, message).await;
            return self.status();
        };

        let Some(spec) =
            self.process_spec(&executable, settings, mdns_runtime.directory.as_deref())
        else {
            let message = "找不到 UxPlay 所需的 GStreamer 執行元件。".to_owned();
            self.supervisor
                .set_status(ReceiverState::DependencyMissing, message.clone());
            self.logger.write(LogLevel::Error, message).await;
            return self.status();
        };

        let receiver_name = settings.receiver_name.trim();
        self.supervisor
            .start_process(
                spec,
                "UxPlay",
                format!("正在以「{receiver_name}」接收 AirPlay。"),
            )
            .await
    }

    pub async fn stop(&self) -> EngineStatus {
        let _lifecycle = self.lifecycle.lock().await;
        self.supervisor.stop("接收器已停止。").await
    }

    fn locate_executable(&self, custom_path: Option<&str>) -> Option<PathBuf> {
        let executable_name = if self.platform == Platform::Windows {
            "uxplay.exe"
        } else {
            "uxplay"
        };

        let mut candidates = Vec::new();
        if let Some(custom_path) = custom_path.filter(|path| !path.trim().is_empty()) {
            candidates.push(PathBuf::from(custom_path));
        }

        if self.platform == Platform::Windows {
            if let Some(directory) = &self.bundled_engine_directory {
                candidates.push(directory.join(executable_name));
            }
            candidates.push(self.user_engine_directory.join(executable_name));
        } else {
            if self.platform == Platform::Macos {
                candidates.push(PathBuf::from("/opt/homebrew/bin/uxplay"));
                candidates.push(PathBuf::from("/usr/local/bin/uxplay"));
            }
            candidates.push(
                self.user_engine_directory
                    .join("uxplay")
                    .join(executable_name),
            );
            candidates.extend(path_candidates(executable_name, self.platform));
        }

        candidates.into_iter().find(|path| path.is_file())
    }

    fn process_spec(
        &self,
        executable: &Path,
        settings: &ReceiverSettings,
        bonjour_directory: Option<&Path>,
    ) -> Option<ProcessSpec> {
        let engine_directory = executable.parent()?.to_path_buf();
        let mut environment = Vec::new();
        let mut path_prefixes = vec![engine_directory.clone()];
        if let Some(directory) = bonjour_directory {
            path_prefixes.push(directory.to_path_buf());
        }

        if self.platform == Platform::Windows {
            let runtime_directory = engine_directory.join("runtime").join("bin");
            let plugin_directory = engine_directory
                .join("runtime")
                .join("lib")
                .join("gstreamer-1.0");
            if !runtime_directory.is_dir() || !plugin_directory.is_dir() {
                return None;
            }
            path_prefixes.push(runtime_directory);
            environment.push((
                OsString::from("GST_PLUGIN_PATH"),
                plugin_directory.clone().into_os_string(),
            ));
            environment.push((
                OsString::from("GST_PLUGIN_SYSTEM_PATH_1_0"),
                plugin_directory.into_os_string(),
            ));
            if let Some(registry) = &self.gstreamer_registry_path {
                environment.push((
                    OsString::from("GST_REGISTRY"),
                    registry.as_os_str().to_os_string(),
                ));
            }
            let scanner = engine_directory
                .join("runtime")
                .join("libexec")
                .join("gstreamer-1.0")
                .join("gst-plugin-scanner.exe");
            if scanner.is_file() {
                environment.push((
                    OsString::from("GST_PLUGIN_SCANNER"),
                    scanner.into_os_string(),
                ));
            }
        }

        if self.platform == Platform::Macos {
            path_prefixes.push(PathBuf::from("/opt/homebrew/bin"));
            path_prefixes.push(PathBuf::from("/usr/local/bin"));
        }
        if let Some(existing_path) = std::env::var_os("PATH") {
            path_prefixes.extend(std::env::split_paths(&existing_path));
        }
        if let Ok(path) = std::env::join_paths(path_prefixes) {
            environment.push((OsString::from("PATH"), path));
        }

        Some(ProcessSpec {
            program: executable.to_path_buf(),
            args: uxplay_args(settings, self.platform),
            cwd: engine_directory,
            env: environment,
        })
    }

    fn missing_engine_message(&self) -> String {
        match self.platform {
            Platform::Windows => {
                "找不到 engine\\uxplay.exe。請先完成 scripts\\build-engine.ps1。".to_owned()
            }
            Platform::Macos => "找不到 UxPlay。請依設定頁的安裝指引安裝或指定執行檔。".to_owned(),
            Platform::Linux => {
                "找不到 UxPlay。請依設定頁的安裝指引安裝發行版套件或指定執行檔。".to_owned()
            }
        }
    }
}

pub fn uxplay_args(settings: &ReceiverSettings, platform: Platform) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-n"),
        OsString::from(settings.receiver_name.trim()),
        OsString::from("-nh"),
    ];
    if settings.fullscreen {
        arguments.push(OsString::from("-fs"));
    }
    if let Some(sink) = platform.video_sink(&settings.renderer) {
        arguments.push(OsString::from("-vs"));
        arguments.push(OsString::from(sink));
    }
    arguments
}

fn path_candidates(executable_name: &str, platform: Platform) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates
            .extend(std::env::split_paths(&path).map(|directory| directory.join(executable_name)));
    }
    if platform == Platform::Linux {
        candidates.push(PathBuf::from("/usr/bin/uxplay"));
        candidates.push(PathBuf::from("/usr/local/bin/uxplay"));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use crate::{Platform, ReceiverSettings};

    use super::uxplay_args;

    #[test]
    fn creates_platform_appropriate_arguments() {
        let settings = ReceiverSettings {
            receiver_name: "Living Room".to_owned(),
            renderer: "D3D11".to_owned(),
            fullscreen: true,
            ..ReceiverSettings::default()
        };

        assert_eq!(
            uxplay_args(&settings, Platform::Windows),
            ["-n", "Living Room", "-nh", "-fs", "-vs", "d3d11videosink"]
                .map(std::ffi::OsString::from)
        );
        assert_eq!(
            uxplay_args(&settings, Platform::Macos),
            ["-n", "Living Room", "-nh", "-fs"].map(std::ffi::OsString::from)
        );
    }
}
