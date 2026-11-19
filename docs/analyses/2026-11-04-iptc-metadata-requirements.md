# 2026-11-04 – Metadata: IPTC requirements and their effect on PhotoWorkflow

- Module: `pw-metadata` (and its users: ratings/labels ADR 0004, ingest IPTC template ADR 0005)
- Question: What do the IPTC standards require of an application that reads and writes photo
  metadata, how does that map to XMP and IIM, and what does it change in PhotoWorkflow?
- Normative reference (owner's decision): IPTC Photo Metadata Standard **2025.1**
  (IPTC Core 1.5, IPTC Extension 1.9). See `docs/references.md`.
- Method: public documents read on 2026-11-04 (level D). No implementation yet.
- Status: the owner's decisions of 2026-11-04 are recorded in §10 and written into ADR 0007
  revision 2. The proposals in §8 F and §9 D–E are **superseded** by that revision.

## 0. What was read, and what could not be read

Every finding below is marked **[read]** (taken from the source text) or **[unverified]**
(from a secondary source or summary, to be checked against the primary text before implementing).

| Source | Status |
|---|---|
| iptc.org/standards/photo-metadata (overview) | read |
| IPTC spec 2025.1 HTML: front matter, sections 1–6, 7.1–7.25, 8.1 | read |
| IPTC spec 2025.1 HTML: **sections 9–17** (Extension tables, *Generic Implementation Notes*, Lang Alt, date value type, max bytes, references, errata) | **read 2026-11-04 from the local copy** (see §8). Previously not reachable |
| IPTC TechReference 2025.1 JSON: all property names, XMP ids, types, cardinality, IIM ids and lengths for Core; Extension and most structures | read (the last ~25 % was summarised, marked) |
| IPTC User Guide (`…/userguide/index.html`) | **read** (first ~85 % verbatim; the end – dates/times in software, guidance for creators, AI images – as a summary, marked) |
| IPTC Mapping Guidelines (version 2023.1, Exif 3.0 included) | **read 2026-11-04 from the local copy, complete** (see §8). Previously not reachable |
| IIM page (iptc.org/standards/iim) | read |
| IPTC developers' guide (Atlassian) + its ExifTool page | read (the main page has links only) |
| MWG Guidelines 2.0 PDF | **read 2026-11-04** from the owner's copy `D:\PhotoWorkflowData\references\mwg_guidance.pdf` (MWG, version 2.0, October 2010, 73 pages; see §9). Previously blocked |
| ExifTool IPTC and MWG tag pages | read |
| Adobe XMP Specification Part 3 | **not readable** (PDF came back as binary) |
| Lightroom Classic help (metadata basics, keywords) | read in part |
| Photo Mechanic help (file formats, IPTC/XMP preferences, metadata template, general preferences, keywords) | read |

**Local copies (2026-11-04).** `D:\PhotoWorkflowData\references\` holds the specification 2025.1
and the Mapping Guidelines. Each is there as the original HTML (byte for byte, with SHA-256 in
`README.md`) and as searchable Markdown with all tables. The check found the specification
complete: all sections up to "17. List of fixed errata", and 166 tables. The Mapping Guidelines
are complete too: all sections and 23 tables.

**Still missing:**
- Adobe XMP Part 3 and ISO 16684-1 (XMP Part 1).
- IIM 4.2 (PDF).
- The PLUS specification (controlled vocabularies of the PLUS properties).

What these affect is listed in §8.

## 1. General rules of the standard

| # | Rule | Source |
|---|---|---|
| G1 | Properties are defined once, with an XMP representation and, for most IPTC Core properties, an IIM representation. | IPTC spec §5, §7 [read] |
| G2 | "The values of properties **should** be saved to an image file using **both** the IIM format and the XMP format as defined in the specification tables." | IPTC spec §5.3.4 [read] |
| G3 | IPTC Core is "fully backwards-compatible with IIM"; IIM development was frozen in 1997 (minor change 2014) and IIM "is still in use, mainly by 'IPTC Fields' for photo metadata". | IPTC spec §3 [read]; IIM page [read] |
| G4 | Each IIM text dataset has a maximum length in **bytes**, given per property in the specification ("max bytes for text"). | IPTC spec §5.1 [read]; values from TechReference [read] |
| G5 | "Language translations are NOT a metadata structure": Lang Alt values are alternative-language versions of one value. | IPTC spec §5.3.1 [read] |
| G6 | For synchronising IIM and XMP, the Metadata Working Group guideline "was widely used" but has not been maintained since October 2018. | IPTC spec §5.3.4 [read] |
| G7 | IIM checksum: an MD5 over the whole IIM block, stored as 16 bytes in Photoshop Image Resource (PSIR) 1061. It "MUST be computed and stored when a Creator or Changer writes XMP and IPTC-IIM simultaneously". Whenever both are present, "whether changed or not, a Changer MUST create or update the checksum". | **MWG 2.0 §4.2.3.2 [read]** (ExifTool's description matches) |
| G8 | MWG as implemented by ExifTool: "IPTC information is written only if the original file contained IPTC." This differs from G2 (IPTC: write both). **Owner's decision (2026-11-04):** XMP is primary; existing IIM is preserved and synced; IIM is created only with an opt-in setting (ADR 0007). | ExifTool MWG page [read] |
| G9 | IIM text encoding: CodedCharacterSet (1:90) = UTF-8 (`ESC % G`). "When creating a new IPTC record from scratch, it is suggested that this be set to 'UTF8'". | ExifTool IPTC page [read]. The IPTC spec only refers to the IIM specification (§15.1) [read]; IIM 4.2 [unverified] |
| G10 | Reading precedence between IIM and XMP when both exist:<br>- Checksum matches, or none: "Any existing XMP value … SHOULD be preferred"; if the XMP value is missing, use IIM.<br>- Checksum differs (an XMP-unaware tool changed IIM): **per property**, predict the IIM value from the XMP (truncation, encoding). If the prediction matches, use XMP, "**Otherwise the IPTC-IIM value is used**". | **MWG 2.0 §4.2.3.2 [read]**. The IPTC documents define no rule of their own. See §9 for the difference from ADR 0007 D3 |

## 1a. IPTC Photo Metadata User Guide (read 2026-11-04)

The User Guide is written for users, not implementers. It does **not** cover IIM/XMP precedence
rules, character encoding, Lang Alt handling, templates or empty values; those stay with the
specification and MWG. What it does say [read]:

| # | Statement | Effect on PhotoWorkflow |
|---|---|---|
| U1 | "IPTC Core fields can be embedded in the IIM format and/or in the XMP format." "IPTC Extension fields can be embedded only in XMP format." | Matches ADR 0007: XMP primary, IIM optional |
| U2 | "the values are synchronised – this should be taken care of by the image management software"; software "has to verify that the IIM and XMP values are the same; if they are not, there are rules about which value should take precedence" (rules not given) | ADR 0007 D3/D4: sync on write, conflict shown on read; precedence = owner's decision (XMP) |
| U3 | "IPTC Core and IPTC Extension fields can be saved as XMP 'sidecar files' for camera Raw files"; the Embedded Metadata Manifesto adds that "sidecar files are potentially exposed to metadata loss" | Sidecars for manufacturer RAW (owner's decision); ingest and moves must always carry the sidecar with its RAW (already so in ADR 0005: shots) |
| U4 | Fields "can be embedded into JPEG, TIFF and PSD files" (APP1/APP13 for JPEG). DNG is not mentioned | DNG embedded is the owner's decision: an open TIFF-based format with a defined place for XMP |
| U5 | Embedded Metadata Manifesto: "Copyright management information metadata must never be removed from the files." "Other metadata should only be removed from files by agreement with their copyright holders." Fields and values "should not be changed across metadata formats" | **Template "Clear" is not offered for copyright-management fields** (Copyright Notice, Creator, Credit Line, Rights Usage Terms, Web Statement of Rights, Copyright Owner, Licensor, Image Creator). Replace is allowed (a new holder). Clearing other fields asks for confirmation. IIM/XMP values must be the same text |
| U6 | Creator's Jobtitle "is a qualifier for the Creator field, the Creator field must also be filled out" | Validation warning: Jobtitle without Creator |
| U7 | "If both the Country and Country Code fields are used, the Country Code is the authoritative reference"; codes are ISO 3166 upper-case 2 or 3 letters | Validation of Country Code; Country Code wins on conflict |
| U8 | Date Created "can be derived from the Exif DateTimeOriginal"; "if a time value is added, time zone should also be recorded" [summary]; "If you change a shown Date Created value this can also change this corresponding Exif tag" | Date Created is filled from DateTimeOriginal **only if empty** (e.g. at ingest), with the camera's offset (OffsetTimeOriginal) when present. PhotoWorkflow still never changes Exif DateTimeOriginal itself (ADR 0007 D6: protects the capture time); the guide describes what some software does, it is not a requirement |
| U9 | Legacy location fields (City, Sublocation, State, Country) "are widely understood to express the location shown in the image"; Location Created / Location Shown are the structured Extension fields | Both are offered; no automatic copying between them |
| U10 | Image Rating: "there is no universal standard for the rating between systems" | `xmp:Rating` as today (ADR 0004) |
| U11 | Person Shown names "as they would be typed in a query, first name / last name"; Digital Source Type: "the identifier of a term must be applied" [summary] | Person Shown as free text; Digital Source Type as a choice from the IPTC controlled vocabulary (URI) |

## 2. Property mapping (XMP ↔ IIM) – IPTC Core 1.5

All 25 IPTC Core properties, from the TechReference 2025.1 JSON [read]. "multi" = may have several
values. **The XMP value types have now been checked row by row against spec §7 [read]:**
- `dc:creator` = Seq ProperName.
- `dc:subject`, `Iptc4xmpCore:Scene` and `Iptc4xmpCore:SubjectCode` = Bag.
- Alt Text, Copyright Notice, Description, Extended Description, Rights Usage Terms and Title =
  Lang Alt.
- Date Created = XMP Date.
- Country Code, Scene and Subject Code = closed choice.
- All others = Text.

The IIM lengths in the table match the spec. Corrections from the full text are in §8.

| Property | XMP | Type | Card. | IIM | IIM max bytes |
|---|---|---|---|---|---|
| Alt Text (Accessibility) | Iptc4xmpCore:AltTextAccessibility | Lang Alt | 1 | – | – |
| City | photoshop:City | Text | 1 | 2:90 City | 32 |
| Copyright Notice | dc:rights | Lang Alt | 1 | 2:116 Copyright Notice | 128 |
| Country | photoshop:Country | Text | 1 | 2:101 Country/Primary Location Name | 64 |
| Country Code | Iptc4xmpCore:CountryCode | Text | 1 | 2:100 Country/Primary Location Code | 3 |
| Creator | dc:creator | Text | multi | 2:80 By-line | 32 (each) |
| Creator's Contact Info | Iptc4xmpCore:CreatorContactInfo | struct | 1 | 2:118 Contact (spec §8.1) | – |
| Creator's Jobtitle | photoshop:AuthorsPosition | Text | 1 | 2:85 By-line Title | 32 |
| Credit Line | photoshop:Credit | Text | 1 | 2:110 Credit | 32 |
| Date Created | photoshop:DateCreated | Text (date-time) | 1 | 2:55 Date Created + 2:60 Time Created | – |
| Description | dc:description | Lang Alt | 1 | 2:120 Caption/Abstract | 2000 |
| Description Writer | photoshop:CaptionWriter | Text | 1 | 2:122 Writer/Editor | 32 |
| Extended Description (Accessibility) | Iptc4xmpCore:ExtDescrAccessibility | Lang Alt | 1 | – | – |
| Headline | photoshop:Headline | Text | 1 | 2:105 Headline | 256 |
| Instructions | photoshop:Instructions | Text | 1 | 2:40 Special Instruction | 256 |
| Intellectual Genre | Iptc4xmpCore:IntellectualGenre | Text | 1 | 2:04 Object Attribute Reference | 64 |
| Job Identifier | photoshop:TransmissionReference | Text | 1 | 2:103 Original Transmission Reference | 32 |
| Keywords | dc:subject | Text | multi | 2:25 Keywords | 64 (each) |
| Province/State | photoshop:State | Text | 1 | 2:95 Province/State | 32 |
| Rights Usage Terms | xmpRights:UsageTerms | Lang Alt | 1 | – | – |
| Scene Code | Iptc4xmpCore:Scene | Text | multi | – | – |
| Source (Supply Chain) | photoshop:Source | Text | 1 | 2:115 Source | 32 |
| Subject Code | Iptc4xmpCore:SubjectCode | Text | multi | 2:12 Subject Reference | 236 |
| Sublocation | Iptc4xmpCore:Location | Text | 1 | 2:92 Sublocation | 32 |
| Title | dc:title | Lang Alt | 1 | 2:05 Object Name | 64 |

**Creator's Contact Info** (structure, XMP only except the IIM Contact dataset): Address
`CiAdrExtadr`, City `CiAdrCity`, State/Province `CiAdrRegion`, Postal Code `CiAdrPcode`, Country
`CiAdrCtry`, Email(s) `CiEmailWork`, Phone(s) `CiTelWork`, Web URL(s) `CiUrlWork` (all
`Iptc4xmpCore:`) [read].

The discrepancy between the JSON and the HTML is now resolved [read, spec §8.1]:
- Address, Email(s), Phone(s) and Web URL(s) have the *basic* cardinality 0..unbounded, but each
  is **one XMP Text value**.
- So several e-mail addresses go into the one text value (the UI should "provide sufficient space
  to fill in multiple …"), not into an XMP array.
- The spec defines no separator, so PhotoWorkflow keeps the text exactly as entered and read.
- The structure belongs to the **first** creator in `dc:creator` (spec §7.7 Note 3; the same holds
  for Creator's Jobtitle, §7.8).

Namespaces [read]: `Iptc4xmpCore` = `http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/`,
`dc` = `http://purl.org/dc/elements/1.1/`, `photoshop` = `http://ns.adobe.com/photoshop/1.0/`,
`xmpRights` = `http://ns.adobe.com/xap/1.0/rights/`. Extension prefixes: `Iptc4xmpExt`, `plus`,
`xmp`, `exif`. Their URIs are now read [spec §10]:
- `Iptc4xmpExt` = `http://iptc.org/std/Iptc4xmpExt/2008-02-29/`
- `plus` = `http://ns.useplus.org/ldf/xmp/1.0/`
- `xmp` = `http://ns.adobe.com/xap/1.0/`
- `exif` = `http://ns.adobe.com/exif/1.0/` (Exif 2.3 or earlier)

## 3. IPTC Extension 1.9 (XMP only – no IIM)

41 properties [read, TechReference, and spec §11 in full]. Those relevant for PhotoWorkflow's first
metadata features. **Corrected 2026-11-04 against spec §11 (see §8 B):** "multi" now shows the
exact cardinality where it is limited.

| Property | XMP | Type | Card. |
|---|---|---|---|
| Person Shown | Iptc4xmpExt:PersonInImage | Text | multi |
| Person Shown (Details) | Iptc4xmpExt:PersonInImageWDetails | struct | multi |
| Location Created / Location Shown | Iptc4xmpExt:LocationCreated / LocationShown | struct Location (XMP Bag) | **0..1** (UI: one location only, §11.26) / 0..unbounded |
| Organisation in Image (Name / Code) | Iptc4xmpExt:OrganisationInImageName / …Code | Text | multi |
| Event Name / Event ID | Iptc4xmpExt:Event (Lang Alt) / EventId (URI) | | 1 / multi |
| Model Release Status / Id | plus:ModelReleaseStatus (URI) / plus:ModelReleaseID | | 1 / multi |
| Property Release Status / Id | plus:PropertyReleaseStatus (URI) / plus:PropertyReleaseID | | 1 / multi |
| Digital Source Type | Iptc4xmpExt:DigitalSourceType | URI (IPTC CV) | 1 |
| Image Creator / Supplier / Copyright Owner / Licensor | plus:ImageCreator (Seq) / ImageSupplier (Seq) / CopyrightOwner (Seq) / Licensor (Bag) | struct (PLUS) | **0..3 / 0..1 / 0..3 / 0..3** |
| Web Statement of Rights | xmpRights:WebStatement | Text (a URL) | 0..1 |
| Image Rating | **xmp:Rating** | Closed Choice Real: **-1 (rejected) or 0..5; 0 = unrated; missing = 0** | 0..1 |
| Max Avail Width / Height | Iptc4xmpExt:MaxAvailWidth / MaxAvailHeight | Integer | 1 |
| Artwork or Object | Iptc4xmpExt:ArtworkOrObject | struct | multi |
| Image Registry Entry | Iptc4xmpExt:RegistryId | struct | multi |
| Additional Model Info, Model Age, Minor Model Age Disclosure | Iptc4xmpExt:AddlModelInfo, ModelAge, plus:MinorModelAgeDisclosure | | |
| Contributor | Iptc4xmpExt:Contributor | struct | multi |
| Data Mining, Other Constraints | plus:DataMining (URI), plus:OtherConstraints (Lang Alt) | | 1 |
| Genre, CV-Term About Image | Iptc4xmpExt:Genre / AboutCvTerm | struct CvTerm | multi |
| Image Region | Iptc4xmpExt:ImageRegion | struct | multi |
| AI Prompt Information / Writer / System / Version (new in 2025.1) | Iptc4xmpExt:AIPromptInformation, AIPromptWriterName, AISystemUsed, AISystemVersionUsed | Text | 1 |

Not part of IPTC but in common use (interoperability only): `xmp:Label` (colour label; Lightroom,
Photo Mechanic "Synchronize Color Class with IPTC/XMP Label" [read]), `lr:hierarchicalSubject`
(hierarchical keywords, Lightroom), `xmpRights:Marked` (Adobe "copyrighted" flag).

## 4. Overlap with Exif

MWG maps several IPTC properties to Exif fields that cameras write (ExifTool MWG page [read]):

| IPTC | Exif |
|---|---|
| Description | ImageDescription |
| Copyright Notice | Copyright |
| Creator | Artist (list separated by "; ") |
| Date Created | DateTimeOriginal (+ SubSec) |

Reading order per MWG composite: Exif, then IIM, then XMP [read, ExifTool]. Writing: ExifTool's
MWG mode writes to "any EXIF/IPTC/XMP records that exist" and creates new ones only in the
standard location [read].

**Replaced by §8 C:** the IPTC Mapping Guidelines 2023.1 are now read. They are IPTC's guideline
for Exif mapping ("not mandatory … highly suggested") and they differ from MWG on the Creator ↔
Artist mapping.

## 5. Storage per file type

| File | Where metadata is written | Source |
|---|---|---|
| Proprietary RAW (ARW, RW2, NEF, CR2, CR3 …) | **XMP sidecar `<basename>.xmp`**; the original is never modified | Owner's requirement 5. Naming rule (Adobe XMP Part 3) [unverified] |
| JPEG | Embedded: XMP (APP1, primary); IIM (Photoshop IRB, APP13) kept in sync if present, created only with the setting | Owner's requirement 6 and decisions 2026-11-04 |
| TIFF | Embedded: XMP (tag 700, primary); IIM (tag 33723 / IRB 34377) as JPEG | Owner's requirement 6; ExifTool IPTC page: IPTC "may be found in JPG, TIFF …" [read] |
| DNG | **Embedded, as JPEG/TIFF (owner's decision: explicit exception from the sidecar rule).** An open TIFF-based format with a defined place for XMP; one copy of the metadata cannot diverge from a sidecar | Owner's decision 2026-11-04 |
| Sidecar contents | XMP only (no IIM: IIM has no sidecar form) | IIM is a binary block in the file [read, IIM page] |

## 6. Effect on PhotoWorkflow today

| Area | Today | Gap against the standard / requirements |
|---|---|---|
| Ratings/labels (ADR 0004) | `xmp:Rating`, `xmp:Label` in the sidecar (RAW) or embedded (DNG/JPEG/TIFF) via ExifTool, in place, `-P` | Compliant for Rating (IPTC Extension "Image Rating" = `xmp:Rating`). DNG embedding needs D5 |
| Ingest IPTC template (ADR 0005) | Writes `dc:creator`, `dc:rights`, `photoshop:Credit`, contact e-mail/phone/URL, `xmpRights:Marked`; only non-empty fields | XMP only: **no IIM** for JPEG/TIFF (G2); no explicit "clear"; no "append"; contact info written as flat `CreatorWork*` tags (ExifTool names for the struct fields; to be checked against the spec structure) |
| Reading | Only Rating/Label are read | No reading of IPTC fields, no IIM/XMP reconciliation |
| IIM | Never written | CodedCharacterSet, lengths and digest must be handled once IIM is written |

## 7. Requirements derived for the design (input to ADR 0007)

1. One internal model keyed by the **IPTC property id** (XMP name), not by IIM or Exif names.
2. Read XMP, IIM and the Exif overlaps; reconcile per G7/G10 (to be verified); show the user when
   sources disagree instead of silently picking one.
3. Write only the properties that change; never rewrite the whole XMP packet; keep unknown
   namespaces, other languages of Lang Alt values, and other files' metadata untouched (ExifTool
   in-place editing, as today).
4. JPEG/TIFF/DNG: XMP always (primary). IIM: kept in sync when the file already has it, created
   only with the opt-in setting; UTF-8 with CodedCharacterSet set; values longer than the IIM
   limit kept in full in XMP and **reported**; IPTC digest updated (G7).
5. Manufacturer RAW: sidecar only, XMP only; the RAW file's bytes never change (as tested in
   ADR 0004).
6. Templates: per field, one of **Keep**, **Replace**, **Append** (multi-valued: add missing
   values; Description: prefix or append text) and **Clear** (remove explicitly). An empty field
   in a template means "Keep", never "Clear". These modes are RawStride's own.

## 8. Comparison with the full specification and the Mapping Guidelines (2026-11-04)

**Sources:**

- The local copies in `D:\PhotoWorkflowData\references\` (see §0):
  - the IPTC Photo Metadata Standard 2025.1, all sections, in particular 9–17, which were not read
    before;
  - the IPTC Photo Metadata Mapping Guidelines 2023.1, complete.
- Every item below is **[read]** from these documents, with its section, unless marked otherwise.
- **"Effect"** names the decision or design text it changes. ADR 0007 is **not** edited (the
  owner's instruction: analysis only). The proposed ADR changes are listed in §8 F.

### A. Previously [unverified], now confirmed or settled

| # | Item | Result |
|---|---|---|
| A1 | XMP container and value type of every Core property (§2) | Checked row by row against spec §7. Seq only for `dc:creator`; Bag for Keywords, Scene Code and Subject Code; Lang Alt for 6 properties; Date for Date Created |
| A2 | Extension namespace URIs | Read (spec §10); see §2 |
| A3 | Lang Alt handling | Spec §14.1: "Any software dealing with this property **must abide to the processing rules for the Lang Alt value type** as specified by the XMP specifications". §12.13: "XMP's Lang Alt implementations strongly use the language identifier `x-default`". The rule itself is in ISO 16684-1 (not read): ADR 0007 D4 ("write `x-default`, keep the other languages") stays, and the exact rule for an `x-default` that duplicates a language entry is still [unverified] |
| A4 | Date value type | Spec §14.2: allowed forms `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, `YYYY-MM-DDThh:mmTZD`, `YYYY-MM-DDThh:mm:ssTZD`, `YYYY-MM-DDThh:mm:ss.sTZD` (ISO 8601). IPTC "encourages" a UI widget that allows all of them |
| A5 | "Max bytes for text" | Spec §15.1: a count of **bytes, not characters**; with UTF-8, non-ASCII characters take several bytes, so fewer characters fit. ADR 0007 D4 ("cut at a UTF-8 character boundary, tell the user") is confirmed. The details are "in the full IPTC IIM specification" (not read) |
| A6 | MWG 2.0 (G7, G10) | Spec §16: the MWG document "was available … until October 2018 … this site is not active anymore". **The IPTC documents define no IIM/XMP precedence and no digest rule.** *Update:* MWG 2.0 has since been read from the owner's copy; see §9 |
| A7 | Required ExifTool version | Spec §5.1 (non-normative help row): "To support all properties of this version of the IPTC standard using **ExifTool version 12.67 or higher** is required". PhotoWorkflow uses 13.59 (ADR 0004 tests) |
| A8 | Creator's Contact Info cardinality (the JSON/HTML discrepancy) | Settled (see §2): several values in **one** Text value; the structure refers to the first creator |

### B. Corrections to the earlier analysis

| # | Was | Correct (source) | Effect |
|---|---|---|---|
| B1 | §3: Image Creator, Copyright Owner, Licensor "multi" | **0..3** each; Image Supplier **0..1** (spec §11.9, 11.18, 11.22, 11.24; PLUS 1.2 structures) | Validation: no more than 3 (warn when reading more) |
| B2 | §3: Location Created "multi" | **0..1** in the basic specs, although the XMP type is a Bag: "at the user interface … only a single location should be supported – as an image can be taken only at a single location" (§11.26) | UI and templates: one Location Created. If a file has several, read and keep them all but edit the first |
| B3 | §3: Image Rating "number" | **Closed Choice Real; "-1 or in the range 0..5. -1 indicates 'rejected' and 0 'unrated'. If an explicit value is missing the implicit default value is 0"** (§11.19) | ADR 0004 (1–5, -1 = reject, no value = unrated) conforms. **New:** reading must treat `0` as unrated, and accept real numbers (`3.0`) while never writing fractions. Values outside −1…5 are kept but shown as invalid |
| B4 | §3: Web Statement of Rights "URI" | Text whose value is a URL (§11.41) | Validation: URL check is a warning, not a type error |
| B5 | §2: Contact info sub-fields "follow the spec text" (as lists) | One XMP Text value each (§8.1) | The ingest template's e-mail/phone/web fields (ADR 0005) are written as single Text values. This **already matches** ExifTool's flat `CreatorWork*` tags |
| B6 | Spec header | The spec heads its Extension part "IPTC Extension schema **1.8** specifications", while §4 and the TechReference say **1.9** (adds the four AI properties) | Editorial inconsistency in the spec; PhotoWorkflow cites Extension 1.9 |

### C. Requirements that were missing

**C1–C7: from the specification**

| # | Requirement | Source |
|---|---|---|
| C1 | **IIM value formats that differ from XMP:** Intellectual Genre is written to IIM 2:04 **prefixed `000:`** (the 64-byte limit applies to the code only); Subject Code is written to IIM 2:12 **prefixed `IPTC:`**. When reading, the prefixes are removed | §7.16, §7.23 |
| C2 | **Date Created in detail:**<br>(a) Truncated dates (year, or year and month) must be possible, both in the editor and in storage. IIM 2:55 then uses `00` for an unknown month or day (`19180000`).<br>(b) Without a time, the time part stays empty. A system that needs a time uses 00:00:00, but does not store it.<br>(c) When reading, IIM 2:55 and 2:60 are **merged** into the XMP value.<br>(d) The first value may be suggested from Exif DateTimeOriginal + **SubSecTimeOriginal** + OffsetTimeOriginal.<br>(e) IIM has **no sub-seconds**: they are dropped for IIM. The IIM time is written as `hhmmss±hhmm` | §7.10; Mapping Guidelines "Exif Note on Date Created" |
| C3 | **Creator ↔ Image Creator (PLUS):** "Synchronising this Creator property with PLUS' Image Creator Name **should be supported by software**". The Mapping Guidelines' Exif notes expect both to hold the same value | §7.6 Note 1; Mapping Guidelines "Exif Note on Image Creator" |
| C4 | **Deprecated IIM datasets** Urgency (2:10), Category (2:15) and Supplemental Categories (2:20) are still synchronised with `photoshop:Urgency`, `photoshop:Category` and `photoshop:SupplementalCategories`, outside IPTC Core. They are **kept, and kept in sync if present**, but not offered for editing. The Category → Subject NewsCode table (§9.2) can be used for a migration aid (optional, not planned) | §9.1, §9.2 |
| C5 | **UI rules that are part of the standard:**<br>- Alt Text: warn above 250 characters.<br>- Alt Text and Extended Description appear together.<br>- Keywords allow phrases (never split on spaces).<br>- Creator's Contact Info appears as one group.<br>- Image Registry Entry: both sub-fields shown and required.<br>- GPS altitude may be one signed field, mapped to GPSAltitude + GPSAltitudeRef.<br>- The panels are called "IPTC Core" and "IPTC Extension".<br>- The spec's labels and help texts are the basis for the (translated) UI texts. | §5.1, §6, §7.1, §7.7, §7.13, §7.18, §10.1, §11.21, §12.10.4 |
| C6 | **Country Code (also inside the Location structure):** 2 or 3 letters (ISO 3166-1). The scheme "would have to be derived from the length of the value string" | §7.5, §12.10.2 |
| C7 | **Image Region with the unit "pixel":** when the image is rescaled (a future export with resizing), the coordinates and radius must be rescaled too | §12.7.2 |

**C8–C13: from the Mapping Guidelines 2023.1 (Exif 2.3x and Exif 3.0)**

| # | Requirement | Source |
|---|---|---|
| C8 | **Creator ↔ Exif:**<br>- IPTC → Exif: "Copy the value of IPTC Creator to Exif **Photographer** (Exif 3.0, 0xA437) **and Artist**". For mapping, Artist holds **a single name** (Exif 2.3x note; Exif 3.0: Artist contains "only a single name").<br>- Exif → IPTC: Photographer if it has a value, else Artist.<br>This **contradicts** the earlier §4 (MWG via ExifTool: Artist = all creators joined with "; ") | "Exif Note on Creator" |
| C9 | **Encoding of Exif text tags:**<br>- Exif 2.3x: only ASCII or UTF-8 bytes (the MWG advice, formally a violation of Exif).<br>- Exif 3.0: free-text tags are "ASCII **or** UTF-8"; a non-ASCII value must use the **UTF-8 data type**.<br>Affects ImageDescription, Copyright, Artist, Photographer, ImageEditor and CameraOwnerName | "Exif Note on the data types ASCII and UTF-8" |
| C10 | **Copyright Notice ↔ Copyright (0x8298):**<br>- Exif 2.3x allows a photographer and an editor part (NUL-separated, or prefixed "Photographer:" / "Editor:").<br>- Exif 3.0: copy both ways, UTF-8 type.<br>Reading must handle a NUL-separated editor part | "Exif Note on Copyright Notice" |
| C11 | **Contributor ↔ Exif ImageEditor (0xA438, Exif 3.0):**<br>- ImageEditor → a Contributor with the role URI `https://cv.iptc.org/newscodes/contentprodpartyrole/contentEditor`.<br>- The reverse direction uses only that role, and never copies to Artist | "Exif Note on Contributor" |
| C12 | **Digital Image GUID ← Exif ImageUniqueID (0xA420), one way only:**<br>- Exif 3.0 (a UUID): copying "is highly recommended"; the IPTC value is then not changed.<br>- Exif 2.3x: copy only if the value is globally unique.<br>- Never IPTC → Exif. The XMP property is "Internal" (generated, not user-entered) | "Exif Note on Digital Image GUID"; spec §11.12 |
| C13 | **What must *not* be mapped:**<br>- GPS → Location Shown: never.<br>- GPS → Location Created: only when the GPS was recorded by the camera, not set by a geocoding service.<br>- Image Region ↔ Exif SubjectArea: never (this guideline was withdrawn in 2023) | Exif Notes on Location Created / Location Shown / Image Region |

**C14: Schema.org**

| # | Requirement | Source |
|---|---|---|
| C14 | Schema.org mappings (`creator`, `copyrightNotice`, `creditText`, `dateCreated`, `caption`, `license` ← Web Statement of Rights, …, all on an `ImageObject`) | Mapping Guidelines. **Not relevant now**: only if PhotoWorkflow ever publishes web galleries |

### D. Still open (sources not available or not read)

| # | Item | Needs |
|---|---|---|
| D1 | Exact Lang Alt processing rules (`x-default` alongside a language entry) | ISO 16684-1 / XMP Part 1 |
| D2 | Sidecar naming rule (`<basename>.xmp`) | Adobe XMP Part 3 (§5 of this analysis) |
| D3 | IIM CodedCharacterSet 1:90 wording; IIM record structure | IIM 4.2 PDF (link in spec §16) |
| D4 | Allowed URI values of Model Release Status, Property Release Status, Minor Model Age Disclosure and Data Mining | PLUS specification (spec §11: "must be one of the URIs listed in the Controlled Vocabulary section of the PLUS specification") |
| D5 | IIM/XMP precedence and digest rule | ~~No longer published~~ **Read in MWG 2.0 (§9).** One owner decision remains (§9 E1) |
| D6 | Whether the test cameras write Exif ImageUniqueID, Photographer and ImageEditor | Read the test files with ExifTool before C8/C12 are implemented |

### E. What this changes in PhotoWorkflow (summary)

- **Already conformant:**
  - Rating storage (B3).
  - Contact info as flat Text values (B5).
  - XMP primary, sidecar for manufacturer RAW, DNG embedded (owner's decisions; the IPTC
    documents do not contradict them).
  - Cutting IIM text at UTF-8 boundaries (A5).
- **Must be added before or with the metadata implementation:** C1, C2, C3, C4, C5, C6, B1, B2,
  B3 (reading rules), C9 and C10.
- **Exif overlap rules to correct:** C8 (single name in Artist; Photographer too) and C11 to C13.
  They replace the MWG-based table in §4.
- **Later features:** C7 (export with resizing), C14 (web publishing).

### F. Proposed ADR 0007 changes (for the owner's approval; ADR not edited)

> **Superseded 2026-11-04:** applied in ADR 0007 revision 2 (see §10). Kept as a record.

1. **D3:**
   - Replace "MWG 2.0, as cited by the IPTC spec §5.3.4; to be verified" with: "no IPTC rule exists
     (spec §16); owner's decision".
   - Add the C2(c) merge of IIM 2:55 + 2:60.
2. **D4:** add the IIM prefixes (C1), the IIM date and time formats (C2), and the preservation and
   sync of the deprecated IIM datasets (C4).
3. **D6:** apply the IPTC Mapping Guidelines instead of MWG:
   - Artist gets **one name** (the first Creator).
   - Photographer (Exif 3.0) is updated when present.
   - Copyright uses the UTF-8 type rules (C9, C10).
   - ImageUniqueID → Digital Image GUID, one way (C12).
   - No GPS → Location Shown, and no SubjectArea mapping (C13).
   - Still "only if the Exif field already exists", and **DateTimeOriginal is still never changed**.
     The guideline describes a mapping in both directions, but it is "not mandatory"; keeping the
     capture time untouched is a deliberate choice and is documented as such.
4. **D7:**
   - Location Created is a single value (B2).
   - Image Creator, Copyright Owner and Licensor have at most 3 entries (B1).
   - Offer Creator → Image Creator Name synchronisation (C3).
5. **D8:**
   - Rating: 0 = unrated, real numbers accepted when reading (B3).
   - Alt Text: warn above 250 characters (C5).
   - Truncated dates (C2a, A4).
   - Country Code: 2 or 3 letters (C6).
   - Image Registry Entry: both sub-fields (C5).
   - ExifTool ≥ 12.67 checked at start (A7).

## 9. MWG Guidelines for Handling Image Metadata 2.0 (read 2026-11-04)

**Source:** `D:\PhotoWorkflowData\references\mwg_guidance.pdf`, supplied by the owner.
- Title "Guidelines For Handling Image Metadata", version 2.0, Metadata Working Group, October
  2010; 73 pages; SHA-256 `3c7cb408…2c15`.
- It is the document the IPTC spec cites (§5.3.4, §16) and says is no longer published. Its
  licence requires its copyright notice on copies; it is kept as a local reference only.
- MWG covers JPEG, TIFF and PSD. **It does not discuss DNG** ("GIF, PNG, DNG, JPEG XR … will not
  be discussed").
- MWG uses "Creator / Changer / Consumer" for the roles of software; PhotoWorkflow is a
  **Changer** and a **Consumer**.
- Everything below is **[read]**, with its MWG section.

### A. Confirms the design

| # | MWG rule | PhotoWorkflow |
|---|---|---|
| M1 | Creator: "SHOULD NOT create IPTC-IIM unless backward compatibility … is a requirement". Changer: "If IPTC-IIM is already in the file, a Changer SHOULD write data back … in both XMP and IPTC-IIM – otherwise only XMP SHOULD be written" (§4.2.3.2) | **Exactly** the owner's decision (IIM kept in sync if present; created only with the opt-in setting) |
| M2 | IIM "MUST be written as UTF-8, and MUST include a 1:90 DataSet". A Changer SHOULD convert existing non-UTF-8 IIM to UTF-8 (§4.2.3.2, text encodings) | ADR 0007 D4 (convert on first write, tell the user) |
| M3 | The checksum (G7): MD5 of the whole IIM block in PSIR 1061, always updated when both are present (§4.2.3.2) | ADR 0007 D4 "digest updated" |
| M4 | Rating: only in `xmp:Rating`, a float −1.0, 0.0…5.0.<br>- Display: round to the closest integer.<br>- "MUST only change the value once the user has changed the rating".<br>- Out-of-range values are clamped (> 5 → 5, < −1 → −1).<br>- Integers may be stored.<br>(§5.5) | ADR 0004 + §8 B3. **Add:** clamp on display; never rewrite a fractional rating unless the user changes it |
| M5 | Keywords ↔ IIM 2:25 (64 bytes each). "Keyword lists SHOULD be completely replaced while reconciling". XMP qualifiers on keywords SHOULD be preserved (§5.1) | Template Replace/Append work on the XMP list; IIM gets the whole list rewritten. Qualifiers are kept (D4 "nothing else removed") |
| M6 | Description: only `dc:description["x-default"]` is mapped to Exif/IIM (§5.2) | D4/D6: `x-default` written, other languages kept |
| M7 | Copyright: the copyright URL goes to `xmpRights:WebStatement`. Exif Copyright may hold creator and editor rights separated by NUL; in other formats they MAY be joined with a line feed (§5.6) | Same as §8 C10. **Reading rule:** NUL → line feed |
| M8 | Where to read and write:<br>- JPEG: XMP in APP1, IIM in APP13 PSIR 1028, Exif in APP1 directly after SOI; "All other occurrences SHOULD be ignored".<br>- TIFF: IFD0 tags 34665 (Exif), 33723 (IPTC), 700 (XMP); the checksum is in PSIR 34377.<br>(§4.2.3.4) | Matches §5 of this analysis. ExifTool writes these locations |

### B. New requirements

| # | Requirement | MWG |
|---|---|---|
| M9 | **Time zones:**<br>- "A Consumer MUST NOT arbitrarily add a time zone"; when Exif DateTimeOriginal (zone-less) is taken into XMP, "use a zone-less form".<br>- "A Changer MUST NOT implicitly add a time zone when editing values".<br>- Never convert to the computer's time zone for display.<br>- Store "local+offset", not "Zulu".<br>So the offset only comes from OffsetTimeOriginal (Exif 2.31, after MWG), or from the user. **This corrects ADR 0007 D8** ("when a time is given, the time zone is recorded"): it is recorded **only when known**. | §4.2.3.4 "Time-zone handling" |
| M10 | **Text encoding when reading** (IIM without 1:90, Exif ASCII):<br>- all bytes < 128 → ASCII;<br>- valid UTF-8 → UTF-8;<br>- otherwise a fallback encoding (user's locale / setting) or ignore.<br>A Consumer SHOULD check whether Exif text is valid UTF-8. BOMs are removed from values | §4.2.3.1, §4.2.3.4 |
| M11 | **Empty values:** "A string property that is comprised of only spaces or only nul characters MUST be treated as non-existent" | §4.2.3.3 |
| M12 | **Exif ↔ XMP writing:**<br>- "When the file format supports both Exif and XMP, a Changer SHOULD update both forms of a value. **If only one form is updated, an existing value in the other form MUST be removed**".<br>- Keep the Exif byte order.<br>- Do not duplicate Exif/TIFF device tags into XMP `exif:`/`tiff:` when the file has native Exif.<br>- No Exif checksum in XMP. | §4.2.3.1 |
| M13 | **Creator ↔ Exif Artist:** recognise the "; " separator between names, with **quoting rules** (quotes around names that contain "; " or begin with a quote; doubled inner quotes) | §5.7 |
| M14 | **Hierarchical keywords:** MWG defines `mwg-kw:Keywords` (`http://www.metadataworkinggroup.com/schemas/keywords/`). Consumers "MUST preserve all nested information". (Lightroom uses `lr:hierarchicalSubject`; both must be kept untouched) | §5.10 |
| M15 | **GPS:** the Exif GPS tags 1–6 hold the **camera** location (Location Created); 19–26 (GPSDest…) the **subject** location (Location Shown) | §5.8.5 |
| M16 | **Date mapping:** Exif DateTimeDigitized ↔ IIM 2:62/2:63 ↔ `xmp:CreateDate`; Exif DateTime ↔ `xmp:ModifyDate` | §5.3 |

### C. Where MWG and the newer IPTC Mapping Guidelines disagree

| Topic | MWG 2.0 (2010) | IPTC Mapping Guidelines 2023.1 | Effect |
|---|---|---|---|
| Creator → Exif Artist | All creators, joined with "; " and quoted (M13) | **A single name**; also Exif 3.0 Photographer (§8 C8) | The newer IPTC guideline wins for **writing**. When **reading** a multi-name Artist (older files), the MWG separator and quoting rules are used to split it |
| Exif encoding | UTF-8 in ASCII-type tags (formally a violation) | Exif 3.0: set the UTF-8 data type for non-ASCII (§8 C9) | Exif 3.0 files: UTF-8 type; older files: the MWG practice |

### D. Owner decisions needed

> **Decided 2026-11-04 (§10):** E1 as MWG recommends, plus a stricter rule when no checksum
> exists; E2 XMP primary with conflicts shown.

| # | Question | MWG says | Today (ADR 0007 D3) |
|---|---|---|---|
| E1 | **IIM edited by an XMP-unaware tool** (checksum differs) | **Per property:** the IIM value wins where it differs from the IIM value predicted from XMP (§4.2.3.2) | XMP wins; the property is shown as a conflict, and the user can take over the IIM value |
| E2 | **Exif vs XMP conflict** (both present, values differ) for Description, Copyright, Creator and Date | "In general, a Consumer SHOULD prefer the values from Exif" (§4.2.3.1). Between Exif and IIM: Exif if the checksum matches or is missing, else IIM (§4.2.3.3) | XMP wins; Exif and IIM only fill gaps |

**Recommendation:**

- **E1:** follow MWG, which avoids losing a legacy tool's edit silently, and keep the conflict
  marker as information.
- **E2:** keep XMP primary (the owner's rule). In practice Exif and XMP only differ when a tool
  wrote one of them alone, and M12 forbids that for compliant tools. Show the conflict as for E1.

Both are the owner's call; ADR 0007 is not changed until then.

### E. Proposed ADR 0007 changes (in addition to §8 F; not applied)

> **Superseded 2026-11-04:** applied in ADR 0007 revision 2 (see §10). Kept as a record.

1. **D3:** E1 and E2 as decided by the owner; M10 and M11 (encodings, empty values).
2. **D4:** M12 (update or remove both Exif and XMP forms), M14 (keep `mwg-kw` and
   `lr:hierarchicalSubject`), and M5 (IIM keyword list replaced as a whole).
3. **D6:**
   - Reading Exif Artist splits names with the MWG separator and quoting rules (M13).
   - Writing follows the IPTC Mapping Guidelines (one name).
   - GPS tags 1–6 → Location Created only (M15, §8 C13).
4. **D8:**
   - Time zone recorded only when known, never invented (M9).
   - Rating clamped when displayed and written back only on a user change (M4).
5. **D2:** DNG is outside MWG's scope; the owner's decision (embedded) is unaffected.

## 10. Owner decisions of 2026-11-04 and their rationale (→ ADR 0007 revision 2)

The owner approved the following on 2026-11-04. Each line says whether the rule **is required by
a standard** or is **a product decision**, and why. The full rules are in ADR 0007 (section given).

### A. Decisions

| # | Decision | Standard or product | Rationale | ADR |
|---|---|---|---|---|
| 1a | IIM checksum matches → XMP has precedence | **Standard** [MWG §4.2.3.2 scenario 3] | A matching checksum proves that the last writer knew both forms | §5.1 |
| 1b | IIM checksum differs → IIM has precedence **per differing property** (compared with the IIM value predicted from XMP) | **Standard** [MWG §4.2.3.2 scenario 4] | A tool that only knew IIM changed the file; that edit must not be lost | §5.1 |
| 1c | No checksum → **conflict, the user decides** | **Product ≠ MWG** (MWG: prefer XMP) | Nothing shows which tool wrote last; preferring XMP could hide an older tool's edit | §5.1 |
| 1d | Conflict markers kept and shown until PhotoWorkflow brings the forms in line | Product | Changes made outside PhotoWorkflow must be visible | §5.1 |
| 1e | Existing IIM is preserved and written back (UTF-8, checksum updated) | **Standard** [MWG §4.2.3.2] | Older systems that read the file keep working | §5.2 |
| 1f | New IIM only with the setting "Skapa IPTC-IIM för kompatibilitet med äldre system" | **Standard** [MWG] / **Product ≠ IPTC §5.3.4** ("should … both") | IIM has byte limits and old encodings; current tools read XMP; the setting covers workflows that need IIM | §5.2 |
| 2a | XMP is primary for the mapped fields; every Exif/XMP difference is detected and shown | **Product ≠ MWG §4.2.3.1** ("prefer Exif") | Owner's choice of XMP as primary; Lang Alt, lists and structures exist only in XMP; compliant tools keep both forms equal | §6.2 |
| 2b | JPEG/TIFF: existing Exif overlap fields are synchronised | **Standard** [MWG §4.2.3.1: update both forms, or remove the other], mapping per [IPTC-MG] | Prevents Exif and XMP from diverging | §6.3 |
| 2c | Manufacturer RAW Exif is preserved, never written; differences shown as conflicts | Product | Originals are never changed | §6.4 |
| 3a | DNG: embedded, never a sidecar, Xiaomi 17 Ultra included | **Product** (no standard covers DNG; MWG excludes it) | Open TIFF-based format with a place for XMP; a sidecar would diverge | §4 |
| 3b | ARW, RW2 and other manufacturer RAW: sidecar only, original never changed | Product | Proprietary formats; the owner's requirement | §4 |
| 3c | JPEG/TIFF: embedded | Product, consistent with [MWG §4.2.3.4] | Standard locations exist | §4 |
| 3d | Safe write: temporary copy → verification → atomic replace → journal | Product | A write must never leave a partly written original (crash, full disk, lock, tool error) | §9 |
| 4a | DateTimeOriginal never changed automatically | Product (IPTC-MG maps both ways, but is "not mandatory") | The capture time is evidence; it is changed only by an explicit future feature (Q12) | §7 |
| 4b | A time zone is never invented; it is stored only when known | **Standard** [MWG §4.2.3.4, MUST] | A guessed zone moves the time | §7 |
| 4c | Truncated dates; IIM `00` for an unknown month or day | **Standard** [IPTC §7.10, §14.2] | Historic and approximate dates | §7 |
| 5a | `mwg-kw:Keywords` and `lr:hierarchicalSubject` kept unchanged | **Standard** [MWG §5.10, MUST] for `mwg-kw`; product for `lr:` | No loss of keyword trees made in other tools | §8 |
| 5b | Rating kept with decimal precision; rounded only for display; written only on a user change | **Standard** [MWG §5.5; IPTC §11.19] | Other tools may store fractions | §8 |
| 5c | Urgency, Category, Supplemental Categories kept and synced if present | **Standard** [IPTC §9.1]; not editable / never created: product | Deprecated but still in use | §5.5 |
| 5d | IIM written as UTF-8 with 1:90; older encodings read per MWG | **Standard** [MWG, MUST for writing] | Correct non-ASCII text | §5.4 |
| 5e | Keywords are phrases, never split at spaces | **Standard** [IPTC §7.18] | | §8 |
| 5f | Clearing removes the value consistently from XMP, IIM and the Exif overlap field; RAW never changed | **Standard** [MWG §4.2.3.1] for the forms; product for RAW | A cleared value must not reappear from another form | §8 |

### B. Consistency of IPTC 2025.1, IPTC-MG 2023.1 and MWG 2.0 in this architecture

- **No rule contradicts a MUST of any of the three documents.**
- Deliberate deviations, all at SHOULD level, are marked [Product ≠ std] in the ADR:
  1. IPTC §5.3.4 "should save … both IIM and XMP" (1f);
  2. MWG "prefer XMP" when no checksum exists (1c);
  3. MWG "prefer Exif" (2a).
- **Where the standards disagree with each other:** Creator → Exif Artist. MWG joins all names
  with "; ", IPTC-MG writes one name (and also Photographer). The newer IPTC-MG is followed for
  writing; MWG's rules are used to split old multi-name values when reading.
- **Not covered by any of the three:** DNG storage (product decision), RAW sidecars (MWG and
  IPTC do not define them; naming per Adobe XMP Part 3, not read: Q8).
- Full table: ADR 0007 §13.

### C. Unresolved interpretations and open questions

These are listed with proposals in ADR 0007 §14 and must be answered before the ADR is approved:

- **Interpretations of the decisions:**
  - Q1 writing with an unresolved no-checksum conflict;
  - Q2 clearing in a RAW sidecar while camera Exif has a value;
  - Q4 whether "JPEG/TIFF sync" includes Exif in DNG;
  - Q5 a foreign sidecar next to a DNG;
  - Q6 whether missing Exif fields are created;
  - Q13 how "backward compatibility requires IIM" is detected.
- **Gaps in the sources read:**
  - Q3 IIM 2:60 without a zone (IIM 4.2 not read);
  - Q7 Lang Alt (XMP Part 1);
  - Q8 sidecar naming (XMP Part 3);
  - Q9 PLUS vocabularies;
  - Q10 fallback encoding.
- **Material and scope:** Q11 level-C samples; Q12 manual camera-time correction.
