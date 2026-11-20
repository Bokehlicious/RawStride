# T236. ADR 0024 O3b: the shell's store of turns and its write queue (pw-loupe, logic only) — Done

- **Why:** ADR 0024 §2.5, O-Q8 (accepted by the owner 2026-11-17): a turn works "as a rating key:
  the image turns at once in the view and its thumbnail; the write goes into the same ordered
  queue as the ratings; a failed write turns it back and shows the reason". O2 (T233) gave the
  metadata worker `write_orientation` and `Event::OrientationWritten`. The shell keeps only the
  XMP orientations it has read (`MetadataCache::orientations`, T232), with no rule for a value
  shown before it is written. This task adds that rule as plain, tested logic, the way
  `RatingStore` (`ratings.rs`) does it for ratings; T237 wires it to the keys, the menu, the
  metadata cache and the view. It settles two needs of the plan's O3 row:
  - **(a)** a shown-versus-confirmed rule like `RatingStore::read`'s, so a folder read that lands
    while a turn is queued or sent does not undo the turn on screen;
  - **(b)** the value a turn starts from for an image with no XMP value: the shell does not have
    the embedded Exif orientation (the view core keeps only LibRaw flips, which lose the mirror,
    and the folder read reads a RAW's sidecar, not the RAW). It is read with `pw_raw::inspect` on
    a thread of the shell's own, as the capture times are (`sort.rs`, `CaptureReader`), and only
    for an image that is turned without a known value. Turns pressed before that answer comes are
    kept and applied when it comes.
- **Chain:** T235 → **T236** → T237 (plan O3 row). Blocked until T235 is Done.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/orientations.rs` (new: points 1–4 and
  their tests), `crates/pw-loupe/src/lib.rs` (`mod orientations;` and its `pub use`, point 5),
  `crates/pw-loupe/src/ratings.rs` (point 5: `absolute_path` made `pub(crate)`, nothing else),
  `crates/pw-loupe/src/rating_text.rs` (point 6), `docs/glossary.md` (point 7),
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **`OrientationWriter`** (`orientations.rs`), as `RatingWriter`:
     `fn write_orientation(&mut self, path: PathBuf, orientation: u16) -> Result<RequestId, SubmitError>`,
     implemented for `pw_metadata::MetadataWorker` by calling its `write_orientation`. Use
     `pw_metadata::{RequestId, SubmitError}`, as `ratings.rs:9` does, not the crate's own types
     of the same names (`import_worker.rs`).
  2. **`OrientationStore`** (`#[derive(Clone, Debug, Default)]`), keyed by absolute path (made
     absolute with `ratings.rs`'s `absolute_path`, as `RatingStore` does). Per path it keeps:
     `confirmed: Option<u16>` (read from XMP, or written by RawStride and answered `Ok`),
     `confirmed_by_write: bool`, `shown: Option<u16>`, `embedded: Option<u16>` (the file's own
     Exif value, once read), `waiting: u8` (clockwise quarter turns, 0–3, pressed before the
     value to start from was known) and `requested: bool` (the embedded value was asked for and
     has not come). The **shown** value is the one told to the view cores and the thumbnails;
     `None` means "the file's own". Values outside `1..=8` are never kept (taken as missing).
     Methods (all paths absolute except where `turn_many` resolves them):
     - `turn_many(&mut self, paths: &[PathBuf], clockwise: bool) -> Result<Vec<PathBuf>, RatingRefused>`:
       the paths that `pw_metadata::supports_rating` refuses are skipped; when none is left,
       `Err(RatingRefused::Unsupported)` and nothing changes (as `RatingStore::set_many`). Each
       other path is resolved (a resolution error is returned as
       `RatingRefused::PathResolution` before anything changes); a path given twice is turned
       once. Then, for each path in order:
       the value to start from is `shown`, else `confirmed`, else `embedded`. When there is one,
       the new value is `pw_viewcore::geometry::turned(start, clockwise)` (mirror kept, ADR 0024
       §2.3) and it is put (below). When there is none, `waiting` becomes
       `(waiting + if clockwise { 1 } else { 3 }) % 4`, and if `requested` is false it is set
       and the path is returned. Returns the paths whose embedded value the caller must ask for
       (T237 asks the reader of point 3). Once the paths are checked it resets the failure count
       (as a rating key does), also when every path only waits. `RatingRefused` is reused as the
       error type; its `Display` gives the rating text, so the caller (T237) shows
       `ROTATION_UNSUPPORTED_TEXT` for `Unsupported`. A folder read that brings `confirmed`
       while turns wait changes nothing about them: they are applied on top of the start value
       when the embedded answer comes (intended).
     - `embedded_read(&mut self, path: &Path, orientation: Option<u16>)`: the answer of the
       reader. `embedded` becomes the value when it is in `1..=8`, else `1` (as the thumbnails
       take a file `pw_raw::inspect` cannot read, `pw-thumbcache` `extract.rs`); `requested`
       becomes false. If `waiting` is not 0, the start value (`shown`, else `confirmed`, else
       the new `embedded`) is turned clockwise `waiting` times, put, and `waiting` becomes 0. A
       path not in the store is ignored.
     - **put** (private): `shown` becomes the value; a queued, unsent write of the path is
       replaced, otherwise a write is queued at the end (as `RatingStore::put`); if `shown`
       changed, `(path, Some(value))` is recorded as a change.
     - `pump(&mut self, writer: &mut impl OrientationWriter) -> Vec<String>`: as
       `RatingStore::pump` (key order, `QueueFull` keeps the rest, `Stopped` fails the write with
       `worker_stopped_text()`).
     - `written(&mut self, id: RequestId, path: &Path, result: Result<(), String>) -> Option<String>`:
       as `RatingStore::written`. `Ok`: `confirmed` becomes the value written; for a file that
       holds its own metadata (`pw_metadata::supports_embedded`: JPEG, TIFF, DNG) also
       `confirmed_by_write` becomes true and `embedded` becomes the value written (it is now the
       file's own Exif value); for a RAW (sidecar) neither, since the folder read reads the
       sidecar. `shown` follows `confirmed` unless a newer write of the path is queued or sent,
       and a change is recorded when `shown` changed. `Err`: unless a newer write is queued or sent, `shown` goes back to `confirmed`
       (the rollback, a change recorded when `shown` changed); returns the message of point 6.
     - `read(&mut self, read_id: RequestId, orientations: &HashMap<PathBuf, u16>, paths: &[PathBuf])`:
       the folder read (as `RatingStore::read`). For each path: a value in the map becomes
       `confirmed` (and `confirmed_by_write` false). A path missing from the map clears
       `confirmed` **unless `confirmed_by_write` is true** (set only for those files): for a JPEG,
       TIFF or DNG the written
       value lives in IFD0, which the folder read does not read, so a missing XMP value says
       nothing about it. Then `shown` becomes `confirmed` unless a write of the path is queued
       or a write sent after `read_id` is not answered (the rule of `RatingStore::read`). A
       change is recorded when `shown` changed.
     - `orientation(&self, path: &Path) -> Option<u16>`: the shown value.
     - `take_changes(&mut self) -> Vec<(PathBuf, Option<u16>)>`: the changes of the shown values
       since the last call, in order; empties the list.
     - `is_pending(&self, path: &Path) -> bool`, `pending(&self) -> usize`,
       `pending_paths(&self) -> Vec<PathBuf>`, `fail_sent(&mut self) -> Vec<String>`: as
       `RatingStore`'s. A path that is only waiting for its embedded value is not pending (no
       write exists yet).
     - `is_waiting(&self, path: &Path) -> bool` (`requested` or `waiting` not 0) and
       `waiting_paths(&self) -> Vec<PathBuf>` (sorted): T237 makes moves, deletes and "Open in"
       wait for these as well, and lists them at exit.
     - `prune(&mut self, keep: impl Fn(&Path) -> bool)`: drops the paths not kept and not
       pending; each dropped path whose `shown` was `Some` records `(path, None)`, the dropped
       paths sorted (the rule of `MetadataCache::prune`, T232: the view cores keep told values per
       path; a `HashMap`'s order is not fixed). A path only waiting for its embedded value is not
       pending, so it is dropped and its waiting turns are lost (a later `embedded_read` for it
       is ignored); that is accepted (the image left the open folders).
     - **Why `confirmed_by_write` must stay** (chain review): the thumbnail cache's key is
       `(path, cell, orientation)` with no file time (`pw-thumbcache` `worker.rs`), and the view
       core keeps the flip it read before the write (a release drops sessions, not sizes). A
       written JPEG, TIFF or DNG whose shown value went back to `None` would therefore be shown
       with a stale thumbnail and a stale flip until a restart. Do not simplify it away.
     - `failure_text(&self) -> Option<String>`: as `RatingStore::failure_text`, with the texts of
       point 6. The store has its own private `fail_write` (copied from `RatingStore`'s, built on
       the rotation texts); `ratings.rs` is not changed beyond point 5.
  3. **`EmbeddedOrientationReader`** (`orientations.rs`), as `sort.rs`'s `CaptureReader`: one
     thread of its own (named `pw-orientation-reader`) that reads one file at a time with
     `pw_raw::inspect` and answers `(path, Option<u16>)`: `Some(info.orientation)` when inspect
     succeeds, `None` when it fails. `start()`, `request(path) -> bool` (false when the reader has
     stopped; the caller then reads the value itself, T237), and
     `try_recv() -> Result<Option<(PathBuf, Option<u16>)>, ReaderStopped>` (`Ok(None)`: no
     answer yet; `Err(ReaderStopped)`: the thread has ended, so no answer will come; a small
     public unit struct `ReaderStopped`, re-exported), with `request(&self, path: PathBuf)`; it
     derives `Debug` (the workspace warns on `missing_debug_implementations`). The thread ends when the reader is dropped;
     when it cannot be started, the reason goes to stderr as in `CaptureReader::start`.
  4. **No wiring:** nothing in `main.rs`, `metadata.rs` or `shell.rs` uses the store in this
     task (T237 does). The module may need `#[allow(dead_code)]` only if clippy asks for it on
     items `lib.rs` does not re-export; prefer re-exporting what T237 will use.
  5. **`lib.rs`:** `mod orientations;` and
     `pub use orientations::{EmbeddedOrientationReader, OrientationStore, OrientationWriter,
     ReaderStopped};`;
     `ratings.rs`'s `absolute_path` becomes `pub(crate)` so the store uses the same rule.
  6. **Texts** (`rating_text.rs`, rule 13, English): `ROTATION_UNSUPPORTED_TEXT`
     ("Rotations cannot be saved for this file type."), `pub(crate) fn
     rotation_failure_message(name, error)` ("Could not save the rotation for {name}: {error}")
     and `pub fn rotation_failures_text(count, name, error)` ("Could not save the rotation for {count}
     images. Last: {name}: {error}", with "1 image" for one), beside the rating texts; the
     module comment names them; `ROTATION_UNSUPPORTED_TEXT` is re-exported from `lib.rs` beside
     `UNSUPPORTED_TEXT`.
  7. **`docs/glossary.md`** (rule 13; it has no such term yet): "Rotate Left / Rotate Right"
     (turn the image 90° anticlockwise / clockwise; for a RAW written to its sidecar, for a
     JPEG, TIFF or DNG to the file's Exif orientation; the pixels are never changed) and
     "rotation" (the turn RawStride shows and writes). Name them in the report.
- **Tests to add** (in `orientations.rs`, with a fake writer as `ratings.rs`'s `FakeWriter`;
  absolute test paths as `ratings.rs`'s `image`; request ids made as `ratings.rs`'s tests make
  them; the tests are inside the module, so they may read the private entry fields, for
  example `waiting`. "Embedded n" below always means: turn first, then `embedded_read(path,
  Some(n))`, since an answer for a path not in the store is ignored):
  - `a_turn_from_a_read_value_is_shown_at_once_and_queued`: read 6 for a path → a clockwise turn
    shows 3, records the change `(path, Some(3))`, and `pump` sends 3.
  - `a_turn_without_a_known_value_asks_for_the_embedded_one_once`: two clockwise turns on a path
    with nothing read → the first returns the path, the second returns nothing; nothing is shown
    or queued; `embedded_read(path, Some(1))` → shown 3 (two turns from 1), one write of 3
    queued; `waiting` is 0 after.
  - `turns_that_cancel_before_the_embedded_value_comes_write_nothing`: clockwise then
    anticlockwise, then `embedded_read(.., Some(6))` → nothing shown or queued.
  - `a_path_waits_from_the_turn_until_its_embedded_value_comes`: `is_waiting` and
    `waiting_paths` true / listed after a turn without a known value, false / empty after
    `embedded_read`.
  - `a_failed_embedded_read_starts_from_1`: `embedded_read(.., None)` after one clockwise turn →
    shown 6.
  - `a_mirrored_value_stays_mirrored`: read 2, clockwise → 7 (ADR 0024 §2.3).
  - `queued_turns_coalesce_into_the_last_value`: a read of 1, then three clockwise turns before a
    pump → one write of 8.
  - `a_failed_write_turns_back_to_the_confirmed_value`: read 6, turn to 3, pump, answer `Err` →
    shown 6, change recorded, `failure_text` names the file with the rotation text.
  - `a_failed_write_of_an_image_without_a_read_value_turns_back_to_its_own`: embedded 1, turn to
    6, pump, `Err` → shown `None`, change `(path, None)`.
  - `a_failed_write_keeps_a_newer_queued_value` and `a_failed_write_keeps_a_newer_sent_value`:
    as `ratings.rs`'s `failed_write_keeps_a_newer_queued_value` and
    `failed_write_keeps_a_newer_sent_value`.
  - `a_folder_read_does_not_undo_a_queued_or_newer_sent_turn`: read 6 (id 1); a clockwise turn →
    3 queued; read 6 (id 2) → still 3; pump (the writer answers id 10); read 6 (id 5) → still 3;
    read 6 (id 11) → 6.
  - `a_written_value_is_kept_when_a_later_read_has_no_xmp_value` (a `.jpg` path): one clockwise
    turn, embedded 1 → 6, pump, `Ok` → confirmed 6 and embedded 6; then `read` with an empty map
    for the path → still shown and confirmed 6; a later read with a value 8 → shown 8.
  - `a_written_sidecar_value_is_taken_away_by_a_read_without_it` (an `.arw` path): the same
    turn and `Ok`, then a read with an empty map → shown `None`.
  - `a_read_without_a_value_takes_a_read_value_away`: read 6, then a read with an empty map →
    shown `None`, change `(path, None)`.
  - `prune_drops_shown_values_as_changes_to_none_and_keeps_pending_paths`: path A read 6, path B
    turned and queued, path C read 8; `prune` keeping nothing → changes `(A, None)` then
    `(C, None)` (sorted), B kept with its shown value.
  - `unsupported_files_are_skipped_and_only_unsupported_is_refused`: as the ratings' batch tests,
    with `RatingRefused::Unsupported` and nothing changed.
  - `the_reader_reads_a_missing_file_as_none`: as `sort.rs`'s
    `the_reader_reads_a_file_without_a_date_as_none` (its answer comes as `Ok(Some(..))`).
  - `rating_text.rs`: the three texts, singular and plural.
- **Not in this task:** keys, menu items, `MetadataCache`, `main.rs` and the view (T237); the
  zoom rule (T235); reading the embedded orientation in the folder's batch call (it would make
  ExifTool open every RAW; the reader asks only for images turned without a known value);
  turning pixels, mirror commands (ADR 0024 §5).
- **Rules:** `docs/tasks/README.md` rules 1–13. No new dependencies (`pw-viewcore`, `pw-raw` and
  `pw-metadata` are already dependencies of `pw-loupe`; check `Cargo.toml` and stop if one is
  not). No `.unwrap()`, `.expect()` or unchecked indexing outside tests.
- **Done when:** `.\tools\check.ps1` is all OK and `git status` shows only the files above
  changed.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Blocked | Added (chain T235 → T236 → T237, ADR 0024 O3); waits for T235 |
| 2026-11-20 | Blocked | Cold read: doable; every test number checked against `turned`; `turned` reachable, no new dependency. Fixed: the glossary terms moved here from T237 (rule 13); `pw_metadata`'s `RequestId`/`SubmitError`; `ROTATION_UNSUPPORTED_TEXT` shown by the caller; own `fail_write`, text visibility; `Debug` on the reader; a change recorded on `Ok`; `confirmed_by_write` and `embedded` updated only for files with their own metadata (a RAW's sidecar is read again), with a RAW test; prune sorted, waiting turns of a pruned path lost (accepted); duplicates turned once; failure reset; the test orders and numbers given |
| 2026-11-20 | Blocked | From T237's cold read: `is_waiting` and `waiting_paths` added (a move must not go ahead while a turn waits for its embedded value), with a test |
| 2026-11-20 | Blocked | Chain review (T235's log): `try_recv` tells a stopped reader apart (`ReaderStopped`); the reason `confirmed_by_write` must stay written down |
| 2026-11-20 | Open | T235 done; set to Open |
| 2026-11-20 | Ready for review | Agent: `orientations.rs` (new: `OrientationWriter`, `OrientationStore`, `EmbeddedOrientationReader`, `ReaderStopped`; 25 tests, beyond the list also an answer for an unknown path ignored, a path given twice, a stopped worker and `fail_sent`, no change on an `Ok` that leaves the shown value, values outside 1–8 on a read, the failure reset and count), `lib.rs` (module and re-exports, `ROTATION_UNSUPPORTED_TEXT`), `ratings.rs` (`absolute_path` `pub(crate)`), `rating_text.rs` (three rotation texts and a test), `docs/glossary.md` ("Rotate Left / Rotate Right", "rotation" in §4), `docs/test-inventory.md` (2042 passed, 75 ignored). check.ps1 all OK, no clippy warnings |
| 2026-11-20 | Done | Reviewed: the diff holds only the named files and matches points 1–7; `read`, `pump` and `fail_write` compared with `RatingStore`'s line by line (the same rule, plus `confirmed_by_write`); the reader's stopped and unstarted cases checked; nothing wired in `main.rs`, `metadata.rs` or `shell.rs`. The 8 tests beyond the list accepted. The reviewer's own clean run: `.\tools\check.ps1` all OK, 2042 passed, 75 ignored, no clippy warnings |
