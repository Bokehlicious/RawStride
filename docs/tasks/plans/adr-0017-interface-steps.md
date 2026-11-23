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
| U10b | T277, T278 | Settings: which sections and fields are shown, and in which order (U10b-Q1–U10b-Q9, ADR 0017 §6.3, 2026-11-22) | written 2026-11-22; placed by the owner on 2026-11-21 as order 2b in [`adr-0018-0025-steps.md`](adr-0018-0025-steps.md), after ADR 0026 F4 |
| U10c | T124 | Rating and Label in the window (section 6), written through the rating path (U10-Q5) | done (T124, 2026-11-14) |
| R1 | reviewer | Read the PLUS controlled vocabulary (ADR 0007 §16.4, Q9) and save it in `D:\PhotoWorkflowData\references\` as P1 was. **Done 2026-11-14:** `LDFXMP-1_2_0-ControlledVocabulary.xml` (7 764 bytes, SHA-256 `26961ebe…08c92fa2`) with Model and Property Release Status and Minor Model Age Disclosure; Data Mining (PLUS 2.0.1, not in that file) taken from the PLUS XMP specification page into the folder's `README.md` | done |
| U10d | T128 | Licence section: Model and Property Release Status, the release ids and Digital Source Type as fixed choice lists (with I11) | done (T128, 2026-11-14) |
| U11a | T136, T139 | Lists of structures in the form, one row per entry with add and remove, within ADR 0007 §8's cardinalities: Location Created, Location Shown (with "copy from Location Created"), Genre, Image Creator, Copyright Owner, Licensor, Image Supplier | done (T136, 2026-11-14: Location Created and Location Shown; T139, 2026-11-15: Genre, Image Creator, Copyright Owner, Licensor and Image Supplier) |
| U11b | T143 | The rest of the structures: Person Shown with Details, Product Shown, Artwork or Object, Contributor, CV-Term About Image, Image Registry Entry (U11c-Q1–U11c-Q8); Image Region later | done (T143, 2026-11-15); hand-checked by the owner on 2026-11-15 |
| U12a | T141 | Recent values and the user's own values per field (▼ beside the field) | done (T141, 2026-11-15) |
| U12b | T125 | Copy and paste all fields between images | done (T125, 2026-11-14) |
| U12c | T126 | "Apply to all marked": the edited fields written to every marked image as a batch (ADR 0007 §9.5), with one confirmation; the copyright and conflict rules of §2.3.1 | done (T126, 2026-11-14) |
| U12d | T179, T180 | "Apply to {n} Marked" sends only the difference of a list field: added entries added, removed entries removed, each image's own kept (U12d-Q1–U12d-Q5, owner 2026-11-17 after the job test): the edit in `pw-metadata` (T179), the shell, the confirmation and "OK and Save write this image only" (T180); the owner's hand check after T180 | done (the owner's hand check, 2026-11-17; U12d-Q9) |

## Stage B, other steps

| Step | Task | Content | Can start |
|---|---|---|---|
| U4 | T135 | Library: folder tree, favourites and recent folders (U4-Q1–U4-Q7) | done (T135, 2026-11-14) |
| U14 | T138 | Sort order: File Name, Capture Time, Modified, File Type, Rating, ascending or descending (U14-Q1–U14-Q7) | done (T138, 2026-11-15) |
| U7a | T142, T144, T145 | RAW histogram and exposure statistics in the preview and the inspector (U7-Q1–U7-Q8); the inspector's section shows statistics only when the preview showed the same image, so T144 gives it its own | done (T142, T144, T145, 2026-11-15); hand-checked by the owner on 2026-11-15 (the inspector's section with T144, T145; the preview's section, T142) |
| U7b | T146, T147, T148, T149 | Clipping overlay on the image (§2.6), on T142's clipping level: the map from the RAW data (T146), the view core (T147), the keys, menu items and section (T148), the marks drawn on the image (T149); the owner's hand check after T149 | done (T146–T149, 2026-11-15/16); hand-checked by the owner on 2026-11-16 |
| U5 | T140 | Tasks area: import progress per file, stop button, pending writes, error list (U5-Q1–U5-Q7) | done (T140, 2026-11-15) |
| U6 | T150, T151, T152, T153, T154 | Compare two images: side by side, above/below, locked zoom: the view core takes a view from outside (T150), the compare model, settings and texts (T151), the second view core and the image area as one function (T152), two images in the preview with their keys (T153), the locked zoom and position (T154); the owner's hand check after T154 | done (T150–T154, 2026-11-16); U6-Q1–U6-Q12 approved 2026-11-16; hand-checked by the owner on 2026-11-16 |
| U13 | – | Templates from the metadata window and variables in text fields; the import's template in a window of its own, saved as an XMP template file (owner, 2026-11-15); decided by the owner 2026-11-22 (ADR 0017 §6.5): a check box per field, only standard XMP, a checked field replaces; amended 2026-11-23 (U13-Q12, U13-Q13): a + box on lists of plain text values adds instead, the template window in two columns | parts U13a–U13d (U13-Q11); U13a is T284 → T285 (written 2026-11-23; both done 2026-11-23, U13a awaiting the owner's hand check with U13c's window), U13b is T287 → T288 → T289 (written 2026-11-23; T287, T288 and T289 done 2026-11-23; hand-checked and approved by the owner on 2026-11-23, follow-up T290 done 2026-11-23), U13c–U13d not yet written; placed by the owner on 2026-11-21 as order 2b in [`adr-0018-0025-steps.md`](adr-0018-0025-steps.md), after U8 |
| U9a | T167, T168, T176 | Focus marking (U9-Q1–U9-Q4): the map in the view core from the half-size render (T167), the key E, the menu item, the setting and the marks drawn (T168); the owner's hand check after T168 | done (T167, T168, T176, 2026-11-17; T176: the map takes about 265 ms, stepping and Z not slower with the marking on); hand-checked and approved by the owner on 2026-11-17, `FOCUS_THRESHOLD` kept |
| R2 | reviewer | Read the companion files of other programs (DxO PhotoLab `.dop` first, RawTherapee `.pp3`, ON1 `.on1`) on the owner's real files: their names only (clean-room level C), written down before U9b | done 2026-11-17: DxO PhotoLab names its file `<base>.<ext>.dop` (ADR 0017 §2.6); no `.pp3` or `.on1` files available |
| U9b | T169–T174, T177, T178 | Move on reject or pick as a setting, with undo (U9-Q5–U9-Q10, §2.6): the files of an image moved all or nothing (T169, `pw-metadata`), the move as a request to the metadata worker (T170), the two settings (T171), the rules, the undo list, the tab's list and the texts (T172), the requests and answers in the shell's metadata part (T173), the keys, the menu items and the wiring (T174), the release before a move without the write stamp (T177, before T174), a move waiting for its write's answer and no image moved twice (T178, after T174); the owner's hand check after T178 | done (T169–T174, T177, T178, 2026-11-17); hand-checked and approved by the owner on 2026-11-17; T183 (a tab on `_rejected` or `_picked` follows a move and an undo, Ctrl+Z while a move runs; from the review of T174) done 2026-11-17, hand-checked and approved by the owner on 2026-11-17 |
| U8 | T279, T280, T281 | Key map file and its page in Settings (U8-Q1–U8-Q11, approved 2026-11-22): the map in plain Rust with `keys.txt` (T279), the shell asks it and the menus and texts show its keys (T280), the page Keyboard Shortcuts and the companion suffixes' field on General (T281); the owner's hand check after T281; T283 (a larger Settings window, U8-Q14); T286 (the page as a table of three columns, U8-Q15, 2026-11-23) | placed by the owner on 2026-11-21 as order 2b in [`adr-0018-0025-steps.md`](adr-0018-0025-steps.md), after U10b; written 2026-11-22 |
| U15 | T244, T245 | Buttons on a thumbnail (addition, accepted by the owner 2026-11-21; ADR 0017 §2.8, U15-Q1–U15-Q9): the metadata window steps between the marked (T244, U15a), the four buttons in the contact sheet (T245, U15b); the owner's hand check after T245 | placed by the owner between ADR 0026 F1 and F2, order 2a in [`adr-0018-0025-steps.md`](adr-0018-0025-steps.md) (2026-11-21); done (T244, T245, 2026-11-21); hand-checked by the owner on 2026-11-21 |
| – | – | Images shown while importing: the destination tab opens when the first file is copied and verified and fills as the import goes on (ADR 0017 §7.2, changes ADR 0010's rule) | moved: taken up by ADR 0026 §3.4, step F4, order 2a in [`adr-0018-0025-steps.md`](adr-0018-0025-steps.md) (accepted 2026-11-18) |

The rows above follow ADR 0017 §7.1's proposed order for what is left (U6, U7b, U13, U9, U8, with
U10b); the owner may reorder them. A step is "done" only after the owner's hand check (ADR 0017
§7); until then its row says "awaiting the owner's hand check".
