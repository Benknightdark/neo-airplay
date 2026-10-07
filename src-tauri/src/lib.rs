mod commands;
#[cfg(windows)]
mod elevated;
mod events;
mod startup;
mod state;
mod tray;

#[cfg(windows)]
use neo_core::{AppLogger, LogLevel};
use neo_core::{MdnsInspector, StartupCheck, SystemMdnsInspector};
use startup::CliArgs;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

pub fn run() {
    let args = CliArgs::parse(std::env::args_os());
    #[cfg(windows)]
    if args.elevated_engine_setup {
        std::process::exit(elevated::run(&args));
    }
    let e2e_profile = args.e2e_profile.is_some();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Neo AirPlay")
                .args(["--background", "--start"])
                .build(),
        );

    #[cfg(feature = "e2e")]
    {
        builder = builder
            .plugin(tauri_plugin_wdio::init())
            .plugin(tauri_plugin_wdio_webdriver::init());
    }

    if !e2e_profile {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _working_directory| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            },
        ));
    }

    let launch_args = args.clone();
    let app = builder
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::start_receiver,
            commands::stop_receiver,
            commands::save_settings,
            commands::apply_settings,
            commands::refresh_android_devices,
            commands::start_android_mirror,
            commands::stop_android_mirror,
            commands::open_log_directory,
            commands::hide_to_tray,
            commands::exit_app
        ])
        .setup(move |app| {
            let state = state::AppState::new(app.handle(), launch_args.e2e_profile.as_deref())?;
            #[cfg(windows)]
            match startup::remove_legacy_windows_autostart() {
                Ok(true) => tauri::async_runtime::block_on(state.logger.write(
                    LogLevel::Information,
                    "已移除舊版 Windows 自動啟動項目。".to_owned(),
                )),
                Ok(false) => {}
                Err(error) => {
                    tauri::async_runtime::block_on(state.logger.write(LogLevel::Warning, error))
                }
            }
            app.manage(state);
            events::forward_events(app.handle().clone(), &app.state::<state::AppState>());
            tray::install(app)?;

            let app_handle = app.handle().clone();
            let start_after_setup = launch_args.start;
            tauri::async_runtime::spawn(async move {
                prepare_runtime(app_handle, start_after_setup).await;
            });

            if launch_args.background
                && !e2e_profile
                && let Some(window) = app.get_webview_window("main")
            {
                window.hide()?;
            }

            Ok(())
        })
        .on_window_event(move |window, event| {
            if !e2e_profile
                && window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Neo AirPlay");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. }) {
            let state = app_handle.state::<state::AppState>();
            tauri::async_runtime::block_on(async {
                let _ = tokio::join!(state.receiver.stop(), state.android.stop());
            });
        }
    });
}

async fn prepare_runtime(app: tauri::AppHandle, start_after_setup: bool) {
    let (e2e_profile, bootstrapper, package_error, receiver) = {
        let state = app.state::<state::AppState>();
        (
            state.e2e_profile,
            state.engine_package_bootstrapper.clone(),
            state.engine_package_error.clone(),
            state.receiver.clone(),
        )
    };

    let (mdns_ready, mut messages) = if e2e_profile {
        (true, Vec::new())
    } else {
        check_mdns(&app).await
    };

    let package_ready = if e2e_profile {
        messages.push("E2E 設定已略過引擎下載。".to_owned());
        true
    } else if let Some(error) = package_error {
        messages.push(error);
        false
    } else if let Some(bootstrapper) = bootstrapper {
        match bootstrapper.prepare().await {
            Ok(result) => {
                messages.push(result.message);
                result.is_ready
            }
            Err(error) => {
                messages.push(format!("引擎套件準備失敗：{error}"));
                false
            }
        }
    } else {
        messages.push("引擎套件準備器未設定。".to_owned());
        false
    };

    if messages.is_empty() {
        messages.push("已載入設定與引擎狀態。".to_owned());
    }
    let status = state::EngineSetupStatus {
        message: messages.join(" "),
        in_progress: false,
    };
    {
        let state = app.state::<state::AppState>();
        *state.engine_setup.write().await = status.clone();
    }
    let _ = app.emit("engine-setup-status", status);

    if start_after_setup && mdns_ready && package_ready {
        let settings = {
            let state = app.state::<state::AppState>();
            state.settings.read().await.clone()
        };
        receiver.start(&settings).await;
    }
}

async fn check_mdns(app: &tauri::AppHandle) -> (bool, Vec<String>) {
    #[cfg(not(windows))]
    let _ = app;
    match SystemMdnsInspector.startup_check() {
        StartupCheck::Ready => (true, Vec::new()),
        StartupCheck::NeedsElevatedRepair { reason } => {
            #[cfg(windows)]
            {
                if confirm_action(
                    app,
                    format!(
                        "{reason}\n\nNeo AirPlay 會使用已安裝的 Bonjour 檔案修復 Service，接著顯示 Windows 系統管理員權限提示。是否繼續？"
                    ),
                    "修復 Bonjour",
                    "稍後",
                )
                .await
                {
                    match tauri::async_runtime::spawn_blocking(elevated::launch_bonjour_repair)
                        .await
                    {
                        Ok(Ok(())) => match SystemMdnsInspector.startup_check() {
                            StartupCheck::Ready => (true, vec!["Bonjour Service 已修復。".to_owned()]),
                            StartupCheck::NeedsElevatedRepair { reason }
                            | StartupCheck::Missing {
                                problem: neo_core::MdnsProblem { message: reason, .. },
                            } => (false, vec![format!("Bonjour Service 尚未就緒：{reason}")]),
                        },
                        Ok(Err(error)) => {
                            (false, vec![format!("Bonjour Service 修復未完成：{error}")])
                        }
                        Err(error) => {
                            (false, vec![format!("無法等待 Bonjour 修復程序：{error}")])
                        }
                    }
                } else {
                    (false, vec![format!("{reason} 使用者略過修復。")])
                }
            }
            #[cfg(not(windows))]
            {
                (false, vec![reason])
            }
        }
        StartupCheck::Missing { problem } => {
            #[cfg(windows)]
            if let Some(help_url) = problem.help_url {
                if confirm_action(
                    app,
                    format!("{}\n\n要開啟 Apple 支援頁嗎？", problem.message),
                    "需要 Bonjour",
                    "稍後",
                )
                .await
                {
                    use tauri_plugin_opener::OpenerExt;
                    if let Err(error) = app.opener().open_url(help_url, None::<String>) {
                        return (
                            false,
                            vec![format!(
                                "{} 無法開啟 Apple 支援頁：{error}",
                                problem.message
                            )],
                        );
                    }
                }
            }
            (false, vec![problem.message])
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
async fn confirm_action(app: &tauri::AppHandle, message: String, yes: &str, no: &str) -> bool {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(message)
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            yes.to_owned(),
            no.to_owned(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });
    receiver.await.unwrap_or(false)
}
