# T42. ADR 0013 B2: rating store and write queue (plain Rust) — Done

- **Review 2026-11-09:** accepted. One gap in this task's text (reads were not covered by the
  back-pressure rule) is fixed in T45.

- **Start after T41 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/ratings.rs`, `crates/pw-loupe/src/metadata.rs`,
  and `crates/pw-loupe/src/main.rs` **only** for the calls that replace today's rating calls (no new
  drawing, no keys; those are T43). Not `pw-metadata`.
- **Why:** ADR 0013 §4.2: optimistic values, writes in key order, the last key wins, rollback on
  failure, and a folder read that never overwrites a newer value.
- **What:**
  1. A small trait so the store can be tested without ExifTool:
     `pub trait RatingWriter { fn write_rating(&mut self, path: PathBuf, rating: Rating) -> Result<RequestId, SubmitError>; }`,
     implemented for `pw_metadata::MetadataWorker` (it calls the existing method).
  2. `pub struct RatingStore`, keyed by absolute path, holding per path `confirmed: Option<Rating>`,
     `shown: Option<Rating>` and its writes (queued, not sent; and sent, with their `RequestId`).
     Methods (names may differ if the tests show the same behaviour):
     - `set(&mut self, path: &Path, key: RatingKey) -> Result<(), RatingRefused>`: refuses a file
       without `pw_metadata::supports_rating` (`RatingRefused::Unsupported`, nothing queued);
       otherwise `shown = apply(shown, key)` and queues a write. If the path already has a queued write
       that was **not sent yet**, that write's value is replaced instead of queuing another.
     - `pump(&mut self, writer: &mut impl RatingWriter)`: sends queued writes in queue order until the
       writer returns `QueueFull` (stop; the rest stays queued for the next frame). `Stopped` fails the
       write as in point 3.
     - `written(&mut self, id, path, result) -> Option<String>`: success → `confirmed` is the written
       value; failure → `confirmed` unchanged, and `shown` goes back to `confirmed` **unless** a newer
       write for the path is queued or sent. Returns the message for the status line on failure:
       "Kunde inte spara betyget för {file name}: {error}".
     - `read(&mut self, read_id: RequestId, ratings: &HashMap<PathBuf, Rating>, paths: &[PathBuf])`:
       for each path, `confirmed` = the read value (absent = `None`); `shown` is set too **only** if the
       path has no queued write and no write sent with an id greater than `read_id`.
     - `shown(&self, path: &Path) -> Option<&Rating>`, `pending(&self) -> usize` (queued + sent),
       `pending_paths(&self) -> Vec<PathBuf>`, and `prune(&mut self, keep: impl Fn(&Path) -> bool)`
       that drops entries for which `keep` is false **and** that have no pending write.
  3. `MetadataCache` keeps the load states (`Loading`, `Ready`, `Unsupported`, `Error`) and the read
     request, and gets a `RatingStore`. `begin_load` no longer drops ratings; a `RatingsRead` for the
     current load calls `store.read(…)`; `RatingWritten` calls `store.written(…)` (today's handling
     in `apply` is replaced). `poll` also calls `store.pump(worker)` (it needs `&mut MetadataWorker`).
     Failure messages are returned from `poll` (for example as `Vec<String>`) so `main.rs` can put the
     last one in `self.message`.
  4. `rating_summary` uses `status_text` (T41). The three English notices become Swedish:
     "Läser metadata…", `UNSUPPORTED_TEXT` and "Metadatafel: {error}". `submit_error` texts: "metadatakön är full" and "metadataarbetaren har
     stannat".
  5. `main.rs`: only adapt the existing calls (`poll` with `&mut`, the message). When a tab is closed,
     call `prune` with the files of the tabs still open.
- **Rules:** no I/O and no threads in `ratings.rs`; no `unwrap`/`expect`/`panic!`; rules 6 and 7.
- **Tests to add** (with a fake writer that records requests, can return `QueueFull` or `Stopped`, and
  lets the test deliver results): `set` shows the value at once; Unsupported refused with nothing queued;
  1-2-3-4-5 before a `pump` gives one write of 5; order kept across two paths; `QueueFull` leaves the
  rest for the next `pump`; `Stopped` rolls back; a failure with a newer write pending keeps the newer
  value, and a failure without one rolls back to `confirmed`; a read with an id lower than a sent write
  does not change `shown` but a read with a higher id does; a read never changes a path with a queued
  write; entries survive `begin_load` of another folder; `prune` keeps paths with pending writes;
  `MetadataCache` with the Swedish notices. Existing `metadata.rs` tests keep passing (update only
  expected English texts to the Swedish ones, and say so in the report).
- **Done when:** `.\tools\check.ps1` is all OK.
