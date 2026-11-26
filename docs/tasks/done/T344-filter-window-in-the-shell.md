# T344. ADR 0022 S4b: the "Filter…" window, its button, menu item and key — Done

- **Why:** ADR 0022 S4 (the "Filter…" window), decided by the owner on 2026-11-26 (ADR 0022
  §3.3, S4-Q1–S4-Q3, S4-Q5, S4-Q8–S4-Q11). T343 makes the window's model (`FilterDraft`,
  `DraftRow`), the parsing, the counts and the texts; this task draws the window and applies
  its rows to the active tab, adds the command bar's button, the menu item and the key, and
  makes the status line count the groups the whole filter needs. The second task of the chain
  T343 → **T344** → T345 (the value lists' reading of the whole folder).
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-loupe/src/main.rs`: `FilterMenuChoice` and `filter_and_search` (about lines
    420–443: Show All returns the search `Some("")`), their use in `draw` (about line 13589)
    and the test `show_all_clears_search_stars_and_labels` (about line 14316); `set_filter`
    (about line 2443), `set_search` (about line 2455: nothing when unchanged, then
    `refilter_active(None)`, `loupe_follows_visible`, `scroll_request`); `focus_search` (about
    line 2521: no tab or a dialog open → nothing; in the preview back to the contact sheet
    with `switch_mode(ViewMode::Grid)`); `key_map_action` (about line 6274; `A::Search` about
    line 6397); `dialog_open`, `other_dialog_open`, `open_window_shown` (about lines
    6151–6180: every modal window is named in all three); `draw_template_window` (about line
    6003: the modal backdrop at `Order::Foreground`, the window as a sublayer above it, Escape
    taken by the window); `filter_menu` (about line 9145: Show All, then the star condition,
    stars and labels as radio items); `menu_item` (about line 9127: the key on the right with
    `shortcut_text(key_text(…))`); `draw_search_box` (about line 9205) and
    `draw_command_bar` (about line 9267: the search box is the last thing in the filter group,
    `add_enabled_ui(tab_filter.is_some(), …)`); `filter_combo` (about line 12491: the accent
    outline of an active list); `search_counts` and `search_waiting` (about lines 12085–12107,
    IPTC only through `TabValues::search_counts`), used by the status line (about lines 4723
    and 10047) and the waiting preview (`active_search_waiting`, about line 2738).
  - `crates/pw-loupe/src/keymap.rs`: `KeyAction` and `ALL` (`Search` about line 170, the last
    of the Edit group), `name` (about line 234), `group` (about line 293), the default keys
    (about line 387; `with_ctrl` and `with_shift` exist, no Ctrl+Shift helper; Ctrl+Shift+F is
    not used by any default or fixed key), `of_command` (about line 450), `command` (about line
    510), the test `the_default_map_is_todays_keys` and its `today()`.
  - `crates/pw-loupe/src/shell.rs`: `Command`, `Command::acts_in`. `shell_text.rs`:
    `command_text` (gives "Search" for `Command::Search`, T342).
  - `crates/pw-loupe/src/tabs.rs`, `main.rs`: `Tab::open` (`tabs.rs` about line 73) clears
    `meta_filter` (search and rows) when a folder is opened; F5 (`reload_tab` →
    `list_into_tab`, `main.rs` about lines 4170–4214) and a relist after a change on disk keep
    `meta_filter`, and F5 clears the values (`filter_values.clear()`). Search and rows live
    together in `meta_filter`, so the rows follow the search box in every case (S4-Q11 needs
    nothing more).
  - `follow_sort` (`main.rs` about line 2597) records the capture reader's answers, the
    filter's Exif values with them (`Tabs::record_capture`), and `request_captures` records
    `None` when the reader has stopped; neither refilters. `follow_filter_values` (about line
    2641) refilters (`refilter_after_filter_read`) only on IPTC answers or a chunk's end. A
    dropped folder is opened unless the metadata window, a delete's or an Open's question is
    shown (about lines 9722–9733).
  - The key loop of `draw` (about lines 9811–9876): each modal window has its own branch
    (`if let Some(window) = &mut self.template_window { … continue; }`) that takes the keys and
    turns Escape into the window's `escape` flag (about lines 9843–9848); the template window
    lets Escape close an open value list first (`value_list_escape`). `ui()` draws the modal
    windows (`draw_template_window`, about line 13613). The caller of `FilterMenuChoice`
    (about lines 13583–13594) runs `apply_filter`, then empties `search_edit` and calls
    `apply_search_edit`; each refilters only on a change. `set_tab_search` (about line 408) is
    the free function the search tests use (a `Loupe` cannot be built in tests; helpers about
    lines 14287–14296).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/keymap.rs`, `crates/pw-loupe/src/shell.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `docs/glossary.md`, `docs/test-inventory.md` (rule 12),
  and this task's own file (status and log).
- **Model:** Opus — a modal window over the key loop and the preview, and the refilter
  (README, "Model"; S4-Q14).
- **What:**
  1. **The command and the key** (S4-Q2): `Command::FilterWindow` in `shell.rs` (`acts_in`
     `Where::Both`); `command_text` gives "Filter…"; `command_enabled` true only with an
     active tab. `KeyAction::FilterWindow` in `keymap.rs` right after `Search` in the enum and
     in `ALL`, name `filter` in `keys.txt`, group `KeyGroup::Edit`, `acts_in` `KeyView::Both`,
     default keys `[with_ctrl_shift(Key::F), None]` with a new `const fn with_ctrl_shift`
     beside `with_ctrl`; arms in `name`, `group`, the default keys, `of_command` and
     `command`; `ALL`'s length grows from 54 to 55. `today()` of
     `the_default_map_is_todays_keys` gets one line beside the Ctrl+F line in its last
     `match view` (about keymap.rs line 1290, not inside the preview-key branch), Ctrl+Shift+F
     → FilterWindow in both views, the only change to it. No default or fixed key uses
     Ctrl+Shift+F; a user whose `keys.txt` already gives it to another action gets the
     "taken" problem as at T342 (accepted). Like Ctrl+F it does nothing while a text field
     (the search box) has the keyboard.
  2. **Opening** (S4-Q2): an arm `A::FilterWindow` in `key_map_action` and
     `Command::FilterWindow` in `run_command` do the same, as `focus_search` does: with no
     active tab or a dialog open, nothing; in the preview (the loupe and Compare) back to the
     contact sheet first (`done.scroll_to` as `A::Search` sets it); then the window opens with
     `FilterDraft::from_filter(&tab.meta_filter)` for the active tab. The menu: in
     `filter_menu`, an item "Filter…" right after "Show All", with its key on the right as
     `menu_item` shows keys (`menu_key`'s way for `KeyAction::FilterWindow`), its text from
     `command_text(Command::FilterWindow)` (rule 13), giving a new
     `FilterMenuChoice::Window` that the caller turns into the same opening.
  3. **The window** (S4-Q1, S4-Q3): the shell keeps `filter_window: Option<FilterWindowState>`
     (the draft, and the index and folder of the tab it was opened for). It is modal as the
     template window is (a backdrop and a sublayer with ids of its own, `Id::unique`), drawn
     by a `draw_filter_window(ctx)` called in `ui()` beside `draw_template_window`, named in
     `dialog_open`, `other_dialog_open` and `open_window_shown`, and given a branch of its own
     in the key loop as the template window has (keys taken, Escape set as a flag), so no key,
     click or other dialog reaches the program behind while it is open. A dropped folder is
     not opened while it is open either (the drop's condition gets
     `&& self.filter_window.is_none()`, as for the metadata window, D16). Each frame, when the
     active tab is no longer the window's (same index and folder; an import can open a tab
     meanwhile), the window closes as Cancel and its draft is lost (rare; the reviewer notes it
     in `known-limits.md` at Done), so the window never shows another tab's values. S4-Q1's
     "the list behind does not change" means the draft is not applied until OK: the applied
     filter goes on as before behind the window (its reads refilter, the watcher relists).
     Title `FilterText::Title`. Inside, top to bottom:
     - with no rows, `FilterText::NoConditions`;
     - one line per row: the field (a combo box listing `Field::ALL` under the headings
       `group_label` File, Exif, IPTC, each field by `field_label`); the condition (a combo
       box of `field.kinds()` by `condition_text`); the value: for Contains, Is, IsNot one
       text field; for AtLeast, AtMost one, for Between two, drawn with the texts in the order of
       `between_parts` (T343), each with `value_hint` as its hint; for OneOf on File Type four check boxes
       (`FILE_TYPES`); for OneOf on a field with `has_value_list` a scrolling list of check
       boxes from `list_entries(&tab.filter_values.value_counts(field, &tab.files), picks)`,
       each `value_count_text`, or `FilterText::NoValues` when it is empty (T345 adds the
       reading of the whole folder; here the list shows what is read already); nothing for
       IsEmpty; and a button `FilterText::Remove`. A row that is not complete shows
       `missing_text` under it in the error colour.
     - the buttons `AddCondition`, `Clear` (removes every row of the draft), `Ok` (enabled only
       while `FilterDraft::result()` is `Ok`) and `Cancel`.
     - Escape and the window's close button are Cancel; while a combo box's list is open,
       Escape only closes that list (as the template window's `value_list_escape`). Enter does
       nothing of its own (not decided). Cancel drops the draft; nothing in the tab changes.
  4. **OK** (S4-Q1): a free `fn set_tab_rows(tabs: &mut Tabs, rows: Vec<Row>) -> bool` beside
     `set_tab_search` (false, changing nothing, when the active tab's rows are already `rows`;
     `Row` is `PartialEq`), and a method `fn set_rows(&mut self, rows: Vec<Row>)` beside
     `set_search` that uses it: on a change `refilter_active(None)`, `loupe_follows_visible`, and the
     contact sheet scrolls to the current image (as `set_search`). OK applies only when the
     active tab is still the one the window was opened for (same index and folder); otherwise
     (an import opened a tab meanwhile) the window closes and nothing changes. The window
     closes. The
     reads for the new rows start by themselves (`follow_filter_values` asks
     `meta_filter.needs`). Marks follow the refilter as for the search (S4-Q9, nothing new).
  4a. **Exif answers refilter** (found in the chain review; S3-Q11, S4-Q8, S4-Q9): today only
     IPTC answers refilter, so a row on an Exif field alone (ISO ≥ 1600, with the sort not by
     capture time) would keep every image hidden as unknown after its Exif is read, the count
     would reach n of n with "No images match the filter", and a waiting preview would never
     go back. A flag `exif_arrived` on the shell is set when `follow_sort` records an answer,
     or `request_captures` records `None`, for a file of the active tab while its
     `meta_filter` needs Exif; `follow_filter_values` refilters (`refilter_after_filter_read`)
     when `had_answers || ended || exif_arrived` and clears the flag.
  5. **The button** (S4-Q10): in `draw_command_bar`, right after the search box inside the
     filter group (disabled without an active tab), a button with
     `filter_button_text(tab.meta_filter.condition_count())`, outlined in the accent colour
     while the count is not 0 with the same stroke as `filter_combo`; a click is
     `Command::FilterWindow`.
  6. **Show All** (S4-Q11): `FilterMenuChoice::ShowAll` also removes every row of the active
     tab. The caller first removes the rows with `set_tab_rows(tabs, Vec::new())` (no
     refilter yet), then runs today's flow (`apply_filter`, the search box emptied,
     `apply_search_edit`, which keeps the box's state in step); when the rows were removed but
     neither the filter nor the search changed, it then refilters once
     (`refilter_active(None)`, `loupe_follows_visible`). The result is every image.
     `FilterMenuChoice::Window` is matched by the caller before `filter_and_search`, whose
     signature and test stay as they are.
  7. **The status line's count** (S4-Q8): `search_counts` (`main.rs`) calls
     `tab.filter_values.filter_counts(&tab.meta_filter, &files)` instead of `search_counts`;
     its users (the status line, the grid's text, the waiting preview, `search_waiting`) stay
     as they are, so the rows' Exif and IPTC reads are counted in "Reading metadata {i} of
     {n} …" and the preview waits for them (S4-Q9, S3-Q11).
  8. **Glossary** (`docs/glossary.md` §3, Window, menus and commands): a row for "Filter…"
     (the menu item and the key action), "added by T344".
- **Tests to add:**
  - `keymap.rs`: `KeyAction::FilterWindow` has Ctrl+Shift+F by default in both views, the name
    `filter` round-trips, `of_command(Command::FilterWindow)` gives it; the tests over `ALL`
    pass with it (`today()` as point 1 says, nothing else in them changed).
  - `shell.rs`: `Command::FilterWindow.acts_in()` is `Where::Both`. `shell_text.rs`: "Filter…".
  - `main.rs`: (a) `set_rows` refilters the active tab only, keeps its search, and does nothing
    for the same rows; (b) a row Camera contains "x" hides an image whose Exif is read without
    it and keeps an image whose Exif is not read hidden until read (unknown), with the
    image's mark kept for the unknown one and dropped for the other (S4-Q9); (c) Show All
    leaves no rows, no search and every image shown (extend
    `show_all_clears_search_stars_and_labels`, or a new test beside it); (d) the status count
    with a row on an Exif field and a search counts an image done only with both groups read;
    (e) the opening as a new plain function of (active tab, dialog open, in the preview) →
    (open, back to the sheet), as `focus_search`'s rules; (f) OK's tab check as a plain
    function (same index and folder → apply; other → not; the same function closes the
    window each frame); (g) the refilter condition of point 4a as a plain function of (IPTC
    answers, chunk ended, Exif arrived), and through `Tabs`: an Exif-only row whose Exif is
    recorded shows the matching image once refiltered. (a), (b) and (c) through
    `set_tab_rows` and the free helpers on `Tabs`, as the search tests do.
- **Not in this task:** reading the whole folder for a value list (T345); saved filters (S5);
  a tooltip on the button (S4-Q10: none); any change to the model or texts of T343 (if one is
  needed, stop and ask).
- **Rules:** `docs/tasks/README.md` rules 1–13. No `git commit`, `git add`, `git stash`,
  `git reset`, `git checkout` and no new branch (rule 8). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No hand check of its own; S4's hand check follows T345.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Added (ADR 0022 §3.3); waits for T343 (uses its model and texts) |
| 2026-11-26 | Blocked | Cold read: the key loop's branch for the modal window, Escape closing an open combo list first, the draw call in `ui()` with ids of its own; `ALL` 54 → 55 and `today()`'s line beside Ctrl+F; Ctrl+Shift+F confirmed free (an older `keys.txt` that took it: "taken", as at T342); Show All through today's flow plus one refilter when only the rows changed, `filter_and_search` unchanged; `set_tab_rows` as a free function for the tests; OK's tab check by index and folder; Enter not OK; F5 clears, a relist on disk keeps |
| 2026-11-26 | Blocked | Chain review (see T343's log): point 4a (Exif answers refilter); the window closes as Cancel when its tab stops being the active one; no folder drop while it is open; S4-Q1's "list behind" read as "the draft is not applied"; F5 keeps `meta_filter` (corrected); the menu item's text from `command_text` |
| 2026-11-26 | Open | T343 done |
| 2026-11-26 | Ready for review | Implemented the modal filter window, shell entry points, Show All, Exif refiltering and combined read counts. Added regression tests. The test inventory and the note about discarding the draft when the active tab changes are for the reviewer. |
| 2026-11-26 | Done | Reviewed (done by Codex): every point and test as written; `.\tools\check.ps1` run by the reviewer, all OK, no clippy warnings. `FilterMenuChoice::filter_and_search` got an arm for `Window` (needed by the new variant; signature and test unchanged); the command bar's enabled state follows `dialog_open()` in the same frame, so the bar is locked at once when the menu opens the window (accepted). The reviewer put back the doc comments of `dialog_open`, `other_dialog_open`, `open_window_shown` and `FilterMenuChoice` with the filter window added, moved `search_counts`' doc comment back from `filter_read_changed`, and gave the new items doc comments (owner's yes). Leftovers in `known-limits.md` (ADR 0022): the draft lost when the active tab changes; the value lists' and the extended filter's counts made in every frame (added to the item of T341); an Exif answer looked up in the tab's file list (owner accepted). `CHANGELOG.md` line under Unreleased. `docs/test-inventory.md` updated by the reviewer (+10). No hand check of its own; S4's follows T345 |
