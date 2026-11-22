# T264. ADR 0007 §9.6 Q19: the DNG image check timed step by step, and which check refuses a damaged copy (pw-imagecheck example) — Open

- **Why:** ADR 0007 revision 29 (§9.6 point 3, Q19; approved by the owner 2026-11-22): the DNG
  image check (§9.3 point 4) is measured before anything about it is decided. T261 found that
  `RawImageCheck::check_copy` is 44–55 % of a DNG edit (250–540 ms), and the analysis
  (`docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T261", hypothesis 2) guesses
  that most of it is LibRaw opening and unpacking the copy, not the preview comparison. The owner
  decides afterwards, in a later ADR revision, whether the check changes. This task only measures;
  it changes no code that the app runs.
- **Crate:** `pw-imagecheck` (an example only). **Files:**
  - `crates/pw-imagecheck/examples/check_timing.rs` (new: points 1–4)
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 5: the results)
  - this task's own file (status and log)
- **What:**
  1. **The program.** `check_timing.rs`, with a doc comment at the top saying what it measures,
     how it is run (point 4), and that it writes only copies in its work folder. Arguments as
     `write_timing.rs` has them: the DNG files to measure, `--runs <n>` (default 3) and
     `--work <folder>` (default a new folder under `std::env::temp_dir()`; a given folder must be
     new or empty, refused otherwise, as `write_timing.rs`'s `refuse_used_folder` does; a work
     folder inside a source file's folder is refused, as its `refuse_source_folders` does).
     ExifTool is started and printed as `write_timing.rs`'s `start_exiftool` does, and only for
     point 3. A file whose extension is not `.dng` (case-insensitive, as `real_files.rs` decides;
     `pw_raw::inspect` gives DNG as `Format::Tiff`, so it cannot tell) is skipped with a line
     saying so.
  2. **The check of an unchanged copy, step by step.** For each DNG and run, the file is copied
     twice into the work folder (an "original" and a "copy"; the source is only read), then each
     timed with `Instant`:
     - `RawImageCheck::without_release().check_copy(original, copy, FileKind::Dng)`: the whole
       check as the app runs it;
     - `pw_libraw::Session::open_with_diagnostics(copy)`: the LibRaw part of it (`check_dng`'s
       path when the copy opens), the session dropped at once;
     - LibRaw's open alone and its unpack alone, through LibRaw's own C API declared in the
       example (`unsafe extern "C"`, as the workspace's edition 2024 requires and
       `crates/pw-libraw/src/lib.rs` writes it: `libraw_init`, `libraw_open_wfile`,
       `libraw_unpack`, `libraw_close`, and `libraw_set_dataerror_handler`; the header
       `%VCPKG_INSTALLED_DIR%\x64-windows-static-md\include\libraw\libraw.h`, the main checkout's
       `vcpkg_installed` per `docs/dev-setup.md`, declares them; read it, do not change it), each
       call's time and return code. `libraw_open_wfile` takes the path as UTF-16 (`const
       wchar_t*`, NUL-terminated; the example converts it with `std::os::windows::ffi::OsStrExt`,
       since `pw-libraw`'s own conversion is private) and exists only on Windows: the LibRaw C
       part is `#[cfg(windows)]`, and on other systems the program prints that it is left out.
       A data-error handler set with `libraw_set_dataerror_handler` counts LibRaw's data errors
       during the unpack; the shim's `damaged` comes from such a handler and from its own data
       stream's short reads (`pwlr.cpp`, `PwStream`, `on_data_error`), not from `unpack`'s return
       code, so the output gives the return code and the data-error count apart. `libraw_init`
       returning null is an error; the handle is closed with `libraw_close` on every path after
       it, a failed open included. The shim (`pwlr_open`) opens through its own data stream and
       settings, so the two parts need not add up to the line above; say so in the output. If
       these symbols do not link from the example, stop and ask: timing inside
       `crates/pw-libraw/csrc/pwlr.cpp` would change the C++ shim and is the owner's decision;
     - the preview comparison alone: `check_copy(original, copy, FileKind::Tiff)`, which for any
       kind other than DNG runs only `check_previews` (say so in the output and the analysis).
     Every `unsafe` block has a `// SAFETY:` comment; the LibRaw handle is closed on every path.
  3. **Which check refuses a damaged copy.** For each DNG, the two damaged copies of
     `crates/pw-imagecheck/tests/real_files.rs` (read it; do not change it): (a) a copy cut at half
     its length, inside its RAW data; (b) a copy with one byte changed (`^= 0x01`) in the middle of
     its largest preview (`pw_raw::inspect(...).previews.last()`). For each, against the unchanged
     original, record whether each of these refuses it, and its time. Point 3 runs once per DNG
     and damage (not `--runs` times). The byte is changed with `get_mut`, not by indexing:
     - the image data hash: `ExifTool::image_data_hash` (T263) of the original and of the
       damaged copy differ, or the call fails; `Ok(None)` on either side is printed as such and
       counts as refused only if the other side is `Some`. The time given is the damaged copy's
       call alone (in the app the original's hash comes from its snapshot);
     - the preview comparison alone (as in point 2);
     - LibRaw's open alone (`libraw_open_wfile` returns an error);
     - LibRaw's unpack (`libraw_unpack` returns an error, or the data-error handler counted any
       error; both given apart) and the shim's verdict
       (`Session::open_with_diagnostics`: an error, and whether it holds `pw_libraw::DAMAGED`);
     - the whole `check_copy(original, damaged, FileKind::Dng)`.
  4. **The output.** Per DNG: name, size in MB, and for each timed step of point 2 every run and
     the median in ms; then the table of point 3 (refused yes/no and the time, per damage and
     check). At the end, two Markdown tables of all files: the medians of point 2, and point 3.
     Run as `cargo run --release -p pw-imagecheck --example check_timing -- <files>`. Afterwards
     the work folder's files are deleted; one that cannot be deleted is named.
  5. **The measurement and the analysis.** Run in a release build on thanos, work folder on C:
     (default), on the seven DNGs of T261: the five `IMG_20261026_*.dng`, `TN002512.DNG` and
     `TN002513.DNG` in `D:\PhotoWorkflowData\RAW`. Add a section "Measured in T264 (Q19: the DNG
     image check)" to the analysis, after "After T263 (Q17)" and before the appendix; the work
     folder is written `%TEMP%`, never the path the program prints (`AGENTS.md` §2): the build
     (commit, `--release`), the
     two tables, the share of LibRaw's open, LibRaw's unpack and the preview comparison in
     `check_copy`, and which checks refuse each damaged copy (in particular: whether the image
     data hash already refuses the cut copy, and whether LibRaw's open alone does). Say whether
     hypothesis 2 of "Measured in T261" holds. Mark each conclusion **inference** or
     **hypothesis**. Propose nothing as decided: what to change is the owner's decision (ADR 0007
     §9.6 point 3).
- **Tests to add:** none (an example; `check.ps1`'s `cargo clippy --workspace --all-targets`
  builds and checks it).
- **Not in this task:**
  - Any change of the image check, the shim, the safe write or `real_files.rs`.
  - The digest pass and the TIFF restart (T265).
  - A cold file cache, and JPEG or TIFF (their check is under 1 ms, T261).
- **Rules:**
  - The program never writes in the source folder; it copies first and writes only in its work
    folder.
  - No new dependency (`Cargo.toml` and `Cargo.lock` unchanged; `examples/` is found by Cargo).
    The LibRaw C API is the one already linked through `pw-libraw`.
  - `AGENTS.md` applies (no `unwrap`/`expect` or unchecked indexing outside tests; do not copy
    `real_files.rs`'s panicking helpers); text in the program is English.
  - `cargo run --release -p pw-imagecheck --example check_timing` is allowed for the
    measurement, as in T261; verification is still `.\tools\check.ps1`.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T264" section,
  and `git status` shows only the files above (the example as a new file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 29 (§9.6 point 3, Q19), approved by the owner 2026-11-22; waits for T263 (the order of §9.6, and point 3 uses `ExifTool::image_data_hash`). T265 follows it (the same analysis) |
| 2026-11-22 | Blocked | T263 done (`ExifTool::image_data_hash` is in `verify.rs`); waits for its cold read (README, "A cold read before Open") |
| 2026-11-22 | Blocked | Cold read: `unsafe extern "C"` (edition 2024); the header path is `%VCPKG_INSTALLED_DIR%` (the main checkout's); `libraw_open_wfile` takes UTF-16 and is Windows-only (`#[cfg(windows)]`); `libraw_set_dataerror_handler` added, since the shim's `damaged` comes from its handler and stream, not from `unpack`'s return code; DNG found by extension only (`pw_raw::inspect` gives `Tiff`); `refuse_source_folders` and `start_exiftool` named; point 3 runs once, `Ok(None)` and which hash call is timed said; `get_mut` for the byte; `%TEMP%` in the analysis; the `-p` run allowed as in T261. No other file needs to change |
| 2026-11-22 | Open | After its cold read |
