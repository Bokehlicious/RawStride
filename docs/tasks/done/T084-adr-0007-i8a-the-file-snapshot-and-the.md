# T84. ADR 0007 I8a: the file snapshot and the verification of a written copy — Done

- **Can start now.** No dependency change.
- **Crates:** `pw-metadata` only. Nothing uses it yet: T86 builds the safe write on it; the old
  `write_rating`/`write_iptc` stay unchanged until I9.
- **Files:** new `crates/pw-metadata/src/verify.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); new `crates/pw-metadata/tests/verify.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §9.1 points 1 and 3 (the file identified by content, its state recorded) and
  §9.3 points 1–3 (the copy checked: image data unchanged, metadata changed only where written,
  structure intact). Measured by the reviewer with ExifTool 13.59 (see the step plan).
- **What:** `pub mod verify` with:
  1. **`FileSnapshot { file_type: String, size: u64, modified: SystemTime, image_data_hash:
     Option<String>, validate: Option<Validate>, listing: BTreeMap<String, String> }`** and
     `Validate { errors: u32, warnings: u32, minor: u32 }`.
  2. **`ExifTool::snapshot(&mut self, file: &Path) -> Result<FileSnapshot>`:** `size` and
     `modified` from `std::fs::metadata`; everything else from **one** `self.execute` call:
     `-j -a -G1 -n -struct -api structformat=jsonq -api ImageHashType=SHA256 -all -ImageDataHash
     -validate -warning <file>` (as in T82, never a command line).
     - `file_type` is `File:FileType` (`JPEG`, `TIFF`, `DNG`, `XMP`; ExifTool decides by content);
     - `image_data_hash` is `File:ImageDataHash` (none for XMP);
     - `validate` from `ExifTool:Validate`, which `-n` prints as three numbers (`"0 7 1"`: errors,
       warnings, minor warnings);
     - `listing`: every other key with its value as text (a JSON string as it is, any other JSON
       value as its JSON text, so a structure is compared whole). The keys `SourceFile`,
       `ExifTool:Warning` (JSON keeps only one of several; the count is in `validate`) and
       `ExifTool:Validate` are left out.
     A missing or unreadable file is the error ExifTool reports, as in `read_metadata`.
  3. **`check_file_type(expected: FileKind, snapshot: &FileSnapshot) -> Result<(), VerifyError>`**
     (§9.1 point 1): `Jpeg` needs `JPEG`, `Tiff` `TIFF`, `Dng` `DNG`, and `Raw` (whose sidecar is
     written) `XMP`; anything else is `WrongFileType { expected, found }`. So a JPEG renamed `.dng`
     is refused.
  4. **`verify_copy(original: &FileSnapshot, copy: &FileSnapshot, plan: &WritePlan) -> Result<(),
     VerifyError>`** (§9.3):
     - `file_type` equal, otherwise `FileTypeChanged`;
     - `image_data_hash` equal, otherwise `ImageDataChanged`;
     - `validate` of the copy not higher than the original's in errors or warnings, otherwise
       `NewWarnings { before, after }` (the reviewer's Xiaomi DNG already has `0 7 1`, so "no
       warnings" cannot be the rule);
     - **every** key added, removed or changed in `listing` must be allowed, otherwise
       `Unexpected { key, before: Option<String>, after: Option<String> }` for the first one in key
       order. Allowed:
       - the groups `System`, `ExifTool` and `Composite` (file name, size and dates; tool data;
         values ExifTool derives, not stored);
       - `File:CurrentIPTCDigest`;
       - tags whose name ends in `Offset`, `Offsets` or `Start`, in any group (measured: a write
         moves `IFD0:TileOffsets` and `SubIFD:PreviewImageStart` of a DNG); a pointer that
         changes and has another name is reported to the owner, not added on your own;
       - `XMP-x:XMPToolkit`, `XMP-xmp:MetadataDate`, `XMP-xmp:ModifyDate`;
       - the tags of the plan's `args`: for each `-<Group>:<Tag>…=` the key `<Group>:<Tag>` and
         every key `<Group>:<Tag>-<language>`, where `<Tag>` is cut at the first `-` and a
         trailing `#` is dropped (`-XMP-dc:Description-x-default=` allows `XMP-dc:Description` and
         `XMP-dc:Description-sv`; `-IPTC:DateCreated#=` allows `IPTC:DateCreated`); `-IPTCDigest=`
         allows `Photoshop:IPTCDigest`;
       - when the plan writes IIM (`-IPTC:CodedCharacterSet=UTF8` is in it):
         `IPTC:EnvelopeRecordVersion` and `IPTC:ApplicationRecordVersion` (measured: ExifTool adds
         them when it creates IIM).
  5. `VerifyError` with `Display` texts in English that name the key or check (they are shown in
     T86's error), and `std::error::Error`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:**
  - unit tests in `verify.rs` with hand-built snapshots: an unchanged copy passes; each allowed
    kind of difference passes (a `System` key, `File:CurrentIPTCDigest`, `SubIFD:PreviewImageStart`,
    `XMP-x:XMPToolkit`, a Lang Alt in two languages for `-XMP-dc:Description-x-default=`, a
    structure, `IPTC:DateCreated` for `-IPTC:DateCreated#=`, the IIM record versions only with
    `-IPTC:CodedCharacterSet=UTF8`, `Photoshop:IPTCDigest` for `-IPTCDigest=new`); an added, a
    removed and a changed key outside the plan are `Unexpected` with both values;
    `XMP-photoshop:CityCode` is not allowed by `-XMP-photoshop:City=` (a whole tag, not a prefix);
    a different hash, a different file type, one more warning and one more error are errors, fewer
    warnings pass; `check_file_type` for every `FileKind`; the `Validate` text `"0 7 1"` and a
    missing `Validate`;
  - `tests/verify.rs` (integration, **ExifTool required**, the JPEG fixture of T82 copied in): a
    plan from `write_plan::plan` (City, a Description in two languages, keywords, IIM created with
    the setting) written with `ExifTool::execute` and `-o <name>.pwtmp-1.jpg` to a new file;
    `snapshot` of both; `verify_copy` passes, and the hash is equal. The same copy against a plan
    without the City arguments is `Unexpected` for `XMP-photoshop:City`. A JPEG renamed `.dng` is
    `WrongFileType`. One test **ignored by default**: every DNG in `PW_TESTDATA`, copied to a
    temporary folder, written with a City plan and `-o`, passes `verify_copy` (report the
    `Validate` counts and the allowed keys that changed for each file in the log row).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
