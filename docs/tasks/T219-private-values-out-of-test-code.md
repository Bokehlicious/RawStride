# T219. Private values out of the test code (public-release plan R7) — Open

- **Why:** the read-through of the documents before the repository is made public (public-release
  plan R7, 2026-11-19; owner: "ja, enligt förslagen") found three private values from the owner's
  own machine and photos that are also written in test code: the serial number in the name of the
  owner's monitor profile, the city in the Location Created of the owner's level-C sample, and the
  owner's real folder names. The documents were fixed by the reviewer (2026-11-19); this task
  replaces the same values in the tests with made-up ones. Test code only: no behaviour changes.
- **Crates:** `pw-loupe`, `pw-metadata`. **Files:** `crates/pw-loupe/src/shell_text.rs`,
  `crates/pw-loupe/src/info_panel.rs`, `crates/pw-loupe/src/library.rs`,
  `crates/pw-loupe/src/library_text.rs`, `crates/pw-metadata/tests/level_c.rs`.
- **What:**
  1. `shell_text.rs`, test `the_window_title_names_the_profile`: the profile file name
     `CX240(…)08CAL (DP).icc` (the dots stand for the monitor's serial number) becomes `Monitor 1.icc`, in the input and in the expected
     title (`"RawStride  ·  Display profile: Monitor 1.icc"`).
  2. `info_panel.rs`, test `value_text_of_location_leaves_out_blank_fields`: the city of the owner's sample becomes
     `Exempelstad` in the field and in the expected text
     (`"City: Exempelstad, Country Name: Sverige"`).
  3. `library.rs`, test `folders_with_the_same_name_get_as_many_parents_as_tell_them_apart`: the
     three paths (`C:\Foto\…` and the two `D:\Data\…`) become `C:\Photos\import\2025\2025-07-25`, `D:\Data\Trip\1\2025\2025-07-25` and
     `D:\Data\Trip\1\2\2025\2025-07-25`, and the first expected name
     `2025-07-25 (import\2025)` (the other two are unchanged).
  4. `library_text.rs`: in the doc comment of `folder_list_name_text` and in the test
     `a_folder_in_a_list_is_named_with_its_parents`, the owner's folder name before `\2025` becomes `import\2025`.
  5. `level_c.rs`: the two asserts that compare a City with the literal city of the owner's sample (in
     `the_two_jpegs_read_iim_as_utf8_in_agreement_with_xmp`, once per JPEG in its loop, and in
     `pm_jpg_reads_the_long_description_location_model_age_and_plus`) compare instead with what ExifTool reads from the same file, through
     the file's existing `tag` helper, as the same tests already do for `dc:rights` and
     `dc:description`: `photoshop:City` with `XMP-photoshop:City`, and the Location Created's
     `Iptc4xmpExt:City` with `XMP-iptcExt:LocationCreatedCity`. Each assert also checks that the
     text ExifTool read is neither empty nor `-` (what `tag`'s `-f` gives for a missing tag), so
     the test cannot pass on a missing value. The Location Created comparison stays after the
     existing `assert_eq!(locations.len(), 1, …)` and relies on it (with more than one location
     ExifTool's flattened `LocationCreatedCity` would be a joined list). No other literal from
     the owner's files is added.
  6. After the change, a `git grep` in `crates` for the serial number, the city and the two folder
     names (the reviewer's list, run in Git Bash, since Windows PowerShell 5.1 may mangle non-ASCII
     letters) finds nothing.
- **Tests to add:** none; the five changed tests keep what they check.
- **Not in this task:** the `TN` file-name prefix in tests and test filters (the owner keeps it,
  2026-11-19); the documents (done by the reviewer); `docs/test-inventory.md` (no count changes).
- **Rules:** `AGENTS.md` §2 ("No personal data in files") and §3; `docs/tasks/README.md` rules
  1–12. The `level_c.rs` tests are ignored without the owner's samples: run them with
  `PW_TESTDATA=D:\PhotoWorkflowData\RAW` and `.\tools\check.ps1 -Ignored`, and report their result.
- **Done when:** `.\tools\check.ps1 -Ignored` (with `PW_TESTDATA` set) is all OK, point 6 finds
  nothing, and `git diff --stat` shows only the five files above and this task's file.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Open | Added (public-release plan R7). Cold read: doable with the five files, every name and literal matches the code, `docs/test-inventory.md` needs no change. Changed after it: point 5 also refuses `-` (a missing tag) and says the Location Created comparison relies on the one-location assert; point 3 names the old paths; point 6 says Git Bash; the expected text in point 2 on one line; "Done when" includes this task's file |
