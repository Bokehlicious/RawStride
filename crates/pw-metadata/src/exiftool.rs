//! A persistent ExifTool process in `-stay_open` mode.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::{Error, Result};

/// One running `exiftool -stay_open True -@ -` process. Commands are serialised; wrap it in a
/// `Mutex` to share it between threads.
#[derive(Debug)]
pub struct ExifTool {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: Arc<Mutex<String>>,
    seq: u64,
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
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let stderr = Arc::new(Mutex::new(String::new()));
        if let Some(err) = child.stderr.take() {
            let sink = stderr.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(|l| l.ok()) {
                    let mut s = sink.lock().unwrap_or_else(|e| e.into_inner());
                    s.push_str(&line);
                    s.push('\n');
                }
            });
        }
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            seq: 0,
        })
    }

    /// Runs one ExifTool command (one argument per element) and returns its standard output.
    /// Warnings and errors printed by ExifTool are appended after a line "--- stderr ---".
    pub fn execute(&mut self, args: &[String]) -> Result<String> {
        self.seq += 1;
        let mut cmd = String::new();
        // Arguments are passed as UTF-8; tell ExifTool to treat file names the same way.
        for a in ["-charset", "filename=utf8"]
            .iter()
            .map(|s| s.to_string())
            .chain(args.iter().cloned())
        {
            if a.contains('\n') {
                return Err(Error::ExifTool("argument contains a line break".into()));
            }
            cmd.push_str(&a);
            cmd.push('\n');
        }
        cmd.push_str(&format!("-execute{}\n", self.seq));
        self.stdin.write_all(cmd.as_bytes())?;
        self.stdin.flush()?;

        let ready = format!("{{ready{}}}", self.seq);
        let mut out = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(Error::ExifTool("ExifTool exited unexpectedly".into()));
            }
            if line.trim_end() == ready {
                break;
            }
            out.push_str(&line);
        }
        // Give the stderr reader a moment to catch up with this command's messages.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut err = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
        if !err.is_empty() {
            out.push_str("--- stderr ---\n");
            out.push_str(&err);
            err.clear();
        }
        Ok(out)
    }
}

impl Drop for ExifTool {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"-stay_open\nFalse\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}
