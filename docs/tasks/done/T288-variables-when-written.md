# T288. Variables filled when an image is written; applying a template file — Done

- **Why:** ADR 0017 U13, part U13b (U13-Q9, Q5, Q12). The variables of T287 are turned into each
  image's own values when the image is written: by the metadata window's edits (Save and "Apply to
  {n} Marked") and by a template. Filling needs the image's capture time, make and model, which
  are read with the image, so it happens between the read and the applying, inside
  `pw-metadata`. U13a's chain review asked for that step and for the template to be passed
  through `TemplateFile::new` again after filling (a `{make}` that comes out empty is then a field
  without content, which does nothing, ADR 0007 §10). This task also adds the function that
  applies a template **file** (`TemplateFile`), which U13c's import and U13d's Apply Template call.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/read.rs`,
  `crates/pw-metadata/src/edit.rs`, `crates/pw-metadata/src/template_file.rs` (`check_variables`,
  comments), `crates/pw-metadata/src/template.rs` (module comment only),
  `crates/pw-metadata/src/lib.rs` (`From<EditError> for WriteError` and its test),
  `crates/pw-metadata/tests/edits.rs`, `crates/pw-metadata/tests/template.rs`,
  `docs/test-inventory.md`.
- **Background (read before starting):** `variables.rs` as T287 left it (`VariableValues::of`,
  `fill_value`, `VariableError`). `read.rs`: `IMAGE_TAGS`, `read_metadata`, `exif_fields` (the
  `tag` and `exif_text` helpers it uses, with the fallback encoding). `edit.rs`:
  `write_edits_safely`, `apply_template_safely`, `apply_edits`, `entries_left_alone`,
  `restore_qualified`, `EditReport`. `template_file.rs`: `TemplateFile`, `TemplateFile::new`,
  `to_template`, the private `clean` (not used by this task). `template.rs`: `apply`, `has_content`. `lib.rs`: `impl
  From<EditError> for WriteError` and `edit_error_tests`. `tests/edits.rs` and
  `tests/template.rs`: their helpers (`write` sets tags with plain ExifTool, `tag` reads one).
- **What:**
  1. **Reading the values** (`read.rs`): `-IFD0:Make` and `-IFD0:Model` added to `IMAGE_TAGS`
     (read from the image file, for a manufacturer RAW the RAW itself, as DateTimeOriginal is).
     A `pub(crate) fn read_metadata_with_values(&mut self, image: &Path, fallback: Fallback) ->
     Result<(MetadataRead, VariableValues)>` does what `read_metadata` does today and also gives
     `VariableValues::of(image, <DateTimeOriginal as read>, <Make>, <Model>)`, the texts decoded
     as `exif_fields` decodes its fields (`exif_text` with `fallback`); `read_metadata` calls it
     and drops the values, so its signature and result stay as they are. `ExifFields` and
     `MetadataRead` get no new field (they are built in many tests in other crates).
  2. **The error:** `EditError::Variable(VariableError)`; its `Display` is the variable error's
     own text (as `Unsupported` gives its reason). In `lib.rs` it becomes `WriteError::Other`
     with that text, as `Template` does, and `edit_error_tests` gets the case.
  3. **The editor's edits** (`write_edits_safely`, which now reads with
     `read_metadata_with_values`): after the read, each edit is filled with the image's values
     before `apply_edits`, and the filled edits are used everywhere after it (`apply_edits`,
     `entries_left_alone`). `apply_edits` itself stays unfilled (the window also calls it,
     `pw-loupe`'s `editor.rs`):
     - `Set`: `fill_value` on its value. When the filled value equals the value given (no
       variable and no `{{`/`}}`), the edit is used exactly as before. Otherwise:
       - a `List`: blank entries and exact repeats dropped (the first kept);
       - a `Struct` or `StructList` of a whole property: **not** `template_file::clean` (it
         cuts Lang Alts to `x-default` and drops fields outside the structure's definition,
         which the window's structure lists keep on purpose, `editor.rs` `ListEntry::value`). A
         narrow pass in `edit.rs` instead: inside each structure, at any depth, a sub-field
         whose value **lost** its content by the filling (it had content before) is dropped,
         and so are the blank and repeated entries of a list sub-field; an entry left with no
         field is dropped. Languages, unknown fields and the order of the fields stay. A
         sub-field dropped this way removes that sub-field's old value from the entry (the
         window's `Set` replaces the whole list): an accepted limit, for `known-limits.md` at
         Done;
       - a `Text` or `LangAlt` (also a `field` of a single structure, Creator's Contact Info):
         not cleaned (a Lang Alt keeps its languages, `x-default` or not);
       - then, a value without content (`template::has_content`) gives **no change**: the edit
         is left out, so the property keeps its value (as a template's empty field, ADR 0007
         §10), never a `ValueError::Empty`. (A later `Entries` on the same property with an
         `Open` conflict is then left alone with its `ConflictKept` notice, as today.)
     - `Entries`: `fill` on each entry of `add` (an entry that comes out blank is ignored, as
       `apply_entries` already does); `remove` is not filled (it names the file's own entries);
     - `Clear`: as it is;
     - the first `VariableError` is `EditError::Variable`, returned before the plan is made, so
       nothing is written to the image. This is a change on purpose (U13-Q9): every edit whose
       text holds a lone `{` or `}`, also a text copied from a file, now fails until the user
       writes `{{` or `}}`; T289 shows it in the window for the fields the user changed.
  4. **Applying a template file:** `ExifTool::apply_template_file_safely(&mut self, image: &Path,
     template: &TemplateFile, options: &EditOptions, checks: &dyn ImageCheck, journal: &Journal)
     -> Result<EditReport, EditError>`, in `edit.rs` beside `apply_template_safely`:
     - a file kind that cannot hold metadata is `EditError::Unsupported` with the text
       `apply_template_safely` gives;
     - the image is read with `read_metadata_with_values`; each field of `template.fields` is
       filled with `fill_value` (the first error is `EditError::Variable`, nothing written); the
       filled fields and `template.add` go through `TemplateFile::new` again (a field empty after
       filling is dropped, a list's repeated entries are merged, U13a's chain review); then
       `to_template()`, `template::apply`, `restore_qualified`, the plan and the safe write, as
       `apply_template_safely` does;
     - the result is an `EditReport`: the plan's notices when the report is `Written`, and a
       `WriteNotice::QualifiersKept` for each list with qualifiers that was put back, also when
       nothing was written, each once (as `write_edits_safely` does; this closes the known limit
       "the import's template drops a change to such a list without a notice" once U13c uses this
       function);
     - `apply_template_safely` stays as it is (the import dialog's six fields use it until U13c
       replaces them; it fills no variables).
  5. **Checking a template:** `pub fn TemplateFile::check_variables(&self) -> Result<(),
     VariableError>` in `template_file.rs`: `variables::check_value` on each field's value, in
     order, the first error returned. U13c (the template window's Save, the import before
     anything is copied, ADR 0017 U13-Q6) and U13d (Apply Template before the question) call it
     so an unknown variable is found before anything is written (U13-Q9);
     `apply_template_file_safely` still finds it per image too.
  6. **Comments:** `edit.rs`'s module comment (the variables, the new function) and `EditReport`'s
     (now the result of both functions), `template.rs`'s
     ("variables … come later" now says where they are filled), `template_file.rs`'s (a template
     keeps its variables; they are filled by `apply_template_file_safely`).
- **Tests to add** (against a real ExifTool, Windows only and failing with an explanation when
  ExifTool is missing, as the files' other tests; each in its own temporary folder; the capture
  time, make and model set up with the files' `write` helper, for example
  `-ExifIFD:DateTimeOriginal=2026:11:22 14:30:05`, `-IFD0:Make=SONY`, `-IFD0:Model=ILCE-1`):
  - `tests/template.rs`, with a `TemplateFile` made by `TemplateFile::new`:
    - **every kind of field:** Headline `{date} {name}`, Description (Lang Alt) `Shot with
      {model}`, Keywords `["{year}", "{make}"]` with the + box checked, Location Created's City
      `{folder}`, and Creator's Contact Info e-mail `{name}@example.com`, applied to the small
      JPEG with keywords `["a"]`: the image gets Headline `2026-11-22 <its file stem>`,
      Description `Shot with ILCE-1`, keywords `["a", "2026", "SONY"]`, City the name of its
      folder and the e-mail filled; applied again, the report is `Unchanged`;
    - **no capture time:** the same template on a JPEG without DateTimeOriginal is
      `EditError::Variable(VariableError::NoCaptureTime(_))`, and the file is byte for byte as
      before;
    - **an unknown variable** (`{job}`) is `EditError::Variable(VariableError::Unknown(_))`,
      nothing written;
    - **empty after filling:** a JPEG without Make, Headline `Old`, and a template with Headline
      `{make}` and keywords `["{make}"]` with + checked: Headline stays `Old`, no keyword added,
      and the report is `Unchanged`;
    - **a manufacturer RAW** (the fake `TN0001.ARW`): Headline `{name}` gives `TN0001` in its
      sidecar, and `{date}` on it is `NoCaptureTime` with no sidecar made;
    - **braces:** Headline `{{x}}` gives `{x}`;
    - **notices:** a JPEG with IIM and a template with a literal City (no variable) longer than
      IIM's limit gives the `Truncated` notice (as `tests/edits.rs`'s
      `a_long_city_is_cut_in_iim_with_a_notice` sets it up).
    - `apply_template_file_safely` returns an `EditReport`, so the file gets a helper of its own
      for it beside `Scratch::apply` and `assert_report`; `VariableError` comes from
      `pw_metadata::variables`. The JPEG's name is the fixture's (`bild ä.jpg`), so the expected
      texts use its stem.
  - `tests/edits.rs`: a `Set` of Headline `{date}`, a `Set` of Description (Lang Alt) `{name}`, an
    `Entries` adding `{year}`, and a `Set` of the field `Iptc4xmpCore:CiEmailWork` of Creator's
    Contact Info `{name}@example.com` are written filled; a `Set` of Headline `{make}` on an
    image without Make is no change (alone: `WriteReport::Unchanged`); a `Set` of Keywords
    `["{make}", "x", "x"]` on an image without Make writes `["x"]`; `{job}` and a date
    variable without a capture time are `EditError::Variable` with the file unchanged; a `Set`
    without variables of a text holding no braces is written exactly as before (an existing
    test may show this); a `Set` of Location Shown with two entries, the first with a Lang Alt
    sub-field holding `x-default` and `sv` and City `{folder}`, the second unchanged, writes City
    filled and keeps `sv` and the second entry as they were; a City `{make}` on an image without
    Make drops only that sub-field from its entry.
  - `template_file.rs`'s unit tests: `check_variables` is `Ok` for a template with `{date}` and
    `{{x}}`, and gives `Unknown` for `{job}` in a Location Shown entry's City.
  - `lib.rs`'s `edit_error_tests`: `EditError::Variable` gives `WriteError::Other` with its text
    (a new case in the `cases` array of
    `unsupported_value_plan_and_template_are_other_with_the_error_text` is allowed).
- **Known limit (the reviewer puts it in `known-limits.md` at Done):** Make, Model and
  DateTimeOriginal are read as `IFD0:`/`ExifIFD:` tags; a RAW whose maker stores them in another
  group (possibly Panasonic RW2) gives an empty `{make}` and `{model}` and no capture time. The
  agent notes in the log what it can see with ExifTool; checking a real file is not required.
- **Not in this task:** the metadata window's check before Save (T289); the template window, the
  import's use of `apply_template_file_safely` and where its ExifTool gets `-config` (U13c); the
  right-click menu and its request to the metadata worker (U13d); the corrected capture time
  (ADR 0020 C2, which `{date}` then takes).
- **Rules:** `docs/tasks/README.md` rules 1–12; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`. A test that lists
  `IMAGE_TAGS` or the read's arguments may get the two new tags added; no other existing test is
  changed.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13b, second of the chain T287 → T288 → T289); waits for T287 |
| 2026-11-23 | Blocked | Cold read: `write_edits_safely` named as reading with `read_metadata_with_values`; the cleaning of a filled `Set` spelled out by kind (`clean` only after `check_kind`, Text and Lang Alt not cleaned, an unknown property or wrong kind kept for `apply_edit`'s error); the deliberate change for a lone brace stated; `EditReport`'s comment; the notices test with a literal City, a helper for `EditReport`, the fixture's file name; a test of a filled list cleaned; the `cases` array may grow; Make/Model of a RAW in another group as a known limit |
| 2026-11-23 | Blocked | Chain review (T287–T289): `template_file::clean` not used for the editor's edits (it would drop other languages and unshown fields of every structure entry); a narrow pass in `edit.rs` instead, with a test keeping `sv`; a structure sub-field emptied by filling dropped from its entry, an accepted limit; `TemplateFile::check_variables` (point 5) so U13c and U13d find an unknown variable before anything is written |
| 2026-11-23 | Open | T287 done (`variables.rs` as specified) |
| 2026-11-23 | Ready for review | Points 1–6 done. `read.rs`: `-IFD0:Make`, `-IFD0:Model` in `IMAGE_TAGS`; `read_metadata_with_values` (Make and Model through `tag` + `exif_text` with `fallback`, DateTimeOriginal as `exif_fields` read it); `read_metadata` calls it. `edit.rs`: `EditError::Variable`; `fill_edits` before `apply_edits` in `write_edits_safely` (filled edits also for `entries_left_alone`); `clean_filled`/`clean_filled_struct` as the narrow pass (a structure's field emptied by filling dropped, its lists cleaned, an entry left without fields dropped; languages, unknown fields and order kept); `apply_template_file_safely`. `template_file.rs`: `check_variables`. `lib.rs`: `Variable` → `WriteError::Other`. Comments as point 6. Tests: 7 in `tests/template.rs`, 5 in `tests/edits.rs`, 2 unit in `template_file.rs`, one case in `edit_error_tests`; a Set without variables written as before is shown by the existing `headline_and_city_are_written_and_the_same_again_is_unchanged`. Known limit (RW2): in ExifTool 13.64's tag list Panasonic's `PanasonicRaw::Main` has group 1 `IFD0`, so `-IFD0:Make`/`-IFD0:Model` should find an RW2's make and model; the table has no Exif pointer, so where an RW2's DateTimeOriginal is (probably the Exif of the embedded JPEG) was not seen; no real file checked. Accepted limit for `known-limits.md`: a structure sub-field emptied by filling removes that sub-field's old value from the entry |
| 2026-11-23 | Done | Reviewed: matches the task point by point; no existing test changed. check.ps1 run again by the reviewer, all OK (2447 passed, 78 ignored), no clippy warnings. Three known limits added to `known-limits.md`: the camera's tags in other groups (RW2 not checked), a structure's sub-field emptied by filling (accepted in the chain review), and, approved by the owner 2026-11-23, a Lang Alt inside a structure keeps an empty `x-default` beside another language. No user-visible change yet, no CHANGELOG line (at T289's Done); the owner's hand check of U13b follows T289. T289 set Open |
