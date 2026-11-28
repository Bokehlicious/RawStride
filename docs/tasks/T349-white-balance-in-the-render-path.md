# T349. ADR 0027 W6: the white balance in the view core's render path — Open

- **Why:** ADR 0027 §3.5 and W6-Q1–W6-Q4 (answered by the owner 2026-11-28, ADR 0027 §4.6). T348
  made `pw-libraw` render with a white balance and give the means under a RAW area, the file's
  presets and the daylight multipliers. This task makes the view core's render path carry a
  balance to the backend, gives the backend the three new questions (the file's balance
  information, the auto balance, the balance from a grey area), and adds the pure pieces the
  state needs: the balance value that can sit in a hashed job key, and the grey area of a click.
  The state still asks for every view as shot; T350 makes it ask for others. The second task of
  W6's chain T348 → **T349** → T350 → T351 → T352 → T353. Waits for T348 (it uses `pw_libraw::Adjust`,
  `WhiteBalance`, `Multipliers`, `render_half_adjusted`, `render_region_adjusted`,
  `Session::channel_means`, `ChannelMeans::multipliers`, `white_balance_presets`, `daylight`,
  `camera_multipliers`, `can_measure`).
- **What the reviewer checked (2026-11-28):** the exposure (T332) went through the same places:
  - `crates/pw-viewcore/src/backend.rs`: the trait `Backend` with `render_half_stoppable` and
    `render_region_stoppable` (default methods that ignore the stop and the exposure), then
    `camera_exposure`, `session_usable`, `to_display`, `profile_unusable`, `statistics` and
    `clipping_map` (each a default that gives nothing); `Stop`; `DecodeError`.
  - `crates/pw-viewcore/src/native.rs`: the two stoppable renders for `NativeSession::Raw` call
    `render_half_exposed` and `render_region_exposed` through `raw_exposure(exposure)` (about line
    292); JPEG and TIFF ignore the exposure; `camera_exposure` (about line 509); `statistics` and
    `clipping_map` for the RAW session.
  - `crates/pw-viewcore/src/core.rs`: `render_view` (about line 148) passes `view.exposure` to
    the two stoppable renders; a test backend `Stoppable` overrides both; `Exposed` (about lines
    1420–1506, its test views `half_view` and `zoom_view` about 1529–1551) is the backend of the
    render tests; the tests in `core.rs` build `JobView` literals too (about lines 1530–1710). The focus job, the statistics and the clipping
    map call `render_half` and the like directly, so they never get a balance (A-Q6).
  - `crates/pw-viewcore/src/state.rs`: `JobView` (`pub(crate)`, `Hash`, `Eq`) with `sharpen`,
    `exposure`, `shadows`, `channel`; every literal of it (the plan's, `STATISTICS_LEND`, the
    tests'; `..job` / `..view` literals need nothing); `Look` (about line 481) is **not** touched
    in this task (T350 adds the balance to it).
  - `crates/pw-viewcore/src/geometry.rs`: `ImageSize { w, h, flip }` in sensor orientation,
    `oriented()`, `to_sensor(x, y)` (edge coordinates of the upright image to the sensor's),
    `sensor_box(size, r: RectF)` (the sensor rectangle that covers an upright rectangle, outwards,
    even origin, inside the image, at least 1×1) and `Rect`.
  - `crates/pw-viewcore/src/exposure.rs`: `Ev` (a private field, hashable), the model of a small
    value type with `from_*` constructors, and its tests.
  - `crates/pw-loupe`: builds no `JobView`; the `Backend` implementors outside `native.rs` and
    `core.rs`'s test backends (`tests/engine.rs`, `pw-loupe` `histogram.rs` and `main.rs`,
    `pw-render/tests/sync.rs`) take the trait's defaults, so a new default method needs no change
    there; a changed signature of the two stoppable renders does for the ones that override them
    (`native.rs`, `core.rs`'s `Stoppable` and `Exposed`; the reviewer found no others: **grep
    `fn render_half_stoppable` and `fn render_region_stoppable` in the workspace** and change
    every override).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/wb.rs` (new),
  `crates/pw-viewcore/src/lib.rs`, `crates/pw-viewcore/src/backend.rs`,
  `crates/pw-viewcore/src/native.rs`, `crates/pw-viewcore/src/core.rs`,
  `crates/pw-viewcore/src/state.rs` (the `JobView` field and its literals only),
  `crates/pw-viewcore/tests/native_files.rs`, `docs/test-inventory.md` (rule 12), and this
  task's own file (status and log). If a test backend outside these files overrides a stoppable
  render, its file is added to this list and named in the log (the task was guessed wrong there).
- **Model:** Opus — the backend's FFI path and more than about four files (README, "Model").
- **What:**
  1. **The balance value** (`wb.rs`, public module; `lib.rs` declares `pub mod wb;` and `pub use
     crate::wb::{Wb, WbInfo, WbPreset};`):
     - `pub enum Wb { Shot, Mul { r: u32, b: u32 } }` (`Clone, Copy, Debug, PartialEq, Eq, Hash,
       Default` with `Shot` the default): the multipliers of red and blue **relative to green**, in
       thousandths (green is 1000), so the value can be part of a hashed job key. Fields are
       public; `Wb::from_multipliers(m: [f32; 3]) -> Option<Wb>`: `None` unless all three are finite
       and above 0 and the largest is at most 64 times the smallest (as `pw_libraw::Multipliers`);
       otherwise `Mul { r: round(1000 · m[0] / m[1]), b: round(1000 · m[2] / m[1]) }` (`f32::round`),
       **and then the same ratio is checked on the rounded values** (`max(r, b, 1000) <= 64 ·
       min(r, b, 1000)`, in integers), `None` when it fails (rounding can push a ratio of exactly 64
       over it: (64, 41, 1) gives r 1561 and b 24, 65.04; the rounded `Wb` must always be one
       `pw_libraw::Multipliers::new` takes); `Wb::multipliers(self) -> Option<[f32; 3]>`
       (`None` for `Shot`, else `[r / 1000, 1, b / 1000]`); `Wb::is_shot(self) -> bool`.
     - `pub struct WbPreset { pub name: String, pub wb: Wb }` (`Clone, Debug, PartialEq, Eq`) and
       `pub struct WbInfo { pub presets: std::sync::Arc<[WbPreset]>, pub daylight: Option<Wb>, pub
       camera: Option<Wb>, pub measurable: bool }` (`Clone, Debug, Default, PartialEq, Eq`; the
       presets are an `Arc` so that the state can hand the list to every frame without copying
       it): what a file gives once when it is opened (the file's own table, LibRaw's daylight, the
       camera's own balance for the hover text, and whether the RAW layout can be measured: the
       auto balance and a click on gray need it, W6-Q2).
     - `pub fn grey_area(size: ImageSize, point: [f64; 2], scale: f64) -> Option<Rect>`: the sensor
       rectangle a click on grey measures (W6-Q2). `point` is the clicked point in the **upright**
       image's edge coordinates (`0..=ow`, `0..=oh`; the shell works it out from the drawn view),
       `scale` the shown scale (screen pixels per image pixel). `None` when `size` is not valid, the
       point or the scale is not finite or `scale` is not above 0, or the point lies outside the
       image. Otherwise: the upright square of 15 screen pixels, `15 / scale` image pixels a side,
       centred on the point (a named constant `GREY_SQUARE_PX = 15.0`), turned into the sensor's
       rectangle by `geometry::sensor_box` (outwards, even origin), then **grown to at least 12 ×
       12 photosites** (a named constant `GREY_MIN_PHOTOSITES = 12`; W6-Q2: every colour of a 6 × 6
       X-Trans pattern is in it), per axis: `side = max(box side, 12)`, at most the image's side;
       `centre = box origin + box side / 2` (an `f64`); the new origin is `centre − side / 2`
       rounded to the **nearest even number** (`((v / 2.0).round() * 2.0)`), then clamped to `0 ..=
       ((image side − side) & !1)`, so the rectangle is **moved inside the image, not cut** (the
       owner accepts this deviation from "cut at the image's edge": every colour stays in it; ADR
       0027 §4.6 says so) and with an odd image side it may end one photosite short of the edge; an
       image smaller than 12 photosites on a side uses the whole side.
     No dependency on `pw-libraw`: `wb.rs` is plain Rust and builds without the `native` feature.
  2. **The job carries it** (`state.rs`, and the literals in `core.rs`'s tests): `JobView` gets `pub
     wb: Wb` (a `pub(crate)` struct, so `pub` only inside the crate; `use crate::wb::Wb`). Every
     literal gets `wb: Wb::Shot`, in `state.rs` (the plan's, `const STATISTICS_LEND`, about 20 in
     its tests; `..job` and `..view` literals need nothing) and in `core.rs`'s tests (`half_view`,
     `zoom_view` and the literals about lines 1530–1710)
     (in this task the state asks for nothing else; `ViewOut`, `Resident` and `Look` do not get
     the field yet: T350's). Nothing else in `state.rs` changes.
  3. **The backend** (`backend.rs`): `render_half_stoppable` and `render_region_stoppable` get a
     parameter `wb: Wb` after `exposure: Ev` and before `stop`; the defaults ignore it (a backend
     that has no balance, and JPEG and TIFF, render as before; the doc comments say so). Three new
     default methods, each documented with ADR 0027 §3.5 and W6:
     - `fn white_balance_info(&self, _session: &Self::Session) -> WbInfo { WbInfo::default() }`:
       asked once after a successful open (`measurable` false in the default);
     - `fn auto_white_balance(&self, _session: &mut Self::Session) -> Option<Wb> { None }`: the
       balance of the whole image by W6-Q1's rule (it may take as long as the statistics, so the
       core asks for it as a job, T350); `None` when the file has none or on a failure;
     - `fn grey_white_balance(&self, _session: &mut Self::Session, _area: Rect) -> Option<Wb> {
       None }`: the balance that makes the mean of `area` (a sensor rectangle, `grey_area`'s)
       grey, by the same rule; `None` when it cannot be measured (too bright or too dark, W6-Q2) or
       on a failure.
  4. **The native backend** (`native.rs`): for `NativeSession::Raw`, the two stoppable renders call
     `render_half_adjusted` and `render_region_adjusted` with `pw_libraw::Adjust { exposure:
     <as today>, white_balance: raw_white_balance(wb)? }`, where `raw_white_balance(wb: Wb) ->
     Result<WhiteBalance, DecodeError>` gives `AsShot` for `Shot` and, for `Mul { r, b }`,
     `Multipliers::new(r / 1000, 1.0, b / 1000)` as `WhiteBalance::Multipliers`, a `None` there
     (not reachable with a `Wb` from `from_multipliers`, which checks the rounded values) giving `DecodeError::new(format!("the white
     balance {r}/{b} is outside the range"))` and no render. JPEG and TIFF sessions ignore the
     balance. `white_balance_info` gives, for RAW, the presets (name and `Wb::from_multipliers`;
     one that gives `None` is left out), `daylight()` and `camera_multipliers()` converted the same
     way, `measurable` from `Session::can_measure()`, and the default for JPEG and TIFF.
     `native.rs`'s `use pw_libraw::{…}` gets `Adjust`, `WhiteBalance` and `Multipliers`.
     `auto_white_balance` calls `Session::channel_means` of `pw_libraw::Rect::new(0, 0,
     s.width(), s.height())` and `ChannelMeans::multipliers`, then `from_multipliers`;
     `grey_white_balance` the same of `area` (converted to `pw_libraw::Rect`); an `Err` or a `None`
     anywhere gives `None`. JPEG and TIFF give `None`.
  5. **The worker** (`core.rs`): `render_view` passes `view.wb` to the two stoppable renders. The
     test backends `Stoppable` and `Exposed` take the new parameter (a test backend that records
     what it was given is the way to test it). Nothing else changes: no new task yet (T350).
- **Tests to add:**
  - `wb.rs`: `from_multipliers` gives `Mul { r: 2415, b: 1568 }` for (2473, 1024, 1606), `Shot`
    is not produced from (1, 1, 1) but `Mul { r: 1000, b: 1000 }` is, and `None` for a zero, a
    negative, NaN, +∞, a ratio of 64.01 and (64, 41, 1), whose ratio is exactly 64 before rounding
    and 65.04 after it; `multipliers()` of
    `Mul { r: 2000, b: 500 }` is `[2.0, 1.0, 0.5]` and of `Shot` `None`; `Wb::default()` is
    `Shot`.
  - `wb.rs`, `grey_area` on an `ImageSize { w: 6000, h: 4000, flip: 0 }` (the numbers worked
    out by `sensor_box` and the rule above): at scale 1 and the point (3000, 2000) the box is 16
    × 16 at (2992, 1992) and stays; at scale 8 (800 %) the box is 3 × 3 at (2998, 1998), grown to
    12 × 12 at (2994, 1994); at scale 0.25 it is 60 × 60 at (2970, 1970); at the point (0, 0) and
    scale 1 the box is 8 × 8 at (0, 0), grown to 12 × 12 at (0, 0); at (6000, 4000) the box is 8 ×
    8 at (5992, 3992), grown and moved to 12 × 12 at (5988, 3988); with `w` 6001 the same point
    gives the origin 5988 (the rectangle ends one photosite short); with `flip` 6, 5 and 3 on a
    6000 × 4000 sensor and scale 1 the upright point (3000, 2000) gives the 16 × 16 rectangle
    around the sensor point `to_sensor` gives (assert its centre within 1 photosite; for flips 5
    and 6 the upright width and height are swapped, so use the point (2000, 3000) there); `size.w
    = 8` gives the whole side; a point outside, NaN, scale 0 and an invalid size give `None`.
  - `core.rs`, `render_view` with a test backend that records the balance it was given: a `Half`
    and a `Zoom` view with `wb: Wb::Mul { r: 2000, b: 1500 }` pass exactly that to it; a view
    with `Wb::Shot` passes `Shot`.
  - `backend.rs`: a backend that overrides nothing gives `WbInfo::default()`, `None` and `None`
    from the three new methods (the defaults).
  - `tests/native_files.rs` (ignored, on the owner's files, with its helpers): the four existing
    calls of the stoppable renders in the exposure tests (about lines 307, 327, 354 and 357) get
    the new argument `Wb::Shot`; for an ARW, `render_half_stoppable` with `Wb::Shot` and `Ev::ZERO`
    gives the bytes of `render_half` of the same session (as the exposure's test at about lines
    298–338 does); with the camera's own balance (`white_balance_info(..).camera`)
    the mean absolute difference per byte from `Wb::Shot` is below 0.5 (the thousandths round it
    a little); with `Mul { r: 3000, b: 600 }` the red mean is higher and the blue lower than as
    shot; an ARW in JPEG mode and a TIFF file ignore a balance (no byte changes);
    `auto_white_balance` is `Some` for every ARW and RAF and its `Mul` has r and b within 200–8000
    (printed); `grey_white_balance` of a 12 × 12 area near the middle of an ARW equals what `channel_means` of
    that area and `multipliers()` give through `Wb::from_multipliers` (`None` is allowed where
    the area is more than half clipped or dark; print it), and of a whole-image area equals
    `auto_white_balance`; `white_balance_info` of an ARW has a preset
    "Daylight", a `daylight` that is `Some` and `measurable` true, of a JPEG-mode session the
    default.
- **Not in this task:** the state (which balance each view gets, the mode, the jobs for the auto
  balance and the click, the cache, the frame's states: T350, T352); keys, menu, marks and the
  pick mode (T351, T353).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text). No new
  dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added; waits for T348, the cold read and the chain review |
| 2026-11-28 | Blocked | Cold read and chain review: `from_multipliers` checks the ratio on the rounded values; the `grey_area` rule worked out (nearest even origin, moved inside, the numbers at scale 8 and at the corners); `WbInfo.measurable` and the preset list as an `Arc`; the `JobView` literals in `core.rs`'s tests and the four calls in `native_files.rs`; the baseline of the native test; the line numbers |
| 2026-11-28 | Blocked | T348 done; T354 (the means made faster, owner 2026-11-28, W6-Q7) now comes before this task: waits for T354 |
| 2026-11-28 | Open | T354 done; set to Open by the reviewer (the cold read and the chain review are above) |
