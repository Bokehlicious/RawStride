# T261. The safe write's time per step, measured in a release build — Open

- **Why:** the owner finds that writing metadata takes long. The analysis
  `docs/analyses/2026-11-21-metadata-write-time.md` timed the safe write's ExifTool steps with a
  stand-alone script, but not the image check (`RawImageCheck::check_copy`) or the write as
  RawStride runs it. A first look (the ignored test `every_dng_is_written_safely_with_the_real_check`
  in `crates/pw-imagecheck/tests/real_files.rs`, dev profile, 2026-11-21) gave 260–570 ms of image
  check per DNG, as much as all of its ExifTool steps. The owner decided (2026-11-21) to measure
  what is missing before any change of ADR 0007 §9 is decided. This task only measures; it changes
  no code that the app runs.
- **Crates:** `pw-imagecheck` (an example only). **Files:**
  - `crates/pw-imagecheck/examples/write_timing.rs` (new: points 1–4)
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 5: the results)
  - this task's own file (status and log)
- **What:**
  1. **The program.** `write_timing.rs`, with a doc comment at the top saying what it measures,
     how it is run (point 4), that it writes only copies, and that it writes only on Windows
     (`fileops` refuses elsewhere; it still compiles everywhere). It takes the image files to
     measure as arguments, an optional `--runs <n>` (default 3) and an optional `--work <folder>`
     (default: a new folder under `std::env::temp_dir()`). ExifTool is started as in
     `real_files.rs` (`PW_EXIFTOOL_PL` through `perl`, otherwise `ExifTool::start()`), and its
     start time (spawn and the answer to `execute(&["-ver"])`) is printed once. The journal is
     `Journal::open` on a folder inside the work folder.
  2. **One run of one file.** The image is copied into the work folder (the source is only read).
     For a manufacturer RAW (`read.state.kind == FileKind::Raw`) its `<basename>.xmp` next to it,
     found without regard to case, is copied too when there is one ("RAW, sidecar exists"); each
     RAW file is also run once more without the sidecar copied ("RAW, new sidecar"), and the two
     are reported as separate rows. Then, each timed with `Instant`:
     - `ExifTool::read_metadata(&copy, Fallback::default())` (`pw_metadata::iim::Fallback`), the
       read every edit makes before its write;
     - `ExifTool::snapshot` of the file the write targets, alone: the image copy for a JPEG, TIFF
       or DNG; the copied sidecar for "RAW, sidecar exists"; none for "RAW, new sidecar" (the
       safe write takes no snapshot of an original there);
     - `ExifTool::write_safely` of the copy with the plan of point 3 and a timing wrapper around
       `RawImageCheck::without_release()`, like `Timed` in `real_files.rs` but without `unwrap`
       or `expect` (`AGENTS.md`): `&dyn ImageCheck` needs no `Sync`, so a `Cell<Option<Duration>>`
       does;
     - the time of `write_safely` without `check_copy` (the difference). `check_copy` is never
       called for a sidecar, so for RAW rows it is 0 and the write's time is all ExifTool and
       files. The write's time includes, as the app's does, the snapshot of the original, the
       journal, the digest pass and, for a TIFF, the ExifTool restart.
     After the run the work folder's copies are deleted: the image, its `.xmp` (copied or made by
     the write) and any `pwtmp`/`pwbak` file. A failed write is printed with its error, the run is
     counted as failed, and the program goes on with the next.
  3. **The plan.** The copy's `read_metadata` result's `reconciled.metadata` with
     `photoshop:City` set to `Value::Text("Malmö")` by `Metadata::set` (which marks it changed even
     when the file already holds that value, so the plan is never empty), then
     `write_plan::plan(&metadata, &read.state, WriteSettings::default())`. This is
     `ExifTool::write_edits_safely`'s plan (`edit.rs`) for one City edit, without its checks for
     unsupported files, left-alone entries and qualifiers, which do not change a City-only plan.
  4. **The output.** Per row (file and, for a RAW, case): name, size in MB, file kind
     (`read.state.kind`), and for each step the time of every run and their median, in ms; at the
     end one table (Markdown) of the medians, one row per row. Run as
     `cargo run --release -p pw-imagecheck --example write_timing -- <files>`.
  5. **The measurement and the analysis.** Run in a release build on thanos with
     `D:\PhotoWorkflowData\RAW` as the source and the work folder on C: (default), and once with
     `--work` on a new folder on D:, on: `TN006331.JPG`, `TN005701_DxO.jpg`; the five
     `IMG_20261026_*.dng` and `TN002512.DNG`, `TN002513.DNG`; `TN002683.tif`, `TN004596.tif`,
     `TN006025.tif`; `P1240231.RW2`, `P1240233.RW2`. Add a section "Measured in T261" to the
     analysis: the build (commit, `--release`), the medians table, the share of `check_copy` in the
     total for each kind, how C: and D: differ, and whether the script's figures (the tables above
     it) hold. Mark each conclusion **inference** or **hypothesis** as the analysis does.
- **Tests to add:** none (an example; `check.ps1`'s `cargo clippy --workspace --all-targets`
  builds and checks it).
- **Not in this task:**
  - Any change of the safe write, the image check or the worker. What to change is the owner's
    decision after this task (ADR 0007 §9).
  - The shell's work after a write (the editor's and the info window's new read, the view and the
    thumbnails made again): a later task, in `pw-loupe`, if the owner wants it measured.
  - A cold file cache.
- **Rules:**
  - The program never writes in the source folder; it copies first and writes only the copies in
    the work folder, and deletes them as point 2 says. If a file cannot be deleted, it says which.
  - No new dependency (`Cargo.toml` and `Cargo.lock` unchanged; `examples/` is found by Cargo).
  - `AGENTS.md` applies (no `unwrap`/`expect` outside tests); text in the program is English.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T261" section,
  and `git status` shows only the files above (the example as a new file).

## Log

| Date | Who | What |
|---|---|---|
| 2026-11-21 | reviewer | Written after the owner's decision to measure what is missing first. |
| 2026-11-21 | reviewer | Cold read: every named API is public and reachable from the example with `pw-imagecheck`'s dependencies; no `Cargo.toml` change. Changed: the RAW cases (sidecar exists / new sidecar, the snapshot of the sidecar, `check_copy` 0, the `.xmp` deleted), the City value (one value; `Metadata::set` always marks it changed), the plan named as a simplification of `write_edits_safely`, `Fallback`'s path, no `unwrap` in the wrapper, the restart inside the write's time, the files named, "Done when" by `git status`. |
