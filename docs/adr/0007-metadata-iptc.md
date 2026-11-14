# ADR 0007: Metadata handling according to the IPTC standards

- Status: **accepted by the owner (2026-11-11, revision 9).** All open questions are answered
  (§14); what is left before the implementation is in §16.
  - The owner's decisions of 2026-11-04 are written in.
  - Planned as milestone 8 of the egui program after its approval (owner, 2026-11-10; ADR 0015 §2;
    moved from milestone 7 by ADR 0016 Q1, 2026-11-11).
  - Q4 was answered by the owner on 2026-11-08: DNG Exif overlap fields are synchronised like TIFF.
  - Q5 was answered by the owner on 2026-11-08: foreign XMP sidecars next to DNG are ignored and reported.
  - Q6 was answered by the owner on 2026-11-08: missing Exif overlap fields are not created.
  - Q7 was answered by the owner on 2026-11-08: edit `x-default` only and preserve all other language entries.
  - Revision 7 (2026-11-08) makes the text consistent with the Q4–Q7 answers; it adds no new decision.
  - Q1–Q3 and Q8–Q13 were answered by the owner on 2026-11-11 (revision 8). Q2 was decided after
    a comparison with Lightroom and Photo Mechanic (§14).
  - Revision 8 also uses the product name **RawStride** (ADR 0012). This ADR was still a proposal
    when the name changed, so ADR 0012 §5 ("earlier ADRs keep the old name") does not apply to it.
    Paths, crate names and technical file suffixes keep their old names (ADR 0012 §4).
  - The owner approved the ADR on 2026-11-11 together with the answers (revision 9).
- Date: 2026-11-04 (revision 1: storage, IIM policy and the role of XMP; revision 2: all owner
  decisions of 2026-11-04, MWG 2.0, the full IPTC specification and the Mapping Guidelines;
  revision 3: Q4 answered by the owner on 2026-11-08; revision 4: Q5 answered by the owner on
  2026-11-08; revision 5: Q6 answered by the owner on 2026-11-08; revision 6: Q7 answered by the
  owner on 2026-11-08; revision 7: consistency fixes after review, 2026-11-08; revision 8: Q1–Q3
  and Q8–Q13 answered by the owner and the product name RawStride, 2026-11-11; revision 9: approved
  by the owner, 2026-11-11).
- Basis: `docs/analyses/2026-11-04-iptc-metadata-requirements.md` (§1–§10) and
  `docs/references.md`. Local copies of the sources are in `D:\PhotoWorkflowData\references\`
  (see its `README.md`).
- Related: ADR 0004 (ratings and labels), ADR 0005 (ingest, IPTC template).

## 1. Context

The owner made the IPTC Photo Metadata Standard **2025.1** (IPTC Core 1.5, IPTC Extension 1.9)
the **normative reference**. Two documents support it:

- the **IPTC Photo Metadata Mapping Guidelines 2023.1** (IPTC ↔ Exif 2.3x/3.0 ↔ Schema.org;
  "not mandatory … highly suggested");
- the **MWG Guidelines for Handling Image Metadata 2.0** (2010; no longer published, but cited by
  the IPTC standard §5.3.4).

RawStride follows these documents, not Photo Mechanic's internals. It stays compatible with
Lightroom and Photo Mechanic through the standards and the tools' public documentation.

### 1.1 How rules are labelled

Every rule in §3–§12 carries its basis:

| Label | Meaning |
|---|---|
| **[IPTC]** | IPTC Photo Metadata Standard 2025.1 (section given) |
| **[IPTC-MG]** | IPTC Photo Metadata Mapping Guidelines 2023.1 |
| **[MWG]** | MWG Guidelines 2.0 (section given) |
| **[Product]** | RawStride's own decision (owner's decision of 2026-11-04 unless said otherwise) |
| **[Product ≠ std]** | A product decision that deliberately departs from a recommendation of a standard. The reason is given |

## 2. Decision summary

| # | Decision | Basis |
|---|---|---|
| 1 | **XMP is RawStride's primary representation** for the explicitly mapped descriptive fields | [Product]; IPTC Core/Extension are defined in XMP [IPTC §1] |
| 2 | **IIM vs XMP:**<br>- checksum matches → XMP;<br>- checksum differs → IIM per differing field;<br>- no checksum → conflict, the user decides;<br>- conflicts always visible | [MWG §4.2.3.2] for the first two; [Product ≠ std] for the third |
| 3 | **IIM is preserved and written back when it exists.** New IIM only with the setting | [MWG §4.2.3.2]; [Product ≠ std] against [IPTC §5.3.4] "should save … both" |
| 4 | **Exif vs XMP:**<br>- XMP wins;<br>- conflicts detected and shown;<br>- JPEG/TIFF/DNG synchronise the corresponding existing Exif fields;<br>- RAW originals are never changed (Exif conflicts there are only shown) | [Product ≠ std] against [MWG §4.2.3.1] "prefer Exif"; sync per [MWG §4.2.3.1] and [IPTC-MG] |
| 5 | **Storage (fixed):**<br>- **DNG: embedded, never a sidecar** (also Xiaomi 17 Ultra DNG);<br>- **ARW, RW2 and other manufacturer RAW: sidecar only, original never changed**;<br>- **JPEG/TIFF: embedded** | [Product] (architecture decision) |
| 6 | **Embedded writes are transactional:** a verified temporary copy, then an atomic replace; never a partially written original | [Product] |
| 7 | **DateTimeOriginal is never changed automatically. A time zone is never invented**; it is stored only when known | [MWG §4.2.3.4 "Time-zone handling"]; [Product] |
| 8 | **Preserved untouched:**<br>- hierarchical keywords (`mwg-kw`, `lr:hierarchicalSubject`);<br>- deprecated IIM datasets (kept in sync if present);<br>- unknown namespaces;<br>- other Lang Alt languages (except when the whole field is cleared, §8) | [MWG §5.10]; [IPTC §9.1]; [Product] |
| 9 | **Rating:** decimal precision kept internally; rounding only for display | [MWG §5.5]; [IPTC §11.19] |

## 3. Data model

- **One internal metadata model keyed by the IPTC property id**, using its XMP name
  (`dc:creator`, `photoshop:City`, `Iptc4xmpExt:PersonInImage`, …). [Product]
- **Value types follow the spec tables** [IPTC §7, §8, §11, §12]:
  - Text; Lang Alt; Seq (`dc:creator`); Bag (Keywords, Scene Code, Subject Code and most
    Extension lists);
  - Date (truncated forms allowed);
  - Closed choice;
  - structures.
- **IIM datasets and Exif tags are only representations.** Their mapping is a table generated
  from the IPTC TechReference 2025.1 and checked into the repo, so a new IPTC version is a table
  update. [Product]
- **First scope:** all IPTC Core properties, plus the Extension properties in the analysis §3.
  [Product]
- **`xmp:Rating`** (IPTC "Image Rating") and **`xmp:Label`** (not IPTC; kept for Adobe/Photo
  Mechanic compatibility) move into the same model. [Product]
- **Each property value carries its origin** (XMP embedded, XMP sidecar, IIM, Exif), so the
  conflicts (§5, §6) can be shown. [Product]

## 4. Storage per file format (fixed architecture decision)

| File | XMP (primary) | IIM | Exif overlap fields (§6) | Original modified? |
|---|---|---|---|---|
| **ARW, RW2, NEF, CR2, CR3, ORF, RAF and other manufacturer RAW** | **only** the sidecar next to the file (§4.2) | – (IIM has no sidecar form) | embedded Exif is **read-only**; differences are shown as conflicts | **never** [Product] |
| **DNG** (all producers, **including the Xiaomi 17 Ultra**) | **embedded in the DNG; no sidecar is ever written or used**; a foreign sidecar is ignored and reported | embedded; preserved and synced if present, created only with the setting (§5.2) | synced if present, like TIFF | yes: metadata only, by the safe write procedure (§9) [Product] |
| **JPEG** | embedded (APP1) | APP13 PSIR 1028; as DNG | synced (§6.3) | yes: metadata only, §9 |
| **TIFF** | embedded (tag 700) | tag 33723, checksum in PSIR 1061 in tag 34377; as DNG | synced (§6.3) | yes: metadata only, §9 |

Where in the file to read and write [MWG §4.2.3.4]:

- JPEG: XMP in the APP1 XMP segment, IIM in APP13 PSIR 1028, Exif in the APP1 Exif segment, which
  must directly follow SOI. "All other occurrences SHOULD be ignored".
- TIFF/DNG: IFD0 tags 34665 (Exif), 33723 (IIM) and 700 (XMP); the IIM checksum in PSIR 1061 in tag 34377.
  MWG does not cover DNG. As a TIFF-based format it is handled like TIFF.

### 4.1 Rationale

- **RAW sidecar:**
  - The owner's requirement that originals are never touched.
  - Lightroom: "for camera raw files that have a proprietary file format, XMP isn't written into
    the original files".
  - Photo Mechanic writes sidecars for RAW by default.
- **DNG embedded:**
  - DNG is an open, documented TIFF-based format with a defined place for XMP.
  - Lightroom writes XMP into DNG, and Photo Mechanic treats DNG as "embedded only".
  - A sidecar next to a DNG would be ignored by those tools and cause the metadata to diverge.
- A foreign `<basename>.xmp` next to a DNG is never read or written; the info panel reports that it
  exists and is ignored.
- **The Xiaomi 17 Ultra DNG is named explicitly.** Phone DNGs differ in structure from camera
  DNGs; for example, the test files keep their preview at the end of the file. The safe write
  procedure (§9) verifies such files individually.

### 4.2 Sidecar rules (manufacturer RAW only)

- **Name:** `<basename>.xmp` (e.g. `DSC01234.xmp`), the same as ADR 0004 and the default of
  Lightroom and Photo Mechanic. **(Q8, owner 2026-11-11)**
  - A sidecar named `<basename>.<ext>.xmp` (e.g. `DSC01234.ARW.xmp`, written by darktable and
    similar tools) is never read or written; the info panel reports that it exists and is ignored,
    as for a foreign sidecar next to a DNG (Q5).
  - To confirm with the level-C samples (Q11; Photo Mechanic and the mixed archive files, no file
    written by Lightroom alone, §12); Adobe XMP Part 3 is read if available (§16).
  - **Confirmed for Photo Mechanic (T127, 2026-11-14):** Photo Mechanic 2026.6 names the sidecar
    `<basename>.XMP` with an upper-case extension (`PM-raw.XMP`); RawStride finds it as
    `<basename>.xmp` and reports no foreign sidecar. After a write by RawStride the file is named
    `PM-raw.xmp` (lower case): one file, but its name changed (an open finding of the review).
    Lightroom alone stays unconfirmed.
- **Created** on the first change; **edited in place** afterwards (unknown content kept), with the
  same transaction (§9.5).
- **RAW+JPEG pairs:** only the RAW reads and writes `<basename>.xmp`; the JPEG keeps its embedded
  metadata.
- **Embedded metadata in the RAW** (Exif, XMP written by the camera) is read-only. It only fills
  properties the sidecar lacks, and differences are shown as conflicts (§6.4). A camera
  `xmp:Rating` of 0 counts as no rating: it neither fills nor gives a conflict (owner, 2026-11-13;
  Sony writes 0 for an unrated image).

## 5. IIM and XMP

### 5.1 Reading: precedence

When IIM and XMP are both present, the IIM checksum decides. The checksum is an MD5 of the whole
IIM block in PSIR 1061 [MWG §4.2.3.2].

| Situation | Rule | Basis |
|---|---|---|
| **Checksum present and matches** | **XMP has precedence**; IIM only fills a property XMP lacks | [MWG §4.2.3.2 scenario 3] |
| **Checksum present and differs** (a tool unaware of XMP changed the IIM) | **Per property:** the IIM value RawStride would write from the XMP is predicted (truncation to the byte limit, UTF-8, IIM formats §5.3). If the prediction equals the actual IIM value, XMP is used. **If it differs, the IIM value has precedence** for that property | [MWG §4.2.3.2 scenario 4] |
| **No checksum** (IIM and XMP both present, values differ) | **Neither is assumed newer.** The property is registered as a **conflict** and the user decides. Until then the info panel shows both values | **[Product ≠ std]:** MWG says to prefer XMP here ("checksum … does not exist … XMP … SHOULD be preferred"). Reason: without a checksum nothing shows which tool wrote last, and preferring XMP could hide an edit made by an older tool |
| Only IIM, or only XMP | That one is used | [MWG §4.2.3.2 scenarios 1–2] |

- **Equality is always tested on the predicted IIM value.** A value that only differs because of
  IIM truncation or encoding is not a conflict. [MWG §4.2.3.2]
- **An IIM value equal to the whole XMP text is not a conflict either** (owner, 2026-11-14, T129):
  some programs ignore the IIM byte limit (Photo Mechanic wrote a 2191-byte Description to IIM
  2:120, level-C sample, T127). Such a value is not rewritten unless the property is edited; then
  it is cut to the limit as §5.2 says. [Product]
- **Conflict marker** [Product]:
  - **Every** difference between IIM and XMP is recorded as a conflict with both values and the
    rule that was applied: XMP (matching checksum), IIM (differing checksum), or open (no
    checksum).
  - The marker stays visible until the next write by RawStride brings both forms in line, so
    metadata changes made outside RawStride become visible.
- **A string of only spaces or only NUL characters is treated as non-existent.** [MWG §4.2.3.3]

### 5.2 Writing

- **IIM present in the file:**
  - every changed property that has an IIM dataset is written to both XMP and IIM;
  - the IIM is rewritten in UTF-8;
  - the checksum is updated.
  [MWG §4.2.3.2: "If IPTC-IIM is already in the file, a Changer SHOULD write data back … in both"]
- **IIM not in the file:** XMP only. IIM is created only when the setting **"Skapa IPTC-IIM för
  kompatibilitet med äldre system"** is on (off by default; JPEG, TIFF, DNG).
  - **[MWG §4.2.3.2]:** Creator "SHOULD NOT create IPTC-IIM unless backward compatibility … is a
    requirement".
  - **[Product ≠ std]** against [IPTC §5.3.4], which says values "should be saved … using both the
    IIM format and the XMP format". Reason: the owner's decision and MWG; IIM has length and
    encoding limits, and current tools read XMP. The setting covers workflows that need IIM.
  - "Backward compatibility requires it" is decided **only by this setting** (and possibly a
    future export profile); there is no automatic detection. **(Q13, owner 2026-11-11)**
- **Checksum:** "If IPTC-IIM and XMP are both present, whether changed or not, a Changer MUST
  create or update the checksum" [MWG §4.2.3.2]. It is computed over the whole IIM block after
  conversion.
  - **TIFF and DNG:** the checksum is written by a second ExifTool call on the temporary copy
    (§9.2), because ExifTool 13.59 stores an out-of-date checksum there when it is written in the
    same call as the IIM (T92).
- **Both forms in line after every write** **(owner 2026-11-12, review of T83)**: §5.1 keeps a
  conflict marker until a write brings both forms in line, and §12 asks for IIM = XMP after every
  write. So when a JPEG, TIFF or DNG has IIM after the write and the write changes anything, these
  properties the user did **not** change are written too:
  - a conflict decided for IIM (differing checksum), and a value found only in IIM: the IIM value
    is written to XMP;
  - a conflict decided for XMP (matching checksum): the XMP value is written to IIM (predicted and
    cut as for any IIM write).

  Unresolved conflicts (below), conflicts with Exif and deprecated IIM datasets (§5.5) are left
  out. Reason: otherwise the new checksum would make XMP win at the next read and silently undo an
  IIM edit made by another program.
- **Unresolved conflicts when writing** (the no-checksum case of §5.1) **(Q1, owner 2026-11-11):**
  - writing other properties is allowed;
  - for each property with an unresolved conflict, both forms keep their values (the IIM text may
    be re-encoded, see below);
  - **no new checksum is written** while any conflict in the file is unresolved, so the write does
    not silently decide the conflict;
  - batch operations list such files in their summary.
  - **[Product ≠ std]:** MWG §4.2.3.2 says a Changer MUST create or update the checksum "whether
    changed or not". This is a deliberate exception, limited to files with an unresolved conflict
    (§13).
  - **An edit decides the conflict** **(owner 2026-11-11, T83)**: when the user changes a property
    that has an unresolved conflict, the change is the user's decision. The new value is written
    to both forms, and that conflict no longer keeps the checksum from being written. Only the
    conflicts the user has not touched do: the file then stays without a checksum, and those
    conflicts are shown again at the next read.
  - **Re-encoding is not a decision** **(owner 2026-11-12, T83)**: when the IIM is converted to
    UTF-8 (§5.4), the IIM text of an unresolved conflict is written again unchanged, only
    re-encoded, so no Latin-1 bytes are left under the UTF-8 marker (1:90). The conflict stays
    unresolved and no checksum is written.
- **Values longer than the IIM limit** are kept in full in XMP and cut at a UTF-8 character
  boundary in IIM. The user is told. The limit counts **bytes, not characters**. [IPTC §15.1]

### 5.3 IIM value formats [IPTC]

| Property | IIM rule | Source |
|---|---|---|
| Intellectual Genre (2:04) | Prefix `000:`; the 64-byte limit applies to the code only | [IPTC §7.16] |
| Subject Code (2:12) | Prefix `IPTC:` | [IPTC §7.23] |
| Date Created (2:55) | `YYYYMMDD`, `00` for an unknown month or day (`19180000`) | [IPTC §7.10] |
| Time Created (2:60) | `hhmmss±hhmm`, no sub-seconds. **Only written when the offset is known** (§7). Otherwise only 2:55 is written and 2:60 is left out; the time stays in XMP, zone-less **(Q3, owner 2026-11-11)** | [IPTC-MG Exif Note on Date Created]; to confirm against IIM 4.2 (§16) |
| Keywords (2:25), By-line (2:80) | One repeated dataset per value; the whole list is rewritten from XMP | [MWG §5.1] |
| Contact (2:118) | As in the spec table | [IPTC §8.1] |

When reading, the prefixes are removed, and 2:55 + 2:60 are merged into the one XMP value.
[IPTC §7.10, §7.16, §7.23]

### 5.4 Character encoding

- **Writing:** IIM "MUST be written as UTF-8, and MUST include a 1:90 DataSet". [MWG text
  encodings]
- **Existing IIM in another encoding** is converted to UTF-8 on the first write. "A robust Changer
  SHOULD convert all properties to UTF-8". The user is told once. [MWG §4.2.3.2]
- **Reading:**
  - 1:90 = UTF-8 → UTF-8 (MUST).
  - Another declared encoding → that encoding.
  - No 1:90:
    1. all bytes < 128 → ASCII;
    2. otherwise valid UTF-8 → UTF-8;
    3. otherwise the fallback encoding: **Windows-1252** by default, changeable in the settings;
       the value is marked as converted. **(Q10, owner 2026-11-11)**
  - BOMs are removed.
  [MWG text encodings]

### 5.5 Deprecated IIM datasets

Urgency (2:10), Category (2:15) and Supplemental Categories (2:20):

- They are **kept**, and **synchronised** with `photoshop:Urgency`, `photoshop:Category` and
  `photoshop:SupplementalCategories` if they exist. [IPTC §9.1]
- They are not offered for editing. [Product]
- They are never created and never removed. [Product]

## 6. Exif and XMP

### 6.1 Which fields

| IPTC property | Exif tag(s) | Direction | Basis |
|---|---|---|---|
| Description (`dc:description` x-default) | ImageDescription 0x010E | both ways | [IPTC-MG]; [MWG §5.2] |
| Copyright Notice (`dc:rights` x-default) | Copyright 0x8298 | both ways | [IPTC-MG]; [MWG §5.6] |
| Creator (`dc:creator`) | Artist 0x013B; Photographer 0xA437 (Exif 3.0) | both ways (§6.5) | [IPTC-MG Exif Note on Creator] |
| Image Creator name (PLUS) | Photographer / Artist | as Creator | [IPTC-MG Exif Note on Image Creator] |
| Contributor with the role "Content Editor" | ImageEditor 0xA438 (Exif 3.0) | both ways | [IPTC-MG Exif Note on Contributor] |
| Date Created | DateTimeOriginal + SubSecTimeOriginal + OffsetTimeOriginal | **Exif → IPTC only** (§7) | [IPTC-MG]; [Product] |
| Digital Image GUID | ImageUniqueID 0xA420 | **Exif → IPTC only**; the IPTC value is then not changed | [IPTC-MG Exif Note on Digital Image GUID] |
| Location Created | GPS tags 1–6 | Exif → IPTC, only when recorded by the camera | [IPTC-MG]; [MWG §5.8.5] |
| Location Shown | — | **never mapped from GPS** | [IPTC-MG Exif Note on Location Shown] |
| Image Region | — | **never mapped from SubjectArea** | [IPTC-MG Exif Note on Image Region] |

### 6.2 Reading: precedence

- **XMP has precedence** for every field it contains. Exif fills only properties that XMP (and
  IIM, §5) lack. [Product]
  - **[Product ≠ std]:** MWG §4.2.3.1 says "In general, a Consumer SHOULD prefer the values from
    Exif".
  - Reason: XMP is RawStride's primary representation (owner's decision); Lang Alt, lists and
    structures exist only in XMP; and compliant tools keep both forms equal anyway [MWG §4.2.3.1].
- **Conflict between Exif and IIM** (both present, XMP missing): Exif if the IIM checksum matches
  or is missing, IIM if it differs. [MWG §4.2.3.3]
- **Every Exif/XMP difference is detected and registered as a conflict** (both values shown).
  [Product]
- **Text encoding of Exif when reading:** ASCII → valid UTF-8 → fallback (Windows-1252 by default,
  §5.4; Q10). Exif 3.0 tags
  of the UTF-8 type are read as UTF-8. [MWG; IPTC-MG]

### 6.3 Writing (JPEG, TIFF and DNG)

- **Overlap fields that exist are updated** whenever the corresponding IPTC property changes.
  This applies only to the fields marked "both ways" in §6.1; fields marked "Exif → IPTC only"
  (DateTimeOriginal and its sub-fields, ImageUniqueID, GPS) are never written
  [Product, as allowed by the owner]. MWG §4.2.3.1: "a Changer SHOULD update both forms of a
  value. If only one form is updated, an existing value in the other form MUST be removed".
- **Missing Exif overlap fields are not created** in JPEG, TIFF or DNG. Existing overlap fields are
  synchronised. This includes Artist and Photographer: each is written only if it already exists,
  and neither is created to complete the pair (§6.5).
- **Encoding:**
  - Files with Exif 3.0: a non-ASCII value uses the UTF-8 data type. [IPTC-MG]
  - Older Exif: UTF-8 bytes in the ASCII type, as MWG advises. Formally this violates Exif 2.3x;
    both IPTC-MG and MWG accept it.
- **The Exif byte order is kept.** Exif/TIFF device tags are not duplicated into XMP `exif:` or
  `tiff:`, and no Exif checksum is written to XMP. [MWG §4.2.3.1]
- **DateTimeOriginal is never written** (§7).

### 6.4 Manufacturer RAW (ARW, RW2, …)

- **Embedded Exif is preserved and never written:** the original is never changed.
- **When embedded Exif differs** from the sidecar's XMP (for example, Artist or Copyright set in
  the camera), the difference is **shown as a conflict**. XMP has precedence.
- **A field cleared in the sidecar stays cleared** although the camera's Exif still has a value
  **(Q2, owner 2026-11-11):**
  - the sidecar records the cleared property in the RawStride namespace (`Cleared`, a Bag of
    property ids; prefix and namespace URI are fixed with the implementation);
  - RawStride does not fill such a property from the embedded Exif; the info panel shows the
    camera value as "hidden by the user";
  - writing a value to the property again removes it from `Cleared`.
  - **Limitation:** other tools ignore the marker and show the camera value. Lightroom has the
    same problem today: it may use the camera's ImageDescription of a raw file on export after the
    caption was cleared (a known bug report since Lightroom 3). Photo Mechanic documents no
    behaviour for this case.
  - **Possible later addition:** writing an explicitly empty value in the sidecar, if it is shown
    that Lightroom and Photo Mechanic then keep the field empty. The level-C samples (Q11) can show
    it for Photo Mechanic only; there is no file written by Lightroom alone (§12, 2026-11-14), so
    for Lightroom it stays untested.
  - **What the samples showed (T127, 2026-11-14):** Photo Mechanic 2026.6 removes a top-level
    field the user emptied (no top-level XMP property with an empty value in `PM-jpg.JPG`, whose
    fields were filled, saved, emptied and saved again), but writes the empty sub-fields of a
    structure as empty strings (Location Created: Sublocation, Province/State, World Region).
    Whether Photo Mechanic keeps a field empty when the sidecar holds an explicitly empty value is
    not shown by the samples.

### 6.5 Creator ↔ Artist/Photographer

- **Writing:** the **first** Creator is copied, as one name, to whichever of Artist and (in Exif
  3.0) Photographer already exists; a missing tag is not created (§6.3). [IPTC-MG Exif Note on
  Creator]; [Product] for not creating
  - **[IPTC-MG] over [MWG]:** MWG §5.7 joins all creators with "; ". The newer IPTC guideline,
    written with CIPA, asks for a single name.
- **Reading:**
  - Photographer if present, else Artist. [IPTC-MG]
  - An Artist with several names (older files) is split by MWG's "; " separator and quoting rules.
    [MWG §5.7]
- **Copyright:**
  - Exif 2.3x may hold photographer and editor parts separated by NUL; they are joined with a line
    feed when read. [MWG §5.6; IPTC-MG]
  - Writing: one value.

## 7. Date, time and time zone

| Rule | Basis |
|---|---|
| **DateTimeOriginal (and SubSecTimeOriginal, OffsetTimeOriginal) is never changed automatically.** No IPTC edit, template or ingest writes it | [Product]; IPTC-MG describes a mapping in both directions but is "not mandatory" |
| **A time zone is never invented.** When the Exif date-time has no offset, the XMP value is written zone-less. No conversion to the computer's time zone, for storage or display | [MWG §4.2.3.4]: "A Consumer MUST NOT arbitrarily add a time zone"; "A Changer MUST NOT implicitly add a time zone" |
| **A time zone is stored only when known:** from OffsetTimeOriginal, or entered by the user | [Product], following MWG |
| When a zone is known, XMP uses local time + offset (`2026-11-04T10:15:00+01:00`), not UTC "Z" | [MWG §4.2.3.4] |
| **Date Created is suggested** from DateTimeOriginal + SubSecTimeOriginal + OffsetTimeOriginal, **only when Date Created is empty** (e.g. at ingest) | [IPTC §7.10 XMP note 2]; [IPTC-MG] |
| **Truncated dates** (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`, with or without a time) are allowed in the editor and in storage. IIM uses `00` for an unknown month or day | [IPTC §7.10, §14.2] |
| Without a time, the time part stays empty. A system that needs a time uses 00:00:00 but does not store it | [IPTC §7.10 note 1] |
| IIM has no sub-seconds: they are dropped for IIM and kept in XMP | [IPTC-MG Exif Note on Date Created] |
| **IIM Time Created (2:60) is left out when the zone is unknown**; only the date (2:55) is written | [Product], following MWG; to confirm against IIM 4.2 (Q3) |
| A manual "correct camera time" feature is **out of scope**. If added later, it is the only way DateTimeOriginal ever changes, and only on the user's explicit command | [Product] (Q12); MWG says such corrections SHOULD be exported to Exif |

## 8. Other rules

| Rule | Basis |
|---|---|
| **Hierarchical keywords** (`mwg-kw:Keywords`, `lr:hierarchicalSubject`) are kept **unchanged**, including all nested information, even though RawStride cannot edit them yet | [MWG §5.10]: "MUST preserve all nested information"; [Product] for `lr:` |
| **Keywords may be phrases** and are never split at spaces | [IPTC §7.18] |
| Keyword lists are rewritten as a whole in IIM; XMP qualifiers on keywords are kept | [MWG §5.1] |
| **Rating:** stored internally with full precision (a real number). Rounded only for display (out-of-range values shown as the nearest of −1…5). Written only when the user changes it. 0 = unrated; a missing value counts as 0; −1 = rejected | [IPTC §11.19]; [MWG §5.5] |
| **Lang Alt:**<br>- RawStride edits `x-default` only and preserves all other language entries, including one that held the same value as the old `x-default`;<br>- when `x-default` is absent, it is added first;<br>- **reading** without `x-default`: the first entry is shown, marked with its language;<br>- clearing removes the whole property, all languages included | [IPTC §14.1]; [Product]. **To verify:** the Adobe XMP Toolkit also updates entries equal to the old `x-default`; if MWG §5.2 or ISO 16684-1 recommend that, this becomes [Product ≠ std] (Q7, §13, §16) |
| **Unknown namespaces, list items not touched, camera maker notes** are never removed or rewritten | [Product] |
| **Creator's Contact Info** and **Creator's Jobtitle** belong to the first Creator. The contact sub-fields are single Text values that may hold several entries | [IPTC §7.7, §7.8, §8.1] |
| **Creator ↔ Image Creator name (PLUS):** synchronisation is offered | [IPTC §7.6 note 1] |
| **Cardinalities:**<br>- Image Creator, Copyright Owner, Licensor: at most 3;<br>- Image Supplier: 1;<br>- Location Created: one in the UI (all are kept when reading) | [IPTC §11.9, 11.18, 11.22, 11.24, 11.26] |
| **Clearing a field** removes it consistently from every representation RawStride may write:<br>- XMP (embedded or sidecar);<br>- IIM;<br>- the Exif overlap field in JPEG/TIFF/DNG.<br>**A manufacturer RAW file is never changed** (Q2) | [MWG §4.2.3.1]; [Product] |
| **Copyright-management fields cannot be cleared by templates** (Copyright Notice, Creator, Credit Line, Rights Usage Terms, Web Statement of Rights, Copyright Owner, Licensor, Image Creator). They can be replaced | IPTC Embedded Metadata Manifesto (User Guide); [Product] |

## 9. Safe writing of embedded metadata (DNG; also JPEG and TIFF)

**Goal:** a metadata write either fully succeeds with a verified file, or leaves the original
exactly as it was. No crash, power cut, full disk, file lock or tool error may leave a partially
written original. [Product]

### 9.1 Before writing

1. **Identify the file by content, not by extension:**
   - DNG: TIFF header with the `DNGVersion` tag (50706);
   - JPEG: SOI;
   - TIFF: header.
   A mismatch is refused.
2. **Writable and safe to write:**
   - the file is writable and not locked by another program;
   - the volume has free space ≥ file size + 10 %;
   - the target is not a read-only or write-protected card;
   - the file is not open in RawStride's own RAW engine (sessions for it are closed first).
3. **Record the original state:**
   - size, modification and creation times, attributes;
   - **`ImageDataHash`** (ExifTool: a hash of the image data only);
   - for DNG also the **preview list** read by pw-raw;
   - a **complete metadata listing** (ExifTool `-a -G1 -s`, all groups).

### 9.2 Write to a temporary copy, never to the original

- ExifTool writes **a new file** next to the original: `-o <name>.pwtmp-<random>`, same folder,
  so the same volume. Only the properties that change are given.
- The original is **only read**.
- No `-overwrite_original_in_place`.
  - *Change from ADR 0004:* today's ratings write uses it.
  - In-place copying back can leave a half-written original if it is interrupted.
- **The checksum pass (TIFF and DNG, T92):** when the write includes a new IIM checksum, a second
  ExifTool call (`-IPTCDigest=new -overwrite_original <copy>`) stores it on the copy (§5.2). It
  must report one file updated and no warning; otherwise the write fails like any write of the
  copy.

### 9.3 Verify the temporary copy

**Restart ExifTool before reading a TIFF copy** **(owner 2026-11-12, T92, "choice A")**: after a
TIFF write that includes a new IIM checksum (also a failed one), ExifTool is restarted before the
copy is read (about 0.25 s). Reason, measured with ExifTool 13.59: after a running ExifTool
process (`-stay_open`) has written the checksum to a TIFF whose Photoshop block holds a checksum
followed by an `AgHg` resource (written by Lightroom and Photo Mechanic), that process reads the
block of every such TIFF wrongly until it ends, so a correct copy would be refused. JPEG and DNG
writes do not do this. If the restart fails, the write fails and ExifTool is marked as broken.

1. **Image data unchanged:** `ImageDataHash` equals the original's.
2. **Metadata as intended:**
   - the full listing differs from the original only in the properties written, their IIM and
     Exif representations, the IIM checksum, and `xmp:MetadataDate` / `xmp:ModifyDate`;
   - anything else that changed is an error.
3. **Structure intact:**
   - ExifTool reads the file without errors or warnings about the structure;
   - pw-raw finds the same previews (count, sizes, decodable JPEG);
   - DNG: `DNGVersion`, `NewSubfileType` of the main image and the IFD chain unchanged.
4. **The RAW image still opens:** for DNG, LibRaw opens and unpacks the copy without a "damaged"
   report (the M2 detection). This catches truncated or moved image data.
5. **Flush:** the temporary file is flushed to disk (`FlushFileBuffers`).

### 9.4 Replace atomically

- **`ReplaceFileW`** (same volume) replaces the original with the verified copy and keeps the
  original as a backup `<name>.pwbak-<random>` in the same step. It also keeps the original's
  creation time, attributes, ACL and alternate data streams.
- **The modification time** is set back to the original's (as `-P` does today; ADR 0004).
- **On success:** the backup is deleted after a final read of the new file (size, `ImageDataHash`).
- **If the final read fails** **(owner 2026-11-12, T86)**: the backup is kept, the journal entry is
  ended (so the recovery at the next start does not delete the backup), and the error names the
  backup as the previous version of the file.
- **If the volume does not support `ReplaceFileW`** (some network shares): rename the original
  to the backup name, rename the copy to the original name (`MoveFileExW` with
  `MOVEFILE_WRITE_THROUGH`), then delete the backup. Each step is reversible.

### 9.5 Errors and recovery

| Failure | Result | Action |
|---|---|---|
| Before or during writing the copy (ExifTool error, full disk, lock) | Original untouched | Delete the copy; report the file and the reason; nothing else changed |
| Verification fails | Original untouched | Delete the copy; report "metadata could not be written safely" with the check that failed; file kept as is |
| `ReplaceFileW` fails, original not replaced (`ERROR_UNABLE_TO_MOVE_REPLACEMENT`) | Original intact | Delete the copy; report |
| `ReplaceFileW` fails after moving the original to the backup (`ERROR_UNABLE_TO_MOVE_REPLACEMENT_2`) | Original under the backup name | Rename the backup back; if that fails, keep both files and tell the user exactly which file is the original |
| Crash or power cut at any point | At most an extra `.pwtmp-*` or `.pwbak-*` file | **Journal:** before step 9.2 a line (original, copy, backup) is written to a journal in the app's data folder and removed after 9.4. At start, open entries are resolved: original intact → delete the copy; original missing and backup present → restore the backup |
| Batch write (several files) | Every file is its own transaction | One failure stops nothing else; the summary lists the failed files |

- **The same transaction is used for XMP sidecars** (temp + verify + atomic replace), except the
  image-data checks.
  - **A new sidecar** (none existed) is checked with the rules of §9.3 against an empty original:
    everything in it must be what the write was meant to make. ExifTool always adds
    `XMP-x:XMPToolkit` to a new sidecar, which is allowed. **(owner 2026-11-12, T86)**
- **Every error names the file** it concerns (the original's path), so a batch summary can list
  it. **(owner 2026-11-12, T86)**
- **Ingest:** metadata is only written to the *copied* file, after its copy has been verified
  (ADR 0005), with this procedure.
- **Tests** (planned here before the implementation; written with T84–T92, state 2026-11-13; the
  tests are described in `docs/test-inventory.md`):
  - **failures at each step**, in `pw-metadata` (`src/fileops.rs`, `src/journal.rs`,
    `src/safe_write.rs`, `tests/safe_write.rs`):
    - a locked or read-only original is refused before writing;
    - too little free space is refused (the space check itself; a real full disk is not
      simulated);
    - ExifTool refusing an argument fails while the copy is written;
    - a copy that fails the metadata check or the image check is refused;
    - an original locked just before the replace stays intact;
    - a replace by renaming puts the original back when the copy is missing;
    - every journal recovery case (the original intact; only the backup left; nothing left);
  - **camera DNGs and Xiaomi 17 Ultra DNGs** (preview at the end of the file), with real files
    (`PW_TESTDATA`, run with `--ignored`): written with an unchanged `ImageDataHash` and the values
    read back (`pw-metadata` `tests/safe_write.rs`), and with RawStride's real image check
    (`pw-imagecheck` `tests/real_files.rs`: previews byte for byte equal, LibRaw opens the copy; a
    copy cut inside its RAW data and a copy with one byte changed in a preview are refused);
  - **not tested as planned:**
    - ExifTool stopping in the middle of a safe write (only the ExifTool worker's restart after
      ExifTool exits is tested);
    - a byte comparison of all non-metadata ranges; `ImageDataHash` and the byte-equal previews
      are used instead.

## 10. Templates (ingest and batch)

Each template field has one of four modes. The default is **Keep**. [Product]

| Mode | Single value | List (Creator, Keywords, Person Shown, …) | Lang Alt |
|---|---|---|---|
| **Keep** | not touched | not touched | not touched |
| **Replace** | set | replaced by the template's list | `x-default` set, other languages kept |
| **Append** | text added with a separator (also **Prefix**), e.g. Description, Headline | missing values added, order kept, no duplicates | as single value |
| **Clear** | removed as in §8 | removed | all languages removed |

- An empty template field never clears anything; clearing is always explicit.
- **No Clear for copyright-management fields** (§8). Clearing any other field asks for
  confirmation once per template.
- **Structures:**
  - set per sub-field with the same modes;
  - Location Created is one location;
  - Image Creator, Copyright Owner and Licensor have at most 3 entries.
- **Variables** (`{date}` …) are expanded before the mode is applied.
- **Template files are XMP.** The modes are stored in a RawStride namespace that other tools
  ignore.

## 11. Validation (warnings, not refusals)

- **Lengths:** against the IIM byte limits. Alt Text: warning above 250 characters [IPTC §7.1].
- **Country Code:** ISO 3166-1, 2 or 3 letters, the scheme derived from the length; the code is
  authoritative over the country name [IPTC §7.5; User Guide].
- **Creator's Jobtitle** without Creator → warning [User Guide].
- **Dates:** the forms of IPTC §14.2; no time zone added (§7).
- **URIs:** for controlled-vocabulary properties (Digital Source Type: IPTC NewsCodes [IPTC
  §11.13]).
- **PLUS properties** (Model Release Status, Property Release Status, Minor Model Age Disclosure,
  Data Mining) **(Q9, owner 2026-11-11):**
  - edited as a fixed choice list with the vocabulary's English labels (GR14; ADR 0017 U10d-Q3,
    owner 2026-11-14, in place of the Swedish labels of 2026-11-11); the full PLUS URI is stored
    in the file. ExifTool removes the prefix `http://ns.useplus.org/ldf/vocab/` when it reads and
    adds it when it writes, so RawStride holds and edits the code alone (`MR-NAP`); a full URI
    given to ExifTool would be stored with the prefix twice (found in the review of T127,
    2026-11-14). URI values are written without ExifTool's print conversion (`#=`), as they are
    read (`-n`), so ExifTool takes the code and not its label (T128, owner 2026-11-14);
  - the allowed values are taken from the PLUS controlled vocabulary into the mapping table (§3);
    the IPTC spec only refers to it ("must be one of the URIs listed in the Controlled Vocabulary
    section of the PLUS specification", [IPTC §11.11]). The list is read before implementation
    (§16);
  - an unknown value in a file is kept and shown with a warning, never refused or removed.
- **Image Registry Entry:** both sub-fields required [IPTC §11.21].
- **Rating:** −1 … 5; real numbers allowed; out-of-range values shown as invalid and not changed
  (§8).
- **ExifTool version:** at least 12.67, checked at start [IPTC §5.1].

## 12. Tool and verification

- **ExifTool** remains the reader and writer, with explicit tag lists per format.
  - The MWG *module* of ExifTool is not used: its "strict" mode ignores non-standard locations.
    The rules above are implemented by RawStride.
  - Writing always goes through §9.
- **Tests:**
  - round trip per format;
  - IIM = XMP after every write, and the checksum updated;
  - all three checksum cases of §5.1, including the predicted-IIM comparison;
  - Exif/XMP conflicts shown;
  - RAW bytes unchanged (hash);
  - DNG/JPEG/TIFF image data unchanged (ImageDataHash);
  - hierarchical keywords, unknown namespaces, other languages and deprecated IIM datasets kept;
  - zone-less dates stay zone-less;
  - truncated dates;
  - IIM prefixes and byte limits;
  - the four template modes;
  - fault injection (§9.5).
- **Level-C samples** (outside the repo, in `PW_TESTDATA`) from the owner **(Q11, owner
  2026-11-11; changed 2026-11-14)**; they are needed before the implementation can be tested, not
  before approval:
  - an ARW with sidecar, a JPEG and a DNG whose metadata only Photo Mechanic has written;
  - a Xiaomi 17 Ultra DNG;
  - files from the owner's archive as they are, written by both Photo Mechanic and Lightroom
    Classic (an ARW with sidecar and a JPEG): read without error, conflicts shown, unknown content
    kept.

  **Changed 2026-11-14 (owner):** the owner has no Lightroom Classic, so there are no files written
  by Lightroom alone; the mixed archive files take their place. The IIM-only JPEG with a checksum
  mismatch is dropped: the case is tested on built data in `pw-metadata` (`reconcile.rs`).

## 13. Consistency check of the standards in this architecture

The three documents were compared for every rule above (details in the analysis §8–§10).

| Topic | IPTC 2025.1 | IPTC-MG 2023.1 | MWG 2.0 | This ADR | Result |
|---|---|---|---|---|---|
| Save IIM and XMP | "should … both" (§5.3.4) | – | IIM only if present or needed for compatibility | as MWG + setting | **Deviation from IPTC "should"**, deliberate (§5.2) |
| IIM/XMP precedence | none | – | matching or no checksum → XMP; differing → IIM per property | as MWG, but **no checksum → user decides** | **Deviation from MWG** (stricter), deliberate (§5.1) |
| IIM checksum on write | – | – | MUST create or update "whether changed or not" | not written while a no-checksum conflict is unresolved | **Deviation from a MWG MUST**, deliberate and limited to that case (§5.2, Q1) |
| Exif/XMP precedence | none | none | prefer Exif | XMP, conflicts shown | **Deviation from MWG**, deliberate (§6.2) |
| Creator → Artist | – | one name; also Photographer | all names, "; " | IPTC-MG for writing, MWG for reading | The standards disagree; the newer IPTC-MG is followed (§6.5) |
| Exif text encoding | – | Exif 3.0: UTF-8 type | UTF-8 in ASCII | by Exif version | Compatible |
| DateTimeOriginal | suggest Date Created from it | mapping both ways | export corrections to Exif | never changed automatically | Compatible (the mapping is not mandatory; corrections are a manual feature, Q12) |
| Time zone | TZD optional (§14.2) | IIM time with offset | never invent | never invent; IIM 2:60 left out without a known zone | Compatible (Q3; to confirm against IIM 4.2) |
| Rating | −1, 0…5, real | – | float, rounding for display | as both | Compatible |
| Hierarchical keywords | – | – | MUST preserve | preserved | Compatible |
| Deprecated IIM datasets | keep in sync (§9.1) | – | – | kept and synced | Compatible |
| DNG embedded / RAW sidecar | not covered | – | DNG not covered | product decision | No conflict |
| Clearing | – | – | update both forms or remove the other | consistent across forms, RAW untouched; a cleared property is marked in the sidecar (Q2) | Compatible: the RAW Exif cannot be changed, and the marker only affects RawStride |
| Lang Alt edits | Lang Alt type (§14.1) | – | §5.2, not yet checked on this point | edit `x-default` only; other entries kept | **To verify:** the Adobe XMP Toolkit also updates entries equal to the old `x-default`. If MWG §5.2 or ISO 16684-1 recommend that, this is a deliberate deviation (Q7) |

**One rule of this ADR deliberately contradicts a MUST:** the IIM checksum is not written while
a no-checksum conflict is unresolved (§5.2, Q1), so that a write never decides the conflict
silently. All other deviations concern SHOULD-level recommendations; each is marked
[Product ≠ std] with its reason. The Lang Alt rule
(§8, Q7) is still to be checked against MWG §5.2 and ISO 16684-1.

## 14. Questions (all answered by the owner)

| # | Question | Answer |
|---|---|---|
| ~~**Q1**~~ | **Writing a file with an unresolved IIM/XMP conflict** (no-checksum case). Writing both forms and the checksum would decide the conflict silently in favour of the written value | **Resolved 2026-11-11:** Writing other fields is allowed. For each conflicting property both forms are left as they are **and no new checksum is written**, until the user decides. MWG requires the checksum "whether changed or not", so this is a deliberate exception, limited to unresolved conflicts (§5.2, §13). Batch operations list such files |
| ~~**Q2**~~ | **Clearing a field for an ARW/RW2 whose embedded camera Exif has a value** (e.g. Artist set in the camera). After clearing, the camera value would fill the gap again (§6.2) | **Resolved 2026-11-11:** The sidecar records the cleared property in the RawStride namespace (`Cleared`, a Bag of property ids), which suppresses the camera value in RawStride (§6.4). Other tools ignore it and will show the camera value. Before deciding, the owner compared Lightroom (may use the camera's raw ImageDescription on export after the caption was cleared; a bug report since Lightroom 3) and Photo Mechanic (copies camera Artist/Copyright one way into IPTC; clearing is not documented). An explicitly empty sidecar value may be added later if the level-C samples show that it works in both tools |
| ~~**Q3**~~ | **IIM 2:60 Time Created without a known time zone.** IIM's time format includes an offset; writing `+0000` would invent one | **Resolved 2026-11-11:** Write only 2:55 (the date) and leave 2:60 out until the zone is known. To be confirmed against the IIM 4.2 specification (not read; §16) |
| ~~**Q4**~~ | **Exif overlap fields inside DNG:** the decision lists JPEG/TIFF for synchronisation | **Resolved 2026-11-08:** Treat DNG like TIFF: overlap fields that exist are updated (MWG requires updating or removing the other form) |
| ~~**Q5**~~ | **A foreign `<basename>.xmp` next to a DNG** (written by another tool) | **Resolved 2026-11-08:** Never written and never used as a source; the info panel shows a notice that it exists and is ignored |
| ~~**Q6**~~ | **Creating missing Exif overlap fields** in JPEG/TIFF | **Resolved 2026-11-08:** Do not create missing fields in JPEG, TIFF or DNG (DNG is handled like TIFF, Q4); synchronise only existing overlap fields |
| ~~**Q7**~~ | Exact Lang Alt rules (`x-default` alongside a language entry) | **Resolved 2026-11-08 (product policy):** edit only `x-default`; leave all other language entries and their values unchanged. If `x-default` is absent, add it first. Clearing removes the whole property. Adobe's SDK also synchronises entries matching the old default ([`SetLocalizedText`](https://github.com/adobe/XMP-Toolkit-SDK/blob/v2025.03/XMPCore/source/XMPMeta-GetSet.cpp), SDK v2025.03); RawStride deliberately does not. The full ISO 16684-1 text was not accessible during review, so verify this policy against the standard before implementation. |
| ~~**Q8**~~ | Normative sidecar naming (`name.xmp` vs `name.ARW.xmp`) and RAW+JPEG pairs | **Resolved 2026-11-11:** `<basename>.xmp`, as ADR 0004 and the default of Lightroom and Photo Mechanic. A `<basename>.<ext>.xmp` is never read or written and is reported, as in Q5. RAW+JPEG pairs as §4.2. To confirm with the level-C samples; Adobe XMP Part 3 is read if available |
| ~~**Q9**~~ | Allowed URI values of PLUS properties (release status, minor model age, data mining) | **Resolved 2026-11-11:** a fixed choice list with Swedish labels (English since 2026-11-14, ADR 0017 U10d-Q3), values taken from the PLUS controlled vocabulary; unknown values kept with a warning (§11). The vocabulary itself must be read before implementation (§16) |
| ~~**Q10**~~ | Fallback encoding for IIM/Exif text that is neither ASCII nor UTF-8 | **Resolved 2026-11-11:** Windows-1252 by default, changeable in the settings; the value is marked as converted (§5.4) |
| ~~**Q11**~~ | Test files | **Resolved 2026-11-11:** the level-C samples of §12, provided by the owner before the implementation is tested. **Changed 2026-11-14:** Photo Mechanic files, a Xiaomi DNG and mixed Photo Mechanic / Lightroom Classic archive files; no Lightroom-only files, no IIM-only JPEG (§12) |
| ~~**Q12**~~ | A manual "correct camera time" feature (MWG: such corrections SHOULD be exported to Exif) | **Resolved 2026-11-11:** Out of scope now; would be the only way DateTimeOriginal ever changes, and only on the user's explicit command (§7) |
| ~~**Q13**~~ | "New IIM only when backward compatibility requires it": how is that detected besides the setting? | **Resolved 2026-11-11:** Only the setting (and possibly a future export profile); no automatic detection (§5.2) |

## 15. Consequences

- `pw-metadata` grows from "rating + label + a few IPTC fields" into:
  - a property model with a mapping table;
  - a reader with reconciliation and conflict records;
  - a transactional writer per format.
- **Changes to ADR 0004/0005 behaviour:**
  - Embedded writes of ratings and labels (DNG/JPEG/TIFF) move from `-overwrite_original_in_place`
    to the safe procedure (§9).
  - Existing IIM is kept in sync, with its checksum updated.
  - The ingest IPTC template gets the four modes. Today it is "Keep for empty fields, Replace for
    filled ones", which is compatible.
  - Contact info stays as single Text values (already conformant).
- **Not part of this ADR:** editing hierarchical keywords (they are only preserved), a
  catalog/index of metadata, C2PA, and Schema.org output.

## 16. Before implementation

1. ~~The owner decides D5 and the IIM default~~ – decided 2026-11-04.
2. ~~Read the IPTC spec 9–17, the Mapping Guidelines and MWG 2.0~~ – done 2026-11-04 (analysis
   §8–§9).
3. ~~Answer the remaining open questions~~ – Q1–Q3 and Q8–Q13 answered 2026-11-11 (§14).
   The ADR was approved by the owner the same day.
4. ~~**Read the PLUS controlled vocabulary**~~ (Q9) – read 2026-11-14 and saved in
   `D:\PhotoWorkflowData\references\` (the LDF 1.2.0 vocabulary file; Data Mining from the PLUS
   XMP specification 2.0.1, in the folder's `README.md`).
5. Read **XMP Part 1** (ISO 16684-1) and **MWG §5.2** to verify the Q7 policy, **XMP Part 3** (Q8)
   and **IIM 4.2** (Q3) if the owner provides them.
6. ~~**Level-C samples**~~ (Q11, the set changed 2026-11-14, §12) – received 2026-11-14 and
   tested by T127 (`crates/pw-metadata/tests/level_c.rs`); what they showed is in §4.2 and §6.4,
   and they led to T129 (§5.1) and to the PLUS codes of §11.

## 17. Decisions during implementation (D1–D16)

The owner's decisions on questions that came up while milestone 8 was built. They were kept in the
step list (now `docs/tasks/plans/adr-0007-milestone-8-steps.md`) until 2026-11-13 and are copied
here unchanged, with each proposal and its approval; the step list points here. Later changes are
listed under the table (§17.1).

| # | Decision | §§ | Status |
|---|---|---|---|
| D1 | The XMP namespace (prefix and URI) of `Cleared` (§6.4: "fixed with the implementation"). Reading and writing `Cleared` gets its own small task after D1 | §6.4 | open |
| D2 | `serde_json` (=1.0.154, already in `Cargo.lock` through the Tauri app, so nothing new is compiled) as a dependency of `pw-metadata`, to read ExifTool's `-j -struct` output. **Approved by the owner on 2026-11-11** | – | done |
| D3 | `windows-sys` (=0.61.2, already in `Cargo.lock` through `pw-loupe`, so no new package) as a Windows-only dependency of `pw-metadata`, features `Win32_Foundation` and `Win32_Storage_FileSystem`, for `ReplaceFileW`, `MoveFileExW`, free space and read-only volumes. **Approved by the owner on 2026-11-12** | §9 | done |
| D4 | A new workspace crate **`pw-imagecheck`** for the real `ImageCheck`, depending on `pw-metadata`, `pw-raw` and `pw-libraw` (all workspace crates: no new package from crates.io; `Cargo.lock` gains only the crate itself). Root `Cargo.toml`: `pw-imagecheck = { path = "crates/pw-imagecheck" }` under `[workspace.dependencies]`; `pw-loupe`: `pw-imagecheck.workspace = true` (T91). Why not elsewhere: `pw-metadata` must not depend on `pw-raw`/LibRaw (§9.3); in `pw-viewcore` the view core would depend on `pw-metadata`; in `pw-loupe` it would need `pw-libraw` as a direct dependency (also a `Cargo.toml` change) and the Tauri app could not use it. **Approved by the owner on 2026-11-12** | §9.3 | done |
| D5 | The separator of `Append` and `Prefix` for a single text or Lang Alt (§10 says "a separator" without naming it). Reviewer's proposal: one space for every property. **Approved by the owner on 2026-11-12**: one space | §10 | done |
| D6 | `xmpRights:Marked` ("Copyrighted" in Lightroom and Photo Mechanic): today's template writes `True` when a copyright is given, but the property is not in the table, so the plan cannot write it. Reviewer's proposal: add it to the table as a non-IPTC entry (as `xmp:Label`) and let `Iptc::to_template` set `True` with a Copyright Notice, as today. The alternative is to stop writing it. **Approved by the owner on 2026-11-12**: added to the table, `True` with a Copyright Notice | §10, §15 | done |
| D7 | The **Tauri app** (`src-tauri`) also writes ratings and the ingest template. Reviewer's proposal: it keeps today's in-place write (`write_rating`, `write_iptc`) until it is retired (GR8), as RawStride is the program in use; moving it as well needs a release of `pw-rawview`'s LibRaw sessions and its own task. **Approved by the owner on 2026-11-12**: the Tauri app keeps today's write | §9, §15 | done |
| D8 | When the safe write creates the Photoshop block of a TIFF or DNG (to store a new `IPTCDigest`), ExifTool adds the validation warning `Non-standard format (undef) for IFD0 0x8649 PhotoshopSettings`, and T84's check refuses the copy for having one more warning. Reviewer's proposal: allow exactly one more warning (not error) in that case: the copy has `Photoshop:IPTCDigest`, the original has not, the file is a TIFF or DNG and the plan writes `-IPTCDigest=new`. The alternative, not writing a checksum in a TIFF/DNG without one, breaks §5.2. **Approved by the owner on 2026-11-12** | §5.2, §9.3 | done |
| D9 | What RawStride does when ExifTool is missing or older than 12.67 (§11 only says "checked at start"). Reviewer's proposal: RawStride starts anyway and shows one message at start; reading works as today; every write (ratings, the editor, the ingest template) is refused with a clear text until ExifTool is updated; a version that cannot be read counts as too old. The alternative is a warning only, with writes allowed. **Approved by the owner on 2026-11-12** | §11, §9 | done |
| D10 | Where and how RawStride's settings are stored (it has no settings file yet). Reviewer's proposal: a text file `%LOCALAPPDATA%\RawStride\settings.txt` (the folder of the write journal), one `key = value` per line, no new dependency; for this user on this computer only. **Approved by the owner on 2026-11-12** | §5.2, §5.4 | done |
| D11 | The Swedish names of the IPTC properties in the info panel and the editor. Reviewer's proposal: the import dialog's names where they exist, and for the rest: Titel (`dc:title`), Rubrik (`photoshop:Headline`), Beskrivning (`dc:description`), Alternativtext (`Iptc4xmpCore:AltTextAccessibility`), Nyckelord (`dc:subject`), Fotograf (`dc:creator`), Fotografens titel (`photoshop:AuthorsPosition`), Fotografens kontaktuppgifter (`Iptc4xmpCore:CreatorContactInfo`, its fields E-post, Telefon, Webb), Copyright (`dc:rights`), Credit (`photoshop:Credit`), Källa (`photoshop:Source`), Användarvillkor (`xmpRights:UsageTerms`), Webbadress för rättigheter (`xmpRights:WebStatement`), Skapad (`photoshop:DateCreated`), Plats (`Iptc4xmpCore:Location`), Stad (`photoshop:City`), Län/delstat (`photoshop:State`), Land (`photoshop:Country`), Landskod (`Iptc4xmpCore:CountryCode`), Instruktioner (`photoshop:Instructions`), Jobb-id (`photoshop:TransmissionReference`), Beskrivningens författare (`photoshop:CaptionWriter`), Betyg (`xmp:Rating`), Etikett (`xmp:Label`), Upphovsrättsskyddad (`xmpRights:Marked`), Plats där bilden togs (`Iptc4xmpExt:LocationCreated`), Plats som visas (`Iptc4xmpExt:LocationShown`), Bildskapare (`plus:ImageCreator`), Upphovsrättsinnehavare (`plus:CopyrightOwner`), Licensgivare (`plus:Licensor`), Person i bilden (`Iptc4xmpExt:PersonInImage`), Händelse (`Iptc4xmpExt:Event`), Digital källtyp (`Iptc4xmpExt:DigitalSourceType`), Prioritet, Kategori, Tilläggskategorier (the deprecated `photoshop:Urgency`, `photoshop:Category`, `photoshop:SupplementalCategories`); every other property with IPTC's English name. **Approved by the owner on 2026-11-12** | §3 | done |
| D12 | What the editor edits in I10. Reviewer's proposal: **one image at a time** (several images later, as a template on the selection); these 22 fields in this order: Titel, Rubrik, Beskrivning, Alternativtext, Nyckelord, Fotograf, Fotografens titel, E-post, Telefon, Webb, Copyright, Credit, Källa, Användarvillkor, Skapad, Plats, Stad, Län/delstat, Land, Landskod, Instruktioner, Jobb-id; plus a choice for every unresolved conflict. Later: lists of structures (Location Created, Image Creator, …), PLUS fields (I11), the other properties. **Approved by the owner on 2026-11-12** | §8, §10, §11 | done |
| D13 | The modes in the import dialog (§10). Reviewer's proposal: the Swedish names Behåll, Ersätt, Lägg till efter, Lägg till före, Rensa; the default Ersätt for every field (an empty field then does nothing, exactly as today); Fotograf offers Behåll, Ersätt, Lägg till efter; Copyright and Credit also Lägg till före; E-post, Telefon and Webb all five (no Rensa of copyright-management fields, §8; no "before" on a list). The modes are remembered during the session like the texts, not between runs. **Approved by the owner on 2026-11-12** | §8, §10 | done |
| D14 | The confirmation before a Clear (§10: "once per template"). Reviewer's proposal: when an import is started with at least one field in Rensa, one window lists those fields ("Rensa och importera" / "Avbryt"); asked once per started import, never per image. The editor does not ask (emptying a field there is the user's own choice for one image). **Approved by the owner on 2026-11-12** | §10 | done |
| D15 | The place of the info panel and the keys. Reviewer's proposal: a panel on the right side, in the loupe and in the contact sheet (the selected image), shown and hidden with `M`, hidden at start; the editor opens with `E` or the button "Redigera" in the panel, in its own window. **Approved by the owner on 2026-11-12** | – | done |
| D16 | One metadata window instead of the panel and the editor (replaces the panel and the keys of D15; the fields of D12 stay). Owner, 2026-11-12: like a floating info window where everything is seen and edited directly, **locked to the image until OK or Avbryt**. Reviewer's proposal for the details: `M` opens a floating, movable and resizable window "Metadata – <fil>" for the current image (one image of a kind with metadata support); `E`, the right-hand panel and the button "Redigera" go away. While it is open it is modal, like the import dialog: the loupe and the contact sheet take no keys or clicks, so the window cannot change image. Top to bottom: the 22 fields editable, "Olösta konflikter" with a choice each, "Varningar", then read-only "Övrig metadata" (every other property with its origin), the conflicts already decided by a rule, and the notices. "OK" writes the edits (nothing to write: closes) and closes after a successful write; with write notices the window stays showing "Sparat." and them until OK or Avbryt; a failed write keeps the window and the typed text. "Avbryt" closes without writing; Escape is Avbryt but asks "Släng ändringarna?" ("Släng" / "Fortsätt redigera") when there are edits. Unsaved edits are dropped at program exit (a write already running is waited for, as today). **Approved by the owner on 2026-11-12, with the details as proposed** | §5.1, §6.2 | done |

### 17.1 Later changes to D1–D16

- **D1** is still open (`docs/tasks/backlog.md`): reading and writing `Cleared` waits for it.
- **D11, language** (owner, 2026-11-12, T102 and T104): `property_label` takes a language
  (`Language::Sv`, `Language::En`). English always gives the IPTC name from the property table;
  Swedish gives the D11 name where there is one and otherwise the IPTC name. The Swedish names are
  kept in one table in `pw-loupe`'s `info_panel.rs`. Swedish names for **every** property, so the
  Swedish panel shows no English IPTC names, is a future improvement (`docs/tasks/backlog.md`).
- **D11, English labels** (owner, 2026-11-13, `docs/glossary.md` G1): English is the interface's
  base language (GUI_REQUIREMENTS GR14). The English label is IPTC's name from the property table,
  except that " (legacy)" is left out, the contact fields are **Email**, **Phone** and **Web URL**,
  `xmp:Rating` is **Rating** and `xmpRights:Marked` is **Copyrighted** (G2); the full IPTC name may
  be shown as a tooltip. Changed in code by T117.
- **D11 and D12, names** (owner, 2026-11-13, T114): "Fotografens titel" (`photoshop:AuthorsPosition`)
  is now **Yrkestitel**, and "Fotografens kontaktuppgifter" (`Iptc4xmpCore:CreatorContactInfo`) is
  now **Kontaktuppgifter** (ADR 0017 §2.3.1).
- **D15** was replaced by D16 the same day (2026-11-12).
- **D16** is changed by ADR 0017:
  - the window is no longer locked to one image: previous/next and "Spara och nästa" with the
    question "Spara ändringarna?" (Spara / Släng / Avbryt), which also replaces "Släng
    ändringarna?" for Escape (ADR 0017 U-Q8, §2.3.1 and §6.1, U12-Q1–U12-Q5; owner, 2026-11-12 and
    2026-11-13);
  - "Övrig metadata" shows its values without an origin line under each one; the conflicts keep
    theirs (owner, 2026-11-13, T114).

## Revision history

| Revision | Date | Changes |
|---|---|---|
| 1 | 2026-11-04 | Storage, IIM policy (only if present + setting), XMP primary, templates, validation |
| 2 | 2026-11-04 | All owner decisions of 2026-11-04 written in:<br>- IIM/XMP precedence by checksum (three cases) and conflict markers;<br>- Exif/XMP rules (XMP primary, JPEG/TIFF sync, RAW Exif kept as conflict);<br>- the fixed storage decision (DNG embedded incl. Xiaomi 17 Ultra, no sidecar);<br>- **the safe write procedure (§9)**;<br>- date, time and time-zone rules;<br>- hierarchical keywords, decimal rating, deprecated IIM, encodings, phrases, clearing.<br>Also: IPTC 2025.1 §9–17, IPTC-MG 2023.1 and MWG 2.0 incorporated with source labels; the consistency check (§13); open questions Q1–Q13 |
| 3 | 2026-11-08 | Owner answered Q4: synchronise existing Exif overlap fields in DNG like TIFF |
| 4 | 2026-11-08 | Owner answered Q5: ignore foreign XMP sidecars next to DNG and show a notice |
| 5 | 2026-11-08 | Owner answered Q6: do not create missing Exif overlap fields |
| 6 | 2026-11-08 | Owner answered Q7: edit only `x-default`, preserving other language entries |
| 7 | 2026-11-08 | Consistency fixes after review, no new decisions:<br>- §6.3/§6.5: Artist and Photographer are only written if they exist (Q6);<br>- §6.3: only "both ways" fields of §6.1 are written;<br>- §8: Lang Alt reading rule without `x-default`; source label for Q7 marked as to be verified, §13 row added;<br>- §2: clearing a Lang Alt field removes all languages;<br>- §4: cross-reference to §5.2, IIM checksum location (PSIR 1061 in tag 34377);<br>- Q6 resolution names DNG; SDK link pinned to v2025.03; spelling "synchronise" |
| 8 | 2026-11-11 | Owner answered Q1–Q3 and Q8–Q13, all as proposed; Q2 after a comparison with Lightroom and Photo Mechanic:<br>- answers written into §4.2 (Q8), §5.2 (Q1, Q13), §5.3 and §7 (Q3, Q12), §5.4 (Q10), §6.4 (Q2), §11 (Q9), §12 (Q11);<br>- §13: the Q1 checksum rule is recorded as a deliberate deviation from a MWG MUST;<br>- §16: what is left before implementation;<br>- product name RawStride (ADR 0012) instead of the former name; paths, crate names and file suffixes unchanged |
| 9 | 2026-11-11 | Owner approved the ADR (status "accepted"); no change to the rules |
| 10 | 2026-11-13 | §4.2: a camera `xmp:Rating` of 0 inside a RAW file counts as no rating (it neither fills nor gives a conflict); owner decision after the hand check of T110 (T113) |
| 11 | 2026-11-13 | §17: the owner's decisions D1–D16 of the implementation, moved unchanged from the step list, with the later changes (§17.1); no new decision |
| 12 | 2026-11-13 | Owner decisions on safe writing that lived only in task logs, written in; no new decision:<br>- §5.2: both forms brought in line after every write (review of T83); an edit decides an unresolved conflict, and re-encoding to UTF-8 does not (T83, Q1); the checksum of a TIFF or DNG in a second call (T92);<br>- §9.2: the checksum pass (T92);<br>- §9.3: ExifTool restarted before a TIFF copy is read (T92, choice A);<br>- §9.4: a failed final read keeps the backup (T86);<br>- §9.5: a new sidecar checked against an empty original; every error names its file (T86); the tests bullet brought up to date with the tests written in T84–T92 |
| 13 | 2026-11-13 | §17.1: the English labels of D11 (`docs/glossary.md` G1, G2); no change to the rules of the ADR |
| 14 | 2026-11-14 | Owner decision: the level-C samples (§12, Q11) are Photo Mechanic files, a Xiaomi 17 Ultra DNG and mixed Photo Mechanic / Lightroom Classic archive files (the owner has no Lightroom Classic); the IIM-only JPEG is dropped; §4 (Q8) and §6.4 note what stays unconfirmed for Lightroom. §16 point 4: the PLUS vocabulary read (R1) |
| 15 | 2026-11-14 | Owner decision (ADR 0017 U10d-Q3): §11 and Q9, the PLUS choice lists show the vocabulary's English labels instead of Swedish ones (GR14); Digital Source Type also becomes a choice list (U10d-Q2, IPTC NewsCodes saved as R2) |
| 16 | 2026-11-14 | After the first run of the level-C tests (T127), owner approved: §5.1, an IIM value equal to the whole XMP text is not a conflict (T129); §11, RawStride holds the PLUS codes, ExifTool adds the URI prefix; URI values written with `#=` (T128) |
| 17 | 2026-11-14 | I12 done (T127): what the level-C samples showed written into §4.2 (Q8, Photo Mechanic's `<basename>.XMP`) and §6.4 (Photo Mechanic removes an emptied field, keeps empty structure sub-fields); §16 point 6 done |
