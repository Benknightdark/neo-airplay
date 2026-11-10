#[cfg(target_os = "windows")]
use crate::state::background_command;
use crate::state::{installer_path, run_with_timeout_in_path};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::process::{Child, Output};
#[cfg(not(target_os = "macos"))]
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CHECK_TIMEOUT: Duration = Duration::from_secs(90);
#[cfg(target_os = "macos")]
const GSTREAMER_REGISTRY_WARMUP_TIMEOUT: Duration = Duration::from_secs(100);
const PROCESS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const RESULT_GRACE: Duration = Duration::from_secs(5);
const LAUNCH_GRACE: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

const INSTALL_MARKER: &str = "install.pid";
const INSTALL_RESULT: &str = "install-result.json";
const INSTALL_LAUNCH: &str = "install-launch.json";
const INSTALL_LOG: &str = "install.log";
const INSTALL_PROGRESS: &str = "install-progress.json";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencySnapshot {
    pub status: String,
    pub message: String,
    pub missing: Vec<String>,
    pub log_path: String,
}

pub struct DependencyManager {
    snapshot: Arc<Mutex<DependencySnapshot>>,
    worker_active: Arc<AtomicBool>,
    engine_dir: PathBuf,
    gstreamer_registry_path: PathBuf,
    setup_dir: PathBuf,
    enabled: bool,
}

impl DependencyManager {
    pub fn new(
        engine_dir: PathBuf,
        gstreamer_registry_path: PathBuf,
        config_dir: &Path,
        enabled: bool,
    ) -> Self {
        let setup_dir = config_dir.join("setup");
        Self {
            snapshot: Arc::new(Mutex::new(DependencySnapshot {
                status: if enabled { "checking" } else { "disabled" }.into(),
                message: if enabled {
                    "正在檢查投影工具…"
                } else {
                    "此工作階段已停用自動安裝。"
                }
                .into(),
                missing: Vec::new(),
                log_path: setup_dir.join(INSTALL_LOG).to_string_lossy().into_owned(),
            })),
            worker_active: Arc::new(AtomicBool::new(false)),
            engine_dir,
            gstreamer_registry_path,
            setup_dir,
            enabled,
        }
    }

    pub fn snapshot(&self) -> Result<DependencySnapshot, String> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| "無法讀取工具安裝狀態。".into())
    }

    pub fn start(&self) -> Result<DependencySnapshot, String> {
        if !self.enabled {
            return self.snapshot();
        }
        if self
            .worker_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return self.snapshot();
        }

        match self.snapshot.lock() {
            Ok(mut snapshot) => {
                snapshot.status = "checking".into();
                snapshot.message = "正在檢查 UxPlay、GStreamer、ADB 與 scrcpy…".into();
            }
            Err(_) => {
                self.worker_active.store(false, Ordering::Release);
                return Err("無法更新工具安裝狀態。".into());
            }
        }

        let shared = Arc::clone(&self.snapshot);
        let worker_active = Arc::clone(&self.worker_active);
        let engine_dir = self.engine_dir.clone();
        let gstreamer_registry_path = self.gstreamer_registry_path.clone();
        let setup_dir = self.setup_dir.clone();
        let spawned = thread::Builder::new()
            .name("neo-airplay-dependency-setup".into())
            .spawn(move || {
                let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    run_setup(&engine_dir, &gstreamer_registry_path, &setup_dir, &shared)
                }));
                let outcome = match run {
                    Ok(Ok(outcome)) => outcome,
                    Ok(Err(error)) => {
                        append_log(&setup_dir.join(INSTALL_LOG), &error);
                        SetupOutcome::failed(error, Vec::new())
                    }
                    Err(_) => {
                        let message = "工具檢查背景程序意外結束。".to_owned();
                        append_log(&setup_dir.join(INSTALL_LOG), &message);
                        SetupOutcome::failed(message, Vec::new())
                    }
                };
                if let Ok(mut snapshot) = shared.lock() {
                    snapshot.status = outcome.status;
                    snapshot.message = outcome.message;
                    snapshot.missing = outcome.missing;
                }
                worker_active.store(false, Ordering::Release);
            });

        if let Err(error) = spawned {
            self.worker_active.store(false, Ordering::Release);
            if let Ok(mut snapshot) = self.snapshot.lock() {
                snapshot.status = "failed".into();
                snapshot.message = format!("無法啟動背景工具檢查：{error}");
            }
            return Err(error.to_string());
        }

        self.snapshot()
    }
}

struct SetupOutcome {
    status: String,
    message: String,
    missing: Vec<String>,
}

impl SetupOutcome {
    fn ready() -> Self {
        Self {
            status: "ready".into(),
            message: "投影工具已就緒。".into(),
            missing: Vec::new(),
        }
    }

    fn failed(message: String, missing: Vec<String>) -> Self {
        Self {
            status: "failed".into(),
            message,
            missing,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    pid: u32,
    exit_code: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl InstallResult {
    fn detail(&self) -> String {
        let mut detail = format!("安裝程序結束代碼 {}", self.exit_code);
        if let Some(stage) = &self.stage {
            detail.push_str(&format!("（{}）", stage_label(stage)));
        }
        if let Some(message) = self
            .message
            .as_deref()
            .filter(|message| !message.is_empty())
        {
            detail.push_str(&format!("：{message}"));
        }
        detail
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct InstallProgress {
    pid: u32,
    stage: String,
    message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchRecord {
    owner_pid: u32,
    worker_pid: Option<u32>,
    started_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeCheck {
    missing: Vec<String>,
}

impl RuntimeCheck {
    fn is_ready(&self) -> bool {
        self.missing.is_empty()
    }
}

fn run_setup(
    engine_dir: &Path,
    gstreamer_registry_path: &Path,
    setup_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
) -> Result<SetupOutcome, String> {
    fs::create_dir_all(setup_dir).map_err(|error| format!("無法建立安裝記錄目錄：{error}"))?;

    let marker_path = setup_dir.join(INSTALL_MARKER);
    let result_path = setup_dir.join(INSTALL_RESULT);
    let launch_path = setup_dir.join(INSTALL_LAUNCH);
    let previous_attempt = read_launch_record(&launch_path)?;

    if let Some(pid) = read_install_marker(&marker_path)? {
        match process_presence(pid, engine_dir) {
            ProcessPresence::Alive | ProcessPresence::Unknown => {
                publish_installing(snapshot, "正在等待已執行的工具安裝程序完成。");
                let _ = wait_for_install(
                    engine_dir,
                    setup_dir,
                    Some(pid),
                    None,
                    previous_attempt.clone(),
                    snapshot,
                )?;
            }
            ProcessPresence::Dead if !result_path.exists() => {
                if let Some(record) = previous_attempt
                    .as_ref()
                    .filter(|record| launch_is_recent(record))
                {
                    publish_installing(snapshot, "正在確認工具安裝程序是否已結束。");
                    let _ = wait_for_install(
                        engine_dir,
                        setup_dir,
                        Some(pid),
                        None,
                        Some(record.clone()),
                        snapshot,
                    )?;
                } else {
                    // A just-finished script removes its marker before atomically writing the result.
                    thread::sleep(RESULT_GRACE);
                }
            }
            ProcessPresence::Dead => {}
        }
    } else if let Some(record) = previous_attempt.as_ref() {
        if let Some(pid) = record.worker_pid {
            match process_presence(pid, engine_dir) {
                ProcessPresence::Alive | ProcessPresence::Unknown => {
                    publish_installing(snapshot, "正在等待已執行的工具安裝程序完成。");
                    let _ = wait_for_install(
                        engine_dir,
                        setup_dir,
                        None,
                        None,
                        Some(record.clone()),
                        snapshot,
                    )?;
                }
                ProcessPresence::Dead if launch_is_recent(record) => {
                    publish_installing(snapshot, "正在確認工具安裝程序是否已結束。");
                    let _ = wait_for_install(
                        engine_dir,
                        setup_dir,
                        None,
                        None,
                        Some(record.clone()),
                        snapshot,
                    )?;
                }
                ProcessPresence::Dead => {}
            }
        } else if launch_is_recent(record) {
            publish_installing(snapshot, "正在確認工具安裝程序是否已啟動。");
            let _ = wait_for_install(
                engine_dir,
                setup_dir,
                None,
                None,
                Some(record.clone()),
                snapshot,
            )?;
        }
    }

    // Avoid rewriting a staged script while an installer from an earlier app process may be
    // reading it. Recovery waits above before refreshing the embedded copy.
    stage_installer(setup_dir)?;
    let runtime = check_runtime(engine_dir, setup_dir)?;
    if runtime.is_ready() {
        warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
        remove_if_exists(&launch_path);
        return Ok(SetupOutcome::ready());
    }

    remove_if_exists(&marker_path);
    remove_if_exists(&result_path);
    remove_if_exists(&launch_path);

    let missing_message = format!("缺少或無法載入：{}", runtime.missing.join("、"));
    append_log(&setup_dir.join(INSTALL_LOG), &missing_message);
    publish_installing(
        snapshot,
        &format!("{missing_message} 正在背景安裝，請完成系統授權提示。"),
    );

    let result = install_dependencies(setup_dir, engine_dir, snapshot);
    let final_check = check_runtime(engine_dir, setup_dir)?;
    if final_check.is_ready() {
        warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
        remove_if_exists(&launch_path);
        return Ok(SetupOutcome::ready());
    }

    let install_detail = match result {
        Ok(result) => result.detail(),
        Err(error) => format!("安裝程序未完成：{error}"),
    };
    let message = format!(
        "{install_detail}；仍缺少 {}。請查看安裝記錄後重試。",
        final_check.missing.join("、")
    );
    append_log(&setup_dir.join(INSTALL_LOG), &message);
    Ok(SetupOutcome::failed(message, final_check.missing))
}

fn warm_gstreamer_registry(
    engine_dir: &Path,
    registry_path: &Path,
    setup_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
) {
    #[cfg(target_os = "macos")]
    {
        if fs::metadata(registry_path).is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }

        publish_installing(snapshot, "正在準備 GStreamer 快取…");
        let started_at = Instant::now();
        let script = setup_dir.join("install.sh");
        let args = vec![
            script.to_string_lossy().into_owned(),
            "--warm-gstreamer-registry".into(),
            "--engine-dir".into(),
            engine_dir.to_string_lossy().into_owned(),
            "--registry-path".into(),
            registry_path.to_string_lossy().into_owned(),
        ];
        let output = run_with_timeout_in_path(
            Path::new("/bin/bash"),
            &args,
            GSTREAMER_REGISTRY_WARMUP_TIMEOUT,
            Some(installer_path(engine_dir).as_os_str()),
        );
        match output {
            Ok(output) => {
                let output_text = combined_output(&output);
                if !output_text.trim().is_empty() {
                    append_log(&setup_dir.join(INSTALL_LOG), output_text.trim());
                }
                if !output.status.success() {
                    append_log(
                        &setup_dir.join(INSTALL_LOG),
                        &format!(
                            "GStreamer registry 預熱失敗，仍繼續啟動：{}",
                            first_useful_line(&output_text)
                                .unwrap_or_else(|| format!("結束代碼 {}", output.status))
                        ),
                    );
                }
            }
            Err(error) => append_log(
                &setup_dir.join(INSTALL_LOG),
                &format!("GStreamer registry 預熱失敗，仍繼續啟動：{error}"),
            ),
        }
        append_log(
            &setup_dir.join(INSTALL_LOG),
            &format!(
                "GStreamer registry 預熱耗時 {} 毫秒。",
                started_at.elapsed().as_millis()
            ),
        );
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (engine_dir, registry_path, setup_dir, snapshot);
}

fn stage_installer(setup_dir: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let (path, contents) = (
        setup_dir.join("install.ps1"),
        include_str!("../../scripts/install.ps1"),
    );
    #[cfg(not(target_os = "windows"))]
    let (path, contents) = (
        setup_dir.join("install.sh"),
        include_str!("../../scripts/install.sh"),
    );

    fs::write(&path, contents).map_err(|error| format!("無法準備工具安裝程序：{error}"))?;
    Ok(path)
}

fn check_runtime(engine_dir: &Path, setup_dir: &Path) -> Result<RuntimeCheck, String> {
    let path = installer_path(engine_dir);
    #[cfg(target_os = "windows")]
    let (executable, args) = (
        PathBuf::from("powershell.exe"),
        vec![
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            setup_dir.join("install.ps1").to_string_lossy().into_owned(),
            "-Check".into(),
            "-EngineDirectory".into(),
            engine_dir.to_string_lossy().into_owned(),
        ],
    );
    #[cfg(not(target_os = "windows"))]
    let (executable, args) = (
        PathBuf::from("/bin/bash"),
        vec![
            setup_dir.join("install.sh").to_string_lossy().into_owned(),
            "--check".into(),
            "--engine-dir".into(),
            engine_dir.to_string_lossy().into_owned(),
        ],
    );

    let output =
        run_with_timeout_in_path(&executable, &args, CHECK_TIMEOUT, Some(path.as_os_str()))
            .map_err(|error| {
                let detail = format!("工具依賴檢查無法完成：{error}");
                append_log(&setup_dir.join(INSTALL_LOG), &detail);
                detail
            })?;
    let output_text = combined_output(&output);
    if !output_text.trim().is_empty() {
        append_log(&setup_dir.join(INSTALL_LOG), output_text.trim());
    }

    let mut missing = parse_missing_dependencies(&output_text);
    if !output.status.success() && missing.is_empty() {
        let detail = first_useful_line(&output_text)
            .unwrap_or_else(|| format!("檢查程序結束代碼 {}", output.status));
        missing.push(format!("依賴檢查失敗：{detail}"));
    }
    if output.status.success() {
        missing.clear();
    }
    Ok(RuntimeCheck { missing })
}

fn install_dependencies(
    setup_dir: &Path,
    engine_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
) -> Result<InstallResult, String> {
    let log_path = setup_dir.join(INSTALL_LOG);
    let marker_path = setup_dir.join(INSTALL_MARKER);
    let result_path = setup_dir.join(INSTALL_RESULT);
    let launch_path = setup_dir.join(INSTALL_LAUNCH);
    remove_if_exists(&marker_path);
    remove_if_exists(&result_path);
    remove_if_exists(&setup_dir.join(INSTALL_PROGRESS));
    let launch = LaunchRecord {
        owner_pid: std::process::id(),
        worker_pid: None,
        started_at: unix_time_seconds(),
    };
    write_json_atomic(&launch_path, &launch)?;

    #[cfg(target_os = "macos")]
    {
        let script = setup_dir.join("install.sh");
        let launch_script = setup_dir.join("launch.sh");
        fs::write(
            &launch_script,
            "#!/bin/bash\nset -u\nexec > >(tee -a \"$2\") 2>&1\nexec /bin/bash \"$1\" --gui --engine-dir \"$3\" --marker-file \"$4\" --result-file \"$5\"\n",
        )
        .map_err(|error| format!("無法準備終端安裝程序：{error}"))?;
        let command_line = [
            Path::new("/bin/bash"),
            launch_script.as_path(),
            script.as_path(),
            log_path.as_path(),
            engine_dir,
            marker_path.as_path(),
            result_path.as_path(),
        ]
        .into_iter()
        .map(|path| shell_quote(&path.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
        let applescript = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            command_line.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let output = run_with_timeout_in_path(
            Path::new("/usr/bin/osascript"),
            &["-e".into(), applescript],
            Duration::from_secs(30),
            Some(installer_path(engine_dir).as_os_str()),
        );
        match output {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                append_log(&log_path, &format!("Terminal 安裝啟動失敗：{detail}"));
                // Terminal may have accepted the command before osascript failed.
                let _ = wait_for_install(engine_dir, setup_dir, None, None, Some(launch), snapshot);
                return read_install_result(&result_path)?.ok_or_else(|| {
                    if detail.is_empty() {
                        format!("無法啟動 Terminal 安裝程序，結束代碼 {}", output.status)
                    } else {
                        format!("無法啟動 Terminal 安裝程序：{detail}")
                    }
                });
            }
            Err(error) => {
                append_log(&log_path, &format!("Terminal 安裝啟動結果不明：{error}"));
                let result =
                    wait_for_install(engine_dir, setup_dir, None, None, Some(launch), snapshot);
                return result?.ok_or_else(|| {
                    "Terminal 啟動結果不明；為避免重複安裝，請確認終端狀態後再重試。".into()
                });
            }
        }
        wait_for_install(engine_dir, setup_dir, None, None, Some(launch), snapshot)?
            .ok_or_else(|| "Terminal 安裝程序沒有回報完成結果。".into())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let script = setup_dir.join(if cfg!(target_os = "windows") {
            "install.ps1"
        } else {
            "install.sh"
        });
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("無法建立安裝記錄：{error}"))?;
        let mut command = installer_command(&script, engine_dir, &marker_path, &result_path);
        command
            .env("PATH", installer_path(engine_dir))
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                log.try_clone()
                    .map_err(|error| format!("無法複製安裝記錄：{error}"))?,
            ))
            .stderr(Stdio::from(log));
        let child = command
            .spawn()
            .map_err(|error| format!("無法啟動工具安裝程序：{error}"))?;
        let mut launch = launch;
        launch.worker_pid = Some(child.id());
        write_json_atomic(&launch_path, &launch)?;
        wait_for_install(
            engine_dir,
            setup_dir,
            None,
            Some(child),
            Some(launch),
            snapshot,
        )?
        .ok_or_else(|| "安裝程序沒有回報完成結果。".into())
    }
}

#[cfg(target_os = "windows")]
fn installer_command(script: &Path, engine_dir: &Path, marker: &Path, result: &Path) -> Command {
    let mut command = background_command("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]);
    command
        .arg(script)
        .arg("-Gui")
        .arg("-EngineDirectory")
        .arg(engine_dir)
        .arg("-MarkerFile")
        .arg(marker)
        .arg("-ResultFile")
        .arg(result)
        .arg("-ProgressFile")
        .arg(result.with_file_name(INSTALL_PROGRESS))
        .arg("-LogFile")
        .arg(result.with_file_name(INSTALL_LOG));
    command
}

#[cfg(target_os = "linux")]
fn installer_command(script: &Path, engine_dir: &Path, marker: &Path, result: &Path) -> Command {
    let mut command = Command::new("/bin/bash");
    command
        .arg(script)
        .args(["--gui", "--engine-dir"])
        .arg(engine_dir)
        .arg("--marker-file")
        .arg(marker)
        .arg("--result-file")
        .arg(result);
    command
}

fn wait_for_install(
    engine_dir: &Path,
    setup_dir: &Path,
    mut expected_pid: Option<u32>,
    mut child: Option<Child>,
    launch: Option<LaunchRecord>,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
) -> Result<Option<InstallResult>, String> {
    let marker_path = setup_dir.join(INSTALL_MARKER);
    let result_path = setup_dir.join(INSTALL_RESULT);
    let launch_deadline = launch
        .as_ref()
        .map(|record| record.started_at.saturating_add(LAUNCH_GRACE.as_secs()));
    let mut child_finished_at = None;
    let mut child_exit = None;
    let mut last_progress = None;
    let mut unknown_process_since: Option<Instant> = None;

    loop {
        if let Ok(Some(progress)) = read_install_progress(&setup_dir.join(INSTALL_PROGRESS)) {
            publish_installing(snapshot, &progress.message);
            last_progress = Some(progress);
        }
        let mut child_alive = false;
        let child_finished = if let Some(process) = child.as_mut() {
            match process.try_wait() {
                Ok(None) => {
                    child_alive = true;
                    false
                }
                Ok(Some(status)) => {
                    child_exit = Some(status);
                    true
                }
                Err(error) => {
                    let _ = process.kill();
                    let _ = process.wait();
                    return Err(format!("無法檢查安裝程序狀態：{error}"));
                }
            }
        } else {
            false
        };
        if child_finished {
            child = None;
            child_finished_at.get_or_insert_with(Instant::now);
        }
        if let Some(result) = read_install_result(&result_path)? {
            remove_if_exists(&marker_path);
            if let Some(mut process) = child.take() {
                // An elevated installer may finish before its waiting launcher exits.
                // Reap it without blocking the completed installation's UI state.
                if let Err(error) = thread::Builder::new()
                    .name("neo-airplay-installer-reaper".into())
                    .spawn(move || {
                        let _ = process.wait();
                    })
                {
                    append_log(
                        &setup_dir.join(INSTALL_LOG),
                        &format!("無法啟動安裝程序回收工作：{error}"),
                    );
                }
            }
            return Ok(Some(result));
        }
        // Reap a directly owned child before probing its PID. On Unix a zombie still
        // answers kill -0 until try_wait collects its exit status.
        let mut unknown_pid = None;
        if let Some(pid) = read_install_marker(&marker_path)? {
            expected_pid = Some(pid);
            match process_presence(pid, engine_dir) {
                ProcessPresence::Alive => {
                    unknown_process_since = None;
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }
                ProcessPresence::Unknown => unknown_pid = Some(pid),
                ProcessPresence::Dead => {}
            }
        }
        if let Some(record) = launch.as_ref() {
            if let Some(pid) = record.worker_pid {
                match process_presence(pid, engine_dir) {
                    ProcessPresence::Alive => child_alive = true,
                    ProcessPresence::Unknown => {
                        unknown_pid.get_or_insert(pid);
                    }
                    ProcessPresence::Dead => {}
                };
            }
        }
        if child_alive {
            unknown_process_since = None;
            thread::sleep(POLL_INTERVAL);
            continue;
        }
        if let Some(pid) = unknown_pid {
            let started_at = unknown_process_since.get_or_insert_with(Instant::now);
            if started_at.elapsed() >= LAUNCH_GRACE {
                let detail = format!(
                    "無法確認安裝程序 {pid} 是否仍在執行；為避免重複安裝，已停止自動重試。"
                );
                append_log(&setup_dir.join(INSTALL_LOG), &detail);
                return Err(detail);
            }
            thread::sleep(POLL_INTERVAL);
            continue;
        }
        unknown_process_since = None;

        if child_finished_at.is_none()
            && launch_deadline.is_some_and(|deadline| unix_time_seconds() < deadline)
        {
            thread::sleep(POLL_INTERVAL);
            continue;
        }

        let grace_started = child_finished_at.get_or_insert_with(Instant::now);
        if grace_started.elapsed() >= RESULT_GRACE {
            let detail =
                interrupted_install_detail(expected_pid, child_exit, last_progress.as_ref());
            append_log(&setup_dir.join(INSTALL_LOG), &detail);
            remove_if_exists(&marker_path);
            remove_if_exists(&setup_dir.join(INSTALL_LAUNCH));
            return Err(detail);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn read_install_progress(path: &Path) -> Result<Option<InstallProgress>, String> {
    match fs::read(path) {
        Ok(contents) => {
            let progress: InstallProgress = serde_json::from_slice(&contents)
                .map_err(|error| format!("安裝階段記錄格式無效：{error}"))?;
            Ok((progress.pid != 0 && !progress.message.trim().is_empty()).then_some(progress))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝階段記錄：{error}")),
    }
}

fn interrupted_install_detail(
    pid: Option<u32>,
    status: Option<std::process::ExitStatus>,
    progress: Option<&InstallProgress>,
) -> String {
    let mut detail = pid
        .map(|pid| format!("安裝程序 {pid} 已結束，但沒有寫入完成結果。"))
        .unwrap_or_else(|| "安裝程序未寫入完成結果。".into());
    if let Some(status) = status {
        detail.push_str(&format!("程序退出狀態：{status}。"));
    }
    if let Some(progress) = progress {
        detail.push_str(&format!("最後階段：{}。", stage_label(&progress.stage)));
    }
    detail
}

fn stage_label(stage: &str) -> &str {
    match stage {
        "checking" => "檢查投影工具",
        "authorizing" => "系統授權",
        "msys2" => "安裝建置工具",
        "updating" => "更新工具套件",
        "packages" => "安裝工具套件",
        "toolchain" => "驗證編譯工具",
        "repairing" => "修復編譯工具",
        "uxplay-download" => "下載 AirPlay 工具",
        "uxplay-build" => "編譯 AirPlay 工具",
        "uxplay-install" => "安裝 AirPlay 工具",
        "scrcpy-download" => "下載 Android 工具",
        "staging" => "整理工具檔案",
        "verifying" => "驗證投影工具",
        "ready" => "準備完成",
        _ => "工具安裝",
    }
}

fn read_install_marker(path: &Path) -> Result<Option<u32>, String> {
    match fs::read_to_string(path) {
        Ok(value) => value
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|pid| *pid != 0)
            .map(Some)
            .ok_or_else(|| "安裝程序 PID 記錄格式無效；為避免啟動重複安裝，請檢查記錄。".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序 PID 記錄：{error}")),
    }
}

fn read_install_result(path: &Path) -> Result<Option<InstallResult>, String> {
    match fs::read(path) {
        Ok(contents) => {
            let result: InstallResult = serde_json::from_slice(&contents)
                .map_err(|error| format!("安裝程序完成記錄格式無效：{error}"))?;
            if result.pid == 0 {
                return Err("安裝程序完成記錄中的 PID 無效。".into());
            }
            Ok(Some(result))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序完成記錄：{error}")),
    }
}

fn read_launch_record(path: &Path) -> Result<Option<LaunchRecord>, String> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map(Some)
            .map_err(|error| format!("安裝程序啟動記錄格式無效：{error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序啟動記錄：{error}")),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessPresence {
    Alive,
    Dead,
    Unknown,
}

fn process_presence(pid: u32, _engine_dir: &Path) -> ProcessPresence {
    if pid == 0 {
        return ProcessPresence::Dead;
    }
    #[cfg(unix)]
    {
        let output = run_with_timeout_in_path(
            Path::new("/bin/kill"),
            &["-0".into(), pid.to_string()],
            PROCESS_PROBE_TIMEOUT,
            None,
        );
        match output {
            Ok(output) if output.status.success() => ProcessPresence::Alive,
            Ok(output) => {
                let detail = combined_output(&output).to_ascii_lowercase();
                if detail.contains("no such process") {
                    ProcessPresence::Dead
                } else {
                    ProcessPresence::Unknown
                }
            }
            Err(_) => ProcessPresence::Unknown,
        }
    }
    #[cfg(target_os = "windows")]
    {
        let output = run_with_timeout_in_path(
            Path::new("tasklist.exe"),
            &[
                "/FI".into(),
                format!("PID eq {pid}"),
                "/FO".into(),
                "CSV".into(),
                "/NH".into(),
            ],
            PROCESS_PROBE_TIMEOUT,
            Some(installer_path(_engine_dir).as_os_str()),
        );
        match output {
            Ok(output) if output.status.success() && tasklist_contains_pid(&output, pid) => {
                ProcessPresence::Alive
            }
            Ok(output) if output.status.success() => ProcessPresence::Dead,
            Ok(_) | Err(_) => ProcessPresence::Unknown,
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn tasklist_contains_pid(output: &Output, pid: u32) -> bool {
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.split(',')
            .nth(1)
            .map(|field| field.trim().trim_matches('"') == pid.to_string())
            .unwrap_or(false)
    })
}

fn launch_is_recent(record: &LaunchRecord) -> bool {
    unix_time_seconds().saturating_sub(record.started_at) < LAUNCH_GRACE.as_secs()
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let temporary_path = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary_path, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary_path, path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        error.to_string()
    })
}

fn remove_if_exists(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

fn append_log(path: &Path, message: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn combined_output(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    text
}

fn parse_missing_dependencies(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("-").map(str::trim))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn first_useful_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("Missing runtime dependencies:"))
        .map(str::to_owned)
}

fn publish_installing(snapshot: &Arc<Mutex<DependencySnapshot>>, message: &str) {
    if let Ok(mut snapshot) = snapshot.lock() {
        snapshot.status = "installing".into();
        snapshot.message = message.to_owned();
    }
}

fn unix_time_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "../../../desktop-test/backend/unit-tests/dependencies.rs"]
mod tests;
