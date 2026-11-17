# T168. ADR 0017 U9a, second part: the focus marking key, menu item and drawing — Blocked: waits for T167

- **Chain:** T167 → T168. The owner's answers U9-Q1–U9-Q4 are in ADR 0017 §6.1 (2026-11-17).
- **Why:** ADR 0017 §2.6, step U9a. T167 gives the current image's focus map in the view core's
  frame (`Frame::focus`, `FocusState`, `ViewCore::set_focus_wanted`). This task gives the user the
  on/off choice (key E, a View menu item), remembers it, tells both view cores when a map is
  wanted, and draws the marks over the image in the preview, on both sides of the compare
  (U9-Q1, U9-Q4). It follows the clipping overlay's way through the shell (T148, T149:
  `clip_highlights`, `wish_clipping_map`, `ClipChoice`, `clipping.rs`, `clip_overlay`,
  `second_clip_overlay`, `draw_image_area`), which stays as it is.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/focus.rs` (new: the map turned into the
  overlay's pixels, and its texture cache, as plain Rust with tests),
  `crates/pw-loupe/src/clipping.rs` (only: `join` made `pub(crate)` so `focus.rs` can use it),
  `crates/pw-loupe/src/lib.rs` (the module, and the key E in the preview's key mapping),
  `crates/pw-loupe/src/settings.rs` (`focus_marking`), `crates/pw-loupe/src/shell.rs` (the
  command in the View menu), `crates/pw-loupe/src/shell_text.rs` (its text),
  `crates/pw-loupe/src/main.rs`; `docs/glossary.md`, `docs/test-inventory.md`.
- **What:**
  1. **Setting** (`settings.rs`): `focus_marking: bool` in the shell settings, `false` by default,
     written and read as `focus_marking = true|false` beside `clip_highlights`, with the same
     handling of a missing or bad value.
  2. **Key** (`lib.rs`): `PreviewKey::FocusMarking` for **E** with no modifier (not with Ctrl,
     Alt or Shift), in `preview_key_action` beside J; taught to `command_of_key` as the other
     preview keys. E does nothing in the contact sheet. Check that E is no key of any other key map
     (as the test for C, K, V does). The existing test `e_does_nothing` (about the editor before
     D16) checks `key_action`, `grid_key_action` and `loupe_key_action` only; it still holds and is
     left exactly as it is.
  3. **Menu** (`shell.rs`, `shell_text.rs`): `Command::FocusMarking`, "Show Focus Marking" (E), in
     the View menu right after "Show Deep Shadows", `Where::Loupe` in `acts_in`, ticked when on, as
     the two clipping items; its text in `command_text`, its key from `key_text`.
  4. **Wish and commands** (`main.rs`): `toggle_focus_marking`, reached from the `PreviewKey`
     match, `run_command` and `command_checked` (beside the clipping ones). A `wish_focus_map`,
     called every frame beside `wish_clipping_map`: the first core wants a map exactly when the
     preview is shown and `focus_marking` is on; the second core when the compare is open as well
     (U9-Q4). `close_compare` also calls `second_core.set_focus_wanted(false)` and releases
     `second_focus_overlay`, beside what it does for the clipping map.
  5. **Pixels** (`focus.rs`): `pub const FOCUS_RGBA: [u8; 4] = [0, 255, 0, 255]` (green, U9-Q3);
     `focus_overlay(map: &FocusMap, max_side: usize) -> Option<clipping::Overlay>`: one RGBA pixel
     per cell, `FOCUS_RGBA` where `FocusMap::EDGE` is set, `clipping::CLEAR_RGBA` elsewhere; a map
     wider or higher than `max_side` is joined 2 × 2 with `clipping::join` and gets the same
     `uv_scale` rule as `clipping::overlay` (the few lines of that rule are repeated in `focus.rs`;
     `clipping::overlay` is not changed); the module is `pub mod focus;` in `lib.rs`, as
     `clipping`; `None` for a map without cells, a `data` length that is
     not `width × height`, or a `max_side` of 0. A `FocusCache<T>` as `clipping::OverlayCache`:
     the texture made again only when the map (`Arc::ptr_eq`) or `max_side` changes, one texture at
     a time, `release()`.
  6. **Drawing** (`main.rs`): `ImageAreaShown` gets `focus: bool`; `draw_image_area` gets a
     `&mut FocusCache<egui::TextureHandle>` beside `clip_overlay`. With `focus` on, this frame's
     `Frame::focus` "ready" and an image shown, the marks are drawn as the clipping marks are (the
     `Draw` with `clipping::overlay_uv(uv, uv_scale)`, `pw_render::mesh`, nearest-neighbour), **before**
     the clipping marks, so a clipping mark is on top. Otherwise the cache is released. Two caches
     (`focus_overlay` and `second_focus_overlay`), released wherever `clip_overlay` and
     `second_clip_overlay` are released today. The marks come from this frame's `Frame::focus` and
     `Draw` only, never from an earlier frame's state.
  7. **Glossary** (`docs/glossary.md`, beside the clipping rows, in their form with "–" in the
     Swedish column): `| – | Focus marking, Show Focus Marking | edges of high local contrast
     marked in green on the image in the preview (ADR 0017 §2.6, U9-Q1–U9-Q4); "Show Focus Marking"
     (E) in the View menu, ticked when on; added by T168 |`.
  8. **Existing tests that list every key, field or item** are extended with the new one, and
     nothing else in them changes (this is not a rewrite under rule 5): `settings.rs`'s
     `saved_file_has_the_comment_line_and_every_key` (the saved text gains `focus_marking = false`)
     and `changed_shell()` (the new field); `shell.rs`'s View menu order test (its slice grows by
     one item); `shell_text.rs`'s list of menu texts. `docs/test-inventory.md` names `src/focus.rs`
     in `pw-loupe`'s file list, besides the counts.
- **Tests to add:**
  - `focus.rs`: an edge cell green and others transparent; the joining marks a joined cell when any
    of its cells is an edge, and fits the given largest side; the `uv_scale` for an odd number of
    cells; `None` for a bad `data` length and for `max_side` 0; the cache does not remake the
    texture for the same map and side, and does for a new map.
  - `settings.rs`: `focus_marking` written and read, `false` by default and when missing.
  - `lib.rs`: E gives `PreviewKey::FocusMarking` only without modifiers, and is no key of the
    contact sheet or of another key map.
  - `shell.rs`, `shell_text.rs`: the menu order and the item's text and key.
- **Not in this task:** a section in the right column; another colour or a strength setting (one
  colour, one threshold, U9-Q3); the contact sheet. Tuning `FOCUS_THRESHOLD` (T167's constant in
  `pw-viewcore`) is done after the owner's hand check, as a log row here if the value changes.
- **Rules:** AGENTS.md §3; `docs/tasks/README.md` rule 13 for the new text (in `shell_text.rs`, the
  glossary row in this task).
- **Hand check by the owner** (ADR 0017 §7), for the whole step U9a: a RAW file with a sharp
  subject and a blurred background, and a JPEG: E marks the sharp edges in green at fit and at
  100 %, on a vertical image, in RAW and JPEG mode; the marks stay when stepping and zooming; with
  J on as well both show; in the compare on both sides; the menu tick follows the key; a restart
  keeps it. The owner says whether the threshold marks too much or too little.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added for ADR 0017 U9a; waits for T167. Nine files, one of them a visibility change in `clipping.rs` (reviewer: accepted rather than copying `join`) |
| 2026-11-17 | Blocked | Cold read: doable with the named files; E is free in every key map. Changed: `Command::FocusMarking` and its places (`acts_in`, `command_text`, `command_checked`, `run_command`, `command_of_key`, the `PreviewKey` match) named; `e_does_nothing` left as it is; `close_compare` releases the second side's focus wish and texture; `pub mod focus`; the `uv_scale` rule repeated, not shared; the glossary row in the clipping rows' form; the existing list tests to extend named (point 8); `src/focus.rs` in the inventory |
