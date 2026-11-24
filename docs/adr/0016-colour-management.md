# ADR 0016: Colour management in RawStride (source profile → monitor profile)

- Status: **accepted by the owner (2026-11-11, step C0).** The scope (§3), F1–F6 as behaviour
  requirements (§7), every decision in §11, `moxcms` =0.8.1 in `pw-colour` and the `windows-sys`
  =0.61.2 features in `pw-loupe` (§6) are accepted. Nothing is built or installed by this ADR. Each
  later step in §9 is its own work part, with its own approval and commit.
- **Implemented (2026-11-11):** steps C1–C4 done (agent tasks T68–T72), the status-line note for an
  unusable embedded profile (§5) in T73, C5 accepted by the owner on 2026-11-11; C6 is this
  documentation update (T74).
- Date: 2026-11-11
- Related:
  - ADR 0015 §3.2 and §9.1 (ICC left out of milestone 6; ProPhoto TIFFs look dull);
  - ADR 0006 (LibRaw renders 8-bit sRGB); ADR 0009 §6 (eframe/egui with `glow`);
  - ADR 0007 (IPTC, milestone 7): this ADR proposes to come **before** it (Q1);
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) GR7 (move to another screen), GR8 (open owner
    question before the switch: colour management), GR9 (honest source), GR10 (dependencies), GR11.

## 1. Context

### 1.1 What RawStride does today

- Every image ends as 8-bit RGB in an egui texture, and those values reach the screen **unchanged**:
  `egui_glow` 0.37.0 uploads `RGBA8` (not `SRGB8_ALPHA8`, `srgb_textures = false`), disables
  `FRAMEBUFFER_SRGB`, and its fragment shader passes the texture value through (with optional
  dithering of ±½ step). **Confirmed on screen (2026-11-11, reviewer):** a generated 8-bit TIFF
  with eight flat patches (pure R, G, B, white, grey 128, (200,100,50), (30,160,220), (10,20,30))
  shown in RawStride (release build) at fit and read back with a screenshot of the CX240: every
  patch had exactly its file values, with no dithering noise.
- So RawStride assumes that **the image is sRGB and that the screen is sRGB**. Both are often wrong:
  - **TIFF:** the owner's 9 test files are Lightroom 16-bit **ProPhoto RGB** (ADR 0015 §9.1).
    `pw-tiff` reduces 16 → 8 bit (`u16_to_u8`) and the ICC profile (tag 34675) is ignored, so they
    look dull.
  - **JPEG:** camera JPEGs and embedded previews are mostly sRGB; a camera set to Adobe RGB marks it
    in Exif (ColorSpace = 0xFFFF and InteropIndex `R03`), and edited JPEGs may carry an ICC profile
    in APP2. Neither is read.
  - **RAW:** LibRaw renders 8-bit sRGB (ADR 0006), so the source is known.
  - **The screen:** see §1.2. Sending sRGB values straight to a wide-gamut screen makes them
    **oversaturated**.

### 1.2 The owner's screen (investigated 2026-11-11)

- **Eizo ColorEdge CX240** (24", 1920 × 1200), DisplayPort, hardware calibrated with ColorNavigator
  7, colour mode **7 "CAL (DP)"** (ADV), target **"105cd 6900K 2,20"**: 105 cd/m², 6 900 K, gamma
  2.20, gamut **Native**, gamut clipping off, black level minimum (result 0.13 cd/m², contrast
  756:1), last adjusted 2026-10-27 (ColorNavigator, owner's screenshot 2026-11-11).
- Windows associates **`CX240(…)08CAL (DP).icc`** with it (per-user association,
  `HKCU\…\ICM\ProfileAssociations\Display\…\0003`). Written by ColorNavigator on 2026-11-11 10:06.
  "08CAL" is only in ColorNavigator's file name; the menu has no such mode, so RawStride must find
  the profile through Windows' association, never by its name.
- **Profile contents** (parsed from the file):

  | Item | Value |
  |---|---|
  | Type | ICC v4.2, display class (`mntr`), **matrix/TRC** (`rXYZ`/`gXYZ`/`bXYZ` + `rTRC`/`gTRC`/`bTRC`), no LUT tags |
  | Tone curve | 1024-point tables, **gamma 2.2** (2.200 at 25 / 50 / 75 %) on all channels |
  | `vcgt` | Present but **identity** (the calibration is in the monitor's own LUT, not the graphics card) |
  | Luminance | 104.9 cd/m² |
  | White (measured) | xy 0.3073, 0.3231, about 6 850 K (D65 is 0.3127, 0.3290) |
  | Primaries (measured, not adapted) | R 0.6835, 0.3064 · G 0.1956, 0.6997 · B 0.1465, 0.0489 |

- The profile matches ColorNavigator's result: the primaries are identical to four decimals, and the
  white is 0.3073, 0.3231 against ColorNavigator's 0.3070, 0.3230 (6 887 K).
- **So the CAL target is the panel's native gamut**, not an emulated sRGB or Adobe RGB:

  | Space | Covered by the CX240 target (xy area) | Size of the target relative to it |
  |---|---|---|
  | sRGB | 100 % | 1.50 × |
  | Adobe RGB | 97.9 % | 1.11 × |
  | Display P3 | 92.1 % | 1.11 × |
  | ProPhoto RGB | far from it: ProPhoto colours outside the panel must be clipped | |

- **Consequence:** an sRGB image in RawStride is shown with about 1.5 × the saturation it should
  have; a ProPhoto TIFF is shown dull. Lightroom and Photoshop show both correctly because they
  convert to this profile. The profile is a simple matrix/TRC profile, so the conversion is cheap.
- **Windows:** HDR ("Use HDR") is off and Windows 11 automatic colour management is supported but
  not enabled for this screen (no `AutoColorManagementEnabled` value under `MonitorDataStore`). If it
  were enabled, Windows would itself map sRGB output to the panel and RawStride must not convert a
  second time (Q7).

## 2. Decision

RawStride converts every image it shows **from its source profile to the monitor profile**,
on the worker threads, before the pixels become a texture. One screen, one profile, read at start.

## 3. Scope

### 3.1 In scope

| Area | Content |
|---|---|
| Source profile | TIFF: embedded ICC (tag 34675). JPEG file and embedded preview: ICC in APP2; else the Exif Adobe RGB marker; else sRGB. RAW rendering: sRGB (LibRaw output) |
| Destination | The ICC profile Windows associates with the screen RawStride's window is on at start. None or unreadable → sRGB (= no conversion, today's behaviour) |
| Conversion | Relative colorimetric, matrix/TRC and LUT profiles, on the loupe workers and the thumbnail worker. 16-bit TIFF is converted **before** the reduction to 8 bit |
| Display | The info/help panel names the monitor profile ("Skärmprofil: …" or "ingen – visas som sRGB"); an image whose embedded profile cannot be used says so in the status line (GR9) |
| Errors | A damaged or unsupported source profile falls back to sRGB with a note, never an error screen (G3) |

### 3.2 Out of scope

- **Moving the window to another screen** with another profile (GR7). Read at start only; a later
  step listens to the screen change and clears the caches (Q4).
- A **new calibration** while RawStride runs: picked up at the next start (Q4).
- UI colours (labels, frames, text): stay sRGB values, slightly oversaturated on this screen.
- LibRaw in a wide-gamut output space (Adobe/ProPhoto at 16 bit): today's sRGB output already fits
  inside the panel; a wider RAW rendering is its own later decision (Q8).
- Soft proofing, rendering intents other than relative colorimetric, black-point compensation (Q6).
- CMYK TIFF stays the naive conversion of ADR 0015 (its ICC profile is not applied).
- The Tauri app: unchanged (it is being retired; not a parity item, GR8).
- HDR and Windows automatic colour management (Q7).

## 4. Architecture

### 4.1 Where the code goes

| Part | Crate | Notes |
|---|---|---|
| Profile parsing, transform cache, `convert_rgb8` / `convert_rgb16_to_8`, the built-in sRGB / Adobe RGB / ProPhoto profiles | **new internal crate `pw-colour`** (Q3) | Wraps `moxcms` (§6). Plain Rust, testable with synthetic profiles. Transforms are built once per (source profile, destination) and shared (`Arc`) |
| Reading the monitor profile | `pw-loupe` (Windows only) | At start, before the loupe core starts (so C4 can hand it to the workers): the **primary** screen (`MonitorFromPoint` with `MONITOR_DEFAULTTOPRIMARY`, `GetMonitorInfoW`, `CreateDCW` on its device name, `GetICMProfileW`), where the maximised window opens. Checked by the reviewer on 2026-11-11: it returns the per-user association, `…\color\CX240(…)08CAL (DP).icc`. Read the file, pass the bytes to `pw-colour`. Non-Windows builds and failures: sRGB |
| TIFF source profile, 16-bit path | `pw-tiff` | `decode` returns the ICC bytes alongside the pixels; a 16-bit image stays 16 bit until `pw-colour` has converted it, then reduces. Memory: +2 bytes/pixel while converting (240 MB for 40 MP), counted in the budget |
| JPEG source profile | `pw-libjpeg` | `tj3GetICCProfile` (libjpeg-turbo 3.1 in vcpkg already has it, with `TJPARAM_SAVEMARKERS`); no new dependency |
| Exif Adobe RGB marker | `pw-raw` | ColorSpace (0xA001) and InteropIndex (`R03`) from the file or the embedded preview's Exif |
| Applying it in the loupe | `pw-viewcore` (`native`) | The session knows its source profile; `render_half` and `render_region` convert their output (RAW, JPEG). A TIFF session holds the already converted full image. Changed by ADR 0027 W1 (T308, T309): the view core converts each view after its reduction |
| Thumbnails | `pw-thumbcache` | Converted after the reduction to the cell size (cheap) |

### 4.2 Caches

- Every cache (loupe, prefetch, thumbnails) holds **display-ready** pixels. The monitor profile is
  fixed for the run, so no cache key changes now. When GR7 comes, a profile change clears them.

### 4.3 Cost

- A matrix/TRC → matrix/TRC transform is three table lookups and a 3 × 3 matrix per pixel. A full
  40 MP 16-bit TIFF is the worst case and is converted once per decode; a loupe region at fit
  (about 2 MP) is small. Measured in step C5 with `latency.ps1` against today's median 16.1 ms
  (ADR 0015 §7).

## 5. Display

- **Window title** (RawStride has no info or help panel, so the title is where it is always
  visible; reviewer's refinement 2026-11-11): "RawStride  ·  Skärmprofil: CX240(…)08CAL (DP)"
  (the file name), "RawStride  ·  Ingen skärmprofil – visas som sRGB", or "RawStride  ·
  Skärmprofilen kunde inte användas – visas som sRGB".
- **Status line (loupe):** unchanged when the source profile is used. When an embedded profile cannot be
  used: "…  ·  TIFF-fil 6000 × 4000  ·  Inbäddad färgprofil kunde inte användas  ·  …" (reviewer's
  refinement 2026-11-11: without "visas som sRGB", since a JPEG with an unusable ICC and the Exif
  Adobe RGB marker is shown as Adobe RGB).
- **Errors:** none new; colour problems never stop an image from being shown.

## 6. Dependencies (GR10, approved by the owner 2026-11-11)

- **`moxcms` =0.8.1** (BSD-3-Clause OR Apache-2.0; pure Rust; MSRV 1.85), in `pw-colour` only. It
  is **already in `Cargo.lock` and already built for `pw-loupe` on Windows** (through `image` 0.25.10
  ← `arboard` ← `egui-winit`), with its dependencies `num-traits` and `pxfm`. So **no new crate in
  `Cargo.lock`**. Alternative: `lcms2` (Little CMS, MIT) via vcpkg: the reference implementation,
  but a new native dependency and a C build.
- **`windows-sys` =0.61.2** as a direct dependency of `pw-loupe` with the features
  `Win32_Graphics_Gdi` and `Win32_UI_ColorSystem`. Already in the build through `eframe` and `rfd`;
  adds features, no crate.
- `pw-libjpeg`: new FFI declarations for `tj3GetICCProfile` and `tj3Set` (`TJPARAM_SAVEMARKERS`),
  no new library.

## 7. Behaviour requirements

| # | Requirement | Target |
|---|---|---|
| **F1** | An sRGB image converted to a known destination profile gives the expected values (within 1 step at 8 bit) | Tests with synthetic profiles (sRGB → Adobe RGB, ProPhoto → sRGB) against reference values |
| **F2** | Source profile detection: TIFF tag, JPEG APP2, Exif Adobe RGB marker, sRGB default | Tests on synthetic files |
| **F3** | Without a monitor profile, every pixel is exactly as today | Test: identity when destination = sRGB; the existing pixel tests stay unchanged |
| **F4** | A broken source or monitor profile never stops an image from being shown (G3) | Tests |
| **F5** | Side by side with Lightroom on the CX240, an sRGB JPEG and a ProPhoto TIFF look the same | By hand (owner) |
| **F6** | Stepping to a prefetched image still meets V1 (median ≤ 30 ms) | `latency.ps1` on the owner's TIFF and RAW sets |

Results (C5, 2026-11-11, the owner's acceptance; the reviewer's checks from the log in
`docs/tasks/done/log-2026-11-08-to-2026-11-13.md`):

| # | Result |
|---|---|
| **F1** | `pw-colour` reference-value tests (T68): every reference value, computed independently of `moxcms` by the reviewer, passes within 1 with no tolerance widened, so `moxcms` stays (Q5) |
| **F2** | Tests on synthetic files: TIFF tag 34675 (T70, also BYTE and UNDEFINED tags), JPEG APP2 ICC (T71, also a reused decoder), the Exif Adobe RGB marker (T71), and the source rule `jpeg_source_profile` (T72). Real files: all 9 Lightroom TIFFs `Embedded` (T70); Sony ARW/JPG `Srgb`, `_TN0633x` ARW/JPG `AdobeRgb` through the Exif marker only, Pixel DNG `Uncalibrated`, RW2, `TN0025xx.DNG` and TIFF `Unknown` (T71); the DxO export `TN005701_DxO.jpg` has an APP2 Adobe RGB profile, `TN005702_DxO.jpg` none (T72 review) |
| **F3** | Identity when source and destination are the same profile (T68); the existing pixel tests unchanged; with an sRGB destination a session and a thumbnail equal today's decode (T72) |
| **F4** | Tests: a damaged or unsupported source profile falls back to sRGB (T68, T70, T72), a missing or unusable monitor profile gives sRGB (T69); the status-line note "Inbäddad färgprofil kunde inte användas" (T73), checked by hand by the owner on `TN005701_DxO_trasig-profil.jpg` (shown) and `TN005702_DxO.jpg` (not shown) |
| **F5** | No Lightroom: objective check instead (C5): the eight sRGB patches of §1.1 shown in RawStride with T72 (release build) and read back from a screenshot of the CX240 matched values computed independently of `moxcms` from the CX240 profile within 1 (seven exactly; before T72 they were shown unchanged). Visual side-by-side check with DxO PhotoLab; accepted by the owner ("godkänt") |
| **F6** | The owner's `latency.ps1` runs (RawStride started on a file, release build with T72–T73): TIFF (label `c4-tiff-next-2`, the 9 TIFFs, RIGHT × 8) first change 16.4–30.6 ms, **median 17.4 ms** (16.1 ms before colour management, ADR 0015 §7); RAW (label `c4-raw-next`, the last 20 rows of `latency-rawstride.csv`, 20 RAW files) 13.7–17.5 ms, **median 17.0 ms**; one change each. V1 is met with colour management on. An earlier run from the grid and one that ran past the end of the folder are not valid (log) |

## 8. Testing and acceptance

### 8.1 Automated (`cargo test`)

- `pw-colour`: F1, F3, F4; transforms are cached; 16-bit path equals 8-bit path within one step.
- `pw-tiff`: the ICC bytes of a synthetic TIFF with tag 34675; existing tests unchanged.
- `pw-libjpeg`: ICC from a synthetic JPEG with an APP2 profile (also a multi-chunk profile).
- `pw-raw`: the Adobe RGB marker.
- `pw-viewcore`, `pw-thumbcache`: a session and a thumbnail are converted with a given destination;
  with sRGB destination they are bit-identical to today.
- The owner's monitor profile is **not** committed (it carries the screen's serial number); tests use
  `moxcms`'s built-in profiles or synthetic ones.

### 8.2 By hand (owner)

- **Passthrough check before C2:** a test image with known values (sRGB patches) shown in RawStride
  and read back with a screenshot: the values are unchanged. **Done 2026-11-11** (§1.1).
- F5: the same sRGB JPEG and ProPhoto TIFF in RawStride and Lightroom side by side; saturated reds
  and greens, skin tones, a grey ramp.
  **The owner has no Lightroom (2026-11-11):** DxO PhotoLab (colour managed) is the visual reference
  instead. Objective check done the same day: the eight sRGB patches of §1.1 shown in RawStride after
  T72 matched values computed independently from the CX240 profile within 1 (seven exactly).
- The info panel names the profile. Rename the profile association away: RawStride says "ingen".
- F6 with `latency.ps1`.

## 9. Work steps

| Step | Content |
|---|---|
| **C0** | Accept this ADR; answer §11; approve §6 (GR10); accept F1–F6 — **Done 2026-11-11** |
| **C1** | New crate `pw-colour` (tests F1, F3, F4) — **T68, accepted 2026-11-11** |
| **C2** | `pw-loupe`: read the monitor profile at start; window title (passthrough check §8.2 done first) — **T69** |
| **C3** | Source profiles: `pw-tiff` (ICC, 16-bit path) — **T70**; `pw-libjpeg` (APP2) and `pw-raw` (Exif marker) — **T71** |
| **C4** | Apply: `pw-viewcore` sessions and `pw-thumbcache` — **T72**. The status-line note for an unusable embedded profile (§5) follows in its own task, since it needs a field through the loupe core |
| **C5** | Acceptance by hand (§8.2) — **accepted by the owner 2026-11-11** (objective patch check within 1; visual check with DxO PhotoLab instead of Lightroom). F6 met: `latency.ps1` medians 17.4 ms (TIFF) and 17.0 ms (RAW), §7 |
| **C6** | Documentation: architecture.md, test inventory, GR8 note, ADR 0015 §3.2 note — **Done:** T74 |

C2 and C3 need C1 and do not depend on each other; C4 needs both.

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| egui does not pass values through on some driver | Converted values are changed again | Passthrough check (§8.2) before C2 |
| Windows automatic colour management turned on later | Double conversion, colours too dull | Q7: detect it and skip our conversion, or document "off" |
| `moxcms` is young (0.8) | Wrong values on unusual profiles | F1 reference tests; fallback to sRGB on errors; `lcms2` as the alternative |
| 16-bit conversion of large TIFFs | Slower decode, more memory | Counted in the budget; measured in C5 |
| ProPhoto colours outside the panel | Clipping, flat saturated areas | Relative colorimetric clips like Lightroom's display path; perceptual needs LUT profiles (Q6) |

## 11. Questions and the owner's answers (2026-11-11)

The owner answered Q1, Q2 and Q5 and then accepted every other proposal as written, together with
§6 and F1–F6.

| # | Question | Decision |
|---|---|---|
| **Q1** | Order: this before IPTC (ADR 0007)? Numbering: ADR 0016 = milestone 7, IPTC moves to 8? | **Decided (owner): colour first, as the better order.** This is milestone 7; ADR 0007 (IPTC) becomes milestone 8. GR8, the README and ADR 0015 §2 are updated in step C6 |
| **Q2** | ColorNavigator target for the CAL mode, and "08CAL" in the file name | **Answered (owner's screenshot):** mode 7 "CAL (DP)", target "105cd 6900K 2,20", gamut Native (§1.2). "08CAL" is only the file name; the menu uses 7 |
| **Q3** | A new crate `pw-colour` or the code inside `pw-viewcore`? | **Accepted: a new crate**: `pw-thumbcache` needs it without pulling in the workers (as `pw-tiff`, ADR 0015 Q1) |
| **Q4** | When is the monitor profile read? | **Accepted: at start only**; screen moves (GR7) and recalibration while running are a later step |
| **Q5** | Library: `moxcms` 0.8.1 or `lcms2` via vcpkg? | **Decided (owner): `moxcms` =0.8.1**, as proposed: already compiled in the build, no C, permissive licence. `lcms2` stays the fallback if F1 fails |
| **Q6** | Rendering intent | **Accepted: relative colorimetric, no black-point compensation**, as Lightroom's display path for matrix profiles |
| **Q7** | Windows automatic colour management / HDR | **Accepted: not supported in this ADR**: RawStride assumes both off and says so in the info panel if it can detect them |
| **Q8** | RAW renderings in a wider space than sRGB (LibRaw Adobe/ProPhoto, 16 bit) so RAW mode uses the whole panel | **Accepted: later**, its own decision (affects ADR 0006 and the render cost) |
| **Q9** | Acceptance images | **Accepted:** the owner's 9 ProPhoto TIFFs, one sRGB camera JPEG, one Adobe RGB camera JPEG (if the camera can), and one JPEG with an embedded profile |

## 12. Consequences

- Images look as in Lightroom on the owner's wide-gamut screen; sRGB images are no longer
  oversaturated and ProPhoto TIFFs no longer dull.
- A new crate `pw-colour`; `moxcms` becomes a direct dependency (no new crate in `Cargo.lock`).
- Every cache holds display-ready pixels, which ties them to one screen until GR7 is done.
- The open GR8 question about colour management is answered by this milestone.
- **Milestone 7 (colour management) is complete** (2026-11-11): C0–C6 done; IPTC (ADR 0007) is
  milestone 8.
