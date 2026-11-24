# T301. Load Template… and Save as Template… in the metadata window — Open

- **Why:** ADR 0017 U13-Q7, U13-Q22, U13-Q23 and U13-Q25 (owner, 2026-11-24; part U13d). The
  metadata window gets two buttons: **Load Template…** puts a template file's fields into the
  form as unsaved edits, **Save as Template…** saves what the window shows as a template file.
  T300 made both in plain Rust; this task draws the buttons and runs the dialogs and the template
  thread.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/editor.rs` (texts only), `docs/glossary.md`, `docs/test-inventory.md`.
- **Background (read before starting):** ADR 0017 §6.5 U13-Q2, U13-Q7, U13-Q14, U13-Q22–U13-Q25.
  `main.rs`: `MetadataWindow`, `EditorStatus`, `draw_metadata_window` (the button row with Copy
  All Fields and Paste All Fields; how `copy` and `paste` are applied after the window is drawn;
  `form_before` / `form_after` and the status cleared when the form changed; `locked`, `saving`,
  `asking`), `template_file_dialog`, `template_question`, `template_load`, `template_save` (the
  save dialog, `with_xmp_extension` and the replace question), `send_template_load`,
  `poll_template_io` (how an answer goes to the import's read or the template window by its id),
  `TemplateWindow::answer` (how a load or save answer and its errors become status lines).
  `template_form.rs`: T300's `window_template`, `template_text` (`Loaded`, `Saved`,
  `LoadStopped`, `NotSavedStopped`), `refused_text`, `save_failed_text`, `load_failed_text`,
  `not_loaded_lines`. `editor.rs`: T300's `EditorForm::load_template`, `EditorText`,
  `editor_text`. `template_io.rs`: `TemplateIo::load`, `save`, `TemplateEvent`.
- **What:**
  1. **The buttons** (U13-Q22): **Load Template…** and **Save as Template…** in the window's
     button row right after Paste All Fields, their texts from `editor_text` (new
     `EditorText` variants, exact texts `Load Template…` and `Save as Template…`). Enabled while
     the form is shown and the window is not locked, not asking a question and not waiting for
     the template thread; Save as Template… also when the form has no changes. Neither has a key.
  2. **Load Template…:** the open dialog (`template_file_dialog`, in the templates folder); no
     question first (the loaded fields are unsaved edits the user can still cancel, U13-Q7).
     The file is sent to the template thread (`TemplateIo::load`) and the window waits for that
     id (a new field of `MetadataWindow`, set in `MetadataWindow::new`). **While it waits**
     (a flag of its own, not ORed into `locked` or `saving`, which would show "Saving…" and stop
     `read_arrived` refilling the form): the form's fields are greyed (the form's enabled flag),
     every button but Cancel is greyed (`busy` for Previous, Next and Save and Next, OK,
     `can_apply`, `can_paste`, Copy, the two new buttons), and the window's keys (moving,
     rating) do nothing; the status area keeps its lines. **Cancel and Escape still close the
     window** as they do otherwise (with the question when the form has changes), so a
     template thread that never answers cannot hold the modal window open; the late answer is
     then dropped. While it waits a repaint is asked for (`poll_template_io`'s
     `request_repaint_after` condition and the window's own, which today look only at the
     import's and the template window's waits and at `locked`), so the answer is shown without
     a mouse move. The answer is taken in
     `poll_template_io` (which runs before the window is drawn, so the "form changed" check of
     the draw does not clear the status it sets), before that function drops answers when no
     template window is open:
     - read: `EditorForm::load_template` with the window's layout; the window's status shows
       `Loaded {file}.` and, under it in the error colour, `not_loaded_lines` of the read and of
       what `load_template` returned (U13-Q23), as the template window shows them after its
       Load (`error_line`);
     - not read: the error line of `load_failed_text`, in the error colour;
     - the thread stopped: `LoadStopped`'s text, in the error colour.
     An answer for a window that has closed or moved to another image (a new `MetadataWindow`)
     is dropped. The dialogs are run by new methods taking the window (the template window's
     `template_load`, `template_save` and `send_template_load` work only on
     `self.template_window`), called after the window is drawn next to Copy and Paste, before
     the draw's early returns.
  3. **Save as Template…:** T300's `window_template` with the window's layout. A refusal opens
     no dialog and is shown in the window, worded for this window (it has no check boxes):
     `FieldErrors` as an OK with field errors shows them (`window.errors` set from the named
     items, so each error stands under its field, and `not_saved_status` naming the fields and
     lists); `Empty` as `No field shown has a value to save as a template.`; `Variables` as
     `Not saved as a template: {reason}` (new `EditorText` variants, exact texts), in the error
     colour. Otherwise the save dialog as the template window's Save… (`template_file_dialog`,
     `.xmp` added, the question before replacing a file whose name got `.xmp` added, asked with
     `template_question`, whose box is titled "Metadata Template"), no file name given; the
     template is sent to `TemplateIo::save` and the window waits as in point 2. The answer:
     saved, `Saved {file}.`; not saved, `save_failed_text` in the error colour; ExifTool without
     its settings file (`SaveError::NoConfig`), the template window's `NotSavedNoConfig` text in
     the error colour; the thread stopped, `NotSavedStopped`'s text. The form and its edits are
     not changed by a save.
  4. **Routing** (`poll_template_io`): the window's load or save answer is taken by its id before
     the template window's answers are looked at; the import's read and the template window keep
     what they get today. The window may keep its waiting id in `MetadataWindow`.
  5. **The status:** a new `EditorStatus` variant (not `Batch`, to which a failed read appends)
     holds the lines of points 2 and 3 as `StatusLine`s, each in the error colour or not; it is
     cleared as the other statuses are, at the next change of the form.
  6. **Glossary:** one row for Load Template… and Save as Template… (the metadata window's
     buttons, U13-Q22), modelled on the Copy All Fields / Paste All Fields row.
  7. **Exports:** T300 exports `window_template` from `lib.rs` and makes `load_template` `pub`;
     if something more is needed from the library, stop and ask (`lib.rs` is not in this task).
- **Tests to add:** `editor.rs`: the two button texts and the two refusal texts (exact).
  `main.rs`, without a `Loupe`: a plain function giving the status lines of a load answer
  (`(path, &TemplateRead, &[String]) -> Vec<StatusLine>`), tested for a read with a skipped
  field and a not-loaded field (the not-loaded lines in the error colour); a plain function
  giving the status of a save answer, tested for saved, not saved, `NoConfig` and stopped. The
  test inventory's `pw-loupe` row and totals are updated.
- **Not in this task:** the right-click menu (T303); a change to the template window or the
  import's template; keeping the loaded template's file name anywhere.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** after T300 is committed. Not beside T303 (both change `main.rs` and
  `poll_template_io`); T303 comes after it.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer adds to the CHANGELOG (Added) that the metadata window can load a
  template into its form and save its fields as a template. The owner's hand check comes with
  T303's (U13d).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0017 U13-Q7, U13-Q22, U13-Q23, U13-Q25, owner, 2026-11-24; part U13d). Waits for T300, its cold read and the chain review of T299–T303 |
| 2026-11-24 | Blocked | Cold read (fresh agent). Fixed: the exports moved to T300 (`lib.rs`); the waiting state as its own flag and every place it greys (form, buttons, Cancel, Escape, keys), not ORed into `locked`/`saving`; the answer taken in `poll_template_io` before its early drop; new methods taking the window for the dialogs; the refusals worded for this window (field errors as an OK shows them, two new texts); `NoConfig`'s text; a new status variant, not `Batch`; the tests made required plain functions |
| 2026-11-24 | Blocked | Chain review (T299–T303; recorded in T299's log): Cancel and Escape stay enabled while waiting (a thread that never answers would hold the modal window); a repaint asked for while waiting; the field-error refusal shown as the window's own OK shows it, ADR 0017 U13-Q25's wording settled to match |
| 2026-11-24 | Open | T300 Done and committed (a53f7d4); cold read and chain review done. Set Open; may run beside T302 (T302 changes `metadata.rs`, this task does not) |
