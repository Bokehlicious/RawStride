# T217. A made-up name in `pw-metadata`'s test data — Open

- **Why:** step R6 of `docs/tasks/plans/public-release.md` (owner, 2026-11-19, PR-Q4: replace
  it). Tests in `pw-metadata` use the owner's real name as the creator, copyright and credit of a
  metadata template. Before the repository is public it is replaced with a made-up name that,
  like the old one, holds non-ASCII letters, so the tests keep checking text outside ASCII.
- **Crates:** `pw-metadata` (tests only). **Files:**
  - `crates/pw-metadata/src/lib.rs` (its unit tests, about lines 840–910)
  - `crates/pw-metadata/src/template.rs` (its unit tests, about lines 957 and 1045–1075)
  - `crates/pw-metadata/tests/template.rs` (about lines 265–345 and 495–530)
- **What:** in exactly these three files, and only in test code:
  1. The owner's full name (first and last name, as written today in these files) → `Anna
     Exempelström`, everywhere it occurs, also inside `© 2026 …` and with the surrounding spaces
     kept where a test has them (`" … "` in `lib.rs` tests the trimming).
  2. The credit `"<last name> Foto"` in `tests/template.rs` → `"Exempelström Foto"`.
  3. The first name alone in `template.rs` (`a_field_of_a_list_of_structures_allows_only_keep`,
     `text("…")`, about line 957) → `"Anna"`.
  4. Nothing else changes: no test is added, removed or renamed, no assertion is changed except
     the name inside it. The new names are a few bytes longer in UTF-8 (the credit four); if a test
     fails because of a length (for example an IIM field limit) rather than the name, stop and explain.
- **Tests to add:** none; the same tests pass with the new name. `tests/template.rs` has no
  ignored tests (it needs ExifTool and Windows, and runs in `check.ps1`), so `check.ps1` covers
  all three files; paste the `template` binary's `test result:` line from `test-output.txt`.
- **Not in this task:** `Cargo.toml` `authors` (the owner is the author; kept); `docs/`.
- **Rules:** no other file changes; if one must, stop and explain (rule 2). After the change, a
  case-insensitive `git grep` for the owner's last name, written without its `ä` as
  `Nyk.nen`, finds nothing under `crates/` (paste the command and its empty result).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the three files above
  and this file's status and log; the `template` result line and the `git grep` are pasted in the
  report.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (plan step R6, owner 2026-11-19, PR-Q4). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README, this file and the three files): every occurrence (lib.rs 8, src/template.rs 7, tests/template.rs 11) is in test code and covered by points 1–3; no test seen depends on the length. Changed after it: `tests/template.rs` has no ignored tests and needs no `PW_TESTDATA`, so the separate run was replaced by its line in `test-output.txt`; the byte difference worded as "a few bytes" |
| 2026-11-19 | Open | Cold read done |
