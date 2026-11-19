# T215. A personal-data step in `check.ps1` — Open

- **Why:** step R11 of `docs/tasks/plans/public-release.md` (owner, 2026-11-19). Step R3 removed
  the only two paths with the owner's Windows user name from the documents; both had been pasted
  in by agents (a session's scratchpad path, a `.cargo` path). So that such paths and real e-mail
  addresses do not come back, `check.ps1`, which every task runs before review and CI will run
  later, gets a step that finds them. The step must not name the owner: it looks for the *shape*
  of a user-folder path and of an e-mail address, never for a particular name.
- **Files:** `tools/check.ps1`; `AGENTS.md` (only the parenthesis in §5's first bullet,
  "(fmt, cargo audit, tests, clippy; …)", about line 36); `README.md` (only the list in line 54,
  "(formatting, `cargo audit`, all tests, clippy without warnings)"); `docs/building.md` (only the
  sentence in lines 51–52, "It runs `cargo fmt --all --check`, `cargo audit`, …").
- **What:**
  1. In `check.ps1`, a new step named `personal data` (header `=== personal data ===`, as the
     other steps), run right after `cargo fmt --all --check` and before `cargo audit` (it is fast
     and needs no network). It is not a cargo command, so it does not use `Invoke-CheckStep`;
     write it in the style of the `cargo audit` "not installed" block (about lines 89–97): its
     own report lines, an `Exit code: <n>` line, a `$script:results` entry `OK` or `FAILED`, and
     `$script:hasFailures = $true` when it fails. A short comment above it says what it looks for
     and why (no name is listed; a placeholder such as `%USERPROFILE%` or `C:\Users\…` passes).
  2. It searches every tracked file **and every untracked file that is not ignored** (a task
     agent's new files are untracked; ignored folders such as `target/` are not searched, since
     `--untracked` keeps the standard excludes), as text, case-insensitive, with
     `git grep --untracked -n -I -i -E <pattern>` run from the repository root, once per pattern.
     Write each pattern in PowerShell single quotes and pass the arguments as an array (as
     `Invoke-CheckStep` does for cargo), so nothing in it is expanded:
     - **A user-folder path:** `[a-z]:[\\/]+users[\\/]+[a-z0-9._-]` (a drive, `Users`, then a
       name that starts with a letter, digit, `.`, `_` or `-`; so `C:\Users\…`,
       `C:\Users\<name>` and `%USERPROFILE%` pass) and the Git Bash form
       `/[a-z]/users/[a-z0-9._-]`. In POSIX ERE `[\\/]` is the set "backslash or slash".
     - **An e-mail address:** `[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}`, after which lines whose
       every match is at an allowed domain are dropped. Allowed domains (a list in the script,
       with a comment): `example.com`, `example.org`, `example.net`, `b.se` (test data in
       `pw-metadata`) and `users.noreply.github.com`; an address matches a domain when it ends in
       `@` + that domain exactly (so `b.example.se` is not `b.se`). Do the filtering in PowerShell
       on `git grep -o`'s output or with a second match per line; how is up to you, as long as a
       line with one allowed and one other address still fails.
  3. Every hit is written to the report as `git grep` prints it (`path:line:text`, the text cut to
     about 200 characters), and any hit fails the step. `git grep`'s exit code 1 means "no match"
     and is not a failure; any other non-zero code (an error) fails the step with its message.
  4. `AGENTS.md` §5, `README.md` line 54 and `docs/building.md` lines 51–52: add the new step to
     the lists of what `check.ps1` runs (personal data, after fmt / formatting), nothing else in
     those files.
- **Tests to add:** none in Rust. The step is shown working by hand, each time with the whole
  `.\tools\check.ps1` (or, to save time, the step's own lines pasted into a PowerShell prompt at
  the repository root; say which), and the results pasted in the report:
  1. On the tree as it is: `personal data: OK`.
  2. **A probe file that must fail.** Write the probe lines from PowerShell, never into a file
     under `docs/` or into this task's text (they would make the check fail on that file): an
     untracked file `t215-probe.txt` in the repository root, made with
     `Set-Content -Encoding utf8 t215-probe.txt -Value @(('C:' + '\Us' + 'ers\someone\x'), ('/c/Us' + 'ers/someone/x'), ('a' + '@' + 'b.example.se'))`.
     The step reports all three lines and `personal data: FAILED`.
  3. **A probe file that must pass:** the same file rewritten with
     `Set-Content -Encoding utf8 t215-probe.txt -Value @('%USERPROFILE%\x', ('foto' + '@' + 'example.com'), ('x' + '@' + 'users.noreply.github.com'))`:
     no hit, `personal data: OK`.
  4. The file deleted, and `.\tools\check.ps1` run once more for the report: all OK.
- **Not in this task:** the commit history (plan steps H1 and H2); the owner's name as test data
  (R6); any other check; CI (C1).
- **Rules:** no new tool or dependency (only `git`; if `git grep --untracked` is not available,
  stop and ask). The patterns hold no real name or address. No other file changes; if one must,
  stop and explain (rule 2).
- **Done when:** `.\tools\check.ps1` is all OK with `personal data: OK` in its summary;
  `git diff --stat` shows only the four files above and this file's status and log; the four
  hand results above are in the report; `t215-probe.txt` is gone.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (plan step R11, owner 2026-11-19). The two patterns tried by the reviewer on the tree at `19632bb`: no user-folder path, and every e-mail address at `example.com`, `example.org` or `b.se`. Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README, this file and the named files; it ran the patterns): this file's own probe lines matched all three patterns, so result 1 would have failed. Changed after it: the probe lines are built in PowerShell from pieces and never written into a document; the domain match said exactly; single quotes and an argument array for the patterns; `--untracked` keeps the excludes said; how to run the step said; `docs/building.md` lines 51–52 added (it lists the steps too) |
| 2026-11-19 | Open | Cold read done |
