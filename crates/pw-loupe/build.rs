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
//!
//! On Windows with MSVC it also gives `rawstride.exe` a version resource and an icon (ADR 0031
//! RL2 / RL-Q11, T226): it writes `rawstride.rc` into `OUT_DIR`, compiles it with the Windows
//! SDK's `rc.exe` and passes the `.res` file to the linker. Without `rc.exe` the build warns and
//! goes on; the test `tests/exe_resources.rs` then fails.

use std::env;
use std::ffi::OsString;
use std::fs;
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
        println!("cargo:rerun-if-changed=assets/rawstride.ico");
        println!("cargo:rerun-if-env-changed=WindowsSdkDir");
        match version_resource() {
            Ok(res) => println!("cargo:rustc-link-arg-bins={}", res.display()),
            Err(why) => println!(
                "cargo:warning=rawstride.exe is built without a version resource and icon: {why}"
            ),
        }
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

/// Writes `rawstride.rc` into `OUT_DIR`, compiles it with `rc.exe` and gives the path of the
/// `.res` file, or why it could not.
fn version_resource() -> Result<PathBuf, String> {
    let out_dir = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or("OUT_DIR is not set")?;
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or("CARGO_MANIFEST_DIR is not set")?;
    let icon = manifest_dir.join("assets").join("rawstride.ico");
    let rc = out_dir.join("rawstride.rc");
    let res = out_dir.join("rawstride.res");
    fs::write(&rc, rc_script(&icon)?).map_err(|e| format!("cannot write {}: {e}", rc.display()))?;

    let rc_exe = find_rc_exe().ok_or("the Windows SDK's rc.exe was not found")?;
    let output = Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/c65001")
        .arg("/fo")
        .arg(&res)
        .arg(&rc)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", rc_exe.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "{} failed ({}): {} {}",
            rc_exe.display(),
            output.status,
            stderr.trim(),
            stdout.trim()
        ));
    }
    Ok(res)
}

/// The resource script: UTF-8 without BOM, no `#include` (every constant is a number), the icon
/// and the version resource, no manifest (the linker embeds that one).
fn rc_script(icon: &Path) -> Result<String, String> {
    let var = |name: &str| env::var(name).map_err(|_| format!("{name} is not set"));
    let major = var("CARGO_PKG_VERSION_MAJOR")?;
    let minor = var("CARGO_PKG_VERSION_MINOR")?;
    let patch = var("CARGO_PKG_VERSION_PATCH")?;
    let version = rc_text(&var("CARGO_PKG_VERSION")?);
    // The same sentence as `AboutText::Copyright` in `shell_text.rs`, which a build script
    // cannot call.
    let authors = var("CARGO_PKG_AUTHORS")?
        .split(':')
        .collect::<Vec<_>>()
        .join(", ");
    let copyright = rc_text(&format!("Copyright © 2026 {authors}"));
    let icon = icon
        .to_str()
        .ok_or("the icon's path is not Unicode")?
        .replace('\\', r"\\");
    let icon = rc_text(&icon);
    Ok(format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "FileDescription", "RawStride"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "rawstride"
            VALUE "LegalCopyright", "{copyright}"
            VALUE "OriginalFilename", "rawstride.exe"
            VALUE "ProductName", "RawStride"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    ))
}

/// A value for a quoted string in the resource script: a `"` is written `""`.
fn rc_text(text: &str) -> String {
    text.replace('"', "\"\"")
}

/// The Windows SDK's `rc.exe` for the build machine: under the SDK root's `bin\`, the folder with
/// the highest version (names like `10.0.26100.0`) that holds `<arch>\rc.exe`.
fn find_rc_exe() -> Option<PathBuf> {
    let root = match env::var_os("WindowsSdkDir") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let program_files = env::var_os("ProgramFiles(x86)").filter(|dir| !dir.is_empty())?;
            PathBuf::from(program_files).join("Windows Kits").join("10")
        }
    };
    let host = env::var("HOST").ok()?;
    let arch = if host.starts_with("x86_64") {
        "x64"
    } else if host.starts_with("aarch64") {
        "arm64"
    } else if host.starts_with("i686") {
        "x86"
    } else {
        return None;
    };
    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    for entry in fs::read_dir(root.join("bin")).ok()?.flatten() {
        let name: OsString = entry.file_name();
        let Some(version) = name.to_str().and_then(sdk_version) else {
            continue;
        };
        let rc_exe = entry.path().join(arch).join("rc.exe");
        if !rc_exe.is_file() {
            continue;
        }
        if best.as_ref().is_none_or(|(highest, _)| version > *highest) {
            best = Some((version, rc_exe));
        }
    }
    best.map(|(_, rc_exe)| rc_exe)
}

/// The numeric parts of an SDK folder's name like `10.0.26100.0`, or `None` for another name.
fn sdk_version(name: &str) -> Option<Vec<u32>> {
    let parts = name
        .split('.')
        .map(|part| part.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    (parts.len() >= 2).then_some(parts)
}
