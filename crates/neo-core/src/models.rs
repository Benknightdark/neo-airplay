use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "PascalCase")]
#[ts(rename_all = "camelCase")]
pub struct ReceiverSettings {
    #[serde(default = "default_receiver_name")]
    pub receiver_name: String,
    #[serde(default = "default_renderer")]
    pub renderer: String,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub start_on_login: bool,
    #[serde(default)]
    pub android_hw_decode: bool,
    #[serde(default)]
    pub uxplay_path: Option<String>,
}

impl Default for ReceiverSettings {
    fn default() -> Self {
        Self {
            receiver_name: default_receiver_name(),
            renderer: default_renderer(),
            fullscreen: false,
            start_on_login: false,
            android_hw_decode: false,
            uxplay_path: None,
        }
    }
}

impl ReceiverSettings {
    pub fn receiver_options(&self) -> ReceiverOptions {
        ReceiverOptions {
            receiver_name: self.receiver_name.clone(),
            renderer: self.renderer.clone(),
            fullscreen: self.fullscreen,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiverOptions {
    pub receiver_name: String,
    pub renderer: String,
    pub fullscreen: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "PascalCase")]
pub enum ReceiverState {
    #[default]
    Stopped,
    Starting,
    Running,
    Stopping,
    Faulted,
    DependencyMissing,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct EngineStatus {
    pub state: ReceiverState,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "PascalCase")]
pub enum AndroidDeviceStatus {
    Ready,
    Unauthorized,
    Offline,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AndroidDevice {
    pub serial: String,
    pub model: String,
    pub status: AndroidDeviceStatus,
}

impl AndroidDevice {
    pub fn display_name(&self) -> String {
        match self.status {
            AndroidDeviceStatus::Ready => format!("{}（{}）", self.model, self.serial),
            AndroidDeviceStatus::Unauthorized => {
                format!("{}（{}，尚未授權）", self.model, self.serial)
            }
            AndroidDeviceStatus::Offline => format!("{}（{}，離線）", self.model, self.serial),
            AndroidDeviceStatus::Unknown => format!("{}（{}，狀態未知）", self.model, self.serial),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "PascalCase")]
pub enum LogLevel {
    Information,
    Warning,
    Error,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Information => "Information",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AppLogEntry {
    #[ts(type = "string")]
    pub timestamp: DateTime<FixedOffset>,
    pub level: LogLevel,
    pub message: String,
}

impl AppLogEntry {
    pub fn display_time(&self) -> String {
        self.timestamp.format("%H:%M:%S").to_string()
    }

    pub const fn display_level(&self) -> &'static str {
        match self.level {
            LogLevel::Warning => "注意",
            LogLevel::Error => "錯誤",
            LogLevel::Information => "資訊",
        }
    }
}

fn default_receiver_name() -> String {
    "Neo AirPlay".to_owned()
}

fn default_renderer() -> String {
    "Auto".to_owned()
}

#[cfg(test)]
mod tests {
    use super::ReceiverSettings;

    #[test]
    fn deserializes_legacy_pascal_case_settings() {
        let json = r#"{"ReceiverName":"Living Room","Renderer":"D3D11","Fullscreen":true,"StartOnLogin":true}"#;

        let settings: ReceiverSettings = serde_json::from_str(json).expect("settings should parse");

        assert_eq!(settings.receiver_name, "Living Room");
        assert_eq!(settings.renderer, "D3D11");
        assert!(settings.fullscreen);
        assert!(settings.start_on_login);
        assert!(!settings.android_hw_decode);
        assert_eq!(settings.uxplay_path, None);
    }

    #[test]
    fn defaults_fields_missing_from_legacy_settings() {
        let settings: ReceiverSettings = serde_json::from_str("{}").expect("settings should parse");

        assert_eq!(settings, ReceiverSettings::default());
    }
}
