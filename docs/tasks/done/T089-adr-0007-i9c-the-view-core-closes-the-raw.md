# T89. ADR 0007 I9c: the view core closes the RAW sessions of a file before it is written — Done

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
