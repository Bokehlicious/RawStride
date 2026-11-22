# T263. ADR 0007 §9.6 Q17: the final read after the replace asks for `ImageDataHash` alone (pw-metadata) — Done

- **Why:** ADR 0007 revision 29 (§9.6 point 1, Q17; approved by the owner 2026-11-22). The final
  read after the replace (§9.4, `SafeWrite::final_check` in `safe_write.rs`) compares only the
  size and `ImageDataHash` of the written file with the verified copy's, but it takes a full
  snapshot (`ExifTool::snapshot`: `-all`, `-validate`, `-warning` as well). Asking for the hash
  alone makes the same check cheaper: about 80–110 ms per camera JPEG and about 17 ms per DNG
  (the analysis `docs/analyses/2026-11-21-metadata-write-time.md`, the table "What a snapshot of
  the original costs without parts of its arguments"). The check itself does not change: the
  same size, the same SHA-256 of the image data.
- **Crate:** `pw-metadata` (and an example run in `pw-imagecheck`, unchanged). **Files:**
  - `crates/pw-metadata/src/verify.rs` (point 1, its unit test)
  - `crates/pw-metadata/src/safe_write.rs` (points 2–3)
  - `crates/pw-metadata/tests/verify.rs` (the integration tests)
  - `docs/test-inventory.md` (the new tests, rule 12)
  - `docs/analyses/2026-11-21-metadata-write-time.md` (point 4)
  - this task's own file (status and log)
- **What:**
  1. **The narrow read.** In `verify.rs`, a new public method
     `ExifTool::image_data_hash(&mut self, file: &Path) -> Result<Option<String>>`: one ExifTool
     call with its own constant argument list (for example `IMAGE_HASH_ARGS`):
     `-j -G1 -api ImageHashType=SHA256 -ImageDataHash -Error` and the file name. The answer is parsed
     with the existing `json_object`; the value is the one under `File:ImageDataHash`
     (`IMAGE_DATA_HASH`) as `json_text` gives it, so it is the same text the snapshot gives for
     the same file; `None` when ExifTool gives none (an XMP sidecar). A file ExifTool cannot read
     is an error, as in `snapshot`: no output (a missing file) is the error `json_object` already
     returns. A file ExifTool cannot parse (a format error, an empty file) gives only
     `{"SourceFile": …}` on standard output when the hash alone is asked for, and the error on
     standard error, which `json_object` drops; that is why `-Error` is in the list: the object
     then holds `ExifTool:Error` (for example "File format error"), and that key is an error with
     ExifTool's text. (Measured with ExifTool 13.59 in the cold read, 2026-11-22.) A text file is
     not an ExifTool error (file type `TXT`, no hash): it gives `None`.
  2. **The final read uses it.** `SafeWrite::final_check` takes the size from
     `std::fs::metadata(self.target)` and the hash from `image_data_hash(self.target)`, and
     compares both with `written` as today. The error texts stay as they are ("the written file
     could not be read: …" for a failed read or metadata call, the two mismatch texts unchanged).
     Nothing else in the safe write changes: the snapshots of the original and of the copy (§9.1,
     §9.3) stay full snapshots.
  3. **Comments.** The module comment of `safe_write.rs`, the doc comment of `final_check`
     (`safe_write.rs`) and the doc comment of `ExifTool::snapshot` (`verify.rs`) say what the final
     read now asks for (ADR 0007 §9.6 point 1, Q17); `verify.rs`'s module comment gets one sentence
     on the new narrow read; no other comment is reworded.
  4. **The measurement.** After the change, run in a release build
     `cargo run --release -p pw-imagecheck --example write_timing -- <files>` (unchanged; its doc
     comment says how) on `TN006331.JPG`, `IMG_20261026_062851.dng` and `TN002683.tif` from
     `D:\PhotoWorkflowData\RAW`, work folder on C: (the default). Add a short section
     "After T263 (Q17)" to the analysis, after "Measured in T261" and before the appendix: the commit and build, a
     table of the three files with the `write_safely` median before (the "Measured in T261"
     table) and after, and the difference in ms. Mark the conclusion **inference** or
     **hypothesis** as the analysis does. If the saving is clearly smaller than the estimate
     above, say so; do not change the code to chase it.
- **Tests to add:**
  - `src/verify.rs` (unit): the narrow read's argument list asks for `-ImageDataHash` with
    `ImageHashType=SHA256` and `-Error`, and holds neither `-all` nor `-validate` nor `-warning`.
  - `tests/verify.rs` (real ExifTool, as the file's other tests): for the small JPEG,
    `image_data_hash` is `Some` and equal to `snapshot(...).image_data_hash`; for an XMP sidecar
    (made in the test, for example with `-o <name>.xmp` from the JPEG; the file's helpers
    `write_copy` and `take_snapshot` show the pattern), it equals the snapshot's (`None`); for a
    missing file it is an error; for a file of the bytes `00 01 02 03` named `.dng` it is an error
    holding ExifTool's text.
  - `tests/verify.rs`, ignored (`PW_TESTDATA`, as the file's real-DNG test, with its own filter
    for the extensions; it hashes the 177 MB TIFF too): for every JPEG, TIFF and DNG directly in
    `PW_TESTDATA`, `image_data_hash` equals the snapshot's `image_data_hash`,
    and is `Some`. This shows the final read compares the same value as before.
  - The existing safe-write tests (`src/safe_write.rs`, `tests/safe_write.rs`, `tests/edit.rs`
    and the rest) pass unchanged; none is rewritten.
- **Not in this task:**
  - Any change of the snapshot of the original or of the copy, the verification (§9.3), the
    digest pass, the TIFF restart or the image check (Q19: T264, T265).
  - Several ExifTool processes (Q18: T266–T268).
  - `write_timing.rs` (run only, not changed).
- **Rules:**
  - The final check proves exactly what it proves today: the written file's size and its
    SHA-256 image data hash equal the verified copy's. A missing hash on one side and not the
    other is still a mismatch.
  - `AGENTS.md` §3 (no `unwrap`/`expect` outside tests); no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK, the analysis has its "After T263 (Q17)" section,
  and `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Written after ADR 0007 revision 29 (§9.6 point 1, Q17), approved by the owner 2026-11-22; first of the order Q17, Q19's measurement (T264, T265), Q18 (T266 → T267 → T268) |
| 2026-11-22 | Open | Cold read: the narrow read asks for `-Error` too (a file ExifTool cannot parse gives no error key otherwise, and would show as a hash mismatch, not a failed read); a text file is not an error; the DNG saving corrected to about 17 ms; point 3 names `verify.rs` for `snapshot` and its module comment; the test helpers and the real-file filter named; where the analysis section goes. No other file needs to change |
| 2026-11-22 | Ready for review | Points 1–4 done. `ExifTool::image_data_hash` (`IMAGE_HASH_ARGS`, `ExifTool:Error` an error) in `verify.rs`; `final_check` takes the size from `std::fs::metadata` and the hash from it, error texts unchanged; comments per point 3. Tests: 2 unit (the argument list as asked; besides it, the answer parsed: hash, none, `ExifTool:Error`), 2 integration in `tests/verify.rs` (1 ignored: 26 real files, all equal). Measured (analysis "After T263 (Q17)"): `write_safely` −32 ms JPEG, −13 ms DNG, −25 ms TIFF; the JPEG saving clearly smaller than the 80–110 ms estimate, the TIFF's within its run spread. check.ps1 all OK (2253 passed, 77 ignored), no clippy warnings; the ignored tests of `tests/verify.rs`, `tests/safe_write.rs`, `tests/edit.rs` and `tests/level_c.rs` run with `PW_TESTDATA` and passed |
| 2026-11-22 | Done | Reviewed: points 1–4 as the brief says; the check unchanged (size from the file system, the same SHA-256 hash; `-n`, `-a` and the other snapshot arguments do not change the hash text, and the ignored test showed it equal on all 26 real files); error texts unchanged; the analysis's figures checked against its table. Reviewer re-ran check.ps1 (all OK, 2253 passed, 77 ignored, no clippy warnings) and the new ignored test with `PW_TESTDATA` (passed). No reviewer changes. No hand check (no visible change). T264 set to Open |
