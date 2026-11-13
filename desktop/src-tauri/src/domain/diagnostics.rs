//! 應用程式診斷日誌型別與環狀緩衝區維護工具。

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 記憶體中最多保留的近期診斷日誌筆數上限。
pub const MAX_RECENT_LOGS: usize = 300;

/// 診斷日誌的重要程度等級。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// 一般資訊與狀態變更紀錄。
    Information,
    /// 非致命的異常提示或降級警告。
    Warning,
    /// 操作失敗、程序錯誤或崩潰紀錄。
    Error,
}

/// 單筆應用程式診斷日誌項目，供診斷頁面即時顯示與問題排查。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppLogEntry {
    /// 紀錄產生的本地時間戳記。
    pub timestamp: DateTime<Local>,
    /// 日誌嚴重等級。
    pub level: LogLevel,
    /// 經過換行正規化與空白修剪的日誌內容。
    pub message: String,
}

impl AppLogEntry {
    /// 建立一筆帶目前時間戳記的診斷日誌，並將換行字元壓平為單行空白。
    ///
    /// # 參數
    /// - `level`: 日誌嚴重等級。
    /// - `message`: 原始日誌訊息文字。
    ///
    /// # 回傳值
    /// 回傳正規化後的 `AppLogEntry` 實例。
    pub fn new(level: LogLevel, message: &str) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            message: message.replace(['\r', '\n'], " ").trim().to_owned(),
        }
    }
}

/// 將新日誌寫入近期日誌佇列尾端，超過 `MAX_RECENT_LOGS` 上限時移除最舊紀錄。
///
/// # 參數
/// - `entries`: 可變動的近期日誌雙端佇列。
/// - `entry`: 欲新增的日誌項目。
pub fn push_recent(entries: &mut VecDeque<AppLogEntry>, entry: AppLogEntry) {
    entries.push_back(entry);
    while entries.len() > MAX_RECENT_LOGS {
        entries.pop_front();
    }
}
