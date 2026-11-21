# T245. ADR 0017 U15b: four buttons on a thumbnail in the contact sheet (pw-loupe) — Done

- **Why:** ADR 0017 §2.8 (addition U15, accepted by the owner 2026-11-21, U15-Q1–U15-Q8): a user
  working with the mouse in the contact sheet reaches the commands used most on one image without
  the menu bar or the right-click menu. Four round buttons, one in each corner of the cell's image
  square, shown only while the pointer is over the cell: Rotate Left (upper left), Rotate Right
  (upper right), Metadata… as an "i" (lower left), Preview as a magnifying glass (lower right).
  Every command keeps its key; the buttons are an aid for the mouse only. Second and last task of
  the chain T244 → T245 (plan `adr-0018-0025-steps.md`, order 2a, row U15); the owner's hand check
  of U15 follows this task. T244 made the metadata window step only between the marked images when
  its image is marked and two or more are (U15-Q9), which the Metadata… button relies on.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/thumb_buttons.rs` (new: points 1–3 and their tests)
  - `crates/pw-loupe/src/lib.rs` (the `mod` line and the re-exports of points 1–3 and 6)
  - `crates/pw-loupe/src/shell.rs` (point 4: the command, its menu item, the button colours and
    their contrast pairs, and the existing tests that list them)
  - `crates/pw-loupe/src/shell_text.rs` (point 6 and its tests)
  - `crates/pw-loupe/src/settings.rs` (point 5 and its tests)
  - `crates/pw-loupe/src/main.rs` (points 7–10)
  - `docs/glossary.md` (point 6)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **The buttons and where they are** (`thumb_buttons.rs`, plain Rust with `egui::Rect` and
     `egui::Pos2` as values, tested without a window; module doc comment naming ADR 0017 §2.8,
     U15, T245):
     - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum ThumbButton { RotateLeft,
       RotateRight, Metadata, Preview }` (`Hash` for the egui id of point 8) and
       `pub const THUMB_BUTTONS: [ThumbButton; 4]` in that order;
     - `ThumbButton::command(self) -> Command`: `Command::RotateLeft`, `Command::RotateRight`,
       `Command::Metadata`, `Command::ToggleView`;
     - `pub const BUTTON_SIZE: f32 = 20.0` (the disc's diameter in points, the same at every
       thumbnail size, U15-Q6) and `pub const BUTTON_INSET: f32 = 4.0` (from the image square's
       edges to the disc's edge);
     - `button_rect(image: egui::Rect, button: ThumbButton) -> egui::Rect`: the disc's bounding
       square in the corner of `image` (the cell's square image area, not the fitted thumbnail,
       §2.8): upper left for Rotate Left, upper right for Rotate Right, lower left for Metadata,
       lower right for Preview, each `BUTTON_INSET` in from both edges;
     - `button_at(image: egui::Rect, pointer: egui::Pos2) -> Option<ThumbButton>`: the button
       whose `button_rect` holds `pointer`, else `None`. The square is the button's whole hit
       area, for the hover and the click alike (point 8); only the drawing is round.
  2. **Which buttons are drawn** (`thumb_buttons.rs`, U15-Q5: a command that cannot run draws
     no button, by the rules that grey its menu item):
     `pub struct ButtonFacts { pub rotate: bool, pub metadata: bool, pub preview: bool }` and
     `ThumbButton::shown(self, facts: &ButtonFacts) -> bool` (Rotate Left and Rotate Right by
     `rotate`, Metadata by `metadata`, Preview by `preview`). The facts are filled in point 8.
  3. **What Preview opens with** (`thumb_buttons.rs`, U15-Q4):
     `preview_only_marked(marked_count: usize, image_marked: bool) -> bool`: true when the image is
     marked and at least two images are marked, else false. The shell sets "only the marked" to
     this value before it opens the preview (point 9).
  4. **The setting's command and the button colours** (`shell.rs`):
     - `Command::ThumbnailButtons` with a doc comment ("Buttons on Thumbnails" on ↔ off, View
       menu, a check item, no key; ADR 0017 §2.8, U15-Q7, T245); in `MENUS`' View menu right after
       `item(Command::Fullscreen, plain(Key::F11))`, as `item(Command::ThumbnailButtons, None)`;
       `acts_in` gives `Where::Both` (the default arm). Not in `IMAGE_CONTEXT_MENU` (add it to that
       constant's doc comment's "Left out on purpose" list: it acts on the view);
     - in `image_area` (the contact sheet keeps the same dark grey in both themes, U1-Q4):
       `THUMB_BUTTON_DISC` (a dark grey, partly transparent: `Color32::from_black_alpha` or
       `from_rgba_unmultiplied`, the agent picks a value that passes the test below),
       `THUMB_BUTTON_DISC_HOVERED` (the lighter disc under the pointer) and
       `THUMB_BUTTON_SYMBOL` (a light grey);
     - `non_text_pairs` gets these pairs, each blended in a small private helper (`Color32` holds
       premultiplied colour and `contrast_ratio` ignores alpha, so the helper computes
       `premultiplied + background × (1 − alpha)` per channel): "thumbnail button symbol on its
       disc over a white image", "… over a black image", "… over the empty cell" (the disc
       blended over white, black and `image_area::CELL`, the symbol against that), the same three
       for the hovered disc, and "thumbnail button disc on a white image" (the disc over white
       against white), so the disc stands out on a light image and the symbol on any image. A
       dark disc cannot stand out against a black image by itself (1:1); there the light symbol
       carries the contrast, which the "over a black image" pairs check. Each must reach 3:1 (WCAG
       non-text contrast, U15-Q6) in the existing test `every_non_text_pair_of_both_themes_reaches_3`
       (a black disc needs an alpha of about 0.42 or more for the white pair). Name the chosen
       values and their ratios in the report.
  5. **The setting** (`settings.rs`): `ShellSettings::thumbnail_buttons: bool`, **on by default**,
     read and written as the line `thumbnail_buttons = true|false` through `parse_bool`, as the
     other check items are (ADR 0017 §2.8, U15-Q7, as corrected by the owner 2026-11-21). It is
     written after `move_to_subfolders` and `companion_suffixes`'s line,
     so the existing test of the whole written file gets the new last line
     `thumbnail_buttons = true`. A file without the line gives on and no problem; a bad value keeps
     on and names the line in the problems.
  6. **Texts** (`shell_text.rs`, rule 13), each from one template:
     - `command_text(Command::ThumbnailButtons)` → `"Buttons on Thumbnails"`; the existing tests
       `every_menu_item_text_in_menu_order` (after `"Full Screen"`) and
       `every_menu_key_text_in_menu_order` (`None` after `Some("F11")`) get the new item;
     - `thumb_button_hover_text(command: Command, key: Option<KeyChord>) -> String`: the hover
       text of a button, the command's menu text and its key, `format!("{command} ({key})")` with
       `command_text` and `key_text`, for example `"Rotate Left (Ctrl+←)"`, `"Rotate Right
       (Ctrl+→)"`, `"Metadata… (M)"`, `"Preview (G)"`; with no key, the menu text alone. Added to
       `lib.rs`'s `pub use shell_text::{…}`. A test lists the four buttons' texts, with each key
       taken from the button's item in `menu_items()` (the same key the menu shows), and one test
       for no key.
     - `docs/glossary.md`: add "Buttons on Thumbnails" (the View menu's check item and the four
       buttons on a thumbnail in the contact sheet, ADR 0017 §2.8) where the shell's View menu
       terms are; name it in the report.
  7. **The View menu item** (`main.rs`): `command_checked(Command::ThumbnailButtons)` →
     `Some(self.settings.shell.thumbnail_buttons)`; `run_command` toggles it through a
     `toggle_thumbnail_buttons` beside `toggle_focus_marking`, which only flips the field: the
     settings file is written when the program closes (`on_exit`), as for the View menu's other
     check items.
  8. **Drawing the buttons** (`main.rs`, `draw_grid`'s loop over the cells, after `paint_cell` and
     the cell's `ui.interact`):
     - the buttons are drawn for a cell only when all hold: `thumbnail_buttons` is on; the cell
       holds the pointer (`response.contains_pointer()`, not `hovered()`, which is false while the
       pointer is over a button on top of the cell); no dialog is open (`self.dialog_open()`, as
       the right-click menu's `context_menu`); and no menu or popup is open (the right-click
       menu, a menu of the menu bar, a drop-down of the command bar: `egui::Popup::is_any_open(ui.ctx())`, which reads
       egui's memory and so also sees a right-click menu of a cell drawn later in the frame; not
       `ctx.any_popup_open()`, which sees only popups already shown this frame; a hover text is
       not a popup there, so a button's own hover text does not hide the buttons). Only in the contact sheet: the filmstrip, the
       preview and the compare draw none (U15-Q3);
     - `ButtonFacts` for the cell's image as if it were current (point 2): `rotate` and `preview`
       by `command_enabled`'s rules for `RotateLeft` and `ToggleView` (a shown image: true for a
       cell in the contact sheet); `metadata` true when the file's kind has metadata support
       (`supports_rating`, as `window_neighbour` checks it). This is stricter than the menu item,
       which is not greyed for such kinds, but `MetadataCache::open_window` refuses them, so the
       menu item does nothing there; §2.8 names this case, so follow it and do not stop to ask. The menu rules have no other
       condition today (a running move does not grey the rotations); follow them as they are and
       add none of your own (the owner, 2026-11-21, ADR 0017 §2.8, U15-Q5);
     - each shown button: the disc (`THUMB_BUTTON_DISC`, or `THUMB_BUTTON_DISC_HOVERED` when
       `button_at` gives this button for the pointer) as a filled circle in `button_rect`, and its symbol drawn with the painter in
       `THUMB_BUTTON_SYMBOL`, lines and shapes only, as the stars are drawn (no icon set, no font
       glyph needed, no new dependency, U15-Q6): ↺ and ↻ as an open arc of about 270° with an
       arrow head at its end, turning anticlockwise and clockwise; "i" as a dot above a short
       vertical bar; the magnifying glass as a ring with a short handle to the lower right. The
       symbols are drawn in a small private function per symbol taking the button's rect;
     - each shown button is its own `ui.interact` area over `button_rect`, with an id of the
       cell's index and the button, added **after** the cell's, so egui gives a click on it to the
       button and the cell's response sees no click and no double-click; its hover text is
       `thumb_button_hover_text` with the key of the command's item in `menu_items()`;
     - a right click on a button is a right click on the cell: the button's `secondary_clicked`
       records `right_clicked` as the cell's does, and the button's response gets the same
       right-click menu (`image_context_menu`, under the same `context_menu` condition);
     - egui counts a double click per pointer, not per widget, so a click on a button followed
       quickly by a click on the cell can report `double_clicked` on the cell. Remember the time
       of the last button click (`ui.input(|i| i.time)`); a cell's `double_clicked` within egui's
       `max_double_click_delay` (from `ui.input(|i| i.options…)`, or 0.3 s if 0.37 has no such
       option; name it in the report) after a button click is handled as a plain click.
  9. **A click on a button** (`main.rs`): recorded in the loop as `(index, ThumbButton)` and done
     after the scroll area, where the cell's click and right click are done today, in this order:
     - the image becomes current with the marking kept, as a right click does
       (`self.right_click(index)`, ADR 0019 §2.9, E-Q16). Its refilter can hide or unmark other
       images, so everything below is read **after** it: the button acts only if the active tab's
       current image is then `index` and shown (`tab.current == index` and
       `tab.position().is_some()`; with an active filter and the ratings not read the visible
       list can be empty), and the marked count, the image's mark and the targets are taken from
       the tab as it is after `right_click`;
     - Rotate Left and Rotate Right: `run_command` with the button's command; they then act on
       the marked images when the image is marked, otherwise on it alone (`key_targets`, ADR 0024
       O-Q2);
     - Preview: `self.settings.shell.filmstrip_marked` is set to `preview_only_marked(marked
       count, image marked)` (the marked count and the image's mark from the active tab:
       `selection.marked_in(&tab.visible)`), then the preview opens on the image as a double click
       opens it (`switch_mode(ViewMode::Loupe)`). The choice stays afterwards, as when `S` sets
       it;
     - Metadata: the metadata window opens **on the clicked image**, also when it is not marked
       and two or more others are; `M` and the menus keep opening it as today
       (`metadata_subject`) (the owner, 2026-11-21, ADR 0017 §2.8, U15-Q4). Split `open_metadata_window` so that
       its body after the choice of path is a function taking the path (for example
       `open_metadata_window_on(path)`), called by `open_metadata_window` with
       `metadata_subject`'s path and by the button with the clicked file's path (resolved as
       `metadata_subject` resolves it). The guard that does nothing while a metadata or Settings
       window is open moves into `open_metadata_window_on`, so the button cannot open a second
       window (for example after `M` opened one earlier in the same frame: keys run before
       `draw_grid`); the marked list for "Apply to {n} Marked" is
       `marked_subjects` as before. With T244 the window then steps between the marked when the
       image is marked and two or more are, else between the shown images.
     A click on a button never changes the marking, and two quick clicks on a rotation button
     turn the image twice and do not open the preview (U15-Q4).
  10. **Repaint:** the contact sheet already repaints when the pointer moves over it (egui does
      on pointer movement); add no timer.
  11. **Fix after the review** (the reviewer, 2026-11-21). The work done so far stays; only this
      changes. **A right click on a button opens the cell's right-click menu for one frame only.**
      In `draw_grid` a cell's buttons are drawn, and their `ui.interact` and `context_menu`
      called, only while `egui::Popup::is_any_open` is false. A right click on a button opens
      that button's menu (`Response::context_menu`, popup id `Popup::default_response_id`, that
      is the button response's id `.with("popup")`); in the next frame a popup is open, so the
      button's `context_menu` is no longer called, and egui 0.37 closes a popup that was not
      shown in a frame (`Memory::end_pass`, "Clean up abandoned popups"). Fix: while the menu of
      one of the cell's buttons is open (`Popup::is_id_open` with that id), that button's
      `ui.interact` (the same id and rect) and its `context_menu` are still called each frame, so
      the menu stays open, but no button is drawn (no disc, no symbol, no hover text) while any
      popup is open, as now. Nothing else in the hiding rule changes. Hand-check item 4 covers it
      ("a right click on a button opens the cell's right-click menu": the menu must stay open
      until an item is chosen or it is closed).
- **Tests to add:**
  - `thumb_buttons.rs`: each button's rect in its corner and `BUTTON_INSET` from both edges, for
    an image square at the origin and one elsewhere; `BUTTON_SIZE` is the same at every size; at
    the smallest thumbnail size (`MIN_THUMBNAIL_SIZE` minus the label row of 18 points, as
    `draw_grid` computes the image square; `GRID_LABEL_HEIGHT` is the binary's, so the test writes
    18 with a comment naming it: a 78-point square leaves 30 points between two discs) the four rects do not overlap and keep at least
    `BUTTON_INSET` between them, and also at `MAX_THUMBNAIL_SIZE`; `button_at` gives the button at
    each disc's centre and at a corner point just inside its square, and `None` in the middle of
    the image square and just outside a button's square; `shown` per fact; `preview_only_marked`: (0, false) → false, (1, true)
    → false, (2, false) → false, (2, true) → true, (5, true) → true.
  - `shell.rs`: the existing menu tests pass with the new item (every command once, every key at
    most once); `Command::ThumbnailButtons.acts_in()` is `Where::Both`; it is not in
    `IMAGE_CONTEXT_MENU`; the new contrast pairs in `non_text_pairs` (the existing test checks 3:1).
  - `shell_text.rs`: the texts of point 6.
  - `settings.rs`: the default is on; written and read for both values; a file without the line
    gives on and no problem; a bad value keeps on and names the line.
  - The drawing, the clicks and the hover in `main.rs` are checked by the owner's hand check; the
    rules they use live in `thumb_buttons.rs`.
- **Not in this task:** buttons in the filmstrip, the preview or the compare (U15-Q3); a Reject or
  any other button (U15-Q1); buttons on the name row; a key for "Buttons on Thumbnails" (U8 may add
  one); a setting for the buttons in the Settings window; the right-click menu's Metadata… (it
  keeps `metadata_subject`, ADR 0019 §2.9).
- **Rules:** `docs/tasks/README.md` rules 1–13 (rule 13 for the texts); no new dependency
  (U15-Q6). No `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or unchecked indexing outside
  tests. More than one area of the shell and eight files: the reviewer gets an independent code
  review of the diff before the owner is asked to approve (`docs/tasks/README.md`).
- **The owner's hand check (U15, after this task; ADR 0017 §7 row U15):**
  1. Each of the four buttons on an image that is not marked: Rotate Left and Right turn only that
     image; "i" opens the metadata window on it; the magnifier opens the preview on it with "only
     the marked" off.
  2. The same with three images marked, on one of them: the rotations turn all three; the
     magnifier opens the preview with "only the marked" on and the steps go only between the
     three; the marking is unchanged after each click.
  3. "i" on a marked image (three marked): Previous, Next and Save and Next step only between the
     three; `M` on a marked image does the same; on an image not marked they step between all the
     shown images. With a filter (for example "unrated") and the marked images rated in the window
     one by one: an image the filter then hides is unmarked when the window moves on, and with
     fewer than two marked the steps go between all the shown images (the rule as written).
  4. Two quick clicks on a rotation button turn the image twice and do not open the preview; a
     click on a button does not change the marking; a click on a button and then quickly on the
     cell does not open the preview; a right click on a button opens the cell's right-click menu.
     "i" on an image not marked (three others marked), then Next onto a marked image: from there
     the steps go only between the marked, and "Apply to 3 Marked" can be used.
  5. A file without metadata support (for example a PNG, if the folder has one) shows no "i".
  6. No buttons while the right-click menu, a menu of the menu bar, the metadata window or the
     Settings window is open; none in the filmstrip, the preview or the compare.
  7. View ▸ Buttons on Thumbnails off: no buttons; still off after a restart; on again.
  8. The smallest and the largest thumbnail size: the buttons fit, the same size, in the corners
     of the square; hover texts as in the menus ("Rotate Left (Ctrl+←)").
  9. Both themes: the buttons are seen on a light and on a dark image.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added (ADR 0017 U15b; second task of the chain T244 → T245); waits for T244 |
| 2026-11-21 | Blocked | The owner's decisions written in (ADR 0017 §2.8, U15-Q4, Q5, Q7): Metadata… on the clicked image; the menu items' rules as they are; `thumbnail_buttons = true\|false`. Cold read: doable with the named files (the only exhaustive matches on `Command` are in `shell_text.rs` and `main.rs`). Changed: the contrast pairs blended correctly (premultiplied) over white, black and the cell, with the symbol carrying the contrast on a black image (the stop clause removed); the square is the hit area for hover and click; `ThumbButton`'s derives; `Popup::is_any_open` named for the open-menu check; the setting written on exit as the View menu's other check items; the test's 18-point label row |
| 2026-11-21 | Open | T244 done; this task can start |
| 2026-11-21 | Ready for review | Points 1–10 done. New `thumb_buttons.rs` (`ThumbButton`, `THUMB_BUTTONS`, `BUTTON_SIZE` 20, `BUTTON_INSET` 4, `button_rect`, `button_at`, `ButtonFacts`, `shown`, `preview_only_marked`); `Command::ThumbnailButtons` after Full Screen; `thumbnail_buttons` (on by default, written last of the shell lines); `thumb_button_hover_text`; `main.rs`: the check item, the drawing and clicks in `draw_grid`, `thumb_button_click`, `open_metadata_window_on`. Colours: disc `from_black_alpha(0x8c)`, hovered disc `from_rgba_unmultiplied_const(0x70, 0x70, 0x70, 0xe0)`, symbol `from_gray(0xf0)`. The double-click delay is egui's `InputOptions::max_double_click_delay`, read through `ctx.options` (`InputState::options` is private in 0.37). Two existing settings tests that took `companion_suffixes` as the last shell line (`the_move_settings_are_written_and_read`, `the_external_program_line_is_after_the_shell_lines_and_before_the_library`) now expect `thumbnail_buttons = true` after it, as point 5 places it. 15 tests added; test-inventory 2119 passed, 76 ignored. check.ps1 all OK, no clippy warnings |
| 2026-11-21 | Open | Sent back after review (the reviewer): points 1–10 as written, checked against the brief (corners, sizes and gaps, the contrast pairs about 4.2:1 and 4.7:1 on white, the setting's line last, the hover texts, the guard in `open_metadata_window_on`, the double-click delay). Finding: a right click on a button opens its menu for one frame only, as egui closes a popup not shown in a frame once the buttons are hidden by `Popup::is_any_open`: point 11. The task agent's code stays uncommitted; check.ps1 and the independent review after the fix |
| 2026-11-21 | Ready for review | Point 11 done (`main.rs`, `draw_grid` only): for each button of a cell, while the setting is on and no dialog is open, its id is made first and `Popup::is_id_open(id.with("popup"))` read; a button is skipped only when it is neither drawn (pointer over the cell, no popup open, shown by its fact) nor has its menu open. A button with its menu open keeps its `ui.interact` (same id and rect) and `context_menu` each frame but draws no disc, symbol or hover text and takes no left click. Nothing else in the hiding rule changed. No new test (drawing and clicks are hand-checked, item 4). check.ps1 all OK, 2119 passed, 76 ignored, no clippy warnings |
| 2026-11-21 | Done | Reviewed (reviewer): points 1–11 as written; point 11 checked against egui 0.37 (the popup id is the button response's id `.with("popup")`). Independent review (fresh agent, read only): nothing serious; it confirmed against egui's source that a button click is the button's alone (one click target per press), that the cell's `contains_pointer` holds over a button, that a hover text is not a popup, and the order after the scroll area. Accepted, not changed: a real double click on a cell within the double-click delay after a button click is taken as a plain click (the rule of point 8 as written); the hovered disc follows the pointer position, not the button's `hovered()` (cosmetic). The reviewer's clean run of check.ps1: all OK, 2119 passed, 76 ignored, no clippy warnings. CHANGELOG: an Added line. Moved to `done/`; the owner's hand check of U15 follows |
| 2026-11-21 | Done | Hand-checked by the owner (U15, items 1–9): all passed. The owner thinks the buttons' symbols need to be larger (backlog) |
