//! 應用程式診斷日誌型別與環狀緩衝區維護工具。

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 記憶體中最多保留的近期診斷日誌筆數上限。
pub const MAX_RECENT_LOGS: usize = 300;
/// 單筆診斷日誌訊息允許保留的最大位元組長度，避免過大記錄消耗記憶體或磁碟。
pub const MAX_LOG_MESSAGE_BYTES: usize = 4096;

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
    /// 紀錄產生的 RFC 3339 時間戳記。
    pub timestamp: Timestamp,
    /// 日誌嚴重等級。
    pub level: LogLevel,
    /// 經過換行正規化、控制字元過濾與長度上限修剪的日誌內容。
    pub message: String,
}

impl AppLogEntry {
    /// 建立一筆帶目前時間戳記的診斷日誌，壓平換行、移除控制／Bidi 字元並限制最大長度。
    ///
    /// # 參數
    /// - `level`: 日誌嚴重等級。
    /// - `message`: 原始日誌訊息文字。
    ///
    /// # 回傳值
    /// 回傳正規化後的 `AppLogEntry` 實例。
    pub fn new(level: LogLevel, message: &str) -> Self {
        let normalized: String = message
            .chars()
            .filter_map(|ch| match ch {
                '\r' | '\n' => Some(' '),
                ch if ch.is_control()
                    || matches!(
                        ch,
                        '\u{202A}'..='\u{202E}'
                            | '\u{2066}'..='\u{2069}'
                            | '\u{200E}'
                            | '\u{200F}'
                            | '\u{061C}'
                    ) =>
                {
                    None
                }
                other => Some(other),
            })
            .collect();
        let trimmed = normalized.trim();
        let bounded = if trimmed.len() <= MAX_LOG_MESSAGE_BYTES {
            trimmed.to_owned()
        } else {
            let mut end = MAX_LOG_MESSAGE_BYTES.saturating_sub("…".len());
            while end > 0 && !trimmed.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &trimmed[..end])
        };
        Self {
            timestamp: Timestamp::now(),
            level,
            message: bounded,
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
