# T296. The ingest template's modes removed from pw-metadata — Open

- **Why:** ADR 0017 U13, part U13c (U13-Q5: a template has no Keep, Append, Prefix or Clear;
  plan notes of U13: "U13c … removes `apply_template_safely` and the modes if nothing else uses
  them"), last of the chain T291 → T292 → T293 → T294 → T295 → T296. After T294 the import
  writes a `TemplateFile` with `apply_template_file_safely`, and nothing in the program uses the
  ingest template of ADR 0007 D13 any more: `Iptc` and `LocalizedValue` (`lib.rs`),
  `ExifTool::apply_template_safely` (`edit.rs`) and the modes Keep, Prefix and Clear of
  `template.rs`. `TemplateFile::to_template` still uses `Template`, `TemplateField`, `apply` and
  the modes Replace and Append, which stay. Code that nothing uses is removed, so the next reader
  of `template.rs` does not keep rules no template can reach (the copyright-management guard of
  Clear, Prefix's join, an empty Keep).
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/lib.rs`,
  `crates/pw-metadata/src/template.rs`, `crates/pw-metadata/src/edit.rs`,
  `crates/pw-metadata/src/template_file.rs` (comments only),
  `crates/pw-metadata/tests/template.rs`, `crates/pw-metadata/tests/edits.rs` (point 6's two
  Clear tests only), `docs/glossary.md` (point 9 only), `docs/test-inventory.md`.
- **Background (read before starting):** `lib.rs` (`Iptc`, `LocalizedValue`, `Iptc::tags`,
  `is_empty`, `to_template`, their unit tests `to_template_tests` and the others on `Iptc`);
  `template.rs` (the module comment, `Mode`, `apply`, the join of Append and Prefix, the Clear
  rules, its unit tests); `edit.rs` (`apply_template_safely` and the module comment's mentions
  of it); `template_file.rs` (`to_template`); `tests/template.rs` (the tests of
  `apply_template_safely` with `Iptc`, and those of `apply_template_file_safely`).
- **What:**
  1. **First check** (before changing anything): with `grep` over `crates/` (sources, tests,
     examples), nothing outside the files above uses `Iptc`, `LocalizedValue`,
     `apply_template_safely`, `Mode::Keep`, `Mode::Prefix` or `Mode::Clear`. If something does,
     stop and report it; do not change that file. A hit in `pw-loupe` (for example
     `import_text.rs`'s `mode_text`) means T294 is not complete: report it to the reviewer.
  2. **`lib.rs`:** `Iptc` and `LocalizedValue` go (both are defined at the crate root), with
     their `impl`s, the private `CONTACT` constant only `Iptc::to_template` uses, `BTreeMap` in
     the `std::collections` import if nothing else uses it, and the test modules
     `to_template_tests` and `localized_iptc_tests` and any other test only of them (each
     named in the report). `pub const X_DEFAULT` goes only if nothing in the workspace uses it
     any more (grep); otherwise it stays. In `edit_error_tests`,
     `unsupported_value_plan_and_template_are_other_with_the_error_text` uses
     `TemplateError::CannotClear`, which goes: it uses `TemplateError::UnknownProperty("foo:Bar")`
     instead, with its text (an intended change, same purpose, named in the report).
  3. **`edit.rs`:** `ExifTool::apply_template_safely` goes, and the `Template` import if it is
     then unused. The module comment describes only `apply_template_file_safely` for templates:
     its paragraph states itself what it took from `apply_template_safely` (which metadata is
     not supported for which files; a template already in the file gives `Unchanged`), the
     sentence that `apply_template_safely` fills no variables goes, and `write_edits_safely`'s
     reference to `apply_template_safely` names `apply_template_file_safely`. Its test
     `errors_are_shown_in_english` asserts the `CannotClear` text: that part uses
     `UnknownProperty` instead (intended, named in the report).
  4. **`template.rs`:** `Mode` keeps `Replace` and `Append`; `Keep`, `Prefix` and `Clear` go,
     with the code only they reach (Prefix's join, the Clear of a property or a field, the
     copyright-management guard of Clear, Keep's early return) and the error variants only they
     can give. Everything `TemplateFile::to_template` gives keeps working exactly as before:
     Replace of a field, of a Lang Alt's `x-default`, of a list, of a whole structure list, of a
     field of a single structure, of a field of Location Created in its first entry; Append of a
     list; an empty value changes nothing; a value equal to the current one is not set. The
     module comment is rewritten for these two modes (no mention of the ingest template's six
     fields or of the confirmation before a Clear). **Append on a text or a Lang Alt stays**
     (reviewer's choice: `TemplateFile::to_template` gives Append only for a list today, but the
     join of D5 is the mode's own rule, not a removed mode's; it keeps its tests and the D5 text
     in the module comment, without Prefix).
  5. **The tests of `template.rs`:** the tests of a removed mode go (each named in the report);
     the tests of Replace and Append stay as they are. If a test exercises Replace or Append
     together with a removed mode, only the removed mode's part goes, and the report says so.
     Expected among them: `clear_without_a_value_changes_nothing`,
     `clearing_the_last_field_clears_the_structure` and
     `copyright_management_properties_cannot_be_cleared` go whole; `every_mode_on_a_text`,
     `every_mode_on_a_lang_alt`, `every_mode_on_a_list`,
     `every_mode_on_a_field_of_creator_contact_info`,
     `append_and_prefix_without_an_old_text_give_the_new_text`,
     `keep_and_empty_values_change_nothing`, `modes_that_are_not_allowed`,
     `only_replace_on_a_field_of_location_created_is_allowed` and
     `a_template_equal_to_the_file_changes_nothing` lose their removed modes' parts. In
     `errors_are_shown_in_english`, the `ModeNotAllowed { mode: Mode::Prefix }` assertion uses
     `Mode::Append` with its changed text, and the `CannotClear` part goes. A test whose name
     names a removed mode is renamed (for example `a_field_of_a_list_of_structures_allows_only_keep`).
     Each is named in the report.
  6. **`tests/template.rs`:** each test of `apply_template_safely` either goes, when an equal
     test of `apply_template_file_safely` exists in the file (both named in the report), or is
     moved to `apply_template_file_safely` with a `TemplateFile` of the same values (named in
     the report), so what was tested of the safe write (a JPEG, a RAW's sidecar, the journal, the
     image checks, an unchanged file not written) is still tested. The mapping:
     - `append_adds_a_keyword_at_the_end` goes (covered by
       `a_saved_template_file_is_read_back_and_applied`);
     - `the_ingest_template_is_written_to_a_jpeg_and_read_back`, the Exif copyright test,
       `a_raw_gets_only_its_sidecar_written` and `a_template_keeps_an_earlier_rating_in_a_sidecar`
       are moved; the ingest template's `TemplateFile` holds `dc:creator` `[name]`, `dc:rights`
       x-default, `photoshop:Credit`, `Iptc4xmpCore:CreatorContactInfo` with its three fields and
       **`xmpRights:Marked` `True` given explicitly** (`Iptc::to_template` set it by itself, D6;
       a `TemplateFile` does not); the empty template is `TemplateFile::new(vec![], vec![])`;
     - `a_saved_template_file_is_read_back_and_applied` calls `apply_template_file_safely` with
       the `TemplateFile` read (in place of `to_template` and `apply_template_safely`);
     - the assertions of an `ApplyReport` become `assert_edit_report(…, &[])` (no notices), the
       same files and values asserted;
     - `clear_removes_city_from_xmp_and_iim` and
       `clear_of_the_only_xmp_value_keeps_the_checksum_current` (the only real-ExifTool tests
       that a cleared property leaves IIM too, and that the checksum stays current when no XMP
       is left) move to `tests/edits.rs` as `write_edits_safely` with
       `Edit::Clear { property: "photoshop:City", field: None }`, with the same setup and
       assertions;
     - the module comment (the ingest template with the modes of ADR 0007 §10) is rewritten,
       and what only the removed tests used goes (`Scratch::apply`, `one_field`,
       `ingest_template`, `assert_report` if unused, the imports of `Mode`, `Template`,
       `TemplateField`, `Iptc` and `LocalizedValue`).
  7. **`template_file.rs`:** its comments name `apply_template_file_safely` where they name
     `apply_template_safely`; no code change.
  8. `known-limits.md`'s tidying item of `edit.rs` and `template.rs` (the shared list-append
     helper and the others) is **not** done here; the report says whether this task made any of
     it moot (for example a blank test that was only Prefix's), so the reviewer can update it.
  9. **`docs/glossary.md`** (owner, 2026-11-23, at T294's review): the two rows of texts T294
     removed from the import dialog go: "Behåll, Ersätt, Lägg till efter, Lägg till före, Rensa
     | Keep, Replace, Append, Prefix, Clear" (the template modes) and "Rensa och importera |
     Clear and Import". Nothing else in the glossary changes.
- **Beside T295:** it may run beside T295 (other crate); both change the test inventory's
  totals, and the second of the two to be reviewed has its totals counted again by the reviewer.
- **Tests to add:** none new; point 6 keeps the safe write's coverage. The test inventory's
  counts and descriptions of `pw-metadata`'s tests are updated.
- **Not in this task:** anything in `pw-loupe` (T294 removed its use); the tidying of point 8;
  ADR 0007 §10's text (the reviewer records there that the modes are gone).
- **Rules:** `docs/tasks/README.md` rules 1–13 (rule 5: the tests removed or moved here are the
  ones this task names, as their code goes); no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  An extra code review (`pw-metadata`'s template path; `docs/tasks/README.md`) before the owner
  is asked to approve.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, last of the chain T291 → T292 → T293 → T294 → T295 → T296). Waits for T294, its cold read and the chain review. No user-visible change, no CHANGELOG line |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: the two Clear integration tests (IIM City cleared, checksum without XMP) have no `TemplateFile` form, so they move to `tests/edits.rs` with `Edit::Clear` (file added); the two tests of `CannotClear`'s text (`lib.rs` `edit_error_tests`, `edit.rs` `errors_are_shown_in_english`) use `UnknownProperty`; `tests/template.rs`'s mapping decided (which test goes, which moves, `xmpRights:Marked` given explicitly, `a_saved_template_file_is_read_back_and_applied` included), the `template.rs` unit tests that go or lose parts named; Append on a text or Lang Alt stays (reviewer's choice, not asked); `lib.rs`'s `CONTACT`, `BTreeMap`, both test modules, `X_DEFAULT` only if unused; `edit.rs`'s module comment's three places; a `pw-loupe` hit means T294 is incomplete (T294 now removes `mode_text`) |
| 2026-11-23 | Open | T294 done and committed; its cold read and the chain review were done before (see T291's log). Point 9 added (owner, at T294's review): the glossary rows of the modes and "Clear and Import". Set Open; may run beside T295 |
