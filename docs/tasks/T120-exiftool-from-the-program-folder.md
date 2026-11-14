# T120. ExifTool from the program folder first — Open

- **Not at the same time as T118** (both change `crates/pw-metadata/src/worker.rs`).
- **Why:** ExifTool ships with RawStride, unmodified, in the program folder (ADR 0002 §2.5, owner
  2026-11-13). Today `pw-metadata` starts `exiftool` from PATH only, so a shipped copy would not be
  found. A developer build and a user's own installation on PATH must keep working.
- **Crate:** `pw-metadata`. **Files:** `crates/pw-metadata/src/exiftool.rs`,
  `crates/pw-metadata/src/worker.rs`, `docs/test-inventory.md`.
- **What:**
  1. In `exiftool.rs`, a public function that gives the ExifTool program to start:
     - if the folder of the running program (`std::env::current_exe()`, its parent) holds a file
       `exiftool.exe`, the full path of that file;
     - otherwise `"exiftool"` (found on PATH by Windows, as today);
     - if the running program's path cannot be found, `"exiftool"`.
     Put the choice in a private function that takes the folder as an argument
     (`Option<&Path>`), so it can be tested without moving the test program.
  2. `ExifTool::start` and `MetadataWorker::start` use that function instead of the literal
     `"exiftool"`. `start_with` (tests, `PW_EXIFTOOL_PL`) and `restart` keep starting exactly the
     program they were given.
  3. The error when ExifTool cannot be started names where it was looked for: replace
     "(is ExifTool on PATH?)" with a text that says neither the program folder nor PATH had it, and
     names the program that was tried. Lower case, no full stop (it is an error's `Display` text,
     T118 point 2).
  4. Only `exiftool.exe` itself is looked for; its `exiftool_files` folder belongs next to it, as in
     the Windows package. Nothing else is checked (no version check here: ADR 0007 D9 already
     refuses writes with an ExifTool older than 12.67).
- **Tests to add:**
  - the private choice: a temporary folder with an `exiftool.exe` file → that path; a temporary
    folder without it → `"exiftool"`; no folder → `"exiftool"`; a folder that holds a
    *folder* named `exiftool.exe` → `"exiftool"`;
  - the error text of a program that cannot be started names that program and both places.
- **Not in this task:** packing ExifTool into a distribution folder (ADR 0002 §4, a RawStride
  distribution folder is still to be made); the Tauri app's packaging (`tools/package-dist.ps1`);
  user-visible texts in `pw-loupe` about a missing ExifTool.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-13 | Open | Added (owner decision: ExifTool ships with RawStride, ADR 0002 §2.5) |
