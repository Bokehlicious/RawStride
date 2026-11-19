# T206. Import recognises an earlier DNG/JPEG/TIFF by its creation time too (ADR 0005 choice 2) — Done

- **Why:** the import's destination check (`pw-ingest` `slot`, ADR 0005 choice 2) counts an
  existing DNG, JPEG or TIFF as "already ingested" only when its modification time is the card
  file's (or one hour off, choice 6 "The same time"). After T204 (ADR 0007 §9.4, Q14) every write
  by RawStride, the import template's included, gives the file the time of the write, so without
  the card's register (a locked card, a source without a drive letter, a deleted
  `.rawstride-ingest.txt`) the same shot would be imported again under a suffixed name. Found by
  the chain review of T204 → T205 (2026-11-19). Owner's decision (2026-11-19): the import also
  sets the copy's **creation time** to the card file's modification time, and the check accepts
  the creation time as well. `ReplaceFileW` keeps the creation time in every safe write (ADR 0007
  §9.4), so it survives RawStride's later writes. First task of the chain T206 → T204 → T205; it
  must be Done before T204.
- **Crates:** `pw-ingest`. **Files:**
  - `crates/pw-ingest/src/lib.rs`
  - `crates/pw-ingest/tests/ingest.rs`
  - `docs/test-inventory.md`
- **What:**
  1. `lib.rs`, `copy_verified` (about line 563): after `out.set_modified(modified)?`, also set
     the creation time to the same `modified`, in the same call: replace the line with
     `out.set_times(FileTimes::new().set_modified(modified)` plus, on Windows,
     `.set_created(modified)` (`std::os::windows::fs::FileTimesExt`, behind `#[cfg(windows)]`;
     other platforms set only the modification time as today). It applies to every copied file,
     the destination's and the backup's, images and sidecars alike. The doc comment of
     `copy_verified` says that the copy gets the card file's modification time as both its
     modification and its creation time.
  2. `lib.rs`, `slot` (about lines 280–307): an image matches when its size rule holds (unchanged)
     and **either** its modification time **or**, on Windows, its creation time is the same as the
     card file's by `same_time`. Read the creation time with `Metadata::created()`; an error or an
     unsupported platform counts as no match on that side. Sidecars are unchanged (`Present`).
     The doc comment's first point becomes: "images: the same size (for DNG/JPEG/TIFF any size,
     since metadata is written into them after the import) and the same modification time or
     creation time ([`same_time`]) mean "already ingested". RawStride's writes give a file a new
     modification time (ADR 0007 §9.4) but keep its creation time, which the import set to the
     card's time; files imported before this change kept the card's modification time instead."
  3. `tests/ingest.rs`, `files_edited_after_ingest_still_count_as_ingested` (about line 301): the
     "different RAW under the same name" is made by overwriting the imported `A001.ARW` in place,
     which keeps the copy's creation time (the card's, after point 1) and would now count as
     already present. A different file has its own creation time: after the `write`, set the
     file's creation time to `T0 + 5` with `FileTimes::set_created` (`#[cfg(windows)]`, as the
     new DNG test does), so the asserted clash stays. Removing the file first is not enough:
     NTFS "file tunneling" gives a file created under the same name within about 15 s of a
     deletion the old file's creation time (seen in the agent's first run, 2026-11-19). This is
     the only change to an existing test (reviewer, 2026-11-19, after the cold read).
  4. On other platforms than Windows the creation time is neither set nor read: both are behind
     `#[cfg(windows)]`, so no unused code or clippy warning appears there.
  5. Nothing else: the register (`register.rs`) still compares the card's own files by
     modification time, which nothing changes on the card.
- **Tests to add** (in `tests/ingest.rs`, after `files_edited_after_ingest_still_count_as_ingested`):
  - `a_dng_written_after_ingest_with_a_new_time_still_counts_as_ingested` (`#[cfg(windows)]`):
    import `B001.DNG` (time `T0`); afterwards overwrite the copy in place (the file's `write`
    helper: `std::fs::write` keeps the file and so its creation time) with another size and the
    modification time `T0 + 999`; a new plan copies nothing and counts it as already present. The
    same with the creation time also changed (`FileTimes::set_created`) to `T0 + 5`: a clash
    (`B001_1.DNG`).
  - `the_copy_gets_the_card_time_as_creation_time` (`#[cfg(windows)]`): after an import, the
    copy's `created()` and `modified()` both equal the card file's modification time (to the
    second), at the destination and at the backup.
- **Not in this task:**
  - Files imported before this task have the import's time as their creation time; they are still
    found by their kept modification time until RawStride writes them after T204. A re-import of
    such a card without a register after a later write gives a suffixed copy (no data lost). Noted
    in ADR 0005's known limits by the reviewer.
  - A volume without `ReplaceFileW` (the safe write's rename fallback, ADR 0007 §9.4) may not
    keep the creation time; same consequence as above. Noted in the same place.
  - The Tauri app's import (D7): unchanged.
- **Rules:** `AGENTS.md`; this README. No existing test changes except point 3; `cargo clippy`
  must show no new warning.
- **Done when:** `.\tools\check.ps1` is all OK, `docs/test-inventory.md` is updated (the two new
  tests in the `tests/ingest.rs` row with its count; the header's totals and "after T87–T203"
  become "after T87–T203, T206"), and `git diff --stat` shows only the files above (besides
  this file and the README's row).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added (owner's decision 2026-11-19 after the chain review of T204 → T205); first of T206 → T204 → T205. Cold read: `set_created` works on `File::create`'s handle (as in `pw-metadata` `fileops.rs`); NTFS keeps the creation time through the rename and an in-place overwrite (measured); the backup uses the same `copy_verified`; `a_card_read_two_hours_later_is_a_clash` still holds; `files_edited_after_ingest_still_count_as_ingested` would break → point 3 (remove the file first); non-Windows behind `#[cfg(windows)]` (point 4) |
| 2026-11-19 | Open | The agent stopped after the code and tests: `files_edited_after_ingest_still_count_as_ingested` failed (`A001.ARW` planned as present) although the file was removed first, because of NTFS file tunneling. Point 3 changed by the reviewer: set the creation time explicitly. The code in `lib.rs` and the two new tests are in the worktree; left: point 3, `check.ps1`, the inventory |
| 2026-11-19 | Ready for review | Point 3 done as changed: after the `write`, `A001.ARW` gets the creation time `T0 + 5` (`#[cfg(windows)]`). `lib.rs` points 1, 2, 4 and the two new tests as before; `docs/test-inventory.md` updated (ingest row 18 → 20, totals 1968 passed, 76 ignored, "after T87–T203, T206"). `.\tools\check.ps1` all OK (fmt, audit, test, clippy; no clippy warning). Note for the reviewer: NTFS file tunneling may also keep the creation time in the safe write's rename fallback, so the second "Not in this task" limit may not hold; worth checking before it goes into ADR 0005 |
| 2026-11-19 | Done | Review (reviewer): `copy_verified` sets the card time as modification and creation time in one `set_times` (Windows; elsewhere modification only); `slot` matches by modification or creation time, sidecars unchanged; point 3 sets the creation time after the `write`; the two new tests cover the destination and the backup and a clash once the creation time differs; `.\tools\check.ps1` all OK (fmt, audit, test, clippy; 1968 passed, 76 ignored), checked in `test-output.txt`. No extra code review (one crate, three files, not the safe write). The agent's note taken up: ADR 0005's known limit now says a volume without `ReplaceFileW` *may* not keep the creation time (NTFS name tunneling keeps it through the renames locally). Leftovers: the two limits are in ADR 0005 (known limits); the Tauri import unchanged (D7, retired with it). The owner's hand check follows (an import, a rating on an imported DNG/JPEG, then the same card again with the register removed: nothing copied) |
| 2026-11-19 | Done | Hand check (owner): approved; an import, a rating on an imported image (new modification time, T204), then the same card again to the same destination and template with `.rawstride-ingest.txt` removed: nothing copied, no `_1` file |
