# T127. ADR 0007 I12: real-file tests with the level-C samples — Open

- **Can start now.** The owner put the level-C samples in `PW_TESTDATA\level-c` on 2026-11-14
  (ADR 0007 §12, Q11 as changed that day). It may run beside T126 (`pw-loupe`): both
  change `docs/test-inventory.md`, T127 only the `pw-metadata` rows and the total at the top; the
  one finished second brings the total up to date.
- **Crates:** `pw-metadata` only. **Files:** `crates/pw-metadata/tests/level_c.rs` (new);
  `docs/test-inventory.md`. Nothing in `src/` changes; if a test shows a bug in `pw-metadata`,
  stop and report it (rule 5) instead of changing the code.
- **Why:** milestone 8 has been tested on files built by the tests and on camera files without
  program metadata. I12 checks the reader, the reconciliation and the safe write on files that
  Photo Mechanic, Lightroom Classic and a Xiaomi phone really wrote.
- **The samples** (in `PW_TESTDATA\level-c`, read by the reviewer with ExifTool 13.59 on
  2026-11-14):

  | File | What it is |
  |---|---|
  | `PM-jpg.JPG` | Sony JPEG; Photo Mechanic 2026.6 wrote XMP and IIM (UTF-8, no stored `IPTCDigest`); Description 2191 bytes in both XMP and IIM (over IIM's 2000); Location Created with `Sublocation`, `ProvinceState` and `WorldRegion` written as empty strings; Model Age `NA`; the PLUS URIs `AG-A25`, `MR-NAP`, `PR-NON`, `DMI-UNSPECIFIED`; `xmpRights:Marked` True; the `photomechanic:` namespace; fields the owner filled, saved, emptied and saved again |
  | `xiaomi_jpg.jpg` | Xiaomi 17 Ultra JPEG; Photo Mechanic XMP and IIM (UTF-8, no stored digest), a short Description |
  | `PM-raw.ARW` + `PM-raw.XMP` | Sony ARW never written; its sidecar written by Photo Mechanic (note the upper-case `.XMP`); Description 2191 bytes |
  | `xiaomi_dng.dng` | Xiaomi 17 Ultra DNG with Photo Mechanic XMP |
  | `TN002363.DNG` | Ricoh GR III DNG written by Lightroom Classic 11.4 and Photo Mechanic 5 (Adobe XMP Core), never by RawStride |

- **What:**
  1. **The folder.** The tests read `PW_TESTDATA\level-c` and expect the six files above; a
     missing file fails the test with its name (it is not skipped). They are `#[ignore]` with the
     reason "needs the level-C samples: set PW_TESTDATA and run with --ignored", as the other
     real-file tests, and need ExifTool on PATH.
  2. **The samples are never written.** Every write is done on a copy in a temporary folder (an
     ARW with its sidecar copied together). A test records each sample's size and SHA-256 at the
     start and checks them unchanged at the end.
  3. **No personal data in the code.** Expected values that are the owner's (name, e-mail, phone,
     copyright text) are not written as literals: compare with what `ExifTool` itself reads from
     the file, or only check that the property is present. Neutral values (`Exempelstad`, the
     keywords, the PLUS URIs, `NA`) may be literals.
  4. **Reading** (`read_metadata`, default fallback), each file without error, and:
     - the two JPEGs: IIM read as UTF-8; Description, Creator, Keywords, City and Copyright Notice
       agree between IIM and XMP with no conflict; the IIM checksum case is the one for a missing
       stored digest (ADR 0007 §5.1);
     - `PM-jpg.JPG`: the whole 2191-byte Description; Location Created with City and Country and
       no empty sub-field shown as a value; Model Age read with its entry `NA` and no error; the
       four PLUS properties with their full URIs; Copyrighted True;
     - `PM-raw.ARW`: the sidecar `PM-raw.XMP` found as `<basename>.xmp` despite the upper-case
       extension (Q8), no foreign-sidecar notice, the whole 2191-byte Description;
     - `TN002363.DNG`: read without error; any conflicts listed with their rule (the test prints
       them; it asserts no specific conflict).
  5. **Writing** on each copy with `ExifTool::write_edits_safely` (default options, a journal in
     the temporary folder, `NoImageCheck` as `tests/edits.rs`): one edit, Headline set to a
     neutral text. Then, read back:
     - the Headline is there and **every other value of the read before is unchanged** (the whole
       reconciled metadata compared, Headline left out);
     - the `photomechanic:` properties, `xmpMM` history, Location Created (with its empty
       sub-fields still in the file), Model Age `NA` and the PLUS URIs are kept (§8, unknown and
       unchecked values never rewritten);
     - `PM-jpg.JPG`: XMP keeps the full Description; IIM Caption-Abstract is written at most 2000
       bytes with a `Truncated` notice for `dc:description` (§5.2); a stored `IPTCDigest` now
       matches the current one;
     - the image data is unchanged (`ImageDataHash` of the JPEGs and DNGs; the ARW's bytes, only
       its sidecar written); no `.pwtmp-*` or `.pwbak-*` file is left and the journal is empty.

     If a write behaves otherwise than these points (for example the IIM Description is not
     brought in line), stop and report what happened with the notices and the read-back values;
     do not change the expectations.
  6. **What the samples show about Photo Mechanic**, printed by the test for the review (not
     asserted beyond point 4): the top-level XMP properties with an empty value in `PM-jpg.JPG`
     (expected none: Photo Mechanic removes an emptied field) and the sidecar's file name.
- **Not in this task:** the real image check (`RawImageCheck` in `pw-imagecheck`); TIFF; ratings;
  the egui program; any change in `pw-metadata`'s code.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add:** the tests of points 1–6 in `crates/pw-metadata/tests/level_c.rs`.
- **Done when:** `.\tools\check.ps1` is all OK (the new tests ignored there), the new tests pass
  with `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace --test level_c --
  --ignored` (output in the report), `docs/test-inventory.md` is updated and `git diff --stat`
  shows only the files above. After the review the reviewer writes what the samples showed into
  ADR 0007 (§4 Q8, §6.4).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Written by the reviewer (ADR 0007 I12) after the owner put the level-C samples in `PW_TESTDATA\level-c` |
