# T279. The key map: actions, default keys, lookup per view and `keys.txt` (model) — Open

- **Why:** ADR 0017 §2.4 and step U8: one key map for the contact sheet and the preview, kept in
  a file and changeable in Settings. The owner answered the step's questions on 2026-11-22
  (ADR 0017 §6.4, U8-Q1–U8-Q13, every proposal approved). This task is the plain-Rust part: the
  changeable actions, their default keys (exactly today's), the fixed keys, the lookup of a key
  in a view, the rules for a new key (U8-Q4, U8-Q5) and reading and writing `keys.txt` (U8-Q6).
  T280 makes the shell ask the map and show its keys; T281 adds the Settings page (both Blocked
  until the task before is Done). Nothing the user sees changes in this task, and `main.rs` is
  not changed.
- **Crates:** `pw-loupe` (library part). **Files:** `crates/pw-loupe/src/keymap.rs` (new),
  `crates/pw-loupe/src/lib.rs`, `crates/pw-loupe/src/field_values.rs` (only `write_synced` made
  `pub(crate)`), `crates/pw-loupe/src/shell.rs` (only if `KeyChord` needs another derive),
  `docs/test-inventory.md`.
- **Background (read before starting):**
  - Today's key functions, in the order `main.rs`'s key loop asks them (about lines 7392–7570):
    `preview_key_action` (lib.rs; only in the preview), `opens_import`, `tab_action`,
    `shell_key_action`, `move_key_action`, `delete_key_action`, `reload_key_action`,
    `open_in_key_action`, `rating_key_action` (`ratings.rs`), `rotation_key_action`
    (`orientations.rs`), `key_action` for `M` and `I`, then per view `grid_key_action` or
    `loupe_key_action` and `key_action` (`grid.rs`, lib.rs). The lib.rs test helper
    `command_of_key` models that order.
  - `shell.rs`: `KeyChord { key, ctrl, shift, alt }`, `HIDE_KEY`, `Command`, `Command::acts_in`
    and `Where`, `MENUS`, `RATING_ITEMS`, `LABEL_ITEMS`, `IMAGE_CONTEXT_MENU`. `shell_text.rs`:
    `key_text` (the key as a menu shows it, "Ctrl+Shift+Tab", "−").
  - egui 0.37's `Key::name()` and `Key::from_name()` are used for the file, not `key_text`.
    `name()` gives for example `A`, `F5`, `Plus`, `Minus`, `Equals`, `Delete`, `Left`, `Right`,
    `Up`, `Down` and `0`–`9`; `from_name` also reads `ArrowLeft` and `Num3`. A key name is read
    as `from_name` reads it (case matters except for a single letter).
  - `RATING_ITEMS` and `LABEL_ITEMS` are private to `shell.rs`: reach them through
    `menu_items()` and the submenu entries of `IMAGE_CONTEXT_MENU`, not by name.
  - `field_values.rs` is the model for a file of its own: `field_values_path`,
    `load_field_values` (problems as English strings for the log, a missing file gives the
    defaults and no problem, a byte order mark skipped), `save_field_values` (folder created,
    written to `<name>.tmp` and renamed onto the file, `write_synced`).
- **What:**
  1. **`KeyAction`** (new, `keymap.rs`, re-exported from `lib.rs`): the changeable actions of
     U8-Q1, `Clone, Copy, Debug, PartialEq, Eq, Hash`, with `KeyAction::ALL` in the order below
     (the order the Settings page lists them in T281). Each has a fixed name in `keys.txt`
     (`name()`, `from_name()`), a group (`group() -> KeyGroup`; `KeyGroup` is `File`, `Edit`,
     `Image`, `View`, the menus they are in), where it acts (`acts_in() -> KeyView`, below) and its
     default keys (`default_keys() -> [Option<KeyChord>; 2]`), exactly today's:

     | Group | Action | Name | Default keys | Acts in |
     |---|---|---|---|---|
     | File | Open Folder | `open_folder` | Ctrl+O | Both (the preview takes its key and does nothing, U3-Q1) |
     | File | Reload Folder | `reload_folder` | F5 | Both |
     | File | Import | `import` | Ctrl+I | Both (taken in the preview, as Open Folder) |
     | File | Close Tab | `close_tab` | Ctrl+W | Both (taken in the preview) |
     | Edit | Undo Move | `undo_move` | Ctrl+Z | Both |
     | Edit | Select All | `select_all` | Ctrl+A | Sheet |
     | Edit | Metadata… | `metadata` | M | Both |
     | Image | 0–5 stars | `stars_0` … `stars_5` | `n`, Alt+`n` | Both |
     | Image | Red, Yellow, Green, Blue | `label_red`, `label_yellow`, `label_green`, `label_blue` | 6, Ctrl+1; 7, Ctrl+2; 8, Ctrl+3; 9, Ctrl+4 | Both |
     | Image | Purple | `label_purple` | Ctrl+5 | Both |
     | Image | No Label | `clear_label` | Ctrl+0 | Both |
     | Image | Reject | `reject` | X | Both |
     | Image | Move to Picked | `move_picked` | P | Both |
     | Image | Rotate Left, Rotate Right | `rotate_left`, `rotate_right` | Ctrl+←, Ctrl+→ | Both |
     | Image | RAW ↔ JPEG | `raw_jpeg` | R | Preview |
     | Image | Open in {name} (the first program) | `open_in` | Ctrl+E | Both |
     | Image | Delete to Recycle Bin | `delete` | Delete | Both |
     | View | Preview | `preview` | G | Both (opens it from the sheet, closes it in the preview) |
     | View | Fit, Fit ↔ 100 %, Zoom In, Zoom Out, Zoom at Pointer | `fit`, `zoom_toggle`, `zoom_in`, `zoom_out`, `zoom_at_pointer` | F, Z, +, −, A | Preview |
     | View | Clipped Highlights, Deep Shadows, Focus Marking | `clipped_highlights`, `deep_shadows`, `focus_marking` | J, Shift+J, E | Preview |
     | View | Compare, Hold Current Image, Swap Images, Above and Below, Lock Zoom and Position | `compare`, `hold_image`, `swap_images`, `above_below`, `lock_zoom` | C, K, Shift+K, V, L | Preview |
     | View | Marked only | `marked_only` | S | Preview |
     | View | Hide Side Columns (in the preview: the right column and the filmstrip) | `hide_columns` | H (`HIDE_KEY`) | Both |
     | View | Inspector (in the preview: its right column) | `inspector` | I | Both |
     | View | Full Screen | `full_screen` | F11 | Both (taken in the preview) |

     `KeyView` is `Sheet`, `Preview` or `Both`; a lookup is made in `Sheet` or `Preview`.
     Also `KeyAction::of_command(Command) -> Option<KeyAction>` and `command(self) ->
     Option<Command>` for the actions that are menu commands (`OpenIn(0)` and `OpenInAnother`
     both give `open_in`, since Ctrl+E runs `OpenInAnother` while the program list is empty;
     `open_in`'s command is `OpenIn(0)`; `marked_only` has no command), used by T280 for the
     menus. Commands whose key is fixed (Clear Selection on Escape, Next Tab and Previous Tab,
     Quit on Alt+F4) have no action; T280 keeps their menu keys as they are.
  2. **The fixed keys** (U8-Q1, U8-Q2): `fixed_key(chord, view) -> Option<FixedKey>`, which T280
     asks for what is not in the map, with
     ```rust
     pub enum FixedKey {
         Navigate(Navigation), // grid::Navigation, in the sheet
         Extend(Navigation),   // with Shift, in the sheet
         OpenPreview,          // Enter, Space (sheet)
         ClearMarking,         // Escape (sheet)
         NextTab, PreviousTab, // Ctrl+Tab, Ctrl+Shift+Tab (sheet)
         NextImage, PreviousImage, FirstImage, LastImage, // preview
         ClosePreview,         // Escape, Enter (preview; the shell closes a compare first)
         ZoomIn,               // `=` (preview)
         Taken,                // a key that does nothing in this view
     }
     ```
     - in the sheet: the arrows, Page Up/Down, Home, End → `Navigate`; the same with Shift →
       `Extend`; Enter and Space → `OpenPreview`; Escape → `ClearMarking`; Ctrl+Tab and
       Ctrl+Shift+Tab → `NextTab`, `PreviousTab`;
     - in the preview, each with or without Shift (today's `key_action` and `loupe_key_action`
       ignore Shift): →, ↓, Page Down → `NextImage`; ←, ↑, Page Up → `PreviousImage`; Home →
       `FirstImage`; End → `LastImage`; Escape and Enter → `ClosePreview`; `=` and Shift+`+`
       (`Plus` with Shift, which some keyboard layouts give for `+`) → `ZoomIn`; Ctrl+Tab and
       Ctrl+Shift+Tab → `Taken` (U3-Q1).
     - `is_fixed(chord) -> bool`: fixed in either view, or Tab or Shift+Tab (egui's focus moves),
       or Alt+F4. A fixed chord can never be given to an action.
  3. **`KeyMap`** (new, re-exported; `Clone, Debug, PartialEq, Eq`; private fields; `Default` =
     every action's `default_keys`):
     - `keys(&self, action) -> [Option<KeyChord>; 2]`; `first_key(&self, action) ->
       Option<KeyChord>` (the first `Some`, the one the menus show, U8-Q2);
     - `action_of(&self, chord, view) -> Option<KeyAction>`: the action whose key is exactly this
       chord (all four modifiers compared) and that acts in `view` (`Both` acts in both);
     - `allowed(chord) -> Result<(), KeyRefusal>` (U8-Q5): a key alone, or with Ctrl, Shift, Alt,
       Ctrl+Shift or Shift+Alt; refused with Ctrl+Alt (with or without Shift) and when
       `is_fixed`;
     - `can_set(&self, action, slot, chord) -> Result<(), KeyRefusal>`: `allowed`, and no other
       action (or the action's other slot) has the chord in a view both act in (U8-Q3, U8-Q4);
       `KeyRefusal` is `CtrlAlt`, `Fixed` or `Taken(KeyAction)`;
     - `set(&mut self, action, slot, chord) -> Result<(), KeyRefusal>`: as `can_set`, then the
       slot gets the chord; nothing changes when refused;
     - `clear(&mut self, action, slot)`; `restore_defaults(&mut self)`.
     `slot` is 0 or 1; another value does nothing (no panic, `AGENTS.md` §3). **The keys are kept
     packed:** `set` on slot 1 while slot 0 is empty fills slot 0; `clear` on slot 0 moves slot
     1's key up; so `keys` never gives `[None, Some(_)]` and a written line reads back equal.
  4. **`keys.txt`** (U8-Q6): `keys_path() -> Option<PathBuf>`
     (`%LOCALAPPDATA%\RawStride\keys.txt`, `None` without `LOCALAPPDATA`), `load_key_map(path) ->
     (KeyMap, Vec<String>)` and `save_key_map(path, &KeyMap) -> io::Result<()>`, as
     `field_values.rs` does.
     - The file starts with the comment line `# RawStride keys`; then one line per action whose
       keys differ from its default, in `KeyAction::ALL` order: `name = key[, key]`, a key written
       as its modifiers in the order Ctrl, Shift, Alt and `Key::name()`, joined by `+`
       (`Ctrl+Shift+K`, `F5`, `Alt+3`, `Ctrl+Left`); an action with no key is written `name =`. A
       map equal to the default gives a file with only the comment line.
     - Reading: blank lines and lines starting with `#` skipped; modifiers read in any order and
       case; spaces around `=`, `,` and `+` ignored. **Two passes**, so a map the program saved
       always reads back (a key moved from a later action to an earlier one gives the earlier line
       first): first every line is parsed; a line with no `=`, an unknown name, a key that cannot
       be read, more than two keys, a key that `allowed` refuses, or a second line for the same
       action is skipped with one problem naming its line number. Then the map is built: every
       action not named by an accepted line keeps its default keys; the named actions start with
       no key, and their lines are applied in file order with `set`; a key that clashes (by
       `can_set`) is skipped with one problem naming the line, and that action then gets its
       default keys back only where they are free (a default key already taken is left out; the
       action may end with no key, which the same problem says). A missing file gives the default
       map and no problem; a file that cannot be read gives the default map and one problem.
- **Tests to add** (in `keymap.rs`; no existing test changed):
  - **The default map is today's keys:** for every `egui::Key` with each of the eight modifier
    combinations, in each view, the new result (`KeyMap::default().action_of`, then `fixed_key`)
    equals today's result, both turned into one test-only type:
    `enum Outcome { Action(KeyAction), Fixed(FixedKey), Nothing }` (`None` and `Taken` →
    `Nothing`). Today's result is got by calling today's functions themselves in `main.rs`'s
    order (lines 7392–7570: in the preview `preview_key_action` first, then `opens_import`,
    `tab_action`, `shell_key_action`, `move_key_action`, `delete_key_action`, `reload_key_action`,
    `open_in_key_action`, `rating_key_action`, `rotation_key_action`, `key_action` for `M` and
    `I`, then `grid_key_action` or `loupe_key_action` and `key_action`) and translating each
    result in a table in the test (`PreviewKey::Nothing` → `Nothing`, the preview's Ctrl+O, Ctrl+I,
    Ctrl+W and F11 then count as equal to the map's `open_folder`, `import`, `close_tab`,
    `full_screen`, which T280 takes and does nothing with; `RatingKey::Stars(3)` → `stars_3`;
    `Action::Next` → `Fixed(NextImage)`; …). lib.rs's private `command_of_key` cannot be called
    from `keymap.rs` and models a different order; do not copy it. The only allowed differences,
    listed in the test by name: a Shift on a letter, a digit or `−` that today's `key_action`
    ignores in the preview, and Shift+`M` and Shift+`I` in both views (with the map, Shift+Z no
    longer toggles the zoom, U8-Q12's exact keys).
  - Every `MENUS`, `RATING_ITEMS`, `LABEL_ITEMS` and `IMAGE_CONTEXT_MENU` item whose command has
    an action shows that action's default first key.
  - `name`/`from_name` round trip and are unique; the default map passes `can_set` for every key
    it holds; `allowed` refuses Ctrl+Alt, Ctrl+Shift+Alt, every fixed chord, Alt+F4, Tab;
    `can_set` refuses a key taken in the same view (`Taken` names the action), and allows `A` for
    a sheet-only action while `zoom_at_pointer` has it (U8-Q3); `set` changes nothing when refused;
    a slot other than 0 or 1 does nothing.
  - `keys.txt`: a default map writes only the comment line and reads back equal; a changed map
    (a key moved from a later action to an earlier one, a second key added, a key cleared, slot 0
    cleared with slot 1 set) reads back equal; each problem case of
    point 4 gives one problem with its line number and leaves that action's default; a missing
    file; a byte order mark.
- **Not in this task:** `main.rs` (T280), the texts with a key in them (T280), the Settings page
  and the suffixes' field (T281). The metadata window's and the dialogs' keys stay as they are
  (U8-Q1).
- **Rules:** `docs/tasks/README.md`; no new dependency; no panic paths (`AGENTS.md` §3). The old
  key functions stay as they are (T280 decides which become test-only).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Written by the reviewer. Cold read: egui's key names (`Left`, `3`), `FixedKey`'s variants, Shift+Escape, Shift+Enter and Shift+`+` fixed in the preview as today, `OpenInAnother` → `open_in`, `write_synced` made `pub(crate)`, the private rating and label items reached through `menu_items()`, the comparison test with a common `Outcome` and today's functions called in `main.rs`'s order (not `command_of_key`). Chain review: `keys.txt` read in two passes so a moved or swapped key reads back, a clashing line's action gets only its free default keys, the keys kept packed so a cleared first slot reads back |
