use std::sync::Arc;
use std::time::Duration;

use neo_core::{
    AndroidDeviceStatus, AndroidMirrorEngine, AppLogger, AppPaths, FileAppLogger, Platform,
    ReceiverSettings, ReceiverState, TokioProcessSpawner, UxPlayEngine,
};

#[tokio::test]
#[ignore = "requires UxPlay, a working mDNS service, scrcpy v5, and an authorized USB Android device"]
async fn projects_airplay_and_android_concurrently() {
    let paths = AppPaths::new(None).expect("application data paths should be available");
    let logger = Arc::new(FileAppLogger::new(&paths.log_directory));
    let logger_trait: Arc<dyn AppLogger> = logger.clone();
    let spawner = Arc::new(TokioProcessSpawner);
    let platform = Platform::current();
    let settings = ReceiverSettings::default();
    let bundled_engine_directory =
        Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine"));

    let receiver = UxPlayEngine::new(
        platform,
        bundled_engine_directory.clone(),
        paths.engine_directory.clone(),
        paths.gstreamer_registry_path.clone(),
        logger_trait.clone(),
        spawner.clone(),
    );
    let android = AndroidMirrorEngine::new(
        platform,
        bundled_engine_directory,
        paths.engine_directory.clone(),
        required_scrcpy_files(),
        logger_trait,
        spawner,
    );

    let receiver_status = receiver.start(&settings).await;
    if receiver_status.state != ReceiverState::Running {
        let message = receiver_status.message.clone();
        let _ = receiver.stop().await;
        panic!("UxPlay failed to start: {message}");
    }

    let devices = android.refresh_devices().await;
    let Some(device) = devices
        .into_iter()
        .find(|device| device.status == AndroidDeviceStatus::Ready)
    else {
        let _ = receiver.stop().await;
        panic!("ADB found no authorized Android device. Connect one and authorize USB debugging.");
    };

    let android_status = android.start(&device.serial, &settings).await;
    if android_status.state != ReceiverState::Running {
        let message = android_status.message.clone();
        let _ = tokio::join!(android.stop(), receiver.stop());
        panic!("scrcpy failed to start for {}: {message}", device.serial);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    let both_running = receiver.status().state == ReceiverState::Running
        && android.status().state == ReceiverState::Running;
    let _ = tokio::join!(android.stop(), receiver.stop());

    assert!(
        both_running,
        "both projection engines must remain active together"
    );
    assert_eq!(android.status().state, ReceiverState::Stopped);
    assert_eq!(receiver.status().state, ReceiverState::Stopped);
}

fn required_scrcpy_files() -> Vec<String> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let manifest = include_str!("../../../scripts/scrcpy-runtime-files.macos-aarch64.txt");
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let manifest = include_str!("../../../scripts/scrcpy-runtime-files.macos-x86_64.txt");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let manifest = include_str!("../../../scripts/scrcpy-runtime-files.linux-x86_64.txt");
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let manifest = include_str!("../../../scripts/scrcpy-runtime-files.windows-x86_64.txt");
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    let manifest = "";

    manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
