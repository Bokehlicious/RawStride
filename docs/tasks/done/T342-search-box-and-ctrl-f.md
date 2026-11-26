# T342. ADR 0022 S3b: the search box and Ctrl+F — Done

- **Why:** ADR 0022 S3 (the search box and Ctrl+F), decided by the owner on 2026-11-26 (ADR 0022
  §3.2, S3-Q1, S3-Q2, S3-Q9, S3-Q13). T341 makes the shown list, the reads and the status line
  follow the active tab's search (`tab.meta_filter`); this task adds what sets it: the box in the
  command bar and the key. The second task of the chain T341 → **T342**; after it the owner's
  hand check of S1–S3 (ADR 0022 §4).
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-loupe/src/main.rs`: `draw_command_bar` (about line 8973): Import…, Open Folder…,
    the star condition, stars and label lists (`filter_combo`, about line 12128, outlines an
    active list in the accent, about lines 12143–12150), then the thumbnail size; it is drawn
    only outside the preview (about lines 13207–13223: in the preview there is no menu bar or
    command bar, U3-Q1). `apply_filter` (about line 8368) gives the keyboard back to the images
    after a filter choice (`surrender_focus`) and scrolls to the current image; `set_filter`
    (about line 2368). `filter_menu` (about line 8909) with "Show All" (about line 8912), which
    keeps the stars' condition and clears stars and label. `run_command` (about line 8490). The
    key map is asked only while no text field has the keyboard (about lines 9596–9615:
    `!ctx.egui_wants_keyboard_input()`). `switch_mode` (about line 4025).
  - `crates/pw-loupe/src/keymap.rs`: `KeyAction` (about line 62), `ALL` (about line 160),
    `name` (about line 220), `group` (`KeyGroup::File`, `Edit`, `Image`, `View`, about line
    296), `acts_in` (`KeyView::Sheet`, `Preview`, `Both`, about line 340), the default keys
    (about line 380; `with_ctrl(Key::…)`), `of_command` (about line 436) and `command` (about
    line 496); the tests `commands_and_actions_correspond`, `names_round_trip_and_are_unique`,
    `the_default_map_passes_can_set_for_every_key_it_holds`,
    `every_menu_item_with_an_action_shows_its_default_first_key` (about lines 1375–1440).
    Ctrl+F is free (F alone is Fit, in the preview).
  - `crates/pw-loupe/src/shell.rs`: `Command` (about line 717), `Where` and `Command::acts_in`
    (about lines 858–870).
  - `crates/pw-loupe/src/shell_text.rs`: `ShellText` with `FilterMenu` and `ShowAll` (about
    lines 38–84), the key action names (about line 431), the command texts (about line 621),
    and their tests.
  - `crates/pw-loupe/src/meta_filter.rs`: `Filter::set_search`, `search` (about lines 443–450);
    a text of spaces only is no search (`has_search`).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/keymap.rs`, `crates/pw-loupe/src/shell.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `docs/glossary.md`, `docs/test-inventory.md` (rule 12),
  and this task's own file (status and log).
- **Model:** Opus — the keyboard focus between the box, the key map and the preview, and a
  delayed apply over tab switches (README, "Model").
- **What:**
  1. **The command and the key** (S3-Q2): `Command::Search` in `shell.rs` (`acts_in` falls to
     `Where::Both`), in no menu (none is decided); `command_enabled` (`main.rs`, about line
     8379) gives `Command::Search` true only with an active tab. `KeyAction::Search` in
     `keymap.rs`, right after `Metadata` (the last of the Edit group) in the enum and in `ALL`
     (`index()` is the declaration order; `ALL` grows by one), name `search` in `keys.txt`,
     group `KeyGroup::Edit`, `acts_in` `KeyView::Both`, default keys
     `[with_ctrl(Key::F), None]`; arms in `name`, `group`, the default keys, `of_command` and
     `command`. `command_text` (`shell_text.rs`, about line 590) gives "Search"; the Keyboard
     Shortcuts page takes the name from there (rule 13). The test `the_default_map_is_todays_keys`
     (`keymap.rs`, about line 1346) compares the map with its reference `today()` (about line
     1177): `today()` gets one line, Ctrl+F → Search in both views, the only change to it. `docs/glossary.md` gets a row in §3 (Window, menus and
     commands) for "Search" (the box's hint text and the action), "added by T342".
  2. **The box** (S3-Q1), in `draw_command_bar` right after the label list, inside the filter
     group and before the separator and the sort list (about lines 9021–9026): a single-line
     text field
     with the hint text "Search" (`ShellText::SearchHint`, the same word as point 1's text,
     its own variant), about 160 points wide (the hint and the text may be cut by the field,
     not by the layout). Disabled when there is no active tab, as the filter lists are. The
     text being typed is the shell's (`App`), not the tab's: `search_edit: String` and
     `search_edited_at: Option<Instant>`.
     - **Typing:** each change sets `search_edited_at` to now; once 0.3 s have passed without a
       change (a repaint is asked for that moment), the text is applied (point 3).
     - **Enter:** applies the text at once and gives the keyboard back to the images, as
       `apply_filter` does, and the contact sheet scrolls to the current image also when the
       text was already applied (`set_search` then does nothing).
     - **Escape** in the box: empties it, applies the empty text at once, and the box loses the
       keyboard. egui (0.37) takes the focus away on Escape at the start of the frame and a
       single-line field gives it up on Enter, so both are seen as `response.lost_focus()` with
       `key_pressed(Escape)` or `key_pressed(Enter)`.
     - **Keys while the box has the keyboard:** the command bar is drawn before the key loop of
       `draw` (about lines 9440–9660), where only the key map is guarded by
       `egui_wants_keyboard_input` (about line 9607); the fixed keys after it (arrows, Home, End,
       Page Up/Down, Space, Enter, Escape, Ctrl+Tab) are not. Both get the guard: no fixed key
       acts while a text field has the keyboard, and none in the frame the box gave it up on
       Enter or Escape (a flag `search_took_key`, set by the box and cleared by the key loop),
       so Escape does not also clear the marking and Enter does not open the preview.
     - **Another tab:** `Tab` has no id. The box remembers what it shows: the active tab's
       index and folder, the number of tabs, and the search it last applied or loaded. Each
       frame, when the active index, folder or number of tabs differs (a tab of the same folder
       closed at the same index is then noticed too), or no change is pending and the tab's search
       (`tab.meta_filter.search()`) differs from the remembered one (a folder opened again
       empties it, `Tab::open`), the box loads the tab's search and a change not yet applied is
       dropped (it was less than 0.3 s of typing).
     - **Outlined** in the accent colour while the tab's applied search is not empty, as an
       active filter list is (the same stroke as `filter_combo`) (S3-Q9). While the box has the
       keyboard egui draws its own focus border; that is accepted.
  3. **Applying** a text: a new `fn set_search(&mut self, text: &str)` in `main.rs`, beside
     `set_filter`: nothing when the active tab's search is already `text`; otherwise
     `tab.meta_filter.set_search(text)`, `refilter_active(None)`, the preview follows
     (`loupe_follows_visible`), and the contact sheet scrolls to the current image (as
     `apply_filter`). T341's reads start by themselves (`follow_filter_values`).
  4. **Ctrl+F**: an arm `A::Search` in `key_map_action` (about line 6038; keys reach it, not
     `run_command`) and `Command::Search` in `run_command` do the same: with no active tab or a
     dialog open (`dialog_open`), nothing. In the preview (the loupe and Compare), back to the
     contact sheet as `A::Preview` leaves it, `done.scroll_to = Some(self.switch_mode(
     ViewMode::Grid))` (about lines 6165–6172; Compare closes with it through
     `follow_compare`), so the key loop's own `scroll_to` is not overwritten (about lines
     9676–9679) (S3-Q13). Then `search_focus_wanted = true`: when the box is next drawn it asks
     for the keyboard with its whole text selected, so typing replaces it (egui:
     `TextEditState::load`, `cursor.set_char_range` over the whole text, `store`, then
     `request_focus`).
  5. **Show All** (S3-Q9, `filter_menu`, about line 8909): also empties the active tab's search
     (and the box). `filter_menu` returns only a `Filter` today (through `draw_menu_bar`'s
     `filter_choice`, about lines 8720 and 13228); it also says that Show All was chosen (a
     small enum or a flag, the agent's choice), and the caller applies the star and label
     filter, then `set_search("")`; the result is every image.
- **Tests to add:**
  - `keymap.rs`: `KeyAction::Search` has Ctrl+F by default in both views, the name `search`
    round-trips, and `of_command(Command::Search)` gives it; the existing tests over `ALL` pass
    with it (`today()` as point 1 says, nothing else in them changed).
  - `main.rs`: the guard of the fixed keys as a plain function (a key with a text field having
    the keyboard, or with `search_took_key`, gives no fixed key), tested for Escape, Enter,
    Space and an arrow.
  - `shell.rs`: `Command::Search.acts_in()` is `Where::Both`.
  - `shell_text.rs`: the texts "Search" (hint, command, key action name).
  - `main.rs`: (a) the delayed apply as a plain function of the edit time and now (applied at
    0.3 s, not before; Enter and Escape at once) so it can be tested without a window; (b)
    `set_search` refilters the active tab only and leaves other tabs' searches as they were;
    (c) Show All leaves the search empty and every image shown; (d) a text of spaces only
    shows every image.
- **Not in this task:** the "Filter…" window, "Filter: {n} conditions" (S4); saved filters
  (S5); any change to the refilter, the reads or the status line (T341); a menu item for Search.
- **Rules:** `docs/tasks/README.md` rules 1–13. No `git commit`, `git add`, `git stash`,
  `git reset`, `git checkout` and no new branch (rule 8). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  Then the owner's hand check of S1–S3 (written by the reviewer at T342's review): typing in the
  box with the delay, Enter, Escape, Ctrl+F in the contact sheet and in the loupe and Compare
  (back to the sheet, the cursor in the box), the matches coming in while "Reading metadata
  {i} of {n} …" counts, several words (AND), a part of a word, a file name, a JPEG with IIM
  only, the search with a star filter, marks kept for unread images, `M` on a shown image that
  then stops matching (it leaves after its read, not before), Show All, another tab with its
  own search; from the chain review: F5 and a change on disk while the values are read, a tab
  switch while they are read, a batch (`M` on several, Apply to All Marked) with a search
  active (the count stands still while the batch writes), the waiting preview's text, a file
  that cannot be read ("could not be read"), Ctrl+A while nothing is shown (the marks stay,
  S3-Q15), and a tab closed while a change in the box is pending; from the review: after Tab has moved the keyboard to a button, the arrows and Space in the contact sheet wait until a click (the guard of point 2 counts every widget with the keyboard, not only a text field). `latency.ps1` is not needed
  (no change to the view). At Done one CHANGELOG line under Added covers T341 and T342 (T341
  gets none: nothing is visible after it alone).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Added (ADR 0022 §3.2, S3-Q1, S3-Q2, S3-Q9, S3-Q13); waits for T341 (both change `main.rs`) |
| 2026-11-26 | Blocked | Chain review (see T341's log): Enter scrolls also with the text already applied; the number of tabs remembered too; the hand check extended; the CHANGELOG line at this task's Done |
| 2026-11-26 | Blocked | Cold read: the fixed keys were not guarded while a text field has the keyboard, and Enter or Escape leaving the box reached the grid in the same frame (point 2: the guard and `search_took_key`); keys go through `key_map_action` (point 4: the `A::Search` arm, leaving the preview as `A::Preview`); `command_enabled`; `the_default_map_is_todays_keys` and its `today()`; the box's place before the sort group; how Show All is told; how a tab change is noticed; select-all through `TextEditState` |
| 2026-11-26 | Open | T341 done |
| 2026-11-26 | Ready for review | Implemented S3b. All eight new tests passed. The sandbox run passed fmt, personal data and clippy. Cargo audit could not lock the RustSec database, and existing workspace tests failed when ExifTool and `mklink /J` could not run. The full check and the owner's hand check remain pending. |
| 2026-11-26 | Done | Reviewed (done by Codex): every point and test as written; Codex's own check failed in its sandbox (ExifTool, `mklink`, the audit lock); `.\tools\check.ps1` run by the reviewer, all OK, no clippy warnings. Changed by the reviewer (owner's yes): the outline follows `meta_filter.is_active()`, so a search of spaces only (no search) is not outlined; the glossary row moved to §3 as point 1 says; doc comments on the new items; `docs/test-inventory.md` rewritten in its form and its total (2841). Noted: the fixed keys' guard counts every widget with the keyboard (`egui_wants_keyboard_input`, as the key map's), so after Tab has focused a button the arrows wait until a click; added to the hand check, no change. CHANGELOG line under Added for T341 and T342. Awaiting the owner's hand check of S1–S3 (the list above) |
