# T234. The level-C tests follow the samples as they are now, and the samples are read-only — Done

- **Why:** every level-C sample in `PW_TESTDATA\level-c` except `PM-raw.ARW` was written on
  2026-11-19 between 22:25 and 22:28, most likely in a hand check in RawStride (ExifTool 13.59).
  Since then `the_two_jpegs_read_iim_as_utf8_in_agreement_with_xmp` fails (`PM-jpg.JPG`: digest
  `Matches`, expected `Missing`). The owner, 2026-11-20: there is no backup of the earlier state
  (the folder `level-c - kopia` is another state and does not satisfy the test either), so the
  tests follow the samples as they are now, and the samples are made read-only so a hand check
  cannot change them again (`backlog.md`, "The level-C samples were changed outside the tests").
  - **Measured by the reviewer (2026-11-20, ExifTool 13.59):** in both `PM-jpg.JPG` and
    `xiaomi_jpg.jpg` `Photoshop:IPTCDigest` is now stored and equals `File:CurrentIPTCDigest`
    (so `read_metadata` gives `IimDigest::Matches`), and `IPTC:City` is now present and equal to
    `XMP-photoshop:City`. Keywords, Description, Creator and Copyright Notice still agree between
    IIM and XMP; the files also got ratings (`XMP-xmp:Rating`), which no test asserts. In T233's
    `.\tools\check.ps1 -Ignored` run every other `level_c` test passed.
  - **Read-only and the copies:** Windows' file copy (`std::fs::copy`) keeps the read-only
    attribute, and the safe write refuses a read-only file in its precheck
    (`tests/edit.rs`, `a_read_only_jpeg_fails_in_precheck`). Once the samples are read-only, every
    write test on a copy would fail unless the copy is made writable.
- **Before the task runs (the owner):** the six samples are made read-only (Windows' read-only
  attribute, `attrib +R` on each file in `PW_TESTDATA\level-c`; the reviewer runs it on the
  owner's yes and writes a log row). The agent does not change the samples or their attributes;
  if any sample is still writable when the task starts, stop and report it.
- **Crates:** `pw-metadata` (tests only). **Files:** `crates/pw-metadata/tests/level_c.rs`,
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **The two JPEGs** (`the_two_jpegs_read_iim_as_utf8_in_agreement_with_xmp`):
     - the digest is `IimDigest::Matches` instead of `Missing`;
     - instead of asserting that `Photoshop:IPTCDigest` is `-`, assert that it is not `-` and
       equals what ExifTool reads as `File:CurrentIPTCDigest` for the same file (both through the
       `tag` helper);
     - City is in IIM in both files now: remove the `None if *property == "photoshop:City"` arm
       and its comment, so all five properties of `IIM_AND_XMP` must be in IIM and agree;
     - the doc comment says what the test now checks ("the stored digest matches", City in IIM),
       with the date 2026-11-20 and the reason in one line (the samples were written outside the
       tests on 2026-11-19; no backup).
  2. **The copies are writable** (`Scratch::copy`): after each `std::fs::copy`, the copy's
     read-only attribute is cleared (`std::fs::set_permissions` with `set_readonly(false)`), with a
     comment saying why (the samples are read-only, the safe write refuses a read-only file).
     `check.ps1` runs clippy on all targets, and its `permissions_set_readonly_false` lint fires
     on this call: write it as its own statement (`let mut permissions = …;` then
     `permissions.set_readonly(false);`) and put
     `#[allow(clippy::permissions_set_readonly_false)]` on that statement, as
     `src/fileops.rs:466–469` does (the file is built on Windows only). Restoring saved
     permissions, as `tests/edit.rs` does, does not work here: the copy is read-only from the
     start. The samples themselves are never written or changed.
  3. **The module comment** at the top of the file: one sentence that the samples were also
     written by RawStride (ExifTool 13.59) on 2026-11-19, that the tests follow them as they are
     since then (2026-11-20, T234), and that the samples are kept read-only.
- **Tests to add:** in `tests/level_c.rs`, ignored like the others:
  - `every_sample_is_read_only` (the same `#[ignore = "needs the level-C samples: set
    PW_TESTDATA and run with --ignored"]` as the others): collects the names in `SAMPLES`
    (through `sample`) whose `std::fs::metadata(..).permissions().readonly()` is false and
    asserts the list is empty; the message lists them and says to make the level-C samples
    read-only (`attrib +R`), so a hand check cannot change them.
  - No other test changes: point 1 is the only test whose assertions change. If any other
    `level_c` test fails in the ignored run, stop and report it with its message; do not change
    it.
- **docs/test-inventory.md:** the `level_c.rs` row says the two JPEGs have a matching stored
  digest and City in IIM (not "a missing stored digest" and "City is in XMP only"), that the
  copies are made writable, and lists the new test. The counts: line 4's ignored total (+1) and
  its "after T87–T233" (→ T234), the passed total as the run gives it; row 42's last two
  columns (tests +1, ignored +1) and its "(12 ignored)" in the prerequisites text.
- **Not in this task:** changing or restoring the samples (the owner has no backup; do not
  propose restoring `level-c - kopia`); any change to `pw-metadata`'s code; a note in ADR 0007
  §12 that the samples now carry RawStride's writes and are read-only (the reviewer writes it at
  Done, `AGENTS.md` §6); the backlog item is struck by the reviewer at Done.
- **Rules:** `docs/tasks/README.md` rules 1–12. Test code may use `.unwrap()`. Run
  `.\tools\check.ps1` and `.\tools\check.ps1 -Ignored` with `PW_TESTDATA` set (`AGENTS.md` §5) and
  report both; ExifTool on `PATH` or `PW_EXIFTOOL_PL` (docs/TESTING.md §4.5).
- **Done when:** `.\tools\check.ps1` is all OK, every `level_c` test passes in the ignored run
  (the new one included), and `git status` shows only the files above changed.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added. Measured first by the reviewer (ExifTool 13.59 on the samples and on `level-c - kopia`): only the stored digest and City in IIM differ from what the test expects |
| 2026-11-20 | Open | Cold read: doable with the named files; the measured claims confirmed with ExifTool. Fixed: the clippy lint fires for certain (allow on its own statement, as `fileops.rs`); stop if a sample is still writable; the new test's ignore text and that it lists every writable sample; the inventory's four count edits named; Done checks `git status` (the task file is new) |
| 2026-11-20 | Open | The six samples in `PW_TESTDATA\level-c` made read-only by the reviewer on the owner's yes (Windows read-only attribute; sizes and times unchanged, checked) |
| 2026-11-20 | Ready for review | All six samples read-only at the start (checked). `tests/level_c.rs`: the two JPEGs' digest `Matches` with `Photoshop:IPTCDigest` set and equal to `File:CurrentIPTCDigest`, the City arm removed, doc and module comments; `Scratch::copy` makes each copy writable (allow on its own statement); new ignored `every_sample_is_read_only`. `docs/test-inventory.md` row 42 and counts. `.\tools\check.ps1` all OK (2016 passed, 75 ignored; the passed total includes T235's tests, which another session was writing in the same worktree), no clippy warnings; `-Ignored` all OK, 13 `level_c` tests passed. Note: the other session ran `check.ps1` at the same time once, mixing the two runs in `test-output.txt`; both checks were run again after it finished. Inventory line 4 was rewritten by that session to "after T87–T235", which includes this task's count |
| 2026-11-20 | Done | Reviewed: the diff of `tests/level_c.rs` and the `level_c` row of `docs/test-inventory.md` match points 1–3 and the new test; all six samples read-only (attrib). The reviewer's own runs: `.\tools\check.ps1` all OK (2016 passed, 75 ignored with T235's tests in the worktree), no clippy warnings; `.\tools\check.ps1 -Ignored` all OK, the 13 `level_c` tests passed. Committed before T235, the inventory header set to after T87–T234 (2008 passed, 75 ignored). Recorded: ADR 0007 §12 (the samples carry RawStride's writes and are kept read-only, owner 2026-11-20); the backlog item struck |
