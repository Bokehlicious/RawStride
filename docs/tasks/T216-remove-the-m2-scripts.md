# T216. Remove the one-off `tools/m2-*.cmd` scripts — Open

- **Why:** step R5 of `docs/tasks/plans/public-release.md` (owner, 2026-11-19, PR-Q5: remove
  them). The five double-click scripts ran the M2 LibRaw measurements of 2026-11-04 and write their
  logs to the owner's data folder; their results are in `docs/analyses/2026-11-04-m2-libraw-decode.md`
  and `docs/libraw-build.md`. The PowerShell scripts they start, in `tools/raw-decode-bench/`,
  stay. One thing lives only in a `.cmd` file and must not be lost: the `vcpkg install` command
  that makes the OpenMP measurement folder (`tools/m2-openmp.cmd` line 11).
- **Files:**
  - deleted: `tools/m2-crops.cmd`, `tools/m2-measure.cmd`, `tools/m2-openmp.cmd`,
    `tools/m2-tiles.cmd`, `tools/m2-verify.cmd`
  - `tools/raw-decode-bench/README.md` (the table under "| Script (double-click) | What |",
    about lines 39–42)
  - `tools/raw-decode-bench/run-openmp.ps1` (comment line 2), `tools/raw-decode-bench/run-verify.ps1`
    (comment line 3)
  - `docs/analyses/2026-11-04-m2-libraw-decode.md` (one note only, point 4)
- **What:**
  1. `git rm` the five `.cmd` files.
  2. `tools/raw-decode-bench/README.md`: the table of `.cmd` scripts becomes a table of the
     PowerShell scripts that are in the folder, each run from the repository root as
     `powershell -NoProfile -ExecutionPolicy Bypass -File tools\raw-decode-bench\<script>.ps1`:
     `run.ps1` (the M2 measurement that `m2-measure.cmd` ran; it needs the normal
     `vcpkg install --triplet x64-windows-static-md` of `docs/building.md` first, say so),
     `run-tiles.ps1`, `run-openmp.ps1`, `run-verify.ps1`, `run-crops.ps1`, one short "What" each,
     taken from the two old rows (tiles, openmp) and, for the other three, from the scripts' own
     first comment lines. Say also that the scripts write their results under the owner's data
     folder (as their own comments say), not into the repository. Below the table, the OpenMP
     prerequisite as a code block, copied from `tools/m2-openmp.cmd` line 11 without the log
     redirection:
     `& "$env:VCPKG_ROOT\vcpkg.exe" install --triplet x64-windows-static-md --x-manifest-root=tools\raw-decode-bench\openmp --x-install-root=vcpkg_installed_openmp`
     with one sentence that `run-openmp.ps1`, `run-verify.ps1` and `run-crops.ps1` need it, and
     that `docs/libraw-build.md` says how to undo it. Keep the README's other text.
  3. `run-openmp.ps1` line 2 and `run-verify.ps1` line 3: "(done by tools\m2-openmp.cmd)" /
     "(tools\m2-openmp.cmd)" → a pointer to `tools\raw-decode-bench\README.md`. Only those
     comments change.
  4. `docs/analyses/2026-11-04-m2-libraw-decode.md` is a record of its day and keeps its text;
     add one line after the header bullets (after the "Licence" bullet, about line 12):
     `- **Note (2026-11-19, T216):** the `tools\m2-*.cmd` scripts named below were removed; `tools\raw-decode-bench\README.md` shows how to run the same PowerShell scripts.`
  5. `docs/tasks/done/` is not changed (T208 names the scripts as they were then).
- **Tests to add:** none (no code changes).
- **Not in this task:** the PowerShell scripts' own behaviour and their output folder in the
  owner's data folder (plan step R4 is about the owner's tools); `docs/libraw-build.md`.
- **Rules:** no other file changes; if one must, stop and explain (rule 2). After the change,
  `git grep -n -E "m2-(crops|measure|openmp|tiles|verify)\.cmd"` finds the scripts only in
  `docs/tasks/` (this task, `done/`) and in `docs/analyses/2026-11-04-m2-libraw-decode.md`: its
  five existing lines (about 93, 171, 238, 258, 265, kept as written) and the note of point 4.
  Other `m2-*.cmd` names in `docs/analyses/` and `docs/distribution.md` are older scripts, removed
  earlier, and not this task's.
- **Done when:** `.\tools\check.ps1` is all OK; `git status --short` shows the five deletions,
  the four files above and this file's status and log; the `git grep` of "Rules" is pasted in the
  report.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-19 | Blocked | Added (plan step R5, owner 2026-11-19, PR-Q5). Waits for the cold read before Open |
| 2026-11-19 | Blocked | Cold read (fresh agent, `AGENTS.md`, the README, this file and the named files): doable; every `run-*.ps1` finds the repository root itself; nothing outside the named files depends on the `.cmd` files. Changed after it: the `git grep` of "Rules" narrowed to the five names and the analysis' five existing lines named as expected; `run.ps1`'s prerequisite (`vcpkg install`) said; the source of each "What" said; the scripts' output folder mentioned |
| 2026-11-19 | Open | Cold read done |
