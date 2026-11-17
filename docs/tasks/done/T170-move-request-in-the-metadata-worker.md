# T170. ADR 0017 U9b, second part: the move as a request to the metadata worker — Done

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174. The owner's answers U9-Q5–U9-Q10 are in ADR 0017 §6.1.
- **Why:** ADR 0017 §2.6: a pending rating or metadata write to the file or its sidecar is
  finished before the move, and the view cores' sessions of the file are closed first. The
  metadata worker runs its requests in order and already closes the sessions before a write
  (`WriteSetup::checks`, `ImageCheck::release`), so a move sent to it after the rating write
  comes after that write, and the files it moves are released the same way. The files are found
  and moved on the worker thread, not on the UI thread.
- **Crates:** `pw-metadata` (and one match in `pw-loupe`). **Files:**
  `crates/pw-metadata/src/worker.rs`, `crates/pw-metadata/src/lib.rs` (re-exports only, if
  needed), `crates/pw-loupe/src/metadata.rs` (only: `apply`'s match on the worker's `Event` has
  no catch-all, so it gets one arm for `Event::Moved` and `Event::MovedBack` that ignores them
  until T172), `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **`MetadataWorker::move_image(&mut self, image: PathBuf, target: PathBuf, image_extensions:
     Vec<String>, suffixes: Vec<String>) -> Result<RequestId, SubmitError>`**, a new
     `Request::Move`, answered with **`Event::Moved { id, image, result: Result<Vec<Moved>,
     String> }`**. On the worker: `relocate::companions` (T169) of `image` (the `Vec<String>`s
     passed as `&[&str]`); then, for each file of the group whose extension is in
     `image_extensions` without regard to case (the image and its partners),
     `checks.release(file)`; then `relocate::move_group(&files, &target)`. An error of either is
     the `Err` text (`MoveError`'s `Display`, or the folder read's error).
  2. **`MetadataWorker::move_back(&mut self, moved: Vec<Moved>) -> Result<RequestId,
     SubmitError>`**, `Request::MoveBack`, answered with **`Event::MovedBack { id, moved, result:
     Result<(), String> }`** (`moved` as sent, so the caller knows which step it was). On the
     worker: `checks.release(to)` for every `to` (releasing a file that no session holds does
     nothing), then `relocate::move_back`.
  3. **Without a `WriteSetup`** (no journal, so no `checks`): both requests fail at once with
     `Err(NO_JOURNAL.to_string())`, as a rating write fails with `WriteError::NoJournal` (the
     sessions cannot be released, so nothing is moved).
  4. Requests run in the order sent, as today: a `move_image` sent after a `write_rating` of the
     same image runs after it. A move does not use ExifTool: `handle` ignores `running` and
     `start_error` for the two new requests, so a missing ExifTool does not fail a move. The
     loop's start attempt before each request stays as it is (nothing else in the worker
     changes).
  5. Re-export `Moved` and `MoveError` (T169) from `lib.rs` if T169 did not.
- **Tests to add** (in `worker.rs`'s tests, started with a missing ExifTool program as the
  existing tests without ExifTool are, a temporary folder, and a new test `ImageCheck` that
  records the paths given to `release`, e.g. in an `Arc<Mutex<Vec<PathBuf>>>`; none exists yet).
  The existing test that matches every `Event` variant gets arms for the two new ones, and
  nothing else in it changes. The order test shows the order of the events only (without
  ExifTool the rating write answers at once with an error):
  - a `move_image` moves the image and its companion into `target`, answers `Moved` with the
    moves, and called `release` for the image (not for the `.xmp`);
  - a `move_image` sent right after a `write_rating` of the same image is answered after the
    `RatingWritten` (order of events);
  - a failing move answers `Err` with the file's name, and nothing is left in `target`;
  - `move_back` returns the files and answers `MovedBack` with `Ok`;
  - without a `WriteSetup`, both answer `Err(NO_JOURNAL)` and nothing moves.
- **Not in this task:** the shell (T171–T174).
- **Rules:** AGENTS.md §3. This changes the metadata worker (`pw-metadata`), so it gets an
  independent code review before the owner's approval (`docs/tasks/README.md`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added for ADR 0017 U9b; waits for T169 |
| 2026-11-17 | Blocked | Cold read: the new `Event` variants break `pw-loupe`'s exhaustive match, so `metadata.rs` gets an ignoring arm here (named); the existing all-variants test is extended; a recording `ImageCheck` must be written; a move ignores ExifTool's state (the loop's start attempt stays); `NO_JOURNAL` as text; image extensions compared without case |
| 2026-11-17 | Open | T169 is done (`pw_metadata::relocate`: `companions`, `move_group`, `move_back`, `Moved`, `MoveError`) |
| 2026-11-17 | Ready for review | `move_image`/`move_back`, `Request::Move`/`MoveBack`, `Event::Moved`/`MovedBack` in `worker.rs` (no ExifTool used; `NO_JOURNAL` without a setup); `Moved` and `MoveError` were already re-exported, so `lib.rs` is unchanged; an ignoring arm in `pw-loupe`'s `apply`; five worker tests with a recording `ImageCheck`; `docs/test-inventory.md` (pw-metadata 395 incl. 1 ignored; the row said 389 before, one short). `check.ps1` all OK, 1751 passed, 66 ignored |
| 2026-11-17 | Review | Reviewer: as written, `check.ps1` all OK. Independent review (`/code-review high`), 8 findings: (1) a move's release notes a histogram stamp in the program that no answer takes → T177, before T174; (2) a view core may reopen a file between release and move → known limit in T174 point 7 and `backlog.md`; (3) `MoveError::left` lost as text → T172 "Not in this task" and `backlog.md`; (4) the image itself now always released, (5) a failed folder read now names the image (`Could not move <name>: …`), both fixed by the reviewer on the owner's word, with two tests; (6) the ExifTool start attempt kept as the task says; (7) the names kept; (8) the order test shows only the order of events, as the task says. `check.ps1` all OK, 1753 passed, 66 ignored |
| 2026-11-17 | Done | Approved by the owner; leftovers in T172, T174, T177 and `backlog.md` |
