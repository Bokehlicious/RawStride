# T231. ADR 0024 O1b: the view core and the thumbnails take an orientation per file (pw-viewcore, pw-thumbcache) — Done

- **Why:** ADR 0024 §2.4 (O-Q6): an XMP orientation is used before the embedded one by the view
  core and the thumbnails. Until it is read, the embedded orientation is used, and the image turns
  when the value arrives. Today both take the orientation only from the file (`pw_raw::inspect`,
  or LibRaw's flip for a RAW render). This task gives both a way to be told an orientation per
  file; T232 tells them. It also adds the turn of an Exif value (ADR 0024 §2.3, O-Q5), which O3
  uses for the keys.
- **Crates:** `pw-viewcore`, `pw-thumbcache` (and the smallest change in `pw-loupe` that keeps
  it building). **Files:** `crates/pw-viewcore/src/geometry.rs` (points 1–2),
  `crates/pw-viewcore/src/native.rs` (point 2), `crates/pw-viewcore/src/state.rs` (point 3),
  `crates/pw-viewcore/src/core.rs` (point 4), `crates/pw-thumbcache/src/extract.rs` (point 5),
  `crates/pw-thumbcache/src/worker.rs` (point 6), `crates/pw-loupe/src/main.rs` (point 7 only),
  `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **The turn of an Exif value** (`geometry.rs`): a new
     `pub fn turned(orientation: u16, clockwise: bool) -> u16`, with a doc comment naming ADR
     0024 §2.3: the orientation after the image is turned 90° clockwise (or anticlockwise). A
     mirrored value stays mirrored. Clockwise: 1→6, 6→3, 3→8, 8→1, and 2→7, 7→4, 4→5, 5→2;
     anticlockwise is the inverse (1→8, 8→3, 3→6, 6→1, and 2→5, 5→4, 4→7, 7→2). A value outside
     `1..=8` is taken as 1.
  2. **The flip of an orientation in one place** (`geometry.rs`, `native.rs`): today's
     `native::flip_of_orientation` moves to `geometry.rs` as
     `pub fn flip_of_orientation(orientation: u16) -> u8`, unchanged (mirrored orientations shown
     unmirrored, ADR 0024 §2.3), because `state.rs` needs it and `native` is behind the `native`
     feature. `native.rs` uses the moved function, and `native::flip_of_orientation` stays as a
     `pub use crate::geometry::flip_of_orientation;` so its callers and its test
     (`orientation_tags_map_to_libraw_flips`) are unchanged.
  3. **The view state** (`state.rs`): a new field `orientations: HashMap<PathBuf, u16>` with a
     doc comment (ADR 0024 §2.4: an orientation told by the shell, used before the file's own).
     It is **not** cleared by `open_folder` or `set_files` (it is per path, and the shell sends it
     once per value, T232). A new
     `pub fn set_orientation(&mut self, path: &Path, orientation: Option<u16>) -> bool`:
     - `Some(o)` with `o` in `1..=8` stores it; `None` (or a value outside `1..=8`) removes the
       path's entry, and the file's own orientation applies again.
     - When an image is opened (`finish_open_checked`, where `sizes.insert(key, size)` is done
       today; this runs again each time an evicted session is reopened), the size's `flip` is
       `flip_of_orientation(o)` if the key's path has an entry; otherwise it is the backend's, as
       today. The backend's own flip (for a RAW render LibRaw's flip, not an orientation) is kept
       beside it in a second map `own_flips: HashMap<Key, u8>`, inserted, cleared
       (`open_folder`) and moved (`set_files`) exactly where `sizes` is, so `None` can bring it
       back without opening the file again.
     - The keys are `(position, Source)`: the opened keys of the path are found through `files`,
       as `release` does. For each (both sources), the size's `flip` is set to the new value. If
       no flip changed, it returns `false`. Otherwise it returns `true`; the scale follows by
       itself (`scale_of` derives it from `sizes`, so fit stays fit), and the zoom and the relative
       centre are kept (the zoom rule of ADR 0024 §2.5 is O3's).
     - **No view is released, and no render in flight is dropped:** every view (Half, Zoom, Ring,
       Scaled) and every clipping and focus map is stored in sensor orientation, its region as a
       sensor rectangle (`sensor_box`), and the flip is applied only when drawing
       (`corner_uvs`). A view whose sensor rectangle no longer covers what is on screen is simply
       not chosen (`serves`), and the plan asks for the one that is needed, as after a pan. The
       view on screen stays until its successor is complete (V2).
  4. **The core** (`core.rs`): `pub fn set_orientation(&self, path: PathBuf, orientation:
     Option<u16>)`, beside `set_view`, with a doc comment: calls the state's `set_orientation`,
     and when it returns `true` marks the state changed, notifies the worker and wakes the UI, as
     `set_view` does. Not a user request (nothing is recorded).
  5. **The thumbnail** (`extract.rs`): `thumbnail` gets a new parameter
     `orientation: Option<u16>` after `cell`, documented (ADR 0024 §2.4: used instead of the
     file's own when given). The JPEG/RAW path orients by `orientation.unwrap_or(info.orientation)`
     and `tiff_thumbnail` by `orientation.unwrap_or(<the file's, as today>)`. A given value outside
     `1..=8` is treated as no value.
  6. **The thumbnail worker** (`worker.rs`): `ThumbnailRequest` gets
     `pub orientation: Option<u16>` (documented as in point 5). It travels with the job to
     `ThumbSource::make` (the trait gets the same parameter; `FileThumbSource` passes it to
     `thumbnail`; every test source takes it, such as `FakeSource`). The orientation is part of
     **every** key the worker compares requests by: `CacheKey`, the duplicate check in
     `request()` (today `(generation, index, cell)`) and `in_flight`, so a request with another
     orientation is a job of its own and a thumbnail made with one orientation is never given out
     for another. `ThumbnailEvent` gets `pub orientation: Option<u16>`, the value the thumbnail
     was made with (a cache hit included), so the shell can drop a late result made with an old
     value (T232).
  7. **The shell keeps building** (`pw-loupe/src/main.rs`): the two `ThumbnailRequest { … }`
     add `orientation: None`. Nothing else in `pw-loupe` (T232).
- **Tests to add:**
  - `geometry.rs`: `turned` for all eight values both ways (the table in point 1); four
    clockwise turns give the start back, for every value; clockwise then anticlockwise gives the
    start back; 0 and 9 are taken as 1.
  - `state.rs` (with the module's `State<u32>` helpers `SIZE`, `VIEW`, `k`, `state()`,
    `with_session()`; images opened through the test-only `finish_open`, since `known()` writes
    `sizes` directly and skips the open path): an opened image with flip 0 told orientation 6 has
    flip 6 and `set_orientation` returns `true`; telling it 6 again returns `false`; `None` brings
    back flip 0; an orientation told before the image is opened is used when it opens, and again
    when an evicted session is reopened; the entry survives `open_folder` and `set_files`, and
    after `set_files` moves the image the flip (and `own_flips`) moved with it; every resident
    view of the key is kept after a turn; at fit, the scale after the turn fits the turned size.
  - `extract.rs`: with the existing generated JPEG fixture, `Some(6)` gives swapped
    dimensions where `None` does not; `Some(9)` acts as `None`; the TIFF path with
    `rgb-400x200-orientation-6.tif` (or the module's existing TIFF helper) and `Some(1)` is not
    turned.
  - `worker.rs` (with `FakeSource`): two requests for the same index and cell with different
    orientations, the first still queued or in flight, are two jobs and give two events, each
    carrying its own orientation; a cached thumbnail is not given out for another orientation;
    a cache hit's event carries the orientation it was made with.
- **Not in this task:** where the shell gets the values and when it sends them (T232); the zoom
  rule at 100 % (ADR 0024 §2.5, O3); showing mirrored images mirrored (ADR 0024 §5); changing
  how the backends read the file's own orientation.
- **Rules:** `docs/tasks/README.md` rules 1–12. No new dependencies. No `.unwrap()` or
  `.expect()` outside tests. More than one crate: the reviewer has the diff reviewed
  independently before Done (README, "An extra code review").
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Blocked | Added (ADR 0024 O1, chain T230 → T231 → T232); waits for T230 |
| 2026-11-20 | Blocked | Cold read: the file list is complete. Changed: point 3 no longer releases views (all views and maps are in sensor orientation, the flip applied when drawing; a release would only cost renders and could drop the view on screen), `own_flips` follows `sizes` where it is inserted, cleared and moved, keys found through `files`, the scale follows by itself; point 6 puts the orientation in the duplicate check and `in_flight` too, and `ThumbnailEvent` carries it; the state tests use `finish_open` and the module's real helpers |
| 2026-11-20 | Blocked | Chain review (whole chain, notes in T230's log): the worker's duplicate key and the event's orientation added here (above); a render in flight across a flip change needs nothing (sensor orientation, the cold read's finding) |
| 2026-11-20 | Open | T230 done (committed); this task can start |
| 2026-11-20 | Ready for review | Points 1–7 done. `geometry.rs`: `turned` and `flip_of_orientation` (moved; `native.rs` re-exports it). `state.rs`: `orientations` (by path, kept across `open_folder`/`set_files`), `own_flips` (inserted, cleared and moved with `sizes`), `set_orientation` (keys found through `files` over the opened sizes, both sources; a size without an own flip, only possible in tests via `known()`, keeps its flip on `None`), the told flip applied in `finish_open_checked`. `core.rs`: `set_orientation`. `extract.rs`: `thumbnail(…, orientation, …)`, out-of-range treated as none. `worker.rs`: the orientation in `ThumbnailRequest`, `ThumbnailEvent`, `ThumbSource::make`, `CacheKey`, the duplicate check and `in_flight` (one `RequestKey`). `pw-loupe/src/main.rs`: `orientation: None` twice. **A file outside the list** (owner's yes in the session): `crates/pw-loupe/src/thumbs.rs`, one line `orientation: None` in the test helper `event()`, which builds a `ThumbnailEvent`. Tests: geometry 3, state 6, extract 3, worker 4 (16). check.ps1 all OK (1988 passed, 72 ignored), no clippy warnings |
| 2026-11-20 | Done | Reviewed by the reviewer and by an independent review agent (README, "An extra code review"): no bugs, no deviation from points 1–7. Checked: `own_flips` follows all three mutation sites of `sizes` (none removes a key); nothing derived from the flip is cached (scale, visible rect, `sensor_box`, `corner_uvs` and `serves` are computed from `sizes`), so no view needs releasing; `native.rs` turns no pixels, so a told flip cannot turn twice; the worker has no old 3-tuple key left; the 16 tests would fail on the old code and do not depend on timing. `thumbs.rs` outside the list on the owner's yes (above). Nits not changed: a size without `own_flips` (only via `known()` in tests) keeps a told flip on `None`; two gate helpers repeat code the older worker tests write inline. Homes: the worker caching `Some(<own value>)` and `None` twice (T232 point 5, accepted cost; out-of-range values cannot come from T230's read); the relative centre not clamped again after a turn at a zoom other than fit (plan O3 row, (c)). `.\tools\check.ps1` all OK (1988 passed, 72 ignored), no clippy warnings. No `CHANGELOG.md` line: nothing a user sees changes until T232; the owner's hand check of O1 follows with T232. Committed on the owner's yes; T232 is now Open |
