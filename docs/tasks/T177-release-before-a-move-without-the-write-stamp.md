# T177. ADR 0017 U9b: the release before a move notes no write stamp — Open

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174; this task is done before T174, which
  sends the first moves. Found in the independent review of T170 (2026-11-17).
- **Why:** the metadata worker's moves (T170) close the view cores' sessions with
  `ImageCheck::release`. In the program, the check given to the worker (`image_check(..,
  note_stamp: true)` in `crates/pw-loupe/src/main.rs`, through `release_before_write`) also notes
  the file's stamp before a write (`HistogramReads::note_before_write`, U7c-Q7, T145), and only a
  write's answer takes it back (`take_before_write`). A move has no write answer, so the stamp
  stays: a later write of the same path takes this old stamp instead of its own, and a kept
  histogram of pixels that changed since is treated as current. The list also grows with every
  move and undo. A release before a move must close the sessions and wait for the histogram read
  as before a write, but note no stamp.
- **Crates:** `pw-metadata`, `pw-imagecheck`, `pw-loupe`. **Files:**
  `crates/pw-metadata/src/safe_write.rs`, `crates/pw-metadata/src/worker.rs`,
  `crates/pw-imagecheck/src/lib.rs`, `crates/pw-imagecheck/src/tests.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`,
  and this task's own file (status and log).
- **What:**
  1. **`ImageCheck::release_for_move(&self, image: &Path)`** (`safe_write.rs`), a provided method
     whose default calls `self.release(image)`. Its doc comment: closes every RAW engine session
     of `image` before a move (ADR 0017 §2.6); no write follows, so an implementation that notes
     something for a write's answer does not note it here. No other implementation in
     `pw-metadata` changes (`NoImageCheck` and the tests' checks use the default), nor `Timed` in
     `crates/pw-imagecheck/tests/real_files.rs` (it uses only `check_copy`).
  2. **The worker** (`worker.rs`): `Request::Move` and `Request::MoveBack` call
     `release_for_move` in place of `release`, for the same files as today. The module doc
     (`worker.rs`, the T170 paragraph near the top, which names `ImageCheck::release` for the
     moves) names `release_for_move`. Nothing else changes.
  3. **`RawImageCheck`** (`pw-imagecheck`): a second, optional closure for moves, set with
     `pub fn with_move_release(self, release: impl Fn(&Path) + Send + Sync + 'static) ->
     RawImageCheck`. `release_for_move` calls it when set, otherwise the `release` closure (as
     the default does); `release` and `check_copy` do not change. The `Debug` output shows
     whether it is set, as it does for `release`, as a field `move_release`. The module doc's
     list (`lib.rs`, the point "sessions closed first") also names `release_for_move`.
  4. **The program** (`main.rs`): `release_before_write` gives its check a move release that
     does what its release does (`release_core`, then `histogram_reads.wait_for(path,
     RELEASE_TIMEOUT)`) without `note_before_write`, whatever `note_stamp` is. `release_core` is
     shared by the two closures (for example in an `Arc`), and each closure gets its own clone of
     `histogram_reads`. `image_check` and its callers do not change.
- **Tests to add:**
  - `worker.rs`: the test check `RecordingCheck` overrides `release_for_move` and records its
    paths in a second list, read with a new `released_for_move()` beside `released()`.
    **Existing tests changed on purpose (README rule 5 lifted for these assertions only):** in
    T170's `move_image_moves_the_image_and_its_sidecar_and_releases_the_image`,
    `move_back_returns_the_files` and `the_image_is_released_even_when_its_extension_is_not_listed`,
    each `check.released()` assertion becomes `check.released_for_move()` with the same paths
    (for `move_back_returns_the_files` the image, then every `to`), plus
    `assert!(check.released().is_empty())`. Nothing else in them changes.
  - `pw-imagecheck` (`src/tests.rs`, next to `release_calls_the_closure_with_the_path`):
    `release_for_move` calls the move closure when set and not the release closure; without one
    it calls the release closure. **Existing test changed on purpose (rule 5 lifted for these
    strings only):** `without_release_does_nothing`'s two expected `Debug` strings gain the new
    field (`RawImageCheck { release: false, move_release: false }` and `RawImageCheck { release:
    true, move_release: false }`); nothing else in it changes.
  - `main.rs` (in `mod write_waits_for_the_histogram_read`, next to
    `the_release_notes_the_stamp_before_the_write_when_asked_to`, which does not change): with `note_stamp` true, `release_for_move` releases the view core and notes
    no stamp (`take_before_write` gives `None`), and a `release` afterwards still notes one.
- **Not in this task:** the shell's moves (T174).
- **Rules:** AGENTS.md §3. This changes more than one crate and the safe write's `ImageCheck`
  (`pw-metadata`), so it gets an independent code review before the owner's approval
  (`docs/tasks/README.md`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added after the independent review of T170 (finding: a move's release notes a histogram stamp no answer takes); waits for its cold read; T174 waits for it |
| 2026-11-17 | Open | Cold read: `pw-imagecheck`'s tests are in `src/tests.rs` (added to Files) and `without_release_does_nothing` checks the exact `Debug` strings (allowed to change, named); `RecordingCheck` overrides `release_for_move` with `released_for_move()`; the changed worker assertions named and allowed explicitly; the module docs of `worker.rs` and `pw-imagecheck` updated; one `histogram_reads` clone per closure; `Timed` unchanged; the main.rs test module named |
