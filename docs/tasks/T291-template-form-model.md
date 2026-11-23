# T291. The template window's form in plain Rust — Open

- **Why:** ADR 0017 U13, part U13c (U13-Q3, U13-Q4, U13-Q12, U13-Q13), first of the chain
  T291 → T292 → T293 → T294 → T295 → T296. The template window (T293) is a form like the
  metadata window's, with a check box per field (only the checked fields are saved), a **+** box
  on the lists of plain text values, **Load…**, **Save…**, **Clear Form** and **Close**. Everything
  that can be worked out without a window is done here and tested: which rows the form has and in
  which order, what typing checks, what a template made from the form holds, what Load puts in
  the form, how the sections are split into two columns, and every text the window shows.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/template_form.rs` (new),
  `crates/pw-loupe/src/editor.rs`, `crates/pw-loupe/src/lib.rs`, `docs/glossary.md`,
  `docs/test-inventory.md`.
- **Background (read before starting):** `pw_metadata::template_file` (`TemplateFile`,
  `TemplateFile::new`, `in_template`, `can_add`, `TemplateFile::check_variables`,
  `TemplateFileError`, `TemplateRead`); `editor.rs`: `EditorForm` (`from_read`, `text`,
  `text_mut`, `set_checked`, `set_choice`, `use_value`, `list_count`, `list_text`,
  `list_text_mut`, `list_error`, `add_entry`, `collect`, `edits`), `field_edit`, `ListEntry`
  (`of`, `pasted`, `value`, `written`), `EditorField` (`ALL`, `property`, `struct_field`,
  `editable`, `check_box`, `choice_list`, `multiline`, `label`, `tooltip`, `text_of`),
  `variable_error_text`, `paste_shown`; `editor_layout.rs`: `EditorLayout` (`sections`, `items`),
  `EditorItem` (`fields`, `property`, `label`); `info_panel::property_label`. The test helper
  `read_of` in `editor.rs`'s tests shows how a `MetadataRead` is built.
- **What:**
  1. **The empty form** (`editor.rs`): a `pub(crate)` constructor of an `EditorForm` with no
     value, no conflict and every structure list empty, as `from_read` of a read with no metadata
     gives (for example `EditorForm::empty()`; it may build that read itself).
  2. **The template's values without escaping** (`editor.rs`): a `pub(crate)` method of
     `EditorForm` that gives, for the template form, each field's value and each structure list's
     entries **as typed**, with nothing escaped (`variables::escape` is for texts read from an
     image; a template has none):
     - for each editable field (`editable()`, not read-only): `field_edit` with an empty start
       text, so a `Set` gives the value, a `Clear` gives no value, and an `Err` the field's error
       text (a wrong date, a variable that cannot be read); a contact field's value is its
       sub-field's;
     - for each structure list: its entries as `ListEntry::value` gives them (the base the
       entry was pasted or loaded from, with its shown sub-fields set from their texts; empty
       entries left out), and the error of each sub-field (`ListEntry::error`), by entry and
       sub-field.

     Its exact shape is the agent's choice; `collect` and `edits` stay as they are.
  3. **Putting a template's values in the form as typed** (`editor.rs`): a `pub(crate)` method
     that takes `(xmp id, Value)` pairs, as `TemplateFile::fields` holds them, and puts each in the
     empty form so it counts as typed (the start texts stay empty, so nothing of it is escaped):
     - a field: its text from `text_of`; Copyrighted `True` or `False`; a choice list's value as
       `set_choice` stores it;
     - Creator's Contact Info (a `Value::Struct`): each of its sub-fields into its contact field;
     - a structure list (a `Value::StructList`): its entries replace the list's, each as
       `ListEntry::pasted` makes it (so its shown sub-fields are typed, U13-Q9 amended, and the
       sub-fields the form does not show are kept in the entry's base);
     - Location Created: its first entry as above.

     Copyrighted (`xmpRights:Marked`): `True` ticks its box and `False` leaves it unticked, and
     both count as put in the form; any other value is not put in the form and is returned as not
     loaded (below).

     It returns the XMP ids (`property` or `property/sub-field`) of what it could not put in the
     form: a pair whose property the form has no field or list for, and a sub-field of Location
     Created or Creator's Contact Info that the template form has no row for (Location Created's
     Location Name and GPS fields, for example; see point 5).
  4. **`template_form.rs`** (new module, exported from `lib.rs`): the template window's form,
     `TemplateForm`, holding the empty `EditorForm` of point 1, which T293 draws with the metadata
     window's field widgets, and the state of every check box and + box. Pure functions, no egui.
     `form(&self) -> &EditorForm` and `form_mut(&mut self) -> &mut EditorForm` give T293 the
     form to draw. It is also the template window's **text module** (`docs/tasks/README.md`
     rule 13, point 15). Since `main.rs` is a binary crate that uses `pw_loupe::…`, everything
     T293 uses is `pub` and re-exported from `lib.rs`: `TemplateForm` and its methods, the row
     and item types of point 5, `TemplateRefused`, the column split, `templates_folder`,
     `with_xmp_extension` and the texts of point 15. `editor.rs`'s `variable_error_text` becomes
     `pub(crate)` (points 11 and 15 use it; T295 too); `text_of`, `field_edit` and `ListEntry`
     stay as they are, since points 1–3 are in `editor.rs`.
  5. **The rows** (U13-Q3, U13-Q4, U13-Q13): a function giving, for an `EditorLayout`, the
     template form's sections and their items in the layout's order, **hidden sections and hidden
     items included** (`EditorLayout::sections` and `items`, not the `shown_` ones), without the
     Rating and Label section (U13-Q10) and without the fields that are not editable (Urgency,
     Category and Supplemental Categories, the deprecated fields). Before the run of contact
     fields and before the run of Location Created's sub-fields comes a row without a check box,
     `Heading(&'static str)` (the property, named by `property_label`); a section's row count
     (T293's first-frame height) counts it. Each item with a check box is one
     of (names are suggestions):
     - `Field(EditorField)`: a field that is not a contact field;
     - `ContactField(EditorField)`: one of the eight contact fields, drawn indented under the
       row "Creator's Contact Info" (no check box of its own on that heading row);
     - `LocationField(&'static str)`: one of Location Created's seven sub-fields
       (`EditorForm::LOCATION_FIELDS`), drawn indented under the row "Location Created" (no check
       box on that heading row); the template form's Location Created list always has exactly one
       entry (made with the form, kept by Clear Form and Load);
     - `List(&'static str)`: any other structure list (Location Shown, Image Creator, …), one
       check box for the whole list.

     Every property of these items is `in_template`, and the 50 properties of `in_template` are
     exactly those of the items (the contact fields as `Iptc4xmpCore:CreatorContactInfo`,
     Location Created's sub-fields as `Iptc4xmpExt:LocationCreated`); a test checks both ways
     (asked for by T284, see Tests).
  6. **Check boxes and + boxes:** `checked(item)` / `set_checked(item, bool)` for every item;
     `added(item)` / `set_added(item, bool)` only for a `Field` whose property is `can_add`
     (U13-Q12); `set_added` on another item does nothing. A + box keeps its state while its field
     is unchecked, but counts only on a checked field.
  7. **Typing checks the field** (U13-Q4): a method called after each frame (for example
     `notice_typing()`) compares each item's value with what it was at the previous call (or
     after `new`, Clear Form or Load), and checks every item whose value changed: a field's text
     (a check box's and a choice list's too), a contact field's or Location Created sub-field's
     text, a structure list's entries (the texts of its entries with the empty entries left out,
     so **Add** alone, an empty entry, checks nothing). It never checks
     or unchecks a + box, and never unchecks a field. The user's own click on a check box is not
     undone by it.
  8. **Changed since the last Load, Save or Clear** (for T293's question, U13-Q15,
     ADR 0017 §6.5): `changed()` is true when any value, check box or + box differs from a
     snapshot taken at `new`, Clear Form, Load and `mark_saved()` (a text typed back to what it
     was is not a change), and does not depend on `notice_typing()` having run.
  9. **Clear Form:** every value emptied, every check box and + box unchecked, Location Created
     back to one empty entry; `changed()` false.
  10. **Load** (`load(&TemplateFile) -> Vec<String>`): Clear Form, then point 3 with the
      template's fields; each item that got a value is checked (a contact field or Location
      Created sub-field by its own sub-field; a structure list as a whole); the + box of each id
      in `TemplateFile::add` is checked. Returns point 3's list. Afterwards `changed()` is false
      and `notice_typing()` does not count the load as typing.
  11. **The template the form holds** (`template() -> Result<TemplateFile, TemplateRefused>`,
      names are suggestions), from point 2:
      - each checked `Field` with a value gives `(property, value)`; a checked `Field` without a
        value gives nothing (U13-Q4: a checked empty field is not saved);
      - the checked contact fields with a value together give one
        `(Iptc4xmpCore:CreatorContactInfo, Value::Struct)` of only those sub-fields;
      - the checked Location Created sub-fields with a value together give one
        `(Iptc4xmpExt:LocationCreated, Value::StructList)` of one entry of only those
        sub-fields;
      - a checked Copyrighted gives `True` when its box is ticked and `False` when not (its
        check box always has a value);
      - a checked `List` with an entry that writes something (`ListEntry::value` not `None`)
        gives `(property, Value::StructList)` of those entries;
      - `add`: the checked `Field`s whose + box is checked;
      - the result is `TemplateFile::new(fields, add)`.

      It is refused (`Err`) when: a checked item has an error (a field error, or a sub-field
      error of a checked list or of a checked Location Created sub-field), each named by item
      (and, for a list, by entry and sub-field) with its text so T293 shows it under the item;
      `TemplateFile::check_variables` of the result
      fails (a variable in a part the form does not show, kept from a loaded file), with
      `variable_error_text`; or the result has no field (nothing checked has a value). An
      unchecked item's error never refuses the template, and its text is not in it. A shape
      such as `TemplateRefused::{FieldErrors(Vec<…>), Variables(String), Empty}`, with a function
      giving the status line's text of each (point 15).
  12. `mark_saved()`: `changed()` false from now on, until the next change.
  13. **Two columns** (U13-Q13): a pure function that, given each section's height in order
      (any unit; T293 passes the heights it measured), gives how many sections go in the left
      column: the sections fill the left column first, then the right, at the split where the
      taller column is lowest; only the splits 1..=n are considered (the left column is never
      empty while there is a section), the first such split on a tie; none gives 0.
  14. **Paths:** `templates_folder(local_app_data: &Path) -> PathBuf` gives
      `<local_app_data>\RawStride\templates` (U13-Q2); `with_xmp_extension(path) -> PathBuf`
      gives the path unchanged when its name ends in `.xmp` (any case), otherwise with `.xmp`
      added to the whole name (`a.txt` → `a.txt.xmp`), so the file always ends in `.xmp`.
  15. **Texts** (rule 13: English, glossary terms, one function per text or one function with a
      text enum as `editor_text` has, whole sentences from one template; field names only from
      `property_label` or `EditorField::label`):
      - the window's title: `Metadata Template`, and `Metadata Template – {file name}` once a
        file is loaded or saved;
      - the buttons `Clear Form`, `Load…`, `Save…`, `Close`;
      - the check box's hover text `Use this field in the template.`; the + box's label `+` and
        hover text `Add the template's entries after the image's own instead of replacing the
        list.`;
      - the name of the Save and Load dialogs' file type: `Metadata Template`;
      - after a load: `Loaded {file name}.`; what was not loaded, from `TemplateRead::skipped`,
        `TemplateRead::dropped` and point 10's list together, by name (`property_label`; a
        sub-field as `{property} – {sub-field}`): `1 field of the file was not loaded: {names}.`
        / `{n} fields of the file were not loaded: {names}.`; each of `TemplateRead::unreadable`
        (a property path and the text read): `{name} could not be read ({text}).`, the path named
        as the not-loaded list names a sub-field (each part by `property_label`, joined by ` – `,
        deeper parts the same way);
      - a load that failed: `{file name} was not loaded: {reason}` (the reason the
        `TemplateFileError`'s text, or `the template reader has stopped` when T292's
        `TemplateIoStopped` is the answer; a save then gives `Not saved: the template reader has
        stopped`);
      - after a save: `Saved {file name}.`;
      - a save refused or failed: `Not saved: a checked field has an error.`, `Not saved:
        {variable error text}`, `Not saved: no checked field has a value.`, `Not saved: a
        template cannot be saved in a folder whose path holds a % sign ({folder}).`
        (`TemplateFileError::PercentInFolder`, U13-Q1 amended), `Not saved: {reason}` (any other
        `TemplateFileError`), and `Not saved: ExifTool cannot use its settings file for
        templates ({reason}).` (T292's save without the config);
      - the question before a Load or Close loses changes (U13-Q15): `Discard the
        changes to the template?` with the buttons `Discard` and `Cancel`;
      - the question before a save over a file whose name got `.xmp` added (point 14; the Windows
        dialog asks only for the name as typed): `{file name} already exists. Replace it?` with
        `Replace` and `Cancel`.
  16. `docs/glossary.md`: the terms of point 15 that it lacks (at least "Metadata Template",
      "Clear Form" and the + box), named in the report. "Metadata Template" is a row of its own
      beside the existing "IPTC-mall → IPTC Template" row, which stays (the import's texts are
      T294's and T295's).
- **Tests to add** (unit tests in `template_form.rs`, and in `editor.rs` for points 1–3):
  - the rows of the default layout: the sections in order without Rating and Label; Headline,
    Keywords, the contact fields as `ContactField`, Location Created's seven sub-fields as
    `LocationField`, Location Shown as `List`; no Urgency; a layout with a hidden section and a
    hidden item still gives both, in the layout's order (a moved section too);
  - **the template's fields equal the window's** (T284): the properties of the default layout's
    rows (point 5's mapping) and the properties of the table for which `in_template` is true are
    the same set of 50;
  - typing: Headline's text changed, then `notice_typing()`, checks Headline and nothing else;
    an added Location Shown entry with a City checks Location Shown; a choice list set and
    Copyrighted ticked check their fields; a field the user unchecked after typing stays
    unchecked at the next call while its text does not change; the + box of Keywords stays
    unchecked after typing in Keywords; an empty Location Shown entry added checks nothing;
  - `template()`: Headline `{date} {name}` checked gives that text unescaped; an unchecked
    Title with text is not in it; a checked empty field is not in it; Keywords `a, b` checked
    with + gives `dc:subject` = `["a", "b"]` in `fields` and `dc:subject` in `add`; the + box of
    an unchecked Keywords is not in `add`; Copyrighted checked with its box unticked gives
    `xmpRights:Marked` `False`, ticked `True`; Email and City of the contact fields checked (Phone typed, then unchecked) give one
    Contact Info structure of Email and City; Location Created City and Country checked give one
    entry of those two; Location Shown with two entries checked gives both; a checked Headline
    `{job}` is refused naming Headline with the `Unknown` text, while an unchecked Headline
    `{job}` with a checked Title is not; a checked Date Created `abc` is refused with the date
    error; nothing checked is refused with "no checked field has a value";
  - Load: a `TemplateFile` of Headline `{date}`, Keywords with +, Contact Info Email, Location
    Created City and Location Shown with one entry gives those texts and entries, each item
    checked, the + of Keywords checked, `changed()` false, and `template()` gives an equal
    `TemplateFile`; a Location Created entry with Location Name returns
    `Iptc4xmpExt:LocationCreated/Iptc4xmpExt:LocationName` as not loaded; a Location Shown
    entry with a sub-field the form does not show and a variable in it is kept by `template()`,
    and `{job}` there refuses the template with the variable error; Copyrighted `False` loads
    checked and unticked, and a value that is neither `True` nor `False` is returned as not
    loaded; `changed()` false after a text typed and typed back;
  - Clear Form empties and unchecks everything and keeps one Location Created entry;
    `changed()` after typing, after Clear Form, after `mark_saved()`;
  - the column split: `[1]` → 1; `[3, 1, 1, 1]` → 1; `[1, 1, 1, 1]` → 2; `[5, 1]` → 1;
    `[2, 0]` → 1; `[]` → 0;
  - `templates_folder` and `with_xmp_extension` (`a.xmp`, `a.XMP`, `a`, `a.txt`);
  - every text of point 15 (exact texts), the not-loaded text with one and with two names.
- **Not in this task:** the window and its drawing, the menu item, the dialogs and the files
  (T293); reading and saving files and the ExifTool process (T292); the import (T294, T295);
  Load Template… and Save as Template… in the metadata window (U13d).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency; no `unwrap`/`expect` or
  unchecked indexing in non-test code; build only in the project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No CHANGELOG line (nothing the user sees until T293).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, first of the chain T291 → T292 → T293 → T294 → T295 → T296). Waits for the owner's answers to the reviewer's proposals U13-Q14–U13-Q19 (listed in `plans/adr-0018-0025-steps.md`, U13's notes), its cold read and the chain review (U13-Q11) |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: Copyrighted decided (reviewer detail, not asked: a checked Copyrighted gives `True` or `False` by its box; Load of `True`/`False` checks it; another value is not loaded); `form`/`form_mut` and the `pub` re-exports T293 needs (main.rs is a binary crate), `TemplateRefused`'s shape; `Heading` rows before the contact fields and Location Created's sub-fields; `variable_error_text` made `pub(crate)`; the column split never leaves the left column empty (`[2, 0]` test); `changed()` is a snapshot comparison; a list counts as typed only by its non-empty entries; unreadable values named by path, `{name} could not be read ({text}).`; `TemplateIoStopped`'s reason text (T292, T293, T295); glossary: "Metadata Template" beside "IPTC Template"; wording of point 5, point 11 and the Keywords test |
| 2026-11-23 | Open | Chain review: T293 gets `lib.rs` (its `pub use shell::{…}` line, the size function); `TemplateWindow::answer` gives `Answered::Saved(path)` and `open_template` the path to load (T295 uses both); the template window's backdrop in `Order::Foreground`, `open_window_shown()` checks it, the import dialog greys its content while it is open (T295); the whole form greyed while a load or save waits (typing then would be lost or taken as saved); T295's flag rules that cannot happen dropped, `confirming` no longer named, the `cannot be used` message made in `import_text.rs` (`variable_error_text` is `pub(crate)`); T291 and T292 side by side share `lib.rs` and the inventory row (reviewer recounts), T294 runs after T293, T296 may run beside T295; CHANGELOG lines at T293's and T295's Done, none for T291, T292, T294, T296; ADR 0007's D13/D14 lines at T294's Done, README's "What it does" at U13c's hand check; T295's hand check gains the `%` folder (U13a) and a notice line (U13-Q19). Set Open |
