# T104. ADR 0007 I10k: the editor's form with its fields, conflict choices, edits and warnings — Done

- **Can start after T96, T97 and T102 are accepted** (D12 approved on 2026-11-12). It changes no file of
  T98, T101 or T103, so it can run at the same time as them (only `lib.rs` lines shared, see T99).
- **Crates:** `pw-loupe` only. The window is T105.
- **Files:** new `crates/pw-loupe/src/editor.rs`; `crates/pw-loupe/src/lib.rs` (one `mod` line, one
  `pub use` line); `docs/test-inventory.md`.
- **Why:** ADR 0007 §8 (Lang Alt: `x-default` only; clearing removes every representation),
  §5.1/Q1 (the user decides an `Open` conflict), §7 (truncated dates, no zone invented), §11
  (warnings).
- **What** (`editor.rs`, pure, no egui), with the fields of D12:
  1. `pub enum EditorField` with the D12 fields in the D12 order; `label()` from T102's
     `property_label(…, Language::Sv)` (the contact fields "E-post", "Telefon", "Webb"; no names
     of its own in `editor.rs`); `multiline()` true for
     Beskrivning, Alternativtext, Instruktioner, Användarvillkor, Nyckelord and Fotograf.
  2. **`EditorForm::from_read(read: &MetadataRead) -> EditorForm`:** one text per field: a text as
     it is; a Lang Alt its shown text (and a note "(<språk>)" when it has no `x-default`, so saving
     adds `x-default`, Q7); a list one entry per line (Nyckelord, Fotograf); a date `to_xmp()`;
     the contact fields from `CreatorContactInfo`.
  3. **Conflicts:** `EditorForm::open_conflicts()` lists every `Open` conflict of the file (also on
     properties that are not editor fields): label and each value with its origin.
     `EditorForm::choose(conflict_index, value_index)` records the choice; for an editor field it
     also puts the value's text in the field.
  4. **`EditorForm::edits(&self) -> Result<Vec<Edit>, Vec<(EditorField, String)>>`:** for each
     changed field (the text differs from the one `from_read` gave): an empty text (after trim) →
     `Edit::Clear`; otherwise `Edit::Set` with the value of the field's kind: a text; a Lang Alt
     `x-default`; a list from the non-empty trimmed lines; a date by `IptcDate::parse_xmp`, an error
     "Skriv datumet som ÅÅÅÅ, ÅÅÅÅ-MM, ÅÅÅÅ-MM-DD eller ÅÅÅÅ-MM-DDTtt:mm[:ss], med tidszon bara om
     den är känd"; the contact fields as `field` edits of `Iptc4xmpCore:CreatorContactInfo`. A
     chosen conflict value is an `Edit::Set` of that value even when the text did not change (T97
     then writes both forms). All field errors are returned together.
  5. **Warnings:** `EditorForm::warnings(&self, read: &MetadataRead, create_iim: bool) ->
     Vec<String>`: the edits applied to a copy of the read metadata with T97's `apply_edits`, then
     T96's `validate` with `iim_written` = the file has IIM, or `create_iim` and the file is not a
     manufacturer RAW; Swedish texts: "<fält> blir avkortat i IIM (högst <n> byte); XMP får hela
     texten", "Alternativtexten är längre än 250 tecken (<n>)", "Landskoden <kod> ska vara två
     eller tre bokstäver (ISO 3166-1)", "Fotografens titel utan Fotograf", "Digital källtyp <uri>
     finns inte i IPTC:s lista", "Registerpost <n> saknar post-id eller organisation", "Betyget
     <v> ligger utanför −1 till 5".
  6. `pub fn notice_text(notice: &WriteNotice) -> String` for the editor's result (T105):
     `Truncated` "<fält> kortades i IIM; XMP har hela texten"; `IimConvertedToUtf8` "IIM-texterna
     skrevs om som UTF-8"; `ConflictKept` "<fält>: konflikten är olöst och skrevs inte";
     `DigestNotWritten` "Ingen ny IIM-kontrollsumma skrevs, eftersom en konflikt är olöst".
- **Text rule** (as T102; owner, 2026-11-12: so English can be added later in one module): every
  text the user sees from the editor (labels, the Lang Alt note, field errors, warnings, notices)
  is made by a function in `editor.rs` or `info_panel.rs`; property names only through
  `property_label`. Each text is a whole sentence built from one template with placeholders
  (`format!` with the values filled in), never joined from separately translated pieces, so
  another language can change the word order; numbers that need singular/plural are formatted
  inside the same function. Nothing returned by `editor.rs` is meant to be completed by string
  joins in the window's code (T105).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Nothing in `pw-metadata` changes.
- **Tests to add** (unit tests on hand-built `MetadataRead`): `from_read` of every field kind and of
  a Lang Alt without `x-default`; no edits for an untouched form; a changed text, an emptied field
  (`Clear`), two keywords on three lines with one blank, a date `2026-11` and a bad date (error
  text), several errors at once, an e-mail as a `field` edit; a chosen `Open` conflict value with
  an unchanged text gives a `Set`; a conflict on a non-editor property is listed and chosen;
  each warning text once; each notice text.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
