# T6. Real-file tests, part 1 (pw-metadata, pw-ingest, pw-libjpeg): never pass silently — Done

- **Owner decision (2026-11-08):** a test that needs the real camera files is **ignored by default**
  and **fails when it is run explicitly without them**. So `cargo test` stays fast and green without
  the files (TESTING.md §5 "Fast Regression") and shows such tests as "ignored", not "passed"
  (§4.4, §6); `cargo test -- --ignored` is the "Real RAW Verification" run.
- **This task sets the pattern** that T12–T14 repeat for the other crates. Keep it simple and
  identical in every file.
- **Files:** `crates/pw-metadata/tests/sidecar.rs` (`real_dng_is_written_in_place`),
  `crates/pw-ingest/tests/ingest.rs` (`real_raw_uses_exif_time`),
  `crates/pw-libjpeg/tests/decode.rs` (`embedded_previews_of_the_test_raws_decode`),
  `tests/fixtures/README.md`.
- **The pattern:**
  - Every test that reads `PW_TESTDATA` gets
    `#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]`.
  - In each test file, a small helper replaces the early `return`s:
    `fn testdata() -> PathBuf` returns the folder from `PW_TESTDATA`, and **panics with a clear
    message** if the variable is not set or the folder does not exist. If a test needs a specific
    file (or a file with a given prefix), a second helper panics with a message that names the
    missing file and the folder it looked in.
  - `panic!`/`unwrap` are allowed here: this is test code (AGENTS.md §3).
  - Tests that do not read `PW_TESTDATA` are not touched and stay un-ignored.
  - Do not change what the tests check. Update the comment at the top of each file so it says how
    to run them (`$env:PW_TESTDATA = "<folder>"; cargo test -p <crate> -- --ignored`).
  - `tests/fixtures/README.md`: replace "are skipped when it is not set" with the new behaviour and
    the command above.
  - Leave `pw-metadata`'s `test_support::fake_exiftool_entry` as it is (it is a helper, already
    ignored).
- **Report back:** the output of
  1. `cargo test -p pw-metadata -p pw-ingest -p pw-libjpeg` (the three tests show as "ignored");
  2. the same with `-- --ignored` and `PW_TESTDATA` **not** set (the three tests fail with your
     message; paste one message);
  3. the same with `-- --ignored` and `PW_TESTDATA` set by the owner (all pass). If a test cannot
     find its file in the owner's folder, **stop and report which file and folder**; do not
     change the test to look elsewhere.
- **Done when:** the three runs behave as described and clippy is clean.
