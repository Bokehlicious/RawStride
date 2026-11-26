# ADR 0022: Filtering on text and metadata

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit. **Done 2026-11-26:** S1–S4 hand-checked by the
  owner; S5 (saved filters) dropped by the owner (§2.6).
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
  `read_ratings` extended or a new request beside it in the metadata worker (decided in S2: a new
  request, S2-Q1 in §3.1). The
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

- **Dropped by the owner, 2026-11-26** (before S5's questions were decided): a folder holds one
  shoot or one day, so a saved filter is of little use in a folder tab; the search box and the
  "Filter…" window cover a folder. Saved searches over many folders come as ADR 0030's smart
  collections (ADR 0030 §3.7 and §7, "later", K-Q7), which can build on this ADR's filter model. Before
  that, S-Q11 said: an extended filter can be saved under a name in `settings.txt` and chosen in
  View ▸ Filter.
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
| S-Q11 | Saved filters | Named, in `settings.txt`, chosen in View ▸ Filter; the active filter not restored at start. **Saved filters dropped by the owner, 2026-11-26** (§2.6): saved searches come as ADR 0030's smart collections; the active filter is still not restored at start |
| S-Q12 | Rejected images | Follow the star filter as today |
| S-Q13 | Steps | S1–S5 as §4 |
| S-Q14 | Not included | As §5 |

### 3.1 S2's questions (decided by the owner, 2026-11-25: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| S2-Q1 | The request for the IPTC values | A **new request** in the metadata worker beside `read_ratings`, which stays as it is (it runs at every folder opening, which must stay as fast as today). It reads the values the same way as the metadata window and Info (`read_metadata`'s reading and reconciliation of XMP and IIM, ADR 0007), but for many files per ExifTool call, so the filter sees exactly what the panel shows, also a JPEG with IIM values only. A plain XMP-only read was rejected: faster, but it could differ from the panel. Its time is measured on one of the owner's folders in S2's first task |
| S2-Q2 | Order and parts | Chunks of 50 images, the visible images first; each chunk's answer counts "{i} of {n}" up (shown by S3). **Amended by the owner, 2026-11-25** (found while S2's tasks were written): every image is read with ExifTool, also a manufacturer RAW without a sidecar, since the metadata window fills such a RAW's values from its Exif and its camera XMP (ADR 0007), and the filter must see what the window shows (S2-Q1). The proposal's shortcut (no call for a RAW without a sidecar) was dropped. **Chunk size amended by the owner, 2026-11-25** (after T336's timing on a folder of 79 RAW, DNG, JPEG and TIFF files: 50 images in one call 1.43 s, about 29 ms per image, against 30 ms per image read one by one, so a large chunk saves almost nothing but holds back the metadata window's read of the next image for up to 1.4 s, since the worker answers one request at a time): **chunks of 10 images** |
| S2-Q3 | Exif | The capture-time reader of the sort (`pw_raw::inspect`, U14-Q4) keeps the whole Exif summary, not only the date: one read per file serves the sort and the filter. File Type from the extension. Camera is "Make Model", or the model alone when it already begins with the make; no serial number until ADR 0020 C1. Capture Date is the embedded DateTimeOriginal, without a time zone; all zeros counts as no date. A file `pw_raw` cannot read has no Exif values (as the sort counts it as having no date) |
| S2-Q4 | A read that fails | The chunk is tried once more; if it fails again, its images count as **"could not be read"**: they stay unknown (hidden while a text or metadata condition is active), are not asked for again until the folder is listed again, and S3 shows their number in the status line. Not counted as having no values (they would wrongly show under "has no Title"). ExifTool answers per file, so one broken file fails only its own image |
| S2-Q5 | RawStride's own writes (S-Q8) | The written images are **read again from the file** with the same request (the editor's save, `M`, Apply to All Marked, a template at an import into an open tab), not filled in from what the user typed, so the values are exactly the file's, also after the IIM–XMP sync. A batch: all its images read again after it, in chunks, also those whose write failed (a batch stopped halfway has written some) |
| S2-Q6 | A read and a write at once; moves and deletes | An answer for an image written after the read was asked is dropped and the image read again after the write. A deleted image leaves the tab's values; an image moved to another folder leaves them too (the destination's tab reads it when needed). New files from the watcher or an import are unknown until read. A sidecar changed by another program is seen only when the folder is listed again (§2.4; `known-limits.md`) |
| S2-Q7 | Split and who | A chain of two tasks, both Codex Astra (concurrency): **S2a** in `pw-metadata` (the request, the reading in chunks, the timing) → **S2b** in `pw-loupe` (the tab keeps the values, the Exif reader extended, reading only while the tab's filter has a text or metadata condition, the reading again after own writes). The filter is per tab, as the star filter. Nothing changes on screen until S3; the hand check after S3 as §4. S2 moved to order 2d beside ADR 0027 W. A Codex task's work is brought to `feature/gui-workspace` as a patch (`git apply --3way`), not by copying its files, since S2b shares `main.rs` and `lib.rs` with ADR 0027's tasks (`docs/tasks/README.md`, "Codex tasks") |

### 3.2 S3's questions (decided by the owner, 2026-11-26: every proposal approved)

S3 was moved by the owner on 2026-11-26 to order 2d, right after ADR 0027 W5, with S4 and S5
(`docs/tasks/plans/adr-0018-0025-steps.md`).

| # | Question | Answer (as proposed) |
|---|---|---|
| S3-Q1 | The box | In the command bar after the label list, with the hint text "Search" while empty. The filter follows the typing after a pause of 0.3 s; Enter gives the keyboard back to the images (arrows and digits work at once, as after a filter choice, ADR 0014 §4.5). Per tab, as the star filter, emptied when the folder is opened again. A text of spaces only counts as empty (no search) |
| S3-Q2 | Ctrl+F | A new action "Search" in the key map (changeable, ADR 0017 U8), Ctrl+F by default, in the contact sheet and in the preview (the loupe and Compare). It puts the cursor in the box. Escape in the box empties it and leaves the box. (The owner's correction 2026-11-26: "the preview" is the loupe and Compare, not a window of its own.) |
| S3-Q3 | While the values are read | An image whose IPTC values are not read yet is hidden while the search is active (§2.4). The list is filtered again after each chunk (10 images, S2-Q2), so the matches come in one by one in the sort order; the current image stays current while it matches (ADR 0014 §2.3 otherwise) |
| S3-Q4 | The status line | "Reading metadata {i} of {n} …": `n` the images the star and label filter lets through, `i` those of them whose IPTC values are read or that could not be read. The search box needs the file name and IPTC only, so the Exif reads are not counted in S3; how the Exif and IPTC counts join is S4's. While nothing is shown yet, the grid shows the same text instead of "No images match the filter" |
| S3-Q5 | Images that could not be read | Hidden while the search is active (S2-Q4); once nothing is being read, the status line says "{k} images could not be read" (one: "1 image could not be read") as long as the search is active. They are read again only when the folder is listed again |
| S3-Q6 | The order of the reads | From the star and label filter's list (not from `visible`, where the unread images are hidden), starting at the current image and going forward, as the reads of today (`read_order`) |
| S3-Q7 | Marks | An image hidden only because its values are unknown keeps its mark (as ADR 0014 amendment 3 for unread ratings); an image hidden because it does not match is unmarked (ADR 0014 §2.8) |
| S3-Q8 | RawStride's own writes | An image written by RawStride (the editor's save, `M`, Apply to All Marked, a template, an import's template into an open tab) **keeps its old values until the new read has answered**, so it does not leave the list and come back; it leaves only if it does not match after the read (S-Q8, ADR 0014 point 2). A batch's images are read again in chunks after it (S2-Q5). This changes S2b's code, which dropped the values at once; S2-Q6's dropped answer also keeps the old values until the next read. New files from an import or the folder watcher are unknown until read |
| S3-Q9 | Show All and what is shown | Show All empties the search box too (S-Q9). A box that is not empty is outlined in the accent colour, as the other filter controls (ADR 0014 point 4). The position "3 / 12 (of 31)" and "No images match the filter ({total} in the folder)" as today |
| S3-Q10 | Split and who | A chain: **S3a** in `pw-loupe`, the plain-Rust part (the refilter with the search, the read order, the counts, the texts in a text module, the glossary) → **S3b** the shell (the box, Ctrl+F in the key map, the status line); both Opus (state); the owner's hand check of S1–S3 after S3b. Whether Codex or Claude does them is the owner's choice per task |
| S3-Q11 | The preview while the values are read (found while S3's tasks were written; decided by the owner 2026-11-26 as proposed) | While an active search's values are read, nothing may be shown in the preview for a moment (since S3-Q13 the search is typed in the contact sheet, but the preview can be left with nothing shown while reads go on, for example when the shown images stop matching after their read again). The preview **waits** while the search's reads go on and nothing is shown: it stays and shows "Reading metadata {i} of {n} …", as it waits for the ratings today (ADR 0014 Q7). It goes back to the contact sheet only when the reads are done and nothing matches (ADR 0014 §2.3) |
| S3-Q12 | A failed read after RawStride's own write (found while S3's tasks were written; decided by the owner 2026-11-26 as proposed) | When the read again after a write (S3-Q8) fails twice, the old values are dropped and the image counts as could not be read (hidden while the search is active), as S2-Q4 says for any read |
| S3-Q13 | Ctrl+F in the preview (found while S3's tasks were written; decided by the owner 2026-11-26 as proposed) | The preview shows no menu bar or command bar (ADR 0017 U3-Q1), so it has no search box. Ctrl+F in the preview (the loupe and Compare) goes back to the contact sheet and puts the cursor in the box; the preview gets no box of its own. This refines S3-Q2 |
| S3-Q14 | Marks while typing (found in the chain review of S3's tasks; decided by the owner 2026-11-26 as proposed) | Each 0.3 s pause applies the text typed so far, and an image it hides because it does not match is unmarked (ADR 0014 §2.8), also when the finished text would have kept it (typing "caz" and erasing the "z"). Accepted as a known limit (`known-limits.md`): a hidden image is never marked, so no action reaches an image the user does not see |
| S3-Q15 | Ctrl+A with nothing shown (found in the chain review; decided by the owner 2026-11-26 as proposed) | Ctrl+A does nothing while no image is shown, so it does not clear the marks kept for images whose values are unknown (S3-Q7) or whose ratings are being read (ADR 0014 amendment 3). This ends the known limit "Ctrl+A while nothing is shown clears the kept marks" |

### 3.3 S4's questions (decided by the owner, 2026-11-26: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| S4-Q1 | The window | A **modal** window "Filter…": the rows are edited freely, and the filter applies only at **OK**. **Cancel** (or Escape) keeps the filter as it was; **Clear** removes every row in the window (applied at OK). The list behind does not change while the window is open |
| S4-Q2 | Opening it | View ▸ Filter ▸ "Filter…"; a button in the command bar after the search box; a new action "Filter…" in the key map (changeable, ADR 0017 U8), Ctrl+Shift+F by default if it is free. In the preview (the loupe and Compare), which has no menu bar or command bar, the key goes back to the contact sheet and opens the window, as Ctrl+F does (S3-Q13) |
| S4-Q3 | The rows | Each row: a field (grouped File, Exif, IPTC, in that order), a condition that follows the field (§2.3, the conditions S1's model allows), and a value. "Add Condition" adds a row, a button on the row removes it. While a row has no value or an invalid one, OK is disabled and the row says what is missing, so what is counted is always what is shown |
| S4-Q4 | Numbers and dates | Exposure Time typed as "1/250" or "2" (seconds); Aperture as "2.8" or "f/2.8"; Focal Length in mm; ISO a whole number; a decimal comma is accepted. Dates as YYYY-MM-DD: "≤ a date" counts to the end of that day, "between" includes both days. Bounds typed in the wrong order are swapped. An image without the value (no ISO, no capture date) does not match a number or date condition |
| S4-Q5 | "is not" and a missing value | "City is not Stockholm" also matches the images without a City, as S1's model does; "is empty" (§2.1's "has no …") is the condition for the images without one |
| S4-Q6 | Value lists | "is one of" on Camera, Lens, Keywords, City and Country shows the folder's values, each with its number of images, as check boxes, several may be picked (S-Q3, S-Q10). Values that differ only in case count as one. File Type has a fixed list: RAW, JPEG, TIFF, DNG |
| S4-Q7 | Reading for the value lists | While the window has a row with a value list, its group (Exif or IPTC) is read for the whole folder (an extension of S-Q7: an open value list needs the values). The list grows as the chunks come in, with "Reading metadata {i} of {n} …" under it. A value already picked that is not in the folder stays, with the count 0 |
| S4-Q8 | The status line (left open by S3-Q4) | An image counts as done when every group the filter needs is read, or its IPTC values could not be read. `n` is the images of the star and label filter's list. Exif never fails as a whole (the file type is known from the extension). The images that could not be read are shown as S3-Q5 says |
| S4-Q9 | Marks and the current image | As for the search box (S3-Q7, S3-Q11): an image hidden because it does not match is unmarked, one hidden because its values are unknown keeps its mark; the preview waits while the values are read |
| S4-Q10 | What is shown | The button says "Filter…" with no rows and "Filter: {n} conditions" with rows ("Filter: 1 condition"), outlined in the accent colour as the other filter controls (ADR 0014 point 4). No tooltip: the conditions are seen in the window |
| S4-Q11 | Show All and tabs | Show All removes every row too (S-Q9). The rows are per tab and follow the search box at F5 and when the folder is opened again |
| S4-Q12 | Own writes, import and the watcher | As S3-Q8 and S3-Q12, nothing new. RawStride writes no Exif value before ADR 0020. New files are unknown until read |
| S4-Q13 | Texts | The IPTC fields' names come from `property_label` (`docs/tasks/README.md` rule 13); the Exif fields and the conditions (contains, is, is not, is one of, ≥, ≤, between, is empty) get texts of their own in a new text module; the glossary is updated |
| S4-Q14 | Split and who | A chain, all Opus (state): **S4a** plain Rust (the window's model, the parsing of numbers and dates, the value lists' counts, the texts) → **S4b** the shell (the window, the button, the menu item, the key, the status line's count) → **S4c** the value lists and the reading of the whole folder. Codex or Claude per task, the owner's choice. **The owner's hand check after S4** as well as after S5 (§4 amended) |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after S3 and after S4 (S4-Q14,
owner 2026-11-26). S5 was dropped by the owner on 2026-11-26 (§2.6), so ADR 0022 is done with S4's
hand check (2026-11-26).

| Step | Content |
|---|---|
| **S1** | The filter model in plain Rust: fields, conditions, matching (case-blind, words, numbers, dates, "has no"), unknown values; tests |
| **S2** | Reading: the IPTC values in the folder's batch call in `pw-metadata` (the request decided here), the Exif values in the tab, both kept per tab and updated by RawStride's own writes |
| **S3** | The search box in the command bar and Ctrl+F; the status line while reading; texts and glossary |
| **S4** | The "Filter…" window with its rows and value lists; "Filter: {n} conditions"; "Show All" |
| S5 | Dropped by the owner, 2026-11-26 (§2.6): saved filters in `settings.txt` and View ▸ Filter |

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
