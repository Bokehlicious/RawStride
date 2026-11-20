# T238. Accept the write warning "Fixed incorrect HiddenDataOffset" by name (Sony ILCE-1 JPEGs) — Open

- **Why:** the owner's hand check of ADR 0024 (2026-11-20) found that a turn of a JPEG from a Sony
  ILCE-1 is always refused and rolled back (the file is unchanged, so it fails safe). Measured by
  the reviewer with ExifTool 13.59 on scratch copies (2026-11-20), writing `-IFD0:Orientation#=6`
  with `-o`:
  - `_TN06333.JPG` (a camera JPEG) and the level-C sample `PM-jpg.JPG` (a camera JPEG later
    written by Photo Mechanic and RawStride): ExifTool answers `1 image files created` and, on
    standard error, `Warning: [minor] Fixed incorrect HiddenDataOffset - <path>`. The safe write
    refuses every answer with a warning outside `HARMLESS_WRITE_WARNINGS`
    (`one_file_without_warning` in `crates/pw-metadata/src/safe_write.rs`), so `copy_written` is
    false and the write fails before `verify_copy` runs.
  - In the copy only `IFD0:Orientation` and `Sony:HiddenDataOffset` changed (13443072 → 13388996
    and 9232384 → 9186032); the file size and the validation counts are the same; the hidden data
    (`-b -HiddenData`, 53248 bytes) has the same SHA-256 in original and copy, so ExifTool only
    corrects the pointer to where the data really is. In the original, the 53248 bytes at the
    stored offset are not the hidden data; in the copy, the bytes at the new offset are.
  - `verify_copy` would accept the copy: `Sony:HiddenDataOffset` ends in `Offset`, which
    `POINTER_ENDINGS` (`verify.rs`) always allows. (The backlog's first note, that verify refuses
    it, was wrong.)
  - An XMP-only write (`-XMP-xmp:Rating=3`) of the same files gives no warning: only a write that
    rewrites the Exif and its maker note does. So the metadata window's writes of Exif fields
    (`IFD0:` tags) into such a file are refused today for the same reason.
  - `TN005701_DxO.jpg` (a DxO export, D17) turns already: ExifTool answers with the accepted
    `Error reading HiddenData` warning, and `Sony:HiddenDataOffset` changes as a pointer.

  **Owner's decision D18 (approved 2026-11-20, ADR 0007 §9.2, §17, revision 28):** add
  `Warning: [minor] Fixed incorrect HiddenDataOffset` to the named list of harmless write warnings
  (D17), for every write, not only an orientation write, since the same warning comes with any
  Exif write into such a file. Every other warning is still refused; §9.3's checks of the copy run
  as before.
- **Crates:** `pw-metadata`. **Files:**
  - `crates/pw-metadata/src/safe_write.rs` (the list, its doc comment, the module comment, one
    unit test)
  - `crates/pw-metadata/tests/level_c.rs` (one real-file test)
  - `docs/test-inventory.md`
  - this task's own file (status and log)
- **What:**
  1. In `safe_write.rs`, add a second entry to `HARMLESS_WRITE_WARNINGS` (about line 597):
     `"Warning: [minor] Fixed incorrect HiddenDataOffset"`. The matching rule does not change
     (case-sensitive line prefix after `trim_start`). Its doc comment (about lines 589–596) gets a
     second bullet in the style of the first: a JPEG from a Sony camera (measured on the ILCE-1)
     whose maker note's `HiddenDataOffset` does not point at the hidden data; on an Exif write
     ExifTool corrects the pointer, and the hidden data and the image data hash are unchanged
     (D18, 2026-11-20). The first line of the doc comment names D17 and D18 ("owner's decisions
     D17, 2026-11-19, and D18, 2026-11-20"). In the module comment (about lines 18–19),
     "(`HARMLESS_WRITE_WARNINGS`, D17)" becomes "(`HARMLESS_WRITE_WARNINGS`, D17 and D18)".
  2. Nothing else in `safe_write.rs` or `verify.rs` changes. If the level-C test (point 3) is
     refused by `verify_copy` or `final_check`, stop and report the key or check that refused it.
  3. In `tests/level_c.rs`, under "The orientation (ADR 0024 O2, T233)" (about line 1057), an
     ignored test `an_orientation_on_a_copy_of_pm_jpg_fixes_the_hidden_data_offset` in the style of
     the two DNG tests (about lines 1092 and 1111):
     - the fingerprints of `PM_JPG`;
     - preconditions on the sample: `IFD0:Orientation#` is `8`, `Sony:HiddenDataOffset#` is
       `9232384` and `Sony:HiddenDataLength#` is `53248` (read with `tag`);
     - the copy turned to `6` with `orientation_on_a_copy` (about line 1065). Its doc comment says
       "the DNG `name`" but it works for any file: change only those words to "the sample `name`";
     - then, on the copy: `Sony:HiddenDataOffset#` is `9186032` and `Sony:HiddenDataLength#` is
       `53248`, and the SHA-256 (`sha256_hex`) of the copy's bytes from that offset for that
       length (read with `std::fs::read`, the range checked against the file's length) is the hash
       of the hidden data, a `const` beside the test:
       `f158d4da9ed6557766c0b89a72b9917526a5693d299347c79cc2b9588b720e7c` (measured by the
       reviewer: the SHA-256 of `exiftool -b -HiddenData PM-jpg.JPG`, the same for the copy;
       `ExifTool::execute` returns text, so the test cannot read the binary itself);
     - and the sample's own bytes at its stored offset (9232384, length 53248) do not have that
       hash, which shows the pointer was wrong before and is right after.

     Its doc comment names D18 and says the numbers were measured on the sample as it is since
     2026-11-20 (T234: the samples are read-only, so the numbers stay).
- **Tests to add:**
  - in `safe_write.rs`, one **new** unit test
    `the_hidden_data_offset_warning_is_accepted_by_name`, after T212's two tests
    (`the_hiddendata_warning_alone_is_accepted_by_name` and the one after it, about lines
    799–874), with a `const HIDDEN_DATA_OFFSET` for the new line beside `HIDDEN_DATA`. The T212
    tests are not changed. It checks: `copy_written` and `digest_written` are true for
    `"    1 image files created\r\n--- stderr ---\nWarning: [minor] Fixed incorrect HiddenDataOffset - C:/x/a.jpg\n"`
    (and `updated`), and true for that line together with the D17 line; still false for it with
    another warning, for `Warning: [Minor] Fixed incorrect HiddenDataOffset`, and with `0 image
    files created` / `updated` or a failed file;
  - the level-C test of point 3 (ignored by default).

  So `.\tools\check.ps1` counts one more passed test and one more ignored test.
- **Not in this task:** ADR 0007 (done by the reviewer, see Why); the read side; the shell (a
  refused turn already rolls back and shows its error, T237); `CHANGELOG.md` (the reviewer's, at
  Done).
- **Rules:** no change to any `[dependencies]`. No other file changes; if one must, stop and
  explain (rule 2). The samples are never written (the level-C file comment).
- **Done when:**
  - `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above;
  - `docs/test-inventory.md`: line 4 (the run's totals, one more passed and one more ignored,
    "after T87–T238"); line 21 (the `pw-metadata` unit-test count, and its description names the
    D18 warning beside the D17 one); line 42 (`tests/level_c.rs`: the test and ignored counts,
    also the "(13 ignored)" in the Prerequisites cell, and the orientation sentence names the
    `PM-jpg.JPG` test beside the two DNGs);
  - the level-C tests pass with
    `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace --test level_c -- --ignored`;
  - as a regression check (a rating write is XMP-only and did not hit the warning), the
    real-file test still passes with
    `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace every_real_dng_jpeg_and_tiff_is_rated_safely -- --ignored`;
  - paste both summaries in the report.

  Then, after the review, the owner's hand check (not the agent's; the agent marks the task
  "Ready for review" without waiting for it): in a release build
  (`cargo run --release -p pw-loupe`), a copy of `_TN06333.JPG` is turned with Ctrl+Right, the
  turn stays after a Reload, no error is shown, and another program shows the file turned.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Blocked | Added from the owner's hand check of ADR 0024 (backlog item "A turn of a Sony ILCE-1 JPEG is always refused"), with the reviewer's measurement above. Waits for the owner's decision D18 and the cold read before Open |
| 2026-11-20 | Blocked | The owner approved D18; the reviewer wrote it into ADR 0007 (§9.2, §17, revision 28) |
| 2026-11-20 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file only): doable with the named files; every name matches the code, `tag` with `#` gives numbers, and `orientation_on_a_copy` fits a JPEG (`PM-jpg.JPG` has no `XMP-tiff:Orientation`, the second write is `Unchanged`). Changed after it: the byte check compared copy and sample in the same range, which passes for any range since the size does not change; it now compares the copy's range with the measured hash of the hidden data, and checks that the sample's old range does not have it; the offsets asserted; one new, named unit test, the T212 tests unchanged; the module comment's "D17" updated; `PM-jpg.JPG` is not straight from the camera; the read-warning sentence removed (not seen in a plain read); the real-file rating test marked as a regression check; the inventory's lines 4, 21 and 42 named; the hand check is the owner's, after the review |
| 2026-11-20 | Open | Cold read done |
