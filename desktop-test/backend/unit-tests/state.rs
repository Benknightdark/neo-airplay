//! 後端基礎設施層狀態管理、設定檔讀寫與子程序控制單元測試集合。

#[cfg(unix)]
use crate::domain::AirplayAuthMode;
use crate::domain::{
    bonjour_instance_matches, parse_adb_devices, renderer_argument, AndroidDeviceStatus,
    EngineKind, EngineSnapshot, EngineStatus, ReceiverSettings, AIRPLAY_READY_TIMEOUT,
};
use crate::infrastructure::persistence::{
    load_settings, write_settings, AppLogger, MAX_SETTINGS_FILE_BYTES,
};
#[cfg(unix)]
use crate::infrastructure::process::find_executable;
#[cfg(target_os = "macos")]
use crate::infrastructure::process::macos_library_paths;
use crate::infrastructure::process::{
    finish_pipe, is_executable_file, read_bounded_lines, read_pipe, sanitize_child_environment,
    MAX_CAPTURED_PROCESS_OUTPUT_BYTES,
};
use crate::infrastructure::state::{AppState, DiscoveryEvent, EngineController};
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
        ..ReceiverSettings::default()
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
        ..ReceiverSettings::default()
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

/// 驗證不存在的路徑與目錄不會通過 `is_executable_file` 檢查，且非 Unix 平台接受常規檔案。
#[test]
fn executable_check_rejects_missing_paths_and_directories() {
    let directory = test_directory("executable-metadata-check");
    fs::create_dir_all(&directory).unwrap();

    assert!(!is_executable_file(&directory));
    assert!(!is_executable_file(&directory.join("missing-binary")));

    #[cfg(not(unix))]
    {
        let regular_file = directory.join("uxplay.exe");
        fs::write(&regular_file, b"windows executable").unwrap();
        assert!(is_executable_file(&regular_file));
    }

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

/// 驗證 `find_executable` 與 `is_executable_file` 會拒絕符號連結執行檔及符號連結子目錄。
#[cfg(unix)]
#[test]
fn executable_lookup_rejects_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let directory = test_directory("symlink-lookup");
    let engine_bin = directory.join("bin");
    let outside_dir = directory.join("outside");
    fs::create_dir_all(&engine_bin).unwrap();
    fs::create_dir_all(&outside_dir).unwrap();

    let outside_exec = outside_dir.join("uxplay");
    fs::write(&outside_exec, b"outside").unwrap();
    fs::set_permissions(&outside_exec, fs::Permissions::from_mode(0o700)).unwrap();

    let symlinked_exec = engine_bin.join("uxplay");
    symlink(&outside_exec, &symlinked_exec).unwrap();
    assert!(!is_executable_file(&symlinked_exec));
    assert_eq!(find_executable(&directory, "uxplay", false), None);

    let android_link = directory.join("android");
    symlink(&outside_dir, &android_link).unwrap();
    let outside_adb = outside_dir.join("adb");
    fs::write(&outside_adb, b"adb").unwrap();
    fs::set_permissions(&outside_adb, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(find_executable(&directory, "adb", true), None);

    let symlinked_engine = test_directory("symlinked-engine-root");
    symlink(&directory, &symlinked_engine).unwrap();
    let real_uxplay = engine_bin.join("real-uxplay");
    fs::write(&real_uxplay, b"real").unwrap();
    fs::set_permissions(&real_uxplay, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        find_executable(&symlinked_engine, "real-uxplay", false),
        None
    );
    let _ = fs::remove_file(&symlinked_engine);

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

/// 驗證 `AppLogger` 能將日誌寫入當日 `.jsonl` 檔案並在重新載入後保留既有與舊版偏移量格式紀錄。
#[test]
fn app_logger_persists_and_reloads_daily_jsonl_entries() {
    let directory = test_directory("app-logger-roundtrip");
    fs::create_dir_all(&directory).unwrap();

    let logger = AppLogger::new(directory.clone());
    logger.write(crate::domain::LogLevel::Information, "runtime started");
    let expected_date = jiff::Zoned::now().strftime("%Y-%m-%d").to_string();
    let daily_log_path = directory.join(format!("{expected_date}.jsonl"));
    assert!(daily_log_path.is_file());

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&daily_log_path)
        .unwrap();
    writeln!(
        file,
        r#"{{"timestamp":"2026-11-13T12:00:00+08:00","level":"warning","message":"legacy line"}}"#
    )
    .unwrap();
    drop(file);

    let reloaded = AppLogger::new(directory.clone());
    let recent = reloaded.recent();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].message, "runtime started");
    assert_eq!(recent[1].message, "legacy line");

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

/// 驗證 `write_settings` 設定 `0o600` 權限，且 `load_settings` 拒絕符號連結與超過上限的設定檔。
#[test]
fn settings_persistence_enforces_size_limit_and_unix_permissions() {
    let directory = test_directory("settings-security");
    fs::create_dir_all(&directory).unwrap();
    let settings_path = directory.join("settings.json");

    let custom = ReceiverSettings {
        receiver_name: "Locked".to_owned(),
        ..ReceiverSettings::default()
    };
    write_settings(&settings_path, &custom).unwrap();
    assert_eq!(load_settings(&settings_path), custom);

    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let mode = fs::metadata(&settings_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let outside = directory.join("outside.json");
        fs::write(&outside, r#"{"receiverName":"Outside"}"#).unwrap();
        let symlink_path = directory.join("symlink-settings.json");
        symlink(&outside, &symlink_path).unwrap();
        assert_eq!(load_settings(&symlink_path), ReceiverSettings::default());
        assert!(write_settings(&symlink_path, &custom).is_err());
    }

    let oversized_path = directory.join("oversized.json");
    let oversized_payload = vec![b' '; (MAX_SETTINGS_FILE_BYTES as usize) + 128];
    fs::write(&oversized_path, oversized_payload).unwrap();
    assert_eq!(load_settings(&oversized_path), ReceiverSettings::default());

    fs::remove_dir_all(&directory).unwrap();
}

/// 驗證 `AppLogger` 拒絕符號連結的當日日誌檔且不覆寫外部目標。
#[cfg(unix)]
#[test]
fn app_logger_refuses_symlinked_daily_log_target() {
    use std::os::unix::fs::symlink;

    let directory = test_directory("logger-symlink");
    fs::create_dir_all(&directory).unwrap();
    let outside = directory.join("outside.txt");
    fs::write(&outside, b"keep-untouched").unwrap();

    let today = jiff::Zoned::now().strftime("%Y-%m-%d").to_string();
    let daily_log = directory.join(format!("{today}.jsonl"));
    symlink(&outside, &daily_log).unwrap();

    let logger = AppLogger::new(directory.clone());
    logger.write(
        crate::domain::LogLevel::Information,
        "should not write through",
    );
    assert_eq!(fs::read(&outside).unwrap(), b"keep-untouched");

    fs::remove_dir_all(&directory).unwrap();
}

/// 驗證 `read_bounded_lines` 會截斷超過上限的單行資料以防止 OOM 並繼續讀取後續行。
#[test]
fn read_bounded_lines_caps_long_lines_and_keeps_reading() {
    let mut payload = vec![b'x'; 65536];
    payload.extend_from_slice(b"\nsecond line\r\n");

    let mut collected = Vec::new();
    read_bounded_lines(Cursor::new(payload), 256, |line| collected.push(line));

    assert_eq!(collected.len(), 2);
    assert!(collected[0].ends_with("…[truncated]"));
    assert!(collected[0].len() <= 256 + "…[truncated]".len());
    assert_eq!(collected[1], "second line");
}

/// 驗證 `sanitize_child_environment` 會移除所有高風險環境變數注入鍵。
#[test]
fn sanitize_child_environment_removes_dangerous_variables() {
    let mut command = Command::new("env");
    sanitize_child_environment(&mut command);

    let removed: std::collections::HashSet<&OsStr> = command
        .get_envs()
        .filter_map(|(key, value)| if value.is_none() { Some(key) } else { None })
        .collect();

    for expected in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_FRAMEWORK_PATH",
        "OPENSSL_CONF",
        "GCONV_PATH",
        "UXPLAYRC",
        "ADB",
        "SCRCPY_SERVER_PATH",
    ] {
        assert!(
            removed.contains(OsStr::new(expected)),
            "expected dangerous env var {expected} to be removed"
        );
    }
}

/// 驗證 macOS `macos_library_paths` 拒絕相對路徑、`..` 遍歷路徑與符號連結目錄。
#[cfg(target_os = "macos")]
#[test]
fn macos_library_paths_rejects_relative_parent_and_symlink_entries() {
    use std::os::unix::fs::symlink;

    let directory = test_directory("macos-lib-paths");
    let real_lib = directory.join("real-lib");
    let symlink_lib = directory.join("sym-lib");
    fs::create_dir_all(&real_lib).unwrap();
    symlink(&real_lib, &symlink_lib).unwrap();

    let combined = format!(
        "relative/lib:/tmp/../escaped:{}:{}",
        symlink_lib.display(),
        real_lib.display()
    );
    let paths = macos_library_paths(&directory, Some(OsStr::new(&combined)));

    assert!(paths.contains(&real_lib));
    assert!(!paths.iter().any(|path| path == Path::new("relative/lib")));
    assert!(!paths
        .iter()
        .any(|path| path == Path::new("/tmp/../escaped")));
    assert!(!paths.contains(&symlink_lib));

    fs::remove_dir_all(&directory).unwrap();
}

/// 驗證 `AppState::start_android_mirror` 在啟動 `scrcpy` 前拒絕不合法的 ADB 序號。
#[test]
fn start_android_mirror_rejects_invalid_serials() {
    let directory = test_directory("invalid-android-serial");
    let state = AppState::new(
        directory.join("config"),
        directory.join("logs"),
        directory.join("engine"),
        directory.join("cache/gstreamer/registry.bin"),
        false,
    )
    .unwrap();

    for bad_serial in ["", "-H", "--no-control", "../bad", "bad serial", "a\nb"] {
        assert!(
            state.start_android_mirror(bad_serial).is_err(),
            "expected invalid serial {bad_serial:?} to be rejected"
        );
    }

    fs::remove_dir_all(&directory).unwrap();
}

/// 驗證 Android 引擎在缺少或為符號連結的 `scrcpy-server` 時拒絕啟動並標記缺少元件。
#[test]
fn android_controller_requires_non_symlink_scrcpy_server() {
    let directory = test_directory("scrcpy-server-check");
    let engine = directory.join("engine");
    let bin = engine.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let exe_name = if cfg!(target_os = "windows") {
        "scrcpy.exe"
    } else {
        "scrcpy"
    };
    let adb_name = if cfg!(target_os = "windows") {
        "adb.exe"
    } else {
        "adb"
    };
    let scrcpy_bin = bin.join(exe_name);
    let adb_bin = bin.join(adb_name);
    fs::write(&scrcpy_bin, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::write(&adb_bin, b"#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&scrcpy_bin, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&adb_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let controller = EngineController::new(
        EngineKind::Android,
        engine.clone(),
        directory.join("cache/reg.bin"),
        false,
        AppLogger::new(directory.join("logs")),
    );
    let snapshot = controller.start(&[], None);
    assert_eq!(snapshot.status, EngineStatus::DependencyMissing);
    assert!(snapshot.message.contains("scrcpy-server"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = directory.join("outside-server");
        fs::write(&outside, b"server").unwrap();
        symlink(&outside, bin.join("scrcpy-server")).unwrap();
        let sym_snapshot = controller.start(&[], None);
        assert_eq!(sym_snapshot.status, EngineStatus::DependencyMissing);
    }

    fs::remove_dir_all(&directory).unwrap();
}

/// 驗證 AirPlay 固定 PIN 模式會寫入 `0600` `uxplay.rc` 與 `uxplay.register`、拒絕符號連結並在日誌中遮罩 PIN。
#[cfg(unix)]
#[test]
fn airplay_fixed_pin_writes_secure_rc_redacts_logs_and_blocks_symlink() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let directory = test_directory("airplay-fixed-pin");
    let engine = directory.join("engine");
    let bin = engine.join("bin");
    let config = directory.join("config");
    let logs = directory.join("logs");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&config).unwrap();

    let uxplay = bin.join("uxplay");
    fs::write(
        &uxplay,
        "#!/bin/sh\nprintf '*** CLIENT MUST NOW ENTER PIN = \"9876\" AS AIRPLAY PASSWORD\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&uxplay, fs::Permissions::from_mode(0o755)).unwrap();

    let logger = AppLogger::new(logs);
    let controller = EngineController::new(
        EngineKind::AirPlay,
        engine,
        directory.join("cache/reg.bin"),
        false,
        logger.clone(),
    )
    .with_config_dir(config.clone());

    let settings = ReceiverSettings {
        airplay_auth_mode: AirplayAuthMode::FixedPin,
        airplay_pin: "9876".to_owned(),
        ..ReceiverSettings::default()
    };
    let snapshot = controller.start_airplay(&settings);
    assert_eq!(snapshot.status, EngineStatus::Running);

    let rc_path = config.join("uxplay.rc");
    let reg_path = config.join("uxplay.register");
    assert_eq!(
        fs::metadata(&rc_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&reg_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let rc_text = fs::read_to_string(&rc_path).unwrap();
    assert!(rc_text.contains("pin 9876\n"));
    assert!(rc_text.contains("reg \""));

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut saw_redacted = false;
    while std::time::Instant::now() < deadline {
        let recent = logger.recent();
        if recent
            .iter()
            .any(|entry| entry.message.contains("ENTER PIN = \"****\""))
        {
            saw_redacted = true;
            assert!(
                recent.iter().all(|entry| !entry.message.contains("9876")),
                "fixed PIN must never leak into logs: {:?}",
                recent
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(saw_redacted, "expected redacted PIN line in captured logs");

    let _ = controller.stop();
    drop(controller);

    let outside = directory.join("outside.rc");
    fs::write(&outside, b"untouched").unwrap();
    fs::remove_file(&rc_path).unwrap();
    symlink(&outside, &rc_path).unwrap();

    let blocked_controller = EngineController::new(
        EngineKind::AirPlay,
        directory.join("engine"),
        directory.join("cache/reg.bin"),
        false,
        logger,
    )
    .with_config_dir(config);
    let blocked = blocked_controller.start_airplay(&settings);
    assert_eq!(blocked.status, EngineStatus::DependencyMissing);
    assert_eq!(fs::read(&outside).unwrap(), b"untouched");

    fs::remove_dir_all(&directory).unwrap();
}
