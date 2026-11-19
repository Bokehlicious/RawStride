# T197. The region render on RAF (ADR 0027 W0) — Done

- **Why:** T182 found (2026-11-17) that `Session::render_region_with` fails on every Fujifilm RAF
  in the test data (`_DSF4271.RAF`, `_DSF4243.RAF`; X-T30, X-Trans) with
  `process: Input/output error`, the error that `pwlr_render` gives when `dcraw_process` fails
  with a `cropbox` (`pwlr.cpp`). It fails with PPG and with AHD, in a fresh session, after
  `statistics` and after `render_half`. A whole-image render and `render_half` work. The loupe's
  Zoom, Ring and Scaled views all render a region (`pw-viewcore` `native.rs`, `render_region`
  with `MARGIN` 16 and `Demosaic::Ppg`), so zooming into a RAF most likely shows an error today.
  ADR 0027 needs a working region render on every format (step W0, a prerequisite; moved forward
  by the owner on 2026-11-18, order 1b in `plans/adr-0018-0025-steps.md`).
- **Crates:** `pw-libraw`. **Files:**
  - `crates/pw-libraw/csrc/pwlr.cpp` (`pwlr_render`, and a helper if needed)
  - `crates/pw-libraw/src/lib.rs`, only if the fix needs the Rust side (point 3): the `extern`
    declaration of `pwlr_render`, the private `Session::render` that every render goes through,
    `Session::render_region_with`, and their doc comments (also `render_region`'s "a margin of at
    least 8 px", which holds for Bayer files only)
  - `crates/pw-libraw/tests/regions.rs` (the new tests and the module comment)
  - `crates/pw-libraw/tests/statistics.rs` (`fujifilm_statistics_are_the_same_before_and_after_renders`
    and the module comment's line that region renders fail on every RAF)
  - `docs/test-inventory.md` (the `pw-libraw` rows: counts, and the text that the region render
    fails on every RAF)
- **What:**
  1. **Find the cause.** Write a temporary test (not kept) that opens `_DSF4243.RAF`, renders one
     region, for example `Rect::new(2048, 2048, 1024, 1024)` with margin 16 and PPG, and prints
     the error. Find out, with as few experiments as needed, what makes `dcraw_process` fail.
     Where LibRaw is (0.22.3, read only, never changed):
     - the headers: `C:\Development\PhotoWorkflow\vcpkg_installed\x64-windows-static-md\include\libraw`
       (`build.rs` uses the `vcpkg_installed` of the `main` worktree; it is not under this
       worktree's root);
     - the source: `C:\Development\vcpkg\buildtrees\libraw\src\0.22.3-3fc2d18973.clean`.

     Two things the source already shows (reviewer's cold read, 2026-11-18); confirm them:
     - `src/demosaic/xtrans_demosaic.cpp`, `xtrans_interpolate`: an image (here the cropped box)
       narrower or lower than `LIBRAW_AHD_TILE` (512) throws `LIBRAW_EXCEPTION_IO_CORRUPT`, which
       becomes `LIBRAW_IO_ERROR`, "Input/output error". So every box under 512×512 fails on
       X-Trans (T182's 288×160 box did).
     - `src/preprocessing/raw2image.cpp`: for X-Trans (`filters == LIBRAW_XTRANS`, 9) the crop's
       origin is rounded **down** to a multiple of 6 while its width and height are kept, so a box
       that does not start on a multiple of 6 comes back shifted by up to 5 px **without an
       error**, and the shim's size check does not see it.
     - X-Trans does not use PPG or AHD: `dcraw_process` calls `xtrans_interpolate` with 1 pass
       for `user_qual` ≤ 2 (PPG) and 3 passes above (AHD).

     Neither explains why a 1056×1056 box (the tile above with its margin) fails; find that out.
     Write down in the report which boxes fail, which work, and why.
  2. **Fix it in `pwlr_render`**, so a region render on a RAF works and gives exactly the pixels of
     the same area of a whole-image render with the same method (the guarantee of ADR 0006 and
     `regions.rs`). For X-Trans the box LibRaw is given must at least start on a multiple of 6
     (so nothing is shifted) and be at least 512×512, both clamped to the image, plus whatever
     point 1 finds for the larger boxes. Return that wider image, so the Rust side cuts the asked
     region out of it as it already does for the margin (point 3). A Bayer file
     (every other format the tests use) must render exactly as today: the change applies only
     where it is needed (for example only when `filters == 9`), and the existing `regions.rs`
     tests keep passing unchanged.
  3. **The Rust side, only if the shim returns a wider image than asked:** today
     `render_region_with` refuses a result whose size differs from the box it asked for ("LibRaw
     returned …"). If point 2 widens the box, the shim must also say where the returned image
     starts (for example two `int *` out-parameters with the box it really used); the `extern`
     declaration and `Session::render` pass them on, and `render_region_with` cuts `r` out of that
     box instead. Full and half renders are unchanged. Keep the refusal for any other size
     difference. Update the doc comments.
  4. **If identity is not reached** with a margin of 16 (X-Trans interpolation looks further than
     PPG on Bayer): find the smallest margin that gives identical pixels for both methods on both
     files, and **stop and report it** before changing `MARGIN` anywhere: `MARGIN` is in
     `pw-viewcore`, which is not in this task.
  5. **If the cause cannot be fixed in the shim** (for example LibRaw cannot crop X-Trans at all),
     stop and report what you found and what the options are (for example rendering the whole
     image once and cutting regions from it, with its time). Do not build such a fallback in this
     task.
- **Tests to add:**
  - `regions.rs`: `fujifilm_ppg_regions_are_identical_to_full` (prefix `_DSF4243`, through
    `check_ppg_regions_identical`), `fujifilm_regions_are_identical_to_full` (prefix `_DSF4271`,
    through `check_regions_identical`, AHD) and `fujifilm_methods_switch_cleanly` (prefix
    `_DSF4243`, through `check_methods_switch_cleanly`), each `#[ignore]` with the same reason as
    the others. If `check_methods_switch_cleanly`'s "PPG and AHD gave identical images" fails on
    X-Trans, stop and report it; do not remove the check.
  - `statistics.rs`, `fujifilm_statistics_are_the_same_before_and_after_renders`: put back the
    region render T182 left out, as its task first asked: after `render_half` and its check,
    `render_region_with(Rect::new(64, 64, 256, 128), 16, Demosaic::Ppg)` succeeds, gives 256×128
    pixels, and the statistics are the same as before (this box is under 512 px, so it also tests
    the widening of point 2); remove the comment that says the region render is left out.
  - Run them with `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"` and
    `cargo test --workspace -- --ignored fujifilm`, and also the whole of `regions.rs` with
    `--ignored` (the Bayer files must still pass); paste the results in the report.
- **Not in this task:** `pw-viewcore` (`MARGIN`, `native.rs`), the loupe, any fallback render,
  LibRaw's version or build, and any other format.
- **Rules:** the C++ change follows the shim's style (no exceptions out of the shim, every error
  through `set_error` or the `error` buffer, nothing printed). This task changes the C++ shim, so
  it gets an independent review of the diff before the owner is asked (README, "An extra code
  review").
- **Done when:** `.\tools\check.ps1` is all OK, the `--ignored` runs above pass with
  `PW_TESTDATA`, and `git diff --stat` shows only the files above and this task's file. Then the owner's hand check:
  open a RAF in the preview, zoom to 100 % and move around; the image is sharp and no error is
  shown.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Open | Added (ADR 0027 W0; the backlog item "A region render fails on every RAF") |
| 2026-11-18 | Open | Cold read: doable with the named files once `lib.rs`'s `extern` declaration and `Session::render` were named. Found in the LibRaw source and checked by the reviewer: X-Trans refuses a box under 512×512 with `IO_CORRUPT` ("Input/output error") and rounds the crop origin down to a multiple of 6 without an error; X-Trans uses 1 or 3 passes of its own interpolation, not PPG or AHD. Changed: the LibRaw header and source paths (the headers are in the `main` worktree's `vcpkg_installed`), the two LibRaw facts written into point 1 and the 512 and 6 rules into point 2, the files' lists (the module comments of both test files, `test-inventory.md`'s rows, `lib.rs`'s `extern` and `Session::render`), the statistics test's size check, and "Done when" names this task's file |
| 2026-11-19 | Ready for review | Cause (point 1): (a) `xtrans_interpolate` refuses a box under 512 px wide or high with `IO_CORRUPT`, and LibRaw's exception handler then calls `recycle()`, which frees the unpacked RAW data, so every later call on the session fails with `Out of order call of libraw function` and `statistics` gives `Ok(None)`. That is why T182's 1056×1056 box failed: it came after a failed small box; in a fresh session it renders. 511×600 fails, 512×512 works. (b) LibRaw rounds an X-Trans crop's origin down to a multiple of 6 without an error (confirmed in `raw2image.cpp`). (c) Not in the cold read: `xtrans_interpolate` works in 512 px tiles every 496 px, counted from 3 px into the box, and the pixels near a seam depend on where it is. With the origin only on a multiple of 6, about 1,000 bytes (PPG) or 9,000 bytes (AHD) per 1024 tile differed from the full render, whatever the margin (up to 32). With the origin on a multiple of 1488 (the LCM of 6 and 496), PPG was identical with a margin of 16 and AHD (3 passes) with 24. The differences at 16 lay at the box's far edge. Fix (point 2): `pwlr_render` processes an X-Trans region (`rawdata.iparams.filters == LIBRAW_XTRANS`) in the asked box plus 32 px on every side, its origin rounded down to a multiple of 1488, at least 512 px, clamped to the image. It refuses an image under 512 px before LibRaw is called, and Bayer files are unchanged. Point 3: four new out-parameters give the box it used; `Session::render` returns it, and `render_region_with` crops from it and refuses an image whose size is not that box, or a box that does not hold the asked one. Point 4 is not needed: identity holds with any margin, 0 included, so `MARGIN` stays. Checked by a temporary probe on both RAFs, both methods, 12 boxes (edges, 1×1, a full-width row, boxes on the 1488 grid). Cost (release, `_DSF4243.RAF`, PPG, margin 16): a 1024 tile at 2048 takes 0.26 s, at 1504 0.45 s (box from 0), 1920×1080 0.38 s, 3840×2160 1.0 s; the whole image takes 1.55 s. `regions.rs`'s `test_file` now also takes `.raf`. Tests: the three `fujifilm_*` tests in `regions.rs`, and the region render back in `fujifilm_statistics_are_the_same_before_and_after_renders`. `--ignored fujifilm`: 3 + 4 passed (+2 in another crate). `regions.rs --ignored`: 11 passed. `check.ps1` all OK. Left: a failed LibRaw call leaves a dead session, which other errors (Bayer included) can still cause, and `statistics` then gives `Ok(None)` without an error. The independent review of the diff (C++ shim) is still to do. |
| 2026-11-19 | Ready for review | Review: the cause and the fix checked against the LibRaw 0.22.3 source; `pwlr_render` widens only an X-Trans region, Bayer is processed in the asked box as before; `render_region_with` crops from the box the shim used and refuses any other size; no panic path added. Independent review of the diff (a fresh review agent, README "An extra code review"): no bug; `pwlr_xtrans_axis` holds the asked span, stays in the image, gives at least 512 px and an origin on a multiple of 1488 in every branch; every LibRaw claim of the shim's comment confirmed with source lines (the 512 minimum and `IO_CORRUPT` with `recycle()`, the tiles from 3 px every 496 px, 1 or 3 passes, the crop origin rounded to 6 in visible-image coordinates, `rawdata.sizes` and `rawdata.iparams.filters` saved at the unpack); no stale cropbox between renders. Its findings and what was done: (1) no kept test checked the pixels for the box widened to 512 px or moved in from the far border: the reviewer added `fujifilm_small_regions_are_identical_to_full` in `regions.rs` (`_DSF4243.RAF`, PPG and AHD, `Rect(1520, 1520, 64, 64)` and the 32 px corner region); (2) `test-inventory.md`'s total still said 72 ignored: now 76, after T87–T197, and the regions row 14 / 12; (3) `regions.rs`'s module comment said LibRaw "cannot process" an origin off a multiple of 6: it moves it silently, reworded; (4) leftovers (a failed LibRaw call kills the session and `statistics` then gives `Ok(None)`, older panic paths in `lib.rs`, the cost of an X-Trans region) in `backlog.md`, "Leftovers from the review of T197". The reviewer's runs: `.\tools\check.ps1` all OK (1931 passed, 76 ignored, 0 failed, the sum of every `test result:`; no clippy warnings); with `PW_TESTDATA` `--ignored fujifilm` 3 + 4 (+2 in another crate) passed before the added test, and the whole of `regions.rs` with `--ignored` 12 passed after it |
| 2026-11-19 | Done | Approved by the owner. The leftovers are in `backlog.md` ("Leftovers from the review of T197"). The owner's hand check (a RAF in the preview at 100 %, moved around: sharp, no error) follows; the plan's row 1b is marked done after it |
