# T24. ADR 0011 K2: thumbnail worker and in-memory cache — Done

- **Start after T23 is done.**
- **Crate:** `pw-thumbcache`. **Files:** `crates/pw-thumbcache/src/` (e.g. `worker.rs`).
- **What (ADR 0011 §3.1, §4.1, §4.2):**
  - A worker with a small pool of background threads (2 by default) that turns requests into
    thumbnails with T23's `thumbnail`. The UI never waits: it sends requests and polls results
    (`try_recv`), as `pw-metadata`'s `MetadataWorker` does.
  - Requests carry a folder generation, an image index, the path and the cell size.
  - **Priority:** the UI can tell the worker which index range is visible (e.g. `set_visible(range)`);
    requests in that range are done first, then the rest. Requests from an older generation, and
    requests far outside the visible range, are dropped (`set_generation` / `cancel`).
  - **Pause:** `pause()` / `resume()`, used while the loupe is shown (§4.2).
  - **In-memory cache** of finished thumbnails for the current generation, with a byte limit; the
    least recently used are evicted first. A cache hit answers without decoding.
  - Errors (`ThumbError`) are results too, so the UI can show the placeholder.
  - To make it testable without files, the work is done through a small trait (e.g.
    `trait ThumbSource { fn make(&mut self, path: &Path, cell: u32) -> Result<Rgb8, ThumbError>; }`);
    the real one uses `thumbnail` with its own `Decoder` per thread.
- **Rules:** no `unwrap`/`expect`/`panic!` in non-test code; a poisoned lock is recovered or turned into
  an error; dropping the worker does not block the UI thread; no new external crates.
- **Tests to add** (with a simulated `ThumbSource`, no real files): every request gets exactly one
  result; visible requests are done before others; results of an older generation never arrive after
  `set_generation`; pause stops new work and resume continues it; the cache returns a hit without
  calling the source again; eviction respects the byte limit; an error result is delivered.
- **Done when:** `.\tools\check.ps1` is all OK.
