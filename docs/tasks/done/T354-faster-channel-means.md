# T354. ADR 0027 W6: the RAW channel means made faster (pw-libraw) — Done

- **Why:** ADR 0027 W6-Q7, decided by the owner 2026-11-28 after T348 (ADR 0027 §4.6, "W6-Q7
  after T348"). With auto white balance on, a step to an image that was not prefetched waits for
  the means of the whole image before its first view. T348's means take 269–275 ms on the
  A7R IV's ARW (8660×5784), 328–332 ms on the A1's (9568×6376) and 128–131 ms on the X-T30's RAF,
  more than the statistics (169–180, 205–215 and 85–89 ms), which read the same data. T348's
  independent review found why: `pwlr_channel_means_of` divides by the colour pattern's width
  (`col % L.fcols`) for every photosite, and by the black level pattern's width
  (`col % L.pcols`) for every photosite of a file with such a pattern. The statistics avoid the
  colour pattern's remainder with a running counter (`k`, line 848); they, and the clipping map,
  still take `col % pcols` per photosite. This task makes the means faster **without changing
  what they give**. It comes between T348 and T349 in W6's chain (T348 → **T354** → T349 → … →
  T353); T349 waits for it, and it waits for T348 to be Done and committed (2026-11-28).
- **What the reviewer checked (2026-11-28):** `crates/pw-libraw/csrc/pwlr.cpp`:
  `pwlr_channel_means_of` (line 730, a template over `Mosaic`) and its one caller
  `pwlr_channel_means` (line 788); the cell's first column `cx` is always a multiple of
  `L.cell` (2 or 3), counted from the image's left edge, so within a cell the column's place in
  the colour pattern is known once per cell: for a Bayer pattern (`fcols` 2) it is `dx`, for
  X-Trans (`fcols` 6) it is `(cx % 6) + dx` with `cx % 6` either 0 or 3. `pwlr_statistics_of`
  (line 813) and `pwlr_clipping_map_of` show the running counters. The tests that hold the
  means' results are `channel_means_count_complete_cells_and_leave_statistics_unchanged`
  (it prints the whole image's means of every file of `PW_TESTDATA`) and the timing
  `timing_of_white_balance` (`crates/pw-libraw/tests/white_balance.rs`, lines 320 and 550).
- **Crate:** `pw-libraw`. **Files:** `crates/pw-libraw/csrc/pwlr.cpp` and this task's own file
  (status and log). No other file changes; no Rust code and no test changes.
- **Model:** Opus — the C++ shim (README, "Model").
- **What:**
  1. **Before changing anything,** run with `PW_TESTDATA` set
     `cargo test --workspace --test white_balance -- --ignored --nocapture
     channel_means_count_complete_cells` and keep its output outside the repository (for example
     in the session's temporary folder). These are the results the change must keep.
  2. In `pwlr_channel_means_of`, take every division and remainder by a value known only at run
     time out of the loop over photosites: the colour of a photosite comes from the cell's place
     in the colour pattern, worked out once per cell (or kept by a running counter, as the
     statistics' `k`), and the black level pattern's column by a running counter that starts at
     `cx % L.pcols` once per cell (or carries on from the previous cell) and goes back to 0 at
     `L.pcols`. Do not assume that `pcols` divides `cell` or is at least `cell` (it can be 1, or
     2 on an X-Trans file): the counter wraps within a cell as well.
     Anything else that can be worked out once per row of cells or once per cell (for example
     a colour's black level with the pattern's extra part) may be. The rules stay exactly as
     they are: whole cells fully inside the rectangle, counted from the image's top left corner;
     a cell left out when any photosite is at or above its colour's white level or every
     photosite is below the deep-shadow limit; the black level taken off; both greens are G.
     Nothing that can throw is allocated, and `pwlr_statistics_of` and `pwlr_clipping_map_of`
     are not changed. The comment above the function says how a photosite's colour is found.
  3. Run the same test as in point 1 again: the printed means, counts and multipliers of every
     file must be **identical** to point 1's. Compare only the lines holding `whole means` and
     `12x12` (cargo's own lines, such as `finished in`, differ every run), for example with
     `Select-String 'whole means|12x12'` on both outputs and `Compare-Object`. If they differ in
     anything, stop and report.
  4. Run all of `tests/white_balance.rs` on `PW_TESTDATA` with
     `cargo test --workspace --test white_balance -- --ignored` (it must pass), and the timing
     with `cargo test --workspace --release --test white_balance -- --ignored --nocapture
     timing_of_white_balance`; write its `means` and `open+means+half` values of each file kind
     (ranges, as T348's log) in the log row, beside T348's numbers.
- **Tests to add:** none. The change keeps the behaviour, and point 3's comparison and the
  existing tests of `tests/white_balance.rs` (counts of a 12 × 12 area, additivity, the same
  result twice, the statistics unchanged) are what hold it.
- **Not in this task:** sharing a pass with the statistics (W6-Q1: they cannot); the view core's
  use of the means (T349, T350); any change to the statistics or the clipping map.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12. No new dependency. An extra code
  review follows (the C++ shim).
- **Done when:** `.\tools\check.ps1` is all OK, `git diff --stat` shows only the files above,
  point 3's outputs are identical, and the log holds the timings.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added by the reviewer after the owner's decision on W6-Q7 at T348's review (ADR 0027 §4.6); waits for the cold read |
| 2026-11-28 | Blocked | Cold read: the statistics keep a running counter only for the colour, not for the black level pattern (Why and point 2 corrected); `pcols` need not divide the cell; point 3 compares only the means' lines (cargo's own lines differ every run); point 4's commands and the timing's column names written out; the wait for T348 stated |
| 2026-11-28 | Open | Set to Open by the reviewer after the cold read, on the owner's decision at T348's review (W6-Q7) |
| 2026-11-28 | Ready for review | Removed the per-photosite remainders. All 126 printed comparison lines matched across 63 RAW files, and all 9 white-balance tests passed. Release timings on `PW_TESTDATA` (i7-9700K, Windows build 26300, range of per-file medians of 5, in ms): A7R IV ARW (19 files), `means` 173.10–175.02 (T348 269–275), `open+means+half` 653.83–666.70 (717–738). A1 ARW (5 files), `means` 211.29–212.63 (328–332), `open+means+half` 805.59–815.27 (881–889). X-T30 RAF (5 files), `means` 71.64–77.07 (128–131), `open+means+half` 509.65–519.42 (540–565). |
| 2026-11-28 | Done | Reviewed by the reviewer: point 2 done as written (the colour from the cell's place in the pattern, the black level pattern's column by a counter that wraps within a cell). The reviewer ran the means test on T348's shim (put back for the run) and on this one: the 126 `whole means` and `12x12` lines of the 63 files identical; `tests/white_balance.rs` with `--ignored` on `PW_TESTDATA`: 8 passed (the timing is the agent's, above). `.\tools\check.ps1` all OK (2897 passed, 108 ignored; no clippy warning). An independent review of the diff (one agent): no bug; two nits accepted as they are (the comment does not name the black level counter; the CFA counter's reset assumes `fcols` a multiple of the cell, which `pwlr_layout_of` gives today: in `known-limits.md`, LibRaw). Leftovers: `known-limits.md` (LibRaw); the timings in ADR 0027 §4.6 |
