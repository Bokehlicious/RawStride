# T182. RAW statistics on X-Trans: channel shares, renders and a file cut short (pw-libraw tests) — Open

- **Why:** from T142 and the first backlog sort (owner, 2026-11-17). T146 added
  `fujifilm_files_have_statistics_and_a_clipping_map_that_agree` in
  `crates/pw-libraw/tests/statistics.rs` over the five Fujifilm X-T30 files (`.RAF`, X-Trans,
  6246 × 4170) in `PW_TESTDATA`: bins and pixel counts add up, whole 3×3 cells, the clipping map,
  `_DSF4271.RAF` clipped in R and G. What the Bayer files have and the X-Trans files do not:
  - the channels' exact shares (a Bayer test checks G = R + B; an X-Trans 6×6 pattern has 8 R,
    20 G and 8 B photosites, so R = B = 2/9 and G = 5/9 of an image whose width and height are
    divisible by 6, as 6246 × 4170 are);
  - a clipped photosite counted in the top bin (`clipped[c] <= histogram[c][255]`);
  - the same statistics before and after renders (`the_statistics_are_the_same_before_and_after_renders`
    takes Bayer files only);
  - a file cut short giving an error (`a_file_cut_short_gives_an_error_and_no_statistics` takes an
    ARW and an RW2).

  A black level per position and three- or four-colour RAW data still have no sample files; they
  stay in `docs/test-inventory.md`'s "Not covered" line.
- **Crates:** `pw-libraw` (tests only). **Files:** `crates/pw-libraw/tests/statistics.rs`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:** three new tests in `tests/statistics.rs`, each `#[ignore = "needs real camera files:
  set PW_TESTDATA and run with --ignored"]` as the others; the existing tests and helpers are not
  changed (new helpers are fine). The module comment gets a bullet for the three, and its run
  line (line 3, `cargo test -p pw-libraw -- --ignored`, against `AGENTS.md` §5) becomes
  `cargo test --workspace -- --ignored`. Each test prints with `eprintln!`, as the others do:
  per file its name, size, `pixels`, and per channel `clipped` against `histogram[c][255]`; for
  the cut files the error text.
- **Tests to add:**
  1. `fujifilm_channels_have_the_x_trans_shares`: for every file of `fujifilm_files()` whose
     width and height (`Session::width()`, `height()`) are both divisible by 6 (at least one file
     must be; none is a failure that names the sizes): `pixels[0] == pixels[2] == all * 2 / 9`
     and `pixels[1] == all * 5 / 9`, with `all = width × height` (check `all % 9 == 0` first); and
     for each channel `clipped[c] <= histogram[c][255]`, as `check` asserts for Bayer files.
     (This holds when the file's black level is the same at every position of the pattern
     (`pwlr.cpp`, the `rest` of the black-level pattern); a RAF with a 6×6 black pattern of
     different values could put a clipped photosite below the top bin. If it fails, stop and
     report: it is a finding, not a test to loosen.)
  2. `fujifilm_statistics_are_the_same_before_and_after_renders`: `_DSF4271.RAF` (with clipping)
     and `_DSF4243.RAF` (without), each as the Bayer test does: the statistics, then
     `render_half`, the same statistics; then `render_region_with(Rect::new(64, 64, 256, 128), 16,
     Demosaic::Ppg)`, the same; and a second session that renders first gives the same statistics
     as the first session's.
  3. `a_fujifilm_file_cut_short_gives_an_error_and_no_statistics`: `_DSF4243.RAF` cut to 40 %
     and to 90 % of its length in a `scratch("cut-raf")` folder, as the ARW and RW2 test does:
     `Session::open` then `statistics` gives an `Err` whose text contains `DAMAGED`. The folder
     is removed at the end.

  If a test fails on the real files (a share that is not exact, a region render that X-Trans
  refuses, a cut RAF that opens, or a cut RAF whose `Err` lacks `DAMAGED`, for example "open: …"
  because the cut took the RAF's directory; damage is found only in `unpack`, and
  `PWLR_SHORT_TOLERANCE = 0` was measured on Bayer and DNG files only), do not change the test or the shim to make it pass: stop and
  report the output, since it is a finding about `pwlr.cpp` or LibRaw for a task of its own.
  Run them with `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored fujifilm`
  (never `-p`, `AGENTS.md` §5; other crates' ignored tests whose names hold `fujifilm` may run
  too) and report what the three printed; `.\tools\check.ps1` runs them only as ignored.
- **`docs/test-inventory.md`:** the `pw-libraw` `tests/statistics.rs` row: the three tests in its
  description, its counts 9 → 12 and 8 → 11 ignored (as `PW_TESTDATA (11 ignored)`), and the
  workspace totals as the file keeps them.
- **Not in this task:** `pwlr.cpp`, `lib.rs` and the other test files (`damaged.rs` keeps its
  Bayer and DNG files; a RAF there is not asked for). The clipping map's edge cells (the test
  files' sizes are divisible by 3).
- **Rules:** `docs/tasks/README.md` rules 1–12; `docs/TESTING.md` for `PW_TESTDATA`. Tests may use
  `unwrap` and `panic!` as the file's other tests do.
- **Done when:** `.\tools\check.ps1` is all OK, the three tests pass with `PW_TESTDATA` set, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added from the first backlog sort (owner, 2026-11-17): the reviewer found the X-Trans files already tested by T146; the owner chose a small task for what T146 left (the shares, the top bin, renders, a file cut short). Waits for T180 (queue order) and for its cold read |
| 2026-11-17 | Blocked | Cold read: doable with the named files and the public API; the 2/9 and 5/9 shares checked against `pwlr.cpp` (three colours only, visible area, whole 6×6 tiles); a region render with PPG is accepted (LibRaw uses its own X-Trans interpolation). Changed: the top-bin check's condition (an equal black level per position) and an `Err` without `DAMAGED` added to the stop list; what each test prints; the module comment's run line without `-p`; the `docs/test-inventory.md` change named. Still waits for T180 (queue order) |
| 2026-11-17 | Open | T180 is Done: set Open (tests only in `pw-libraw`; T180 changed nothing it uses) |
