# T314. ADR 0027 W2: the key, the menu item and the mark of fine details — Done

- **Why:** ADR 0027 §2.1, §3.1, W2-Q4 and W2-Q5 (answered by the owner 2026-11-24, ADR 0027
  §4.2). T313 gave the view core fine details (`ViewCore::set_fine_details`, `Frame::fine`).
  This task lets the user turn it on and off with **D** and View ▸ Show Fine Details, and
  makes the preview's row of adjustment marks: an adjustment that is on is always visible
  (§2.1), so the mark "Fine" is shown while fine details is on, dimmed while it is paused. W3
  later adds "USM" to the same row. Fine details is not saved: it lasts for the session (A-Q2).
  The last task of the chain T312 → T313 → **T314**; the owner's hand check and `latency.ps1`
  follow it (W2-Q7). Waits for T313.
- **What the reviewer checked (2026-11-24):** the focus marking (`E`, T168) goes through every
  place a toggle of the preview does, and fine details follows it, except that it is not a
  setting:
  - `shell.rs`: `Command` (`FocusMarking`), its `acts_in` and the View menu's list
    (`item(Command::FocusMarking, plain(Key::E))`); `HIDE_KEY` is H.
  - `keymap.rs`: the key map's actions (`FocusMarking`, name `"focus_marking"`, default
    `plain(Key::E)`, group `KeyGroup::View`, acting in `KeyView::Preview`), its conversions to and
    from `Command`, and the `PreviewKey` mapping. **D** is bound to nothing today (Ctrl+D is
    not either; `open_in_key_action` refuses Ctrl+D, `lib.rs`).
  - `lib.rs`: `PreviewKey` (`FocusMarking`), `preview_key_action` (`(K::E, false, false,
    false)`), the mapping to `Command`, and their tests.
  - `shell_text.rs`: `command_text` (`"Show Focus Marking"`).
  - `main.rs`: the action match of the preview's own keys (`A::FocusMarking =>
    self.toggle_focus_marking()`), the menu's ticked state (`Command::FocusMarking =>
    Some(shell.focus_marking)`) and its command (`Command::FocusMarking =>
    self.toggle_focus_marking()`), `wish_focus_map` (called every frame: the first core while
    the preview is shown, the second while the compare is open as well), `draw_image_area`
    (draws one image area; called for the preview and for each side of the compare) and
    `paint_side_text` (a text on a dark plate, `SIDE_TEXT_PLATE`, at the top of a side).
  - `preview_text.rs`: the preview's texts, made whole by functions (rule 13).
  - `docs/glossary.md`: the row "Focus marking, Show Focus Marking".
  - `paint_rating_badge` (`main.rs`) draws the rating badge 8 px from the image area's
    bottom left.
  - `ImageAreaShown` (`main.rs`) carries what `draw_image_area` needs, for example `mode_key`,
    filled from `self.key_map.first_key(…)`; the image-area colours are in `shell.rs`'s
    `image_area` module (`TEXT` is grey 0xbe), the plate is `SIDE_TEXT_PLATE` in `main.rs`.
  - As T313 was made (known and accepted, owner 2026-11-24): an image that cannot be shown
    (`Status::Error`) gives `Paused(Failed)` also in JPEG mode; and after a zoom, when the new
    fine render fails or is refused while an earlier fine view still serves, that fine view stays
    drawn while `Frame::fine` is `Paused(Failed)`. The mark follows `Frame::fine` as it is; this
    task does not change the view core.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/keymap.rs`,
  `crates/pw-loupe/src/lib.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. **The command and the key:** `Command::FineDetails`, everywhere `Command::FocusMarking` is
     handled in `shell.rs`, `keymap.rs` and `lib.rs`, the same way:
     - `keymap.rs`: `KeyAction::FineDetails` declared **right after** `FocusMarking`, and in
       `KeyAction::ALL` at the same place (its length 44 → 45; that order is the order on the
       Keyboard Shortcuts page); name `"fine_details"`, group View, default key `plain(Key::D)`,
       in `acts_in`'s `KeyView::Preview` list; both conversions to and from `Command`; and the
       test helper `today()` maps `PreviewKey::FineDetails` to it.
     - `shell.rs`: `Command::FineDetails` in `acts_in`'s `Where::Loupe` list (the code's name for
       "acts in the preview"), and the View menu lists "Show Fine Details" (D) right after "Show
       Focus Marking", ticked while fine details is on. The existing test
       `the_clipping_commands_have_j_and_shift_j_after_the_zoom_items`, which asserts the exact
       slice of the View menu after the zoom items, is **updated** to hold the new item after
       Focus Marking (its slice grows by one); this change is asked for here and is not a
       weakening (rule 5).
     - `lib.rs`: `PreviewKey::FineDetails`, `(K::D, false, false, false)` in
       `preview_key_action`, and its mapping to `Command::FineDetails`.
     - `shell_text.rs`: `command_text(Command::FineDetails)` is "Show Fine Details".
  2. **The flag:** the app holds `fine_details: bool`, off at start, in the app's own state,
     **not** in `ShellSettings` (not written to `settings.txt`, W2-Q4). D and the menu item
     toggle it, under the same conditions as `E` (the action match of the preview's own keys, not
     while a dialog is open; `command_checked` and `run_command` read and toggle
     `self.fine_details`). Every frame, the first core gets
     `set_fine_details(self.in_preview() && fine_details)` and the second core
     `set_fine_details(self.in_preview() && fine_details && self.compare.is_some())`, in a
     function of its own, `wish_fine_details`, called beside `wish_focus_map`.
  3. **The row of marks** (W2-Q5): in `draw_image_area`, after the image and its overlays, when
     the frame's `fine` is not `FineState::Off` and an image is drawn (`frame.draw` is `Some`):
     the row of adjustment marks at the image area's **bottom right**, on a dark plate as
     `paint_side_text` draws (the same plate colour `SIDE_TEXT_PLATE`, text size and margin). The
     bottom left holds the rating badge (`paint_rating_badge`) and the top the compare's side
     text, so the bottom right meets neither. In
     the compare each side draws its own row from its own frame. Today the row holds one mark,
     "Fine": in `image_area::TEXT` for `Waiting` and `Shown`, and dimmed for `Paused(_)` in a
     new constant `image_area::WEAK_TEXT` in `shell.rs`, `Color32::from_gray(0x80)` (the
     theme's `weak_text` is too dark on the black plate in the Light theme); the existing
     contrast tests of the `image_area` colours get `WEAK_TEXT` against black (the plate is
     black at about two thirds over the image) at `TEXT_CONTRAST`. The hover text (point 4) is
     shown **without a second response** (one over the plate could take the hover from the image
     area's response, which the wheel zoom reads): when the image area's own
     `response.hover_pos()` lies inside the row's plate, the text is shown as a tooltip at the
     pointer (for example `egui::show_tooltip_at_pointer`), so clicks, drags and the wheel still
     reach the image area. The owner's hand check tries the wheel over the row. The key comes from a
     new field of `ImageAreaShown`, `fine_key: Option<KeyChord>`, filled with
     `self.key_map.first_key(KeyAction::FineDetails)` where `mode_key` is filled.
  4. **The marks and their texts** (rule 13), in `preview_text.rs`, none in drawing code: a type
     `AdjustmentMark { text: String, dimmed: bool, hover: String }` and a function
     `adjustment_marks(fine: &FineState, key: Option<KeyChord>) -> Vec<AdjustmentMark>`
     (empty for `Off`; one mark "Fine" otherwise; written so W3 can add "USM" beside it). Each
     hover text is a whole sentence with two templates, one with the key and one without (the
     key formatted with `key_text`, as `preview_text_with_key` does; that function itself is not
     reused, it appends a key to a name):
     - `Waiting` and `Shown`: "Fine details ({key}): below 50%, the RAW view is made from the
       full-resolution rendering.";
     - `Paused(Mode)`: "Fine details ({key}) is paused: it applies only to RAW renderings.";
     - `Paused(Zoom)`: "Fine details ({key}) is paused: it applies only below 50%.";
     - `Paused(Failed)`: "Fine details ({key}) could not be applied to this image.";
     and without a key the same sentences without " ({key})". "50%" is written without a space,
     as the preview's "100%".
  5. **The glossary:** a row "Fine details, Show Fine Details, Fine": the full-resolution RAW
     rendering reduced for views below 50 % (ADR 0027 §3.1, W2), its View menu item (D), and
     its mark in the preview's row of adjustment marks, dimmed while paused; added by T314.
- **Tests to add:**
  - D (no modifiers) is `PreviewKey::FineDetails` and `Command::FineDetails`; Ctrl+D, Shift+D and
    Alt+D are not; the default key map binds `fine_details` to D in the View group, acting in
    the preview, and has no other action on D; `KeyAction::ALL` holds it right after
    `FocusMarking`.
  - The View menu has "Show Fine Details" with D, right after "Show Focus Marking" (the updated
    test of point 1); `Command::FineDetails.acts_in()` is `Where::Loupe`.
  - `command_text(Command::FineDetails)` is "Show Fine Details".
  - `adjustment_marks`: none for `Off`; one "Fine" not dimmed for `Waiting` and `Shown`; one
    dimmed for each `Paused(_)`; each hover text of point 4, with D and with no key.
  - `WEAK_TEXT`'s contrast, as point 3 says.
  (That `fine_details` is not in the settings file needs no new test: the existing test of the
  whole default settings file in `settings.rs` compares it exactly, and `ShellSettings` is not
  changed.)
- **Not in this task:** the view core (T313); the reset of every adjustment (A-Q3, with W3, when
  there is more than one); the USM mark (W3); a fine view for +1 (W2-Q7).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by a function in
  `preview_text.rs` or `shell_text.rs`, the glossary row added). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W2-Q4, W2-Q5); waits for T313 |
| 2026-11-24 | Blocked | Cold read: the View menu's order test is updated (asked for, rule 5); `KeyAction::ALL` 44 → 45 with the new action after `FocusMarking`, `today()` and both `acts_in` lists (`KeyView::Preview`, `Where::Loupe`) named; a new `image_area::WEAK_TEXT` (0x80) for the dimmed mark; the hover by a second hover-only response; the key through a new `ImageAreaShown::fine_key`; the marks function and its type placed in `preview_text.rs` with two templates per text, "50%" as "100%"; the row at the bottom left, only while an image is drawn, so it never meets the compare's side text; the settings test dropped (the whole-file test covers it, `settings.rs` not touched). D checked free |
| 2026-11-24 | Blocked | Chain review (T313's log): the row moved to the bottom right (the bottom left is the rating badge's); the hover text as a tooltip from the image area's own pointer position, not a second response, so the wheel zoom keeps working (hand check); the mark over an earlier image's kept view while the next one is prepared accepted (a moment) |
| 2026-11-24 | Open | T313 Done; what T313 leaves for the mark (an error in JPEG mode, an earlier fine view drawn while paused) noted under the reviewer's checks. Set Open |
| 2026-11-24 | Ready for review | Done as written: `Command::FineDetails` and `KeyAction::FineDetails` (D, after `FocusMarking`, `ALL` 44 → 45), `PreviewKey::FineDetails`, "Show Fine Details"; the app's `fine_details` flag (not a setting) toggled by D and the menu, `wish_fine_details` beside `wish_focus_map`; the row of marks at the bottom right in `draw_image_area` (`paint_adjustment_marks`, the tooltip from the image area's own response by `egui::Tooltip::for_widget(..).at_pointer()`); `AdjustmentMark` and `adjustment_marks` in `preview_text.rs`, re-exported from `lib.rs`; `image_area::WEAK_TEXT`; glossary row. The View menu order test's slice grew by one, as asked. Not named in the task: `shell_text.rs`'s two lists of every menu item (`every_menu_item_text_in_menu_order`, `every_menu_key_text_in_menu_order`) failed without the new item and each got one entry ("Show Fine Details", "D") after Focus Marking; nothing removed (for the reviewer to confirm, rule 5). 6 tests added. The wheel over the row is for the hand check |
| 2026-11-24 | Done | Reviewed: points 1–5 as written, the 6 tests; the two full menu lists in `shell_text.rs` each got one entry, nothing removed (not a weakening, rule 5); `test-inventory.md` counts right (pw-loupe 1441 → 1447, 2630 passed); check.ps1 all OK, no clippy warning. Independent review (fresh review agent, rule on large tasks, 9 files): no bugs; findings accepted by the owner 2026-11-24: (1) a `keys.txt` from 0.5.0 with D on another action loses that line on loading (the key map's rule for a taken key; the first new default key since 0.5.0), written in `CHANGELOG.md`; (2) over a bright image the plate is about grey 0x55, so the dimmed mark has about 1.9:1 (the test holds it against black, as point 3 says), for the hand check; (3) the tooltip shows without a delay, also while panning over the row; (4) in a compare side under about 200 px wide the rating badge can cover the row. Change log line added. The owner's hand check (the wheel over the row) and `latency.ps1` (W2-Q7) follow |
| 2026-11-24 | Done | Hand-checked by the owner and approved: D and the menu item, the mark, the swap (the image does not move). `latency.ps1` on a release build of cb1b5e1 (labels `t314-*`, the A7R and A1 folders of S6a): V1 and V4 hold with fine details on (numbers in ADR 0027 §4.2); the visible gain is small on the owner's 1920×1200 screen. The owner decided (W2-Q7): no task for +1 and no more work on fine details, written in ADR 0027 §4.2 |
