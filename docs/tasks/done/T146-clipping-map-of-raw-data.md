# T146. ADR 0017 U7b, first part: the clipping map of the RAW data — Done

- **Can start now.** The owner approved the proposals of U7b-Q1–U7b-Q10 (at the end of this
  task) on 2026-11-15; they are written into the tasks of the chain and into ADR 0017 §6.1. They
  decide the whole step U7b, which is the chain **T146 → T147 → T148 → T149**: this task the map
  in the C++ shim and `pw-libraw`; T147 the view core asking for it and giving it with the frame;
  T148 the keys, menu items, settings and the section in the preview; T149 the overlay drawn on
  the image.
- **Why:** ADR 0017 §2.6 and step U7b: clipped highlights and the deepest shadows are marked on
  the image, in RAW mode from the RAW data. T142 counts them (the shares in the Histogram
  section) but does not say where they are. This task gives, for an opened RAW file, a map of the
  visible image saying which parts are clipped or in the deepest shadows, by the same rules as
  T142's numbers, from the RAW data `pwlr_open` has already unpacked.
- **Crates:** `pw-libraw`. **Files:** `crates/pw-libraw/csrc/pwlr.cpp` (a new function beside
  `pwlr_statistics`), `crates/pw-libraw/src/lib.rs` (its binding, `Session::clipping_map` and the
  type `ClippingMap`, with unit tests where the crate has them), `crates/pw-libraw/tests/statistics.rs`
  (the fixture tests below); `docs/test-inventory.md`.
- **What:**
  1. **A new shim function `pwlr_clipping_map`** (U7b-Q2, U7b-Q3, U7b-Q7) and its `Session`
     method `clipping_map`. What `pwlr_statistics` and `Session::statistics` give is **not**
     changed (the inspector's reader of T144 uses them and must not get a map). Like
     `pwlr_statistics`, it reads `imgdata.rawdata` only, calls no LibRaw function, decodes
     nothing, throws nothing (work buffers with `nothrow`), and uses the same levels (black level
     with its pattern, white level `linear_max` or `maximum`, the deep-shadow limit 1/256 of the
     white level above black) and the same layout checks, with the same "not made for this
     layout" answer (0 and a reason) for what `pwlr_statistics` refuses. **To share them,** the
     layout checks, the black-level pattern (`common`, `rest`) and the levels may be moved out of
     `pwlr_statistics_of` into helpers that both functions call; the statistics' results must stay
     exactly as they are (T142's tests in `tests/statistics.rs` pass unchanged). The map needs only
     each colour's black, white and shadow levels, not the 64K bin tables of `pwlr_levels`; it may
     skip building them.
  2. **The map** covers the visible image (`sizes.width` × `sizes.height`, the same grid as
     `Session::width()`/`height()` and the render's regions), in **sensor orientation** (no flip
     applied), divided into **cells**: 2×2 photosites for a Bayer mosaic and for a three- or
     four-colour image, 3×3 for X-Trans, counted from the visible image's top left corner. The
     map has `ceil(width / cell)` × `ceil(height / cell)` cells; a cell at the right or bottom edge
     that holds fewer photosites is judged on those it holds. One byte per cell, row by row:
     - bit 0, 1, 2: **clipped** in R, G, B: at least one photosite (pixel) of that colour in the
       cell has a raw value at or above the white level, before white balance (U7-Q4; the two
       greens of a Bayer sensor are G);
     - bit 3: **deep shadows**: every photosite of the cell, in every channel, is below the
       deep-shadow limit above black (U7-Q4, as T142 judges a cell);
     - bits 4–7: 0.
     **The buffer:** Rust allocates it and the shim fills it, in two calls. Called with no buffer
     (a null pointer), the function makes the same layout checks and returns the cell, the width
     and the height of the map (in cells), or the "not made for this layout" answer. Called with a
     buffer and its length, it checks that the length is exactly `width * height` for the same
     file (otherwise an error, nothing written) and fills it. The shim never keeps a pointer to
     the buffer after it returns.
  3. **`ClippingMap`** in `pw-libraw`: `cell` (photosites per cell side), `width` and `height`
     (in cells), `data: Vec<u8>` (one byte per cell as above), with public constants or methods
     for the four bits. `Session::clipping_map(&mut self) -> Result<Option<ClippingMap>>`:
     `Ok(None)` for a layout the shim is not made for, `Err` on an internal failure, as
     `Session::statistics`.
  4. **The same rules as the numbers:** for the same file, the map must agree with
     `Session::statistics` where they count the same thing: a cell with a clipped bit has at least
     one clipped photosite of that colour, and no clipped photosite lies in a cell without that bit;
     in a mosaic (Bayer, X-Trans), a whole cell (not cut by the edge) has the deep-shadow bit
     exactly when T142 counts it as deep shadows. (A three- or four-colour image is counted per
     pixel by T142 and per 2×2 cell here, so the two numbers are not compared for it.)
- **Tests to add:** `pw-libraw` (`tests/statistics.rs`, with `PW_TESTDATA`; ignored without it, as
  the other fixture tests), on the files T142's tests use (`TN006067.ARW` with clipping,
  `TN006331.ARW` without, `TN100426.ARW` dark): the cell is 2 for these Bayer files, and
  `width * cell` and `height * cell` are at least `Session::width()` and `height()` and less than
  them plus the cell (so the map's grid is the render's); only bits 0–3 are used; the number of
  cells with the deep-shadow bit, among the whole cells, times 4 equals the statistics'
  `deep_shadows` (the dark file has some); the file with clipping has cells with a clipped bit and
  the one without has none; a render before and after does not change the map; T142's statistics
  tests pass unchanged. The edge cells cannot be tested without a file (the shim has no test seam);
  if the fixture files' sizes are all divisible by 2, say so in the report.
  **Added in the review (owner, 2026-11-15): X-Trans.** The owner added five Fujifilm X-T30 files
  to `PW_TESTDATA` (`_DSF4243.RAF`, `_DSF4248.RAF`, `_DSF4252.RAF`, `_DSF4254.RAF`, `_DSF4271.RAF`;
  6246 × 4170, X-Trans). Add, in `tests/statistics.rs` and ignored without `PW_TESTDATA` as the
  others, a test over every `.raf` file in the folder (none found is a failure, so the test
  cannot pass empty): statistics and a map for each; the statistics add up (the bins of each
  channel are its `pixels`, the three `pixels` together are `Session::width() × height()`,
  `shadow_pixels` is the whole 3×3 cells' photosites, shares within 0–1); the map's cell is 3, its
  size `ceil(width / 3)` × `ceil(height / 3)`, only bits 0–3; the whole cells with the deep-shadow
  bit times 9 equal the statistics' `deep_shadows`; each channel has clipped cells exactly when the
  statistics have clipped photosites of it, and no more cells than photosites; the map is the same
  after `render_half`. And for the named files: `_DSF4271.RAF` has clipped cells in R and G and
  none in B, and the other four have no clipped cells (the reviewer's probe found this). Do not
  add `.raf` to the existing every-file test (its checks assume a Bayer pattern); keep it as it
  is. Update the test's module comment and `docs/test-inventory.md` with the new test. The sizes
  are divisible by 3, so the edge cells stay untested; say so in the report.
- **Not in this task:** the view core (T147), keys and texts (T148), the drawing (T149), JPEG and
  TIFF (T147), any change to what `pwlr_statistics` and `Session::statistics` give, or to the
  render.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency; the C++ shim's existing
  rules (no exception crosses into Rust; every return checked; nothing allocated that can throw).
  This task changes the C++ shim, so it gets an independent review of the diff before the owner is
  asked to approve (README, "An extra code review").
- **Done when:** `.\tools\check.ps1` is all OK, the ignored `statistics` fixture tests pass with
  `PW_TESTDATA` set (give their `test result` line), and `git diff --stat` shows only the files
  above.

## Questions for the whole step U7b (the reviewer's proposals; the owner answers before T146 opens)

| # | Question | Proposal |
|---|---|---|
| U7b-Q1 | How is the work split? | Four tasks: T146 the map from the RAW data (C++ shim, `pw-libraw`); T147 the view core asks for it and gives it with the frame, and makes it for JPEG and TIFF; T148 the keys, menu items, settings, section and texts in the preview; T149 the overlay drawn on the image. The owner's hand check comes after T149 |
| U7b-Q2 | What is marked as clipped? | A part where at least one channel is at the clipping level, by T142's rule (RAW: a raw value at or above the white level, before white balance; JPEG/TIFF: 255). Which channel is kept in the map, so a colour per channel can come later without new computing |
| U7b-Q3 | What is marked as deep shadows? | T142's deep shadows: every channel below 1/256 of the white level above black (JPEG/TIFF: every channel 0), so the marks are exactly what the panel's number counts |
| U7b-Q4 | Colours | Clipped highlights solid red, deep shadows solid blue, drawn over the image; nothing else of the image changes |
| U7b-Q5 | Keys | **J** highlights on/off, **Shift+J** deep shadows on/off (J is free today); both also in the View menu, ticked when on, with their keys shown; they act in the preview |
| U7b-Q6 | Where and how long | In the preview only (not the contact sheet's thumbnails). Each overlay stays on when stepping to another image and is remembered after a restart (`settings.txt`), as the column sections are |
| U7b-Q7 | How fine? | One mark per cell of 2×2 photosites (3×3 for X-Trans) in RAW mode, the cell that holds every colour; 2×2 pixels in JPEG and TIFF mode, so a map is a quarter of the image's pixels and fits a GPU texture. At 100 % a mark is 2×2 screen pixels; beyond 100 % it grows with the image |
| U7b-Q8 | When is it computed, and how much is kept? | Only while an overlay is on: the view core asks for the current image's map after its first view is on the screen (as the statistics, U7-Q6), so the first image is not slower and nothing is computed when both are off. The maps of the last 4 images shown are kept (a 61 MP file's map is about 15 MB); an older one is computed again when needed. Until the map is ready nothing is marked. The shown overlay is one GPU texture of 4 bytes per cell (about 61 MB for a 61 MP file), outside the view's texture budget |
| U7b-Q9 | The right column | A section "Clipping" under Histogram, collapsible and remembered, with two check boxes "Clipped highlights (J)" and "Deep shadows (Shift+J)" that do the same as the keys; it says "Reading …" while the map is computed and "No clipping map for this file" when there is none |
| U7b-Q10 | Names (English, GR14) | Clipping, Clipped highlights, Deep shadows (as the Histogram section's term), Show Clipped Highlights, Show Deep Shadows (the menu), No clipping map for this file; "Reading …" as today; in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added with T147–T149 (the chain of U7b); waits for U7b-Q1–U7b-Q10 |
| 2026-11-15 | Blocked | Cold read (fresh agent, of T146–T148): helpers shared with `pwlr_statistics_of` allowed, its results unchanged; the two-call buffer protocol made required; the deep-shadow comparison limited to mosaics; the fixture files named and a grid check against `Session::width()` added; the damaged-file test and the edge-cell unit test removed (`pwlr_open` refuses damaged files; the shim has no test seam). T148 was too big and is split into T148 and T149; U7b-Q1, Q5, Q8–Q10 changed with it ("Deep shadows" as the Histogram section's term, the texture's memory stated) |
| 2026-11-15 | Open | The owner approved U7b-Q1–U7b-Q10 as proposed; written into ADR 0017 §6.1 |
| 2026-11-15 | Ready for review | Done by the task agent: `pwlr_clipping_map` (two calls, the map's own rows as work space) and the shared helpers `pwlr_layout_of` and `pwlr_limits` in `pwlr.cpp` (`pwlr_statistics_of` now calls them, same checks in the same order and same results); `ClippingMap` and `Session::clipping_map` in `lib.rs` with 2 unit tests; 3 fixture tests in `tests/statistics.rs`. `check.ps1` all OK (1519 passed, 61 ignored, 0 failed); the `statistics` fixture tests with `PW_TESTDATA`: 7 passed. Every fixture file's width and height are even, so the edge cells are not tested. Needs the independent review of the diff (shim change) |
| 2026-11-15 | Open | Reviewed: the diff is right (the reviewer's own reading and an independent review agent found no bugs; `check.ps1` all OK; the `statistics` fixture tests with `PW_TESTDATA`: 7 passed). The independent review noted, not blocking: the clipped-bit check in the test is rough (the statistics cannot show every clipped photosite's cell; the deep-shadow check is exact); edge cells, X-Trans and three- or four-colour RAW are untested. **Sent back for one thing:** the owner added five Fujifilm X-T30 RAF files (X-Trans); a reviewer's probe (not kept) found statistics and map right on all five. Left: the X-Trans test added under **Tests to add** ("Added in the review"); nothing else is to change |
| 2026-11-15 | Ready for review | X-Trans test added by the task agent: `fujifilm_files_have_statistics_and_a_clipping_map_that_agree` in `tests/statistics.rs` over every `.raf` file (none found fails), with the checks of "Added in the review" and the named files (`_DSF4271.RAF` clipped cells in R and G, none in B; the other four none; each named file must be found); module comment and `docs/test-inventory.md` updated; the every-file test unchanged. All five files are 6246 × 4170, divisible by 3, so the edge cells stay untested. The `statistics` fixture tests with `PW_TESTDATA`: 8 passed; `check.ps1` all OK. No change to `pwlr.cpp` or `lib.rs` in this round |
| 2026-11-15 | Done | Reviewed again: the X-Trans test does what the review asked; `check.ps1` all OK (1519 passed, 0 failed, 62 ignored); the `statistics` fixture tests with `PW_TESTDATA`: 8 passed, run again by the reviewer. Approved by the owner. Leftovers: the edge cells, a black level per position and three- or four-colour RAW data are in `docs/test-inventory.md`'s "Not covered" line (the sample files in `docs/tasks/backlog.md`); the rough clipped-bit check is noted there too by the statement of what the test checks. Next: T147 |
