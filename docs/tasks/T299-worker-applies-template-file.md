# T299. The metadata worker applies a template file — Open

- **Why:** ADR 0017 U13-Q8, U13-Q27 and U13-Q28 (owner, 2026-11-24; part U13d). The right-click
  menu's **Apply Template** writes a template file to many images in one batch, through the
  metadata worker as "Apply to {n} Marked" does (T302, T303). The worker has a request for the
  editor's edits (`write_edits`) but none for a `TemplateFile`. And a template's + list on a
  property with an open IIM/XMP conflict is written today (both forms get the merged list), while
  the metadata window leaves such a list alone and says so (U12d-Q6); U13-Q28 makes the template
  do as the window does, for the import too.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/worker.rs`,
  `crates/pw-metadata/src/edit.rs`, `crates/pw-metadata/tests/template.rs`,
  `docs/test-inventory.md`.
- **Background (read before starting):** ADR 0017 §6.5 U13-Q5, U13-Q12, U13-Q19, U13-Q28.
  `edit.rs`: the module comment, `write_edits_safely` (how `entries_left_alone` and
  `ConflictKept` are made), `apply_template_file_safely`, `has_open_conflict`,
  `restore_qualified`. `worker.rs`: `Request::WriteEdits`, `MetadataWorker::write_edits`,
  `Event::EditsWritten`, `WriteHeader`, `Write` (`from_request`, `into_request`, the dispatch
  that calls `write_edits_safely`), `handle_write`, and the tests in its `mod tests`
  (`edits_result`, `without_a_setup_edits_fail_and_the_file_is_unchanged`,
  `an_old_exiftool_refuses_edits_with_the_typed_kind`). `tests/template.rs` (its helpers) and
  `tests/edit.rs`'s `an_open_keyword_conflict_is_left_alone_and_reported` (how an open keyword
  conflict is made in a JPEG).
- **What:**
  1. **A + list with an open conflict is left alone** (`edit.rs`, `apply_template_file_safely`,
     U13-Q28): after the template is applied, each property in the filled template's `add`
     that has an `Open` conflict in the image as read (`has_open_conflict` on the metadata before
     the template), whether or not the template changed it, is put back to its value before the template
     (`Metadata::restore`, as `restore_qualified` does) and named in a
     `WriteNotice::ConflictKept { property }`, also when the report is `Unchanged`, never twice.
     A property the template replaces (not in `add`) is not touched by this: it decides the
     conflict, as typing does. The source is the filled template's `add` (`TemplateFile::new`
     keeps an id there only when its field survived filling); `entries_left_alone` knows only
     `Edit::Entries`, so a small helper over `add` is needed. The step comes after
     `template::apply` and before `restore_qualified`, so a list that has both an open conflict
     and qualifiers is named only in its `ConflictKept` (it was left alone for the conflict). The
     order of the notices: the plan's, then the `ConflictKept` of this point, then the
     `QualifiersKept` (as `write_edits_safely` orders them); a property is named once even when
     the plan also names it. The doc comments of `apply_template_file_safely`, of the module and
     of `EditReport` (which says `ConflictKept` is "edits only") say so.
  2. **The worker's request** (`worker.rs`): `MetadataWorker::apply_template(path, template:
     TemplateFile) -> Result<RequestId, SubmitError>`, queued without blocking as
     `write_edits` is, answered with `Event::EditsWritten { id, path, result }` (the same event
     and `EditReport`, so the shell's batch can take both kinds). It is a write like the others
     (`Write::from_request`, `into_request`, the write pool, the order of one image's writes,
     the setup's checks and journal, the options), run with `apply_template_file_safely` and
     the worker's current `EditOptions`. A `TemplateFile` is kept in the write as its edits
     are (a new field or a variant; the agent chooses, without cloning the template more than
     once per request). The places to change: a `Request` variant, its arm in `handle` (which
     builds the `Write` itself, as the rating and orientation arms do), `Write::from_request`
     and `into_request`, a `WriteHeader` variant (or the `Edits` one reused) with `path()` and
     `failed()`, and `Write::run`, which calls the safe write. Without a setup, or with an
     ExifTool too old, it fails as an edit write does. The module comment (its list of the
     writes the pool runs) and the `Write` struct's doc name the new request.
- **Tests to add:**
  - `tests/template.rs` (real ExifTool, as the file's other tests): a JPEG with XMP keywords
    `sol`, `hav` and IIM Keywords `sol`, `is` without a checksum (an `Open` conflict on
    `dc:subject`): (a) a template with only Keywords `vinter` and `dc:subject` in `add` writes
    nothing, its report is `Unchanged` with exactly one `ConflictKept { "dc:subject" }` (compared
    as the whole list), and both forms are as before; (b) the same template with also Headline
    `Ny` writes the Headline, keeps both keyword forms and names the conflict once (count the
    `ConflictKept` notices for `dc:subject`; the plan gives other notices too, so the whole list
    is not compared, as `tests/edit.rs`'s conflict test does); (c) a template with Keywords `vinter`
    **without** + replaces the list in both forms and gives no `ConflictKept` for `dc:subject`.
  - `worker.rs` `mod tests`: `apply_template` without a setup fails with the edit write's error
    and leaves the file unchanged; with the fake ExifTool too old it fails with
    `WriteError::ExifToolTooOld` (as the two edit tests named above); and in the `pooled`
    module, as `two_writes_of_one_image_answer_in_order` does (the `slow` fake, two or more
    processes, the event ids of the answers), an `apply_template` and a `write_edits` of one
    image answer in the order sent.
  - The test inventory's `pw-metadata` rows (the `src/` row for `worker.rs` and the
    `tests/template.rs` row; both lines are long, so they are changed by script, `CLAUDE.md`
    §4) and totals are updated.
- **Not in this task:** anything in `pw-loupe` (T302 calls the new request); a change to how
  `write_edits_safely` or the import's write treat conflicts beyond point 1; a `ConflictKept` for
  a replacing field.
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** may run beside T300 (other crate). T302 waits for it.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No hand check of its own (the right-click menu's, after T303, covers it). At Done the reviewer
  adds to the CHANGELOG (Changed) that a template's + list is no longer written where IIM and XMP
  disagree, and is named in the import's summary instead.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0017 U13-Q8, U13-Q27, U13-Q28, owner, 2026-11-24; part U13d). Waits for its cold read and the chain review of T299–T303 |
| 2026-11-24 | Blocked | Cold read (fresh agent): fits the named files; the import already lists every notice (`template_notice_text`), so a `ConflictKept` reaches its summary. Fixed: the worker's places named (`handle`, `from_request`/`into_request`, `WriteHeader`'s `path` and `failed`, `Write::run`); the order test in the `pooled` module (the fake cannot write); test (b) counts the `ConflictKept`; `EditReport`'s doc and the module's list of writes; the step's place before `restore_qualified` (a list with a conflict and qualifiers named only as a conflict kept); `add` of the filled template as the source |
| 2026-11-24 | Open | Chain review (T299–T303, one fresh agent, after each task's cold read): no misrouting on the template thread (one id counter); the hand-offs fixed: the names T301 and T303 call made fixed in T300 and T302, T303 removes T302's `dead_code` attributes (`metadata.rs` in its files), repaints while T301's and T303's waits and the batch run, Cancel kept enabled in T301 so a thread that never answers cannot hold the modal window; a right-click batch runs while the main window is free: moves, deletes, a stopped thread or worker, no undo and an import's backup tabs go to `known-limits.md` at T303's Done; U13-Q24's known limit at T300's Done; U13-Q25's and U13-Q27's wording settled in ADR 0017 to match T301 and T303; T303's hand check widened (U13-Q28 also through an import). Set Open; may run beside T300 |
