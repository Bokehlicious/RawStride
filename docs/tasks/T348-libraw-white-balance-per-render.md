# T348. ADR 0027 W6: the white balance per LibRaw render, the means under a RAW area and the file's presets — Open

- **Why:** ADR 0027 §3.5 and W6-Q1–W6-Q4 and W6-Q7 (answered by the owner 2026-11-28, ADR 0027
  §4.6). The white balance is applied in LibRaw as `user_mul`, passed **with each render** and
  never left in the session (as the exposure, T331), so the focus job, the statistics and the
  clipping map always run on ADR 0006 §2's settings (A-Q6). The auto rule and the click on grey
  both need the mean of each colour over a RAW area, and the presets need the file's own
  multipliers and LibRaw's daylight multipliers read at the open. This task makes the shim's and
  `pw-libraw`'s part and measures what the later tasks depend on. The first task of W6's chain
  **T348** → T349 → T350 → T351 → T352 → T353; nothing uses the new functions yet.
- **What the reviewer checked (2026-11-28):**
  - `crates/pw-libraw/csrc/pwlr.cpp`: `pwlr_open` sets `use_camera_wb = 1`, `use_auto_wb = 0` and
    leaves `use_camera_matrix` at LibRaw's default once (line 165) and never `user_mul`; `pwlr_render` (line 297) sets `user_qual`, `half_size`,
    the exposure and the crop box per render, then `dcraw_process`; `pwlr_info` (line 73) is the
    struct `pwlr_open` fills (T331 added `camera_ev` and `camera_ev_from` before `error`);
    `pwlr_layout_of` (line 491), `PwLayout`, `PwLimits` (black, white and shadow limit per CFA
    colour, line 432) and `pwlr_statistics_of` (line 578, which already scales the camera's
    multipliers so the smallest is 1) are what the means below reuse; `pwlr_statistics` (line 669)
    and `pwlr_clipping_map` (line 775) are the models of a function that works on the unpacked
    RAW data without LibRaw and without exceptions.
  - LibRaw 0.22's headers (`vcpkg_installed\x64-windows-static-md\include\libraw`, the main
    checkout has them): `imgdata.params.user_mul[4]` (float); `imgdata.color.cam_mul[4]` and
    `pre_mul[4]` (float); `rgb_cam[3][4]` (float); `WB_Coeffs[256][4]` (int, R G1 B G2, indexed
    by `LIBRAW_WBI_*`: Daylight 1, Fluorescent 2, Tungsten 3, Flash 4, FineWeather 9, Cloudy 10,
    Shade 11, FL_D 12 … FL_L 16, Ill_A 17 … Ill_C 19, D55 20, D65 21, D75 22, D50 23,
    StudioTungsten 24, Sunset 64, Underwater 65, FluorescentHigh 66, HT_Mercury 67; AsShot 81,
    Auto 82 and 85–88, Custom 83 and 90–95, PC_Set 96–100, Measured 110, BW 120, Kelvin 254,
    Other 255 are not presets) and `WBCT_Coeffs[64][5]` (float: the temperature in K, then R G1 B
    G2). By LibRaw's `scale_colors` (as dcraw's) a non-zero `user_mul[0]` is copied into `pre_mul`
    **first**, and then `use_camera_wb` (with a valid `cam_mul`) copies the camera's multipliers
    **over it**: with `use_camera_wb` left at 1 a `user_mul` has no effect. A render with a
    balance must therefore set `use_camera_wb = 0` (point 1); the render then scales the
    multipliers so the smallest is 1 (the highlight mode is "clip", `highlight = 0`). All of this
    is [T] and **is checked by point 6's tests, not taken as read**. Turning `use_camera_wb` off
    also changes what LibRaw's `use_camera_matrix` (default 1: "use the embedded colour matrix
    only when the camera's balance is used") does, so a balanced render sets `use_camera_matrix =
    3` ("always use it when the file has one") to keep the colour conversion the as-shot render
    has; the equality test of point 6 shows whether that holds. `rawdata.color` is what the shim
    reads at the open (the statistics do the same, `pwlr.cpp` lines 402–404).
  - `crates/pw-libraw/src/lib.rs`: `Exposure` (line 95) and `Session::render` (line 665, private)
    with the FFI declaration of `pwlr_render` (line 303) and the one call (line 688);
    `render_half_exposed` (line 767), `render_region_exposed` (line 822) and the private
    `render_region_checked` (line 833, which also takes the `Exposure`) are what the view core
    calls (through `pw-viewcore` `native.rs`, which keeps working: the signatures of the public
    ones stay); `Info` (line 46) and its literal in `open_with_diagnostics` (lines 496–509);
    `Session::statistics` (line 568) and `clipping_map` (line 596) as models; `Rect` (line 379)
    and `Rgb8` (line 394).
  - `crates/pw-libraw/tests/exposure.rs`: the model for a test on the owner's files (`PW_TESTDATA`,
    `#[ignore]`, `files_with`, the printed timing with `--release --nocapture`) and for the
    "nothing is left in the session" and region-identity tests.
  - The owner's files (ExifTool 13.64, ADR 0027 §4.6): Sony ARW 15 named levels (Daylight, Cloudy,
    Shade, Tungsten, Flash, Fluorescent and three variants, 2500, 3200, 4500, 6000, 8500 K);
    Panasonic RW2 7 entries; Fujifilm RAF "Standard" and "Auto" only; DNG (Ricoh GR III `TN*`,
    Xiaomi `IMG_*`) `AsShotNeutral` only. Which of them LibRaw gives is what point 4 finds out.
- **Crate:** `pw-libraw`. **Files:** `crates/pw-libraw/csrc/pwlr.cpp`,
  `crates/pw-libraw/src/lib.rs`, `crates/pw-libraw/tests/white_balance.rs` (new),
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — the C++ shim and the FFI (README, "Model").
- **What:**
  1. **The balance per render (shim):** `pwlr_render` gets a parameter `const float *user_mul`
     right after `exp_shift` (a pointer to four floats R, G1, B, G2, or null for the camera's
     balance as shot). Every render sets, before `dcraw_process`: with a pointer, `user_mul[0..4]` to
     the four values, `use_camera_wb = 0` and `use_camera_matrix = 3`; with a null pointer
     `user_mul[0..4] = 0`, `use_camera_wb = 1` and `use_camera_matrix = 1` (LibRaw's default, as
     today), so a render without a balance leaves no trace of an earlier one and gives the bytes
     it gives today (point 6's first test). A pointer whose values are not all
     finite and above 0 is refused before LibRaw is called (after the exposure's check, before the
     stop's first ask), with the error `"white balance multipliers are not usable"`, and the
     session is untouched (not spent). The comment above `pwlr_render` and the file's head comment
     (lines 3–10, which name the camera's white balance among the fixed settings) say that the
     exposure and the balance are the settings passed per render, and that `use_camera_wb` and
     `use_camera_matrix` follow the balance.
  2. **The balance per render (Rust):** a public type `WhiteBalance` (`Clone, Copy, Debug,
     PartialEq`): `WhiteBalance::AsShot` and `WhiteBalance::Multipliers(Multipliers)`; and
     `Multipliers` (`Clone, Copy, Debug, PartialEq`) with a private field `[f32; 3]` (R, G, B):
     `Multipliers::new(r, g, b) -> Option<Multipliers>` is `None` unless the three are finite, above
     0 and the largest is at most 64 times the smallest; `get(self) -> [f32; 3]`. The shim gets
     `[r, g, b, g]` (G2 = G). A new public struct `Adjust { pub exposure: Exposure, pub
     white_balance: WhiteBalance }` (`Clone, Copy, Debug, PartialEq`, `Adjust::NONE`). `Session::render`
     (private) takes an `Adjust` instead of the `Exposure`. Two new public renders:
     `render_half_adjusted(&mut self, adjust: Adjust, stop: &(dyn Fn() -> bool + Sync)) ->
     Result<Rgb8>` and `render_region_adjusted(&mut self, r: Rect, margin: u32, demosaic:
     Demosaic, adjust: Adjust, stop: ...) -> Result<Rgb8>`, documented like the exposed ones (the
     same stop and its cost) plus the balance. `render_half_exposed` and `render_region_exposed`
     stay with their signatures and call them with `WhiteBalance::AsShot`; every other render
     passes `Adjust::NONE`. The module's head comment (lib.rs lines 5–13) names the exposure and
     the balance as the per-render settings.
  3. **The means under a RAW area (W6-Q1, W6-Q2):** a shim function `pwlr_channel_means(session,
     x, y, w, h, out, error)` and `Session::channel_means(&mut self, r: Rect) ->
     Result<Option<ChannelMeans>>`, with `pub struct ChannelMeans { pub mean: [f64; 3], pub used:
     [u64; 3], pub total: [u64; 3] }` (`Clone, Copy, Debug, PartialEq`): over the photosites of
     the **visible image in sensor orientation**, per colour (both greens of a mosaic are G, as the
     statistics). It judges **whole cells**, as the clipping map does (`pwlr_clipping_map_of`:
     cells of 2 × 2 photosites for a Bayer pattern and a three- or four-colour image, 3 × 3 for
     X-Trans, counted from the image's top left corner), so that dim or bright areas do not skew
     the colours' means (leaving out single dark photosites would raise the weak colour's mean,
     and single clipped ones would lower green's): only the cells that lie **fully inside `r`**
     are looked at; a cell is **left out** when any photosite in it is at or above its colour's
     white level (before any balance) or when every photosite of it is below the deep-shadow limit
     (the clipping map's two rules, `PwLimits`). `total` is the number of photosites of the looked-at cells per colour, `used` the number in
     the cells that are kept, and `mean` the mean of the used photosites after the black level is
     taken off (a `double`, in the RAW's own levels, no balance); a colour with no used photosite
     has `mean` 0. It reuses `pwlr_layout_of`, `PwLimits` and `PwLayout.rest` (the black level
     pattern is already in the layout; nothing is factored out of `pwlr_statistics_of`, and its
     results must not change: point 6's tests hold it). `r` must be non-empty and inside the
     image (the sum `x + w` is checked for overflow), else `Err` (as `render_region`); a spent
     session gives the `spent:` error as `statistics` does. `Ok(None)` for a layout
     `statistics()` gives `Ok(None)` for. Allocates nothing that can throw, as the statistics.
     A rectangle may be as large as the whole image (the auto rule, W6-Q1), so it is one pass over
     the cells, not a per-pixel call. The multipliers from it, in plain Rust (so they are tested
     without a file): `ChannelMeans::multipliers(&self) -> Option<Multipliers>`: `None` unless
     every colour has `used × 2 >= total` (W6-Q2's "at least half"; `total` 0 gives `None`) **and** a
     mean above 0;
     otherwise `mul(c) = mean(G) / mean(c)` for R and B, 1 for G, as `Multipliers::new` (a ratio
     above 64 gives `None`).
  4. **The presets and the daylight multipliers (W6-Q4):** read once in `pwlr_open` after a
     successful unpack, from `rawdata.color`, and kept in the session. Rust: `Session::
     white_balance_presets(&self) -> &[Preset]` with `pub struct Preset { pub name: String, pub
     multipliers: Multipliers }`, `Session::daylight(&self) -> Option<Multipliers>`,
     `Session::camera_multipliers(&self) -> Option<Multipliers>` (the file's `cam_mul`, for the
     tests and the hover text) and `Session::can_measure(&self) -> bool` (the layout is one
     `channel_means` can read: `pwlr_layout_of` passed at the open; it costs no pass over the
     data). These are fields of `Session`, filled in `open_with_diagnostics`. **The FFI:**
     `pwlr_info` gets, before `error`, `float cam_mul[4]`, `float pre_mul[4]` (as read at the
     open), `float matrix_mul[4]` (the daylight multipliers worked out below), `int daylight_from`
     (0 none, 1 `pre_mul`, 2 the matrix), `int measurable`, `int preset_count`, `unsigned char
     other_ids[16]` and `int other_count` (the ids with coefficients that are not presets, at most
     16) and `long long presets_ns` (the time the shim took to read all of this, with
     `std::chrono::steady_clock`), mirrored in `Info` and its literal alike; the presets
     themselves by one function `pwlr_presets(session, pwlr_preset *out, int max) -> int`
     (`struct pwlr_preset { char name[24]; float mul[4]; }`, the Rust mirror `#[repr(C)]`),
     called by `open_with_diagnostics` once. The shim keeps at most **96** presets (the 28 named
     ids and the 64 temperature rows); a public accessor `Session::white_balance_diagnostics(&self)
     -> WbDiagnostics` (`pub struct WbDiagnostics { pub cam_mul: [f32; 4], pub pre_mul: [f32; 4],
     pub matrix_mul: [f32; 4], pub daylight_from: u8, pub other_ids: Vec<u8>, pub presets_ns: u64
     }`, documented as for tests and logs) gives the rest to the tests. **Names:** the ids of the list above (1–24, 64–67) named as LibRaw
     names them in `libraw_const.h` without the prefix, spaced and capitalised as the menu will
     show them: "Daylight", "Fluorescent", "Tungsten", "Flash", "Fine Weather", "Cloudy", "Shade",
     "Fluorescent D", "Fluorescent N", "Fluorescent W", "Fluorescent WW", "Fluorescent L",
     "Illuminant A", "Illuminant B", "Illuminant C", "D55", "D65", "D75", "D50", "Studio Tungsten",
     "Sunset", "Underwater", "Fluorescent High", "Mercury Vapor"; each `WBCT_Coeffs` row with a
     temperature above 0 is "{n} K" (whole kelvin, rounded). The names are this task's own list
     (not all are LibRaw's: `HT_Mercury` is "Mercury Vapor"). A row whose **R, G1 or B** is not
     finite or not above 0 is left out (W6-Q4); G2 is not looked at (many makers leave it 0, and
     the balance uses `[r, g, b, g]`), as is a preset with the same name as an earlier one. The
     order is the ids' order, then the temperatures ascending. Ids outside the list are not
     presets; they are kept in `other_ids` for the tests to print. **Daylight (W6-Q4):** the first thing to
     find out [T]: whether `pre_mul` read in `pwlr_open` (before any render) is LibRaw's daylight
     multipliers or a copy of the camera's balance. `matrix_mul` is worked out as dcraw does for
     D65 (**not** from `rgb_cam`, whose rows each sum to 1 and would give 1 for every colour):
     `cam_rgb[i][j] = sum over k of cam_xyz[i][k] * xyz_rgb[k][j]` with `imgdata.color.cam_xyz`
     (4 × 3) and dcraw's sRGB/D65 matrix `xyz_rgb = {{0.412453, 0.357580, 0.180423}, {0.212671,
     0.715160, 0.072169}, {0.019334, 0.119193, 0.950227}}` (a named constant in the shim, a comment
     says it is dcraw's), `num[i] = sum over j of cam_rgb[i][j]` and `matrix_mul[i] = 1 / num[i]`
     for i = 0..2 (and `matrix_mul[3] = matrix_mul[1]`); all zero when `cam_xyz` is all zero or a
     `num` is not above 0. Write the code with `pre_mul` as it is, run the `daylight` test of
     point 6 (it prints, per file, `cam_mul`, `pre_mul` at the open and `matrix_mul`), then finish
     this point: if `pre_mul` differs from `cam_mul` and equals `matrix_mul` within 0.5 % (as
     ratios to green), use `pre_mul` (`daylight_from` 1); if it equals `cam_mul` (or is 0) and
     `matrix_mul` is usable, use `matrix_mul` (2); if neither is usable the file has no daylight
     (0, `None`). The comment says which was found on which files. The multipliers are given as
     LibRaw has them (not scaled).
  5. **The measurement (W6-Q1, W6-Q6, W6-Q7):** in `tests/white_balance.rs`, an `#[ignore]` test
     `timing_of_white_balance` (run with `--release --nocapture`, as `timing_of_exposure`) that
     prints, per ARW and RAF file of `PW_TESTDATA`, the median of 5: `Session::open`; `statistics()`;
     `channel_means` of the whole image; the sum of open, whole-image means and a half-size
     render with the balance that `channel_means(whole).multipliers()` gives (a step to an image
     that was not prefetched, with auto white balance on); the sum of open, statistics, means and
     the render (with auto exposure on as well, two passes in a row; W6-Q1 asks whether the means
     can share the statistics' pass: they cannot, the statistics bin per pixel and the means
     judge cells, so the log says what the two passes cost together); a half-size render at
     `Adjust::NONE` and with a non-trivial balance; a whole-image PPG render at both. The numbers
     go in the log row. It also prints `white_balance_diagnostics().presets_ns` for every file
     (the shim's own time for the presets and the daylight read at the open, W6-Q4's
     "negligible"; microseconds are expected, and the log says what it was). **ADR 0027 §4.6**
     (the cost with everything off): the reviewer records `latency.ps1` with every adjustment off
     at the current head **before this task starts** and again after T353; this task needs
     nothing of it beyond the figure above.
  6. **The statistics, the clipping map and the focus job stay as shot:** nothing in
     `pwlr_statistics`, `pwlr_clipping_map` or `pwlr_open`'s settings changes with the balance; the
     tests below show it.
- **Tests to add** (`tests/white_balance.rs`, on the owner's files, `#[ignore]` with the reason of
  `exposure.rs`, `PW_TESTDATA` and its helpers copied, with lists for ARW, RAF, RW2 and DNG; the
  unit tests in `lib.rs`'s `mod tests`):
  - Unit: `Multipliers::new` gives `Some` for (2.4, 1, 1.6) and (1, 1, 1) and `None` for a zero, a
    negative, NaN, +∞ and a ratio of 64.01 (and `Some` at exactly 64); `Adjust::NONE` has
    `AsShot` and 0 EV. `ChannelMeans::multipliers`: means (100, 200, 50) with every colour fully
    used give (2, 1, 4); a colour with `used × 2 < total` gives `None`, with `used × 2 == total`
    `Some`; a mean of 0 gives `None`; means whose ratio is above 64 give `None`.
  - A render at `WhiteBalance::AsShot` (half-size and a PPG region) gives the bytes of the plain
    render of a fresh session, on an ARW and a RAF (a half-size RAF render is not deterministic,
    T327, so only its size is compared there).
  - A render with `Multipliers` equal to `camera_multipliers()` gives the same bytes as `AsShot`
    (the balance passed as the camera's own changes nothing: this shows that `user_mul` replaces
    the camera's balance, that `use_camera_matrix = 3` keeps the colour conversion and that the
    smallest-is-1 scaling is LibRaw's), on an ARW and an RW2 and a DNG (half-size), and on a RAF
    as a PPG region. The shim passes `[r, g, b, g]`, so it equals the camera's own only where
    `cam_mul[3]` is 0 or equals `cam_mul[1]`: on a mismatch print `cam_mul[3]` before stopping. If
    it does not hold, stop and report. This test and the direction test together are the guard
    that the balance reaches the render.
  - A render with a balance followed by one at `AsShot` of the same session: the second gives the
    plain bytes (nothing is left in the session).
  - The region identity with a balance: a region of 600×400 px at a third of the image's width and
    height (PPG, margin 8) with a non-trivial balance (for example 1.3, 1, 0.8 times the
    camera's) equals the same area of the whole-image render with it, on an ARW and a RAF.
  - The direction: with the red multiplier of `camera_multipliers()` raised by 30 % the mean of
    the red channel of a half-size render rises, and with the blue one lowered by 30 % the mean of
    the blue channel falls, on every ARW and RAF.
  - `statistics()` and `clipping_map()` after a render with a balance equal those of a fresh session.
  - `channel_means` of the whole image of every ARW, RAF, RW2 and DNG: `used <= total` per colour,
    `mean` finite; `multipliers()` is printed for every file and is `Some` with values within
    0.2–8 for every file whose colours all have `used × 2 >= total` (the others are printed);
    of a 12×12 area at an even origin near the middle: for a Bayer file `total` is exactly 36, 72,
    36 (R, G, B), for an X-Trans file each colour's `total` is at least 9, and for a three- or
    four-colour (linear) DNG 144 each (printed with the file's kind); at the same area read twice
    the same result; an area outside the image and an empty one give `Err`; two adjacent areas
    whose edges lie on cell borders add up to the whole area (counts, and the weighted means).
  - `channel_means` does not change what `statistics()` gives (fresh session before and after).
  - `white_balance_presets()` and `camera_multipliers()`: print per file the names and the
    multipliers; **`level-c\PM-raw.ARW`** of the owner's folder (in a subfolder, so opened by its
    path, and skipped when missing) has a preset named "Daylight" whose R/G and B/G are within 1 %
    of ExifTool's `WB_RGBLevelsDaylight` (2473 : 1024 : 1606); every ARW of the list is printed
    with its presets (a file without "Daylight" is reported, not failed); every RW2 has at least
    one preset; for every DNG and RAF the list is printed (it may be empty); every preset's
    multipliers are valid; no two presets of a file share a name; no file has more than 96. The
    ids with coefficients that are not presets are printed once per file (the ids), and a file
    that reached the limit of 96 is named.
  - `daylight`: the printout of point 4 (`white_balance_diagnostics()`); for every ARW
    `daylight()` is `Some` and within **15 %** of the "Daylight" preset's multipliers (R/G and B/G;
    LibRaw's are for D65 and the camera's preset is about D55, so a few percent differ), or the
    test stops and reports (then point 4's rule is wrong for that file); both values are printed.
  - `can_measure()` is true for every ARW, RAF, RW2 and DNG of the owner's folder.
  - Point 1's refusal is a defence the Rust API cannot reach (`Multipliers::new` checks first); no
    test calls the shim directly. The shim has no environment variable or test build of its own.
- **Not in this task:** the view core's use of any of it (T349, T350, T352); keys, menu, marks and
  the pick mode (T351, T353).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text; the preset
  names are data of the shim, the menu's texts are T351's). No new dependency. An extra code review
  follows (the C++ shim).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above;
  the log holds the timings, the presets found per file and the finding of point 4.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added; waits for the cold read and the chain review |
| 2026-11-28 | Blocked | Cold read and chain review: `use_camera_wb` off (and `use_camera_matrix` 3) for a balanced render, since LibRaw copies the camera's multipliers over `user_mul`; the daylight fallback worked out from `cam_xyz` × dcraw's sRGB/D65 matrix (`rgb_cam`'s rows each sum to 1); the means judge whole cells as the clipping map does; the FFI of the presets and the diagnostics named; the shim times its own preset read (no environment variable); the 96-preset limit and G2 not checked; the daylight test within 15 %; `PM-raw.ARW` opened by its path; exact counts for a 12 × 12 area; `can_measure`; the statistics' time beside the means; the line numbers; the baseline `latency.ps1` is the reviewer's, before this task |
| 2026-11-28 | Open | Set to Open by the reviewer after the owner's approval of the chain T348–T353; T349–T353 wait in turn. The reviewer records the baseline `latency.ps1` (every adjustment off) before an agent starts this task |
| 2026-11-28 | Open | Baseline `latency.ps1` before this task (ADR 0027 §4.6), run by the owner: release build of `a1c7e88`, `s6\a1`, the preview at fit, every adjustment off, 13 steps × 2.5 s (log `pm-study\results\run-log-w6.txt`, label `w6-fit-base-a1`): every step seen; first change median 6.0 ms (steps 2–13: 5.2–9.2 ms, median 6.05 ms); step 1 51.4 ms (settled 114 ms, two changes: the next image was not yet prefetched when the series started, so it is left out of the comparison). Memory after 13 steps 2.4 GB. W5's baseline was 5.5 ms (max 10.5); the difference is within the run-to-run spread. The comparison after T353 uses steps 2–13 |
