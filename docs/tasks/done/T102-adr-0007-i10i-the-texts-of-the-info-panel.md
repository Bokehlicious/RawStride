# T102. ADR 0007 I10i: the texts of the info panel (values, conflicts, notices) — Done

- **Can start now.** The owner approved D11 on 2026-11-12. It only needs the reader's types (T82), so it can run at the same time as T94–T100 (only
  `lib.rs` lines shared, see T99).
- **Crates:** `pw-loupe` only. Drawing the panel is T103.
- **Files:** new `crates/pw-loupe/src/info_panel.rs`; `crates/pw-loupe/src/lib.rs` (one `mod` line,
  one `pub use` line); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 ("conflicts always visible": both values and the rule applied, until the
  next write by RawStride), §6.2, §6.4 ("hidden by the user"), §5.4 (converted text marked), Q5/Q8
  (ignored sidecars reported), §7 (suggested Date Created).
- **What** (`info_panel.rs`, pure functions, no egui):
  1. `pub enum Language { Sv, En }` and `pub fn property_label(xmp_id: &str, language: Language)
     -> String` (owner, 2026-11-12: so an English RawStride later only has to choose the language):
     - `Sv`: the Swedish name of D11 for the properties and fields in its list, otherwise as `En`;
     - `En`: always the IPTC name from the table (`name` of `properties::property`, or of the field
       in `properties::struct_def` for a structure field), otherwise the id.
     The Swedish names are in **one** table in `info_panel.rs`, keyed by XMP id; no other code holds
     property names. Everything else in this task uses `Language::Sv` (the rest of RawStride's
     texts stay Swedish in the code; a language setting is not part of I10).
  2. `pub fn value_text(value: &Value) -> String`: `Text` and `Number` as they are; a Lang Alt its
     shown text, with ` (<språk>)` when it is not `x-default` and ` +N språk` when it has N other
     entries; a list its entries joined with `; `; a date `IptcDate::to_xmp()`; a structure
     `fält: värde` joined with `, ` (field labels by `property_label` of the field id); a list of
     structures one structure per line.
  3. `pub fn origin_text(origin: Origin) -> &'static str`: "XMP", "XMP-sidofil", "IIM", "Exif",
     "Ändrad".
  4. **`pub fn info_panel(read: &MetadataRead) -> InfoPanel`** with
     `InfoPanel { values: Vec<ValueRow>, conflicts: Vec<ConflictRow>, notices: Vec<String> }`:
     - `values`: each property with a value, in table order: `label`, `value_text`,
       `origin_text`;
     - `conflicts`: each conflict: `label`, each value with its origin, and the rule text:
       `Xmp` "XMP används", `Iim` "IIM används (kontrollsumman stämmer inte)", `Exif` "Exif
       används", `Open` "Olöst: välj värde i redigeraren";
     - `notices`, in this order (texts exactly so):
       - `FileNotice::ForeignSidecarIgnored(p)`: "<filnamn> bredvid DNG-filen används inte (skriven
         av ett annat program)"; `ExtSidecarIgnored(p)`: "<filnamn> läses inte (RawStride använder
         <basnamn>.xmp)"; `IimBlockUnreadable(r)`: "IIM-blocket kunde inte läsas: <r>";
       - `Notice::Iim(Converted)`: "<fält>: IIM-texten hade okänd teckenkodning och har
         konverterats"; `Repeated`: "<fält>: fanns flera gånger i IIM, det första värdet används";
         `Unreadable`: "<fält>: kunde inte läsas (<text>)";
       - `Notice::Refused`: "<fält> från <ursprung> lästes inte: <skäl>" with the reason in
         Swedish per `ValueError` ("fel form", "för många värden", "tomt värde", "okänt fält");
       - `Notice::HiddenByUser`: "<fält>: kamerans värde <värde> är dolt av användaren";
       - `Notice::Unreadable`: "<fält> från <ursprung> kunde inte läsas (<text>)";
       - `suggested_date_created`: "Förslag till Skapad från kameran: <datum>";
       - the IIM checksum when the file has IIM (`state`): `Matches` nothing, `Differs` "IIM har
         ändrats av ett annat program efter XMP (kontrollsumman stämmer inte)", `Missing` "IIM
         saknar kontrollsumma".
  5. **`pub enum PanelText`** and **`pub fn panel_text(text: &PanelText) -> String`** for the
     panel's fixed texts that T103 shows: `Selected(n)` "<n> bilder markerade", `NoneSelected`
     "Ingen bild vald", `Loading` "Läser metadata …", `ReadError(e)` "Metadata kunde inte läsas:
     <e>", and the section headings `Values` "Metadata", `Conflicts` "Konflikter", `Notices`
     "Meddelanden".
- **Text rule** (owner, 2026-11-12: so English can be added later in one module): every text the
  user sees in the info panel is made by a function in `info_panel.rs`; drawing code holds no
  user-visible string literals. Each text is a whole sentence built from one template with
  placeholders (`format!` with the values filled in), never joined from separately translated
  pieces, so another language can change the word order. Numbers that need singular/plural
  (such as ` +N språk`) are formatted inside the same function.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Nothing in `pw-metadata` changes.
- **Tests to add** (unit tests on hand-built `MetadataRead`): `property_label` for a listed, an
  unlisted and an unknown id, each with `Sv` and `En` (`dc:creator` gives "Fotograf" and
  "Creator"; a contact field gives "E-post" and the table's field name); `value_text` for every `Value` kind (Lang Alt with `x-default` only,
  without `x-default`, with two other languages; a list; a date with and without a zone; a contact
  structure; two Location Created); every rule text; every notice text above once; every
  `PanelText` once; the order of values follows the table.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
