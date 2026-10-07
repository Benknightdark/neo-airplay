use neo_core::EngineStatus;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

use crate::state::AppState;

pub fn forward_events(app: AppHandle, state: &AppState) {
    let mut receiver_status = state.receiver.subscribe();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if receiver_status.changed().await.is_err() {
                break;
            }
            emit_status(
                &app_handle,
                "receiver-status",
                receiver_status.borrow().clone(),
            );
        }
    });

    let mut android_status = state.android.subscribe();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if android_status.changed().await.is_err() {
                break;
            }
            emit_status(
                &app_handle,
                "android-status",
                android_status.borrow().clone(),
            );
        }
    });

    let mut logs = state.logger.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match logs.recv().await {
                Ok(entry) => {
                    let _ = app.emit("log-entry", entry);
                }
                Err(RecvError::Lagged(_)) => {
                    let _ = app.emit("log-entry-refresh-required", ());
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

fn emit_status(app: &AppHandle, event: &str, status: EngineStatus) {
    let _ = app.emit(event, status);
}
