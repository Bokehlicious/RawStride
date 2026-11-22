# T268. ADR 0007 §9.6 Q18: the app's writes and the import's template use the owner's number of ExifTool processes (pw-loupe) — Blocked: waits for T267 and the owner's number

- **Why:** ADR 0007 §9.6 point 2 (Q18; approved by the owner 2026-11-22). T266 gave
  `pw-metadata` a pool of ExifTool write processes (`write_pool.rs`) and made the metadata worker
  use it when `WriteSetup::write_processes` is above 1; the app still passes 1. T267 measured a
  batch with 1, 2 and 4 processes, and the owner fixed the number afterwards (recorded in ADR 0007
  §9.6 by the reviewer; **the reviewer writes the number here before this task is set to Open**:
  `<n>`). This task makes the app use it: the editor's, the ratings' and the rotations' batches
  through the metadata worker, and the import's template through the pool. Last task of the chain
  T266 → T267 → T268. If the owner fixes the number at 1, this task is not done; the reviewer
  removes it and records why.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/main.rs` (point 1)
  - `crates/pw-loupe/src/import_worker.rs` (points 2–4, and its tests)
  - `crates/pw-loupe/src/metadata.rs` (point 5: tests; a fix only if a test shows answers out of
    order are mishandled, and then named in the report)
  - `docs/test-inventory.md` (the new tests, rule 12)
  - this task's own file (status and log)
- **What:**
  1. **The number.** One constant in `main.rs` (for example `WRITE_PROCESSES`) with the owner's
     number and a comment citing ADR 0007 §9.6 point 2 and the date of the owner's decision. The
     metadata worker's `WriteSetup` gets it; the import worker is given it where it gets its
     `WriteContext` (whose `checks` becomes an `Arc`, as `WriteSetup::checks` did in T266).
  2. **The import's template through the pool.** With a number above 1, the `ShotWriter`
     (`import_worker.rs`) gives each finished shot's template writes (its `ShotTargets::writes`)
     to a `WritePool` of that size, each with its key (`write_key`), as jobs that call
     `apply_template_safely` as `write_template` does today, and returns, so the copy goes on with
     the next shot while they run. With 1 it writes as today (no pool).
  3. **What stays as today** (ADR 0026 §3.4, T258–T262):
     - **`Placed`** for a shot is sent when every write of that shot has answered, and the shots'
       `Placed` events go out in shot order (a later shot whose writes answered first waits for
       the earlier one), so the shell's tab rules see what they see today;
     - **a cancel writes nothing more:** when a cancel is seen, the pool's jobs not started are
       discarded; a shot with a discarded or unstarted write is not placed, as today a cancel
       during a shot's writes places nothing (`a_cancel_during_a_shots_template_writes_places_nothing`);
     - **a failed start or a too old ExifTool** is listed once, as today, the jobs not started are
       discarded, no later shot gets the template, and those shots are still placed, as today when
       `writes_stopped` is set; the pool is not kept for the next import, so the next import tries
       again, as today;
     - each failed write is one line in `iptc_errors`, as today; their order may differ from the
       plan's;
     - `ShotWriter::finish` (the end of the import) waits until every write given has answered or
       was discarded before `ImportDone` is sent, so the summary holds every write's result.
     - A pool kept between imports (as `IptcRuntime::exiftool` keeps its process today) or one per
       import: the agent chooses, says which and why in the report.
  4. **The progress** shows only the copy (ADR 0026 §3.4, unchanged); the template's writes add no
     progress step.
  5. **Answers out of order in the shell.** With more than one process, the metadata worker may
     answer a batch's writes in another order than they were sent. Read how `metadata.rs` handles
     `EditsWritten` in an editor batch (`batch_written`), `RatingWritten` (`ratings.written`) and
     `OrientationWritten` (`orientations.written`), and add tests that each gives the same result
     when the answers of a batch of three come back in reverse order. If one does not, stop and
     explain before changing it.
- **Tests to add:**
  - `import_worker.rs` (fake ExifTool, as its tests start it, with the number 2): a shot's
    `Placed` comes after its writes; two shots' `Placed` in shot order when the second's writes
    answer first; a cancel during the writes places no shot whose writes were not all made and
    writes nothing more; a too old ExifTool is listed once and the shots are still placed; with
    the number 1 the existing tests pass unchanged (none is rewritten);
  - `import_worker.rs` with a real ExifTool (as `the_template_is_written_safely_to_an_imported_jpeg`
    does): an import of at least six JPEGs with the number 2, the template in every file, the
    journal empty afterwards;
  - `metadata.rs`: point 5.
- **Hand check (the owner):** in a release build,
  1. an edit of City on 20 or more camera JPEGs in one folder at once, timed by hand, against the
     same with the previous release build; every file has the new City;
  2. an import of a card (or a folder) of 30 or more images with a template, timed, against the
     previous release build; every file has the template, and the tabs open as before (ADR 0026
     F4: the first folder's tab on its first image, a later folder's when it opens if the user has
     not moved);
  3. a cancel in the middle of that import: no file written after the cancel, the summary as
     before.
- **Not in this task:**
  - Any change in `pw-metadata` (the pool and the worker are T266's); a setting for the number.
  - Making one file's write faster (§9.6 point 2: one file alone is not faster).
- **Rules:**
  - Every file is its own transaction (ADR 0007 §9.1–§9.5), written by `apply_template_safely` or
    the worker as today.
  - `AGENTS.md` §3; no new dependency. Rule 13 if a text the user sees changes (none is planned).
  - An extra code review (`docs/tasks/README.md`: the safe write while the user keeps working).
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above; then
  the owner's hand check.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written with T266 and T267 (ADR 0007 §9.6 point 2, Q18); waits for T267 and the owner's number |
