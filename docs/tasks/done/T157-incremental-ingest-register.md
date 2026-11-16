# T157. Incremental ingest: the register on the card (pw-ingest) — Done

- **Why:** the owner's job test (2026-11-16) missed "copy new photos only". **Owner, 2026-11-16**
  (ADR 0005 choice 6): RawStride keeps its own register `.rawstride-ingest.txt` in the root of the
  card. Files in it are left out of the next import, and a line is added after each verified file.
  This task is the engine part. T158 adds the dialog's check box, the texts and the setting, and
  waits for this task.
- **Crates:** `pw-ingest`. **Files:** `crates/pw-ingest/src/register.rs` (new),
  `crates/pw-ingest/src/lib.rs`, `crates/pw-ingest/tests/ingest.rs`, `docs/test-inventory.md`.
- **What:**
  1. **The file** (`register.rs`): `pub const REGISTER_FILE: &str = ".rawstride-ingest.txt";`. It is
     UTF-8. The first line is exactly `RawStride ingest register 1`, then one line per file:
     `{path}\t{size}\t{modified}`. `{path}` is the file's path below the register's root, with `/`
     between the parts. `{size}` is in bytes. `{modified}` is whole seconds since 1970 (UTC, as
     `secs` in `lib.rs`). Every line ends with `\n`. A tab or line break cannot be in a FAT or exFAT
     file name; a path that holds one anyway is not written (it counts as not imported next time).
  2. **The root:** `pub fn register_root(source: &Path) -> Option<PathBuf>` gives `{drive}\` from
     `drive_of` (for `E:\DCIM` it is `E:\`). A source without a drive letter gives `None`, and then
     there is no register.
  3. **Reading:** `pub struct Register` (derive `Debug, Default`) and
     `pub fn read_register(root: &Path) -> Register`.
     - No file gives an empty register.
     - A file that cannot be read, or whose first line is not the version line above, gives an
       empty register marked *foreign*: `Register::is_foreign()`. The writer then will not touch it
       (point 5).
     - Correction after the cold read: only a file whose **first line exists and is another
       version line** is foreign. An empty (0-byte) file is an empty register that may be written
       (the version line is then written first). A file that exists but cannot be read gives an
       empty register with `Register::read_error() -> Option<&str>`. `open` then refuses with the
       general "could not be marked: {error}" text, not the foreign one.
     - A line that does not parse is skipped.
     - `Register::contains(root: &Path, file: &SourceFile) -> bool`: the path below `root` compared
       without regard to case (Windows), and the same size and the same `modified` seconds. A file
       not below `root`, or whose time is before 1970 (`crate::secs` gives `None`), is never
       contained and never written.
  4. **Leaving out:** `pub fn new_shots(shots: &[Shot], root: &Path, register: &Register) -> (Vec<Shot>, usize)`
     gives the shots that are not in the register, and the number of files left out (every file
     of the left-out shots, companions included). A shot is left out only when **every** file of it
     whose extension (lower-cased: `SourceFile::ext` keeps its case) is not in
     `COMPANION_EXTENSIONS` is in the register; its companions go with it. The order of the shots is kept, so `plan` gives the new
     shots running numbers from `first_seq` with no gaps.
  5. **Writing:** `pub struct RegisterWriter` (derive `Debug`) with
     `pub fn open(root: &Path, register: &Register) -> Result<RegisterWriter, String>` and
     `pub fn add(&mut self, file: &Path, size: u64, modified: SystemTime) -> Result<(), String>`
     (the writer keeps the root from `open`).
     - `open` refuses a foreign register, and an unreadable one (point 3). Opening for appending
       also fails on a read-only file or a locked card (Windows: os error 5); that `Err` is how the
       caller learns that the card cannot be marked (T158 shows it).
     - `add` returns `Ok(())` and writes nothing for a path not below the root, with a tab or line
       break in it, or with a time before 1970. Every other item is written, companions (`.xmp`)
       included; a line written twice is harmless (`contains` reads a set).
     - It creates the file with its version line when there is none, else opens it for appending.
       A file that does not end with `\n` gets one first.
     - `add` writes one line, then `flush` and `sync_data`, so that a card pulled out loses at most
       the line being written.
     - The error texts (rule 13; made in `register.rs`, each one whole sentence):
       - `"The card could not be marked as imported: {error}."` (open or write);
       - `"The card could not be marked as imported: its file {REGISTER_FILE} was not made by this version of RawStride."` (foreign).
     - `pub fn Register::problem(&self) -> Option<String>` gives the same text `open` would refuse
       with (foreign or unreadable) **without** opening the file, so that T158's preview can show
       it without writing to the card.
  6. **Running:** a new `pub fn execute_recording(plan, opt, cancel, register: Option<&mut RegisterWriter>, progress) -> Report`.
     `execute` keeps its signature and calls it with `None`, so the Tauri app and the example are
     unchanged. With a register:
     - every item that needs no copy (`!to_dest && !to_backup`) is added before the first copy;
     - every item copied and verified to all its targets is added right after, **before**
       `progress` is called for it (so a cancel set in `progress` cannot lose its line);
     - a failed, cancelled or half-copied item is not added;
     - the first error of `add` is kept in a new field `Report::register_error: Option<String>`.
       After it nothing more is written, and the copying goes on.
  7. **Docs:** the crate's doc comment (`lib.rs`) says "The source is only ever opened for reading."
     Change it to say that the source's images are only read, and that RawStride's own register
     file in the card's root is the one file it writes there (ADR 0005 choice 6). Export the new
     items from `lib.rs`.
- **Tests to add:**
  - Unit (`register.rs`):
    - `register_root` for `E:\DCIM`, `e:`, `\\server\share` and `/media/card`.
    - Reading: no file; a good file; a file with a bad line in the middle (the other lines count);
      a foreign first line (`is_foreign`); an upper/lower-case path difference still matches; another
      size or time does not match.
    - `new_shots`: a shot left out when all its images are in; kept when one of its images is
      missing (a RAW in, its JPEG not); a companion alone does not keep a shot; the order and the
      count of files left out.
    - The writer: a new file gets the version line; a 0-byte file gets the version line; appending
      to an existing file; a file without a final `\n`; a foreign register is refused with the text
      above; a path with a tab and a path outside the root are not written.
  - Integration (`tests/ingest.rs`, temporary folders, with the temporary source folder as the
    root):
    - an import with a register adds a line per copied file and per file already present;
    - a second scan → `new_shots` → `plan` → `execute_recording` copies nothing, even into a new
      destination with another name template;
    - after a new file is added to the source, the next import into a **fresh** destination with a
      `{seq}` name template copies only it, named with `first_seq` (into the old destination the
      name would clash with the earlier file and get `_1`);
    - `#[cfg(windows)]`: with the register file set read-only, `RegisterWriter::open` returns the
      "could not be marked" `Err`, and `execute_recording(…, None, …)` still copies everything; the
      test clears the read-only flag before it ends, so the temporary folder can be removed;
    - a cancelled run (cancel set from the progress callback after the first file, in a fresh
      destination) has a line only for that file.
- **Not in this task:** the dialog, the setting and the texts the user sees in `pw-loupe` (T158);
  the Tauri app (`src-tauri`) keeps working as today without a register.
- **Rules:** AGENTS.md §3 (no panics; every file error is a `Result`); `docs/tasks/README.md` rule 13
  for the two error texts. No new dependencies.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above
  (besides files of other tasks still uncommitted in the tree, which are left as they are).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added after the owner's job test (bug 3 of 5); the owner's decision is ADR 0005 choice 6. T158 waits for this task |
| 2026-11-16 | Open | Cold read: a read-only register makes `open` fail, so the test now expects that `Err` and runs without a register; the line is added before `progress`; the `{seq}` and cancel tests use a fresh destination; a 0-byte file is empty, not foreign, and an unreadable one gives the general text; paths outside the root, with a tab or before 1970 are skipped; the writer keeps the root; companions are counted and written; `Register::problem` added for T158's preview |
| 2026-11-16 | Ready for review | `register.rs` (new), `lib.rs` (`execute_recording`, `Report::register_error`, doc comment, exports), `tests/ingest.rs`, `docs/test-inventory.md`. 20 unit and 5 integration tests added (one Windows only). `.\tools\check.ps1` all OK: 1649 passed, 63 ignored, 0 failed; clippy without warnings. Beyond the text, for the reviewer: `RegisterWriter::open` writes the version line at once (so a locked card fails in `open`, also with no file there) and reads the file's first line itself, so a file that became foreign after `read_register` is refused too; a path that is not Unicode is not written, like one with a tab (the file is UTF-8); a shot with companions only is never left out; a line with more or fewer than three fields is skipped |
| 2026-11-16 | Done | Reviewed: no bugs; `.\tools\check.ps1` all OK (1649 passed, 63 ignored, 0 failed), clippy without warnings. Leftovers: (1) a FAT32 card's times move an hour when daylight saving time changes, so the register and the destination check miss every file: T162; (2) a source without a drive letter (also a `\\?\` path) has no register: T158 already gives the note `NoRegisterRoot` and passes the validated source; (3) a file whose modification time cannot be read is scanned as 1970 and written with time 0, and (4) no test for an `add` that fails in the middle of a run (a read-only flag set after `open` does not stop an open handle on Windows): both in `docs/test-inventory.md` |
