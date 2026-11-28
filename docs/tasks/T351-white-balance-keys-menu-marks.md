# T351. ADR 0027 W6: the key, the menu and the mark of the white balance — Open

- **Why:** ADR 0027 §2.1, §3.5 and W6-Q4, W6-Q6, W6-Q8 (answered by the owner 2026-11-28, ADR 0027
  §4.6). T350 gave the view core the white balance's mode (`pw_viewcore::WbMode`,
  `ViewCore::set_white_balance` and `reset_white_balance`, `Frame::white_balance:
  WhiteBalanceState` with the mode, the base, the balance, the view's state, the file's preset
  names and `has_daylight`). This task lets the user use it: the key W, the submenu View ▸ White
  Balance (with the current image's presets) and the same submenu in the preview's right-click
  menu (owner, 2026-11-28: the preview has no menu bar, ADR 0017 U3-Q1; ADR 0027 §4.6 "Where the
  menu is reached", ADR 0019 E-Q17), the mark in the preview's row of adjustment marks,
  both sides of the compare, and Reset Adjustments for it. **The click on gray (its key, its menu
  item, its mode and its click) is T353's.** The fourth task of W6's chain T348 → T349 → T350 →
  **T351** → T352 → T353; the owner's hand check of the auto balance, the daylight and the presets
  can follow it. Waits for T350.
- **What the reviewer checked (2026-11-28, and by a cold read the same day):** the exposure (T335),
  the shadows and the channel (T347) went through the same places:
  - `keymap.rs`: the enum `KeyAction` and `KeyAction::ALL` (an array whose length is the number
    of actions: `[KeyAction; 57]` after T347), both with `Sharpening`, `Shadows`,
    `ExposureBrighter`, `ExposureDarker`, `ResetExposure`, `AutoExposure`, `CameraBrightness`,
    `NextChannel`, `PreviousChannel`, `ResetAdjustments` in that order; `index()` is `self as
    usize`, checked by `names_round_trip_and_are_unique`; the names for `keys.txt`, `group()` and
    `acts_in()`, `default_keys` (W is bound to nothing; **Ctrl+W is `CloseTab`**, a different
    chord), `of_command` (`_ => return None` for commands without a key, line 528) and `command`;
    the test helper `today()` (about line 1231) matches `PreviewKey` exhaustively; the test that
    takes `FineDetails`' place and asserts `KeyAction::ALL[place + n] == ResetAdjustments` (n is
    10 after T347, about line 1989) and the test of unique default keys
    `exposure_shadows_and_channel_have_unique_default_keys_and_follow_sharpening` (about line 1993,
    the model for the new key's). `fixed_in_preview` (line 722) does not fix W.
  - `shell.rs`: `Command` is `Copy + Eq` and holds `Stars(u8)`, `OpenIn(u8)`, `ApplyTemplate(u16)`
    and `Theme(Theme)`, so `WhiteBalancePreset(u8)` fits; `SubmenuKind` (with `Theme`, `Exposure`
    and `Channel`); `EXPOSURE_ITEMS` and `CHANNEL_ITEMS` as static slices of `MenuItem`; the View
    menu: `Shadows` (O), `MenuEntry::Submenu(SubmenuKind::Exposure, EXPOSURE_ITEMS)`,
    `MenuEntry::Submenu(SubmenuKind::Channel, CHANNEL_ITEMS)`, then `ResetAdjustments`
    (Backspace); `acts_in`'s `Where::Loupe` list; the tests of the View menu's slices after Zoom
    at Pointer and after Fine Details (they count items: `anchor + 16` and `fine + 6` after T347,
    about lines 2216 and 2368, both become one more with the new Submenu entry). The right-click
    menu (reviewer, 2026-11-28, after the owner's decision): `MenuEntry` (about line 945, with
    `ExternalPrograms` and `Templates` as entries drawn from data) and `IMAGE_CONTEXT_MENU` (about
    line 1206: … `Metadata`, `Templates`, `Separator`, `DeleteToRecycleBin`), one slice for the
    contact sheet and the preview; its key test (about line 1271) and `keymap.rs`'s
    `every_menu_item_with_an_action_shows_its_default_first_key` (about line 1463) walk its entries
    with a `_` arm.
  - `main.rs`: `MenuEntry::Submenu(SubmenuKind, &'static [MenuItem])` is drawn generically by
    `menu_entry` (about line 9303); the closest model of a list built from data **inside a
    submenu** is `MenuEntry::Templates` / `templates_menu` (about line 9321: items by index from
    data, `ApplyTemplate(u16)`, `u16::try_from(index)`, a separator), not `ExternalPrograms` (a
    top-level entry); `menu_item` takes the text from `command_text(item.command)`, which has no
    name to give for a preset. `menu_entry` matches `MenuEntry` exhaustively (`Filter | Sort =>
    {}` for the entries the right-click menu does not draw); `image_context_menu` (about line 9355)
    draws `IMAGE_CONTEXT_MENU` and is called from the contact sheet (about lines 4760 and 4824) and
    from the preview (about line 10402, and 10486 on the compare's held side); `in_preview()`
    (about line 6805). `PreviewExposure` ("Session adjustments shared by the current and
    held previews", `#[derive(Default)]`: `exposure_mode`, `shadows`, `channel`), with
    `toggle_shadows(repeat)`, `step_channel(forward, repeat)`, `set_channel(c)`, `reset(cores)`
    and `wish(…)` (sets both cores each frame: the first while the preview is shown, the second
    while the compare is open as well); its tests build `ViewCore<NativeBackend>` on files that do
    not exist (`cores_with_manual_steps`, about line 15170; the JPEG fixture of
    `only_visible_preview_cores_receive_the_channel_view`, about line 15241, gives a core with a
    real frame); **there is no test backend in `pw-loupe`**. `key_map_action` (the list of preview
    actions about lines 6567–6588 and its match); `reset_adjustments` and `any_adjustment_on`;
    `command_enabled` (`_ => true`), `command_checked` (with `Command::Theme(theme)` and
    `Command::Channel(c)` as the models of a ticked radio item) and `run_command` (exhaustive, no
    `_` arm), all `&self` methods of `Loupe` with no frame in reach; the field `last_preview_source`
    (about line 10490) as the model of a value the last drawn frame leaves on `Loupe`; the test
    `Loupe` literal (about line 14564); `ImageAreaShown` (built once per frame, with
    `shadows_key`, `shadows_amount` and the channel's key) and `draw_image_area` (a free function:
    it has no access to `PreviewExposure`; it calls `adjustment_marks` at about line 10737).
  - `preview_text.rs`: `adjustment_marks` (line 223, already **eight** parameters, with
    `#[allow(clippy::too_many_arguments)]`) with `exposure_mark`, `shadows_mark` (dimmed while
    paused: the model) and `channel_mark` (never dimmed); eleven call sites (ten in the file's
    tests, one in `main.rs`); `key_text`.
  - `shell_text.rs`: `command_text` (exhaustive; `ApplyTemplate(_)` has a "never shown" text,
    about line 671), `submenu_text`, `key_action_text`; the tests `every_menu_item_text_in_menu_order`
    (the texts of `menu_items()`) and `every_menu_key_text_in_menu_order`, and
    `every_fixed_text_of_the_shell` (about lines 770–774), where the submenus' names are.
  - `docs/glossary.md`: G5 (American spelling: "Color", "gray"); the rows "Shadows, Show Shadows",
    "Channel view, …" and "Reset Adjustments".
  - `crates/pw-viewcore/src/lib.rs` (after T350) exports `WbMode`, `WbBase`, `WhiteBalanceState`,
    `Wb` and `WbPreset`.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/keymap.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — about eight files, a run-time menu list, and the two cores of the compare kept
  in step (README, "Model").
- **What:**
  1. **The commands and the key** (W6-Q4, W6-Q6), everywhere `Command::Shadows` is handled in
     `keymap.rs`, `shell.rs`, `lib.rs` and `main.rs`, the same way: one command and key action,
     group View, acting in the preview, in the enum `KeyAction` **and** in `KeyAction::ALL` right
     after `PreviousChannel` and before `ResetAdjustments` (the array one longer):
     | Command / `KeyAction` | name in `keys.txt` | default key | `command_text` |
     |---|---|---|---|
     | `AutoWhiteBalance` | `auto_white_balance` | W | "Auto White Balance" |
     and three commands without a key and without a `KeyAction` (`of_command` gives `None`):
     `WhiteBalanceShot` ("As Shot"), `WhiteBalanceDaylight` ("Daylight (LibRaw)") and
     `WhiteBalancePreset(u8)` (the file's preset by its place in the current image's list, 0 to
     95, the shim's limit; its `command_text` is a "never shown" placeholder as
     `ApplyTemplate(_)`'s, "White balance preset", since the menu draws the file's own name).
     `preview_key_action` gets a `PreviewKey` for W; `command_of_key`, `today()` and
     `key_map_action`'s list and match their arms; `run_command` gets the four arms (it has no
     `_`). The four commands are listed in `acts_in`'s `Where::Loupe` list.
  2. **The menu** (W6-Q4): in the View menu, **after the submenu Exposure and before the submenu
     Channel**, a submenu "White Balance" (`SubmenuKind::WhiteBalance`, `submenu_text` "White
     Balance", also in `every_fixed_text_of_the_shell`) from a static slice
     `WHITE_BALANCE_ITEMS`: As Shot, Auto White Balance (W), Daylight (LibRaw). **The presets:**
     `menu_entry` special-cases this submenu kind, as `templates_menu` is built: after the static
     items, when the current image's `frame.white_balance.presets` is not empty, a separator and
     one `egui::Button::new(name).selected(on)` per preset (`name` is the file's own, data and not
     a text of the program), each running `Command::WhiteBalancePreset(i)` (`u8::try_from(index)`);
     the list is the current image's (the compare's second side's is not shown). The frame comes
     from a new field `last_white_balance: WhiteBalanceState` on `Loupe`, set where
     `last_preview_source` is set from the current side's frame (and in the test `Loupe`
     literal); `command_checked` and `command_enabled` read it. The mode is ticked: by the frame's
     `mode` (`WhiteBalanceShot` for `Shot`, `AutoWhiteBalance` for `Auto`, `WhiteBalanceDaylight`
     for `Daylight`, a preset item when the mode is `Preset(name)` and the item's name is `name`),
     as pure functions of the frame so they can be tested (point 3). **Enabled** (`command_enabled`):
     in the preview only; Daylight (LibRaw) only when `has_daylight`; the others in every mode and
     for every file (in JPEG mode they set the mode, which is paused, §2.2). These existing tests
     are **updated** to the new items (asked for here, not a weakening, rule 5): the tests of the
     View menu's slices (the counts), `every_menu_item_text_in_menu_order` and
     `every_menu_key_text_in_menu_order` (the texts "As Shot", "Auto White Balance", "Daylight
     (LibRaw)" and the keys None, W, None, in the menu's order, before the Channel items),
     `every_fixed_text_of_the_shell` ("White Balance"), and the keymap test that asserts
     `ALL[place + n] == ResetAdjustments` (n grows by one).
  2b. **The right-click menu** (owner, 2026-11-28; ADR 0027 §4.6 "Where the menu is reached", ADR
     0019 E-Q17): the preview has no menu bar (ADR 0017 U3-Q1), so the same submenu White Balance
     is also in the image's right-click menu **in the preview only** (on both sides of the
     compare). In `shell.rs`: a new entry `MenuEntry::PreviewWhiteBalance` (doc comment: the
     submenu White Balance and a separator, drawn only in the preview) in `IMAGE_CONTEXT_MENU`
     between its last `Separator` and `DeleteToRecycleBin`, and a pure function
     `image_context_entries(in_preview: bool) -> Vec<MenuEntry>`: `IMAGE_CONTEXT_MENU` with
     `PreviewWhiteBalance` replaced by `MenuEntry::Submenu(SubmenuKind::WhiteBalance,
     WHITE_BALANCE_ITEMS)` and `MenuEntry::Separator` when `in_preview`, and left out otherwise,
     so the contact sheet's menu is exactly today's. `main.rs`: `image_context_menu` draws
     `image_context_entries(self.in_preview())`, so the submenu goes through `menu_entry`'s
     special case of point 2 (the static items, then the current image's presets) and the same
     enabled and ticked rules; on the compare's held side the presets listed are the current
     image's, as in the View menu. `menu_entry` gets an arm `MenuEntry::PreviewWhiteBalance =>
     {}` beside `Filter | Sort` (it is never drawn directly); the other exhaustive matches on
     `MenuEntry` get it too: `menu_items()` (`shell.rs` about line 1226, with the entries that hold
     no menu-bar item) and `draw_menu_bar` (`main.rs` about lines 9239–9287, never in the menu
     bar). The doc comments of `IMAGE_CONTEXT_MENU` (about line 1200) and `image_context_menu`
     (about line 9351) say that the submenu is drawn in the preview only. These existing tests are
     **updated** (asked for here, not a weakening, rule 5): the two that check the keys of the
     right-click menu's items (`shell.rs` `the_default_map_gives_every_menu_item_its_old_key`, about
     line 1268, and `keymap.rs` `every_menu_item_with_an_action_shows_its_default_first_key`, about
     line 1463) walk `image_context_entries(true)` instead, so the white balance items' keys are
     checked too; `the_image_context_menu_holds_the_image_commands_in_order` (about line 1962)
     gets `MenuEntry::PreviewWhiteBalance` in its expected slice between the last `Separator` and
     `DeleteToRecycleBin`; `every_item_of_the_image_context_menu_is_the_menu_bars_item` (about
     line 2010) keeps walking `IMAGE_CONTEXT_MENU` and treats `PreviewWhiteBalance` as
     `Templates` (in the arm about line 2048, its doc comment naming it: the submenu is the View
     menu's, not the Image menu's). The View menu keeps its submenu (point 2); it is drawn only
     outside the preview, so its items stay greyed there, as Exposure's and Channel's do.
  3. **The state the shell holds** (A-Q2, W6-Q6): `white_balance: WbMode` (`Shot` at the start,
     never `Grey`) and `wb_sent: [Option<WbMode>; 2]` (what the shell last set on each core) in
     `PreviewExposure`, not in `ShellSettings` (not saved), so it can be tested as the exposure's
     state is:
     - `toggle_auto_white_balance(&mut self, repeat: bool, effective: &WbMode)`: a repeated key
       press is ignored (as `toggle_shadows`; every press makes the view again); `effective` is
       the **frame's** mode of the current core (what the user sees): `Auto` → `Shot`, anything
       else (also `Grey`, T353) → `Auto`; the shell's mode takes the result and both `wb_sent` are
       cleared so the next `wish` sends it;
     - `set_white_balance(&mut self, mode: WbMode)` for `WhiteBalanceShot`, `Daylight`, and
       `set_preset(&mut self, place: u8, presets: &[WbPreset])` for `WhiteBalancePreset(place)`
       (`Preset(name)` of the place in the **current frame's list**; a place outside it does
       nothing); each clears both `wb_sent`;
     - `reset(…)` also sets `Shot` and clears `wb_sent`;
     - `wish(…)` also sets the mode on both cores: the first gets the target while the preview is
       shown, otherwise `WbMode::Shot`; the second the same while the compare is open as well (both
       sides, W6-Q3). What it sends is decided by a pure function `wb_to_send(target: &WbMode,
       sent: &Option<WbMode>, frame_mode: &WbMode) -> Option<WbMode>`: `Some(target)` when `sent
       != Some(target)`, or when the core's frame mode is neither the target nor `Grey` (the core
       turned its own mode into `Shot` when another image was shown, T350/T352, and the shell must
       send `Auto` again; a core in `Grey` after a click keeps it until the next explicit choice),
       else `None`; it stores `sent` per core. `wish` needs each core's frame (it already has the
       cores: the frame's mode is read from `core.frame(...)` or from the frames the caller
       passes; say which in the log);
     - the tick and "any on" read through small pure functions on the frame's state (for example
       `wb_checked(command, frame: &WhiteBalanceState) -> Option<bool>` and `wb_on(frame: &WhiteBalanceState) -> bool`),
       so the tests need no `Loupe`.
  4. **The reset** (A-Q3, W3-Q8): `reset_adjustments()` sets the balance to `Shot` (through
     `PreviewExposure::reset`) and calls `reset_white_balance` on both cores; `any_adjustment_on()`
     is also true while the frame's mode is not `Shot`. Update the doc comment of
     `reset_adjustments` ("W6 adds its adjustments here") to name the white balance, and the Reset
     Adjustments row of the glossary.
  5. **The mark** (W6-Q8): `adjustment_marks` gets a **ninth** parameter, `(white_balance,
     wb_key): (&WhiteBalanceState, Option<KeyChord>)`, placed before the channel's to match the
     row's order (its `#[allow(clippy::too_many_arguments)]` and reason comment stay; the existing
     parameters keep their meaning); its eleven call sites (ten in `preview_text.rs`'s tests, one
     in `main.rs`) are updated to pass `&WhiteBalanceState::off()` and `None` (asked for here, not
     a weakening, rule 5). The mark comes **after** the exposure's and the shadows' and **before**
     the channel's. By `view` and `base`:
     - `Off`: no mark.
     - `Shown` or `Waiting`, not dimmed; `Paused`, dimmed (as `shadows_mark`): the text by the
       base: `Auto` and `AutoWaiting` "WB auto"; `Grey` "WB gray"; `Daylight` "WB Daylight
       (LibRaw)"; `Preset(name)` "WB {name}"; `AutoNone` "WB auto: none"; `DaylightNone` "WB Daylight
       (LibRaw): none"; `PresetNone(name)` "WB {name}: none"; `Shot` while `view` is not `Off` (a
       balanced picture still drawn after returning to as shot, §2.1) "WB as shot".
     - The hover texts (whole sentences, one template with the key and one without, American
       spelling, the color made neutral explained): for `Auto`: "Auto white balance ({key}): red
       ×{r}, blue ×{b}, worked out over the whole image so that its average color is neutral; the
       histogram and the overlays are as shot." and without the key "Auto white balance: red …";
       for `Grey`: "White balance from gray: red ×{r}, blue ×{b}; the histogram and the overlays
       are as shot."; for `Daylight`: "Daylight (LibRaw): red ×{r}, blue ×{b}; the histogram and
       the overlays are as shot."; for `Preset(name)`: "White balance {name} (from the file): red
       ×{r}, blue ×{b}; the histogram and the overlays are as shot."; for `AutoWaiting`: "Auto
       white balance ({key}): being worked out."; for `Waiting` with a known base: the sentence
       of the base with " The view is being made again." appended; for `AutoNone`: "Auto white
       balance ({key}): none could be worked out, the view is as shot."; `DaylightNone`:
       "Daylight (LibRaw): the file has none, the view is as shot."; `PresetNone(name)`: "White
       balance {name}: the file has none, the view is as shot."; for `Shot` while `view` is not
       `Off`: "White balance as shot: the view is being made again."; **for `Paused`** (JPEG mode,
       a JPEG or TIFF file; `state.wb` is `Shot` there, so no numbers) one sentence per base
       without multipliers: "Auto white balance ({key}): applies only to a RAW file in RAW
       mode." and likewise for gray, Daylight (LibRaw) and a preset by name. `{r}` and `{b}` are
       the thousandths of `state.wb` formatted by **integer** rounding to hundredths (`(m + 5) /
       10`, so 2415 gives "2.42"; never through `f32`), a key without a key map entry omitted as the
       other marks do. Every text, the marks' texts included, by a function in `preview_text.rs`
       (rule 13), not in `paint_adjustment_marks`; `draw_image_area` passes each side's
       `frame.white_balance` and the key, filled where `shadows_key` is (`ImageAreaShown`).
  6. **The glossary** (rule 13): rows "White balance, Auto White Balance, As Shot, Daylight
     (LibRaw), white balance preset" (the preview's balance for RAW mode, ADR 0027 §3.5: the key
     W, the submenu White Balance in the preview's right-click menu and in the View menu, the marks "WB auto" and so on, what each means in one
     sentence, that the histogram and the overlays stay as shot, that a preset is the file's own,
     and that "Daylight (LibRaw)" is LibRaw's own daylight multipliers for every RAW file), and the
     changed row "Reset Adjustments" (the white balance too).
- **Tests to add:**
  - `keymap.rs`: the name `auto_white_balance` round trips and is unique; its default key is W,
    unique among the defaults, and `KeyAction::ALL` has it right before `ResetAdjustments`; it
    is in the preview's group and acts in the preview; the model is
    `exposure_shadows_and_channel_have_unique_default_keys_and_follow_sharpening`.
  - `lib.rs`: W gives its `PreviewKey` and command in the preview and nothing in the grid (as N
    does); Ctrl+W still gives `PreviewKey::Nothing` in the preview and `CloseTab` in the grid.
  - `shell.rs`: the View menu has the submenu "White Balance" with the three static commands in
    order between Exposure and Channel; the four commands act in the preview only;
    `WhiteBalancePreset`, `WhiteBalanceDaylight` and `WhiteBalanceShot` have no key and no
    `KeyAction`; `image_context_entries(false)` equals today's contact-sheet list (every entry of
    `IMAGE_CONTEXT_MENU` but `PreviewWhiteBalance`, in order) and `image_context_entries(true)`
    has `Templates`, `Separator`, the submenu White Balance with `WHITE_BALANCE_ITEMS`,
    `Separator`, `DeleteToRecycleBin` at its end.
  - `shell_text.rs`: the new menu texts in order (point 2), `submenu_text`, and the placeholder.
  - `main.rs`, on `PreviewExposure` and pure functions without a window: `toggle_auto_white_
    balance` with `effective` `Auto` gives `Shot`, with `Shot` and with `Grey` gives `Auto`, a
    repeat is ignored; As Shot, Daylight and `set_preset` set the mode, and a place outside the
    list does nothing; `reset` sets `Shot`; `wb_to_send`: nothing the first time with an equal
    target and `sent` (`None`), sends when `sent` is `None`, sends again when the frame's mode is
    `Shot` while the target is `Auto` and `sent` says `Auto`, and sends nothing when the frame's
    mode is `Grey` and `sent` equals the target; `wish` with the JPEG fixture cores of
    `only_visible_preview_cores_receive_the_channel_view` sets both cores' mode while the compare
    is open and `Shot` while the preview is not shown (observed through `frame().white_balance.mode`);
    `wb_on` and `wb_checked` for each mode (the preset by name).
  - `preview_text.rs`, the white balance's mark: no mark for `Off`; the marks "WB auto", "WB
    gray", "WB Daylight (LibRaw)", "WB Shade", their "none" forms and "WB as shot", each with and
    without a key, `Paused` dimmed with its sentence without numbers, `Waiting` and `Shown` not
    dimmed; the hover texts with the multipliers worked out by hand in the test's comment
    (`Wb::Mul { r: 2415, b: 1568 }` gives "×2.42" and "×1.57" by integer rounding); the order of
    the marks: exposure, shadows, white balance, channel.
- **Not in this task:** the click on gray, its key Shift+W, its menu item Pick Gray and its mode
  (T353); buttons for the balance in the status bar (W6-Q6: the hand check may add them, as it did
  for the exposure); a setting "Auto white balance at start" (W6-Q6: not now); a key per preset;
  the Settings page.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (user-visible text: the program's
  texts are in `preview_text.rs` and `shell_text.rs` as whole sentences with placeholders, American
  spelling; the preset names are the file's data). No new dependency. A new term goes into
  `docs/glossary.md` (point 6).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added (as T351 with the pick mode); waits for T350 |
| 2026-11-28 | Blocked | Cold read and chain review: the pick mode split out to T353; `wish` redesigned (per-core `wb_sent`, the pure `wb_to_send`, the toggle from the frame's mode); the preset list drawn as `templates_menu` is, with a placeholder `command_text`; the tests through pure functions (no test backend in `pw-loupe`); `last_white_balance` on `Loupe`; the ninth parameter of `adjustment_marks` and its eleven call sites; "gray" spelling; paused sentences without numbers; integer rounding of the multipliers; the test names and counts |
| 2026-11-28 | Open | T350 done; set to Open by the reviewer (the cold read and the chain review are above) |
| 2026-11-28 | Open | The owner's decision (2026-11-28, asked by the task agent before it started: the preview has no menu bar, U3-Q1): the submenu White Balance also in the preview's right-click menu, before Delete to Recycle Bin; written into ADR 0027 §4.6 and ADR 0019 §2.9 / E-Q17; point 2b and its tests added, the files checked; Exposure and Channel stay out (backlog, W6's hand check). Cold read of the changed text: see the next row |
| 2026-11-28 | Open | Cold read of the changed text (point 2b, one fresh agent): the names and lines hold, `in_preview()` is true for both sides of the compare and false in the contact sheet, a `Submenu` from a `Vec` reaches `menu_entry`'s special case; added the two other exhaustive matches (`menu_items`, `draw_menu_bar`), the two tests not named (`the_image_context_menu_holds_the_image_commands_in_order`, `every_item_of_the_image_context_menu_is_the_menu_bars_item`), the doc comments, and that the View menu's submenu stays greyed outside the preview |
