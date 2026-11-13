# T92. ADR 0007 I9f: a current IIM checksum in TIFF and DNG — Done

- **Can start now.** The owner approved D8 (one more validation warning allowed when the safe
  write creates the Photoshop block of a TIFF or DNG) on 2026-11-12. It can run at the same time
  as T88 (no shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/safe_write.rs`; `crates/pw-metadata/src/verify.rs`;
  `crates/pw-metadata/tests/safe_write.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 and §12 (the checksum is written and current after every write that has
  IIM and XMP), §9.3 (the copy is verified). Found in the review of T87: the ignored real-file
  test refuses `TN002683.tif` (a TIFF with IIM and a matching `IPTCDigest`) in `Verify` with one
  more warning, `IPTCDigest is not current`. Measured by the reviewer with ExifTool 13.59 (see the
  step plan): in a **TIFF or DNG**, `-IPTCDigest=new` in the same call as an IIM write stores the
  MD5 of the IIM block **without** the padding ExifTool adds to it in the TIFF structure, while
  `CurrentIPTCDigest` (what the reader compares, §5.1) includes it. A JPEG is not affected. A
  second call with only `-IPTCDigest=new` on the written copy stores the right checksum, with the
  `ImageDataHash` unchanged. And when the file had no Photoshop block (`Photoshop:IPTCDigest`
  missing), creating it adds the warning `Non-standard format (undef) for IFD0 0x8649
  PhotoshopSettings`, so T84's "no more warnings" refuses it (D8).
- **What:**
  1. **`safe_write.rs`, the digest pass:** after ExifTool has written the copy (9.2) and before
     its snapshot (9.3), when the target is a `Tiff` or `Dng` and the plan's `args` hold
     `-IPTCDigest=new`, a second `execute` on the copy only: `-IPTCDigest=new
     -overwrite_original <copy>` (the copy is RawStride's own temporary file; the original is still
     only read, §9.2). ExifTool's answer must report one file updated and no warning (as
     `write_succeeded`, and no `Warning` line), otherwise `Stage::WriteCopy`, the copy deleted as
     for any failure. Never for a JPEG or a sidecar. Update the module comment (9.2) with the
     reason.
  2. **`verify.rs`, D8:** in `verify_copy`, one more warning (not error) is allowed when the copy
     has the key `Photoshop:IPTCDigest`, the original has not, the `file_type` is `TIFF` or
     `DNG`, and the plan's `args` hold `-IPTCDigest=new`. Everything else of the check stays as it
     is. Document it at the check with the measured warning text.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs`.
- **Tests to add:**
  - unit tests in `verify.rs`: one more warning passes with a new `Photoshop:IPTCDigest` in a
    `TIFF` and a `DNG` copy with `-IPTCDigest=new` in the plan; it is still `NewWarnings` for a
    `JPEG`, without `-IPTCDigest=new`, when the original already had `Photoshop:IPTCDigest`, with
    two more warnings, and with one more error;
  - unit test in `safe_write.rs` for the choice of the digest pass (`Tiff`/`Dng` with
    `-IPTCDigest=new` yes; `Jpeg`, `Raw` and a plan without it no);
  - `tests/safe_write.rs` (ExifTool required): a minimal TIFF built in the test (a header and one
    IFD with a 1 × 1 strip, enough for ExifTool to write it), first given IIM (City) and a
    digest with `ExifTool::execute` in two calls so the digest is current; then
    `write_safely` with a plan from `write_plan::plan` that changes the City (read with
    `read_metadata`, `state` from T87) → `Written`; `IPTCDigest` equals `CurrentIPTCDigest`
    afterwards and `read_metadata` shows `digest` `Matches`; the same TIFF without IIM and a plan
    with the setting "create IIM" → `Written` with a current digest (D8);
  - one test **ignored by default**: every TIFF and DNG in `PW_TESTDATA`, copied, with a City plan
    and `WriteSettings { create_iim: true }` through `write_safely` → `Written` and a current
    digest. Also run T87's ignored test `every_real_dng_jpeg_and_tiff_is_rated_safely` and report
    that `TN002683.tif` passes.
- **Done when:** `.\tools\check.ps1` is all OK; both ignored runs pass; `docs/test-inventory.md`
  is updated; `git diff --stat` shows only the files above.
