# T87. ADR 0007 I9a: ratings and labels through the write plan and the safe write — Done

- **Can start now.** No dependency change. Can run at the same time as T89 (no shared files).
- **Crates:** `pw-metadata` only. Nothing in RawStride uses the new function yet (T91 wires it in).
  The old `write_rating`, `write_iptc` and `write_tags` stay **unchanged** (the Tauri app still
  uses them; owner decision D7).
- **Files:** `crates/pw-metadata/src/read.rs`; new `crates/pw-metadata/src/edit.rs`;
  `crates/pw-metadata/src/lib.rs` (the `mod` line only); new `crates/pw-metadata/tests/edit.rs`;
  `crates/pw-metadata/tests/read.rs` (new tests only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §15 (embedded writes of ratings and labels move from
  `-overwrite_original_in_place` to §9), §9.2, §8 (Rating: written only when the user changes it;
  a missing value counts as 0), §4.2 (a RAW is never written). The write plan (T83) needs a
  `FileState`, which no code builds from a real file yet (`FileState` is built by hand in the
  tests). Measured by the reviewer with ExifTool 13.59 (see the step plan).
- **What:**
  1. **`read.rs`:** `MetadataRead` gains `pub state: FileState` (`write_plan::FileState`), filled
     from what `read_metadata` already reads, before `reconcile` takes the readings:
     - `kind`: the file kind;
     - for `Jpeg`, `Tiff` and `Dng`: `iim_present` when the file has a readable IIM block (an
       unreadable block, `FileNotice::IimBlockUnreadable`, counts as none), `iim_charset` and
       `iim_values` from its `IimValues` (`charset`, `values`), `digest` from its `IimRead`
       (`Missing` without IIM); `exif_present` from `ExifFields` (`image_description`,
       `copyright`, `artist`, `photographer`: `is_some()`);
     - for `Raw`: `iim_present: false`, `iim_charset: CodedCharset::Absent`, no `iim_values`,
       `digest: Missing`, `exif_present` all `false` (the plan never writes the RAW file; the
       sidecar has no IIM and no Exif).
     Update the module comment. Nothing else in `read.rs` changes.
  2. **`edit.rs`** (`pub mod edit`):
     - `pub struct EditOptions { pub settings: WriteSettings, pub fallback: Fallback }`
       (`Default`: both defaults; the settings screen is I10);
     - `pub enum EditError { Unsupported(String), Read(Error), Value(ValueError), Plan(PlanError),
       Write(SafeWriteError) }` with `Display` in English (each variant's own text; `Write` is the
       `SafeWriteError` text) and `std::error::Error`;
     - **`ExifTool::write_rating_safely(&mut self, image: &Path, rating: &Rating, options:
       &EditOptions, checks: &dyn ImageCheck, journal: &Journal) -> Result<WriteReport,
       EditError>`:**
       - `supports_rating(image)` false → `Unsupported` (same text as `write_rating`); stars
         outside −1…5 → `Unsupported` (same text as `write_rating`); nothing is read or written;
       - `read_metadata(image, options.fallback)`; an error is `Read`;
       - **Rating** (`xmp:Rating`): `Some(n)` → `Metadata::set("xmp:Rating",
         Value::Number(n.to_string()))`; `None` → `Metadata::clear("xmp:Rating")`. **Not a
         change** (nothing is set or cleared): the read value is the same number
         (`reconcile::same_value`), or `None` while the file has no Rating or a Rating whose
         number is 0 (§8: a missing value counts as 0, so a `0` written by another program is
         not removed);
       - **Label** (`xmp:Label`): `Some(text)` → `set("xmp:Label", Value::Text(text))`, `None` →
         `clear`; not a change when the read value is the same text, or `None` while the file has
         no Label;
       - `write_plan::plan(&metadata, &read.state, options.settings)`; an error is `Plan`;
       - `write_safely(image, &plan, checks, journal)`; an error is `Write`. An empty plan gives
         `WriteReport::Unchanged` there, so a rating that is already in the file writes nothing
         and a RAW without a sidecar gets none for `Rating::default()`.
       Errors of `set`/`clear` are `Value`.
  3. Module comment in `edit.rs` explaining the above, with the reference to §8, §9, §15.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs`,
  `safe_write.rs`, `verify.rs`, `fileops.rs`, `journal.rs` or the old write functions; if one of
  them must change, stop and explain.
- **Tests to add:**
  - `tests/read.rs` (new tests, ExifTool required, files made as the existing tests make them):
    a JPEG with IIM (City, `IPTCDigest` written) → `state.iim_present`, `iim_charset` `Utf8`,
    the City in `iim_values`, `digest` `Matches`; a JPEG without IIM → `iim_present` false and
    `digest` `Missing`; a JPEG with `IFD0:ImageDescription` and `IFD0:Artist` → only those two
    `exif_present` flags; the fake RAW with a sidecar → `kind` `Raw`, no IIM, no Exif flags;
  - `tests/edit.rs` (integration, **ExifTool required**; the JPEG fixture and the fake RAW of T82
    in temporary folders; a temporary journal folder; `NoImageCheck`):
    - a JPEG: 3 stars and `Red` → `Written`; `read_ratings` gives 3 and `Red`; the modification
      time is the original's; no `pwtmp`/`pwbak` file; the journal folder is empty;
    - the same rating again → `Unchanged`, the file's bytes unchanged;
    - `Rating::default()` on that JPEG → `Written`, `read_ratings` gives nothing;
    - a JPEG whose `xmp:Rating` is `0` (written with `ExifTool::execute`) and
      `Rating::default()` → `Unchanged`, the `0` still in the file;
    - −1 (rejected) is written and read back as −1;
    - 6 stars → `Unsupported`, the file's bytes unchanged;
    - a JPEG with IIM (City) and a matching `IPTCDigest`: after a rating, `read_metadata` shows
      `digest` `Matches` and the City unchanged in IIM and XMP;
    - the fake RAW without a sidecar and `Rating::default()` → `Unchanged`, no sidecar; with 2
      stars → the sidecar is created holding Rating 2, the RAW's bytes unchanged;
    - a read-only JPEG → `EditError::Write` with `Stage::Precheck`, the file unchanged;
    - one test **ignored by default**: every DNG, JPEG and TIFF in `PW_TESTDATA`, copied to a
      temporary folder: 3 stars and `Red` → `Written`, `ImageDataHash` unchanged (`snapshot`),
      read back with `read_ratings`; report the time per file in the log row.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
