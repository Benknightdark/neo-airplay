//! 應用層命令型使用案例（Commands），協調設定異動、引擎啟停與工具維護業務流程。

use crate::application::ports::{
    AutostartGateway, DependencyRuntime, EngineAccessGuard, ReceiverRuntime,
};
use crate::domain::{
    AndroidDevice, DependencyAction, DependencySnapshot, DomainError, DomainResult, EngineSnapshot,
    EngineStatus, ReceiverSettings,
};
use std::sync::Mutex;

/// 儲存接收器設定並同步作業系統登入啟動狀態的交易型使用案例。
#[derive(Default)]
pub struct SaveSettingsUseCase {
    /// 序列化並保護設定與系統自啟狀態兩階段提交的一致性鎖。
    transaction_lock: Mutex<()>,
}

impl SaveSettingsUseCase {
    /// 驗證並保存接收器設定；若 `start_on_login` 異動且後續寫入失敗，會補償回滾系統自啟狀態。
    ///
    /// # 參數
    /// - `runtime`: 提供設定讀取、持久化寫入與日誌記錄的接收器執行期埠。
    /// - `autostart`: 負責同步作業系統開機／登入自啟設定的閘道埠。
    /// - `settings`: 前端傳入的新接收器設定。
    ///
    /// # 回傳值
    /// 成功時回傳經過領域正規化後的 `ReceiverSettings`；失敗或互斥鎖損毀時回傳 `DomainError`。
    pub fn execute(
        &self,
        runtime: &dyn ReceiverRuntime,
        autostart: &dyn AutostartGateway,
        settings: ReceiverSettings,
    ) -> DomainResult<ReceiverSettings> {
        let _transaction = self
            .transaction_lock
            .lock()
            .map_err(|_| DomainError::conflict("設定儲存狀態無法更新。"))?;
        let current = runtime.settings()?;
        let next = settings.sanitized();

        if current.start_on_login != next.start_on_login {
            autostart
                .set_enabled(next.start_on_login)
                .map_err(|error| {
                    DomainError::external_process(format!("無法更新登入啟動設定：{error}"))
                })?;
        }

        if let Err(error) = runtime.save_settings(next.clone()) {
            if current.start_on_login != next.start_on_login {
                let _ = autostart.set_enabled(current.start_on_login);
            }
            return Err(error);
        }

        runtime.log_info("設定已儲存。");
        Ok(next)
    }
}

/// 在依賴工具未忙碌的條件下啟動 AirPlay 接收器。
///
/// # 參數
/// - `runtime`: 接收器執行期控制埠。
/// - `guard`: 確保依賴工具安裝或維護未進行中的存取守衛。
///
/// # 回傳值
/// 回傳 AirPlay 引擎最新 `EngineSnapshot`；若守衛拒絕或啟動失敗則回傳 `DomainError`。
pub fn start_airplay(
    runtime: &dyn ReceiverRuntime,
    guard: &dyn EngineAccessGuard,
) -> DomainResult<EngineSnapshot> {
    let mut outcome = None;
    guard.execute_if_available(&mut || {
        outcome = Some(runtime.start_airplay());
    })?;
    outcome.unwrap_or_else(|| Err(DomainError::conflict("無法啟動 AirPlay。")))
}

/// 停止運作中的 AirPlay 接收器。
///
/// # 參數
/// - `runtime`: 接收器執行期控制埠。
///
/// # 回傳值
/// 回傳停止操作後的 `EngineSnapshot`。
pub fn stop_airplay(runtime: &dyn ReceiverRuntime) -> DomainResult<EngineSnapshot> {
    runtime.stop_airplay()
}

/// 在依賴工具未忙碌的條件下透過 ADB 重新整理可連線的 Android 裝置列表。
///
/// # 參數
/// - `runtime`: 接收器執行期控制埠。
/// - `guard`: 確保依賴工具維護未進行中的存取守衛。
///
/// # 回傳值
/// 回傳目前偵測到的 `AndroidDevice` 清單；若遭阻擋或查詢異常則回傳 `DomainError`。
pub fn refresh_android_devices(
    runtime: &dyn ReceiverRuntime,
    guard: &dyn EngineAccessGuard,
) -> DomainResult<Vec<AndroidDevice>> {
    let mut outcome = None;
    guard.execute_if_available(&mut || {
        outcome = Some(runtime.refresh_android_devices());
    })?;
    outcome.unwrap_or_else(|| Err(DomainError::conflict("無法重新整理 Android 裝置。")))
}

/// 檢查裝置序號非空並在依賴工具未忙碌時啟動指定 Android 裝置的畫面投影。
///
/// # 參數
/// - `runtime`: 接收器執行期控制埠。
/// - `guard`: 確保依賴工具維護未進行中的存取守衛。
/// - `serial`: 欲投影裝置的 ADB 序號。
///
/// # 回傳值
/// 回傳 Android 引擎最新 `EngineSnapshot`；序號為空或啟動失敗時回傳 `DomainError`。
pub fn start_android_mirror(
    runtime: &dyn ReceiverRuntime,
    guard: &dyn EngineAccessGuard,
    serial: &str,
) -> DomainResult<EngineSnapshot> {
    if serial.trim().is_empty() {
        return Err(DomainError::validation("請先選擇 Android 裝置。"));
    }
    let mut outcome = None;
    guard.execute_if_available(&mut || {
        outcome = Some(runtime.start_android_mirror(serial));
    })?;
    outcome.unwrap_or_else(|| Err(DomainError::conflict("無法啟動 Android 投影。")))
}

/// 停止運作中的 Android 畫面投影引擎。
///
/// # 參數
/// - `runtime`: 接收器執行期控制埠。
///
/// # 回傳值
/// 回傳停止操作後的 `EngineSnapshot`。
pub fn stop_android_mirror(runtime: &dyn ReceiverRuntime) -> DomainResult<EngineSnapshot> {
    runtime.stop_android_mirror()
}

/// 觸發投影依賴工具的自動檢查與缺失項目安裝。
///
/// # 參數
/// - `dependencies`: 依賴工具管理執行期埠。
///
/// # 回傳值
/// 回傳進入檢查或維護狀態的 `DependencySnapshot`。
pub fn install_dependencies(
    dependencies: &dyn DependencyRuntime,
) -> DomainResult<DependencySnapshot> {
    dependencies.start_install()
}

/// 執行依賴工具升級或重新安裝維護流程，並在前置步驟先停止所有作用中的投影服務。
///
/// # 參數
/// - `runtime`: 用於維護前停止 AirPlay 與 Android 投影的執行期埠。
/// - `dependencies`: 依賴工具管理執行期埠。
/// - `action`: 使用者請求的維護動作（升級或重裝）。
///
/// # 回傳值
/// 成功停止服務並啟動維護流程後回傳 `DependencySnapshot`；任一步驟失敗則回傳 `DomainError`。
pub fn manage_dependencies(
    runtime: &dyn ReceiverRuntime,
    dependencies: &dyn DependencyRuntime,
    action: DependencyAction,
) -> DomainResult<DependencySnapshot> {
    dependencies.manage(action, &mut || {
        let airplay = runtime
            .stop_airplay()
            .map_err(|error| DomainError::external_process(format!("無法停止 AirPlay：{error}")))?;
        let android = runtime.stop_android_mirror().map_err(|error| {
            DomainError::external_process(format!("無法停止 Android 投影：{error}"))
        })?;
        if airplay.status == EngineStatus::Faulted || android.status == EngineStatus::Faulted {
            return Err(DomainError::external_process(format!(
                "停止投影服務失敗，工具檔案未移除。{} {}",
                airplay.message, android.message
            )));
        }
        Ok(())
    })
}
