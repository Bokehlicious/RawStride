# T332. ADR 0027 W5: the exposure and the shadows in the view core's render — Done

- **Why:** ADR 0027 §3.4 and W5-Q2, W5-Q3, W5-Q6 and A-Q11 (answered by the owner 2026-11-25 and
  2026-11-18, ADR 0027 §4.4 and §4). The view core's render path learns to carry an exposure to
  the backend and to lift the shadows of a view, and gets the two pure functions the state needs:
  the auto exposure's rule from the RAW statistics, and the shadows' curve. The state still asks
  for every view at 0 EV and without shadows; T333 makes it ask for others. The second task of
  W5's chain T331 → **T332** → T333 → T334 → T335. Waits for T331 (it uses
  `pw_libraw::Exposure`, `render_half_exposed`, `render_region_exposed` and
  `Session::camera_exposure`).
- **What the reviewer checked (2026-11-25):**
  - `crates/pw-viewcore/src/backend.rs`: the trait `Backend` (line 451) with
    `render_half_stoppable` and `render_region_stoppable` (default methods that ignore the stop,
    lines 471–488) and `profile_unusable` (asked once after a successful open); `Rgb` (line 13);
    `Statistics` (line 154: `origin`, `histogram: [[u64; 256]; 3]` on the sRGB-like curve for RAW
    data with the white level as the top bin, `clipped: [f64; 3]` shares) and
    `StatisticsOrigin`.
  - `crates/pw-viewcore/src/native.rs`: `render_half_stoppable` and `render_region_stoppable`
    for `NativeSession::Raw` call `pw_libraw`'s stoppable renders (lines 459–488); JPEG and TIFF
    fall back to the plain renders.
  - `crates/pw-viewcore/src/core.rs`: the worker's `Task::Render` (lines 80–153): the render
    (stoppable), the cancel check, the reduction per kind, the sharpening (`view.sharpen`),
    `to_display`, the check `is_consistent`. The focus job (`Task::Focus`, line 175) renders with
    `render_half`, not stoppable. The test backend `Stoppable` (about line 1240) overrides both
    stoppable renders.
  - `crates/pw-viewcore/src/state.rs`: `JobView` (line 379, `Hash`, `Eq`, `pub(crate)`, not
    re-exported) and its 17 literals, all in `state.rs`: lines 448 (`STATISTICS_LEND`), 1276
    (`pan_view`), 1313, 1329, 1363, 1370, 1378, 1385 (`req`), 2450 (`finish_render`'s half), and
    in the tests 3248, 3439, 3449, 4048, 6269, 6283, 6689 (`sharpened`, which uses `..job` and
    needs nothing) and 6946.
  - The other `Backend` implementors (`tests/engine.rs`, `pw-loupe` `histogram.rs` and
    `main.rs`, `pw-render/tests/sync.rs`, the test backends in `backend.rs` and `core.rs`) do not
    override the stoppable renders, so the trait's defaults cover them; only `native.rs` and
    `core.rs`'s `Stoppable` override them.
  - `crates/pw-viewcore/src/sharpen.rs`: the model for a module of pure functions on `Rgb`
    (`Sharpening` with integer fields so it can be hashed, range constants, `DEFAULT`).
  - `crates/pw-viewcore/tests/native_files.rs`: tests on the owner's files (`PW_TESTDATA`,
    ignored, `--features native`).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/exposure.rs` (new),
  `crates/pw-viewcore/src/shadows.rs` (new), `crates/pw-viewcore/src/backend.rs`,
  `crates/pw-viewcore/src/native.rs`, `crates/pw-viewcore/src/core.rs`,
  `crates/pw-viewcore/src/state.rs` (the two `JobView` fields and their literals only),
  `crates/pw-viewcore/src/lib.rs`, `crates/pw-viewcore/tests/native_files.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — more than about eight files, and the backend's FFI path (README, "Model").
- **What:**
  1. **The exposure value** (`exposure.rs`; `lib.rs` declares `pub mod exposure;` and `pub mod
     shadows;` beside `pub mod sharpen;`, and `pub use exposure::{AutoExposure, Ev}`; T334
     reaches `pw_viewcore::shadows::DEFAULT_AMOUNT` by its path): `Ev`, an exposure in thousandths of an EV (`Clone, Copy, Debug,
     PartialEq, Eq, Hash, Default`), so it can be part of a hashed job key: `pub struct Ev {
     milli: i32 }` with the field **private** (every `Ev` is in the range); tests compare
     `milli()` or a constructed `Ev`:
     - `Ev::ZERO`; `Ev::MIN_MILLI: i32 = -2000` and `Ev::MAX_MILLI: i32 = 3000` (W5-Q2: −2 to +3
       EV);
     - `Ev::from_thirds(thirds: i32) -> Ev`: `thirds × 1000 / 3` rounded to the nearest
       thousandth, halves away from zero (`f64::round`; so 1 gives 333, 2 gives 667, 3 gives
       1000, −1 gives −333), limited to the range;
     - `Ev::from_ev(ev: f64) -> Ev`: `(ev × 1000).round()` (`f64::round`), limited to the range;
       a value that is not finite gives `Ev::ZERO`;
     - `milli(self) -> i32`, `ev(self) -> f64`, `is_zero(self) -> bool`.
  2. **The auto exposure's rule** (A-Q11, W5-Q4; `exposure.rs`): `pub fn auto_exposure(stats:
     &Statistics) -> AutoExposure`, with `pub enum AutoExposure { Thirds(i32), Clipped, NotRaw,
     NoData }` (`Clone, Copy, Debug, PartialEq, Eq`):
     - `stats.origin` is not `StatisticsOrigin::RawData` → `NotRaw`;
     - the largest of `stats.clipped` is above 0.001 (more than 0.1 % clipped in the RAW data) →
       `Clipped`; a share that is NaN counts as clipped (`!(share <= 0.001)`), so broken
       statistics never brighten;
     - per channel with any pixels (the sum of its bins), the 99.9th percentile is the smallest
       bin `b` whose running sum from bin 0 (as `u64`) satisfies `sum × 1000 >= total × 999`
       (integers, no float compare); its value is
       the bin's upper edge, `(b + 1) / 256`, on the sRGB-like curve; `v` is the largest of the
       channels'. No channel with pixels → `NoData`;
     - `v` back to linear with the inverse sRGB curve (`v ≤ 0.04045`: `v / 12.92`; else
       `((v + 0.055) / 1.055)^2.4`), `L`; the correction that puts `L` ⅓ EV below clipping is
       `e = −⅓ − log2(L)`; the result is `Thirds(floor(3e + 1e-9))` limited to −6..=9 (rounded
       down to ⅓ EV and limited to −2..+3 EV, A-Q11, W5-Q2).
     A comment names A-Q11 and says the rule is tuned in the hand check, and that it never
     darkens by more than ⅓ EV (`L ≤ 1`, so the result is at least `Thirds(-1)`; the lower
     limit is kept for safety).
  3. **The shadows' curve** (W5-Q6, A-Q10; `shadows.rs`, public module): `AMOUNT_RANGE: RangeInclusive<u8>
     = 0..=100`, `DEFAULT_AMOUNT: u8 = 50`; `pub fn table(amount: u8) -> [u8; 256]` (an amount
     above 100 counts as 100): with `x = v / 255` and `s = amount / 100`, `y = x + 1.7 · s · x ·
     (1 − x)³`, the entry is `round(255 · y)` limited to 255. The curve keeps 0 and 255, never
     lowers a value, is monotonic (its slope is at least `1 − 0.425 s > 0`) and lifts most at
     `x = ¼`, the top of the darkest quarter (a comment says so). `pub fn lift(img: &Rgb, amount: u8) ->
     Option<Rgb>`: every byte of the image (each channel) through the table; `None` when
     `img.data.len()` is not `width × height × 3`.
  4. **The job carries them** (`state.rs`): `JobView` gets `exposure: Ev` and `shadows:
     Option<u8>` (`None` = off). Every literal gets `exposure: Ev::ZERO, shadows: None` (in this
     task the state asks for nothing else; `ViewOut` and `Resident` do not get the fields yet:
     T333's). Nothing else in `state.rs` changes.
  5. **The backend** (`backend.rs`): `render_half_stoppable` and `render_region_stoppable` get a
     parameter `exposure: Ev` before `stop`; the defaults ignore it (a backend that has no
     exposure, and JPEG and TIFF, render as before; the doc comments say so). A new default
     method `fn camera_exposure(&self, _session: &Self::Session) -> Option<f64> { None }`: the
     camera's own brightness offset in EV as the file gives it (not limited), asked once after a
     successful open (by T333).
  6. **The native backend** (`native.rs`): for `NativeSession::Raw`, the two stoppable renders
     call `render_half_exposed` and `render_region_exposed` with
     `pw_libraw::Exposure::from_ev(exposure.milli() as f32 / 1000.0)`; a `None` there (impossible with an
     `Ev` in its range) gives `DecodeError::new(format!("the exposure {} EV is outside the range",
     …))` and no render. JPEG and TIFF sessions ignore the exposure. `camera_exposure` gives
     `f64::from` of `Session::camera_exposure()`'s `ev` for RAW, `None` for JPEG and TIFF.
  7. **The worker** (`core.rs`): the body of `Task::Render` from the render to the consistency
     check moves, unchanged otherwise, into a free generic function `fn render_view<B:
     Backend>(backend: &B, session: &mut B::Session, view: JobView, stop: &Stop) -> Result<Rgb,
     DecodeError>` in `core.rs` (its tests `use crate::state::JobView`), so it can be tested on
     its own. In it: the stoppable renders get `view.exposure`; after the reduction (also for
     the too-small half-size image a `Reduced` job uses as it is, T309 R1) and **before** the
     sharpening and `to_display` (W5-Q3), a view whose `shadows` is `Some(a)` goes through
     `shadows::lift(&img, a)`, a `None` result giving `DecodeError::new("the decoder returned a
     malformed image")` (no new text, as the sharpening's). The focus job, the statistics and the
     clipping map are unchanged (they never get an exposure, A-Q6). The test backend
     `Stoppable` takes the new parameter.
- **Tests to add:**
  - `exposure.rs`: `from_thirds` for −7, −6, −1, 0, 1, 2, 3, 9, 10 (−6 and 9 the limits, −7 and
    10 limited); `from_ev` for −2.5, 0.4999, 3.0004, NaN, +∞; `ev()` and `is_zero`.
  - `exposure.rs`, `auto_exposure` on built `Statistics` (`origin: RawData` unless named): every
    pixel in bin 255 and `clipped` all 0 → `Thirds(-1)`; every pixel in bin 0 → `Thirds(9)`;
    `clipped` `[0.0, 0.002, 0.0]` → `Clipped`, and `[0.001, 0.001, 0.001]` → not `Clipped`;
    `origin: EmbeddedJpeg` → `NotRaw`; all bins 0 → `NoData`; a histogram whose 99.9th
    percentile lies in a chosen bin gives the value worked out by hand in the test's comment
    (one case where `3e` is just above a whole number, so the floor is visible; and one where
    999 of 1000 pixels of every channel are in a dark bin and 1 is in bin 255: the running sum
    reaches 999 in the dark bin, so the result follows the dark bin and the one bright pixel does
    not decide); the brightest channel decides (R dark, G bright → follows G).
  - `shadows.rs`: `table(0)` is the identity; for 50 and 100: entry 0 is 0, entry 255 is 255,
    monotonic, never below the input, the largest lift at an entry from 48 to 80; an amount
    of 101 gives `table(100)`; `lift` changes each channel by the table and gives `None` for
    data of the wrong length.
  - `core.rs`, `render_view` with a test backend that records the exposure it was given: a
    `Half` and a `Zoom` view with `exposure: Ev::from_thirds(3)` pass an `Ev` whose `milli()` is
    1000 to it; a view
    with `shadows: Some(100)` and a uniform dark image (value 40) comes back lifted to
    `table(100)[40]` before the sharpening (with `sharpen: Some(…)` too, the result equals
    sharpening the lifted image); `shadows: None` leaves the bytes as today.
  - `tests/native_files.rs` (ignored, on the owner's files, with its helpers `test_files` and
    `tiff_files`): `camera_exposure` is `None` for every ARW and RW2 and `Some` for at least one
    DNG (T331: a DNG without `BaselineExposure` gives `None`); for an ARW,
    `render_half_stoppable` at `Ev::from_thirds(3)` has a higher mean than at `Ev::ZERO`, and at
    `Ev::ZERO` gives the bytes of `render_half`; for an ARW opened in JPEG mode and for a TIFF
    file an exposure changes no byte. (A plain JPEG file goes the same JPEG path; no helper for
    one is added.)
- **Not in this task:** the state (which exposure and shadows each view gets, the statistics
  before the first view, the cache and the frame's states: T333); settings, keys and marks (T334,
  T335).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text). No new
  dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for T331. Cold read: the files suffice (no other `Backend` implementor overrides the stoppable renders); the maths of the rule and the curve checked (peak lift 23 at s = 0.5 and 46 at s = 1, at entries 55–73). Fixed: 17 `JobView` literals listed by line (not 26); `Ev`'s private field, constants' type and rounding stated; the percentile in integers; NaN in `clipped` counts as clipped; `render_view` a free generic function; the lift also on the unreduced half and before `to_display`; `f64::from` for the camera's value; the native tests use ARW in JPEG mode and TIFF, not a plain JPEG; the focus job's line |
| 2026-11-25 | Open | Set to Open by the reviewer on the owner's word after T331 was Done (0a6699b); T331's API as this task names it (`Exposure`, `render_half_exposed`, `render_region_exposed`, `Session::camera_exposure`); no change to this text needed |
| 2026-11-25 | Ready for review | Done as written (Opus). `exposure.rs` (`Ev`, `AutoExposure`, `auto_exposure`) and `shadows.rs` (`table`, `lift`) new; `JobView` has `exposure` and `shadows`, every one of the 16 literals `Ev::ZERO` / `None` (also `finish_render`'s half, line 2450, as the task says: T333 decides whether it carries the view's values) plus the `Ev` import; the trait's two stoppable renders take `exposure: Ev`, and `camera_exposure` is a new default; `native.rs` calls `render_half_exposed` / `render_region_exposed` and gives the RAW camera value; `core.rs`'s render body moved into `render_view`, with the lift after the reduction and before the sharpening. The NaN rule is written `share.is_nan() \|\| share > 0.001` (the same as `!(share <= 0.001)`, which clippy's `neg_cmp_op_on_partial_ord` refuses). Beyond the list: one test of the too-small half-size image of a `Reduced` view lifted, one that the other order (sharpen, then lift) differs, and the RAW region at 0 and +1 EV in the native test. Native tests run on `PW_TESTDATA` (3 new, all OK). `docs/test-inventory.md`: the header's total is this task's alone, 2759 passed and 97 ignored (+21 and +3); the `check.ps1` run counts 2765 / 98 because T336's work was staged in the tree during the run (its 7 tests, 1 of them ignored), which this task did not touch |
| 2026-11-25 | Done | Reviewed by the reviewer: no bug. `Ev`, `auto_exposure` (the hand-worked cases checked again: bin 187 gives 3e ≈ 2.013), the shadows' curve, `render_view` (the body moved unchanged; the lift after the reduction and before the sharpening), the 16 `JobView` literals (the 17th, `..job`, needs none) and `native.rs` as written. Fixed by the reviewer: in `native.rs` `raw_exposure` had been put between `raw_statistics`' doc comment and the function, so the comment moved back to `raw_statistics`. No extra code review (Rules name none; one crate). `.\tools\check.ps1` by the reviewer after the fix and without T336's work in the tree: all OK, 2759 passed, 97 ignored, no clippy warning (the first run failed once on `pw-metadata`'s timing test `a_template_and_edits_of_different_images_overlap` with 4 processes, a file this task does not touch; it passed on the second run). The 3 native tests run by the reviewer on `PW_TESTDATA` with `--ignored`: passed. No CHANGELOG line (nothing the user sees before T333). Leftovers: none |
