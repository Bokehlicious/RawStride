# T27. ADR 0011 K4b: thumbnail bookkeeping for the grid (plain Rust) — Done

- **Start after T26 is done.**
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add `pw-thumbcache.workspace = true`
  to `[dependencies]`; approved by ADR 0011 §6), a new `crates/pw-loupe/src/thumbs.rs`, declared and
  re-exported in `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **What:** a struct `ThumbGrid` that remembers, for the folder shown in the grid, which thumbnails
  are missing, asked for, ready or failed. It holds no images and no textures (the program keeps the
  textures; T28).
  - `ThumbGrid::new(generation: u64, count: usize)`.
  - `to_request(&self, range: Range<usize>, cell: u32) -> Vec<usize>`: the indexes in `range` (clamped
    to `count`) that are missing, or ready/asked for with a **different** cell size. Already asked for
    with the same cell is included too (T26 makes repeating safe, and it covers requests the worker
    dropped); ready with the same cell and failed are not.
  - `mark_requested(index, cell)`.
  - `apply(&mut self, event: &pw_thumbcache::ThumbnailEvent) -> bool`: records ready (with the cell
    size that was asked for) or failed (with the error text); returns `false` and changes nothing for
    an event of another generation or an index outside the folder.
  - `state(index) -> CellState` with `CellState::{Missing, Requested, Ready, Failed(String)}`, for
    drawing (failed shows the error placeholder; `ThumbError::NoPreview` shows
    "ingen förhandsvisning").
  - `forget_outside(&mut self, keep: Range<usize>) -> Vec<usize>`: every ready or failed index outside
    `keep` goes back to missing; returns those indexes, so the program can release their textures.
  - A free function `request_cell(size_points: u32, pixels_per_point: f32) -> u32`: the cell size in
    physical pixels, rounded **up** to a multiple of 128 (so moving the size slider decodes again
    only when it crosses a step), at least 128; a non-finite or non-positive `pixels_per_point`
    counts as 1.0.
  - A free function `color_image(image: &pw_thumbcache::Rgb8) -> Option<egui::ColorImage>`:
    `None` if the data length is not width × height × 3.
- **Rules:** no egui drawing; no `unwrap`/`expect`/`panic!`/unchecked indexing in non-test code.
- **Tests to add:** a new grid asks for every index in range; an index asked for with the same cell
  is asked for again, a ready one is not; a ready one with another cell is asked for again; a failed
  one is not; an event of an old generation is ignored; an index outside the folder is ignored;
  `forget_outside` returns and resets exactly the indexes outside `keep`; `request_cell` for 200 pt
  at 1.0 → 256, at 1.5 → 384, 96 pt at 1.0 → 128, and a bad `pixels_per_point`; `color_image` for a
  2×1 image and for wrong data length.
- **Done when:** `.\tools\check.ps1` is all OK.
