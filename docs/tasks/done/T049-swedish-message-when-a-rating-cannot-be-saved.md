# T49. Swedish message when a rating cannot be saved — Done

- **Added by the agent at the owner's request (2026-11-09), found in the T48 hand check.** For a Claude
  agent. **Start after T48.**
- **Crates:** `pw-metadata`, `pw-loupe`. **Files:** `crates/pw-metadata/src/lib.rs`,
  `crates/pw-metadata/src/worker.rs`, `crates/pw-metadata/tests/sidecar.rs`,
  `crates/pw-loupe/src/metadata.rs`. No new dependencies.
- **What happens:** since T48 a rating on an image with a read-only sidecar is refused and rolled
  back, but the message shows ExifTool's English answer:
  `Kunde inte spara betyget för TN005979.ARW: ExifTool: 0 image files updated … Error renaming
  temporary file …`.
- **Owner's decision (2026-11-09):** a read-only file gives
  `Kunde inte spara betyget för <fil>: filen är skrivskyddad`; any other write that ExifTool refuses
  gives `Kunde inte spara betyget för <fil>: filen kunde inte skrivas`. The Swedish texts live in
  `pw-loupe`; `pw-metadata` reports the kind of error, and `pw-loupe` never compares error text (as T3).
- **What:**
  1. `pw-metadata` `Error`: two new variants, `ReadOnly(PathBuf)` and `NotWritten(String)` (ExifTool's
     answer). When `write_succeeded` is false, `write_tags` returns `ReadOnly(target)` if the target
     file exists and is read-only, otherwise `NotWritten(answer)`. ExifTool still decides whether
     the write failed; the read-only check only names the reason. Put the choice in a small function
     that can be tested with a temporary file.
  2. `pw-metadata`: a public `WriteError` (`ReadOnly`, `NotWritten(String)`, `Other(String)`) with
     `From<Error>` and `Display`; `WorkerEvent::RatingWritten` carries `Result<(), WriteError>`
     instead of `Result<(), String>`. `Other` holds today's text for everything else (ExifTool could
     not start, crashed, I/O), so those messages are unchanged.
  3. `pw-loupe` `metadata.rs`: turn the `WriteError` into the text for the rating store with a small
     function: `ReadOnly` → "filen är skrivskyddad", `NotWritten` → "filen kunde inte skrivas",
     `Other(text)` → `text`. `RatingStore` and its tests are not changed.
  4. Test helpers that build or read `RatingWritten` (`FakeSubmit` in `pw-loupe/src/metadata.rs`,
     `write_result` in `pw-metadata/src/worker.rs`) convert between `String` and `WriteError::Other`;
     their assertions stay as they are. T48's test in `tests/sidecar.rs` now expects
     `Error::ReadOnly` for the read-only sidecar and JPEG (the old rating is still kept).
- **Tests to add:** the choice in point 1 (read-only file → `ReadOnly`, writable file → `NotWritten`,
  missing file → `NotWritten`); `From<Error>` for the three kinds; the text function for the three
  kinds; a `RatingWritten` with `ReadOnly` gives "Kunde inte spara betyget för <fil>: filen är
  skrivskyddad" through `MetadataCache::poll`.
- **Not in this task:** messages for ExifTool missing or crashing stay as today (English); the Tauri
  app shows the new English `Display` text of the two variants.
- **Done when:** `.\tools\check.ps1` is all OK. Then the owner checks by hand: rate an image with a
  read-only sidecar → "Kunde inte spara betyget för <fil>: filen är skrivskyddad" and the old rating.
