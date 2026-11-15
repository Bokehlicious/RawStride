# T142. ADR 0017 U7, first part: the RAW histogram and exposure statistics — Done

- **Can start now.** The owner approved the proposals of U7-Q1–U7-Q8 (at the end of this task)
  on 2026-11-15; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T143** (the rest of the structures in the metadata window). Both change
  `crates/pw-loupe/src/main.rs`, in different functions: this task the preview's right column and
  the inspector (`draw_preview_column`, `draw_inspector` and what they call); T143 only the metadata
  window. Both add rows to `docs/glossary.md`: add rows, never rewrite another's. **Run
  `.\tools\check.ps1` only when the other task's agent is not running it.**
- **Crates:** `pw-libraw`, `pw-viewcore`, `pw-loupe`. **Files:** `crates/pw-libraw/csrc/pwlr.cpp`
  and `crates/pw-libraw/src/lib.rs` (the statistics of the unpacked RAW data, point 2), a test in
  `crates/pw-libraw/tests/`; `crates/pw-viewcore/src/backend.rs`, `native.rs`, `state.rs` and
  `core.rs` (the statistics asked for once per opened image and given in the `Frame`), its tests;
  `crates/pw-loupe/src/histogram.rs` (new: what the panel draws, from the statistics, as plain
  Rust, with its tests), the text module the preview or the inspector uses (rule 13),
  `crates/pw-loupe/src/settings.rs` (the sections' remembered states), `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/main.rs` (only the parts named above); `docs/glossary.md`;
  `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.6 and step U7: judging the exposure of a RAW file from the RAW data, not
  from the camera's JPEG or the 8-bit image on the screen. Today the program shows no histogram.
- **What:**
  1. **Two parts** (U7-Q1): this task the histogram and the statistics; the clipping overlay on the
     image (§2.6) is the next task, built on the same statistics' clipping level.
  2. **RAW statistics, in the C++ shim** (§2.6, U7-Q3, U7-Q4): a new `pwlr` function and its
     `Session` method compute, from the RAW data `pwlr_open` has already unpacked (no new decode,
     and before a render changes the image buffer), per colour channel (R, G, B; the two greens of a
     Bayer sensor as one G):
     - a histogram of 256 bins of the values after the black level is subtracted and the camera's
       white balance applied, scaled so the white level is the top bin, with an sRGB-like curve on
       the value so the shadows are not pressed into the first bins (U7-Q3);
     - the share of the channel's pixels **at the clipping level**: a raw value at or above the
       white level LibRaw reports (`maximum`, or `linear_max` when it is set), before white
       balance (U7-Q4);
     - the share of all pixels in the **deepest shadows**: every channel below 1/256 of the white
       level above black, eight stops under clipping (U7-Q4).
     A file whose RAW data is not a Bayer or a three- or four-colour image the shim can read
     returns no statistics, and the panel says so (point 5).
  3. **JPEG and TIFF** (§2.6, U7-Q5): in JPEG mode the same numbers from the embedded JPEG as
     decoded, **before** the conversion to the screen profile (ADR 0016): clipping at 255, deepest
     shadows at 0 in every channel; a TIFF from its own decoded image, the same way.
  4. **The view core** (U7-Q6): `Backend` gets a method that gives an opened image's statistics,
     asked once per opened image after its first image is shown, so the first image is not delayed
     (V1: `latency.ps1` before and after, no worse); `Frame` gets the statistics of the current
     image when they are ready (`None` until then and for another image).
  5. **The panel** (U7-Q2, U7-Q7): a section "Histogram" in the preview's right column, under Info
     and Zoom, and in the inspector, under Metadata; each collapsible and remembered
     (`preview_histogram`, `inspector_histogram` in `settings.txt`). It draws the three channels
     over each other in red, green and blue, partly transparent (where they overlap the colours
     mix); the height scaled to the highest bin, the two end bins left out of the scale so a
     clipped image does not flatten the rest. Under it: "RAW data" / "Embedded JPEG" / "TIFF" (where
     the numbers come from, G1), "Clipped: R {r} %  G {g} %  B {b} %" and "Deep shadows: {s} %",
     each with two decimals; "Reading …" until the statistics come; "No histogram for this file"
     when there are none.
  6. **Names** (U7-Q8): Histogram, RAW data, Embedded JPEG, TIFF, Clipped, Deep shadows, Reading …,
     No histogram for this file; English (GR14), in the text module and the glossary.
- **Tests to add:** `pw-libraw`: the statistics of real RAW files (`PW_TESTDATA`; ignored without
  it, as the other fixture tests): 256 bins per channel that add up to the pixel count, shares
  between 0 and 100 %, a file of a known overexposed and a known dark scene if the test folder
  has them (the agent names the files it used), a damaged file gives an error, not a panic;
  `pw-viewcore`: the statistics asked once per opened image, given in the `Frame` of that image
  only, JPEG statistics taken before the profile conversion (a test backend that records the
  order); `histogram.rs`: the scaling without the end bins, the share texts, the source text; the
  settings saved and loaded; the texts through their functions.
- **Not in this task:** the clipping overlay (next task); a luminance histogram; the statistics
  of a region or the zoomed part; exposure warnings in the contact sheet; anything that changes a
  file.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency; the C++ shim's existing
  rules (no exception crosses into Rust; every LibRaw return code checked). Run
  `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` on a release build before and after, and
  report V1.
- **Done when:** `.\tools\check.ps1` is all OK, V1 is not worse, and `git diff --stat` shows only
  the files above (with T143's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U7-Q1 | One task or two? | Two: the histogram and statistics now, the clipping overlay next, on the same clipping level |
| U7-Q2 | Which histogram? | Three channels R, G, B drawn over each other, partly transparent; no luminance curve |
| U7-Q3 | How are the RAW values placed on the histogram? | After black subtraction and the camera's white balance, scaled to the white level, with an sRGB-like curve so the shadows spread out as in an ordinary histogram |
| U7-Q4 | What counts as clipped, and as deepest shadows? | Clipped: a raw value at or above the white level, before white balance, per channel. Deepest shadows: every channel below 1/256 of the white level (eight stops under clipping) |
| U7-Q5 | JPEG and TIFF? | The same numbers from the embedded JPEG (or the TIFF) before the screen-profile conversion; clipped at 255, shadows at 0; the panel names the source |
| U7-Q6 | When is it computed? | Once per opened image, after its first image is shown, so the first image is not slower; `latency.ps1` checked |
| U7-Q7 | Where is it shown? | A collapsible "Histogram" section in the preview's right column (under Info and Zoom) and in the inspector (under Metadata), each remembered; the clipped shares per channel and the deep-shadow share under it |
| U7-Q8 | Names | Histogram, RAW data, Embedded JPEG, TIFF, Clipped, Deep shadows, Reading …, No histogram for this file; English (GR14), in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added; waits for U7-Q1–U7-Q8 |
| 2026-11-15 | Open | The owner approved U7-Q1–U7-Q8 as proposed |
| 2026-11-15 | Ready for review | Done by the agent. **A new task is needed for the inspector:** it is drawn only in the contact sheet, where the view core has no image open, so its Histogram section shows statistics only when the last preview frame was of the same image and otherwise stays at "Reading …". The owner approved the changes outside the task's file list (the re-export in `pw-viewcore/src/lib.rs`, `statistics: None` in two test frames of `pw-loupe/src/loupe_text.rs`, the parts of `main.rs` outside the two named functions, two more lines in the settings test of every key). Deep shadows of a mosaic are counted in cells of 2×2 photosites (X-Trans 3×3); the bars are each channel's own shares; a plain JPEG file is labelled "Embedded JPEG"; a TIFF converted when opened is decoded a second time. `latency.ps1` on A1, median before → after: RAW fit 4.9 → 5.1 ms (max 7.3 → 9.1), JPEG fit 4.9 → 5.0 ms; the 100 % run registered only two steps both times. The section was not checked by eye in the running program. |
| 2026-11-15 | Done | Reviewed: `check.ps1` all OK (1495 passed, 0 failed, 58 ignored); the ignored `statistics` and `native_files` tests run on `PW_TESTDATA` (4 + 6 passed). The owner confirmed the changes outside the file list (the agent could not finish without them). Left for T144: the inspector's Histogram section, and the 100 % view's latency right after the statistics start. Not covered by a test file: X-Trans, a black level per position, three- or four-colour RAW data. The section was not checked by eye. |
