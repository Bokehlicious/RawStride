# T334. ADR 0027 W5: the shadows' amount on the Settings page Preview — Open

- **Why:** ADR 0027 §3.4 and W5-Q6 (answered by the owner 2026-11-25, ADR 0027 §4.4). The
  shadows (a lift of the dark tones of the RAW view) are a display mode for the session, turned
  on and off with a key (T335); their **amount** (0–100) is a setting on the Settings page
  Preview, section Shadows, default 50, saved in `settings.txt` with the rules of W3-Q7. This
  task adds the section and the saved value; T335 passes it to the view core. As every page, the
  window applies it only at Save. The fourth task of W5's chain T331 → T332 → T333 → **T334** →
  T335. Waits for T333 (it uses T332's `pw_viewcore::shadows::{AMOUNT_RANGE, DEFAULT_AMOUNT}`;
  the chain is done in order).
- **What the reviewer checked (2026-11-25):** T317 made the page and the section Sharpening, and
  the section Shadows follows it:
  - `crates/pw-loupe/src/settings.rs`: `ShellSettings` (line 186) with `sharpening`; the keys
    `SHARPEN_AMOUNT`, `SHARPEN_RADIUS`, `SHARPEN_THRESHOLD` (lines 90–92), read at about lines
    573–596 with `parse_sharpen_amount` and the others (a bad value: `bad(expected)` and the
    default kept), written last (about line 829, after `thumbnail_buttons`); the test of the
    whole default file and `changed_shell()` (line 1447).
  - `crates/pw-loupe/src/shell_text.rs`: `SettingsText` with `SharpeningHeading`,
    `SharpeningNote`, `SharpenAmount`, `SharpenRadius`, `SharpenThreshold` (about lines 328–381)
    and their test (about line 1298).
  - `crates/pw-loupe/src/main.rs`: `save_settings_window` (about line 1677) takes
    `chosen.shell.sharpening` into the new `shell`; the bottom panel's `SettingsPage::Preview`
    arm sets `restore_sharpening` (about line 1850), handled after the window (about line 2044);
    the page draws `draw_sharpening_section(ui, &mut chosen.shell.sharpening)` (about line
    2002), defined at about line 10023 (heading, a small note that wraps, the sliders).
  - `docs/glossary.md`: the rows "Preview (Settings page)" (line 287) and "Amount (%), Radius
    (pixels), Threshold (levels)" (line 290).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/settings.rs`,
  `crates/pw-loupe/src/shell_text.rs`, `crates/pw-loupe/src/main.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Sonnet — one layer, three code files named with their functions, the tests
  specified, no design choice left (README, "Model").
- **What:**
  1. **The value:** `ShellSettings` gets `shadows_amount: u8`, default
     `pw_viewcore::shadows::DEFAULT_AMOUNT` (50). (T332 defines `shadows::AMOUNT_RANGE:
     RangeInclusive<u8> = 0..=100` and `DEFAULT_AMOUNT: u8 = 50` in a public module.) The key
     `shadows_amount = 50`, a `const SHADOWS_AMOUNT`, written right after `sharpen_threshold`,
     the last line of `shell_lines()` (`save_settings_with_library`, about lines 713–716, adds
     the editor layout, `external_program` and library lines after it). Read as a whole number
     (`u8`) inside `AMOUNT_RANGE`; a value that cannot be read or lies outside keeps the default
     and is reported as the sharpening's are (about lines 575–579), with the text built from the
     range constant, `bad(&format!("a whole number from {} to {}", …))`, which gives "a whole
     number from 0 to 100". A file without the line gives the default. `save_settings_window`
     also takes `chosen.shell.shadows_amount` into the new `shell`, beside `sharpening`, and its
     doc comment (about lines 1676–1678) names the amount.
  2. **The section** (on the page Preview, after the section Sharpening, with
     `ui.add_space(PANEL_ROW_SPACING)` between them): a function `draw_shadows_section(ui,
     amount: &mut u8)` beside `draw_sharpening_section`, drawn the same way: a heading
     "Shadows", a small note that wraps, "Lifts the dark tones of the preview in RAW mode while
     the shadows are turned on. Not saved in the images.", and one `egui::Slider` over
     `AMOUNT_RANGE`, `.step_by(5.0)`, its text "Amount" (its doc comment says, as the
     sharpening's, that a value from the file that is not a multiple of 5 is kept until the
     slider is dragged). New `SettingsText` variants
     `ShadowsHeading`, `ShadowsNote`, `ShadowsAmount` (rule 13; no text in drawing code).
  3. **Restore Defaults** of the page Preview restores both sections: where
     `restore_sharpening` is handled, `chosen.shell.shadows_amount =
     shadows::DEFAULT_AMOUNT` too; the flag `restore_sharpening` is renamed `restore_preview`
     (its comment: the page Preview's Restore Defaults, T317, T334); kept only by Save.
  4. **The glossary:** the row "Preview (Settings page)" names the sections Sharpening and
     Shadows; a new row "Shadows (Settings page section), Amount": the amount (0–100) of the
     lift of the dark tones in the RAW preview, default 50 (ADR 0027 §3.4, W5-Q6; added by T334);
     it says it is not the "Deep shadows" of the clipping marks (the rows about line 260) and
     that its "Amount" is not the sharpening's "Amount (%)". The mode's menu item and mark are
     T335's.
- **Tests to add:**
  - `settings.rs`: the shell lines end with `shadows_amount = 50` after `sharpen_threshold`.
    These four existing tests are **updated** with the new line (asked for here, not a
    weakening, rule 5): `saved_file_has_the_comment_line_and_every_key` (the whole file, about
    lines 1271–1301); `the_thumbnail_buttons_are_written_and_read` (an `ends_with`, about lines
    2483–2498); the `move_to_subfolders` / `companion_suffixes` test (an `ends_with`, about lines
    2893–2899, and its comment "come last but `thumbnail_buttons` (T245) and the sharpening
    (T317)", which names the shadows too); and the `save_settings_with_library` test (about lines
    3040–3042), which expects `sharpen_threshold = 2` right before `external_program = …`.
    `changed_shell()` gets a non-default amount (for example 35).
  - `settings.rs`: written and read back, 0, 35 and 100 come back equal.
  - `settings.rs`: `shadows_amount = 101`, `= -1`, `= half` each keep 50 and give one problem
    with the expected text; a file without the line gives 50 and no problem.
  - `shell_text.rs`: the three new texts are those strings, added to the existing test
    `the_preview_page_has_the_sharpening_texts` (about line 1290).
- **Not in this task:** the key, the menu item, the mark and passing the amount to the view core
  (T335). Until T335 is done the section names a mode nothing can turn on: `main` is not moved
  forward and no release is made between T334's Done and T335's (chain review, 2026-11-25); no
  `CHANGELOG.md` line at T334's Done (T335's covers W5).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: every text by
  `settings_text`, the glossary rows). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for T333. Cold read: all names and lines found; fixed: "the last line" is the last shell line (library lines follow); the four tests to update named, the library test among them; the constants' types stated; the flag's name decided (`restore_preview`); doc comments and the glossary's distinction from "Deep shadows" named; the `shell_text.rs` test named. Sonnet kept: no gap left |
| 2026-11-26 | Open | Set to Open by the reviewer on the owner's word after T333 was Done; T333 changed no file this task names, so its text is unchanged |
