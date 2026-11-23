# T295. The import dialog's Metadata Template row — Open

- **Why:** ADR 0017 U13, part U13c (U13-Q6; U13-Q16, U13-Q17 and U13-Q18), fifth of the chain T291 → T292 → T293 → T294 → T295 → T296. After T294 the import writes a
  `TemplateFile` it is given, and the dialog has no template row. This task adds the row
  **Metadata Template:** with the file's name and the buttons **Choose…**, **Edit…** and
  **None**, keeps the file chosen in `settings.txt`, and reads the file when the import starts,
  so a file that is gone, cannot be read, holds nothing or has an unknown variable stops the
  import **before anything is copied**. It ends U13c; the owner's hand check of U13c (and of
  U13a, which shows nothing on its own) follows.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/settings.rs`, `crates/pw-loupe/src/import_text.rs`, `docs/glossary.md`,
  `docs/test-inventory.md`.
- **Background (read before starting):** `main.rs`: `open_import`, `close_import`,
  `request_import_start`, `start_import`, `draw_import_dialog`, `import_submit_error`,
  `last_form`, the template window of T293 (`open_template_window`, its file, its save) and
  the `TemplateIo` answers; `import.rs`'s `ImportForm::template` (T294);
  `ImportWorker::run`'s `template` argument (T294); `settings.rs`'s `import_incremental` (a
  setting of the import dialog kept in `settings.txt`, its reading, writing and tests);
  `template_form.rs`'s `templates_folder` (T291); `pw_metadata::template_file`
  (`TemplateRead`, `TemplateFile::check_variables`, `TemplateFileError`) and
  `editor.rs`'s `variable_error_text` (`pub(crate)` since T291); `import_text.rs`'s
  `ImportText`; `on_exit` and `Loupe`'s `last_form` initialiser in `main.rs`;
  `import_dialog.rs`'s `ImportDialog::start`, `edit` and `can_start`.
- **What:**
  1. **`settings.txt`** (`settings.rs`): `ShellSettings::import_template: Option<PathBuf>`
     (default `None`), kept under a key `import_template`, the full path of the import's template
     file. The line is written only when a template is chosen, so a file without a template is
     written exactly as today; a missing or empty value gives `None`. The value is read by
     `parse_folder`'s rule (a full path, no line break); anything else is one problem of the file
     and gives `None`. Read at the start into `Loupe`'s `last_form` (as `import_incremental` is),
     and copied back when the dialog closes (`close_import`) **and in `on_exit` when the dialog is
     open**, both as `import_incremental` is. The existing test that builds a whole
     `ShellSettings` literal gets the new field (an intended change, named in the report); the
     tests that compare a whole written file stay as they are, since the default writes no line.
     **The old six fields** (U13-Q18):
     they were never kept in `settings.txt` (only in the dialog's last form while the program
     runs), so nothing is read or converted; the task's report says so.
  2. **The row** (`draw_import_dialog`), where the six fields were (T294): `Metadata Template:`,
     then the chosen file's name (its file name only; the full path as the hover text) or
     `None` in the weak text colour, then the buttons:
     - **Choose…** (the existing `ImportText::Choose`): the Windows open dialog
       (`rfd::FileDialog`) with the title `Choose Metadata Template`, starting in the chosen
       file's folder when a template is chosen, otherwise in `templates_folder(%LOCALAPPDATA%)`
       (made when missing; no start folder without `LOCALAPPDATA`), file type `Metadata
       Template`, extension `xmp`; a file chosen becomes the form's template (through
       `ImportDialog::edit`);
     - **Edit…**: `open_template_window` (T293) with the chosen file, or without a file when
       none is chosen (U13-Q17), with the window's flag "opened from the import dialog" set
       (this task adds the field to T293's `TemplateWindow`; a parameter or a field set after the
       call, the agent's choice); a window opened from the File menu has it false. A later
       successful **Save…** (T293's `Answered::Saved(path)`) in a window with the flag makes the
       saved file the import's template (through `ImportDialog::edit`) when an import dialog is
       open and is editing or previewing (not while it runs or shows its result); otherwise the
       save only saves;
     - **None** (greyed without a template): no template.

     Changing the template never rescans (T294). While the template window is open,
     `draw_import_dialog` greys its whole content (the window is modal, T293, and the import
     dialog behind it is never clicked).
  3. **The read at Start** (U13-Q6; U13-Q16): `start_import`, for a run that copies
     (not `marks_only`, which writes no template) with a template chosen:
     - when the file is not there (`is_file`), the import does not start and the dialog shows
       `The metadata template {file name} was not found. Choose another or None.` as its submit
       error;
     - otherwise the file is sent to `TemplateIo::load` (T292) and the import waits for the
       answer (a small plain-Rust wait in `main.rs`, for example `ImportTemplateWait { id, path }`,
       whose method takes a `TemplateEvent` and gives `None` for another id, otherwise the
       template or the message of the next point). While it waits: `Reading the metadata
       template…` is shown as a normal (not error) line where the submit error is shown; the
       form, Start and the template row are greyed (`add_enabled_ui(!running &&
       !reading_template)` or similar; T294 removed the `confirming` state);
       `request_import_start` (the button and the Enter key) does nothing; closing the dialog
       drops the wait (a late answer is ignored). The per-frame read of `TemplateIo`'s answers
       (T293) gives a `Loaded` answer with the import's wait id to the import, and repaints while
       the window or the import waits;
     - `TemplateIo::load` answers `Err(TemplateIoStopped)` → `The metadata template {file name}
       could not be read: the template reader has stopped.` and nothing starts;
     - the answer: an error → `The metadata template {file name} could not be read: {reason}`,
       the reason the `TemplateFileError`'s text as it is (it may name the file again), and
       nothing starts; a template with no field → `The metadata template {file name} has no
       field to write.` and nothing starts; `check_variables` fails → `The metadata template
       {file name} cannot be used: {variable error text}` and nothing starts. When the dialog
       can no longer start (`!can_start()`) or its template is no longer the path read, the
       answer is dropped and nothing starts. Otherwise the whole of today's `start_import`
       (`ImportDialog::start`, the eject preparation, the watcher, `run`) runs now, with
       `ImportWorker::run(form, options, Some(template))`. What `TemplateRead` names as skipped,
       dropped or unreadable is not shown here; Edit… shows it (U13-Q16).

     Without a template, and for `marks_only`, the import starts at once with `None`, as after
     T294. A missing file's check is a plain function (for example
     `missing_template_error(path: &Path) -> Option<String>`) so it can be tested.
  4. **Texts** (`import_text.rs`, rule 13): `Metadata Template:`, `Edit…`, `None`
     (the button and the empty row's text), `Choose Metadata Template` (the open dialog's
     title), `Reading the metadata template…`, and the five messages of point 3 (`Choose…` is
     the existing `ImportText::Choose`); the dialogs' file type name is T291's. The `cannot be
     used` message is a function in `import_text.rs` that takes the `&VariableError` of
     `check_variables` and calls `editor.rs`'s `variable_error_text` itself (`pub(crate)`, which
     `main.rs`, a binary crate, cannot call).
  5. `docs/glossary.md`: the terms of point 4 it lacks; its row "IPTC-mall, Mallen, Variabler"
     and the modes' row stay (ADR 0007's terms) unless the owner says otherwise.
- **Tests to add:** `settings.rs`: `import_template` written and read; a file without it gives
  no template and no problem; an empty value gives none; a relative path gives one problem and
  none; the default settings write no `import_template` line. `import_text.rs`: each text of
  point 4 (exact). `main.rs`, without a `Loupe` (no test builds one), on the plain pieces of
  points 2 and 3: `missing_template_error` of a path that is not there gives the message, of a
  file that is there `None`; the import's wait given a `Loaded` answer with a template of no
  field, and one with Headline `{job}`, gives their messages, an `Err` answer the read message,
  a good template the template, and an answer for another request id `None`; the function that
  decides whether a `Saved` answer sets the import's template says yes for a window with the
  "opened from the import dialog" flag and an editing dialog, no without the flag, no while the
  dialog runs.
- **Not in this task:** `pw-metadata`'s cleanup (T296); Load Template… and Save as Template… in
  the metadata window and the right-click submenu (U13d).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  The owner's hand check of U13c (and U13a) follows; the reviewer writes its points in the plan:
  File ▸ Metadata Template… opens the window in two columns (one when narrow), every field shown
  also with a section and a field hidden in the metadata window's layout; typing checks a field,
  unchecking keeps its text out of the saved file; Keywords with + saved, loaded again with +
  checked; Contact Info and Location Created sub-fields each with their own check box; Save…
  starts in `%LOCALAPPDATA%\RawStride\templates\`, asks before replacing, a name without `.xmp`
  gets it; a field with `{job}` refuses the save; Close with changes asks; a template file
  written by another program (an XMP sidecar) loads with what was not loaded named; in the import
  dialog Choose…, Edit… (a save there becomes the import's template) and None; an import with
  Headline `{date} {name}` and Keywords with + gives each image its own values and adds to its
  keywords; a template file renamed away before Start stops the import with nothing copied; a
  template with Headline `{job}`, and one with no field, stop Start with their messages; the
  form and Start are greyed while the template is read; the file is remembered after a restart,
  also when the program is quit with the import dialog open; a save in a folder whose path holds
  a `%` is refused with its text (U13a); an import with Keywords + onto a file whose keywords
  have qualifiers, or a Headline longer than IIM allows, lists the notice in the summary
  (U13-Q19). At Done the reviewer adds the CHANGELOG line (Changed: the import dialog's IPTC
  Template fields, modes and "Clear fields?" question are replaced by a Metadata Template row,
  Choose…, Edit…, None, read at Start, with variables filled per image and the writes' notices
  listed), and in the commit of the hand check changes `README.md`'s "What it does" from "an
  IPTC template" to "a metadata template" (the Planned item stays until U13d).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, fifth of the chain T291 → T292 → T293 → T294 → T295 → T296). Waits for T293 and T294, the owner's answers to U13-Q16–U13-Q18, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: `main.rs` tests cannot build a `Loupe`, so the missing-file check, the import's wait and the "sets the import's template" decision are plain pieces the tests use; the whole of today's `start_import` runs after a good answer, the form is greyed and Start does nothing while the read waits, a stale answer (dialog cannot start, template changed) is dropped; `on_exit` keeps the setting too (`import_incremental` is copied there); `ShellSettings::import_template` named, its line written only when set, read by `parse_folder`'s rule, the whole-literal test's change named; the waiting text is a normal line; a stopped `TemplateIo` gets a message; `Choose…` reuses `ImportText::Choose`, the dialog gets a title, its start folder decided; answers routed by id; the window's "opened from the import dialog" flag defined (Edit… sets, File menu clears, only an editing or previewing dialog takes the save); hand check gains `{job}`, no field, greyed while reading, quit with the dialog open. `variable_error_text` made `pub(crate)` by T291 |
| 2026-11-23 | Open | T294 done and committed; its cold read and the chain review were done before (see T291's log). Set Open |
