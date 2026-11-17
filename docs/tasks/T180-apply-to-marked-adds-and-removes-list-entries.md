# T180. ADR 0017 U12d: "Apply to {n} Marked" adds and removes list entries (pw-loupe) — Blocked

- **Chain:** T179 → T180. Blocked: waits for T179 (`Edit::Entries` in `pw-metadata`). The owner's
  hand check of U12d follows this task.
- **Why:** the owner's job test (2026-11-17): keywords added to several marked images replaced each
  image's own keywords, because "Apply to {n} Marked" sends a changed list field to the other
  images as an `Edit::Set` of the whole list (`EditorForm::batch_field`, U12c-Q3, T126). And OK in
  the window wrote only one image, which looked like a failure. **Owner, 2026-11-17** (ADR 0017
  §6.1, U12d-Q1–U12d-Q4): only the difference of a list field goes to the other images, the
  confirmation names what is added and removed, and the window says that OK and Save write this
  image only.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/editor.rs`, `crates/pw-loupe/src/main.rs`
  (the line under the title, point 5; the existing tests named below), `docs/glossary.md` (only if
  point 4's texts need a term it lacks), `docs/test-inventory.md`, and this task's own file
  (status and log).
- **What:**
  1. **Which fields** (U12d-Q2): every editable field with `is_list()` and no `struct_field()`
     (today: Keywords, Person Shown, Organisation Name, Organisation Code, Creator, Subject Code,
     Scene Code, Model Release Id, Property Release Id; Supplemental Categories is not editable,
     `editable()`, so `batch_field` leaves it out as today). The
     structure lists (`EditorForm::ALL_LISTS`, `batch_lists`) are not changed: they are still
     replaced as whole lists (U11a-Q5). Every other field is as today.
  2. **What the other images get** (`EditorForm::batch_field`, U12d-Q1): for such a field whose
     text differs from its start text, with `start = list_entries(start text)` and
     `now = list_entries(text)`:
     - `add`: the entries of `now` not in `start` (exact text), in `now`'s order, once each;
     - `remove`: the entries of `start` not in `now` (exact text), in `start`'s order, once each;
     - for Creator (`copyright_management()`), `remove` is not sent: those entries are removed on
       the window's image only (U12d-Q2, ADR 0007 §8);
     - `batch.others` gets `Edit::Entries { property, add, remove }` when the sent `add` or
       `remove` is not empty; otherwise nothing for this field (for example only the order or
       the spaces changed).
     Removing only Creator entries therefore gives the other images nothing: `others` stays
     empty and `apply_allowed` keeps the button disabled, as for an emptied copyright field today
     (`main.rs` test at about line 8447); that is intended.
     `batch.this_image` is unchanged (`edits()`, the whole list as today); the chosen conflict
     values are as today. A list field never goes to `changed`, `emptied` or `emptied_here_only`
     any more.
  3. **`BatchEdits`** gets `pub list_changes: Vec<ListChange>`, in form order, one per list field
     of point 2 whose text differs from its start text and whose `add`, sent `remove` or
     removed-here entries are not empty:

     ```rust
     /// What "Apply to {n} Marked" does to one list field (ADR 0017 U12d-Q1–U12d-Q3).
     #[derive(Clone, Debug, PartialEq, Eq)]
     pub struct ListChange {
         pub field: EditorField,
         /// Added on every marked image that lacks them.
         pub adds: Vec<String>,
         /// Removed on every marked image that has them.
         pub removes: Vec<String>,
         /// Removed on the window's image only (Creator).
         pub removes_here: Vec<String>,
     }
     ```

     `field_count` counts each `ListChange` once. Update the doc comments of `BatchEdits`, its
     fields and `field_count`, and of `apply_allowed` (`main.rs`), to say this.
  4. **The confirmation** (`apply_confirmation`, U12d-Q3): after the "Changed:" line, one line per
     `ListChange`, in order, made by new `EditorText` variants (rule 13, whole sentences; the
     entries are `Vec<String>` in the variant, joined with ", " inside `editor_text` as data; the
     field's name is `field.label()`):
     - `ListAdds { field, entries }` → `"{field}: adds {entries}"`;
     - `ListRemoves { field, entries }` → `"{field}: removes {entries}"`;
     - `ListAddsRemoves { field, adds, removes }` → `"{field}: adds {adds}; removes {removes}"`;
     - `ListRemovesHere { field, entries }` → `"{field}: removes {entries} on this image only"`.
     A `ListChange` gives the one of the first three that fits its `adds` and `removes` (none when
     both are empty), and then, when `removes_here` is not empty, a `ListRemovesHere` line.
  5. **The line under the title** (U12d-Q4): a new `EditorText::ThisImageOnly` →
     `"OK and Save write this image only."`, drawn in `main.rs` right under the `MarkedImages`
     line (about line 2817), with the same condition (`marked_count > 1`).
  6. **`apply_allowed`** (`main.rs`) is unchanged: it allows the button when `batch.others` is not
     empty, which now also holds `Entries`.
  7. **An image left alone** (owner, 2026-11-17, ADR 0017 U12d-Q6, T179 point 5): a marked
     image whose list field has an `Open` conflict gets no change to that field, and its
     report names the property in a `WriteNotice::ConflictKept`, also when nothing was
     written. The batch summary already shows every `ConflictKept` of another image's report as
     "Unresolved conflict kept: {file} ({field})" (`crates/pw-loupe/src/metadata.rs`, the
     batch's write result, about line 858), whether the report is `Written` or `Unchanged`. No
     code is to change for this; if it does not hold, stop and say so. The confirmation
     (point 4) says nothing about conflicts (the images are not read before the write).
- **Tests to add** (`editor.rs` unless named):
  1. `an_added_keyword_is_added_to_the_other_images`: `dc:subject` `sol, hav`, typed `sol, hav,
     vinter` → `others == [Entries { "dc:subject", add ["vinter"], remove [] }]`, `this_image` the
     whole list, one `ListChange`.
  2. `a_removed_keyword_is_removed_from_the_other_images`: `sol, hav` → `hav`.
  3. `added_and_removed_keywords_in_one_edit`: `sol, hav` → `hav, is`.
  4. `an_emptied_keyword_list_removes_only_its_own_entries`: `sol, hav` → empty: `others` gets
     `Entries` removing `sol` and `hav` (not a `Clear`).
  5. `only_the_order_changed_sends_nothing_to_the_other_images`: `sol, hav` → `hav,  sol`:
     `others` empty, no `ListChange`, `this_image` has the new order.
  6. `a_removed_creator_stays_on_the_windows_image`: Creator `Anna, Bo` → `Anna, Cecilia`:
     `others` gets `Entries` adding `Cecilia` with no removal; the `ListChange` has
     `removes_here ["Bo"]`.
  7. `the_confirmation_names_what_each_list_adds_and_removes`: the four texts of point 4 and their
     order after "Changed:", including `"Keywords: adds vinter, älg; removes skog"`.
  8. `every_editor_text` (about line 4332) gets `ThisImageOnly` and the four list texts.
  - **Existing tests to change** (they expect the old whole-list behaviour, U12c-Q3): in
    `editor.rs`, `a_changed_text_lang_alt_list_and_check_box_go_to_every_image` (Keywords:
    `others` gets `Entries` adding `sol`, `hav`; `changed` no longer holds Keywords) and
    `an_emptied_copyright_field_is_emptied_only_on_the_windows_image` (Creator: a `ListChange`
    with `removes_here`, no longer in `emptied_here_only`; `field_count` unchanged);
    `each_t143_list_is_read_edited_and_written_as_one_struct_list_edit` (about line 7163: Keywords
    `sky` is an `Entries` adding `sky` in `others`, and leaves the "Changed:" line for its own
    line "Keywords: adds sky" after it; the count stays 10); and
    `the_confirmation_names_the_changed_emptied_and_copyright_only_fields_and_the_skipped_files`
    (about line 4784: its full `BatchEdits { .. }` gets `list_changes: Vec::new()`). Change them
    only as far as this task's points ask, and name every changed assertion in the report. If
    another existing test fails, stop and say which instead of changing it.
- **Not in this task:** structure lists (U11a-Q5); copy and paste (U12b); templates (U13).
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added with T179 from the owner's job test and U12d-Q1–U12d-Q5 (ADR 0017 §6.1); waits for T179 and its cold read |
| 2026-11-17 | Blocked | Cold read: doable with the named files. Changed: the real list of editable top-level list fields (Supplemental Categories is not editable); two more existing tests named (the T143 list test and the confirmation test's full `BatchEdits` literal); `field.label()` for the name; the list texts take `Vec<String>`; `ListChange` derives; `every_editor_text` named; a Creator-only removal leaves the button disabled, as intended; the stale doc comments named. Waits for T179 |
| 2026-11-17 | Blocked | From T179's review (owner, ADR 0017 U12d-Q6): an image with an `Open` conflict on a list field is left alone and named in the summary's "Unresolved conflict kept" line; point 7 added (no new code, the summary already shows it). Waits for T179 |
| 2026-11-17 | Blocked | From T179's review: T179 added the arm `Edit::Entries { property, .. } => !clear && property == list` in `BatchEdits::lists` only so the workspace builds. That closure checks `EditorForm::ALL_LISTS` (structure lists), which `Entries` refuses, so the arm is never true for an edit that can be written; T180 counts list fields through `list_changes` (point 3) and may leave the arm or make it `false`, saying which in its report. T179 is Done |
