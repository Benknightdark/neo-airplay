use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager};

pub fn install(app: &mut App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "開啟 Neo AirPlay", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start", "啟動接收器", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "停止接收器", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "結束", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &start, &stop, &exit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("the Tauri configuration declares a window icon");

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Neo AirPlay")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "start" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<crate::state::AppState>();
                    let settings = state.settings.read().await.clone();
                    state.receiver.start(&settings).await;
                });
            }
            "stop" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    app.state::<crate::state::AppState>().receiver.stop().await;
                });
            }
            "exit" => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<crate::state::AppState>();
                    let _ = tokio::join!(state.receiver.stop(), state.android.stop());
                    app_handle.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } | TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
