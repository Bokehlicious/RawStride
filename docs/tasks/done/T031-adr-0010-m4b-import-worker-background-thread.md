# T31. ADR 0010 M4b: import worker (background thread) — Done

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
