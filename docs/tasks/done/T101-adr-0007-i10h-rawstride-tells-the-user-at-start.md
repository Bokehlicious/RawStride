# T101. ADR 0007 I10h: RawStride tells the user at start when ExifTool is missing or too old — Done

- **Can start after T94 and T100 are accepted** (D9 approved on 2026-11-12). Not at the same time as T103,
  T105 or T106 (`main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/import_worker.rs`;
  `crates/pw-loupe/src/metadata.rs` (`write_error_text` only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §11 (checked at start) and D9. T94 gives the result; nothing shows it yet, and
  the import worker starts its own ExifTool for the template.
- **What:**
  1. **`main.rs`:** each frame until it has been shown, `metadata_worker.exiftool_version()`;
     when it is `Some`, at most one message window (as the recovery message, T91):
     - `Err(text)`: title "ExifTool saknas", text "ExifTool kunde inte startas: <text>. Betyg och
       metadata kan inte läsas eller sparas.";
     - `Ok(v)` below `MIN_EXIFTOOL_VERSION`: title "ExifTool är för gammal", text "ExifTool <v> är
       för gammal. RawStride behöver version 12.67 eller senare. Betyg och metadata sparas inte
       förrän ExifTool har uppdaterats.";
     - `Ok(v)` at least 12.67: nothing.
     A pure function `exiftool_message(&Result<ToolVersion, String>) -> Option<(&str, String)>`
     makes the title and text, so it can be tested.
  2. **`import_worker.rs`:** after the import worker has started its ExifTool for the template, it
     asks `version()` once; below 12.67 or unreadable, the template is not written and one line
     `"<file>: ExifTool <v> är för gammal (minst 12.67)"` (or the error text) is listed, as a
     failed start is today; the copied files are kept.
  3. **`metadata.rs`:** the rating writes refused by T94 (`WriteError::Other` holding
     `EXIFTOOL_TOO_OLD`) get the Swedish reason "ExifTool är för gammal (minst 12.67)" in
     `write_error_text`, as `NO_JOURNAL` gets its text today.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:** `exiftool_message` for an error, 12.40, 12.67 and 13.59; the
  `write_error_text` of `EXIFTOOL_TOO_OLD`; the import worker with
  a fake ExifTool program that answers `-ver` with `12.40` (as the existing fake-program tests of
  the import worker, or the `test_support` pattern) lists the line and writes nothing.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
