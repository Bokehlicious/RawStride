# T30. ADR 0010 M4a: import form and preview summary (plain Rust) — Done

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add `pw-ingest.workspace = true` to
  `[dependencies]`; internal, approved by ADR 0010 §6), a new `crates/pw-loupe/src/import.rs`, declared
  and re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`. Do **not** add `rfd` yet (M5).
- **Why:** the egui import dialog (step A of ADR 0010) needs the same rules and messages as the Tauri
  dialog. They come from `pw-ingest`; this task wraps them in a small, testable form model.
- **What:**
  1. `pub struct ImportForm { pub source: String, pub dest: String, pub folder: String, pub name: String }`
     with `Default`: empty source and destination, folder `{year}/{date} {job}`, name `{name}` (ADR 0005
     choice 1, the Tauri defaults). Step B fields (backup, job, running number) are not in the form;
     validation passes backup `None`, job `""` and first number `1`.
  2. `pub enum Field { Source, Dest, Folder, Name }` and
     `pub struct FieldError { pub field: Field, pub message: String }`.
  3. `ImportForm::validate(&self) -> Result<Validated, FieldError>` where
     `pub struct Validated { pub source: PathBuf, pub options: pw_ingest::Options }`. Decide the field
     **by which check fails, never by reading the message text**:
     - `pw_ingest::validate_source(&self.source)` fails → `Field::Source`;
     - `pw_ingest::Template::parse(&self.folder)` fails → `Field::Folder`, message
       `"Mappmall: {error}"`;
     - `pw_ingest::Template::parse(&self.name)` fails → `Field::Name`, message `"Filnamnsmall: {error}"`;
     - then `pw_ingest::validate_options(OptionsInput { … })`; any error left → `Field::Dest`.
     The messages are exactly those of `pw-ingest`.
  4. `pub struct Preview` built by `Preview::new(shots: &[pw_ingest::Shot], plan: &pw_ingest::Plan)`,
     with the same numbers as the Tauri `preview_of` in `src-tauri/src/ingest.rs`: `shots`,
     `files_to_copy`, `bytes_to_copy`, `present`, `renamed` (renamed for a conflict **and** to be
     copied), `no_exif`, `items_total`, and `lines`: the first 20 plan items as
     `PreviewLine { source_name: String, rel: String, action: ItemAction }`, with
     `pub enum ItemAction { Copy, Renamed, Present, BackupOnly }` following the Tauri `action()` rules.
     (Do not change `src-tauri`.)
  5. `pub fn result_folder(copied: usize, folders: &[(String, usize)]) -> Option<PathBuf>`: the folder
     the import opens in a new tab (GR12, ADR 0010 Q2): `None` when nothing was copied, otherwise the
     first entry of `folders` (the output of `pw_ingest::folder_summary`, largest first).
- **Rules:** no threads, no file writes; no `unwrap`/`expect`/`panic!` in non-test code; no new external
  crates.
- **Tests to add:** the defaults; an empty source → `Source` with "Välj en källa."; an empty destination
  → `Dest` with "Välj ett mål."; a destination inside the source → `Dest`; a bad folder template →
  `Folder` starting with "Mappmall: "; a bad name template → `Name` starting with "Filnamnsmall: "; a
  valid form (temporary source folder) → `Ok` with the destination and templates; `Preview::new` on a
  hand-made plan (items for each action, a renamed item that is not copied, a shot without EXIF time,
  more than 20 items); `result_folder` for 0 copied, and for several folders.
- **Done when:** `.\tools\check.ps1` is all OK.
