# T221. ADR 0019 E4: a right-click menu on images, and the owner's hand check (pw-loupe) — Done

- **Why:** ADR 0019 §2.9 (addition 1, accepted by the owner 2026-11-19), step E4: a right-click on
  an image in the contact sheet, or on the image in the preview (the compare included), opens a
  menu with the commands that act on images. Today "Show in File Explorer" has no key (E-Q11) and
  the preview draws no menu bar, so it cannot be reached from the preview at all. The owner's
  answers are E-Q15–E-Q19 in ADR 0019 §3; ADR 0019 §4 puts the owner's hand check after E4, and it
  is written here.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/shell.rs` (point 1),
  `crates/pw-loupe/src/browse.rs` (point 2), `crates/pw-loupe/src/lib.rs` (only the export of
  point 1), `crates/pw-loupe/src/main.rs` (points 3–6), `docs/test-inventory.md`, and this task's
  own file (status and log).
- **What:**
  1. **The menu's entries** (`shell.rs`): a new
     `pub const IMAGE_CONTEXT_MENU: &[MenuEntry]`, beside `MENUS`, with a doc comment naming ADR
     0019 §2.9 and E-Q17, holding in this order (E-Q17):
     - `MenuEntry::ExternalPrograms` ("Open in {name}" per program, or "Open in Another
       Program…" with an empty list, drawn from the settings as in the Image menu);
     - `item(Command::ShowInExplorer, None)`;
     - `MenuEntry::Separator`;
     - `MenuEntry::Submenu(SubmenuKind::Rating, RATING_ITEMS)`;
     - `item(Command::Reject, plain(Key::X))`;
     - `item(Command::MovePicked, plain(Key::P))`;
     - `MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS)` (the colours and "No label", the
       same slice as the Image menu);
     - `MenuEntry::Separator`;
     - `item(Command::Metadata, plain(Key::M))`;
     - `MenuEntry::Separator`;
     - `item(Command::DeleteToRecycleBin, plain(Key::Delete))`.

     Left out on purpose (ADR 0019 §2.9): `MoveToSubfolders`, `ToggleMode` (RAW ↔ JPEG) and
     `DeleteRejected`, which act on the folder or the view, not on the images clicked. Every item
     uses the same key as its item in `MENUS`, so the two show the same (E-Q18; tested below).
     `IMAGE_CONTEXT_MENU` is exported in `lib.rs` beside `MENUS` and imported in `main.rs`.
     `menu_items()` stays as it is (it lists the menu bar only).
  2. **A right-click on a cell** (`browse.rs`): a new `Tab::right_click(&mut self, index: usize,
     ratings_ready: bool, rating_of: &dyn Fn(&Path) -> Option<Rating>) -> usize`, beside
     `Tab::click`, with a doc comment naming ADR 0019 §2.9 and E-Q16: a click on an image that is
     not visible changes nothing (as `click`, S5); otherwise `index` becomes current and the
     marking **and its anchor** are kept as they are (no call to `Selection`); then `refilter`
     keeping the new current image, as `click` ends. Returns the new current file index. Together
     with `Selection::targets` (unchanged) this gives E-Q16: a right-click on a marked image,
     with more than one marked, acts on all the marked images; on an image that is not marked,
     on that image alone.
  3. **One function draws the entries of both menus** (`main.rs`): the part of `draw_menu_bar`'s
     inner `match` that draws `MenuEntry::Item`, `MenuEntry::Separator`, `MenuEntry::Submenu` and
     `MenuEntry::ExternalPrograms` moves into a new
     `fn menu_entry(&self, ui: &mut egui::Ui, entry: MenuEntry, commands: &mut Vec<Command>)`,
     which calls the existing `menu_item` and `external_programs_menu` as today. `draw_menu_bar`
     calls it for those four kinds and keeps its own handling of `MenuEntry::Filter`,
     `MenuEntry::Sort` and the "Add to Favourites" button after `Command::CloseTab` (that button
     stays in `draw_menu_bar`, drawn right after `menu_entry` has drawn the Close Tab item).
     `menu_entry` does nothing for `Filter` and `Sort` (an empty arm with a comment that the menu
     bar draws them; `IMAGE_CONTEXT_MENU` holds neither, tested below). The menu bar looks and
     acts exactly as before.
  4. **The menu itself** (`main.rs`): a new
     `fn image_context_menu(&self, ui: &mut egui::Ui, commands: &mut Vec<Command>)` that calls
     `menu_entry` for every entry of `IMAGE_CONTEXT_MENU`. The texts, the keys on the right and
     the greying are those of the menu bar, since the same `menu_item`, `external_programs_menu`,
     `command_enabled` and `command_checked` are used (E-Q18: a program whose `.exe` is gone, no
     current image, Move to Picked while the setting is off). The menu closes after a choice by
     itself (egui 0.37's `Response::context_menu` is a popup with `CloseOnClick`; no `ui.close()`
     is needed, and a click on a greyed item also closes it). The
     commands go into the frame's `commands` list and through `run_command`, as the menu bar's
     do. No new text is shown, so no text function and no glossary change are needed (rule 13).
  5. **The contact sheet** (`main.rs`, `draw_grid`): the cell's `response` (today
     `egui::Sense::click()`, kept) gets, besides the left click and double click:
     - `response.secondary_clicked()` → the cell's index is remembered as this frame's
       right-click (as `clicked` is today) and, after the scroll area, passed to a new
       `fn right_click(&mut self, index: usize)` beside `fn click` (`main.rs` about line 1122),
       which calls `Tab::right_click` of point 2 with `ratings_ready` and `tab_rating` as `click`
       does;
     - `response.context_menu(|ui| self.image_context_menu(ui, commands))`.

     A right-click on the empty part of the contact sheet opens nothing (E-Q15): only the cells
     get the menu. `draw_grid` gets a `commands: &mut Vec<Command>` parameter for this.
  6. **The preview and the compare** (`main.rs`, `draw` and `draw_image_area`):
     `draw_image_area` (a free function, about line 6539) returns its `egui::Response` beside the
     `Frame` it returns today (`(Frame, egui::Response)`; its doc comment says so), so its caller
     can attach the menu. In `draw`, both calls (the current image's area, and the held image's
     area while comparing) attach `response.context_menu(|ui| self.image_context_menu(ui,
     commands))`: the current side's right after its `draw_image_area` call; the held side's
     **after** the `match (halves, &mut self.compare)` block has ended (the held side's
     `Response` kept in a variable declared before the `match`, `Option<egui::Response>`), since
     `open: &mut` from `&mut self.compare` is alive until `open.held_size_known` is set and a
     `&self` call inside the block does not compile (E0502). A right-click there changes **nothing** about which image is current or held:
     in the preview the shown image is already the current one, and in the compare both sides'
     menus act on the current image, as the keys do (E-Q15, E-Q16). The existing mouse handling of
     `draw_image_area` (double-click, drag, wheel) is not changed. `draw` gets a
     `commands: &mut Vec<Command>` parameter, passed from both its callers (`draw_preview`, which
     already has one, and `ui`, the frame's own `commands`), and passes it on to `draw_grid`.
  7. **Not while a dialog holds the program:** while `self.dialog_open()` is true (the metadata
     window, the import or settings window, the delete confirmation, the question of "Open in
     {name}"), no right-click menu opens and a right-click changes no current image: points 5 and
     6 attach the menu and handle `secondary_clicked` only when `!self.dialog_open()`, as the
     bars are disabled then (`ui`, `blocked`). (`run_command` also refuses then, as today.)
- **Tests to add:**
  - `shell.rs`: `the_image_context_menu_holds_the_image_commands_in_order`: `IMAGE_CONTEXT_MENU`
    equals the list of point 1, entry for entry.
  - `shell.rs`: `every_item_of_the_image_context_menu_is_the_menu_bars_item`: for every
    `MenuEntry::Item` of `IMAGE_CONTEXT_MENU` there is an item in `menu_items()` with the same
    command and the same key; every `MenuEntry::Submenu` of it is also an entry of the Image menu
    of `MENUS` (same kind, same slice); it holds no `MenuEntry::Filter` and no `MenuEntry::Sort`;
    and no item of it is `MoveToSubfolders`, `ToggleMode` or `DeleteRejected`.
  - `browse.rs`: `a_right_click_makes_the_image_current_and_keeps_the_marking`: with three images
    marked by clicks (and so an anchor), a right-click on a fourth visible image that is not
    marked makes it current, leaves the same three marked and the anchor unchanged, and
    `selection.targets(&visible, current, ViewMode::Grid)` is that image alone.
  - `browse.rs`: `a_right_click_on_a_marked_image_targets_every_marked_image`: with three images
    marked (the existing helper `three_marked`, which leaves the current image on a marked one)
    and the current one then moved to an image that is not marked without touching the marking
    (`preview_move(LoupeMove::To(…), …)`), a right-click on one of the marked makes it current and
    `targets` gives the three marked in display order.
  - `browse.rs`: `a_right_click_on_a_hidden_image_changes_nothing`: as the existing
    `a_click_on_a_hidden_image_changes_nothing` (`browse.rs`, about line 666, a filter set first),
    for `right_click`: it returns the current index as it was, and the tab (current image, marking
    and anchor) equals the tab before.
  - The drawing in `main.rs` (points 3–7) is tried in the owner's hand check below; no test needs a
    window.
- **The owner's hand check** (ADR 0019 §4, E4), done by the owner after the review:
  1. Contact sheet, several images marked: right-click a marked image → it becomes current, the
     marking stays; "Show in File Explorer" opens that image's folder; a rating from the menu's
     Rating submenu goes to all the marked images.
  2. Contact sheet: right-click an image that is not marked → it becomes current, the marking
     stays on screen; a rating from the menu goes to that image alone.
  3. Contact sheet: right-click the empty space after the last cell → no menu.
  4. Preview: right-click the image → the menu; "Show in File Explorer" and "Open in {name}"
     work on the shown image.
  5. Compare: right-click each side → the menu acts on the current image (the one with the blue
     frame), the held image stays held.
  6. A greyed item: Move to Picked with the setting off, and a program whose `.exe` was renamed
     (greyed in both the Image menu and the right-click menu).
  7. The menu bar's Image, Edit and View menus look and act as before (point 3).
- **Not in this task:** a right-click menu on the filmstrip's images (ADR 0019 §2.9 names the
  contact sheet and the preview only); a submenu to apply a metadata template (later, with
  ADR 0017 U13, ADR 0019 §2.9); opening the menu from the keyboard (Shift+F10 or the menu key);
  any change to the keys, to `Selection` or to `command_enabled`'s rules. The keys keep acting
  while the menu is open (Escape closes it and also clears the marking, the arrows move the
  current image under it), as they do while a menu of the menu bar is open; not changed here.
- **Rules:** `AGENTS.md`, `docs/tasks/README.md` (rules 1–13), `docs/TESTING.md`. No new
  dependency. No `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or unchecked indexing in
  non-test code. No user-visible string literal in drawing code. Every existing test stays as it
  is.
- **Done when:** `.\tools\check.ps1` is all OK, with no `cargo clippy` warning, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added (ADR 0019 E4, §2.9, E-Q15–E-Q19). The owner, 2026-11-19: no menu on the filmstrip for now. Cold read: doable with the named files, names and lines right. Changed: point 4 says egui 0.37's context menu closes by itself (no `ui.close()`); point 6 places the held side's menu after the compare `match` (borrow of `self.compare`, E0502); the hidden-image test names its return value and the filter; the marked-image test names `three_marked` and moving the current image first; "Not in this task" says the keys keep acting while the menu is open, as with the menu bar |
| 2026-11-19 | Ready for review | Points 1–7 and the five tests done. `shell.rs`: `IMAGE_CONTEXT_MENU`; `browse.rs`: `Tab::right_click`; `main.rs`: `menu_entry` (used by `draw_menu_bar` and `image_context_menu`), `right_click`, the menu on the contact sheet's cells and on both sides of the preview and compare (the held side's after the compare `match`), none while `dialog_open()`; `draw_image_area` returns `(Frame, egui::Response)`; `draw` and `draw_grid` take `commands`. The submenu test compares the slices by content (a `const` slice has no fixed address). `docs/test-inventory.md`: `pw-loupe` 975 → 980, total 1962. check.ps1 all OK: 1962 passed, 72 ignored |
| 2026-11-19 | Done | Reviewed by the reviewer: every point and the five tests as written; check.ps1 all OK (1962 passed, 72 ignored), no clippy warning; no independent code review needed (one crate, six files). The owner's choice (no menu on the filmstrip for now) written into ADR 0019 §2.9. Leftovers (keys while the menu is open, opening it from the keyboard, the first frame's greying) in `backlog.md`, "Leftovers of T221". Committed; the owner's hand check follows (plan E4 row)
| 2026-11-19 | Done | The owner's hand check (points 1-7): OK, approved by the owner |
