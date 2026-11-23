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
  `clear_confirmation_text`, the `ImportText` texts of the six fields, the done lines);
  `main.rs` (`request_import_start`, `import_clear_confirm` and its window, the six fields'
  block in `draw_import_dialog`, `mode_text`); `pw_metadata::edit`
  (`apply_template_file_safely`, `EditReport`, `EditError`); `editor.rs`'s `notice_text`.
- **What:**
  1. **The form** (`import.rs`): `ImportForm::iptc` is replaced by
     `template: Option<PathBuf>`, the template file chosen (`None`: no template; T295 sets it).
     `IptcForm`, `IptcField`, `IptcModes`, `allowed_modes`, `template_field` and the
     `CONTACT` constant go, with their exports in `lib.rs`; so do the tests in `import.rs` that
     test only them (each named in the report).
  2. **The worker's template** (`import_worker.rs`): `ImportWorker::run` takes the template as
     a new argument, `template: Option<TemplateFile>` (T295 reads it before the run; the worker
     never reads a template file). `ShotWriter` keeps it in place of today's `Template` and
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
     qualifiers kept, a text cut in IIM, …) is one line `"{file}: {notice}"` with `notice_text`,
     in a new `ImportDone::template_notices: Vec<String>`, in the order of the writes. With the
     pool, the pool's answer type carries the notices (`TemplateAnswer` becomes
     `Result<Vec<WriteNotice>, String>` or similar). `import_text.rs`'s done lines show at most
     ten of them after the `IPTC:` lines, each as a normal line (not an error), with the same
     "… and {n} more" rule as the failures when there are more.
  5. **The dialog** (`import_dialog.rs`): the comparison of what the preview reads leaves
     `template` out where it left `iptc` out (choosing another template never rescans). The
     tests that edit `form.iptc` to show that an edit the preview does not read keeps the
     preview are changed to set `form.template` to a path instead, with the same purpose and the
     same assertions (an intended change, named in the report).
  6. **The "Clear fields?" question goes** (`main.rs`): `request_import_start` starts the
     import at once (no field can clear any more); `import_clear_confirm`, its window and
     `clear_confirmation_text` go. `draw_import_dialog`'s block of the six fields (the
     collapsing "IPTC Template" with the fields and their modes, and its note) goes, with
     nothing in its place until T295; `mode_text` goes if nothing else uses it. `start_import`
     passes `None` as the template until T295.
  7. **Texts** (`import_text.rs`): the texts used only by the removed code go
     (`IptcField::label`, `clear_confirmation_text`, the `ImportText` variants of the six fields'
     block); the done lines show `template_notices` (point 4).
- **Tests to add or change:**
  - the import worker's tests that set `form.iptc.creator` (or other `iptc` fields) to make the
    import write a template now pass a `TemplateFile` of the same field to `run` (`dc:creator`
    `["Fotograf"]`); `creator_template` and the tests' other template helpers return a
    `TemplateFile`; the assertions stay as they are (an intended change, named in the report);
  - the two tests of modes (`iptc.modes.email = Clear`, `iptc.modes.creator = Append`) test
    behaviour that no longer exists: Clear is removed with this test; the Append test becomes a
    test that a template with Keywords and its + box adds to an imported JPEG's keywords (with
    ExifTool on PATH, as `the_template_is_written_safely_to_an_imported_jpeg`); both named in
    the report;
  - with ExifTool on PATH: a template with Headline `{name}` gives each imported JPEG its own
    file name without the extension;
  - a template whose `fields` are empty starts no ExifTool and writes nothing (as
    `empty_iptc_template_does_not_start_exiftool`, renamed for it);
  - `import_text.rs`: the done lines with three notices show them after the `IPTC:` lines as
    normal lines; with twelve, ten and "… and 2 more".
- **Not in this task:** the dialog's Metadata Template row, `settings.txt`, reading the file at
  Start and its errors, Edit… (T295); removing `apply_template_safely`, `Iptc` and the modes in
  `pw-metadata` (T296); the right-click submenu (U13d).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At Done the reviewer removes the known limit "the import's template drops a change to such a
  list without a notice" from `known-limits.md` (point 4).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, fourth of the chain T291 → T292 → T293 → T294 → T295 → T296). Needs nothing of T291–T293 (other files except `main.rs`'s import dialog, which T293 does not touch). Waits for the owner's answers to U13-Q18 and U13-Q19, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
