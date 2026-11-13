//! 透過 Tauri Autostart 外掛實作 `AutostartGateway` 應用層輸出埠。

use crate::application::AutostartGateway;
use crate::domain::{DomainError, DomainResult};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// 封裝 Tauri `AppHandle` 的系統開機／登入自動啟動閘道轉接器。
pub struct TauriAutostartGateway<'a> {
    /// Tauri 應用程式執行控制代碼參考。
    app: &'a AppHandle,
}

impl<'a> TauriAutostartGateway<'a> {
    /// 建立連結至指定 `AppHandle` 的 `TauriAutostartGateway`。
    ///
    /// # 參數
    /// - `app`: Tauri 應用程式控制代碼參考。
    ///
    /// # 回傳值
    /// 回傳新的 `TauriAutostartGateway` 實例。
    pub fn new(app: &'a AppHandle) -> Self {
        Self { app }
    }
}

impl AutostartGateway for TauriAutostartGateway<'_> {
    /// 呼叫 Tauri autolaunch 外掛啟用或停用作業系統登入自動啟動。
    ///
    /// # 參數
    /// - `enabled`: `true` 代表啟用登入啟動，`false` 代表停用。
    ///
    /// # 回傳值
    /// 套用成功回傳 `Ok(())`；失敗時映射為 `DomainError::external_process`。
    fn set_enabled(&self, enabled: bool) -> DomainResult<()> {
        let manager = self.app.autolaunch();
        let result = if enabled {
            manager.enable()
        } else {
            manager.disable()
        };
        result.map_err(|error| DomainError::external_process(error.to_string()))
    }
}
