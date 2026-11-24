# T312. ADR 0027 W2: the timing of a fine view on real files — Done

- **Why:** ADR 0027 §3.1 and W2-Q1 (answered by the owner 2026-11-24, ADR 0027 §4.2): before the
  view core gets fine details (T313), the time of what a fine view costs is measured on the
  owner's own files. A fine view is the whole image rendered at full resolution with PPG and
  reduced to the fit size with the Lanczos filter, instead of the half-size render reduced (W1).
  ADR 0008 §11.2 measured the render alone at about 1012 ms for a 61 Mpix A7R image on all
  threads; this task measures it again with today's code, together with the reduction and the
  conversion to the display. The first task of the chain **T312** → T313 → T314. The
  measurement adds no behaviour; nothing in the program changes.
- **What the reviewer checked (2026-11-24):**
  - `crates/pw-viewcore/tests/native_files.rs` holds the tests on real files. They are
    `#[ignore]`, read the folder from `PW_TESTDATA` (`testdata()`), and find files with
    `test_files(prefix)` (ARW, RW2 and DNG whose names start with `prefix`).
  - `pw_viewcore::native::NativeBackend` implements `pw_viewcore::Backend`: `open(path,
    Source::Raw)` gives the session and the `ImageSize`; `render_half(&mut session)` is the
    half-size render (2×2 binning); `render_region(&mut session, rect)` the region at full
    resolution with PPG; `to_display(&session, img)` the conversion to the monitor profile.
  - `pw_viewcore::geometry::{whole, fit_scale}` and `pw_viewcore::scale::{scaled_size, reduce}`
    are public; `scale::reduce` is the Lanczos reduction of W1 (threaded, T311).
  - The owner's folder `D:\PhotoWorkflowData\RAW` holds Sony ARW files (names `TN…`) and
    Panasonic RW2 files (names `P124…`, 16 Mpix). Which ARW files are 61 Mpix (A7R IVA) and which
    50 Mpix (A1) is not known from the names, so the test measures one file of every distinct
    pixel size it finds.
- **Crate:** `pw-viewcore` (tests only). **Files:** `crates/pw-viewcore/tests/native_files.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. **The timing test** in `tests/native_files.rs`:
     `#[ignore = "a time measurement on real files; run with --release (ADR 0027 W2-Q1)"]`
     `fn timing_of_a_fine_view()`. It takes the files of `test_files("TN")` and `test_files("P")`
     together (if one prefix finds nothing, the test stops with the helper's message as the other
     tests do), opens each with `NativeBackend::default()` as `Source::Raw`, and keeps the first
     file of every distinct **upright** size (`w`×`h` turned by the size's `flip`, as
     `fit_scale` sees it; so a portrait and a landscape image of one camera are both measured,
     their fit scales differ). Opening every file to find the sizes takes some tens of seconds;
     that is expected. For each kept file it measures **three runs** and prints the **median** of
     each stage in ms (the existing helpers `median_and_max` and `ms` may be used), with the file
     name, the sensor `w×h`, the upright size, the Mpix and `s`:
     - (a) `render_region` of `geometry::whole(size)`;
     - (b) `scale::reduce` of (a)'s image to `scale::scaled_size(size.w, size.h, s)`, where
       `s = geometry::fit_scale(size, [3840, 2160])` (the owner's 4K screen, as T307's timing);
     - (c) `to_display` of (b)'s result;
     - (d) for comparison, today's reduced view: `render_half`, then `scale::reduce` of it to the
       same size as (b), then `to_display`, as one time. **Only when `s` is below 0.5**: at 0.5
       or more the view core never uses the half-size render (it renders `Scaled`), and
       `scale::reduce` refuses an output larger than its input; there (d) prints "n/a (fit
       scale ≥ 0.5)" and is skipped. (Likely for a landscape 16 Mpix RW2 at 4K, s ≈ 0.63.)
     Each run opens the file again (a new session), so LibRaw's state is the same every time; the
     open is not timed. A stage that returns an error, or a `scale::reduce` that returns `None`
     where it is run, fails the test with the file's name and the stage. The test asserts only
     that (b)'s image has the size it asked for.
  2. **The measurement:** with the owner's folder, run
     `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace --release -- --ignored --nocapture timing_of_a_fine_view`
     (a debug build would measure the wrong thing). **The `--release` run is approved by the
     owner for this one measurement (W2-Q1, 2026-11-24)**; it builds in the project's
     `target\release`; if `rawstride.exe` is running and locks it, ask the owner to close it. Put
     every printed line in the log row, and name the CPU (`Get-CimInstance Win32_Processor`).
- **Tests to add:** the `#[ignore]` timing test of point 1 (it is not run by `.\tools\check.ps1`,
  which has no `PW_TESTDATA`).
- **Not in this task:** any change to the view core (T313); deciding whether the time is
  acceptable (the reviewer compares it with ADR 0008 §11.2, and the owner decides if it is far
  above, W2-Q1).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12. No new dependency. The test may use
  `.unwrap()`/`expect` as test code does; the existing helpers' style (`unwrap_or_else` with a
  message naming the file) is preferred.
- **Done when:** `.\tools\check.ps1` is all OK, the timing of point 2 is in the log, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W2-Q1); Open after its cold read and the chain review |
| 2026-11-24 | Blocked | Cold read: (d) cannot reduce the half-size render when the fit scale is 0.5 or more (a landscape RW2 at 4K): skipped there with "n/a"; files kept per upright size, since `fit_scale` depends on the flip; `reduce` gives `None`, not an error; the helpers `median_and_max` and `ms` named; opening every file takes tens of seconds. The API names, `test_files("P")` and the `native` feature under `--workspace` checked as written |
| 2026-11-24 | Open | Chain review (T313's log): nothing for T312; its time also bounds the waits T313 accepts (Z, E, J, a metadata write's release). Set Open |
| 2026-11-24 | Ready for review | `timing_of_a_fine_view` added to `tests/native_files.rs`; `docs/test-inventory.md` updated (row and run line). Measured with `cargo test --workspace --release -- --ignored --nocapture timing_of_a_fine_view`, `PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`, CPU Intel(R) Core(TM) i7-9700K CPU @ 3.60GHz (8 cores, 8 logical processors); `NativeBackend::default()`, so `to_display` converts to sRGB (no conversion, hence under 1 ms). `test_files("TN")` also finds the TN DNG files (24.2 Mpix), so they are measured too. Printed lines: `TN002512.DNG: sensor 6020x4024, upright 6020x4024, 24.2 Mpix, s 0.5368 -> 3232x2160; median of 3: (a) render_region 435.9 ms, (b) reduce 70.3 ms, (c) to_display 0.8 ms, fine view (a+b+c) 507.1 ms; (d) half-size view n/a (fit scale ≥ 0.5)` / `TN002513.DNG: sensor 6020x4024, upright 4024x6020, 24.2 Mpix, s 0.3588 -> 2160x1444; median of 3: (a) render_region 432.2 ms, (b) reduce 46.3 ms, (c) to_display 0.4 ms, fine view (a+b+c) 478.9 ms; (d) half-size view 134.3 ms` / `TN005701.ARW: sensor 8660x5784, upright 8660x5784, 50.1 Mpix, s 0.3734 -> 3235x2160; median of 3: (a) render_region 891.2 ms, (b) reduce 95.1 ms, (c) to_display 0.8 ms, fine view (a+b+c) 987.1 ms; (d) half-size view 273.9 ms` / `TN100423.ARW: sensor 9568x6376, upright 6376x9568, 61.0 Mpix, s 0.2258 -> 2160x1440; median of 3: (a) render_region 1155.5 ms, (b) reduce 84.3 ms, (c) to_display 0.4 ms, fine view (a+b+c) 1240.2 ms; (d) half-size view 299.3 ms` / `P1240231.RW2: sensor 4608x3464, upright 4608x3464, 16.0 Mpix, s 0.6236 -> 2874x2160; median of 3: (a) render_region 363.7 ms, (b) reduce 57.7 ms, (c) to_display 0.7 ms, fine view (a+b+c) 422.1 ms; (d) half-size view n/a (fit scale ≥ 0.5)` / `P1240235.RW2: sensor 4608x3464, upright 3464x4608, 16.0 Mpix, s 0.4688 -> 2160x1624; median of 3: (a) render_region 361.5 ms, (b) reduce 40.7 ms, (c) to_display 0.5 ms, fine view (a+b+c) 402.6 ms; (d) half-size view 138.6 ms` (the folder paths `D:\PhotoWorkflowData\RAW\` left out). `.\tools\check.ps1`: all OK (2607 passed, 80 ignored, 0 failed) |
| 2026-11-24 | Done | Reviewed: the test as point 1 asks, `test-inventory.md` counts right (row 10 tests, 9 ignored; 80 ignored in all), check.ps1 all OK with no clippy warning. The 61 Mpix render 1156 ms is about 14 % above ADR 0008 §11.2, the fine view 1240 ms; the owner accepted the time 2026-11-24 (W2-Q1, written in ADR 0027 §4.2). T313 set Open |
