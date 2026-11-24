# T309. ADR 0027 W1: the Lanczos filter in the view, the half-size view reduced, the gestures — Open

- **Why:** ADR 0027 §3.2 and W1-Q2/W1-Q3/W1-Q5/W1-Q7 (answered by the owner 2026-11-24, ADR 0027
  §4.1): every view below 100 % is reduced on the worker with the Lanczos filter of T307
  (`pw-resample`), after T308 moved the conversion to the monitor profile behind the reduction.
  Today a fit view below 50 % is the half-size render drawn smaller by the GPU from a mipmapped
  texture; after this task it is the half-size render reduced on the worker to the exact scale
  and drawn at 1:1. During a zoom gesture the unreduced half-size view is used as today. The last
  task of the chain T307 → T308 → T311 → **T309**; the owner's hand check and `latency.ps1` follow it.
- **What the reviewer checked (2026-11-24):**
  - `pw-viewcore` `state.rs`: `ViewKind` is `Half`, `Zoom`, `Ring`, `Scaled`; `req` makes `Half`
    (region the whole image, density `HALF_DENSITY` = 0.5) for every scale ≤ 0.5, `Zoom` at 1 and
    above, `Scaled` between. `serving` picks the least dense resident view whose region covers
    the requirement and whose density is at least the requirement's; `frame` keeps the last drawn
    view (`Status::Preparing`) while nothing serves. `wanted` asks during a zoom gesture
    (`in_gesture`; `set_zoom` with `gesture`, `zoom_by`, and `set_view`, which ADR 0017 U6's second
    core calls) only for the current image's shown view when its scale is ≤ 0.5.
    `estimate_bytes` and `finish_render` have a branch per kind.
  - `pw-viewcore` `core.rs`, `Task::Render`: `Scaled` → `render_region` then `scale::downscale`
    (the box filter); after T308, then `to_display`. `native.rs`: a TIFF's `render_half` is its
    whole converted image reduced by `scale::downscale` to `scaled_size(…, 0.5)`.
  - `pw-render` `lib.rs`: `mipmapped(kind)` is `kind == ViewKind::Half`; its test lists the four
    kinds. Nothing else outside `pw-viewcore` matches on `ViewKind`.
  - Existing tests that expect the fit view to be `Half` at a scale below 0.5: `state.rs` (around
    the asserts `kind(&mut s, Zoom::Fit) == Some(ViewKind::Half)`, `kind(&mut s,
    Zoom::Scale(0.4))`, a plan whose first job is a `Half` render, and a view whose kind is `Half`),
    `tests/native_files.rs` `a7r_vertical_images_fit_and_zoom_with_correct_sizes` (`#[ignore]`,
    real files), and probably tests in `tests/engine.rs` (its view is 1000×800 and its RAW images
    4000×3000, so its fit scale is 0.25).
- **Crates:** `pw-viewcore`, `pw-render`. **Files:** `crates/pw-viewcore/Cargo.toml` (point 1),
  `crates/pw-viewcore/src/state.rs` (points 2–5), `crates/pw-viewcore/src/core.rs` (point 6),
  `crates/pw-viewcore/src/scale.rs` (point 7), `crates/pw-viewcore/src/native.rs` (point 8),
  `crates/pw-viewcore/tests/engine.rs` and `tests/native_files.rs` (the test changes below),
  `docs/architecture.md` (the `pw-viewcore` row gains: "views below 100 % reduced with
  `pw-resample` (Lanczos, linear light) and converted to the monitor profile after the reduction
  (ADR 0027 W1)"),
  `crates/pw-render/src/lib.rs` (point 9), `Cargo.lock`, `docs/test-inventory.md` (rule 12), and
  this task's own file (status and log).
- **What:**
  1. **The dependency** (approved by the owner 2026-11-18, A-Q7): in `pw-viewcore`'s
     `[dependencies]`, `pw-resample.workspace = true`, **not** optional (the generic core uses it,
     not only the `native` feature).
  2. **A new view kind `ViewKind::Reduced`** in `state.rs`, doc comment: "The whole image below
     50 %: the half-size render reduced on the worker to the view's density with the Lanczos
     filter (ADR 0027 §3.2, W1-Q3)". `name()` gives `"reduced"`.
  3. **`req`:** a scale more than `EPS` below `HALF_DENSITY` gives `JobView { kind: Reduced,
     region: whole image, density_q: quantise(scale) }`, for fit and free levels alike; a scale
     within `EPS` of 0.5 gives `Half` as today (drawn at 1:1, no reduction); `Zoom` and `Scaled`
     unchanged. **During a zoom gesture** (`in_gesture()`) a `Want::Shown` scale ≤ 0.5 gives
     `Half` as today, so a gesture never starts a render per frame (W1-Q3); only the job changes,
     not the requirement's cover or density, and `Want::Fit`/`Target`/`Ring` are not affected by
     the gesture. At rest `wanted` no longer asks for `Half`, so the half-size view stays resident
     only while the memory budget (V8) keeps it.
     - **`satisfied`** (what the plan counts as done): for a requirement whose job is `Reduced`,
       only a resident or in-flight `Reduced` view of the same image and generation with the
       job's `density_q` counts, so a `Half` (0.5) or a denser `Reduced` no longer stops the
       exact view from being made (after a gesture pause, after zooming out). **`serving`** (what
       `frame` draws) is unchanged: a denser view stands in until the exact one arrives.
     - **`remove_inflight`** matches on the density too (two `Reduced` views of one image differ
       only by it); **`accept`** treats an unwanted `Reduced` view as it treats an unwanted
       `Scaled` one.
     - **Free levels below 50 % (W1-Q8):** while the zoom is a free level below 0.5, the
       neighbours' primary prefetch is the view at the **current scale** (`Reduced`) instead of
       the fit view, so stepping at such a level stays as quick as today (it was served by the
       prefetched `Half`); at fit and at 50 % or more, `wanted` is unchanged.
     - **A changed view size** (window resized, a panel shown or hidden) changes the fit scale on
       every frame. Today `Half` served every size; with an exact `Reduced` per density each size
       would plan a new half-size render. So a change of the view size counts as a gesture step
       (as `set_view` does): `Half` while it goes on, the new `Reduced` once it pauses
       (`Config::gesture_pause`); the old view stands in meanwhile (`serving`).
  4. **The first gesture after a step** (W1-Q3, W1-Q9): while a gesture runs at a scale ≤ 0.5 and
     no resident view serves the requirement, `frame` draws the current image's densest resident
     `Reduced` view of the whole image, placed at the gesture's scale and position as any view is
     drawn (enlarged), instead of keeping the last frame still; it sets `last_draw` and `last_vis`
     (so the clipping and focus overlays follow it), but the status stays `Preparing` and
     `waiting_since` and `mark_shown` are not touched (the stand-in is not the finished view, and
     the latency records stay right). Only a view of the **current** image and generation
     qualifies (V2: never another image). When the half-size view arrives it replaces it in one
     swap. A gesture above 0.5, a step outside a gesture, and `set_view` above 0.5 change nothing.
  5. **`estimate_bytes` and `finish_render`** for `Reduced`: as `Scaled` (`area · d² · 4` bytes;
     the density from the image's width and height over the region's).
  6. **`core.rs`, `Task::Render`:** `Reduced` → `render_half`, then reduced to
     `scale::scaled_size(region.w, region.h, view.density())`; `Scaled` → `render_region`, then
     reduced the same way; both through `scale::reduce` (point 7); then `to_display` (T308) and the
     consistency check. A reduction that fails gives the error "scaling the view failed", as today
     (W1-Q7). If the half-size render is smaller than the target size in either dimension (it never should be: the
     target is below half the image), the half-size render is used unreduced rather than failing,
     with a comment.
  7. **`scale.rs`:** `downscale` (the box filter) is **removed** and replaced by
     `pub fn reduce(img: &Rgb, out_w: u32, out_h: u32) -> Option<Rgb>`, which calls
     `pw_resample::reduce_rgb8` and keeps `downscale`'s contract (`None` for an inconsistent input,
     a 0 size or an enlargement; the same size is a copy). The module comment is updated (Lanczos,
     ADR 0027 §3.2). The box filter's own tests `a_half_reduction_averages_2x2_blocks`,
     `a_flat_image_stays_flat_at_any_reduction` and `invalid_requests_are_refused` go with it and
     are replaced by tests of `reduce` (below); `sizes_round_and_stay_positive` and the crop test
     stay unchanged.
  8. **`native.rs`, TIFF:** the TIFF's `render_half` uses `scale::reduce` instead of
     `scale::downscale` (W1-Q5); its error text unchanged. A TIFF's `Reduced` view is thus
     reduced twice (whole → half → target); accepted, a TIFF is rare in the owner's work (a
     comment says so).
  9. **`pw-render` `lib.rs`:** `Reduced` is not mipmapped (it is drawn at 1:1); `mipmapped` stays
     `kind == ViewKind::Half`. **Magnification (W1-Q9):** `Reduced` and `Half` get linear
     magnification (they are magnified only as the gesture's brief stand-in, and nearest-neighbour
     would be blocky); `Zoom`, `Ring` and `Scaled` keep nearest (sharp pixels above 100 %). The
     doc comments of `mipmapped` and `texture_options` and the test of the kinds are updated to
     say so; the test gains `Reduced` and the magnification per kind (the existing asserts on
     mipmaps and `texture_bytes` unchanged).
  10. **Threads (W1-Q2):** T307's timing (440 ms on one thread) was above the reviewer's
      threshold of 100 ms (at about 7 images/s, V4, a neighbour's half-size render and its
      reduction must fit in about 140 ms), so T311 split `pw-resample`'s output rows over threads
      (owner 2026-11-24). This task uses `reduce_rgb8`/`reduce_rgb16` as they are and does not
      change `pw-resample`. If the owner's `latency.ps1` still fails V1 or V4 after this task, the
      owner decides (W1-Q2; the W1 row in `plans/` says so).
- **Changes allowed to existing tests:** only where W1-Q3 or W1-Q8 changes what a test expects:
  the kind of a view below 50 % (`Half` → `Reduced`), its size (the half-size render's size → the
  scaled size), its pixels (box or GPU reduction → Lanczos), and the jobs a plan lists at rest; and
  the setup of a test that relied on `Half` serving every scale ≤ 0.5 may be restructured so it
  tests the same thing. Known: in `state.rs` the helper `out_in` (a `Reduced` arm),
  `the_job_follows_the_scale`, `plan_unpacks_first_then_renders`, the statistics test that checks
  `view.kind` (around the assert `view.kind == ViewKind::Half`),
  `a_view_is_never_drawn_upscaled_below_100_percent` (restructured), and
  `a_finished_render_returns_its_session…` (passes only because 2000/4000 is 0.5: keep it, note
  it); T308's core test of `to_display` (it renders a flat colour, so it should keep passing);
  `tests/engine.rs` `a_free_zoom_level_is_rendered_scaled_and_sharp` should keep passing
  because the stand-in stays `Preparing` (point 4). Each changed test is named in the report with
  its old and new expectation. The `#[ignore]` test
  `a7r_vertical_images_fit_and_zoom_with_correct_sizes` expects `Reduced` and the size
  `scaled_size(region.w, region.h, job density)` (the quantised density, not the raw fit scale),
  and is run with `PW_TESTDATA` if the files are there. A test that fails for any other reason:
  stop and explain (rule 5).
- **Tests to add:**
  - `scale.rs`: `reduce` keeps `downscale`'s contract (the refusals, the copy), returns the asked
    size, and a flat image stays flat.
  - `state.rs`: at rest, a fit scale of 0.25 asks for `Reduced` with density 0.25; a free level of
    0.4 for `Reduced` 0.4; a scale of exactly 0.5 for `Half`; 0.6 for `Scaled`; during a gesture
    0.25 and 0.4 ask for `Half`; when the gesture pauses with `Half` resident, `Reduced` at the
    gesture's last scale is still planned (`satisfied`), while `Half` is drawn until it arrives.
  - `state.rs`: during a gesture to 0.4 with only `Reduced` 0.25 resident, `frame` draws it
    enlarged to the gesture's scale, status `Preparing`, `last_vis` at the gesture's position;
    with no whole-image view of the current image resident, the last frame is kept as today; a
    resident view of another image is never drawn; a gesture above 0.5 keeps today's behaviour.
  - `state.rs`: zooming out from `Reduced` 0.4 to 0.3 draws the 0.4 view and plans the 0.3 one;
    the least dense serving view is still chosen when both `Half` and `Reduced` serve; two
    `Reduced` views of one image in flight at different densities are each accepted and removed
    as their own (`remove_inflight`).
  - `state.rs` (W1-Q8): at a free level of 0.4 the neighbours' prefetch asks for `Reduced` 0.4;
    at fit it asks for the fit view as today.
  - `state.rs`: a run of view-size changes at fit asks for `Half` while it goes on and for one
    `Reduced` at the last size when it pauses; never more than one `Reduced` job of the image is
    planned during the run.
  - `core.rs`: with a simulated backend, a `Reduced` job calls `render_half` once, gives an image
    of the scaled size and is converted (`to_display`) after the reduction; a failing reduction
    gives the view's error.
  - `pw-render`: `Reduced` is not mipmapped; `Reduced` and `Half` magnify linearly, the others
    nearest.
- **Not in this task:** fine details (W2, the full render at fit); any change to the thumbnails or
  `pw-tiff`'s box filter (W1-Q6); the export; a setting or key for the filter (W1: none); the
  `latency.ps1` measurement (the owner's, after this task).
- **Rules:** `docs/tasks/README.md` rules 1–12; the only `Cargo.toml` change is point 1 (no
  `Cargo.toml` change in `pw-resample`). `docs/test-inventory.md`:
  besides the added tests, the `pw-viewcore` and `pw-render` rows and the totals are updated for
  the three removed box-filter tests and the changed kinds test.
- **Extra code review:** yes (more than one crate, about ten files; README "An extra code
  review").
- **At Done (the reviewer):** `CHANGELOG.md` gets "Changed: views below 50 % are reduced with a
  sharper filter" (T307 and T308 get none); ADR 0016 (the line that `render_half` and
  `render_region` convert their output) and ADR 0009 (the fit view scaled down on the GPU) get a
  note "changed by ADR 0027 W1".
- **Done when:** `.\tools\check.ps1` is all OK; the `#[ignore]` tests of `tests/native_files.rs`
  pass with `PW_TESTDATA` set, or the log says why not; and `git diff --stat` shows only the files
  above.
- **The owner's hand check** (after review and commit): at fit on a 61 Mpix ARW and a 24 Mpix
  file in RAW and JPEG mode, the image is a little crisper than before, with no ringing halos on
  hard edges and no jagged lines; a zoom gesture with the wheel from fit to 50 % and back is as
  smooth as before; the first gesture after a step shows the image enlarged briefly, then sharp;
  a TIFF at fit looks right; dragging the window's edge at fit gives no stutter and ends sharp;
  colours unchanged at fit and 100 % for a RAW and a JPEG in Adobe RGB on the owner's monitor
  profile (T308); `latency.ps1` at fit and at 100 % on a release build, against V1 and V4
  (W1-Q2).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W1, W1-Q2/W1-Q3/W1-Q5/W1-Q7); waits for T308 |
| 2026-11-24 | Blocked | Cold read: a resident `Half` would have satisfied every `Reduced` requirement, so `satisfied` now counts only the exact `Reduced` view (`serving` unchanged); the gesture switch only for `Want::Shown`; `remove_inflight` by density, `accept` for an unwanted `Reduced`; the stand-in stays `Preparing`, sets `last_draw`/`last_vis`, only at ≤ 0.5; free levels below 50 % keep quick stepping (W1-Q8); linear magnification for `Reduced`/`Half` (W1-Q9); the tests that change named; the a7r test's size by the quantised density |
| 2026-11-24 | Blocked | Chain review (see T307's log): a view-size change counts as a gesture step; `docs/architecture.md`, the test-inventory removals, point 10's threshold, the extra code review, the Done steps and hand-check points added |
| 2026-11-24 | Blocked | T308 Done; T307's timing (440 ms) is above point 10's 100 ms, so the threads come first as T311 (owner 2026-11-24); waits for T311 |
| 2026-11-24 | Open | T311 Done (61–63 ms on 8 threads) |
