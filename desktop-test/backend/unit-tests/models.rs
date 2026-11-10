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
