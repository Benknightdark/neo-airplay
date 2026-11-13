//! 領域層模組進入點，集中匯出核心值物件、實體、領域邏輯與錯誤定義。

/// Android 裝置解析與狀態模型。
pub mod android;
/// 外部投影依賴套件與安裝狀態模型。
pub mod dependency;
/// 應用程式執行期診斷日誌模型與緩衝區工具。
pub mod diagnostics;
/// AirPlay 與 Android 投影引擎狀態機與輔助判斷。
pub mod engine;
/// 領域錯誤分類與結果型別別名。
pub mod error;
/// 接收器設定值物件與視訊渲染器對應規則。
pub mod settings;

pub use android::{parse_adb_devices, AndroidDevice, AndroidDeviceStatus};
pub use dependency::{
    first_useful_line, interrupted_install_detail, parse_missing_dependencies, stage_label,
    DependencyAction, DependencyOperation, DependencySnapshot, InstallProgress, InstallResult,
    LaunchRecord, RuntimeCheck, SetupOutcome,
};
pub use diagnostics::{push_recent, AppLogEntry, LogLevel, MAX_RECENT_LOGS};
pub use engine::{
    bonjour_instance_matches, exit_snapshot, EngineKind, EngineSnapshot, EngineStatus,
    AIRPLAY_READY_TIMEOUT,
};
pub use error::{DomainError, DomainResult, ErrorKind};
pub use settings::{
    available_renderers, renderer_argument, ReceiverName, ReceiverSettings, VideoRenderer,
};

#[cfg(test)]
#[path = "../../../../desktop-test/backend/unit-tests/models.rs"]
mod tests;
