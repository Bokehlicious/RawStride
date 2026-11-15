# T149. ADR 0017 U7b, last part: the clipping marks drawn on the image — Blocked

- **Blocked: waits for T148** (the keys, settings and section), which waits for T147 and T146.
  The owner's answers to U7b-Q1–U7b-Q10 are in `docs/tasks/done/T146-clipping-map-of-raw-data.md`.
  Chain: T146 → T147 → T148 → T149.
- **Why:** ADR 0017 §2.6, step U7b. T147 gives the current image's clipping map with the
  coordinates that place it on the drawn image, and T148 the user's two on/off choices. This task
  draws the marks over the image in the preview.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/clipping.rs` (new: the map turned into
  the overlay's pixels and coordinates, as plain Rust with its tests), `crates/pw-loupe/src/lib.rs`
  (only the module), `crates/pw-loupe/src/main.rs` (only: the overlay's texture kept in the app,
  and the overlay drawn in `draw` after the image); `docs/test-inventory.md`.
- **What:**
  1. **The overlay's pixels** (`clipping.rs`, U7b-Q4): from the frame's map, one RGBA pixel per
     cell: solid red where a clipped bit (any of R, G, B) is set and the highlight overlay is on;
     solid blue where the deep-shadow bit is set and the shadow overlay is on; fully transparent
     elsewhere.
  2. **Too large for the GPU:** a map wider or higher than the largest texture side
     (`max_texture_side`, already read in `draw`) is made smaller by joining 2×2 cells (a joined
     cell is marked when any of its cells is) until it fits. A side with an odd number of cells
     gives a joined grid that reaches past the map by half a joined cell; the texture coordinates
     are scaled for it (for example by `w / (2 · ceil(w / 2))` per joining), so the marks stay in
     place.
  3. **Made once:** the texture is made again only when the map (T147's `Arc`, compared by
     `Arc::ptr_eq`) or the overlay choice changes, not every frame, and released when no overlay is
     shown. Its memory is outside the view's texture budget (U7b-Q8); one texture at a time.
  4. **Drawn over the image** in the preview's image area: with the `Draw`'s destination
     rectangle and T147's four texture coordinates of the map (scaled as in point 2), with
     nearest-neighbour sampling, so each mark is a sharp block (U7b-Q7). `pw_render::mesh` may be
     used for the quad. Nothing is drawn when both overlays are off, while the frame's clipping
     state is not "ready", or when the image area shows no image.
  5. **Texts:** none (T148 has them).
- **Tests to add:** `clipping.rs`: a highlight bit gives red only when that overlay is on, a
  shadow bit blue only when that one is on, other cells transparent; the joining marks a joined
  cell when any of its cells is marked and fits the given largest side; the coordinate scaling for
  an odd number of cells (a corner of the map lands on the same image point before and after
  joining); the texture is not remade for the same map and choice.
- **Hand check by the owner** (ADR 0017 §7), for the whole step U7b: a RAW file with burnt-out sky
  and one with black shadows: J and Shift+J mark them in red and blue, at fit and at 100 %, on a
  vertical image, in RAW and JPEG mode; the marks stay when stepping and while zooming; the check
  boxes and menu ticks follow the keys; a restart keeps them.
- **Not in this task:** a colour per channel (the map keeps the channels for later, U7b-Q2);
  marks in the contact sheet or the inspector; blinking; changes to `pw-viewcore` or
  `pw-libraw`.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Split from T148 after the cold read (the drawing part); waits for T148 |
