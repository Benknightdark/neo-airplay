use crate::models::{
    AndroidDevice, AndroidDeviceStatus, AppLogEntry, DashboardSnapshot, EngineSnapshot,
    EngineStatus, LogLevel, ReceiverSettings,
};
use chrono::Local;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_RECENT_LOGS: usize = 300;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(15);

pub struct AppState {
    settings_path: PathBuf,
    settings: Mutex<ReceiverSettings>,
    logger: AppLogger,
    airplay: EngineController,
    android: EngineController,
    devices: Mutex<Vec<AndroidDevice>>,
}

impl AppState {
    pub fn new(
        config_dir: PathBuf,
        log_dir: PathBuf,
        resource_dir: PathBuf,
    ) -> Result<Self, String> {
        fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
        fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

        let settings_path = config_dir.join("settings.json");
        let logger = AppLogger::new(log_dir);
        Ok(Self {
            settings_path: settings_path.clone(),
            settings: Mutex::new(load_settings(&settings_path)),
            airplay: EngineController::new(
                EngineKind::AirPlay,
                resource_dir.clone(),
                logger.clone(),
            ),
            android: EngineController::new(EngineKind::Android, resource_dir, logger.clone()),
            logger,
            devices: Mutex::new(Vec::new()),
        })
    }

    pub fn settings(&self) -> Result<ReceiverSettings, String> {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .map_err(|_| "設定狀態無法讀取。".to_owned())
    }

    pub fn save_settings(&self, settings: ReceiverSettings) -> Result<(), String> {
        let settings = settings.sanitized();
        write_settings(&self.settings_path, &settings)?;
        let mut current = self
            .settings
            .lock()
            .map_err(|_| "設定狀態無法更新。".to_owned())?;
        *current = settings;
        Ok(())
    }

    pub fn dashboard(&self) -> Result<DashboardSnapshot, String> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| "Android 裝置狀態無法讀取。".to_owned())?
            .clone();
        Ok(DashboardSnapshot {
            settings: self.settings()?,
            renderer_options: crate::models::available_renderers()
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            airplay: self.airplay.snapshot(),
            android: self.android.snapshot(),
            devices,
            logs: self.logger.recent(),
        })
    }

    pub fn start_airplay(&self) -> Result<EngineSnapshot, String> {
        let args = airplay_arguments(&self.settings()?);
        Ok(self.airplay.start(&args))
    }

    pub fn stop_airplay(&self) -> Result<EngineSnapshot, String> {
        Ok(self.airplay.stop())
    }

    pub fn refresh_android_devices(&self) -> Result<Vec<AndroidDevice>, String> {
        let Some(adb_path) = find_executable(&self.android.resource_dir, "adb", true) else {
            let message = "找不到 ADB。請安裝 Android Platform Tools 或 scrcpy。".to_owned();
            self.logger.write(LogLevel::Error, &message);
            let mut devices = self
                .devices
                .lock()
                .map_err(|_| "Android 裝置狀態無法更新。".to_owned())?;
            devices.clear();
            self.android
                .set_external_status(EngineStatus::DependencyMissing, message);
            return Ok(devices.clone());
        };

        match run_with_timeout(
            &adb_path,
            &["devices".to_owned(), "-l".to_owned()],
            PROCESS_TIMEOUT,
        ) {
            Ok(output) if output.status.success() => {
                let parsed = parse_adb_devices(&String::from_utf8_lossy(&output.stdout));
                let mut devices = self
                    .devices
                    .lock()
                    .map_err(|_| "Android 裝置狀態無法更新。".to_owned())?;
                *devices = parsed;
                self.android.set_available();
                if devices.is_empty() {
                    self.logger.write(
                        LogLevel::Information,
                        "找不到 Android 裝置，請連接 USB 或已配對的網路裝置。",
                    );
                }
                Ok(devices.clone())
            }
            Ok(output) => {
                let detail = first_non_empty_line(&output.stderr)
                    .or_else(|| first_non_empty_line(&output.stdout))
                    .unwrap_or_else(|| format!("結束代碼 {}", output.status));
                let message = format!("ADB 裝置查詢失敗：{detail}");
                self.logger.write(LogLevel::Error, &message);
                let mut devices = self
                    .devices
                    .lock()
                    .map_err(|_| "Android 裝置狀態無法更新。".to_owned())?;
                devices.clear();
                self.android
                    .set_external_status(EngineStatus::Faulted, message);
                Ok(devices.clone())
            }
            Err(error) => {
                let message = format!("無法查詢 Android 裝置：{error}");
                self.logger.write(LogLevel::Error, &message);
                let mut devices = self
                    .devices
                    .lock()
                    .map_err(|_| "Android 裝置狀態無法更新。".to_owned())?;
                devices.clear();
                self.android
                    .set_external_status(EngineStatus::Faulted, message);
                Ok(devices.clone())
            }
        }
    }

    pub fn start_android_mirror(&self, serial: &str) -> Result<EngineSnapshot, String> {
        if serial.trim().is_empty() {
            return Err("請先選擇 Android 裝置。".to_owned());
        }
        let devices = self
            .devices
            .lock()
            .map_err(|_| "Android 裝置狀態無法讀取。".to_owned())?;
        let Some(device) = devices
            .iter()
            .find(|device| device.serial == serial && device.is_ready())
        else {
            return Err(
                "找不到可用的 Android 裝置，請重新整理裝置清單並確認手機已授權。".to_owned(),
            );
        };

        let device_name = device.display_name();
        let settings = self.settings()?;
        let mut args = vec!["--serial".to_owned(), device.serial.clone()];
        if settings.fullscreen {
            args.push("--fullscreen".to_owned());
        }
        drop(devices);

        let snapshot = self.android.start(&args);
        if snapshot.status == EngineStatus::Running {
            self.logger.write(
                LogLevel::Information,
                &format!("正在投影 Android 裝置「{device_name}」。"),
            );
        }
        Ok(snapshot)
    }

    pub fn stop_android_mirror(&self) -> Result<EngineSnapshot, String> {
        Ok(self.android.stop())
    }

    pub fn log_info(&self, message: &str) {
        self.logger.write(LogLevel::Information, message);
    }
}

fn load_settings(path: &Path) -> ReceiverSettings {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ReceiverSettings>(&bytes).ok())
        .unwrap_or_default()
        .sanitized()
}

fn write_settings(path: &Path, settings: &ReceiverSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "設定目錄無法解析。".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| format!("無法建立設定目錄：{error}"))?;
    let temporary_path = path.with_extension("json.tmp");
    let serialized =
        serde_json::to_vec_pretty(settings).map_err(|error| format!("設定序列化失敗：{error}"))?;
    fs::write(&temporary_path, serialized).map_err(|error| format!("設定寫入失敗：{error}"))?;
    fs::rename(&temporary_path, path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        format!("設定檔更新失敗：{error}")
    })
}

#[derive(Clone)]
struct AppLogger {
    log_dir: PathBuf,
    entries: Arc<Mutex<VecDeque<AppLogEntry>>>,
}

impl AppLogger {
    fn new(log_dir: PathBuf) -> Self {
        let log_path = log_dir.join(format!("{}.jsonl", Local::now().format("%Y-%m-%d")));
        let mut entries = VecDeque::new();
        if let Ok(file) = fs::File::open(log_path) {
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if let Ok(entry) = serde_json::from_str::<AppLogEntry>(&line) {
                    entries.push_back(entry);
                    while entries.len() > MAX_RECENT_LOGS {
                        entries.pop_front();
                    }
                }
            }
        }
        Self {
            log_dir,
            entries: Arc::new(Mutex::new(entries)),
        }
    }

    fn write(&self, level: LogLevel, message: &str) {
        let entry = AppLogEntry {
            timestamp: Local::now(),
            level,
            message: message.replace(['\r', '\n'], " ").trim().to_owned(),
        };
        append_log(&self.log_dir, &entry);
        if let Ok(mut entries) = self.entries.lock() {
            push_recent(&mut entries, entry);
        }
    }

    fn recent(&self) -> Vec<AppLogEntry> {
        self.entries
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum EngineKind {
    AirPlay,
    Android,
}

struct EngineInner {
    child: Option<Child>,
    snapshot: EngineSnapshot,
}

struct EngineController {
    kind: EngineKind,
    resource_dir: PathBuf,
    inner: Mutex<EngineInner>,
    logger: AppLogger,
}

impl EngineController {
    fn new(kind: EngineKind, resource_dir: PathBuf, logger: AppLogger) -> Self {
        let message = match kind {
            EngineKind::AirPlay => "AirPlay 接收器尚未啟動。",
            EngineKind::Android => "Android 投影尚未啟動。",
        };
        Self {
            kind,
            resource_dir,
            inner: Mutex::new(EngineInner {
                child: None,
                snapshot: EngineSnapshot::stopped(message),
            }),
            logger,
        }
    }

    fn start(&self, args: &[String]) -> EngineSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return EngineSnapshot::stopped("接收器狀態無法更新。");
        };
        if let Some(child) = inner.child.as_mut() {
            match child.try_wait() {
                Ok(None) => return inner.snapshot.clone(),
                Ok(Some(status)) => {
                    inner.child = None;
                    let (status, message, level) = exit_snapshot(self.kind, status);
                    inner.snapshot = EngineSnapshot {
                        status,
                        message: message.clone(),
                        process_id: None,
                    };
                    self.logger.write(level, &message);
                }
                Err(error) => {
                    let message = format!("無法檢查投影引擎狀態：{error}");
                    inner.snapshot = EngineSnapshot {
                        status: EngineStatus::Faulted,
                        message: message.clone(),
                        process_id: inner.child.as_ref().map(Child::id),
                    };
                    self.logger.write(LogLevel::Error, &message);
                    return inner.snapshot.clone();
                }
            }
        }

        let Some(executable) = self.executable() else {
            let message = match self.kind {
                EngineKind::AirPlay => "找不到 UxPlay。請安裝 UxPlay 及其 GStreamer 執行環境。",
                EngineKind::Android => "找不到 scrcpy。請安裝 scrcpy 與 Android Platform Tools。",
            }
            .to_owned();
            inner.snapshot = EngineSnapshot {
                status: EngineStatus::DependencyMissing,
                message: message.clone(),
                process_id: None,
            };
            self.logger.write(LogLevel::Error, &message);
            return inner.snapshot.clone();
        };

        let mut command = Command::new(&executable);
        command
            .args(args)
            .current_dir(executable.parent().unwrap_or(&self.resource_dir))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match command.spawn() {
            Ok(mut child) => {
                let process_id = child.id();
                self.capture_output(&mut child);
                let name = match self.kind {
                    EngineKind::AirPlay => "UxPlay",
                    EngineKind::Android => "scrcpy",
                };
                let message = format!("{name} 已啟動。");
                inner.child = Some(child);
                inner.snapshot = EngineSnapshot {
                    status: EngineStatus::Running,
                    message: message.clone(),
                    process_id: Some(process_id),
                };
                self.logger.write(LogLevel::Information, &message);
            }
            Err(error) => {
                let message = format!("無法啟動引擎：{error}");
                inner.snapshot = EngineSnapshot {
                    status: EngineStatus::Faulted,
                    message: message.clone(),
                    process_id: None,
                };
                self.logger.write(LogLevel::Error, &message);
            }
        }
        inner.snapshot.clone()
    }

    fn snapshot(&self) -> EngineSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return EngineSnapshot::stopped("接收器狀態無法讀取。");
        };
        let observation = inner.child.as_mut().map(Child::try_wait);
        match observation {
            Some(Ok(Some(exit_status))) => {
                inner.child = None;
                let (status, message, level) = exit_snapshot(self.kind, exit_status);
                inner.snapshot = EngineSnapshot {
                    status,
                    message: message.clone(),
                    process_id: None,
                };
                self.logger.write(level, &message);
            }
            Some(Err(error)) => {
                let process_id = inner.child.as_ref().map(Child::id);
                let message = format!("無法檢查投影引擎狀態：{error}");
                inner.snapshot = EngineSnapshot {
                    status: EngineStatus::Faulted,
                    message: message.clone(),
                    process_id,
                };
                self.logger.write(LogLevel::Error, &message);
            }
            _ => {}
        }
        inner.snapshot.clone()
    }

    fn stop(&self) -> EngineSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return EngineSnapshot::stopped("接收器狀態無法更新。");
        };
        let Some(mut child) = inner.child.take() else {
            if inner.snapshot.status != EngineStatus::DependencyMissing {
                let message = match self.kind {
                    EngineKind::AirPlay => "AirPlay 接收器已停止。",
                    EngineKind::Android => "Android 投影已停止。",
                };
                inner.snapshot = EngineSnapshot::stopped(message);
            }
            return inner.snapshot.clone();
        };

        inner.snapshot.status = EngineStatus::Stopping;
        inner.snapshot.message = "正在停止投影引擎。".to_owned();
        inner.snapshot.process_id = Some(child.id());
        let stop_result = match child.try_wait() {
            Ok(Some(status)) => Ok(status),
            Ok(None) => child.kill().and_then(|()| child.wait()),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        };
        let name = match self.kind {
            EngineKind::AirPlay => "UxPlay",
            EngineKind::Android => "scrcpy",
        };
        let (status, message, level) = match stop_result {
            Ok(_) => (
                EngineStatus::Stopped,
                format!("{name} 已停止。"),
                LogLevel::Information,
            ),
            Err(error) => (
                EngineStatus::Faulted,
                format!("停止 {name} 失敗：{error}"),
                LogLevel::Error,
            ),
        };
        inner.snapshot = EngineSnapshot {
            status,
            message: message.clone(),
            process_id: None,
        };
        self.logger.write(level, &message);
        inner.snapshot.clone()
    }

    fn set_external_status(&self, status: EngineStatus, message: String) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(child) = inner.child.as_mut() {
                match child.try_wait() {
                    Ok(None) | Err(_) => return,
                    Ok(Some(_)) => inner.child = None,
                }
            }
            inner.snapshot = EngineSnapshot {
                status,
                message,
                process_id: None,
            };
        }
    }

    fn set_available(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(child) = inner.child.as_mut() {
                match child.try_wait() {
                    Ok(None) | Err(_) => return,
                    Ok(Some(_)) => inner.child = None,
                }
            }
            if matches!(
                inner.snapshot.status,
                EngineStatus::DependencyMissing | EngineStatus::Faulted
            ) {
                let message = match self.kind {
                    EngineKind::AirPlay => "AirPlay 接收器尚未啟動。",
                    EngineKind::Android => "Android 投影尚未啟動。",
                };
                inner.snapshot = EngineSnapshot::stopped(message);
            }
        }
    }

    fn executable(&self) -> Option<PathBuf> {
        let (name, android) = match self.kind {
            EngineKind::AirPlay => ("uxplay", false),
            EngineKind::Android => ("scrcpy", true),
        };
        find_executable(&self.resource_dir, name, android)
    }

    fn capture_output(&self, child: &mut Child) {
        let name = match self.kind {
            EngineKind::AirPlay => "UxPlay",
            EngineKind::Android => "scrcpy",
        };
        if let Some(stdout) = child.stdout.take() {
            let logger = self.logger.clone();
            let name = name.to_owned();
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    logger.write(LogLevel::Information, &format!("{name}: {line}"));
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let logger = self.logger.clone();
            let name = name.to_owned();
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    logger.write(LogLevel::Warning, &format!("{name}: {line}"));
                }
            });
        }
    }
}

impl Drop for EngineController {
    fn drop(&mut self) {
        if let Ok(inner) = self.inner.get_mut() {
            if let Some(child) = inner.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn find_executable(resource_dir: &Path, name: &str, android: bool) -> Option<PathBuf> {
    let resource_dirs = if android {
        vec![
            resource_dir.join("engine/android"),
            resource_dir.join("engine"),
        ]
    } else {
        vec![resource_dir.join("engine"), resource_dir.to_owned()]
    };
    let names = executable_names(name);
    for directory in resource_dirs {
        for candidate_name in &names {
            let candidate = directory.join(candidate_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let mut search_directories = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    search_directories.extend(system_search_paths());
    for directory in search_directories {
        for candidate_name in &names {
            let candidate = directory.join(candidate_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn system_search_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/snap/bin"),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        Vec::new()
    }
}

fn executable_names(name: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![format!("{name}.exe"), name.to_owned()]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![name.to_owned()]
    }
}

fn run_with_timeout(
    executable: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<Output, String> {
    let mut child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout_reader = child.stdout.take().map(read_pipe);
    let stderr_reader = child.stderr.take().map(read_pipe);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = finish_pipe(stdout_reader);
                let stderr = finish_pipe(stderr_reader);
                return Ok(Output {
                    status,
                    stdout: stdout?,
                    stderr: stderr?,
                });
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = finish_pipe(stdout_reader);
                let _ = finish_pipe(stderr_reader);
                return Err(format!("無法檢查 ADB 程序狀態：{error}"));
            }
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = finish_pipe(stdout_reader);
            let _ = finish_pipe(stderr_reader);
            return Err("ADB 裝置查詢逾時。".to_owned());
        }
        thread::sleep(Duration::from_millis(40));
    }
}

fn read_pipe<R: Read + Send + 'static>(
    mut reader: R,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn finish_pipe(
    reader: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
) -> Result<Vec<u8>, String> {
    reader
        .map(|reader| {
            reader
                .join()
                .map_err(|_| "無法讀取 ADB 程序輸出。".to_owned())?
                .map_err(|error| format!("無法讀取 ADB 程序輸出：{error}"))
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn exit_snapshot(
    kind: EngineKind,
    exit_status: std::process::ExitStatus,
) -> (EngineStatus, String, LogLevel) {
    match (kind, exit_status.success()) {
        (EngineKind::AirPlay, true) => (
            EngineStatus::Stopped,
            "AirPlay 接收器已停止。".to_owned(),
            LogLevel::Information,
        ),
        (EngineKind::Android, true) => (
            EngineStatus::Stopped,
            "Android 投影已停止。".to_owned(),
            LogLevel::Information,
        ),
        (_, false) => (
            EngineStatus::Faulted,
            format!("投影引擎已結束，結束代碼為 {exit_status}。"),
            LogLevel::Warning,
        ),
    }
}

fn parse_adb_devices(output: &str) -> Vec<AndroidDevice> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            let state = fields.next()?;
            if serial == "List" || serial.starts_with('*') {
                return None;
            }
            let status = match state {
                "device" => AndroidDeviceStatus::Ready,
                "unauthorized" => AndroidDeviceStatus::Unauthorized,
                "offline" => AndroidDeviceStatus::Offline,
                _ => AndroidDeviceStatus::Unknown,
            };
            let model = fields
                .find_map(|field| field.strip_prefix("model:"))
                .unwrap_or(serial)
                .replace('_', " ");
            Some(AndroidDevice {
                serial: serial.to_owned(),
                model,
                status,
            })
        })
        .collect()
}

fn airplay_arguments(settings: &ReceiverSettings) -> Vec<String> {
    let mut args = vec![
        "-n".to_owned(),
        settings.receiver_name.clone(),
        "-nh".to_owned(),
    ];
    if settings.fullscreen {
        args.push("-fs".to_owned());
    }
    if let Some(renderer) = renderer_argument(&settings.renderer) {
        args.extend(["-vs".to_owned(), renderer.to_owned()]);
    }
    args
}

fn renderer_argument(renderer: &str) -> Option<&'static str> {
    match renderer {
        "D3D11" => Some("d3d11videosink"),
        "D3D12" => Some("d3d12videosink"),
        "osxvideosink" => Some("osxvideosink"),
        "glimagesink" => Some("glimagesink"),
        "ximagesink" => Some("ximagesink"),
        "waylandsink" => Some("waylandsink"),
        _ => None,
    }
}

fn first_non_empty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

fn append_log(log_dir: &Path, entry: &AppLogEntry) {
    let path = log_dir.join(format!("{}.jsonl", entry.timestamp.format("%Y-%m-%d")));
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_vec(entry) {
            let _ = file.write_all(&line);
            let _ = file.write_all(b"\n");
        }
    }
}

fn push_recent(entries: &mut VecDeque<AppLogEntry>, entry: AppLogEntry) {
    entries.push_back(entry);
    while entries.len() > MAX_RECENT_LOGS {
        entries.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        airplay_arguments, load_settings, parse_adb_devices, renderer_argument, write_settings,
        AppLogger, EngineController, EngineKind,
    };
    use crate::models::{AndroidDeviceStatus, EngineSnapshot, EngineStatus, ReceiverSettings};
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn parses_android_devices_and_states() {
        let devices = parse_adb_devices(
            "List of devices attached\nusb-1 device product:foo model:Pixel_8\nusb-2 unauthorized usb:1-2\nusb-3 offline transport_id:8\n",
        );
        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].model, "Pixel 8");
        assert_eq!(devices[0].status, AndroidDeviceStatus::Ready);
        assert_eq!(devices[1].status, AndroidDeviceStatus::Unauthorized);
        assert_eq!(devices[2].status, AndroidDeviceStatus::Offline);
    }

    #[test]
    fn ignores_adb_server_messages() {
        assert!(
            parse_adb_devices("* daemon started successfully\nList of devices attached\n")
                .is_empty()
        );
    }

    #[test]
    fn keeps_receiver_name_in_a_separate_argument() {
        let settings = ReceiverSettings {
            receiver_name: "Living Room; safe".to_owned(),
            renderer: "Auto".to_owned(),
            fullscreen: true,
            start_on_login: false,
        };
        assert_eq!(
            airplay_arguments(&settings),
            ["-n", "Living Room; safe", "-nh", "-fs"]
        );
    }

    #[test]
    fn renderer_names_are_allowlisted() {
        assert_eq!(renderer_argument("D3D11"), Some("d3d11videosink"));
        assert_eq!(renderer_argument("Auto"), None);
        assert_eq!(renderer_argument("arbitrary"), None);
    }

    #[test]
    fn settings_roundtrip_through_json_file() {
        let directory = test_directory("settings");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.json");
        let settings = ReceiverSettings {
            receiver_name: "Studio".to_owned(),
            renderer: "Auto".to_owned(),
            fullscreen: true,
            start_on_login: true,
        };

        write_settings(&path, &settings).unwrap();

        assert_eq!(load_settings(&path), settings);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn adb_refresh_error_does_not_hide_a_running_engine() {
        let directory = test_directory("running-engine");
        fs::create_dir_all(&directory).unwrap();
        let logger = AppLogger::new(directory.clone());
        let controller = EngineController::new(EngineKind::Android, PathBuf::new(), logger);
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "state::tests::engine_test_child_process",
                "--nocapture",
            ])
            .env("NEO_AIRPLAY_HOLD_ENGINE_TEST_CHILD", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let process_id = child.id();
        {
            let mut inner = controller.inner.lock().unwrap();
            inner.child = Some(child);
            inner.snapshot = EngineSnapshot {
                status: EngineStatus::Running,
                message: "scrcpy 正在執行。".to_owned(),
                process_id: Some(process_id),
            };
        }

        controller.set_external_status(EngineStatus::Faulted, "ADB 查詢失敗。".to_owned());
        assert_eq!(controller.snapshot().status, EngineStatus::Running);
        assert_eq!(controller.stop().status, EngineStatus::Stopped);

        drop(controller);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn engine_test_child_process() {
        if std::env::var_os("NEO_AIRPLAY_HOLD_ENGINE_TEST_CHILD").is_some() {
            std::thread::sleep(Duration::from_secs(30));
        }
    }

    fn test_directory(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("neo-airplay-{name}-{}-{nonce}", std::process::id()))
    }
}
