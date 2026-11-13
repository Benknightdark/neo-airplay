//! 後端基礎設施層狀態管理、設定檔讀寫與子程序控制單元測試集合。

use crate::domain::{
    bonjour_instance_matches, parse_adb_devices, renderer_argument, AndroidDeviceStatus,
    EngineKind, EngineSnapshot, EngineStatus, ReceiverSettings, AIRPLAY_READY_TIMEOUT,
};
use crate::infrastructure::persistence::{load_settings, write_settings, AppLogger};
#[cfg(target_os = "macos")]
use crate::infrastructure::process::macos_library_paths;
#[cfg(unix)]
use crate::infrastructure::process::{find_executable, is_executable_file};
use crate::infrastructure::process::{finish_pipe, read_pipe, MAX_CAPTURED_PROCESS_OUTPUT_BYTES};
use crate::infrastructure::state::{AppState, DiscoveryEvent, EngineController};
#[cfg(target_os = "macos")]
use std::ffi::OsStr;
use std::fs;
use std::io::Cursor;
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::Duration;

/// 驗證 macOS `DYLD_LIBRARY_PATH` 只包含內建 `lib`、基礎 Homebrew 目錄與既有環境路徑。
#[cfg(target_os = "macos")]
#[test]
fn macos_library_path_uses_only_base_homebrew_directories() {
    let paths = macos_library_paths(Path::new("/engine"), Some(OsStr::new("/inherited/lib")));

    assert_eq!(
        paths,
        [
            PathBuf::from("/engine/lib"),
            PathBuf::from("/opt/homebrew/lib"),
            PathBuf::from("/usr/local/lib"),
            PathBuf::from("/inherited/lib"),
        ]
    );
}

/// 驗證 `parse_adb_devices` 能正確解析已就緒、未授權與離線等不同狀態的 Android 裝置。
#[test]
fn parses_android_devices_and_states() {
    let devices = parse_adb_devices(
            "List of devices attached\nusb-1 device product:foo model:Pixel_8\nusb-2 unauthorized usb:1-2\nusb-3 offline transport_id:8\n",
        );
    assert_eq!(devices.len(), 3);
    assert_eq!(devices[0].model, "Pixel 8");
    assert_eq!(devices[0].status, AndroidDeviceStatus::Ready);
    assert_eq!(devices[1].status, AndroidDeviceStatus::Unauthorized);
    assert_eq!(devices[2].status, AndroidDeviceStatus::Offline);
}

/// 驗證 ADB daemon 啟動與標題資訊列會被正確略過。
#[test]
fn ignores_adb_server_messages() {
    assert!(
        parse_adb_devices("* daemon started successfully\nList of devices attached\n").is_empty()
    );
}

/// 驗證僅完全符合接收器實例名稱的 Bonjour `Add` 事件會視為就緒。
#[test]
fn matches_only_added_airplay_services_with_the_configured_instance_name() {
    let matching = "11:23:10.123 Add 2 4 local. _airplay._tcp. Neo AirPlay";
    let repeated_spaces = "11:23:10.123 Add 2 4 local. _airplay._tcp. Living  Room";
    let removed = "11:23:10.123 Rmv 2 4 local. _airplay._tcp. Neo AirPlay";
    let other_name = "11:23:10.123 Add 2 4 local. _airplay._tcp. Other Receiver";

    assert!(bonjour_instance_matches(matching, "Neo AirPlay"));
    assert!(bonjour_instance_matches(repeated_spaces, "Living  Room"));
    assert!(!bonjour_instance_matches(removed, "Neo AirPlay"));
    assert!(!bonjour_instance_matches(other_name, "Neo AirPlay"));
    assert!(!bonjour_instance_matches("Browse complete", "Neo AirPlay"));
}

/// 驗證收到 Bonjour 就緒事件時 AirPlay 狀態會由 `Starting` 轉換為 `Running`。
#[test]
fn bonjour_ready_event_moves_airplay_from_starting_to_running() {
    let directory = test_directory("airplay-ready-event");
    fs::create_dir_all(&directory).unwrap();
    let controller = EngineController::new(
        EngineKind::AirPlay,
        PathBuf::new(),
        PathBuf::new(),
        false,
        AppLogger::new(directory.clone()),
    );
    let (sender, events) = std::sync::mpsc::channel();
    sender.send(DiscoveryEvent::Ready).unwrap();
    let mut inner = controller.inner.lock().unwrap();
    inner.snapshot = EngineSnapshot {
        status: EngineStatus::Starting,
        message: "等待 Bonjour。".to_owned(),
        process_id: Some(42),
    };
    inner.discovery_events = Some(events);
    inner.started_at = Some(std::time::Instant::now());

    controller.update_airplay_readiness(&mut inner);

    assert_eq!(inner.snapshot.status, EngineStatus::Running);
    assert_eq!(inner.snapshot.process_id, None);
    assert!(inner.snapshot.message.contains("Bonjour 上就緒"));
    drop(inner);
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證超過 `AIRPLAY_READY_TIMEOUT` 仍未找到 Bonjour 廣播時 AirPlay 會進入 `Faulted` 失敗狀態。
#[test]
fn missing_bonjour_service_times_out_and_faults_airplay_start() {
    let directory = test_directory("airplay-ready-timeout");
    fs::create_dir_all(&directory).unwrap();
    let controller = EngineController::new(
        EngineKind::AirPlay,
        PathBuf::new(),
        PathBuf::new(),
        false,
        AppLogger::new(directory.clone()),
    );
    let (_sender, events) = std::sync::mpsc::channel();
    let mut inner = controller.inner.lock().unwrap();
    inner.snapshot = EngineSnapshot {
        status: EngineStatus::Starting,
        message: "等待 Bonjour。".to_owned(),
        process_id: Some(42),
    };
    inner.discovery_events = Some(events);
    inner.started_at = Some(std::time::Instant::now() - AIRPLAY_READY_TIMEOUT);

    controller.update_airplay_readiness(&mut inner);

    assert_eq!(inner.snapshot.status, EngineStatus::Faulted);
    assert_eq!(inner.snapshot.process_id, None);
    assert!(inner
        .snapshot
        .message
        .contains("90 秒內沒有在 Bonjour 找到"));
    drop(inner);
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證含特殊符號的接收器名稱會保留在獨立的 `-n` 命令列參數元素中而不被 Shell 切割。
#[test]
fn keeps_receiver_name_in_a_separate_argument() {
    let settings = ReceiverSettings {
        receiver_name: "Living Room; safe".to_owned(),
        renderer: "Auto".to_owned(),
        fullscreen: true,
        start_on_login: false,
    };
    #[allow(unused_mut)]
    let mut expected = vec![
        "-n".to_owned(),
        "Living Room; safe".to_owned(),
        "-nh".to_owned(),
        "-fs".to_owned(),
    ];
    #[cfg(target_os = "macos")]
    expected.extend([
        "-avdec".to_owned(),
        "-vsync".to_owned(),
        "no".to_owned(),
        "-vs".to_owned(),
        "osxvideosink".to_owned(),
    ]);
    assert_eq!(settings.airplay_arguments(), expected);
}

/// 驗證視訊渲染器參數僅接受白名單內的合法項目，未知選項會回傳 `None`。
#[test]
fn renderer_names_are_allowlisted() {
    assert_eq!(renderer_argument("D3D11"), Some("d3d11videosink"));
    #[cfg(target_os = "macos")]
    assert_eq!(renderer_argument("Auto"), Some("osxvideosink"));
    #[cfg(not(target_os = "macos"))]
    assert_eq!(renderer_argument("Auto"), None);
    assert_eq!(renderer_argument("arbitrary"), None);
}

/// 驗證設定透過 JSON 寫入與載入後可完整往返還原。
#[test]
fn settings_roundtrip_through_json_file() {
    let directory = test_directory("settings");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("settings.json");
    let settings = ReceiverSettings {
        receiver_name: "Studio".to_owned(),
        renderer: "Auto".to_owned(),
        fullscreen: true,
        start_on_login: true,
    };

    write_settings(&path, &settings).unwrap();

    assert_eq!(load_settings(&path), settings);
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證多執行緒並發寫入設定時，記憶體快取與磁碟 `settings.json` 檔案仍保持最終一致。
#[test]
fn concurrent_settings_saves_keep_memory_and_disk_in_sync() {
    const WORKERS: usize = 8;
    const SAVES_PER_WORKER: usize = 40;

    let directory = test_directory("concurrent-settings");
    let state = Arc::new(
        AppState::new(
            directory.join("config"),
            directory.join("logs"),
            PathBuf::new(),
            directory.join("cache/gstreamer/registry.bin"),
            false,
        )
        .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|worker| {
            let state = Arc::clone(&state);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for save in 0..SAVES_PER_WORKER {
                    state
                        .save_settings(ReceiverSettings {
                            receiver_name: format!("worker-{worker}-save-{save}"),
                            ..ReceiverSettings::default()
                        })
                        .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();

    for worker in workers {
        worker.join().unwrap();
    }

    let persisted = load_settings(&directory.join("config/settings.json"));
    assert_eq!(persisted, state.settings().unwrap());
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證 Unix 平台缺少執行權限的檔案不會通過 `is_executable_file` 檢查。
#[cfg(unix)]
#[test]
fn executable_check_rejects_files_without_execute_permission() {
    use std::os::unix::fs::PermissionsExt;

    let directory = test_directory("non-executable");
    let engine_directory = directory.join("bin");
    fs::create_dir_all(&engine_directory).unwrap();
    let executable = engine_directory.join("uxplay");
    fs::write(&executable, b"not executable").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(!is_executable_file(&executable));
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證執行檔搜尋只會檢查受控管 `engine` 目錄，不會意外拾取外部同級目錄的可執行檔。
#[cfg(unix)]
#[test]
fn executable_lookup_uses_only_the_engine_directory() {
    use std::os::unix::fs::PermissionsExt;

    let directory = test_directory("engine-only-lookup");
    let engine_bin = directory.join("bin");
    let outside_bin = directory.join("outside");
    fs::create_dir_all(&engine_bin).unwrap();
    fs::create_dir_all(&outside_bin).unwrap();
    let outside_executable = outside_bin.join("uxplay");
    fs::write(&outside_executable, b"outside engine").unwrap();
    fs::set_permissions(&outside_executable, fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(find_executable(&directory, "uxplay", false), None);

    let engine_executable = engine_bin.join("uxplay");
    fs::write(&engine_executable, b"engine tool").unwrap();
    fs::set_permissions(&engine_executable, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        find_executable(&directory, "uxplay", false),
        Some(engine_executable)
    );

    fs::remove_dir_all(directory).unwrap();
}

/// 驗證管道輸出超過 1 MiB 上限時，`finish_pipe` 會安全拒絕並回傳容量超限錯誤。
#[test]
fn process_output_capture_rejects_overflow_after_draining_pipe() {
    let output = vec![b'x'; MAX_CAPTURED_PROCESS_OUTPUT_BYTES + 1];
    let result = finish_pipe(Some(read_pipe(Cursor::new(output))));
    assert_eq!(result.unwrap_err(), "外部程序輸出超過 1 MiB 上限。");
}

/// 驗證正在執行中的 Android 引擎不會被後續 ADB 重新整理失敗狀態覆蓋。
#[test]
fn adb_refresh_error_does_not_hide_a_running_engine() {
    let directory = test_directory("running-engine");
    fs::create_dir_all(&directory).unwrap();
    let logger = AppLogger::new(directory.clone());
    let controller = EngineController::new(
        EngineKind::Android,
        PathBuf::new(),
        PathBuf::new(),
        false,
        logger,
    );
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "infrastructure::state::tests::engine_test_child_process",
            "--nocapture",
        ])
        .env("NEO_AIRPLAY_HOLD_ENGINE_TEST_CHILD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let process_id = child.id();
    {
        let mut inner = controller.inner.lock().unwrap();
        inner.child = Some(child);
        inner.snapshot = EngineSnapshot {
            status: EngineStatus::Running,
            message: "scrcpy 正在執行。".to_owned(),
            process_id: Some(process_id),
        };
    }

    controller.set_external_status(EngineStatus::Faulted, "ADB 查詢失敗。".to_owned());
    assert_eq!(controller.snapshot().status, EngineStatus::Running);
    assert_eq!(controller.stop().status, EngineStatus::Stopped);

    drop(controller);
    fs::remove_dir_all(directory).unwrap();
}

/// 作為 `adb_refresh_error_does_not_hide_a_running_engine` 測試使用的存活子程序替身（由環境變數觸發休眠）。
#[test]
fn engine_test_child_process() {
    if std::env::var_os("NEO_AIRPLAY_HOLD_ENGINE_TEST_CHILD").is_some() {
        std::thread::sleep(Duration::from_secs(30));
    }
}

/// 驗證 Windows `background_command` 建立的子程序不具備主控台視窗且雙輸出管道可安全擷取。
#[cfg(target_os = "windows")]
#[test]
fn background_child_has_no_console_and_both_pipes_are_captured() {
    let output =
        crate::infrastructure::process::background_command(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "infrastructure::state::tests::background_console_probe",
                "--nocapture",
            ])
            .env("NEO_AIRPLAY_TEST_BACKGROUND_CONSOLE", "1")
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() >= 128 * 1024);
    assert!(output.stderr.len() >= 128 * 1024);
}

/// Windows 子程序探針：確認無主控台視窗句柄並寫入大量 `stdout`/`stderr` 資料測試無死鎖排空。
#[cfg(target_os = "windows")]
#[test]
fn background_console_probe() {
    if std::env::var_os("NEO_AIRPLAY_TEST_BACKGROUND_CONSOLE").is_none() {
        return;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }
    // This fixture runs in a child process created with CREATE_NO_WINDOW.
    assert!(unsafe { GetConsoleWindow().is_null() });
    use std::io::Write;
    for _ in 0..16 {
        std::io::stdout().write_all(&[b'o'; 8192]).unwrap();
        std::io::stderr().write_all(&[b'e'; 8192]).unwrap();
    }
}

/// 建立含 PID 與奈秒亂數的隔離暫存測試目錄路徑。
///
/// # 參數
/// - `name`: 測試識別前綴字串。
///
/// # 回傳值
/// 回傳位於系統暫存目錄下的唯一測試目錄 `PathBuf`。
fn test_directory(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("neo-airplay-{name}-{}-{nonce}", std::process::id()))
}
