# Milestone 8 (ADR 0007, IPTC metadata): steps

ADR 0007 was accepted by the owner on 2026-11-11 (revision 9). It has no step list of its own, so
the steps are kept here. The first four tasks (T76–T79) are plain Rust in `pw-metadata`, testable
without ExifTool; nothing in RawStride or the Tauri app uses them yet. The old API
(`write_rating`, `write_iptc`, `read_ratings`, `Iptc`, `LocalizedValue`) stays unchanged until I9.
The tasks for I5 and later are written after T76–T79 are accepted, because they build on those
types. T80–T83 (I5–I7) were written on 2026-11-11 after T76–T79 were done; I8 and later follow when
T83 is accepted.

**What the reviewer measured with ExifTool 13.59 before writing T80–T83** (2026-11-11, on small
JPEGs in a temporary folder):
- `-b -IPTC` gives the **raw IIM block** (in JSON as `base64:…`); its MD5 is
  `CurrentIPTCDigest`, and `Photoshop:IPTCDigest` is the stored checksum. Writing IIM does **not**
  update an existing `IPTCDigest`; `-IPTCDigest=new` must be given (then ExifTool warns "IPTCDigest
  is not current" when they differ). Without an existing digest none is created.
- `-j -struct -G1` gives XMP structures as JSON objects keyed by the ExifTool field names (`etTag`),
  lists as arrays, Lang Alt as one key per language (`XMP-dc:Description` is `x-default`,
  `XMP-dc:Description-sv` and so on), and keys named as `etXMP` in the table.
- XMP dates come in ExifTool's own form **even with `-n`**: `2026:11`, `2026:11:04 10:15`,
  `2026:11:04 10:15:30.5+01:00`. Truncation and zone are kept, so the form can be turned back into
  the XMP form without loss. A text such as `007` stays a JSON string; `xmp:Rating` is a JSON number.
- **Arguments on ExifTool's command line are converted to the Windows code page** (from PowerShell
  and from Git Bash alike: "Göteborg" was stored as Latin-1). `pw-metadata` sends arguments through
  `-stay_open` as UTF-8 and is not affected; **tests must use the `ExifTool` type, never a command
  line**. An argument file (`-@`) in UTF-8 also works.

**What the reviewer measured with ExifTool 13.59 before writing T84–T86** (2026-11-12, on a copy of
a Xiaomi DNG from `PW_TESTDATA`, a JPEG, a Panasonic RW2 and its sidecar, in a temporary folder):
- `-api ImageHashType=SHA256 -ImageDataHash` works for JPEG and DNG, and the DNG's hash is the same
  after a metadata write with `-o`.
- `-o <new file>` refuses a name that exists ("already exists", nothing written).
- The listing of a DNG copy written with `-o` differs only in `System` keys, `IFD0:TileOffsets`
  (its binary placeholder), `SubIFD:PreviewImageStart` and the written XMP plus
  `XMP-x:XMPToolkit`. Creating IIM in a JPEG adds `File:CurrentIPTCDigest`,
  `IPTC:EnvelopeRecordVersion`, `IPTC:ApplicationRecordVersion`, `IPTC:CodedCharacterSet`,
  `Photoshop:IPTCDigest` and the written tags.
- The Xiaomi DNG has 7 validation warnings of its own (`ExifTool:Validate` is `0 7 1` with `-n`),
  so the check is "no new warnings". In `-j -a` output only one `ExifTool:Warning` key survives,
  so the count comes from `Validate`.
- `-o x.xmp` with a RAW as the source copies the RAW's Exif into the new sidecar (`XMP-exif`,
  `XMP-tiff`, …). A new sidecar must be written without a source file: ExifTool then creates an
  XMP file holding only the given tags.

**What the reviewer measured with ExifTool 13.59 before writing T87–T91** (2026-11-12, on copies
in a temporary folder of a Ricoh GR III DNG with Photo Mechanic XMP (`TN002512.DNG`, 36 MB), a
Xiaomi DNG (`IMG_20261026_092351.dng`, 20 MB), a DxO JPEG (`TN005701_DxO.jpg`, 16 MB) and a
scanned TIFF with IIM and a matching `IPTCDigest` (`TN002683.tif`, 41 MB), all from
`PW_TESTDATA`; arguments through UTF-8 argument files):
- `-XMP-xmp:Rating=3 -XMP-xmp:Label=Red -o <copy>`: `ImageDataHash` equal for all four. The
  listing differs only in Rating, Label, `XMP-x:XMPToolkit` (JPEG and TIFF: another program's
  toolkit text replaced), pointers ending in `Offsets`/`Start` (DNG: `IFD0:StripOffsets`,
  `SubIFD:StripOffsets`, `Pentax:PreviewImageStart`, `SubIFD1:PreviewImageStart`,
  `SubIFD:PreviewImageStart`; TIFF: `IFD0:StripOffsets`, `SubIFD:StripOffsets`) and, for the TIFF,
  `Validate` going from 6 to 5 warnings. All of it is allowed by T84's `verify_copy`.
- Clearing both again (`-XMP-xmp:Rating= -XMP-xmp:Label=`) gives back the original listing
  exactly, pointers included. A JPEG without XMP gets only `XMPToolkit` and the Rating; clearing
  the Rating removes both again.
- The embedded previews are **byte for byte equal** after the write (both DNGs' `PreviewImage`,
  3 032 721 and 8 546 141 bytes; the JPEG's IFD1 thumbnail); only their offsets move.
- Time per ExifTool call (a new process each time; its start, ~270 ms, subtracted): a T84 snapshot
  230–400 ms, the `-o` write 310–430 ms; today's in-place write 340–500 ms. With `read_metadata`,
  two snapshots and the final read, a rating on a DNG will take about 1.5–2 s instead of about
  0.5 s, plus LibRaw (~0.3 s for 61 MP, T90). The worker queues ratings, so the user does not
  wait, but a batch of 100 DNGs takes a few minutes.
- A structure written whole (`-XMP-iptcCore:CreatorContactInfo={CiEmailWork=…,CiTelWork=…,
  CiUrlWork=…}`) changes only that key; `-XMP-dc:Subject` given four times replaces the list and
  leaves `lr:HierarchicalSubject` as it was.
- `xmpRights:Marked`, which today's template writes as `True` with a copyright, is **not in the
  property table**, so the plan cannot write it (D6). The TIFF and the Ricoh DNG already carry it
  from Lightroom and Photo Mechanic.
- **Windows** (PowerShell, `[IO.File]::Replace`, which calls `ReplaceFileW`): while another handle
  has the original open for reading with the share mode Read or ReadWrite, the replace fails ("används
  i en annan process") and the original is unchanged; with ReadWrite + Delete it succeeds. Opening
  with share mode 0 (T85's `is_locked`) fails in all three cases. So **any** open handle makes
  `can_write` refuse the file as `Locked`. A LibRaw session keeps its file open until it is closed
  (`pw-libraw/csrc/pwlr.cpp`), so the view core must close its sessions first (T89).

**What the reviewer measured with ExifTool 13.59 for T92** (2026-11-12, after the review of T87;
copies of `TN002683.tif`, the Ricoh and the Xiaomi DNG):
- In a **TIFF or DNG**, `-IPTCDigest=new` given in the same call as an IIM write stores the MD5 of
  the new IIM block without the NUL padding ExifTool adds in the TIFF structure: the 292-byte block
  of `TN002683.tif` after a Rating write with `-IPTC:CodedCharacterSet=UTF8` has
  `CurrentIPTCDigest` `2c586d07…`, while the stored `IPTCDigest` `f379e478…` is the MD5 of the
  block less its last 3 bytes; `Validate` then has the warning `IPTCDigest is not current`. The
  same when IIM is created in a DNG or a TIFF without IIM. A JPEG is not affected (stored and
  current equal).
- A second call with only `-IPTCDigest=new -overwrite_original` on the written copy stores the
  current checksum; `ImageDataHash` unchanged (TIFF and Ricoh DNG).
- When the file had no Photoshop block, creating it adds the warning `Non-standard format (undef)
  for IFD0 0x8649 PhotoshopSettings` (Xiaomi DNG `0 7 1` → `0 8 1`, Ricoh DNG one more as well)
  even after the second call. Its listing changes only in keys T84 already allows
  (`File:CurrentIPTCDigest`, the IIM tags and record versions, `Photoshop:IPTCDigest`, the
  written XMP, `SubIFD:PreviewImageStart`).

**What the reviewer measured with ExifTool 13.59 for T93** (2026-11-12, after the review of T88;
a JPEG given City in IIM and XMP and a checksum, so `Validate` is `OK`): clearing City with
`-XMP-photoshop:City= -IPTC:CodedCharacterSet=UTF8 -IPTC:City=` removes the whole XMP packet and
leaves the stored `IPTCDigest` out of date (`Validate`: `IPTCDigest is not current`); with
`-IPTCDigest=new` added the stored and the current checksum are equal and `Validate` is `OK`
(`-IPTCDigest=` removes the checksum, also `OK`).

| Step | Task | Content | ADR 0007 | Can start |
|---|---|---|---|---|
| P1 | owner/reviewer | Save the IPTC TechReference 2025.1 JSON (`iptc-pmd-techreference_2025.1.json`, link in `docs/references.md`) in `D:\PhotoWorkflowData\references\`, with bytes and SHA-256 in its `README.md`. **Done 2026-11-11:** 140 839 bytes, SHA-256 `592c37b0…433128a8` | §3 | done |
| I1 | T76 | The property table, generated from the TechReference | §3 | now |
| I2 | T77 | Dates and times: truncated dates, zones never invented, IIM and Exif forms | §7, §5.3 | now |
| I3 | T78 | The value model: values, origins, Lang Alt rules, changes | §3, §8 | after T76 and T77 |
| I4 | T79 | IIM text rules: blank values, byte limits, prefixes, character encodings | §5.1–§5.4, §6.2 | now |
| I5a | T80 | IIM block: the datasets of the raw IIM block, decoded and turned into values | §5.1, §5.3, §5.4 | now |
| I5b | T81 | Reconciliation: IIM/XMP by checksum, Exif/XMP, Creator ↔ Artist/Photographer, `Cleared` as an input | §5.1, §6.2, §6.4, §6.5 | after T80 |
| D1 | owner | The XMP namespace (prefix and URI) of `Cleared` (§6.4: "fixed with the implementation"). Reading and writing `Cleared` gets its own small task after D1 | §6.4 | – |
| D2 | owner | `serde_json` (=1.0.154, already in `Cargo.lock` through the Tauri app, so nothing new is compiled) as a dependency of `pw-metadata`, to read ExifTool's `-j -struct` output. **Approved by the owner on 2026-11-11** | – | done |
| I6 | T82 | Reader with ExifTool for every format; sidecar rules and notices (foreign sidecar next to DNG, `<name>.<ext>.xmp`) | §4, §4.2 | after T81 |
| I7 | T83 | Write plan: XMP, IIM if present (or with the setting), existing Exif overlap fields, clearing, unresolved conflicts without checksum; checked against ExifTool | §5.2, §6.3, §8 | after T82 |
| I8a | T84 | The file snapshot (one ExifTool call: listing, `ImageDataHash`, `Validate`) and the verification of a written copy | §9.1, §9.3 | now |
| D3 | owner | `windows-sys` (=0.61.2, already in `Cargo.lock` through `pw-loupe`, so no new package) as a Windows-only dependency of `pw-metadata`, features `Win32_Foundation` and `Win32_Storage_FileSystem`, for `ReplaceFileW`, `MoveFileExW`, free space and read-only volumes. **Approved by the owner on 2026-11-12** | §9 | done |
| I8b | T85 | Windows file operations (`ReplaceFileW` with the rename fallback, free space, read-only volume, locks) and the write journal with recovery | §9.1, §9.4, §9.5 | now |
| I8c | T86 | The safe write transaction for embedded metadata and sidecars, with an `ImageCheck` the caller gives | §9 | after T84 and T85 |
| I9a | T87 | Ratings and labels through the write plan and the safe write (`write_rating_safely`); the reader also gives the plan's `FileState` | §8, §9, §15 | now |
| D4 | owner | A new workspace crate **`pw-imagecheck`** for the real `ImageCheck`, depending on `pw-metadata`, `pw-raw` and `pw-libraw` (all workspace crates: no new package from crates.io; `Cargo.lock` gains only the crate itself). Root `Cargo.toml`: `pw-imagecheck = { path = "crates/pw-imagecheck" }` under `[workspace.dependencies]`; `pw-loupe`: `pw-imagecheck.workspace = true` (T91). Why not elsewhere: `pw-metadata` must not depend on `pw-raw`/LibRaw (§9.3); in `pw-viewcore` the view core would depend on `pw-metadata`; in `pw-loupe` it would need `pw-libraw` as a direct dependency (also a `Cargo.toml` change) and the Tauri app could not use it. **Approved by the owner on 2026-11-12** | §9.3 | done |
| D5 | owner | The separator of `Append` and `Prefix` for a single text or Lang Alt (§10 says "a separator" without naming it). Reviewer's proposal: one space for every property. **Approved by the owner on 2026-11-12**: one space | §10 | done |
| D6 | owner | `xmpRights:Marked` ("Copyrighted" in Lightroom and Photo Mechanic): today's template writes `True` when a copyright is given, but the property is not in the table, so the plan cannot write it. Reviewer's proposal: add it to the table as a non-IPTC entry (as `xmp:Label`) and let `Iptc::to_template` set `True` with a Copyright Notice, as today. The alternative is to stop writing it. **Approved by the owner on 2026-11-12**: added to the table, `True` with a Copyright Notice | §10, §15 | done |
| D7 | owner | The **Tauri app** (`src-tauri`) also writes ratings and the ingest template. Reviewer's proposal: it keeps today's in-place write (`write_rating`, `write_iptc`) until it is retired (GR8), as RawStride is the program in use; moving it as well needs a release of `pw-rawview`'s LibRaw sessions and its own task. **Approved by the owner on 2026-11-12**: the Tauri app keeps today's write | §9, §15 | done |
| I9b | T88 | The ingest template with the modes Keep, Replace, Append, Prefix and Clear (`template::apply`, `apply_template_safely`, `Iptc::to_template`); lists of structures, variables and template files later | §8, §10, §15 | after T87 |
| I9c | T89 | The view core closes the RAW sessions of a file before it is written (`Releaser::release`) | §9.1 | now |
| I9d | T90 | The real `ImageCheck` in `pw-imagecheck`: previews byte for byte with `pw-raw`, the DNG opened with LibRaw (§9.3 points 3–4) | §9.1, §9.3 | now (D4 approved) |
| I9e | T91 | RawStride: the metadata worker and the ingest write through T87/T88 with the journal (recovery at start) and T90's check; Swedish texts. The choice of mode in the import dialog comes with I10 | §9, §9.5, §15 | after T87–T90 |
| D8 | owner | When the safe write creates the Photoshop block of a TIFF or DNG (to store a new `IPTCDigest`), ExifTool adds the validation warning `Non-standard format (undef) for IFD0 0x8649 PhotoshopSettings`, and T84's check refuses the copy for having one more warning. Reviewer's proposal: allow exactly one more warning (not error) in that case: the copy has `Photoshop:IPTCDigest`, the original has not, the file is a TIFF or DNG and the plan writes `-IPTCDigest=new`. The alternative, not writing a checksum in a TIFF/DNG without one, breaks §5.2. **Approved by the owner on 2026-11-12** | §5.2, §9.3 | done |
| I9f | T92 | A current IIM checksum in TIFF and DNG: a second ExifTool call on the copy writes `IPTCDigest`, and the D8 allowance in `verify_copy` | §5.2, §9.3, §12 | now |
| I9g | T93 | The IIM checksum stays current when a write changes IIM and no XMP is left (a file with a stored checksum) | §5.2, §12 | now |
| D9 | owner | What RawStride does when ExifTool is missing or older than 12.67 (§11 only says "checked at start"). Reviewer's proposal: RawStride starts anyway and shows one message at start; reading works as today; every write (ratings, the editor, the ingest template) is refused with a clear text until ExifTool is updated; a version that cannot be read counts as too old. The alternative is a warning only, with writes allowed. **Approved by the owner on 2026-11-12** | §11, §9 | done |
| D10 | owner | Where and how RawStride's settings are stored (it has no settings file yet). Reviewer's proposal: a text file `%LOCALAPPDATA%\RawStride\settings.txt` (the folder of the write journal), one `key = value` per line, no new dependency; for this user on this computer only. **Approved by the owner on 2026-11-12** | §5.2, §5.4 | done |
| D11 | owner | The Swedish names of the IPTC properties in the info panel and the editor. Reviewer's proposal: the import dialog's names where they exist, and for the rest: Titel (`dc:title`), Rubrik (`photoshop:Headline`), Beskrivning (`dc:description`), Alternativtext (`Iptc4xmpCore:AltTextAccessibility`), Nyckelord (`dc:subject`), Fotograf (`dc:creator`), Fotografens titel (`photoshop:AuthorsPosition`), Fotografens kontaktuppgifter (`Iptc4xmpCore:CreatorContactInfo`, its fields E-post, Telefon, Webb), Copyright (`dc:rights`), Credit (`photoshop:Credit`), Källa (`photoshop:Source`), Användarvillkor (`xmpRights:UsageTerms`), Webbadress för rättigheter (`xmpRights:WebStatement`), Skapad (`photoshop:DateCreated`), Plats (`Iptc4xmpCore:Location`), Stad (`photoshop:City`), Län/delstat (`photoshop:State`), Land (`photoshop:Country`), Landskod (`Iptc4xmpCore:CountryCode`), Instruktioner (`photoshop:Instructions`), Jobb-id (`photoshop:TransmissionReference`), Beskrivningens författare (`photoshop:CaptionWriter`), Betyg (`xmp:Rating`), Etikett (`xmp:Label`), Upphovsrättsskyddad (`xmpRights:Marked`), Plats där bilden togs (`Iptc4xmpExt:LocationCreated`), Plats som visas (`Iptc4xmpExt:LocationShown`), Bildskapare (`plus:ImageCreator`), Upphovsrättsinnehavare (`plus:CopyrightOwner`), Licensgivare (`plus:Licensor`), Person i bilden (`Iptc4xmpExt:PersonInImage`), Händelse (`Iptc4xmpExt:Event`), Digital källtyp (`Iptc4xmpExt:DigitalSourceType`), Prioritet, Kategori, Tilläggskategorier (the deprecated `photoshop:Urgency`, `photoshop:Category`, `photoshop:SupplementalCategories`); every other property with IPTC's English name. **Approved by the owner on 2026-11-12** | §3 | done |
| D12 | owner | What the editor edits in I10. Reviewer's proposal: **one image at a time** (several images later, as a template on the selection); these 22 fields in this order: Titel, Rubrik, Beskrivning, Alternativtext, Nyckelord, Fotograf, Fotografens titel, E-post, Telefon, Webb, Copyright, Credit, Källa, Användarvillkor, Skapad, Plats, Stad, Län/delstat, Land, Landskod, Instruktioner, Jobb-id; plus a choice for every unresolved conflict. Later: lists of structures (Location Created, Image Creator, …), PLUS fields (I11), the other properties. **Approved by the owner on 2026-11-12** | §8, §10, §11 | done |
| D13 | owner | The modes in the import dialog (§10). Reviewer's proposal: the Swedish names Behåll, Ersätt, Lägg till efter, Lägg till före, Rensa; the default Ersätt for every field (an empty field then does nothing, exactly as today); Fotograf offers Behåll, Ersätt, Lägg till efter; Copyright and Credit also Lägg till före; E-post, Telefon and Webb all five (no Rensa of copyright-management fields, §8; no "before" on a list). The modes are remembered during the session like the texts, not between runs. **Approved by the owner on 2026-11-12** | §8, §10 | done |
| D14 | owner | The confirmation before a Clear (§10: "once per template"). Reviewer's proposal: when an import is started with at least one field in Rensa, one window lists those fields ("Rensa och importera" / "Avbryt"); asked once per started import, never per image. The editor does not ask (emptying a field there is the user's own choice for one image). **Approved by the owner on 2026-11-12** | §10 | done |
| D15 | owner | The place of the info panel and the keys. Reviewer's proposal: a panel on the right side, in the loupe and in the contact sheet (the selected image), shown and hidden with `M`, hidden at start; the editor opens with `E` or the button "Redigera" in the panel, in its own window. **Approved by the owner on 2026-11-12** | – | done |
| D16 | owner | One metadata window instead of the panel and the editor (replaces the panel and the keys of D15; the fields of D12 stay). Owner, 2026-11-12: like a floating info window where everything is seen and edited directly, **locked to the image until OK or Avbryt**. Reviewer's proposal for the details: `M` opens a floating, movable and resizable window "Metadata – <fil>" for the current image (one image of a kind with metadata support); `E`, the right-hand panel and the button "Redigera" go away. While it is open it is modal, like the import dialog: the loupe and the contact sheet take no keys or clicks, so the window cannot change image. Top to bottom: the 22 fields editable, "Olösta konflikter" with a choice each, "Varningar", then read-only "Övrig metadata" (every other property with its origin), the conflicts already decided by a rule, and the notices. "OK" writes the edits (nothing to write: closes) and closes after a successful write; with write notices the window stays showing "Sparat." and them until OK or Avbryt; a failed write keeps the window and the typed text. "Avbryt" closes without writing; Escape is Avbryt but asks "Släng ändringarna?" ("Släng" / "Fortsätt redigera") when there are edits. Unsaved edits are dropped at program exit (a write already running is waited for, as today). **Approved by the owner on 2026-11-12, with the details as proposed** | §5.1, §6.2 | done |
| I10a | T94 | The ExifTool version (at least 12.67) checked when the metadata worker starts; writes refused below it (D9) | §11 | now |
| I10b | T95 | The reader looks up the two sidecar names directly instead of listing the folder for every image (from T82) | §4.1, §4.2 | now |
| I10c | T96 | Validation warnings (§11): IIM byte limits, Alt Text length, Country Code form, Jobtitle without Creator, Digital Source Type, Image Registry Entry, Rating | §11 | now |
| I10d | T97 | Edits of single properties and sub-fields (`Edit`, `apply_edits`, `write_edits_safely`) with the plan's write notices; choosing a value decides an `Open` conflict | §5.1, §5.2, §8, §9 | now |
| I10e | T98 | The metadata worker reads the whole `MetadataRead`, writes edits and takes new `EditOptions` | §9 | after T94 and T97 |
| I10f | T99 | RawStride's settings file: create IIM, fallback encoding (D10) | §5.2, §5.4 | now |
| I10g | T100 | The import dialog: a mode per template field and the confirmation before a Clear (left over from T88/T91) | §10 | now |
| I10h | T101 | RawStride tells the user at start when ExifTool is missing or too old; the import worker checks it too | §11 | after T94 and T100 |
| I10i | T102 | The texts of the info panel: values with origin, conflicts with the rule, notices (D11) | §5.1, §5.4, §6.2, §6.4, §7 | now |
| I10j | T103 | The info panel in RawStride (one read in flight, after 200 ms) | §5.1, §6.2 | after T98, T101 and T102 |
| I10k | T104 | The editor's form: fields, conflict choices, edits, warnings in Swedish (D12) | §5.1, §7, §8, §11 | after T96, T97 and T102 |
| I10l | T105 | The editor window in RawStride, saved through the worker | §9 | after T103 and T104 |
| I10m | T106 | The settings window; the settings used by ratings, the editor, the panel and the import | §5.2, §5.4 | after T98, T99 and T101 |
| I10n | T107 | One metadata window locked to its image, opened with `M`: the 22 fields edited directly with OK and Avbryt, the conflicts, warnings, the other metadata and the notices (D16; replaces the panel of T103 and the window of T105) | §5.1, §6.2, §9 | now; not with T106 |
| I10o | T108 | Nyckelord and Fotograf on one line separated by commas; the reason "filen är öppen i ett annat program" when a write is refused because another program holds the file (owner's check of T107) | §5.1, §9 | now |
| I11 | – | PLUS fields as fixed choice lists | §11, Q9 | **after the PLUS controlled vocabulary is read** (§16.4) |
| I12 | – | Real-file tests with the owner's level-C samples | §12, Q11 | when the owner provides the samples |
| I13 | – | Documentation after milestone 8 | – | last |
