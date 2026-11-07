# Authoritative Agent Rules for PhotoWorkflow

## 1. Project Ownership and Authority
- The project owner has final authority over architecture, requirements, implementation decisions, and acceptance criteria.
- Explicit instructions from the project owner take precedence over an agent's assumptions.
- These rules apply to every coding agent and automated development tool working on this repository.

## 2. Worktree and Git Authorization
- You ARE explicitly authorized to read and modify the experimental/messy code currently placed in git staging (`git diff --staged`).
- You ARE authorized to create local git commits **ONLY** after all Rust tests (`cargo test`) are 100% green and successful.
- Never push changes to remote repositories or merge into the main branch autonomously.
- Preserve all existing uncommitted, untracked work outside the authorized task scope.

## 3. Rust Code Quality & Clean-Room Discipline
- **NEVER CRASH:** Non-test code must return errors instead of panicking. 
- **NO PANIC METHODS:** Absolutely no usage of `.unwrap()`, `.expect()`, or `panic!` in production code. All errors must be handled explicitly using `Result`, `Option`, or the `?` operator.
- Write idiomatic, memory-safe Rust that strictly adheres to the rules of the borrow checker.
- Run `cargo clippy` and fix all warnings before completing a task.

## 4. Testing and Verification
- Read `docs/TESTING.md` before changing production code or tests.
- Do not claim a test passed unless it was actually executed and its result was observed.
- Never weaken, remove, bypass, or rewrite tests merely to obtain a passing result.
