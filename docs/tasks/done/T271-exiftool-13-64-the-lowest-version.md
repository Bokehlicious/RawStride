# T271. ADR 0007 §9.7 Q20: ExifTool 13.64 is the lowest version the app writes with (pw-metadata, pw-loupe) — Done

- **Why:** ADR 0007 revision 30 (§9.7 point 1, Q20; approved by the owner 2026-11-22). The safe
  write will drop the digest pass and the TIFF restart (T272), which only ExifTool 13.64 or newer
  makes safe to drop. So the version below which every write is refused (§11, D9; today 12.67)
  becomes 13.64. T270 ships and pins 13.64. Reading with an older ExifTool keeps working, as D9
  says; only writes are refused, with the text the app already shows.
- **Before this task (the owner):** ExifTool 13.64 is installed in a folder of its own and put
  first on PATH, the 13.59 folder kept; otherwise `.\tools\check.ps1`'s tests that write with the
  real ExifTool are refused.
- **Crates:** `pw-metadata`, `pw-loupe` (only the version constant and texts and tests that name
  it; no behaviour change). **Files:**
  - `crates/pw-metadata/src/exiftool.rs` (`MIN_EXIFTOOL_VERSION`, the module comment, its tests)
  - `crates/pw-metadata/src/worker.rs` (`EXIFTOOL_TOO_OLD`, the version in its tests)
  - `crates/pw-metadata/src/test_support.rs` (the fake's `-ver` answer)
  - `crates/pw-metadata/src/lib.rs` (the test of `EXIFTOOL_TOO_OLD`)
  - `crates/pw-loupe/src/shell_text.rs` (its tests)
  - `crates/pw-loupe/src/editor.rs`, `crates/pw-loupe/src/metadata.rs`,
    `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/import_worker.rs` (tests and
    comments that name 12.67)
  - `docs/building.md` (the ExifTool row: "version 12.67 or newer" → 13.64)
  - `docs/dev-setup.md` (the ExifTool row: 13.59 → 13.64, now on PATH)
  - `docs/test-inventory.md` (the rows that quote 12.67, and the counts)
  - this task's own file (status and log)
- **What:**
  1. **The constant.** `MIN_EXIFTOOL_VERSION` becomes 13.64. Its doc comment and the module
     comment name ADR 0007 §9.7 Q20 (13.64 fixes the misread and the out-of-date checksum the safe
     write relied on working around, T269); the module comment's "at least 12.67, checked at
     start" from IPTC §5.1 is kept as the standard's floor and the new value explained beside it.
  2. **The texts.** `EXIFTOOL_TOO_OLD` ("ExifTool is older than 12.67") is a fixed text; make it
     say 13.64. If it can be built from `MIN_EXIFTOOL_VERSION` without a new dependency and
     without changing its type (`&str`, used in `WriteError`'s `Display`), say why you did or did
     not; do not change its type. The other user texts (`shell_text.rs`, `editor.rs`,
     `import_text.rs`) already use `MIN_EXIFTOOL_VERSION` and change by themselves.
  3. **The tests.** Every test that names 12.67 as the lowest version follows the new value:
     `exiftool.rs` (`MIN_EXIFTOOL_VERSION` equals 13.64; its `to_string`), `lib.rs`
     (`EXIFTOOL_TOO_OLD`), `shell_text.rs` (the message for 12.40 names 13.64;
     `exiftool_message_is_none_from_12_67` becomes `…_from_13_64` with 13.64 and a later version,
     and 13.59 gives the "too old" message), and the expected texts in `editor.rs`, `metadata.rs`,
     `import_text.rs` and `import_worker.rs`. Tests of `ToolVersion` that parse or order 12.67 or
     13.59 as plain versions are not about the lowest version and stay as they are; in
     `versions_are_ordered_by_number` and the `to_string` test only the asserts on
     `MIN_EXIFTOOL_VERSION` change. The test fake's `-ver` answer in `test_support.rs` (the code
     at the `"exit-after-first-request" | "slow"` arm and the two doc comments that name it)
     becomes 13.64: the `slow` mode's writes in `write_pool.rs` and the pooled worker tests depend
     on it. Tests that only check that a version is reported and never write
     (`worker.rs`'s `the_version_is_known_without_any_request`, `exiftool.rs`'s
     `version_asks_the_process`) keep 13.59. Change no test's meaning.
  4. **Comments.** `import_worker.rs`'s comment "older than 12.67" names 13.64. Comments that
     record what was measured with 13.59 stay.
- **Tests to add:** in `exiftool.rs`, that 13.63 is below `MIN_EXIFTOOL_VERSION` and 13.64 is not;
  in `shell_text.rs`, that 13.59 gives the "too old" message (the loop of the renamed test covers
  13.64 and a later version). `docs/test-inventory.md` is updated (its pw-metadata and pw-loupe
  rows quote 12.67, and the counts change; README rule 12). ADR 0007 §11 and D9 still say 12.67;
  the reviewer changes them at Done, not the agent.
- **Not in this task:** the digest pass, the restart, the checksum check (T272); CI, packaging and
  notices (T270); `CHANGELOG.md` (the reviewer's).
- **Rules:** `AGENTS.md` applies; texts the user sees follow README rule 13.
- **Done when:** `.\tools\check.ps1` is all OK with ExifTool 13.64 on PATH and `PW_EXIFTOOL_PL`
  unset (`exiftool -ver` from the same command, in the log), and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-22 | Blocked | Written after ADR 0007 revision 30 (§9.7, Q20); second of the chain T270 → T271 → T272. Waits for T270, and for the owner's update of the ExifTool on PATH to 13.64 |
| 2026-11-22 | Blocked | Cold read: the fake's 13.59 in `test_support.rs` (code and two doc comments) becomes 13.64, since the `slow` mode's writes in `write_pool.rs` and the pooled worker tests depend on it; the two tests that only report a version keep 13.59; only the `MIN_EXIFTOOL_VERSION` asserts change in `exiftool.rs`'s ordering and `to_string` tests; a new `shell_text.rs` assert for 13.59; `docs/test-inventory.md` is certain to change; ADR 0007 §11 and D9 are the reviewer's. `worker.rs`'s real-ExifTool version test needs 13.64 on PATH (the owner's precondition) |
| 2026-11-22 | Blocked | Chain review: see T270's log (the owner's install of 13.64 beside 13.59; `PW_EXIFTOOL_PL` unset and `exiftool -ver` logged) |
| 2026-11-22 | Blocked | T270 done (13.64 shipped, pinned and in CI). Waits for the owner to put ExifTool 13.64 first on PATH ("Before this task") |
| 2026-11-22 | Open | The owner's ExifTool 13.64 is first on PATH (`%LOCALAPPDATA%\Programs\ExifTool-13.64`, before the kept 13.59 folder); after a restart `exiftool -ver` answers 13.64 |
| 2026-11-22 | Ready for review | `MIN_EXIFTOOL_VERSION` 13.64 (doc and module comment name §9.7 Q20 and T269; IPTC §5.1's 12.67 kept as the standard's floor). `EXIFTOOL_TOO_OLD` stays a fixed `&str` saying 13.64: a `const &str` cannot be formatted from a `ToolVersion` without a macro crate or a type change, and `lib.rs`'s test pins the text. Tests follow 13.64 as the task lists; new `writes_need_at_least_13_64` (exiftool.rs) and `exiftool_message_says_that_13_59_is_too_old` (shell_text.rs); the renamed loop uses 13.64 and 13.70. Also changed, in a named file: `import_worker.rs`'s five T268 tests calling `observed_write(&dirs, "13.59", 2)` now pass 13.64, since they write through the fake and 13.59 would now be refused (meaning unchanged; the task's text did not list them). Version-only tests keep 13.59. `docs/test-inventory.md`: pw-loupe 1225 → 1226, pw-metadata 523 → 524, the 12.67 quotes, the total 2288. check.ps1 all OK (2288 passed, 77 ignored), no clippy warnings; same command: `PW_EXIFTOOL_PL` empty, `exiftool -ver` 13.64 from `%LOCALAPPDATA%\Programs\ExifTool-13.64` |
| 2026-11-22 | Done | Reviewed: the diff is only the listed files; the five T268 tests in `import_worker.rs` that now pass 13.64 write through the fake and would be refused with 13.59 (meaning unchanged). No other 12.67 or 13.59 names the lowest version (the rest parse or order versions, or record measurements). The reviewer re-ran `.\tools\check.ps1` with `PW_EXIFTOOL_PL` empty and `exiftool -ver` 13.64 from `%LOCALAPPDATA%\Programs\ExifTool-13.64` in the same command: all OK, 2288 passed, 77 ignored, no clippy warnings. ADR 0007 §11, §9.7 point 1, §17.1 (D9) and the revision 30 line updated by the reviewer; CHANGELOG (Changed). T272 Open. No hand check (no behaviour change with 13.64 on PATH) |
