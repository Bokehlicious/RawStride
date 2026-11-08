# Agent task list

Small, well-defined backend tasks for coding agents (local AI). Each task is reviewed by the
project owner together with a reviewer before it is merged. Read `AGENTS.md`, `CLAUDE.md` and
`docs/TESTING.md` first; they take precedence over this file.

## How to work on a task

1. **Branch:** work on `feature/gui-workspace` only. Never commit to `m2-and-adr0008-phase1` or
   `main`; the owner moves `m2-and-adr0008-phase1` forward after review.
2. **One task at a time.** Change only the files the task names. If another file must change, stop
   and explain why instead of changing it.
3. **No new dependencies** and no changes to any `Cargo.toml` `[dependencies]` unless the task says
   so.
4. **Rust rules:** no `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or unchecked indexing in
   non-test code. Return errors instead.
5. **Tests:** every behaviour change gets a test. Never weaken, delete or rewrite an existing test
   to make it pass; if a test seems wrong, stop and explain.
6. **Before you report back,** run and keep the output of:
   ```powershell
   cargo fmt --all --check
   cargo test -p <crate>
   cargo clippy -p <crate> --all-targets
   ```
   (`<crate>` is named in the task; for `src-tauri` it is `photoworkflow`.)
7. **Do not commit.** Leave the changes uncommitted so they can be reviewed with `git diff`.
   Do not create checkpoint commits or refs in this repository.
8. **Report:** list every file changed, what changed and why, and paste the exact output of the
   commands in step 6. Then mark the task "Ready for review" below.

## Status legend

| Status | Meaning |
|---|---|
| Open | Not started |
| In progress | An agent is working on it |
| Ready for review | Done by the agent, waiting for review |
| Done | Reviewed and committed |
| Blocked | Needs a decision from the owner first |

## Tasks

Ordered from easiest to hardest. Start at the top.

### T1. Fix the five existing clippy warnings — Open

- **Crates:** `pw-rawview` (tests), `photoworkflow` (`src-tauri`).
- **Files:** `crates/pw-rawview/tests/engine.rs`, `src-tauri/src/imaging.rs`.
- **What:** `cargo clippy --workspace --all-targets` reports:
  - `crates/pw-rawview/tests/engine.rs:987` — `cloned_ref_to_slice_refs`: use
    `std::slice::from_ref(&bad)` instead of `&[bad.clone()]`.
  - `src-tauri/src/imaging.rs:49, 53, 57, 116` — `chunks_exact_to_as_chunks`: use
    `as_chunks::<N>()` as clippy suggests.
- **Done when:** `cargo clippy --workspace --all-targets` shows no warnings from these files and
  `cargo test -p pw-rawview` and `cargo test -p photoworkflow` pass with the same test counts as
  before.
- **Do not:** use `#[allow(...)]` to silence the warnings.

### T2. pw-loupe: use the workspace dependency for pw-metadata — Open

- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/Cargo.toml` (this task may change it).
- **What:** replace `pw-metadata = { path = "../pw-metadata" }` with
  `pw-metadata.workspace = true`, like the other internal crates.
- **Done when:** `cargo test -p pw-loupe` passes and `Cargo.lock` does not change (check with
  `git diff Cargo.lock`).

### T3. pw-loupe: typed metadata state instead of comparing display text — Open

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/metadata.rs`, `crates/pw-loupe/src/main.rs`.
- **Problem:** `main.rs` decides behaviour by comparing user-visible text:
  `notice == "Metadata loading…"` (to keep repainting) and `notice.starts_with("Metadata error")`
  (to show the error overlay). Changing the wording would silently break both.
- **What:**
  - Add `pub fn is_loading(&self, path: Option<&Path>) -> bool` and
    `pub fn is_error(&self, path: Option<&Path>) -> bool` to `MetadataCache`.
  - Use them in `main.rs` instead of the two text comparisons. `notice()` stays for display.
  - Rename the test `metadata_cache_uses_absolute_paths_and_reports_queue_errors` to a name that
    matches what it checks (absolute paths and the loading state), e.g.
    `metadata_cache_uses_absolute_paths_and_starts_loading`.
- **Tests to add:** `is_loading` is true right after `begin_load` for a supported file;
  `is_error` is true after the worker disconnects (see the existing
  `disconnected_worker_turns_loading_into_an_error` test for how); both are false for an
  unsupported file and for `None`.
- **Done when:** no string comparison on notice text remains in `main.rs`; `cargo test -p pw-loupe`
  passes.

### T4. Ingest summary: only count files that were copied or already present — Open

- **Crate:** `photoworkflow`. **File:** `src-tauri/src/ingest.rs`.
- **Problem:** in `run()`, the `folders` summary counts every plan item that did not fail. When the
  user cancels, files that were never copied are still counted.
- **What:**
  - Move the folder counting into a separate function that does not need Tauri, for example
    `fn folder_summary(items: &[&pw_ingest::PlanItem], dest: &Path) -> Vec<(String, usize)>`,
    keeping the current sort order (largest first, then by name).
  - Call it only with items that are actually in the destination. `pw_ingest::Report` only
    gives counts, not which files were copied, so decide per item by checking whether
    `opt.dest.join(&item.rel)` exists. This is reliable because `pw-ingest` renames a copy to its
    final name only after it has been verified (ADR 0005, point 6). Do not change `pw-ingest`.
  - Keep the existence check outside `folder_summary`, so the function itself stays pure and
    testable.
- **Tests to add:** the sort order; items in two folders; only the items passed in are counted.
- **Done when:** `cargo test -p photoworkflow` passes.

### T5. Card eject: safer PowerShell call and a real wait — Open

- **Crate:** `photoworkflow`. **File:** `src-tauri/src/ingest.rs`, function `eject()`.
- **What:**
  - Start PowerShell by full path:
    `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` (read `SystemRoot` with
    `std::env::var_os`; if it is missing, return a clear message instead of using `PATH`).
  - Treat a non-zero exit status as a failure and include it in the message.
  - Replace the fixed 1.5 s sleep with polling: check every 250 ms, for at most 5 s, whether
    `<drive>\DCIM` is gone.
- **Tests to add:** put the polling in a small function that takes a "still present?" closure and
  test it without a real drive (gone at once, gone after a few polls, never gone).
- **Done when:** `cargo test -p photoworkflow` passes. The owner tests a real eject by hand.

### T6. Metadata test with real files: make a missing dataset visible — Blocked

- **Crate:** `pw-metadata`. **File:** `crates/pw-metadata/tests/sidecar.rs`.
- **Problem:** `real_dng_is_written_in_place` returns early and is counted as passed when
  `PW_TESTDATA` is not set (`docs/TESTING.md` §4.4: a skipped test must not count as passed).
- **Decision needed from the owner:** should the test fail when the dataset is missing (like the
  ExifTool tests now do), or be marked `#[ignore]` and run explicitly with
  `cargo test -- --include-ignored`?
- **Do not start** until the owner has decided and updated this entry.

## Not for agents yet

These need design decisions, new dependencies or platform-specific code, and stay with the owner
and reviewer:

- A timeout when ExifTool hangs (process supervision across threads).
- Suppressing the Windows "no disk in drive" dialog when scanning empty card readers (needs a
  Windows API call and a new dependency).
- Anything from ADR 0007 (IPTC/IIM/Exif metadata): the ADR is not approved yet.
- The import dialog fields for title and description in `ui/` (frontend).

## Log

| Date | Task | Status | Note |
|---|---|---|---|
| 2026-11-08 | – | – | List created |
