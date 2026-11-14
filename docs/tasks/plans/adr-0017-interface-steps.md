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
| U10d | – | Licence section: Model and Property Release Status, the release ids and Digital Source Type as fixed choice lists (with I11) | now (R1 done) |
| U11a | – | Lists of structures in the form, one row per entry with add and remove, within ADR 0007 §8's cardinalities: Location Created, Location Shown (with "copy from Location Created"), Genre, Image Creator, Copyright Owner, Licensor, Image Supplier | after T123 |
| U12a | – | Recent values and the user's own values per field (▼ beside the field) | after T123 |
| U12b | T125 | Copy and paste all fields between images | done (T125, 2026-11-14) |
| U12c | – | "Apply to all marked": the edited fields written to every marked image as a batch (ADR 0007 §9.5), with one confirmation; the copyright and conflict rules of §2.3.1 | now (U12b done) |
