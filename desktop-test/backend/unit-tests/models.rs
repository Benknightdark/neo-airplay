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

/// 驗證 `AppLogEntry` 序列化為標準 RFC 3339（`Z`）格式，且可向下相容反序列化既有含時區偏移量的舊日誌。
#[test]
fn log_entry_timestamp_serializes_rfc3339_and_parses_legacy_offsets() {
    let entry = AppLogEntry::new(LogLevel::Warning, "test event");
    let serialized = serde_json::to_value(&entry).unwrap();
    let timestamp_str = serialized["timestamp"].as_str().unwrap();
    assert!(
        timestamp_str.ends_with('Z'),
        "預期序列化時間戳記應為 RFC 3339 UTC 格式：{timestamp_str}"
    );

    let legacy_json = r#"{"timestamp":"2026-11-13T12:00:00.123456+08:00","level":"information","message":"legacy"}"#;
    let parsed: AppLogEntry = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(parsed.timestamp.to_string(), "2026-11-13T04:00:00.123456Z");
    assert_eq!(parsed.level, LogLevel::Information);
    assert_eq!(parsed.message, "legacy");
}

/// 驗證版本與雜湊白名單規則能阻擋 Shell 注入、非官方網址與非法格式。
#[test]
fn tool_catalog_validation_enforces_allowlists_and_formats() {
    use super::tool_catalog::{
        validate_catalog_url, validate_optional_sha256, validate_scrcpy_version,
        validate_tool_install_request, validate_uxplay_ref, PlatformOs, ToolId, ToolInstallRequest,
        ToolReleaseOption, VersionControlMode,
    };

    assert!(validate_catalog_url("https://api.github.com/repos/FDH2/UxPlay/releases").is_ok());
    assert!(validate_catalog_url("https://formulae.brew.sh/api/formula/gstreamer.json").is_ok());
    assert!(validate_catalog_url(
        "https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-gstreamer"
    )
    .is_ok());
    assert!(
        validate_catalog_url("https://packages.debian.org/trixie/utils/gstreamer1.0-tools").is_ok()
    );
    assert!(
        validate_catalog_url("https://packages.ubuntu.com/noble/utils/gstreamer1.0-tools").is_ok()
    );
    assert!(
        validate_catalog_url("https://packages.fedoraproject.org/pkgs/gstreamer1/gstreamer1/")
            .is_ok()
    );
    assert!(validate_catalog_url("https://archlinux.org/packages/extra/x86_64/gstreamer/").is_ok());
    assert!(validate_catalog_url("http://api.github.com/repos/FDH2/UxPlay/releases").is_err());
    assert!(validate_catalog_url("https://evil.example/releases").is_err());
    assert!(validate_catalog_url("https://user@api.github.com/repos").is_err());

    assert_eq!(validate_scrcpy_version("v5.0.1").unwrap(), "5.0.1");
    assert_eq!(validate_scrcpy_version(" 4.1 ").unwrap(), "4.1");
    assert!(validate_scrcpy_version("5.0.1; rm -rf /").is_err());
    assert!(validate_scrcpy_version("--upgrade").is_err());

    let (ver, r) = validate_uxplay_ref("v1.73.6").unwrap();
    assert_eq!(ver, "1.73.6");
    assert_eq!(r, "v1.73.6");
    let sha = "a".repeat(40);
    let (ver_sha, ref_sha) = validate_uxplay_ref(&sha.to_uppercase()).unwrap();
    assert_eq!(ver_sha, sha);
    assert_eq!(ref_sha, sha);
    assert!(validate_uxplay_ref("../bad").is_err());

    assert_eq!(validate_optional_sha256(Some("   ")).unwrap(), None);
    assert!(validate_optional_sha256(Some("deadbeef")).is_err());
    assert_eq!(
        validate_optional_sha256(Some(&"F".repeat(64))).unwrap(),
        Some("f".repeat(64))
    );

    let unverified_req = ToolInstallRequest {
        tool: ToolId::UxPlay,
        version: "1.73.6".into(),
        sha256: None,
        allow_unverified: false,
    };
    assert!(
        validate_tool_install_request(&unverified_req, PlatformOs::Macos, "aarch64", &[]).is_err()
    );

    let known_remote = vec![ToolReleaseOption {
        version: "1.73.6".into(),
        reference: "21eef8df25d91e12635c36d8176ad192725baca2".into(),
        label: "v1.73.6".into(),
        published_at: Some("2026-09-01".into()),
        prerelease: false,
        recommended: false,
        sha256: None,
    }];
    let verified_req = ToolInstallRequest {
        allow_unverified: true,
        ..unverified_req
    };
    let validated =
        validate_tool_install_request(&verified_req, PlatformOs::Macos, "aarch64", &known_remote)
            .unwrap();
    assert_eq!(
        validated.reference,
        "21eef8df25d91e12635c36d8176ad192725baca2"
    );
    assert!(!validated.is_default);

    let arm64_linux_scrcpy = ToolInstallRequest {
        tool: ToolId::Scrcpy,
        version: "5.0.1".into(),
        sha256: None,
        allow_unverified: false,
    };
    assert!(
        validate_tool_install_request(&arm64_linux_scrcpy, PlatformOs::Linux, "aarch64", &[])
            .is_err()
    );
    assert!(validate_tool_install_request(
        &ToolInstallRequest {
            tool: ToolId::GStreamer,
            version: "1.28.9".into(),
            sha256: None,
            allow_unverified: false,
        },
        PlatformOs::Macos,
        "aarch64",
        &[],
    )
    .is_err());
    let _ = VersionControlMode::Selectable;
}

/// 驗證 GitHub API / Atom / Homebrew / MSYS2 / SHA256SUMS 與本機輸出純文字解析邏輯。
#[test]
fn tool_catalog_parsers_and_os_matrix_work_offline() {
    use super::tool_catalog::{
        build_tool_catalog_snapshot, parse_arch_gstreamer_version, parse_debian_gstreamer_version,
        parse_dnf_gstreamer_version, parse_fedora_gstreamer_version, parse_github_releases_atom,
        parse_github_scrcpy_releases_json, parse_github_uxplay_releases_json,
        parse_homebrew_gstreamer_version, parse_installed_adb_version,
        parse_installed_gstreamer_version, parse_installed_scrcpy_version,
        parse_installed_uxplay_version, parse_msys2_gstreamer_version, parse_scrcpy_sha256sums,
        parse_ubuntu_gstreamer_version, CatalogContext, InstalledVersions, PlatformOs,
        RemoteCatalogCache, ToolId, ToolVersionPreferences, VersionControlMode,
    };

    let scrcpy_json = r#"[
        {"tag_name":"v5.0.1","draft":false,"prerelease":false,"published_at":"2026-10-08T06:44:27Z","assets":[{"name":"scrcpy-macos-aarch64-v5.0.1.tar.gz"}]},
        {"tag_name":"v4.1","draft":false,"prerelease":false,"published_at":"2026-07-12T18:15:37Z","assets":[{"name":"scrcpy-win64-v4.1.zip"}]}
    ]"#;
    let mac_list =
        parse_github_scrcpy_releases_json(scrcpy_json, PlatformOs::Macos, "aarch64").unwrap();
    assert_eq!(mac_list.len(), 1);
    assert_eq!(mac_list[0].version, "5.0.1");

    let uxplay_json = r#"[{"tag_name":"v1.73.7","draft":false,"prerelease":false,"published_at":"2026-09-10T01:02:03Z"}]"#;
    let tags_json =
        r#"[{"name":"v1.73.7","commit":{"sha":"df67c212a433cf6dda3676dd40c097900d24e645"}}]"#;
    let ux_list = parse_github_uxplay_releases_json(uxplay_json, Some(tags_json)).unwrap();
    assert_eq!(
        ux_list[0].reference,
        "df67c212a433cf6dda3676dd40c097900d24e645"
    );

    let atom_xml = r#"<feed><entry><link href="https://github.com/Genymobile/scrcpy/releases/tag/v5.0"/><updated>2026-10-05T10:30:35Z</updated></entry></feed>"#;
    let atom_list = parse_github_releases_atom(atom_xml, ToolId::Scrcpy);
    assert_eq!(atom_list.len(), 1);
    assert_eq!(atom_list[0].version, "5.0");
    assert_eq!(atom_list[0].published_at.as_deref(), Some("2026-10-05"));

    let sums = "b12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5  scrcpy-win64-v5.0.1.zip\n";
    assert_eq!(
        parse_scrcpy_sha256sums(sums, "scrcpy-win64-v5.0.1.zip").as_deref(),
        Some("b12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5")
    );
    assert_eq!(
        parse_homebrew_gstreamer_version(r#"{"versions":{"stable":"1.28.9"}}"#).as_deref(),
        Some("1.28.9")
    );
    assert_eq!(
        parse_msys2_gstreamer_version("<dt>Version:</dt><dd class=\"mb-2\">1.28.9-1</dd>")
            .as_deref(),
        Some("1.28.9-1")
    );
    assert_eq!(
        parse_debian_gstreamer_version("<h1>Package: gstreamer1.0-tools (1:1.26.2+ds-2)</h1>")
            .as_deref(),
        Some("1:1.26.2+ds-2")
    );
    assert_eq!(
        parse_ubuntu_gstreamer_version(
            "<h1>Package: gstreamer1.0-tools (1.24.2-1ubuntu0.1 and others)</h1>"
        )
        .as_deref(),
        Some("1.24.2-1ubuntu0.1")
    );
    assert_eq!(
        parse_fedora_gstreamer_version(
            "<table><tr><td>Fedora 44</td><td><a>1.27.1-1.fc44</a></td></tr><tr><td>Fedora 45</td><td><a href=\"/pkg\">1.28.9-1.fc45</a></td></tr></table>",
            "45"
        )
        .as_deref(),
        Some("1.28.9-1.fc45")
    );
    assert_eq!(
        parse_arch_gstreamer_version("<h2>gstreamer 1.28.9-3</h2>").as_deref(),
        Some("1.28.9-3")
    );
    assert_eq!(
        parse_dnf_gstreamer_version("\n1.28.9-1.el10\n").as_deref(),
        Some("1.28.9-1.el10")
    );
    assert!(parse_dnf_gstreamer_version("1.28.9-1\n1.27.9-1").is_none());

    assert_eq!(
        parse_installed_uxplay_version("UxPlay version 1.73.7; AirPlay2").as_deref(),
        Some("1.73.7")
    );
    assert_eq!(
        parse_installed_scrcpy_version("scrcpy 5.0.1 <https://github.com>").as_deref(),
        Some("5.0.1")
    );
    assert_eq!(
        parse_installed_adb_version("Android Debug Bridge version 1.0.41").as_deref(),
        Some("1.0.41")
    );
    assert_eq!(
        parse_installed_gstreamer_version("GStreamer 1.28.9").as_deref(),
        Some("1.28.9")
    );

    let linux_pacman = build_tool_catalog_snapshot(&CatalogContext {
        os: PlatformOs::Linux,
        arch: "aarch64",
        package_manager: "pacman",
        management_enabled: true,
        installed: &InstalledVersions::default(),
        preferences: &ToolVersionPreferences::default(),
        remote: &RemoteCatalogCache::default(),
    });
    assert!(linux_pacman.platform_warning.is_some());
    assert_eq!(
        linux_pacman.tools[1].version_control,
        VersionControlMode::PackageManager
    );

    let macos_catalog = build_tool_catalog_snapshot(&CatalogContext {
        os: PlatformOs::Macos,
        arch: "aarch64",
        package_manager: "homebrew",
        management_enabled: true,
        installed: &InstalledVersions::default(),
        preferences: &ToolVersionPreferences::default(),
        remote: &RemoteCatalogCache::default(),
    });
    assert!(macos_catalog.platform_warning.is_none());
    assert_eq!(
        macos_catalog.tools[1].version_control,
        VersionControlMode::Selectable
    );
}
