use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct CliArgs {
    pub background: bool,
    pub start: bool,
    pub e2e_profile: Option<PathBuf>,
    pub elevated_engine_setup: bool,
    pub enable_bonjour: bool,
}

#[cfg(windows)]
pub fn remove_legacy_windows_autostart() -> Result<bool, String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const LEGACY_VALUE: &str = "NeoAirPlay";

    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = match current_user.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("無法檢查舊版自動啟動項目：{error}")),
    };

    match run_key.delete_value(LEGACY_VALUE) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("無法移除舊版自動啟動項目：{error}")),
    }
}

impl CliArgs {
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Self {
        let mut parsed = Self::default();
        let mut arguments = arguments.into_iter().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.to_string_lossy().as_ref() {
                "--background" => parsed.background = true,
                "--start" => parsed.start = true,
                "--elevated-engine-setup" => parsed.elevated_engine_setup = true,
                "--enable-bonjour" => parsed.enable_bonjour = true,
                "--e2e-profile" => parsed.e2e_profile = arguments.next().map(PathBuf::from),
                _ => {}
            }
        }
        parsed
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::CliArgs;

    #[test]
    fn parses_background_start_and_e2e_profile_flags() {
        let args = CliArgs::parse(
            [
                "neo-airplay",
                "--background",
                "--start",
                "--e2e-profile",
                "/tmp/profile",
            ]
            .map(OsString::from),
        );

        assert!(args.background);
        assert!(args.start);
        assert_eq!(
            args.e2e_profile.as_deref(),
            Some(std::path::Path::new("/tmp/profile"))
        );
    }
}
