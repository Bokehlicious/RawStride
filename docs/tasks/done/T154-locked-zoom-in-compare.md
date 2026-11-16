# T154. ADR 0017 U6, fifth part: locked zoom and position in the compare — Done

- **Can start now.** T153 (the compare open in the preview: `CompareOpen` in `Loupe` with
  `view_pending` and `mode_sent`, the two sides drawn by `draw_image_area`, the keys C, K, Shift+K
  and V), T152, T151 and T150 are Done. The owner's answers to U6-Q1–U6-Q12 are in
  `docs/tasks/done/T150-locked-view-in-the-view-core.md` and ADR 0017 §6.1. Chain: T150 → T151 →
  T152 → T153 → T154. The owner's hand check of U6 comes after this task.
- **Why:** ADR 0017 §2.2 and step U6, "locked zoom": with the lock on, the held image shows the
  same zoom level and the same point of the image as the current one, so the same detail is
  judged on both. T150 gave the view core `set_view` and `Frame::centre`, T151 the rule `follow`.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`
  (the key L in `preview_key_action`, and the test helper `command_of_key` taught it),
  `crates/pw-loupe/src/shell.rs` (one command, after T153's "Above and Below"; its menu-order
  test extended),
  `crates/pw-loupe/src/shell_text.rs` (its text); `docs/glossary.md`; `docs/test-inventory.md`.
- **What:**
  1. **L** and the View menu item "Lock Zoom and Position" (U6-Q8): the lock on ↔ off, remembered
     (`compare_locked`, T151), ticked when on, greyed outside the compare.
  2. **Locked:** every frame while comparing, after the current side's frame, T151's `follow`
     with the current side's `zoom` and `centre`; what it gives is sent to the second core with
     `set_view`. What was sent last is forgotten when the compare opens, the held image changes
     (K, Shift+K), the lock is turned on, or the held side's frame first gives its image's size
     (the size was not known when the view was sent, T150), so the held side takes the current
     side's view at once and again when its image is opened.
  3. **The mouse while locked:** a drag, the wheel and a double-click on **either** side act on
     the current side's core, so the held side follows through point 2. The anchor of a wheel or
     double-click on the held side is the same point of the current side's view (the same offset
     from the half's top left corner; the halves are equal). This keeps the pointed-at detail
     still only when the two images have the same size and orientation; otherwise it is near it.
     That is accepted (the lock follows the current image, U6-Q8). Unlocked, T153's rule stands
     (the side the mouse is on).
  4. **Keys:** the zoom keys (Z, F, +, −) act on the current side and, while locked, reach the held
     side through point 2; unlocked, on the current side only (as in T153).
  5. **Texts** (U6-Q12, rule 13): "Lock Zoom and Position" in `shell_text.rs`; in
     `docs/glossary.md`.
- **Tests to add:** `lib.rs`: L maps to its preview key, not with other modifiers; `command_of_key`
  knows it. `shell.rs`: the command follows Above and Below in the View menu with its key. The rest is T151's `follow` (tested there) and the
  owner's hand check.
- **Not in this task:** anything of T153's that is not named here; a lock of the display mode
  (R already acts on both, T153).
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency. Run
  `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` on a release build before and after (compare
  closed) and report V1; it must not be worse.
- **Done when:** `.\tools\check.ps1` is all OK, V1 is not worse, and `git diff --stat` shows only
  the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added with T150–T152 (the chain of U6) as T153; renamed T154 when the cold read split T152; "sent" forgotten when the held image's size becomes known, the anchor on the held side spelled out (near the detail when the images differ in size or orientation), the menu place and `command_of_key` named; waits for T153 |
| 2026-11-16 | Open | T153 Done |
| 2026-11-16 | Ready for review | Done by the agent. `lib.rs`: `PreviewKey::LockZoom` for L (no modifier) in `preview_key_action`, `command_of_key` taught it, one test (`l_is_the_lock_key_of_the_preview`). `shell.rs`: `Command::LockZoom` (`Where::Loupe`) after Above and Below in the View menu, the menu-order test extended (L in the compare's keys). `shell_text.rs`: "Lock Zoom and Position", its row in the compare's menu test and in the two lists of every menu item and key. `main.rs`: `CompareOpen` has `view_sent` (what was last given to the held side while locked) and `held_size_known`; `toggle_compare_lock` (L and the menu item; nothing outside the compare; turned on, `view_sent` forgotten); greyed outside the compare, ticked by `compare_locked`; each frame, after the current side's frame, `follow(compare_locked, (zoom, centre), view_sent)` and `set_view` on the second core with what it gives (T153's one-time start view kept for the unlocked case); `view_sent` forgotten at open, K, Shift+K, lock on, and when the held frame first gives its size (then a repaint is asked so it is sent at once); `draw_image_area` takes a `mouse_core`: the current side's core for both sides while locked (the anchor is the offset in the side's own half, so the same point of the current side's view), each side's own core unlocked. The zoom keys unchanged (current core; the held side follows through `follow`). `docs/glossary.md`: Lock Zoom and Position. `docs/test-inventory.md`: pw-loupe 804 tests, the lock's tests described. **Choices to confirm:** while locked, the zoom keys' anchor is still taken only when the pointer is on the current side (as T153), not mapped from the held side as the mouse is; `held_size_known` is set also when unlocked, but `view_sent` is forgotten for it only while locked; a held-side mouse action shows on the next frame (the current side's frame is taken before the held side's input). `check.ps1` all OK (1607 passed, 0 failed, 63 ignored; no clippy warning). `latency.ps1` (T152's protocol: A1 files TN006061–67, started on TN006061, release builds, compare closed, RIGHT × 6, 1.5 s apart, `results/latency-t154.csv`): V1 next image at fit, median 5.0 and 5.2 ms before, 5.4 and 5.6 ms after; four more runs alternating before/after gave 5.5, 5.5 before and 5.4, 5.4 after: not worse. The before build is kept as `target/release/pw-loupe-t154-before.exe`. Not checked by eye; the lock is for the owner's hand check of U6 |
| 2026-11-16 | Done | Reviewed: the diff does what points 1–5 ask (L and the menu item, remembered, ticked, greyed outside the compare; `follow` every frame after the current side's frame and `set_view` on the second core, what was sent forgotten at open, K, Shift+K, lock on and when the held size first becomes known; locked, the mouse on either side acts on the current core at the same offset in its half; the zoom keys on the current core); only the named files; V1 not worse (5.0–5.5 ms before, 5.4–5.6 ms after, six runs each, alternating). Accepted: the zoom keys' anchor only from the current side while locked; a held-side mouse action shown one frame later; `#[allow(clippy::too_many_arguments)]` on `draw_image_area` (eight arguments; four such allows already in the repository). No independent review (one crate, seven files). Nothing left over. `check.ps1` all OK (1607 passed, 0 failed, 63 ignored), newer than the last change; no clippy warning. Owner approved 2026-11-16; U6 awaits the owner's hand check |
| 2026-11-16 | Done | Checked by hand by the owner and approved, for the whole step U6 (T150–T154): opening and closing the compare (C, Escape), hold (K), swap (Shift+K), the layout (V), the lock (L) with zoom and pan on both sides, the side lines and frames, the clipping marks and R on both sides, the compare closing by itself, and stepping while comparing. No fix needed |
