# T294. The import writes a template file instead of its six fields — Blocked

- **Why:** ADR 0017 U13, part U13c (U13-Q5, U13-Q6, U13-Q9; U13-Q18 and U13-Q19), fourth of the chain T291 → T292 → T293 → T294 → T295 → T296. The import dialog's six
  template fields and their modes (ADR 0007 D13) are replaced by a template file (U13-Q6). This
  task changes the import's worker and form: the worker is given the template already read
  (a `TemplateFile`) and writes it to each copied image with
  `ExifTool::apply_template_file_safely` (T288), which fills the variables per image and gives
  the write's notices; the six fields, their modes and the "Clear fields?" question go. The
  dialog's new row, the remembered file and the read at Start come in T295; until then an import
  writes no template (T294 and T295 are done one after the other, with no push or release
  between).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`,
  `crates/pw-loupe/src/import_worker.rs`, `crates/pw-loupe/src/import_dialog.rs`,
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/main.rs`,
  `crates/pw-loupe/src/lib.rs`, `docs/test-inventory.md`.
- **Background (read before starting):** `import.rs` (`ImportForm`, `IptcForm`, `IptcField`,
  `IptcModes`, `allowed_modes`, `template_field`, `to_template`, `template_is_empty`,
  `cleared_fields`, `to_iptc`); `import_worker.rs` (`ImportWorker::run`, `ImportRun`,
  `run_import`, `ShotWriter` with `write_template` and `submit_template`, `TemplateAnswer`,
  `ImportDone::iptc_errors`, its tests and their helper `creator_template`);
  `import_dialog.rs` (the comparison that leaves `iptc` and `eject` out of what the preview
  reads, and its tests that edit `form.iptc`); `import_text.rs` (`IptcField::label`,
  `mode_text`, the `clear_*` texts, the `ImportText` texts of the six fields, the done lines);
  `main.rs` (`request_import_start`, `import_clear_confirm` and its window, the six fields'
  block in `draw_import_dialog`); `pw_metadata::edit`
  (`apply_template_file_safely`, `EditReport`, `EditError`); `editor.rs`'s `notice_text`.
- **What:**
  1. **The form** (`import.rs`): `ImportForm::iptc` is replaced by
     `template: Option<PathBuf>`, the template file chosen (`None`: no template; T295 sets it).
     `IptcForm`, `IptcField`, `IptcModes`, `allowed_modes`, `template_field` and the
     `CONTACT` constant go, with their exports in `lib.rs`; so do the tests in `import.rs` that
     test only them, with their helpers (`filled_form`, `template_field_of`, `text`) and the
     `clear_confirmation_text` import: `iptc_form_maps_copyright_to_x_default_and_empty_form_is_empty`,
     `default_modes_give_the_template_of_to_iptc`, `to_template_uses_each_fields_mode`,
     `copyright_marks_the_file_with_replace_append_and_prefix`, `allowed_modes_of_each_field`,
     `modes_and_texts_are_reached_by_field`, `template_is_empty_when_no_field_does_anything`,
     `cleared_fields_and_their_confirmation_text` (each named in the report).
     `form_defaults_match_the_ingest_templates` builds `template: None` in place of
     `iptc: IptcForm::default()` (an intended change).
  2. **The worker's template** (`import_worker.rs`): `ImportWorker::run(&mut self, form:
     ImportForm, options: EditOptions, template: Option<TemplateFile>)` (the new argument last;
     T295 reads the template before the run; the worker never reads a template file); `Request::Run`
     and `ImportRun` carry it, `ShotWriter::new` takes it in place of `&IptcForm`, and
     `ShotWriter::finish` also gives the notices (point 4). `ShotWriter` keeps it in place of today's `Template` and
     writes it to each target exactly where it writes today's (the same targets, the same
     journal and ExifTool rules, one process or the pool), with
     `apply_template_file_safely(target, &template, &options, checks, journal)` in place of
     `apply_template_safely`. A template whose `fields` are empty is the same as `None` (nothing
     written, ExifTool not started).
  3. **Failures:** a failed write is one line `"{file}: {error}"` in `iptc_errors` as today
     (`EditError`'s text; a date variable on an image without a capture time names its reason,
     U13-Q9 amended, and the other images are written).
  4. **Notices** (U13-Q19; the known limit "the import's template drops a change to
     such a list without a notice"): each notice of a write's `EditReport::notices` (a list with
     qualifiers kept, a text cut in IIM, …) is one line `"{file}: {notice}"`, made by a new
     function in `import_text.rs` (for example `template_notice_text(file: &str, notice:
     &WriteNotice) -> String`, built on `editor.rs`'s `notice_text`; rule 13), in a new
     `ImportDone::template_notices: Vec<String>`, in the order the answers arrive (with one
     process, the order of the writes; tests of the pool assert no order). With the
     pool, the pool's answer type carries the notices (`TemplateAnswer` becomes
     `Result<Vec<WriteNotice>, String>` or similar). `import_text.rs`'s done lines show at most
     ten of them after the `IPTC:` lines, each as a normal line (not an error) with no prefix;
     with more, one normal line `… and {n} more` (n the count less ten). The `IPTC:` error lines
     stay as they are. The Tasks area (`tasks.rs`) shows the failures and IPTC errors only, as
     today; the notices are shown in the import dialog only (intended, `tasks.rs` is not changed).
  5. **The dialog** (`import_dialog.rs`): the comparison of what the preview reads leaves
     `template` out where it left `iptc` out (choosing another template never rescans). The
     tests that edit `form.iptc` to show that an edit the preview does not read keeps the
     preview are changed to set `form.template` to a path instead, with the same purpose and the
     same assertions (an intended change, named in the report).
  6. **The "Clear fields?" question goes** (`main.rs`): `request_import_start` starts the
     import at once (no field can clear any more); the field `import_clear_confirm` and every
     use of it go (its window with the confirm and cancel handling, the dialog's `confirming`
     and `idle` checks, its `DialogKey` arms, its initialiser). `draw_import_dialog`'s block of
     the six fields (the collapsing "IPTC Template" with the fields and their modes, and its
     note) goes, with nothing in its place until T295; the `use pw_metadata::template::Mode;`
     that only it used goes. `start_import` passes `None` as the template until T295.
  7. **Texts** (`import_text.rs`): the texts used only by the removed code go, with their
     exports in `lib.rs` and imports in `main.rs`: `IptcField::label`, `mode_text`,
     `clear_confirmation_title`, `clear_confirmation_text`, `clear_and_import_text`,
     `clear_cancel_text`, and the `ImportText` variants `IptcTemplate`, `CopyrightHint` and
     `IptcNote`; the done lines show `template_notices` (point 4).
- **Tests to add or change:**
  - the import worker's tests that set `form.iptc.creator` (or other `iptc` fields) to make the
    import write a template now pass a `TemplateFile` of the same field to `run` (`dc:creator`
    `["Fotograf"]`); `creator_template` and the tests' other template helpers return a
    `TemplateFile`; every other `run`, `ImportRun { .. }` and `ShotWriter::new` in the tests
    passes `None` (the inline `IptcForm` of `cancellation_before_iptc_writes_nothing` becomes
    the same `TemplateFile`), and the tests that take `finish()`'s value take the new one; the
    assertions stay as they are (an intended change, named in the report);
  - `import_text.rs`: `mode_texts_and_field_labels` goes (it tests only removed texts), and
    `every_moved_import_text_keeps_its_wording` loses the entries of `IptcTemplate`,
    `CopyrightHint` and `IptcNote` (intended, named in the report);
  - the two tests of modes (`iptc.modes.email = Clear`, `iptc.modes.creator = Append`) test
    behaviour that no longer exists: Clear is removed with this test; the Append test becomes a
    test that a template with Keywords and its + box adds to an imported JPEG's keywords (with
    ExifTool on PATH, as `the_template_is_written_safely_to_an_imported_jpeg`); both named in
    the report;
  - with ExifTool on PATH: a template with Headline `{name}` gives each imported JPEG its own
    file name without the extension;
  - `None`, and `Some` of a `TemplateFile` whose `fields` are empty, each start no ExifTool and
    give no `iptc_errors` and no `template_notices` (as `empty_iptc_template_does_not_start_exiftool`,
    renamed for it);
  - with ExifTool on PATH: a template whose Headline is longer than IIM's 256 bytes, imported
    with IIM created (as `import_creator_with`), gives one `template_notices` line naming the
    file and the cut, with one write process and with two;
  - with ExifTool on PATH: Headline `{date}` on two JPEGs, one whose DateTimeOriginal was removed
    first, gives one `iptc_errors` line naming that image and the reason, and the other image is
    written;
  - `import_text.rs`: `template_notice_text` (exact); the done lines with three notices show them
    after the `IPTC:` lines as normal lines; with twelve, ten and a normal `… and 2 more`.
- **Not in this task:** the dialog's Metadata Template row, `settings.txt`, reading the file at
  Start and its errors, Edit… (T295); removing `apply_template_safely`, `Iptc` and the modes in
  `pw-metadata` (T296); the right-click submenu (U13d).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Order:** runs after T293 is committed, never beside it (both change `main.rs`'s key loop,
  `lib.rs` and the test inventory's `pw-loupe` row).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No CHANGELOG line of its own (the user's change is T295's line). At Done the reviewer updates
  ADR 0007 (the line "Until U13's tasks are done, the dialog keeps D13", and D14, the question
  before a Clear, marked replaced by U13). At Done the reviewer also removes the known limit "the import's template drops a change to such a
  list without a notice" from `known-limits.md` (point 4).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, fourth of the chain T291 → T292 → T293 → T294 → T295 → T296). Needs nothing of T291–T293 (other files except `main.rs`'s import dialog, which T293 does not touch). Waits for the owner's answers to U13-Q18 and U13-Q19, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: the tests that go or change are named (eight in `import.rs`, `form_defaults_match_the_ingest_templates`, two in `import_text.rs`, the worker tests' `run`/`ImportRun`/`ShotWriter::new`/`finish` call sites); `mode_text` and the `clear_*` texts are in `import_text.rs`, all listed with their exports, the `Mode` import and every use of `import_clear_confirm`; `run`'s signature and the worker's plumbing named; the notice line made by `template_notice_text` in `import_text.rs` (rule 13), at most ten, a normal "… and {n} more", the IPTC error lines unchanged; notices in the order answers arrive; tests added for the notices (one and two processes), for a date variable failing on one image, and for both `None` and an empty template; `tasks.rs` unchanged on purpose |
