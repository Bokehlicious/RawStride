# T141. ADR 0017 U12a: recent values and the user's own values per field in the metadata window — Done

- **Can start now.** The owner approved the proposals of U12a-Q1–U12a-Q7 (at the end of this task)
  on 2026-11-15; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T140** (the Tasks area). Both change `crates/pw-loupe/src/main.rs`, in
  different functions: this task only the metadata window (`draw_window_field` and what it calls,
  and the window's handling of a write that succeeded); T140 the left column, the import dialog's
  Hide and Stop, and `draw`'s taking of `metadata.poll`'s failures. Both add a module line to
  `lib.rs` and rows to `docs/glossary.md`: add lines, never rewrite another's. **Run
  `.\tools\check.ps1` only when the other task's agent is not running it.**
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/field_values.rs` (new: the recent and own
  values per field, choosing one into a field's text, the file they are kept in; with its tests);
  `crates/pw-loupe/src/editor.rs` (the texts of the ▼ list, rule 13, and what a chosen value does
  to a field's text, if it belongs with the form); `crates/pw-loupe/src/lib.rs` (the module line and
  exports); `crates/pw-loupe/src/main.rs` (only the metadata window); `docs/glossary.md`;
  `docs/test-inventory.md`. `settings.rs` does not change: the values have a file of their own.
- **Why:** ADR 0017 §2.3.1 "Speed while captioning": "each field has a list of recent values and
  the user's own values (▼ beside the field)", step U12 (U12b and U12c are done, T125, T126).
  Today the same photographer, city or event is typed again for every shoot.
- **What:**
  1. **Which fields** (U12a-Q1): every editable one-line text field and list field of the form (the
     contact fields included); not the multi-line fields, Date Created, the check box, the choice
     lists, nor the sub-fields of the structure lists (T136, T139). A read-only field (a conflict not
     decided, a value the form cannot show) has no ▼.
  2. **Recent values** (U12a-Q2): what a write that succeeded put in a field, from OK, "Save and
     Next" and "Apply to All Marked": the last 10 per field, newest first, no value twice (the same
     text moves first). A list field (Keywords, Person Shown …) keeps each entry on its own, so a
     keyword is offered alone; only the entries the write adds count (reviewer: an image's many
     old keywords would push the new one out of the 10). An emptied field adds nothing.
  3. **Own values** (U12a-Q3): per field a list the user keeps: "Add to My Values" in the ▼ list
     adds the field's present text (each entry on its own for a list field); "×" beside an own value
     removes it. No limit; sorted alphabetically, ignoring case.
  4. **Choosing** (U12a-Q4): a value chosen replaces a text field's text; for a list field it is
     added as a new entry at the end when it is not already there. The field shows "Changed" as
     after typing; nothing is written until the window's write.
  5. **The ▼** (U12a-Q5): a small "▼" button to the right of each such field opens a list with
     "Recent" and "My Values" (each only with values; "No values yet" when both are empty), then
     "Add to My Values". Alt+↓ in the field opens it too (the Windows custom for a drop-down); ↑/↓
     and Enter choose, Escape closes it without leaving the window (the window's own Escape is not
     triggered).
  6. **Kept where** (U12a-Q6): in `field-values.txt` beside `settings.txt` (in
     `%LOCALAPPDATA%\RawStride\`), one line per value, `recent.<field> = <text>` and
     `own.<field> = <text>`, `<field>` the field's XMP id (the sub-field's for the contact fields),
     in list order; written as `settings.txt` is (to a temporary file, then renamed), when a value is
     added or removed; an unreadable line is warned about and skipped, as in `settings.txt`. Only on
     this computer; nothing is sent anywhere.
  7. **Names** (U12a-Q7): Recent, My Values, Add to My Values, No values yet; the tooltip of "▼"
     "Recent and saved values"; English (GR14), in the glossary.
- **Tests to add** (`field_values.rs`, `editor.rs`): a write adds to recent, newest first, at most
  10, no duplicates, a list field's entries one by one, an emptied field nothing; own values added,
  removed, sorted, no duplicates; choosing replaces a text and appends to a list once; the fields
  with a ▼ (and the ones without); the file saved and loaded in order, a bad line warned and
  skipped, a missing file empty; a value with `=` or leading spaces kept as it is; the texts through
  their functions.
- **Not in this task:** values for the multi-line fields and the structure lists' sub-fields;
  values from other files or a shared list; templates (U13); variables; editing the list of own
  values elsewhere than the ▼ list; a list of approved keywords (§7.2, the keyword panel).
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T140's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U12a-Q1 | Which fields get a ▼? | Every editable one-line text and list field (contact fields included); not the multi-line fields, the date, the check box, the choice lists or the structure lists' sub-fields |
| U12a-Q2 | What is a recent value? | What a successful write (OK, Save and Next, Apply to All Marked) put in the field; the last 10, newest first, no duplicates; a list field's entries one by one |
| U12a-Q3 | How are own values kept? | "Add to My Values" in the ▼ list adds the field's text; "×" removes one; no limit; alphabetical |
| U12a-Q4 | What does choosing a value do? | Replaces a text field's text; adds an entry at the end of a list field when it is not already there; written with the window's write as typing is |
| U12a-Q5 | How is the list opened? | A "▼" button right of the field, or Alt+↓ in the field; ↑/↓ and Enter choose, Escape closes only the list |
| U12a-Q6 | Where are the values kept? | `field-values.txt` beside `settings.txt`, one line per value, written safely when changed; only on this computer |
| U12a-Q7 | Names | Recent, My Values, Add to My Values, No values yet, tooltip "Recent and saved values"; English (GR14), in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added; waits for U12a-Q1–U12a-Q7 |
| 2026-11-15 | Open | The owner approved U12a-Q1–U12a-Q7 as proposed |
| 2026-11-15 | Ready for review | Done by the agent: `field_values.rs` (new), `editor.rs`, `lib.rs`, `main.rs` (metadata window; also its Escape routing in the key loop and the values loaded in `run`), glossary, test inventory; `check.ps1` all OK |
| 2026-11-15 | Done | Reviewed: a list field records only the entries the write adds; after the owner's hand check the field row's height fixed and the ▼ painted (no glyph in the fonts), ✕ changed to ×; approved |
