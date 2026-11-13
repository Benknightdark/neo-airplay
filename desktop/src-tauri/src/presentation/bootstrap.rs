//! Tauri 桌面應用程式組裝進入點，負責外掛初始化、目錄解析、系統匣與視窗生命週期管理。

use crate::application::SaveSettingsUseCase;
use crate::infrastructure::{AppState, DependencyManager};
use crate::presentation::commands;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

/// 建立並啟動 Tauri 應用程式執行迴圈，註冊狀態、外掛、系統匣與 IPC 命令處理器。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(SaveSettingsUseCase::default())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    #[cfg(feature = "wdio")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .setup(|app| {
            let e2e_profile = e2e_profile_directory();
            let (config_dir, log_dir) =
                application_directories(app.handle(), e2e_profile.as_deref())?;
            let gstreamer_registry_path = if cfg!(target_os = "macos") {
                match e2e_profile.as_deref() {
                    Some(profile) => profile.join("cache/gstreamer/registry.bin"),
                    None => app.path().app_cache_dir()?.join("gstreamer/registry.bin"),
                }
            } else {
                std::path::PathBuf::new()
            };
            let engine_dir = match &e2e_profile {
                Some(profile) => profile.join("engine"),
                None => {
                    #[cfg(debug_assertions)]
                    let engine_dir =
                        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../engine");

                    #[cfg(not(debug_assertions))]
                    let engine_dir = {
                        let engine_dir = app.path().app_local_data_dir()?.join("engine");
                        let bundled_engine = app.path().resource_dir()?.join("engine");
                        seed_bundled_engine(&bundled_engine, &engine_dir)?;
                        engine_dir
                    };

                    engine_dir
                }
            };
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&log_dir)?;
            std::fs::create_dir_all(&engine_dir)?;
            let state = AppState::new(
                config_dir.clone(),
                log_dir,
                engine_dir.clone(),
                gstreamer_registry_path.clone(),
                cfg!(target_os = "macos") && e2e_profile.is_none(),
            )
            .map_err(std::io::Error::other)?;
            if state
                .settings()
                .map_err(std::io::Error::other)?
                .start_on_login
            {
                app.autolaunch()
                    .enable()
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
            }
            app.manage(state);
            let dependencies = DependencyManager::new(
                engine_dir,
                gstreamer_registry_path,
                &config_dir,
                e2e_profile.is_none()
                    && std::env::var_os("NEO_AIRPLAY_SKIP_SETUP").is_none_or(|value| value != "1"),
            );
            // Check and repair missing runtime tools on every normal app start. The manager
            // performs this work on a background thread so the window can open immediately.
            dependencies.start().map_err(std::io::Error::other)?;
            app.manage(dependencies);

            let show_item = MenuItem::with_id(app, "show", "顯示主視窗", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "結束 Neo AirPlay", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| std::io::Error::other("找不到系統匣圖示。"))?;

            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Neo AirPlay")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_dashboard,
            commands::get_dependencies,
            commands::install_dependencies,
            commands::manage_dependencies,
            commands::read_install_log,
            commands::save_settings,
            commands::start_airplay,
            commands::stop_airplay,
            commands::refresh_android_devices,
            commands::start_android_mirror,
            commands::stop_android_mirror
        ])
        .run(tauri::generate_context!())
        .expect("無法啟動 Neo AirPlay 桌面應用程式");
}

/// 在正式發行版本首次啟動時，將隨附打包的外部工具與其權限複製至使用者本機工具目錄並寫入版本標記。
///
/// # 參數
/// - `bundled_engine`: 隨應用程式資源打包的原始 `engine` 目錄。
/// - `engine_dir`: 使用者資料目錄下的目標 `engine` 目錄。
///
/// # 回傳值
/// 成功複製或已存在版本標記時回傳 `Ok(())`；檔案 I/O 失敗回傳 `std::io::Error`。
#[cfg(not(debug_assertions))]
fn seed_bundled_engine(
    bundled_engine: &std::path::Path,
    engine_dir: &std::path::Path,
) -> Result<(), std::io::Error> {
    use std::fs;

    let marker = engine_dir.join(format!(".bundled-{}", env!("CARGO_PKG_VERSION")));
    if marker.exists() || !bundled_engine.is_dir() {
        return Ok(());
    }

    /// 遞迴複製來源目錄內的所有子目錄、檔案與執行權限設定至目標目錄。
    ///
    /// # 參數
    /// - `source`: 來源目錄路徑。
    /// - `destination`: 目標目錄路徑。
    ///
    /// # 回傳值
    /// 完成遞迴複製回傳 `Ok(())`，否則回傳 `std::io::Error`。
    fn copy_contents(
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let metadata = fs::metadata(&source_path)?;
            if metadata.is_dir() {
                copy_contents(&source_path, &destination_path)?;
            } else if metadata.is_file() {
                fs::copy(&source_path, &destination_path)?;
                fs::set_permissions(&destination_path, metadata.permissions())?;
            }
        }
        Ok(())
    }

    copy_contents(bundled_engine, engine_dir)?;
    fs::write(marker, b"seeded\n")
}

/// 顯示、取消最小化並將焦點帶至主視窗。
///
/// # 參數
/// - `app`: Tauri 應用程式執行控制代碼。
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 依是否啟用 E2E 測試隔離設定檔，解析設定檔與日誌檔案的存放目錄。
///
/// # 參數
/// - `app`: Tauri 應用程式控制代碼，用於查詢作業系統標準應用程式路徑。
/// - `e2e_profile`: 若處於 E2E 測試隔離模式則傳入隔離根目錄，否則為 `None`。
///
/// # 回傳值
/// 回傳 `(config_dir, log_dir)` 路徑元組或 `tauri::Error`。
fn application_directories(
    app: &tauri::AppHandle,
    e2e_profile: Option<&std::path::Path>,
) -> Result<(std::path::PathBuf, std::path::PathBuf), tauri::Error> {
    if let Some(profile) = e2e_profile {
        return Ok((profile.join("config"), profile.join("logs")));
    }

    Ok((app.path().app_config_dir()?, app.path().app_log_dir()?))
}

/// 在啟用 `wdio` 功能旗標時，從環境變數或 CLI `--e2e-profile` 參數取得 E2E 測試隔離目錄。
///
/// # 回傳值
/// 處於 E2E 隔離測試時回傳 `Some(PathBuf)`，一般桌面模式則回傳 `None`。
fn e2e_profile_directory() -> Option<std::path::PathBuf> {
    #[cfg(feature = "wdio")]
    {
        if let Some(profile) = std::env::var_os("NEO_AIRPLAY_E2E_PROFILE") {
            return Some(std::path::PathBuf::from(profile));
        }

        let mut args = std::env::args_os().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--e2e-profile" {
                if let Some(profile) = args.next() {
                    let path = std::path::PathBuf::from(profile);
                    std::env::set_var("NEO_AIRPLAY_E2E_PROFILE", &path);
                    return Some(path);
                }
            }
        }
    }

    None
}
