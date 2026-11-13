# T47. pw-loupe does not start since T44: embed the Common Controls 6 manifest — Done

- **Do this first; it blocks the hand checks of T43, T44 and T46.** For a Claude agent.
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/build.rs`; `crates/pw-loupe/Cargo.toml` only
  if Cargo needs `build = "build.rs"` (it finds `build.rs` by itself, so normally no change). No new
  dependencies.
- **What happens (owner, 2026-11-09):** since T44, `target\debug\pw-loupe.exe` exits at once without
  a window or a message, exit code `-1073741511` = `0xC0000139` (STATUS_ENTRYPOINT_NOT_FOUND: Windows
  cannot find a function the program imports). The T43 + T45 build starts normally.
- **Likely cause:** T44 added `rfd::MessageDialog`. In this workspace `rfd` 0.17.2 is shared with the
  Tauri app's `tauri-plugin-dialog`, and Cargo merges the features of a shared crate, so `rfd` is
  probably built with `common-controls-v6`. Its message box then imports `TaskDialogIndirect` from
  `comctl32.dll`, which exists only in Common Controls version 6. Windows loads version 6 only when the
  program's manifest asks for it; the Tauri app has such a manifest, `pw-loupe` has none. The test
  binary still loads because nothing in it calls `on_exit`, so the linker drops the import (that is
  why `check.ps1` stayed green).
- **What:**
  1. **Confirm first** and paste the output in the report:
     `cargo tree --workspace -e features -i rfd` (shows which crate enables `common-controls-v6`). If
     it is **not** enabled, stop and report; the fix below would then be the wrong one.
  2. Add `crates/pw-loupe/build.rs` that, only when `CARGO_CFG_TARGET_OS` is `windows` and
     `CARGO_CFG_TARGET_ENV` is `msvc`, prints
     ```
     cargo:rustc-link-arg-bins=/MANIFEST:EMBED
     cargo:rustc-link-arg-bins=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'
     ```
     and `cargo:rerun-if-changed=build.rs`. No `unwrap`/`expect` (read the variables with `match` or
     `.ok()`). A short comment explains why (this task, ADR 0013 T44).
  3. Nothing else changes; `on_exit` and the message box stay.
- **Note:** a new build script may be stopped by the owner's antivirus (LNK1104, rules 6–7). Build
  only with `cargo build --workspace` in `target`; if it is blocked, stop and tell the owner.
- **Done when:** `.\tools\check.ps1` is all OK, **and** `cargo build --workspace` followed by
  `.\target\debug\pw-loupe.exe D:\PhotoWorkflowData\RAW` opens the window (report `$LASTEXITCODE`
  after closing it: `0`). Then the owner checks by hand: rate an image, close the window at once,
  reopen the folder → the rating is there; and, with a read-only sidecar, the message box "Betyg som
  inte sparades" appears at exit.
