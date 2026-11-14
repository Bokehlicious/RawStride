# T136. ADR 0017 U11a, first part: Location Created and Location Shown in the metadata window — Open

- **Can start now.** The owner approved the proposals of U11a-Q1–U11a-Q6 (at the end of this task)
  on 2026-11-14; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T135** (the library). Both change `crates/pw-loupe/src/main.rs`, in different
  functions: this task only the metadata window (`draw_window_form`, `window_editor_properties`
  and what they call); T135 only the left column, `open` and the start. Neither touches the
  other's functions. Both may add terms to `docs/glossary.md`: add rows, never rewrite another
  row. **Run `.\tools\check.ps1` only when the other task's agent is not running it** (both build
  in the same `target` folder).
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/editor.rs` (the form's structure lists,
  their edits, copy, paste and "Apply to All Marked", their texts); `crates/pw-loupe/src/main.rs`
  (only the metadata window); `docs/glossary.md`; `docs/test-inventory.md`. Nothing in
  `pw-metadata` changes: it already reads and writes lists of structures (`Value::StructList`,
  `write_plan.rs`). If a write of a location turns out wrong, stop and report it (rule 5).
- **Why:** ADR 0017 §2.3.1 section 3 (Time and Place) and step U11: Location Created (where the
  image was taken) and Location Shown (what the image shows) are shown today only read-only under
  Other Metadata. They are the structured places a photo desk and an archive search on. U11a is
  split (U11a-Q1): places first, Genre and the four PLUS structures in a later task.
- **What:**
  1. **Two lists in Time and Place** (U11a-Q2), after the older fields Sublocation … Country Code:
     **Location Created** and **Location Shown**. Each entry is one framed group with seven
     one-line text fields in this order: Sublocation, City, Province/State, Country Name, Country
     ISO Code, World Region, Location Identifier (`Iptc4xmpExt:Sublocation`, `City`,
     `ProvinceState`, `CountryName`, `CountryCode`, `WorldRegion`, `LocationId`; names from
     `property_label` or IPTC's structure field names in the property table, U11a-Q6), and a
     "Remove" button. Under each list an "Add Location" button. Location Identifier is a list of
     URIs in IPTC; it is one line separated by commas, as Keywords (U10-Q2).
  2. **Entries** (U11a-Q3): a list with no entry shows only its Add button. An entry whose fields
     are all empty is not written (dropped when the edits are made), so Add followed by OK writes
     nothing. Removing every entry clears the property (ADR 0007 §8, as clearing a field today).
     The order of the entries is kept; there is no reordering in this task.
  3. **"Copy from Location Created"** (U11a-Q4): a button under Location Shown, enabled when
     Location Created has a non-empty entry, adds one new Location Shown entry with the fields of
     the first Location Created entry. It never changes an existing entry.
  4. **Unknown fields are kept** (ADR 0007 §8): sub-fields of a read entry that the form does not
     show (for example `LocationName` or a field of another namespace) are kept unchanged when the
     entry is written, also when a shown field of that entry is edited. An entry with such fields is
     never dropped by point 2.
  5. **Several images** (U11a-Q5): the two lists join "Copy All Fields", "Paste All Fields" and
     "Apply to All Marked" with the rules of U12b and U12c, each list as one field: a pasted or
     applied list replaces the other image's whole list; an empty list is not pasted (U12b-Q2);
     an emptied list is named in the confirmation (U12c-Q4).
  6. **The edits** come from `EditorForm::edits` as one `Edit` per changed property with a
     `Value::StructList`, written through the existing path; the write notices, conflicts and
     "Save changes?" questions are unchanged. A property with an open conflict is shown read-only
     as today until the conflict is decided.
- **Tests to add** (`editor.rs`): a read list shown as its entries with the seven fields; an edited
  field gives one `StructList` edit with the other entries unchanged; Add then OK with nothing
  typed gives no edit; removing every entry clears; unknown sub-fields kept through an edit;
  "Copy from Location Created" adds an entry and changes no other; copy, paste and Apply to All
  Marked of the lists with the U12b/U12c rules; the texts through their functions. One
  `pw-metadata` level write is not needed (the write plan's tests cover `StructList`).
- **Not in this task:** Genre, Image Creator, Copyright Owner, Licensor and Image Supplier (the
  second part of U11a), Person Shown with details, reordering entries, recent values (U12a),
  synchronising the older location fields with Location Created (ADR 0017 §2.3.1: not
  synchronised), GPS.
- **Rules:** rules 1–13 of `docs/tasks/README.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T135's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U11a-Q1 | U11a in one task or two? | Two: Location Created and Location Shown now; Genre, Image Creator, Copyright Owner, Licensor and Image Supplier in the next task |
| U11a-Q2 | Where and how are the locations shown? | In Time and Place after the older fields; one framed group per entry with the seven sub-fields and "Remove"; "Add Location" under each list |
| U11a-Q3 | Empty and removed entries | An all-empty entry is not written; removing every entry clears the property; no reordering |
| U11a-Q4 | "Copy from Location Created" | Adds a new Location Shown entry from the first Location Created entry; never changes an existing one |
| U11a-Q5 | Copy, paste and Apply to All Marked | Included, each list as one field that replaces the whole list; empty lists not pasted |
| U11a-Q6 | Names | IPTC's English names (GR14): Location Created, Location Shown, Sublocation, City, Province/State, Country Name, Country ISO Code, World Region, Location Identifier; "Add Location", "Remove", "Copy from Location Created" in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Added; waits for U11a-Q1–U11a-Q6 |
| 2026-11-14 | Open | The owner approved U11a-Q1–U11a-Q6 as proposed |
