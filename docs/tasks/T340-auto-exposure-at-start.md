# T340. ADR 0027 W5: the setting "Auto exposure at start" — Blocked

- **Why:** W5's hand check (owner, 2026-11-26; ADR 0027 §4.4 "W5's hand check", which amends
  §2.1's "off by default and never saved" for auto exposure alone). The owner finds the RAW view
  at 0 EV often clearly darker than the camera's JPEG. A setting on the Settings page Preview,
  **"Auto exposure at start"**, saved in `settings.txt`, **on by default**: auto exposure is on
  when RawStride starts; B (or the button "A", T339, or Backspace) turns it off for the session
  as before. The third task of the chain T338 → T339 → **T340**. Blocked: waits for T339 (both
  change `main.rs` and `docs/glossary.md`).
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-loupe/src/settings.rs`: `ShellSettings` (about line 186; `shadows_amount` about
    line 270, its default about line 347); the bool keys are read together (about lines
    478–499: `parse_bool`, `bad("true or false")`); `shadows_amount` is written last of the
    shell lines (about line 851: `text.push_str(&format!("{SHADOWS_AMOUNT} = {}\n", …))`).
    Tests that hold the whole shell text or its end, each ending with `shadows_amount = 50`:
    `saved_file_has_the_comment_line_and_every_key` (about line 1317), the thumbnail-buttons
    test (about line 2517), `the_shadows_amount_is_written_and_read_back` (about line 2683), the
    `move_to_subfolders` / `companion_suffixes` test (about line 2969) and
    `the_external_program_line_is_after_the_shell_lines_and_before_the_library` (about line
    3096; the new line goes between `shadows_amount` and `external_program` there);
    `changed_shell()` (about line 1463, the only `ShellSettings` literal without `..`).
  - `crates/pw-loupe/src/shell_text.rs`: `SettingsText` with `ShadowsHeading`, `ShadowsNote`,
    `ShadowsAmount` (about lines 340–392) and their test (about line 1350).
  - `crates/pw-loupe/src/main.rs`: `PreviewExposure` (about line 1227; `exposure_mode` "off at
    start, not saved"), made with `PreviewExposure::default()` where the `Loupe` is built (about
    line 13640); `save_settings_window` (about line 1749) takes `sharpening` and
    `shadows_amount` from the chosen settings; the page Preview draws
    `draw_sharpening_section` and `draw_shadows_section` (about lines 2076–2080); Restore
    Defaults of the page (`restore_preview`, about lines 2121–2124); `draw_shadows_section`
    (about line 10308). The tests of `PreviewExposure` (about lines 13775–13811).
  - `docs/glossary.md`: the rows "Preview (Settings page)" and "Auto exposure, Auto Exposure"
    (about line 290).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/settings.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `crates/pw-loupe/src/main.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Sonnet — one layer, three code files with their functions named, the tests
  specified, no design choice left (README, "Model").
- **What:**
  1. **The value:** `ShellSettings` gets `auto_exposure_at_start: bool`, default **true**. The
     key `auto_exposure_at_start`, a `const AUTO_EXPOSURE_AT_START`, read with the other bool
     keys (an alternative in the outer pattern and its own arm in the nested `match key`, before
     the last arm `_ => shell.columns_hidden`; `bad("true or false")` for a bad value, the
     default kept);
     written right after `shadows_amount` (`auto_exposure_at_start = true`). A file without the
     line gives true. `save_settings_window` takes `chosen.shell.auto_exposure_at_start` into
     the new `shell` beside `shadows_amount`, and its comment names it.
  2. **The section** on the page Preview, after Shadows, with `ui.add_space(PANEL_ROW_SPACING)`
     between: `fn draw_exposure_section(ui, at_start: &mut bool)` beside
     `draw_shadows_section`, drawn the same way: a heading "Exposure", a small note that wraps,
     "Auto exposure sets the RAW preview's brightness from the RAW histogram. B turns it on or
     off for the session. Not saved in the images.", and one `ui.checkbox(at_start, …)` with the
     text "Auto exposure at start". New `SettingsText` variants `ExposureHeading`,
     `ExposureNote`, `AutoExposureAtStart` (rule 13).
  3. **Restore Defaults** of the page Preview sets it to true as well (where `restore_preview`
     is handled).
  4. **At start:** a function `PreviewExposure::at_start(auto: bool) -> PreviewExposure` gives
     `exposure_mode: ExposureMode::Auto` when `auto`, else `Off`, the rest as `default()`; the
     `Loupe` is built with `PreviewExposure::at_start(settings.shell.auto_exposure_at_start)`
     (about line 13640). The doc comment of `exposure_mode` says "off at start unless the
     setting Auto exposure at start is on (T340), not saved". `reset` (Backspace) still turns it
     off.
  5. **At Save:** when Save changes the value (the old and the new setting differ), the mode
     follows: turned on, the mode becomes `Auto` (also when it was `Camera`, A-Q12: one at a
     time); turned off, a mode that is `Auto` becomes `Off` and `Camera` stays. A Save that does
     not change the value leaves the mode as the user set it. A method
     `PreviewExposure::follow_setting(&mut self, old: bool, new: bool)` does this; it is called
     in `save_settings_window` with the old setting read at its start, before the new `Settings`
     literal and before `self.settings` is replaced (about line 1762).
     Expected and not a fault: with auto on at start, Reset Adjustments is enabled and View ▸
     Exposure ▸ Auto Exposure ticked from the start, and the button "A" (T339) is selected, also
     greyed on a JPEG view.
  6. **The glossary:** the row "Preview (Settings page)" names the section Exposure; the row
     "Auto exposure, Auto Exposure" says it is on at start while the setting "Auto exposure at
     start" (Settings ▸ Preview ▸ Exposure, default on) is on (ADR 0027 §4.4, owner 2026-11-26;
     T340). Keep T339's addition to that row; its ending becomes "Added by T335, the button by
     T339, the setting by T340".
- **Tests to add:**
  - `settings.rs`: the shell lines end with `auto_exposure_at_start = true` after
    `shadows_amount`. The five existing tests named above are **updated** with the new line
    (asked for here, not a weakening, rule 5); `changed_shell()` gets `false`.
  - `settings.rs`: written and read back, true and false come back equal; `= yes` keeps true and
    gives one problem "true or false"; a file without the line gives true and no problem.
  - `shell_text.rs`: the three new texts are those strings, in the test of the Preview page's
    texts (about line 1350).
  - `main.rs`: `at_start(true)` gives `Auto`, `at_start(false)` gives `Off`, shadows off in both;
    `follow_setting`: (false → true) from `Off` and from `Camera` gives `Auto`; (true → false)
    from `Auto` gives `Off` and from `Camera` keeps `Camera`; (true → true) and (false → false)
    keep `Off`, `Auto` and `Camera` as they were.
- **Not in this task:** the auto rule's target (`backlog.md`); auto exposure for the compare's
  second side is already the shared mode (`wish`), nothing to add.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by
  `settings_text`, the glossary rows). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At its Done the reviewer adds a `CHANGELOG.md` line (Added: the setting Auto exposure at
  start, on by default) and a `known-limits.md` note if the review finds one.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Added after W5's hand check (ADR 0027 §4.4); waits for T339. Cold read (also of T339 beside it): names and lines found; fixed: the fifth settings test's real name, the nested bool arm, when the old value is read, the expected side effects (Reset Adjustments enabled, Auto Exposure ticked, "A" selected), the glossary row shared with T339. No other test breaks. Sonnet kept |
