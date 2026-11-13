# T100. ADR 0007 I10g: the import dialog chooses the mode of each template field, and asks before a Clear — Done

- **Can start now.** The owner approved D13 and D14 on 2026-11-12. It needs nothing new in `pw-metadata` (T88's `Template`
  exists). It can run at the same time as T94–T99 and T102 (only `lib.rs` lines shared, see T99);
  not at the same time as T101, T103, T105 or T106 (all change `main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/import.rs`; `crates/pw-loupe/src/import_worker.rs`;
  `crates/pw-loupe/src/main.rs` (the IPTC part of the import dialog and the start of the import
  only); `crates/pw-loupe/src/lib.rs` (the `pub use import::…` line only);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §10 (each template field has a mode; an empty field never clears; no Clear for
  copyright-management fields; "Clearing any other field asks for confirmation once per
  template"). Left over from T88 and T91: today the import dialog's six fields always use
  `Iptc::to_template` (Replace for a filled field, Keep for an empty one).
- **What** (as D13 and D14):
  1. **`import.rs`:**
     - `pub enum IptcField { Creator, Copyright, Credit, Email, Phone, Url }` with
       `IptcField::ALL` in this order and `label()` giving the dialog's texts ("Fotograf",
       "Copyright", "Credit", "E-post", "Telefon", "Webb");
     - `IptcForm` gains `pub modes: IptcModes`, one `pw_metadata::template::Mode` per field,
       default `Replace` for all six (D13; with an empty text that does nothing, so a form nobody
       touches behaves exactly as today);
     - `pub fn allowed_modes(field: IptcField) -> &'static [Mode]`: Creator `Keep, Replace,
       Append`; Copyright and Credit `Keep, Replace, Append, Prefix`; Email, Phone and Url all
       five (§8/§10: no Clear of Creator, Copyright Notice or Credit Line; no Prefix on a list);
     - `pub fn mode_text(mode: Mode) -> &'static str`: the Swedish names of D13 ("Behåll",
       "Ersätt", "Lägg till efter", "Lägg till före", "Rensa");
     - **`IptcForm::to_template(&self) -> Template`**: per field its mode and its trimmed text
       (`None` when empty) on the same properties as `Iptc::to_template` (Creator a list of one
       name on `dc:creator`; Copyright `dc:rights` `x-default`; Credit `photoshop:Credit`; the
       three contact fields as fields of `Iptc4xmpCore:CreatorContactInfo`); `Clear` without a
       value; `xmpRights:Marked` `True` with `Replace` when Copyright has a text and the mode
       `Replace`, `Append` or `Prefix` (D6);
     - `IptcForm::template_is_empty(&self) -> bool`: true when no field would do anything (each
       is `Keep`, or `Replace`/`Append`/`Prefix` with an empty text);
     - `IptcForm::cleared_fields(&self) -> Vec<IptcField>`: the fields in `Clear`, in order;
     - `pub fn clear_confirmation_text(fields: &[IptcField]) -> String` (D14): "Mallen tömmer de
       här fälten i alla bilder som importeras:" and one line per field with its label;
     - `to_iptc` stays (its tests unchanged).
  2. **`import_worker.rs`:** `apply_iptc` takes the form's `to_template()` instead of
     `Iptc::to_template`, and skips the ExifTool start when `template_is_empty()` (as
     `iptc.is_empty()` does today, so `empty_iptc_template_does_not_start_exiftool` keeps
     passing).
  3. **`main.rs`, the dialog:** after each of the six fields a drop-down list of its
     `allowed_modes` with `mode_text`; the text field is disabled when the mode is `Behåll` or
     `Rensa`. The modes are kept with the rest of the form between imports in the session
     (`last_form`), as the texts are.
  4. **`main.rs`, the start (D14):** when `cleared_fields()` is not empty, pressing the start
     button first opens a window "Rensa fält?" with `clear_confirmation_text` and the buttons
     "Rensa och importera" (starts the import) and "Avbryt" (back to the dialog, nothing
     started). Asked once per start of an import, never per image.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Nothing in `pw-metadata` changes.
- **Tests to add:**
  - `import.rs`: `to_template` of the default form with texts gives the same template as
    `to_iptc().to_template()` (all six filled; none filled); Append on Creator, Prefix on Credit,
    Clear on E-post (no value) and Keep on a filled Copyright (no `xmpRights:Marked`);
    `allowed_modes` for each field; `template_is_empty` (all Keep; all Replace with empty texts;
    one Clear → false); `cleared_fields` and `clear_confirmation_text` for two fields;
  - `import_worker.rs` (ExifTool required, as the existing IPTC import test): an import of the
    JPEG fixture whose file already has a work e-mail, with E-post in `Clear`, leaves no e-mail
    (read back with `read_metadata`); Creator in `Append` on a file with a creator gives both
    names in order.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand: an import with a
  cleared Webb asks once and clears it in every imported image.
