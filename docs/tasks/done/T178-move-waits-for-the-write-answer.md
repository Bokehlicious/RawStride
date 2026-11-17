# T178. ADR 0017 U9b: a move waits for its rating write's answer, and an image is not moved twice — Done

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
     is not sent and needs no answer. `BatchImage` gets a `skipped: bool`; `to_send`, `sent` and
     `answered` ignore a skipped image, `done` counts it as answered, and `finish` puts it in
     neither `moved` nor `failed`. An image not in the batch, or already sent or answered,
     changes nothing.
  3. **Which images a batch holds** (`moves.rs`): `pub fn contains(&self, image: &Path) ->
     bool`: the image is one of the batch's images and is not skipped, compared as given. A
     skipped image is no longer held: a later X that rejects it again queues a new batch for it
     (review 2026-11-17). An image whose move failed stays held (its answer may be a JPG that
     its RAW partner moved along).
  4. **A write waiting for its answer** (`ratings.rs`): `pub fn is_pending(&self, path: &Path)
     -> bool`: a write for `path` is queued or sent and not yet answered (`queued` or `sent`).
     `is_queued` stays.
  5. **Through the cache** (`metadata.rs`): `MetadataCache::is_queued` is replaced by
     `pub(crate) fn is_pending(&self, path: &Path) -> bool`, calling the store's.
  6. **Sending** (`main.rs`, `send_moves`): `to_send` is given `is_pending` in place of
     `is_queued`, so an image waits until its rating write has its answer (the answers, and a
     failed write's rollback, are applied in the metadata cache's `poll`, before `follow_moves`).
     Then, for a batch of kind `Reject`, each image `to_send` gives whose `current_rating` is no longer rejected (`stars != Some(-1)`; its
     write failed and was rolled back, or a later key cleared the reject) is skipped with
     `skip` and not sent. Nothing is shown for a skipped image (a failed write already shows
     its own message). `Pick` batches are sent as today.
  7. **Not twice** (`main.rs`, `queue_moves`): an image held by a waiting batch or by the
     running batch (`contains`) is left out of a new batch; a batch left with no images is not
     queued. The calls of `Batch::new` pass the kind (`MoveKind::Reject` from `rate`,
     `MoveKind::Pick` from `move_picked`); `MoveImages` and `queue_moves` carry the kind as
     needed. A running undo is not checked: its images are in no tab outside `_rejected` and
     `_picked`, where `target_folder` gives `None`.
  8. **The undo's message** (`main.rs`, `undo_done`): when no image failed to move back but
     `errors` (from `restore_rating` or listing a folder) is not empty, the status line shows
     the last error, as `batch_done` does; otherwise as today.
- **Tests to add:**
  - `moves.rs`: `kind` gives the kind passed to `new`; `skip` of an image not sent makes the
    batch `done` once the others are answered, and `finish` lists it in neither list; `skip` of
    an image already sent changes nothing; `contains` is true for each image of the batch and
    false for another path; `contains` is false for a skipped image and still true for the
    batch's other images, and true for an image whose answer failed.
  - `ratings.rs`: `is_pending` is true after `set` (queued), still true after `pump` (sent),
    and false after the write's answer (`written`), for success and for failure.
  - `metadata.rs`: `is_pending` follows the store: true after an X until the worker's answer
    is polled.
  - **Existing tests changed on purpose (README rule 5 lifted for these lines only):**
    - `moves.rs`: each `Batch::new(…)` gets the kind as its first argument, `MoveKind::Reject`
      where the target is `_rejected` and `MoveKind::Pick` where it is `_picked`: Reject in
      `to_send_waits_for_a_queued_write_and_never_gives_a_sent_image_again` and
      `finish_sorts_the_answers_and_drops_a_partner_moved_by_another_image`, Pick in
      `a_failed_image_is_not_dropped_for_its_own_moves`.
    - `metadata.rs`: `is_queued_and_current_rating_follow_the_rating_store` is renamed
      `is_pending_and_current_rating_follow_the_rating_store`; it sets `worker.write_result =
      Some(Ok(()))` first (so each write's answer is queued when it is sent), and asserts with
      `is_pending`: true after each `set_rating`, still true after the `poll` that sends the
      write, false after the next `poll` that applies its answer; the `current_rating`
      assertions and `write_attempts` stay.
    - `metadata.rs`: `restore_rating_queues_the_rating` uses `is_pending` in place of
      `is_queued`; after its `poll` (no `write_result`, so no answer) it asserts `is_pending` is
      still true in place of `!is_queued`.

    Nothing else in them changes.
  - No test of `Loupe` (it cannot be built in a test); the owner's hand check of U9b covers
    points 6–8.
- **Not in this task:** a tab open on `_rejected` or `_picked` itself, Ctrl+Z while a batch
  runs, and the undo's state in `moves.rs` (`backlog.md`, "Leftovers from the review of T174").
  Also left (`backlog.md`, the same item): P then X within a frame (the image is held by the
  Pick batch, so it goes to `_picked` though rejected); a RAW skipped while its JPG partner is
  sent (the JPG's move takes the RAW along; `finish` lists the RAW in neither list); a write
  whose answer never comes keeps its image's move waiting; a second P or X on an image whose
  move failed in the running batch is left out until that batch is done.
- **Rules:** AGENTS.md §3; no new texts.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added after the independent review of T174 (findings 1–3); waits for its cold read; the owner's hand check of U9b waits for it |
| 2026-11-17 | Open | Cold read: `skip` needs a `skipped` field (named); the kind in `moves.rs`'s tests by target folder, the three tests named; the two `metadata.rs` tests that use `is_queued` named with their exact changes (the second was missing); `is_pending` and the order in a frame checked against the code; a running undo needs no `contains`; P then X, a skipped RAW with a sent JPG partner and a write never answered noted as left; `main.rs` has tests, but none of `Loupe` |
| 2026-11-17 | Ready for review | Points 1–8 done in `moves.rs`, `ratings.rs`, `metadata.rs`, `main.rs`; 6 tests added (4 in `moves.rs`, 1 in `ratings.rs`, 1 in `metadata.rs`: a failed X pending until its answer is polled, then rolled back) and the named existing tests changed as listed; `docs/test-inventory.md` 1792 passed. `.\tools\check.ps1` all OK, no clippy warnings. No test of `Loupe`: points 6–8 wait for the owner's hand check |
| 2026-11-17 | Open | Review: points 1–8 and the tests as written; `.\tools\check.ps1` run again by the reviewer, all OK (1792 passed, 66 ignored, 0 failed, no clippy warnings). Sent back for a gap in point 3 (the brief's, not the agent's): `contains` held a skipped image, so an X that rejects it again while its batch still runs was left out, and the image stayed rejected but unmoved. Point 3 now leaves a skipped image out of `contains`, with a test added under **Tests to add**; a retry of a failed move while its batch runs noted as left (`backlog.md`). Left to do: only that change and its test; the rest of the diff stays |
| 2026-11-17 | Ready for review | Point 3 after the review: `contains` leaves a skipped image out, a failed one stays held; test `contains_is_false_for_a_skipped_image_and_true_for_a_failed_one` added; `docs/test-inventory.md` 1793 passed. `.\tools\check.ps1` all OK (1793 passed, 66 ignored, 0 failed), no clippy warnings |
| 2026-11-17 | Done | Second review: point 3 as changed (`contains` leaves a skipped image out, a failed one stays held) and its test; `.\tools\check.ps1` run again by the reviewer, all OK (1793 passed, 66 ignored, 0 failed, no clippy warnings). Approved by the owner. No owner decision to record (the change to point 3 is the review's, in this file). Left: in `backlog.md`, "Leftovers from the review of T174" (*Left by T178*, with the retry of a failed move from this review). Points 6–8 wait for the owner's hand check of U9b (`plans/adr-0017-interface-steps.md`) |
