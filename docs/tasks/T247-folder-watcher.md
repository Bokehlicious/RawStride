# T247. ADR 0026 F2a: the folder watcher, one thread with `ReadDirectoryChangesW` (pw-loupe) — Open

- **Why:** ADR 0026 step F2 (accepted by the owner 2026-11-18, F-Q2, F-Q3, F-Q7): a tab is to
  follow files that come and go on disk. This task makes the watcher on its own, in plain, tested
  Rust apart from the Windows calls (ADR 0026 §7): one thread, one directory handle per watched
  folder, Windows' change notifications through the `windows-sys` that `pw-loupe` already uses,
  and the quiet moment, after which it says "this folder changed" (§3.2). Nothing in the shell
  uses it yet; T248 connects it to the tabs and ends the watches before an eject, and F3 makes the
  tabs follow it. First task of the chain T247 → T248 (plan, ADR 0026 F2 row).
- **Crate:** `pw-loupe`. **Files:**
  - `crates/pw-loupe/src/watcher.rs` (new: points 1–7 and its tests)
  - `crates/pw-loupe/src/lib.rs` (the `mod watcher;` line and the `pub use` of point 1's items,
    as the other modules' items are re-exported; nothing else)
  - `crates/pw-loupe/Cargo.toml` (point 8: one feature name added to the existing `windows-sys`
    line; approved by the owner in ADR 0026 F-Q2)
  - `docs/test-inventory.md` (counts and the description of `pw-loupe`'s tests)
  - this task's own file (status and log)
- **What:**
  1. **The interface** (`watcher.rs`), used by T248 from `main.rs`:
     - `pub struct FolderWatcher`, `FolderWatcher::start(wake: Box<dyn Fn() + Send>) -> Self`:
       starts the thread (named `pw-folder-watcher`, `std::thread::Builder` as
       `inspector.rs`'s `ExifReader::start`). `wake` is called on the watcher thread after each
       event is sent, so the shell can give it `egui::Context::request_repaint` (an idle window
       draws no frames). When the thread or its completion port cannot be made, `start` still
       returns a watcher: it logs the reason with `eprintln!("pw-loupe: …")` as `ExifReader`
       does, and every folder later given to it is answered with `WatchEvent::Ended` (point 5),
       so the shell falls back as for a folder that cannot be watched (F-Q3).
     - `pub fn set_folders(&mut self, folders: Vec<PathBuf>)`: the folders to watch from now on,
       the whole set each time. It does not block: the thread opens a watch for each folder not
       watched yet and ends each watch whose folder is no longer in the set. Folders that
       `tabs::same_folder` calls the same get one watch (ADR 0026 §3.2, "one handle per open
       folder"); the first spelling given is the one events name.
     - `pub fn wait_until_applied(&self, wait: Duration) -> bool`: blocks until the thread has
       applied the last `set_folders` (every ended watch's handle closed, point 4) or `wait` has
       passed; true when applied. Used by T248 before an import that ejects its card. With no
       thread (point 1's first bullet) it returns true at once: no handle is open.
     - `pub fn try_recv(&mut self) -> Option<WatchEvent>`, never blocking.
     - `pub enum WatchEvent { Changed(PathBuf), Ended { folder: PathBuf, reason: String } }`:
       `Changed` after a quiet moment (point 3); `Ended` when a watch cannot start (the folder
       does not exist, cannot be opened, or the volume does not support notifications) or stops
       (an error completion, for example the drive is gone or the folder was deleted). `reason`
       is the system error's text, for the log only; it is not shown to the user (F3 writes the
       status texts). After `Ended`, the folder is not watched again until it has left the set
       and been given again: point 7's set function keeps an ended folder as an entry with no
       handle, so the next `set_folders` with it does not open it again; when the folder leaves
       the set the entry goes, and when it comes back it is opened again. A folder already
       watched under one spelling and given again under another keeps the first spelling. In
       the no-thread case (point 1's first bullet) `set_folders` queues the `Ended` events itself
       and `wake` is not called (the shell reads them in the same frame).
     - `Drop`: ends every watch (point 4), then the thread, and returns only when the thread has
       closed every handle, waiting at most 2 s through a "done" channel's `recv_timeout`
       (`JoinHandle` has no timed join). A thread stuck opening an unreachable network folder is
       then left to end on its own (logged).
     - **Who owns what** (so nothing Windows still writes into is freed): every read buffer and
       `OVERLAPPED` is owned by the watcher thread only. If the thread ever gives up waiting for
       a cancelled read's completion (at its own end, point 4), it leaks that watch's buffer and
       `OVERLAPPED` (`Box::leak`) instead of freeing them. The completion port's handle is held
       in a small wrapper shared through an `Arc` by `FolderWatcher` and the thread, which closes
       it on the last drop, so neither side posts to or waits on a closed port.
  2. **What is watched:** only the folder itself, not its subfolders (`bWatchSubtree` false, ADR
     0026 §2 point 7), and only file names coming and going (`FILE_NOTIFY_CHANGE_FILE_NAME`; a
     file that only changed is not followed, F-Q7).
  3. **The quiet moment** (§3.2): a plain-Rust `QuietMoment` in `watcher.rs`, tested on its
     own with given `Instant`s. `notice(folder, now)` notes a notification; a folder is due when
     no notification has come for `QUIET` (0.5 s) since its last, or `MOST` (2 s) has passed since
     its first notification of this round while they keep coming; `take_due(now)` returns the
     due folders and forgets them; `next_due(now) -> Option<Duration>` is how long the thread
     may sleep (its `GetQueuedCompletionStatus` timeout; `INFINITE` when nothing waits, a local
     `const INFINITE: u32 = u32::MAX;`, since `windows-sys` has it only under
     `Win32_System_Threading`, which is not enabled). The two
     durations are named constants with ADR 0026 §3.2 in their doc comments. Each due folder is
     sent as one `WatchEvent::Changed`.
  4. **Ending a watch** (also before an eject, §3.2): the read is cancelled (`CancelIoEx`), the
     thread waits for that read's completion (Windows writes into the buffer and the `OVERLAPPED`
     until then, so both are freed only after it), then closes the handle. Each watch keeps a
     "read pending" flag: a watch with no read pending is closed at once, and `CancelIoEx`
     failing with `ERROR_NOT_FOUND` means the read's completion is already queued, so the
     thread still waits for that packet. The wait is in the thread's main loop (the watch is
     marked "ending"), not a blocking wait on one watch, so other folders' packets and control
     messages keep being handled meanwhile. A folder that left the set gives no event.
     "Applied" in `wait_until_applied` means this is done for every folder that left the set.
  5. **The Windows part** (`#[cfg(windows)]`, kept small; everything it decides is in tested
     plain-Rust functions):
     - the folder is opened with `std::fs::OpenOptions` and
       `std::os::windows::fs::OpenOptionsExt` (`.read(false)`, `.access_mode(FILE_LIST_DIRECTORY)`,
       `.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)` so the user can still
       rename or delete files and the folder itself,
       `.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED)`; not creating gives
       `OPEN_EXISTING`), as `pw-ingest`'s `eject.rs` opens a volume, not with `CreateFileW`,
       which in `windows-sys` 0.61.2 needs the `Win32_Security` feature. The `File` owns the
       handle and closes it when dropped (point 4's order still holds: dropped only after the
       read's completion). The handle (`as_raw_handle`) is associated with the thread's one I/O
       completion port (`CreateIoCompletionPort`);
     - completion keys are never reused: a counter from 1 per watch opened, 0 for the control
       post; a packet is matched to its watch by key and `OVERLAPPED` pointer, and one matching
       no live or ending watch is ignored;
     - `GetQueuedCompletionStatus` returning FALSE with a null `OVERLAPPED` is a timeout (or a
       port error, logged); FALSE with a non-null `OVERLAPPED` is a failed read of that watch
       (`ERROR_OPERATION_ABORTED` after a cancel, otherwise the watch ends with `Ended`);
     - `ReadDirectoryChangesW` with a DWORD-aligned buffer (for example a `Box<[u32]>` of
       16 KiB; network folders allow at most 64 KiB) and a boxed `OVERLAPPED`; after each
       completion the read is started again;
     - the thread waits in `GetQueuedCompletionStatus`; `set_folders`, `Drop` and the applied
       signal reach it through an `mpsc` channel plus `PostQueuedCompletionStatus` with a key no
       watch uses, so a waiting thread wakes at once;
     - a completion with an error (the drive gone, the folder deleted, access lost) or a read
       that cannot be started again ends the watch (point 4's order) and sends `Ended`; its
       `reason` is `std::io::Error::from_raw_os_error(code).to_string()`;
     - every `unsafe` block has a `// SAFETY:` comment saying why the pointers are valid for as
       long as Windows uses them; no `unwrap`, `expect`, `panic!` or unchecked indexing
       (`AGENTS.md` §3).
     - `#[cfg(not(windows))]`: no thread; every folder given is answered with `Ended` (reason:
       not supported), so the crate still builds elsewhere.
  6. **Which notifications count** (plain Rust, tested with built byte buffers): a function
     taking `&[u8]` (the thread copies the completion's bytes out of its `u32` buffer with
     `to_ne_bytes`; no `unsafe` cast, no new dependency) reads the buffer's `FILE_NOTIFY_INFORMATION` records (`NextEntryOffset`, `FileNameLength`
     in bytes, the name as UTF-16) with checked offsets (`get`, never indexing past the
     completion's byte count) and says whether any record names a file `crate::is_listed`
     lists. Only then is the folder `notice`d. A completion with 0 bytes (Windows' buffer
     overflowed and the records are lost) or a record that cannot be read counts as a change:
     the shell lists the folder again either way, so a wrong "changed" only costs one listing,
     a wrong "not changed" loses a file. Reason: RawStride's own rating writes to RAW files go to
     `.xmp` sidecars through temporary names, and the safe write's `.pwtmp-`/`.pwbak-` names are
     not listed (ADR 0026 §2 point 6); they then start no listing. The safe write's final rename
     to the image's own name does count, and F3 finds the same list (§3.2, "the notification that
     follows finds the same list").
  7. **Duplicates and the set** (plain Rust, tested): a function that, given the watched folders
     and the new set, gives the folders to open and the folders to end, with
     `same_folder`-equal folders counted once. It takes the comparison as a parameter
     (`impl Fn(&Path, &Path) -> bool`) so its tests need no disk; the thread passes
     `tabs::same_folder`.
  8. **`Cargo.toml`:** `"Win32_System_IO"` is added to the `windows-sys` features
     (`ReadDirectoryChangesW` itself is in `Win32_Storage_FileSystem` but is compiled only with
     `Win32_System_IO`; `OVERLAPPED`, `CreateIoCompletionPort`, `GetQueuedCompletionStatus`,
     `PostQueuedCompletionStatus` and `CancelIoEx` are there). `pw-ingest` already enables it,
     so the workspace would build without the line, but `pw-loupe` must name what it uses. No
     other feature, version or dependency changes; `Cargo.lock` does not change. If another
     feature turns out to be needed, stop and ask.
- **Tests to add** (in `watcher.rs`'s test module; the folders are made under
  `std::env::temp_dir()` with a unique name and removed afterwards, as `library.rs`'s `TempDir`
  does; waits poll `try_recv` every 20 ms up to a stated limit, never a bare sleep for the
  result):
  - `QuietMoment`: one notification is due after 0.5 s and not before; notifications every
    0.3 s are due 2 s after the first; two folders are due separately; `next_due` is `None` when
    nothing waits and the shortest wait otherwise; a folder taken is not due again until it is
    noticed again.
  - The notification buffer: a record naming `a.ARW` counts; records naming only `a.xmp`,
    `a.pwtmp-0f.jpg` and `a.pwbak-0f.jpg` do not; the second of two records counting is found
    (`NextEntryOffset`); 0 bytes counts; a `NextEntryOffset` or `FileNameLength` pointing past
    the byte count counts and does not read past it; a byte count larger than the buffer is
    clamped.
  - The set: a folder given twice in two spellings (a fake comparison) is opened once; a folder
    left out is ended; an unchanged set opens and ends nothing.
  - Windows (`#[cfg(windows)]`, on a real temporary folder):
    - creating an empty `x.jpg` in a watched folder gives a `Changed` naming the folder as
      given within 5 s, and no second one in the 2 s after it;
    - creating 20 `.jpg` files one after another gives at most three `Changed` counted until
      3 s after the last file (the quiet moment; three, since a virus scanner or a loaded
      machine can stretch a gap past 0.5 s), the first within 5 s;
    - deleting a listed file gives `Changed`; renaming `x.jpg` to `y.jpg` gives `Changed`;
    - creating only `x.xmp` and `x.pwtmp-0f.jpg` gives no `Changed` within 1.5 s;
    - a file created in a subfolder of the watched folder gives no `Changed` within 1.5 s;
    - a folder that does not exist gives `Ended` for it within 5 s, and nothing else;
    - after a `Changed` has shown the watch is open, `set_folders(vec![])` and
      `wait_until_applied(2 s)` give true, and the watched folder's parent can then be renamed
      (`std::fs::rename`) and the watched folder deleted; after a `Drop` the same holds for a
      second watcher. The rename and the delete are retried for up to 2 s before the test fails
      (the search indexer or a virus scanner may hold a handle under `%TEMP%` for a moment);
    - the watched folder, emptied and then deleted while watched, gives `Ended` for it within
      5 s; a `Changed` before the `Ended` is accepted (Windows often first completes with
      0 bytes, then the restarted read fails). If Windows gives no `Ended` on this machine,
      stop and report what happened instead of changing the test.
  - `wait_until_applied` with no change since the last applied set returns true at once.
- **To report** (in the log row, not as a test assertion: logged by a test with `eprintln!`,
  or tried by hand; the test does not fail on either answer): whether renaming the watched folder's
  **parent** in File Explorer, and with `std::fs::rename`, fails while the folder is watched.
  This decides what the owner is told in T248's hand check (ADR 0026 F-Q4 covers only the
  folder itself being renamed).
- **Not in this task:** anything in `main.rs` or `tabs.rs` (T248); listing a folder, the tabs
  following, status texts and the fallback's listing on activation (F3); reading changed files
  again (F-Q7, F5 does it); following subfolders (§4); telling RawStride's own writes from outside
  changes beyond point 6 (F3, `plans/adr-0018-0025-steps.md`, "Taken up when a step is written").
- **Rules:** `docs/tasks/README.md` rules 1–12; no user-visible text (the `reason` is for the
  log); `AGENTS.md` §3 (no panic paths; `unsafe` only around the Windows calls).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-21 | Open | Added (ADR 0026 F2, first of T247 → T248) |
| 2026-11-21 | Open | Cold read: the folder opened with `std::fs::OpenOptions` + `OpenOptionsExt` instead of `CreateFileW` (it needs `Win32_Security`); a local `INFINITE`; an ended folder kept as an entry so it is not reopened, the first spelling kept, no `wake` without a thread; ownership of buffers, `OVERLAPPED` and the port (`Arc` wrapper, leak on give-up), "read pending" per watch and `ERROR_NOT_FOUND`, the wait in the main loop, keys never reused, `GetQueuedCompletionStatus`'s two FALSE cases; Drop's wait through `recv_timeout`; the parser takes `&[u8]` (`to_ne_bytes`); tests given counting windows, three `Changed` allowed for 20 files, retries for rename and delete, a `Changed` before `Ended` accepted; the parent rename only reported |
