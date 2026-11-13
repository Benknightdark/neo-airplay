use crate::application::{DependencyRuntime, EngineAccessGuard};
use crate::domain::{
    first_useful_line, interrupted_install_detail, parse_missing_dependencies, DependencyAction,
    DependencyOperation, DependencySnapshot, DomainError, DomainResult, InstallProgress,
    InstallResult, LaunchRecord, RuntimeCheck, SetupOutcome,
};
#[cfg(target_os = "windows")]
use crate::infrastructure::process::background_command;
use crate::infrastructure::process::{installer_path, run_with_timeout_in_path};
use serde::Serialize;
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

pub struct DependencyManager {
    snapshot: Arc<Mutex<DependencySnapshot>>,
    pub(crate) worker_active: Arc<AtomicBool>,
    operation_gate: Arc<Mutex<()>>,
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
                operation: None,
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
            operation_gate: Arc::new(Mutex::new(())),
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

    pub fn read_install_log(&self) -> Result<String, String> {
        fs::read_to_string(self.setup_dir.join(INSTALL_LOG)).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "尚無安裝記錄。".to_owned()
            } else {
                format!("無法讀取安裝記錄：{error}")
            }
        })
    }

    pub fn start(&self) -> Result<DependencySnapshot, String> {
        self.begin_operation(DependencyOperation::Install, true, &mut || Ok(()))
    }

    pub fn manage(
        &self,
        action: DependencyAction,
        prepare: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<DependencySnapshot, String> {
        self.begin_operation(action.into(), false, prepare)
    }

    pub fn with_engine_access<T>(
        &self,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _operation = self
            .operation_gate
            .lock()
            .map_err(|_| "工具維護狀態無法讀取。".to_owned())?;
        if self.worker_active.load(Ordering::Acquire) {
            return Err("工具正在檢查或維護，請稍候再試。".into());
        }
        action()
    }

    fn begin_operation(
        &self,
        operation: DependencyOperation,
        return_existing: bool,
        prepare: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<DependencySnapshot, String> {
        if !self.enabled {
            if return_existing {
                return self.snapshot();
            }
            return Err("此工作階段已停用工具管理。".into());
        }

        {
            let _operation = self
                .operation_gate
                .lock()
                .map_err(|_| "工具維護狀態無法更新。".to_owned())?;
            if self.worker_active.swap(true, Ordering::AcqRel) {
                if return_existing {
                    return self.snapshot();
                }
                return Err("另一項工具檢查或維護作業仍在進行。".into());
            }
            let mut snapshot = match self.snapshot.lock() {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    self.worker_active.store(false, Ordering::Release);
                    return Err("無法更新工具安裝狀態。".into());
                }
            };
            snapshot.status = if operation == DependencyOperation::Install {
                "checking".into()
            } else {
                "installing".into()
            };
            snapshot.operation = Some(operation);
            snapshot.message = match operation {
                DependencyOperation::Install => "正在檢查 UxPlay、GStreamer、ADB 與 scrcpy…".into(),
                DependencyOperation::Upgrade => "正在準備升級投影工具…".into(),
                DependencyOperation::Reinstall => "正在準備清除並重新安裝…".into(),
            };
        }

        if let Err(error) = prepare() {
            self.fail_operation(&error);
            return Err(error);
        }

        let shared = Arc::clone(&self.snapshot);
        let worker_active = Arc::clone(&self.worker_active);
        let operation_gate = Arc::clone(&self.operation_gate);
        let engine_dir = self.engine_dir.clone();
        let gstreamer_registry_path = self.gstreamer_registry_path.clone();
        let setup_dir = self.setup_dir.clone();
        let spawned = thread::Builder::new()
            .name("neo-airplay-dependency-setup".into())
            .spawn(move || {
                let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    run_setup(
                        &engine_dir,
                        &gstreamer_registry_path,
                        &setup_dir,
                        &shared,
                        operation,
                    )
                }));
                let outcome = match run {
                    Ok(Ok(outcome)) => outcome,
                    Ok(Err(error)) => {
                        append_log(&setup_dir.join(INSTALL_LOG), &error);
                        SetupOutcome::failed(error, Vec::new())
                    }
                    Err(_) => {
                        let message = "工具維護背景程序意外結束。".to_owned();
                        append_log(&setup_dir.join(INSTALL_LOG), &message);
                        SetupOutcome::failed(message, Vec::new())
                    }
                };
                if let Ok(_operation) = operation_gate.lock() {
                    if let Ok(mut snapshot) = shared.lock() {
                        snapshot.status = outcome.status;
                        snapshot.message = outcome.message;
                        snapshot.missing = outcome.missing;
                    }
                    worker_active.store(false, Ordering::Release);
                } else {
                    worker_active.store(false, Ordering::Release);
                }
            });

        if let Err(error) = spawned {
            self.fail_operation(&format!("無法啟動背景工具維護：{error}"));
            return Err(error.to_string());
        }

        self.snapshot()
    }

    /// 當前置步驟或執行緒生成失敗時，寫入失敗日誌、將快照標記為 `failed` 並釋放 `worker_active`。
    ///
    /// # 參數
    /// - `message`: 失敗原因訊息。
    fn fail_operation(&self, message: &str) {
        append_log(&self.setup_dir.join(INSTALL_LOG), message);
        if let Ok(_operation) = self.operation_gate.lock() {
            if let Ok(mut snapshot) = self.snapshot.lock() {
                snapshot.status = "failed".into();
                snapshot.message = message.to_owned();
                snapshot.missing.clear();
            }
            self.worker_active.store(false, Ordering::Release);
        } else {
            self.worker_active.store(false, Ordering::Release);
        }
    }
}

impl EngineAccessGuard for DependencyManager {
    /// 實作應用層 `EngineAccessGuard` 埠，阻擋維護期間同時操作投影引擎。
    ///
    /// # 參數
    /// - `action`: 無衝突時執行的可變閉包參考。
    ///
    /// # 回傳值
    /// 執行成功回傳 `Ok(())`；工具忙碌中時回傳 `DomainError::dependency_busy`。
    fn execute_if_available(&self, action: &mut dyn FnMut()) -> DomainResult<()> {
        let _operation = self
            .operation_gate
            .lock()
            .map_err(|_| DomainError::conflict("工具維護狀態無法讀取。"))?;
        if self.worker_active.load(Ordering::Acquire) {
            return Err(DomainError::dependency_busy(
                "工具正在檢查或維護，請稍候再試。",
            ));
        }
        action();
        Ok(())
    }
}

impl DependencyRuntime for DependencyManager {
    /// 透過 `DependencyRuntime` 埠查詢目前快照。
    ///
    /// # 回傳值
    /// 回傳 `DependencySnapshot` 或映射後的 `DomainError::storage`。
    fn snapshot(&self) -> DomainResult<DependencySnapshot> {
        DependencyManager::snapshot(self).map_err(DomainError::storage)
    }

    /// 透過 `DependencyRuntime` 埠讀取安裝日誌。
    ///
    /// # 回傳值
    /// 回傳安裝日誌內容或 `DomainError::not_found`。
    fn read_install_log(&self) -> DomainResult<String> {
        DependencyManager::read_install_log(self).map_err(DomainError::not_found)
    }

    /// 透過 `DependencyRuntime` 埠啟動自動檢查與安裝。
    ///
    /// # 回傳值
    /// 回傳更新後的 `DependencySnapshot` 或 `DomainError::dependency_busy`。
    fn start_install(&self) -> DomainResult<DependencySnapshot> {
        DependencyManager::start(self).map_err(DomainError::dependency_busy)
    }

    /// 透過 `DependencyRuntime` 埠啟動手動維護作業。
    ///
    /// # 參數
    /// - `action`: 維護動作類型。
    /// - `prepare`: 前置停止與檢查閉包。
    ///
    /// # 回傳值
    /// 回傳更新後的 `DependencySnapshot` 或 `DomainError::dependency_busy`。
    fn manage(
        &self,
        action: DependencyAction,
        prepare: &mut dyn FnMut() -> DomainResult<()>,
    ) -> DomainResult<DependencySnapshot> {
        DependencyManager::manage(self, action, &mut || {
            prepare().map_err(|error| error.message)
        })
        .map_err(DomainError::dependency_busy)
    }
}

/// 在背景執行緒執行的完整檢查、清理、安裝與結果驗證工作流程。
///
/// # 參數
/// - `engine_dir`: 受控管投影工具目錄。
/// - `gstreamer_registry_path`: macOS GStreamer 快取路徑。
/// - `setup_dir`: 暫存腳本與安裝追蹤檔案目錄。
/// - `snapshot`: 用於即時發布狀態進度的共用快照鎖。
/// - `operation`: 本次執行的依賴作業類型。
///
/// # 回傳值
/// 回傳最終 `SetupOutcome` 或不可恢復的錯誤訊息字串。
fn run_setup(
    engine_dir: &Path,
    gstreamer_registry_path: &Path,
    setup_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
    operation: DependencyOperation,
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

    if operation == DependencyOperation::Reinstall {
        publish_installing(snapshot, "正在清除 Neo AirPlay 專用工具與快取…");
        clear_app_tools(engine_dir, gstreamer_registry_path)?;
    } else if operation == DependencyOperation::Upgrade {
        publish_installing(snapshot, "正在升級投影工具並更新 GStreamer 快取…");
        clear_gstreamer_registry(gstreamer_registry_path)?;
    }

    stage_installer(setup_dir)?;
    let runtime = check_runtime(engine_dir, setup_dir)?;
    if runtime.is_ready() && operation == DependencyOperation::Install {
        warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
        remove_if_exists(&launch_path);
        return Ok(SetupOutcome::ready(operation));
    }

    remove_if_exists(&marker_path);
    remove_if_exists(&result_path);
    remove_if_exists(&launch_path);

    let message = match operation {
        DependencyOperation::Install => {
            let missing_message = format!("缺少或無法載入：{}", runtime.missing.join("、"));
            append_log(&setup_dir.join(INSTALL_LOG), &missing_message);
            format!("{missing_message} 正在背景安裝，請完成系統授權提示。")
        }
        DependencyOperation::Upgrade => "正在更新平台相依套件與 Neo AirPlay 工具…".into(),
        DependencyOperation::Reinstall => "正在更新平台相依套件並重新安裝工具…".into(),
    };
    publish_installing(snapshot, &message);

    let result = install_dependencies(
        setup_dir,
        engine_dir,
        snapshot,
        operation != DependencyOperation::Install,
    );
    let final_check = check_runtime(engine_dir, setup_dir)?;
    let installer_succeeded = result.as_ref().is_ok_and(|result| result.exit_code == 0);
    if final_check.is_ready() && (operation == DependencyOperation::Install || installer_succeeded)
    {
        warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
        remove_if_exists(&launch_path);
        return Ok(SetupOutcome::ready(operation));
    }

    let install_detail = match result {
        Ok(result) => result.detail(),
        Err(error) => format!("安裝程序未完成：{error}"),
    };
    let missing_detail = if final_check.missing.is_empty() {
        "工具檢查仍通過，但維護作業未成功完成。".to_owned()
    } else {
        format!("仍缺少 {}。", final_check.missing.join("、"))
    };
    let message = format!("{install_detail}；{missing_detail}請查看安裝記錄後重試。");
    append_log(&setup_dir.join(INSTALL_LOG), &message);
    Ok(SetupOutcome::failed(message, final_check.missing))
}

/// 在 macOS 上於首次就緒或快取失效時預熱 GStreamer registry 快取，加速後續 UxPlay 啟動。
///
/// # 參數
/// - `engine_dir`: 受控管工具目錄。
/// - `registry_path`: 目標 GStreamer registry 二進位檔案路徑。
/// - `setup_dir`: 放置 `install.sh` 的 setup 目錄。
/// - `snapshot`: 用於更新目前介面安裝提示文字的快照鎖。
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

/// 將編譯時嵌入二進位檔的平台安裝腳本釋放至 `setup_dir`。
///
/// # 參數
/// - `setup_dir`: 目標腳本存放目錄。
///
/// # 回傳值
/// 成功釋放回傳腳本檔案路徑 `Ok(PathBuf)`；寫入失敗回傳錯誤字串。
fn stage_installer(setup_dir: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let (path, contents) = (
        setup_dir.join("install.ps1"),
        include_str!("../../../scripts/install.ps1"),
    );
    #[cfg(not(target_os = "windows"))]
    let (path, contents) = (
        setup_dir.join("install.sh"),
        include_str!("../../../scripts/install.sh"),
    );

    fs::write(&path, contents).map_err(|error| format!("無法準備工具安裝程序：{error}"))?;
    Ok(path)
}

/// 呼叫平台安裝腳本的檢查模式驗證目前工具與執行期環境是否齊備。
///
/// # 參數
/// - `engine_dir`: 待檢查的工具根目錄。
/// - `setup_dir`: 包含已釋放安裝腳本與 `install.log` 的目錄。
///
/// # 回傳值
/// 回傳包含缺少項目清單的 `RuntimeCheck`；檢查指令無法啟動或逾時則回傳錯誤字串。
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

/// 依作業系統啟動安裝腳本（macOS 透過 AppleScript 開啟 Terminal，其餘平台直接背景執行）並等待其完成。
///
/// # 參數
/// - `setup_dir`: 安裝腳本、日誌、PID 標記與結果檔目錄。
/// - `engine_dir`: 目標投影工具安裝目錄。
/// - `snapshot`: 用於即時推送進度提示的快照鎖。
/// - `force_upgrade`: 是否傳入升級旗標以強制更新現有套件。
///
/// # 回傳值
/// 回傳由腳本寫入的 `InstallResult`；啟動或監控過程失敗則回傳錯誤字串。
fn install_dependencies(
    setup_dir: &Path,
    engine_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
    force_upgrade: bool,
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
        let upgrade_argument = if force_upgrade { " --upgrade" } else { "" };
        fs::write(
            &launch_script,
            format!("#!/bin/bash\nset -u\nexec > >(tee -a \"$2\") 2>&1\nexec /bin/bash \"$1\" --gui --engine-dir \"$3\" --marker-file \"$4\" --result-file \"$5\"{upgrade_argument}\n"),
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
        let mut command = installer_command(
            &script,
            engine_dir,
            &marker_path,
            &result_path,
            force_upgrade,
        );
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

/// 建構 Windows PowerShell 非互動式安裝命令。
///
/// # 參數
/// - `script`: `install.ps1` 腳本檔案路徑。
/// - `engine_dir`: 目標工具安裝目錄。
/// - `marker`: PID 標記檔案路徑。
/// - `result`: 結果輸出 JSON 檔案路徑。
/// - `force_upgrade`: 是否附帶 `-Upgrade` 旗標。
///
/// # 回傳值
/// 回傳設定完成參數的 Windows `Command`。
#[cfg(target_os = "windows")]
pub(crate) fn installer_command(
    script: &Path,
    engine_dir: &Path,
    marker: &Path,
    result: &Path,
    force_upgrade: bool,
) -> Command {
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
    if force_upgrade {
        command.arg("-Upgrade");
    }
    command
}

/// 建構 Linux Bash 安裝命令。
///
/// # 參數
/// - `script`: `install.sh` 腳本檔案路徑。
/// - `engine_dir`: 目標工具安裝目錄。
/// - `marker`: PID 標記檔案路徑。
/// - `result`: 結果輸出 JSON 檔案路徑。
/// - `force_upgrade`: 是否附帶 `--upgrade` 旗標。
///
/// # 回傳值
/// 回傳設定完成參數的 Linux `Command`。
#[cfg(target_os = "linux")]
pub(crate) fn installer_command(
    script: &Path,
    engine_dir: &Path,
    marker: &Path,
    result: &Path,
    force_upgrade: bool,
) -> Command {
    let mut command = Command::new("/bin/bash");
    command
        .arg(script)
        .args(["--gui", "--engine-dir"])
        .arg(engine_dir)
        .arg("--marker-file")
        .arg(marker)
        .arg("--result-file")
        .arg(result);
    if force_upgrade {
        command.arg("--upgrade");
    }
    command
}

/// 持續輪詢安裝進度檔、子程序狀態、PID 標記與結果檔，直到安裝產出結果或確認中斷。
///
/// # 參數
/// - `engine_dir`: 投影工具目錄。
/// - `setup_dir`: 追蹤檔案（`install.pid`、`install-result.json`、`install-progress.json`）所在目錄。
/// - `expected_pid`: 已知的既有安裝程序 PID（若有）。
/// - `child`: 直接產生的背景安裝子程序句柄（macOS 透過 Terminal 啟動時為 `None`）。
/// - `launch`: 本次或上一次啟動紀錄。
/// - `snapshot`: 用於即時更新 UI 進度提示的快照鎖。
///
/// # 回傳值
/// 成功讀到結果檔回傳 `Ok(Some(InstallResult))`；程序意外結束或無法確認狀態時回傳錯誤字串。
pub(crate) fn wait_for_install(
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

/// 讀取並驗證安裝進度 JSON 檔案。
///
/// # 參數
/// - `path`: `install-progress.json` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；有效進度回傳 `Ok(Some(InstallProgress))`；格式或讀取異常回傳錯誤字串。
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

/// 讀取安裝程序 PID 標記檔案。
///
/// # 參數
/// - `path`: `install.pid` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；有效非零 PID 回傳 `Ok(Some(u32))`；內容無效或讀取失敗回傳錯誤字串。
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

/// 讀取安裝結果 JSON 檔案。
///
/// # 參數
/// - `path`: `install-result.json` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；有效結果回傳 `Ok(Some(InstallResult))`；格式或 PID 無效回傳錯誤字串。
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

/// 讀取上一次或當前啟動的安裝紀錄檔案。
///
/// # 參數
/// - `path`: `install-launch.json` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；讀取或反序列化成功回傳 `Ok(Some(LaunchRecord))`；失敗回傳錯誤字串。
fn read_launch_record(path: &Path) -> Result<Option<LaunchRecord>, String> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map(Some)
            .map_err(|error| format!("安裝程序啟動記錄格式無效：{error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序啟動記錄：{error}")),
    }
}

/// 外部行程存活探測結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessPresence {
    /// 確認行程仍在作業系統中執行。
    Alive,
    /// 確認行程已不存在。
    Dead,
    /// 因權限或探測逾時無法確定行程狀態。
    Unknown,
}

/// 依作業系統檢查指定 PID 的安裝程序是否仍在執行。
///
/// # 參數
/// - `pid`: 欲探測的作業系統行程識別碼。
/// - `_engine_dir`: 投影工具目錄（Windows 上用於組裝 `tasklist.exe` 查詢環境 PATH）。
///
/// # 回傳值
/// 回傳 `ProcessPresence::Alive`、`Dead` 或 `Unknown`。
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

/// 解析 Windows `tasklist.exe /FO CSV /NH` 輸出，判斷目標 PID 是否仍在執行清單中。
///
/// # 參數
/// - `output`: `tasklist.exe` 指令輸出。
/// - `pid`: 目標行程識別碼。
///
/// # 回傳值
/// 若 CSV 第二欄精確吻合 `pid` 則回傳 `true`。
#[cfg(any(target_os = "windows", test))]
pub(crate) fn tasklist_contains_pid(output: &Output, pid: u32) -> bool {
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.split(',')
            .nth(1)
            .map(|field| field.trim().trim_matches('"') == pid.to_string())
            .unwrap_or(false)
    })
}

/// 判斷啟動紀錄是否仍落在 `LAUNCH_GRACE` 寬限視窗內，用於等待尚未寫入 PID 標記的提權程序。
///
/// # 參數
/// - `record`: 上一次寫入的 `LaunchRecord`。
///
/// # 回傳值
/// 若距今小於 `LAUNCH_GRACE` 秒則回傳 `true`。
fn launch_is_recent(record: &LaunchRecord) -> bool {
    unix_time_seconds().saturating_sub(record.started_at) < LAUNCH_GRACE.as_secs()
}

/// 以暫存檔寫入並更名的方式原子性將資料序列化為 JSON 檔案。
///
/// # 參數
/// - `path`: 目標 JSON 檔案路徑。
/// - `value`: 實作 `Serialize` 的資料結構參考。
///
/// # 回傳值
/// 成功回傳 `Ok(())`；序列化、寫入或更名失敗時回傳錯誤字串。
pub(crate) fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let temporary_path = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary_path, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary_path, path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        error.to_string()
    })
}

/// 若目標檔案存在則盡力移除，忽略不存在或其他刪除錯誤（用於清理非關鍵標記檔）。
///
/// # 參數
/// - `path`: 欲清理的檔案路徑。
fn remove_if_exists(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

/// 重新安裝前清除應用程式專用工具目錄內容與 GStreamer 快取檔案。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的投影工具目錄。
/// - `gstreamer_registry_path`: GStreamer registry 快取檔案路徑。
///
/// # 回傳值
/// 成功清除回傳 `Ok(())`；遇到符號連結目錄或刪除失敗則回傳錯誤字串。
pub(crate) fn clear_app_tools(
    engine_dir: &Path,
    gstreamer_registry_path: &Path,
) -> Result<(), String> {
    clear_engine_directory(engine_dir)?;
    clear_gstreamer_registry(gstreamer_registry_path)
}

/// 移除指定的 GStreamer registry 快取檔案（若路徑非空）。
///
/// # 參數
/// - `path`: GStreamer registry 快取檔案路徑。
///
/// # 回傳值
/// 成功移除或檔案本不存在回傳 `Ok(())`；刪除失敗回傳錯誤字串。
pub(crate) fn clear_gstreamer_registry(path: &Path) -> Result<(), String> {
    if !path.as_os_str().is_empty() {
        remove_file_if_exists(path)?;
    }
    Ok(())
}

/// 安全清空工具目錄下的可重建項目，但保留 `.GITKEEP` 與 `.bundled-*` 標記檔並拒絕符號連結。
///
/// # 參數
/// - `path`: 工具目錄路徑。
///
/// # 回傳值
/// 清理成功回傳 `Ok(())`；目錄無效或遭遇系統權限錯誤回傳錯誤字串。
fn clear_engine_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("無法建立工具目錄：{error}"))?;
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("無法檢查工具目錄：{error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("工具目錄不是安全的資料夾，未移除任何工具。".into());
    }

    for entry in fs::read_dir(path).map_err(|error| format!("無法讀取工具目錄：{error}"))?
    {
        let entry = entry.map_err(|error| format!("無法讀取工具目錄項目：{error}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".GITKEEP" || name.starts_with(".bundled-") {
            continue;
        }
        let entry_path = entry.path();
        let entry_type = fs::symlink_metadata(&entry_path)
            .map_err(|error| format!("無法檢查工具目錄項目：{error}"))?
            .file_type();
        if entry_type.is_dir() {
            fs::remove_dir_all(&entry_path)
                .map_err(|error| format!("無法清除工具目錄項目：{error}"))?;
        } else {
            fs::remove_file(&entry_path)
                .map_err(|error| format!("無法清除工具目錄項目：{error}"))?;
        }
    }
    Ok(())
}

/// 刪除單一檔案並對「找不到檔案」視為成功，其餘 I/O 錯誤轉為可讀訊息。
///
/// # 參數
/// - `path`: 欲移除的檔案路徑。
///
/// # 回傳值
/// 成功或檔案不存在回傳 `Ok(())`；其他刪除失敗回傳錯誤字串。
fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("無法清除工具快取：{error}")),
    }
}

/// 將單行診斷或腳本輸出附加寫入安裝日誌檔尾端。
///
/// # 參數
/// - `path`: 安裝日誌檔案（`install.log`）路徑。
/// - `message`: 欲附加的文字內容。
fn append_log(path: &Path, message: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

/// 合併外部程序輸出的 `stdout` 與非空 `stderr` 為單一 UTF-8 字串。
///
/// # 參數
/// - `output`: 外部程序的 `Output` 結構參考。
///
/// # 回傳值
/// 回傳合併後的文字內容。
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

/// 更新共用依賴狀態快照為 `installing` 並刷新介面顯示訊息。
///
/// # 參數
/// - `snapshot`: 受互斥鎖保護的共用 `DependencySnapshot`。
/// - `message`: 最新的階段提示文字。
fn publish_installing(snapshot: &Arc<Mutex<DependencySnapshot>>, message: &str) {
    if let Ok(mut snapshot) = snapshot.lock() {
        snapshot.status = "installing".into();
        snapshot.message = message.to_owned();
    }
}

/// 取得目前的 Unix 時間戳記（單位：秒）。
///
/// # 回傳值
/// 回傳自 Unix 紀元以來的秒數；若系統時間異常則回傳 `0`。
fn unix_time_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 在 macOS 下為 shell 命令參數加上單引號跳脫保護，防止空白或特殊字元被 Shell 誤解析。
///
/// # 參數
/// - `value`: 欲跳脫的原始字串（例如路徑）。
///
/// # 回傳值
/// 回傳經過單引號跳脫包覆的安全字串。
#[cfg(target_os = "macos")]
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "../../../../desktop-test/backend/unit-tests/dependencies.rs"]
mod tests;
