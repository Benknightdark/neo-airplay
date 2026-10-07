use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use async_trait::async_trait;
use process_wrap::tokio::{ChildWrapper as WrappedChild, CommandWrap};
use tokio::process::{ChildStderr, ChildStdout};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait ChildHandle: Send {
    fn id(&self) -> Option<u32>;
    fn take_stdout(&mut self) -> Option<ChildStdout>;
    fn take_stderr(&mut self) -> Option<ChildStderr>;
    async fn wait(&mut self) -> io::Result<ExitStatus>;
    async fn request_stop(&mut self) -> io::Result<()>;
    async fn kill_tree(&mut self) -> io::Result<()>;
}

#[async_trait]
pub trait ProcessSpawner: Send + Sync {
    async fn spawn(&self, spec: ProcessSpec) -> io::Result<Box<dyn ChildHandle>>;
    async fn run_to_end(&self, spec: ProcessSpec, timeout: Duration) -> io::Result<CommandOutput>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TokioProcessSpawner;

#[async_trait]
impl ProcessSpawner for TokioProcessSpawner {
    async fn spawn(&self, spec: ProcessSpec) -> io::Result<Box<dyn ChildHandle>> {
        let mut command = build_wrapped_command(spec);
        let child = command.spawn()?;
        Ok(Box::new(ProcessWrapChildHandle { child }))
    }

    async fn run_to_end(&self, spec: ProcessSpec, timeout: Duration) -> io::Result<CommandOutput> {
        let mut command = build_wrapped_command(spec);
        let child = command.spawn()?;
        let output_future = Box::into_pin(child.wait_with_output());
        let output = tokio::time::timeout(timeout, output_future)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "child process timed out"))??;

        Ok(CommandOutput {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

struct ProcessWrapChildHandle {
    child: Box<dyn WrappedChild>,
}

#[async_trait]
impl ChildHandle for ProcessWrapChildHandle {
    fn id(&self) -> Option<u32> {
        self.child.id()
    }

    fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout().take()
    }

    fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr().take()
    }

    async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }

    async fn request_stop(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.child.signal(libc::SIGTERM)
        }
        #[cfg(windows)]
        {
            request_windows_close(self.id())
        }
    }

    async fn kill_tree(&mut self) -> io::Result<()> {
        self.child.start_kill()
    }
}

fn build_wrapped_command(spec: ProcessSpec) -> CommandWrap {
    let mut command = CommandWrap::with_new(spec.program, |command| {
        command
            .args(spec.args)
            .current_dir(spec.cwd)
            .envs(spec.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    });

    #[cfg(unix)]
    {
        use process_wrap::tokio::{KillOnDrop, ProcessGroup};
        command.wrap(ProcessGroup::leader()).wrap(KillOnDrop);
    }
    #[cfg(windows)]
    {
        use process_wrap::tokio::{CreationFlags, JobObject, KillOnDrop};
        use windows::Win32::System::Threading::CREATE_NO_WINDOW;
        command
            .wrap(KillOnDrop)
            .wrap(JobObject)
            .wrap(CreationFlags(CREATE_NO_WINDOW));
    }

    command
}

#[cfg(windows)]
fn request_windows_close(pid: Option<u32>) -> io::Result<()> {
    type WindowHandle = isize;
    type LongParameter = isize;
    type Boolean = i32;
    const WM_CLOSE: u32 = 0x0010;

    #[repr(C)]
    struct WindowSearch {
        pid: u32,
        handles: Vec<WindowHandle>,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(
            callback: Option<unsafe extern "system" fn(WindowHandle, LongParameter) -> Boolean>,
            parameter: LongParameter,
        ) -> Boolean;
        fn GetWindowThreadProcessId(window: WindowHandle, process_id: *mut u32) -> u32;
        fn PostMessageW(
            window: WindowHandle,
            message: u32,
            wparam: usize,
            lparam: LongParameter,
        ) -> Boolean;
    }

    unsafe extern "system" fn collect_window(
        window: WindowHandle,
        parameter: LongParameter,
    ) -> Boolean {
        let search = unsafe { &mut *(parameter as *mut WindowSearch) };
        let mut window_pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut window_pid) };
        if window_pid == search.pid {
            search.handles.push(window);
        }
        1
    }

    let Some(pid) = pid else {
        return Ok(());
    };
    let mut search = WindowSearch {
        pid,
        handles: Vec::new(),
    };
    let parameter = &mut search as *mut WindowSearch as LongParameter;
    let enumerated = unsafe { EnumWindows(Some(collect_window), parameter) };
    if enumerated == 0 {
        return Err(io::Error::last_os_error());
    }

    for handle in search.handles {
        let posted = unsafe { PostMessageW(handle, WM_CLOSE, 0, 0) };
        if posted == 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{ProcessSpawner, ProcessSpec, TokioProcessSpawner};

    #[tokio::test]
    async fn runs_program_with_separate_arguments_and_environment() {
        let spec = ProcessSpec {
            program: std::env::current_exe().expect("test executable should be available"),
            args: ["--exact", "process::tests::child_helper", "--nocapture"]
                .into_iter()
                .map(OsString::from)
                .collect(),
            cwd: std::env::current_dir().expect("current directory should be available"),
            env: vec![(
                OsString::from("NEO_PROCESS_CHILD_HELPER"),
                OsString::from("1"),
            )],
        };

        let output = TokioProcessSpawner
            .run_to_end(spec, Duration::from_secs(10))
            .await
            .expect("helper process should finish");

        assert_eq!(output.exit_code, Some(0));
        assert!(output.stdout.contains("child process marker"));
    }

    #[test]
    fn child_helper() {
        if std::env::var("NEO_PROCESS_CHILD_HELPER").as_deref() == Ok("hold") {
            std::thread::sleep(Duration::from_millis(500));
            println!("child process marker");
        } else if std::env::var_os("NEO_PROCESS_CHILD_HELPER").is_some() {
            println!("child process marker");
        }
    }

    #[test]
    fn process_spec_keeps_program_and_arguments_as_path_values() {
        let spec = ProcessSpec {
            program: PathBuf::from("/path/to/program"),
            args: vec![OsString::from("one argument")],
            cwd: PathBuf::from("/working/directory"),
            env: Vec::new(),
        };

        assert_eq!(spec.program, PathBuf::from("/path/to/program"));
        assert_eq!(spec.args.as_slice(), &[OsString::from("one argument")]);
    }
}
