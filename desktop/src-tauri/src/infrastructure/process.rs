//! 外部執行檔搜尋、環境變數組裝與具逾時及輸出大小上限保護的跨平台程序執行工具。

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// 單一外部程序管道允許擷取的最大輸出位元組數（1 MiB），防止記憶體耗盡。
pub const MAX_CAPTURED_PROCESS_OUTPUT_BYTES: usize = 1024 * 1024;
/// 同步短程指令（如 `adb devices`）的預設等待逾時時間。
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(15);

/// 在受控管 `engine_dir` 的優先目錄順序中尋找可執行檔，不直接依賴全域 PATH 搜尋。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的工具根目錄。
/// - `name`: 目標執行檔名稱（不含平台副檔名）。
/// - `android`: 若為 `true`，會額外優先搜尋 `android` 與 `android/platform-tools` 子目錄。
///
/// # 回傳值
/// 找到具執行權限的檔案時回傳 `Some(PathBuf)`，否則回傳 `None`。
pub fn find_executable(engine_dir: &Path, name: &str, android: bool) -> Option<PathBuf> {
    let engine_dirs = if android {
        vec![
            engine_dir.join("android"),
            engine_dir.join("android/platform-tools"),
            engine_dir.join("bin"),
            engine_dir.to_owned(),
        ]
    } else {
        vec![engine_dir.join("bin"), engine_dir.to_owned()]
    };
    let names = executable_names(name);
    for directory in engine_dirs {
        for candidate_name in &names {
            let candidate = directory.join(candidate_name);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
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
        paths.extend(std::env::split_paths(inherited));
    }
    paths
}

/// 建構供依賴檢查與安裝腳本使用的擴充 `PATH`，將內建工具目錄置於系統 `PATH` 之前。
///
/// # 參數
/// - `engine_dir`: 應用程式管理的工具根目錄。
///
/// # 回傳值
/// 回傳合併後的 `PATH` `OsString`。
pub fn installer_path(engine_dir: &Path) -> OsString {
    let mut paths = std::env::split_paths(&resolved_path(engine_dir)).collect::<Vec<_>>();
    paths.extend(
        std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .unwrap_or_default(),
    );
    std::env::join_paths(paths).unwrap_or_else(|_| resolved_path(engine_dir))
}

/// 檢查指定路徑是否為存在且具執行權限（於 Unix）的常規檔案。
///
/// # 參數
/// - `path`: 待檢查的檔案路徑。
///
/// # 回傳值
/// 若為可執行的常規檔案則回傳 `true`，否則回傳 `false`。
pub fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
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
    let mut command = background_command(executable);
    command.args(args);
    if let Some(path) = path {
        command.env("PATH", path);
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
