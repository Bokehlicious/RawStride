# T318. ADR 0027 W3: the key, the menu item and the mark of sharpening, and the reset — Blocked

- **Why:** ADR 0027 §2.1, §3.3, W3-Q5, W3-Q6 and W3-Q8 (answered by the owner 2026-11-24, ADR
  0027 §4.3). T316 gave the view core sharpening (`ViewCore::set_sharpening`, `Frame::sharpen`)
  and T317 the saved values (`ShellSettings::sharpening`). This task lets the user turn sharpening
  on and off with **U** and View ▸ Show Sharpening, passes the saved values to the view core, adds
  the mark "USM" to the preview's row of adjustment marks (made by T314), and adds the reset of
  every adjustment (A-Q3): **Backspace** and View ▸ Reset Adjustments, which turn off fine details
  and sharpening. Sharpening is not saved: it lasts for the session (A-Q2). The last task of the
  chain T315 → T319 → T316 → T317 → **T318**; the owner's `latency.ps1` with sharpening on and the hand
  check, which sets the defaults, follow it. Waits for T317.
- **What the reviewer checked (2026-11-24):** fine details (T314) goes through every place a
  toggle of the preview does that is not a setting, and sharpening follows it:
  - `keymap.rs`: `KeyAction::FineDetails` (name `"fine_details"`, `default_keys` `plain(Key::D)`,
    group View, acting in `KeyView::Preview`, its place in `KeyAction::ALL` right after
    `FocusMarking`), `of_command` and `command`, and the test helper `today()`; the test
    `the_default_map_is_todays_keys`. **U** and **Backspace** are bound to nothing today, and
    Backspace is not a fixed key (`is_fixed`, `KeyMap::allowed`).
  - `shell.rs`: `Command::FineDetails` in `acts_in`'s `Where::Loupe` list, and the View menu's
    `item(Command::FineDetails, plain(Key::D))` (about line 1057); the test of the View menu's
    slice after the zoom items (about line 2104) and `acts_in`'s test.
  - `lib.rs`: `PreviewKey::FineDetails`, `(K::D, false, false, false)` in
    `preview_key_action`, its mapping to `Command::FineDetails` (about line 1214), and their tests.
  - `shell_text.rs`: `command_text(Command::FineDetails)` is "Show Fine Details".
  - `main.rs`: `fine_details: bool` in the app's state; `toggle_fine_details`; the action match
    of the preview's own keys; `command_checked` (`Command::FineDetails =>
    Some(self.fine_details)`) and `run_command`; `command_enabled` (by `acts_in`, then per
    command, for example Move Picked greyed while its setting is off); `wish_fine_details`
    (called every frame: the first core while the preview is shown, the second while the compare
    is open as well); `draw_image_area` calls `adjustment_marks(&frame.fine, shown.fine_key)` and
    `paint_adjustment_marks` while `frame.draw` is `Some`; `ImageAreaShown::fine_key` from
    `self.key_map.first_key(KeyAction::FineDetails)`.
  - `preview_text.rs`: `AdjustmentMark { text, dimmed, hover }` and `adjustment_marks(fine, key)`
    with the hover texts of fine details (with and without a key, `key_text`), and their tests.
  - `docs/glossary.md`: the row "Fine details, Show Fine Details, Fine" and T317's row
    "Sharpening".
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/keymap.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. **The commands and the keys:** `Command::Sharpening` and `Command::ResetAdjustments`,
     everywhere `Command::FineDetails` is handled in `shell.rs`, `keymap.rs` and `lib.rs`, the
     same way:
     - `keymap.rs`: `KeyAction::Sharpening` (name `"sharpening"`, default `plain(Key::U)`) and
       `KeyAction::ResetAdjustments` (name `"reset_adjustments"`, default
       `plain(Key::Backspace)`), both group View, acting in `KeyView::Preview`, declared and in
       `KeyAction::ALL` **right after** `FineDetails`, in that order (`[KeyAction; 45]`, about
       line 153, becomes 47); in **both** lists `group()` (about line 286) and `acts_in()` (about
       line 310), and in `default_keys`; both conversions to and from `Command`; the test helper `today()` maps the two new
       `PreviewKey`s to them.
     - `shell.rs`: both in `acts_in`'s `Where::Loupe` list; the View menu lists "Show Sharpening"
       (U) and then "Reset Adjustments" (Backspace) right after "Show Fine Details"; Show
       Sharpening is ticked while sharpening is on. The existing test of the View menu's slice
       after the zoom items (about line 2097, `anchor + 1..anchor + 11`, and its comment at about
       line 2084) is **updated** to `anchor + 1..anchor + 13` with the two new items (asked for
       here, not a weakening, rule 5).
     - `lib.rs`: `PreviewKey::Sharpening` for `(K::U, false, false, false)` and
       `PreviewKey::ResetAdjustments` for `(K::Backspace, false, false, false)` in
       `preview_key_action`, and their mappings to the two commands.
     - `shell_text.rs`: `command_text` "Show Sharpening" and "Reset Adjustments".
  2. **The flag and the values:** the app holds `sharpening: bool`, off at start, in the app's own
     state beside `fine_details` (`sharpening: false` beside `fine_details: false` in the
     constructor, about main.rs line 13199), **not** in `ShellSettings` (not written to
     `settings.txt`, W3-Q5). U and the menu item toggle it under the same conditions as D: both
     new actions go in `key_map_action`'s list of the preview's own keys (about main.rs lines
     5825–5848, behind `!self.dialog_open()`), and `command_checked` gives
     `Command::Sharpening => Some(self.sharpening)` and `Command::ResetAdjustments => None`. Every frame, in a
     function of its own, `wish_sharpening`, called beside `wish_fine_details`: the first core
     gets `set_sharpening(Some(self.settings.shell.sharpening))` while `self.in_preview() &&
     sharpening`, otherwise `set_sharpening(None)`; the second core the same while the compare
     is open as well. So a value saved on the page Preview (T317) applies at Save.
  3. **The reset** (W3-Q8): two functions in main.rs, `reset_adjustments()`, which turns
     `fine_details` and `sharpening` off (the cores follow on the next frame through the two
     `wish_` functions), and `any_adjustment_on() -> bool`. The key (from the preview's key list)
     and `run_command` both call `reset_adjustments()`; `command_enabled` greys the menu item,
     after its `Where::Loupe` check, while `any_adjustment_on()` is false (a key does not check
     `command_enabled`; the reset then changes nothing, which is harmless). It changes no setting.
     W5 and W6 add their adjustments to these two functions.
  4. **The mark** (W3-Q6): `adjustment_marks` takes the state of both adjustments, for example
     `adjustment_marks(fine: &FineState, fine_key: Option<KeyChord>, sharpen: &SharpenState,
     sharpen_key: Option<KeyChord>, values: Sharpening) -> Vec<AdjustmentMark>`, and gives
     "Fine" (unchanged) and then **"USM"**: none for `SharpenState::Off`; not dimmed for
     `Waiting` and `Shown`; dimmed for `Paused` (T316's `Paused` is "the scale is 100 % or
     more"). Hover texts (rule 13; whole sentences, one template with the key and one without,
     the key by `key_text`; the radius written as `"{}.{}"` of `radius_tenths / 10` and
     `radius_tenths % 10`, so 6 gives "0.6"); `preview_text.rs` imports `SharpenState` and
     `Sharpening` from `pw_viewcore`:
     - `Shown`: "Sharpening ({key}): amount {amount}%, radius {radius} px, threshold
       {threshold}.";
     - `Waiting` (not dimmed): "Sharpening ({key}): the view is being made again." (the new
       values are not drawn yet, or a sharpened view is still drawn after turning it off;
       chain review);
     - `Paused`: "Sharpening ({key}) is paused: it applies only below 100%.";
     and without a key the same sentences without " ({key})". `draw_image_area` calls it with
     `frame.sharpen`, two new fields of `ImageAreaShown` (about main.rs line 9527),
     `sharpen_key` (filled with `self.key_map.first_key(KeyAction::Sharpening)` where `fine_key`
     is filled, about line 9356) and `sharpening: Sharpening` (filled from
     `self.settings.shell.sharpening` there). `paint_adjustment_marks` is not changed: it already
     lays out several marks on one plate with a hover area per mark.
  5. **The glossary:** the row "Sharpening" (T317's) gains "Show Sharpening" (U) in the View menu,
     ticked when on, and its mark "USM" in the preview's row of adjustment marks, dimmed while
     paused; a new row "Reset Adjustments": the View menu item (Backspace) that turns off every
     adjustment of the preview (ADR 0027 A-Q3, W3-Q8); added by T318.
- **Tests to add:**
  - U (no modifiers) is `PreviewKey::Sharpening` and `Command::Sharpening`; Backspace is
    `PreviewKey::ResetAdjustments` and `Command::ResetAdjustments`; Ctrl+U, Shift+U, Alt+U and
    Ctrl+Backspace are neither; the default key map binds `sharpening` to U and
    `reset_adjustments` to Backspace in the View group, acting in the preview, with no other
    action on either; `KeyAction::ALL` holds them right after `FineDetails`.
  - The View menu has "Show Sharpening" with U and "Reset Adjustments" with Backspace right after
    "Show Fine Details" (the updated test of point 1); both commands act in `Where::Loupe`.
  - `command_text` gives "Show Sharpening" and "Reset Adjustments".
  - The existing test `the_adjustment_marks_follow_fine_details` (`preview_text.rs` about line
    466) is **updated** to the new signature (sharpening `Off`; its keys and expectations as
    before; asked for here, not a weakening, rule 5).
  - `adjustment_marks`: with both off, none; fine off and sharpening `Shown`, one "USM" not
    dimmed; `Waiting` one "USM" not dimmed with the "being made again" text; both on, "Fine" then "USM"; `Paused` gives "USM" dimmed; each hover text of point 4
    with U and with no key, with the default values (60, 0.6, 2) and with others (150, 1.3, 7).
  (That `sharpening` is not in the settings file needs no new test: `ShellSettings` gains no
  field in this task, and the test of the whole default file compares it exactly.)
- **Known and accepted (as for D in T314, owner 2026-11-24):** a `keys.txt` that binds U or
  Backspace to another action acting in the preview loses that line on loading (the key map's
  rule for a taken key: the new default holds it); the reviewer writes it in `CHANGELOG.md` and
  `known-limits.md` at Done. Holding U toggles at the key-repeat rate, as D does (the preview's
  key list ignores nothing for repeats); each toggle plans new views.
- **Not in this task:** the view core (T316); the page Preview and the saved values (T317); the
  owner's `latency.ps1` and the defaults (the hand check).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by a function in
  `preview_text.rs` or `shell_text.rs`, the glossary rows added). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W3-Q5, W3-Q6, W3-Q8); waits for T317 |
| 2026-11-24 | Blocked | Cold read: the existing test `the_adjustment_marks_follow_fine_details` named as updated; `ImageAreaShown::sharpening` added for the values; the constructor, `key_map_action`'s list and `command_checked` named; the reset as `reset_adjustments()` and `any_adjustment_on()` (keys do not check `command_enabled`); `group()` and `acts_in()` both named, `ALL` 45 → 47; the View menu test's range to +13; no change to `paint_adjustment_marks`; the radius format and the imports named; a `keys.txt` with U or Backspace on another action is accepted as for D. U and Backspace checked free and not fixed. Waits for T317 and the chain review |
| 2026-11-24 | Blocked | Chain review: `Waiting` gets a hover text of its own ("the view is being made again"), since T316 now gives `Waiting` also while a sharpened view is still drawn after turning off; holding U accepted as for D. Waits for T317 |
