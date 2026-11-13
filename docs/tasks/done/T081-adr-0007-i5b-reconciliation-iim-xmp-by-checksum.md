# T81. ADR 0007 I5b: reconciliation — IIM/XMP by checksum, Exif/XMP, Creator, `Cleared` — Done

- **Can start after T80 is accepted.** Plain Rust; no ExifTool. Nothing uses the module yet.
- **Crates:** `pw-metadata` only.
- **Files:** new `crates/pw-metadata/src/reconcile.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 (IIM/XMP precedence by checksum, equality on the predicted IIM value,
  every difference is a conflict with the rule applied), §6.2 (XMP has precedence over Exif; Exif
  only fills; Exif against IIM), §6.4 (manufacturer RAW: the file's metadata only fills, `Cleared`
  suppresses it), §6.5 (Creator ↔ Artist/Photographer), §7 (Date Created is only suggested from
  DateTimeOriginal), §4.2 (embedded camera XMP in a RAW only fills).
- **What:** `pub mod reconcile` with:
  1. **Input** (all `Clone`, `Debug`):
     - `FileKind { Jpeg, Tiff, Dng, Raw }` (`Raw` = manufacturer RAW with a sidecar, §4);
     - `IimDigest { Matches, Differs, Missing }` (`IPTCDigest` against `CurrentIPTCDigest`);
     - `IimRead { values: IimValues, digest: IimDigest }` (T80);
     - `ExifFields { image_description, copyright, artist, photographer, image_unique_id,
       date_time_original, sub_sec_time_original, offset_time_original }`, each
       `Option<String>`, already decoded (T82);
     - `Readings { kind: FileKind, xmp: Vec<(String, Value)>, camera_xmp: Vec<(String, Value)>,
       iim: Option<IimRead>, exif: ExifFields, cleared: BTreeSet<String> }`. `xmp` is the primary
       XMP: embedded for JPEG/TIFF/DNG, the sidecar for `Raw`. `camera_xmp` is the XMP inside a
       `Raw` file (empty otherwise). `cleared` is the sidecar's `Cleared` list (only for `Raw`;
       empty until D1).
  2. **Output:** `Reconciled { metadata: Metadata, notices: Vec<Notice>,
     suggested_date_created: Option<IptcDate> }` and `Notice { Iim(IimNotice), Refused { property,
     origin, error: ValueError }, HiddenByUser { property, value: Value, origin: Origin },
     Unreadable { property, origin, text } }`.
  3. **`reconcile(readings: Readings) -> Reconciled`**, in this order:
     1. **Primary XMP:** each value through `Metadata::insert_read` with origin `XmpEmbedded`
        (`XmpSidecar` for `Raw`); a refused value gives `Notice::Refused` and is left out. A blank
        text (T79 `is_blank`) counts as absent.
     2. **IIM** (§5.1), for each IIM property (T80 notices are passed on as `Notice::Iim`):
        - XMP lacks the property → the IIM value is inserted with origin `Iim`;
        - XMP has it and the two are **equal** (point 4) → nothing;
        - they differ → a `Conflict` with both values (XMP first) and: `Matches` → rule `Xmp`, XMP
          kept; `Differs` → rule `Iim`, the IIM value replaces the XMP value (origin `Iim`);
          `Missing` → rule `Open`, the XMP value stays in the model until the user decides (the
          panel shows both);
        - for `Raw` the IIM in the RAW file is camera data: it only fills, and a difference is a
          conflict with rule `Xmp` whatever the digest (§4.2, §6.4).
     3. **Camera XMP** (`Raw` only, §4.2): fills a property still absent (origin `XmpEmbedded`); a
        difference to the sidecar value is a conflict with rule `Xmp`.
     4. **Exif** (§6.1, §6.2, §6.5), for the both-ways fields Description ↔ ImageDescription,
        Copyright Notice ↔ Copyright, Creator ↔ Photographer/Artist:
        - the Exif value is Photographer if present, else Artist, for Creator; ImageDescription and
          Copyright as they are; blank counts as absent;
        - property absent → filled from Exif (origin `Exif`): Description and Copyright Notice as a
          Lang Alt with one `x-default` entry; Creator as `[photographer]`, or `split_artist(artist)`;
        - property present and **different** (point 4) → a conflict. If the current value came from
          XMP: rule `Xmp`, value kept. If it came from IIM (no XMP, §6.2): `Differs` → rule `Iim`,
          kept; `Matches` or `Missing` → rule `Exif`, the Exif value replaces it (origin `Exif`).
          For `Raw` always rule `Xmp`/kept (camera data);
        - **Digital Image GUID** (`Iptc4xmpExt:DigImageGUID`; ExifTool tag `XMP-iptcExt:DigitalImageGUID`) ← ImageUniqueID only fills; a
          difference is **not** a conflict (§6.1: the IPTC value is then not changed);
        - ImageEditor ↔ Contributor and GPS → Location Created are **not** part of this task (they
          get a task later; leave a comment).
     5. **`Cleared`** (`Raw`, §6.4): a property in `cleared` that the sidecar does not hold is never
        filled from the RAW file (IIM, camera XMP or Exif). The first value that would have filled
        it gives `Notice::HiddenByUser { property, value, origin }` (shown as "dold av användaren"
        later). A sidecar value for a cleared property wins (another tool wrote it).
     6. **Date Created** (§7): if `photoshop:DateCreated` is still absent, `suggested_date_created`
        is `IptcDate::from_exif` of the three Exif fields; `Empty` gives `None`, `Invalid` gives
        `Notice::Unreadable` (origin `Exif`). **Never inserted** into the metadata.
  4. **Equality** (`pub fn same_value(xmp_id, a: &Value, b: &Value) -> bool`, also used by T83):
     - against IIM (§5.1), on the predicted IIM value: `Text` and `Number` with `iim::predict`
       (the property's `max_bytes` and `iim_format`) and `same_as_predicted`; a Lang Alt by its
       shown text (`LangAlt::display`) **through the same prediction** (corrected after the first
       review: the XMP Lang Alt's shown text is predicted and compared with the IIM Lang Alt's
       shown text, so a Title cut to 64 bytes in 2:05 is not a conflict); a `List` item by item in
       order, same length; a `Date` by `to_iim()` (both 2:55 and 2:60 equal);
     - against Exif (§6.5): Description and Copyright by the shown text; Creator: the **first**
       creator against the Exif value's first name (RawStride writes only the first creator, so
       `["A","B"]` against Artist `A` is not a conflict);
     - text equality is exact (no trimming, no case folding).
  5. **`split_artist(text: &str) -> Vec<String>`** (MWG §5.7): names separated by `"; "`; a name in
     double quotes may contain `;` and doubled quotes `""` stand for one quote. Read the rule in
     `D:\PhotoWorkflowData\references\mwg_guidance.pdf` §5.7 first; if it differs from this
     description, **stop and report**. Blank names are dropped.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `reconcile.rs`; `Readings` built by hand):
  - **the three checksum cases** for `photoshop:City` (XMP `Göteborg`, IIM `Malmö`): `Matches` →
    `Göteborg`, conflict rule `Xmp`; `Differs` → `Malmö` origin `Iim`, rule `Iim`; `Missing` →
    `Göteborg`, rule `Open`; each conflict holds both values;
  - **equality on the prediction**: XMP City of 40 bytes against its IIM value cut to 32 bytes is
    not a conflict; IIM `000:Feature` (already stripped by T80) against XMP Genre `Feature` is
    not; a Description Lang Alt `[x-default "A", sv "B"]` against IIM `A` is not; keywords in
    another order are a conflict; a zone-less XMP date against IIM 2:55 only is not; XMP with a
    known zone against IIM without 2:60 is;
  - IIM only fills when XMP lacks the property (origin `Iim`, no conflict); a blank XMP text counts
    as absent;
  - **Exif**: Exif fills Description, Copyright and Creator when XMP and IIM lack them; Photographer
    wins over Artist; `split_artist("A; B")` → `["A","B"]`, `"\"Smith; J\"; B"` →
    `["Smith; J","B"]`, `"\"Say \"\"hi\"\"\"; B"` → `["Say \"hi\"","B"]`; XMP `["A","B"]` with
    Artist `A` is not a conflict, with Artist `C` it is (rule `Xmp`); IIM-only Creator against
    Artist with `Matches` → Exif wins (rule `Exif`), with `Differs` → IIM kept (rule `Iim`);
  - GUID fills from ImageUniqueID, and a different XMP GUID is kept without a conflict;
  - **RAW**: sidecar wins over camera XMP (conflict `Xmp`); camera XMP fills; RAW IIM with `Differs`
    only fills and gives rule `Xmp`; a cleared property is not filled from camera XMP, IIM or Exif
    and gives one `HiddenByUser` with the camera value; a sidecar value for a cleared property is
    used;
  - Date Created: absent → suggestion from Exif (zone only with an offset) and not in the metadata;
    present → no suggestion; an invalid DateTimeOriginal → `Unreadable`;
  - a refused XMP value (e.g. `Text` for `dc:subject`) gives `Refused` and the IIM value fills;
  - T80 notices arrive as `Notice::Iim`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
