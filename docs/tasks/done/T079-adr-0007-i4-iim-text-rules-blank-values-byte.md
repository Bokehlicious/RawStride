# T79. ADR 0007 I4: IIM text rules — blank values, byte limits, prefixes, character encodings — Done

- **Can start now.** Independent of T76 and T77 (the byte limit is a parameter here; I5 takes it
  from the table).
- **Crates:** `pw-metadata` only. Nothing uses the module yet.
- **Files:** new `crates/pw-metadata/src/iim.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 (blank values; equality on the predicted IIM value), §5.2 (cut at a UTF-8
  boundary, the user is told), §5.3 (prefixes of 2:04 and 2:12), §5.4 and Q10 (encodings and the
  fallback), §6.2 (the same order for Exif text).
- **What:** `pub mod iim` with:
  1. `is_blank(&str) -> bool`: true for an empty text and for a text of only spaces and/or NUL
     characters (MWG §4.2.3.3). Other whitespace (tabs, line feeds) is not blank.
  2. `truncate_utf8(text: &str, max_bytes: usize) -> (&str, bool)`: the longest prefix of at most
     `max_bytes` **bytes** that ends on a character boundary, and whether it was cut.
  3. `IimFormat { Plain, Genre, SubjectCode }` and
     `predict(text: &str, format: IimFormat, max_bytes: Option<u16>) -> Predicted { value: String,
     truncated: bool }`: the IIM value RawStride would write.
     - `Plain`: the text cut to `max_bytes`.
     - `Genre` (2:04, §5.3): `000:` + the code, where the 64-byte limit applies to the code only.
     - `SubjectCode` (2:12): `IPTC:` + the code, the limit counting the whole value. Check this
       against IPTC §7.23 in `D:\PhotoWorkflowData\references\IPTC-PhotoMetadata-2025.1.md`; if the
       spec says otherwise, **stop and report**.
  4. `strip_prefix(value: &str, format: IimFormat) -> &str` for reading: `Genre` removes a leading
     three digits and a colon (any number, not only `000`); `SubjectCode` removes a leading `IPTC:`;
     a value without the prefix is returned unchanged.
  5. `same_as_predicted(actual: &str, predicted: &Predicted) -> bool`: equal when the texts are
     equal, or when both are blank (§5.1: a value that only differs because of IIM truncation or
     encoding is not a conflict; the caller compares decoded text).
  6. **Decoding** (§5.4, §6.2):
     - `CodedCharset { Utf8, Latin1, Other(Vec<u8>), Absent }` and `coded_charset(value: Option<&[u8]>)
       -> CodedCharset` for dataset 1:90: `ESC % G` (`1B 25 47`) is `Utf8`; `ESC . A`
       (`1B 2E 41`) is `Latin1` (ISO 8859-1); any other bytes are `Other`.
     - `Fallback { Windows1252, Latin1 }` (default `Windows1252`, Q10).
     - `decode(bytes: &[u8], charset: &CodedCharset, fallback: Fallback) -> Decoded { text: String,
       converted: bool }`: a leading UTF-8 BOM is removed. `Utf8`: UTF-8, invalid sequences
       replaced with U+FFFD and `converted` set. `Latin1`: ISO 8859-1. `Absent` and `Other`: all
       bytes < 128 → ASCII; otherwise valid UTF-8 → UTF-8; otherwise the fallback with
       `converted` set. `Other` always sets `converted` (the declared encoding is not one of the two
       RawStride decodes; see the note below).
     - Windows-1252 by a table in the code for 0x80–0x9F; the five unassigned bytes (0x81, 0x8D,
       0x8F, 0x90, 0x9D) become the C1 control character of the same number, as Windows does; no
       new dependency.
     - `pub const UTF8_CODED_CHARSET: &[u8] = b"\x1b%G";` for writing (MWG: IIM is written as UTF-8
       with 1:90).
  7. Module comment with the ADR sections, and the note: other declared IIM encodings (e.g. ISO
     2022 sets) are decoded by the fallback order and marked converted; decoding them properly would
     need a new dependency and an owner decision.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `iim.rs`):
  - `is_blank`: `""`, `"   "`, `"\0\0"`, `" \0 "` true; `"\t"`, `" a "` false;
  - `truncate_utf8("Åsa", 1)` is `("", true)`; `("Åsa", 2)` is `("Å", true)`; a 33-byte `Plain`
    City with `max_bytes` 32 is cut to 32 bytes; a value of `ä` × 20 (40 bytes) cut to 32 gives
    `ä` × 16, never half a character;
  - `predict("Portrait", Genre, Some(64))` is `000:Portrait`; a 70-byte genre code is cut to 64
    bytes **before** the prefix; `predict("04000000", SubjectCode, Some(236))` is
    `IPTC:04000000`;
  - `strip_prefix("012:Feature", Genre)` is `Feature`; `strip_prefix("IPTC:04000000",
    SubjectCode)` is `04000000`; `strip_prefix("Feature", Genre)` is `Feature`;
  - `same_as_predicted`: the 32-byte cut value equals the prediction of the long text; `"  "`
    equals the prediction of `""`; a different word does not;
  - decoding: `coded_charset(Some(b"\x1b%G"))` is `Utf8`; with `Absent`, `b"abc"` is ASCII
    (not converted), `"Göteborg"` as UTF-8 bytes is UTF-8 (not converted), `b"G\xf6teborg"` is
    `Göteborg` via Windows-1252 (converted); `b"\x80"` is `€` with `Windows1252` and U+0080 with
    `Latin1`; `b"\x81"` is U+0081; a UTF-8 BOM is removed; `Other(..)` with ASCII bytes gives the
    text with `converted` set; invalid UTF-8 under `Utf8` gives U+FFFD and `converted`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
