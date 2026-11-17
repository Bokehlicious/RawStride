# T176. ADR 0017 U9a: the focus map's time, and whether it slows stepping and Z — Done

- **Why:** the review of T167 asked for a measurement before the owner's hand check of U9a. T167's
  focus map costs an extra half-size render while the image's session is lent (in RAW mode a
  LibRaw render). The question is whether stepping or Z gets slower when the focus marking is on.
  T168 was to report this with `latency.ps1`, but that script is not in the repository, and the
  program neither shows nor logs `Stats::focus_ms` (review of T168, 2026-11-17). This task measures
  it in the view core on the owner's RAW files, without a window, as
  `a_camera_raw_gets_its_statistics_in_both_modes` already does for the statistics.
- **Crates:** `pw-viewcore` (tests only). **Files:** `crates/pw-viewcore/tests/native_files.rs`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **One new ignored test** in `native_files.rs`:
     `focus_map_time_and_stepping_and_zoom_with_and_without_the_marking`, with the same
     `#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]` as the others,
     on `test_files("TN")` (the TN RAW files in `PW_TESTDATA`), in RAW mode. Production code is not
     changed.
  2. **Two runs, each on a fresh core** (`start()`, `open_folder(files)`, `Screen`, `until_ready`):
     first with the focus marking off, then with `set_focus_wanted(true)` called before
     `open_folder`. Each run goes through every file once:
     - **Stepping:** `step(1)` as soon as the image before is `Ready`, without waiting for its
       statistics or focus map, so in the second run the map is still being made when the user
       steps. The time from `step(1)` to the first frame whose `pos` is the new position and
       whose `status` is `Ready`.
     - **Z:** after the stepping pass, go back to the first file (`go(0)`, `until_ready`). For each
       file, wait until its statistics are there (`until_statistics`) and, in the second run, until
       its `focus` is no longer `Reading` (a new helper `until_focus`, like `until_clipping`). Then
       time `toggle_zoom(None)` to `Ready`, `fit()`, `until_ready`, and `step(1)` to the next file.
     - A frame with `Status::Error` or a wait past `WAIT` fails the test and names the file.
  3. **The results are printed** with `eprintln!`, one line each:
     - per run, the number of files and the median and the largest time of stepping and of Z, in
       ms;
     - for the second run, `stats().focus_maps` and the median and largest of `stats().focus_ms`.

     The test asserts no times. It asserts only what must hold: in the second run every file's
     `focus` ended `Ready` (a TN RAW file always has a map), and `focus_maps` is at least the
     number of files; in the first run `focus_maps` is 0.
  4. **Run it** with `PW_TESTDATA` set (on the owner's machine `D:\PhotoWorkflowData\RAW`):
     `cargo test --workspace -- --ignored --nocapture focus_map_time` (this is
     `cargo test --workspace`, allowed by `AGENTS.md` §5; not `-p pw-viewcore`). Run it **three
     times** and report all three runs. Then run `.\tools\check.ps1` as usual.
  5. **Report in this file's log** (`docs/TESTING.md` §4.6): the hardware (CPU, cores, RAM, as
     `Get-CimInstance Win32_Processor` and `Win32_ComputerSystem` give them), the Windows version,
     the build (the test profile `cargo test` used, as its output says), the data set (the folder,
     how many files, which extensions), the method (points 2–4), and the printed lines of the
     three runs. Say in one sentence whether stepping or Z was slower with the marking on, by
     the medians. Do not judge whether it is acceptable; the owner does.
  6. **`docs/test-inventory.md`:** the `native_files.rs` row of `pw-viewcore` names the new test;
     the counts of ignored tests are updated.
- **Tests to add:** the one test of point 1. It is a measurement, so it has no asserts on times.
- **Not in this task:** any change to `pw-viewcore`'s code or to `FOCUS_THRESHOLD`; a latency
  script; a way to show `Stats` in the program; JPEG mode and the compare (the owner's hand check
  covers them).
- **Rules:** AGENTS.md §3 and §5; `docs/TESTING.md` §4.4 and §4.6. If a TN file cannot be opened,
  or `PW_TESTDATA` is not set, stop and say so instead of changing the file list.
- **Done when:** the three runs are reported in the log, `.\tools\check.ps1` is all OK and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added from the review of T168: its point "Time of the map, before the hand check" could not be done (`latency.ps1` is missing, `Stats::focus_ms` is not shown) |
| 2026-11-17 | Open | No cold read (owner, 2026-11-17: not needed for this small test-only task) |
| 2026-11-17 | Ready for review | Measured (`docs/TESTING.md` §4.6). **Hardware:** Intel Core i7-9700K @ 3.60 GHz, 8 cores / 8 logical processors; 32 GB RAM. **OS:** Windows 11 Home 10.0.26300. **Build:** `cargo test` `test` profile `[optimized + debuginfo]`. **Data set:** `PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`, `test_files("TN")`: 24 files, 20 `.ARW` and 4 `.DNG`, RAW mode, view 1920 × 1080. **Method:** points 2–4; the new ignored test `focus_map_time_and_stepping_and_zoom_with_and_without_the_marking` in `native_files.rs`, run three times with `cargo test --workspace -- --ignored --nocapture focus_map_time`; a step is timed from `step(1)` to the first frame with the new `pos` and `Ready`, a Z from `toggle_zoom(None)` to the first frame not at fit and `Ready`. **Run 1:** `marking off: 24 files; stepping median 452.7 ms, largest 664.8 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking on: 24 files; stepping median 445.3 ms, largest 657.4 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking on: focus_maps 24; focus_ms median 267.8 ms, largest 338.8 ms`. **Run 2:** `marking off: 24 files; stepping median 448.6 ms, largest 650.8 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking on: 24 files; stepping median 451.6 ms, largest 656.6 ms (23 steps); Z median 0.0 ms, largest 0.3 ms (24 Z)` / `marking on: focus_maps 24; focus_ms median 261.8 ms, largest 332.3 ms`. **Run 3:** `marking off: 24 files; stepping median 447.0 ms, largest 651.5 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking on: 24 files; stepping median 452.9 ms, largest 649.8 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking on: focus_maps 24; focus_ms median 261.4 ms, largest 323.4 ms`. **Result:** by the medians, stepping was not clearly slower with the marking on (−7.4, +3.0 and +5.9 ms in the three runs, within about 1.5 %), and Z was not slower (0.0 ms in both). Note for the owner: Z measures 0.0 ms because the core plans the current image's 100 % view right after its fit view (`State::wanted`), so once its statistics (and focus map) are there, that view has already been rendered and Z is a hit; Z right after a step is not what this method measures. No production code changed; `.\tools\check.ps1` all OK |
| 2026-11-17 | Open | Review: the test, the runs and the report are as the task asked; `.\tools\check.ps1` all OK. Sent back because the method (the reviewer's) misses the case in question: the core starts the focus map only after the image has been shown for `Config::dwell` (150 ms), so stepping as soon as an image is shown makes no map, and Z after the map is ready is a hit. **To add, in the same test and file, nothing else changed:** two more passes in each run (off and on), after the two there are: **(a) stepping at 200 ms:** `go(0)`, shown; for each file but the last, once it is shown keep calling `screen.frame()` for 200 ms (measured from the first `Ready` frame of that image), then time `step(1)` to the next image shown; **(b) Z at 200 ms:** `go(0)`, shown; for each file, the same 200 ms, then time `toggle_zoom(None)` to `Ready` not at fit, then `fit()` and `until_ready`, and `step(1)` to the next file (not timed). In both passes, count the steps or Z where the last frame before them had `focus` `Reading` (in the run with the marking on this shows that the map was being made), and print per pass the median, the largest and that count, in the same form as the other lines. The existing passes and asserts stay as they are. Run three times again and report the new lines of all three runs (the hardware, OS, build and data set may refer to the row above if unchanged), and one sentence per pass on whether it was slower with the marking on, by the medians |
| 2026-11-17 | Ready for review | The two passes at 200 ms added to the same test as the review asked ((a) stepping and (b) Z, each image shown for 200 ms from its first `Ready` frame, counting those whose last frame before had `focus` `Reading`); the existing passes and asserts are unchanged. Hardware, OS, build and data set as in the Ready for review row above. Three runs with `cargo test --workspace -- --ignored --nocapture focus_map_time`. **Run 1:** `marking off: 24 files; stepping median 453.1 ms, largest 653.0 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking off: stepping at 200 ms median 116.6 ms, largest 258.2 ms (23 steps, 0 with focus Reading)` / `marking off: Z at 200 ms median 0.0 ms, largest 20.2 ms (24 Z, 0 with focus Reading)` / `marking on: 24 files; stepping median 451.5 ms, largest 659.9 ms (23 steps); Z median 0.0 ms, largest 31.9 ms (24 Z)` / `marking on: stepping at 200 ms median 120.0 ms, largest 236.3 ms (23 steps, 20 with focus Reading)` / `marking on: Z at 200 ms median 0.0 ms, largest 0.0 ms (24 Z, 21 with focus Reading)` / `marking on: focus_maps 25; focus_ms median 268.2 ms, largest 340.5 ms`. **Run 2:** `marking off: 24 files; stepping median 449.0 ms, largest 654.9 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking off: stepping at 200 ms median 122.3 ms, largest 195.1 ms (23 steps, 0 with focus Reading)` / `marking off: Z at 200 ms median 0.0 ms, largest 43.8 ms (24 Z, 0 with focus Reading)` / `marking on: 24 files; stepping median 450.0 ms, largest 654.8 ms (23 steps); Z median 0.0 ms, largest 11.7 ms (24 Z)` / `marking on: stepping at 200 ms median 109.2 ms, largest 241.7 ms (23 steps, 20 with focus Reading)` / `marking on: Z at 200 ms median 0.0 ms, largest 33.7 ms (24 Z, 23 with focus Reading)` / `marking on: focus_maps 31; focus_ms median 265.4 ms, largest 339.5 ms`. **Run 3:** `marking off: 24 files; stepping median 448.9 ms, largest 695.5 ms (23 steps); Z median 0.0 ms, largest 0.0 ms (24 Z)` / `marking off: stepping at 200 ms median 131.9 ms, largest 201.0 ms (23 steps, 0 with focus Reading)` / `marking off: Z at 200 ms median 0.0 ms, largest 0.0 ms (24 Z, 0 with focus Reading)` / `marking on: 24 files; stepping median 453.1 ms, largest 660.2 ms (23 steps); Z median 0.0 ms, largest 13.9 ms (24 Z)` / `marking on: stepping at 200 ms median 111.2 ms, largest 238.8 ms (23 steps, 22 with focus Reading)` / `marking on: Z at 200 ms median 0.0 ms, largest 0.0 ms (24 Z, 20 with focus Reading)` / `marking on: focus_maps 26; focus_ms median 263.7 ms, largest 332.7 ms`. **(a) Stepping at 200 ms:** by the medians not slower with the marking on (on − off: +3.4, −13.1 and −20.7 ms), with the map being made before 20–22 of the 23 steps. **(b) Z at 200 ms:** by the medians not slower with the marking on (0.0 ms in all runs, off and on), with the map being made before 20–23 of the 24 Z. The two earlier passes again show no difference by the medians. `focus_maps` is now 25–31 for 24 files: the extra passes visit each file again and only four maps are kept (`KEPT_FOCUS_MAPS`). `docs/test-inventory.md`: the row's description names the passes at 200 ms. `.\tools\check.ps1` all OK |
| 2026-11-17 | Done | Second review: the passes at 200 ms measure the case in question (the map was being made before 20–23 of each 23–24 steps or Z). Result: the focus map takes a median of about 265 ms on the TN RAW files, and neither stepping nor Z was slower with the marking on, by the medians of three runs. Approved by the owner and committed. Leftovers: none; the hand check of U9a is in `plans/` row U9a. No owner decision to record in an ADR |
