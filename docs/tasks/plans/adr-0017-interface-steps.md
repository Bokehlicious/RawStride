# ADR 0017 (the interface): steps

ADR 0017 was accepted by the owner on 2026-11-12 and amended on 2026-11-13. Its steps are U0–U14
(§7); stage A, the usable version, is U1, U2 and U3 plus previous/next in today's `M` window (the
first part of U12, §7.1). Each step becomes one or more tasks here, written when the step before
it is accepted. Stage A changes the shell in `pw-loupe` only; the core crates are not touched.

## Stage B, first part: the metadata window (U10, U11, the rest of U12)

**The owner moved these first on 2026-11-14**, before I12 and I13 of milestone 8 (U-Q4 had them
after; I12 waits for the owner's level-C samples, I13 documents milestone 8 and can come last).
The interface language is English (GR14), so the section names of ADR 0017 §2.3.1 become English
terms in `docs/glossary.md` (proposed in T123). The tasks are written one step at a time, each when
the one before it is accepted.

| Step | Task | Content | Can start |
|---|---|---|---|
| U10a | T123 | Every simple text, date and list field of sections 1–4 editable (the fields of §2.3.1 that are not structures), the contact address fields and Copyrighted; the form in collapsible sections whose state is remembered; the deprecated IIM fields read-only in their section | done (T123, 2026-11-14) |
| U10b | – | Settings: which sections and fields are shown, and in which order | after T123 |
| U10c | T124 | Rating and Label in the window (section 6), written through the rating path (U10-Q5) | done (T124, 2026-11-14) |
| R1 | reviewer | Read the PLUS controlled vocabulary (ADR 0007 §16.4, Q9) and save it in `D:\PhotoWorkflowData\references\` as P1 was. **Done 2026-11-14:** `LDFXMP-1_2_0-ControlledVocabulary.xml` (7 764 bytes, SHA-256 `26961ebe…08c92fa2`) with Model and Property Release Status and Minor Model Age Disclosure; Data Mining (PLUS 2.0.1, not in that file) taken from the PLUS XMP specification page into the folder's `README.md` | done |
| U10d | T128 | Licence section: Model and Property Release Status, the release ids and Digital Source Type as fixed choice lists (with I11) | done (T128, 2026-11-14) |
| U11a | T136, T139 | Lists of structures in the form, one row per entry with add and remove, within ADR 0007 §8's cardinalities: Location Created, Location Shown (with "copy from Location Created"), Genre, Image Creator, Copyright Owner, Licensor, Image Supplier | done (T136, 2026-11-14: Location Created and Location Shown; T139, 2026-11-15: Genre, Image Creator, Copyright Owner, Licensor and Image Supplier) |
| U11b | T143 | The rest of the structures: Person Shown with Details, Product Shown, Artwork or Object, Contributor, CV-Term About Image, Image Registry Entry (U11c-Q1–U11c-Q8); Image Region later | done (T143, 2026-11-15); hand-checked by the owner on 2026-11-15 |
| U12a | T141 | Recent values and the user's own values per field (▼ beside the field) | done (T141, 2026-11-15) |
| U12b | T125 | Copy and paste all fields between images | done (T125, 2026-11-14) |
| U12c | T126 | "Apply to all marked": the edited fields written to every marked image as a batch (ADR 0007 §9.5), with one confirmation; the copyright and conflict rules of §2.3.1 | done (T126, 2026-11-14) |

## Stage B, other steps

| Step | Task | Content | Can start |
|---|---|---|---|
| U4 | T135 | Library: folder tree, favourites and recent folders (U4-Q1–U4-Q7) | done (T135, 2026-11-14) |
| U14 | T138 | Sort order: File Name, Capture Time, Modified, File Type, Rating, ascending or descending (U14-Q1–U14-Q7) | done (T138, 2026-11-15) |
| U7a | T142, T144, T145 | RAW histogram and exposure statistics in the preview and the inspector (U7-Q1–U7-Q8); the inspector's section shows statistics only when the preview showed the same image, so T144 gives it its own | done (T142, T144, T145, 2026-11-15); hand-checked by the owner on 2026-11-15 (the inspector's section with T144, T145; the preview's section, T142) |
| U7b | T146, T147, T148, T149 | Clipping overlay on the image (§2.6), on T142's clipping level: the map from the RAW data (T146), the view core (T147), the keys, menu items and section (T148), the marks drawn on the image (T149); the owner's hand check after T149 | done (T146–T149, 2026-11-15/16); hand-checked by the owner on 2026-11-16 |
| U5 | T140 | Tasks area: import progress per file, stop button, pending writes, error list (U5-Q1–U5-Q7) | done (T140, 2026-11-15) |
| U6 | T150, T151, T152, T153, T154 | Compare two images: side by side, above/below, locked zoom: the view core takes a view from outside (T150), the compare model, settings and texts (T151), the second view core and the image area as one function (T152), two images in the preview with their keys (T153), the locked zoom and position (T154); the owner's hand check after T154 | U6-Q1–U6-Q12 approved 2026-11-16; T150 done; T151 open |
| U13 | – | Templates from the metadata window (with the modes of ADR 0007 §10) and variables in text fields; the import's template in a window of its own, saved as an XMP template file (owner, 2026-11-15) | needs the namespace of the modes decided first (as D1 of ADR 0007) |
| U9 | – | Focus marking; move on reject or select as a setting, with undo (§2.6); companion file names checked on real files first | not written |
| U8 | – | Key map file and its page in Settings | not written |
| – | – | Images shown while importing: the destination tab opens when the first file is copied and verified and fills as the import goes on (ADR 0017 §7.2, changes ADR 0010's rule) | not placed (owner, 2026-11-15) |

The rows above follow ADR 0017 §7.1's proposed order for what is left (U6, U7b, U13, U9, U8, with
U10b); the owner may reorder them. A step is "done" only after the owner's hand check (ADR 0017
§7); until then its row says "awaiting the owner's hand check".
