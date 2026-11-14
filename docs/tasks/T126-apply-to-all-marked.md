# T126. ADR 0017 U12c: Apply to all marked — Open

- **Can start now.** The owner approved the proposals of U12c-Q1–U12c-Q7 (at the end of this
  task) on 2026-11-14; they are written into the points below and into ADR 0017 §6.1. Not at the
  same time as another task that changes `main.rs` or `metadata.rs`.
- **Crates:** `pw-loupe` only. **Files:** `crates/pw-loupe/src/editor.rs` (which edits go to the
  other images, the texts); `crates/pw-loupe/src/metadata.rs` (the batch: one write per file,
  its progress, its result, the wait at exit); `crates/pw-loupe/src/main.rs` (`M` with several
  marked, the button, the confirmation, the progress and the summary); `crates/pw-loupe/src/lib.rs`
  (re-exports only); `docs/glossary.md`; `docs/test-inventory.md`.
- **Why:** ADR 0017 §2.3.1, "Several images": "använd på alla markerade" writes the edited fields
  to every marked image, with one confirmation that names the count; fields not touched are left
  as they are on each image; it runs as a batch under ADR 0007 §9.5 (each file its own write, a
  summary of the files that failed); an emptied copyright field is not emptied on the other images
  (ADR 0007 §8); other emptied fields are named in the confirmation (as D14); files with open
  conflicts are listed (Q1). Today `M` does nothing with several images marked (`metadata_subject`).
  With T125 a caption is pasted into the window and then applied to the whole marking.
- **What:**
  1. **Opening** (U12c-Q1): with two or more images marked in the active tab (in the contact sheet
     or the preview), `M` opens the window on the current image if it is marked, else on the first
     marked image in the shown order. The window shows that image's values as today, and a line
     under the title: "{n} images marked" (one function). Previous / Next and their keys work as
     today; the marking does not change while the window is open (it is modal).
  2. **The button** (U12c-Q2): "Apply to {n} Marked" in the button row after "Save and Next",
     shown only with two or more marked images and enabled when the form has edits, no field
     errors, no write runs and no question is asked. OK, Save and "Save and Next" still write only
     the window's image.
  3. **What is written** (U12c-Q3): a pure function in `editor.rs` (for example
     `EditorForm::batch_edits`) gives two lists from the form:
     - **the window's image:** `EditorForm::edits()` as today (the changed fields and the chosen
       conflict values);
     - **every other marked image:** only the edits of the **changed fields** (a field whose text
       differs from its start text), never a chosen conflict value (it belongs to this file's
       conflict); and no `Clear` of a copyright-management field (Creator, Copyright Notice,
       Credit Line, Rights Usage Terms, Web Statement of Rights; ADR 0007 §8), which is emptied on
       the window's image only.

     Fields not changed are not written to any image. A changed field on a property with an open
     conflict in another file decides it there (T83, Q1); a field the file cannot take
     (`PlanError`) fails that file only.
  4. **The confirmation** (U12c-Q4): one question before anything is written, never per image:
     title "Apply to All Marked", the text "Write {k} changed fields to {n} images?", then the
     changed fields by name, the emptied fields by name ("Emptied: …"), the copyright fields
     emptied only on this image ("Emptied only on this image: …"), and the marked files that are
     skipped because their kind has no metadata support ("Skipped: {m} files"). Buttons
     "Write" and "Cancel"; Escape is Cancel. Nothing is written on Cancel and the edits stay.
  5. **The batch** (U12c-Q5): each image is its own write through the worker's `write_edits`
     (ADR 0007 §9.5, the same safe write as today), sent as the queue takes them; one failure stops
     nothing else. While it runs the window shows "Writing {i} of {n} …", the form is disabled and
     the window cannot move or close (as during today's write). When every write has answered:
     the window's image is read again and its form refilled (as after OK); the inspector and the
     window reread the images written as today (`window_written`, `inspector_written`).
  6. **The summary** (U12c-Q6), shown in the window until the next action: "Written to {w} of {n}
     images." and, when there are any, the failed files each with its reason (the text of today's
     write errors), and the files where an unresolved conflict was kept (a `ConflictKept` notice;
     "Unresolved conflict kept: {file} ({property})"). Other notices of the other images are not
     listed one by one; the window's own image shows its notices as today.
  7. **Exit** (U12c-Q7): the wait for writes at program exit (`EXIT_WAIT_LIMIT`, T105) also waits
     for the batch's writes and lists those still pending or failed, as for the window's write.
  8. **Texts:** every new text in `editor_text` (rule 13), with singular and plural in one function
     ("1 image" / "{n} images", "1 field" / "{n} fields"), and the new terms in
     `docs/glossary.md` (Swedish "Använd på alla markerade" as in ADR 0017).
- **Not in this task:** pasting or applying from the contact sheet without the window; templates
  (U13); a batch of ratings (the keys already rate several images); the reject move (U9); any
  change in `pw-metadata`, the write plan or the safe write.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add:** `metadata_subject` (or its replacement) with two or more marked: the current
  image when marked, else the first marked in shown order, in the contact sheet and the preview;
  one marked or none as today. `batch_edits` on hand-built `MetadataRead` values: an unchanged
  field written to no image; a changed text, Lang Alt, list and the check box to every image; a
  chosen conflict value only to the window's image; an emptied Creator, Copyright Notice, Credit
  Line, Rights Usage Terms and Web Statement of Rights only on the window's image; an emptied
  other field (City) on every image; field errors give no batch. The confirmation's lines for
  changed, emptied, copyright-only and skipped fields and files. With the fake submitter in
  `metadata.rs`: one `write_edits` per file, sent as the queue takes them after `QueueFull`; the
  progress count; one failure and the others written; the summary's written count, failed files
  and kept conflicts from `ConflictKept`; the window's image reread when all have answered; a
  stopped worker failing the rest; the exit wait counting and listing the batch. Every new text,
  singular and plural.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated and
  `git diff --stat` shows only the files above. The owner checks by hand on three marked JPEGs and
  an ARW: `M` opens on the current image with "4 images marked"; a changed Headline and City and
  an emptied Credit go to all four (seen in Photo Mechanic) after one question that names them;
  an emptied Copyright Notice is emptied only on the window's image; a file made read-only is
  listed as failed and the others written; Cancel writes nothing.

## Questions (answered by the owner, 2026-11-14: every proposal approved)

- **U12c-Q1 Opening.** With two or more marked, `M` opens the window on the current image (or the
  first marked one if the current is not marked), showing that image's values and "{n} images
  marked". *Alternative:* a separate window with empty fields for the whole marking (more work;
  the values of different images cannot be shown at once).
- **U12c-Q2 The button.** "Apply to {n} Marked" beside the other buttons; OK and Save still write
  only the window's image.
- **U12c-Q3 What goes to the other images.** Only the fields changed in the form, never the
  chosen conflict values, and no emptied copyright-management field (it is emptied on the
  window's image only).
- **U12c-Q4 The confirmation.** One question naming the count, the changed fields, the emptied
  fields, the copyright fields emptied only here and the skipped files; "Write" / "Cancel".
- **U12c-Q5 Running.** One safe write per file; the window shows "Writing {i} of {n} …" and is
  locked until all have answered; then the window's image is read again.
- **U12c-Q6 The summary.** "Written to {w} of {n} images", the failed files with their reasons,
  and the files where an unresolved conflict was kept.
- **U12c-Q7 Exit.** The exit wait includes the batch's writes.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Blocked | Written by the reviewer (ADR 0017 U12c, after T125); waits for U12c-Q1–U12c-Q7 |
| 2026-11-14 | Open | Owner: U12c-Q1–U12c-Q7 approved as proposed; written into ADR 0017 §6.1 |
