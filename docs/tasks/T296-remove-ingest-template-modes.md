# T296. The ingest template's modes removed from pw-metadata — Blocked

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
  `crates/pw-metadata/tests/template.rs`, `docs/test-inventory.md`.
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
     stop and report it; do not change that file.
  2. **`lib.rs`:** `Iptc` and `LocalizedValue` go, with their `impl`s, re-exports and unit
     tests (each test named in the report).
  3. **`edit.rs`:** `ExifTool::apply_template_safely` goes; the module comment describes only
     `apply_template_file_safely` for templates.
  4. **`template.rs`:** `Mode` keeps `Replace` and `Append`; `Keep`, `Prefix` and `Clear` go,
     with the code only they reach (Prefix's join, the Clear of a property or a field, the
     copyright-management guard of Clear, Keep's early return) and the error variants only they
     can give. Everything `TemplateFile::to_template` gives keeps working exactly as before:
     Replace of a field, of a Lang Alt's `x-default`, of a list, of a whole structure list, of a
     field of a single structure, of a field of Location Created in its first entry; Append of a
     list; an empty value changes nothing; a value equal to the current one is not set. The
     module comment is rewritten for these two modes (no mention of the ingest template's six
     fields or of the confirmation before a Clear).
  5. **The tests of `template.rs`:** the tests of a removed mode go (each named in the report);
     the tests of Replace and Append stay as they are. If a test exercises Replace or Append
     together with a removed mode, only the removed mode's part goes, and the report says so.
  6. **`tests/template.rs`:** each test of `apply_template_safely` either goes, when an equal
     test of `apply_template_file_safely` exists in the file (both named in the report), or is
     moved to `apply_template_file_safely` with a `TemplateFile` of the same values and the same
     assertions (named in the report), so what was tested of the safe write (a JPEG, a RAW's
     sidecar, the journal, the image checks, an unchanged file not written) is still tested.
  7. **`template_file.rs`:** its comments name `apply_template_file_safely` where they name
     `apply_template_safely`; no code change.
  8. `known-limits.md`'s tidying item of `edit.rs` and `template.rs` (the shared list-append
     helper and the others) is **not** done here; the report says whether this task made any of
     it moot (for example a blank test that was only Prefix's), so the reviewer can update it.
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
