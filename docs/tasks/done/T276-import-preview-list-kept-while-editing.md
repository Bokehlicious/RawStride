# T276. The import dialog's preview list stays while the form is edited — Done

- **Why:** the owner's bug report (2026-11-22). While the IPTC template in the import dialog is
  filled in, the list of the images to be imported (the preview list at the bottom of the dialog)
  disappears at each key and comes back when the typing pauses, so the dialog's window shrinks
  and grows while the user types. The cause:
  - `ImportDialog::edit` (`import_dialog.rs`) sets the state to `Editing { preview: None, .. }`
    at every change of the form, so the list is not drawn; after `PREVIEW_DELAY` (300 ms) `tick`
    moves to `Previewing`, where only "Previewing …" is drawn; the worker's answer brings the
    list back (`main.rs`, `draw_import_dialog`, the `match &state` under the "Preview" heading).
  - The IPTC template (`form.iptc`) and "Eject" (`form.eject`) do not change the preview at all:
    the worker's `preview` (`import_worker.rs`) uses `form.validate()` (source, destination,
    backup, folder and name templates, job, first number) and `form.incremental`, never
    `form.iptc` or `form.eject`. Each key in the template asks for a preview that gives the same
    list.
  - The same flicker happens, smaller, while the folder or name template, Job, First number or a
    path is typed: there a new preview is needed, but the old list need not disappear meanwhile.
- **Crates:** `pw-loupe` (the shell). **Files:** `crates/pw-loupe/src/import_dialog.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`.
- **What:**
  1. **(A) An edit of only the IPTC template or Eject keeps the preview.** In
     `ImportDialog::edit`, when the state is `Editing` or `Previewing`, the change made the form
     different from before and every field except `iptc` and `eject` is unchanged, the form is
     updated and nothing else: the state, `request_id`, `changed_at` and `source_changed` stay as
     they were (a preview on its way is still applied when it comes; a debounce already started
     by an earlier change still runs; a field error shown stays; Start stays enabled while the
     template is typed, since the preview stays current). In `Running`, `Ejecting` and `Done`
     every edit works as today (in `Done` the form can still be edited, and an edit goes back to
     `Editing`; in `Running` and `Ejecting` `changed_at` is set). An edit that changes any other
     field, and an edit that changes nothing (the tests' `schedule_preview` calls
     `edit(|_| {}, now)` to start a preview), work as today. (`backup` is read by the worker only
     when `backup_on` is on; an edit of `backup` still restarts the preview, which is harmless.) The comparison is a private helper in `import_dialog.rs` (for example
     `preview_fields_equal(a, b)` comparing every field of `ImportForm` except `iptc` and
     `eject`, written out field by field, so a field added to `ImportForm` later must be placed
     on one side or the other); `ImportForm` itself does not change. The edit's doc comment says
     which fields restart the preview and why (the worker's `preview` does not read the others).
  2. **(B) The last list is kept, dimmed, until the next one comes.** `ImportDialog` keeps the
     last successful preview in a new private field (for example `shown: Option<Preview>`):
     - set to the preview in `apply` when an `ImportEvent::Previewed` with `Ok` is applied;
     - cleared in `apply` when a `Previewed` with `Err` is applied (a field error shows no list),
       in `disconnected` (its `Previewing` branch) and in `back_to_editing` (after an import the
       old list is wrong);
     - not cleared by `edit`, `tick`, `rescan` or `start`.
     Two new public methods:
     - `shown_preview(&self) -> Option<&Preview>`: the kept preview while the state is
       `Editing { error: None, .. }` or `Previewing`; `None` in `Editing` with an error,
       `Running`, `Ejecting` and `Done` (the list is not drawn there today and stays so).
     - `preview_is_current(&self) -> bool`: true only in `Editing { preview: Some(_), error: None }`
       (the same condition `can_start` and `marks_only` match on).
     `ImportDialogState` does not change, and `can_start` and `marks_only` do not change: an
     import still starts only from a current preview, never from the kept one.
  3. **The drawing (`main.rs`, `draw_import_dialog`).** Under the "Preview" heading, the list
     (the summary line, the register note and the scroll area with the lines and the "more" line,
     drawn exactly as today) is drawn from `dialog.shown_preview()` instead of from the state's
     `preview`; when `preview_is_current()` is false it is drawn dimmed and not interactive
     (`ui.add_enabled_ui(false, ..)` around the same code). "Previewing …"
     (`import_text(ImportText::Previewing)`) is drawn in the heading's row, after the
     "Search Again" button, while the state is `Previewing`, instead of on its own line below,
     so the list does not move down while a preview runs. Read both values before the window is
     drawn, beside `can_start` and `can_close` (about `main.rs:2224-2226`), the preview cloned
     (`dialog.shown_preview().cloned()`) as `state` is; the heading's row is at about
     `main.rs:2374-2379` and the `match &state` the list replaces at about `main.rs:2380-2407`.
     The list stays inside the form's existing `add_enabled_ui(!running && !confirming, ..)`. No other drawing
     changes, and no new text.
- **Tests to add** (in `import_dialog.rs`'s `tests`). Each preview is reached as the existing
  tests do: `schedule_preview`, then `sent(id)`, then `apply(&ImportEvent::Previewed { id, .. })`
  (not by setting `dialog.state` directly, which skips the new field); `ImportForm::default()` or
  `TempDirs` may be used, since `apply` does not validate the form. `t` is one `Instant`:
  - an IPTC-only edit (for example `form.iptc.creator` changed) in `Editing` with a current
    preview keeps the state, the preview and `can_start()`, and `tick(t + PREVIEW_DELAY * 2)`
    returns `None` (`changed_at` is `None` once the preview was applied, and A leaves it so);
    the same for an Eject-only edit;
  - an IPTC-only edit while `Previewing` keeps `Previewing`, and the worker's `Previewed` for the
    request sent before is still applied (the state becomes `Editing` with that preview);
  - an IPTC-only edit during a running debounce does not restart or cancel it: a folder-template
    edit at `t`, an IPTC-only edit at `t + 100 ms`, then `tick(t + PREVIEW_DELAY)` returns the
    preview request (with the IPTC change in its form);
  - an IPTC-only edit in `Editing` with a field error (a `Previewed` with `Err` applied) keeps
    the error;
  - an IPTC-only edit in `Done` goes back to `Editing` with no preview, as every edit there does
    today;
  - an edit of the name template (a field outside A) after a current preview: `shown_preview()`
    still returns the old preview, `preview_is_current()` is false and `can_start()` is false,
    through the debounce and in `Previewing`; when the new preview is applied, `shown_preview()`
    returns the new one and `preview_is_current()` is true;
  - `rescan` keeps `shown_preview()`, with `preview_is_current()` false;
  - a `Previewed` with `Err` clears `shown_preview()`; `disconnected` while `Previewing` clears
    it; `back_to_editing` from `Done` clears it;
  - `shown_preview()` is `None` in `Running` (after `start`) and in `Done`.
  The existing tests are not changed.
- **Not in this task:** the window's size or a fixed height for the list; the card refresh
  (`CardRefresh`); the worker (`import_worker.rs`) and `ImportForm` (`import.rs`). The reviewer
  updates ADR 0010 §4.1's Preview row ("recomputed shortly after each change": not after a change
  of only the IPTC template or Eject; the last list kept, dimmed, until the next one) and
  `CHANGELOG.md` (Fixed) at Done.
- **Rules:** `docs/tasks/README.md` "How to work on a task"; no new user-visible text.
- **Hand check (the owner):** open the import dialog with a card or folder that has images; type
  in an IPTC field: the list stays and does not move; type in the name template: the list stays,
  dimmed, with "Previewing …" beside "Search Again", and shows the new names after the pause;
  "Search Again" keeps the list dimmed until the scan is done; an invalid folder template shows
  its error and no list.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Added (the owner's bug report: the list flickers while the IPTC template is typed in). Cold read: the named files suffice, no existing test breaks, "every field except iptc and eject" checked against `validate` and the worker's `preview`; A limited to `Editing` and `Previewing` (an edit in `Done`, `Running` and `Ejecting` as today) with a test for `Done`; Start enabled while the template is typed noted; the preview cloned and the line ranges in `main.rs` named; the tests reach a preview through `sent` and `apply`, the debounce test's times and the reason `tick` returns `None` written out |
| 2026-11-22 | Ready for review | A and B done as written, plus one owner decision (2026-11-22, asked during the task): an edit that leaves `Done` also clears the kept preview, as `back_to_editing` does, so the list from before the import is not shown dimmed (`edit` otherwise never clears it); the Done test checks it. `preview_fields_equal` destructures `ImportForm` so a new field fails to compile until it is placed. `import_dialog.rs`: `shown` field, `preview_fields_equal`, `edit` (early return, doc), `apply`, `disconnected`, `back_to_editing`, `shown_preview`, `preview_is_current`, 9 tests (existing tests unchanged). `main.rs`: the list drawn from `shown_preview()`, dimmed when not current, "Previewing …" in the heading's row. Test inventory: pw-loupe row and totals. check.ps1 all OK (2303 passed, 78 ignored), no clippy warnings |
| 2026-11-22 | Done | Reviewed: A and B done as written. `preview_fields_equal` destructures `ImportForm`, so a new field must be placed before the crate builds; `edit` returns early only in `Editing` and `Previewing` when the form changed and every field the preview reads is equal, so `edit(|_| {}, now)` still starts a preview; `changed_at`, `request_id` and `source_changed` untouched there. Every state transition checked: `Running` leads only to `Ejecting` or `Done`, and `Done` is left only through `edit` or `back_to_editing`, both of which clear `shown`, so a list from before an import is never shown. `can_start` and `marks_only` unchanged. Nine tests cover every listed case; no existing test changed. Test inventory checked against `test-output.txt` (2303 passed, 78 ignored; pw-loupe 1226 -> 1235). `test-output.txt` newer than every change, check.ps1 all OK, no clippy warnings (the agent's run). One nit left: the dimmed list cannot be scrolled ("not interactive", as the task says). The owner's decision during the task (an edit that leaves `Done` clears the kept list) recorded in ADR 0010 §3.1's Preview row. Reviewer: ADR 0010 §3.1 Preview row, CHANGELOG (Fixed), README index, done list. The owner's hand check follows |
| 2026-11-22 | Done | Hand-checked by the owner: every point passed (typing in an IPTC field keeps the list in place; typing in the name template keeps it dimmed with "Previewing …" beside Search Again and shows the new names after the pause; Search Again keeps it dimmed until the scan is done; an invalid folder template, for example `{foo}`, shows its error and no list) |
