//! A persistent ExifTool process in `-stay_open` mode.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{Error, Result};

/// How long `Drop` waits for ExifTool to exit on its own before killing it.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

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
