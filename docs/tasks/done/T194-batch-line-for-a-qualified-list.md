# T194. The batch names a list with qualifiers that was not written in another image (ADR 0007 §8) — Done

- **Why:** T193 adds `WriteNotice::QualifiersKept { property }`, returned by
  `write_edits_safely` when a changed list with qualifiers was put back instead of written
  (ADR 0007 §8, owner 2026-11-18). When the metadata window writes several images, the batch in
  `pw-loupe` shows the window image's notices, but for the **other** images it lists only
  `WriteNotice::ConflictKept` (`metadata.rs`, about lines 918–927, `batch.kept`). A
  `QualifiersKept` of another image would be dropped and the user would not learn that its
  keywords were not changed.
- **Crates:** `pw-loupe` only. **Files:**
  - `crates/pw-loupe/src/metadata.rs`
  - `crates/pw-loupe/src/editor.rs`
  - `docs/test-inventory.md`
- **What:**
  1. `editor.rs`, `EditorText`: add `QualifiersKept { file: String, property: String }` next to
     `ConflictKept` (about line 2719), and its arm in the text function `editor_text` (about
     line 2777; next to the `ConflictKept` arm, about line 2935): `"List with qualifiers not
     written: {file} ({field})"` (the same form as `"Unresolved conflict kept: {file}
     ({field})"`) with `field = property_label(property, Language::En)`.
  2. `metadata.rs`, where the batch handles another image's `report.notices` (about lines
     918–927): a `WriteNotice::QualifiersKept { property }` pushes
     `editor_text(&EditorText::QualifiersKept { file: file_name(path), property })` to
     `batch.kept`, in the same loop and the same way as `ConflictKept`, in the order the notices
     come.
  3. Doc comments that say the list holds only unresolved conflicts now name both kinds:
     `BatchSummary.kept` (about line 223), `BatchSummary::lines` (about line 230),
     `batch_written` (about lines 899–900) in `metadata.rs`, and `EditorText::ConflictKept`
     (about line 2718) in `editor.rs`.
- **Tests to add:**
  - `editor.rs`: a row for the new text in the table of
    `every_apply_to_all_marked_text_in_singular_and_plural` (about line 5269, where the
    `ConflictKept` row is, about line 5341); the exact text with the label "Keywords".
  - `metadata.rs`, a new test built as
    `one_failure_stops_nothing_and_the_summary_counts_failures_and_kept_conflicts` (about line
    3321: `FakeSubmit`, `start_three`, `edits_written(id, path, Ok(report(notices)))`, `poll`,
    then `EditorEvent::BatchDone(BatchSummary { .. })` from `take_editor_events()`;
    `batch.kept` is private, so check `BatchSummary.kept`, `.notices` and `lines()`):
    `a_list_with_qualifiers_kept_in_another_image_is_a_summary_line`. The window image's report
    has `QualifiersKept { property: "dc:subject" }`; the two other images' reports, answered in
    the order second then first, have `[ConflictKept { property: "photoshop:City" },
    QualifiersKept { property: "dc:subject" }]` and `[QualifiersKept { property: "dc:subject" }]`.
    `BatchSummary.kept` holds exactly the three lines, with their exact texts, in the order the
    answers and their notices came; the window image's notice is in `BatchSummary.notices` and
    not in `kept`.
- **Not in this task:** anything in `pw-metadata` (T193); the template and import paths.
- **Rules:** `docs/tasks/README.md`, its point 13 for the text.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-18 | Blocked | Added: split from T193 after its cold read (the batch dropped every notice but `ConflictKept` for other images); waits for T193 |
| 2026-11-18 | Blocked | Cold read: a fresh agent found the named places right and no other `match` on `EditorText`; `batch.kept` is private (the test checks `BatchSummary`), the test to copy is at `metadata.rs` ~3321, the `ConflictKept` text test is a table row, four doc comments still say "conflicts" only (point 3), and one notice alone would not show order or a wrong arm. Fixed: the text in the form of the `ConflictKept` line, point 3, a test with two other images and both notices in one report, exact texts |
| 2026-11-18 | Open | T193 is Done, so the task is no longer blocked; `metadata.rs` is unchanged by T193, the line numbers in `editor.rs` are about 4 lower than now (T193 added the `QualifiersKept` arm of `notice_text`) |
| 2026-11-19 | Ready for review | Done as written: `EditorText::QualifiersKept` and its text in `editor.rs` (with a row in the text table), the batch's loop over another image's notices in `metadata.rs` now a `match` pushing `ConflictKept` and `QualifiersKept` lines in the order they come, the four doc comments, the test `a_list_with_qualifiers_kept_in_another_image_is_a_summary_line`, `docs/test-inventory.md` (pw-loupe 938 → 939, total 1906 passed, 72 ignored); `.\tools\check.ps1` all OK |
| 2026-11-19 | Ready for review | Review: every point and test as written, `docs/tasks/` diff only status and log row, `git diff --stat` only the named files, `test-output.txt` newer than the code (1906 passed, 72 ignored, 0 failed, the sum of every `test result:`; no clippy warnings). Noted, not changed: another image's `Truncated`, `IimConvertedToUtf8` and `DigestNotWritten` are still dropped by the batch (as before T194), and the `match` ends in `_ => {}`, so a new notice kind would be dropped without a decision |
| 2026-11-19 | Done | Approved by the owner. The leftover (another image's other notices dropped, the `_ => {}` arm) is in `backlog.md` (ADR 0007 item, "Leftovers from the review of T194"). The owner's hand check of the batch line follows |
| 2026-11-19 | Done | Hand check (owner): approved; a batch write where an image other than the window's has Keywords with qualifiers shows "List with qualifiers not written: <file> (Keywords)" in the summary |
