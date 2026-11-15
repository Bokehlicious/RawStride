# T145. ADR 0017 U7, the inspector's Histogram kept after RawStride's own metadata writes — Done

- **Can start now.** The owner approved the proposal of U7c-Q7 (at the end of this task) on 2026-11-15; it is written into the points below and into ADR 0017 §6.1.
- **Why:** T144 keeps the inspector's last 64 histograms and reads one again when its file's size
  or modification time changed (U7c-Q3). A metadata write by RawStride changes both for a DNG, a
  JPEG and a TIFF (embedded metadata, ADR 0007 §5), although it never changes the pixels
  (U7c-Q4). So after a rating, a label or an editor save of such a file, going back to it reads its
  histogram again (a full pass over the RAW data of a DNG, likely 100–300 ms, with "Reading …"
  meanwhile). Manufacturer RAW (ARW, RW2 …) is not affected: its metadata goes to the sidecar and
  the RAW file is left as it is. Found in the review of T144.
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/histogram.rs` (the kept results and
  `HistogramReads`, with their tests), `crates/pw-loupe/src/metadata.rs` (only to hand on the
  paths of the writes that succeeded, `EditsWritten` and `RatingWritten`), `crates/pw-loupe/src/main.rs`
  (`release_before_write` and the place in the frame loop where the written paths are passed to
  the inspector); `docs/test-inventory.md`.
- **What:**
  1. **The stamp before the write:** when a metadata write releases a file
     (`release_before_write`, U7c-Q4), the file's stamp (`file_stamp`) just before the write is
     noted for that path, in the shared `HistogramReads` (it is already reached from the writer's
     thread).
  2. **The kept result follows a write by RawStride** (U7c-Q7): when the metadata cache has the
     answer that a write of a file succeeded (an editor save, "Apply to All Marked", a rating or
     label key), the inspector's kept result of that path (both display modes) takes the file's
     stamp as it is now, **only if** the kept stamp equals the stamp noted before that write.
     Otherwise nothing is changed, so a change made outside RawStride before the write is still
     read again. The noted stamp is then forgotten. A write that failed changes nothing and the
     noted stamp is forgotten too.
  3. **Everything else stays as T144 made it:** a change outside RawStride is still read again;
     another folder still empties the kept results; the view core's statistics are not touched.
  4. **Texts:** none.
  If the metadata cache does not give the path of every successful write in one place (for
  example a batch answers only with a summary), stop and report which writes lack it; do not
  change `pw-metadata`.
- **Tests to add:** `histogram.rs`: a kept result whose file is changed by a write noted as
  RawStride's (the stamp noted before, then the file changed, then the write reported as done) is
  not read again in either mode; a file changed outside RawStride before the write (the kept stamp
  differs from the one noted) is read again; a failed write changes nothing; a write reported for a
  path with no kept result does nothing; `metadata.rs`: the successful writes of an editor save and
  of a rating key hand on their paths, a failed one does not; `main.rs`: `release_before_write`
  notes the stamp before the release returns (the T144 test reader may be reused).
- **Not in this task:** keeping results across a change outside RawStride; a cache on disk;
  changes to `pw-metadata` or `pw-viewcore`; the import's writes (they write new files, which have
  no kept result).
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Questions (the reviewer's proposals)

| # | Question | Proposal |
|---|---|---|
| U7c-Q7 | Is a kept histogram read again after RawStride's own metadata write of its file? | No: a write by RawStride does not change the pixels, so a kept result whose stamp equals the file's stamp just before that write takes the file's new stamp. A file changed in any other way is still read again (U7c-Q3) |

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added after the review of T144; waits for U7c-Q7 |
| 2026-11-15 | Open | The owner approved U7c-Q7 as proposed |
| 2026-11-15 | Ready for review | Stamp noted in `HistogramReads` by the metadata worker's release (not the import's); `MetadataCache::take_written` hands on every write answer with its path and outcome (a failed one too, so its noted stamp is forgotten); `HistogramInspector::written`; 7 tests |
| 2026-11-15 | Done | Reviewed: `check.ps1` all OK (1514 passed, 0 failed, 58 ignored), run again by the reviewer. A write that ends before its release (`Unchanged`, `Unsupported`, a read error) notes no stamp, so its answer can take a later write's stamp of the same file; that gives one extra read, never a wrong histogram. Not checked by eye in the running program; the owner checks T144 and T145 by hand after this commit. |
| 2026-11-15 | Done | Checked by hand by the owner and approved, after the inspector repaint fix (`bcaa34e`: the Histogram section stayed on "Reading …" until the next input; Info and Metadata were fixed the same way). |
