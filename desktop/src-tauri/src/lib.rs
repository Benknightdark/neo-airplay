//! Neo AirPlay 桌面應用程式核心程式庫，依 Clean Architecture 劃分為領域、應用、基礎設施與表現層。

/// 應用層：協調設定、工具、投影引擎與診斷紀錄的使用案例與介面契約。
pub mod application;
/// 領域層：包含純粹的業務模型、驗證規則、錯誤型別與領域邏輯。
pub mod domain;
/// 基礎設施層：提供檔案設定保存、外部程序執行、狀態管理與依賴套件安裝實作。
pub mod infrastructure;
/// 表現層：負責 Tauri 命令橋接、自動啟動設定與應用程式啟動流程。
mod presentation;

pub use presentation::bootstrap::run;
