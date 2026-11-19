use crate::application::{DependencyRuntime, EngineAccessGuard, ToolCatalogFetcher};
use crate::domain::{
    build_tool_catalog_snapshot, first_useful_line, interrupted_install_detail,
    parse_github_releases_atom, parse_github_scrcpy_releases_json,
    parse_github_uxplay_releases_json, parse_homebrew_gstreamer_version,
    parse_installed_adb_version, parse_installed_gstreamer_version, parse_installed_scrcpy_version,
    parse_installed_uxplay_version, parse_missing_dependencies, parse_msys2_gstreamer_version,
    validate_optional_sha256, validate_scrcpy_version, validate_tool_install_request,
    validate_uxplay_ref, CatalogContext, DependencyAction, DependencyOperation, DependencySnapshot,
    DomainError, DomainResult, InstallProgress, InstallResult, InstalledVersions, LaunchRecord,
    PinnedToolVersion, PlatformOs, RemoteCatalogCache, RuntimeCheck, SetupOutcome,
    ToolCatalogSnapshot, ToolId, ToolInstallRequest, ToolVersionPreferences, ValidatedToolInstall,
    HOMEBREW_GSTREAMER_API_URL, MSYS2_GSTREAMER_PAGE_URL, SCRCPY_RELEASES_API_URL,
    SCRCPY_RELEASES_ATOM_URL, UXPLAY_RELEASES_API_URL, UXPLAY_RELEASES_ATOM_URL,
    UXPLAY_TAGS_API_URL,
};
#[cfg(target_os = "windows")]
use crate::infrastructure::process::background_command;
#[cfg(target_os = "macos")]
use crate::infrastructure::process::macos_library_path;
use crate::infrastructure::process::{
    ensure_safe_directory, find_executable, read_bounded_regular_file, reject_symlink_path,
    resolved_path, run_with_timeout_env, run_with_timeout_in_path, write_file_atomic_secure,
};
use serde::Serialize;
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use std::ffi::OsStr;
#[cfg(target_os = "macos")]
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::process::{Child, Output};
#[cfg(not(target_os = "macos"))]
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CHECK_TIMEOUT: Duration = Duration::from_secs(90);
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(target_os = "macos")]
const GSTREAMER_REGISTRY_WARMUP_TIMEOUT: Duration = Duration::from_secs(100);
const PROCESS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const RESULT_GRACE: Duration = Duration::from_secs(5);
const LAUNCH_GRACE: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_INSTALL_LOG_READ_BYTES: usize = 1024 * 1024;
const MAX_INSTALL_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_INSTALL_METADATA_BYTES: u64 = 64 * 1024;

const INSTALL_MARKER: &str = "install.pid";
const INSTALL_RESULT: &str = "install-result.json";
const INSTALL_LAUNCH: &str = "install-launch.json";
const INSTALL_LOG: &str = "install.log";
const INSTALL_PROGRESS: &str = "install-progress.json";
const TOOL_VERSIONS_FILE: &str = "tool-versions.json";

/// 傳遞給安裝腳本執行的目標範圍與自訂版本參數規格。
#[derive(Clone, Debug, Default)]
pub(crate) struct InstallInvocationOptions {
    pub only_tool: Option<ToolId>,
    pub uxplay: Option<PinnedToolVersion>,
    pub scrcpy: Option<PinnedToolVersion>,
    pub validated_install: Option<ValidatedToolInstall>,
}

impl InstallInvocationOptions {
    fn from_preferences(preferences: ToolVersionPreferences) -> Self {
        Self {
            only_tool: None,
            uxplay: preferences.uxplay,
            scrcpy: preferences.scrcpy,
            validated_install: None,
        }
    }

    fn from_validated(validated: ValidatedToolInstall) -> Self {
        let pinned = (!validated.is_default).then(|| PinnedToolVersion {
            version: validated.version.clone(),
            reference: validated.reference.clone(),
            sha256: validated.sha256.clone(),
            allow_unverified: validated.allow_unverified,
        });
        match validated.tool {
            ToolId::UxPlay => Self {
                only_tool: Some(ToolId::UxPlay),
                uxplay: pinned,
                scrcpy: None,
                validated_install: Some(validated),
            },
            ToolId::Scrcpy => Self {
                only_tool: Some(ToolId::Scrcpy),
                uxplay: None,
                scrcpy: pinned,
                validated_install: Some(validated),
            },
            ToolId::GStreamer => Self::default(),
        }
    }

    fn expected_uxplay_version(&self) -> Option<&str> {
        self.uxplay.as_ref().map(|pinned| pinned.version.as_str())
    }
}

pub struct DependencyManager {
    snapshot: Arc<Mutex<DependencySnapshot>>,
    pub(crate) worker_active: Arc<AtomicBool>,
    operation_gate: Arc<Mutex<()>>,
    engine_dir: PathBuf,
    gstreamer_registry_path: PathBuf,
    setup_dir: PathBuf,
    config_dir: PathBuf,
    remote_catalog: Arc<Mutex<RemoteCatalogCache>>,
    enabled: bool,
}

impl DependencyManager {
    pub fn new(
        engine_dir: PathBuf,
        gstreamer_registry_path: PathBuf,
        config_dir: &Path,
        enabled: bool,
    ) -> Self {
        let setup_dir = config_dir.join("setup");
        Self {
            snapshot: Arc::new(Mutex::new(DependencySnapshot {
                status: if enabled { "checking" } else { "disabled" }.into(),
                operation: None,
                message: if enabled {
                    "正在檢查投影工具…"
                } else {
                    "此工作階段已停用自動安裝。"
                }
                .into(),
                missing: Vec::new(),
                log_path: setup_dir.join(INSTALL_LOG).to_string_lossy().into_owned(),
            })),
            worker_active: Arc::new(AtomicBool::new(false)),
            operation_gate: Arc::new(Mutex::new(())),
            engine_dir,
            gstreamer_registry_path,
            setup_dir,
            config_dir: config_dir.to_path_buf(),
            remote_catalog: Arc::new(Mutex::new(RemoteCatalogCache::default())),
            enabled,
        }
    }

    pub fn snapshot(&self) -> Result<DependencySnapshot, String> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| "無法讀取工具安裝狀態。".into())
    }

    pub fn read_install_log(&self) -> Result<String, String> {
        read_log_tail(
            &self.setup_dir.join(INSTALL_LOG),
            MAX_INSTALL_LOG_READ_BYTES,
        )
    }

    pub fn start(&self) -> Result<DependencySnapshot, String> {
        let options = InstallInvocationOptions::from_preferences(load_tool_version_preferences(
            &self.config_dir,
        ));
        self.begin_operation(DependencyOperation::Install, true, options, &mut || Ok(()))
    }

    pub fn manage(
        &self,
        action: DependencyAction,
        prepare: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<DependencySnapshot, String> {
        let options = InstallInvocationOptions::from_preferences(load_tool_version_preferences(
            &self.config_dir,
        ));
        self.begin_operation(action.into(), false, options, prepare)
    }

    /// 取得目前作業系統平台之工具目錄、已安裝版本與快取線上版本。
    pub fn tool_catalog(&self) -> DomainResult<ToolCatalogSnapshot> {
        let os = detect_platform_os();
        let arch = detect_platform_arch();
        let package_manager = detect_package_manager();
        let installed = if self.enabled {
            probe_installed_versions(&self.engine_dir)
        } else {
            InstalledVersions::default()
        };
        let preferences = load_tool_version_preferences(&self.config_dir);
        let remote = self
            .remote_catalog
            .lock()
            .map(|cache| cache.clone())
            .unwrap_or_default();
        Ok(build_tool_catalog_snapshot(&CatalogContext {
            os,
            arch,
            package_manager,
            management_enabled: self.enabled,
            installed: &installed,
            preferences: &preferences,
            remote: &remote,
        }))
    }

    /// 從白名單官方來源爬取工具版本清單，更新記憶體快取後回傳最新快照。
    pub fn refresh_tool_catalog(
        &self,
        fetcher: &dyn ToolCatalogFetcher,
    ) -> DomainResult<ToolCatalogSnapshot> {
        let os = detect_platform_os();
        let arch = detect_platform_arch();
        let mut next = RemoteCatalogCache {
            fetched_at: Some(jiff::Timestamp::now().to_string()),
            ..RemoteCatalogCache::default()
        };
        let mut errors = Vec::new();

        match fetch_scrcpy_releases(fetcher, os, arch) {
            Ok(versions) => next.scrcpy = versions,
            Err(error) => errors.push(format!("scrcpy：{}", error.message)),
        }
        match fetch_uxplay_releases(fetcher) {
            Ok(versions) => next.uxplay = versions,
            Err(error) => errors.push(format!("UxPlay：{}", error.message)),
        }
        match fetch_gstreamer_latest(fetcher, os) {
            Ok(latest) => next.gstreamer_latest = latest,
            Err(error) => errors.push(format!("GStreamer：{}", error.message)),
        }

        if !errors.is_empty() {
            next.error = Some(errors.join("；"));
        }

        if let Ok(mut guard) = self.remote_catalog.lock() {
            if next.scrcpy.is_empty() && !guard.scrcpy.is_empty() {
                next.scrcpy = guard.scrcpy.clone();
            }
            if next.uxplay.is_empty() && !guard.uxplay.is_empty() {
                next.uxplay = guard.uxplay.clone();
            }
            if next.gstreamer_latest.is_none() {
                next.gstreamer_latest = guard.gstreamer_latest.clone();
            }
            *guard = next;
        }

        self.tool_catalog()
    }

    /// 驗證並啟動指定單一工具與版本之背景安裝／切換流程。
    pub fn install_tool_version(
        &self,
        request: ToolInstallRequest,
        prepare: &mut dyn FnMut() -> DomainResult<()>,
    ) -> DomainResult<DependencySnapshot> {
        if !self.enabled {
            return Err(DomainError::dependency_disabled(
                "此工作階段已停用工具管理。",
            ));
        }

        let known_remote = self
            .remote_catalog
            .lock()
            .map(|cache| match request.tool {
                ToolId::UxPlay => cache.uxplay.clone(),
                ToolId::Scrcpy => cache.scrcpy.clone(),
                ToolId::GStreamer => Vec::new(),
            })
            .unwrap_or_default();

        let validated = validate_tool_install_request(
            &request,
            detect_platform_os(),
            detect_platform_arch(),
            &known_remote,
        )?;

        let options = InstallInvocationOptions::from_validated(validated);
        self.begin_operation(
            DependencyOperation::InstallTool,
            false,
            options,
            &mut || prepare().map_err(|error| error.message),
        )
        .map_err(DomainError::dependency_busy)
    }

    /// 清除所有自訂工具版本偏好並回傳最新目錄快照。
    pub fn reset_tool_versions(&self) -> DomainResult<ToolCatalogSnapshot> {
        reset_tool_version_preferences(&self.config_dir).map_err(DomainError::storage)?;
        self.tool_catalog()
    }

    pub fn with_engine_access<T>(
        &self,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _operation = self
            .operation_gate
            .lock()
            .map_err(|_| "工具維護狀態無法讀取。".to_owned())?;
        if self.worker_active.load(Ordering::Acquire) {
            return Err("工具正在檢查或維護，請稍候再試。".into());
        }
        action()
    }

    fn begin_operation(
        &self,
        operation: DependencyOperation,
        return_existing: bool,
        options: InstallInvocationOptions,
        prepare: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<DependencySnapshot, String> {
        if !self.enabled {
            if return_existing {
                return self.snapshot();
            }
            return Err("此工作階段已停用工具管理。".into());
        }

        {
            let _operation = self
                .operation_gate
                .lock()
                .map_err(|_| "工具維護狀態無法更新。".to_owned())?;
            if self.worker_active.swap(true, Ordering::AcqRel) {
                if return_existing {
                    return self.snapshot();
                }
                return Err("另一項工具檢查或維護作業仍在進行。".into());
            }
            let mut snapshot = match self.snapshot.lock() {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    self.worker_active.store(false, Ordering::Release);
                    return Err("無法更新工具安裝狀態。".into());
                }
            };
            snapshot.status = if operation == DependencyOperation::Install {
                "checking".into()
            } else {
                "installing".into()
            };
            snapshot.operation = Some(operation);
            snapshot.message = match operation {
                DependencyOperation::Install => "正在檢查 UxPlay、GStreamer、ADB 與 scrcpy…".into(),
                DependencyOperation::Upgrade => "正在準備升級投影工具…".into(),
                DependencyOperation::Reinstall => "正在準備清除並重新安裝…".into(),
                DependencyOperation::InstallTool => match options.validated_install.as_ref() {
                    Some(validated) => format!(
                        "正在準備安裝 {} {}…",
                        match validated.tool {
                            ToolId::UxPlay => "UxPlay",
                            ToolId::Scrcpy => "scrcpy",
                            ToolId::GStreamer => "GStreamer",
                        },
                        validated.version
                    ),
                    None => "正在準備安裝指定版本工具…".into(),
                },
            };
        }

        if let Err(error) = prepare() {
            self.fail_operation(&error);
            return Err(error);
        }

        let shared = Arc::clone(&self.snapshot);
        let worker_active = Arc::clone(&self.worker_active);
        let operation_gate = Arc::clone(&self.operation_gate);
        let engine_dir = self.engine_dir.clone();
        let gstreamer_registry_path = self.gstreamer_registry_path.clone();
        let setup_dir = self.setup_dir.clone();
        let config_dir = self.config_dir.clone();
        let spawned = thread::Builder::new()
            .name("neo-airplay-dependency-setup".into())
            .spawn(move || {
                let run = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    run_setup(
                        &engine_dir,
                        &gstreamer_registry_path,
                        &setup_dir,
                        &config_dir,
                        &shared,
                        operation,
                        &options,
                    )
                }));
                let outcome = match run {
                    Ok(Ok(outcome)) => outcome,
                    Ok(Err(error)) => {
                        append_log(&setup_dir.join(INSTALL_LOG), &error);
                        SetupOutcome::failed(error, Vec::new())
                    }
                    Err(_) => {
                        let message = "工具維護背景程序意外結束。".to_owned();
                        append_log(&setup_dir.join(INSTALL_LOG), &message);
                        SetupOutcome::failed(message, Vec::new())
                    }
                };
                if let Ok(_operation) = operation_gate.lock() {
                    if let Ok(mut snapshot) = shared.lock() {
                        snapshot.status = outcome.status;
                        snapshot.message = outcome.message;
                        snapshot.missing = outcome.missing;
                    }
                    worker_active.store(false, Ordering::Release);
                } else {
                    worker_active.store(false, Ordering::Release);
                }
            });

        if let Err(error) = spawned {
            self.fail_operation(&format!("無法啟動背景工具維護：{error}"));
            return Err(error.to_string());
        }

        self.snapshot()
    }

    /// 當前置步驟或執行緒生成失敗時，寫入失敗日誌、將快照標記為 `failed` 並釋放 `worker_active`。
    ///
    /// # 參數
    /// - `message`: 失敗原因訊息。
    fn fail_operation(&self, message: &str) {
        append_log(&self.setup_dir.join(INSTALL_LOG), message);
        if let Ok(_operation) = self.operation_gate.lock() {
            if let Ok(mut snapshot) = self.snapshot.lock() {
                snapshot.status = "failed".into();
                snapshot.message = message.to_owned();
                snapshot.missing.clear();
            }
            self.worker_active.store(false, Ordering::Release);
        } else {
            self.worker_active.store(false, Ordering::Release);
        }
    }
}

impl EngineAccessGuard for DependencyManager {
    /// 實作應用層 `EngineAccessGuard` 埠，阻擋維護期間同時操作投影引擎。
    ///
    /// # 參數
    /// - `action`: 無衝突時執行的可變閉包參考。
    ///
    /// # 回傳值
    /// 執行成功回傳 `Ok(())`；工具忙碌中時回傳 `DomainError::dependency_busy`。
    fn execute_if_available(&self, action: &mut dyn FnMut()) -> DomainResult<()> {
        let _operation = self
            .operation_gate
            .lock()
            .map_err(|_| DomainError::conflict("工具維護狀態無法讀取。"))?;
        if self.worker_active.load(Ordering::Acquire) {
            return Err(DomainError::dependency_busy(
                "工具正在檢查或維護，請稍候再試。",
            ));
        }
        action();
        Ok(())
    }
}

impl DependencyRuntime for DependencyManager {
    /// 透過 `DependencyRuntime` 埠查詢目前快照。
    ///
    /// # 回傳值
    /// 回傳 `DependencySnapshot` 或映射後的 `DomainError::storage`。
    fn snapshot(&self) -> DomainResult<DependencySnapshot> {
        DependencyManager::snapshot(self).map_err(DomainError::storage)
    }

    /// 透過 `DependencyRuntime` 埠讀取安裝日誌。
    ///
    /// # 回傳值
    /// 回傳安裝日誌內容或 `DomainError::not_found`。
    fn read_install_log(&self) -> DomainResult<String> {
        DependencyManager::read_install_log(self).map_err(DomainError::not_found)
    }

    /// 透過 `DependencyRuntime` 埠啟動自動檢查與安裝。
    ///
    /// # 回傳值
    /// 回傳更新後的 `DependencySnapshot` 或 `DomainError::dependency_busy`。
    fn start_install(&self) -> DomainResult<DependencySnapshot> {
        DependencyManager::start(self).map_err(DomainError::dependency_busy)
    }

    /// 透過 `DependencyRuntime` 埠啟動手動維護作業。
    ///
    /// # 參數
    /// - `action`: 維護動作類型。
    /// - `prepare`: 前置停止與檢查閉包。
    ///
    /// # 回傳值
    /// 回傳更新後的 `DependencySnapshot` 或 `DomainError::dependency_busy`。
    fn manage(
        &self,
        action: DependencyAction,
        prepare: &mut dyn FnMut() -> DomainResult<()>,
    ) -> DomainResult<DependencySnapshot> {
        DependencyManager::manage(self, action, &mut || {
            prepare().map_err(|error| error.message)
        })
        .map_err(DomainError::dependency_busy)
    }

    fn tool_catalog(&self) -> DomainResult<ToolCatalogSnapshot> {
        DependencyManager::tool_catalog(self)
    }

    fn refresh_tool_catalog(
        &self,
        fetcher: &dyn ToolCatalogFetcher,
    ) -> DomainResult<ToolCatalogSnapshot> {
        DependencyManager::refresh_tool_catalog(self, fetcher)
    }

    fn install_tool_version(
        &self,
        request: ToolInstallRequest,
        prepare: &mut dyn FnMut() -> DomainResult<()>,
    ) -> DomainResult<DependencySnapshot> {
        DependencyManager::install_tool_version(self, request, prepare)
    }

    fn reset_tool_versions(&self) -> DomainResult<ToolCatalogSnapshot> {
        DependencyManager::reset_tool_versions(self)
    }
}

fn detect_platform_os() -> PlatformOs {
    #[cfg(target_os = "macos")]
    {
        PlatformOs::Macos
    }
    #[cfg(target_os = "windows")]
    {
        PlatformOs::Windows
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        PlatformOs::Linux
    }
}

fn detect_platform_arch() -> &'static str {
    std::env::consts::ARCH
}

fn detect_package_manager() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "homebrew"
    }
    #[cfg(target_os = "windows")]
    {
        "winget-msys2"
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        if has_command_on_path("apt-get") {
            "apt"
        } else if has_command_on_path("dnf") {
            "dnf"
        } else if has_command_on_path("pacman") {
            "pacman"
        } else {
            "linux-pkg"
        }
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn has_command_on_path(binary: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|directory| {
        let candidate = directory.join(binary);
        fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file())
    })
}

fn probe_installed_versions(engine_dir: &Path) -> InstalledVersions {
    let engine_path = resolved_path(engine_dir);
    #[cfg(target_os = "macos")]
    let dyld_path = macos_library_path(engine_dir);
    #[cfg(target_os = "macos")]
    let gst_plugins: OsString = engine_dir.join("lib/gstreamer-1.0").into_os_string();
    #[cfg(target_os = "macos")]
    let mac_env: [(&str, &OsStr); 3] = [
        ("DYLD_LIBRARY_PATH", dyld_path.as_os_str()),
        ("GST_PLUGIN_PATH_1_0", gst_plugins.as_os_str()),
        ("GST_PLUGIN_SYSTEM_PATH_1_0", gst_plugins.as_os_str()),
    ];

    let run_probe = |executable: &Path, args: &[&str]| -> Option<String> {
        let owned_args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        #[cfg(target_os = "macos")]
        let output = run_with_timeout_env(
            executable,
            &owned_args,
            VERSION_PROBE_TIMEOUT,
            Some(engine_path.as_os_str()),
            &mac_env,
        )
        .ok()?;
        #[cfg(not(target_os = "macos"))]
        let output = run_with_timeout_env(
            executable,
            &owned_args,
            VERSION_PROBE_TIMEOUT,
            Some(engine_path.as_os_str()),
            &[],
        )
        .ok()?;
        Some(combined_output(&output))
    };

    InstalledVersions {
        uxplay: find_executable(engine_dir, "uxplay", false)
            .and_then(|exe| run_probe(&exe, &["-v"]))
            .and_then(|out| parse_installed_uxplay_version(&out)),
        scrcpy: find_executable(engine_dir, "scrcpy", true)
            .and_then(|exe| run_probe(&exe, &["--version"]))
            .and_then(|out| parse_installed_scrcpy_version(&out)),
        adb: find_executable(engine_dir, "adb", true)
            .and_then(|exe| run_probe(&exe, &["version"]))
            .and_then(|out| parse_installed_adb_version(&out)),
        gstreamer: find_executable(engine_dir, "gst-inspect-1.0", false)
            .and_then(|exe| run_probe(&exe, &["--version"]))
            .and_then(|out| parse_installed_gstreamer_version(&out)),
    }
}

fn fetch_scrcpy_releases(
    fetcher: &dyn ToolCatalogFetcher,
    os: PlatformOs,
    arch: &str,
) -> DomainResult<Vec<crate::domain::ToolReleaseOption>> {
    match fetcher.fetch_text(SCRCPY_RELEASES_API_URL) {
        Ok(body) => parse_github_scrcpy_releases_json(&body, os, arch),
        Err(api_error) => {
            let atom_body = fetcher
                .fetch_text(SCRCPY_RELEASES_ATOM_URL)
                .map_err(|_| api_error)?;
            let fallback = parse_github_releases_atom(&atom_body, ToolId::Scrcpy);
            if fallback.is_empty() {
                Err(DomainError::external_process(
                    "無法解析 scrcpy 線上發行清單。",
                ))
            } else {
                Ok(fallback)
            }
        }
    }
}

fn fetch_uxplay_releases(
    fetcher: &dyn ToolCatalogFetcher,
) -> DomainResult<Vec<crate::domain::ToolReleaseOption>> {
    match fetcher.fetch_text(UXPLAY_RELEASES_API_URL) {
        Ok(body) => {
            let tags_body = fetcher.fetch_text(UXPLAY_TAGS_API_URL).ok();
            parse_github_uxplay_releases_json(&body, tags_body.as_deref())
        }
        Err(api_error) => {
            let atom_body = fetcher
                .fetch_text(UXPLAY_RELEASES_ATOM_URL)
                .map_err(|_| api_error)?;
            let fallback = parse_github_releases_atom(&atom_body, ToolId::UxPlay);
            if fallback.is_empty() {
                Err(DomainError::external_process(
                    "無法解析 UxPlay 線上發行清單。",
                ))
            } else {
                Ok(fallback)
            }
        }
    }
}

fn fetch_gstreamer_latest(
    fetcher: &dyn ToolCatalogFetcher,
    os: PlatformOs,
) -> DomainResult<Option<String>> {
    match os {
        PlatformOs::Macos => {
            let body = fetcher.fetch_text(HOMEBREW_GSTREAMER_API_URL)?;
            Ok(parse_homebrew_gstreamer_version(&body))
        }
        PlatformOs::Windows => {
            let body = fetcher.fetch_text(MSYS2_GSTREAMER_PAGE_URL)?;
            Ok(parse_msys2_gstreamer_version(&body))
        }
        PlatformOs::Linux => Ok(None),
    }
}

/// 從 `config_dir/tool-versions.json` 安全讀取使用者自訂版本偏好，遇到符號連結或無效內容時安全退回預設。
pub(crate) fn load_tool_version_preferences(config_dir: &Path) -> ToolVersionPreferences {
    let path = config_dir.join(TOOL_VERSIONS_FILE);
    let Ok(bytes) = read_bounded_regular_file(&path, MAX_INSTALL_METADATA_BYTES) else {
        return ToolVersionPreferences::default();
    };
    let Ok(mut preferences) = serde_json::from_slice::<ToolVersionPreferences>(&bytes) else {
        return ToolVersionPreferences::default();
    };
    preferences.uxplay = preferences.uxplay.filter(|pin| {
        validate_uxplay_ref(&pin.reference).is_ok()
            && validate_uxplay_ref(&pin.version).is_ok()
            && validate_optional_sha256(pin.sha256.as_deref()).is_ok()
    });
    preferences.scrcpy = preferences.scrcpy.filter(|pin| {
        validate_scrcpy_version(&pin.version).is_ok()
            && validate_optional_sha256(pin.sha256.as_deref()).is_ok()
    });
    preferences
}

/// 將已驗證通過的工具版本選擇原子寫入 `config_dir/tool-versions.json`（若為推薦預設版則清除該工具覆寫）。
pub(crate) fn save_tool_version_preference(
    config_dir: &Path,
    validated: &ValidatedToolInstall,
) -> Result<(), String> {
    ensure_safe_directory(config_dir, false)
        .map_err(|error| format!("無法建立或驗證設定目錄：{error}"))?;
    let path = config_dir.join(TOOL_VERSIONS_FILE);
    reject_symlink_path(&path).map_err(|error| format!("工具版本設定路徑不安全：{error}"))?;

    let mut preferences = load_tool_version_preferences(config_dir);
    let next_pin = (!validated.is_default).then(|| PinnedToolVersion {
        version: validated.version.clone(),
        reference: validated.reference.clone(),
        sha256: validated.sha256.clone(),
        allow_unverified: validated.allow_unverified,
    });
    match validated.tool {
        ToolId::UxPlay => preferences.uxplay = next_pin,
        ToolId::Scrcpy => preferences.scrcpy = next_pin,
        ToolId::GStreamer => {}
    }

    if preferences.uxplay.is_none() && preferences.scrcpy.is_none() {
        remove_file_if_exists(&path)
    } else {
        write_json_atomic(&path, &preferences)
    }
}

/// 安全刪除 `config_dir/tool-versions.json` 以還原各作業系統內建推薦版。
pub(crate) fn reset_tool_version_preferences(config_dir: &Path) -> Result<(), String> {
    let path = config_dir.join(TOOL_VERSIONS_FILE);
    reject_symlink_path(&path).map_err(|error| format!("工具版本設定路徑不安全：{error}"))?;
    remove_file_if_exists(&path)
}

/// 在背景執行緒執行的完整檢查、清理、安裝與結果驗證工作流程。
///
/// # 參數
/// - `engine_dir`: 受控管投影工具目錄。
/// - `gstreamer_registry_path`: macOS GStreamer 快取路徑。
/// - `setup_dir`: 暫存腳本與安裝追蹤檔案目錄。
/// - `snapshot`: 用於即時發布狀態進度的共用快照鎖。
/// - `operation`: 本次執行的依賴作業類型。
///
/// # 回傳值
/// 回傳最終 `SetupOutcome` 或不可恢復的錯誤訊息字串。
fn run_setup(
    engine_dir: &Path,
    gstreamer_registry_path: &Path,
    setup_dir: &Path,
    config_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
    operation: DependencyOperation,
    options: &InstallInvocationOptions,
) -> Result<SetupOutcome, String> {
    ensure_safe_directory(setup_dir, true)
        .map_err(|error| format!("無法建立安裝記錄目錄：{error}"))?;
    ensure_safe_directory(engine_dir, false)
        .map_err(|error| format!("無法建立或驗證工具目錄：{error}"))?;

    let marker_path = setup_dir.join(INSTALL_MARKER);
    let result_path = setup_dir.join(INSTALL_RESULT);
    let launch_path = setup_dir.join(INSTALL_LAUNCH);
    let previous_attempt = read_launch_record(&launch_path)?;

    if let Some(pid) = read_install_marker(&marker_path)? {
        match process_presence(pid, engine_dir) {
            ProcessPresence::Alive | ProcessPresence::Unknown => {
                publish_installing(snapshot, "正在等待已執行的工具安裝程序完成。");
                let _ = wait_for_install(
                    engine_dir,
                    setup_dir,
                    Some(pid),
                    None,
                    previous_attempt.clone(),
                    snapshot,
                )?;
            }
            ProcessPresence::Dead if !result_path.exists() => {
                if let Some(record) = previous_attempt
                    .as_ref()
                    .filter(|record| launch_is_recent(record))
                {
                    publish_installing(snapshot, "正在確認工具安裝程序是否已結束。");
                    let _ = wait_for_install(
                        engine_dir,
                        setup_dir,
                        Some(pid),
                        None,
                        Some(record.clone()),
                        snapshot,
                    )?;
                } else {
                    thread::sleep(RESULT_GRACE);
                }
            }
            ProcessPresence::Dead => {}
        }
    } else if let Some(record) = previous_attempt.as_ref() {
        if let Some(pid) = record.worker_pid {
            match process_presence(pid, engine_dir) {
                ProcessPresence::Alive | ProcessPresence::Unknown => {
                    publish_installing(snapshot, "正在等待已執行的工具安裝程序完成。");
                    let _ = wait_for_install(
                        engine_dir,
                        setup_dir,
                        None,
                        None,
                        Some(record.clone()),
                        snapshot,
                    )?;
                }
                ProcessPresence::Dead if launch_is_recent(record) => {
                    publish_installing(snapshot, "正在確認工具安裝程序是否已結束。");
                    let _ = wait_for_install(
                        engine_dir,
                        setup_dir,
                        None,
                        None,
                        Some(record.clone()),
                        snapshot,
                    )?;
                }
                ProcessPresence::Dead => {}
            }
        } else if launch_is_recent(record) {
            publish_installing(snapshot, "正在確認工具安裝程序是否已啟動。");
            let _ = wait_for_install(
                engine_dir,
                setup_dir,
                None,
                None,
                Some(record.clone()),
                snapshot,
            )?;
        }
    }

    if operation == DependencyOperation::Reinstall {
        publish_installing(snapshot, "正在清除 Neo AirPlay 專用工具與快取…");
        clear_app_tools(engine_dir, gstreamer_registry_path)?;
    } else if operation == DependencyOperation::Upgrade {
        publish_installing(snapshot, "正在升級投影工具並更新 GStreamer 快取…");
        clear_gstreamer_registry(gstreamer_registry_path)?;
    }

    stage_installer(setup_dir)?;
    let expected_uxplay = options.expected_uxplay_version();
    let runtime = check_runtime(engine_dir, setup_dir, expected_uxplay)?;
    if runtime.is_ready() && operation == DependencyOperation::Install {
        stage_installer(setup_dir)?;
        warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
        remove_if_exists(&launch_path);
        return Ok(SetupOutcome::ready(operation));
    }

    remove_if_exists(&marker_path);
    remove_if_exists(&result_path);
    remove_if_exists(&launch_path);

    let message = match operation {
        DependencyOperation::Install => {
            let missing_message = format!("缺少或無法載入：{}", runtime.missing.join("、"));
            append_log(&setup_dir.join(INSTALL_LOG), &missing_message);
            format!("{missing_message} 正在背景安裝，請完成系統授權提示。")
        }
        DependencyOperation::Upgrade => "正在更新平台相依套件與 Neo AirPlay 工具…".into(),
        DependencyOperation::Reinstall => "正在更新平台相依套件並重新安裝工具…".into(),
        DependencyOperation::InstallTool => match options.validated_install.as_ref() {
            Some(validated) => format!(
                "正在背景下載並安裝 {} {}…",
                match validated.tool {
                    ToolId::UxPlay => "UxPlay",
                    ToolId::Scrcpy => "scrcpy",
                    ToolId::GStreamer => "GStreamer",
                },
                validated.version
            ),
            None => "正在安裝指定版本的投影工具…".into(),
        },
    };
    publish_installing(snapshot, &message);

    stage_installer(setup_dir)?;
    let result = install_dependencies(
        setup_dir,
        engine_dir,
        snapshot,
        matches!(
            operation,
            DependencyOperation::Upgrade | DependencyOperation::Reinstall
        ),
        options,
    );
    let final_check = check_runtime(engine_dir, setup_dir, expected_uxplay)?;
    let installer_succeeded = result.as_ref().is_ok_and(|result| result.exit_code == 0);
    if operation == DependencyOperation::InstallTool {
        if installer_succeeded && target_tool_present(engine_dir, options.only_tool) {
            if let Some(validated) = options.validated_install.as_ref() {
                save_tool_version_preference(config_dir, validated)?;
            }
            stage_installer(setup_dir)?;
            remove_if_exists(&launch_path);
            if final_check.is_ready() {
                warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
                return Ok(SetupOutcome::ready(operation));
            }
            let partial_message = format!(
                "指定版本工具已安裝，但目前環境仍缺少 {}。",
                final_check.missing.join("、")
            );
            append_log(&setup_dir.join(INSTALL_LOG), &partial_message);
            return Ok(SetupOutcome::failed(partial_message, final_check.missing));
        }
    } else if final_check.is_ready()
        && (operation == DependencyOperation::Install || installer_succeeded)
    {
        stage_installer(setup_dir)?;
        warm_gstreamer_registry(engine_dir, gstreamer_registry_path, setup_dir, snapshot);
        remove_if_exists(&launch_path);
        return Ok(SetupOutcome::ready(operation));
    }

    let install_detail = match result {
        Ok(result) => result.detail(),
        Err(error) => format!("安裝程序未完成：{error}"),
    };
    let missing_detail = if final_check.missing.is_empty() {
        "工具檢查仍通過，但維護作業未成功完成。".to_owned()
    } else {
        format!("仍缺少 {}。", final_check.missing.join("、"))
    };
    let message = format!("{install_detail}；{missing_detail}請查看安裝記錄後重試。");
    append_log(&setup_dir.join(INSTALL_LOG), &message);
    Ok(SetupOutcome::failed(message, final_check.missing))
}

fn target_tool_present(engine_dir: &Path, only_tool: Option<ToolId>) -> bool {
    match only_tool {
        Some(ToolId::UxPlay) => find_executable(engine_dir, "uxplay", false).is_some(),
        Some(ToolId::Scrcpy) => {
            find_executable(engine_dir, "scrcpy", true).is_some()
                && find_executable(engine_dir, "adb", true).is_some()
        }
        Some(ToolId::GStreamer) | None => false,
    }
}

/// 在 macOS 上於首次就緒或快取失效時預熱 GStreamer registry 快取，加速後續 UxPlay 啟動。
///
/// # 參數
/// - `engine_dir`: 受控管工具目錄。
/// - `registry_path`: 目標 GStreamer registry 二進位檔案路徑。
/// - `setup_dir`: 放置 `install.sh` 的 setup 目錄。
/// - `snapshot`: 用於更新目前介面安裝提示文字的快照鎖。
fn warm_gstreamer_registry(
    engine_dir: &Path,
    registry_path: &Path,
    setup_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
) {
    #[cfg(target_os = "macos")]
    {
        if fs::metadata(registry_path).is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }

        publish_installing(snapshot, "正在準備 GStreamer 快取…");
        let started_at = Instant::now();
        let script = setup_dir.join("install.sh");
        let args = vec![
            script.to_string_lossy().into_owned(),
            "--warm-gstreamer-registry".into(),
            "--engine-dir".into(),
            engine_dir.to_string_lossy().into_owned(),
            "--registry-path".into(),
            registry_path.to_string_lossy().into_owned(),
        ];
        let output = run_with_timeout_in_path(
            Path::new("/bin/bash"),
            &args,
            GSTREAMER_REGISTRY_WARMUP_TIMEOUT,
            None,
        );
        match output {
            Ok(output) => {
                let output_text = combined_output(&output);
                if !output_text.trim().is_empty() {
                    append_log(&setup_dir.join(INSTALL_LOG), output_text.trim());
                }
                if !output.status.success() {
                    append_log(
                        &setup_dir.join(INSTALL_LOG),
                        &format!(
                            "GStreamer registry 預熱失敗，仍繼續啟動：{}",
                            first_useful_line(&output_text)
                                .unwrap_or_else(|| format!("結束代碼 {}", output.status))
                        ),
                    );
                }
            }
            Err(error) => append_log(
                &setup_dir.join(INSTALL_LOG),
                &format!("GStreamer registry 預熱失敗，仍繼續啟動：{error}"),
            ),
        }
        append_log(
            &setup_dir.join(INSTALL_LOG),
            &format!(
                "GStreamer registry 預熱耗時 {} 毫秒。",
                started_at.elapsed().as_millis()
            ),
        );
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (engine_dir, registry_path, setup_dir, snapshot);
}

/// 將編譯時嵌入二進位檔的平台安裝腳本安全釋放至受保護的 `setup_dir`（原子寫入並拒絕符號連結）。
///
/// # 參數
/// - `setup_dir`: 目標腳本存放目錄。
///
/// # 回傳值
/// 成功釋放回傳腳本檔案路徑 `Ok(PathBuf)`；遇到符號連結或寫入失敗回傳錯誤字串。
pub(crate) fn stage_installer(setup_dir: &Path) -> Result<PathBuf, String> {
    ensure_safe_directory(setup_dir, true)
        .map_err(|error| format!("無法準備工具安裝目錄：{error}"))?;
    #[cfg(target_os = "windows")]
    let (path, contents, mode) = (
        setup_dir.join("install.ps1"),
        include_str!("../../../scripts/install.ps1"),
        0o600,
    );
    #[cfg(not(target_os = "windows"))]
    let (path, contents, mode) = (
        setup_dir.join("install.sh"),
        include_str!("../../../scripts/install.sh"),
        0o700,
    );

    write_file_atomic_secure(&path, contents.as_bytes(), mode)
        .map_err(|error| format!("無法準備工具安裝程序：{error}"))?;
    Ok(path)
}

/// 呼叫平台安裝腳本的檢查模式驗證目前工具與執行期環境是否齊備。
///
/// # 參數
/// - `engine_dir`: 待檢查的工具根目錄。
/// - `setup_dir`: 包含已釋放安裝腳本與 `install.log` 的目錄。
///
/// # 回傳值
/// 回傳包含缺少項目清單的 `RuntimeCheck`；檢查指令無法啟動或逾時則回傳錯誤字串。
fn check_runtime(
    engine_dir: &Path,
    setup_dir: &Path,
    expect_uxplay_version: Option<&str>,
) -> Result<RuntimeCheck, String> {
    #[cfg(target_os = "windows")]
    let (executable, mut args) = (
        windows_powershell_executable(),
        vec![
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            setup_dir.join("install.ps1").to_string_lossy().into_owned(),
            "-Check".into(),
            "-EngineDirectory".into(),
            engine_dir.to_string_lossy().into_owned(),
        ],
    );
    #[cfg(not(target_os = "windows"))]
    let (executable, mut args) = (
        PathBuf::from("/bin/bash"),
        vec![
            setup_dir.join("install.sh").to_string_lossy().into_owned(),
            "--check".into(),
            "--engine-dir".into(),
            engine_dir.to_string_lossy().into_owned(),
        ],
    );
    if let Some(version) = expect_uxplay_version {
        #[cfg(target_os = "windows")]
        args.extend(["-ExpectUxPlayVersion".into(), version.to_owned()]);
        #[cfg(not(target_os = "windows"))]
        args.extend(["--expect-uxplay-version".into(), version.to_owned()]);
    }

    let output =
        run_with_timeout_in_path(&executable, &args, CHECK_TIMEOUT, None).map_err(|error| {
            let detail = format!("工具依賴檢查無法完成：{error}");
            append_log(&setup_dir.join(INSTALL_LOG), &detail);
            detail
        })?;
    let output_text = combined_output(&output);
    if !output_text.trim().is_empty() {
        append_log(&setup_dir.join(INSTALL_LOG), output_text.trim());
    }

    let mut missing = parse_missing_dependencies(&output_text);
    if !output.status.success() && missing.is_empty() {
        let detail = first_useful_line(&output_text)
            .unwrap_or_else(|| format!("檢查程序結束代碼 {}", output.status));
        missing.push(format!("依賴檢查失敗：{detail}"));
    }
    if output.status.success() {
        missing.clear();
    }
    Ok(RuntimeCheck { missing })
}

/// 依作業系統啟動安裝腳本（macOS 透過 AppleScript 開啟 Terminal，其餘平台直接背景執行）並等待其完成。
///
/// # 參數
/// - `setup_dir`: 安裝腳本、日誌、PID 標記與結果檔目錄。
/// - `engine_dir`: 目標投影工具安裝目錄。
/// - `snapshot`: 用於即時推送進度提示的快照鎖。
/// - `force_upgrade`: 是否傳入升級旗標以強制更新現有套件。
///
/// # 回傳值
/// 回傳由腳本寫入的 `InstallResult`；啟動或監控過程失敗則回傳錯誤字串。
fn install_dependencies(
    setup_dir: &Path,
    engine_dir: &Path,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
    force_upgrade: bool,
    options: &InstallInvocationOptions,
) -> Result<InstallResult, String> {
    let log_path = setup_dir.join(INSTALL_LOG);
    let marker_path = setup_dir.join(INSTALL_MARKER);
    let result_path = setup_dir.join(INSTALL_RESULT);
    let launch_path = setup_dir.join(INSTALL_LAUNCH);
    rotate_install_log_if_oversized(&log_path)?;
    remove_if_exists(&marker_path);
    remove_if_exists(&result_path);
    remove_if_exists(&setup_dir.join(INSTALL_PROGRESS));
    let launch = LaunchRecord {
        owner_pid: std::process::id(),
        worker_pid: None,
        started_at: unix_time_seconds(),
    };
    write_json_atomic(&launch_path, &launch)?;

    #[cfg(target_os = "macos")]
    {
        struct LaunchScriptCleanup<'a>(&'a Path);
        impl Drop for LaunchScriptCleanup<'_> {
            fn drop(&mut self) {
                remove_if_exists(self.0);
            }
        }

        let script = setup_dir.join("install.sh");
        let launch_script = setup_dir.join("launch.sh");
        for path in [
            script.as_path(),
            launch_script.as_path(),
            log_path.as_path(),
            engine_dir,
            marker_path.as_path(),
            result_path.as_path(),
        ] {
            validate_terminal_path(path)?;
        }
        let launch_contents = "#!/bin/bash\nset -euo pipefail\nif [ -L \"$1\" ] || [ -L \"$2\" ]; then\n  printf 'Refusing symlinked installer or log target.\\n' >&2\n  exit 1\nfi\nexec > >(tee -a \"$2\") 2>&1\ninstaller=\"$1\"\nengine=\"$3\"\nmarker=\"$4\"\nresult=\"$5\"\nshift 5\nexec /bin/bash \"$installer\" --gui --engine-dir \"$engine\" --marker-file \"$marker\" --result-file \"$result\" \"$@\"\n";
        write_file_atomic_secure(&launch_script, launch_contents.as_bytes(), 0o700)
            .map_err(|error| format!("無法準備終端安裝程序：{error}"))?;
        let _cleanup_launch_script = LaunchScriptCleanup(&launch_script);

        let mut raw_args = vec![
            "/bin/bash".to_owned(),
            launch_script.to_string_lossy().into_owned(),
            script.to_string_lossy().into_owned(),
            log_path.to_string_lossy().into_owned(),
            engine_dir.to_string_lossy().into_owned(),
            marker_path.to_string_lossy().into_owned(),
            result_path.to_string_lossy().into_owned(),
        ];
        raw_args.extend(unix_extra_installer_args(force_upgrade, options));
        let command_line = raw_args
            .into_iter()
            .map(|value| shell_quote(&value))
            .collect::<Vec<_>>()
            .join(" ");
        let osascript_args = macos_terminal_osascript_args(&command_line)?;
        let output = run_with_timeout_in_path(
            Path::new("/usr/bin/osascript"),
            &osascript_args,
            Duration::from_secs(30),
            None,
        );
        match output {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                append_log(&log_path, &format!("Terminal 安裝啟動失敗：{detail}"));
                let _ = wait_for_install(engine_dir, setup_dir, None, None, Some(launch), snapshot);
                return read_install_result(&result_path)?.ok_or_else(|| {
                    if detail.is_empty() {
                        format!("無法啟動 Terminal 安裝程序，結束代碼 {}", output.status)
                    } else {
                        format!("無法啟動 Terminal 安裝程序：{detail}")
                    }
                });
            }
            Err(error) => {
                append_log(&log_path, &format!("Terminal 安裝啟動結果不明：{error}"));
                let result =
                    wait_for_install(engine_dir, setup_dir, None, None, Some(launch), snapshot);
                return result?.ok_or_else(|| {
                    "Terminal 啟動結果不明；為避免重複安裝，請確認終端狀態後再重試。".into()
                });
            }
        }
        wait_for_install(engine_dir, setup_dir, None, None, Some(launch), snapshot)?
            .ok_or_else(|| "Terminal 安裝程序沒有回報完成結果。".into())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let script = setup_dir.join(if cfg!(target_os = "windows") {
            "install.ps1"
        } else {
            "install.sh"
        });
        reject_symlink_path(&log_path)
            .map_err(|error| format!("安裝記錄檔案路徑不安全：{error}"))?;
        let mut log_options = OpenOptions::new();
        log_options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            log_options.mode(0o600);
        }
        let log = log_options
            .open(&log_path)
            .map_err(|error| format!("無法建立安裝記錄：{error}"))?;
        let mut command = installer_command_with_options(
            &script,
            engine_dir,
            &marker_path,
            &result_path,
            force_upgrade,
            options,
        );
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                log.try_clone()
                    .map_err(|error| format!("無法複製安裝記錄：{error}"))?,
            ))
            .stderr(Stdio::from(log));
        let child = command
            .spawn()
            .map_err(|error| format!("無法啟動工具安裝程序：{error}"))?;
        let mut launch = launch;
        launch.worker_pid = Some(child.id());
        write_json_atomic(&launch_path, &launch)?;
        wait_for_install(
            engine_dir,
            setup_dir,
            None,
            Some(child),
            Some(launch),
            snapshot,
        )?
        .ok_or_else(|| "安裝程序沒有回報完成結果。".into())
    }
}

/// 組裝 Unix（macOS / Linux）傳遞給 `install.sh` 的額外旗標與安全驗證後之版本參數。
#[cfg(any(target_os = "macos", target_os = "linux", test))]
pub(crate) fn unix_extra_installer_args(
    force_upgrade: bool,
    options: &InstallInvocationOptions,
) -> Vec<String> {
    let mut args = Vec::new();
    if force_upgrade {
        args.push("--upgrade".into());
    }
    if let Some(only_tool) = options.only_tool {
        args.push("--only".into());
        args.push(only_tool.as_cli_value().into());
    }
    if let Some(uxplay) = options.uxplay.as_ref() {
        args.push("--uxplay-ref".into());
        args.push(uxplay.reference.clone());
        args.push("--expect-uxplay-version".into());
        args.push(uxplay.version.clone());
        if let Some(sha) = uxplay.sha256.as_ref() {
            args.push("--uxplay-sha256".into());
            args.push(sha.clone());
        } else if uxplay.allow_unverified {
            args.push("--allow-unverified-uxplay".into());
        }
    }
    if let Some(scrcpy) = options.scrcpy.as_ref() {
        args.push("--scrcpy-version".into());
        args.push(scrcpy.version.clone());
        if let Some(sha) = scrcpy.sha256.as_ref() {
            args.push("--scrcpy-sha256".into());
            args.push(sha.clone());
        }
    }
    args
}

/// 組裝 Windows 傳遞給 `install.ps1` 的額外旗標與安全驗證後之版本參數。
#[cfg(any(target_os = "windows", test))]
pub(crate) fn windows_extra_installer_args(
    force_upgrade: bool,
    options: &InstallInvocationOptions,
) -> Vec<String> {
    let mut args = Vec::new();
    if force_upgrade {
        args.push("-Upgrade".into());
    }
    if let Some(only_tool) = options.only_tool {
        args.push("-Only".into());
        args.push(only_tool.as_cli_value().into());
    }
    if let Some(uxplay) = options.uxplay.as_ref() {
        args.push("-UxPlayRef".into());
        args.push(uxplay.reference.clone());
        args.push("-ExpectUxPlayVersion".into());
        args.push(uxplay.version.clone());
        if let Some(sha) = uxplay.sha256.as_ref() {
            args.push("-UxPlaySha256".into());
            args.push(sha.clone());
        } else if uxplay.allow_unverified {
            args.push("-AllowUnverifiedUxPlay".into());
        }
    }
    if let Some(scrcpy) = options.scrcpy.as_ref() {
        args.push("-ScrcpyVersionOverride".into());
        args.push(scrcpy.version.clone());
        if let Some(sha) = scrcpy.sha256.as_ref() {
            args.push("-ScrcpySha256".into());
            args.push(sha.clone());
        }
    }
    args
}

#[cfg(all(target_os = "linux", test))]
pub(crate) fn installer_command(
    script: &Path,
    engine_dir: &Path,
    marker: &Path,
    result: &Path,
    force_upgrade: bool,
) -> Command {
    installer_command_with_options(
        script,
        engine_dir,
        marker,
        result,
        force_upgrade,
        &InstallInvocationOptions::default(),
    )
}

#[cfg(target_os = "windows")]
pub(crate) fn installer_command_with_options(
    script: &Path,
    engine_dir: &Path,
    marker: &Path,
    result: &Path,
    force_upgrade: bool,
    options: &InstallInvocationOptions,
) -> Command {
    let mut command = background_command(windows_powershell_executable());
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]);
    command
        .arg(script)
        .arg("-Gui")
        .arg("-EngineDirectory")
        .arg(engine_dir)
        .arg("-MarkerFile")
        .arg(marker)
        .arg("-ResultFile")
        .arg(result)
        .arg("-ProgressFile")
        .arg(result.with_file_name(INSTALL_PROGRESS))
        .arg("-LogFile")
        .arg(result.with_file_name(INSTALL_LOG));
    command.args(windows_extra_installer_args(force_upgrade, options));
    command
}

/// 建構 Linux Bash 安裝命令。
#[cfg(target_os = "linux")]
pub(crate) fn installer_command_with_options(
    script: &Path,
    engine_dir: &Path,
    marker: &Path,
    result: &Path,
    force_upgrade: bool,
    options: &InstallInvocationOptions,
) -> Command {
    let mut command = Command::new("/bin/bash");
    command
        .arg(script)
        .args(["--gui", "--engine-dir"])
        .arg(engine_dir)
        .arg("--marker-file")
        .arg(marker)
        .arg("--result-file")
        .arg(result);
    command.args(unix_extra_installer_args(force_upgrade, options));
    command
}

/// 持續輪詢安裝進度檔、子程序狀態、PID 標記與結果檔，直到安裝產出結果或確認中斷。
///
/// # 參數
/// - `engine_dir`: 投影工具目錄。
/// - `setup_dir`: 追蹤檔案（`install.pid`、`install-result.json`、`install-progress.json`）所在目錄。
/// - `expected_pid`: 已知的既有安裝程序 PID（若有）。
/// - `child`: 直接產生的背景安裝子程序句柄（macOS 透過 Terminal 啟動時為 `None`）。
/// - `launch`: 本次或上一次啟動紀錄。
/// - `snapshot`: 用於即時更新 UI 進度提示的快照鎖。
///
/// # 回傳值
/// 成功讀到結果檔回傳 `Ok(Some(InstallResult))`；程序意外結束或無法確認狀態時回傳錯誤字串。
pub(crate) fn wait_for_install(
    engine_dir: &Path,
    setup_dir: &Path,
    mut expected_pid: Option<u32>,
    mut child: Option<Child>,
    launch: Option<LaunchRecord>,
    snapshot: &Arc<Mutex<DependencySnapshot>>,
) -> Result<Option<InstallResult>, String> {
    let marker_path = setup_dir.join(INSTALL_MARKER);
    let result_path = setup_dir.join(INSTALL_RESULT);
    let launch_deadline = launch
        .as_ref()
        .map(|record| record.started_at.saturating_add(LAUNCH_GRACE.as_secs()));
    let mut child_finished_at = None;
    let mut child_exit = None;
    let mut last_progress = None;
    let mut unknown_process_since: Option<Instant> = None;

    loop {
        if let Ok(Some(progress)) = read_install_progress(&setup_dir.join(INSTALL_PROGRESS)) {
            publish_installing(snapshot, &progress.message);
            last_progress = Some(progress);
        }
        let mut child_alive = false;
        let child_finished = if let Some(process) = child.as_mut() {
            match process.try_wait() {
                Ok(None) => {
                    child_alive = true;
                    false
                }
                Ok(Some(status)) => {
                    child_exit = Some(status);
                    true
                }
                Err(error) => {
                    let _ = process.kill();
                    let _ = process.wait();
                    return Err(format!("無法檢查安裝程序狀態：{error}"));
                }
            }
        } else {
            false
        };
        if child_finished {
            child = None;
            child_finished_at.get_or_insert_with(Instant::now);
        }
        if let Some(result) = read_install_result(&result_path)? {
            remove_if_exists(&marker_path);
            if let Some(mut process) = child.take() {
                if let Err(error) = thread::Builder::new()
                    .name("neo-airplay-installer-reaper".into())
                    .spawn(move || {
                        let _ = process.wait();
                    })
                {
                    append_log(
                        &setup_dir.join(INSTALL_LOG),
                        &format!("無法啟動安裝程序回收工作：{error}"),
                    );
                }
            }
            return Ok(Some(result));
        }
        let mut unknown_pid = None;
        if let Some(pid) = read_install_marker(&marker_path)? {
            expected_pid = Some(pid);
            match process_presence(pid, engine_dir) {
                ProcessPresence::Alive => {
                    unknown_process_since = None;
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }
                ProcessPresence::Unknown => unknown_pid = Some(pid),
                ProcessPresence::Dead => {}
            }
        }
        if let Some(record) = launch.as_ref() {
            if let Some(pid) = record.worker_pid {
                match process_presence(pid, engine_dir) {
                    ProcessPresence::Alive => child_alive = true,
                    ProcessPresence::Unknown => {
                        unknown_pid.get_or_insert(pid);
                    }
                    ProcessPresence::Dead => {}
                };
            }
        }
        if child_alive {
            unknown_process_since = None;
            thread::sleep(POLL_INTERVAL);
            continue;
        }
        if let Some(pid) = unknown_pid {
            let started_at = unknown_process_since.get_or_insert_with(Instant::now);
            if started_at.elapsed() >= LAUNCH_GRACE {
                let detail = format!(
                    "無法確認安裝程序 {pid} 是否仍在執行；為避免重複安裝，已停止自動重試。"
                );
                append_log(&setup_dir.join(INSTALL_LOG), &detail);
                return Err(detail);
            }
            thread::sleep(POLL_INTERVAL);
            continue;
        }
        unknown_process_since = None;

        if child_finished_at.is_none()
            && launch_deadline.is_some_and(|deadline| unix_time_seconds() < deadline)
        {
            thread::sleep(POLL_INTERVAL);
            continue;
        }

        let grace_started = child_finished_at.get_or_insert_with(Instant::now);
        if grace_started.elapsed() >= RESULT_GRACE {
            let detail =
                interrupted_install_detail(expected_pid, child_exit, last_progress.as_ref());
            append_log(&setup_dir.join(INSTALL_LOG), &detail);
            remove_if_exists(&marker_path);
            remove_if_exists(&setup_dir.join(INSTALL_LAUNCH));
            return Err(detail);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// 讀取並驗證安裝進度 JSON 檔案。
///
/// # 參數
/// - `path`: `install-progress.json` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；有效進度回傳 `Ok(Some(InstallProgress))`；格式或讀取異常回傳錯誤字串。
fn read_install_progress(path: &Path) -> Result<Option<InstallProgress>, String> {
    match read_bounded_regular_file(path, MAX_INSTALL_METADATA_BYTES) {
        Ok(contents) => {
            let progress: InstallProgress = serde_json::from_slice(&contents)
                .map_err(|error| format!("安裝階段記錄格式無效：{error}"))?;
            Ok((progress.pid != 0 && !progress.message.trim().is_empty()).then_some(progress))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝階段記錄：{error}")),
    }
}

/// 讀取安裝程序 PID 標記檔案。
///
/// # 參數
/// - `path`: `install.pid` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；有效非零 PID 回傳 `Ok(Some(u32))`；內容無效或讀取失敗回傳錯誤字串。
fn read_install_marker(path: &Path) -> Result<Option<u32>, String> {
    match read_bounded_regular_file(path, MAX_INSTALL_METADATA_BYTES) {
        Ok(bytes) => {
            let value = String::from_utf8(bytes).map_err(|_| {
                "安裝程序 PID 記錄格式無效；為避免啟動重複安裝，請檢查記錄。".to_owned()
            })?;
            value
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|pid| *pid != 0)
                .map(Some)
                .ok_or_else(|| "安裝程序 PID 記錄格式無效；為避免啟動重複安裝，請檢查記錄。".into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序 PID 記錄：{error}")),
    }
}

/// 讀取安裝結果 JSON 檔案。
///
/// # 參數
/// - `path`: `install-result.json` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；有效結果回傳 `Ok(Some(InstallResult))`；格式或 PID 無效回傳錯誤字串。
fn read_install_result(path: &Path) -> Result<Option<InstallResult>, String> {
    match read_bounded_regular_file(path, MAX_INSTALL_METADATA_BYTES) {
        Ok(contents) => {
            let result: InstallResult = serde_json::from_slice(&contents)
                .map_err(|error| format!("安裝程序完成記錄格式無效：{error}"))?;
            if result.pid == 0 {
                return Err("安裝程序完成記錄中的 PID 無效。".into());
            }
            Ok(Some(result))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序完成記錄：{error}")),
    }
}

/// 讀取上一次或當前啟動的安裝紀錄檔案。
///
/// # 參數
/// - `path`: `install-launch.json` 檔案路徑。
///
/// # 回傳值
/// 檔案不存在回傳 `Ok(None)`；讀取或反序列化成功回傳 `Ok(Some(LaunchRecord))`；失敗回傳錯誤字串。
fn read_launch_record(path: &Path) -> Result<Option<LaunchRecord>, String> {
    match read_bounded_regular_file(path, MAX_INSTALL_METADATA_BYTES) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map(Some)
            .map_err(|error| format!("安裝程序啟動記錄格式無效：{error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("無法讀取安裝程序啟動記錄：{error}")),
    }
}

/// 外部行程存活探測結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessPresence {
    /// 確認行程仍在作業系統中執行。
    Alive,
    /// 確認行程已不存在。
    Dead,
    /// 因權限或探測逾時無法確定行程狀態。
    Unknown,
}

/// 依作業系統檢查指定 PID 的安裝程序是否仍在執行。
///
/// # 參數
/// - `pid`: 欲探測的作業系統行程識別碼。
/// - `_engine_dir`: 保留參數（Windows 改用系統 `System32` 絕對路徑呼叫 `tasklist.exe`）。
///
/// # 回傳值
/// 回傳 `ProcessPresence::Alive`、`Dead` 或 `Unknown`。
fn process_presence(pid: u32, _engine_dir: &Path) -> ProcessPresence {
    if pid == 0 {
        return ProcessPresence::Dead;
    }
    #[cfg(unix)]
    {
        let output = run_with_timeout_in_path(
            Path::new("/bin/kill"),
            &["-0".into(), pid.to_string()],
            PROCESS_PROBE_TIMEOUT,
            None,
        );
        match output {
            Ok(output) if output.status.success() => ProcessPresence::Alive,
            Ok(output) => {
                let detail = combined_output(&output).to_ascii_lowercase();
                if detail.contains("no such process") {
                    ProcessPresence::Dead
                } else {
                    ProcessPresence::Unknown
                }
            }
            Err(_) => ProcessPresence::Unknown,
        }
    }
    #[cfg(target_os = "windows")]
    {
        let output = run_with_timeout_in_path(
            &windows_tasklist_executable(),
            &[
                "/FI".into(),
                format!("PID eq {pid}"),
                "/FO".into(),
                "CSV".into(),
                "/NH".into(),
            ],
            PROCESS_PROBE_TIMEOUT,
            None,
        );
        match output {
            Ok(output) if output.status.success() && tasklist_contains_pid(&output, pid) => {
                ProcessPresence::Alive
            }
            Ok(output) if output.status.success() => ProcessPresence::Dead,
            Ok(_) | Err(_) => ProcessPresence::Unknown,
        }
    }
}

/// 解析 Windows `SystemRoot` 環境變數以取得受保護的 `System32` 目錄路徑，忽略相對或 UNC 路徑。
#[cfg(any(target_os = "windows", test))]
pub(crate) fn windows_system32_dir_from(system_root: Option<&OsStr>) -> PathBuf {
    let valid_root = system_root
        .filter(|value| !value.is_empty())
        .and_then(|value| value.to_str())
        .filter(|text| {
            !text.starts_with(r"\\")
                && !text.starts_with("//")
                && (text.starts_with('/')
                    || (text.len() >= 3
                        && text.as_bytes()[0].is_ascii_alphabetic()
                        && text.as_bytes()[1] == b':'
                        && matches!(text.as_bytes()[2], b'\\' | b'/')))
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    valid_root.join("System32")
}

/// 依 `SystemRoot` 組裝內建 Windows PowerShell 5.1 的絕對執行路徑，防止 PATH 劫持。
#[cfg(any(target_os = "windows", test))]
pub(crate) fn windows_powershell_executable_from(system_root: Option<&OsStr>) -> PathBuf {
    windows_system32_dir_from(system_root)
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_powershell_executable() -> PathBuf {
    windows_powershell_executable_from(std::env::var_os("SystemRoot").as_deref())
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_tasklist_executable() -> PathBuf {
    windows_system32_dir_from(std::env::var_os("SystemRoot").as_deref()).join("tasklist.exe")
}

/// 解析 Windows `tasklist.exe /FO CSV /NH` 輸出，判斷目標 PID 是否仍在執行清單中。
///
/// # 參數
/// - `output`: `tasklist.exe` 指令輸出。
/// - `pid`: 目標行程識別碼。
///
/// # 回傳值
/// 若 CSV 第二欄精確吻合 `pid` 則回傳 `true`。
#[cfg(any(target_os = "windows", test))]
pub(crate) fn tasklist_contains_pid(output: &Output, pid: u32) -> bool {
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.split(',')
            .nth(1)
            .map(|field| field.trim().trim_matches('"') == pid.to_string())
            .unwrap_or(false)
    })
}

/// 判斷啟動紀錄是否仍落在 `LAUNCH_GRACE` 寬限視窗內，用於等待尚未寫入 PID 標記的提權程序。
///
/// # 參數
/// - `record`: 上一次寫入的 `LaunchRecord`。
///
/// # 回傳值
/// 若距今小於 `LAUNCH_GRACE` 秒則回傳 `true`。
fn launch_is_recent(record: &LaunchRecord) -> bool {
    unix_time_seconds().saturating_sub(record.started_at) < LAUNCH_GRACE.as_secs()
}

/// 以唯一暫存檔與原子更名方式將資料序列化為 JSON 檔案，並拒絕覆寫符號連結。
///
/// # 參數
/// - `path`: 目標 JSON 檔案路徑。
/// - `value`: 實作 `Serialize` 的資料結構參考。
///
/// # 回傳值
/// 成功回傳 `Ok(())`；序列化、寫入或更名失敗時回傳錯誤字串。
pub(crate) fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    write_file_atomic_secure(path, &bytes, 0o600).map_err(|error| error.to_string())
}

/// 若目標檔案存在則盡力移除，忽略不存在或其他刪除錯誤（用於清理非關鍵標記檔）。
///
/// # 參數
/// - `path`: 欲清理的檔案路徑。
fn remove_if_exists(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

/// 重新安裝前清除應用程式專用工具目錄內容與 GStreamer 快取檔案。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的投影工具目錄。
/// - `gstreamer_registry_path`: GStreamer registry 快取檔案路徑。
///
/// # 回傳值
/// 成功清除回傳 `Ok(())`；遇到符號連結目錄或刪除失敗則回傳錯誤字串。
pub(crate) fn clear_app_tools(
    engine_dir: &Path,
    gstreamer_registry_path: &Path,
) -> Result<(), String> {
    clear_engine_directory(engine_dir)?;
    clear_gstreamer_registry(gstreamer_registry_path)
}

/// 移除指定的 GStreamer registry 快取檔案（若路徑非空），並拒絕符號連結路徑與父目錄。
///
/// # 參數
/// - `path`: GStreamer registry 快取檔案路徑。
///
/// # 回傳值
/// 成功移除或檔案本不存在回傳 `Ok(())`；遇到符號連結或刪除失敗回傳錯誤字串。
pub(crate) fn clear_gstreamer_registry(path: &Path) -> Result<(), String> {
    if !path.as_os_str().is_empty() {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            reject_symlink_path(parent)
                .map_err(|_| "GStreamer 快取目錄不是安全的資料夾，未移除任何檔案。".to_owned())?;
        }
        reject_symlink_path(path)
            .map_err(|_| "GStreamer 快取檔案不可為符號連結，未移除任何檔案。".to_owned())?;
        remove_file_if_exists(path)?;
    }
    Ok(())
}

/// 安全清空工具目錄下的可重建項目，但保留 `.GITKEEP` 與 `.bundled-*` 標記檔並拒絕符號連結。
///
/// # 參數
/// - `path`: 工具目錄路徑。
///
/// # 回傳值
/// 清理成功回傳 `Ok(())`；目錄無效或遭遇系統權限錯誤回傳錯誤字串。
fn clear_engine_directory(path: &Path) -> Result<(), String> {
    ensure_safe_directory(path, false)
        .map_err(|_| "工具目錄不是安全的資料夾，未移除任何工具。".to_owned())?;

    for entry in fs::read_dir(path).map_err(|error| format!("無法讀取工具目錄：{error}"))?
    {
        let entry = entry.map_err(|error| format!("無法讀取工具目錄項目：{error}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".GITKEEP" || name.starts_with(".bundled-") {
            continue;
        }
        let entry_path = entry.path();
        let entry_type = fs::symlink_metadata(&entry_path)
            .map_err(|error| format!("無法檢查工具目錄項目：{error}"))?
            .file_type();
        if entry_type.is_dir() {
            fs::remove_dir_all(&entry_path)
                .map_err(|error| format!("無法清除工具目錄項目：{error}"))?;
        } else {
            fs::remove_file(&entry_path)
                .map_err(|error| format!("無法清除工具目錄項目：{error}"))?;
        }
    }
    Ok(())
}

/// 刪除單一檔案並對「找不到檔案」視為成功，其餘 I/O 錯誤轉為可讀訊息。
///
/// # 參數
/// - `path`: 欲移除的檔案路徑。
///
/// # 回傳值
/// 成功或檔案不存在回傳 `Ok(())`；其他刪除失敗回傳錯誤字串。
fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("無法清除工具快取：{error}")),
    }
}

/// 安全讀取安裝日誌檔案（最多僅回傳尾端 `max_bytes`），拒絕符號連結或非一般檔案。
fn read_log_tail(path: &Path, max_bytes: usize) -> Result<String, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("尚無安裝記錄。".to_owned());
        }
        Err(error) => return Err(format!("無法讀取安裝記錄：{error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("安裝記錄檔案路徑不安全。".to_owned());
    }
    if metadata.len() <= max_bytes as u64 {
        let bytes = fs::read(path).map_err(|error| format!("無法讀取安裝記錄：{error}"))?;
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    let mut file = fs::File::open(path).map_err(|error| format!("無法讀取安裝記錄：{error}"))?;
    file.seek(SeekFrom::End(-(max_bytes as i64)))
        .map_err(|error| format!("無法讀取安裝記錄：{error}"))?;
    let mut bytes = Vec::with_capacity(max_bytes);
    file.take(max_bytes as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("無法讀取安裝記錄：{error}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let aligned = match text.find('\n') {
        Some(index) => &text[index + 1..],
        None => text.as_ref(),
    };
    Ok(format!(
        "[已省略較早記錄，僅顯示最後 1 MiB 內容]\n{aligned}"
    ))
}

/// 在啟動新一輪安裝前，若既有 `install.log` 超過大小上限，原子保留最後 1 MiB 內容以防止磁碟無限制膨脹。
fn rotate_install_log_if_oversized(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("無法檢查安裝記錄：{error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("安裝記錄檔案路徑不安全。".to_owned());
    }
    if metadata.len() <= MAX_INSTALL_LOG_FILE_BYTES {
        return Ok(());
    }
    let tail = read_log_tail(path, MAX_INSTALL_LOG_READ_BYTES)?;
    write_file_atomic_secure(path, tail.as_bytes(), 0o600)
        .map_err(|error| format!("無法修剪過大的安裝記錄：{error}"))
}

/// 將單行診斷或腳本輸出附加寫入安裝日誌檔尾端，並拒絕追蹤符號連結。
///
/// # 參數
/// - `path`: 安裝日誌檔案（`install.log`）路徑。
/// - `message`: 欲附加的文字內容。
fn append_log(path: &Path, message: &str) {
    if reject_symlink_path(path).is_err() {
        return;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(path) {
        let _ = writeln!(file, "{message}");
    }
}

/// 合併外部程序輸出的 `stdout` 與非空 `stderr` 為單一 UTF-8 字串。
///
/// # 參數
/// - `output`: 外部程序的 `Output` 結構參考。
///
/// # 回傳值
/// 回傳合併後的文字內容。
fn combined_output(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    text
}

/// 更新共用依賴狀態快照為 `installing` 並刷新介面顯示訊息。
///
/// # 參數
/// - `snapshot`: 受互斥鎖保護的共用 `DependencySnapshot`。
/// - `message`: 最新的階段提示文字。
fn publish_installing(snapshot: &Arc<Mutex<DependencySnapshot>>, message: &str) {
    if let Ok(mut snapshot) = snapshot.lock() {
        snapshot.status = "installing".into();
        snapshot.message = message.to_owned();
    }
}

/// 取得目前的 Unix 時間戳記（單位：秒）。
///
/// # 回傳值
/// 回傳自 Unix 紀元以來的秒數；若系統時間異常則回傳 `0`。
fn unix_time_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 驗證傳遞給 macOS 終端機安裝器的路徑不含控制字元（NUL、LF、CR）。
#[cfg(any(target_os = "macos", test))]
pub(crate) fn validate_terminal_path(path: &Path) -> Result<(), String> {
    let text = path.to_string_lossy();
    if text.chars().any(|ch| matches!(ch, '\0' | '\n' | '\r')) {
        return Err("終端機安裝路徑包含不安全的控制字元。".into());
    }
    Ok(())
}

/// 建構使用 `on run argv` 安全傳遞參數給 Terminal.app 的 `osascript` 命令列參數。
#[cfg(any(target_os = "macos", test))]
pub(crate) fn macos_terminal_osascript_args(command_line: &str) -> Result<Vec<String>, String> {
    if command_line
        .chars()
        .any(|ch| matches!(ch, '\0' | '\n' | '\r'))
    {
        return Err("終端機安裝命令包含不安全的控制字元。".into());
    }
    Ok(vec![
        "-e".into(),
        "on run argv".into(),
        "-e".into(),
        "tell application \"Terminal\" to do script (item 1 of argv)".into(),
        "-e".into(),
        "end run".into(),
        "--".into(),
        command_line.to_owned(),
    ])
}

/// 在 macOS 下為 shell 命令參數加上單引號跳脫保護，防止空白或特殊字元被 Shell 誤解析。
///
/// # 參數
/// - `value`: 欲跳脫的原始字串（例如路徑）。
///
/// # 回傳值
/// 回傳經過單引號跳脫包覆的安全字串。
#[cfg(any(target_os = "macos", test))]
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "../../../../desktop-test/backend/unit-tests/dependencies.rs"]
mod tests;
