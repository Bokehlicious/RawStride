# T19. pw-viewcore: run the "not a RAW file" test in every test run — Done

- **File:** `crates/pw-viewcore/tests/native_files.rs`, test `a_file_that_is_not_raw_shows_an_error`.
- **Why:** the test writes its own broken file and does not use the real camera files, but it is
  ignored and calls `testdata()` only because it used to check `PW_TESTDATA`.
- **What:** remove its `#[ignore = …]` line and the `let _ = testdata();` line. Nothing else.
- **Done when:** `cargo test -p pw-viewcore` runs it (1 more passed, 1 fewer ignored) and clippy is
  clean.
