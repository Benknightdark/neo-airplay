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
            resolved_sha256: None,
        },
    )
    .unwrap();
}

/// 同一測試程序內遞增的暫存目錄序號，避免 macOS 時鐘僅有微秒精度時平行測試取得相同路徑。
static TEST_DIRECTORY_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 建立含 PID、時間戳與遞增序號的唯一暫存測試目錄路徑。
///
/// # 參數
/// - `prefix`: 測試目錄名稱前綴。
///
/// # 回傳值
/// 回傳位於系統暫存目錄下且不與平行測試衝突的 `PathBuf`。
fn unique_test_directory(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

/// 建立安裝日誌測試專用的隔離暫存目錄。
///
/// # 回傳值
/// 回傳位於系統暫存目錄下的唯一測試目錄 `PathBuf`。
fn install_log_test_directory() -> PathBuf {
    unique_test_directory("neo-install-log-test")
}

/// 啟動執行 `install_test_child_process` 的子程序，並建立測試用暫存目錄與初始快照。
///
/// # 參數
/// - `mode`: 指定模擬行為模式（例如 `"complete"` 或 `"interrupt"`）。
///
/// # 回傳值
/// 回傳 `(directory, snapshot, child)` 元組供安裝等待邏輯測試使用。
fn install_test_process(mode: &str) -> (PathBuf, Arc<Mutex<DependencySnapshot>>, Child) {
    let directory = unique_test_directory("neo-install-test");
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
    let directory = unique_test_directory("neo-clean-tools-test");
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
    let directory = unique_test_directory("neo-upgrade-cache-test");
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
#[test]
fn terminal_paths_are_shell_quoted() {
    assert_eq!(
        shell_quote("/tmp/a b/'$(touch injected)'"),
        "'/tmp/a b/'\\''$(touch injected)'\\'''"
    );
}

/// 驗證 macOS `osascript` 會透過 `on run argv` 傳遞命令列並拒絕換行與 NUL 控制字元。
#[test]
fn macos_terminal_osascript_uses_argv_and_rejects_control_characters() {
    let args = macos_terminal_osascript_args("/bin/bash '/tmp/safe path/launch.sh'").unwrap();
    assert_eq!(args[0], "-e");
    assert_eq!(args[1], "on run argv");
    assert_eq!(
        args[3],
        "tell application \"Terminal\" to do script (item 1 of argv)"
    );
    assert_eq!(args[6], "--");
    assert_eq!(args[7], "/bin/bash '/tmp/safe path/launch.sh'");

    assert!(macos_terminal_osascript_args("echo hi\nrm -rf /").is_err());
    assert!(macos_terminal_osascript_args("echo hi\rwhoami").is_err());
    assert!(macos_terminal_osascript_args("echo hi\0bad").is_err());
    assert!(validate_terminal_path(Path::new("/tmp/bad\npath")).is_err());
    assert!(validate_terminal_path(Path::new("/tmp/good path")).is_ok());
}

/// 驗證 Windows PowerShell 系統路徑解析會採用 `SystemRoot\System32` 並忽略 UNC 或相對路徑。
#[test]
fn windows_powershell_path_ignores_unc_and_relative_system_roots() {
    let default_path = windows_powershell_executable_from(None);
    assert!(default_path.to_string_lossy().contains("WindowsPowerShell"));
    assert!(default_path.to_string_lossy().ends_with("powershell.exe"));

    let custom_path = windows_powershell_executable_from(Some(OsStr::new(r"D:\Windows")));
    assert!(custom_path.to_string_lossy().starts_with(r"D:\Windows"));

    let unc_fallback = windows_powershell_executable_from(Some(OsStr::new(r"\\attacker\share")));
    assert!(unc_fallback.to_string_lossy().starts_with(r"C:\Windows"));

    let rel_fallback = windows_powershell_executable_from(Some(OsStr::new("Windows")));
    assert!(rel_fallback.to_string_lossy().starts_with(r"C:\Windows"));
}

/// 驗證 `read_install_log` 會保留正常大小日誌並截斷超過 1 MiB 的超大日誌。
#[test]
fn read_install_log_caps_oversized_logs_to_tail() {
    let config_dir = install_log_test_directory();
    let setup_dir = config_dir.join("setup");
    fs::create_dir_all(&setup_dir).unwrap();
    let line = "0123456789abcdef0123456789abcdef\n";
    let repeated = line.repeat(40_000);
    assert!(repeated.len() > MAX_INSTALL_LOG_READ_BYTES);
    fs::write(setup_dir.join(INSTALL_LOG), &repeated).unwrap();
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        &config_dir,
        true,
    );

    let read_back = manager.read_install_log().unwrap();
    assert!(read_back.starts_with("[已省略較早記錄，僅顯示最後 1 MiB 內容]\n"));
    assert!(read_back.len() <= MAX_INSTALL_LOG_READ_BYTES + 128);
    assert!(read_back.ends_with(line));
    fs::remove_dir_all(config_dir).unwrap();
}

/// 驗證 `stage_installer` 在 Unix 會套用 `0700` 目錄／腳本權限且拒絕符號連結路徑與目標。
#[cfg(unix)]
#[test]
fn stage_installer_enforces_permissions_and_rejects_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let root = install_log_test_directory();
    let setup_dir = root.join("setup");
    let staged = stage_installer(&setup_dir).unwrap();
    let dir_mode = fs::symlink_metadata(&setup_dir)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    let file_mode = fs::symlink_metadata(&staged).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700);
    assert_eq!(file_mode, 0o700);

    let outside_target = root.join("outside.txt");
    fs::write(&outside_target, b"original").unwrap();
    fs::remove_file(&staged).unwrap();
    symlink(&outside_target, &staged).unwrap();
    assert!(stage_installer(&setup_dir).is_err());
    assert_eq!(fs::read(&outside_target).unwrap(), b"original");

    let symlink_setup = root.join("symlink-setup");
    symlink(&setup_dir, &symlink_setup).unwrap();
    assert!(stage_installer(&symlink_setup).is_err());

    let symlink_log = setup_dir.join(INSTALL_LOG);
    symlink(&outside_target, &symlink_log).unwrap();
    let manager = DependencyManager::new(
        PathBuf::from("unused"),
        PathBuf::from("unused-cache"),
        &root,
        true,
    );
    assert!(manager.read_install_log().is_err());

    fs::remove_dir_all(root).unwrap();
}

/// 驗證 `clear_engine_directory` 拒絕懸空符號連結且 `clear_gstreamer_registry` 拒絕符號連結快取路徑。
#[cfg(unix)]
#[test]
fn cleanup_rejects_dangling_and_symlinked_paths() {
    use std::os::unix::fs::symlink;

    let root = install_log_test_directory();
    fs::create_dir_all(&root).unwrap();
    let dangling_engine = root.join("dangling-engine");
    symlink(root.join("missing-target"), &dangling_engine).unwrap();
    assert!(clear_engine_directory(&dangling_engine).is_err());

    let real_cache_dir = root.join("cache");
    fs::create_dir_all(&real_cache_dir).unwrap();
    let target_file = root.join("outside-keep.txt");
    fs::write(&target_file, b"keep").unwrap();
    let symlink_registry = real_cache_dir.join("registry.bin");
    symlink(&target_file, &symlink_registry).unwrap();
    assert!(clear_gstreamer_registry(&symlink_registry).is_err());
    assert_eq!(fs::read(&target_file).unwrap(), b"keep");

    let symlink_cache_dir = root.join("symlink-cache");
    symlink(&real_cache_dir, &symlink_cache_dir).unwrap();
    assert!(clear_gstreamer_registry(&symlink_cache_dir.join("registry.bin")).is_err());

    fs::remove_dir_all(root).unwrap();
}

/// 驗證 `seed_bundled_engine_in_dir` 會安全複製常規檔案、略過來源符號連結並拒絕目的地符號連結。
#[cfg(unix)]
#[test]
fn seed_bundled_engine_ignores_source_symlinks_and_rejects_destination_symlinks() {
    use crate::infrastructure::process::seed_bundled_engine_in_dir;
    use std::os::unix::fs::{symlink, PermissionsExt};

    let root = install_log_test_directory();
    let bundled = root.join("bundled");
    let engine = root.join("engine");
    let outside = root.join("outside-secret");
    fs::create_dir_all(bundled.join("bin")).unwrap();
    fs::write(&outside, b"secret").unwrap();
    fs::write(bundled.join("bin/uxplay"), b"binary").unwrap();
    fs::set_permissions(
        bundled.join("bin/uxplay"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    symlink(&outside, bundled.join("bin/smuggled")).unwrap();

    seed_bundled_engine_in_dir(&bundled, &engine, "0.2.5").unwrap();
    assert!(engine.join("bin/uxplay").is_file());
    assert!(!engine.join("bin/smuggled").exists());
    assert!(engine.join(".bundled-0.2.5").is_file());

    let tampered_engine = root.join("tampered-engine");
    fs::create_dir_all(tampered_engine.join("bin")).unwrap();
    symlink(&outside, tampered_engine.join("bin/uxplay")).unwrap();
    assert!(seed_bundled_engine_in_dir(&bundled, &tampered_engine, "0.2.5").is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"secret");

    fs::remove_dir_all(root).unwrap();
}

/// 驗證 Unix 與 Windows 安裝器引數建構函式正確傳遞 `--only`、自訂版本、SHA-256 與 `--allow-unverified-uxplay` 參數。
#[test]
fn extra_installer_args_encode_selected_tool_versions_without_shell_interpolation() {
    use super::{
        unix_extra_installer_args, windows_extra_installer_args, InstallInvocationOptions,
    };
    use crate::domain::{PinnedToolVersion, ToolId};

    let options = InstallInvocationOptions {
        only_tool: Some(ToolId::UxPlay),
        uxplay: Some(PinnedToolVersion {
            version: "1.73.6".into(),
            reference: "v1.73.6".into(),
            sha256: None,
            allow_unverified: true,
        }),
        scrcpy: Some(PinnedToolVersion {
            version: "4.1".into(),
            reference: "v4.1".into(),
            sha256: Some("a".repeat(64)),
            allow_unverified: false,
        }),
        validated_install: None,
    };

    let unix = unix_extra_installer_args(false, &options);
    assert_eq!(
        unix,
        vec![
            "--only",
            "uxplay",
            "--uxplay-ref",
            "v1.73.6",
            "--expect-uxplay-version",
            "1.73.6",
            "--allow-unverified-uxplay",
            "--scrcpy-version",
            "4.1",
            "--scrcpy-sha256",
            &"a".repeat(64),
        ]
    );

    let windows = windows_extra_installer_args(true, &options);
    assert_eq!(
        windows,
        vec![
            "-Upgrade",
            "-Only",
            "uxplay",
            "-UxPlayRef",
            "v1.73.6",
            "-ExpectUxPlayVersion",
            "1.73.6",
            "-AllowUnverifiedUxPlay",
            "-ScrcpyVersionOverride",
            "4.1",
            "-ScrcpySha256",
            &"a".repeat(64),
        ]
    );
}

/// 驗證 `tool-versions.json` 安全持久化（寫入、讀取、恢復預設、拒絕符號連結）與 API 失敗回退 Atom XML 爬取流程。
#[test]
fn tool_version_preferences_and_catalog_atom_fallback_work() {
    use super::{
        load_tool_version_preferences, reset_tool_version_preferences, save_tool_version_preference,
    };
    use crate::application::ToolCatalogFetcher;
    use crate::domain::{DomainError, DomainResult, ToolId, ValidatedToolInstall};

    let config_dir = install_log_test_directory();
    fs::create_dir_all(&config_dir).unwrap();

    let custom_scrcpy = ValidatedToolInstall {
        tool: ToolId::Scrcpy,
        version: "4.1".into(),
        reference: "v4.1".into(),
        sha256: None,
        allow_unverified: false,
        is_default: false,
    };
    save_tool_version_preference(&config_dir, &custom_scrcpy).unwrap();
    let loaded = load_tool_version_preferences(&config_dir);
    assert_eq!(loaded.scrcpy.as_ref().unwrap().version, "4.1");

    reset_tool_version_preferences(&config_dir).unwrap();
    assert!(load_tool_version_preferences(&config_dir).scrcpy.is_none());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = config_dir.join("outside.json");
        fs::write(&outside, b"{}").unwrap();
        symlink(&outside, config_dir.join(super::TOOL_VERSIONS_FILE)).unwrap();
        assert!(save_tool_version_preference(&config_dir, &custom_scrcpy).is_err());
        assert!(reset_tool_version_preferences(&config_dir).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"{}");
    }

    struct FallbackFetcher;
    impl ToolCatalogFetcher for FallbackFetcher {
        fn fetch_text(&self, url: &str) -> DomainResult<String> {
            if url.contains("api.github.com") {
                return Err(DomainError::external_process("HTTP 403 rate limited"));
            }
            if url.contains("scrcpy/releases.atom") {
                return Ok(r#"<feed><entry><link href="https://github.com/Genymobile/scrcpy/releases/tag/v5.0.1"/><updated>2026-10-08T00:00:00Z</updated></entry></feed>"#.into());
            }
            if url.contains("UxPlay/releases.atom") {
                return Ok(r#"<feed><entry><link href="https://github.com/FDH2/UxPlay/releases/tag/v1.73.7"/><updated>2026-09-10T00:00:00Z</updated></entry></feed>"#.into());
            }
            Ok(r#"{"versions":{"stable":"1.28.9"}}"#.into())
        }
    }

    let manager = DependencyManager::new(
        config_dir.join("engine"),
        config_dir.join("cache/registry.bin"),
        &config_dir,
        false,
    );
    let catalog = manager.refresh_tool_catalog(&FallbackFetcher).unwrap();
    assert!(catalog.remote_fetched_at.is_some());
    assert_eq!(
        catalog.tools[0].latest_remote_version.as_deref(),
        Some("1.73.7")
    );
    assert_eq!(catalog.tools[1].available_versions[0].version, "5.0.1");

    fs::remove_dir_all(&config_dir).unwrap();
}

/// 驗證受支援 Linux 發行版從 `/etc/os-release` 解析版本代號，並拒絕未支援衍生版與不安全路徑片段。
#[test]
fn linux_distribution_detection_uses_supported_ids_and_safe_release_values() {
    assert_eq!(
        parse_linux_distribution("ID=debian\nVERSION_CODENAME=trixie\n").unwrap(),
        LinuxDistribution::Debian {
            codename: "trixie".into()
        }
    );
    assert_eq!(
        parse_linux_distribution("ID=ubuntu\nUBUNTU_CODENAME=noble\n").unwrap(),
        LinuxDistribution::Ubuntu {
            codename: "noble".into()
        }
    );
    assert_eq!(
        parse_linux_distribution("ID=fedora\nVERSION_ID=45\n").unwrap(),
        LinuxDistribution::Fedora {
            release: "45".into()
        }
    );
    assert_eq!(
        parse_linux_distribution("ID=rhel\nVERSION_ID=10.0\n").unwrap(),
        LinuxDistribution::Rhel
    );
    assert_eq!(
        parse_linux_distribution("ID=arch\n").unwrap(),
        LinuxDistribution::Arch
    );
    assert!(parse_linux_distribution("ID=rocky\nID_LIKE=rhel fedora\n").is_err());
    assert!(parse_linux_distribution("ID=ubuntu\nVERSION_CODENAME=../noble\n").is_err());
}

/// 驗證 Linux 各發行版選用白名單官方 GStreamer 套件頁並取得平台套件版本。
#[test]
fn linux_gstreamer_queries_use_official_distribution_sources() {
    use crate::application::ToolCatalogFetcher;
    use crate::domain::{DomainResult, ARCH_GSTREAMER_PACKAGES_URL, FEDORA_GSTREAMER_PACKAGES_URL};

    struct FixtureFetcher {
        expected_url: &'static str,
        body: &'static str,
    }
    impl ToolCatalogFetcher for FixtureFetcher {
        fn fetch_text(&self, url: &str) -> DomainResult<String> {
            assert_eq!(url, self.expected_url);
            Ok(self.body.into())
        }
    }

    assert_eq!(
        fetch_linux_gstreamer_latest(
            &FixtureFetcher {
                expected_url: "https://packages.debian.org/trixie/utils/gstreamer1.0-tools",
                body: "<h1>Package: gstreamer1.0-tools (1.26.2-2)</h1>",
            },
            &LinuxDistribution::Debian {
                codename: "trixie".into()
            },
            "x86_64",
        )
        .unwrap(),
        "1.26.2-2"
    );
    assert_eq!(
        fetch_linux_gstreamer_latest(
            &FixtureFetcher {
                expected_url: "https://packages.ubuntu.com/noble/utils/gstreamer1.0-tools",
                body: "<h1>Package: gstreamer1.0-tools (1.24.2-1ubuntu0.1 and others)</h1>",
            },
            &LinuxDistribution::Ubuntu {
                codename: "noble".into()
            },
            "aarch64",
        )
        .unwrap(),
        "1.24.2-1ubuntu0.1"
    );
    assert_eq!(
        fetch_linux_gstreamer_latest(
            &FixtureFetcher {
                expected_url: FEDORA_GSTREAMER_PACKAGES_URL,
                body: "<table><tr><td>Fedora 45</td><td><a>1.28.9-1.fc45</a></td></tr></table>",
            },
            &LinuxDistribution::Fedora {
                release: "45".into()
            },
            "x86_64",
        )
        .unwrap(),
        "1.28.9-1.fc45"
    );
    assert_eq!(
        fetch_linux_gstreamer_latest(
            &FixtureFetcher {
                expected_url: ARCH_GSTREAMER_PACKAGES_URL,
                body: "<h2>gstreamer 1.28.9-3</h2>",
            },
            &LinuxDistribution::Arch,
            "x86_64",
        )
        .unwrap(),
        "1.28.9-3"
    );
    assert!(fetch_linux_gstreamer_latest(
        &FixtureFetcher {
            expected_url: ARCH_GSTREAMER_PACKAGES_URL,
            body: "<h2>gstreamer 1.28.9-3</h2>",
        },
        &LinuxDistribution::Arch,
        "aarch64",
    )
    .is_err());
}

/// 驗證 RHEL 版本查詢只呼叫唯讀 `dnf repoquery` 並限制輸出、參數與執行時間。
#[test]
fn rhel_gstreamer_query_is_bounded_and_read_only() {
    assert_eq!(
        RHEL_GSTREAMER_DNF_ARGS,
        [
            "--quiet",
            "repoquery",
            "--latest-limit=1",
            "--queryformat=%{version}-%{release}",
            "gstreamer1-tools",
        ]
    );
    assert_eq!(RHEL_GSTREAMER_QUERY_TIMEOUT, Duration::from_secs(15));
}

/// 驗證「清除工具」會移除 App 專用工具、GStreamer 快取、`tool-versions.json` 與過渡性標記，
/// 並保留 `.GITKEEP`、`.bundled-*`、`settings.json` 與 `install.log`，最後轉為 `failed` 與缺漏清單。
#[test]
fn clear_installed_tools_clears_tools_caches_and_preferences_while_preserving_logs_and_settings() {
    let root = install_log_test_directory();
    let engine = root.join("engine");
    let cache = root.join("cache/gstreamer/registry.bin");
    let config = root.join("config");
    let setup = config.join("setup");

    fs::create_dir_all(engine.join("bin")).unwrap();
    fs::create_dir_all(engine.join("lib/gstreamer-1.0")).unwrap();
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    fs::create_dir_all(&setup).unwrap();

    fs::write(engine.join(".GITKEEP"), b"tracked\n").unwrap();
    fs::write(engine.join(".bundled-0.2.5"), b"seeded\n").unwrap();
    fs::write(engine.join("bin/uxplay"), b"fake-uxplay").unwrap();
    fs::write(engine.join("bin/scrcpy"), b"fake-scrcpy").unwrap();
    fs::write(engine.join("lib/gstreamer-1.0/libgst.so"), b"plugin").unwrap();
    fs::write(&cache, b"registry-data").unwrap();
    fs::write(
        config.join("settings.json"),
        b"{\"receiverName\":\"Living Room\"}",
    )
    .unwrap();
    fs::write(
        config.join(TOOL_VERSIONS_FILE),
        b"{\"scrcpy\":{\"version\":\"4.1\",\"reference\":\"v4.1\",\"allowUnverified\":false}}",
    )
    .unwrap();
    fs::write(setup.join(INSTALL_LOG), b"previous install log\n").unwrap();
    fs::write(setup.join(INSTALL_MARKER), b"999999\n").unwrap();
    fs::write(setup.join(INSTALL_RESULT), b"{\"pid\":1,\"exitCode\":0}").unwrap();
    fs::write(
        setup.join(INSTALL_LAUNCH),
        b"{\"ownerPid\":1,\"startedAt\":1}",
    )
    .unwrap();
    fs::write(
        setup.join(INSTALL_PROGRESS),
        b"{\"pid\":1,\"stage\":\"ready\",\"message\":\"done\"}",
    )
    .unwrap();
    fs::write(setup.join("launch.sh"), b"#!/bin/bash\n").unwrap();

    let snapshot = Arc::new(Mutex::new(DependencySnapshot {
        status: "ready".into(),
        operation: Some(DependencyOperation::Clear),
        message: "正在準備清除已安裝的投影工具…".into(),
        missing: Vec::new(),
        log_path: setup.join(INSTALL_LOG).to_string_lossy().into_owned(),
    }));

    let outcome = clear_installed_tools(&engine, &cache, &setup, &config, &snapshot).unwrap();

    assert_eq!(outcome.status, "failed");
    assert!(outcome
        .message
        .contains("已清除所有 Neo AirPlay 專用工具與快取"));
    assert!(!outcome.missing.is_empty());

    assert!(engine.join(".GITKEEP").is_file());
    assert!(engine.join(".bundled-0.2.5").is_file());
    assert!(!engine.join("bin/uxplay").exists());
    assert!(!engine.join("bin/scrcpy").exists());
    assert!(!engine.join("lib").exists());
    assert!(!cache.exists());
    assert!(!config.join(TOOL_VERSIONS_FILE).exists());
    assert!(!setup.join(INSTALL_MARKER).exists());
    assert!(!setup.join(INSTALL_RESULT).exists());
    assert!(!setup.join(INSTALL_LAUNCH).exists());
    assert!(!setup.join(INSTALL_PROGRESS).exists());
    assert!(!setup.join("launch.sh").exists());

    assert_eq!(
        fs::read_to_string(config.join("settings.json")).unwrap(),
        "{\"receiverName\":\"Living Room\"}"
    );
    let log = fs::read_to_string(setup.join(INSTALL_LOG)).unwrap();
    assert!(log.contains("previous install log"));
    assert!(log.contains("已清除所有 Neo AirPlay 專用投影工具"));

    fs::remove_dir_all(&root).unwrap();
}

/// 驗證透過 `DependencyManager::manage(DependencyAction::Clear, ...)` 能非同步完成清除並重置工具目錄狀態。
#[test]
fn dependency_manager_clear_action_executes_and_updates_snapshot() {
    let root = install_log_test_directory();
    let engine = root.join("engine");
    let cache = root.join("cache/gstreamer/registry.bin");
    let config = root.join("config");
    fs::create_dir_all(engine.join("bin")).unwrap();
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    fs::create_dir_all(&config).unwrap();
    fs::write(engine.join("bin/uxplay"), b"fake").unwrap();
    fs::write(&cache, b"registry").unwrap();

    let manager = DependencyManager::new(engine.clone(), cache.clone(), &config, true);
    let initial = manager
        .manage(DependencyAction::Clear, &mut || Ok(()))
        .unwrap();
    assert_eq!(initial.operation, Some(DependencyOperation::Clear));

    let deadline = Instant::now() + Duration::from_secs(10);
    while manager.worker_active.load(Ordering::Acquire) {
        assert!(
            Instant::now() < deadline,
            "清除工具背景工作者未在期限內結束"
        );
        thread::sleep(Duration::from_millis(20));
    }

    let final_snapshot = manager.snapshot().unwrap();
    assert_eq!(final_snapshot.status, "failed");
    assert_eq!(final_snapshot.operation, Some(DependencyOperation::Clear));
    assert!(!final_snapshot.missing.is_empty());
    assert!(!engine.join("bin/uxplay").exists());
    assert!(!cache.exists());

    let catalog = manager.tool_catalog().unwrap();
    assert!(catalog
        .tools
        .iter()
        .all(|tool| tool.installed_version.is_none()));

    fs::remove_dir_all(&root).unwrap();
}

/// 驗證 `clear_transient_setup_files` 與 `clear_installed_tools` 拒絕符號連結路徑且不刪除外部目標檔案。
#[cfg(unix)]
#[test]
fn clear_transient_setup_files_rejects_symlinks() {
    use std::os::unix::fs::symlink;

    let root = install_log_test_directory();
    let setup = root.join("setup");
    let outside = root.join("outside-keep.txt");
    fs::create_dir_all(&setup).unwrap();
    fs::write(&outside, b"keep").unwrap();
    symlink(&outside, setup.join(INSTALL_MARKER)).unwrap();

    assert!(clear_transient_setup_files(&setup).is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"keep");

    fs::remove_dir_all(&root).unwrap();
}

/// 驗證首次透過官方 HTTPS 安裝自訂 UxPlay 成功後，`apply_tofu_sha256` 會自動鎖定雜湊並停用未驗證旗標。
#[test]
fn apply_tofu_sha256_pins_valid_hash_and_clears_allow_unverified() {
    let validated = ValidatedToolInstall {
        tool: ToolId::UxPlay,
        version: "1.73.6".to_owned(),
        reference: "v1.73.6".to_owned(),
        sha256: None,
        allow_unverified: true,
        is_default: false,
    };
    let valid_sha = "168de53ed688f87c4022d79357ce91f9608ad5c53236bff95134d19b88efeb3f";
    let result = InstallResult {
        pid: 1,
        exit_code: 0,
        stage: None,
        message: Some("ok".to_owned()),
        resolved_sha256: Some(valid_sha.to_uppercase()),
    };

    let locked = apply_tofu_sha256(&validated, Some(&result));
    assert_eq!(locked.sha256.as_deref(), Some(valid_sha));
    assert!(!locked.allow_unverified);

    let bad_result = InstallResult {
        pid: 1,
        exit_code: 0,
        stage: None,
        message: Some("ok".to_owned()),
        resolved_sha256: Some("not-a-sha".to_owned()),
    };
    let unchanged = apply_tofu_sha256(&validated, Some(&bad_result));
    assert_eq!(unchanged.sha256, None);
    assert!(unchanged.allow_unverified);
}

/// 驗證未釘選且未允許 `allow_unverified` 的自訂 UxPlay 偏好設定載入時會被拒絕清除。
#[test]
fn load_tool_version_preferences_rejects_unpinned_uxplay_without_opt_in() {
    let config_dir = install_log_test_directory();
    fs::create_dir_all(&config_dir).unwrap();
    let prefs = ToolVersionPreferences {
        scrcpy: None,
        uxplay: Some(PinnedToolVersion {
            version: "1.73.6".to_owned(),
            reference: "v1.73.6".to_owned(),
            sha256: None,
            allow_unverified: false,
        }),
    };
    fs::write(
        config_dir.join(TOOL_VERSIONS_FILE),
        serde_json::to_string(&prefs).unwrap(),
    )
    .unwrap();

    let loaded = load_tool_version_preferences(&config_dir);
    assert!(loaded.uxplay.is_none());

    fs::remove_dir_all(&config_dir).unwrap();
}

/// 驗證 macOS 終端機啟動包裝腳本使用 `umask 077`、符號連結防護、`env -i` 與 Bash 3.2 安全陣列展開。
#[test]
fn build_macos_launch_script_enforces_hardened_wrapper() {
    let script = build_macos_launch_script(
        Path::new("/tmp/install.sh"),
        Path::new("/tmp/install.log"),
        Path::new("/tmp/engine"),
        Path::new("/tmp/install.done"),
        Path::new("/tmp/install-result.json"),
        &["--only".to_owned(), "uxplay".to_owned()],
    );
    assert!(script.contains("umask 077"));
    assert!(script.contains("if [ -L \"$target\" ]"));
    assert!(script.contains("exec /usr/bin/env -i"));
    assert!(script.contains("${extra_args[@]+\"${extra_args[@]}\"}"));
    assert!(script.contains("'--only' 'uxplay'"));
}

/// 驗證 `read_log_tail` 拒絕符號連結日誌檔案且不讀取外部目標內容。
#[cfg(unix)]
#[test]
fn read_log_tail_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let root = install_log_test_directory();
    let outside = root.join("secret.txt");
    let log_path = root.join("install.log");
    fs::create_dir_all(&root).unwrap();
    fs::write(&outside, b"secret-content").unwrap();
    symlink(&outside, &log_path).unwrap();

    let err = read_log_tail(&log_path, 4096).unwrap_err();
    assert!(!err.contains("secret-content"));

    fs::remove_dir_all(&root).unwrap();
}
