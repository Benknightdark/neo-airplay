//! 外部執行檔搜尋、環境變數組裝與具逾時及輸出大小上限保護的跨平台程序執行工具。

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 單一外部程序管道允許擷取的最大輸出位元組數（1 MiB），防止記憶體耗盡。
pub const MAX_CAPTURED_PROCESS_OUTPUT_BYTES: usize = 1024 * 1024;
/// 單行外部程序輸出或日誌載入允許保留的最大位元組數（8 KiB），防止無換行超長輸出造成 OOM。
pub const DEFAULT_MAX_LOG_LINE_BYTES: usize = 8192;
/// 同步短程指令（如 `adb devices`）的預設等待逾時時間。
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(15);

/// 啟動外部子程序前必須主動移除的環境變數清单，避免動態函式庫預載、GStreamer 外掛或 ADB/scrcpy/UxPlay 路徑遭到劫持。
const DANGEROUS_CHILD_ENV_VARS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "LD_DEBUG",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_FRAMEWORK_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_FALLBACK_FRAMEWORK_PATH",
    "DYLD_IMAGE_SUFFIX",
    "GST_PLUGIN_PATH",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_PATH_1_0",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
    "GST_PLUGIN_SCANNER",
    "GST_PLUGIN_SCANNER_1_0",
    "GST_REGISTRY",
    "GST_REGISTRY_1_0",
    "GST_REGISTRY_FORK",
    "ADB",
    "ADB_SERVER_SOCKET",
    "ANDROID_ADB_SERVER_ADDRESS",
    "ANDROID_ADB_SERVER_PORT",
    "ANDROID_SERIAL",
    "SCRCPY_SERVER_PATH",
    "SCRCPY_ICON_DIR",
    "UXPLAYRC",
    "OPENSSL_CONF",
    "GCONV_PATH",
    "GIO_EXTRA_MODULES",
];

static TEMP_FILE_NONCE: AtomicU64 = AtomicU64::new(0);

/// 在受控管 `engine_dir` 的優先目錄順序中尋找可執行檔，不直接依賴全域 PATH 搜尋，且拒絕任何符號連結。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的工具根目錄。
/// - `name`: 目標執行檔名稱（不含平台副檔名）。
/// - `android`: 若為 `true`，會額外優先搜尋 `android` 與 `android/platform-tools` 子目錄。
///
/// # 回傳值
/// 找到位於受控管實體目錄內的具執行權限常規檔案時回傳 `Some(PathBuf)`，否則回傳 `None`。
pub fn find_executable(engine_dir: &Path, name: &str, android: bool) -> Option<PathBuf> {
    let subdirs: &[&[&str]] = if android {
        &[&["android"], &["android", "platform-tools"], &["bin"], &[]]
    } else {
        &[&["bin"], &[]]
    };
    let names = executable_names(name);
    for components in subdirs {
        let Some(directory) = safe_engine_subdir(engine_dir, components) else {
            continue;
        };
        for candidate_name in &names {
            let candidate = directory.join(candidate_name);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// 驗證目標路徑是否為非符號連結的實體目錄。
///
/// # 參數
/// - `path`: 待檢查的目錄路徑。
///
/// # 回傳值
/// 路徑存在且為常規目錄（非符號連結）時回傳 `true`。
pub fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

/// 逐層驗證 `engine_dir` 及其子目錄鏈皆為非符號連結的實體目錄。
fn safe_engine_subdir(engine_dir: &Path, components: &[&str]) -> Option<PathBuf> {
    if !is_real_directory(engine_dir) {
        return None;
    }
    let mut current = engine_dir.to_path_buf();
    for component in components {
        current = current.join(component);
        if !is_real_directory(&current) {
            return None;
        }
    }
    Some(current)
}

/// 驗證從 `root` 到指定子目錄鏈皆為非符號連結的實體目錄。
pub fn is_safe_directory_chain(root: &Path, components: &[&str]) -> bool {
    safe_engine_subdir(root, components).is_some()
}

/// 檢查目標路徑是否為非符號連結、非空且為一般檔案的實體檔案。
pub fn is_safe_regular_non_empty_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink() && metadata.is_file() && metadata.len() > 0
    })
}

/// 清除子程序環境中的動態載入器預載、外掛路徑及 ADB／scrcpy／UxPlay 覆寫變數。
pub fn sanitize_child_environment(command: &mut Command) {
    for name in DANGEROUS_CHILD_ENV_VARS {
        command.env_remove(name);
    }
}

/// 建構僅包含內建投影工具目錄的 `PATH` 環境變數字串。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的工具根目錄。
///
/// # 回傳值
/// 回傳組合完成的作業系統原生路徑列表（`OsString`）。
pub fn resolved_path(engine_dir: &Path) -> OsString {
    let paths = [
        engine_dir.join("bin"),
        engine_dir.join("android/platform-tools"),
        engine_dir.join("android"),
        engine_dir.to_owned(),
    ];
    std::env::join_paths(paths).unwrap_or_default()
}

/// 組裝 macOS 執行 UxPlay／GStreamer 所需的 `DYLD_LIBRARY_PATH` 環境變數值。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的工具根目錄。
///
/// # 回傳值
/// 回傳將內建 `lib` 與 Homebrew 函式庫目錄置於既有機制之前的路徑字串。
#[cfg(target_os = "macos")]
pub fn macos_library_path(engine_dir: &Path) -> OsString {
    let paths = macos_library_paths(engine_dir, std::env::var_os("DYLD_LIBRARY_PATH").as_deref());
    std::env::join_paths(paths).unwrap_or_default()
}

/// 計算 macOS 動態函式庫搜尋路徑順序清單，將 `engine_dir/lib`、Homebrew 目錄與繼承路徑串接。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的工具根目錄。
/// - `inherited`: 目前程序環境既有的 `DYLD_LIBRARY_PATH` 原始值（若有）。
///
/// # 回傳值
/// 回傳依優先權排序的函式庫目錄清單。
#[cfg(target_os = "macos")]
pub fn macos_library_paths(engine_dir: &Path, inherited: Option<&OsStr>) -> Vec<PathBuf> {
    let mut paths = vec![engine_dir.join("lib")];
    for prefix in [Path::new("/opt/homebrew"), Path::new("/usr/local")] {
        paths.push(prefix.join("lib"));
    }
    if let Some(inherited) = inherited {
        for entry in std::env::split_paths(inherited) {
            if !entry.is_absolute()
                || entry
                    .components()
                    .any(|component| matches!(component, Component::ParentDir))
            {
                continue;
            }
            let safe_entry = match fs::symlink_metadata(&entry) {
                Ok(metadata) => !metadata.file_type().is_symlink() && metadata.is_dir(),
                Err(error) => error.kind() == ErrorKind::NotFound,
            };
            if safe_entry {
                paths.push(entry);
            }
        }
    }
    paths
}

/// 檢查路徑是否為符號連結；若目標不存在則視為通過。
///
/// # 參數
/// - `path`: 待檢查的檔案或目錄路徑。
///
/// # 回傳值
/// 非符號連結或尚不存在時回傳 `Ok(())`；若為符號連結或檢查失敗回傳 `std::io::Error`。
pub fn reject_symlink_path(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::new(
            ErrorKind::InvalidInput,
            "目標路徑不可為符號連結。",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// 確保目標目錄存在且本身非符號連結；在 Unix 上可選擇強制限制僅擁有者讀寫執行（`0700`）。
///
/// # 參數
/// - `path`: 目標目錄路徑。
/// - `owner_only_unix`: 若為 `true`，於 Unix 系統套用 `0o700` 權限。
///
/// # 回傳值
/// 成功時回傳 `Ok(())`；若為符號連結、非目錄或 I/O 失敗則回傳 `std::io::Error`。
pub fn ensure_safe_directory(path: &Path, owner_only_unix: bool) -> std::io::Result<()> {
    #[cfg(not(unix))]
    let _ = owner_only_unix;

    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "目錄路徑不可為符號連結或非資料夾。",
                ));
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "目錄路徑不可為符號連結或非資料夾。",
                ));
            }
        }
        Err(error) => return Err(error),
    }

    #[cfg(unix)]
    if owner_only_unix {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

/// 使用 `create_new` 唯一暫存檔與原子更名寫入檔案，拒絕追蹤或覆寫符號連結。
///
/// # 參數
/// - `path`: 目標檔案路徑。
/// - `contents`: 欲寫入的原始位元組內容。
/// - `unix_mode`: Unix 平台上套用的檔案權限模式（如 `0o600` 或 `0o700`）。
///
/// # 回傳值
/// 成功寫入與置換回傳 `Ok(())`；遇到符號連結或 I/O 失敗回傳 `std::io::Error`。
pub fn write_file_atomic_secure(
    path: &Path,
    contents: &[u8],
    unix_mode: u32,
) -> std::io::Result<()> {
    stage_temp_file_atomic(path, unix_mode, None, |file| file.write_all(contents))
}

/// 安全驗證並開啟非符號連結的常規檔案；在 Unix 平台上於開啟後核對 `(dev, ino)` 以消除 TOCTOU 競態。
pub fn open_regular_file_secure(path: &Path) -> std::io::Result<File> {
    let pre_meta = fs::symlink_metadata(path)?;
    if pre_meta.file_type().is_symlink() || !pre_meta.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "目標檔案不可為符號連結或非一般檔案。",
        ));
    }
    let file = File::open(path)?;
    let open_meta = file.metadata()?;
    if !open_meta.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "目標檔案不可為非一般檔案。",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if open_meta.dev() != pre_meta.dev() || open_meta.ino() != pre_meta.ino() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "目標檔案在開啟期間遭到置換，已拒絕讀取。",
            ));
        }
    }
    Ok(file)
}

/// 以附加模式安全開啟非符號連結的常規檔案（Unix 套用指定檔案權限），並於開啟後驗證未遭符號連結置換。
pub fn open_append_regular_file_secure(path: &Path, unix_mode: u32) -> std::io::Result<File> {
    #[cfg(not(unix))]
    let _ = unix_mode;

    reject_symlink_path(path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(unix_mode);
    }
    let file = options.open(path)?;
    let link_meta = fs::symlink_metadata(path)?;
    if link_meta.file_type().is_symlink() || !link_meta.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "目標檔案不可為符號連結或非一般檔案。",
        ));
    }
    let open_meta = file.metadata()?;
    if !open_meta.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "目標檔案不可為非一般檔案。",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if open_meta.dev() != link_meta.dev() || open_meta.ino() != link_meta.ino() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "目標檔案路徑與已開啟檔案節點不一致，已拒絕寫入。",
            ));
        }
    }
    Ok(file)
}

/// 以有界限的方式讀取非符號連結常規檔案內容，避免記憶體耗盡與符號連結越界讀取。
///
/// # 參數
/// - `path`: 欲讀取的檔案路徑。
/// - `max_bytes`: 允許讀取的最大位元組數。
///
/// # 回傳值
/// 回傳完整位元組向量；若為符號連結、非一般檔案或超過上限則回傳 `std::io::Error`。
pub fn read_bounded_regular_file(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let file = open_regular_file_secure(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("檔案大小超過 {max_bytes} 位元組上限。"),
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let limit = max_bytes.saturating_add(1);
    file.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("檔案大小超過 {max_bytes} 位元組上限。"),
        ));
    }
    Ok(bytes)
}

/// 在正式版本首次啟動時安全地將隨附引擎目錄複製至使用者資料目錄並寫入版本標記，
/// 略過任何來源符號連結且拒絕目標路徑中的符號連結。
///
/// # 參數
/// - `bundled_engine`: 應用程式套件隨附的來源 `engine` 目錄。
/// - `engine_dir`: 目標本機投影工具根目錄。
/// - `version`: 用於 `.bundled-<version>` 標記檔的版本字串。
///
/// # 回傳值
/// 完成複製或無需複製回傳 `Ok(())`；遇到不安全符號連結或 I/O 失敗回傳 `std::io::Error`。
pub fn seed_bundled_engine_in_dir(
    bundled_engine: &Path,
    engine_dir: &Path,
    version: &str,
) -> std::io::Result<()> {
    match fs::symlink_metadata(bundled_engine) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "隨附引擎來源目錄路徑不安全。",
                ));
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }

    ensure_safe_directory(engine_dir, false)?;
    let marker = engine_dir.join(format!(".bundled-{version}"));
    match fs::symlink_metadata(&marker) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "隨附引擎版本標記檔路徑不安全。",
                ));
            }
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    copy_directory_safely(bundled_engine, engine_dir)?;
    write_file_atomic_secure(&marker, b"seeded\n", 0o600)
}

fn copy_directory_safely(source: &Path, destination: &Path) -> std::io::Result<()> {
    ensure_safe_directory(destination, false)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            copy_directory_safely(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            let permissions = metadata.permissions();
            stage_temp_file_atomic(&destination_path, 0o600, Some(permissions), |out_file| {
                let mut in_file = File::open(&source_path)?;
                std::io::copy(&mut in_file, out_file)?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn open_exclusive_temp_file(
    parent: &Path,
    prefix: &str,
    unix_mode: u32,
) -> std::io::Result<(PathBuf, File)> {
    #[cfg(not(unix))]
    let _ = unix_mode;

    let safe_prefix: String = prefix
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    for _ in 0..16 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let nonce = TEMP_FILE_NONCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{safe_prefix}.{}.{nanos}.{nonce}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(unix_mode);
        }
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::new(
        ErrorKind::AlreadyExists,
        "無法建立唯一暫存檔案。",
    ))
}

fn stage_temp_file_atomic(
    path: &Path,
    unix_mode: u32,
    permissions: Option<fs::Permissions>,
    write_body: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    reject_symlink_path(path)?;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        Some(_) => Path::new("."),
        None => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "無法解析目標檔案上層目錄。",
            ))
        }
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tmp");
    let (temp_path, mut file) = open_exclusive_temp_file(parent, file_name, unix_mode)?;
    let result = (|| {
        write_body(&mut file)?;
        file.flush()?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(unix_mode))?;
            }
        }
        drop(file);
        reject_symlink_path(path)?;
        fs::rename(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

/// 檢查指定路徑是否為存在且非符號連結、具執行權限（於 Unix）的常規檔案。
///
/// # 參數
/// - `path`: 待檢查的檔案路徑。
///
/// # 回傳值
/// 若為可執行的常規檔案（非符號連結）則回傳 `true`，否則回傳 `false`。
pub fn is_executable_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        let is_regular_file = !metadata.file_type().is_symlink() && metadata.is_file();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            is_regular_file && metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            is_regular_file
        }
    })
}

/// 依作業系統慣例展開候選執行檔名稱（Windows 優先嘗試 `.exe` 副檔名）。
///
/// # 參數
/// - `name`: 基礎執行檔名稱。
///
/// # 回傳值
/// 回傳依搜尋優先權排列的檔名清單。
fn executable_names(name: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![format!("{name}.exe"), name.to_owned()]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![name.to_owned()]
    }
}

/// 使用預設 ADB 逾時語意執行外部指令並擷取輸出，逾時時將錯誤訊息轉為 ADB 專用提示。
///
/// # 參數
/// - `executable`: 執行檔完整路徑。
/// - `args`: 傳遞給執行檔的參數陣列。
/// - `timeout`: 最長允許執行時間。
///
/// # 回傳值
/// 成功收集完成輸出回傳 `Ok(Output)`；啟動失敗、逾時或輸出超限回傳 `Err(String)`。
pub fn run_with_timeout(
    executable: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<Output, String> {
    run_with_timeout_in_path(executable, args, timeout, None).map_err(|error| {
        if error == "程序執行逾時。" {
            "ADB 裝置查詢逾時。".to_owned()
        } else {
            error
        }
    })
}

/// 在可選自訂 `PATH` 下啟動外部指令，以非阻塞輪詢監控逾時並同時排空 `stdout` 與 `stderr`。
///
/// # 參數
/// - `executable`: 執行檔完整路徑。
/// - `args`: 命令列參數陣列。
/// - `timeout`: 最長等待完成時間，超時後會強制終止子程序。
/// - `path`: 選擇性覆蓋的 `PATH` 環境變數值。
///
/// # 回傳值
/// 回傳含結束狀態與輸出位元組的 `Output`，或描述錯誤／逾時的字串。
pub fn run_with_timeout_in_path(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    path: Option<&OsStr>,
) -> Result<Output, String> {
    run_with_timeout_env(executable, args, timeout, path, &[])
}

/// 在可選自訂 `PATH` 與額外環境變數下啟動外部指令，以非阻塞輪詢監控逾時並同時排空輸出管道。
pub fn run_with_timeout_env(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    path: Option<&OsStr>,
    extra_env: &[(&str, &OsStr)],
) -> Result<Output, String> {
    run_with_timeout_env_inner(executable, args, timeout, path, extra_env, false)
}

/// 在完整淨化子程序環境變數後，以可選自訂 `PATH` 與額外環境變數啟動外部指令並限制執行逾時。
pub fn run_with_timeout_env_sanitized(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    path: Option<&OsStr>,
    extra_env: &[(&str, &OsStr)],
) -> Result<Output, String> {
    run_with_timeout_env_inner(executable, args, timeout, path, extra_env, true)
}

fn run_with_timeout_env_inner(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    path: Option<&OsStr>,
    extra_env: &[(&str, &OsStr)],
    sanitize_all: bool,
) -> Result<Output, String> {
    let mut command = background_command(executable);
    if sanitize_all {
        sanitize_child_environment(&mut command);
    } else {
        for var in [
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_DEBUG",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_IMAGE_SUFFIX",
        ] {
            command.env_remove(var);
        }
    }
    command.args(args);
    if let Some(path) = path {
        command.env("PATH", path);
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout_reader = child.stdout.take().map(read_pipe);
    let stderr_reader = child.stderr.take().map(read_pipe);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = finish_pipe(stdout_reader);
                let stderr = finish_pipe(stderr_reader);
                return Ok(Output {
                    status,
                    stdout: stdout?,
                    stderr: stderr?,
                });
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = finish_pipe(stdout_reader);
                let _ = finish_pipe(stderr_reader);
                return Err(format!("無法檢查 ADB 程序狀態：{error}"));
            }
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = finish_pipe(stdout_reader);
            let _ = finish_pipe(stderr_reader);
            return Err("程序執行逾時。".to_owned());
        }
        thread::sleep(Duration::from_millis(40));
    }
}

/// 建立不會在 Windows 上閃出主控台黑框的背景 `Command`。
///
/// # 參數
/// - `executable`: 執行檔路徑或命令名稱。
///
/// # 回傳值
/// 回傳已完成平台隱藏視窗旗標設定的 `Command`。
pub fn background_command(executable: impl AsRef<OsStr>) -> Command {
    let command = Command::new(executable);
    #[cfg(target_os = "windows")]
    let command = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}

/// 背景管道讀取器回傳的原始輸出緩衝與是否觸及位元組上限旗標。
pub struct CapturedPipe {
    /// 已擷取的輸出位元組內容（最多 `MAX_CAPTURED_PROCESS_OUTPUT_BYTES`）。
    pub bytes: Vec<u8>,
    /// 若子程序輸出超過上限而被截斷，則為 `true`。
    pub exceeded_limit: bool,
}

/// 在獨立執行緒持續排空子程序 `stdout` 或 `stderr` 管道，防止管線緩衝區滿載導致死鎖。
///
/// # 參數
/// - `reader`: 具備 `Read + Send` 能力的子程序輸出管道端點。
///
/// # 回傳值
/// 回傳背景執行緒的 `JoinHandle`，完成後產出 `CapturedPipe` 或 I/O 錯誤。
pub fn read_pipe<R: Read + Send + 'static>(
    mut reader: R,
) -> thread::JoinHandle<std::io::Result<CapturedPipe>> {
    thread::spawn(move || {
        let mut output = Vec::with_capacity(MAX_CAPTURED_PROCESS_OUTPUT_BYTES.min(8192));
        let mut exceeded_limit = false;
        let mut buffer = [0_u8; 8192];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            let remaining = MAX_CAPTURED_PROCESS_OUTPUT_BYTES.saturating_sub(output.len());
            let captured = count.min(remaining);
            output.extend_from_slice(&buffer[..captured]);
            exceeded_limit |= captured < count;
        }
        Ok(CapturedPipe {
            bytes: output,
            exceeded_limit,
        })
    })
}

/// 等待管道讀取執行緒結束並驗證輸出未超過記憶體安全上限。
///
/// # 參數
/// - `reader`: 由 `read_pipe` 產生的執行緒控制權（若無管道則為 `None`）。
///
/// # 回傳值
/// 回傳完整擷取到的位元組向量；若讀取失敗或超過 1 MiB 上限則回傳 `Err(String)`。
pub fn finish_pipe(
    reader: Option<thread::JoinHandle<std::io::Result<CapturedPipe>>>,
) -> Result<Vec<u8>, String> {
    reader
        .map(|reader| {
            let output = reader
                .join()
                .map_err(|_| "無法讀取 ADB 程序輸出。".to_owned())?
                .map_err(|error| format!("無法讀取 ADB 程序輸出：{error}"))?;
            if output.exceeded_limit {
                return Err("外部程序輸出超過 1 MiB 上限。".to_owned());
            }
            Ok(output.bytes)
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// 從子程序輸出位元組中以 lossy UTF-8 擷取第一行非空白文字。
///
/// # 參數
/// - `bytes`: 外部程序輸出的原始位元組切片。
///
/// # 回傳值
/// 找到非空白行時回傳 `Some(String)`，全為空白或空輸出時回傳 `None`。
pub fn first_non_empty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

/// 以有界緩衝區逐行讀取串流（單行最多保留 `max_line_bytes` 位元組），超長行截斷後仍持續排空至換行或 EOF 以防 OOM 或管線阻塞。
pub fn read_bounded_lines<R: Read>(
    reader: R,
    max_line_bytes: usize,
    mut on_line: impl FnMut(String),
) {
    let mut buffered = BufReader::with_capacity(8192, reader);
    let mut line_bytes = Vec::with_capacity(max_line_bytes.min(1024));
    let mut truncated = false;

    loop {
        let chunk = match buffered.fill_buf() {
            Ok([]) | Err(_) => {
                if !line_bytes.is_empty() || truncated {
                    emit_bounded_line(&mut line_bytes, &mut truncated, &mut on_line);
                }
                break;
            }
            Ok(chunk) => chunk,
        };

        if let Some(newline_index) = chunk.iter().position(|byte| *byte == b'\n') {
            append_bounded_slice(
                &mut line_bytes,
                &chunk[..newline_index],
                max_line_bytes,
                &mut truncated,
            );
            buffered.consume(newline_index + 1);
            emit_bounded_line(&mut line_bytes, &mut truncated, &mut on_line);
        } else {
            let chunk_len = chunk.len();
            append_bounded_slice(&mut line_bytes, chunk, max_line_bytes, &mut truncated);
            buffered.consume(chunk_len);
        }
    }
}

fn append_bounded_slice(
    line_bytes: &mut Vec<u8>,
    slice: &[u8],
    max_line_bytes: usize,
    truncated: &mut bool,
) {
    if line_bytes.len() < max_line_bytes {
        let remaining = max_line_bytes - line_bytes.len();
        let take = slice.len().min(remaining);
        line_bytes.extend_from_slice(&slice[..take]);
        if take < slice.len() {
            *truncated = true;
        }
    } else if !slice.is_empty() {
        *truncated = true;
    }
}

fn emit_bounded_line(
    line_bytes: &mut Vec<u8>,
    truncated: &mut bool,
    on_line: &mut impl FnMut(String),
) {
    if line_bytes.last().copied() == Some(b'\r') {
        line_bytes.pop();
    }
    let mut line = String::from_utf8_lossy(line_bytes).into_owned();
    if *truncated {
        line.push_str("…[truncated]");
    }
    line_bytes.clear();
    *truncated = false;
    on_line(line);
}
