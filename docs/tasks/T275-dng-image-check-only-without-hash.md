# T275. ADR 0007 §9.7 point 4 (Q23): LibRaw checks a DNG copy only when the original has no image data hash (pw-metadata, pw-imagecheck) — Open

- **Why:** ADR 0007 revision 32 (§9.3 point 4, §9.7 point 4, Q23; the owner's decision
  2026-11-22, after T273). T273 found no kind of DNG damage that only LibRaw's unpack refuses: the
  snapshot comparison (§9.3 points 1–3) refused all 42 damaged copies, the image check with LibRaw
  11, each also refused by the snapshot comparison (the analysis
  `docs/analyses/2026-11-21-metadata-write-time.md`, "Measured in T273"). LibRaw's open and unpack
  (`pw_libraw::Session::open_with_diagnostics`, one call) take 250–550 ms per DNG, about half of a
  DNG write. They now run only when the original has no `ImageDataHash`, since then nothing else
  checks the RAW data. When it has one, `verify_copy` has already refused any copy whose hash
  differs, and LibRaw is not run; the preview comparison (§9.3 point 3) always runs.
- **Crates:** `pw-metadata` (the safe write), `pw-imagecheck`. **Files:**
  - `crates/pw-metadata/src/safe_write.rs` (the trait `ImageCheck`, the new `DataHash`,
    `write_and_verify`, the module comment)
  - `crates/pw-metadata/tests/safe_write.rs` (point 4's test)
  - `crates/pw-imagecheck/src/lib.rs` (`RawImageCheck`, the module comment)
  - `crates/pw-imagecheck/src/tests.rs` (point 5's unit tests)
  - `crates/pw-imagecheck/tests/real_files.rs` (point 5's real-file test)
  - `docs/test-inventory.md` (the rows of the files above; the counts)
  - this task's own file (status and log)
- **What:**
  1. **Read first:** in `safe_write.rs`, the trait `ImageCheck`, `NoImageCheck`,
     `write_and_verify` (where `verify_copy` runs and then `check_copy`) and the module comment's
     §9.3 lines; in `verify.rs`, the hash comparison at the top of `verify_copy` (a copy whose
     `image_data_hash` differs from the original's is refused, `None` against `None` passes); in
     `pw-imagecheck`'s `lib.rs`, `check_copy` and `check_dng` and the module comment's §9.3 point
     4; the unit tests in `src/tests.rs` and `a_dng_copy_cut_inside_its_raw_data_is_refused_as_damaged`
     in `tests/real_files.rs`.
  2. **`DataHash` and a second trait method (`safe_write.rs`).** A new public enum
     `DataHash { Compared, Missing }` with a doc comment: whether §9.3 point 1 compared an image
     data hash (`Compared`: the original's snapshot has an `ImageDataHash`, so `verify_copy`
     found the copy's equal; `Missing`: it has none). `ImageCheck` gets a provided method
     `check_copy_with(&self, original: &Path, copy: &Path, kind: FileKind, hash: DataHash) ->
     Result<(), String>` whose default calls `check_copy(original, copy, kind)`; its doc says
     that the safe write calls it, that an implementation may leave out a check the hash
     already covers (ADR 0007 §9.3 point 4, §9.7 point 4), that a wrapper forwards it to the
     check it wraps (as `release_for_move`'s doc says for wrappers), and that an implementation
     whose `check_copy` calls `check_copy_with` must override `check_copy_with` (else the two
     call each other without end). `check_copy`'s signature is unchanged, so the other
     implementations (tests, examples, `pw-loupe`'s test check) do not change. Derive what the
     other small enums of the file derive (`Clone, Copy, Debug, PartialEq, Eq`). The module
     comment (its §9.3 line naming `ImageCheck::check_copy`) and the doc of `Stage::ImageCheck`
     name `check_copy_with`.
  3. **The safe write passes it.** In `write_and_verify`, after `verify_copy` has passed, call
     `checks.check_copy_with(self.target, &open.copy, kind, hash)` instead of `check_copy`, with
     `hash` `Compared` when `original.image_data_hash.is_some()`, else `Missing`. Nothing else in
     the transaction changes (the sidecar case still never calls the check).
  4. **The `pw-metadata` test.** In `tests/safe_write.rs`, a check that records the `DataHash`
     it was given (as `FailingCheck` and `LockingCheck` are written there), and a test that
     writing City into the small JPEG through `write_safely` gives `Compared`. Then find a file
     the safe write checks (JPEG, TIFF or DNG) for which ExifTool gives no `ImageDataHash` (try
     the file's `tiny_tiff()` first: `exiftool -api ImageHashType=SHA256 -ImageDataHash
     <file>`); if there is one, the same test (or a second) shows `Missing` for it. If there is
     none, say so in the log and leave `Missing` to point 5's unit test; do not change
     `verify.rs` or the snapshot to make one.
  5. **`RawImageCheck` (`pw-imagecheck`).** It overrides `check_copy_with`: for a DNG, `check_dng`
     runs only when `hash` is `Missing`; `check_previews` runs in every case, as now. Its
     `check_copy` keeps today's behaviour (it is `check_copy_with(…, DataHash::Missing)`), so the
     examples `check_timing.rs`, `write_timing.rs` and `dng_damage.rs`, which call `check_copy`,
     still measure the check with LibRaw and are not changed. The module comment's §9.3 point 4
     says that LibRaw runs only when the original had no image data hash (ADR 0007 §9.7 point 4,
     Q23). Tests:
     - `src/tests.rs` (the file's synthetic `tiff(true)`, which LibRaw cannot open): with
       `FileKind::Dng` and `DataHash::Compared`, an identical copy passes, and a copy with one
       byte changed in its thumbnail is refused with the same preview error as
       `a_copy_with_one_byte_changed_in_its_thumbnail_is_refused`; with `DataHash::Missing`,
       `check_copy_with` gives the same result as `check_copy` for the case of
       `a_dng_copy_libraw_cannot_open_passes_when_the_original_does_not_open_either`. Whether
       LibRaw ran cannot be seen with files LibRaw cannot open; the real-file test below shows it.
     - `tests/real_files.rs` (ignored, `PW_TESTDATA`): `a_dng_copy_cut_inside_its_raw_data_is_not_opened_by_libraw_when_the_hash_was_compared`:
       the cut copy of each DNG checked with `check_copy_with(…, FileKind::Dng,
       DataHash::Compared)` gives no `LibRaw: ` error: any `Ok` or any `Err` not starting with
       `LibRaw: ` passes (print which, and the time against `check_copy`'s). It shows that
       LibRaw was skipped by contrast with the existing
       `a_dng_copy_cut_inside_its_raw_data_is_refused_as_damaged`, where every cut copy gives
       LibRaw's error; that test is not changed.
     - `tests/real_files.rs`'s wrapper `Timed` forwards `check_copy_with` too (to
       `RawImageCheck::check_copy_with`, timed as its `check_copy` is), so
       `every_dng_is_written_safely_with_the_real_check` runs the app's path (`Compared`, no
       LibRaw) end to end; its assertions are not changed. The example `write_timing.rs` has
       the same kind of wrapper; it is not changed here and then times the write with LibRaw
       (the reviewer notes it in the backlog).
  6. **Run** after `.\tools\check.ps1`, with `PW_TESTDATA=D:\PhotoWorkflowData\RAW`:
     `cargo test --workspace -- --ignored every_real_` (the five tests T274 ran) and
     `cargo test --workspace --test real_files -- --ignored` (`pw-imagecheck`'s, and `pw-raw`'s
     file of the same name, which runs too); summaries in the log.
- **Tests to add:** points 4 and 5.
- **Not in this task:** `pw-libraw` and the C++ shim (no open-only call: the owner, 2026-11-22);
  `verify.rs` (the hash comparison is unchanged); the examples; JPEG and TIFF (their check is
  unchanged); `CHANGELOG.md` and ADR 0007 (the reviewer's, at Done).
- **Rules:**
  - `AGENTS.md` applies. The extra code review of the README applies (the safe write, two
    crates).
  - `PW_EXIFTOOL_PL` unset, and `exiftool -ver` (13.64) logged from the same command as
    `.\tools\check.ps1`.
  - No existing test is changed or removed (adding `DataHash` to a test file's `use` lines and
    point 5's forwarding in `Timed` are not changes of a test).
- **Hand check (the owner):** in a release build, rate about 20 DNGs at once (for example the
  `IMG_20261026_*.dng` and `TN*.DNG` of `D:\PhotoWorkflowData\RAW`, copied to a new folder) before
  and after this task, and compare the time until the last is saved; each rating is shown after a
  restart.
- **Done when:** `.\tools\check.ps1` is all OK, the real-file tests of point 6 pass (summaries in
  the log), and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Open | Written after ADR 0007 revision 32 (the owner, 2026-11-22, after T273: Q23 decided again, LibRaw only when the original has no `ImageDataHash`; no open-only shim call). A second trait method with a default keeps `check_copy`'s signature, so the test checks, `pw-loupe` and the examples do not change |
| 2026-11-22 | Open | Cold read: the named files suffice (no re-export of `safe_write` items; no non-test code wraps `ImageCheck`; `original` is a `FileSnapshot` at the call); every named helper and test exists; the small JPEG has an `ImageDataHash`. Changed: the module comment's §9.3 line and `Stage::ImageCheck`'s doc named; the new method's doc says wrappers forward it and warns of the recursion; `real_files.rs`'s `Timed` forwards `check_copy_with`, so the real-file write test runs the app's path; `write_timing.rs` left (backlog at Done); the real-file test's pass rule spelled out; `pw-raw`'s `real_files` runs too; `use` lines are not test changes |
