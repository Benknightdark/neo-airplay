use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverSettings {
    #[serde(default = "default_receiver_name")]
    pub receiver_name: String,
    #[serde(default = "default_renderer")]
    pub renderer: String,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub start_on_login: bool,
}

impl Default for ReceiverSettings {
    fn default() -> Self {
        Self {
            receiver_name: default_receiver_name(),
            renderer: default_renderer(),
            fullscreen: false,
            start_on_login: false,
        }
    }
}

impl ReceiverSettings {
    pub fn sanitized(mut self) -> Self {
        self.receiver_name = self.receiver_name.trim().to_owned();
        if self.receiver_name.is_empty() {
            self.receiver_name = default_receiver_name();
        }
        if !available_renderers().contains(&self.renderer.as_str()) {
            self.renderer = default_renderer();
        }
        self
    }
}

fn default_receiver_name() -> String {
    "Neo AirPlay".to_owned()
}

fn default_renderer() -> String {
    "Auto".to_owned()
}

pub fn available_renderers() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["Auto", "D3D11", "D3D12"]
    }
    #[cfg(target_os = "macos")]
    {
        &["Auto", "osxvideosink", "glimagesink"]
    }
    #[cfg(target_os = "linux")]
    {
        &["Auto", "glimagesink", "ximagesink", "waylandsink"]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        &["Auto"]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EngineStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Faulted,
    DependencyMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngineSnapshot {
    pub status: EngineStatus,
    pub message: String,
    pub process_id: Option<u32>,
}

impl EngineSnapshot {
    pub fn stopped(message: impl Into<String>) -> Self {
        Self {
            status: EngineStatus::Stopped,
            message: message.into(),
            process_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AndroidDeviceStatus {
    Ready,
    Unauthorized,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevice {
    pub serial: String,
    pub model: String,
    pub status: AndroidDeviceStatus,
}

impl AndroidDevice {
    pub fn is_ready(&self) -> bool {
        self.status == AndroidDeviceStatus::Ready
    }

    pub fn display_name(&self) -> String {
        if self.model.is_empty() || self.model == self.serial {
            self.serial.clone()
        } else {
            format!("{} ({})", self.model, self.serial)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Information,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppLogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub settings: ReceiverSettings,
    pub renderer_options: Vec<String>,
    pub airplay: EngineSnapshot,
    pub android: EngineSnapshot,
    pub devices: Vec<AndroidDevice>,
    pub logs: Vec<AppLogEntry>,
}

#[cfg(test)]
mod tests {
    use super::{AndroidDevice, AndroidDeviceStatus, ReceiverSettings};

    #[test]
    fn settings_are_normalized_and_invalid_renderer_uses_auto() {
        let settings = ReceiverSettings {
            receiver_name: "  Studio  ".to_owned(),
            renderer: "unknown-sink".to_owned(),
            fullscreen: true,
            start_on_login: false,
        }
        .sanitized();

        assert_eq!(settings.receiver_name, "Studio");
        assert_eq!(settings.renderer, "Auto");
        assert!(settings.fullscreen);
    }

    #[test]
    fn blank_receiver_name_uses_default() {
        let settings = ReceiverSettings {
            receiver_name: "  ".to_owned(),
            ..ReceiverSettings::default()
        }
        .sanitized();

        assert_eq!(settings.receiver_name, "Neo AirPlay");
    }

    #[test]
    fn device_label_includes_model_and_serial() {
        let device = AndroidDevice {
            serial: "abc123".to_owned(),
            model: "Pixel_8".to_owned(),
            status: AndroidDeviceStatus::Ready,
        };

        assert_eq!(device.display_name(), "Pixel_8 (abc123)");
        assert!(device.is_ready());
    }
}
