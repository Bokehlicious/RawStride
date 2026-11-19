# T199. The notice that no new IIM checksum was written names a reason that holds for both causes (ADR 0007 §8) — Open

- **Why:** `WriteNotice::DigestNotWritten` is given when an unresolved conflict remains (ADR 0007
  §5.2 Q1) **or** when a list with qualifiers is held back and not written to XMP (§8, T193; also
  an unchanged such list). Its text, `"No new IIM checksum was written, because a conflict is
  unresolved"`, is wrong in the second case. The owner decided on 2026-11-19 (ADR 0007 revision
  24) on one text that holds for both: `"No new IIM checksum was written, because IIM and XMP
  still differ"`.
- **Not at the same time as T198:** both change `editor.rs` and the tests of `metadata.rs`.
  Whichever runs second finds the other's change in the working tree or committed.
- **Crates:** `pw-loupe` only. **Files:**
  - `crates/pw-loupe/src/editor.rs`
  - `crates/pw-loupe/src/metadata.rs` (one expected text in a test only)
- **What:**
  1. `editor.rs`, `notice_text` (about line 2545), the `WriteNotice::DigestNotWritten` arm (about
     line 2556): the text becomes exactly `"No new IIM checksum was written, because IIM and XMP
     still differ"`. No other arm changes.
  2. **The existing tests that change on purpose,** only their expected text of this notice, from
     the old text to the new one, nothing else in them:
     - `editor.rs`, `every_notice_text` (about line 4509), its `DigestNotWritten` assertion (about
       line 4528);
     - `metadata.rs`, `one_failure_stops_nothing_and_the_summary_counts_failures_and_kept_conflicts`
       (about line 3333), the line of the expected `lines()` with the old text (about line 3393;
       it is the window image's notice).

     Search the whole workspace for the old text (`because a conflict is unresolved`) before you
     report; no `.rs` file may still hold it. If another test holds it, stop and ask.
- **Tests to add:** none; the two changed assertions of point 2 are the test of the new text. So
  `docs/test-inventory.md` does not change (it does not quote the old text, and no test is
  added).
- **Not in this task:** a reason field in `WriteNotice::DigestNotWritten` or anything else in
  `pw-metadata`; the batch's lines for other images (T198).
- **Rules:** `docs/tasks/README.md`, its point 13 for the text ("checksum" is in
  `docs/glossary.md`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added: from the backlog sorted after T194 (owner, 2026-11-19; ADR 0007 revision 24) |
| 2026-11-19 | Open | Cold read: a fresh agent found the three places of the old text exact (no other `.rs`, test or inventory text; `notice_text` the only builder, its callers `main.rs` and `BatchSummary::lines` follow), the new text true in both cases (an `Open` conflict and a held list with qualifiers leave IIM and XMP different, `write_plan.rs` ~276), and the clash with T198 already noted. Nothing changed: its "Done when" remark (the owner's uncommitted docs) is gone once the backlog commit is made |
