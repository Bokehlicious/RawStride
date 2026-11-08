# ADR 0007: Metadata handling according to the IPTC standards

- Status: **proposed for review (revision 7, 2026-11-08).**
  - The owner's decisions of 2026-11-04 are written in.
  - Q4 was answered by the owner on 2026-11-08: DNG Exif overlap fields are synchronised like TIFF.
  - Q5 was answered by the owner on 2026-11-08: foreign XMP sidecars next to DNG are ignored and reported.
  - Q6 was answered by the owner on 2026-11-08: missing Exif overlap fields are not created.
  - Q7 was answered by the owner on 2026-11-08: edit `x-default` only and preserve all other language entries.
  - Revision 7 (2026-11-08) makes the text consistent with the Q4–Q7 answers; it adds no new decision.
  - The open questions in §14 must be answered, and the ADR approved, **before any
    implementation starts**.
- Date: 2026-11-04 (revision 1: storage, IIM policy and the role of XMP; revision 2: all owner
  decisions of 2026-11-04, MWG 2.0, the full IPTC specification and the Mapping Guidelines;
  revision 3: Q4 answered by the owner on 2026-11-08; revision 4: Q5 answered by the owner on
  2026-11-08; revision 5: Q6 answered by the owner on 2026-11-08; revision 6: Q7 answered by the
  owner on 2026-11-08; revision 7: consistency fixes after review, 2026-11-08).
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

PhotoWorkflow follows these documents, not Photo Mechanic's internals. It stays compatible with
Lightroom and Photo Mechanic through the standards and the tools' public documentation.

### 1.1 How rules are labelled

Every rule in §3–§12 carries its basis:

| Label | Meaning |
|---|---|
| **[IPTC]** | IPTC Photo Metadata Standard 2025.1 (section given) |
| **[IPTC-MG]** | IPTC Photo Metadata Mapping Guidelines 2023.1 |
| **[MWG]** | MWG Guidelines 2.0 (section given) |
| **[Product]** | PhotoWorkflow's own decision (owner's decision of 2026-11-04 unless said otherwise) |
| **[Product ≠ std]** | A product decision that deliberately departs from a recommendation of a standard. The reason is given |

## 2. Decision summary

| # | Decision | Basis |
|---|---|---|
| 1 | **XMP is PhotoWorkflow's primary representation** for the explicitly mapped descriptive fields | [Product]; IPTC Core/Extension are defined in XMP [IPTC §1] |
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

- **Name:** `<basename>.xmp`, the same as ADR 0004 (Adobe convention). The normative naming rule
  in Adobe XMP Part 3 is not read: **Q8**.
- **Created** on the first change; **edited in place** afterwards (unknown content kept), with the
  same transaction (§9.5).
- **RAW+JPEG pairs:** only the RAW reads and writes `<basename>.xmp`; the JPEG keeps its embedded
  metadata.
- **Embedded metadata in the RAW** (Exif, XMP written by the camera) is read-only. It only fills
  properties the sidecar lacks, and differences are shown as conflicts (§6.4).

## 5. IIM and XMP

### 5.1 Reading: precedence

When IIM and XMP are both present, the IIM checksum decides. The checksum is an MD5 of the whole
IIM block in PSIR 1061 [MWG §4.2.3.2].

| Situation | Rule | Basis |
|---|---|---|
| **Checksum present and matches** | **XMP has precedence**; IIM only fills a property XMP lacks | [MWG §4.2.3.2 scenario 3] |
| **Checksum present and differs** (a tool unaware of XMP changed the IIM) | **Per property:** the IIM value PhotoWorkflow would write from the XMP is predicted (truncation to the byte limit, UTF-8, IIM formats §5.3). If the prediction equals the actual IIM value, XMP is used. **If it differs, the IIM value has precedence** for that property | [MWG §4.2.3.2 scenario 4] |
| **No checksum** (IIM and XMP both present, values differ) | **Neither is assumed newer.** The property is registered as a **conflict** and the user decides. Until then the info panel shows both values | **[Product ≠ std]:** MWG says to prefer XMP here ("checksum … does not exist … XMP … SHOULD be preferred"). Reason: without a checksum nothing shows which tool wrote last, and preferring XMP could hide an edit made by an older tool |
| Only IIM, or only XMP | That one is used | [MWG §4.2.3.2 scenarios 1–2] |

- **Equality is always tested on the predicted IIM value.** A value that only differs because of
  IIM truncation or encoding is not a conflict. [MWG §4.2.3.2]
- **Conflict marker** [Product]:
  - **Every** difference between IIM and XMP is recorded as a conflict with both values and the
    rule that was applied: XMP (matching checksum), IIM (differing checksum), or open (no
    checksum).
  - The marker stays visible until the next write by PhotoWorkflow brings both forms in line, so
    metadata changes made outside PhotoWorkflow become visible.
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
  - When "backward compatibility requires it" is detected other than by this setting: **Q13**.
- **Checksum:** "If IPTC-IIM and XMP are both present, whether changed or not, a Changer MUST
  create or update the checksum" [MWG §4.2.3.2]. It is computed over the whole IIM block after
  conversion.
- **Unresolved conflicts when writing:** **Q1** (a proposal is given there).
- **Values longer than the IIM limit** are kept in full in XMP and cut at a UTF-8 character
  boundary in IIM. The user is told. The limit counts **bytes, not characters**. [IPTC §15.1]

### 5.3 IIM value formats [IPTC]

| Property | IIM rule | Source |
|---|---|---|
| Intellectual Genre (2:04) | Prefix `000:`; the 64-byte limit applies to the code only | [IPTC §7.16] |
| Subject Code (2:12) | Prefix `IPTC:` | [IPTC §7.23] |
| Date Created (2:55) | `YYYYMMDD`, `00` for an unknown month or day (`19180000`) | [IPTC §7.10] |
| Time Created (2:60) | `hhmmss±hhmm`, no sub-seconds. **Only written when the offset is known** (§7); otherwise: **Q3** | [IPTC-MG Exif Note on Date Created] |
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
    3. otherwise the fallback encoding (**Q10**).
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
  - Reason: XMP is PhotoWorkflow's primary representation (owner's decision); Lang Alt, lists and
    structures exist only in XMP; and compliant tools keep both forms equal anyway [MWG §4.2.3.1].
- **Conflict between Exif and IIM** (both present, XMP missing): Exif if the IIM checksum matches
  or is missing, IIM if it differs. [MWG §4.2.3.3]
- **Every Exif/XMP difference is detected and registered as a conflict** (both values shown).
  [Product]
- **Text encoding of Exif when reading:** ASCII → valid UTF-8 → fallback (**Q10**). Exif 3.0 tags
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
- How a field **cleared** in the sidecar stays cleared, although the camera's Exif still has a
  value: **Q2**.

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
| IIM Time Created when the zone is unknown | **Q3** |

## 8. Other rules

| Rule | Basis |
|---|---|
| **Hierarchical keywords** (`mwg-kw:Keywords`, `lr:hierarchicalSubject`) are kept **unchanged**, including all nested information, even though PhotoWorkflow cannot edit them yet | [MWG §5.10]: "MUST preserve all nested information"; [Product] for `lr:` |
| **Keywords may be phrases** and are never split at spaces | [IPTC §7.18] |
| Keyword lists are rewritten as a whole in IIM; XMP qualifiers on keywords are kept | [MWG §5.1] |
| **Rating:** stored internally with full precision (a real number). Rounded only for display (out-of-range values shown as the nearest of −1…5). Written only when the user changes it. 0 = unrated; a missing value counts as 0; −1 = rejected | [IPTC §11.19]; [MWG §5.5] |
| **Lang Alt:**<br>- PhotoWorkflow edits `x-default` only and preserves all other language entries, including one that held the same value as the old `x-default`;<br>- when `x-default` is absent, it is added first;<br>- **reading** without `x-default`: the first entry is shown, marked with its language;<br>- clearing removes the whole property, all languages included | [IPTC §14.1]; [Product]. **To verify:** the Adobe XMP Toolkit also updates entries equal to the old `x-default`; if MWG §5.2 or ISO 16684-1 recommend that, this becomes [Product ≠ std] (Q7, §13, §16) |
| **Unknown namespaces, list items not touched, camera maker notes** are never removed or rewritten | [Product] |
| **Creator's Contact Info** and **Creator's Jobtitle** belong to the first Creator. The contact sub-fields are single Text values that may hold several entries | [IPTC §7.7, §7.8, §8.1] |
| **Creator ↔ Image Creator name (PLUS):** synchronisation is offered | [IPTC §7.6 note 1] |
| **Cardinalities:**<br>- Image Creator, Copyright Owner, Licensor: at most 3;<br>- Image Supplier: 1;<br>- Location Created: one in the UI (all are kept when reading) | [IPTC §11.9, 11.18, 11.22, 11.24, 11.26] |
| **Clearing a field** removes it consistently from every representation PhotoWorkflow may write:<br>- XMP (embedded or sidecar);<br>- IIM;<br>- the Exif overlap field in JPEG/TIFF/DNG.<br>**A manufacturer RAW file is never changed** (Q2) | [MWG §4.2.3.1]; [Product] |
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
   - the file is not open in PhotoWorkflow's own RAW engine (sessions for it are closed first).
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

### 9.3 Verify the temporary copy

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
- **Ingest:** metadata is only written to the *copied* file, after its copy has been verified
  (ADR 0005), with this procedure.
- **Tests** (to be written with the implementation; none yet):
  - fault injection at each step (stop ExifTool, full disk, locked file, failing replace);
  - camera DNGs and **Xiaomi 17 Ultra DNGs** (preview at the end of the file), checked with
    ImageDataHash, previews, LibRaw and a byte comparison of all non-metadata ranges.

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
- **Template files are XMP.** The modes are stored in a PhotoWorkflow namespace that other tools
  ignore.

## 11. Validation (warnings, not refusals)

- **Lengths:** against the IIM byte limits. Alt Text: warning above 250 characters [IPTC §7.1].
- **Country Code:** ISO 3166-1, 2 or 3 letters, the scheme derived from the length; the code is
  authoritative over the country name [IPTC §7.5; User Guide].
- **Creator's Jobtitle** without Creator → warning [User Guide].
- **Dates:** the forms of IPTC §14.2; no time zone added (§7).
- **URIs:** for controlled-vocabulary properties (Digital Source Type: IPTC NewsCodes [IPTC
  §11.13]; PLUS values: Q9).
- **Image Registry Entry:** both sub-fields required [IPTC §11.21].
- **Rating:** −1 … 5; real numbers allowed; out-of-range values shown as invalid and not changed
  (§8).
- **ExifTool version:** at least 12.67, checked at start [IPTC §5.1].

## 12. Tool and verification

- **ExifTool** remains the reader and writer, with explicit tag lists per format.
  - The MWG *module* of ExifTool is not used: its "strict" mode ignores non-standard locations.
    The rules above are implemented by PhotoWorkflow.
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
- **Level-C samples** (outside the repo) from the owner:
  - an ARW with sidecar, a JPEG and a DNG written by Lightroom, and the same written by Photo
    Mechanic;
  - one IIM-only JPEG edited by an old tool (checksum mismatch);
  - a Xiaomi 17 Ultra DNG.

## 13. Consistency check of the standards in this architecture

The three documents were compared for every rule above (details in the analysis §8–§10).

| Topic | IPTC 2025.1 | IPTC-MG 2023.1 | MWG 2.0 | This ADR | Result |
|---|---|---|---|---|---|
| Save IIM and XMP | "should … both" (§5.3.4) | – | IIM only if present or needed for compatibility | as MWG + setting | **Deviation from IPTC "should"**, deliberate (§5.2) |
| IIM/XMP precedence | none | – | matching or no checksum → XMP; differing → IIM per property | as MWG, but **no checksum → user decides** | **Deviation from MWG** (stricter), deliberate (§5.1) |
| Exif/XMP precedence | none | none | prefer Exif | XMP, conflicts shown | **Deviation from MWG**, deliberate (§6.2) |
| Creator → Artist | – | one name; also Photographer | all names, "; " | IPTC-MG for writing, MWG for reading | The standards disagree; the newer IPTC-MG is followed (§6.5) |
| Exif text encoding | – | Exif 3.0: UTF-8 type | UTF-8 in ASCII | by Exif version | Compatible |
| DateTimeOriginal | suggest Date Created from it | mapping both ways | export corrections to Exif | never changed automatically | Compatible (the mapping is not mandatory; corrections are a manual feature, Q12) |
| Time zone | TZD optional (§14.2) | IIM time with offset | never invent | never invent | Compatible, except IIM 2:60 without a known zone (Q3) |
| Rating | −1, 0…5, real | – | float, rounding for display | as both | Compatible |
| Hierarchical keywords | – | – | MUST preserve | preserved | Compatible |
| Deprecated IIM datasets | keep in sync (§9.1) | – | – | kept and synced | Compatible |
| DNG embedded / RAW sidecar | not covered | – | DNG not covered | product decision | No conflict |
| Clearing | – | – | update both forms or remove the other | consistent across forms, RAW untouched | Compatible; the RAW + camera-Exif case is Q2 |
| Lang Alt edits | Lang Alt type (§14.1) | – | §5.2, not yet checked on this point | edit `x-default` only; other entries kept | **To verify:** the Adobe XMP Toolkit also updates entries equal to the old `x-default`. If MWG §5.2 or ISO 16684-1 recommend that, this is a deliberate deviation (Q7) |

**No rule of this ADR contradicts a MUST of the three documents.** The deviations concern
SHOULD-level recommendations; each is marked [Product ≠ std] with its reason. The Lang Alt rule
(§8, Q7) is still to be checked against MWG §5.2 and ISO 16684-1.

## 14. Open questions (must be answered before approval)

| # | Question | Proposal |
|---|---|---|
| **Q1** | **Writing a file with an unresolved IIM/XMP conflict** (no-checksum case). Writing both forms and the checksum would decide the conflict silently in favour of the written value | Writing other fields is allowed. For each conflicting property both forms are left as they are **and no new checksum is written**, until the user decides. MWG requires the checksum "whether changed or not", so this is a deliberate exception, limited to unresolved conflicts. Batch operations list such files |
| **Q2** | **Clearing a field for an ARW/RW2 whose embedded camera Exif has a value** (e.g. Artist set in the camera). After clearing, the camera value would fill the gap again (§6.2) | The sidecar records the cleared property in the PhotoWorkflow namespace (`pwf:Cleared`, a Bag of property ids), which suppresses the camera value. Other tools ignore it and will show the camera value |
| **Q3** | **IIM 2:60 Time Created without a known time zone.** IIM's time format includes an offset; writing `+0000` would invent one | Write only 2:55 (the date) and leave 2:60 out until the zone is known. To be confirmed against the IIM 4.2 specification (not read) |
| ~~**Q4**~~ | **Exif overlap fields inside DNG:** the decision lists JPEG/TIFF for synchronisation | **Resolved 2026-11-08:** Treat DNG like TIFF: overlap fields that exist are updated (MWG requires updating or removing the other form) |
| ~~**Q5**~~ | **A foreign `<basename>.xmp` next to a DNG** (written by another tool) | **Resolved 2026-11-08:** Never written and never used as a source; the info panel shows a notice that it exists and is ignored |
| ~~**Q6**~~ | **Creating missing Exif overlap fields** in JPEG/TIFF | **Resolved 2026-11-08:** Do not create missing fields in JPEG, TIFF or DNG (DNG is handled like TIFF, Q4); synchronise only existing overlap fields |
| ~~**Q7**~~ | Exact Lang Alt rules (`x-default` alongside a language entry) | **Resolved 2026-11-08 (product policy):** edit only `x-default`; leave all other language entries and their values unchanged. If `x-default` is absent, add it first. Clearing removes the whole property. Adobe's SDK also synchronises entries matching the old default ([`SetLocalizedText`](https://github.com/adobe/XMP-Toolkit-SDK/blob/v2025.03/XMPCore/source/XMPMeta-GetSet.cpp), SDK v2025.03); PhotoWorkflow deliberately does not. The full ISO 16684-1 text was not accessible during review, so verify this policy against the standard before implementation. |
| **Q8** | Normative sidecar naming (`name.xmp` vs `name.ARW.xmp`) and RAW+JPEG pairs | Read Adobe XMP Part 3; check Lightroom and Photo Mechanic samples |
| **Q9** | Allowed URI values of PLUS properties (release status, minor model age, data mining) | Read the PLUS specification |
| **Q10** | Fallback encoding for IIM/Exif text that is neither ASCII nor UTF-8 | Windows-1252 by default, changeable in the settings; the value is marked as converted |
| **Q11** | Test files | The level-C samples of §12 |
| **Q12** | A manual "correct camera time" feature (MWG: such corrections SHOULD be exported to Exif) | Out of scope now; would be the only way DateTimeOriginal ever changes, and only on the user's explicit command |
| **Q13** | "New IIM only when backward compatibility requires it": how is that detected besides the setting? | Only the setting (and possibly a future export profile); no automatic detection |

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
3. **Answer the remaining open questions** (Q1–Q3 and Q8–Q13; proposals in §14), then **review and approve this ADR**.
4. Read **XMP Part 1** (ISO 16684-1) and **MWG §5.2** to verify the Q7 policy, **XMP Part 3** (Q8), **IIM 4.2** (Q3) and
   **PLUS** (Q9) if the owner provides them.
5. **Level-C samples** (Q11).

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
