use std::path::Path;
use std::process::{Command, Stdio};

use super::{MdnsProblem, MdnsRuntime};

const AVAHI_PID_FILES: [&str; 2] = ["/run/avahi-daemon/pid", "/var/run/avahi-daemon/pid"];

pub(super) fn runtime() -> Result<MdnsRuntime, MdnsProblem> {
    if systemd_service_is_active()
        || dbus_service_is_available()
        || AVAHI_PID_FILES
            .iter()
            .any(|path| daemon_is_running(Path::new(path)))
    {
        return Ok(MdnsRuntime { directory: None });
    }

    Err(MdnsProblem {
        message: "Avahi 尚未執行。請執行 sudo systemctl enable --now avahi-daemon，並確認防火牆允許 UDP 5353。".to_owned(),
        help_url: None,
    })
}

fn systemd_service_is_active() -> bool {
    command_succeeded(
        "/usr/bin/systemctl",
        &["is-active", "--quiet", "avahi-daemon"],
    )
}

fn dbus_service_is_available() -> bool {
    command_succeeded(
        "/usr/bin/busctl",
        &[
            "--system",
            "--timeout=3s",
            "status",
            "org.freedesktop.Avahi",
        ],
    )
}

fn command_succeeded(program: &str, arguments: &[&str]) -> bool {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn daemon_is_running(pid_file: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(pid_file) else {
        return false;
    };
    let Ok(pid) = contents.trim().parse::<i32>() else {
        return false;
    };
    if pid <= 0 {
        return false;
    }

    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::daemon_is_running;

    #[test]
    fn rejects_invalid_and_missing_pid_files() {
        assert!(!daemon_is_running(Path::new("/path/that/does/not/exist")));
    }
}
