# T233. ADR 0024 O2: writing the orientation through the safe write (pw-metadata) — Open

- **Why:** ADR 0024 §2.2 (O-Q3, O-Q4, accepted by the owner 2026-11-17): a rotation of a
  manufacturer RAW is written as `tiff:Orientation` in its sidecar, the RAW never changed; a
  rotation of a JPEG, TIFF or DNG is written as the Exif tag IFD0 Orientation in the file by the
  safe write, metadata only, with `tiff:Orientation` in the file's XMP kept in step when it exists.
  The pixels are never turned. O1 (T230–T232) reads the value; this task writes it; O3 adds the
  keys, the turn shown at once and the rollback in the shell. The value written is the new Exif
  value (1–8); the turn itself (mirror kept, §2.3) is computed by the caller in O3
  (`pw_viewcore::geometry::turned`), not here.
- **Crates:** `pw-metadata` (and one match arm in `pw-loupe` that keeps it building).
  **Files:** `crates/pw-metadata/src/edit.rs` (points 1–2), `crates/pw-metadata/src/verify.rs`
  (point 3), `crates/pw-metadata/src/worker.rs` (point 4), `crates/pw-metadata/tests/edits.rs`,
  `crates/pw-metadata/tests/level_c.rs` (tests), `crates/pw-loupe/src/metadata.rs` (point 5
  only), `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **The write** (`edit.rs`): a new method on `ExifTool` beside `write_rating_safely`:

     ```rust
     pub fn write_orientation_safely(
         &mut self,
         image: &Path,
         orientation: u16,
         checks: &dyn ImageCheck,
         journal: &Journal,
     ) -> Result<WriteReport, EditError>
     ```

     - A file kind that cannot hold ratings (`supports_rating` is false) is
       `EditError::Unsupported("orientation is not supported for <path>")`, and an `orientation`
       outside `1..=8` is `EditError::Unsupported("orientation <n> is outside 1..8")`; in both
       cases nothing is read or written (as `write_rating_safely`'s first two checks).
     - **The current values** are read in one ExifTool call (`self.execute`, numbers with `-n`,
       for example `-j -n -G1 -IFD0:Orientation -XMP-tiff:Orientation <file>`; the JSON is
       parsed in `edit.rs` with `serde_json` after splitting off ExifTool's `--- stderr ---`
       part, because `read.rs`'s `read_json` is private and `read.rs` is not changed), of the file that
       holds the metadata: for a manufacturer RAW its sidecar as stored
       (`crate::read::plain_sidecar`; no sidecar means no current value, and the RAW file itself
       is not read); otherwise the image. A failed read is `EditError::Read`. A value that is not
       a number in `1..=8` counts as missing.
     - **The plan** is a `WritePlan` with no options and no notices, whose `args` are:
       - **manufacturer RAW** (`supports_sidecar`): `-XMP-tiff:Orientation#=<n>`, unless the
         sidecar's `XMP-tiff:Orientation` already is `n`. The embedded Exif value of the RAW is
         not looked at: a sidecar value always wins (ADR 0024 §2.4), so turning back to the
         embedded value still writes it (T230 keeps `1`).
       - **JPEG, TIFF and DNG:** `-IFD0:Orientation#=<n>` unless IFD0 Orientation already is
         `n` (a missing tag is created, the exception to ADR 0007 §6.3's "never created" made by
         ADR 0024 §2.6); and `-XMP-tiff:Orientation#=<n>` only when the file's XMP has a
         `tiff:Orientation` that is not `n` (kept in step, never created).
     - The plan is written with `self.write_safely(image, &plan, checks, journal)`, the error
       mapped to `EditError::Write`. An empty plan is `WriteReport::Unchanged` there, so a value
       that is already in the file writes nothing and a RAW gets no sidecar for it. (The worker
       answers `Unchanged` and `Written` alike with `Ok(())`, as for ratings; O3 needs no
       difference.) Add `supports_sidecar` to the `crate::{…}` import.
     - Nothing else is written: no `xmp:Rating`, no checksum, no IIM; `tiff:Orientation` is not
       added to the property table, so `read_metadata` and the reconcile never show or compare it
       (for a RAW a sidecar value that differs from the embedded one is not a conflict, ADR 0024
       §2.4 and ADR 0007 rule 4 as amended).
  2. **The module comment** of `edit.rs` gets a paragraph for `write_orientation_safely` in the
     style of the others (the two refusals with their texts, the read, the plan per file kind,
     ADR 0024 §2.2 and §2.6).
  3. **The verification allows the Exif block a write creates** (`verify.rs`). Measured by the
     reviewer with ExifTool 13.59 on the small JFIF JPEG of `tests/edits.rs` (no Exif):
     `-IFD0:Orientation#=6` adds exactly `File:ExifByteOrder`, `IFD0:Orientation`,
     `IFD0:XResolution`, `IFD0:YResolution`, `IFD0:ResolutionUnit` and `IFD0:YCbCrPositioning`
     (all `1` except the byte order `MM`), and the validation counts do not change. Today
     `verify_copy` refuses that copy: `IFD0:Orientation` is allowed by the plan, but none of the
     five `EXIF_BLOCK_KEYS` is (the `File` group is not always allowed either). Add, beside `drops_exif_block` and in its style, a
     `creates_exif_block(original, copy, plan) -> bool`: true when the plan sets an `IFD0:` tag
     (an argument `-IFD0:<tag>=<value>` or `-IFD0:<tag>#=<value>` with a non-empty value), the
     original has no `File:ExifByteOrder` and the copy has it. Then, in `verify_copy`, a key of
     `EXIF_BLOCK_KEYS` that is missing in the original and present in the copy is allowed (only
     added, never changed or removed). Its doc comment names the measurement and ADR 0024 §2.6,
     and the module comment's list of allowed changes (lines 8–11) gets the same exception in a
     few words. Nothing else in `verify.rs` changes.
  4. **The worker** (`worker.rs`), in the same queue as the ratings (ADR 0024 §2.5, O-Q8):
     - `pub fn write_orientation(&mut self, path: PathBuf, orientation: u16)
       -> std::result::Result<RequestId, SubmitError>` beside `write_rating`, doc comment "Queues
       an orientation write without blocking. Writes run in request order." with ADR 0024.
     - a `Request::WriteOrientation { id, path, orientation }`, run exactly as `Request::Write`
       (`NoJournal` without a setup, `ExifToolTooOld` while `!running.may_write`, the error of a
       stopped ExifTool as there), calling `write_orientation_safely` with the setup's checks and
       journal, `.map(|_report| ())`, `.map_err(WriteError::from)`;
     - a new `Event::OrientationWritten { id, path, orientation, result:
       std::result::Result<(), WriteError> }` after `RatingWritten`, with a doc comment.
     - The module comment gets one line for it (as `write_edits` has one).
     - The existing test `reports_exiftool_start_errors_asynchronously` matches `Event`
       exhaustively (around line 960): add `| Event::OrientationWritten { .. }` to its pattern,
       nothing else (needed for the build, not a change of what it tests).
  5. **The shell keeps building** (`pw-loupe/src/metadata.rs`): one arm
     `WorkerEvent::OrientationWritten { .. } => {}` beside `WorkerEvent::RatingsRead { .. } => {}`
     (around line 1188). Nothing else in `pw-loupe`; the shell uses it in O3.
- **Tests to add** (existing tests unchanged, rule 5):
  - `tests/edits.rs` (real ExifTool, its `Scratch`, `tag`, `write` helpers; `tag(.., "IFD0:Orientation#")`
    and `tag(.., "XMP-tiff:Orientation#")` give the number, `-` when missing; `pw_metadata::Rating`
    is added to the imports):
    - `a_raw_is_turned_in_a_new_sidecar`: the fake RAW without a sidecar, orientation 6 →
      `Written` with the target `TN0001.xmp`, whose `XMP-tiff:Orientation#` is `6`; the RAW's
      bytes are still `FAKE_RAW`; `assert_clean`; orientation 6 again → `Unchanged`.
    - `a_turned_raw_keeps_its_rating_and_is_not_a_conflict`: on the fake RAW,
      `write_rating_safely` stars 3, then orientation 8, then stars 4: the sidecar holds
      `XMP-xmp:Rating` 4 and `XMP-tiff:Orientation#` 8; `read_metadata` of the RAW has no
      conflicts (`reconciled.metadata.conflicts()` is empty).
    - `a_jpeg_without_exif_gets_ifd0_orientation`: the small JPEG (no Exif), orientation 6 →
      `Written`; `IFD0:Orientation#` is `6`, `XMP-tiff:Orientation#` is `-` (not created);
      `assert_clean`; orientation 6 again → `Unchanged`.
    - `a_jpeg_with_exif_but_no_orientation_keeps_its_exif`: the small JPEG set up with
      `write(.., &["-IFD0:Make=Test"])`, orientation 8 → `IFD0:Orientation#` `8`,
      `IFD0:Make` still `Test`.
    - `a_jpeg_with_an_xmp_orientation_is_kept_in_step`: the small JPEG set up with
      `-IFD0:Orientation#=1` and `-XMP-tiff:Orientation#=1`, orientation 3 → both are `3`.
    - `bad_orientations_and_file_kinds_are_refused`: orientation 0 and 9 on the small JPEG, and
      orientation 6 on a `.png` path, are `EditError::Unsupported` with the texts of point 1;
      the JPEG's bytes are unchanged and no sidecar or temporary file exists.
  - `verify.rs` unit tests (its `snapshot` helper):
    - `creating_the_exif_block_with_an_ifd0_tag_is_allowed`: an original without
      `File:ExifByteOrder`, a copy with the six keys of point 3, the plan
      `-IFD0:Orientation#=6` → `Ok`.
    - `the_exif_block_keys_need_an_ifd0_write`: the same snapshots with the plan
      `-XMP-tiff:Orientation#=6` → `Err(VerifyError::Unexpected { .. })`.
    - `an_existing_exif_block_gets_no_free_keys`: an original with `File:ExifByteOrder` `MM` and
      `IFD0:XResolution` `72`, a copy with the same `File:ExifByteOrder` `MM`,
      `IFD0:XResolution` `1` and `IFD0:Orientation` `6`, the
      plan `-IFD0:Orientation#=6` → `Err(VerifyError::Unexpected { .. })` naming
      `IFD0:XResolution`.
  - `worker.rs` tests (`write_result` and `read_result` are for ratings; add a small helper of
    the same kind for `write_orientation` / `Event::OrientationWritten`):
    - `without_a_setup_an_orientation_write_fails_and_writes_nothing`: as
      `without_a_setup_a_rating_write_fails_and_writes_nothing`, with `write_orientation` →
      `Event::OrientationWritten` with `Err(WriteError::NoJournal)`, the id, path and orientation
      as sent, and no sidecar.
    - `an_old_exiftool_refuses_orientation_writes`: as
      `an_old_exiftool_refuses_rating_writes_but_reads` → `Err(WriteError::ExifToolTooOld)`.
    - in the module with the real ExifTool (beside
      `a_rating_is_written_safely_and_leaves_an_empty_journal`):
      `an_orientation_sent_after_a_rating_is_answered_after_it`: a rating write and then an
      orientation write (6) on a fake RAW written by the test, with no sidecar at the start,
      queued at once: `RatingWritten` comes first, then
      `OrientationWritten`, both `Ok`; the sidecar holds both values; the journal is empty.
  - `tests/level_c.rs` (ignored like the others, needs `PW_TESTDATA`):
    - `an_orientation_on_a_copy_of_the_xiaomi_dng`: a copy of `xiaomi_dng.dng` (its `Scratch`;
      the sample's IFD0 Orientation is 6, checked by the cold read), orientation 3 with
      `NoImageCheck` → `Written`; the copy's `IFD0:Orientation#` is `3` and its image data hash
      (`image_data_hash`) is the original's; orientation 3 again → `Unchanged`.
    - `an_orientation_on_a_copy_of_tn002363_keeps_xmp_in_step`: a copy of `TN002363.DNG` (IFD0
      and `XMP-tiff:Orientation` both 1), orientation 8 → `Written`; both are `8`; the image data
      hash is the original's.
    - In both, the samples are unchanged (`Fingerprints`).
- **Not in this task:** the keys, menu items, the turn shown at once, the rollback and the zoom
  rule (O3, plan row); computing a turn (O3, `pw_viewcore::geometry::turned`); a TIFF sample
  test (the level-C folder has no TIFF; a TIFF's IFD0 is written the same way as a DNG's);
  `IFD1:Orientation` (a JPEG's thumbnail; programs use IFD0) and an existing
  `XMP-tiff:Orientation` outside 1–8, which counts as missing and is left as it is;
  orientation in a RAW's own embedded XMP (the sidecar only, as for ratings; `backlog.md`);
  showing the orientation in Info or the metadata window (ADR 0024 §2.4 needs nothing shown,
  O-Q7); a `CHANGELOG.md` line (nothing a user sees changes until O3).
- **Rules:** `docs/tasks/README.md` rules 1–12. No new dependencies (`serde_json` is already
  one). No `.unwrap()` or `.expect()` outside tests. The level-C test is run with
  `.\tools\check.ps1 -Ignored` and `PW_TESTDATA` set (`AGENTS.md` §5); report its result.
- **Done when:** `.\tools\check.ps1` is all OK, the ignored run of `level_c` passes, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added (ADR 0024 O2). The reviewer measured point 3 with ExifTool 13.59 before writing (`-IFD0:Orientation#=6` on the JFIF JPEG adds the five Exif-block keys and the tag, no new warning; a new sidecar for `-XMP-tiff:Orientation#=6` holds only `XMP-x:XMPToolkit` and the tag). Safe-write area: an independent code review at Done (README, "An extra code review") |
| 2026-11-20 | Open | Cold read: doable with the named files. Fixed: the existing exhaustive `Event` match in `reports_exiftool_start_errors_asynchronously` may get the new variant (point 4); the current-value JSON is parsed in `edit.rs` (`read_json` is private); point 3's wording (all five block keys are refused today) and a line in `verify.rs`'s module comment; `an_existing_exif_block_gets_no_free_keys` keeps the byte order in the copy; the Xiaomi DNG already has IFD0 Orientation 6, so the level-C test turns it to 3, and a second test on `TN002363.DNG` (IFD0 and XMP both 1) checks the XMP kept in step; helper and import notes for the tests; IFD1 and out-of-range XMP values named under Not in this task |
