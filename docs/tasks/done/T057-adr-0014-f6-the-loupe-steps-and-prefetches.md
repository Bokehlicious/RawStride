# T57. ADR 0014 F6: the loupe steps and prefetches within the filter — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Start after T54, T56 and T59 are done.** **For a Claude agent.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` for the
  status line.
- **Why:** ADR 0014 §2.2 (amendment 1), §2.3, §4.4 (way C), §5.3 (amendment 1), S2, S3.
- **What:**
  1. Loupe steps (Next, Previous, First, Last) go through `Tab::loupe_move` instead of
     `core.step`/`go(0)`/`last`. If `visible` changed, call `core.set_files(paths of visible,
     position)`; otherwise `core.go(position)`.
  2. A filter change while the loupe is shown: `refilter`, then `set_files`; if nothing is visible,
     return to the grid (§2.3).
  3. A rating key in the loupe: after `Tab::rated` (T59), if `visible` changed, `core.set_files(paths
     of visible, position)`, so the next visible image comes from the prefetched views; if nothing
     is visible, return to the grid (§2.3).
  4. The tab's current image is taken from the tab, not from `frame.pos`, while the loupe is shown
     (the core follows the tab).
  5. Status line: `status_line` gets the folder's total and uses `position_text`
     ("3 / 12 (av 31)") first, as in the grid (§5.3, amendment 1); update its test.
  6. Remove the interim `open_folder` of T56 point 9 and T59 point 3 for changes of the visible
     list; opening the loupe from the grid and a tab load still use `open_folder`.
- **Rules:** as T56.
- **Tests to add:** the `app.js` case "loupe within filter": Red, Home, Enter, step right → IMG_002
  then IMG_004 (on `Tab::loupe_move`); the status-line text with and without a filter.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: under "≥ 3 stjärnor"
  stepping stays in the filter and holding an arrow is smooth; under "Ej betygsatt" rating the shown
  image shows the next unrated image at once, without a cold load; `latency.ps1` under the filter for
  V1 (S3); a batch of 30 then closing the window at once keeps the values (ADR 0013 R4).
