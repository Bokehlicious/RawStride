# T272. ADR 0007 §9.7 Q21 and Q22: the digest pass and the TIFF restart dropped, the copy's checksum checked (pw-metadata) — Open

- **Why:** ADR 0007 revision 30 (§9.7 points 2 and 3, Q21 and Q22; approved by the owner
  2026-11-22). With ExifTool 13.64, now the lowest version (T271), `-IPTCDigest=new` in the IIM
  write's own call stores a current checksum in a TIFF or DNG, and a process that has written a
  TIFF with an `AgHg` resource reads TIFFs correctly afterwards (T269, the analysis
  `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T269"). So:
  - **Q21:** the digest pass (`-IPTCDigest=new -overwrite_original <copy>`, §9.2) is dropped. In
    its place, §9.3 point 2 gets a check: when the plan writes `-IPTCDigest=new`, the copy's
    `Photoshop:IPTCDigest` must equal its `File:CurrentIPTCDigest`, or the copy is refused like
    any failed check. Both keys are already in the copy's snapshot, so the check adds no
    ExifTool call. A checksum that is out of date is then never stored silently.
  - **Q22:** the restart after a TIFF write is dropped. If a misread came back, it would show as
    new warnings and §9.3 would refuse the copy; a file is never written wrongly.

  Saving: about 17–24 % of a TIFF write and 8–11 % of a DNG write. A JPEG, a sidecar and a write
  without a new checksum are unchanged.
- **Crate:** `pw-metadata`. **Files:**
  - `crates/pw-metadata/src/safe_write.rs` (the module comment; `write_copy`; `digest_pass`,
    `needs_digest_pass`, `needs_restart` and the restart in the write; their unit tests)
  - `crates/pw-metadata/src/verify.rs` (the new check in `verify_copy`, a new `VerifyError`
    variant, unit tests)
  - `crates/pw-metadata/src/exiftool.rs` (the doc comment of `ExifTool::restart` only)
  - `crates/pw-metadata/tests/safe_write.rs` (the T92 tests' comments; one new test)
  - `docs/test-inventory.md` (the tests removed, changed and added)
  - this task's own file (status and log)
- **What:**
  1. **Read first:** `safe_write.rs`'s module comment (§9.2, §9.3), `write_copy`, `digest_pass`,
     `needs_digest_pass`, `needs_restart` and where the restart is called (`restarted`);
     `verify.rs`'s `verify_copy`, `VerifyError`, `writes_new_digest`, the D8 allowance
     (`created_photoshop_block` or its equivalent) and the keys it ignores
     (`File:CurrentIPTCDigest`); the T92 tests in `tests/safe_write.rs`
     (`a_tiff_with_iim_keeps_a_current_digest`, `a_tiff_without_iim_gets_iim_and_a_current_digest`,
     `a_tiff_with_an_aghg_resource_is_written_and_read_correctly_afterwards`,
     `every_real_tiff_and_dng_gets_a_current_digest`).
  2. **The digest pass dropped.** `write_copy` makes the plan's one call and nothing more;
     `digest_pass`, `needs_digest_pass` and whatever only they use (for example the answer check
     of the digest pass) are removed. The plan still holds `-IPTCDigest=new` where it does today
     (`write_plan.rs` is not changed). D17 and D18's harmless write warnings still apply to the
     one call.
  3. **The restart dropped.** The safe write no longer calls `ExifTool::restart` after a TIFF
     write; `needs_restart` is removed. `ExifTool::restart` itself stays (its own tests in
     `exiftool.rs` and `examples/digest_and_restart.rs` use it; the worker and the pool start new
     processes with `ExifTool::start_with`, not with it); its doc comment no longer names the T92
     misread as the safe write's reason, and says that since ADR 0007 §9.7 (ExifTool 13.64) the
     safe write does not restart. If anything else (a field, a log text, a `Stage`, a parameter
     such as `write_copy`'s `kind`, an import such as `NEW_DIGEST` or `writes_new_digest` in
     `safe_write.rs`) is left unused, it goes too (clippy must stay clean).
  4. **The checksum check.** In `verify_copy`: when `writes_new_digest(plan)`, the copy's listing
     must hold `Photoshop:IPTCDigest` and `File:CurrentIPTCDigest`, equal to each other; otherwise
     a new `VerifyError` variant (for example `DigestNotCurrent { stored, current }`, each an
     `Option<String>`) refuses the copy. When the plan does not write the checksum, nothing is
     checked: a file whose stored checksum is out of date because of an unresolved conflict (§5.2
     Q1) must still pass, as today. The check does not look at the file kind, so it covers a JPEG
     too (a JPEG never had a digest pass; T265 and T269 found its one call current, and
     `tests/verify.rs`'s JPEG test with `-IPTCDigest=new` must still pass). The variant's text
     follows the other `VerifyError` texts and README rule 13. Put the check **after** the
     validation check (the new-warnings test), with the listing checks, so a copy with both a new
     warning and a stale checksum is still refused as `NewWarnings` (the existing test
     `one_more_warning_is_refused_outside_the_photoshop_block_case` expects that).
  5. **Comments.** The module comment of `safe_write.rs` (§9.2, §9.3) describes the write as it now
     is, names ADR 0007 §9.7 (Q21, Q22) and T269, and no longer describes the digest pass and the
     restart as steps. The T92 tests in `tests/safe_write.rs` stay and keep their assertions; their
     comments say they now check that ExifTool 13.64 or newer writes a current checksum in one call
     and reads correctly afterwards. Comments that record what was measured with 13.59 may stay
     when they say so. `verify.rs`'s module comment names the new check, and the doc comment of
     `NEW_DIGEST` no longer calls it the digest pass's argument.
  6. **Existing tests that change (the owner's decision, Q21 and Q22; README rule 5 is lifted for
     exactly these):**
     - `verify.rs`: the test copies of `the_digest_argument_allows_the_photoshop_digest` and of
       the `photoshop_block` helper (used by
       `one_more_warning_passes_when_a_tiff_or_dng_gets_its_photoshop_block` and the D8 tests) hold
       `Photoshop:IPTCDigest` without `File:CurrentIPTCDigest`; add an equal
       `File:CurrentIPTCDigest` to them, so they test what they tested before. Their expected
       results do not change.
     - `safe_write.rs`: the tests of `needs_digest_pass`, `needs_restart` and the digest pass's
       answer check (`digest_written`) go with the functions, including the `digest_written`
       asserts inside the D17 and D18 tests; those tests keep their asserts on the write call's
       answer. Name every removed or cut test in the log.
- **Tests to add:**
  - `verify.rs`: a TIFF or DNG copy whose plan writes `-IPTCDigest=new` and whose stored and
    current checksums differ is refused with the new variant; equal ones pass; one where
    `Photoshop:IPTCDigest` is missing is refused; a plan without `-IPTCDigest=new` passes with
    different checksums (the §5.2 Q1 case).
  - `tests/safe_write.rs`: none beyond the T92 tests, which already check a current checksum
    after `write_safely` in a TIFF with and without IIM and after the `AgHg` TIFF (the number of
    ExifTool calls cannot be seen from outside, and with 13.64 one or two calls give the same
    file). DNG is covered by `every_real_tiff_and_dng_gets_a_current_digest` (run in the Rules
    below), since no small DNG fixture exists.
- **Not in this task:** D8's allowance (stays; a later decision after T270's finding); the DNG
  image check (T273); `write_plan.rs`; the example `examples/digest_and_restart.rs` (it times
  ExifTool calls of its own and stays as T265 and T269 used it); `CHANGELOG.md` and ADR 0007's
  §5.2, §9.2, §9.3 (the reviewer's, at Done).
- **Rules:**
  - The extra code review of the README applies (the safe write).
  - Run, besides `.\tools\check.ps1` (ExifTool 13.64 on PATH, `PW_EXIFTOOL_PL` unset, `exiftool -ver`
    logged from the same command), the real-file tests of
    `pw-metadata` with `PW_TESTDATA=D:\PhotoWorkflowData\RAW`:
    `cargo test --workspace -- --ignored every_real_` (exactly the five tests T269 named); record
    the summary.
- **Done when:** `.\tools\check.ps1` is all OK, the five real-file tests pass, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 30 (§9.7, Q21 and Q22); last of the chain T270 → T271 → T272. Waits for T271 |
| 2026-11-22 | Blocked | Cold read: point 6 added (the `verify.rs` test copies get an equal `File:CurrentIPTCDigest`; the `digest_written` asserts in the D17/D18 tests go; rule 5 lifted for exactly these); the check placed after the validation check; it covers JPEG too; `ExifTool::restart` is used only by its own tests and the example, not by the worker or the pool; unused parameter and imports removed; `verify.rs`'s module comment and `NEW_DIGEST`'s doc; no call-count test (not observable), DNG covered by the real-file test. pw-loupe has no match on `VerifyError` |
| 2026-11-22 | Blocked | Chain review: see T270's log. An undo goes in reverse order: this task before T271 (with this task and an ExifTool below 13.64, every TIFF and DNG write with a new checksum would be refused; ADR 0007 §9.7) |
| 2026-11-22 | Open | T271 done (13.64 the lowest version the app writes with) |
