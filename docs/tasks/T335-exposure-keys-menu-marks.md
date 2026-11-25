# T335. ADR 0027 W5: the keys, the menu and the marks of the exposure and the shadows — Blocked

- **Why:** ADR 0027 §2.1, §3.4 and W5-Q3, W5-Q8, W5-Q9 (answered by the owner 2026-11-25, ADR
  0027 §4.4). T333 gave the view core the exposure's mode, the manual steps and the shadows
  (`ViewCore::set_exposure_mode`, `step_exposure`, `reset_exposure_steps`, `set_shadows`,
  `Frame::exposure`, `Frame::shadows`), and T334 the saved amount of the shadows
  (`ShellSettings::shadows_amount`). This task lets the user use them: the keys and View ▸
  Exposure ▸ …, View ▸ Show Shadows, the marks in the preview's row of adjustment marks, the
  compare's shared steps, and Reset Adjustments for them. The last task of W5's chain T331 →
  T332 → T333 → T334 → **T335**; the owner's hand check and `latency.ps1` at fit with exposure
  and with auto exposure on follow it. Waits for T334.
- **What the reviewer checked (2026-11-25):** sharpening (T318) goes through every place a
  toggle of the preview goes, and the new commands follow it:
  - `keymap.rs`: `KeyAction::Sharpening` and `ResetAdjustments` (names, `default_keys`, group
    View, acting in `KeyView::Preview`, their places in `KeyAction::ALL`, which is
    `[KeyAction; 47]` at line 155, in `group()`, `acts_in()`, both conversions to and from
    `Command`), the helpers `plain`, `with_ctrl`, `with_shift` (lines 117–144), the test helper
    `today()` and `the_default_map_is_todays_keys`. `fixed_in_preview` (about line 615) fixes
    the arrows without Ctrl only, so Ctrl+↑ and Ctrl+↓ are free; B, Shift+B, O and
    Shift+Backspace are bound to nothing (Ctrl+O is `PreviewKey::Nothing` in the preview).
  - `shell.rs`: the View menu (about lines 1058–1082: `item(Command::FineDetails, …)`,
    `Sharpening`, `ResetAdjustments`), `SubmenuKind` (`Rating`, `Label`, `Theme`, line 903) and
    `MenuEntry::Submenu(kind, &[MenuItem])` (the Theme submenu in the View menu is the model),
    `acts_in`'s `Where::Loupe` list, the test of the View menu's slice after the zoom items
    (about line 2104).
  - `lib.rs`: `PreviewKey` and `preview_key_action` (line 534, matching `(key, ctrl, shift,
    alt)`), with their tests; the mapping of a `PreviewKey` to a `Command` there (about line
    1214) is the test helper `command_of_key` (the tests start at about line 636), used by
    `every_menu_key_gives_its_items_command`. The production path is the key map first
    (`key_map.action_of`, main.rs about line 9306), then `key_map_action`'s match (main.rs
    about lines 5824–5885) and `run_command`.
  - Ctrl+↑ and Ctrl+↓ step nothing (cold read 2026-11-25): the loupe's `key_action` returns
    `None` for any Ctrl (lib.rs about line 373), `fixed_in_preview` returns early for Ctrl,
    `rotation_key_action` covers only Ctrl+← and Ctrl+→, and egui moves the focus only for
    arrows without modifiers.
  - A submenu's items are drawn by `menu_item` (main.rs about lines 8516–8520), which calls
    `command_checked` and `.selected(on)` (about line 8621); `Command::Theme(…)` is ticked so
    today (about line 8212), from the static slice `THEME_ITEMS`.
  - The shell keeps no side's frame: `frame` and `held_frame` are locals of the drawing (main.rs
    about lines 9471 and 9515); it keeps `last_frame_status`, `last_zoom`, `last_clipping` and
    `last_statistics`. `close_compare` empties the second core (about line 6292);
    `open_compare` already sets both cores' files (about lines 6271–6273).
  - `shell_text.rs`: `command_text` and `submenu_text` (line 548).
  - `main.rs`: `fine_details` and `sharpening` in the app's state; `toggle_sharpening`;
    `reset_adjustments` and `any_adjustment_on` (about lines 6172–6180, "W5 and W6 add their
    adjustments here"); `wish_sharpening` (line 6224, the first core while the preview is shown,
    the second while the compare is open as well); `command_checked`, `command_enabled`,
    `run_command`; the action match of the preview's own keys; `ImageAreaShown` (line 9629, with
    `sharpen_key`) and `draw_image_area`'s call of `adjustment_marks` (about line 9780) for each
    side of the compare; `open_compare` and `close_compare`.
  - `preview_text.rs`: `AdjustmentMark`, `adjustment_marks(fine, fine_key, sharpen, sharpen_key,
    values)` (line 206), `fine_mark`, `sharpen_mark`, `key_text`.
  - `docs/glossary.md`: the rows "Sharpening, Show Sharpening, USM" and "Reset Adjustments".
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/keymap.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`, `docs/test-inventory.md` (rule 12),
  and this task's own file (status and log).
- **Model:** Opus — about eight files, and the compare's two cores kept in step (state across
  the shell and two view cores) (README, "Model").
- **What:**
  1. **The commands and the keys** (W5-Q8), everywhere `Command::Sharpening` is handled in
     `keymap.rs`, `shell.rs` and `lib.rs`, the same way: six commands and key actions, group
     View, acting in the preview, in `KeyAction::ALL` right after `Sharpening` in this order
     (`[KeyAction; 53]`):
     | Command / `KeyAction` | name in `keys.txt` | default key | `command_text` |
     |---|---|---|---|
     | `Shadows` | `shadows` | O | "Show Shadows" |
     | `ExposureBrighter` | `exposure_brighter` | Ctrl+↑ | "Brighter" |
     | `ExposureDarker` | `exposure_darker` | Ctrl+↓ | "Darker" |
     | `ResetExposure` | `reset_exposure` | Shift+Backspace | "Reset Exposure" |
     | `AutoExposure` | `auto_exposure` | B | "Auto Exposure" |
     | `CameraBrightness` | `camera_brightness` | Shift+B | "Camera Brightness" |
     `preview_key_action` gets a `PreviewKey` for each of the six chords, `command_of_key` and
     `key_map_action`'s match their arms. On the Keyboard Shortcuts page, where the menu's
     context is missing, `key_action_text` gives "Exposure: Brighter" and "Exposure: Darker"
     for the two (the menu keeps "Brighter" and "Darker" inside its submenu Exposure). A
     repeated key press (`repeat`) of B, Shift+B or O is ignored, so holding them does not flip
     the mode; Ctrl+↑ and Ctrl+↓ do repeat (each a step).
  2. **The menu** (W5-Q8): in the View menu, after "Show Sharpening": "Show Shadows" (O), then
     a submenu "Exposure" (`SubmenuKind::Exposure`, `submenu_text` "Exposure") with Brighter,
     Darker, Reset Exposure, Auto Exposure, Camera Brightness and their keys, then "Reset
     Adjustments" as today. Auto Exposure is ticked while the mode is auto, Camera Brightness
     while it is the camera's, Show Shadows while the shadows are on, drawn as a ticked item
     of the Theme submenu is (a static slice `EXPOSURE_ITEMS` beside `THEME_ITEMS`). These
     existing tests are **updated** to the new items (asked for here, not a weakening, rule 5):
     the test of the View menu's slice (shell.rs about lines 2099–2135); the test at shell.rs
     about lines 2255–2264, which expects `Sharpening` and `ResetAdjustments` side by side right
     after `FineDetails`; the test at shell.rs about lines 2241–2243, which checks only
     `chord.key == Key::Backspace` and expects Reset Adjustments (Reset Exposure,
     Shift+Backspace, is now a submenu item that `menu_items()` holds: the test also compares
     Shift); and `sharpening_is_u_and_reset_adjustments_backspace_right_after_fine_details`
     (keymap.rs about lines 1849–1850, `ALL[place + 2] == ResetAdjustments`).
  3. **The state the shell holds** (A-Q2, W5-Q6): `exposure_mode: ExposureMode` (off at start)
     and `shadows: bool` (off) beside `sharpening`, not in `ShellSettings` (not saved). B sets
     the mode to auto, or off if it is auto; Shift+B the same for the camera's (each turns the
     other off, A-Q12). O toggles `shadows`. Every frame, in a function `wish_exposure` called
     beside `wish_sharpening`: the first core gets `set_exposure_mode(mode)` while
     `self.in_preview()`, otherwise `ExposureMode::Off`, and `set_shadows(Some(
     self.settings.shell.shadows_amount))` while `self.in_preview() && self.shadows`, otherwise
     `None`; the second core the same while the compare is open as well (W5-Q3: the modes apply
     to both sides, each worked out for its own image by its core).
  4. **The manual steps and the compare** (W5-Q3): the steps live in the cores (T333; each core
     resets its own when its image changes). Brighter calls `self.core.step_exposure(1)` and,
     only when that returned `true` and the compare is open, `self.second_core.step_exposure(1)`;
     Darker the same with −1; Reset Exposure calls `reset_exposure_steps()` on both cores. So
     both sides are compared at the same offset. New fields of the app keep, from the frames
     the drawing made, each side's last `Frame::path` (unresolved, as the frame has it), and
     the first core's `source` and `exposure.steps` (for points 5 and 6). When one side's path
     changes while the compare is open, the **other** core's steps are reset (the side whose
     image changed has reset its own). Opening the compare resets both cores' steps (with
     `open_compare`'s own `set_files` it is redundant, and harmless); closing it empties the
     second core, and the first keeps its steps. Leaving the preview (to the contact sheet)
     resets both cores' steps (they last only while an image is shown, A-Q2): `wish_exposure`
     takes `&mut self` and keeps a field `was_in_preview`; when `self.in_preview()` was true
     on the frame before and is false now, it calls `reset_exposure_steps()` on both cores.
     Switching tabs while staying in the preview is not leaving it (the new tab's image is
     another path, which resets the steps through the core).
  5. **Enabled and greyed:** `command_enabled` greys the six commands (after their
     `Where::Loupe` check) while the first core's frame's `source` is not `Source::Raw` (JPEG
     mode, a JPEG or TIFF file; W5-Q8), and Reset Exposure also while its `exposure.steps` is 0.
     A key does not check `command_enabled` (as today): a step there is refused by the core and
     changes nothing; B and Shift+B change the mode, which shows paused.
  6. **The reset** (A-Q3, W3-Q8): `reset_adjustments()` also sets the mode to off, the shadows
     off, and calls `reset_exposure_steps()` on both cores; `any_adjustment_on()` is also true
     while the mode is not off, the shadows are on, or the first core's frame's
     `exposure.steps` is not 0 (from the frame the shell last drew; add a field if none is
     kept).
  7. **The marks** (W5-Q9): `adjustment_marks` also takes the exposure's state, the shadows'
     state, the amount and the keys (for example `exposure: &ExposureState, exposure_keys:
     ExposureKeys` with the keys of Brighter, Darker, Auto Exposure and Camera Brightness,
     `shadows: &AdjustState, shadows_key: Option<KeyChord>, shadows_amount: u8`), and gives after
     "USM" the exposure's mark, then "Shadows". `view == Off` gives no mark; `Paused` a dimmed
     mark; `Waiting` and `Shown` a mark not dimmed. Every text by a function in
     `preview_text.rs` (rule 13), whole sentences, one template with the key and one without
     where a key is named:
     - **The value** (a function `ev_text(ev: Ev) -> String`): a total that is a whole number of
       thirds (`Ev::from_thirds(t) == ev` with `t = (f64::from(milli) * 3.0 / 1000.0).round()`,
       `f64::round`, as an `i32`) is written with "⅓" or "⅔":
       333 "+⅓", 1333 "+1⅓", −667 "−⅔", 3000 "+3", 0 "0"; any other with one decimal, the
       tenths `(milli / 100)` rounded half away from zero (`f64::round`): −510 "−0.5", −177
       "−0.2", 250 "+0.3", 40 "+0.0". A sign always for a non-zero value (also when it rounds to
       0.0), none for 0; the minus is "−" (U+2212).
     - **The other numbers:** `{base}` is `ev_text(Ev::from_thirds(t))`; `{steps}` is
       `ev_text(Ev::from_thirds(steps))`; `{v}`, the file's value, has two decimals with the sign
       as above ("−0.51", "+0.70", "0.00"); `{limit}` is "−2" or "+3".
     - **The exposure's mark text:** mode off: "{value} EV"; auto: "{value} EV auto"; the
       camera's: "{value} EV camera", except `CameraNone` with no steps: "Camera EV: none".
     - **Its hover text**, by the base (with "{steps}" the steps as `ev_text(Ev::from_thirds(
       steps))`, and the second form of each when the steps are not 0):
       - `Manual`: "Exposure: {value} EV by hand ({brighter} brighter, {darker} darker)."; when
         either key is missing "Exposure: {value} EV by hand.";
       - `Auto(t)`: "Auto exposure ({key}): {base} EV from the histogram." / "Auto exposure
         ({key}): {base} EV from the histogram, plus {steps} EV by hand.";
       - `AutoClipped`: "Auto exposure ({key}): 0 EV, since more than 0.1% of the RAW data is
         clipped." / "…, plus {steps} EV by hand." as one sentence;
       - `AutoNone`: "Auto exposure ({key}): 0 EV, since the file has no usable RAW statistics." / with
         the steps as above;
       - `AutoWaiting`: "Auto exposure ({key}): the histogram is being read.";
       - `CameraWaiting`: "Camera brightness ({key}): the file is being read.";
       - `Camera(v)`: "Camera brightness ({key}): {v} EV from the file." with `v` to two
         decimals; a `v` outside −2..+3: "Camera brightness ({key}): {v} EV from the file,
         limited to {limit} EV." (−2 or +3); each with a form "…, plus {steps} EV by hand.";
       - `CameraNone`: "Camera brightness ({key}): the file gives no value, so 0 EV is used." /
         with the steps;
       - `view == Waiting` after a change (not `AutoWaiting`): "Exposure: the view is being made
         again.";
       - `Paused`: "Exposure is paused: it applies only to RAW renderings.".
     - **The shadows' mark** "Shadows"; hover: `Shown` "Shadows ({key}): the dark tones are lifted
       by {amount}."; `Waiting` "Shadows ({key}): the view is being made again."; `Paused`
       "Shadows ({key}) is paused: it applies only to RAW renderings."; each also without the
       key.
     `draw_image_area` passes each side's `frame.exposure` and `frame.shadows`; the keys from
     `self.key_map.first_key(…)` and the amount from the settings, filled where `sharpen_key` is
     (`ImageAreaShown`). `paint_adjustment_marks` is not changed.
  8. **The glossary** (rule 13): rows "Exposure, Brighter, Darker, Reset Exposure" (the
     temporary exposure of the RAW preview in ⅓ EV, −2 to +3 EV, never saved; the submenu View ▸
     Exposure and its keys; its mark, for example "+1⅓ EV"), "Auto exposure, Auto Exposure" (the
     exposure worked out from the RAW histogram, A-Q11; mark "… EV auto"), "Camera brightness,
     Camera Brightness" (the camera's own brightness offset, DNG BaselineExposure and the
     Fujifilm value; mark "… EV camera" or "Camera EV: none"), "Shadows, Show Shadows" (the lift
     of the dark tones of the RAW preview; mark "Shadows"; the amount on the Settings page
     Preview, T334's row; not the "Deep shadows" of the clipping marks); the row "Reset
     Adjustments" names them too. The exposure rows say the histogram shows the unexposed RAW
     data (A-Q6), and so do the clipping marks and the focus marking.
- **Tests to add:**
  - The six chords give their `PreviewKey`s and commands; Ctrl+Alt+↑, Ctrl+Shift+↑, Alt+B and
    Ctrl+B are none of them; Ctrl+↑ and Ctrl+↓ are not a fixed key in the preview; the default
    key map binds each action to its key in the View group, acting in the preview, with no
    other action on it; `KeyAction::ALL` holds them right after `Sharpening` in the order of
    point 1.
  - The View menu: "Show Shadows" (O) after "Show Sharpening", then the submenu "Exposure" with
    its five items and keys, then "Reset Adjustments" (the updated slice test); the six commands
    act in `Where::Loupe`; `command_text` and `submenu_text` give the texts of points 1–2.
  - `ev_text` for 0 "0", 333 "+⅓", 667 "+⅔", 1000 "+1", 1333 "+1⅓", −333 "−⅓", −667 "−⅔",
    −2000 "−2", 3000 "+3", −510 "−0.5", −177 "−0.2", 250 "+0.3", 40 "+0.0".
  - `key_action_text` gives "Exposure: Brighter" and "Exposure: Darker"; the menu items
    "Brighter" and "Darker".
  - `adjustment_marks`: the existing tests are **updated** to the new signature (exposure and
    shadows off; their expectations as before; asked for here, rule 5); every mark text and
    hover text of point 7, with the default keys and with no keys; the order Fine, USM, the
    exposure, Shadows; `Paused` dimmed; `Off` no mark.
  - What is testable without a window in `main.rs`'s tests (as T318's were placed): B twice
    gives auto then off; B then Shift+B gives the camera's; a repeated B changes nothing;
    `reset_adjustments` turns the mode and the shadows off; leaving the preview resets both
    cores' steps.
- **Known and accepted (as for U in T318; the reviewer writes these at Done in a new
  `known-limits.md` area "Exposure and shadows (ADR 0027 W5)", and one `CHANGELOG.md` line under
  Added for the whole of W5):** a `keys.txt` that binds one of the six default
  keys to another action acting in the preview loses that line on loading (the key map's rule).
  The histogram, the clipping marks and the focus marking are made from the unexposed RAW data
  (A-Q6), so at +2 EV the clipping marks do not match the picture. In the compare with a JPEG or
  TIFF file (or JPEG mode) on the first side, the six commands are greyed and a step is refused
  there, so a RAW file on the second side cannot be stepped by hand (its modes and shadows still
  apply). In the compare, a step
  the second core refuses at its own limit (its image's base is higher or lower) leaves the two
  sides one step apart until a reset; each mark shows its own total. A step made in the same
  frame as a step to another image on one side of the compare is kept on that side and reset on
  the other by point 4's rule (the keys run before the drawing); rare, and the marks show it.
- **Not in this task:** the view core (T332, T333); the amount on the Settings page (T334); a
  button in the title line (W5-Q8: the hand check may ask for one); the owner's `latency.ps1`
  and the hand check, which tunes `exp_preser`, the auto rule and the default amount.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by a function
  in `preview_text.rs` or `shell_text.rs`, the glossary rows). No new dependency. An extra code
  review follows (about eight files).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for T334. Cold read: every cited line found; Ctrl+↑/↓ step nothing, and a submenu's item can be ticked (both recorded under "What the reviewer checked", the "stop and report" points removed). Fixed: three more existing tests to update named; `command_of_key` is test code, the production path named; the fields the shell must keep named; only the other core is reset on a path change; leaving the preview resets the steps; repeats of B, Shift+B and O ignored; `ev_text`'s rounding and every number's format; `CameraWaiting`'s text; the Manual hover with one key; "Exposure: Brighter" on the Keyboard Shortcuts page; the glossary apart from "Deep shadows" |
