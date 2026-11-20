# T226. ADR 0031 RL2: a version resource and an icon in rawstride.exe, and the window's icon (pw-loupe) — Done

- **Why:** ADR 0031 (accepted by the owner 2026-11-20), step RL2 and RL-Q11: `rawstride.exe`
  carries a Windows version resource (version, product name, copyright) and an icon, so
  Explorer's Properties ▸ Details and Task Manager show them; "without a new dependency if the
  existing `build.rs` and the Build Tools' resource compiler can do it". They can: the Windows
  SDK, which the Build Tools install and the build already needs (`docs/building.md`), has
  `rc.exe`, and the MSVC linker takes the `.res` file it writes as an input. Today the exe has no
  version resource and no icon, and the window's title bar and the taskbar show eframe's default
  egui logo. The icon is the old Tauri app's icon from M1 (2026-11-03; a blue ring with an orange
  square on a dark rounded square), chosen by the owner on 2026-11-20; a new icon later is in
  `backlog.md` ("A new program icon").
- **Builds on T225** (done, 19a23ba), which also changed `build.rs` and `main.rs`: keep all of
  T225's part of `build.rs` (the commit, its `rerun-if-changed` lines).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/assets/rawstride.ico` and
  `crates/pw-loupe/assets/rawstride-128.png` (new, point 1), `crates/pw-loupe/build.rs`
  (points 2–4), `crates/pw-loupe/src/main.rs` (point 5), `crates/pw-loupe/tests/exe_resources.rs`
  (new, point 6), `docs/building.md` (point 7), `docs/test-inventory.md` (point 8), and this
  task's own file (status and log).
- **What:**
  1. **The icon files**, taken from git history unchanged, in **Git Bash** (its `>` keeps the
     bytes; PowerShell 5.1's `>` changes them), with no other folder created:
     ```
     mkdir -p crates/pw-loupe/assets
     git show 4e30ff8~1:src-tauri/icons/icon.ico > crates/pw-loupe/assets/rawstride.ico
     git show 4e30ff8~1:src-tauri/icons/128x128.png > crates/pw-loupe/assets/rawstride-128.png
     sha256sum crates/pw-loupe/assets/*
     ```
     Their SHA-256 must be:
     - `rawstride.ico`: `232b8e9aee84c93c00b31941c075dac97da5ba0e9396ecd74d9d7c0462556195`
       (seven images, 16–256 px);
     - `rawstride-128.png`: `c55a39894ce48d957be5681989e6241a952dc6dcacf2c76ca9bc1ef82e8a8071`
       (128 × 128, RGBA).
  2. **The resource script** (`build.rs`), only when `CARGO_CFG_TARGET_OS` is `windows` and
     `CARGO_CFG_TARGET_ENV` is `msvc` (as the manifest today): write `rawstride.rc` into
     `OUT_DIR`, **UTF-8 without BOM**, with **no `#include`** (`INCLUDE` is not set outside a
     Developer prompt, so every constant is written as a number), holding in this order
     - `#pragma code_page(65001)` as the first line;
     - `1 ICON "<path>"` with the absolute path of `assets/rawstride.ico` under
       `CARGO_MANIFEST_DIR`, its backslashes doubled;
     - `1 VERSIONINFO` (the number 1, **not** `VS_VERSION_INFO`, a `winver.h` macro that without
       the include would become a string name and `GetFileVersionInfoW` would find nothing):
       `FILEVERSION` and `PRODUCTVERSION` `<major>,<minor>,<patch>,0` from
       `CARGO_PKG_VERSION_MAJOR` / `_MINOR` / `_PATCH`; `FILEFLAGSMASK 0x3fL`, `FILEFLAGS 0x0L`,
       `FILEOS 0x40004L` (`VOS_NT_WINDOWS32`), `FILETYPE 0x1L` (`VFT_APP`), `FILESUBTYPE 0x0L`; a
       `StringFileInfo` block `"040904b0"` (US English, Unicode) without `CompanyName`, holding
       `FileDescription` `"RawStride"`, `FileVersion` and `ProductVersion`
       `CARGO_PKG_VERSION` (the full text, a pre-release suffix included), `ProductName`
       `"RawStride"`, `InternalName` `"rawstride"`, `OriginalFilename` `"rawstride.exe"`,
       `LegalCopyright` `"Copyright © 2026 <authors>"` with `<authors>` from `CARGO_PKG_AUTHORS`,
       its `:` separator shown as `", "` (the same sentence as T225's `AboutText::Copyright`; a
       comment names it, since `build.rs` cannot call `shell_text.rs`; the authors may hold
       letters outside ASCII, which the UTF-8 code page carries); and a `VarFileInfo` block
       `"Translation", 0x409, 1200`. A `"` inside a value is written `""`. Nothing is typed by
       hand that names a person (the personal-data step of `check.ps1`);
     - **no** manifest resource: the manifest stays embedded by the linker arguments `build.rs`
       already prints, and a second one would clash.
  3. **Finding `rc.exe`** (`build.rs`, `std` only): the Windows SDK root is `WindowsSdkDir` if
     that environment variable is set (a Developer prompt), otherwise
     `%ProgramFiles(x86)%\Windows Kits\10\` (from the environment variable `ProgramFiles(x86)`).
     Under its `bin\`, take the folder with the highest version (compare the numeric parts of
     names like `10.0.26100.0`; ignore other names) that holds `<arch>\rc.exe`, where `<arch>` is
     `x64` for a `HOST` starting `x86_64`, `arm64` for `aarch64`, `x86` for `i686`. Run it as
     `rc.exe /nologo /c65001 /fo <OUT_DIR>\rawstride.res <OUT_DIR>\rawstride.rc` with
     `Command::output()`.
  4. **Linking it** (`build.rs`): always (whether or not `rc.exe` was found) print
     `cargo:rerun-if-changed=assets/rawstride.ico` and `cargo:rerun-if-env-changed=WindowsSdkDir`,
     so the script runs again after the icon changes or an SDK is installed. On success print
     `cargo:rustc-link-arg-bins=<OUT_DIR>\rawstride.res` (the path as one argument). If `rc.exe`
     is not found or fails, print one `cargo:warning=` line saying `rawstride.exe` is built
     without a version resource and icon and why (with `rc.exe`'s stderr when it ran), and go on:
     the build does not fail (a build without the SDK in the usual place must still work), but
     point 6's test then fails, which is the real guard (also in CI). T225's part of `build.rs`
     is not changed.
  5. **The window's icon** (`main.rs`): a new
     `fn window_icon() -> Option<egui::IconData>` decoding
     `include_bytes!("../assets/rawstride-128.png")` with `eframe::icon_data::from_png_bytes`
     (`None` on an error, no panic), with a doc comment naming ADR 0031 RL-Q11 and T226. Where
     the `egui::ViewportBuilder` is made, `.with_icon(std::sync::Arc::new(icon))` is added when
     it is `Some`; with `None` eframe keeps its own icon. The title bar and the taskbar then show
     RawStride's icon.
  6. **Tests:** see below.
  7. **`docs/building.md`:** one sentence where the Build Tools are listed (or in the section on
     building the program): the build runs the Windows SDK's resource compiler `rc.exe` to give
     `rawstride.exe` its version resource and icon; without it the build warns and goes on.
  8. **`docs/test-inventory.md`** (never read whole; changed by a script, `CLAUDE.md` §3–§4): a
     new row for `crates/pw-loupe/tests/exe_resources.rs` like the other crates' integration test
     rows, pw-loupe's unit test count +1 (point 6's `main.rs` test), and the totals at the top.
- **Tests to add:**
  - `crates/pw-loupe/tests/exe_resources.rs` (new integration test; the whole file under
    `#![cfg(all(windows, target_env = "msvc"))]`, the same condition as `build.rs`): reads the
    version resource of `env!("CARGO_BIN_EXE_rawstride")` with `GetFileVersionInfoSizeW`,
    `GetFileVersionInfoW` and `VerQueryValueW` (`windows-sys`, feature
    `Win32_Storage_FileSystem`, already in `pw-loupe`'s `Cargo.toml`; each `unsafe` block with a
    `// SAFETY:` comment). A string value's length from `VerQueryValueW` counts its terminating
    NUL: read up to the first NUL before comparing. It asserts:
    `\StringFileInfo\040904b0\ProductVersion` equals `env!("CARGO_PKG_VERSION")`;
    `ProductName` and `FileDescription` are `"RawStride"`; `OriginalFilename` is
    `"rawstride.exe"`; `LegalCopyright` starts with `"Copyright © 2026 "` (not compared against
    a name); the `VS_FIXEDFILEINFO` (`\`) has `dwFileVersionMS` / `dwFileVersionLS` giving
    `<major>,<minor>,<patch>,0` of `CARGO_PKG_VERSION_*`.
  - `main.rs` tests: `window_icon()` is `Some` and 128 × 128 with `128 * 128 * 4` bytes.
  - The icon resource itself is not tested by code (reading it would need a `windows-sys`
    feature `pw-loupe` does not have); the hand check covers it.
- **Not in this task:** a new icon design (backlog), `Cargo.toml` (no new dependency or
  feature), the release workflow and `--remap-path-prefix` (RL6), the zip (RL4), signing, any
  other file under `assets/`.
- **Rules:** `docs/tasks/README.md` rules 1–13. No new dependency, no `build-dependencies`. The
  personal-data step of `check.ps1` must stay OK.
- **Hand check (the owner, after Done):** first `cargo build --release -p pw-loupe`
  (`check.ps1` builds only the debug exe). In File Explorer, `target\release\rawstride.exe` shows
  the icon (if Explorer still shows the old plain icon from its cache, copy the exe to a new
  folder, or run `ie4uinit.exe -show`); Properties ▸ Details shows File description
  "RawStride", File version `0.1.0.0` (the numeric version), Product name "RawStride", Product
  version `0.1.0` (the text), Copyright with a correct © and the author's name spelled right
  (non-ASCII letters included); the running program shows the icon in the title bar, the taskbar
  and Alt+Tab; Task Manager shows "RawStride" (from File description) with the icon.
- **Done when:** `.\tools\check.ps1` is all OK (point 6's test passing shows the resource is
  linked) and `git diff --stat` and `git status` show only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Written by the reviewer from ADR 0031 RL2 / RL-Q11; the icon is the owner's choice (2026-11-20). Written while T225 was running (same `build.rs` and `main.rs`); T225 is done (19a23ba), so it is Open. `rc.exe` found at `Windows Kits\10\bin\10.0.26100.0\x64` on the owner's machine (the only version folder with it); the blobs and hashes checked by the reviewer. Cold read (fresh agent, 2026-11-20): can be done with the named files (`VS_FIXEDFILEINFO` and the version functions under `Win32_Storage_FileSystem`, `eframe::icon_data` public with `image`/png). Fixed in the text: `1 VERSIONINFO` instead of `VS_VERSION_INFO` (a macro, no `#include` possible), `#pragma code_page(65001)` and UTF-8 without BOM, the `.ico` and `WindowsSdkDir` rerun lines always printed, the test file's cfg as `build.rs`'s and the string NUL, the icons taken with `git show` in Git Bash into a created `assets` folder (no `src-tauri` folder), `test-inventory.md`'s row, count and totals as point 8, the "Done when" relying on the test, the hand check's release build, icon cache, numeric vs text version and the name's letters |
| 2026-11-20 | Ready for review | Done by the task agent. The icons taken with `git show` in Git Bash, both SHA-256 as given. `build.rs`: `version_resource`, `rc_script`, `rc_text`, `find_rc_exe`, `sdk_version` (std only), the rc script as point 2, `rc.exe` found under the SDK's highest `bin` version, the `.res` linked with `rustc-link-arg-bins`, a `cargo:warning` and no failure without it; T225's part unchanged. `main.rs`: `window_icon()` and `.with_icon`, one unit test. `tests/exe_resources.rs` passes; `(Get-Item rawstride.exe).VersionInfo` shows the copyright with © and the non-ASCII letters of the author's name. `docs/building.md` one sentence, `docs/test-inventory.md` new row, pw-loupe 983 → 984, the header 1967 passed (T225's commit had left it at 1962). check.ps1 all OK: 1967 passed, 72 ignored, no clippy warning |
| 2026-11-20 | Done | Reviewed (reviewer): points 1–8 done as written. Both icons' SHA-256 as given. `build.rs` only under windows/msvc as the manifest, T225's part unchanged; the `.rc` as point 2 (`#pragma code_page(65001)` first, `1 ICON` with doubled backslashes, `1 VERSIONINFO` with numeric constants, the copyright from `CARGO_PKG_AUTHORS` with `:` as `", "`, `"` doubled, no manifest); `rc.exe` from `WindowsSdkDir` or `ProgramFiles(x86)`, the highest numeric `bin` folder with `<arch>c.exe`; the two rerun lines always printed; a `cargo:warning` and no failure without it. `window_icon()` returns `None` on an error, no panic. The test reads strings up to the first NUL and the fixed info unaligned, each `unsafe` with `SAFETY`. Checked by the reviewer: both T226 tests pass with nothing rebuilt (the same binaries as the agent's check.ps1 run); the debug exe's `VersionInfo` shows File/Product version `0.1.0`, "RawStride", and the copyright with © and the author's non-ASCII letters right. Fixed by the reviewer: `test-inventory.md`'s pw-loupe unit row now also describes the icon test (only its count had changed). check.ps1 (task agent's run, not rerun since another agent was working): all OK, 1967 passed, 72 ignored, no clippy warning outside raw-decode-bench. Awaiting the owner's hand check (release build) |
| 2026-11-20 | Hand check OK | The owner, on a release build: Explorer shows the icon; Properties ▸ Details shows File description "RawStride", File version 0.1.0.0, Product name "RawStride", Product version 0.1.0 and the copyright with © and the name spelled right; the running program shows the icon in the title bar, the taskbar and Alt+Tab; Task Manager shows "RawStride" with the icon |
