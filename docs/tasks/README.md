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
| [`done/`](done/README.md) | Finished tasks T1–T167 and T175, and the log they were kept in until 2026-11-13 |


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
   `cargo clippy -p <crate>` on their own (`AGENTS.md` §5). While working you may run
   `cargo build --workspace` or `cargo test --workspace`.
7. **Build only in the project's own `target` folder** (`AGENTS.md` §5). Do not set `CARGO_TARGET_DIR` or
   `--target-dir`, and do not build in a temporary folder (for example under `%TEMP%`); a
   development machine may exclude only `target` from antivirus scanning.
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
| [T168](T168-focus-marking-key-and-drawing.md) | ADR 0017 U9a, second part: the focus marking key, menu item and drawing | Open |
| [T169](T169-move-an-image-with-its-companions.md) | ADR 0017 U9b, first part: moving an image with its companion files | Open |
| [T170](T170-move-request-in-the-metadata-worker.md) | ADR 0017 U9b, second part: the move as a request to the metadata worker | Blocked: waits for T169 |
| [T171](T171-move-settings.md) | ADR 0017 U9b, third part: the two move settings | Blocked: waits for T170 |
| [T172](T172-move-rules-and-undo-list.md) | ADR 0017 U9b, fourth part: the move rules, the undo list, the tab's list and the texts | Blocked: waits for T171 |
| [T173](T173-move-events-in-the-metadata-cache.md) | ADR 0017 U9b, fifth part: the move requests and answers in the shell's metadata part | Blocked: waits for T172 |
| [T174](T174-move-keys-menu-and-undo.md) | ADR 0017 U9b, last part: the keys, the menu items, the moves and the undo in the shell | Blocked: waits for T173 |

## Writing a task (owner and reviewer)

- **Number:** the next free number after the highest in this folder and `done/` (T176 next).
- **File:** `docs/tasks/T176-<a-few-words>.md`; three digits, lower case, words joined by `-`.
- **Index:** add a row to "Open tasks" above and keep its status in step with the file's heading.
- **Size (owner, 2026-11-15, after T142):** one task is one layer (a C++ shim or a core crate, or
  the shell in `pw-loupe`), about eight files at most, and gives one result the owner can check by
  hand or a test can show. A step that needs more (for example new statistics in `pw-libraw`, the
  view core asking for them, and a panel drawing them) is split. A latency measurement and a new
  panel are not in the same task. If a task's agent must change files outside the task's list, the
  task was too big or its list was guessed: note it in the log, and split the next one.
- **Chains:** a split step is written as a chain at once (for example T146 → T147 → T148), the later
  tasks "Blocked: waits for T146" until the one before is Done. The step's row in `plans/` names
  the whole chain. Writing the last task's file first is how a gap is found before code is written
  (T142's inspector part was found only after T142 was done).
- **A cold read before Open (owner, 2026-11-15):** before a task is set to Open, the reviewer has
  a fresh agent, with none of the reviewer's history, read only `AGENTS.md`, this README and the
  task's file, and answer: can the task be done with only the named files and points; what is
  missing, unclear or contradicts the code; which other files would have to change. The reviewer
  fixes the task's text (or asks the owner) and notes the cold read in the log ("Cold read: …",
  what was changed). The fresh agent changes nothing.
- **An extra code review for large or risky tasks:** a task that changes the C++ shim, the safe
  write (`pw-metadata`), more than one crate, or more than about eight files gets, besides the
  reviewer's own review, an independent review of the diff (`/code-review` or a fresh review
  agent) before the owner is asked to approve. Its findings and what was done with them go in
  the log's review row.
- **When it is Done and committed:** `git mv` the file to `done/`, move its row to `done/README.md`.
- **Leftovers:** before a task is set to Done, everything its log calls left, not covered, noted
  or not changed gets a home: a new task, a row in `plans/`, an item in `backlog.md`, or a line in
  `docs/test-inventory.md`. The Done row of the log says where. A done task's log is not read again
  when the next task is written, so nothing may live only there.
- **Hand check:** a step's row in `plans/` says "done" only after the owner's hand check
  (ADR 0017 §7); until then it says "awaiting the owner's hand check". A fix found in the hand check
  is a log row in the task it belongs to.
- **Parallel tasks:** two tasks may run at the same time only if they change different functions
  (as T142 and T143 did); `.\tools\check.ps1` is run by one at a time, and each task is reviewed
  and committed on its own, one commit per task.
- **Owner decisions** (decisions, approved Q proposals) are written into the ADR or requirement
  they change before the task is Done, not only into the task's log (`AGENTS.md` §6).

Skeleton (the fields recent tasks use; leave out those that do not apply):

```markdown
# T176. <Title> — Open

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
