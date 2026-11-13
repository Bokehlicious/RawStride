# T106. ADR 0007 I10m: the settings window (create IIM, fallback encoding) — Done

- **Can start after T98, T99 and T101 are accepted.** Not at the same time as T103 or T105
  (`main.rs`); the reviewer decides the order of T103, T105 and T106.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/import_worker.rs`;
  `crates/pw-loupe/src/metadata.rs` (only if the panel's read must be renewed after a change);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 (the setting "Skapa IPTC-IIM för kompatibilitet med äldre system", off by
  default) and §5.4/Q10 (the fallback encoding, changeable in the settings).
- **What:**
  1. **At start** (`run`): `load_settings(settings_path())` (T99), each problem written with
     `eprintln!`; the settings' `edit_options()` go into the metadata worker's `WriteSetup` and
     into each import (point 3) instead of `EditOptions::default()`.
  2. **The window "Inställningar"**, opened with a button "Inställningar" in the top bar (next to
     the import button): a check box with the exact text "Skapa IPTC-IIM för kompatibilitet med
     äldre system" and under it, in small text, "Gäller JPEG, TIFF och DNG som saknar IIM. Av som
     standard."; a drop-down "Teckenkodning för äldre IPTC- och Exif-text" with "Windows-1252
     (västeuropeisk)" and "ISO 8859-1 (Latin-1)"; the buttons "Spara" and "Avbryt". "Spara" saves
     the file, sends `set_options` to the metadata worker (T98) and empties the panel's cache so
     the image is read again with the new encoding; when the file cannot be saved, the settings
     still apply until RawStride closes and the message "Inställningarna kunde inte sparas: <fel>"
     is shown.
  3. **Import:** `ImportWorker::run` takes the `EditOptions` of the settings for that import (a
     new parameter, or a field next to the form), used by `apply_template_safely`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:** `import_worker.rs` (ExifTool required, as the existing IPTC import test): an
  import with `create_iim: true` of the JPEG fixture (no IIM) and a Creator gives IIM By-line in
  the copy, with the default options no IIM; a pure function in `main.rs` (or `settings.rs` if
  you need it, then say so) for the two encoding texts and back.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand: the setting survives a
  restart, and with "Skapa IPTC-IIM" on, a Title written in the editor to a JPEG without IIM
  appears in IIM (ExifTool `-IPTC:all`).
