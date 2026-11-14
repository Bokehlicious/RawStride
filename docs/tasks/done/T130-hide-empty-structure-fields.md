# T130. Empty structure fields are not shown as values — Done

- **Can start now.** Found in the review of T127 (finding 3); the owner approved the fix on
  2026-11-14. It may run beside T128 and T129 (other files); all three change
  `docs/test-inventory.md`, and the one finished last brings the total up to date.
- **Crates:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/info_panel.rs` (`struct_text`);
  `docs/test-inventory.md`.
- **Why:** Photo Mechanic writes the empty sub-fields of Location Created as empty strings
  (`Sublocation`, `ProvinceState`, `WorldRegion` in the level-C sample `PM-jpg.JPG`). The reader
  keeps them, as it should (ADR 0007 §8: values are never dropped, and the write keeps them in the
  file). But `struct_text` shows every field, so the inspector, the info panel and the metadata
  window's Other Metadata show "Sublocation: , City: Exempelstad, Province or State: , Country
  Name: Sverige, World Region: ". A blank value counts as missing (ADR 0007 §5.1, MWG §4.2.3.3).
- **What:**
  1. **`struct_text`** leaves out a field whose value is blank: a `Text` or `Number` that is
     empty or only spaces and/or NUL (`pw_metadata::iim::is_blank`, the rule the rest of the
     program uses), a Lang Alt whose shown text is blank, an empty list, and a structure or list
     of structures whose own text is empty by this rule. The other fields keep their order and
     their texts.
  2. **A structure with no field left** gives an empty text; in a list of structures such an
     entry gives no line (no empty line between two locations).
  3. Only the shown text changes. The read values, the edits and the write are not touched, and
     the conflict list still shows both values of a conflict as they are.
- **Not in this task:** any change in `pw-metadata`; the structures' editing (U11a); the
  metadata window's form fields.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add:** `value_text` of a Location Created with three empty fields, one field of only
  spaces and one of only NUL gives only City and Country Name; of a structure whose every field is
  blank gives an empty text; of a list of two locations where the first is all blank gives the
  second's line only; a nested structure (Contact Info) with an empty field leaves it out; the
  existing `value_text_of_*` tests stay as they are and pass.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated, and
  `git diff --stat` shows only the files above. The owner checks by hand that `PM-jpg.JPG` shows
  its Location Created as "City: Exempelstad, Country Name: Sverige" in the inspector.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Written by the reviewer after the review of T127 (finding 3); owner approved |
| 2026-11-14 | Ready for review | `struct_text` leaves out blank fields, no line for an all-blank structure; 4 tests. check.ps1: test and clippy OK, fmt FAILED only in `editor.rs` (T128 in progress); inventory total left to the last of T128–T130 |
| 2026-11-14 | Ready for review | Reviewer: code and tests accepted, not committed; `check.ps1` all OK with T128–T129 (1318 passed, 53 ignored). Waits for the owner's hand check |
| 2026-11-14 | Done | Owner: hand check done and approved (`PM-jpg.JPG` shows only City and Country Name); committed by the reviewer |
