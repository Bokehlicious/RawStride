# T223. A Recycle Bin test that does not depend on the working folder's drive — Open

- **Why:** the first push run of CI (run 44684074582, `main` at 1284f2c, 2026-11-20; the run on
  `feature/gui-workspace` at the same commit failed too) failed on one test:
  `recycle::tests::windows::a_relative_path_to_a_missing_file_is_named_as_given` in
  `crates/pw-metadata/src/recycle.rs`. It expects `DeleteRefusal::Missing` and got
  `RecycleBinUnknown("The system cannot find the file specified. (os error 2)")`.
  The test passes a relative path, `pw-metadata-recycle-no-such-file.ARW`, which resolves against
  the test's working folder (the crate folder). `imp::inspect` judges the folder's drive
  (`drive_of`) before it looks at each file, and `read_settings` reads `NukeOnDelete` and
  `MaxCapacity` under the user's `Explorer\BitBucket\Volume\{GUID}` key. On GitHub's Windows
  runner the checkout is on `D:`, and that key did not exist for `D:`, so the drive was refused
  as "settings unreadable" (ADR 0018 §2, B-Q14) before the file was looked at. The same test
  passed in the green run at 659712e, so whether the key exists differs between runners. The
  code does what ADR 0018 says; the test asks for more than it is about. Every other test in
  the module uses `TempFolder` under `std::env::temp_dir()`, which is on `C:` and passed.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/recycle.rs` (the test only).
- **What:**
  1. In `a_relative_path_to_a_missing_file_is_named_as_given`, keep the relative path and the
     `check_group` call. The point of the test is that the error names the file as it was given
     (relative, not made absolute or resolved). Assert that:
     - `error.file` equals the relative path as given;
     - `error.left` and `error.deleted` are empty;
     - `error.reason` is `DeleteRefusal::Missing`, or one of the refusals of the drive check
       that depend on the machine: `RecycleBinUnknown(_)`, `NoRecycleBin`, `Card` or
       `RecycleBinOff`. Use `matches!` with a message that prints the error. `TooLarge` cannot
       happen (`Missing` returns before the size is compared with the capacity) and is not
       accepted. `Failed(_)` (from `std::path::absolute`, `canonicalize` of the working folder,
       `volume_root` or `wide`) is not accepted either, on purpose: it would mean an odd
       working folder, and the test should then fail loudly.
  2. A comment above the test, one or two lines: the path resolves against the working folder,
     whose drive is judged first, so the reason depends on the machine (CI's `D:` had no Recycle
     Bin settings, 2026-11-20); the file's name is what is tested.
  3. No change to any other test and no change outside the test module.
- **Tests to add:** none; the number of tests stays the same, so `docs/test-inventory.md` is not
  changed.
- **Not in this task:** the product's behaviour on a fixed drive whose `BitBucket\Volume\{GUID}`
  key does not exist. Today such a drive refuses every delete as "settings unreadable". When
  Windows creates the key is not known (the runner's `D:` had none), so a user's second drive may
  lack it too. Whether a missing key should count as Windows' defaults (bin on,
  default size) is the owner's decision for ADR 0018 (B-Q14), and the reviewer puts it in
  `backlog.md` with this task's Done row. Also not here: changing the working folder in tests
  (tests run in parallel), the CI workflow, or `tools/check.ps1`.
- **Rules:** `AGENTS.md`; tests only, no change to `imp` or the public functions.
- **Done when:** `.\tools\check.ps1` is all OK (1962 passed, 72 ignored, as before);
  `git diff --stat` shows only `recycle.rs` and this file (its status and one Log row). The test
  is checked by reading for both outcomes: `Missing` on a drive with readable settings (the
  owner's machine) and `RecycleBinUnknown` on a drive without them. The next CI push run is the
  test on GitHub, and the reviewer notes its result in this log.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added after CI run 44684074582 failed on this test (`test-output.txt` from the run's artifact). Cold read (fresh agent, 2026-11-20): can be done with the named file; variant and field names, `matches!` with the module's imports and the order of checks in `imp::inspect` match the code; `docs/test-inventory.md` line 21 ("a relative path to a missing file named as given") stays true. Fixed in the text: `Failed(_)` named as not accepted on purpose, and the reason `TooLarge` cannot happen made exact |
