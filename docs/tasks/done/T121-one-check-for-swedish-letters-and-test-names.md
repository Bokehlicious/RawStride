# T121. One check for Swedish letters; test names that say English — Done

- **Not at the same time as T120** (both change `crates/pw-metadata/src/worker.rs`).
- **Why:** T117 and T118 left two loose ends (reviewer notes, 2026-11-14):
  - the check that non-test string literals hold no Swedish letters exists in five copies: in
    `pw-loupe` (`info_panel.rs`, the text modules only) and in `pw-ingest` (`request.rs`),
    `pw-metadata` (`worker.rs`), `pw-thumbcache` (`extract.rs`) and `pw-viewcore` (`native.rs`),
    about 200 lines each;
  - test names still say "swedish" or "decimal_comma" where the texts they check are now English
    or use a decimal point.
- **Crates:** `pw-loupe`, `pw-ingest`, `pw-metadata`, `pw-thumbcache`, `pw-viewcore`; tests only.
  **Files:** `crates/pw-loupe/src/info_panel.rs`, `editor.rs`, `import_text.rs`, `inspector.rs`,
  `metadata.rs`, `rating_text.rs`, `shell_text.rs`; `crates/pw-ingest/src/request.rs`;
  `crates/pw-metadata/src/worker.rs`; `crates/pw-thumbcache/src/extract.rs`;
  `crates/pw-viewcore/src/native.rs`; `docs/test-inventory.md`.
- **What:**
  1. **One check for the whole workspace.** Keep one copy of the check, in `pw-loupe`'s
     `info_panel.rs` tests, built from the most thorough of today's copies (T118's: it blanks
     every `#[cfg(test)]` item, skips the files those items declare, comments, character literals
     and lifetimes, and reads raw strings). It reads the non-test code of every `.rs` file under
     `crates/*/src/` (found from `CARGO_MANIFEST_DIR`, one folder up), so a new crate is covered
     without a change. The only exception is `SV_LABELS` in `pw-loupe`'s `info_panel.rs`, as
     today. `src-tauri` is not read (it keeps its Swedish texts, GR8). The failure message names
     file and line, as today.
  2. **Remove the four other copies** and their two tests each from `request.rs`, `worker.rs`,
     `extract.rs` and `native.rs`. Keep the test that checks the check itself (a made-up source
     with a Swedish letter in a string, a comment, a raw string, a test module), once, next to
     the check.
  3. Before you change anything, run the combined check over the whole workspace once. If it
     finds a Swedish letter outside `SV_LABELS`, stop and report it (do not translate it here).
  4. **Rename the tests whose names no longer say what they check.** Only the name and, where it
     repeats the name, the doc comment change; the body does not. For example:
     - `inspector.rs`: `focal_length_and_aperture_have_a_decimal_comma_and_at_most_one_decimal`,
       `exposure_time_follows_pw_raw_with_a_decimal_comma` → "decimal_point";
     - `metadata.rs`: the tests that say `swedish_text`, `swedish_error`, `swedish_message`,
       `are_swedish` (about ten) → "english" or a name without the language;
     - `rating_text.rs`: `swedish_label_names`, `unsupported_text_is_swedish`;
     - `every_moved_…_keeps_its_swedish_wording` in `editor.rs`, `import_text.rs`,
       `rating_text.rs`, `shell_text.rs` → a name that says the texts keep their wording.
     Names that are about the Swedish table itself stay (`every_swedish_label_is_a_known_property_or_field`,
     `the_swedish_labels_of_the_english_exceptions_are_unchanged`, the `Language::Sv` tests). Find
     the rest with a search for `swedish` and `comma` in test names; list every rename in the report.
- **Tests:** the number of tests goes down by the removed copies (eight tests); no other test is
  removed or changed except its name. Update `docs/test-inventory.md` with the new counts.
- **Not in this task:** Swedish words in code comments; the Tauri app; any change to a text the
  user sees.
- **Rules:** `docs/tasks/README.md` rules 1–13.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Added from the reviewer's notes on T117 and T118 |
| 2026-11-14 | Ready for review | One workspace check in `info_panel.rs` (no Swedish letter found outside `SV_LABELS`); four copies and their 8 tests removed; 15 tests renamed; `check.ps1` all OK, 1215 passed, 43 ignored, 0 failed |
| 2026-11-14 | Ready for review | Reviewer: accepted, not committed. The four copies removed were only the check and its two tests (8 tests); the one check in `info_panel.rs` takes T118's scanner and now reads the non-test code of every crate under `crates/` (`pw-loupe` included, where T117's check read only the text modules), `SV_LABELS` the only exception; 15 test names changed, bodies untouched. The reviewer's `check.ps1` (with T119): fmt, test and clippy OK, 1215 passed, 0 failed, 43 ignored. Waits for the owner's approval |
| 2026-11-14 | Done | Owner: hand check done and approved; committed by the reviewer |
