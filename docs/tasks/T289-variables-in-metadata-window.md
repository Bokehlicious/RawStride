# T289. The metadata window checks the variables before Save — Open

- **Why:** ADR 0017 U13, part U13b (U13-Q9). After T288 the variables (`{date}`, `{name}` …) in
  the metadata window's text fields are filled with each image's own values when it is written,
  and an unknown variable fails the write of each image. "Apply to {n} Marked" would then fail on
  every marked image with the same reason. An unknown variable, or a brace that is not closed,
  does not depend on the image, so the window finds it **before anything is written**, as a field
  error beside the field, the way a wrong date is shown today (a field's error when Save, OK,
  Save and Next or Apply is refused; a structure list's sub-field error at once). A date variable on an image
  without a capture time can only be found when that image is written; it stays T288's failure
  of that image.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/editor.rs`, `docs/glossary.md` (only
  if a term of point 1 is missing; "Variables" and "Capture Time" are there, so probably none),
  `docs/test-inventory.md`.
- **Background (read before starting):** `pw_metadata::variables` (T287: `check`,
  `VariableError` and its four variants). `editor.rs`: `field_edit` (a changed field's `Edit`, or
  its error text), `date_error_text` (the text of a wrong date), `EditorForm::edits` and
  `batch_edits` (any field error gives `Err`, so neither Save nor "Apply to {n} Marked" writes
  anything), `ListEntry::error`, `has_error` and `EditorForm::list_error` (the error of a
  structure list's sub-field, so far only a date, T143), `SubFieldKind`, `FieldKind`.
- **What:**
  1. **A text function** in `editor.rs` (the window's text module, `docs/tasks/README.md` rule
     13): `fn variable_error_text(error: &VariableError) -> String`, one whole English sentence
     per variant, each from one template with the name filled in:
     - `Unknown(name)`: `{<name>} is not a variable. Variables: {date} {year} {month} {day} {time}
       {make} {model} {name} {folder}. Write {{ or }} for a brace.`
     - `Unclosed(_)`: `A { is not closed. Write {{ for a brace.`
     - `UnexpectedClose`: `A } is not opened. Write }} for a brace.`
     - `NoCaptureTime(name)`: `The image has no capture time for {<name>}.` (not given by
       `check`, but every variant has a text).
  2. **Fields:** in `field_edit`, a field of kind `Text`, `LangAlt` or `List` that is not a check
     box or a choice list (`field.choice_list()`) and whose text is not empty is checked with
     `variables::check`; an error is the field's error, with the text of point 1. A `Date` field
     is not checked (U13-Q9: not in dates; its own date check stays). A choice-list field arrives
     as `FieldKind::Text`, so the check tests `field.choice_list().is_none()` itself. The
     sub-fields of Creator's Contact Info (`FieldKind::Text`) are checked like any text field. In
     a list field only the entries the user typed are checked: those not among the entries of its
     start text (exact text). The `Edit` made carries the typed text with its variables, which
     T288 fills per image.
  3. **Structure lists:** `ListEntry::error` also gives the text of point 1 for a sub-field of
     kind `Text`, `List` or `LangAlt` whose text was changed (as a date sub-field is today: an
     unchanged text never has an error) and that `variables::check` refuses. A `Choice` sub-field
     is not checked. `has_error` and `lists_with_errors` then cover it as they do a date.
  4. **Texts the user did not type are escaped** (`variables::escape`, T287) in the edits the form
     makes, so T288's filling gives them back exactly and a brace another program wrote (or a
     `{x}` RawStride wrote from `{{x}}`) never fails a write or blocks a Save:
     - a list field's entries that are among its start text's entries, in the `Set` of the
       window's image (the entries the batch adds to other images are the typed ones, unchanged);
     - a chosen conflict value (`collect`'s `Set` of the chosen value);
     - in a structure list's `Set`: every text of an unchanged entry, and in a changed entry the
       texts of its base that the user did not change (unchanged sub-fields, the other languages
       of a Lang Alt sub-field, sub-fields the form does not show), at any depth; `Date` and
       `Number` values are not texts and stay as they are.

     A field or sub-field the user typed in is not escaped and is checked (points 2 and 3), so
     its variables are filled. An unchanged field gives no edit, as today.
  5. The comments that say a list's sub-field error is only a date (`ListEntry::error`,
     `EditorForm::list_error`, `EditorForm::edits`) are updated.
  6. **Recent values** (the ▼ list, `written_values`) keep the text as typed, `{date} {name}`,
     not a filled value, so a variable can be chosen again; unchanged, stated in its comment.
- **Tests to add** (unit tests in `editor.rs`, beside the existing form tests):
  - Headline changed to `{job}` makes `edits()` and `batch_edits()` give `Err` with Headline's
    error, whose text names `{job}` and the nine variables; Headline `{date} {name}` gives `Ok`
    with a `Set` of exactly that text; `{{x}}` gives `Ok`;
  - Headline `{date`, and `a}b`, give the error texts of `Unclosed` and `UnexpectedClose`;
  - Keywords (a list field) changed to `a, {job}` gives the error; Description (a Lang Alt) the
    same;
  - a Location Shown entry's City (the structure list `Iptc4xmpExt:LocationShown`, sub-field
    `Iptc4xmpExt:City`; the tests' `located` and `list_text_mut` helpers) changed to `{job}`
    gives `list_error` with the text and `edits()` `Err`; changed to `{folder}` gives no error;
    an added entry with City `{job}` gives the error too;
  - a form read with Headline `{x}` (another program's text) and only Title changed gives `Ok`
    with no edit of Headline;
  - Creator's Contact Info e-mail changed to `{job}` gives the error; a choice-list field is
    never given a variable error;
  - the Date Created field with `{date}` still gives the date error, not a variable error;
  - `variable_error_text` for each of the four variants (exact texts);
  - escaping: Keywords read as `["{draft}", "a"]` and changed to `{draft}, a, {year}` gives `Ok`
    with a `Set` of `["{{draft}}", "a", "{year}"]`; a chosen conflict value `{x}` gives a `Set`
    of `{{x}}`; a Location Shown list with an unchanged entry whose City is `{x}` and a second
    entry changed to City `{folder}` gives a `Set` with `{{x}}` in the first and `{folder}` in
    the second, and a Lang Alt sub-field's other language with a brace escaped too;
  - recent values: after a save of Headline `{date}`, the recent values hold `{date}`.
- **Not in this task:** filling the variables (T288); a list of the variables shown in the window
  (U13-Q9 asks for none; the error text of point 1 names them); the template window (U13c); the
  §11 warnings, which still look at the text with its variables (an IIM length warning counts
  `{date}`, not the date).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  The owner's hand check of U13b follows (the reviewer writes its points in the plan): Headline
  `{date} {name}` saved on one image and applied to several marked images gives each its own
  date and name; `{job}` is shown as an error and nothing is written; an image without a capture
  time fails with its reason while the others are written (a JPEG without DateTimeOriginal is
  needed, as the owner's RAW files all have one), and the form after the batch and the list of
  failures are looked at; a keyword with a brace written by another program is kept when another
  keyword is added.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13b, last of the chain T287 → T288 → T289); waits for T288 |
| 2026-11-23 | Blocked | Cold read: when the errors show (after Save is refused; a sub-field's at once); the choice-list test done in the check itself; Contact Info sub-fields checked; a list field's whole text checked; values written but not changed by the user (a chosen conflict value, unchanged list entries, unchanged parts of a structure list) named as an accepted limit for `known-limits.md` at Done; the "only a date" comments updated; the test's Caption replaced by Title, Location Shown's City spelled out, tests for an added entry, Contact Info and a choice list |
| 2026-11-23 | Blocked | Chain review (T287–T289): texts the user did not type are escaped with `variables::escape` (point 4: unchanged list entries, a chosen conflict value, the unchanged parts of a structure list), so a brace from another program never fails a write and the accepted limit of the cold read is gone; only typed list entries checked; recent values keep the typed text (point 6); `docs/glossary.md` listed in case; tests for escaping; the hand check looks at the form after a batch with a failure and needs a JPEG without DateTimeOriginal |
| 2026-11-23 | Open | T288 done (`apply_template_file_safely`, `EditError::Variable`, `TemplateFile::check_variables` as specified) |
