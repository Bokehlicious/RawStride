# T139. ADR 0017 U11a, second part: Genre and the PLUS structures in the metadata window — Done

- **Can start now.** The owner approved the proposals of U11b-Q1–U11b-Q6 (at the end of this task)
  on 2026-11-14; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T138** (the sort order). Both change `crates/pw-loupe/src/main.rs`, in
  different functions: this task only the metadata window (`draw_window_form`,
  `draw_location_lists` and what they call, `window_editor_properties`); T138 the command bar, the
  Visa menu, the start and the sort's follow-up. Both may add terms to `docs/glossary.md`: add
  rows, never rewrite another row. **Run `.\tools\check.ps1` only when the other task's agent is
  not running it.**
- **Crates:** `pw-loupe`, `pw-metadata` (one vocabulary). **Files:**
  `crates/pw-loupe/src/editor.rs`; `crates/pw-loupe/src/main.rs` (only the metadata window);
  `crates/pw-metadata/src/vocabulary.rs` (the telephone types, point 3); `docs/glossary.md`;
  `docs/test-inventory.md`. Nothing else in `pw-metadata` changes: it reads and writes lists of
  structures already (T136 wrote Location Created and Location Shown through it). If a write turns
  out wrong, stop and report it (rule 5).
- **Why:** ADR 0017 §2.3.1 sections 4 and 5 and step U11; U11a-Q1 split U11a, and T136 did the
  locations. These five properties are shown today only read-only under Other Metadata.
- **What:**
  1. **One kind of structure list** (U11b-Q1): T136's location lists become one case of a general
     structure list in `editor.rs` (the list's XMP id, its shown sub-fields in order, its maximum
     number of entries), so the locations and the five new lists share the entries, Add, Remove,
     the empty-entry rule (U11a-Q3), the kept unknown sub-fields (ADR 0007 §8), the conflict and
     read-only rules, and copy, paste and Apply to All Marked (U11a-Q5). T136's tests must keep
     passing unchanged.
  2. **The lists and their sub-fields** (names from `property_label`, IPTC's English names):
     - in **Assignment**, after Scene Code: **Genre** (`Iptc4xmpExt:Genre`): CV-Term Name
       (`Iptc4xmpExt:CvTermName`, a Lang Alt: the field edits `x-default` and keeps the other
       languages, ADR 0007 §8), CV-Term ID, CV-Term CV ID (U11b-Q2); no limit;
     - in **Licence (PLUS)**, before the release fields: **Image Creator** (Name, ID; up to 3),
       **Copyright Owner** (Name, ID; up to 3), **Licensor** (up to 3; point 3), **Image
       Supplier** (Name, ID; at most 1). "Add" is disabled at the limit (`max_entries`), with the
       tooltip "At most {n} entries".
  3. **Licensor** (U11b-Q3): Name, ID, Email, URL, Telephone 1 with its type, Telephone 2 with its
     type, then the address under a collapsed "Address" line in each entry (Street Address, Address
     Detail, City, State or Province, Postal Code, Country). The two telephone types are choice
     lists from the PLUS vocabulary (`work`, `cell`, `fax`, `home`, `pager`, written as the codes
     ExifTool expects, as the other PLUS choices, T128), with an empty entry; an unknown value is
     shown as "Unknown value: {uri}" and kept (U10d-Q4). The values come from
     `LDFXMP-1_2_0-ControlledVocabulary.xml` (R1) and are added to `vocabulary.rs` with their
     English labels.
  4. **"Copy from Creator"** (U11b-Q4, ADR 0007 §8 "Creator ↔ Image Creator name: synchronisation
     is offered"): a button under Image Creator, enabled when Creator (`dc:creator`) has a name
     and Image Creator has room: adds one Image Creator entry per Creator name that is not already
     an Image Creator Name, up to the limit; never changes an existing entry. Nothing is
     synchronised automatically.
  5. **Other Metadata:** the five properties are left out of it (`window_editor_properties`), as
     T136 did for the locations; the test of `window_editor_properties` is brought up to date for
     them (the reviewer's change of T136 is the pattern).
  6. **Several images** (U11b-Q5): as U11a-Q5: each list one field that replaces the whole list;
     an empty list is not pasted; Image Creator, Copyright Owner and Licensor are
     copyright-management fields (ADR 0007 §8), so an **emptied** one is emptied on the window's
     image only and named as such in the confirmation (U12c-Q3, U12c-Q4).
- **Tests to add** (`editor.rs`, `vocabulary.rs`): each list read, edited and written as one
  `StructList` edit; the limit (Add disabled at 3 and at 1); Genre's name edits only `x-default`;
  a Licensor with an address and a telephone type read and written back unchanged when another
  field is edited; an unknown telephone type kept; "Copy from Creator" (two creators, one already
  present, the limit); the copyright-management rule for an emptied Image Creator in Apply to All
  Marked; T136's location tests unchanged; the telephone types in the vocabulary; the texts through
  their functions.
- **Not in this task:** Person Shown with details, Artwork, Product and the other structures of
  Other Metadata; the IPTC Genre vocabulary as a choice list (U11b-Q2); reordering entries.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T138's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U11b-Q1 | Build the five lists like the locations? | Yes: T136's location lists become one general structure list, so all seven lists behave the same |
| U11b-Q2 | Genre: free text or a choice list? | Free text (name, ID, CV ID) for now; IPTC's Genre vocabulary as a choice list later, when it is saved in the references folder |
| U11b-Q3 | Licensor has 14 sub-fields; how are they shown? | Name, ID, Email, URL and the two telephones (with a type chosen from the PLUS list) open; the six address fields under a collapsed "Address" line in each entry |
| U11b-Q4 | Creator ↔ Image Creator (ADR 0007 §8: offered) | A button "Copy from Creator" under Image Creator; nothing synchronised automatically |
| U11b-Q5 | Copy, paste and Apply to All Marked | As the locations; Image Creator, Copyright Owner and Licensor emptied only on the window's image (copyright-management fields, §8) |
| U11b-Q6 | Names | IPTC's English names from the property table (GR14); "Copy from Creator", "Address", "At most {n} entries" in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Added; waits for U11b-Q1–U11b-Q6 |
| 2026-11-14 | Open | The owner approved U11b-Q1–U11b-Q6 as proposed |
| 2026-11-14 | Ready for review | Done by the task agent; `check.ps1` all OK (1404 passed, 53 ignored, 0 failed, with T138's uncommitted changes in the same tree) |
| 2026-11-15 | Done | Reviewed (two comments rewrapped); the owner checked it by hand and approved it |
