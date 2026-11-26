# T347. ADR 0027 W4: the keys, the menu and the mark of the channel view — Done

- **Why:** ADR 0027 §2.1, §3.6 and W4-Q5, W4-Q6 (answered by the owner 2026-11-26, ADR 0027
  §4.5). T346 gave the view core the channel (`pw_viewcore::Channel` with `next` and
  `previous`, `ViewCore::set_channel`, `Frame::channel: AdjustState`, never `Paused`). This task
  lets the user use it: the keys N and Shift+N, the submenu View ▸ Channel, the mark in the
  preview's row of adjustment marks, both sides of the compare, and Reset Adjustments for it.
  The last task of W4's chain T346 → **T347**; the owner's hand check and `latency.ps1` at fit
  with a channel shown follow it. Waits for T346.
- **What the reviewer checked (2026-11-26):** the shadows (T335) and sharpening (T318) go through
  every place a toggle of the preview goes, and the new commands follow them:
  - `keymap.rs`: the enum `KeyAction` (about lines 91–115; `index()` is `self as usize`, line
    232, checked by `names_round_trip_and_are_unique`, about line 1489) and `KeyAction::ALL`,
    `[KeyAction; 55]` (line 172), both with `Sharpening`, `Shadows`, `ExposureBrighter`,
    `ExposureDarker`, `ResetExposure`, `AutoExposure`, `CameraBrightness`, `ResetAdjustments`
    in that order; the names for `keys.txt` (about line 277), `group()` and `acts_in()` (about
    lines 340 and 375), `default_keys` (line 394; N and Shift+N are bound to nothing),
    `of_command` (line 458, `_ => return None` for commands without a key, as
    `Command::Theme(…)`; no test breaks when a menu command has none) and `command` (line 518).
    The test helper `today()` (about line 1203) matches `PreviewKey` exhaustively (about lines
    1215–1243). The test at about line 1966 takes `place` as `FineDetails`' place and asserts
    `KeyAction::ALL[place + 8] == ResetAdjustments` (about line 1971);
    `exposure_and_shadows_have_unique_default_keys_and_follow_sharpening` (about line 1975) is
    the model for the new keys' test. `fixed_in_preview` (line 688) does not fix N.
  - `shell.rs`: `Command` (line 717) with `Theme(Theme)` (line 841) as the model of a command
    with a value; `SubmenuKind` (line 926, with `Theme` and `Exposure`); `EXPOSURE_ITEMS` (line
    1014) and the Theme items (about line 1039) as static slices of `MenuItem` (a slice cannot
    hold a separator); the View menu (about lines 1105–1125): `Shadows` (O),
    `MenuEntry::Submenu(SubmenuKind::Exposure, EXPOSURE_ITEMS)`, then `ResetAdjustments`
    (Backspace); `acts_in`'s `Where::Loupe` list (about line 890; a command not listed acts in
    `Where::Both`); the test of the View menu's slice after Zoom at Pointer (about lines
    2159–2200, `anchor + 1..anchor + 15`) and the test at about line 2340 (`fine + 1..fine +
    5`).
  - `lib.rs`: `PreviewKey` (line 496) and `preview_key_action` (line 550; `(K::O, false, false,
    false) => Some(PreviewKey::Shadows)` at line 567) and the test helper `command_of_key` (line
    1172), with their tests (about lines 1880–1935).
  - `main.rs`: `PreviewExposure` (line 1364, "Session adjustments shared by the current and held
    previews", `#[derive(Default)]`) holds `exposure_mode` and `shadows`, with `toggle_shadows(
    repeat)` (line 1406, a repeated key is ignored there), `reset(cores)` and `wish(…)` (about
    line 1420, which sets both cores each frame: the first while the preview is shown, the
    second while the compare is open as well); its tests (about line 15122,
    `cores_with_manual_steps`; for example
    `exposure_toggles_between_off_auto_and_camera_and_ignores_repeats`, about line 15133) test
    it without a window. The app (`Loupe`) is built only in `main()` and has no test
    constructor. `key_map_action` (line 6504; the list of preview actions about lines 6540–6556
    and their match about lines 6560–6580); `reset_adjustments` and `any_adjustment_on` (lines
    6868 and 6875); `command_enabled` (line 8838, `_ => true` at its end), `command_checked`
    (line 8919, with `Command::Theme(theme) => Some(shell.theme == theme)` at line 8944),
    `run_command` (line 8953, exhaustive, no `_` arm); a submenu's items are drawn by
    `menu_item` with `.selected(on)` (about lines 9364–9372), so a ticked radio item needs no
    new drawing code; `ImageAreaShown` (line 10537, with `shadows_key` and `shadows_amount`,
    built once per frame about lines 10325–10340 and used by `draw_image_area` for each side
    with that side's own `frame`) and the call of `adjustment_marks` (line 10692).
    `draw_image_area` carries `#[allow(clippy::too_many_arguments)]` with a reason comment
    (about line 10595).
  - `preview_text.rs`: `adjustment_marks` (line 221) already has seven parameters (fine,
    sharpening, the exposure as a tuple, the shadows as a tuple), the clippy limit;
    `shadows_mark` (line 539) is the model of a mark with and without a key; `key_text`.
  - `shell_text.rs`: `command_text`, `submenu_text` (about line 576), `key_action_text` (line
    432); the tests `every_menu_item_text_in_menu_order` (about line 775) and
    `every_menu_key_text_in_menu_order` (about line 853) list every menu item in order.
  - `docs/glossary.md`: G5 (line 329, American spelling: "Color", as "color profile" in the
    program); the rows "Shadows, Show Shadows" (line 297) and "Reset Adjustments" (line 298).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/keymap.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`, `docs/test-inventory.md` (rule 12),
  and this task's own file (status and log).
- **Model:** Opus — about eight files, and the two cores of the compare kept in step (README,
  "Model").
- **What:**
  1. **The commands and the keys** (W4-Q5), everywhere `Command::Shadows` is handled in
     `keymap.rs`, `shell.rs`, `lib.rs` and `main.rs`, the same way: two commands and key
     actions, group View, acting in the preview, in the enum `KeyAction` **and** in
     `KeyAction::ALL` right after `CameraBrightness` and before `ResetAdjustments`
     (`[KeyAction; 57]`):
     | Command / `KeyAction` | name in `keys.txt` | default key | `command_text` |
     |---|---|---|---|
     | `NextChannel` | `next_channel` | N | "Next Channel" |
     | `PreviousChannel` | `previous_channel` | Shift+N | "Previous Channel" |
     and one command with a value, `Command::Channel(Channel)` (the five channels as radio
     items, as `Command::Theme(…)`: no key, no `KeyAction`, `of_command` gives `None`), whose
     `command_text` is "Color", "Red", "Green", "Blue" or "Luminance". `preview_key_action`
     gets a `PreviewKey` for N and Shift+N; `command_of_key`, `today()` and `key_map_action`'s
     list and match their arms; `run_command` gets three arms (it has no `_`). The three
     commands are listed in `acts_in`'s `Where::Loupe` list.
  2. **The menu** (W4-Q5): in the View menu, after the submenu Exposure and before Reset
     Adjustments, a submenu "Channel" (`SubmenuKind::Channel`, `submenu_text` "Channel") from a
     static slice `CHANNEL_ITEMS`, without a separator: Next Channel (N), Previous Channel
     (Shift+N), then `Channel(Color)`, `Channel(Red)`, `Channel(Green)`, `Channel(Blue)`,
     `Channel(Luminance)` without keys. The channel shown is ticked (`command_checked`:
     `Command::Channel(c) => Some(c == …)`, from the state of point 3), drawn as Theme's items
     are (no new drawing code). All seven are enabled in the preview in every mode and for
     every file (§2.2: the channel view applies in RAW and JPEG mode, to JPEG and TIFF files).
     These existing tests are **updated** to the new items (asked for here, not a weakening,
     rule 5): the test of the View menu's slice (shell.rs about lines 2159–2200, `anchor + 15`
     becomes `anchor + 16`); the test at shell.rs about line 2340 (`fine + 5` becomes `fine +
     6`); `every_menu_item_text_in_menu_order` and `every_menu_key_text_in_menu_order`
     (shell_text.rs, the seven texts, and the keys N, Shift+N and five `None`s, after "Camera
     Brightness" / Shift+B); and keymap.rs about line 1971 (`ALL[place + 8]` becomes
     `ALL[place + 10]`, `place` being `FineDetails`' place).
  3. **The state the shell holds** (A-Q2): `channel: Channel` (`Color` at the start) in
     `PreviewExposure`, beside `shadows`, not in `ShellSettings` (not saved), so it can be
     tested as the exposure's state is:
     - `step_channel(&mut self, forward: bool, repeat: bool)`: a repeated key press is ignored
       (as `toggle_shadows`, since every press makes the view again, W4-Q4); otherwise
       `channel.next()` or `channel.previous()`. Next Channel and Previous Channel call it (a
       menu click with `repeat` false);
     - `set_channel(&mut self, c: Channel)` for `Command::Channel(c)`;
     - `reset(…)` also sets `Color`;
     - `wish(…)` also calls `set_channel` on both cores: the first gets `self.channel` while the
       preview is shown, otherwise `Channel::Color`; the second the same while the compare is
       open as well (W4-Q1: both sides). No separate `wish_channel`.
     - `command_checked`'s tick and point 4's "any on" read it through small methods of
       `PreviewExposure` (for example `channel()`), so the tests need no `Loupe`.
  4. **The reset** (A-Q3, W3-Q8): `reset_adjustments()` sets the channel to `Color` (through
     `PreviewExposure::reset`); `any_adjustment_on()` is also true while the channel is not
     `Color`. Update the doc comment of `reset_adjustments` ("W6 adds its adjustments here") to
     name the channel view as well.
  5. **The mark** (W4-Q6): `adjustment_marks` also takes `(channel_state, channel,
     channel_key)`: `(&AdjustState, Channel, Option<KeyChord>)`, with each side's own frame's
     `channel` state, the shell's channel, and the key map's first key of Next Channel. That
     makes eight parameters, above clippy's limit: put `#[allow(clippy::too_many_arguments)]`
     on it with a reason comment, as `draw_image_area` has (main.rs about line 10595); the
     existing parameters stay as they are. It gives the channel's mark **after** "Shadows". By
     the state and the channel:
     - `Off`, whatever the channel: no mark.
     - `Shown` or `Waiting` with Red, Green, Blue or Luminance: the mark "R", "G", "B" or "Lum",
       not dimmed. While `Waiting` it already shows the new channel's letter although the old
       channel's view is still drawn (the mark counts as waiting, W4-Q6).
     - `Waiting` with `Color` (a view of another channel still drawn after a return to color,
       W4-Q6): the mark "RGB", not dimmed.
     - `Shown` with `Color` cannot happen (`adjust_state` gives only `Off` or `Waiting` while
       the channel is off); if it comes, no mark. `Paused` cannot happen (T346); if it comes, it
       is drawn as `Waiting`.
     Every text, the mark texts "R", "G", "B", "Lum" and "RGB" included, by a function in
     `preview_text.rs` (rule 13), not in `paint_adjustment_marks`; whole sentences, one
     template with the key and one without:
     - `Shown` (American spelling, G5: "gray"): "Channel view ({key}): the red channel as gray.", "… the green channel as gray.", "… the blue
       channel as gray.", "Channel view ({key}): the luminance as gray."; without a key
       "Channel view: the red channel as gray." and so on (each channel its own template, so
       another language can word each).
     - `Waiting` with a channel: "Channel view ({key}): the view is being made again." /
       "Channel view: the view is being made again."
     - `Waiting` with `Color`: "Channel view ({key}): the color view is being made again." /
       "Channel view: the color view is being made again."
     The key and the channel are filled where `shadows_key` is (`ImageAreaShown`), and
     `draw_image_area` passes each side's `frame.channel`. `paint_adjustment_marks` is not
     changed.
  6. **The glossary** (rule 13): a row "Channel view, Channel, Next Channel, Previous Channel,
     Color, Red, Green, Blue, Luminance" (the preview shows one channel, or the luminance, as
     gray, in RAW and JPEG mode, never saved; the submenu View ▸ Channel and its keys N and
     Shift+N; its marks "R", "G", "B", "Lum" and "RGB"; the histogram, the clipping marks and
     the focus marking do not change; a TIFF's channel is the display's, W4-Q3); the row "Reset
     Adjustments" names the channel view too.
- **Tests to add:**
  - N and Shift+N give their `PreviewKey`s and commands; Ctrl+N, Alt+N and Ctrl+Shift+N are
    neither; N is not a fixed key in the preview; the default key map binds each action to its
    key in the View group, acting in the preview, with no other action on it; `KeyAction::ALL`
    holds them right after `CameraBrightness` and before `ResetAdjustments`; their names in
    `keys.txt` are `next_channel` and `previous_channel`; `of_command(Command::Channel(_))` is
    `None`.
  - The View menu: the submenu "Channel" after the submenu Exposure and before Reset
    Adjustments, with its seven items and the two keys (the updated slice test); the three
    commands act in `Where::Loupe`; `command_text` and `submenu_text` give the texts of points
    1–2.
  - `adjustment_marks`: the existing tests are **updated** to the new signature (the channel
    off; their expectations as before; asked for here, rule 5); every mark text and hover text
    of point 5, with the default key and with no key; the order Fine, USM, the exposure,
    Shadows, the channel; `Off` with Red gives no mark; `Shown` with `Color` gives no mark;
    `Waiting` with `Color` gives "RGB".
  - `PreviewExposure` (with `cores_with_manual_steps`, as the exposure's tests): N three times
    gives Red, Green, Blue; Shift+N from Color gives Luminance; a repeated N changes nothing;
    `set_channel(Blue)` sets Blue and the tick is true only for Blue; `reset` sets Color; "any
    on" follows the channel; `wish` gives the first core the channel in the preview and Color
    outside it, and the second core the channel only while the compare is open.
- **Known and accepted (the reviewer writes these at Done in `known-limits.md`'s area "The
  channel view (ADR 0027 W4)", and one `CHANGELOG.md` line under Added for W4):** a `keys.txt`
  that binds N or Shift+N to another action acting in the preview loses that line on loading
  (the key map's rule, as for O, B and U). The histogram, the clipping marks and the focus
  marking show the image's data, not the channel (§2.1). Each change of channel makes the shown
  image's and the prefetched neighbours' views again (W4-Q4). A view that fails to render with
  a channel shows today's error line, and the mark stays "being made again" (as the shadows').
  The `latency.ps1` result at fit with a channel shown is written in this task's log and in W4's
  row in `plans/adr-0018-0025-steps.md`.
- **Not in this task:** the view core (T346); a button in the title line (W4-Q5: the hand check
  may ask for one); the owner's `latency.ps1` and the hand check.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by a function
  in `preview_text.rs` or `shell_text.rs`, the glossary row; American spelling, G5). No new
  dependency. An extra code review follows (about eight files).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Written; waits for T346. Cold read (fresh agent): the named files suffice; added the enum `KeyAction` beside `ALL`, `today()`, the two menu-order tests of `shell_text.rs`, `fine + 6`, `run_command`'s three arms, `acts_in`; the state moved into `PreviewExposure` so the tests need no `Loupe`; the repeat handled in `step_channel`; clippy's argument limit (an allow with a reason); no separator in the submenu; `Shown` with Color and `Off` with a channel give no mark; the mark texts in `preview_text.rs`; "Color" and "gray" by glossary G5. Chain review (T346–T347, fresh agent, noted in T346's log): the failed render's mark and the re-made neighbours added to the accepted list; where `latency.ps1`'s result is written |
| 2026-11-26 | Open | T346 done; set Open |
| 2026-11-26 | Ready for review | Added the channel shortcuts, View submenu, session selection and reset, and marks on both sides of the compare. Added five tests and extended the shortcut and menu tests. The owner's hand check and latency measurement follow review. |
| 2026-11-26 | Ready for review | Review: nothing to fix. Independent review (fresh agent): nothing to fix; three nits, two taken by the reviewer: the JPEG test's temporary file removed without `unwrap` (a scanner may hold it), `ImageAreaShown`'s channel fields after `shadows_amount` with a doc comment each. The glossary's "lasts for the session" kept (means "never saved"). |
| 2026-11-26 | Done | Approved by the owner; `.\tools\check.ps1` all OK. The accepted points written in `known-limits.md` ("The channel view (ADR 0027 W4)"), one `CHANGELOG.md` line under Added, W4's row in `plans/` awaiting the owner's hand check and `latency.ps1` (its result goes in W4's row). |
