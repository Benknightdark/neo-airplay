//! 後端應用層使用案例（設定儲存回滾、引擎存取守衛、維護作業前置停止）的單元測試與假物件定義。

use super::commands::{
    manage_dependencies, refresh_android_devices, start_airplay, start_android_mirror,
    SaveSettingsUseCase,
};
use super::contracts::DashboardSnapshot;
use super::ports::{AutostartGateway, DependencyRuntime, EngineAccessGuard, ReceiverRuntime};
use crate::domain::{
    AndroidDevice, DependencyAction, DependencyOperation, DependencySnapshot, DomainError,
    DomainResult, EngineSnapshot, EngineStatus, ErrorKind, ReceiverSettings,
};
use std::sync::Mutex;

/// 模擬 `ReceiverRuntime` 埠的記憶體測試替身，記錄停止順序與資訊日誌。
struct FakeRuntime {
    /// 記憶體中保存的設定狀態。
    settings: Mutex<ReceiverSettings>,
    /// 設為 `true` 時模擬 `save_settings` 寫入磁碟失敗。
    fail_save_settings: bool,
    /// `stop_airplay` 呼叫後預設回傳的引擎狀態快照。
    airplay_stop_snapshot: Mutex<EngineSnapshot>,
    /// `stop_android_mirror` 呼叫後預設回傳的引擎狀態快照。
    android_stop_snapshot: Mutex<EngineSnapshot>,
    /// 依序記錄被呼叫停止的引擎名稱（`"airplay"` / `"android"`）。
    stopped_order: Mutex<Vec<&'static str>>,
    /// 記錄呼叫 `log_info` 時寫入的訊息清單。
    logged_info: Mutex<Vec<String>>,
}

impl FakeRuntime {
    /// 以指定初始設定建立 `FakeRuntime` 測試替身。
    ///
    /// # 參數
    /// - `settings`: 模擬環境中的起始 `ReceiverSettings`。
    ///
    /// # 回傳值
    /// 回傳預設不失敗的 `FakeRuntime`。
    fn new(settings: ReceiverSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
            fail_save_settings: false,
            airplay_stop_snapshot: Mutex::new(EngineSnapshot::stopped("AirPlay 已停止。")),
            android_stop_snapshot: Mutex::new(EngineSnapshot::stopped("Android 已停止。")),
            stopped_order: Mutex::new(Vec::new()),
            logged_info: Mutex::new(Vec::new()),
        }
    }
}

impl ReceiverRuntime for FakeRuntime {
    /// 取得記憶體中目前儲存的設定副本。
    ///
    /// # 回傳值
    /// 回傳目前 `ReceiverSettings`。
    fn settings(&self) -> DomainResult<ReceiverSettings> {
        Ok(self.settings.lock().unwrap().clone())
    }

    /// 儲存設定至記憶體；當 `fail_save_settings` 為 `true` 時回傳儲存體錯誤。
    ///
    /// # 參數
    /// - `settings`: 欲儲存的設定內容。
    ///
    /// # 回傳值
    /// 成功回傳 `Ok(())`，模擬失敗時回傳 `DomainError::storage`。
    fn save_settings(&self, settings: ReceiverSettings) -> DomainResult<()> {
        if self.fail_save_settings {
            return Err(DomainError::storage("模擬寫入設定檔失敗"));
        }
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }

    /// 產生供測試使用的最小 `DashboardSnapshot`。
    ///
    /// # 回傳值
    /// 回傳含預設雙引擎停止狀態與空清單的快照。
    fn dashboard(&self) -> DomainResult<DashboardSnapshot> {
        Ok(DashboardSnapshot {
            settings: self.settings()?,
            renderer_options: vec!["Auto".to_owned()],
            airplay: EngineSnapshot::stopped("stopped"),
            android: EngineSnapshot::stopped("stopped"),
            devices: Vec::new(),
            logs: Vec::new(),
        })
    }

    /// 模擬啟動 AirPlay 接收器成功。
    ///
    /// # 回傳值
    /// 回傳 `EngineStatus::Running` 快照。
    fn start_airplay(&self) -> DomainResult<EngineSnapshot> {
        Ok(EngineSnapshot {
            status: EngineStatus::Running,
            message: "running".to_owned(),
            process_id: Some(10),
        })
    }

    /// 模擬停止 AirPlay 並將 `"airplay"` 記下以驗證呼叫順序。
    ///
    /// # 回傳值
    /// 回傳預先設定的 `airplay_stop_snapshot`。
    fn stop_airplay(&self) -> DomainResult<EngineSnapshot> {
        self.stopped_order.lock().unwrap().push("airplay");
        Ok(self.airplay_stop_snapshot.lock().unwrap().clone())
    }

    /// 模擬重新整理 Android 裝置列表並回傳空清單。
    ///
    /// # 回傳值
    /// 回傳空 `Vec<AndroidDevice>`。
    fn refresh_android_devices(&self) -> DomainResult<Vec<AndroidDevice>> {
        Ok(Vec::new())
    }

    /// 模擬啟動指定序號的 Android 畫面投影成功。
    ///
    /// # 參數
    /// - `serial`: 目標裝置序號。
    ///
    /// # 回傳值
    /// 回傳包含 `mirror {serial}` 訊息的 `EngineStatus::Running` 快照。
    fn start_android_mirror(&self, serial: &str) -> DomainResult<EngineSnapshot> {
        Ok(EngineSnapshot {
            status: EngineStatus::Running,
            message: format!("mirror {serial}"),
            process_id: Some(20),
        })
    }

    /// 模擬停止 Android 投影並將 `"android"` 記下以驗證呼叫順序。
    ///
    /// # 回傳值
    /// 回傳預先設定的 `android_stop_snapshot`。
    fn stop_android_mirror(&self) -> DomainResult<EngineSnapshot> {
        self.stopped_order.lock().unwrap().push("android");
        Ok(self.android_stop_snapshot.lock().unwrap().clone())
    }

    /// 將資訊訊息記錄至 `logged_info` 清單。
    ///
    /// # 參數
    /// - `message`: 紀錄文字。
    fn log_info(&self, message: &str) {
        self.logged_info.lock().unwrap().push(message.to_owned());
    }
}

/// 模擬 `AutostartGateway` 埠的測試替身，追蹤所有切換歷程與可設定的錯誤注入點。
struct FakeAutostart {
    /// 依時序記錄每次 `set_enabled` 收到的布林值。
    history: Mutex<Vec<bool>>,
    /// 若設為 `Some(flag)`，當傳入相同值時模擬外部程序設定失敗。
    fail_on: Option<bool>,
}

impl FakeAutostart {
    /// 建立預設不失敗且歷程為空的 `FakeAutostart`。
    ///
    /// # 回傳值
    /// 回傳新的 `FakeAutostart` 實例。
    fn new() -> Self {
        Self {
            history: Mutex::new(Vec::new()),
            fail_on: None,
        }
    }
}

impl AutostartGateway for FakeAutostart {
    /// 模擬設定系統自動啟動並記錄操作歷程。
    ///
    /// # 參數
    /// - `enabled`: 目標自動啟動狀態。
    ///
    /// # 回傳值
    /// 命中 `fail_on` 時回傳 `DomainError::external_process`，否則回傳 `Ok(())`。
    fn set_enabled(&self, enabled: bool) -> DomainResult<()> {
        if self.fail_on == Some(enabled) {
            return Err(DomainError::external_process("模擬系統啟動設定失敗"));
        }
        self.history.lock().unwrap().push(enabled);
        Ok(())
    }
}

/// 模擬 `EngineAccessGuard` 的測試替身，可控制依賴工具是否忙碌。
struct FakeGuard {
    /// `true` 表示工具維護中並應阻擋引擎存取。
    busy: bool,
}

impl EngineAccessGuard for FakeGuard {
    /// 若不忙碌則執行傳入閉包，否則回傳 `DependencyBusy` 錯誤。
    ///
    /// # 參數
    /// - `action`: 確認可用時執行的可變閉包。
    ///
    /// # 回傳值
    /// 忙碌時回傳 `DomainError::dependency_busy`；否則回傳 `Ok(())`。
    fn execute_if_available(&self, action: &mut dyn FnMut()) -> DomainResult<()> {
        if self.busy {
            return Err(DomainError::dependency_busy(
                "工具正在檢查或維護，請稍候再試。",
            ));
        }
        action();
        Ok(())
    }
}

/// 模擬 `DependencyRuntime` 埠的測試替身，記錄實際放行的維護動作。
struct FakeDependencyRuntime {
    /// 記錄成功通過前置準備並執行的 `DependencyAction` 清單。
    managed_actions: Mutex<Vec<DependencyAction>>,
}

impl FakeDependencyRuntime {
    /// 建立初始紀錄為空的 `FakeDependencyRuntime`。
    ///
    /// # 回傳值
    /// 回傳新的 `FakeDependencyRuntime` 實例。
    fn new() -> Self {
        Self {
            managed_actions: Mutex::new(Vec::new()),
        }
    }
}

impl DependencyRuntime for FakeDependencyRuntime {
    /// 回傳固定就緒的 `DependencySnapshot`。
    ///
    /// # 回傳值
    /// 回傳 `status` 為 `"ready"` 的快照。
    fn snapshot(&self) -> DomainResult<DependencySnapshot> {
        Ok(DependencySnapshot {
            status: "ready".into(),
            operation: None,
            message: "已就緒".into(),
            missing: Vec::new(),
            log_path: String::new(),
        })
    }

    /// 回傳固定 `"log"` 字串作為模擬安裝日誌。
    ///
    /// # 回傳值
    /// 回傳 `"log"` 字串。
    fn read_install_log(&self) -> DomainResult<String> {
        Ok("log".into())
    }

    /// 直接回傳現有模擬快照。
    ///
    /// # 回傳值
    /// 回傳就緒狀態 `DependencySnapshot`。
    fn start_install(&self) -> DomainResult<DependencySnapshot> {
        self.snapshot()
    }

    /// 先執行 `prepare` 閉包，成功後才記錄維護動作並回傳安裝中快照。
    ///
    /// # 參數
    /// - `action`: 維護動作類型。
    /// - `prepare`: 前置停止服務閉包。
    ///
    /// # 回傳值
    /// `prepare` 成功時回傳 `"installing"` 快照；若 `prepare` 失敗則直接傳遞其 `DomainError`。
    fn manage(
        &self,
        action: DependencyAction,
        prepare: &mut dyn FnMut() -> DomainResult<()>,
    ) -> DomainResult<DependencySnapshot> {
        prepare()?;
        self.managed_actions.lock().unwrap().push(action);
        Ok(DependencySnapshot {
            status: "installing".into(),
            operation: Some(DependencyOperation::from(action)),
            message: "執行中".into(),
            missing: Vec::new(),
            log_path: String::new(),
        })
    }
}

/// 驗證當設定寫入磁碟失敗時，`SaveSettingsUseCase` 會補償回滾已變更的系統自啟設定且不記錄成功日誌。
#[test]
fn save_settings_rolls_back_autostart_when_persistence_fails() {
    let mut runtime = FakeRuntime::new(ReceiverSettings {
        start_on_login: false,
        ..ReceiverSettings::default()
    });
    runtime.fail_save_settings = true;
    let autostart = FakeAutostart::new();
    let use_case = SaveSettingsUseCase::default();

    let result = use_case.execute(
        &runtime,
        &autostart,
        ReceiverSettings {
            receiver_name: "  Living Room  ".to_owned(),
            start_on_login: true,
            ..ReceiverSettings::default()
        },
    );

    assert_eq!(result.unwrap_err().kind, ErrorKind::Storage);
    assert_eq!(*autostart.history.lock().unwrap(), vec![true, false]);
    assert!(runtime.logged_info.lock().unwrap().is_empty());
}

/// 驗證設定儲存成功時會正規化空白欄位並記錄「設定已儲存。」日誌。
#[test]
fn save_settings_sanitizes_and_logs_on_success() {
    let runtime = FakeRuntime::new(ReceiverSettings::default());
    let autostart = FakeAutostart::new();
    let use_case = SaveSettingsUseCase::default();

    let saved = use_case
        .execute(
            &runtime,
            &autostart,
            ReceiverSettings {
                receiver_name: "   ".to_owned(),
                renderer: "invalid".to_owned(),
                fullscreen: true,
                start_on_login: true,
            },
        )
        .unwrap();

    assert_eq!(saved.receiver_name, "Neo AirPlay");
    assert_eq!(saved.renderer, "Auto");
    assert!(saved.fullscreen);
    assert_eq!(*autostart.history.lock().unwrap(), vec![true]);
    assert_eq!(*runtime.logged_info.lock().unwrap(), vec!["設定已儲存。"]);
}

/// 驗證執行依賴維護前會先停止 AirPlay 與 Android 引擎；若任一停止失敗則阻擋移除工具。
#[test]
fn manage_dependencies_stops_both_engines_and_blocks_if_faulted() {
    let runtime = FakeRuntime::new(ReceiverSettings::default());
    let deps = FakeDependencyRuntime::new();

    let snapshot = manage_dependencies(&runtime, &deps, DependencyAction::Upgrade).unwrap();
    assert_eq!(snapshot.operation, Some(DependencyOperation::Upgrade));
    assert_eq!(
        *runtime.stopped_order.lock().unwrap(),
        vec!["airplay", "android"]
    );
    assert_eq!(
        *deps.managed_actions.lock().unwrap(),
        vec![DependencyAction::Upgrade]
    );

    *runtime.airplay_stop_snapshot.lock().unwrap() = EngineSnapshot::faulted("無法停止", None);
    let err = manage_dependencies(&runtime, &deps, DependencyAction::Reinstall).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ExternalProcess);
    assert!(err.message.contains("工具檔案未移除"));
    assert_eq!(deps.managed_actions.lock().unwrap().len(), 1);
}

/// 驗證當依賴背景作業進行中時 `EngineAccessGuard` 會攔截引擎啟動與裝置掃描請求，並檢查空序號驗證規則。
#[test]
fn engine_access_guard_blocks_operations_while_dependency_worker_is_busy() {
    let runtime = FakeRuntime::new(ReceiverSettings::default());
    let busy_guard = FakeGuard { busy: true };

    assert_eq!(
        start_airplay(&runtime, &busy_guard).unwrap_err().kind,
        ErrorKind::DependencyBusy
    );
    assert_eq!(
        refresh_android_devices(&runtime, &busy_guard)
            .unwrap_err()
            .kind,
        ErrorKind::DependencyBusy
    );
    assert_eq!(
        start_android_mirror(&runtime, &busy_guard, "serial-1")
            .unwrap_err()
            .kind,
        ErrorKind::DependencyBusy
    );

    let free_guard = FakeGuard { busy: false };
    assert_eq!(
        start_android_mirror(&runtime, &free_guard, "  ")
            .unwrap_err()
            .kind,
        ErrorKind::Validation
    );
    assert_eq!(
        start_android_mirror(&runtime, &free_guard, "serial-1")
            .unwrap()
            .status,
        EngineStatus::Running
    );
}
