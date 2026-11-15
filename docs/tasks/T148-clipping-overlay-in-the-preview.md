# T148. ADR 0017 U7b, third part: the clipping keys, menu items and section — Blocked

- **Blocked: waits for T147** (the map's state in the view core's frame), which waits for T146.
  The owner's answers to U7b-Q1–U7b-Q10 are in `docs/tasks/T146-clipping-map-of-raw-data.md`.
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
  `docs/test-inventory.md`.
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
