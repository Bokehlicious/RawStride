# T280. The shell asks the key map, and the menus and texts show its keys — Blocked: waits for T279

- **Why:** ADR 0017 §2.4, step U8 (ADR 0017 §6.4, U8-Q1–U8-Q13, owner 2026-11-22). T279 made the
  key map (`KeyMap`, `KeyAction`, `fixed_key`, `keys.txt`). This task makes the program use it:
  `keys.txt` read at start, every changeable key looked up in the map, and every text that names
  a key showing the map's key (U8-Q8). With no `keys.txt` everything works and reads as today,
  except the exact modifiers of T279 (Shift+Z no longer zooms). T281 adds the page that changes the
  map.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/preview_text.rs`, `crates/pw-loupe/src/loupe_text.rs`,
  `crates/pw-loupe/src/library_text.rs`, `crates/pw-loupe/src/keymap.rs` (only for tests or a
  small helper), `docs/test-inventory.md`.
- **Background (read before starting):** T279's file and `keymap.rs`; `main.rs`'s key loop
  (about lines 7253–7570: the events collected first, `tab_action` computed already in that
  closure at about line 7266, then the branches in order); `external_programs_menu` (about lines
  6728–6756: "Open in Another Program…" and "Open in {name}" both show `OPEN_IN_KEY`);
  `menu_item` (about line 6761); the thumbnail buttons' hover text (`thumb_button_hover_text`,
  about line 3306); the library's F5 (about lines 7070–7083, `library_took_f5`) and its hover
  text in `draw_library` (about lines 9745–9755, a free function that gets `&shell`); `StatusBar`
  (about line 312, `mode: Option<&'static str>`, set at about line 7814); the error texts of
  `loupe_text.rs` (lines 187 and 190) drawn at about line 7975; `start_field_values` (about line
  10441) as the model for reading a file at start.
- **What:**
  0. **egui's own keys off** (U8-Q13): at start, `zoom_with_keyboard = false` and
     `quit_shortcuts` emptied in egui's options, so Ctrl+Q, Ctrl+plus, Ctrl+minus and Ctrl+0 do
     nothing of egui's own (Ctrl+0 is then only No Label).
  1. **At start:** the map is read with `load_key_map` at `keys_path()`, each problem written to
     the log as `start_field_values` does; without a path the default map. The app holds
     `key_map: KeyMap` and `keys_path: Option<PathBuf>` (beside `settings_path`; T281 saves to it).
  2. **The key loop:** every chord is looked up first with `self.key_map.action_of(chord, view)`
     (`view` = `Preview` while `in_preview()`, else `Sheet`), then with `fixed_key`. The chord is
     built as today (`ctrl = modifiers.ctrl || modifiers.command`). Each action does what today's
     branch for its old key does; the code that turns a `KeyAction` into `rate`'s `RatingKey`,
     into `Action`, `ShellAction`, `switch_mode` or a command, and a `FixedKey` into `grid_move`
     (with its `Navigation`), `tab_action`, `act` or closing the preview, lives in `main.rs` (or a
     small tested helper in `keymap.rs`). The order of the branches before it (the delete's and
     Open's questions, the metadata window, the import dialog) is unchanged. **Guards:**
     - **every map action** does nothing while a text field has the keyboard
       (`egui_wants_keyboard_input`): a key moved to a letter must never act while the user
       types; today's Ctrl+I and Ctrl+W branches had no such guard because they used Ctrl;
     - **while the Settings window is open**, no map action and no fixed key acts (today the
       ratings, rotations and navigation still did); the other dialogs block as today
       (`dialog_open()` where today's branch asked it; Import sets `dialog_consumes_keys` as today);
     - kept as today: no repeat for Reload Folder, Open in, the rotations; `step_allowed` for the
       next and previous image; `!self.library_took_f5` on `reload_folder`; Escape closes an open
       compare first (U6-Q11); in the preview Open Folder, Import, Close Tab and Full Screen are
       taken and do nothing (today's `PreviewKey::Nothing`), as is `FixedKey::Taken`.
     - `tab_action` is no longer computed from the raw key in the collecting closure: Close Tab
       comes from the map, Next and Previous Tab from `fixed_key`.
     No old key function is asked for a changeable key any more, so a key moved away from an
     action stops doing it.
  3. **The library's Refresh** (U4-Q1) takes the keys of `reload_folder`, exactly (a modifier
     counts, so Ctrl+F5 over the column no longer refreshes; intended), instead of
     `key_pressed(F5)`; `draw_library` gets the key it shows (or the finished text) as a parameter.
  4. **The menus and texts** (U8-Q8). One pure function, `menu_key(item: MenuItem, map: &KeyMap)
     -> Option<KeyChord>` in `shell.rs`: the map's `first_key` for an item whose command has a
     `KeyAction` (`OpenInAnother` too, through `of_command`), else the item's own fixed key
     (Escape, Ctrl+Tab, Ctrl+Shift+Tab, Alt+F4). Every menu item, right-click item, both "Open
     in" items and the thumbnail buttons' hover text use it. The preview's texts ("Fit (F)", "Fit
     ↔ 100% (Z)", "Zoom In (+)", "Zoom Out (−)", "Zoom at Pointer (A)", "Marked only (S)",
     "Collapse Column (I)", "Clipped highlights (J)", "Deep shadows (Shift+J)"), the mode labels
     ("Mode: RAW (R)", "Mode: JPEG (R)"; `StatusBar.mode` becomes a `String`), the two error texts
     ("Switch to JPEG mode (R) …", "Switch to RAW mode (R).") and the library's "Read the drives
     again (F5)" are made by new functions in their `*_text.rs` file from the text without the key
     and an `Option<KeyChord>` (`key_text`; rule 13). **An action with no key:** the text without
     the parentheses, everywhere. "Show Column (I)" (`PreviewText::ExpandColumn`) is drawn
     nowhere and stays as it is. The existing text functions and their tests stay; a new test
     shows that the default map gives exactly today's texts.
  5. **`key_text`** gets ↑ and ↓ as it has ← and →, so a key the user may give shows well; other
     keys keep egui's name ("PageUp", "Equals"), recorded as a known limit by the reviewer.
  6. **The old key functions** (`preview_key_action`, `shell_key_action`, `move_key_action`,
     `delete_key_action`, `reload_key_action`, `open_in_key_action`, `opens_import`,
     `tab_action`, `grid_key_action`, `loupe_key_action`, `rotation_key_action`, `key_action`,
     `OPEN_IN_KEY`) stay as they are, `pub` and re-exported: they are not deleted and their
     tests are not changed, and T279's comparison test keeps comparing with them. Being `pub`
     items of the library, they give no `dead_code` warning; `main.rs` only stops importing the
     ones it no longer calls (no unused import). `rating_key_action` stays in use through
     `editor_rating_key`; `editor_key_action` and `dialog_key_action` stay as they are (U8-Q1).
- **Tests to add:** a map with a key moved (Fit on `Q`, `F` cleared): `action_of` gives Fit for
  `Q` and nothing for `F` in the preview; `menu_key` for a moved, a cleared and a fixed item, and
  for `OpenInAnother`; each new text function with the default key (today's text), a moved key
  and no key; `key_text` of ↑ and ↓; the helper that turns a `KeyAction` into a `RatingKey` (if
  written in `keymap.rs`). Say in the report what was only checked by hand (the library's key,
  the Settings window blocking keys).
- **Not in this task:** the Settings page, the suffixes' field and saving the map (T281).
- **Rules:** `docs/tasks/README.md`; no new dependency; no panic paths. Every test that passed
  before passes unchanged.
- **Done when:** `.\tools\check.ps1` is all OK, no clippy warnings, and `git diff --stat` shows
  only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written by the reviewer. Cold read: the guards listed per action (a key moved to a letter must not act in a text field), `tab_action` computed in the collecting closure, "Open in Another Program…" shows `open_in`'s key, the missing texts (the mode label's JPEG form, the two error texts) and the unused "Show Column (I)", `StatusBar.mode` a `String`, `draw_library`'s parameter, no parentheses without a key, `menu_key` named, `key_text` ↑ ↓, the old functions left `pub` rather than `#[cfg(test)]` (no warning, no split re-exports), `rating_key_action` stays in use. Chain review: every map action guarded against text fields; no key acts while the Settings window is open |
