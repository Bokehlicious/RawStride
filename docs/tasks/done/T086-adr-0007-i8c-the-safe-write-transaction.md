# T86. ADR 0007 I8c: the safe write transaction — Done

- **Can start after T84 and T85 are accepted.**
- **Crates:** `pw-metadata` only. The old `write_rating`/`write_iptc` stay unchanged (I9 moves them
  to this). The previews and LibRaw checks of §9.3 points 3–4 need `pw-raw` and LibRaw, which
  `pw-metadata` must not depend on: the transaction calls a check the caller gives
  (`ImageCheck`); the real one is written in I9.
- **Files:** new `crates/pw-metadata/src/safe_write.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); new `crates/pw-metadata/tests/safe_write.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §9 as a whole, §4.2 (a RAW is never changed; its sidecar gets the same
  transaction, §9.5).
- **What:**
  1. **`pub trait ImageCheck`** with `fn release(&self, image: &Path)` (close RAW engine sessions
     for the file, §9.1 point 2) and `fn check_copy(&self, original: &Path, copy: &Path, kind:
     FileKind) -> Result<(), String>` (§9.3 points 3–4); `NoImageCheck` does nothing (for tests and
     sidecars).
  2. **`ExifTool::write_safely(&mut self, image: &Path, plan: &WritePlan, checks: &dyn ImageCheck,
     journal: &Journal) -> Result<WriteReport, SafeWriteError>`**:
     - an empty plan writes nothing: `WriteReport::Unchanged`;
     - the target: the image for `Jpeg`/`Tiff`/`Dng`; for `Raw` the sidecar `<basename>.xmp`
       (`sidecar_path`); the RAW file itself is never written;
     - **9.1:** `checks.release(image)`; `can_write` (T85; for a new sidecar only the free space and
       the volume); `snapshot` and `check_file_type` (T84) of an existing target;
     - **journal** `begin` (T85) with the copy name `temp_name(target, "pwtmp")` and the backup name
       `temp_name(target, "pwbak")`;
     - **9.2:** one `execute` with `plan.options`, `plan.args`, `-o <copy>` and the target (an
       existing target is only read). A new sidecar is made **without a source file** (`plan`'s
       arguments and the copy name only): measured, `-o x.xmp` from a RAW copies the RAW's Exif into
       the sidecar. ExifTool's answer must report one file created and no warning (as
       `write_succeeded`), otherwise `WriteCopy`;
     - **9.3:** `snapshot` of the copy and `verify_copy` against the original's; for a new sidecar
       only `check_file_type` and that every key outside `System`, `ExifTool`, `File` comes from
       the plan; then `checks.check_copy` for `Jpeg`/`Tiff`/`Dng`; then `set_modified` (the
       original's time) and `flush`;
     - **9.4:** `replace` (T85), or `move_new` for a new sidecar; then the **final read**: size and
       `ImageDataHash` of the target equal the copy's; then the backup is deleted and the journal
       entry ended;
     - every failure leaves no `pwtmp` file and ends the journal entry when the original is intact;
       `OriginalAtBackup` keeps the entry and tries once to rename the backup back (§9.5).
  3. **`WriteReport { Written { target: PathBuf }, Unchanged }`** and **`SafeWriteError { stage:
     Stage, original: OriginalState, reason: String }`** with `Stage { Precheck, WriteCopy, Verify,
     ImageCheck, Replace, FinalCheck }` and `OriginalState { Intact, AtBackup(PathBuf), Replaced }`;
     `Display` in English naming the file, the stage and the reason (the Swedish texts are I10's).
     A failed final check keeps the backup and gives `Replaced`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`tests/safe_write.rs`, integration, **ExifTool required**; the JPEG fixture and
  the fake RAW of T82 in temporary folders; a temporary journal folder):
  - a JPEG with a plan (City, a two-line Description, keywords, IIM created): the values are read
    back with `read_metadata`, the modification time is the original's, the folder holds no
    `pwtmp`/`pwbak` file and the journal folder is empty;
  - an empty plan: `Unchanged`, the file's bytes unchanged;
  - a RAW without a sidecar: the sidecar is created with only the plan's values (no `XMP-exif`
    keys), the RAW's bytes unchanged; a RAW with a sidecar: the sidecar updated, its other values
    kept;
  - failures, each leaving the original **byte for byte** unchanged, no `pwtmp` file and an empty
    journal: a plan with an argument ExifTool refuses → `WriteCopy`; a plan with `-XMP:all=` on a
    file with XMP → `Verify`; an `ImageCheck` that returns an error → `ImageCheck`; an
    `ImageCheck` that opens the original with `share_mode(0)` and keeps it open → `Replace` with
    `Intact`; the original opened with `share_mode(0)` before the call → `Precheck`; a read-only
    original → `Precheck`;
  - one test **ignored by default**: every DNG in `PW_TESTDATA`, copied, written with a City and
    Description plan and `NoImageCheck`: `Written`, `ImageDataHash` unchanged, the values read
    back.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
