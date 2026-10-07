use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::{AppLogger, LogLevel, Platform, ReceiverSettings};

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("settings serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[async_trait]
pub trait SettingsStore: Send + Sync {
    async fn load(&self) -> ReceiverSettings;
    async fn save(&self, settings: &ReceiverSettings) -> Result<ReceiverSettings, SettingsError>;
}

pub struct JsonSettingsStore {
    path: PathBuf,
    logger: Arc<dyn AppLogger>,
    platform: Platform,
    write_lock: Mutex<()>,
}

impl JsonSettingsStore {
    pub fn new(path: impl Into<PathBuf>, logger: Arc<dyn AppLogger>, platform: Platform) -> Self {
        Self {
            path: path.into(),
            logger,
            platform,
            write_lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn sanitize(&self, mut settings: ReceiverSettings) -> ReceiverSettings {
        settings.receiver_name = if settings.receiver_name.trim().is_empty() {
            "Neo AirPlay".to_owned()
        } else {
            settings.receiver_name.trim().to_owned()
        };
        settings.renderer = self.platform.sanitize_renderer(&settings.renderer);
        settings
    }
}

#[async_trait]
impl SettingsStore for JsonSettingsStore {
    async fn load(&self) -> ReceiverSettings {
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return ReceiverSettings::default();
            }
            Err(error) => {
                self.logger
                    .write(
                        LogLevel::Warning,
                        format!("設定檔無法讀取，已使用預設值。{error}"),
                    )
                    .await;
                return ReceiverSettings::default();
            }
        };

        match deserialize_settings(&bytes) {
            Ok(settings) => self.sanitize(settings),
            Err(error) => {
                self.logger
                    .write(
                        LogLevel::Warning,
                        format!("設定檔無法讀取，已使用預設值。{error}"),
                    )
                    .await;
                ReceiverSettings::default()
            }
        }
    }

    async fn save(&self, settings: &ReceiverSettings) -> Result<ReceiverSettings, SettingsError> {
        let _write_guard = self.write_lock.lock().await;
        let sanitized = self.sanitize(settings.clone());
        let directory = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("settings directory is unavailable"))?;
        tokio::fs::create_dir_all(directory).await?;

        let mut temporary_path = self.path.as_os_str().to_os_string();
        temporary_path.push(".tmp");
        let temporary_path = PathBuf::from(temporary_path);
        let contents = serde_json::to_vec_pretty(&sanitized)?;

        let write_result = async {
            let mut file = tokio::fs::File::create(&temporary_path).await?;
            file.write_all(&contents).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temporary_path, &self.path).await
        }
        .await;

        if write_result.is_err() {
            let _ = tokio::fs::remove_file(&temporary_path).await;
        }
        write_result?;
        Ok(sanitized)
    }
}

fn deserialize_settings(bytes: &[u8]) -> Result<ReceiverSettings, serde_json::Error> {
    let mut value = serde_json::from_slice::<serde_json::Value>(bytes)?;
    if let serde_json::Value::Object(object) = &mut value {
        let property_names = [
            "ReceiverName",
            "Renderer",
            "Fullscreen",
            "StartOnLogin",
            "AndroidHwDecode",
            "UxplayPath",
        ];
        for property_name in property_names {
            let actual_name = object
                .keys()
                .find(|name| name.eq_ignore_ascii_case(property_name))
                .cloned();
            if let Some(actual_name) = actual_name
                && actual_name != property_name
                && let Some(property_value) = object.remove(&actual_name)
            {
                object.insert(property_name.to_owned(), property_value);
            }
        }
    }
    serde_json::from_value(value)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{FileAppLogger, Platform, ReceiverSettings};

    use super::{JsonSettingsStore, SettingsStore};

    #[tokio::test]
    async fn missing_settings_file_returns_defaults() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = Arc::new(FileAppLogger::new(temp.path().join("logs")));
        let store =
            JsonSettingsStore::new(temp.path().join("settings.json"), logger, Platform::Macos);

        assert_eq!(store.load().await, ReceiverSettings::default());
    }

    #[tokio::test]
    async fn save_sanitizes_and_round_trips_legacy_settings() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = Arc::new(FileAppLogger::new(temp.path().join("logs")));
        let settings_path = temp.path().join("settings.json");
        let store = JsonSettingsStore::new(&settings_path, logger, Platform::Macos);
        let input = ReceiverSettings {
            receiver_name: "  Living Room  ".to_owned(),
            renderer: "D3D11".to_owned(),
            fullscreen: true,
            start_on_login: true,
            ..ReceiverSettings::default()
        };

        let saved = store.save(&input).await.expect("settings should be saved");
        let loaded = store.load().await;

        assert_eq!(saved.receiver_name, "Living Room");
        assert_eq!(saved.renderer, "Auto");
        assert!(loaded.fullscreen);
        assert!(loaded.start_on_login);
        assert_eq!(loaded, saved);
        let contents = tokio::fs::read_to_string(settings_path)
            .await
            .expect("settings file should be readable");
        assert!(contents.contains("\"ReceiverName\""));
    }

    #[tokio::test]
    async fn malformed_json_returns_defaults_and_logs_warning() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = Arc::new(FileAppLogger::new(temp.path().join("logs")));
        let settings_path = temp.path().join("settings.json");
        tokio::fs::write(&settings_path, "{")
            .await
            .expect("fixture should be written");
        let store = JsonSettingsStore::new(settings_path, logger.clone(), Platform::Linux);

        assert_eq!(store.load().await, ReceiverSettings::default());
        assert_eq!(logger.snapshot().len(), 1);
        assert_eq!(logger.snapshot()[0].level, crate::LogLevel::Warning);
    }

    #[tokio::test]
    async fn load_accepts_case_insensitive_property_names() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let logger = Arc::new(FileAppLogger::new(temp.path().join("logs")));
        let settings_path = temp.path().join("settings.json");
        tokio::fs::write(
            &settings_path,
            r#"{"receiverName":"Living Room","startOnLogin":true}"#,
        )
        .await
        .expect("fixture should be written");
        let store = JsonSettingsStore::new(settings_path, logger, Platform::Macos);

        let settings = store.load().await;

        assert_eq!(settings.receiver_name, "Living Room");
        assert!(settings.start_on_login);
    }
}
