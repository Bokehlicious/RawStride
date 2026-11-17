# T179. ADR 0017 U12d: an edit that adds and removes entries of a list (pw-metadata) — Open

- **Chain:** T179 → T180. This task is the core part; T180 lets "Apply to {n} Marked" use it.
- **Why:** the owner's job test (2026-11-17): keywords added to several marked images replaced each
  image's own keywords. "Apply to {n} Marked" sends a changed list field as an `Edit::Set` of the
  whole list (U12c-Q3, T126), so every marked image gets the window's list. **Owner, 2026-11-17**
  (ADR 0017 §6.1, U12d-Q1, U12d-Q5): only the difference goes to the other images; an entry added
  in the window is added to each image that lacks it, an entry removed in the window is removed
  from each image that has it, and every other entry of each image is kept. The difference must be
  applied to each file's own values when that file is written: `ExifTool::write_edits_safely`
  already reads the file and then calls `apply_edits` on what it read (`edit.rs`), so a new kind
  of `Edit` is enough.
- **Crates:** `pw-metadata` (and two match arms in `pw-loupe` so the workspace builds).
  **Files:** `crates/pw-metadata/src/edit.rs`, `crates/pw-metadata/tests/edit.rs`,
  `crates/pw-loupe/src/editor.rs` (only the two arms of point 4), `docs/test-inventory.md`, and
  this task's own file (status and log).
- **What:**
  1. **The edit** (`edit.rs`): a third variant of `pub enum Edit`:

     ```rust
     /// Adds and removes entries of a list `property` (a `Multi` property that is not a
     /// structure, `Value::List`), applied to the file's own entries (ADR 0017 U12d-Q1).
     Entries {
         /// The XMP id of a property of the table.
         property: String,
         /// The entries to add, in order.
         add: Vec<String>,
         /// The entries to remove.
         remove: Vec<String>,
     },
     ```

  2. **Applying it** (`apply_edit` in `edit.rs`; a function of its own is fine):
     - The property must be in the table (`properties::property`), else
       `EditError::Value(ValueError::UnknownProperty(..))` as for `Set`.
     - It must be a property with `def.occurrence == Occurrence::Multi` whose `def.kind` is not
       `ValueKind::Struct(_)` (the properties whose value is `Value::List`; not
       `properties::list_kind`, which also answers for lists of structures), else
       `EditError::Unsupported` with an
       English reason that names the property (for example `"{property} is not a list of
       entries"`).
     - The current entries are the read value's entries (`metadata.get(property)` as
       `Value::List`), or none when the property is absent.
     - The result is: the current entries without every entry that is in `remove` (exact text,
       every occurrence), then each entry of `add` that the result does not hold yet appended at
       the end, in `add`'s order, once each (exact text, as a template's Append on a list,
       `template.rs`). Entries that are blank after `trim` in `add` are ignored.
     - A result equal to the current entries (the same `Vec<String>`, `==`) is **no change**:
       nothing is set or cleared, even when the property has an `Open` conflict (the conflict is
       kept, as for an unchanged field).
     - A changed, non-empty result is set with `set_if_changed`; an empty result (the property had
       entries, all removed) clears the property as `Edit::Clear` does (`metadata.clear`;
       `Metadata::set` refuses an empty list with `ValueError::Empty`). An absent property with
       nothing to add is no change, never a `clear` (`clear` marks the property changed even when
       it is absent).
     - Too many entries for the property is the error `Metadata::set` already gives.
  3. **The module comment** of `edit.rs` gets a bullet for **Entries** after **Clear**, saying
     the above in two or three sentences.
  4. **`pw-loupe` only so that it builds** (`editor.rs`), no other change there:
     - in the closure `has` of `BatchEdits::lists` (about line 1173), the arm
       `Edit::Entries { property, .. } => !clear && property == list`;
     - in `EditorForm::batch_lists` (about line 2167), the arm `Edit::Entries { .. } => false`.
     If any other `match` in the workspace fails to build, stop and say where instead of changing
     it.
- **Tests to add:**
  - In `edit.rs`'s `mod tests` (with the helpers `read`, `edited`, `changes` and a new
    `entries(property, add, remove) -> Edit`):
    1. `entries_are_added_at_the_end_and_the_old_ones_kept`: `dc:subject` `["sol", "hav"]`, add
       `["vinter", "älg"]` → `["sol", "hav", "vinter", "älg"]`.
    2. `an_entry_already_there_is_not_added_twice`: add `["hav", "hav", "is"]` to `["sol",
       "hav"]` → `["sol", "hav", "is"]`.
    3. `entries_are_removed_and_the_others_kept`: remove `["sol"]` from `["sol", "hav"]` →
       `["hav"]`; removing an entry the file does not have changes nothing.
    4. `add_and_remove_in_one_edit`: `["sol", "hav"]`, add `["is"]`, remove `["sol"]` →
       `["hav", "is"]`.
    5. `the_comparison_is_exact_text`: removing `"Sol"` keeps `"sol"`; adding `"Sol"` to
       `["sol"]` gives `["sol", "Sol"]`.
    6. `removing_every_entry_clears_the_property`: the property is absent afterwards (as
       `a_copyright_notice_may_be_cleared_by_an_edit` checks a clear).
    7. `adding_to_an_absent_property_sets_it`: no `dc:subject` → `["is"]`.
    8. `no_change_keeps_an_open_conflict`: a `dc:subject` with an `Open` conflict (as
       `the_same_text_with_an_open_conflict_is_a_change` builds one with `add_conflict`, its values
       `Value::List`s) and an `Entries` that adds an entry already there: no change, the conflict
       still `Open`.
    9. `entries_on_a_text_or_a_structure_list_are_unsupported`: `photoshop:City` and
       `Iptc4xmpExt:LocationCreated` give `EditError::Unsupported`; an unknown property gives
       `EditError::Value(ValueError::UnknownProperty(..))`.
  - In `crates/pw-metadata/tests/edit.rs` (real ExifTool, the `Scratch` JPEG, as
    `edits_to_a_jpeg_held_open_by_another_program_are_locked` calls `write_edits_safely`):
    10. `keywords_are_added_to_a_jpeg_and_its_own_kept`: write `XMP-dc:Subject` `sol` and `hav`
        with the file's `write` helper, then `write_edits_safely` with `Entries` on `dc:subject`
        adding `vinter` and removing `sol`; read back with `tag(.., "XMP-dc:Subject")`: `hav,
        vinter`; the result is `Ok(report)` with `report.report == written(&file)`. The journal is
        clean and the folder removed, as the
        other tests do.
- **Not in this task:** the shell's use of `Entries` ("Apply to {n} Marked", the confirmation,
  the line under the title) is T180. Templates (U13) are not changed.
- **Rules:** `docs/tasks/README.md` rules 1–12; no `unwrap`/`expect` outside tests. This task
  changes the crate of the safe write, so an independent review of the diff follows the
  reviewer's own.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Blocked | Added from the owner's job test and U12d-Q1–U12d-Q5 (ADR 0017 §6.1); waits for its cold read |
| 2026-11-17 | Open | Cold read: doable with the named files; every `match` on `Edit` is in `edit.rs` or the two arms of point 4 (`src-tauri` has none). Changed: the list test is `Multi` and not `Struct` (`list_kind` also answers for structure lists, so test 9 would have failed); "no change" is `==` on the entries; an absent property with nothing to add is never cleared; the conflict built with `add_conflict`; test 10 compares `report.report` with `written` |
