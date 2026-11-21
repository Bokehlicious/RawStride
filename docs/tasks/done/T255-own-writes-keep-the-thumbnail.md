# T255. ADR 0026 F3f: RawStride's own write of an added file does not make its thumbnail anew (pw-loupe) — Done

- **Why:** the owner's hand check of T254 (2026-11-21, point 4): a JPEG copied into a folder on
  the PC while it was open, then turned, has its thumbnail jump once more after the turn. The
  file is kept in `added_files` (T254). The turn makes its thumbnail anew at once
  (`thumb_grid.forget` in the orientation code), and the cell's stamp, taken before the first
  request, stays. RawStride's safe write of the turn into the JPEG then changes the file's size
  and write time; the watcher gives `Changed`; the listing finds nothing came or went, and
  `made_anew_if_stale` finds the file differs from its stamp and makes the thumbnail anew once
  more. RawStride's metadata writes never change the pixels (as the histograms already rely on,
  `HistogramInspector::written`, U7c-Q7, T145), so a change RawStride made itself must not count.
  The same happens for a rating, an editor save or "Apply to All Marked" written into such a
  JPEG, DNG or TIFF. A RAW is not affected (its sidecar is written). The owner chose a task
  (2026-11-21) over accepting it as a known limit.
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/thumbs.rs` (point 1, and its tests)
  - `crates/pw-loupe/src/main.rs` (points 2–4)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **`AddedFiles::restamp(&mut self, path: &Path, stamp: Option<FileStamp>)`** (`thumbs.rs`),
     with a doc comment: for a kept path with a recorded stamp, the stamp becomes `stamp` (also
     `None`); a kept path without a stamp, or a path not kept: nothing. The mark "check when
     answered" is left as it is.
     And **`AddedFiles::kept_as(&self, absolute: &Path, cwd: Option<&Path>) -> Option<PathBuf>`**:
     the kept path whose resolved form equals `absolute`, resolved as `tab_pending` resolves a
     tab path (a relative kept path joined to `cwd`; an absolute one, or any path when `cwd` is
     `None`, as it is); `None` when none is.
  2. **After RawStride's own write (`main.rs`, `draw`):** in the loop over
     `self.metadata.take_written()` that already feeds `self.histograms.written`, for each answer
     with `succeeded` and `added_files.kept_as(&written.path, cwd)` (the current directory as
     `tab_pending` gets it) giving a kept path: `added_files.restamp(&kept,
     file_stamp(&written.path))`. Then, when that path is in the active tab's `files` and its
     cell is `CellState::Failed(_)` (a thumbnail read while the safe write had renamed the file
     away for a moment): `thumb_worker.forget_path(&kept)`, `thumb_grid.forget(index)`,
     `added_files.made_anew(&kept)` and `ctx.request_repaint()`, so the cell does not stay
     "Unreadable" until F5. A failed write changes nothing (the safe write leaves the file as it
     was). Nothing is read from disk for a path not kept.
  3. **While RawStride is writing the file (`main.rs`, `made_anew_if_stale`):** a kept path for
     which `tab_pending` (with the current directory) is true is skipped: not compared, not
     marked. Its stamp is renewed when the write answers (point 2), and the next listing compares
     against that. So a listing that comes between the write's last change of the file and its
     answer does not make the thumbnail anew.
  4. **When a thumbnail comes back (`main.rs`, `poll_thumbnails`, T254 point 6):** the mark is
     taken as today, but while `tab_pending` is true for the event's path nothing is made anew
     (the mark is then gone; accepted, see Not in). The doc comments of `follow_disk_changes`,
     `made_anew_if_stale` and `poll_thumbnails` say that RawStride's own writes do not count
     (T255).
- **Tests to add** (`thumbs.rs`, `AddedFiles`):
  - `restamp` replaces a recorded stamp (then `is_stale` is false for the new stamp and true for
    the old one), also with `None`; does nothing for a kept path without a stamp (the next
    `note_requested` still records its stamp) and for a path not kept (it stays not kept); leaves
    the mark (`take_check` true after `mark_check` and `restamp`);
  - `kept_as`: a relative kept path is found by its form joined to `cwd`; an absolute kept path
    by itself, also with a `cwd`; with `cwd` `None` a relative kept path is found only by itself;
    a path not kept gives `None`.

  Points 2 (the failed cell), 3 and 4 are conditions in `main.rs`'s drawing code, checked by the
  owner's hand check (as T254's points 3–6).
- **Not in this task:** RawStride's write renews the stamp of a kept file whatever changed it
  before, so a thumbnail made from the part of a file written so far stays until F5 when
  RawStride writes the file (a rating or turn) during its copy, or after the copy's last change
  but before the listing that would have found it changed; and a mark set by an earlier listing
  is dropped when its thumbnail comes back while RawStride writes the file. Each to
  `known-limits.md`, "Tabs following the disk", by the reviewer at Done. RawStride's own JPEG
  writes still give a `Changed` and a listing on the UI thread (`known-limits.md` already says
  so; T254).
- **Rules:** `docs/tasks/README.md` rules 1–13 (no new texts); `AGENTS.md` §3.
- **Hand check (the owner):** with a release build:
  1. Copy a JPEG into a folder on the PC that is open in RawStride, then turn it, and rate it:
     its thumbnail follows the turn without jumping once more, and nothing jumps at the rating.
  2. T254 point 1 again (several large RAW files copied to a USB stick): the whole thumbnails are
     shown without F5 once the copy has ended.
  3. Rate and turn a RAW (sidecar) and a JPEG that were already in the folder: nothing changes
     from before.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Blocked | Added from the owner's hand check of T254 (point 4 failed: a copied-in JPEG's thumbnail jumps once more after a turn; the owner chose a task over a known limit); waits for its cold read |
| 2026-11-21 | Open | Cold read: names, the coverage of `take_written` (ratings, labels, turns, editor and batch saves), absolute written paths and the per-frame order (`follow_disk_changes` before `draw`, `metadata.poll` clears pending) confirmed; added `AddedFiles::kept_as` (the path matching testable in `thumbs.rs`, `cwd` `None` as `tab_pending`), a failed cell remade after the write's answer (a read while the safe write renamed the file away), the hand-check note for the `main.rs` conditions, and Not in widened (a write after the copy's last change but before its listing; a mark dropped while pending), known-limits by the reviewer at Done |
| 2026-11-21 | Ready for review | Points 1–4 done: `AddedFiles::restamp` and `kept_as` (`thumbs.rs`); in `draw`, a succeeded write of a kept file renews its stamp and remakes its cell when it is `Failed`; `made_anew_if_stale` skips a kept file `tab_pending` reports; `poll_thumbnails` takes the mark but makes nothing anew while pending; doc comments of the three functions name T255. 2 tests (`restamp`, `kept_as`); test-inventory counts and `pw-loupe` description. check.ps1 all OK (2204 passed, 76 ignored), no clippy warnings |
| 2026-11-21 | Done | Reviewed: points 1–4 as written (`kept_as` resolves as `tab_pending`, `resolve_path` being `cwd.join`; nothing read for a path not kept). Reviewer's own check.ps1 all OK (2204 passed, 76 ignored), no clippy warnings. known-limits "Tabs following the disk": the T254 item on RawStride's JPEG writes amended, a T255 item (Not in, and a failed read whose answer comes after the write's answer, `take_written` being handled before `poll_thumbnails`). No CHANGELOG line (a fix inside the unreleased F3 line). Awaiting the owner's hand check (3 points) |
