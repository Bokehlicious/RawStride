# T298. Template changes for one import only — Done

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
     the window's `file` the edit's `file` (its title and Save…'s suggested name). The load
     settles the form as `load` always does, so Close asks only after further changes in the
     window (U13-Q21, amended 2026-11-24): the edit itself stays the import's until another Use
     for This Import, a Save…, Choose… or None, so closing loses nothing of it. This path does not
     go through `open_template_window`'s file load (nothing is read from the file); it opens the
     window as `open_template` does, sets `from_import`, and puts the edit in it.
  4. **A Save…** in a window opened from the import dialog that makes the saved file the
     import's template (`poll_template_io`, T295) also drops the edit: the saved file now holds
     it. The edit is dropped there explicitly, also when the saved file is the one already chosen
     (the form's `template` then does not change, so `applies` alone would keep it).
  5. **Start** (`start_import`): for a run that copies, with an edit that applies, the import
     runs at once with `Some(edit.template.clone())` (no file check, no read; the checks were
     made at the button). Without an edit, as today. `marks_only` as today (no template).
  6. **The edit is dropped** in `close_import` (with the template read's wait), so a closed
     dialog never keeps it; `settings.txt` keeps only the file, unchanged. An import run does not
     drop it: after New Import in the same dialog it still applies, as the file stays chosen
     (U13-Q21, amended 2026-11-24).
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
  changed version, and Close there without a further change asks nothing and keeps the edit;
  after the import, New Import still shows "(changed for this import)"; `{job}` refuses the button with its text; None and Choose… drop the edit; a
  Save… after an edit makes the saved file the template; closing and opening the import dialog
  shows the file without the changes.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13-Q21, owner, 2026-11-23, at U13c's hand check). Waits for T297 and its cold read |
| 2026-11-24 | Open | Cold read by the reviewer after T297 (e2a1d8a, hand-checked b3c16ce), against main.rs and template_form.rs. `TemplateForm::load` always settles the form, so "counted as changed" in point 3 had no way to be made; the owner decided (U13-Q21 amended 2026-11-24, ADR 0017 §6.5) that Close asks only after further changes, as a plain load gives, and that the edit stays after New Import in the same dialog. Point 3: Edit… with an edit opens the window without `open_template_window`'s file load. Point 4: the edit dropped explicitly on a Save…, also of the file already chosen. Point 6 and the hand check updated |
| 2026-11-24 | Ready for review | Done by the task agent. `main.rs`: `ImportTemplateEdit` (`file`, `template`, `applies`) and `Loupe::import_template_edit`; `template_row_text` (plain, tested) gives the row's text and its weak colour; Use for This Import (`template_use_for_import`) in the button row before Close while `saved_sets_import_template` says yes, a refusal shown through `TemplateWindow::refused`; Edit… with an edit that applies opens the window through `open_template_window_with_edit` (`TemplateForm::load`, the edit's file, nothing read; an already open window is kept as `open_template` keeps it); None (enabled also for an edit that applies) and Choose… with a file drop the edit; a Save… that sets the import's template drops it in `poll_template_io`; `start_import` runs a copying import at once with the edit; `close_import` drops it. The `load`'s not-loaded list is ignored for the edit (the form made the template). Texts in `template_form.rs` and `import_text.rs`, exported from `lib.rs`; glossary row; test inventory: 4 tests, `pw-loupe` 1397 |
| 2026-11-24 | Done | Reviewed and approved by the owner: every point checked in the code (the button greyed while a request waits; Choose… drops the edit before the window opens, so it opens the file; a Save… drops it also for the file already chosen; Start, close_import and New Import as decided). From the review: the doc comment of `draw_template_window` re-wrapped (comment only). check.ps1 all OK (test-output.txt written after the task agent's last code change; 2523 passed, 78 ignored, as the test inventory), no clippy warnings. CHANGELOG: one sentence added to T295's Changed line (T297's part of it re-wrapped). The owner's hand check (above) follows |
| 2026-11-24 | Done | Hand-checked by the owner: every point works, approved (recorded in the plan) |
