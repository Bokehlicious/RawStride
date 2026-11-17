# T165. The compare after a step that did not happen — Done

- **Why:** from the review of T153 (backlog, 2026-11-16). Opening the compare (C) and swapping
  (Shift+K) change the compare's state first and then step with `loupe_move(LoupeMove::To(..))`
  (`open_compare` and `swap_images`, `main.rs`). That step can silently not happen: the refilter
  before the step (`refilter_before_move`, `browse.rs`) can drop the target, for example after a
  rolled-back rating write. Then the held image and the current image are the same, both sides
  show it, and only C or Escape leaves it. **Owner, 2026-11-17** (ADR 0017 U6-Q11): when the step
  does not happen, C leaves the compare closed and Shift+K leaves the held image as it was. A
  filmstrip click on the held image is a real step and stays as it is.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/compare.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`.
- **What:**
  1. In `compare.rs`, on `Compare`:
     `pub fn swap_target(&self, list: &[(usize, &Path)], current: usize) -> Option<(usize, PathBuf)>`:
     what `swap` would do, without changing anything. It gives the file index the current image
     moves to (the held image's) and the path that would become held (the current image's), with
     the same `None` cases as `swap`. `swap` is rewritten to call `swap_target` and then set
     `held`, so that its behaviour and its existing tests stay as they are.
  2. In `main.rs`, `open_compare`: step first, then open. After `compare_open_indexes` and
     reading the held path and the folder (as today), call `self.loupe_move(LoupeMove::To(current))`.
     Then only if the active tab's `current` equals that target: `hold_in_second_core(&held)` and
     set `self.compare = Some(CompareOpen { .. })` with the same fields as today. Otherwise nothing
     more happens, and the compare stays closed.
  3. In `main.rs`, `swap_images`: use `swap_target` in place of `swap`, which gives `to` and the
     new held path. Release the borrow of the tab and the compare, and call
     `self.loupe_move(LoupeMove::To(to))`. Then only if the active tab's `current` equals `to`:
     take a new borrow of `self.compare` (`let Some(open) = &mut self.compare else { return; }`;
     the one from before the step has ended), `open.compare.hold(&new_held)`, set
     `view_pending = true`, `view_sent = None`, `held_size_known = false`, and after the last use
     of `open` call `self.hold_in_second_core(&new_held)`. Otherwise the compare and the second
     core are left as they were.
  4. Update the doc comments of `open_compare` and `swap_images` to the new order (step first,
     then the compare changes only if the step happened). `swap`'s doc stays, with a pointer to
     `swap_target`.
  5. Nothing else changes: `loupe_move`, `browse.rs`, `hold_current_image`, the filmstrip click
     and `follow_compare` stay as they are.
- **Tests to add** (in `compare.rs`):
  - `swap_target` gives the same index as `swap` would, and the current image's path as the new
    held one, and leaves `held()` unchanged;
  - `swap_target` gives `None` in the three cases `swap` does (held image not in the list, held
    image is the current one, current not in the list), with `held()` unchanged.

  The order in `main.rs` (step, then change the compare) has no unit test: `Loupe` cannot be built
  in a test. The reviewer checks it in the diff.
- **Not in this task:** a filmstrip click on the held image (owner, 2026-11-17: a real step, left
  as it is); K (hold) has no step; changing `loupe_move` or `preview_move`.
- **Rules:** AGENTS.md §3; no new texts.
- **Hand check (owner):** C, K, Shift+K, V and L work as before (ADR 0017 U6). The rare case
  itself (a rolled-back rating write just before C or Shift+K) cannot be made to happen by hand.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added from the backlog item "The compare after a move that did not happen" (review of T153); the owner's decision is in ADR 0017 U6-Q11 |
| 2026-11-17 | Open | Cold read: doable with the named files; "current equals the target" is a sound test (the old current never equals it), `follow_compare` and `loupe_follows_visible` do not touch the compare inside `loupe_move`. Changed: point 3 names the new borrow of `self.compare` after the step; new point 4 updates the doc comments. Noted, unchanged as today: a refilter that drops the held image but not the target opens the compare with the held image outside the stepped list (Shift+K then does nothing, `still_valid` keeps it open) |
| 2026-11-17 | Ready for review | `swap_target` added and `swap` calls it; `open_compare` and `swap_images` step first and change the compare only when the active tab's `current` equals the target; doc comments updated; two tests in `compare.rs`. `check.ps1` all OK (1696 passed, 0 failed, 65 ignored) |
| 2026-11-17 | Done | Reviewed: the diff does points 1–5 (`swap_target` without changes, `swap` through it; `open_compare` and `swap_images` step first and change the compare only when the tab's `current` is the target, with a new borrow after the step; doc comments updated); no bugs. The order in `main.rs` has no unit test (`Loupe` cannot be built in a test), checked in the diff. Reviewer's `.\tools\check.ps1` on this tree (T165 and T166): all OK, 1700 passed, 65 ignored, 0 failed; clippy without warnings; the inventory's header set to this count. Nothing left over. The owner's hand check (C, K, Shift+K, V, L as before) after the commit |
| 2026-11-17 | Done | The owner's hand check approved |
