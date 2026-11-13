# T88. ADR 0007 I9b: the ingest template with the modes Keep, Replace, Append, Prefix and Clear — Done

- **Can start after T87 is accepted.** The owner approved D5 (the separator is one space) and D6
  (`xmpRights:Marked` added to the table) on 2026-11-12.
- **Crates:** `pw-metadata` only. Nothing in RawStride uses it yet (T91). The old `Iptc`,
  `Iptc::tags` and `write_iptc` stay unchanged (D7).
- **Files:** new `crates/pw-metadata/src/template.rs`; `crates/pw-metadata/src/edit.rs`;
  `crates/pw-metadata/src/lib.rs` (the `mod` line and `Iptc::to_template` only);
  `crates/pw-metadata/src/properties.rs` (D6: one hand-written entry, as `xmp:Label`); new
  `crates/pw-metadata/tests/template.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §10 (the four modes, Prefix as a form of Append; an empty field never clears;
  no Clear for copyright-management fields, §8), §15 (today's template is "Keep for empty fields,
  Replace for filled ones", which stays compatible), §9.5 (ingest writes with the safe procedure).
- **What:**
  1. **`template.rs`** (`pub mod template`):
     - `pub enum Mode { Keep, Replace, Append, Prefix, Clear }`;
     - `pub struct TemplateField { pub property: String, pub field: Option<String>, pub mode: Mode,
       pub value: Option<Value> }`: `property` is an XMP id of the table; `field` the XMP id of a
       sub-field when `property` is a **single** structure (`ValueKind::Struct`, e.g.
       `Iptc4xmpCore:CreatorContactInfo` / `Iptc4xmpCore:CiEmailWork`);
     - `pub struct Template { pub fields: Vec<TemplateField> }`;
     - `pub enum TemplateError { UnknownProperty(String), CannotClear(String), ModeNotAllowed {
       property: String, mode: Mode }, Value(ValueError) }` with `Display` in English and
       `std::error::Error`;
     - **`pub fn apply(template: &Template, metadata: &mut Metadata) -> Result<(),
       TemplateError>`**, field by field in order, by §10:
       - `Keep` → nothing. `Replace`, `Append` or `Prefix` with no value or a value without
         content (empty text or only spaces, an empty list) → nothing (§10: an empty field never
         clears);
       - `Replace`: the new value with `Metadata::set` (a Lang Alt sets `x-default` and keeps the
         other languages, as `set` already does; a list is replaced by the template's list);
       - `Append` / `Prefix` on `Text` and on a Lang Alt (its shown text, `LangAlt::display`): the
         old text, one space (D5) and the new text (`Prefix`: new, separator, old); without
         an old text only the new text. On a `List` (`Append` only): the template's entries
         that are missing are added at the end, in order, without duplicates (exact text);
         `Prefix` on a list is `ModeNotAllowed`. On a `Date`, a `Number` or a whole structure
         both are `ModeNotAllowed`;
       - a sub-field (`field` is `Some`): the structure's current value (or an empty one), the
         mode applied to that field as above, then `set` of the whole structure; a sub-field of
         a **list** of structures (Location Created, Image Creator, Copyright Owner, Licensor) is
         `ModeNotAllowed` for every mode but `Keep` (later, with the template editor);
       - `Clear`: `CannotClear` for the copyright-management properties of §8 (`dc:rights`,
         `dc:creator`, `photoshop:Credit`, `xmpRights:UsageTerms`, `xmpRights:WebStatement`,
         `plus:CopyrightOwner`, `plus:Licensor`, `plus:ImageCreator`) and their sub-fields;
         otherwise `Metadata::clear`, or for a sub-field the field removed (the property cleared
         when no field is left). Nothing to clear (no value) → nothing;
       - a result equal to the current value (`reconcile::same_value`) is **not** set, so a
         template that is already in the file gives an empty plan;
       - the cardinality limits come from `set` (`TooMany`, as `Value`).
     The confirmation before a Clear (§10, once per template) is the user interface's (I10);
     variables (`{date}` …) and template files in XMP (§10) come later.
  2. **`edit.rs`:** `EditError` gains `Template(TemplateError)`; **`ExifTool::apply_template_safely(
     &mut self, image: &Path, template: &Template, options: &EditOptions, checks: &dyn
     ImageCheck, journal: &Journal) -> Result<WriteReport, EditError>`**: `supports_rating(image)`
     false → `Unsupported` (same text as `write_iptc`); `read_metadata`, `template::apply`,
     `plan`, `write_safely`, as in T87.
  3. **`Iptc::to_template(&self) -> Template`** (`lib.rs`): each field that `Iptc::tags` writes
     today, with `Replace` and its value, the others `Keep`: `creator` → `dc:creator` as a list
     of one name; `title`, `description`, `copyright` → `dc:title`, `dc:description`, `dc:rights`
     (`x-default`); `credit` → `photoshop:Credit`; `email`, `phone`, `url` →
     `Iptc4xmpCore:CreatorContactInfo` with the fields `Iptc4xmpCore:CiEmailWork`,
     `Iptc4xmpCore:CiTelWork`, `Iptc4xmpCore:CiUrlWork`; `xmpRights:Marked` `True` with `Replace` when `copyright` is given (D6: add `xmpRights:Marked`
     to `properties.rs` as a non-IPTC entry like `xmp:Label`, kind Text, no IIM, no Exif). Values
     are trimmed; **line breaks are kept** (the plan writes them with `-ec`; `tags` turned them
     into spaces).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs` or
  `safe_write.rs`.
- **Tests to add:**
  - unit tests in `template.rs` on hand-built `Metadata`: every mode for a `Text`, a Lang Alt (an
    `sv` entry kept by `Replace` and `Append`; `Clear` removes every language), a list (`Append`
    keeps the order and adds no duplicates) and a sub-field of `CreatorContactInfo` (the other
    fields kept; the last field cleared clears the property); `Keep` and an empty value change
    nothing (`changes()` empty); `CannotClear` for each of the eight properties of §8;
    `ModeNotAllowed` for `Append` on a `Date` and a `Number`, `Prefix` on a list, and a sub-field
    of `plus:ImageCreator`; `TooMany` for four Image Creators; a template equal to the file gives
    no change;
  - unit tests of `Iptc::to_template` (`lib.rs`): a full template, an empty one (every field
    `Keep`), a description with a line break kept;
  - `tests/template.rs` (integration, **ExifTool required**; the JPEG fixture and the fake RAW of
    T82; `NoImageCheck`; a temporary journal): `Iptc::to_template` on a JPEG → the values read
    back with `read_metadata` (Creator, Copyright Notice, Credit, the three contact fields); the
    same again → `Unchanged`; a JPEG with `IFD0:Copyright` → Exif Copyright updated too (§6.3);
    `Append` of a keyword to a JPEG with two keywords → three, in order; `Clear` of
    `photoshop:City` on a JPEG with IIM → gone from XMP and IIM; the fake RAW → only the sidecar
    written, the RAW's bytes unchanged.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
