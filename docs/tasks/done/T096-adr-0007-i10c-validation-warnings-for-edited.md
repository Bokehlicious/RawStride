# T96. ADR 0007 I10c: validation warnings for edited metadata (§11) — Done

- **Can start now.** No dependency change. Can run at the same time as T95, T97 and T99; not at
  the same time as T94 (both change `lib.rs`).
- **Crates:** `pw-metadata` only. The editor shows the warnings (T104).
- **Files:** new `crates/pw-metadata/src/validate.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §11: warnings, never refusals. PLUS values (I11) and the controlled vocabularies
  other than Digital Source Type are not part of this task; dates are checked when the editor turns
  a text into a date (T104).
- **What:**
  1. **`validate.rs`** (`pub mod validate`):
     - `pub enum Warning` (`Clone, Debug, PartialEq, Eq`):
       - `TooLongForIim { property: String, max_bytes: u16 }`: a value of a property with an IIM
         dataset and a byte limit whose IIM form would be cut
         (`iim::predict(text, iim_read::iim_format(property), max_bytes).truncated`; for a Lang Alt
         its shown text, for a list each entry, the property once);
       - `AltTextTooLong { characters: usize }`: `Iptc4xmpCore:AltTextAccessibility` (shown text)
         over 250 characters (`chars().count()`);
       - `CountryCodeForm { code: String }`: `Iptc4xmpCore:CountryCode` that is not 2 or 3 ASCII
         letters (§11: ISO 3166-1; the list of codes itself is not checked);
       - `JobtitleWithoutCreator`: `photoshop:AuthorsPosition` without `dc:creator`;
       - `DigitalSourceTypeUnknown { uri: String }`: `Iptc4xmpExt:DigitalSourceType` that does
         not start with `http://cv.iptc.org/newscodes/digitalsourcetype/` followed by a code;
       - `RegistryEntryIncomplete { index: usize }`: an entry of `Iptc4xmpExt:RegistryId` without
         `Iptc4xmpExt:RegItemId` or `Iptc4xmpExt:RegOrgId` (a blank text counts as missing; the
         index from 0);
       - `RatingOutOfRange { value: String }`: `xmp:Rating` that is not a number between −1 and 5
         (a real number inside is fine, §8);
     - **`pub fn validate(metadata: &Metadata, iim_written: bool) -> Vec<Warning>`**: the warnings
       of every value in `metadata`, the properties in table order (`properties::properties()`);
       `TooLongForIim` only when `iim_written` (the file has IIM, or the setting creates it);
     - `Display` in English for `Warning` (the Swedish texts are T104's).
  2. Module comment with §11 and what is left out (PLUS, I11; the other vocabularies; dates).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `iim.rs`, `iim_read.rs`
  or `properties.rs`; if `iim_format` or `predict` cannot be used as they are, stop and explain.
- **Tests to add** (unit tests in `validate.rs` on hand-built `Metadata`): every warning once with
  its exact content; a 33-byte City with `iim_written` → `TooLongForIim` with 32, without it
  nothing; `ä` × 16 (32 bytes) in City → nothing, × 17 → the warning; a keyword list with one long
  entry → one warning; Alt Text of 250 characters nothing, 251 the warning; country codes `SE`,
  `SWE`, `se` nothing, `S`, `SVER`, `S1`, `Sverige` the warning; Jobtitle with and without Creator;
  a Digital Source Type `http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture` nothing,
  the bare prefix and `digitalCapture` the warning; two registry entries, the second without an
  organisation → index 1; ratings `-1`, `0`, `2.5`, `5` nothing, `6`, `-2`, `abc` the warning; an
  empty `Metadata` gives nothing; the order follows the table.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
