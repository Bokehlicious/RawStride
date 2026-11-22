# T266. ADR 0007 §9.6 Q18: a pool of ExifTool write processes, used by the metadata worker (pw-metadata) — Blocked: waits for T265

- **Why:** ADR 0007 revision 29 (§9.6 point 2, Q18; approved by the owner 2026-11-22). All
  writes go through one ExifTool process in one worker thread
  (`crates/pw-metadata/src/worker.rs`), so a batch (a rating, an edit or a rotation applied to
  several images) costs the sum of its files; the import's template (`pw-loupe`'s
  `import_worker.rs`) writes through its own single process the same way. Q18: writes are shared
  among up to **four** ExifTool processes, each in its own thread, with every file still its own
  safe-write transaction. This task builds the pool of write processes in `pw-metadata`, usable
  by the metadata worker now and by the import worker in T268, and makes the metadata worker use
  it, with the number of processes as a parameter. With **one** process the worker runs exactly as
  today, and the app keeps one until the owner has fixed the number after T267's measurement
  (T268 sets it). First task of the chain T266 → T267 → T268.
- **Crate:** `pw-metadata`, and one call site in `pw-loupe`. **Files:**
  - `crates/pw-metadata/src/write_pool.rs` (new: point 2, and its tests)
  - `crates/pw-metadata/src/lib.rs` (the module declared public; nothing else)
  - `crates/pw-metadata/src/worker.rs` (points 1, 3–5, and its tests)
  - `crates/pw-metadata/src/journal.rs` (point 6, a test only, unless the test finds a fault)
  - `crates/pw-metadata/src/test_support.rs` (only if a new fake ExifTool mode is needed)
  - `crates/pw-loupe/src/main.rs` (point 7: the `WriteSetup` it builds; no other change)
  - `docs/test-inventory.md` (the new tests, rule 12)
  - this task's own file (status and log)
- **What:**
  1. **The number.** `WriteSetup` gets `write_processes: usize`, clamped to 1..=4 when the worker
     starts (0 counts as 1). Its doc comment says what it is (ADR 0007 §9.6 point 2).
  2. **The pool** (`write_pool.rs`, public, with a module comment citing §9.6 point 2). Started
     with the ExifTool program and leading arguments and a number `n` (2..=4; the callers use no
     pool for 1). It owns `n` threads, each owning its own ExifTool process, started when the
     thread first needs it, its version asked and writes allowed or not as `worker.rs`'s
     `start_tool` decides today (`MIN_EXIFTOOL_VERSION`); a process that breaks
     (`ExifTool::is_broken`) is dropped and the thread's next job starts a new one. What it gives:
     - **a job** is a closure run with the thread's `&mut ExifTool`, given with a **key**; its
       answer comes back with the job's ticket as either the closure's result, or, when the
       closure was not run, why: the process could not be started (the error's text), or its
       ExifTool is too old or its version unknown (D9);
     - **the key** (a public function of the pool's module, for example `write_key(image)`): the
       image's folder and file stem, the stem compared without regard to case, so `a.CR2` and
       `a.NEF` in one folder (which may share a sidecar) have one key. Two jobs with one key never
       run at the same time and run in the order given; a job is held back while an earlier one
       with its key has not answered;
     - **outstanding work**: whether any job given has not answered yet;
     - **discard**: the jobs given but not started are dropped and answered as discarded (T268's
       cancel at import); jobs that started finish;
     - **the end**: when the pool is dropped, jobs that started finish and every thread is joined;
       a job is never cut off mid-transaction. Jobs not started are answered as discarded.
     - The first version a pool process reports is not needed by the pool's callers in this task;
       the worker keeps its own first version (point 3).
  3. **The worker with one process is today's worker.** With `write_processes == 1` there is no
     pool: one thread, one ExifTool process, every request in order, unchanged.
  4. **The worker with several processes.** With `write_processes == n > 1`:
     - the worker thread keeps its own process for every request that is not a write (the ratings
       read, `read_metadata`, moves, deletes, `prepare_delete`), as today; `exiftool_version`
       stays the first start of that process;
     - the writes (`write_rating`, `write_orientation`, `write_edits`) are given to a pool of `n`
       with the key of their image. Each job runs the same safe-write call as `handle` does today,
       with the same `WriteSetup` (journal, checks) and the options that were current when its
       request was taken from the queue (`set_options` keeps its meaning: requests queued after it
       use the new options). Its answer becomes the same `Event` as today; a "too old" answer is
       `WriteError::ExifToolTooOld`, a start failure `WriteError::Other` with its text, as today;
     - **everything else waits for earlier writes:** a request that is not a write starts only when
       the pool has no outstanding work, so a move, a delete or a read sent after a write of the
       same image still runs after it, as the module comment promises today;
     - the worker thread keeps taking requests from the queue while writes run (so the shell,
       which fills the queue until `QueueFull`, keeps up to `n` writes busy), and sends each
       write's event when it answers; a later write may answer before an earlier one of another
       image. Every event already names its request id and file; the module comment says that the
       answers of writes may come in another order than they were asked when `n > 1`;
     - the TIFF restart (§9.3) is already per process (`ExifTool::restart` on the process that
       wrote); nothing restarts another thread's process;
     - every accepted request is run, as today; when the `MetadataWorker` is dropped, the writes
       already given finish before the worker thread ends.
  5. **The image check is shared.** `WriteSetup::checks` becomes
     `Arc<dyn ImageCheck + Send + Sync>` (today `Box<dyn ImageCheck + Send>`), so the pool's
     threads share it. Check that `RawImageCheck` and the shell's check (`image_check` and
     `release_before_write` in `crates/pw-loupe/src/main.rs`: the view cores' releasers and
     `HistogramReads`, whose `note_before_write` notes a file's stamp per path) are `Sync` and that
     releases of two files from two threads at once are safe; if one is not, stop and say which,
     since that is a change in another crate.
  6. **The journal.** A test in `journal.rs` that entries begun from several threads at once all
     get distinct files and are all ended, and that `recover` with several open entries resolves
     each (§9.5: the journal holds several open entries, each written whole). If it fails, stop and
     explain before changing `journal.rs`.
  7. **The app.** `crates/pw-loupe/src/main.rs` builds its `WriteSetup` with `write_processes: 1`
     and the check as an `Arc`; nothing else in `pw-loupe` changes. The app behaves as before.
- **Tests to add** (with the fake ExifTool of `src/test_support.rs` where a real one is not
  needed; a new fake mode, for example a write that answers only after a short wait or a signal,
  is added there and named in the report):
  - `write_pool.rs`: jobs with different keys overlap (a slow job does not keep a quick one of
    another key from answering first); two jobs with one key, and with `a.CR2` and `a.NEF` in one
    folder, never overlap and answer in the order given; a process that breaks is replaced and the
    other threads go on; a process too old answers "too old" without running the closure; discard
    drops only jobs not started; dropping the pool lets started jobs finish and joins every
    thread; `write_key` for the same stem in different case, different stems, different folders;
  - `worker.rs`: with 1 process, the existing worker tests pass unchanged (none is rewritten);
    with 2 and with 4 processes, writes of different images overlap and may answer out of order;
    two writes of one image answer in order; a move and a `read_metadata` of an image asked after
    its write run after the write has answered; `set_options` between two writes: the first uses
    the old options, the second the new; dropping the worker while writes run: they finish, and
    no thread is left;
  - `worker.rs` with a real ExifTool (as the file's real-ExifTool tests start it): at least eight
    copies of the small test JPEG written with 4 processes, every file written, its value read
    back, and the journal folder empty afterwards;
  - `journal.rs`: point 6.
- **Not in this task:**
  - The import worker's template and the number the app uses (T268); the measurement with 1, 2
    and 4 processes (T267).
  - Any change of the safe write itself (`safe_write.rs`, `verify.rs`) or of the shell's handling
    of answers.
  - A setting for the number (§9.6 point 2: "may become a setting later").
- **Rules:**
  - Every file is its own transaction with every check of ADR 0007 §9.1–§9.5; nothing in a
    transaction is shared between threads except the journal folder and the image check.
  - The RAW engine's sessions of an image are released before its write (§9.1), as today: by the
    safe write's own call of `ImageCheck::release`.
  - `AGENTS.md` §3 (no `unwrap`, `expect`, `panic!` or unchecked indexing outside tests; a
    poisoned lock or a failed thread start is an error, not a panic). No new dependency.
  - An extra code review (`docs/tasks/README.md`: the safe write's worker, more than one crate).
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 29 (§9.6 point 2, Q18), approved by the owner 2026-11-22; Q18 comes after Q19's measurement (§9.6 order), so it waits for T265. Chain T266 → T267 → T268. Before it is set to Open: a cold read of each of the three, and a review of the whole chain (`docs/tasks/README.md`: the metadata worker and the shell, several writes at once) |
