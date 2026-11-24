# T297. Choose… in the import dialog also opens the template window — Done

- **Why:** ADR 0017 U13-Q20 (owner, 2026-11-23, at U13c's hand check). After T295, Choose…
  in the import dialog only picks the template file; to see what the template holds the user has
  to press Edit… as well. Choose… now does both: the file chosen becomes the import's template
  and the template window opens with it loaded, as Edit… opens it.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`
  (only if a test is added or changed).
- **Background (read before starting):** `main.rs`: `draw_import_dialog` (the Metadata Template
  row of T295: the Choose… button with `import_template_dialog(…).pick_file()`, the flag
  `edit_template`, the `form != original_form` block that hands the form to `ImportDialog::edit`,
  and the `if edit_template` block after it that calls `open_template_window(chosen, true)`);
  `open_template_window` and `TemplateWindow::from_import`; `poll_template_io` with
  `saved_sets_import_template`; ADR 0017 §6.5 U13-Q17 and U13-Q20.
- **What:**
  1. When Choose…'s open dialog gives a file, the form's template is set to it as today, and
     the template window opens with that file loaded, **exactly as Edit… opens it**: the same
     call (`open_template_window(Some(path), true)`), so the window is marked as opened from the
     import dialog and a later Save… there makes the saved file the import's template
     (U13-Q17). The window opens after the form has been handed to `ImportDialog::edit`, so the
     dialog's template is the file chosen (for example by setting `edit_template` when a file
     was chosen, since that block already runs after the form is handed over and reads the
     dialog's template).
  2. Cancelling the open dialog changes nothing and opens no window.
  3. Closing the template window without a save keeps the file chosen (nothing in the close
     path changes the import's template; check that it stays so).
  4. Edit… and None are unchanged. Choosing a template never rescans (T294) and clears the
     submit error as any form change does (T295), unchanged.
- **Tests:** `main.rs` cannot build a `Loupe` in a test, and the change is one more call of the
  existing path that T295's tests cover (`saved_sets_import_template`); no new test is required.
  If the agent splits out a plain piece (for example a function that says whether the window
  opens after Choose…), it gets a test and the test inventory's `pw-loupe` row and totals are
  updated. The behaviour is covered by the owner's hand check (below).
- **Not in this task:** any other change to the template window, the open dialog or its start
  folder; U13d.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer adds to T295's CHANGELOG line that Choose… opens the chosen template in
  the template window. The owner's hand check: Choose… with a file opens the template window
  with that file loaded and the import dialog showing its name; Close without changes keeps the
  file chosen; a Save… under another name there makes that file the import's template; Cancel
  in the open dialog opens no window and keeps the template as it was.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Open | Added (ADR 0017 U13-Q20, owner, 2026-11-23, at U13c's hand check point 11; owner chose a task for a task agent). Small and in one file, so no separate cold read: the reviewer read T295's code for it (the order of the form hand-over and the `edit_template` block) |
| 2026-11-24 | Ready for review | main.rs: Choose… sets `edit_template` when the open dialog gives a file, so the existing block after the form hand-over opens the template window with `open_template_window(chosen, true)` (as Edit…); cancel sets nothing. Close path checked: `template_close` only drops the window, the import's template is kept. Comments updated (the Choose… branch, the Edit… block, the doc comment of `open_template_window`). No test added (no plain piece split out), test inventory unchanged. check.ps1 all OK, no clippy warnings |
| 2026-11-24 | Done | Reviewed and approved by the owner: the order (form handed over, then the window opened with the dialog's template), Cancel, Close and a Save… from the window checked in the code; check.ps1 all OK (test-output.txt written after the last change of main.rs), no clippy warnings. CHANGELOG: T295's Changed line says Choose… opens the chosen template. The owner's hand check (above) follows |
