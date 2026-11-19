# T212. Accept the write warning "Error reading HiddenData" by name (DxO exports of Sony RAWs) — Open

- **Why:** owner's decision D17 (2026-11-19, ADR 0007 §9.2): a JPEG (and possibly a DNG) that DxO
  PhotoLab exported from a Sony RAW keeps Sony's maker note, whose `HiddenInfo` points at data that
  exists only in the original ARW. When ExifTool writes the copy it answers
  `Warning: [minor] Error reading HiddenData - <path>`, and the safe write refuses every answer
  with a warning (`one_file_without_warning` in `crates/pw-metadata/src/safe_write.rs`), so such
  files cannot be rated or edited. The reviewer checked a copy written by ExifTool: the
  `ImageDataHash` is unchanged and the pointer is kept as it was, so nothing is lost. The ignored
  real-file test `every_real_dng_jpeg_and_tiff_is_rated_safely` (`tests/edit.rs`) fails on
  `TN005701_DxO.jpg` and `TN005702_DxO.jpg` for this reason.
- **Crates:** `pw-metadata`. **Files:**
  - `crates/pw-metadata/src/safe_write.rs` (code and its unit tests)
  - `docs/test-inventory.md`
- **What:**
  1. In `safe_write.rs`, a named list of known harmless write warnings (a `const` slice of line
     prefixes with a doc comment naming D17), today one entry:
     `Warning: [minor] Error reading HiddenData`. A line of ExifTool's answer that starts with an
     entry, case-sensitive (after `trim_start`; the rest of the line, ` - <path>`, may be
     anything), is not counted
     as a warning by `one_file_without_warning`. Every other `Warning` line still makes it false,
     also other `[minor]` ones. Nothing else in the function changes: exactly one file, no failed
     file. This applies to both callers, `copy_written` and `digest_written`.
  2. Update the doc comments of `one_file_without_warning`, `copy_written` and `digest_written`
     (about lines 587–600, "no warning" → no warning outside the list), and the module comment
     where it states the rule (about lines 17–18 and 25).
  3. §9.3's checks of the copy do not change: `verify_copy` in `verify.rs` (the `-validate`
     warning counts of copy and original compared, the listing, the image data hash) and
     `final_check`. If the real-file run shows a DxO copy refused there, stop and report.
- **Tests to add** (unit tests next to the existing `copy_written` / `digest_written` tests, about
  lines 705–780):
  - `copy_written` is true for
    `"    1 image files created\r\n--- stderr ---\nWarning: [minor] Error reading HiddenData - C:/x/a.jpg\n"`
    (the form `ExifTool::execute` gives, `exiftool.rs` about lines 261–266; stdout may end its
    lines with `\r\n`, stderr not), and `digest_written` is true for the same warning with
    `1 image files updated`;
  - still false, for both functions: the HiddenData line together with another warning; a
    different `[minor]` warning alone (the existing `IPTC:City exceeds length limit` case stays
    false); the HiddenData line with `0 image files created` / `updated` or a failed file.
- **Not in this task:** the read side and §9.3 (point 3). ADR 0007 is the reviewer's.
- **Rules:** no change to any `[dependencies]`. No other file changes; if one must, stop and
  explain (rule 2).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above and
  this file's status and log; `docs/test-inventory.md` has the new unit-test count (line 21) and
  totals (line 4, "after T87–T212"); the ignored real-file test passes on every file it finds,
  the two DxO JPEGs included:
  `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace every_real_dng_jpeg_and_tiff_is_rated_safely -- --ignored`
  (paste its summary in the report); and the owner's hand check: that test uses `NoImageCheck`,
  so with a release build (`cargo run --release -p pw-loupe`) the owner rates a copy of
  `TN005701_DxO.jpg` in RawStride (the real image check runs), sees no error, and sees the stars
  in DxO PhotoLab or another program.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added after the owner's decision D17 (ADR 0007 §9.2), from T211's real-file run. Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file only): doable in `safe_write.rs` alone; nothing else in the write path or in `pw-imagecheck` and the worker reads warning lines, but `verify_copy` compares the `-validate` warning counts of copy and original. Changed after it: the match is case-sensitive; the test input written in the form `execute` gives (`--- stderr ---`, CRLF only on stdout); both functions get every false case; `verify_copy` and `final_check` named with the stop rule; the module comment lines named; "all 30 files" replaced by every file the test finds; an owner hand check added, since the real-file test uses `NoImageCheck` |
| 2026-11-19 | Open | Cold read done |
