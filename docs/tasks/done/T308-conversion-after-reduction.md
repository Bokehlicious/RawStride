# T308. ADR 0027 W1: the conversion to the monitor profile moved after the reduction — Done

- **Why:** ADR 0027 §3.2, §7 (ADR 0016's order) and W1-Q4/W1-Q5 (answered by the owner
  2026-11-24, ADR 0027 §4.1): today the backend converts every RAW and JPEG view to the monitor
  profile right after the render, before the view core reduces it. The Lanczos filter of T307
  works in linear light with the sRGB curve, which is only right on LibRaw's sRGB output (or the
  JPEG's own colour space), not on monitor-profile data. This task moves the conversion: the
  backend renders unconverted, and the view core converts each view **after** reducing it, in a
  backend step of its own. The box filter stays in this task (T309 replaces it), so what the user
  sees changes very little (a box mean of sRGB data, then converted, instead of a box mean of
  converted data). The chain: T307 → **T308** → T309.
- **What the reviewer checked (2026-11-24):**
  - `pw-viewcore` `native.rs`: `NativeSession::Raw(Session, Arc<Transform>)`, `NativeSession::Jpeg(
    JpegSession)` with `transform` and `full` (the whole decode, kept for the 100 % views), and
    `NativeSession::Tiff(TiffSession)`, whose `image` is converted once when the file is opened
    (`pw_tiff::decode_rgb8_for_display`). `convert(transform, img)` is called in `render_half`
    (Raw, Jpeg), `render_region` (Raw; Jpeg when it makes `j.full`) and in `jpeg_statistics` (when
    it makes `j.full`). `jpeg_statistics` and `jpeg_clipping_map` work around a converted `j.full`
    (they decode once more when the transform is not the identity).
  - `pw-viewcore` `core.rs`, `Task::Render`: `Half` → `render_half`; `Zoom`/`Ring` →
    `render_region`; `Scaled` → `render_region`, then `scale::downscale`; then the consistency
    check. `Task::Focus` makes the focus map from `render_half` (ADR 0017 U9-Q2).
  - The trait `Backend` (`backend.rs`) has default methods (`profile_unusable`, `statistics`,
    `clipping_map`); the simulated backends in `core.rs`, `backend.rs`, `tests/engine.rs`,
    `pw-render/tests/sync.rs`, `pw-loupe` `histogram.rs` and `main.rs` implement the required ones
    and a few defaulted ones (`Mapped` in `core.rs` has `clipping_map`, `Recording` in `pw-loupe`
    has `statistics`), none `to_display`, so a new method with a default needs no change there.
    Only `ViewCore` and `native_files.rs`' `timed_half` call the renders.
  - With a monitor profile other than sRGB, the RAW and JPEG focus maps change slightly (made
    from the unconverted render): intended (W1-Q4).
  - The wait for T307 is order only; this task does not use `pw-resample`.
- **Crates:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/backend.rs` (point 1),
  `crates/pw-viewcore/src/core.rs` (point 2), `crates/pw-viewcore/src/native.rs` (points 3–5 and
  its tests), `crates/pw-viewcore/tests/native_files.rs` (point 6), `docs/test-inventory.md`
  (rule 12), and this task's own file (status and log).
- **What:**
  1. **`Backend::to_display`**, a new trait method with a default:
     ```rust
     /// Converts a rendered and reduced view of the opened image to the display (the monitor
     /// profile), after the reduction (ADR 0016, ADR 0027 §3.2, W1-Q4). The default leaves it
     /// unchanged.
     fn to_display(&self, _session: &Self::Session, img: Rgb) -> Result<Rgb, DecodeError> {
         Ok(img)
     }
     ```
  2. **`core.rs`, `Task::Render`:** after the render and, for `Scaled`, after `scale::downscale`,
     call `self.backend.to_display(&session, img)`, then the consistency check as today. The
     order is render → reduction → `to_display` → check. `Task::Focus` does **not** call it: the
     focus map is made from the unconverted half-size render (W1-Q4; its threshold unchanged).
     Write a comment at both places naming W1-Q4.
  3. **`native.rs`, the renders return unconverted:** `render_half` and `render_region` no longer
     call `convert` for `Raw` and `Jpeg`. `j.full` is kept **unconverted** (its doc comment says
     so), and a `Zoom`/`Ring` crop of it is converted by `to_display` like any other view.
  4. **`native.rs`, `to_display`:** `Raw(_, t)` → `convert(t, img)`; `Jpeg(j)` →
     `convert(&j.transform, img)`; `Tiff` → `Ok(img)` unchanged (W1-Q5: a TIFF stays converted
     when it is opened). A failed conversion returns the same `DecodeError` as today's `convert`.
  5. **`native.rs`, statistics and clipping map of a JPEG:** since `j.full` is unconverted, both
     use it directly when it is there. When it is not: the statistics decode it once and keep it
     for the 100 % views (no conversion); the clipping map decodes once and does **not** keep it,
     as today. Their results do not change (they were always of the unconverted decode, ADR 0017
     U7-Q5).
  6. **Doc comments** that describe the old order are updated: the module comment of `native.rs`
     ("Every image is converted…"), `JpegSession.full`, `jpeg_statistics`, `jpeg_clipping_map`,
     and the trait docs of `render_half` and `render_region` in `backend.rs` ("before the
     conversion to the display, `to_display`").
  7. **`tests/native_files.rs`:** the helper `timed_half` calls `backend.to_display(&session, …)`
     after `render_half` (and includes it in the time), so `a_camera_raw_is_converted_to_the_
     monitor_profile_in_both_modes` keeps checking the shown, converted view; its asserted values
     do not change.
- **Tests:** existing tests that check the **shown** colours of a render keep their expected
  values; they now get them through `render_* → to_display`. This is the only change allowed to
  existing tests in this task, and it is made in the helpers, not in the asserts: `jpeg_renders`
  in `native.rs` (used by `a_plain_jpeg_is_converted_from_srgb_to_the_destination`,
  `a_jpeg_with_the_destinations_own_icc_profile_is_unchanged`,
  `a_jpeg_with_the_exif_adobe_rgb_marker_is_converted_from_adobe_rgb`; its second render, the
  `again == region` check, goes through `to_display` too) and `timed_half` (point 7).
  Where a test calls `render_region` directly and compares with a converted crop
  (`jpeg_statistics_are_taken_before_the_conversion_whatever_was_rendered_first`,
  `a_jpegs_clipping_map_is_made_before_the_conversion_whatever_was_rendered_first`), the call
  becomes `render_region` followed by `to_display`, with the same expected crop; and an assert
  that `j.full` is (or is not) present stays as it is (point 5 keeps the clipping map's
  behaviour, so `j.full.is_none()` after a clipping map before any render still holds). A comment
  that names the converted full decode (for example "leave the converted full decode") is
  corrected. If any other existing test fails, stop and explain (rule 5). The existing focus test
  `the_workers_make_the_focus_map_from_the_half_size_render…` stays valid unchanged (its backend
  keeps the default `to_display`). **Tests to add** (in `native.rs`'s tests unless named):
  - With a non-sRGB destination (Adobe RGB): a JPEG's `render_half` and `render_region` return
    the **plain** decode (unconverted), and `to_display` of them equals today's converted result;
    for a TIFF, `to_display` changes nothing. (RAW is covered by point 7's file test.)
  - A JPEG session: after the first `render_region`, `j.full` is `Some` and equals the plain full
    decode; a second crop leaves it the same; both crops are unconverted, and `to_display` of each
    equals the converted crop.
  - `jpeg_statistics` after a 100 % render equals the one before any render (as today) and leaves
    `j.full` present; `jpeg_statistics` before any render leaves `j.full` present and unconverted.
  - `core.rs`: a **new** simulated backend in its tests (for example 4000×3000, as `Fake` in
    `tests/engine.rs`, so the fit scale on an 800×600 view is below 0.5), rendering a **flat
    colour** (so "render plus 1" still holds after T309's Lanczos reduction), whose `to_display` adds 1
    to every byte (saturating) and records the size it got. Assert: the number of `to_display`
    calls equals the view renders (focus renders not counted); every view the core gives out is
    its render plus 1; for a `Scaled` view (`set_view(Zoom::Scale(0.75), …)`) the recorded size is
    the reduced size; the focus map equals `FocusMap::of_rgb8` of the unconverted half-size
    render.
  - `core.rs`: a `to_display` that fails gives the frame `Status::Error` with its message, as a
    failed render does today.
- **Not in this task:** the Lanczos filter and `pw-resample` (T309); the new view kind and the
  gestures (T309); TIFF's conversion at open (W1-Q5, unchanged); `pw-tiff`, `pw-colour`,
  `pw-render`, `pw-loupe` (no change).
- **Rules:** `docs/tasks/README.md` rules 1–12; no `Cargo.toml` change.
- **Done when:** `.\tools\check.ps1` is all OK; `cargo test --workspace -- --ignored
  a_camera_raw_is_converted_to_the_monitor_profile_in_both_modes` with `PW_TESTDATA` set passes,
  or the log says it could not be run and why; and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W1, W1-Q4/W1-Q5); waits for T307 |
| 2026-11-24 | Blocked | Cold read: point 5 contradicted an existing assert (the clipping map now keeps no decode, as today); the stale doc comments named (point 6); the simulated backends' description corrected; the core tests get a new 4000×3000 backend and countable asserts; "decodes once" made observable; the focus maps' small change with a non-sRGB monitor noted |
| 2026-11-24 | Open | T307 Done |
| 2026-11-24 | Ready for review | Points 1–7 done: `Backend::to_display` (default unchanged); the core converts after the render and the reduction, before the check, not for the focus map; RAW/JPEG renders and `j.full` unconverted; `to_display` converts RAW and JPEG, leaves TIFF; JPEG statistics and clipping map use `j.full` as it is (the clipping map keeps no decode); doc comments; `timed_half` through `to_display`. Existing tests changed only in the helpers (`jpeg_renders`, new `shown_region`, `timed_half`) and two comments. 5 tests added (3 in `native.rs`, 2 in `core.rs`); the core test sets the view at 0.75 near a corner (`[0.15, 0.15]`), because at the centre the 100 % ring already covers the region and no scaled view is rendered. With one flat colour the focus map is the same converted or not; that the focus render is not converted is shown by counting the renders (renders = view renders + focus maps). `.\tools\check.ps1` all OK (2590 passed, 79 ignored, 0 failed); `a_camera_raw_is_converted_to_the_monitor_profile_in_both_modes` with `PW_TESTDATA` passed |
| 2026-11-24 | Done | Reviewed: the diff holds only the named files; points 1–7 and every test asked for are there; existing tests changed only in their helpers and two comments; no other caller of the renders. `test-output.txt` (15:16, after the last change) all OK. The reviewer ran the two new core tests six times: all passed (the test waits with a fixed 200 ms sleep). Noted, not changed: with a flat image the focus map's assert cannot tell converted from unconverted; counting the renders shows it instead. Nothing left over; no CHANGELOG line (the colours change very little; T309 gets the line). Approved by the owner. T309 stays Blocked: T307's timing (440 ms) is above point 10's 100 ms, so the rows are split over threads first, in a task of its own, T311 (owner 2026-11-24) |
