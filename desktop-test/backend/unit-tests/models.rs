//! 後端領域層模型、值物件、日誌環狀緩衝區與純文字解析函式的單元測試集合。

use super::{
    bonjour_instance_matches, parse_adb_devices, parse_missing_dependencies, stage_label,
    AndroidDevice, AndroidDeviceStatus, AppLogEntry, LogLevel, ReceiverName, ReceiverSettings,
    VideoRenderer,
};
use std::collections::VecDeque;

/// 驗證 `ReceiverSettings::sanitized` 會修剪名稱空白並將不支援的渲染器退回 `"Auto"`。
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

/// 驗證全空白接收器名稱會自動退回預設值 `"Neo AirPlay"`。
#[test]
fn blank_receiver_name_uses_default() {
    let settings = ReceiverSettings {
        receiver_name: "  ".to_owned(),
        ..ReceiverSettings::default()
    }
    .sanitized();

    assert_eq!(settings.receiver_name, "Neo AirPlay");
}

/// 驗證 `ReceiverName` 與 `VideoRenderer` 值物件建構時的不變量強制執行。
#[test]
fn value_objects_enforce_invariants() {
    assert_eq!(ReceiverName::new("  Living Room  ").as_str(), "Living Room");
    assert_eq!(ReceiverName::new("   ").as_str(), "Neo AirPlay");
    assert_eq!(VideoRenderer::new("invalid-renderer").as_str(), "Auto");
}

/// 驗證 `AndroidDevice::display_name` 與 `is_ready` 在模型與序號同時存在時的行為。
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

/// 驗證日誌項目會移除換行字元，且 `push_recent` 會將佇列長度限制在 `MAX_RECENT_LOGS` 內。
#[test]
fn log_entry_normalizes_newlines_and_caps_recent_history() {
    let entry = AppLogEntry::new(LogLevel::Information, " hello\r\nworld ");
    assert_eq!(entry.message, "hello  world");

    let mut entries = VecDeque::new();
    for index in 0..305 {
        super::push_recent(
            &mut entries,
            AppLogEntry::new(LogLevel::Information, &format!("entry-{index}")),
        );
    }
    assert_eq!(entries.len(), super::MAX_RECENT_LOGS);
    assert_eq!(entries.front().unwrap().message, "entry-5");
}

/// 驗證純領域解析函式（ADB 輸出、缺少套件、Bonjour 實例名稱）與階段中文標籤對應。
#[test]
fn domain_parsers_and_stage_labels_work_without_io() {
    assert_eq!(stage_label("repairing"), "修復編譯工具");
    assert_eq!(
        parse_missing_dependencies("- uxplay\n  - scrcpy\n"),
        vec!["uxplay", "scrcpy"]
    );
    assert_eq!(parse_adb_devices("List of devices attached\n").len(), 0);
    assert!(bonjour_instance_matches(
        "11:23:10.123 Add 2 4 local. _airplay._tcp. Neo AirPlay",
        "Neo AirPlay"
    ));
}
