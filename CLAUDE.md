# Claude Code Instructions

`AGENTS.md` holds the project's rules and comes first; this file adds only what is specific to
Claude Code sessions. Do not repeat or reinterpret `AGENTS.md` here.

## 1. Before You Change Anything
1. Check the working directory, worktree and branch. The work happens in
   `C:\Development\PhotoWorkflow-GUI` on `feature/gui-workspace`; `C:\Development\PhotoWorkflow` is
   the `main` worktree and is not edited.
2. Run `git status` and `git diff --staged`. Uncommitted, untracked and staged changes are the
   owner's unless the owner says otherwise (`AGENTS.md` §2).
3. Read the code, tests and documents related to the task. For an agent task, read
   `docs/tasks/README.md` and the task's own file under `docs/tasks/`.

## 2. Scope
- Change only what the task or the owner's request covers, with the smallest change that does it.
  No unrelated refactoring.
- Ask before changing a file the owner has not asked about.

## 3. Shell Scripts
- An edit or helper script longer than a few lines is written with the Write tool to the session
  scratchpad and run as `python <path>`; never sent inline as a heredoc or `python -c`, which
  breaks on quoting in this environment.
- A handful of exact replacements in one file is done with the Edit tool, not a script.

## 4. Completion Report
- Every file modified or created, what changed in each and why.
- The tests actually run, with the summary from `test-output.txt` (`.\tools\check.ps1`), or a
  statement that no tests were run and why.
- Remaining risks, known limitations, and any `cargo clippy` warnings.
