# T85. ADR 0007 I8b: Windows file operations and the write journal — Done

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
