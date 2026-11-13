# T25. ADR 0011 K3: grid model in pw-loupe — Done

- **Can be done in parallel with T23** (different crate).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/` (e.g. a new `grid.rs` declared in `lib.rs`),
  `crates/pw-loupe/src/tabs.rs`, `crates/pw-loupe/src/lib.rs`. **Not** `main.rs` (drawing is K4).
- **What (ADR 0011 §3.1, §11):**
  1. **Layout in plain Rust:** the number of columns for an available width, cell size and spacing
     (at least 1); the row of an index; the range of indexes visible for a scroll offset and view
     height, plus a margin of rows.
  2. **Navigation:** for a count, a column count and a current index: Left, Right, Up, Down (by one
     row), PageUp, PageDown (by the number of visible rows), Home, End; clamped at the edges, including
     a short last row (Down from the row above it goes to the last image).
  3. **View mode per tab:** add a mode (`Grid` or `Loupe`) to `Tab`. A newly opened folder starts in
     `Grid` (GR13); a tab keeps its mode when switching tabs (Q5).
  4. **Keys** as plain functions with tests:
     - in the grid: arrows, Page Up/Down, Home/End → navigation; **Enter, G, Space** → open the loupe;
       Escape → nothing (Q6);
     - in the loupe: **Escape, G, Enter** → back to the grid (ADR 0009 §3.3).
     `key_action` (the loupe's own keys) must not change, and `tab_action` stays as it is.
  5. **Thumbnail size:** clamp to 96–480 px, default 200 (Q2); a step function for Ctrl + mouse wheel.
- **Rules:** no egui drawing; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** columns for several widths; visible range at the top, middle and end; every
  navigation direction including both edges and a short last row; a new tab starts in `Grid`; the mode
  survives tab switching; the key functions for both modes; size clamping.
- **Done when:** `.\tools\check.ps1` is all OK.
