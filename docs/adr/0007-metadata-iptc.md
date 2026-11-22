# ADR 0007: Metadata handling according to the IPTC standards

- Status: **accepted by the owner (2026-11-11, revision 9).** All open questions are answered
  (§14); what is left after milestone 8 is in §18.
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
  - **Implemented (2026-11-14):** the steps of `docs/tasks/plans/adr-0007-milestone-8-steps.md`
    and their tasks: I1 T76, I2 T77, I3 T78, I4 T79, I5 T80–T81, I6 T82, I7 T83, I8 T84–T86,
    I9 T87–T93, I10 T94–T108, I11 T128 (with ADR 0017 U10d), I12 T127 (with T129, T130, T132),
    I13 T133; the owner's decisions D1–D16 are in §17.
  - **Amended by ADR 0020 (owner, 2026-11-17):** a manual correction of the capture time, the only
    way DateTimeOriginal changes (§6.3, §7, Q12, rule 4).
  - **Amended by ADR 0024 (owner, 2026-11-17):** a rotation writes `tiff:Orientation` (§6.3, rule 4).
  - **Amended by the owner (2026-11-18, revision 19):** Q2 changed: a cleared field of a
    manufacturer RAW file is not marked, and the camera's value fills it again (§6.4). D1 is
    dropped (§17.1).
  - **Sources checked (2026-11-18, revision 20):** Q3 and Q8 confirmed by IIM 4.2 and XMP Part 3;
    the Lang Alt rule (Q7) is a deliberate deviation from a "should" of XMP Part 1 (§8, §13).
  - **Amended by the owner (2026-11-18, revision 22):** camera GPS fills the coordinates of
    Location Created (§6.7); Scene Code, Subject Code and Genre are checked against IPTC's
    NewsCodes (§11).
  - **Amended by the owner (2026-11-18, revision 21):** Image Editor ↔ Contributor (§6.6); a list
    whose items have qualifiers is not written (§8).
  - **Amended by the owner (2026-11-19, revision 26)**, from the test with DxO PhotoLab 10.3
    (ADR 0029 step P0): a written file gets a new modification time (§9.4, Q14); a flat
    `lr:hierarchicalSubject` is kept equal to the keywords (§8, Q15); existing Exif and XMP
    rating percentages are kept equal to the rating (§6.1, §6.3, Q16). Two tasks carry the three
    (§18; T204, T205, split by the owner 2026-11-19).
  - **Amended by the owner (2026-11-22, revision 29):** a faster safe write without a weaker
    check, from the analysis `docs/analyses/2026-11-21-metadata-write-time.md` and its
    measurement T261; the owner chose three of its four directions and approved Q17 (a narrow
    final read), Q18 (a batch written by several ExifTool processes at once) and Q19 (the DNG image
    check, the digest pass and the TIFF restart measured before they are decided again), all as
    proposed (§9.6, §14). Tasks: §18.
  - **Amended by the owner (2026-11-22, revision 30):** Q19 decided after T264, T265 and
    T269, all as proposed: ExifTool 13.64 or newer required (Q20), the digest pass replaced by a check of the
    copy's checksum (Q21), the TIFF restart dropped (Q22), the DNG image check kept (Q23)
    (§9.7, §14); the unpack measured further (T273). Tasks: §18.
  - **Amended by the owner (2026-11-22, revision 31):** D8's allowance dropped (§9.7 point 5,
    §17.1); task T274.
  - **Amended by the owner (2026-11-22, revision 32):** Q23 decided again after T273: LibRaw's
    open and unpack of a DNG copy run only when the original has no `ImageDataHash` (§9.3 point
    4, §9.7 point 4); task T275.
  - **Amended by the owner (2026-11-22, revision 33):** with ADR 0017 U13 (§6.5), a template is
    a plain XMP file with only standard values, each field has a check box, and a checked field
    replaces the image's value; the modes Keep, Append, Prefix and Clear and the RawStride
    namespace of §10 are dropped, and the import dialog's modes (D13) go with U13 (§10, §17.1).
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
| **[XMP-1]** | Adobe XMP Specification Part 1 (April 2012), the text of ISO 16684-1:2011 (section given; the 2019 edition was not read) |
| **[XMP-3]** | Adobe XMP Specification Part 3, Storage in Files (January 2020) |
| **[IIM 4.2]** | IPTC-NAA Information Interchange Model 4.2 (July 2014; dataset given) |
| **[Product]** | RawStride's own decision (owner's decision of 2026-11-04 unless said otherwise) |
| **[Product ≠ std]** | A product decision that deliberately departs from a recommendation of a standard. The reason is given |

## 2. Decision summary

| # | Decision | Basis |
|---|---|---|
| 1 | **XMP is RawStride's primary representation** for the explicitly mapped descriptive fields | [Product]; IPTC Core/Extension are defined in XMP [IPTC §1] |
| 2 | **IIM vs XMP:**<br>- checksum matches → XMP;<br>- checksum differs → IIM per differing field;<br>- no checksum → conflict, the user decides;<br>- conflicts always visible | [MWG §4.2.3.2] for the first two; [Product ≠ std] for the third |
| 3 | **IIM is preserved and written back when it exists.** New IIM only with the setting | [MWG §4.2.3.2]; [Product ≠ std] against [IPTC §5.3.4] "should save … both" |
| 4 | **Exif vs XMP:**<br>- XMP wins;<br>- conflicts detected and shown;<br>- JPEG/TIFF/DNG synchronise the corresponding existing Exif fields;<br>- RAW originals are never changed (Exif conflicts there are only shown; a corrected capture time or a rotation in the sidecar is not a conflict, ADR 0020, ADR 0024) | [Product ≠ std] against [MWG §4.2.3.1] "prefer Exif"; sync per [MWG §4.2.3.1] and [IPTC-MG] |
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
  - **Confirmed by [XMP-3] (2026-11-18):** "look in the same directory for a file with the same
    name as the main document but with an .xmp extension"; for MPEG-2 it says "with the file
    extension .xmp replacing the original file extension". `<basename>.<ext>.xmp` is not
    mentioned. [XMP-3] says nothing about RAW+JPEG pairs.
  - **Confirmed for Photo Mechanic (T127, 2026-11-14):** Photo Mechanic 2026.6 names the sidecar
    `<basename>.XMP` with an upper-case extension (`PM-raw.XMP`); RawStride finds it as
    `<basename>.xmp` and reports no foreign sidecar. Until T132 a write by RawStride renamed it to
    `PM-raw.xmp` (lower case); since T132 a write keeps an existing sidecar's name as it is on
    disk (`PM-raw.XMP`), and only a new sidecar is named `<basename>.xmp`.
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
  - **TIFF and DNG:** the checksum is written in the same call as the IIM, as in a JPEG (owner,
    2026-11-22, §9.7 Q21; T272): ExifTool 13.64, the lowest version, stores a current one there
    (T269). §9.3 refuses a copy whose stored checksum is not current. Until T272 a second call
    wrote it, because ExifTool 13.59 stored an out-of-date checksum in the IIM's call (T92).
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
| Time Created (2:60) | `hhmmss±hhmm`, no sub-seconds. **Only written when the offset is known** (§7). Otherwise only 2:55 is written and 2:60 is left out; the time stays in XMP, zone-less **(Q3, owner 2026-11-11)** | [IPTC-MG Exif Note on Date Created]; [IIM 4.2 2:60]: always 11 octets `HHMMSS±HHMM`, no form without an offset; the dataset is not mandatory |
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
| Contributor with the role "Content Editor" | ImageEditor 0xA438 (Exif 3.0) | both ways (§6.6) | [IPTC-MG Exif Note on Contributor] |
| Date Created | DateTimeOriginal + SubSecTimeOriginal + OffsetTimeOriginal | **Exif → IPTC only** (§7) | [IPTC-MG]; [Product] |
| Digital Image GUID | ImageUniqueID 0xA420 | **Exif → IPTC only**; the IPTC value is then not changed | [IPTC-MG Exif Note on Digital Image GUID] |
| Location Created | GPS tags 1–6 | Exif → IPTC, only when recorded by the camera; the coordinates only (§6.7) | [IPTC-MG]; [MWG §5.8.5] |
| Location Shown | — | **never mapped from GPS** | [IPTC-MG Exif Note on Location Shown] |
| Rating (`xmp:Rating`; not an IPTC property, ADR 0004) | Rating 0x4746 and RatingPercent 0x4749 (Microsoft, IFD0); also a non-standard `xmp:RatingPercent` | **XMP → Exif only**, existing fields only (§6.3, Q16, owner 2026-11-19, revision 26) | [Product]; [MWG §4.2.3.1] |
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
  `tiff:`, and no Exif checksum is written to XMP. [MWG §4.2.3.1] Exception: `tiff:Orientation`,
  written by a rotation (ADR 0024, owner 2026-11-17).
- **DateTimeOriginal is never written** (§7), except by the manual correction of ADR 0020
  (owner, 2026-11-17).
- **Rating percentages (Q16, owner 2026-11-19, revision 26):** when RawStride writes a rating, existing
  Exif Rating and RatingPercent and an existing `xmp:RatingPercent` are written to match: stars
  0–5 as Rating 0–5 and RatingPercent 0, 1, 25, 50, 75, 99; a reject (−1) removes the three, since
  they have no reject value. None of them is created, and none is read as a source: `xmp:Rating`
  stays the only rating RawStride reads. Reason: DxO PhotoLab adds all three when it writes a DNG
  (ADR 0029 P0, 2026-11-19), and without this they would keep an old rating beside a new one.
  `xmp:RatingPercent` is kept equal in a manufacturer RAW's sidecar too (owner, 2026-11-19): DxO
  PhotoLab adds it there as well; the RAW file itself is never written (§4.2).

### 6.4 Manufacturer RAW (ARW, RW2, …)

- **Embedded Exif is preserved and never written:** the original is never changed.
- **When embedded Exif differs** from the sidecar's XMP (for example, Artist or Copyright set in
  the camera), the difference is **shown as a conflict**. XMP has precedence.
- **A field cleared in the sidecar is filled again from the camera's Exif** when the camera wrote
  a value, for example Artist or Copyright set in the camera **(Q2, owner 2026-11-11, changed
  2026-11-18):**
  - clearing removes the property from the sidecar (§8); nothing records that it was cleared;
  - the next read fills the property from the embedded Exif as in §6.2, and the info panel shows
    it as the camera's value;
  - why: a marker would only work inside RawStride, since other programs reading the sidecar show
    the camera's value anyway; it would concern only the few fields a camera writes; and it would
    need an XMP namespace of RawStride's own, which stays in every sidecar once written. To keep a
    camera value out of the images, change the setting in the camera.
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

### 6.6 Image Editor ↔ Contributor (owner, 2026-11-18)

- **The content editor** is the first Contributor whose Role holds the URI
  `https://cv.iptc.org/newscodes/contentprodpartyrole/contentEditor`; the same URI with `http://`
  counts too. A Contributor RawStride creates gets the `https://` form. [IPTC-MG Exif Note on
  Contributor]; [Product] for `http://`
- **Reading:**
  - No Contributor at all: Exif ImageEditor fills it with one Contributor, Name (`x-default`) =
    ImageEditor, Role = the URI above. [IPTC-MG]
  - A Contributor without a content editor, or one whose shown Name differs from ImageEditor: a
    conflict, XMP wins (§6.2). The Exif side of the conflict is the same list with the content
    editor added at the end or its Name replaced, so the other Contributors are kept whichever
    side the user chooses. [Product]
  - Manufacturer RAW: as every Exif field, camera data (§6.4).
- **Writing (JPEG, TIFF, DNG):** when Contributor changes, the content editor's shown Name is
  written to ImageEditor if the tag exists (§6.3: never created); without a content editor
  ImageEditor is removed. It is never written to Artist. [IPTC-MG Exif Note on Contributor];
  [Product] for not creating

### 6.7 Camera GPS → Location Created (owner, 2026-11-18)

- **What is mapped:** only the coordinates. GPS tags 1–6 (latitude, longitude, altitude and their
  references) fill the sub-fields GPS Latitude, GPS Longitude and GPS Altitude (with GPS Altitude
  Ref) of Location Created. [MWG §5.8.5]; [IPTC-MG]
- **Reading:** as every Exif field (§6.2): the camera's coordinates fill Location Created when it
  has no coordinates; a Location Created with other coordinates is a conflict, XMP wins. The
  other sub-fields (city, country …) are never filled from a position. Two positions are the same
  when latitude and longitude each differ by at most 0.0001° (T196); the altitude is not
  compared, and is added only together with the coordinates. Any Exif GPS counts as the
  camera's: the file does not say who wrote it. [Product]
- **Which entry, and its origin** (owner, 2026-11-18; written in 2026-11-19, revision 25): only the
  first entry of Location Created is looked at (§8: one location in the window). An absent or
  empty Location Created is filled with one entry of the coordinates, origin Exif. A first entry
  without coordinates gets them added, its other sub-fields and the other entries unchanged, and
  the property keeps its origin: no conflict, no notice. Its altitude is added only when it has
  neither GPS Altitude nor GPS Altitude Ref. A Location Created the model refused is not
  replaced by one built from Exif (reviewer's choice, T196). Exif GPS that cannot be used (a
  latitude or longitude without a valid reference, out of range, signed or not a number) gives a
  notice that the value could not be read. [Product]
- **Writing:** never (§6.3, "Exif → IPTC only").
- **Not here:** filling city, country and location from a position (reverse geocoding), a GPX
  track and a map need network or new dependencies and belong to the GPS ADR (ADR 0017 §7.2).
  [Product]

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
| **IIM Time Created (2:60) is left out when the zone is unknown**; only the date (2:55) is written | [Product], following MWG; confirmed by [IIM 4.2 2:60] (2:60 has no form without an offset and is not mandatory; Q3) |
| A manual "correct camera time" feature is ADR 0020 (owner, 2026-11-17; out of scope until then). It is the only way DateTimeOriginal ever changes, and only on the user's explicit command. For manufacturer RAW the corrected time is the sidecar's `exif:DateTimeOriginal`, which is read before the embedded value and is not a conflict against it | [Product] (Q12); MWG says such corrections SHOULD be exported to Exif |

## 8. Other rules

| Rule | Basis |
|---|---|
| **Hierarchical keywords** (`mwg-kw:Keywords`, `lr:hierarchicalSubject`) are kept **unchanged**, including all nested information, even though RawStride cannot edit them yet. **Exception (Q15, owner 2026-11-19, revision 26):** an `lr:hierarchicalSubject` whose every entry is flat (no `\|`) is written equal to the keywords (`dc:subject`) whenever RawStride writes the keywords; one entry with a `\|` keeps the whole property unchanged as before. Reason: DxO PhotoLab writes the keywords into `lr:hierarchicalSubject` as flat entries and reads them back from there, so a keyword renamed in RawStride came back (ADR 0029 P0, 2026-11-19) | [MWG §5.10]: "MUST preserve all nested information" (a flat entry has none); [Product] for `lr:` |
| **Keywords may be phrases** and are never split at spaces | [IPTC §7.18] |
| Keyword lists are rewritten as a whole in IIM; XMP qualifiers on keywords are kept | [MWG §5.1] |
| **A list with qualifiers is not written** (owner, 2026-11-18): when an item of a list property (`dc:subject` and every other XMP list RawStride writes as a whole) in the primary XMP has a qualifier other than `xml:lang`, RawStride shows the list as usual with a notice, and never writes that property to XMP; a change to it is left out of the write with a write notice, and the other properties are written. Reason: ExifTool 13.59 cannot change or remove a qualified item (rewriting the list keeps it and adds the text again as a plain item; `-=` and clearing leave it; checked 2026-11-18). Editing such a list with its qualifiers kept would need RawStride to edit the XMP packet itself (`docs/tasks/backlog.md`) | [MWG §5.1] "SHOULD be preserved"; [Product] |
| **Rating:** stored internally with full precision (a real number). Rounded only for display (out-of-range values shown as the nearest of −1…5). Written only when the user changes it. 0 = unrated; a missing value counts as 0; −1 = rejected | [IPTC §11.19]; [MWG §5.5] |
| **Lang Alt:**<br>- RawStride edits `x-default` only and preserves all other language entries, including one that held the same value as the old `x-default`;<br>- when `x-default` is absent, it is added first;<br>- **reading** without `x-default`: the first entry is shown, marked with its language;<br>- clearing removes the whole property, all languages included | [IPTC §14.1]; [Product]; `x-default` first: [XMP-1 §8.2.2.4] "shall be first in the array" (ExifTool inserts it first, T83); reading the first entry: [XMP-1 §8.2.2.4] "a default value, if known, should be the first array item"; [MWG §5.2] discusses only `x-default`, other languages MAY be supported. **[Product ≠ std]** against [XMP-1 §8.2.2.4] "its simple text value should be repeated in another item in which xml:lang specifies its actual language": after an edit, an entry that held the old `x-default` text keeps it. Reason: RawStride does not know the language of the typed text, and writing it into such an entry could give it the wrong language. A Lang Alt with `x-default` alone, the common case, is allowed (Q7, §13, checked 2026-11-18) |
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
- **One call** (owner, 2026-11-22, §9.7 Q21; T272): the copy is written with one ExifTool call,
  also when the write includes a new IIM checksum (§5.2). The checksum pass of T92 (a second call,
  `-IPTCDigest=new -overwrite_original <copy>`, for TIFF and DNG) is dropped; §9.3 point 2 checks
  that the stored checksum is current instead.
- **Known harmless write warnings (owner 2026-11-19, D17; 2026-11-20, D18):** a write answered
  only with warnings on the named list (today `[minor] Error reading HiddenData` and
  `[minor] Fixed incorrect HiddenDataOffset`) counts as written. Every other warning still fails
  the write.

### 9.3 Verify the temporary copy

**No restart before reading a TIFF copy** **(owner 2026-11-22, §9.7 Q22; T272)**: the process
that wrote the copy also reads it. The restart of T92 ("choice A", owner 2026-11-12) is dropped:
with ExifTool 13.59 a process that had written the checksum to a TIFF whose Photoshop block holds
a checksum followed by an `AgHg` resource read such TIFFs wrongly until it ended; ExifTool 13.64,
the lowest version, reads them correctly (T269). A misread would show as new warnings, and the
copy would be refused (point 3).

1. **Image data unchanged:** `ImageDataHash` equals the original's.
2. **Metadata as intended:**
   - the full listing differs from the original only in the properties written, their IIM and
     Exif representations, the IIM checksum, and `xmp:MetadataDate` / `xmp:ModifyDate`;
   - anything else that changed is an error;
   - when the write includes a new IIM checksum, the copy's stored checksum
     (`Photoshop:IPTCDigest`) is present and equals the one computed from its IIM block
     (`File:CurrentIPTCDigest`), for every file type (owner, 2026-11-22, §9.7 Q21; T272).
3. **Structure intact:**
   - ExifTool reads the file without errors or warnings about the structure;
   - pw-raw finds the same previews (count, sizes, decodable JPEG);
   - DNG: `DNGVersion`, `NewSubfileType` of the main image and the IFD chain unchanged.
4. **The RAW image still opens:** for DNG, LibRaw opens and unpacks the copy without a "damaged"
   report (the M2 detection). This catches truncated or moved image data. **Only when the
   original has no `ImageDataHash`** (owner, 2026-11-22, §9.7 point 4, Q23; T275): when it has
   one, point 1 has compared it, and LibRaw is not run (point 3's preview comparison still is).
5. **Flush:** the temporary file is flushed to disk (`FlushFileBuffers`).

### 9.4 Replace atomically

- **`ReplaceFileW`** (same volume) replaces the original with the verified copy and keeps the
  original as a backup `<name>.pwbak-<random>` in the same step. It also keeps the original's
  creation time, attributes, ACL and alternate data streams.
- **The modification time** is the time of the write, for a sidecar and for an image written in
  place alike (Q14, owner 2026-11-19; revision 26). It is no longer set back to the original's,
  as revisions 1–25 and ADR 0004 had it. Reason: other programs, backup tools and the catalog of
  ADR 0030 find a changed file by its modification time (and size). With the old time, DxO
  PhotoLab with metadata synchronisation on ignored RawStride's new keywords in a sidecar and in a
  DNG (ADR 0029 P0, 2026-11-19), and an incremental backup could miss a change that kept the
  size. The creation time, attributes, ACL and alternate data streams are still kept.
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

### 9.6 Speed (owner, 2026-11-22, revision 29)

**Background:** the owner finds that writing metadata takes long (2026-11-21). Measured in a
release build with a warm cache (T261, analysis of 2026-11-21): one edit costs about 0.65 s for a
13.5 MB camera JPEG, 0.48 s for a 12 MB DNG (0.26 s of it the image check, §9.3 point 4), 1.2 s
for a 36 MB DNG, 3.8 s for a 177 MB TIFF and 0.06 s for a RAW with a sidecar. The three snapshots
are more than half of an image file's write, almost all of it `ImageDataHash`; a batch costs the
sum of its files, since all writes go through one ExifTool process. **The owner, 2026-11-22:** make
the write faster without weakening what §9 proves; three directions are taken up (Q17–Q19), the
fourth of the analysis (a cheaper final check that no longer proves the image data) is not.

1. **A narrow final read (Q17).** The final read after the replace (§9.4) compares only the size
   and `ImageDataHash` with the verified copy's, but today it takes a full snapshot (`-all`,
   `-validate`). It asks ExifTool for `ImageDataHash` alone (SHA-256, as today). The check is the
   same. **Estimated saving:** the `-all` and `-validate` part of one snapshot of three, about
   80–110 ms per JPEG, about 30 ms per DNG, little for a TIFF (whose snapshot is almost all hash).
2. **A batch written by several ExifTool processes at once (Q18).** Writes of several files (the
   ingest template, an edit or a rating applied to several images, a rotation of several) are
   shared among up to **four** ExifTool processes, each in its own thread; the number is fixed
   after a measurement with 1, 2 and 4 processes on a batch of camera JPEGs and DNGs, and may
   become a setting later. **The owner, 2026-11-22 (after T267):** the number is **four**. T267
   measured a batch of 27 camera JPEGs and DNGs written 3.5 times as fast with four processes as
   with one (warm; about 1.96 times with two), on an NVMe and a SATA SSD alike
   (`docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T267"); a memory card, a
   network share, a hard disk and the app at work were not measured. T268 makes the app and the
   import's template use it. Rules:
   - every file is still its own transaction with every check of §9.1–§9.5;
   - two writes to the same target (an image, or a sidecar) are never made at the same time, and
     the writes to one target keep the order they were asked in. This holds within one pool of
     processes; the metadata worker's pool and the ingest template's pool can still meet on one
     target, as the two single processes can before Q18. **The owner, 2026-11-22 (T266–T268's
     chain review):** that is a known limit (`docs/tasks/known-limits.md`, recorded when T268 is
     done), not a rule change;
   - the journal (§9.5) holds several open entries at once, each written whole, and the recovery
     at start is unchanged;
   - the RAW engine's sessions for an image are released before its write, as today (§9.1);
   - the TIFF restart (§9.3) restarted only the process that wrote that TIFF (dropped by §9.7
     Q22, T272);
   - the answers may come back in another order than the files were asked; each names its file
     (§9.5), and a batch's summary is made when all have answered;
   - one file alone is not faster. The gain is largest for many files on a fast disk, and less on
     a card or a network share.
3. **The DNG image check, the digest pass and the TIFF restart measured first (Q19).** No rule
   changes now. A measurement task times, in a release build: §9.3 point 4 split into LibRaw's
   open and its unpack, and whether a cheaper form still refuses the damaged copies of
   `pw-imagecheck`'s `tests/real_files.rs` (a copy cut inside its RAW data, a copy with one byte
   changed in a preview); and whether the measured reasons for the digest pass and the TIFF
   restart (§9.2, §9.3, T92) still hold with the ExifTool in use. The owner then decides whether
   any of the three changes, in a later revision.

**Order (approved by the owner 2026-11-22):** Q17 first (small, the check unchanged), then Q19's
measurement, then Q18 (the largest gain for a batch, and the most work). Until each is done,
§9.1–§9.5 apply as written.
### 9.7 Q19 decided after the measurements (owner, 2026-11-22, revision 30)

**Proposed by the reviewer and approved by the owner 2026-11-22, all as proposed (Q20–Q23).**
Until T271 (point 1) and T272 (points 2 and 3) are done, §5.2, §9.2, §9.3 and §11 apply as
written; the reviewer changes them at those tasks' Done.

**Background:** Q19 measured three steps of the safe write (§9.6 point 3), in
`docs/analyses/2026-11-21-metadata-write-time.md`:
- **T265 (ExifTool 13.59):** both reasons from T92 still hold. In a TIFF or DNG, `-IPTCDigest=new`
  in the IIM write's own call stores an out-of-date checksum, so the digest pass is needed
  (39–438 ms). A process that has written a TIFF with an `AgHg` resource then reads such TIFFs
  with two extra warnings, so the restart is needed (about 190 ms). Together they are 17–24 % of a
  TIFF write; the digest pass is 8–11 % of a DNG write.
- **T269 (ExifTool 13.64, 2026-11-16):** both reasons are gone. The one call gave a current
  checksum in all 3 TIFFs and all 7 DNGs, in every run, and the writing process read every TIFF
  the same as a new process. The owner's repro, reported to ExifTool's author, agrees. The safe
  write as it is, the workspace tests and the five real-file tests all pass with 13.64. 13.64's
  change notes name both fixes, and a third: new `PhotoshopSettings` in a TIFF are written with a
  proper format code (the cause of D8's extra warning).
- **T264 (the DNG image check, §9.3 point 4):** LibRaw's unpack is 96–99 % of the check
  (249–553 ms per DNG). The `ImageDataHash` comparison refused every cut copy, and only the
  preview comparison refused a changed preview byte. LibRaw's open alone refused none of the
  damaged copies. Other kinds of damage (a wrong tile offset, a broken structure that the hash and
  the listing do not show) were not measured.

1. **ExifTool 13.64 or newer is required (Q20).** `MIN_EXIFTOOL_VERSION` becomes 13.64 (12.67
   before; done by T271 2026-11-22, §11). The shipped copy (ADR 0002 §2.5,
   `docs/distribution.md`) and CI's pinned version become 13.64, each checked by SHA-256 as today. ADR 0002's rule holds: a version is shipped only
   after the real-file tests of `pw-metadata` and `pw-imagecheck` pass with it (T269 ran those of
   `pw-metadata`; those of `pw-imagecheck` are run in the task). An older ExifTool on PATH is
   treated as D9 says: reading works, every write is refused with a clear text. One safe write
   for every version, so CI tests the only path there is.
   - **Alternative:** keep 12.67 and choose by version (the digest pass and the restart only below
     13.64). Two paths, and CI tests only one of them.
   - **Note:** 13.64 is a development release; ExifTool's newest production release, 13.55, has
     neither fix. RawStride already ships a development release (13.59).
2. **The digest pass is dropped, and the checksum is checked instead (Q21).** The checksum is
   written in the same call as the IIM (§5.2, §9.2). A new check is added to §9.3 point 2: when the
   plan writes `-IPTCDigest=new`, the copy's `Photoshop:IPTCDigest` must equal its
   `File:CurrentIPTCDigest`; otherwise the copy is refused like any other failed check. Both keys
   are already in the copy's snapshot, so the check costs nothing. Today an out-of-date checksum
   would not be noticed at all; with the check it can never be stored silently.
   - **Alternative:** run the digest pass only when the check finds the checksum out of date, then
     read the copy again. The write then succeeds in a case 13.64 might still get wrong (a TIFF
     without IIM before the write, a DNG with IIM before it: not measured), at the cost of one
     more path.
3. **The TIFF restart is dropped (Q22).** The process is no longer restarted after a TIFF write.
   This is safe even if a misread comes back in some file: the misread shows as new warnings, and
   §9.3 refuses the copy. A file is never written wrongly; at worst a write fails with "could not be
   written safely". `ExifTool::restart` itself stays (its own tests and the T265 example use it; the
   worker and the pool start new processes with `ExifTool::start_with`).
4. **The DNG image check is kept as it is (Q23).** The owner's rule for §9.6 is "faster without a
   weaker check". LibRaw's unpack is the only check of the RAW data's structure, and the damage
   that only it would find was not measured. It stays until a measurement shows that the hash and
   the preview comparison refuse those kinds of damage too. **The owner, 2026-11-22:** that
   measurement is made (T273); whether the unpack is dropped is decided after it.
   - **Alternative:** drop the unpack and keep LibRaw's open, the preview comparison and the
     `ImageDataHash` comparison. Saves 250–550 ms per DNG (about half of a DNG write), with the
     unmeasured risk above.
   - **Decided again by the owner 2026-11-22 (after T273, revision 32): LibRaw runs only when
     the original has no `ImageDataHash`** (§9.3 point 4). T273 (the analysis
     `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T273") made seven kinds of
     DNG damage in seven DNGs: the snapshot comparison (points 1–3 of §9.3) refused all 42
     copies, the image check with LibRaw 11, each of them also refused by the snapshot
     comparison; no copy was refused only by the unpack, and the unpack passed 31 of the 42. When
     the original has a hash, the copy's equal hash covers the RAW data's bytes, the listing the
     tags that say how to read them, and the validation the structure; LibRaw's open is not kept
     on its own (`pw-libraw` opens and unpacks in one call; in T273 the open refused only a
     broken link, which the snapshot comparison also refused). When the original has no hash,
     nothing else checks the RAW data, so the open and unpack run as before. Not measured, and
     accepted: damage that neither the hash, the listing nor the validation shows (for example
     inside a binary block of the same length). Saves 250–550 ms per DNG with a hash (every DNG
     in `D:\PhotoWorkflowData\RAW` that T273 used). Task T275.
5. **D8 is kept for now.** With 13.64, a TIFF or DNG that gets a new Photoshop block may no longer
   get D8's extra warning. The task for point 1 records whether it still occurs; if not, removing
   D8's allowance is a later decision (in `docs/tasks/backlog.md`).
   - **Decided by the owner 2026-11-22 (after T270 point 6): D8's allowance is dropped** (§17.1).
     T270 found that 13.64 no longer writes the warning (a Xiaomi DNG: 8 warnings with 13.59, 7
     with 13.64; the writer differs, not the reader). A copy that gets one more validation warning
     is then refused in every case (§9.3). T274 removes the allowance, with every real-file test
     run with 13.64 as the gate: if a file is refused, the task stops and the owner decides again.

**Saving:** points 2 and 3 together about 17–24 % of a TIFF write (310–630 ms) and 8–11 % of a DNG
write (40–110 ms); nothing for a JPEG or a sidecar.

**Tasks** (§18, written 2026-11-22): the chain T270 → T271 → T272 and the measurement T273.
T270 ships and pins 13.64 (CI, packaging, notices, documents; the real-file tests of
`pw-metadata` and `pw-imagecheck` with 13.64; D8's warning recorded). T271 makes 13.64 the
lowest version (`MIN_EXIFTOOL_VERSION` and the texts and tests that name it). T272 drops the
digest pass and the restart and adds the checksum check in `pw-metadata`. T273 measures which
checks refuse other kinds of DNG damage (point 4). Before T271 the owner installs 13.64 in a
folder of its own, first on PATH, and keeps the 13.59 folder (T270 compares the two). T267 runs
whole before T270 (its timings with the digest pass, as measured so far). An undo goes in
reverse order: T272 before T271 (with T272 and an ExifTool below 13.64, every TIFF and DNG write
with a new checksum would be refused).

## 10. Templates (ingest and batch)

**Changed by the owner (2026-11-22, revision 33; ADR 0017 §6.5, U13-Q1–U13-Q11):** a template
holds only standard XMP and has no modes. Until then each field had one of the modes Keep,
Replace, Append (with Prefix) and Clear, kept in a RawStride namespace of the template file; the
import dialog's modes (D13) were the part of that built. [Product]

- **Template files are plain XMP** with only standard values: nothing of RawStride's own, no
  namespace and no modes, so every program reads the file in the same way.
- **Each field has a check box** in the template window. Only the checked fields are saved, and a
  field present in the file is a checked field when it is loaded. An unchecked field's text is
  not saved, so it never reaches another program.
- **A checked field replaces** the image's value:

  | Kind | What the template does |
  |---|---|
  | Single value | set |
  | List (Creator, Keywords, Person Shown, …) | replaced by the template's list |
  | Lang Alt | `x-default` set, other languages kept |

- An empty template field never clears anything, and a template clears nothing at all: a field
  is cleared only in the metadata window. The copyright-management fields (§8) are replaced as
  any other field and never cleared.
- **Structures:**
  - Location Created and Creator's Contact Info: a check box per sub-field; a checked sub-field
    replaces that sub-field and keeps the others;
  - a list of structures (Image Creator, Copyright Owner, Licensor, Location Shown, …) is
    replaced as a whole;
  - Image Creator, Copyright Owner and Licensor have at most 3 entries.
- **Variables** (`{date}`, `{year}` …; ADR 0017 U13-Q9) are kept as written in the template
  file and turned into each image's own values when it is written; the image never gets the
  variable itself.

## 11. Validation (warnings, not refusals)

- **Lengths:** against the IIM byte limits. Alt Text: warning above 250 characters [IPTC §7.1].
- **Country Code:** ISO 3166-1, 2 or 3 letters, the scheme derived from the length; the code is
  authoritative over the country name [IPTC §7.5; User Guide].
- **Creator's Jobtitle** without Creator → warning [User Guide].
- **Dates:** the forms of IPTC §14.2; no time zone added (§7).
- **URIs:** for controlled-vocabulary properties (Digital Source Type: IPTC NewsCodes [IPTC
  §11.13]).
- **The other IPTC NewsCodes (owner 2026-11-18, revision 22):** a warning, never a refusal; an
  unknown value is kept and shown, never removed (as the PLUS properties below).
  - **Scene Code:** each entry 6 digits and a code of IPTC Scene NewsCodes
    (`http://cv.iptc.org/newscodes/scene/`) [IPTC §7.21: "Required: IPTC Scene NewsCodes"];
  - **Subject Code:** each entry 8 digits; not checked against the vocabulary, as IPTC retired it
    before 2010 and points to Media Topics in CV-Term About Image [IPTC §7.23];
  - **Genre** (Extension): checked against IPTC Genre NewsCodes only when the entry's CV ID is
    that vocabulary (`http://cv.iptc.org/newscodes/genre/`, also with `https://` and with or
    without the trailing `/`); a term from another vocabulary is not checked, as Genre allows any
    controlled vocabulary [IPTC §11.17];
  - **Intellectual Genre** (legacy): not checked; IPTC names no required vocabulary [IPTC §7.16];
  - the Scene and Genre vocabularies are read and saved before implementation (§16 point 7).
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
- **ExifTool version:** at least 12.67, checked at start [IPTC §5.1]. RawStride writes only with
  13.64 or newer (owner, 2026-11-22, §9.7 Q20; T271); 12.67 is the standard's floor.

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

  **Where each test is** (from the rows of `docs/test-inventory.md`; paths under
  `crates/pw-metadata/` unless another crate is named; tests on `PW_TESTDATA` are ignored by
  default):

  | Test | Where |
  |---|---|
  | Round trip per format | JPEG: `tests/write_plan.rs`, `tests/safe_write.rs`, `tests/edits.rs`; TIFF: `tests/safe_write.rs` (a minimal TIFF; every TIFF in `PW_TESTDATA`), `tests/edit.rs` (`PW_TESTDATA`); DNG: `tests/safe_write.rs`, `tests/edit.rs` (`PW_TESTDATA`); manufacturer RAW with sidecar: `tests/safe_write.rs`, `tests/edit.rs`, `tests/edits.rs`; the level-C samples: `tests/level_c.rs` |
  | IIM = XMP after every write, and the checksum updated | `tests/write_plan.rs` (both forms changed, the two digests equal), `tests/edit.rs` (the checksum still `Matches` after a rating), `tests/template.rs` (Clear), `tests/edits.rs` (a cut City), `tests/safe_write.rs` (a TIFF, T92), `tests/level_c.rs`; `src/write_plan.rs` (both forms brought in line, the checksum written or not, T93) |
  | All three checksum cases of §5.1, including the predicted-IIM comparison | `src/reconcile.rs` (the three cases, equality on the predicted IIM value), `src/iim.rs` (the predicted IIM value); with ExifTool `tests/read.rs` |
  | Exif/XMP conflicts shown | `src/reconcile.rs` (Artist, ImageDescription, RAW Exif); shown: `crates/pw-loupe/src/info_panel.rs` (conflict rows with each value, its origin and the rule) |
  | RAW bytes unchanged (hash) | `tests/read.rs`, `tests/safe_write.rs`, `tests/edit.rs`, `tests/template.rs`, `tests/edits.rs` (a fake RAW), `tests/level_c.rs` (the ARW) |
  | DNG/JPEG/TIFF image data unchanged (ImageDataHash) | `tests/verify.rs`, `tests/safe_write.rs`, `tests/edit.rs`, `tests/level_c.rs`; `src/verify.rs` (`ImageDataChanged`) |
  | Hierarchical keywords, unknown namespaces, other languages and deprecated IIM datasets kept | hierarchical keywords: `tests/edits.rs` (`hierarchical_keywords_in_a_jpeg_are_kept_when_the_keywords_are_edited`, `hierarchical_keywords_in_a_sidecar_are_kept_when_the_title_is_edited`); unknown namespaces: `tests/level_c.rs` (`photomechanic:`, `xmpMM`); other languages: `tests/write_plan.rs`, `src/value.rs`, `src/template.rs`, `src/edit.rs`; deprecated IIM datasets: `src/write_plan.rs` (`NotEditable`, not brought in line), kept in a written file: `tests/edits.rs` (`deprecated_iim_datasets_are_kept_when_the_headline_is_edited`) |
  | Zone-less dates stay zone-less | `src/date.rs` (a zone never added through XMP, IIM and back), `src/write_plan.rs`, `tests/write_plan.rs` (no 2:60) |
  | Truncated dates | `src/date.rs`, `tests/write_plan.rs` (`2026-11` as `20261100`), `tests/read.rs` (`2026-11` read back) |
  | IIM prefixes and byte limits | `src/iim.rs`, `src/iim_read.rs` (prefixes removed when reading), `src/write_plan.rs` (`Truncated`, prefixes), `src/validate.rs` (`TooLongForIim`), `src/reconcile.rs` (over-limit values, T129), `tests/edits.rs` (a 40-byte City) |
  | The four template modes | `src/template.rs` (every mode), `tests/template.rs` (with ExifTool: `Iptc::to_template`, Append, Clear), `crates/pw-loupe/src/import.rs` (the modes of the import dialog, T100) |
  | Fault injection (§9.5) | `tests/safe_write.rs` (a failure in each stage, the original unchanged), `tests/edit.rs` (read-only and locked files), `src/fileops.rs` (a held original, a missing copy), `src/journal.rs` (recovery), `src/safe_write.rs` (refused ExifTool answers) |

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

  **Changed 2026-11-20 (owner, T234):** every sample except `PM-raw.ARW` was written by RawStride
  (ExifTool 13.59) on 2026-11-19, most likely in a hand check, and no backup of the earlier state
  exists. The tests follow the samples as they are since then (in the two JPEGs the stored IIM
  checksum matches and City is in IIM), and the six samples are kept read-only (Windows' read-only
  attribute) so a hand check cannot change them again; the tests work on writable copies.

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
| Time zone | TZD optional (§14.2) | IIM time with offset | never invent | never invent; IIM 2:60 left out without a known zone | Compatible (Q3; confirmed by IIM 4.2 2:60, 2026-11-18) |
| Rating | −1, 0…5, real | – | float, rounding for display | as both | Compatible |
| Hierarchical keywords | – | – | MUST preserve | preserved | Compatible |
| Deprecated IIM datasets | keep in sync (§9.1) | – | – | kept and synced | Compatible |
| DNG embedded / RAW sidecar | not covered | – | DNG not covered | product decision | No conflict |
| Clearing | – | – | update both forms or remove the other | consistent across forms, RAW untouched; a cleared property of a manufacturer RAW is filled again from the camera's Exif (Q2) | Compatible: the RAW Exif cannot be changed |
| Lang Alt edits | Lang Alt type (§14.1) | – | §5.2: only `x-default` discussed, other languages MAY be supported | edit `x-default` only; other entries kept | Compatible with MWG. **Deviation from an [XMP-1 §8.2.2.4] "should"**, deliberate: the `x-default` text should be repeated in an entry with its actual language, and after an edit an entry with the old text is not updated (Q7, §8) |

**One rule of this ADR deliberately contradicts a MUST:** the IIM checksum is not written while
a no-checksum conflict is unresolved (§5.2, Q1), so that a write never decides the conflict
silently. All other deviations concern SHOULD-level recommendations; each is marked
[Product ≠ std] with its reason. The Lang Alt rule (§8, Q7) was checked against MWG §5.2 and
[XMP-1] on 2026-11-18 and is one of them.

## 14. Questions (all answered by the owner)

| # | Question | Answer |
|---|---|---|
| ~~**Q1**~~ | **Writing a file with an unresolved IIM/XMP conflict** (no-checksum case). Writing both forms and the checksum would decide the conflict silently in favour of the written value | **Resolved 2026-11-11:** Writing other fields is allowed. For each conflicting property both forms are left as they are **and no new checksum is written**, until the user decides. MWG requires the checksum "whether changed or not", so this is a deliberate exception, limited to unresolved conflicts (§5.2, §13). Batch operations list such files |
| ~~**Q2**~~ | **Clearing a field for an ARW/RW2 whose embedded camera Exif has a value** (e.g. Artist set in the camera). After clearing, the camera value would fill the gap again (§6.2) | **Resolved 2026-11-11, changed by the owner 2026-11-18:** no marker. A cleared property is removed from the sidecar and the camera's value fills it again (§6.4). The answer of 2026-11-11, a `Cleared` Bag in a RawStride namespace that hid the camera's value in RawStride only, was never implemented and is dropped for the reasons in §6.4. An explicitly empty sidecar value may be added later if it is shown to work with the files of other programs (§6.4) |
| ~~**Q3**~~ | **IIM 2:60 Time Created without a known time zone.** IIM's time format includes an offset; writing `+0000` would invent one | **Resolved 2026-11-11:** Write only 2:55 (the date) and leave 2:60 out until the zone is known. **Confirmed 2026-11-18** by IIM 4.2: 2:60 is always `HHMMSS±HHMM` and is not mandatory, so leaving it out is allowed and nothing is invented |
| ~~**Q4**~~ | **Exif overlap fields inside DNG:** the decision lists JPEG/TIFF for synchronisation | **Resolved 2026-11-08:** Treat DNG like TIFF: overlap fields that exist are updated (MWG requires updating or removing the other form) |
| ~~**Q5**~~ | **A foreign `<basename>.xmp` next to a DNG** (written by another tool) | **Resolved 2026-11-08:** Never written and never used as a source; the info panel shows a notice that it exists and is ignored |
| ~~**Q6**~~ | **Creating missing Exif overlap fields** in JPEG/TIFF | **Resolved 2026-11-08:** Do not create missing fields in JPEG, TIFF or DNG (DNG is handled like TIFF, Q4); synchronise only existing overlap fields |
| ~~**Q7**~~ | Exact Lang Alt rules (`x-default` alongside a language entry) | **Resolved 2026-11-08 (product policy):** edit only `x-default`; leave all other language entries and their values unchanged. If `x-default` is absent, add it first. Clearing removes the whole property. Adobe's SDK also synchronises entries matching the old default ([`SetLocalizedText`](https://github.com/adobe/XMP-Toolkit-SDK/blob/v2025.03/XMPCore/source/XMPMeta-GetSet.cpp), SDK v2025.03); RawStride deliberately does not. The full ISO 16684-1 text was not accessible during review, so verify this policy against the standard before implementation. **Checked 2026-11-18** against [XMP-1] (ISO 16684-1:2011) and MWG §5.2: compatible with MWG; a deliberate deviation from the [XMP-1 §8.2.2.4] "should" that the `x-default` text is repeated in an entry with its actual language (§8, §13) |
| ~~**Q8**~~ | Normative sidecar naming (`name.xmp` vs `name.ARW.xmp`) and RAW+JPEG pairs | **Resolved 2026-11-11:** `<basename>.xmp`, as ADR 0004 and the default of Lightroom and Photo Mechanic. A `<basename>.<ext>.xmp` is never read or written and is reported, as in Q5. RAW+JPEG pairs as §4.2. To confirm with the level-C samples; Adobe XMP Part 3 is read if available. **Confirmed** for Photo Mechanic by the level-C samples (T127, 2026-11-14) and by [XMP-3] (2026-11-18, §4.2) |
| ~~**Q9**~~ | Allowed URI values of PLUS properties (release status, minor model age, data mining) | **Resolved 2026-11-11:** a fixed choice list with Swedish labels (English since 2026-11-14, ADR 0017 U10d-Q3), values taken from the PLUS controlled vocabulary; unknown values kept with a warning (§11). The vocabulary itself must be read before implementation (§16) |
| ~~**Q10**~~ | Fallback encoding for IIM/Exif text that is neither ASCII nor UTF-8 | **Resolved 2026-11-11:** Windows-1252 by default, changeable in the settings; the value is marked as converted (§5.4) |
| ~~**Q11**~~ | Test files | **Resolved 2026-11-11:** the level-C samples of §12, provided by the owner before the implementation is tested. **Changed 2026-11-14:** Photo Mechanic files, a Xiaomi DNG and mixed Photo Mechanic / Lightroom Classic archive files; no Lightroom-only files, no IIM-only JPEG (§12) |
| ~~**Q12**~~ | A manual "correct camera time" feature (MWG: such corrections SHOULD be exported to Exif) | **Resolved 2026-11-11:** Out of scope now; would be the only way DateTimeOriginal ever changes, and only on the user's explicit command (§7). **Taken up 2026-11-17 (owner): ADR 0020** |
| ~~**Q13**~~ | "New IIM only when backward compatibility requires it": how is that detected besides the setting? | **Resolved 2026-11-11:** Only the setting (and possibly a future export profile); no automatic detection (§5.2) |
| ~~**Q14**~~ | **(Revision 26.)** The modification time of a written file: set back to the original's, or the time of the write? | **Decided by the owner 2026-11-19:** the time of the write, for every file (§9.4) |
| ~~**Q15**~~ | **(Revision 26.)** A flat `lr:hierarchicalSubject` beside the keywords | **Decided by the owner 2026-11-19:** written equal to `dc:subject` when every entry is flat; unchanged when one entry has a `\|` (§8) |
| ~~**Q16**~~ | **(Revision 26.)** Exif Rating/RatingPercent and `xmp:RatingPercent` that another program added | **Decided by the owner 2026-11-19:** existing ones written to match the rating, a reject removes them, never created or read (§6.3) |
| ~~**Q17**~~ | **(Revision 29.)** The final read after the replace (§9.4): a full snapshot, or `ImageDataHash` alone? | **Decided by the owner 2026-11-22, as proposed:** `ImageDataHash` alone; it is all the final read compares, so the check is unchanged (§9.6 point 1) |
| ~~**Q18**~~ | **(Revision 29.)** A batch: one ExifTool process, or several at once? | **Decided by the owner 2026-11-22, as proposed:** up to four at once, the number fixed after a measurement with 1, 2 and 4; every file its own transaction; one target never written twice at once and its writes in order; the journal holds several open entries (§9.6 point 2). **The owner, 2026-11-22 (after T267's measurement):** the number is four (§9.6 point 2) |
| ~~**Q19**~~ | **(Revision 29.)** The DNG image check (§9.3 point 4), the digest pass and the TIFF restart (§9.2, §9.3): changed now, or measured first? | **Decided by the owner 2026-11-22, as proposed:** measured first in a task, no rule changed now; the owner decides afterwards (§9.6 point 3). Measured in T264, T265 and T269; the decision is Q20–Q23 |
| ~~**Q20**~~ | **(Revision 30.)** The lowest ExifTool version | **Decided by the owner 2026-11-22, as proposed:** 13.64, which fixes both reasons of T92 (T269); shipped and in CI too. Alternative: 12.67 kept, the steps chosen by version (§9.7 point 1) |
| ~~**Q21**~~ | **(Revision 30.)** The digest pass (§9.2) | **Decided by the owner 2026-11-22, as proposed:** dropped; the checksum written in the IIM call and the copy refused unless its checksum is current. Alternative: the digest pass only when the checksum is out of date (§9.7 point 2) |
| ~~**Q22**~~ | **(Revision 30.)** The TIFF restart (§9.3) | **Decided by the owner 2026-11-22, as proposed:** dropped; a misread would show as new warnings and the copy be refused (§9.7 point 3) |
| ~~**Q23**~~ | **(Revision 30.)** The DNG image check (§9.3 point 4) | **Decided by the owner 2026-11-22, as proposed:** kept as it is; the damage only LibRaw's unpack finds was not measured, and is measured in T273 before the unpack is decided again. Alternative: the unpack dropped, 250–550 ms saved per DNG (§9.7 point 4). **Decided again by the owner 2026-11-22 (revision 32, after T273):** LibRaw's open and unpack run only when the original has no `ImageDataHash` (§9.3 point 4); task T275 |

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

## 16. Before implementation (what is left after milestone 8 is in §18)

1. ~~The owner decides D5 and the IIM default~~ – decided 2026-11-04.
2. ~~Read the IPTC spec 9–17, the Mapping Guidelines and MWG 2.0~~ – done 2026-11-04 (analysis
   §8–§9).
3. ~~Answer the remaining open questions~~ – Q1–Q3 and Q8–Q13 answered 2026-11-11 (§14).
   The ADR was approved by the owner the same day.
4. ~~**Read the PLUS controlled vocabulary**~~ (Q9) – read 2026-11-14 and saved in
   `D:\PhotoWorkflowData\references\` (the LDF 1.2.0 vocabulary file; Data Mining from the PLUS
   XMP specification 2.0.1, in the folder's `README.md`).
5. ~~Read **XMP Part 1** (ISO 16684-1) and **MWG §5.2** to verify the Q7 policy, **XMP Part 3** (Q8)
   and **IIM 4.2** (Q3) if the owner provides them~~ – read 2026-11-18 and saved in
   `D:\PhotoWorkflowData\references\` (R3; XMP Part 1 is the text of ISO 16684-1:2011). Q3 and Q8
   confirmed; Q7 compatible with MWG and a deliberate deviation from an [XMP-1] "should" (§8, §13).
6. ~~**Level-C samples**~~ (Q11, the set changed 2026-11-14, §12) – received 2026-11-14 and
   tested by T127 (`crates/pw-metadata/tests/level_c.rs`); what they showed is in §4.2 and §6.4,
   and they led to T129 (§5.1) and to the PLUS codes of §11.
7. ~~**Read IPTC Scene NewsCodes and IPTC Genre NewsCodes**~~ (§11, revision 22) – read
   2026-11-18 and saved in `D:\PhotoWorkflowData\references\` (R4): 24 Scene codes, none retired;
   58 Genre codes, 7 of them retired. Genre codes are words whose case varies, so they are compared
   exactly as in the file.

## 17. Decisions during implementation (D1–D16)

The owner's decisions on questions that came up while milestone 8 was built. They were kept in the
step list (now `docs/tasks/plans/adr-0007-milestone-8-steps.md`) until 2026-11-13 and are copied
here unchanged, with each proposal and its approval; the step list points here. Later changes are
listed under the table (§17.1).

| # | Decision | §§ | Status |
|---|---|---|---|
| D1 | The XMP namespace (prefix and URI) of `Cleared` (§6.4: "fixed with the implementation"). Reading and writing `Cleared` gets its own small task after D1 | §6.4 | dropped (§17.1) |
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
| D17 | A JPEG (and possibly a DNG) exported by DxO PhotoLab from a Sony RAW keeps Sony's maker note, whose `HiddenInfo` points at data (`HiddenDataOffset`) that exists only in the original ARW. Writing the copy, ExifTool answers `Warning: [minor] Error reading HiddenData`, and §9.2 refuses any write answered with a warning, so such files cannot be rated (found by T211's real-file run on `TN005701_DxO.jpg` and `TN005702_DxO.jpg`; checked by the reviewer: the copy's `ImageDataHash` equals the original's and the pointer is unchanged, nothing is lost). Reviewer's proposal: a short, named list of known harmless write warnings, today only this one, accepted in the answer to the write of the copy and to the checksum pass; every other warning is refused as before (also other `[minor]` ones, such as a truncated `IPTC:City`), and §9.3's checks of the copy run as usual. Alternatives: accept every `[minor]` warning (breaks the truncation rule), or keep the refusal with a clearer message. **Approved by the owner on 2026-11-19** | §9.2, §9.3 | T212 |
| D18 | A JPEG straight from a Sony camera (the ILCE-1; found in the owner's hand check of ADR 0024, 2026-11-20) has a maker note whose `HiddenDataOffset` does not point at the hidden data. On any write that rewrites the Exif (an orientation, an `IFD0:` field of the metadata window) ExifTool 13.59 corrects the pointer and answers `Warning: [minor] Fixed incorrect HiddenDataOffset`, so §9.2 refuses the write and a turn of such a file is always rolled back; an XMP-only write gives no warning. Measured by the reviewer on copies of `_TN06333.JPG` and the level-C `PM-jpg.JPG`: only `IFD0:Orientation` and `Sony:HiddenDataOffset` change (a pointer, which §9.3 already allows), the validation counts are the same, and the hidden data (53248 bytes) is byte for byte the same, now found at the new offset. Reviewer's proposal: add this warning to D17's named list, for every write; every other warning is still refused and §9.3 runs as usual. **Approved by the owner on 2026-11-20** | §9.2 | T238 |

### 17.1 Later changes to D1–D16

- **D1** is dropped (owner, 2026-11-18): Q2 was changed so that nothing records a cleared field
  (§6.4), and RawStride has no XMP namespace of its own. If one is needed later (the original
  capture time, ADR 0020 §2.7), it is decided with that step. A template file needs none: it
  holds only standard XMP (owner, 2026-11-22, revision 33, §10).
- **D8** is dropped (owner, 2026-11-22, §9.7 point 5; T274): ExifTool 13.64, the lowest version,
  writes a new Photoshop block without the extra validation warning (T270 point 6), so one more
  warning is refused also when the write creates the Photoshop block of a TIFF or DNG.
- **D9, the version** (owner, 2026-11-22, §9.7 Q20; T271): "older than 12.67" is now older than
  13.64 (`MIN_EXIFTOOL_VERSION`); the rest of D9 is unchanged.
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
- **D13** is replaced (owner, 2026-11-22, revision 33; ADR 0017 U13-Q6): the import dialog's six
  template fields and their modes give way to a template file chosen in the dialog, whose
  checked fields replace (§10). Until U13's tasks are done, the dialog keeps D13.
- **D15** was replaced by D16 the same day (2026-11-12).
- **D16** is changed by ADR 0017:
  - the window is no longer locked to one image: previous/next and "Spara och nästa" with the
    question "Spara ändringarna?" (Spara / Släng / Avbryt), which also replaces "Släng
    ändringarna?" for Escape (ADR 0017 U-Q8, §2.3.1 and §6.1, U12-Q1–U12-Q5; owner, 2026-11-12 and
    2026-11-13);
  - "Övrig metadata" shows its values without an origin line under each one; the conflicts keep
    theirs (owner, 2026-11-13, T114).

## 18. What is left after milestone 8

- §6.6 Exif ImageEditor ↔ Contributor: T192.
- §8 lists with qualifiers not written: T193, T194 (done). The notice that no new checksum was
  written gives a reason that holds for both causes (§5.2 Q1, §8): T199 (done). The batch's lines for the
  other images' notices: T198 (done; ADR 0017 U12d-Q10). Editing such a list with its qualifiers kept
  (`docs/tasks/backlog.md`).
- §6.7 camera GPS → the coordinates of Location Created: T196 (done). Reverse
  geocoding, GPX and a map: the GPS ADR after ADR 0017 (its §7.2).
- §11 the other IPTC NewsCodes (Scene Code, Subject Code, Genre): T195 (done).
- From §10: template sub-fields of lists of structures, variables, template files in XMP
  (`docs/tasks/backlog.md`; ADR 0017 U13).
- Image Region (ADR 0017 U11b). The other lists of structures and editing several images are done
  (ADR 0017 U11, U12; T136–T143, T125, T126, T179, T180).
- The Tauri app keeps its in-place write (D7) until it is retired (GR8). **Retired 2026-11-19
  (T207); owner, 2026-11-19:** the in-place write calls `ExifTool::write_rating` and
  `ExifTool::write_iptc` (and the private `write_tags`) are removed with the tests that use only
  them, so every write goes through the safe write (§9); coverage they alone had is moved to the
  safe write's tests (T211).
- Revision 26 (§9.4 Q14, §8 Q15, §6.3 Q16): two tasks (ADR 0029 step P0b; T204 the
  modification time, T205 Q15 and Q16); both done and hand-checked by the owner 2026-11-19.
- Revision 29 (§9.6, Q17–Q19, a faster safe write): tasks written 2026-11-22, in the order of
  §9.6: T263 (Q17), T264 and T265 (Q19's measurement), the chain T266 → T267 → T268 (Q18; the
  number of processes is fixed by the owner after T267). T267 done 2026-11-22; the owner fixed
  the number at four the same day (§9.6 point 2); T268 makes the app use it.
- Revision 30 (§9.7, Q20–Q23, Q19 decided): tasks written 2026-11-22: the chain T270 → T271 →
  T272 (ExifTool 13.64 shipped and pinned; 13.64 the lowest version; the digest pass and the
  restart dropped, the checksum checked) and T273 (other kinds of DNG damage measured, for Q23).
  T270 and T271 done 2026-11-22 (13.64 shipped and pinned; 13.64 the lowest version, §11, D9).
  T272 done 2026-11-22 (one call and no restart; the stored checksum checked: §5.2, §9.2, §9.3).
  T273 done 2026-11-22 (seven kinds of DNG damage; none refused only by the unpack: the analysis
  `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T273"); §9.7 point 4 (Q23)
  waits for the owner's decision on the unpack.
- Revision 31 (D8 dropped, §9.7 point 5): task T274 written 2026-11-22. T274 done 2026-11-22
  (the allowance removed from `verify_copy`; every real-file test passed with ExifTool 13.64).
- Revision 32 (Q23 decided again, §9.3 point 4, §9.7 point 4): task T275 written 2026-11-22
  (LibRaw's check of a DNG copy only when the original has no `ImageDataHash`). T275 done
  2026-11-22 (`DataHash` and `ImageCheck::check_copy_with`; with a compared hash a cut DNG
  copy was checked in under 10 ms instead of 301–907 ms); hand-checked by the owner 2026-11-22
  (32 DNGs rated at once in a release build: 11.34 s and 10.55 s before, 6.97 s and 6.18 s
  after; every rating shown after a restart).

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
| 18 | 2026-11-14 | I13 (T133): implemented status, where each §12 test is, §18 what is left; no new decision |
| 19 | 2026-11-18 | Owner changed Q2: a cleared field of a manufacturer RAW is not marked and is filled again from the camera's Exif (§6.4, §13, Q2); D1 dropped (§17, §17.1); §18 brought up to date (U11 and U12 done) |
| 20 | 2026-11-18 | §16 point 5 done: XMP Part 1 (ISO 16684-1:2011), XMP Part 3, IIM 4.2 and MWG §5.2 read (R3); labels [XMP-1], [XMP-3], [IIM 4.2] (§1.1). Q3 and Q8 confirmed (§4.2, §5.3, §7, §13, §14); Q7 compatible with MWG §5.2 and marked [Product ≠ std] against an [XMP-1 §8.2.2.4] "should" (§8, §13, §14), as revision 8 foresaw; §18 brought up to date. No rule changed |
| 21 | 2026-11-18 | Owner decisions: §6.6 Image Editor ↔ Contributor (the content editor by its Role URI, `http://` too; Exif fills an absent Contributor, otherwise a conflict whose Exif side keeps the other Contributors; ImageEditor written only if it exists); §8 a list whose items have qualifiers is shown but not written, after a check showed that ExifTool 13.59 cannot change or remove a qualified item; §18: T192, T193, T194 |
| 22 | 2026-11-18 | Owner decisions: §6.7 camera GPS fills only the coordinates of Location Created (no network, no new dependency; reverse geocoding, GPX and a map stay with the GPS ADR); §11 Scene Code (6 digits, IPTC Scene NewsCodes), Subject Code (8 digits, format only) and Genre (IPTC Genre NewsCodes only when its CV ID is that vocabulary) give warnings, Intellectual Genre is not checked; §16 point 7 reads the two vocabularies (R4); §18 brought up to date |
| 23 | 2026-11-18 | §16 point 7 done (R4: 24 Scene codes; 58 Genre codes, 7 retired); §11 the Genre CV ID may also use `https://` and lack the trailing `/` (cold read of T195); §6.7 the tolerance of 0.0001°, the altitude not compared, any Exif GPS counts as the camera's (cold read of T196); §18 names T195 and T196 |
| 24 | 2026-11-19 | Owner decision (backlog sorted after T194): the notice that no new IIM checksum was written names a reason that holds both for an unresolved conflict (§5.2 Q1) and for a held list with qualifiers (§8), "No new IIM checksum was written, because IIM and XMP still differ" (T199); §18 names T198 and T199 |
| 25 | 2026-11-19 | §6.7 written in from T196 (no new rule): only the first entry of Location Created is looked at; a first entry without coordinates gets them and keeps its origin (owner, 2026-11-18), its altitude only when it has none; a refused Location Created is not replaced (reviewer's choice); unusable Exif GPS gives a notice; §18 marks T195 and T196 done |
| 26 | 2026-11-19 | From the test with DxO PhotoLab 10.3 (ADR 0029 step P0, test copies in `D:\PhotoWorkflowData\P0-dxo`): §9.4 a written file gets the time of the write as its modification time, sidecars and in-place writes alike (Q14; replaces the old time kept since ADR 0004); §8 a flat `lr:hierarchicalSubject` written equal to the keywords (Q15), §6.1 and §6.3 existing Exif Rating/RatingPercent and `xmp:RatingPercent` written to match the rating (Q16); all three decided by the owner; §14 Q14–Q16; §18 one task |
| 27 | 2026-11-19 | Owner decision D17: the write warning `[minor] Error reading HiddenData` (a DxO export of a Sony RAW) is accepted by name; every other write warning is still refused (§9.2); task T212 |
| 28 | 2026-11-20 | Owner decision D18: the write warning `[minor] Fixed incorrect HiddenDataOffset` (a Sony ILCE-1 JPEG on an Exif write) is accepted by name, added to D17's list (§9.2, §17); task T238 |
| 29 | 2026-11-22 | Owner decision (after T261): a faster safe write without a weaker check: §9.6, Q17 the final read asks for `ImageDataHash` alone, Q18 a batch shared among up to four ExifTool processes, Q19 the DNG image check, the digest pass and the TIFF restart measured before any change; proposed and approved by the owner the same day, all as proposed (§14); §18 |
| 30 | 2026-11-22 | Proposal (after T264, T265 and T269, at the owner's request): Q19 decided in §9.7, Q20 ExifTool 13.64 or newer, Q21 the digest pass replaced by a check that the copy's checksum is current, Q22 the TIFF restart dropped, Q23 the DNG image check kept; approved by the owner the same day, all as proposed, with the measurement for Q23 as a task (T273); §18 |
| 31 | 2026-11-22 | Owner decision (after T270 point 6 and T272): D8's allowance of one more validation warning when the write creates the Photoshop block of a TIFF or DNG is dropped, since ExifTool 13.64 no longer writes that warning (§9.7 point 5, §17.1); task T274, with every real-file test run with 13.64 as the gate |
| 32 | 2026-11-22 | Owner decision (after T273: no kind of DNG damage measured was refused only by the unpack): Q23 decided again, LibRaw's open and unpack of a DNG copy run only when the original has no `ImageDataHash`; with a hash, the snapshot comparison and the preview comparison are the check (§9.3 point 4, §9.7 point 4); task T275 |
| 33 | 2026-11-22 | Owner decision with ADR 0017 U13 (§6.5, U13-Q1–U13-Q11): a template file holds only standard XMP; each field has a check box and only the checked fields are saved and applied; a checked field replaces the image's value; the modes Keep, Append, Prefix and Clear and the RawStride namespace are dropped from templates (§10); D1's note and D13 changed (§17.1); variables turned into each image's own values when written. Tasks with U13 |
