use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

use serde::Deserialize;
use windows_service::service::{ServiceAccess, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use winreg::RegKey;
use winreg::enums::HKEY_LOCAL_MACHINE;

use super::{MdnsProblem, MdnsRuntime, StartupCheck, read_pe_machine_type};

const SETTINGS: &str = include_str!("../../../../scripts/BonjourServiceSettings.json");
const SERVICE_NAME: &str = "Bonjour Service";
const HELP_URL: &str = "https://support.apple.com/zh-tw/106380";
const PE_I386: u16 = 0x014c;
const PE_AMD64: u16 = 0x8664;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BonjourSettings {
    program_files_relative_directory: String,
    program_files_relative_source_executable_paths: Vec<String>,
    program_files_x86_relative_source_executable_paths: Vec<String>,
    missing_compatible_x64_dns_sd_message: String,
}

#[derive(Debug, PartialEq, Eq)]
struct FileMetadata {
    product: String,
    version: String,
}

pub(super) fn runtime() -> Result<MdnsRuntime, MdnsProblem> {
    let settings = settings();
    let program_files = env_path("ProgramFiles");
    let directory = program_files.join(safe_relative(&settings.program_files_relative_directory)?);
    let responder = directory.join("mDNSResponder.exe");
    let dns_sd = directory.join("dnssd.dll");
    if !compatible_pair(&responder, &dns_sd) {
        return Err(problem(
            "系統尚未備妥 Neo AirPlay 的 Bonjour 元件。請重新啟動 App，並在提示時允許啟用 Bonjour Service。",
        ));
    }

    let registry = service_registry_key()
        .map_err(|_| problem("找不到 Windows 的 Bonjour Service。請先安裝 Bonjour，或讓 Neo AirPlay 使用可信任的本機 Bonjour 檔案建立服務。"))?;
    let image_path: String = registry
        .get_value("ImagePath")
        .map_err(|_| problem("Bonjour Service 尚未設定執行檔路徑。"))?;
    if !path_equals(&extract_executable_path(&image_path), &responder) {
        return Err(problem(format!(
            "Bonjour Service 尚未指向 {}。請重新啟動 App，並在提示時允許啟用 Bonjour Service。",
            responder.display()
        )));
    }

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|error| {
            problem(format!(
                "無法連線至 Windows Service Control Manager：{error}"
            ))
        })?;
    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .map_err(|error| problem(format!("無法查詢 Bonjour Service：{error}")))?;
    let status = service
        .query_status()
        .map_err(|error| problem(format!("無法查詢 Bonjour Service 狀態：{error}")))?;
    if status.current_state != ServiceState::Running || status.process_id.unwrap_or_default() == 0 {
        let disabled = registry.get_value::<u32, _>("Start").ok() == Some(4);
        return Err(problem(if disabled {
            "Bonjour Service 在 Windows 中為停用狀態；Neo AirPlay 不會自動變更這項設定。"
        } else {
            "Bonjour Service 尚未執行。"
        }));
    }

    Ok(MdnsRuntime {
        directory: Some(directory),
    })
}

pub(super) fn startup_check() -> StartupCheck {
    match runtime() {
        Ok(_) => StartupCheck::Ready,
        Err(runtime_problem) => match find_installed_pair() {
            Ok(Some(_)) => StartupCheck::NeedsElevatedRepair {
                reason: runtime_problem.message,
            },
            Ok(None) => StartupCheck::Missing {
                problem: MdnsProblem {
                    message: "找不到可信任且有效的 Bonjour Service 執行檔。請依 Apple 官方說明安裝 Bonjour，再重新啟動 Neo AirPlay。".to_owned(),
                    help_url: Some(HELP_URL.to_owned()),
                },
            },
            Err(problem) => StartupCheck::Missing { problem },
        },
    }
}

fn find_installed_pair() -> Result<Option<(PathBuf, PathBuf)>, MdnsProblem> {
    let settings = settings();
    let roots = [env_path("ProgramFiles"), env_path("ProgramFiles(x86)")];
    let mut candidates = Vec::new();
    for (root, relative_paths) in roots.iter().zip([
        &settings.program_files_relative_source_executable_paths,
        &settings.program_files_x86_relative_source_executable_paths,
    ]) {
        for relative in relative_paths {
            candidates.push(root.join(safe_relative(relative)?));
        }
    }
    let runtime_directory =
        env_path("ProgramFiles").join(safe_relative(&settings.program_files_relative_directory)?);
    candidates.push(runtime_directory.join("mDNSResponder.exe"));
    if let Ok(registry) = service_registry_key() {
        if let Ok(image_path) = registry.get_value::<String, _>("ImagePath") {
            candidates.push(extract_executable_path(&image_path));
        }
    }

    let mut found_responder = false;
    for executable in candidates {
        if !trusted_executable_root(&executable) || !valid_responder(&executable) {
            continue;
        }
        found_responder = true;
        let mut dns_candidates = vec![
            executable.with_file_name("dnssd.dll"),
            runtime_directory.join("dnssd.dll"),
        ];
        dns_candidates.push(env_path("SystemRoot").join("System32").join("dnssd.dll"));
        if let Some(dns_sd) = dns_candidates
            .into_iter()
            .find(|path| trusted_dns_root(path) && compatible_pair(&executable, path))
        {
            return Ok(Some((executable, dns_sd)));
        }
    }
    if found_responder {
        Err(problem(settings.missing_compatible_x64_dns_sd_message))
    } else {
        Ok(None)
    }
}

fn compatible_pair(responder: &Path, dns_sd: &Path) -> bool {
    if !trusted_executable_root(responder) || !trusted_dns_root(dns_sd) {
        return false;
    }
    if !valid_responder(responder) {
        return false;
    }
    let Some(dns_metadata) = file_metadata(dns_sd) else {
        return false;
    };
    let Some(responder_machine) = pe_machine(responder) else {
        return false;
    };
    let Some(dns_machine) = pe_machine(dns_sd) else {
        return false;
    };
    let Some(responder_metadata) = file_metadata(responder) else {
        return false;
    };
    responder_metadata.version == dns_metadata.version
        && dns_metadata.product == "Bonjour"
        && matches!(responder_machine, PE_I386 | PE_AMD64)
        && dns_machine == PE_AMD64
}

fn valid_responder(responder: &Path) -> bool {
    responder
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("mDNSResponder.exe"))
        && file_metadata(responder).is_some_and(|metadata| metadata.product == "Bonjour")
        && pe_machine(responder).is_some_and(|machine| matches!(machine, PE_I386 | PE_AMD64))
}

fn pe_machine(path: &Path) -> Option<u16> {
    let mut file = File::open(path).ok()?;
    read_pe_machine_type(&mut file)
}

fn file_metadata(path: &Path) -> Option<FileMetadata> {
    let wide_path = wide(path.as_os_str());
    let mut ignored = 0;
    let size = unsafe { GetFileVersionInfoSizeW(wide_path.as_ptr(), &mut ignored) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0_u8; size as usize];
    if unsafe {
        GetFileVersionInfoW(
            wide_path.as_ptr(),
            0,
            size,
            data.as_mut_ptr().cast::<c_void>(),
        )
    } == 0
    {
        return None;
    }

    let mut translation_pointer = ptr::null_mut();
    let mut translation_size = 0;
    let translation_query = wide(OsStr::new("\\VarFileInfo\\Translation"));
    if unsafe {
        VerQueryValueW(
            data.as_ptr().cast::<c_void>(),
            translation_query.as_ptr(),
            &mut translation_pointer,
            &mut translation_size,
        )
    } == 0
        || translation_pointer.is_null()
        || translation_size < 4
    {
        return None;
    }
    let translation = unsafe { std::slice::from_raw_parts(translation_pointer.cast::<u16>(), 2) };
    let product = query_string(&data, translation[0], translation[1], "ProductName")?;
    let version = query_string(&data, translation[0], translation[1], "FileVersion")?;
    Some(FileMetadata { product, version })
}

fn query_string(data: &[u8], language: u16, codepage: u16, key: &str) -> Option<String> {
    let query = wide(OsStr::new(&format!(
        "\\StringFileInfo\\{language:04x}{codepage:04x}\\{key}"
    )));
    let mut value = ptr::null_mut();
    let mut length = 0;
    if unsafe {
        VerQueryValueW(
            data.as_ptr().cast::<c_void>(),
            query.as_ptr(),
            &mut value,
            &mut length,
        )
    } == 0
        || value.is_null()
        || length == 0
    {
        return None;
    }
    let words = unsafe { std::slice::from_raw_parts(value.cast::<u16>(), length as usize) };
    let end = words
        .iter()
        .position(|word| *word == 0)
        .unwrap_or(words.len());
    String::from_utf16(&words[..end]).ok()
}

fn service_registry_key() -> io::Result<winreg::RegKey> {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SYSTEM\\CurrentControlSet\\Services\\Bonjour Service")
}

fn env_path(name: &str) -> PathBuf {
    std::env::var_os(name).map_or_else(PathBuf::new, PathBuf::from)
}

fn settings() -> BonjourSettings {
    serde_json::from_str(SETTINGS).expect("embedded Bonjour settings are valid")
}

fn safe_relative(value: &str) -> Result<PathBuf, MdnsProblem> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        || value.split(['\\', '/']).any(|segment| {
            segment.is_empty()
                || segment.chars().any(|character| {
                    !(character.is_ascii_alphanumeric() || ". _-".contains(character))
                })
        })
    {
        return Err(problem(
            "BonjourServiceSettings.json 含有不安全的相對路徑。",
        ));
    }
    Ok(value.split(['\\', '/']).collect())
}

fn trusted_executable_root(path: &Path) -> bool {
    under_root(path, &env_path("ProgramFiles")) || under_root(path, &env_path("ProgramFiles(x86)"))
}

fn trusted_dns_root(path: &Path) -> bool {
    trusted_executable_root(path) || under_root(path, &env_path("SystemRoot").join("System32"))
}

fn under_root(path: &Path, root: &Path) -> bool {
    let (Ok(path), Ok(root)) = (path.canonicalize(), root.canonicalize()) else {
        return false;
    };
    path.starts_with(root)
}

fn path_equals(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn extract_executable_path(image_path: &str) -> PathBuf {
    let image_path = image_path.trim();
    if let Some(quoted) = image_path.strip_prefix('"') {
        return quoted.split_once('"').map_or_else(
            || PathBuf::from(image_path),
            |(path, _)| PathBuf::from(path),
        );
    }
    image_path
        .get(
            ..image_path
                .to_ascii_lowercase()
                .find(".exe")
                .map_or(image_path.len(), |index| index + 4),
        )
        .map_or_else(|| PathBuf::from(image_path), PathBuf::from)
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

fn problem(message: impl Into<String>) -> MdnsProblem {
    MdnsProblem {
        message: message.into(),
        help_url: None,
    }
}

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(file_name: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(
        file_name: *const u16,
        handle: u32,
        length: u32,
        data: *mut c_void,
    ) -> i32;
    fn VerQueryValueW(
        block: *const c_void,
        sub_block: *const u16,
        buffer: *mut *mut c_void,
        length: *mut u32,
    ) -> i32;
}
