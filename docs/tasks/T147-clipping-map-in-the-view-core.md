# T147. ADR 0017 U7b, second part: the clipping map in the view core — Blocked

- **Blocked: waits for T146** (the RAW map in `pw-libraw`), which waits for the owner's answers to
  U7b-Q1–U7b-Q10 in `docs/tasks/T146-clipping-map-of-raw-data.md`. Chain: T146 → T147 → T148 →
  T149.
- **Why:** ADR 0017 §2.6, step U7b. T146 gives the map of an opened RAW file. The view core must
  ask for it at the right time, make the same map for JPEG and TIFF, keep a few, and give the
  preview the current image's map with the texture coordinates that place it on the shown image
  (orientation, zoom and position), so the shell only has to show its state (T148) and draw it
  (T149).
- **Crates:** `pw-viewcore` (and two test frames in `pw-loupe`). **Files:**
  `crates/pw-viewcore/src/backend.rs` (the type `ClippingMap`, its making from 8-bit pixels, the
  `Backend` method), `native.rs` (the method for RAW, JPEG and TIFF), `state.rs` (the job, the
  wish, the kept maps and the frame), `core.rs` (running the job), `lib.rs` (re-exports), their
  unit tests, `crates/pw-viewcore/tests/native_files.rs`; `crates/pw-loupe/src/loupe_text.rs`
  (only the new `Frame` field in its two test frames, so the workspace builds);
  `docs/test-inventory.md`.
- **What:**
  1. **`ClippingMap`** in `backend.rs`, the same layout as T146's `pw_libraw::ClippingMap`: `cell`,
     `width`, `height` (in cells) and one byte per cell, row by row, in sensor orientation (bit 0–2
     clipped in R, G, B; bit 3 deep shadows; U7b-Q2, U7b-Q3). A function makes it from 8-bit RGB
     pixels (3 bytes per pixel, the grid of `ImageSize`) with cells of 2×2 pixels (U7b-Q7): a
     channel's bit when at least one pixel of the cell has that channel at 255; the deep-shadow bit
     when every channel of every pixel of the cell is 0 (U7-Q5). Edge cells as T146:
     `ceil(w / 2)` × `ceil(h / 2)` cells, judged on the pixels they hold.
  2. **`Backend::clipping_map(&self, session) -> Option<ClippingMap>`**, `None` by default (as
     `statistics`; the other backends keep the default). `NativeBackend`: RAW from
     `Session::clipping_map` (converted to the view core's type; an error or an unsupported layout
     give `None`); JPEG and TIFF from the image's own decoded pixels **before** the conversion to
     the screen profile (ADR 0016, U7-Q5), as `jpeg_statistics` and `tiff_statistics` take theirs:
     the session's pixels when they were not converted, otherwise a new decode of the file. The
     session's own pixels are not changed. `Backend::statistics` is not changed.
  3. **Asked only when wanted** (U7b-Q8): `ViewCore` gets a setter for whether a map is wanted
     (the shell sets it when the highlight or shadow overlay is on). When it is, a new job asks the
     current image's map in its display mode, once, after the current image's first view is on the
     screen and after its statistics' job (it comes after `statistics_job` in `pick` and waits
     while that job of the same image runs; U7-Q6, U7c-Q5: a wanted 100 % view still comes first).
     The session is lent as the statistics' job lends it (booked under `STATISTICS_LEND`, or a
     sentinel of its own if sharing would mix the two up; say which in the report). Nothing is
     asked for other images (no prefetch) and nothing at all while it is not wanted. A failure or
     `None` is remembered for that image and mode, as the statistics' failures, and not asked
     again.
  4. **Kept** (U7b-Q8): the maps of the last 4 images shown (by the view core's key, image and
     mode); a fifth drops the one shown longest ago, which is asked again if it is shown again
     with the wish on. Another folder empties them (`open_folder`); a changed list moves them to
     their new keys as the statistics are moved (`set_files`).
  5. **In the frame:** `Frame` gets the current image's clipping state: not wanted; being made
     ("Reading"); none for this file; or ready, with the map (shared, `Arc`, so the shell can tell
     a new map from the one it has by `Arc::ptr_eq`) and the texture coordinates of the drawn
     image's four corners in the map (top left, top right, bottom right, bottom left, as
     `Draw::uv`). The coordinates come from
     `geometry::corner_uvs(size, Rect::new(0, 0, map.width * map.cell, map.height * map.cell), vis)`
     with the same `size` and `vis` as the `Draw` (the map's grid may reach past the image's right
     and bottom edges by less than a cell, which this rectangle accounts for); `frame()` is
     restructured as needed so `vis` is known outside the closure that makes the `Draw`.
     "Ready" is given whenever the drawn view is of the current image, also while a new zoom or
     position of it is being prepared (the overlay does not flicker while zooming); it is not given
     while an earlier image's view is kept on the screen (`Preparing` after a step, V2) or on an
     error.
- **Tests to add:** `backend.rs`: the map of 8-bit pixels (a channel at 255 sets only its bit; a
  cell of all-zero pixels sets the shadow bit, one pixel above 0 clears it; an odd width and
  height give the edge cells); `state.rs`: no map job while not wanted; with the wish on, one job
  for the current image after its first view and its statistics, none for neighbours; a failure
  not asked again; the fifth image's map drops the oldest; another folder empties them; a changed
  list keeps them under the new keys; the frame gives "ready" while the drawn view is of the
  current image (also while its zoom is being prepared) and not while an earlier image's view is
  kept, with the coordinates of `corner_uvs` for a flipped image (flip 6) and a map wider than the
  image by part of a cell; `native.rs`: a JPEG's map is made before the profile conversion, with
  a 100 % view rendered before or after (as the T142 test of the statistics); `tests/native_files.rs`
  (with `PW_TESTDATA`, ignored without it): a RAW file's map through `NativeBackend` has T146's
  size.
- **Not in this task:** showing or drawing the map, keys, settings and texts (T148, T149); any
  change to `Backend::statistics`, to the inspector's reader in `pw-loupe`, or to `pw-libraw`.
- **Rules:** rules 1–13 of `docs/tasks/README.md`; no new dependency. Run
  `D:\PhotoWorkflowData\pm-study\tools\latency.ps1` on a release build before and after (the
  overlays are off in its runs) and report V1; it must not be worse.
- **Done when:** `.\tools\check.ps1` is all OK, V1 is not worse, the ignored `native_files` tests
  pass with `PW_TESTDATA` set, and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-15 | Blocked | Added with T146, T148 and T149 (the chain of U7b); waits for T146 |
| 2026-11-15 | Blocked | Cold read (fresh agent): `loupe_text.rs`'s two test frames added to the files; the JPEG/TIFF decode rule restated (a new decode whenever the session's pixels are converted); the order after `statistics_job`, the lending and the moving of kept maps in `set_files` made explicit; "ready" also while the current image's zoom is prepared; `Arc` identity for the shell |
