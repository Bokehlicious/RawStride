# T22. pw-ingest: tidy the import API before the egui dialog uses it — Done

- **Why:** T20 moved the logic correctly, but the egui dialog (ADR 0010 M4) will call these functions
  from Rust, so the API should be hard to misuse, and a few required tests are missing.
- **Crates:** `pw-ingest`, `photoworkflow`. **Files:** `crates/pw-ingest/src/volumes.rs`,
  `crates/pw-ingest/src/request.rs`, `crates/pw-ingest/src/lib.rs`, `src-tauri/src/ingest.rs`.
- **What:**
  1. **Card detection:** rename `pw_ingest::ingest_volumes()` to `card_volumes()` and return
     `Vec<PathBuf>`. Put the loop over candidate roots in a small function that takes the roots, e.g.
     `fn with_dcim(roots: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf>`, and test it with
     temporary folders (a root with `DCIM` is found, one without is not, a missing root is skipped).
     The Tauri command `ingest_volumes()` keeps its name and converts the paths to strings.
  2. **Validation input:** replace the seven positional arguments of `validate_options` with a plain
     struct, e.g. `pub struct OptionsInput<'a> { source, dest, backup: Option<&'a str>, folder, name,
     job, first_seq }`, so source and destination cannot be swapped by mistake. Messages unchanged.
  3. **Missing tests** from T20: empty destination ("Välj ett mål."); source inside the destination;
     a bad folder template starts with "Mappmall: " and a bad file-name template with
     "Filnamnsmall: "; a valid input with a backup folder.
- **Rules:** behaviour and messages unchanged; no new dependencies; nothing else in `src-tauri` changes.
- **Done when:** `.\tools\check.ps1` is all OK and the new tests pass.
