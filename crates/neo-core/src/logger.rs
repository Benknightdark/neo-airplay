use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::Local;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex as AsyncMutex, broadcast};

use crate::{AppLogEntry, LogLevel};

pub const LOG_SNAPSHOT_CAPACITY: usize = 300;
const LOG_EVENT_CAPACITY: usize = 64;

#[async_trait]
pub trait AppLogger: Send + Sync {
    async fn write(&self, level: LogLevel, message: String);
}

pub struct FileAppLogger {
    log_directory: PathBuf,
    write_lock: AsyncMutex<()>,
    entries: Mutex<VecDeque<AppLogEntry>>,
    events: broadcast::Sender<AppLogEntry>,
}

impl FileAppLogger {
    pub fn new(log_directory: impl Into<PathBuf>) -> Self {
        let (events, _) = broadcast::channel(LOG_EVENT_CAPACITY);
        Self {
            log_directory: log_directory.into(),
            write_lock: AsyncMutex::new(()),
            entries: Mutex::new(VecDeque::with_capacity(LOG_SNAPSHOT_CAPACITY)),
            events,
        }
    }

    pub fn log_directory(&self) -> &Path {
        &self.log_directory
    }

    pub fn snapshot(&self) -> Vec<AppLogEntry> {
        lock_unpoisoned(&self.entries).iter().cloned().collect()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppLogEntry> {
        self.events.subscribe()
    }

    async fn append_to_file(&self, entry: &AppLogEntry) {
        if tokio::fs::create_dir_all(&self.log_directory)
            .await
            .is_err()
        {
            return;
        }

        let path = self
            .log_directory
            .join(format!("{}.log", entry.timestamp.format("%Y-%m-%d")));
        let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
        else {
            return;
        };

        let safe_message = entry.message.replace(['\r', '\n'], " ");
        let line = format!(
            "[{}] [{}] {}\n",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f %:z"),
            entry.level.as_str(),
            safe_message
        );
        if file.write_all(line.as_bytes()).await.is_ok() {
            let _ = file.flush().await;
        }
    }
}

#[async_trait]
impl AppLogger for FileAppLogger {
    async fn write(&self, level: LogLevel, message: String) {
        let _write_guard = self.write_lock.lock().await;
        let entry = AppLogEntry {
            timestamp: Local::now().fixed_offset(),
            level,
            message: message.trim().to_owned(),
        };

        self.append_to_file(&entry).await;
        {
            let mut entries = lock_unpoisoned(&self.entries);
            if entries.len() == LOG_SNAPSHOT_CAPACITY {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }
        let _ = self.events.send(entry);
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::TryRecvError;

    use super::{AppLogger, FileAppLogger, LOG_SNAPSHOT_CAPACITY};
    use crate::LogLevel;

    #[tokio::test]
    async fn writes_one_safe_line_and_publishes_entry() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = FileAppLogger::new(temp.path());
        let mut events = logger.subscribe();

        logger
            .write(LogLevel::Information, " first\r\nsecond ".to_owned())
            .await;

        let entry = events.try_recv().expect("entry should be published");
        assert_eq!(entry.message, "first\r\nsecond");
        let file = std::fs::read_to_string(
            std::fs::read_dir(logger.log_directory())
                .expect("log directory should exist")
                .next()
                .expect("log file should exist")
                .expect("directory entry should be readable")
                .path(),
        )
        .expect("log file should be readable");
        assert!(file.contains("first  second"));
        assert!(!file.contains("first\r\nsecond"));
    }

    #[tokio::test]
    async fn snapshot_keeps_only_the_most_recent_three_hundred_entries() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = FileAppLogger::new(temp.path());

        for index in 0..=LOG_SNAPSHOT_CAPACITY {
            logger.write(LogLevel::Information, index.to_string()).await;
        }

        let snapshot = logger.snapshot();
        assert_eq!(snapshot.len(), LOG_SNAPSHOT_CAPACITY);
        assert_eq!(
            snapshot.first().map(|entry| entry.message.as_str()),
            Some("1")
        );
        assert_eq!(
            snapshot.last().map(|entry| entry.message.as_str()),
            Some("300")
        );
    }

    #[tokio::test]
    async fn file_failure_does_not_suppress_memory_or_event_log() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let file_path = temp.path().join("not-a-directory");
        std::fs::write(&file_path, "file").expect("fixture should be written");
        let logger = FileAppLogger::new(&file_path);
        let mut events = logger.subscribe();

        logger
            .write(LogLevel::Warning, "available in memory".to_owned())
            .await;

        assert_eq!(logger.snapshot().len(), 1);
        assert!(events.try_recv().is_ok());
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }
}
