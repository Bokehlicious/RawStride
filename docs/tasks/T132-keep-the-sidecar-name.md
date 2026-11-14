# T132. A write keeps the sidecar's name as it is on disk — Open

- **Can start now.** Found in the review of T127 (2026-11-14); the owner approved a task for it.
  Not at the same time as another task that changes `safe_write.rs` or `read.rs`.
- **Crates:** `pw-metadata` only. **Files:** `crates/pw-metadata/src/safe_write.rs` (the target
  of a manufacturer RAW); `crates/pw-metadata/src/read.rs` (only to make `find_sidecars`, or a
  small function over it, usable from `safe_write.rs` within the crate: `pub(crate)`, no change
  to what it finds); `crates/pw-metadata/tests/sidecar.rs` (the test); `docs/test-inventory.md`.
- **Why:** Photo Mechanic names a sidecar `<basename>.XMP` with an upper-case extension (level-C
  sample `PM-raw.XMP`, ADR 0007 §4.2). The reader finds it, because it looks up the name without
  regard to case and keeps the name as stored (`find_sidecars`, `sidecar_as_stored`). But the safe
  write takes its target from `sidecar_path(image)`, which always gives a lower-case `.xmp`; the
  replace (`ReplaceFileW`) then leaves the file named `PM-raw.xmp`. It is still one file, and
  Windows does not care, but RawStride changes a file name nobody asked it to change, and on a
  file system that compares names with regard to case (a NAS share, a copy to macOS or Linux)
  another program may no longer find it under the name it wrote.
- **What:**
  1. **The target of a manufacturer RAW** in the safe write (`safe_write.rs`, where `kind ==
     FileKind::Raw` today gives `sidecar_path(image)`): when the image has a `<basename>.xmp`
     sidecar in any case (`find_sidecars`' `plain`), the target is that file **as stored**
     (`PM-raw.XMP`); only when there is none is it `sidecar_path(image)` (a new sidecar is still
     created as `<basename>.xmp`, ADR 0007 §4.2). A `<basename>.<ext>.xmp` is never the target
     (Q8, unchanged).
  2. The name as stored is used for the whole transaction: the journal entry, the temporary copy
     and the backup next to it, the verification and the `Written { target }` of the report.
  3. A failure to look up the sidecar (a folder that cannot be read) is a `Precheck` error with
     the original `Intact`, as other precheck failures; it is never a reason to fall back to
     `sidecar_path` silently.
  4. `sidecar_path` and `rating_file` stay as they are (other callers use them only to name or
     find the file, which works in any case on Windows).
- **Not in this task:** the reader (it already finds the sidecar in any case); the name of a new
  sidecar; `<basename>.<ext>.xmp`; JPEG, TIFF and DNG (written in place).
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add** (`tests/sidecar.rs`, Windows, with ExifTool, as the other tests there): a fake
  RAW `TN0001.ARW` with a sidecar `TN0001.XMP` (upper case, written by the test with ExifTool or as
  a small XMP packet); an edit through `write_edits_safely` (a Headline): afterwards the folder
  listing holds exactly `TN0001.ARW` and `TN0001.XMP` (byte-exact names, no `TN0001.xmp`), the
  Headline is read back, the report's target ends in `TN0001.XMP`, no `.pwtmp-` or `.pwbak-` file
  is left and the journal is empty; the same with a mixed case `TN0001.Xmp`; and a RAW without a
  sidecar gets a new `TN0001.xmp` (unchanged behaviour).
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated, and
  `git diff --stat` shows only the files above. The reviewer then runs the level-C tests: after
  the write the sample copy's sidecar is still named `PM-raw.XMP`.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Written by the reviewer after the review of T127 (a write renamed `PM-raw.XMP` to `PM-raw.xmp`); owner approved a task for it |
