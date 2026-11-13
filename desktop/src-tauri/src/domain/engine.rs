//! AirPlay 與 Android 投影引擎狀態模型、mDNS 廣播驗證及程序結束狀態判斷。

use crate::domain::diagnostics::LogLevel;
use serde::{Deserialize, Serialize};
use std::process::ExitStatus;
use std::time::Duration;

/// AirPlay（UxPlay）啟動後等待完成區網 mDNS 註冊的最長逾時時間。
pub const AIRPLAY_READY_TIMEOUT: Duration = Duration::from_secs(90);

/// 區分目前管理的投影引擎種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    /// 負責接收 Apple AirPlay 鏡像的 UxPlay 引擎。
    AirPlay,
    /// 負責接收 Android 裝置畫面的 scrcpy 引擎。
    Android,
}

impl EngineKind {
    /// 取得投影引擎的人類可讀顯示名稱。
    ///
    /// # 回傳值
    /// 回傳 `"UxPlay"` 或 `"scrcpy"`。
    pub fn display_name(self) -> &'static str {
        match self {
            Self::AirPlay => "UxPlay",
            Self::Android => "scrcpy",
        }
    }

    /// 取得引擎對應的可執行檔基本檔名（不含副檔名）。
    ///
    /// # 回傳值
    /// 回傳 `"uxplay"` 或 `"scrcpy"`。
    pub fn executable_name(self) -> &'static str {
        match self {
            Self::AirPlay => "uxplay",
            Self::Android => "scrcpy",
        }
    }

    /// 判斷此引擎種類是否為 Android 投影引擎。
    ///
    /// # 回傳值
    /// 若為 `EngineKind::Android` 則回傳 `true`。
    pub fn is_android(self) -> bool {
        matches!(self, Self::Android)
    }

    /// 取得引擎尚未啟動時的初始狀態描述訊息。
    ///
    /// # 回傳值
    /// 回傳對應引擎的預設待命說明文字。
    pub fn not_started_message(self) -> &'static str {
        match self {
            Self::AirPlay => "AirPlay 接收器尚未啟動。",
            Self::Android => "Android 投影尚未啟動。",
        }
    }

    /// 取得引擎正常停止後的狀態描述訊息。
    ///
    /// # 回傳值
    /// 回傳對應引擎的已停止說明文字。
    pub fn stopped_message(self) -> &'static str {
        match self {
            Self::AirPlay => "AirPlay 接收器已停止。",
            Self::Android => "Android 投影已停止。",
        }
    }

    /// 取得找不到對應外部執行檔時的錯誤提示訊息。
    ///
    /// # 回傳值
    /// 回傳引導使用者安裝缺少工具的提示字串。
    pub fn missing_dependency_message(self) -> &'static str {
        match self {
            Self::AirPlay => "找不到 UxPlay。請安裝 UxPlay 及其 GStreamer 執行環境。",
            Self::Android => "找不到 scrcpy。請安裝 scrcpy 與 Android Platform Tools。",
        }
    }
}

/// 單一投影引擎目前的執行生命週期狀態。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EngineStatus {
    /// 已停止運作，未持有作用中子程序。
    Stopped,
    /// 正在初始化或等待廣播就緒中。
    Starting,
    /// 已就緒並穩定執行中。
    Running,
    /// 正在送出停止訊號並回收子程序中。
    Stopping,
    /// 啟動失敗或執行期間非預期異常結束。
    Faulted,
    /// 系統找不到所需的外部執行檔或執行環境依賴。
    DependencyMissing,
}

/// 投影引擎在特定時間點的狀態快照，用於前後端狀態同步。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngineSnapshot {
    /// 引擎目前生命週期狀態。
    pub status: EngineStatus,
    /// 對應此狀態的說明或錯誤訊息。
    pub message: String,
    /// 作用中子程序的作業系統行程識別碼（PID），無程序時為 `None`。
    pub process_id: Option<u32>,
}

impl EngineSnapshot {
    /// 建立已停止狀態的 `EngineSnapshot`。
    ///
    /// # 參數
    /// - `message`: 顯示於介面與日誌的停止說明文字。
    ///
    /// # 回傳值
    /// 回傳 `status` 為 `EngineStatus::Stopped` 且 `process_id` 為 `None` 的快照。
    pub fn stopped(message: impl Into<String>) -> Self {
        Self {
            status: EngineStatus::Stopped,
            message: message.into(),
            process_id: None,
        }
    }

    /// 建立異常失敗狀態的 `EngineSnapshot`。
    ///
    /// # 參數
    /// - `message`: 說明錯誤或異常結束原因的訊息。
    /// - `process_id`: 若仍需標記相關子程序 PID 則傳入 `Some(pid)`，否則為 `None`。
    ///
    /// # 回傳值
    /// 回傳 `status` 為 `EngineStatus::Faulted` 的快照。
    pub fn faulted(message: impl Into<String>, process_id: Option<u32>) -> Self {
        Self {
            status: EngineStatus::Faulted,
            message: message.into(),
            process_id,
        }
    }

    /// 判斷快照是否正處於啟動中狀態。
    ///
    /// # 回傳值
    /// 若為 `EngineStatus::Starting` 則回傳 `true`。
    pub fn is_starting(&self) -> bool {
        self.status == EngineStatus::Starting
    }

    /// 判斷快照是否正處於執行中狀態。
    ///
    /// # 回傳值
    /// 若為 `EngineStatus::Running` 則回傳 `true`。
    pub fn is_running(&self) -> bool {
        self.status == EngineStatus::Running
    }
}

/// 檢查 macOS `dns-sd -B` 或 mDNS 輸出列是否公告了指定名稱的 AirPlay 服務實例。
///
/// # 參數
/// - `line`: 來自 Bonjour／mDNS 探索指令的單行輸出。
/// - `expected_name`: 預期比對的接收器廣播名稱。
///
/// # 回傳值
/// 若該行包含 `Add` 動作、`_airplay._tcp` 服務類型且實例名稱完全吻合，則回傳 `true`。
pub fn bonjour_instance_matches(line: &str, expected_name: &str) -> bool {
    let Some((prefix, suffix)) = line.split_once("_airplay._tcp") else {
        return false;
    };
    if !prefix.split_whitespace().any(|field| field == "Add") {
        return false;
    }
    let Some(boundary) = suffix.chars().next() else {
        return false;
    };
    if boundary != '.' && !boundary.is_whitespace() {
        return false;
    }
    suffix.strip_prefix('.').unwrap_or(suffix).trim() == expected_name
}

/// 依據子程序結束代碼推導引擎結束後的狀態、提示訊息與日誌等級。
///
/// # 參數
/// - `kind`: 結束的子程序所屬投影引擎種類。
/// - `exit_status`: 作業系統回報的子程序結束狀態。
///
/// # 回傳值
/// 回傳 `(EngineStatus, String, LogLevel)` 三元組；成功結束為 `Stopped`，非零結束則為 `Faulted`。
pub fn exit_snapshot(
    kind: EngineKind,
    exit_status: ExitStatus,
) -> (EngineStatus, String, LogLevel) {
    if exit_status.success() {
        (
            EngineStatus::Stopped,
            kind.stopped_message().to_owned(),
            LogLevel::Information,
        )
    } else {
        (
            EngineStatus::Faulted,
            format!("投影引擎已結束，結束代碼為 {exit_status}。"),
            LogLevel::Warning,
        )
    }
}
