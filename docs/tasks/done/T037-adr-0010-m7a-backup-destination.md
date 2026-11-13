# T37. ADR 0010 M7a: backup destination — Done

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/main.rs`.
- **What (ADR 0010 §3.2; the Tauri dialog's "Backup" checkbox and field):**
  1. `ImportForm` gets `backup_on: bool` (default `false`) and `backup: String` (default empty).
     `Field` gets `Backup`.
  2. `validate()`: when `backup_on` is false, the backup is `None` as today. When it is true, pass the
     trimmed `backup` (an empty field counts as no backup, as in `pw-ingest`). To put an error on the
     right field **without reading the message text**: first call `validate_options` **without** the
     backup; an error there is `Field::Dest` (as today); then call it **with** the backup; an error that
     only appears now is `Field::Backup`. Messages unchanged ("Backupmålet …").
  3. Dialog (`main.rs`): under "Mål", a checkbox "Backup" and, on the same row, a text field and
     "Välj…" (title "Välj backupmål", starting in the typed folder), both disabled while the checkbox is
     off; the `Backup` field error under it. Disabled while running, like the other fields.
- **Rules:** the copy itself already handles `Options::backup` (`pw-ingest`); nothing changes in the
  worker; rules 6 and 7.
- **Tests to add** (`import.rs`): backup off → `None` even with text in the field; backup on with a
  folder → `Some`; backup on with an empty field → `None`; backup inside the source → `Field::Backup`;
  backup equal to the destination → `Field::Backup` with "Backupmålet måste vara en annan mapp än målet.";
  a bad destination together with a bad backup → `Field::Dest` (the destination is reported first).
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: an import with backup
  writes the same files to both folders; the preview shows "(bara backup)" for files that only the
  backup lacks.
