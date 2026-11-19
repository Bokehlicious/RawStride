# T204. A written file gets the time of the write as its modification time (ADR 0007 Q14) — Open

- **Why:** ADR 0007 revision 26, §9.4 (Q14, owner 2026-11-19): a file the safe write writes, a
  sidecar and an image written in place alike, gets the time of the write as its modification
  time. Today the safe write sets the original's modification time back on the copy before the
  replace. In the test with DxO PhotoLab (ADR 0029 step P0, §1.1) that program, with its XMP
  synchronisation on, read a change only when the file was newer than its own `.dop` file, so it
  ignored every RawStride write until the file was touched. ADR 0030's catalog and incremental
  backups also find a changed file by its modification time. Chain T206 → T204 → T205 (ADR 0029
  step P0b): T206 first, because the import's destination check relied on the kept time (chain
  review, 2026-11-19); T205 (the rating percentages and `lr:hierarchicalSubject`) does not depend on
  this one but changes two of the same test files, so it waits.
- **Crates:** `pw-metadata`. **Files:**
  - `crates/pw-metadata/src/safe_write.rs`
  - `crates/pw-metadata/src/fileops.rs` (module comment only)
  - `crates/pw-metadata/tests/safe_write.rs`
  - `crates/pw-metadata/tests/edit.rs`
  - `docs/test-inventory.md`
- **What:**
  1. `safe_write.rs`, `write_and_verify` (about lines 412–424): remove the call
     `fileops::set_modified(&open.copy, original.modified)` and its `map_err` (lines 421–422).
     The flush after the `match` stays. Nothing else in the transaction changes: the creation
     time, attributes, ACL and alternate data streams are still kept by `ReplaceFileW` (§9.4).
  2. `safe_write.rs`, the comments that describe the removed step:
     - module comment, the **9.3** point (about lines 30–35): "then the original's modification
       time is set on the copy and the copy is flushed" becomes "then the copy is flushed; it
       keeps the time of the write as its modification time (§9.4, Q14)";
     - `Stage::Verify` (about lines 115–116): "setting the modification time and the flush"
       becomes "the flush";
     - the doc comment of `write_and_verify` (about lines 382–383): "sets the original's
       modification time on it and flushes it" becomes "flushes it".
  3. `fileops.rs`: `set_modified` is still used by `replace` (after `ReplaceFileW`, it puts the
     copy's own modification time back, which is now the time of the write) and by the test
     `replace_keeps_creation_time_attributes_and_streams_and_takes_the_copys_modification_time`.
     Leave both, and leave `replace` unchanged. Only the module comment (lines 6–7) changes:
     "[`flush`] and [`set_modified`] flush a written copy and set its modification time (§9.3
     point 5, §9.4)" becomes "[`flush`] flushes a written copy (§9.3 point 5); [`set_modified`]
     sets a modification time, used by [`replace`] (§9.4)".
  4. `tests/safe_write.rs`, `a_jpeg_is_written_safely_and_its_values_read_back` (doc comment
     line 385, lines 392 and 431–434): before the write, set the JPEG's modification time to
     2020-09-13 (`UNIX_EPOCH + 1_600_000_000 s`, with `std::fs::File::options().write(true)
     .open(..)?.set_modified(..)`; `fileops` is `pub(crate)` and not reachable from an
     integration test), take `let before = SystemTime::now();` just before the write, and assert
     afterwards that the file's modification time is not earlier than `before` minus 2 seconds
     (the margin covers a file system that stores times in coarser steps). The doc comment says
     "the modification time is the time of the write" instead of "the original's". The test
     files import `SystemTime`, `UNIX_EPOCH` and `Duration` from `std::time` as needed
     (`tests/edit.rs` imports only `Instant` today, `tests/safe_write.rs` nothing from it).
  5. `tests/edit.rs`, `a_rating_and_label_are_written_safely_to_a_jpeg` (doc comment line 228,
     lines 235 and 246–249): the same change as point 4 ("the modification time the time of the
     write" in the doc comment).
- **Tests to add** (in `tests/edit.rs`, after `a_raw_gets_a_sidecar_only_for_a_rating`):
  - `an_existing_sidecar_gets_the_time_of_the_write`: a fake RAW (`Scratch::raw`) whose sidecar
    already exists (written with the file's `write` helper, e.g. `-XMP-xmp:Label=Red`), the
    sidecar's modification time set to 2020-09-13 as in point 4; `write_rating_safely` with 2
    stars gives `Written` for the sidecar; the sidecar's modification time is not earlier than
    the time taken just before the write minus 2 seconds; the RAW file's bytes and modification
    time (read before the write) are unchanged (§4.2: the RAW is never written); `assert_clean`.
- **Not in this task:**
  - The Tauri path (`ExifTool::write_rating`, `write_iptc`, `write_tags` in `lib.rs`) keeps
    ExifTool's `-P` and the old modification time until it is retired (ADR 0007 D7, §18). Its
    test `check_embedded` in `tests/sidecar.rs` ("mtime preserved") stays as it is.
  - Moving, renaming and deleting files (`relocate.rs`, `recycle.rs`) do not write metadata and
    are unchanged.
  - The rating percentages and `lr:hierarchicalSubject` (§6.3 Q16, §8 Q15): T205.
  - The documents: the reviewer changes ADR 0007 §18, ADR 0029 §5 (P0b) and ADR 0004 when the
    chain is Done.
- **Rules:** `AGENTS.md`; this README. No other test changes; `cargo clippy` must show no new
  warning.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated (the two
  changed tests in the `tests/safe_write.rs` and `tests/edit.rs` rows say "the modification time
  the time of the write"; the new test added to the `tests/edit.rs` row with its count; the
  header's totals and its "after …" range include T204), and `git diff --stat` shows
  only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (ADR 0029 step P0b split into T204 → T205 by the owner, 2026-11-19). Cold read: the test imports of `std::time` named, the RAW's time read before the write; nothing else missing. Chain review (T204 → T205): the import's destination check (`pw-ingest` `slot`) relied on the kept modification time; owner's decision → T206 first, T204 waits for it. Also noted: the contact sheet's sort by modification time now moves a written JPEG/TIFF/DNG (intended, §9.4); the histogram cache re-stamps after RawStride's own write (U7c-Q7); ADR 0026's watcher and ADR 0030's change check must recognise RawStride's own writes (for those ADRs' steps) |
| 2026-11-19 | Open | T206 Done; T204 can start |
