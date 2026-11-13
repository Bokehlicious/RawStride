# T59. ADR 0014 amendment 1, §2.2: a rating removes an image that stops matching at once — Done

- **Review 2026-11-10: code accepted.** Waiting for the owner's hand check ("Done when", plus a
  folder given as a relative path on the command line: ratings shown and filtered).

- **Start after T56 is done.** Before T57.
- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/src/browse.rs`, `crates/pw-loupe/src/main.rs`
  (the rating key and the shown-rating lookup of point 4).
- **Why:** ADR 0014 §2.2, §4.2 and S2 as changed by amendment 1 (§13). Changes the behaviour T55
  built: a rated image no longer stays until the user moves on.
- **What:**
  1. `Tab::rated(&mut self, ratings_ready: bool, rating_of: &dyn Fn(&Path) -> Option<Rating>) -> bool`:
     `refilter` without keeping any image. Images that no longer match leave `visible`; if the
     current image left, the nearest following visible image becomes current (the last visible one
     if none follows; unchanged when nothing is visible, §2.3); marked images that left are
     unmarked, the others stay marked. Returns whether `visible` changed.
  2. `grid_move`, `loupe_move` and `click` stay as they are (they still recompute before and after
     a move, which matters when a rating changes without a key, e.g. a rolled-back write). Update
     the doc comments in `browse.rs` (module, `refilter_before_move`, `grid_move`, `loupe_move`)
     and of `Tab.visible` in `tabs.rs` that describe the old §2.2 rule. No other change in `tabs.rs`.
  3. `main.rs`: after `set_ratings` returns `Ok`, call `rated` on the active tab with the same
     readiness as the other refilters. Grid: if `visible` changed, scroll to the current image's
     position. Loupe (until T57): if `visible` changed, give the core the visible list as
     `reopen_loupe` does; if nothing is visible, return to the grid (§2.3). A refused key
     (`Unsupported`, a path error) does not call `rated`. A rolled-back write does not refilter.
  4. **Resolved paths for the shown rating** (found in the T56 review): the filter's `rating_of`
     (`refilter_active`, `grid_move`, `click`, and `rated`) and the grid's `shown_rating` call pass
     `tab.files` as they are, but `MetadataCache` keys by absolute path. A folder given as a relative
     path on the command line then filters and draws every image as unrated. Look the rating up by
     the path resolved as `resolve_paths` does (for example a small helper on `Loupe`). Add a test
     for the helper if it is plain.
- **Rules:** as T55; rules 6, 7, 10–12.
- **Tests to change** (approved by amendment 1; nothing else in these tests changes):
  - `the_filter_select_cases_of_app_js`: the block from the comment "Under "unrated", rating the
    current image (2 stars) keeps it visible" up to the comment "Multi-selection with every image
    shown" becomes: rate image 4 with 2 stars, call `rated` → returns true, current 5, visible
    [0, 5, 6, 7, 8, 9, 10, 11], the file at current is `IMG_005.ARW`.
  - `a_loupe_step_drops_the_rated_image_it_leaves` and `batch_rated_under_unrated`: comments only
    (the ratings change without `rated`, as after a rolled-back write); assertions unchanged.
- **Tests to add:**
  - Unrated, current 5, marked 5–7, all three get stars, `rated` → visible [0, 4, 8, 9, 10, 11],
    current 8, nothing marked.
  - "Alla utom avvisade" (visible every index but 3), current 2, marked 1, 2 and 4; image 2 gets
    rejected, `rated` → 2 hidden, current 4, marked [1, 4].
  - The last visible image rated out → the previous visible image is current.
  - Red (visible [2, 4]), both marked, both lose Red, `rated` → nothing visible, returns true,
    current unchanged, nothing marked.
  - `rated` when nothing stopped matching → returns false; current and marking unchanged.
- **Done when:** `.\tools\check.ps1` is all OK, and the owner checks by hand: under "Ej betygsatt"
  a rated image leaves at once in grid and loupe, and a rated marking leaves at once; under "Grön"
  another label removes the image at once.
