# T323. ADR 0027 W3: the button "USM" in the preview's title line — Open

- **Why:** W3's hand check (owner, 2026-11-25; ADR 0027 §3.3 and W3-Q5, added 2026-11-25, §4.3
  "W3's hand check"): a button turns sharpening on and off beside the RAW/JPEG control, so it can
  be done with the mouse and its state is seen at a glance. Waits for T321 (both change
  `preview_text.rs` and the glossary row of sharpening).
- **What the reviewer checked (2026-11-25):** in `crates/pw-loupe/src/main.rs`:
  - `struct StatusBar` (about line 324; derives `Clone, Debug, Default, PartialEq`): `line` and
    `mode: Option<String>`, the RAW/JPEG control's text. In the preview it is drawn as the
    preview's title line above the image (`draw_preview`, `Panel::top("preview-title")`, about
    line 6430); the main window's bottom status bar (about line 12805) is drawn only outside the
    preview, where `mode` is always `None`. So the control, and the new button, appear only in the
    preview's title line, also in the compare. `draw_status_bar` (about line 8779) draws `mode` as
    `ui.add_enabled(!blocked, egui::Button::new(mode.as_str()))` and pushes
    `Command::ToggleMode` on a click, then the line.
  - `mode` is set in `draw` (about line 9584) from
    `mode_label_with_key(frame.mode, self.key_map.first_key(KeyAction::RawJpeg))` while the loupe
    shows an image; elsewhere it is `None` (about line 4341, and `StatusBar::default()` about line
    9132). `self.next_status` is copied into `self.status_bar` when it changed (about line 12850),
    so the struct derives `PartialEq` and `Clone`.
  - `Command::Sharpening` toggles sharpening (`run_command` → `toggle_sharpening`, about line
    8256), and `command_checked(Command::Sharpening)` is `Some(self.sharpening)`. The menu draws a
    checked item with `egui::Button::new(..).selected(on)` (about line 8606).
  - `loupe_text` and `preview_text` are private modules; `main.rs` reaches their functions through
    `pub use` lists in `crates/pw-loupe/src/lib.rs` (about lines 125–128 and 152–153).
  - A click's command goes through `run_command`, which does nothing unless `command_enabled` is
    true and no dialog is open (about line 8206); `Command::Sharpening` acts in the loupe, where
    the button is shown.
  - `crates/pw-loupe/src/loupe_text.rs`: `mode_label_with_key(mode, key)` gives "Mode: RAW (R)",
    "Mode: RAW" without a key, with `key_text`.
  - `crates/pw-loupe/src/preview_text.rs`: `sharpen_mark` writes the hover text of the shown mark,
    "Sharpening (U): amount 80%, radius 0.8 px, threshold 2." and, without a key, "Sharpening:
    amount …" (after T320), in two arms of its own.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`
  (the two new functions in its `pub use` lists),
  `crates/pw-loupe/src/loupe_text.rs`, `crates/pw-loupe/src/preview_text.rs`, `docs/glossary.md`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. **The button's text:** `sharpen_button_label(key: Option<KeyChord>) -> String` in
     `loupe_text.rs`: "USM (U)" with the key map's key given (`key_text`), "USM" when the action
     has no key.
  2. **Its hover text:** `sharpening_values_text(key: Option<KeyChord>, values: Sharpening) ->
     String` in `preview_text.rs`, the text of the shown mark ("Sharpening (U): amount 80%, radius
     0.8 px, threshold 2." / "Sharpening: amount 80%, radius 0.8 px, threshold 2."); `sharpen_mark`'s
     two `Shown` arms call it, so the mark and the button say the same. The mark's texts do not
     change.
  3. **The title line:** `StatusBar` gains the button's state (a type that derives what
     `StatusBar` derives, `None` by default), set beside `mode` in `draw` and `None` wherever `mode`
     is `None` (about line 4341 too): its text from item 1 with
     `self.key_map.first_key(KeyAction::Sharpening)`, whether it is selected (`self.sharpening`)
     and its hover text from item 2 with the same key and `self.settings.shell.sharpening`.
     `draw_status_bar` draws it right after the RAW/JPEG control, as that control is:
     `add_enabled(!blocked, egui::Button::new(text).selected(on))` with `.on_hover_text(hover)`; a
     click pushes `Command::Sharpening`. No string literal in the drawing code (rule 13). The hover
     text gives the saved values also while sharpening is off (they are the values it would use).
  4. **The glossary:** the row "Sharpening, Show Sharpening, USM" names the button "USM" in the
     preview's title line, after the RAW/JPEG control, selected while sharpening is on.
- **Tests to add:** `sharpen_button_label` with U ("USM (U)"), with Shift+U ("USM (Shift+U)")
  and without a key ("USM");
  `sharpening_values_text` with and without a key, with the default values and with others
  (150, 1.3, 7); the existing tests of the mark still pass unchanged.
- **Not in this task:** what sharpening does (T321, T322); the compare (the button toggles the
  session's sharpening, which both sides follow already). No `CHANGELOG.md` line from the agent
  (the reviewer writes one at Done).
- **Rules:** `AGENTS.md`, `docs/tasks/README.md` (rule 13).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added after W3's hand check (owner, 2026-11-25); waits for T321 |
| 2026-11-25 | Blocked | Cold read (fresh agent): line numbers and claims checked. Added: `lib.rs` (the `pub use` lists) to the files; the control is in the preview's title line above the image, not the window's status bar (task, glossary item and ADR 0027 §3.3/W3-Q5 reworded); `StatusBar`'s derives; that a click is gated by `run_command`; the expected texts of the key test; that the hover text shows the saved values also while off |
| 2026-11-25 | Open | T322 Done; the owner chose T323 before T324 (2026-11-25) |
