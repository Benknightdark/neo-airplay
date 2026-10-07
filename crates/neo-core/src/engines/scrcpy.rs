use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::{
    AndroidDevice, AndroidDeviceStatus, AppLogger, CommandOutput, EngineStatus, LogLevel, Platform,
    ProcessSpawner, ProcessSpec, ReceiverSettings, ReceiverState,
};

use super::ProcessSupervisor;

const ADB_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub struct AndroidMirrorEngine {
    platform: Platform,
    bundled_engine_directory: Option<PathBuf>,
    user_engine_directory: PathBuf,
    required_files: Vec<String>,
    supervisor: ProcessSupervisor,
    logger: Arc<dyn AppLogger>,
    spawner: Arc<dyn ProcessSpawner>,
    lifecycle: Mutex<()>,
    devices: Mutex<Vec<AndroidDevice>>,
}

impl AndroidMirrorEngine {
    pub fn new(
        platform: Platform,
        bundled_engine_directory: Option<PathBuf>,
        user_engine_directory: PathBuf,
        required_files: Vec<String>,
        logger: Arc<dyn AppLogger>,
        spawner: Arc<dyn ProcessSpawner>,
    ) -> Self {
        Self {
            platform,
            bundled_engine_directory,
            user_engine_directory,
            required_files,
            supervisor: ProcessSupervisor::new(
                "請連接 Android 裝置後重新整理清單。",
                Arc::clone(&logger),
                Arc::clone(&spawner),
            ),
            logger,
            spawner,
            lifecycle: Mutex::new(()),
            devices: Mutex::new(Vec::new()),
        }
    }

    pub fn status(&self) -> EngineStatus {
        self.supervisor.status()
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineStatus> {
        self.supervisor.subscribe()
    }

    pub async fn devices(&self) -> Vec<AndroidDevice> {
        self.devices.lock().await.clone()
    }

    pub async fn refresh_devices(&self) -> Vec<AndroidDevice> {
        let _lifecycle = self.lifecycle.lock().await;
        let directory = match self.runtime_directory() {
            Ok(directory) => directory,
            Err(message) => {
                self.devices.lock().await.clear();
                self.set_refresh_failure(ReceiverState::DependencyMissing, message)
                    .await;
                return Vec::new();
            }
        };
        let adb = directory.join(self.adb_executable_name());
        let spec = command_spec(adb, directory.clone(), ["devices", "-l"]);
        let output = match self.spawner.run_to_end(spec, ADB_COMMAND_TIMEOUT).await {
            Ok(output) => output,
            Err(error) => {
                let message = if error.kind() == std::io::ErrorKind::TimedOut {
                    "Android 裝置查詢逾時（15 秒）。".to_owned()
                } else {
                    format!("無法查詢 Android 裝置：{error}")
                };
                self.devices.lock().await.clear();
                self.set_refresh_failure(ReceiverState::Faulted, message)
                    .await;
                return Vec::new();
            }
        };

        if output.exit_code != Some(0) {
            let detail = first_non_empty_line(&output.stderr, &output.stdout);
            let message = if detail.is_empty() {
                format!("ADB 裝置查詢失敗，結束代碼：{}。", exit_code_label(&output))
            } else {
                format!("ADB 裝置查詢失敗：{detail}")
            };
            self.devices.lock().await.clear();
            self.set_refresh_failure(ReceiverState::Faulted, message)
                .await;
            return Vec::new();
        }

        let devices = parse_adb_devices(&output.stdout);
        *self.devices.lock().await = devices.clone();
        if self.supervisor.is_running().await {
            self.supervisor.set_status(
                ReceiverState::Running,
                "Android 投影仍在執行；裝置清單已更新。",
            );
        } else {
            let mut message = discovery_message(&devices);
            if self.platform == Platform::Linux
                && output
                    .stdout
                    .lines()
                    .any(|line| line.to_ascii_lowercase().contains("no permissions"))
            {
                message.push_str(
                    " Linux 未授權存取 Android 裝置；請安裝發行版的 Android udev rules，然後重新連接裝置。",
                );
            }
            self.supervisor
                .set_status(ReceiverState::Stopped, message.clone());
            self.logger.write(LogLevel::Information, message).await;
        }
        devices
    }

    pub async fn start(&self, serial: &str, settings: &ReceiverSettings) -> EngineStatus {
        let _lifecycle = self.lifecycle.lock().await;
        if self.supervisor.is_running().await {
            return self.status();
        }

        self.supervisor.set_status(
            ReceiverState::Starting,
            "正在啟動 Android 投影。請留意手機上的偵錯授權提示。",
        );
        let directory = match self.runtime_directory() {
            Ok(directory) => directory,
            Err(message) => {
                self.supervisor
                    .set_status(ReceiverState::DependencyMissing, message.clone());
                self.logger.write(LogLevel::Error, message).await;
                return self.status();
            }
        };

        let device = self
            .devices
            .lock()
            .await
            .iter()
            .find(|device| device.status == AndroidDeviceStatus::Ready && device.serial == serial)
            .cloned();
        let Some(device) = device else {
            let message = "找不到可用的 Android 裝置。請重新整理清單，並確認手機已授權 USB 偵錯。";
            self.supervisor.set_status(ReceiverState::Faulted, message);
            self.logger
                .write(LogLevel::Warning, message.to_owned())
                .await;
            return self.status();
        };

        let executable = directory.join(self.scrcpy_executable_name());
        let spec = scrcpy_process_spec(&executable, &directory, &device.serial, settings);
        self.logger
            .write(
                LogLevel::Information,
                format!("scrcpy 正在啟動，裝置序號為 {}。", device.serial),
            )
            .await;
        self.supervisor
            .start_process(
                spec,
                "scrcpy",
                format!("正在投影 Android 裝置「{}」。", device.display_name()),
            )
            .await
    }

    pub async fn stop(&self) -> EngineStatus {
        let _lifecycle = self.lifecycle.lock().await;
        self.supervisor.stop("Android 投影已停止。").await
    }

    fn runtime_directory(&self) -> Result<PathBuf, String> {
        if self.required_files.is_empty() {
            return Err("Android 投影元件檔案清單未隨程式提供。請重新建置應用程式。".to_owned());
        }

        let mut candidates = Vec::new();
        if let Some(bundled_directory) = &self.bundled_engine_directory {
            candidates.push(bundled_directory.join("android"));
        }
        candidates.push(self.user_engine_directory.join("android"));
        if let Some(directory) = candidates.iter().find(|directory| {
            self.required_files
                .iter()
                .all(|file| directory.join(file).is_file())
        }) {
            return Ok(directory.clone());
        }

        let user_directory = self.user_engine_directory.join("android");
        let missing = self
            .required_files
            .iter()
            .filter(|file| !user_directory.join(file).is_file())
            .cloned()
            .collect::<Vec<_>>();
        Err(format!(
            "Android 投影元件不完整，缺少：{}。請依 README 的 staging 指引準備引擎。",
            missing.join("、")
        ))
    }

    fn adb_executable_name(&self) -> &'static str {
        if self.platform == Platform::Windows {
            "adb.exe"
        } else {
            "adb"
        }
    }

    fn scrcpy_executable_name(&self) -> &'static str {
        if self.platform == Platform::Windows {
            "scrcpy.exe"
        } else {
            "scrcpy"
        }
    }

    async fn set_refresh_failure(&self, failure_state: ReceiverState, message: String) {
        if self.supervisor.is_running().await {
            self.supervisor.set_status(
                ReceiverState::Running,
                format!("Android 投影仍在執行，但無法更新裝置清單：{message}"),
            );
        } else {
            self.supervisor.set_status(failure_state, message.clone());
        }
        self.logger.write(LogLevel::Error, message).await;
    }
}

pub fn parse_adb_devices(output: &str) -> Vec<AndroidDevice> {
    output.lines().filter_map(parse_adb_device_line).collect()
}

fn parse_adb_device_line(line: &str) -> Option<AndroidDevice> {
    let mut columns = line.split_whitespace();
    let serial = columns.next()?;
    let state = columns.next()?;
    if serial.eq_ignore_ascii_case("List") || serial.starts_with('*') {
        return None;
    }

    let status = match state {
        "device" => AndroidDeviceStatus::Ready,
        "unauthorized" => AndroidDeviceStatus::Unauthorized,
        "offline" => AndroidDeviceStatus::Offline,
        _ => AndroidDeviceStatus::Unknown,
    };
    let model = columns
        .filter_map(|column| {
            column
                .split_once(':')
                .and_then(|(name, value)| name.eq_ignore_ascii_case("model").then_some(value))
        })
        .next()
        .map(|model| model.replace('_', " "))
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| serial.to_owned());

    Some(AndroidDevice {
        serial: serial.to_owned(),
        model,
        status,
    })
}

fn scrcpy_process_spec(
    executable: &std::path::Path,
    directory: &std::path::Path,
    serial: &str,
    settings: &ReceiverSettings,
) -> ProcessSpec {
    let mut args = vec![
        OsString::from("--serial"),
        OsString::from(serial),
        OsString::from("--window-title"),
        OsString::from(format!("Neo AirPlay - Android ({serial})")),
    ];
    if settings.fullscreen {
        args.push(OsString::from("--fullscreen"));
    }
    args.push(OsString::from(if settings.android_hw_decode {
        "--hwdec=auto"
    } else {
        "--hwdec=disabled"
    }));

    ProcessSpec {
        program: executable.to_path_buf(),
        args,
        cwd: directory.to_path_buf(),
        env: path_environment(directory),
    }
}

fn command_spec<const N: usize>(
    program: PathBuf,
    directory: PathBuf,
    args: [&str; N],
) -> ProcessSpec {
    ProcessSpec {
        program,
        args: args.into_iter().map(OsString::from).collect(),
        cwd: directory.clone(),
        env: path_environment(&directory),
    }
}

fn path_environment(directory: &std::path::Path) -> Vec<(OsString, OsString)> {
    let mut paths = vec![directory.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths)
        .map(|path| vec![(OsString::from("PATH"), path)])
        .unwrap_or_default()
}

fn first_non_empty_line(first: &str, second: &str) -> String {
    first
        .lines()
        .chain(second.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn exit_code_label(output: &CommandOutput) -> String {
    output
        .exit_code
        .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
}

fn discovery_message(devices: &[AndroidDevice]) -> String {
    let ready_count = devices
        .iter()
        .filter(|device| device.status == AndroidDeviceStatus::Ready)
        .count();
    if ready_count > 0 {
        return format!("找到 {ready_count} 台可投影的 Android 裝置。");
    }
    if devices
        .iter()
        .any(|device| device.status == AndroidDeviceStatus::Unauthorized)
    {
        return "Android 裝置尚未授權。請解鎖手機並允許 USB 偵錯。".to_owned();
    }
    if devices
        .iter()
        .any(|device| device.status == AndroidDeviceStatus::Offline)
    {
        return "ADB 找到離線裝置。請重新連接 USB，或確認手機與電腦的無線偵錯連線。".to_owned();
    }
    "目前沒有可投影的 Android 裝置。請連接 USB，或連上已配對的無線偵錯裝置。".to_owned()
}

#[cfg(test)]
mod tests {
    use crate::AndroidDeviceStatus;

    use super::{discovery_message, parse_adb_devices};

    #[test]
    fn parses_usb_wireless_and_unavailable_devices() {
        let devices = parse_adb_devices(
            "List of devices attached\nusb-123 device product:pixel model:Pixel_8 transport_id:1\nunauthorized-456 unauthorized usb:1-1\n192.168.1.20:5555 device model:Pixel_8_Pro\noffline-789 offline usb:1-2\n",
        );

        assert_eq!(devices.len(), 4);
        assert_eq!(devices[0].serial, "usb-123");
        assert_eq!(devices[0].model, "Pixel 8");
        assert_eq!(devices[0].status, AndroidDeviceStatus::Ready);
        assert_eq!(devices[1].status, AndroidDeviceStatus::Unauthorized);
        assert_eq!(devices[2].model, "Pixel 8 Pro");
        assert_eq!(devices[3].status, AndroidDeviceStatus::Offline);
        assert!(discovery_message(&devices).contains("2 台"));
    }

    #[test]
    fn ignores_adb_server_messages_and_unknown_rows_are_kept() {
        let devices = parse_adb_devices(
            "* daemon not running; starting now at tcp:5037\nList of devices attached\nphone-1 bootloader product:test\n",
        );

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].status, AndroidDeviceStatus::Unknown);
        assert_eq!(devices[0].model, "phone-1");
    }
}
