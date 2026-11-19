# T198. The batch names every write notice of the other images (ADR 0017 U12d-Q10) — Done

- **Why:** when the metadata window writes several images ("Apply to {n} Marked"), the batch in
  `pw-loupe` shows the window image's notices as after its own write, but for the **other**
  images it lists only `WriteNotice::ConflictKept` and `WriteNotice::QualifiersKept` (T194).
  Their `Truncated`, `IimConvertedToUtf8` and `DigestNotWritten` are dropped by a `_ => {}` arm,
  so the user does not learn, for example, that a field was truncated in IIM in another image.
  The owner decided on 2026-11-19 (ADR 0017 U12d-Q10) that every notice of another image is a line
  of the summary, in the order the answers and their notices come; the `match` names every
  notice kind, so a new one cannot be dropped without a decision.
- **Not at the same time as T199:** both change `editor.rs` and the tests of `metadata.rs`.
  Whichever runs second finds the other's change in the working tree or committed; the texts of
  `notice_text` are not changed here.
- **Crates:** `pw-loupe` only. **Files:**
  - `crates/pw-loupe/src/metadata.rs`
  - `crates/pw-loupe/src/editor.rs`
  - `docs/test-inventory.md`
- **What:**
  1. `editor.rs`, `EditorText`: add, after `QualifiersKept` (about line 2727), three variants with
     a doc comment each in the form of the `ConflictKept` and `QualifiersKept` ones:
     - `TruncatedInIim { file: String, property: String }`: a line of the summary, the IIM value
       of `property` (its XMP id) truncated in `file` (ADR 0007 §5.2);
     - `IimRewrittenAsUtf8 { file: String }`: a line of the summary, the IIM texts of `file`
       written again as UTF-8 (ADR 0007 §5.4);
     - `NoDigestWritten { file: String }`: a line of the summary, no new IIM checksum written in
       `file` (ADR 0007 §5.2 Q1, §8).

     Their arms in `editor_text` (next to the `QualifiersKept` arm, about line 2947), exactly:
     - `"Truncated in IIM: {file} ({field})"` with `field = property_label(property,
       Language::En)`;
     - `"IIM texts rewritten as UTF-8: {file}"`;
     - `"No new IIM checksum written: {file}"`.
  2. `metadata.rs`, `batch_written` (about line 904), the loop over another image's
     `report.notices` (about line 923): the `match` gets an arm for each of the five
     `WriteNotice` kinds and **no** `_` arm. `Truncated { property }`, `IimConvertedToUtf8` and
     `DigestNotWritten` push the text of the matching new `EditorText` with `file_name(path)` to
     `batch.kept`, in the same loop and the same way as `ConflictKept` and `QualifiersKept`, in
     the order the notices come. The window image's notices are unchanged (`batch.notices`).
  3. Doc comments that name only the conflict and qualifier lines now say the list holds every
     notice of the other images: `BatchSummary.kept` (about line 223), `BatchSummary::lines`
     (about line 232), `batch_written` (about lines 901–903) in `metadata.rs`, and the second
     line of the doc comment of `EditorText::ConflictKept` (about line 2723) in `editor.rs`.
     The field keeps its name `kept`.
  4. **The existing test that changes on purpose:**
     `one_failure_stops_nothing_and_the_summary_counts_failures_and_kept_conflicts`
     (`metadata.rs`, about line 3333). Its third image's report is `[ConflictKept { property:
     "photoshop:City" }, DigestNotWritten]`, and the test today expects that image's
     `DigestNotWritten` to be dropped (`kept` holds one line, about line 3384). By U12d-Q10 it is
     now a line: change only `kept` and the matching line of the expected `lines()` (about line
     3392) to add `"No new IIM checksum written: pw-loupe-batch-c.DNG"` right after `"Unresolved
     conflict kept: pw-loupe-batch-c.DNG (City)"`. Nothing else in that test changes; its window
     notices stay in `notices`. No other existing test may change; if one fails, stop and ask.
- **Tests to add:**
  - `editor.rs`: three rows in the table of
    `every_apply_to_all_marked_text_in_singular_and_plural` (after the `QualifiersKept` row,
    about line 5374), with the exact texts: `TruncatedInIim { file: "d.jpg", property:
    "photoshop:Headline" }` → `"Truncated in IIM: d.jpg (Headline)"`, `IimRewrittenAsUtf8 { file:
    "e.jpg" }` → `"IIM texts rewritten as UTF-8: e.jpg"`, `NoDigestWritten { file: "f.jpg" }` →
    `"No new IIM checksum written: f.jpg"`.
  - `metadata.rs`, a new test after `a_list_with_qualifiers_kept_in_another_image_is_a_summary_line`
    (about line 3420), built the same way (`FakeSubmit`, `start_three`, `edits_written(id, path,
    Ok(report(notices)))`, `poll`, then `EditorEvent::BatchDone(BatchSummary { .. })` from
    `take_editor_events()`; check the whole `BatchSummary` and its `lines()`):
    `every_notice_of_another_image_is_a_summary_line`. `start_three` gives `a` (the window
    image, `pw-loupe-batch-a.jpg`, id 900), `b` (`pw-loupe-batch-b.ARW`, id 901) and `c`
    (`pw-loupe-batch-c.DNG`, id 902). The answers come in this order:
    1. `a` (900): `[Truncated { property: "photoshop:Headline" }, IimConvertedToUtf8]`;
    2. `c` (902): `[DigestNotWritten, Truncated { property: "dc:title" }, IimConvertedToUtf8]`;
    3. `b` (901): `[ConflictKept { property: "photoshop:City" }, DigestNotWritten]`.

    `BatchSummary` is `written: 3`, `total: 3`, no `failed`, `notices` = `a`'s two notices, and
    `kept` exactly, in this order:
    - `"No new IIM checksum written: pw-loupe-batch-c.DNG"`
    - `"Truncated in IIM: pw-loupe-batch-c.DNG (Title)"`
    - `"IIM texts rewritten as UTF-8: pw-loupe-batch-c.DNG"`
    - `"Unresolved conflict kept: pw-loupe-batch-b.ARW (City)"`
    - `"No new IIM checksum written: pw-loupe-batch-b.ARW"`

    `lines()` is `"Written to 3 of 3 images."`, those five lines, then `a`'s two notices as
    `notice_text` gives them: `"Headline was truncated in IIM; XMP has the whole text"` and
    `"The IIM texts were rewritten as UTF-8"`.
- **Not in this task:** the texts of `notice_text` (T199 for `DigestNotWritten`); anything in
  `pw-metadata`; the template and import paths; the "Written" count (U12d-Q8).
- **Rules:** `docs/tasks/README.md`, its point 13 for the texts ("truncated" and "checksum" are
  in `docs/glossary.md`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added: from the backlog sorted after T194 (owner, 2026-11-19; ADR 0017 U12d-Q10) |
| 2026-11-19 | Open | Cold read: a fresh agent found every name, helper and line number right (`WriteNotice` has five kinds, `editor_text` the only other `match` to grow, only the named existing test changes; the single-image save at `metadata.rs` ~2636 is not the batch). Unclear: which report went to which image ("second then first") and the new test's texts not written out. Fixed: the images named `a`/`b`/`c` with their ids and the order of the answers, the exact `kept` and `lines()` |
| 2026-11-19 | Ready for review | Done as written: three `EditorText` variants and texts (`editor.rs`), the batch's `match` over another image's notices names all five `WriteNotice` kinds with no `_` arm (`metadata.rs`), doc comments updated, `one_failure_stops_nothing_…` changed only in `kept` and `lines()` as point 4 says, the new test `every_notice_of_another_image_is_a_summary_line`, three rows in the singular/plural table; `docs/test-inventory.md` (pw-loupe 941, total 1932). `.\tools\check.ps1` all OK: 1932 passed, 76 ignored, 0 failed; no clippy warnings |
| 2026-11-19 | Ready for review | Review: every point and test as written (three `EditorText` variants with the exact texts, the batch's `match` over another image's notices names all five `WriteNotice` kinds with no `_` arm, the window image's notices unchanged, only the named existing test changed and only in `kept` and `lines()`, no panic path in non-test code); `.\tools\check.ps1` run again by the reviewer: all OK, 1932 passed, 76 ignored, 0 failed, no clippy warnings. Found: ADR 0017 had two rows U12d-Q9 (the "; " of the info panel, 2026-11-17, and this task's decision, 2026-11-19); this task's decision renamed U12d-Q10 (owner, 2026-11-19) in ADR 0017, ADR 0007 §18, this file, the task list and the code's comments |
| 2026-11-19 | Done | Approved by the owner. The owner's hand check ("Apply to {n} Marked" on images where another image than the window's gets a notice, for example a field truncated in IIM) follows |
