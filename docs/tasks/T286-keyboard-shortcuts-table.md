# T286. The page Keyboard Shortcuts as a table of three columns — Open

- **Why:** ADR 0017 U8-Q15 (owner, 2026-11-23). Today each row of the page Keyboard Shortcuts is
  an action's name followed by its two slots in one `ui.horizontal_wrapped`, so where the keys start
  depends on the name's length, and a narrow window breaks a row in the middle. The rows do not line
  up. The owner chose a table of three columns, **Action**, **First Key** and **Second Key**, with
  the Action column's width changed by dragging and saved between sessions.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/shell.rs`
  (only the width function, its constants and its test), `crates/pw-loupe/src/lib.rs` (only its
  re-export), `crates/pw-loupe/src/shell_text.rs` (the three headings and their test),
  `crates/pw-loupe/src/settings.rs` (the saved width and its tests), `docs/glossary.md`,
  `docs/test-inventory.md`.
- **Background (read before starting):** `draw_key_rows` in `main.rs` (about line 8514: the groups
  of `key_page_groups`, each action a `horizontal_wrapped` row, a slot's key or "–" or "Press a key…",
  Change and Clear, the refusal under its row, `wait_hovered`); `draw_settings_window` (about lines
  1213–1460: the bottom panel with Restore Defaults, Save and Cancel, then one `ScrollArea` per page
  with its own `id_salt`, `settings-keyboard-shortcuts`); `save_settings_window` (about line 1123:
  only the theme and the companion suffixes are taken from the window's copy, the rest of
  `self.settings.shell` is kept); the side columns' remembered widths in `settings.rs`
  (`LEFT_COLUMN_WIDTH`, its parsing about line 467, `shell_lines` about line 664) and in `main.rs`
  (about line 7525, `self.settings.shell.left_column_width = …`); `column_width_range` and
  `clamp_column_width` in `shell.rs` for the shape of a pure width function with its test.
- **What:**
  1. **The table** (`draw_key_rows`): one row per action, three columns.
     - **Action:** the action's name on one line, cut with "…" when it is wider than the column
       (`egui::Label::truncate`, which in egui 0.37 already shows the whole name on hover:
       `show_tooltip_when_elided` is on by default, so no `on_hover_text` of its own).
     - **First Key** and **Second Key:** for slot 0 and slot 1 the key text as today (`key_text`,
       `SettingsText::NoKey` when empty, `SettingsText::PressKey` while its Change waits), then
       Change, then Clear (greyed without a key), on one line, never wrapped.
     - **Straight columns:** each key column has one width for every row: the widest key text the
       column shows on the page (every action's key in that slot of the `KeyMap`), "–" and "Press a
       key…" included, plus the widths of Change and Clear (each its text's width plus
       `spacing.button_padding.x * 2`) and the item spacing between the three, measured in the
       frame with `ui.painter().layout_no_wrap(..)` (or `ui.ctx().fonts_mut(..)` as `shell.rs`
       does; `Ui` has no `fonts`). These widths and the Action column's kept width (point 4) are
       worked out **before** the heading row and the rows are drawn, since the heading comes
       first; `draw_key_rows` gets them as parameters (its signature changes). The key
       text sits left in its space, so every Change and every Clear stands straight under the one
       above. Use fixed-width cells (`ui.allocate_ui_with_layout`, `ui.add_sized` or the like), not
       `horizontal_wrapped`, so a row never breaks; a window narrower than the table scrolls
       sideways instead (point 3).
     - The **groups' headings** (the menus' names, strong, as today) and a **refused key's reason**
       (warning colour, under its action's row, as today) span the table's whole width.
     - Clicks, `KeyClick`, `wait_hovered`, the waiting Change and the refusals work as today; only
       where things are drawn changes.
  2. **The heading row** (texts point 5): Action, First Key and Second Key above their columns, at
     the same x as the columns below, drawn **above** the page's scroll area (inside the window, under
     the page buttons' separator), so it stays at the top while the rows scroll; only on this page.
     Between Action and First Key a thin vertical line (`ui.separator()` style) that can be
     **dragged** (`ui.interact` on a few points wide rect with `Sense::drag()`, the cursor
     `CursorIcon::ResizeHorizontal` on hover and while dragging): the drag changes the Action
     column's width by the pointer's horizontal movement, kept by point 4's function. The heading
     row and the rows use the same widths in the same frame.
  3. **Sideways:** the page's scroll area becomes `ScrollArea::both()` (still
     `auto_shrink([false, false])` and `id_salt("settings-keyboard-shortcuts")`) only on this page,
     and the heading row scrolls sideways with it: the scroll area's horizontal offset from its
     output (`output.state.offset().x`; the field `offset` is private) is kept in a field of
     `Loupe` and used to shift the heading row in the next frame, or the heading is drawn in its
     own `ScrollArea::horizontal()` set to that offset with `horizontal_scroll_offset`. A heading
     that follows one frame late is accepted. The other pages keep `ScrollArea::vertical()`.
  4. **The width** (a pure function in `shell.rs`, re-exported from `lib.rs`, for example
     `key_action_width(width: f32, page_width: f32, keys_width: f32) -> f32`): constants
     `KEY_ACTION_WIDTH_DEFAULT = 220.0` and `KEY_ACTION_WIDTH_MIN = 120.0`; the result is `width`
     clamped between the minimum and `page_width − keys_width` (the room left by the two key
     columns), never below the minimum (the minimum wins when the page is too narrow; the page then
     scrolls sideways); a width that is not finite gives the default before clamping.
     `page_width` is the width available on the page **before** the `ScrollArea::both` is entered
     (inside it the content may grow sideways). It is stored as whole points in a new field
     `ShellSettings::key_action_width: u32` (default 220). The window's closure works on local
     copies (`chosen`, `page`, `keys`): the dragged width leaves it in a local and is written to
     `self.settings.shell` after the window, as `self.settings_page = page` is (not to the
     window's copy `chosen`, so `save_settings_window` keeps it and Cancel does not undo it, as
     the side columns' widths). `on_exit` then saves it with the rest of `self.settings`, so it
     survives a restart. In `settings.rs`: the key `key_action_width` (a `const` beside
     `PREVIEW_COLUMN_WIDTH`), parsed as a width in points like the side columns (an unreadable
     value is a problem and keeps the default); if it joins the arm `LEFT_COLUMN_WIDTH |
     RIGHT_COLUMN_WIDTH | PREVIEW_COLUMN_WIDTH`, it gets its own guard before that arm's last
     case, which sends every other key to `right_column_width`. Written in `shell_lines` as its
     own line right after `preview_hidden`'s, the last line of the first block.
  5. **Texts** (`docs/tasks/README.md` rule 13): three new `SettingsText` variants in
     `shell_text.rs`, "Action", "First Key" and "Second Key", made by `settings_text` like the page's
     other texts; no string literal in the drawing code. Add the three headings to
     `docs/glossary.md` §6 beside the rows of Keyboard Shortcuts and Change (the first key is the one
     the menus show, U8-Q2).
  6. Nothing else changes: the actions, groups and their order, the keys, Change, Clear, Restore
     Defaults, Save, Cancel, the key handling (`take_key_for_change`), `keys.txt`, and the other
     pages.
- **Tests to add:**
  - `key_action_width`: a width inside the range is kept; below the minimum gives the minimum;
    above `page_width − keys_width` gives that; a page too narrow for the minimum gives the minimum;
    NaN and infinity give the default (then clamped).
  - `settings.rs`: `key_action_width = 300` is read; a missing line gives 220; `wide` and `-5` are
    problems and keep 220 (added to
    `a_bad_value_of_each_shell_key_keeps_its_default_and_names_the_line`, as the side columns'
    cases); the value is written and read back (the round-trip test with `changed_shell()` and the
    exact full-file test extended with the new line).
  - `shell_text.rs`: the three headings' texts.
  - Say in the report what was only checked by hand (the straight columns, the drag, the cursor,
    the heading staying at the top, sideways scrolling, the cut name and its hover).
- **Hand check (the owner, after Done):** on Keyboard Shortcuts the keys, Change and Clear of every
  row stand in straight columns under Action, First Key and Second Key; dragging the line between
  Action and First Key makes the Action column wider and narrower, not below its minimum and not so
  wide that the key columns are pushed out of a wide window; a name too long is cut with "…" and
  shown whole on hover; scrolling down keeps the heading row at the top; in a narrow window the page
  scrolls sideways with the heading following; after Save, Cancel and a restart of RawStride the
  Action column has the width it was given; a Change still takes the next key, Clear and Restore
  Defaults still work, and a refused key's reason still shows under its row.
- **Not in this task:** sorting or filtering the actions; a search field; changing the key columns'
  width by hand; the other pages of the Settings window; the metadata window.
- **Rules:** `docs/tasks/README.md`; no new dependency (no `egui_extras`); no panic paths
  (`AGENTS.md` §3); English texts with the glossary's terms (`AGENTS.md` §7).
- **Done when:** `.\tools\check.ps1` is all OK, no clippy warnings, and `git diff --stat` shows
  only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Open | Written by the reviewer from the owner's choice (ADR 0017 U8-Q15: three columns, the Action column's width dragged and saved between sessions). Cold read: no other file needs to change (every other `ShellSettings` literal uses `..`; `on_exit` saves `self.settings`, so the width survives a restart); worked in: measuring with `ui.painter().layout_no_wrap` (`Ui` has no `fonts`) and the buttons' padding, the widths worked out before the heading and passed to `draw_key_rows`, `Label::truncate`'s own hover, the scroll offset through `output.state.offset()` (one frame late accepted), the width leaving the window's closure in a local, `page_width` taken before the scroll area, the parse arm's guard, the line's place in `shell_lines`, the bad-value test named |
