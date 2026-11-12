# Agent task list

Small, well-defined backend tasks for coding agents (local AI). Each task is reviewed by the
project owner together with a reviewer before it is merged. Read `AGENTS.md`, `CLAUDE.md` and
`docs/TESTING.md` first; they take precedence over this file.

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
13. **Texts the user sees** (so another language can be added later in one place). This rule applies
    to every task that adds or changes a text shown to the user, whichever rules the task lists:
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

### T55. ADR 0014 F5a: browse logic of a tab under a filter (plain Rust) — Done

- **Second review 2026-11-10: accepted.** The review point below is done.

- **Review 2026-11-10: changes requested (one point).** Accepted: `refilter`, `position`, `click`
  (a hidden index changes nothing), the moves doing nothing when nothing is visible, `Tab::open`
  showing every file, `to_request_indexes`/`forget_not_in`, `ratings_ready`/`read_error` (the
  temporary `allow(dead_code)` on `read_error` is removed in T56), the texts, and the tests. Still to
  do, in `crates/pw-loupe/src/browse.rs` only:
  1. **Refilter before the move too, as `go()` in `app.js` does** (you asked; ADR 0014 §4.2 says "as
     in `go()`"). `grid_move` and `loupe_move` must first `refilter` keeping the **current** image,
     then compute the move over that list, then `refilter` keeping the new current one, as now.
     Otherwise, after a batch rating, images other than the current one that stopped matching are
     still in `visible`, the move lands on one of them, and `keep` keeps it: under "Ej betygsatt",
     rating four marked images and pressing Right walks through the four rated images one by one.
     `click` stays as it is. Add a test: filter Unrated (visible [0, 4, 5, …, 11]), current 5, marked
     5–7, all three get stars; `grid_move(Right)` → current 8 and visible [0, 4, 8, 9, 10, 11]; the
     same case with `loupe_move(Next)`. Update the doc comments.
  Then run `.\tools\check.ps1`.

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

### T56. ADR 0014 F5b: draw the filter and the marking in the grid, wire clicks and keys — Done

- **Review 2026-11-10: accepted**, after the owner's hand check. The owner's decisions in the check:
  the marking count before the file name in the status line (ADR 0014 §5.3, amendment 1), and
  amendment 1 itself (§13), which becomes T59 and T60 and changes T57. The "Done when" check "under
  'Ej betygsatt' a rated image stays until moving on" no longer applies; T59 replaces it.

- **Note from the T53 review:** remove `#[cfg_attr(not(test), allow(dead_code))]` from
  `MetadataCache::set_ratings` when point 7 calls it.
- **Note from the T55 review:** the same for `MetadataCache::read_error` when the grid shows the
  read error (point 3).

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

### T59. ADR 0014 amendment 1, §2.2: a rating removes an image that stops matching at once — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when", plus a
  folder given as a relative path on the command line: ratings shown and filtered).

- **Start after T56 is done.** Before T57.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/browse.rs`, `crates/pw-loupe/src/main.rs`
  (the rating key and the shown-rating lookup of point 4).
- **Why:** ADR 0014 §2.2, §4.2 and S2 as changed by amendment 1 (§13). Changes the behaviour T55
  built: a rated image no longer stays until the user moves on.
- **What:**
  1. `Tab::rated(&mut self, ratings_ready: bool, rating_of: &dyn Fn(&Path) -> Option<Rating>) -> bool`:
     `refilter` without keeping any image. Images that no longer match leave `visible`; if the
     current image left, the nearest following visible image becomes current (the last visible one
     if none follows; unchanged when nothing is visible, §2.3); marked images that left are
     unmarked, the others stay marked. Returns whether `visible` changed.
  2. `grid_move`, `loupe_move` and `click` stay as they are (they still recompute before and after
     a move, which matters when a rating changes without a key, e.g. a rolled-back write). Update
     the doc comments in `browse.rs` (module, `refilter_before_move`, `grid_move`, `loupe_move`)
     and of `Tab.visible` in `tabs.rs` that describe the old §2.2 rule. No other change in `tabs.rs`.
  3. `main.rs`: after `set_ratings` returns `Ok`, call `rated` on the active tab with the same
     readiness as the other refilters. Grid: if `visible` changed, scroll to the current image's
     position. Loupe (until T57): if `visible` changed, give the core the visible list as
     `reopen_loupe` does; if nothing is visible, return to the grid (§2.3). A refused key
     (`Unsupported`, a path error) does not call `rated`. A rolled-back write does not refilter.
  4. **Resolved paths for the shown rating** (found in the T56 review): the filter's `rating_of`
     (`refilter_active`, `grid_move`, `click`, and `rated`) and the grid's `shown_rating` call pass
     `tab.files` as they are, but `MetadataCache` keys by absolute path. A folder given as a relative
     path on the command line then filters and draws every image as unrated. Look the rating up by
     the path resolved as `resolve_paths` does (for example a small helper on `Loupe`). Add a test
     for the helper if it is plain.
- **Rules:** as T55; rules 6, 7, 10–12.
- **Tests to change** (approved by amendment 1; nothing else in these tests changes):
  - `the_filter_select_cases_of_app_js`: the block from the comment "Under "unrated", rating the
    current image (2 stars) keeps it visible" up to the comment "Multi-selection with every image
    shown" becomes: rate image 4 with 2 stars, call `rated` → returns true, current 5, visible
    [0, 5, 6, 7, 8, 9, 10, 11], the file at current is `IMG_005.ARW`.
  - `a_loupe_step_drops_the_rated_image_it_leaves` and `batch_rated_under_unrated`: comments only
    (the ratings change without `rated`, as after a rolled-back write); assertions unchanged.
- **Tests to add:**
  - Unrated, current 5, marked 5–7, all three get stars, `rated` → visible [0, 4, 8, 9, 10, 11],
    current 8, nothing marked.
  - "Alla utom avvisade" (visible every index but 3), current 2, marked 1, 2 and 4; image 2 gets
    rejected, `rated` → 2 hidden, current 4, marked [1, 4].
  - The last visible image rated out → the previous visible image is current.
  - Red (visible [2, 4]), both marked, both lose Red, `rated` → nothing visible, returns true,
    current unchanged, nothing marked.
  - `rated` when nothing stopped matching → returns false; current and marking unchanged.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: under "Ej betygsatt"
  a rated image leaves at once in grid and loupe, and a rated marking leaves at once; under "Grön"
  another label removes the image at once.

### T57. ADR 0014 F6: the loupe steps and prefetches within the filter — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Start after T54, T56 and T59 are done.** **For a Claude agent.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/lib.rs` for the
  status line.
- **Why:** ADR 0014 §2.2 (amendment 1), §2.3, §4.4 (way C), §5.3 (amendment 1), S2, S3.
- **What:**
  1. Loupe steps (Next, Previous, First, Last) go through `Tab::loupe_move` instead of
     `core.step`/`go(0)`/`last`. If `visible` changed, call `core.set_files(paths of visible,
     position)`; otherwise `core.go(position)`.
  2. A filter change while the loupe is shown: `refilter`, then `set_files`; if nothing is visible,
     return to the grid (§2.3).
  3. A rating key in the loupe: after `Tab::rated` (T59), if `visible` changed, `core.set_files(paths
     of visible, position)`, so the next visible image comes from the prefetched views; if nothing
     is visible, return to the grid (§2.3).
  4. The tab's current image is taken from the tab, not from `frame.pos`, while the loupe is shown
     (the core follows the tab).
  5. Status line: `status_line` gets the folder's total and uses `position_text`
     ("3 / 12 (av 31)") first, as in the grid (§5.3, amendment 1); update its test.
  6. Remove the interim `open_folder` of T56 point 9 and T59 point 3 for changes of the visible
     list; opening the loupe from the grid and a tab load still use `open_folder`.
- **Rules:** as T56.
- **Tests to add:** the `app.js` case "loupe within filter": Red, Home, Enter, step right → IMG_002
  then IMG_004 (on `Tab::loupe_move`); the status-line text with and without a filter.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: under "≥ 3 stjärnor"
  stepping stays in the filter and holding an arrow is smooth; under "Ej betygsatt" rating the shown
  image shows the next unrated image at once, without a cold load; `latency.ps1` under the filter for
  V1 (S3); a batch of 30 then closing the window at once keeps the values (ADR 0013 R4).

### T60. ADR 0014 amendment 1, §2.1: the star condition ≥, =, ≤ — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Start after T57 is done** (all three change `main.rs`).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/filter.rs`, `crates/pw-loupe/src/lib.rs`
  (re-exports), `crates/pw-loupe/src/main.rs` (the filter lists only), `crates/pw-loupe/src/browse.rs`
  (tests only, see below).
- **Why:** ADR 0014 §2.1 and §5.2 as changed by amendment 1 (§13).
- **What:**
  1. `pub enum StarCondition { AtLeast, Exactly, AtMost }`, `AtLeast` the default;
     `pub const STAR_CONDITIONS: [StarCondition; 3]` in that order; `star_condition_text`:
     "≥", "=", "≤".
  2. `StarFilter::AtLeast(u8)` becomes `StarFilter::Stars(u8)` (a number of stars, clamped to 1–5
     as today). `Filter` gets `pub condition: StarCondition`.
  3. `matches` for `Stars(n)`: `AtLeast` → n stars or more; `Exactly` → exactly n; `AtMost` → no
     stars (unrated) or 0 to n stars. A reject (−1) never matches `Stars(n)`. The other star choices
     ignore `condition`.
  4. `Filter::is_active`: true when `stars` is not `All` or `label` is not `All`; the condition alone
     never makes a filter active.
  5. `STAR_FILTERS`: All, Stars(1)…Stars(5), Unrated, Rejected, NotRejected. `star_filter_text`:
     "Alla betyg", "1 stjärna", "2 stjärnor", "3 stjärnor", "4 stjärnor", "5 stjärnor",
     "Ej betygsatt", "Avvisade", "Alla utom avvisade" (outside 1–5: the nearest valid value, as today).
  6. `main.rs`: a narrow condition list just before the star list (order: condition, stars, label).
     Enabled only when the star choice is a number of stars; it keeps its value while disabled.
     Yellow outline on the condition and the star list when a number of stars is chosen. A choice
     works as the other two lists (`set_filter`, refilter without keeping, no widget keeps focus).
- **Rules:** as T51; rules 6, 7, 10–12.
- **Tests to change** (approved by amendment 1): every `StarFilter::AtLeast(n)` in the tests of
  `filter.rs` and `browse.rs` becomes `StarFilter::Stars(n)` with the default condition; assertions
  unchanged, except `star_filter_list_and_texts` and `star_filter_text_outside_one_to_five_uses_the_nearest_valid_value`,
  which get the new texts.
- **Tests to add:**
  - Each condition with n = 1–5 against unrated, 1–5 stars and rejected (≤ n includes unrated, no
    condition matches a reject).
  - `Exactly` and `AtMost` outside 1–5 behave like the nearest valid value (= 0 as = 1, ≤ 9 as ≤ 5).
  - The condition is ignored for All, Unrated, Rejected and NotRejected.
  - `is_active`: All/All with each condition is inactive; `Stars(n)` with each condition is active.
  - `STAR_CONDITIONS` order and texts.
  - With the `browse.rs` setup (1: 3 stars, 2: 5 stars Red, 3: rejected, 4: Red only): = 3 → [1];
    = 5 → [2]; ≤ 3 → [0, 1, 4, 5, …, 11]; ≤ 3 and Red → [4]; ≥ 3 → [1, 2] (unchanged).
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand (§8.2): "= 3", "≤ 2" and
  "≥ 4" on the test set; the condition disabled under "Ej betygsatt" and keeping its value; two tabs
  with different conditions.

### T61. ADR 0014 amendment 2: filter on several labels at once — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Can start now.** ADR 0014 §14 accepted by the owner (Q10–Q13 as proposed).
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/filter.rs`, `crates/pw-loupe/src/lib.rs`
  (re-exports), `crates/pw-loupe/src/main.rs` (the label list only), and the tests of `browse.rs`,
  `tabs.rs` and `lib.rs` that build a `Filter` (the label field only).
- **Why:** ADR 0014 §2.1 and §5.2 as changed by amendment 2 (§14).
- **What:**
  1. `pub enum LabelChoice { Label(&'static str), NoLabel }` and `pub const LABEL_CHOICES:
     [LabelChoice; 6]` (the five labels in `pw_metadata::LABELS` order, then `NoLabel`);
     `label_choice_text`: the Swedish names, "Ingen etikett".
  2. `LabelFilter` becomes a set of `LabelChoice` (for example a small bit set; `Copy`, `Default` =
     empty = all). Methods: `all()`, `is_all()`, `contains(choice)`, `toggle(choice)` (ticking the
     sixth choice clears the set, Q12), `only(choice)` for tests and callers.
  3. `matches`: with an empty set, every label; otherwise the image's label (or `NoLabel` when it has
     none) must be in the set. AND with stars as today. `Filter::is_active`: stars not `All` or the
     set not empty.
  4. `label_filter_text(&LabelFilter) -> String`: "Alla etiketter"; one name; two names joined with
     ", "; "{n} etiketter" from three.
  5. `main.rs`: the label drop-down shows "Alla etiketter" (clears the set) and one check box per
     `LABEL_CHOICES` item; the popup stays open while ticking (egui's close-on-click-outside
     behaviour); each tick goes through `set_filter` as today (refilter without keeping, no widget
     keeps the keyboard focus after the popup closes). Yellow outline when the set is not empty.
- **Rules:** as T51; rules 6, 7, 10–12.
- **Tests to change:** `LabelFilter::Label(l)` / `NoLabel` / `All` in existing tests become
  `LabelFilter::only(…)` / `LabelFilter::all()`; assertions unchanged, except the label text tests,
  which get the new texts.
- **Tests to add:** every single choice as before; Red + Yellow; Red + Ingen etikett; all five
  colours (unlabelled images hidden); ticking all six clears the set; toggling twice returns to the
  start; `is_active` for the empty and a non-empty set; the texts for 0, 1, 2 and 3 choices; with the
  `browse.rs` setup (2: 5 stars Red, 4: Red only): Red + Yellow → [2, 4]; Ingen etikett →
  [0, 1, 3, 5, …, 11]; ≥ 3 and Red + Ingen etikett → [1, 2].
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: two and three colours;
  a colour plus "Ingen etikett"; ticking all six; the outline; two tabs with different sets.

### T58. ADR 0014 F8: documentation after milestone 5 — Done

- **Review 2026-11-10: accepted.** Including amendment 2 in the ADR 0004 note (the task text named
  only amendment 1; adding it is right).

- **Start after T61 and the owner's acceptance (F7).** Documentation only; no code, no `Cargo.toml`. For a
  Claude agent.
- **Files:** `docs/adr/0014-egui-filter-and-multi-selection.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/architecture.md`, `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`,
  `docs/test-inventory.md` (only if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T51–T57, T59–T61 and the owner's F7 notes in this file; write only what
  they record.
- **What:**
  1. **ADR 0014:** "Implemented" status line; §7 results for S1–S6; §9 results per step; a short
     "Findings during implementation" section if the log has any.
  2. **ADR 0004** addendum "filter and multi-selection": a note that the egui program implements it
     (ADR 0014), with the differences: filter per tab, not saved between sessions, "Alla utom
     avvisade", and amendment 1 (ADR 0014 §13): the star condition ≥, =, ≤ (points 1) and a rated
     image that stops matching leaves at once (point 2).
  3. **architecture.md:** `pw-loupe` (filter, selection, batch ratings) and `pw-viewcore`
     (`set_files`).
  4. **GUI_REQUIREMENTS.md** GR8 and the root `README.md`: filter and multi-selection done; left
     before the switch: IPTC metadata, TIFF viewing.
  5. **docs/README.md:** "0014 milestone 5: filter and multi-selection in egui – done {date}".
- **Rules:** English, the style of the existing documents; change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

### T62. ADR 0009 amendment 1: a held step key in the loupe waits for the shown image — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Can start now.** Small; for a Claude agent or a local agent.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/lib.rs` (a plain function and its tests),
  `crates/pw-loupe/src/main.rs`.
- **Why:** ADR 0009 §13 (amendment 1, owner 2026-11-10), found in the F7 check of ADR 0014: in RAW
  mode a held arrow runs to the end of the list and only the last image is shown.
- **What:**
  1. `lib.rs`: `pub fn step_allowed(action: Action, repeat: bool, status: &Status) -> bool`: false
     only when `repeat` is true, `action` is `Action::Next` or `Action::Previous`, and `status` is
     `Status::Preparing { .. }`; true otherwise (`Ready`, `Error`, `Empty`, every other action, and
     every press that is not a repeat).
  2. `main.rs`: keep egui's `repeat` flag of each key event in the collected key actions (today
     `Event::Key { key, pressed: true, modifiers, .. }`). Remember whether the loupe's last frame was
     shown (the `Status` of the last `core.frame`, for example as a field updated where the frame is
     taken). In the loupe, call `act` for a step only when `step_allowed` says so; an ignored repeat
     does nothing else. The grid, the dialog keys, the rating keys and the tab keys are unchanged.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; rules 6, 7, 10–12. No change of
  `pw-viewcore`.
- **Tests to add:** `step_allowed` for Next and Previous with and without `repeat` against `Ready`,
  `Preparing`, `Error` and `Empty`; a repeat of `First`, `Last`, `ToggleZoom` and `ZoomStep` is always
  allowed.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: in RAW mode, holding an
  arrow shows every image passed (stepping follows the rendering) and stops at once when the key is
  released; in JPEG mode a held arrow moves as before; single quick presses move at once in both
  modes; a damaged file (error message) does not stop a held arrow; the grid's arrows are unchanged.

### T63. ADR 0015 D1: new crate `pw-tiff` with the TIFF decoder from the Tauri app — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's commit.

- **Can start now.** ADR 0015 accepted by the owner (D0, 2026-11-10), including `tiff` =0.9.1.
- **Crates:** new `pw-tiff`, `photoworkflow` (`src-tauri`).
- **Files:** new `crates/pw-tiff/Cargo.toml` and `crates/pw-tiff/src/lib.rs` (more module files in
  `crates/pw-tiff/src/` are fine); the root `Cargo.toml` (`[workspace.dependencies]` only);
  `src-tauri/Cargo.toml`; `src-tauri/src/tiff_view.rs`; `src-tauri/src/imaging.rs`; `Cargo.lock`
  (by cargo); `docs/test-inventory.md`. This task may change the three `Cargo.toml` files as written
  below and nothing else in them.
- **Why:** ADR 0015 §4.1, §4.4 and §6: RawStride and the Tauri app share one TIFF decoder; RawStride
  needs a memory limit and one decode at a time.
- **What:**
  1. **Crate:** `crates/pw-tiff/Cargo.toml` as the other internal crates (`version.workspace = true`
     and the other `*.workspace` keys, `[lints] workspace = true`), description "TIFF decoding to 8-bit
     RGB (ADR 0015)", one dependency `tiff = "=0.9.1"` with default features. Root `Cargo.toml`: add
     `pw-tiff = { path = "crates/pw-tiff" }` under "Internal crates" (the members glob
     `crates/*` already covers the folder).
  2. **Move from `src-tauri/src/imaging.rs`** to `pw-tiff`, unchanged in behaviour and messages:
     `Layout` (with `channels`), `Rgb8`, `to_rgb8`, `u16_to_u8`, `f32_to_u8`, and the test
     `conversions` with its assertions. In `to_rgb8`, replace the slice indexing `samples[..n * k]`
     with `samples.get(..n * k)` and the same "image data is short: …" error (rule 4); the result is
     the same. `imaging.rs` keeps `fit` and `downscale` and their tests (the RAW view uses them) and
     gets `pub use pw_tiff::Rgb8;` so `rawview.rs` and `main.rs` do not change.
  3. **Move from `src-tauri/src/tiff_view.rs`** to `pw-tiff`: `open`, `dimensions` and `decode_rgb8`.
     New signatures: `pub fn dimensions(path: &Path) -> Result<(u32, u32), TiffError>` and
     `pub fn decode_rgb8(path: &Path, limit: Option<usize>) -> Result<Rgb8, TiffError>`.
     - `limit = None` keeps today's `Limits::unlimited()` (the Tauri app). `Some(bytes)` starts from
       `Limits::default()` and sets `decoding_buffer_size` and `intermediate_buffer_size` to `bytes`
       (the core budget in RawStride, ADR 0015 Q4).
     - `pub enum TiffError { Io(String), Decode(String), Unsupported(String), TooLarge { limit: usize } }`
       with `Display` and `std::error::Error`. `Display` gives **exactly today's texts** for the first
       three ("{path}: {io error}", "TIFF: {error}", "TIFF colour type {other:?} is not supported
       yet", "TIFF sample format is not supported yet", "image data is short: …"). `tiff`'s
       `LimitsExceeded` becomes `TooLarge`, shown as "TIFF: the file needs more memory than the
       limit of {limit / 1 MiB} MB". The Swedish texts of ADR 0015 §5 come in D4, not here.
  4. **One decode at a time (ADR 0015 §4.4):** a private `static` `Mutex<()>` in `pw-tiff`, taken by
     `decode_rgb8` for the whole decode (a poisoned lock is taken with `into_inner`, as `tiff_view`'s
     `lock` does). `dimensions` does not take it.
  5. **Tauri app:** `src-tauri/Cargo.toml` replaces `tiff = "0.9"` with `pw-tiff.workspace = true`.
     `tiff_view.rs` keeps `TiffView`, the cache, `max_edge`, `encode_jpeg` and its own decode lock,
     and keeps the functions `dimensions(path) -> Result<_, String>` and `decode_rgb8(path) ->
     Result<Rgb8, String>` as one-line wrappers over `pw_tiff` (`limit = None`, `.map_err(|e|
     e.to_string())`), so `main.rs` does not change. Update the module comments of both files to say
     where the decoder now lives.
- **Rules:** a move with additions; no change in what the Tauri app shows or reports; rules 3–7 and
  10–12. Check with `git diff Cargo.lock`: the only changes are the new `pw-tiff` package and the
  dependency lists of `photoworkflow` and `pw-tiff`; **no new external package** (`tiff` 0.9.1 is
  already there). If cargo wants anything else, stop and report.
- **Tests to add** (in `pw-tiff`; build the files in the test with `tiff::encoder::TiffEncoder`, or
  by hand as `crates/pw-metadata/tests/sidecar.rs` does):
  - 8-bit RGB, 16-bit RGB, 8-bit Gray, 8-bit RGBA, 8-bit CMYK and 32-bit float RGB, each 2×2 or
    larger, decode to the expected RGB bytes; one LZW-compressed RGB file decodes to the same bytes
    as its uncompressed twin;
  - `dimensions` gives the encoded size;
  - a palette TIFF gives `Unsupported` (build it by hand: PhotometricInterpretation 3 with a
    ColorMap);
  - a truncated file (a valid file cut in half) gives an error, not a panic;
  - `Some(limit)` smaller than the image gives `TooLarge`; `None` and a large enough limit decode;
  - the lock: while a test holds the decode lock (a `#[cfg(test)]` accessor is fine), a
    `decode_rgb8` on another thread does not finish (`recv_timeout` of 200 ms gives a timeout); after
    the lock is released it finishes;
  - a file that does not exist gives `Io`.
- **Done when:** `.\tools\check.ps1` is all OK; `photoworkflow` has the same tests as before minus
  `conversions`, which `pw-tiff` now has, all passing; `docs/test-inventory.md` has a `pw-tiff` row
  and the new totals; `git diff --stat` shows only the files above.

### T64. ADR 0015 D2: the loupe core opens TIFF files — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's commit.

- **Can start now.** Independent of T65 (they share only `docs/test-inventory.md` and this file).
- **Crates:** `pw-viewcore`, `pw-loupe` (one line).
- **Files:** `crates/pw-viewcore/Cargo.toml`, `crates/pw-viewcore/src/native.rs`,
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/lib.rs` (re-export),
  `crates/pw-viewcore/tests/native_files.rs`, `crates/pw-loupe/src/main.rs` (the `NativeBackend`
  construction in `run` only), `Cargo.lock` (by cargo), `docs/test-inventory.md`.
- **Why:** ADR 0015 §4.1–§4.4 and §5: a TIFF is shown as itself in both display modes, decoded once
  with `pw-tiff` under the core budget.
- **What:**
  1. **Cargo:** `pw-tiff = { workspace = true, optional = true }` under `[dependencies]` and
     `"dep:pw-tiff"` added to the `native` feature (update its comment); `tiff = "=0.9.1"` under
     `[dev-dependencies]` (to write test files; already in `Cargo.lock`).
  2. **`state.rs`:** a new `pub fn is_shown_as_itself(path: &Path) -> bool`: true for the extensions
     jpg, jpeg, tif, tiff (any case). `source_in` uses it instead of `is_plain_jpeg`, so a TIFF is
     `Source::Jpeg` in both modes (ADR 0015 Q3). `is_plain_jpeg` stays unchanged (pw-loupe uses it for
     the "JPEG-fil" text until T66). Re-export the new function from `lib.rs` next to it.
  3. **`native.rs`, the limit:** `NativeBackend` gets a private field `tiff_limit: usize` (keeps
     `Clone, Copy, Debug`); `impl Default` = `4 << 30` (the default core budget);
     `pub fn with_tiff_limit(bytes: usize) -> NativeBackend`. `pw-loupe` `run`:
     `NativeBackend::with_tiff_limit(usize::try_from(config.core_budget).unwrap_or(usize::MAX))`
     instead of `NativeBackend` (ADR 0015 Q4). `native_files.rs`: `NativeBackend` →
     `NativeBackend::default()`, nothing else in the existing tests.
  4. **`native.rs`, the session:** `NativeSession::Tiff(Rgb)` holding the full decoded image.
     `open` checks the file type **first**, for either `Source`: a TIFF (`pw_raw::is_tiff_file`) goes
     to a new `open_tiff(path, limit)`:
     - `pw_tiff::decode_rgb8(path, Some(limit))`, converted to `Rgb` (same three fields);
     - the orientation from `pw_raw::inspect(path)`; if inspect fails (BigTIFF), orientation 1
       (ADR 0015 §4.3); `ImageSize { w, h, flip: flip_of_orientation(orientation) }` with the
       decoded size.
     - `render_half`: `scale::downscale` of the full image to `scale::scaled_size(w, h, 0.5)`;
       `render_region`: `scale::crop` as for the JPEG's full decode, with the same error texts
       ("the TIFF was not decoded" is not needed: the image is always there).
     - `session_bytes` needs no change: a TIFF is keyed `Source::Jpeg`, which already counts
       `w × h × 3`, the decoded RGB.
  5. **Error texts (Swedish, ADR 0015 §5; the loupe shows them after "Filen kan inte visas: "):**
     `TiffError::Unsupported(m)` → "TIFF-varianten stöds inte än ({m})";
     `TiffError::TooLarge { limit }` → "TIFF-filen är för stor för minnesbudgeten ({limit / 1 MiB}
     MB)"; `Io` and `Decode` → their own text. Never `DecodeError::damaged` (that is for RAW data,
     G3). Put the mapping in a small function `tiff_error(e: pw_tiff::TiffError) -> DecodeError` so
     it can be tested.
- **Rules:** no change of `Source`, of the cache keys or of RAW/JPEG behaviour; rules 3–7 and 10–12.
  `git diff Cargo.lock`: only the new dependency lines of `pw-viewcore`; no new package.
- **Tests to add:**
  - `state.rs`: `is_shown_as_itself` for `a.tif`, `b.TIFF`, `c.jpg`, not for `d.arw`, `e.dng`, `f`;
    a TIFF is keyed `Source::Jpeg` in RAW mode and in JPEG mode, and has no "other mode" entry
    (follow `a_plain_jpeg_file_is_shown_as_itself_in_both_modes`).
  - `native.rs` (write the files with `tiff::encoder::TiffEncoder` into a test folder of its own):
    a 6×4 8-bit RGB TIFF opens with `w = 6, h = 4`, `flip = 0`, and `render_half` gives 3×2 and
    `render_region` gives exactly the requested pixels; opening it with `Source::Raw` gives the same
    TIFF session (the file type wins); a TIFF with Orientation 6 (write the tag with the encoder's
    directory API, or by hand) gives `flip = 6`; a palette TIFF (copy `palette_tiff` from
    `crates/pw-tiff/src/tests.rs`) gives the "TIFF-varianten stöds inte än" text and `damaged =
    false`; a limit smaller than the image gives the "för stor för minnesbudgeten" text;
    `tiff_error` for all four kinds.
  - `native_files.rs`, ignored like the others (needs `PW_TESTDATA`): every `.tif` / `.tiff` in
    `PW_TESTDATA` (a new helper; at least one must exist) is shown in fit view with
    `Source::Jpeg` in **both** modes, at the size of `pw_tiff::dimensions`, without an error.
- **Done when:** `.\tools\check.ps1` is all OK; the owner runs `.\tools\check.ps1 -Ignored` with
  `PW_TESTDATA` set and the new real-file test passes on his 9 TIFFs; `git diff --stat` shows only
  the files above.

### T65. ADR 0015 D3: thumbnails of TIFF files — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's commit.

- **Can start now.** Independent of T64 (they share only `docs/test-inventory.md` and this file).
- **Crates:** `pw-tiff`, `pw-thumbcache`, `photoworkflow` (`src-tauri`).
- **Files:** `crates/pw-tiff/src/` (a new module for the moved helpers, and `lib.rs`),
  `src-tauri/src/imaging.rs`, `crates/pw-thumbcache/Cargo.toml`, `crates/pw-thumbcache/src/extract.rs`,
  `Cargo.lock` (by cargo), `docs/test-inventory.md`.
- **Why:** ADR 0015 §4.1 and Q7: a TIFF thumbnail is decoded from the image data and reduced to the
  cell with a box filter. The box filter already exists in the Tauri app.
- **What:**
  1. **Move `fit` and `downscale`** with their tests (`fit_keeps_aspect_and_never_upscales`,
     `downscale_averages_blocks`, assertions unchanged) from `src-tauri/src/imaging.rs` to `pw-tiff`
     (for example `crates/pw-tiff/src/resize.rs`), exported from `pw-tiff`. Behaviour unchanged.
     In `downscale`, replace slice indexing (`src.data[..]`, `out[..]`, `col[x]`, `acc[..]`,
     `cnt[t]`) with forms that cannot panic in non-test code (`get`/`get_mut`, iterators,
     `chunks_exact`), keeping the result bit for bit; the existing test and one more on a 7×5 image
     reduced to 3×2 (compare with the result of today's code, computed in the test) pin it.
     `imaging.rs` becomes `pub use pw_tiff::{Rgb8, downscale, fit};` plus its module comment, so
     `rawview.rs`, `tiff_view.rs` and `main.rs` do not change.
  2. **`pw-thumbcache`:** `pw-tiff.workspace = true` under `[dependencies]` and `tiff = "=0.9.1"`
     under `[dev-dependencies]`. In `thumbnail`, before `pw_raw::inspect`: for a TIFF
     (`pw_raw::is_tiff_file`), a new private `tiff_thumbnail(path, cell)`:
     - `pw_tiff::decode_rgb8(path, Some(TIFF_LIMIT))`, with `const TIFF_LIMIT: usize = 4 << 30`
       (the default core budget; the thumbnail worker does not get the user's budget, ADR 0015 §9.1);
     - reduce with `pw_tiff::fit(w, h, cell)` and `pw_tiff::downscale` (never upscales);
     - convert to `pw_libjpeg::Rgb8` (same three fields) and apply `orient` with the orientation of
       `pw_raw::inspect`, or 1 if inspect fails (BigTIFF);
     - errors become `ThumbError::Unreadable(e.to_string())`.
     The embedded previews of a TIFF are not used (ADR 0015 Q7).
- **Rules:** RAW and JPEG thumbnails unchanged; the Tauri app's output unchanged; rules 3–7 and
  10–12. `git diff Cargo.lock`: only the new dependency lines of `pw-thumbcache`; no new package.
- **Tests to add** (`extract.rs`; write the files with `tiff::encoder::TiffEncoder` into a test
  folder of its own):
  - a 400×200 8-bit RGB TIFF with `cell = 100` gives 100×50; with `cell = 1000` it stays 400×200
    (no upscale); a uniform colour stays that colour;
  - a TIFF with Orientation 6 gives a thumbnail turned to 50×100;
  - a palette TIFF gives `Unreadable` with the `pw-tiff` text;
  - a TIFF with an embedded JPEG preview is still decoded from its image data (optional if the
    encoder cannot write one; then say so in the report);
  - ignored like the existing real-file test (needs `PW_TESTDATA`): every `.tif` in `PW_TESTDATA`
    gives a thumbnail whose long edge is 256 for `cell = 256`.
- **Done when:** `.\tools\check.ps1` is all OK; `photoworkflow` has two tests fewer and `pw-tiff`
  three more; the owner's `check.ps1 -Ignored` run passes the new real-file test; `git diff --stat`
  shows only the files above.

### T66. ADR 0015 D4: RawStride lists TIFF files and names them in the status line — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when").

- **Can start now.** T64 and T65 are done: the core opens TIFF files and the thumbnail worker
  makes their thumbnails, so listing them is the last step before the owner's acceptance (D5).
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/lib.rs` (functions and tests);
  `docs/test-inventory.md`. No change of `main.rs` is expected; if one is needed, stop and explain.
- **Why:** ADR 0015 §3.1 and §5: TIFF files are listed in the grid and the loupe, and the status line
  names the real source (GR9). The owner accepted replacing the test that pins the old rule (ADR
  0015 D0, §8.1).
- **What:**
  1. **Listing:** `is_listed` lists every extension of `pw_raw::SUPPORTED_EXTENSIONS`, TIFF
     included (remove `&& !pw_raw::is_tiff_file(path)`). Update its doc comment: "the file types of
     the current app (TIFF since ADR 0015)".
  2. **Status texts for a TIFF** (`frame.path` is a TIFF by `pw_raw::is_tiff_file`; the core keys it
     `Source::Jpeg`, ADR 0015 §4.2). Add a private helper, for example `fn is_tiff(frame: &Frame) ->
     bool`, and use it in:
     - `source_text`: "TIFF-fil{size}" (before the JPEG cases), for example "TIFF-fil 7741 × 5161";
     - `status_line` while preparing without a size: "förbereder TIFF" instead of "förbereder JPEG";
     - `indicator_text`: "Förbereder TIFF …" instead of "Förbereder JPEG …".
     RAW files, embedded JPEGs and JPEG files keep their texts exactly.
  3. **Usage text** (`USAGE`): `--core-mb` "memory for RAW sessions and decoded JPEGs and TIFFs
     (default 4096)". Nothing else in `USAGE`.
- **Rules:** rules 4–7 and 10–12. No change of `error_text`: the TIFF error texts come from
  `pw-viewcore` (T64) after "Filen kan inte visas: ". No `Cargo.toml` change.
- **Test to replace (owner's approval, ADR 0015 D0):** `the_folder_lists_supported_files_by_name_without_tiff`
  becomes `the_folder_lists_supported_files_by_name`, with the same files and steps; only the
  expected list changes to `["a.jpg", "b.ARW", "C.dng", "d.tif", "f.rw2"]` (`e.xmp`, `notes.txt` and
  the folder `sub.arw` stay out; `C.dng` stays at position 2). Every other test keeps its assertions.
- **Tests to add:** `is_listed` for `a.tif`, `b.TIFF`, `c.tiff`; `source_text` of a ready TIFF frame
  (path `D:/Scans/Scan 01.tif`, `Source::Jpeg`, a size with `flip = 6`) is "TIFF-fil" with the oriented
  size, in both `mode` values; `status_line` of a preparing TIFF frame without a size contains
  "förbereder TIFF"; `indicator_text` of a TIFF frame is "Förbereder TIFF …"; a `.JPG` frame still
  gives "JPEG-fil" and "Förbereder JPEG …".
- **Done when:** `.\tools\check.ps1` is all OK; `git diff --stat` shows only the files above; and
  the owner checks by hand that `D:\PhotoWorkflowData\RAW` shows its TIFFs in the grid (thumbnails)
  and the loupe, with "TIFF-fil W × H" in the status line in both modes. The full acceptance with
  ratings, zoom, errors and timing is step D5 (ADR 0015 §8.2).

### T67. ADR 0015 D6–D7: documentation after milestone 6 — Ready for review

- **Review 2026-11-11: accepted.** Waiting for the owner's commit.

- **Can start now.** T63–T66 and the owner's acceptance (D5, 2026-11-11) are done. Documentation
  only; no code, no `Cargo.toml`. For a Claude agent.
- **Files:** `docs/adr/0015-egui-tiff-viewing.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/adr/0009-egui-loupe-migration.md`, `docs/adr/0007-metadata-iptc.md`, `docs/architecture.md`,
  `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`, `docs/test-inventory.md` (only
  if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T63–T67 and D5 in this file, and ADR 0015 §2, §7, §9 and §9.1
  as they are now; write only what they record. Do not change ADR 0015 §7 or §9.1, which the reviewer
  wrote from the log.
- **What:**
  1. **ADR 0015:** after the status line, an "Implemented (2026-11-11)" line as in ADR 0014: steps
     D1–D4 done (agent tasks T63–T66), D5 accepted by the owner, D6–D7 this documentation update.
     §9: D6 and D7 "**Done:** T67". §12 (Consequences): keep the text; add one point that colour
     management (ICC) is not applied and is an open owner question before the switch (§9.1).
  2. **ADR 0004** addendum "viewing TIFF": a note "2026-11-11: implemented in the egui program (ADR
     0015)" with the differences: the decoder is the shared crate `pw-tiff`; RawStride keeps the
     decoded image instead of JPEGs at three sizes; one decode at a time in the whole process; a
     memory limit (the core budget in the loupe, 4 GiB for thumbnails); a TIFF is shown as itself in
     both display modes; thumbnails are decoded from the image data, embedded previews not used.
  3. **ADR 0009:** §3.1 table row "Open a folder": replace "except TIFF (later)" with "TIFF since
     ADR 0015"; §3.2: the item "TIFF viewing." gets "(done: ADR 0015, 2026-11-11)". Nothing else.
  4. **ADR 0007:** one new line in the status list, after the first: "Planned as milestone 7 of the
     egui program after its approval (owner, 2026-11-10; ADR 0015 §2)." Nothing else.
  5. **architecture.md**, crate table only (the diagram stays):
     - a new row `pw-tiff` after `pw-thumbcache`: TIFF decoding to 8-bit RGB (`tiff` 0.9.1), one
       decode at a time in the process, an optional memory limit, and the box filter `fit` /
       `downscale`; shared by RawStride and the Tauri app (ADR 0015);
     - `pw-thumbcache`: "TIFF files are decoded from their image data with `pw-tiff` (ADR 0015)";
     - `pw-viewcore`: "a TIFF is opened whole with `pw-tiff` and shown as itself in both modes (ADR
       0015)";
     - `pw-loupe`: "and TIFF files (ADR 0015)" where it lists what the program has.
  6. **GUI_REQUIREMENTS.md** GR8: the status becomes "Status 2026-11-11: ratings and labels (ADR
     0013), filter and multi-selection (ADR 0014) and TIFF viewing (ADR 0015) are done in the egui
     program. IPTC metadata: the Tauri app's only IPTC feature, the import template, is in the egui
     program (ADR 0010, T39); full IPTC handling (ADR 0007) is milestone 7 and not a parity item. No
     feature of the Tauri app is left; the switch waits for the acceptance tests. Open owner question
     before the switch: colour management (ADR 0015 §9.1)." Keep the rest of GR8 as it is.
  7. **Root `README.md`** status: the program also has TIFF viewing; replace "until the egui program
     covers the rest of its features (IPTC metadata, TIFF viewing; GUI_REQUIREMENTS GR8)" with "until
     the switch (GUI_REQUIREMENTS GR8)".
  8. **docs/README.md:** "0015 milestone 6: TIFF viewing in egui – done 2026-11-11" (instead of
     "accepted 2026-11-10"); 0007 stays "proposed".
- **Rules:** English, the style of the existing documents (short sentences, no marketing words);
  change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

### T68. ADR 0016 C1: new crate `pw-colour` for colour conversion — Done

- **Can start now.** ADR 0016 accepted by the owner (C0, 2026-11-11), including `moxcms` =0.8.1.
- **Crates:** new `pw-colour` only. No other crate uses it yet (that is C2–C4).
- **Files:** new `crates/pw-colour/Cargo.toml` and `crates/pw-colour/src/lib.rs` (more module files
  in `crates/pw-colour/src/` are fine); the root `Cargo.toml` (`[workspace.dependencies]` only);
  `Cargo.lock` (by cargo); `docs/test-inventory.md`. This task may change the two `Cargo.toml` files
  as written below and nothing else in them.
- **Why:** ADR 0016 §4.1, §6 and Q3, Q5, Q6: one place that turns pixels from a source profile into
  the monitor profile, testable without a screen, for `pw-loupe`, `pw-viewcore`, `pw-thumbcache` and
  `pw-tiff` later.
- **What:**
  1. **Crate:** `crates/pw-colour/Cargo.toml` as `crates/pw-tiff/Cargo.toml` (the `*.workspace` keys,
     `[lints] workspace = true`), description "Colour conversion between ICC profiles (ADR 0016)",
     one dependency `moxcms = "=0.8.1"` with **default features** (the same features `image` already
     builds, so nothing new is compiled). Root `Cargo.toml`: add
     `pw-colour = { path = "crates/pw-colour" }` under "Internal crates", next to `pw-tiff`.
  2. **`Profile`** (`Clone`, `Debug`): a parsed RGB profile plus its identity.
     - `Profile::srgb()`, `Profile::adobe_rgb()`, `Profile::pro_photo_rgb()` from moxcms's
       `ColorProfile::new_srgb()`, `new_adobe_rgb()`, `new_pro_photo_rgb()`.
     - `Profile::from_icc(bytes: &[u8]) -> Result<Profile, ColourError>` with
       `ColorProfile::new_from_slice`. A profile whose data colour space is not RGB (gray, CMYK, Lab)
       gives `Unsupported`; bytes moxcms cannot parse give `Profile`. Never a panic, whatever the
       bytes (empty, garbage, a valid profile cut short).
     - The identity is a private `enum ProfileId { Srgb, AdobeRgb, ProPhotoRgb, Icc(Arc<[u8]>) }`
       (`Clone`, `PartialEq`, `Eq`, `Hash`); an ICC profile is identified by its bytes.
  3. **`Transform`** (must be `Send + Sync`): `Transform::new(source: &Profile, destination: &Profile)
     -> Result<Transform, ColourError>` builds an 8-bit and a 16-bit RGB → RGB transform with
     `create_transform_8bit` / `create_transform_16bit`, `Layout::Rgb` on both sides and
     `TransformOptions { rendering_intent: RenderingIntent::RelativeColorimetric,
     ..Default::default() }` (ADR 0016 Q6; moxcms has no black-point compensation).
     - **Identity:** when source and destination have the same `ProfileId`, no moxcms transform is
       built, `is_identity()` is true and the conversions return without touching the data (F3:
       bit-identical to today).
     - `convert_rgb8(&self, data: &mut [u8]) -> Result<(), ColourError>` and
       `convert_rgb16(&self, data: &mut [u16]) -> Result<(), ColourError>`, both **in place**, working
       through the data in chunks of at most 65 536 pixels with one scratch buffer, so a 40 MP image
       does not need a second full copy. A length that is not a multiple of 3 gives `Length` and
       leaves the data unchanged.
  4. **`TransformCache`** (`Default`, `Send + Sync`): `get(&self, source: &Profile, destination:
     &Profile) -> Result<Arc<Transform>, ColourError>`, keyed by the two `ProfileId`s in a
     `Mutex<HashMap<…>>`. The same pair gives the same `Arc` (built once, ADR 0016 §4.1). A failed
     build is returned and not cached. A poisoned lock is taken with `into_inner`, as `pw-tiff` does.
  5. **`ColourError`**: `Profile(String)`, `Unsupported(String)`, `Transform(String)`,
     `Length { len: usize }`, with `Display` and `std::error::Error`. English texts, as `pw-tiff`:
     "ICC profile: {detail}", "ICC profile is not RGB ({colour space:?})", "colour transform:
     {detail}", "pixel data of {len} values is not whole RGB pixels". Swedish texts come in the
     steps that show them.
  6. Module comment: what the crate does, with a reference to ADR 0016, and that it is not used yet.
- **Rules:** rules 3–7 and 10–12. Check with `git diff Cargo.lock`: the only changes are the new
  `pw-colour` package and its dependency list; **no new external package** (`moxcms` 0.8.1,
  `num-traits` and `pxfm` are already there). If cargo wants anything else, stop and report.
- **Tests to add** (in `pw-colour`; no file from the owner's computer, ADR 0016 §8.1). The reference
  values below were computed independently of moxcms (floating point, Bradford adaptation, sRGB
  piecewise curve, Adobe RGB gamma 563/256, ProPhoto gamma 1.8). Each channel must be **within 1**
  of them (F1). If moxcms is further off, stop and report: that is the signal for the `lcms2`
  fallback of ADR 0016 Q5, not a reason to widen the tolerance.
  - **sRGB → Adobe RGB, 8 bit:** (255,0,0) → (219,0,0); (0,255,0) → (144,255,60); (0,0,255) →
    (0,0,250); (255,255,255) → (255,255,255); (0,0,0) → (0,0,0); (128,128,128) → (127,127,127);
    (200,100,50) → (177,100,56); (30,160,220) → (93,159,217).
  - **ProPhoto RGB → sRGB, 8 bit:** (255,255,255) → (255,255,255); (0,0,0) → (0,0,0);
    (128,128,128) → (146,146,146); (150,110,90) → (200,118,105); (100,140,120) → (64,165,136);
    (180,160,140) → (213,171,154); (120,120,160) → (125,139,181).
  - **16 bit:** the ProPhoto → sRGB colours above as 16-bit values (8-bit × 257), converted with
    `convert_rgb16` and reduced as `pw_tiff::u16_to_u8` does (`v >> 8`, written out in the test; no
    dependency on `pw-tiff`), are within 1 of the same references (ADR 0016 §8.1: the 16-bit path
    equals the 8-bit path within one step).
  - **Identity (F3):** sRGB → sRGB is `is_identity()`, and a buffer of all 256 grey values plus the
    colours above is unchanged byte for byte, in 8 and 16 bit. The same for two `Profile`s from the
    same ICC bytes.
  - **ICC bytes:** `Profile::from_icc` of `ColorProfile::new_adobe_rgb().encode()` converts the
    sRGB → Adobe RGB colours above within 1 of the references (the path real files take).
  - **Bad profiles (F4):** empty bytes, 100 garbage bytes, and the first half of the encoded Adobe
    RGB profile give an error, never a panic; `ColorProfile::new_gray_with_gamma(2.2).encode()` gives
    `Unsupported`.
  - **Length:** 4 bytes give `Length { len: 4 }` and leave the data unchanged; an empty buffer is
    fine.
  - **Chunks:** an image of 65 536 × 2 + 7 pixels (a repeating pattern of the colours above) gives the
    same result as converting each pixel on its own.
  - **Cache:** two `get` calls with the same pair return `Arc::ptr_eq` transforms; a different pair
    gives a different one; a failing pair (gray ICC) returns the error.
  - **Threads:** a compile-time check that `Transform` and `TransformCache` are `Send + Sync`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has a `pw-colour` row and the
  new totals; `git diff --stat` shows only the files above.

### T69. ADR 0016 C2: RawStride reads the monitor profile at start and names it in the window title — Done

- **Can start now.** T68 (`pw-colour`) accepted. The passthrough check of ADR 0016 §8.2 is done
  (§1.1). T69, T70 and T71 do not depend on each other.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/Cargo.toml`; `crates/pw-loupe/src/lib.rs`; a new module
  `crates/pw-loupe/src/monitor_profile.rs`; `crates/pw-loupe/src/main.rs`; `Cargo.lock` (by cargo);
  `docs/test-inventory.md`. This task may change `crates/pw-loupe/Cargo.toml` as written below and
  nothing else in it.
- **Why:** ADR 0016 §4.1 ("Reading the monitor profile"), §5 (window title), Q4 (read at start
  only), §6 (`windows-sys` features approved in C0).
- **What:**
  1. **Dependencies** in `crates/pw-loupe/Cargo.toml`: `pw-colour.workspace = true` under
     `[dependencies]`, and a new section
     `[target.'cfg(windows)'.dependencies]` with
     `windows-sys = { version = "=0.61.2", features = ["Win32_Foundation", "Win32_Graphics_Gdi", "Win32_UI_ColorSystem"] }`.
  2. **`monitor_profile.rs`** (library module, exported from `lib.rs`):
     - `pub enum ProfileSource { File(String), Missing, Unusable(String) }` (`Debug`, `Clone`,
       `PartialEq`, `Eq`): `File` holds the profile's **file name** (not the path); `Unusable` holds
       the detail (English, for a later log; not shown).
     - `pub struct MonitorProfile { pub profile: pw_colour::Profile, pub source: ProfileSource }`
       (`Debug`, `Clone`).
     - `pub fn load_monitor_profile(path: Result<Option<PathBuf>, String>) -> MonitorProfile`: the
       testable part. `Ok(Some(path))`: read the file and `Profile::from_icc`; success is `File(name)`
       with that profile. `Ok(None)` (no profile associated) is `Missing`. A read error, a parse
       error, an `Unsupported` profile or `Err(detail)` is `Unusable(detail)`. Every case except
       `File` uses `Profile::srgb()`, so the pixels stay as today (F3). Never a panic.
     - `pub fn primary_monitor_profile_path() -> Result<Option<PathBuf>, String>`: on Windows, the
       sequence the reviewer checked on the owner's machine (ADR 0016 §4.1): `MonitorFromPoint`
       (point 0,0, `MONITOR_DEFAULTTOPRIMARY`), `GetMonitorInfoW` with `MONITORINFOEXW`,
       `CreateDCW(device, device, null, null)` with its `szDevice`, `GetICMProfileW` twice (first
       for the length, then into a buffer of that length), `DeleteDC` on every path after a
       successful `CreateDCW`. A failing call gives `Err` with the call's name; `GetICMProfileW`
       reporting no profile gives `Ok(None)`. Each `unsafe` block gets a `// SAFETY:` comment, as in
       `pw-libjpeg`. On other platforms it returns `Ok(None)`.
     - `pub fn window_title(source: &ProfileSource) -> String`, exactly (ADR 0016 §5, two spaces
       around the dot as in the status line):
       - `File(name)`: "RawStride  ·  Skärmprofil: {name}"
       - `Missing`: "RawStride  ·  Ingen skärmprofil – visas som sRGB"
       - `Unusable(_)`: "RawStride  ·  Skärmprofilen kunde inte användas – visas som sRGB"
  3. **`main.rs`, `run`:** call `load_monitor_profile(primary_monitor_profile_path())` **before**
     `ViewCore::start`, give the viewport `.with_title(window_title(&monitor.source))` instead of
     `WINDOW_TITLE` (the first argument of `run_native`, the app name, stays `WINDOW_TITLE`), and keep
     the `MonitorProfile` in a new `Loupe` field `monitor_profile` for C4. It is not used for pixels
     in this task. If the compiler warns that the field is never read, mark that one field
     `#[allow(dead_code)]` with the comment "used by ADR 0016 C4".
- **Rules:** rules 3–7 and 10–12. Check with `git diff Cargo.lock`: the only change is `pw-colour`
  and `windows-sys` in `pw-loupe`'s dependency list; **no new package**. If cargo wants anything
  else, stop and report.
- **Tests to add** (in `pw-loupe`'s library; files in `std::env::temp_dir()` with the process id in
  the name, removed afterwards, as the existing tests do; an ICC file is made with
  `moxcms::ColorProfile::new_adobe_rgb().encode()` — add `moxcms = "=0.8.1"` under
  `[dev-dependencies]` for that, the only other `Cargo.toml` change allowed):
  - an Adobe RGB ICC file gives `File("<its file name>")`, and its profile converts sRGB
    (255,0,0) to within 1 of (219,0,0) (the same check as T68, through `pw_colour::Transform`);
  - `Ok(None)` gives `Missing` with an sRGB profile: an sRGB → that profile transform is
    `is_identity()`;
  - a path that does not exist, a file of garbage bytes, a gray profile
    (`ColorProfile::new_gray_with_gamma(2.2).encode()`) and `Err("x")` each give `Unusable` with an
    sRGB profile (identity as above);
  - `window_title` gives the three texts exactly;
  - `primary_monitor_profile_path()` returns without panicking (no assertion on the value: it
    depends on the machine).
- **Check by hand (owner, after review):** start RawStride; the title says
  "RawStride  ·  Skärmprofil: CX240(…)08CAL (DP).icc"; the images look exactly as before
  (no conversion yet).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has the new tests in the
  `pw-loupe` row and the new totals; `git diff --stat` shows only the files above.

### T70. ADR 0016 C3 (TIFF): `pw-tiff` reads the embedded ICC profile and converts 16-bit data before the reduction — Done

- **Can start now.** T68 accepted. Independent of T69 and T71.
- **Crates:** `pw-tiff` only. Nothing calls the new function yet (that is C4); the Tauri app and
  `decode_rgb8` are unchanged.
- **Files:** `crates/pw-tiff/Cargo.toml`; `crates/pw-tiff/src/lib.rs`; `crates/pw-tiff/src/convert.rs`;
  `crates/pw-tiff/src/tests.rs`; `Cargo.lock` (by cargo); `docs/test-inventory.md`. This task may
  change `crates/pw-tiff/Cargo.toml` as written below and nothing else in it.
- **Why:** ADR 0016 §3.1 and §4.1: the owner's TIFFs are 16-bit ProPhoto RGB; the profile is in
  tag 34675 and must be applied **before** `u16_to_u8`, or the 8-bit result bands.
- **What:**
  1. `crates/pw-tiff/Cargo.toml`: `pw-colour.workspace = true` under `[dependencies]`; under
     `[dev-dependencies]` `moxcms = "=0.8.1"` (to encode ICC profiles in tests).
  2. **`pub enum SourceColour { Embedded, AssumedSrgb, Unusable(String) }`** (`Debug`, `Clone`,
     `PartialEq`, `Eq`): which source profile was used. `Embedded`: the file's ICC profile.
     `AssumedSrgb`: no profile, or a layout whose profile is not applied (below). `Unusable(detail)`:
     the file has a profile that could not be used (the `ColourError` text); the pixels are treated
     as sRGB.
  3. **`pub fn decode_rgb8_for_display(path: &Path, limit: Option<usize>, destination:
     &pw_colour::Profile, cache: &pw_colour::TransformCache) -> Result<(Rgb8, SourceColour),
     TiffError>`**, under the same decode lock as `decode_rgb8`, for the whole decode and
     conversion:
     - Read the ICC profile with `find_tag(Tag::Unknown(34675))` and `into_u8_vec` (the `tiff` crate
       reads type UNDEFINED, which Lightroom writes, as bytes). A tag that cannot be read counts as
       `Unusable`, never as an error of the decode.
     - **Source profile:** for `Layout::Rgb` and `Layout::Rgba`, the ICC profile through
       `Profile::from_icc` (`Embedded`), sRGB when there is none (`AssumedSrgb`), sRGB when it fails
       (`Unusable`). For Gray, GrayAlpha and CMYK the profile is **not** applied (a gray or CMYK
       profile does not describe the RGB that `to_rgb8` makes; ADR 0016 §3.2): sRGB,
       `AssumedSrgb`.
     - Get the transform with `cache.get(source, destination)`; a failure there is `Unusable` with
       sRGB as the source.
     - **16-bit RGB or RGBA:** make 16-bit RGB first (a new helper `to_rgb16` in `convert.rs`, the
       16-bit twin of `to_rgb8` for `Rgb` and `Rgba` only, with the same "image data is short" error;
       for `Rgb` it uses the samples without a copy when their length is exact), then
       `convert_rgb16`, then `u16_to_u8`. **8-bit and float:** today's path to `Rgb8`, then
       `convert_rgb8`. Gray, GrayAlpha and CMYK at 16 bit keep today's path (reduce, then
       `to_rgb8`, then `convert_rgb8` from sRGB).
     - A conversion error (cannot happen for whole pixels, but handled) returns the image as it is
       with `Unusable`.
  4. `decode_rgb8` and everything the Tauri app uses stay unchanged; their tests stay unchanged.
- **Rules:** rules 3–7 and 10–12. `git diff Cargo.lock`: only `pw-colour` and `moxcms` in
  `pw-tiff`'s dependency list; no new package.
- **Tests to add** (synthetic files as the existing `pw-tiff` tests; the ICC tag written with
  `encoder().write_tag(Tag::Unknown(34675), &bytes[..])`, plus **one hand-built file** with the tag
  as type UNDEFINED (7), as Lightroom writes it). The references are T68's tables
  (`PRO_PHOTO_TO_SRGB`, `SRGB_TO_ADOBE`); copy the values into `tests.rs`. Each channel within 1:
  - 8-bit RGB with a ProPhoto ICC (`ColorProfile::new_pro_photo_rgb().encode()`), destination sRGB:
    the ProPhoto → sRGB references, `Embedded`; the same with the hand-built UNDEFINED tag;
  - **16-bit** RGB with the ProPhoto ICC (values × 257), destination sRGB: the same references,
    `Embedded`;
  - 16-bit RGBA with the ProPhoto ICC: the same, alpha dropped;
  - 8-bit RGB without ICC, destination Adobe RGB: the sRGB → Adobe RGB references, `AssumedSrgb`;
  - **identity (F3):** 8-bit RGB, 16-bit RGB, Gray and CMYK without ICC, destination sRGB: pixels
    equal to `decode_rgb8` byte for byte, `AssumedSrgb`;
  - a 100-byte garbage ICC tag and a gray ICC on an RGB file: `Unusable`, and with destination sRGB
    the pixels equal `decode_rgb8`;
  - a Gray file **with** a gray ICC: `AssumedSrgb`, pixels equal `decode_rgb8` (destination sRGB);
  - the limit, a truncated file and a missing file give the same errors as `decode_rgb8`;
  - **16 bit is better than 8 bit:** a 16-bit ProPhoto ramp of 4 096 dark values (0 to 4 095,
    grey) converted to sRGB gives at least as many distinct 8-bit output values as the same ramp
    reduced to 8 bit first and then converted (shows that the conversion runs before the
    reduction);
  - **real files** (`#[ignore]`, `PW_TESTDATA`, as the existing real-file tests): every TIFF there
    decodes with `Embedded` against destination sRGB.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has the new tests in the
  `pw-tiff` row and the new totals; `git diff --stat` shows only the files above.

### T71. ADR 0016 C3 (JPEG): `pw-libjpeg` returns the ICC profile, `pw-raw` the Exif colour space — Done

- **Can start now.** Independent of T69 and T70. Nothing calls the new code yet (that is C4).
- **Crates:** `pw-libjpeg`, `pw-raw`.
- **Files:** `crates/pw-libjpeg/src/lib.rs`; `crates/pw-raw/src/lib.rs`; `crates/pw-raw/src/tiff.rs`;
  `crates/pw-raw/src/jpeg.rs` (only if the JPEG path needs it); `docs/test-inventory.md`. No
  `Cargo.toml` changes; `Cargo.lock` must not change.
- **Why:** ADR 0016 §3.1 and §4.1: a JPEG's source profile is the ICC profile in APP2, else the
  Exif Adobe RGB marker, else sRGB.
- **What:**
  1. **`pw-libjpeg`:** declare `tj3Set(handle, param, value) -> c_int`,
     `tj3GetICCProfile(handle, *mut *mut u8, *mut usize) -> c_int` and `tj3Free(*mut c_void)`
     (libjpeg-turbo 3.2.0 in vcpkg has them); `TJPARAM_SAVEMARKERS = 25` (its position in the
     `TJPARAM` enum of that `turbojpeg.h`; write that in a comment).
     - `Decoder::new` sets `TJPARAM_SAVEMARKERS` to `2` (keep ICC markers when reading a header); a
       failing `tj3Set` is an error from `new`.
     - New `pub fn icc_profile(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>>`: `header(data)`,
       then `tj3GetICCProfile`; a null buffer or size 0 is `Ok(None)`; otherwise copy the bytes into
       a `Vec` and free the buffer with `tj3Free` on every path. `// SAFETY:` comments as in the
       rest of the file. `decode` is unchanged in behaviour.
  2. **`pw-raw`:** `ImageInfo` gets `pub colour_space: ExifColourSpace` with
     `pub enum ExifColourSpace { #[default] Unknown, Srgb, AdobeRgb, Uncalibrated }`
     (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Default`), filled from the first Exif IFD that
     has it, like the other Exif fields:
     - ColorSpace (0xA001, SHORT) = 1 → `Srgb`;
     - ColorSpace = 0xFFFF and the Interoperability IFD (0xA005 in the Exif IFD) has
       InteroperabilityIndex (0x0001, ASCII) "R03" → `AdobeRgb` (the DCF rule cameras use);
     - ColorSpace = 0xFFFF otherwise → `Uncalibrated`; no ColorSpace → `Unknown`; any other value
       → `Unknown`.
     For a RAW file this is the camera's colour-space setting, which its embedded JPEG previews
     follow; for a JPEG file it comes from its APP1 Exif. Malformed values are ignored, never an
     error of `inspect`.
- **Rules:** rules 3–7 and 10–12.
- **Tests to add:**
  - `pw-libjpeg`: the 16 × 8 JPEG fixture of `pw-thumbcache`
    (`#[path = "../../pw-thumbcache/src/jpeg_fixture.rs"] mod jpeg_fixture;` in the test module)
    gives `Ok(None)`; the same JPEG with one APP2 segment `ICC_PROFILE\0` + sequence 1 + count 1 +
    100 test bytes inserted after SOI gives exactly those bytes; with the bytes split over **two**
    APP2 segments (sequence 1 and 2 of 2) gives them joined; `decode` of the JPEG with APP2 gives the
    same pixels as without; garbage data gives an error.
  - `pw-raw`: generated files as the existing tests (`lib.rs` builds TIFF and JPEG structures by
    hand): ColorSpace 1 → `Srgb`; 0xFFFF with "R03" → `AdobeRgb`; 0xFFFF with "R98" → `Uncalibrated`;
    0xFFFF without an Interop IFD → `Uncalibrated`; no ColorSpace → `Unknown`; a JPEG file with
    APP1 Exif ColorSpace 1 → `Srgb`; an Interop offset pointing outside the file → no panic,
    `Uncalibrated`.
  - `pw-raw` real files (`#[ignore]`, `PW_TESTDATA`): print each file's `colour_space`, assert
    nothing (the reviewer reads the output).
- **Done when:** `.\tools\check.ps1` is all OK; `git diff Cargo.lock` is empty;
  `docs/test-inventory.md` has the new tests and totals; `git diff --stat` shows only the files above.

### T72. ADR 0016 C4: the loupe and the thumbnails convert to the monitor profile — Done

- **Can start now.** T68–T71 are done: `pw-colour` (transforms), the monitor profile in `pw-loupe`
  (T69), `pw_tiff::decode_rgb8_for_display` (T70), `Decoder::icc_profile` and
  `ImageInfo::colour_space` (T71). **This is the step that changes what the owner sees.**
- **Crates:** `pw-colour`, `pw-viewcore` (feature `native`), `pw-thumbcache`, `pw-loupe`.
- **Files:** `crates/pw-colour/src/lib.rs`, `crates/pw-colour/src/tests.rs`;
  `crates/pw-viewcore/Cargo.toml`, `crates/pw-viewcore/src/native.rs`,
  `crates/pw-viewcore/tests/native_files.rs`; `crates/pw-thumbcache/Cargo.toml`,
  `crates/pw-thumbcache/src/extract.rs`, `crates/pw-thumbcache/src/worker.rs`;
  `crates/pw-loupe/src/main.rs`; `Cargo.lock` (by cargo); `docs/test-inventory.md`. The two
  `Cargo.toml` files may change only as written below.
- **Why:** ADR 0016 §2, §3.1, §4.1 ("Applying it in the loupe", "Thumbnails"), §4.2 (caches hold
  display-ready pixels), F3, F5, F6.
- **Not in this task:** the status-line note for an embedded profile that cannot be used (ADR 0016
  §5) needs a new field through the loupe core; it is a later task. Here an unusable profile only
  falls back to sRGB.
- **What:**
  1. **`pw-colour`: the JPEG rule in one place.** `pub fn jpeg_source_profile(icc: Option<&[u8]>,
     exif_adobe_rgb: bool) -> Profile` (ADR 0016 §3.1): the ICC profile through
     `Profile::from_icc` when there is one and it parses as RGB; else `Profile::adobe_rgb()` when
     `exif_adobe_rgb`; else `Profile::srgb()`. An ICC profile that cannot be used falls through to
     the Exif rule, then sRGB. Never an error.
  2. **`pw-viewcore`** (`Cargo.toml`: `pw-colour = { workspace = true, optional = true }` and
     `"dep:pw-colour"` in the `native` feature):
     - `NativeBackend` gets `destination: pw_colour::Profile` and `cache:
       Arc<pw_colour::TransformCache>`. It becomes `Clone` instead of `Copy` (nothing needs `Copy`;
       check with the compiler). `Default` and `with_tiff_limit` use `Profile::srgb()`, so every
       existing test sees identity transforms and unchanged pixels (F3). New
       `pub fn with_monitor_profile(self, profile: pw_colour::Profile) -> NativeBackend`.
     - **TIFF:** `open_tiff` calls `pw_tiff::decode_rgb8_for_display(path, Some(limit),
       &destination, &cache)` instead of `decode_rgb8` (the `SourceColour` is not used yet). The
       session holds the converted image, so `render_half` and `render_region` stay as they are.
     - **JPEG** (embedded preview and plain JPEG file): `open_jpeg` gets the transform once from
       `jpeg_source_profile(decoder.icc_profile(&data)?, info.colour_space ==
       ExifColourSpace::AdobeRgb)` (`icc_profile` before `header`; an `icc_profile` error is a
       `jpeg_error` like `header`'s) and the cache, and keeps it in `JpegSession` as
       `Arc<Transform>`. `render_half` converts its output; the full decode for `render_region` is
       converted **once** when it is made, before it is stored in `full`, so crops need no work.
     - **RAW:** LibRaw renders sRGB (ADR 0006): `render_half` and `render_region` convert their
       output with the sRGB → destination transform. Keep it in `NativeSession::Raw` next to the
       LibRaw session (for example `Raw(Session, Arc<Transform>)` or a small struct).
     - A conversion error (cannot happen for whole RGB pixels) is a `DecodeError::new` with the
       `ColourError` text.
  3. **`pw-thumbcache`** (`Cargo.toml`: `pw-colour.workspace = true` under `[dependencies]`;
     `moxcms = "=0.8.1"` under `[dev-dependencies]` for test profiles):
     - `thumbnail(path, cell, decoder)` gets two more parameters, `destination: &Profile` and
       `cache: &TransformCache`.
     - **JPEG and embedded previews:** the source profile with `jpeg_source_profile` as in the
       loupe (`decoder.icc_profile(&bytes)` and `info.colour_space`); convert **after** the scaled
       decode and before `orient` (the thumbnail is small, so this is cheap).
     - **TIFF:** `pw_tiff::decode_rgb8_for_display(path, Some(TIFF_LIMIT), destination, cache)`
       instead of `decode_rgb8`, then `fit`, `downscale` and `orient` as today.
     - `FileThumbSource` holds the destination and an `Arc<TransformCache>` shared by the worker
       threads. New `pub fn start_with_monitor_profile(profile: Profile) -> io::Result<ThumbWorker>`
       (the default thread count and cache limit); `start()` and `start_with_limits` use sRGB.
  4. **`pw-loupe/src/main.rs`:** the backend becomes
     `NativeBackend::with_tiff_limit(…).with_monitor_profile(monitor_profile.profile.clone())` and
     the thumbnails `ThumbWorker::start_with_monitor_profile(monitor_profile.profile.clone())`.
     Remove the `#[allow(dead_code)] // used by ADR 0016 C4` on `monitor_profile` only if the field
     is now read; if not, keep the field and the allow (the status-line task will read it).
- **Rules:** rules 3–7 and 10–12. `git diff Cargo.lock`: only `pw-colour` (and `moxcms` for
  `pw-thumbcache`'s tests) in the dependency lists of `pw-viewcore` and `pw-thumbcache`; no new
  package. Every existing test keeps passing **unchanged**: with an sRGB destination nothing may
  change (F3). If one has to change, stop and explain.
- **Tests to add** (synthetic files as the existing tests of each crate; ICC profiles from
  `moxcms`; a pixel is "converted" when it equals the plain result run through
  `TransformCache::get(source, destination)` and `convert_rgb8`, byte for byte):
  - `pw-colour`: `jpeg_source_profile` checked by identity (`Transform::new(&result,
    &expected).is_identity()`): an Adobe RGB ICC with or without the marker → the profile from
    `Profile::from_icc` of the same bytes; no ICC and the marker → `Profile::adobe_rgb()`; no ICC and
    no marker → `Profile::srgb()`; a garbage or gray ICC and the marker → `Profile::adobe_rgb()`; a
    garbage ICC and no marker → `Profile::srgb()`.
  - `pw-viewcore` (`native.rs` tests):
    - a TIFF with a ProPhoto ICC and the backend's destination sRGB: the opened image equals
      `decode_rgb8_for_display` for that file; the same TIFF with the default backend and a TIFF
      without ICC: equal to `decode_rgb8` (F3);
    - a plain JPEG file (the `pw-thumbcache` fixture through `#[path]`, as T71 did) with
      destination Adobe RGB: `render_half` and a `render_region` are the plain decodes converted
      sRGB → Adobe RGB; with destination sRGB: the plain decodes byte for byte;
    - the same JPEG with an APP2 ICC holding the Adobe RGB profile and destination Adobe RGB
      parsed from **the same bytes**: unchanged (identity by ICC bytes);
    - the same JPEG with an APP1 Exif ColorSpace 0xFFFF and InteroperabilityIndex "R03" (build it
      as the `pw-raw` tests of T71 do) and destination sRGB: converted Adobe RGB → sRGB.
  - `pw-viewcore` real files (`tests/native_files.rs`, `#[ignore]`, `PW_TESTDATA`): for one
    camera RAW in RAW mode and in JPEG mode, a backend with destination Adobe RGB gives the
    `render_half` of the sRGB backend converted sRGB → Adobe RGB (JPEG mode: from the file's own
    source profile); print the time of each `render_half` with and without conversion.
  - `pw-thumbcache`: the fixture JPEG thumbnail with destination Adobe RGB is the sRGB thumbnail
    converted; a uniform ProPhoto TIFF of (150,110,90) with its ICC and destination sRGB gives
    (200,118,105) within 1 in every pixel (T68's reference); with destination sRGB and no ICC,
    thumbnails equal today's byte for byte.
- **Check by hand (owner, after review; ADR 0016 C5):** the ProPhoto TIFFs are no longer dull, sRGB
  images are no longer oversaturated, the `_TN0633x` Adobe RGB shots look like Lightroom in JPEG
  mode, side by side with Lightroom on the CX240 (F5); `latency.ps1` on the TIFF and RAW sets (F6).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` has the new tests and
  totals; `git diff --stat` shows only the files above.

### T73. ADR 0016 §5: the loupe says when an embedded colour profile cannot be used — Done

- **Can start now.** T72 is done. Independent of T74's text, but T74 describes it, so T73 first.
- **Crates:** `pw-colour`, `pw-viewcore`, `pw-loupe`.
- **Files:** `crates/pw-colour/src/lib.rs`, `crates/pw-colour/src/tests.rs`;
  `crates/pw-viewcore/src/backend.rs`, `crates/pw-viewcore/src/core.rs`,
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/native.rs`; `crates/pw-loupe/src/lib.rs`;
  any test file that builds a `Frame` literal (today `crates/pw-loupe/src/lib.rs`; add
  `profile_unusable: false` and nothing else there); `docs/test-inventory.md`. No `Cargo.toml`
  change; `Cargo.lock` must not change.
- **Why:** ADR 0016 §3.1 ("Display") and §5 (refined 2026-11-11), GR9: the user must see when the
  colours rest on a fallback because the file's own profile was broken.
- **What:**
  1. **`pw-colour`:** `pub fn jpeg_source_profile_checked(icc: Option<&[u8]>, exif_adobe_rgb: bool)
     -> (Profile, bool)`: the same choice as `jpeg_source_profile`, plus `true` when an ICC profile
     was present but could not be used. `jpeg_source_profile` becomes a wrapper that drops the flag
     (its behaviour and tests unchanged).
  2. **`pw-viewcore` `Backend`:** a new method with a default,
     `fn profile_unusable(&self, _session: &Self::Session) -> bool { false }`, so the fake backends
     of the tests need no change.
  3. **Core and state:** after a successful open the worker asks
     `backend.profile_unusable(&session)` and the state keeps the keys for which it is true in a new
     `HashSet<Key>` next to `sizes`: cleared where `sizes` is cleared, moved by `set_files` exactly
     as `sizes` is moved. `Frame` gets `pub profile_unusable: bool` (true when the current key is in
     the set), documented like the other fields.
  4. **`native.rs`:** the flag per session. TIFF: `SourceColour::Unusable(_)` from
     `decode_rgb8_for_display` (keep it next to the image in `NativeSession::Tiff`). JPEG:
     `jpeg_source_profile_checked`, plus `true` when `NativeBackend::transform` had to fall back to
     sRGB because no transform could be built from the source profile. RAW: always `false`.
     `profile_unusable` returns the session's flag.
  5. **`pw-loupe` `status_line`:** when `frame.profile_unusable` and the source text is shown (not
     "kan inte visas", not "förbereder …"), insert the part
     **"Inbäddad färgprofil kunde inte användas"** right after the source text, with the usual
     `"  ·  "`. Nothing else in the line changes. The grid's status line is not changed.
- **Rules:** rules 3–7 and 10–12. Existing tests keep their assertions; the only allowed change to
  existing test code is the new field in `Frame` literals.
- **Tests to add:**
  - `pw-colour`: `jpeg_source_profile_checked`: no ICC → `false` (with and without the marker); a
    usable Adobe RGB ICC → `false`; a garbage ICC → `true` with sRGB, and with the marker `true` with
    Adobe RGB; a gray ICC → `true`.
  - `pw-viewcore` `native.rs`: a TIFF with a garbage ICC tag → `profile_unusable` true; with a
    ProPhoto ICC and without ICC → false; the fixture JPEG with a garbage APP2 ICC → true; without
    APP2 and with a usable one → false.
  - `pw-viewcore` state (with the simulated backend of the existing state or core tests; a fake
    that reports `true` for one path): the frame of that image has `profile_unusable` true and the
    others false; after `set_files` reorders the list the flag follows the path; after
    `open_folder` it is gone.
  - `pw-loupe`: `status_line` of a ready frame with the flag contains
    "TIFF-fil 6 × 4  ·  Inbäddad färgprofil kunde inte användas  ·  " (or the JPEG equivalent);
    without the flag the line equals today's; a `Preparing` frame without size and an `Error` frame
    with the flag do not show the part.
- **Check by hand (owner, after review):** a copy of a JPEG with its APP2 profile damaged (the
  reviewer provides one) shows the part in the loupe status line; the DxO and Sony files do not.
- **Done when:** `.\tools\check.ps1` is all OK; `git diff Cargo.lock` is empty;
  `docs/test-inventory.md` has the new tests and totals; `git diff --stat` shows only the files above.

### T74. ADR 0016 C6: documentation after milestone 7 (colour management) — Done

- **Can start after T73 is done.** Documentation only; no code, no tests change.
- **Files:** `docs/adr/0016-colour-management.md`; `docs/adr/0015-egui-tiff-viewing.md`;
  `docs/adr/0007-metadata-iptc.md` (one status line); `docs/architecture.md`;
  `docs/GUI_REQUIREMENTS.md` (GR8 only); `docs/README.md` (the ADR list line only); the root
  `README.md`. Change only the lines needed; rules 10–12.
- **Why:** ADR 0016 §9 C6, as T67 did for ADR 0015.
- **What:**
  1. **ADR 0016:** after the status, an "Implemented (date)" line: C1–C4 = T68–T72, the status-line
     note = T73, C5 accepted by the owner on 2026-11-11, C6 = T74. §7: a results table under the F1–F6
     table, in the style of ADR 0015 §7, filled **from the log rows of T68–T73 and C5** in
     `docs/agent-tasks.md` (F1: T68 within 1; F2: T70/T71 tests and the real-file results; F3:
     unchanged tests and identity; F4: the fallback tests; F5: the objective patch check within 1 and
     the owner's DxO check; F6: the `latency.ps1` result **if a log row gives one**, otherwise
     "not measured yet; the render timings of the T72 review show the conversion within the noise").
     §9: C6 "**Done:** T74". §12: one point that milestone 7 is complete.
  2. **ADR 0015:** §2 "ADR 0007 becomes milestone 7" and §9 D7 and §12 "milestone 7": add
     "(moved to milestone 8 by ADR 0016 Q1, 2026-11-11)" once each, nothing else; §3.2 and §12 colour
     points: add "(done: ADR 0016, 2026-11-11)".
  3. **ADR 0007:** its "Planned as milestone 7" status line becomes milestone 8 with
     "(ADR 0016 Q1, 2026-11-11)"; status still "proposed".
  4. **architecture.md:** a new `pw-colour` row (ICC profiles and transforms with `moxcms` 0.8.1,
     the transform cache, the JPEG source rule; ADR 0016) after `pw-tiff`; one short addition each in
     the `pw-tiff`, `pw-thumbcache`, `pw-viewcore`, `pw-loupe`, `pw-libjpeg` and `pw-raw` rows (what
     T69–T73 added there). Diagrams unchanged.
  5. **GR8:** the status sentence: the open owner question about colour management is answered —
     done in ADR 0016 (2026-11-11); full IPTC handling (ADR 0007) is now milestone 8.
  6. **docs/README.md:** add "0016 colour management – done 2026-11-11" to the ADR list.
  7. **Root README.md:** list colour management (conversion to the monitor profile) with the other
     done features of RawStride.
- **Done when:** `.\tools\check.ps1` is all OK (no code changed) and `git diff --stat` shows only the
  files above.

### T75. ADR 0009 S6: acceptance measurement of RawStride and a results report — Done

- **Review 2026-11-11: accepted.** Waiting for the owner's commit.

- **Starts after the owner's runs (S6a below).** The report itself is documentation only; no code, no
  `Cargo.toml`. For a Claude agent.
- **Why:** ADR 0009 §9 step S6 was never done. There is no log row, no `D:\PhotoWorkflowData\analysis\s6`
  and no report; ADR 0009's status still ends at S0. GR8 says the switch from the Tauri app waits for
  the acceptance tests, and S6 is those tests. Since S5 the program has gained the contact sheet,
  tabs, import, ratings, filter, TIFF and colour management (ADR 0010–0016), so S6 measures
  RawStride as it is now, not the milestone 1 loupe alone. The earlier runs in
  `latency-rawstride.csv` (`t57-…`, `t66-…`, `c4-…`) cover next/previous only.
- **S7** (TESTING.md: the loupe's tests) is a separate task after this one.

**S6a – the owner's runs** (not an agent task). The method is ADR 0009 §8.2, with these changes:

- the window title is **"RawStride"** (ADR 0012), not "PhotoWorkflow Loupe";
- RawStride is started **with a file path**, which opens the loupe; a folder opens the grid (F6 log
  row, 2026-11-11);
- a release build of the committed HEAD; the window maximised on the main screen; default budgets;
- `latency.ps1 -Title RawStride -Region 0.12,0.1,0.5,0.5 -Out
  D:\PhotoWorkflowData\pm-study\results\latency-rawstride.csv`, with `-Proc` set to RawStride's
  process name (check it in Task Manager) so the memory columns are filled;
- every label starts with `s6-`; screenshots and notes go to `D:\PhotoWorkflowData\analysis\s6\`;
- each run gets one row in the Log ("S6a"), as for F6; an invalid run is marked invalid with the
  reason, and repeated.

| # | Check | Requirement | Run | Label |
|---|---|---|---|---|
| 1 | Next at fit, RAW mode, per camera (A7R IV, A1, RW2, DNG) | V1, V2 | `-Key RIGHT -Count 20 -Gap 1500` | `s6-raw-fit-next-<camera>` |
| 2 | The same in JPEG mode | V1, V2 | as 1 | `s6-jpg-fit-next-<camera>` |
| 3 | Next at 100 % (Z first), RAW and JPEG, A7R IV | V1, V6 | as 1 | `s6-raw-100-next`, `s6-jpg-100-next` |
| 4 | Z after a pause on a shown image (ADR 0008 S6), A7R IV and RW2; Z toggles, so the odd presses are fit → 100 % | V3, V7 | `-Key Z -Count 10 -Gap 3000` | `s6-raw-z-pause-<camera>` |
| 5 | Z right after an image change (ADR 0008 S1); Z toggles, so the odd presses are fit → 100 %; with a short delay the image change itself can be the first change, so the time to the sharp 100 % view is `settled_ms` | V7 | `-PreKey RIGHT -PreDelay 0`, `50` and `150`, `-Key Z` | `s6-raw-z-next-<delay>` |
| 6 | Z in JPEG mode on the A1 (full-size embedded JPEG) | V3 | `-Key Z -Count 10 -Gap 3000` | `s6-jpg-z-a1` |
| 7 | Rapid stepping at 100 %, about 7 images/s, RAW and JPEG. `latency.ps1` waits `-Quiet` ms without change after each press, so the defaults cannot reach 7/s; with these values a press without a change within 70 ms ends after 70 ms (empty `first_ms` = over the V4 limit) | V4, V2 | `-Key RIGHT -Count 30 -Quiet 1 -Timeout 70 -Gap 70` | `s6-raw-100-7fps`, `s6-jpg-100-7fps` |
| 8 | R on a shown image, and right after an image change | G4 (ADR 0008 S3) | `-Key R`; `-PreKey RIGHT -Key R` | `s6-r-shown`, `s6-r-next` |
| 9 | Free zoom from fit, anchor at the centre (A) | V2, §3.4 | `-Key ADD -Count 5`; then `-Key SUBTRACT -Count 5` | `s6-zoom-plus`, `s6-zoom-minus` |
| 10 | Back after three steps (ADR 0008 S5) | V1 | `-Key LEFT` after RIGHT × 3 | `s6-raw-back` |
| 11 | First image after opening, cold (after a restart) and warm, both modes | V5 | by hand or a screen recording; the note names the method | – |
| 12 | Long session: about 15 min of mixed stepping and Z in RAW mode on the A7R IV files; private memory at start (L1), after the first image (L2), after 10 min (L3), after closing the tab (L4); a clean exit | V8 | the `-Proc` columns of runs 1–10, plus Task Manager | `s6-mem` (note) |
| 13 | By hand, with screenshots: zoom and position persist and small images are centred (V6); the status line in both modes, for RAW, JPEG and TIFF (G1); a truncated RAW file shows an error, never bad data (G3); the mode never changes by itself (G4); focus after minimise, restore, resize and the import dialog (GR6, GR7) | V6, G1–G4, GR6, GR7 | checklist | – |

TIFF and RAW next with colour management are already measured (`c4-tiff-next-2`; the last 20 rows of
`c4-raw-next`, F6) and are not repeated. DPI change, a second screen and GPU device loss stay postponed
(GR7, ADR 0008 §18.4).

**T75 – the report** (Claude agent, after S6a):

- **Files:** a new `docs/analyses/<date of the runs>-adr0009-s6-acceptance.md` (from
  `docs/analyses/TEMPLATE.md`); `docs/adr/0009-egui-loupe-migration.md` (the status list and §9
  only); `docs/GUI_REQUIREMENTS.md` (the GR8 status sentence only). This file: rule 11.
- **Source of truth:** the `s6-` rows of `latency-rawstride.csv`, the S6a log rows and the notes in
  `D:\PhotoWorkflowData\analysis\s6\`; for TIFF/RAW with colour management, the F6 log rows. Write only
  what they record. The CSV uses decimal commas.
- **What:**
  1. **The report:** one table row per requirement (V1–V8, G1–G4, GR6, GR7) with the target, what
     was measured (median, 95th percentile, maximum and n; never a median alone, ADR 0008 §8.3), the
     number of screen changes per press for V2, and **met / not met / not measured**. A column with
     the P7 figure (ADR 0008 §18.3) for comparison; P7 is not an acceptance criterion (TESTING.md §2).
     The known limits of ADR 0009 §8.3 in their own section, stated as expected. Invalid runs listed
     with the reason, not used. Measured values marked [M], interpretations [T], as in the ADRs.
  2. **ADR 0009:** a status line "S6 done (<date>): results in `docs/analyses/…`" after amendment 1;
     §9 S6 "**Done:** T75". Nothing else.
  3. **GR8:** after "the switch waits for the acceptance tests", add "S6 (ADR 0009) measured on
     <date>: <all met / the requirements not met>." Nothing else.
- **Runs 11–13 were deferred by the owner (2026-11-11).** The report marks V5, V8 over a long session
  (L1–L4) and the checks of run 13 (V6, G1, G3, G4, GR6, GR7) as **not measured**, says that S6 is
  partly done, and that `s6a.ps1 -Run 11,12,13` runs them later. The per-run memory columns
  (`proc_private_mb`) are reported as they are, without an L1–L4 claim. ADR 0009 §9 S6 then reads
  "**Partly done:** T75 (runs 11–13 open)" instead of "**Done:** T75", and the GR8 sentence says the same.
- **Rules:** the report draws no conclusion about the switch; that is the owner's decision. English,
  the style of the existing documents; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

### Milestone 8 (ADR 0007, IPTC metadata): steps

ADR 0007 was accepted by the owner on 2026-11-11 (revision 9). It has no step list of its own, so
the steps are kept here. The first four tasks (T76–T79) are plain Rust in `pw-metadata`, testable
without ExifTool; nothing in RawStride or the Tauri app uses them yet. The old API
(`write_rating`, `write_iptc`, `read_ratings`, `Iptc`, `LocalizedValue`) stays unchanged until I9.
The tasks for I5 and later are written after T76–T79 are accepted, because they build on those
types. T80–T83 (I5–I7) were written on 2026-11-11 after T76–T79 were done; I8 and later follow when
T83 is accepted.

**What the reviewer measured with ExifTool 13.59 before writing T80–T83** (2026-11-11, on small
JPEGs in a temporary folder):
- `-b -IPTC` gives the **raw IIM block** (in JSON as `base64:…`); its MD5 is
  `CurrentIPTCDigest`, and `Photoshop:IPTCDigest` is the stored checksum. Writing IIM does **not**
  update an existing `IPTCDigest`; `-IPTCDigest=new` must be given (then ExifTool warns "IPTCDigest
  is not current" when they differ). Without an existing digest none is created.
- `-j -struct -G1` gives XMP structures as JSON objects keyed by the ExifTool field names (`etTag`),
  lists as arrays, Lang Alt as one key per language (`XMP-dc:Description` is `x-default`,
  `XMP-dc:Description-sv` and so on), and keys named as `etXMP` in the table.
- XMP dates come in ExifTool's own form **even with `-n`**: `2026:11`, `2026:11:04 10:15`,
  `2026:11:04 10:15:30.5+01:00`. Truncation and zone are kept, so the form can be turned back into
  the XMP form without loss. A text such as `007` stays a JSON string; `xmp:Rating` is a JSON number.
- **Arguments on ExifTool's command line are converted to the Windows code page** (from PowerShell
  and from Git Bash alike: "Göteborg" was stored as Latin-1). `pw-metadata` sends arguments through
  `-stay_open` as UTF-8 and is not affected; **tests must use the `ExifTool` type, never a command
  line**. An argument file (`-@`) in UTF-8 also works.

**What the reviewer measured with ExifTool 13.59 before writing T84–T86** (2026-11-12, on a copy of
a Xiaomi DNG from `PW_TESTDATA`, a JPEG, a Panasonic RW2 and its sidecar, in a temporary folder):
- `-api ImageHashType=SHA256 -ImageDataHash` works for JPEG and DNG, and the DNG's hash is the same
  after a metadata write with `-o`.
- `-o <new file>` refuses a name that exists ("already exists", nothing written).
- The listing of a DNG copy written with `-o` differs only in `System` keys, `IFD0:TileOffsets`
  (its binary placeholder), `SubIFD:PreviewImageStart` and the written XMP plus
  `XMP-x:XMPToolkit`. Creating IIM in a JPEG adds `File:CurrentIPTCDigest`,
  `IPTC:EnvelopeRecordVersion`, `IPTC:ApplicationRecordVersion`, `IPTC:CodedCharacterSet`,
  `Photoshop:IPTCDigest` and the written tags.
- The Xiaomi DNG has 7 validation warnings of its own (`ExifTool:Validate` is `0 7 1` with `-n`),
  so the check is "no new warnings". In `-j -a` output only one `ExifTool:Warning` key survives,
  so the count comes from `Validate`.
- `-o x.xmp` with a RAW as the source copies the RAW's Exif into the new sidecar (`XMP-exif`,
  `XMP-tiff`, …). A new sidecar must be written without a source file: ExifTool then creates an
  XMP file holding only the given tags.

**What the reviewer measured with ExifTool 13.59 before writing T87–T91** (2026-11-12, on copies
in a temporary folder of a Ricoh GR III DNG with Photo Mechanic XMP (`TN002512.DNG`, 36 MB), a
Xiaomi DNG (`IMG_20261026_092351.dng`, 20 MB), a DxO JPEG (`TN005701_DxO.jpg`, 16 MB) and a
scanned TIFF with IIM and a matching `IPTCDigest` (`TN002683.tif`, 41 MB), all from
`PW_TESTDATA`; arguments through UTF-8 argument files):
- `-XMP-xmp:Rating=3 -XMP-xmp:Label=Red -o <copy>`: `ImageDataHash` equal for all four. The
  listing differs only in Rating, Label, `XMP-x:XMPToolkit` (JPEG and TIFF: another program's
  toolkit text replaced), pointers ending in `Offsets`/`Start` (DNG: `IFD0:StripOffsets`,
  `SubIFD:StripOffsets`, `Pentax:PreviewImageStart`, `SubIFD1:PreviewImageStart`,
  `SubIFD:PreviewImageStart`; TIFF: `IFD0:StripOffsets`, `SubIFD:StripOffsets`) and, for the TIFF,
  `Validate` going from 6 to 5 warnings. All of it is allowed by T84's `verify_copy`.
- Clearing both again (`-XMP-xmp:Rating= -XMP-xmp:Label=`) gives back the original listing
  exactly, pointers included. A JPEG without XMP gets only `XMPToolkit` and the Rating; clearing
  the Rating removes both again.
- The embedded previews are **byte for byte equal** after the write (both DNGs' `PreviewImage`,
  3 032 721 and 8 546 141 bytes; the JPEG's IFD1 thumbnail); only their offsets move.
- Time per ExifTool call (a new process each time; its start, ~270 ms, subtracted): a T84 snapshot
  230–400 ms, the `-o` write 310–430 ms; today's in-place write 340–500 ms. With `read_metadata`,
  two snapshots and the final read, a rating on a DNG will take about 1.5–2 s instead of about
  0.5 s, plus LibRaw (~0.3 s for 61 MP, T90). The worker queues ratings, so the user does not
  wait, but a batch of 100 DNGs takes a few minutes.
- A structure written whole (`-XMP-iptcCore:CreatorContactInfo={CiEmailWork=…,CiTelWork=…,
  CiUrlWork=…}`) changes only that key; `-XMP-dc:Subject` given four times replaces the list and
  leaves `lr:HierarchicalSubject` as it was.
- `xmpRights:Marked`, which today's template writes as `True` with a copyright, is **not in the
  property table**, so the plan cannot write it (D6). The TIFF and the Ricoh DNG already carry it
  from Lightroom and Photo Mechanic.
- **Windows** (PowerShell, `[IO.File]::Replace`, which calls `ReplaceFileW`): while another handle
  has the original open for reading with the share mode Read or ReadWrite, the replace fails ("används
  i en annan process") and the original is unchanged; with ReadWrite + Delete it succeeds. Opening
  with share mode 0 (T85's `is_locked`) fails in all three cases. So **any** open handle makes
  `can_write` refuse the file as `Locked`. A LibRaw session keeps its file open until it is closed
  (`pw-libraw/csrc/pwlr.cpp`), so the view core must close its sessions first (T89).

**What the reviewer measured with ExifTool 13.59 for T92** (2026-11-12, after the review of T87;
copies of `TN002683.tif`, the Ricoh and the Xiaomi DNG):
- In a **TIFF or DNG**, `-IPTCDigest=new` given in the same call as an IIM write stores the MD5 of
  the new IIM block without the NUL padding ExifTool adds in the TIFF structure: the 292-byte block
  of `TN002683.tif` after a Rating write with `-IPTC:CodedCharacterSet=UTF8` has
  `CurrentIPTCDigest` `2c586d07…`, while the stored `IPTCDigest` `f379e478…` is the MD5 of the
  block less its last 3 bytes; `Validate` then has the warning `IPTCDigest is not current`. The
  same when IIM is created in a DNG or a TIFF without IIM. A JPEG is not affected (stored and
  current equal).
- A second call with only `-IPTCDigest=new -overwrite_original` on the written copy stores the
  current checksum; `ImageDataHash` unchanged (TIFF and Ricoh DNG).
- When the file had no Photoshop block, creating it adds the warning `Non-standard format (undef)
  for IFD0 0x8649 PhotoshopSettings` (Xiaomi DNG `0 7 1` → `0 8 1`, Ricoh DNG one more as well)
  even after the second call. Its listing changes only in keys T84 already allows
  (`File:CurrentIPTCDigest`, the IIM tags and record versions, `Photoshop:IPTCDigest`, the
  written XMP, `SubIFD:PreviewImageStart`).

**What the reviewer measured with ExifTool 13.59 for T93** (2026-11-12, after the review of T88;
a JPEG given City in IIM and XMP and a checksum, so `Validate` is `OK`): clearing City with
`-XMP-photoshop:City= -IPTC:CodedCharacterSet=UTF8 -IPTC:City=` removes the whole XMP packet and
leaves the stored `IPTCDigest` out of date (`Validate`: `IPTCDigest is not current`); with
`-IPTCDigest=new` added the stored and the current checksum are equal and `Validate` is `OK`
(`-IPTCDigest=` removes the checksum, also `OK`).

| Step | Task | Content | ADR 0007 | Can start |
|---|---|---|---|---|
| P1 | owner/reviewer | Save the IPTC TechReference 2025.1 JSON (`iptc-pmd-techreference_2025.1.json`, link in `docs/references.md`) in `D:\PhotoWorkflowData\references\`, with bytes and SHA-256 in its `README.md`. **Done 2026-11-11:** 140 839 bytes, SHA-256 `592c37b0…433128a8` | §3 | done |
| I1 | T76 | The property table, generated from the TechReference | §3 | now |
| I2 | T77 | Dates and times: truncated dates, zones never invented, IIM and Exif forms | §7, §5.3 | now |
| I3 | T78 | The value model: values, origins, Lang Alt rules, changes | §3, §8 | after T76 and T77 |
| I4 | T79 | IIM text rules: blank values, byte limits, prefixes, character encodings | §5.1–§5.4, §6.2 | now |
| I5a | T80 | IIM block: the datasets of the raw IIM block, decoded and turned into values | §5.1, §5.3, §5.4 | now |
| I5b | T81 | Reconciliation: IIM/XMP by checksum, Exif/XMP, Creator ↔ Artist/Photographer, `Cleared` as an input | §5.1, §6.2, §6.4, §6.5 | after T80 |
| D1 | owner | The XMP namespace (prefix and URI) of `Cleared` (§6.4: "fixed with the implementation"). Reading and writing `Cleared` gets its own small task after D1 | §6.4 | – |
| D2 | owner | `serde_json` (=1.0.154, already in `Cargo.lock` through the Tauri app, so nothing new is compiled) as a dependency of `pw-metadata`, to read ExifTool's `-j -struct` output. **Approved by the owner on 2026-11-11** | – | done |
| I6 | T82 | Reader with ExifTool for every format; sidecar rules and notices (foreign sidecar next to DNG, `<name>.<ext>.xmp`) | §4, §4.2 | after T81 |
| I7 | T83 | Write plan: XMP, IIM if present (or with the setting), existing Exif overlap fields, clearing, unresolved conflicts without checksum; checked against ExifTool | §5.2, §6.3, §8 | after T82 |
| I8a | T84 | The file snapshot (one ExifTool call: listing, `ImageDataHash`, `Validate`) and the verification of a written copy | §9.1, §9.3 | now |
| D3 | owner | `windows-sys` (=0.61.2, already in `Cargo.lock` through `pw-loupe`, so no new package) as a Windows-only dependency of `pw-metadata`, features `Win32_Foundation` and `Win32_Storage_FileSystem`, for `ReplaceFileW`, `MoveFileExW`, free space and read-only volumes. **Approved by the owner on 2026-11-12** | §9 | done |
| I8b | T85 | Windows file operations (`ReplaceFileW` with the rename fallback, free space, read-only volume, locks) and the write journal with recovery | §9.1, §9.4, §9.5 | now |
| I8c | T86 | The safe write transaction for embedded metadata and sidecars, with an `ImageCheck` the caller gives | §9 | after T84 and T85 |
| I9a | T87 | Ratings and labels through the write plan and the safe write (`write_rating_safely`); the reader also gives the plan's `FileState` | §8, §9, §15 | now |
| D4 | owner | A new workspace crate **`pw-imagecheck`** for the real `ImageCheck`, depending on `pw-metadata`, `pw-raw` and `pw-libraw` (all workspace crates: no new package from crates.io; `Cargo.lock` gains only the crate itself). Root `Cargo.toml`: `pw-imagecheck = { path = "crates/pw-imagecheck" }` under `[workspace.dependencies]`; `pw-loupe`: `pw-imagecheck.workspace = true` (T91). Why not elsewhere: `pw-metadata` must not depend on `pw-raw`/LibRaw (§9.3); in `pw-viewcore` the view core would depend on `pw-metadata`; in `pw-loupe` it would need `pw-libraw` as a direct dependency (also a `Cargo.toml` change) and the Tauri app could not use it. **Approved by the owner on 2026-11-12** | §9.3 | done |
| D5 | owner | The separator of `Append` and `Prefix` for a single text or Lang Alt (§10 says "a separator" without naming it). Reviewer's proposal: one space for every property. **Approved by the owner on 2026-11-12**: one space | §10 | done |
| D6 | owner | `xmpRights:Marked` ("Copyrighted" in Lightroom and Photo Mechanic): today's template writes `True` when a copyright is given, but the property is not in the table, so the plan cannot write it. Reviewer's proposal: add it to the table as a non-IPTC entry (as `xmp:Label`) and let `Iptc::to_template` set `True` with a Copyright Notice, as today. The alternative is to stop writing it. **Approved by the owner on 2026-11-12**: added to the table, `True` with a Copyright Notice | §10, §15 | done |
| D7 | owner | The **Tauri app** (`src-tauri`) also writes ratings and the ingest template. Reviewer's proposal: it keeps today's in-place write (`write_rating`, `write_iptc`) until it is retired (GR8), as RawStride is the program in use; moving it as well needs a release of `pw-rawview`'s LibRaw sessions and its own task. **Approved by the owner on 2026-11-12**: the Tauri app keeps today's write | §9, §15 | done |
| I9b | T88 | The ingest template with the modes Keep, Replace, Append, Prefix and Clear (`template::apply`, `apply_template_safely`, `Iptc::to_template`); lists of structures, variables and template files later | §8, §10, §15 | after T87 |
| I9c | T89 | The view core closes the RAW sessions of a file before it is written (`Releaser::release`) | §9.1 | now |
| I9d | T90 | The real `ImageCheck` in `pw-imagecheck`: previews byte for byte with `pw-raw`, the DNG opened with LibRaw (§9.3 points 3–4) | §9.1, §9.3 | now (D4 approved) |
| I9e | T91 | RawStride: the metadata worker and the ingest write through T87/T88 with the journal (recovery at start) and T90's check; Swedish texts. The choice of mode in the import dialog comes with I10 | §9, §9.5, §15 | after T87–T90 |
| D8 | owner | When the safe write creates the Photoshop block of a TIFF or DNG (to store a new `IPTCDigest`), ExifTool adds the validation warning `Non-standard format (undef) for IFD0 0x8649 PhotoshopSettings`, and T84's check refuses the copy for having one more warning. Reviewer's proposal: allow exactly one more warning (not error) in that case: the copy has `Photoshop:IPTCDigest`, the original has not, the file is a TIFF or DNG and the plan writes `-IPTCDigest=new`. The alternative, not writing a checksum in a TIFF/DNG without one, breaks §5.2. **Approved by the owner on 2026-11-12** | §5.2, §9.3 | done |
| I9f | T92 | A current IIM checksum in TIFF and DNG: a second ExifTool call on the copy writes `IPTCDigest`, and the D8 allowance in `verify_copy` | §5.2, §9.3, §12 | now |
| I9g | T93 | The IIM checksum stays current when a write changes IIM and no XMP is left (a file with a stored checksum) | §5.2, §12 | now |
| D9 | owner | What RawStride does when ExifTool is missing or older than 12.67 (§11 only says "checked at start"). Reviewer's proposal: RawStride starts anyway and shows one message at start; reading works as today; every write (ratings, the editor, the ingest template) is refused with a clear text until ExifTool is updated; a version that cannot be read counts as too old. The alternative is a warning only, with writes allowed. **Approved by the owner on 2026-11-12** | §11, §9 | done |
| D10 | owner | Where and how RawStride's settings are stored (it has no settings file yet). Reviewer's proposal: a text file `%LOCALAPPDATA%\RawStride\settings.txt` (the folder of the write journal), one `key = value` per line, no new dependency; for this user on this computer only. **Approved by the owner on 2026-11-12** | §5.2, §5.4 | done |
| D11 | owner | The Swedish names of the IPTC properties in the info panel and the editor. Reviewer's proposal: the import dialog's names where they exist, and for the rest: Titel (`dc:title`), Rubrik (`photoshop:Headline`), Beskrivning (`dc:description`), Alternativtext (`Iptc4xmpCore:AltTextAccessibility`), Nyckelord (`dc:subject`), Fotograf (`dc:creator`), Fotografens titel (`photoshop:AuthorsPosition`), Fotografens kontaktuppgifter (`Iptc4xmpCore:CreatorContactInfo`, its fields E-post, Telefon, Webb), Copyright (`dc:rights`), Credit (`photoshop:Credit`), Källa (`photoshop:Source`), Användarvillkor (`xmpRights:UsageTerms`), Webbadress för rättigheter (`xmpRights:WebStatement`), Skapad (`photoshop:DateCreated`), Plats (`Iptc4xmpCore:Location`), Stad (`photoshop:City`), Län/delstat (`photoshop:State`), Land (`photoshop:Country`), Landskod (`Iptc4xmpCore:CountryCode`), Instruktioner (`photoshop:Instructions`), Jobb-id (`photoshop:TransmissionReference`), Beskrivningens författare (`photoshop:CaptionWriter`), Betyg (`xmp:Rating`), Etikett (`xmp:Label`), Upphovsrättsskyddad (`xmpRights:Marked`), Plats där bilden togs (`Iptc4xmpExt:LocationCreated`), Plats som visas (`Iptc4xmpExt:LocationShown`), Bildskapare (`plus:ImageCreator`), Upphovsrättsinnehavare (`plus:CopyrightOwner`), Licensgivare (`plus:Licensor`), Person i bilden (`Iptc4xmpExt:PersonInImage`), Händelse (`Iptc4xmpExt:Event`), Digital källtyp (`Iptc4xmpExt:DigitalSourceType`), Prioritet, Kategori, Tilläggskategorier (the deprecated `photoshop:Urgency`, `photoshop:Category`, `photoshop:SupplementalCategories`); every other property with IPTC's English name. **Approved by the owner on 2026-11-12** | §3 | done |
| D12 | owner | What the editor edits in I10. Reviewer's proposal: **one image at a time** (several images later, as a template on the selection); these 22 fields in this order: Titel, Rubrik, Beskrivning, Alternativtext, Nyckelord, Fotograf, Fotografens titel, E-post, Telefon, Webb, Copyright, Credit, Källa, Användarvillkor, Skapad, Plats, Stad, Län/delstat, Land, Landskod, Instruktioner, Jobb-id; plus a choice for every unresolved conflict. Later: lists of structures (Location Created, Image Creator, …), PLUS fields (I11), the other properties. **Approved by the owner on 2026-11-12** | §8, §10, §11 | done |
| D13 | owner | The modes in the import dialog (§10). Reviewer's proposal: the Swedish names Behåll, Ersätt, Lägg till efter, Lägg till före, Rensa; the default Ersätt for every field (an empty field then does nothing, exactly as today); Fotograf offers Behåll, Ersätt, Lägg till efter; Copyright and Credit also Lägg till före; E-post, Telefon and Webb all five (no Rensa of copyright-management fields, §8; no "before" on a list). The modes are remembered during the session like the texts, not between runs. **Approved by the owner on 2026-11-12** | §8, §10 | done |
| D14 | owner | The confirmation before a Clear (§10: "once per template"). Reviewer's proposal: when an import is started with at least one field in Rensa, one window lists those fields ("Rensa och importera" / "Avbryt"); asked once per started import, never per image. The editor does not ask (emptying a field there is the user's own choice for one image). **Approved by the owner on 2026-11-12** | §10 | done |
| D15 | owner | The place of the info panel and the keys. Reviewer's proposal: a panel on the right side, in the loupe and in the contact sheet (the selected image), shown and hidden with `M`, hidden at start; the editor opens with `E` or the button "Redigera" in the panel, in its own window. **Approved by the owner on 2026-11-12** | – | done |
| I10a | T94 | The ExifTool version (at least 12.67) checked when the metadata worker starts; writes refused below it (D9) | §11 | now |
| I10b | T95 | The reader looks up the two sidecar names directly instead of listing the folder for every image (from T82) | §4.1, §4.2 | now |
| I10c | T96 | Validation warnings (§11): IIM byte limits, Alt Text length, Country Code form, Jobtitle without Creator, Digital Source Type, Image Registry Entry, Rating | §11 | now |
| I10d | T97 | Edits of single properties and sub-fields (`Edit`, `apply_edits`, `write_edits_safely`) with the plan's write notices; choosing a value decides an `Open` conflict | §5.1, §5.2, §8, §9 | now |
| I10e | T98 | The metadata worker reads the whole `MetadataRead`, writes edits and takes new `EditOptions` | §9 | after T94 and T97 |
| I10f | T99 | RawStride's settings file: create IIM, fallback encoding (D10) | §5.2, §5.4 | now |
| I10g | T100 | The import dialog: a mode per template field and the confirmation before a Clear (left over from T88/T91) | §10 | now |
| I10h | T101 | RawStride tells the user at start when ExifTool is missing or too old; the import worker checks it too | §11 | after T94 and T100 |
| I10i | T102 | The texts of the info panel: values with origin, conflicts with the rule, notices (D11) | §5.1, §5.4, §6.2, §6.4, §7 | now |
| I10j | T103 | The info panel in RawStride (one read in flight, after 200 ms) | §5.1, §6.2 | after T98, T101 and T102 |
| I10k | T104 | The editor's form: fields, conflict choices, edits, warnings in Swedish (D12) | §5.1, §7, §8, §11 | after T96, T97 and T102 |
| I10l | T105 | The editor window in RawStride, saved through the worker | §9 | after T103 and T104 |
| I10m | T106 | The settings window; the settings used by ratings, the editor, the panel and the import | §5.2, §5.4 | after T98, T99 and T101 |
| I11 | – | PLUS fields as fixed choice lists | §11, Q9 | **after the PLUS controlled vocabulary is read** (§16.4) |
| I12 | – | Real-file tests with the owner's level-C samples | §12, Q11 | when the owner provides the samples |
| I13 | – | Documentation after milestone 8 | – | last |

### T76. ADR 0007 I1: the IPTC property table, generated from the TechReference — Done

- **Can start now.** P1 is done: the JSON is
  `D:\PhotoWorkflowData\references\iptc-pmd-techreference_2025.1.json` (SHA-256 in that folder's
  `README.md`; the report checks it). Independent of T77 and T79.
- **Crates:** `pw-metadata` only. Nothing uses the table yet.
- **Files:** new `tools/iptc-table.ps1`; new `crates/pw-metadata/src/iptc_table.rs` (generated);
  new `crates/pw-metadata/src/properties.rs`; `crates/pw-metadata/src/lib.rs` (the `mod` lines and
  module comment line only); `tools/README.md` (one line for the script); `docs/test-inventory.md`.
- **Why:** ADR 0007 §3: one internal model keyed by the IPTC property's XMP name; IIM datasets and
  Exif tags are only representations; "their mapping is a table generated from the IPTC
  TechReference 2025.1 and checked into the repo, so a new IPTC version is a table update".
- **What:**
  1. **Script** `tools/iptc-table.ps1 -Json <path> [-Out <path>]` (default `-Out`
     `crates/pw-metadata/src/iptc_table.rs`), for Windows PowerShell 5.1 (`ConvertFrom-Json`, no
     modules). It reads `ipmd_top` and `ipmd_struct` and writes Rust source:
     - a header comment: generated by `tools/iptc-table.ps1` from `<file name>`
       (`release_timestamp` of the JSON), the SHA-256 of the JSON, "Do not edit: run the script
       again";
     - `pub(crate) static TOP: &[PropertyDef]`, one entry per `ipmd_top` entry, ordered by
       `sortorder`;
     - `pub(crate) static STRUCTS: &[StructDef]`, one entry per `ipmd_struct` structure ordered by
       name, each with its fields ordered by `sortorder`. The `AltLang` entry of `ipmd_struct` is
       left out: it describes the Lang Alt type (its fields `Note`, `BCP47langid_1`,
       `BCP47langid_toMany` have no `datatype`), it is not a structure;
     - per entry the JSON keys `name`, `ipmdschema`, `datatype`, `dataformat`, `propoccurrence`,
       `XMPid`, `IIMid`, `IIMname`, `IIMmaxbytes`, `EXIFid`, `etXMP`, `etIIM`, `etEXIF`, and the JSON
       key of the entry itself (e.g. `creatorNames`); for structure fields `etTag` instead of the
       `et*`, IIM and Exif keys. A missing key becomes `None`. Not copied: `label`, `helptext`,
       `usernotes`, `SCHEMAid`, `ugtopic`, `specidx`, `isrequired`;
     - strings as Rust literals (`\` and `"` escaped, non-ASCII kept as UTF-8); the file is UTF-8
       without BOM;
     - `IIMid` `"2:90"` becomes `IimDataset { record: 2, dataset: 90, max_bytes: Some(32) }` (from
       `IIMmaxbytes`);
     - `datatype` + `dataformat` become a `ValueKind` (point 2). **Any other combination, or an
       unknown `propoccurrence` (`single`, `multi`) or `ipmdschema` (`IptcCore`, `IptcExt`), stops
       the script** with a message naming the property and the value; nothing is written;
     - the same JSON gives byte-identical output every time.
     The output must already be in `rustfmt` layout (check: run the script, `cargo fmt --all`,
     `git diff` shows no change in `iptc_table.rs`). If that cannot be reached, put
     `#[rustfmt::skip]` on the `mod iptc_table;` line and say so in the report.
  2. **`properties.rs`** (hand-written; `pub mod properties` in `lib.rs`, `mod iptc_table` private):
     - `Schema { Core, Extension, Other }` (`Other`: point 3);
     - `ValueKind { Text, LangAlt, Date, Uri, Url, Integer, Number, Any, Struct(&'static str) }`,
       from the JSON: `string` without `dataformat` → `Text`; **`struct` + `AltLang` → `LangAlt`**
       (that is how the JSON marks Lang Alt, 21 times); `string` + `date-time` → `Date`;
       `string` + `uri` → `Uri`; `string` + `url` → `Url`; `number` + `integer` → `Integer`;
       `number` without `dataformat` → `Number`; `any` → `Any` (once: the `$anypmdproperty` field
       of `ImageRegion`); `struct` + any other `<name>` → `Struct("<name>")`, where `<name>` must be
       an `ipmd_struct` entry other than `AltLang`;
     - `Occurrence { Single, Multi }`; `ListKind { Seq, Bag }`;
     - `IimDataset { record: u8, dataset: u8, max_bytes: Option<u16> }`;
     - `PropertyDef { key, name, schema, kind, occurrence, xmp_id, iim, iim_name, exif_id, et_xmp,
       et_iim, et_exif }` with `&'static str` / `Option<&'static str>` fields (`exif_id` kept as the
       JSON text);
     - `StructDef { name, fields: &'static [FieldDef] }`,
       `FieldDef { key, name, kind, occurrence, xmp_id, et_tag }`;
     - lookups: `properties() -> impl Iterator<Item = &'static PropertyDef>` (TOP, then the extra
       entries of point 3), `property(xmp_id) -> Option<&'static PropertyDef>`,
       `by_iim(record, dataset) -> Option<&'static PropertyDef>`,
       `struct_def(name) -> Option<&'static StructDef>`.
  3. **Hand-written rules** in `properties.rs`, each with a comment naming its ADR section (the
     TechReference does not hold them):
     - `list_kind(xmp_id) -> Option<ListKind>`: `None` for a `Single` property; `Seq` for
       `dc:creator`, `plus:ImageCreator`, `plus:ImageSupplier`, `plus:CopyrightOwner`; `Bag` for
       every other `Multi` property. Check each `Multi` property against its "XMP Value Type" in
       `D:\PhotoWorkflowData\references\IPTC-PhotoMetadata-2025.1.md` (§7, §11). If the spec gives
       `Seq` for a property not named here, **stop and report**.
     - `max_entries(xmp_id) -> Option<usize>`: `plus:ImageCreator`, `plus:CopyrightOwner`,
       `plus:Licensor` 3; `plus:ImageSupplier` 1 (§8). Nothing else.
     - Extra entries with `Schema::Other` (§3 and §5.5): `xmp:Label` (Text, single, `etXMP`
       `XMP-xmp:Label`, no IIM); and the deprecated IIM properties `photoshop:Urgency` (2:10, 1 byte),
       `photoshop:Category` (2:15, 3 bytes) and `photoshop:SupplementalCategories` (2:20, 32 bytes
       each, `Multi`, Bag), with `etXMP` `XMP-photoshop:<name>` and `etIIM` `IPTC:Urgency`,
       `IPTC:Category`, `IPTC:SupplementalCategories`; `is_deprecated_iim(xmp_id) -> bool` true for
       those three.
  4. Module comments: what the table is, where it comes from, how to regenerate it (ADR 0007 §3).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. The JSON is **not** checked in.
- **Tests to add** (`#[cfg(test)] mod tests` in `properties.rs`; no ExifTool, no file):
  - `TOP` has 66 entries, 25 of them `Schema::Core`, and `STRUCTS` has 18 (the 19 `ipmd_struct`
    entries without `AltLang`), as in the saved JSON (P1);
  - `dc:description`, `dc:rights` and `dc:title` are `LangAlt`;
  - all 25 IPTC Core properties of the analysis §2 table
    (`docs/analyses/2026-11-04-iptc-metadata-requirements.md`) are found by `property()` with the
    table's XMP id, IIM dataset and byte limit (written out in the test; e.g. `photoshop:City`
    2:90/32, `dc:creator` 2:80/32 `Multi`, `dc:description` `LangAlt` 2:120/2000,
    `Iptc4xmpCore:IntellectualGenre` 2:04/64, `Iptc4xmpCore:SubjectCode` 2:12/236 `Multi`;
    Alt Text, Extended Description, Rights Usage Terms and Scene Code without IIM);
  - `photoshop:DateCreated` is `Date` with IIM 2:55, `et_iim` `IPTC:DateCreated+IPTC:TimeCreated`
    and `et_exif` naming `ExifIFD:DateTimeOriginal`;
  - `Iptc4xmpCore:CreatorContactInfo` is `Struct("CreatorContactInfo")`, and that structure has the
    eight fields `Iptc4xmpCore:CiAdrExtadr`, `CiAdrCity`, `CiAdrRegion`, `CiAdrPcode`, `CiAdrCtry`,
    `CiEmailWork`, `CiTelWork`, `CiUrlWork`; `Iptc4xmpExt:LocationCreated` is `Struct("Location")`,
    `Multi`;
  - every `Struct(name)` in `TOP` and in every structure's fields names an existing `StructDef`;
  - XMP ids are unique over `properties()`; IIM datasets are unique over `properties()`;
  - `xmp:Rating` (Image Rating) and `xmp:Label` are found;
  - `list_kind`: `dc:creator` Seq, `dc:subject` Bag, `plus:Licensor` Bag, `photoshop:City` `None`;
  - `max_entries` as point 3; `by_iim(2, 80)` is `dc:creator`, `by_iim(2, 10)` is
    `photoshop:Urgency`, `by_iim(2, 0)` is `None`.
- **Done when:** `.\tools\check.ps1` is all OK; running the script again on the same JSON leaves
  `iptc_table.rs` unchanged (`git diff`), and the report gives the JSON's SHA-256;
  `docs/test-inventory.md` is updated; `git diff --stat` shows only the files above.

### T77. ADR 0007 I2: dates and times — truncated dates, no invented time zone — Done

- **Can start now.** Independent of T76 and T79.
- **Crates:** `pw-metadata` only. Nothing uses the module yet.
- **Files:** new `crates/pw-metadata/src/date.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §7 (truncated dates, a zone is never invented, DateTimeOriginal is only read),
  §5.3 (IIM 2:55 and 2:60, Q3), §6.1 (Date Created from Exif).
- **What:** `pub mod date` with:
  1. **Types** (`Clone`, `Debug`, `PartialEq`, `Eq`):
     - `IptcDate { year: u16, month: Option<u8>, day: Option<u8>, time: Option<Time> }`; a day only
       with a month, a time only with a day;
     - `Time { hour: u8, minute: u8, second: Option<u8>, fraction: Option<String>, zone: Option<Zone> }`;
       `fraction` holds the digits after the decimal point exactly as read (no rounding);
     - `Zone { Utc, Offset(i16) }` (minutes east of UTC); `Utc` is a `Z` in the text and is written
       back as `Z`.
  2. **XMP form** (IPTC §14.2): `IptcDate::parse_xmp(&str) -> Result<IptcDate, DateError>` accepts
     `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, `YYYY-MM-DDThh:mm`, `YYYY-MM-DDThh:mm:ss`,
     `YYYY-MM-DDThh:mm:ss.s…`, each time form with or without `Z` or `±hh:mm`. Ranges are checked
     (month 1–12; the day exists in that month, leap years included; hour 0–23; minute and second
     0–59; offset hours 0–23 and minutes 0–59). Anything else is an error, never a panic.
     `to_xmp(&self) -> String` writes the same form back: a valid input gives exactly the same text.
  3. **IIM form** (§5.3, Q3):
     - `to_iim(&self) -> (String, Option<String>)`: 2:55 as `YYYYMMDD` with `00` for an unknown
       month or day (`19180000`); 2:60 as `hhmmss±hhmm` **only when there is a time and its zone is
       known** (`Utc` → `+0000`), otherwise `None`; the fraction is dropped; missing seconds are
       written as `00`;
     - `IptcDate::from_iim(date: &str, time: Option<&str>) -> Result<IptcDate, DateError>`: `00`
       gives `None` for month or day (a day without a month is an error); a time `hhmmss±hhmm` gives
       a zone, a time `hhmmss` without an offset is accepted **without a zone**; a time with a date
       that has no day is an error.
  4. **Exif** (§6.1, §7): `IptcDate::from_exif(date_time: &str, subsec: Option<&str>,
     offset: Option<&str>) -> Result<IptcDate, DateError>` for DateTimeOriginal
     `YYYY:MM:DD hh:mm:ss`, SubSecTimeOriginal (digits → `fraction`) and OffsetTimeOriginal
     (`±hh:mm` → zone). **Without an offset the result has no zone.** An empty value, only spaces,
     or the "unknown" forms with zeros or blanks (`0000:00:00 00:00:00`,
     `    :  :     :  :  `) give `DateError::Empty`. No function in this module writes Exif.
  5. **`DateError`** (`Empty`, `Invalid(String)` with the text) with `Display` (English) and
     `std::error::Error`.
  6. No time-zone conversion anywhere, no use of the computer's time zone, no new dependency.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `date.rs`):
  - each XMP form of point 2 parses and round-trips exactly, including `2026-11-04T10:15:00+01:00`,
    `2026-11-04T10:15:00Z`, `2026-11-04T10:15:00.123-05:30` and the zone-less
    `2026-11-04T10:15:00`;
  - invalid: `2026-13`, `2026-02-30`, `2023-02-29`, `2026-11-04T24:00`, `2026-11-04T10:60`,
    `2026-11-04T10:15+25:00`, `2026-11-04 10:15`, `26-11-04`, `""`; `2024-02-29` is valid;
  - IIM: `1918` → `("19180000", None)`; `2026-11` → `("20261100", None)`;
    `2026-11-04T10:15:30.5+01:00` → `("20261104", Some("101530+0100"))`;
    `2026-11-04T10:15` (no zone) → `("20261104", None)`; `…T10:15Z` → `Some("101500+0000")`;
  - `from_iim`: `19180000` → year only; `20261104` + `101530+0100` → time with zone;
    `20261104` + `101530` → time without zone; `20260004` and `20261100` + a time are errors;
  - Exif: `2026:11:04 10:15:30` + `123` + `+01:00` → `2026-11-04T10:15:30.123+01:00`; without
    offset → `2026-11-04T10:15:30` (no zone); the empty and unknown forms → `Empty`;
  - a zone is never added: a zone-less value stays zone-less through `to_xmp`, `to_iim` and back
    through `from_iim`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T78. ADR 0007 I3: the value model — values, origins, Lang Alt, changes — Done

- **Can start now.** T76 and T77 are accepted and committed (4e7df84); this task uses the property
  table and `IptcDate`. Independent of T79.
- **Crates:** `pw-metadata` only. Nothing uses the model yet; `LocalizedValue` and `Iptc` stay as
  they are (they move to the new model in I9).
- **Files:** new `crates/pw-metadata/src/value.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `crates/pw-metadata/src/properties.rs` (**only** the new function of point 8 and its
  test); `docs/test-inventory.md`.
- **Why:** ADR 0007 §3 (one model keyed by the XMP id, value types from the spec, each value
  carries its origin), §8 (Lang Alt: edit `x-default` only, Q7; clearing removes the whole
  property; cardinalities), §10 (clearing is always explicit).
- **What:** `pub mod value` with:
  1. **`Origin { XmpEmbedded, XmpSidecar, Iim, Exif, Edited }`** (`Copy`, `Eq`, `Hash`, `Debug`).
  2. **`LangAlt`**: the entries `(language, text)` **in file order** (a `Vec`, not a map).
     - `LangAlt::from_entries(Vec<(String, String)>)`: languages compared without regard to case;
       a repeated language keeps its first entry.
     - `set_default(&mut self, text)`: if `x-default` exists, only its text changes; otherwise
       `x-default` is inserted **first**. Every other entry stays as it is, **including one that held
       the same text as the old `x-default`** (Q7). A text that is empty or only spaces gives
       `ValueError::Empty` (clearing goes through `Metadata::clear`).
     - `display(&self) -> Option<(&str, Option<&str>)>`: the `x-default` text with `None`;
       without `x-default` the first entry with `Some(language)` (§8 reading rule); `None` when
       empty.
  3. **`Value`**: `Text(String)`, `LangAlt(LangAlt)`, `List(Vec<String>)` (Seq or Bag from
     `properties::list_kind`), `Date(date::IptcDate)`, `Number(String)` (the text exactly as read,
     so a rating keeps its full precision, §2.9, §8), `Struct(StructValue)`,
     `StructList(Vec<StructValue>)`; `StructValue { fields: Vec<(String, Value)> }` keyed by the
     field's XMP id, in order.
  4. **`PropertyValue { value: Value, origin: Origin }`**; **`Conflict { property: String,
     values: Vec<(Origin, Value)>, rule: ConflictRule }`** with `ConflictRule { Xmp, Iim, Exif,
     Open }` (§5.1: the rule that was applied; `Open` = the user decides). Data only; filling it is
     I5.
  5. **`Metadata`**: values keyed by XMP id (`BTreeMap<String, PropertyValue>`), the conflicts, and
     the set of changed property ids.
     - `get(xmp_id) -> Option<&PropertyValue>`.
     - `insert_read(xmp_id, value, origin) -> Result<(), ValueError>` (for the reader): the
       property must be in `properties::property` (`UnknownProperty`) and the value must fit its
       kind (point 6); **no cardinality limit** (§8: all are kept when reading); not marked changed.
     - `set(xmp_id, value) -> Result<(), ValueError>` (for edits): as `insert_read`, plus
       `max_entries` (`TooMany`), and a value that is empty or only spaces (text, list, Lang Alt or
       structure without content) gives `Empty`; origin `Edited`; marked changed. A Lang Alt edit
       goes through `set_default` on the existing value, so other languages are kept.
     - `clear(xmp_id) -> Result<(), ValueError>`: removes the value (all languages of a Lang Alt) and
       marks it changed; an unknown property is `UnknownProperty`.
     - `changes() -> impl Iterator<Item = (&str, Option<&Value>)>`: each changed property with its
       new value, or `None` when cleared.
  6. **Kind check:** `Text`, `Uri`, `Url`, `Any` → `Text`; `LangAlt` → `LangAlt`; `Date` → `Date`;
     `Integer` → `Number` whose text parses as `i64`; `Number` → `Number` whose text parses as
     `f64`; `Struct` single → `Struct`, multi → `StructList`; any `Multi` non-structure → `List`.
     Otherwise `ValueError::Kind`. **Inside a structure the same check applies to every field,
     at any depth:** each field's id must be a field of that structure (`properties::struct_def`),
     and its value must fit that field's kind and occurrence by the same rules (a `Multi` field is
     a `List` or `StructList`). The `property` of a `Kind` error inside a structure names the
     property and the field (e.g. `Iptc4xmpCore:CreatorContactInfo/Iptc4xmpCore:CiAdrCity`).
  7. **`ValueError`** (`UnknownProperty(String)`, `Kind { property, expected }`,
     `TooMany { property, max }`, `Empty(String)`) with `Display` (English) and
     `std::error::Error`.
  8. **Seq or Bag for structure fields** (T76 review: `list_kind` covers top-level properties
     only). In `properties.rs`, next to `list_kind`:
     `pub fn field_list_kind(struct_name: &str, field_xmp_id: &str) -> Option<ListKind>`: `None`
     for an unknown structure or field and for a `Single` field; `Seq` for
     `Iptc4xmpExt:AOCreator` and `Iptc4xmpExt:AOCreatorId` in `ArtworkOrObject` and
     `Iptc4xmpExt:rbVertices` in `RegionBoundary`; `Bag` for every other `Multi` field. The
     reviewer found these three as `Seq` in the "XMP Specs" rows of
     `D:\PhotoWorkflowData\references\IPTC-PhotoMetadata-2025.1.md`; check every `Multi` field of
     every structure there. If the spec gives `Seq` for a field not named here, **stop and
     report**. A comment names ADR 0007 §3 and the check date, as `list_kind` does.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `value.rs`):
  - Lang Alt: `[("x-default","A"),("sv","A"),("en","B")]` + `set_default("C")` gives
    `[("x-default","C"),("sv","A"),("en","B")]` (Q7); `[("sv","A")]` + `set_default("C")` gives
    `[("x-default","C"),("sv","A")]`; `display` without `x-default` gives `("A", Some("sv"))`;
    `X-Default` and `x-default` are the same language; a repeated language keeps the first;
  - `set` + `clear` of `dc:description` removes all languages and `changes()` gives `None` for it;
  - `set("plus:ImageCreator", …)` with 4 entries is `TooMany { max: 3 }`; `insert_read` with 4
    entries is accepted;
  - kind errors: `Text` for `dc:subject`, `List` for `photoshop:City`, `Number("abc")` for
    `xmp:Rating`, a structure field that does not belong to `CreatorContactInfo`;
  - fields inside structures: `Iptc4xmpCore:CreatorContactInfo` with `Iptc4xmpCore:CiAdrCity` as
    `Text` is accepted, with `CiAdrCity` as `List` is a `Kind` error naming
    `Iptc4xmpCore:CreatorContactInfo/Iptc4xmpCore:CiAdrCity`; `Iptc4xmpExt:LocationCreated` as a
    `StructList` of one `Location` with `Iptc4xmpExt:City` as `Text` is accepted; an error two
    levels down (in `Iptc4xmpExt:ImageRegion`, a `RegionBoundary` whose `rbVertices` is `Text`) is
    found;
  - `field_list_kind` (in `properties.rs`): `("ArtworkOrObject", "Iptc4xmpExt:AOCreator")`,
    `("ArtworkOrObject", "Iptc4xmpExt:AOCreatorId")` and
    `("RegionBoundary", "Iptc4xmpExt:rbVertices")` are `Seq`; a `Multi` field of another structure
    is `Bag`; `("Location", "Iptc4xmpExt:City")` (single), an unknown field and an unknown
    structure are `None`; every `Multi` field of every structure has `Some`, every `Single` field
    `None`;
  - `Number("2.5")` for `xmp:Rating` is kept as the text `2.5`;
  - `set` with `""`, `"   "`, an empty list and an empty Lang Alt is `Empty`;
  - an unknown property (`foo:Bar`) is `UnknownProperty` for `set`, `insert_read` and `clear`;
  - `insert_read` marks nothing changed; `set` gives origin `Edited`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T79. ADR 0007 I4: IIM text rules — blank values, byte limits, prefixes, character encodings — Done

- **Can start now.** Independent of T76 and T77 (the byte limit is a parameter here; I5 takes it
  from the table).
- **Crates:** `pw-metadata` only. Nothing uses the module yet.
- **Files:** new `crates/pw-metadata/src/iim.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 (blank values; equality on the predicted IIM value), §5.2 (cut at a UTF-8
  boundary, the user is told), §5.3 (prefixes of 2:04 and 2:12), §5.4 and Q10 (encodings and the
  fallback), §6.2 (the same order for Exif text).
- **What:** `pub mod iim` with:
  1. `is_blank(&str) -> bool`: true for an empty text and for a text of only spaces and/or NUL
     characters (MWG §4.2.3.3). Other whitespace (tabs, line feeds) is not blank.
  2. `truncate_utf8(text: &str, max_bytes: usize) -> (&str, bool)`: the longest prefix of at most
     `max_bytes` **bytes** that ends on a character boundary, and whether it was cut.
  3. `IimFormat { Plain, Genre, SubjectCode }` and
     `predict(text: &str, format: IimFormat, max_bytes: Option<u16>) -> Predicted { value: String,
     truncated: bool }`: the IIM value RawStride would write.
     - `Plain`: the text cut to `max_bytes`.
     - `Genre` (2:04, §5.3): `000:` + the code, where the 64-byte limit applies to the code only.
     - `SubjectCode` (2:12): `IPTC:` + the code, the limit counting the whole value. Check this
       against IPTC §7.23 in `D:\PhotoWorkflowData\references\IPTC-PhotoMetadata-2025.1.md`; if the
       spec says otherwise, **stop and report**.
  4. `strip_prefix(value: &str, format: IimFormat) -> &str` for reading: `Genre` removes a leading
     three digits and a colon (any number, not only `000`); `SubjectCode` removes a leading `IPTC:`;
     a value without the prefix is returned unchanged.
  5. `same_as_predicted(actual: &str, predicted: &Predicted) -> bool`: equal when the texts are
     equal, or when both are blank (§5.1: a value that only differs because of IIM truncation or
     encoding is not a conflict; the caller compares decoded text).
  6. **Decoding** (§5.4, §6.2):
     - `CodedCharset { Utf8, Latin1, Other(Vec<u8>), Absent }` and `coded_charset(value: Option<&[u8]>)
       -> CodedCharset` for dataset 1:90: `ESC % G` (`1B 25 47`) is `Utf8`; `ESC . A`
       (`1B 2E 41`) is `Latin1` (ISO 8859-1); any other bytes are `Other`.
     - `Fallback { Windows1252, Latin1 }` (default `Windows1252`, Q10).
     - `decode(bytes: &[u8], charset: &CodedCharset, fallback: Fallback) -> Decoded { text: String,
       converted: bool }`: a leading UTF-8 BOM is removed. `Utf8`: UTF-8, invalid sequences
       replaced with U+FFFD and `converted` set. `Latin1`: ISO 8859-1. `Absent` and `Other`: all
       bytes < 128 → ASCII; otherwise valid UTF-8 → UTF-8; otherwise the fallback with
       `converted` set. `Other` always sets `converted` (the declared encoding is not one of the two
       RawStride decodes; see the note below).
     - Windows-1252 by a table in the code for 0x80–0x9F; the five unassigned bytes (0x81, 0x8D,
       0x8F, 0x90, 0x9D) become the C1 control character of the same number, as Windows does; no
       new dependency.
     - `pub const UTF8_CODED_CHARSET: &[u8] = b"\x1b%G";` for writing (MWG: IIM is written as UTF-8
       with 1:90).
  7. Module comment with the ADR sections, and the note: other declared IIM encodings (e.g. ISO
     2022 sets) are decoded by the fallback order and marked converted; decoding them properly would
     need a new dependency and an owner decision.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `iim.rs`):
  - `is_blank`: `""`, `"   "`, `"\0\0"`, `" \0 "` true; `"\t"`, `" a "` false;
  - `truncate_utf8("Åsa", 1)` is `("", true)`; `("Åsa", 2)` is `("Å", true)`; a 33-byte `Plain`
    City with `max_bytes` 32 is cut to 32 bytes; a value of `ä` × 20 (40 bytes) cut to 32 gives
    `ä` × 16, never half a character;
  - `predict("Portrait", Genre, Some(64))` is `000:Portrait`; a 70-byte genre code is cut to 64
    bytes **before** the prefix; `predict("04000000", SubjectCode, Some(236))` is
    `IPTC:04000000`;
  - `strip_prefix("012:Feature", Genre)` is `Feature`; `strip_prefix("IPTC:04000000",
    SubjectCode)` is `04000000`; `strip_prefix("Feature", Genre)` is `Feature`;
  - `same_as_predicted`: the 32-byte cut value equals the prediction of the long text; `"  "`
    equals the prediction of `""`; a different word does not;
  - decoding: `coded_charset(Some(b"\x1b%G"))` is `Utf8`; with `Absent`, `b"abc"` is ASCII
    (not converted), `"Göteborg"` as UTF-8 bytes is UTF-8 (not converted), `b"G\xf6teborg"` is
    `Göteborg` via Windows-1252 (converted); `b"\x80"` is `€` with `Windows1252` and U+0080 with
    `Latin1`; `b"\x81"` is U+0081; a UTF-8 BOM is removed; `Other(..)` with ASCII bytes gives the
    text with `converted` set; invalid UTF-8 under `Utf8` gives U+FFFD and `converted`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T80. ADR 0007 I5a: the IIM block — datasets, decoding, values — Done

- **Can start now.** T76–T79 are done. Plain Rust; no ExifTool. Nothing uses the module yet.
- **Crates:** `pw-metadata` only.
- **Files:** new `crates/pw-metadata/src/iim_read.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 (blank values are non-existent), §5.3 (prefixes; 2:55 + 2:60 merged into
  one Date Created), §5.4 (1:90, decoding, the fallback marks the value as converted), §5.5 (the
  deprecated datasets are read like the others). The reader (T82) gets the raw IIM block from
  ExifTool (`-b -IPTC`, measured above) and needs it as values of the model (T78) for the
  reconciliation (T81).
- **What:** `pub mod iim_read` with:
  1. **`Dataset { record: u8, dataset: u8, data: Vec<u8> }`** and
     **`parse_block(bytes: &[u8]) -> Result<Vec<Dataset>, IimError>`**, in file order:
     - each dataset is the tag marker `0x1C`, the record number, the dataset number and a 2-byte
       big-endian length, then the data;
     - a length with the top bit set is an **extended length**: its low 15 bits give how many of the
       following bytes hold the real length (big-endian); 1–4 bytes are accepted, more is an error;
     - zero bytes after the last complete dataset are padding and are ignored (Photoshop pads its
       resources to an even length); any other byte where a tag marker is expected, and a length
       that runs past the end, are `IimError::Malformed { offset }` (the offset of the bad byte);
     - an empty block is `Ok(vec![])`. Never a panic and no unchecked indexing, whatever the bytes.
  2. **`IimError`** with `Display` (English, "IIM block is malformed at byte {offset}") and
     `std::error::Error`.
  3. **`iim_format(xmp_id: &str) -> IimFormat`**: `Genre` for
     `Iptc4xmpCore:IntellectualGenre`, `SubjectCode` for `Iptc4xmpCore:SubjectCode`, `Plain`
     otherwise (T79's formats; T83 uses the same function for writing).
  4. **`IimValues { values: Vec<(String, Value)>, charset: CodedCharset, notices: Vec<IimNotice> }`**
     and **`iim_values(datasets: &[Dataset], fallback: Fallback) -> IimValues`**:
     - `charset` is `iim::coded_charset` of dataset 1:90 (the first one; `Absent` without it). Every
       other record-1 dataset and 2:00 (record version) are ignored;
     - every record-2 dataset is decoded with `iim::decode` under that charset; a decoded value that
       is `is_blank` is **non-existent** (§5.1) and skipped;
     - the property is `properties::by_iim(2, dataset)`; a dataset without a property is ignored (it
       stays in the file, because RawStride never writes it);
     - the prefix is removed with `strip_prefix(text, iim_format(xmp_id))`;
     - the value follows the property's kind and occurrence (T78 point 6): a `Multi` property gives
       one `List` of all its datasets in file order; a `Single` property takes its **first**
       dataset, and a repeated one adds `IimNotice::Repeated { property }`; `Text`/`Uri`/`Url` give
       `Text`, `LangAlt` gives a Lang Alt with one `x-default` entry, `Integer`/`Number` give
       `Number` with the text as decoded (no trimming);
     - **Date Created:** 2:55 with the first 2:60 (if any) through `IptcDate::from_iim`. A 2:55 of
       only zeros (`00000000`) is **non-existent** (T77 review). An error, or a 2:60 without a 2:55,
       gives `IimNotice::Unreadable { property, text }` (the text as read, 2:55 and 2:60 joined with
       a space) and no value;
     - any other value that cannot be built (a kind that has no IIM form, a number that does not
       fit the kind) gives `Unreadable` and no value. Values are checked by the same rules as
       `Metadata::insert_read`; use it (or the same check) instead of a copy of the rules;
     - a value decoded with `converted` set gives `IimNotice::Converted { property }` **once per
       property**;
     - `values` is ordered by the position of each property's first dataset.
  5. **`IimNotice { Converted { property: String }, Repeated { property: String },
     Unreadable { property: String, text: String } }`** (`Clone`, `Debug`, `PartialEq`, `Eq`).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `iim_read.rs`; byte strings written out in the test):
  - **a block written by ExifTool 13.59** (base64
    `HAFaAAMbJUccAQAAAgAEHAIZAANldHQcAhkAA3R25RwCWgAIR/Z0ZWJvcmccAgAAAgAE`, decoded in the test
    without a new dependency, e.g. a small base64 helper in the test module): six datasets 1:90,
    1:00, 2:25, 2:25, 2:90, 2:00. The block declares UTF-8 but holds Latin-1 bytes (the arguments
    came through the Windows command line), so `dc:subject` is `["ett", "tv\u{FFFD}"]` and
    `photoshop:City` is `"G\u{FFFD}teborg"`, each with a `Converted` notice;
  - the same datasets without 1:90: `["ett", "två"]`, `"Göteborg"`, `Converted` for both
    (Windows-1252 fallback); with the `Latin1` fallback the same text;
  - extended length (a 2:120 of 40 000 bytes with a 2-byte extended length) is read; a 5-byte
    extended length, a length past the end, a missing tag marker, and 3 bytes of garbage are
    `Malformed` with the right offset; trailing zero bytes are ignored; random byte strings of
    every length 0–64 (a fixed seed, no new dependency) never panic;
  - blank values: a 2:90 of spaces, of NUL bytes, and an all-blank 2:25 list give no value;
    `00000000` in 2:55 gives no value and no notice;
  - prefixes: `2:04 = "000:Feature"` gives `Iptc4xmpCore:IntellectualGenre` `Feature`;
    `2:12 = "IPTC:04000000"` gives `Iptc4xmpCore:SubjectCode` `["04000000"]`;
  - kinds: 2:120 gives a Lang Alt `[("x-default", …)]` for `dc:description`; 2:80 twice gives
    `dc:creator` `List` of two; 2:90 twice gives the first and `Repeated`; 2:10 `"5"` gives
    `photoshop:Urgency` `Number("5")`; 2:10 `"x"` gives `Unreadable`;
  - Date Created: `20261104` + `101530+0100` gives the date with zone; `20261104` alone gives a date
    without time; `20261304` and a 2:60 without 2:55 give `Unreadable` with the text;
  - an unknown dataset (2:200) is ignored; 1:00 and 2:00 are ignored;
  - `iim_format` for the two prefixed properties and `photoshop:City`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T81. ADR 0007 I5b: reconciliation — IIM/XMP by checksum, Exif/XMP, Creator, `Cleared` — Done

- **Can start after T80 is accepted.** Plain Rust; no ExifTool. Nothing uses the module yet.
- **Crates:** `pw-metadata` only.
- **Files:** new `crates/pw-metadata/src/reconcile.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 (IIM/XMP precedence by checksum, equality on the predicted IIM value,
  every difference is a conflict with the rule applied), §6.2 (XMP has precedence over Exif; Exif
  only fills; Exif against IIM), §6.4 (manufacturer RAW: the file's metadata only fills, `Cleared`
  suppresses it), §6.5 (Creator ↔ Artist/Photographer), §7 (Date Created is only suggested from
  DateTimeOriginal), §4.2 (embedded camera XMP in a RAW only fills).
- **What:** `pub mod reconcile` with:
  1. **Input** (all `Clone`, `Debug`):
     - `FileKind { Jpeg, Tiff, Dng, Raw }` (`Raw` = manufacturer RAW with a sidecar, §4);
     - `IimDigest { Matches, Differs, Missing }` (`IPTCDigest` against `CurrentIPTCDigest`);
     - `IimRead { values: IimValues, digest: IimDigest }` (T80);
     - `ExifFields { image_description, copyright, artist, photographer, image_unique_id,
       date_time_original, sub_sec_time_original, offset_time_original }`, each
       `Option<String>`, already decoded (T82);
     - `Readings { kind: FileKind, xmp: Vec<(String, Value)>, camera_xmp: Vec<(String, Value)>,
       iim: Option<IimRead>, exif: ExifFields, cleared: BTreeSet<String> }`. `xmp` is the primary
       XMP: embedded for JPEG/TIFF/DNG, the sidecar for `Raw`. `camera_xmp` is the XMP inside a
       `Raw` file (empty otherwise). `cleared` is the sidecar's `Cleared` list (only for `Raw`;
       empty until D1).
  2. **Output:** `Reconciled { metadata: Metadata, notices: Vec<Notice>,
     suggested_date_created: Option<IptcDate> }` and `Notice { Iim(IimNotice), Refused { property,
     origin, error: ValueError }, HiddenByUser { property, value: Value, origin: Origin },
     Unreadable { property, origin, text } }`.
  3. **`reconcile(readings: Readings) -> Reconciled`**, in this order:
     1. **Primary XMP:** each value through `Metadata::insert_read` with origin `XmpEmbedded`
        (`XmpSidecar` for `Raw`); a refused value gives `Notice::Refused` and is left out. A blank
        text (T79 `is_blank`) counts as absent.
     2. **IIM** (§5.1), for each IIM property (T80 notices are passed on as `Notice::Iim`):
        - XMP lacks the property → the IIM value is inserted with origin `Iim`;
        - XMP has it and the two are **equal** (point 4) → nothing;
        - they differ → a `Conflict` with both values (XMP first) and: `Matches` → rule `Xmp`, XMP
          kept; `Differs` → rule `Iim`, the IIM value replaces the XMP value (origin `Iim`);
          `Missing` → rule `Open`, the XMP value stays in the model until the user decides (the
          panel shows both);
        - for `Raw` the IIM in the RAW file is camera data: it only fills, and a difference is a
          conflict with rule `Xmp` whatever the digest (§4.2, §6.4).
     3. **Camera XMP** (`Raw` only, §4.2): fills a property still absent (origin `XmpEmbedded`); a
        difference to the sidecar value is a conflict with rule `Xmp`.
     4. **Exif** (§6.1, §6.2, §6.5), for the both-ways fields Description ↔ ImageDescription,
        Copyright Notice ↔ Copyright, Creator ↔ Photographer/Artist:
        - the Exif value is Photographer if present, else Artist, for Creator; ImageDescription and
          Copyright as they are; blank counts as absent;
        - property absent → filled from Exif (origin `Exif`): Description and Copyright Notice as a
          Lang Alt with one `x-default` entry; Creator as `[photographer]`, or `split_artist(artist)`;
        - property present and **different** (point 4) → a conflict. If the current value came from
          XMP: rule `Xmp`, value kept. If it came from IIM (no XMP, §6.2): `Differs` → rule `Iim`,
          kept; `Matches` or `Missing` → rule `Exif`, the Exif value replaces it (origin `Exif`).
          For `Raw` always rule `Xmp`/kept (camera data);
        - **Digital Image GUID** (`Iptc4xmpExt:DigImageGUID`; ExifTool tag `XMP-iptcExt:DigitalImageGUID`) ← ImageUniqueID only fills; a
          difference is **not** a conflict (§6.1: the IPTC value is then not changed);
        - ImageEditor ↔ Contributor and GPS → Location Created are **not** part of this task (they
          get a task later; leave a comment).
     5. **`Cleared`** (`Raw`, §6.4): a property in `cleared` that the sidecar does not hold is never
        filled from the RAW file (IIM, camera XMP or Exif). The first value that would have filled
        it gives `Notice::HiddenByUser { property, value, origin }` (shown as "dold av användaren"
        later). A sidecar value for a cleared property wins (another tool wrote it).
     6. **Date Created** (§7): if `photoshop:DateCreated` is still absent, `suggested_date_created`
        is `IptcDate::from_exif` of the three Exif fields; `Empty` gives `None`, `Invalid` gives
        `Notice::Unreadable` (origin `Exif`). **Never inserted** into the metadata.
  4. **Equality** (`pub fn same_value(xmp_id, a: &Value, b: &Value) -> bool`, also used by T83):
     - against IIM (§5.1), on the predicted IIM value: `Text` and `Number` with `iim::predict`
       (the property's `max_bytes` and `iim_format`) and `same_as_predicted`; a Lang Alt by its
       shown text (`LangAlt::display`) **through the same prediction** (corrected after the first
       review: the XMP Lang Alt's shown text is predicted and compared with the IIM Lang Alt's
       shown text, so a Title cut to 64 bytes in 2:05 is not a conflict); a `List` item by item in
       order, same length; a `Date` by `to_iim()` (both 2:55 and 2:60 equal);
     - against Exif (§6.5): Description and Copyright by the shown text; Creator: the **first**
       creator against the Exif value's first name (RawStride writes only the first creator, so
       `["A","B"]` against Artist `A` is not a conflict);
     - text equality is exact (no trimming, no case folding).
  5. **`split_artist(text: &str) -> Vec<String>`** (MWG §5.7): names separated by `"; "`; a name in
     double quotes may contain `;` and doubled quotes `""` stand for one quote. Read the rule in
     `D:\PhotoWorkflowData\references\mwg_guidance.pdf` §5.7 first; if it differs from this
     description, **stop and report**. Blank names are dropped.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `reconcile.rs`; `Readings` built by hand):
  - **the three checksum cases** for `photoshop:City` (XMP `Göteborg`, IIM `Malmö`): `Matches` →
    `Göteborg`, conflict rule `Xmp`; `Differs` → `Malmö` origin `Iim`, rule `Iim`; `Missing` →
    `Göteborg`, rule `Open`; each conflict holds both values;
  - **equality on the prediction**: XMP City of 40 bytes against its IIM value cut to 32 bytes is
    not a conflict; IIM `000:Feature` (already stripped by T80) against XMP Genre `Feature` is
    not; a Description Lang Alt `[x-default "A", sv "B"]` against IIM `A` is not; keywords in
    another order are a conflict; a zone-less XMP date against IIM 2:55 only is not; XMP with a
    known zone against IIM without 2:60 is;
  - IIM only fills when XMP lacks the property (origin `Iim`, no conflict); a blank XMP text counts
    as absent;
  - **Exif**: Exif fills Description, Copyright and Creator when XMP and IIM lack them; Photographer
    wins over Artist; `split_artist("A; B")` → `["A","B"]`, `"\"Smith; J\"; B"` →
    `["Smith; J","B"]`, `"\"Say \"\"hi\"\"\"; B"` → `["Say \"hi\"","B"]`; XMP `["A","B"]` with
    Artist `A` is not a conflict, with Artist `C` it is (rule `Xmp`); IIM-only Creator against
    Artist with `Matches` → Exif wins (rule `Exif`), with `Differs` → IIM kept (rule `Iim`);
  - GUID fills from ImageUniqueID, and a different XMP GUID is kept without a conflict;
  - **RAW**: sidecar wins over camera XMP (conflict `Xmp`); camera XMP fills; RAW IIM with `Differs`
    only fills and gives rule `Xmp`; a cleared property is not filled from camera XMP, IIM or Exif
    and gives one `HiddenByUser` with the camera value; a sidecar value for a cleared property is
    used;
  - Date Created: absent → suggestion from Exif (zone only with an offset) and not in the metadata;
    present → no suggestion; an invalid DateTimeOriginal → `Unreadable`;
  - a refused XMP value (e.g. `Text` for `dc:subject`) gives `Refused` and the IIM value fills;
  - T80 notices arrive as `Notice::Iim`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T82. ADR 0007 I6: the reader — ExifTool output into readings, sidecar rules — Done

- **Can start after T81 is accepted.** D2 (`serde_json` = 1.0.154 in `pw-metadata`) was approved by
  the owner on 2026-11-11. `Cleared` (D1) is not read in this task: `Readings::cleared` stays
  empty.
- **Crates:** `pw-metadata` only. Nothing in RawStride uses the reader yet (the info panel is I10);
  the old `read_ratings` is unchanged.
- **Files:** new `crates/pw-metadata/src/read.rs`; `crates/pw-metadata/src/date.rs` (**only** the new
  function of point 4 and its tests); `crates/pw-metadata/src/lib.rs` (the `mod` line only);
  `crates/pw-metadata/Cargo.toml` (one line: `serde_json = "=1.0.154"` under `[dependencies]`);
  `Cargo.lock` (by cargo); new `crates/pw-metadata/tests/read.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §4 (where each format keeps its metadata), §4.1 and Q5 (a foreign sidecar next
  to a DNG is ignored and reported), §4.2 and Q8 (`<basename>.xmp`; `<basename>.<ext>.xmp` is
  ignored and reported; RAW+JPEG pairs), §5.4/§6.2 (decoding with the fallback), §6.5 (Copyright
  parts), §12 (round trip per format).
- **What:**
  1. **`ExifTool::read_metadata(&mut self, image: &Path, fallback: Fallback) ->
     Result<MetadataRead>`** with `MetadataRead { reconciled: Reconciled, file_notices:
     Vec<FileNotice> }` and `FileNotice { ForeignSidecarIgnored(PathBuf),
     ExtSidecarIgnored(PathBuf), IimBlockUnreadable(String) }`.
  2. **File kind** by extension, from the existing lists in `lib.rs` (do not change them):
     `supports_sidecar` → `Raw`; `dng` → `Dng`; `jpg`/`jpeg` → `Jpeg`; `tif`/`tiff` → `Tiff`; anything
     else is `Error::Unsupported`.
  3. **ExifTool calls** through `self.execute` (never a command line, see the measurements above):
     - the image file: `-j -n -b -struct -G1 -XMP:all -IPTC -Photoshop:IPTCDigest
       -CurrentIPTCDigest -IFD0:ImageDescription -IFD0:Artist -IFD0:Copyright
       -ExifIFD:Photographer -ExifIFD:ImageUniqueID -ExifIFD:DateTimeOriginal
       -ExifIFD:SubSecTimeOriginal -ExifIFD:OffsetTimeOriginal <file>`;
     - for `Raw`, also the sidecar `<basename>.xmp` (`sidecar_path`) if it exists, with
       `-j -n -b -struct -G1 -XMP:all <sidecar>`; its XMP is `Readings::xmp`, the RAW file's XMP is
       `camera_xmp`. Without a sidecar `xmp` is empty;
     - for `Dng`: a `<basename>.xmp` next to it is **never read** and gives
       `ForeignSidecarIgnored` (Q5);
     - for every kind: a `<basename>.<ext>.xmp` (e.g. `DSC01234.ARW.xmp`) gives `ExtSidecarIgnored`
       and is never read (Q8). Compare names without regard to case (Windows).
     If ExifTool reports the file missing or unreadable, return the error as `read_ratings` does.
  4. **XMP JSON → values** (with `serde_json::Value`): a key is mapped to a property by the table's
     `et_xmp` (`properties()`); unknown keys are ignored (they stay in the file).
     - Lang Alt: `<et_xmp>` is `x-default`, `<et_xmp>-<language>` is that language (the language may
       contain `-`, e.g. `en-US`); the entries are kept in the order of the JSON object (enable
       `serde_json`'s `preserve_order` feature **only if** it adds no new package to `Cargo.lock`;
       otherwise stop and report);
     - lists (JSON arrays) → `List`; objects → `Struct`, arrays of objects → `StructList`; structure
       fields by the field's `et_tag` (`FieldDef`), at any depth, Lang Alt fields as above;
     - JSON numbers → `Number` with the number's text; JSON strings → by the kind;
     - **dates:** add `IptcDate::parse_exiftool(text) -> Result<IptcDate, DateError>` in `date.rs`:
       ExifTool's form (`2026`, `2026:11`, `2026:11:04`, `2026:11:04 10:15`, `…:30`, `…:30.5`, each
       with or without `Z`/`±hh:mm`) is turned into the XMP form (the first two `:` of the date
       become `-`, the space becomes `T`) and parsed with `parse_xmp`. An unparsable date gives
       `Notice::Unreadable` through the reconciliation's notices (keep the text);
     - a value that cannot be built is passed on and refused by `reconcile` (`Notice::Refused`).
  5. **IIM:** the `IPTC` value (`base64:…`) is decoded (a small base64 decoder in `read.rs`, no new
     dependency), parsed with `parse_block` and turned into values with `iim_values`;
     `IimError` gives `FileNotice::IimBlockUnreadable` and no IIM. The digest: `Matches` when
     `IPTCDigest` equals `CurrentIPTCDigest`, `Differs` when both exist and differ, `Missing`
     without `IPTCDigest`. No `IPTC` key → `iim: None`.
  6. **Exif:** each value is a JSON string, or `base64:…` when ExifTool found bytes that are not
     UTF-8 (`-b`); those bytes are decoded with `iim::decode(bytes, &CodedCharset::Absent,
     fallback)` (§6.2: ASCII → UTF-8 → fallback). Copyright: parts separated by NUL are joined with
     a line feed (§6.5); trailing NULs and spaces are dropped. Blank values are `None`.
  7. `reconcile(readings)` gives `reconciled`.
- **Rules:** rules 3–7 and 10–12. The `Cargo.toml` line of D2 is the only dependency change. Check
  with `git diff Cargo.lock`: `serde_json` added to `pw-metadata`'s dependency list, **no new
  package**; otherwise stop and report.
- **Tests to add:**
  - `date.rs`: `parse_exiftool` for every form of point 4 gives the same `IptcDate` as `parse_xmp`
    of the XMP form; `2026:11:04T10:15` and `2026-11-04 10:15` are errors;
  - `read.rs` unit tests: the base64 decoder (RFC 4648 vectors, padding, invalid input is an error,
    never a panic); JSON → values for a hand-written JSON object with a Lang Alt in three languages
    (order kept, `en-US`), a list, a structure, a list of structures with a Lang Alt field, a date
    and a number; the digest cases; the Copyright NUL join; Exif `base64:` bytes in Windows-1252;
  - `tests/read.rs` (integration, **ExifTool required**; it fails with an explanation when ExifTool
    is missing, as `tests/sidecar.rs` does): a JPEG made from the `SMALL` bytes of
    `crates/pw-thumbcache/src/jpeg_fixture.rs` (copied into the test file), written with
    `ExifTool::execute` in a temporary folder:
    - XMP Description in two languages, two creators, Creator's Contact Info, Location Created,
      Rating `2.5`, Date Created `2026-11`, plus IIM City and Keywords with `-IPTCDigest=new`
      → read back exactly, digest `Matches`, no conflicts;
    - then IIM City changed without `-IPTCDigest=new` → `Differs`, City from IIM, conflict `Iim`;
    - IIM without a digest and a different XMP City → `Missing`, conflict `Open`;
    - Exif Artist and ImageDescription only → filled from Exif;
    - a fake RAW (`TN0001.ARW`, bytes as in `tests/sidecar.rs`) with `TN0001.xmp` → `XmpSidecar`
      values; with `TN0001.ARW.xmp` → `ExtSidecarIgnored`;
    - a JPEG renamed `.dng` is not a DNG, so a DNG test needs a real file: one test **ignored by
      default** reads the first DNG in `PW_TESTDATA` with a `<basename>.xmp` written next to a copy
      of it in a temporary folder → `ForeignSidecarIgnored`, and the sidecar's value is not used.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T83. ADR 0007 I7: the write plan — XMP, IIM, Exif overlap fields, conflicts — Done

- **Can start after T82 is accepted** (its tests read the result back with `read_metadata`).
- **Crates:** `pw-metadata` only. The plan is not used for real files yet: the safe write procedure
  (§9) is I8; ratings and templates move to it in I9.
- **Files:** new `crates/pw-metadata/src/write_plan.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); new `crates/pw-metadata/tests/write_plan.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 (IIM written when present or with the setting, UTF-8 with 1:90, the
  checksum, Q1, byte limits), §5.4 (existing IIM in another encoding is converted on the first
  write), §5.5 (deprecated datasets never edited), §6.3 (only existing "both ways" Exif fields are
  synced, never created; DateTimeOriginal never written), §6.5 (first creator to Artist and
  Photographer), §7, §8 (Lang Alt, clearing removes every representation; a RAW is never changed).
- **What:** `pub mod write_plan` with:
  1. **Input:** `FileState { kind: FileKind, iim_present: bool, iim_charset: CodedCharset,
     iim_values: Vec<(String, Value)>, digest: IimDigest, exif_present: ExifPresent }`, where
     `ExifPresent { image_description, copyright, artist, photographer }` are `bool`s (the tag
     exists in the file) and `iim_values` are the values T80 read from the file's IIM;
     `WriteSettings { create_iim: bool }` (§5.2, off by default); and the `Metadata` with its
     `changes()` and `conflicts()`. T82 gets a helper `FileState::from_read(&MetadataRead, …)` only if
     it is needed here; otherwise the tests build `FileState` by hand.
  2. **Output:** `plan(metadata: &Metadata, state: &FileState, settings: WriteSettings) ->
     Result<WritePlan, PlanError>` with `WritePlan { options: Vec<String>, args: Vec<String>, notices: Vec<WriteNotice> }`.
     `args` are ExifTool tag arguments only (`-TAG=value`); `options` holds only `-ec` when needed;
    no file name and no other options such as
     `-o` or `-P` (they belong to I8). `WriteNotice { Truncated { property }, IimConvertedToUtf8,
     ConflictKept { property }, DigestNotWritten }`. `PlanError { NotEditable(String) }` for a
     changed deprecated IIM property (§5.5).
  3. **XMP** for each changed property (`et_xmp`):
     - text, number, URI: `-<et_xmp>=<text>`; date: `to_xmp()` (ExifTool accepts the XMP form);
       line breaks are kept: `-stay_open` takes one argument per line, so a line break is written
       as `\n` (and `\` as `\\`) and the plan lists ExifTool's `-ec` option (C-style escapes) in
       `WritePlan::options`; prove in the integration test that a two-line Description comes back
       with its line break;
     - Lang Alt: only `x-default` (`-<et_xmp>=<text>`), the other languages are not touched (Q7);
     - list: the whole list (`-<et_xmp>=` followed by one `-<et_xmp>=<item>` per item);
     - structure or list of structures: ExifTool's structure syntax
       (`{Field=value,Field2=value}`, `[{…},{…}]`, fields by `et_tag`, Lang Alt fields with
       `-<lang>`), with ExifTool's escape character `|` before `,`, `}`, `]`, `|` and a leading
       `[`/`{` inside values;
     - cleared: the property is removed; for a Lang Alt **all** languages. Find the ExifTool
       arguments that remove every language and prove them in the integration test; if ExifTool
       cannot do it in one write, stop and report.
  4. **IIM** (JPEG, TIFF, DNG; never `Raw`):
     - written when `iim_present`, or when `settings.create_iim` (§5.2);
     - every changed property with an IIM dataset is written as `iim::predict` of its value with
       `iim_format` and the table's byte limit (lists item by item; Lang Alt by its `x-default`); a
       cut value gives `Truncated`. Date Created: 2:55 and, only when `to_iim()` gives one, 2:60;
       otherwise 2:60 is removed (Q3). Find the ExifTool value form for `IPTC:DateCreated` and
       `IPTC:TimeCreated` that stores exactly the `to_iim()` text and prove it in the test;
     - cleared → the dataset(s) are removed;
     - `-IPTC:CodedCharacterSet=UTF8` whenever IIM is written (MWG: UTF-8 with 1:90);
     - **existing IIM not declared UTF-8** (`iim_charset` other than `Utf8`) with any non-ASCII
       value in `iim_values`: every value of `iim_values` that is not changed is written again as
       UTF-8, once, with `IimConvertedToUtf8` (§5.4);
     - **checksum:** when both IIM and XMP will exist after the write, `-IPTCDigest=new`, **unless a
       conflict with rule `Open` remains** (Q1): then no digest is written and `DigestNotWritten`
       is given;
     - **Q1 conflicts:** a property with an `Open` conflict that the user did **not** change is not
       written in either form (`ConflictKept`). A property the user changed counts as decided: both
       forms are written and its conflict no longer blocks the digest. (This is the reviewer's
       reading of Q1 — "until the user decides" — **approved by the owner on 2026-11-11**.)
     - **bring both forms in line (added after the review, 2026-11-12):** §5.1 says a conflict
       marker "stays visible until the next write by RawStride brings both forms in line", and §12
       asks for "IIM = XMP after every write". So whenever IIM is written or kept (JPEG, TIFF, DNG
       with IIM after the write) and the plan is not empty, these properties that the user did
       **not** change are also written:
       - a conflict with rule `Iim` (differing checksum, the IIM value won): the stored value is
         written to **XMP** (as point 3); IIM already holds it;
       - a conflict with rule `Xmp` (matching checksum, IIM differs): the stored XMP value is
         written to **IIM** (as above, predicted and cut);
       - a stored value with origin `Iim` and no conflict (XMP lacked it): written to **XMP**.
       They no longer block the digest. Conflicts with rule `Open` stay untouched (Q1), deprecated
       IIM properties (§5.5) and conflicts with rule `Exif` are left out, and a `Raw` gets nothing of
       this. Without this the new digest would make XMP win at the next read and silently undo an
       IIM edit made by another program.
  5. **Exif** (JPEG, TIFF, DNG; never `Raw`), only for the fields that **exist** (`ExifPresent`),
     never created (§6.3): Description → `IFD0:ImageDescription` (shown `x-default`), Copyright Notice
     → `IFD0:Copyright`, Creator → the **first** creator to `IFD0:Artist` and
     `ExifIFD:Photographer`; cleared → the existing field is removed (MWG: the other form must be
     removed). DateTimeOriginal, SubSecTimeOriginal, OffsetTimeOriginal, ImageUniqueID and GPS are
     never in the plan, whatever changed.
  6. **`Raw`:** XMP only (the plan is applied to the sidecar by I8). A cleared property is just
     removed from the sidecar; recording it in `Cleared` waits for D1.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:**
  - unit tests in `write_plan.rs` with hand-built `Metadata`/`FileState`: the exact argument lists
    for a text, a Lang Alt, a list, a structure with characters that need `|`, a date with and
    without zone (2:60 written or removed), a clear of each kind; IIM not written without IIM and
    without the setting, written with the setting; `Truncated` for a 40-byte City; Genre and
    Subject Code prefixes; the UTF-8 conversion of unchanged Latin-1 values; the digest with and
    without an `Open` conflict, a changed property with an `Open` conflict written in both forms;
    Exif fields only when present; nothing for DateTimeOriginal; `NotEditable` for
    `photoshop:Category`; `Raw` gives XMP arguments only;
  - `tests/write_plan.rs` (integration, **ExifTool required**, same JPEG fixture as T82): apply the
    plan with `ExifTool::execute` to a **temporary copy** (with `-overwrite_original`, test only) and
    read it back with `read_metadata`: the changed values are there; other XMP languages are kept;
    a cleared Lang Alt has no language left; IIM equals XMP and `CurrentIPTCDigest` equals
    `IPTCDigest`; with an `Open` conflict the digest is unchanged; an existing Artist is updated, a
    missing one is not created; the IIM bytes are UTF-8 with 1:90 (`-b -IPTC`); a Date Created
    without zone has no 2:60.
  - **added after the review (2026-11-12), for "bring both forms in line":** unit tests: a
    `Differs` file with a City conflict of rule `Iim` and only Rating changed gives
    `-XMP-photoshop:City=<IIM value>` and `-IPTCDigest=new`; a City conflict of rule `Xmp` gives the
    IIM City from the XMP value; an IIM-only Headline (origin `Iim`) gives the XMP Headline; an
    `Open` conflict is still untouched with `DigestNotWritten`; a deprecated property and a `Raw`
    get none of this. Integration test: IIM City changed without updating the checksum, then only
    the Rating edited and the plan applied → read back, City is the IIM value with origin
    `XmpEmbedded`, no conflict, and `CurrentIPTCDigest` equals `IPTCDigest`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T84. ADR 0007 I8a: the file snapshot and the verification of a written copy — Done

- **Can start now.** No dependency change.
- **Crates:** `pw-metadata` only. Nothing uses it yet: T86 builds the safe write on it; the old
  `write_rating`/`write_iptc` stay unchanged until I9.
- **Files:** new `crates/pw-metadata/src/verify.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); new `crates/pw-metadata/tests/verify.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §9.1 points 1 and 3 (the file identified by content, its state recorded) and
  §9.3 points 1–3 (the copy checked: image data unchanged, metadata changed only where written,
  structure intact). Measured by the reviewer with ExifTool 13.59 (see the step plan).
- **What:** `pub mod verify` with:
  1. **`FileSnapshot { file_type: String, size: u64, modified: SystemTime, image_data_hash:
     Option<String>, validate: Option<Validate>, listing: BTreeMap<String, String> }`** and
     `Validate { errors: u32, warnings: u32, minor: u32 }`.
  2. **`ExifTool::snapshot(&mut self, file: &Path) -> Result<FileSnapshot>`:** `size` and
     `modified` from `std::fs::metadata`; everything else from **one** `self.execute` call:
     `-j -a -G1 -n -struct -api structformat=jsonq -api ImageHashType=SHA256 -all -ImageDataHash
     -validate -warning <file>` (as in T82, never a command line).
     - `file_type` is `File:FileType` (`JPEG`, `TIFF`, `DNG`, `XMP`; ExifTool decides by content);
     - `image_data_hash` is `File:ImageDataHash` (none for XMP);
     - `validate` from `ExifTool:Validate`, which `-n` prints as three numbers (`"0 7 1"`: errors,
       warnings, minor warnings);
     - `listing`: every other key with its value as text (a JSON string as it is, any other JSON
       value as its JSON text, so a structure is compared whole). The keys `SourceFile`,
       `ExifTool:Warning` (JSON keeps only one of several; the count is in `validate`) and
       `ExifTool:Validate` are left out.
     A missing or unreadable file is the error ExifTool reports, as in `read_metadata`.
  3. **`check_file_type(expected: FileKind, snapshot: &FileSnapshot) -> Result<(), VerifyError>`**
     (§9.1 point 1): `Jpeg` needs `JPEG`, `Tiff` `TIFF`, `Dng` `DNG`, and `Raw` (whose sidecar is
     written) `XMP`; anything else is `WrongFileType { expected, found }`. So a JPEG renamed `.dng`
     is refused.
  4. **`verify_copy(original: &FileSnapshot, copy: &FileSnapshot, plan: &WritePlan) -> Result<(),
     VerifyError>`** (§9.3):
     - `file_type` equal, otherwise `FileTypeChanged`;
     - `image_data_hash` equal, otherwise `ImageDataChanged`;
     - `validate` of the copy not higher than the original's in errors or warnings, otherwise
       `NewWarnings { before, after }` (the reviewer's Xiaomi DNG already has `0 7 1`, so "no
       warnings" cannot be the rule);
     - **every** key added, removed or changed in `listing` must be allowed, otherwise
       `Unexpected { key, before: Option<String>, after: Option<String> }` for the first one in key
       order. Allowed:
       - the groups `System`, `ExifTool` and `Composite` (file name, size and dates; tool data;
         values ExifTool derives, not stored);
       - `File:CurrentIPTCDigest`;
       - tags whose name ends in `Offset`, `Offsets` or `Start`, in any group (measured: a write
         moves `IFD0:TileOffsets` and `SubIFD:PreviewImageStart` of a DNG); a pointer that
         changes and has another name is reported to the owner, not added on your own;
       - `XMP-x:XMPToolkit`, `XMP-xmp:MetadataDate`, `XMP-xmp:ModifyDate`;
       - the tags of the plan's `args`: for each `-<Group>:<Tag>…=` the key `<Group>:<Tag>` and
         every key `<Group>:<Tag>-<language>`, where `<Tag>` is cut at the first `-` and a
         trailing `#` is dropped (`-XMP-dc:Description-x-default=` allows `XMP-dc:Description` and
         `XMP-dc:Description-sv`; `-IPTC:DateCreated#=` allows `IPTC:DateCreated`); `-IPTCDigest=`
         allows `Photoshop:IPTCDigest`;
       - when the plan writes IIM (`-IPTC:CodedCharacterSet=UTF8` is in it):
         `IPTC:EnvelopeRecordVersion` and `IPTC:ApplicationRecordVersion` (measured: ExifTool adds
         them when it creates IIM).
  5. `VerifyError` with `Display` texts in English that name the key or check (they are shown in
     T86's error), and `std::error::Error`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:**
  - unit tests in `verify.rs` with hand-built snapshots: an unchanged copy passes; each allowed
    kind of difference passes (a `System` key, `File:CurrentIPTCDigest`, `SubIFD:PreviewImageStart`,
    `XMP-x:XMPToolkit`, a Lang Alt in two languages for `-XMP-dc:Description-x-default=`, a
    structure, `IPTC:DateCreated` for `-IPTC:DateCreated#=`, the IIM record versions only with
    `-IPTC:CodedCharacterSet=UTF8`, `Photoshop:IPTCDigest` for `-IPTCDigest=new`); an added, a
    removed and a changed key outside the plan are `Unexpected` with both values;
    `XMP-photoshop:CityCode` is not allowed by `-XMP-photoshop:City=` (a whole tag, not a prefix);
    a different hash, a different file type, one more warning and one more error are errors, fewer
    warnings pass; `check_file_type` for every `FileKind`; the `Validate` text `"0 7 1"` and a
    missing `Validate`;
  - `tests/verify.rs` (integration, **ExifTool required**, the JPEG fixture of T82 copied in): a
    plan from `write_plan::plan` (City, a Description in two languages, keywords, IIM created with
    the setting) written with `ExifTool::execute` and `-o <name>.pwtmp-1.jpg` to a new file;
    `snapshot` of both; `verify_copy` passes, and the hash is equal. The same copy against a plan
    without the City arguments is `Unexpected` for `XMP-photoshop:City`. A JPEG renamed `.dng` is
    `WrongFileType`. One test **ignored by default**: every DNG in `PW_TESTDATA`, copied to a
    temporary folder, written with a City plan and `-o`, passes `verify_copy` (report the
    `Validate` counts and the allowed keys that changed for each file in the log row).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T85. ADR 0007 I8b: Windows file operations and the write journal — Done

- **Can start now.** D3 (`windows-sys` in `pw-metadata`) was approved by the owner on 2026-11-12.
  Can run at the same time as T84 (no shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/Cargo.toml` (only the D3 lines); `Cargo.lock` (by cargo); new
  `crates/pw-metadata/src/fileops.rs`; new `crates/pw-metadata/src/journal.rs`;
  `crates/pw-metadata/src/lib.rs` (the two `mod` lines only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §9.1 point 2 (writable and safe to write), §9.3 point 5 (flush), §9.4 (atomic
  replace, modification time) and §9.5 (errors, journal, recovery).
- **What:**
  1. **`Cargo.toml`:** under a new `[target.'cfg(windows)'.dependencies]`:
     `windows-sys = { version = "=0.61.2", features = ["Win32_Foundation",
     "Win32_Storage_FileSystem"] }`. Check with `git diff Cargo.lock`: `windows-sys` added to
     `pw-metadata`'s list, **no new package**; otherwise stop and report.
  2. **`fileops.rs`** (`pub(crate)`; the Windows calls behind `#[cfg(windows)]`; on other
     platforms each function returns `Error::Unsupported`):
     - `free_space(folder) -> Result<u64>` (`GetDiskFreeSpaceExW`, bytes available to the user);
     - `is_read_only_volume(path) -> Result<bool>` (`GetVolumePathNameW`, then
       `GetVolumeInformationW` and `FILE_READ_ONLY_VOLUME`);
     - `is_locked(path) -> Result<bool>`: opening the file for reading and writing with
       `std::os::windows::fs::OpenOptionsExt::share_mode(0)` fails with a sharing violation
       (`ERROR_SHARING_VIOLATION`, `ERROR_LOCK_VIOLATION`) → `true`; another error is an error;
     - `can_write(original, extra_bytes) -> Result<(), Refusal>` (§9.1 point 2) with `Refusal {
       ReadOnlyFile, ReadOnlyVolume, Locked, NoSpace { needed, free } }`: needed is the file size
       + 10 % (+ `extra_bytes`);
     - `set_modified(file, time)` and `flush(file)` with `std` (`File::set_modified`,
       `File::sync_all`, which calls `FlushFileBuffers`);
     - `replace(original, copy, backup) -> Result<(), ReplaceError>` with `ReplaceFileW` (no
       flags, `backup` as the backup name) and `ReplaceError { OriginalIntact(String),
       OriginalAtBackup(String) }`: `ERROR_UNABLE_TO_MOVE_REPLACEMENT` and every other failure where
       the original still exists → `OriginalIntact`; `ERROR_UNABLE_TO_MOVE_REPLACEMENT_2` →
       `OriginalAtBackup`. For `ERROR_INVALID_FUNCTION` and `ERROR_NOT_SUPPORTED` (a volume
       without `ReplaceFileW`, §9.4) it uses `replace_by_rename`;
     - `replace_by_rename(original, copy, backup)`: `MoveFileExW(original → backup,
       MOVEFILE_WRITE_THROUGH)`, then `MoveFileExW(copy → original, MOVEFILE_WRITE_THROUGH)`; if the
       second fails, the first is undone (`OriginalIntact`), or `OriginalAtBackup` if that fails
       too;
     - `move_new(copy, target)`: `MoveFileExW` with `MOVEFILE_WRITE_THROUGH` and **without**
       `MOVEFILE_REPLACE_EXISTING` (a new sidecar never overwrites a file that appeared meanwhile);
     - `temp_name(original, tag) -> PathBuf`: `<stem>.<tag>-<16 hex>.<ext>` in the same folder
       (`pwtmp`, `pwbak`), the hex from the time, the process id and a counter (no new
       dependency); a name that exists is never returned.
  3. **`journal.rs`:** `Journal::open(folder) -> Result<Journal>` (creates the folder); one file
     `<id>.json` per transaction, written and flushed **before** the copy is made (§9.5), with
     `Entry { original, copy, backup, original_existed: bool }` (`serde_json`); `begin(&Entry) ->
     Result<EntryId>`, `end(EntryId) -> Result<()>` (deletes the file); `recover(&self) ->
     Result<Vec<Recovered>>` for each open entry, by §9.5:
     - original present → the copy is deleted if present, the backup is deleted if present
       (it can only exist after a completed replace or a completed second rename);
     - original missing and backup present → the backup is renamed back, the copy deleted;
     - `original_existed == false` → the copy is deleted if present;
     - anything else (original and backup missing, the copy present) → nothing is deleted, the
       entry is kept and reported as `Recovered::NeedsUser { entry }`;
     `Recovered` says what was done per entry; an entry file that cannot be read is reported, not
     deleted.
- **Rules:** rules 3–7 and 10–12. The D3 lines are the only dependency change. `unsafe` only around
  the Windows calls, each with a `// SAFETY:` comment; paths are passed as NUL-terminated UTF-16.
- **Tests to add** (unit tests in the two files, on temporary folders; Windows only where they
  call Windows):
  - `free_space` of the temporary folder is above 0; `is_read_only_volume` is `false` for it;
    `is_locked` is `true` while the test holds the file with `share_mode(0)` and `false` after;
    `can_write` refuses a read-only file, a locked file, and `NoSpace` when `extra_bytes` is
    larger than the free space;
  - `replace`: the original's content is the copy's afterwards; its **creation time** (set to
    2020-01-01 with `FileTimesExt::set_created` first), its **hidden attribute** and an
    **alternate data stream** (`<file>:pw.test`, written with `std::fs`) are kept; the modification
    time set on the copy with `set_modified` is the result's; the backup holds the old content;
  - `replace` with the original held open with `share_mode(0)`: `OriginalIntact`, original
    content unchanged, copy still there;
  - `replace_by_rename` gives the same content result; with the copy missing it is
    `OriginalIntact` and the original is back under its name;
  - `move_new` refuses an existing target;
  - `temp_name` keeps the folder and extension and never returns an existing name;
  - the journal: `begin` writes a file and `end` removes it; `recover` for each case of point 3
    with files made by hand, including `NeedsUser` and an unreadable entry file.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T86. ADR 0007 I8c: the safe write transaction — Done

- **Can start after T84 and T85 are accepted.**
- **Crates:** `pw-metadata` only. The old `write_rating`/`write_iptc` stay unchanged (I9 moves them
  to this). The previews and LibRaw checks of §9.3 points 3–4 need `pw-raw` and LibRaw, which
  `pw-metadata` must not depend on: the transaction calls a check the caller gives
  (`ImageCheck`); the real one is written in I9.
- **Files:** new `crates/pw-metadata/src/safe_write.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); new `crates/pw-metadata/tests/safe_write.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §9 as a whole, §4.2 (a RAW is never changed; its sidecar gets the same
  transaction, §9.5).
- **What:**
  1. **`pub trait ImageCheck`** with `fn release(&self, image: &Path)` (close RAW engine sessions
     for the file, §9.1 point 2) and `fn check_copy(&self, original: &Path, copy: &Path, kind:
     FileKind) -> Result<(), String>` (§9.3 points 3–4); `NoImageCheck` does nothing (for tests and
     sidecars).
  2. **`ExifTool::write_safely(&mut self, image: &Path, plan: &WritePlan, checks: &dyn ImageCheck,
     journal: &Journal) -> Result<WriteReport, SafeWriteError>`**:
     - an empty plan writes nothing: `WriteReport::Unchanged`;
     - the target: the image for `Jpeg`/`Tiff`/`Dng`; for `Raw` the sidecar `<basename>.xmp`
       (`sidecar_path`); the RAW file itself is never written;
     - **9.1:** `checks.release(image)`; `can_write` (T85; for a new sidecar only the free space and
       the volume); `snapshot` and `check_file_type` (T84) of an existing target;
     - **journal** `begin` (T85) with the copy name `temp_name(target, "pwtmp")` and the backup name
       `temp_name(target, "pwbak")`;
     - **9.2:** one `execute` with `plan.options`, `plan.args`, `-o <copy>` and the target (an
       existing target is only read). A new sidecar is made **without a source file** (`plan`'s
       arguments and the copy name only): measured, `-o x.xmp` from a RAW copies the RAW's Exif into
       the sidecar. ExifTool's answer must report one file created and no warning (as
       `write_succeeded`), otherwise `WriteCopy`;
     - **9.3:** `snapshot` of the copy and `verify_copy` against the original's; for a new sidecar
       only `check_file_type` and that every key outside `System`, `ExifTool`, `File` comes from
       the plan; then `checks.check_copy` for `Jpeg`/`Tiff`/`Dng`; then `set_modified` (the
       original's time) and `flush`;
     - **9.4:** `replace` (T85), or `move_new` for a new sidecar; then the **final read**: size and
       `ImageDataHash` of the target equal the copy's; then the backup is deleted and the journal
       entry ended;
     - every failure leaves no `pwtmp` file and ends the journal entry when the original is intact;
       `OriginalAtBackup` keeps the entry and tries once to rename the backup back (§9.5).
  3. **`WriteReport { Written { target: PathBuf }, Unchanged }`** and **`SafeWriteError { stage:
     Stage, original: OriginalState, reason: String }`** with `Stage { Precheck, WriteCopy, Verify,
     ImageCheck, Replace, FinalCheck }` and `OriginalState { Intact, AtBackup(PathBuf), Replaced }`;
     `Display` in English naming the file, the stage and the reason (the Swedish texts are I10's).
     A failed final check keeps the backup and gives `Replaced`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`tests/safe_write.rs`, integration, **ExifTool required**; the JPEG fixture and
  the fake RAW of T82 in temporary folders; a temporary journal folder):
  - a JPEG with a plan (City, a two-line Description, keywords, IIM created): the values are read
    back with `read_metadata`, the modification time is the original's, the folder holds no
    `pwtmp`/`pwbak` file and the journal folder is empty;
  - an empty plan: `Unchanged`, the file's bytes unchanged;
  - a RAW without a sidecar: the sidecar is created with only the plan's values (no `XMP-exif`
    keys), the RAW's bytes unchanged; a RAW with a sidecar: the sidecar updated, its other values
    kept;
  - failures, each leaving the original **byte for byte** unchanged, no `pwtmp` file and an empty
    journal: a plan with an argument ExifTool refuses → `WriteCopy`; a plan with `-XMP:all=` on a
    file with XMP → `Verify`; an `ImageCheck` that returns an error → `ImageCheck`; an
    `ImageCheck` that opens the original with `share_mode(0)` and keeps it open → `Replace` with
    `Intact`; the original opened with `share_mode(0)` before the call → `Precheck`; a read-only
    original → `Precheck`;
  - one test **ignored by default**: every DNG in `PW_TESTDATA`, copied, written with a City and
    Description plan and `NoImageCheck`: `Written`, `ImageDataHash` unchanged, the values read
    back.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T87. ADR 0007 I9a: ratings and labels through the write plan and the safe write — Done

- **Can start now.** No dependency change. Can run at the same time as T89 (no shared files).
- **Crates:** `pw-metadata` only. Nothing in RawStride uses the new function yet (T91 wires it in).
  The old `write_rating`, `write_iptc` and `write_tags` stay **unchanged** (the Tauri app still
  uses them; owner decision D7).
- **Files:** `crates/pw-metadata/src/read.rs`; new `crates/pw-metadata/src/edit.rs`;
  `crates/pw-metadata/src/lib.rs` (the `mod` line only); new `crates/pw-metadata/tests/edit.rs`;
  `crates/pw-metadata/tests/read.rs` (new tests only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §15 (embedded writes of ratings and labels move from
  `-overwrite_original_in_place` to §9), §9.2, §8 (Rating: written only when the user changes it;
  a missing value counts as 0), §4.2 (a RAW is never written). The write plan (T83) needs a
  `FileState`, which no code builds from a real file yet (`FileState` is built by hand in the
  tests). Measured by the reviewer with ExifTool 13.59 (see the step plan).
- **What:**
  1. **`read.rs`:** `MetadataRead` gains `pub state: FileState` (`write_plan::FileState`), filled
     from what `read_metadata` already reads, before `reconcile` takes the readings:
     - `kind`: the file kind;
     - for `Jpeg`, `Tiff` and `Dng`: `iim_present` when the file has a readable IIM block (an
       unreadable block, `FileNotice::IimBlockUnreadable`, counts as none), `iim_charset` and
       `iim_values` from its `IimValues` (`charset`, `values`), `digest` from its `IimRead`
       (`Missing` without IIM); `exif_present` from `ExifFields` (`image_description`,
       `copyright`, `artist`, `photographer`: `is_some()`);
     - for `Raw`: `iim_present: false`, `iim_charset: CodedCharset::Absent`, no `iim_values`,
       `digest: Missing`, `exif_present` all `false` (the plan never writes the RAW file; the
       sidecar has no IIM and no Exif).
     Update the module comment. Nothing else in `read.rs` changes.
  2. **`edit.rs`** (`pub mod edit`):
     - `pub struct EditOptions { pub settings: WriteSettings, pub fallback: Fallback }`
       (`Default`: both defaults; the settings screen is I10);
     - `pub enum EditError { Unsupported(String), Read(Error), Value(ValueError), Plan(PlanError),
       Write(SafeWriteError) }` with `Display` in English (each variant's own text; `Write` is the
       `SafeWriteError` text) and `std::error::Error`;
     - **`ExifTool::write_rating_safely(&mut self, image: &Path, rating: &Rating, options:
       &EditOptions, checks: &dyn ImageCheck, journal: &Journal) -> Result<WriteReport,
       EditError>`:**
       - `supports_rating(image)` false → `Unsupported` (same text as `write_rating`); stars
         outside −1…5 → `Unsupported` (same text as `write_rating`); nothing is read or written;
       - `read_metadata(image, options.fallback)`; an error is `Read`;
       - **Rating** (`xmp:Rating`): `Some(n)` → `Metadata::set("xmp:Rating",
         Value::Number(n.to_string()))`; `None` → `Metadata::clear("xmp:Rating")`. **Not a
         change** (nothing is set or cleared): the read value is the same number
         (`reconcile::same_value`), or `None` while the file has no Rating or a Rating whose
         number is 0 (§8: a missing value counts as 0, so a `0` written by another program is
         not removed);
       - **Label** (`xmp:Label`): `Some(text)` → `set("xmp:Label", Value::Text(text))`, `None` →
         `clear`; not a change when the read value is the same text, or `None` while the file has
         no Label;
       - `write_plan::plan(&metadata, &read.state, options.settings)`; an error is `Plan`;
       - `write_safely(image, &plan, checks, journal)`; an error is `Write`. An empty plan gives
         `WriteReport::Unchanged` there, so a rating that is already in the file writes nothing
         and a RAW without a sidecar gets none for `Rating::default()`.
       Errors of `set`/`clear` are `Value`.
  3. Module comment in `edit.rs` explaining the above, with the reference to §8, §9, §15.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs`,
  `safe_write.rs`, `verify.rs`, `fileops.rs`, `journal.rs` or the old write functions; if one of
  them must change, stop and explain.
- **Tests to add:**
  - `tests/read.rs` (new tests, ExifTool required, files made as the existing tests make them):
    a JPEG with IIM (City, `IPTCDigest` written) → `state.iim_present`, `iim_charset` `Utf8`,
    the City in `iim_values`, `digest` `Matches`; a JPEG without IIM → `iim_present` false and
    `digest` `Missing`; a JPEG with `IFD0:ImageDescription` and `IFD0:Artist` → only those two
    `exif_present` flags; the fake RAW with a sidecar → `kind` `Raw`, no IIM, no Exif flags;
  - `tests/edit.rs` (integration, **ExifTool required**; the JPEG fixture and the fake RAW of T82
    in temporary folders; a temporary journal folder; `NoImageCheck`):
    - a JPEG: 3 stars and `Red` → `Written`; `read_ratings` gives 3 and `Red`; the modification
      time is the original's; no `pwtmp`/`pwbak` file; the journal folder is empty;
    - the same rating again → `Unchanged`, the file's bytes unchanged;
    - `Rating::default()` on that JPEG → `Written`, `read_ratings` gives nothing;
    - a JPEG whose `xmp:Rating` is `0` (written with `ExifTool::execute`) and
      `Rating::default()` → `Unchanged`, the `0` still in the file;
    - −1 (rejected) is written and read back as −1;
    - 6 stars → `Unsupported`, the file's bytes unchanged;
    - a JPEG with IIM (City) and a matching `IPTCDigest`: after a rating, `read_metadata` shows
      `digest` `Matches` and the City unchanged in IIM and XMP;
    - the fake RAW without a sidecar and `Rating::default()` → `Unchanged`, no sidecar; with 2
      stars → the sidecar is created holding Rating 2, the RAW's bytes unchanged;
    - a read-only JPEG → `EditError::Write` with `Stage::Precheck`, the file unchanged;
    - one test **ignored by default**: every DNG, JPEG and TIFF in `PW_TESTDATA`, copied to a
      temporary folder: 3 stars and `Red` → `Written`, `ImageDataHash` unchanged (`snapshot`),
      read back with `read_ratings`; report the time per file in the log row.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T88. ADR 0007 I9b: the ingest template with the modes Keep, Replace, Append, Prefix and Clear — Done

- **Can start after T87 is accepted.** The owner approved D5 (the separator is one space) and D6
  (`xmpRights:Marked` added to the table) on 2026-11-12.
- **Crates:** `pw-metadata` only. Nothing in RawStride uses it yet (T91). The old `Iptc`,
  `Iptc::tags` and `write_iptc` stay unchanged (D7).
- **Files:** new `crates/pw-metadata/src/template.rs`; `crates/pw-metadata/src/edit.rs`;
  `crates/pw-metadata/src/lib.rs` (the `mod` line and `Iptc::to_template` only);
  `crates/pw-metadata/src/properties.rs` (D6: one hand-written entry, as `xmp:Label`); new
  `crates/pw-metadata/tests/template.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §10 (the four modes, Prefix as a form of Append; an empty field never clears;
  no Clear for copyright-management fields, §8), §15 (today's template is "Keep for empty fields,
  Replace for filled ones", which stays compatible), §9.5 (ingest writes with the safe procedure).
- **What:**
  1. **`template.rs`** (`pub mod template`):
     - `pub enum Mode { Keep, Replace, Append, Prefix, Clear }`;
     - `pub struct TemplateField { pub property: String, pub field: Option<String>, pub mode: Mode,
       pub value: Option<Value> }`: `property` is an XMP id of the table; `field` the XMP id of a
       sub-field when `property` is a **single** structure (`ValueKind::Struct`, e.g.
       `Iptc4xmpCore:CreatorContactInfo` / `Iptc4xmpCore:CiEmailWork`);
     - `pub struct Template { pub fields: Vec<TemplateField> }`;
     - `pub enum TemplateError { UnknownProperty(String), CannotClear(String), ModeNotAllowed {
       property: String, mode: Mode }, Value(ValueError) }` with `Display` in English and
       `std::error::Error`;
     - **`pub fn apply(template: &Template, metadata: &mut Metadata) -> Result<(),
       TemplateError>`**, field by field in order, by §10:
       - `Keep` → nothing. `Replace`, `Append` or `Prefix` with no value or a value without
         content (empty text or only spaces, an empty list) → nothing (§10: an empty field never
         clears);
       - `Replace`: the new value with `Metadata::set` (a Lang Alt sets `x-default` and keeps the
         other languages, as `set` already does; a list is replaced by the template's list);
       - `Append` / `Prefix` on `Text` and on a Lang Alt (its shown text, `LangAlt::display`): the
         old text, one space (D5) and the new text (`Prefix`: new, separator, old); without
         an old text only the new text. On a `List` (`Append` only): the template's entries
         that are missing are added at the end, in order, without duplicates (exact text);
         `Prefix` on a list is `ModeNotAllowed`. On a `Date`, a `Number` or a whole structure
         both are `ModeNotAllowed`;
       - a sub-field (`field` is `Some`): the structure's current value (or an empty one), the
         mode applied to that field as above, then `set` of the whole structure; a sub-field of
         a **list** of structures (Location Created, Image Creator, Copyright Owner, Licensor) is
         `ModeNotAllowed` for every mode but `Keep` (later, with the template editor);
       - `Clear`: `CannotClear` for the copyright-management properties of §8 (`dc:rights`,
         `dc:creator`, `photoshop:Credit`, `xmpRights:UsageTerms`, `xmpRights:WebStatement`,
         `plus:CopyrightOwner`, `plus:Licensor`, `plus:ImageCreator`) and their sub-fields;
         otherwise `Metadata::clear`, or for a sub-field the field removed (the property cleared
         when no field is left). Nothing to clear (no value) → nothing;
       - a result equal to the current value (`reconcile::same_value`) is **not** set, so a
         template that is already in the file gives an empty plan;
       - the cardinality limits come from `set` (`TooMany`, as `Value`).
     The confirmation before a Clear (§10, once per template) is the user interface's (I10);
     variables (`{date}` …) and template files in XMP (§10) come later.
  2. **`edit.rs`:** `EditError` gains `Template(TemplateError)`; **`ExifTool::apply_template_safely(
     &mut self, image: &Path, template: &Template, options: &EditOptions, checks: &dyn
     ImageCheck, journal: &Journal) -> Result<WriteReport, EditError>`**: `supports_rating(image)`
     false → `Unsupported` (same text as `write_iptc`); `read_metadata`, `template::apply`,
     `plan`, `write_safely`, as in T87.
  3. **`Iptc::to_template(&self) -> Template`** (`lib.rs`): each field that `Iptc::tags` writes
     today, with `Replace` and its value, the others `Keep`: `creator` → `dc:creator` as a list
     of one name; `title`, `description`, `copyright` → `dc:title`, `dc:description`, `dc:rights`
     (`x-default`); `credit` → `photoshop:Credit`; `email`, `phone`, `url` →
     `Iptc4xmpCore:CreatorContactInfo` with the fields `Iptc4xmpCore:CiEmailWork`,
     `Iptc4xmpCore:CiTelWork`, `Iptc4xmpCore:CiUrlWork`; `xmpRights:Marked` `True` with `Replace` when `copyright` is given (D6: add `xmpRights:Marked`
     to `properties.rs` as a non-IPTC entry like `xmp:Label`, kind Text, no IIM, no Exif). Values
     are trimmed; **line breaks are kept** (the plan writes them with `-ec`; `tags` turned them
     into spaces).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs` or
  `safe_write.rs`.
- **Tests to add:**
  - unit tests in `template.rs` on hand-built `Metadata`: every mode for a `Text`, a Lang Alt (an
    `sv` entry kept by `Replace` and `Append`; `Clear` removes every language), a list (`Append`
    keeps the order and adds no duplicates) and a sub-field of `CreatorContactInfo` (the other
    fields kept; the last field cleared clears the property); `Keep` and an empty value change
    nothing (`changes()` empty); `CannotClear` for each of the eight properties of §8;
    `ModeNotAllowed` for `Append` on a `Date` and a `Number`, `Prefix` on a list, and a sub-field
    of `plus:ImageCreator`; `TooMany` for four Image Creators; a template equal to the file gives
    no change;
  - unit tests of `Iptc::to_template` (`lib.rs`): a full template, an empty one (every field
    `Keep`), a description with a line break kept;
  - `tests/template.rs` (integration, **ExifTool required**; the JPEG fixture and the fake RAW of
    T82; `NoImageCheck`; a temporary journal): `Iptc::to_template` on a JPEG → the values read
    back with `read_metadata` (Creator, Copyright Notice, Credit, the three contact fields); the
    same again → `Unchanged`; a JPEG with `IFD0:Copyright` → Exif Copyright updated too (§6.3);
    `Append` of a keyword to a JPEG with two keywords → three, in order; `Clear` of
    `photoshop:City` on a JPEG with IIM → gone from XMP and IIM; the fake RAW → only the sidecar
    written, the RAW's bytes unchanged.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T89. ADR 0007 I9c: the view core closes the RAW sessions of a file before it is written — Done

- **Can start now.** No dependency change. Can run at the same time as T87 (no shared files).
- **Crates:** `pw-viewcore` only.
- **Files:** `crates/pw-viewcore/src/core.rs`; `crates/pw-viewcore/src/state.rs`;
  `crates/pw-viewcore/src/lib.rs` (the `pub use` line only); `crates/pw-viewcore/tests/native_files.rs`
  (one new test); `docs/test-inventory.md`.
- **Why:** ADR 0007 §9.1 point 2 ("the file is not open in RawStride's own RAW engine (sessions
  for it are closed first)"). A LibRaw session keeps its file open for its whole life
  (`pw-libraw/csrc/pwlr.cpp`: the stream is closed in `pwlr_close`). Measured by the reviewer:
  while any other handle has the file open, `can_write` (T85) refuses it as `Locked`, and
  `ReplaceFileW` fails unless that handle allows deleting. So a rating on the DNG shown in RAW
  mode fails today as soon as T91 switches to the safe write.
- **What:**
  1. **`ViewCore::releaser(&self) -> Releaser<B>`**: a handle that can be cloned and sent to
     another thread (`Clone + Send + Sync`; it holds the core's shared state).
  2. **`Releaser::release(&self, path: &Path, timeout: Duration) -> bool`** (returns at once when
     the core has no session of `path`):
     - every **owned** session of `path` is dropped (and its bytes taken off the session budget,
       as an eviction does);
     - a **lent** session of `path` (a decode is using it) is marked to be dropped when it comes
       back, as `set_files` already does for lent sessions (`detached`);
     - it waits (on the core's condition variable, never by polling with sleep) until no session
       of `path` is lent any more, or `timeout` has passed; `true` when none is left;
     - the views (textures) of `path` are **kept**: the image data does not change when metadata
       is written. The next RAW render opens a new session as usual;
     - recorded as `"release"` (`record`), so the trace shows it;
     - paths are compared as stored in the file list (the same `PathBuf` the loupe gave).
  3. `pub use` of `Releaser` in `lib.rs`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. No change to the decoding or the
  budgets otherwise.
- **Tests to add:**
  - unit tests with the existing fake backend: an owned session of the path is dropped and the
    session bytes go down; the sessions of other paths are kept; a lent session is dropped when
    it comes back and `release` (on another thread) returns `true` only after that; with a lent
    session that never comes back within the time, `release` returns `false` after about the
    timeout; a path the core does not know returns `true` at once; the views of the path are still
    there after `release`;
  - `tests/native_files.rs`, one test **ignored by default** (`PW_TESTDATA`, `NativeBackend`,
    LibRaw): open a folder with one DNG in RAW mode until a session exists; while it exists,
    opening the DNG with `std::os::windows::fs::OpenOptionsExt::share_mode(0)` fails; after
    `release(path, 5 s)` returns `true`, the same open succeeds (Windows only).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T90. ADR 0007 I9d: the real image check — previews with `pw-raw`, the DNG opened with LibRaw — Done

- **Can start now.** The owner approved D4 (the new crate `pw-imagecheck`) on 2026-11-12. It needs
  only the `ImageCheck` trait of T86, so it can run at the same time as T87–T89.
- **Crates:** new `pw-imagecheck`.
- **Files:** `Cargo.toml` (root: the D4 line under `[workspace.dependencies]`); `Cargo.lock` (by
  cargo); new `crates/pw-imagecheck/Cargo.toml`; new `crates/pw-imagecheck/src/lib.rs`; new
  `crates/pw-imagecheck/src/tests.rs`; new `crates/pw-imagecheck/tests/real_files.rs`;
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §9.3 point 3 ("pw-raw finds the same previews (count, sizes, decodable JPEG)")
  and point 4 ("for DNG, LibRaw opens and unpacks the copy without a 'damaged' report"), §9.1
  point 2 (sessions closed first). `pw-metadata` must not depend on `pw-raw` or LibRaw, so T86
  takes the check from the caller; this crate has both. The rest of §9.3 point 3 (`DNGVersion`,
  `NewSubfileType`, the IFD chain) is already covered by T84's listing comparison, where any
  changed or missing key outside the allowed ones is an error. Measured by the reviewer: after a
  metadata write the embedded previews are **byte for byte equal**, only their offsets move.
- **What:**
  1. **`Cargo.toml` of the crate** (as D4): `pw-metadata`, `pw-raw`, `pw-libraw` (all
     `.workspace = true`), `[lints] workspace = true`, the workspace package fields as the other
     crates. Check with `git diff Cargo.lock`: only the `pw-imagecheck` package is added, **no
     other package**; otherwise stop and report.
  2. **`pub struct RawImageCheck`** with `RawImageCheck::new(release: impl Fn(&Path) + Send +
     Sync + 'static)` and `RawImageCheck::without_release()`; it implements
     `pw_metadata::safe_write::ImageCheck` and is `Send`:
     - `release(image)` calls the closure (the loupe passes T89's `Releaser`, T91);
     - **`check_copy(original, copy, kind)`:**
       - **previews** (every kind): `pw_raw::inspect` of both. When the original cannot be
         inspected (for example a BigTIFF), the preview check is skipped. Otherwise the copy
         must be inspected too, with the same number of previews, and each preview (by index)
         the same width, height and length and the **same bytes** (`pw_raw::read_preview`).
         Equal bytes decode exactly as the original's; this is the reviewer's reading of
         "decodable JPEG", and it never refuses a file whose own preview was already broken;
       - **`Dng`:** `pw_libraw::Session::open_with_diagnostics(copy)`; the session is dropped
         at once (the file is closed before the replace). When the copy does not open, the
         original is opened the same way: when it fails too, the check passes (the write did not
         make it worse); otherwise the error is the copy's LibRaw message;
       - an error is a text in English naming the check (`"preview 2 differs"`, `"the copy has 1
         preview, the original 2"`, `"LibRaw: damaged: …"`); the Swedish texts are T91's.
  3. Module comment with the reasons above (§9.3 points 3–4, why the bytes are compared).
- **Rules:** rules 3–7 and 10–12. The D4 lines are the only dependency change. The crate has no
  build script.
- **Tests to add:**
  - `src/tests.rs` (the JPEG fixture of `pw-thumbcache` via `#[path]`, as other crates do; files
    in a temporary folder): a file and an identical copy pass; a copy with one byte changed
    inside its embedded thumbnail (found with `pw_raw::inspect`) → an error naming the preview; a
    copy without the thumbnail → the count error; an original that cannot be inspected (a few
    bytes of text named `.tif`) → passes whatever the copy is; `release` calls the closure with
    the path; `without_release` does nothing;
  - `tests/real_files.rs`, **ignored by default** (`PW_TESTDATA`): every DNG copied and written
    with a City plan through `ExifTool::write_safely` with `RawImageCheck::without_release()` →
    `Written` (report the time of `check_copy` per file in the log row); every DNG with a copy cut
    inside its RAW data (as `pw-libraw/tests/damaged.rs` cuts) → `check_copy` fails with LibRaw's
    `damaged:` text; the cut copy checked against a cut original → passes; a DNG copy whose
    largest preview has one byte changed → the preview error.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T91. ADR 0007 I9e: RawStride writes ratings and the ingest template safely — Done

- **Can start after T87, T88, T89 and T90 are accepted.** D4, D5 and D6 were approved on 2026-11-12.
- **Crates:** `pw-metadata` (the worker), `pw-loupe`.
- **Files:** `crates/pw-metadata/src/worker.rs`; `crates/pw-metadata/src/lib.rs` (`WriteError` only);
  `crates/pw-loupe/Cargo.toml` (the D4 line only); `Cargo.lock` (by cargo);
  `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/import_worker.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §9 for every embedded write RawStride makes, §9.5 (the journal and recovery at
  start; ingest writes only to the verified copy, with this procedure), §15.
- **What:**
  1. **The worker** (`worker.rs`): `pub struct WriteSetup { pub journal: Journal, pub checks:
     Box<dyn ImageCheck + Send>, pub options: EditOptions }`; `MetadataWorker::start(setup)` and
     `start_with(program, pre_args, setup)` take it; a rating write runs
     `write_rating_safely(&path, &rating, &setup.options, &*setup.checks, &setup.journal)`.
     Reading (`read_ratings`) is unchanged. The existing calls of `start_with` in tests
     (`worker.rs`, `pw-loupe/src/metadata.rs`) get a setup with `NoImageCheck` and a temporary
     journal; nothing else in those tests changes.
  2. **`WriteError`** (`lib.rs`) gains `NotSafe { stage: Stage, original: OriginalState, reason:
     String }`; `From<EditError>`: `Unsupported`, `Read`, `Value`, `Plan`, `Template` → `Other`
     with the error text (`Read` through `From<Error>` as today, so `ReadOnly`/`NotWritten` keep
     their kind); `Write` with `Stage::Precheck` while the target exists and is read-only (as
     `refused_write_error` checks) → `ReadOnly`; any other `Write` → `NotSafe`. `Display` in
     English.
  3. **`pw-loupe` start** (`main.rs`): the journal folder is `%LOCALAPPDATA%\RawStride\journal`
     (`Journal::open`). **Before** the workers start, `Journal::recover` runs once; when it
     returns `NeedsUser`, `Failed` or `Unreadable` entries, a message box (as the exit message,
     T44) says "RawStride avslutades mitt i en metadataskrivning. Kontrollera de här filerna:" and
     one line per entry (`<original>` and, when there is one, `(säkerhetskopia: <backup>)`);
     `Cleaned` and `Restored` are not shown. When the folder cannot be opened (no
     `LOCALAPPDATA`, an error), RawStride still starts; every rating write and ingest template
     then fails with "skrivloggen kunde inte öppnas" and writes nothing.
  4. **The image check:** one `pw_imagecheck::RawImageCheck::new` whose closure calls
     `releaser.release(path, 5 s)` (T89's `Releaser` of the loupe's core) for the metadata worker,
     and one for the import worker.
  5. **Texts** (`metadata.rs`, `write_error_text`, after "Kunde inte spara betyget för <fil>: "):
     `NotSafe` with `Intact` → "metadata kunde inte skrivas säkert, filen är oförändrad";
     `AtBackup(backup)` → "metadata kunde inte skrivas säkert, originalet ligger nu som <filnamn
     på backup>"; `Replaced` → "filen skrevs men slutkontrollen misslyckades, den tidigare
     versionen finns kvar som säkerhetskopia". `ReadOnly`, `NotWritten` and `Other` as today.
  6. **Ingest** (`import_worker.rs`): `ImportWorker::start` takes a journal and a
     `Box<dyn ImageCheck + Send>`; `apply_iptc` uses `apply_template_safely(current,
     &iptc.to_template(), &EditOptions::default(), …)` instead of `write_iptc`; a failure is listed
     as today (`"<file>: <error>"`). The existing tests that start the import worker get
     `NoImageCheck` and a temporary journal.
  7. **`Cargo.toml`** of `pw-loupe`: `pw-imagecheck.workspace = true` (D4); `git diff Cargo.lock`
     shows only that line in `pw-loupe`'s list.
- **Rules:** rules 3–7 and 10–12. Nothing in `src-tauri` changes (D7). `write_rating` and
  `write_iptc` stay for the Tauri app.
- **Tests to add:**
  - `worker.rs`: a rating written through the worker with a JPEG and a temporary journal arrives
    as `RatingWritten` with `Ok`, and the journal folder is empty afterwards; a read-only JPEG
    gives `WriteError::ReadOnly`; an `ImageCheck` that fails gives `NotSafe` with `Intact`;
  - `lib.rs`: `From<EditError>` for each case of point 2;
  - `pw-loupe` `metadata.rs`: the three `NotSafe` texts (`write_error_text`);
  - `pw-loupe` `main.rs` (or where the start code is testable): the recovery message for a
    `NeedsUser` entry with and without a backup, and none for `Cleaned`/`Restored`;
  - `pw-loupe` `import_worker.rs`: an import of the JPEG fixture with a creator in the template
    writes it (read back with `read_metadata`) and leaves no `pwtmp` file.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: a rating on
  a DNG shown in RAW mode is saved and the image is still shown; the same on a JPEG and an ARW
  (sidecar).

### T92. ADR 0007 I9f: a current IIM checksum in TIFF and DNG — Done

- **Can start now.** The owner approved D8 (one more validation warning allowed when the safe
  write creates the Photoshop block of a TIFF or DNG) on 2026-11-12. It can run at the same time
  as T88 (no shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/safe_write.rs`; `crates/pw-metadata/src/verify.rs`;
  `crates/pw-metadata/tests/safe_write.rs`; `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 and §12 (the checksum is written and current after every write that has
  IIM and XMP), §9.3 (the copy is verified). Found in the review of T87: the ignored real-file
  test refuses `TN002683.tif` (a TIFF with IIM and a matching `IPTCDigest`) in `Verify` with one
  more warning, `IPTCDigest is not current`. Measured by the reviewer with ExifTool 13.59 (see the
  step plan): in a **TIFF or DNG**, `-IPTCDigest=new` in the same call as an IIM write stores the
  MD5 of the IIM block **without** the padding ExifTool adds to it in the TIFF structure, while
  `CurrentIPTCDigest` (what the reader compares, §5.1) includes it. A JPEG is not affected. A
  second call with only `-IPTCDigest=new` on the written copy stores the right checksum, with the
  `ImageDataHash` unchanged. And when the file had no Photoshop block (`Photoshop:IPTCDigest`
  missing), creating it adds the warning `Non-standard format (undef) for IFD0 0x8649
  PhotoshopSettings`, so T84's "no more warnings" refuses it (D8).
- **What:**
  1. **`safe_write.rs`, the digest pass:** after ExifTool has written the copy (9.2) and before
     its snapshot (9.3), when the target is a `Tiff` or `Dng` and the plan's `args` hold
     `-IPTCDigest=new`, a second `execute` on the copy only: `-IPTCDigest=new
     -overwrite_original <copy>` (the copy is RawStride's own temporary file; the original is still
     only read, §9.2). ExifTool's answer must report one file updated and no warning (as
     `write_succeeded`, and no `Warning` line), otherwise `Stage::WriteCopy`, the copy deleted as
     for any failure. Never for a JPEG or a sidecar. Update the module comment (9.2) with the
     reason.
  2. **`verify.rs`, D8:** in `verify_copy`, one more warning (not error) is allowed when the copy
     has the key `Photoshop:IPTCDigest`, the original has not, the `file_type` is `TIFF` or
     `DNG`, and the plan's `args` hold `-IPTCDigest=new`. Everything else of the check stays as it
     is. Document it at the check with the measured warning text.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs`.
- **Tests to add:**
  - unit tests in `verify.rs`: one more warning passes with a new `Photoshop:IPTCDigest` in a
    `TIFF` and a `DNG` copy with `-IPTCDigest=new` in the plan; it is still `NewWarnings` for a
    `JPEG`, without `-IPTCDigest=new`, when the original already had `Photoshop:IPTCDigest`, with
    two more warnings, and with one more error;
  - unit test in `safe_write.rs` for the choice of the digest pass (`Tiff`/`Dng` with
    `-IPTCDigest=new` yes; `Jpeg`, `Raw` and a plan without it no);
  - `tests/safe_write.rs` (ExifTool required): a minimal TIFF built in the test (a header and one
    IFD with a 1 × 1 strip, enough for ExifTool to write it), first given IIM (City) and a
    digest with `ExifTool::execute` in two calls so the digest is current; then
    `write_safely` with a plan from `write_plan::plan` that changes the City (read with
    `read_metadata`, `state` from T87) → `Written`; `IPTCDigest` equals `CurrentIPTCDigest`
    afterwards and `read_metadata` shows `digest` `Matches`; the same TIFF without IIM and a plan
    with the setting "create IIM" → `Written` with a current digest (D8);
  - one test **ignored by default**: every TIFF and DNG in `PW_TESTDATA`, copied, with a City plan
    and `WriteSettings { create_iim: true }` through `write_safely` → `Written` and a current
    digest. Also run T87's ignored test `every_real_dng_jpeg_and_tiff_is_rated_safely` and report
    that `TN002683.tif` passes.
- **Done when:** `.\tools\check.ps1` is all OK; both ignored runs pass; `docs/test-inventory.md`
  is updated; `git diff --stat` shows only the files above.

### T93. ADR 0007 I9g: the IIM checksum stays current when no XMP is left — Done

- **Can start now.** No dependency change. Can run at the same time as T91 (no shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/write_plan.rs`; `crates/pw-metadata/tests/write_plan.rs`
  (new tests only); `crates/pw-metadata/tests/template.rs` (one new test only);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 and §12 (the checksum is updated by every write that changes IIM). Found
  in T88: in a JPEG with IIM and a stored `IPTCDigest` whose only XMP value is City, a template
  `Clear` of City writes `-XMP-photoshop:City=` and `-IPTC:City=` but no `-IPTCDigest=new`
  (`plan` writes the checksum only when IIM **and XMP** exist after the write). The old checksum
  is then out of date, ExifTool's validation warns, and the copy check (T84) refuses the write
  (the original is untouched, but the Clear can never be saved). Measured by the reviewer with
  ExifTool 13.59 (see the step plan): with `-IPTCDigest=new` added the stored and the current
  checksum are equal and `Validate` is `OK`.
- **What:**
  1. **`write_plan.rs`, the checksum rule:** besides today's case (IIM and XMP both exist after
     the write), `-IPTCDigest=new` is written when the plan **writes IIM** and the file **has a
     stored checksum** (`state.digest` is `Matches` or `Differs`), even when no XMP is left. Q1 is
     unchanged: an unresolved (`Open`) conflict still gives `DigestNotWritten` (such conflicts
     only exist without a stored checksum, so the new case never meets one). A file without a
     stored checksum and without XMP after the write still gets none. Update the module comment
     (the checksum bullet) and the comment at the rule.
  2. Nothing else changes; for a TIFF or DNG the digest pass of T92 follows from the argument.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `safe_write.rs`,
  `verify.rs` or `template.rs`; do not change existing tests (the T88 test that gives the JPEG a
  keyword stays as it is).
- **Tests to add:**
  - unit tests in `write_plan.rs`: a JPEG with IIM, `digest` `Matches`, City the only value,
    City cleared → the plan holds `-XMP-photoshop:City=`, `-IPTC:City=` and `-IPTCDigest=new`;
    the same with `digest` `Differs` → also `-IPTCDigest=new`; with `digest` `Missing` and no XMP
    left → no `-IPTCDigest=new` and no `DigestNotWritten`; a TIFF with the same change → also
    `-IPTCDigest=new`; a plan that does not write IIM (only `xmp:Rating` in a file whose XMP
    is otherwise empty and whose IIM is in line) → unchanged from today;
  - `tests/write_plan.rs` (ExifTool required): the JPEG fixture given City in IIM and XMP and a
    checksum (`-IPTCDigest=new`), then the plan that clears City applied with `-o`: `IPTCDigest`
    equals `CurrentIPTCDigest` and `read_metadata` shows no City;
  - `tests/template.rs` (ExifTool required): the same JPEG (City the only XMP value, **no**
    keyword) and a template `Clear` of `photoshop:City` through `apply_template_safely` →
    `Written`, City gone from XMP and IIM, the checksum current.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T94. ADR 0007 I10a: the ExifTool version is checked when the metadata worker starts — Done

- **Can start now.** The owner approved D9 on 2026-11-12. Can run at the same time as T99 (no
  shared files).
- **Crates:** `pw-metadata` only. RawStride shows nothing yet (T101).
- **Files:** `crates/pw-metadata/src/exiftool.rs`; `crates/pw-metadata/src/worker.rs`;
  `crates/pw-metadata/src/lib.rs` (the `pub use exiftool::…` and `pub use worker::…` lines only);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §11 ("ExifTool version: at least 12.67, checked at start", from IPTC §5.1).
  Nothing checks the version today. The safe write (§9) also relies on newer ExifTool features
  (`-api ImageHashType`, `ImageDataHash`), so an old ExifTool must not write.
- **What:**
  1. **`exiftool.rs`:**
     - `pub struct ToolVersion { pub major: u32, pub minor: u32 }` (`Clone, Copy, Debug, PartialEq,
       Eq, PartialOrd, Ord`; the field order makes 12.67 < 13.10); `Display` as `major.minor` with
       the minor part in two digits (`12.67`, `13.05`);
     - `ToolVersion::parse(text: &str) -> Option<ToolVersion>`: the text trimmed, two runs of ASCII
       digits around one `.` (`13.59`, `12.67`); anything else is `None` (`13`, `13.5.1`, `v13.59`,
       an empty text);
     - `pub const MIN_EXIFTOOL_VERSION: ToolVersion` = 12.67;
     - **`ExifTool::version(&mut self) -> Result<ToolVersion>`**: `execute` with `-ver`; an answer
       that `parse` does not accept is `Error::ExifTool("unreadable ExifTool version: <text>")`.
  2. **`worker.rs`:**
     - the worker thread starts ExifTool **at once** when it starts (before the first request),
       and asks `version()` each time it starts a process (at the start and at every restart);
     - the result is kept in a slot shared with `MetadataWorker` (`Arc<OnceLock<…>>` or
       `Arc<Mutex<…>>`, no new dependency): **`MetadataWorker::exiftool_version(&self) ->
       Option<std::result::Result<ToolVersion, String>>`** is `None` until the first start has been
       tried, then the version, or the start or version error as text. The first result is kept;
       later restarts do not change it;
     - by D9: while the version of the running process is below `MIN_EXIFTOOL_VERSION` or could
       not be read, every rating write fails with `WriteError::Other(EXIFTOOL_TOO_OLD.into())` and
       nothing is written; `pub const EXIFTOOL_TOO_OLD: &str = "ExifTool is older than 12.67"`.
       Reading ratings works as today;
     - **no new `Event` variant** (RawStride matches every variant; T101 asks
       `exiftool_version`);
     - when the first start fails, the next request still retries the start, as today.
  3. `lib.rs`: export `ToolVersion`, `MIN_EXIFTOOL_VERSION` and `EXIFTOOL_TOO_OLD`.
  4. Module comments of both files updated (§11).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. If an existing test of the worker
  expects ExifTool to be started only by the first request (for example a test that counts the
  starts of a fake process), **stop and report**; do not change the test.
- **Tests to add:**
  - unit tests of `ToolVersion::parse` (the forms above, with spaces and a line feed around) and
    of the order (12.66 < 12.67 < 13.00; 12.99 < 13.00; `Display` of major 13, minor 5 is
    `13.05`);
  - `worker.rs` with the fake ExifTool of `test_support` (as the existing worker tests): a fake
    that answers `-ver` with `13.59` gives `Some(Ok(13.59))` without any request; a fake that
    answers `12.40` gives `Some(Ok(12.40))` and a rating write fails with `EXIFTOOL_TOO_OLD`
    while a read still answers; a fake that answers `abc` behaves as too old; a program that does
    not exist gives `Some(Err(…))` holding the start error;
  - with the real ExifTool (required, as the other worker tests): `exiftool_version()` is at least
    `MIN_EXIFTOOL_VERSION` within 5 s of the start.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T95. ADR 0007 I10b: the reader looks up the two sidecar names instead of listing the folder — Done

- **Can start now.** No dependency change. Can run at the same time as T94, T96, T97 and T99 (no
  shared files).
- **Crates:** `pw-metadata` only.
- **Files:** `crates/pw-metadata/src/read.rs` (`find_sidecars` and the module comment only);
  `crates/pw-metadata/tests/read.rs` (new tests only); `docs/test-inventory.md`.
- **Why:** found in T82: `find_sidecars` reads the whole folder (`read_dir`) for every image it
  reads, to find `<basename>.xmp` and `<basename>.<ext>.xmp` without regard to case (§4.1, §4.2,
  Q5, Q8). The info panel (T103) reads one image each time the user moves to the next, so in a
  folder of thousands of files every step lists them all (slow on a memory card or a network
  share).
- **What:**
  1. **`find_sidecars` on Windows:** the two names are looked up directly
     (`folder.join(name)`, `is_file()`); NTFS and FAT/exFAT compare names without regard to case,
     so `IMG_1.XMP` is found for `img_1.dng`. The path returned is the name **as it is in the
     folder** (the reader reports it to the user): `folder.join(<file name of
     std::fs::canonicalize(found)>)`, since Windows gives the stored case there; check it in the
     test below. No `read_dir`. A folder that cannot be read is the error it is today.
  2. **Other systems** (`#[cfg(not(windows))]`): the listing stays as it is (their file systems
     may compare case).
  3. The module comment says why.
- **Rules:** rules 3–7 and 10–12. No other change in `read.rs`; what `read_metadata` returns must
  be the same for every existing test.
- **Tests to add** (`tests/read.rs`, ExifTool required, files made as the existing tests make them):
  - a DNG with `IMG_1.XMP` (other case) next to it → `ForeignSidecarIgnored` with the name
    `IMG_1.XMP` as in the folder; a fake RAW `img_2.arw` with `IMG_2.xmp` → the sidecar is read;
    `x.arw.XMP` → `ExtSidecarIgnored`;
  - one test **ignored by default**: a temporary folder with 20 000 empty files and one JPEG;
    `read_metadata` of the JPEG ten times; report the time per read in the log row (before and
    after the change if you can).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T96. ADR 0007 I10c: validation warnings for edited metadata (§11) — Done

- **Can start now.** No dependency change. Can run at the same time as T95, T97 and T99; not at
  the same time as T94 (both change `lib.rs`).
- **Crates:** `pw-metadata` only. The editor shows the warnings (T104).
- **Files:** new `crates/pw-metadata/src/validate.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §11: warnings, never refusals. PLUS values (I11) and the controlled vocabularies
  other than Digital Source Type are not part of this task; dates are checked when the editor turns
  a text into a date (T104).
- **What:**
  1. **`validate.rs`** (`pub mod validate`):
     - `pub enum Warning` (`Clone, Debug, PartialEq, Eq`):
       - `TooLongForIim { property: String, max_bytes: u16 }`: a value of a property with an IIM
         dataset and a byte limit whose IIM form would be cut
         (`iim::predict(text, iim_read::iim_format(property), max_bytes).truncated`; for a Lang Alt
         its shown text, for a list each entry, the property once);
       - `AltTextTooLong { characters: usize }`: `Iptc4xmpCore:AltTextAccessibility` (shown text)
         over 250 characters (`chars().count()`);
       - `CountryCodeForm { code: String }`: `Iptc4xmpCore:CountryCode` that is not 2 or 3 ASCII
         letters (§11: ISO 3166-1; the list of codes itself is not checked);
       - `JobtitleWithoutCreator`: `photoshop:AuthorsPosition` without `dc:creator`;
       - `DigitalSourceTypeUnknown { uri: String }`: `Iptc4xmpExt:DigitalSourceType` that does
         not start with `http://cv.iptc.org/newscodes/digitalsourcetype/` followed by a code;
       - `RegistryEntryIncomplete { index: usize }`: an entry of `Iptc4xmpExt:RegistryId` without
         `Iptc4xmpExt:RegItemId` or `Iptc4xmpExt:RegOrgId` (a blank text counts as missing; the
         index from 0);
       - `RatingOutOfRange { value: String }`: `xmp:Rating` that is not a number between −1 and 5
         (a real number inside is fine, §8);
     - **`pub fn validate(metadata: &Metadata, iim_written: bool) -> Vec<Warning>`**: the warnings
       of every value in `metadata`, the properties in table order (`properties::properties()`);
       `TooLongForIim` only when `iim_written` (the file has IIM, or the setting creates it);
     - `Display` in English for `Warning` (the Swedish texts are T104's).
  2. Module comment with §11 and what is left out (PLUS, I11; the other vocabularies; dates).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `iim.rs`, `iim_read.rs`
  or `properties.rs`; if `iim_format` or `predict` cannot be used as they are, stop and explain.
- **Tests to add** (unit tests in `validate.rs` on hand-built `Metadata`): every warning once with
  its exact content; a 33-byte City with `iim_written` → `TooLongForIim` with 32, without it
  nothing; `ä` × 16 (32 bytes) in City → nothing, × 17 → the warning; a keyword list with one long
  entry → one warning; Alt Text of 250 characters nothing, 251 the warning; country codes `SE`,
  `SWE`, `se` nothing, `S`, `SVER`, `S1`, `Sverige` the warning; Jobtitle with and without Creator;
  a Digital Source Type `http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture` nothing,
  the bare prefix and `digitalCapture` the warning; two registry entries, the second without an
  organisation → index 1; ratings `-1`, `0`, `2.5`, `5` nothing, `6`, `-2`, `abc` the warning; an
  empty `Metadata` gives nothing; the order follows the table.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T97. ADR 0007 I10d: edits of single properties written safely, with the write notices — Done

- **Can start now.** No dependency change. Can run at the same time as T94, T95, T96 and T99 (no
  shared files).
- **Crates:** `pw-metadata` only. The worker gets it in T98, the editor in T104/T105.
- **Files:** `crates/pw-metadata/src/edit.rs`; `crates/pw-metadata/src/template.rs` (only to make
  its sub-field helper `pub(crate)` if you reuse it; no behaviour change); new
  `crates/pw-metadata/tests/edits.rs`; `crates/pw-metadata/src/verify.rs` (point 6 only, owner
  decision 2026-11-12); `docs/test-inventory.md`.
- **Why:** ADR 0007 §8 (clearing removes every representation; Lang Alt edits `x-default` only),
  §5.1 and Q1 (an `Open` conflict is decided by the user: an edit of that property writes both forms),
  §5.2 and §5.4 (the user is told when IIM was cut or converted: today the plan's
  `WriteNotice`s are dropped by `write_safely`), §9. The editor needs to set and clear any property
  of the table, which the template (§10: no Clear of copyright fields) does not allow.
- **What** (all in `edit.rs`):
  1. `pub enum Edit { Set { property: String, field: Option<String>, value: Value }, Clear {
     property: String, field: Option<String> } }`; `field` is the XMP id of a sub-field of a
     **single** structure (as `TemplateField::field`, T88).
  2. **`pub fn apply_edits(metadata: &mut Metadata, edits: &[Edit]) -> Result<(), EditError>`**,
     edit by edit in order:
     - `Set` without `field`: `Metadata::set`, **except** when the value equals the current value
       (`reconcile::same_value`) **and** the property has no conflict with the rule `Open` (then
       nothing is set). So choosing the value already shown in an `Open` conflict still decides it
       (both forms written, Q1);
     - `Set` with `field`: the structure's current value (or an empty one), the field set (a new
       field in the order of the structure's fields, as T88), then `set` of the whole structure,
       with the same rule;
     - `Clear` without `field`: `Metadata::clear` when the property has a value or an `Open`
       conflict; otherwise nothing. **Copyright fields may be cleared here** (§8 forbids it for
       templates only);
     - `Clear` with `field`: the field removed; the property cleared when no field is left;
       nothing when the field is not there;
     - a field of a **list** of structures, or a `field` on a property that is not a single
       structure → `EditError::Unsupported("<property>/<field> cannot be edited field by field")`;
     - errors of `set`/`clear` are `EditError::Value`.
  3. `pub struct EditReport { pub report: WriteReport, pub notices: Vec<WriteNotice> }`.
  4. **`ExifTool::write_edits_safely(&mut self, image: &Path, edits: &[Edit], options:
     &EditOptions, checks: &dyn ImageCheck, journal: &Journal) -> Result<EditReport, EditError>`**:
     `supports_rating(image)` false → `Unsupported` (same text as `write_iptc`); `read_metadata`
     with `options.fallback`; `apply_edits`; `write_plan::plan` with `options.settings`;
     `write_safely`; the report with the plan's `notices` (empty when `Unchanged`).
  5. Module comment extended (§5.1/Q1, §5.2, §8).
  `write_rating_safely` and `apply_template_safely` are unchanged.
  6. **The Exif block that ExifTool drops** (`verify.rs`; owner decision A, 2026-11-12, found in
     this task): in a JPEG whose only Exif is `IFD0:Copyright`, clearing it (`-IFD0:Copyright=`)
     makes ExifTool 13.59 drop the whole Exif block, so `File:ExifByteOrder` and
     `IFD0:YCbCrPositioning` (which ExifTool adds itself when it creates Exif) are removed too and
     `verify_copy` refuses the write as `Unexpected` (measured by the reviewer: with any other Exif
     tag left, such as `IFD0:Make`, `IFD0:XResolution` or `ExifIFD:ExifVersion`, the block and
     both keys stay). New private `fn drops_exif_block(original, copy, plan) -> bool`: the plan
     has an argument that clears an `IFD0:` or `ExifIFD:` tag (`-IFD0:<tag>=` or
     `-ExifIFD:<tag>=` with an empty value), the original has `File:ExifByteOrder`, and the copy
     has no `File:ExifByteOrder` and no key in the groups `IFD0`, `ExifIFD`, `IFD1`, `GPS` or
     `InteropIFD`. When it is true, the **removal** (a value before, none after) of exactly
     `File:ExifByteOrder` and `IFD0:YCbCrPositioning` is allowed in the key loop of
     `verify_copy`; a changed value of either key is still `Unexpected`. Comment with the
     measurement. Nothing else in `verify.rs` changes.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Do not change `write_plan.rs`,
  `safe_write.rs` or `value.rs`.
- **Tests to add:**
  - unit tests of `apply_edits` in `edit.rs` on hand-built `Metadata`: a text set; the same text →
    no change (`changes()` empty); the same text on a property with an `Open` conflict → changed;
    a Lang Alt with an `sv` entry keeps it; a clear of `dc:rights` (allowed here); a clear of an
    absent property → nothing; a contact field set (other fields kept), cleared, the last field
    cleared clears the property; a field of `plus:ImageCreator` and a `field` on `photoshop:City`
    → `Unsupported`; an unknown property → `Value`;
  - `tests/edits.rs` (integration, **ExifTool required**; the JPEG fixture and the fake RAW of
    T82 in temporary folders; `NoImageCheck`; a temporary journal):
    - Headline and City set on a JPEG → `Written`, read back with `read_metadata`; the same again
      → `Unchanged`;
    - a JPEG with IIM (City) and a matching digest, a 40-byte City → `Written` with
      `WriteNotice::Truncated` for `photoshop:City`, XMP holding all 40 bytes;
    - a JPEG with City in IIM and another City in XMP and **no** checksum (an `Open` conflict),
      the XMP value set again → `Written`, both forms equal afterwards and no conflict when read;
    - `dc:rights` cleared on a JPEG with `IFD0:Copyright` → gone from XMP and Exif (§8);
    - the fake RAW: a Title set → only the sidecar written, the RAW's bytes unchanged.
  - `verify.rs` unit tests (point 6, with the existing helpers): the two keys removed with
    `-IFD0:Copyright=` in the plan and no Exif key left → passes; the same with `IFD0:Make` still
    in the copy → `Unexpected` for `File:ExifByteOrder`; the same without a clearing argument (a
    plan that sets `-IFD0:Copyright=x`) → `Unexpected`; `File:ExifByteOrder` changed from `MM` to
    `II` with the clearing argument → `Unexpected`.
  - `tests/edits.rs` `clearing_the_copyright_notice_removes_it_from_xmp_and_exif` (the JPEG whose
    only Exif is Copyright) passes unchanged.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T98. ADR 0007 I10e: the metadata worker reads all metadata, writes edits and takes new settings — Open

- **Can start after T94 and T97 are accepted** (both change what this task builds on;
  `worker.rs` is T94's). No owner decision of its own.
- **Crates:** `pw-metadata`, and in `pw-loupe` only the arms that ignore the new events.
- **Files:** `crates/pw-metadata/src/worker.rs`; `crates/pw-loupe/src/metadata.rs` (the match in
  `MetadataCache::apply` only: the two new events are ignored until T103/T105);
  `docs/test-inventory.md`.
- **Why:** the info panel (T103) needs the whole `MetadataRead` of the image shown, and the
  editor (T105) writes through the worker, which owns the ExifTool process and the write setup
  (G2: the UI only polls). The settings screen (T106) changes the `EditOptions` while RawStride
  runs.
- **What** (`worker.rs`):
  1. `MetadataWorker::read_metadata(&mut self, path: PathBuf) -> Result<RequestId, SubmitError>` →
     `Event::MetadataRead { id, path, result: std::result::Result<MetadataRead, String> }`,
     read with the current options' `fallback` (`Fallback::default()` without a setup).
  2. `MetadataWorker::write_edits(&mut self, path: PathBuf, edits: Vec<Edit>) ->
     Result<RequestId, SubmitError>` → `Event::EditsWritten { id, path, result:
     std::result::Result<EditReport, WriteError> }` through `write_edits_safely` with the setup;
     without a setup `WriteError::Other(NO_JOURNAL)`, with a too old ExifTool
     `EXIFTOOL_TOO_OLD` (T94); errors through `From<EditError>` as for ratings.
  3. `MetadataWorker::set_options(&mut self, options: EditOptions) -> Result<(), SubmitError>`:
     a request without an event; every request after it uses the new options (rating writes,
     edits and reads). Requests run in order, so a write queued before it uses the old options.
  4. Module comment updated.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. The existing requests and events are
  unchanged.
- **Tests to add** (`worker.rs`; ExifTool required where a real file is written, Windows as the
  existing write tests):
  - `read_metadata` of a JPEG with a Headline → `MetadataRead` holding it; of a missing file →
    `Err`;
  - `write_edits` of a Headline → `EditsWritten` with `Ok` and `Written`, read back with
    `read_metadata`; without a setup → `NO_JOURNAL`, the file unchanged;
  - `set_options` with `create_iim: true`, then `write_edits` of a City on a JPEG without IIM →
    the City is in IIM afterwards; the same without `set_options` → no IIM;
  - `set_options` with `Fallback::Latin1` changes how a JPEG with IIM 0x80 in City and no 1:90 is
    read (`€` with Windows-1252, U+0080 with Latin-1).
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T99. ADR 0007 I10f: RawStride's settings file (create IIM, fallback encoding) — Done

- **Can start now.** The owner approved D10 on 2026-11-12. Can run at the same time as T94 (other
  crate). T100 and T102 also add lines to `lib.rs`; if
  they run at the same time, each agent adds only its own `mod` line and its own `pub use` line.
- **Crates:** `pw-loupe` only. Nothing uses the file yet (T106).
- **Files:** new `crates/pw-loupe/src/settings.rs`; `crates/pw-loupe/src/lib.rs` (one `mod` line,
  one `pub use` line); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 (the setting "Skapa IPTC-IIM för kompatibilitet med äldre system", off by
  default) and §5.4/Q10 (the fallback encoding, Windows-1252 by default, changeable). RawStride
  has no settings file yet.
- **What** (`settings.rs`), as D10:
  1. `pub struct Settings { pub create_iim: bool, pub fallback: Fallback }`; `Default`: `false`
     and `Fallback::Windows1252`; `Settings::edit_options(&self) -> EditOptions`.
  2. `pub fn settings_path() -> Option<PathBuf>`: `%LOCALAPPDATA%\RawStride\settings.txt` (`None`
     without `LOCALAPPDATA`).
  3. `pub fn load_settings(path: &Path) -> (Settings, Vec<String>)`: a missing file gives the
     defaults and no problem. The file is UTF-8 text, one `key = value` per line; spaces around
     both are ignored; empty lines and lines starting with `#` are skipped. Keys:
     `create_iim` (`true`/`false`) and `fallback_encoding` (`windows-1252`/`iso-8859-1`). An
     unknown key, a bad value or a line without `=` keeps the default for it and gives one problem
     text in English naming the line number (for the log, `eprintln!` by the caller); a file that
     cannot be read gives the defaults and one problem.
  4. `pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()>`: creates the
     folder, writes both keys (with a first comment line `# RawStride`) to `settings.txt.tmp` in
     the same folder, then `std::fs::rename` onto `settings.txt` (replaces it on Windows), so a
     crash never leaves half a file.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change (no serde in `pw-loupe`).
- **Tests to add** (unit tests in `settings.rs`, temporary folders): defaults for a missing file;
  save then load gives the same settings for all four combinations; a file with comments, empty
  lines and spaces; an unknown key, a bad value and a line without `=` each give one problem with
  its line number and the defaults for that key while the other key is read; `save` into a folder
  that does not exist creates it; no `.tmp` file is left after `save`; `edit_options`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T100. ADR 0007 I10g: the import dialog chooses the mode of each template field, and asks before a Clear — Ready for review

- **Can start now.** The owner approved D13 and D14 on 2026-11-12. It needs nothing new in `pw-metadata` (T88's `Template`
  exists). It can run at the same time as T94–T99 and T102 (only `lib.rs` lines shared, see T99);
  not at the same time as T101, T103, T105 or T106 (all change `main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/import.rs`; `crates/pw-loupe/src/import_worker.rs`;
  `crates/pw-loupe/src/main.rs` (the IPTC part of the import dialog and the start of the import
  only); `crates/pw-loupe/src/lib.rs` (the `pub use import::…` line only);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §10 (each template field has a mode; an empty field never clears; no Clear for
  copyright-management fields; "Clearing any other field asks for confirmation once per
  template"). Left over from T88 and T91: today the import dialog's six fields always use
  `Iptc::to_template` (Replace for a filled field, Keep for an empty one).
- **What** (as D13 and D14):
  1. **`import.rs`:**
     - `pub enum IptcField { Creator, Copyright, Credit, Email, Phone, Url }` with
       `IptcField::ALL` in this order and `label()` giving the dialog's texts ("Fotograf",
       "Copyright", "Credit", "E-post", "Telefon", "Webb");
     - `IptcForm` gains `pub modes: IptcModes`, one `pw_metadata::template::Mode` per field,
       default `Replace` for all six (D13; with an empty text that does nothing, so a form nobody
       touches behaves exactly as today);
     - `pub fn allowed_modes(field: IptcField) -> &'static [Mode]`: Creator `Keep, Replace,
       Append`; Copyright and Credit `Keep, Replace, Append, Prefix`; Email, Phone and Url all
       five (§8/§10: no Clear of Creator, Copyright Notice or Credit Line; no Prefix on a list);
     - `pub fn mode_text(mode: Mode) -> &'static str`: the Swedish names of D13 ("Behåll",
       "Ersätt", "Lägg till efter", "Lägg till före", "Rensa");
     - **`IptcForm::to_template(&self) -> Template`**: per field its mode and its trimmed text
       (`None` when empty) on the same properties as `Iptc::to_template` (Creator a list of one
       name on `dc:creator`; Copyright `dc:rights` `x-default`; Credit `photoshop:Credit`; the
       three contact fields as fields of `Iptc4xmpCore:CreatorContactInfo`); `Clear` without a
       value; `xmpRights:Marked` `True` with `Replace` when Copyright has a text and the mode
       `Replace`, `Append` or `Prefix` (D6);
     - `IptcForm::template_is_empty(&self) -> bool`: true when no field would do anything (each
       is `Keep`, or `Replace`/`Append`/`Prefix` with an empty text);
     - `IptcForm::cleared_fields(&self) -> Vec<IptcField>`: the fields in `Clear`, in order;
     - `pub fn clear_confirmation_text(fields: &[IptcField]) -> String` (D14): "Mallen tömmer de
       här fälten i alla bilder som importeras:" and one line per field with its label;
     - `to_iptc` stays (its tests unchanged).
  2. **`import_worker.rs`:** `apply_iptc` takes the form's `to_template()` instead of
     `Iptc::to_template`, and skips the ExifTool start when `template_is_empty()` (as
     `iptc.is_empty()` does today, so `empty_iptc_template_does_not_start_exiftool` keeps
     passing).
  3. **`main.rs`, the dialog:** after each of the six fields a drop-down list of its
     `allowed_modes` with `mode_text`; the text field is disabled when the mode is `Behåll` or
     `Rensa`. The modes are kept with the rest of the form between imports in the session
     (`last_form`), as the texts are.
  4. **`main.rs`, the start (D14):** when `cleared_fields()` is not empty, pressing the start
     button first opens a window "Rensa fält?" with `clear_confirmation_text` and the buttons
     "Rensa och importera" (starts the import) and "Avbryt" (back to the dialog, nothing
     started). Asked once per start of an import, never per image.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Nothing in `pw-metadata` changes.
- **Tests to add:**
  - `import.rs`: `to_template` of the default form with texts gives the same template as
    `to_iptc().to_template()` (all six filled; none filled); Append on Creator, Prefix on Credit,
    Clear on E-post (no value) and Keep on a filled Copyright (no `xmpRights:Marked`);
    `allowed_modes` for each field; `template_is_empty` (all Keep; all Replace with empty texts;
    one Clear → false); `cleared_fields` and `clear_confirmation_text` for two fields;
  - `import_worker.rs` (ExifTool required, as the existing IPTC import test): an import of the
    JPEG fixture whose file already has a work e-mail, with E-post in `Clear`, leaves no e-mail
    (read back with `read_metadata`); Creator in `Append` on a file with a creator gives both
    names in order.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand: an import with a
  cleared Webb asks once and clears it in every imported image.

### T101. ADR 0007 I10h: RawStride tells the user at start when ExifTool is missing or too old — Open

- **Can start after T94 and T100 are accepted** (D9 approved on 2026-11-12). Not at the same time as T103,
  T105 or T106 (`main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/import_worker.rs`;
  `crates/pw-loupe/src/metadata.rs` (`write_error_text` only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §11 (checked at start) and D9. T94 gives the result; nothing shows it yet, and
  the import worker starts its own ExifTool for the template.
- **What:**
  1. **`main.rs`:** each frame until it has been shown, `metadata_worker.exiftool_version()`;
     when it is `Some`, at most one message window (as the recovery message, T91):
     - `Err(text)`: title "ExifTool saknas", text "ExifTool kunde inte startas: <text>. Betyg och
       metadata kan inte läsas eller sparas.";
     - `Ok(v)` below `MIN_EXIFTOOL_VERSION`: title "ExifTool är för gammal", text "ExifTool <v> är
       för gammal. RawStride behöver version 12.67 eller senare. Betyg och metadata sparas inte
       förrän ExifTool har uppdaterats.";
     - `Ok(v)` at least 12.67: nothing.
     A pure function `exiftool_message(&Result<ToolVersion, String>) -> Option<(&str, String)>`
     makes the title and text, so it can be tested.
  2. **`import_worker.rs`:** after the import worker has started its ExifTool for the template, it
     asks `version()` once; below 12.67 or unreadable, the template is not written and one line
     `"<file>: ExifTool <v> är för gammal (minst 12.67)"` (or the error text) is listed, as a
     failed start is today; the copied files are kept.
  3. **`metadata.rs`:** the rating writes refused by T94 (`WriteError::Other` holding
     `EXIFTOOL_TOO_OLD`) get the Swedish reason "ExifTool är för gammal (minst 12.67)" in
     `write_error_text`, as `NO_JOURNAL` gets its text today.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:** `exiftool_message` for an error, 12.40, 12.67 and 13.59; the
  `write_error_text` of `EXIFTOOL_TOO_OLD`; the import worker with
  a fake ExifTool program that answers `-ver` with `12.40` (as the existing fake-program tests of
  the import worker, or the `test_support` pattern) lists the line and writes nothing.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T102. ADR 0007 I10i: the texts of the info panel (values, conflicts, notices) — Open

- **Can start now.** The owner approved D11 on 2026-11-12. It only needs the reader's types (T82), so it can run at the same time as T94–T100 (only
  `lib.rs` lines shared, see T99).
- **Crates:** `pw-loupe` only. Drawing the panel is T103.
- **Files:** new `crates/pw-loupe/src/info_panel.rs`; `crates/pw-loupe/src/lib.rs` (one `mod` line,
  one `pub use` line); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 ("conflicts always visible": both values and the rule applied, until the
  next write by RawStride), §6.2, §6.4 ("hidden by the user"), §5.4 (converted text marked), Q5/Q8
  (ignored sidecars reported), §7 (suggested Date Created).
- **What** (`info_panel.rs`, pure functions, no egui):
  1. `pub enum Language { Sv, En }` and `pub fn property_label(xmp_id: &str, language: Language)
     -> String` (owner, 2026-11-12: so an English RawStride later only has to choose the language):
     - `Sv`: the Swedish name of D11 for the properties and fields in its list, otherwise as `En`;
     - `En`: always the IPTC name from the table (`name` of `properties::property`, or of the field
       in `properties::struct_def` for a structure field), otherwise the id.
     The Swedish names are in **one** table in `info_panel.rs`, keyed by XMP id; no other code holds
     property names. Everything else in this task uses `Language::Sv` (the rest of RawStride's
     texts stay Swedish in the code; a language setting is not part of I10).
  2. `pub fn value_text(value: &Value) -> String`: `Text` and `Number` as they are; a Lang Alt its
     shown text, with ` (<språk>)` when it is not `x-default` and ` +N språk` when it has N other
     entries; a list its entries joined with `; `; a date `IptcDate::to_xmp()`; a structure
     `fält: värde` joined with `, ` (field labels by `property_label` of the field id); a list of
     structures one structure per line.
  3. `pub fn origin_text(origin: Origin) -> &'static str`: "XMP", "XMP-sidofil", "IIM", "Exif",
     "Ändrad".
  4. **`pub fn info_panel(read: &MetadataRead) -> InfoPanel`** with
     `InfoPanel { values: Vec<ValueRow>, conflicts: Vec<ConflictRow>, notices: Vec<String> }`:
     - `values`: each property with a value, in table order: `label`, `value_text`,
       `origin_text`;
     - `conflicts`: each conflict: `label`, each value with its origin, and the rule text:
       `Xmp` "XMP används", `Iim` "IIM används (kontrollsumman stämmer inte)", `Exif` "Exif
       används", `Open` "Olöst: välj värde i redigeraren";
     - `notices`, in this order (texts exactly so):
       - `FileNotice::ForeignSidecarIgnored(p)`: "<filnamn> bredvid DNG-filen används inte (skriven
         av ett annat program)"; `ExtSidecarIgnored(p)`: "<filnamn> läses inte (RawStride använder
         <basnamn>.xmp)"; `IimBlockUnreadable(r)`: "IIM-blocket kunde inte läsas: <r>";
       - `Notice::Iim(Converted)`: "<fält>: IIM-texten hade okänd teckenkodning och har
         konverterats"; `Repeated`: "<fält>: fanns flera gånger i IIM, det första värdet används";
         `Unreadable`: "<fält>: kunde inte läsas (<text>)";
       - `Notice::Refused`: "<fält> från <ursprung> lästes inte: <skäl>" with the reason in
         Swedish per `ValueError` ("fel form", "för många värden", "tomt värde", "okänt fält");
       - `Notice::HiddenByUser`: "<fält>: kamerans värde <värde> är dolt av användaren";
       - `Notice::Unreadable`: "<fält> från <ursprung> kunde inte läsas (<text>)";
       - `suggested_date_created`: "Förslag till Skapad från kameran: <datum>";
       - the IIM checksum when the file has IIM (`state`): `Matches` nothing, `Differs` "IIM har
         ändrats av ett annat program efter XMP (kontrollsumman stämmer inte)", `Missing` "IIM
         saknar kontrollsumma".
  5. **`pub enum PanelText`** and **`pub fn panel_text(text: &PanelText) -> String`** for the
     panel's fixed texts that T103 shows: `Selected(n)` "<n> bilder markerade", `NoneSelected`
     "Ingen bild vald", `Loading` "Läser metadata …", `ReadError(e)` "Metadata kunde inte läsas:
     <e>", and the section headings `Values` "Metadata", `Conflicts` "Konflikter", `Notices`
     "Meddelanden".
- **Text rule** (owner, 2026-11-12: so English can be added later in one module): every text the
  user sees in the info panel is made by a function in `info_panel.rs`; drawing code holds no
  user-visible string literals. Each text is a whole sentence built from one template with
  placeholders (`format!` with the values filled in), never joined from separately translated
  pieces, so another language can change the word order. Numbers that need singular/plural
  (such as ` +N språk`) are formatted inside the same function.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Nothing in `pw-metadata` changes.
- **Tests to add** (unit tests on hand-built `MetadataRead`): `property_label` for a listed, an
  unlisted and an unknown id, each with `Sv` and `En` (`dc:creator` gives "Fotograf" and
  "Creator"; a contact field gives "E-post" and the table's field name); `value_text` for every `Value` kind (Lang Alt with `x-default` only,
  without `x-default`, with two other languages; a list; a date with and without a zone; a contact
  structure; two Location Created); every rule text; every notice text above once; every
  `PanelText` once; the order of values follows the table.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T103. ADR 0007 I10j: the info panel in RawStride — Open

- **Can start after T98, T101 and T102 are accepted** (D15 approved on 2026-11-12). Not at the same time as
  T105 or T106 (`main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/lib.rs` (`Action` and `key_action` only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.1 and §6.2 (conflicts visible), I10 in the step plan.
- **What** (as D15):
  1. **`lib.rs`:** `Action::ToggleMetadata` on the key `M` (no Ctrl or Alt), in the loupe and the
     contact sheet.
  2. **`metadata.rs`:** the cache keeps the `MetadataRead` (or the error) of **one** image, the one
     shown in the panel. Requests through `MetadataWorker::read_metadata` (T98), at most **one in
     flight**: when the image changes while a read runs, the newest image is asked for when it
     comes back, and results for another image are dropped. A read is asked for only while the
     panel is shown and after the image has been the current one for 200 ms (moving quickly with
     the arrow keys queues nothing). After `RatingWritten` or `EditsWritten` for the image, it is
     read again. A loaded folder change empties it.
  3. **`main.rs`:** a panel on the right (`egui::SidePanel::right`, width 320, resizable, its own
     scroll area), shown and hidden with `M`, hidden at start, not remembered between runs. The
     image is the loupe's current image, or in the contact sheet the single selected image; with
     several selected `PanelText::Selected(n)`, with none `PanelText::NoneSelected`. Contents:
     `PanelText::Loading` while loading; `PanelText::ReadError(fel)` on an error; otherwise
     the file name and three sections from T102's `info_panel`, headed by `PanelText::Values`
     (label, value, origin in grey), `PanelText::Conflicts` (shown only when there are any, in the
     warning colour) and `PanelText::Notices` (only when there are any). A file kind without
     metadata support shows `UNSUPPORTED_TEXT`.
- **Text rule** (as T102): the panel's drawing code in `main.rs` holds no user-visible string
  literals; every text comes from `info_panel.rs` (`info_panel`, `panel_text`). A text that turns
  out to be missing is not written in `main.rs`: report it in the hand-over, and it is added to
  `info_panel.rs` as a whole-sentence template. The panel does not assume a fixed text width (labels and values wrap).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. The rating path is unchanged.
- **Tests to add:** `key_action` for `M` (with and without Ctrl); `metadata.rs` with the existing
  fake submitter: one read in flight, the newest image asked for after it, a stale result dropped,
  no read before 200 ms or while hidden, a new read after a rating write of the image.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand: the panel shows the
  values of a JPEG, a DNG and an ARW with a sidecar, and a conflict (a file from the level-C samples
  or one made by the reviewer).

### T104. ADR 0007 I10k: the editor's form with its fields, conflict choices, edits and warnings — Open

- **Can start after T96, T97 and T102 are accepted** (D12 approved on 2026-11-12). It changes no file of
  T98, T101 or T103, so it can run at the same time as them (only `lib.rs` lines shared, see T99).
- **Crates:** `pw-loupe` only. The window is T105.
- **Files:** new `crates/pw-loupe/src/editor.rs`; `crates/pw-loupe/src/lib.rs` (one `mod` line, one
  `pub use` line); `docs/test-inventory.md`.
- **Why:** ADR 0007 §8 (Lang Alt: `x-default` only; clearing removes every representation),
  §5.1/Q1 (the user decides an `Open` conflict), §7 (truncated dates, no zone invented), §11
  (warnings).
- **What** (`editor.rs`, pure, no egui), with the fields of D12:
  1. `pub enum EditorField` with the D12 fields in the D12 order; `label()` from T102's
     `property_label(…, Language::Sv)` (the contact fields "E-post", "Telefon", "Webb"; no names
     of its own in `editor.rs`); `multiline()` true for
     Beskrivning, Alternativtext, Instruktioner, Användarvillkor, Nyckelord and Fotograf.
  2. **`EditorForm::from_read(read: &MetadataRead) -> EditorForm`:** one text per field: a text as
     it is; a Lang Alt its shown text (and a note "(<språk>)" when it has no `x-default`, so saving
     adds `x-default`, Q7); a list one entry per line (Nyckelord, Fotograf); a date `to_xmp()`;
     the contact fields from `CreatorContactInfo`.
  3. **Conflicts:** `EditorForm::open_conflicts()` lists every `Open` conflict of the file (also on
     properties that are not editor fields): label and each value with its origin.
     `EditorForm::choose(conflict_index, value_index)` records the choice; for an editor field it
     also puts the value's text in the field.
  4. **`EditorForm::edits(&self) -> Result<Vec<Edit>, Vec<(EditorField, String)>>`:** for each
     changed field (the text differs from the one `from_read` gave): an empty text (after trim) →
     `Edit::Clear`; otherwise `Edit::Set` with the value of the field's kind: a text; a Lang Alt
     `x-default`; a list from the non-empty trimmed lines; a date by `IptcDate::parse_xmp`, an error
     "Skriv datumet som ÅÅÅÅ, ÅÅÅÅ-MM, ÅÅÅÅ-MM-DD eller ÅÅÅÅ-MM-DDTtt:mm[:ss], med tidszon bara om
     den är känd"; the contact fields as `field` edits of `Iptc4xmpCore:CreatorContactInfo`. A
     chosen conflict value is an `Edit::Set` of that value even when the text did not change (T97
     then writes both forms). All field errors are returned together.
  5. **Warnings:** `EditorForm::warnings(&self, read: &MetadataRead, create_iim: bool) ->
     Vec<String>`: the edits applied to a copy of the read metadata with T97's `apply_edits`, then
     T96's `validate` with `iim_written` = the file has IIM, or `create_iim` and the file is not a
     manufacturer RAW; Swedish texts: "<fält> blir avkortat i IIM (högst <n> byte); XMP får hela
     texten", "Alternativtexten är längre än 250 tecken (<n>)", "Landskoden <kod> ska vara två
     eller tre bokstäver (ISO 3166-1)", "Fotografens titel utan Fotograf", "Digital källtyp <uri>
     finns inte i IPTC:s lista", "Registerpost <n> saknar post-id eller organisation", "Betyget
     <v> ligger utanför −1 till 5".
  6. `pub fn notice_text(notice: &WriteNotice) -> String` for the editor's result (T105):
     `Truncated` "<fält> kortades i IIM; XMP har hela texten"; `IimConvertedToUtf8` "IIM-texterna
     skrevs om som UTF-8"; `ConflictKept` "<fält>: konflikten är olöst och skrevs inte";
     `DigestNotWritten` "Ingen ny IIM-kontrollsumma skrevs, eftersom en konflikt är olöst".
- **Text rule** (as T102; owner, 2026-11-12: so English can be added later in one module): every
  text the user sees from the editor (labels, the Lang Alt note, field errors, warnings, notices)
  is made by a function in `editor.rs` or `info_panel.rs`; property names only through
  `property_label`. Each text is a whole sentence built from one template with placeholders
  (`format!` with the values filled in), never joined from separately translated pieces, so
  another language can change the word order; numbers that need singular/plural are formatted
  inside the same function. Nothing returned by `editor.rs` is meant to be completed by string
  joins in the window's code (T105).
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Nothing in `pw-metadata` changes.
- **Tests to add** (unit tests on hand-built `MetadataRead`): `from_read` of every field kind and of
  a Lang Alt without `x-default`; no edits for an untouched form; a changed text, an emptied field
  (`Clear`), two keywords on three lines with one blank, a date `2026-11` and a bad date (error
  text), several errors at once, an e-mail as a `field` edit; a chosen `Open` conflict value with
  an unchanged text gives a `Set`; a conflict on a non-editor property is listed and chosen;
  each warning text once; each notice text.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.

### T105. ADR 0007 I10l: the editor window in RawStride, saved through the worker — Open

- **Can start after T103 and T104 are accepted.** Not at the same time as T106 (`main.rs`).
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/metadata.rs`;
  `crates/pw-loupe/src/lib.rs` (`Action` and `key_action` only); `docs/test-inventory.md`.
- **Why:** I10 in the step plan: the editor, written with §9 through T97/T98.
- **What** (as D12 and D15):
  1. **Opening:** `Action::EditMetadata` on the key `E` (no Ctrl or Alt), and a button "Redigera"
     at the top of the info panel; only for one image whose kind can hold metadata
     (`supports_rating`), and only when its `MetadataRead` is loaded (otherwise the key does
     nothing). The window "Redigera metadata – <filnamn>" belongs to that image; moving to another
     image does not change it.
  2. **Contents:** the fields of `EditorForm` (a multi-line box where `multiline()`), the field
     errors under their fields, the "Olösta konflikter" section with a choice per conflict (radio
     buttons with origin and value), the warnings (in the warning colour), and the buttons
     "Spara" and "Stäng".
  3. **Saving:** "Spara" with field errors does nothing but show them; with no edits it does
     nothing; otherwise `write_edits` (T98). While it runs the form is read-only with "Sparar …".
     `EditsWritten`: `Ok(report)` → "Sparat." and the notice texts (T104), the panel's read is
     renewed (T103) and the form is filled again from the new read; `Err(e)` → "Kunde inte spara
     metadata för <fil>: <skäl>" with the reason as `write_error_text` gives it for ratings
     (`metadata.rs`), the form kept as typed.
  4. **Closing** with unsaved edits asks "Spara ändringarna?" with "Spara", "Släng" and "Avbryt".
     At program exit an editor write still running is waited for as rating writes are (T44's
     exit wait, the same limit), and listed if it does not finish.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change. Keys typed into the editor's text fields
  must not reach the rating and navigation keys (as the import dialog already prevents).
- **Tests to add:** `key_action` for `E`; `metadata.rs` with the fake submitter: an edits request
  sent with the form's edits, `EditsWritten` `Ok` asks for a new read of the image, `Err` keeps the
  pending state cleared and gives the Swedish text; the exit wait counts a running edits write.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand in RawStride: a Title and a
  keyword on a JPEG, a DNG shown in RAW mode and an ARW (sidecar), read back in the panel and in
  another program (Photo Mechanic or Lightroom); an emptied Description; a conflict chosen.

### T106. ADR 0007 I10m: the settings window (create IIM, fallback encoding) — Open

- **Can start after T98, T99 and T101 are accepted.** Not at the same time as T103 or T105
  (`main.rs`); the reviewer decides the order of T103, T105 and T106.
- **Crates:** `pw-loupe` only.
- **Files:** `crates/pw-loupe/src/main.rs`; `crates/pw-loupe/src/import_worker.rs`;
  `crates/pw-loupe/src/metadata.rs` (only if the panel's read must be renewed after a change);
  `docs/test-inventory.md`.
- **Why:** ADR 0007 §5.2 (the setting "Skapa IPTC-IIM för kompatibilitet med äldre system", off by
  default) and §5.4/Q10 (the fallback encoding, changeable in the settings).
- **What:**
  1. **At start** (`run`): `load_settings(settings_path())` (T99), each problem written with
     `eprintln!`; the settings' `edit_options()` go into the metadata worker's `WriteSetup` and
     into each import (point 3) instead of `EditOptions::default()`.
  2. **The window "Inställningar"**, opened with a button "Inställningar" in the top bar (next to
     the import button): a check box with the exact text "Skapa IPTC-IIM för kompatibilitet med
     äldre system" and under it, in small text, "Gäller JPEG, TIFF och DNG som saknar IIM. Av som
     standard."; a drop-down "Teckenkodning för äldre IPTC- och Exif-text" with "Windows-1252
     (västeuropeisk)" and "ISO 8859-1 (Latin-1)"; the buttons "Spara" and "Avbryt". "Spara" saves
     the file, sends `set_options` to the metadata worker (T98) and empties the panel's cache so
     the image is read again with the new encoding; when the file cannot be saved, the settings
     still apply until RawStride closes and the message "Inställningarna kunde inte sparas: <fel>"
     is shown.
  3. **Import:** `ImportWorker::run` takes the `EditOptions` of the settings for that import (a
     new parameter, or a field next to the form), used by `apply_template_safely`.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add:** `import_worker.rs` (ExifTool required, as the existing IPTC import test): an
  import with `create_iim: true` of the JPEG fixture (no IIM) and a Creator gives IIM By-line in
  the copy, with the default options no IIM; a pure function in `main.rs` (or `settings.rs` if
  you need it, then say so) for the two encoding texts and back.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above. The owner checks by hand: the setting survives a
  restart, and with "Skapa IPTC-IIM" on, a Title written in the editor to a JPEG without IIM
  appears in IIM (ExifTool `-IPTC:all`).

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
- ADR 0007 (IPTC metadata) steps I11–I13 and the `Cleared` task (see "Milestone 8 (ADR 0007, IPTC
  metadata): steps"; I9 is T87–T93, I10 is T94–T106): `Cleared` waits for D1; I11 waits for the PLUS controlled
  vocabulary; I12 waits for the level-C samples. Also later: Exif ImageEditor ↔ Contributor and
  GPS → Location Created (§6.1), left out of T81; keyword qualifiers (§8), left out of T83; from
  §10, left out of T88: template sub-fields of lists of structures (Location Created, Image
  Creator, Copyright Owner, Licensor), variables (`{date}` …), template files in XMP with the
  modes in a RawStride namespace (needs the same kind of namespace decision as D1). Left out of
  I10: editing several images at once, lists of structures in the editor, the other controlled
  vocabularies of §11.
- **Future improvement (owner, 2026-11-12, after D11):** Swedish names for **every** IPTC property
  and structure field in T102's name table, so the Swedish info panel and editor show no English
  IPTC names. Today D11 names about 35 of them; the rest fall back to IPTC's English name. Needs
  the owner's approval of the full list.
- The import dialog fields for title and description in `ui/` (frontend).
- ADR 0010 steps M3–M7 (the rest of the import) wait until the contact sheet (ADR 0011) is done
  (owner decision, 2026-11-08). M4 and M7 are added to this list then.
- ADR 0011 step K4 (drawing the contact sheet) is planned after T23–T25.
- Moving rejected or picked images to `_rejected` / `_picked` folders (ADR 0013 §13): needs its own
  ADR first.
- **A filter bar of toggle buttons instead of drop-down lists** (owner idea, 2026-11-10, after T61;
  ADR 0014 §15): a row "0★ 1★ 2★ 3★ 4★ 5★" and a row of colour swatches, each clicked on and off,
  with a check box per row that turns the row on or off. Needs an amendment of ADR 0014 first: the
  star part would become a set of star values instead of the ≥ / = / ≤ condition (T60).
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
| 2026-11-10 | T55 | Ready for review | (Agent) New `browse.rs` (`impl Tab`, declared in `lib.rs`, `LoupeMove` re-exported): `refilter` (all without an active filter; empty with an active filter before the ratings are read; otherwise `filter::visible` with `keep`; then `retain_visible` and `nearest_visible`; returns whether `visible` changed), `position`, `grid_move`, `click`, `loupe_move` (each ends with `refilter` keeping the new current; `loupe_move` and plain moves use `selection.click`). Decisions to check: a click on a file index that is not visible changes nothing (S5); `grid_move` and `loupe_move` with nothing visible change nothing (otherwise keep = current would show a hidden image; `app.js` ignores keys then); a move does not refilter before moving (as written in the task and §4.2; `go()` in `app.js` also refilters first, which differs only when another image than the current one stopped matching, e.g. after a batch rating). `tabs.rs`: `Tab.visible`; `Tab::open` ends with `refilter(true, …, None)`, so a newly opened folder shows every file. `thumbs.rs`: `to_request_indexes`, `forget_not_in`, sharing private `wants`/`forget` with the range methods. `metadata.rs`: `ratings_ready`, `read_error` (set only when a folder read fails: refused, failed answer, or the worker stops while the read is sent or waiting; cleared by `begin_load`); `read_error` has `#[cfg_attr(not(test), allow(dead_code))]` until T56. `lib.rs`: `position_text` (`pos` zero-based), `no_match_text`, `marked_text`, `READING_RATINGS_TEXT`. 15 tests; existing tests unchanged; test inventory updated (`pw-loupe` 204). `.\tools\check.ps1`: fmt, test (427 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T55 | Open | Review: accepted except one point; `grid_move` and `loupe_move` must also refilter (keeping the current image) before the move, as `go()` in `app.js` and ADR 0014 §4.2, so images that stopped matching after a batch rating drop out before the move instead of becoming current. `read_error`'s temporary `allow(dead_code)` is removed in T56. Checked: `test-output.txt` newer than the changes, fmt/test (427 passed, 27 ignored, 0 failed)/clippy OK |
| 2026-11-10 | T55 | Ready for review | (Agent) Review point, `browse.rs` only: `grid_move` and `loupe_move` first call the private `refilter_before_move` (`refilter` keeping the **current** image, as `go()` in `app.js`; with nothing visible it returns at once and the move changes nothing, as accepted), then move over that list, then `refilter` keeping the new current image as before. `click` unchanged. Doc comments updated. 2 tests (shared setup: Unrated, current 5, marked 5–7, all three get stars): `grid_move(Right)` and `loupe_move(Next)` → current 8, visible [0, 4, 8, 9, 10, 11], marking cleared. Existing tests unchanged and passing. Test inventory updated (`pw-loupe` 206). `.\tools\check.ps1`: fmt, test (429 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T55 | Done | Second review: `grid_move` and `loupe_move` refilter keeping the current image before the move (`refilter_before_move`), then after it as before; `click` unchanged; the two batch-rating tests give current 8 and visible [0, 4, 8, 9, 10, 11] in grid and loupe. Checked: `test-output.txt` newer than the change, fmt/test (429 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 206, matching the test inventory. T56 can start (T53 and T55 done) |
| 2026-11-10 | T56 | Ready for review | (Agent) `main.rs`: two `ComboBox` after "Importera…" (`STAR_FILTERS`/`LABEL_FILTERS` with their texts; yellow outline when not "Alla …"); a choice calls `Tabs::set_filter` and `refilter(None)`, then surrenders the keyboard focus. Refilter on a filter change, on a tab load (`load_active_tab`, keep `None`) and when the active tab's readiness changes; readiness = "ratings read since the tab load" (new `ratings_read`, so the second folder read that `switch_mode` sends when the loupe opens does not empty the grid or drop a kept image) and no `read_error` (a read error also refilters, so an active filter shows nothing). Grid over `tab.visible`: rows, visible range, keys and scrolling by position; `to_request_indexes`, `set_visible` with the smallest..largest file index + 1 (new helper `index_span`, 1 test), `forget_not_in` of the keep range; with nothing visible the read error, `READING_RATINGS_TEXT` or `no_match_text` in the middle. Marked cells: faint blue fill and a 2 px `#4c8dff` outline outside the cell; the yellow current outline stays inside. Clicks read Ctrl/Shift and call `Tab::click`; a double-click opens the loupe without a second click. Keys: `Navigate`/`Extend` → `grid_move`, `SelectAll`, `ClearSelection`. Rating keys: `selection.targets` → `set_ratings` (nothing when the current image is not shown). Status line: `position_text` · `marked_text` · name (marking before the name, owner decision after the first report), or the nothing-shown text. Loupe (interim way A): `open_folder` with the visible paths and `go(position)`, also after a filter change or a readiness refilter while the loupe is shown; `frame.pos` mapped through `visible`; Enter/G/Space and double-click do nothing with nothing visible. `metadata.rs`: `allow(dead_code)` removed from `set_ratings` and `read_error`; added to `set_rating`, which only the tests use now (decision to check). Test inventory updated (`pw-loupe` 207). `.\tools\check.ps1`: fmt, test (430 passed, 27 ignored, 0 failed) and clippy OK; no commit. Not checked by hand (needs the owner, §8.2). |
| 2026-11-10 | T56 | Done | Reviewed after the owner's hand check: filter lists with outline and focus release; readiness = ratings read since the tab load and no read error (so the loupe's second read does not empty the grid); the grid over `visible` (positions for layout, file indexes for thumbnails, `index_span` for the worker's priority); marked cells; clicks with Ctrl/Shift; marking keys; rating keys through `targets` and `set_ratings`; status line with the marking before the name (owner); interim loupe on the visible list. `allow(dead_code)` moved from `set_ratings`/`read_error` to `set_rating` (tests only), accepted. One finding for T59: `rating_of` and the grid's `shown_rating` use unresolved `tab.files`, so a relative command-line folder filters as all unrated. Checked: `test-output.txt` newer than the changes, fmt/test (430 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 207, matching the test inventory |
| 2026-11-10 | – | Note | ADR 0014 amendment 1 (owner, §13): §2.1 star condition ≥, =, ≤; §2.2 a rated image that stops matching leaves at once; §5.3 marking before the name. ADR updated (§2, §3.1, §4.2, §5, §7 S2 and S6, §8.2, §9, §13) |
| 2026-11-10 | T57, T59, T60 | Open | T59 added (§2.2, with the resolved-path point from the T56 review); T57 replaced (loupe after a rating, status line order); T60 added (§2.1). Order T59 → T57 → T60 (all change `main.rs`); T58 also covers amendment 1 |
| 2026-11-10 | T59 | Ready for review | (Agent) `browse.rs`: `Tab::rated` (`refilter` without keeping any image; returns whether `visible` changed); `grid_move`, `loupe_move`, `click` unchanged; doc comments updated (module, `refilter`, `refilter_before_move`, `grid_move`, `loupe_move`), and `Tab.visible` in `tabs.rs` (comment only). `main.rs`: after `set_ratings` returns `Ok`, `rated_active` (same readiness as the other refilters); if `visible` changed: grid → scroll to the current position; loupe → `reopen_loupe` (interim), or `switch_mode(Grid)` when nothing is visible. A refused key does not call `rated`; a rolled-back write does not refilter. Point 4: `resolve_path(cwd, path)` (used by `resolve_paths`), `shown_rating(metadata, cwd, path)` and `tab_rating`, used by `refilter_active`, `rated_active`, `grid_move`, `click` and the grid's badge lookup; the current folder is read once per call, and without one the path is looked up as it is (decision to check). Tests changed as approved: the block in `the_filter_select_cases_of_app_js`; comments in `a_loupe_step_drops_the_rated_image_it_leaves` and `batch_rated_under_unrated`. 6 tests added (5 for `rated` in `browse.rs`, 1 for `resolve_path` in `main.rs`). Test inventory updated (`pw-loupe` 213). `.\tools\check.ps1`: fmt, test (436 passed, 27 ignored, 0 failed) and clippy OK; no commit. Not checked by hand (needs the owner): the relative command-line folder and the "Done when" hand checks. |
| 2026-11-10 | T59 | Ready for review | Review: code accepted. `rated` refilters without keeping; `main.rs` calls it only after an accepted key (grid scrolls, loupe reopens on the visible list or returns to the grid when nothing is left); the approved test changes are limited to the named block and comments; five `rated` tests match the task. Point 4: `shown_rating`/`tab_rating` resolve relative tab paths as `begin_load` does; looking a path up as it is when there is no current folder is accepted. Note: the grid now reads the current folder once per frame (a cheap call, no disk I/O); fine for now. Checked: `test-output.txt` newer than the changes, fmt/test (436 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 213, matching the test inventory. Waiting for the owner's hand check |
| 2026-11-10 | T59 | Done | Owner's hand check passed (a rated image and a rated marking leave at once in grid and loupe; another label under "Grön"; a relative command-line folder shows and filters ratings). Committed. T57 can start |
| 2026-11-10 | T57 | Ready for review | (Agent) `main.rs`: loupe steps (Next, Previous, First, Last) go through `Tab::loupe_move` (new `Loupe::loupe_move`); if `visible` changed, `core.set_files(paths of visible, position)`, otherwise `core.go(position)`. New `loupe_follows_visible` (loupe shown: `set_files` with the resolved visible paths and the position; nothing visible → `switch_mode(Grid)`) replaces the interim `reopen_loupe` for a filter change, the ratings becoming ready and an accepted rating key after `Tab::rated`. The tab's current image is no longer taken from `frame.pos` (the core follows the tab). Opening the loupe from the grid (`switch_mode`) and a tab load still use `open_folder` (`open_loupe_core`). `lib.rs`: `status_line(frame, anchor_at_pointer, total)` starts with `position_text(pos, frame.count, total)` ("3 / 12 (av 31)" or "3 / 31"); the existing status-line tests pass `total = 31`, the first one now checks `"3 / 31  ·  "`. 2 tests added in `lib.rs`: the loupe status line with and without a filter; the `app.js` case "loupe within filter" on `Tab::loupe_move` (Red, Home, Enter → IMG_002, step right → IMG_004, both ends stay in the filter). Test inventory updated (`pw-loupe` 215). `.\tools\check.ps1`: fmt, test (438 passed, 27 ignored, 0 failed) and clippy OK; no commit. Not checked by hand (needs the owner): smooth stepping under "≥ 3 stjärnor", rating under "Ej betygsatt" without a cold load, `latency.ps1` (V1, S3), batch of 30 then closing at once (R4). |
| 2026-11-10 | T57 | Ready for review | Review: code accepted. Every loupe step goes through `Tab::loupe_move`; the core gets `set_files` only when `visible` changed, otherwise `go`; `loupe_follows_visible` replaces `reopen_loupe` for a filter change, the ratings becoming ready and a rating key; `open_folder` remains only for opening the loupe from the grid and a tab load (checked: no other `core.step`/`go`/`last`/`open_folder` call); `frame.pos` no longer sets the tab's current image. `status_line` takes the folder total; the one existing assertion that changed is stricter (`"3 / 31  ·  "`). Checked: `test-output.txt` newer than the changes, fmt/test (438 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 215, matching the test inventory. Waiting for the owner's hand check |
| 2026-11-10 | T57 | Ready for review | Owner's `latency.ps1` run (`D:\PhotoWorkflowData\pm-study\results\latency-rawstride.csv`, label `t57-filter-next`): loupe under a filter, RIGHT × 7, maximised window. Six steps: first change 10.9–17.0 ms (median 13.7 ms; one frame is about 16.7 ms, so these are within one frame), one change each (V1 ≤ 30 ms and V2 met, S3). The seventh press gave no change: the end of the visible list, no step outside the filter. No run without a filter for comparison |
| 2026-11-10 | T57 | Done | Owner's hand checks passed (smooth stepping under "≥ 3 stjärnor", the next unrated image at once after a rating under "Ej betygsatt", a batch of 30 kept after closing at once). Committed. T60 can start |
| 2026-11-10 | T60 | Ready for review | (Agent) `filter.rs`: `StarCondition { AtLeast (default), Exactly, AtMost }`, `STAR_CONDITIONS`, `star_condition_text` ("≥", "=", "≤"); `StarFilter::AtLeast(u8)` → `Stars(u8)` (clamped 1–5); `Filter.condition`; `matches` for `Stars(n)`: ≥ n, exactly n, or unrated/0–n; a reject never matches; `is_active` is `stars != All \|\| label != All`; star texts "1 stjärna" … "5 stjärnor". `lib.rs`: re-exports. `main.rs`: a narrow (40 pt) condition list before the star list, enabled only for a number of stars (keeps its value while disabled), yellow outline then; same choice handling as the other lists (`filter_combo` got an optional width). Tests: `AtLeast(n)` → `Stars(n)` with the default condition in `filter.rs`/`browse.rs`, the two text tests get the new texts; with the owner's approval the `Filter` literals in the `tabs.rs` test helpers `unrated`/`red` and the `lib.rs` test got `condition: AtLeast` (assertions unchanged). 6 tests added (5 in `filter.rs`, 1 in `browse.rs`: = 3, = 5, ≤ 3, ≤ 3 and Red, ≥ 3). Test inventory updated (`pw-loupe` 221). `.\tools\check.ps1`: fmt, test (444 passed, 27 ignored, 0 failed) and clippy OK; no commit. Not checked by hand (needs the owner): "= 3", "≤ 2", "≥ 4" on the test set; the condition disabled under "Ej betygsatt" keeping its value; two tabs with different conditions |
| 2026-11-10 | T60 | Ready for review | Review: code accepted. `matches` for `Stars(n)` clamps n to 1–5 and compares by the condition; ≤ n takes unrated and 0–n, and no condition matches −1; `is_active` ignores the condition. `main.rs`: the condition list is disabled unless a number of stars is chosen, keeps its value, has the yellow outline then, and goes through the same `set_filter` path. Test changes limited to the rename and the two text tests; the `condition` field added to the `Filter` literals in the `tabs.rs` and `lib.rs` tests (owner's approval, assertions unchanged). Checked: `test-output.txt` newer than the changes, fmt/test (444 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 221, matching the test inventory. Waiting for the owner's hand check |
| 2026-11-10 | T60 | Done | Owner's hand check passed ("= 3", "≤ 2", "≥ 4"; the condition disabled under "Ej betygsatt" keeping its value; two tabs). Committed |
| 2026-11-10 | T61 | Blocked | Added after the owner's hand check of T60: filter on several labels at once. ADR 0014 §14 (amendment 2, proposed) with Q10–Q13; T61 waits for the answers. T58 now starts after T61 |
| 2026-11-10 | T61 | Open | ADR 0014 amendment 2 (§14) accepted by the owner with Q10–Q13 as proposed; T61 can start |
| 2026-11-10 | T61 | Ready for review | (Agent) `filter.rs`: `LabelChoice { Label, NoLabel }`, `LABEL_CHOICES` (the five `LABELS`, then `NoLabel`), `label_choice_text`; `LabelFilter` is now a `Copy` bit set (empty = all) with `all`, `is_all`, `contains`, `toggle` (a full set of six is cleared, Q12), `only`; a label outside `LABELS` is not a choice (ignored by `only`/`toggle`/`contains`, never matches a non-empty set). `matches`: empty set → every label, otherwise the image's label or `NoLabel` must be in the set, AND with stars; `is_active`: stars not `All` or the set not empty. `label_filter_text(&LabelFilter) -> String`: "Alla etiketter", one name, two joined with ", " (list order), "{n} etiketter" from three. `LABEL_FILTERS` removed (replaced by `LABEL_CHOICES`). `lib.rs`: re-exports. `main.rs`: a new `label_combo` replaces the `filter_combo` call for the label list: "Alla etiketter" (clears the set) and a check box per choice, `PopupCloseBehavior::CloseOnClickOutside`, yellow outline when the set is not empty, each change through `set_filter` as before. Tests: `LabelFilter::Label/NoLabel/All` → `only(…)`/`all()` in `filter.rs`, `browse.rs`, `tabs.rs`, `lib.rs` (assertions unchanged; `label_filter_list_and_texts` gets the new lists and texts). 10 tests added (9 in `filter.rs`, 1 in `browse.rs`: Red + Yellow → [2, 4], Ingen etikett → [0, 1, 3, 5, …, 11], ≥ 3 and Red + Ingen etikett → [1, 2]). Test inventory updated (`pw-loupe` 231). `.\tools\check.ps1`: fmt, test (454 passed, 27 ignored, 0 failed) and clippy OK; no commit. Not checked by hand (needs the owner): two and three colours, a colour plus "Ingen etikett", ticking all six, the outline, two tabs with different sets |
| 2026-11-10 | T61 | Ready for review | Review: code accepted. `LabelFilter` is a six-bit set (empty = all; the sixth tick clears it, Q12); `matches` is OR within the set and AND with stars; a label outside `LABELS` (written by another program) is never in a non-empty set, documented. `label_combo`: "Alla etiketter" clears, check boxes per choice, `CloseOnClickOutside`, yellow outline when not empty, changes through `set_filter`. `LABEL_FILTERS` replaced by `LABEL_CHOICES`; test changes limited to the new constructors and the list/text test. Note for the hand check: while the popup is open, keys still reach the grid (as with the other lists). Checked: `test-output.txt` newer than the changes, fmt/test (454 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 231, matching the test inventory. Waiting for the owner's hand check |
| 2026-11-10 | T61 | Done | Owner's hand check passed (two and three colours, a colour plus "Ingen etikett", all six clearing the set, the outline, two tabs). Committed |
| 2026-11-10 | – | Note | Owner idea for later: a filter bar of toggle buttons (stars and colour swatches) instead of the drop-down lists. Recorded in ADR 0014 §15 and under "Not for agents yet"; needs its own amendment |
| 2026-11-10 | F7 | Done | Owner's acceptance of ADR 0014 (§8.2 and amendments 1–2): every check passed (filters, the condition, several labels, two tabs, thumbnails not reloaded on a filter change (S1), filter reset after a restart, a rating removes the image at once in grid and loupe, marking and status line, 20 marked + 7 twice → no label (S4), Ctrl+A/Escape, a batch of 30 kept after closing at once). Stepping with an arrow held: smooth in the filter as measured in T57; in RAW mode the position runs ahead of the rendering and only the last image is shown, the same without a filter, so outside this milestone (added under "Not for agents yet"). T58 can start |
| 2026-11-10 | T58 | Ready for review | (Agent) Documentation only. ADR 0014: "Implemented" status line; §7 results for S1–S6 from the log (S3 with the owner's T57 `latency.ps1` run; S6 notes that the log has no check-by-check list against the 22 checks); §9 "Tasks" column → "Result"; new §9.1 "Findings during implementation" (`open_folder` after T54, readiness in T56, relative folders T56/T59, moves with nothing visible T55, keys while a filter list is open T61, the arrow held in RAW mode F7). ADR 0004 addendum: egui note with the differences (per tab, not saved, "Alla utom avvisade", amendment 1 points 1 and 2) and **also amendment 2 (several labels), not named in point 2 (decision to check)**. `architecture.md`: `pw-loupe` row extended; **new `pw-viewcore` row** (the table had none). GR8, root `README.md` (on top of the owner's uncommitted change), `docs/README.md` updated. `docs/test-inventory.md` unchanged (matches: 454 passed, 27 ignored). `.\tools\check.ps1`: fmt, test (454 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T58 | Done | Reviewed: only the named files changed; every result in ADR 0014 §7 and §9 is taken from the log; §9.1 findings correct; ADR 0004 note also names amendment 2 (accepted); `architecture.md` gains a `pw-viewcore` row (accepted, the table had none); GR8, both READMEs, ADR list correct; test inventory matches `test-output.txt` (454 passed, 27 ignored). **Milestone 5 (ADR 0014) is complete** |
| 2026-11-10 | T62 | Open | Added: ADR 0009 amendment 1 (§13, owner): a repeated Next/Previous in the loupe is ignored while the current image is still being prepared. Replaces the "held arrow in RAW mode" item under "Not for agents yet" |
| 2026-11-10 | T62 | Ready for review | (Agent) `lib.rs`: added and tested `step_allowed`; repeated Next/Previous are ignored only for `Preparing`, while single presses and other actions remain allowed. `main.rs`: preserves egui's repeat flag, checks loupe actions against the last `core.frame` status; grid, dialog, rating and tab handling unchanged. `docs/test-inventory.md`: `pw-loupe` 233; workspace 456 passed, 27 ignored. `.\tools\check.ps1`: fmt, test and clippy OK; no commit. Owner hand check remains: held arrows in RAW and JPEG, quick presses, damaged file, and unchanged grid arrows |
| 2026-11-10 | T62 | Ready for review | Review: code accepted. `step_allowed` is false only for a repeated Next/Previous while the last frame is `Preparing`; tests cover the four statuses and the non-step actions. `main.rs` keeps egui's `repeat` flag, gates only the loupe's `key_action`, and stores the status of the last `core.frame`. Note: the status is the previous frame's, so two repeats in one slow frame could both step; unlikely (the UI thread does not decode), so accepted. Checked: `test-output.txt` newer than the changes, fmt/test (456 passed, 27 ignored, 0 failed)/clippy OK; `pw-loupe` 233, matching the test inventory. Waiting for the owner's hand check |
| 2026-11-10 | T62 | Done | Owner's hand check and commit done. Also: the display mode of the T57 `latency.ps1` run is not known (owner); ADR 0014 S3 says "not recorded" |
| 2026-11-10 | – | Note | ADR 0015 (milestone 6: TIFF viewing) accepted by the owner (D0): scope, E1–E6, every proposal of §11 as written, `tiff` =0.9.1 in the new crate `pw-tiff`, and replacing the test `the_folder_lists_supported_files_by_name_without_tiff` in D4. IPTC is split off: parity is met by the import template (T39); ADR 0007 becomes milestone 7 after its approval |
| 2026-11-10 | T63 | Open | Added: ADR 0015 D1, the TIFF decoder moves from `src-tauri` to the new crate `pw-tiff`, with a memory limit and one decode at a time. `imaging::fit` and `downscale` stay in `src-tauri` (its RAW view uses them) |
| 2026-11-10 | T63 | Ready for review | (Agent) New crate `pw-tiff` (`lib.rs`, `convert.rs`, `tests.rs`; one dependency `tiff = "=0.9.1"`): `Layout`, `Rgb8`, `to_rgb8` (now `samples.get(..n * k)`, same "image data is short" text), `u16_to_u8`, `f32_to_u8` and the test `conversions` moved from `imaging.rs`; `dimensions` and `decode_rgb8(path, limit)` moved from `tiff_view.rs`. `TiffError` holds each message as shown today; `tiff`'s `UnsupportedError` (e.g. a palette TIFF) becomes `Unsupported` with the same "TIFF: …" text; `LimitsExceeded` becomes `TooLarge` only when a limit is set. A private static decode lock (poison taken with `into_inner`) is held by `decode_rgb8` only. `src-tauri`: `pw-tiff.workspace = true` replaces `tiff`; `imaging.rs` keeps `fit`/`downscale` and re-exports `Rgb8`; `tiff_view.rs` has one-line wrappers; `main.rs` and `rawview.rs` unchanged. `Cargo.lock`: only the new package and the two dependency lists. 16 tests in `pw-tiff`, `photoworkflow` 4; test inventory updated. `.\tools\check.ps1`: fmt, test (471 passed, 27 ignored, 0 failed) and clippy OK; no commit. |
| 2026-11-10 | T63 | Ready for review | Review: code accepted. A move with the named additions: same texts (`Display` of `TiffError`), `samples.get(..)` instead of indexing, the limit sets both buffer sizes, the lock is held for the whole decode and not by `dimensions`, the Tauri wrappers keep `main.rs` and `rawview.rs` unchanged. `Cargo.lock`: only `pw-tiff` and the two dependency lists, no new external package. Checked: `test-output.txt` newer than the changes, fmt/test (471 passed, 0 failed)/clippy OK; `pw-tiff` 16, `photoworkflow` 4, matching the test inventory. Reviewer also decoded the owner's 9 TIFFs in `D:\PhotoWorkflowData\RAW` with a temporary probe (removed): all decode, the same pixels with no limit and with 4 GiB, 26–164 ms warm (3201×2134 to 7741×5161). Notes, not blocking: a palette TIFF is `Unsupported` through `tiff`'s own error ("TIFF: …"), not the colour-type text, which matters for the Swedish texts in D4; the tests leave small files in `%TEMP%\pw-tiff-{pid}`. Finding for D5 in ADR 0015 §9.1: all 9 files are 16-bit RGB, uncompressed, orientation 1, **ProPhoto RGB** |
| 2026-11-10 | T63 | Done | Committed by the owner (2132129) |
| 2026-11-10 | T64, T65 | Open | Added: ADR 0015 D2 (the loupe core opens TIFF: a TIFF session, shown as itself in both modes, the limit from the core budget, Swedish error texts made in `pw-viewcore`) and D3 (TIFF thumbnails; `fit` and `downscale` move from `src-tauri` to `pw-tiff`, a fixed 4 GiB limit). Both can start now and do not depend on each other; T66 (D4) is added after them |
| 2026-11-10 | T64 | Ready for review | (Agent) `pw-viewcore`: `pw-tiff` optional under the `native` feature, `tiff = "=0.9.1"` as dev-dependency. `state.rs`: `is_shown_as_itself` (jpg, jpeg, tif, tiff) used by `source_in`, re-exported from `lib.rs`; `is_plain_jpeg` unchanged. `native.rs`: `NativeBackend { tiff_limit }` (`Default` = `4 << 30`, `with_tiff_limit`), `NativeSession::Tiff(Rgb)`; `open` checks `pw_raw::is_tiff_file` first for either `Source` and calls `open_tiff` (`decode_rgb8(path, Some(limit))`, orientation from `pw_raw::inspect` or 1); `render_half` = `scale::downscale` to `scaled_size(w, h, 0.5)`, `render_region` = `scale::crop` ("region … is outside the TIFF"); `tiff_error` gives the Swedish texts of ADR 0015 §5, never `damaged`. `pw-loupe` `run`: `NativeBackend::with_tiff_limit(core_budget)`. `native_files.rs`: `NativeBackend::default()` and the ignored `tiff_files_are_shown_as_themselves_in_both_modes`. `Cargo.lock`: only `pw-tiff` and `tiff 0.9.1` in `pw-viewcore`'s list. `pw-viewcore` unit 61 (+8), `native_files.rs` 4 (3 ignored); test inventory updated. `.\tools\check.ps1`: fmt, test (479 passed, 28 ignored, 0 failed) and clippy OK; `-Ignored` with `PW_TESTDATA` also run: the new test passes on the 9 TIFFs. No commit |
| 2026-11-10 | T64 | Ready for review | Review: code accepted. `is_shown_as_itself` keys a TIFF `Source::Jpeg` in both modes and leaves `is_plain_jpeg` alone; `open` checks the file type before the mode; `NativeSession::Tiff` holds the decode, `render_half` and `render_region` use `scale`; `session_bytes` unchanged (`w × h × 3` is the decoded RGB); `tiff_error` gives the §5 texts, never `damaged`; the loupe passes its core budget. `Cargo.lock`: only `pw-viewcore`'s list. Checked: `test-output.txt` newer than the changes, fmt/test (479 passed, 28 ignored, 0 failed)/clippy OK; counts match the test inventory (+8 unit, +1 ignored). The reviewer ran the new real-file test with `PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`: passed, 9 TIFFs × 2 modes in 3.7 s. Notes, not blocking: `Io`/`Decode` texts stay English after "Filen kan inte visas: " (as the task says); the tests leave small files in `%TEMP%`; with two workers, a TIFF open can wait for the other worker's TIFF decode (about 0.15 s on the owner's files), RAW opens never wait |
| 2026-11-10 | T65 | Ready for review | (Agent) New `pw-tiff/src/resize.rs`: `fit` and `downscale` moved from `src-tauri/src/imaging.rs` with `fit_keeps_aspect_and_never_upscales` and `downscale_averages_blocks` (assertions unchanged), exported from `pw-tiff`. `downscale` without slice indexing (`get`, `get_mut`, `as_chunks`, `chunks_exact`, iterators); sums in `u64` instead of `u32` (the same result wherever the old code did not overflow); a source without pixels or with short `data` gives black target pixels instead of a panic (decision to check). New test `downscale_of_7x5_to_3x2_matches_the_code_before_the_move` compares with a copy of the old code (7×5 → 3×2, plus seven sizes of a 13×11 image, plus the empty and short source). `imaging.rs` is now `pub use pw_tiff::{Rgb8, downscale, fit};` with its module comment; `rawview.rs`, `tiff_view.rs`, `main.rs` unchanged. `pw-thumbcache`: `pw-tiff` dependency, `tiff = "=0.9.1"` dev-dependency; `thumbnail` sends a TIFF (`is_tiff_file`) to `tiff_thumbnail` before `inspect` (`decode_rgb8` under `TIFF_LIMIT` = 4 GiB, `fit` + `downscale` to the cell, orientation from `inspect` or 1, errors `Unreadable(e.to_string())`); embedded previews not used. 5 tests in `extract.rs` (400×200 → 100×50 at cell 100, unchanged at 1000, uniform colour kept; Orientation 6 → 50×100; palette → `Unreadable` with the `pw-tiff` text; a TIFF with a JPEGInterchangeFormat preview that `pw_raw::inspect` reports (16×8) still gives 100×50 from the image data; ignored: every TIFF in `PW_TESTDATA` has long edge 256 at cell 256). `Cargo.lock`: only `pw-tiff` and `tiff 0.9.1` in `pw-thumbcache`'s list. `photoworkflow` 2 (−2), `pw-tiff` 19 (+3), `pw-thumbcache` 34 (5 ignored); test inventory updated. `.\tools\check.ps1`: fmt, test (484 passed, 29 ignored, 0 failed) and clippy OK; the new ignored test also run with `PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`: passed on the 9 TIFFs. No commit |
| 2026-11-10 | T65 | Ready for review | Review: code accepted. `fit` unchanged; `downscale` without slice indexing, pinned against a copy of the old code on 7×5 → 3×2 and seven sizes of a 13×11 image. Accepted the agent's two decisions: sums in `u64` (the old `u32` could overflow when a very large image is reduced to a few pixels; same result otherwise) and black pixels instead of a panic for an empty or short source (`to_rgb8` never gives one, so no visible change). `imaging.rs` only re-exports, the Tauri views unchanged. `tiff_thumbnail` runs before `inspect`, frees the full decode before orienting, uses the fixed 4 GiB limit; the embedded-preview test proves `pw-raw` sees a 16×8 preview that is not used. `Cargo.lock`: only `pw-thumbcache`'s list. Checked: `test-output.txt` newer than the changes, fmt/test (484 passed, 29 ignored, 0 failed)/clippy OK; counts match the test inventory. The reviewer ran the new real-file test with `PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`: passed, 9 thumbnails in 1.1 s (about 0.12 s each). Note, not blocking: a TIFF thumbnail has exactly the cell's long edge, a JPEG one at least the cell (DCT scaling); the grid scales both |
| 2026-11-10 | T64, T65 | Done | Committed by the owner (0a36b60, ea9ba94) |
| 2026-11-10 | T66 | Open | Added: ADR 0015 D4, `is_listed` lists TIFF (the test `…_without_tiff` replaced, owner's approval in D0), "TIFF-fil W × H", "förbereder TIFF" and "Förbereder TIFF …" for a TIFF frame. After T66: the owner's acceptance D5, then the documentation D6–D7 |
| 2026-11-10 | T66 | Ready for review | (Agent) `pw-loupe/src/lib.rs`: `is_listed` lists every extension of `pw_raw::SUPPORTED_EXTENSIONS`, TIFF included (doc comment updated); private `is_tiff(frame)` by `pw_raw::is_tiff_file` on `frame.path`, used before the JPEG cases in `source_text` ("TIFF-fil W × H"), `status_line` while preparing without a size ("förbereder TIFF") and `indicator_text` ("Förbereder TIFF …"); RAW, embedded JPEG and JPEG-file texts unchanged; `USAGE` `--core-mb` text "…decoded JPEGs and TIFFs…". Test `the_folder_lists_supported_files_by_name_without_tiff` → `the_folder_lists_supported_files_by_name`, only the expected list changed (adds `d.tif`). New tests: `tiff_files_are_listed`, `a_tiff_frame_is_named_tiff_file_with_its_oriented_size_in_both_modes`, `a_preparing_tiff_frame_says_tiff`, `a_jpeg_file_keeps_its_texts`. `main.rs`, `error_text`, `Cargo.toml` unchanged. `pw-loupe` 237 (+4); test inventory updated. `.\tools\check.ps1`: fmt, test (488 passed, 29 ignored, 0 failed) and clippy OK. Hand check of `D:\PhotoWorkflowData\RAW` left to the owner. No commit |
| 2026-11-10 | T66 | Ready for review | Review: code accepted. `is_listed` only drops the TIFF exclusion; `is_tiff` looks at the path because the core keys a TIFF `Source::Jpeg`, and comes before the JPEG cases in `source_text`, `status_line` and `indicator_text`; RAW, embedded-JPEG and JPEG-file texts unchanged (a new test pins the JPEG file's). The replaced test changes only its name and the expected list (`d.tif` added, `C.dng` still at 2), as approved in D0. `main.rs`, `error_text`, `Cargo.toml` unchanged. Checked: no file newer than `test-output.txt`; fmt/test (488 passed, 29 ignored, 0 failed)/clippy OK; `pw-loupe` 237, matching the test inventory. Waiting for the owner's hand check |
| 2026-11-10 | T66 | Done | Owner's hand check (TIFFs in grid and loupe, "TIFF-fil W × H" in both modes) and commit done (4bf5023). Next: the owner's acceptance D5 (ADR 0015 §8.2) |
| 2026-11-11 | D5 | Done | Owner's acceptance of ADR 0015 (§8.2): every check passed. The R check was worded unclearly: R switches the folder's display mode and its control even on a TIFF, while the image and "TIFF-fil" stay, as on a plain JPEG file; §8.2 reworded. `latency.ps1` (label `t66-tiff-next`, RAW mode, the 9 TIFFs alone in `D:\PhotoWorkflowData\TIFF`, RIGHT × 8, 1.5 s apart): first change 15.5–30.7 ms, median 16.1 ms, one change each, so V1 and V2 are met for prefetched TIFFs (E5). Results in ADR 0015 §7. Next: documentation D6–D7 |
| 2026-11-11 | T67 | Open | Added: ADR 0015 D6–D7, documentation after milestone 6 (ADR 0015, 0004, 0009, 0007 status line, architecture.md, GR8, both READMEs). GR8: no feature of the Tauri app is left; IPTC parity through the import template; colour management named as an open owner question |
| 2026-11-11 | T67 | Ready for review | (Agent) Documentation only. ADR 0015: "Implemented (2026-11-11)" line after the status (D1–D4 = T63–T66, D5 accepted by the owner, D6–D7 this update); §9 D6 and D7 "**Done:** T67"; §12 one new point: ICC colour management is not applied, an open owner question before the switch (§9.1); §7 and §9.1 unchanged. ADR 0004 addendum "viewing TIFF": the 2026-11-11 egui note with the six differences. ADR 0009: §3.1 "TIFF since ADR 0015", §3.2 "TIFF viewing (done: ADR 0015, 2026-11-11)." ADR 0007: one status item after the first ("Planned as milestone 7 …"). architecture.md crate table: new `pw-tiff` row after `pw-thumbcache`, TIFF notes in `pw-thumbcache`, `pw-viewcore`, `pw-loupe`. GR8 status 2026-11-11 as written in the task. Root `README.md`: TIFF viewing listed, "until the switch (GUI_REQUIREMENTS GR8)". docs/README.md: 0015 "done 2026-11-11". `docs/test-inventory.md` unchanged (matches: 488 passed, 29 ignored, 0 failed). `.\tools\check.ps1`: fmt, test (488 passed, 29 ignored, 0 failed) and clippy OK. No commit |
| 2026-11-11 | T67 | Ready for review | Review: accepted. Only the named files changed, each in the named lines: ADR 0015 "Implemented" line, D6–D7 results and the colour point in §12 (§7 and §9.1 untouched); ADR 0004 note with the six differences; ADR 0009 §3.1 row and §3.2 item; ADR 0007 one status line, still "proposed"; architecture.md new `pw-tiff` row and three short additions, diagram unchanged; GR8 status as written in the task; root README "until the switch"; docs/README "done 2026-11-11". Test inventory unchanged (no code change). `check.ps1` OK. **Milestone 6 (ADR 0015) is complete** once committed |
| 2026-11-11 | T68 | Open | Added: ADR 0016 C1, new crate `pw-colour` (moxcms =0.8.1, approved in C0) with profiles, transforms, a transform cache and reference-value tests (F1, F3, F4). The reference values were computed independently of moxcms by the reviewer |
| 2026-11-11 | T68 | Ready for review | (Agent) New crate `pw-colour` (`Cargo.toml` as `pw-tiff`'s, `moxcms = "=0.8.1"` with default features); root `Cargo.toml`: `pw-colour` under "Internal crates" after `pw-tiff`. `lib.rs`: `Profile` (`srgb`, `adobe_rgb`, `pro_photo_rgb`, `from_icc`: parse error `Profile`, non-RGB `Unsupported`), private `ProfileId` (ICC by its bytes; `Debug` prints only the length), `Transform` (8- and 16-bit moxcms transforms, `Layout::Rgb`, relative colorimetric; same `ProfileId` = identity, nothing built, data untouched; `convert_rgb8`/`convert_rgb16` in place in chunks of 65 536 pixels through one scratch `Vec`; length not a multiple of 3 = `Length`, checked before the identity shortcut too), `TransformCache` (`Mutex<HashMap>`, built under the lock, failures not cached, poisoned lock via `into_inner`), `ColourError` with the four English texts (each `String` holds the detail; `Display` adds the text, `Unsupported` holds the `{:?}` of the colour space, e.g. "Gray"). If moxcms fails on a later chunk, earlier chunks are already converted (documented). `tests.rs`: 13 tests; every reference value within 1 (no tolerance widened). The cache failure test builds a gray `Profile` by hand, since `from_icc` refuses gray. `Cargo.lock`: only the new `pw-colour` package with `moxcms`; no new external package. Test inventory: `pw-colour` row (13), totals 501/29. `.\tools\check.ps1`: fmt, test (501 passed, 29 ignored, 0 failed) and clippy OK. No commit |
| 2026-11-11 | T68 | Ready for review | Review: accepted. Only the named files changed; `Cargo.lock` gains only the `pw-colour` package (no new external package). `lib.rs` follows points 1–6: no panic paths, the length checked before the identity shortcut, chunks of 65 536 pixels through one scratch buffer, relative colorimetric, failures not cached. 13 tests cover every point under **Tests to add**; every reference value of F1 passes within 1 with no tolerance widened, so `moxcms` stays (ADR 0016 Q5). The gray failure pair is built by hand inside the crate, since `from_icc` refuses gray: accepted. Notes for later steps, not faults: the cache builds under its lock (builds are serialised; fine for a handful of profiles), and a `moxcms` error in a later chunk leaves earlier chunks converted (only a length mismatch makes it fail, which `check_length` rules out). Reviewer re-ran `check.ps1`: fmt, test (501 passed, 29 ignored, 0 failed) and clippy OK. Next: C2 and C3 (ADR 0016 §9) |
| 2026-11-11 | T69–T71 | Open | Added: ADR 0016 C2 (T69, monitor profile at start, window title) and C3 split in two (T70 TIFF ICC and 16-bit path; T71 JPEG ICC and Exif colour space). Before writing them the reviewer confirmed on the owner's CX240 that egui passes texture values through unchanged (ADR 0016 §1.1) and that `GetICMProfileW` on the primary screen returns the per-user CX240 profile (§4.1). ADR 0016 §5 refined: no info panel exists, so the profile is named in the window title. The three tasks are independent |
| 2026-11-11 | T69 | Ready for review | (Agent) `pw-loupe/Cargo.toml`: `pw-colour.workspace = true`; `[target.'cfg(windows)'.dependencies]` `windows-sys = "=0.61.2"` with the three features; `[dev-dependencies]` `moxcms = "=0.8.1"`. New `src/monitor_profile.rs` (exported from `lib.rs`): `ProfileSource`, `MonitorProfile`, `load_monitor_profile` (read + `Profile::from_icc`; `File(file name)`, `Missing`, else `Unusable(detail)`; sRGB in every case but `File`), `primary_monitor_profile_path` (`MonitorFromPoint` (0,0, primary) → `GetMonitorInfoW` (`MONITORINFOEXW`) → `CreateDCW(szDevice, szDevice, null, null)` → `GetICMProfileW` for the length (0 = `Ok(None)`) and into a buffer of that length; `DeleteDC` through a drop guard on every path after `CreateDCW`; `Err` names the failing call, with `GetLastError` where it applies; `// SAFETY:` on each `unsafe`; `Ok(None)` off Windows), `window_title` with the three texts. `main.rs`: profile read before `ViewCore::start`, viewport `.with_title(window_title(..))`, app name stays `WINDOW_TITLE`, new field `monitor_profile` with `#[allow(dead_code)] // used by ADR 0016 C4`. 5 new tests in the library. `Cargo.lock`: only `moxcms`, `pw-colour` and `windows-sys 0.61.2` in `pw-loupe`'s list; no new package. Test inventory: `pw-loupe` 242, totals 506/29. `.\tools\check.ps1`: fmt, test (506 passed, 29 ignored, 0 failed) and clippy OK. Not run: the title check by hand. No commit |
| 2026-11-11 | T69 | Ready for review | Review: accepted. Only the named files changed; `Cargo.lock` gains only `moxcms`, `pw-colour` and `windows-sys 0.61.2` in `pw-loupe`'s list (no new package). `monitor_profile.rs` follows points 1–3: the Windows lookup is the sequence checked on the owner's machine, with a drop guard for `DeleteDC`, the NUL check on `szDevice`, `// SAFETY:` on every `unsafe`; every failure falls back to sRGB. The 5 tests cover the list. Reviewer re-ran `check.ps1` (fmt, test 506 passed, 29 ignored, 0 failed, clippy OK), built the workspace and started RawStride: the window title read back was "RawStride  ·  Skärmprofil: CX240(…)08CAL (DP).icc". Note for C4: on a machine without a monitor profile Windows may return its default `sRGB Color Space Profile.icm`, so `File` can name an sRGB profile; C4 converts sRGB → that profile (near identity, within 1), which is correct but costs time. Owner: only the visual check is left (images unchanged) |
| 2026-11-11 | T70 | Ready for review | (Agent) `pw-tiff/Cargo.toml`: `pw-colour.workspace = true`; `[dev-dependencies]` `moxcms = "=0.8.1"`. `convert.rs`: `to_rgb16` (crate-private; RGB and RGBA only, RGB of exact length without a copy, the same "image data is short" text as `to_rgb8`). `lib.rs`: `SourceColour { Embedded, AssumedSrgb, Unusable(String) }`, `decode_rgb8_for_display` under the decode lock (tag 34675 read for RGB/RGBA only; Gray, GrayAlpha, CMYK = sRGB, `AssumedSrgb`; parse/`Unsupported`/unreadable tag = `Unusable` with sRGB; a failing `cache.get` = `Unusable` with sRGB as source; 16-bit RGB/RGBA: `to_rgb16` → `convert_rgb16` → `u16_to_u8`; 8-bit, float and 16-bit Gray/CMYK: today's path → `convert_rgb8`; a conversion error keeps the image, `Unusable`). `decode_rgb8` unchanged. **Deviation to review:** `tiff` 0.9.1 `into_u8_vec` reads only type UNDEFINED; a BYTE tag (what `write_tag(&bytes[..])` in the listed tests writes) comes back as `UnsignedBig` values, so a small helper `icc_bytes` takes those one by one and uses `into_u8_vec` otherwise. 12 new tests (11 in `tests.rs`, one of them ignored real-file; `rgb16_conversions` in `convert.rs`); ramp: 20 distinct values via 16 bit, 13 reduced first. Real files (`--ignored`, `PW_TESTDATA`): all 9 TIFFs `Embedded`. `Cargo.lock`: only `moxcms` and `pw-colour` in `pw-tiff`'s list; no new package. Test inventory: `pw-tiff` 31/1, totals 517/30. `.\tools\check.ps1`: fmt, test (517 passed, 30 ignored, 0 failed) and clippy OK. No commit |
| 2026-11-11 | T70 | Ready for review | Review: accepted. Only the named files changed; `Cargo.lock` gains only `moxcms` and `pw-colour` in `pw-tiff`'s list. `decode_rgb8` untouched. **The deviation is accepted:** checked in `tiff` 0.9.1 (`decoder/ifd.rs`): a BYTE tag longer than 4 bytes is decoded as `UnsignedBig` values, which `into_u8_vec` refuses, so `icc_bytes` is needed for the test files; real files use UNDEFINED and take `into_u8_vec`. The layout rules, the 16-bit path (`to_rgb16` → `convert_rgb16` → `u16_to_u8`), the fallbacks to sRGB and the tests follow the task; the hand-built UNDEFINED file passes; the ramp gives 20 distinct values via 16 bit against 13 reduced first. Reviewer re-ran `check.ps1` (fmt, test 517 passed, 30 ignored, 0 failed, clippy OK) and the ignored real-file test on `D:\PhotoWorkflowData\TIFF`: all 9 Lightroom TIFFs `Embedded`, decode and conversion to sRGB of all nine (3201×2134 to 7741×5161) in 1.78 s together (debug profile with optimised dependencies). Input for C4: the conversion runs under the decode lock, so it adds to the time thumbnails wait |
| 2026-11-11 | T71 | Ready for review | (Agent) `pw-libjpeg/src/lib.rs`: `tj3Set`, `tj3GetICCProfile`, `tj3Free` declared; `TJPARAM_SAVEMARKERS = 25` (entry 25 of `enum TJPARAM` in the vcpkg `turbojpeg.h` 3.2.0, checked); `new` sets it to 2 (a failing `tj3Set` is an error, the handle destroyed); `icc_profile`: `header`, then `tj3GetICCProfile`; null or size 0 = `Ok(None)` (without a profile TurboJPEG returns -1 as a warning), a fatal error = `Err`; the buffer is freed by a guard (`TjBuffer`, `tj3Free`) on every path; `decode` unchanged. **Deviation approved by the owner during the task:** libjpeg-turbo 3.2.0 never clears an extracted profile (`turbojpeg.c` replaces `decompICCBuf` only when the next header has one), so a reused decoder would report an earlier JPEG's profile for a JPEG without one, also after `decode`. `icc_profile` first asks for the size only (`tj3GetICCProfile(handle, NULL, &size)`, no allocation) and replaces the instance with a fresh one when a profile is held; one extra test. The task's `#[path]` needed one more `..` (an inline `tests` module resolves from `src/tests/`). `pw-raw`: `ExifColourSpace` and `ImageInfo::colour_space`; ColorSpace (SHORT only) and the Interop IFD read in `read_exif_ifd` (so also from a JPEG's APP1); unreadable Interop IFD = not "R03"; `jpeg.rs` unchanged. Tests: 6 in `pw-libjpeg` (5 listed + the reuse test), 8 in `pw-raw` (7 listed, the 0xFFFF/R98 and other-value cases included; 1 ignored real-file in `src/lib.rs`, since `tests/real_files.rs` is not in the file list). Real files (`D:\PhotoWorkflowData\RAW`): Sony ARW and JPG `Srgb`, `_TN0633x` ARW and JPG `AdobeRgb`, Pixel DNG `Uncalibrated`, RW2, `TN0025xx.DNG` and TIFF `Unknown`. Test inventory: totals 530/31. `Cargo.toml` and `Cargo.lock` unchanged. `.\tools\check.ps1`: fmt, test (530 passed, 31 ignored, 0 failed) and clippy OK. No commit |
| 2026-11-11 | T71 | Ready for review | Review: accepted. Only the named files changed; `Cargo.toml` and `Cargo.lock` unchanged. `pw-libjpeg`: the FFI declarations match `turbojpeg.h` 3.2.0 (`TJPARAM_SAVEMARKERS` = 25 recounted); the buffer is freed by a guard on every path. **The extra instance reset is accepted:** checked in the vcpkg source (`buildtrees/libjpeg-turbo/…/turbojpeg.c`, `tj3DecompressHeader`): `decompICCBuf` is replaced only when the new header has a profile and is never cleared, so without the reset a reused decoder would report an earlier JPEG's profile; the reuse test pins it. The `#[path]` with one more `..` is correct for an inline module. `pw-raw`: ColorSpace (SHORT) and the Interop IFD in `read_exif_ifd`, so RAW and JPEG APP1 both work; an unreadable Interop IFD is not "R03". Reviewer re-ran `check.ps1` (fmt, test 530 passed, 31 ignored, 0 failed, clippy OK) and the real-file print on `D:\PhotoWorkflowData\RAW`: Sony ARW/JPG `Srgb`, `_TN0633x` ARW/JPG `AdobeRgb`, Pixel DNG `Uncalibrated`, RW2, `TN0025xx.DNG` and TIFF `Unknown`, as the agent reported. **Input for C4:** none of the five camera JPEGs has an APP2 ICC profile, so the three Adobe RGB shots (and their ARW previews) are only recognised through the Exif marker; no JPEG with an embedded profile exists in the set yet (ADR 0016 Q9) |
| 2026-11-11 | T72 | Open | Added: ADR 0016 C4, conversion to the monitor profile in the loupe (TIFF through `decode_rgb8_for_display`, JPEG through its ICC or the Exif Adobe RGB marker, RAW from sRGB) and the thumbnails; `jpeg_source_profile` in `pw-colour`. T69–T71 committed (Done). The status-line note for an unusable embedded profile (ADR 0016 §5) is left for a later task, so the loupe core's interface does not change here |
| 2026-11-11 | T72 | Ready for review | Agent: `jpeg_source_profile` in `pw-colour`; `NativeBackend` holds the destination and a shared `TransformCache` (`Clone`, no longer `Copy`; nothing needed `Copy`), `with_monitor_profile`; TIFF through `decode_rgb8_for_display`, JPEG converted from its ICC / Exif marker (the full decode converted once), RAW sRGB → destination; thumbnails likewise, `ThumbWorker::start_with_monitor_profile`; `pw-loupe` passes the monitor profile to both (the `monitor_profile` field is still not read, so its `allow(dead_code)` stays). With the owner's approval: `moxcms` =0.8.1 also under `pw-viewcore` `[dev-dependencies]` (ICC bytes for the `native.rs` tests), and the existing `thumbnail(…)` test calls got the two sRGB arguments (no assertion changed). A source profile no transform can be built from falls back to sRGB (as `pw-tiff`). Read the "same TIFF with the default backend" test as: the ProPhoto TIFF with the default (sRGB) backend equals `decode_rgb8_for_display`, a TIFF without ICC equals `decode_rgb8`. `check.ps1`: fmt OK, test 542 passed, 32 ignored, 0 failed, clippy OK; real-file tests of `native_files.rs` and `pw-thumbcache` run with `PW_TESTDATA` and pass |
| 2026-11-11 | T72 | Ready for review | Review: accepted. Only the named files changed, plus two deviations the agent reports as approved by the owner: `moxcms` as a dev-dependency of `pw-viewcore` (test ICC bytes; no new package in `Cargo.lock`) and the existing `thumbnail(…)` test calls given the two sRGB arguments the new signature needs (no assertion changed). `native.rs`: TIFF through `decode_rgb8_for_display`; JPEG from `jpeg_source_profile` (APP2 ICC, Exif marker, sRGB), the full decode converted once; RAW sRGB → destination on every render; a source with no buildable transform falls back to sRGB. `extract.rs`/`worker.rs`: the same rules, one shared `TransformCache`. Reviewer re-ran `check.ps1` (fmt, test, clippy OK) and the ignored real-file tests in release on `D:\PhotoWorkflowData\RAW` (all pass): `TN100423.ARW` render_half 4784×3188 359 ms to sRGB and 312 ms to Adobe RGB, embedded JPEG 808×540 8.3 / 8.7 ms, so the conversion is lost in the noise of the render. The owner's new DxO PhotoLab exports: `TN005701_DxO.jpg` has an APP2 ICC "Adobe RGB (1998)" (560 bytes); `TN005702_DxO.jpg` has none (sRGB). Next: the owner's check by hand (ADR 0016 C5) |
| 2026-11-11 | C5 | — | Objective check of F5 instead of Lightroom (the owner has no Lightroom): the eight-patch sRGB TIFF of the passthrough check shown in RawStride with T72 (release build) and read back from a screenshot of the CX240. Expected values computed independently of moxcms from the CX240 profile (sRGB D50 matrix, the profile's matrix inverted, its 1024-point TRC tables inverted): all eight patches within 1, seven exactly (sRGB red: expected 216,64,31, shown 216,63,30). Before T72 the same patches were shown unchanged (255,0,0 etc.). The visual side-by-side check uses DxO PhotoLab instead of Lightroom |
| 2026-11-11 | C5 | Done | The owner accepted T72 and the C5 check by hand ("godkänt"). Still open: F6 with `latency.ps1` on the TIFF and RAW sets; the status-line note for an unusable embedded profile (ADR 0016 §5); the documentation step C6 |
| 2026-11-11 | T73–T74 | Open | Added: T73, the loupe's status-line part "Inbäddad färgprofil kunde inte användas" (ADR 0016 §5, text refined by the reviewer: no "visas som sRGB", since the Exif marker can take over); T74, documentation step C6 (after T73). T72 committed (Done) |
| 2026-11-11 | T73 | Ready for review | Agent: `pw-colour::jpeg_source_profile_checked` (`jpeg_source_profile` now wraps it, behaviour and tests unchanged); `Backend::profile_unusable` with default `false`; the worker asks it after a successful open through a new `State::finish_open_checked` (`finish_open` keeps its signature for the existing tests, delegates with `false` and is now `#[cfg(test)]`, since only tests call it); `State::profile_unusable: HashSet<Key>` cleared in `open_folder` and moved by `set_files` like `sizes`; `Frame::profile_unusable`. `native.rs`: TIFF flag from `SourceColour::Unusable`, kept with the image in a new `TiffSession` struct (`NativeSession::Tiff(TiffSession)`, so the existing `NativeSession::Tiff(_)` patterns compile unchanged); JPEG from `jpeg_source_profile_checked` or the sRGB fallback of `transform_checked`; RAW `false`. `pw-loupe::status_line`: `PROFILE_UNUSABLE_TEXT` after the source text when flagged (not for "kan inte visas" or "förbereder …"). Existing test code changed only by `profile_unusable: false` in the two `Frame` literals. 14 new tests (pw-colour 4, native.rs 4, state.rs 3 with a simulated `profile_unusable` true for `img10.raw`, pw-loupe 3). `Cargo.toml`/`Cargo.lock` unchanged. `check.ps1`: fmt OK, test 556 passed, 32 ignored, 0 failed, clippy OK. No commit |
| 2026-11-11 | T73 | Ready for review | Review: accepted. Only the named files changed; `Cargo.lock` unchanged. `jpeg_source_profile_checked` with `jpeg_source_profile` as its wrapper; `Backend::profile_unusable` with default `false`; the worker passes it to `State::finish_open_checked`, and the flag set is cleared and moved like `sizes`. Accepted deviation: `finish_open` kept with its signature for the existing state tests, delegating with `false`, and made `#[cfg(test)]` because only tests call it. `native.rs`: the TIFF flag in a small `TiffSession`, JPEG from the checked rule or the sRGB fallback, RAW never. `status_line` adds the part after the source text only when the source text is shown. Existing test code changed only by `profile_unusable: false` in the two `Frame` literals. Reviewer re-ran `check.ps1` (fmt, test 556 passed, 32 ignored, 0 failed, clippy OK). For the owner's check: `D:\PhotoWorkflowData\ICC-test` holds `TN005701_DxO_trasig-profil.jpg` (the DxO Adobe RGB export with the ICC signature and colour space overwritten) and `TN005702_DxO.jpg` (no profile) |
| 2026-11-11 | F6 | — | The owner's first `latency.ps1` run (labels `c4-tiff-next`, `c4-raw-next`) is **not valid**: every press has `changes` 0 and no `first_ms`, i.e. no screen change was seen. RawStride was started with a folder, which opens the grid, not the loupe; to be repeated with a file path (the loupe) |
| 2026-11-11 | F6 | — | Repeated run, RawStride started on a file (loupe), release build with T72–T73. **TIFF** (`c4-tiff-next-2`, the 9 TIFFs, RIGHT × 8, 1.5 s): first change 16.4–30.6 ms, **median 17.4 ms**, one change each (T66 before colour management: median 16.1 ms): V1 met. **RAW folder** (`c4-raw-next-2`, from `TN100423.ARW`, RIGHT × 20): the four `TN1004xx` A7R IV steps 16.7–26.7 ms, but the `_TN0633x` ARW/JPG pairs (copied to the folder today, never read before) took 0.6–6.7 s, and three presses changed nothing. Not yet attributed: the T72 render timings show the conversion in the noise, and the slow files were cold in the file cache; a second run on warm files decides |
| 2026-11-11 | F6 | Done | **Corrects the previous row.** The `c4-raw-next-2` run started near the end of the folder: after four `TN1004xx` steps it reached the `_TN0633x` files and then the end of the list, so it does not measure stepping. The owner re-ran it on 20 RAW files (label `c4-raw-next`, the **last 20 rows** of `latency-rawstride.csv`; the first 20 rows with that label are the invalid grid run): first change 13.7–17.5 ms, **median 17.0 ms**, one change each. With TIFF median 17.4 ms (`c4-tiff-next-2`), **F6 is met**: V1 (median ≤ 30 ms) holds with colour management on |
| 2026-11-11 | T73 | Done | Owner's check by hand passed: `TN005701_DxO_trasig-profil.jpg` shows "Inbäddad färgprofil kunde inte användas" in the loupe status line, `TN005702_DxO.jpg` does not. Committed (3b06d4f). T74 can start |
| 2026-11-11 | T74 | Ready for review | (Agent) Documentation only. ADR 0016: "Implemented (2026-11-11)" line after the status (C1–C4 = T68–T72, the status-line note = T73, C5 accepted by the owner, C6 = T74); §7 results table F1–F6 from the log rows of T68–T73, C5 and F6 (F6: TIFF median 17.4 ms, RAW median 17.0 ms, V1 met); §9 C6 "**Done:** T74"; §12 one point: milestone 7 complete. The §9 C5 row still says "F6 (`latency.ps1`) not yet measured" (not in the task's list, left unchanged). ADR 0015: "(moved to milestone 8 by ADR 0016 Q1, 2026-11-11)" in §2, §9 D7 and §12; "(done: ADR 0016, 2026-11-11)" in §3.2 and the §12 colour point. ADR 0007: status line milestone 8, still "proposed". architecture.md: new `pw-colour` row after `pw-tiff`; additions in `pw-raw`, `pw-tiff`, `pw-thumbcache`, `pw-loupe`, `pw-viewcore`; **the table had no `pw-libjpeg` row, so a new one was added after `pw-raw` (owner's answer during the task)**; diagram unchanged. GR8 status as written in the task. docs/README.md: 0016 "done 2026-11-11". Root `README.md`: colour management listed. `docs/test-inventory.md` unchanged (no code change). `.\tools\check.ps1`: fmt, test (556 passed, 32 ignored, 0 failed) and clippy OK. No commit |
| 2026-11-11 | T74 | Done | Review: accepted. Only the named files changed, each in the named lines; the results table of ADR 0016 §7 matches the log rows (F1–F6). The new `pw-libjpeg` row in architecture.md is accepted: the table had none, and the task asked for an addition there. The reviewer updated the stale "F6 not yet measured" in ADR 0016 §9 C5 (F6 met). `check.ps1` OK. **Milestone 7 (ADR 0016, colour management) is complete**; IPTC (ADR 0007) is milestone 8 |
| 2026-11-11 | T75 | Open | Added: ADR 0009 S6 (acceptance measurement and report) was never done: no log row, no `analysis\s6`, no report. Owner's runs S6a with `latency.ps1` against "RawStride" (started on a file), labels `s6-…`; then T75, the report, ADR 0009 status/§9 and the GR8 sentence. S7 (TESTING.md) follows separately |
| 2026-11-11 | S6a | — | Owner's runs with `D:\PhotoWorkflowData\pm-study\tools\s6a.ps1` (release build, RawStride started on the first file of `D:\PhotoWorkflowData\s6\<camera>`, maximised, restarted before each run; folders a7r 5, a1 14, rw2 21, dng 9 files). Rows in `latency-rawstride.csv`, labels `s6-…`; summaries in `D:\PhotoWorkflowData\analysis\s6\s6a-log.txt`. Every press in runs 1–10 had at most one screen change; no run invalid. Times are `first_ms` median / max |
| 2026-11-11 | S6a | — | Run 1 (next at fit, RAW), **run twice** (14:33–14:37 and 14:42–14:44; both valid, same labels): a7r n=4 15.6/16.0 and 16.8/17.0; a1 n=13 16.7/17.5 and 16.8/17.4; rw2 n=20 16.8/17.8 and 16.8/17.3; dng n=8 16.8/17.2 and 16.8/17.0 ms. Process private memory up to 5087 MB (a1, first round) and 3007 MB (a1, second round) |
| 2026-11-11 | S6a | — | Run 2 (next at fit, JPEG), **run twice** (14:38–14:41 and 14:45–14:47): a7r 17.0/17.4 and 16.9/17.2; a1 16.8/17.8 and 16.9/17.3; rw2 16.8/17.2 and 16.9/17.3; dng 16.8/17.1 and 16.8/17.5 ms. Memory up to 5111 and 5157 MB on a1 in JPEG mode as well |
| 2026-11-11 | S6a | — | Run 3 (next at 100 %, a7r, n=4): RAW 17.2/17.8 ms, JPEG 16.6/16.9 ms |
| 2026-11-11 | S6a | — | Run 4 (Z after a 3 s pause, RAW, n=10; odd presses fit → 100 %): a7r fit → 100 % 82.6, 15.8, 16.5, 16.5, 17.0 ms (median 16.5), back to fit median 16.9, max 19.7; rw2 fit → 100 % 98.9, 16.2, 16.7, 16.2, 16.7 ms (median 16.7), back median 16.1. The slow press is the first one in both |
| 2026-11-11 | S6a | — | Run 5 (RIGHT then Z, RAW, rw2, n=10; one change per press, so `settled_ms` = `first_ms`): delay 0 ms: fit → 100 % 81.9, 16.3, 33.2, 33.0, 32.1 (median 33.0); delay 50 ms: 99.8, 17.0, 83.9, 16.8, 17.0 (median 17.0); delay 150 ms: 231.4, 17.1, 17.3, 17.2, 17.2 (median 17.2) ms. Again the first press is the slowest |
| 2026-11-11 | S6a | — | Run 6 (Z, JPEG, a1, n=10): fit → 100 % 51.8, 16.4, 16.7, 16.9, 16.0 ms (median 16.7); back median 16.7 ms |
| 2026-11-11 | S6a | — | Run 7 (about 7 images/s at 100 %, rw2, n=20, `-Quiet 1 -Timeout 70 -Gap 70`): **RAW: 16 of 20 presses showed no change within 70 ms** (the 4 others 15.9–22.6 ms), the known limit of ADR 0009 §8.3; **JPEG: all 20 within 70 ms**, median 50.0, max 51.0 ms (values 16.9–51.0) |
| 2026-11-11 | S6a | — | Run 8 (R, RAW start): on a shown image (a7r, n=6) median 17.1, max 21.1 ms; right after RIGHT (rw2, n=6) median 32.3, max 47.8 ms |
| 2026-11-11 | S6a | — | Run 9 (free zoom from fit, anchor at the centre, a7r, n=5 each): + median 16.7, max 17.1 ms; − median 16.6, max 16.9 ms; one change per press |
| 2026-11-11 | S6a | — | Run 10 (LEFT after RIGHT × 3, RAW, a7r, n=3): 17.0/17.0 ms |
| 2026-11-11 | S6a | — | **Runs 11–13 deferred by the owner.** A first attempt at run 12 (15:02, about 30 s) was not done as intended (no long session) and is not used; `mem.csv` from it stays as a record. Not measured: V5, V8 over a long session (L1–L4), and the hand checks V6, G1, G3, G4, GR6, GR7. `s6a.ps1` was updated for these runs (run 11 measured by the script, run 12 with a timer, run 13 as a checklist); `-Run 11,12,13` runs them later. T75 can start; it reports these as not measured |
| 2026-11-11 | T75 | Ready for review | Report `docs/analyses/2026-11-11-adr0009-s6-acceptance.md` from the `s6-` CSV rows, the S6a and F6 log rows and `analysis\s6`: V1 and V2 met; V3 (first Z, JPEG A1 51.8 ms), V4 (RAW: 16 of 20 steps over 70 ms) and V7 (first Z 81.9–231.4 ms) not met; V5, V6, V8, G1–G4, GR6, GR7 not measured (runs 11–13 deferred). ADR 0009 status line and §9 S6 "Partly done"; GR8 sentence added. The status line says "S6 partly done" instead of "S6 done", to match the deferral |
| 2026-11-11 | T75 | Ready for review | Review: accepted. Only the named files changed (the report, ADR 0009 status line and §9 S6, the GR8 sentence; this file: the heading status and one log row); `test-output.txt` newer than the changes, fmt/test/clippy OK. Every median, 95th percentile, maximum and n recomputed from the `s6-` CSV rows: all match (medians within 0.05 ms, rounded as in `s6a-log.txt`); the P7 figures match ADR 0008 §18.3. Accepted deviation: the ADR 0009 status line says "partly done", as the deferral requires. The reviewer corrected three lines of the report: G2 was not in the S6a plan (not "in run 13"); the number of screens is not recorded in S6a; `s6-zoom-minus` was not preceded by a restart. Noted, not changed: the F6 log row gives the TIFF median as 17.4 ms, the CSV gives 17.25 ms |
| 2026-11-11 | T76–T79 | Open | Added: milestone 8 (ADR 0007, accepted 2026-11-11, revision 9). Step plan P1, I1–I13 above T76. T76 the property table generated from the IPTC TechReference 2025.1; T77 dates and times; T78 the value model (after T76 and T77); T79 IIM text rules. P1 done the same day (the JSON saved in `D:\PhotoWorkflowData\references\`, 140 839 bytes, 66 properties); T76, T77 and T79 can start now. Tasks for I5–I13 are written after T76–T79 are accepted |
| 2026-11-11 | T76 | Ready for review | `tools/iptc-table.ps1` generates `crates/pw-metadata/src/iptc_table.rs` from `iptc-pmd-techreference_2025.1.json` (SHA-256 `592c37b01fe02c7333cd7059a212bdceee13c1147ad7bcc6dbe996ba433128a8`): 66 properties, 18 structures; two runs byte-identical, `cargo fmt --all` leaves it unchanged (no `#[rustfmt::skip]`); stops on a bad kind, occurrence or schema without writing. `properties.rs` with the types, lookups, `list_kind` (Seq only for the four named; all 27 `Multi` properties checked against spec §7/§11), `max_entries`, `xmp:Label` and the three deprecated IIM entries; 11 tests. `ipmdschema` is checked for `ipmd_top` only (structure fields carry `PLUS`, and `FieldDef` has no schema). `check.ps1` OK: 579 passed, 0 failed, 32 ignored, which includes T77's `date.rs` tests (another agent in the same working tree); the 4 clippy warnings are all in `date.rs` |
| 2026-11-11 | T77 | Ready for review | New `crates/pw-metadata/src/date.rs` (`pub mod date`): `IptcDate`, `Time`, `Zone`, `DateError`; `parse_xmp`/`to_xmp` (exact round trip), `to_iim`/`from_iim` (2:60 only with a known zone), `from_exif` (read only, no zone without an offset). 12 tests; `pw-metadata` unit tests 44 (1 ignored), workspace 579 passed, 32 ignored, 0 failed (with T76). Choices for the reviewer: `-00:00` is refused (it cannot be written back as the same text, and in RFC 3339 it means an unknown offset); a blank SubSecTimeOriginal or OffsetTimeOriginal counts as not given. T76 was done at the same time in the same worktree, so `git diff --stat` also shows its files |
| 2026-11-11 | T76 | Ready for review | Review: accepted. T76 and T77 ran at the same time in the same working tree; their files do not overlap, and the shared files (`lib.rs`, `docs/test-inventory.md`, this file) hold both changes correctly. Only the named files changed. The reviewer ran the script again: byte-identical `iptc_table.rs` (SHA-256 of the file `1974a269…2fac8534`); with an unknown `propoccurrence` it stops and writes nothing. The reviewer's `check.ps1`: fmt, test and clippy OK, 579 passed, 0 failed, 32 ignored. Accepted deviations: `ipmdschema` is only checked for `ipmd_top` (structure fields carry `PLUS`, and `FieldDef` has no schema); `photoshop:Urgency` is `Integer` (Adobe XMP). The reviewer corrected one line of the module comment of `properties.rs` (the SHA-256 is not in `docs/references.md`). Noted for T78/I5: the spec also gives `Seq` for three **structure fields** (`Iptc4xmpExt:AOCreator`, `AOCreatorId`, `rbVertices`); `list_kind` covers top-level properties only |
| 2026-11-11 | T77 | Ready for review | Review: accepted. Every point and test of the task is there; the same `check.ps1` run as T76. Accepted choices: `-00:00` is refused; a blank SubSecTimeOriginal or OffsetTimeOriginal counts as not given. Noted for I5/I6: an IIM date `00000000` reads as year 0, not as `Empty`; a date the reader cannot parse must be kept and shown with a warning, not dropped (§11). The test `writers_do_not_panic_on_values_built_by_hand` pins the text `-546:08` for an impossible offset; that only shows there is no panic |
| 2026-11-11 | T78 | Open | Task text changed before start, after the T76 review: can start now; point 6 checks fields inside structures at any depth; new point 8 `properties::field_list_kind` (Seq for `AOCreator`, `AOCreatorId`, `rbVertices`), so `properties.rs` is added to the files for that function only; tests for both |
| 2026-11-11 | T79 | Ready for review | New `pw-metadata::iim` (`is_blank`, `truncate_utf8`, `predict`, `strip_prefix`, `same_as_predicted`, `coded_charset`, `decode`, `UTF8_CODED_CHARSET`) with 13 unit tests; `lib.rs` the `mod` line; `docs/test-inventory.md`. IPTC §7.23 checked: 2:12 has 236 bytes and, unlike §7.16, no exception for the prefix, so the limit counts the whole value. `check.ps1`: 592 passed, 0 failed, 32 ignored |
| 2026-11-11 | T79 | Ready for review | Review: accepted. Only the named files changed (`iim.rs`, the `mod` line, `docs/test-inventory.md`; this file: the heading status and one log row). Every point and test of the task is there; the Windows-1252 table checked byte for byte; no `unwrap`/`expect` outside tests. The reviewer's `check.ps1`: fmt, test and clippy OK, 592 passed, 0 failed, 32 ignored (13 new `iim` tests). Accepted: the 2:12 limit counts the whole value (IPTC §7.23 has no prefix exception). **Open, an error in the task text, not in the work:** the task named only `ESC . A` (G2) as the 1:90 declaration of ISO 8859-1; the usual ISO 2022 form is `ESC - A` (G1). ExifTool 13.59 only knows `UTF8` for 1:90 and settles nothing. `ESC - A` should also be `Latin1`, and a 1:90 value with several escape sequences should be handled; the owner decides where (a small reviewer fix or part of I5/I6) |
| 2026-11-11 | T79 | Ready for review | Reviewer fix of the open point above (owner's choice, 2026-11-11): `coded_charset` splits 1:90 into its ISO 2022 escape sequences; `Latin1` is one or more of `ESC - A` (G1) and `ESC . A` (G2), optionally with `ESC ( B` (ASCII in G0), and nothing else; `Utf8` stays exactly `ESC % G`; everything else, including UTF-8 mixed with other sequences, is `Other`. Two tests added, no existing test changed. `check.ps1`: fmt, test and clippy OK, 594 passed, 0 failed, 32 ignored; `docs/test-inventory.md` updated |
| 2026-11-11 | T78 | Ready for review | New `pw-metadata::value` (`Origin`, `LangAlt`, `Value`, `StructValue`, `PropertyValue`, `Conflict`, `ConflictRule`, `Metadata`, `ValueError`) with 14 unit tests; `properties::field_list_kind` with 1 test; `lib.rs` the `mod` line; `docs/test-inventory.md`. Point 8 checked: of the 13 `Multi` structure fields, the spec's "XMP Specs" rows give `Seq` for exactly `AOCreator`, `AOCreatorId` and `rbVertices`, `Bag` for the other ten. Choices for review: a Lang Alt `set` takes the text the new value shows (`display`) and applies it with `set_default` to the existing value (without one, the new value's entries are the start); "only spaces" means the space character only (as T79 `is_blank`); `Number` must parse as a finite `f64` (`NaN`/`inf` refused); a `List` is not checked entry by entry (point 6 only asks for `List`). `check.ps1`: fmt, test and clippy OK, 609 passed, 0 failed, 32 ignored |
| 2026-11-11 | T78 | Ready for review | Review: accepted. Only the named files changed (`value.rs`, the `mod` line, `properties.rs` with `field_list_kind` and its test only, `docs/test-inventory.md`; this file: the heading status and one log row). Every point and test of the task is there, including the structure-field check at any depth with the `/` path and the two-level case; no `unwrap`/`expect`/`panic!` outside tests. `field_list_kind` matches the reviewer's search of the spec (exactly seven `Seq` in the "XMP Specs" rows: four top-level, three fields). The reviewer's `check.ps1`: fmt, test and clippy OK, 609 passed, 0 failed, 32 ignored (14 new `value` tests, 1 new `properties` test). Accepted choices: a Lang Alt `set` applies the new value's shown text to `x-default` of the existing value; "only spaces" means the space character (as T79); a `Number` must be finite. Noted for I5/I7: the entries of a `List` are not checked against the kind (e.g. `Iptc4xmpExt:ModelAge` is a list of integers); `Integer` and `Number` accept surrounding spaces (the text is kept as it is); `insert_read` replaces an earlier value of the same property, so I5 must reconcile before it inserts |
| 2026-11-11 | T80–T83 | Open / Blocked | Added after T76–T79 were done: T80 the IIM block (datasets, decoding, values); T81 the reconciliation (checksum cases, Exif, Creator, `Cleared` as an input); T82 the reader with ExifTool (**Blocked** on owner decision D2, `serde_json` in `pw-metadata`); T83 the write plan, checked against ExifTool. The step plan gains I5a/I5b and the owner decisions D1 (namespace of `Cleared`) and D2, and records the reviewer's ExifTool 13.59 measurements (raw IIM block, `CurrentIPTCDigest`, `-IPTCDigest=new`, the JSON form, ExifTool's date form, the Windows code page on the command line). **For the owner (T83 point 4):** an edit of a property with an `Open` conflict counts as the user's decision, so both forms are written; other `Open` conflicts keep the digest from being written (reviewer's reading of Q1). T80 can start now |
| 2026-11-11 | T82 | Open | Owner decision D2: `serde_json` = 1.0.154 as a dependency of `pw-metadata` approved. T82 is no longer blocked; it starts after T81 is accepted |
| 2026-11-11 | T80 | Ready for review | New `pw-metadata::iim_read` (`Dataset`, `parse_block`, `IimError`, `iim_format`, `IimValues`, `iim_values`, `IimNotice`) with 14 unit tests; `lib.rs` the `mod` line; `docs/test-inventory.md`. Values are checked with `Metadata::insert_read` on a scratch `Metadata` (`check_kind` is private to `value.rs`). Choices for review: a length error is reported at the offset of the length field, a header cut before it at the tag marker, and zero bytes followed by other bytes at the first zero byte; a value that is blank after removing the 2:04/2:12 prefix (`000:`, `IPTC:`) is non-existent like a blank one; a `00000000` 2:55 with a 2:60 gives `Unreadable` for the 2:60 (no date to put it on); a repeated 2:60 also gives `Repeated` for Date Created; a 2:60 decoded with `converted` gives `Converted`; `Unreadable` of a list joins its texts with `; ` (no IIM property reaches it). `check.ps1`: fmt, test and clippy OK, 623 passed, 0 failed, 32 ignored |
| 2026-11-11 | T80 | Ready for review | Review: accepted. Only the named files changed (`iim_read.rs`, the `mod` line, `docs/test-inventory.md`; this file: the heading status and one log row). Every point and test of the task is there; `parse_block` checks every length and end with `get`/`checked_add` and has no `unwrap`/`expect` outside tests; the random-byte test covers lengths 0–64 with a fixed seed. The reviewer's `check.ps1`: fmt, test and clippy OK, 623 passed, 0 failed, 32 ignored (14 new `iim_read` tests). Accepted choices: the error offsets as described; a value that is only a prefix (`000:`, `IPTC:`) is non-existent; a `00000000` 2:55 with a 2:60 reports the 2:60 as `Unreadable`; `Repeated` for a repeated 2:60; values checked through `insert_read` on a scratch `Metadata`. Noted for T81/I12: IIM text is kept exactly as decoded, so an old tool's trailing NUL or space (`Lund\0`) would differ from XMP `Lund` and give a conflict; if the level-C samples show such values, the equality of T81 may need to ignore trailing NUL/space |
| 2026-11-11 | T81 | Ready for review | New `pw-metadata::reconcile` (`FileKind`, `IimDigest`, `IimRead`, `ExifFields`, `Readings`, `Notice`, `Reconciled`, `reconcile`, `same_value`, `split_artist`) with 42 unit tests; `lib.rs` the `mod` line; `docs/test-inventory.md`. MWG §5.7 read in `mwg_guidance.pdf`: it agrees with point 5 (separator `"; "`; quotes needed for a name with `"; "` or a leading quote; doubled quotes only inside quotes), and both MWG examples are a test. Choices for review: (1) `same_value` keeps the one signature for IIM and Exif by the shape of `b`: the Exif side is passed as `Text` (the Exif text, or the first name for Creator), so `LangAlt`/`List` against `Text` is the Exif rule (shown text, first creator) and same-shape pairs are the IIM rule; (2) Text/Number/list items are compared on `predict` with the prefix removed again from the prediction, because T80 reads 2:04/2:12 without it; a Lang Alt is compared by its shown text without prediction, as written; (3) camera XMP against the sidecar is compared exactly (`==`); against a value filled from the RAW's IIM it uses `same_value`, and a difference replaces the IIM value (XMP over IIM, rule `Xmp`), a case the task does not name; (4) a blank value is a blank text/number, a Lang Alt or list of only blank entries; (5) `Cleared` is applied only for `Raw`; (6) an invalid DateTimeOriginal gives `Unreadable` with DateTimeOriginal, SubSecTimeOriginal and OffsetTimeOriginal as read, joined with a space. Trailing NUL/space in IIM still gives a conflict (T80 note). `check.ps1`: fmt, test and clippy OK, 665 passed, 0 failed, 32 ignored |
| 2026-11-11 | T81 | Ready for review | Review: accepted except one point. Only the named files changed (`reconcile.rs`, the `mod` line, `docs/test-inventory.md`; this file: the heading status and one log row); no `unwrap`/`expect`/`panic!` outside tests; every test of the task is there (42). The reviewer checked `split_artist` against MWG §5.7 (`mwg_guidance.pdf` p. 43): it agrees, and both MWG examples are tests. The reviewer's `check.ps1`: fmt, test and clippy OK, 665 passed, 0 failed, 32 ignored. Accepted choices (1)–(6) of the log row above; (3), the camera XMP replacing a value filled from the RAW's IIM, follows §5.1 (XMP over IIM). **Open, an error in the task text, not in the work:** point 4 said "a Lang Alt by its shown text", and the agent compares a Lang Alt against IIM without the prediction. §5.1 says equality is always tested on the predicted IIM value, so a Title over 64 bytes (2:05) or a Description over 2000 bytes would be a conflict every time, even right after RawStride wrote the file. A Lang Alt against IIM must compare the shown texts through the prediction, as `Text` does; the owner decides where it is fixed |
| 2026-11-11 | T81 | Open | **Sent back by the owner** (2026-11-11) for the open point above. Point 4 of the task text is corrected: a Lang Alt against IIM is compared by its shown text **through the prediction**. What is left, and nothing else: (a) in `same_value`, `(LangAlt, LangAlt)` compares `shown_text(a)` with `shown_text(b)` through `same_on_prediction`, as `Text` does; (b) tests: an XMP `dc:title` of 70 bytes against its IIM value cut to 64 bytes at a character boundary (with `å` so the cut is not at byte 64) is **not** a conflict for each checksum case; an XMP `dc:description` over 2000 bytes against its cut IIM value is not; a really different Title is still a conflict; the existing test `a_description_is_compared_by_its_shown_text` keeps passing unchanged. Keep everything else of T81 as it is; update the test count in `docs/test-inventory.md`; then `check.ps1`, the heading status and one log row as usual |
| 2026-11-11 | T81 | Ready for review | Fix after the send-back: (a) `same_value` compares `(LangAlt, LangAlt)` as `same_on_prediction(xmp_id, shown_text(a), shown_text(b))`, as `Text` does; the doc comment says so. The Exif rule (`LangAlt` against `Text`, exact shown text) is unchanged. (b) 3 new tests in `reconcile.rs`: `a_title_cut_to_the_iim_limit_is_not_a_conflict_for_any_checksum` (`dc:title` of 70 bytes, 63 × `a` + `å` + 5 × `b`, against its IIM value cut to 63 bytes; no conflict and XMP kept for `Matches`, `Differs` and `Missing`), `a_description_cut_to_the_iim_limit_is_not_a_conflict` (`ä` × 1001 = 2002 bytes against its value cut to 2000), `a_really_different_title_is_still_a_conflict` (`Solnedgång`/`Soluppgång`, and the long Title against 63 × `c`); `a_description_is_compared_by_its_shown_text` passes unchanged. Nothing else in T81 changed. `docs/test-inventory.md`: pw-metadata unit tests 130 → 133, total 668. `check.ps1`: fmt, test and clippy OK, 668 passed, 0 failed, 32 ignored |
| 2026-11-11 | T81 | Ready for review | Second review: accepted. The fix is exactly (a): `(LangAlt, LangAlt)` in `same_value` goes through `same_on_prediction`, with the doc comment updated; the Exif rule is unchanged, and the production code otherwise grew only by those lines (the test module starts 3 lines later). The three new tests cover (b): the 70-byte Title cut at byte 63 before `å` for `Matches`, `Differs` and `Missing`, the Description over 2000 bytes, and two really different Titles; `a_description_is_compared_by_its_shown_text` is unchanged. The reviewer's `check.ps1`: fmt, test and clippy OK, 668 passed, 0 failed, 32 ignored (45 `reconcile` tests) |
| 2026-11-11 | T82 | Ready for review | New `pw-metadata::read`: `ExifTool::read_metadata(image, fallback) -> MetadataRead { reconciled, file_notices }`, `FileNotice` (`ForeignSidecarIgnored`, `ExtSidecarIgnored`, `IimBlockUnreadable`), file kind by extension, the two ExifTool calls through `execute`, sidecar names compared without regard to case, XMP JSON → values by `et_xmp`/`et_tag` at any depth (Lang Alt in JSON order), IIM block with a small base64 decoder and the digest, Exif with `base64:` bytes and the Copyright NUL join; `IptcDate::parse_exiftool` in `date.rs`; `serde_json` =1.0.154 with `preserve_order` (adds `indexmap` 2.14.2 and `foldhash` to serde_json's list, both already in `Cargo.lock`: no new package). **Deviation approved by the owner during the task:** ExifTool prints every value that looks like a number as a bare JSON number (Headline `1984`, City `2.50` → `2.5`), so both calls also pass `-api structformat=jsonq` and values are built by the property's kind; a bare JSON number still becomes `Number`. A value that cannot be built is passed on as a structure with an unnamed field, which the model refuses for every kind (a text would be accepted by a text property). Note: `preserve_order` is unified for the whole workspace build, so `src-tauri`'s `serde_json` maps now keep insertion order (all tests pass). 16 unit tests (2 `date`, 14 `read`), 6 in `tests/read.rs` (1 ignored, real DNG; run with `--ignored` and passed). `check.ps1`: fmt, test and clippy OK, 689 passed, 0 failed, 33 ignored |
| 2026-11-11 | T82 | Ready for review | Review: accepted. The owner confirmed the deviation (`-api structformat=jsonq` in both calls); the reviewer checked it with ExifTool 13.59: without it Headline `1984`, City `2.50` and Keywords `7` come as bare JSON numbers (`2.50` printed as such but read as a number), with it every value is quoted. `StructFormat` `JSONQ` is a documented API option, so it is the right fix and no better one was found. `serde_json` with `preserve_order` adds no new package (`indexmap` 2.14.2 and `foldhash` were already in `Cargo.lock`). Only the files of the task changed. Notes for later tasks: `preserve_order` is unified for the whole workspace build; `find_sidecars` lists the whole folder for every image (a cost for I10 in large folders); a value that cannot be built is passed on as a structure with an unnamed field so that `reconcile` refuses it. The reviewer's `check.ps1`: fmt, test and clippy OK, 689 passed, 0 failed, 33 ignored; the ignored DNG test in `tests/read.rs` passed with `--ignored` |
| 2026-11-11 | T83 | Decision | Owner approved the reviewer's reading of Q1 in T83 point 4: an edit of a property with an `Open` conflict is the user's decision, so both forms are written and that conflict no longer blocks the digest; any other `Open` conflict keeps the digest from being written (`DigestNotWritten`), so the file stays without a checksum and the remaining conflicts are shown again at the next read. The task text says so; T83 can start |
| 2026-11-12 | T83 | Ready for review | New `pw-metadata::write_plan` (`ExifPresent`, `FileState`, `WriteSettings`, `WritePlan`, `WriteNotice`, `PlanError`, `plan`) with 30 unit tests; `lib.rs` the `mod` line; new `tests/write_plan.rs` (6 tests, ExifTool required); `docs/test-inventory.md`. `FileState::from_read` was not needed (the tests build `FileState` by hand). Checked with ExifTool 13.59: `-ec` turns `\n`, `\r` and `\\` into characters (every argument is escaped when a value holds a line break); `-<et_xmp>=` removes every language of a Lang Alt, with or without `x-default`; `-IPTC:DateCreated#=` / `-IPTC:TimeCreated#=` store exactly the `to_iim()` text. **Deviations approved by the owner during the task:** (1) a Lang Alt is written as `-<et_xmp>-x-default=<text>`, because `-<et_xmp>=<text>` makes ExifTool replace the whole Lang Alt (the `sv` entry was lost in the integration test); the named form keeps the other languages and inserts `x-default` first when it is missing (Q7); (2) a conversion to UTF-8 (§5.4) also writes the IIM text of an undecided (`Open`) conflict again, unchanged and only re-encoded, so its Latin-1 bytes are not left under a UTF-8 1:90; the conflict is not decided and no checksum is written; (3) structure values also get `|` before a leading space, which ExifTool otherwise drops. Other choices for review: without changes the plan is empty; a clear in a file without IIM writes no IIM (it would only create an empty block); IIM is created with the setting only for a change with an IIM dataset; "XMP after the write" is a written XMP value or an embedded XMP value or conflict in the `Metadata` (`FileState` has no XMP flag); the IIM of a cleared Date Created removes 2:55 and 2:60. `check.ps1`: fmt, test and clippy OK, 725 passed, 0 failed, 33 ignored |
| 2026-11-12 | T83 | Open | **Sent back by the owner** (2026-11-12). Review: the owner's three answers during the task are right (Lang Alt as `-<et_xmp>-x-default=`, re-encoding the IIM text of an `Open` conflict on the UTF-8 conversion, `|` before a leading space); the reviewer's `check.ps1`: fmt, test and clippy OK, 725 passed, 0 failed, 33 ignored. The gap, from the task text and not the agent: the plan writes only the user's changes, so in a `Differs` file where the IIM City won, an edit of only the Rating writes a new digest while XMP keeps the old City; the next read then has a matching checksum and XMP wins, silently undoing the IIM edit (§5.1: the marker stays until a write brings both forms in line; §12: IIM = XMP after every write). What is left, and nothing else: the new bullet "bring both forms in line" in point 4 and its tests under "Tests to add". Keep everything else of T83 as it is; update `docs/test-inventory.md`; then `check.ps1`, the heading status and one log row as usual |
| 2026-11-12 | T83 | Ready for review | Second review: accepted. The agent made the correction but could not set the status: Windows Smart App Control (`os error 4551`, "En programkontrollprincip har blockerat den här filen") refused to run the `pw-metadata` unit-test binary of the workspace build, so `check.ps1` reported the test step as FAILED; the agent asked the owner (rule 7) instead of building elsewhere, and the reviewer set the status. The fix: `bring_in_line` (only for a JPEG, TIFF or DNG with IIM, and only when the plan has changes) writes an unchanged property with an IIM conflict of rule `Iim`, or a value with origin `Iim` and no conflict, to XMP, and one with rule `Xmp` to IIM (predicted and cut); `Open` (Q1) and `Exif` conflicts, XMP/Exif-only conflicts and deprecated IIM properties are left out; a value brought to IIM is not written again with its old text by the UTF-8 conversion; such properties no longer block the digest. Tests: the unit tests of the task text, and `an_iim_value_that_won_by_the_checksum_stays_after_the_write` (IIM City `Malmö` won over XMP `Lund`, only the Rating edited → City `Malmö` from XMP, no conflict, digests equal). The reviewer's `check.ps1`: fmt and clippy OK; the test step showed the same block for the `pw-metadata` unit-test binary with every other binary passing (548 passed, 0 failed, 32 ignored); `cargo test -p pw-metadata --lib` (another build, not blocked) passed 185, 1 ignored; together 733 passed, 0 failed, 33 ignored, as in `docs/test-inventory.md` |
| 2026-11-12 | T84–T86 | Open / Blocked | Added after T83 was done: ADR 0007 I8 split in three. T84 the file snapshot and the verification of a written copy (can start now); T85 the Windows file operations and the write journal (**Blocked** on owner decision D3, `windows-sys` =0.61.2 in `pw-metadata`, no new package); T86 the safe write transaction, after T84 and T85, with an `ImageCheck` the caller gives (the real one with `pw-raw` and LibRaw moves to I9). The step plan gains I8a–I8c and D3 and records the reviewer's ExifTool 13.59 measurements for I8. T84 and T85 touch different files and can run at the same time |
| 2026-11-12 | T85 | Open | Owner approved D3 (`windows-sys` =0.61.2, Windows only, features `Win32_Foundation` and `Win32_Storage_FileSystem`, in `pw-metadata`); T85 can start, at the same time as T84 |
| 2026-11-12 | T84 | In progress | Implemented: `src/verify.rs` (`FileSnapshot`, `Validate`, `ExifTool::snapshot`, `check_file_type`, `verify_copy`, `VerifyError`), the `mod` line in `lib.rs`, `tests/verify.rs`, `docs/test-inventory.md` (rows; the header total not updated, no complete run). Not set to Ready for review on the owner's decision: in three `check.ps1` runs Windows Smart App Control (`os error 4551`, CodeIntegrity events 3077/3118) refused to run 5 rebuilt test binaries (`photoworkflow` bin, `pw-loupe` lib, `pw-metadata` lib, `pw-metadata` tests `sidecar` and `write_plan`), so the test step is FAILED and the 22 unit tests in `verify.rs` have **not run**; fmt and clippy OK; the binaries that ran: 315 passed, 0 failed, 32 ignored. `tests/verify.rs`: 3 passed, 1 ignored. The ignored DNG test (`cargo test --workspace --test verify -- --ignored --nocapture`, `PW_TESTDATA=D:\PhotoWorkflowData\RAW`): all 5 DNGs pass, `Validate` `0 7 1` before and after for each; changed keys (all allowed): IMG_20261026_062851 and _084628 `SubIFD:PreviewImageStart`, `System:*`, `XMP-photoshop:City`, `XMP-x:XMPToolkit` (062851 also `IFD0:TileOffsets`); _091722, _092349, _092351 the same without `XMP-x:XMPToolkit` (already present). `System:*` = FileModifyDate, FileName, FileSize, ZoneIdentifier (the source copies carry a Zone.Identifier stream; the `-o` copy does not). Choices to review: `NewWarnings` also when one side has no `Validate` and the other has; an unreadable `Validate` text is a snapshot error; the listing keeps `File:FileType` and `File:ImageDataHash` (only the three named keys left out); "without the City arguments" tested both as without the XMP argument (`XMP-photoshop:City`) and without XMP and IIM (`IPTC:City`, first in key order); the Description's `sv` entry is in the metadata only, as the plan writes `x-default` (Q7). Risk: cutting `<Tag>` at the first `-` makes `-IPTC:By-line=` allow `IPTC:By-lineTitle` too |
| 2026-11-12 | T85 | In progress | Implemented: `Cargo.toml` D3 lines (`git diff Cargo.lock`: only `windows-sys 0.61.2` added to `pw-metadata`'s list, no new package), `src/fileops.rs` (`free_space`, `is_read_only_volume`, `is_locked`, `can_write`/`Refusal`, `set_modified`, `flush`, `replace`/`ReplaceError`, `replace_by_rename`, `move_new`, `temp_name`), `src/journal.rs` (`Journal`, `Entry`, `EntryId`, `Recovered`), the two `mod` lines in `lib.rs`, `docs/test-inventory.md` (the unit row; the header total not updated, no complete run). Not set to Ready for review on the owner's decision: in three `check.ps1` runs Windows Smart App Control (`os error 4551`) refused to run `pw-loupe --bin`, `pw-metadata --test read` and `--test write_plan` (Blocked, not run); fmt and clippy OK; the binaries that ran: 747 passed, 0 failed, 33 ignored, among them the `pw-metadata` unit tests (233 passed, 1 ignored) with all 26 new T85 tests. Owner decisions in this session: (1) `Refusal::Error(String)` added for checks that cannot be made (missing file, a failing Windows call, not Windows); (2) measured on NTFS: when the original has an alternate data stream, `ReplaceFileW` writes it into the copy and sets the modification time to now (without a stream the copy's time is kept), so `replace` reads the copy's modification time before the call and sets it again after; if only that step fails `replace` still returns `Ok`. Other choices to review: `FILE_READ_ONLY_VOLUME` defined locally (0x80000; in `windows-sys` it needs the `Win32_System_SystemServices` feature, not in D3); after a failed `ReplaceFileW` other than `_2` the result is `OriginalAtBackup` only when the original is missing and the backup present, otherwise `OriginalIntact`; `Recovered` has `Cleaned`, `Restored`, `NeedsUser`, `Failed` (a step failed, entry kept) and `Unreadable` (entry file kept); the journal refuses paths that are not valid Unicode (`Error::Unsupported`); `set_modified`, `flush` and `temp_name` use `std` only and work on every platform. T84 is being done at the same time in the same worktree: `lib.rs`, `docs/test-inventory.md` and this file are shared and were edited in place |
| 2026-11-12 | T84, T85 | Ready for review | Review: both accepted. Both agents stopped at "In progress" on the owner's decision, because Windows Smart App Control (`os error 4551`) refused to run some rebuilt test programs; the reviewer set the status. **T84:** `snapshot` is one `execute` call with the arguments of the task; `verify_copy` checks type, hash, `Validate` counts and the listing with the allowed groups, keys, pointer endings and plan tags; the 5 real DNGs pass (agent's ignored run). **Reviewer's fix:** a tag argument is cut at the first `-` only in an XMP group, because IIM names hold `-` themselves (`-IPTC:By-line=` allowed `IPTC:By-lineTitle`); new unit test `an_iim_name_with_a_hyphen_is_a_whole_tag`. The agent's other choices are accepted (`NewWarnings` when only one side has `Validate`; an unreadable `Validate` is an error). **T85:** the D3 lines exactly, `Cargo.lock` gains only `windows-sys 0.61.2` in `pw-metadata`'s list; every Windows call has a `// SAFETY:` comment; the two owner decisions are right: `Refusal::Error` for checks that cannot be made, and the copy's modification time set again after `ReplaceFileW`, which writes an alternate data stream into the copy and so changes its time; `FILE_READ_ONLY_VOLUME` defined locally rather than enabling another feature is accepted. Note for T86: `recover` assumes one RawStride at a time (another running copy's `pwtmp` file would be deleted). The reviewer's `check.ps1`: fmt and clippy OK; test step FAILED only because Smart App Control refused `tests/read.rs` and `tests/write_plan.rs` (unchanged tests); every other program passed (773 passed, 0 failed, 33 ignored, among them the 23 `verify` and 26 T85 unit tests and `tests/verify.rs` 3 passed, 1 ignored); the two refused programs, run with `cargo test -p pw-metadata --test read --test write_plan`, passed (5 + 1 ignored, 7); total 785 passed, 0 failed, 34 ignored, as in `docs/test-inventory.md` (header updated by the reviewer) |
| 2026-11-12 | – | Decision | Owner added an exception to rule 6 of "How to work on a task": when Windows Smart App Control (`os error 4551`) refuses to run rebuilt test programs, an agent runs only those programs on their own with `cargo test -p <crate>`, reports the `check.ps1` summary, the refused programs, the separate runs and the total, and may set "Ready for review" if they pass and nothing else failed. Seen in T83, T84 and T85 |
| 2026-11-12 | T86 | In progress | Implemented: new `src/safe_write.rs` (`ImageCheck`, `NoImageCheck`, `ExifTool::write_safely`, `WriteReport`, `SafeWriteError`, `Stage`, `OriginalState`), the `mod` line in `lib.rs`, new `tests/safe_write.rs` (`#![cfg(windows)]`, 11 tests, 1 ignored), `docs/test-inventory.md` (the unit row 235 → 242 and a new row; the header total not updated, no complete run). Not set to Ready for review on the owner's decision: Smart App Control (`os error 4551`) refused programs in `check.ps1` and in the separate runs of rule 6. Last `check.ps1`: fmt OK, clippy OK, test FAILED only because 6 programs were refused (`pw-loupe --lib`, `--bin pw-loupe`, `pw-metadata --lib`, `--test sidecar`, `--test verify`, `--test write_plan`); the programs that ran: 299 passed, 0 failed, 32 ignored, among them `tests/safe_write.rs` 10 passed, 1 ignored. Separate runs: `pw-metadata --lib` 240 passed, 1 failed, 1 ignored (all 7 new `safe_write` unit tests pass; the failure is the unchanged `worker::tests::retries_a_failed_start_with_the_next_request`, whose copy of the test program in `%TEMP%` Smart App Control refuses), `--test sidecar` 7 passed, 1 ignored, `--test verify` 3 passed, 1 ignored, `pw-loupe --bin pw-loupe` 25 passed; **never ran** (refused every time): `pw-metadata --test write_plan` (7) and `pw-loupe --lib` (220). Total: 574 passed, 1 failed, 35 ignored, 227 not run. Ignored DNG test (`PW_TESTDATA=D:\PhotoWorkflowData\RAW`): all 9 DNGs (5 Xiaomi `IMG_20261026_*`, 4 `TN00251x`) `Written`, hash unchanged, City and Description read back. Owner decisions in this session: (1) measured with ExifTool 13.59, a sidecar made without a source file always holds `XMP-x:XMPToolkit`, so a new sidecar is checked with T84's allowed rules (keys outside `System`, `ExifTool`, `File` must be allowed by `verify_copy` for the plan; done against an "empty original", `verify.rs` unchanged); (2) a failed final check ends the journal entry so recovery does not delete the kept backup; (3) `SafeWriteError` has a `file` field (the target) so `Display` can name it. Other choices to review: the file kind by extension is a copy of the reader's private `file_kind` (`read.rs` not changed); "one file created and no warning" = `write_succeeded`, exactly one `image files created` and no `Warning` line (measured: an unknown tag gives `1 image files copied` and a warning, and the copy exists, so it is deleted); `can_write` gets the plan's argument bytes as `extra_bytes`; a new sidecar needs the arguments + 10 % + 4096 bytes free; a failed journal `begin` is `WriteCopy`; after `OriginalIntact` the copy and a backup (if any) are deleted and the entry ended only when both deletions worked, otherwise it is kept for recovery; after success a backup that cannot be deleted keeps the entry open (recovery deletes it) and the result is still `Written`; `OriginalAtBackup` that is renamed back gives `Intact`. `OriginalAtBackup` and `FinalCheck` have no integration test (no way to provoke them without fault injection) |
| 2026-11-12 | T86 | Ready for review | Review: code accepted, **not committed**: AGENTS.md allows commits only with green tests, and Windows Smart App Control now refuses most rebuilt test programs, including ones it ran an hour earlier and the `crc32fast` build script of a single-crate build, so rule 6's exception no longer gets them to run. The transaction follows §9 and the task; the agent stopped correctly. The owner's three decisions are right (a new sidecar checked with T84's rules against an "empty original"; a failed final check ends the journal entry so recovery keeps the backup; `SafeWriteError::file`). **Reviewer's fix:** `check_copy` was also called for an existing sidecar (`kind` `Raw`), against the `ImageCheck` contract ("never for a sidecar"); now only for JPEG, TIFF and DNG; new test `the_image_check_is_not_called_for_an_existing_sidecar` (a failing check, the sidecar still updated); `docs/test-inventory.md` `tests/safe_write.rs` 11 → 12. What ran in the reviewer's `check.ps1` (fmt and clippy OK): 532 passed, 2 failed, 32 ignored; the `pw-metadata` unit tests ran (239 passed, among them the 7 `safe_write` and 23 `verify` unit tests) and the 2 failures are the unchanged `worker` tests `retries_a_failed_start_with_the_next_request` and `restarts_exiftool_after_it_exits`, which time out because their copy of the test program in `%TEMP%` is refused; `tests/safe_write.rs` 11 passed, 1 ignored (the new test included); refused: `photoworkflow`, both `pw-loupe` programs, `tests/read.rs`, `sidecar.rs`, `verify.rs`, `write_plan.rs`. Separate runs: `tests/verify.rs` 3 passed, 1 ignored and `tests/safe_write.rs` again 11 passed; everything else refused. Not run since T85: `pw-loupe`, `photoworkflow`, `tests/read.rs`, `sidecar.rs`, `write_plan.rs` (none changed by T86). The agent's ignored DNG run: 9 DNGs written, hashes unchanged. Waiting for the owner's decision on Smart App Control before a full test run and the commit |
| 2026-11-12 | T86 | Done | Smart App Control no longer blocked the test programs. The reviewer's `check.ps1`: fmt, test and clippy OK, **803 passed, 0 failed, 35 ignored**, every program ran (the two `worker` tests pass again); `docs/test-inventory.md` header updated. Committed |
| 2026-11-12 | T87–T91 | Open / Blocked | Added after T86 was done: ADR 0007 I9 split in five. T87 ratings and labels through the write plan and the safe write, with the plan's `FileState` from the reader (can start now); T88 the ingest template with the modes of §10 (**Blocked** on D5, the separator of Append/Prefix, and D6, `xmpRights:Marked`); T89 the view core closes the RAW sessions of a file before it is written (can start now, at the same time as T87); T90 the real `ImageCheck` in a new crate `pw-imagecheck` (**Blocked** on D4, the new crate); T91 RawStride writes ratings and the ingest template safely, with the journal and recovery at start (after T87–T90). D7 (the Tauri app keeps today's write until it is retired) does not block a task. The step plan gains I9a–I9e and D4–D7 and records the reviewer's ExifTool 13.59 and Windows measurements for I9 (a reader holding the file open makes the safe write refuse it, so LibRaw sessions must be closed first; previews are byte for byte equal after a write; a DNG rating will take about 1.5–2 s) |
| 2026-11-12 | T88, T90, T91 | Open | The owner approved D4–D7 as proposed: D4 the new crate `pw-imagecheck` (`pw-metadata`, `pw-raw`, `pw-libraw`; no new package); D5 the separator of Append and Prefix is one space; D6 `xmpRights:Marked` is added to the property table as a non-IPTC entry and set to `True` by `Iptc::to_template` with a copyright, as today; D7 the Tauri app keeps today's in-place write until it is retired. T88 can start after T87, T90 now, T91 after T87–T90 |
| 2026-11-12 | T87 | Ready for review | `MetadataRead::state` (`FileState`) from the reader; new `edit.rs` with `EditOptions`, `EditError` and `ExifTool::write_rating_safely`; tests: `src/edit.rs` 4 unit, `tests/read.rs` +4, new `tests/edit.rs` 9 + 1 ignored; all pw-metadata programs passed and clippy OK. `check.ps1` was **not** all OK because of work by another agent in the same worktree (T89/T90: `cargo fmt --check` differences in `pw-imagecheck` and `pw-viewcore`, 2 failed tests in `pw-imagecheck`); without those crates' changes the counts are 820 passed, 36 ignored (header of `docs/test-inventory.md`). Ignored real-file test (`PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`, 25 files, debug build, one run): 24 written with the image data hash unchanged and read back; time per file DNG 219–740 ms (9), JPEG 643–973 ms (7), TIFF 844–3945 ms (8; 41–240 MB); **TN002683.tif refused in `Verify`** (original intact): the copy has 7 warnings/4 minor against 6/3. That TIFF has IIM with a matching checksum; by hand, `-IPTCDigest=new` on it gives ExifTool's warning "IPTCDigest is not current". The cause lies in the plan or the verification (files T87 may not change); owner to decide |
| 2026-11-12 | T90 | Ready for review | (Agent) New crate `pw-imagecheck` (`Cargo.toml`, `src/lib.rs`, `src/tests.rs`, `tests/real_files.rs`), the D4 line in the root `Cargo.toml`, `Cargo.lock` (only the `pw-imagecheck` package added, no other package), `docs/test-inventory.md` (two new rows). `RawImageCheck::new(release)` / `without_release()`, `Send + Sync`, implements `ImageCheck`: `release` calls the closure; `check_copy` compares the embedded previews of `pw_raw::inspect` (count, then per index width, height, length and bytes with `read_preview`; skipped when the original cannot be inspected) and, for `Dng`, opens the copy with `Session::open_with_diagnostics` (dropped at once; if it fails, passes when the original fails too, else the copy's LibRaw text, which also names the path: `LibRaw: <path>: damaged: …`). **Two departures from the task text, for the owner to confirm** (the code and an earlier version of this row call them owner decisions; no record of that decision was found): (1) for a plain JPEG `pw_raw::inspect` lists the file itself as its only preview (offset 0), which every metadata write changes, so a preview at offset 0 is left out on both sides (otherwise every JPEG write would be refused); (2) the unit tests use a hand-built TIFF holding `pw-thumbcache`'s JPEG fixture as `JPEGInterchangeFormat` (pw-raw lists no embedded thumbnail of a plain JPEG). Other choice to review: for a DNG the LibRaw check runs before the preview check, so a copy cut inside its RAW data gives LibRaw's `damaged:` text and not a preview count error. Tests: 11 unit tests, 3 ignored real-file tests. Ignored run (`PW_TESTDATA=D:\PhotoWorkflowData\RAW`, 9 DNGs, each with 1 embedded preview, debug build): 3 passed; all 9 `Written` through `write_safely` with the real check; `check_copy` time per file: IMG_20261026_062851 272 ms, _084628 269 ms, _091722 264 ms, _092349 289 ms, _092351 289 ms, TN002512 571 ms, TN002513 515 ms, TN002514 586 ms, TN002515 519 ms; every copy cut to half refused with `damaged:` (287–898 ms) and passing against a cut original; one byte changed in the largest preview → `preview 1 differs` for all 9. `.\tools\check.ps1` (with T87 and T89 in the same worktree): fmt, test and clippy OK, **841 passed, 0 failed, 40 ignored**, matching the `docs/test-inventory.md` header. No commit |
| 2026-11-12 | T89 | Ready for review | (Agent) `ViewCore::releaser` and `Releaser::release(path, timeout)` in `core.rs` (waits on the core's condition variable), `pub use` in `lib.rs`; `state.rs`: `State::release` drops owned sessions of the path and marks sessions lent in the current generation (new maps `lent_paths` and `released`, keyed by generation, key and view), `finish_render` drops a marked session but still delivers its view, `open_folder` clears `released`. Addition to check: while a release waits, `pick` opens no new session of that path (`begin_release`/`end_release`), so the plan does not reopen it during the wait; an open already running when `release` starts is not waited for. Tests: `state.rs` +6, `core.rs` +4 (a test backend with a gate in `core.rs`, since `tests/engine.rs` is not in the task's files), `native_files.rs` +1 ignored (passed with `--ignored` on a DNG in `PW_TESTDATA`). `check.ps1` all OK (841 passed, 40 ignored, 0 failed, with T87 and T90 in the same worktree); `docs/test-inventory.md` updated including the header total |
| 2026-11-12 | T87 | Ready for review | (Agent) Re-check after T89 and T90 were ready: no code change to the T87 files (`read.rs`, `edit.rs`, `lib.rs`, `tests/read.rs`, `tests/edit.rs`). `check.ps1` is now **all OK** (fmt, test, clippy): **841 passed, 0 failed, 40 ignored** in 52 programs, as in the header of `docs/test-inventory.md` (`test-output.txt` holds two complete runs with these counts because another agent ran `check.ps1` at the same time). pw-metadata: unit 245 passed + 1 ignored, `tests/edit.rs` 9 + 1 ignored, `tests/read.rs` 9 + 1 ignored. Ignored real-file test run again (`PW_TESTDATA` = `D:\PhotoWorkflowData\RAW`, debug build): 24 of 25 written, image data hash unchanged, read back; DNG 237–763 ms (9), JPEG 664–976 ms (7), TIFF 867–4166 ms (8); **TN002683.tif again refused in `Verify`** (original intact; 7 warnings/4 minor against 6/3). Still for the owner: the cause is in `write_plan.rs` or `verify.rs`, which T87 may not change |
| 2026-11-12 | T87, T89, T90 | Done | Review: all three accepted. **T89:** `Releaser::release` drops owned sessions, marks lent ones (`released`, also across `set_files`/`open_folder`), waits on the condition variable and blocks new opens of the path meanwhile (`begin_release`/`end_release`, an addition beyond the task text, accepted); views kept; the ignored DNG test shows the exclusive open failing with a session and working after `release`. Known limit for T91: the core may reopen the file after `release` returns and before the write, or a short pw-raw/thumbnail open may race; the safe write then refuses it as `Locked` with the file untouched. **T90:** the crate and the D4 lines exactly, `Cargo.lock` gains only `pw-imagecheck`; previews compared by count, size and bytes, LibRaw on the DNG copy (and on the original only when the copy fails). The agent's two choices are accepted by the reviewer, but they were **not owner decisions** as the code said: a preview at offset 0 (a plain JPEG, which `pw_raw::inspect` lists as its own preview) is left out on both sides, and the unit tests hold the JPEG fixture as the thumbnail of a hand-built TIFF; the reviewer changed the wording to "accepted in the review, 2026-11-12" in `lib.rs`, `tests.rs` and `docs/test-inventory.md`. **T87:** `MetadataRead::state` and `write_rating_safely` follow the task; the ignored real-file test writes 24 of 25 files. **Finding outside T87** (reviewer, ExifTool 13.59): `TN002683.tif` (TIFF with IIM and a matching `IPTCDigest`) is refused in `Verify` with one more warning, `IPTCDigest is not current`. In a TIFF, `-IPTCDigest=new` in the same call as an IIM write is the MD5 of the new IIM block **without** the padding ExifTool adds in the TIFF (3 NUL bytes here: `f379e478…` is the MD5 of the 292-byte block less 3 bytes), while `CurrentIPTCDigest` includes them (`2c586d07…`); a second call with only `-IPTCDigest=new` on the copy gives a matching digest; a JPEG is not affected. So every write that touches IIM in a TIFF (and probably a DNG with IIM) is refused, correctly, by T84's check: a fix in the write plan or the safe write needs its own task. The reviewer's `check.ps1` after the wording change: fmt, test and clippy OK, **841 passed, 0 failed, 40 ignored**. Committed |
| 2026-11-12 | T92 | Blocked | Added after the review of T87: in a TIFF or DNG, `-IPTCDigest=new` in the same call as an IIM write leaves the checksum out of date (MD5 without the TIFF padding), so T84's check refuses every write that touches IIM there (`TN002683.tif`). T92 adds a second ExifTool call on the copy for the checksum and, after owner decision D8, allows the one warning ExifTool adds when it creates the Photoshop block of a TIFF or DNG. The step plan gains I9f and D8 and records the reviewer's measurements. T91 does not wait for T92 |
| 2026-11-12 | T92 | Open | The owner approved D8 as proposed: `verify_copy` allows exactly one more warning (not error) when the copy of a TIFF or DNG has a new `Photoshop:IPTCDigest` and the plan writes `-IPTCDigest=new`. T92 can start, at the same time as T88 |
| 2026-11-12 | T88 | Ready for review | (Agent) New `src/template.rs` (`Mode`, `TemplateField`, `Template`, `TemplateError`, `template::apply`); `edit.rs`: `EditError::Template` and `ExifTool::apply_template_safely`; `lib.rs`: the `mod` line and `Iptc::to_template` (plus its unit tests); `properties.rs`: `xmpRights:Marked` as a hand-written entry (D6), with the existing count assertion `66 + 4` → `66 + 5` (the D6 entry) and a new test; new `tests/template.rs` (5); `docs/test-inventory.md` (unit row 250 → 272, a new row; header total not updated, because the run includes T92's uncommitted work). `check.ps1` all OK (fmt, test, clippy): **874 passed, 0 failed, 41 ignored** in 53 programs, T92's work in progress in the same worktree included. **Found:** with City as the only XMP value, Clear on a JPEG with IIM fails the copy check: `write_plan.rs` (T83) writes no `-IPTCDigest=new` when no XMP is left, so the old `IPTCDigest` stays out of date ("IPTCDigest is not current", one more warning). Owner chose to give the test JPEG a keyword in XMP as well; `write_plan.rs` not changed, the bug is for a follow-up |
| 2026-11-12 | T92 | Ready for review | (Agent) `safe_write.rs`: the digest pass (`-IPTCDigest=new -overwrite_original <copy>` for a `Tiff`/`Dng` plan with `-IPTCDigest=new`; one file updated and no warning, else `WriteCopy`); `verify.rs`: D8 (one more warning when a TIFF/DNG copy gets a new `Photoshop:IPTCDigest` and the plan writes `-IPTCDigest=new`). **Found and decided with the owner (choice A, outside the task text):** the digest pass alone did not make `TN002683.tif` pass. Measured with ExifTool 13.59: the written copy is correct (read in a new process: 5/3 warnings, digest current), but after a `-stay_open` process has written `-IPTCDigest=new` to a TIFF whose Photoshop block holds an `IPTCDigest` followed by an `AgHg` resource (0x1f40; Lightroom/Photo Mechanic), that process reads the block of every such TIFF wrongly until it ends (+2 warnings: `[minor] Non-standard Photoshop at TIFF-IFD0-Photoshop-IPTCDigest`, `Bad Photoshop IRB resource "<first 4 digest bytes>"`), also files it never wrote; JPEG and DNG writes do not do this. So the owner approved changing **`exiftool.rs`** too: new `ExifTool::restart` (same program and arguments; a failed restart marks the instance broken), and `write_safely` restarts ExifTool after a TIFF write with `-IPTCDigest=new` (also a failed one) before the copy is read (about 0.25 s). The other TIFFs had only passed before because both of their snapshots were already misread (TN002694 read 8/4, really 6/3). Tests: unit `verify.rs` +2, `safe_write.rs` +3, `exiftool.rs` +3; `tests/safe_write.rs` +3 (minimal TIFF with IIM; without IIM and "create IIM" (D8); with an `IPTCDigest` + `AgHg` block, which fails in `Verify` without the restart) + 1 ignored. Ignored runs (`PW_TESTDATA=D:\PhotoWorkflowData\RAW`): `every_real_tiff_and_dng_gets_a_current_digest` passed (18 files, all written with a current digest; TN002683 6/3 → 5/3; DNGs one more warning by D8), T87's `every_real_dng_jpeg_and_tiff_is_rated_safely` passed (25 of 25, TN002683 included). `check.ps1` all OK: **879 passed, 0 failed, 41 ignored** in 53 programs, with T88's uncommitted work in the same worktree; `docs/test-inventory.md` header updated to that run. `git diff --stat` also shows T88's files. Suggestion: report the misread to ExifTool's author |
| 2026-11-12 | T88, T92 | Done | Review: both accepted. **T88:** `template::apply` follows §10 and the task (one space as the separator, D5; no Clear of the eight copyright-management properties; a sub-field of a list of structures refused; a result equal to the file is not set), `apply_template_safely`, `Iptc::to_template` with `xmpRights:Marked` `True` beside a copyright (D6, a hand-written entry in `properties.rs`). **T92:** the digest pass on the copy for a TIFF/DNG plan with `-IPTCDigest=new`, and the D8 allowance in `verify_copy`. The owner's choice A during the task, which also changes `exiftool.rs` (`ExifTool::restart`, used by `write_safely` after a TIFF write with `-IPTCDigest=new`), is right: the reviewer reproduced it with one `-stay_open` ExifTool 13.59 process and argument files: an untouched copy of `TN002683.tif` reads `6 Warnings (3 minor)` before the process writes `-IPTCDigest=new` to another copy and `8 Warnings (4 minor)` after it, while a new process reads 6 again. The reviewer's `check.ps1`: fmt, test and clippy OK, **879 passed, 0 failed, 41 ignored**; ignored runs (`PW_TESTDATA=D:\PhotoWorkflowData\RAW`): `edit` `every_real_dng_jpeg_and_tiff_is_rated_safely` passed (25 files, `TN002683.tif` included), `safe_write` `every_real_dng_is_written_safely` and `every_real_tiff_and_dng_gets_a_current_digest` passed. **Left for a follow-up** (found in T88, in `write_plan.rs` of T83): clearing the only XMP value of a JPEG with IIM writes no `-IPTCDigest=new`, so the old checksum stays out of date and the copy check refuses the write (the original is untouched). Also suggested: report the `-stay_open` misread to ExifTool's author. Committed |
| 2026-11-12 | T93 | Open | Added after the review of T88: `write_plan` writes `-IPTCDigest=new` only when IIM and XMP both exist after the write, so clearing the only XMP value of a file with IIM and a stored checksum leaves the checksum out of date and the copy check refuses the write. T93 also writes it when the plan writes IIM and the file has a stored checksum. No owner decision needed (§5.2, §12). Can run at the same time as T91 |
| 2026-11-12 | T93 | Ready for review | (Agent) `write_plan.rs`: `-IPTCDigest=new` is also written when the plan writes IIM and the file has a stored checksum (`Matches`/`Differs`), even when no XMP is left; Q1 unchanged; module comment and the comment at the rule updated. Five new unit tests, `clearing_the_only_xmp_value_keeps_the_stored_digest_current` in `tests/write_plan.rs`, `clear_of_the_only_xmp_value_keeps_the_checksum_current` in `tests/template.rs`. **Decided with the owner (outside the task text):** the existing unit test `the_digest_needs_xmp_after_the_write` used `jpeg_with_iim()` (`Matches`) and asserted no checksum, exactly the case T93 changes; on the owner's choice its state is now `missing_digest()` (assertion unchanged, comment says "without a stored checksum"). `check.ps1`: fmt, test, clippy OK; 886 passed, 41 ignored, 0 failed |
| 2026-11-12 | T91 | Ready for review | (Agent) `worker.rs`: `WriteSetup` (journal, checks, options); a rating write runs `write_rating_safely`. `lib.rs`: `WriteError::NotSafe` and `From<EditError>` as in point 2, English `Display`; also the `pub use` line (exports `WriteSetup` and `NO_JOURNAL`, needed by `pw-loupe`). `pw-loupe`: `main.rs` opens `%LOCALAPPDATA%\RawStride\journal`, runs `Journal::recover` before the workers and shows the Swedish message box (title "Metadataskrivning avbröts"); one `RawImageCheck` with `releaser.release(path, 5 s)` per worker; `metadata.rs`: the three `NotSafe` texts; `import_worker.rs`: `apply_template_safely`; `Cargo.toml` + `Cargo.lock`: only `pw-imagecheck`. **Decided with the owner during the task (outside the task text):** (1) the fake-ExifTool tests `restarts_exiftool_after_it_exits` and `retries_a_failed_start_with_the_next_request` drive the worker with `read_ratings` of a RAW with a sidecar (a safe write needs a JSON read, which the fake cannot answer); what they check is unchanged; (2) without a journal `MetadataWorker::start`/`start_with` take `Option<WriteSetup>` and `ImportWorker::start` `Option<Journal>`: reading works, a rating write gives `WriteError::Other(NO_JOURNAL)` shown as "skrivloggen kunde inte öppnas", the template step lists that text once (as a failed ExifTool start) and writes nothing; (3) the recovery line names the backup only when the backup file exists. `Failed` entries are listed like `NeedsUser`, `Unreadable` by the journal file; a journal that opens but cannot be read is only reported on stderr. 17 new tests (pw-metadata unit +9, pw-loupe lib +2, bin +6). `check.ps1` all OK: 903 passed, 0 failed, 41 ignored in 53 programs; clippy without warnings. Not run: the owner's manual check in RawStride (DNG in RAW mode, JPEG, ARW) |
| 2026-11-12 | T91, T93 | Done / Ready for review | Review: both accepted. **T93** (Done): `-IPTCDigest=new` also when the plan writes IIM to a file with a stored checksum and no XMP is left; the owner's change of the existing unit test `the_digest_needs_xmp_after_the_write` (its state now `missing_digest()`) is right, because that test asserted exactly the case T93 corrects. **T91** (code accepted, stays Ready for review until the owner's hand check of its Done when): `WriteSetup` and `write_rating_safely` in the worker, `WriteError::NotSafe` and `From<EditError>`, the journal in `%LOCALAPPDATA%\RawStride\journal` with recovery before the workers start, one `RawImageCheck` with `Releaser::release` (5 s) per worker, the Swedish texts, the ingest through `apply_template_safely`; `Cargo.toml`/`Cargo.lock` add only `pw-imagecheck` to `pw-loupe`. The owner's three decisions during the task are accepted: the two fake-ExifTool worker tests now drive the worker with `read_ratings` (a safe write needs a JSON read the fake cannot answer; what they check is unchanged); without a journal the workers start with `None` and every write fails with "skrivloggen kunde inte öppnas"; the recovery line names the backup only when it exists. The reviewer's `check.ps1`: fmt, test and clippy OK, **903 passed, 0 failed, 41 ignored**; ignored runs (`PW_TESTDATA`): `edit` (25 files) and both `safe_write` real-file tests passed. Committed. Left: the owner's hand check in RawStride (a rating on a DNG shown in RAW mode, on a JPEG and on an ARW with a sidecar) |
| 2026-11-12 | T91 | Done | Owner hand check in RawStride: works (the Done when check: a rating on a DNG shown in RAW mode, on a JPEG and on an ARW with a sidecar). T91 is complete |
| 2026-11-12 | T94–T106 | Open / Blocked | Added after T91 was done: ADR 0007 I10 split in thirteen. `pw-metadata`: T94 the ExifTool version checked when the worker starts (**Blocked** on D9); T95 the two sidecar names looked up directly instead of listing the folder for every image (from T82); T96 the validation warnings of §11; T97 edits of single properties and sub-fields written safely, with the plan's write notices (`Edit`, `apply_edits`, `write_edits_safely`); T98 the worker reads the whole `MetadataRead`, writes edits and takes new options (after T94, T97). `pw-loupe`: T99 the settings file (**Blocked** on D10); T100 a mode per field in the import dialog and the confirmation before a Clear (**Blocked** on D13, D14; left over from T88/T91); T101 the start message for a missing or too old ExifTool (after T94, T100); T102 the texts of the info panel (**Blocked** on D11); T103 the info panel (after T98, T101, T102, D15); T104 the editor's form (after T96, T97, T102, D12); T105 the editor window (after T103, T104); T106 the settings window (after T98, T99, T101). T95, T96 and T97 can start now and at the same time (T96 not with T94: both change `lib.rs`); T101, T103, T105 and T106 one at a time (`main.rs`). Owner decisions D9–D15 added to the step plan; D1 still open |
| 2026-11-12 | T95 | In progress | (Agent) `read.rs`: on Windows `find_sidecars` looks up `<basename>.xmp` and `<basename>.<ext>.xmp` directly (`std::fs::metadata`; only `NotFound` is "no sidecar", any other failure is the error) and returns `folder.join(<file name of std::fs::canonicalize>)`, the name as stored; other systems keep the listing (`#[cfg(not(windows))]`); module comment says why. `tests/read.rs`: `IMG_1.XMP` next to a fake `img_1.dng` → `ForeignSidecarIgnored` naming `IMG_1.XMP`; `img_2.arw` + `IMG_2.xmp` read; `x.arw.XMP` → `ExtSidecarIgnored` naming `x.arw.XMP`; ignored timing test. Time per read, JPEG next to 20 000 empty files, ten reads, one run each, Windows 11, test profile, ExifTool through `-stay_open`: **21.0 ms before, 5.7 ms after**. `check.ps1`: tests OK (917 passed, 0 failed, 42 ignored, including another agent's unfinished T97 tests), clippy OK, **fmt FAILED only in `edit.rs`** (T97, changed by another agent in this worktree while T95 ran; not touched); `read.rs` and `tests/read.rs` pass `rustfmt --check`. Waits for a green `check.ps1` |
| 2026-11-12 | T96 | In progress | (Agent) New `validate.rs` (`pub mod validate` in `lib.rs`): `Warning` with the seven warnings, `validate(&Metadata, iim_written)` in table order, English `Display`, module comment (§11; left out PLUS/I11, other vocabularies, dates). `TooLongForIim` through `iim::predict` with `iim_read::iim_format` (shown text of a Lang Alt, each list entry, the property once). A blank value (`iim::is_blank`) counts as missing, also for Creator in `JobtitleWithoutCreator`; a Digital Source Type code is one or more ASCII letters or digits. Owner decision 2026-11-12: `abc` cannot be stored as `xmp:Rating` in a `Metadata` (`check_kind`), so it is tested through the private `rating_warning` that `validate` uses. 15 unit tests, all passed; `docs/test-inventory.md` row 290 → 305, header 920 passed / 42 ignored. `check.ps1`: fmt OK, clippy OK, **test FAILED only in `tests/edits.rs`** (another agent's unfinished T97, untracked; `clearing_the_copyright_notice_removes_it_from_xmp_and_exif`); 936 passed, 1 failed, 42 ignored; `pw_metadata` lib 316 passed, 1 ignored. Waits for a green `check.ps1` |
| 2026-11-12 | T102, T104 | Blocked | Owner decision on D11 and translation: `property_label` takes a language (`Language::Sv`, `Language::En`); English always gives the IPTC name from the property table, Swedish the D11 name where there is one. The Swedish names stay in one table in `info_panel.rs`; T104 takes its labels from it. Swedish names for every property (no English fallback in the Swedish panel) are kept as a future improvement under "Not for agents yet". D11 itself (the proposed names) is still open |
| 2026-11-12 | T95, T96 | Done | Review: both accepted. **T95:** on Windows `find_sidecars` looks up `<basename>.xmp` and `<basename>.<ext>.xmp` directly and reports the name as stored (`canonicalize`); other systems keep the listing. The small deviation from the task text is accepted: `std::fs::metadata` instead of `is_file()`, so only `NotFound` means "no sidecar" and any other failure is the error, as the task asks for an unreadable folder. Measured by the agent: 21.0 ms → 5.7 ms per read next to 20 000 files. **T96:** `validate.rs` with the seven warnings in table order, English `Display`; the owner's decision to test `abc` through the private `rating_warning` (a `Metadata` cannot hold it as `xmp:Rating`) is right. The reviewer's `check.ps1`: fmt and clippy OK; test 936 passed, **1 failed**, 42 ignored, the failure only in T97's unfinished `tests/edits.rs` (untracked, not committed); without T97's 17 tests that is 920 passed, 42 ignored, as `docs/test-inventory.md` says. Committed only T95's and T96's files |
| 2026-11-12 | T97 | Open | Stopped by the agent: `tests/edits.rs` `clearing_the_copyright_notice_removes_it_from_xmp_and_exif` fails in `verify_copy` (`File:ExifByteOrder changed unexpectedly from "MM" to (none)`). Measured by the reviewer with ExifTool 13.59: a JPEG whose only Exif is `IFD0:Copyright` loses the whole Exif block when it is cleared, with `File:ExifByteOrder` and `IFD0:YCbCrPositioning`; with any other Exif tag left the block stays. **Owner decision A (2026-11-12):** `verify.rs` allows the removal of exactly those two keys when the plan clears an `IFD0`/`ExifIFD` tag and the copy has no Exif left; added to T97 as point 6 with its tests and `verify.rs` in its files. T97's other code was read by the reviewer and follows the task so far; it is reviewed when Ready for review |
| 2026-11-12 | T97 | Ready for review | (Agent) `edit.rs`: `Edit`, `apply_edits`, `EditReport`, `write_edits_safely` (the plan's notices when `Written`, empty when `Unchanged`), module comment (§5.1/Q1, §5.2, §8); `template.rs` unchanged (the sub-field logic is in `edit.rs`). Not in the task text: an unknown field of a single structure is `Value(UnknownProperty("<property>/<field>"))`; a Lang Alt field edits `x-default` only (as the template). Point 6 (`verify.rs`, `drops_exif_block`): **owner decision 2026-11-12, five keys instead of two** — with the JFIF fixture ExifTool 13.59 also creates `IFD0:XResolution`, `IFD0:YResolution` and `IFD0:ResolutionUnit` with the Copyright and drops them with the block, so `EXIF_BLOCK_KEYS` holds `File:ExifByteOrder`, those three and `IFD0:YCbCrPositioning` (removal only, no Exif key left). Tests: 12 `edit::` and 4 `verify::` unit tests, `tests/edits.rs` 5 (ExifTool), all passed; `docs/test-inventory.md` updated. `check.ps1`: fmt OK, test OK (941 passed, 0 failed, 42 ignored), clippy OK |
| 2026-11-12 | T97 | Done | Review: accepted. `Edit`, `apply_edits` (the `Open` conflict rule of Q1, Clear of copyright fields allowed, sub-fields of a single structure in the structure's order), `EditReport` and `write_edits_safely` with the plan's notices follow the task; `write_rating_safely` and `apply_template_safely` unchanged. The two additions outside the text are accepted: an unknown field of a single structure is `Value(UnknownProperty("<property>/<field>"))`, and a Lang Alt field edits `x-default` only (as the template). Point 6: the owner's decision for five keys is right; the reviewer measured it with ExifTool 13.59 on a JPEG with only a JFIF density: `IFD0:Copyright` written gives `XResolution`, `YResolution`, `ResolutionUnit` and `YCbCrPositioning` too, and clearing it removes all of them and `File:ExifByteOrder`. Reviewer fix: the comment of `drops_exif_block` said the block stays with `IFD0:XResolution` left; it now says that holds for a resolution of its own (300), and the line over 100 characters is wrapped. The reviewer's `check.ps1`: fmt, test and clippy OK, **941 passed, 0 failed, 42 ignored** in 54 programs. Committed |
| 2026-11-12 | – | Decision | Owner added rule 13 to "How to work on a task": every text the user sees is made by a function in its area's text module as a whole-sentence template with placeholders, no user-visible string literals in drawing code, property names only through `property_label`, no fixed text width; existing hard-coded texts stay unless a task moves them. It applies to every task that adds or changes such a text, whichever rules the task lists. T102–T104 already state the same rule for the info panel and the editor (commit fc5b3e0) |
| 2026-11-12 | T94, T99–T104 | Open | The owner approved D9–D15 as proposed: D9 an ExifTool that is missing or older than 12.67 gives one message at start and refuses every write, reading as today; D10 the settings in `%LOCALAPPDATA%\RawStride\settings.txt`, `key = value`; D11 the Swedish property names of the list (the others IPTC's English name); D12 the editor edits one image at a time with the 22 fields and the conflict choices; D13 the modes Behåll, Ersätt, Lägg till efter, Lägg till före, Rensa, default Ersätt, the allowed modes per field; D14 one confirmation per started import that clears fields; D15 the panel on the right with `M`, the editor with `E` or "Redigera". T94, T99, T100 and T102 can start now; T101, T103 and T104 wait for the tasks they name |
| 2026-11-12 | T99 | Ready for review | (Agent) New `crates/pw-loupe/src/settings.rs` as D10: `Settings` (default no IIM, Windows-1252) with `edit_options`, `settings_path`, `load_settings` (problems in English with the line number; unreadable file → defaults and one problem), `save_settings` (`# RawStride` and both keys to `settings.txt.tmp`, synced, then renamed; the `.tmp` removed if the write or rename fails); `lib.rs` one `mod` and one `pub use` line. Not in the task text: a leading UTF-8 byte order mark is skipped (an editor may write one), an empty `LOCALAPPDATA` counts as missing. 15 unit tests; `docs/test-inventory.md` updated |
| 2026-11-12 | T94 | Ready for review | (Agent) `exiftool.rs`: `ToolVersion` (`parse`, order, `Display` `13.05`), `MIN_EXIFTOOL_VERSION` = 12.67, `ExifTool::version`. `worker.rs`: the worker thread starts ExifTool at once and asks the version at every start of a process; the first result is kept in an `Arc<OnceLock<…>>` for `MetadataWorker::exiftool_version`; a rating write fails with `EXIFTOOL_TOO_OLD` while the running process is older or its version unreadable (`NO_JOURNAL` is checked first), reading unchanged, no new `Event`; a failed first start is retried with the next request. `lib.rs`: the three exports. **Decided with the owner during the task (outside the task text):** (1) `test_support.rs` changed: fake modes `version=<text>` and `exit-after-first-request` (answers `-ver` with `13.59`, exits at the second other command); (2) the existing test `restarts_exiftool_after_it_exits` now uses `exit-after-first-request` (with `-ver` at start, `exit-after-first` would answer only the version; what it checks is unchanged); (3) `version()` parses the last non-empty line of standard output before `--- stderr ---` (the fake is a libtest program that writes `running 1 test` before the first answer; also a stderr message cannot make a good ExifTool "too old"). 12 new tests in the `pw-metadata` library (7 `exiftool::`, 5 `worker::`, one with the real ExifTool), all passed. `check.ps1`: fmt, test, clippy OK; 968 passed, 0 failed, 42 ignored in 54 programs (includes T99's uncommitted tests) |
| 2026-11-12 | T100 | Ready for review | (Agent) `import.rs`: `IptcField` (`ALL`, `label`), `IptcModes` (default Ersätt for all six) as `IptcForm::modes`, `allowed_modes`, `mode_text`, `IptcForm::to_template`, `template_is_empty`, `cleared_fields`, `clear_confirmation_text`; `text`/`text_mut` and `IptcModes::get`/`get_mut` so the dialog draws the six fields in a loop. Owner's answers during the task: (1) `to_template` gives the same nine fields in the same order as `Iptc::to_template` (`dc:title` and `dc:description` as `Keep`), and Replace, Append or Prefix with an empty text become `Keep` without a value, so the test compares the templates with `assert_eq`; (2) rule 13: the window's title and buttons are functions in `import.rs` (`clear_confirmation_title`, `clear_and_import_text`, `clear_cancel_text`). `import_worker.rs`: `apply_iptc` takes the `IptcForm` and writes its `to_template()`, ExifTool not started when `template_is_empty()`. `main.rs`: a drop-down list of the allowed modes after each field, the text disabled in Behåll and Rensa, the modes kept in `last_form`; with fields in Rensa the start button or Enter opens "Rensa fält?" ("Rensa och importera" starts, "Avbryt" or Escape goes back); the dialog is disabled while it is open. Two existing tests needed a mechanical change, assertions unchanged: `iptc_form_maps_copyright_to_x_default_and_empty_form_is_empty` gets `..IptcForm::default()` (new field) and `cancellation_before_iptc_writes_nothing` passes the form instead of `.to_iptc()` (new parameter type). 10 new tests (8 in `import.rs`, 2 ExifTool tests in `import_worker.rs`, run with ExifTool on PATH). `check.ps1`: fmt, test and clippy OK, **978 passed, 0 failed, 42 ignored** in 54 programs (with T94's and T99's uncommitted changes in the tree). The owner's manual check (a cleared Webb asks once and clears it in every imported image) is not done |
| 2026-11-12 | T94, T99, T100 | Done / Done / Ready for review | Review: all three accepted. **T94** (Done): `ToolVersion`, `MIN_EXIFTOOL_VERSION` 12.67 and `ExifTool::version`; the worker starts ExifTool at once, asks the version at every start, keeps the first result for `exiftool_version`, and refuses rating writes with `EXIFTOOL_TOO_OLD` while the running process is too old or unreadable (`NO_JOURNAL` first), reading unchanged, no new `Event`. The owner's three decisions during the task are accepted: the fake's modes `version=<text>` and `exit-after-first-request` in `test_support.rs`; `restarts_exiftool_after_it_exits` on the new mode (what it checks unchanged); the version from the last non-empty standard-output line, so a stderr message cannot make a good ExifTool too old. **T99** (Done): `settings.rs` as D10; skipping a leading byte order mark and treating an empty `LOCALAPPDATA` as missing (outside the text) are accepted. **T100** (code accepted, stays Ready for review until the owner's hand check of its Done when): `IptcField`, `IptcModes` (default Ersätt), `allowed_modes` (no Clear of Creator, Copyright, Credit; no Prefix on Creator), `mode_text`, `IptcForm::to_template` (equal to `Iptc::to_template` for the default form, D6 mark with Replace/Append/Prefix), `template_is_empty`, `cleared_fields`, the D14 texts as functions (rule 13); the import worker writes the form's template and starts no ExifTool for an empty one; the dialog has a mode list per field, the text disabled in Behåll and Rensa, and "Rensa fält?" asked once per start (Enter does not answer it, Escape is Avbryt, the dialog disabled while it is open). The owner's answers during the task (the same nine fields as `Iptc::to_template`; Replace/Append/Prefix with an empty text become `Keep`) are accepted; the two mechanical test changes keep their assertions. The reviewer's `check.ps1`: fmt, test and clippy OK, **978 passed, 0 failed, 42 ignored** in 54 programs. Committed. Left: the owner's hand check of T100 (an import with a cleared Webb asks once and clears it in every imported image) |
