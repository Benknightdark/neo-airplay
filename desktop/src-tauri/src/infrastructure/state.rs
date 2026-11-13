//! 應用程式執行期狀態管理、AirPlay／Android 投影引擎子程序控制與 Bonjour／ADB 偵測實作。

use crate::application::{DashboardSnapshot, ReceiverRuntime, SettingsRepository};
use crate::domain::{
    available_renderers, bonjour_instance_matches, exit_snapshot, parse_adb_devices, AndroidDevice,
    DomainError, DomainResult, EngineKind, EngineSnapshot, EngineStatus, LogLevel,
    ReceiverSettings, AIRPLAY_READY_TIMEOUT,
};
use crate::infrastructure::persistence::{AppLogger, JsonSettingsRepository};
#[cfg(target_os = "macos")]
use crate::infrastructure::process::macos_library_path;
use crate::infrastructure::process::{
    background_command, find_executable, first_non_empty_line, resolved_path, run_with_timeout,
    PROCESS_TIMEOUT,
};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

/// 管理應用程式設定、日誌、AirPlay／Android 雙投影引擎與偵測裝置清單的核心狀態物件。
pub struct AppState {
    /// 設定檔讀寫儲存庫。
    settings_repo: JsonSettingsRepository,
    /// 目前記憶體中的接收器設定快取（受互斥鎖保護）。
    settings: Mutex<ReceiverSettings>,
    /// 應用程式診斷日誌寫入器。
    logger: AppLogger,
    /// AirPlay（UxPlay）子程序生命週期控制器。
    airplay: EngineController,
    /// Android（scrcpy）子程序生命週期控制器。
    android: EngineController,
    /// 序列化 `adb devices -l` 重新整理的互斥鎖，避免並行 ADB 查詢互相干擾。
    android_refresh: Mutex<()>,
    /// 最近一次掃描取得的 Android 裝置列表。
    devices: Mutex<Vec<AndroidDevice>>,
}

impl AppState {
    /// 初始化 `AppState`，建立必要目錄並載入設定與當日日誌。
    ///
    /// # 參數
    /// - `config_dir`: 應用程式設定與 setup 目錄根路徑。
    /// - `log_dir`: 每日診斷日誌 `.jsonl` 存放目錄。
    /// - `engine_dir`: 外部投影工具（UxPlay、scrcpy、ADB、GStreamer）存放根目錄。
    /// - `gstreamer_registry_path`: macOS 下隔離使用的 GStreamer registry 快取檔案路徑。
    /// - `require_airplay_discovery`: 啟動 AirPlay 時是否必須等待 Bonjour `_airplay._tcp` 廣播就緒。
    ///
    /// # 回傳值
    /// 初始化成功回傳 `Ok(AppState)`；目錄建立或設定讀取失敗回傳錯誤字串。
    pub fn new(
        config_dir: PathBuf,
        log_dir: PathBuf,
        engine_dir: PathBuf,
        gstreamer_registry_path: PathBuf,
        require_airplay_discovery: bool,
    ) -> Result<Self, String> {
        fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
        fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        if let Some(parent) = gstreamer_registry_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let settings_repo = JsonSettingsRepository::new(config_dir.join("settings.json"));
        let initial_settings = settings_repo.load().map_err(|error| error.to_string())?;
        let logger = AppLogger::new(log_dir);
        Ok(Self {
            settings_repo,
            settings: Mutex::new(initial_settings),
            logger: logger.clone(),
            airplay: EngineController::new(
                EngineKind::AirPlay,
                engine_dir.clone(),
                gstreamer_registry_path,
                require_airplay_discovery,
                logger.clone(),
            ),
            android: EngineController::new(
                EngineKind::Android,
                engine_dir,
                PathBuf::new(),
                false,
                logger,
            ),
            android_refresh: Mutex::new(()),
            devices: Mutex::new(Vec::new()),
        })
    }

    /// 複製並回傳目前的接收器設定。
    ///
    /// # 回傳值
    /// 回傳 `Ok(ReceiverSettings)`；互斥鎖異常時回傳錯誤字串。
    pub fn settings(&self) -> Result<ReceiverSettings, String> {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .map_err(|_| "設定狀態無法讀取。".to_owned())
    }

    /// 清理並保存新設定至磁碟與記憶體快取。
    ///
    /// # 參數
    /// - `settings`: 欲套用的新接收器設定。
    ///
    /// # 回傳值
    /// 寫入成功回傳 `Ok(())`；鎖定或寫入失敗回傳錯誤字串。
    pub fn save_settings(&self, settings: ReceiverSettings) -> Result<(), String> {
        let settings = settings.sanitized();
        let mut current = self
            .settings
            .lock()
            .map_err(|_| "設定狀態無法更新。".to_owned())?;
        self.settings_repo
            .save(&settings)
            .map_err(|error| error.message)?;
        *current = settings;
        Ok(())
    }

    /// 組裝儀表板所需的設定、渲染器選項、引擎狀態、裝置與近期日誌快照。
    ///
    /// # 回傳值
    /// 成功時回傳 `DashboardSnapshot`；狀態鎖異常時回傳錯誤字串。
    pub fn dashboard(&self) -> Result<DashboardSnapshot, String> {
        let devices = self
            .devices
            .lock()
            .map_err(|_| "Android 裝置狀態無法讀取。".to_owned())?
            .clone();
        Ok(DashboardSnapshot {
            settings: self.settings()?,
            renderer_options: available_renderers()
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            airplay: self.airplay.snapshot(),
            android: self.android.snapshot(),
            devices,
            logs: self.logger.recent(),
        })
    }

    /// 讀取最新設定並啟動 AirPlay（UxPlay）投影引擎。
    ///
    /// # 回傳值
    /// 回傳啟動操作後的 `EngineSnapshot`。
    pub fn start_airplay(&self) -> Result<EngineSnapshot, String> {
        let settings = self.settings()?;
        let args = settings.airplay_arguments();
        Ok(self.airplay.start(&args, Some(&settings.receiver_name)))
    }

    /// 停止運作中的 AirPlay（UxPlay）投影引擎。
    ///
    /// # 回傳值
    /// 回傳停止操作後的 `EngineSnapshot`。
    pub fn stop_airplay(&self) -> Result<EngineSnapshot, String> {
        Ok(self.airplay.stop())
    }

    /// 執行 `adb devices -l` 重新掃描連線中的 Android 裝置並同步更新 Android 引擎可用狀態。
    ///
    /// # 回傳值
    /// 回傳最新偵測到的 `AndroidDevice` 清單（找不到 ADB 或指令失敗時會清空裝置並更新引擎錯誤狀態）。
    pub fn refresh_android_devices(&self) -> Result<Vec<AndroidDevice>, String> {
        let _refresh = self
            .android_refresh
            .lock()
            .map_err(|_| "Android 裝置更新狀態無法讀取。".to_owned())?;
        let Some(adb_path) = find_executable(&self.android.engine_dir, "adb", true) else {
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

    /// 驗證目標 Android 裝置處於就緒狀態後啟動 `scrcpy` 畫面投影。
    ///
    /// # 參數
    /// - `serial`: 已偵測清單中的目標 Android 裝置序號。
    ///
    /// # 回傳值
    /// 回傳啟動後的 `EngineSnapshot`；若序號空白或裝置未就緒則回傳錯誤字串。
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
        let args = settings.android_arguments(&device.serial);
        drop(devices);

        let snapshot = self.android.start(&args, None);
        if snapshot.status == EngineStatus::Running {
            self.logger.write(
                LogLevel::Information,
                &format!("正在投影 Android 裝置「{device_name}」。"),
            );
        }
        Ok(snapshot)
    }

    /// 停止運作中的 Android（scrcpy）投影引擎。
    ///
    /// # 回傳值
    /// 回傳停止操作後的 `EngineSnapshot`。
    pub fn stop_android_mirror(&self) -> Result<EngineSnapshot, String> {
        Ok(self.android.stop())
    }

    /// 寫入一筆資訊等級（Information）的應用程式日誌。
    ///
    /// # 參數
    /// - `message`: 紀錄文字內容。
    pub fn log_info(&self, message: &str) {
        self.logger.write(LogLevel::Information, message);
    }
}

impl ReceiverRuntime for AppState {
    /// 透過 `ReceiverRuntime` 埠取得目前設定。
    ///
    /// # 回傳值
    /// 回傳 `ReceiverSettings` 或對應 `DomainError::storage`。
    fn settings(&self) -> DomainResult<ReceiverSettings> {
        AppState::settings(self).map_err(DomainError::storage)
    }

    /// 透過 `ReceiverRuntime` 埠保存設定。
    ///
    /// # 參數
    /// - `settings`: 欲保存的設定物件。
    ///
    /// # 回傳值
    /// 成功回傳 `Ok(())`；失敗時映射為 `DomainError::storage`。
    fn save_settings(&self, settings: ReceiverSettings) -> DomainResult<()> {
        AppState::save_settings(self, settings).map_err(DomainError::storage)
    }

    /// 透過 `ReceiverRuntime` 埠查詢儀表板完整快照。
    ///
    /// # 回傳值
    /// 回傳 `DashboardSnapshot` 或 `DomainError::storage`。
    fn dashboard(&self) -> DomainResult<DashboardSnapshot> {
        AppState::dashboard(self).map_err(DomainError::storage)
    }

    /// 透過 `ReceiverRuntime` 埠啟動 AirPlay 接收器。
    ///
    /// # 回傳值
    /// 回傳 `EngineSnapshot` 或 `DomainError::external_process`。
    fn start_airplay(&self) -> DomainResult<EngineSnapshot> {
        AppState::start_airplay(self).map_err(DomainError::external_process)
    }

    /// 透過 `ReceiverRuntime` 埠停止 AirPlay 接收器。
    ///
    /// # 回傳值
    /// 回傳 `EngineSnapshot` 或 `DomainError::external_process`。
    fn stop_airplay(&self) -> DomainResult<EngineSnapshot> {
        AppState::stop_airplay(self).map_err(DomainError::external_process)
    }

    /// 透過 `ReceiverRuntime` 埠重新整理 Android 裝置清單。
    ///
    /// # 回傳值
    /// 回傳 `AndroidDevice` 向量或 `DomainError::external_process`。
    fn refresh_android_devices(&self) -> DomainResult<Vec<AndroidDevice>> {
        AppState::refresh_android_devices(self).map_err(DomainError::external_process)
    }

    /// 透過 `ReceiverRuntime` 埠啟動指定裝置的 Android 畫面投影。
    ///
    /// # 參數
    /// - `serial`: 目標裝置 ADB 序號。
    ///
    /// # 回傳值
    /// 回傳 `EngineSnapshot` 或對應驗證錯誤 `DomainError::validation`。
    fn start_android_mirror(&self, serial: &str) -> DomainResult<EngineSnapshot> {
        AppState::start_android_mirror(self, serial).map_err(DomainError::validation)
    }

    /// 透過 `ReceiverRuntime` 埠停止 Android 畫面投影。
    ///
    /// # 回傳值
    /// 回傳 `EngineSnapshot` 或 `DomainError::external_process`。
    fn stop_android_mirror(&self) -> DomainResult<EngineSnapshot> {
        AppState::stop_android_mirror(self).map_err(DomainError::external_process)
    }

    /// 透過 `ReceiverRuntime` 埠寫入資訊日誌。
    ///
    /// # 參數
    /// - `message`: 紀錄文字內容。
    fn log_info(&self, message: &str) {
        AppState::log_info(self, message);
    }
}

/// 單一投影引擎受互斥鎖保護的內部程序與狀態資料。
pub(crate) struct EngineInner {
    /// 正在執行中的主要投影子程序（`uxplay` 或 `scrcpy`）。
    pub(crate) child: Option<Child>,
    /// macOS AirPlay 啟動期間額外執行的 Bonjour `dns-sd` 探索子程序。
    pub(crate) discovery_child: Option<Child>,
    /// 接收 Bonjour 廣播就緒通知的非同步通道端點。
    pub(crate) discovery_events: Option<Receiver<DiscoveryEvent>>,
    /// 引擎本次啟動時間點，用於計算等待廣播耗時與逾時判斷。
    pub(crate) started_at: Option<Instant>,
    /// 引擎最新對外回報快照。
    pub(crate) snapshot: EngineSnapshot,
}

/// Bonjour mDNS 監控執行緒回報給引擎控制器的事件類型。
pub(crate) enum DiscoveryEvent {
    /// 已偵測到符合接收器名稱的 `_airplay._tcp` 廣播服務。
    Ready,
}

/// 封裝單一投影引擎（AirPlay 或 Android）子程序啟動、狀態輪詢、輸出擷取與安全終止邏輯。
pub(crate) struct EngineController {
    /// 此控制器負責的引擎種類。
    kind: EngineKind,
    /// 受控管投影工具根目錄。
    engine_dir: PathBuf,
    /// macOS 專用 GStreamer registry 快取檔案路徑。
    #[cfg(target_os = "macos")]
    gstreamer_registry_path: PathBuf,
    /// 是否要求 AirPlay 在通過 Bonjour 偵測後才轉為 `Running` 狀態。
    require_airplay_discovery: bool,
    /// 受互斥鎖保護的程序句柄與快照狀態。
    pub(crate) inner: Mutex<EngineInner>,
    /// 用於記錄子程序標準輸出與異常事件的日誌寫入器。
    logger: AppLogger,
}

impl EngineController {
    /// 建立新的 `EngineController` 並將初始狀態設為未啟動。
    ///
    /// # 參數
    /// - `kind`: 引擎種類（AirPlay 或 Android）。
    /// - `engine_dir`: 外部工具根目錄。
    /// - `gstreamer_registry_path`: macOS GStreamer 快取路徑（非 macOS 平台會忽略）。
    /// - `require_airplay_discovery`: 是否啟用 Bonjour mDNS 就緒驗證。
    /// - `logger`: 應用程式診斷日誌寫入器。
    ///
    /// # 回傳值
    /// 回傳處於 `Stopped` 初始狀態的 `EngineController`。
    pub(crate) fn new(
        kind: EngineKind,
        engine_dir: PathBuf,
        gstreamer_registry_path: PathBuf,
        require_airplay_discovery: bool,
        logger: AppLogger,
    ) -> Self {
        #[cfg(not(target_os = "macos"))]
        drop(gstreamer_registry_path);

        Self {
            kind,
            engine_dir,
            #[cfg(target_os = "macos")]
            gstreamer_registry_path,
            require_airplay_discovery,
            inner: Mutex::new(EngineInner {
                child: None,
                discovery_child: None,
                discovery_events: None,
                started_at: None,
                snapshot: EngineSnapshot::stopped(kind.not_started_message()),
            }),
            logger,
        }
    }

    /// 啟動引擎子程序；若既有子程序仍在執行則直接回傳現行快照。
    ///
    /// # 參數
    /// - `args`: 傳遞給執行檔的命令列參數陣列。
    /// - `receiver_name`: 僅 AirPlay 使用的廣播名稱，作為 Bonjour 監控比對依據。
    ///
    /// # 回傳值
    /// 回傳啟動後或異常發生時的 `EngineSnapshot`。
    pub(crate) fn start(&self, args: &[String], receiver_name: Option<&str>) -> EngineSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return EngineSnapshot::stopped("接收器狀態無法更新。");
        };
        if let Some(child) = inner.child.as_mut() {
            match child.try_wait() {
                Ok(None) => return inner.snapshot.clone(),
                Ok(Some(status)) => {
                    inner.child = None;
                    stop_discovery_monitor(&mut inner);
                    inner.started_at = None;
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
                    inner.snapshot = EngineSnapshot::faulted(
                        message.clone(),
                        inner.child.as_ref().map(Child::id),
                    );
                    self.logger.write(LogLevel::Error, &message);
                    return inner.snapshot.clone();
                }
            }
        }

        let Some(executable) = self.executable() else {
            let message = self.kind.missing_dependency_message().to_owned();
            inner.snapshot = EngineSnapshot {
                status: EngineStatus::DependencyMissing,
                message: message.clone(),
                process_id: None,
            };
            self.logger.write(LogLevel::Error, &message);
            return inner.snapshot.clone();
        };

        let mut command = background_command(&executable);
        command
            .args(args)
            .env("PATH", resolved_path(&self.engine_dir))
            .current_dir(executable.parent().unwrap_or(&self.engine_dir))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let plugin_directory = self.engine_dir.join("lib/gstreamer-1.0");
        command
            .env("GST_PLUGIN_PATH_1_0", &plugin_directory)
            .env("GST_PLUGIN_SYSTEM_PATH_1_0", &plugin_directory);
        if matches!(self.kind, EngineKind::AirPlay) {
            #[cfg(target_os = "macos")]
            command
                .env("GST_REGISTRY", &self.gstreamer_registry_path)
                .env("GST_REGISTRY_1_0", &self.gstreamer_registry_path);
        }
        #[cfg(target_os = "macos")]
        command.env("DYLD_LIBRARY_PATH", macos_library_path(&self.engine_dir));
        #[cfg(target_os = "windows")]
        command.env(
            "GST_PLUGIN_SCANNER_1_0",
            self.engine_dir.join("bin/gst-plugin-scanner.exe"),
        );
        #[cfg(not(target_os = "windows"))]
        command.env(
            "GST_PLUGIN_SCANNER_1_0",
            self.engine_dir.join("bin/gst-plugin-scanner"),
        );
        match command.spawn() {
            Ok(mut child) => {
                let process_id = child.id();
                self.capture_output(&mut child);
                let name = self.kind.display_name();
                let started_at = Instant::now();
                inner.child = Some(child);
                inner.started_at = Some(started_at);
                if matches!(self.kind, EngineKind::AirPlay) && self.require_airplay_discovery {
                    let expected_name = receiver_name.unwrap_or("Neo AirPlay");
                    match spawn_discovery_monitor(expected_name, self.logger.clone()) {
                        Ok((discovery_child, events)) => {
                            inner.discovery_child = Some(discovery_child);
                            inner.discovery_events = Some(events);
                            let message = format!(
                                "UxPlay 已啟動，正在等待 Bonjour 廣播（最多 {} 秒）。",
                                AIRPLAY_READY_TIMEOUT.as_secs()
                            );
                            inner.snapshot = EngineSnapshot {
                                status: EngineStatus::Starting,
                                message: message.clone(),
                                process_id: Some(process_id),
                            };
                            self.logger.write(LogLevel::Information, &message);
                        }
                        Err(error) => {
                            terminate_child(&mut inner.child);
                            let message = format!("無法監控 Bonjour 服務：{error}");
                            inner.snapshot = EngineSnapshot::faulted(message.clone(), None);
                            inner.started_at = None;
                            self.logger.write(LogLevel::Error, &message);
                        }
                    }
                } else {
                    let message = format!("{name} 已啟動。");
                    inner.snapshot = EngineSnapshot {
                        status: EngineStatus::Running,
                        message: message.clone(),
                        process_id: Some(process_id),
                    };
                    self.logger.write(LogLevel::Information, &message);
                }
            }
            Err(error) => {
                let message = format!("無法啟動引擎：{error}");
                inner.snapshot = EngineSnapshot::faulted(message.clone(), None);
                self.logger.write(LogLevel::Error, &message);
            }
        }
        inner.snapshot.clone()
    }

    /// 輪詢子程序存活與 Bonjour 就緒狀態，更新並回傳最新 `EngineSnapshot`。
    ///
    /// # 回傳值
    /// 回傳即時同步後的引擎狀態快照。
    pub(crate) fn snapshot(&self) -> EngineSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return EngineSnapshot::stopped("接收器狀態無法讀取。");
        };
        let observation = inner.child.as_mut().map(Child::try_wait);
        match observation {
            Some(Ok(Some(exit_status))) => {
                let was_starting = inner.snapshot.is_starting();
                inner.child = None;
                stop_discovery_monitor(&mut inner);
                inner.started_at = None;
                let (status, message, level) = if was_starting {
                    (
                        EngineStatus::Faulted,
                        format!(
                            "AirPlay 啟動失敗：UxPlay 在 Bonjour 就緒前結束（{exit_status}）。"
                        ),
                        LogLevel::Error,
                    )
                } else {
                    exit_snapshot(self.kind, exit_status)
                };
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
                inner.snapshot = EngineSnapshot::faulted(message.clone(), process_id);
                self.logger.write(LogLevel::Error, &message);
            }
            _ => {}
        }
        if inner.snapshot.is_starting() {
            self.update_airplay_readiness(&mut inner);
        }
        inner.snapshot.clone()
    }

    /// 檢查處於 `Starting` 狀態的 AirPlay 是否收到 Bonjour 就緒通知、失敗或逾時。
    ///
    /// # 參數
    /// - `inner`: 已取得互斥鎖的引擎內部可變狀態參考。
    pub(crate) fn update_airplay_readiness(&self, inner: &mut EngineInner) {
        let discovery_result = match inner.discovery_events.as_ref() {
            Some(events) => match events.try_recv() {
                Ok(DiscoveryEvent::Ready) => Some(Ok(())),
                Err(TryRecvError::Disconnected) => Some(Err("Bonjour 監控程序已中斷。".to_owned())),
                Err(TryRecvError::Empty) => None,
            },
            None => Some(Err("Bonjour 監控程序未啟動。".to_owned())),
        };

        if let Some(result) = discovery_result {
            match result {
                Ok(()) => {
                    stop_discovery_monitor(inner);
                    let elapsed = inner
                        .started_at
                        .take()
                        .map(|started_at| started_at.elapsed().as_millis());
                    let message = match elapsed {
                        Some(milliseconds) => {
                            format!("AirPlay 已在 Bonjour 上就緒（{} 毫秒）。", milliseconds)
                        }
                        None => "AirPlay 已在 Bonjour 上就緒。".to_owned(),
                    };
                    let process_id = inner.child.as_ref().map(Child::id);
                    inner.snapshot = EngineSnapshot {
                        status: EngineStatus::Running,
                        message: message.clone(),
                        process_id,
                    };
                    self.logger.write(LogLevel::Information, &message);
                    return;
                }
                Err(reason) => {
                    self.fail_airplay_start(inner, reason);
                    return;
                }
            }
        }

        if let Some(discovery_child) = inner.discovery_child.as_mut() {
            match discovery_child.try_wait() {
                Ok(Some(status)) => {
                    self.fail_airplay_start(
                        inner,
                        format!("Bonjour 瀏覽程序提早結束（{status}）。"),
                    );
                    return;
                }
                Err(error) => {
                    self.fail_airplay_start(inner, format!("無法檢查 Bonjour 瀏覽程序：{error}"));
                    return;
                }
                Ok(None) => {}
            }
        }

        if inner
            .started_at
            .is_some_and(|started_at| started_at.elapsed() >= AIRPLAY_READY_TIMEOUT)
        {
            self.fail_airplay_start(
                inner,
                format!(
                    "{} 秒內沒有在 Bonjour 找到 AirPlay 接收器。請確認區域網路與防火牆設定後重試。",
                    AIRPLAY_READY_TIMEOUT.as_secs()
                ),
            );
        }
    }

    /// 中止失敗的 AirPlay 啟動流程、清理探索與引擎子程序並記錄錯誤狀態。
    ///
    /// # 參數
    /// - `inner`: 已取得互斥鎖的引擎內部可變狀態參考。
    /// - `reason`: 導致啟動失敗的詳細原因說明。
    fn fail_airplay_start(&self, inner: &mut EngineInner, reason: String) {
        stop_discovery_monitor(inner);
        terminate_child(&mut inner.child);
        inner.started_at = None;
        let message = format!("AirPlay 啟動失敗：{reason}");
        inner.snapshot = EngineSnapshot::faulted(message.clone(), None);
        self.logger.write(LogLevel::Error, &message);
    }

    /// 停止投影引擎子程序與連帶的探索監控程序並等待資源回收。
    ///
    /// # 回傳值
    /// 回傳停止完成或失敗時的 `EngineSnapshot`。
    pub(crate) fn stop(&self) -> EngineSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return EngineSnapshot::stopped("接收器狀態無法更新。");
        };
        stop_discovery_monitor(&mut inner);
        let Some(mut child) = inner.child.take() else {
            inner.started_at = None;
            if inner.snapshot.status != EngineStatus::DependencyMissing {
                inner.snapshot = EngineSnapshot::stopped(self.kind.stopped_message());
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
        let name = self.kind.display_name();
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
        inner.started_at = None;
        self.logger.write(level, &message);
        inner.snapshot.clone()
    }

    /// 由外部檢查流程（例如 ADB 掃描失敗）設定引擎快照狀態，若仍有運作中子程序則忽略。
    ///
    /// # 參數
    /// - `status`: 要套用的外部狀態（如 `DependencyMissing` 或 `Faulted`）。
    /// - `message`: 顯示於介面的說明訊息。
    pub(crate) fn set_external_status(&self, status: EngineStatus, message: String) {
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

    /// 當前置工具檢測恢復正常時，清除先前的 `DependencyMissing` 或 `Faulted` 狀態為待機。
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
                inner.snapshot = EngineSnapshot::stopped(self.kind.not_started_message());
            }
        }
    }

    /// 依引擎種類在受控管 `engine_dir` 內尋找對應可執行檔路徑。
    ///
    /// # 回傳值
    /// 找到可執行檔時回傳 `Some(PathBuf)`，否則回傳 `None`。
    fn executable(&self) -> Option<PathBuf> {
        find_executable(
            &self.engine_dir,
            self.kind.executable_name(),
            self.kind.is_android(),
        )
    }

    /// 啟動背景執行緒持續讀取子程序的 `stdout` 與 `stderr` 並寫入應用程式日誌。
    ///
    /// # 參數
    /// - `child`: 剛啟動的子程序可變參考，將接管其 `stdout` 與 `stderr` 控制權。
    fn capture_output(&self, child: &mut Child) {
        let name = self.kind.display_name();
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
    /// 控制器釋放時確保終止並回收所有尚未結束的引擎與 Bonjour 監控子程序，避免孤兒行程殘留。
    fn drop(&mut self) {
        if let Ok(inner) = self.inner.get_mut() {
            stop_discovery_monitor(inner);
            if let Some(child) = inner.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// 啟動 macOS `/usr/bin/dns-sd -B _airplay._tcp local.` 瀏覽並等待目標名稱廣播出現。
///
/// # 參數
/// - `receiver_name`: 預期出現的 AirPlay 接收器名稱。
/// - `logger`: 用於記錄 `dns-sd` 錯誤輸出的應用程式日誌器。
///
/// # 回傳值
/// 成功時回傳 `(Child, Receiver<DiscoveryEvent>)`；啟動失敗則回傳錯誤字串。
fn spawn_discovery_monitor(
    receiver_name: &str,
    logger: AppLogger,
) -> Result<(Child, Receiver<DiscoveryEvent>), String> {
    let mut child = Command::new("/usr/bin/dns-sd")
        .args(["-B", "_airplay._tcp", "local."])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let (sender, events) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        let expected_name = receiver_name.to_owned();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if bonjour_instance_matches(&line, &expected_name) {
                    let _ = sender.send(DiscoveryEvent::Ready);
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                logger.write(LogLevel::Warning, &format!("Bonjour: {line}"));
            }
        });
    }
    Ok((child, events))
}

/// 強制終止並回收 Bonjour 監控子程序並清除事件接收通道。
///
/// # 參數
/// - `inner`: 引擎內部狀態可變參考。
fn stop_discovery_monitor(inner: &mut EngineInner) {
    if let Some(mut child) = inner.discovery_child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    inner.discovery_events = None;
}

/// 若可選子程序仍在執行則送出強制終止訊號並等待結束。
///
/// # 參數
/// - `child`: 欲清理的可選子程序控制權欄位。
fn terminate_child(child: &mut Option<Child>) {
    if let Some(mut child) = child.take() {
        if matches!(child.try_wait(), Ok(None) | Err(_)) {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

#[cfg(test)]
#[path = "../../../../desktop-test/backend/unit-tests/state.rs"]
mod tests;
