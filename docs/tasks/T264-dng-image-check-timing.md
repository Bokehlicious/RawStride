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
     new or empty, refused otherwise, as `write_timing.rs`'s `refuse_used_folder` does). ExifTool
     is started as in `write_timing.rs`, and only for point 3. A file that is not a DNG
     (`pw_raw::inspect` or the extension) is skipped with a line saying so.
  2. **The check of an unchanged copy, step by step.** For each DNG and run, the file is copied
     twice into the work folder (an "original" and a "copy"; the source is only read), then each
     timed with `Instant`:
     - `RawImageCheck::without_release().check_copy(original, copy, FileKind::Dng)`: the whole
       check as the app runs it;
     - `pw_libraw::Session::open_with_diagnostics(copy)`: the LibRaw part of it (`check_dng`'s
       path when the copy opens), the session dropped at once;
     - LibRaw's open alone and its unpack alone, through LibRaw's own C API declared in the
       example (`extern "C"`: `libraw_init`, `libraw_open_wfile`, `libraw_unpack`, `libraw_close`;
       `vcpkg_installed/x64-windows-static-md/include/libraw/libraw.h` declares them), each call's
       time and return code. The shim (`pwlr_open`) opens through its own data stream and settings,
       so the two parts need not add up to the line above; say so in the output. If these symbols
       do not link from the example, stop and ask: timing inside `crates/pw-libraw/csrc/pwlr.cpp`
       would change the C++ shim and is the owner's decision;
     - the preview comparison alone: `check_copy(original, copy, FileKind::Tiff)`, which for any
       kind other than DNG runs only `check_previews` (say so in the output and the analysis).
     Every `unsafe` block has a `// SAFETY:` comment; the LibRaw handle is closed on every path.
  3. **Which check refuses a damaged copy.** For each DNG, the two damaged copies of
     `crates/pw-imagecheck/tests/real_files.rs` (read it; do not change it): (a) a copy cut at half
     its length, inside its RAW data; (b) a copy with one byte changed (`^= 0x01`) in the middle of
     its largest preview (`pw_raw::inspect(...).previews.last()`). For each, against the unchanged
     original, record whether each of these refuses it, and its time:
     - the image data hash: `ExifTool::image_data_hash` (T263) of the original and of the
       damaged copy differ, or the call fails;
     - the preview comparison alone (as in point 2);
     - LibRaw's open alone (`libraw_open_wfile` returns an error);
     - LibRaw's unpack (`libraw_unpack` returns an error) and the shim's verdict
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
     image check)" to the analysis, after "After T263 (Q17)": the build (commit, `--release`), the
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
  - `AGENTS.md` applies (no `unwrap`/`expect` outside tests); text in the program is English.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T264" section,
  and `git status` shows only the files above (the example as a new file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 29 (§9.6 point 3, Q19), approved by the owner 2026-11-22; waits for T263 (the order of §9.6, and point 3 uses `ExifTool::image_data_hash`). T265 follows it (the same analysis) |
| 2026-11-22 | Open | T263 done (`ExifTool::image_data_hash` is in `verify.rs`) |
