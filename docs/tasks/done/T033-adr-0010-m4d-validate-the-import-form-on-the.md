# T33. ADR 0010 M4d: validate the import form on the worker thread (G2) — Done

- **Start after T32 is committed.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import_worker.rs`,
  `crates/pw-loupe/src/import_dialog.rs`, `crates/pw-loupe/src/lib.rs` (re-exports only). **Not**
  `main.rs`, not `import.rs`.
- **Why (found in the T32 review):** `ImportForm::validate` checks that the source exists and
  canonicalises the source and destination (`pw_ingest::resolve`). That is file I/O. T32 calls it in
  `tick()` and `start()`, which run on the UI thread. On a sleeping hard disk (the F:, G: and H:
  destinations), a card reader without a card or a network folder this can block for seconds, and the
  window freezes while the user types. ADR 0010 §5 and G2: no file I/O on the UI thread. The Tauri app
  validates inside its background task too.
- **What:**
  1. **Worker:** `ImportWorker::preview(form: ImportForm, rescan: bool)` and `ImportWorker::run(form:
     ImportForm)` take the form instead of `source` and `options`; the worker thread calls
     `form.validate()` first.
     - `ImportEvent::Previewed { id, result: Result<Preview, FieldError> }`: a validation error is
       returned as it is; a scan error becomes `Field::Source` with the existing message
       (`"Kunde inte läsa {source}: {error}"`).
     - `run` with an invalid form ends with `ImportEvent::Done` whose `error` is the field error's
       message and nothing copied.
     - Everything else (scan cache, fresh scan for a run, progress, cancel, the gate, drop) is unchanged.
  2. **Dialog:** `DialogRequest::Preview { form: ImportForm, rescan: bool }` and
     `DialogRequest::Run { form: ImportForm }`.
     - `tick()` no longer validates: when the form changed at least 300 ms ago and the state is
       `Editing`, it goes to `Previewing` and returns the request with a copy of the form.
     - `apply(Previewed { result: Err(field_error) })` → `Editing` with that error (any field).
     - `start()` no longer validates: it needs `Editing` with a preview that has `files_to_copy > 0` and
       no error, and returns `Run` with a copy of the form.
     - The debounce, `rescan`, request ids, `disconnected`, `cancel`, `can_close`, `back_to_editing`
       and `result_folder` stay as they are.
- **Tests:** this task changes the API that some T31 and T32 tests call. Update **only those calls**
  and keep what each test verifies. The one test whose behaviour moves, T32's
  `invalid_form_shows_field_error_without_a_request`, becomes: the invalid form gives a preview request,
  and applying `Previewed` with a `Source` field error shows that error. Add:
  - worker: an invalid form (empty destination) gives `Previewed` with `Field::Dest` and "Välj ett
    mål."; `run` with an invalid form gives `Done` with that message and `copied == 0`;
  - dialog: `tick()` and `start()` work with a form whose source does not exist (no file access in the
    dialog); a field error for `Field::Folder` from the worker is shown on that field.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies; rule 7 (build only in
  `target`).
- **Done when:** `.\tools\check.ps1` is all OK.
