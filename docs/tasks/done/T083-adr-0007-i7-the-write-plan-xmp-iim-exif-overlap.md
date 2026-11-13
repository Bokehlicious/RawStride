# T83. ADR 0007 I7: the write plan — XMP, IIM, Exif overlap fields, conflicts — Done

- **Can start after T82 is accepted** (its tests read the result back with `read_metadata`).
- **Crates:** `pw-metadata` only. The plan is not used for real files yet: the safe write procedure
  (§9) is I8; ratings and templates move to it in I9.
- **Files:** new `crates/pw-metadata/src/write_plan.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); new `crates/pw-metadata/tests/write_plan.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 (IIM written when present or with the setting, UTF-8 with 1:90, the
  checksum, Q1, byte limits), §5.4 (existing IIM in another encoding is converted on the first
  write), §5.5 (deprecated datasets never edited), §6.3 (only existing "both ways" Exif fields are
  synced, never created; DateTimeOriginal never written), §6.5 (first creator to Artist and
  Photographer), §7, §8 (Lang Alt, clearing removes every representation; a RAW is never changed).
- **What:** `pub mod write_plan` with:
  1. **Input:** `FileState { kind: FileKind, iim_present: bool, iim_charset: CodedCharset,
     iim_values: Vec<(String, Value)>, digest: IimDigest, exif_present: ExifPresent }`, where
     `ExifPresent { image_description, copyright, artist, photographer }` are `bool`s (the tag
     exists in the file) and `iim_values` are the values T80 read from the file's IIM;
     `WriteSettings { create_iim: bool }` (§5.2, off by default); and the `Metadata` with its
     `changes()` and `conflicts()`. T82 gets a helper `FileState::from_read(&MetadataRead, …)` only if
     it is needed here; otherwise the tests build `FileState` by hand.
  2. **Output:** `plan(metadata: &Metadata, state: &FileState, settings: WriteSettings) ->
     Result<WritePlan, PlanError>` with `WritePlan { options: Vec<String>, args: Vec<String>, notices: Vec<WriteNotice> }`.
     `args` are ExifTool tag arguments only (`-TAG=value`); `options` holds only `-ec` when needed;
    no file name and no other options such as
     `-o` or `-P` (they belong to I8). `WriteNotice { Truncated { property }, IimConvertedToUtf8,
     ConflictKept { property }, DigestNotWritten }`. `PlanError { NotEditable(String) }` for a
     changed deprecated IIM property (§5.5).
  3. **XMP** for each changed property (`et_xmp`):
     - text, number, URI: `-<et_xmp>=<text>`; date: `to_xmp()` (ExifTool accepts the XMP form);
       line breaks are kept: `-stay_open` takes one argument per line, so a line break is written
       as `\n` (and `\` as `\\`) and the plan lists ExifTool's `-ec` option (C-style escapes) in
       `WritePlan::options`; prove in the integration test that a two-line Description comes back
       with its line break;
     - Lang Alt: only `x-default` (`-<et_xmp>=<text>`), the other languages are not touched (Q7);
     - list: the whole list (`-<et_xmp>=` followed by one `-<et_xmp>=<item>` per item);
     - structure or list of structures: ExifTool's structure syntax
       (`{Field=value,Field2=value}`, `[{…},{…}]`, fields by `et_tag`, Lang Alt fields with
       `-<lang>`), with ExifTool's escape character `|` before `,`, `}`, `]`, `|` and a leading
       `[`/`{` inside values;
     - cleared: the property is removed; for a Lang Alt **all** languages. Find the ExifTool
       arguments that remove every language and prove them in the integration test; if ExifTool
       cannot do it in one write, stop and report.
  4. **IIM** (JPEG, TIFF, DNG; never `Raw`):
     - written when `iim_present`, or when `settings.create_iim` (§5.2);
     - every changed property with an IIM dataset is written as `iim::predict` of its value with
       `iim_format` and the table's byte limit (lists item by item; Lang Alt by its `x-default`); a
       cut value gives `Truncated`. Date Created: 2:55 and, only when `to_iim()` gives one, 2:60;
       otherwise 2:60 is removed (Q3). Find the ExifTool value form for `IPTC:DateCreated` and
       `IPTC:TimeCreated` that stores exactly the `to_iim()` text and prove it in the test;
     - cleared → the dataset(s) are removed;
     - `-IPTC:CodedCharacterSet=UTF8` whenever IIM is written (MWG: UTF-8 with 1:90);
     - **existing IIM not declared UTF-8** (`iim_charset` other than `Utf8`) with any non-ASCII
       value in `iim_values`: every value of `iim_values` that is not changed is written again as
       UTF-8, once, with `IimConvertedToUtf8` (§5.4);
     - **checksum:** when both IIM and XMP will exist after the write, `-IPTCDigest=new`, **unless a
       conflict with rule `Open` remains** (Q1): then no digest is written and `DigestNotWritten`
       is given;
     - **Q1 conflicts:** a property with an `Open` conflict that the user did **not** change is not
       written in either form (`ConflictKept`). A property the user changed counts as decided: both
       forms are written and its conflict no longer blocks the digest. (This is the reviewer's
       reading of Q1 — "until the user decides" — **approved by the owner on 2026-11-11**.)
     - **bring both forms in line (added after the review, 2026-11-12):** §5.1 says a conflict
       marker "stays visible until the next write by RawStride brings both forms in line", and §12
       asks for "IIM = XMP after every write". So whenever IIM is written or kept (JPEG, TIFF, DNG
       with IIM after the write) and the plan is not empty, these properties that the user did
       **not** change are also written:
       - a conflict with rule `Iim` (differing checksum, the IIM value won): the stored value is
         written to **XMP** (as point 3); IIM already holds it;
       - a conflict with rule `Xmp` (matching checksum, IIM differs): the stored XMP value is
         written to **IIM** (as above, predicted and cut);
       - a stored value with origin `Iim` and no conflict (XMP lacked it): written to **XMP**.
       They no longer block the digest. Conflicts with rule `Open` stay untouched (Q1), deprecated
       IIM properties (§5.5) and conflicts with rule `Exif` are left out, and a `Raw` gets nothing of
       this. Without this the new digest would make XMP win at the next read and silently undo an
       IIM edit made by another program.
  5. **Exif** (JPEG, TIFF, DNG; never `Raw`), only for the fields that **exist** (`ExifPresent`),
     never created (§6.3): Description → `IFD0:ImageDescription` (shown `x-default`), Copyright Notice
     → `IFD0:Copyright`, Creator → the **first** creator to `IFD0:Artist` and
     `ExifIFD:Photographer`; cleared → the existing field is removed (MWG: the other form must be
     removed). DateTimeOriginal, SubSecTimeOriginal, OffsetTimeOriginal, ImageUniqueID and GPS are
     never in the plan, whatever changed.
  6. **`Raw`:** XMP only (the plan is applied to the sidecar by I8). A cleared property is just
     removed from the sidecar; recording it in `Cleared` waits for D1.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:**
  - unit tests in `write_plan.rs` with hand-built `Metadata`/`FileState`: the exact argument lists
    for a text, a Lang Alt, a list, a structure with characters that need `|`, a date with and
    without zone (2:60 written or removed), a clear of each kind; IIM not written without IIM and
    without the setting, written with the setting; `Truncated` for a 40-byte City; Genre and
    Subject Code prefixes; the UTF-8 conversion of unchanged Latin-1 values; the digest with and
    without an `Open` conflict, a changed property with an `Open` conflict written in both forms;
    Exif fields only when present; nothing for DateTimeOriginal; `NotEditable` for
    `photoshop:Category`; `Raw` gives XMP arguments only;
  - `tests/write_plan.rs` (integration, **ExifTool required**, same JPEG fixture as T82): apply the
    plan with `ExifTool::execute` to a **temporary copy** (with `-overwrite_original`, test only) and
    read it back with `read_metadata`: the changed values are there; other XMP languages are kept;
    a cleared Lang Alt has no language left; IIM equals XMP and `CurrentIPTCDigest` equals
    `IPTCDigest`; with an `Open` conflict the digest is unchanged; an existing Artist is updated, a
    missing one is not created; the IIM bytes are UTF-8 with 1:90 (`-b -IPTC`); a Date Created
    without zone has no 2:60.
  - **added after the review (2026-11-12), for "bring both forms in line":** unit tests: a
    `Differs` file with a City conflict of rule `Iim` and only Rating changed gives
    `-XMP-photoshop:City=<IIM value>` and `-IPTCDigest=new`; a City conflict of rule `Xmp` gives the
    IIM City from the XMP value; an IIM-only Headline (origin `Iim`) gives the XMP Headline; an
    `Open` conflict is still untouched with `DigestNotWritten`; a deprecated property and a `Raw`
    get none of this. Integration test: IIM City changed without updating the checksum, then only
    the Rating edited and the plan applied → read back, City is the IIM value with origin
    `XmpEmbedded`, no conflict, and `CurrentIPTCDigest` equals `IPTCDigest`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
