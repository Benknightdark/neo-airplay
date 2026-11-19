//! 應用層唯讀查詢使用案例（Queries），負責回傳各頁面所需快照與日誌文字。

use crate::application::contracts::DashboardSnapshot;
use crate::application::ports::{DependencyRuntime, ReceiverRuntime, ToolCatalogFetcher};
use crate::domain::{DependencySnapshot, DomainResult, ToolCatalogSnapshot};

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

/// 查詢依目前作業系統組裝之工具清單、本機已安裝版本與線上版本快取快照。
///
/// # 參數
/// - `dependencies`: 實作 `DependencyRuntime` 埠的依賴管理服務。
///
/// # 回傳值
/// 回傳包含作業系統特徵與各工具資訊的 `ToolCatalogSnapshot`。
pub fn get_tool_catalog(dependencies: &dyn DependencyRuntime) -> DomainResult<ToolCatalogSnapshot> {
    dependencies.tool_catalog()
}

/// 從官方白名單來源爬取各工具線上發行版本資訊並回傳最新目錄快照。
///
/// # 參數
/// - `dependencies`: 實作 `DependencyRuntime` 埠的依賴管理服務。
/// - `fetcher`: 負責安全 HTTPS 文字擷取的 `ToolCatalogFetcher` 埠實作。
///
/// # 回傳值
/// 回傳包含更新後線上版本清單（或部分降級錯誤）之 `ToolCatalogSnapshot`。
pub fn fetch_tool_versions(
    dependencies: &dyn DependencyRuntime,
    fetcher: &dyn ToolCatalogFetcher,
) -> DomainResult<ToolCatalogSnapshot> {
    dependencies.refresh_tool_catalog(fetcher)
}
