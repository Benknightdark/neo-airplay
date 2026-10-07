#[cfg(target_os = "macos")]
use neo_core::AppLogger;
use neo_core::{EngineStatus, ReceiverSettings, SettingsStore};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppSnapshot, AppState, IpcReceiverSettings, SaveResult};

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    Ok(state.snapshot().await)
}

#[tauri::command]
pub async fn start_receiver(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EngineStatus, String> {
    #[cfg(target_os = "macos")]
    show_macos_airplay_conflict_warning(&app, &state).await;
    #[cfg(not(target_os = "macos"))]
    let _ = app;

    let settings = state.settings.read().await.clone();
    Ok(state.receiver.start(&settings).await)
}

#[cfg(target_os = "macos")]
async fn show_macos_airplay_conflict_warning(app: &AppHandle, state: &AppState) {
    use std::sync::atomic::Ordering;

    if state.e2e_profile
        || state
            .macos_airplay_warning_shown
            .swap(true, Ordering::AcqRel)
    {
        return;
    }

    let _ = crate::confirm_action(
        app,
        "macOS 內建的「隔空播放接收器」可能會占用 UxPlay 使用的連接埠。若 UxPlay 無法啟動，請到「系統設定 > 一般 > AirDrop 與接力」關閉「隔空播放接收器」，再試一次。".to_owned(),
        "知道了",
        "稍後",
    )
    .await;

    let marker = state
        .paths
        .settings_path
        .with_file_name("macos-airplay-warning-shown");
    if let Err(error) = tokio::fs::write(marker, b"shown\n").await {
        state
            .logger
            .write(
                neo_core::LogLevel::Warning,
                format!("無法儲存 macOS AirPlay 衝突提示狀態：{error}"),
            )
            .await;
    }
}

#[tauri::command]
pub async fn stop_receiver(state: State<'_, AppState>) -> Result<EngineStatus, String> {
    Ok(state.receiver.stop().await)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: IpcReceiverSettings,
) -> Result<SaveResult, String> {
    let previous = state.settings.read().await.clone();
    let saved = state
        .settings_store
        .save(&ReceiverSettings::from(settings))
        .await
        .map_err(|error| error.to_string())?;

    if !state.e2e_profile && previous.start_on_login != saved.start_on_login {
        let update_result = if saved.start_on_login {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(error) = update_result {
            let rollback = state.settings_store.save(&previous).await;
            return Err(match rollback {
                Ok(_) => format!("無法更新登入後自動啟動設定：{error}"),
                Err(rollback_error) => format!(
                    "無法更新登入後自動啟動設定：{error}；還原設定檔也失敗：{rollback_error}"
                ),
            });
        }
    }

    let apply_pending = state.receiver.status().state == neo_core::ReceiverState::Running
        && (previous.receiver_name != saved.receiver_name
            || previous.renderer != saved.renderer
            || previous.fullscreen != saved.fullscreen
            || previous.uxplay_path != saved.uxplay_path);
    *state.settings.write().await = saved.clone();

    Ok(SaveResult {
        settings: saved.into(),
        apply_pending,
        message: if apply_pending {
            "設定已儲存；重新啟動 AirPlay 接收器後套用。".to_owned()
        } else {
            "設定已儲存。".to_owned()
        },
    })
}

#[tauri::command]
pub async fn apply_settings(state: State<'_, AppState>) -> Result<EngineStatus, String> {
    let settings = state.settings.read().await.clone();
    if state.receiver.status().state == neo_core::ReceiverState::Running {
        state.receiver.stop().await;
    }
    Ok(state.receiver.start(&settings).await)
}

#[tauri::command]
pub async fn refresh_android_devices(
    state: State<'_, AppState>,
) -> Result<Vec<neo_core::AndroidDevice>, String> {
    Ok(state.android.refresh_devices().await)
}

#[tauri::command]
pub async fn start_android_mirror(
    state: State<'_, AppState>,
    serial: String,
) -> Result<EngineStatus, String> {
    let settings = state.settings.read().await.clone();
    Ok(state.android.start(&serial, &settings).await)
}

#[tauri::command]
pub async fn stop_android_mirror(state: State<'_, AppState>) -> Result<EngineStatus, String> {
    Ok(state.android.stop().await)
}

#[tauri::command]
pub fn open_log_directory(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    app.opener()
        .open_path(state.paths.log_directory.to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn hide_to_tray(app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "找不到主視窗。".to_owned())?
        .hide()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn exit_app(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let _ = tokio::join!(state.receiver.stop(), state.android.stop(),);
    app.exit(0);
    Ok(())
}
