use super::*;

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

#[test]
fn install_result_requires_pid_and_exit_code() {
    let parsed: InstallResult = serde_json::from_str(r#"{"pid":123,"exitCode":7}"#).unwrap();
    assert_eq!(parsed.pid, 123);
    assert_eq!(parsed.exit_code, 7);
    assert!(parsed.stage.is_none());
    assert!(parsed.message.is_none());
    assert!(serde_json::from_str::<InstallResult>(r#"{"pid":123}"#).is_err());
}

#[test]
fn install_failure_includes_the_stage_and_summary() {
    let result: InstallResult = serde_json::from_str(
        r#"{"pid":123,"exitCode":1,"stage":"repairing","message":"編譯工具修復後仍無法使用。"}"#,
    )
    .unwrap();
    assert!(result.detail().contains("修復編譯工具"));
    assert!(result.detail().contains("編譯工具修復後仍無法使用。"));
}

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
        message: "waiting".into(),
        missing: Vec::new(),
        log_path: String::new(),
    }));
    let child = crate::state::background_command(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "dependencies::tests::install_test_child_process",
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

#[cfg(target_os = "macos")]
#[test]
fn terminal_paths_are_shell_quoted() {
    assert_eq!(
        shell_quote("/tmp/a b/'$(touch injected)'"),
        "'/tmp/a b/'\\''$(touch injected)'\\'''"
    );
}
