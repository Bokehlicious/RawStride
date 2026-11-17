# T169. ADR 0017 U9b, first part: moving an image with its companion files — Open

- **Chain:** T169 → T170 → T171 → T172 → T173 → T174. The owner's answers U9-Q5–U9-Q10 are in ADR 0017 §6.1,
  the companion files in §2.6 (with R2, 2026-11-17).
- **Why:** ADR 0017 §2.6, step U9b: with the setting on, a reject moves the image with all its
  companion files to `_rejected` in its own folder, and P to `_picked`. The move is all or
  nothing, and undo moves the files back. This task is the file part only, in `pw-metadata`
  beside the safe write's file operations (`fileops.rs`, whose `move_new` never overwrites a
  file): which files belong to an image, and moving a group of files there and back.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/src/relocate.rs` (new),
  `crates/pw-metadata/src/lib.rs` (the module), `docs/test-inventory.md`, and this task's own file
  (status and log).
- **What:**
  0. **The module:** `pub mod relocate;` in `lib.rs`, and `Moved` and `MoveError` re-exported at
     the crate root. Extensions and suffixes are given without the leading dot, in any case.
  1. **`pub fn companions(image: &Path, image_extensions: &[&str], suffixes: &[&str]) ->
     std::io::Result<Vec<PathBuf>>`**: reads the image's folder once and returns the files (not
     folders) that move with it, the image first. With `base` the image's name without its
     extension and `name` its whole name, and all names compared **without regard to case**
     (Windows; R2 saw `.ARW.dop` and `.XMP`):
     - the image;
     - its partners: every other file `<base>.<e>` whose extension `e` is in `image_extensions`
       (the RAW+JPEG partner; the caller passes the listed image types);
     - for the image and each partner, with `m` its whole name: `<m>.xmp` and `<m>.<s>` for each
       suffix `s`;
     - `<base>.xmp` and `<base>.<s>` for each suffix `s`.

     After the image come the partners, then the other files by name (lower case). A file is
     listed once. An image that does not exist is an error.
  2. **`pub struct Moved { pub from: PathBuf, pub to: PathBuf }`** (`Clone`, `Debug`,
     `PartialEq`, `Eq`) and **`pub struct MoveError`** with `pub file: PathBuf` (the file that
     could not be moved), `pub reason: String` (English, naming the cause: the name is taken in
     the target folder, the file is open in another program, or the system's error text) and
     `pub left: Vec<Moved>` (files that could not be moved back; empty when the rollback was
     complete). `move_new` returns `Error::Io` with the Windows error; `relocate.rs` reads its
     `raw_os_error()` itself (`fileops.rs` is not changed): 32 (`ERROR_SHARING_VIOLATION`) is "the
     file is open in another program", 80 or 183 (`ERROR_FILE_EXISTS`, `ERROR_ALREADY_EXISTS`)
     "the name is taken in the target folder", anything else the error's own text. `Display` gives a whole sentence with the file's name and the reason; when
     `left` is not empty it adds that some files stayed in the target folder and names them.
  3. **`pub fn move_group(files: &[PathBuf], target: &Path) -> Result<Vec<Moved>, MoveError>`**:
     creates `target` if it does not exist (one level; an error if it exists and is not a
     folder). Before anything is moved, every `target/<file name>` is checked: a name that is
     taken fails the whole group with nothing moved. Then the files are moved in order with
     `fileops::move_new` (never overwriting). If one fails, the files already moved are moved back
     in reverse order with `move_new`, and the error is returned (with `left` as above). A
     `target` folder this call created is removed again after a failure if it is empty (a failure
     to remove it is ignored). On success, the moves in order.
  4. **`pub fn move_back(moved: &[Moved]) -> Result<(), MoveError>`**: the same rules the other
     way (`to` → `from`): every `from` checked free first; then moved in reverse order; on a
     failure, the files already moved back are moved to `to` again. The target folder is left as
     it is (it may hold other images).
  5. No other file changes. No journal: a crash between two moves leaves the moved files in the
     target folder, where the user finds them (accepted, reviewer 2026-11-17; the safe write's
     journal is for replacing a file's content, which a move never does).
- **Tests to add** (in `relocate.rs`, in a temporary folder of each test's own; `fileops.rs`'s test
  helpers are private, so `relocate.rs`'s tests have their own small temporary-folder helper. The
  `companions` tests run on every platform, the move tests on Windows only, where `move_new` is):
  - `companions`: an ARW with `<base>.ARW.dop` and `<base>.XMP` gives the three, the image first
    (R2's case, upper-case extensions); a JPG partner with `<base>.JPG.xmp` and a `<base>.dop`;
    names in other case are found; another image `<base>2.ARW`, a folder named like a companion,
    and a suffix not in the list are not; no file is listed twice; a missing image is an error.
  - `move_group`: all files moved into a new `_rejected` and the moves returned in order; a name
    taken in the target folder moves nothing and names the file; a file held open **that is not the
    first of the group** (opened with `std::os::windows::fs::OpenOptionsExt::share_mode` of read
    and write only, without delete; Rust's default share mode allows the move) makes the group
    fail with the sharing-violation reason, every file moved before it comes back, and the created
    folder is removed; an existing target folder is kept.
  - `move_back`: the files return; a name taken at the original place moves nothing back.
- **Not in this task:** the worker request and releasing the view cores (T170); the shell's
  rules, keys, undo list and texts (T171, T172).
- **Rules:** AGENTS.md §3; no user-visible text in the shell from this task (the reasons are the
  lower crate's English sentences, as the safe write's).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added for ADR 0017 U9b after R2; the owner's answers are U9-Q5–U9-Q10 in ADR 0017 §6.1 |
| 2026-11-17 | Open | Cold read: doable with the named files; `move_new` never overwrites and stays on one volume, as needed. Changed: the module and re-exports named (point 0); suffixes `&[&str]`, without dots; the Windows error codes read in `relocate.rs` (32, 80, 183), `fileops.rs` unchanged; own test helper; the held file opened without delete sharing and not first in the group, so the rollback is tested; `companions` tests on every platform |
