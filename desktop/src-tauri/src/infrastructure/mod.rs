//! 基礎設施層模組進入點，實作應用層介面與系統邊界操作（檔案、子程序、作業系統環境）。

/// 依賴工具安裝、升級、重裝與狀態管理實作。
pub mod dependencies;
/// 本機 JSON 設定檔與 JSONL 診斷日誌持久化儲存。
pub mod persistence;
/// 外部執行檔搜尋、環境變數組裝與帶逾時控制的程序執行工具。
pub mod process;
/// 應用程式執行期狀態、引擎生命週期與 ADB 裝置管理實作。
pub mod state;

pub use dependencies::DependencyManager;
pub use persistence::{AppLogger, JsonSettingsRepository};
pub use state::AppState;
