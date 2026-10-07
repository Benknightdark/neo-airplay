use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicBool;

use neo_core::{
    AndroidDevice, AndroidMirrorEngine, AppLogEntry, AppPaths, EnginePackageBootstrapper,
    EnginePackageConfig, EngineStatus, FileAppLogger, JsonSettingsStore, Platform,
    ReceiverSettings, RendererOption, SettingsStore, TokioProcessSpawner, UxPlayEngine,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri::path::BaseDirectory;
use tokio::sync::RwLock;
use ts_rs::TS;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct EngineSetupStatus {
    pub message: String,
    pub in_progress: bool,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub receiver: EngineStatus,
    pub android: EngineStatus,
    pub devices: Vec<AndroidDevice>,
    pub settings: IpcReceiverSettings,
    pub platform: Platform,
    pub renderer_options: Vec<RendererOption>,
    pub logs: Vec<AppLogEntry>,
    pub engine_setup: EngineSetupStatus,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct SaveResult {
    pub settings: IpcReceiverSettings,
    pub apply_pending: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct IpcReceiverSettings {
    pub receiver_name: String,
    pub renderer: String,
    pub fullscreen: bool,
    pub start_on_login: bool,
    pub android_hw_decode: bool,
    pub uxplay_path: Option<String>,
}

impl From<ReceiverSettings> for IpcReceiverSettings {
    fn from(settings: ReceiverSettings) -> Self {
        Self {
            receiver_name: settings.receiver_name,
            renderer: settings.renderer,
            fullscreen: settings.fullscreen,
            start_on_login: settings.start_on_login,
            android_hw_decode: settings.android_hw_decode,
            uxplay_path: settings.uxplay_path,
        }
    }
}

impl From<IpcReceiverSettings> for ReceiverSettings {
    fn from(settings: IpcReceiverSettings) -> Self {
        Self {
            receiver_name: settings.receiver_name,
            renderer: settings.renderer,
            fullscreen: settings.fullscreen,
            start_on_login: settings.start_on_login,
            android_hw_decode: settings.android_hw_decode,
            uxplay_path: settings.uxplay_path,
        }
    }
}

pub struct AppState {
    pub platform: Platform,
    pub paths: AppPaths,
    pub logger: Arc<FileAppLogger>,
    pub settings_store: Arc<JsonSettingsStore>,
    pub settings: RwLock<ReceiverSettings>,
    pub receiver: Arc<UxPlayEngine>,
    pub android: Arc<AndroidMirrorEngine>,
    pub engine_package_bootstrapper: Option<Arc<EnginePackageBootstrapper>>,
    pub engine_package_error: Option<String>,
    pub engine_setup: RwLock<EngineSetupStatus>,
    pub e2e_profile: bool,
    #[cfg(target_os = "macos")]
    pub macos_airplay_warning_shown: AtomicBool,
}

impl AppState {
    pub fn new(
        app: &tauri::AppHandle,
        profile: Option<&std::path::Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let paths = AppPaths::new(profile)?;
        std::fs::create_dir_all(
            paths
                .settings_path
                .parent()
                .ok_or("settings directory is unavailable")?,
        )?;
        std::fs::create_dir_all(&paths.log_directory)?;
        std::fs::create_dir_all(&paths.engine_directory)?;
        std::fs::create_dir_all(&paths.engine_package_cache_directory)?;

        let platform = Platform::current();
        let logger = Arc::new(FileAppLogger::new(&paths.log_directory));
        let settings_store = Arc::new(JsonSettingsStore::new(
            &paths.settings_path,
            logger.clone(),
            platform,
        ));
        let settings = tauri::async_runtime::block_on(settings_store.load());
        #[cfg(target_os = "macos")]
        let macos_airplay_warning_shown = AtomicBool::new(
            paths.e2e_profile
                || paths
                    .settings_path
                    .with_file_name("macos-airplay-warning-shown")
                    .is_file(),
        );
        let process_spawner = Arc::new(TokioProcessSpawner);
        let bundled_engine_directory = app.path().resolve("engine", BaseDirectory::Resource).ok();
        let (engine_package_bootstrapper, engine_package_error) = if paths.e2e_profile {
            (None, None)
        } else {
            match load_engine_package_bootstrapper(
                app,
                platform,
                &paths,
                bundled_engine_directory.clone(),
                logger.clone(),
            ) {
                Ok(bootstrapper) => (Some(Arc::new(bootstrapper)), None),
                Err(error) => (None, Some(error)),
            }
        };

        let receiver = Arc::new(UxPlayEngine::new(
            platform,
            bundled_engine_directory.clone(),
            paths.engine_directory.clone(),
            paths.gstreamer_registry_path.clone(),
            logger.clone(),
            process_spawner.clone(),
        ));
        let android = Arc::new(AndroidMirrorEngine::new(
            platform,
            bundled_engine_directory,
            paths.engine_directory.clone(),
            scrcpy_required_files(),
            logger.clone(),
            process_spawner,
        ));

        Ok(Self {
            platform,
            e2e_profile: paths.e2e_profile,
            paths,
            logger,
            settings_store,
            settings: RwLock::new(settings),
            receiver,
            android,
            engine_package_bootstrapper,
            engine_package_error,
            engine_setup: RwLock::new(EngineSetupStatus {
                message: "正在檢查投影引擎。".to_owned(),
                in_progress: true,
            }),
            #[cfg(target_os = "macos")]
            macos_airplay_warning_shown,
        })
    }

    pub async fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            receiver: self.receiver.status(),
            android: self.android.status(),
            devices: self.android.devices().await,
            settings: self.settings.read().await.clone().into(),
            platform: self.platform,
            renderer_options: self.platform.renderer_options(),
            logs: self.logger.snapshot(),
            engine_setup: self.engine_setup.read().await.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

fn load_engine_package_bootstrapper(
    app: &tauri::AppHandle,
    platform: Platform,
    paths: &AppPaths,
    bundled_engine_directory: Option<std::path::PathBuf>,
    logger: Arc<FileAppLogger>,
) -> Result<EnginePackageBootstrapper, String> {
    let config_path = app
        .path()
        .resolve("engine-package.json", BaseDirectory::Resource)
        .map_err(|error| format!("找不到引擎套件設定檔：{error}"))?;
    let config_text = std::fs::read_to_string(config_path)
        .map_err(|error| format!("無法讀取引擎套件設定檔：{error}"))?;
    let config = EnginePackageConfig::parse(&config_text).map_err(|error| error.to_string())?;
    let target_key = EnginePackageConfig::target_key(platform, std::env::consts::ARCH);
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|error| format!("無法建立引擎下載用戶端：{error}"))?;

    EnginePackageBootstrapper::new(
        config,
        target_key,
        bundled_engine_directory,
        paths.engine_directory.clone(),
        paths.engine_package_cache_directory.clone(),
        client,
        logger,
    )
    .map_err(|error| error.to_string())
}

fn scrcpy_required_files() -> Vec<String> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let manifest = include_str!("../../scripts/scrcpy-runtime-files.macos-aarch64.txt");
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let manifest = include_str!("../../scripts/scrcpy-runtime-files.macos-x86_64.txt");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let manifest = include_str!("../../scripts/scrcpy-runtime-files.linux-x86_64.txt");
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let manifest = include_str!("../../scripts/scrcpy-runtime-files.windows-x86_64.txt");
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    let manifest = "";

    manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
