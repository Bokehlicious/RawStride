# T310. ADR 0031 RL10: 0.x versions as regular releases in the release workflow (CI, docs) — Done

- **Why:** ADR 0031 RL-Q14 (owner, 2026-11-24, amends RL-Q2 and §3.2): from `0.6.0` on, a `0.x`
  version is published as a regular release, not a pre-release, so the newest version is GitHub's
  "Latest" and `.../releases/latest` finds it. Today the release workflow marks every `0.x`
  draft as a pre-release, and `docs/distribution.md` says so.
- **What the reviewer checked (2026-11-24):**
  - `.github/workflows/release.yml`, step "Create the draft release": the comment
    `# A 0.x version is a pre-release (ADR 0031 section 3.2).` and the `if` block after it, which
    adds `'--prerelease'` to `$arguments` when `$version` starts with `0.`. Nothing else in the
    file speaks of pre-releases.
  - `docs/distribution.md`, "Making a release", point 4: "creates a **draft** GitHub Release (a
    pre-release while the version starts with `0.`)". Nothing else in the file speaks of
    pre-releases. Point 5 says only "The owner reads the draft and publishes it by hand."
  - `gh release create` without `--prerelease` and without `--latest` makes a regular draft;
    GitHub's "Publish release" page then has "Set as the latest release" ticked by default.
  - `README.md` (status line, install steps) is changed by the reviewer together with the change
    of `v0.5.0` on GitHub (RL10), not by this task.
- **Crates:** none. **Files:** `.github/workflows/release.yml` (point 1), `docs/distribution.md`
  (point 2), and this task's own file (status and log).
- **What:**
  1. **`release.yml`:** remove the comment and the `if` block that add `--prerelease`, so
     `gh release create` gets the same arguments for every version. In their place one comment
     line: `# Every version, 0.x included, is a regular release (ADR 0031 RL-Q14).` Nothing else
     in the step or the file changes.
  2. **`docs/distribution.md`, "Making a release":** in point 4, the words "(a pre-release while
     the version starts with `0.`)" become "(a regular release, also for `0.x`, ADR 0031
     RL-Q14)"; every other word of the point stays, and its lines are wrapped again at about 100
     characters. Point 5 becomes: `The owner reads the draft and publishes it by hand, with "Set as
     the latest release" ticked (the default).`
- **Tests to add:** none; the workflow creates a release only on a pushed tag, and its test run
  (Run workflow) makes no release. The reviewer checks the next release's draft.
- **Not in this task:** `README.md` (line 6, "latest pre-release", and line 82, "a first
  pre-release": the reviewer's); `tools/package-msi.ps1`'s "pre-release suffix" (a version's
  syntax, not the release type); the release `v0.5.0` on GitHub and the older releases
  (the owner's and the reviewer's part of RL10); `CHANGELOG.md` (no line: CI and documents only);
  any other change to the workflow.
- **Rules:** `docs/tasks/README.md`, "How to work on a task"; tooling changes go through this task.
- **Done when:** `.\tools\check.ps1` is all OK (run when no other task's check is running) and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Open | Added (ADR 0031 RL10, RL-Q14). Cold read: doable with the named files; point 2 now names the exact words replaced and the re-wrap, and the "Not in this task" list names `README.md`'s two lines and `package-msi.ps1`'s unrelated "pre-release suffix" |
| 2026-11-24 | Ready for review | `release.yml`: the `--prerelease` comment and `if` block replaced by the one comment line; `docs/distribution.md` points 4 and 5 changed as written. `.\tools\check.ps1` all OK; no tests added (none asked) |
| 2026-11-24 | Done | Review: the diff is exactly points 1 and 2, only the named files; `test-output.txt` newer than the change, all OK, no clippy warnings of the workspace's own. No `CHANGELOG.md` line (CI and documents only). Leftovers: `README.md`'s lines 6 and 82 and `v0.5.0` on GitHub are RL10's reviewer part (its row in `plans/adr-0018-0025-steps.md`); the next release's draft shows the workflow change (the same row) |
