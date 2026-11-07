# Agent Operating Instructions

## 1. Instruction Hierarchy
`AGENTS.md` is the authoritative source for shared project rules. This file provides additional operational instructions for coding agents. If instructions conflict, follow `AGENTS.md` first.

## 2. Mandatory Investigation
Before making any changes:
1. Verify the current working directory, Git worktree, and branch.
2. Inspect Git status, focusing heavily on **staged changes** (`git diff --staged`). The project owner has left experimental code in staging for you to clean up.
3. Inspect the existing implementation and tests related to the task.

## 3. Scope Control & Implementation Discipline
- Modify only files explicitly related to the current task or currently in staging.
- Make the smallest change that fully satisfies the task. Avoid unrelated refactoring.
- Do not install software, toolchains, or new Cargo dependencies (`Cargo.toml`) without explicit approval.

## 4. Completion Report
A completion report must include:
- Every file modified or created.
- Summary of each change and why it was made.
- Tests actually executed (e.g., `cargo test`) and their exact results.
- Remaining risks, known limitations, or warnings from `cargo clippy`.
