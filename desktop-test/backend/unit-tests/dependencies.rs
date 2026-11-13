//! 後端依賴工具管理器的單元與整合測試集合，涵蓋安裝日誌讀取、並發互斥、子程序監控及清除重裝範圍驗證。

use super::*;

/// 驗證存在 `install.log` 時 `read_install_log` 能完整讀取其內容。
#[test]
fn read_install_log_returns_saved_content() {
    let config_dir = install_log_test_directory();
    let setup_dir = config_dir.join("setup");
    fs::create_dir_all(&setup_dir).unwrap();
    fs::write(
        setup_dir.join(INSTALL_LOG),
        "install started\ninstall finished\n",
    )
    .unwrap();
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        &config_dir,
        true,
    );

    assert_eq!(
        manager.read_install_log().unwrap(),
        "install started\ninstall finished\n"
    );
    fs::remove_dir_all(config_dir).unwrap();
}

/// 驗證當 `install.log` 尚未建立時會回傳「尚無安裝記錄。」錯誤提示。
#[test]
fn read_install_log_reports_when_file_is_missing() {
    let config_dir = install_log_test_directory();
    fs::create_dir_all(&config_dir).unwrap();
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        &config_dir,
        true,
    );

    assert_eq!(manager.read_install_log().unwrap_err(), "尚無安裝記錄。");
    fs::remove_dir_all(config_dir).unwrap();
}

/// 驗證停用自動安裝的工作階段呼叫 `start` 仍維持 `"disabled"` 而不執行安裝。
#[test]
fn disabled_session_does_not_start_installation() {
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        Path::new("unused"),
        false,
    );
    assert_eq!(manager.start().unwrap().status, "disabled");
    assert_eq!(manager.snapshot().unwrap().status, "disabled");
}

/// 驗證當已有背景工作者執行時，重複呼叫 `start` 會沿用現有工作者並維持 `"checking"` 狀態。
#[test]
fn concurrent_start_requests_share_one_background_worker() {
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        Path::new("unused"),
        true,
    );
    manager.worker_active.store(true, Ordering::Release);
    assert_eq!(manager.start().unwrap().status, "checking");
    assert!(manager.worker_active.load(Ordering::Acquire));
}

/// 驗證依賴檢查輸出中的每筆 `- ` 缺失項目都能被正確解析成清單。
#[test]
fn check_output_reports_each_missing_runtime_component() {
    let missing = parse_missing_dependencies(
            "Missing runtime dependencies:\n  - uxplay 1.73.7 or newer\n  - GStreamer element avdec_aac\n",
        );
    assert_eq!(
        missing,
        ["uxplay 1.73.7 or newer", "GStreamer element avdec_aac"]
    );
}

/// 驗證 `InstallResult` 反序列化時強制要求非空的 `pid` 與 `exitCode` 欄位。
#[test]
fn install_result_requires_pid_and_exit_code() {
    let parsed: InstallResult = serde_json::from_str(r#"{"pid":123,"exitCode":7}"#).unwrap();
    assert_eq!(parsed.pid, 123);
    assert_eq!(parsed.exit_code, 7);
    assert!(parsed.stage.is_none());
    assert!(parsed.message.is_none());
    assert!(serde_json::from_str::<InstallResult>(r#"{"pid":123}"#).is_err());
}

/// 驗證安裝失敗結果組裝的 `detail()` 會呈現中文階段標籤與腳本錯誤摘要。
#[test]
fn install_failure_includes_the_stage_and_summary() {
    let result: InstallResult = serde_json::from_str(
        r#"{"pid":123,"exitCode":1,"stage":"repairing","message":"編譯工具修復後仍無法使用。"}"#,
    )
    .unwrap();
    assert!(result.detail().contains("修復編譯工具"));
    assert!(result.detail().contains("編譯工具修復後仍無法使用。"));
}

/// 驗證 `wait_for_install` 能即時同步進度訊息並在完成時正確回傳結果。
#[test]
fn installation_reports_progress_and_a_completion_result() {
    let (directory, snapshot, child) = install_test_process("complete");
    let result = wait_for_install(
        Path::new("unused"),
        &directory,
        Some(child.id()),
        Some(child),
        None,
        &snapshot,
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(snapshot.lock().unwrap().message, "正在編譯投影工具…");
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證安裝子程序中途中止未寫入結果檔時，會組裝含退出碼與階段名稱的失敗訊息並清除鎖定標記檔以便重試。
#[test]
fn interrupted_installation_preserves_exit_status_and_allows_retry() {
    let (directory, snapshot, child) = install_test_process("interrupt");
    fs::write(directory.join(INSTALL_LAUNCH), b"attempt in progress").unwrap();
    let detail = wait_for_install(
        Path::new("unused"),
        &directory,
        Some(child.id()),
        Some(child),
        None,
        &snapshot,
    )
    .unwrap_err();
    assert!(detail.contains("11"));
    assert!(detail.contains("編譯 AirPlay 工具"));
    assert!(!directory.join(INSTALL_MARKER).exists());
    assert!(!directory.join(INSTALL_LAUNCH).exists());
    assert!(!directory.join(INSTALL_RESULT).exists());
    fs::remove_dir_all(directory).unwrap();
}

/// 作為 `wait_for_install` 測試啟動的子程序替身，依據 `NEO_AIRPLAY_TEST_INSTALL_MODE` 模擬正常完成或異常中斷。
#[test]
fn install_test_child_process() {
    let Ok(mode) = std::env::var("NEO_AIRPLAY_TEST_INSTALL_MODE") else {
        return;
    };
    let directory = PathBuf::from(std::env::var_os("NEO_AIRPLAY_TEST_INSTALL_DIR").unwrap());
    fs::write(
        directory.join(INSTALL_MARKER),
        std::process::id().to_string(),
    )
    .unwrap();
    write_json_atomic(
        &directory.join(INSTALL_PROGRESS),
        &InstallProgress {
            pid: std::process::id(),
            stage: "uxplay-build".into(),
            message: "正在編譯投影工具…".into(),
        },
    )
    .unwrap();
    if mode == "interrupt" {
        std::process::exit(11);
    }
    thread::sleep(Duration::from_millis(100));
    write_json_atomic(
        &directory.join(INSTALL_RESULT),
        &InstallResult {
            pid: std::process::id(),
            exit_code: 0,
            stage: None,
            message: None,
        },
    )
    .unwrap();
}

/// 建立安裝日誌測試專用的隔離暫存目錄。
///
/// # 回傳值
/// 回傳位於系統暫存目錄下的唯一測試目錄 `PathBuf`。
fn install_log_test_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neo-install-log-test-{}-{nonce}",
        std::process::id()
    ))
}

/// 啟動執行 `install_test_child_process` 的子程序，並建立測試用暫存目錄與初始快照。
///
/// # 參數
/// - `mode`: 指定模擬行為模式（例如 `"complete"` 或 `"interrupt"`）。
///
/// # 回傳值
/// 回傳 `(directory, snapshot, child)` 元組供安裝等待邏輯測試使用。
fn install_test_process(mode: &str) -> (PathBuf, Arc<Mutex<DependencySnapshot>>, Child) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("neo-install-test-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let snapshot = Arc::new(Mutex::new(DependencySnapshot {
        status: "installing".into(),
        operation: Some(DependencyOperation::Install),
        message: "waiting".into(),
        missing: Vec::new(),
        log_path: String::new(),
    }));
    let child =
        crate::infrastructure::process::background_command(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "infrastructure::dependencies::tests::install_test_child_process",
                "--nocapture",
            ])
            .env("NEO_AIRPLAY_TEST_INSTALL_MODE", mode)
            .env("NEO_AIRPLAY_TEST_INSTALL_DIR", &directory)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
    (directory, snapshot, child)
}

/// 驗證清除重裝時僅移除應用程式專用工具與指定的 GStreamer registry，保留 `.GITKEEP`、`.bundled-*` 與其他快取檔。
#[test]
fn clean_reinstall_clears_only_engine_files_and_the_selected_cache_file() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "neo-clean-tools-test-{}-{nonce}",
        std::process::id()
    ));
    let engine = directory.join("engine");
    let cache = directory.join("cache/gstreamer/registry.bin");
    fs::create_dir_all(engine.join("bin")).unwrap();
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    fs::write(engine.join(".GITKEEP"), b"tracked\n").unwrap();
    fs::write(engine.join(".bundled-0.2.3"), b"seeded\n").unwrap();
    fs::write(engine.join("bin/uxplay"), b"tool").unwrap();
    fs::write(&cache, b"registry").unwrap();
    fs::write(directory.join("cache/keep.txt"), b"other cache").unwrap();

    clear_app_tools(&engine, &cache).unwrap();

    assert!(engine.join(".GITKEEP").is_file());
    assert!(engine.join(".bundled-0.2.3").is_file());
    assert!(!engine.join("bin/uxplay").exists());
    assert!(!cache.exists());
    assert_eq!(
        fs::read(directory.join("cache/keep.txt")).unwrap(),
        b"other cache"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證升級前只會使目標 GStreamer registry 快取失效而不波及同目錄其他快取檔案。
#[test]
fn upgrade_invalidates_only_the_selected_gstreamer_registry() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "neo-upgrade-cache-test-{}-{nonce}",
        std::process::id()
    ));
    let registry = directory.join("cache/gstreamer/registry.bin");
    fs::create_dir_all(registry.parent().unwrap()).unwrap();
    fs::write(&registry, b"registry").unwrap();
    fs::write(directory.join("cache/keep.txt"), b"other cache").unwrap();

    clear_gstreamer_registry(&registry).unwrap();

    assert!(!registry.exists());
    assert_eq!(
        fs::read(directory.join("cache/keep.txt")).unwrap(),
        b"other cache"
    );
    fs::remove_dir_all(directory).unwrap();
}

/// 驗證背景依賴工作者活動期間 `with_engine_access` 會拒絕執行並避免呼入動作閉包。
#[test]
fn engine_access_is_blocked_while_a_dependency_worker_is_active() {
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        Path::new("unused"),
        true,
    );
    manager.worker_active.store(true, Ordering::Release);
    let mut called = false;
    let result = manager.with_engine_access(|| {
        called = true;
        Ok(())
    });
    assert!(result.is_err());
    assert!(!called);
}

/// 驗證 Linux 安裝命令會透過獨立參數傳遞含空白目錄路徑與 `--upgrade`，不經由 Shell 字串內插。
#[cfg(target_os = "linux")]
#[test]
fn installer_command_passes_upgrade_mode_without_shell_interpolation() {
    let command = installer_command(
        Path::new("installer script.sh"),
        Path::new("engine directory"),
        Path::new("marker file"),
        Path::new("result file"),
        true,
    );
    let arguments = command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(arguments.last().map(String::as_str), Some("--upgrade"));
    assert!(arguments
        .iter()
        .any(|argument| argument == "engine directory"));
}

/// 驗證 `tasklist_contains_pid` 會精確比對完整 PID 欄位而不會誤認前綴相符的 PID。
#[test]
fn tasklist_pid_matching_does_not_accept_prefixes() {
    let output = Output {
        status: std::process::ExitStatus::default(),
        stdout: b"\"powershell.exe\",\"1234\",\"Console\"\n".to_vec(),
        stderr: Vec::new(),
    };
    assert!(tasklist_contains_pid(&output, 1234));
    assert!(!tasklist_contains_pid(&output, 123));
}

/// 驗證 macOS 終端指令參數跳脫函式會正確保護空白與反引號／命令替換字元。
#[cfg(target_os = "macos")]
#[test]
fn terminal_paths_are_shell_quoted() {
    assert_eq!(
        shell_quote("/tmp/a b/'$(touch injected)'"),
        "'/tmp/a b/'\\''$(touch injected)'\\'''"
    );
}
