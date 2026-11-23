# T292. Reading and saving template files on a thread of their own — Blocked

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
- **Crates:** `pw-loupe`. **Files:** `crates/pw-loupe/src/template_io.rs` (new),
  `crates/pw-loupe/src/lib.rs`, `docs/test-inventory.md`.
- **Background (read before starting):** `pw_metadata::template_file` (the module comment on
  `-config`, `write_exiftool_config`, `EXIFTOOL_CONFIG`, `TemplateFile`, `TemplateRead`,
  `TemplateFileError`, `ExifTool::write_template_file`, `ExifTool::read_template_file`);
  `pw_metadata::ExifTool::start_with` (the `pre_args` come before `-stay_open`) and the program
  `ExifTool::start` uses (`exiftool_program`); `crates/pw-metadata/tests/template_file.rs`
  (how its tests start ExifTool with `-config`); `import_worker.rs` (a worker thread with a
  request channel and `try_recv`, and its tests that need ExifTool on PATH,
  `docs/TESTING.md` §4.5).
- **What:**
  1. **`template_io.rs`** (new module, exported from `lib.rs`): `TemplateIo`, a thread that owns
     one `ExifTool` for template files. `TemplateIo::start(program: Option<String>,
     config_folder: Option<PathBuf>)` starts the thread and returns at once; ExifTool is started
     on the thread at the first request (`program`, or the program `ExifTool::start` uses when
     `None`), and again at the next request when it could not be started or has stopped. The
     thread ends, and its ExifTool with it, when the `TemplateIo` is dropped. No version check:
     a template file is a new XMP file, not a safe write of an image.
  2. **The config** (`config_folder` is `%LOCALAPPDATA%\RawStride`, given by T293; `None`
     without `LOCALAPPDATA`): before ExifTool is started, `write_exiftool_config(config_folder)`
     writes `rawstride-exiftool.config` there, and ExifTool is started with
     `-config <path>` as its first arguments (after the script with `perl`). The path goes on
     ExifTool's command line, where Windows passes only the letters of the system's code page,
     so a pure function `config_argument(path: &Path) -> Option<String>` decides the text passed:
     - the path itself when it is all ASCII;
     - otherwise, on Windows, its short (8.3) form (`GetShortPathNameW`, `windows-sys`'s
       `Win32_Storage_FileSystem`, already among `pw-loupe`'s features) when that is all ASCII;
     - otherwise `None`.

     Without `config_folder`, when `write_exiftool_config` fails, or when `config_argument` gives
     `None`, ExifTool is started **without** `-config`; loading works as before (reading needs no
     config, T285), and every save answers `SaveError::NoConfig(reason)` (point 4) without
     asking ExifTool to write anything. The reason is a short English text naming what failed
     and the path (it goes into T291's `Not saved: ExifTool cannot use its settings file for
     templates ({reason}).`).
  3. **Requests:** `load(path: PathBuf) -> Result<u64, TemplateIoStopped>` and
     `save(path: PathBuf, template: TemplateFile) -> Result<u64, TemplateIoStopped>`, each
     returning a new request id (counting from 1), answered in order. `TemplateIoStopped` is the
     error when the thread has ended (the channel is closed); the shell shows it as a failed
     load or save.
  4. **Answers:** `try_recv() -> Option<TemplateEvent>`, with
     - `TemplateEvent::Loaded { id, path, result: Result<TemplateRead, TemplateFileError> }`
       (`read_template_file`);
     - `TemplateEvent::Saved { id, path, result: Result<(), SaveError> }`, where `SaveError` is
       `File(TemplateFileError)` (`write_template_file`'s error) or `NoConfig(String)`
       (point 2).

     When ExifTool cannot be started, a load answers `TemplateFileError::ExifTool(<the start
     error's text>)` and a save `SaveError::File` of the same. The thread itself never panics; a
     request it cannot answer because the receiver is gone is dropped.
- **Tests to add** (unit tests in `template_io.rs`):
  - `config_argument`: an ASCII path gives itself; a path with a letter outside ASCII (a
    folder named with `Å` made in a temporary folder) gives either `None` or an ASCII text
    naming the same folder (8.3 names may be off on the test machine; both are accepted, the
    test says why);
  - with ExifTool on PATH (fails with an explanation when it is missing, as `import_worker.rs`'s
    tests do, `docs/TESTING.md` §4.5), in a temporary folder: a `TemplateFile` of Headline
    `{date}` and Keywords `a, b` with + is saved and loaded back with the same fields and `add`;
    a load of a missing file answers `TemplateFileError::NotFound`; with `config_folder` `None`
    a save answers `SaveError::NoConfig` and no file is made, while a load of the file saved
    before still works; two requests answer in order with their ids;
  - after the `TemplateIo` is dropped, the thread ends (for example its join handle finishes
    within a few seconds).
- **Not in this task:** the window, the dialogs and where templates are kept (T291, T293); the
  import (T294, T295); the metadata worker and the import's ExifTool processes, which stay as
  they are (started without `-config`).
- **Rules:** `docs/tasks/README.md` rules 1–13; no new dependency and no change to
  `Cargo.toml`; no `unwrap`/`expect` or unchecked indexing in non-test code; build only in the
  project's `target`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-23 | Blocked | Added (ADR 0017 U13, part U13c, second of the chain T291 → T292 → T293 → T294 → T295 → T296). Needs nothing of T291 and may run beside it once Open (different files). Waits for the owner's answer to U13-Q14, its cold read and the chain review |
| 2026-11-23 | Blocked | The owner approved the reviewer's proposals U13-Q14–U13-Q19 (recorded in ADR 0017 §6.5); waits for its cold read and the chain review |
