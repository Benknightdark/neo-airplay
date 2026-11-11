mod dependencies;
mod models;
mod state;

use models::{DashboardSnapshot, EngineStatus, ReceiverSettings};
use state::AppState;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

#[derive(Default)]
struct SettingsTransaction(std::sync::Mutex<()>);

#[tauri::command]
fn get_dependencies(
    state: State<'_, dependencies::DependencyManager>,
) -> Result<dependencies::DependencySnapshot, String> {
    state.snapshot()
}

#[tauri::command]
fn install_dependencies(
    state: State<'_, dependencies::DependencyManager>,
) -> Result<dependencies::DependencySnapshot, String> {
    state.start()
}

#[tauri::command]
fn manage_dependencies(
    app_state: State<'_, AppState>,
    dependencies: State<'_, dependencies::DependencyManager>,
    action: dependencies::DependencyAction,
) -> Result<dependencies::DependencySnapshot, String> {
    dependencies.manage(action, || {
        let airplay = app_state.stop_airplay();
        let android = app_state.stop_android_mirror();
        let airplay = airplay.map_err(|error| format!("無法停止 AirPlay：{error}"))?;
        let android = android.map_err(|error| format!("無法停止 Android 投影：{error}"))?;
        if airplay.status == EngineStatus::Faulted || android.status == EngineStatus::Faulted {
            return Err(format!(
                "停止投影服務失敗，工具檔案未移除。{} {}",
                airplay.message, android.message
            ));
        }
        Ok(())
    })
}

#[tauri::command]
fn get_dashboard(state: State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    state.dashboard()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    transaction: State<'_, SettingsTransaction>,
    settings: ReceiverSettings,
) -> Result<ReceiverSettings, String> {
    let _transaction = transaction.0.lock().map_err(|_| "設定儲存狀態無法更新。")?;
    let current = state.settings()?;
    let next = settings.sanitized();

    if current.start_on_login != next.start_on_login {
        let manager = app.autolaunch();
        let result = if next.start_on_login {
            manager.enable()
        } else {
            manager.disable()
        };
        if let Err(error) = result {
            return Err(format!("無法更新登入啟動設定：{error}"));
        }
    }

    if let Err(error) = state.save_settings(next.clone()) {
        if current.start_on_login != next.start_on_login {
            let manager = app.autolaunch();
            if current.start_on_login {
                let _ = manager.enable();
            } else {
                let _ = manager.disable();
            }
        }
        return Err(error);
    }

    state.log_info("設定已儲存。");
    Ok(next)
}

#[tauri::command]
fn start_airplay(
    state: State<'_, AppState>,
    dependencies: State<'_, dependencies::DependencyManager>,
) -> Result<models::EngineSnapshot, String> {
    dependencies.with_engine_access(|| state.start_airplay())
}

#[tauri::command]
fn stop_airplay(state: State<'_, AppState>) -> Result<models::EngineSnapshot, String> {
    state.stop_airplay()
}

#[tauri::command(async)]
fn refresh_android_devices(
    state: State<'_, AppState>,
    dependencies: State<'_, dependencies::DependencyManager>,
) -> Result<Vec<models::AndroidDevice>, String> {
    dependencies.with_engine_access(|| state.refresh_android_devices())
}

#[tauri::command]
fn start_android_mirror(
    state: State<'_, AppState>,
    dependencies: State<'_, dependencies::DependencyManager>,
    serial: String,
) -> Result<models::EngineSnapshot, String> {
    dependencies.with_engine_access(|| state.start_android_mirror(&serial))
}

#[tauri::command]
fn stop_android_mirror(state: State<'_, AppState>) -> Result<models::EngineSnapshot, String> {
    state.stop_android_mirror()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(SettingsTransaction::default())
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
            let dependencies = dependencies::DependencyManager::new(
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
            get_dashboard,
            get_dependencies,
            install_dependencies,
            manage_dependencies,
            save_settings,
            start_airplay,
            stop_airplay,
            refresh_android_devices,
            start_android_mirror,
            stop_android_mirror
        ])
        .run(tauri::generate_context!())
        .expect("無法啟動 Neo AirPlay 桌面應用程式");
}

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

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn application_directories(
    app: &tauri::AppHandle,
    e2e_profile: Option<&std::path::Path>,
) -> Result<(std::path::PathBuf, std::path::PathBuf), tauri::Error> {
    if let Some(profile) = e2e_profile {
        return Ok((profile.join("config"), profile.join("logs")));
    }

    Ok((app.path().app_config_dir()?, app.path().app_log_dir()?))
}

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
