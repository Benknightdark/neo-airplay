#[cfg(any(windows, test))]
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux_avahi;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows_bonjour;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdnsRuntime {
    pub directory: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdnsProblem {
    pub message: String,
    pub help_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupCheck {
    Ready,
    NeedsElevatedRepair { reason: String },
    Missing { problem: MdnsProblem },
}

pub trait MdnsInspector: Send + Sync {
    fn runtime(&self) -> Result<MdnsRuntime, MdnsProblem>;
    fn startup_check(&self) -> StartupCheck;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemMdnsInspector;

#[cfg(any(windows, test))]
fn read_pe_machine_type(stream: &mut (impl Read + Seek)) -> Option<u16> {
    let original_position = stream.stream_position().ok()?;
    let result = (|| {
        let length = stream.seek(SeekFrom::End(0)).ok()?;
        if length < 0x40 {
            return None;
        }
        stream.seek(SeekFrom::Start(0x3c)).ok()?;
        let mut offset_bytes = [0_u8; 4];
        stream.read_exact(&mut offset_bytes).ok()?;
        let pe_offset = u64::from(u32::from_le_bytes(offset_bytes));
        if pe_offset.checked_add(6)? > length {
            return None;
        }
        stream.seek(SeekFrom::Start(pe_offset)).ok()?;
        let mut signature_and_machine = [0_u8; 6];
        stream.read_exact(&mut signature_and_machine).ok()?;
        (u32::from_le_bytes(signature_and_machine[..4].try_into().ok()?) == 0x0000_4550)
            .then(|| u16::from_le_bytes(signature_and_machine[4..].try_into().expect("two bytes")))
    })();
    let _ = stream.seek(SeekFrom::Start(original_position));
    result
}

#[cfg(target_os = "macos")]
impl MdnsInspector for SystemMdnsInspector {
    fn runtime(&self) -> Result<MdnsRuntime, MdnsProblem> {
        Ok(MdnsRuntime { directory: None })
    }

    fn startup_check(&self) -> StartupCheck {
        StartupCheck::Ready
    }
}

#[cfg(target_os = "linux")]
impl MdnsInspector for SystemMdnsInspector {
    fn runtime(&self) -> Result<MdnsRuntime, MdnsProblem> {
        linux_avahi::runtime()
    }

    fn startup_check(&self) -> StartupCheck {
        match self.runtime() {
            Ok(_) => StartupCheck::Ready,
            Err(problem) => StartupCheck::Missing { problem },
        }
    }
}

#[cfg(target_os = "windows")]
impl MdnsInspector for SystemMdnsInspector {
    fn runtime(&self) -> Result<MdnsRuntime, MdnsProblem> {
        windows_bonjour::runtime()
    }

    fn startup_check(&self) -> StartupCheck {
        windows_bonjour::startup_check()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::read_pe_machine_type;

    #[test]
    fn parses_pe_machine_without_moving_the_input_cursor() {
        let mut data = vec![0_u8; 0x90];
        data[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        data[0x80..0x84].copy_from_slice(&0x0000_4550_u32.to_le_bytes());
        data[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
        let mut cursor = Cursor::new(data);
        cursor.set_position(7);

        assert_eq!(read_pe_machine_type(&mut cursor), Some(0x8664));
        assert_eq!(cursor.position(), 7);
    }
}
