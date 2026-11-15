# T144. ADR 0017 U7, the inspector's Histogram: statistics of its own, and the 100 % view first — Blocked

- **Waits for the owner** on the proposals U7c-Q1–U7c-Q6 (at the end of this task).
- **Why:** T142 gave the preview's right column and the inspector a Histogram section (ADR 0017
  §2.6, U7-Q7). The statistics come from the view core, which opens images only for the preview
  and the loupe. The inspector is drawn in the contact sheet, where the view core has no image open,
  so its section shows "Reading …" unless the preview showed the same image last (T142's log). The
  review of T142 also left one thing unmeasured: the statistics borrow the current image's session
  for as long as they take (a full pass over the RAW data, likely 100–300 ms on a 60 MP file), so a
  100 % view asked for right then waits for them, and `latency.ps1`'s 100 % run registered only two
  steps.
- **Crates:** `pw-loupe`, `pw-viewcore`. **Files:** `crates/pw-loupe/src/histogram.rs` (the
  inspector's statistics reader, point 2, as plain Rust with its tests, beside what is there), the
  text module of T142 (`preview_text.rs`) only if a text is needed, `crates/pw-loupe/src/lib.rs`,
  `crates/pw-loupe/src/main.rs` (the inspector, `draw_inspector` and what calls it, the reader's
  start and its place in the frame loop, and the release before a metadata write, point 4);
  `crates/pw-viewcore/src/state.rs` and its tests (point 5); `docs/test-inventory.md`;
  `docs/glossary.md` only if a text is added.
- **What:**
  1. **One section, one source of truth** (U7c-Q1): the Histogram section of the inspector shows
     the statistics of the inspector's image (`inspector_subject`) in the tab's display mode
     (RAW or JPEG; a JPEG or TIFF file as itself, as the view core does). When the view core's last
     frame has them for that image and mode (T142's `last_statistics`), they are used and nothing is
     read.
  2. **A reader of its own** (U7c-Q2), with the rules of the Info section's Exif reader
     (`inspector.rs`, T110): one thread; a read is asked for only while the inspector and its
     Histogram section are shown and the image has been the current one for
     `INSPECTOR_READ_DELAY`; at most one read in flight; the newest image is asked for when it comes
     back; a result for another image is dropped. A read opens the file with the same
     `NativeBackend` (and so the same LibRaw build and limits) as the view core, asks
     `Backend::statistics`, and drops the session at once; it renders nothing.
  3. **Kept results** (U7c-Q3): the last 64 results by absolute path and mode, in memory only, so
     going back to an image shows its histogram at once; a result the preview gave fills the same
     store. Another folder empties it, as the Exif reader does; a result of a file whose size or
     modification time changed since is read again.
  4. **Metadata writes** (U7c-Q4): the reader never holds a file while it is written. Before a write
     of a file the reader has open, the write waits for the read to finish (as the view core's
     `release`, ADR 0014 §4.4); the read's result is still used, since a metadata write does not
     change the pixels.
  5. **The 100 % view first** (U7c-Q5): in the view core a statistics job is not started while the
     current image has a view at 100 % or more that is wanted and not yet shown; such a view is
     picked before the statistics. Measure with
     `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` on a release build before and after this
     task, and report the 100 % run. If its 100 % run still registers only a few steps, stop and
     report it: the script is not changed in this task.
  6. **Texts** (U7c-Q6): the texts of T142 ("Reading …", "No histogram for this file" and the
     rest); a read that fails shows "No histogram for this file". No new text unless the owner
     asks for one.
- **Tests to add:** `histogram.rs`: the reader's rules as the Exif reader's tests do them (no read
  while hidden or before the delay, one in flight, newest wins, a result for another image dropped,
  a stopped reader), a preview result used without a read, the 64 kept results (the oldest goes
  first, another folder empties them, a changed file is read again), a failed read gives "No
  histogram for this file"; `main.rs`: a write of the file being read waits for the read
  (a test reader that records the order); `state.rs`: a wanted 100 % view of the current image is
  picked before its statistics, and the statistics follow once it is shown.
- **Not in this task:** the clipping overlay (U7b); statistics of the marked images or of a region;
  statistics in the contact sheet's cells; a cache on disk; changing `latency.ps1`.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK, V1 (fit and 100 %) is not worse than before this
  task, and `git diff --stat` shows only the files above.

## Questions (the reviewer's proposals)

| # | Question | Proposal |
|---|---|---|
| U7c-Q1 | Where do the inspector's statistics come from when the preview already has them? | From the preview's last frame when it showed the same image and mode; otherwise from the reader of point 2 |
| U7c-Q2 | How are they read in the contact sheet? | A reader thread of its own in `pw-loupe`, with the Exif reader's rules (shown, 200 ms, one in flight, newest wins), opening the file with the same `NativeBackend` and dropping the session at once. Not the view core: it opens images for the preview only, and opening the contact sheet's image there would take memory and workers from the preview |
| U7c-Q3 | Are results kept? | The last 64 by path and mode, in memory; another folder empties them; a changed file is read again |
| U7c-Q4 | A metadata write of the file being read? | The write waits for the read, as for the view core's sessions; the result is kept |
| U7c-Q5 | The 100 % view while the statistics are computed? | A wanted 100 % view of the current image is picked before its statistics; `latency.ps1` before and after, the 100 % run reported |
| U7c-Q6 | New texts? | None: T142's texts, a failed read as "No histogram for this file" |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added after the review of T142; waits for U7c-Q1–U7c-Q6 |
