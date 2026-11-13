# T48. pw-metadata: a write that ExifTool refused is reported as saved — Done

- **Added by the agent at the owner's request (2026-11-09), found in the T47 hand check.** For a Claude
  agent.
- **Crate:** `pw-metadata`. **Files:** `crates/pw-metadata/src/lib.rs`,
  `crates/pw-metadata/tests/sidecar.rs`. No new dependencies.
- **What happens:** with a read-only `.xmp`, a rating can be set in `pw-loupe` without any message;
  the file is not changed. ExifTool refuses the write and answers
  ```
  Error renaming temporary file to …/test.xmp
      0 image files updated
      1 files weren't updated due to errors
  ```
  (a read-only JPEG gives `Error opening … for writing` and the same two lines). `write_tags` treats
  every answer that contains the text `files updated` as success, so `0 image files updated` counts
  as saved. `write_rating` and `write_iptc` both go through `write_tags`.
- **What:**
  1. Add a small pure function in `lib.rs`, e.g. `fn write_succeeded(out: &str) -> bool`, that reads
     ExifTool's summary lines (`<n> image files created|updated|unchanged`,
     `<n> files weren't … due to errors`) as numbers: success only when at least one file was
     created, updated or unchanged and no file failed.
  2. `write_tags` uses it instead of the three `contains` checks. A failure still returns
     `Error::ExifTool(out.trim())` as today (the message text is unchanged).
  3. Nothing else changes; `pw-loupe` already shows the error and rolls the rating back when the
     write fails (T42, T43).
- **Tests to add:**
  - unit tests for `write_succeeded`: created, updated, unchanged (success); the read-only answer
    above, `0 image files updated` alone and an empty answer (failure);
  - in `tests/sidecar.rs` (real ExifTool): a rating written to an ARW whose sidecar is read-only
    returns an error, and the sidecar keeps its old rating; the same for a read-only JPEG (embedded).
    Restore the permissions afterwards so the temporary folder can be removed.
- **Done when:** `.\tools\check.ps1` is all OK. Then the owner checks by hand: rate an image with a
  read-only sidecar → the error message and the old rating; at exit with such a write pending, the
  message box "Betyg som inte sparades".
