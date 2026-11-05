# External references

## Normative: metadata (owner's decision, 2026-11-04)

PhotoWorkflow follows the IPTC standards for photo metadata. It does **not** imitate any other
product's internal implementation. Where other tools are mentioned (Lightroom, Photo Mechanic), it
is only to stay interoperable with files they read and write, based on their public documentation
(clean-room level D).

| Document | Version | URL |
|---|---|---|
| IPTC Photo Metadata Standard (overview) | – | https://iptc.org/standards/photo-metadata/ |
| IPTC Photo Metadata Standard – specification | **2025.1** rev 1, 2025-11-26 (IPTC Core 1.5, IPTC Extension 1.9) | https://www.iptc.org/std/photometadata/specification/IPTC-PhotoMetadata (→ `IPTC-PhotoMetadata-2025.1.html`) |
| IPTC Photo Metadata TechReference (machine-readable property list) | 2025.1, 2025-11-03 | https://iptc.org/std/photometadata/specification/iptc-pmd-techreference_2025.1.json (also `.yml`); docs: https://iptc.org/std/photometadata/documentation/techreference/ |
| IPTC Photo Metadata User Guide (intended meaning and use of the fields; Embedded Metadata Manifesto) | current | https://www.iptc.org/std/photometadata/documentation/userguide/ (readable as `…/userguide/index.html`) |
| IPTC Photo Metadata Mapping Guidelines (IIM / XMP / Exif / schema.org) | current | https://www.iptc.org/std/photometadata/documentation/mappingguidelines/ |
| IPTC Information Interchange Model (IIM) | 4.2 (2014; frozen since 1997) | https://iptc.org/standards/iim/ |
| IPTC developers' guide to reading and writing photo metadata | 2023 | https://iptc.atlassian.net/wiki/spaces/PMD/pages/606601427/ |
| XMP (ISO 16684-1) | – | referenced by the IPTC specification |

## Informative

| Document | Use | URL |
|---|---|---|
| Metadata Working Group, *Guidelines for Handling Image Metadata* 2.0 (2010) | IIM ↔ XMP ↔ Exif reconciliation (IPTC spec 5.3.4 refers to it; no longer maintained since 2018) | archived copy cited by ExifTool: https://web.archive.org/web/20180919181934/http://www.metadataworkinggroup.org/pdf/mwg_guidance.pdf |
| ExifTool tag documentation (IPTC, XMP, MWG) | How the tool we use implements IIM, XMP and MWG | https://exiftool.org/TagNames/IPTC.html, https://exiftool.org/TagNames/MWG.html |
| Adobe XMP Specification Part 3 (storage in files, sidecars) | Sidecar and embedding rules | https://developer.adobe.com/xmp/docs/ |
| Adobe Lightroom Classic help: metadata, keywords | Interoperability | https://helpx.adobe.com/lightroom-classic/help/metadata-basics-actions.html, …/keywords.html |
| Camera Bits Photo Mechanic help (file formats, IPTC/XMP preferences, metadata template, general preferences) | Interoperability | https://docs.camerabits.com/support/solutions/articles/48000361354, …/48001146198, …/48000207623, …/48001144865 |

Which parts of these documents were actually read for the metadata analysis is recorded in
`docs/analyses/2026-11-04-iptc-metadata-requirements.md`.
