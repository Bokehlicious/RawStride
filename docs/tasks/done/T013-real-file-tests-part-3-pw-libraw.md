# T13. Real-file tests, part 3 (pw-libraw) — Done

- **Start after T6 is done.** Use exactly the pattern from T6.
- **Files:** `crates/pw-libraw/tests/damaged.rs` (`complete_files_are_never_called_damaged`,
  `truncated_files_are_damaged`), `crates/pw-libraw/tests/regions.rs` (the tests that call
  `check_regions_identical`, `check_ppg_regions_identical` or `check_methods_switch_cleanly`).
  `unreadable_files_fail_but_are_not_damaged`, `not_a_raw_file_is_an_error` and
  `version_is_reported` do not need the files and stay as they are.
- **Already fixed by the reviewer (2026-11-08):** `scratch()` now takes the test name, so each test
  has its own temporary folder (two tests shared one, and one removed it while the other was still
  writing). Keep that.
- **Note:** some of these tests look for a file by name prefix (e.g. a Panasonic file). A missing
  prefix must now fail with a message, not print "skipped".
- **Report back** the same three runs as in T6, for `-p pw-libraw`.
- **Done when:** as in T12.
