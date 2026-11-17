# T176. ADR 0017 U9a: the focus map's time, and whether it slows stepping and Z — Open

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
