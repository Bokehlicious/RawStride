# T65. ADR 0015 D3: thumbnails of TIFF files — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's commit.

- **Can start now.** Independent of T64 (they share only `docs/test-inventory.md` and this file).
- **Crates:** `pw-tiff`, `pw-thumbcache`, `photoworkflow` (`src-tauri`).
- **Files:** `crates/pw-tiff/src/` (a new module for the moved helpers, and `lib.rs`),
  `src-tauri/src/imaging.rs`, `crates/pw-thumbcache/Cargo.toml`, `crates/pw-thumbcache/src/extract.rs`,
  `Cargo.lock` (by cargo), `docs/test-inventory.md`.
- **Why:** ADR 0015 §4.1 and Q7: a TIFF thumbnail is decoded from the image data and reduced to the
  cell with a box filter. The box filter already exists in the Tauri app.
- **What:**
  1. **Move `fit` and `downscale`** with their tests (`fit_keeps_aspect_and_never_upscales`,
     `downscale_averages_blocks`, assertions unchanged) from `src-tauri/src/imaging.rs` to `pw-tiff`
     (for example `crates/pw-tiff/src/resize.rs`), exported from `pw-tiff`. Behaviour unchanged.
     In `downscale`, replace slice indexing (`src.data[..]`, `out[..]`, `col[x]`, `acc[..]`,
     `cnt[t]`) with forms that cannot panic in non-test code (`get`/`get_mut`, iterators,
     `chunks_exact`), keeping the result bit for bit; the existing test and one more on a 7×5 image
     reduced to 3×2 (compare with the result of today's code, computed in the test) pin it.
     `imaging.rs` becomes `pub use pw_tiff::{Rgb8, downscale, fit};` plus its module comment, so
     `rawview.rs`, `tiff_view.rs` and `main.rs` do not change.
  2. **`pw-thumbcache`:** `pw-tiff.workspace = true` under `[dependencies]` and `tiff = "=0.9.1"`
     under `[dev-dependencies]`. In `thumbnail`, before `pw_raw::inspect`: for a TIFF
     (`pw_raw::is_tiff_file`), a new private `tiff_thumbnail(path, cell)`:
     - `pw_tiff::decode_rgb8(path, Some(TIFF_LIMIT))`, with `const TIFF_LIMIT: usize = 4 << 30`
       (the default core budget; the thumbnail worker does not get the user's budget, ADR 0015 §9.1);
     - reduce with `pw_tiff::fit(w, h, cell)` and `pw_tiff::downscale` (never upscales);
     - convert to `pw_libjpeg::Rgb8` (same three fields) and apply `orient` with the orientation of
       `pw_raw::inspect`, or 1 if inspect fails (BigTIFF);
     - errors become `ThumbError::Unreadable(e.to_string())`.
     The embedded previews of a TIFF are not used (ADR 0015 Q7).
- **Rules:** RAW and JPEG thumbnails unchanged; the Tauri app's output unchanged; rules 3–7 and
  10–12. `git diff Cargo.lock`: only the new dependency lines of `pw-thumbcache`; no new package.
- **Tests to add** (`extract.rs`; write the files with `tiff::encoder::TiffEncoder` into a test
  folder of its own):
  - a 400×200 8-bit RGB TIFF with `cell = 100` gives 100×50; with `cell = 1000` it stays 400×200
    (no upscale); a uniform colour stays that colour;
  - a TIFF with Orientation 6 gives a thumbnail turned to 50×100;
  - a palette TIFF gives `Unreadable` with the `pw-tiff` text;
  - a TIFF with an embedded JPEG preview is still decoded from its image data (optional if the
    encoder cannot write one; then say so in the report);
  - ignored like the existing real-file test (needs `PW_TESTDATA`): every `.tif` in `PW_TESTDATA`
    gives a thumbnail whose long edge is 256 for `cell = 256`.
- **Done when:** `.\tools\check.ps1` is all OK; `photoworkflow` has two tests fewer and `pw-tiff`
  three more; the owner's `check.ps1 -Ignored` run passes the new real-file test; `git diff --stat`
  shows only the files above.
