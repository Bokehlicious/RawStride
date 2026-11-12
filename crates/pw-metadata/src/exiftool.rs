//! A persistent ExifTool process in `-stay_open` mode.
//!
//! [`ExifTool::version`] asks the running process for its version (ADR 0007 §11: "ExifTool
//! version: at least 12.67, checked at start", from IPTC §5.1). [`MIN_EXIFTOOL_VERSION`] is that
//! minimum; the safe write (§9) also relies on newer features (`-api ImageHashType`,
//! `ImageDataHash`). The metadata worker asks it each time it starts a process (T94).

use std::fmt;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{Error, Result};

/// How long `Drop` waits for ExifTool to exit on its own before killing it.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// An ExifTool version as ExifTool prints it with `-ver` (`13.59`). The field order makes the
/// derived order numeric: 12.67 < 13.10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ToolVersion {
    pub major: u32,
    pub minor: u32,
}

/// The oldest ExifTool the program writes with (ADR 0007 §11, D9).
pub const MIN_EXIFTOOL_VERSION: ToolVersion = ToolVersion {
    major: 12,
    minor: 67,
};

impl ToolVersion {
    /// Parses the answer to `-ver`: the trimmed text must be two runs of ASCII digits around one
    /// `.` (`13.59`). Anything else (`13`, `13.5.1`, `v13.59`, an empty text) is `None`.
    pub fn parse(text: &str) -> Option<ToolVersion> {
        let (major, minor) = text.trim().split_once('.')?;
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        if !digits(major) || !digits(minor) {
            return None;
        }
        Some(ToolVersion {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
        })
    }
}

impl fmt::Display for ToolVersion {
    /// `major.minor` with the minor part in two digits (`12.67`, `13.05`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.major, self.minor)
    }
}

/// One running `exiftool -stay_open True -@ -` process. Commands are serialised; wrap it in a
/// `Mutex` to share it between threads.
///
/// After any I/O failure or unexpected exit the request/response stream can no longer be trusted,
/// so the instance is marked broken ([`ExifTool::is_broken`]) and refuses further commands.
/// Callers that need to keep going should drop it and start a new one.
///
/// Dropping an instance asks ExifTool to exit and waits up to two seconds before killing it, so
/// drop it on a worker thread rather than the UI thread.
#[derive(Debug)]
pub struct ExifTool {
    child: Child,
    /// `None` only while shutting down: taken in `Drop` so the pipe closes before waiting.
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: Arc<Mutex<String>>,
    seq: u64,
    broken: bool,
    /// The program and leading arguments it was started with, for [`ExifTool::restart`].
    program: String,
    pre_args: Vec<String>,
}

impl ExifTool {
    /// Starts `exiftool` from PATH.
    pub fn start() -> Result<Self> {
        Self::start_with("exiftool", &[])
    }

    /// Starts ExifTool with an explicit program and leading arguments
    /// (e.g. `("perl", &["/path/to/exiftool"])` in tests).
    pub fn start_with(program: &str, pre_args: &[&str]) -> Result<Self> {
        let mut cmd = Command::new(program);
        cmd.args(pre_args)
            .args(["-stay_open", "True", "-@", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| {
            Error::ExifTool(format!(
                "could not start {program}: {e} (is ExifTool on PATH?)"
            ))
        })?;
        let (stdin, stdout) = match (child.stdin.take(), child.stdout.take()) {
            (Some(stdin), Some(stdout)) => (stdin, BufReader::new(stdout)),
            (stdin, stdout) => {
                let missing = if stdin.is_none() { "stdin" } else { "stdout" };
                drop((stdin, stdout));
                let mut message = format!("could not access ExifTool {missing} pipe");
                let kill_error = child.kill().err();
                if let Err(wait_error) = child.wait() {
                    let _ = write!(message, "; failed to wait for ExifTool child: {wait_error}");
                    if let Some(kill_error) = kill_error {
                        let _ = write!(message, "; failed to stop child: {kill_error}");
                    }
                }
                return Err(Error::ExifTool(message));
            }
        };
        let stderr_pipe = child.stderr.take();
        // From here on `Drop` owns the cleanup of the child process, also on the error path below.
        let tool = Self {
            child,
            stdin: Some(stdin),
            stdout,
            stderr: Arc::new(Mutex::new(String::new())),
            seq: 0,
            broken: false,
            program: program.to_string(),
            pre_args: pre_args.iter().map(|arg| arg.to_string()).collect(),
        };
        if let Some(err) = stderr_pipe {
            let sink = Arc::clone(&tool.stderr);
            std::thread::Builder::new()
                .name("pw-exiftool-stderr".into())
                .spawn(move || {
                    for line in BufReader::new(err).lines().map_while(|l| l.ok()) {
                        let mut s = sink.lock().unwrap_or_else(|e| e.into_inner());
                        s.push_str(&line);
                        s.push('\n');
                    }
                })
                .map_err(|e| {
                    Error::ExifTool(format!("could not start ExifTool stderr reader: {e}"))
                })?;
        }
        Ok(tool)
    }

    /// Stops this ExifTool process and starts a new one with the same program and arguments
    /// (T92). The safe write needs it: measured with ExifTool 13.59, after a process has written
    /// `-IPTCDigest=new` to a TIFF whose Photoshop block holds an `IPTCDigest` and an `AgHg`
    /// resource, that process reads the Photoshop block of such TIFFs wrongly until it ends (two
    /// more validation warnings, "Non-standard Photoshop at TIFF-IFD0-Photoshop-IPTCDigest" and
    /// "Bad Photoshop IRB resource"), even of files it never wrote. If the new process cannot be
    /// started this instance is marked broken, so callers that start a new one when
    /// [`ExifTool::is_broken`] do so.
    pub fn restart(&mut self) -> Result<()> {
        let pre_args: Vec<&str> = self.pre_args.iter().map(String::as_str).collect();
        match Self::start_with(&self.program, &pre_args) {
            Ok(fresh) => {
                // The old process is stopped when it is dropped here.
                *self = fresh;
                Ok(())
            }
            Err(error) => {
                self.broken = true;
                Err(error)
            }
        }
    }

    /// Asks the running process for its version (`-ver`). The last non-empty line of the standard
    /// output (owner's decision in T94) must be accepted by [`ToolVersion::parse`]; otherwise
    /// the answer is an [`Error::ExifTool`] naming that line. Messages on stderr and text before
    /// the version (the test fake's libtest header) do not make a good version unreadable.
    pub fn version(&mut self) -> Result<ToolVersion> {
        let answer = self.execute(&["-ver".to_string()])?;
        let line = version_line(&answer);
        ToolVersion::parse(line)
            .ok_or_else(|| Error::ExifTool(format!("unreadable ExifTool version: {line}")))
    }

    /// True after an I/O failure or unexpected exit; the instance then rejects all commands.
    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// Runs one ExifTool command (one argument per element) and returns its standard output.
    /// Warnings and errors printed by ExifTool are appended after a line "--- stderr ---".
    ///
    /// Output that is not valid UTF-8 is decoded lossily (U+FFFD) instead of failing, so a
    /// response is always read up to its end marker and the stream stays in step.
    pub fn execute(&mut self, args: &[String]) -> Result<String> {
        if self.broken {
            return Err(Error::ExifTool(
                "ExifTool process is unusable after an earlier failure".into(),
            ));
        }
        // Validate before writing anything: one argument per line in the -stay_open protocol.
        if args.iter().any(|a| a.contains(['\n', '\r'])) {
            return Err(Error::ExifTool("argument contains a line break".into()));
        }
        self.seq = self.seq.wrapping_add(1);
        // Arguments are passed as UTF-8; tell ExifTool to treat file names the same way.
        let mut cmd = String::from("-charset\nfilename=utf8\n");
        for a in args {
            cmd.push_str(a);
            cmd.push('\n');
        }
        let _ = writeln!(cmd, "-execute{}", self.seq);

        let mut out = match self.round_trip(&cmd) {
            Ok(out) => out,
            Err(error) => {
                // A partial response may still be in the pipe; never reuse this stream.
                self.broken = true;
                return Err(error);
            }
        };
        // Give the stderr reader a moment to catch up with this command's messages.
        std::thread::sleep(Duration::from_millis(2));
        let mut err = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
        if !err.is_empty() {
            out.push_str("--- stderr ---\n");
            out.push_str(&err);
            err.clear();
        }
        Ok(out)
    }

    /// Sends one command and reads standard output up to its `{readyN}` marker.
    fn round_trip(&mut self, cmd: &str) -> Result<String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| Error::ExifTool("ExifTool input is closed".into()))?;
        stdin.write_all(cmd.as_bytes())?;
        stdin.flush()?;

        let ready = format!("{{ready{}}}", self.seq);
        let mut out = String::new();
        let mut buf = Vec::new();
        loop {
            buf.clear();
            if self.stdout.read_until(b'\n', &mut buf)? == 0 {
                return Err(Error::ExifTool("ExifTool exited unexpectedly".into()));
            }
            let line = String::from_utf8_lossy(&buf);
            if line.trim_end() == ready {
                return Ok(out);
            }
            out.push_str(&line);
        }
    }
}

/// The last non-empty line, trimmed, of the standard-output part of an [`ExifTool::execute`]
/// answer (the part before the `--- stderr ---` line), or an empty text when there is none.
fn version_line(answer: &str) -> &str {
    answer
        .lines()
        .take_while(|line| *line != "--- stderr ---")
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .last()
        .unwrap_or("")
}

impl Drop for ExifTool {
    fn drop(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"-stay_open\nFalse\n");
            let _ = stdin.flush();
            // `stdin` is dropped here, so ExifTool also sees end-of-input.
        }
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) | Err(_) => break,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::start_fake;
    use std::sync::mpsc;

    #[test]
    fn invalid_utf8_output_does_not_desync_the_stream() {
        let mut et = start_fake("invalid-utf8");
        let first = et
            .execute(&["-ver".into()])
            .expect("lossy decoding must not fail");
        assert!(first.contains('\u{FFFD}'), "{first:?}");
        assert!(first.contains("first-only"), "{first:?}");
        let second = et.execute(&["-ver".into()]).expect("second command");
        assert!(
            !second.contains("first-only"),
            "leftover from command 1: {second:?}"
        );
        assert!(second.contains("files updated"), "{second:?}");
        assert!(!et.is_broken());
    }

    #[test]
    fn unexpected_exit_marks_the_instance_broken() {
        let mut et = start_fake("exit-after-first");
        assert!(et.execute(&["-ver".into()]).is_ok());
        assert!(et.execute(&["-ver".into()]).is_err());
        assert!(et.is_broken());
        match et.execute(&["-ver".into()]) {
            Err(Error::ExifTool(m)) => assert!(m.contains("unusable"), "{m}"),
            other => panic!("expected an 'unusable' error, got {other:?}"),
        }
    }

    #[test]
    fn line_breaks_are_rejected_without_breaking_the_instance() {
        let mut et = start_fake("normal");
        assert!(et.execute(&["a\rb".into()]).is_err());
        assert!(et.execute(&["a\nb".into()]).is_err());
        assert!(!et.is_broken());
        assert!(et.execute(&["-ver".into()]).is_ok());
    }

    #[test]
    fn restart_starts_a_new_process_that_answers() {
        let mut et = start_fake("normal");
        assert!(et.execute(&["-ver".into()]).is_ok());
        let first = et.child.id();
        assert!(et.restart().is_ok());
        assert_ne!(et.child.id(), first);
        assert!(!et.is_broken());
        assert!(et.execute(&["-ver".into()]).is_ok());
    }

    #[test]
    fn restart_brings_a_broken_instance_back() {
        let mut et = start_fake("exit-after-first");
        assert!(et.execute(&["-ver".into()]).is_ok());
        assert!(et.execute(&["-ver".into()]).is_err());
        assert!(et.is_broken());
        assert!(et.restart().is_ok());
        assert!(!et.is_broken());
        assert!(et.execute(&["-ver".into()]).is_ok());
    }

    #[test]
    fn a_failed_restart_marks_the_instance_broken() {
        let mut et = start_fake("normal");
        et.program = "pw-no-such-exiftool-program".to_string();
        assert!(et.restart().is_err());
        assert!(et.is_broken());
        assert!(et.execute(&["-ver".into()]).is_err());
    }

    fn version(major: u32, minor: u32) -> ToolVersion {
        ToolVersion { major, minor }
    }

    #[test]
    fn a_version_is_two_runs_of_digits_around_one_dot() {
        assert_eq!(ToolVersion::parse("13.59"), Some(version(13, 59)));
        assert_eq!(ToolVersion::parse("12.67"), Some(version(12, 67)));
        assert_eq!(ToolVersion::parse("  13.59 "), Some(version(13, 59)));
        assert_eq!(ToolVersion::parse("13.59\n"), Some(version(13, 59)));
        assert_eq!(ToolVersion::parse("\r\n 12.40\r\n"), Some(version(12, 40)));
    }

    #[test]
    fn other_version_texts_are_not_accepted() {
        for text in [
            "13", "13.5.1", "v13.59", "", " \n", "13.", ".59", "13.5a", "1 3.59", "13,59",
        ] {
            assert_eq!(ToolVersion::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn versions_are_ordered_by_number() {
        assert!(version(12, 66) < version(12, 67));
        assert!(version(12, 67) < version(13, 0));
        assert!(version(12, 99) < version(13, 0));
        assert!(version(12, 67) < version(13, 10));
        assert_eq!(MIN_EXIFTOOL_VERSION, version(12, 67));
    }

    #[test]
    fn a_version_is_shown_with_a_two_digit_minor_part() {
        assert_eq!(version(13, 5).to_string(), "13.05");
        assert_eq!(version(12, 67).to_string(), "12.67");
        assert_eq!(MIN_EXIFTOOL_VERSION.to_string(), "12.67");
    }

    #[test]
    fn the_version_is_the_last_line_of_standard_output() {
        assert_eq!(version_line("13.59\n"), "13.59");
        assert_eq!(version_line("\nrunning 1 test\n13.59\n"), "13.59");
        assert_eq!(
            version_line("13.59\n--- stderr ---\nWarning: something\n"),
            "13.59"
        );
        assert_eq!(version_line("abc\n\n  \n"), "abc");
        assert_eq!(version_line(""), "");
        assert_eq!(version_line("--- stderr ---\n13.59\n"), "");
    }

    #[test]
    fn version_asks_the_process() {
        let mut et = start_fake("version=13.59");
        assert_eq!(et.version().ok(), Some(version(13, 59)));
    }

    #[test]
    fn an_unreadable_version_answer_is_an_error() {
        let mut et = start_fake("version=abc");
        match et.version() {
            Err(Error::ExifTool(m)) => assert_eq!(m, "unreadable ExifTool version: abc"),
            other => panic!("expected an unreadable version, got {other:?}"),
        }
        assert!(!et.is_broken());
    }

    #[test]
    fn drop_kills_a_process_that_ignores_the_stop_request() {
        let mut et = start_fake("hang");
        assert!(et.execute(&["-ver".into()]).is_ok());
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            drop(et);
            let _ = done_tx.send(());
        });
        assert!(
            done_rx
                .recv_timeout(SHUTDOWN_TIMEOUT + Duration::from_secs(10))
                .is_ok(),
            "Drop did not return"
        );
    }
}
