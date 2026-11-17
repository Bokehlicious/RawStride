# T178. ADR 0017 U9b: a move waits for its rating write's answer, and an image is not moved twice — Blocked: waits for its cold read

- **Chain:** T169 → … → T174, T177; this task comes after T174 and before the owner's hand check
  of U9b. Found in the independent review of T174 (2026-11-17).
- **Why:** three gaps in the moves as T172–T174 built them:
  1. A batch image waits only while its rating write is *queued* (`Batch::to_send` with
     `is_queued`). Once the write is sent, the move is sent too, whatever the write's answer.
     A reject write that fails is rolled back (`RatingStore::fail_write`), but the file still
     moves to `_rejected` without a reject (ADR 0017 §2.6: only a rejected image goes there).
  2. An image whose batch waits or runs stays in the tab until the batch is done, so a second
     key can target it again. Two quick X: the second clears the reject, its write leaves the
     queue, and the first batch then moves an image that is no longer rejected. Two quick P (or
     X then P): a second batch moves a path the first one has moved already, and the user sees
     "could not be moved".
  3. After an undo, the status line shows `undone_text` even when restoring a rating or
     listing a folder again failed; those errors are only in the Tasks list. `batch_done` shows
     the last such error in the same case.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/moves.rs`,
  `crates/pw-loupe/src/ratings.rs`, `crates/pw-loupe/src/metadata.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`, and this task's own file (status and
  log).
- **What:**
  1. **The batch knows its kind** (`moves.rs`): `Batch::new(kind: MoveKind, images: Vec<(PathBuf,
     PathBuf, Option<Rating>)>)` and `pub fn kind(&self) -> MoveKind`.
  2. **An image can be skipped** (`moves.rs`): `pub fn skip(&mut self, image: &Path)`: the image
     is not sent and needs no answer; `done` counts it as answered, and `finish` puts it in
     neither `moved` nor `failed`. An image not in the batch, or already sent or answered,
     changes nothing.
  3. **Which images a batch holds** (`moves.rs`): `pub fn contains(&self, image: &Path) ->
     bool`: the image is one of the batch's images, compared as given.
  4. **A write waiting for its answer** (`ratings.rs`): `pub fn is_pending(&self, path: &Path)
     -> bool`: a write for `path` is queued or sent and not yet answered (`queued` or `sent`).
     `is_queued` stays.
  5. **Through the cache** (`metadata.rs`): `MetadataCache::is_queued` is replaced by
     `pub(crate) fn is_pending(&self, path: &Path) -> bool`, calling the store's.
  6. **Sending** (`main.rs`, `send_moves`): `to_send` is given `is_pending` in place of
     `is_queued`, so an image waits until its rating write has its answer (the answers are
     applied in the metadata cache's `poll`, before `follow_moves`). Then, for a batch of kind
     `Reject`, an image whose `current_rating` is no longer rejected (`stars != Some(-1)`; its
     write failed and was rolled back, or a later key cleared the reject) is skipped with
     `skip` and not sent. Nothing is shown for a skipped image (a failed write already shows
     its own message). `Pick` batches are sent as today.
  7. **Not twice** (`main.rs`, `queue_moves`): an image held by a waiting batch or by the
     running batch (`contains`) is left out of a new batch; a batch left with no images is not
     queued. The calls of `Batch::new` pass the kind (`MoveKind::Reject` from `rate`,
     `MoveKind::Pick` from `move_picked`); `MoveImages` and `queue_moves` carry the kind as
     needed.
  8. **The undo's message** (`main.rs`, `undo_done`): when no image failed to move back but
     `errors` (from `restore_rating` or listing a folder) is not empty, the status line shows
     the last error, as `batch_done` does; otherwise as today.
- **Tests to add:**
  - `moves.rs`: `kind` gives the kind passed to `new`; `skip` of an image not sent makes the
    batch `done` once the others are answered, and `finish` lists it in neither list; `skip` of
    an image already sent changes nothing; `contains` is true for each image of the batch and
    false for another path.
  - `ratings.rs`: `is_pending` is true after `set` (queued), still true after `pump` (sent),
    and false after the write's answer (`written`), for success and for failure.
  - `metadata.rs`: `is_pending` follows the store: true after an X until the worker's answer
    is polled.
  - **Existing tests changed on purpose (README rule 5 lifted for these lines only):** in
    `moves.rs`, each `Batch::new(…)` gets the kind as its first argument (`MoveKind::Reject`
    where `restore` is `Some`, otherwise `MoveKind::Pick`); in `metadata.rs`,
    `is_queued_and_current_rating_follow_the_rating_store` uses `is_pending` in place of
    `is_queued`, with its comment and assertions changed so `is_pending` stays true after
    `poll` sends the write and becomes false once the answer is polled. Nothing else in them
    changes.
  - `main.rs` has no unit test (`Loupe` cannot be built in a test); the owner's hand check of
    U9b covers points 6–8.
- **Not in this task:** a tab open on `_rejected` or `_picked` itself, Ctrl+Z while a batch
  runs, and the undo's state in `moves.rs` (`backlog.md`, "Leftovers from the review of T174").
- **Rules:** AGENTS.md §3; no new texts.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added after the independent review of T174 (findings 1–3); waits for its cold read; the owner's hand check of U9b waits for it |
