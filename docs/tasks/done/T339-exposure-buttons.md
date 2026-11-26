# T339. ADR 0027 W5: the buttons "−", "A" and "+" for the exposure — Done

- **Why:** W5's hand check (owner, 2026-11-26; ADR 0027 §4.4 "W5's hand check"; W5-Q8 left a
  button to the hand check). Three buttons after "USM" in the status bar: **"−"** and **"+"**
  step the RAW view's exposure by 1/3 EV, as Ctrl+↓ and Ctrl+↑ (View ▸ Exposure ▸ Darker,
  Brighter); **"A"** turns auto exposure on and off, as B (View ▸ Exposure ▸ Auto Exposure),
  and is drawn selected while it is on. No number in the buttons: the preview's mark shows the
  total. The camera's brightness and Reset Exposure get no button. The second task of the chain
  T338 → **T339** → T340. Blocked: waits for T338 (one agent at a time; both chains' files are
  committed in order).
- **What the reviewer checked (2026-11-26):** T323 made the button "USM" and this task adds
  three beside it the same way:
  - `crates/pw-loupe/src/main.rs`: `StatusBar` (about line 326) with `sharpen:
    Option<SharpenButton>`; `SharpenButton` (about line 337: `text`, `on`, `hover`). The loupe's
    `draw` builds `self.next_status` (about line 9850) with `sharpen: Some(SharpenButton { … })`;
    the contact sheet's (about line 4509) has `sharpen: None`. `draw_status_bar` (about line
    9012) draws the RAW/JPEG control and the button "USM" with
    `add_enabled(!blocked, egui::Button::new(…).selected(…)).on_hover_text(…)` and pushes
    `Command::Sharpening` when clicked.
  - The commands already exist and act on both sides of the compare: `Command::ExposureDarker`,
    `Command::ExposureBrighter` and `Command::AutoExposure` are run by the menu's handler (about
    lines 8485–8489: `self.step_exposure(-1)`, `self.step_exposure(1)`,
    `self.exposure.toggle_mode(ExposureMode::Auto, false)`). `command_enabled` (about line
    8319) greys them unless `self.last_preview_source == Some(Source::Raw)` (about lines
    8375–8379); a step beyond −2 or +3 EV changes nothing (W5-Q2) and the menu items are not
    greyed there, so neither are the buttons.
  - The keys: `self.key_map.first_key(KeyAction::ExposureDarker)`, `ExposureBrighter`,
    `AutoExposure` (`keymap.rs`, defaults Ctrl+↓, Ctrl+↑, B).
  - `crates/pw-loupe/src/loupe_text.rs`: `sharpen_button_label` (about line 242) and its test
    `the_sharpen_button_shows_the_maps_key` (about line 388; `KeyChord` built as a literal,
    about line 390). `loupe_text` is a private module of the library (`lib.rs` line 31),
    re-exported by the `pub use loupe_text::{…}` at about lines 127–131 and imported by
    `main.rs` at about line 96.
  - Only two `StatusBar { … }` literals exist (about lines 4509 and 9850); the others use
    `StatusBar::default()`.
  - `docs/glossary.md`: the rows "Exposure, Brighter, Darker, Reset Exposure" (about line 289)
    and "Auto exposure, Auto Exposure" (about line 290); the row "Sharpening, Show Sharpening,
    USM" (about line 288) says where "USM" is.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/loupe_text.rs`, `crates/pw-loupe/src/lib.rs` (the re-export only),
  `docs/glossary.md`, `docs/test-inventory.md` (rule 12),
  and this task's own file (status and log).
- **Model:** Sonnet — one layer, two code files with their functions named, the commands
  existing, the tests specified, no design choice left (README, "Model").
- **What:**
  1. **The texts** (rule 13), in `loupe_text.rs` beside `sharpen_button_label`:
     - `pub fn exposure_button_label(button: ExposureButton) -> &'static str`: "−" (U+2212
       MINUS SIGN) for `Darker`, "A" for `Auto`, "+" for `Brighter`. `ExposureButton` is a new
       `pub enum ExposureButton { Darker, Auto, Brighter }` in `loupe_text.rs` (`Clone, Copy,
       Debug, PartialEq, Eq`). The three names are added to `lib.rs`'s `pub use
       loupe_text::{…}` and to `main.rs`'s import of it. Write the minus as `\u{2212}` (with a
       comment) in code and test, never ASCII `-`.
     - `pub fn exposure_button_hover(button: ExposureButton, key: Option<KeyChord>) -> String`:
       with a key, "Darker by ⅓ EV (Ctrl+↓).", "Auto exposure on or off (B).", "Brighter by ⅓
       EV (Ctrl+↑)." with the key map's key (`key_text`); without a key, the same sentence
       without the parenthesis ("Darker by ⅓ EV."). One `format!` template per sentence.
  2. **The state:** a new `struct ExposureButtons { auto_on: bool, enabled: bool, hovers:
     [String; 3] }` (`Clone, Debug, PartialEq`; the hovers in the order Darker, Auto,
     Brighter), and `StatusBar` gets `exposure: Option<ExposureButtons>`. The loupe's `draw`
     fills it beside `sharpen` (about line 9850): `auto_on` is `self.exposure.exposure_mode ==
     ExposureMode::Auto`, `enabled` is `self.command_enabled(Command::ExposureBrighter)` (the
     three commands share the condition), the hovers from `exposure_button_hover` with
     `first_key` of the three actions; `enabled` is read into a local before the literal. The
     contact sheet's `StatusBar` (about line 4509) gets `exposure: None`.
  3. **The drawing:** in `draw_status_bar`, right after the button "USM" and before the line,
     the three buttons in the order "−", "A", "+", each
     `ui.add_enabled(!blocked && buttons.enabled, egui::Button::new(label))` with its hover
     (egui 0.37 shows `on_hover_text` only on an enabled widget, so add both `on_hover_text`
     and `on_disabled_hover_text` with the same text); "A" is `.selected(buttons.auto_on)`. A click pushes `Command::ExposureDarker`,
     `Command::AutoExposure` or `Command::ExposureBrighter` into `commands`, so a click does
     exactly what the menu item does (both sides of the compare included). The three sit
     together with `ui.spacing_mut().item_spacing.x` left as it is (no new layout constant).
  4. **The glossary:** the two rows (about lines 289–290) name the buttons: "−" and "+" in the
     status bar after "USM" step as Darker and Brighter; "A" toggles auto exposure and is
     selected while it is on (ADR 0027 §4.4, W5's hand check; T339). The row about "USM" (about
     line 288) says "the button "USM" in the preview's title line"; leave it as it is.
- **Tests to add:**
  - `loupe_text.rs`: the three labels; the three hovers with the default keys (`KeyChord`s for
    Ctrl+↓, B, Ctrl+↑ as the existing tests build them), with Shift+B for Auto (a changed key
    shown), and without a key.
  - `main.rs`: a test of the function that builds `ExposureButtons` if it is split out (see
    below); otherwise none (the drawing is egui). To make the state testable, write a free
    function `fn exposure_buttons(mode: ExposureMode, enabled: bool, keys: [Option<KeyChord>;
    3]) -> ExposureButtons` and test: Auto gives `auto_on`, Off and Camera do not; `enabled`
    passed through; the hovers name the given keys.
- **Not in this task:** the setting "Auto exposure at start" (T340); a button for the camera's
  brightness, for Reset Exposure or for the shadows (not asked for).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: no text in drawing code;
  the glossary rows). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At its Done the reviewer adds a `CHANGELOG.md` line (Added: the buttons −, A and + after USM).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Added after W5's hand check (ADR 0027 §4.4); waits for T338. Cold read: names, lines and egui 0.37's API found; fixed: `lib.rs` added (the re-export of `loupe_text`), the minus as `\u{2212}`, only two `StatusBar` literals, both hover calls explained. Sonnet kept: the gap was the one-line re-export, now named |
| 2026-11-26 | Open | T338 done; its cold read is in the row above |
| 2026-11-26 | Ready for review | Added the three status-bar buttons and their shortcut hovers, using the existing exposure commands and availability. All three new tests passed. The sandbox check could not lock the RustSec database or start ExifTool. The full check and the owner's hand check are pending |
| 2026-11-26 | Done | Reviewed: labels and hovers in `loupe_text.rs` (minus as `\u{2212}`, one template per sentence), `ExposureButtons` and `exposure_buttons` in `main.rs`, the three buttons after "USM" pushing the menu's commands, enabled as `command_enabled(ExposureBrighter)`, both hover calls; the glossary rows; three tests as asked. The reviewer updated the inventory's total (2809→2812; the agent had changed only the `pw-loupe` row, 1504→1507). `.\tools\check.ps1` all OK: 2812 passed, 0 failed, 98 ignored; no clippy warnings. CHANGELOG: the buttons added to Unreleased ▸ Added (the exposure itself is not released yet). The owner's hand check follows |
