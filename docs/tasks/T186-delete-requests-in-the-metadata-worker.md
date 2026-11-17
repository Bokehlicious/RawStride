# T186. ADR 0018 B2: preparing and deleting an image as requests to the metadata worker (pw-metadata) — Blocked: waits for T185

- **Chain:** T185 (B1) → T186 (B2) → T187, T188 (B3) → T189 (B4) → T190 (B5). The owner's
  answers are B-Q1–B-Q15 in ADR 0018 §3; how the steps fit is ADR 0018 §2.10.
- **Why:** ADR 0018 §2.3: a delete is a request to the metadata worker, so a rating or metadata
  write sent earlier is finished first, the view cores' sessions are released on the worker,
  and nothing is done on the UI thread. The confirmation (§2.5) needs the number of files and
  their size before anything is deleted; finding the companion files and checking the drive is
  file I/O too (a network drive can be slow), so it is a second request, `prepare_delete`, that
  changes nothing. Both answer with T185's structured `DeleteError`, so the shell can tell a
  card from a locked file (plan row B2).
- **Crates:** `pw-metadata` (and one match in `pw-loupe`). **Files:**
  `crates/pw-metadata/src/worker.rs`, `crates/pw-metadata/src/lib.rs` (re-exports only),
  `crates/pw-loupe/src/metadata.rs` (only: `apply`'s match on the worker's `Event` has no
  catch-all, so it gets one arm `WorkerEvent::DeletePrepared { .. } | WorkerEvent::Deleted { .. }
  => {}` that ignores them until T188; the worker's `Event` is `WorkerEvent` there), `docs/test-inventory.md`, and this task's own file (status and log).
  **Starts after T185 and T184 are committed** (T184 changes `worker.rs`'s tests).
- **What:**
  1. **Types** (`worker.rs`, re-exported from `lib.rs`): `pub struct DeleteGroup { pub files:
     Vec<PathBuf>, pub bytes: u64 }` (the group as `relocate::companions` gives it, the image
     first, and its size from `recycle::inspect_group`) and `pub struct PreparedDelete { pub
     image: PathBuf, pub result: Result<DeleteGroup, DeleteError> }`, both `Clone`, `Debug`,
     `PartialEq`, `Eq`.
  2. **`MetadataWorker::prepare_delete(&mut self, images: Vec<PathBuf>, image_extensions:
     Vec<String>, suffixes: Vec<String>) -> Result<RequestId, SubmitError>`**, a new
     `Request::PrepareDelete`, answered with **`Event::DeletePrepared { id, images:
     Vec<PreparedDelete> }`**: one entry per image, in the order given. For each image:
     `relocate::companions` (an `io::ErrorKind::NotFound` is `DeleteError { file: image, reason:
     DeleteRefusal::Missing, left: [] }`, `InvalidInput` (the image is not a file) is `NotAFile`,
     any other error `DeleteRefusal::Failed` with the error's text; `file` is the image made
     absolute with `std::path::absolute`, or as given if that fails), then `recycle::inspect_group` of the files (its error as it is). It needs no
     `WriteSetup` and no ExifTool, releases nothing and changes nothing on disk. Overlapping groups
     (a marked RAW and its JPG partner) are each answered; the shell joins them (T187).
  3. **`MetadataWorker::delete_image(&mut self, image: PathBuf, image_extensions: Vec<String>,
     suffixes: Vec<String>) -> Result<RequestId, SubmitError>`**, `Request::Delete`, answered with
     **`Event::Deleted { id, image, result: Result<Vec<PathBuf>, DeleteError> }`**: the files
     that went to the Recycle Bin. On the worker: `relocate::companions` (errors as in point 2);
     then `checks.release_for_move(file)` for the image files of the group by the same rule as
     `move_image` (the image itself always, and every file whose extension is in
     `image_extensions`, without regard to case); then `recycle::delete_group(&files)`; `Ok` is
     the group's files, exactly as `relocate::companions` built them from the image as sent
     (the shell's form); `image` in both events is echoed as sent, as `Moved` does. Only the release loop of `move_image` becomes one private helper used by
     both (no change of behaviour for the move); the move keeps its own error text.
  4. **Without a `WriteSetup`** (no journal, so no `checks`): `delete_image` fails at once with
     `DeleteRefusal::Failed(NO_JOURNAL.to_string())` and `file` the image, as a move does (the
     sessions cannot be released). `prepare_delete` works without one.
  5. Requests run in the order sent, as today: a `delete_image` sent after a `write_rating` of the
     same image runs after it. Neither request uses ExifTool: `handle` ignores `running` and
     `start_error` for them, as for a move (the loop's ExifTool start attempt before each request
     stays as it is). The module's doc comment (`worker.rs` about lines 26–37) gets the two
     requests beside the moves, and the new methods and events get doc comments as the move's
     have. Nothing else in the worker changes.
  6. **The move's error stays text** (plan row B2 asked whether `Event::Moved` gets a structured
     error too): not in this task; the backlog item "Leftovers from the review of T170" keeps it
     (owner, 2026-11-17, ADR 0018 B-Q17).
- **Tests to add** (in `worker.rs`'s tests, started with a missing ExifTool program as the move
  tests are, a temporary folder of each test's own, and the existing `RecordingCheck`). The
  existing test that matches every `Event` variant gets arms for the two new ones, and nothing
  else in it changes. Every test that holds a file open or expects an `Ok` group is
  `#[cfg(windows)]` (T185's functions return `Unsupported` elsewhere); the extensions passed are
  `["arw", "jpg"]` and the suffixes `["dop"]`. They depend on the temporary folder's drive passing
  T185's drive check; if it does not, the test fails with the refusal in its message and the
  agent stops and reports it. These tests put nothing in the Recycle Bin:
  - `prepare_delete` of two images, one with an `.ARW.dop` and an `.XMP`, answers
    `DeletePrepared` with each group (the image first) and its byte count, in the order given,
    and changes nothing (every file still there; `RecordingCheck`'s `released()` and
    `released_for_move()` both empty);
  - `prepare_delete` of a missing image answers `Missing` for it and the other image's group;
  - `prepare_delete` works without a `WriteSetup`;
  - `delete_image` without a `WriteSetup` answers `Failed(NO_JOURNAL)` and nothing is deleted;
  - `delete_image` of a group whose `.XMP` is held open (as in T185's tests, read and write
    sharing without delete) answers `OpenElsewhere` naming the `.XMP`, `release_for_move` was
    called for the image (not the `.XMP`), and every file is still there;
  - a `delete_image` sent right after a `write_rating` of the same image is answered after the
    `RatingWritten` (order of events, as `a_move_sent_after_a_rating_write_is_answered_after_it`;
    the image's `.XMP` held open as above so nothing is deleted, and the answer's reason is
    asserted to be `OpenElsewhere`).

  **With the real Recycle Bin** (`#[ignore]`, ADR 0018 B-Q15): `delete_image` of an `.ARW` with
  its `.ARW.dop` (named `pw-metadata-recycle-test-…`, as T185's) answers `Deleted` with both
  files, both are gone, and `release_for_move` was called for the image; it waits up to 30
  seconds for the answer and fails with "a Windows dialog may be open" (not `next_event`'s 10
  seconds). Its name contains `delete_to_recycle_bin`; run it once with `cargo test --workspace
  -- --ignored delete_to_recycle_bin` and paste the output in the report.
  `.\tools\check.ps1 -Ignored` runs it too and so puts files in the Recycle Bin (B-Q15).
- **Not in this task:** the shell (T187–T190). A rating write sent after the delete's answer, for
  a file that is gone, fails as any write to a missing file does (T189 keeps the keys from it).
- **Rules:** `AGENTS.md` §3. This changes the metadata worker (`pw-metadata`) and two crates, so
  it gets an independent code review before the owner's approval (`docs/tasks/README.md`).
- **Done when:** `.\tools\check.ps1` is all OK with no clippy warnings, the ignored test has been
  run once with its output in the report, and `git diff --stat` shows no change from this task
  outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked: waits for T185 and T184 | Added for ADR 0018 B2, written with B3–B5 for the chain review. Plan row B2's question about the move's text error: the reviewer proposes to leave it (the backlog item stays); the owner's answer goes here |
| 2026-11-17 | Blocked: waits for T185 and T184 | Cold read: doable with the named files (the only exhaustive matches on `Event` are the worker test and `metadata.rs`'s `apply`). Changed: the arm written with `WorkerEvent`; companions' errors (absolute `file`, `InvalidInput` is `NotAFile`); only the release loop shared; doc comments; `#[cfg(windows)]`, the extensions and suffixes, and the drive's dependence for the tests; the ignored test named, prefixed, with a 30-second wait, and its own filter (`worker::` also matched an import-worker test) |
| 2026-11-17 | Blocked: waits for T185 and T184 | Point 6 decided by the owner: the move's error stays text (ADR 0018 B-Q17) |
| 2026-11-17 | Blocked: waits for T185 and T184 | Chain review (fresh agent, T185–T190 read together): the answers' paths are the shell's form (companions built from the image as sent; T185's errors give paths as given), so T187–T189 can compare them |
| 2026-11-17 | Blocked: waits for T185 | T184 is Done and committed: waits for T185 only. T184 added an empty `release_for_move` to `FailingCheck` in `worker.rs`'s tests (about line 1376): line numbers to be checked when this task is set Open |
