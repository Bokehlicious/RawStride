# T20. ADR 0010 M1: move card detection and request validation to pw-ingest — Done

- **Why:** the egui import (ADR 0010) must give the same answers as the Tauri dialog. The last two
  pieces of import logic that still live in `src-tauri` move to the shared crate first (ADR 0010
  §4.3).
- **Crates:** `pw-ingest`, `photoworkflow`.
- **Files:** `crates/pw-ingest/src/` (new module files, e.g. `volumes.rs` and `request.rs`, plus
  `lib.rs`), `src-tauri/src/ingest.rs`.
- **What:**
  1. **Card detection.** Move the body of `ingest_volumes()` to `pw-ingest` as
     `pub fn card_volumes() -> Vec<PathBuf>`. Put the check itself in a small function that takes the
     candidate roots, e.g. `fn with_dcim(roots: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf>`,
     so it can be tested with temporary folders. `card_volumes()` passes `A:\` … `Z:\` on Windows and
     keeps today's two-level walk of `/Volumes`, `/media`, `/run/media` elsewhere. The Tauri command
     `ingest_volumes()` stays and only converts the paths to strings.
  2. **Request validation.** Move the logic of `IngestRequest::source()` and `IngestRequest::options()`
     to `pw-ingest` as plain functions on a plain input struct, e.g.
     `pub struct OptionsInput { source, dest, backup: Option<String>, folder, name, job, first_seq }`
     and `pub fn validate_source(source: &str) -> Result<PathBuf, String>` /
     `pub fn build_options(input: &OptionsInput) -> Result<Options, String>`.
     `IngestRequest` in `src-tauri` keeps its serde fields and calls these.
- **Rules:**
  - **Behaviour unchanged:** the same checks in the same order and **the same Swedish messages**
    (including the "Mappmall: " and "Filnamnsmall: " prefixes).
  - No serde or Tauri types in `pw-ingest`; no new dependencies.
  - Nothing else in `src-tauri` changes; `ui/web` is not touched.
- **Tests to add (in `pw-ingest`):**
  - `with_dcim`: a temporary root with `DCIM` is found, one without is not, a missing root is skipped;
  - `validate_source`: empty, relative, missing folder, valid folder;
  - `build_options`: empty destination; destination inside the source; source inside the destination;
    backup equal to the destination; a bad folder template and a bad file-name template (check the
    prefix of the message); a valid input with and without backup.
- **Done when:** `.\tools\check.ps1` is all OK, the Tauri app's test count is unchanged, and the new
  tests pass.
