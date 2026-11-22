# T274. ADR 0007 §9.7 point 5: D8's allowance dropped, one more validation warning always refused (pw-metadata) — Done

- **Why:** ADR 0007 revision 31 (§9.7 point 5, §17.1; the owner's decision 2026-11-22). D8 (ADR
  0007 §17, 2026-11-12) lets the safe write accept exactly one more validation warning when the
  write creates the Photoshop block of a TIFF or DNG, because ExifTool 13.59 then added "Non-standard
  format (undef) for IFD0 0x8649 PhotoshopSettings". ExifTool 13.64, now the lowest version the app
  writes with (T271), writes that block without the warning (T270 point 6: a Xiaomi DNG, 8
  warnings with 13.59, 7 with 13.64; the writer differs, not the reader). The allowance now only
  lets an unrelated new warning through unnoticed, so it goes: one more warning is refused in every
  case (§9.3).
- **Before this task (the owner):** ExifTool 13.64 first on PATH (as for T271 and T272).
- **Crate:** `pw-metadata`. **Files:**
  - `crates/pw-metadata/src/verify.rs` (the allowance in `verify_copy`, `creates_photoshop_tag`,
    `PHOTOSHOP_TAG_TYPES`, the module comment, the D8 unit tests)
  - `crates/pw-metadata/tests/safe_write.rs` (the comment of
    `a_tiff_without_iim_gets_iim_and_a_current_digest` only)
  - `docs/test-inventory.md` (the two rows that name D8: the `pw-metadata` unit tests and
    `tests/safe_write.rs`; the counts if they change)
  - this task's own file (status and log)
- **What:**
  1. **Read first:** in `verify.rs`, `verify_copy` (the comment "D8 (owner, 2026-11-12; T92)" and
     `extra_warnings`), `creates_photoshop_tag`, `PHOTOSHOP_TAG_TYPES`, the module comment, the
     test helper `photoshop_block` and the tests
     `one_more_warning_passes_when_a_tiff_or_dng_gets_its_photoshop_block` and
     `one_more_warning_is_refused_outside_the_photoshop_block_case`; in `tests/safe_write.rs`,
     `a_tiff_without_iim_gets_iim_and_a_current_digest` (a synthetic TIFF without IIM written with
     "create IIM": the D8 case, and the only TIFF test of it, since every TIFF in `PW_TESTDATA`
     already has a checksum).
  2. **The allowance removed.** `verify_copy` refuses a copy with more errors or more warnings
     than the original, with no exception; `extra_warnings`, `creates_photoshop_tag` and
     `PHOTOSHOP_TAG_TYPES` go if nothing else uses them (clippy must stay clean). The rest of the
     check (`(None, None)` passes, one side missing is refused, minor warnings are not compared)
     is unchanged.
  3. **Comments.** `verify.rs`'s module comment no longer names the D8 exception; the comment in
     `verify_copy` may say in one line that D8's allowance was dropped (ADR 0007 §9.7 point 5,
     ExifTool 13.64). The comment of `a_tiff_without_iim_gets_iim_and_a_current_digest` no longer
     says "with one more validation warning (D8)"; it says that the copy is accepted without the
     allowance (no new validation warning with ExifTool 13.64 or newer). Its assertions stay as
     they are.
  4. **Existing tests that change (the owner's decision; README rule 5 is lifted for exactly
     these):** `one_more_warning_passes_when_a_tiff_or_dng_gets_its_photoshop_block` is replaced
     by `one_more_warning_is_refused_also_when_a_tiff_or_dng_gets_its_photoshop_block`, a test
     that the same cases (`photoshop_block(file_type, false, &[NEW_DIGEST],
     validate(0, 8, 1))` for TIFF and DNG) are now refused with `VerifyError::NewWarnings`, and
     that `validate(0, 7, 2)` (more minor warnings only) and `validate(0, 7, 1)` (no change) still
     pass. `one_more_warning_is_refused_outside_the_photoshop_block_case` keeps every refused case
     (its name may drop "outside the photoshop block case"). `photoshop_block` stays if a test
     uses it. Name every renamed or replaced test in the log.
  5. **The gate (stop if it fails).** After `.\tools\check.ps1`, run the real-file tests of
     `pw-metadata` with `PW_TESTDATA=D:\PhotoWorkflowData\RAW`:
     `cargo test --workspace -- --ignored every_real_` (the five tests T269 and T272 named). The
     D8 case is covered by `every_real_tiff_and_dng_gets_a_current_digest` (it writes City with
     "create IIM" into every TIFF and DNG at the top of `PW_TESTDATA`; nine of those DNGs, five
     Xiaomi and four Ricoh, have no `Photoshop:IPTCDigest`, so the safe write creates their
     Photoshop block) and, for TIFF, by `a_tiff_without_iim_gets_iim_and_a_current_digest` in
     `check.ps1`. Also run `cargo test --workspace --test level_c -- --ignored` (`PW_TESTDATA` as
     above) as a regression run only: its samples have no IIM and its writes do not create IIM,
     so it does not meet the D8 case. If any of these tests fails, **do not** put the allowance
     back or change a test: set the task to Blocked, and write in the log which file was refused
     and its validation counts before and after. For that, copy the file into a new folder under
     `%TEMP%`, write it there with
     `exiftool -o <copy> -IPTC:CodedCharacterSet=UTF8 -IPTC:City=Lund -XMP-photoshop:City=Lund -IPTCDigest=new <file>`,
     run `exiftool -validate -warning -a` on the file and on the copy, record both, and delete
     the folder.
- **Tests to add:** none beyond point 4.
- **Not in this task:** D17 and D18 (write warnings accepted by name, `safe_write.rs`); the
  checksum check (T272); `CHANGELOG.md`, ADR 0007 and `backlog.md` (the reviewer's, at Done).
- **Rules:**
  - `AGENTS.md` applies. The extra code review of the README applies (the safe write).
  - `PW_EXIFTOOL_PL` unset, and `exiftool -ver` (13.64) logged from the same command as
    `.\tools\check.ps1`.
- **Done when:** `.\tools\check.ps1` is all OK, the real-file tests of point 5 pass (summaries in
  the log), and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Written after ADR 0007 revision 31 (the owner dropped D8's allowance, 2026-11-22, after T270 point 6). The reviewer listed the TIFFs and DNGs of `D:\PhotoWorkflowData\RAW` without `Photoshop:IPTCDigest` with ExifTool 13.64 (read only): 13 of 22, all DNG (Xiaomi and Ricoh, nine at the top, two in `level-c`, two in a copy of `level-c`); every TIFF has a checksum |
| 2026-11-22 | Open | Cold read: the named symbols and tests exist, nothing else uses `creates_photoshop_tag`, `PHOTOSHOP_TAG_TYPES` or `extra_warnings`; point 4's change matches the current tests. Changed: the replacement test named (`one_more_warning_is_refused_also_when_a_tiff_or_dng_gets_its_photoshop_block`); `level_c` is only a regression run (the reviewer checked: its two DNGs have no IIM, and its writes do not create IIM), the D8 gate is `every_real_tiff_and_dng_gets_a_current_digest` and the synthetic TIFF test; the ExifTool command for a refused file written out; the two test-inventory rows that name D8 |
| 2026-11-22 | Ready for review | `verify.rs`: the allowance removed from `verify_copy` (one more error or warning always refused; `(None, None)` passes, one side missing refused, minor warnings not compared, all unchanged), with a one-line comment naming ADR 0007 §9.7 point 5; `extra_warnings`, `creates_photoshop_tag` and `PHOTOSHOP_TAG_TYPES` removed (nothing else used them); the module comment no longer names D8; the helper `photoshop_block` kept (both tests use it), "D8:" dropped from its doc. Tests: `one_more_warning_passes_when_a_tiff_or_dng_gets_its_photoshop_block` replaced by `one_more_warning_is_refused_also_when_a_tiff_or_dng_gets_its_photoshop_block` (TIFF and DNG, `validate(0, 8, 1)` → `NewWarnings`; `validate(0, 7, 2)` and `validate(0, 7, 1)` pass); `one_more_warning_is_refused_outside_the_photoshop_block_case` renamed `one_more_warning_is_refused`, every case kept. `tests/safe_write.rs`: only the comment of `a_tiff_without_iim_gets_iim_and_a_current_digest` (accepted without D8's allowance, ExifTool 13.64 or newer); assertions unchanged. `docs/test-inventory.md`: the two D8 rows; counts unchanged (one test replaced, one renamed). check.ps1 all OK (2291 passed, 77 ignored; PW_EXIFTOOL_PL empty, exiftool -ver 13.64 in the same command), no clippy warnings. Gate, PW_TESTDATA=D:\PhotoWorkflowData\RAW, exiftool -ver 13.64: `cargo test --workspace -- --ignored every_real_`: 5 passed (1 + 2 + 2, incl. `every_real_tiff_and_dng_gets_a_current_digest`); `cargo test --workspace --test level_c -- --ignored`: 14 passed. No file refused. The extra code review (safe write) is left to the reviewer |
| 2026-11-22 | Done | Reviewed: the allowance fully removed, the replaced and renamed tests as point 4 says, every refused case kept. Extra code review by an independent review agent: no bugs, no unused items (`plan`, `DIGEST_KEY`, `writes_new_digest` still used), no new panic paths; two nits left as they are (the case labels of `one_more_warning_is_refused` still name D8's conditions; the new doc sentence of `a_tiff_without_iim_gets_iim_and_a_current_digest` is long). Reviewer's own run: check.ps1 all OK (2291 passed, 77 ignored; PW_EXIFTOOL_PL empty, exiftool -ver 13.64 in the same command), no clippy warnings; PW_TESTDATA=D:\PhotoWorkflowData\RAW: `every_real_` 5 passed, `level_c` 14 passed. Reviewer: CHANGELOG (Changed), ADR 0007 §18 line, README index, done list, plan row 2a. No hand check (the real-file tests cover the case) |
