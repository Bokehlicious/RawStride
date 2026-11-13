# T26. ADR 0011 K4a: pw-thumbcache — skip duplicate requests, export the image type — Done

- **Crate:** `pw-thumbcache`. **Files:** `crates/pw-thumbcache/src/worker.rs`,
  `crates/pw-thumbcache/src/lib.rs`.
- **Why:** the grid (T28) asks again for every visible cell that has no thumbnail yet each time the
  visible range changes, because the worker drops requests far outside the visible range without a
  result (T24). That is simple and safe only if asking twice does not decode twice.
- **What:**
  1. In `ThumbWorker::request`: if a request with the same generation, index and cell is already
     **queued** or **being made** by a worker thread, ignore the new one. Keep a small set of the
     in-flight `(generation, index, cell)` in `State`; a worker adds the entry when it takes a job
     and removes it when the job is done (also when the result is thrown away).
     `set_generation`, `cancel` and drop clear the queue as today; in-flight entries of an old
     generation simply never match again.
  2. In `lib.rs`: add `pub use pw_libjpeg::Rgb8;`, so `pw-loupe` can name the image type without its
     own dependency on `pw-libjpeg`.
- **Rules:** behaviour otherwise unchanged; the existing tests keep their names and assertions; no
  `unwrap`/`expect`/`panic!` in non-test code; no new dependencies.
- **Tests to add** (simulated source, as in the existing tests): the same request sent twice while it
  is queued gives one source call and one result; the same request sent again while it is being made
  (use the gated source) gives one source call and one result; the same index with another cell size
  is not a duplicate; after the result has arrived, asking again is answered from the cache (one
  source call).
- **Done when:** `.\tools\check.ps1` is all OK.
