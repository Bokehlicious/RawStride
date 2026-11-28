# ADR 0027: Display adjustments for assessment

- Status: **accepted by the owner (2026-11-18):** every question answered (§4); A-Q1, A-Q3,
  A-Q4, A-Q6, A-Q10, A-Q14, A-Q15 and A-Q17 as proposed, A-Q2 and A-Q5 changed, A-Q7, A-Q8, A-Q9,
  A-Q11, A-Q12, A-Q13 and A-Q16 with additions. Nothing is built, installed or added as a
  dependency by this ADR. Each step in §5 becomes its own task, with its own approval and commit.
  W1's questions W1-Q1–W1-Q9 answered by the owner on 2026-11-24, all as proposed (§4.1); W2's
  W2-Q1–W2-Q8 the same day, all as proposed (§4.2); W3's
  W3-Q1–W3-Q8 the same day, all as proposed (§4.3). W3's hand check on 2026-11-25 set the
  sharpening's defaults and changed A-Q8, W1-Q8, W3-Q1, W3-Q5 and W3-Q6 (§4.3, "W3's hand
  check"). W5's W5-Q1–W5-Q9 answered by the owner on 2026-11-25, all as proposed (§4.4); they
  change §3.4's range to −2 to +3 EV and make the shadows a display mode for the session. W4's
questions (§4.5) were answered on 2026-11-26 and W6's W6-Q1–W6-Q8 on 2026-11-28, all as proposed
(§4.6); W6 hand-checked and approved by the owner on 2026-11-28 with no change (§4.6).
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
  example exposure in JPEG mode, A-Q9) keeps its mark, shown as
  paused (for example dimmed); it applies again as soon as the view allows it, without a key.
- **It is off by default and is never saved** per image or in `settings.txt` (A-Q2):
  - a value taken from one image is reset when another image is shown: the **manual exposure**
    and the white balance from a **click on grey**;
  - **fine details, sharpening, the shadows (W5-Q6, owner 2026-11-25) and the channel view** are
    display modes and last for the session, for every image;
  - a choice that is worked out again for every image, or is a fixed choice, is a mode that lasts
    for the session: **auto exposure** and **the camera's brightness** (§3.4), and **auto white
    balance** and the **white balance presets** (§3.5; owner, 2026-11-18); auto exposure alone
    may be on at start, by the setting "Auto exposure at start" (owner, 2026-11-26, §4.4 "W5's
    hand check");
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

- A key (A-Q3; **U**, W3-Q5) and a button "USM" in the preview's title line above the image, after
  the RAW/JPEG control (W3-Q5, owner 2026-11-25), turn sharpening on and off. It is applied **last on the worker**, to
  the view at screen resolution after the downscale (sharpening before a reduction is mostly
  lost), and before the conversion to the monitor profile (ADR 0016).
- On luminance only, so no colour fringes are added.
- Three settings: amount, radius (px) and threshold (levels). Defaults (set in the owner's hand
  check, 2026-11-25): amount 80 %, radius 0.8 px, threshold 2.
- **At every zoom level** (A-Q8, changed by the owner on 2026-11-25): at 100 % and above too,
  since the user often judges sharpness there, and the key and the button make it a visible
  choice that is undone at once. The views above 100 % are the 100 % views drawn larger, so they
  are sharpened with them; the mark "USM" is never paused.
- Cost [T]: a separable Gaussian blur and a difference on ≈ 8 Mpix (4K) is a few milliseconds on
  several threads; it is done in the view core as plain, testable Rust (GR11). A GPU shader is
  considered only if the measurement fails V1.

### 3.4 Temporary exposure and shadows (RAW mode)

- **Exposure:** −2 to +3 EV (W5-Q2, owner 2026-11-25; first −3 to +3, below LibRaw's lowest
  `exp_shift`) in steps of 1/3 EV, with keys for up, down and zero (A-Q3). Applied in
  LibRaw (`exp_correc`, `exp_shift`, with `exp_preser` to protect highlights), so a negative
  correction can use the headroom above the camera's white point. Every view of the image is
  rendered again [T: the cost is a new `dcraw_process` per view; the unpacked data is kept in the
  session]. Reset when another image is shown (A-Q2).
- **Shadows:** a lift of the dark tones (a curve on the view, 0–100) applied after the render, **on
  the 8-bit view** (A-Q10): a table lookup per pixel. It shows whether there is detail in the
  shadows; banding in smooth dark areas is accepted for that purpose. If the hand check sees
  banding that hides detail, that step decides how the lift moves into the render (a 16-bit output
  or LibRaw's own tone curve) [T]. Today the whole chain after LibRaw is 8-bit (`output_bps = 8`).
  A display mode for the session, turned on and off with a key; the amount is a setting on the
  Settings page Preview (W5-Q6, owner 2026-11-25).
- **Auto exposure** (A-Q11): sets the exposure from U7's RAW statistics (`RawStatistics`, the
  histogram after white balance, scaled to the white level). Our own rule, not LibRaw's
  `no_auto_bright = 0`, so the result is explained by the histogram the user sees:
  - the **99.9th percentile of the brightest channel** is placed **1/3 EV below clipping** (a few
    hot pixels or a specular highlight do not decide the brightness of the whole image);
  - the result is **rounded down to 1/3 EV** and limited to **−2 to +3 EV** (W5-Q2), the steps and range
    of the manual exposure; the mark shows for example "+1⅓ EV auto", and up and down go on from
    there;
  - if **more than 0.1 %** of the pixels are already clipped in the RAW data, the result is 0 EV:
    what is clipped in the RAW cannot be brought back, and a negative value would only darken the
    rest;
  - the rule is tuned in the hand check.
- **The camera's brightness** (A-Q12): a choice to apply the camera's own brightness offset (for
  example DNG `BaselineExposure`, and the makers' equivalents where LibRaw or ExifTool gives them
  [T: which formats carry it is checked in W5]), so the RAW view can be compared with the embedded
  JPEG fairly; the file's value is applied with its sign turned (owner, 2026-11-26, §4.4 "W5's
  hand check"). ADR 0006 §2 left matching the camera JPEG's look as "a separate, later question"
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
| A-Q8 | Sharpening at 100 % and above? | Off (as proposed); **added:** the mark "USM" is shown as paused, and sharpening applies again when the user zooms out (§3.3). **Changed by the owner on 2026-11-25** (W3's hand check): sharpening applies at every zoom level, 100 % and above included, and the mark is never paused (§3.3) |
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
| W1-Q8 | Stepping at a free zoom level below 50 % (asked after T309's cold read, answered 2026-11-24 as proposed) | Today it is quick because the prefetched half-size view serves every scale ≤ 0.5; a reduced view serves only its own scale and below. So while the zoom is a free level below 0.5, the neighbours are prefetched at the current scale instead of at fit; at fit and at 50 % or more the prefetch is unchanged. **Changed by the owner on 2026-11-25** (W3's hand check, §4.3): at every free level below 100 %, 50 % or more included, the neighbours are prefetched at the current scale (ADR 0008 §12.1): below 50 % with the 100 % view, as before, and from 50 % to below 100 % with the fit view, Z's target there; at fit unchanged (T324) |
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
| W3-Q1 | Where it applies | Last on the worker, after the reduction and before the conversion to the monitor profile (§3.3, W1-Q4), to every view below 100 % made on the worker: the reduced view, the fine view, `Scaled`, and the half-size view at exactly 50 % (drawn at 1:1). In RAW and JPEG mode, for JPEG and TIFF files, and on both sides of the compare. At 100 % and above (`Zoom`, `Ring`) it is paused (A-Q8). During a zoom gesture the stand-in (the mipmapped half-size view, W1-Q3) is drawn unsharpened, and the mark counts as waiting. **Clarified by the owner 2026-11-24, after T316's cold read:** the half-size view a gesture asks for is made sharpened (it is drawn at 1:1 at 50 % when the gesture ends; an unsharpened one would be rendered twice there and make the mark flip); during the gesture the view already there is drawn, sharpened or not, and the mark shows waiting. **Changed by the owner on 2026-11-25** (W3's hand check, A-Q8): the 100 % views (`Zoom`, `Ring`) are sharpened too, so every zoom level is; nothing is paused (§3.3) |
| W3-Q2 | How it is computed | On luminance only, so no colour fringes are added: the luminance Y′ = 0.2126 R′ + 0.7152 G′ + 0.0722 B′ of the sRGB-encoded values (not linear light), blurred with a separable Gaussian whose sigma is the radius; where the difference between Y′ and the blur is below the threshold (levels, 0–255) nothing changes, elsewhere the amount times the difference is added equally to R′, G′ and B′, each clamped to 0–255. The edges repeat the edge pixels, so an image of 1×1 or 1×n works. A module of its own in `pw-viewcore`, plain Rust, no new dependency |
| W3-Q3 | Its speed | The first task times it on a screen-sized view (3840×2160 and the owner's 1920×1200). If it takes more than 10 ms on one thread, the rows are split over threads with `std::thread::scope` in the same task (as T311) |
| W3-Q4 | The cache, and what the user sees when it is turned on or off | The view's key gains the sharpening state (on or off, and the three values), so turning it on or off or changing a value makes new views. The drawn view stays until the new one is ready and is replaced in one swap (V2). The unsharpened view is kept as the memory budget allows, so turning sharpening off shows it at once. Cost accepted: turning sharpening on renders the shown view again (about 0.3 s for 61 Mpix at fit, about 1.2 s with fine details on); keeping the unsharpened pixels in memory to sharpen them in milliseconds is not done (more memory under V8, more code) |
| W3-Q5 | Key and menu | The key **U** (free in the key map), in the key map (U8) so it can be changed, and View ▸ Show Sharpening, right after Show Fine Details. It lasts for the session and is not saved (A-Q2). **Added by the owner on 2026-11-25** (W3's hand check): a button "USM" with the key map's key (for example "USM (U)") in the preview's title line above the image, right after the RAW/JPEG control, shown and disabled as that control is; it is drawn as selected while sharpening is on, and its hover text is the mark's (W3-Q6) |
| W3-Q6 | The mark, and when it goes wrong | "USM" in the row of adjustment marks (W2-Q5), after "Fine"; dimmed while paused (100 % and above). The hover text names the key and the three values (for example "Sharpening (U): amount 80%, radius 0.8 px, threshold 2."). Sharpening can fail only on inconsistent image data; such a view then fails as any render does and shows today's error line. Nothing new is shown to the user. **Changed by the owner on 2026-11-25** (W3's hand check, A-Q8): the mark is never dimmed for sharpening, which applies at every zoom level; it still counts as waiting during a gesture or while its view is made |
| W3-Q7 | The settings | A new Settings page **Preview** (W7 adds the thresholds and colours of §3.7 to it) with a section Sharpening: amount 10–300 %, radius 0.3–2.0 px, threshold 0–20 levels, and a button Restore Defaults. Defaults 80 %, 0.8 px and 2 (set in the owner's hand check, 2026-11-25; first 60 %, 0.6 px and 2). Saved in `settings.txt`; a value in the file that cannot be read or lies outside its range gives that value's default. A changed value applies when the slider is released (or the field is left), not on every frame of a drag, since each change renders the views again |
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

The CPU or the GPU (owner, 2026-11-24): §3.3 keeps sharpening on the CPU, but the choice is
checked once, after T318 and before W5 starts, since W5 and W6 put more steps on the CPU before
the conversion (§7), and a move to the GPU later would take the whole chain, not only
sharpening. The check is the owner's `latency.ps1` with sharpening on, at fit and at 50 % (the
half-size view, which is sharpened whole whatever the screen's size); if V1 or V4 fails, a GPU
step is proposed as a question of its own before W5. The 3840×2160 timings weigh less: the owner
expects a faster CPU by the time a 4K screen is used.

The measurement (reviewer, 2026-11-24, release build of T318's commit, 1920×1200): at fit,
the next image's first change 5.4 ms with sharpening on and 5.5 ms off, and at 41 % (exact 50 %
cannot be reached with the keys) 5.5 and 5.4 ms; V1 and V4 hold, so sharpening stays on the CPU.
At 51.2 % the next image takes about 155–180 ms with sharpening on and off alike: not caused by
sharpening (`backlog.md`, Technical work).

**W3's hand check (owner, 2026-11-25):** approved, with these changes:

- The defaults are amount 80 %, radius 0.8 px and threshold 2 (§3.3, W3-Q7); 60 % was too weak.
- Sharpening applies at every zoom level, 100 % and above included (A-Q8, W3-Q1, W3-Q6, §3.3).
- A button "USM" in the preview's title line above the image, after the RAW/JPEG control (W3-Q5).
- Found in the hand check: between about 51 % and 99 %, the pixels move when sharpening is
  turned on or off. Sharpening itself does not move them (the blur is symmetric). The cause
  (reviewer, 2026-11-25, from the code): the current image's ring at 100 % is always prefetched,
  and between 50 % and 100 % it satisfies the shown requirement when its region covers the
  visible one and its sharpening is the requirement's. With sharpening off the ring is drawn,
  reduced by the GPU; with it on a `Scaled` view is made, reduced with the Lanczos filter on the
  worker. **Decided by the owner on 2026-11-25:** between 50 % and 100 % only the exact `Scaled`
  view satisfies the shown requirement, as the reduced view below 50 % (W1-Q3); a denser view
  (the ring, the 100 % view) is drawn meanwhile and replaced in one swap. Every view below 100 %
  is then reduced with the Lanczos filter (§3.2), with sharpening on and off alike. **Also decided
  the same day:** a pan counts as a gesture step for that view: while the user drags between 50 %
  and 100 %, no scaled view is asked for (the ring is drawn); it is asked for once the drag has
  paused as long as a zoom gesture's pause, then one swap.
- The 51.2 % stepping time above: between 50 % and 100 % the neighbours are prefetched at fit
  and at 100 % only, not at the shown scale, so the next image waits for a full render of its
  visible region. **Decided by the owner on 2026-11-25:** there the neighbours are prefetched
  at the shown scale (`Scaled`) and at fit, as ADR 0008 §12.1 says ("the shown view and the zoom
  target": from a level between 50 % and 100 %, Z goes to fit); the owner measures it with
  `latency.ps1` afterwards.
- **The cost, accepted by the owner on 2026-11-25:** after the dwell, up to four full-resolution
  region renders on the background worker (+1, +2, the other display mode and −1), made again
  after every pan or zoom, so a step right after a pan is not quick; and the sharpened rings and
  100 % views (T321), which with the views of both sharpenings kept (W3-Q4) can take about
  800 MB of the 1 GB texture budget on a 3840×2160 screen, so turning sharpening off shows the
  kept view at once less often there. Measured after T324: stepping at 51.2 % and at one more
  level between 50 % and 100 % with sharpening off and on, a step right after a pan, and U at
  100 %.

**The hand check of T320–T324, and the measurement (owner and reviewer, 2026-11-25):** the owner
hand-checked the five and approved them. `latency.ps1` (release build of 173319d, folder
`s6\a1`, 1920×1200, defaults 80 %/0.8 px/2, 13 steps a series, labels `t324-*`): the next
image's first change, median (worst), is 5.3 (8.4) ms at fit with sharpening off and 5.4 (6.9)
ms on; 5.3 (6.7) ms off and 5.3 (6.7) ms on at the level reached by Z and three presses of
minus (51.2 %); 5.3 (7.0) ms off and 5.4 (6.5) ms on at two presses (64 %). Every step was one
change, none refined. V1 and V4 hold at every level, and the 155–180 ms at 51.2 % before T324
are gone. Two limits of the run: the title shows no zoom level, and in three setup series one
press of minus gave one change instead of two, so a press may have been swallowed and a level
may have been 64 or 80 % (inside the same band); the levels are named as the keys were pressed.
U at 100 % could not be measured: `latency.ps1` counts a change only where enough sampled grey
values differ by 6 levels or more (`Diff(…, 6)`), and sharpening at 80 %/0.8 px changes too
little for that, so only the first press (a new render, 188 ms, 1.3 s of CPU) was seen. The
owner saw by eye that the first U on a new image is a little slower and that off and on after
that are immediate (W3-Q4: the kept view). The step right after a pan was checked by the owner
by eye: a new image is sharp quickly, no remark.

**The pan between 50 % and 100 % (owner, 2026-11-25, W3's hand check):** with the Sony A1's RAW
files, dragging the image with the left mouse button stutters through the whole drag at about
51–59 %, with sharpening on and off alike; at other levels it runs better. The cause, from the
code and not profiled: the ring drawn while a drag goes on (T322) is made over the region
visible at 50 %, so its margin for panning is `1 − 0.5 / s`: about 2 % at 51 %, 15 % at 59 %,
33 % at 75 %. **Decided by the owner on 2026-11-25**, of three alternatives (A a larger ring, B a
pan view, C a known limit): **B**. Between 50 % and 100 % the ring is replaced by a **pan view**:
a scaled view at the shown scale over two times the visible region on each axis (clamped to the
image), made after the dwell in the ring's place in the plan, kept while it contains one and a
half times the visible region, not asked for during a drag, and reduced with the same filter as
the shown view, so the swap to the exact view moves nothing. It satisfies the shown requirement
while it contains the visible region. At 100 % and above and at 50 % and below the ring is as
before. Cost: about four times the pixels of a shown view's render, in the background, made again
after a pan beyond the kept region and after a zoom step; the owner re-checks the drag and the
stepping after T325 (T325).

**The check after T325, and the pan view only after a drag (owner and reviewer, 2026-11-25):**
the owner found the drag at 51–59 % now as smooth as at 60–99 %. `latency.ps1` (release build of
4659495, folder `s6\a1`, 1920×1200, defaults 80 %/0.8 px/2, 13 steps a series, labels `t325-*`,
the program restarted for each series): at 51.2 % and at 64 %, sharpening off and on, 5–6 of 13
steps showed no change within the 8 s timeout (51.2 %: 8 of 13 seen in both series; 64 %: 7 of
13), each such press with about 5 s of CPU and 300 MB more memory (the next image opened and
rendered from the start); the steps that were seen took 5.2–5.4 ms (median), and the memory grew
to about 4.7 GB a series, against 2.5 GB after T324, when all 13 were seen. V1 and V4 no longer
held between 50 % and 100 %. The cause, from the code and not profiled: at 51.2 % the pan view is
nearly the whole image (about 7500 × 4700 of 8640 × 5760 sensor pixels), one full-resolution render
of it about four times the ring's, asked for on every image after the dwell; a render under way
is not cancelled by a step, so the worker is still busy with the last image's pan view when +2
should be prefetched. **Decided by the owner on 2026-11-25** (of A, a pan view only after a drag;
B, a smaller pan view; C, a render that a step cancels): **A**. The pan view is asked for only
once the current image has been dragged (a `pan` that moved the centre, or a `set_view` that
moved only the centre, as the compare's second core is dragged) since it became current; a zoom step or the other display mode keeps that, a step to
another image or a new folder ends it. The first drag on an image draws the last drawn view
still until the drag pauses (as T322 did before the ring was resident); the drags after it run
on the pan view. Stepping without dragging pays nothing for it. At 100 % and above and at 50 %
and below the ring is as before (T326). Changed by the owner after W5-Q1's chain (2026-11-25,
§4.4, "The pan view without a drag"): it is also asked for once the view has stood still for a
while.

**The hand check of T325 and T326, and the measurement (owner and reviewer, 2026-11-25):** the
owner checked the drag after T326 and approved it (the first drag on an image still until it
pauses, the later ones smooth), and hopes for a better solution after W5, once a step can
cancel a render (backlog, "A render that a step cancels"). `latency.ps1` (release build of
aceae24, folder `s6\a1`, 1920×1200, defaults 80 %/0.8 px/2, 13 steps a series, labels `t326-*`,
the program restarted for each series): every step was seen and was one change; the next image's
first change, median (worst), is 5.1 (7.5) ms at 51.2 % with sharpening off and 5.1 (7.1) ms on,
5.2 (7.1) ms off and 5.1 (6.9) ms on at 64 %. V1 and V4 hold again between 50 % and 100 %. The
process's memory grew to 4.1–4.4 GB over a series (2.4–2.5 GB in T324's run, 4.7 GB after
T325), inside the core and texture budgets (V8); not looked into further.

The tasks: T320 (the defaults), T321 (sharpening at 100 % and above), T322 (only the exact
`Scaled` view between 50 % and 100 %, and the pan's pause), T323 (the button "USM"), T324 (the
neighbours prefetched at the shown scale and at fit between 50 % and 100 %), T325 (the pan
view), T326 (the pan view only after a drag).

### 4.4 W5's questions (answered by the owner, 2026-11-25, all as proposed)

Exposure, auto exposure, shadows and the camera's brightness (§3.4) are the third step built
(order W3, W5, W4: kept by the owner on 2026-11-25). Batch and import do not apply: they change
only how the shown image is made. The RAW histogram, the exposure statistics, the clipping
overlay and the focus marking do not change (§2.1). Each proposal below also answers the edge
cases of `docs/tasks/README.md` ("Edge cases before the tasks") where they apply.

What the code and the files say today (reviewer, 2026-11-25, code read and ExifTool 13.64):

- `pwlr.cpp` sets every LibRaw parameter once, in `pwlr_open`; a render changes only the
  demosaic, half size and the crop box. One session per image serves every view, the statistics,
  the clipping map and the focus job (`Task::Focus` renders the half-size view again).
- The statistics (`RawStatistics`: a 256-bin histogram per channel on an sRGB-like curve, and the
  clipped counts) are asked for only for the current image, and only after its view is on the
  screen (U7-Q6, U7c-Q5); the neighbours have none.
- LibRaw 0.22 (vcpkg) has `exp_correc`, `exp_shift` and `exp_preser`; by LibRaw's documentation
  `exp_shift` is usable from 0.25 to 8.0 (−2 to +3 EV) [T, not checked in the code], and
  `exp_preser` (0–1) protects the highlights only when the image is made brighter. It has a
  progress callback (`progress_cb`) whose non-zero return cancels the processing [T: how often
  it is called during a demosaic].
- The camera's brightness in the owner's files: DNG `BaselineExposure` (Ricoh GR III −0.51 EV,
  a Xiaomi phone 0), Fujifilm RAF `RawExposureBias` −0.7 (X-T30); **none** in Sony ARW (A1,
  A7R IV) or Panasonic RW2 (GM1). LibRaw gives `color.dng_levels.baseline_exposure` and
  `makernotes.fuji.ExpoMidPointShift` [T: whether this is the same value as ExifTool's
  `RawExposureBias`, and its sign].
- Free default keys (key map, U8): Ctrl+↑ and Ctrl+↓ (the arrows alone step), B, N, O, Q, T, Y and
  W in the preview; Backspace is the reset of every adjustment (W3-Q8).

| # | Question | Proposal |
|---|---|---|
| W5-Q1 | A render that a step cancels (backlog, "To be taken up before W5's questions are decided") | **A chain of its own before W5's tasks**: the shim sets LibRaw's progress callback and returns "cancel" when the session's cancel flag is set (`pwlr.cpp`, an extra review); `pw-libraw` gives a cancel handle that another thread may set; the view core sets it when a render under way is no longer wanted (a step to another image, a new exposure value, a new folder), and the cancelled render is neither shown nor counted as failed. Its first task measures, on the owner's 61 Mpix files, the time from the flag to the return for a half-size render and a full PPG render; if it is above 100 ms the owner decides whether the chain goes on. Without it, W5 still works: the plan asks only for the newest value, so at most one render with an old value finishes first (about 0.3 s at fit, 1.2 s with fine details on). Whether the pan view may then be asked for without a drag again (T326) is a question of that chain, not of W5 |
| W5-Q2 | The range and the steps | **−2 to +3 EV** in steps of 1/3 EV (§3.4 said −3 to +3; LibRaw's `exp_shift` goes no lower than 0.25), with `exp_preser` at 1.0, tuned in the hand check. Auto exposure (A-Q11) and the camera's brightness are limited to the same range, and so is the sum of a mode and the manual steps; a key press beyond a limit changes nothing and shows nothing new (the mark already shows the limit). −3 EV would need a second step after LibRaw and is not built unless the hand check asks for it |
| W5-Q3 | Where it applies | In RAW mode, to every view of the image (the half-size, reduced, fine, scaled, 100 %, ring and pan views), in LibRaw before the demosaic; the shadows after the reduction and before sharpening (§7). Paused (the mark dimmed, A-Q9) in JPEG mode and for JPEG and TIFF files. The parameters are passed with each render, not left in the session, so the focus job, the statistics and the clipping map always run on ADR 0006 §2's settings (A-Q6), and the export has nothing to undo. **The compare:** the modes (auto, camera) apply to both sides, each worked out for its own image; the manual steps apply to both sides as the same offset, so two images are compared at the same correction, and are reset when either side's image changes. **The neighbours:** prefetched with the active mode's value for their own file (the manual steps are reset by the step anyway) |
| W5-Q4 | Auto exposure and the statistics | While auto exposure is on, an image's statistics are made **when it is opened, before its first view** (the current image and the prefetched neighbours), so a step shows the auto-exposed view as one change (V2) and the next image never first appears at 0 EV. The first task times the statistics on the owner's 61 Mpix files [T]; if a step to an image that was not prefetched (a jump, a new folder) then shows its first view later than V1 allows, the owner decides between that and a first view at 0 EV replaced by the auto-exposed one (decided after T331, 2026-11-25: the step waits; "W5-Q4 after T331" below). The rule is A-Q11's, read from the histogram the user sees. **Without a value:** a file without statistics (a layout `RawStatistics` does not read) gets 0 EV and the mark says so; more than 0.1 % clipped gives 0 EV by the rule, and the hover text says why |
| W5-Q5 | The camera's brightness: which values | DNG `BaselineExposure` and the Fujifilm RAF value through LibRaw, nothing else in this step; the RAF value's sign and whether it is ExifTool's `RawExposureBias` are checked in the first task against the embedded JPEG [T]. Applied exactly (not rounded to 1/3 EV); manual steps of 1/3 EV are added on top (A-Q12). Sony ARW and Panasonic RW2 carry none: the mark says "Camera EV: none" and the view is at 0 EV (A-Q12), so for the owner's Sony files the choice does nothing visible. Read once when the file is opened (no ExifTool call), so it costs no time on a step. A value outside the range is limited to it, and the hover text gives the file's value |
| W5-Q6 | The shadows: how long they last, and the value | Not named in A-Q2. Proposed: **a display mode for the session**, as sharpening (on or off with a key, the same for every image, not saved); the amount (0–100) a setting on the Settings page Preview (section Shadows, default 50, set in the hand check), saved in `settings.txt` with the rules of W3-Q7. The curve: a smooth table over 0–255 that keeps 0 and 255, never lowers a value and lifts the darkest quarter most; its formula is the first task's, tested to be monotonic. In RAW mode only (A-Q9), on the 8-bit view (A-Q10) |
| W5-Q7 | The cache, and what the user sees while a value changes | The view's key gains the exposure (in 1/3 EV, plus the camera's value) and the shadows' amount, so a change never shows an old texture; the drawn view stays until the new one is ready and is replaced in one swap (V2). Several quick presses ask only for the newest value. After a change the shown view comes first; the ring, the 100 % view and the neighbours follow after the dwell, as after a zoom step. The views of the earlier value are kept as the memory budget allows (V8), so Shift+Backspace or a press back often shows a kept view at once. Cost accepted: every change renders the shown view again (about 0.3 s at fit for 61 Mpix, 1.2 s with fine details on) |
| W5-Q8 | Keys and menu | **Ctrl+↑ / Ctrl+↓** ±1/3 EV, **Shift+Backspace** the manual steps back to 0, **B** auto exposure on and off, **Shift+B** the camera's brightness on and off (each turns the other off, A-Q12), **O** the shadows on and off; all in the key map (U8), so they can be changed. View ▸ Exposure ▸ Brighter, Darker, Reset Exposure, Auto Exposure, Camera Brightness, and View ▸ Show Shadows after Show Sharpening; greyed where they cannot apply (JPEG mode, a JPEG or TIFF file). Backspace (W3-Q8) also turns these off and resets the steps. No button in the title line in this step (the hand check may add one, as for "USM") |
| W5-Q9 | The marks, and when it goes wrong | One exposure mark after "USM" in the row of marks (W2-Q5) giving the total: "+1⅓ EV" (manual), "+1 EV auto", "−0.5 EV camera", "Camera EV: none"; the hover text names the key and the parts (for example "Auto exposure (B): +1 EV from the histogram, plus +⅓ EV by hand."). A mark "Shadows" after it. Both dimmed while paused, and waiting while their view is made. A render that fails shows today's error line; nothing new. New glossary terms: exposure, auto exposure, camera brightness, shadows (rule 13) |

W5-Q1's chain, written 2026-11-25 and done first: T327 (a LibRaw render that can be stopped, and
its timing; `pwlr.cpp`, `pw-libraw`) → T328 (the view core stops a render no longer wanted). W5's own chain is then proposed as
three or four tasks: the shim and `pw-libraw` (the exposure per render, the camera's value at the
open, the range checked, the statistics timed; an extra review, C++), the view core (the state,
the view key, the statistics at the open while auto is on, the shadows' function), the Settings
page (the shadows' amount), and the shell (keys, menu, marks, reset, glossary). After it, the
owner's hand check and `latency.ps1` at fit with exposure and with auto exposure on.

**W5-Q1 after T327's measurement (owner, 2026-11-25):** a stop takes effect only between LibRaw's
stages: 220–350 ms from the stop to the return for a whole-image PPG render of the 50 and 61 Mpix
ARW files, 0.55–1.3 s for the X-Trans RAF, 35–90 ms for a half-size render (i7-9700K, T327's log).
A stop while LibRaw works always makes LibRaw free the unpacked RAW data, so the session must be
opened again (about 0.3 s for 61 Mpix). The chain goes on, and **T328 stops a render only when its
image is no longer wanted** (a step to another image, a new folder); a new exposure value of the
same image does not stop the render under way, which finishes first (stop plus reopen would cost
as much or more). LibRaw's `setCancelFlag()`, which might stop sooner, waits in `backlog.md`.
At T328's cold read the owner added (2026-11-25): a change of display mode (RAW and the embedded
JPEG of the same file) is the same image and stops nothing. At T328's review the owner decided
(2026-11-25): a render stopped because its image was left has its stop cleared when the image is
wanted again before the render ended (a step away and back); if LibRaw has already looked at the
stop, the render is cancelled all the same. A release for a metadata write stops the render of
that file's session, also of the current image, so the write does not wait for it; a render that
comes back whole with its stop set is not used, and the view is made again after the write
(accepted). A cancelled render adds no time to the render timings, also in an older folder.

**The pan view without a drag (owner, 2026-11-25, after T328's hand check):** of A (keep the pan
view only after a drag, T326), B (ask for it without a drag again, as after T325, and rely on
T328's stop) and C (without a drag, but only once the view has stood still for a while, and last
in the plan), the owner chose **C**. Between 50 % and 100 % the pan view of the current image is
asked for when the image has been dragged (as T326 made it, in the same place in the plan), or,
without a drag, once its view has **stood still for 1 s**: no step to another image, no new
folder or file list, no zoom step and no `set_view` that changed anything for that long. Without
a drag it comes **after everything else the plan asks for** (the backward neighbour included), so
quick stepping never starts it and the neighbours are prefetched first. A step to another image
while it renders stops it (T328); that costs the worker the stop (220–350 ms on a whole-image ARW
render) and the left image's session, accepted, since the next image is already prefetched and
only the prefetch after it waits. The 1 s is a setting of the core (`Config`), not of the user,
and is tuned in the hand check. Edge cases: batch and import do not apply (only the view changes);
the compare's second core follows the same rule (its known limit, a new held image counted as
dragged, stays); a JPEG or TIFF file and JPEG mode follow the same rule as RAW, as the pan view
does today; what the user sees: nothing new, the first drag on an image that has stood still is
smooth, on one that has not it is still as after T326. After the task: the owner's hand check
(the first drag after a pause, quick stepping) and `latency.ps1` at 51.2 % and 64 % with the
clipping marks off and on (T329).

**T329's hand check (owner, 2026-11-25):** the first drag after the view has stood still is
smooth and quick stepping is unchanged, for RAW files in RAW mode. `latency.ps1` (the reviewer,
2026-11-25, `s6\a1`, 13 steps per series, RawStride restarted per series): every step seen; at
51.2 % a median of 4.9 ms with the clipping marks off and 5.3 ms with them on, at 64 % 4.9 ms and
5.3 ms (as after T326 and T328); memory up to about 4.6 GB per series, as after T326. The 1 s
stays. Found in the hand check: TIFF files in RAW mode stutter badly when dragged, also after the
pause (`backlog.md`).

**W5-Q5 after T331 (owner, 2026-11-25, during the task):** LibRaw 0.22.3 never sets
`LIBRAW_DNGFM_BASELINEEXPOSURE` in `dng_levels.parsedfields`, so a DNG's `BaselineExposure` is
taken when LibRaw's value differs from its "not set" value −999; the Fujifilm value is read the
same way (a RAF without the tag gives none). On the owner's X-T30 files LibRaw gives **−0.72**,
ExifTool's unrounded `RawExposureBias` (`exiftool -n`; ExifTool shows −0.7), with the same sign;
it is kept as LibRaw has it. Whether that sign is the one to add to the view is still the owner's
hand check after T335. The owner's DNG files give −0.5062 (Ricoh GR III) and 0 (Xiaomi).

**T331's timings for W5-Q4 (2026-11-25, i7-9700K, median of 5, `--release`):** a step to an
image that was not prefetched, with auto exposure on (open, statistics and a half-size render),
takes 625–694 ms on the A7R IV's ARW (8660×5784), 769–794 ms on the A1's (9568×6376) and
506–567 ms on the X-T30's RAF; the statistics alone 169–218 ms on the ARW files and 86–91 ms on
the RAF. An exposed half-size render costs about 30 % more than one at 0 EV (+65–85 ms), an
exposed whole-image PPG render the same as one at 0 EV.

**W5-Q4 after T331 (owner, 2026-11-25):** a step to an image that was not prefetched, with auto
exposure on, **waits for its statistics** before its first view, as W5-Q4 proposed; no first view
at 0 EV replaced by the auto-exposed one. Reasons: it happens only on a jump, a new folder or the
first image (stepping to a neighbour finds it prefetched with its statistics); such a step in RAW
mode already takes 450–560 ms without the statistics, which add 170–220 ms (V1's 30 ms is for the
embedded preview, not the RAW view); and a brightness change after the first view would be two
changes (V2) at the moment the exposure is judged. If the wait is felt in the hand check after
T335, the statistics could later be made in a session of their own beside the render (not now).

**W5's hand check (owner, 2026-11-26, the build of T335):** approved, with three changes that
follow as tasks (T338 → T339 → T340):

- *The manual exposure, auto exposure, the shadows, the paused marks, the menu and the compare*
  work as written. The highlights at +2 to +3 EV look fairly protected but still burn out
  visibly; `exp_preser` is already at its highest (1.0), so this is LibRaw's limit and is
  accepted. The shadows' default amount 50 stays; no banding that hides detail (A-Q10 holds,
  the lift stays on the 8-bit view); the auto rule and W5-Q4's wait approved as they are. Every
  press of Ctrl+↑ is seen, at any speed.
- *The camera's brightness has the wrong sign* (W5-Q5): with Shift+B the view of an X-T30 RAF
  (−0.72) and of a Ricoh GR III DNG (−0.51) is darker than the RAW view at 0 EV, while the
  embedded JPEG is brighter than both. The file's value says how far the camera kept the RAW
  data below its own rendering, which its JPEG lifts again; so the view applies the file's
  value **with the sign turned** (+0.72, +0.51), for both formats, and the hover text names it
  as the camera's brightness rather than the file's value (**T338**). The match with the JPEG
  stays approximate, since the camera's JPEG has its own tone curve. An offset worked out from
  the embedded JPEG's histogram, which would also serve ARW and RW2, is in `backlog.md`.
- *Buttons for the exposure* (W5-Q8 left this to the hand check): three buttons **"−", "A",
  "+"** after "USM" in the status bar: − and + step the exposure by 1/3 EV (Ctrl+↓, Ctrl+↑), A
  turns auto exposure on and off (B) and is drawn selected while it is on. No number in the
  buttons (the mark shows the total); the camera's brightness and Reset Exposure get no button.
  Greyed where the menu items are (**T339**).
- *Auto exposure at start* (amends §2.1's "off by default and never saved" for auto exposure
  alone): the owner finds the RAW view at 0 EV often clearly darker than the camera's JPEG. A
  setting on the Settings page Preview, **"Auto exposure at start"**, saved in `settings.txt`,
  **on by default**: auto exposure is on when RawStride starts, and B (or A, or Backspace) turns
  it off for the session as before (**T340**). With it on, the first view of a jump waits for
  its statistics (W5-Q4) and the overlays stay unexposed (`known-limits.md`), both accepted.
  The view of ADR 0006 §2 stays one key away and is what the export renders (A-Q14).
- *The auto rule's target* (1/3 EV below clipping) may be raised a little later; not now
  (`backlog.md`).

`latency.ps1` (the reviewer, 2026-11-26, `s6\a1`, at fit, 13 steps per series, RawStride
restarted per series, the mode pressed and the prefetch left to finish first): every step seen;
the next image's first change, median (max), 5.5 ms (10.5) with every adjustment off, 5.8 ms
(7.4) with auto exposure, 5.5 ms (7.2) with the shadows; with fine details on 5.4 ms (10.5),
6.0 ms (15.1) and 5.3 ms (7.0). V1 and V4 met. Memory after 13 steps 2.4–2.8 GB. Turning auto
exposure on: the view changes after 56 ms and is whole after 406 ms. Ctrl+↑ on one image at fit:
42 ms to the first change and 280–295 ms to the exposed view; the second press of each series
showed no change in the measured area although the owner sees it by hand (taken as the
measurement's, not the program's). The camera's brightness was measured only on files without a
value (the folder's order is by capture date) and is measured again after T338.

### 4.5 W4's questions (answered by the owner, 2026-11-26, all as proposed)

The channel view (§3.6) is the fourth adjustment built (after W5, by the order of §5). Batch and
import do not apply: it changes only how the shown image is made, and it has no value that a file
can lack. The RAW histogram, the exposure statistics, the clipping overlay and the focus marking do
not change (§2.1).

What the code says today (reviewer, 2026-11-26): `render_view` (`core.rs`) renders, reduces, lifts
the shadows, sharpens and then converts to the display (`Backend::to_display`); the view's look
(`Look` in `state.rs`: sharpening, exposure, shadows) is part of every view's identity in the cache.
A TIFF is converted to the display's profile once when it is opened (W1-Q5), so its
`to_display` passes it on unchanged. Free single keys in the preview: N, Q, T, W and Y.

| # | Question | Answer |
|---|---|---|
| W4-Q1 | Where it applies | Last on the worker, after the sharpening and before the conversion to the display (§7), to every kind of view (half size, reduced, fine, scaled, 100 %, ring and pan), at every zoom level. In RAW and JPEG mode, for JPEG and TIFF files (§2.2), and on both sides of the compare. The histogram, the clipping overlay and the focus marking do not change (§2.1) |
| W4-Q2 | How it is computed | Red, green or blue: that channel's value v shown as grey (v, v, v). Luminance: Y′ = 0.2126 R′ + 0.7152 G′ + 0.0722 B′ of the encoded values, rounded to the nearest level and clamped to 0–255, as the sharpening's luminance (W3-Q2), shown as grey. Colour leaves the view as it is. A module of its own in `pw-viewcore` (`channel.rs`), plain Rust, no new dependency |
| W4-Q3 | TIFF | A TIFF is already in the display's profile when the channel is taken out (W1-Q5), so its channel is the display's red, not the file's. Accepted; an item in `known-limits.md` |
| W4-Q4 | The cache, and what the user sees on a change | The channel is part of the view's look and key, as sharpening (W3-Q4): every press makes the shown view again (about 0.3 s at fit for 61 Mpix, 1.2 s with fine details on). The drawn view stays until the new one is ready and is replaced in one swap (V2); views of the other channels are kept as the memory budget allows (V8). Taking the channel out of the finished view in a few milliseconds (in the display's colours, with more memory) is not done |
| W4-Q5 | Keys and menu | **N** steps forward (colour → red → green → blue → luminance → colour) and **Shift+N** back, both in the key map (U8), so they can be changed. A submenu View ▸ Channel after Show Shadows and the submenu Exposure (which follows it), with Next Channel (N), Previous Channel (Shift+N) and the five channels Color, Red, Green, Blue and Luminance as radio items (the program's texts in American spelling, glossary G5; the hover texts say "gray"; noted at T347's cold read). It lasts for the session and is not saved (A-Q2). Backspace (W3-Q8) sets it back to colour. No button in the title line in this step (the hand check may add one) |
| W4-Q6 | The mark, and when it goes wrong | "R", "G", "B" or "Lum" last in the row of adjustment marks (W2-Q5); none for colour, except "RGB" while a view of another channel is still drawn after a return to colour (§2.1: what is shown is always marked; reviewer, 2026-11-26, when T347 was written). The hover text names the key and the channel (for example "Channel view (N): the red channel as gray."); the mark counts as waiting while its view is made. It is never paused (§2.2: it applies in both modes). It fails only on inconsistent image data, and then as any render does, with today's error line. New glossary term: channel view |
| W4-Q7 | Its speed | The first task times the function on 3840×2160 and 1920×1200 in a release build; if the median is above 10 ms on one thread, the rows are split over threads with `std::thread::scope` in the same task (as W3-Q3) |

W4 is written as the chain T346 (the channel function, its timing and the channel in the view
core, `pw-viewcore`) → T347 (the shell: the keys, the menu, the mark, the reset, the texts,
`pw-loupe`). After it, the owner's hand check and `latency.ps1` at fit with a channel shown.

### 4.6 W6's questions (answered by the owner, 2026-11-28, all as proposed)

White balance (§3.5) is the fifth adjustment built (order W3, W5, W4, W6). Batch and import do
not apply: it changes only how the shown image is made. The RAW histogram, the exposure
statistics, the clipping overlay and the focus marking do not change (§2.1). Each proposal below
also answers the edge cases of `docs/tasks/README.md` ("Edge cases before the tasks") where they
apply.

What the code and the files say today (reviewer, 2026-11-28, code read and ExifTool 13.64):

- `pwlr_open` sets `use_camera_wb = 1` and `use_auto_wb = 0` once; `pwlr_render` passes only the
  demosaic, half size, the crop box and (since T331) the exposure. The statistics shim already
  works out the multipliers itself (the camera's `cam_mul`, else `pre_mul`, scaled so the smallest
  is 1), as the render uses them.
- The view's look (`Look` in `state.rs`: sharpening, exposure, shadows, channel) is part of every
  view's identity in the cache; the white balance joins it as W5 and W4 did.
- In the preview the mouse is taken: double-click is Z, drag pans, the wheel zooms (`main.rs`).
  Free single keys: Q, T, W and Y (B, N, O, U and D are taken); Backspace resets every adjustment
  (W3-Q8).
- The presets in the owner's files (ExifTool; which of them LibRaw's `WB_Coeffs` gives is checked
  in the first task [T]): Sony ARW (A1, A7R IV) 15 named levels (Daylight, Cloudy, Shade,
  Tungsten, Flash, Fluorescent and its three variants, and 2500, 3200, 4500, 6000, 8500 K) besides
  the "auto" levels; Panasonic RW2 (GM1) 7 named entries (Fine Weather, Cloudy, Shade, Tungsten,
  Flash, D55, ISO Studio Tungsten); Fujifilm RAF (X-T30) only "Standard" and "Auto" levels; DNG
  (Ricoh GR III, a Xiaomi phone) only `AsShotNeutral`, no presets. So for the owner's DNG and RAF
  files the choice of presets does little, and "Daylight (LibRaw)" is the only fixed reference.

| # | Question | Proposal |
|---|---|---|
| W6-Q1 | Auto white balance: the rule | **Our own rule**, as A-Q11 for exposure, so the result is explained and the same in every view: the multipliers are worked out **once per image, over the whole RAW data**, as the mean of each colour's photosites (black level taken off) **leaving out photosites at the clipping level and in the deep shadows** (the limits of the clipping map, U7), so that G = 1 after the scaling: `mul(c) = mean(G) / mean(c)`, then scaled so the smallest is 1 as today. This is the grey-world rule: a picture dominated by one colour (a green forest, a sunset) gets a cast of the other kind; accepted, since click on grey and the presets are the answers to it, and the hover text says what the rule is. LibRaw's `use_auto_wb` is not used (it measures the area it renders, so a crop box would give every region its own balance, §3.5). One pass in the shim like the statistics (`pwlr.cpp`, an extra review); the first task times it on the owner's 61 Mpix files [T] and says whether it can share the statistics' pass. Passed to every view as `user_mul` (W6-Q3) |
| W6-Q2 | Click on grey | A key, **Shift+W**, and the item View ▸ White Balance ▸ Pick Grey start a **pick mode**: the pointer becomes a cross, and the **next click** on the image sets the balance and ends the mode; Esc, another key that changes the view, or a step to another image ends it without a change. Outside the mode the mouse is as today (double-click Z, drag pans, wheel zooms), so nothing existing changes. The sample is a square of **15 × 15 pixels of the shown view**, mapped to the RAW data by the clipping overlay's mapping (U7, orientation included) and grown, where it is smaller, to at least **12 × 12 photosites** (so every colour of a 6 × 6 X-Trans pattern is in it) and cut at the image's edge; the mean of each colour (black level off, clipped photosites and deep shadows left out) gives `mul(c) = mean(G) / mean(c)` as in W6-Q1. **Edge cases:** if fewer than half of the photosites are usable, or one colour has none (a clipped or very dark spot), nothing changes and the status line says "That area is too bright or too dark to measure grey." (the program's texts in American spelling, glossary G5); a click outside the image changes nothing and ends the mode; a file whose layout the shim cannot read (as `RawStatistics`) greys the item. A new function in `pw-libraw` returns the means under a RAW rectangle; it is the first task's |
| W6-Q3 | Where it applies, and the parameters | In RAW mode, to every view of the image (half size, reduced, fine, scaled, 100 %, ring and pan), in LibRaw before the demosaic, as `user_mul` **passed with each render and cleared after it** (as the exposure, W5-Q3), so the focus job, the statistics and the clipping map always run on ADR 0006 §2's settings (A-Q6) and the export has nothing to undo. The multipliers are scaled so the smallest is 1, so brightness stays that of the camera's balance. `user_mul` taking the place of `use_camera_wb` is checked in the first task [T]. Paused (the mark dimmed, A-Q9) in JPEG mode and for JPEG and TIFF files. **The compare:** auto and the presets apply to both sides, each worked out or looked up for its own image; a click on grey belongs to the image it was taken on and is reset when that side's image changes (a value from one image is wrong for another). **The neighbours:** prefetched with the active mode for their own file (auto worked out, the preset looked up); a click on grey is reset by the step, so they are prefetched as shot |
| W6-Q4 | The presets | **From the file:** what LibRaw gives in `WB_Coeffs` (with the colour temperatures of `WBCT_Coeffs`), read **once when the file is opened**, no ExifTool call (as the camera's brightness, W5-Q5), named as LibRaw names them ("Daylight", "Cloudy", "Shade", "Tungsten", "Fluorescent", "Flash", and the temperatures as "4500 K"). **"Daylight (LibRaw)"** for every RAW file: LibRaw's daylight multipliers from its colour matrix; whether `pre_mul` read before a render is those multipliers, and not the camera's balance, is checked in the first task [T], and if it is not, they are worked out from the matrix by the D65 white point in the shim. When the file's own table has "Daylight" too, both are shown with their different names (§3.5). **The menu** View ▸ White Balance after the Exposure submenu: As Shot (the default), Auto (W), Daylight (LibRaw), Pick Grey (Shift+W), then a separator and **the presets of the current image** (the list is the image's; the choice is kept by name). **Edge cases:** a chosen preset is looked up by name in the next image; one that lacks it is shown as shot and the mark says "WB Shade: none" (as "Camera EV: none"); a file with no presets shows only the four fixed items; a preset whose multipliers are zero or not finite is left out of the list; a file with no `cam_mul` and no `pre_mul` is shown as shot and only auto and click on grey (which need neither) work. There is **no key per preset** and no cycling key; the keys are W (auto on and off) and Shift+W (pick grey), both in the key map (U8) |
| W6-Q5 | What the other measurements see | As W5 and W4: unchanged (§2.1). The RAW histogram, the exposure statistics, the clipping overlay and the focus marking stay on the camera's balance, so with a balance changed the overlay can mark a channel clipped that the view no longer shows clipped, or the other way round. **Auto exposure (A-Q11) is not worked out again** when the balance changes: its value comes from the unadjusted histogram, and a balance that lifts one colour can move the brightness a little. Both are accepted and written into `known-limits.md` with the exposure's earlier items; the hover text of the mark says that the measurements are as shot |
| W6-Q6 | How long it lasts, and a setting | Auto, the presets and "Daylight (LibRaw)" are modes for the session (§2.1), the same for every image, not saved; a click on grey is reset when another image is shown (A-Q2). Backspace (W3-Q8) sets the balance back to as shot, and View ▸ White Balance ▸ As Shot does the same alone (the exposure's Shift+Backspace has no counterpart). **No setting "Auto white balance at start"**: the camera's balance is mostly right, and the fair view of ADR 0006 §2 stays the start; the owner may add it after the hand check, as for the exposure (T340). **No buttons** in the status bar in this step (the hand check may add them, as for the exposure) |
| W6-Q7 | The cache, and what the user sees on a change | The balance is part of the view's look and key (as W5-Q7): a change never shows an old texture; the drawn view stays until the new one is ready and is replaced in one swap (V2); several quick changes ask only for the newest; the ring, the 100 % view and the neighbours follow after the dwell. Every change renders the shown view again (about 0.3 s at fit for 61 Mpix, 1.2 s with fine details on); the views of the earlier balance are kept as the memory budget allows (V8). With auto on, the multipliers of an image that was not prefetched are made **when it is opened, before its first view** (as auto exposure, W5-Q4), so a step shows the balanced view as one change (V2); the first task's timing says what a jump then costs, and the owner decides if it is too much |
| W6-Q8 | The marks, and when it goes wrong | One mark after the exposure's in the row of marks (W2-Q5): "WB auto", "WB grey", "WB Shade", "WB Daylight (LibRaw)"; none for as shot; "WB Shade: none" when the file lacks the preset (W6-Q4); the hover text names the key and the multipliers (for example "Auto white balance (W): red ×1.9, blue ×1.4, worked out over the whole image; the histogram and overlays are as shot."). Dimmed while paused, waiting while its view is made. Auto on an image with too few usable photosites (all clipped or all dark) gives as shot and the mark "WB auto: none". A render that fails shows today's error line. New glossary terms: white balance, auto white balance, click on grey, white balance preset (rule 13) |

W6 is written as a chain of six tasks (2026-11-28, after the cold reads and the chain review):
T348 (the shim and `pw-libraw`: `user_mul` per render with `use_camera_wb` off, the means under a
RAW rectangle, the presets and the daylight multipliers read at the open, each timed and checked
against the owner's four formats; an extra review, C++), T349 (the view core's render path and the
backend's three questions), T350 (the view core's state: the mode, the info, the auto balance made
before the first view), T351 (the shell: the key W, the menu, the mark), T352 (the view core's
click on gray) and T353 (the shell: Pick Gray, the pick mode and its click). After it, the
owner's hand check and `latency.ps1` at fit with auto white balance on.

**Decided while the tasks were written (reviewer, 2026-11-28, for the owner's notice):**
the program's texts use American spelling (glossary G5), so the menu item is "Pick Gray" and the
mark "WB gray" (the questions above wrote "grey"); the sample square is **moved** inside the
image when it is near the edge, not cut at it (W6-Q2 said "cut"), so that every colour of an
X-Trans pattern stays in it; the means for auto and for the click judge whole cells (2 × 2, or
3 × 3 for X-Trans) as the clipping map does, and leave out a cell with a clipped photosite or one
that is all deep shadow, so that a dim or bright area does not skew a colour's mean; a balance is
passed to LibRaw with `use_camera_wb` off (LibRaw otherwise puts the camera's own multipliers
over a `user_mul`).

**Where the menu is reached (owner, 2026-11-28, at T351):** the preview has no menu bar (ADR 0017
U3-Q1), so View ▸ White Balance alone could not be used where the balance acts, and As Shot,
Daylight (LibRaw) and the file's presets have no key. The submenu **White Balance** (the same
items, the presets and, with T353, Pick Gray) is therefore also in the image's **right-click menu,
in the preview only** (the compare's two sides too), after the template submenu and before Delete
to Recycle Bin, each group set off by a separator; the contact sheet's right-click menu is
unchanged (ADR 0019 §2.9, E-Q17). U3-Q1 stays as it is. The View menu keeps the submenu, as it
keeps Exposure and Channel. Exposure and Channel stay out of the right-click menu for now (they
have keys for every item); whether they join it is taken up at W6's hand check (backlog).

**Cost with everything off (owner, 2026-11-28, asked before approving):** the only thing W6 does
with every adjustment off is read the file's presets when it is opened (W6-Q4). The first task
therefore reports the time of that read itself (the shim times it), and the reviewer records
`latency.ps1` with every adjustment off at the head before T348 starts and again after T353; if a
step is measurably slower, the presets are read only when the menu is opened. No
global switch for all adjustments is built: every adjustment is off by default and Backspace
turns them off (§2.1), and the measurement is what guards the cost.

**The daylight multipliers, how they are chosen and checked (owner, 2026-11-28, during T348):**
T348's first run found `pre_mul` read at the open equal to the D65 matrix's multipliers within
0.5 % on the owner's ARW, RAF and RW2 files, and on a Sony file (TN005701.ARW) a B/G 15.85 % below
the camera's "Daylight" preset: the preset is for about D55 and LibRaw's multipliers for D65,
whose bluer light needs less blue gain, so the two differ by that much by design. No DNG got a
source, because the rule had no case for a `pre_mul` that differs from both `cam_mul` and the
matrix. Decided: **the rule** is `pre_mul` when it equals the matrix's multipliers within 0.5 %
(as ratios to green), else the matrix's multipliers when they are usable, else none. **The check**
on the owner's files: the daylight multipliers exist (from `pre_mul` or the matrix); from
`pre_mul` they equal the matrix's within 0.5 %; they differ from the camera's own balance
(`cam_mul`) by more than 0.5 % in R/G or B/G (not a copy of it); and they are within 30 % of the
file's "Daylight" preset where it has one, a guard against a wrong value only, not a measure of
the D55–D65 difference. The 15 % of the cold read is dropped.

**T348's timings for W6-Q1, W6-Q4 and W6-Q7 (2026-11-28, i7-9700K, Windows build 26300, median
of 5, `--release`):** the means of the whole image take 269–275 ms on the A7R IV's ARW
(8660×5784), 328–332 ms on the A1's (9568×6376) and 128–131 ms on the X-T30's RAF, against the
statistics' 169–180, 205–215 and 85–89 ms. A step to an image that was not prefetched, with auto
white balance on (open, means and a balanced half-size render), takes 717–738, 881–889 and
540–565 ms; with auto exposure on as well, 890–908, 1093–1104 and 636–660 ms. **The means cannot
share the statistics' pass** (W6-Q1): the statistics bin each photosite and the means judge whole
cells, so they are two passes. A balanced half-size render costs about 15–20 % more than one as
shot (+35–60 ms); a balanced whole-image PPG render no more than one as shot. The presets and the
daylight read at the open take 9–26 µs (W6-Q4: negligible, so they stay read at the open).

**W6-Q7 after T348 (owner, 2026-11-28):** T348 approved as it is; the means are made faster in a
task of their own, **T354**, before T349. The independent review found the cost in the inner loop
(a remainder by the colour pattern's width for every photosite, which the statistics avoid with a
running counter); T354 changes only how fast the means are made, not what they give. A step to an image
that was not prefetched, with auto white balance on, then waits for the means before its first
view, as W6-Q7 proposed. **After T354 (2026-11-28, same machine and runs):** the means take 173–175 ms
on the A7R IV's ARW, 211–213 ms on the A1's and 72–77 ms on the RAF (about the statistics' time), and
the step with auto white balance 654–667, 806–815 and 510–519 ms, with the same results as T348's.

**W6's hand check (owner, 2026-11-28, the build of T353):** approved as built, with no change:
auto (W), the presets, "Daylight (LibRaw)", Pick Gray (Shift+W) and its edge cases, Backspace,
the paused mark, the compare and the unchanged measurements were checked. So W6-Q6 stands (no
buttons and no setting "Auto white balance at start"), and the wait of W6-Q7 for an image that
was not prefetched is accepted. Seen, not a fault: the View menu's White Balance submenu does
little, since the preview has no menu bar and the contact sheet does not use the balance; it is
taken up with Exposure and Channel in the right-click menu (backlog), which was not decided at
the hand check.

`latency.ps1` (owner, 2026-11-28, release build of 5726305, `s6\a1`, at fit, 13 steps × 2.5 s,
steps 2–13 compared, as for the baseline before T348): every step seen and settled at its first
change. The next image's first change, median (range), **6.05 ms (5.5–8.8) with every adjustment
off**, the same as the baseline's 6.05 ms (5.2–9.2), so reading the presets at the open costs
nothing measurable and they stay read there (§4.6, "Cost with everything off"); **6.05 ms
(5.9–6.4) with auto white balance on**. Memory after 13 steps 2.4–2.5 GB. V1 and V4 met. A series
on the key W showed only its first press (61.5 ms to the first change, 114 ms settled) and no
change or work in the program after it; taken as the measurement's, not the program's (the owner
saw W work by hand), as for W5's Ctrl+↑, and not measured again (owner). **W6 is done.**

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
