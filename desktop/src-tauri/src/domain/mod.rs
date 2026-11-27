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
/// 跨作業系統投影工具目錄模型、版本與雜湊白名單驗證及上游版本解析邏輯。
pub mod tool_catalog;

pub use android::{is_valid_android_serial, parse_adb_devices, AndroidDevice, AndroidDeviceStatus};
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
    available_renderers, renderer_argument, sanitize_airplay_pin, AirplayAuthMode, ReceiverName,
    ReceiverSettings, VideoRenderer,
};
pub use tool_catalog::{
    build_tool_catalog_snapshot, default_scrcpy_sha256, default_uxplay_spec,
    parse_arch_gstreamer_version, parse_debian_gstreamer_version, parse_dnf_gstreamer_version,
    parse_fedora_gstreamer_version, parse_github_releases_atom, parse_github_scrcpy_releases_json,
    parse_github_uxplay_releases_json, parse_homebrew_gstreamer_version,
    parse_installed_adb_version, parse_installed_gstreamer_version, parse_installed_scrcpy_version,
    parse_installed_uxplay_version, parse_msys2_gstreamer_version, parse_scrcpy_sha256sums,
    parse_ubuntu_gstreamer_version, scrcpy_asset_name, validate_catalog_url,
    validate_optional_sha256, validate_scrcpy_version, validate_tool_install_request,
    validate_uxplay_ref, CatalogContext, InstalledVersions, ManagedToolInfo, PinnedToolVersion,
    PlatformOs, RemoteCatalogCache, ToolCatalogSnapshot, ToolId, ToolInstallRequest,
    ToolReleaseOption, ToolVersionPreferences, ValidatedToolInstall, VersionControlMode,
    ALLOWED_CATALOG_HOSTS, ARCH_GSTREAMER_PACKAGES_URL, DEFAULT_SCRCPY_VERSION,
    DEFAULT_UXPLAY_COMMIT_UNIX, DEFAULT_UXPLAY_COMMIT_WINDOWS, DEFAULT_UXPLAY_SHA256_UNIX,
    DEFAULT_UXPLAY_SHA256_WINDOWS, DEFAULT_UXPLAY_VERSION_UNIX, DEFAULT_UXPLAY_VERSION_WINDOWS,
    FEDORA_GSTREAMER_PACKAGES_URL, HOMEBREW_GSTREAMER_API_URL, MSYS2_GSTREAMER_PAGE_URL,
    SCRCPY_RELEASES_API_URL, SCRCPY_RELEASES_ATOM_URL, UXPLAY_RELEASES_API_URL,
    UXPLAY_RELEASES_ATOM_URL, UXPLAY_TAGS_API_URL,
};

#[cfg(test)]
#[path = "../../../../desktop-test/backend/unit-tests/models.rs"]
mod tests;
