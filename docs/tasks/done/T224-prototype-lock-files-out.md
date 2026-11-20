# T224. The Tauri prototypes' lock files out of the repository — Done

- **Why:** the push of 2026-11-20 (eedbf4c) was answered by GitHub with five moderate Dependabot
  alerts on `main`. The alerts themselves were not read (the API needs a login). `cargo audit` on
  every tracked `Cargo.lock` (nine files; reviewer, 2026-11-20) finds no vulnerability anywhere,
  and one advisory in exactly five files: `glib` 0.18.5 is unsound, RUSTSEC-2024-0429 (fixed in
  0.20). It is in the lock files of the five Tauri prototypes: `prototypes/phase2/p2-webview2`,
  `prototypes/phase2/p3-wgpu`, `prototypes/phase3/p6-gpu-demosaic`, `prototypes/phase3/p6b-gpu-ppg`
  and `prototypes/phase3/p6c-gpu-memory`. `glib` comes in only through Tauri's `gtk` on Linux
  (`cargo tree -i glib` finds it for `x86_64-unknown-linux-gnu`, not for `x86_64-pc-windows-msvc`),
  and Tauri 2 still uses `gtk` 0.18, so no update removes it. The same five files also carry
  warnings for `chrono` 0.4.45 (RUSTSEC-2026-0543) and `proc-macro-error` (RUSTSEC-2024-0370), and
  `p3-wgpu` for `paste` (RUSTSEC-2024-0436); they go with the files, so whichever of these the five
  alerts are, they are in these files. The prototypes are history: not workspace members, not
  built, and their icon path broken since T207 (accepted there). The workspace's own `Cargo.lock`
  has no `glib` (T207 removed it). Without the lock files GitHub's dependency graph sees only the
  prototypes' direct dependencies in `Cargo.toml`, where none of these crates is named, so the
  alerts are expected to close as fixed after the push (to be seen on GitHub, see Done when).
  T137 left the prototypes' lock files out of `cargo audit` on purpose; that stays.
- **Crates:** none. **Files:** the five lock files, removed:
  - `prototypes/phase2/p2-webview2/Cargo.lock`
  - `prototypes/phase2/p3-wgpu/Cargo.lock`
  - `prototypes/phase3/p6-gpu-demosaic/Cargo.lock`
  - `prototypes/phase3/p6b-gpu-ppg/Cargo.lock`
  - `prototypes/phase3/p6c-gpu-memory/Cargo.lock`
- **What:**
  1. `git rm` the five files above; the deletions are staged on purpose (`AGENTS.md` §2: this is
     the task's staging, not the owner's). Nothing else under `prototypes/` changes: the
     `Cargo.toml` files, the READMEs, the scripts and the other two lock files stay
     (`prototypes/phase1/Cargo.lock` reports nothing; `prototypes/gui/p7-egui-presenter/Cargo.lock`
     only the same three "unmaintained" warnings as the root, `zbus_names`, `zvariant`,
     `zvariant_derive`).
  2. **Known and accepted break in the scripts:** each prototype's `run-*.ps1` and `build-*.cmd`
     copies a lock file in when none exists. `p2`, `p3`, `p6` and `p6b` copy the root
     `Cargo.lock`, which has had no Tauri since T207, so Cargo would resolve Tauri afresh.
     `p6c` copies `p6b`'s lock file, which this task removes: `run-p6c.ps1` line 25
     (`Copy-Item ... -ErrorAction Stop`) then stops at once, and `build-p6c.cmd` line 7 fails
     silently and resolves afresh. The scripts are not changed, as with the icon path in T207;
     the reviewer gives this a home in `backlog.md` with the Done row.
  3. Check, before and after, with `cargo audit -n -f <file>` on every tracked `Cargo.lock`
     (`git ls-files '*Cargo.lock'`): before, the five files each report RUSTSEC-2024-0429; after,
     four files are left, none reports it, and none reports a vulnerability. The report gives both
     lists.
- **Not in this task:** deleting or archiving the prototypes themselves; changing their scripts
  (point 2); updating their dependencies (no fixed `glib` is reachable through Tauri 2); a
  `.gitignore` rule for lock files a later build of a prototype would create (`git status` shows
  them); `.github/dependabot.yml` or GitHub's alert settings; the "unmaintained" warnings in the
  workspace's `Cargo.lock` (backlog item "`cargo audit` warns of unmaintained crates");
  `tools/check.ps1` and `.github/workflows/ci.yml` (neither reads these files).
- **Rules:** `AGENTS.md`. No `cargo update`, no build or run script in `prototypes/` (it would
  write a new lock file).
- **Done when:** `git diff --staged --stat` shows exactly the five deletions; `git status` shows
  besides them only this file (its status and one Log row, on top of the reviewer's uncommitted
  changes from before the task started); `.\tools\check.ps1` is all OK (1962 passed, 72 ignored,
  as before; the workspace does not read these files). The agent's part ends at Ready for review.
  After the reviewer's commit and the push, the owner checks on GitHub (Security → Dependabot)
  that the five alerts are closed as fixed, and the reviewer notes it in this log; an alert still
  open after that is a new log row and a finding, not part of this task.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added after five Dependabot alerts on `main` (push of eedbf4c); the match to RUSTSEC-2024-0429 in five lock files is the reviewer's `cargo audit` of every tracked lock file. Cold read (fresh agent, 2026-11-20): `glib` and the advisory confirmed in exactly the five files, none in the other four; `check.ps1` and `ci.yml` read only the root lock file; `docs/test-inventory.md` names none. Fixed in the text: the scripts that seed a lock file, `p6c`'s from `p6b`'s, named as a known break (point 2) with a home in `backlog.md` at Done; phase1 and p7's warnings stated exactly; the other warnings that go with the five files named; the staging of the deletions said to be intended; Done when by `git diff --staged` and `git status`, with the GitHub check after the push as the reviewer's and owner's |
| 2026-11-20 | Ready for review | The five lock files removed with `git rm` (staged; nothing else under `prototypes/` changed, no script run). `cargo audit -n -f` on every tracked `Cargo.lock`, before (nine files): RUSTSEC-2024-0429 in exactly `p2-webview2`, `p3-wgpu`, `p6-gpu-demosaic`, `p6b-gpu-ppg`, `p6c-gpu-memory` (each also RUSTSEC-2024-0370 and RUSTSEC-2026-0543, `p3-wgpu` also RUSTSEC-2024-0436); the root and `p7-egui-presenter` RUSTSEC-2026-0370/0371/0372 (unmaintained); `phase1` and `tools/egui-demo` nothing; no vulnerability in any. After (four files: root, `p7-egui-presenter`, `phase1`, `tools/egui-demo`): unchanged, none reports RUSTSEC-2024-0429, none a vulnerability. `check.ps1` all OK: 1962 passed, 0 failed, 72 ignored; clippy no warnings besides the LibRaw build note |
| 2026-11-20 | Done | Reviewed (reviewer): exactly the five deletions staged, nothing else under `prototypes/`; the four lock files left audited again by the reviewer, none reports RUSTSEC-2024-0429 or a vulnerability; `check.ps1` all OK: 1962 passed, 72 ignored, no clippy warnings (the `^warning` grep finds only cargo audit's allowed advisories and `raw-decode-bench`). The alerts read with `gh` (reviewer, 2026-11-20): open alerts 1–5 are all `glib`, medium, GHSA-wrw7-89jp-8q8g (RUSTSEC-2024-0429), one in each of the five removed lock files, so the match is confirmed. Not checked yet: the alerts closed after the push; the next Log row. Leftover: the scripts that seed a lock file (point 2), in `backlog.md` as "Leftover of T224" |
