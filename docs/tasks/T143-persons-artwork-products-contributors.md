# T143. ADR 0017 U11, last part: persons with details, artwork, products, contributors and the other structures — Open

- **Can start now.** The owner approved the proposals of U11c-Q1–U11c-Q8 (at the end of this task)
  on 2026-11-15; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T142** (the histogram). Both change `crates/pw-loupe/src/main.rs`, in different
  functions: this task only the metadata window (`draw_window_form`, `draw_structure_lists` and what
  they call, `window_editor_properties`); T142 the preview's right column and the inspector. Both
  add rows to `docs/glossary.md`: add rows, never rewrite another's. **Run `.\tools\check.ps1` only
  when the other task's agent is not running it.**
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/editor.rs`; `crates/pw-loupe/src/main.rs`
  (only the metadata window); `docs/glossary.md`; `docs/test-inventory.md`. `pw-metadata` does not
  change: it reads and writes lists of structures already (T136, T139). If a write turns out wrong,
  stop and report it (rule 5).
- **Why:** ADR 0017 §2.3.1 and step U11 ("persons, locations, creators, licensors, releases").
  T136 and T139 did the locations, Genre and the PLUS lists; these structures are still shown
  only read-only under Other Metadata.
- **What:**
  1. **Which lists** (U11c-Q1), each one more case of T139's structure list (`ListSpec`), with its
     entries, Add, Remove, the empty-entry rule, kept unknown sub-fields, conflicts, read-only, copy,
     paste and Apply to All Marked:
     - **Person Shown in the Image with Details** (`Iptc4xmpExt:PersonInImageWDetails`): Name,
       Identifier, Description;
     - **Product Shown in the Image** (`Iptc4xmpExt:ProductInImage`): Name, GTIN, Identifier,
       Description;
     - **Artwork or Object in the Image** (`Iptc4xmpExt:ArtworkOrObject`): its 17 sub-fields
       (point 3);
     - **Contributor** (`Iptc4xmpExt:Contributor`): Name, Role, Identifier;
     - **CV-Term About Image** (`Iptc4xmpExt:AboutCvTerm`): CV-Term Name, CV-Term ID, CV-Term CV ID
       (as Genre);
     - **Image Registry Entry** (`Iptc4xmpExt:RegistryId`): Organisation Id,
       Item Id, Role (IPTC's names).
     Not **Image Region** (it is drawn on the image and needs a step of its own) nor the
     machine-readable rights expressions (Embedded and Linked Encoded Rights Expression).
  2. **Where** (U11c-Q2): in Content, after Person Shown: Person Shown with Details, then Product
     Shown, then Artwork or Object; in Creator and Rights, at the end: Contributor; in Assignment,
     after Genre: CV-Term About Image, and at the end: Image Registry Entry.
  3. **Artwork or Object** (U11c-Q3): open in each entry Title, Creator, Date Created, Source,
     Source Inventory Number and Copyright Notice; the other eleven (Circa Date Created, Style
     Period, Content Description, Contribution Description, Physical Description, Creator ID,
     Current Copyright Owner Name and ID, Current Licensor Name and ID, Source Inventory URL) under a
     collapsed "More" line, as the Licensor's "Address" (T139).
  4. **Sub-field kinds** (U11c-Q4): a Lang Alt edits `x-default` and keeps the other languages, a
     multi-value sub-field (Artwork's Creator and Creator ID, Contributor's Role, an Identifier) is
     one line split at commas, as Location Identifier (T136); Artwork's **Date Created** is checked
     as the form's Date Created is (an IPTC date; the field's error shown and nothing written while
     it is wrong); Circa Date Created is free text.
  5. **Person Characteristics** (U11c-Q5): the nested list of CV terms in a Person with Details
     entry is not edited; it is kept unchanged (ADR 0007 §8), as any unknown sub-field.
  6. **"Copy from Person Shown"** (U11c-Q6): a button under Person Shown with Details, as "Copy from
     Creator" (T139): one entry per Person Shown name that is not already a Name there; never
     changes an existing entry; nothing synchronised automatically.
  7. **Several images** (U11c-Q7): as the other lists (U11a-Q5, U11b-Q5); none of these is a
     copyright-management field of the image (ADR 0007 §8), so an emptied list is emptied on every
     image and named in the confirmation.
  8. **Other Metadata and names** (U11c-Q8): the six properties are left out of Other Metadata
     (`window_editor_properties` and its test, as T136 and T139); IPTC's English names from
     `property_label` (GR14); "More", "Copy from Person Shown" and "Add {list}" in the glossary.
- **Tests to add** (`editor.rs`): each list read, edited and written as one `StructList` edit, its
  place among the fields and in the confirmation; Artwork's open and collapsed sub-fields; a Lang
  Alt sub-field edits only `x-default`; a multi-value sub-field split at commas; a wrong Artwork
  Date Created is an error and gives no edit; Person Characteristics kept when the name is edited;
  "Copy from Person Shown" (two names, one already present); T136's and T139's tests unchanged;
  the texts through their functions.
- **Not in this task:** Image Region; the rights expressions; the IPTC vocabularies of the
  CV terms as choice lists; editing Person Characteristics; reordering entries.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T142's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U11c-Q1 | Which structures? | Person Shown with Details, Product Shown, Artwork or Object, Contributor, CV-Term About Image, Image Registry Entry; not Image Region (needs drawing on the image) nor the rights expressions |
| U11c-Q2 | Where in the form? | Content after Person Shown: Person with Details, Product, Artwork; Creator and Rights at the end: Contributor; Assignment after Genre: CV-Term About Image, at the end: Image Registry Entry |
| U11c-Q3 | Artwork has 17 sub-fields; how are they shown? | Title, Creator, Date Created, Source, Source Inventory Number and Copyright Notice open; the other eleven under a collapsed "More" line |
| U11c-Q4 | How are the sub-fields edited? | Lang Alt edits `x-default`; multi-value sub-fields one line split at commas; Artwork's Date Created checked as an IPTC date; Circa free text |
| U11c-Q5 | Person Characteristics (a list inside the list)? | Not edited; kept unchanged |
| U11c-Q6 | Person Shown ↔ Person Shown with Details | A button "Copy from Person Shown"; nothing synchronised automatically |
| U11c-Q7 | Copy, paste and Apply to All Marked | As the other lists; none is a copyright-management field, so an emptied list is emptied on every image |
| U11c-Q8 | Names | IPTC's English names (GR14); "More", "Copy from Person Shown" in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added; waits for U11c-Q1–U11c-Q8 |
| 2026-11-15 | Open | The owner approved U11c-Q1–U11c-Q8 as proposed |
