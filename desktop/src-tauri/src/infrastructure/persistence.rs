//! 本機設定檔（JSON）與應用程式診斷日誌（JSONL）的檔案系統持久化實作。

use crate::application::ports::{LogRepository, SettingsRepository};
use crate::domain::{
    push_recent, AppLogEntry, DomainError, DomainResult, LogLevel, ReceiverSettings,
    MAX_RECENT_LOGS,
};
use crate::infrastructure::process::{
    ensure_safe_directory, open_append_regular_file_secure, open_regular_file_secure,
    read_bounded_lines, read_bounded_regular_file, write_file_atomic_secure,
};
use jiff::{tz::TimeZone, Zoned};
use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 本機設定檔 `settings.json` 允許讀取的最大位元組數（64 KiB）。
pub const MAX_SETTINGS_FILE_BYTES: u64 = 64 * 1024;
/// 啟動載入當日 `.jsonl` 診斷記錄時最多從尾端讀取的位元組數（2 MiB）。
const MAX_DAILY_LOG_LOAD_BYTES: u64 = 2 * 1024 * 1024;
/// 單日診斷日誌檔案允許增長的最大位元組數（10 MiB），超過時僅保留尾端 `1 MiB`。
pub const MAX_DAILY_LOG_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// 單日診斷日誌超過大小上限時輪替保留的尾端位元組數（1 MiB）。
const MAX_DAILY_LOG_ROTATED_TAIL_BYTES: usize = 1024 * 1024;
/// 讀取單行診斷 JSONL 記錄允許的最大位元組數（16 KiB）。
const MAX_DAILY_LOG_LINE_BYTES: usize = 16 * 1024;

/// 以本機 JSON 檔案作為儲存後端的設定儲存庫實作。
pub struct JsonSettingsRepository {
    /// 設定檔（`settings.json`）在磁碟上的完整路徑。
    path: PathBuf,
}

impl JsonSettingsRepository {
    /// 建立指向指定路徑的 `JsonSettingsRepository`。
    ///
    /// # 參數
    /// - `path`: 設定 JSON 檔案路徑。
    ///
    /// # 回傳值
    /// 回傳新的 `JsonSettingsRepository` 實例。
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SettingsRepository for JsonSettingsRepository {
    /// 從設定檔讀取並正規化接收器設定；檔案不存在或格式損毀時回傳預設值。
    ///
    /// # 回傳值
    /// 回傳包裝為 `DomainResult` 的 `ReceiverSettings`。
    fn load(&self) -> DomainResult<ReceiverSettings> {
        Ok(load_settings(&self.path))
    }

    /// 以原子暫存檔置換方式將設定寫入磁碟。
    ///
    /// # 參數
    /// - `settings`: 欲保存的設定物件參考。
    ///
    /// # 回傳值
    /// 成功回傳 `Ok(())`；I/O 或序列化失敗時映射為 `DomainError::storage`。
    fn save(&self, settings: &ReceiverSettings) -> DomainResult<()> {
        write_settings(&self.path, settings).map_err(DomainError::storage)
    }
}

/// 從指定路徑安全載入 `ReceiverSettings`（拒絕符號連結並限制大小），失敗時退回預設設定並套用欄位清理規則。
///
/// # 參數
/// - `path`: 設定檔路徑。
///
/// # 回傳值
/// 回傳經過 `sanitized()` 正規化處理的設定實例。
pub(crate) fn load_settings(path: &Path) -> ReceiverSettings {
    read_bounded_regular_file(path, MAX_SETTINGS_FILE_BYTES)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ReceiverSettings>(&bytes).ok())
        .unwrap_or_default()
        .sanitized()
}

/// 透過唯一暫存檔原子更名寫入設定檔（Unix `0600`），拒絕符號連結並確保設定目錄安全。
///
/// # 參數
/// - `path`: 目標設定檔路徑。
/// - `settings`: 欲序列化的接收器設定。
///
/// # 回傳值
/// 成功回傳 `Ok(())`；目錄建立、序列化、寫入或更名失敗則回傳錯誤訊息字串。
pub(crate) fn write_settings(path: &Path, settings: &ReceiverSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "設定目錄無法解析。".to_owned())?;
    ensure_safe_directory(parent, true).map_err(|error| format!("無法建立設定目錄：{error}"))?;
    let serialized =
        serde_json::to_vec_pretty(settings).map_err(|error| format!("設定序列化失敗：{error}"))?;
    write_file_atomic_secure(path, &serialized, 0o600)
        .map_err(|error| format!("設定檔更新失敗：{error}"))
}

/// 負責將診斷日誌寫入每日 JSONL 檔案，並在記憶體中維護近期日誌環狀緩衝區。
#[derive(Clone)]
pub struct AppLogger {
    /// 存放每日 `.jsonl` 日誌檔的目錄路徑。
    log_dir: PathBuf,
    /// 執行緒安全的近期日誌記憶體快取。
    entries: Arc<Mutex<VecDeque<AppLogEntry>>>,
    /// 序列化磁碟日誌寫入與大小輪替的互斥鎖。
    disk_lock: Arc<Mutex<()>>,
}

impl AppLogger {
    /// 建立 `AppLogger`，並安全載入當日 `.jsonl` 檔尾端最多 `MAX_RECENT_LOGS` 筆紀錄。
    ///
    /// # 參數
    /// - `log_dir`: 日誌存放目錄路徑。
    ///
    /// # 回傳值
    /// 回傳初始化完成的 `AppLogger`。
    pub fn new(log_dir: PathBuf) -> Self {
        let log_path = log_dir.join(format!("{}.jsonl", Zoned::now().strftime("%Y-%m-%d")));
        let mut entries = VecDeque::new();
        if let Ok(mut file) = open_regular_file_secure(&log_path) {
            let file_len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
            let mut skip_first_partial = false;
            if file_len > MAX_DAILY_LOG_LOAD_BYTES
                && file
                    .seek(SeekFrom::End(-(MAX_DAILY_LOG_LOAD_BYTES as i64)))
                    .is_ok()
            {
                skip_first_partial = true;
            }
            read_bounded_lines(file, MAX_DAILY_LOG_LINE_BYTES, |line| {
                if skip_first_partial {
                    skip_first_partial = false;
                    return;
                }
                if let Ok(entry) = serde_json::from_str::<AppLogEntry>(&line) {
                    entries.push_back(entry);
                    while entries.len() > MAX_RECENT_LOGS {
                        entries.pop_front();
                    }
                }
            });
        }
        Self {
            log_dir,
            entries: Arc::new(Mutex::new(entries)),
            disk_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 建立一筆新日誌並同時寫入當日檔案與記憶體環狀緩衝區。
    ///
    /// # 參數
    /// - `level`: 日誌嚴重等級。
    /// - `message`: 原始日誌內容文字。
    pub fn write(&self, level: LogLevel, message: &str) {
        let entry = AppLogEntry::new(level, message);
        {
            let _guard = self.disk_lock.lock();
            append_log(&self.log_dir, &entry);
        }
        if let Ok(mut entries) = self.entries.lock() {
            push_recent(&mut entries, entry);
        }
    }

    /// 複製並回傳記憶體中的所有近期日誌項目。
    ///
    /// # 回傳值
    /// 回傳近期 `AppLogEntry` 向量；若鎖定失敗則回傳空向量。
    pub fn recent(&self) -> Vec<AppLogEntry> {
        self.entries
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl LogRepository for AppLogger {
    /// 透過 `LogRepository` 埠寫入一筆診斷日誌。
    ///
    /// # 參數
    /// - `level`: 日誌嚴重等級。
    /// - `message`: 紀錄內容文字。
    fn write(&self, level: LogLevel, message: &str) {
        AppLogger::write(self, level, message);
    }

    /// 透過 `LogRepository` 埠讀取近期診斷日誌清單。
    ///
    /// # 回傳值
    /// 回傳近期 `AppLogEntry` 清單。
    fn recent(&self) -> Vec<AppLogEntry> {
        AppLogger::recent(self)
    }
}

/// 將單筆 `AppLogEntry` 序列化為 JSON 後安全附加寫入當日 `.jsonl` 檔案末尾（拒絕符號連結並限制檔案大小上限）。
///
/// # 參數
/// - `log_dir`: 日誌存放目錄路徑。
/// - `entry`: 欲寫入磁碟的日誌項目參考。
fn append_log(log_dir: &Path, entry: &AppLogEntry) {
    if ensure_safe_directory(log_dir, true).is_err() {
        return;
    }
    let local_time = entry.timestamp.to_zoned(TimeZone::system());
    let path = log_dir.join(format!("{}.jsonl", local_time.strftime("%Y-%m-%d")));
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if meta.file_type().is_symlink() || !meta.is_file() {
            return;
        }
        if meta.len() >= MAX_DAILY_LOG_FILE_BYTES {
            let _ = rotate_daily_log_tail(&path);
        }
    }
    if let Ok(mut file) = open_append_regular_file_secure(&path, 0o600) {
        if let Ok(line) = serde_json::to_vec(entry) {
            let _ = file.write_all(&line);
            let _ = file.write_all(b"\n");
        }
    }
}

fn rotate_daily_log_tail(path: &Path) -> std::io::Result<()> {
    let mut file = open_regular_file_secure(path)?;
    let len = file.metadata()?.len();
    if len <= MAX_DAILY_LOG_ROTATED_TAIL_BYTES as u64 {
        return Ok(());
    }
    file.seek(SeekFrom::End(-(MAX_DAILY_LOG_ROTATED_TAIL_BYTES as i64)))?;
    let mut tail = Vec::with_capacity(MAX_DAILY_LOG_ROTATED_TAIL_BYTES);
    std::io::Read::take(file, MAX_DAILY_LOG_ROTATED_TAIL_BYTES as u64).read_to_end(&mut tail)?;
    let aligned = match tail.iter().position(|byte| *byte == b'\n') {
        Some(index) => &tail[index + 1..],
        None => &tail[..],
    };
    write_file_atomic_secure(path, aligned, 0o600)
}
