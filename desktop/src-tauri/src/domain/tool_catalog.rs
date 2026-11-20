//! 跨作業系統投影工具目錄模型、版本與雜湊白名單驗證，以及上游版本來源純文字解析邏輯。

use crate::domain::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Unix（macOS / Linux）內建推薦 UxPlay 版本號。
pub const DEFAULT_UXPLAY_VERSION_UNIX: &str = "1.73.7";
/// Unix（macOS / Linux）內建推薦 UxPlay Git Commit SHA。
pub const DEFAULT_UXPLAY_COMMIT_UNIX: &str = "df67c212a433cf6dda3676dd40c097900d24e645";
/// Unix（macOS / Linux）內建推薦 UxPlay 原始碼封存檔 SHA-256。
pub const DEFAULT_UXPLAY_SHA256_UNIX: &str =
    "168de53ed688f87c4022d79357ce91f9608ad5c53236bff95134d19b88efeb3f";

/// Windows 內建推薦 UxPlay 版本號（對應相容修正之推薦提交）。
pub const DEFAULT_UXPLAY_VERSION_WINDOWS: &str = "1.74";
/// Windows 內建推薦 UxPlay Git Commit SHA。
pub const DEFAULT_UXPLAY_COMMIT_WINDOWS: &str = "f524d8aa28028ad5572ae1b462d119e4cec1f9f4";
/// Windows 內建推薦 UxPlay 原始碼封存檔 SHA-256。
pub const DEFAULT_UXPLAY_SHA256_WINDOWS: &str =
    "6cd720c5e2debee120fb076df69e76d35d19d11d9f7e377c3a747d4b8d6b692a";

/// 內建推薦 scrcpy 發行版本號（三平台通用）。
pub const DEFAULT_SCRCPY_VERSION: &str = "5.0.1";
/// macOS ARM64 `scrcpy` 5.0.1 官方壓縮檔 SHA-256。
pub const DEFAULT_SCRCPY_SHA256_MACOS_ARM64: &str =
    "33611e51977a8289e2e124b220f727895181ef2eb9060e0987a4dc2db9cf0d6b";
/// macOS x86_64 `scrcpy` 5.0.1 官方壓縮檔 SHA-256。
pub const DEFAULT_SCRCPY_SHA256_MACOS_X64: &str =
    "31a5467a9e907f162b093267cbbab1276b15c6e709c2ea8159566a8d165beb50";
/// Windows x64 `scrcpy` 5.0.1 官方壓縮檔 SHA-256。
pub const DEFAULT_SCRCPY_SHA256_WIN64: &str =
    "b12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5";
/// Windows ARM64 `scrcpy` 5.0.1 官方壓縮檔 SHA-256。
pub const DEFAULT_SCRCPY_SHA256_WINARM64: &str =
    "16c3fc2068df64946f670e3504b4752073ebae47a416a45b46e79a5ab19c689b";
/// Linux x86_64 `scrcpy` 5.0.1 官方壓縮檔 SHA-256。
pub const DEFAULT_SCRCPY_SHA256_LINUX_X64: &str =
    "9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e";

/// 允許進行線上版本爬取之官方網站主機白名單。
pub const ALLOWED_CATALOG_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "formulae.brew.sh",
    "packages.msys2.org",
    "packages.debian.org",
    "packages.ubuntu.com",
    "packages.fedoraproject.org",
    "archlinux.org",
];

/// GitHub API：`scrcpy` 官方 Releases 清單端點。
pub const SCRCPY_RELEASES_API_URL: &str =
    "https://api.github.com/repos/Genymobile/scrcpy/releases?per_page=15";
/// GitHub Atom：`scrcpy` Releases 備援端點（避免 API 匿名速率限制）。
pub const SCRCPY_RELEASES_ATOM_URL: &str = "https://github.com/Genymobile/scrcpy/releases.atom";
/// GitHub API：`UxPlay` 官方 Releases 清單端點。
pub const UXPLAY_RELEASES_API_URL: &str =
    "https://api.github.com/repos/FDH2/UxPlay/releases?per_page=15";
/// GitHub API：`UxPlay` Tags（用於解析對應 commit SHA）端點。
pub const UXPLAY_TAGS_API_URL: &str = "https://api.github.com/repos/FDH2/UxPlay/tags?per_page=20";
/// GitHub Atom：`UxPlay` Releases 備援端點。
pub const UXPLAY_RELEASES_ATOM_URL: &str = "https://github.com/FDH2/UxPlay/releases.atom";
/// Homebrew Formulae API：macOS GStreamer 最新穩定版端點。
pub const HOMEBREW_GSTREAMER_API_URL: &str = "https://formulae.brew.sh/api/formula/gstreamer.json";
/// MSYS2 UCRT64 套件頁：Windows GStreamer 最新套件版端點。
pub const MSYS2_GSTREAMER_PAGE_URL: &str =
    "https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-gstreamer";
/// Fedora 官方套件頁：Fedora 各版本 GStreamer RPM 套件資訊。
pub const FEDORA_GSTREAMER_PACKAGES_URL: &str =
    "https://packages.fedoraproject.org/pkgs/gstreamer1/gstreamer1/";
/// Arch Linux 官方套件頁：x86_64 GStreamer 套件資訊。
pub const ARCH_GSTREAMER_PACKAGES_URL: &str =
    "https://archlinux.org/packages/extra/x86_64/gstreamer/";

/// 目前執行中的作業系統平台分類。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlatformOs {
    /// Apple macOS。
    Macos,
    /// Microsoft Windows。
    Windows,
    /// Linux 桌面發行版。
    Linux,
}

/// 受 Neo AirPlay 管理或監控之投影工具識別碼。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ToolId {
    /// AirPlay 接收引擎（UxPlay）。
    #[serde(rename = "uxplay")]
    UxPlay,
    /// Android 有線／螢幕投影工具（內含 ADB）。
    Scrcpy,
    /// 視音訊解碼與管線框架（GStreamer）。
    #[serde(rename = "gstreamer")]
    GStreamer,
}

impl ToolId {
    /// 取得傳遞給安裝腳本 `--only`／`-Only` 參數的小寫工具識別名稱。
    ///
    /// # 回傳值
    /// 回傳 `"uxplay"`、`"scrcpy"` 或 `"gstreamer"`。
    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::UxPlay => "uxplay",
            Self::Scrcpy => "scrcpy",
            Self::GStreamer => "gstreamer",
        }
    }
}

/// 單一工具的版本控制權限模式。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VersionControlMode {
    /// 允許使用者自線上清單選擇、手動輸入版本，並單獨下載安裝。
    Selectable,
    /// 由系統套件管理器（Homebrew／MSYS2／apt／dnf／pacman）統一維護版本。
    PackageManager,
}

/// 單一工具之可安裝或線上參考版本項目。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolReleaseOption {
    /// 顯示用與檢核用標準版本字串（不含前導 `v`，若為 commit 則為完整或縮寫識別）。
    pub version: String,
    /// 實際下載參照（GitHub tag 如 `v5.0.1` 或 40 碼 commit SHA）。
    pub reference: String,
    /// 顯示給使用者的版本說明標籤。
    pub label: String,
    /// 上游發佈日期（`YYYY-MM-DD`，若有）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// 是否為上游預覽／pre-release 版本。
    pub prerelease: bool,
    /// 是否為 Neo AirPlay 內建鎖定且推薦的版本。
    pub recommended: bool,
    /// 若已內建或已知 SHA-256 則填入（小寫 64 碼十六進位）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// 單一平台相關工具的完整資訊與版本清單快照。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedToolInfo {
    /// 工具識別碼。
    pub id: ToolId,
    /// 介面顯示名稱。
    pub name: String,
    /// 工具用途簡述。
    pub summary: String,
    /// 依目前 OS 決定的安裝來源說明。
    pub source_label: String,
    /// 上游專案或發行來源 URL。
    pub source_url: String,
    /// 版本控制權限模式。
    pub version_control: VersionControlMode,
    /// 本機 `engine/` 目錄目前偵測到的已安裝版本。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    /// 隨附或相依的子工具資訊（例如隨 scrcpy 打包的 `ADB 35.x`）。
    #[serde(default)]
    pub bundled_components: Vec<String>,
    /// 該 OS 下的 Neo AirPlay 內建推薦版本。
    pub default_version: String,
    /// 該 OS 下內建推薦版本對應的下載參照（commit SHA 或 tag）。
    pub default_reference: String,
    /// 使用者目前已指定並持久化記憶的自訂版本（若使用預設則為 `None`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_version: Option<String>,
    /// 最近一次線上爬取到的最新上游版本。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_remote_version: Option<String>,
    /// 可選版本清單（內建推薦版置頂，後續接續爬取版本）。
    #[serde(default)]
    pub available_versions: Vec<ToolReleaseOption>,
    /// 介面是否允許展開手動 SHA-256 輸入欄。
    pub supports_manual_sha256: bool,
    /// 安裝時是否會自動取得並校驗上游官方 `SHA256SUMS.txt`。
    pub auto_verifies_checksum: bool,
    /// 依平台或架構顯示的額外說明（例如 Linux ARM64 套件庫說明）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// 呈現給「工具管理」頁面的作業系統與工具版本完整快照。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogSnapshot {
    /// 目前執行作業系統。
    pub os: PlatformOs,
    /// CPU 架構識別（如 `aarch64`、`x86_64`）。
    pub arch: String,
    /// 人類可讀的作業系統與架構標籤。
    pub platform_label: String,
    /// 目前平台採用的主要套件管理器描述。
    pub package_manager: String,
    /// 針對目前作業系統之授權、前置與安裝方式提示。
    pub platform_notice: String,
    /// 平台專屬高風險維護警告（例如僅在 Linux `pacman` 顯示之整機升級提醒）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_warning: Option<String>,
    /// 本工作階段是否啟用工具安裝與版本管理。
    pub management_enabled: bool,
    /// 最近一次線上爬取成功的 RFC 3339 時間戳記。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_fetched_at: Option<String>,
    /// 最近一次線上爬取過程中的部分或全部錯誤提示（不影響使用內建或手輸版本）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_error: Option<String>,
    /// 依目前作業系統與架構組裝的受管理與參考工具清單。
    pub tools: Vec<ManagedToolInfo>,
}

/// 前端送出安裝／切換特定工具版本之請求契約。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolInstallRequest {
    /// 目標工具識別碼。
    pub tool: ToolId,
    /// 使用者選擇或手動輸入的版本號、tag 或 40 碼 commit SHA。
    pub version: String,
    /// 使用者選填之 SHA-256 雜湊校驗碼。
    #[serde(default)]
    pub sha256: Option<String>,
    /// 當安裝非預設 `UxPlay` 且未填 SHA-256 時，是否已於介面明確確認以 HTTPS 官方來源安裝。
    #[serde(default)]
    pub allow_unverified: bool,
}

/// 持久化儲存於 `config_dir/tool-versions.json` 的單一工具鎖定版本資訊。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PinnedToolVersion {
    /// 標準化版本字串（或 40 碼 commit SHA）。
    pub version: String,
    /// 實際傳給安裝腳本下載的參照（tag 或 commit SHA）。
    pub reference: String,
    /// 使用者或預設提供的 64 碼 SHA-256（若有）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// 若為未附 SHA-256 的自訂 UxPlay 版本則為 `true`。
    #[serde(default)]
    pub allow_unverified: bool,
}

/// 持久化儲存於 `config_dir/tool-versions.json` 的使用者工具版本偏好檔案模型。
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersionPreferences {
    /// 已自訂鎖定的 UxPlay 版本偏好。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uxplay: Option<PinnedToolVersion>,
    /// 已自訂鎖定的 scrcpy 版本偏好。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrcpy: Option<PinnedToolVersion>,
}

/// 通過領域驗證後產生的確定安裝規格，供基礎設施層傳遞給安裝腳本與成功後持久化。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedToolInstall {
    /// 目標工具識別碼。
    pub tool: ToolId,
    /// 標準化顯示與預期版本。
    pub version: String,
    /// 實際傳入 `--uxplay-ref` 或標準版本的參照值。
    pub reference: String,
    /// 確定使用的 SHA-256（若有）。
    pub sha256: Option<String>,
    /// 是否允許以純 HTTPS TLS 1.2+ 官方來源下載編譯非預設 UxPlay。
    pub allow_unverified: bool,
    /// 是否等同目前 OS 的內建推薦版本（若是則清除自訂偏好覆寫）。
    pub is_default: bool,
}

/// 本機 `engine/` 工具實測偵測到的已安裝版本摘要。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstalledVersions {
    /// 已安裝的 UxPlay 版本。
    pub uxplay: Option<String>,
    /// 已安裝的 scrcpy 版本。
    pub scrcpy: Option<String>,
    /// 已安裝的 ADB 版本。
    pub adb: Option<String>,
    /// 已安裝的 GStreamer 版本。
    pub gstreamer: Option<String>,
}

/// 記憶體快取的線上版本爬取結果。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoteCatalogCache {
    /// 爬取到的 UxPlay 線上版本清單。
    pub uxplay: Vec<ToolReleaseOption>,
    /// 爬取到的 scrcpy 線上版本清單（已篩選目前 OS / 架構）。
    pub scrcpy: Vec<ToolReleaseOption>,
    /// 爬取到的平台 GStreamer 最新穩定／套件版。
    pub gstreamer_latest: Option<String>,
    /// 最近爬取完成時間（RFC 3339）。
    pub fetched_at: Option<String>,
    /// 最近爬取錯誤摘要（若部分來源失敗仍保留其他已取得結果）。
    pub error: Option<String>,
}

/// 傳入作業系統目錄組裝函式之目前執行環境與偏好上下文。
#[derive(Clone, Debug)]
pub struct CatalogContext<'a> {
    /// 目前作業系統。
    pub os: PlatformOs,
    /// 目前 CPU 架構字串。
    pub arch: &'a str,
    /// 偵測到的套件管理器識別或顯示名稱（macOS `"homebrew"`、Windows `"winget-msys2"`、Linux `"apt"`／`"dnf"`／`"pacman"`）。
    pub package_manager: &'a str,
    /// 目前工作階段是否啟用工具管理。
    pub management_enabled: bool,
    /// 本機實測已安裝版本。
    pub installed: &'a InstalledVersions,
    /// 磁碟讀取之使用者自訂版本偏好。
    pub preferences: &'a ToolVersionPreferences,
    /// 記憶體快取之線上爬取結果。
    pub remote: &'a RemoteCatalogCache,
}

/// 驗證線上爬取網址必須使用 HTTPS 並嚴格屬於官方白名單主機，防止 SSRF 或非預期重導向。
///
/// # 參數
/// - `url`: 欲請求之原始或重導向目標網址。
///
/// # 回傳值
/// 合法則回傳 `Ok(())`；不符規則回傳 `DomainError::validation`。
pub fn validate_catalog_url(url: &str) -> DomainResult<()> {
    if url
        .chars()
        .any(|ch| ch.is_control() || ch.is_ascii_whitespace())
    {
        return Err(DomainError::validation("爬取來源網址包含不安全字元。"));
    }
    let after_scheme = url
        .strip_prefix("https://")
        .ok_or_else(|| DomainError::validation("僅允許使用 HTTPS 協定存取官方版本來源。"))?;
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') || authority.contains(':') {
        return Err(DomainError::validation("版本來源網址主機格式無效。"));
    }
    let host = authority.to_ascii_lowercase();
    if !ALLOWED_CATALOG_HOSTS.contains(&host.as_str()) {
        return Err(DomainError::validation(format!(
            "拒絕連線至非官方白名單網域：{host}"
        )));
    }
    Ok(())
}

/// 取得指定作業系統之內建推薦 UxPlay `(版本, commit_sha, sha256)` 設定。
pub fn default_uxplay_spec(os: PlatformOs) -> (&'static str, &'static str, &'static str) {
    match os {
        PlatformOs::Windows => (
            DEFAULT_UXPLAY_VERSION_WINDOWS,
            DEFAULT_UXPLAY_COMMIT_WINDOWS,
            DEFAULT_UXPLAY_SHA256_WINDOWS,
        ),
        PlatformOs::Macos | PlatformOs::Linux => (
            DEFAULT_UXPLAY_VERSION_UNIX,
            DEFAULT_UXPLAY_COMMIT_UNIX,
            DEFAULT_UXPLAY_SHA256_UNIX,
        ),
    }
}

/// 取得指定作業系統與架構之內建 `scrcpy` 5.0.1 官方壓縮檔 SHA-256（若有）。
pub fn default_scrcpy_sha256(os: PlatformOs, arch: &str) -> Option<&'static str> {
    let normalized = normalize_arch(arch);
    match (os, normalized) {
        (PlatformOs::Macos, "arm64") => Some(DEFAULT_SCRCPY_SHA256_MACOS_ARM64),
        (PlatformOs::Macos, "x86_64") => Some(DEFAULT_SCRCPY_SHA256_MACOS_X64),
        (PlatformOs::Windows, "arm64") => Some(DEFAULT_SCRCPY_SHA256_WINARM64),
        (PlatformOs::Windows, "x86_64") => Some(DEFAULT_SCRCPY_SHA256_WIN64),
        (PlatformOs::Linux, "x86_64") => Some(DEFAULT_SCRCPY_SHA256_LINUX_X64),
        _ => None,
    }
}

/// 將各平台常見 CPU 架構名稱正規化為 `"arm64"`、`"x86_64"` 或原小寫字串。
fn normalize_arch(arch: &str) -> &str {
    match arch.trim() {
        "aarch64" | "arm64" | "ARM64" => "arm64",
        "x86_64" | "amd64" | "AMD64" => "x86_64",
        other => other,
    }
}

/// 根據目前 OS 與 CPU 架構推導該平台官方 scrcpy 壓縮檔檔名。
///
/// # 回傳值
/// 支援該平台官方二進位壓縮檔時回傳 `Some(檔名)`；Linux 非 x86_64 等不支援組合回傳 `None`。
pub fn scrcpy_asset_name(os: PlatformOs, arch: &str, version: &str) -> Option<String> {
    let normalized = normalize_arch(arch);
    match (os, normalized) {
        (PlatformOs::Macos, "arm64") => Some(format!("scrcpy-macos-aarch64-v{version}.tar.gz")),
        (PlatformOs::Macos, "x86_64") => Some(format!("scrcpy-macos-x86_64-v{version}.tar.gz")),
        (PlatformOs::Windows, "arm64") => Some(format!("scrcpy-winarm64-v{version}.zip")),
        (PlatformOs::Windows, "x86_64") => Some(format!("scrcpy-win64-v{version}.zip")),
        (PlatformOs::Linux, "x86_64") => Some(format!("scrcpy-linux-x86_64-v{version}.tar.gz")),
        _ => None,
    }
}

/// 驗證並標準化 `scrcpy` 版本字串（僅允許純數字點號格式如 `5.0.1` 或 `v5.0.1`）。
///
/// # 回傳值
/// 回傳去除前導 `v` 之標準版本字串，若含空白、旗標或非數字字元則回傳驗證錯誤。
pub fn validate_scrcpy_version(input: &str) -> DomainResult<String> {
    let trimmed = input.trim();
    let version = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
    if !is_dotted_numeric_version(version, 2, 3) {
        return Err(DomainError::validation(
            "scrcpy 版本格式無效，請輸入例如 5.0.1 或 v4.1 的純版本號。",
        ));
    }
    Ok(version.to_owned())
}

/// 驗證並標準化 `UxPlay` 版本號或 GitHub 40 碼 commit SHA。
///
/// # 回傳值
/// 回傳 `(標準顯示版本或縮寫/commit, 實際下載用 Git ref)`。
pub fn validate_uxplay_ref(input: &str) -> DomainResult<(String, String)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(DomainError::validation(
            "請輸入 UxPlay 版本號或 Git commit。",
        ));
    }
    if is_hex_len(trimmed, 40) {
        let lower = trimmed.to_ascii_lowercase();
        return Ok((lower.clone(), lower));
    }
    let version = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
    if is_dotted_numeric_version(version, 2, 4) {
        return Ok((version.to_owned(), format!("v{version}")));
    }
    Err(DomainError::validation(
        "UxPlay 版本格式無效，請輸入版本號（如 1.73.7）或完整 40 碼十六進位 commit SHA。",
    ))
}

/// 驗證選填的 SHA-256 校驗碼（留白視為未提供；非空則須為精確 64 碼十六進位）。
pub fn validate_optional_sha256(input: Option<&str>) -> DomainResult<Option<String>> {
    let Some(raw) = input.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if !is_hex_len(raw, 64) {
        return Err(DomainError::validation(
            "SHA-256 校驗碼必須為精確 64 碼十六進位字元。",
        ));
    }
    Ok(Some(raw.to_ascii_lowercase()))
}

/// 驗證並解析來自前端的工具安裝請求，執行平台限制、安全白名單與 Option 1A 策略檢查。
pub fn validate_tool_install_request(
    request: &ToolInstallRequest,
    os: PlatformOs,
    arch: &str,
    known_remote: &[ToolReleaseOption],
) -> DomainResult<ValidatedToolInstall> {
    let explicit_sha = validate_optional_sha256(request.sha256.as_deref())?;
    match request.tool {
        ToolId::GStreamer => Err(DomainError::validation(
            "GStreamer 由作業系統套件管理器維護，請使用升級工具統一更新。",
        )),
        ToolId::Scrcpy => {
            if scrcpy_asset_name(os, arch, "1.0").is_none() {
                return Err(DomainError::validation(
                    "目前平台架構不支援安裝官方 scrcpy 發行版，請透過作業系統套件管理器安裝。",
                ));
            }
            let version = validate_scrcpy_version(&request.version)?;
            let is_default = version == DEFAULT_SCRCPY_VERSION && explicit_sha.is_none();
            let sha256 = if version == DEFAULT_SCRCPY_VERSION {
                explicit_sha.or_else(|| default_scrcpy_sha256(os, arch).map(str::to_owned))
            } else {
                explicit_sha
            };
            Ok(ValidatedToolInstall {
                tool: ToolId::Scrcpy,
                reference: format!("v{version}"),
                version,
                sha256,
                allow_unverified: false,
                is_default,
            })
        }
        ToolId::UxPlay => {
            let (default_version, default_commit, default_sha) = default_uxplay_spec(os);
            let (normalized_version, normalized_ref) = validate_uxplay_ref(&request.version)?;
            let matches_default = normalized_version == default_version
                || normalized_ref == default_commit
                || normalized_ref == format!("v{default_version}");
            if matches_default && explicit_sha.is_none() {
                return Ok(ValidatedToolInstall {
                    tool: ToolId::UxPlay,
                    version: default_version.to_owned(),
                    reference: default_commit.to_owned(),
                    sha256: Some(default_sha.to_owned()),
                    allow_unverified: false,
                    is_default: true,
                });
            }

            // 優先採用線上 tags 對應到的不可變 commit SHA（若與使用者所選版本相符）
            let resolved_ref = known_remote
                .iter()
                .find(|option| {
                    option.version == normalized_version || option.reference == normalized_ref
                })
                .map(|option| option.reference.clone())
                .unwrap_or(normalized_ref);
            let resolved_sha = explicit_sha.or_else(|| {
                known_remote
                    .iter()
                    .find(|option| option.version == normalized_version)
                    .and_then(|option| option.sha256.clone())
            });

            if resolved_sha.is_none() && !request.allow_unverified {
                return Err(DomainError::validation(
                    "安裝非內建預設版本的 UxPlay 需要提供 SHA-256 校驗碼，或明確確認以 HTTPS 官方 GitHub 來源下載。",
                ));
            }

            Ok(ValidatedToolInstall {
                tool: ToolId::UxPlay,
                version: normalized_version,
                reference: resolved_ref,
                sha256: resolved_sha,
                allow_unverified: request.allow_unverified,
                is_default: false,
            })
        }
    }
}

/// 解析 GitHub Releases JSON（`Genymobile/scrcpy`），並僅保留含目前平台架構封裝之版本。
pub fn parse_github_scrcpy_releases_json(
    body: &str,
    os: PlatformOs,
    arch: &str,
) -> DomainResult<Vec<ToolReleaseOption>> {
    let entries: Vec<Value> = serde_json::from_str(body).map_err(|error| {
        DomainError::external_process(format!("解析 scrcpy 版本清單失敗：{error}"))
    })?;
    let mut options = Vec::new();
    let mut seen = HashSet::new();

    for entry in entries {
        if entry["draft"].as_bool().unwrap_or(false) {
            continue;
        }
        let Some(tag_name) = entry["tag_name"].as_str() else {
            continue;
        };
        let Ok(version) = validate_scrcpy_version(tag_name) else {
            continue;
        };
        let Some(expected_asset) = scrcpy_asset_name(os, arch, &version) else {
            continue;
        };
        if let Some(assets) = entry["assets"].as_array() {
            if !assets
                .iter()
                .any(|asset| asset["name"].as_str() == Some(expected_asset.as_str()))
            {
                continue;
            }
        }
        if !seen.insert(version.clone()) {
            continue;
        }
        let published_at = entry["published_at"].as_str().and_then(extract_iso_date);
        let prerelease = entry["prerelease"].as_bool().unwrap_or(false);
        options.push(ToolReleaseOption {
            version: version.clone(),
            reference: tag_name.to_owned(),
            label: format!("v{version}"),
            published_at,
            prerelease,
            recommended: false,
            sha256: None,
        });
    }
    Ok(options)
}

/// 解析 GitHub Releases 與 Tags JSON（`FDH2/UxPlay`），盡可能將每個 release 對應至不可變 commit SHA。
pub fn parse_github_uxplay_releases_json(
    releases_json: &str,
    tags_json: Option<&str>,
) -> DomainResult<Vec<ToolReleaseOption>> {
    let entries: Vec<Value> = serde_json::from_str(releases_json).map_err(|error| {
        DomainError::external_process(format!("解析 UxPlay 版本清單失敗：{error}"))
    })?;
    let tag_commits = tags_json.map(parse_github_tag_commits).unwrap_or_default();

    let mut options = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        if entry["draft"].as_bool().unwrap_or(false) {
            continue;
        }
        let Some(tag_name) = entry["tag_name"].as_str() else {
            continue;
        };
        let Ok((version, fallback_ref)) = validate_uxplay_ref(tag_name) else {
            continue;
        };
        if !seen.insert(version.clone()) {
            continue;
        }
        let commit_sha = tag_commits
            .get(tag_name)
            .or_else(|| tag_commits.get(&fallback_ref))
            .cloned();
        let reference = commit_sha.clone().unwrap_or(fallback_ref);
        let label = match commit_sha.as_deref() {
            Some(sha) if sha.len() >= 7 => format!("v{version} ({})", &sha[..7]),
            _ => format!("v{version}"),
        };
        options.push(ToolReleaseOption {
            version,
            reference,
            label,
            published_at: entry["published_at"].as_str().and_then(extract_iso_date),
            prerelease: entry["prerelease"].as_bool().unwrap_or(false),
            recommended: false,
            sha256: None,
        });
    }
    Ok(options)
}

/// 解析 GitHub `releases.atom` 訂閱源作為 GitHub REST API 遭遇速率限制時之備援版本來源。
pub fn parse_github_releases_atom(xml: &str, tool: ToolId) -> Vec<ToolReleaseOption> {
    let mut options = Vec::new();
    let mut seen = HashSet::new();
    for chunk in xml.split("<entry>").skip(1) {
        let entry = chunk.split("</entry>").next().unwrap_or(chunk);
        let Some(tag_marker_start) = entry.find("/releases/tag/") else {
            continue;
        };
        let after_tag = &entry[tag_marker_start + "/releases/tag/".len()..];
        let Some(tag_end) = after_tag.find(['"', '\'', '<', ' ', '&']) else {
            continue;
        };
        let raw_tag = &after_tag[..tag_end];
        let published_at = extract_xml_tag(entry, "updated").and_then(extract_iso_date);

        let parsed = match tool {
            ToolId::Scrcpy => validate_scrcpy_version(raw_tag)
                .ok()
                .map(|version| (version, raw_tag.to_owned())),
            ToolId::UxPlay => validate_uxplay_ref(raw_tag).ok(),
            ToolId::GStreamer => None,
        };
        let Some((version, reference)) = parsed else {
            continue;
        };
        if !seen.insert(version.clone()) {
            continue;
        }
        options.push(ToolReleaseOption {
            version: version.clone(),
            reference,
            label: format!("v{version}"),
            published_at,
            prerelease: false,
            recommended: false,
            sha256: None,
        });
    }
    options
}

/// 從 `scrcpy` 官方 `SHA256SUMS.txt` 內容擷取指定資產檔案之 64 碼 SHA-256 雜湊值。
pub fn parse_scrcpy_sha256sums(contents: &str, asset_name: &str) -> Option<String> {
    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        let cleaned_name = name.trim_start_matches('*').trim_start_matches("./");
        if cleaned_name == asset_name && is_hex_len(hash, 64) {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// 解析 Homebrew `formula/gstreamer.json` 取得目前最新穩定版本號。
pub fn parse_homebrew_gstreamer_version(json_text: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(json_text).ok()?;
    let stable = parsed["versions"]["stable"].as_str()?.trim();
    is_package_version(stable).then(|| stable.to_owned())
}

/// 解析 MSYS2 `packages.msys2.org` 網頁擷取 UCRT64 `gstreamer` 最新版本號。
pub fn parse_msys2_gstreamer_version(html: &str) -> Option<String> {
    let dt_index = html.find("Version:")?;
    let after_dt = &html[dt_index..];
    let dd_start = after_dt.find("<dd")?;
    let after_dd_tag = after_dt[dd_start..].find('>')?;
    let content = &after_dt[dd_start + after_dd_tag + 1..];
    let dd_end = content.find("</dd>")?;
    let version = content[..dd_end].trim();
    is_package_version(version).then(|| version.to_owned())
}

/// 解析 Debian 官方套件頁取得指定 GStreamer 工具套件的套件版本。
pub fn parse_debian_gstreamer_version(html: &str) -> Option<String> {
    parse_debian_family_gstreamer_version(html)
}

/// 解析 Ubuntu 官方套件頁取得指定 GStreamer 工具套件的套件版本。
pub fn parse_ubuntu_gstreamer_version(html: &str) -> Option<String> {
    parse_debian_family_gstreamer_version(html)
}

fn parse_debian_family_gstreamer_version(html: &str) -> Option<String> {
    let marker = "Package: gstreamer1.0-tools (";
    let after_marker = html.get(html.find(marker)? + marker.len()..)?;
    let package_details = after_marker.split_once(')')?.0;
    let version = package_details.split_whitespace().next()?;
    is_package_version(version).then(|| version.to_owned())
}

/// 解析 Fedora 官方套件頁中指定 Fedora 發行版的 GStreamer 套件版本。
pub fn parse_fedora_gstreamer_version(html: &str, release: &str) -> Option<String> {
    if release.is_empty() || !release.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let marker = format!("Fedora {release}</td>");
    let marker_index = html.find(&marker)?;
    let row_start = html[..marker_index].rfind("<tr")?;
    let row_end = html[marker_index..].find("</tr>")? + marker_index;
    let row = &html[row_start..row_end];
    let anchor_start = row.find("<a")?;
    let content_start = row[anchor_start..].find('>')? + anchor_start + 1;
    let content_end = row[content_start..].find("</a>")? + content_start;
    let version = row[content_start..content_end].trim();
    is_package_version(version).then(|| version.to_owned())
}

/// 解析 Arch Linux 官方套件頁取得 x86_64 GStreamer 套件版本。
pub fn parse_arch_gstreamer_version(html: &str) -> Option<String> {
    let marker = "<h2>gstreamer ";
    let after_marker = html.get(html.find(marker)? + marker.len()..)?;
    let version = after_marker.split(['<', ' ', '\n', '\r', '\t']).next()?;
    is_package_version(version).then(|| version.to_owned())
}

/// 解析 DNF `repoquery` 對 GStreamer 套件輸出的單一版本行。
pub fn parse_dnf_gstreamer_version(output: &str) -> Option<String> {
    let mut version = None;
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !is_package_version(line) || version.as_deref().is_some_and(|found| found != line) {
            return None;
        }
        version = Some(line.to_owned());
    }
    version
}

fn is_package_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && version.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'-' | b'+' | b'~' | b':' | b'_' | b'^')
        })
}

/// 解析本機 `uxplay -v` 指令輸出中的版本號。
pub fn parse_installed_uxplay_version(output: &str) -> Option<String> {
    let lower = output.to_ascii_lowercase();
    let marker = "uxplay version";
    let start = lower.find(marker)?;
    let rest = output[start + marker.len()..].trim_start();
    let token: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    let trimmed = token.trim_matches('.');
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// 解析本機 `scrcpy --version` 指令輸出中的版本號。
pub fn parse_installed_scrcpy_version(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("scrcpy ") {
            let token = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches(['v', 'V']);
            if is_dotted_numeric_version(token, 2, 4) {
                return Some(token.to_owned());
            }
        }
    }
    None
}

/// 解析本機 `adb version` 指令輸出中的版本號。
pub fn parse_installed_adb_version(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Android Debug Bridge version ") {
            let token = rest.split_whitespace().next().unwrap_or("");
            if !token.is_empty() && token.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
                return Some(token.to_owned());
            }
        }
    }
    None
}

/// 解析本機 `gst-inspect-1.0 --version` 指令輸出中的版本號。
pub fn parse_installed_gstreamer_version(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("GStreamer ") {
            let token = rest.split_whitespace().next().unwrap_or("");
            if is_dotted_numeric_version(token, 2, 4) {
                return Some(token.to_owned());
            }
        }
    }
    None
}

/// 依作業系統、架構與套件管理器組裝前端呈現用之完整工具目錄快照。
pub fn build_tool_catalog_snapshot(context: &CatalogContext<'_>) -> ToolCatalogSnapshot {
    let (platform_label, package_manager_label, platform_notice, platform_warning) =
        describe_platform(context.os, context.arch, context.package_manager);

    let uxplay = build_uxplay_info(context);
    let scrcpy = build_scrcpy_info(context);
    let gstreamer = build_gstreamer_info(context);

    ToolCatalogSnapshot {
        os: context.os,
        arch: context.arch.to_owned(),
        platform_label,
        package_manager: package_manager_label,
        platform_notice,
        platform_warning,
        management_enabled: context.management_enabled,
        remote_fetched_at: context.remote.fetched_at.clone(),
        remote_error: context.remote.error.clone(),
        tools: vec![uxplay, scrcpy, gstreamer],
    }
}

/// 組合單一平台的標題、套件管理方式與操作／系統級警告說明。
fn describe_platform(
    os: PlatformOs,
    arch: &str,
    package_manager: &str,
) -> (String, String, String, Option<String>) {
    match os {
        PlatformOs::Macos => (
            format!("macOS ({arch})"),
            "Homebrew + GitHub 官方來源".into(),
            "macOS 會透過 Homebrew 維護 GStreamer 與編譯相依，並開啟 Terminal 視窗完成安裝授權；UxPlay 與 scrcpy 安裝至 App 專用工具目錄。"
                .into(),
            None,
        ),
        PlatformOs::Windows => (
            format!("Windows ({arch})"),
            "WinGet + MSYS2 UCRT64".into(),
            "Windows 僅在補裝系統前置項目時透過 UAC 要求管理員授權；MSYS2、UxPlay 與 scrcpy 皆以一般使用者權限安裝至 App 工具目錄。"
                .into(),
            None,
        ),
        PlatformOs::Linux => {
            let manager_display = match package_manager {
                "apt" => "APT（Debian / Ubuntu）",
                "dnf" => "DNF（Fedora / RHEL）",
                "pacman" => "Pacman（Arch Linux）",
                _ => "Linux 系統套件管理器",
            };
            let warning = (package_manager == "pacman").then(|| {
                "偵測到 Arch Linux pacman：執行「升級工具」或「清除並重新安裝」可能會同時執行 pacman -Syu 更新系統套件。"
                    .to_owned()
            });
            (
                format!("Linux ({arch})"),
                manager_display.into(),
                format!(
                    "Linux 偵測到 {manager_display}，系統相依與 Avahi 服務將透過 pkexec 請求授權維護。"
                ),
                warning,
            )
        }
    }
}

/// 建立 UxPlay 工具資訊卡片資料。
fn build_uxplay_info(context: &CatalogContext<'_>) -> ManagedToolInfo {
    let (default_version, default_commit, default_sha) = default_uxplay_spec(context.os);
    let recommended = ToolReleaseOption {
        version: default_version.to_owned(),
        reference: default_commit.to_owned(),
        label: match context.os {
            PlatformOs::Windows => {
                format!(
                    "{default_version}（Windows 推薦鎖定提交 {}）",
                    &default_commit[..7]
                )
            }
            PlatformOs::Macos | PlatformOs::Linux => {
                format!("v{default_version}（內建推薦驗證版）")
            }
        },
        published_at: None,
        prerelease: false,
        recommended: true,
        sha256: Some(default_sha.to_owned()),
    };

    let available_versions = merge_release_options(recommended, &context.remote.uxplay);
    let latest_remote_version = context
        .remote
        .uxplay
        .first()
        .map(|option| option.version.clone());

    ManagedToolInfo {
        id: ToolId::UxPlay,
        name: "UxPlay".into(),
        summary: "AirPlay 鏡像接收引擎（從 GitHub 官方原始碼建置並安裝至專用工具目錄）。".into(),
        source_label: match context.os {
            PlatformOs::Windows => "GitHub 原始碼（MSYS2 UCRT64 編譯）".into(),
            PlatformOs::Macos => "GitHub 原始碼（Apple Clang / CMake 編譯）".into(),
            PlatformOs::Linux => "GitHub 原始碼（系統 CMake 編譯）".into(),
        },
        source_url: "https://github.com/FDH2/UxPlay".into(),
        version_control: VersionControlMode::Selectable,
        installed_version: context.installed.uxplay.clone(),
        bundled_components: Vec::new(),
        default_version: default_version.to_owned(),
        default_reference: default_commit.to_owned(),
        selected_version: context
            .preferences
            .uxplay
            .as_ref()
            .map(|pinned| pinned.version.clone()),
        latest_remote_version,
        available_versions,
        supports_manual_sha256: true,
        auto_verifies_checksum: false,
        note: Some(match context.os {
            PlatformOs::Windows => {
                "Windows 預設鎖定支援最新 MSVC/UCRT64 修正之推薦提交；自訂版可選填 SHA-256，留空將於確認後透過 HTTPS 官方來源下載編譯。".into()
            }
            PlatformOs::Macos | PlatformOs::Linux => {
                "內建推薦版含鎖定 SHA-256；自訂或爬取版本可選填 SHA-256，留空將於確認後以 HTTPS 官方來源下載編譯。".into()
            }
        }),
    }
}

/// 建立 scrcpy（內含 ADB）工具資訊卡片資料。
fn build_scrcpy_info(context: &CatalogContext<'_>) -> ManagedToolInfo {
    let supports_official_archive =
        scrcpy_asset_name(context.os, context.arch, DEFAULT_SCRCPY_VERSION).is_some();
    let bundled_components = context
        .installed
        .adb
        .as_ref()
        .map(|version| vec![format!("ADB {version}")])
        .unwrap_or_else(|| vec!["隨附 ADB（隨 scrcpy 同包部署）".into()]);

    if !supports_official_archive {
        return ManagedToolInfo {
            id: ToolId::Scrcpy,
            name: "scrcpy（內含 ADB）".into(),
            summary: "Android USB 畫面投影與 ADB 裝置管理引擎。".into(),
            source_label: format!("系統套件庫（{}）", context.package_manager),
            source_url: "https://github.com/Genymobile/scrcpy".into(),
            version_control: VersionControlMode::PackageManager,
            installed_version: context.installed.scrcpy.clone(),
            bundled_components,
            default_version: DEFAULT_SCRCPY_VERSION.to_owned(),
            default_reference: format!("v{DEFAULT_SCRCPY_VERSION}"),
            selected_version: None,
            latest_remote_version: None,
            available_versions: Vec::new(),
            supports_manual_sha256: false,
            auto_verifies_checksum: false,
            note: Some(format!(
                "官方 scrcpy 靜態發行壓縮包僅支援 Linux x86_64；目前架構（{}）請透過系統套件管理器安裝或更新。",
                context.arch
            )),
        };
    }

    let recommended = ToolReleaseOption {
        version: DEFAULT_SCRCPY_VERSION.to_owned(),
        reference: format!("v{DEFAULT_SCRCPY_VERSION}"),
        label: format!("v{DEFAULT_SCRCPY_VERSION}（內建推薦驗證版）"),
        published_at: None,
        prerelease: false,
        recommended: true,
        sha256: default_scrcpy_sha256(context.os, context.arch).map(str::to_owned),
    };
    let available_versions = merge_release_options(recommended, &context.remote.scrcpy);
    let latest_remote_version = context
        .remote
        .scrcpy
        .first()
        .map(|option| option.version.clone());

    let arch_label = match (context.os, normalize_arch(context.arch)) {
        (PlatformOs::Macos, "arm64") => "macOS Apple Silicon 官方壓縮包",
        (PlatformOs::Macos, _) => "macOS Intel 官方壓縮包",
        (PlatformOs::Windows, "arm64") => "Windows ARM64 官方壓縮包",
        (PlatformOs::Windows, _) => "Windows x64 官方壓縮包",
        (PlatformOs::Linux, _) => "Linux x86_64 官方靜態壓縮包",
    };

    ManagedToolInfo {
        id: ToolId::Scrcpy,
        name: "scrcpy（內含 ADB）".into(),
        summary: "Android USB 畫面投影、ADB 偵測與壓縮包同步部署。".into(),
        source_label: format!("GitHub 官方發行（{arch_label}）"),
        source_url: "https://github.com/Genymobile/scrcpy/releases".into(),
        version_control: VersionControlMode::Selectable,
        installed_version: context.installed.scrcpy.clone(),
        bundled_components,
        default_version: DEFAULT_SCRCPY_VERSION.to_owned(),
        default_reference: format!("v{DEFAULT_SCRCPY_VERSION}"),
        selected_version: context
            .preferences
            .scrcpy
            .as_ref()
            .map(|pinned| pinned.version.clone()),
        latest_remote_version,
        available_versions,
        supports_manual_sha256: true,
        auto_verifies_checksum: true,
        note: Some(
            "切換版本時會自動從該發行版官方 SHA256SUMS.txt 驗證壓縮檔雜湊，並同步更新內附 ADB。"
                .into(),
        ),
    }
}

/// 建立 GStreamer（依作業系統套件管理器維護）工具資訊卡片資料。
fn build_gstreamer_info(context: &CatalogContext<'_>) -> ManagedToolInfo {
    let (source_label, source_url, note) = match context.os {
        PlatformOs::Macos => (
            "Homebrew Formulae（gstreamer）".into(),
            "https://formulae.brew.sh/formula/gstreamer".into(),
            "GStreamer 與其外掛由 Homebrew 統一維護並同步複製至 App 專用目錄，請透過「升級工具」更新。".into(),
        ),
        PlatformOs::Windows => (
            "MSYS2 UCRT64（mingw-w64-ucrt-x86_64-gstreamer）".into(),
            MSYS2_GSTREAMER_PAGE_URL.into(),
            "GStreamer 與編譯工具由 MSYS2 UCRT64 pacman 套件庫維護，請透過「升級工具」更新。".into(),
        ),
        PlatformOs::Linux => (
            format!("Linux 系統套件庫（{}）", context.package_manager),
            "https://gstreamer.freedesktop.org/".into(),
            "Linux 環境之 GStreamer 與 Avahi 服務由系統套件庫管理，請透過「升級工具」更新。".into(),
        ),
    };

    ManagedToolInfo {
        id: ToolId::GStreamer,
        name: "GStreamer 媒體管線".into(),
        summary: "UxPlay AirPlay 視訊／音訊解碼與渲染核心函式庫。".into(),
        source_label,
        source_url,
        version_control: VersionControlMode::PackageManager,
        installed_version: context.installed.gstreamer.clone(),
        bundled_components: Vec::new(),
        default_version: context
            .remote
            .gstreamer_latest
            .clone()
            .unwrap_or_else(|| "套件庫最新穩定版".into()),
        default_reference: "package-manager".into(),
        selected_version: None,
        latest_remote_version: context.remote.gstreamer_latest.clone(),
        available_versions: Vec::new(),
        supports_manual_sha256: false,
        auto_verifies_checksum: false,
        note: Some(note),
    }
}

/// 將推薦預設版置頂，並依序併入未重複的線上爬取版本清單。
fn merge_release_options(
    recommended: ToolReleaseOption,
    remote: &[ToolReleaseOption],
) -> Vec<ToolReleaseOption> {
    let mut merged = Vec::with_capacity(remote.len() + 1);
    let mut seen = HashSet::new();
    seen.insert(recommended.version.clone());
    merged.push(recommended);

    for option in remote {
        if seen.insert(option.version.clone()) {
            merged.push(option.clone());
        }
    }
    merged
}

/// 從 GitHub `/tags` JSON 建立 `tag_name -> commit_sha` 對照表。
fn parse_github_tag_commits(tags_json: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(entries) = serde_json::from_str::<Vec<Value>>(tags_json) else {
        return map;
    };
    for entry in entries {
        if let (Some(name), Some(sha)) = (entry["name"].as_str(), entry["commit"]["sha"].as_str()) {
            if is_hex_len(sha, 40) {
                map.insert(name.to_owned(), sha.to_ascii_lowercase());
            }
        }
    }
    map
}

/// 從 XML 片段提取 `<tag>...</tag>` 文字內容。
fn extract_xml_tag<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    Some(xml[start..start + end].trim())
}

/// 從 ISO 8601 / RFC 3339 時間戳記擷取前 10 碼日期 `YYYY-MM-DD`。
fn extract_iso_date(raw: &str) -> Option<String> {
    let date = raw.trim().get(..10)?;
    let bytes = date.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| idx == 4 || idx == 7 || byte.is_ascii_digit())
    {
        Some(date.to_owned())
    } else {
        None
    }
}

/// 檢查字串是否為 `min_parts..=max_parts` 段的純數字點分隔版本號（例：`5.0.1`）。
fn is_dotted_numeric_version(value: &str, min_parts: usize, max_parts: usize) -> bool {
    if value.is_empty() || value.len() > 32 {
        return false;
    }
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < min_parts || parts.len() > max_parts {
        return false;
    }
    parts
        .iter()
        .all(|part| !part.is_empty() && part.len() <= 8 && part.bytes().all(|b| b.is_ascii_digit()))
}

/// 檢查字串是否為精確指定長度的 ASCII 十六進位字串。
fn is_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
