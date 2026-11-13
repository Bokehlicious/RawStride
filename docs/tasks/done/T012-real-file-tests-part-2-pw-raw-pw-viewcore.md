# T12. Real-file tests, part 2 (pw-raw, pw-viewcore) — Done

- **Start after T6 is done.** Use exactly the pattern from T6.
- **Watch for:** with `PW_TESTDATA` set, `real_files.rs` finished its 4 tests in 0.00 s, so they
  probably still skip because their files are not found in that folder. Expect the `--ignored` run
  to show which file is missing, and report it.
- **Files:** `crates/pw-raw/tests/real_files.rs` (4 tests), `crates/pw-viewcore/tests/native_files.rs`
  (3 tests).
- **Report back** the same three runs as in T6, for `-p pw-raw -p pw-viewcore`.
- **Done when:** all 7 tests are ignored by default, fail with a clear message without the files and
  pass with them; clippy is clean.
