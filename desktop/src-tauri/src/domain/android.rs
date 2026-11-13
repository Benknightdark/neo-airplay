//! Android 裝置實體定義與 `adb devices -l` 輸出解析邏輯。

use serde::{Deserialize, Serialize};

/// 透過 ADB 偵測到的 Android 裝置連線授權狀態。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AndroidDeviceStatus {
    /// 裝置已授權並可立即進行投影。
    Ready,
    /// 裝置已連線但尚未在手機端允許 USB 偵錯授權。
    Unauthorized,
    /// 裝置處於離線或無回應狀態。
    Offline,
    /// ADB 回傳了非預期的裝置狀態字串。
    Unknown,
}

/// 代表一部偵測到的 Android 實體或模擬器裝置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevice {
    /// ADB 識別用的裝置序號或連線位址。
    pub serial: String,
    /// 解析後的裝置型號名稱（空白替代底線）；若無型號則退回序號。
    pub model: String,
    /// 裝置目前的 ADB 狀態。
    pub status: AndroidDeviceStatus,
}

impl AndroidDevice {
    /// 判斷該裝置是否已進入可進行投影的就緒狀態。
    ///
    /// # 回傳值
    /// 當狀態為 `AndroidDeviceStatus::Ready` 時回傳 `true`。
    pub fn is_ready(&self) -> bool {
        self.status == AndroidDeviceStatus::Ready
    }

    /// 產生適合介面顯示的裝置名稱字串。
    ///
    /// # 回傳值
    /// 若型號為空或與序號相同則只回傳序號，否則回傳 `型號 (序號)` 格式。
    pub fn display_name(&self) -> String {
        if self.model.is_empty() || self.model == self.serial {
            self.serial.clone()
        } else {
            format!("{} ({})", self.model, self.serial)
        }
    }
}

/// 解析 `adb devices -l` 的標準輸出文字為結構化的 `AndroidDevice` 清單。
///
/// # 參數
/// - `output`: 執行 `adb devices -l` 所取得的原始標準輸出字串。
///
/// # 回傳值
/// 回傳過濾標題列與 daemon 訊息後的裝置列表。
pub fn parse_adb_devices(output: &str) -> Vec<AndroidDevice> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            let state = fields.next()?;
            if serial == "List" || serial.starts_with('*') {
                return None;
            }
            let status = match state {
                "device" => AndroidDeviceStatus::Ready,
                "unauthorized" => AndroidDeviceStatus::Unauthorized,
                "offline" => AndroidDeviceStatus::Offline,
                _ => AndroidDeviceStatus::Unknown,
            };
            let model = fields
                .find_map(|field| field.strip_prefix("model:"))
                .unwrap_or(serial)
                .replace('_', " ");
            Some(AndroidDevice {
                serial: serial.to_owned(),
                model,
                status,
            })
        })
        .collect()
}
