# T208. Remove the scripts in `tools/` that build or run the Tauri app — Open

- **Why:** after T207 the Tauri app (`photoworkflow`) is gone, so the scripts that build, package,
  measure or verify it no longer work. Second task of the chain T207 → T208 → T209 (owner's
  decision 2026-11-19 to retire the Tauri app; see T207).
- **Crates:** none. **Files:**
  - removed: `tools/package-dist.ps1`, `tools/verify-dist.ps1`, `tools/bench-app.ps1`,
    `tools/measure-app.ps1`, `tools/build-m1-reference.cmd`, `tools/m2-errors.cmd`,
    `tools/m2-errors2.cmd`, `tools/m2-final.cmd`, `tools/m2-step1.cmd`, `tools/m2-step3.cmd`,
    `tools/m2-step3b.cmd`, `tools/m2-step4.cmd`, `tools/m2-step4b.cmd`, `tools/m2-step5.cmd`,
    `tools/m2-step5b.cmd` (15 scripts)
  - changed: `.gitignore`
- **What:**
  1. Before you remove each script, check with a search that it names `photoworkflow`,
     `package-dist`, `verify-dist`, `bench-app` or `measure-app` (each of the 15 did when this
     task was written). If one does not, keep it and say so in the report. Then remove the 15 with
     `git rm`.
  2. These stay: `tools/m2-step2.cmd` (it runs only the tests of `pw-rawview`, which stays; its
     fate follows the backlog item "`pw-rawview` and the direct ExifTool write calls after
     T207"), the five `m2-*.cmd` scripts that run only `raw-decode-bench` (`m2-crops`,
     `m2-measure`, `m2-openmp`, `m2-tiles`, `m2-verify`), and every other file in `tools/`.
     `tools/README.md` names none of the removed scripts and does not change.
  3. `.gitignore`: remove the line `/dist/` and its comment
     (`# Standalone distribution folder (tools/package-dist.ps1)`).
- **Tests to add:** none.
- **Not in this task:** `docs/libraw-build.md` and other documents that name `package-dist.ps1`
  (T209); a distribution folder for `pw-loupe` (ADR 0002 §4, its own task later).
- **Rules:** remove with `git rm`. No other file changes; if one must, stop and explain.
- **Done when:** `.\tools\check.ps1` is all OK, and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added: split from T207 by its cold read. `m2-step2.cmd` kept (it tests `pw-rawview`, not the Tauri app). Waits for T207 |
| 2026-11-19 | Open | T207 done; T208 can start |
