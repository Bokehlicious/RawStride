# ADR 0027: Display adjustments for assessment

- Status: **accepted by the owner (2026-11-18):** every question answered (§4); A-Q1, A-Q3,
  A-Q4, A-Q6, A-Q10, A-Q14, A-Q15 and A-Q17 as proposed, A-Q2 and A-Q5 changed, A-Q7, A-Q8, A-Q9,
  A-Q11, A-Q12, A-Q13 and A-Q16 with additions. Nothing is built, installed or added as a
  dependency by this ADR. Each step in §5 becomes its own task, with its own approval and commit.
  W1's questions W1-Q1–W1-Q9 answered by the owner on 2026-11-24, all as proposed (§4.1); W2's
  W2-Q1–W2-Q8 the same day, all as proposed (§4.2); W3's
  W3-Q1–W3-Q8 the same day, all as proposed (§4.3).
- Date: 2026-11-18
- Related:
  - ADR 0006 §2 (decode settings identical everywhere; the view already uses PPG instead of its
    AHD, ADR 0008 §11): this ADR lets the **view** differ from them while an adjustment is on
    (§2.1);
  - ADR 0008 and [requirements-image-viewer.md](../requirements-image-viewer.md) (V1–V8, G1–G4):
    every adjustment is measured against them (§2.1); ADR 0008 §11.2 gives the measured cost of a
    full render (§3.1);
  - ADR 0016 (colour management): where each adjustment sits relative to the conversion to the
    monitor profile (§3.2, §7);
  - ADR 0017 §2.6 (technical assessment) and §7.2 (the wish "temporary exposure and white balance
    for assessment"), which this ADR takes up; ADR 0017 U6 (the two-image compare, §6);
  - ADR 0025 §2.2, §2.4 and §2.9 (the export's source, its separable Lanczos filter in plain Rust,
    and its new crate `pw-export`): the same filter is used by the view, in a crate of its own
    (§3.2, A-Q7); the export takes over no adjustment (A-Q14);
  - the backlog item "A region render fails on every RAF", a prerequisite (§5, W0; task T197 since 2026-11-18);
  - [clean-room.md](../clean-room.md): this ADR describes the functions in our own words, from
    RawStride's own needs.

## 1. Context

Judging a RAW file in the preview today means judging LibRaw's neutral rendering at fixed
settings. That is right for a fair, repeatable view (ADR 0006), but it leaves four gaps:

1. **Fine detail at fit is not always from a full demosaic.** What the code does today
   (`pw-viewcore` `state.rs`, `req`) [M, code read 2026-11-18]:
   - fit scale ≤ `HALF_DENSITY` (0.5): view kind `Half`, LibRaw's half-size render (for a Bayer
     sensor each 2×2 cell becomes one pixel, no demosaic), drawn smaller on the GPU from a
     mipmapped texture (`pw-render`, `texture_options`);
   - fit scale between 0.5 and 1: view kind `Scaled`, a full-resolution region render (PPG) reduced
     on the worker with a **box filter** (`scale.rs`, `downscale`);
   - 100 % and above: `Zoom` and `Ring`, full resolution.

   The fit scale is the smaller of the width and height ratios (`geometry.rs`, `fit_scale`), so a
   3:2 image on a 16:9 screen is limited by the height. On a preview area the full height of a 4K
   screen (2160 px), a 61 Mpix A7R IV image (9568×6376) has fit ≈ 0.34 and is shown from the
   half-size render; a 24 Mpix image (6000×4000) has fit ≈ 0.54 and a 26 Mpix image (6240×4160)
   ≈ 0.52, so they get `Scaled` only while the preview area is at least about 2000–2080 px high;
   below that they too are shown from the half-size render. The half-size render shows noise, moiré
   and micro-contrast differently from the full demosaic, and the box filter gives a slightly soft
   reduction.
2. **No sharpening in the view.** A screen-sized image without output sharpening looks softer than
   the same image after any editor's default, so a sharp image can be taken for a soft one.
3. **Fixed brightness and white balance.** `pwlr.cpp` renders with the camera's white balance,
   `no_auto_bright = 1`, `bright = 1.0` and highlights clipped. Many cameras expose to protect
   highlights and lift the result in their own JPEG, so the RAW view is often darker than the
   embedded JPEG, and a well-exposed image can look underexposed. There is no way to look into
   the shadows or to check a colour cast. ADR 0017 §7.2 lists this as a wish.
4. **Fixed thresholds.** The clipping level is the white level LibRaw reports, the deep shadows are
   below 1/256 of it (`pw-libraw` `RawStatistics`, computed in `pwlr.cpp`), and the focus
   threshold is a constant (`FocusMap::FOCUS_THRESHOLD` = 160). None of them can be changed.

A prerequisite: `Session::render_region_with` fails on every RAF file tried (backlog). `Scaled`,
`Zoom` and `Ring` use it, so §3.1 cannot work for RAF until that is fixed.

## 2. Rules for every adjustment

### 2.1 What an adjustment is

- **It changes only what is shown.** No file, sidecar, rating store or cache on disk is changed,
  and nothing is written to XMP. RawStride stays a culling and captioning program, not an editor.
- **It does not change the export** (A-Q14). ADR 0025 §2.2 renders a RAW "with the same settings
  as the view's RAW mode"; that means the view's RAW mode with every adjustment of this ADR off,
  that is the settings of ADR 0006 §2.
- **Measurements are taken from unadjusted data.** The RAW histogram, the exposure statistics,
  the clipping overlay (all from the RAW data, U7) and the focus marking do not change when an
  adjustment is turned on. The focus map is not made from the shown view: it has its own job,
  which renders the image's half-size render again (`core.rs`, `Task::Focus`; ADR 0017 U9-Q2). It
  therefore never sees the downscale, sharpening, channel view or fine details; but exposure and
  white balance are set in LibRaw in the same session (§3.4, §3.5), so the focus job renders with
  the settings of ADR 0006 §2 whatever adjustment is on (A-Q6).
- **It is always visible that an adjustment is on.** The preview's overlay shows a short mark
  for each active adjustment (for example "+1 EV", "WB auto", "USM", "R"), in the same way the
  source is always stated (G1). An adjustment that is on but does not apply to what is shown (for
  example sharpening at 100 %, A-Q8, or exposure in JPEG mode, A-Q9) keeps its mark, shown as
  paused (for example dimmed); it applies again as soon as the view allows it, without a key.
- **It is off by default and is never saved** per image or in `settings.txt` (A-Q2):
  - a value taken from one image is reset when another image is shown: the **manual exposure**
    and the white balance from a **click on grey**;
  - **fine details, sharpening and the channel view** are display modes and last for the session,
    for every image;
  - a choice that is worked out again for every image, or is a fixed choice, is a mode that lasts
    for the session: **auto exposure** and **the camera's brightness** (§3.4), and **auto white
    balance** and the **white balance presets** (§3.5; owner, 2026-11-18);
  - one key resets every adjustment (A-Q3).

  The adjustments' own parameters (USM amount, radius and threshold, the thresholds and colours of
  §3.7) are settings and are saved.
- **V1–V8 apply with every adjustment off,** unchanged. With an adjustment on, the step time is
  measured with the owner's `latency.ps1` (not in the repository; ADR 0008 §5, ADR 0016 F6) and
  reported in its step; a slower step is accepted only by the owner.
- **Caches:** a view rendered with other settings is a different view. The view core's key gains
  the adjustment state, so a change of adjustment is never shown on an old texture; the memory
  budget (V8) is unchanged, so turning adjustments on and off evicts views sooner.

### 2.2 RAW and JPEG mode

| Adjustment | RAW mode | JPEG mode (embedded preview, JPEG and TIFF files) |
|---|---|---|
| Fine details (§3.1) | yes | no (at fit ≤ 0.5 a JPEG is decoded at half size by libjpeg's DCT scaling, a real reduction and not a binning; above it, the full decode is reduced as for RAW) |
| Downscale filter (§3.2) | yes | yes |
| Sharpening (§3.3) | yes | yes |
| Exposure, shadows (§3.4) | yes | no (A-Q9) |
| White balance (§3.5) | yes | no (an 8-bit JPEG's balance cannot be redone honestly) |
| Channel view (§3.6) | yes | yes |

## 3. Decision

### 3.1 Fine details

- A key (A-Q3; **D**, W2-Q4: no item in Settings, nothing saved) turns **fine details** on. While
  it is on, a view whose scale is < 0.5 is made like `Scaled` (full-resolution render, reduced on
  the worker) instead of from the half-size render (W2-Q3; since W1 the reduced view).
- The reduced view is shown first, as now; the fine view replaces it **in one swap** when it is
  ready (V2: never an intermediate frame), after the dwell (today 150 ms), on the background
  worker once the neighbours' views are ready (W2-Q2).
- Demosaic: **PPG**, as `Scaled` today (A-Q4; ADR 0008 §11). At fit the differences between PPG
  and AHD are mostly lost in the reduction; whether AHD is offered at 100 % stays open (ADR 0008
  §11.3).
- Prefetch (A-Q5): **only the current image** gets the fine view. A LibRaw render cannot be
  cancelled once it has started (`pwlr.cpp` has no progress callback), so a fine render for a
  neighbour would hold the CPU while the user steps on. Prefetching +1 in the direction of travel
  is added only if W2's measurement with `latency.ps1` shows that V1 and V4 hold while a fine
  render runs; otherwise it is a later question.
- Cost: a full PPG render of a 61 Mpix file is the most expensive job the view core runs. ADR 0008
  §11.2 measured it [M, P4b, i7-9700K, 2026-11-05]: a whole A7R image (9568×6376) takes 1012 ms
  with PPG on all threads (AHD 1871 ms) and 1926 ms on one thread (AHD 5936 ms); a 16 Mpix RW2
  (4608×3464) 347 ms with PPG on all threads. Fine details is therefore a "look closer" mode, not
  a browse mode. W2 measures it again on the owner's files in the app [T].

### 3.2 A better downscale filter

- The box filter in `pw-viewcore` `scale.rs` is replaced by a **separable Lanczos-3 filter in
  linear light** (A-Q7), the result clamped to 0–255. Done on the worker, so the GPU only draws at
  1:1.
- **One filter for the view and the export** (A-Q7): it lives in a new workspace crate without
  dependencies (for example `pw-resample`), in plain Rust, with an 8-bit and a 16-bit version.
  `pw-viewcore` and `pw-export` (ADR 0025 §2.9) depend on it; the owner approved these dependency
  lines on 2026-11-18 (AGENTS.md §3). No new package. The crate is made by W1 or by ADR 0025's X2,
  whichever is built first. Rejected: the filter in `pw-colour` (the view core would need
  `pw-colour`, and so `moxcms`, as a non-optional dependency), in `pw-viewcore` (the export would
  depend on the view core) or in `pw-export` (the view core would pull in the metadata code and
  `tiff`).
- The half-size view, today drawn reduced by the GPU from a mipmapped texture, is reduced on the
  worker to the screen size by the same filter. During a zoom gesture the view core asks only for
  views at a scale ≤ 0.5 (`state.rs`, `wanted`), today the half-size view; a half-size view
  reduced to the fit size would be drawn enlarged during a gesture between fit and 50 %; the
  unreduced half-size view is kept for gestures (W1-Q3, owner 2026-11-24).
- Today every view is converted to the monitor profile in the backend, before the box filter
  (`native.rs`, `convert` in `render_half` and `render_region`). This ADR moves the conversion
  after the downscale and the post steps (§7, ADR 0016), so the filter works on LibRaw's sRGB
  output (or the JPEG's own colour space), where linear light is well defined. A TIFF stays
  converted when it is opened (W1-Q5).
- Applies to every view kind below 100 % and to JPEG mode. The cost is higher than the box
  filter's (floating point); W1 measures it [T].

### 3.3 Sharpening in the view (unsharp mask)

- A key (A-Q3; **U**, W3-Q5) turns sharpening on and off. It is applied **last on the worker**, to the view at
  screen resolution after the downscale (sharpening before a reduction is mostly lost), and
  before the conversion to the monitor profile (ADR 0016).
- On luminance only, so no colour fringes are added.
- Three settings: amount, radius (px) and threshold (levels). Defaults [T, set in the hand check]:
  amount 60 %, radius 0.6 px, threshold 2.
- **Not at 100 % and above** (A-Q8): there the user judges the file's own pixels, and there is no
  reduction to make up for. Sharpening stays on, its mark "USM" is shown as paused (§2.1), and it
  applies again when the user zooms out.
- Cost [T]: a separable Gaussian blur and a difference on ≈ 8 Mpix (4K) is a few milliseconds on
  several threads; it is done in the view core as plain, testable Rust (GR11). A GPU shader is
  considered only if the measurement fails V1.

### 3.4 Temporary exposure and shadows (RAW mode)

- **Exposure:** −3 to +3 EV in steps of 1/3 EV, with keys for up, down and zero (A-Q3). Applied in
  LibRaw (`exp_correc`, `exp_shift`, with `exp_preser` to protect highlights), so a negative
  correction can use the headroom above the camera's white point. Every view of the image is
  rendered again [T: the cost is a new `dcraw_process` per view; the unpacked data is kept in the
  session]. Reset when another image is shown (A-Q2).
- **Shadows:** a lift of the dark tones (a curve on the view, 0–100) applied after the render, **on
  the 8-bit view** (A-Q10): a table lookup per pixel. It shows whether there is detail in the
  shadows; banding in smooth dark areas is accepted for that purpose. If the hand check sees
  banding that hides detail, that step decides how the lift moves into the render (a 16-bit output
  or LibRaw's own tone curve) [T]. Today the whole chain after LibRaw is 8-bit (`output_bps = 8`).
- **Auto exposure** (A-Q11): sets the exposure from U7's RAW statistics (`RawStatistics`, the
  histogram after white balance, scaled to the white level). Our own rule, not LibRaw's
  `no_auto_bright = 0`, so the result is explained by the histogram the user sees:
  - the **99.9th percentile of the brightest channel** is placed **1/3 EV below clipping** (a few
    hot pixels or a specular highlight do not decide the brightness of the whole image);
  - the result is **rounded down to 1/3 EV** and limited to **−3 to +3 EV**, the steps and range
    of the manual exposure; the mark shows for example "+1⅓ EV auto", and up and down go on from
    there;
  - if **more than 0.1 %** of the pixels are already clipped in the RAW data, the result is 0 EV:
    what is clipped in the RAW cannot be brought back, and a negative value would only darken the
    rest;
  - the rule is tuned in the hand check.
- **The camera's brightness** (A-Q12): a choice to apply the camera's own brightness offset (for
  example DNG `BaselineExposure`, and the makers' equivalents where LibRaw or ExifTool gives them
  [T: which formats carry it is checked in W5]), so the RAW view can be compared with the embedded
  JPEG fairly. ADR 0006 §2 left matching the camera JPEG's look as "a separate, later question"
  and required it to be one value per image, never per tile; this offset is such a value.
  - **Off by default**: the default view stays the fair view of ADR 0006, and its brightness does
    not depend on the file format.
  - A mode that lasts for the session (§2.1), read again for every image.
  - **It and auto exposure exclude each other**: turning one on turns the other off. Manual steps
    (±1/3 EV) are added on top of the active mode and are reset when another image is shown.
  - A file without the value says so in the mark (for example "Camera EV: none") and gets 0, so it
    never looks as if the offset applied.

### 3.5 Temporary white balance (RAW mode)

- **As shot** (today), **auto** and **click on grey**:
  - **auto** is worked out once per image, over the whole image, and passed as `user_mul` to every
    view of it. LibRaw's `use_auto_wb` measures the area it renders, so with a `cropbox`
    (`Scaled`, `Zoom`, `Ring`) every region could get a balance of its own and a 100 % view
    another colour than the fit view, against ADR 0006's "consistent between tiles" [T: how the
    multipliers are worked out is decided in W6]. A mode that lasts for the session (§2.1): it is
    worked out again for every image, and the mark "WB auto" is always shown;
  - **click on grey**: the user clicks a neutral area; the multipliers are taken from the mean of
    the RAW data under a small square there (in the same mapping between the RAW and the shown
    image as the clipping overlay, U7) and passed as `user_mul`. Taken from one image, so it is
    reset when another image is shown (A-Q2).
- **Presets** (A-Q13):
  - **from the file**: the camera's own preset table (daylight, shade, tungsten and so on), as
    LibRaw reads it from the maker note (`imgdata.color.WB_Coeffs`) [T, from LibRaw's
    documentation; which presets the owner's files carry is checked in W6]; no generic presets,
    since they need a colour model per camera;
  - **"Daylight (LibRaw)"** for every file: LibRaw's daylight multipliers (`pre_mul`), worked out
    from its colour matrix for every camera it supports [T, checked in W6]; a neutral reference
    that can be compared between cameras. When the file's own table also has a daylight preset,
    both are shown, with different names (G1).
  - A preset is a fixed choice and lasts for the session (§2.1); a file without the chosen preset
    says so in the mark and is shown as shot.
- No temperature/tint sliders in this ADR: they need a colour model per camera that LibRaw does
  not give directly.

### 3.6 Channel view

- A key cycles through **colour → R → G → B → luminance**, each single channel shown as grey. It is
  applied after the render (and after sharpening, if on). In RAW mode it shows the rendered
  channel, not the raw photosites (the clipping overlay already shows the raw data, U7).

### 3.7 Thresholds and the histogram scale

- In Settings (U8's page or the Preview page): the **clipping level** as a share of the white level
  (100 % today), the **deep-shadow level** (1/256 today), the **focus threshold** (160 today) and the
  **colours** of the marks (red, blue, green today; the focus point's colour, §3.9, is added).
- The histogram (U7) gets a choice between a **linear** and an **EV (logarithmic)** horizontal axis,
  so the headroom below clipping can be read in stops.

### 3.8 The camera's crop frame

- Many cameras let the user choose an aspect ratio (3:2, 4:3, 16:9, 1:1) or a crop mode, and save it
  in the maker note while the RAW keeps the whole sensor. A key shows that frame on the image, the
  rest dimmed, drawn in the same mapping between the RAW and the shown image as the clipping
  overlay (orientation, borders, half size). A later step (W8, A-Q15):
  - **the survey comes first**: which of the owner's cameras save the frame, and which tags
    ExifTool gives for it, is not known [T]; if none of the owner's files carries it, W8 is closed
    without building anything;
  - **only frames inside a whole sensor image** are shown; a crop mode that makes the RAW itself
    smaller (for example a smaller-sensor mode on a full-frame camera) has no frame to show.

### 3.9 The focus point

- A key draws the camera's focus point or points (the AF area used, from the maker note) on the
  image, as a frame. It complements the focus marking: the marking shows where the image is sharp,
  the point shows where the camera meant it to be. A later step (W9, A-Q16):
  - the makers in the order of the owner's files: **Sony** (ARW) first, then **Panasonic** (RW2)
    and **Fujifilm** (RAF); Nikon and Canon only if the owner has such files (none in the test
    folders on 2026-11-18);
  - **each maker is a step of its own**, since the tags differ per maker and model [T]; a maker
    whose tags do not give a certain position is left out rather than shown with an approximate
    frame;
  - the focus point and the focus marking can be on together (as U9-Q4); the point has a colour of
    its own (§3.7).

## 4. Questions (answered by the owner, 2026-11-18)

| # | Question | Answer |
|---|---|---|
| A-Q1 | One ADR for all of §3, or split (rendering here, marks in another)? | One ADR; the steps in §5 can be reordered (as proposed) |
| A-Q2 | How long does an adjustment last: per session, per image, or saved between sessions? | **Changed:** the manual exposure and a click on grey are reset when another image is shown; fine details, sharpening and the channel view last for the session; auto exposure, the camera's brightness (A-Q12), auto white balance and the white balance presets are modes that last for the session, worked out per image (auto white balance and presets added by the owner the same day); one key resets everything; nothing is saved, the parameters are (§2.1) |
| A-Q3 | Keys for fine details, sharpening, exposure ±/0, auto exposure, white balance, channel view, crop frame, focus point, and the reset of A-Q2 | Proposed in each step, checked against the key map (U8) (as proposed) |
| A-Q4 | Fine details with PPG or AHD? | PPG (as proposed): at fit the difference is lost in the reduction, AHD takes nearly twice as long (ADR 0008 §11.2) |
| A-Q5 | Fine view prefetched for neighbours? | **Changed:** only the current image in W2; +1 in the direction of travel is added only if W2's measurement shows that V1 and V4 (not V8) hold while a fine render runs, since a LibRaw render cannot be cancelled (§3.1) |
| A-Q6 | Focus marking with or without the adjustments? | Without (as proposed): the focus job renders with ADR 0006 §2's settings, so the marking never changes with a display setting (§2.1) |
| A-Q7 | Filter, linear light or gamma, and the crate shared with ADR 0025's `pw-export`? | Lanczos-3 in linear light, clamped to 0–255; **added:** in a new workspace crate without dependencies, 8- and 16-bit, used by `pw-viewcore` and `pw-export`; the dependency lines approved (§3.2) |
| A-Q8 | Sharpening at 100 % and above? | Off (as proposed); **added:** the mark "USM" is shown as paused, and sharpening applies again when the user zooms out (§3.3) |
| A-Q9 | Exposure in JPEG mode (a gain on 8-bit data)? | No, for exposure and shadows (as proposed); **added:** the mark is shown as paused in JPEG mode (§2.1) |
| A-Q10 | Shadow lift: on the 8-bit view, or in the render? | On the 8-bit view first; moved into the render only if the hand check sees banding that hides detail (as proposed, §3.4) |
| A-Q11 | Auto exposure's rule? | 99.9th percentile of the brightest channel at −1/3 EV (as proposed); **added:** rounded down to 1/3 EV, limited to −3 to +3 EV, 0 EV when more than 0.1 % is clipped in the RAW; tuned in the hand check (§3.4) |
| A-Q12 | Apply the camera's brightness offset by default when it is known? | No, a choice shown in the overlay (as proposed); **added:** a mode for the session, excluding auto exposure, manual steps on top; a file without the value says so (§3.4) |
| A-Q13 | White balance presets only from the file, or also generic ones? | Only from the file (as proposed); **added:** "Daylight (LibRaw)" from `pre_mul` for every file (§3.5) |
| A-Q14 | May the export (ADR 0025) take over the view's adjustments? | No (as proposed); ADR 0025 §2.2's "the same settings as the view's RAW mode" means with every adjustment off (§2.1) |
| A-Q15 | Crop frame (§3.8) in this ADR? | Yes, as a later step (as proposed), after a survey of the owner's files; only frames inside a whole sensor image |
| A-Q16 | Focus point (§3.9) in this ADR? | Yes, as a later step (as proposed); **changed order:** Sony, Panasonic, Fujifilm, Nikon and Canon only with files; a step per maker; its own colour; can be on with the focus marking |
| A-Q17 | 2×2 compare here or later? | Later, as an amendment of U6 (as proposed); moved to §6 |

### 4.1 W1's questions (answered by the owner, 2026-11-24, all as proposed; W1-Q8 and W1-Q9 after T309's cold read)

The filter is not an adjustment (§2.1): it is always on, has no key, no setting and no mark, and
V1–V8 apply to it without exception. Batch and import do not apply: W1 changes only how one shown
image is reduced.

| # | Question | Answer |
|---|---|---|
| W1-Q1 | The crate and what it holds | `pw-resample`, a workspace crate without dependencies, in plain Rust. Lanczos-3, separable, in linear light: the input is taken as sRGB-encoded and made linear with the sRGB curve, the result encoded back with it and clamped to 0–255 (0–65535 for 16 bit); lookup tables for the curve. An 8-bit and a 16-bit version of the same filter. Only reduction: an output larger than the input, a size of 0 or a buffer of the wrong length is refused (no panic), the same size is a copy; the kernel's weights are normalised per output pixel, so the edges and an image of 1×1 or 1×n work. Another transfer curve (for example already linear data) is added by ADR 0025 X2 if the export needs it |
| W1-Q2 | Its speed | The filter is always on, so V1–V8 must hold with it. It runs on one worker thread first; the owner's `latency.ps1` on a release build, on 61 Mpix files at fit, measures it in W1's last task. If V1 does not hold, the rows are split over threads with `std::thread::scope` (no new dependency); if it still does not hold, the owner decides |
| W1-Q3 | The half-size view and zoom gestures (§3.2's open point) | At rest, every view whose scale is below 0.5 (fit or a free level) is the half-size render reduced on the worker to that scale with the filter: a new view kind, not mipmapped, drawn at 1:1. At exactly 0.5 the half-size render is drawn as it is. During a zoom gesture the unreduced half-size view is asked for, as today (mipmapped, reduced by the GPU), so a gesture never makes a new render per frame; it is kept only as the memory budget (V8) allows. The first gesture after a step shows the reduced view enlarged until the half-size view is ready, which replaces it in one swap (V2) |
| W1-Q4 | Where the conversion to the monitor profile goes (RAW and JPEG) | After the reduction (§7): the backend's renders return LibRaw's sRGB output or the JPEG's own colour space, and the view core converts each view after reducing it, in a backend step of its own. The decoded whole JPEG is kept unconverted and each crop (`Zoom`, `Ring`) is converted on its own (a crop is about screen-sized). The focus map is then made from the unconverted half-size render, so it no longer depends on the monitor profile; its threshold is unchanged (A-Q6). For a JPEG in another colour space (for example Adobe RGB) the sRGB curve is an approximation, accepted |
| W1-Q5 | TIFF files | Unchanged: converted when the file is opened (`pw-tiff`, ADR 0015/0016), since a 16-bit TIFF in a wide colour space would lose precision as 8-bit sRGB. The filter then works on the converted data with the sRGB curve, an approximation accepted. The TIFF's half-size view, today made with the box filter, uses the new filter |
| W1-Q6 | Thumbnails and `pw-tiff`'s box filter | Unchanged: thumbnails (`pw-thumbcache`) and the Tauri app keep `pw_tiff::downscale` (§7: the thumbnails do not differ) |
| W1-Q7 | When it goes wrong, and small images | An image whose fit scale is 1 or more is not reduced (`Zoom`, as today). A reduction that fails shows today's error line ("scaling the view failed"); a conversion that fails or a profile that cannot be used behaves as today. Nothing new is shown to the user |
| W1-Q8 | Stepping at a free zoom level below 50 % (asked after T309's cold read, answered 2026-11-24 as proposed) | Today it is quick because the prefetched half-size view serves every scale ≤ 0.5; a reduced view serves only its own scale and below. So while the zoom is a free level below 0.5, the neighbours are prefetched at the current scale instead of at fit; at fit and at 50 % or more the prefetch is unchanged |
| W1-Q9 | The brief enlarged view during a gesture (asked after T309's cold read, answered 2026-11-24 as proposed) | The reduced and the half-size views are magnified linearly (they are magnified only as that stand-in; nearest-neighbour would look blocky); `Zoom`, `Ring` and `Scaled` keep nearest-neighbour, sharp pixels above 100 %. The stand-in is drawn only during a gesture at ≤ 0.5 and counts as "preparing", not as the finished view |

W1 is written as the chain T307 (the crate) → T308 (the conversion moved after the reduction, the
box filter still used) → T311 (the output rows split over threads) → T309 (the filter in the view,
the half-size view reduced, the gestures, and the owner's `latency.ps1` after it); the conversion
moves first because the filter's linear light is only right before the data is converted to the
monitor profile. W1-Q2 (owner, 2026-11-24): T307's timing on one thread was 440 ms (a 61 Mpix
image's half-size render reduced to fit, Intel Core i7-9700K), above the 100 ms a neighbour's
reduction may take at about 7 images/s (V4), so the rows are split over threads with
`std::thread::scope` before the filter is used in the view, in a task of its own (T311).

Decided during T309 (owner, 2026-11-24): (a) the fit view is the reduced view, so tests that
expected the half-size view's size at fit expect the reduced size (`pw-render`
`tests/sync.rs`); (b) just below 0.5, where the reduced size of an odd side would not fit in a
half-size render that rounds down (less than a pixel apart), the half-size view is used as at
0.5, so no render is repeated; and, after T309's review, (c) a half-size render that comes back
smaller than the reduced size is kept as the image's half-size view, which it gets below 50 %
from then on (no render repeated), and (d) a reduced view, which has no mipmaps, is never drawn
reduced by the GPU more than 2:1: below half its density the mipmapped half-size view serves,
also during a gesture.

W1-Q2's measurement (owner, 2026-11-24, after T309's hand check, approved): V1 and V4 hold
with the filter. Next image at fit and at 100 %, median first change 5.4–5.6 ms; at about 7
images/s every step within 70 ms but a series' first press, which a scripted run showed to be
the method (0.4.0 and T309's build alike, 7.1–7.4 ms). No further step is needed.

### 4.2 W2's questions (answered by the owner, 2026-11-24, all as proposed)

Fine details (§3.1) is the first adjustment built. Batch and import do not apply: it changes only
how the shown image is made. The RAW histogram, the clipping overlay and the focus marking do not
change (§2.1).

| # | Question | Answer |
|---|---|---|
| W2-Q1 | The measurement first | An ignored timing test measures a whole PPG render plus the Lanczos reduction to a fit size, on one of the owner's 61 Mpix ARW files and a 16 Mpix RW2, in a release build (the `--release` run approved for this one measurement, as in T307). If the time is far above ADR 0008 §11.2's (about 1 s for 61 Mpix), the owner decides before the view core is changed |
| W2-Q2 | When and where it runs | Only for the current image, after the dwell (150 ms) and never during a zoom gesture. It runs on the background worker, and only when the neighbours' views the plan wants (+1, +2) are ready, so stepping never waits behind a render that cannot be cancelled; the interactive worker stays free. When the user steps on, a fine render that has started finishes and is kept as the memory budget allows |
| W2-Q3 | Where it applies | RAW mode, at every scale below 0.5 (fit or a free level), where W1 shows the reduced view. At 0.5 and above the view is already made from the full render (`Scaled`, `Zoom`), so fine details is paused there; in JPEG mode (embedded preview, JPEG and TIFF files, §2.2) it is paused too. After a zoom gesture that ends at a new scale below 0.5, a new fine render is made for that scale. A fine view also serves smaller scales down to half its own density, as the reduced view does (W1, (d)) |
| W2-Q4 | Key and setting | The key **D** (free in the key map), in the key map (U8) so it can be changed. No item in Settings and nothing saved: fine details lasts for the session (A-Q2); §3.1's "a setting and a key" is read as the key only |
| W2-Q5 | The mark | §2.1 requires that an adjustment that is on is always visible. W2 therefore makes the preview overlay's row of adjustment marks, with its first mark "Fine", shown dimmed while fine details is paused (0.5 and above, JPEG mode). W3 adds "USM" to the same row (moved from W3 to W2) |
| W2-Q6 | Failure and memory | A fine render that fails, or that does not fit the memory budget (V8), leaves the reduced view on screen and shows no error line (a correct view is shown); the mark is shown as paused for that image. The known limit that a failed LibRaw call ends the image's session (`known-limits.md`, LibRaw) applies as before |
| W2-Q7 | Prefetching +1 (A-Q5) | The owner measures with `latency.ps1` with fine details on: stepping at about 7 images/s, and a step that comes while a fine render runs. Only if V1 and V4 hold is a task written for +1 in the direction of travel; otherwise this is written here and fine details stays on the current image only |
| W2-Q8 | The cache | The fine view is a view kind of its own, with its own key in the cache. The reduced view is kept as the memory budget allows, so turning fine details off shows it at once |

W2 is written as the chain T312 (the measurement) → T313 (the view core: the view kind, the plan,
the worker and the cache, `pw-viewcore`) → T314 (the shell: the key, the mark and the glossary,
`pw-loupe`), then the owner's `latency.ps1` and the decision on +1 (W2-Q7).

W2-Q1's measurement (T312, 2026-11-24, release build, i7-9700K, 4K fit size): a fine view of a
61 Mpix ARW takes 1240 ms (the PPG render 1156 ms, about 14 % above ADR 0008 §11.2), of a 50 Mpix
ARW 987 ms and of a 16 Mpix RW2 403–422 ms; today's half-size view of the same files 299, 274 and
139 ms. **The owner accepted the time on 2026-11-24** (not far above §11.2's); T313 goes ahead as
written.

W2-Q7's measurement (2026-11-24, release build of cb1b5e1, `latency.ps1`, fit, RAW, on the owner's
1920×1200 screen; labels `t314-*`): with fine details on, the next image's first change has a
median of 5.2–6.3 ms (A7R and A1; at most 10.8 ms), and a step 500 or 900 ms into a fine render a
median of 5.1–8 ms (at most 34 ms), so V1 holds; at about 7 images/s with a warm cache every step
is at most 61 ms (off: 86 ms), so V4 holds. From a freshly started program, stepping at 7 images/s
misses V4 with fine details on and off alike (the steps come faster than the dwell, so nothing is
prefetched; not caused by W2). The owner's hand check passed (2026-11-24): the image does not move
at the swap. The visible gain is small on that screen, at fit and at 45 % alike. **The owner
decided on 2026-11-24 that no task is written for +1 (A-Q5) and no more work is spent on fine
details**; it stays on the current image only.

### 4.3 W3's questions (answered by the owner, 2026-11-24, all as proposed)

Sharpening (§3.3) is the second adjustment built. Batch and import do not apply: it changes only
how the shown image is made. The RAW histogram, the exposure statistics, the clipping overlay and
the focus marking do not change (§2.1): they are made from the file's data, not from the view.

| # | Question | Answer |
|---|---|---|
| W3-Q1 | Where it applies | Last on the worker, after the reduction and before the conversion to the monitor profile (§3.3, W1-Q4), to every view below 100 % made on the worker: the reduced view, the fine view, `Scaled`, and the half-size view at exactly 50 % (drawn at 1:1). In RAW and JPEG mode, for JPEG and TIFF files, and on both sides of the compare. At 100 % and above (`Zoom`, `Ring`) it is paused (A-Q8). During a zoom gesture the stand-in (the mipmapped half-size view, W1-Q3) is drawn unsharpened, and the mark counts as waiting. **Clarified by the owner 2026-11-24, after T316's cold read:** the half-size view a gesture asks for is made sharpened (it is drawn at 1:1 at 50 % when the gesture ends; an unsharpened one would be rendered twice there and make the mark flip); during the gesture the view already there is drawn, sharpened or not, and the mark shows waiting |
| W3-Q2 | How it is computed | On luminance only, so no colour fringes are added: the luminance Y′ = 0.2126 R′ + 0.7152 G′ + 0.0722 B′ of the sRGB-encoded values (not linear light), blurred with a separable Gaussian whose sigma is the radius; where the difference between Y′ and the blur is below the threshold (levels, 0–255) nothing changes, elsewhere the amount times the difference is added equally to R′, G′ and B′, each clamped to 0–255. The edges repeat the edge pixels, so an image of 1×1 or 1×n works. A module of its own in `pw-viewcore`, plain Rust, no new dependency |
| W3-Q3 | Its speed | The first task times it on a screen-sized view (3840×2160 and the owner's 1920×1200). If it takes more than 10 ms on one thread, the rows are split over threads with `std::thread::scope` in the same task (as T311) |
| W3-Q4 | The cache, and what the user sees when it is turned on or off | The view's key gains the sharpening state (on or off, and the three values), so turning it on or off or changing a value makes new views. The drawn view stays until the new one is ready and is replaced in one swap (V2). The unsharpened view is kept as the memory budget allows, so turning sharpening off shows it at once. Cost accepted: turning sharpening on renders the shown view again (about 0.3 s for 61 Mpix at fit, about 1.2 s with fine details on); keeping the unsharpened pixels in memory to sharpen them in milliseconds is not done (more memory under V8, more code) |
| W3-Q5 | Key and menu | The key **U** (free in the key map), in the key map (U8) so it can be changed, and View ▸ Show Sharpening, right after Show Fine Details. It lasts for the session and is not saved (A-Q2) |
| W3-Q6 | The mark, and when it goes wrong | "USM" in the row of adjustment marks (W2-Q5), after "Fine"; dimmed while paused (100 % and above). The hover text names the key and the three values (for example "Sharpening (U): amount 60%, radius 0.6 px, threshold 2."). Sharpening can fail only on inconsistent image data; such a view then fails as any render does and shows today's error line. Nothing new is shown to the user |
| W3-Q7 | The settings | A new Settings page **Preview** (W7 adds the thresholds and colours of §3.7 to it) with a section Sharpening: amount 10–300 %, radius 0.3–2.0 px, threshold 0–20 levels, and a button Restore Defaults. Defaults 60 %, 0.6 px and 2 [T, set in the owner's hand check]. Saved in `settings.txt`; a value in the file that cannot be read or lies outside its range gives that value's default. A changed value applies when the slider is released (or the field is left), not on every frame of a drag, since each change renders the views again |
| W3-Q8 | The reset of every adjustment (A-Q3; left from W2 by T314) | The key **Backspace** (free in the key map), in the key map, and View ▸ Reset Adjustments: it turns off every adjustment that is on (today fine details and sharpening; later exposure and white balance, W5, W6). The menu item is greyed while no adjustment is on |

After the chain the owner measures with `latency.ps1` with sharpening on, at fit (V1, V4), and
sets the defaults in the hand check.

W3 is written as the chain T315 (the sharpening function and its timing, `pw-viewcore`) → T316
(the view core: the flag, the values, the view's key and the state for the mark, `pw-viewcore`)
→ T317 (the Settings page Preview and `settings.txt`, `pw-loupe`) → T318 (the shell: the key U,
the menu item, the mark "USM", the reset, and the saved values passed to the view core,
`pw-loupe`). The Settings window applies its pages only at Save (as every page today), so W3-Q7's
"not on every frame of a drag" holds: a changed value applies at Save (reviewer, 2026-11-24,
when the tasks were written).

W3-Q3's timing (T315, 2026-11-24, Intel Core i7-9700K, median): above 10 ms on one thread, so
the rows were split; on 8 threads 21.2 ms (radius 0.6) and 27.5 ms (radius 2.0) on 3840×2160,
6.5 and 7.4 ms on 1920×1200, and 40.3 and 69.8 ms on 4800×3200, the half-size view sharpened at
50 %. The owner decided on 2026-11-24: a task of its own before T316, T319, makes the two blur
passes faster with the same output to the byte (the horizontal pass with the taps in the outer
loop, no buffer made per row), times it again, and the owner decides on the result.

## 5. Steps

Each step ends with tests (GR11), the owner's hand check and, where an image is shown,
`latency.ps1`.

| Step | Content |
|---|---|
| **W0** | (Prerequisite, its own task) the region render on RAF fixed (T197) |
| **W1** | The Lanczos filter in its own crate (A-Q7) for every view below 100 %; the half-size view reduced on the worker and its use during zoom gestures; the conversion to the monitor profile moved after the downscale (§3.2) |
| **W2** | Fine details (§3.1): measure a full PPG render of the owner's 61 Mpix files and a smaller format first; then the view kind and the swap; the overlay's row of adjustment marks with "Fine", paused marks included (§2.1; moved from W3, W2-Q5); the measurement that decides the prefetch of +1 (A-Q5) |
| **W3** | Sharpening (§3.3) with its settings; its mark "USM" in the overlay's row of marks (made by W2, W2-Q5) |
| **W4** | Channel view (§3.6) |
| **W5** | Exposure, auto exposure, shadows and the camera's brightness (§3.4); the new LibRaw parameters through `pwlr.cpp` and `pw-libraw`; the focus job kept on ADR 0006 §2's settings (A-Q6) |
| **W6** | White balance: auto (worked out once per image and passed as `user_mul`, the same in every view), click on grey, the file's presets and "Daylight (LibRaw)" (§3.5) |
| **W7** | Thresholds, colours and the EV histogram (§3.7) |
| **W8** | The camera's crop frame (§3.8), after a survey of the owner's files |
| **W9** | The focus point (§3.9), after a survey of the owner's files; one step per maker |

Order: W0, W1, W2, W3, W5, W4, W6, W7, W8, W9.

The steps were called A0–A9 until 2026-11-18, when the owner renamed them W0–W9, since ADR
0023's steps are A1–A3; the questions keep their names A-Q1–A-Q17.

**Placed in the step plan (owner, 2026-11-18):** last, after ADR 0025, in
[`adr-0018-0025-steps.md`](../tasks/plans/adr-0018-0025-steps.md) as order 9. The shared filter
crate (A-Q7) is therefore made by ADR 0025 X2, and W1 takes it into the view. **W0** (the
region render on RAF, a fault today) is moved forward the same day: order 1b, before ADR 0024.

**Moved by the owner on 2026-11-24:** W1–W9 to order 2d, after ADR 0031 RL9 and before ADR 0022
(a change from the metadata work; renaming, copying, moving and export are not in use for now).
W1 therefore makes the shared filter crate (A-Q7, ADR 0025 §2.9: "whichever comes first"), and
ADR 0025 X2 uses it.

## 6. Not included

- Saving any adjustment into the file, a sidecar or XMP; development settings of any kind.
- Noise reduction, lens corrections, local adjustments, temperature/tint sliders, generic white
  balance presets other than "Daylight (LibRaw)".
- Adjustments in the export (A-Q14).
- **Several images at the same place at 100 %** (A-Q17): comparing up to four images (2×2) with
  locked zoom and position, to pick the sharpest of a burst. It changes the preview's layout,
  marking and stepping and needs its own measurement within V8, so it belongs to U6 (ADR 0017), as
  a later amendment. Until then V6 keeps zoom and position when stepping, so a burst can be judged
  one image at a time at 100 %.
- The loupe under the pointer in the contact sheet and the separate preview window (ADR 0017 §7.2).

## 7. Consequences

- **ADR 0006 §2** gains a note: the view may differ from these settings while an ADR 0027
  adjustment is on (besides the demosaic method, PPG, of ADR 0008 §11); the export and the
  thumbnails do not.
- **ADR 0017**: §2.6's line "Not included: temporary exposure or white balance…" and the §7.2 row
  point to this ADR; §7.2 gains a row for the 2×2 compare (§6).
- **ADR 0025**: §2.2 says that "the same settings as the view's RAW mode" are those with every
  ADR 0027 adjustment off (A-Q14); §2.9 gains the shared filter crate (A-Q7).
- **ADR 0016**: the order becomes render → (exposure, white balance in LibRaw) → downscale →
  shadows → sharpening → channel view → conversion to the monitor profile. Today the conversion
  comes right after the render, before the downscale (`native.rs`); W1 moves it.
- **`pw-viewcore`**: the view key and the job gain the adjustment state; new post steps after the
  render, each in plain, tested Rust.
- **`pwlr.cpp` / `pw-libraw`**: new parameters for exposure, white balance multipliers and presets,
  and the sampling of a grey area.
- **A new crate** for the shared filter (A-Q7).
- **Settings**: USM parameters, thresholds and colours, and the histogram scale.
- Rendering more often (fine details, exposure changes) uses more CPU while it is on; it is off by
  default, so V1–V8 are unchanged for a user who does not turn it on.
