# T292. Reading and saving template files on a thread of their own — Done

- **Why:** ADR 0017 U13, part U13c (U13-Q1, U13-Q2; U13-Q14), second of
  the chain T291 → T292 → T293 → T294 → T295 → T296. The template window (T293) and the import's
  start (T295) read template files, and the window saves them, with
  `ExifTool::read_template_file` and `ExifTool::write_template_file` (T285). Both run ExifTool,
  so they cannot run on the window's thread. A save also needs an ExifTool started with
  `-config` and the file `write_exiftool_config` makes, or the + boxes (`rawstride:AddTo`) are
  refused (T285; plan notes of U13). This task gives the program one small thread for template
  files with **its own ExifTool process**, so the metadata worker's and the import's processes
  are never started with the config, and a config that cannot be used only stops saving
  templates.
- **Crates:** `pw-loupe` (and one line of `pw-metadata`). **Files:**
  `crates/pw-loupe/src/template_io.rs` (new), `crates/pw-loupe/src/lib.rs`,
  `crates/pw-metadata/src/lib.rs` (point 0 only), `docs/test-inventory.md`.
- **Background (read before starting):** `pw_metadata::template_file` (the module comment on
  `-config`, `write_exiftool_config`, `EXIFTOOL_CONFIG`, `TemplateFile`, `TemplateRead`,
  `TemplateFileError`, `ExifTool::write_template_file`, `ExifTool::read_template_file`);
  `pw_metadata::ExifTool::start_with` (the `pre_args` come before `-stay_open`) and the program
  `ExifTool::start` uses (`exiftool_program`); `crates/pw-metadata/tests/template_file.rs`
  (how its tests start ExifTool with `-config`); `import_worker.rs` (a worker thread with a
  request channel and `try_recv`, and its tests that need ExifTool on PATH,
  `docs/TESTING.md` §4.5).
- **What:**
  0. **`pw-metadata`'s `lib.rs`:** `exiftool_program` is re-exported beside `ExifTool`
     (`pub use exiftool::{ExifTool, MIN_EXIFTOOL_VERSION, ToolVersion, exiftool_program};`), so
     `pw-loupe` can start the same program `ExifTool::start` uses, with `-config` added. Nothing
     else in `pw-metadata` changes.
  1. **`template_io.rs`** (new module, exported from `lib.rs`; `TemplateIo`, `TemplateEvent`,
     `SaveError` and `TemplateIoStopped` are `pub` and re-exported, since T293's `main.rs` is a
     binary crate that uses `pw_loupe::…`): `TemplateIo`, a thread that owns
     one `ExifTool` for template files. `TemplateIo::start(program: Option<String>,
     config_folder: Option<PathBuf>) -> std::io::Result<TemplateIo>` starts the thread
     (`std::thread::Builder::spawn`, as `ImportWorker` does) and returns at once; the request
     channel is unbounded (`mpsc::channel`). ExifTool is started on the thread at the first
     request, with `ExifTool::start_with(program, pre_args)` (`program`, or `exiftool_program()`
     when `None`). Before each request, an ExifTool that `is_broken()` is dropped and started
     again at that request (with the config written again, point 2; not `ExifTool::restart`,
     which keeps the old arguments); a start that failed is tried again at the next request.
     There is no script argument: `PW_EXIFTOOL_PL` (`docs/TESTING.md` §4.5) is not supported
     here. `TemplateIo` holds the thread's `Option<JoinHandle<()>>`; dropping it closes the
     request channel and does **not** join (as `ImportWorker`, so the window's thread never
     waits for ExifTool to stop); the thread then ends, and its ExifTool with it. A
     `#[cfg(test)]` method (for example `take_thread`) gives the tests the handle. No version
     check: a template file is a new XMP file, not a safe write of an image.
  2. **The config** (`config_folder` is `%LOCALAPPDATA%\RawStride`, given by T293; `None`
     without `LOCALAPPDATA`): before ExifTool is started, `write_exiftool_config(config_folder)`
     writes `rawstride-exiftool.config` there, and ExifTool is started with
     `-config <path>` as its `pre_args`. The path goes on
     ExifTool's command line, where Windows passes only the letters of the system's code page,
     so a function `config_argument(path: &Path) -> Option<String>` (it asks the file system on
     Windows; the path must exist) decides the text passed:
     - the path itself when it is all ASCII;
     - otherwise, on Windows, its short (8.3) form (`GetShortPathNameW`, `windows-sys`'s
       `Win32_Storage_FileSystem`, already among `pw-loupe`'s features) when that is all ASCII;
     - otherwise `None` (off Windows, a path that is not all ASCII gives `None`).

     Without `config_folder`, when `write_exiftool_config` fails, or when `config_argument` gives
     `None`, ExifTool is started **without** `-config`; loading works as before (reading needs no
     config, T285), and every save answers `SaveError::NoConfig(reason)` (point 4) **before**
     ExifTool is started or asked anything, until the next start of ExifTool (when the config is
     tried again). The reason is error detail, like `TemplateFileError`'s texts: one function in
     `template_io.rs` (`no_config_reason`, a name of the agent's choice) makes it, one English
     sentence template per case, with the path and the error's text filled in: `LOCALAPPDATA is
     not set`, `{path} could not be written: {error}`, `{path} has letters ExifTool cannot be
     given`. It goes into T291's `Not saved: ExifTool cannot use its settings file for templates
     ({reason}).`.
  3. **Requests:** `load(path: PathBuf) -> Result<u64, TemplateIoStopped>` and
     `save(path: PathBuf, template: TemplateFile) -> Result<u64, TemplateIoStopped>`, each
     returning a new request id (counting from 1), answered in order. `TemplateIoStopped` is the
     error when the thread has ended (the channel is closed), or when `start` failed and T293
     keeps no `TemplateIo`; the shell shows it as a failed load or save with T291's text for it.
  4. **Answers:** `try_recv() -> Option<TemplateEvent>`, with
     - `TemplateEvent::Loaded { id, path, result: Result<TemplateRead, TemplateFileError> }`
       (`read_template_file`);
     - `TemplateEvent::Saved { id, path, result: Result<(), SaveError> }`, where `SaveError` is
       `File(TemplateFileError)` (`write_template_file`'s error) or `NoConfig(String)`
       (point 2).

     When ExifTool cannot be started, a load answers `TemplateFileError::ExifTool(text)`, where
     `text` is the inner text `m` of the start error `Error::ExifTool(m)` (any other error's
     `to_string()`), so the text does not say "ExifTool:" twice, and a save `SaveError::File` of
     the same. `TemplateEvent` and `SaveError` derive `Debug, Clone, PartialEq, Eq`. The thread
     itself never panics; a request it cannot answer because the receiver is gone is dropped.
- **Tests to add** (unit tests in `template_io.rs`):
  - `config_argument`: an ASCII path gives itself; a path with a letter outside ASCII (a
    folder named with `Å` made in a temporary folder) gives either `None` or an all-ASCII text
    whose `std::fs::canonicalize` equals the folder's (8.3 names may be off on the test machine;
    both are accepted, the test says why);
  - without ExifTool: `program` `Some("missing-exiftool")` (as `import_worker.rs`'s test of a
    missing program): a load answers `TemplateFileError::ExifTool(..)` and a save
    `SaveError::File(TemplateFileError::ExifTool(..))`;
  - with ExifTool on PATH (a `required_exiftool()` check that fails with an explanation when it
    is missing, as `import_worker.rs`'s tests do, `docs/TESTING.md` §4.5), in a temporary
    folder: a `TemplateFile` of Headline
    `{date}` and Keywords `a, b` with + is saved and loaded back with the same fields and `add`;
    a load of a missing file answers `TemplateFileError::NotFound`; with `config_folder` `None`
    a save answers `SaveError::NoConfig` and no file is made, while a load of the file saved
    before still works; two requests answer in order with their ids;
  - after the `TemplateIo` is dropped, the thread ends: the test takes the handle (the
    `#[cfg(test)]` method of point 1), drops the `TemplateIo`, and sees `is_finished()` within
    5 seconds.
- **Not in this task:** the window, the dialogs and where templates are kept (T291, T293); the
  import (T294, T295); the metadata worker and the import's ExifTool processes, which stay as
  they are (started without `-config`).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency and no change to
  `Cargo.toml`; in `pw-metadata` only point 0's re-export; no `unwrap`/`expect` or unchecked indexing in non-test code; build only in the
  project's `target`.
- **Beside T291:** both add lines to `pw-loupe`'s `lib.rs` (`mod` and `pub use`) and change the
  test inventory's one `pw-loupe` row and its totals. When T292 runs beside T291, its agent
  leaves the inventory's count of that row and the totals as they are and names its count in
  the report; the reviewer counts them again at the second of the two Dones.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  No CHANGELOG line (nothing the user sees until T293).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, second of the chain T291 → T292 → T293 → T294 → T295 → T296). Needs nothing of T291 and may run beside it once Open (different files). Waits for the owner's answer to U13-Q14, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
| 2026-11-23 | Blocked | Cold read: `exiftool_program` is not exported from `pw-metadata`, so point 0 re-exports it (`pw-metadata/src/lib.rs` added; two crates, but one line: the reviewer's own review covers it, the README's extra code review is for the safe write and larger changes); `start` returns `io::Result`, unbounded channel; restart rule spelled out (`is_broken`, not `restart`, config written again); no `perl` script; `NoConfig` answered before ExifTool starts, its reason made by one function with three sentence templates; the start error's inner text (no double "ExifTool:"); drop does not join, the test takes the handle through a `#[cfg(test)]` method; `config_argument` asks the file system, the Å test compares `canonicalize`; a test without ExifTool added; `TemplateIoStopped` gets T291's text (T291, T293) |
| 2026-11-23 | Open | Chain review done (see T291's log); may run beside T291 (see "Beside T291"). Set Open |
| 2026-11-23 | Ready for review | Done: `exiftool_program` re-exported from `pw-metadata`; `template_io.rs` with `TemplateIo` (own thread and ExifTool, `-config` when usable, a broken process started again with the config written again, drop without join), `config_argument` (ASCII path, else the 8.3 form by `GetShortPathNameW`, else `None`), `no_config_reason` (three sentences; the "could not be written" case names the config folder, as the file name is private to `pw-metadata`), `NoConfig` answered before ExifTool starts, the start error's inner text; a manual `Debug` for `TemplateIo` (the crate warns on a public type without one). 8 unit tests (one more than listed: the three `NoConfig` reasons). Ran beside T291: the inventory's `pw-loupe` row got the file and the tests' description, its count (1325) and the totals left as they were; T292 adds 8. `check.ps1` all OK; the four dead-code warnings in its output are from `editor.rs` (T291's work in progress), none from T292's files. `cargo fmt --all` was run during the work while `editor.rs` had T291's uncommitted changes |
| 2026-11-23 | Done | Reviewed: points 0–4 and the tests as written; the restart rule (a broken process dropped, the config written again before the next start), `NoConfig` answered before ExifTool starts, drop without join. Accepted: the "could not be written" reason names the config folder, since the file name is private to `pw-metadata`. The reviewer recounted the shared inventory row after T291's commit: 1362 -> 1370 (library 1196 passed and 1 ignored, binary 173), totals 2509 passed, 78 ignored. `.\tools\check.ps1` run again by the reviewer: all OK, no clippy warnings (the dead-code warnings the agent saw were T291's work in progress, gone). Nothing left over. Moved to `done/`; T293 Open |
