# T97. ADR 0007 I10d: edits of single properties written safely, with the write notices — Done

- **Can start now.** No dependency change. Can run at the same time as T94, T95, T96 and T99 (no
  shared files).
- **Crates:** `pw-metadata` only. The worker gets it in T98, the editor in T104/T105.
- **Files:** `crates/pw-metadata/src/edit.rs`; `crates/pw-metadata/src/template.rs` (only to make
  its sub-field helper `pub(crate)` if you reuse it; no behaviour change); new
  `crates/pw-metadata/tests/edits.rs`; `crates/pw-metadata/src/verify.rs` (point 6 only, owner
  decision 2026-11-12); `docs/test-inventory.md`.
- **Why:** ADR 0007 §8 (clearing removes every representation; Lang Alt edits `x-default` only),
  §5.1 and Q1 (an `Open` conflict is decided by the user: an edit of that property writes both forms),
  §5.2 and §5.4 (the user is told when IIM was cut or converted: today the plan's
  `WriteNotice`s are dropped by `write_safely`), §9. The editor needs to set and clear any property
  of the table, which the template (§10: no Clear of copyright fields) does not allow.
- **What** (all in `edit.rs`):
  1. `pub enum Edit { Set { property: String, field: Option<String>, value: Value }, Clear {
     property: String, field: Option<String> } }`; `field` is the XMP id of a sub-field of a
     **single** structure (as `TemplateField::field`, T88).
  2. **`pub fn apply_edits(metadata: &mut Metadata, edits: &[Edit]) -> Result<(), EditError>`**,
     edit by edit in order:
     - `Set` without `field`: `Metadata::set`, **except** when the value equals the current value
       (`reconcile::same_value`) **and** the property has no conflict with the rule `Open` (then
       nothing is set). So choosing the value already shown in an `Open` conflict still decides it
       (both forms written, Q1);
     - `Set` with `field`: the structure's current value (or an empty one), the field set (a new
       field in the order of the structure's fields, as T88), then `set` of the whole structure,
       with the same rule;
     - `Clear` without `field`: `Metadata::clear` when the property has a value or an `Open`
       conflict; otherwise nothing. **Copyright fields may be cleared here** (§8 forbids it for
       templates only);
     - `Clear` with `field`: the field removed; the property cleared when no field is left;
       nothing when the field is not there;
     - a field of a **list** of structures, or a `field` on a property that is not a single
       structure → `EditError::Unsupported("<property>/<field> cannot be edited field by field")`;
     - errors of `set`/`clear` are `EditError::Value`.
  3. `pub struct EditReport { pub report: WriteReport, pub notices: Vec<WriteNotice> }`.
  4. **`ExifTool::write_edits_safely(&mut self, image: &Path, edits: &[Edit], options:
     &EditOptions, checks: &dyn ImageCheck, journal: &Journal) -> Result<EditReport, EditError>`**:
     `supports_rating(image)` false → `Unsupported` (same text as `write_iptc`); `read_metadata`
     with `options.fallback`; `apply_edits`; `write_plan::plan` with `options.settings`;
     `write_safely`; the report with the plan's `notices` (empty when `Unchanged`).
  5. Module comment extended (§5.1/Q1, §5.2, §8).
  `write_rating_safely` and `apply_template_safely` are unchanged.
  6. **The Exif block that ExifTool drops** (`verify.rs`; owner decision A, 2026-11-12, found in
     this task): in a JPEG whose only Exif is `IFD0:Copyright`, clearing it (`-IFD0:Copyright=`)
     makes ExifTool 13.59 drop the whole Exif block, so `File:ExifByteOrder` and
     `IFD0:YCbCrPositioning` (which ExifTool adds itself when it creates Exif) are removed too and
     `verify_copy` refuses the write as `Unexpected` (measured by the reviewer: with any other Exif
     tag left, such as `IFD0:Make`, `IFD0:XResolution` or `ExifIFD:ExifVersion`, the block and
     both keys stay). New private `fn drops_exif_block(original, copy, plan) -> bool`: the plan
     has an argument that clears an `IFD0:` or `ExifIFD:` tag (`-IFD0:<tag>=` or
     `-ExifIFD:<tag>=` with an empty value), the original has `File:ExifByteOrder`, and the copy
     has no `File:ExifByteOrder` and no key in the groups `IFD0`, `ExifIFD`, `IFD1`, `GPS` or
     `InteropIFD`. When it is true, the **removal** (a value before, none after) of exactly
     `File:ExifByteOrder` and `IFD0:YCbCrPositioning` is allowed in the key loop of
     `verify_copy`; a changed value of either key is still `Unexpected`. Comment with the
     measurement. Nothing else in `verify.rs` changes.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs`,
  `safe_write.rs` or `value.rs`.
- **Tests to add:**
  - unit tests of `apply_edits` in `edit.rs` on hand-built `Metadata`: a text set; the same text →
    no change (`changes()` empty); the same text on a property with an `Open` conflict → changed;
    a Lang Alt with an `sv` entry keeps it; a clear of `dc:rights` (allowed here); a clear of an
    absent property → nothing; a contact field set (other fields kept), cleared, the last field
    cleared clears the property; a field of `plus:ImageCreator` and a `field` on `photoshop:City`
    → `Unsupported`; an unknown property → `Value`;
  - `tests/edits.rs` (integration, **ExifTool required**; the JPEG fixture and the fake RAW of
    T82 in temporary folders; `NoImageCheck`; a temporary journal):
    - Headline and City set on a JPEG → `Written`, read back with `read_metadata`; the same again
      → `Unchanged`;
    - a JPEG with IIM (City) and a matching digest, a 40-byte City → `Written` with
      `WriteNotice::Truncated` for `photoshop:City`, XMP holding all 40 bytes;
    - a JPEG with City in IIM and another City in XMP and **no** checksum (an `Open` conflict),
      the XMP value set again → `Written`, both forms equal afterwards and no conflict when read;
    - `dc:rights` cleared on a JPEG with `IFD0:Copyright` → gone from XMP and Exif (§8);
    - the fake RAW: a Title set → only the sidecar written, the RAW's bytes unchanged.
  - `verify.rs` unit tests (point 6, with the existing helpers): the two keys removed with
    `-IFD0:Copyright=` in the plan and no Exif key left → passes; the same with `IFD0:Make` still
    in the copy → `Unexpected` for `File:ExifByteOrder`; the same without a clearing argument (a
    plan that sets `-IFD0:Copyright=x`) → `Unexpected`; `File:ExifByteOrder` changed from `MM` to
    `II` with the clearing argument → `Unexpected`.
  - `tests/edits.rs` `clearing_the_copyright_notice_removes_it_from_xmp_and_exif` (the JPEG whose
    only Exif is Copyright) passes unchanged.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
