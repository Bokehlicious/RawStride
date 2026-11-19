# Making the repository public: steps

The owner said on 2026-11-19 that the next milestone is probably making the repository
(`github.com/Bokehlicious/RawStride`, private today) public. This plan gathers what `backlog.md`
lists for it ("CI before the repository becomes public", "Review the git history before the
repository becomes public", "Before the repository is made public", the README note under "Linux
and macOS ports") and ADR 0002 §4 into one order. **Accepted by the owner on 2026-11-19:** every proposal in §1
(PR-Q1–PR-Q10) approved as written.

**Making the repository public is not a binary release.** A release of the program (an exe for
others to download) has its own list in ADR 0002 §4 and is §6 here, after this milestone.

Facts checked on 2026-11-19 (work branch at `f1d2444`): 573 commits on all branches, every one by
the owner's private e-mail address; branches `main` and `feature/gui-workspace`; no `.github/`
folder; `tests/fixtures/` holds only a README; `Cargo.toml` has `repository = ""`; `vcpkg.json`
still says `photoworkflow`; `src-tauri/` is gone (T207); `docs/agent-tasks.md` is already a short
stub pointing to `docs/tasks/` (the backlog's "about 700 KB" is old, the full file lives only in
the history). Some tens of commit hashes are quoted in the documents (most in `done/`, also
`docs/libraw-build.md`, `docs/references.md`, the `plans/`).

## 1. Owner decisions first

Each step below says which decision it waited for. The last column is the reviewer's proposal;
the owner approved all ten as written on 2026-11-19, so each is now the decision.

| Id | Question | Decision (owner, 2026-11-19: as proposed) |
|---|---|---|
| PR-Q1 | **Which branches are published?** Both, or only `main` (the work branch kept private, or the work moved to a public branch after the milestone)? | Both, as today: `main` is moved only by fast-forward, so the work branch holds nothing `main` will not hold. |
| PR-Q2 | **The owner's e-mail address in all 573 commits.** Keep it, or rewrite the history to the GitHub noreply address? | Rewrite (H2), since it is the only personal data found so far and cannot be taken back once public. If H1 finds other things to remove from the history, they go in the same rewrite. |
| PR-Q3 | **A trademark search for "RawStride"** (ADR 0012, ADR 0002 §4). Before the repository is public, or only before a binary release? | Before: a rename after the name is public (repository, README, texts in the program) costs more than now. The search is the owner's (or a lawyer's); the reviewer can list where to search. |
| PR-Q4 | **The owner's name as test data** in `pw-metadata` (`src/lib.rs`, `src/template.rs`, `tests/template.rs`). Keep, or replace with a made-up name? | Replace with a made-up name (a small task, R6). |
| PR-Q5 | **The one-off `tools/m2-*.cmd` scripts** (paths to the test-data folder). Keep, or remove? | Remove (their results are in `docs/analyses/`); keep only what `docs/building.md` or a test needs. |
| PR-Q6 | **The latency tools outside the repository** (`D:\PhotoWorkflowData\pm-study\tools\latency.ps1`, named by `docs/requirements-image-viewer.md`, ADR 0008, ADR 0015). Bring the RawStride-only scripts into `tools/`, or say in those documents that the tools are the owner's and not published? | Say so in the documents (R4); bringing them in is a later task if someone asks. |
| PR-Q7 | **`Signed-off-by`** — CONTRIBUTING asks contributors for it; the owner's own commits do not carry it. | Keep it for contributors and add one sentence that it applies from the day the repository is public. |
| PR-Q8 | **Extras in `.github/`**: issue templates, `SECURITY.md` (how to report a security problem privately). | `SECURITY.md` yes (GitHub's private vulnerability reporting); issue templates later. |
| PR-Q9 | **Free test files**: a few of the owner's own images (small RAW, DNG, JPEG, TIFF) released as CC0 into `tests/fixtures/`, so the ignored real-file tests can run for others and in CI. | After the repository is public (R9), not a blocker; the owner picks the images. |
| PR-Q10 | **`gitleaks` (or a similar scanner) for H1.** Run it once as an outside tool (not a dependency of the project), or not at all? | Once, outside the repository, on a mirror clone; it adds nothing to the project. |

## 2. Steps

| Order | Step | Who | Content | Can start |
|---|---|---|---|---|
| 1 | R1 | T213 | `.gitignore`: add `TokenEconomizer/` and `.claude/settings.local.json` (hidden before only on the owner's machine, by `.git/info/exclude`); done (T213, 2026-11-19) | now |
| 1 | R2 | T214 | `Cargo.toml` `repository` set to the GitHub address and inherited by the 17 members; `vcpkg.json` name and description to RawStride (a vcpkg dry run shows the manifest is read) | now |
| 1 | R3 | reviewer (docs only) | Paths with the owner's user name in the files: `done/T115-…` (scratchpad path) and `docs/analyses/2026-11-04-m2-pipeline-steps.md` line 502 (`C:\Users\…\.cargo`); a search of the whole tree for `Users\`, the e-mail address and the owner's name gives the full list | now |
| 1 | R4 | reviewer (docs only) | The three documents that name `pm-study` say the latency tools are the owner's and not published (PR-Q6) | after PR-Q6 |
| 1 | R5 | task | `tools/m2-*.cmd` removed, and the documents that name them updated (PR-Q5) | after PR-Q5 |
| 1 | R6 | task | The owner's name as test data in `pw-metadata` replaced (PR-Q4) | after PR-Q4 |
| 1 | R7 | read-through (fresh agents) + reviewer | `done/` (214 files) and `docs/analyses/` read once "with public eyes": personal data, private paths, test-image names that say more than needed, wording about other products against `docs/clean-room.md`. Split by number ranges (for example T1–T70, T71–T140, T141–T212, the logs, the analyses); the agents only report, the reviewer fixes | now |
| 1 | R8 | reviewer (docs only) | README: "Windows only today; open an issue if you want Linux or macOS" (backlog, Linux and macOS ports); CONTRIBUTING per PR-Q7; `SECURITY.md` per PR-Q8; backlog items done here struck | after PR-Q7, PR-Q8 |
| 1 | R10 | task | **The last step of the rename** (ADR 0012 decision 3, "step 2"): the binary renamed from `pw-loupe` to `rawstride` (`[[bin]]` in `crates/pw-loupe/Cargo.toml`), and the documents and scripts that name `pw-loupe.exe` updated (`docs/building.md`, `latency.ps1` if it starts the exe). ADR 0012 set it "at the latest when the egui program replaces the Tauri app", which happened with T207 on 2026-11-19; it had no task. Unchanged as ADR 0012 decision 4 says: crate names, folders, branches, `PW_TESTDATA`. ADR 0012 gets a line that step 2 is done | after PR-Q3 (a failed trademark search would mean another name), before C1 (the workflow and `docs/building.md` name the exe) |
| 2 | H1 | read-through (fresh agent) + reviewer | **The whole history**, every commit on the branches of PR-Q1, not only today's files: personal data, keys, tokens, passwords, private test images and their metadata, copyrighted reference specs (the IPTC files), large binaries (`git rev-list --objects --all` with sizes), files that were later removed (for example the old `docs/agent-tasks.md` and `src-tauri/`). `gitleaks` per PR-Q10. Result: a list, each finding "remove from the history" or "leave", decided by the owner | after R1–R8 (so the list is not about things already fixed in the files) |
| 3 | H2 | reviewer, with the owner | **Only if PR-Q2 is "rewrite" or H1 found something to remove.** On a fresh mirror clone, not in the worktrees: `git filter-repo` with a `.mailmap` (and the paths to remove). filter-repo also rewrites short hashes in commit messages; hashes quoted in files are not rewritten, so the documents' hashes are updated from filter-repo's `commit-map` in the same run (a script, AGENTS.md and CLAUDE.md §3). Checked: same tree at the tip of each branch apart from the intended changes, `check.ps1` passes on a fresh clone | after H1 and the owner's yes |
| 3 | H3 | owner on GitHub, reviewer guides | **Only after H2.** The old repository renamed (for example `RawStride-old`), kept private and archived; a new empty `RawStride` created; the rewritten branches pushed; "Block command line pushes that expose my email" turned on. The two worktrees (`C:\Development\PhotoWorkflow` and `…-GUI`) are cloned anew from the new repository; the old ones kept until the new ones build | after H2 |
| 4 | C1 | task (tooling) | `.github/workflows/ci.yml` as in the backlog item: `windows-latest`, the pinned toolchain, `vcpkg install` with binary cache, ExifTool 12.67+ on `PATH`, `.\tools\check.ps1` without `-Ignored`, `test-output.txt` as an artifact. While private only `workflow_dispatch` and pull requests to `main`. The first run also tests `docs/building.md` on a clean machine | after H3 (or after H1 if there is no rewrite), so the workflow is not written twice |
| 4 | C2 | owner + reviewer | One green run by hand; what the clean machine found goes into `docs/building.md` (a task if it is more than text) | after C1 |
| 5 | P1 | owner on GitHub, reviewer lists | Settings before the switch: description and topics, Issues on, private vulnerability reporting on, `main` protected against force pushes and deletion, Actions permissions read-only by default | after C2 and PR-Q3 |
| 5 | P2 | owner | The repository made public | after P1 |
| 5 | P3 | task (tooling) | CI on every push (the trigger widened) | after P2 |

Order 1 steps can run in parallel (they touch different files), one task at a time per
`check.ps1` run. R1, R2, R5, R6, R10 and C1, P3 change tools or code, so they are tasks with briefs
(rule: tooling changes via a task); R3, R4, R8 are documentation the reviewer writes.

## 3. Risks

- **A history rewrite changes every commit hash.** Everything that points at a hash breaks: the
  documents (handled in H2), the memory notes of the sessions, open pull requests (none today),
  and any clone the owner has elsewhere. Done once, with both worktrees clean and pushed first.
- **Once public, the history is out.** Forks and caches keep what was pushed; H1 has to be done
  before P2, not after.
- **CI cost**: Windows minutes count double while private; that is why C1 runs only by hand until
  P3. Building LibRaw from source without a warm cache takes long on the first run.
- **The real-file tests** are ignored without `PW_TESTDATA`, so CI shows green without one RAW
  being decoded or written (R9 / PR-Q9 fixes it later). The README or `docs/TESTING.md` should say
  so, so a green badge is not read as more than it is.
- **`cargo audit`'s warnings** (`zbus_names`, `zvariant`, `zvariant_derive` unmaintained,
  RUSTSEC-2026-0370 to 0372, through `eframe`'s Linux accessibility bridge, not built on Windows)
  will show in a public CI log. Warnings, the check passes; the backlog item stays.

## 4. Not in this milestone

- **After the repository is public:** R9 free test files (PR-Q9); issue templates; Linux and macOS
  ports only if there is interest (backlog).
- **Before the first binary release** (ADR 0002 §4, a later milestone): RawStride's distribution
  folder (exe, runtime DLLs, ExifTool per §2.5, `LICENSE`); `THIRD-PARTY-NOTICES.txt`; the GPL
  "Appropriate Legal Notices" in the program (an About box; needs an ADR 0002 §4 line first,
  backlog); SPDX headers in the source files (optional, decide then).

## Revision history

| Revision | Date | Changes |
|---|---|---|
| 1 | 2026-11-19 | Proposed by the reviewer from `backlog.md`, ADR 0002 §4 and the project status; waits for the owner's answers to PR-Q1–PR-Q10 |
| 2 | 2026-11-19 | R10 added (owner): the binary renamed to `rawstride` (ADR 0012 step 2), after PR-Q3 and before C1 |
| 3 | 2026-11-19 | Accepted: the owner approved PR-Q1–PR-Q10 as proposed |
