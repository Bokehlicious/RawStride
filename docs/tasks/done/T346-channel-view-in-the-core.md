# T346. ADR 0027 W4: the channel view's function, its timing and the channel in the view core — Done

- **Why:** ADR 0027 §3.6 and W4-Q1, W4-Q2, W4-Q4, W4-Q6, W4-Q7 (answered by the owner 2026-11-26,
  ADR 0027 §4.5). The channel view shows the red, green or blue channel, or the luminance, as
  grey. This task makes the function that does it, times it, and has the view core apply it last
  on the worker, after the sharpening and before the conversion to the display, as part of every
  view's look, with the frame's state for the shell's mark. Nothing in the program turns it on
  yet; T347 adds the keys, the menu and the mark. The first task of W4's chain **T346** → T347.
- **What the reviewer checked (2026-11-26):** the shadows (T332, T333) and sharpening (T316) go
  through every place a view's look must go, and the channel follows them:
  - `crates/pw-viewcore/src/core.rs`: `render_view` (line 148): the render, the reduction, the
    shadows (line 197), the sharpening (line 207), then `backend.to_display` (line 215) and the
    consistency check. `ViewCore::set_sharpening` (line 428) and `set_shadows` (line 442) go
    through `update_adjustment`. The statistics, the clipping map and the focus map call
    `render_half` directly (line 129), not `render_view`, so the channel never reaches them
    (§2.1). The render tests near line 1398 use the backend `Exposed` (about lines 1404–1478),
    whose `pixel: fn(u32, u32) -> u8` makes every pixel grey `[v, v, v]` and whose `to_display`
    is the default identity (`backend.rs` line 513); they build `JobView`s by hand (about lines
    1491 and 1502).
  - `crates/pw-viewcore/src/state.rs`: `Look` (line 472) with `sharpen`, `exposure`, `shadows`,
    and its `look()` for `Resident`, `ViewOut` and `JobView` (lines 478–509); every comparison of
    looks (`serving_of`, `exact`, `satisfied`, `replaced_fine`, `remove_inflight`) goes through
    `look()`; `serves()` (about line 1653) compares only the region and the density, as for
    sharpening. The one direct comparison is `accept`'s fine branch (about lines 2986–2988).
    `ViewOut` (line 114) and `Resident` (line 136) carry `shadows`; it is copied where a
    `ViewOut` is made in `finish_render` (about lines 2736 and 2760) and into the `Resident` in
    `accept` (about line 3029). `JobView`s are built in the plan with `shadows:
    self.shadows_of(key)` (about lines 1555, 1575, 1609, 1618, 1628, 1637); `shadows_of` (line
    1071) gives `None` for a key that is not RAW. The pan view (`..shown.job`, about line 1514)
    and the half-size relabel in `finish_render` (`..view`, about line 2713) carry the look by
    themselves; the neighbours' prefetch (about lines 1419–1440) goes through the same requests.
    Other `JobView` literals: the `const STATISTICS_LEND` (line 565, non-test code), and in the
    tests about lines 3586, 3780, 3793, 6618, 6634 and 7299; the test helpers `out_in` (about
    line 3599, which builds a `ViewOut` from a job) and `o_job` (about line 4394, a `JobView`
    from a `ViewOut`). `set_shadows` (line 987) sets `self.since`. `frame_of` (line 3076) builds
    the `Frame` literal (about line 3088) with `shadows: AdjustState::Off` and sets
    `frame.shadows = self.shadows_state(key, &frame)` (line 3205); `adjust_state` (line 3220)
    gives `Off`, `Paused` (a key that is not RAW), `Waiting` or `Shown`, and `Shown` only while
    the status is ready, no gesture is under way and auto exposure is not waiting for its
    statistics (line 3237). `gesture_stand_in` (line 3328) picks the densest reduced (or fine)
    view of the current image during a zoom gesture, whatever its look; `satisfied` lets a
    half-size view of any look serve the shown image during a gesture (`any_look`, about line
    1770). Tests with a JPEG key use `set_mode(Source::Jpeg)` (about line 4537) or a key
    `(10, Source::Jpeg)` (about line 4136).
  - `crates/pw-viewcore/src/lib.rs`: `pub mod sharpen;`, `pub mod shadows;` and `pub use
    crate::sharpen::Sharpening;`; `AdjustState` is exported from `state`.
  - `crates/pw-viewcore/src/sharpen.rs`: the model of a function on `Rgb` (from `backend.rs`:
    `width`, `height`, `data` as RGB bytes, `is_consistent()`), returning `None` for an
    inconsistent image, with an `#[ignore]` timing test (T315).
  - `crates/pw-loupe/src/loupe_text.rs`: the test helpers `frame` and `tiff_frame` (about lines
    331 and 567) build `Frame` literals, which need the new field (`small_tiff_frame` uses
    `tiff_frame`). No other crate builds a `Frame`, `JobView`, `ViewOut` or `Resident`.
- **Crates:** `pw-viewcore`, `pw-loupe` (test helpers only). **Files:**
  `crates/pw-viewcore/src/channel.rs` (new), `crates/pw-viewcore/src/lib.rs`,
  `crates/pw-viewcore/src/core.rs`, `crates/pw-viewcore/src/state.rs`,
  `crates/pw-loupe/src/loupe_text.rs` (the test helpers' `Frame` literals only),
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — the view core's state and cache (the look of every view, the frame's state,
  the gesture's stand-in), the type of T316 and T333 (README, "Model").
- **What:**
  1. **The channel and the function** (W4-Q2), in `channel.rs`, plain Rust, no new dependency:
     - `pub enum Channel { Color, Red, Green, Blue, Luminance }`, deriving `Clone, Copy, Debug,
       Default, PartialEq, Eq, Hash`, with `Color` the default (American spelling, as the
       program's texts, glossary G5);
     - `Channel::next(self)` and `Channel::previous(self)`: the order Color → Red → Green → Blue
       → Luminance → Color, and back;
     - `pub fn apply(img: &Rgb, channel: Channel) -> Option<Rgb>`: `None` for an image that is
       not consistent; `Color` gives a copy; `Red`, `Green` or `Blue` give each pixel that
       channel's value v as (v, v, v); `Luminance` gives each pixel (y, y, y) with
       `y = (2126 · r + 7152 · g + 722 · b + 5000) / 10000` in integers (`u32`), which is Y′ of
       W3-Q2 rounded to the nearest level and never above 255. An image without pixels is
       copied. No indexing that can panic (chunks of 3, as `sharpen.rs` reads them).
     - `lib.rs`: `pub mod channel;` and `pub use crate::channel::Channel;`.
  2. **Its speed** (W4-Q7): written on one thread first. The timing test below is run in a
     release build (`cargo test --workspace --release -- --ignored --nocapture
     timing_of_channel_view`; the owner approved release runs for timing tests on 2026-11-24).
     If the **median** for `Luminance` on 3840×2160 is above **10 ms**, the rows are split over
     threads with `std::thread::scope` (as T311 and T315 did), the result the same byte for
     byte, and the timing run again; if it is still above 10 ms, report it in the log and do not
     optimise further: the owner decides. Write every median (both sizes, both channels) in the
     log.
  3. **In the render** (W4-Q1): `JobView` gains `pub channel: Channel`. In `render_view`, after
     the sharpening and before `backend.to_display`, a channel other than `Color` is applied
     with `channel::apply`; a `None` gives `DecodeError::new("the decoder returned a malformed
     image")`, as the sharpening's does. `Color` does not call it. It applies to every
     `ViewKind`, and for every source (RAW, JPEG, TIFF). `STATISTICS_LEND` gets `channel:
     Channel::Color`.
  4. **In the state** (W4-Q1, W4-Q4):
     - `State` holds `channel: Channel` (`Color` at the start, in `State::new`; a new folder or
       list of files does not change it), and `pub fn set_channel(&mut self, c: Channel) ->
       bool`, which returns whether it changed and, on a change, sets `self.since` as
       `set_shadows` does.
     - `Look` gains `channel`, and so do `ViewOut` and `Resident` (copied where `shadows` is
       copied: in `finish_render`'s `ViewOut`s and `accept`'s `Resident`), so a view of another
       channel is another view in every comparison that uses `look()`.
     - Every `JobView` the plan builds gets `channel: self.channel` for **every** key, RAW or
       not (unlike `shadows_of`: the channel view applies in JPEG mode and to JPEG and TIFF
       files, §2.2). The neighbours' prefetch is therefore made in the current channel too, so
       each change of channel makes the shown image's and the neighbours' views again (the cost
       W4-Q4 accepted). `accept`'s fine branch also asks `out.channel == self.channel`.
     - `ViewCore::set_channel(&self, c: Channel)` in `core.rs`, through `update_adjustment` as
       `set_shadows`, with a doc comment (ADR 0027 §3.6, W4; the shell holds it for the session,
       not saved; not a user request, nothing is recorded).
     So a change of channel plans the wanted views again, a render of the old channel under way
     finishes (`stop_unwanted` is not changed: owner, 2026-11-25, never stopped for a changed
     setting of the current image; confirmed for the channel 2026-11-26), and the drawn view stays until one with the new channel
     replaces it in one swap (V2), as sharpening does (W3-Q4).
  5. **The gesture's stand-in** (§2.1, from the chain review): during a zoom gesture
     `gesture_stand_in` may today draw a view of another look, which for a grey channel shows
     the image in colour while the mark says "R". It now prefers a view whose `look()` is the
     wanted one (the look first, then the density and the fine kind as today); with none of the
     wanted look it takes the best of the others as today. This also applies to sharpening, the
     exposure and the shadows, for which it is only better.
  6. **The frame's state** (W4-Q6): `Frame` gains `pub channel: AdjustState` (doc comment: the
     state of the channel view for the current image, ADR 0027 §3.6, W4), `AdjustState::Off` in
     `frame_of`'s literal (with the other states, so the early returns give `Off`), and set
     beside `frame.shadows` by a `channel_state(key, &frame)` that gives what `adjust_state`
     gives with `on = self.channel != Channel::Color` and `matches = r.channel ==
     self.channel`, **except** that it is never `Paused` (the channel view applies to every
     source). Factor `adjust_state` (for example a parameter saying whether a source that is
     not RAW pauses it) rather than copying it; the exposure's and the shadows' states must not
     change. Its other conditions apply to the channel as they are, on purpose: during a gesture
     it is `Waiting`, and while auto exposure waits for its statistics it is `Waiting`, since
     the view is made again with the new exposure. The test helpers' `Frame` literals in
     `loupe_text.rs` get `channel: AdjustState::Off`; nothing else in `pw-loupe` changes.
  7. **The test literals:** every `JobView` literal in the tests of `core.rs` and `state.rs`
     gets `channel: Channel::Color`; the helpers `out_in` and `o_job` **copy** it (`channel:
     job.channel`, `channel: o.channel`), or no test view could ever carry a channel.
- **Tests to add:**
  - `channel.rs`: a 2×1 image (255, 0, 0), (10, 200, 30): `Red` gives (255, 255, 255), (10, 10,
    10); `Green` (0, 0, 0), (200, 200, 200); `Blue` (0, 0, 0), (30, 30, 30); `Luminance` of
    (255, 0, 0) is 54, of (0, 255, 0) 182, of (0, 0, 255) 18, of (255, 255, 255) 255, of
    (0, 0, 0) 0, of (10, 200, 30) 147; `Color` gives the image unchanged; an inconsistent image
    (data one byte short) gives `None` for every channel; a 0×0 image is copied; `next` and
    `previous` go round all five and back in the order of point 1; `Channel::default()` is
    `Color`.
  - `#[ignore = "a time measurement; run with --release (ADR 0027 W4-Q7)"] fn
    timing_of_channel_view()`: synthetic images of 3840×2160 and 1920×1200, `Red` and
    `Luminance`, at least 11 runs each, printing the median and the maximum in milliseconds.
  - `core.rs`: `Exposed` makes only grey pixels, on which no channel changes anything, so give
    it a coloured pattern: an optional second pixel function (for example `colour: Option<fn(u32,
    u32) -> [u8; 3]>`, `None` in every existing constructor, so the existing tests are
    unchanged) or a new small backend beside it. With a coloured, varying pattern: a view with
    `channel: Red` reaches `to_display` as `channel::apply(&plain, Red)`; with sharpening and
    `Red` the result is `apply(sharpen(plain))`, and the test asserts that this differs from
    `sharpen(apply(plain))` for its data (the channel after the sharpening); with the shadows,
    sharpening and `Luminance` the result is `apply(sharpen(lift(plain)))` (the order shadows →
    sharpening → channel); `Color` leaves the view as without the field.
  - `state.rs`: `set_channel` reports a change and none for the same value; the plan's
    `JobView`s carry the channel for a RAW key **and** for a JPEG key (and the shadows stay
    `None` for the JPEG key), and so does a prefetched neighbour's job; after a change of
    channel a view of the old channel is not `exact` for the new job and new views are planned,
    while the drawn view stays until the new one is accepted (as the sharpening's tests near
    line 7648 show it); Red then Green before the red view has arrived: Green stays wanted, a
    late red view accepted meanwhile does not make the state `Shown`, only the green view does;
    `Frame::channel` is `Off` with `Color`, `Waiting` until a view with the channel is drawn,
    `Shown` then, `Waiting` after a return to `Color` while the red view is still drawn and
    `Off` once the color view is drawn; for a JPEG key (through `set_mode(Source::Jpeg)` or a
    JPEG file's key, as the tests near lines 4136 and 4537) it is `Shown`, never `Paused`; a
    fine view of another channel does not take the place of the wanted one in `accept`; during
    a gesture, with a reduced view of the wanted channel and a denser one of another channel
    resident, `gesture_stand_in` draws the wanted channel's. The exposure's and the shadows'
    existing state tests pass unchanged.
- **Not in this task:** the keys, the menu, the mark and the reset (T347); the TIFF's channel
  being the display's (W4-Q3, accepted, `known-limits.md`); the histogram, the clipping overlay
  and the focus marking, which do not change (§2.1).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13. No new dependency. No text the user
  sees. The independent code review follows (the view core's state and cache).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Blocked | Written |
| 2026-11-26 | Open | Cold read (fresh agent): the named files suffice; `Exposed` makes only grey pixels, so the render tests could not show the order (a coloured pattern asked for); `STATISTICS_LEND`, the test literals and the helpers `out_in` and `o_job` named (they must copy the channel); the neighbours' prefetch in the channel said; the `AutoWaiting` and gesture conditions said to apply on purpose; the JPEG key's route in the tests named; `render_view`'s line fixed. Chain review (T346–T347, fresh agent): `gesture_stand_in` drew a view of any look (point 5 added, with a test); a test for Red then Green before the red view arrives and one for a prefetched neighbour added; the 1920×1200 medians logged too; W4-Q6 added to Why. Spelling `Color` (glossary G5, from T347's cold read). Set Open |
| 2026-11-26 | Ready for review | Implemented the channel view in rendering, planning, cache identity and frame state. Release timing on Windows (build 26300, Core i7-9700K, synthetic RGB, 11 runs per size and channel), median/max in ms. Single thread at 3840×2160: Red 9.248/10.268, Luminance 13.914/14.586. At 1920×1200: Red 2.505/2.972, Luminance 3.975/4.392. With up to eight row bands at 3840×2160: Red 6.191/6.773, Luminance 7.205/7.976. At 1920×1200: Red 1.691/1.774, Luminance 2.108/2.226. A test checks byte-for-byte agreement with a single pass. The full workspace check passed with the `C` locale for ExifTool (2890 passed, 99 ignored). |
| 2026-11-26 | Ready for review | Review: point 4's text said a render of the old channel is stopped, against the owner's rule of 2026-11-25 (T328: never stopped for a changed setting of the current image); the agent had added `view.channel != self.channel` to `stop_unwanted` and removed the rule from its doc comment. The owner chose to keep the rule for the channel (2026-11-26). The reviewer removed the condition, restored the doc comment, made the Red-then-Green test assert the red render is not stopped, fixed point 4's text, rewrapped two comments and completed `ViewCore::set_channel`'s doc comment (not saved). |
| 2026-11-26 | Done | Approved by the owner; `.\tools\check.ps1` all OK (2890 passed, 99 ignored, no clippy warnings). Nothing left over: the TIFF's channel (W4-Q3) is in `known-limits.md`; the keys, the menu and the mark are T347. No change-log line (nothing turns the channel view on yet). |
| 2026-11-26 | Done | Independent review after the commit (fresh agent; it had been missed before b4c3676): nothing to fix. Nits: a doc comment line of `gesture_stand_in` over 100 characters (the reviewer's own rewrap; fixed); the Red-then-Green test leans on the resident order for its drawn colour view and does not assert the picked key (kept; its channel-state assertions hold); the stand-in prefers unexposed views while auto exposure waits (harmless, kept). |
