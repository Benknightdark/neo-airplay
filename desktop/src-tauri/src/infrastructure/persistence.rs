//! 本機設定檔（JSON）與應用程式診斷日誌（JSONL）的檔案系統持久化實作。

use crate::application::ports::{LogRepository, SettingsRepository};
use crate::domain::{
    push_recent, AppLogEntry, DomainError, DomainResult, LogLevel, ReceiverSettings,
    MAX_RECENT_LOGS,
};
use chrono::Local;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// 從指定路徑載入 `ReceiverSettings`，失敗時使用預設設定並套用欄位清理規則。
///
/// # 參數
/// - `path`: 設定檔路徑。
///
/// # 回傳值
/// 回傳經過 `sanitized()` 正規化處理的設定實例。
pub(crate) fn load_settings(path: &Path) -> ReceiverSettings {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ReceiverSettings>(&bytes).ok())
        .unwrap_or_default()
        .sanitized()
}

/// 透過先寫入 `.json.tmp` 再 `rename` 的方式安全寫入設定檔，避免寫到一半毀損原檔。
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
    fs::create_dir_all(parent).map_err(|error| format!("無法建立設定目錄：{error}"))?;
    let temporary_path = path.with_extension("json.tmp");
    let serialized =
        serde_json::to_vec_pretty(settings).map_err(|error| format!("設定序列化失敗：{error}"))?;
    fs::write(&temporary_path, serialized).map_err(|error| format!("設定寫入失敗：{error}"))?;
    fs::rename(&temporary_path, path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        format!("設定檔更新失敗：{error}")
    })
}

/// 負責將診斷日誌寫入每日 JSONL 檔案，並在記憶體中維護近期日誌環狀緩衝區。
#[derive(Clone)]
pub struct AppLogger {
    /// 存放每日 `.jsonl` 日誌檔的目錄路徑。
    log_dir: PathBuf,
    /// 執行緒安全的近期日誌記憶體快取。
    entries: Arc<Mutex<VecDeque<AppLogEntry>>>,
}

impl AppLogger {
    /// 建立 `AppLogger`，並預先載入當日 `.jsonl` 檔中最後最多 `MAX_RECENT_LOGS` 筆紀錄。
    ///
    /// # 參數
    /// - `log_dir`: 日誌存放目錄路徑。
    ///
    /// # 回傳值
    /// 回傳初始化完成的 `AppLogger`。
    pub fn new(log_dir: PathBuf) -> Self {
        let log_path = log_dir.join(format!("{}.jsonl", Local::now().format("%Y-%m-%d")));
        let mut entries = VecDeque::new();
        if let Ok(file) = fs::File::open(log_path) {
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if let Ok(entry) = serde_json::from_str::<AppLogEntry>(&line) {
                    entries.push_back(entry);
                    while entries.len() > MAX_RECENT_LOGS {
                        entries.pop_front();
                    }
                }
            }
        }
        Self {
            log_dir,
            entries: Arc::new(Mutex::new(entries)),
        }
    }

    /// 建立一筆新日誌並同時寫入當日檔案與記憶體環狀緩衝區。
    ///
    /// # 參數
    /// - `level`: 日誌嚴重等級。
    /// - `message`: 原始日誌內容文字。
    pub fn write(&self, level: LogLevel, message: &str) {
        let entry = AppLogEntry::new(level, message);
        append_log(&self.log_dir, &entry);
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

/// 將單筆 `AppLogEntry` 序列化為 JSON 後附加寫入當日 `.jsonl` 檔案末尾。
///
/// # 參數
/// - `log_dir`: 日誌存放目錄路徑。
/// - `entry`: 欲寫入磁碟的日誌項目參考。
fn append_log(log_dir: &Path, entry: &AppLogEntry) {
    let path = log_dir.join(format!("{}.jsonl", entry.timestamp.format("%Y-%m-%d")));
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_vec(entry) {
            let _ = file.write_all(&line);
            let _ = file.write_all(b"\n");
        }
    }
}
