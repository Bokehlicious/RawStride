# T148. ADR 0017 U7b, third part: the clipping keys, menu items and section — Done

- **Can start now.** T147 (the map's state in the view core's frame, `Frame::clipping`,
  `ViewCore::set_clipping_wanted`) and T146 are Done. The owner's answers to U7b-Q1–U7b-Q10 are in `docs/tasks/done/T146-clipping-map-of-raw-data.md`.
  Chain: T146 → T147 → T148 → T149.
- **Why:** ADR 0017 §2.6, step U7b: the clipped highlights and the deepest shadows marked on the
  image in the preview, each with its own key. This task gives the user the two on/off choices
  (keys, menu items, check boxes), remembers them, tells the view core when a map is wanted, and
  shows the map's state in a section of the right column. T149 draws the marks on the image.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/shell.rs` (two commands with their keys
  in the View menu), `crates/pw-loupe/src/shell_text.rs` (the menu items' texts),
  `crates/pw-loupe/src/lib.rs` (the keys J and Shift+J in the preview's key mapping, and the
  menu-key test helper `command_of_key` taught Shift chords), `crates/pw-loupe/src/preview_text.rs`
  (the section's texts), `crates/pw-loupe/src/settings.rs` (the remembered states),
  `crates/pw-loupe/src/main.rs` (only: the two commands in `run_command` and `command_checked`,
  the keys in the key dispatch, the wish given to the view core, the frame's clipping state kept
  for the column, and the section in `draw_preview_column`); `docs/glossary.md`;
  `docs/test-inventory.md`; `crates/pw-libraw/tests/statistics.rs` (only point 6).
- **What:**
  1. **Two commands** (U7b-Q5): "Show Clipped Highlights" with **J** and "Show Deep Shadows" with
     **Shift+J**, in the View menu after the zoom items, ticked when on (`command_checked`, as
     the menu's other on/off items), their keys shown as the menu shows keys. Each turns its
     overlay on or off. They act in the preview (`Where::Loupe`); the marks are drawn only there
     (U7b-Q6). J without Shift must not fire on Shift+J (today's `key_action` ignores Shift: the
     mapping for these two keys must look at it).
  2. **Remembered** (U7b-Q6): `clip_highlights` and `clip_shadows` (on/off) in `settings.txt`,
     both off by default; they stay as they are when stepping to another image.
  3. **The wish:** every frame the shell tells the view core (T147's setter) that a map is wanted
     exactly when the preview is shown and at least one overlay is on.
  4. **The section "Clipping"** (U7b-Q9) in the preview's right column, under Histogram,
     collapsible and remembered (`preview_clipping` in `settings.txt`, open by default): two check
     boxes "Clipped highlights (J)" and "Deep shadows (Shift+J)" that call the same functions as
     the keys (after a click no widget keeps the keyboard, as the Zoom section's buttons); under
     them, from the last frame's clipping state (T147), "Reading …" while the map is being made
     and "No clipping map for this file" when there is none; nothing when both are off or the map
     is ready.
  5. **Texts** (U7b-Q10, rule 13): Clipping, Clipped highlights (J), Deep shadows (Shift+J), Show
     Clipped Highlights, Show Deep Shadows, No clipping map for this file; each from a function in
     `preview_text.rs` (the section) or `shell_text.rs` (the menu); the new terms in
     `docs/glossary.md` (Deep shadows is already there, T142's row; add to it, do not rewrite it).
  6. **A clippy warning left by T146** (from T147's review): `cargo clippy` warns
     `needless_range_loop` in `crates/pw-libraw/tests/statistics.rs`, in the X-Trans test
     (`for c in 0..3` indexing `clipped_cells`). Rewrite that loop so the warning goes, without
     changing what the test checks; nothing else in that file. This is the one file outside
     `pw-loupe` this task changes.
- **Tests to add:** `settings.rs`: the three new keys saved and loaded, their defaults, an unknown
  value ignored (and the test that lists every key); `shell.rs` / `lib.rs`: the two commands have
  J and Shift+J, no other menu item has those keys, J does not give the Shift+J command and Shift+J
  not the J command, and the every-menu-key test covers them; the texts through their functions.
- **Not in this task:** drawing the marks (T149); a colour per channel; marks in the contact sheet
  or the inspector; changes to `pw-viewcore` or `pw-libraw`.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added with T146, T147 and T149 (the chain of U7b); waits for T147 |
| 2026-11-15 | Blocked | Cold read (fresh agent): too big as first written (keys, menu, settings, GPU drawing and a section); split: the drawing went to T149. `lib.rs` (key mapping, `command_of_key` and Shift), `run_command`, `command_checked` and the key dispatch named; "ticked when on" as the menu already does; `Where::Loupe`; the texts follow the Histogram section's "Deep shadows" |
| 2026-11-15 | Open | T147 is Done; point 6 (T146's clippy warning) added from T147's review |
| 2026-11-15 | Ready for review | Points 1–6 done in the files named. `Command::ClipHighlights` (J) and `Command::ClipShadows` (Shift+J) in the View menu after "Zoom at Pointer", `Where::Loupe`, ticked from the settings; the keys are `PreviewKey::ClipHighlights` / `ClipShadows` in `preview_key_action` (exactly those modifiers), and `command_of_key` asks it before the loupe's keys. `clip_highlights`, `clip_shadows` (off) and `preview_clipping` (open) in `settings.txt`. `wish_clipping_map` calls `set_clipping_wanted` every frame after the keys; `last_clipping` keeps the frame's state. The Clipping section under Histogram: two check boxes calling the keys' functions, then `clipping_state_text` ("Reading …" is the Histogram section's text, reused; "No clipping map for this file"; nothing when both are off, not wanted or ready). Texts in `preview_text.rs` (`ClippingText`) and `command_text`; one new glossary row (T142's row not changed). Point 6: the X-Trans loop goes over `clipped_cells.iter_mut().enumerate()`. Existing tests extended, not weakened: the two lists of every menu item and key in `shell_text.rs`, the every-key file text and `changed_shell` in `settings.rs`. 10 new tests. `.\tools\check.ps1` all OK: 1550 passed, 63 ignored, 0 failed; clippy without warnings. Not tested automatically (hand check): the section's drawing, the check boxes, the wish and the menu ticks in `main.rs` |
| 2026-11-16 | Done | Reviewed: the reviewer's reading and an independent review agent (the task touched a `pw-libraw` test too) found no bugs; `check.ps1` all OK (1550 passed, 0 failed, 63 ignored), clippy without warnings. Approved by the owner, with the glossary's new terms in a row of their own (T142's row unchanged). Leftovers, each with a home: the section's state line can show the previous image's state for one frame (it is drawn before the frame; it corrects itself on the next repaint): accepted, and T149 draws the marks only from the current frame's state (T149 point 4); the menu's ticks are seen only in the contact sheet's menu, greyed, as Fit and Zoom: as designed. Noted only: `command_of_key` in the test asks the preview keys after the shell keys, `main.rs` before them (the same result while no other key uses J, which the test asserts). The drawing of the section, the check boxes, the wish and the menu ticks are checked by hand after T149. Next: T149 |
