//! Embeds a manifest that asks Windows for Common Controls version 6 (T47).
//!
//! It was needed while the retired Tauri app's `tauri-plugin-dialog` shared `rfd` in this
//! workspace and so built it with the `common-controls-v6` feature: `rfd`'s message box then
//! imported `TaskDialogIndirect` from `comctl32.dll`, which exists only in version 6, and without
//! the manifest Windows loaded version 5 and the program exited at start with 0xC0000139. No
//! package turns that feature on any longer; the manifest is kept so the program's dialogs look the
//! same.

use std::env;

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
}
