# T94. ADR 0007 I10a: the ExifTool version is checked when the metadata worker starts — Done

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
