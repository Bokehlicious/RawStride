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
