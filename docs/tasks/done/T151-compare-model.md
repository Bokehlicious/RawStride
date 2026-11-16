# T151. ADR 0017 U6, second part: the compare model, its settings and texts — Done

- **Can start now.** T150 (`ViewCore::set_view`, `Frame::centre`) is Done. The owner's answers to
  U6-Q1–U6-Q12 are in `docs/tasks/done/T150-locked-view-in-the-view-core.md` and ADR 0017 §6.1. Chain: T150 → T151 → T152 → T153 → T154.
- **Why:** ADR 0017 §2.2 and step U6. The rules of comparing (which images, which side, hold,
  swap, layout, when it closes, what the locked side is sent) are plain Rust that can be tested
  without a window (GR11), as `preview.rs` holds the filmstrip's rules. T153 and T154 draw and
  wire them; this task decides nothing on the screen.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/compare.rs` (new, with unit tests),
  `crates/pw-loupe/src/lib.rs` (only: `mod compare;` and its public items re-exported as the
  other modules' are), `crates/pw-loupe/src/settings.rs` (two remembered states; the test of the exact saved text
  extended with their two lines, no other existing test changed),
  `crates/pw-loupe/src/loupe_text.rs` (the side line); `docs/glossary.md`;
  `docs/test-inventory.md`.
- **What:**
  1. **`Layout`**: `SideBySide` (the default) or `AboveBelow` (U6-Q7), and **`sides(area, layout)
     -> (held, current)`**: the image area split in two equal halves, held left or on top, current
     right or below (U6-Q5), with no gap and no overlap; the two halves together are the area. An
     area of zero or negative size gives two empty rectangles. (`egui::Rect`, as `lib.rs` already
     uses egui's types.)
  2. **The stepped list:** in this task, "the shown list" is the list the preview steps through
     (`browse.rs`'s `LoupeMove`): the tab's visible images, or with "only the marked" on the
     marked ones among them, in that order; a position is a place in that list, not a file
     index. The functions below take that list as file indexes (`&[usize]`) and the current
     file index, and give file indexes, so T153 can move through the tab with
     `LoupeMove::To(index)` as a filmstrip click does.
  3. **Opening** (U6-Q4): `open_indexes(list: &[usize], current: usize) -> Option<(usize,
     usize)>`: the held and the new current file index: the current one is held and
     the next one becomes current, or the previous one when the current is the last; `None` when
     the list has fewer than two images or `current` is not in it. (With "only the marked" on, the stepped list is the marked
     images, so two marked images give those two; no rule of its own.)
  4. **`Compare`**, the state while comparing: the held image's path (`PathBuf`). Functions:
     - **hold** (U6-Q6, K): the held path becomes the current image's path;
     - **swap** (U6-Q6, Shift+K): given the stepped list (file indexes with their paths) and the
       current file index, the file index the current image moves to (the held image's) and the
       new held path (the old current one); `None`, and nothing changed, when the held image is not in the
       stepped list (a filter or "only the marked" left it out) or is the current image;
     - **still valid**: whether the compare stays open after the tab's list of files changed: it
       closes when the held path is no longer among the tab's files (all files, not only the
       shown ones; U6-Q11). A held image that the filter leaves out stays held.
  5. **The lock** (U6-Q8), for T154: `follow(locked, current: (Zoom, [f64; 2]), sent:
     Option<(Zoom, [f64; 2])>) -> Option<(Zoom, [f64; 2])>`: what to send the held side's core
     (T150's `set_view`) this frame: nothing when unlocked or when it equals what was sent last;
     otherwise the current side's zoom and centre. Comparing for "equal" uses the values as they
     are (no rounding), so a drag sends every change.
  6. **Remembered** (U6-Q7, U6-Q8): `compare_above_below` (off by default: side by side) and
     `compare_locked` (on by default) in `settings.txt`, read and written as the other on/off
     states (`clip_highlights` is the pattern; a bad value keeps the default and is reported as
     theirs are). The compare itself is not remembered (U6-Q11).
  7. **The side line** (U6-Q10, rule 13): `compare_side_text(frame: &Frame) -> String` in
     `loupe_text.rs`, made whole in that one function, not joined from `source_text` and
     `zoom_text`: one `format!` template per source, each with the file name (not the folder) and
     the zoom in percent filled in, for example "{file} · RAW rendering · {zoom}%", "{file} ·
     Embedded JPEG · {zoom}%", "{file} · JPEG file · {zoom}%", "{file} · TIFF file · {zoom}%"
     (the source told apart as `source_text` tells it, G1, but without the pixel size), and the
     same with "(Fit)" after the percent at fit; with no path or no scale yet, a template without
     the missing part.
  8. **Glossary** (U6-Q12): Compare, Held image; with the source in the ADR (ADR 0017 §2.2, U6).
- **Tests to add:** in `compare.rs`: `sides` for both layouts (held left/top, current right/below,
  halves equal, together the area), and an empty area; `open_indexes` in the middle, at the end
  (the previous one), with two images, with one and none (`None`), with `current` not in the list,
  and on a list of marked file indexes with gaps (`[2, 7]` gives held 2, current 7);
  hold; swap within the list (file indexes and paths), with the held image filtered out and with the
  held image the current one (`None`, unchanged); still valid when the held file is filtered out
  but in the tab, not valid when it left the tab; `follow` unlocked (nothing), locked the first
  time (the current view), locked unchanged (nothing), locked after a pan (the new centre) and a
  zoom (the new zoom). In `settings.rs`: the two states read, written and defaulted, and an
  a bad value kept at the default and reported as for the others. In `loupe_text.rs`: the side line for each of the four
  sources, at fit and at 100 %, and without a path.
- **Not in this task:** anything drawn, the keys and menu items, the second view core (T152), the
  compare drawn (T153), the lock applied (T154).
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Blocked | Added with T150, T152, T153 (the chain of U6); waits for T150 and U6-Q1–U6-Q12 |
| 2026-11-16 | Blocked | Cold read (with T150): the stepped list defined (visible, or the marked among them) and the functions given file indexes (`open_indexes`); a bad settings value is reported, not ignored; the saved-text test named; the side line made whole in one function, without the pixel size (rule 13); T153 and T154 renamed with the split of T152 |
| 2026-11-16 | Open | T150 Done |
| 2026-11-16 | Ready for review | `compare.rs` (new: `Layout`, `sides`, `open_indexes`, `Compare` with hold, swap and still valid, `follow`), re-exported from `lib.rs`; `compare_above_below` and `compare_locked` in `settings.rs`; `compare_side_text` in `loupe_text.rs`; glossary (Compare, Held image) and test inventory. 34 new tests (24 + 5 + 5). `.\tools\check.ps1` all OK: 1603 passed, 63 ignored, 0 failed; clippy without warnings. Noted for the review: the test helper `changed_shell()` in `settings.rs` got the two new fields (it lists every field, so it does not compile without them); `swap` changes the held path itself and gives the file index, and also gives `None` when the current file index is not in the list; `sides` gives empty rectangles for an area that is not finite too |
| 2026-11-16 | Done | Reviewed: the diff does what the task asks (`sides` two equal halves without gap or overlap, held left/top; `open_indexes` on file indexes, the marked `[2, 7]`; hold, swap, still valid on all the tab's files; `follow` without rounding; the two states as `clip_highlights`, a bad value reported; the side line one whole template per case, without the pixel size, as U6-Q12); only the named files; `check.ps1` all OK (1603 passed, 0 failed, 63 ignored), newer than the last change; no clippy warning. Accepted: the test helper `changed_shell()` got the two fields (needed to compile, no test weakened); `swap` also `None` when the current index is not in the list; `sides` empty for an area that is not finite. Nothing left over. Owner approved 2026-11-16 |
