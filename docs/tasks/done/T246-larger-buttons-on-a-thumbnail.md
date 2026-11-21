# T246. ADR 0017 U15: the buttons on a thumbnail are 28 points (pw-loupe) — Done

- **Why:** the owner's hand check of U15 (2026-11-21) passed, but the owner found the buttons'
  symbols too small and chose 28-point buttons in place of 20 (ADR 0017 §2.8 and U15-Q6, changed
  by the owner 2026-11-21; backlog entry "Larger buttons on a thumbnail"). The symbols are already
  drawn scaled from the disc's square (`rect.width() / 20.0` in `paint_rotate_symbol`,
  `paint_info_symbol` and `paint_magnifier_symbol`, `main.rs`), so a larger disc gives larger
  symbols with no change to the drawing.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/thumb_buttons.rs` (point 1 and its tests)
  - `docs/test-inventory.md` (the description of `pw-loupe`'s tests: "(a 78-point square, 30
    points between two discs)" becomes "(a 78-point square, 14 points between two discs)"; the
    counts do not change)
  - this task's own file (status and log)
- **What:**
  1. **`BUTTON_SIZE`** (`thumb_buttons.rs`): `20.0` → `28.0`. Its doc comment keeps "the same at
     every thumbnail size (U15-Q6)" and adds, in the agent's own words, that the owner changed it
     from 20 after the hand check of U15 (2026-11-21, T246). `BUTTON_INSET` stays 4. Nothing else in the file's code changes.
  2. **The tests** (`thumb_buttons.rs`): `the_buttons_do_not_overlap_at_the_smallest_and_the_largest_size`
     now gets a gap of 14 points at the smallest size (78 − 2 × 4 − 2 × 28): its comment ("a
     78-point square leaves 30 points between two discs") and its `assert_eq!(gap, 30.0)` become 14.
     This is the expected change of a value the owner changed, not a weakened test; every other
     assertion stays as it is (the four squares inside the image square and at least
     `BUTTON_INSET` apart at the smallest and the largest size). The other tests use
     `BUTTON_SIZE` and need no change; if one does, stop and report it.
  3. **`main.rs` is not changed.** The symbol functions' `/ 20.0` is the drawing's reference size,
     so at 28 points every symbol and stroke is 1.4 times as large. The contrast pairs do not
     depend on the size.
- **Tests to add:** none (point 2 changes the expected gap).
- **Not in this task:** a different inset, other symbols, a setting for the size, the CHANGELOG
  (the buttons are not released yet; T245's Added line does not name a size).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new texts. No `.unwrap()`, `.expect()`,
  `panic!`, `unreachable!` or unchecked indexing outside tests.
- **The owner's hand check (after this task):** at the smallest and the largest thumbnail size
  the four buttons are larger, fit in the corners with space between them and do not cover each
  other; the symbols are clear and centred on their discs; the hover and the clicks work as
  before (one click on each button).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer strikes the backlog entry "Larger buttons on a thumbnail".

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Open | Added (the owner's decision after the hand check of U15: 28-point buttons, ADR 0017 §2.8, U15-Q6) |
| 2026-11-21 | Open | Cold read: doable with the named files; the 14-point gap, the symbols scaling with the disc and staying inside it at 28, and nothing else depending on 20 confirmed. Changed: the test-inventory phrase named; the doc comment in the agent's own words |
| 2026-11-21 | Ready for review | Points 1–3 done: `BUTTON_SIZE` 20 → 28 with its doc comment (20 until the owner's hand check of U15, 2026-11-21, T246); the gap test's comment and `assert_eq!` 30 → 14, nothing else in the tests changed; `main.rs` not changed; the test-inventory phrase 30 → 14 points, counts unchanged. check.ps1 all OK, 2119 passed, 76 ignored, no clippy warnings |
| 2026-11-21 | Done | Reviewed (reviewer): as written; only `thumb_buttons.rs`, `test-inventory.md` and this file. The reviewer's clean run of check.ps1: all OK, 2119 passed, 76 ignored, no clippy warnings. Backlog entry struck. Moved to `done/`; the owner's hand check follows |
| 2026-11-21 | Done | Hand-checked by the owner: the 28-point buttons fit at the smallest and the largest thumbnail size, the symbols clear and centred, hover and clicks as before |
