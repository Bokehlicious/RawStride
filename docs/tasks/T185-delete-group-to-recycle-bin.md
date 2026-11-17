# T185. ADR 0018 B1: an image's files to the Recycle Bin, with the drive and the files checked first (pw-metadata) — Blocked: waits for T181

- **Chain:** ADR 0018 steps B1 → B2 → B3 → B4 → B5 (`docs/tasks/plans/adr-0018-0025-steps.md`);
  this task is B1. The owner's answers are B-Q1–B-Q15 in ADR 0018 §3; B-Q14 and B-Q15 were
  answered when this task was written.
- **Why:** ADR 0018 lets the user delete an image with its companion files, always to the
  Windows Recycle Bin, never permanently and never on a memory card. This task is the file part
  only, in `pw-metadata` beside `relocate.rs` (T169): whether a drive allows a delete, the checks
  of every file of one image's group, and sending the group to the Recycle Bin with one Windows
  call. The worker request (B2) and the shell (B3, B4) come later and use what this task makes.
- **Crates:** `pw-metadata`. **Files:** `crates/pw-metadata/Cargo.toml` (only the `windows-sys`
  feature list), `crates/pw-metadata/src/recycle.rs` (new), `crates/pw-metadata/src/lib.rs` (the
  module and re-exports), `docs/test-inventory.md`, and this task's own file (status and log).
  `Cargo.lock` does not change (features are not in it); if it does, stop and say so. Read
  `docs/TESTING.md` first (`AGENTS.md` §5). **Starts after T181 is committed:** T181 also changes
  `docs/test-inventory.md`, and that file's diff must hold only this task's change.
- **What:**
  0. **Dependency features (owner, ADR 0018 B-Q11 and B-Q14):** add `"Win32_UI_Shell"` and
     `"Win32_System_Registry"` to the existing `windows-sys = "=0.61.2"` feature list in
     `pw-metadata`'s `[target.'cfg(windows)'.dependencies]`. No other change to `Cargo.toml`.
     The `DRIVE_*` constants are in `Win32_System_WindowsProgramming`, which is **not** enabled:
     write them as local `const`s with a comment, as `fileops.rs` does for
     `FILE_READ_ONLY_VOLUME` (`DRIVE_UNKNOWN` 0, `DRIVE_NO_ROOT_DIR` 1, `DRIVE_REMOVABLE` 2,
     `DRIVE_FIXED` 3, `DRIVE_REMOTE` 4, `DRIVE_CDROM` 5, `DRIVE_RAMDISK` 6). These constants and
     the decision function of point 4 are **not** behind `#[cfg(windows)]` (their tests run on
     every platform); where they are unused off Windows, `#[cfg_attr(not(windows),
     allow(dead_code))]` as `relocate.rs` does.
  1. **The module:** `pub mod recycle;` in `lib.rs`; `DeleteError` and `DeleteRefusal`
     re-exported at the crate root, as `Moved` and `MoveError` are. The module's doc comment
     says what it does and that RawStride never deletes permanently (ADR 0018 §2.6).
  2. **`pub enum DeleteRefusal`** (`Clone`, `Debug`, `PartialEq`, `Eq`), why a file or its drive
     may not be deleted, with `Display` giving the reason as an English phrase without a full
     stop, in the style of `relocate.rs`'s reasons (the texts below are the exact ones):
     - `Card` — "the drive is a memory card" (a removable drive, or a `DCIM` folder at the
       drive's root; ADR 0018 §2.6, B-Q5);
     - `NoRecycleBin` — "the drive has no Recycle Bin" (a network drive or share, an optical or
       RAM disk, a drive of unknown type, or no volume name; B-Q6);
     - `RecycleBinOff` — "the Recycle Bin is turned off for the drive" (`NukeOnDelete` not 0, or
       the policy `NoRecycleFiles` not 0; B-Q14);
     - `TooLarge { bytes: u64, capacity: u64 }` — "the files are too large for the Recycle Bin
       ({bytes} bytes, the Recycle Bin holds {capacity})" (B-Q14);
     - `RecycleBinUnknown(String)` — "the Recycle Bin's settings for the drive could not be read:
       {text}" (B-Q14: refused, never guessed);
     - `Missing` — "the file does not exist";
     - `NotAFile` — "it is not a file";
     - `OpenElsewhere` — "the file is open in another program" (the same words as
       `relocate.rs`'s `OPEN_ELSEWHERE`);
     - `NotOneFolder` — "the files are not in one folder";
     - `PathTooLong` — "the path is too long for the Recycle Bin" (a full path of 260 UTF-16
       units or more; `SHFileOperationW` does not take longer paths or the `\\?\` form);
     - `Unsupported` — "deleting to the Recycle Bin is only available on Windows";
     - `Failed(String)` — the system's error text (an `io::Error`'s own `to_string()`, without the
       crate's "I/O error: " prefix), "the file is listed twice", or for `SHFileOperationW`
       "Windows could not move the files (code {code:#x})".
  3. **`pub struct DeleteError`** (`Clone`, `Debug`, `PartialEq`, `Eq`) with `pub file: PathBuf`
     (the file, or for a drive refusal the first file of the group), `pub reason: DeleteRefusal`,
     `pub left: Vec<PathBuf>`: the files of the group that are still in place **when some of
     the group already went to the Recycle Bin** (ADR 0018 §2.4, B-Q8), empty when nothing of the
     group was deleted; and `pub deleted: Vec<PathBuf>`: the files of the group that did go to
     the Recycle Bin before the failure (empty when nothing was deleted), so the shell can drop
     them too. **Every path in a `DeleteError` (`file`, `left`, `deleted`) is the caller's path as
     given** (not the absolute or normal form used for the checks and the call), so the caller
     can match it with its own lists. `Display` gives one whole sentence: "Could not move {name} to the
     Recycle Bin: {reason}." and, when `left` is not empty, adds " Some files are already in the
     Recycle Bin; these were left: {paths}." with each path of `left` shown in full, joined by
     ", " (as `MoveError` does). `impl std::error::Error`. The structured reason is what B2's
     answer event carries (plan row B2), so the shell can tell a card from a locked file; how the
     shell shows it under rule 13 is B3's decision, not this task's.
  4. **`pub fn drive_allows_delete(path: &Path) -> Result<(), DeleteRefusal>`**: whether the
     drive of `path` (a file or a folder; made absolute with `std::path::absolute` first) allows a
     delete, without touching any file. It is a thin wrapper around a **private** function that
     returns `Result<u64, DeleteRefusal>`, the bin's capacity in bytes, which `check_group` uses
     for point 5.5. Before anything else the path is put in the form `SHFileOperationW` takes
     (point 4a). On Windows, in this order:
     1. the volume root with a `GetVolumePathNameW` call of `recycle.rs`'s own (the same pattern
        as `fileops::is_read_only_volume`, which returns only a `bool` and is not changed; a
        failure is `Failed` with the error's text);
     2. `GetDriveTypeW` of the root: `DRIVE_REMOVABLE` is `Card`; anything other than
        `DRIVE_FIXED` is `NoRecycleBin`;
     3. a `DCIM` folder at the root (`root.join("DCIM").is_dir()`, the rule of
        `pw_ingest::volumes`, which `pw-metadata` does not depend on; do not add the dependency)
        is `Card`;
     4. the volume's name with `GetVolumeNameForVolumeMountPointW` (`\\?\Volume{GUID}\`); a
        failure, or a name without a `{…}` part, is `NoRecycleBin`;
     5. the Recycle Bin settings with `RegGetValueW` and `RRF_RT_REG_DWORD`: `NoRecycleFiles` under
        `Software\Microsoft\Windows\CurrentVersion\Policies\Explorer` in `HKEY_CURRENT_USER` and
        in `HKEY_LOCAL_MACHINE` (a missing key or value counts as 0; any value not 0 is
        `RecycleBinOff`); then `NukeOnDelete` and `MaxCapacity` under
        `Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{GUID}` in
        `HKEY_CURRENT_USER` (with `{GUID}` as Windows gave it, braces included). `NukeOnDelete`
        not 0 is `RecycleBinOff`. For `NoRecycleFiles`, only `ERROR_FILE_NOT_FOUND` (2; key or
        value missing) counts as 0; any other error (access denied, a value that is not a DWORD,
        …) is `RecycleBinUnknown`. If the volume's key or either of its two values cannot be read
        (missing included), the result is `RecycleBinUnknown`. The text is the Windows error's
        (`std::io::Error::from_raw_os_error` of the returned code). The reviewer saw both values
        for every local fixed drive on the owner's machine (2026-11-17); `MaxCapacity` is in
        megabytes. A drive whose key does not exist yet (one the Recycle Bin has never been used
        on) is therefore refused; accepted (B-Q14).

     The decision itself (points 2–5 from their inputs) is a **private plain function**, for
     example `fn decide(drive_type: u32, has_dcim: bool, volume_guid: Option<&str>, policy:
     Result<bool, String>, settings: Result<BinSettings, String>) -> Result<u64, DeleteRefusal>`
     (`policy` is `Ok(true)` when either `NoRecycleFiles` is not 0, `Err` when one could not be
     read) returning the
     bin's capacity in bytes (`MaxCapacity` × 1 048 576, with `saturating_mul`), so every case is
     tested without a real drive. Its name and signature are the agent's choice; it does no
     Windows call. The Windows calls are behind `#[cfg(windows)]`; off Windows,
     `drive_allows_delete` returns `Err(DeleteRefusal::Unsupported)`. No `unsafe` block without a
     `// SAFETY:` comment, as in `fileops.rs`.

     **4a. The path form:** the folder is first resolved with `std::fs::canonicalize`, so a
     junction or a folder symlink is judged by the drive it really points to (a link on `C:` to a
     card or a drive with the bin off must not pass; reviewer, chain review, ADR 0018 §2.6); a
     folder that cannot be canonicalized is `Failed` with its text. The drive check and the
     paths given to `SHFileOperationW` use that canonical folder joined with each file's name.
     After `canonicalize` (or `std::path::absolute` where only a path is turned into a form), a verbatim disk path (`\\?\C:\…`,
     `std::path::Prefix::VerbatimDisk`, as `canonicalize` gives) is turned into the normal form
     (`C:\…`); any other verbatim or device form (`\\?\UNC\…`, `\\?\Volume{…}`, `\\.\…`) is
     `NoRecycleBin`. The same applies to each file in point 5, so `SHFileOperationW` only ever
     gets normal paths.
  5. **`pub fn check_group(files: &[PathBuf]) -> Result<u64, DeleteError>`**: every check of
     ADR 0018 §2.4 and §2.6, before anything is deleted; returns the group's total size in
     bytes. Nothing is changed on disk. In this order, the first failure returned:
     1. an empty list is `Ok(0)` (nothing to check);
     2. each path made absolute and put in the normal form (point 4a; a failure is `Failed` with
        its text); every file must have the same parent folder as the first, compared without
        regard to case (`NotOneFolder`, naming the first file that differs;
        `relocate::companions` always gives one folder); a file whose path equals an earlier
        one's without regard to case is `Failed("the file is listed twice")`, naming it;
     3. `drive_allows_delete` of the folder; a refusal names the first file;
     4. for each file in order: `std::fs::symlink_metadata` (not found is `Missing`, other errors
        `Failed`), it is a file (`NotAFile`), its full path is shorter than 260 UTF-16 units
        (`PathTooLong`), and it is not open in another program. **Not** with `fileops::is_locked`:
        it opens for writing, so a file with the read-only attribute (a camera's protected image)
        would fail with "access denied". `recycle.rs` has its own check: open with `read(true)`
        and `share_mode(0)`; a sharing or lock violation (32, 33) is `OpenElsewhere`, success is
        free, any other error `Failed` with its text. A read-only file is allowed (the Recycle
        Bin keeps its attribute). Each file's size is added up;
     5. the total larger than the drive's Recycle Bin capacity is `TooLarge` (naming the first
        file). The total is compared, not each file, so a group is never partly too large.

     Every path in an error is the caller's path as given (point 3); "the first file" is taken
     with `first()`, never `[0]`. `left` and `deleted` are always empty here.

     **`pub fn inspect_group(files: &[PathBuf]) -> Result<u64, DeleteError>`** does the same
     checks **without** the "open in another program" check of 5.4, for B2's confirmation (the
     view cores still hold the current image open then; they release it only right before the
     delete). `check_group` is `inspect_group` plus that check, sharing one implementation.
  6. **`pub fn delete_group(files: &[PathBuf]) -> Result<(), DeleteError>`**: `check_group`
     first; a failed check returns its error with nothing deleted, and an empty list is `Ok(())`
     without a Windows call. Then all files of the group
     go to the Recycle Bin in **one** `SHFileOperationW` call (ADR 0018 §2.9): `wFunc` `FO_DELETE`,
     `fFlags` `FOF_ALLOWUNDO | FOF_NO_UI` (as `u16`, with `u16::try_from` and an error, not `as`),
     `hwnd` null, `pFrom` the absolute paths as UTF-16, each ended by a NUL and the list by a
     second NUL; `pTo` null. No COM initialisation (the worker thread has none; B-Q11).
     Afterwards, each file is looked at again with `std::fs::symlink_metadata`: `NotFound` means
     it is gone; any other result, an error included, counts as still there:
     - the call returned 0, `fAnyOperationsAborted` is 0, and no file of the group exists: `Ok`;
     - otherwise, with `remaining` the files of the group that still exist: when `remaining` is
       the whole group, `Err` with `left` empty (nothing was deleted); when some are gone, `Err`
       with `left` = `remaining` (ADR 0018 §2.4: the files already in the Recycle Bin cannot be
       taken back by RawStride), and `deleted` the others (each as given); when none remain
       although the call reported a failure, `Ok`.
       `file` is the first remaining file (or the first of the group), and `reason` is `Failed`
       with "Windows could not move the files (code {code:#x})" for a code that is not 0, or
       "Windows stopped the operation" when only `fAnyOperationsAborted` was set.

     It never deletes permanently (all refusals of point 4 come first), never empties the
     Recycle Bin, and never deletes a folder. **Known limit** (ADR 0018 §2.6): `FOF_NO_UI`
     includes `FOF_NOCONFIRMATION`, so if the bin's settings change between the check and the
     call, Windows would delete permanently; the check right before the call is the only guard
     (`FOF_WANTNUKEWARNING` would show a Windows dialog on the worker thread, which §2.3
     forbids). Off Windows, `check_group` and `delete_group`
     return `Unsupported` at once (naming the first file, or an empty path for an empty list),
     with nothing touched.
  7. No other file changes. `relocate.rs`, `fileops.rs` and `worker.rs` are not changed: the
     helpers `recycle.rs` needs from `fileops.rs` (`is_locked`, `wide`) are used as they are if
     they are visible to the module; if `wide` is private, `recycle.rs` has its own small
     conversion (with `std::os::windows::ffi::OsStrExt`) rather than changing `fileops.rs`.
- **Tests to add** (in `recycle.rs`, each in a temporary folder of its own; a small temporary-folder
  helper of the module's own, as `relocate.rs` has):
  - **The decision (every platform):** the plain function of point 4 with each case:
    `DRIVE_REMOVABLE` is `Card`; `DRIVE_FIXED` with `DCIM` at the root is `Card`; `DRIVE_REMOTE`,
    `DRIVE_CDROM`, `DRIVE_RAMDISK`, `DRIVE_UNKNOWN` and `DRIVE_NO_ROOT_DIR` are `NoRecycleBin`;
    a fixed drive without a volume GUID is `NoRecycleBin`; the policy on is `RecycleBinOff`;
    `NukeOnDelete` 1 is `RecycleBinOff`; settings that could not be read are
    `RecycleBinUnknown` with the text; a fixed drive with the bin on gives the capacity in bytes
    (and a huge `MaxCapacity` does not overflow).
  - **The texts (every platform):** `Display` of each `DeleteRefusal` variant as in point 2, and
    of `DeleteError` with and without `left` as in point 3.
  - **The checks (Windows; these put nothing in the Recycle Bin):** `drive_allows_delete` of the
    test's temporary folder is `Ok` (it is on a fixed drive with the bin on; if this machine's
    temporary folder is not, the test fails with the refusal in its message, and the agent stops
    and reports it); `check_group` returns the total size of three files; an empty list, a
    missing file, a folder in the list, two files from two folders, and a file held open (opened
    with `std::os::windows::fs::OpenOptionsExt::share_mode` of read and write only, not first in
    the group, as in `relocate.rs`) each give their refusal, naming the right file, with every
    file still in place, and `inspect_group` gives `Ok` with the size for the group with the held
    file (the folder in the list is a sub-folder of the same temporary folder,
    so it reaches `NotAFile`, not `NotOneFolder`); a read-only file passes; the same file twice
    (once in other case) is "the file is listed twice"; the `\\?\` form of a file in the
    temporary folder is accepted and named in the normal form; a relative path to a file that
    does not exist
    (`pw-metadata-recycle-no-such-file.ARW`) is `Missing` with `file` equal to the relative
    path as given (the test changes no process state); a folder reached through a directory
    junction or symlink is judged by its target's drive (a test with a junction to the
    temporary folder gives the same result as the folder itself; if a junction cannot be made
    without rights this machine lacks, the agent says so and leaves the test out). These
    tests depend on the temporary folder's drive and the crate folder's drive passing point 4;
    if one does not, the test fails with the refusal in its message and the agent stops and
    reports it.
  - **The real Recycle Bin (Windows, `#[ignore]` by B-Q15; run by hand):**
    - `delete_group` of three files (an `.ARW`, its `.ARW.dop` and its `.XMP`, a few bytes each,
      names starting `pw-metadata-recycle-test-`) returns `Ok` within 30 seconds, the call made on
      a thread spawned by the test (no COM there; a Windows dialog would block it, so a timeout
      fails the test with "a Windows dialog may be open"), and afterwards none of the three
      exists, and `SHQueryRecycleBinW` on the drive's root (`cbSize` set) shows at least three
      items more than before. This count can be disturbed (the bin evicting old items, or being
      emptied meanwhile); a failure is reported as it is, not retried or loosened.
    - The partial case, in a temporary folder of its own (after a timeout, the stuck thread and
      the held handle are left alone until the process ends): two files and a third held open
      without delete sharing are passed to
      the private function that makes the Windows call, **bypassing `check_group`**; within 30
      seconds it returns an error (no dialog), and `left` holds the held file and only files
      that still exist (Windows decides whether the other two went first; the test asserts
      what is true either way: `left` empty means all three still exist, otherwise the files in
      `left` exist and the others do not).

    A real network drive is not tested here: the decision tests cover `DRIVE_REMOTE` and a
    missing volume name, and the owner's hand check in B4 refuses a network folder.

    The command is `cargo test --workspace -- --ignored recycle::` (`AGENTS.md` §5 allows
    `cargo test --workspace`). The agent runs it once and pastes the result in the report; the
    files it puts in the Recycle Bin stay there (the owner approved this, B-Q15).
    `.\tools\check.ps1 -Ignored` runs these tests as well and so also puts files in the Recycle
    Bin.
- **Not in this task:** the worker request, its answer event and releasing the view cores (B2);
  the shell's targets, confirmation, texts and lists (B3); keys, menu item and window (B4);
  "Delete Rejected Images" (B5). A RAW+JPEG pair on two drives does not occur (`companions` is
  one folder).
- **Rules:** `AGENTS.md` §3 (no panic paths; `unsafe` only around the Windows calls, each with a
  `// SAFETY:` comment); the two features of point 0 are the only change to `Cargo.toml`
  (approved, B-Q11 and B-Q14); no user-visible text in the shell from this task (the reasons are
  the lower crate's English phrases, as the safe write's and `relocate.rs`'s are).
- **Done when:** `.\tools\check.ps1` is all OK with no clippy warnings, the ignored tests have
  been run once with the command above and their output is in the report, and `git diff --stat`
  shows no change from this task outside the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added for ADR 0018 B1. B-Q14 (refuse where Windows would delete permanently: bin off for the drive, files too large for it, settings not readable; feature `Win32_System_Registry`) and B-Q15 (tests with the real Recycle Bin `#[ignore]`) answered by the owner and written into ADR 0018 §2.6, §2.9 and §3 |
| 2026-11-17 | Blocked: waits for T181 | Cold read (fresh agent; windows-sys names, types, features and `fileops` visibility confirmed). Changed: starts after T181 is committed (`test-inventory.md`); `check_group` gets the capacity from a private function behind `drive_allows_delete`; own `GetVolumePathNameW` call; `NoRecycleFiles` errors other than "not found" are `RecycleBinUnknown`, and the decision takes them; own lock check with read access, so a read-only file is not refused as "access denied"; `Failed` without the "I/O error: " prefix; `\\?\C:\` paths turned into the normal form, other verbatim forms refused (4a); an empty list is `Ok`; the folder compared without regard to case and a file listed twice refused; the look after the call defined; the decision's constants not behind `cfg(windows)`; the known limit (settings changed between check and call) written here and in ADR 0018 §2.6; the network test dropped (it would make every ignored run fail without a share; the decision tests and B4's hand check cover it); notes on the tests' drives, the bin count and the partial case |
| 2026-11-17 | Blocked: waits for T181 | When B2–B5 were written: `inspect_group` (point 5) added, the checks without the lock check, for B2's confirmation; tested with the held file |
| 2026-11-17 | Blocked: waits for T181 | Chain review (fresh agent, T185–T190 read together): every path in a `DeleteError` is the caller's as given, so the shell can match it; `DeleteError::deleted` added, so a partly failed group's deleted files leave the tab and caches (T187, T189); the folder canonicalized before the drive check, so a junction or folder symlink to a card or a drive with the bin off cannot pass (recorded in ADR 0018 §2.6); tests adjusted |
