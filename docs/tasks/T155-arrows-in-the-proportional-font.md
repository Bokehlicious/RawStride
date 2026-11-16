# T155. The arrows drawn as boxes: Hack as the last font of normal text — Open

- **Why:** the owner's job test (2026-11-16) found a box in the preview's Zoom button
  "Fit ↔ 100% (Z)". The program builds egui with `default_fonts` but without
  `monochrome_emoji_fonts`, so normal (proportional) text has only the fonts `Ubuntu-Light` and
  `egui-icons`. Neither has these arrows, and egui draws a box for each:
  - `↔` (U+2194): `PreviewText::ToggleZoom` "Fit ↔ 100% (Z)" (`preview_text.rs`) and the menu items
    "Fit ↔ 100%" and "RAW ↔ JPEG" (`command_text`, `shell_text.rs`);
  - `→` (U+2192): the import preview's lines (`preview_line`, `import_text.rs`) and the import row
    in Tasks (`import_route_text`, `tasks_text.rs`);
  - `↑` and `↓` (U+2191, U+2193): the sort direction button (`sort_direction_text`, `shell_text.rs`).

  egui already bundles the font `Hack` (used for monospace text), and it has all four. **Owner,
  2026-11-16:** `Hack` is added as the last font of the proportional family. No text changes, and no
  new font or dependency. The stars and "✕" under the thumbnails are drawn as shapes
  (`paint_rating_glyphs`, T043) and are not affected.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/shell.rs`, `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`.
- **What:**
  1. In `shell.rs`, add `pub fn font_definitions() -> egui::FontDefinitions`. It starts from
     `egui::FontDefinitions::default()` and appends `"Hack"` to the end of the
     `egui::FontFamily::Proportional` list. The fonts already in the list keep their order, so
     `Ubuntu-Light` stays the first font of normal text. If the family is missing from the map, the
     function returns the defaults unchanged (no panic, AGENTS.md §3). A doc comment gives the reason
     (the arrows of this task, owner 2026-11-16).
  2. Export it from `lib.rs` with the other `shell` items.
  3. In `main.rs`, in the `eframe::run_native` closure, call
     `cc.egui_ctx.set_fonts(font_definitions())` before the visuals and text sizes are set.
- **Tests to add** (in `shell.rs`):
  - `font_definitions` keeps the default proportional fonts first, in their order, and has `"Hack"`
    as the last proportional font; the monospace family is unchanged.
  - With an `egui::Context` that has `font_definitions()` set and has run one frame
    (`ctx.run_ui(Default::default(), |_| {}).drop_without_applying_deltas()`; egui 0.37 has no
    `ctx.run`, and a plain `let _ =` trips a `debug_assert!` on the unapplied font texture), every
    one of these texts has all its glyphs for `egui::FontId::proportional(14.0)`
    (`ctx.fonts_mut(|fonts| fonts.has_glyphs(&font_id, text))`): `preview_text` for every
    `PreviewText`, its ten variants listed by hand in the test as `preview_text.rs`'s own test does
    (no `ALL` constant is added); `command_text` for every item of `menu_items()`;
    `sort_direction_text(true)` and `sort_direction_text(false)`; `preview_line` for
    `PreviewLine { source_name: "A.ARW", rel: "folder/A.ARW", action }` with each of the four
    `ItemAction`s; `import_route_text("E:\\DCIM", "D:\\Photos")`. On failure the message names the
    text.
  - The tests rely on egui's `default_fonts` feature, which `pw-loupe` gets through `eframe`
    (the workspace's `egui` has default features off); the doc comment of `font_definitions` says so.
  - The same check with `egui::FontDefinitions::default()` reports `"Fit ↔ 100% (Z)"` as missing a
    glyph. This shows that the test above can fail, and it fails if a later egui version changes
    the defaults.
- **Not in this task:** changing any text; turning on `monochrome_emoji_fonts`; a new font file;
  the same glyph check for the other text modules (`editor.rs`, `info_panel.rs`, …): the script
  the reviewer ran on 2026-11-16 found no other missing characters in `pw-loupe`'s string literals.
- **Rules:** AGENTS.md §3 and §7; `docs/tasks/README.md` rule 13 (no text changes here).
- **Hand check:** the preview's Zoom section shows "Fit ↔ 100% (Z)" with an arrow; View ▸ "Fit ↔ 100%"
  and "RAW ↔ JPEG" in the menu, the sort direction button (↑/↓) and the import preview's lines (→)
  show arrows, not boxes.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added after the owner's job test (bug 2 of 5). The owner chose Hack as the fallback over changing the texts |
| 2026-11-16 | Open | Cold read: `ctx.run` does not exist in egui 0.37 (now `run_ui` + `drop_without_applying_deltas`); the `PreviewText` variants are listed by hand; sample `PreviewLine` given; the `default_fonts` reliance noted. The owner's decision is written into ADR 0017 §2.5 |
