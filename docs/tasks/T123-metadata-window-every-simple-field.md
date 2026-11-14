# T123. ADR 0017 U10, first part: every simple field of the metadata window, in sections — Open

- **Can start now.** The owner approved the proposals of U10-Q1–U10-Q6 (at the end of this task)
  on 2026-11-14; they are written into the points below and into ADR 0017 §6.1. Not at the same
  time as another task that changes `main.rs`.
- **Crates:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/editor.rs`;
  `crates/pw-loupe/src/main.rs` (`draw_window_form`, `window_editor_properties`);
  `crates/pw-loupe/src/settings.rs` (the sections' state, point 5); `docs/glossary.md` (the new
  terms, point 6); `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.3.1 and U10: the metadata window edits only the 22 fields of D12 (ADR 0007
  §17); every other property is read-only under "Other Metadata". The owner wants a full caption
  window. This first part takes every field that is a single value or a plain list (no
  structures) and puts the form in sections. Structures (U11), the PLUS choice lists (U10d), the
  user's choice of fields (U10b) and the rest of U12 come in later tasks
  (`plans/adr-0017-interface-steps.md`).
- **What:**
  1. **The fields** (`EditorField`, `EditorField::ALL`), in this order and these sections (U10-Q1, U10-Q4). The 22
     present fields keep their meaning; *new* marks the others:
     - **Content:** Title; Headline; Description; Alt Text (Accessibility); *Extended Description
       (Accessibility)* (`Iptc4xmpCore:ExtDescrAccessibility`, Lang Alt, several lines);
       *Description Writer* (`photoshop:CaptionWriter`); Keywords; *Person Shown in the Image*
       (`Iptc4xmpExt:PersonInImage`, list); *Name of Organisation Featured in the Image*
       (`Iptc4xmpExt:OrganisationInImageName`, list); *Code of Organisation Featured in the Image*
       (`Iptc4xmpExt:OrganisationInImageCode`, list); *Event Name* (`Iptc4xmpExt:Event`, Lang Alt).
     - **Creator and Rights:** Creator; Creator's jobtitle; Creator's Contact Info with *Address*,
       *City*, *State/Province*, *Postal Code*, *Country* (`Iptc4xmpCore:CiAdrExtadr`,
       `CiAdrCity`, `CiAdrRegion`, `CiAdrPcode`, `CiAdrCtry`), Email, Phone, Web URL; Copyright
       Notice; Credit Line; Source (Supply Chain); Rights Usage Terms; *Web Statement of Rights*
       (`xmpRights:WebStatement`); *Copyrighted* (`xmpRights:Marked`, a check box, point 3).
     - **Time and Place:** Date Created; Sublocation; City; Province or State; Country; Country
       Code (the legacy fields, as today; not synchronised with Location Created, which is a
       structure and comes in U11, ADR 0017 §2.3.1).
     - **Assignment:** Instructions; Job Id; *Subject Code* (`Iptc4xmpCore:SubjectCode`, list);
       *Scene Code* (`Iptc4xmpCore:Scene`, list); Urgency, Category and Supplemental Categories
       **read-only** (point 4).

     A list field is one line with its entries separated by commas, as Keywords (T108); an entry cannot hold a
     comma (U10-Q2). A Lang Alt
     field follows the present rules (`x-default` only, the note when it is missing, Q7). The value
     kind of each field still comes from the property table.
  2. **Edits, conflicts and warnings** work for the new fields exactly as for the present ones
     (`EditorForm::from_read`, `choose`, `edits`, `warnings`): a changed field is a `Set`, an
     emptied one a `Clear`; a contact address field is a `Set` of its structure field, as Email
     is. No change in `pw-metadata`.
  3. **Copyrighted** (U10-Q3): a check box. Checked gives `Set` `True`, unchecked `Set` `False`;
     a file without the property shows it unchecked, and an untouched box gives no edit. A value
     other than `True` or `False` (any case) is shown read-only with its text and gives no edit.
  4. **Urgency, Category, Supplemental Categories** (ADR 0017 §2.3.1, ADR 0007 §5.5): shown in
     Assignment as read-only text, only when the file has them; never an edit. Rating and Label
     are not in the window in this task (U10-Q5).
  5. **Sections** (U10-Q6): each a collapsible heading, all open by default; the state of each is
     remembered in `settings.txt` (D10), one key per section, as the inspector's sections are
     (T110). The open conflicts, the warnings and the read-only parts stay below the sections as
     today.
  6. **Texts** (rule 13): the section headings come from `editor.rs` (`EditorText`); every field
     name from `property_label`. The new names and headings are added to `docs/glossary.md`
     (sections 1 and 6) and named in the report. The contact address fields are drawn under their
     structure's heading as Email is (T114).
  7. **Other Metadata** no longer lists a property that is now a field, nor the deprecated fields
     of point 4. `Iptc4xmpCore:CreatorContactInfo` leaves the list, since all its fields are now
     fields.
- **Not in this task:** structures and lists of structures (Location Created, Location Shown,
  Genre, the PLUS structures: U11); the PLUS release fields and Digital Source Type (U10d); Rating
  and Label (U10c); choosing and ordering the fields (U10b); recent values, copy and paste, several
  images (U12); any change in `pw-metadata` or to the write path.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add / change:** `EditorField::ALL` order and sections; the property, structure field
  and kind of each new field; `from_read` → `edits` for each new kind (Lang Alt, list, contact
  address field): unchanged → no edit, changed → `Set`, emptied → `Clear`; Copyrighted (absent,
  `True`, `False`, other text read-only; untouched → no edit); the deprecated fields never give an
  edit; `window_editor_properties` and Other Metadata without the new fields; the sections' keys
  read and written in `settings.rs` (missing key → open); `editor_text` of the headings; the label
  test covers every field.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` and `docs/glossary.md`
  are updated, and `git diff --stat` shows only the files above. The owner checks by hand on a
  JPEG, an ARW (sidecar) and a DNG: each new field written, then read back with `M` and in Photo
  Mechanic; Copyrighted set and cleared; sections collapsed, the program restarted, still
  collapsed; an image with Urgency or Category shows them read-only.

## Questions (answered by the owner, 2026-11-14: every proposal approved)

- **U10-Q1 Section names.** The interface is English (GR14). Proposal: **Content**, **Creator and
  Rights**, **Time and Place**, **Assignment**, later **Licence (PLUS)** and **Rating and Label**,
  in place of the Swedish names of ADR 0017 §2.3.1.
- **U10-Q2 Lists with commas.** Person Shown, the two organisation fields, Subject Code and Scene
  Code are one line separated by commas, as Keywords (T108). A name that holds a comma ("Smith,
  John") cannot be written that way. Proposal: accept it for now; a later step can give lists one
  row per entry if it turns out to matter.
- **U10-Q3 Copyrighted.** Proposal: a check box writing `True` or `False` (point 3). The
  alternative is three states (unknown, yes, no), where unknown clears the property.
- **U10-Q4 Genre.** ADR 0017 §2.3.1 lists "Genre" under Assignment. IPTC has the structure `Genre`
  (a controlled vocabulary term, U11) and the plain text `Intellectual Genre (legacy)`. Proposal:
  Genre is the structure and comes in U11; Intellectual Genre stays under Other Metadata.
- **U10-Q5 Rating and Label in the window.** ADR 0017 §2.3.1 section 6. They are written today by
  their own path (keys, the rating store, the badges). Proposal: a later small task (U10c) shows
  them in the window with the same effect as the keys, written through the rating path, not the
  form's edits; T123 leaves them out.
- **U10-Q6 Sections remembered.** Proposal: open or collapsed per section in `settings.txt`, all
  open at first (point 5).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Written by the reviewer on the owner's request (U10–U12 before I12 and I13); waits for U10-Q1–U10-Q6 |
| 2026-11-14 | Open | Owner: U10-Q1–U10-Q6 approved as proposed; written into the points and ADR 0017 §6.1 |
