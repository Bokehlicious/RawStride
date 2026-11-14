# T129. ADR 0007 §5.1: an IIM value equal to the whole XMP text is no conflict — Open

- **Can start now.** Found by T127 on the level-C sample `PM-jpg.JPG`; the owner approved the fix
  on 2026-11-14. T127 waits for this task. Not at the same time as another task that changes
  `reconcile.rs`. It may run beside T128 (which changes other files of `pw-metadata`); both change
  `docs/test-inventory.md`, and the one finished second brings the total up to date.
- **Crates:** `pw-metadata` only. **Files:** `crates/pw-metadata/src/reconcile.rs`
  (`same_on_prediction` and the comment of `same_value`); `docs/test-inventory.md`.
- **Why:** Photo Mechanic writes the whole Description to IIM even when it is longer than the IIM
  limit (2191 bytes in `PM-jpg.JPG`, where IIM 2:120 allows 2000). XMP and IIM then hold exactly
  the same text. RawStride compares the IIM value with the value *it* would write, cut to 2000
  bytes (ADR 0007 §5.1), finds them different and, with no stored checksum, records an Open
  conflict between two identical texts. The write then keeps the conflict (`ConflictKept`) and
  writes no checksum (`DigestNotWritten`), so the file is never brought in line. An IIM value that
  is the whole XMP text is not a difference.
- **What:**
  1. **`same_on_prediction`** (`reconcile.rs`): the IIM value `actual` is also the same when it
     equals the prediction made **without the byte limit** (the same format and prefix handling
     as today, only `max_bytes` left out). Everything else is unchanged: the comparison is exact
     (no trimming, no case folding), blank against blank stays the same, a value cut to the limit
     stays the same, and an IIM value that is neither the cut nor the whole text is still
     different. This applies everywhere `same_value` compares text with text (the reconciliation
     and `edit.rs`'s "no change" test), which is wanted: an edit that sets the same long text again
     is no change.
  2. **The comment** of `same_value` and the module comment of `reconcile.rs` say so, citing
     ADR 0007 §5.1 ("or the whole XMP text, as a program that ignores the IIM limit writes it").
  3. **Nothing else changes.** The write plan's bringing in line (`bring_in_line`) is not
     touched: a Description that is no conflict and was not edited is not rewritten, so the
     2191-byte IIM value stays as Photo Mechanic wrote it until the user edits the Description;
     then the IIM value is cut to 2000 bytes with a `Truncated` notice as today (§5.2).
- **Not in this task:** any change in `write_plan.rs`, `iim.rs` or the reader; the level-C tests
  themselves (T127).
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Tests to add** (unit tests in `reconcile.rs`, as the existing ones):
  - a JPEG reading whose XMP Description (`dc:description`, Lang Alt) and IIM Caption-Abstract are
    the same 2191-byte text with Swedish letters, digest `Missing`: no conflict, the XMP value
    kept with origin XMP; the same with digest `Matches` and `Differs`: no conflict;
  - the same for a plain text property with a limit (City, 32 bytes) and a list entry (Keywords,
    64 bytes): an IIM value equal to the whole over-limit text is no conflict;
  - still a conflict: an IIM value that is the over-limit text cut at another place (neither at
    the limit nor whole), and one that differs in one character after byte 2000;
  - `same_value` of two equal over-limit texts is true; of two over-limit texts that differ only
    after the limit is false (so an edit there is still a change);
  - the existing tests stay as they are and pass.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated, and
  `git diff --stat` shows only the files above. The reviewer then runs T127's level-C tests.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Written by the reviewer after the review of T127 (finding 2); owner approved the fix; ADR 0007 §5.1 changed the same day |
