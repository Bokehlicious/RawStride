# Claude Code Instructions

`AGENTS.md` holds the project's rules and comes first; this file adds only what is specific to
Claude Code sessions. Do not repeat or reinterpret `AGENTS.md` here.

The paths, branches and roles below are the maintainer's own setup. A session for an outside
contributor follows `CONTRIBUTING.md` and `AGENTS.md` §2's first point instead of §1 here.

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

## 4. Large Files
- A document over about 20 kB is not read whole. Find the part needed first (`grep -n '^## '` for
  the headings, or a search for the term) and read only that range with offset and limit.
- `docs/adr/0007-metadata-iptc.md` (~80 kB) is read one section at a time.
- `docs/test-inventory.md` (~145 kB, lines over 1,000 characters) is never read whole: only the
  matching lines are searched, read with `cut -c` or an awk `substr` range, and changed by
  script (`AGENTS.md` and §3 apply).
- `test-output.txt` and other build or test logs are read through a search (`^test result:`,
  `error`, `warning`), not whole.

## 5. Completion Report
- Every file modified or created, what changed in each and why.
- The tests actually run, with the summary from `test-output.txt` (`.\tools\check.ps1`), or a
  statement that no tests were run and why.
- Remaining risks, known limitations, and any `cargo clippy` warnings.
