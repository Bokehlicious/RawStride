# T281. The Settings page Keyboard Shortcuts and the companion suffixes' field — Blocked: waits for T280

- **Why:** ADR 0017 §2.4, step U8 (ADR 0017 §6.4, owner 2026-11-22): the key map is changed on a
  page of the Settings window (U8-Q7), and the companion suffixes get a field on the General page
  (U8-Q10, settling U9-Q10 and U10b-Q8). The last task of U8; the owner's hand check follows.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/shell.rs` (`SettingsPage`), `crates/pw-loupe/src/shell_text.rs`
  (`SettingsText`, the action names), `crates/pw-loupe/src/settings.rs`
  (`parse_companion_suffixes` made `pub`), `crates/pw-loupe/src/lib.rs`, `docs/glossary.md`,
  `docs/test-inventory.md`.
- **Background (read before starting):** T279's and T280's files; `draw_settings_window` (about
  lines 1079–1257: the pages, `chosen`, the clicks applied after drawing, Save and Cancel),
  `open_settings_window`, `save_settings_window` (it keeps **only the theme** of `chosen.shell`;
  `companion_suffixes` is in `ShellSettings`, so it must be taken over explicitly), `draw_layout_rows`
  and `LayoutClick` as the model for a page's rows; `settings.rs`'s `parse_companion_suffixes` and
  the `", "` join; the key loop in `main.rs` (after T280 no map action or fixed key acts while the
  Settings window is open, but the events still reach the loop, the library's F5 check and focused
  buttons). The frame order in `ui()` (about
  line 10263): `draw_columns` (the library's F5 read straight from the input, about line 7077),
  then `draw` with the key loop, then `draw_settings_window`; the Settings window is drawn after
  the key loop. `draw_metadata_window` (about lines 3775–3800) takes keys out of the input with
  `ctx.input_mut(|i| i.events.retain(..))`, the model for point 3.
- **What:**
  1. **`SettingsPage::KeyboardShortcuts`**, after Metadata Window in `SettingsPage::ALL`, named
     "Keyboard Shortcuts" (`SettingsText::Page`).
  2. **The page** (U8-Q7): the actions of `KeyAction::ALL` under their group's heading (File, Edit,
     Image, View; the menus' own names), each row: the action's name (the menu text where it has a
     command, `command_text`; "Marked only" and "Open in First Program" (or the glossary's name)
     from `shell_text.rs`; the group headings from `menu_text(MenuKind)` through a mapping from
     `KeyGroup`), its two keys as `key_text` (an empty slot shown as "–"), and for each slot
     **Change** and **Clear**. The window is 440 points wide: a row wraps
     (`horizontal_wrapped`, as `draw_layout_rows`). In a scroll area like the Metadata Window
     page's (the same `layout_height`, its own `id_salt`), then **Restore Defaults** (the whole
     map). Clicks are collected and applied after drawing, as `LayoutClick`.
     **State** (new `Loupe` fields, set in `open_settings_window`, cleared on Save and Cancel):
     the map's copy (`Option<KeyMap>`), the waiting Change (action and slot), the last refusal
     (its row and its reason), the suffixes' text. `keys.txt`'s path is held beside
     `settings_path` (a `keys_path: Option<PathBuf>` field, set at start; add it here if T280 did
     not). **Save** puts the copy in use (`self.key_map`) and, **only when the copy differs from the
     map in use**, writes it with `save_key_map` (so a Save of the theme alone never rewrites a
     hand-written `keys.txt`), in the Save branch beside `save_settings_window` (whose signature
     takes only `Settings`). Without a
     path the reason is `SettingsText::NoFolder`; a failed write shows its own warning ("The
     keyboard shortcuts could not be saved: {error}", a `String` function in `shell_text.rs`),
     and the map is still used until RawStride closes. When both files fail, two warnings are
     shown. **Cancel** drops the copy.
  3. **Change:** the row shows "Press a key…" and the **next key press** is taken for it **at the
     top of `ui()`, before anything is drawn**, with `ctx.input_mut(|i| i.events.retain(..))` as
     `draw_metadata_window` does: while a Change waits, every `Event::Key` is removed from the input,
     so neither the library's F5, the key loop nor a focused button (Enter or Space would click
     the Change button again) sees it. The first press (not a repeat) of a key that is not a
     modifier itself (`is_modifier_key`, T279; egui reports Shift, Ctrl, Alt and the Windows key
     as keys of their own, and such a press is removed and skipped) is used: the chord built as
     the key loop builds it (`ctrl = modifiers.ctrl || modifiers.command`); Escape cancels; any
     other chord is tried with `can_set` on the copy. Refused: the row says why ("Ctrl+Alt is not
     allowed", "This key cannot be changed", "Used by {action}", U8-Q4; `Modifier`, which the
     skipped press never reaches, "Shift, Ctrl or Alt alone is not allowed"; a `String` function
     `key_refusal_text(KeyRefusal)` with `format!`, rule 13), and the copy is unchanged; a new
     Change or a successful key clears the refusal. egui moves the focus on Tab and drops it on
     Escape before `ui()` runs; that cannot be stopped and does no harm (Tab is refused as fixed).
     **Cancelled** also by a pointer press anywhere this frame other than on the waiting slot's
     Change (a second click on that Change cancels it too), by a page change, by Save (which then
     saves the copy without the waiting Change) and by Cancel. Only one Change waits at a time.
  4. **The suffixes' field** (U8-Q10): on the General page, after the theme, a label "Companion
     File Suffixes" and a one-line text field. Its text is a `String` held while the window is open
     (`companion_suffixes` joined by `, ` when it opens), not parsed while the user types; on Save
     it is read with `parse_companion_suffixes` (made `pub` in `settings.rs`, re-exported), the
     same rule as `settings.txt`: an entry with `.`, `/`, `\`, `:` or a space is dropped, and a
     field with no valid entry gives the default list (`dop, pp3, on1`), as an empty line in
     `settings.txt` does. `save_settings_window` takes the list into `self.settings.shell`
     explicitly (as it does the theme), so it is saved and used by the next move. A short note
     under the field: "Files with these suffixes move with an image; empty means the default"
     (wording in the glossary).
  5. **Texts** (rule 13): the page name, Change, Clear, "Press a key…", the three refusals, the
     failed save of the keys, the suffixes' label and note, in `shell_text.rs`; in the glossary
     (§6) Keyboard Shortcuts, Change, Companion File Suffixes, "Clear" for a key (other senses
     exist) and "Restore Defaults" extended to this page.
- **Tests to add:** the page's rows in `KeyAction::ALL` order under their groups, each name not
  empty (a function in `shell_text.rs` or `keymap.rs` the page draws from); each refusal text;
  the suffixes' text written and read back (`dop, pp3, on1`; `dop, xmp2`), an empty field giving
  the default list; a function that turns a key event into the copy's change (taken, refused,
  Escape, a repeat skipped) tested without a window. **An existing test extended:**
  `the_settings_pages_are_in_order_with_distinct_names` (`shell_text.rs`) lists the three pages
  today; it gets the fourth page and its name added, nothing else in it changed (allowed by this
  task).
- **Hand check (the owner, after Done):** each group listed; Change on Fit to `Q`, then `Q` fits
  in the preview, `F` does nothing, the View menu and the preview's button show "Q"; Change to a
  taken key (refused, names the action), to Ctrl+Alt+K (refused), to an arrow (refused); Escape
  cancels a Change; no rating or import while a Change waits; Clear on a key, the menu without a
  key; a second key added to an action, both work; Save, restart, the keys kept (`keys.txt` has
  only the changed lines); Restore Defaults and Save, `keys.txt` with only the comment line;
  Cancel drops the changes; a Save of only the theme leaves a hand-edited `keys.txt` as it is; a
  hand-made bad line in `keys.txt` (the default kept; the log line is seen only with a console,
  the release build has none); the
  suffixes' field changed and saved, a move of a picked image takes a file with the new suffix;
  the page scrolls in a small window.
- **Not in this task:** keys for the metadata window or the dialogs (U8-Q1); the mouse (U8-Q9).
- **Rules:** `docs/tasks/README.md`; no new dependency; no panic paths.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written by the reviewer. Cold read: the key taken at the top of `ui()` by removing the events (the library's F5 and focused buttons see them otherwise), Tab and Escape moved by egui first, the page's state as `Loupe` fields, `keys_path`, the failed save's own text, the suffixes' text buffer, `parse_companion_suffixes` made `pub` and an empty field giving the default list (the existing rule; the owner may choose otherwise), the existing pages test extended, wrapped rows, the glossary's Clear and Restore Defaults. Chain review: `keys.txt` written only when the map changed; Save while a Change waits; the log not visible in the release build |
