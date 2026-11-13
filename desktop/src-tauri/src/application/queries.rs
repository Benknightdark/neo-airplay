//! 應用層唯讀查詢使用案例（Queries），負責回傳各頁面所需快照與日誌文字。

use crate::application::contracts::DashboardSnapshot;
use crate::application::ports::{DependencyRuntime, ReceiverRuntime};
use crate::domain::{DependencySnapshot, DomainResult};

/// 查詢儀表板所需的即時狀態、裝置與診斷日誌快照。
///
/// # 參數
/// - `runtime`: 實作 `ReceiverRuntime` 埠的接收器執行期服務。
///
/// # 回傳值
/// 回傳最新的 `DashboardSnapshot`，若存取狀態失敗則回傳 `DomainError`。
pub fn get_dashboard(runtime: &dyn ReceiverRuntime) -> DomainResult<DashboardSnapshot> {
    runtime.dashboard()
}

/// 查詢投影依賴工具的目前檢查／安裝狀態快照。
///
/// # 參數
/// - `dependencies`: 實作 `DependencyRuntime` 埠的依賴管理服務。
///
/// # 回傳值
/// 回傳 `DependencySnapshot` 或底層查詢錯誤。
pub fn get_dependencies(dependencies: &dyn DependencyRuntime) -> DomainResult<DependencySnapshot> {
    dependencies.snapshot()
}

/// 讀取最近一次依賴工具安裝或維護腳本所寫入的詳細日誌內容。
///
/// # 參數
/// - `dependencies`: 實作 `DependencyRuntime` 埠的依賴管理服務。
///
/// # 回傳值
/// 回傳日誌檔純文字內容，若檔案無法讀取則回傳 `DomainError`。
pub fn read_install_log(dependencies: &dyn DependencyRuntime) -> DomainResult<String> {
    dependencies.read_install_log()
}
