# T43. ADR 0013 B3: rating keys and display in the grid and the loupe — Done

- **Second review 2026-11-09: code accepted.** Points 1 and 2 below are fixed; point 3 is decided by
  the owner during the hand check. Waiting for the owner's hand check ("Done when").
- **Review 2026-11-09: changes requested.** Keys, focus rule, grid row, dimming and the badge are
  accepted. Still to do, in `crates/pw-loupe/src/main.rs` only:
  1. **The star is drawn wrongly.** `paint_rating_glyphs` fills the ten-point star with
     `egui::Shape::convex_polygon`, but a star is not convex: egui fills a convex polygon as a fan
     from its first point, so the notches between the tips are filled in (the star looks like a
     pentagon) and the edge anti-aliasing is wrong. Draw it as ten triangles from the centre in an
     `egui::Mesh` (centre, point *i*, point *i*+1), or as the inner pentagon plus five tip triangles,
     each of which is convex. Keep the size and spacing.
  2. **Badge width:** measure the label text with `painter.layout_no_wrap(…)` (the galley's
     `rect.width()`) instead of `chars().count() × 8`, so "Röd" and "Lila" neither overflow nor leave a
     gap.
  3. **Badge content:** ADR 0013 §5.2 asks for the same look as the grid row (stars on the label
     colour). The Swedish label name in the badge is an addition; keep it only if the owner says so
     after looking at it — otherwise remove it (the status line already names the label).
  Then run `.\tools\check.ps1` and hand over for the owner's manual check (the "Done when" list).

- **Start after T42 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/ratings.rs`
  only for a small plain function with a test (for example the label colours).
- **What:**
  1. **Keys:** in the key loop, after the dialog and tab checks and before the mode-specific keys,
     `rating_key_action(key, ctrl, shift, alt)` acts on the current image of the active tab (grid: the
     current cell; loupe: the shown image, `tab.current`). Skip rating keys when
     `ctx.wants_keyboard_input()` is true or the import dialog consumes keys (ADR 0013 §4.3). A refusal
     puts `UNSUPPORTED_TEXT` in the status line.
  2. **Grid row (ADR 0013 §5.1):** in the row under each thumbnail, `star_glyphs` on the left, the file
     name (shortened as today) after it. With a label, the row's background is the label colour —
     Red `#e5484d`, Yellow `#f5c400`, Green `#3fb950`, Blue `#4c8dff`, Purple `#a371f7` — with dark text
     on Yellow and Green. A rejected image's thumbnail is drawn dimmed (for example a tint of about 40 %).
  3. **Loupe badge (§5.2):** in the lower left corner of the image area, the same look as the grid row
     (stars on the label colour), only when the image has stars or a label, and only while the
     information is shown (I key). The status line shows `status_text` as today's rating summary does.
  4. If egui's bundled fonts lack "★", "•" or "✕", draw them as shapes instead; no new font.
- **Rules:** as T28; drawing reads `RatingStore::shown` only, never files; no other key changes.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0013 §8.2): keys on
  the main row and the number pad in grid and loupe; Shift+digit and AltGr do nothing; digits typed
  into the import dialog never rate; ARW, RW2, DNG and JPEG keep their values after reopening the
  folder; a DNG rated in RAW mode still shows correctly after stepping away and back; a read-only
  sidecar gives the message and the old value; holding a digit while stepping keeps the loupe smooth.
