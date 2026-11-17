# T166. Mark a card whose images are at the destination already — Done

- **Why:** from the review of T158 (backlog, 2026-11-16). A card whose images are all at the
  destination already, for example from an import made before the register existed (T157), cannot
  be marked. The dialog will not start an import with nothing to copy (`ImportDialog::start` and
  `can_start`, `files_to_copy == 0`), so the register never gets those files. They are then
  protected only by the destination check, not when importing to another destination. The engine
  already adds the files that need no copy to the register at the start of a run
  (`pw_ingest::execute_recording`); only the dialog stops it. **Owner, 2026-11-17** (ADR 0005
  choice 6): with "copy new images only" on, a register that can be written, nothing to copy and
  at least one file at the destination already, the start button reads "Mark Card as Imported"
  and the run adds those files to the register. Eject after import applies as for any import. The
  summary is the usual one, and no tab opens.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import_dialog.rs`,
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/import_worker.rs` (a test only), `docs/glossary.md`,
  `docs/test-inventory.md`.
- **What:**
  1. In `import_dialog.rs`, on `ImportDialog`: `pub fn marks_only(&self) -> bool`. It is true
     exactly when the state is `Editing { preview: Some(preview), error: None }`,
     `self.form.incremental` is on, `preview.files_to_copy == 0`, `preview.present > 0` and
     `preview.register_note` is `None` (a register note means the card has no register root, or
     its register is foreign or unreadable, so it cannot be marked). The preview cannot see that a
     card is write-protected: there the button shows, and the run ends with the usual
     `register_error` in the summary and nothing marked (accepted, reviewer 2026-11-17).
  2. `can_start` and `start` accept `preview.files_to_copy > 0 || self.marks_only()`, with
     everything else as today. `start` sends the same `DialogRequest::Run`; the worker needs no
     change.
  3. In `import_text.rs`: `ImportText::MarkCard` with the text "Mark Card as Imported". Add the
     pair to `the_incremental_texts_keep_their_wording`.
  4. In `main.rs`:
     - the start button (`draw_import_dialog`, today `import_text(ImportText::Start)`) shows
       `ImportText::MarkCard` when `dialog.marks_only()`, otherwise `ImportText::Start`. Read
       `marks_only` where `can_start` is read today, before the drawing closure;
     - `request_import_start`: when `marks_only()`, start at once without the Clear confirmation,
       since nothing is copied and no IPTC field is written. Enter goes through the same function,
       as today.
  5. In `docs/glossary.md`, §5 Import: a row `| Markera kortet som importerat | Mark Card as
     Imported | the start button when nothing is copied (T166) |`.
- **Tests to add:**
  - `import_dialog.rs`: `marks_only` and `can_start` are true with `incremental` on,
    `files_to_copy == 0`, `present > 0` and no register note, and `start` then gives
    `DialogRequest::Run`. Each of these makes `marks_only` and `can_start` false: `incremental`
    off, `present == 0`, a register note, a field error. With `files_to_copy > 0`, `marks_only` is
    false and `can_start` true, as today.
  - `import_worker.rs`, with the existing helpers for a run and a preview with the register in the
    test's own folder (T158; `incremental_form`, `import_with_root`, `preview_with_root`,
    `register_text`, and `dirs.form` for `incremental` off), every form with the same folder
    template (for example `"imported"`, as the T158 test that imports twice): an import with `incremental` off copies two files and writes no
    register. A preview with `incremental` on then shows `files_to_copy == 0`, `present == 2` and
    no note. A run with that form copies nothing (`copied == 0`, `present == 2`, no
    `register_error`) and writes both files to the register. A new preview then shows
    `earlier == 2` and `present == 0`.
- **Not in this task:** the summary's wording (the usual "N already existed"); opening tabs
  (`new_folders` is empty, so none open, T163); the engine (`pw-ingest`), which needs no change;
  a separate menu item for marking a card.
- **Rules:** AGENTS.md §3; `docs/tasks/README.md` rule 13 for the new text (made in
  `import_text.rs`, the glossary row in this task).
- **Hand check (owner):** with a card whose images are at the destination already and whose
  `.rawstride-ingest.txt` is missing or does not list them: the preview shows nothing to copy and
  the button reads "Mark Card as Imported". After the run, a new preview counts them as "imported
  earlier"; with eject on, the card is ejected.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added from the backlog item "Mark a card imported before the register" (review of T158); the owner's decision is in ADR 0005 choice 6 |
| 2026-11-17 | Open | Cold read: doable with the named files; a run with nothing to copy writes the present files to the register, starts no ExifTool, opens no tab and ejects as usual; `present` leaves out files in the register; `edit()` clears the preview, so it cannot go stale. Changed: the `ImportText` test named, the glossary row given its Note column, the worker test's helpers and shared folder template named, a write-protected card (button shown, run ends with `register_error`) stated as accepted |
| 2026-11-17 | Ready for review | Done as written: `marks_only`, `can_start`/`start` in `import_dialog.rs`; `ImportText::MarkCard`; the button text and no Clear confirmation in `main.rs`; the glossary row; four new tests (three in `import_dialog.rs`, one in `import_worker.rs`) and the `MarkCard` assertion in `the_incremental_texts_keep_their_wording`. `check.ps1` all OK; `pw-loupe` 841 tests, 1 ignored |
| 2026-11-17 | Done | Reviewed: the diff does points 1–5 (`marks_only` with the owner's conditions, `start` through `can_start`, `ImportText::MarkCard`, the button text and no Clear confirmation, the glossary row); four tests, the worker test runs the whole case through the register; no bugs. Reviewer's `.\tools\check.ps1` on this tree (T165 and T166): all OK, 1700 passed, 65 ignored, 0 failed; clippy without warnings. Left over: none (the write-protected card is accepted in point 1 and ADR 0005). The owner's hand check after the commit |
