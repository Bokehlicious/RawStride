# T82. ADR 0007 I6: the reader — ExifTool output into readings, sidecar rules — Done

- **Can start after T81 is accepted.** D2 (`serde_json` = 1.0.154 in `pw-metadata`) was approved by
  the owner on 2026-11-11. `Cleared` (D1) is not read in this task: `Readings::cleared` stays
  empty.
- **Crates:** `pw-metadata` only. Nothing in RawStride uses the reader yet (the info panel is I10);
  the old `read_ratings` is unchanged.
- **Files:** new `crates/pw-metadata/src/read.rs`; `crates/pw-metadata/src/date.rs` (**only** the new
  function of point 4 and its tests); `crates/pw-metadata/src/lib.rs` (the `mod` line only);
  `crates/pw-metadata/Cargo.toml` (one line: `serde_json = "=1.0.154"` under `[dependencies]`);
  `Cargo.lock` (by cargo); new `crates/pw-metadata/tests/read.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §4 (where each format keeps its metadata), §4.1 and Q5 (a foreign sidecar next
  to a DNG is ignored and reported), §4.2 and Q8 (`<basename>.xmp`; `<basename>.<ext>.xmp` is
  ignored and reported; RAW+JPEG pairs), §5.4/§6.2 (decoding with the fallback), §6.5 (Copyright
  parts), §12 (round trip per format).
- **What:**
  1. **`ExifTool::read_metadata(&mut self, image: &Path, fallback: Fallback) ->
     Result<MetadataRead>`** with `MetadataRead { reconciled: Reconciled, file_notices:
     Vec<FileNotice> }` and `FileNotice { ForeignSidecarIgnored(PathBuf),
     ExtSidecarIgnored(PathBuf), IimBlockUnreadable(String) }`.
  2. **File kind** by extension, from the existing lists in `lib.rs` (do not change them):
     `supports_sidecar` → `Raw`; `dng` → `Dng`; `jpg`/`jpeg` → `Jpeg`; `tif`/`tiff` → `Tiff`; anything
     else is `Error::Unsupported`.
  3. **ExifTool calls** through `self.execute` (never a command line, see the measurements above):
     - the image file: `-j -n -b -struct -G1 -XMP:all -IPTC -Photoshop:IPTCDigest
       -CurrentIPTCDigest -IFD0:ImageDescription -IFD0:Artist -IFD0:Copyright
       -ExifIFD:Photographer -ExifIFD:ImageUniqueID -ExifIFD:DateTimeOriginal
       -ExifIFD:SubSecTimeOriginal -ExifIFD:OffsetTimeOriginal <file>`;
     - for `Raw`, also the sidecar `<basename>.xmp` (`sidecar_path`) if it exists, with
       `-j -n -b -struct -G1 -XMP:all <sidecar>`; its XMP is `Readings::xmp`, the RAW file's XMP is
       `camera_xmp`. Without a sidecar `xmp` is empty;
     - for `Dng`: a `<basename>.xmp` next to it is **never read** and gives
       `ForeignSidecarIgnored` (Q5);
     - for every kind: a `<basename>.<ext>.xmp` (e.g. `DSC01234.ARW.xmp`) gives `ExtSidecarIgnored`
       and is never read (Q8). Compare names without regard to case (Windows).
     If ExifTool reports the file missing or unreadable, return the error as `read_ratings` does.
  4. **XMP JSON → values** (with `serde_json::Value`): a key is mapped to a property by the table's
     `et_xmp` (`properties()`); unknown keys are ignored (they stay in the file).
     - Lang Alt: `<et_xmp>` is `x-default`, `<et_xmp>-<language>` is that language (the language may
       contain `-`, e.g. `en-US`); the entries are kept in the order of the JSON object (enable
       `serde_json`'s `preserve_order` feature **only if** it adds no new package to `Cargo.lock`;
       otherwise stop and report);
     - lists (JSON arrays) → `List`; objects → `Struct`, arrays of objects → `StructList`; structure
       fields by the field's `et_tag` (`FieldDef`), at any depth, Lang Alt fields as above;
     - JSON numbers → `Number` with the number's text; JSON strings → by the kind;
     - **dates:** add `IptcDate::parse_exiftool(text) -> Result<IptcDate, DateError>` in `date.rs`:
       ExifTool's form (`2026`, `2026:11`, `2026:11:04`, `2026:11:04 10:15`, `…:30`, `…:30.5`, each
       with or without `Z`/`±hh:mm`) is turned into the XMP form (the first two `:` of the date
       become `-`, the space becomes `T`) and parsed with `parse_xmp`. An unparsable date gives
       `Notice::Unreadable` through the reconciliation's notices (keep the text);
     - a value that cannot be built is passed on and refused by `reconcile` (`Notice::Refused`).
  5. **IIM:** the `IPTC` value (`base64:…`) is decoded (a small base64 decoder in `read.rs`, no new
     dependency), parsed with `parse_block` and turned into values with `iim_values`;
     `IimError` gives `FileNotice::IimBlockUnreadable` and no IIM. The digest: `Matches` when
     `IPTCDigest` equals `CurrentIPTCDigest`, `Differs` when both exist and differ, `Missing`
     without `IPTCDigest`. No `IPTC` key → `iim: None`.
  6. **Exif:** each value is a JSON string, or `base64:…` when ExifTool found bytes that are not
     UTF-8 (`-b`); those bytes are decoded with `iim::decode(bytes, &CodedCharset::Absent,
     fallback)` (§6.2: ASCII → UTF-8 → fallback). Copyright: parts separated by NUL are joined with
     a line feed (§6.5); trailing NULs and spaces are dropped. Blank values are `None`.
  7. `reconcile(readings)` gives `reconciled`.
- **Rules:** rules 3–7 and 10–12. The `Cargo.toml` line of D2 is the only dependency change. Check
  with `git diff Cargo.lock`: `serde_json` added to `pw-metadata`'s dependency list, **no new
  package**; otherwise stop and report.
- **Tests to add:**
  - `date.rs`: `parse_exiftool` for every form of point 4 gives the same `IptcDate` as `parse_xmp`
    of the XMP form; `2026:11:04T10:15` and `2026-11-04 10:15` are errors;
  - `read.rs` unit tests: the base64 decoder (RFC 4648 vectors, padding, invalid input is an error,
    never a panic); JSON → values for a hand-written JSON object with a Lang Alt in three languages
    (order kept, `en-US`), a list, a structure, a list of structures with a Lang Alt field, a date
    and a number; the digest cases; the Copyright NUL join; Exif `base64:` bytes in Windows-1252;
  - `tests/read.rs` (integration, **ExifTool required**; it fails with an explanation when ExifTool
    is missing, as `tests/sidecar.rs` does): a JPEG made from the `SMALL` bytes of
    `crates/pw-thumbcache/src/jpeg_fixture.rs` (copied into the test file), written with
    `ExifTool::execute` in a temporary folder:
    - XMP Description in two languages, two creators, Creator's Contact Info, Location Created,
      Rating `2.5`, Date Created `2026-11`, plus IIM City and Keywords with `-IPTCDigest=new`
      → read back exactly, digest `Matches`, no conflicts;
    - then IIM City changed without `-IPTCDigest=new` → `Differs`, City from IIM, conflict `Iim`;
    - IIM without a digest and a different XMP City → `Missing`, conflict `Open`;
    - Exif Artist and ImageDescription only → filled from Exif;
    - a fake RAW (`TN0001.ARW`, bytes as in `tests/sidecar.rs`) with `TN0001.xmp` → `XmpSidecar`
      values; with `TN0001.ARW.xmp` → `ExtSidecarIgnored`;
    - a JPEG renamed `.dng` is not a DNG, so a DNG test needs a real file: one test **ignored by
      default** reads the first DNG in `PW_TESTDATA` with a `<basename>.xmp` written next to a copy
      of it in a temporary folder → `ForeignSidecarIgnored`, and the sidecar's value is not used.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
