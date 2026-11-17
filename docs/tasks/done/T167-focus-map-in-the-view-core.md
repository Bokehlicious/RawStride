# T167. ADR 0017 U9a, first part: the focus map in the view core — Done

- **Chain:** T167 → T168. The owner's answers U9-Q1–U9-Q4 are in ADR 0017 §6.1 (2026-11-17).
- **Why:** ADR 0017 §2.6, step U9a: edges of high local contrast are marked in a colour on the
  image, so sharp areas can be seen at fit without zooming. The map is computed from the shown
  image: the half-size render of the whole image (U9-Q2), not from the RAW data. (The view draws
  that render at fit levels up to 50 %; at larger fit levels it draws a sharper view, and the map
  still comes from the half-size render.) The view core makes the map when it is wanted, keeps a
  few, and gives the current image's map in the frame with the texture coordinates that place it
  on the drawn image. T168 gives the key and draws it. This follows the clipping map's way through
  the core (`ClippingMap`, `Job::Clipping`, `Task::Clipping`, `clipping_job`, `finish_clipping`,
  `clipping_map`, `keep_clipping_map`, `clipping_maps`, `clipping_none`, `ClippingState`,
  `Frame::clipping`, `ViewCore::set_clipping_wanted`), which stays as it is.
- **Crates:** `pw-viewcore` (and two test frames in `pw-loupe`). **Files:**
  `crates/pw-viewcore/src/backend.rs` (the type `FocusMap` and its making from 8-bit pixels),
  `crates/pw-viewcore/src/state.rs` (the job, the wish, the kept maps and the frame),
  `crates/pw-viewcore/src/core.rs` (running the job, the setter), `crates/pw-viewcore/src/lib.rs`
  (re-exports), their unit tests; `crates/pw-loupe/src/loupe_text.rs` (only the new `Frame`
  field in its two test frames, so the workspace builds); `docs/test-inventory.md`; and this
  task's own file (status and log, `docs/tasks/README.md` rule 11).
- **What:**
  1. **`FocusMap`** in `backend.rs`, the same fields as `ClippingMap` (`cell`, `width`, `height`
     in cells, `data` one byte per cell, row by row, in the pixel grid's orientation) and a
     `Debug` that does not print the data. `pub const EDGE: u8 = 1`. `pub const FOCUS_THRESHOLD:
     u16 = 160` (the owner tunes it after T168's hand check, U9-Q3).
     `FocusMap::of_rgb8(width, height, data) -> Option<FocusMap>` from a half-size render (3 bytes
     per pixel, rows top to bottom): **one cell per pixel of the render, `cell = 2`** (a pixel of
     the half-size render covers 2 × 2 pixels of the image's grid). For each pixel, the luminance
     `y = (54·R + 183·G + 19·B) >> 8` (integer, Rec. 709 weights summing to 256, so 0–255), and
     the Sobel gradient over its 3 × 3 neighbours, `|gx| + |gy|` (each of `gx`, `gy` in
     −1020…1020; compute in `i32`). The cell gets `EDGE` when that sum is at least
     `FOCUS_THRESHOLD`. The outermost row and column of pixels have no full neighbourhood and are
     never marked. `None` for an image narrower or lower than 3 pixels, or a buffer shorter than
     the image, as `ClippingMap::of_rgb8`. `at(x, y)` as `ClippingMap::at`.
  2. **Made on the worker, from the session:** `Job::Focus(Key)` (with its arm in `Job::key`) and
     `Task::Focus { generation, key, session }`. The worker arm in `core.rs` (beside
     `Task::Clipping`) calls `backend.render_half(&mut session)`, keeps the result only if
     `is_consistent()`, makes `FocusMap::of_rgb8` of it, and calls `State::finish_focus`, built
     as `finish_clipping` (remove the running job, count it, return the session with the same
     generation check, then keep the map or note the image in `focus_none`). A failed render or
     `None` gives no map. No new `Backend` method, so every backend (also the tests' fake one)
     gets the map from its own half-size render. `Stats` counts `focus_maps` and `focus_ms`, as
     `clipping_maps` and `clipping_ms`.
  3. **Asked only when wanted:** `ViewCore::set_focus_wanted(bool)` and `State::focus_wanted`
     (in `State::new`, `false`), as `set_clipping_wanted`. `focus_job` follows `clipping_job`
     exactly (the current image only, after its first view is on the screen, after the dwell, not
     during a zoom gesture, after a wanted 100 % view, `Job::Open` when its session is not open,
     not while the statistics' or the clipping job of the same image runs). In `pick` the focus
     job is inserted after the statistics' and the clipping job (count both insertions when
     choosing its place). The session is lent and booked under `STATISTICS_LEND`, as the
     clipping job's (an image's session is lent to one job at a time); update that constant's
     comment ("The two share it") to name all three jobs, and the ordering comment of
     `clipping_job` if it names the order. A failure or `None` is remembered for that image and
     mode (`focus_none`) and not asked again. Nothing is asked for other images, and nothing while
     it is not wanted.
  4. **Kept:** the maps of the last 4 images shown (`KEPT_FOCUS_MAPS = 4`), with `focus_map` and
     `keep_focus_map` as `clipping_map` and `keep_clipping_map`, and kept again as the newest when
     shown, as the clipping state does; a fifth drops the one shown longest ago. Another folder
     empties `focus_maps` and `focus_none`; a changed list moves them to their new keys; both
     exactly where the clipping maps are emptied and moved. The extra half-size render while the
     job runs and the kept maps (one byte per cell, about 15 MB each for a 60 MP image) are
     outside the core budget, as the clipping maps are (U7b-Q8); accepted.
  5. **In the frame:** `Frame::focus: FocusState` with the same four states as `ClippingState`
     (`NotWanted`, `Reading`, `Unavailable`, `Ready { map: Arc<FocusMap>, uv }`) and the same
     rules for when each is given, at the same places in `frame()` (the early default as well as
     the end; also "Ready" while a new zoom or position of the current image is prepared; not
     while an earlier image's view is kept on the screen, or on an error). `uv` comes from
     `geometry::corner_uvs(size, Rect::new(0, 0, size.w, size.h), vis)` with the same `size` and
     `vis` as the `Draw`: the map covers the whole image, as the half-size view does
     (`whole(size)`), whatever the render's rounding (it rounds up, so `map.width · 2` may be one
     more than `size.w`).
  6. Re-export `FocusMap` and `FocusState` from `lib.rs`. In `loupe_text.rs`'s two test frames add
     `focus: pw_viewcore::FocusState::NotWanted`; nothing more in `pw-loupe`.
- **Tests to add** (in the files' own test modules):
  - `backend.rs`: a flat image gives no edges; a vertical step marks exactly the cells on both
    sides of the step (a step of height `d` gives `4·d` there) and nothing else; the outermost rows
    and columns are never marked; a step with `4·d` just below `FOCUS_THRESHOLD` is not marked and
    one at or just above is (derive `d` from the constant, so the test holds when it is tuned);
    `None` for 2 × 2 pixels and for a short buffer; the map's size and `cell == 2`.
  - `state.rs`, as the clipping map's tests: no job while not wanted; the job after the first view
    and after the statistics' and the clipping job of the same image; not asked again after
    `None`; four kept and the fifth drops the oldest; a new folder empties them and `set_files`
    moves them; the frame's four states; `uv` spans the whole image.
  - `core.rs`: with the fake backend (`Mapped`), wanting the map gives `FocusState::Ready` for the
    current image, and turning the wish off gives `NotWanted`, as the clipping test there.
- **Not in this task:** the key, the menu item, the setting and the drawing (T168); a GPU or SIMD
  version; tuning the threshold (after T168's hand check).
- **Rules:** AGENTS.md §3; no new texts (no user-visible text in `pw-viewcore`).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added for ADR 0017 U9a; the owner's answers are U9-Q1–U9-Q4 in ADR 0017 §6.1 |
| 2026-11-17 | Open | Cold read: doable with the named files; a third job lent under `STATISTICS_LEND` works, the fake backend's `render_half` serves the core test, the Sobel ranges are right. Changed: `main.rs` taken out (it builds no `Frame`); `Job::Focus`, `finish_focus`, `focus_map`/`keep_focus_map`, `State::new` and both places in `frame()` named; the place in `pick` after both other jobs; `uv` over the whole image (the half render rounds up, not down); "at fit" made exact; memory outside the budget stated as accepted; the threshold test derives its step from the constant; this file named for its status and log |
| 2026-11-17 | Ready for review | Done as written: `FocusMap` with `of_rgb8` (Sobel on Rec. 709 luminance, `cell = 2`), `Job::Focus`/`Task::Focus`, `focus_job` after the statistics' and the clipping job (under `STATISTICS_LEND`), `finish_focus`, four kept maps, `FocusState` in the frame with `uv` over the whole image, `set_focus_wanted`; 16 new tests (7 `backend.rs`, 8 `state.rs`, 1 `core.rs`); `.\tools\check.ps1` all OK, 1716 passed, 65 ignored, 0 failed |
| 2026-11-17 | Done | Reviewed: the diff does points 1–6 as written; every place the clipping map goes through the core (job, lending under `STATISTICS_LEND`, `finish_*`, `open_folder`, `set_files`, the four kept maps, both places in `frame()`) has its focus twin; the job waits for the statistics' and the clipping job; `of_rgb8` has no `unwrap` or unchecked indexing; 16 tests, the threshold test derived from the constant; no bugs. The `pw-loupe` change is two test frames only, so no independent review (one crate in substance). Reviewer's `.\tools\check.ps1` on this tree: all OK, 1716 passed, 65 ignored, 0 failed; clippy without warnings. Left over: the time of the map (an extra half-size render in RAW mode while the session is lent) is counted in `focus_ms` but not measured; now a point in T168 before the hand check |
