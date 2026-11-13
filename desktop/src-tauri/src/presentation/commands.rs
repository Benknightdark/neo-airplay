//! Tauri IPC 命令轉接層，負責接收前端請求並委派給應用層查詢與命令使用案例。

use crate::application::{commands, queries, DashboardSnapshot, SaveSettingsUseCase};
use crate::domain::{
    AndroidDevice, DependencyAction, DependencySnapshot, EngineSnapshot, ReceiverSettings,
};
use crate::infrastructure::{AppState, DependencyManager};
use crate::presentation::autostart::TauriAutostartGateway;
use tauri::{AppHandle, State};

/// Tauri 命令：查詢目前投影依賴工具的檢查與安裝狀態快照。
///
/// # 參數
/// - `state`: Tauri 託管的 `DependencyManager` 狀態參考。
///
/// # 回傳值
/// 成功回傳 `DependencySnapshot`；失敗回傳錯誤訊息字串。
#[tauri::command]
pub fn get_dependencies(state: State<'_, DependencyManager>) -> Result<DependencySnapshot, String> {
    queries::get_dependencies(state.inner()).map_err(Into::into)
}

/// Tauri 命令：讀取投影工具安裝或維護腳本產生的完整文字日誌。
///
/// # 參數
/// - `state`: Tauri 託管的 `DependencyManager` 狀態參考。
///
/// # 回傳值
/// 回傳安裝日誌全文或錯誤訊息字串。
#[tauri::command]
pub fn read_install_log(state: State<'_, DependencyManager>) -> Result<String, String> {
    queries::read_install_log(state.inner()).map_err(Into::into)
}

/// Tauri 命令：檢查並視需要啟動背景自動安裝缺失投影工具流程。
///
/// # 參數
/// - `state`: Tauri 託管的 `DependencyManager` 狀態參考。
///
/// # 回傳值
/// 回傳進入檢查／安裝狀態的 `DependencySnapshot` 或錯誤字串。
#[tauri::command]
pub fn install_dependencies(
    state: State<'_, DependencyManager>,
) -> Result<DependencySnapshot, String> {
    commands::install_dependencies(state.inner()).map_err(Into::into)
}

/// Tauri 命令：執行手動觸發的投影工具升級或重新安裝維護動作。
///
/// # 參數
/// - `app_state`: Tauri 託管的 `AppState`（用於維護前先停止運行中的投影服務）。
/// - `dependencies`: Tauri 託管的 `DependencyManager` 狀態參考。
/// - `action`: 前端指定的維護動作（`Upgrade` 或 `Reinstall`）。
///
/// # 回傳值
/// 回傳維護啟動後的 `DependencySnapshot` 或錯誤字串。
#[tauri::command]
pub fn manage_dependencies(
    app_state: State<'_, AppState>,
    dependencies: State<'_, DependencyManager>,
    action: DependencyAction,
) -> Result<DependencySnapshot, String> {
    commands::manage_dependencies(app_state.inner(), dependencies.inner(), action)
        .map_err(Into::into)
}

/// Tauri 命令：查詢儀表板畫面所需的設定、引擎狀態、Android 裝置與近期診斷日誌快照。
///
/// # 參數
/// - `state`: Tauri 託管的 `AppState` 狀態參考。
///
/// # 回傳值
/// 回傳組合後的 `DashboardSnapshot` 或錯誤字串。
#[tauri::command]
pub fn get_dashboard(state: State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    queries::get_dashboard(state.inner()).map_err(Into::into)
}

/// Tauri 命令：驗證、儲存接收器設定並同步作業系統開機／登入自啟設定。
///
/// # 參數
/// - `app`: Tauri 應用程式控制代碼，供建立 `TauriAutostartGateway`。
/// - `state`: Tauri 託管的 `AppState`。
/// - `use_case`: 序列化設定與自啟狀態交易的 `SaveSettingsUseCase`。
/// - `settings`: 前端表單送出的接收器設定內容。
///
/// # 回傳值
/// 成功時回傳正規化後的 `ReceiverSettings`；失敗時回傳錯誤字串。
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    use_case: State<'_, SaveSettingsUseCase>,
    settings: ReceiverSettings,
) -> Result<ReceiverSettings, String> {
    let autostart = TauriAutostartGateway::new(&app);
    use_case
        .execute(state.inner(), &autostart, settings)
        .map_err(Into::into)
}

/// Tauri 命令：在依賴工具未忙碌時啟動 AirPlay 接收器。
///
/// # 參數
/// - `state`: Tauri 託管的 `AppState`。
/// - `dependencies`: 用作引擎存取守衛的 `DependencyManager`。
///
/// # 回傳值
/// 回傳 AirPlay 引擎最新 `EngineSnapshot` 或錯誤字串。
#[tauri::command]
pub fn start_airplay(
    state: State<'_, AppState>,
    dependencies: State<'_, DependencyManager>,
) -> Result<EngineSnapshot, String> {
    commands::start_airplay(state.inner(), dependencies.inner()).map_err(Into::into)
}

/// Tauri 命令：停止目前運作中的 AirPlay 接收器。
///
/// # 參數
/// - `state`: Tauri 託管的 `AppState`。
///
/// # 回傳值
/// 回傳停止後的 `EngineSnapshot` 或錯誤字串。
#[tauri::command]
pub fn stop_airplay(state: State<'_, AppState>) -> Result<EngineSnapshot, String> {
    commands::stop_airplay(state.inner()).map_err(Into::into)
}

/// Tauri 非同步命令：在背景執行緒重新整理目前連接的 Android 裝置列表。
///
/// # 參數
/// - `state`: Tauri 託管的 `AppState`。
/// - `dependencies`: 用作引擎存取守衛的 `DependencyManager`。
///
/// # 回傳值
/// 回傳偵測到的 `AndroidDevice` 陣列或錯誤字串。
#[tauri::command(async)]
pub fn refresh_android_devices(
    state: State<'_, AppState>,
    dependencies: State<'_, DependencyManager>,
) -> Result<Vec<AndroidDevice>, String> {
    commands::refresh_android_devices(state.inner(), dependencies.inner()).map_err(Into::into)
}

/// Tauri 命令：對指定序號的已授權 Android 裝置啟動畫面投影。
///
/// # 參數
/// - `state`: Tauri 託管的 `AppState`。
/// - `dependencies`: 用作引擎存取守衛的 `DependencyManager`。
/// - `serial`: 目標 Android 裝置的 ADB 序號字串。
///
/// # 回傳值
/// 回傳 Android 引擎最新 `EngineSnapshot` 或錯誤字串。
#[tauri::command]
pub fn start_android_mirror(
    state: State<'_, AppState>,
    dependencies: State<'_, DependencyManager>,
    serial: String,
) -> Result<EngineSnapshot, String> {
    commands::start_android_mirror(state.inner(), dependencies.inner(), &serial).map_err(Into::into)
}

/// Tauri 命令：停止目前運作中的 Android 畫面投影。
///
/// # 參數
/// - `state`: Tauri 託管的 `AppState`。
///
/// # 回傳值
/// 回傳停止後的 `EngineSnapshot` 或錯誤字串。
#[tauri::command]
pub fn stop_android_mirror(state: State<'_, AppState>) -> Result<EngineSnapshot, String> {
    commands::stop_android_mirror(state.inner()).map_err(Into::into)
}
