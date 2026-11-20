# T230. ADR 0024 O1a: the folder's batch read also reads `XMP-tiff:Orientation` (pw-metadata) — Done

- **Why:** ADR 0024 §2.4 (O-Q6, accepted by the owner 2026-11-17): an XMP `tiff:Orientation`
  (in a RAW's sidecar, or in the XMP of a DNG, JPEG or TIFF) is used before the file's embedded
  Exif orientation. It is read in the folder's batch call with the ratings, beside
  `XMP-xmp:Rating` and `XMP-xmp:Label`. This task is the read; T231 lets the view core and the
  thumbnails take an orientation per file, and T232 wires the two together in the shell. The
  owner checked on 2026-11-20 that DxO PhotoLab reads a sidecar's `tiff:Orientation` (ADR 0024
  §2.7).
- **Crates:** `pw-metadata` (and the smallest change in `pw-loupe` that keeps it building).
  **Files:** `crates/pw-metadata/src/lib.rs` (points 1–2), `crates/pw-metadata/src/worker.rs`
  (point 3), `crates/pw-metadata/tests/sidecar.rs` (tests), `crates/pw-loupe/src/metadata.rs`
  (point 4 only), `docs/test-inventory.md`, and this task's own file (status and log).
- **What:**
  1. **The values of a batch read** (`lib.rs`): a new public type beside `Rating`, with a doc
     comment naming ADR 0024 §2.4:

     ```rust
     #[derive(Clone, Debug, Default, PartialEq, Eq)]
     pub struct FolderValues {
         /// As `read_ratings` returns them: unrated images are absent.
         pub ratings: HashMap<PathBuf, Rating>,
         /// `XMP-tiff:Orientation` (1–8) of the images whose rating file holds one; others absent.
         pub orientations: HashMap<PathBuf, u16>,
     }
     ```

  2. **One ExifTool call reads both** (`lib.rs`): a new
     `pub fn read_folder_values(&mut self, images: &[PathBuf]) -> Result<FolderValues>` on
     `ExifTool`, built from today's `read_ratings`: the same files (`supports_rating`,
     `rating_file`, only files that exist), the same single `execute` call, with one more
     column, `-XMP-tiff:Orientation#` (the `#` gives the number, not ExifTool's text such as
     "Rotate 90 CW"), after `-XMP-xmp:Label`. Per line:
     - the rating is made exactly as today (stars `0` is no rating, `-` or empty is no label, an
       empty `Rating` is left out);
     - the orientation is kept when the column parses as a `u16` in `1..=8`; `-` (missing),
       empty, `0`, values above 8 and text that does not parse are left out. **`1` is kept**: a
       sidecar saying 1 overrides an embedded 6 (an image turned back).
     - An image is in `orientations` even when it has no rating, and in `ratings` even when it has
       no orientation.
     - The orientation column is taken with `cols.get(3)` (rule 4: no unchecked indexing; today's
       guard `cols.len() < 3` stays as it is, and a line without a fourth column simply has no
       orientation). The early return when no rating file exists becomes
       `Ok(FolderValues::default())`.

     `read_ratings` stays with its signature and doc comment and becomes
     `self.read_folder_values(images).map(|values| values.ratings)`, so its callers and tests
     (`tests/edit.rs`, `tests/template.rs`) are unchanged.
  3. **The worker** (`worker.rs`): `Request::Read` calls `read_folder_values`.
     `Event::RatingsRead` gets one new field after `result`:
     `orientations: HashMap<PathBuf, u16>`, with a doc comment (ADR 0024 §2.4; empty when the
     read failed). On success `result` is `Ok(values.ratings)` and `orientations` is
     `values.orientations`; on failure `result` is the `Err` as today and `orientations` is empty.
     Every other place in `worker.rs` that makes or matches `Event::RatingsRead` (`read_result`
     around line 837, and the `Err(error)` pattern around line 932; line 940 already has `..`)
     gets the field (`HashMap::new()`) or `..` as fits.
     `MetadataWorker::read_ratings` keeps its name and signature.
  4. **The shell keeps building** (`pw-loupe/src/metadata.rs`): only what the new field needs.
     The match arm `WorkerEvent::RatingsRead { id, result }` becomes
     `WorkerEvent::RatingsRead { id, result, .. }`, and each test that makes a `RatingsRead`
     event (around lines 1701, 1751, 2131, 2156, 2206 and 4242) adds
     `orientations: HashMap::new()`. The shell does
     nothing with the orientations yet (T232). No other change in `pw-loupe`.
- **Tests to add** (`tests/sidecar.rs`, `#[cfg(windows)]` like the file's other ExifTool tests,
  using its `exiftool()` and `temp_dir()`; each sidecar written by the test as a small XMP text
  file holding only the values it needs (the file has no such packet helper; a small
  `#[cfg(windows)]` helper that writes one is fine), and each image file only needs to exist; new
  top-level imports are `#[cfg(windows)]` like the existing ones):
  - `folder_values_read_the_sidecar_orientation`: three fake RAW names (`a.ARW`, `b.RW2`,
    `c.RAF`, any bytes) with sidecars holding `tiff:Orientation` 6, 8 and 3; `read_folder_values`
    gives exactly those three in `orientations` and nothing in `ratings`.
  - `an_orientation_without_a_rating_and_a_rating_without_an_orientation`: one sidecar with
    orientation 6 and no rating, one with `xmp:Rating` 3 and no orientation; each image is in
    the one map only.
  - `orientation_1_is_kept_and_bad_values_are_left_out`: sidecars with orientation 1, 0, 9 and
    no sidecar at all; only the 1 is in `orientations`.
  - `read_ratings_gives_what_folder_values_give`: a sidecar with rating 4, label Red and
    orientation 6; `read_ratings` returns the same `ratings` map as `read_folder_values`.
  - In `worker.rs`'s tests, a **new** test (existing tests unchanged, rule 5): a read on a worker
    started without ExifTool (as `start_without_exiftool` around line 1246 or
    `reports_exiftool_start_errors_asynchronously` around line 911 set it up) gives
    `Event::RatingsRead` with an `Err` result and empty `orientations`.
- **Not in this task:** using the orientation anywhere (T231, T232); writing it (ADR 0024 O2);
  reading the embedded Exif orientation through ExifTool (the view core and the thumbnails keep
  reading it with `pw-raw`); XMP inside a RAW file itself (the sidecar only, as for ratings;
  `backlog.md`); the inspector's and the metadata window's reads (`read.rs`) — ADR
  0024 §2.4 names Info, and Info shows no orientation today (T232 says so).
- **Rules:** `docs/tasks/README.md` rules 1–12. No new dependencies. No `.unwrap()` or
  `.expect()` outside tests.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added (ADR 0024 O1, chain T230 → T231 → T232) |
| 2026-11-20 | Open | Cold read: doable with the named files; no other file needs to change. Added: `cols.get(3)` and `FolderValues::default()` for the early return (point 2), the exact `RatingsRead` sites in `worker.rs` and `metadata.rs` (points 3–4), the worker test as a new test on a worker without ExifTool, a packet helper and `#[cfg(windows)]` imports in `sidecar.rs` |
| 2026-11-20 | Open | Chain review (one fresh agent, the whole chain T230–T232 with ADR 0024): order and data flow sound; a reload or tab switch during a read is safe; a failed read changes nothing. Fixed in the tasks: the thumbnail worker's duplicate key and `ThumbnailEvent` carry the orientation (T231 point 6), late thumbnails with an old value dropped and `prune` pushes `(path, None)` so the cores never keep a stale turn (T232 points 1 and 6), the compare's `second_core` told (T232 point 4). Its point that a render in flight must be dropped after a flip change was not taken: T231's cold read showed every view is in sensor orientation. Homes: O3's needs (a shown-versus-confirmed rule, the effective Exif value with the mirror) in the plan's O3 row; a RAW's own embedded XMP orientation in `backlog.md`; the double thumbnail decode at folder open named in T232 as an accepted cost |
| 2026-11-20 | Ready for review | Done as written: `FolderValues` and `ExifTool::read_folder_values` (one call, `-XMP-tiff:Orientation#` as the fourth column, taken with `cols.get(3)`, kept only in `1..=8`), `read_ratings` now maps `read_folder_values`; `Event::RatingsRead` carries `orientations` (empty on failure); `pw-loupe/src/metadata.rs` only `..` in the match arm and `orientations: HashMap::new()` in six test events. Tests: four in `tests/sidecar.rs` (with a `#[cfg(windows)]` helper writing a small XMP packet), one new worker test `a_failed_read_gives_no_orientations`; the existing `reports_exiftool_start_errors_asynchronously` and `read_result` patterns got `..` (point 3). `docs/test-inventory.md` updated (counts 1972 passed, 72 ignored). `.\tools\check.ps1` all OK, no clippy warnings |
| 2026-11-20 | Done | Reviewed: the diff holds only the named files and matches points 1–4; the five new tests ran and passed in `test-output.txt`, `.\tools\check.ps1` all OK, no clippy warnings; counts in `docs/test-inventory.md` checked (1967 + 5). Nothing left over. No `CHANGELOG.md` line: nothing a user sees changes until T232. The read has no visible effect of its own, so the owner's hand check of O1 follows with T232 (plan O1 row). Committed on the owner's yes; T231 is now Open |
