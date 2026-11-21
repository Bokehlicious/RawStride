# T258. ADR 0026 F4: the import writes the template per shot and says which images it has put in place (pw-loupe import worker) — Done

- **Why:** ADR 0026 §3.4 (F-Q11, and what the owner settled on 2026-11-21): the destination tabs
  open on the first image and fill as the import goes on, so each image must have its template
  before it is shown. Today `run_import` (`import_worker.rs`) copies every file with
  `pw_ingest::execute_recording`, then writes the template to all copies (`apply_iptc`), then sends
  `Done`. This task moves the template write into the copy: when all files of a shot are copied
  and verified, the template is written to the shot's images, and an event names the shot's
  destination images. Before copying, an event names every destination image the import will put
  in place, so the shell can leave out the ones not shown yet when it lists such a folder (T260).
  The separate progress step "Writing IPTC n / total" goes (the owner). First task of the import
  part, T258 → T259 → T260; the shell does not act on the new events until T260.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/import_worker.rs` (points 1–5, and its tests)
  - `crates/pw-loupe/src/import_dialog.rs` (point 6)
  - `crates/pw-loupe/src/tasks.rs` (point 6)
  - `crates/pw-loupe/src/tasks_text.rs` (point 6: only its module doc comment, which names
    `iptc_progress_text` and the import's IPTC step)
  - `crates/pw-loupe/src/main.rs` (point 6)
  - `crates/pw-loupe/src/import_text.rs` (point 6)
  - `crates/pw-loupe/src/lib.rs` (point 6, the re-export)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **Two new events.** `ImportEvent` gains, with doc comments:
     - `Planned { id: RequestId, images: Vec<PathBuf> }`: sent once by `run_import` after the plan
       is made and before the first copy: for each plan item with `to_dest` that is not a
       companion (`pw_ingest::COMPANION_EXTENSIONS`, compared as `iptc_targets` does today),
       `options.dest.join(&item.rel)`, in plan order. Sent also when the list is empty. Not sent
       when the import ends before planning (a validation or scan error).
     - `Placed { id: RequestId, images: Vec<PathBuf> }`: one shot's destination images now in
       place and given the template (point 3), in plan order; never sent empty.
  2. **The shots.** A plain function `copy_shots(plan: &pw_ingest::Plan) -> Vec<Range<usize>>`,
     with a doc comment: the items `execute_recording` copies (`to_dest || to_backup`), in plan
     order and numbered from 0, grouped into ranges of consecutive items whose
     `rel.with_extension("")` is equal (a shot's files share their folder and name, `pw_ingest::plan`;
     two shots next to each other that got the same name with other extensions, possible when
     the name template has no `{seq}`, fall into one range, which only shows them together;
     say so in the doc comment).
     `execute_recording` calls its progress closure once after each such item with
     `files_done` raised by one (also when the item failed), and not for an item interrupted by a
     cancel; so after the call with `files_done == range.end` the shot of `range` is finished.
  3. **At the end of each shot**, inside the progress closure given to `execute_recording`, after
     the `Progress` event is sent as today:
     - an item of the shot counts as copied when every target it has exists (`dest.join(rel)`
       when `to_dest`, the backup's when `to_backup`), as `report.copied` counts an item only when
       all its targets succeeded (a target that failed because another program made a file of
       that name during the copy also exists; this rare case gets the template and is shown,
       accepted, the reviewer notes it in `known-limits.md`); an item whose destination copy
       succeeded but whose backup copy failed does not count as copied: it gets no template and
       is not in `Placed`, so its destination file stays left out of the tab as waiting (T260)
       and appears by the listing after the import, as a cancelled shot's files do;
     - when `cancel` is set (checked at the shot's end and, as `apply_iptc` does today, again
       before each target's write), nothing more is done for this shot or any later one: no
       template, no further write,
       no `Placed` (the owner, 2026-11-21: the files are shown by the listing after the import,
       without the template); the import is reported cancelled as today;
     - otherwise the template is written, when it is not empty (`template_is_empty`), to each
       copied item that is not a companion: its destination target (when `to_dest`) and its
       backup target (when `to_backup`), in that order, exactly as `apply_iptc` writes one target
       today (the journal check, ExifTool started once and kept in `exiftool`, the version check,
       `apply_template_safely` with the form's template and the import's `EditOptions`, each
       failure as one line in `iptc_errors`). When the journal is missing or ExifTool cannot be
       started or is too old, that is listed once, as today, and no template is written to any
       later shot of this import; the copy and the `Placed` events go on;
     - then `Placed` is sent with the destination targets of the shot's copied items that are
       not companions and have `to_dest`, when there is at least one. A shot whose template write
       failed is shown anyway (§3.4: "a file whose template write fails is shown anyway and is
       listed in the error list").
  4. **What goes.** `apply_iptc` and `iptc_targets` are replaced by point 3 (a helper for one
     shot is fine); the template is no longer written after `execute_recording`. `ImportDone`
     keeps every field (`new_folders` included, still filled by `new_folders`); `iptc_errors` and
     `cancelled` are filled from point 3 (`cancelled` is `report.cancelled` or a cancel seen in
     point 3: a cancel after the last file's progress call is not in `report`). The eject and the rest of `run_import` are unchanged.
  5. **No progress event for the template.** `ImportEvent::IptcProgress` is removed; nothing is
     sent while a template is written (the `Progress` event of the shot's last file stays the
     last one shown).
  6. **The shell without the IPTC step** (the owner, 2026-11-21: the progress shows only the
     copy): `ImportDialogState::Running` loses its `iptc` field and `ImportDialog::apply` its
     `IptcProgress` arm (`import_dialog.rs`); `ImportStep::Iptc` and its arm in `ImportTasks`
     go (`tasks.rs`); in `main.rs`, the dialog's progress bar and text and
     `hidden_import_status` use only the copy's progress, and `draw_import_row` loses its
     `ImportStep::Iptc` arm; `iptc_progress_text` is removed from `import_text.rs` and from the
     re-export in `lib.rs`. `Planned` and `Placed` change nothing in the dialog or the Tasks area
     (they fall into the existing "other event" arms; add arms only if a `match` needs them).
- **Tests to add** (`import_worker.rs`, with its `TempDirs`, fake and real ExifTool helpers):
  - `copy_shots`: a plan with a RAW, its JPEG and its card sidecar, then another shot, gives two
    ranges; an item not copied (present) is left out and does not split a shot; an empty plan
    gives none;
  - a run over two shots (no template): one `Planned` before the first `Progress`, with the two
    shots' destination images in plan order and no companion; then for each shot, after its last
    `Progress`, one `Placed` with its destination images only (no backup path, no companion), the
    first shot's `Placed` before the second shot's first `Progress`; then `Done` as today;
  - with a backup: `Planned` and `Placed` name only destination paths;
  - a shot with a file that cannot be copied (as the existing failure tests make one): the shot's
    `Placed` names only its copied images, or is not sent when none was copied; and a shot whose
    backup copy fails while the destination copy succeeds is not in any `Placed`;
  - a cancel set during a shot's template writes (for example by a fake ExifTool or a check
    hook, or by setting `cancel` from the progress closure in a test helper): no `Placed` for
    that shot, and `Done` says cancelled;
  - with the real ExifTool (Windows, `required_exiftool`, as the existing template tests): a RAW
    with a sidecar on the card (`A001.ARW` and `A001.xmp`): the sidecar is copied, the template is
    in the destination sidecar afterwards, nothing failed, and `Placed` names `A001.ARW` only;
    and the existing template tests still pass unchanged;
  - with the fake ExifTool that cannot start (as `unavailable_exiftool_reports_one_iptc_error_after_copying`):
    over two shots, one line in `iptc_errors`, both `Placed` events sent;
  - cancel: a run cancelled before its first shot is finished sends `Planned`, no `Placed`, and
    writes no template; the existing `cancel_during_many_file_run_leaves_no_partial_named_files`
    still passes.
  - **Existing tests:** the tests that call `apply_iptc` or `iptc_targets` directly
    (`iptc_targets_images_in_destination_and_backup_but_not_companions_or_failures`,
    `cancellation_before_iptc_writes_nothing`) may be rewritten for point 3's code, keeping what
    each one shows (companions and failed items get no template, the backup copy gets it; a
    cancel writes nothing and starts no ExifTool). The arms matching `IptcProgress` in the test
    helpers (`done_event`, `run_copies_reports_progress_and_summarizes_destination_folders`) are
    removed, and `Planned` and `Placed` accepted there. `import_dialog.rs`'s test of the
    `IptcProgress` event, `tasks.rs`'s `the_iptc_and_ejecting_steps` (keep its ejecting part)
    and `import_text.rs`'s test of `iptc_progress_text` lose what tests removed code. Name each
    such change in the report. Existing test code that only builds `ImportDialogState::Running`
    with `iptc: None` loses that field (a compile edit, not a change of what it tests); no other
    existing test is changed.
- **Not in this task:** opening and filling tabs (T259, T260); `pw-ingest` is not changed; the
  text of the summary and error list is unchanged.
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new texts; one removed); `AGENTS.md` §3.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added when F4 was written (ADR 0026 §3.4, the owner's choices of 2026-11-21); waits for T257 (the chain is run in order; it does not depend on T256–T257's code) |
| 2026-11-21 | Blocked | Cold read: doable as written, `execute_recording`'s progress calls and the shots' order confirmed in `pw-ingest`, the closure's borrows work; added `tasks_text.rs` (doc comment), the cancel check before each target, `cancelled` as `report.cancelled` or point 3's, the merged-name shots and the "appeared during copy" case (accepted, to `known-limits.md` at Done), and the `iptc: None` compile edits |
| 2026-11-21 | Blocked | Chain review: a late cancel reported (already fixed after the cold read: `cancelled` from point 3, checked before each target, no `Placed` for an interrupted shot), now also tested; a shot whose backup copy failed is not shown during the import (stated and tested) |
| 2026-11-21 | Open | T257 done; ready for its agent |
| 2026-11-21 | Ready for review | Done as written: `ImportEvent::Planned`/`Placed`, `copy_shots`, `shot_targets` and `ShotWriter` (the template written at each shot's end inside `execute_recording`'s progress closure); `apply_iptc`, `iptc_targets`, `IptcProgress`, `ImportStep::Iptc`, the dialog's `iptc` field and `iptc_progress_text` removed. 10 new tests in `import_worker.rs` (two need ExifTool on PATH and Windows); rewritten for point 3: `iptc_targets_…` as `shot_targets_…`, `cancellation_before_iptc_writes_nothing` on `ShotWriter`; removed with their code: `import_text.rs`'s `iptc_progress_uses_one_based_file_number_and_name`, the `IptcProgress` part of `import_dialog.rs`'s `progress_and_done_update_the_state`, the IPTC part of `tasks.rs`'s test (renamed `the_ejecting_step`); `iptc: None` compile edits. `tasks.rs`'s `running` doc comment no longer names IPTC. check.ps1 all OK (2232 passed, 76 ignored), no clippy warnings |
| 2026-11-21 | Done | Reviewed: points 1–6 as written; `copy_verified` (pw-ingest) leaves no target after a failed or cancelled copy, so "every target exists" holds; the 10 new tests (the two with the real ExifTool included) and the 2 rewritten seen passing in `test-output.txt` (newer than the change), no clippy warnings; the other existing-test edits only those the task allows. Extra code review (a fresh review agent, nine files and the import's template write): no bug; shot accounting against `execute_recording` (cancel mid-item, failed, backup-only items, the register), the cancel checks, the ExifTool failures and the removed IPTC step confirmed. Its two risks accepted by the owner (2026-11-21) and in `known-limits.md` under "Import: the template per shot": a file another program makes under a planned name after the plan (widened from "during the copy") gets the template and is placed; two non-adjacent shots of the same name can make a later card sidecar's copy fail. Its nits (the partial-failure test drives `ShotWriter` directly; the early-cancel test cancels before any copy; `tasks.rs`'s test renamed) left as they are. No change log line until T260 (the owner) |
