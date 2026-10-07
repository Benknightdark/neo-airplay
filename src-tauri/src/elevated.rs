use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::Foundation::{CloseHandle, WAIT_FAILED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{
    IsUserAnAdmin, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::PCWSTR;

use crate::startup::CliArgs;

const UAC_CANCELLED_HRESULT: i32 = 0x8007_04c7_u32 as i32;

pub fn run(args: &CliArgs) -> i32 {
    if !args.enable_bonjour {
        eprintln!("Elevated setup requires --enable-bonjour.");
        return 2;
    }
    if !unsafe { IsUserAnAdmin().as_bool() } {
        eprintln!("Bonjour setup helper must be started through Windows UAC.");
        return 3;
    }
    match install_bonjour_service() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Bonjour setup failed: {error}");
            1
        }
    }
}

pub fn launch_bonjour_repair() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .ok_or_else(|| "無法判斷受保護的 Program Files 目錄。".to_owned())?;
    if !path_is_under(&executable, &program_files) {
        return Err("Bonjour 修復只允許從 Program Files 中的受保護安裝啟動。".to_owned());
    }

    let executable_wide = wide(executable.as_os_str());
    let directory = executable
        .parent()
        .ok_or_else(|| "找不到 Neo AirPlay 安裝目錄。".to_owned())?;
    let directory_wide = wide(directory.as_os_str());
    let parameters = wide(OsStr::new("--elevated-engine-setup --enable-bonjour"));
    let mut info = SHELLEXECUTEINFOW::default();
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = windows::core::w!("runas");
    info.lpFile = PCWSTR(executable_wide.as_ptr());
    info.lpParameters = PCWSTR(parameters.as_ptr());
    info.lpDirectory = PCWSTR(directory_wide.as_ptr());
    info.nShow = SW_HIDE.0;

    unsafe { ShellExecuteExW(&mut info) }.map_err(|error| {
        if error.code().0 == UAC_CANCELLED_HRESULT {
            "使用者取消了 Windows 系統管理員權限提示。".to_owned()
        } else {
            format!("無法啟動系統管理員修復程序：{error}")
        }
    })?;
    if info.hProcess.is_invalid() {
        return Err("Windows 沒有回傳修復程序控制代碼。".to_owned());
    }

    let wait_result = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
    let mut exit_code = 1_u32;
    let exit_result = unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) };
    let _ = unsafe { CloseHandle(info.hProcess) };
    if wait_result == WAIT_FAILED {
        return Err("等待系統管理員修復程序時發生錯誤。".to_owned());
    }
    if wait_result != WAIT_OBJECT_0 {
        return Err("系統管理員修復程序未正常結束。".to_owned());
    }
    exit_result.map_err(|error| format!("無法讀取修復程序結果：{error}"))?;
    if exit_code != 0 {
        return Err(format!("修復程序以代碼 {exit_code} 結束。"));
    }
    Ok(())
}

fn install_bonjour_service() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let resource_directory = executable
        .parent()
        .ok_or_else(|| "Neo AirPlay 安裝目錄不存在。".to_owned())?;
    let scripts_directory = resource_directory.join("scripts");
    for script in ["Stage-BonjourRuntime.ps1", "Install-BonjourService.ps1"] {
        let script_path = scripts_directory.join(script);
        if !script_path.is_file() {
            return Err(format!(
                "找不到 Bonjour 管理腳本：{}",
                script_path.display()
            ));
        }
        run_powershell(&script_path, resource_directory)?;
    }
    Ok(())
}

fn run_powershell(script: &Path, working_directory: &Path) -> Result<(), String> {
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or_else(|| "找不到 Windows 系統目錄。".to_owned())?;
    let powershell = system_root
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let output = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .creation_flags(0x0800_0000)
        .current_dir(working_directory)
        .output()
        .map_err(|error| format!("無法啟動 Bonjour 管理腳本：{error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if details.is_empty() {
        format!("{} 結束代碼為 {}。", script.display(), output.status)
    } else {
        details
    })
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    path.to_string_lossy().to_lowercase().starts_with(&format!(
        "{}\\",
        root.to_string_lossy()
            .to_lowercase()
            .trim_end_matches(['\\', '/'])
    ))
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
