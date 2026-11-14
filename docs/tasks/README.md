# Agent tasks

Small, well-defined tasks for coding agents. Each task is reviewed by the project owner together
with a reviewer before it is merged. Read `AGENTS.md`, `CLAUDE.md` and `docs/TESTING.md` first; they
take precedence over this folder.

**An agent reads this README and its own task's file, nothing else in this folder** unless the task
names another file. Finished tasks are in `done/`; reading them is not needed to do a new task.

| Where | What |
|---|---|
| `T<nnn>-<name>.md` (this folder) | Open tasks, one file each |
| [`backlog.md`](backlog.md) | Not for agents yet: needs a decision, a dependency or an ADR first |
| [`plans/`](plans/) | Step lists for ADRs that have none of their own (ADR 0007 milestone 8, ADR 0017) |
| [`done/`](done/README.md) | Finished tasks T1–T137 and the log they were kept in until 2026-11-13 |


## How to work on a task

1. **Branch:** work on `feature/gui-workspace` only. Never commit to `main`; the owner
   moves `main` forward after review.
2. **One task at a time.** Change only the files the task names. If another file must change, stop
   and explain why instead of changing it.
3. **No new dependencies** and no changes to any `Cargo.toml` `[dependencies]` unless the task says
   so.
4. **Rust rules:** no `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or unchecked indexing in
   non-test code. Return errors instead.
5. **Tests:** every behaviour change gets a test. Never weaken, delete or rewrite an existing test
   to make it pass; if a test seems wrong, stop and explain.
6. **Before you report back,** run `.\tools\check.ps1` and keep `test-output.txt` from that run
   (it must be newer than your last change). Do not run `cargo test -p <crate>` or
   `cargo clippy -p <crate>` on their own: a single-crate build uses a different feature set, which
   builds new build scripts that the owner's antivirus may block (seen as linker error LNK1104). While
   working you may run `cargo build --workspace` or `cargo test --workspace`.
   **One exception (owner, 2026-11-12):** Windows Smart App Control sometimes refuses to run a
   rebuilt test program (`could not execute process … (never executed)`, `os error 4551`, "En
   programkontrollprincip har blockerat den här filen"). Then, and only for the programs it
   refused, run them on their own (`cargo test -p <crate> --lib`, `--bin <name>` or
   `--test <name>`). If they pass and nothing else in `check.ps1` failed, the task may be set to
   "Ready for review". In the report, give the `check.ps1` summary, the refused programs, the
   separate runs with their `test result` lines, and the total (the `check.ps1` run plus the
   separate runs). Never change a Windows security setting, and never build elsewhere (rule 7).
7. **Build only in the project's own `target` folder** (`AGENTS.md` §5). Do not set `CARGO_TARGET_DIR` or
   `--target-dir`, and do not build in a temporary folder (for example under `%TEMP%`). The
   owner's antivirus has an exception for `target` only; builds elsewhere are quarantined.
   If the program is running and `target` is locked, ask the owner to close it.
8. **Do not commit.** Leave the changes uncommitted so they can be reviewed with `git diff`.
   Do not create checkpoint commits or refs in this repository.
9. **Report:** list every file changed, what changed and why, and paste the exact output of the
   summary of `test-output.txt` from step 6. Then mark the task "Ready for review" in its file.
10. **Do the whole task as written in its file.** Read your task's file under `docs/tasks/` as it is
    in your working tree when you start, and do every numbered point under **What** and every test
    under **Tests to add**. If a point seems wrong, unclear or impossible, stop and ask; do not leave
    it out, shorten it or replace it with your own version.
11. **`docs/tasks/` is the owner's.** An agent may change exactly two things, both in its own task's
    file:
    - the status at the end of the file's heading (for example `— Open` → `— Ready for review`);
    - one new row at the **end** of the file's Log table.

    Never rewrite, shorten or move task text (your own task included), never touch other files in
    `docs/tasks/` (other tasks, this README, `backlog.md`, `plans/`, `done/`), and never write the
    file from another copy or from memory. Edit it in place. Check with `git diff docs/tasks/`
    before you report: the diff must show only those two changes.
12. **Other documentation:** update `docs/test-inventory.md` (counts and the description of the
    crate's tests) when you add tests; it is the only other file under `docs/` an agent changes, unless
    the task names one.
13. **Texts the user sees** (so another language can be added later in one place). This rule applies
    to every task that adds or changes a text shown to the user, whichever rules the task lists:
    - **the language is English** (owner, 2026-11-13; `docs/GUI_REQUIREMENTS.md` GR14), with the
      terms of `docs/glossary.md`; a term the glossary lacks is added to it in the same task and
      named in the report;
    - each such text is made by a function in its area's text module (for example `info_panel.rs`
      or `editor.rs`, as the task names); drawing code (egui) holds no user-visible string literals;
    - each text is a whole sentence built from one template with placeholders (`format!` with the
      values filled in), never joined from separately made pieces, so another language can change
      the word order; numbers that need singular/plural are formatted inside the same function;
    - property names come only from `property_label`;
    - layout does not assume a fixed text width (texts wrap or the area grows).

    Existing hard-coded texts (for example `UNSUPPORTED_TEXT` and the loupe's texts) stay where they
    are unless the task says to move them. If a text the task needs has no function in a file the
    task names, do not write it in drawing code: stop and ask.

## Status legend

| Status | Meaning |
|---|---|
| Open | Not started, or sent back after a review (the log says what is left) |
| In progress | An agent is working on it |
| Ready for review | Done by the agent, waiting for review |
| Done | Reviewed and committed |
| Blocked | Needs a decision from the owner first |

## Open tasks

| Task | Title | Status |
|---|---|---|

## Writing a task (owner and reviewer)

- **Number:** the next free number after the highest in this folder and `done/` (T138 next).
- **File:** `docs/tasks/T138-<a-few-words>.md`; three digits, lower case, words joined by `-`.
- **Index:** add a row to "Open tasks" above and keep its status in step with the file's heading.
- **When it is Done and committed:** `git mv` the file to `done/`, move its row to `done/README.md`.
- **Owner decisions** (decisions, approved Q proposals) are written into the ADR or requirement
  they change before the task is Done, not only into the task's log (`AGENTS.md` §6).

Skeleton (the fields recent tasks use; leave out those that do not apply):

```markdown
# T138. <Title> — Open

- **Why:** …
- **Crates:** … **Files:** …
- **What:**
  1. …
- **Tests to add:** …
- **Not in this task:** …
- **Rules:** …
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| YYYY-MM-DD | Open | Added |
```
