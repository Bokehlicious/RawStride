# T98. ADR 0007 I10e: the metadata worker reads all metadata, writes edits and takes new settings — Done

- **Can start after T94 and T97 are accepted** (both change what this task builds on;
  `worker.rs` is T94's). No owner decision of its own.
- **Crates:** `pw-metadata`, and in `pw-loupe` only the arms that ignore the new events.
- **Files:** `crates/pw-metadata/src/worker.rs`; `crates/pw-loupe/src/metadata.rs` (the match in
  `MetadataCache::apply` only: the two new events are ignored until T103/T105);
  `docs/test-inventory.md`.
- **Why:** the info panel (T103) needs the whole `MetadataRead` of the image shown, and the
  editor (T105) writes through the worker, which owns the ExifTool process and the write setup
  (G2: the UI only polls). The settings screen (T106) changes the `EditOptions` while RawStride
  runs.
- **What** (`worker.rs`):
  1. `MetadataWorker::read_metadata(&mut self, path: PathBuf) -> Result<RequestId, SubmitError>` →
     `Event::MetadataRead { id, path, result: std::result::Result<MetadataRead, String> }`,
     read with the current options' `fallback` (`Fallback::default()` without a setup).
  2. `MetadataWorker::write_edits(&mut self, path: PathBuf, edits: Vec<Edit>) ->
     Result<RequestId, SubmitError>` → `Event::EditsWritten { id, path, result:
     std::result::Result<EditReport, WriteError> }` through `write_edits_safely` with the setup;
     without a setup `WriteError::Other(NO_JOURNAL)`, with a too old ExifTool
     `EXIFTOOL_TOO_OLD` (T94); errors through `From<EditError>` as for ratings.
  3. `MetadataWorker::set_options(&mut self, options: EditOptions) -> Result<(), SubmitError>`:
     a request without an event; every request after it uses the new options (rating writes,
     edits and reads). Requests run in order, so a write queued before it uses the old options.
  4. Module comment updated.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. The existing requests and events are
  unchanged.
- **Tests to add** (`worker.rs`; ExifTool required where a real file is written, Windows as the
  existing write tests):
  - `read_metadata` of a JPEG with a Headline → `MetadataRead` holding it; of a missing file →
    `Err`;
  - `write_edits` of a Headline → `EditsWritten` with `Ok` and `Written`, read back with
    `read_metadata`; without a setup → `NO_JOURNAL`, the file unchanged;
  - `set_options` with `create_iim: true`, then `write_edits` of a City on a JPEG without IIM →
    the City is in IIM afterwards; the same without `set_options` → no IIM;
  - `set_options` with `Fallback::Latin1` changes how a JPEG with IIM 0x80 in City and no 1:90 is
    read (`€` with Windows-1252, U+0080 with Latin-1).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
