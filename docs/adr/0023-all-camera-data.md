# ADR 0023: All camera data in the inspector

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0017 §2.1 and U2 (the inspector's Info and Metadata sections), U2-Q2 (Info's seven
    values), **amended by this ADR** (§2.5); U7c-Q3 (the histogram's cache rules); T103 (the rules
    for a panel that follows the current image);
  - ADR 0007 (the metadata worker and ExifTool; camera data is never edited, ADR 0017 §2.3.1);
  - ADR 0020 C1 (the body serial number).

## 1. Context

- The inspector's Info shows seven values from `pw_raw::inspect` (U2-Q2). The Metadata section
  shows the XMP and IPTC of the image and its sidecar. Nothing shows the rest of what the camera
  wrote: exposure program, metering, flash, focus, white balance, the camera's own maker notes,
  the colour profile.
- The metadata read with ExifTool (`read.rs`) reads XMP and a fixed list of about ten Exif and
  IPTC tags, without print conversion. No call reads every tag today.

## 2. Decision

### 2.1 Where

- A collapsible section **"All Camera Data"** in the inspector under Metadata, and in the
  preview's right column under Clipping (A-Q1). Collapsed at first; its state is remembered in
  `settings.txt`, apart for the inspector and the preview, as the other sections are.

### 2.2 Reading

- ExifTool reads every tag of the image file: JSON, every tag also when repeated, group 1 names,
  with print conversion so the values are readable (A-Q2). The request runs on the metadata worker.
- Read only while the section is open, by T103's rules for a panel that follows the current image:
  a 200 ms wait, at most one read at a time, old answers discarded (A-Q2).
- **Only the image file's tags**, maker notes, Composite, ICC and File included. The sidecar's XMP
  is already in the Metadata section and is not repeated (A-Q3).
- **Cache** (A-Q11): the last 64 lists in memory, by U7c-Q3's rules: another folder empties it, a
  changed file is read again.
- **Without ExifTool** the section shows the same text as the Metadata section does then (A-Q9).

### 2.3 Groups, names and values

- **Groups** are ExifTool's group 1 names, in a fixed order: File, IFD0 and ExifIFD, the camera's
  maker notes, Composite, then the others (XMP-…, IPTC, ICC_Profile, …). Each group can be
  collapsed (A-Q4).
- **Names and values** are ExifTool's English tag names and readable values as given (for example
  "1/250", "Auto"); the raw number is not shown (A-Q5).
- **Binary data** (previews, thumbnails, raw maker data) is shown as "(binary data, {n} bytes)" and
  never extracted (A-Q6).
- Nothing can be edited; camera data is never edited (A-Q8, ADR 0017 §2.3.1).

### 2.4 Search and copy

- A search box in the section filters on tag name and value, without regard to case (A-Q7).
- **"Copy"** puts the rows shown as text on the Windows clipboard, one tag per line
  (`{group} {name}: {value}`) (A-Q7).

### 2.5 More rows in Info (amends U2-Q2, owner 2026-11-17)

- Info adds **Exposure Compensation** and **Image Size**, from `pw-raw`, which reads both already;
  and the camera's **Serial Number** when ADR 0020 C1 is built. Everything else is in the full list
  (A-Q10).

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| A-Q1 | Where | A section "All Camera Data" under Metadata in the inspector and under Clipping in the preview; collapsed at first, remembered |
| A-Q2 | Reading | ExifTool, every tag of the image file with readable values, on the metadata worker, only while the section is open, by T103's rules |
| A-Q3 | What | Only the image file's tags (maker notes, Composite, ICC, File included); the sidecar's XMP stays in Metadata |
| A-Q4 | Groups | ExifTool's group 1 names in a fixed order, each collapsible |
| A-Q5 | Names and values | ExifTool's English names and readable values as given; no raw numbers |
| A-Q6 | Binary data | "(binary data, {n} bytes)", never extracted |
| A-Q7 | Search and copy | A search box over name and value; "Copy" of the rows shown to the clipboard |
| A-Q8 | Editing | None |
| A-Q9 | Without ExifTool | The Metadata section's text |
| A-Q10 | Info | Exposure Compensation and Image Size added; Serial Number after ADR 0020 C1; U2-Q2 amended |
| A-Q11 | Cache | The last 64 lists, by U7c-Q3's rules |
| A-Q12 | Steps | A1–A3 as §4 |
| A-Q13 | Not included | As §5 |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after A2.

| Step | Content |
|---|---|
| **A1** | The read in `pw-metadata`: every tag of one file with readable values and group names, binary values as their size; a new request to the metadata worker; tests on the test files (a RAW with maker notes, a JPEG, a TIFF, a DNG) |
| **A2** | The section in the inspector and the preview's right column: the reader with T103's rules and the cache, the groups, the search box, "Copy"; texts and glossary |
| **A3** | Exposure Compensation and Image Size in Info (and Serial Number after ADR 0020 C1) |

## 5. Not included

- Editing Exif or maker notes.
- Exporting the list to a file.
- Comparing the data of two images.
- RawStride decoding maker notes itself.
- Names of RawStride's own or Swedish tag names.
- GPS on a map (the GPS ADR).

## 6. Consequences

- Everything the camera wrote can be read next to the image, searched and copied, without another
  program.
- The cost is one ExifTool read per image, only while the section is open.
