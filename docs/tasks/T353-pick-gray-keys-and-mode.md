# T353. ADR 0027 W6: Pick Gray — the key, the menu item, the pick mode and its click — Blocked

- **Why:** ADR 0027 §3.5 and W6-Q2, W6-Q6, W6-Q8 (answered by the owner 2026-11-28, ADR 0027
  §4.6). T352 gave the view core the click on gray (`ViewCore::pick_grey(point, view_px) ->
  PickResult`, `Frame::white_balance.pick: PickState` and `can_pick`, the mode `Grey`). T351 gave
  the shell the key W, the submenu View ▸ White Balance and the mark. This task adds the way to
  make the click: the key Shift+W and the menu item Pick Gray start a **pick mode**, the next
  click on the image measures, and one line says what to do or what went wrong. The last task of
  W6's chain T348 → T349 → T350 → T351 → T352 → **T353**; the owner's hand check of the whole
  white balance and `latency.ps1` at fit (with auto white balance on, and with every adjustment off,
  against the baseline the reviewer recorded before T348) follow it. Waits for T352 and T351.
  **Spelling:** the program says "gray" (glossary G5, American); ADR 0027 §4.6 wrote "grey"; the
  menu item is "Pick Gray", the mark (T351) "WB gray", and the name in `keys.txt` `pick_gray`.
- **What the reviewer checked (2026-11-28, and by a cold read and a chain review the same day):**
  - `keymap.rs`, `shell.rs`, `lib.rs`: as T351 describes them (the enum `KeyAction` and
    `KeyAction::ALL` with T351's `AutoWhiteBalance` before `ResetAdjustments`; `WHITE_BALANCE_ITEMS`;
    `preview_key_action`; the tests that count the View menu's items and `ALL[place + n]`).
  - `main.rs`: `draw_image_area` (a free function, about line 10641) makes `ui.interact(rect, id,
    egui::Sense::click_and_drag())`, takes `response.double_clicked()` (zoom) and `dragged()` (pan)
    and calls `adjustment_marks`; it has no access to `PreviewExposure` (T351's state), so it
    gets `picking: bool` in `ImageAreaShown` and the pick is **made by its caller** from the
    returned `response`; `zoom_anchor(anchor_at_pointer, pointer, rect, ppp)` returns `None` when
    Zoom at Pointer is off, so it cannot give the click's point; `view_px(rect, ppp)`; where the
    compare's two sides call `draw_image_area` (about lines 10408–10490 and 10434–10438, where the
    held side's `mouse_core` is the current core while the compare is locked: **the pick uses the
    side's own core**, not `mouse_core`); the Esc handling of the preview: Esc is a fixed key
    (`fixed_in_preview`, `keymap.rs` line 722, `FixedKey::ClosePreview`) handled at about lines
    10238–10248, where it closes an open compare first; `last_preview_path != frame.path`
    (about lines 10408 and 10489) is where "another image is shown" is seen; the painters
    (`paint_side_text` draws at the top; the marks' plate is at the bottom right and the rating
    badge at the bottom left of an image area).
  - `egui` with `Sense::click_and_drag`: `response.clicked()` is true on the first release of a
    double-click (`double_clicked()` only on the second), and a drag does not give `clicked()`.
  - `preview_text.rs`, `shell_text.rs`, `docs/glossary.md`: T351's texts and rows.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/keymap.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — about eight files, a mouse mode and the compare's two sides (README, "Model").
- **What:**
  1. **The command and the key** (W6-Q2), everywhere T351's `AutoWhiteBalance` is handled, the
     same way: `PickGray`, in `KeyAction` **and** `KeyAction::ALL` right after `AutoWhiteBalance`
     (the array one longer), name `pick_gray`, default key **Shift+W**, `command_text` "Pick Gray",
     group View, acting in the preview, a `PreviewKey` for Shift+W, and the arms in `command_of_key`,
     `today()`, `key_map_action` and `run_command`; listed in `acts_in`'s `Where::Loupe` list.
  2. **The menu item:** "Pick Gray" (Shift+W) is added to `WHITE_BALANCE_ITEMS` **after** Daylight
     (LibRaw) and before the file's presets. `command_enabled`: in the preview only and only when
     the current frame's `white_balance.can_pick`; `command_checked` (T351's `wb_checked`) ticks it
     while the frame's mode is `Grey`. The key path does not go through `command_enabled`, so the
     key is refused the same way: `toggle_pick` does nothing when `can_pick` is false (a JPEG
     mode, a file that is not RAW or not measurable). The tests that count the View menu's items
     and `ALL[place + n]` and the menu text tests grow by one again (asked for here, not a
     weakening, rule 5).
  3. **The state the shell holds** (W6-Q2), in `PreviewExposure` (not saved): `picking: bool`.
     - `toggle_pick(&mut self, repeat: bool, can_pick: bool)`: Shift+W and Pick Gray turn the mode
       on and off (a repeated key press is ignored; a menu click has `repeat` false); turning it on
       needs `can_pick`;
     - `end_pick(&mut self)`;
     - a pure function `pick_ends(picking, event) -> bool`, with `event` one of: Esc; a balance
       choice (`WhiteBalanceShot`, `Daylight`, a preset, Auto White Balance); **any preview key action
       or command that changes the view** (the zoom keys and Z, Fit, the display mode R, the
       exposure, shadows, channel, fine details and sharpening keys and commands, Reset
       Adjustments / Backspace); another image shown; the preview left; `can_pick` turning false
       (the display mode changed to JPEG while picking); a primary click anywhere outside the
       image areas; and the click itself. The shell calls `end_pick` for each (the list is in one
       place, the function, so it is tested);
     - `reset(…)` also ends the mode.
  4. **The click** (W6-Q2): while `picking`, `draw_image_area` sets the cursor to a cross
     (`egui::CursorIcon::Crosshair`) while the pointer is over the area (it gets `picking` in
     `ImageAreaShown`). The pick is made **by the caller**, for each side that was drawn: when
     `picking` and that side's `response.clicked()` (a drag, which pans, does not give it and does
     not end the mode; **the first click of a double-click is the pick and the second zooms as
     usual: accepted**), it computes the point from `response.interact_pointer_pos()` as
     `[(pos.x − rect.min.x) · ppp, (pos.y − rect.min.y) · ppp]` in a small pure function
     `pointer_in_view(pos, rect, ppp) -> [f64; 2]` (not `zoom_anchor`: it is `None` when Zoom at
     Pointer is off), calls **that side's own core**: `core.pick_grey(point, view_px(rect, ppp))`
     (the held side's `core`, not `mouse_core`; a click's gray belongs to the core it was made
     on, T352), and then `end_pick`. The result `Outside` ends the mode silently, `Unusable` shows
     its line (point 5), `Started` shows "Measuring" until the core's frame says otherwise.
  5. **The lines** (W6-Q2, rule 13), by functions in `preview_text.rs`, whole sentences, American
     spelling: while `picking`: "Click on a neutral gray area. Esc cancels."; while a side's
     frame says `pick` = `Measuring`: "Measuring gray."; `TooBrightOrDark`: "That area is too bright
     or too dark to measure gray."; `Unusable`: "Pick Gray needs a RAW file shown in RAW mode.".
     Drawn as one line of text with the marks' plate's background, **centred at the bottom of an
     image area, above the marks' plate and the rating badge** (a new small painter beside
     `paint_adjustment_marks`; it is clipped to the area as the marks are, so one side of the
     compare never draws on the other): the "Click on" line on **both** sides of an open compare
     while `picking`; the other three on the side whose frame says so, until its `pick` is `Idle`
     again (T352 clears them). They are not part of the marks. The text and when it shows are
     tested by a pure function `pick_line(picking: bool, pick: PickState) -> Option<String>`.
  6. **Esc:** in the `FixedKey::ClosePreview` arm (about lines 10238–10248), **before**
     `close_compare`: when the key is Escape and `picking`, call `end_pick` and do nothing else (the
     preview and the compare stay open). Enter still closes the preview, which also ends the mode
     (point 3).
  7. **The glossary** (rule 13): rows "Pick Gray, click on gray" (the preview's way to set the white
     balance from a neutral area: Shift+W or View ▸ White Balance ▸ Pick Gray, then one click on
     the image; the program measures a small square of the RAW data there; the mark reads "WB
     gray"; it applies to this image only and ends when another image is shown, ADR 0027 §3.5,
     W6-Q2), and the changed rows "White balance, …" (T351; Pick Gray named) and "Reset
     Adjustments".
- **Tests to add:**
  - `keymap.rs`: the name `pick_gray` round trips and is unique; its default key is Shift+W,
    unique among the defaults; `KeyAction::ALL` has it right after `AutoWhiteBalance`; it acts in
    the preview.
  - `lib.rs`: Shift+W gives its `PreviewKey` and command in the preview and nothing in the grid.
  - `shell.rs` and `shell_text.rs`: the White Balance submenu's four static items in order (As
    Shot, Auto White Balance, Daylight (LibRaw), Pick Gray); Pick Gray acts in the preview only;
    the menu texts and keys in order (Shift+W).
  - `main.rs`, without a window: `toggle_pick` turns the mode on and off, a repeat is ignored,
    `can_pick` false does nothing; `pick_ends` for every event of point 3 (each ends it; an event
    that does not change the view, such as a pointer move, does not); `reset` ends it;
    `pointer_in_view` for a rectangle not at the origin and for `ppp` 1.5; `wb_checked` for
    `PickGray` with the frame mode `Grey`; `command_enabled`'s rule for Pick Gray (as a pure
    function of `can_pick`).
  - `preview_text.rs`: `pick_line` gives the four sentences by their states and `None` for `Idle`
    without picking; each has no key (they name Esc in words).
- **Not in this task:** buttons in the status bar for the balance; a key per preset; the Settings
  page; the mark (T351).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (user-visible text as T351's). No new
  dependency. **The reviewer adds, at Done:** the changelog line for the whole white balance, the
  `known-limits.md` items of W6-Q5 (the histogram and the overlays as shot; auto exposure not
  worked out again; the grey-world rule) and the backlog sort.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added (from the cold reads and the chain review of T350 and T351); waits for T351 and T352 |
