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
6. **Before you report back,** run `.\tools\check.ps1` and keep `test-output.txt` from that run
   (it must be newer than your last change). Do not run `cargo test -p <crate>` or
   `cargo clippy -p <crate>` on their own: a single-crate build uses a different feature set, which
   builds new build scripts that the owner's antivirus may block (seen as linker error LNK1104). While
   working you may run `cargo build --workspace` or `cargo test --workspace`.
7. **Build only in the project's own `target` folder.** Do not set `CARGO_TARGET_DIR` or
   `--target-dir`, and do not build in a temporary folder (for example under `%TEMP%`). The
   owner's antivirus has an exception for `target` only; builds elsewhere are quarantined.
   If the program is running and `target` is locked, ask the owner to close it.
8. **Do not commit.** Leave the changes uncommitted so they can be reviewed with `git diff`.
   Do not create checkpoint commits or refs in this repository.
9. **Report:** list every file changed, what changed and why, and paste the exact output of the
   summary of `test-output.txt` from step 6. Then mark the task "Ready for review" below.
10. **Do the whole task as written here.** Read the task in this file as it is in your working tree
    when you start, and do every numbered point under **What** and every test under **Tests to add**.
    If a point seems wrong, unclear or impossible, stop and ask; do not leave it out, shorten it or
    replace it with your own version.
11. **This file (`docs/agent-tasks.md`) is the owner's.** An agent may change exactly two things in it:
    - the status at the end of its own task's heading (for example `— Open` → `— Ready for review`);
    - one new row at the **end** of the Log table.

    Never rewrite, shorten or move task text (your own task included), never touch other tasks, the
    "Not for agents yet" list or earlier log rows, and never write the file from another copy or from
    memory. Edit it in place. Check with `git diff docs/agent-tasks.md` before you report: the diff
    must show only those two changes.
12. **Other documentation:** update `docs/test-inventory.md` (counts and the description of the
    crate's tests) when you add tests; it is the only other file under `docs/` an agent changes, unless
    the task names one.

## Status legend

| Status | Meaning |
|---|---|
| Open | Not started, or sent back after a review (the log says what is left) |
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

### T20. ADR 0010 M1: move card detection and request validation to pw-ingest — Done

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

### T21. ADR 0010 M2: folder tabs in pw-loupe — Done

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
     **Changed 2026-11-08 (owner, after testing):** a dropped folder always opens in a new tab; an
     empty active tab is filled instead (ADR 0010 Q3).
- **Rules:** the loupe's existing keys and behaviour are unchanged (ADR 0009 §3.3); one view core for
  all tabs; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** the tab model (open in active, open new, open an already open folder, next and
  previous with wrap, close the last tab, close a middle tab and which tab becomes active, remembered
  image per tab, titles) and the tab keys.
- **Done when:** `.\tools\check.ps1` is all OK; the owner checks by hand that tabs can be opened with
  Ctrl+drag-and-drop, switched with the keys and the mouse, and closed, and that each tab comes back
  on its own image.

### T22. pw-ingest: tidy the import API before the egui dialog uses it — Done

- **Why:** T20 moved the logic correctly, but the egui dialog (ADR 0010 M4) will call these functions
  from Rust, so the API should be hard to misuse, and a few required tests are missing.
- **Crates:** `pw-ingest`, `photoworkflow`. **Files:** `crates/pw-ingest/src/volumes.rs`,
  `crates/pw-ingest/src/request.rs`, `crates/pw-ingest/src/lib.rs`, `src-tauri/src/ingest.rs`.
- **What:**
  1. **Card detection:** rename `pw_ingest::ingest_volumes()` to `card_volumes()` and return
     `Vec<PathBuf>`. Put the loop over candidate roots in a small function that takes the roots, e.g.
     `fn with_dcim(roots: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf>`, and test it with
     temporary folders (a root with `DCIM` is found, one without is not, a missing root is skipped).
     The Tauri command `ingest_volumes()` keeps its name and converts the paths to strings.
  2. **Validation input:** replace the seven positional arguments of `validate_options` with a plain
     struct, e.g. `pub struct OptionsInput<'a> { source, dest, backup: Option<&'a str>, folder, name,
     job, first_seq }`, so source and destination cannot be swapped by mistake. Messages unchanged.
  3. **Missing tests** from T20: empty destination ("Välj ett mål."); source inside the destination;
     a bad folder template starts with "Mappmall: " and a bad file-name template with
     "Filnamnsmall: "; a valid input with a backup folder.
- **Rules:** behaviour and messages unchanged; no new dependencies; nothing else in `src-tauri` changes.
- **Done when:** `.\tools\check.ps1` is all OK and the new tests pass.

### T23. ADR 0011 K1: thumbnail extraction in pw-thumbcache — Done

- **Crate:** `pw-thumbcache`. **Files:** `crates/pw-thumbcache/Cargo.toml` (add the internal
  dependency `pw-libjpeg.workspace = true`; `pw-raw` is already there), `crates/pw-thumbcache/src/`
  (e.g. a new `extract.rs`, declared in `lib.rs`).
- **What (ADR 0011 §3.1, §4.1):**
  1. `pub fn choose_preview(info: &pw_raw::ImageInfo, cell: u32) -> Option<&pw_raw::PreviewRef>`:
     the smallest embedded preview whose **long edge** is at least `cell`; if none is that large, the
     largest one; `None` if there is no preview.
  2. `pub fn choose_scale(width: u32, height: u32, cell: u32) -> pw_libjpeg::Scale`: the smallest
     decode (Eighth, then Quarter, then Half, then Full) whose long edge is still at least `cell`
     (use `Scale::size`).
  3. `pub fn orient(image: pw_libjpeg::Rgb8, orientation: u16) -> pw_libjpeg::Rgb8`: applies the EXIF
     orientation in pixels, all eight values (1 = as is, 2 = mirror, 3 = 180°, 4 = flip, 5 =
     transpose, 6 = 90° clockwise, 7 = transverse, 8 = 270° clockwise); unknown values act as 1.
  4. `pub fn thumbnail(path: &Path, cell: u32, decoder: &mut pw_libjpeg::Decoder) -> Result<Rgb8, ThumbError>`:
     `pw_raw::inspect`, then for a plain JPEG file (`Format::Jpeg`) read and decode the file itself,
     otherwise `choose_preview` + `pw_raw::read_preview`; decode with `choose_scale`; apply `orient`
     with the file's orientation.
     `ThumbError` is an enum with at least `NoPreview` (the "ingen förhandsvisning" case) and
     `Unreadable(String)` (I/O, format or decode errors), and implements `Display`.
- **Rules:** never decode RAW data (G1); no `unwrap`/`expect`/`panic!`/unchecked indexing in non-test
  code; no new external crates. Update the crate description in `Cargo.toml` to "Thumbnail
  extraction, background generation and an in-memory cache (disk cache later)".
- **Tests to add:**
  - `choose_preview`: several previews (picks the smallest large enough), none large enough (picks the
    largest), no previews (`None`), a portrait preview (long edge is the height);
  - `choose_scale`: e.g. 1616×1080 with cell 200 → Eighth (202 px), with cell 300 → Quarter;
    160×120 with cell 200 → Full; exact boundaries (corrected in review: the first example said
    Quarter by mistake);
  - `orient`: a small 2×3 image with distinct pixels, checked for all eight values (size and pixel
    positions);
  - `thumbnail`: a non-existent file → `Unreadable`; a file that is not an image → `Unreadable`, not a
    panic; a small JPEG written to a temporary file (you may reuse the small JPEG bytes of
    `crates/pw-libjpeg/tests/decode.rs`) → a thumbnail of the expected size;
  - real files (ignored by default, the T6 pattern): one thumbnail per camera format of the test set,
    with the oriented width/height checked for an A7R IVA vertical image (`TN100423.ARW`, orientation 8).
- **Done when:** `.\tools\check.ps1` and `.\tools\check.ps1 -Ignored` are all OK.

### T24. ADR 0011 K2: thumbnail worker and in-memory cache — Done

- **Start after T23 is done.**
- **Crate:** `pw-thumbcache`. **Files:** `crates/pw-thumbcache/src/` (e.g. `worker.rs`).
- **What (ADR 0011 §3.1, §4.1, §4.2):**
  - A worker with a small pool of background threads (2 by default) that turns requests into
    thumbnails with T23's `thumbnail`. The UI never waits: it sends requests and polls results
    (`try_recv`), as `pw-metadata`'s `MetadataWorker` does.
  - Requests carry a folder generation, an image index, the path and the cell size.
  - **Priority:** the UI can tell the worker which index range is visible (e.g. `set_visible(range)`);
    requests in that range are done first, then the rest. Requests from an older generation, and
    requests far outside the visible range, are dropped (`set_generation` / `cancel`).
  - **Pause:** `pause()` / `resume()`, used while the loupe is shown (§4.2).
  - **In-memory cache** of finished thumbnails for the current generation, with a byte limit; the
    least recently used are evicted first. A cache hit answers without decoding.
  - Errors (`ThumbError`) are results too, so the UI can show the placeholder.
  - To make it testable without files, the work is done through a small trait (e.g.
    `trait ThumbSource { fn make(&mut self, path: &Path, cell: u32) -> Result<Rgb8, ThumbError>; }`);
    the real one uses `thumbnail` with its own `Decoder` per thread.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; a poisoned lock is recovered or turned into
  an error; dropping the worker does not block the UI thread; no new external crates.
- **Tests to add** (with a simulated `ThumbSource`, no real files): every request gets exactly one
  result; visible requests are done before others; results of an older generation never arrive after
  `set_generation`; pause stops new work and resume continues it; the cache returns a hit without
  calling the source again; eviction respects the byte limit; an error result is delivered.
- **Done when:** `.\tools\check.ps1` is all OK.

### T25. ADR 0011 K3: grid model in pw-loupe — Done

- **Can be done in parallel with T23** (different crate).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/` (e.g. a new `grid.rs` declared in `lib.rs`),
  `crates/pw-loupe/src/tabs.rs`, `crates/pw-loupe/src/lib.rs`. **Not** `main.rs` (drawing is K4).
- **What (ADR 0011 §3.1, §11):**
  1. **Layout in plain Rust:** the number of columns for an available width, cell size and spacing
     (at least 1); the row of an index; the range of indexes visible for a scroll offset and view
     height, plus a margin of rows.
  2. **Navigation:** for a count, a column count and a current index: Left, Right, Up, Down (by one
     row), PageUp, PageDown (by the number of visible rows), Home, End; clamped at the edges, including
     a short last row (Down from the row above it goes to the last image).
  3. **View mode per tab:** add a mode (`Grid` or `Loupe`) to `Tab`. A newly opened folder starts in
     `Grid` (GR13); a tab keeps its mode when switching tabs (Q5).
  4. **Keys** as plain functions with tests:
     - in the grid: arrows, Page Up/Down, Home/End → navigation; **Enter, G, Space** → open the loupe;
       Escape → nothing (Q6);
     - in the loupe: **Escape, G, Enter** → back to the grid (ADR 0009 §3.3).
     `key_action` (the loupe's own keys) must not change, and `tab_action` stays as it is.
  5. **Thumbnail size:** clamp to 96–480 px, default 200 (Q2); a step function for Ctrl + mouse wheel.
- **Rules:** no egui drawing; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** columns for several widths; visible range at the top, middle and end; every
  navigation direction including both edges and a short last row; a new tab starts in `Grid`; the mode
  survives tab switching; the key functions for both modes; size clamping.
- **Done when:** `.\tools\check.ps1` is all OK.

### T26. ADR 0011 K4a: pw-thumbcache — skip duplicate requests, export the image type — Done

- **Crate:** `pw-thumbcache`. **Files:** `crates/pw-thumbcache/src/worker.rs`,
  `crates/pw-thumbcache/src/lib.rs`.
- **Why:** the grid (T28) asks again for every visible cell that has no thumbnail yet each time the
  visible range changes, because the worker drops requests far outside the visible range without a
  result (T24). That is simple and safe only if asking twice does not decode twice.
- **What:**
  1. In `ThumbWorker::request`: if a request with the same generation, index and cell is already
     **queued** or **being made** by a worker thread, ignore the new one. Keep a small set of the
     in-flight `(generation, index, cell)` in `State`; a worker adds the entry when it takes a job
     and removes it when the job is done (also when the result is thrown away).
     `set_generation`, `cancel` and drop clear the queue as today; in-flight entries of an old
     generation simply never match again.
  2. In `lib.rs`: add `pub use pw_libjpeg::Rgb8;`, so `pw-loupe` can name the image type without its
     own dependency on `pw-libjpeg`.
- **Rules:** behaviour otherwise unchanged; the existing tests keep their names and assertions; no
  `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add** (simulated source, as in the existing tests): the same request sent twice while it
  is queued gives one source call and one result; the same request sent again while it is being made
  (use the gated source) gives one source call and one result; the same index with another cell size
  is not a duplicate; after the result has arrived, asking again is answered from the cache (one
  source call).
- **Done when:** `.\tools\check.ps1` is all OK.

### T27. ADR 0011 K4b: thumbnail bookkeeping for the grid (plain Rust) — Done

- **Start after T26 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add `pw-thumbcache.workspace = true`
  to `[dependencies]`; approved by ADR 0011 §6), a new `crates/pw-loupe/src/thumbs.rs`, declared and
  re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **What:** a struct `ThumbGrid` that remembers, for the folder shown in the grid, which thumbnails
  are missing, asked for, ready or failed. It holds no images and no textures (the program keeps the
  textures; T28).
  - `ThumbGrid::new(generation: u64, count: usize)`.
  - `to_request(&self, range: Range<usize>, cell: u32) -> Vec<usize>`: the indexes in `range` (clamped
    to `count`) that are missing, or ready/asked for with a **different** cell size. Already asked for
    with the same cell is included too (T26 makes repeating safe, and it covers requests the worker
    dropped); ready with the same cell and failed are not.
  - `mark_requested(index, cell)`.
  - `apply(&mut self, event: &pw_thumbcache::ThumbnailEvent) -> bool`: records ready (with the cell
    size that was asked for) or failed (with the error text); returns `false` and changes nothing for
    an event of another generation or an index outside the folder.
  - `state(index) -> CellState` with `CellState::{Missing, Requested, Ready, Failed(String)}`, for
    drawing (failed shows the error placeholder; `ThumbError::NoPreview` shows
    "ingen förhandsvisning").
  - `forget_outside(&mut self, keep: Range<usize>) -> Vec<usize>`: every ready or failed index outside
    `keep` goes back to missing; returns those indexes, so the program can release their textures.
  - A free function `request_cell(size_points: u32, pixels_per_point: f32) -> u32`: the cell size in
    physical pixels, rounded **up** to a multiple of 128 (so moving the size slider decodes again
    only when it crosses a step), at least 128; a non-finite or non-positive `pixels_per_point`
    counts as 1.0.
  - A free function `color_image(image: &pw_thumbcache::Rgb8) -> Option<egui::ColorImage>`:
    `None` if the data length is not width × height × 3.
- **Rules:** no egui drawing; no `unwrap`/`expect`/`panic!`/unchecked indexing in non-test code.
- **Tests to add:** a new grid asks for every index in range; an index asked for with the same cell
  is asked for again, a ready one is not; a ready one with another cell is asked for again; a failed
  one is not; an event of an old generation is ignored; an index outside the folder is ignored;
  `forget_outside` returns and resets exactly the indexes outside `keep`; `request_cell` for 200 pt
  at 1.0 → 256, at 1.5 → 384, 96 pt at 1.0 → 128, and a bad `pixels_per_point`; `color_image` for a
  2×1 image and for wrong data length.
- **Done when:** `.\tools\check.ps1` is all OK.

### T28. ADR 0011 K4c: draw the contact sheet — Done

- **Start after T27 is done.**
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only.
- **What:** when the active tab's `mode` is `ViewMode::Grid`, the image area shows the contact sheet
  instead of the loupe. Switching between the modes is T29; in this task a tab is shown in the grid
  as soon as it is opened (that is already the default of `Tab`), and the loupe code path stays as
  it is for tabs in `Loupe` mode.
  - Start a `pw_thumbcache::ThumbWorker` in `run()` next to the metadata worker, and keep a
    `ThumbGrid`, a generation counter and a `HashMap<usize, egui::TextureHandle>` in `Loupe`.
    Every time a tab is loaded (`load_active_tab`), increase the generation, call
    `set_generation`, create a new `ThumbGrid` and clear the textures.
  - Draw the grid in an `egui::ScrollArea::vertical()` with `show_viewport`: content height =
    rows × (cell + spacing); spacing 8 pt; use `columns`, `visible_range` (margin 1 row) from T25.
    The cell size comes from a field `thumb_size: u32` in `Loupe`, starting at
    `DEFAULT_THUMBNAIL_SIZE`.
  - Each frame: `set_visible` with the visible range, then for `to_request(range, cell)` send a
    `ThumbnailRequest` (path from the tab's file list, `cell = request_cell(thumb_size, ppp)`) and
    call `mark_requested`; poll `try_recv` until empty, `apply` each event and turn a ready image
    into a texture with `color_image` + `ctx.load_texture` (linear filtering); call
    `forget_outside` with the visible range plus 2 rows and remove those textures.
  - A cell: the thumbnail scaled to fit the square, centred, keeping its proportions; a grey
    placeholder while missing or asked for; the text "ingen förhandsvisning" for `NoPreview` and a
    short error text for other failures; the file name under the cell, shortened when too long
    (egui `Label` with `truncate()` or a clipped painter text). The current image (`tab.current`)
    gets a clear frame.
  - Clicking a cell makes it current (`Tabs::set_current`).
  - Keys in the grid: `grid_key_action` + `navigate` (visible rows for Page Up/Down = view height ÷
    (cell + spacing), at least 1); after a key move, scroll so the current cell is visible
    (`ui.scroll_to_rect`). `OpenLoupe` does nothing yet (T29). The tab keys keep working; the loupe's
    `key_action` is **not** used while the grid is shown.
  - Status line in the grid: file name of the current image, "position / count", and the message as
    today.
  - Ask for a repaint while thumbnails are still asked for (e.g. `request_repaint_after` 50 ms), so
    results are shown without moving the mouse.
- **Rules:** no file I/O or decoding on the UI thread (G2); no `unwrap`/`expect`/`panic!`; no new
  dependencies; the loupe behaviour for tabs in `Loupe` mode is unchanged.
- **Tests:** the logic is in T25 and T27; this task adds none unless a new plain function is needed
  (put it in the library with a test).
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: drop the test folder → a
  grid appears at once with placeholders that fill in; arrows, Page Up/Down, Home/End move the frame
  and scroll; clicking selects; a vertical A7R IVA image stands upright.

### T29. ADR 0011 K4d: switch between grid and loupe, size control — Done

- **Start after T28 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` only
  if a small plain function is needed (with a test).
- **What:**
  1. **Grid → loupe:** Enter, G, Space (`GridKeyAction::OpenLoupe`) or a double-click on a cell:
     `set_mode(ViewMode::Loupe)`, then `core.open_folder(files)` and `core.go(current)` (through the
     metadata cache as `load_active_tab` does today), and `ThumbWorker::pause()`.
  2. **Loupe → grid:** Escape, G, Enter (`loupe_key_action`): `set_mode(ViewMode::Grid)`; the image
     shown in the loupe becomes the current cell (`tab.current` is already kept up to date by the
     loupe); `resume()`; scroll so that cell is visible. These three keys are checked **before**
     `key_action` in loupe mode (they are reserved there and do nothing today).
  3. **The view core only gets the folder in the loupe (ADR 0011 §4.2):** when a tab in `Grid` mode is
     loaded, call `core.open_folder(Vec::new())` instead of giving it the files, so the loupe does not
     decode in the background; a tab in `Loupe` mode is loaded as today. Switching tabs therefore
     shows each tab in its own mode (Q5).
  4. **Command line (Q1):** a folder opens in the grid; a single file opens the loupe on that file
     (set `ViewMode::Loupe` after opening when the path is a file).
  5. **Size:** a slider (96–480, `clamp_thumbnail_size`) in the status line while the grid is shown,
     and Ctrl + mouse wheel over the grid with `step_thumbnail_size` (egui turns Ctrl + wheel into
     `zoom_delta()`; use its direction, and make sure the scroll area does not scroll at the same
     time). The size is kept for the session in `Loupe`, shared by all tabs.
  6. The display-mode button of the loupe's status line is only shown in loupe mode.
- **Rules:** as T28; the loupe's own keys, mouse and zoom behaviour are unchanged in loupe mode.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0011 §8.2): Enter
  and double-click open the loupe on the right image; Escape returns with the same image framed; two
  tabs keep their own mode; the slider and Ctrl + wheel change the size; `pw-loupe <file>` opens the
  loupe and `pw-loupe <folder>` the grid.

### T30. ADR 0010 M4a: import form and preview summary (plain Rust) — Done

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add `pw-ingest.workspace = true` to
  `[dependencies]`; internal, approved by ADR 0010 §6), a new `crates/pw-loupe/src/import.rs`, declared
  and re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`. Do **not** add `rfd` yet (M5).
- **Why:** the egui import dialog (step A of ADR 0010) needs the same rules and messages as the Tauri
  dialog. They come from `pw-ingest`; this task wraps them in a small, testable form model.
- **What:**
  1. `pub struct ImportForm { pub source: String, pub dest: String, pub folder: String, pub name: String }`
     with `Default`: empty source and destination, folder `{year}/{date} {job}`, name `{name}` (ADR 0005
     choice 1, the Tauri defaults). Step B fields (backup, job, running number) are not in the form;
     validation passes backup `None`, job `""` and first number `1`.
  2. `pub enum Field { Source, Dest, Folder, Name }` and
     `pub struct FieldError { pub field: Field, pub message: String }`.
  3. `ImportForm::validate(&self) -> Result<Validated, FieldError>` where
     `pub struct Validated { pub source: PathBuf, pub options: pw_ingest::Options }`. Decide the field
     **by which check fails, never by reading the message text**:
     - `pw_ingest::validate_source(&self.source)` fails → `Field::Source`;
     - `pw_ingest::Template::parse(&self.folder)` fails → `Field::Folder`, message
       `"Mappmall: {error}"`;
     - `pw_ingest::Template::parse(&self.name)` fails → `Field::Name`, message `"Filnamnsmall: {error}"`;
     - then `pw_ingest::validate_options(OptionsInput { … })`; any error left → `Field::Dest`.
     The messages are exactly those of `pw-ingest`.
  4. `pub struct Preview` built by `Preview::new(shots: &[pw_ingest::Shot], plan: &pw_ingest::Plan)`,
     with the same numbers as the Tauri `preview_of` in `src-tauri/src/ingest.rs`: `shots`,
     `files_to_copy`, `bytes_to_copy`, `present`, `renamed` (renamed for a conflict **and** to be
     copied), `no_exif`, `items_total`, and `lines`: the first 20 plan items as
     `PreviewLine { source_name: String, rel: String, action: ItemAction }`, with
     `pub enum ItemAction { Copy, Renamed, Present, BackupOnly }` following the Tauri `action()` rules.
     (Do not change `src-tauri`.)
  5. `pub fn result_folder(copied: usize, folders: &[(String, usize)]) -> Option<PathBuf>`: the folder
     the import opens in a new tab (GR12, ADR 0010 Q2): `None` when nothing was copied, otherwise the
     first entry of `folders` (the output of `pw_ingest::folder_summary`, largest first).
- **Rules:** no threads, no file writes; no `unwrap`/`expect`/`panic!` in non-test code; no new external
  crates.
- **Tests to add:** the defaults; an empty source → `Source` with "Välj en källa."; an empty destination
  → `Dest` with "Välj ett mål."; a destination inside the source → `Dest`; a bad folder template →
  `Folder` starting with "Mappmall: "; a bad name template → `Name` starting with "Filnamnsmall: "; a
  valid form (temporary source folder) → `Ok` with the destination and templates; `Preview::new` on a
  hand-made plan (items for each action, a renamed item that is not copied, a shot without EXIF time,
  more than 20 items); `result_folder` for 0 copied, and for several folders.
- **Done when:** `.\tools\check.ps1` is all OK.

### T31. ADR 0010 M4b: import worker (background thread) — Done

- **Start after T30 is done.**
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/import_worker.rs`, declared and
  re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **What:** a worker thread that does the slow parts of the import, so the UI never waits (G2, GR2),
  built like `pw-metadata`'s `MetadataWorker` and `pw-thumbcache`'s `ThumbWorker`.
  - `ImportWorker::start() -> io::Result<Self>`; requests go through a **bounded** channel; a full queue
    or a stopped worker is reported as an error, never a panic.
  - Requests, each with a `RequestId` (a counter):
    - `preview(source: PathBuf, options: Options, rescan: bool) -> Result<RequestId, SubmitError>`:
      scan (reusing the last scan when the source is the same and `rescan` is false, as the Tauri
      `IngestState::shots` does), plan, and answer with `ImportEvent::Previewed { id, result:
      Result<Preview, String> }`. A scan error uses the Tauri message `"Kunde inte läsa {source}: {error}"`.
    - `run(source: PathBuf, options: Options) -> Result<RequestId, SubmitError>`: **always scan again
      and plan again** (the card may have been swapped, the destination may have changed; see the
      comments in the Tauri `run`), then `pw_ingest::execute` with progress. Progress is sent as
      `ImportEvent::Progress { id, progress: pw_ingest::Progress }`; at the end
      `ImportEvent::Done { id, done: ImportDone }` with `copied`, `present`, `bytes`, `failed:
      Vec<String>` (`"{file name}: {error}"`), `cancelled`, `elapsed: Duration`, `folders` (from
      `pw_ingest::folder_summary` on the plan items that exist in the destination, as the Tauri `run`
      does) and `error: Option<String>` (scan error).
    - `cancel()`: sets the cancel flag that `execute` checks; the flag is cleared when a new `run` starts.
  - `try_recv() -> Result<ImportEvent, TryRecvError>` for the UI to poll.
  - Dropping the worker cancels a running copy and does not block the UI thread (do not join).
- **Rules:** step A only: no backup, IPTC or eject here (step B, M7); the ADR 0005 guarantees stay
  (`pw-ingest` does the copying); no `unwrap`/`expect`/`panic!` in non-test code; a poisoned lock is
  recovered; no new crates.
- **Tests to add** (temporary folders; a small "card" with a few files in `DCIM/100TEST`, as in
  `crates/pw-ingest/tests/ingest.rs`): a preview gives the expected counts; a second preview with
  `rescan = false` reuses the scan (add a file in between: it is not counted) and with `rescan = true`
  sees it; a run copies the files, sends progress and a `Done` with `copied`, `folders` and no error; a
  run into a destination that already holds the files reports them as present; a missing source gives
  a preview error with the Tauri message; `cancel` before a run of many files ends with `cancelled` and
  no final-named partial files; dropping the worker during a run returns at once.
- **Done when:** `.\tools\check.ps1` is all OK.

### T32. ADR 0010 M4c: import dialog state machine (plain Rust) — Done

- **Start after T31 is done.**
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/import_dialog.rs`, declared and
  re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **What (ADR 0010 §4.2):** `ImportDialog` holds an `ImportForm` and the state, and never touches files
  or threads itself. It returns the requests to send and takes the worker's events, so every transition
  is a plain function that is tested without a window or a worker.
  - States: `Editing { preview: Option<Preview>, error: Option<FieldError> }`, `Previewing`,
    `Running { progress: Option<Progress> }`, `Done { done: ImportDone }`.
  - `edit(&mut self, change: impl FnOnce(&mut ImportForm), now: Instant)`: changes the form and marks
    it as changed at `now`; remembers whether the **source** changed (a new scan is needed) or only the
    other fields (the scan can be reused).
  - `tick(&mut self, now: Instant) -> Option<DialogRequest>`: when the form changed at least 300 ms ago
    and nothing is running: validate; on an error go to `Editing` with that error and no request; when
    valid return `DialogRequest::Preview { source, options, rescan }` and go to `Previewing`.
  - `start(&mut self) -> Option<DialogRequest>`: only in `Editing` with a preview that has
    `files_to_copy > 0` and no error → `DialogRequest::Run { source, options }`, state `Running`.
  - `sent(&mut self, id: RequestId)` records the id of the request just sent; `apply(&mut self, event:
    &ImportEvent)` ignores events of any other id (an older preview, as in the loupe's metadata cache):
    `Previewed` → `Editing` with the preview or with the error on `Field::Source`; `Progress` → updates
    `Running`; `Done` → `Done`.
  - `disconnected(&mut self)`: if the worker is gone, `Previewing` and `Running` become an error
    (`Editing` with a `Source` error "Importen avbröts: arbetaren har stoppats.", or `Done` with that
    `error`), never "running" forever.
  - `cancel(&self) -> bool` (true while `Running`, so the UI calls `ImportWorker::cancel`), `can_close()`
    (false while `Running`), `back_to_editing()` from `Done` (keeps the form, clears the preview and
    marks the form as changed so a fresh preview follows), and `result_folder()` from `Done` using
    T30's `result_folder`.
- **Rules:** no egui drawing; no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add:** no request before 300 ms, one after; an invalid form gives its field error and no
  request; a source change asks for `rescan = true`, a template change for `rescan = false`; an old
  preview event is ignored; `start` is refused without a preview, with 0 files to copy, and while
  previewing; progress and done move the state; `disconnected` while previewing and while running;
  `can_close` is false while running; `result_folder` is `None` when nothing was copied and the
  largest folder otherwise; `back_to_editing` asks for a new preview.
- **Done when:** `.\tools\check.ps1` is all OK.

### T33. ADR 0010 M4d: validate the import form on the worker thread (G2) — Done

- **Start after T32 is committed.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import_worker.rs`,
  `crates/pw-loupe/src/import_dialog.rs`, `crates/pw-loupe/src/lib.rs` (re-exports only). **Not**
  `main.rs`, not `import.rs`.
- **Why (found in the T32 review):** `ImportForm::validate` checks that the source exists and
  canonicalises the source and destination (`pw_ingest::resolve`). That is file I/O. T32 calls it in
  `tick()` and `start()`, which run on the UI thread. On a sleeping hard disk (the F:, G: and H:
  destinations), a card reader without a card or a network folder this can block for seconds, and the
  window freezes while the user types. ADR 0010 §5 and G2: no file I/O on the UI thread. The Tauri app
  validates inside its background task too.
- **What:**
  1. **Worker:** `ImportWorker::preview(form: ImportForm, rescan: bool)` and `ImportWorker::run(form:
     ImportForm)` take the form instead of `source` and `options`; the worker thread calls
     `form.validate()` first.
     - `ImportEvent::Previewed { id, result: Result<Preview, FieldError> }`: a validation error is
       returned as it is; a scan error becomes `Field::Source` with the existing message
       (`"Kunde inte läsa {source}: {error}"`).
     - `run` with an invalid form ends with `ImportEvent::Done` whose `error` is the field error's
       message and nothing copied.
     - Everything else (scan cache, fresh scan for a run, progress, cancel, the gate, drop) is unchanged.
  2. **Dialog:** `DialogRequest::Preview { form: ImportForm, rescan: bool }` and
     `DialogRequest::Run { form: ImportForm }`.
     - `tick()` no longer validates: when the form changed at least 300 ms ago and the state is
       `Editing`, it goes to `Previewing` and returns the request with a copy of the form.
     - `apply(Previewed { result: Err(field_error) })` → `Editing` with that error (any field).
     - `start()` no longer validates: it needs `Editing` with a preview that has `files_to_copy > 0` and
       no error, and returns `Run` with a copy of the form.
     - The debounce, `rescan`, request ids, `disconnected`, `cancel`, `can_close`, `back_to_editing`
       and `result_folder` stay as they are.
- **Tests:** this task changes the API that some T31 and T32 tests call. Update **only those calls**
  and keep what each test verifies. The one test whose behaviour moves, T32's
  `invalid_form_shows_field_error_without_a_request`, becomes: the invalid form gives a preview request,
  and applying `Previewed` with a `Source` field error shows that error. Add:
  - worker: an invalid form (empty destination) gives `Previewed` with `Field::Dest` and "Välj ett
    mål."; `run` with an invalid form gives `Done` with that message and `copied == 0`;
  - dialog: `tick()` and `start()` work with a form whose source does not exist (no file access in the
    dialog); a field error for `Field::Folder` from the worker is shown on that field.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; no new dependencies; rule 7 (build only in
  `target`).
- **Done when:** `.\tools\check.ps1` is all OK.

### T34. ADR 0010 M5a: card detection on the worker, dialog keys and texts (plain Rust) — Done

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add `pw-core.workspace = true`;
  internal), `crates/pw-loupe/src/import_worker.rs`, `crates/pw-loupe/src/import_dialog.rs`, a new
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **Why:** M5 draws the import dialog (T35, T36). Everything that can be tested without a window is
  done first, here.
- **What:**
  1. **Card detection on the worker** (it checks drives A–Z, which is file I/O): `ImportWorker::cards()
     -> Result<RequestId, SubmitError>`; the worker answers `ImportEvent::Cards { id, cards: Vec<PathBuf> }`
     from `pw_ingest::card_volumes()`.
  2. **Dialog additions** in `ImportDialog`: `can_start(&self) -> bool` (true exactly when `start()`
     would return a request, without changing anything) and `rescan(&mut self, now: Instant)` (marks the
     form as changed with a new scan, for a "Sök igen" button; ignored while running).
  3. **Keys** (plain functions):
     - `opens_import(key, ctrl, shift, alt) -> bool`: true only for **Ctrl+I** (no Shift, no Alt). The
       loupe's `I` (info) without Ctrl is unchanged.
     - `dialog_key_action(key, ctrl, alt) -> Option<DialogKey>` with `DialogKey::{Close, Start}`:
       Escape → `Close`, Enter → `Start`; nothing with Ctrl or Alt.
  4. **Swedish texts**, the same as the Tauri dialog (`ui/web/ingest.js`, `igRenderPreview`,
     `igOnProgress`, `igOnDone`), with `pw_core::format_bytes` for sizes:
     - `preview_summary(&Preview) -> String`: "Inga bilder hittades i källan." when there are no shots;
       otherwise "{shots} bilder · {files} filer att kopiera ({bytes})", followed, when not zero, by
       "{n} finns redan", "{n} får nytt namn pga namnkrock" and "{n} utan EXIF-datum (fildatum används)",
       all joined with " · ".
     - `preview_line(&PreviewLine) -> String`: "{source}  →  {rel}" plus "  (namnkrock)",
       "  (finns redan)" or "  (bara backup)" for those actions; `more_lines(&Preview) -> Option<String>`:
       "… och {n} till" when `items_total` is larger than the lines shown.
     - `progress_text(&pw_ingest::Progress) -> String`: "Kopierar och verifierar {done} / {total} ·
       {bytes done} av {bytes total} · {file name of current}".
     - `done_lines(&ImportDone) -> Vec<DoneLine>` with `DoneLine { text: String, error: bool }`: with an
       `error`, only that line (as an error). Otherwise "Klar: " or "Avbruten: " + "{copied} filer kopierade
       och verifierade ({bytes} på {seconds:.1} s · {MB/s} MB/s)" (the speed part only when the time is
       above zero; MB/s = bytes / 1 000 000 / seconds, no decimals) + ", {n} fanns redan" when not zero +
       "."; then up to 20 "Misslyckades: {failure}" error lines and "… och {n} till"; then, when there is
       more than one folder, "Mappar: {folder} ({n}), …".
- **Rules:** no drawing; no `unwrap`/`expect`/`panic!` in non-test code; no new external crates; rules 6
  and 7.
- **Tests to add:** `cards()` answers with its id (the list itself depends on the machine); `can_start`
  in each state (and that it changes nothing); `rescan` asks for `rescan = true` and is ignored while
  running; the key functions (Ctrl+I yes; I, Ctrl+Shift+I, Alt+I no; Escape and Enter; with Ctrl
  nothing); every text function, including the zero cases, the 20-failure limit and one versus several
  folders.
- **Done when:** `.\tools\check.ps1` is all OK.

### T35. ADR 0010 M5b: draw the import dialog (editing and preview) — Done

- **Start after T34 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add
  `rfd = { version = "=0.17.2", default-features = false }`, approved 2026-11-09, GUI_REQUIREMENTS §6.1),
  `crates/pw-loupe/src/main.rs`. `Cargo.lock` may change **only** by listing `rfd` among `pw-loupe`'s
  dependencies; `rfd` 0.17.2 is already in it. Check with `git diff Cargo.lock` and report it.
- **What (ADR 0010 §3.1 step A, without the copy):**
  - Start an `ImportWorker` in `run()` next to the other workers. `Loupe` gets the worker, an
    `Option<ImportDialog>` (open or not), `last_form: ImportForm` (the settings of this session, Q5) and
    the detected cards.
  - **Open:** Ctrl+I (`opens_import`) or an "Importera…" button at the end of the tab bar:
    `ImportDialog::new(last_form.clone())` and ask the worker for `cards()`. **Close:** Escape
    (`dialog_key_action`) or a "Stäng" button, only when `can_close()`; the form is then saved in
    `last_form`. While the dialog is open, keys go to the dialog only (not to the grid, the loupe or the
    tabs); text fields keep their normal keys.
  - **Window:** an `egui::Window` titled "Importera", centred, not collapsible. In it, in this order:
    - **Källa:** a text field, a "Välj…" button (`rfd::FileDialog::new().set_title("Välj källa")
      .pick_folder()`; add `.set_parent(…)` with the eframe window if it compiles, otherwise leave it
      out), and one button per detected card (its drive, for example `E:\`) that sets the source, plus
      "Sök igen" (`rescan`) next to the preview.
    - **Mål:** a text field and "Välj…" (title "Välj mål").
    - **Mappmall** and **Filnamnsmall:** text fields.
    - Under each field, its `FieldError` in red when there is one.
    - **Preview:** "Förhandsvisar …" while previewing; otherwise `preview_summary`, then the lines
      (`preview_line`) in a small scroll area and `more_lines`.
  - **Each frame while open:** apply text changes with `edit(…, Instant::now())`; call `tick()` and send
    a request with `preview()`, then `sent(id)`; a `SubmitError` is shown as an error line in the dialog.
    Poll the worker until empty: `Cards` updates the card list, other events go to `apply`;
    `Disconnected` calls `disconnected()`. Ask for a repaint (about 100 ms) while a preview is pending.
  - No start button yet (T36).
- **Rules:** no file I/O on the UI thread (the folder dialog itself is the accepted exception, ADR 0010
  §10); no `unwrap`/`expect`/`panic!`; the grid and loupe behave as before when the dialog is closed;
  rules 6 and 7.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: Ctrl+I and the button open
  the dialog; a card in the reader shows as a button; "Välj…" opens the Windows folder dialog; a bad
  template or an empty destination shows its message under the right field; typing does not freeze the
  window; the preview updates shortly after typing stops; Escape closes, and Ctrl+I shows the same
  settings again.

### T36. ADR 0010 M5c: run the import and open the result in a new tab — Done

- **Start after T35 is done.**
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only.
- **What (ADR 0010 §3.1, GR12, Q2):**
  - **Start:** a "Starta import" button, enabled when `can_start()`, and Enter (`DialogKey::Start`):
    `start()`, then `run(form)` on the worker and `sent(id)`.
  - **Running:** the fields and buttons of the form are disabled; a progress bar (bytes done of bytes
    total) with `progress_text`; "Avbryt" calls `ImportWorker::cancel()` when `dialog.cancel()` is true.
    The dialog cannot be closed (Escape does nothing), but a **"Dölj"** button hides it so the grid and
    loupe can be used during the copy (ADR 0010 §8.2); Ctrl+I shows it again. While it is hidden and
    running, the status line shows `progress_text` in short form ("Importerar {done} / {total}").
    Keep polling the worker every frame whether the dialog is shown or hidden, and ask for repaints
    while running.
  - **Done:** show `done_lines` (error lines in red). When the state becomes `Done` and `result_folder()`
    is `Some`, open that folder **once** in a new tab with the existing `open(&folder, true)` (it starts
    in the grid, GR13; an already open folder is activated, GR12). If the dialog is hidden at that
    moment, the status line shows "Importen klar: {copied} filer" or "Importen avbruten: {copied} filer"
    (or the error). Buttons: "Ny import" (`back_to_editing`) and "Stäng".
- **Rules:** as T35; the copy never runs on the UI thread; rules 6 and 7.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0010 §8.2, step A):
  import from a real card into an empty folder: progress, summary, and the folder opens in a new tab as
  a contact sheet; cancel during the copy leaves no final-named partial files and the summary says
  "Avbruten"; "Dölj" during the copy, then stepping in the grid and the loupe stays smooth; a second
  import of the same card shows the files as already present and opens no new tab when nothing was
  copied.

### T37. ADR 0010 M7a: backup destination — Done

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/main.rs`.
- **What (ADR 0010 §3.2; the Tauri dialog's "Backup" checkbox and field):**
  1. `ImportForm` gets `backup_on: bool` (default `false`) and `backup: String` (default empty).
     `Field` gets `Backup`.
  2. `validate()`: when `backup_on` is false, the backup is `None` as today. When it is true, pass the
     trimmed `backup` (an empty field counts as no backup, as in `pw-ingest`). To put an error on the
     right field **without reading the message text**: first call `validate_options` **without** the
     backup; an error there is `Field::Dest` (as today); then call it **with** the backup; an error that
     only appears now is `Field::Backup`. Messages unchanged ("Backupmålet …").
  3. Dialog (`main.rs`): under "Mål", a checkbox "Backup" and, on the same row, a text field and
     "Välj…" (title "Välj backupmål", starting in the typed folder), both disabled while the checkbox is
     off; the `Backup` field error under it. Disabled while running, like the other fields.
- **Rules:** the copy itself already handles `Options::backup` (`pw-ingest`); nothing changes in the
  worker; rules 6 and 7.
- **Tests to add** (`import.rs`): backup off → `None` even with text in the field; backup on with a
  folder → `Some`; backup on with an empty field → `None`; backup inside the source → `Field::Backup`;
  backup equal to the destination → `Field::Backup` with "Backupmålet måste vara en annan mapp än målet.";
  a bad destination together with a bad backup → `Field::Dest` (the destination is reported first).
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: an import with backup
  writes the same files to both folders; the preview shows "(bara backup)" for files that only the
  backup lacks.

### T38. ADR 0010 M7b: job name and first running number — Done

- **Start after T37 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/main.rs`.
- **What:**
  1. `ImportForm` gets `job: String` (default empty) and `first_seq: u32` (default 1). `validate()`
     passes them on instead of `""` and `1` (the job is trimmed by `pw-ingest`).
  2. Dialog: after "Filnamnsmall", a row "Jobb" with a text field and "Första nr" with an
     `egui::DragValue` (range 0 to 999 999, as the Tauri field allows 0), and the help text
     "Variabler: {year} {year2} {month} {day} {hour} {minute} {second} {date} {time} {make} {model}
     {name} {seq} {seq:4} {job}" in a small, grey font (the list in the `pw_ingest::template` module
     documentation).
- **Rules:** rules 6 and 7.
- **Tests to add:** the defaults; `job` and `first_seq` reach `Options`; a job with spaces around it is
  trimmed.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand that `{job}` in the folder
  template and `{seq:4}` in the file-name template give the expected names in the preview.

### T39. ADR 0010 M7c: IPTC template on the new copies — Done

- **Start after T38 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/import_worker.rs`,
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/main.rs`, `crates/pw-loupe/src/lib.rs`.
- **What (ADR 0010 §3.2; the Tauri `run`, "IPTC template on the new copies"):**
  1. **Form:** `ImportForm` gets `iptc: IptcForm` with the strings `creator`, `copyright`, `credit`,
     `email`, `phone`, `url` (the six fields of the Tauri dialog; title and description are not in that
     dialog either). `IptcForm::to_iptc(&self) -> pw_metadata::Iptc`, as the Tauri `From<IptcDto>`:
     copyright as `LocalizedValue::x_default`, title and description empty.
  2. **Worker:** after the copy, when the result is not cancelled and the template is not empty
     (`Iptc::is_empty`), write it to every copied file that is not a companion file
     (`pw_ingest::COMPANION_EXTENSIONS`), in the destination and, when `to_backup`, in the backup — the
     same selection as the Tauri `run`. Use a `pw_metadata::ExifTool` that the worker thread starts
     itself, only when needed, and keeps for later runs. The program name comes from
     `ImportWorker::start()` (`"exiftool"`, as `ExifTool::start`) or from a new
     `ImportWorker::start_with_exiftool(program: &str)` used by the tests.
     - Send `ImportEvent::IptcProgress { id, done, total, current: PathBuf }` before each file.
     - Check the cancel flag before each file; when set, stop and mark the result `cancelled`.
     - `ImportDone` gets `iptc_errors: Vec<String>` (`"{file name}: {error}"`); if ExifTool cannot be
       started, one entry with that error and no further attempts in this run.
  3. **Dialog state:** `Running` also shows IPTC progress: add an `IptcProgress` case to the state
     machine (`Running { progress }` keeps the copy progress; add `iptc: Option<(usize, usize, PathBuf)>`
     or a small enum, your choice, tested).
  4. **Texts:** `iptc_progress_text(done, total, current) -> String` = "Skriver IPTC {done + 1} / {total}
     · {file name}"; `done_lines` adds up to 10 "IPTC: {error}" error lines after the failures (as the
     Tauri dialog).
  5. **Dialog (`main.rs`):** a collapsing section "IPTC-mall" (closed by default) with the six fields
     labelled Fotograf, Copyright (hint "© 2026 Namn"), Credit, E-post, Telefon, Webb, and the help text
     "Skrivs till .xmp-sidofilen för RAW och in i DNG/JPG/TIF. Tomma fält lämnas orörda." While running,
     the progress bar shows the IPTC progress (files) after the copy.
- **Rules:** ExifTool runs only on the worker thread; no `unwrap`/`expect`/`panic!` in non-test code;
  rules 6 and 7.
- **Tests to add:** `to_iptc` (copyright becomes `x-default`; an all-empty form is `is_empty`); the
  worker with an **empty** template never starts ExifTool (use a program name that does not exist: no
  `iptc_errors`); with a template and a program that does not exist: the copy is reported as usual and
  `iptc_errors` has one entry; cancel before the IPTC step writes nothing; the state machine and the
  texts (including the 10-line limit). A real write is checked by hand.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: after an import with
  Fotograf and Copyright filled in, the `.xmp` sidecar of a RAW (and a DNG or JPG itself) carries them,
  for example in Photo Mechanic or with `exiftool -XMP:Creator -XMP:Rights <file>`.

### T40. ADR 0010 M7d: eject the card after the import — Done

- **Start after T39 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/import.rs`, `crates/pw-loupe/src/import_worker.rs`,
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/tabs.rs`, `crates/pw-loupe/src/main.rs`.
- **What (ADR 0010 §3.2 and the risk in §10; the Tauri `run`):**
  1. `ImportForm` gets `eject: bool` (default `false`).
  2. **Worker,** after the copy and the IPTC step, when `eject` is set:
     - if the result is cancelled or has failures: `ImportDone::eject = Some("Kortet matades inte ut
       eftersom importen inte blev komplett.")`;
     - otherwise send `ImportEvent::Ejecting { id }`, forget the cached scan, and set
       `eject = Some(pw_ingest::eject(&source))` (its message as it is).
  3. **Tabs:** `Tabs::clear_inside(&mut self, root: &Path) -> bool` empties every tab whose folder is
     the root or below it (compare paths component by component, case-insensitively on Windows, with no
     file access) and returns whether the active tab was among them.
  4. **Dialog (`main.rs`):** a checkbox "Mata ut kortet när importen är klar (inget raderas från kortet)".
     When a run starts with `eject` set, call `clear_inside(source)` first and reload the active tab if
     it was emptied, so no tab reads from the card while it is ejected (ADR 0010 §10). While ejecting,
     the progress text is "Matar ut {source} …". After `Done` with an `eject` message, ask the worker
     for `cards()` again so the ejected card disappears from the buttons.
  5. **Texts:** `done_lines` adds the eject message as a normal line after the IPTC errors.
- **Rules:** `pw_ingest::eject` runs only on the worker thread (it waits for Windows); rules 6 and 7.
- **Tests to add:** eject not requested → `eject` is `None`; requested with a cancelled or failed result
  → the "inte komplett" message and no eject attempt (use a run that is cancelled before it starts);
  `clear_inside` (the root itself, a subfolder, a folder with a similar name such as `E:\DCIM2` next to
  `E:\DCIM`, different case, the active tab returned); the dialog state and `done_lines` with an eject
  message. A real eject is checked by hand.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: an import with the
  checkbox ejects the card at the end and the message says so; a cancelled import with the checkbox
  does not eject and says why; a tab showing the card is emptied before the eject.

### T41. ADR 0013 B1: rating keys, applying a key, rating texts (plain Rust) — Done

- **2026-11-09:** the agent did points 1–3; the review asked for point 4 and more tests, and the
  reviewer (Claude) finished them. The owner's `.\tools\check.ps1` run passed.

- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/ratings.rs`, declared and re-exported in
  `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`, `metadata.rs` or any `Cargo.toml`.
- **Why:** ADR 0013 §4.1. The keys of ADR 0004 (and X, ADR 0013 Q5) as tested plain Rust, before
  anything is drawn or written.
- **What:**
  1. `pub enum RatingKey { Stars(i8), Label(&'static str), ClearLabel, Reject }` (`Stars` is 0–5;
     `Label` holds one of `pw_metadata::LABELS`).
  2. `pub fn rating_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<RatingKey>`,
     with the same `(key, ctrl, shift, alt)` that `main.rs` already collects:
     - **Shift held → `None`.** Ctrl **and** Alt (AltGr) → `None`.
     - No modifier: `Num0`–`Num5` → `Stars(0..=5)`; `Num6`–`Num9` → `Label` Red, Yellow, Green, Blue;
       `X` → `Reject`.
     - Alt only: `Num0`–`Num5` → `Stars`; Alt+6–9 and Alt+X → `None`.
     - Ctrl only: `Num0` → `ClearLabel`; `Num1`–`Num5` → `Label` Red … Purple; Ctrl+6–9 and Ctrl+X →
       `None`.
     - Every other key → `None`.
     (egui reports the number pad digits as `Num0`–`Num9` too; this is checked by hand in T43.)
  3. `pub fn apply(current: &pw_metadata::Rating, key: RatingKey) -> pw_metadata::Rating`:
     `Stars(0)` clears the stars (also a reject) and keeps the label; `Stars(n)` sets them and keeps the
     label; `Label(l)` sets `l`, or clears the label if it already is `l`; `ClearLabel` clears it;
     `Reject` sets the stars to −1, or clears them if they already are −1, and keeps the label.
  4. Texts (Swedish, ADR 0013 Q2):
     - `pub fn label_name(label: &str) -> &str`: Red → "Röd", Yellow → "Gul", Green → "Grön",
       Blue → "Blå", Purple → "Lila"; any other text is returned as it is.
     - `pub fn status_text(rating: Option<&Rating>) -> String`: "Betyg: " followed by "Avvisad" (−1),
       "1 stjärna", "{n} stjärnor" or "Ej betygsatt", then ", {label_name}" when there is a label.
       Examples: "Betyg: 3 stjärnor, Röd", "Betyg: Avvisad", "Betyg: Ej betygsatt, Blå".
     - `pub fn star_glyphs(stars: Option<i8>) -> String`: "" for `None`, "✕" for −1, otherwise `n` × "★"
       followed by `5 − n` × "•".
     - `pub const UNSUPPORTED_TEXT: &str = "Betyg och etiketter kan inte sparas för den här filtypen.";`
- **Rules:** no I/O, no threads; no `unwrap`/`expect`/`panic!` or unchecked indexing in non-test code.
  `key_action` and `grid_key_action` are **not** changed (digits and X keep returning `None` there).
- **Tests to add:** every row of point 2, including Shift+digit, Ctrl+Alt+digit, Alt+6, Ctrl+6, Ctrl+X
  and an unrelated key; `key_action` and `grid_key_action` still return `None` for `Num0`–`Num9` and
  `X`; every rule of point 3 (including Stars(0) on a rejected image and a label toggle on an image
  with stars); the texts for −1, 1, 3, `None`, and with and without a label; `label_name` of an
  unknown label.
- **Done when:** `.\tools\check.ps1` is all OK.

### T42. ADR 0013 B2: rating store and write queue (plain Rust) — Done

- **Review 2026-11-09:** accepted. One gap in this task's text (reads were not covered by the
  back-pressure rule) is fixed in T45.

- **Start after T41 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/ratings.rs`, `crates/pw-loupe/src/metadata.rs`,
  and `crates/pw-loupe/src/main.rs` **only** for the calls that replace today's rating calls (no new
  drawing, no keys; those are T43). Not `pw-metadata`.
- **Why:** ADR 0013 §4.2: optimistic values, writes in key order, the last key wins, rollback on
  failure, and a folder read that never overwrites a newer value.
- **What:**
  1. A small trait so the store can be tested without ExifTool:
     `pub trait RatingWriter { fn write_rating(&mut self, path: PathBuf, rating: Rating) -> Result<RequestId, SubmitError>; }`,
     implemented for `pw_metadata::MetadataWorker` (it calls the existing method).
  2. `pub struct RatingStore`, keyed by absolute path, holding per path `confirmed: Option<Rating>`,
     `shown: Option<Rating>` and its writes (queued, not sent; and sent, with their `RequestId`).
     Methods (names may differ if the tests show the same behaviour):
     - `set(&mut self, path: &Path, key: RatingKey) -> Result<(), RatingRefused>`: refuses a file
       without `pw_metadata::supports_rating` (`RatingRefused::Unsupported`, nothing queued);
       otherwise `shown = apply(shown, key)` and queues a write. If the path already has a queued write
       that was **not sent yet**, that write's value is replaced instead of queuing another.
     - `pump(&mut self, writer: &mut impl RatingWriter)`: sends queued writes in queue order until the
       writer returns `QueueFull` (stop; the rest stays queued for the next frame). `Stopped` fails the
       write as in point 3.
     - `written(&mut self, id, path, result) -> Option<String>`: success → `confirmed` is the written
       value; failure → `confirmed` unchanged, and `shown` goes back to `confirmed` **unless** a newer
       write for the path is queued or sent. Returns the message for the status line on failure:
       "Kunde inte spara betyget för {file name}: {error}".
     - `read(&mut self, read_id: RequestId, ratings: &HashMap<PathBuf, Rating>, paths: &[PathBuf])`:
       for each path, `confirmed` = the read value (absent = `None`); `shown` is set too **only** if the
       path has no queued write and no write sent with an id greater than `read_id`.
     - `shown(&self, path: &Path) -> Option<&Rating>`, `pending(&self) -> usize` (queued + sent),
       `pending_paths(&self) -> Vec<PathBuf>`, and `prune(&mut self, keep: impl Fn(&Path) -> bool)`
       that drops entries for which `keep` is false **and** that have no pending write.
  3. `MetadataCache` keeps the load states (`Loading`, `Ready`, `Unsupported`, `Error`) and the read
     request, and gets a `RatingStore`. `begin_load` no longer drops ratings; a `RatingsRead` for the
     current load calls `store.read(…)`; `RatingWritten` calls `store.written(…)` (today's handling
     in `apply` is replaced). `poll` also calls `store.pump(worker)` (it needs `&mut MetadataWorker`).
     Failure messages are returned from `poll` (for example as `Vec<String>`) so `main.rs` can put the
     last one in `self.message`.
  4. `rating_summary` uses `status_text` (T41). The three English notices become Swedish:
     "Läser metadata…", `UNSUPPORTED_TEXT` and "Metadatafel: {error}". `submit_error` texts: "metadatakön är full" and "metadataarbetaren har
     stannat".
  5. `main.rs`: only adapt the existing calls (`poll` with `&mut`, the message). When a tab is closed,
     call `prune` with the files of the tabs still open.
- **Rules:** no I/O and no threads in `ratings.rs`; no `unwrap`/`expect`/`panic!`; rules 6 and 7.
- **Tests to add** (with a fake writer that records requests, can return `QueueFull` or `Stopped`, and
  lets the test deliver results): `set` shows the value at once; Unsupported refused with nothing queued;
  1-2-3-4-5 before a `pump` gives one write of 5; order kept across two paths; `QueueFull` leaves the
  rest for the next `pump`; `Stopped` rolls back; a failure with a newer write pending keeps the newer
  value, and a failure without one rolls back to `confirmed`; a read with an id lower than a sent write
  does not change `shown` but a read with a higher id does; a read never changes a path with a queued
  write; entries survive `begin_load` of another folder; `prune` keeps paths with pending writes;
  `MetadataCache` with the Swedish notices. Existing `metadata.rs` tests keep passing (update only
  expected English texts to the Swedish ones, and say so in the report).
- **Done when:** `.\tools\check.ps1` is all OK.

### T43. ADR 0013 B3: rating keys and display in the grid and the loupe — Done

- **Second review 2026-11-09: code accepted.** Points 1 and 2 below are fixed; point 3 is decided by
  the owner during the hand check. Waiting for the owner's hand check ("Done when").
- **Review 2026-11-09: changes requested.** Keys, focus rule, grid row, dimming and the badge are
  accepted. Still to do, in `crates/pw-loupe/src/main.rs` only:
  1. **The star is drawn wrongly.** `paint_rating_glyphs` fills the ten-point star with
     `egui::Shape::convex_polygon`, but a star is not convex: egui fills a convex polygon as a fan
     from its first point, so the notches between the tips are filled in (the star looks like a
     pentagon) and the edge anti-aliasing is wrong. Draw it as ten triangles from the centre in an
     `egui::Mesh` (centre, point *i*, point *i*+1), or as the inner pentagon plus five tip triangles,
     each of which is convex. Keep the size and spacing.
  2. **Badge width:** measure the label text with `painter.layout_no_wrap(…)` (the galley's
     `rect.width()`) instead of `chars().count() × 8`, so "Röd" and "Lila" neither overflow nor leave a
     gap.
  3. **Badge content:** ADR 0013 §5.2 asks for the same look as the grid row (stars on the label
     colour). The Swedish label name in the badge is an addition; keep it only if the owner says so
     after looking at it — otherwise remove it (the status line already names the label).
  Then run `.\tools\check.ps1` and hand over for the owner's manual check (the "Done when" list).

- **Start after T42 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/ratings.rs`
  only for a small plain function with a test (for example the label colours).
- **What:**
  1. **Keys:** in the key loop, after the dialog and tab checks and before the mode-specific keys,
     `rating_key_action(key, ctrl, shift, alt)` acts on the current image of the active tab (grid: the
     current cell; loupe: the shown image, `tab.current`). Skip rating keys when
     `ctx.wants_keyboard_input()` is true or the import dialog consumes keys (ADR 0013 §4.3). A refusal
     puts `UNSUPPORTED_TEXT` in the status line.
  2. **Grid row (ADR 0013 §5.1):** in the row under each thumbnail, `star_glyphs` on the left, the file
     name (shortened as today) after it. With a label, the row's background is the label colour —
     Red `#e5484d`, Yellow `#f5c400`, Green `#3fb950`, Blue `#4c8dff`, Purple `#a371f7` — with dark text
     on Yellow and Green. A rejected image's thumbnail is drawn dimmed (for example a tint of about 40 %).
  3. **Loupe badge (§5.2):** in the lower left corner of the image area, the same look as the grid row
     (stars on the label colour), only when the image has stars or a label, and only while the
     information is shown (I key). The status line shows `status_text` as today's rating summary does.
  4. If egui's bundled fonts lack "★", "•" or "✕", draw them as shapes instead; no new font.
- **Rules:** as T28; drawing reads `RatingStore::shown` only, never files; no other key changes.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0013 §8.2): keys on
  the main row and the number pad in grid and loupe; Shift+digit and AltGr do nothing; digits typed
  into the import dialog never rate; ARW, RW2, DNG and JPEG keep their values after reopening the
  folder; a DNG rated in RAW mode still shows correctly after stepping away and back; a read-only
  sidecar gives the message and the old value; holding a digit while stepping keeps the loupe smooth.

### T44. ADR 0013 B4: finish pending rating writes on exit — Done

- **Start after T43 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/ratings.rs` or
  `metadata.rs` only for a plain, tested helper.
- **What (ADR 0013 Q6, R4):**
  1. When the program exits (`eframe::App::on_exit`, or the equivalent hook in eframe 0.37), keep
     calling `pump` and `poll` with a short sleep (about 20 ms) until `pending()` is 0 or **5 s** have
     passed. Put the loop in a plain helper with an injectable clock and sleep, so it can be tested.
  2. If writes are still pending, or some failed during the wait, show one message box listing the file
     names (at most 20, then "… och {n} till"), titled "Betyg som inte sparades", with
     `rfd::MessageDialog` (`rfd` is already a dependency of `pw-loupe`). If `MessageDialog` is not
     available with the pinned features, **stop and report** instead of adding a feature.
  3. If the window stays visible during the wait, say so in the report; the owner decides whether that
     is acceptable.
- **Rules:** as T42; no new dependencies and no feature changes.
- **Tests to add:** the helper returns at once when nothing is pending; ends when the last write is
  confirmed; gives up after 5 s with the pending files listed (fake clock); the list text with more
  than 20 files.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: press a rating key and
  close the window at once; reopen the folder and the value is there.

### T45. ADR 0013 B2 follow-up: send the folder read again when the metadata queue is full — Done

- **Start after T42 is done.** Small; can be done before or after T43.
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/metadata.rs` only.
- **Why:** `MetadataCache::begin_load` sends the folder read with `worker.read_ratings`. Since T42,
  rating writes can fill the worker's queue (8 requests). If a tab is loaded at that moment, the read
  gets `SubmitError::QueueFull`, and every image of the folder shows "Metadatafel: metadatakön är
  full" until the tab is loaded again. ADR 0013 §4.2 rule 2 (back-pressure is retried, not an error)
  should apply to the read too.
- **What:**
  1. When `read_ratings` returns `QueueFull`, keep the files to read (for example
     `pending_read: Option<Vec<PathBuf>>`) and leave the images in `Loading`. `Stopped` still fails the
     load as today.
  2. In `poll`, after draining the events, send the pending read **before** `ratings.pump(worker)`.
     On `Ok(id)` set `load_request = Some(id)` and clear `pending_read`; on `QueueFull` keep it for the
     next frame; on `Stopped` fail the load.
  3. A new `begin_load` replaces any pending read (only the active tab's folder is read).
  4. To test this without ExifTool, let the read go through a small trait, as `RatingWriter` does
     for writes (for example `trait MetadataSubmit: RatingWriter { fn read_ratings(…) }` implemented
     for `MetadataWorker`), and make `begin_load` and `poll` generic over it, or test a private
     helper; your choice, as long as the test uses a fake that can return `QueueFull`.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; rules 6, 7, 10–12.
- **Tests to add:** `QueueFull` on the read keeps the images loading (no error notice); the next
  `poll` sends the read and a `RatingsRead` with that id makes them ready; `Stopped` gives the error
  as today; a second `begin_load` while a read is pending sends only the new folder; the read is sent
  before queued writes in the same `poll`.
- **Done when:** `.\tools\check.ps1` is all OK.

### T46. ADR 0013 §5: rounded, inset label bar in the grid and the loupe badge — Done

- **Start after T43 is done.** For a Claude agent (drawing code, checked by hand).
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only.
- **Why (owner, 2026-11-09):** the label colour fills the whole row under a thumbnail, edge to edge,
  with square corners. The owner wants it inset from the cell edges and clearly rounded.
- **What:**
  1. **Grid row:** draw the label colour as a rounded rectangle **inside** `label_rect`: about 6 pt in
     from the left and right edges and 2 pt from the top and bottom, with the corner radius equal to
     half its height (a pill shape). Use the painter's rounded `rect_filled` (corner radius), no new
     shape code. Without a label nothing is drawn behind the row, as today.
  2. The stars and the file name start inside the pill (move their left edge in by the same inset)
     and the file name stays clipped to the row.
  3. **Loupe badge:** the same rounding (radius = half the badge height) for both the label colour
     and the dark background used when there is no label.
  4. Keep the colours, the dark text on Yellow and Green, the dimming of rejected thumbnails, the
     cell size and the selection frame as they are.
- **Rules:** as T43; no new dependencies; rules 6, 7, 10–12.
- **Tests:** none required (drawing only); if you factor the inset rectangle into a small plain
  function, give it a test.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: the colour bar no longer
  touches the cell edges, its ends are round, the stars and name sit inside it at thumbnail sizes 96,
  200 and 480, and the loupe badge has the same shape.

### T47. pw-loupe does not start since T44: embed the Common Controls 6 manifest — Done

- **Do this first; it blocks the hand checks of T43, T44 and T46.** For a Claude agent.
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/build.rs`; `crates/pw-loupe/Cargo.toml` only
  if Cargo needs `build = "build.rs"` (it finds `build.rs` by itself, so normally no change). No new
  dependencies.
- **What happens (owner, 2026-11-09):** since T44, `target\debug\pw-loupe.exe` exits at once without
  a window or a message, exit code `-1073741511` = `0xC0000139` (STATUS_ENTRYPOINT_NOT_FOUND: Windows
  cannot find a function the program imports). The T43 + T45 build starts normally.
- **Likely cause:** T44 added `rfd::MessageDialog`. In this workspace `rfd` 0.17.2 is shared with the
  Tauri app's `tauri-plugin-dialog`, and Cargo merges the features of a shared crate, so `rfd` is
  probably built with `common-controls-v6`. Its message box then imports `TaskDialogIndirect` from
  `comctl32.dll`, which exists only in Common Controls version 6. Windows loads version 6 only when the
  program's manifest asks for it; the Tauri app has such a manifest, `pw-loupe` has none. The test
  binary still loads because nothing in it calls `on_exit`, so the linker drops the import (that is
  why `check.ps1` stayed green).
- **What:**
  1. **Confirm first** and paste the output in the report:
     `cargo tree --workspace -e features -i rfd` (shows which crate enables `common-controls-v6`). If
     it is **not** enabled, stop and report; the fix below would then be the wrong one.
  2. Add `crates/pw-loupe/build.rs` that, only when `CARGO_CFG_TARGET_OS` is `windows` and
     `CARGO_CFG_TARGET_ENV` is `msvc`, prints
     ```
     cargo:rustc-link-arg-bins=/MANIFEST:EMBED
     cargo:rustc-link-arg-bins=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'
     ```
     and `cargo:rerun-if-changed=build.rs`. No `unwrap`/`expect` (read the variables with `match` or
     `.ok()`). A short comment explains why (this task, ADR 0013 T44).
  3. Nothing else changes; `on_exit` and the message box stay.
- **Note:** a new build script may be stopped by the owner's antivirus (LNK1104, rules 6–7). Build
  only with `cargo build --workspace` in `target`; if it is blocked, stop and tell the owner.
- **Done when:** `.\tools\check.ps1` is all OK, **and** `cargo build --workspace` followed by
  `.\target\debug\pw-loupe.exe D:\PhotoWorkflowData\RAW` opens the window (report `$LASTEXITCODE`
  after closing it: `0`). Then the owner checks by hand: rate an image, close the window at once,
  reopen the folder → the rating is there; and, with a read-only sidecar, the message box "Betyg som
  inte sparades" appears at exit.

### T48. pw-metadata: a write that ExifTool refused is reported as saved — Done

- **Added by the agent at the owner's request (2026-11-09), found in the T47 hand check.** For a Claude
  agent.
- **Crate:** `pw-metadata`. **Files:** `crates/pw-metadata/src/lib.rs`,
  `crates/pw-metadata/tests/sidecar.rs`. No new dependencies.
- **What happens:** with a read-only `.xmp`, a rating can be set in `pw-loupe` without any message;
  the file is not changed. ExifTool refuses the write and answers
  ```
  Error renaming temporary file to …/test.xmp
      0 image files updated
      1 files weren't updated due to errors
  ```
  (a read-only JPEG gives `Error opening … for writing` and the same two lines). `write_tags` treats
  every answer that contains the text `files updated` as success, so `0 image files updated` counts
  as saved. `write_rating` and `write_iptc` both go through `write_tags`.
- **What:**
  1. Add a small pure function in `lib.rs`, e.g. `fn write_succeeded(out: &str) -> bool`, that reads
     ExifTool's summary lines (`<n> image files created|updated|unchanged`,
     `<n> files weren't … due to errors`) as numbers: success only when at least one file was
     created, updated or unchanged and no file failed.
  2. `write_tags` uses it instead of the three `contains` checks. A failure still returns
     `Error::ExifTool(out.trim())` as today (the message text is unchanged).
  3. Nothing else changes; `pw-loupe` already shows the error and rolls the rating back when the
     write fails (T42, T43).
- **Tests to add:**
  - unit tests for `write_succeeded`: created, updated, unchanged (success); the read-only answer
    above, `0 image files updated` alone and an empty answer (failure);
  - in `tests/sidecar.rs` (real ExifTool): a rating written to an ARW whose sidecar is read-only
    returns an error, and the sidecar keeps its old rating; the same for a read-only JPEG (embedded).
    Restore the permissions afterwards so the temporary folder can be removed.
- **Done when:** `.\tools\check.ps1` is all OK. Then the owner checks by hand: rate an image with a
  read-only sidecar → the error message and the old rating; at exit with such a write pending, the
  message box "Betyg som inte sparades".

### T49. Swedish message when a rating cannot be saved — Done

- **Added by the agent at the owner's request (2026-11-09), found in the T48 hand check.** For a Claude
  agent. **Start after T48.**
- **Crates:** `pw-metadata`, `pw-loupe`. **Files:** `crates/pw-metadata/src/lib.rs`,
  `crates/pw-metadata/src/worker.rs`, `crates/pw-metadata/tests/sidecar.rs`,
  `crates/pw-loupe/src/metadata.rs`. No new dependencies.
- **What happens:** since T48 a rating on an image with a read-only sidecar is refused and rolled
  back, but the message shows ExifTool's English answer:
  `Kunde inte spara betyget för TN005979.ARW: ExifTool: 0 image files updated … Error renaming
  temporary file …`.
- **Owner's decision (2026-11-09):** a read-only file gives
  `Kunde inte spara betyget för <fil>: filen är skrivskyddad`; any other write that ExifTool refuses
  gives `Kunde inte spara betyget för <fil>: filen kunde inte skrivas`. The Swedish texts live in
  `pw-loupe`; `pw-metadata` reports the kind of error, and `pw-loupe` never compares error text (as T3).
- **What:**
  1. `pw-metadata` `Error`: two new variants, `ReadOnly(PathBuf)` and `NotWritten(String)` (ExifTool's
     answer). When `write_succeeded` is false, `write_tags` returns `ReadOnly(target)` if the target
     file exists and is read-only, otherwise `NotWritten(answer)`. ExifTool still decides whether
     the write failed; the read-only check only names the reason. Put the choice in a small function
     that can be tested with a temporary file.
  2. `pw-metadata`: a public `WriteError` (`ReadOnly`, `NotWritten(String)`, `Other(String)`) with
     `From<Error>` and `Display`; `WorkerEvent::RatingWritten` carries `Result<(), WriteError>`
     instead of `Result<(), String>`. `Other` holds today's text for everything else (ExifTool could
     not start, crashed, I/O), so those messages are unchanged.
  3. `pw-loupe` `metadata.rs`: turn the `WriteError` into the text for the rating store with a small
     function: `ReadOnly` → "filen är skrivskyddad", `NotWritten` → "filen kunde inte skrivas",
     `Other(text)` → `text`. `RatingStore` and its tests are not changed.
  4. Test helpers that build or read `RatingWritten` (`FakeSubmit` in `pw-loupe/src/metadata.rs`,
     `write_result` in `pw-metadata/src/worker.rs`) convert between `String` and `WriteError::Other`;
     their assertions stay as they are. T48's test in `tests/sidecar.rs` now expects
     `Error::ReadOnly` for the read-only sidecar and JPEG (the old rating is still kept).
- **Tests to add:** the choice in point 1 (read-only file → `ReadOnly`, writable file → `NotWritten`,
  missing file → `NotWritten`); `From<Error>` for the three kinds; the text function for the three
  kinds; a `RatingWritten` with `ReadOnly` gives "Kunde inte spara betyget för <fil>: filen är
  skrivskyddad" through `MetadataCache::poll`.
- **Not in this task:** messages for ExifTool missing or crashing stay as today (English); the Tauri
  app shows the new English `Display` text of the two variants.
- **Done when:** `.\tools\check.ps1` is all OK. Then the owner checks by hand: rate an image with a
  read-only sidecar → "Kunde inte spara betyget för <fil>: filen är skrivskyddad" and the old rating.

### T50. ADR 0013 B6: documentation after milestone 4 — Done

- **Documentation only; no code, no `Cargo.toml`.** For a Claude agent.
- **Files:** `docs/adr/0013-egui-ratings-and-labels.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/architecture.md`, `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`,
  `docs/test-inventory.md` (only if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T41–T49 in this file; write only what they record. Do not
  invent measurements or results.
- **What:**
  1. **ADR 0013:**
     - Status: add "**Implemented (2026-11-10):** steps B1–B5 done (agent tasks T41–T49); B6 is this
       documentation update." (as ADR 0011's status line).
     - §9: the Result column of B1–B6 with their tasks (B1 T41; B2 T42, T45; B3 T43, T46; B4 T44, T47;
       B5 the owner's hand checks; B6 T50), and one line under the table that T47–T49 were found in the
       hand checks.
     - §7: a short result per R1–R6 from the log (R1–R5 checked by hand as recorded; R3 with the Swedish
       messages of T49; R4 with the exit wait of T44). **R6 (Lightroom / Photo Mechanic) is not in the
       log:** mark it "not checked yet" — do not mark it passed.
     - A new short section "Findings during implementation": the Common Controls 6 manifest (T47:
       `rfd` features are merged with the Tauri app, `check.ps1` cannot catch a binary that fails to
       load), refused writes (T48: ExifTool's "0 image files updated"), the Swedish reasons (T49:
       `WriteError`), and the label name kept in the loupe badge (owner, T43).
  2. **ADR 0004:** a note "2026-11-10: implemented in the egui program (ADR 0013)". Under "Open", keep
     the clean-room level C check open (it is the same as ADR 0013 R6) and add: "A write that ExifTool
     refuses is now reported (T48); a read-only file gives its own error (T49)."
  3. **architecture.md:** in the crate table, `pw-metadata` also reports refused writes by kind
     (`WriteError`), and `pw-loupe` also has ratings and labels (key map, the rating store with its
     ordered write queue, the exit wait; ADR 0013). Mention `crates/pw-loupe/build.rs` (manifest) in one
     line.
  4. **GUI_REQUIREMENTS.md** GR8 and the root `README.md` status: ratings and labels are done in the
     egui program; still left before the switch: filter and multi-selection, IPTC metadata, TIFF
     viewing. Keep the rest of GR8 as it is.
  5. **docs/README.md:** the ADR list says "0013 milestone 4: ratings and labels in egui – done
     2026-11-10".
- **Rules:** English, the style of the existing documents (short sentences, no marketing words);
  change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK (formatting and tests are unaffected) and
  `git diff --stat` shows only the files above.

### T51. ADR 0014 F1: the filter model and the filter of a tab (plain Rust) — Done

- **Second review 2026-11-10: accepted.** The review points below are fixed.

- **Review 2026-11-10: changes requested (one point).** Everything else is accepted: `matches`,
  `visible`, `nearest_visible`, the lists and texts, the `Tab` field, the reset in `Tab::open`, and the
  tests. Still to do, in `crates/pw-loupe/src/filter.rs` only:
  1. **`AtLeast` outside 1–5 must match the way its text reads.** `star_filter_text` shows
     `AtLeast(0)` as "≥ 1 stjärna" and `AtLeast(6)` as "5 stjärnor", but `matches` uses the raw
     value (`AtLeast(0)` lets through stars ≥ 0, `AtLeast(6)` nothing). In `matches`, clamp the
     minimum to 1–5 first, so the filter does what its text says. Add a test: `AtLeast(0)` behaves as
     `AtLeast(1)` (a reject still never matches) and `AtLeast(6)` as `AtLeast(5)`, for every value in
     `STAR_VALUES`.
  Then run `.\tools\check.ps1`.

- **Can start now.** Independent of T52–T54.
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/filter.rs`, declared and re-exported in
  `crates/pw-loupe/src/lib.rs`; `crates/pw-loupe/src/tabs.rs`. **Not** `main.rs` or any `Cargo.toml`.
- **Why:** ADR 0014 §2.1–§2.3, §4.1, §5.2, Q1, Q8. The filter as tested plain Rust before anything is
  drawn.
- **What:**
  1. Types:
     - `pub enum StarFilter { All, AtLeast(u8), Unrated, Rejected, NotRejected }` (`AtLeast` is 1–5;
       `AtLeast(5)` is "5 stjärnor");
     - `pub enum LabelFilter { All, Label(&'static str), NoLabel }` (`Label` holds one of
       `pw_metadata::LABELS`);
     - `pub struct Filter { pub stars: StarFilter, pub label: LabelFilter }`, `Default` = all / all,
       and `pub fn is_active(&self) -> bool` (anything other than all / all).
  2. `pub fn matches(filter: &Filter, rating: Option<&pw_metadata::Rating>) -> bool`. No rating and a
     rating without stars count as unrated. Stars: `All` → true; `AtLeast(n)` → stars ≥ n (so a reject,
     −1, never matches); `Unrated` → no stars; `Rejected` → −1; `NotRejected` → anything except −1
     (unrated included). Label: `All` → true; `Label(l)` → the label is `l`; `NoLabel` → no label.
     Stars and label combine with AND.
  3. `pub fn visible(count: usize, keep: Option<usize>, is_match: impl Fn(usize) -> bool) -> Vec<usize>`:
     the indexes `0..count` that match, in order, plus `keep` if it is `< count` (the keep-current rule,
     ADR 0014 §2.2).
  4. `pub fn nearest_visible(visible: &[usize], current: usize) -> Option<usize>`: `current` if it is
     in `visible`; otherwise the first index after it; otherwise the last one; `None` when `visible` is
     empty (§2.3).
  5. Texts (ADR 0014 §5.2), in the order of the drop-down lists:
     - `pub const STAR_FILTERS: [StarFilter; 9]` = All, AtLeast(1)…AtLeast(4), AtLeast(5), Unrated,
       Rejected, NotRejected, and `pub fn star_filter_text(f: StarFilter) -> &'static str`: "Alla
       betyg", "≥ 1 stjärna", "≥ 2 stjärnor", "≥ 3 stjärnor", "≥ 4 stjärnor", "5 stjärnor", "Ej
       betygsatt", "Avvisade", "Alla utom avvisade" (Q8);
     - `pub const LABEL_FILTERS: [LabelFilter; 7]` = All, the five labels in `LABELS` order, NoLabel,
       and `pub fn label_filter_text(f: LabelFilter) -> &'static str`: "Alla etiketter", the Swedish
       names of `label_name`, "Ingen etikett". `AtLeast` outside 1–5 gives the text of the nearest
       valid value; no panic.
  6. `tabs.rs`: `Tab` gets `pub filter: Filter`. `Tab::open` resets it to the default (a new or
     refreshed folder starts without a filter, Q1). `Tabs::set_filter(&mut self, filter: Filter)` sets
     the filter of the active tab. Other tabs keep their own filter.
- **Rules:** no I/O, no threads; no `unwrap`/`expect`/`panic!` or unchecked indexing in non-test code;
  rules 6, 7, 10–12. Existing tests are not changed.
- **Tests to add:** every star choice against unrated, 1, 3, 5, −1 (with and without a label); every
  label choice against no label, Red and Purple; AND of both; "≥ 1" never matches −1;
  `NotRejected` matches unrated; `visible` with and without `keep`, and `keep` out of range;
  `nearest_visible` for current visible, a following index, only earlier ones, empty; the text lists
  (length, order, Swedish texts); a new tab and `Tab::open` start without a filter; `set_filter` changes
  only the active tab; two tabs keep different filters across `next`/`previous`.
- **Done when:** `.\tools\check.ps1` is all OK.

### T52. ADR 0014 F2: multi-selection model, Shift in the grid key map (plain Rust) — Done

- **Second review 2026-11-10: accepted.** The review points below are fixed.

- **Review 2026-11-10: changes requested (two points).** The key map, the `Tab` field,
  `active_mut`, the `main.rs` call and the tests are accepted. The change to the existing key test
  (Escape now gives `ClearSelection`) is correct: point 2 of this task gives Escape that meaning, and
  the task's "existing key tests keep passing" was wrong for that one line. The two `ratings.rs` test
  calls outside the file list are accepted (the new argument made them necessary). Still to do, in
  `crates/pw-loupe/src/selection.rs` only:
  1. **`extend_to` must never mark a hidden image** (ADR 0014 S5). When `index` is not in `visible`,
     it marks `index` today. Change it: if `index` is not visible, leave the marking and the anchor
     unchanged. If only the anchor is not visible, mark `index` alone, as today. Update the doc comment
     and add a test that an `index` outside `visible` changes nothing.
  2. **`all` must replace the marking**, as `pickAll` in `app.js` does (`clearPicked`, then every
     visible image), not add to it. Today the result is the same only while every marked image is
     still visible. Add a test: mark 2 and 3, then `all(&[0, 1])` → exactly [0, 1].
  Then run `.\tools\check.ps1`.

- **Can start now.** Independent of T51, T53 and T54. If T51 is done first, keep its `Tab` field.
- **Crate:** `pw-loupe`. **Files:** a new `crates/pw-loupe/src/selection.rs` (declared and re-exported
  in `lib.rs`), `crates/pw-loupe/src/grid.rs`, `crates/pw-loupe/src/tabs.rs`, and
  `crates/pw-loupe/src/main.rs` **only** for the changed `grid_key_action` call (new arms do nothing
  yet; T56 wires them).
- **Why:** ADR 0014 §2.5, §2.6, §2.8, §4.5, Q5. The marking rules of `app.js` (`clickCell`,
  `pickRange`, `pickAll`, `clearPicked`, `go`, `ratingTargets`) as tested plain Rust.
- **What:**
  1. `pub struct Selection` with the marked file indexes and an anchor (`Option<usize>`). All
     positions are taken from a `visible: &[usize]` argument (file indexes in display order).
     Methods:
     - `click(&mut self, index)`: clears the marking; anchor = `index`.
     - `ctrl_click(&mut self, current, index)`: if nothing is marked, mark `current` first; then if
       `index` is marked **and** more than one is marked, unmark it, otherwise mark it; anchor = `index`.
     - `extend_to(&mut self, visible, current, index)`: if nothing is marked, anchor = `current`; then
       the marking is replaced by every visible index between the anchor and `index` (inclusive, by
       position in `visible`). If the anchor is not visible, the range is `index` alone. The anchor
       does not change. (Shift+click and Shift+navigation.)
     - `all(&mut self, visible)`: marks every visible index (anchor unchanged).
     - `clear(&mut self)`: unmarks all (anchor unchanged).
     - `retain_visible(&mut self, visible)`: unmarks indexes not in `visible` (§2.8).
     - `count()`, `is_marked(index)`, and `marked_in(&self, visible) -> Vec<usize>` (marked, in
       display order).
     - `targets(&self, visible, current, mode: ViewMode) -> Vec<usize>`: in `Grid`, `marked_in(visible)`
       if more than one is marked and `current` is marked; otherwise (and always in `Loupe`)
       `vec![current]` (§2.6).
  2. `grid.rs`: `grid_key_action(key, ctrl, shift, alt)` (new `shift` argument). New
     `GridKeyAction` variants: `Extend(Navigation)` for Shift + any navigation key, `SelectAll` for
     Ctrl+A (no Shift, no Alt), `ClearSelection` for Escape (no modifier). Shift with Enter, G or Space
     → `None`; every other Ctrl or Alt combination → `None` as today. `loupe_key_action` and the
     loupe's `key_action` are unchanged (Ctrl+A stays `None` there).
  3. `tabs.rs`: `Tab` gets `pub selection: Selection`; `Tab::open` clears it (anchor `None`).
     `Tabs::active_mut(&mut self) -> Option<&mut Tab>`.
  4. `main.rs`: pass `shift` to `grid_key_action`; the new variants do nothing yet.
- **Rules:** as T51.
- **Tests to add:** the `app.js` cases with 12 images, all visible: click 5, Shift+click 8 → 4
  marked; Ctrl+click 10 → 5 marked; Ctrl+click on the only marked image keeps it; Ctrl+click with
  nothing marked marks current and index; Shift range backwards; Shift from an anchor that is no
  longer visible; `all` then `clear`; `retain_visible`; `targets` in grid with current marked, with
  current not marked, with one marked, and in the loupe; `extend_to` twice from the same anchor
  replaces the range. Key map: every navigation key with Shift → `Extend`, Ctrl+A → `SelectAll`,
  Ctrl+Shift+A and Alt+A → `None`, Escape → `ClearSelection`, Shift+Enter → `None`; the existing key
  tests keep passing with `shift = false`.
- **Done when:** `.\tools\check.ps1` is all OK.

### T53. ADR 0014 F3: one rating key on several images, and the batch failure text (plain Rust) — Done

- **Review 2026-11-10: accepted.** Commit together with T54.

- **Can start now.** Independent of T51, T52 and T54.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/ratings.rs`, `crates/pw-loupe/src/metadata.rs`,
  and `crates/pw-loupe/src/main.rs` **only** to use the new failure text (point 4). Not `pw-metadata`.
- **Why:** ADR 0014 §2.7, §4.3, Q6. `apply` toggles per image, so several targets need one decision
  for the whole batch.
- **What:**
  1. `RatingStore::set_many(&mut self, paths: &[PathBuf], key: RatingKey) -> Result<usize, RatingRefused>`:
     - Paths without `supports_rating` are skipped. If none is left → `Err(RatingRefused::Unsupported)`,
       nothing changes. Resolve every remaining path to absolute first; if one fails, return that error
       before anything changes.
     - Decide **once**, from the shown value of each ratable path (as `set` reads it): `Label(l)` →
       if all have `l`, clear the label on all, otherwise set `l` on all; `Reject` → if all have −1,
       clear the stars on all, otherwise set −1 on all; `Stars(n)` and `ClearLabel` → as `apply`. The
       other half of each rating is kept.
     - Then, for each path in the given order, the same as `set` with that explicit value: `shown` at
       once, an unsent queued write for the path is replaced, otherwise a new write is queued. Share
       this with `set` through a private helper rather than copying it.
     - Returns the number of images set.
  2. With one path, `set_many` gives the same result as `set` for every key (the toggle included).
  3. Failure text (Q6): `RatingStore` counts write failures since the last `set` or `set_many` and
     keeps the file name and error of the last one. `pub fn failure_text(&self) -> Option<String>`:
     none → `None`; one → today's text "Kunde inte spara betyget för {fil}: {fel}"; more →
     "Kunde inte spara betyget för {n} bilder. Senast: {fil}: {fel}". `set` and `set_many` reset the
     count. The strings returned by `pump`, `written` and `fail_sent` are unchanged (the exit list of
     T44 uses them).
  4. `MetadataCache`: `set_ratings(&mut self, paths: &[PathBuf], key) -> Result<usize, RatingRefused>`
     and `failure_text()`, passing through. `main.rs`: where the last message of `poll` is put in the
     status line, use `failure_text()` instead when `poll` returned a failure. No other change.
- **Rules:** as T42; rules 6, 7, 10–12. Existing tests are not changed.
- **Tests to add** (with the existing fake writer): the table of point 1 — label on a mix → set on all;
  label on all that have it → cleared on all; X on a mix → −1 on all; X on all rejected → cleared;
  `Stars(3)` and `ClearLabel` on several; the other half kept in each case; an unsupported file
  (`x.png`) among the targets is skipped; only unsupported → `Unsupported` and nothing queued; order of
  writes follows the paths; two batches before a `pump` coalesce per path; one failed image of a batch
  rolls back only that image; `set_many` with one path equals `set` for every key on a few starting
  values; `failure_text` for 0, 1 and 3 failures, and reset by the next key. The `app.js` case: five
  marked images (one unsupported), label Yellow → four writes with Yellow; Yellow again → four writes
  without a label.
- **Done when:** `.\tools\check.ps1` is all OK.

### T54. ADR 0014 F4: pw-viewcore keeps its caches when the file list changes — Done

- **Second review 2026-11-10: accepted.** The review point below is done. Commit together with T53.

- **Review 2026-11-10: changes requested (one point).** `set_files` is accepted as written: the
  remap by path, the release of removed paths with exact byte counts, `lent`/`detached` so a stale
  render never touches a session under a key of the new order, `last_draw`, and the eight tests.
  Still to do, in `crates/pw-viewcore/src/state.rs` only:
  1. **The generation in `running` also changes `open_folder`, and that needs a test.** Before, a job
     of the old folder still running blocked the job with the same key in the new folder (`pick`
     skipped `Open(key)` while any job with that key ran), so the new folder's first image waited for
     the old folder's unpack. Now it starts at once. That is better and is accepted, but it is a
     behaviour change outside `set_files` ("`open_folder` keeps its behaviour"), and every behaviour
     change gets a test. Add one: an `Open` picked before `open_folder` is still running; after
     `open_folder`, `pick` hands out the `Open` of the same position in the new folder; when the old
     one finishes, it is discarded and the new one is kept. Mention the change in your log row.
  Then run `.\tools\check.ps1`.

- **Can start now.** Independent of T51–T53. **For a Claude agent** (the cache bookkeeping of
  `pw-viewcore`).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `crates/pw-viewcore/src/core.rs`. Not `pw-loupe`.
- **Why:** ADR 0014 §4.4, Q3 (way C). The loupe gets the visible list. When it changes (an image
  drops out after a rating, or the filter changes), images already opened and rendered must be kept
  and prefetch must follow the new order. Today every cache is keyed by position
  (`Key = (usize, Source)`) and `open_folder` empties them all.
- **What:**
  1. `State::set_files(&mut self, files: Vec<PathBuf>, current: usize)` and
     `ViewCore::set_files(&self, files, current)` (through `act`, like `open_folder`).
  2. Same list (equal paths in the same order) → only `go(current)`; nothing else changes.
  3. Otherwise:
     - **Finished state is moved to the new positions by path:** `sizes`, `failed`, owned sessions
       (`Some`) with `session_bytes_of` and `session_lru`, `resident` (`pos`), and `ever_resident`.
       Resident views keep their **ids**, so the textures in `pw-loupe` stay valid and need no upload.
     - **State of paths no longer in the list is released** the way eviction does: resident ids go to
       `take_dropped`, sessions are dropped and their bytes subtracted, `texture_bytes` and
       `session_bytes` stay exact.
     - **Work in flight** (`running`, `inflight`, `outbox`, lent sessions) belongs to the old order.
       You may start a new generation for it, as `open_folder` does, as long as the moved resident
       views still serve (they are checked against the generation in `serves`). The rule: **a result
       of the old order must never be stored, shown, or remove anything under a key of the new
       order.** Note that today a stale `finish_render` removes `sessions[key]`; after a remap that key
       can belong to another image. If lent sessions need to remember the generation they were lent in,
       change that inside `state.rs`.
     - `cur` becomes `current` (clamped); zoom, centre, direction and display mode are kept. If the
       current image is the same path as before, the view on screen stays until a new one is ready
       (no flash, as `the_view_on_screen_is_kept_while_the_next_is_prepared`).
  4. `open_folder` keeps its behaviour.
  5. If a structure cannot be remapped safely in this way, **stop and report** instead of choosing
     another design.
- **Rules:** as the other tasks; no `unwrap`/`expect`/`panic!` or unchecked indexing in non-test code;
  rules 6, 7, 10–12. Existing `pw-viewcore` tests are not changed.
- **Tests to add** (in `state.rs`, with the existing helpers):
  - a resident view of an image that stays moves to its new position with the same id, and `frame`
    draws it without a new render;
  - removing an image releases its resident id (`take_dropped`) and its session bytes;
  - after `set_files`, `wanted` and `plan` use the neighbours of the **new** list (+1, +2 are the next
    paths in the new order);
  - a render lent before `set_files` and finished after it is discarded, leaves the session of the
    image now at that position alone, and leaves `session_bytes` exact;
  - an open finished after `set_files` is discarded;
  - the same list → nothing dropped, generation unchanged;
  - the current image unchanged → its view keeps being drawn; zoom kept;
  - an empty list → no current image, everything released.
- **Done when:** `.\tools\check.ps1` is all OK.

### T55. ADR 0014 F5a: browse logic of a tab under a filter (plain Rust) — Open

- **Start after T51 and T52 are done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/tabs.rs` (or a new `browse.rs` with an
  `impl Tab`, declared in `lib.rs`), `crates/pw-loupe/src/thumbs.rs`, `crates/pw-loupe/src/metadata.rs`,
  `crates/pw-loupe/src/lib.rs` (texts and re-exports). **Not** `main.rs`.
- **Why:** ADR 0014 §2.2–§2.4, §4.2, §5.3, Q5, Q7; GR11. Everything T56 needs to draw the grid under a
  filter, as tested plain Rust, so `main.rs` only calls it.
- **What:**
  1. `Tab` gets `pub visible: Vec<usize>` (file indexes shown, in order). It is set only by the
     methods below, never by drawing.
  2. `Tab::refilter(&mut self, ratings_ready: bool, rating_of: &dyn Fn(&Path) -> Option<Rating>, keep: Option<usize>)`:
     - without an active filter: all indexes;
     - with an active filter and `ratings_ready == false`: empty (Q7);
     - otherwise `filter::visible` with `keep`;
     - then `selection.retain_visible`, and if `current` is not visible, `current` =
       `nearest_visible` (unchanged when nothing is visible).
     Returns whether `visible` changed.
  3. Moves (each ends with `refilter` keeping the **new** current, §4.2):
     - `grid_move(&mut self, nav: Navigation, extend: bool, columns, visible_rows, …)`: the new
       position from `grid::navigate` over `visible.len()`; `extend` → `selection.extend_to`, otherwise
       `selection.click(new)` (plain moves clear the marking);
     - `click(&mut self, index, ctrl: bool, shift: bool, …)`: Shift → `extend_to`, Ctrl → `ctrl_click`,
       neither → `click`; then `current = index`;
     - `loupe_move(&mut self, to: LoupeMove, …)` with `LoupeMove::{Next, Previous, First, Last}` over
       the visible list; clears the marking (Q5).
     Each returns the new current file index.
  4. `Tab::position(&self) -> Option<usize>` (position of `current` in `visible`).
  5. `MetadataCache::ratings_ready(&self) -> bool`: false while the folder read is sent or waiting to
     be sent (`load_request` or `pending_read`), true otherwise; `read_error(&self) -> Option<String>`
     when the read failed (Q7: an active filter then shows nothing, and the status line gives the
     error). Treat a read error as `ratings_ready == false` in `refilter`'s caller (say so in a doc
     comment).
  6. `thumbs.rs`: `ThumbGrid::to_request_indexes(&self, indexes: &[usize], cell) -> Vec<usize>` and
     `forget_not_in(&mut self, keep: &[usize]) -> Vec<usize>`, with the same rules as `to_request` and
     `forget_outside`. The range methods stay.
  7. Texts (`lib.rs`, ADR 0014 §5.3): `position_text(pos: usize, shown: usize, total: usize) -> String`
     ("3 / 12 (av 31)", or "3 / 31" when `shown == total`); `no_match_text(total)` → "Inga bilder
     matchar filtret (31 i mappen)"; `marked_text(n) -> Option<String>` → "12 markerade" for n > 1;
     `pub const READING_RATINGS_TEXT: &str = "Läser betyg …";`.
- **Rules:** as T51.
- **Tests to add:** port these checks of `ui/tests/filter-select.test.js` with 12 files `IMG_000.ARW` …
  `IMG_011.ARW` and the ratings 1: 3 stars, 2: 5 stars Red, 3: rejected, 4: Red only (ADR 0014 S6):
  all 12 visible; ≥ 3 → [1, 2] and position text "1 / 2 (av 12)"; ≥ 3 and Red → [2]; Red → [2, 4];
  Purple → empty and the no-match text; Rejected → [3]; Unrated → [0, 4, 5, …, 11]; under Unrated, a
  rating of the current image keeps it visible, a move right drops it and the next one is current;
  click 5, Shift+click 8 → 4 marked; Ctrl+click 10 → 5; Escape (`selection.clear`) → none;
  Shift+Left twice → 3; `all` → 12; a plain Left clears. Also: an active filter before the ratings are
  read shows nothing, and after the read the matching images; the filter hides the current image →
  the nearest following one; a marked image that the filter hides is unmarked; `loupe_move` clears the
  marking; `to_request_indexes` and `forget_not_in`; the four texts.
- **Done when:** `.\tools\check.ps1` is all OK.

### T56. ADR 0014 F5b: draw the filter and the marking in the grid, wire clicks and keys — Open

- **Note from the T53 review:** remove `#[cfg_attr(not(test), allow(dead_code))]` from
  `MetadataCache::set_ratings` when point 7 calls it.

- **Start after T53 and T55 are done.** **For a Claude agent** (drawing and wiring, checked by hand).
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/main.rs` only (a small plain helper with a test
  may go in the library).
- **Why:** ADR 0014 §3.1, §4.5, §5. Until T57, the loupe opens on the visible list with `open_folder`
  (ADR 0014 §9, way A).
- **What:**
  1. **Filter controls:** two `egui::ComboBox` in the tab bar row after "Importera…", listing
     `STAR_FILTERS` and `LABEL_FILTERS` with their texts. An active list has a yellow outline. A choice
     calls `Tabs::set_filter` and `refilter` (keep `None`, as `setFilter` in `app.js`); afterwards no
     widget keeps keyboard focus, so arrows and digits work at once.
  2. **When to refilter:** a filter change, a tab load, and when the active tab's ratings become ready
     (`ratings_ready` turns true). A rating key does **not** refilter (§2.2); moves do (T55).
  3. **The grid over `tab.visible`:** layout, rows, the visible range and scrolling use positions in
     `visible`; cells map to file indexes. Thumbnails: `to_request_indexes` for the file indexes of the
     visible positions; `set_visible` gets the range from the smallest to the largest of those file
     indexes (+1); `forget_not_in` with the file indexes of the keep range. With nothing visible, the
     grid shows `no_match_text` (or `READING_RATINGS_TEXT`, or the read error) in the middle.
  4. **Marked cells (§5.1):** blue outline `#4c8dff` and a faint blue fill behind the thumbnail; the
     current cell keeps its yellow outline (both when current and marked).
  5. **Clicks:** read Ctrl and Shift from the input; `Tab::click`. A double-click opens the loupe as
     today.
  6. **Keys in the grid:** `Navigate` and `Extend` → `grid_move`; `SelectAll` → `selection.all`;
     `ClearSelection` → `selection.clear`; scroll to the new current as today.
  7. **Rating keys:** targets from `selection.targets(visible, current, mode)`, then
     `metadata.set_ratings`; `Unsupported` gives `UNSUPPORTED_TEXT` as today.
  8. **Status line in the grid:** `position_text` (with `visible.len()` and `files.len()`),
     `marked_text`, the read notice; replaces today's "{name}  {n} / {total}".
  9. **Loupe (interim):** opening the loupe gives the core the paths of `visible` with `open_folder`
     and `go(position)`; where `main.rs` sets the tab's current from `frame.pos`, map the position
     through `visible`. With nothing visible, Enter/G/Space and a double-click do nothing.
- **Rules:** drawing reads `tab.visible`, `RatingStore` and `Selection` only, never files; no new
  dependencies; rules 6, 7, 10–12.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (ADR 0014 §8.2): each
  filter; an empty filter; two tabs with different filters; under "Ej betygsatt" a rated image stays
  until moving on; Shift+click, Shift+arrows, Ctrl+click, a label pressed twice on a marking; Ctrl+A;
  Escape; thumbnails appear at once when the filter changes (S1).

### T57. ADR 0014 F6: the loupe steps and prefetches within the filter — Open

- **Start after T54 and T56 are done.** **For a Claude agent.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` for the
  status line.
- **Why:** ADR 0014 §2.2, §2.3, §4.4 (way C), S2, S3.
- **What:**
  1. Loupe steps (Next, Previous, First, Last) go through `Tab::loupe_move` instead of
     `core.step`/`go(0)`/`last`. If `visible` changed, call `core.set_files(paths of visible,
     position)`; otherwise `core.go(position)`.
  2. A filter change while the loupe is shown: `refilter`, then `set_files`; if nothing is visible,
     return to the grid (§2.3).
  3. The tab's current image is taken from the tab, not from `frame.pos`, while the loupe is shown
     (the core follows the tab).
  4. Status line: `status_line` gets the folder's total and uses `position_text`
     ("3 / 12 (av 31)"); update its test.
  5. Remove the interim `open_folder` of T56 point 9 for changes of the visible list; opening the loupe
     from the grid and a tab load still use `open_folder`.
- **Rules:** as T56.
- **Tests to add:** the `app.js` case "loupe within filter": Red, Home, Enter, step right → IMG_002 then
  IMG_004 (on `Tab::loupe_move`); the status-line text with and without a filter.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: under "≥ 3 stjärnor"
  stepping stays in the filter and holding an arrow is smooth; under "Ej betygsatt" a rated image stays
  until the next step and the step after it is not a cold load; `latency.ps1` under the filter for V1
  (S3); a batch of 30 then closing the window at once keeps the values (ADR 0013 R4).

### T58. ADR 0014 F8: documentation after milestone 5 — Open

- **Start after the owner's acceptance (F7).** Documentation only; no code, no `Cargo.toml`. For a
  Claude agent.
- **Files:** `docs/adr/0014-egui-filter-and-multi-selection.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/architecture.md`, `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`,
  `docs/test-inventory.md` (only if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T51–T57 and the owner's F7 notes in this file; write only what
  they record.
- **What:**
  1. **ADR 0014:** "Implemented" status line; §7 results for S1–S6; §9 results per step; a short
     "Findings during implementation" section if the log has any.
  2. **ADR 0004** addendum "filter and multi-selection": a note that the egui program implements it
     (ADR 0014), with the differences: filter per tab, not saved between sessions, "Alla utom
     avvisade".
  3. **architecture.md:** `pw-loupe` (filter, selection, batch ratings) and `pw-viewcore`
     (`set_files`).
  4. **GUI_REQUIREMENTS.md** GR8 and the root `README.md`: filter and multi-selection done; left
     before the switch: IPTC metadata, TIFF viewing.
  5. **docs/README.md:** "0014 milestone 5: filter and multi-selection in egui – done {date}".
- **Rules:** English, the style of the existing documents; change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

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
- ADR 0010 steps M3–M7 (the rest of the import) wait until the contact sheet (ADR 0011) is done
  (owner decision, 2026-11-08). M4 and M7 are added to this list then.
- ADR 0011 step K4 (drawing the contact sheet) is planned after T23–T25.
- Moving rejected or picked images to `_rejected` / `_picked` folders (ADR 0013 §13): needs its own
  ADR first.
- Note for M5: when the import opens its folder with `Tabs::open_new`, compare folders by their
  resolved form (`pw_ingest::resolve`), so `C:\Foto` and `c:\foto\` do not open two tabs.

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
| 2026-11-08 | T20 | Ready for review | Moved card detection and import request validation into `pw-ingest`; Tauri calls the shared functions. Added validation and DCIM detection tests. `cargo test -p pw-ingest -p photoworkflow`: pw-ingest 25 unit + 7 integration passed (1 ignored), photoworkflow 5 passed; fmt and clippy clean. |
| 2026-11-08 | T21 | Ready for review | Added the plain-Rust tab model, tab shortcuts and egui tab bar; tabs remember the current image, Ctrl+drop opens a tab, and the final tab clears instead of closing. `cargo test -p pw-loupe`: 13 library + 3 binary tests passed; workspace check: 192 passed, 23 ignored, 0 failed; fmt and clippy clean. Manual desktop acceptance remains for the owner. |
| 2026-11-08 | T20 | Done | Reviewed: logic and messages moved unchanged; Tauri calls `validate_source`, `validate_options` and `ingest_volumes` in pw-ingest. Deviations from the task (String return, seven positional arguments, some tests missing) go to T22 |
| 2026-11-08 | T21 | Done (manual test pending) | Reviewed: tab model with 6 tests, tab keys with a test, tab bar, Ctrl+drop; `frame.pos` is read after the core's state is updated, so each tab remembers its own image. Workspace: 192 passed, 23 ignored, 0 failed. Owner checks tabs by hand |
| 2026-11-08 | T22 | Open | Added: follow-ups from the T20 review |
| 2026-11-08 | T21 | Manual test | Owner: Ctrl+drop did not open a new tab (Windows does not report Ctrl while dragging from Explorer), so tab switching could not be tried; closing works. Owner decided: dropping always opens a new tab. Reviewer changed `main.rs` (drop always uses `open_new`) and `Tabs::open_new` (fills an empty active tab), added a test and adjusted the setup of one test that relied on the old behaviour; ADR 0010 Q3 updated. Tab switching to be tested again |
| 2026-11-08 | T21 | Done | Owner's manual test after the drop fix: new tabs on drop, switching with keys and mouse, each tab returns to its own image, closing works |
| 2026-11-08 | – | Decision | Owner accepted ADR 0011 (contact sheet) with Q1–Q6 as proposed and C1–C5 as behaviour requirements; GR13 added. The rest of ADR 0010 (M3–M7) follows after the contact sheet |
| 2026-11-08 | T23–T25 | Open | Added: ADR 0011 steps K1 (thumbnail extraction), K2 (worker and cache), K3 (grid model) |
| 2026-11-08 | T23 | Ready for review | Added preview choice, scaling, EXIF orientation and JPEG/embedded-preview extraction in pw-thumbcache. Unit tests and ignored ARW/RW2/DNG/A7R IVA real-file tests pass; workspace checks with and without ignored tests, fmt and clippy are clean. The 1616×1080/cell 200 example selects Quarter via a 2× decode threshold. |
| 2026-11-08 | T25 | Ready for review | Added plain-Rust grid layout, visible range, navigation, grid/loupe key actions and thumbnail-size helpers; added per-tab view mode defaulting to Grid and tests for mode persistence across tab switches. |
| 2026-11-08 | T24 | Ready for review | Added a two-thread thumbnail worker pool, generation-aware request queue/cancellation, visible-range priority with nearby prefetch, pause/resume, per-thread decoders, and generation-scoped LRU cache. Simulated-source tests cover exact results, priority/drop, stale work, pause, cache hits/eviction, errors, and non-blocking drop. Crate and workspace fmt, tests and clippy pass. |
| 2026-11-08 | T22 | Ready for review | Changed card detection to return `PathBuf` values and made the Tauri command convert them to strings; replaced positional validation arguments with `OptionsInput`. Added DCIM-root and requested validation tests. Package and workspace checks pass. |
| 2026-11-08 | T22 | Done | Reviewed: `card_volumes() -> Vec<PathBuf>` with a testable `with_dcim`; `OptionsInput` struct; messages unchanged; all required tests present. Fixed in review: the existing test `identifies_volumes_with_a_dcim_directory` had been replaced, so it is restored (rule 5); the new `with_dcim` test now uses one temporary root and removes it. pw-ingest 30 passed in the cloud check (Linux), clippy clean |
| 2026-11-08 | T23 | Done | Reviewed: preview choice, orientation (all eight values checked by hand against the 2×3 test) and `thumbnail` match the spec; no RAW decoding. The task's example 1616×1080/200 → Quarter was wrong (1616/8 = 202 ≥ 200); the agent added a 2× rule to satisfy it. Fixed in review: `choose_scale` follows the spec (long edge ≥ cell); `cell` is in physical pixels, so K4 multiplies by the display scale on high-DPI screens; tests corrected with exact boundaries |
| 2026-11-08 | T24 | Done | Reviewed: two threads, generation and visible-range priority, pause, LRU cache with a byte limit, errors as results, poisoned locks recovered, drop does not join. Doc fix in review: after `cancel()` the UI must call `set_generation`. Note for K4: requests far outside the visible range are dropped without a result, so the UI re-requests visible cells that have no thumbnail when the visible range changes |
| 2026-11-08 | T25 | Done | Reviewed: columns, visible range, navigation (short last row), grid/loupe keys, size clamp 96–480 and Ctrl+wheel step 16; `ViewMode` per tab, new folders start in Grid; `key_action` and `tab_action` unchanged; main.rs not touched. Workspace check: fmt, test and clippy OK |
| 2026-11-08 | T26–T29 | Open | Added: ADR 0011 K4 split into four parts for the agent (duplicate-free requests, thumbnail bookkeeping, drawing the grid, switching grid/loupe and size). K5 (acceptance and first measurement) follows T29 |
| 2026-11-08 | T26 | Ready for review | Deduplicated queued and in-flight thumbnail requests by generation/index/cell, added simulated-source tests for duplicate handling, distinct cell sizes and completed cache hits, and re-exported `Rgb8`. `cargo fmt --all --check`, `cargo test -p pw-thumbcache`, `cargo clippy -p pw-thumbcache --all-targets` and `.\tools\check.ps1` pass. |
| 2026-11-08 | T27 | Ready for review | Added `ThumbGrid` state tracking and completion handling, 128-pixel physical cell sizing, RGB-to-egui conversion and tests. Crate fmt, tests and clippy pass. |
| 2026-11-09 | T26 | Done | Reviewed: queued and in-flight duplicates (generation, index, cell) are skipped; the in-flight entry is removed under the same lock that fills the cache, so a later request is a cache hit, never a gap; `Rgb8` re-exported. Four required tests present. pw-thumbcache 25 passed, 4 ignored; workspace fmt, test and clippy OK. No changes in review |
| 2026-11-09 | T27 | Done | Reviewed: `ThumbGrid` (to_request, mark_requested, apply, state, forget_outside), `request_cell` (128 px steps, bad scale counts as 1.0) and `color_image` match the spec; all required tests present; pw-thumbcache added as an internal dependency. Note: `ThumbnailEvent` carries no cell size, so `apply` records the cell of the latest request; a late result for an older size is shown until the newer one arrives and replaces it, which is harmless. Workspace fmt, test and clippy OK. No changes in review |
| 2026-11-09 | T28 | Done | Reviewed: grid in a vertical scroll area with `show_viewport`, requests only for the visible range, polling, textures released outside the visible range ± 3 rows, placeholders, error text, file names, frame on the current image, click and key navigation, repaint while thumbnails are pending; loupe path unchanged. Fixed in review: key navigation used the column count of the full image width while the drawing is narrower (scroll bar), so Down could land in the wrong column at some widths — the drawn column count is now remembered and used; the scroll area is limited to the image area so the last row no longer runs under the status line. Workspace fmt, test and clippy OK; owner checked by hand: grid fills in, keys, click, upright A7R IVA images, resizing |
| 2026-11-08 | T29 | Ready for review | Added grid/loupe switching by keys and double-click, per-tab core/thumbnail worker state, file-versus-folder startup mode, grid size slider and Ctrl+wheel resizing. |
| 2026-11-09 | T29 | Done | Reviewed: grid ↔ loupe by keys and double-click, per-tab mode, empty view core in grid mode, file opens in the loupe and folder in the grid, size slider, mode button only in the loupe. Fixed in review: switching mode reloaded the whole tab (new generation), so every visit to the loupe threw the thumbnails and their cache away; `switch_mode` now only pauses/resumes the thumbnail worker and fills/empties the view core (ADR 0011 §4.2). Owner feedback: one Ctrl + wheel notch changed the size too much, because egui spreads a notch over several frames of `zoom_delta()`; now the raw wheel events give exactly one step per notch (`take_wheel_steps`, tested) and `THUMBNAIL_SIZE_STEP` is 8 instead of 16 (owner request; the step test uses the new values). Owner kept wheel = scroll, Ctrl + wheel = size. Workspace fmt, test and clippy OK; owner checked by hand (ADR 0011 §8.2 steps for C3, C4) |
| 2026-11-09 | ADR 0011 K5, K6 | Done | K5 by the owner: grid, thumbnails and loupe switching all appear within the same second for the test folder; C3–C5 passed by hand. Owner set the targets C1 and C2 to within 1 s (also the first time after a restart). No disk cache needed (Q4). K6: architecture.md, test inventory (236 passed, 27 ignored), ADR 0010 (imports open as a contact sheet) and ADR 0011 updated |
| 2026-11-09 | ADR 0010 M3 | Done | Owner approved `rfd` =0.17.2 (MIT, `default-features = false`) for the folder dialog; its Windows dependencies are already in the workspace (GUI_REQUIREMENTS §6.1). Added T30–T32 (M4: form and preview summary, worker, dialog state machine). `rfd` is added in M5 |
| 2026-11-09 | T30 | Ready for review | Added the plain-Rust import form, shared `pw-ingest` validation, plan preview summary and result-folder selection, with the specified unit tests. |
| 2026-11-09 | – | – | Rule 7 added (owner's antivirus): build only in the project's `target` folder, never in a temporary target folder. During T30 the agent built in `%TEMP%\pw-loupe-t30-target` and the antivirus quarantined a build script there |
| 2026-11-09 | T30 | Done | Reviewed: `ImportForm` with the Tauri defaults; the field is chosen by which check fails (source, folder template, name template, then the rest as destination), never by the message text; messages are exactly those of `pw-ingest`; `Preview::new` matches the Tauri `preview_of`/`action` rules; `result_folder` follows Q2. All required tests present; pw-ingest added as an internal dependency. The first run failed to compile because the antivirus quarantined a build script in the agent's temporary target folder (rule 7); the owner's rerun: workspace fmt, test (245 passed, 27 ignored) and clippy OK. No changes in review |
| 2026-11-09 | T31 | Ready for review | Added the bounded background import worker, cached preview scans, fresh scans and execution for runs, progress/completion events, cancellation, and worker lifecycle tests. `.\tools\check.ps1` passed, including 42 pw-loupe library tests, 3 binary tests and workspace clippy. |
| 2026-11-09 | T31 | Done | Reviewed: one worker thread with a bounded request queue (full queue and stopped worker are errors), cached scan for previews and a fresh scan and plan for every run (as the Tauri `run`), progress and `Done` events with the Tauri summary and messages, cancel flag cleared per run behind a small gate so a new run never starts with an old flag, drop cancels and does not join. All required tests present, including no partial files after cancel. Comments added in review to explain the gate. Workspace fmt, test (251 passed, 27 ignored) and clippy OK |
| 2026-11-09 | T32 | Ready for review | Added the plain-Rust import dialog state machine, debounced preview requests with rescan tracking, request-ID filtering, progress/done/error/disconnect handling, cancellation/close checks, result-folder lookup and back-to-editing behavior, with transition tests. `.\tools\check.ps1` passed: pw-loupe 53 library tests and 3 binary tests; workspace fmt, tests and clippy OK. The separate `cargo test -p pw-loupe` and `cargo clippy -p pw-loupe --all-targets` commands failed while linking the thiserror build script (MSVC LNK1104); no pw-loupe process was running. |
| 2026-11-09 | T32 | Reviewed | The state machine matches the spec: 300 ms debounce, rescan only when the source changed, stale events ignored by request id, start only with a preview that has files to copy, disconnect turns previewing and running into errors, no close while running, result folder from T30, back to editing asks for a fresh scan. All required tests present. Finding: `tick()` and `start()` call `ImportForm::validate`, which touches the disk (exists check and canonicalise) on the UI thread; on a sleeping HDD this can freeze the window. Fixed in T33 (validation on the worker). The agent's `test-output.txt` predates the final T32 code, so the owner reruns `.\tools\check.ps1` before the commit. The agent also saw LNK1104 (linker could not open the thiserror build script) in separate cargo runs |
| 2026-11-09 | T32 | Done | Owner reran `.\tools\check.ps1` on the final code: fmt, test (262 passed, 27 ignored) and clippy OK; committed. Rule 6 changed: the agent runs `.\tools\check.ps1` (whole workspace) instead of single-crate `cargo test -p`/`clippy -p`, whose different feature set builds new build scripts that failed to link (LNK1104), most likely blocked by the antivirus. Noted in review: `rfd` 0.17.2 is already in `Cargo.lock` through `tauri-plugin-dialog`, so adding it to `pw-loupe` in M5 adds no new crate at all |
| 2026-11-09 | T33 | Ready for review | Import requests now carry the form; validation and file access run on the worker, and field errors are preserved through preview and dialog state. Added worker and dialog regression tests. |
| 2026-11-09 | T33 | Done | Reviewed: the worker takes the form and validates it first; preview errors keep their field (scan errors on `Source` with the Tauri message); a run with an invalid form ends with its message and copies nothing; `tick()` and `start()` no longer touch the disk. Only the calls of the existing tests changed, as allowed. The T31 test for a missing source now checks the validation message ("Källan … finns inte."), because validation now runs before the scan; the scan-error path ("Kunde inte läsa …") is unchanged code but no longer reached by a test, since a missing folder is caught first. Workspace fmt, test (266 passed, 27 ignored) and clippy OK. No changes in review. ADR 0010 M4 is complete |
| 2026-11-09 | T34 | Ready for review | Added worker-side card detection, import key actions, pure Swedish import texts and full tests for dialog transitions and text output. |
| 2026-11-09 | T34–T36 | Open | Added: ADR 0010 M5 split into card detection on the worker plus keys and Swedish texts (plain Rust), drawing the dialog with editing and preview (adds `rfd`), and running the import with progress, hide, summary and opening the result in a new tab. M6 (acceptance by the owner) follows T36 |
| 2026-11-09 | T34 | Done | Reviewed: `cards()` runs `card_volumes` on the worker; `can_start` (no side effects) and `rescan`; `opens_import` (Ctrl+I only) and `dialog_key_action` (in `lib.rs`, next to the other key maps); Swedish texts match the Tauri dialog, with `pw_core::format_bytes`. All required tests present. Fixed in review: the speed was truncated (4.6 MB/s showed as 4); it is now rounded as `toFixed(0)` in the Tauri dialog, with a test. Workspace fmt, test (280 passed, 27 ignored) and clippy OK before the fix; the owner reruns `.\tools\check.ps1` |
| 2026-11-09 | T35 | Ready for review | Added the centred import dialog, folder and card selection, editable form, field validation errors, debounced worker previews and Ctrl+I/tab-bar opening; preserved settings when closing. `.\tools\check.ps1` passed (fmt, workspace tests and clippy); Cargo.lock only adds `rfd` to `pw-loupe`. Owner's manual GUI checks remain. |
| 2026-11-09 | T35 | Done | Reviewed: `rfd` added as approved and `Cargo.lock` only lists it under `pw-loupe`; Ctrl+I and the "Importera…" button open the dialog; cards are found on the worker; folder dialogs, field errors under the right field, preview on the worker; keys go only to the dialog while it is open; settings kept for the session. Fixed in review: the heading "Preview" is now "Förhandsvisning"; owner feedback: "Välj…" always opened Documents, so the folder dialog now starts in the folder typed in the field, else (for the source) the first detected card. Owner checked by hand: everything else works. Note: the first hand test ran an old `pw-loupe.exe`, because `cargo build -p pw-loupe` fails on the `thiserror` build script; use `cargo build --workspace` |
| 2026-11-09 | T36 | Ready for review | Added worker-backed import start, byte progress, cancellation and hide/show during copying, completion summaries, and one-time opening of the imported folder in a new grid tab. |
| 2026-11-09 | T36 | Done | Reviewed: start by button or Enter only when `can_start`; copy on the worker; fields locked, progress bar with `progress_text`, "Avbryt" and "Dölj" while running, no close; the worker is polled every frame, also while hidden, and the status line shows the progress; on `Done` the result folder opens once in a new tab (grid), none when nothing was copied; "Ny import" and "Stäng". No changes in review. Workspace fmt, test (281 passed, 27 ignored) and clippy OK |
| 2026-11-09 | ADR 0010 M6 | Done | Owner acceptance of step A with a real card: full import into an empty folder (progress, summary, new tab as contact sheet), cancel (summary "Avbruten", no partial files), "Dölj" during the copy (grid and loupe stay smooth), second import (files present, no new tab). All passed |
| 2026-11-09 | T37–T40 | Open | Added: ADR 0010 M7 (step B) as four parts: backup destination, job name and first number, IPTC template on the new copies (ExifTool on the import worker), eject the card (tabs on the card are emptied first). M8 (documentation) is done by the reviewer after T40 |
| 2026-11-09 | T37 | Ready for review | Added the optional backup destination to the import form and dialog, with destination-first validation and coverage for disabled, empty, valid and invalid backup paths. |
| 2026-11-09 | T37 | Reviewed | `backup_on` and `backup` in the form; the field of an error is found by validating without and then with the backup, never by the message text; an empty or disabled backup is `None`; checkbox, field and "Välj backupmål" in the dialog, disabled when off. All required tests present; the defaults test now includes the two new fields. Workspace fmt, test (287 passed, 27 ignored) and clippy OK. No changes in review; waits for the owner's hand test |
| 2026-11-09 | T38 | Ready for review | Added job and first sequence number to the import form and options; added the job/number controls and template-variable help to the dialog; validation tests cover defaults, passed values and job trimming. |
| 2026-11-09 | T37 | Done | Owner moved on to T38 after the hand test of the backup (both folders get the same files) |
| 2026-11-09 | T38 | Reviewed | `job` and `first_seq` in the form and passed to `Options` (job trimmed by `pw-ingest`); dialog row "Jobb" and "Första nr" (`DragValue` 0–999 999) and the variable help, matching the `pw_ingest::template` list. All required tests present. Workspace fmt, test (289 passed, 27 ignored) and clippy OK. No changes in review; waits for the owner's hand test |
| 2026-11-09 | T39 | Ready for review | Added the six-field IPTC template, worker-thread ExifTool writes for copied images in the destination and backup, cancellation, progress, errors and UI; added mapping, target-selection, lazy-start, cancellation and text tests. Workspace check passed; the real metadata write remains for the owner's hand test. |
| 2026-11-09 | T38 | Done | Owner checked by hand: `{job}` and `{seq:4}` give the expected names |
| 2026-11-09 | T39 | Reviewed | `IptcForm` with the six fields of the Tauri dialog and `to_iptc` (copyright as `x-default`); the worker writes the template after the copy, not when cancelled or empty, to copied images (no companions, no failures) in the destination and the backup, as the Tauri `run`; ExifTool is started on the worker only when needed and kept; progress, cancel per file and up to 10 "IPTC:" lines; collapsing "IPTC-mall" section with the help text. All required tests present. Note: an ExifTool start error is listed with the first file's name in front; harmless. Workspace fmt, test (296 passed, 27 ignored) and clippy OK. No changes in review; waits for the owner's hand test |
| 2026-11-09 | T40 | Ready for review | Added the optional eject checkbox, worker-thread eject after a complete import, cancellation/failure messaging, clearing card-backed tabs before starting, eject progress and card-list refresh. Added state, path-boundary, worker, and text tests; real-card eject remains for owner hand testing. |
| 2026-11-09 | T39 | Done | Owner checked by hand: Fotograf and Copyright are in the sidecar of a RAW and in a DNG/JPG; committed |
| 2026-11-09 | T40 | Reviewed | `eject` in the form; the worker ejects only after a complete import (not cancelled, no failures, no error), forgets the cached scan and reports `pw_ingest::eject`'s message, otherwise "Kortet matades inte ut eftersom importen inte blev komplett."; `Tabs::clear_inside` compares path components without file access (case-insensitive on Windows, `DCIM2` is not inside `DCIM`); tabs on the card are emptied when the run starts; "Matar ut …" progress; the card list is refreshed after the eject. All required tests present. Fixed in review: the source path is trimmed before the tab comparison, as validation trims it. Workspace fmt, test (304 passed, 27 ignored) and clippy OK before the fix; waits for the owner's check and hand test |
| 2026-11-09 | T40 | Done | Owner checked by hand: an import with the checkbox empties the tab showing the card, then ejects it (the card disappears from Explorer; Windows shows no notice for the Eject verb, as in the Tauri app); a cancelled import does not eject and says why. Workspace check after the review fix: fmt, test (304 passed, 27 ignored) and clippy OK |
| 2026-11-09 | ADR 0010 M8 | Done | Documentation: ADR 0005 (egui dialog section), ADR 0010 (all steps done), architecture.md (`pw-ingest`, `pw-loupe`), test inventory (304 passed, 27 ignored). ADR 0010 is complete |
| 2026-11-09 | ADR 0013 B0 | Done | Owner accepted ADR 0013 (milestone 4: ratings and labels) and every proposal in §11. Direction for later: X moves the image to `_rejected`, another key to `_picked`, possibly as a menu option (ADR 0013 §13) |
| 2026-11-09 | T41–T44 | Open | Added: ADR 0013 steps B1–B4 |
| 2026-11-09 | T41 | Ready for review | (Agent) Added the rating key map and pure `apply` function per ADR 0013 B1, with tests for shortcuts, ignored modifiers, existing key maps, and rating/label/reject updates. Updated the test inventory. Workspace fmt, test (307 passed, 27 ignored) and clippy OK. |
| 2026-11-09 | T41 | Changes requested | Reviewed: key map and `apply` correct (`Stars` as `u8` accepted). Missing: point 4 (`label_name`, `status_text`, `star_glyphs`, `UNSUPPORTED_TEXT`) and several tests. The agent had also replaced this file with an older copy (T41 rewritten, T42–T44, a note and two log rows removed); restored. Rules 10–12 added under "How to work on a task" |
| 2026-11-09 | T41 | Ready for review | Reviewer (Claude) finished point 4 and the missing tests in `ratings.rs` (11 tests, was 3) and re-exported the texts in `lib.rs`; test inventory updated (pw-loupe 109, total 315). Checked in the sandbox with a stand-in for `egui::Key` (crates.io not reachable there): rustfmt clean, 11 tests pass, clippy without findings in `ratings.rs`. **Not yet run on Windows:** the owner runs `.\tools\check.ps1` |
| 2026-11-09 | T41 | Done | Owner's `check.ps1`: fmt, test (315 passed, 27 ignored, 0 failed; all 11 `ratings` tests) and clippy OK. T42 can start |
| 2026-11-09 | T42 | Ready for review | Added the optimistic `RatingStore` and testable writer queue, wired read/write events and polling into `MetadataCache`, preserved ratings across folder loads, pruned closed-tab ratings, and localized metadata notices. Added queue, rollback, stale-read, pruning and cache tests; updated the existing metadata notice expectations and test inventory. Workspace fmt, test (330 passed, 27 ignored) and clippy OK; no commit. |
| 2026-11-09 | T42 | Done | Reviewed: `RatingStore` follows ADR 0013 §4.2 rules 1–6 (coalescing of unsent writes, back-pressure, rollback that respects newer writes, read merge by request id, prune keeps pending writes); `MetadataCache` keeps only load states; Swedish notices; 15 store tests + 2 cache tests. Owner's `check.ps1`: fmt, test (330 passed, 27 ignored, 0 failed) and clippy OK. Gap in the task text: a folder read refused with `QueueFull` becomes an error → T45. Minor, no action: a cleared rating is stored as an empty `Some(Rating)` after a write but as `None` after a read (both draw the same); tabs emptied by the import's eject are not pruned (harmless). The agent changed this file only as rule 11 allows. The T41 "Done" status and its log row were missing because the reviewer's own earlier write of this file did not land (a file-transfer problem on the reviewer's side, not the agent); restored above |
| 2026-11-09 | T45 | Open | Added: retry the folder read on `QueueFull` (T42 follow-up) |
| 2026-11-09 | T43 | Ready for review | Wired rating shortcuts with keyboard-focus/import-dialog guards; added grid rating rows, label colours, rejected-thumbnail dimming and the loupe badge. `MetadataCache` exposes the store's set/shown operations; glyphs are drawn as shapes. Added a palette test and updated the test inventory. Workspace fmt, tests (331 passed, 27 ignored) and clippy OK. Owner's manual acceptance remains pending; no commit. |
| 2026-11-09 | T43 | Changes requested | Reviewed: rating keys sit after the dialog and tab checks and before the mode keys, skipped while egui wants keyboard input; act on `tab.current`; refusal shows `UNSUPPORTED_TEXT`. Grid row (glyphs left, name left-aligned, label colour, dark text on Yellow/Green), 40 % dimming of rejected thumbnails, loupe badge only with the I information. Owner's `check.ps1`: fmt, test (331 passed, 27 ignored, 0 failed) and clippy OK. Requested: the star shape is filled as a convex polygon although it is concave (notches filled); measure the badge text; the label name in the badge is not in ADR 0013 §5.2 (owner decides). Note: two small accessors were added to `metadata.rs`, which the task did not name (rule 2 says stop and ask); accepted, as the store lives inside `MetadataCache` |
| 2026-11-09 | T45 | Ready for review | On `QueueFull`, `MetadataCache` retains the active folder read and retries it in `poll` before rating writes; `Stopped` still reports the existing load error. Added five fake-worker tests for loading state, retry/read completion, stopped workers, replacing pending folders and read/write order. Updated the test inventory. Workspace fmt, tests (336 passed, 27 ignored) and clippy OK; no commit. |
| 2026-11-09 | T43 | Ready for review | (Agent) Review fixes in `main.rs` only: the star is drawn as ten centre triangles in an `egui::Mesh`; the badge's label text is measured with `painter.layout_no_wrap` and drawn as that galley. The Swedish label name in the badge is kept for the owner to look at during the hand check (point 3), then kept or removed. No test changes. Workspace fmt, test (336 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-09 | T45 | Done | Reviewed: `MetadataSubmit` trait (read + events) over `MetadataWorker`; `QueueFull` keeps the read pending and the images loading; `poll` retries it after draining events and before `pump`, so a freed slot goes to the read first; `Stopped` fails the load as before; a new `begin_load` and `fail_loading` clear the pending read. Five tests cover every point of the task. Owner's `check.ps1`: fmt, test (336 passed, 27 ignored, 0 failed) and clippy OK |
| 2026-11-09 | T43 | Ready for review | Second review: the star is ten centre triangles in an `egui::Mesh` (indices checked: centre 0, ring 1–10, wrapping); the badge text is measured with `layout_no_wrap` and drawn as that galley. Accepted; meshes are not anti-aliased, so the stars may look slightly hard-edged at small sizes (owner judges in the hand check). Point 3 (label name in the badge) is the owner's choice. Waiting for the hand check |
| 2026-11-09 | T39 | Done | Heading corrected to Done (the log already said Done; found by a reviewer) |
| 2026-11-09 | T46 | Open | Added: owner wants the label colour inset and rounded (pill) in the grid row and the loupe badge |
| 2026-11-09 | T44 | Ready for review | (Agent) Started at the owner's request while T43 waits for the hand check. `metadata.rs`: `finish_pending_writes` (injectable clock and sleep; polls every 20 ms until no write is pending or 5 s have passed; returns the failures and the still-pending file names), `unsaved_list_text` (20 lines, then "… och {n} till"), `pending_writes` accessor; the test `FakeSubmit` can now answer writes (new optional field, existing tests unchanged). `main.rs`: `on_exit` runs the wait and shows one `rfd::MessageDialog` titled "Betyg som inte sparades" (available with the pinned features). Failed writes are listed with their existing message (it names the file), pending ones by file name. The window probably stays visible during the wait (`on_exit` runs before eframe destroys it); not checked by hand. 5 tests added; test inventory updated. Workspace fmt, test (341 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-09 | T46 | Ready for review | (Agent) Started at the owner's request while T43 waits for the hand check; T44 is still uncommitted, so `main.rs` holds both. `main.rs` only: `label_pill` insets the row 6 pt sideways and 2 pt vertically; the label colour is drawn there with corner radius = half its height; stars and name start 6 pt further in; the name is still clipped to the row. The loupe badge (label colour and dark background) has radius = half its height. Colours, dark text, dimming, cell size and selection frame unchanged. 1 test for `label_pill`; test inventory updated (pw-loupe 136). Workspace fmt, test (342 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-09 | T44 | Ready for review | Reviewed (code): `finish_pending_writes` returns at once with nothing pending, polls every 20 ms, stops at 5 s and lists failures and still-pending files; `unsaved_list_text` caps at 20 lines; `on_exit` shows one `rfd::MessageDialog` only when something was not saved. Tests cover the task's list. Owner's `check.ps1`: fmt, test (342 passed, 27 ignored, 0 failed) and clippy OK. Not Done: the owner's first start of the program showed nothing; nothing in T44 or T46 runs at start-up, so the cause is not visible in the diff. Waiting for a start from a console (debug build prints the error) and the hand check |
| 2026-11-09 | T46 | Ready for review | Reviewed (code): `label_pill` = row shrunk 6 pt sideways and 2 pt vertically (tested); label colour drawn there with radius = half its height; stars and name moved inside; badge rounded the same way. Accepted; waiting for the hand check (see T44) |
| 2026-11-09 | T47 | Open | Added: since T44 `pw-loupe.exe` exits at start with 0xC0000139 (entry point not found); T43+T45 build starts. Likely `rfd`'s message box needs Common Controls 6, which `pw-loupe` has no manifest for. Blocks the hand checks of T43, T44, T46 |
| 2026-11-09 | T47 | Ready for review | (Agent) Confirmed: `cargo tree --workspace -e features -i rfd` shows `rfd` feature `common-controls-v6` enabled by `tauri-plugin-dialog` v2.8.4. New `crates/pw-loupe/build.rs`: on windows + msvc only, `/MANIFEST:EMBED` and the Common Controls 6.0.0.0 `/MANIFESTDEPENDENCY` for the bins, plus `rerun-if-changed=build.rs`; reads the variables with `.ok()`. `Cargo.toml` unchanged. `cargo build --workspace` OK (not blocked); `pw-loupe.exe D:\PhotoWorkflowData\RAW` opened the "RawStride" window, closed with `CloseMainWindow`, exit code 0. Workspace fmt, test (342 passed, 27 ignored, 0 failed) and clippy OK; no commit. Owner's hand checks (rating at exit, read-only sidecar message box) not done |
| 2026-11-09 | T48 | Open | Added by the agent at the owner's request: in the T47 hand check a rating could be set on an image with a read-only sidecar without a message. ExifTool refuses ("0 image files updated", "1 files weren't updated due to errors"), but `write_tags` takes any answer containing "files updated" as success |
| 2026-11-09 | T48 | Ready for review | (Agent) `lib.rs`: `write_succeeded` reads ExifTool's summary lines as numbers (created/updated/unchanged > 0 and no "files weren't … due to errors"); `write_tags` uses it, error text unchanged. 3 unit tests (success answers; the read-only sidecar and JPEG answers; "0 image files updated" alone and empty). `tests/sidecar.rs`: 1 ExifTool test, read-only sidecar and read-only JPEG give `Error::ExifTool` and keep the old rating (permissions restored before the assertions). Test inventory updated. Workspace fmt, test (346 passed, 27 ignored, 0 failed) and clippy OK; no commit. Owner's hand check not done |
| 2026-11-09 | T48 | Ready for review | Owner's hand check: a rating on a RAW file with a read-only `.xmp` is now refused, but the message is in English (ExifTool's answer). T49 added |
| 2026-11-09 | T49 | Open | Added by the agent at the owner's request. Owner's decision: "filen är skrivskyddad" for a read-only file, "filen kunde inte skrivas" for other refused writes |
| 2026-11-09 | T49 | Ready for review | (Agent) `pw-metadata`: `Error::ReadOnly(PathBuf)` and `Error::NotWritten(String)`; `refused_write_error` picks `ReadOnly` only after ExifTool refused and the target is read-only; public `WriteError` (`ReadOnly`, `NotWritten`, `Other`) with `From<Error>`; `RatingWritten` carries `Result<(), WriteError>`. `pw-loupe` `metadata.rs`: `write_error_text` gives the two Swedish texts, `Other` keeps its text; `RatingStore` unchanged. Test helpers convert `String` ↔ `WriteError::Other` (assertions unchanged); T48's sidecar test now expects `Error::ReadOnly`. 6 tests added; test inventory updated. Workspace fmt, test (352 passed, 27 ignored, 0 failed) and clippy OK; no commit. Owner's hand check not done |
| 2026-11-09 | T49 | Ready for review | Owner's hand check OK: a rating on an image with a read-only sidecar gives the Swedish message "filen är skrivskyddad" and keeps the old rating. Waiting for review and commit |
| 2026-11-09 | T47 | Ready for review | Owner's hand check: the program starts; rate an image, close at once, reopen → the rating is kept (R4) OK. The message box at exit is not checked yet: a write finishes before the window can be closed |
| 2026-11-09 | T47 | Ready for review | Owner's hand check of the message box at exit OK, with ExifTool (`ExifTool.exe` and `perl.exe`) paused in Resource Monitor: (A) a write still pending at exit → after about 5 s "Betyg som inte sparades" lists the file; (B) a write to a read-only sidecar, ExifTool resumed during the wait → the box shows "Kunde inte spara betyget för TN005979.ARW: filen är skrivskyddad". All hand checks of T47 (and the exit part of T44) done; waiting for review and commit |
| 2026-11-09 | T47 | Done | Reviewed: `build.rs` embeds the Common Controls 6 manifest for the bins on windows + msvc only, no `unwrap`, `rerun-if-changed`; cause confirmed with `cargo tree` (`tauri-plugin-dialog` enables `common-controls-v6`). Owner's start and exit checks passed. Lesson: `check.ps1` cannot catch a binary that fails to load, because the test binaries never reference `on_exit` |
| 2026-11-09 | T44 | Done | Exit wait and message box checked by hand in T47 (pending write → list after about 5 s; read-only sidecar → Swedish reason); code reviewed earlier |
| 2026-11-09 | T48 | Done | Reviewed: `write_succeeded` reads the summary counts (created/updated/unchanged > 0 and no "files weren't … due to errors"); `write_iptc` gets the same, stricter check, which is correct. Unit tests for the three answers; real ExifTool test for a read-only sidecar and JPEG keeps the old rating and restores permissions before asserting |
| 2026-11-09 | T49 | Done | Reviewed: `Error::ReadOnly`/`NotWritten` chosen only after ExifTool refused; `WriteError` keeps the kind across the worker; `pw-loupe` maps it to "filen är skrivskyddad" / "filen kunde inte skrivas" without comparing text; `RatingStore` unchanged; test helpers convert as the task said. Owner's `check.ps1`: fmt, test (352 passed, 27 ignored, 0 failed) and clippy OK; owner's hand check passed |
| 2026-11-10 | T43 | Done | Owner's hand check passed (keys in grid and loupe, display, label name in the badge kept) |
| 2026-11-10 | T46 | Done | Owner's hand check passed (inset, rounded label bar and badge) |
| 2026-11-10 | T50 | Open | Added: ADR 0013 B6, documentation after milestone 4 (R6, the Lightroom/Photo Mechanic check, stays open) |
| 2026-11-10 | T50 | Ready for review | (Agent) Documentation only. ADR 0013: "Implemented" status line, §7 results for R1–R6 (R6 not checked yet), §9 Result column with tasks and a note that T47–T49 came from the hand checks, new §9.1 "Findings during implementation". ADR 0004: implementation note; level C check still open (= ADR 0013 R6); refused and read-only writes noted. architecture.md (`pw-metadata` `WriteError`, `pw-loupe` ratings and `build.rs`), GR8 status, root README status, docs/README ADR list. Test inventory unchanged (matches `test-output.txt`). Workspace fmt, test (352 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | – | Note | R6 / ADR 0004 clean-room check done with Photo Mechanic by the owner: stars and labels both ways; a reject (−1) is not shown in Photo Mechanic (its scale is 0–5); PM writes `.XMP` and its own `ColorClass`. Reviewer updated ADR 0013 (§7 R6, §9 B5, new §14) and ADR 0004 (Open). Lightroom not checked |
| 2026-11-10 | T50 | Done | Reviewed: only the named files changed; every result in ADR 0013 §7 and §9 is taken from the log (R6 updated afterwards by the reviewer with the Photo Mechanic check); §9.1 findings, ADR 0004 note, architecture.md, GR8, both READMEs correct. Milestone 4 (ADR 0013) is complete |
| 2026-11-10 | – | Note | ADR 0014 (milestone 5: filter and multi-selection) accepted by the owner (F0): scope, S1–S6 and every proposal of §11 as written |
| 2026-11-10 | T51–T58 | Open | Added: ADR 0014 F1–F6 and F8. T51–T54 can start now and are independent; T55 after T51 and T52; T56 after T53 and T55; T57 after T54 and T56; T58 after the owner's acceptance (F7). T54, T56, T57 and T58 are for a Claude agent |
| 2026-11-10 | T51 | Ready for review | (Agent) New `pw-loupe/src/filter.rs` (re-exported in `lib.rs`): `StarFilter`, `LabelFilter`, `Filter` (`Default`, `is_active`), `matches`, `visible` (keep-current rule), `nearest_visible`, `STAR_FILTERS`/`star_filter_text`, `LABEL_FILTERS`/`label_filter_text`; 17 tests. `tabs.rs`: `Tab::filter`, reset in `Tab::open`, `Tabs::set_filter`; 3 tests. Existing tests unchanged; `main.rs` and `Cargo.toml` untouched. Test inventory updated (`pw-loupe` 158). Workspace fmt, test (372 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T52 | Ready for review | (Agent) New `pw-loupe/src/selection.rs` (re-exported in `lib.rs`): `Selection` with `click`, `ctrl_click`, `extend_to`, `all`, `clear`, `retain_visible`, `count`, `is_marked`, `anchor`, `marked_in`, `targets`; 12 tests. `grid.rs`: `grid_key_action(key, ctrl, shift, alt)`, new `Extend`, `SelectAll`, `ClearSelection`; 1 new test; in the existing key test the calls get `shift = false` and its Escape line now expects `ClearSelection` (point 2). `tabs.rs`: `Tab::selection`, cleared in `Tab::open`, `Tabs::active_mut`; 1 test. `main.rs`: passes `shift`, new arms do nothing. **Outside the file list:** `ratings.rs`, two existing test calls of `grid_key_action` get `shift = false` (no other change). Test inventory updated (`pw-loupe` 172). Workspace fmt, test (386 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T51 | Open | Review: accepted except one point; `AtLeast` outside 1–5 must match the way its text reads (clamp in `matches`, with a test). See the review note in the task |
| 2026-11-10 | T52 | Open | Review: accepted except two points in `selection.rs`; `extend_to` must not mark an index that is not visible (S5), and `all` must replace the marking as `pickAll` does. The changed Escape line in the existing key test and the two `ratings.rs` test calls are accepted. Checked: `test-output.txt` newer than the changes, fmt/test (386 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 153 + 19 = 172 tests, matching the test inventory; no `unwrap`/`expect`/`panic!` in the new non-test code |
| 2026-11-10 | T51 | Ready for review | (Agent) Review point 1: `matches` clamps the `AtLeast` minimum to 1–5 first, so `AtLeast(0)` behaves as `AtLeast(1)` (a reject still never matches) and `AtLeast(6)`/`AtLeast(255)` as `AtLeast(5)`, as `star_filter_text` reads; new test `at_least_outside_one_to_five_matches_like_the_nearest_valid_value` over every value in `STAR_VALUES`. Only `filter.rs` changed in code; test inventory updated (`pw-loupe` 173). Workspace fmt, test (387 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T52 | Ready for review | (Agent) Review points 1–2, `selection.rs` only: `extend_to` returns without changing the marking or the anchor when `index` is not in `visible` (S5; an anchor that is not visible still marks `index` alone), doc comment updated; `all` clears the marking first, as `pickAll` does. New tests `extend_to_a_hidden_index_changes_nothing` and `all_replaces_the_marking` (mark 2 and 3, `all(&[0, 1])` → [0, 1]). Existing tests unchanged; test inventory updated (`pw-loupe` 175). Workspace fmt, test (389 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T51 | Done | Second review: `matches` clamps the `AtLeast` minimum to 1–5, so it does what `star_filter_text` says; the new test covers 0, 6 and 255 against every value in `STAR_VALUES`, and a reject never matches |
| 2026-11-10 | T52 | Done | Second review: `extend_to` returns before touching the marking or the anchor when `index` is not visible (S5); `all` clears first, as `pickAll` does; both have tests. Checked: `test-output.txt` newer than the changes, fmt/test (389 passed, 27 ignored, 0 failed)/clippy OK (the only `warning:` line is the existing `raw-decode-bench` build-script note); `pw-loupe` 175 tests, matching the test inventory. T51 and T52 can be committed together |
| 2026-11-10 | T53 | Ready for review | (Agent) `ratings.rs`: `RatingStore::set_many` (skips files without `supports_rating`, `Unsupported` when none is left, resolves every path before anything changes; one decision for the batch: a label or a reject is cleared only when every image has it, otherwise set on all; `Stars`/`ClearLabel` as `apply`; other half kept; returns the count). `set` and `set_many` share the private `current` and `put` helpers. Failure count and last file/error recorded in `fail_write`; `failure_text` (none / today's text / "Kunde inte spara betyget för {n} bilder. Senast: {fil}: {fel}"); reset by an accepted `set` or `set_many` (a refused key changes nothing, the count included). Strings from `pump`, `written`, `fail_sent` unchanged. 13 tests. `metadata.rs`: `set_ratings` (marked `#[cfg_attr(not(test), allow(dead_code))]` until T56 calls it) and `failure_text`; 1 test. `main.rs`: the status line uses `failure_text()` when `poll` returned a failure. Existing tests unchanged; test inventory updated (`pw-loupe` 189). Workspace fmt, test (403 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T54 | Ready for review | (Agent) `state.rs`: `State::set_files(files, current)`. Same list → only `go(current)`. Otherwise a new generation; old position → new position by path (each new position taken once); `sizes`, `failed`, owned sessions with `session_bytes_of` and `session_lru`, `resident` (`pos`, generation, same ids) and `ever_resident` (old generation only) are moved; state of removed paths is released (resident ids to `take_dropped`, `texture_bytes`/`session_bytes` subtracted); `inflight` and `outbox` cleared. Lent sessions are remembered with the generation they were lent in (`lent`); at `set_files` they leave `sessions` for `detached` (bytes still counted) and are dropped when they come back, so a stale `finish_render` never removes a session under a key of the new order (after `open_folder` it clears only the slot it was lent from, as before). `running` holds `(generation, Job)`, so an old-order job neither blocks nor is removed by a new-order job with the same key. `cur` = `current` clamped; zoom, centre, direction and mode kept; `since` kept when the current path is unchanged; `last_draw` kept unless its texture was dropped. `open_folder` unchanged apart from clearing `detached`. `core.rs`: `ViewCore::set_files` through `act`. 8 tests; existing tests unchanged; test inventory updated (`pw-viewcore` unit 52). Workspace fmt, test (411 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T53 | Done | Reviewed: `set_many` decides once per batch (label and reject cleared only when all have them), skips unsupported files, resolves every path before changing anything; `set` and `set_many` share `current`/`put`; the failure count is reset only by an accepted key; strings of `pump`/`written`/`fail_sent` unchanged, so the exit list of T44 is unaffected; `set_many` with one path equals `set` for every key and six starting values. The temporary `allow(dead_code)` on `MetadataCache::set_ratings` is removed in T56. Commit together with T54 |
| 2026-11-10 | T54 | Open | Review: `set_files` accepted; one point: the generation in `running` also lets the new folder start an open of a position whose old-folder open is still running (`open_folder` behaviour change, better but untested). Add a test and mention it. Checked: `test-output.txt` newer than the changes, fmt/test (411 passed, 27 ignored, 0 failed)/clippy OK |
| 2026-11-10 | T54 | Ready for review | (Agent) Review point: new test `a_new_folder_does_not_wait_for_an_open_of_the_old_folder` in `state.rs`. Behaviour change of `open_folder` (from the generation in `running`): an open of a position whose open from the old folder is still running is handed out at once instead of waiting; the old open coming back stores nothing (no session, size or bytes) and does not free the new one's place (it is not handed out twice); the new open then stores its session. Production code unchanged since the review. Test inventory updated (`pw-viewcore` unit 53). Workspace fmt, test (412 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T54 | Done | Second review: the new test pins the `open_folder` change (the new folder's open starts at once, the old one is discarded without storing or freeing anything, the new one is not handed out twice); production code unchanged since the first review. Checked: `test-output.txt` newer than the change, fmt/test (412 passed, 27 ignored, 0 failed)/clippy OK; `pw-viewcore` unit 53, matching the test inventory. T53 and T54 committed together. T55 can start (T51, T52 done); T56 after T55; T57 after T56 |
