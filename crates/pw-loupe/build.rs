//! Embeds a manifest that asks Windows for Common Controls version 6 (T47).
//!
//! It was needed while the retired Tauri app's `tauri-plugin-dialog` shared `rfd` in this
//! workspace and so built it with the `common-controls-v6` feature: `rfd`'s message box then
//! imported `TaskDialogIndirect` from `comctl32.dll`, which exists only in version 6, and without
//! the manifest Windows loaded version 5 and the program exited at start with 0xC0000139. No
//! package turns that feature on any longer; the manifest is kept so the program's dialogs look the
//! same.
//!
//! It also passes the commit the program is built from to the compiler as `RAWSTRIDE_COMMIT`, for
//! the window "About RawStride" (ADR 0031 RL-Q9, T225): the first 12 characters of `HEAD`, or
//! `unknown` when there is no Git repository (a build from GitHub's source archive). A failing
//! `git` never fails the build and prints no warning.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").ok();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").ok();
    if target_os.as_deref() == Some("windows") && target_env.as_deref() == Some("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
    let commit = manifest_dir
        .as_deref()
        .and_then(|dir| git(dir, &["rev-parse", "--verify", "-q", "HEAD"]))
        .filter(|hash| is_full_hash(hash))
        .and_then(|hash| hash.get(..12).map(str::to_owned))
        // The same text as `shell::UNKNOWN_COMMIT`.
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=RAWSTRIDE_COMMIT={commit}");

    if let Some(dir) = manifest_dir.as_deref() {
        watch_git_files(dir);
    }
}

/// Runs `git` with `args` in `dir` and gives its trimmed standard output, or `None` when `git`
/// cannot be started or exits with an error. Its standard error is captured, not printed.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned())
}

/// A full commit hash: 40 (SHA-1) or 64 (SHA-256) characters of `0-9a-f`.
fn is_full_hash(text: &str) -> bool {
    (text.len() == 40 || text.len() == 64)
        && text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Makes a new commit or checkout run this script again: `HEAD`, `logs/HEAD` (appended on every
/// commit and checkout, also when the branch's ref is only in `packed-refs`), the branch's ref and
/// `packed-refs`, each only if it exists (a missing path would run the script on every build).
/// Works in a linked worktree, where `.git` is a file.
fn watch_git_files(dir: &Path) {
    let mut names = vec![
        "HEAD".to_owned(),
        "logs/HEAD".to_owned(),
        "packed-refs".to_owned(),
    ];
    if let Some(reference) = git(dir, &["symbolic-ref", "-q", "HEAD"])
        && !reference.is_empty()
    {
        names.push(reference);
    }
    for name in names {
        let Some(path) = git(dir, &["rev-parse", "--git-path", &name]) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        let path = dir.join(path);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
