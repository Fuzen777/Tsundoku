//! The one place that starts external programs (yt-dlp, ffmpeg, Deno). Each starts without a console window and, on
//! Windows, inside a job object, so ending it also ends every process it started (yt-dlp's ffmpeg) and nothing outlives
//! the app. Its output is read line by line as it comes.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::cancel::CancelToken;

/// How often a running program is checked for cancellation, its time limit and its end.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A program to run, with its arguments.
#[derive(Debug, Clone)]
pub struct ProgramCommand {
    program: PathBuf,
    args: Vec<OsString>,
}

impl ProgramCommand {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn program(&self) -> &Path {
        &self.program
    }
}

/// One line a program wrote. A carriage return also ends a line, so progress drawn in place arrives line by line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

impl OutputLine {
    pub fn text(&self) -> &str {
        match self {
            Self::Stdout(text) | Self::Stderr(text) => text,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("couldn't start {program}: {source}")]
    Start { program: PathBuf, source: io::Error },
    #[error("cancelled")]
    Cancelled,
    #[error("didn't finish in time")]
    TimedOut,
    #[error("couldn't wait for the program: {0}")]
    Wait(io::Error),
}

/// A finished program's exit status and everything it wrote.
#[derive(Debug)]
pub struct CollectedOutput {
    pub status: ExitStatus,
    pub lines: Vec<OutputLine>,
}

impl CollectedOutput {
    pub fn stdout(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().filter_map(|line| match line {
            OutputLine::Stdout(text) => Some(text.as_str()),
            OutputLine::Stderr(_) => None,
        })
    }

    /// The last line it wrote to either output, which is usually the error when it failed.
    pub fn last_line(&self) -> Option<&str> {
        self.lines.last().map(OutputLine::text)
    }
}

/// Runs a program to its end, handing each line it writes to `on_line`. Ends it, and everything it started, when
/// `cancel` is set or after `timeout`.
pub fn run(
    command: &ProgramCommand,
    cancel: &CancelToken,
    timeout: Option<Duration>,
    mut on_line: impl FnMut(OutputLine),
) -> Result<ExitStatus, RunError> {
    let started = Instant::now();
    let mut process = SpawnedProcess::start(command)?;
    let (sender, lines) = mpsc::channel();
    if let Some(stdout) = process.child.stdout.take() {
        forward_lines(stdout, OutputLine::Stdout, sender.clone());
    }
    if let Some(stderr) = process.child.stderr.take() {
        forward_lines(stderr, OutputLine::Stderr, sender);
    }

    let mut stopped_because = None;
    loop {
        // The channel closes once both outputs are closed, which is when the program and everything it started ended.
        match lines.recv_timeout(POLL_INTERVAL) {
            Ok(line) => on_line(line),
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                // Something it started may outlive it and hold its output open; end that too, so reading stops.
                if process.has_exited() {
                    process.end_tree();
                }
            }
        }
        if stopped_because.is_none() {
            if cancel.is_cancelled() {
                stopped_because = Some(RunError::Cancelled);
            } else if timeout.is_some_and(|limit| started.elapsed() >= limit) {
                stopped_because = Some(RunError::TimedOut);
            }
            if stopped_because.is_some() {
                process.end_tree();
            }
        }
    }

    let status = process.child.wait().map_err(RunError::Wait)?;
    match stopped_because {
        Some(reason) => Err(reason),
        None => Ok(status),
    }
}

/// Runs a program to its end and returns everything it wrote. Ends it after `timeout`.
pub fn run_collecting(
    command: &ProgramCommand,
    timeout: Duration,
) -> Result<CollectedOutput, RunError> {
    run_collecting_with_cancel(command, &CancelToken::new(), timeout)
}

/// Like `run_collecting`, and can be cancelled.
pub fn run_collecting_with_cancel(
    command: &ProgramCommand,
    cancel: &CancelToken,
    timeout: Duration,
) -> Result<CollectedOutput, RunError> {
    let mut lines = Vec::new();
    let status = run(command, cancel, Some(timeout), |line| lines.push(line))?;
    Ok(CollectedOutput { status, lines })
}

/// Reads `source` on its own thread and sends each line, until the output closes or nobody listens anymore.
fn forward_lines(
    source: impl Read + Send + 'static,
    wrap: fn(String) -> OutputLine,
    sender: Sender<OutputLine>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(source);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    for segment in buffer.split(|&byte| byte == b'\n' || byte == b'\r') {
                        if segment.is_empty() {
                            continue;
                        }
                        let line = wrap(String::from_utf8_lossy(segment).into_owned());
                        if sender.send(line).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });
}

struct SpawnedProcess {
    child: Child,
    /// Dropping it (on Windows, closing the job) ends anything still running in it.
    tree: process_tree::ProcessTree,
}

impl SpawnedProcess {
    fn start(command: &ProgramCommand) -> Result<Self, RunError> {
        let mut std_command = Command::new(&command.program);
        std_command
            .args(&command.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (child, tree) =
            process_tree::spawn(&mut std_command).map_err(|source| RunError::Start {
                program: command.program.clone(),
                source,
            })?;
        Ok(Self { child, tree })
    }

    fn has_exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    /// Ends the program and everything it started.
    fn end_tree(&mut self) {
        self.tree.end(&mut self.child);
    }
}

#[cfg(windows)]
mod process_tree {
    use std::io;
    use std::mem;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command};
    use std::ptr;

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
    };

    /// The job object holding a program and everything it starts. Closing it, when this is dropped, ends them all.
    pub struct ProcessTree {
        job: OwnedHandle,
    }

    impl ProcessTree {
        /// Ends every process in the job, `child` included.
        pub fn end(&self, _child: &mut Child) {
            // SAFETY: `job` is a job object handle this struct owns, open until it's dropped.
            unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) };
        }
    }

    /// Starts the program without a console window, inside a new job object. It starts suspended so it can't start
    /// anything before it's in the job, and is resumed once it is.
    pub fn spawn(command: &mut Command) -> io::Result<(Child, ProcessTree)> {
        let job = create_job()?;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        // SAFETY: both handles are open: the job is owned above, the process handle by `child`.
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } != 0;
        let started = if assigned {
            resume_threads(child.id())
        } else {
            Err(io::Error::last_os_error())
        };
        if let Err(error) = started {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, ProcessTree { job }))
    }

    fn create_job() -> io::Result<OwnedHandle> {
        // SAFETY: null attributes and name are allowed: a default, unnamed job.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `handle` is a new, valid handle that nothing else owns.
        let job = unsafe { OwnedHandle::from_raw_handle(handle) };

        // SAFETY: the struct is plain data, for which all zeros is a valid value.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the pointer and size describe `limits`, which outlives the call.
        let set = unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Resumes the threads of a process started suspended (just its main thread, in practice).
    fn resume_threads(process_id: u32) -> io::Result<()> {
        // SAFETY: a plain call; the result is checked below.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `snapshot` is a new, valid handle that nothing else owns.
        let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };

        // SAFETY: the struct is plain data, for which all zeros is a valid value.
        let mut entry: THREADENTRY32 = unsafe { mem::zeroed() };
        entry.dwSize = mem::size_of::<THREADENTRY32>() as u32;
        let mut resumed_any = false;
        // SAFETY: `entry` is a valid THREADENTRY32 with its size set, as the API requires.
        let mut has_entry = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
        while has_entry {
            if entry.th32OwnerProcessID == process_id {
                // SAFETY: a plain call; the result is checked below.
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if !thread.is_null() {
                    // SAFETY: `thread` is a new, valid handle that nothing else owns.
                    let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
                    // SAFETY: `thread` is open with THREAD_SUSPEND_RESUME access.
                    resumed_any |= unsafe { ResumeThread(thread.as_raw_handle()) } != u32::MAX;
                }
            }
            // SAFETY: as for Thread32First.
            has_entry = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
        }

        if resumed_any {
            Ok(())
        } else {
            Err(io::Error::other(
                "couldn't resume the program after starting it",
            ))
        }
    }
}

#[cfg(not(windows))]
mod process_tree {
    use std::io;
    use std::process::{Child, Command};

    /// Without job objects, only the program itself is ended.
    pub struct ProcessTree;

    impl ProcessTree {
        pub fn end(&self, child: &mut Child) {
            let _ = child.kill();
        }
    }

    pub fn spawn(command: &mut Command) -> io::Result<(Child, ProcessTree)> {
        Ok((command.spawn()?, ProcessTree))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn cmd(script: &str) -> ProgramCommand {
        ProgramCommand::new("cmd.exe").args(["/d", "/c", script])
    }

    #[test]
    fn collects_both_outputs_line_by_line() {
        let output = run_collecting(
            &cmd("echo first&& 1>&2 echo second&& echo third"),
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout().collect::<Vec<_>>(), ["first", "third"]);
        assert!(output.lines.contains(&OutputLine::Stderr("second".into())));
    }

    #[test]
    fn reports_the_exit_code() {
        let output = run_collecting(&cmd("exit 3"), Duration::from_secs(10)).unwrap();
        assert_eq!(output.status.code(), Some(3));
    }

    #[test]
    fn a_missing_program_fails_to_start() {
        let error = run_collecting(
            &ProgramCommand::new(r"C:\does\not\exist.exe"),
            Duration::from_secs(10),
        )
        .unwrap_err();
        assert!(
            matches!(error, RunError::Start { source, .. } if source.kind() == io::ErrorKind::NotFound)
        );
    }

    #[test]
    fn a_timeout_ends_the_program() {
        let started = Instant::now();
        let error =
            run_collecting(&cmd("ping -n 30 127.0.0.1"), Duration::from_millis(300)).unwrap_err();
        assert!(matches!(error, RunError::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    /// cmd.exe starts ping, which holds the output open: the run only ends quickly if ping is ended with cmd.exe.
    #[test]
    fn cancelling_ends_everything_the_program_started() {
        let cancel = CancelToken::new();
        let canceller = cancel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            canceller.cancel();
        });

        let started = Instant::now();
        let error = run(&cmd("ping -n 30 127.0.0.1"), &cancel, None, |_| {}).unwrap_err();
        assert!(matches!(error, RunError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
