# T46. ADR 0013 §5: rounded, inset label bar in the grid and the loupe badge — Done

- **Start after T43 is done.** For a Claude agent (drawing code, checked by hand).
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only.
- **Why (owner, 2026-11-09):** the label colour fills the whole row under a thumbnail, edge to edge,
  with square corners. The owner wants it inset from the cell edges and clearly rounded.
- **What:**
  1. **Grid row:** draw the label colour as a rounded rectangle **inside** `label_rect`: about 6 pt in
     from the left and right edges and 2 pt from the top and bottom, with the corner radius equal to
     half its height (a pill shape). Use the painter's rounded `rect_filled` (corner radius), no new
     shape code. Without a label nothing is drawn behind the row, as today.
  2. The stars and the file name start inside the pill (move their left edge in by the same inset)
     and the file name stays clipped to the row.
  3. **Loupe badge:** the same rounding (radius = half the badge height) for both the label colour
     and the dark background used when there is no label.
  4. Keep the colours, the dark text on Yellow and Green, the dimming of rejected thumbnails, the
     cell size and the selection frame as they are.
- **Rules:** as T43; no new dependencies; rules 6, 7, 10–12.
- **Tests:** none required (drawing only); if you factor the inset rectangle into a small plain
  function, give it a test.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: the colour bar no longer
  touches the cell edges, its ends are round, the stars and name sit inside it at thumbnail sizes 96,
  200 and 480, and the loupe badge has the same shape.
