//! Embeds a manifest that asks Windows for Common Controls version 6 (T47).
//!
//! In this workspace `rfd` is shared with the Tauri app's `tauri-plugin-dialog`, so Cargo builds
//! it with the `common-controls-v6` feature. Its message box (used by `on_exit`, ADR 0013 T44) then
//! imports `TaskDialogIndirect` from `comctl32.dll`, which exists only in version 6. Without this
//! manifest Windows loads version 5 and the program exits at start with 0xC0000139.

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
