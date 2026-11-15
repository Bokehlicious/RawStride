# T140. ADR 0017 U5: the Tasks area: import progress, stop, pending writes and errors — Open

- **Can start now.** The owner approved the proposals of U5-Q1–U5-Q7 (at the end of this task)
  on 2026-11-15; they are written into the points below and into ADR 0017 §6.1.
- **May run beside T141** (recent and own values in the metadata window). Both change
  `crates/pw-loupe/src/main.rs`, in different functions: this task the left column's Tasks part
  (`draw_columns`, `hidden_import_status` and what replaces it), the import dialog's Hide and Stop,
  and where `metadata.poll`'s failures are taken in `draw`; T141 only the metadata window. Both add
  a module line to `lib.rs` and rows to `docs/glossary.md`: add lines, never rewrite another's.
  **Run `.\tools\check.ps1` only when the other task's agent is not running it.**
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/tasks.rs` (new: the Tasks area's state as
  plain Rust: the import row, the pending writes, the error list; with its tests);
  `crates/pw-loupe/src/tasks_text.rs` (new: its texts, rule 13); `crates/pw-loupe/src/lib.rs` (the
  module lines and exports); `crates/pw-loupe/src/settings.rs` (the section's remembered state);
  `crates/pw-loupe/src/import_text.rs` (only if a text moves from there); `crates/pw-loupe/src/main.rs`
  (only the parts named above); `docs/glossary.md`; `docs/test-inventory.md`. `pw-ingest`,
  `pw-metadata`, `metadata.rs` and `ratings.rs` do not change: the import worker's events
  (`ImportEvent`, `ImportDone`), `metadata.pending_writes()`, `metadata.batch_progress()` and the
  failures `metadata.poll` returns already give everything shown.
- **Why:** ADR 0017 §2.1 ("Tasks (left, below)") and step U5. Today the Tasks heading shows one
  line, the hidden import's status (U1-Q3, "until U5"); a rating write that fails shows only in the
  status line until the next message replaces it; an import's failed files are seen only in the
  import dialog.
- **What:**
  1. **Where** (U5-Q1): the Tasks section stays in the left column under the Library, a collapsible
     section like the Library's three (U4-Q7), its state remembered in `settings.txt`
     (`library_tasks = open|collapsed`). With nothing to show it says "No tasks", as today.
  2. **The import** (U5-Q2): while an import runs (shown or hidden dialog) a row with the source and
     the destination folder, a progress bar by bytes and the line "Importing {i} of {n}: {file}"
     (the IPTC step "Writing IPTC {i} of {n}: {file}" and "Ejecting {source}" as the dialog has
     them, `iptc_progress_text`, `ejecting_progress_text`). A click on the row shows the import
     dialog again. When it is done the row says "Import done: {n} files" (or "Import cancelled: {n}
     files", `hidden_done_text`), with the failed files counted, and stays until the next import
     or "✕" on the row.
  3. **Stop** (U5-Q3): a "Stop" button on the running import's row does what the dialog's Cancel
     does (`ImportDialog::cancel` and `ImportWorker::cancel`), without a question; the files copied
     so far are kept, as today.
  4. **Pending writes** (U5-Q4): one line each while they run, gone when done: "Writing ratings:
     {n}" (`metadata.pending_writes()`), "Saving metadata: {i} of {n}" while "Apply to All Marked"
     runs (`metadata.batch_progress()`), and "Saving metadata" while the metadata window's own write
     runs.
  5. **Errors** (U5-Q5): a list "Errors ({n})" under the rows, newest first, each with its time
     (`HH:MM`) and the text: every failure `metadata.poll` returns (rating writes, reads), and an
     import's failed files, IPTC errors and error (`ImportDone::failed`, `iptc_errors`, `error`).
     At most 200 are kept (the oldest dropped); a "Clear" button empties the list; not saved when
     the program ends. The status line still shows the one line it shows today.
  6. **Message boxes** (U5-Q6): the error list does not replace a box that needs an answer before
     the program can go on (ExifTool missing or too old at start, unfinished writes at the last
     exit); those stay as they are.
  7. **Names** (U5-Q7): Tasks, No tasks, Stop, Errors ({n}), Clear, Writing ratings: {n}, Saving
     metadata: {i} of {n}, Saving metadata; English (GR14), in `tasks_text.rs` and the glossary.
- **Tests to add** (`tasks.rs`, `tasks_text.rs`, `settings.rs`): the import row from each
  `ImportEvent` (progress, IPTC step, ejecting, done, cancelled, with failed files); the row kept
  after done until "✕" or a new import; the pending-write lines appear and go; errors newest first,
  the limit of 200, Clear; an import's failures added once; the section state saved and loaded, an
  unknown value warned and ignored; the texts through their functions.
- **Not in this task:** a history of earlier imports; saving the error list; a separate window
  for the tasks; stopping a rating write or "Apply to All Marked" (a batch runs to its end, ADR 0007
  §9.5); the status-line rules of ADR 0014 Q6.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (with T141's files if it runs at the same time).

## Questions (the reviewer's proposals; written into the points above)

| # | Question | Proposal |
|---|---|---|
| U5-Q1 | Where is the Tasks area? | In the left column under the Library, collapsible like the Library's sections, its state remembered |
| U5-Q2 | What does the import row show? | Source → destination, a progress bar by bytes and "Importing {i} of {n}: {file}" (IPTC and ejecting steps as in the dialog); a click shows the dialog; when done "Import done: {n} files" with failures counted, kept until "✕" or the next import |
| U5-Q3 | Does Stop ask first? | No: it does what the dialog's Cancel does; the files copied so far are kept |
| U5-Q4 | Which pending writes are shown? | Rating writes ("Writing ratings: {n}"), "Apply to All Marked" ("Saving metadata: {i} of {n}") and the window's own save, each only while it runs |
| U5-Q5 | What goes into the error list, and how long is it kept? | Failed rating writes and reads, an import's failed files and errors; newest first with the time; at most 200; "Clear"; not saved at exit |
| U5-Q6 | Do message boxes go away? | Only background errors go to the list; the boxes that need an answer (ExifTool missing or too old, unfinished writes at the last exit) stay |
| U5-Q7 | Names | Tasks, No tasks, Stop, Errors ({n}), Clear, Writing ratings: {n}, Saving metadata (: {i} of {n}); English (GR14), in the glossary |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added; waits for U5-Q1–U5-Q7 |
| 2026-11-15 | Open | The owner approved U5-Q1–U5-Q7 as proposed |
