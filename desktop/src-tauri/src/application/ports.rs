//! 應用層定義的抽象輸出埠（Output Ports），用於反轉對外圍實作與系統框架的依賴。

use crate::application::contracts::DashboardSnapshot;
use crate::domain::{
    AndroidDevice, AppLogEntry, DependencyAction, DependencySnapshot, DomainResult, EngineSnapshot,
    LogLevel, ReceiverSettings,
};

/// 接收器設定持久化儲存介面。
pub trait SettingsRepository: Send + Sync {
    /// 從儲存媒體讀取接收器設定。
    ///
    /// # 回傳值
    /// 回傳載入並正規化後的 `ReceiverSettings`；若讀取失敗則回傳 `DomainError`。
    fn load(&self) -> DomainResult<ReceiverSettings>;

    /// 將接收器設定寫入儲存媒體。
    ///
    /// # 參數
    /// - `settings`: 欲保存的設定實例參考。
    ///
    /// # 回傳值
    /// 成功寫入回傳 `Ok(())`；失敗時回傳 `DomainError`。
    fn save(&self, settings: &ReceiverSettings) -> DomainResult<()>;
}

/// 作業系統開機／登入自動啟動設定閘道介面。
pub trait AutostartGateway: Send + Sync {
    /// 切換應用程式是否於使用者登入時自動啟動。
    ///
    /// # 參數
    /// - `enabled`: `true` 代表啟用自動啟動，`false` 代表停用。
    ///
    /// # 回傳值
    /// 設定套用成功回傳 `Ok(())`，失敗回傳 `DomainError`。
    fn set_enabled(&self, enabled: bool) -> DomainResult<()>;
}

/// 應用程式診斷日誌儲存與讀取介面。
pub trait LogRepository: Send + Sync {
    /// 寫入一筆診斷日誌紀錄。
    ///
    /// # 參數
    /// - `level`: 日誌嚴重等級。
    /// - `message`: 紀錄內容字串。
    fn write(&self, level: LogLevel, message: &str);

    /// 取得目前保留的近期診斷日誌清單。
    ///
    /// # 回傳值
    /// 回傳依時間先後排列的 `AppLogEntry` 向量。
    fn recent(&self) -> Vec<AppLogEntry>;
}

/// 接收器與投影引擎核心執行期控制介面。
pub trait ReceiverRuntime: Send + Sync {
    /// 取得目前生效中的接收器設定。
    ///
    /// # 回傳值
    /// 回傳目前的 `ReceiverSettings` 或讀取錯誤。
    fn settings(&self) -> DomainResult<ReceiverSettings>;

    /// 驗證並套用新的接收器設定。
    ///
    /// # 參數
    /// - `settings`: 欲套用的新設定內容。
    ///
    /// # 回傳值
    /// 套用與寫入成功回傳 `Ok(())`，否則回傳 `DomainError`。
    fn save_settings(&self, settings: ReceiverSettings) -> DomainResult<()>;

    /// 彙整目前儀表板所需的所有即時狀態與日誌。
    ///
    /// # 回傳值
    /// 回傳組合完成的 `DashboardSnapshot`。
    fn dashboard(&self) -> DomainResult<DashboardSnapshot>;

    /// 啟動 AirPlay 接收器引擎。
    ///
    /// # 回傳值
    /// 回傳啟動結果的 `EngineSnapshot`；啟動失敗時回傳 `DomainError`。
    fn start_airplay(&self) -> DomainResult<EngineSnapshot>;

    /// 停止目前運作中的 AirPlay 接收器引擎。
    ///
    /// # 回傳值
    /// 回傳停止後的 `EngineSnapshot`。
    fn stop_airplay(&self) -> DomainResult<EngineSnapshot>;

    /// 重新掃描已透過 USB 或網路連接的 Android 裝置列表。
    ///
    /// # 回傳值
    /// 回傳掃描到的 `AndroidDevice` 清單。
    fn refresh_android_devices(&self) -> DomainResult<Vec<AndroidDevice>>;

    /// 針對指定 Android 裝置啟動畫面投影。
    ///
    /// # 參數
    /// - `serial`: 目標裝置的 ADB 序號字串。
    ///
    /// # 回傳值
    /// 回傳啟動後的 Android 引擎狀態快照 `EngineSnapshot`。
    fn start_android_mirror(&self, serial: &str) -> DomainResult<EngineSnapshot>;

    /// 停止目前的 Android 畫面投影引擎。
    ///
    /// # 回傳值
    /// 回傳停止後的 `EngineSnapshot`。
    fn stop_android_mirror(&self) -> DomainResult<EngineSnapshot>;

    /// 寫入一筆資訊等級（Information）的系統操作日誌。
    ///
    /// # 參數
    /// - `message`: 欲寫入的資訊訊息文字。
    fn log_info(&self, message: &str);
}

/// 控制執行引擎與維護作業互斥的存取守衛介面，避免安裝工具時與運行中引擎衝突。
pub trait EngineAccessGuard: Send + Sync {
    /// 在沒有投影引擎佔用或衝突的前提下執行指定動作。
    ///
    /// # 參數
    /// - `action`: 確認可安全執行後要呼叫的閉包。
    ///
    /// # 回傳值
    /// 成功執行回傳 `Ok(())`；若被互斥狀態阻擋則回傳 `DomainError`。
    fn execute_if_available(&self, action: &mut dyn FnMut()) -> DomainResult<()>;
}

/// 投影依賴工具管理執行期介面，負責狀態查詢、自動安裝與升級／重裝流程。
pub trait DependencyRuntime: Send + Sync {
    /// 取得目前依賴工具的檢查與安裝狀態快照。
    ///
    /// # 回傳值
    /// 回傳 `DependencySnapshot`。
    fn snapshot(&self) -> DomainResult<DependencySnapshot>;

    /// 讀取工具安裝／維護腳本產生的完整文字日誌內容。
    ///
    /// # 回傳值
    /// 回傳安裝日誌全文字串，若無法讀取則回傳 `DomainError`。
    fn read_install_log(&self) -> DomainResult<String>;

    /// 檢查並視需要啟動背景自動安裝缺少工具的流程。
    ///
    /// # 回傳值
    /// 回傳啟動檢查後的 `DependencySnapshot`。
    fn start_install(&self) -> DomainResult<DependencySnapshot>;

    /// 執行手動觸發的升級或重新安裝維護動作。
    ///
    /// # 參數
    /// - `action`: 要執行的維護動作（`Upgrade` 或 `Reinstall`）。
    /// - `prepare`: 實際啟動維護作業前必須先完成的前置停止／清理閉包。
    ///
    /// # 回傳值
    /// 回傳啟動維護作業後的 `DependencySnapshot`。
    fn manage(
        &self,
        action: DependencyAction,
        prepare: &mut dyn FnMut() -> DomainResult<()>,
    ) -> DomainResult<DependencySnapshot>;
}
