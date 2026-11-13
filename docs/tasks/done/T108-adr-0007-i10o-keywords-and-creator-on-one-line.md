# T108. ADR 0007 I10o: keywords and creator on one line, the reason of a refused write (owner's check of T107) — Done

- **Can start now** (T106 and T107 are committed). Nothing else is open in these files.
- **Crates:** `pw-loupe`, `pw-metadata`.
- **Files:** `crates/pw-loupe/src/editor.rs`; `crates/pw-loupe/src/metadata.rs` (`write_error_text`
  and its tests only); `crates/pw-metadata/src/lib.rs` (`WriteError` and `From<EditError>`);
  `crates/pw-metadata/src/safe_write.rs` and `crates/pw-metadata/src/fileops.rs` (only if the
  refusal must be carried as a kind, point 2); `crates/pw-metadata/tests/edit.rs` (the new test);
  `docs/test-inventory.md`.
- **Why:** the owner's hand check of T107 on 2026-11-12:
  - Nyckelord shows one keyword per line; the owner wants them on one line separated by commas
    ("fågel, fåglar, test, inte bra, slaguggla"), as in other photo programs. Fotograf shall
    likewise be one line, like E-post.
  - A write to a JPEG that Photo Mechanic held open failed with "Kunde inte spara metadata för
    konflikt-stad.jpg: metadata kunde inte skrivas säkert, filen är oförändrad". The safe write
    refused rightly at its precheck (`Refusal::Locked`, "the file is open in another program"),
    but the message drops the reason, so the user cannot know what to do.
- **What:**
  1. **One line with commas** (`editor.rs`): `EditorField::Keywords` and `EditorField::Creator`
     are no longer `multiline()`. A list value is shown as its entries joined with `", "`; the
     text of a list field is split at `,`, each part trimmed, empty parts dropped (`"sol, hav ,,
     "` gives `["sol", "hav"]`; only spaces and commas is `Clear`). A keyword or name that
     itself holds a comma cannot be entered (accepted by the owner's choice; say in the
     hand-over if the read shows one, which is then split when the field is changed). The
     comparison with the read text (`start_text`) uses the same joining, so an unchanged field
     gives no edit.
  2. **The reason of a refused write:** a safe write refused at its precheck because another
     program holds the file open gives `WriteError::Locked` (new variant, as `ReadOnly` is for a
     read-only file), decided from the refusal's kind, **not** by comparing the English reason
     text (carry the kind on `SafeWriteError` or check with `fileops` as `From<EditError>` does
     for read-only; say which in the hand-over). It covers rating writes and editor writes alike.
     `write_error_text` gives "filen är öppen i ett annat program", so the window shows "Kunde
     inte spara metadata för <fil>: filen är öppen i ett annat program" and a rating "Kunde inte
     spara betyget för <fil>: filen är öppen i ett annat program". Every other refusal keeps its
     text.
- **Rules:** rules 3–7 and 10–13. No `Cargo.toml` change (holding a file open in the test uses
  `std::os::windows::fs::OpenOptionsExt::share_mode`).
- **Tests to add / change:** `editor.rs`: the text of a read list (`"sol, hav"`), splitting with
  spaces and empty parts, only commas as `Clear`, an unchanged list gives no edit, `multiline()`
  false for both fields (the existing tests that use `"\n"` rewritten; say which);
  `metadata.rs`: `write_error_text(WriteError::Locked)`; `pw-metadata`: `From<EditError>` gives
  `Locked` for the locked refusal and `NotSafe` for another precheck refusal, and an integration
  test in `tests/edit.rs` (ExifTool required, Windows) that writes edits to a JPEG held open
  without delete sharing and gets `Locked` with the file unchanged.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: Nyckelord
  and Fotograf are one line each with commas; keywords typed with commas are read back as
  separate keywords (ExifTool or Photo Mechanic); `OK` on an image open in Photo Mechanic shows
  "… filen är öppen i ett annat program"; and the conflict step of T107 (the reviewer recreates
  the conflict file).
