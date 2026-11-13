# T69. ADR 0016 C2: RawStride reads the monitor profile at start and names it in the window title — Done

- **Can start now.** T68 (`pw-colour`) accepted. The passthrough check of ADR 0016 §8.2 is done
  (§1.1). T69, T70 and T71 do not depend on each other.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/Cargo.toml`; `crates/pw-loupe/src/lib.rs`; a new module
  `crates/pw-loupe/src/monitor_profile.rs`; `crates/pw-loupe/src/main.rs`; `Cargo.lock` (by cargo);
  `docs/test-inventory.md`. This task may change `crates/pw-loupe/Cargo.toml` as written below and
  nothing else in it.
- **Why:** ADR 0016 §4.1 ("Reading the monitor profile"), §5 (window title), Q4 (read at start
  only), §6 (`windows-sys` features approved in C0).
- **What:**
  1. **Dependencies** in `crates/pw-loupe/Cargo.toml`: `pw-colour.workspace = true` under
     `[dependencies]`, and a new section
     `[target.'cfg(windows)'.dependencies]` with
     `windows-sys = { version = "=0.61.2", features = ["Win32_Foundation", "Win32_Graphics_Gdi", "Win32_UI_ColorSystem"] }`.
  2. **`monitor_profile.rs`** (library module, exported from `lib.rs`):
     - `pub enum ProfileSource { File(String), Missing, Unusable(String) }` (`Debug`, `Clone`,
       `PartialEq`, `Eq`): `File` holds the profile's **file name** (not the path); `Unusable` holds
       the detail (English, for a later log; not shown).
     - `pub struct MonitorProfile { pub profile: pw_colour::Profile, pub source: ProfileSource }`
       (`Debug`, `Clone`).
     - `pub fn load_monitor_profile(path: Result<Option<PathBuf>, String>) -> MonitorProfile`: the
       testable part. `Ok(Some(path))`: read the file and `Profile::from_icc`; success is `File(name)`
       with that profile. `Ok(None)` (no profile associated) is `Missing`. A read error, a parse
       error, an `Unsupported` profile or `Err(detail)` is `Unusable(detail)`. Every case except
       `File` uses `Profile::srgb()`, so the pixels stay as today (F3). Never a panic.
     - `pub fn primary_monitor_profile_path() -> Result<Option<PathBuf>, String>`: on Windows, the
       sequence the reviewer checked on the owner's machine (ADR 0016 §4.1): `MonitorFromPoint`
       (point 0,0, `MONITOR_DEFAULTTOPRIMARY`), `GetMonitorInfoW` with `MONITORINFOEXW`,
       `CreateDCW(device, device, null, null)` with its `szDevice`, `GetICMProfileW` twice (first
       for the length, then into a buffer of that length), `DeleteDC` on every path after a
       successful `CreateDCW`. A failing call gives `Err` with the call's name; `GetICMProfileW`
       reporting no profile gives `Ok(None)`. Each `unsafe` block gets a `// SAFETY:` comment, as in
       `pw-libjpeg`. On other platforms it returns `Ok(None)`.
     - `pub fn window_title(source: &ProfileSource) -> String`, exactly (ADR 0016 §5, two spaces
       around the dot as in the status line):
       - `File(name)`: "RawStride  ·  Skärmprofil: {name}"
       - `Missing`: "RawStride  ·  Ingen skärmprofil – visas som sRGB"
       - `Unusable(_)`: "RawStride  ·  Skärmprofilen kunde inte användas – visas som sRGB"
  3. **`main.rs`, `run`:** call `load_monitor_profile(primary_monitor_profile_path())` **before**
     `ViewCore::start`, give the viewport `.with_title(window_title(&monitor.source))` instead of
     `WINDOW_TITLE` (the first argument of `run_native`, the app name, stays `WINDOW_TITLE`), and keep
     the `MonitorProfile` in a new `Loupe` field `monitor_profile` for C4. It is not used for pixels
     in this task. If the compiler warns that the field is never read, mark that one field
     `#[allow(dead_code)]` with the comment "used by ADR 0016 C4".
- **Rules:** rules 3–7 and 10–12. Check with `git diff Cargo.lock`: the only change is `pw-colour`
  and `windows-sys` in `pw-loupe`'s dependency list; **no new package**. If cargo wants anything
  else, stop and report.
- **Tests to add** (in `pw-loupe`'s library; files in `std::env::temp_dir()` with the process id in
  the name, removed afterwards, as the existing tests do; an ICC file is made with
  `moxcms::ColorProfile::new_adobe_rgb().encode()` — add `moxcms = "=0.8.1"` under
  `[dev-dependencies]` for that, the only other `Cargo.toml` change allowed):
  - an Adobe RGB ICC file gives `File("<its file name>")`, and its profile converts sRGB
    (255,0,0) to within 1 of (219,0,0) (the same check as T68, through `pw_colour::Transform`);
  - `Ok(None)` gives `Missing` with an sRGB profile: an sRGB → that profile transform is
    `is_identity()`;
  - a path that does not exist, a file of garbage bytes, a gray profile
    (`ColorProfile::new_gray_with_gamma(2.2).encode()`) and `Err("x")` each give `Unusable` with an
    sRGB profile (identity as above);
  - `window_title` gives the three texts exactly;
  - `primary_monitor_profile_path()` returns without panicking (no assertion on the value: it
    depends on the machine).
- **Check by hand (owner, after review):** start RawStride; the title says
  "RawStride  ·  Skärmprofil: CX240(…)08CAL (DP).icc"; the images look exactly as before
  (no conversion yet).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has the new tests in the
  `pw-loupe` row and the new totals; `git diff --stat` shows only the files above.
