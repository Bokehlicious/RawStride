# T177. ADR 0017 U9b: the release before a move notes no write stamp — Blocked: waits for its cold read

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
  `crates/pw-imagecheck/src/lib.rs`, `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`,
  and this task's own file (status and log).
- **What:**
  1. **`ImageCheck::release_for_move(&self, image: &Path)`** (`safe_write.rs`), a provided method
     whose default calls `self.release(image)`. Its doc comment: closes every RAW engine session
     of `image` before a move (ADR 0017 §2.6); no write follows, so an implementation that notes
     something for a write's answer does not note it here. No other implementation in
     `pw-metadata` changes (`NoImageCheck` and the tests' checks use the default).
  2. **The worker** (`worker.rs`): `Request::Move` and `Request::MoveBack` call
     `release_for_move` in place of `release`, for the same files as today. Nothing else changes.
  3. **`RawImageCheck`** (`pw-imagecheck`): a second, optional closure for moves, set with
     `pub fn with_move_release(self, release: impl Fn(&Path) + Send + Sync + 'static) ->
     RawImageCheck`. `release_for_move` calls it when set, otherwise the `release` closure (as
     the default does); `release` and `check_copy` do not change. The `Debug` output shows
     whether it is set, as it does for `release`.
  4. **The program** (`main.rs`): `release_before_write` gives its check a move release that
     does what its release does (`release_core`, then `histogram_reads.wait_for(path,
     RELEASE_TIMEOUT)`) without `note_before_write`, whatever `note_stamp` is. `release_core` is
     shared by the two closures (for example in an `Arc`). `image_check` and its callers do not
     change.
- **Tests to add:**
  - `worker.rs`: the test check `RecordingCheck` records the paths of `release` and of
    `release_for_move` separately; T170's move tests (`move_image_moves_the_image_and_its_sidecar_and_releases_the_image`,
    `move_back_returns_the_files`, `the_image_is_released_even_when_its_extension_is_not_listed`)
    check the same paths in `release_for_move` and that `release` got none. Only those
    assertions change.
  - `pw-imagecheck`: `release_for_move` calls the move closure when set and not the release
    closure; without one it calls the release closure.
  - `main.rs` (next to `the_release_notes_the_stamp_before_the_write_when_asked_to`, which does
    not change): with `note_stamp` true, `release_for_move` releases the view core and notes
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
