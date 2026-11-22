# T268. ADR 0007 §9.6 Q18: the app's writes and the import's template use the owner's number of ExifTool processes (pw-loupe) — Done

- **Why:** ADR 0007 §9.6 point 2 (Q18; approved by the owner 2026-11-22). T266 gave
  `pw-metadata` a pool of ExifTool write processes (`write_pool.rs`) and made the metadata worker
  use it when `WriteSetup::write_processes` is above 1; the app still passes 1. T267 measured a
  batch with 1, 2 and 4 processes, and the owner fixed the number afterwards (recorded in ADR 0007
  §9.6 by the reviewer; **the reviewer writes the number here before this task is set to Open**:
  **4**, the owner, 2026-11-22). This task makes the app use it: the editor's, the ratings' and
  the rotations' batches through the metadata worker, and the import's template through the
  pool. Last task of the chain T266 → T267 → T268. If the owner fixes the number at 1, this task is not done; the reviewer
  removes it and records why (ADR 0007 §9.6 and §14, the plan row), and the pool stays in
  `pw-metadata` unused by the import.
- **The reviewer's documents** (not the agent's): before Open, the number and its date in ADR
  0007 §9.6 point 2, §14 (Q18) and §18; at Done, a CHANGELOG line (Changed: batches and the
  import's template are written by up to four ExifTool processes), `docs/tasks/known-limits.md`
  (T266's free-space line, and this task's limits in Rules), a `docs/tasks/backlog.md` item "a
  setting for the number of ExifTool write processes" (§9.6 point 2: "may become a setting
  later"), and the plan row (awaiting the owner's hand check).
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/main.rs` (point 1)
  - `crates/pw-loupe/src/import_worker.rs` (points 2–4, and its tests)
  - `crates/pw-loupe/src/metadata.rs`, `crates/pw-loupe/src/ratings.rs`,
    `crates/pw-loupe/src/orientations.rs` (point 5: tests; a fix only if a test shows answers out
    of order are mishandled, and then only after stopping and explaining)
  - `docs/test-inventory.md` (the new tests, rule 12)
  - this task's own file (status and log)
- **What:**
  1. **The number.** One constant in `main.rs` (for example `WRITE_PROCESSES`) with the owner's
     number and a comment citing ADR 0007 §9.6 point 2 and the date of the owner's decision. The
     metadata worker's `WriteSetup` gets it. The import worker gets it through a new constructor
     `ImportWorker::start_with_processes(journal, checks, write_processes)`, called at the
     import worker's call site in `main.rs` (`ImportWorker::start(...)` today), and for the tests
     `start_with_exiftool_args_and_processes(program, args, journal, checks, write_processes)`;
     `WriteContext` holds the number. The existing constructors `start`, `start_with_exiftool` and
     `start_with_exiftool_args` keep their names and mean the number 1. Their `checks` parameter
     and `WriteContext::checks` become `Arc<dyn ImageCheck + Send + Sync>` (as
     `WriteSetup::checks` in T266). **Allowed in existing tests, and nothing else:** the
     mechanical edits this needs, `Box::new(..)` → `Arc::new(..)` for the check and
     `write_processes: 1` in a `WriteContext { .. }` literal; no assertion or step of a test
     changes.
  2. **The import's template through the pool.** With a number above 1, the `ShotWriter`
     (`import_worker.rs`) gives each finished shot's template writes (its `ShotTargets::writes`)
     to a `WritePool` of that size (T266: `WritePool::start(n)` when `exiftool_program` is `None`,
     `WritePool::start_with(program, args, n)` otherwise), each with its key (`write_key`), as
     jobs that call `apply_template_safely` as `write_template` does today, and returns, so the
     copy goes on with the next shot while they run. Answers are taken with `try_answer` at each
     progress call during the copy and with `answer_timeout` in `finish`; a `Placed` may thus go
     out at the next progress call after its shot's last answer, which is accepted. With 1 it
     writes as today (no pool). The `ShotWriter` keeps a map from each `Ticket` to its shot and
     target. Submitting is not bounded (the copy may run ahead of the template writes; `Placed`
     then waits, and a cancel discards more); say so in the report. The pool is started when
     the first write is given, so, as today, an import whose template writes nothing or that has
     no journal starts no ExifTool; a failed `WritePool::start` is one `"{file}: {error}"` line
     and sets `writes_stopped`, as a failed `ExifTool::start` does today. The pool's answers map
     to today's lines: `TooOld(Ok(version))` → `exiftool_too_old_text(file, version)`;
     `TooOld(Err(text))`, `StartFailed(text)` and `Lost(text)` → `"{file}: {text}"`, as
     `write_template` lists them. **Load:** with the number `n`, the metadata worker (its own
     process and its pool) and the import's pool make up to `2n + 1` ExifTool processes, up to
     `2n` transactions and up to `2n` DNG image checks (LibRaw) at once; say so in the report.
  3. **What stays as today** (ADR 0026 §3.4, T258–T262):
     - **`Placed`** for a shot is sent when every write of that shot has answered, and the shots'
       `Placed` events go out in shot order (a later shot whose writes answered first waits for
       the earlier one), so the shell's tab rules see what they see today;
     - **a cancel starts no write:** when a cancel is seen (at a progress call, or between the
       waits in `finish`), `discard()` drops the jobs not started; writes that already started
       finish (a transaction is never cut off) and their results are listed as today. From the
       cancel on, **no further `Placed` is sent**, whatever its shot's writes did: a shot whose
       writes all finished but whose `Placed` was held back for an earlier shot is not placed,
       and neither is a shot whose writes all started before the cancel, as today a cancel stops
       every later `Placed` (`cancel_seen` in `finish_shot`;
       `a_cancel_during_a_shots_template_writes_places_nothing`). A discarded write adds no line
       to `iptc_errors`;
     - **a failed start or a too old ExifTool** is listed once, as today (the first such answer;
       later `StartFailed` or `TooOld` answers from the pool's other threads are dropped without
       a line), the jobs not started are discarded, no later shot gets the template, and those
       shots are still placed, as today when `writes_stopped` is set; the next import tries again,
       as today;
     - each failed write is one line in `iptc_errors`, as today; their order may differ from the
       plan's;
     - `ShotWriter::finish` (the end of the import) waits until every write given has answered or
       was discarded before `ImportDone` is sent, so the summary holds every write's result. A
       wait that fails (a pool that is gone) is an `iptc_errors` line, never a panic.
     - A pool kept between imports (as `IptcRuntime::exiftool` keeps its process today) or one per
       import: the agent chooses, says which and why in the report; a pool that answered
       `StartFailed` or `TooOld` is never kept. `ImportWorker`'s drop does not join its thread
       (as today): at the app's exit a write may be cut off, and the journal recovers it, as
       today.
  4. **The progress** shows only the copy (ADR 0026 §3.4, unchanged); the template's writes add no
     progress step.
  5. **Answers out of order in the shell.** With more than one process, the metadata worker may
     answer a batch's writes in another order than they were sent. Read how `metadata.rs` handles
     `EditsWritten` in an editor batch (`batch_written`, `metadata.rs`), `RatingWritten`
     (`Ratings::written`, `ratings.rs`) and `OrientationWritten` (`Orientations::written`,
     `orientations.rs`), and add tests, in the file of each, that each gives the same result when
     the answers of a batch of three different images come back in reverse order (one image's
     writes keep their order, T266). "The same result" is the same values, pending state and
     counts; lists filled as answers come (an editor batch's `failed` and `kept`, `last_failure`)
     are compared without regard to order, since their order follows the answers. If one does
     not give the same result, stop and explain before changing it.
- **Tests to add:**
  - `import_worker.rs` (fake ExifTool, as its tests start it, with the number 2). The import
    tests' fake answers at once, so a shot's writes are made slow or observed by an `ImageCheck`
    in the test module whose `release` records each call and can block on a signal for chosen
    files (as `CancelOnRelease` does), or by a new mode of the tests' own fake; either may be
    added. Tests: a shot's `Placed` comes after its writes' `release` calls; two shots' `Placed`
    in shot order when the second's writes answer first; a cancel during the writes sends no
    further `Placed` and starts no write after it; a too old ExifTool is listed once with its
    version and the shots are still placed; with the number 1 the existing tests pass with only
    the mechanical edits of point 1;
  - `import_worker.rs` with a real ExifTool (as `the_template_is_written_safely_to_an_imported_jpeg`
    does): an import of at least six JPEGs with the number 2, the template in every file, the
    journal empty afterwards;
  - `metadata.rs`: point 5.
- **Hand check (the owner):** in a release build of this task, against a release build of the
  commit before it (the same code with 1 process, so only Q18 differs; the reviewer names both
  commits):
  1. an edit of City on 20 or more camera JPEGs in one folder at once, timed by hand; every file
     has the new City;
  2. an import of a card (or a folder) of 30 or more images with a template, timed; every file
     has the template, and the tabs open as before (ADR 0026 F4: the first folder's tab on its
     first image, a later folder's when it opens if the user has not moved);
  3. a cancel in the middle of that import: no write started after the cancel (writes running
     at the cancel finish), no tab placed after it, the summary as before;
  4. while an import with a template runs, a rating and an edit of City on a batch of images in
     another open folder: both written, the import's template in every file, nothing refused.
- **Not in this task:**
  - Any change in `pw-metadata` (the pool and the worker are T266's); a setting for the number.
  - Making one file's write faster (§9.6 point 2: one file alone is not faster).
- **Rules:**
  - Every file is its own transaction (ADR 0007 §9.1–§9.5), written by `apply_template_safely` or
    the worker as today.
  - **Known limits, as today** (the reviewer records them in `docs/tasks/known-limits.md` at
    Done): `write_key` keeps one target from being written twice at once only within one pool,
    so the metadata worker's pool and the import's pool can still meet on one target (a backup
    folder open in a tab, or a sidecar `A001.xmp` shared by an existing `A001.CR2` and an
    imported `A001.ARW`), as the worker's and the import's single processes can today; and at
    the app's exit up to `n` transactions of each pool can be cut off after `EXIT_WAIT_LIMIT`,
    which the journal recovers at the next start, as with one today.
  - `AGENTS.md` §3; no new dependency. Rule 13 if a text the user sees changes (none is planned).
  - An extra code review (`docs/tasks/README.md`: the safe write while the user keeps working).
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above; then
  the owner's hand check.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written with T266 and T267 (ADR 0007 §9.6 point 2, Q18); waits for T267 and the owner's number |
| 2026-11-22 | Blocked | Cold read: the default program solved in T266 (`WritePool::start(n)`); the constructors: old ones mean 1, new `start_with_processes` and `start_with_exiftool_args_and_processes`, `checks` an `Arc`, the mechanical test edits (`Box::new` → `Arc::new`, `write_processes: 1`) allowed and nothing else; the pool's answers mapped to today's lines (`TooOld` carries the version, T266), later start or too-old answers dropped, a discarded write no line; the cancel rule made exact (no `Placed` after a cancel, started writes finish, a cancel seen in `finish`'s wait), hand check 3 worded to match; a pool that failed is never kept; answers taken at progress calls and in `finish`; `ratings.rs` and `orientations.rs` named, three different images; a recording or blocking `ImageCheck` or a new fake mode allowed in the tests; no panic in the waits; the process count said. At Done the reviewer adds a CHANGELOG line (Changed) |
| 2026-11-22 | Blocked | The owner, 2026-11-22: two pools meeting on one target is a known limit, not an ADR rule change (recorded in ADR 0007 §9.6 point 2); this task's Rules stand |
| 2026-11-22 | Open | T267 done; the owner, 2026-11-22: the number is four (ADR 0007 §9.6 point 2, §14 Q18, §18); written in "Why" and the CHANGELOG line |
| 2026-11-22 | Ready for review | `WRITE_PROCESSES = 4` in `main.rs` for the metadata worker and the import (`start_with_processes`); `checks` an `Arc`. With more than one process the `ShotWriter` gives a finished shot's template writes to a `WritePool` kept for that import only (started at the first write, dropped at its end; `IptcRuntime` and `run_import` unchanged), holds each shot until its writes and every earlier shot's `Placed` are done, takes answers at each progress call and in `finish`, discards on a cancel or the first `StartFailed`/`TooOld`; with 1 the old path. Tests: five with a new mode of the tests' fake (logs each command's file, holds back marked files), one with real ExifTool (six JPEGs), and point 5 in `metadata.rs`, `ratings.rs` and `orientations.rs`; all three give the same result, no fix. Notes for review: the fake cannot reach `release` (its read answer is not metadata), so the tests observe its commands; `answer_timeout` cannot tell a gone pool from a timeout, and the pool holds its own sender, so the wait in `finish` has no failure path to list. check.ps1 all OK (2285 passed, 77 ignored), no clippy warnings |
| 2026-11-22 | Open | Review (the reviewer, and an independent review agent of the diff as the README asks): the pooled path is correct (`Placed` after a shot's writes and in shot order, none after a cancel, `discard` at the first cancel seen, a discarded write no line, `StartFailed`/`TooOld` once and the shots still placed, no journal one line, `finish` waits for every answer), the 1-process path unchanged, point 5 done, existing tests changed only mechanically, `AGENTS.md` §3 kept. Sent back by the owner (2026-11-22) for one point; everything else stays as it is. **1.** With more than one process `ShotWriter::finish` → `wait_for_writes` calls `cancel_seen()` before it looks for a pool, and `place_ready()` again after the loop, also when no pool was started (an import without a template, or whose writes were all stopped before the first submit). A cancel that comes after the last progress call and before `finish` then marks the import cancelled and skips the eject (`run_import`, `done.cancelled`), although every file was copied; with 1 process such a late cancel is not counted. Make `finish` with no pool behave as with 1 process (no `cancel_seen()` and no `place_ready()`-cancel check when `self.pool` is `None`; held shots with no writes are already placed at the progress calls), and add a test in `import_worker.rs` with 2 processes: a `ShotWriter` whose import has no template, the cancel flag set after the last `file_done`, then `finish` answers `cancelled == false` and every shot was placed. Not to change: the cancel check in `finish` when a pool exists. Recorded at Done by the reviewer, not by the agent: a cancel during a long copy is seen at the next progress call, so the pool may start queued writes until then (known limits); the report did not name the unbounded submit and the load of up to 2n + 1 ExifTool processes (point 2), say both in the report this time. `.\tools\check.ps1` again |
| 2026-11-22 | Ready for review | Point 1: `ShotWriter::finish` waits for the pool's writes (with its cancel check) only when a pool was started; without one it does what it does with 1 process. New test `with_two_processes_and_no_template_a_cancel_after_the_last_file_is_not_counted` (no pool started, the cancel set after the last `file_done`, `finish` not cancelled, both shots placed). `docs/test-inventory.md` updated. The report names the unbounded submit and the load of up to 2n + 1 ExifTool processes. check.ps1 all OK (2286 passed, 77 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewed again and approved by the owner 2026-11-22: point 1 done (`finish` waits, with its cancel check, only when a pool was started; without one as with 1 process; the new test fails without it); nothing else changed since the first review. `test-output.txt` newer than the code, all OK (2286 passed, 77 ignored), no clippy warnings. The extra code review was the independent review agent's in the first round. Recorded by the reviewer: a CHANGELOG line (Changed), `known-limits.md` (two pools on one target, up to `n` writes of each pool cut off at the app's exit, a cancel during a long copy seen at the next progress call; T266's free-space line already there), a `backlog.md` item (a setting for the number), the plan row. Load: up to 2n + 1 = 9 ExifTool processes and 8 transactions at once; the template's writes are submitted without a bound, so the progress may stay at 100 % while they finish. The owner's hand check follows, a release build of this commit against `4d52b3d` (the same code with 1 process) |
| 2026-11-22 | Done | The owner's hand check (release build of `1d91448` against `4d52b3d`, ExifTool 13.59): points 1, 2, 3 and 4 passed. Point 1: City on 50 Sony A1 JPEGs, 33.81 s with 1 process, 9.79 s with 4 (3.5×). Point 2: an import of 55 RW2 files with a template, 17.53 s with 1 process, 2.6 s with 4, every template write done by then (6.7×; with 4 the copy no longer waits for each shot's template writes) |
