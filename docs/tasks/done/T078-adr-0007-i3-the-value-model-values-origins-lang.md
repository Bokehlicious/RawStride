# T78. ADR 0007 I3: the value model — values, origins, Lang Alt, changes — Done

- **Can start now.** T76 and T77 are accepted and committed (4e7df84); this task uses the property
  table and `IptcDate`. Independent of T79.
- **Crates:** `pw-metadata` only. Nothing uses the model yet; `LocalizedValue` and `Iptc` stay as
  they are (they move to the new model in I9).
- **Files:** new `crates/pw-metadata/src/value.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `crates/pw-metadata/src/properties.rs` (**only** the new function of point 8 and its
  test); `docs/test-inventory.md`.
- **Why:** ADR 0007 §3 (one model keyed by the XMP id, value types from the spec, each value
  carries its origin), §8 (Lang Alt: edit `x-default` only, Q7; clearing removes the whole
  property; cardinalities), §10 (clearing is always explicit).
- **What:** `pub mod value` with:
  1. **`Origin { XmpEmbedded, XmpSidecar, Iim, Exif, Edited }`** (`Copy`, `Eq`, `Hash`, `Debug`).
  2. **`LangAlt`**: the entries `(language, text)` **in file order** (a `Vec`, not a map).
     - `LangAlt::from_entries(Vec<(String, String)>)`: languages compared without regard to case;
       a repeated language keeps its first entry.
     - `set_default(&mut self, text)`: if `x-default` exists, only its text changes; otherwise
       `x-default` is inserted **first**. Every other entry stays as it is, **including one that held
       the same text as the old `x-default`** (Q7). A text that is empty or only spaces gives
       `ValueError::Empty` (clearing goes through `Metadata::clear`).
     - `display(&self) -> Option<(&str, Option<&str>)>`: the `x-default` text with `None`;
       without `x-default` the first entry with `Some(language)` (§8 reading rule); `None` when
       empty.
  3. **`Value`**: `Text(String)`, `LangAlt(LangAlt)`, `List(Vec<String>)` (Seq or Bag from
     `properties::list_kind`), `Date(date::IptcDate)`, `Number(String)` (the text exactly as read,
     so a rating keeps its full precision, §2.9, §8), `Struct(StructValue)`,
     `StructList(Vec<StructValue>)`; `StructValue { fields: Vec<(String, Value)> }` keyed by the
     field's XMP id, in order.
  4. **`PropertyValue { value: Value, origin: Origin }`**; **`Conflict { property: String,
     values: Vec<(Origin, Value)>, rule: ConflictRule }`** with `ConflictRule { Xmp, Iim, Exif,
     Open }` (§5.1: the rule that was applied; `Open` = the user decides). Data only; filling it is
     I5.
  5. **`Metadata`**: values keyed by XMP id (`BTreeMap<String, PropertyValue>`), the conflicts, and
     the set of changed property ids.
     - `get(xmp_id) -> Option<&PropertyValue>`.
     - `insert_read(xmp_id, value, origin) -> Result<(), ValueError>` (for the reader): the
       property must be in `properties::property` (`UnknownProperty`) and the value must fit its
       kind (point 6); **no cardinality limit** (§8: all are kept when reading); not marked changed.
     - `set(xmp_id, value) -> Result<(), ValueError>` (for edits): as `insert_read`, plus
       `max_entries` (`TooMany`), and a value that is empty or only spaces (text, list, Lang Alt or
       structure without content) gives `Empty`; origin `Edited`; marked changed. A Lang Alt edit
       goes through `set_default` on the existing value, so other languages are kept.
     - `clear(xmp_id) -> Result<(), ValueError>`: removes the value (all languages of a Lang Alt) and
       marks it changed; an unknown property is `UnknownProperty`.
     - `changes() -> impl Iterator<Item = (&str, Option<&Value>)>`: each changed property with its
       new value, or `None` when cleared.
  6. **Kind check:** `Text`, `Uri`, `Url`, `Any` → `Text`; `LangAlt` → `LangAlt`; `Date` → `Date`;
     `Integer` → `Number` whose text parses as `i64`; `Number` → `Number` whose text parses as
     `f64`; `Struct` single → `Struct`, multi → `StructList`; any `Multi` non-structure → `List`.
     Otherwise `ValueError::Kind`. **Inside a structure the same check applies to every field,
     at any depth:** each field's id must be a field of that structure (`properties::struct_def`),
     and its value must fit that field's kind and occurrence by the same rules (a `Multi` field is
     a `List` or `StructList`). The `property` of a `Kind` error inside a structure names the
     property and the field (e.g. `Iptc4xmpCore:CreatorContactInfo/Iptc4xmpCore:CiAdrCity`).
  7. **`ValueError`** (`UnknownProperty(String)`, `Kind { property, expected }`,
     `TooMany { property, max }`, `Empty(String)`) with `Display` (English) and
     `std::error::Error`.
  8. **Seq or Bag for structure fields** (T76 review: `list_kind` covers top-level properties
     only). In `properties.rs`, next to `list_kind`:
     `pub fn field_list_kind(struct_name: &str, field_xmp_id: &str) -> Option<ListKind>`: `None`
     for an unknown structure or field and for a `Single` field; `Seq` for
     `Iptc4xmpExt:AOCreator` and `Iptc4xmpExt:AOCreatorId` in `ArtworkOrObject` and
     `Iptc4xmpExt:rbVertices` in `RegionBoundary`; `Bag` for every other `Multi` field. The
     reviewer found these three as `Seq` in the "XMP Specs" rows of
     `D:\PhotoWorkflowData\references\IPTC-PhotoMetadata-2025.1.md`; check every `Multi` field of
     every structure there. If the spec gives `Seq` for a field not named here, **stop and
     report**. A comment names ADR 0007 §3 and the check date, as `list_kind` does.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `value.rs`):
  - Lang Alt: `[("x-default","A"),("sv","A"),("en","B")]` + `set_default("C")` gives
    `[("x-default","C"),("sv","A"),("en","B")]` (Q7); `[("sv","A")]` + `set_default("C")` gives
    `[("x-default","C"),("sv","A")]`; `display` without `x-default` gives `("A", Some("sv"))`;
    `X-Default` and `x-default` are the same language; a repeated language keeps the first;
  - `set` + `clear` of `dc:description` removes all languages and `changes()` gives `None` for it;
  - `set("plus:ImageCreator", …)` with 4 entries is `TooMany { max: 3 }`; `insert_read` with 4
    entries is accepted;
  - kind errors: `Text` for `dc:subject`, `List` for `photoshop:City`, `Number("abc")` for
    `xmp:Rating`, a structure field that does not belong to `CreatorContactInfo`;
  - fields inside structures: `Iptc4xmpCore:CreatorContactInfo` with `Iptc4xmpCore:CiAdrCity` as
    `Text` is accepted, with `CiAdrCity` as `List` is a `Kind` error naming
    `Iptc4xmpCore:CreatorContactInfo/Iptc4xmpCore:CiAdrCity`; `Iptc4xmpExt:LocationCreated` as a
    `StructList` of one `Location` with `Iptc4xmpExt:City` as `Text` is accepted; an error two
    levels down (in `Iptc4xmpExt:ImageRegion`, a `RegionBoundary` whose `rbVertices` is `Text`) is
    found;
  - `field_list_kind` (in `properties.rs`): `("ArtworkOrObject", "Iptc4xmpExt:AOCreator")`,
    `("ArtworkOrObject", "Iptc4xmpExt:AOCreatorId")` and
    `("RegionBoundary", "Iptc4xmpExt:rbVertices")` are `Seq`; a `Multi` field of another structure
    is `Bag`; `("Location", "Iptc4xmpExt:City")` (single), an unknown field and an unknown
    structure are `None`; every `Multi` field of every structure has `Some`, every `Single` field
    `None`;
  - `Number("2.5")` for `xmp:Rating` is kept as the text `2.5`;
  - `set` with `""`, `"   "`, an empty list and an empty Lang Alt is `Empty`;
  - an unknown property (`foo:Bar`) is `UnknownProperty` for `set`, `insert_read` and `clear`;
  - `insert_read` marks nothing changed; `set` gives origin `Edited`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
