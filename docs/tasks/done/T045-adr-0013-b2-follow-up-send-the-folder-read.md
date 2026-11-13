# T45. ADR 0013 B2 follow-up: send the folder read again when the metadata queue is full — Done

- **Start after T42 is done.** Small; can be done before or after T43.
- **Crate:** `pw-loupe`. **File:** `crates/pw-loupe/src/metadata.rs` only.
- **Why:** `MetadataCache::begin_load` sends the folder read with `worker.read_ratings`. Since T42,
  rating writes can fill the worker's queue (8 requests). If a tab is loaded at that moment, the read
  gets `SubmitError::QueueFull`, and every image of the folder shows "Metadatafel: metadatakön är
  full" until the tab is loaded again. ADR 0013 §4.2 rule 2 (back-pressure is retried, not an error)
  should apply to the read too.
- **What:**
  1. When `read_ratings` returns `QueueFull`, keep the files to read (for example
     `pending_read: Option<Vec<PathBuf>>`) and leave the images in `Loading`. `Stopped` still fails the
     load as today.
  2. In `poll`, after draining the events, send the pending read **before** `ratings.pump(worker)`.
     On `Ok(id)` set `load_request = Some(id)` and clear `pending_read`; on `QueueFull` keep it for the
     next frame; on `Stopped` fail the load.
  3. A new `begin_load` replaces any pending read (only the active tab's folder is read).
  4. To test this without ExifTool, let the read go through a small trait, as `RatingWriter` does
     for writes (for example `trait MetadataSubmit: RatingWriter { fn read_ratings(…) }` implemented
     for `MetadataWorker`), and make `begin_load` and `poll` generic over it, or test a private
     helper; your choice, as long as the test uses a fake that can return `QueueFull`.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; rules 6, 7, 10–12.
- **Tests to add:** `QueueFull` on the read keeps the images loading (no error notice); the next
  `poll` sends the read and a `RatingsRead` with that id makes them ready; `Stopped` gives the error
  as today; a second `begin_load` while a read is pending sends only the new folder; the read is sent
  before queued writes in the same `poll`.
- **Done when:** `.\tools\check.ps1` is all OK.
