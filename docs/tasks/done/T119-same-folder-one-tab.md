# T119. The same folder opens in one tab, however its path is written — Done

- **Starts after T116 is Done** (both change `crates/pw-loupe/src/tabs.rs`).
- **Why:** `Tabs::open_new` finds an open folder by comparing paths exactly, so `C:\Foto` and
  `c:\foto\` (from a drop, the folder dialog, an import or the command line) open two tabs for one
  folder. GR12 says an already open folder is activated instead (`docs/tasks/backlog.md`,
  "Same folder, two tabs").
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/tabs.rs`, `docs/test-inventory.md`. (The
  reviewer removes the backlog entry when this task is Done.)
- **What:**
  1. In `tabs.rs`, a private function `same_folder(a: &Path, b: &Path) -> bool` compares the
     **resolved** forms: each path is made absolute (`std::path::absolute`), then resolved with
     `pw_ingest::resolve` (canonical, so case, trailing separators, `\\?\` and `subst` drives do
     not matter). If either step fails for a path, that path is used as it is. Equal resolved
     forms are the same folder.
  2. `open_new` uses `same_folder` to find an open tab instead of `==`. When it finds one, it
     activates and refreshes that tab as today; the folder stored in the tab is the path just
     given, as today.
  3. The path stored in a tab, shown in its title and passed on to the rest of the program does
     **not** change: only the comparison uses the resolved form (a `\\?\` path must not reach
     ExifTool, LibRaw or the window).
  4. Nothing else changes. `path_starts_with` (used when a card is ejected, around line 156) also
     compares paths exactly; do not change it, but say in the report whether the same problem
     can happen there.
- **Tests to add** (in `tabs.rs`'s tests, with a temporary folder made by the test):
  - the folder opened as `<tmp>\Foto` and then as the same path in other letter case → one tab,
    the first tab active;
  - the same with a trailing `\` → one tab;
  - a relative path to the same folder (made from the current directory only if that is safe in
    the test; otherwise skip this case and say why) → one tab;
  - two different folders → two tabs (today's behaviour);
  - a folder that does not exist compared with itself → one tab (the fallback of point 1);
  - after the second open the tab's `folder` is the path given the second time (point 2).
- **Not in this task:** `path_starts_with`; changing paths elsewhere in `pw-loupe`; the Tauri app.
- **Rules:** `docs/tasks/README.md` rules 1–13 (no user-visible text is added).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-13 | Open | Added from the backlog ("Same folder, two tabs"); starts after T116 |
| 2026-11-14 | Ready for review | `same_folder` in `tabs.rs`, used by `open_new`; five tests (the case test on Windows only); the relative case uses the crate's `src` folder (no change of the current directory); `check.ps1` all OK, 1223 passed, 43 ignored |
| 2026-11-14 | Ready for review | Reviewer: code accepted, not committed. `same_folder` compares `std::path::absolute` + `pw_ingest::resolve` forms and falls back to the path as given; the tab keeps the path given (point 3); the five tests cover the task's cases. Point 4 was not answered in the report; the reviewer's answer: `path_starts_with` compares components exactly, but std upper-cases the drive letter and the card root is a drive, so a folder on the card is still found; a different letter case in a folder name below the root would not be (low risk, not changed). The reviewer's `check.ps1` ran while T121's agent was working: no test failed, but `cargo fmt` stopped on T121's unfinished `info_panel.rs` and the two runs wrote into the same `test-output.txt`; a clean run is needed after T121 |
| 2026-11-14 | Ready for review | Reviewer: clean `check.ps1` after T121 (T119 and T121 together): fmt, test and clippy OK, 1215 passed, 0 failed, 43 ignored. Waits for the owner's approval |
| 2026-11-14 | Done | Owner: hand check done and approved; committed by the reviewer |
