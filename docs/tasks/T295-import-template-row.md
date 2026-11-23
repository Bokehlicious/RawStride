# T295. The import dialog's Metadata Template row — Blocked

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
  `editor.rs`'s `variable_error_text`; `import_text.rs`'s `ImportText`.
- **What:**
  1. **`settings.txt`** (`settings.rs`): a key `import_template`, the full path of the import's
     template file, empty or missing for none. Read at the start into the form `open_import`
     makes (as `import_incremental` is), and written when the dialog closes (as
     `import_incremental` is, in `close_import`). A path is kept as text; an unreadable line is
     a problem of the file as for the other keys. **The old six fields** (U13-Q18):
     they were never kept in `settings.txt` (only in the dialog's last form while the program
     runs), so nothing is read or converted; the task's report says so.
  2. **The row** (`draw_import_dialog`), where the six fields were (T294): `Metadata Template:`,
     then the chosen file's name (its file name only; the full path as the hover text) or
     `None` in the weak text colour, then the buttons:
     - **Choose…**: the Windows open dialog (`rfd::FileDialog`), starting in
       `templates_folder(%LOCALAPPDATA%)` (made when missing) or the chosen file's folder, file
       type `Metadata Template`, extension `xmp`; a file chosen becomes the form's template;
     - **Edit…**: `open_template_window` (T293) with the chosen file, or without a file when
       none is chosen (U13-Q17). The window remembers that it was opened from the
       import dialog: a later successful **Save…** in it makes the saved file the import's
       template, if the import dialog is still open;
     - **None** (greyed without a template): no template.

     Changing the template never rescans (T294).
  3. **The read at Start** (U13-Q6; U13-Q16): `start_import`, for a run that copies
     (not `marks_only`, which writes no template) with a template chosen:
     - when the file is not there (`is_file`), the import does not start and the dialog shows
       `The metadata template {file name} was not found. Choose another or None.` as its submit
       error;
     - otherwise the file is sent to `TemplateIo::load` (T292), the dialog says `Reading the
       metadata template…` and Start is greyed until the answer; closing the dialog drops the
       wait (a late answer is ignored);
     - the answer: an error → `The metadata template {file name} could not be read: {reason}`
       and nothing starts; a template with no field → `The metadata template {file name} has no
       field to write.` and nothing starts; `check_variables` fails → `The metadata template
       {file name} cannot be used: {variable error text}` and nothing starts; otherwise
       `ImportWorker::run(form, options, Some(template))`, as `start_import` runs it today.
       What `TemplateRead` names as skipped, dropped or unreadable is not shown here; Edit…
       shows it (U13-Q16).

     Without a template, and for `marks_only`, the import starts at once with `None`, as after
     T294.
  4. **Texts** (`import_text.rs`, rule 13): `Metadata Template:`, `Choose…`, `Edit…`, `None`
     (the button and the empty row's text), `Reading the metadata template…`, and the four
     messages of point 3; the dialogs' file type name is T291's.
  5. `docs/glossary.md`: the terms of point 4 it lacks; its row "IPTC-mall, Mallen, Variabler"
     and the modes' row stay (ADR 0007's terms) unless the owner says otherwise.
- **Tests to add:** `settings.rs`: `import_template` written and read; a file without it gives
  no template and no problem; an empty value gives none. `import_text.rs`: each text of point 4
  (exact). `main.rs`, as far as it can be done without drawing: Start with a template file that
  is not there starts no import and sets the submit error; a `Loaded` answer with a template of
  no field, and one with Headline `{job}`, start nothing and give their messages; a `Loaded`
  answer for another request id changes nothing; a save in a template window opened from the
  import dialog sets the dialog's template, one opened from the File menu does not.
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
  keywords; a template file renamed away before Start stops the import with nothing copied; the
  file is remembered after a restart.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, fifth of the chain T291 → T292 → T293 → T294 → T295 → T296). Waits for T293 and T294, the owner's answers to U13-Q16–U13-Q18, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
