# T317. ADR 0027 W3: the Settings page Preview and the sharpening values — Blocked

- **Why:** ADR 0027 §3.3 and W3-Q7 (answered by the owner 2026-11-24, ADR 0027 §4.3).
  Sharpening has three values, amount, radius and threshold, which are settings and are saved
  (§2.1); sharpening itself (on or off) is not. This task adds a Settings page **Preview** with a
  section Sharpening for the three values and a Restore Defaults button, and saves them in
  `settings.txt`. W7 later adds the thresholds and colours of §3.7 to the same page. The values
  are not used yet: T318 passes them to the view core. The Settings window applies a page only at
  Save, as every page does today, so no view is rendered again while a slider is dragged (W3-Q7;
  ADR 0027 §4.3, the note after the table). The fourth task of the chain T315 → T319 → T316 → **T317** →
  T318. Waits for T316 (it uses T315's `Sharpening`; T316 is not needed for it, but the chain is
  done in order).
- **What the reviewer checked (2026-11-24; line numbers from the cold read):**
  - `crates/pw-loupe/src/shell.rs`: `SettingsPage` (`General`, `ExternalPrograms`,
    `MetadataWindow`, `KeyboardShortcuts`) and `SettingsPage::ALL`, the order of the page
    buttons.
  - `crates/pw-loupe/src/shell_text.rs`: `SettingsText` and `settings_text` (for example
    `Page(SettingsPage::General) => "General"`, `RestoreDefaults => "Restore Defaults"`); its test
    of `SettingsPage::ALL` near line 1214.
  - `crates/pw-loupe/src/main.rs` (the Settings window, about lines 1795–1990): the page buttons
    from `SettingsPage::ALL`; the bottom panel with Restore Defaults for `MetadataWindow` and
    `KeyboardShortcuts` and nothing for `General | ExternalPrograms`; the scroll area's
    `scroll_id` per page; the page content per page, working on `chosen` (a copy of the settings
    written only by Save; Cancel drops it).
  - `crates/pw-loupe/src/settings.rs`: `Settings { …, shell: ShellSettings, … }`; the keys as
    `const` names (for example `FOCUS_MARKING`, `KEY_ACTION_WIDTH`); the reader matches the key
    and on a value it cannot read pushes `bad("…")` to `problems` and keeps the default; the
    writer writes the lines in a fixed order, the last today `thumbnail_buttons`; the test of the
    whole default file (about line 1209) compares the text exactly.
  - `pw_viewcore::Sharpening` (T315): `amount` (10–300 %), `radius_tenths` (3–20), `threshold`
    (0–20), the range constants, `DEFAULT`, `is_valid`.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/settings.rs`,
  `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/glossary.md`, `docs/test-inventory.md` (rule 12), and this
  task's own file (status and log).
- **What:**
  1. **The values:** `ShellSettings` gets `sharpening: pw_viewcore::Sharpening`, default
     `Sharpening::DEFAULT`. Three keys, written after `thumbnail_buttons`, in this order:
     `sharpen_amount = 60` (whole percent), `sharpen_radius = 0.6` (pixels, one decimal, written
     from `radius_tenths`), `sharpen_threshold = 2` (levels). Reading: `sharpen_amount` and
     `sharpen_threshold` as whole numbers (`u16`, `u8`), `sharpen_radius` as an `f64` that must be
     finite, made into tenths with `(v * 10.0).round()` and then checked against
     `RADIUS_TENTHS_RANGE` (so 0.25 is read as 3 and is valid; the range is checked before any
     cast to `u8`); a value that cannot be read, or lies outside its range (W3-Q7), keeps that
     value's default and is reported as the reader reports any bad value (`bad(expected)`,
     settings.rs about line 426), with these `expected` texts built from the range constants: "a
     whole number from 10 to 300", "a number from 0.3 to 2.0", "a whole number from 0 to 20". A
     file without the lines gives the defaults. `ShellSettings` derives `Clone, Debug,
     PartialEq`; `Sharpening` (T315) derives more, so nothing else is needed.
     **Save** (`save_settings_window`, main.rs about line 1659) builds the new `shell` from the
     current one and takes only `theme` and `companion_suffixes` from `chosen`: it also takes
     `chosen.shell.sharpening`, or the page's values would never apply or be saved.
  2. **The page:** `SettingsPage::Preview`, last in `SettingsPage::ALL` (after Keyboard
     Shortcuts), named "Preview" (`settings_text`). Its own `scroll_id` ("settings-preview").
     Its content, on `chosen.shell.sharpening`:
     - a heading "Sharpening" and a small note under it that wraps (drawn as
       `CompanionSuffixesNote` is, `egui::Label::new(…).wrap()`, main.rs about line 1910): "Used
       in the preview below 100% while sharpening is turned on. Not saved in the images." (the
       menu item is not named: T318 adds it);
     - three `egui::Slider`s, each with its label as the slider's `.text(…)` (as the slider at
       main.rs about line 8694): "Amount (%)", 10–300, `.step_by(5.0)` (a value from the file
       that is not a multiple of 5, for example 62, is kept until the slider is dragged);
       "Radius (pixels)", on an `f64` copy of `radius_tenths / 10`, 0.3–2.0, `.step_by(0.1)` and
       `.fixed_decimals(1)`, stored back as `(v * 10.0).round()` tenths; "Threshold (levels)",
       0–20 in steps of 1;
     - the bottom panel's **Restore Defaults** for this page too (`SettingsText::RestoreDefaults`,
       as on the pages Metadata Window and Keyboard Shortcuts): a new flag `restore_sharpening`
       with its own arm in the panel's `match page` (about line 1816; `General |
       ExternalPrograms` keep the empty arm), and after the window, as `restore_defaults` is
       handled (about line 2010), `chosen.shell.sharpening = Sharpening::DEFAULT`; like every
       change on the page it is kept only by Save.
     `SettingsPage::ALL` becomes `[SettingsPage; 5]`; the exhaustive matches on `SettingsPage`
     are in main.rs (about lines 1816, 1838, 1870) and `settings_text`. The new texts are new
     `SettingsText` variants, for example `SharpeningHeading`, `SharpeningNote`, `SharpenAmount`,
     `SharpenRadius`, `SharpenThreshold`.
     Every text by `settings_text` (rule 13), none in drawing code; the layout wraps (the note
     wraps as `CompanionSuffixesNote` does).
  3. **The glossary:** a row of its own, "Preview (Settings page)", apart from the existing row
     "Preview" for the preview mode (the Settings window's page for the preview's
     display settings; today the section Sharpening, ADR 0027 W3; added by T317) and for
     "Sharpening", "Amount (%)", "Radius (pixels)" and "Threshold (levels)" (the unsharp mask's
     three values, ADR 0027 §3.3, W3-Q7; added by T317). The menu item and the mark are T318's
     (it adds them to the "Sharpening" row).
- **Tests to add:**
  - `settings.rs`: the default settings file holds the three lines with 60, 0.6 and 2. These
    existing tests are **updated** (asked for here, not a weakening, rule 5): the test of the whole
    default file (about lines 1192–1222) with the three new lines after `thumbnail_buttons`; the
    tests that expect `thumbnail_buttons` to be the last line (about lines 2412, 2702–2706 and
    2848), so that the sharpening lines follow it; and `changed_shell()` (about line 1367), which
    builds `ShellSettings` as a whole literal, gets a non-default `sharpening` (for example 150,
    13, 7).
  - `settings.rs`: written and read back, a non-default value (amount 150, radius 1.3, threshold
    7) comes back equal.
  - `settings.rs`: each bad value keeps its default and gives one problem: `sharpen_amount = 5`,
    `= 301`, `= sixty`; `sharpen_radius = 0.2`, `= 2.1`, `= wide`; `sharpen_threshold = 21`,
    `= -1`; a file without the lines gives `Sharpening::DEFAULT` and no problem.
  - `settings.rs`: `sharpen_radius = 0.64` is read as 6 tenths and `= 0.66` as 7; `= inf` and
    `= NaN` keep the default and give a problem.
  - `shell_text.rs`: `SettingsPage::ALL` ends with `Preview` and its name is "Preview" (the
    existing test of the pages' order and names, about line 1212, is **updated**, as above); `settings_text(Page(Preview))` is "Preview", and the new
    texts of point 2 are those strings.
- **Not in this task:** passing the values to the view core, the key, the mark and the reset
  (T318); the thresholds and colours of §3.7 (W7).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by a function in
  `shell_text.rs`; the glossary rows added). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W3-Q7); waits for T316 |
| 2026-11-24 | Blocked | Cold read: Save takes only the theme and the suffixes from the window, so it must also take `sharpening` (point 1); the four existing tests that end with `thumbnail_buttons` or build `ShellSettings` whole named as updated; the reader's rule for the radius (finite, rounded to tenths, then the range) and the three `expected` texts; the sliders (`.text`, `step_by`, `fixed_decimals`), the Restore Defaults flag and arm, the exhaustive matches, `ALL` of 5, the new `SettingsText` variants named; the note no longer names T318's menu item and wraps; the glossary row named "Preview (Settings page)"; the order test is in `shell_text.rs`. Waits for T316 and the chain review |
| 2026-11-24 | Blocked | Chain review: nothing for T317's text; its `CHANGELOG.md` line is written together with T318's at T318's Done (until then there is no way to turn sharpening on). Waits for T316 |
