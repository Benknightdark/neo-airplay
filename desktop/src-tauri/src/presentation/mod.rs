//! 表現層模組進入點，負責對接 Tauri 框架、桌面系統整合與命令分派。

/// 作業系統登入自動啟動轉接器。
pub mod autostart;
/// 應用程式建置、系統匣與啟動生命週期組裝。
pub mod bootstrap;
/// 暴露給前端呼叫的 Tauri `invoke` 命令進入點。
pub mod commands;
