# T267. ADR 0007 §9.6 Q18: a batch write timed with 1, 2 and 4 ExifTool processes (pw-imagecheck example) — Done

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
     refused otherwise, as `write_timing.rs`'s `refuse_used_folder` does, and one inside a source
     file's folder is refused, as its `refuse_source_folders` does). The worker is started with
     `MetadataWorker::start_with("perl", &[<pl>], ..)` when `PW_EXIFTOOL_PL` is set, otherwise
     with `MetadataWorker::start(..)` (the program folder's `exiftool.exe`, else `exiftool` on
     PATH; the program path itself is not public, so the source is printed as a text, as
     `write_timing.rs`'s `start_exiftool` does). The version is taken from the first worker's
     `exiftool_version()` and printed once; an `Err` or a version below `MIN_EXIFTOOL_VERSION`
     stops the program with that text.
  2. **One run.** For each number of processes `p` and each run: the batch is copied fresh into
     the work folder (names made unique per source and copy, `<stem>-<s>-<i>.<ext>` with `s` the
     source's index, so two sources with one stem never share a pool key; the sources are only
     read). A new `MetadataWorker` is started as in point 1 with a `WriteSetup` of a journal
     opened anew in the work folder's `journal` folder, `Arc::new(RawImageCheck::without_release())`
     as the check, the default `EditOptions` and `write_processes: p`. The program waits until
     `exiftool_version()` is known (the start is not timed), then takes the time and sends one
     `write_edits` per file with one `Edit::Set { property: "photoshop:City".into(), field: None,
     value: Value::Text("Malmö".into()) }`. The worker blocks when its event channel (8 events)
     is full, so while sending the program also takes every event with `try_recv()`; after
     `QueueFull` it takes events and sends again. With nothing to send and no event it yields
     with a sleep of 1 ms (a constant, named in the doc comment; it adds at most about 1 ms to a
     run's time). The time stops when every file has answered with `WorkerEvent::EditsWritten`.
     A failed write is printed with its file and error and counted; an `Ok` whose report is
     `WriteReport::Unchanged` (`EditReport.report`) is counted apart as "unchanged" (it would make the batch look
     faster) and printed.
     **The pool's start:** with `p > 1` the pool's processes start when a thread first needs one
     (T266), so the first batch of a worker includes up to `p` ExifTool starts. Each run therefore
     writes the batch twice on the same worker, fresh copies each time: the first batch timed as
     "first (with starts)", the second as "warm". Both are reported; the speed-up is given for
     both.
     Afterwards the worker is dropped, the program waits until no ExifTool process of the worker
     can still be ending (at least 2 s, `ExifTool`'s drop waits at most that long) before the next run,
     and the batch's files (and any `.xmp`, `pwtmp`, `pwbak`) and the journal folder are deleted.
     At the end the work folder is deleted when it was the default one, as `write_timing.rs`
     does.
  3. **What is reported per run:** `p`, the run, the batch (first or warm), the number of files,
     failures, unchanged, the wall time in ms, and files per second.
  4. **The output.** Every run as it ends, then one Markdown table: per `p` and batch (first,
     warm), the median wall time, files per second, and the speed-up against `p = 1`. Run as
     `cargo run --release -p pw-imagecheck --example batch_timing -- <files>`.
  5. **The measurement and the analysis.** Run in a release build on thanos with the camera JPEGs
     `TN006331.JPG`, `TN005701_DxO.jpg` and the seven DNGs of T261 (the five
     `IMG_20261026_*.dng`, `TN002512.DNG`, `TN002513.DNG`) from `D:\PhotoWorkflowData\RAW`,
     `--copies 3` (27 files), once with the work folder on C: (default) and once with `--work` on
     a new folder on D:. Also once with JPEGs only and once with DNGs only on C:, so the two kinds
     can be told apart. Add a section "Measured in T267 (Q18: 1, 2 and 4 processes)" to the
     analysis, after "Measured in T265" and before the appendix (the work folder written
     `%TEMP%`, never the path the program prints, `AGENTS.md` §2): the build (commit,
     `--release`), the machine (thanos, i7-9700K: 8 cores, no SMT), the tables, and the speed-up
     per kind, drive and batch (first, warm). Mark each conclusion **inference** or
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
  - `AGENTS.md` applies (no `unwrap`/`expect` or unchecked indexing outside tests); text in the
    program is English.
  - `cargo run --release -p pw-imagecheck --example batch_timing` is allowed for the measurement,
    as in T261; verification is still `.\tools\check.ps1`.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "Measured in T267" section,
  and `git status` shows only the files above (the example as a new file).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written with T266 and T268 (ADR 0007 §9.6 point 2, Q18); waits for T266 |
| 2026-11-22 | Blocked | Cold read: the worker started with `MetadataWorker::start` or `start_with("perl", ..)` (the default program is not public), the version from `exiftool_version()` printed once, a too-old or unknown version stops; events drained with `try_recv()` while sending (the 8-event channel blocks the worker), a 1 ms sleep named; the pool's lazy start reported apart (a first and a warm batch per run on the same worker); `Unchanged` counted apart; the exact `Edit::Set`; names `<stem>-<s>-<i>` (one pool key per file); the journal opened per run, old ExifTool processes waited for, the journal and default work folder deleted; the section before the appendix, `%TEMP%`, the machine's cores; `refuse_source_folders`; the `-p` run allowed as in T261. Follows T266's API as its cold read named it |
| 2026-11-22 | Open | T266 done |
| 2026-11-22 | Ready for review | `batch_timing.rs` added (points 1–4; `refuse_source_folders` also refuses a work folder inside a source's folder, not only the folder itself; the wait after a dropped worker is 3 s; the D: work folder was made new by the measuring script and removed after it). Measured at `88fa430` with the example uncommitted, ExifTool 13.59, thanos: 216 of 216 batches wrote every file, no failure, no unchanged. Warm speed-up against 1 process: 27 files on C: 1.98× (2) and 3.55× (4), on D: 1.96× and 3.46×; JPEGs only (6 files) 1.87× and 2.91×; DNGs only (21) 1.93× and 3.31×; the first batch 0.04–0.51× lower. "Measured in T267" added to the analysis. The example was reformatted by `rustfmt` after the measurement (formatting only). check.ps1 all OK (2276 passed, 77 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewed and approved by the owner 2026-11-22: points 1–5 as written (the work folder also refused inside a source's folder, and the 3 s wait, are both stricter than asked); `AGENTS.md` §3 kept; `test-output.txt` newer than the example, all OK, no clippy warnings. Every figure in the analysis recomputed from its table. The reviewer re-ran the mixed batch on C: once (13.59): warm 1.95× (2) and 3.52× (4), first 1.91× and 3.30×, within about 2 % of the medians; 27 of 27 written, nothing left behind. Minor, kept: `--work` is made before the source-folder check, so a folder refused there is left empty (as `write_timing.rs`). **The owner, 2026-11-22:** the number is four, recorded in ADR 0007 §9.6 point 2, §14 (Q18) and §18; T268 makes the app use it. No hand check (a measurement) |
