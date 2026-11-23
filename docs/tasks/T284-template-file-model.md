# T284. The template file's model: checked fields, the + box, and Location Created's sub-fields — Open

- **Why:** ADR 0017 U13, part U13a (U13-Q1, Q4, Q5, Q12; ADR 0007 §10, revision 33 and 34). A
  metadata template is a set of **checked fields**, each with its value; a list of plain text
  values can have its **+ box** checked, and then adds its entries instead of replacing the list.
  This task makes that model in plain Rust and turns it into the existing
  `pw_metadata::template::Template`, which `ExifTool::apply_template_safely` already applies
  through the write plan and the safe write. Reading and writing the template file with ExifTool
  is the next task (T285); the variables (U13b), the template window and the import (U13c) come
  later.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/template_file.rs` (new),
  `crates/pw-metadata/src/template.rs`, `crates/pw-metadata/src/value.rs` (only `check_kind`
  made `pub(crate)`), `crates/pw-metadata/src/lib.rs` (only `pub mod template_file;`),
  `docs/test-inventory.md`. Read only, not changed: `crates/pw-loupe/src/editor.rs` and
  `crates/pw-loupe/src/main.rs` (for the list of point 2).
- **Background (read before starting):** `template.rs` (the whole module comment; `Mode`,
  `TemplateField`, `Template`, `TemplateError`, `apply`, `apply_field`, `apply_sub_field`,
  `combine`, `has_content`, `x_default`): Replace sets a value (a Lang Alt only its `x-default`,
  other languages kept), Append on a list adds the missing entries at the end without duplicates,
  an empty value never changes anything, a value equal to the current one is not set. Today a
  field of a **list** of structures allows only Keep (`apply_sub_field` returns
  `ModeNotAllowed` for `Occurrence::Multi`). `properties.rs`: `property`, `properties`,
  `struct_def`, `Occurrence`, `ValueKind`, `is_deprecated_iim`. `value.rs`: `Value` (`Text`,
  `LangAlt`, `List`, `Date`, `Number`, `Struct`, `StructList`), `StructValue`, `LangAlt::display`,
  `check_kind` (what `Metadata::set` accepts for a property, structures at any depth).
  In the property table `Iptc4xmpExt:LocationCreated` is a `Multi` structure (a list) and
  `Iptc4xmpCore:CreatorContactInfo` a `Single` structure.
- **What:**
  1. **A sub-field of Location Created** (`template.rs`, `apply_sub_field`): with **Replace**, a
     field of `Iptc4xmpExt:LocationCreated` (for example `Iptc4xmpExt:City`) is set in the
     **first entry** of the image's list, and that entry's other fields and every later entry are
     kept. When the image has no Location Created, the result is a list of one entry holding only
     that field. A value without content changes nothing; a result equal to the current list is
     not set (compared with `==` on the whole `Value`, as for a single structure, not with
     `same_value`); the entry's fields are put in the structure's order, as `apply_sub_field`
     already does for a single structure. Only Location Created gets this: a field of any other
     list of structures, and Append, Prefix or Clear on a field of Location Created, stay
     `ModeNotAllowed`. Update the comments that no longer hold: the module comment's first line
     and "A field of a **list** of structures … allows only Keep for now", and the doc of
     `TemplateField::field` ("when `property` is a single structure": also a field of Location
     Created). The existing test `a_field_of_a_list_of_structures_allows_only_keep` still passes;
     leave its body as it is, and only correct its comment "The same for Location Created" (it
     tries only Clear there).
  2. **The model** (`template_file.rs`, new module with a module comment that names ADR 0017
     U13-Q1, Q4, Q5, Q12 and ADR 0007 §10):
     - `pub struct TemplateFile { pub fields: Vec<(String, Value)>, pub add: Vec<String> }` with
       `Debug, Clone, Default, PartialEq, Eq`: `fields` are the **checked fields** with their
       values, each keyed by its property's XMP id; `add` the XMP ids of the fields whose
       **+ box** is checked. Document the shape of each kind: a simple field its value; a Lang Alt
       a `Value::LangAlt` with only `x-default`; Location Created a `Value::StructList` of **one**
       entry holding only the checked sub-fields (U13-Q4); Creator's Contact Info a
       `Value::Struct` holding only the checked sub-fields; any other list of structures
       (Location Shown, Image Creator, Copyright Owner, Licensor, …) a `Value::StructList`, whole
       (U13-Q4: one check box for the list).
     - `pub fn can_add(xmp_id: &str) -> bool`: true only for a property that is `in_template`
       and a **list of plain text values** (`Occurrence::Multi` with `ValueKind::Text` or
       `ValueKind::Uri`, whose value is a `Value::List`; U13-Q12): `dc:subject`, `dc:creator`,
       `Iptc4xmpExt:PersonInImage`, `Iptc4xmpCore:SubjectCode`, `Iptc4xmpCore:Scene`,
       `Iptc4xmpExt:OrganisationInImageName`, `Iptc4xmpExt:OrganisationInImageCode`, and also
       `plus:ModelReleaseID` and `plus:PropertyReleaseID` (nine). If the rule gives another
       property of the list of `in_template`, stop and ask. False for text, Lang Alt, dates,
       numbers, structures and lists of structures, for a list the metadata window does not edit
       (`Iptc4xmpExt:EventId`, `Iptc4xmpExt:ModelAge`: not `in_template`), the deprecated
       `photoshop:SupplementalCategories`, and an unknown id. (The reviewer checked the kinds in
       the table on 2026-11-23.)
     - `pub fn in_template(xmp_id: &str) -> bool`: true only for a property **the metadata
       window edits** (U13-Q3: a template never holds something the template window cannot
       show), written out as an explicit list (a `const` of XMP ids in `template_file.rs`): the
       properties of `EditorField::property` and `EditorForm::ALL_LISTS` in
       `pw-loupe/src/editor.rs`, with the eight contact fields as the one property
       `Iptc4xmpCore:CreatorContactInfo`; the same list `window_editor_properties()` in
       `pw-loupe/src/main.rs` builds (its test names 55 properties), **less** `xmp:Rating` and
       `xmp:Label` (U13-Q10) and the three deprecated IIM properties
       (`properties::is_deprecated_iim`, never edited, ADR 0007 §5.5). Everything else is false,
       for example `Iptc4xmpCore:IntellectualGenre`, `Iptc4xmpExt:ImageRegion`, the rights
       expressions, `Iptc4xmpExt:DigImageGUID`, `Iptc4xmpExt:EventId` and the AI fields (shown
       only under Other Metadata, or per-image identifiers). U13c adds a test in `pw-loupe` that
       this list and the window's agree; this task names its count in a test.
     - `pub fn new(fields: Vec<(String, Value)>, add: Vec<String>) -> TemplateFile`, the one way
       the reader (T285) and the window (U13c) make a template, so both get the same rules:
       a field that is not `in_template` is dropped; a value `value::check_kind` refuses for
       its property (for example a `Value::Text` for `dc:subject`, a `Value::Struct` for
       Location Created, a number that is not one, a structure field that is not a field of that
       structure) is dropped, so `Metadata::set` never refuses a kind after `new`; **first**
       every Lang Alt, at the top and inside a structure, is cut to its `x-default` (matched
       without regard to case, as `LangAlt::display` does; one without it is dropped); **then**
       every list, at the top and inside a structure, loses its blank entries (only white space)
       and its exact duplicates (the first kept), so a list never writes an empty entry; **then**
       content is checked: a value without content (`template::has_content`, made `pub(crate)`;
       not `value.rs`'s own check, which counts only spaces) is dropped;
       inside **every** structure (Location Created, Creator's Contact Info and each entry of a
       list of structures such as Location Shown, at any depth) a field without content is
       dropped and the fields are put in the structure's order (`struct_def`, as the reader
       gives them), and an entry or structure left with no field is dropped; a list inside a
       structure (for example Location's `LocationId`) is otherwise kept as it is; so a template read back
       from a file (T285) compares equal to the one written; Location Created with more than one
       entry keeps only the first; a field named twice keeps the first; `fields` are put in the order of the property table
       (`properties::properties()`); `add` keeps only ids that are `can_add` **and** among the
       kept fields (the + box counts only on a checked field, U13-Q12), each once, in the same
       table order. `new` can be run again on its own result with no change (U13b runs it again
       after the variables are filled in, as blank entries can appear then).
     - `pub fn to_template(&self) -> Template`: one `TemplateField` per kept field, in order,
       except Location Created and Creator's Contact Info, which give one per sub-field:
       a field in `add` gets `Mode::Append` with its list; Location Created gives one
       `Mode::Replace` field per sub-field of its entry, with `field: Some(sub)` (point 1);
       Creator's Contact Info likewise, one per sub-field (the single-structure case
       `apply_sub_field` already has); every other field `Mode::Replace` with its whole value.
       `to_template` is used on a value made by `new`; it does not check again.
  3. Keep the old model as it is: `Mode::Keep`, `Prefix` and `Clear`, `Iptc::to_template` and the
     import's use of them stay until U13c replaces the import's six fields; nothing that uses them
     changes in this task.
- **Tests to add** (unit tests in `template_file.rs` and `template.rs`, no ExifTool):
  - Location Created (point 1): City replaced in the first entry, its other fields and a second
    entry kept; no Location Created → one entry with only that field; an equal value → no change
    (`metadata.changes()` empty; build the image's entry with its fields in the structure's
    order, Sublocation before City, or the sorted result differs); a value without content → no change; Append and Clear on a field
    of Location Created, and Replace on a field of Location Shown, → `ModeNotAllowed`.
  - `can_add`: true for the nine ids named in point 2; false for `dc:title` (Lang Alt),
    `photoshop:City` (text), `photoshop:DateCreated`, `Iptc4xmpExt:ModelAge`,
    `Iptc4xmpExt:EventId`,
    `Iptc4xmpExt:LocationShown`, `plus:Licensor`, `Iptc4xmpCore:CreatorContactInfo`,
    `photoshop:SupplementalCategories` and an unknown id.
  - `in_template`: the list holds 50 properties (55 of the window less five; if the count
    differs, stop and ask); false for `xmp:Rating`, `xmp:Label`, the three deprecated IIM
    properties, `Iptc4xmpCore:IntellectualGenre`, `Iptc4xmpExt:ImageRegion`,
    `Iptc4xmpExt:DigImageGUID` and an unknown id; true for `dc:subject`,
    `Iptc4xmpCore:CreatorContactInfo` and `Iptc4xmpExt:LocationCreated`.
  - `new`: each rule of point 2 on its own (not in template, a value of the wrong kind, an
    unknown structure field, without content, empty sub-field, Lang Alt cut to `x-default`, a
    Lang Alt `[("x-default", " "), ("sv", "Hamn")]` dropped (cut first, then checked), a Lang Alt
    without `x-default`, a Lang Alt sub-field of Location Created cut, sub-fields given out of
    order sorted and an empty one dropped inside a Location Shown entry, a list `["a", " ", "a",
    "b"]` kept as `["a", "b"]`, a list of only blanks dropped, `X-Default` matched as
    `x-default`, a Location Created entry whose City is a number `Value::Text("12")` kept and
    a `Value::List` for City dropped, two Location Created entries,
    a field named twice, table order); `add` with an id that is not checked, one that is not
    `can_add` (`dc:title`), and one named twice; `new` run on its own result gives the same.
  - `to_template` applied with `template::apply` to a `Metadata` holding keywords `["a", "b"]`:
    with `dc:subject` in `add` and the template's list `["b", "c"]` the result is
    `["a", "b", "c"]`; without it, `["b", "c"]`. A Location Created with City only replaces the
    city and keeps the image's Sublocation. A Creator's Contact Info with only an e-mail keeps
    the image's phone. A Location Shown list of two entries replaces the image's list of three.
- **Not in this task:** reading or writing a file (T285), the namespace of the + list (T285),
  variables (U13b), the template window, the import and the right-click menu (U13c, U13d),
  removing the old modes.
- **Rules:** `docs/tasks/README.md` rules 1–12; no new dependency; no `unwrap`/`expect` in
  non-test code.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13a, first of the chain T284 → T285); waits for the chain review (U13-Q11) |
| 2026-11-23 | Blocked | Cold read: `can_add` widened to every list of text or URIs in the template (also `plus:ModelReleaseID`, `plus:PropertyReleaseID`, `Iptc4xmpExt:EventId`; not `ModelAge`, not the deprecated list); `new` drops a value of the wrong kind and an unknown structure field, cuts Lang Alt before the content check (also inside structures), drops empty fields and sorts fields in every structure; comments of `template.rs` to correct named; the existing list-of-structures test kept, only its comment corrected; equality named for Location Created |
| 2026-11-23 | Open | Chain review (T284 → T285, one fresh agent): `in_template` is now the metadata window's editable properties as an explicit list (50), not every table property, so another program's XMP cannot bring in hidden fields (U13-Q3); `can_add` nine lists (EventId out); `new` uses `value::check_kind` (`value.rs` added to the files), matches `x-default` without regard to case, drops blank entries and duplicates in lists, and gives the same when run again (for U13b); T285 got a unique temp file in a `.saving` subfolder, cleanup on every failure, `TemplateRead::dropped`, more round-trip cases. Left for later parts (in the plan's U13 notes): U13b needs a step between read and apply in `apply_template_safely`; U13d needs its notices and a TemplateFile → form mapping; U13c decides the old six import fields in `settings.txt` |
