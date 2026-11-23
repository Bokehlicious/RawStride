# T298. Template changes for one import only — Blocked

- **Why:** ADR 0017 U13-Q21 (owner, 2026-11-23, at U13c's hand check). After T295 (and T297) a
  change made with Edit… reaches the import only when it is saved to the template file, so a
  change meant for this one set of images changes the file too. The template window opened from
  the import dialog gets a button **Use for This Import** that hands the form to the import
  without saving a file.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/template_form.rs`, `crates/pw-loupe/src/import_text.rs`,
  `crates/pw-loupe/src/lib.rs` (exports only), `docs/glossary.md`, `docs/test-inventory.md`.
- **Background (read before starting):** ADR 0017 §6.5 U13-Q17, U13-Q20, U13-Q21. `main.rs`:
  `TemplateWindow` (`form`, `file`, `from_import`, `refused`), `open_template_window`,
  `template_save` (how `TemplateForm::template` refuses and how the refusal is shown),
  `template_close` (the discard question), `draw_template_window` (the button row),
  `poll_template_io` and `saved_sets_import_template`, `draw_import_dialog`'s Metadata Template
  row (Choose…, Edit…, None; T297's Choose… that opens the window), `start_import`,
  `run_import`, `close_import`; `template_form.rs`: `TemplateForm::template`, `load`,
  `changed`, `mark_saved`, `TemplateText`, `template_text`; `import_text.rs`'s `ImportText`.
- **What:**
  1. **The edit kept for the import** (`main.rs`): a field of `Loupe`, for example
     `import_template_edit: Option<ImportTemplateEdit>` with `file: Option<PathBuf>` (the
     import's template file when the button was pressed, `None` without one) and
     `template: TemplateFile`. It counts only while `file` equals the import form's
     `template`; a plain function (for example `ImportTemplateEdit::applies(&self, chosen:
     Option<&Path>) -> bool`) says so, and every use below goes through it.
  2. **The button** (`draw_template_window`): **Use for This Import**, in the button row before
     Close, shown only in a window with `from_import` while `saved_sets_import_template` would
     say yes (an import dialog editing or previewing); greyed while a request waits, as the
     other buttons. Pressed: `window.form.template()`; a refusal is shown exactly as Save… shows
     it (`window.refused`) and the window stays; otherwise the template is kept as the import's
     edit with the import form's current `template` as `file`, the submit error is cleared, and
     the window closes **without** the discard question (the changes are taken, not lost).
  3. **The row** (`draw_import_dialog`): while an edit applies, the row's text is
     `{file} (changed for this import)` (the file's name; the full path stays the hover text), or
     `Not saved (this import only)` without a file, in the normal text colour; otherwise as
     today. **None** is enabled while a template or an edit applies, and drops both.
     **Choose…** drops the edit (a file chosen, or the dialog cancelled: cancelled keeps
     everything as it was, edit included). **Edit…** with an edit that applies opens the window
     (`from_import` true) and puts the edit's template in the form with `TemplateForm::load`,
     the window's `file` the edit's `file` (its title and Save…'s suggested name), and the form
     counted as changed so Close asks before the changes in the window are lost (the edit itself
     stays the import's until another Use for This Import, a Save…, Choose… or None).
  4. **A Save…** in a window opened from the import dialog that makes the saved file the
     import's template (`poll_template_io`, T295) also drops the edit: the saved file now holds
     it.
  5. **Start** (`start_import`): for a run that copies, with an edit that applies, the import
     runs at once with `Some(edit.template.clone())` (no file check, no read; the checks were
     made at the button). Without an edit, as today. `marks_only` as today (no template).
  6. **The edit is dropped** in `close_import` (with the template read's wait), so a closed
     dialog never keeps it; `settings.txt` keeps only the file, unchanged.
  7. **Texts** (rule 13): `Use for This Import` in `template_form.rs` (`TemplateText`, the
     window's text); `{file} (changed for this import)` and `Not saved (this import only)` in
     `import_text.rs` (a function for the first, an `ImportText` variant for the second),
     exported from `lib.rs` as T295's are. `docs/glossary.md`: one row for the three.
- **Tests to add:** `template_form.rs`: the button's text (exact). `import_text.rs`: the two row
  texts (exact). `main.rs`, without a `Loupe`: `ImportTemplateEdit::applies` with the same
  file, another file, `None` against `None`, `None` against a file; and, if the row's choice of
  text is a plain function, its three cases (no template, a file, an edit with and without a
  file). The test inventory's `pw-loupe` row and totals are updated.
- **Not in this task:** keeping an edit between runs of the program; U13d; any change to how a
  template file is read or saved.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** after T297 is committed, never beside it (both change `draw_import_dialog`'s
  template row).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer adds to the CHANGELOG that the import's template can be changed for one
  import without saving it. The owner's hand check: Edit…, a change, Use for This Import → the
  row shows "(changed for this import)", the import writes the changed values, the file is
  unchanged (opened again with Edit… after closing the dialog); Edit… with an edit shows the
  changed version; `{job}` refuses the button with its text; None and Choose… drop the edit; a
  Save… after an edit makes the saved file the template; closing and opening the import dialog
  shows the file without the changes.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13-Q21, owner, 2026-11-23, at U13c's hand check). Waits for T297 and its cold read |
