# T80. ADR 0007 I5a: the IIM block — datasets, decoding, values — Done

- **Can start now.** T76–T79 are done. Plain Rust; no ExifTool. Nothing uses the module yet.
- **Crates:** `pw-metadata` only.
- **Files:** new `crates/pw-metadata/src/iim_read.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 (blank values are non-existent), §5.3 (prefixes; 2:55 + 2:60 merged into
  one Date Created), §5.4 (1:90, decoding, the fallback marks the value as converted), §5.5 (the
  deprecated datasets are read like the others). The reader (T82) gets the raw IIM block from
  ExifTool (`-b -IPTC`, measured above) and needs it as values of the model (T78) for the
  reconciliation (T81).
- **What:** `pub mod iim_read` with:
  1. **`Dataset { record: u8, dataset: u8, data: Vec<u8> }`** and
     **`parse_block(bytes: &[u8]) -> Result<Vec<Dataset>, IimError>`**, in file order:
     - each dataset is the tag marker `0x1C`, the record number, the dataset number and a 2-byte
       big-endian length, then the data;
     - a length with the top bit set is an **extended length**: its low 15 bits give how many of the
       following bytes hold the real length (big-endian); 1–4 bytes are accepted, more is an error;
     - zero bytes after the last complete dataset are padding and are ignored (Photoshop pads its
       resources to an even length); any other byte where a tag marker is expected, and a length
       that runs past the end, are `IimError::Malformed { offset }` (the offset of the bad byte);
     - an empty block is `Ok(vec![])`. Never a panic and no unchecked indexing, whatever the bytes.
  2. **`IimError`** with `Display` (English, "IIM block is malformed at byte {offset}") and
     `std::error::Error`.
  3. **`iim_format(xmp_id: &str) -> IimFormat`**: `Genre` for
     `Iptc4xmpCore:IntellectualGenre`, `SubjectCode` for `Iptc4xmpCore:SubjectCode`, `Plain`
     otherwise (T79's formats; T83 uses the same function for writing).
  4. **`IimValues { values: Vec<(String, Value)>, charset: CodedCharset, notices: Vec<IimNotice> }`**
     and **`iim_values(datasets: &[Dataset], fallback: Fallback) -> IimValues`**:
     - `charset` is `iim::coded_charset` of dataset 1:90 (the first one; `Absent` without it). Every
       other record-1 dataset and 2:00 (record version) are ignored;
     - every record-2 dataset is decoded with `iim::decode` under that charset; a decoded value that
       is `is_blank` is **non-existent** (§5.1) and skipped;
     - the property is `properties::by_iim(2, dataset)`; a dataset without a property is ignored (it
       stays in the file, because RawStride never writes it);
     - the prefix is removed with `strip_prefix(text, iim_format(xmp_id))`;
     - the value follows the property's kind and occurrence (T78 point 6): a `Multi` property gives
       one `List` of all its datasets in file order; a `Single` property takes its **first**
       dataset, and a repeated one adds `IimNotice::Repeated { property }`; `Text`/`Uri`/`Url` give
       `Text`, `LangAlt` gives a Lang Alt with one `x-default` entry, `Integer`/`Number` give
       `Number` with the text as decoded (no trimming);
     - **Date Created:** 2:55 with the first 2:60 (if any) through `IptcDate::from_iim`. A 2:55 of
       only zeros (`00000000`) is **non-existent** (T77 review). An error, or a 2:60 without a 2:55,
       gives `IimNotice::Unreadable { property, text }` (the text as read, 2:55 and 2:60 joined with
       a space) and no value;
     - any other value that cannot be built (a kind that has no IIM form, a number that does not
       fit the kind) gives `Unreadable` and no value. Values are checked by the same rules as
       `Metadata::insert_read`; use it (or the same check) instead of a copy of the rules;
     - a value decoded with `converted` set gives `IimNotice::Converted { property }` **once per
       property**;
     - `values` is ordered by the position of each property's first dataset.
  5. **`IimNotice { Converted { property: String }, Repeated { property: String },
     Unreadable { property: String, text: String } }`** (`Clone`, `Debug`, `PartialEq`, `Eq`).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `iim_read.rs`; byte strings written out in the test):
  - **a block written by ExifTool 13.59** (base64
    `HAFaAAMbJUccAQAAAgAEHAIZAANldHQcAhkAA3R25RwCWgAIR/Z0ZWJvcmccAgAAAgAE`, decoded in the test
    without a new dependency, e.g. a small base64 helper in the test module): six datasets 1:90,
    1:00, 2:25, 2:25, 2:90, 2:00. The block declares UTF-8 but holds Latin-1 bytes (the arguments
    came through the Windows command line), so `dc:subject` is `["ett", "tv\u{FFFD}"]` and
    `photoshop:City` is `"G\u{FFFD}teborg"`, each with a `Converted` notice;
  - the same datasets without 1:90: `["ett", "två"]`, `"Göteborg"`, `Converted` for both
    (Windows-1252 fallback); with the `Latin1` fallback the same text;
  - extended length (a 2:120 of 40 000 bytes with a 2-byte extended length) is read; a 5-byte
    extended length, a length past the end, a missing tag marker, and 3 bytes of garbage are
    `Malformed` with the right offset; trailing zero bytes are ignored; random byte strings of
    every length 0–64 (a fixed seed, no new dependency) never panic;
  - blank values: a 2:90 of spaces, of NUL bytes, and an all-blank 2:25 list give no value;
    `00000000` in 2:55 gives no value and no notice;
  - prefixes: `2:04 = "000:Feature"` gives `Iptc4xmpCore:IntellectualGenre` `Feature`;
    `2:12 = "IPTC:04000000"` gives `Iptc4xmpCore:SubjectCode` `["04000000"]`;
  - kinds: 2:120 gives a Lang Alt `[("x-default", …)]` for `dc:description`; 2:80 twice gives
    `dc:creator` `List` of two; 2:90 twice gives the first and `Repeated`; 2:10 `"5"` gives
    `photoshop:Urgency` `Number("5")`; 2:10 `"x"` gives `Unreadable`;
  - Date Created: `20261104` + `101530+0100` gives the date with zone; `20261104` alone gives a date
    without time; `20261304` and a 2:60 without 2:55 give `Unreadable` with the text;
  - an unknown dataset (2:200) is ignored; 1:00 and 2:00 are ignored;
  - `iim_format` for the two prefixed properties and `photoshop:City`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
