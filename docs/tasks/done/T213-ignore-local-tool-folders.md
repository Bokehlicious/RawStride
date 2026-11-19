# T213. Local tool folders ignored by `.gitignore` — Done

- **Why:** step R1 of `docs/tasks/plans/public-release.md` (accepted by the owner, 2026-11-19),
  backlog "Before the repository is made public", item 5. Two local folders are hidden today
  only on the owner's machine: `TokenEconomizer/` and `.claude/settings.local.json` are listed in
  the repository's local `.git/info/exclude`, not in `.gitignore`. In anyone else's clone a
  `git add -A` would take them into a commit.
- **Files:** `.gitignore` only.
- **What:**
  1. In `.gitignore`, a new section after "# Session outputs" (before "# Editors / OS"), with a
     one-line comment saying these are per-user tool files that never belong in the repository:
     - `/TokenEconomizer/`
     - `/.claude/settings.local.json`

     Only the local settings file, not the whole `.claude/` folder as the plan's row R1 first
     said: a shared `.claude/settings.json` may be committed later on purpose. The new section
     is set off by one blank line before and after, as the other sections are.
  2. Nothing else in `.gitignore` changes. `.git/info/exclude` is the owner's local file and is
     not touched.
- **Tests to add:** none (no code changes). The check in "Done when" shows the rules work.
- **Not in this task:** any other ignore rule; the `Claude outputs/` rule that is already there.
- **Rules:** no other file changes; if one must, stop and explain (rule 2).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only `.gitignore` and this
  file's status and log; `git check-ignore -v TokenEconomizer/x .claude/settings.local.json`
  names `.gitignore` and its new line numbers for both paths (a `.gitignore` rule wins over
  `.git/info/exclude`, so the output shows the new rules even though the exclude file still
  holds the same ones), pasted in the report.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (plan step R1). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README, this file and `.gitignore`): doable in `.gitignore` alone, the section names match. Changed after it: the narrowing from the plan's `.claude/` to the settings file said, the blank lines said; the `git status` check removed (the local exclude file already hides both, so it proved nothing) |
| 2026-11-19 | Open | Cold read done |
| 2026-11-19 | Ready for review | `.gitignore` lines 38–40: new section "Per-user tool files" with `/TokenEconomizer/` and `/.claude/settings.local.json`, after "Session outputs" and before "Editors / OS". `git check-ignore -v` names `.gitignore:39` and `.gitignore:40`. `check.ps1` all OK |
| 2026-11-19 | Done | Review: the diff is only `.gitignore` (four lines, blank lines as the other sections) and this file's status and log; `git check-ignore -v` checked again, names `.gitignore:39` and `:40`. `test-output.txt` newer than the change, fmt, audit, tests (1957 passed, 0 failed, 72 ignored) and clippy all exit 0, no clippy warnings. Nothing left; no hand check needed. `cargo audit` now warns `zbus_names`, `zvariant`, `zvariant_derive` unmaintained (RUSTSEC-2026-0370–0372) instead of `chrono`; not caused by this task, raised with the owner |
