//! 應用層模組進入點，彙整命令型與查詢型使用案例、介面契約及輸出入埠定義。

/// 寫入或狀態變更類型的使用案例（Commands）。
pub mod commands;
/// 跨層傳遞的資料傳輸契約（DTOs）。
pub mod contracts;
/// 供基礎設施層實作的抽象介面（Ports）。
pub mod ports;
/// 唯讀查詢類型的使用案例（Queries）。
pub mod queries;

pub use commands::{
    install_dependencies, manage_dependencies, refresh_android_devices, start_airplay,
    start_android_mirror, stop_airplay, stop_android_mirror, SaveSettingsUseCase,
};
pub use contracts::DashboardSnapshot;
pub use ports::{
    AutostartGateway, DependencyRuntime, EngineAccessGuard, LogRepository, ReceiverRuntime,
    SettingsRepository,
};
pub use queries::{get_dashboard, get_dependencies, read_install_log};

#[cfg(test)]
#[path = "../../../../desktop-test/backend/unit-tests/application.rs"]
mod tests;
