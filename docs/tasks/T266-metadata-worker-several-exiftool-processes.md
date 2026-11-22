# T266. ADR 0007 §9.6 Q18: a pool of ExifTool write processes, used by the metadata worker (pw-metadata) — Open

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
  - `crates/pw-metadata/src/test_support.rs` (the slow fake mode of the tests)
  - `crates/pw-loupe/src/main.rs` (point 7: the `WriteSetup` it builds; no other change)
  - `crates/pw-loupe/src/metadata.rs` (point 7: its test helper `test_setup` only)
  - `docs/test-inventory.md` (the new tests, rule 12)
  - this task's own file (status and log)
- **What:**
  1. **The number.** `WriteSetup` gets `write_processes: usize`, clamped to 1..=4 when the worker
     starts (0 counts as 1). Its doc comment says what it is (ADR 0007 §9.6 point 2).
  2. **The pool** (`write_pool.rs`, public, with a module comment citing §9.6 point 2). These
     names are the API T267 and T268 rely on; the agent may change a name only if it says so in
     the report, so the reviewer can follow it in T267 and T268:
     - **`WritePool<T: Send + 'static>`**, started with **`WritePool::start(n)`** (the ExifTool
       of `exiftool_program`, as `MetadataWorker::start`; so `pw-loupe`, which cannot name that
       program, can start a pool) or **`WritePool::start_with(program, pre_args, n)`**. `n` is
       clamped to 2..=4 (the callers use no pool for 1). Both return `Result`: a thread that
       cannot be spawned is an error (the threads already spawned are ended), never a panic.
     - It owns `n` threads, each owning its own ExifTool process, started **when the thread first
       needs it** (so with `n > 1` there are up to `n + 1` processes with the worker's own, and a
       thread's first job pays for an ExifTool start; T267 reports it). Starting it, asking its
       version and allowing writes or not is `worker.rs`'s `start_tool`, moved to `write_pool.rs`
       as `pub(crate)` with the first-version slot made optional, and used by both (not
       duplicated). A process that breaks (`ExifTool::is_broken`) is dropped and the thread's next
       job starts a new one.
     - **A job:** `submit(key: WriteKey, job: Box<dyn FnOnce(&mut ExifTool) -> T + Send>) ->
       Ticket` (`Ticket` a copyable number, unique per pool). Its **answer** is a
       `JobAnswer<T>`: `Done(T)`; `StartFailed(String)` (the process could not be started, with
       the error's text); `TooOld(Result<ToolVersion, String>)` (its ExifTool is too old, with the
       version, or its version is unknown, with the version error's text; D9); `Discarded` (point
       "discard"); `Lost(String)` (the job panicked: it is run under `catch_unwind`, its process is
       dropped as broken, and the thread goes on, so no answer is ever missing).
     - **Answers** come back on an unbounded channel inside the pool, so a pool thread never
       blocks on giving one: `try_answer() -> Option<(Ticket, JobAnswer<T>)>` (non-blocking) and
       `answer_timeout(Duration) -> Option<(Ticket, JobAnswer<T>)>`.
     - **The key:** `pub fn write_key(image: &Path) -> WriteKey`: the image's folder as given
       (compared as is) and its file stem lowercased, so `a.CR2` and `a.NEF` in one folder (which
       may share a sidecar, `sidecar_path`) have one key. Two jobs with one key never run at the
       same time and run in the order given; a job is held back while an earlier one with its
       key has not answered.
     - **`outstanding() -> usize`:** the jobs given whose answer has not yet been **taken** by
       `try_answer` or `answer_timeout` (an answer waiting in the channel still counts). So when
       it is 0, every answer given has been taken, and the worker has sent its event (point 4).
     - A `Lost` job may leave its journal entry and temporary files; they are resolved by
       `Journal::recover` at the next start, as after a crash today. The module comment says so.
     - **`discard()`:** the jobs given but not started are dropped and answered `Discarded`
       (T268's cancel at import); jobs that started finish.
     - **The end:** when the pool is dropped, jobs that started finish, the jobs not started are
       answered `Discarded`, and every thread is joined; a job is never cut off mid-transaction.
     - The first version a pool process reports is not needed by the pool's callers in this task;
       the worker keeps its own first version (point 3).
  3. **The worker with one process is today's worker.** With `write_processes == 1` there is no
     pool: one thread, one ExifTool process, every request in order, unchanged.
  4. **The worker with several processes.** With `write_processes == n > 1` and a `WriteSetup`
     (without one there is no pool, and writes answer `WriteError::NoJournal` as today):
     - the worker thread starts its pool (`WritePool::start_with` with the worker's program and
       arguments) when it takes its first write; a failed start answers that write
       `WriteError::Other` with the error's text, and the next write tries again (as a failed
       ExifTool start is tried again with the next request today);
     - the worker thread keeps its own process for every request that is not a write (the ratings
       read, `read_metadata`, moves, deletes, `prepare_delete`), as today; `exiftool_version`
       stays the first start of that process;
     - the writes (`write_rating`, `write_orientation`, `write_edits`) are given to a pool of `n`
       with the key of their image. Each job runs the same safe-write call as `handle` does today,
       with the same `WriteSetup` (journal, checks) and the options that were current when its
       request was taken from the queue (`set_options` keeps its meaning: requests queued after it
       use the new options). Its answer becomes the same `Event` as today: `TooOld` is
       `WriteError::ExifToolTooOld`, `StartFailed` and `Lost` are `WriteError::Other` with their
       text, as today; `Discarded` does not happen in the worker (it never discards);
     - **everything else waits for earlier writes:** a request that is not a write starts only when
       the pool's `outstanding()` is 0, that is, every earlier write has answered **and its event
       has been sent**, so a move, a delete or a read sent after a write of the same image still
       runs after it and its event comes after the write's, as the module comment promises today;
     - **how the worker waits:** with no outstanding writes it blocks on the request queue as
       today. With outstanding writes it alternates: `answer_timeout` with a short interval (a
       named constant, at most 10 ms), sending each answer's event at once, then `try_recv` on the
       request queue. It takes a new write only while `outstanding() < n`; otherwise, and while a
       non-write is held back, it only waits for answers. So the request queue (`REQUEST_CAPACITY`)
       still fills and the shell still gets `QueueFull` (back-pressure as today), and at most `n`
       writes are given at once. A later write may answer before an earlier one of another image.
       Every event already names its request id and file. The comments that promise serial
       order are brought up to date: the module comment's "Requests run in order" (twice, at
       `set_options` and at moves and deletes) and `MetadataWorker`'s doc comment ("Bounded,
       serial metadata I/O") say that with `n > 1` writes of different images may answer in
       another order than they were asked, while one image's writes, and everything after a
       write, keep their order;
     - the TIFF restart (§9.3) is already per process (`ExifTool::restart` on the process that
       wrote); nothing restarts another thread's process;
     - **the end, as today:** `Drop for MetadataWorker` stays non-blocking (it does not join; the
       UI thread never waits for ExifTool). The worker thread ends, as today, when the request
       queue is closed and empty or an event cannot be sent; it then drops the pool, so writes
       that started finish (their transactions and journal entries ended) and the pool's threads
       are joined on the worker thread. Requests still in the queue, and writes given but not
       started, are not run (today, too, only the request in progress finishes).
  5. **The image check is shared.** `WriteSetup::checks` becomes
     `Arc<dyn ImageCheck + Send + Sync>` (today `Box<dyn ImageCheck + Send>`), so the pool's
     threads share it. Check that `RawImageCheck` and the shell's check (`image_check` and
     `release_before_write` in `crates/pw-loupe/src/main.rs`: the view cores' releasers and
     `HistogramReads`, whose `note_before_write` notes a file's stamp per path) are `Sync` and that
     releases of two files from two threads at once are safe; and that `RawImageCheck::check_copy`
     (`crates/pw-imagecheck/src/lib.rs`: `pw_raw::inspect`, and for a DNG
     `pw_libraw::Session::open_with_diagnostics`) is safe on several threads at once. If one is
     not, stop and say which, since that is a change in another crate.
  6. **The journal.** A test in `journal.rs` that entries begun from several threads at once all
     get distinct files and are all ended, and that `recover` with several open entries resolves
     each (§9.5: the journal holds several open entries, each written whole). If it fails, stop and
     explain before changing `journal.rs`.
  7. **The app.** `crates/pw-loupe/src/main.rs` builds its `WriteSetup` with `write_processes: 1`
     and the check as an `Arc`; `metadata.rs`'s test helper `test_setup` builds its `WriteSetup`
     the same way; nothing else in `pw-loupe` changes. The app behaves as before.
- **Tests to add** (with the fake ExifTool of `src/test_support.rs` where a real one is not
  needed). A **slow fake mode** is added to `test_support.rs` and named in the report: a command
  that names a file whose name holds a marker (for example `slow`) is answered only after a short
  wait, so overlap and answer order can be seen. The test helpers in `worker.rs` (`setup_with`,
  and the real-ExifTool `start`) may change for the new field and the `Arc`; no test body is
  rewritten:
  - `write_pool.rs`: jobs with different keys overlap (a slow job does not keep a quick one of
    another key from answering first); two jobs with one key, and with `a.CR2` and `a.NEF` in one
    folder, never overlap and answer in the order given; a process that breaks is replaced and the
    other threads go on; a process too old answers `TooOld` with its version without running the
    closure; a job that panics answers `Lost` and the thread takes the next job; discard drops
    only jobs not started; dropping the pool lets started jobs finish, answers the rest
    `Discarded` and joins every thread; `n` 0, 1 and 9 clamped to 2..=4; `write_key` for the
    same stem in different case, different stems, different folders;
  - `worker.rs`: with 1 process, the existing worker tests pass unchanged; with 2 and with 4
    processes, writes of different images overlap and may answer out of order; two writes of one
    image answer in order; a move and a `read_metadata` of an image asked after its write run
    after the write has answered, and their events come after the write's event (with the write
    slow); a failed pool start answers the write `Other` and the next write tries again; with more writes sent than `n` the queue gives `QueueFull` as
    with 1; dropping the worker while writes run: within a timeout every file is either unchanged
    or written and the journal folder is empty (no thread check: the drop does not join);
  - `worker.rs` with a real ExifTool (as the file's real-ExifTool tests start it): at least eight
    copies of the small test JPEG written with 4 processes, every file written, its value read
    back, and the journal folder empty afterwards; `set_options` between two writes with 2
    processes: the first uses the old options, the second the new (next to
    `set_options_with_create_iim_writes_the_city_to_iim`);
  - `journal.rs`: point 6.
- **Not in this task:**
  - The import worker's template and the number the app uses (T268); the measurement with 1, 2
    and 4 processes (T267).
  - Any change of the safe write itself (`safe_write.rs`, `verify.rs`) or of the shell's handling
    of answers.
  - A setting for the number (§9.6 point 2: "may become a setting later").
- **Rules:**
  - Every file is its own transaction with every check of ADR 0007 §9.1–§9.5; nothing in a
    transaction is shared between threads except the journal folder and the image check. Known
    and accepted: each file's free-space check (§9.1, §9.2) sees the space before the other
    threads' copies exist, so several writes on an almost full volume can all pass it; the copy
    write then fails cleanly. The agent names it in the report; the reviewer records it in
    `docs/tasks/known-limits.md` at Done.
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
| 2026-11-22 | Blocked | Cold read: the pool's API named (`WritePool<T>`, `start(n)` with the default program for `pw-loupe`, `start_with`, `submit` → `Ticket`, `JobAnswer` with `TooOld` carrying the version and `Lost` for a panicked job, `try_answer`/`answer_timeout` on an unbounded channel, `write_key(&Path) -> WriteKey`, `outstanding`, `discard`), `n` clamped, a failed spawn an error; `start_tool` moved, not duplicated; how the worker waits on two sources and takes a write only while `outstanding() < n` (`QueueFull` kept); the end as today (Drop does not join; the worker thread drops the pool; queued and unstarted requests not run), the drop test changed to match; the slow fake mode required, the `set_options` test moved to the real-ExifTool module, test helpers may change; `metadata.rs`'s `test_setup` added to the files; `check_copy` on several threads in point 5; the free-space check under several writes accepted, recorded in `known-limits.md` at Done; lazy process start and `n + 1` processes said |
| 2026-11-22 | Blocked | Chain review (T266 → T267 → T268): T267 and T268 use T266's API as named. Changed: T266 `outstanding()` counts until an answer is taken, so a non-write runs and its event goes out only after every earlier write's event (test added); the worker starts its pool at its first write, a failed start answers `Other` and is tried again, no pool without a setup; the stale "serial"/"in order" comments named; a `Lost` job's journal entry is recovered at the next start. T268: a failed pool start, when the import's pool starts, the ticket map, unbounded submit said; the load of both pools (up to `2n + 1` processes, `2n` transactions and DNG checks); hand check 4 (a batch while an import writes) and the comparison with the commit before; summary lists compared without order; known limits (two pools on one target, cut transactions at exit, as today); the reviewer's documents listed (ADR §9.6/§14/§18, CHANGELOG, known-limits, a backlog item for the setting, plan row) and what happens with the number 1. T267: `EditReport.report`, at least 2 s |
| 2026-11-22 | Open | T265 done (the analysis has "Measured in T265") |
