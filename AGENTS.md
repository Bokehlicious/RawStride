# Authoritative Agent Rules for RawStride (formerly PhotoWorkflow)

## 1. Ownership, Roles and Precedence
- The project owner has final authority over architecture, requirements, implementation decisions, and acceptance criteria.
- Explicit instructions from the project owner take precedence over an agent's assumptions.
- These rules apply to every coding agent and automated development tool working on this repository.
- **Roles:**
  - **Owner:** decides, approves and does the hand checks.
  - **Task agent:** carries out one task from `docs/tasks/` and reports back; it follows `docs/tasks/README.md` and its task's file.
  - **Reviewer:** an agent or person the owner asks to write tasks, review a task agent's work, record decisions (§6) and commit after the owner's approval.
- **Precedence:** this file comes first, then `CLAUDE.md`, `docs/TESTING.md` and `docs/tasks/README.md`. When two rules differ, the stricter one applies.

## 2. Worktree and Git
- The work happens on `feature/gui-workspace` in its own worktree. Never commit to `main`; the owner moves `main` forward after review.
- Never push to a remote repository or merge into `main`.
- **Task agents never commit.** They leave their changes uncommitted for review (`docs/tasks/README.md`, rule 8).
- **The reviewer** may create a local commit on `feature/gui-workspace` only after the owner has approved the change and `.\tools\check.ps1` (§5) is all OK.
- Staged changes (`git diff --staged`) are touched only when the owner says so for the task at hand. Otherwise they are the owner's work, like any other uncommitted or untracked change outside the task: leave them as they are.

## 3. Rust Code Quality
- **Never crash:** non-test code returns errors instead of panicking.
- **No panic paths in non-test code:** no `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or unchecked indexing. Handle errors explicitly with `Result`, `Option` or the `?` operator.
- Write idiomatic, memory-safe Rust that strictly adheres to the rules of the borrow checker.
- No new dependencies, and no change to any `Cargo.toml` `[dependencies]`, without the owner's approval. Do not install software or toolchains without the owner's approval.
- `cargo clippy` must have no warnings; it runs as part of `.\tools\check.ps1` (§5).

## 4. Clean Room
- Follow `docs/clean-room.md`. RawStride is inspired by other culling tools, including Photo Mechanic, and must contain nothing derived from their code.
- **Level E is always forbidden:** no decompiling, disassembling, debugger attachment or other analysis of Photo Mechanic's executables and libraries, with any tool (REA, Ghidra or similar).
- Clean-room material and session outputs about other products stay outside the repository (`.gitignore`: `/pm-clean-room/`, `/Claude outputs/`).

## 5. Building, Testing and Verification
- Read `docs/TESTING.md` before changing production code or tests.
- **Verify with `.\tools\check.ps1`** (fmt, tests, clippy; it writes `test-output.txt`). Do not run `cargo test -p <crate>` or `cargo clippy -p <crate>` on their own: a single-crate build uses a different feature set, and its new build scripts have been blocked by the owner's antivirus (linker error LNK1104). While working, `cargo build --workspace` and `cargo test --workspace` are fine.
- **Build only in the project's own `target` folder.** Do not set `CARGO_TARGET_DIR` or `--target-dir`, and do not build in a temporary folder: the antivirus has an exception for `target` only, and builds elsewhere are quarantined. If the program is running and `target` is locked, ask the owner to close it.
- Never change a Windows security setting.
- Do not claim a test passed unless it was actually executed and its result was observed.
- Never weaken, remove, bypass, or rewrite tests merely to obtain a passing result.

## 6. Recording Owner Decisions
- When the owner decides something during a task or review (a decision, an approved Q proposal, a changed requirement), the reviewer writes it into the ADR or requirement document it affects, with the date, before the task is set to Done.
- The task log may refer to the decision but must not be the only place it is written down.
- A task agent does not edit ADRs or requirement documents unless the task names them (`docs/tasks/README.md`, rule 12); it reports the decision so the reviewer can record it.

## 7. Language
- The documentation, code, comments, commit messages and task texts are in English.
- **Every text the user sees is English** (owner, 2026-11-13; `docs/GUI_REQUIREMENTS.md` GR14), with
  the terms of `docs/glossary.md`, and is made as `docs/tasks/README.md` rule 13 says.
