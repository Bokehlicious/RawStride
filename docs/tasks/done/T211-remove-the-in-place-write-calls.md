# T211. Remove the in-place write calls `ExifTool::write_rating` and `write_iptc` — Done

- **Why:** owner's decision (2026-11-19, ADR 0007 D7 and the open-items list after §17): the
  in-place writes `ExifTool::write_rating` and `ExifTool::write_iptc` (with the private
  `write_tags`) were kept only for the Tauri app (D7), which is retired (T207). They write with
  `-overwrite_original_in_place`, without the write plan, the safe write or the journal (§9), and
  since T207 only `crates/pw-metadata/tests/sidecar.rs` calls them; the program writes through the
  worker (`write_rating_safely`, `apply_template_safely`, `write_edits_safely`). Removing them
  leaves one write path. Coverage that only their tests give is moved to the safe write's tests,
  not dropped.
- **Crates:** `pw-metadata`. **Files:**
  - `crates/pw-metadata/src/lib.rs`
  - `crates/pw-metadata/src/edit.rs` (comments only)
  - `crates/pw-metadata/tests/sidecar.rs`
  - `crates/pw-metadata/tests/edit.rs` and `crates/pw-metadata/tests/template.rs` (moved tests,
    point 3)
  - `docs/test-inventory.md`
- **What:**
  1. `src/lib.rs`:
     - Remove `ExifTool::write_rating`, `ExifTool::write_iptc`, the private `ExifTool::write_tags`
       and `refused_write_error` (about line 427), which only `write_tags` uses.
     - In the test module `refused_write_tests` (about lines 590–676) remove only the three tests
       of `refused_write_error` (about lines 605, 623 and 636) and the helpers only they use.
       **Keep** `write_error_keeps_the_kind_of_the_error` (about line 650) and
       `the_typed_worker_refusals_keep_their_texts` (about line 666): they test `WriteError`, not
       the removed function. Rename the module to fit what is left (for example
       `write_error_tests`).
     - Reword the comments that describe the removed write: the module comment (about lines 7–13,
       in-place writing with `-P`), the doc of the `From<EditError> for WriteError` impl (about
       line 124, "checked as [`refused_write_error`] does"; the impl's own read-only check does not
       change), and the doc of `Iptc::tags` (about lines 278–279, ExifTool arguments) to say what
       it is used for now.
     - Keep `write_succeeded` (used by `safe_write.rs`), `Iptc::tags` (used by `Iptc::is_empty`),
       `supports_embedded`, `supports_rating`, `rating_file`, `sidecar_path`, `read_ratings` and
       the public `Error` variants (`Error::ReadOnly` and `Error::NotWritten` are no longer built
       by production code; they stay, and you name this in the report). If anything else becomes
       unused, stop and report.
  2. `src/edit.rs`, module comment: the three intra-doc links to the old functions (about lines 9,
     26 and 34, `[`ExifTool::write_rating`]`, `[`ExifTool::write_iptc`]`) and the D7 paragraph
     (about lines 72–74) are reworded so nothing links to a removed function; the error texts
     themselves do not change. `tools\check.ps1` does not run `cargo doc`, so check the links by
     search (done-when).
  3. `tests/sidecar.rs`: remove the seven tests that call `write_rating` or `write_iptc`, after
     moving what only they check to the safe path. The cold read mapped them as follows; check it
     and write the final pairs in the report.

     | Old test | Already covered by the safe path | To move (new test) |
     |---|---|---|
     | `write_read_update_clear` | JPEG set/reject/clear (`tests/edit.rs` ~231, ~278, ~316); RAW sidecar created (~401) and updated (~525) | reject and clear in a RAW's sidecar, the label in a sidecar → `tests/edit.rs` |
     | `existing_sidecar_content_is_preserved` | only via `write_safely` (`tests/safe_write.rs` ~515) | a sidecar's other content (for example a City) kept by `write_rating_safely` → `tests/edit.rs` |
     | `unicode_and_spaces_in_paths` | a JPEG named `bild ä.jpg` (`tests/edit.rs` ~125) | a RAW with its sidecar in a folder whose name has Unicode and spaces → `tests/edit.rs` |
     | `jpeg_and_tiff_are_written_in_place` | JPEG (`tests/edit.rs` ~231); TIFF only in the ignored ~753 | a TIFF rated by `write_rating_safely` (not ignored, `tiny_tiff`) → `tests/edit.rs`. The check that the modification time is kept is **dropped**: the safe write gives the time of the write (ADR 0007 §9.4, Q14) |
     | `a_write_to_a_read_only_file_is_an_error_and_keeps_the_old_rating` | read-only JPEG (`tests/edit.rs` ~549) | a read-only sidecar: old rating set first, the write fails, `WriteError::from(error)` is `WriteError::ReadOnly`, the old rating is read back → `tests/edit.rs` |
     | `real_dng_is_written_in_place` (ignored) | `every_real_dng_jpeg_and_tiff_is_rated_safely` (`tests/edit.rs` ~753, ignored) | nothing; clearing on a real DNG is covered by the JPEG/DNG clear tests' logic, and the modification-time check is dropped as above |
     | `iptc_template_in_sidecar_and_embedded` | `tests/template.rs` ~278, ~486 | an earlier rating in a sidecar kept by `apply_template_safely` → `tests/template.rs`. The check that a line break in a URL becomes a space is **dropped**: it was the old `Iptc::tags` conversion; the template keeps line breaks |

     - A moved test checks the same outcome through the safe path; it is not weakened
       (`AGENTS.md` §5). A check that holds only for the in-place write is dropped and named in the
       report (the two above, and "no `_original` file", which `-overwrite_original` gave).
     - Keep `which_file_holds_the_rating` (it tests `rating_file`) and the Windows module with
       `headline_written` (it uses the safe write).
     - Then remove what becomes unused in `sidecar.rs` (clippy `--all-targets` fails otherwise):
       the imports `Iptc`, `LocalizedValue`, `Rating` (line 13) if unused; `testdata` and
       `test_file` (used only by the DNG test); `tiny_jpeg`, `tiny_tiff`, `image_hash`,
       `check_embedded`, `write_while_read_only` unless a kept test uses them (a helper a moved
       test needs is copied to the file it moves to). `exiftool` and `temp_dir` are used only by
       the `cfg(windows)` module once the rest is gone: give them the same `cfg(windows)` (or
       move them into the module) so a non-Windows build has no dead code. Reword the module
       comment (lines 1–9; the real-DNG lines go) to say what the file now tests.
  4. `docs/test-inventory.md` (change it by script, `CLAUDE.md` §4): update line 21 (the
     `pw-metadata` unit tests: the count, and the text "the kind of a refused write (read-only,
     not written)"), line 33 (`sidecar.rs`: text and counts), the rows of `tests/edit.rs` and
     `tests/template.rs` (counts, and text if a new behaviour is named), and the totals in the
     text of line 4 ("after T87–T211") from the new `test-output.txt`.
- **Tests to add:** only the moved tests of point 3.
- **Not in this task:** the safe write (`safe_write.rs`, `write_plan.rs`) and the worker do not
  change. ADRs, `docs/tasks/plans/` and other documents that name `write_rating` /
  `write_iptc` as history (ADR 0007, ADR 0010, ADR 0013, `plans/adr-0007-milestone-8-steps.md`)
  are the reviewer's.
- **Rules:** no change to any `[dependencies]`. No other file changes; if one must, stop and
  explain (rule 2). Run the ignored real-file tests of `tests/edit.rs` once with `PW_TESTDATA`
  set (`D:\PhotoWorkflowData\RAW`) and give the result.
- **Review:** this changes `pw-metadata`'s public API next to the safe write, so the reviewer has
  the diff reviewed independently as well (`docs/tasks/README.md`, extra code review).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above and
  this file's status and log; and
  `git grep -nE "ExifTool::write_rating\b|write_iptc|refused_write_error|overwrite_original_in_place|\.write_tags\(" -- crates`
  finds nothing outside `crates/pw-metadata/src/worker.rs`'s own test helper `write_tags`
  (about lines 1986–2127, which calls `execute` itself) and the comments that say the old write
  was removed.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added after the owner's decision to remove the in-place write calls (ADR 0007 D7). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README and this file only): the first version would have removed two unrelated tests with `refused_write_tests` (they test `WriteError`); missed the unused imports and helpers in `sidecar.rs` (clippy), stale comments in `lib.rs` (module, `Iptc::tags`), inventory line 21, and that `check.ps1` does not check doc links; its done-when grep was wrong. Changed: point 1 keeps the two tests, point 3 has the cold read's coverage table with what moves and what is dropped (modification time kept, URL line break, `_original` file), the helpers and `cfg(windows)` named, point 4 adds line 21, the done-when grep rewritten |
| 2026-11-19 | Blocked | Waits for T210 (both change line 4 of `docs/test-inventory.md`); then Open, the cold read is done |
| 2026-11-19 | Open | T210 done; T211 can start |
| 2026-11-19 | Ready for review | Done as written. The in-place write, `refused_write_error`, its three tests and the seven `sidecar.rs` tests removed; five tests moved to `tests/edit.rs`, one to `tests/template.rs`. The copied `tiny_tiff` was given three BitsPerSample values: with one, ExifTool warns and the safe write refuses the copy. Dropped checks: modification time kept, URL line break as a space, no `_original` file. `check.ps1` all OK, 1955 passed, 72 ignored. The ignored real-file test of `tests/edit.rs` (`PW_TESTDATA`) failed on `TN005701_DxO.jpg` and `TN005702_DxO.jpg` (ExifTool `Warning: [minor] Error reading HiddenData`, refused in `WriteCopy`); the other 28 files passed. That code is not changed by this task |
| 2026-11-19 | Done | Review: the diff holds only the brief's files; the five moved tests in `tests/edit.rs` and the one in `tests/template.rs` check what the removed `sidecar.rs` tests checked, through the safe path; the two `WriteError` tests kept; the three dropped checks are the ones the brief names. Independent review (fresh agent): no bugs; no reference to a removed item left in the workspace; the `tiny_tiff` change (three BitsPerSample values) is sound; four new comment lines over 100 characters, rewrapped by the reviewer (`src/edit.rs` 26 and 34, `tests/edit.rs` 6 and 733). `Error::ReadOnly` and `Error::NotWritten` are no longer built by production code: backlog item "Leftover of T211". `.\tools\check.ps1` run by the reviewer after the rewrap: all OK, 1955 passed, 72 ignored, 0 failed, no clippy warnings. The ignored real-file test `every_real_dng_jpeg_and_tiff_is_rated_safely` rerun by the reviewer with `PW_TESTDATA`: fails on `TN005701_DxO.jpg` and `TN005702_DxO.jpg` (ExifTool `[minor] Error reading HiddenData`, the safe write refuses the copy), as the task agent saw; `safe_write.rs` and that test are unchanged, so it is not caused by T211: backlog item "A JPEG from DxO PhotoLab cannot be rated". No hand check: the program writes as before |
