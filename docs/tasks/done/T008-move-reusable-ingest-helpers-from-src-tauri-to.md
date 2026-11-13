# T8. Move reusable ingest helpers from src-tauri to pw-ingest — Done

- **Why:** the Tauri app is being replaced by the egui GUI (ADR 0001 amended 2026-11-07, ADR 0009).
  Logic that the egui GUI will also need belongs in the shared crate `pw-ingest`, not in
  `src-tauri`. `src-tauri` only keeps the Tauri glue and calls the shared functions (GR8: the
  Tauri app keeps working).
- **Crates:** `pw-ingest`, `photoworkflow`.
- **Files:** `crates/pw-ingest/src/lib.rs` (or a new module file in `crates/pw-ingest/src/`, e.g.
  `paths.rs`), `src-tauri/src/ingest.rs`.
- **What:** move these functions from `src-tauri/src/ingest.rs` to `pw-ingest` and make them `pub`:
  - `inside(a, b)` and `resolve(p, what)` (path overlap checks);
  - `drive_of(path)` (drive letter of a path);
  - `folder_summary(items, dest)` (folder counts for the summary).
- **Rules:**
  - **Behaviour unchanged:** same signatures, same results, same (Swedish) error messages. This is
    a move, not a rewrite.
  - Move their tests with them (`inside_…`, `resolve_…`, `drive_letter_…`, `folder_summary_…`).
    The tests keep their names and assertions.
  - `src-tauri/src/ingest.rs` calls `pw_ingest::…` instead of its own copies, and the copies are
    deleted. Nothing else in `src-tauri` changes.
  - Add a short `///` doc comment to each moved function.
  - No new dependencies.
- **Done when:** `cargo test -p pw-ingest` and `cargo test -p photoworkflow` pass; the total number
  of tests in the two crates is the same as before the move; `cargo clippy` shows no warnings.
