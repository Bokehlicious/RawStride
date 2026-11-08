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

### T1. Fix the five existing clippy warnings — Done

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

### T2. pw-loupe: use the workspace dependency for pw-metadata — Done

- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/Cargo.toml` (this task may change it).
- **What:** replace `pw-metadata = { path = "../pw-metadata" }` with
  `pw-metadata.workspace = true`, like the other internal crates.
- **Done when:** `cargo test -p pw-loupe` passes and `Cargo.lock` does not change (check with
  `git diff Cargo.lock`).

### T3. pw-loupe: typed metadata state instead of comparing display text — Done

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

### T4. Ingest summary: only count files that were copied or already present — Done

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

### T8. Move reusable ingest helpers from src-tauri to pw-ingest — Done

- **Why:** the Tauri app is being replaced by the egui GUI (ADR 0001 amended 2026-11-07, ADR 0009).
  Logic that the egui GUI will also need belongs in the shared crate `pw-ingest`, not in
  `src-tauri`. `src-tauri` only keeps the Tauri glue and calls the shared functions (GR8: the
  Tauri app keeps working).
- **Crates:** `pw-ingest`, `photoworkflow`.
- **Files:** `crates/pw-ingest/src/lib.rs` (or a new module file in `crates/pw-ingest/src/`, e.g.
  `paths.rs`), `src-tauri/src/ingest.rs`.
- **What:** move these functions from `src-tauri/src/ingest.rs` to `pw-ingest` and make them `pub`:
  - `inside(a, b)` and `resolve(p, what)` (path overlap checks);
  - `drive_of(path)` (drive letter of a path);
  - `folder_summary(items, dest)` (folder counts for the summary).
- **Rules:**
  - **Behaviour unchanged:** same signatures, same results, same (Swedish) error messages. This is
    a move, not a rewrite.
  - Move their tests with them (`inside_…`, `resolve_…`, `drive_letter_…`, `folder_summary_…`).
    The tests keep their names and assertions.
  - `src-tauri/src/ingest.rs` calls `pw_ingest::…` instead of its own copies, and the copies are
    deleted. Nothing else in `src-tauri` changes.
  - Add a short `///` doc comment to each moved function.
  - No new dependencies.
- **Done when:** `cargo test -p pw-ingest` and `cargo test -p photoworkflow` pass; the total number
  of tests in the two crates is the same as before the move; `cargo clippy` shows no warnings.

### T5. Card eject in pw-ingest: safer PowerShell call and a real wait — Done

- **Start after T8 is done.**
- **Crates:** `pw-ingest`, `photoworkflow`.
- **Files:** `crates/pw-ingest/src/` (same place as `drive_of` after T8), `src-tauri/src/ingest.rs`.
- **What:**
  - Move `eject()` from `src-tauri/src/ingest.rs` to `pw-ingest` next to `drive_of`, as
    `pub fn eject(source: &Path) -> String`. `src-tauri` calls it; its own copy is deleted.
  - Start PowerShell by full path:
    `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` (read `SystemRoot` with
    `std::env::var_os`; if it is missing, return a clear message instead of using `PATH`).
  - Treat a non-zero exit status as a failure and include it in the message.
  - Replace the fixed 1.5 s sleep with polling: check every 250 ms, for at most 5 s, whether
    `<drive>\DCIM` is gone.
- **Tests to add:** put the polling in a small function that takes a "still present?" closure and
  test it without a real drive (gone at once, gone after a few polls, never gone).
- **Done when:** `cargo test -p pw-ingest` and `cargo test -p photoworkflow` pass. The owner tests
  a real eject by hand.

### T9. Eject polish: readable exit status, fast timeout test, no dead code off Windows — Done

- **Crate:** `pw-ingest`. **File:** `crates/pw-ingest/src/paths.rs`.
- **What:**
  1. In `eject()`, show the exit status with `{status}` instead of `{status:?}`, so the message
     reads "exit code: 1" instead of `ExitStatus(ExitStatus(1))`.
  2. Give `wait_until_gone` the timeout and the poll interval as parameters
     (`fn wait_until_gone(still_present: impl FnMut() -> bool, timeout: Duration, interval: Duration) -> bool`).
     `eject()` passes the current values (5 s and 250 ms) as named constants. The three tests pass
     short values (e.g. 20 ms timeout, 1 ms interval), so the "never gone" test no longer waits
     5 seconds.
  3. `wait_until_gone` is only used by the Windows code and by tests. Mark it
     `#[cfg(any(windows, test))]` so other platforms do not warn about dead code.
- **Rules:** behaviour on Windows unchanged; keep the three existing test names and what they check.
- **Done when:** `cargo test -p pw-ingest` passes and finishes in well under a second;
  `cargo clippy -p pw-ingest --all-targets` shows no warnings.

### T10. Card eject: try again when Windows is still busy after the import — Done

- **Start after T9 is done** (it uses T9's `wait_until_gone` with parameters).
- **Crate:** `pw-ingest`. **File:** `crates/pw-ingest/src/paths.rs`.
- **Finding (owner test, 2026-11-08):** right after an import, `eject()` reported that the card
  could not be ejected. Running the same PowerShell eject command by hand a moment later, with the
  app still open, worked. So the app does not hold the card; Windows is briefly busy after all files
  were read (e.g. Defender or the search indexer). The eject verb then fails silently, and waiting
  longer alone does not help: the verb must be invoked again.
- **What:**
  - Add a small function that does the retrying and can be tested without Windows, e.g.
    `fn retry_until_gone(attempts: usize, invoke: impl FnMut() -> Result<(), String>, still_present: impl FnMut() -> bool, timeout: Duration, interval: Duration) -> Result<bool, String>`:
    for each attempt, call `invoke`, then `wait_until_gone(still_present, timeout, interval)`;
    return `Ok(true)` as soon as the drive is gone, `Ok(false)` after the last attempt, and the
    error at once if `invoke` fails (e.g. PowerShell could not start or exited with an error).
  - `eject()` uses it with 3 attempts, 5 s timeout and 250 ms interval (named constants), and
    `invoke` runs the existing PowerShell command.
  - Keep the existing messages. The final failure message may add how many attempts were made.
- **Tests to add** (with short timeouts so they run instantly): gone after the first attempt (invoke
  called once); gone after the second attempt (invoke called twice); never gone (invoke called 3
  times, `Ok(false)`); invoke fails on the first attempt (error returned, no further attempts).
- **Done when:** `cargo test -p pw-ingest` passes quickly and clippy is clean. The owner tests a real
  import with eject again.

### T11. Card eject: keep PowerShell running until the card is gone — Done

- **Crate:** `pw-ingest`. **File:** `crates/pw-ingest/src/paths.rs`.
- **Finding (owner test, 2026-11-08):** the Windows eject verb finishes in the background. When the
  PowerShell process exits right after `InvokeVerb('Eject')`, the eject is cancelled. Tested by hand
  with the app closed:
  - `powershell -Command "…InvokeVerb('Eject')"` then wait 5 s outside → card still there (`True`);
  - `powershell -Command "…InvokeVerb('Eject'); Start-Sleep 5"` → card ejected (`False`).
  So T10's retries cannot help on their own: every attempt is cancelled the same way.
- **What:**
  - Build the PowerShell script in a small pure function, e.g. `fn eject_script(drive: &str) -> String`,
    available on all platforms (so it can be tested), and use it in `eject()`.
  - The script invokes the verb and then **stays alive until `<drive>\DCIM` is gone, at most 5 s**,
    polling every 250 ms, for example:
    `$d = (New-Object -ComObject Shell.Application).Namespace(17).ParseName('E:'); $d.InvokeVerb('Eject'); for ($i = 0; $i -lt 20 -and (Test-Path 'E:\DCIM'); $i++) { Start-Sleep -Milliseconds 250 }`
  - Keep everything from T10 (the Rust side still checks and retries up to 3 times) and keep the
    messages.
- **Tests to add:** `eject_script("E:")` contains `ParseName('E:')`, `InvokeVerb('Eject')`,
  `Test-Path 'E:\DCIM'` and `Start-Sleep`; it contains no other drive letter.
- **Done when:** `cargo test -p pw-ingest` passes and clippy is clean. The owner tests a real import
  with eject again; T5, T10 and T11 are then marked Done together.

### T19. pw-viewcore: run the "not a RAW file" test in every test run — Done

- **File:** `crates/pw-viewcore/tests/native_files.rs`, test `a_file_that_is_not_raw_shows_an_error`.
- **Why:** the test writes its own broken file and does not use the real camera files, but it is
  ignored and calls `testdata()` only because it used to check `PW_TESTDATA`.
- **What:** remove its `#[ignore = …]` line and the `let _ = testdata();` line. Nothing else.
- **Done when:** `cargo test -p pw-viewcore` runs it (1 more passed, 1 fewer ignored) and clippy is
  clean.

### T20. ADR 0010 M1: move card detection and request validation to pw-ingest — Open

- **Why:** the egui import (ADR 0010) must give the same answers as the Tauri dialog. The last two
  pieces of import logic that still live in `src-tauri` move to the shared crate first (ADR 0010
  §4.3).
- **Crates:** `pw-ingest`, `photoworkflow`.
- **Files:** `crates/pw-ingest/src/` (new module files, e.g. `volumes.rs` and `request.rs`, plus
  `lib.rs`), `src-tauri/src/ingest.rs`.
- **What:**
  1. **Card detection.** Move the body of `ingest_volumes()` to `pw-ingest` as
     `pub fn card_volumes() -> Vec<PathBuf>`. Put the check itself in a small function that takes the
     candidate roots, e.g. `fn with_dcim(roots: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf>`,
     so it can be tested with temporary folders. `card_volumes()` passes `A:\` … `Z:\` on Windows and
     keeps today's two-level walk of `/Volumes`, `/media`, `/run/media` elsewhere. The Tauri command
     `ingest_volumes()` stays and only converts the paths to strings.
  2. **Request validation.** Move the logic of `IngestRequest::source()` and `IngestRequest::options()`
     to `pw-ingest` as plain functions on a plain input struct, e.g.
     `pub struct OptionsInput { source, dest, backup: Option<String>, folder, name, job, first_seq }`
     and `pub fn validate_source(source: &str) -> Result<PathBuf, String>` /
     `pub fn build_options(input: &OptionsInput) -> Result<Options, String>`.
     `IngestRequest` in `src-tauri` keeps its serde fields and calls these.
- **Rules:**
  - **Behaviour unchanged:** the same checks in the same order and **the same Swedish messages**
    (including the "Mappmall: " and "Filnamnsmall: " prefixes).
  - No serde or Tauri types in `pw-ingest`; no new dependencies.
  - Nothing else in `src-tauri` changes; `ui/web` is not touched.
- **Tests to add (in `pw-ingest`):**
  - `with_dcim`: a temporary root with `DCIM` is found, one without is not, a missing root is skipped;
  - `validate_source`: empty, relative, missing folder, valid folder;
  - `build_options`: empty destination; destination inside the source; source inside the destination;
    backup equal to the destination; a bad folder template and a bad file-name template (check the
    prefix of the message); a valid input with and without backup.
- **Done when:** `.\tools\check.ps1` is all OK, the Tauri app's test count is unchanged, and the new
  tests pass.

### T21. ADR 0010 M2: folder tabs in pw-loupe — Open

- **Can be done in parallel with T20** (different files).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/lib.rs` (or a new module file such as
  `src/tabs.rs` declared in `lib.rs`), `crates/pw-loupe/src/main.rs`.
- **What (ADR 0010 §3.3 and §11):**
  1. **Tab model in plain Rust** (no egui types), e.g. `pub struct Tabs` with tabs that each hold
     `folder: Option<PathBuf>`, `files: Vec<PathBuf>` and `current: usize`:
     - starts with one empty tab;
     - `open_in_active(folder, files, pos)`; `open_new(folder, files, pos)`, which activates the
       existing tab instead if that folder is already open (GR12);
     - `activate(i)`, `next()` and `previous()` (wrapping), `close_active()` (the last tab becomes
       empty instead of closing);
     - `set_current(pos)` to remember the image of the active tab;
     - `titles()`: the folder name, or "Tom" for an empty tab.
  2. **Tab keys** as a plain function with tests, separate from `key_action` (which must keep
     returning `None` for Ctrl combinations): Ctrl+Tab next, Ctrl+Shift+Tab previous, Ctrl+W close.
  3. **Drawing in `main.rs`:** a tab bar above the image; clicking a tab activates it. On activation:
     `core.open_folder(files)`, `core.go(current)` and the metadata `begin_load` for that folder.
     Remember the image from `frame.pos` with `set_current`. Zoom resets on activation (Q1).
  4. **Drag-and-drop:** opens in the active tab as today; with **Ctrl** held it opens a new tab (Q3).
- **Rules:** the loupe's existing keys and behaviour are unchanged (ADR 0009 §3.3); one view core for
  all tabs; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** the tab model (open in active, open new, open an already open folder, next and
  previous with wrap, close the last tab, close a middle tab and which tab becomes active, remembered
  image per tab, titles) and the tab keys.
- **Done when:** `.\tools\check.ps1` is all OK; the owner checks by hand that tabs can be opened with
  Ctrl+drag-and-drop, switched with the keys and the mouse, and closed, and that each tab comes back
  on its own image.

### T7. Format the files that `cargo fmt --check` reports — Done

- **Crates:** `photoworkflow` (`src-tauri`), `preview-bench` (`tools/preview-bench`).
- **Files:** `src-tauri/src/main.rs`, `src-tauri/src/rawview.rs`, `tools/preview-bench/build.rs`.
- **What:** run `cargo fmt --all`. Only these three files should change, and only in layout (line
  breaks, the order of the `mod` lines in `main.rs`). If any other file changes, stop and report it.
- **Done when:** `cargo fmt --all --check` prints nothing, `git diff --stat` lists only these three
  files, and `cargo test -p photoworkflow` passes with the same test count as before.

### T6. Real-file tests, part 1 (pw-metadata, pw-ingest, pw-libjpeg): never pass silently — Done

- **Owner decision (2026-11-08):** a test that needs the real camera files is **ignored by default**
  and **fails when it is run explicitly without them**. So `cargo test` stays fast and green without
  the files (TESTING.md §5 "Fast Regression") and shows such tests as "ignored", not "passed"
  (§4.4, §6); `cargo test -- --ignored` is the "Real RAW Verification" run.
- **This task sets the pattern** that T12–T14 repeat for the other crates. Keep it simple and
  identical in every file.
- **Files:** `crates/pw-metadata/tests/sidecar.rs` (`real_dng_is_written_in_place`),
  `crates/pw-ingest/tests/ingest.rs` (`real_raw_uses_exif_time`),
  `crates/pw-libjpeg/tests/decode.rs` (`embedded_previews_of_the_test_raws_decode`),
  `tests/fixtures/README.md`.
- **The pattern:**
  - Every test that reads `PW_TESTDATA` gets
    `#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]`.
  - In each test file, a small helper replaces the early `return`s:
    `fn testdata() -> PathBuf` returns the folder from `PW_TESTDATA`, and **panics with a clear
    message** if the variable is not set or the folder does not exist. If a test needs a specific
    file (or a file with a given prefix), a second helper panics with a message that names the
    missing file and the folder it looked in.
  - `panic!`/`unwrap` are allowed here: this is test code (AGENTS.md §3).
  - Tests that do not read `PW_TESTDATA` are not touched and stay un-ignored.
  - Do not change what the tests check. Update the comment at the top of each file so it says how
    to run them (`$env:PW_TESTDATA = "<folder>"; cargo test -p <crate> -- --ignored`).
  - `tests/fixtures/README.md`: replace "are skipped when it is not set" with the new behaviour and
    the command above.
  - Leave `pw-metadata`'s `test_support::fake_exiftool_entry` as it is (it is a helper, already
    ignored).
- **Report back:** the output of
  1. `cargo test -p pw-metadata -p pw-ingest -p pw-libjpeg` (the three tests show as "ignored");
  2. the same with `-- --ignored` and `PW_TESTDATA` **not** set (the three tests fail with your
     message; paste one message);
  3. the same with `-- --ignored` and `PW_TESTDATA` set by the owner (all pass). If a test cannot
     find its file in the owner's folder, **stop and report which file and folder**; do not
     change the test to look elsewhere.
- **Done when:** the three runs behave as described and clippy is clean.

### T12. Real-file tests, part 2 (pw-raw, pw-viewcore) — Done

- **Start after T6 is done.** Use exactly the pattern from T6.
- **Watch for:** with `PW_TESTDATA` set, `real_files.rs` finished its 4 tests in 0.00 s, so they
  probably still skip because their files are not found in that folder. Expect the `--ignored` run
  to show which file is missing, and report it.
- **Files:** `crates/pw-raw/tests/real_files.rs` (4 tests), `crates/pw-viewcore/tests/native_files.rs`
  (3 tests).
- **Report back** the same three runs as in T6, for `-p pw-raw -p pw-viewcore`.
- **Done when:** all 7 tests are ignored by default, fail with a clear message without the files and
  pass with them; clippy is clean.

### T13. Real-file tests, part 3 (pw-libraw) — Done

- **Start after T6 is done.** Use exactly the pattern from T6.
- **Files:** `crates/pw-libraw/tests/damaged.rs` (`complete_files_are_never_called_damaged`,
  `truncated_files_are_damaged`), `crates/pw-libraw/tests/regions.rs` (the tests that call
  `check_regions_identical`, `check_ppg_regions_identical` or `check_methods_switch_cleanly`).
  `unreadable_files_fail_but_are_not_damaged`, `not_a_raw_file_is_an_error` and
  `version_is_reported` do not need the files and stay as they are.
- **Already fixed by the reviewer (2026-11-08):** `scratch()` now takes the test name, so each test
  has its own temporary folder (two tests shared one, and one removed it while the other was still
  writing). Keep that.
- **Note:** some of these tests look for a file by name prefix (e.g. a Panasonic file). A missing
  prefix must now fail with a message, not print "skipped".
- **Report back** the same three runs as in T6, for `-p pw-libraw`.
- **Done when:** as in T12.

### T14. Real-file tests, part 4 (pw-rawview with LibRaw) — Done

- **Start after T6 is done.** Use exactly the pattern from T6.
- **File:** `crates/pw-rawview/tests/libraw_engine.rs` (3 tests). The file only builds on Windows
  with the `libraw` feature; keep its `#![cfg(...)]` line.
- **Note:** `memory_stays_within_budget_and_is_freed` measures the memory of the whole test process.
  The file's own instructions say to run it with `--release` and `--test-threads=1`; keep that
  instruction in the comment and use it for the `--ignored` run.
- **Report back** the same three runs as in T6, for `-p pw-rawview` (the `--ignored` runs with
  `--release -- --ignored --test-threads=1`).
- **Done when:** as in T12.

### T15. A script that runs the checks and writes a readable report — Done

- **File:** `tools/check.ps1` (new).
- **Why:** the owner runs the same commands after every task and the output file is written as
  UTF-16, which is hard to read.
- **What:** a PowerShell script that, run from the repository root:
  - uses `$env:VCPKG_INSTALLED_DIR` if it is set; otherwise, if
    `..\PhotoWorkflow\vcpkg_installed` exists next to this worktree, uses that and prints which
    folder it chose; otherwise stops with a clear message;
  - runs, in order, `cargo fmt --all --check`, `cargo test --workspace` and
    `cargo clippy --workspace --all-targets`, and keeps going after a failure so all three are
    reported;
  - writes everything to `test-output.txt` in the repository root as **UTF-8**, and ends with a
    short summary: for each of the three steps "OK" or "FAILED";
  - takes an optional `-Ignored` switch that adds `-- --ignored` to the test step (for the
    real-file runs from T6–T14);
  - exits with code 0 only if all steps passed.
- **Rules:** no new tools or modules; plain PowerShell 5.1 (the version in Windows). Do not
  change any other file; `test-output.txt` is not committed.
- **Done when:** the owner runs `.\tools\check.ps1` and gets a readable `test-output.txt` with the
  summary at the end.

### T16. Developer setup: document what the tests need — Done

- **Start after T6 and T15 are done.**
- **File:** `docs/dev-setup.md`.
- **What:** add a short "Running the tests" section:
  - in a worktree, `vcpkg_installed` is not shared; set `VCPKG_INSTALLED_DIR` to the main checkout's
    folder (one line for the current window, one for a permanent user variable);
  - ExifTool must be on `PATH` (or `PW_EXIFTOOL_PL` set), otherwise the `pw-metadata` integration
    tests fail on purpose;
  - `PW_TESTDATA` and `-- --ignored` for the real-file tests (T6–T14);
  - `.\tools\check.ps1` (T15) runs all checks.
- **Rules:** only document what exists and has been verified (TESTING.md §12). Keep the existing
  sections.
- **Done when:** the section is there and every command in it has been run once by the owner.

### T17. Test inventory — Done

- **Start after T12–T14 are done.**
- **File:** `docs/test-inventory.md` (new).
- **What:** one table with a row per test file (unit-test modules counted per crate): crate, file,
  what it verifies (one line), category from TESTING.md §4 (unit, integration, real-file,
  external tool), prerequisites (none / ExifTool / `PW_TESTDATA` / vcpkg libraries / Windows),
  number of tests, and how many are ignored by default.
- **Rules:** describe only tests that exist; read each file, do not guess from names. Do not
  claim a requirement is verified (TESTING.md §7).
- **Done when:** the numbers match a `cargo test --workspace` run (the owner checks the totals).

### T18. pw-ingest: split `paths.rs` and restore the explanations — Done

- **Crate:** `pw-ingest`. **Files:** `crates/pw-ingest/src/paths.rs`, new files next to it,
  `crates/pw-ingest/src/lib.rs`.
- **What:** `paths.rs` now holds three unrelated things. Split it, moving tests with their
  functions:
  - `paths.rs`: `inside`, `resolve`;
  - `eject.rs`: `drive_of`, `eject`, `eject_script`, `wait_until_gone`, `retry_until_gone`;
  - `summary.rs`: `folder_summary`.
  `lib.rs` keeps exporting the same public names.
- **Restore the fuller doc comments** that were shortened in T8, for example that `inside()`
  expects paths that went through `resolve()`, and why `resolve()` canonicalises (`\\?\` prefixes,
  `subst` drives, folders that do not exist yet). Keep the comment on `eject_script` about why
  PowerShell must stay alive.
- **Rules:** a move only: no change in behaviour, signatures, messages or test assertions; keep the
  `#[cfg(any(windows, test))]` markings.
- **Done when:** `cargo test -p pw-ingest -p photoworkflow` passes with the same test counts as
  before and clippy is clean.

## Not for agents yet

These need design decisions, new dependencies or platform-specific code, and stay with the owner
and reviewer:

- A timeout when ExifTool hangs (process supervision across threads).
- Suppressing the Windows "no disk in drive" dialog when scanning empty card readers (needs a
  Windows API call and a new dependency).
- Anything from ADR 0007 (IPTC/IIM/Exif metadata): the ADR is not approved yet.
- The import dialog fields for title and description in `ui/` (frontend).
- ADR 0010 steps M3 (approving the folder-dialog crate), M5 (drawing the import dialog) and M6
  (acceptance by hand) stay with the owner and reviewer for now. M4 and M7 are added to this list
  when M1 and M2 are done.

## Log

| Date | Task | Status | Note |
|---|---|---|---|
| 2026-11-08 | – | – | List created |
| 2026-11-08 | T1 | Done | Reviewed: exact clippy suggestions, no behaviour change. Tests: photoworkflow 10, pw-rawview engine 24 + libraw_engine 3, all passed; clippy without lint warnings. Agent did not report test output or update the status itself |
| 2026-11-08 | T7 | Open | Added: pre-existing formatting differences found during the T1 review |
| 2026-11-08 | T2 | Done | Reviewed: `pw-metadata.workspace = true`, Cargo.lock unchanged. Tests: pw-loupe 6 + 2 passed |
| 2026-11-08 | T7 | Done | Reviewed: only the three named files changed, layout only; `cargo fmt --all --check` clean. Tests: photoworkflow 10 passed; clippy without lint warnings. Agent again did not report test output or update the status |
| 2026-11-08 | T3 | Done | Reviewed: is_loading/is_error read LoadState; no text comparisons left in main.rs; all required tests present. Tests: pw-loupe 6 + 3 passed; fmt and clippy clean. Agent updated the status itself this time |
| 2026-11-08 | T4 | Done | Reviewed: folder_summary() is pure, existence check in run(); three required tests present. Tests: photoworkflow 13 passed; fmt and clippy clean |
| 2026-11-08 | T5, T8 | Open | Plan changed (owner): reusable ingest logic moves to pw-ingest for the egui GUI; T8 added, T5 now moves eject() to pw-ingest after T8 |
| 2026-11-08 | T8 | Ready for review | Moved reusable ingest path and folder summary helpers with their tests to pw-ingest; pre/post crate test counts preserved. |
| 2026-11-08 | T8 | Done | Reviewed: pure move to `pw-ingest/src/paths.rs`, functions identical, src-tauri calls them. Tests: photoworkflow 5 + pw-ingest 12 (was 13 + 4), integration 8; fmt and clippy clean. Nit: doc comments shorter than the originals |
| 2026-11-08 | T5 | Ready for review | Moved eject() to pw-ingest, use full-path PowerShell and poll DCIM for up to 5 s; added three polling tests. Tests: pw-ingest 23 (was 20), photoworkflow 5 (was 5); fmt and clippy clean. Manual eject verification remains with the owner. |
| 2026-11-08 | T5 | Done | Reviewed: eject() moved to pw-ingest, full-path PowerShell, exit status checked, polling up to 5 s with three tests. Tests: pw-ingest 15 + 8, photoworkflow 5; fmt and clippy clean. Owner still tests a real eject by hand. Follow-ups in T9 |
| 2026-11-08 | T9 | Open | Added: small follow-ups from the T5 review |
| 2026-11-08 | T5 | Manual test | Import and path checks work. Eject right after import failed; a manual eject a moment later worked with the app open. T10 added |
| 2026-11-08 | T10 | Open | Added: retry the eject when Windows is still busy |
| 2026-11-08 | – | Note | Owner test: "Öppna i visaren" opens the imported folder correctly. Requirement noted for the egui import: open the imported folder automatically in a new tab |
| 2026-11-08 | T9 | Ready for review | Display-formatted PowerShell exit status; parameterized eject polling with named production timing constants and fast test timings; wait helper is Windows/test-only. Tests: pw-ingest 15 unit + 8 integration passed; fmt and clippy clean. |
| 2026-11-08 | T9 | Done | Reviewed: all three points done; reviewer added `#[cfg(any(windows, test))]` on the `std::time` import, which was unused off Windows. Tests: pw-ingest 15 (0.02 s, was 5 s) + 8, photoworkflow 5; fmt and clippy clean |
| 2026-11-08 | T10 | Ready for review | Added three-attempt eject retry and four fast unit tests. Tests: pw-ingest 19 unit + 8 integration passed; fmt and clippy clean. |
| 2026-11-08 | T10 | Done (manual test pending) | Reviewed: retry_until_gone with 3 attempts and four tests; pw-ingest 19 unit tests pass. Card test: still not ejected ("Antal försök: 3") |
| 2026-11-08 | T11 | Open | Added: owner test showed PowerShell must stay alive after InvokeVerb('Eject') (A: exits at once → not ejected; B: Start-Sleep 5 inside → ejected) |
| 2026-11-08 | T11 | Ready for review | Eject PowerShell script now waits for DCIM to disappear for up to 5 s; added a cross-platform script test. |
| 2026-11-08 | T11 | Done | Reviewed: eject_script() keeps PowerShell alive until DCIM is gone (max 5 s); reviewer added `#[cfg(any(windows, test))]` and a doc comment. Tests: pw-ingest 20 + 8, photoworkflow 5; fmt and clippy clean |
| 2026-11-08 | T5, T10, T11 | Done | Owner card test: import with eject now ejects the card |
| 2026-11-08 | T6 | Open | Owner decision: real-file tests are ignored by default and fail when run with `--ignored` without the files. T6 extended; T12–T14 cover the other crates |
| 2026-11-08 | T15–T18 | Open | Added: check script, test documentation, test inventory, pw-ingest module split |
| 2026-11-08 | T15 | Ready for review | Added `tools/check.ps1`; formatting and clippy passed. Workspace tests fail in `pw-libraw`'s `truncated_files_are_damaged` test with a path-not-found error while writing a temporary file. |
| 2026-11-08 | T15 | Done | Reviewed: all requirements met. Reviewer added `--no-fail-fast` to the test step (cargo stopped at the first failing crate) and `/test-output.txt` to `.gitignore`. Full run with `PW_TESTDATA` set: fmt, test and clippy OK |
| 2026-11-08 | – | Fix | With `PW_TESTDATA` set, `pw-libraw` `truncated_files_are_damaged` failed: two tests shared one scratch folder and one removed it. Reviewer gave each test its own folder (test code only); now passes |
| 2026-11-08 | T6, T12 | Done | Reviewed: same pattern in all five files; 10 tests ignored by default. `check.ps1 -Ignored` with `PW_TESTDATA` set: all 10 pass (pw-raw 4, pw-viewcore 3, pw-ingest 1, pw-libjpeg 1, pw-metadata 1). The earlier worry about pw-raw (0.00 s) was wrong: it only reads file headers, which is fast. The run without `PW_TESTDATA` was not reported |
| 2026-11-08 | T13 | Ready for review | `pw-libraw`: 10 real-file tests ignored by default; explicit run without `PW_TESTDATA` fails with a clear message; all 10 pass with the owner test data. Formatting and clippy clean. |
| 2026-11-08 | T14 | Ready for review | `pw-rawview` LibRaw: all 3 real-file tests ignored by default; explicit release run without `PW_TESTDATA` fails with a clear message; all 3 pass with the owner test data. Formatting and clippy clean. |
| 2026-11-08 | T18 | Ready for review | Split `pw-ingest` paths, eject, and folder-summary helpers into separate modules; restored path-resolution explanations. `cargo test -p pw-ingest -p photoworkflow`: pw-ingest 20 unit + 7 integration passed (1 ignored), photoworkflow 5 passed; fmt and clippy clean. |
| 2026-11-08 | T19 | Open | Added: the "not a RAW file" test needs no real files |
| 2026-11-08 | T13, T14 | Done | Reviewed: same pattern; helpers now pick only RAW files (.arw/.rw2/.dng). Owner's `check.ps1 -Ignored`: all 23 real-file tests pass (pw-libraw 10, pw-rawview 3, plus the 10 from T6/T12); fmt and clippy OK |
| 2026-11-08 | T18 | Done | Reviewed: pure split into paths.rs, eject.rs, summary.rs; same exports and 20 unit tests; fuller doc comments restored. Reviewer moved the `PathBuf` import into the Windows-only block of eject.rs (unused elsewhere) |
| 2026-11-08 | T19 | Ready for review | Removed the ignored marker and unnecessary PW_TESTDATA check from the synthetic non-RAW test. `cargo test -p pw-viewcore`: 43 unit + 12 engine tests pass (the native_files target is feature-gated); with `--features native`, the named test passes. Fmt and clippy clean. |
| 2026-11-08 | T16 | Ready for review | Added worktree vcpkg setup, ExifTool requirements, PW_TESTDATA/ignored test instructions, and the check-script commands. |
| 2026-11-08 | T17 | Ready for review | Added `docs/test-inventory.md` from the inspected Rust test modules and files; counts match the `cargo test --workspace` run (180 passed, 23 ignored, 0 failed). |
| 2026-11-08 | T19, T16, T17 | Done | Reviewed. T19: test now runs in every run (native_files 1 passed, 2 ignored). T16: section added; reviewer replaced the internal task numbers with a pointer to the test inventory. T17: inventory matches the owner's run (180 passed, 23 ignored, 0 failed; 22 real-file tests plus the pw-metadata helper) |
| 2026-11-08 | – | Decision | Owner accepted ADR 0010 (import and folder tabs in pw-loupe) with all proposals for Q1–Q6; GR12 added to GUI_REQUIREMENTS |
| 2026-11-08 | T20, T21 | Open | Added: ADR 0010 steps M1 (shared import logic in pw-ingest) and M2 (folder tabs) |
