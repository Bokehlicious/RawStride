# T181. The metadata window's status line says when nothing was saved because of a field error (pw-loupe) — Blocked

- **Why:** from the review of T143: with an Artwork or Object's Date Created that is not an IPTC
  date, OK writes nothing and the reason is shown only under that box, which may be under the
  collapsed "More" line, so the user does not see why nothing happened (and a later "Copy All
  Fields" copies the file's earlier date). **Owner, 2026-11-17** (the first backlog sort; ADR 0017
  §6.1, U11c-Q9): when OK, "Save and Next", the "Save" answer of "Save changes?" or "Apply to {n}
  Marked" writes nothing because of a field error, the window's status line says so too, naming
  the fields and structure lists that have an error; the texts under the fields stay as they are.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/editor.rs`,
  `crates/pw-loupe/src/main.rs`, `docs/test-inventory.md`, and this task's own file (status and
  log).
- **What:**
  1. **The lists with an error** (`editor.rs`): `pub fn lists_with_errors(&self) -> Vec<&'static
     str>`: the XMP id of each editable structure list (not read-only, as `has_list_error`
     checks) with an entry that has a sub-field with an error (`ListEntry::has_error`), each once,
     in the order of `self.lists`. `has_list_error` may call it (`!….is_empty()`); its behaviour
     does not change.
  2. **The text** (`editor.rs`): a variant of `EditorText`, after `SaveFailed`:

     ```rust
     /// The window's status line when OK, Save and Next, Save or "Apply to {n} Marked" wrote
     /// nothing because of field errors (U11c-Q9, T181): the fields and the structure lists
     /// (XMP ids of [`EditorForm::ALL_LISTS`]) that have an error.
     NotSaved {
         fields: Vec<EditorField>,
         lists: Vec<&'static str>,
     },
     ```

     and its arm in `editor_text`, with the names from `item_names(fields, lists)` (form order):
     - one field or list in all: `"Nothing was saved. Correct the error in {items}."`;
     - two or more: `"Nothing was saved. Correct the errors in {items}."`;
     - none (cannot happen today; no panic): `"Nothing was saved. Correct the errors in the
       form."`.
  3. **OK, Save and Next, and Save** (`main.rs`, `write_window`, the `Err(errors)` arm, about line
     2467): besides `window.errors = errors` (unchanged), `window.status` becomes
     `EditorStatus::Failed(editor_text(&EditorText::NotSaved { fields, lists }))`, with `fields`
     the fields of `errors` in their order (taken before `errors` is moved into `window.errors`;
     it may be empty when only a list has an error, `edits()` then gives `Err(Vec::new())`) and `lists` the form's `lists_with_errors()` (read
     where `write_window` reads `edits()` from the form, before the `match`). Nothing else in
     `write_window` changes; the window stays as today (`WriteOutcome::FieldErrors`).
  4. **"Apply to {n} Marked"** (`main.rs`, the `Err(errors)` arm after `ApplyPlan::new`, about
     line 2953): the same status as point 3, besides `window.errors = errors` (unchanged); no
     confirmation is shown, as today.
  5. The status goes back to `EditorStatus::Idle` when the form changes, as every status does
     today (`form_after != form_before`, about line 2903; it runs before points 3 and 4 in the
     frame, so the text is shown until the next change). Check this order in the code; if it is
     not so, stop and say why instead of moving code.
- **Tests to add** (`editor.rs`'s `mod tests`):
  1. `lists_with_errors_names_each_list_with_a_wrong_date_once`: with `form_with(t143_metadata())`
     no list has an error; after `type_entry(…, ARTWORK_LIST, 0, AO_DATE, "förra året")` it is
     `[ARTWORK_LIST]`; a second entry added with `add_entry(ARTWORK_LIST)` (`t143_metadata()` has
     only one Artwork entry) and given a wrong date too still gives it once; after the first
     date is typed back to the read text (`"1893"`) and the second entry's date is emptied, it is
     empty again.
  2. `the_not_saved_texts`: `EditorText::NotSaved` with one field (Date Created): `"Nothing was
     saved. Correct the error in Date Created."` (the name as `EditorField::DateCreated.label()`
     gives it; write the expected string from that label); with the list Artwork or Object only:
     `"Nothing was saved. Correct the error in Artwork or Object in the Image."`; with Date
     Created and the Artwork list: `"Nothing was saved. Correct the errors in Artwork or Object in
     the Image, Date Created."` (the Artwork list sits after Person Shown, before Date Created, in
     `item_names`' order); with neither: `"Nothing was saved. Correct the errors in the
     form."`.
  3. `every_editor_text` (about line 4334, a list of examples) gets one `NotSaved` entry with
     one field, beside T180's entries.
  - No test of `Loupe` (it cannot be built in a test); the owner's hand check covers points 3–5:
    a wrong Artwork Date Created under a collapsed "More", then OK, then "Apply to {n} Marked"
    with two images marked.
- **Not in this task:** opening the collapsed "More" or "Address" line by itself, scrolling to
  the field, and "Copy All Fields" copying the form's text of a wrong date (unchanged: it copies
  what the form shows). The status of a write that failed (`SaveFailed`) is not changed.
- **Rules:** `docs/tasks/README.md` rules 1–13 (rule 13: the text is made in `editor.rs`'s
  `editor_text`, one template per sentence, the singular and plural in the same function; the
  names come from `item_names`, so from `EditorField::label` and `property_label`); no
  `unwrap`/`expect` outside tests.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added from the first backlog sort (owner, 2026-11-17: the status line says so too; OK and "Apply to {n} Marked", naming the fields and lists, U11c-Q9 in ADR 0017 §6.1); waits for T180 (the same files) and for its cold read |
| 2026-11-17 | Blocked | Cold read: doable with the named files; point 5's order checked (the status reset at about line 2903 runs before the Apply arm and both calls of `write_window`); no borrow problem; the names and their order checked against `item_names`. Changed: `fields` taken before `errors` is moved, and may be empty; test 1 adds a second entry with `add_entry` and clears both dates; the plural string written out; `NotSaved` added to `every_editor_text` (test 3). T180 touches the same enum and `every_editor_text`, so the line numbers will move. Still waits for T180 |
