# T122. A free-space test that does not fail by chance — Done

- **Why:** `fileops::tests::can_write_refuses_when_the_extra_bytes_exceed_the_free_space` failed
  once during T108 (log 2026-11-12) and passed in the next run. The test reads the free space,
  then calls `can_write(&file, free + 1)`, which reads the free space a second time. If another
  program frees more than 111 bytes between the two readings (a temporary file deleted, a cache
  trimmed), `needed` (110 + `free` + 1) is no longer above the new free space and `can_write`
  returns `Ok(())` instead of `NoSpace`. The test depends on the disk standing still; the code
  under test is right.
- **Crates:** `pw-metadata`; tests only. **Files:** `crates/pw-metadata/src/fileops.rs`;
  `docs/test-inventory.md` only if the count or description changes.
- **What:**
  1. Change the test so that no change of the free space that can happen during a test run
     makes it pass or fail: ask for extra bytes far above the free space, with a margin no disk
     frees in a moment (for example `free + 1 TiB`, using `saturating_add`), not `free + 1`.
  2. Keep what the test checks today: the result is `Refusal::NoSpace`, `needed` is exactly
     100 bytes + 10 % + the extra bytes (`110 + extra`), and the reported free space is below
     `needed`. Keep the test's name, or rename it only if the name no longer says what it checks
     (say so in the report).
  3. Do not use `u64::MAX` as the extra bytes in this test: `needed` would then saturate and the
     test would no longer check the sum.
  4. Look through the other tests in `fileops.rs` and `safe_write.rs` (and `pw-metadata`'s
     `tests/`) for the same pattern: a value read from the disk, then compared with a second
     reading inside the code under test. List any you find in the report; change only the test of
     point 1.
- **Tests to add:** none; one test changes (README rule 5 is lifted for this test only, by the
  owner, 2026-11-14). The number of tests stays the same.
- **Not in this task:** any change to `can_write`, `free_space` or `safe_write.rs`'s own space
  check; a way to inject the free space into `can_write`.
- **Rules:** `docs/tasks/README.md` rules 1–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Added by the owner, from the flaky failure noted on T108 (2026-11-12) |
| 2026-11-14 | Ready for review | Test asks for free space + 1 TiB (`saturating_add`); same name and checks. No other test with the pattern. `check.ps1` all OK |
| 2026-11-14 | Ready for review | Reviewer: accepted, not committed. Only the test changed: extra bytes = free space + 1 TiB (`saturating_add`), `needed == 110 + extra` and `reported < needed` kept, not `u64::MAX`; name unchanged. Point 4 confirmed: the other disk readings in `pw-metadata`'s tests are modification times of files only the test touches. The reviewer's `check.ps1`: fmt, test and clippy OK, 1220 passed, 0 failed, 43 ignored. Waits for the owner's approval |
| 2026-11-14 | Done | Owner: approved; committed by the reviewer |
