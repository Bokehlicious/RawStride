# T267. ADR 0007 §9.6 Q18: a batch write timed with 1, 2 and 4 ExifTool processes (pw-imagecheck example) — Blocked: waits for T266

- **Why:** ADR 0007 §9.6 point 2 (Q18; approved by the owner 2026-11-22): the number of ExifTool
  processes a batch is shared among (at most four) "is fixed after a measurement with 1, 2 and 4
  processes on a batch of camera JPEGs and DNGs". T266 gave the metadata worker the number as
  `WriteSetup::write_processes`. This task measures; the owner then fixes the number, and T268
  makes the app use it. It changes no code that the app runs.
- **Crate:** `pw-imagecheck` (an example only). **Files:**
  - `crates/pw-imagecheck/examples/batch_timing.rs` (new: points 1–4)
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 5: the results)
  - this task's own file (status and log)
- **What:**
  1. **The program.** `batch_timing.rs`, with a doc comment at the top saying what it measures,
     how it is run (point 4), and that it writes only copies in its work folder. Arguments: the
     source image files, `--copies <k>` (default 3: each source is copied `k` times into the
     batch), `--processes <list>` (default `1,2,4`), `--runs <n>` (default 3) and `--work <folder>`
     (default a new folder under `std::env::temp_dir()`; a given folder must be new or empty,
     refused otherwise, as `write_timing.rs` does). The ExifTool program and leading arguments are
     found as in `write_timing.rs` (`PW_EXIFTOOL_PL` through `perl`, otherwise the program
     `ExifTool::start` uses) and printed once with its version.
  2. **One run.** For each number of processes `p` and each run: the batch is copied fresh into
     the work folder (names made unique, for example `<stem>-<i>.<ext>`; the sources are only
     read). A new `MetadataWorker` is started with `MetadataWorker::start_with` and a
     `WriteSetup` of a journal in the work folder, `RawImageCheck::without_release()` as the
     check, the default `EditOptions` and `write_processes: p`. The program waits until
     `exiftool_version()` is known (the start is not timed), then takes the time, sends one
     `write_edits` per file with one `Edit::Set` of `photoshop:City` to `"Malmö"` (resending after
     `QueueFull`, as the shell does), and stops the time when every file has answered with
     `WorkerEvent::EditsWritten`. A failed write is printed with its file and error and counted.
     The worker is dropped, and the batch's files (and any `.xmp`, `pwtmp`, `pwbak`) are deleted
     before the next run. The time includes starting the extra write processes, as an app's first
     batch does; say so in the output.
  3. **What is reported per run:** `p`, the run, the number of files, failures, the wall time in
     ms, and files per second.
  4. **The output.** Every run as it ends, then one Markdown table: per `p`, the median wall time,
     files per second, and the speed-up against `p = 1`. Run as
     `cargo run --release -p pw-imagecheck --example batch_timing -- <files>`.
  5. **The measurement and the analysis.** Run in a release build on thanos with the camera JPEGs
     `TN006331.JPG`, `TN005701_DxO.jpg` and the seven DNGs of T261 (the five
     `IMG_20261026_*.dng`, `TN002512.DNG`, `TN002513.DNG`) from `D:\PhotoWorkflowData\RAW`,
     `--copies 3` (27 files), once with the work folder on C: (default) and once with `--work` on
     a new folder on D:. Also once with JPEGs only and once with DNGs only on C:, so the two kinds
     can be told apart. Add a section "Measured in T267 (Q18: 1, 2 and 4 processes)" to the
     analysis, after "Measured in T265": the build (commit, `--release`), the machine (cores), the
     tables, and the speed-up per kind and drive. Mark each conclusion **inference** or
     **hypothesis**. Propose nothing as decided: the number is the owner's decision.
- **Tests to add:** none (an example; `check.ps1`'s `cargo clippy --workspace --all-targets`
  builds and checks it).
- **Not in this task:**
  - Any change of the worker, the safe write or the app; the number the app uses (T268, after the
    owner's decision).
  - A memory card or a network share (§9.6 point 2 expects less gain there; the owner may ask for
    it after reading the results). A cold cache.
- **Rules:**
  - The program never writes in the source folder; it copies first and writes only in its work
    folder. If a file cannot be deleted, it says which.
  - No new dependency (`Cargo.toml` and `Cargo.lock` unchanged).
  - `AGENTS.md` applies (no `unwrap`/`expect` outside tests); text in the program is English.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T267" section,
  and `git status` shows only the files above (the example as a new file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written with T266 and T268 (ADR 0007 §9.6 point 2, Q18); waits for T266 |
