//! Test-only stand-in for ExifTool, so process and protocol failures can be tested without
//! ExifTool, Perl or any other external program.
//!
//! The unit-test binary re-runs itself with `--ignored --exact` so that only
//! [`fake_exiftool_entry`] runs in the child. That function speaks the `-stay_open` protocol on
//! stdin/stdout and behaves according to the `fake-exiftool-mode=…` argument.

use std::io::{BufRead, Write};
use std::time::Duration;

use crate::ExifTool;

const ENTRY: &str = "test_support::fake_exiftool_entry";
const MODE_PREFIX: &str = "fake-exiftool-mode=";

/// In the `slow` mode, a command with a line holding this (a file's name) is answered only after
/// [`SLOW_WAIT`] (T266).
pub(crate) const SLOW_MARKER: &str = "pwslowmarker";
/// In the `slow` mode, a command with a line holding this makes the fake exit without answering
/// (T266: a process that breaks).
pub(crate) const EXIT_MARKER: &str = "pwexitmarker";
/// How long the `slow` mode waits before it answers a command naming [`SLOW_MARKER`].
pub(crate) const SLOW_WAIT: Duration = Duration::from_millis(500);
/// A leading argument after the fake's own, `<prefix><file>`: in the `slow` mode the fake appends
/// each line holding [`SLOW_MARKER`] to the file when it reads it (T266: which writes reached an
/// ExifTool process).
pub(crate) const LOG_PREFIX: &str = "fake-exiftool-log=";

/// Program and leading arguments that start the fake ExifTool in `mode`:
/// - `normal`: answers every command with "1 image files updated".
/// - `invalid-utf8`: like `normal`, but the first answer also contains invalid UTF-8.
/// - `exit-after-first`: answers the first command, then exits without answering.
/// - `hang`: answers commands but ignores `-stay_open False` and end-of-input.
/// - `version=<text>`: answers a `-ver` command with `<text>` and every other command like
///   `normal` (T94).
/// - `exit-after-first-request`: answers every `-ver` command with `13.64`, answers the first
///   other command, then exits at the next other command without answering (T94: the worker
///   asks `-ver` when it starts a process).
/// - `slow`: answers every `-ver` command with `13.64` and every other command like `normal`, but
///   a command with a line holding [`SLOW_MARKER`] only after [`SLOW_WAIT`], and at a command
///   with a line holding [`EXIT_MARKER`] it exits without answering (T266). With a leading
///   argument [`LOG_PREFIX`]`<file>` it logs the lines holding [`SLOW_MARKER`] to that file.
pub(crate) fn fake_exiftool(mode: &str) -> (String, Vec<String>) {
    let exe = std::env::current_exe().expect("test binary path");
    (exe.to_string_lossy().into_owned(), fake_args(mode))
}

/// Leading arguments for the fake, for use with a copy of the test binary.
pub(crate) fn fake_args(mode: &str) -> Vec<String> {
    vec![
        "--ignored".into(),
        "--exact".into(),
        "--test-threads=1".into(),
        "--quiet".into(),
        // Everything after `--` is a test-name filter for libtest, including the
        // `-stay_open True -@ -` that `ExifTool::start_with` appends; only ENTRY matches.
        "--".into(),
        ENTRY.into(),
        format!("{MODE_PREFIX}{mode}"),
    ]
}

/// Starts the fake ExifTool in `mode`.
pub(crate) fn start_fake(mode: &str) -> ExifTool {
    let (program, args) = fake_exiftool(mode);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    ExifTool::start_with(&program, &args).expect("fake ExifTool should start")
}

#[test]
#[ignore = "helper process for other tests; they start it, it is not a test itself"]
fn fake_exiftool_entry() {
    let Some(mode) = std::env::args().find_map(|a| a.strip_prefix(MODE_PREFIX).map(str::to_owned))
    else {
        return;
    };
    run(&mode);
    std::process::exit(0);
}

fn run(mode: &str) {
    // Write through the raw handle: libtest only captures the print! macros.
    let mut out = std::io::stdout();
    let version = mode.strip_prefix("version=");
    let mut commands = 0u32;
    let mut other_commands = 0u32;
    let mut asks_version = false;
    let mut names_slow = false;
    let mut names_exit = false;
    let log = std::env::args().find_map(|a| a.strip_prefix(LOG_PREFIX).map(str::to_owned));
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line == "False" && mode != "hang" {
            return;
        }
        if line == "-ver" {
            asks_version = true;
        }
        if mode == "slow"
            && line.contains(SLOW_MARKER)
            && let Some(log) = &log
        {
            let logged = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .and_then(|mut file| file.write_all(format!("{line}\n").as_bytes()));
            if logged.is_err() {
                return;
            }
        }
        names_slow |= line.contains(SLOW_MARKER);
        names_exit |= line.contains(EXIT_MARKER);
        let Some(seq) = line.strip_prefix("-execute") else {
            continue;
        };
        let is_version = std::mem::take(&mut asks_version);
        let is_slow = std::mem::take(&mut names_slow);
        let is_exit = std::mem::take(&mut names_exit);
        commands += 1;
        if mode == "exit-after-first" && commands > 1 {
            return;
        }
        if mode == "slow" && is_exit {
            return;
        }
        if mode == "slow" && is_slow {
            std::thread::sleep(SLOW_WAIT);
        }
        let version = match (mode, version) {
            (_, Some(text)) if is_version => Some(text),
            ("exit-after-first-request" | "slow", _) if is_version => Some("13.64"),
            _ => None,
        };
        if mode == "exit-after-first-request" && !is_version {
            other_commands += 1;
            if other_commands > 1 {
                return;
            }
        }
        let mut answer = Vec::new();
        if mode == "invalid-utf8" && commands == 1 {
            answer.extend_from_slice(b"first-only \xff\xfe\n");
        }
        match version {
            Some(text) => answer.extend_from_slice(format!("{text}\n{{ready{seq}}}\n").as_bytes()),
            None => answer.extend_from_slice(
                format!("    1 image files updated\n{{ready{seq}}}\n").as_bytes(),
            ),
        }
        if out.write_all(&answer).and_then(|()| out.flush()).is_err() {
            return;
        }
    }
    if mode == "hang" {
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
}
