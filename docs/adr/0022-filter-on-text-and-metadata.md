# ADR 0022: Filtering on text and metadata

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0014 (the star and label filter, the rule that an image leaves the list when it stops
    matching, the visible filter); its §3.2 left filters on file type, date, camera, keyword and
    other metadata out, which this ADR takes up; its §15 (a bar of toggle buttons) stays a separate
    idea;
  - ADR 0007 (the IPTC properties, the metadata worker), ADR 0017 U2 (Info from `pw_raw::inspect`)
    and U14 (capture times read in the background for sorting);
  - ADR 0020 (the corrected capture time, the camera with its serial number);
  - GUI_REQUIREMENTS GR14 (English texts, IPTC's English names).

## 1. Context

- The filter of ADR 0014 knows stars and labels only. In a large folder the user also needs to
  find images by text (a name, a keyword, a caption), by camera or lens, by exposure or date, and
  the images still missing a caption or keywords.
- Ratings and labels are read for a whole folder in one ExifTool call (`read_ratings`: file name,
  `XMP-xmp:Rating`, `XMP-xmp:Label`); more XMP properties can be read in the same call at little
  cost. Exif values are read without ExifTool by `pw_raw::inspect`, as for sorting by capture time.
- Opening a folder must stay as fast as today, so the extra values are read only when a filter
  needs them.

## 2. Decision

### 2.1 Fields

- **Exif** (S-Q4): Camera (make and model, a body with a serial number a camera of its own,
  ADR 0020 C1), Lens, Focal Length, Aperture, Exposure Time, ISO, Capture Date (the corrected one
  where there is one, ADR 0020) and File Type (RAW, JPEG, TIFF, DNG).
- **IPTC** (S-Q5): Title, Headline, Description, Keywords, Person Shown, Creator, Copyright Notice,
  City, Country and Location (Sublocation), with IPTC's English names (GR14). Also "has no Title",
  "has no Description", "has no Keywords" and "has no Copyright Notice". More fields can be added
  later.
- **File:** the file name.

### 2.2 Quick search

- A search box in the command bar (S-Q1) searches the file name, Title, Headline, Description,
  Keywords, Person Shown, Location, City, Country and Creator.
- Case does not matter and part of a word is enough; several words must all be found (AND), each
  in any of the fields.
- **Ctrl+F** puts the cursor in the box; **Escape** in the box empties it.

### 2.3 The extended filter

- A window **"Filter…"** (S-Q2) where the user adds one row per condition: a field, a condition
  and a value. The conditions follow the field: contains / is / is not for text, = for choices,
  ≥ / ≤ / between for numbers and dates, and "has no …" for the IPTC fields of §2.1.
- **Every row must match** (AND). There is no OR between rows; within one row, several values of
  a choice (for example two cameras) can be picked, and an image matches any of them (S-Q3).
- **Value lists** (S-Q10): "=" on Camera, Lens, Keywords, City and Country offers the values found
  in the folder, each with its number of images.

### 2.4 Reading the values

- The IPTC values are read with the same kind of batch call per folder as the ratings: either
  `read_ratings` extended or a new request beside it in the metadata worker (decided in S2). The
  Exif values are read as for sorting (U14-Q4), visible images first (S-Q6).
- **Only when needed** (S-Q7): the values are read when a text or metadata condition is active,
  not when a folder is opened. They are kept in memory for the tab until its folder is listed
  again.
- An image whose values are not read yet counts as **unknown**: it is hidden by an active text or
  metadata condition until its values are read. The status line shows "Reading metadata {i} of
  {n}", and the list is filtered once more when the reading is done (S-Q6).
- **RawStride's own writes** (`M`, Apply to All Marked, ADR 0020) update the tab's values in
  memory; an image that no longer matches leaves the list by ADR 0014's point 2 (S-Q8).

### 2.5 Combination and display

- Stars, label, the search box and the extended filter all apply together (AND). "Show All" clears
  all of them (S-Q9).
- An active filter is always visible (ADR 0014 point 4): the outlined control and the status line
  as today, and "Filter: {n} conditions" in the command bar for the extended filter.
- Rejected images follow the star filter as today; the search box and the extended filter remove
  nothing on their own (S-Q12).

### 2.6 Saved filters

- An extended filter can be saved under a name in `settings.txt` and chosen in View ▸ Filter
  (S-Q11).
- The active filter is not restored after a restart: the program starts with every image shown
  (ADR 0014 Q2).

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| S-Q1 | Quick search | A box in the command bar over file name and nine IPTC fields; case-blind, part of a word, all words (AND); Ctrl+F, Escape empties |
| S-Q2 | Extended filter | A "Filter…" window with one row per condition (field, condition, value); every row must match |
| S-Q3 | OR | Not between rows; several values in one row of a choice |
| S-Q4 | Exif fields | Camera (with serial number), Lens, Focal Length, Aperture, Exposure Time, ISO, Capture Date (corrected), File Type |
| S-Q5 | IPTC fields | Title, Headline, Description, Keywords, Person Shown, Creator, Copyright Notice, City, Country, Location; "has no" for Title, Description, Keywords, Copyright Notice |
| S-Q6 | Reading | IPTC in the folder's batch call, Exif as for sorting; unknown until read, "Reading metadata {i} of {n}", filtered again when done |
| S-Q7 | When | Only while a text or metadata condition is active; kept for the tab until the folder is listed again |
| S-Q8 | Changed values | RawStride's own writes update the values; an image that stops matching leaves the list (ADR 0014 point 2) |
| S-Q9 | Combination | Everything AND; "Show All" clears everything; the filter visible, "Filter: {n} conditions" |
| S-Q10 | Value lists | For Camera, Lens, Keywords, City, Country, with counts |
| S-Q11 | Saved filters | Named, in `settings.txt`, chosen in View ▸ Filter; the active filter not restored at start |
| S-Q12 | Rejected images | Follow the star filter as today |
| S-Q13 | Steps | S1–S5 as §4 |
| S-Q14 | Not included | As §5 |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after S3 and after S5.

| Step | Content |
|---|---|
| **S1** | The filter model in plain Rust: fields, conditions, matching (case-blind, words, numbers, dates, "has no"), unknown values; tests |
| **S2** | Reading: the IPTC values in the folder's batch call in `pw-metadata` (the request decided here), the Exif values in the tab, both kept per tab and updated by RawStride's own writes |
| **S3** | The search box in the command bar and Ctrl+F; the status line while reading; texts and glossary |
| **S4** | The "Filter…" window with its rows and value lists; "Filter: {n} conditions"; "Show All" |
| **S5** | Saved filters in `settings.txt` and View ▸ Filter |

## 5. Not included

- OR between conditions and groups of conditions.
- Regular expressions.
- Searching across folders, or in a catalogue (`pw-catalog`, ADR 0017 §7.2).
- The bar of toggle buttons (ADR 0014 §15), which needs its own amendment.
- GPS position (the GPS ADR).
- Hierarchical keywords (kept but not edited, ADR 0007 §8).

## 6. Consequences

- Large folders can be narrowed by any common text or camera value, and the images still missing
  a caption or keywords found.
- A folder opens as fast as today; the cost of reading the values is paid only while such a filter
  is on.
- The tab holds a second set of read values beside the ratings, kept in step with RawStride's own
  writes.
