# ADR 0008: Image presentation architecture for the viewer

- Status: **accepted by the owner (2026-11-05)**, as a whole: the common core, the display modes (G4), the
  prototype plan (section 8) and the method of choosing.
  - **Amendment 1 (2026-11-05):** the demosaic method is decided. PPG is the default method; AHD is kept as
    the quality reference. GPU demosaicing (P6) is still investigated. See section 11.
  - **Amendment 2 (2026-11-05):** phase 1 is done (P0, P4b, P4, P5). The scheduling strategy and a
    memory budget of **4 GB** are decided. See section 12.
  - **Amendment 3 (2026-11-05):** the presenter is decided: **B2-res**, meaning WebView2 shared
    buffers into WebGL2, with prepared views kept as GPU textures. A (wgpu) is not chosen. See
    section 13.
  - **Amendment 4 (2026-11-05):** P6 results (GPU demosaicing in B2) and the orientation check. No new
    decision: PPG stays the default method and AHD the quality reference. P6b (PPG as a GPU shader) is
    planned and waits for the owner's approval. See section 14.
  - **Amendment 5 (2026-11-05):** P6b results (PPG as a GPU shader in B2-res). No production decision:
    PPG stays the default method and AHD the quality reference. GPU PPG is a promising candidate for
    production, with open questions on dead-pixel correction, the memory budget, the texture cache size
    and the CPU unpack. GPU PPG does not solve the slow unpack of new RAW files. See section 15.
  - **Amendment 6 (2026-11-07, owner decision):** the GUI is built in **pure Rust with eframe + egui**
    ([GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md)).
    - **Superseded:** the presenter decision B2-res (section 13) and the rejection of a native UI (C3).
      No WebView2, WebGL2 or separate wgpu pipelines for the UI.
    - **Kept:** the core (section 2), PPG/AHD (section 11), strategy p5 and the 4 GB budget
      (section 12), V1–V8 and G1–G4, and all measurements as a record.
    - See section 16.
  - **Amendment 7 (2026-11-07):** P6c results (GPU PPG memory budget and texture cache, measured in
    the WebView2 presenter). No decision. The memory factors are specific to WebView2/ANGLE and do not
    describe the eframe + egui GUI. The findings on cache size, hit rates, eviction and measurement
    method carry over as questions to measure again. See section 17.
  - New dependencies (wgpu or another GPU library) still need separate approval before they are added or
    installed. libjpeg-turbo was approved for prototype P0 on 2026-11-05; using it in the app is a
    separate decision.
- Date: 2026-11-04 (section 9 FastRawViewer reference, G4 and the prototype plan updated the same day)
- Basis:
  - [requirements-image-viewer.md](../requirements-image-viewer.md) (V1–V8, G1–G4; approved 2026-11-04, G4 and the V2 clarification added 2026-11-04/05);
  - ADR 0003 (M1 viewer), ADR 0006 (RAW pipeline M2);
  - `docs/analyses/2026-11-04-m2-libraw-decode.md`, `docs/analyses/2026-11-04-m2-pipeline-steps.md`;
  - the pixel measurements of 2026-11-04 (`D:\PhotoWorkflowData\pm-study\results\`).
- Out of scope:
  - floating windows and free window/panel placement (a future UI requirement);
  - large folders (1 000+ images; deferred);
  - the contact sheet;
  - a database. The architecture stays file-based: caches live in memory and, where useful, as cache
    files.

## 0. How claims are marked

| Mark | Meaning |
|---|---|
| **[M]** | Measured by us on the development PC (source given) |
| **[D]** | Verified from documentation or source code (link given) |
| **[T]** | Theory or estimate. Plausible, not verified |
| **[P]** | Must be prototyped and measured before it can be relied on |

## 1. Problem

The requirements ask for:

- **≤ 30 ms** for next/previous image, fit ↔ 100 % and **sharp real 100 % from RAW**, even when the user
  zooms at once (V1, V3, V7);
- no intermediate frames (V2);
- zoom and position kept across images (V6);
- a configurable memory budget (V8).

Measured baseline (dist build M2, pixel method) **[M]**:

| Case | Today | Target |
|---|---|---|
| Next image, fit, A1/RW2 | 16–34 ms | ≤ 30 |
| Next image, fit, Xiaomi DNG preview 4096 px / Ricoh DNG 6000×4000 | 67–99 / 141–185 ms | ≤ 30 |
| First zoom to 100 % on an A1 (8640 px embedded JPEG) | blurred after 33 ms, sharp after 435 ms | ≤ 30, no blur |
| A7R IVA real 100 % from RAW, after 3–4 s at fit | first change 17–33 ms, sharp after 682–699 ms | ≤ 30, also without waiting |
| Zoom across images | reset to fit | kept (V6) |

**Where the time goes today** [M]/[D]:

1. **WebView2 decodes the JPEG at display time.** Rust serves *encoded* bytes through `pw://` (ADR 0003).
   - A large preview is decoded only when it is needed: Ricoh/Xiaomi DNG previews at fit, the A1 8640 px
     JPEG at the first zoom.
   - In M1 our own reader decoded the 8640×5760 JPEG in 174 ms single-threaded (zune-jpeg)
     (`2026-11-03-pw-raw-preview-timing.md`).
2. **RAW tiles are rendered after Z, then JPEG-encoded and decoded again.**
   - A 1024² tile costs ~62 ms render + ~20 ms JPEG encoding, plus decoding in WebView2.
   - A 100 % view needs about 4 tiles.
   - Unpacking costs 290–380 ms if it has not been done (`m2-pipeline-steps.md`).
3. **The loupe calls `fitToWindow()` on every image change** (`ui/web/app.js`). This breaks V6.

## 2. Common core (needed by every option)

Most of the work is the same for every presentation option, so it is decided here.

### 2.1 A view core in Rust (`pw-viewcore`, a new crate built on `pw-rawview`)

| Part | Content |
|---|---|
| **View model** | Current image, zoom, and position as a *relative point in the oriented image* (V6). Clamping, and centring for smaller images. **Display mode RAW or JPEG**, set only by the user (G4) |
| **Decoded-image cache** | Decoded RGBA/RGB images (embedded JPEG, full size), and for RAW the **rendered regions** (tiles) and the **unpacked RAW sessions**. Each entry knows its source (G1) |
| **Prefetch policy** | What to prepare, and in which order (2.3) |
| **Scheduler** | `pw-rawview`'s workers, priorities, cancellation and budgets (ADR 0006), extended with JPEG-decode and region-render job types |
| **Budget** | One user setting, split between decoded images, RAW sessions and tiles (V8). The current image is never evicted. **Decided 2026-11-05 (section 12):** 4 GB by default, as one budget for sessions and views, with LRU eviction of what the scheduler no longer wants. The GPU texture cache of the presenter (section 13) needs a budget of its own |
| **Swap rule (V2)** | The presentation receives only a *complete* view: all tiles visible at the target zoom, or the whole image at fit. It swaps to it in **one direct swap** as soon as it is ready. **No blurred, low-resolution or unfinished image is ever shown.** Until the swap, the previous view stays. **Permitted exception (owner, 2026-11-05; V2 in the requirements):** if the finished view is not ready after **1 s**, a **neutral loading indicator** may be shown. It contains no image content of the new image, and it never blocks stepping, zooming or other actions. When the finished view arrives, it replaces everything in one swap. The 1 s value is a starting point and may be tuned after the prototypes |

### 2.2 Decoders

- **JPEG** (embedded previews) [D] [P]:
  - libjpeg-turbo / TurboJPEG 3 supports **scaled decoding** (multiples of 1/8) and **cropped
    decoding**: `tj3SetScalingFactor`, `tj3SetCroppingRegion`
    ([TurboJPEG API](https://rawcdn.githack.com/libjpeg-turbo/libjpeg-turbo/main/doc/html/group___turbo_j_p_e_g.html)).
  - zune-jpeg (already used in the bench) is listed as **not multithreaded**
    ([zune-jpeg Benches.md](https://rust-digger.code-maven.com/src/zune-jpeg-0.4.14/Benches.md)).
  - Which is faster on our files, and whether one large JPEG can be split across cores, is **[P]**.
  - Parallelism across *several* images is always possible.
- **RAW:** `pw-libraw` (LibRaw 0.22.3 with OpenMP), unchanged [M]. Region rendering is identical to the
  same area of a whole-image decode (M2 follow-ups 3–4).

### 2.3 How 30 ms for real RAW 100 % can be reached (V7)

Rendering a 100 % view of an A7R IVA after Z takes ~250–400 ms of work [M]. **30 ms is therefore only
possible if the view has been rendered before Z is pressed.** That is speculative pre-rendering, not a
faster decoder.

1. **Zoom target of the current image.**
   - After the image has been current for a short dwell (today 150 ms), unpack it.
   - Then render the tiles that **Z would show**: the V6 position carried over from the previous image,
     or the centre.
   - Then render a ring of neighbouring tiles for panning.
   - Estimated cost for A7R IVA [T]: unpack 290–380 ms + 4 tiles × ~62 ms on one worker ≈ 0.55–0.65 s.
     With tile rendering on 2–4 workers or with OpenMP, it may be ≈ 0.4 s [P].
2. **Neighbour in the direction of travel.** Unpack the next image, and render its zoom target, when the
   CPU budget allows.
3. **Zoom at once on arrival (V7 "without waiting").** This can only meet 30 ms if step 2 has already
   prepared the image as a neighbour.
   - Feasibility at a steady pace [T]: one A7R image needs ≈ 0.4–0.65 s of work. On 8 cores, about
     **2–4 images/s** could be fully prepared ahead.
   - At faster stepping, the newest image cannot be ready. V7 is then missed, and the view must follow V2
     (no blur; the old view stays, then one swap).
   - **[P]:** measure the real throughput with OpenMP and parallel tile workers.
   - **Owner's decision (2026-11-05):** the 30 ms target of V7 is **not** limited to images that have had
     time to be prepared. V7 is tested both **immediately after an image change** and **after a pause**,
     and the effectiveness of pre-rendering is measured **during normal stepping** (8.3).
4. **Don't encode tiles.** Tiles stay as RGB in memory and go to the presentation without JPEG encoding.
   This removes ~20 ms per tile, plus the decoding in WebView2 [M/T].
5. **Cancellation.**
   - Tile jobs are short (~62 ms) and are dropped when the image changes (existing engine).
   - **Unpacking cannot be interrupted** (LibRaw), so at most one unpack runs per worker, and the current
     image's unpack has priority (ADR 0006 rule).
   - Prefetch never delays a job for the current image. Prefetch jobs start only when no job for the
     current image is waiting.

### 2.4 Memory (V8) [T] – figures are per image

| Item | A1 | A7R IVA |
|---|---|---|
| Decoded embedded JPEG, full size (RGBA) | 8640×5760×4 ≈ 199 MB | 1616×1080×4 ≈ 7 MB |
| Unpacked RAW session | – | ≈ 126 MB (measured kept session, M2) [M] |
| 100 % view + ring (≈ 12 tiles × 1024² × 3) | – | ≈ 38 MB |

A budget of about 2–3 GB holds the current image plus 2–4 neighbours in each class. The default is to be
set after measurement **[P]**.

## 3. Presentation options

> **Superseded 2026-11-07 (section 16).** Options A, B and C below assumed a Tauri/WebView2 shell. The
> owner has chosen a native Rust GUI with eframe + egui (`docs/GUI_REQUIREMENTS.md`). The analysis is
> kept as the record of how the presenter was chosen in phase 2.

### A. Rust + wgpu: native GPU surface for the image area, UI stays in WebView2

- The window gets a wgpu surface that covers the image area. The WebView2 UI is a **transparent child**
  above it, or the UI and the image area are laid out side by side.
- wry's own example renders wgpu in the whole window, with a transparent child webview built with
  `build_as_child` [D] ([wry wgpu.rs](https://docs.rs/crate/wry/latest/source/examples/wgpu.rs)).
- A Tauri 2 project shows a "Webview UI being rendered over a WebGPU texture in the same window" [D]
  ([tauri-wgpu-cam](https://uithub.com/clearlysid/tauri-wgpu-cam)); its exact mechanism is not verified.
- **Images are GPU textures.** Decoded images and tiles are uploaded once and kept in VRAM within the
  budget. Fit uses mipmaps or a scaled copy. Fit ↔ 100 % and panning only change sampling, so they cost
  about one frame [T].
- **Swap timing (V2)** is fully ours: the new texture set is presented in one frame.
- **Backends:** wgpu on Windows uses DX12 (or Vulkan). Nothing is specific to the Arc B580.
- **Risks** [D]/[T]:
  - z-order, focus, DPI and resize handling between the surface and the child webview. The wry example
    does not even update the webview bounds on resize.
  - Tauri multi-webview/child APIs are partly `unstable`, with open bugs (#8705, #9257, #10420) [D]
    ([Syrinx notes](https://github.com/Fledge-Software/Syrinx-app/wiki/Background-01-shell-and-engine)).
  - Overlays (rating, badge, histogram) must be drawn natively or by the transparent webview on top.
  - A transparent webview may cost extra GPU power (measured on macOS by another project) [D].
- **New dependency:** wgpu (+ raw-window-handle). Licence MIT/Apache-2.0 [T – to verify in the
  dependency review]. Ongoing API churn between wgpu versions [T].

### B. Rust + WebView2: the same core; decoded pixels go to WebView2

Variants for moving pixels:

| Variant | Mechanism | Copies | Status |
|---|---|---|---|
| **B1** today's `pw://` with *encoded* JPEG | WebView2 decodes | encode (RAW tiles) + decode | measured baseline; cannot meet V3/V7 |
| **B2** shared memory | `CreateSharedBuffer` + `PostSharedBufferToScript`; script gets an `ArrayBuffer` on the same memory ("accessible from both the app process and script"), < 2 GB, read-only mode available; WebView2 SDK ≥ 1.0.1661.34 [D] ([docs](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2environment.createsharedbuffer)) | Rust writes RGBA once → JS uploads to a WebGL/WebGPU texture or `ImageData` → GPU | **[P]** |
| **B3** `pw://` with *raw* RGBA (no encoding) | custom-protocol response body | Rust → IPC copy → JS buffer → texture | **[P]**, probably slower than B2 [T] |

- **Tauri does not expose the shared-buffer API.** It must be called through the native WebView2 COM
  object (Tauri's platform webview access + the `webview2-com` bindings) [T – to verify].
- **Presentation in WebView2:** a WebGL2 or WebGPU canvas holds the textures. Fit/100 %/panning are
  shader changes, as in A, but inside Chromium's compositor.
  - WebGPU support in WebView2/Edge depends on the runtime version [T]. WebGL2 is the safe baseline.
- **Cost per swap [T]:**
  - copy into the shared buffer (A1 full image ≈ 199 MB; one 100 % view ≈ 6–12 MB);
  - texture upload in Chromium's GPU process;
  - compositor latency.
  - For a 100 % view (a few MB) this is plausibly a few ms. For a full 8640 px image it may be tens of
    ms. **Whether a full-size A1 upload fits in 30 ms is [P].**
- **Swap control (V2):** the JS side can swap a whole set of textures in one `requestAnimationFrame`.
  Frame pacing is Chromium's [T].
- **Advantages:**
  - one UI technology;
  - no z-order/child-window problems;
  - overlays stay in HTML;
  - the M2 UI code (badge, chips, tiles) stays close to today.
- **Risks:**
  - shared-buffer lifetime and release (`releaseBuffer`, `RO_E_CLOSED`);
  - a renderer crash if a read-only buffer is written [D];
  - VRAM residency managed by Chromium, not by us;
  - an extra copy compared with A.

### C. Hybrid / other options considered

| Option | Assessment |
|---|---|
| **C1** WebView2 UI + **native Direct2D/DirectComposition** visual for the image area | Windows-only. It gives the same control as A without wgpu, but binds the code to the Windows APIs. wgpu covers the same need portably. **Not recommended** [T] |
| **C2** Keep today's B1 path, but prefetch with `createImageBitmap` of the *large* previews in JS | Cheapest change. It may help V1/V3 for JPEG previews. It gives no control of memory (V8), cannot carry RAW tiles without encoding, and has no Rust budget. **Fallback only** (prototype P1 measures it) |
| **C3** Full native UI (no WebView) | Large rewrite of the working UI. Not justified by V1–V8. **Rejected** (2026-11-04). **Reversed 2026-11-07:** the owner chose a native UI in Rust with eframe + egui (section 16) |
| **C4** A and B2 behind one `Presenter` interface | The core (section 2) does not depend on the presenter. A and B2 can be compared on the same core, and one can remain as a fallback. **Recommended structure** |

## 4. Options against V1–V8 (expected; all [T] until the prototypes are measured)

| Req. | A wgpu | B2 WebView2 + shared buffer | Main dependency |
|---|---|---|---|
| V1 next ≤ 30 ms | likely, if the image is decoded ahead | likely, if decoded ahead + upload ≤ ~10 ms [P] | prefetch in the core |
| V2 no intermediate frames | full control | control via the JS swap; Chromium frame timing | swap rule in the core |
| V3 fit ↔ 100 % ≤ 30 ms | sampling change (≈ 1 frame) if the full image is resident on the GPU | same if the texture is resident; otherwise one upload [P] | VRAM/texture residency |
| V4 rapid stepping ≤ 70 ms | prefetch throughput | the same + upload per step | decode throughput [P] |
| V5 first image ≤ 150 ms | decode (+ upload) | decode + copy + upload | cold JPEG decode time [P] |
| V6 zoom/position kept | core view model | core view model | – |
| V7 sharp RAW 100 % ≤ 30 ms | only if pre-rendered (2.3); presentation ≈ 1 frame | the same; upload of ~4–12 tiles [P] | **CPU throughput of unpack + tiles** |
| V8 budget | RAM in the core; VRAM ours | RAM in the core; VRAM inside Chromium | – |
| G1–G3 | kept (core) | kept (core) | – |

**Conclusion of the comparison:** V7 is decided mainly by the **core** (pre-rendering and
scheduling), not by the presenter. The presenter decides how close to one frame a swap gets, and how
well VRAM can be controlled.

## 5. Reuse of existing code

> **2026-11-07 (section 16):** the rows for `src-tauri`, `ui/web` and `app.js` assumed a WebView2 UI.
> In the eframe + egui GUI, that logic (tile grid, badge, warnings, sequence guards, zoom and pan)
> moves to Rust. The core crates are reused as stated.

| Code | Use |
|---|---|
| `pw-raw` (preview location, EXIF, orientation) | kept unchanged [M] |
| `pw-libraw` (region-identical decode, damaged-file detection) | kept unchanged [M] |
| `pw-rawview` (workers, priorities, cancellation, budgets, session reuse, tests) | kept as the base of `pw-viewcore`. **Extended:** JPEG-decode jobs, zoom-target pre-render, neighbour prefetch, a shared RAM budget, unencoded tiles |
| `pw-rawview::libraw` JPEG encoding of tiles and halves | **replaced** by unencoded RGB in both A and B2 (kept only for B1 fallback) |
| `src-tauri` `pw://` routes | kept for thumbnails and the fallback; the main loupe path changes |
| `ui/web/raw.js`, `rawtiles.js` (tile grid, badge, warnings, sequence guards) | **B2:** adapted to textures. **A:** the logic moves to Rust; badges and chips stay in HTML |
| `app.js` loupe zoom/pan (`fitToWindow` on every change) | **rebuilt** around the core's view model (V6) |
| Error handling M2, `bench`, `verify-dist.ps1`, `latency.ps1` | kept; `latency.ps1` measures the acceptance tests |

- **G4 (2026-11-04):** the automatic step "embedded preview first, RAW swapped in when ready" (M1/M2,
  `raw.js`) is not carried into the new viewer. Its parts are reused for the explicit RAW and JPEG modes:
  the badge, the warnings, the damaged-file handling and the sequence guards.

## 6. Risks, dependencies, licences, maintenance

| Item | Risk | Note |
|---|---|---|
| V7 throughput | **High** | The CPU may not prepare A7R images fast enough for "zoom at once" while stepping. The target stays 30 ms in both cases (owner, 2026-11-05). P4/P6 measure how close we get, and the result is reported as a hit rate (8.3) |
| wgpu (A) | Medium | API churn; window/child-webview integration in Tauri; DPI/resize/focus. MIT/Apache-2.0 [T – verify] |
| WebView2 shared buffer (B2) | Medium | Not exposed by Tauri (COM access needed); release/lifetime rules; WebView2 runtime version (≥ 1.0.1661) |
| libjpeg-turbo (if chosen) | Low–medium | C library through vcpkg (like LibRaw); BSD-style/IJG/zlib licences [T – verify]; more build complexity. zune-jpeg is pure Rust but single-threaded per image |
| GPU portability | Low | Use no vendor-specific features. Test on the Arc B580 and on one other GPU before release [P] |
| Memory | Medium | 199 MB per decoded A1; the budget must hold under fast switching (as tested in M2) |
| Maintenance | – | A: two rendering systems (wgpu + WebView2). B2: one, plus a COM bridge. The core is shared in both cases |

No new tools or libraries are installed or added by this proposal.

## 7. Recommendation

> **2026-11-07:** points 2 and 3 (choosing between A and B2, and keeping B1 as the fallback inside
> WebView2) are superseded by the eframe + egui decision (section 16). Point 1, the core behind a
> `Presenter` interface, still applies: the egui GUI is the presenter.

1. **Build the presentation-independent core first** (section 2), behind a `Presenter` interface (C4):
   - view model (V6);
   - decoded-image and tile caches with one RAM budget (V8);
   - zoom-target pre-rendering and neighbour prefetch (V7);
   - unencoded tiles;
   - the swap rule (V2).
   
   This is needed whichever presenter wins, and it decides V7.
2. **Choose between A (wgpu) and B2 (WebView2 + shared buffer) only after prototypes P2 and P3** have
   been measured against V1–V8 with `latency.ps1`. Neither is assumed faster.
   - Preliminary view [T]: B2 keeps one UI technology and fewer integration risks. A gives full control of
     frame timing and VRAM. If B2 meets V1–V5 and V7 with margin, it is the simpler choice. If its
     upload/compositor cost breaks the 30 ms targets, A is chosen.
3. **Keep B1 (today's path) as the fallback** until the chosen presenter passes the acceptance tests.

## 8. Prototypes and measurements needed before the choice

### 8.1 Rules for all prototypes

- Each prototype is built **apart from the working app**: a separate crate or example, or a feature flag
  that is off by default. The M1/M2 app keeps working unchanged.
- **No prototype starts before the owner has reviewed and approved this updated ADR.**
- **New dependencies** (libjpeg-turbo, wgpu or another GPU library) need separate approval before they are
  added. Nothing is installed without that approval.
- **Same test set for every prototype:** `D:\PhotoWorkflowData\RAW`:
  - A7R IVA ×5 (61 Mpix);
  - A1 ×13 (50 Mpix, 8640 px JPEG);
  - GM1 RW2 ×4;
  - Ricoh DNG ×4;
  - Xiaomi 17 Ultra DNG ×5.
- **Same machine and method:**
  - internal timers (ms per step, in the prototype);
  - `latency.ps1` for everything that reaches the screen;
  - `gpu-sampler.ps1` for CPU, RAM, VRAM and GPU load.
- **Results:** one analysis document per prototype, with measured facts kept apart from interpretation.
  The owner reviews each result before the next phase depends on it.

### 8.2 The prototypes

| # | Prototype | Measures | Answers | Status |
|---|---|---|---|---|
| **P0** | JPEG decoders, Rust, no UI: zune-jpeg vs libjpeg-turbo, full / 1/2–1/8 scaled / cropped region, on the A1 8640 px, Ricoh 6000×4000 and Xiaomi 4096 px previews; single image and several in parallel | ms per image, ms per 100 % view region, cores used | JPEG mode (G4): which decoder, and whether region or scaled decoding is needed for V1/V3/V5 | **done 2026-11-05** (libjpeg-turbo approved for the prototype); results in section 12 |
| **P4b** | Demosaicing, CPU, Rust + LibRaw, no UI: AHD (today) against faster LibRaw methods (linear, VNG, PPG, DHT, AAHD, DCB) and half-size, whole image and one 100 % view region, 1–8 threads (OpenMP) | time; peak memory; quality (8.3) | Which demosaic may be used for browsing in RAW mode, with AHD on demand | **done 2026-11-05**; decision in section 11 |
| **P4** | Core scheduling, no UI: RAW zoom-target pre-render (unpack + the tiles Z would show + a ring); **visible region first** after an image change or zoom, then the rest; neighbour prefetch while stepping at 1, 2, 4 and 7 images/s; with the demosaic candidates from P4b | time until the visible view is ready; % of simulated Z presses served ≤ 30 ms; CPU load | V7: what is reachable "without waiting"; whether "visible region first" helps (testing the FastRawViewer hypothesis on our own code) | **done 2026-11-05**; decision in section 12 |
| **P5** | Memory budget: 2/3/4 GB under fast switching and repeated zooms, both display modes cached | peak RAM/VRAM, evictions, hit rate | The default for V8 | **done 2026-11-05**; 4 GB decided (section 12) |
| **P2** | Presenter B2: WebView2 shared buffer → WebGL2 (and WebGPU if the runtime has it); 100 % view (≈ 6–12 MB) and a full A1 image (199 MB) | swap latency on screen, upload ms, memory, buffer lifetime | Can WebView2 present finished images within the 30 ms targets? | **done 2026-11-05**; chosen as B2-res (section 13) |
| **P3** | Presenter A: wgpu surface for the image area under a transparent Tauri webview; same images | swap latency, resize/DPI/focus behaviour, GPU power | Can wgpu be integrated cleanly, and how much faster is a swap? | **done 2026-11-05**; not chosen (section 13). wgpu stays in the prototype only |
| **P6** | **GPU demosaicing in our own presenter:** upload the unpacked Bayer data (≈ 2 bytes per pixel) and demosaic plus colour-convert in a shader. Variants: P3 (wgpu compute/render) and P2 (WebGPU/WebGL in WebView2). Visible region first. Compared with the CPU (P4b) on the same files | time from image change and from Z to the visible sharp view; upload time; CPU/GPU/VRAM; quality (8.3) | Whether the GPU path shortens image change and zoom, as OpenGL did in FastRawViewer (9.9a), and in which presenter | **done 2026-11-05** in B2 (WebGL2); results in section 14. P6b (PPG shader) planned, not started |
| **P1** | Optional quick fix: JS `createImageBitmap` prefetch of large previews in today's app, behind a flag | `latency.ps1` V1/V3 in JPEG mode | Is a cheap interim fix worth doing? | **approved**; lowest priority |

### 8.3 How the prototypes are compared

**Performance (P0, P4, P4b, P5, P6; and P2/P3 on screen):**

Mandatory scenarios (owner, 2026-11-05). Each is run in RAW and in JPEG mode, at fit and at 100 %:

| # | Scenario | Measured |
|---|---|---|
| S1 | **Image change and immediate zoom:** next image, then Z after 0 / 50 / 150 ms | time from Z to the sharp 100 % view; time from the image change |
| S2 | **Rapid stepping without pauses:** at 4, 7 and 10 images/s (key repeat), at fit and at 100 % | time per step; stale or intermediate frames (must be 0); share of steps ≤ 30 ms |
| S3 | **Switch RAW ↔ JPEG** on the shown image, and right after an image change | time to the new view |
| S4 | **First display of an image that is not in any cache** (cold: new folder, caches emptied) | time to the first complete view and to sharp 100 % |
| S5 | **Re-display of an image already prepared** (back to it after 1–5 steps) | time to the complete view; cache hit or miss |
| S6 | **V7 after a pause:** Z after 1 s, 3 s and 5 s on the image | time from Z to the sharp 100 % view |

- **Pre-rendering effectiveness during normal stepping:**
  - stepping with natural variation (randomised 150–1500 ms between images, and occasional Z);
  - measured: the share of Z presses and image changes served ≤ 30 ms (**hit rate**), the misses with
    their times, and the wasted pre-render work (images prepared but never shown).
- For every scenario, the median, 95th percentile and maximum. A single median is never reported alone.
- CPU cores used, peak RAM, VRAM.
- Throughput: images per second that can be fully prepared ahead.

**Image quality (P4b, P6):**
- **Reference:** the AHD rendering of the same file with the same settings (ADR 0006 §2). It is compared
  on the same 100 % crops: fine detail, edges, high-contrast diagonals, dark noise, and saturated colour
  areas (for example bird plumage, branches against the sky, test charts if available).
- **Objective:**
  - colour: mean and 99th percentile of **ΔE00** in CIELAB against the reference;
  - structure: **SSIM** (and MS-SSIM) on luminance against the reference. PSNR is only a supplement, never
    used alone;
  - **sharpness, measured directly:**
    - edge acutance: the 10–90 % rise distance across the same edges;
    - high-frequency energy: the ratio of Laplacian/gradient energy to the reference on detail areas;
    - where a slanted edge exists in the test images, an MTF50 estimate (slanted-edge method).
  - artefact counters: maze and zipper patterns at edges (high-frequency energy in flat-edge
    neighbourhoods), and false colour (chroma at neutral edges).
- **Visual:** side-by-side 100 % crops (PNG, lossless) for the owner's review. Method names are hidden
  until the review is done.
- **Memory:** peak working memory per method and image.

**Presenters (P2, P3, and P6 inside them):**
- Screen latency with `latency.ps1`, using the same tests as B.2a and section 9:
  - next/previous;
  - Z;
  - Z immediately after an image change;
  - rapid stepping;
  - mode switch RAW ↔ JPEG.
- Robustness: resize, DPI change, focus and keyboard, multi-screen move, GPU memory loss.

**Decision matrix (filled in after measuring):**
- one row per requirement (V1–V8, G1–G4), and one column per combination
  (core + demosaic choice + presenter + CPU/GPU);
- each cell holds the measured value against the target;
- the owner chooses from this matrix. No combination is assumed faster before it is measured.

### 8.4 Proposed order

1. **Phase 1, core without UI:**
   - P0 + P4b, which run independently;
   - then P4 with the chosen candidates;
   - then P5.
   
   This answers how far CPU preparation can get towards V7.
2. **Phase 2, presenters:** P2 and P3 in parallel, with the same finished images from phase 1.
3. **Phase 3, GPU demosaicing:** P6 in the presenter(s) that pass phase 2. It is compared with the CPU path
   from phase 1.
4. **Decision:** owner review of the decision matrix. This ADR is then updated to "accepted" with the
   measured basis.

**Acceptance before the choice:** `latency.ps1` runs of V1–V7 and G4 on the test set. V7 is measured both
after a pause and immediately on arrival.

## 9. Technical reference: FastRawViewer (added 2026-11-04, for review)

### 9.1 Method and setup

- Sources:
  - **[D]** the official documentation;
  - **[M]** hands-on use on the development PC, timed with the same pixel method (`latency.ps1`), the same
    A7R IVA files and the same image region as PhotoWorkflow;
  - passive process counters (CPU time, private memory, threads).
- No decompilation, disassembly, debugger, code injection or analysis of its binaries.
- No setting was changed.
- Raw notes and log: `D:\PhotoWorkflowData\pm-study\results\run-log-frv.txt` (outside the repo).
- **Version:** FastRawViewer 2.0.11, build 2088 (x64, DirectX 9, AVX2), © LibRaw LLC (the authors of
  LibRaw). Trial licence. [M]
- **Settings in use (unchanged):**
  - RAW shown, not the embedded JPEG: status "R", image size 9492 × 6328;
  - "Process RAW data on GPU" = **None** (demosaicing on the CPU);
  - graphics engine DirectX 9;
  - cache 40 RAW + 40 RGB files;
  - prefetch depth 8, with 2 parallel prefetches auto-extending to 8;
  - "Image zoom on next file: Keep zoom ratio";
  - Toggle Zoom = 100 %.
  
  [M]

### 9.2 Documented design [D]

| Topic | Documentation says |
|---|---|
| Decoding | Reads the whole RAW and processes all its data ([manual v2 p.13](https://updates.fastrawviewer.com/data/FastRawViewer2-Manual-ENG.pdf)). Uses fast demosaicing methods for browsing ([manual](https://www.fastrawviewer.com/usermanual16/single-image-view)). Files above a size limit show only the preview |
| When | "decoding the next file ahead"; random browsing prefetches half the depth in each direction; optional idle prefetch ([node/219](https://www.fastrawviewer.com/node/219), [node/768](https://www.fastrawviewer.com/node/768)) |
| CPU/GPU | Demosaicing on the CPU (maximum speed) or the GPU (higher quality). In GPU mode the RAW data is loaded onto the GPU as floating point, and demosaicing plus colour conversion run there. Zoom is done on the GPU ([node/483](https://www.fastrawviewer.com/node/483), [node/219](https://www.fastrawviewer.com/node/219)) |
| Cache | Counted in files. Memory per megapixel: 2 MB Bayer, 4 MB RGB, 8 MB full colour, 16 MB float. Defaults are set from RAM. Prefetch depth ≤ half the cache. Parallel prefetches grow in the direction of travel ([node/768](https://www.fastrawviewer.com/node/768)) |
| Speed | Target "5–7 RAW files per second" on recommended hardware. No benchmark numbers are published |
| Not documented | The decoder internals, whether half-size comes first, default sizes, whether zoom re-decodes |

### 9.3 Measured on the A7R IVA (61 Mpix) [M]

| Test | FastRawViewer | PhotoWorkflow M2 (B.2a / pw-raw100) |
|---|---|---|
| Z fit ↔ 100 % on the image already shown | **14–33 ms**, one change, sharp; ~16 ms CPU | after 3–4 s pause: sharp after **682–699 ms** (blurred first) |
| Next/previous image at fit | **195–211 ms** (first one 340 ms), one change, no intermediate frame; **375–1250 ms CPU per step** (≈ 2–6 cores) | 32 ms first change (embedded 1616 px preview), still after 81–97 ms; half-size RAW later |
| Z 50 ms after RIGHT (timed from Z) | final view **82–94 ms** after Z (2 changes), i.e. ≈ 130–145 ms after RIGHT | not measured |
| Next image at 100 % (zoom kept), 1.5 s apart | **200–209 ms**, one change, sharp | zoom not kept (V6) |
| Rapid at 100 %, no pause | **198–201 ms** per image, sharp | – |
| Process | private memory 2.5 → 3.8 GB; 47–51 threads | – |

### 9.4 What this shows

- **[M]** FastRawViewer shows a **sharp, real RAW 100 % on every image change, in about 200 ms**, with
  no intermediate frame. Fit ↔ 100 % on the shown image costs **one frame**.
- **[T]** The shown image is kept as a processed full-resolution image (zoom only resamples on the GPU),
  and every image change re-processes the RAW (≈ 200 ms of wall time with several cores). Its prefetch
  seems to keep decoded RAW data rather than finished RGB images; the documented cache sizes (2 MB/Mpix
  Bayer) fit this reading. A Z pressed while the image is still being processed finished ≈ 90 ms later.
  This may mean that the visible region is processed first, or simply that the job was already running.
- **[M]** Even this specialised viewer **does not reach ≤ 30 ms** for "next image" or "sharp 100 % right
  after arriving" on 61 Mpix RAW, in its CPU mode on this PC. GPU mode was not tested: it needs a setting
  change (owner approval).

### 9.5 Principles we can reuse (ideas only, no code or implementation copied)

| Principle | Evidence | Use in PhotoWorkflow |
|---|---|---|
| Keep the **current image fully rendered** at full resolution; fit/100 % is then only resampling | [M] 14–33 ms | Core 2.1: after a short dwell, render the whole current image, or at least the zoom target plus a ring. V3/V7 "after a moment" |
| **Atomic swap**, never an intermediate frame | [M] one change per step | Core swap rule (V2), as planned |
| **Fast demosaicing for browsing**; quality modes separate | [D] | Today we render tiles with AHD (~62 ms per 1024² tile ≈ 60 ns/px [M]). A whole A7R image in AHD is ≈ 3.6 s single-threaded [T from M]. A fast demosaic for the "browse" view, with AHD as a later refinement or on demand, is worth measuring (P4b) |
| **Prefetch of decoded RAW data in the direction of travel**, cache counted in files with a memory budget | [D] | Core 2.3 (neighbour unpack), V8 budget |
| **GPU-side RAW processing** as an option | [D], not measured | A possible later step (P6). It favours option A (wgpu compute) or WebGPU in option B; it is not a reason to choose now |

### 9.6 Does pre-decoding plus a cache of finished RAW tiles remove our ~0.7 s?

- **After a moment on the image: yes, in principle.**
  - Our 682–699 ms is spent unpacking (if not done), rendering 4+ tiles after Z, then JPEG-encoding and
    decoding them [M].
  - If the zoom target is already rendered and kept unencoded (core 2.3), Z only swaps textures. That is
    the same mechanism that gives FastRawViewer 14–33 ms.
  - To be confirmed by P4.
- **Immediately on arrival: not by caching alone.**
  - FastRawViewer itself needs ≈ 200 ms per image switch, and ≈ 90 ms after an immediate Z, on 61 Mpix.
  - Meeting 30 ms there requires that the **next** image's zoom target was finished *before* arrival
    (neighbour pre-render).
  - That is bounded by CPU throughput: ≈ 0.4–0.65 s of work per A7R image today [T].
  - P4 must show how many images per second can be served this way. If the answer is below the user's
    stepping speed, V7 is missed in that case. The target is not relaxed (owner, 2026-11-05). The result is
    reported as a hit rate (8.3).

### 9.7 Effect on the comparison of M2, A and B

| | M2 today | A (wgpu) | B2 (WebView2 + shared buffer) |
|---|---|---|---|
| Keep the full-resolution current image ready (instant Z) | no (tiles after Z, JPEG round trip) | natural: textures resident in VRAM | possible: textures in WebGL/WebGPU; the upload cost per new image is to be measured (P2) |
| Swap without intermediate frames | partly (blur first) | full control | swap in one `requestAnimationFrame`; Chromium timing |
| GPU RAW processing later | no | wgpu compute shaders [T] | WebGPU compute, if the runtime supports it [T] |

**2026-11-07:** the "GPU RAW processing later" row (wgpu or WebGPU compute shaders) is not a plan any
more. No separate compute pipelines are built for the UI (section 16, GUI_REQUIREMENTS.md GR4).

FastRawViewer uses a native GPU presentation (DirectX/OpenGL). This **does not prove** that WebView2 is
too slow. It confirms that the presentation must hold *finished, resident textures* and swap them whole.

### 9.8 Changes to the recommendation (proposed, not final)

1. **Unchanged:** build the core first. Choose A or B2 only after P2/P3.
2. **Strengthened:** the core keeps a **fully rendered current image** (or zoom target + ring),
   unencoded, as the primary means to meet V3/V7. FastRawViewer shows that this gives one-frame zoom.
3. **Prototype P4b** (approved 2026-11-04): faster demosaicing against AHD; see section 8.
4. **Prototype P6** (approved 2026-11-04): GPU demosaicing in our own presenter; see section 8.
5. **Reference measurement with "Process RAW data on GPU" on:** done (9.9). In DirectX 9 mode it was
   about 3× slower per image change.

### 9.9 GPU reference measurement (owner-approved, 2026-11-04) [M]

**Procedure:**
1. The original setting was documented: "Process RAW data on GPU" = None, graphics engine "Legacy:
   DirectX 9".
2. Only that one setting was changed, to "Bayer RAW only".
3. FastRawViewer then warned that GPU processing with DirectX 9 is a *suboptimal configuration* and
   recommended OpenGL or DirectX 11. The graphics engine was **not** changed, because that was not
   approved.
4. After the test the setting was restored to None. This was verified by reopening the dialog: the GPU
   page is identical to the original.

Same A7R IVA files, script and image region as before, plus a sampler of Windows performance counters
(≈ 1.5 s per sample):

| Test | CPU mode (re-run) | GPU mode, DirectX 9 |
|---|---|---|
| Z fit ↔ 100 % on the image shown | 15–35 ms | 18–33 ms |
| Next/previous at fit | **195–229 ms** (0.3–0.5 s CPU per step) | **595–662 ms** (0.6–0.95 s CPU per step) |
| Z 50 ms after RIGHT (timed from Z) | 65–81 ms | 455–477 ms |
| Next/previous at 100 % | 195–216 ms | 595–616 ms |
| Private memory | 6.2 GB | 11.0 GB |
| GPU dedicated memory (VRAM) | 1.2 GB | 4.1 GB |
| GPU 3D utilisation (coarse) | ≤ 13 % | ≤ 17 % |

**Conclusion [M + T]** (superseded in part by 9.9a, where OpenGL + GPU was the fastest configuration):
- In the configuration we were allowed to test, GPU processing was about **3× slower** per image change
  and used more RAM and VRAM. Zooming the shown image was unchanged.
- The vendor's own warning means this does **not** show that GPU demosaicing is slow in general. With
  DirectX 11/OpenGL it is untested.
- What the measurement shows is narrower: **moving RAW data to the GPU per image has a cost that can
  outweigh the gain**. For PhotoWorkflow, GPU demosaicing (P6) stays optional and must be measured, never
  assumed. CPU rendering ahead of time (P4/P4b) remains the main path.

### 9.9a Second reference test: DirectX 11 and OpenGL (owner-approved, 2026-11-04) [M]

**Setup and restore:**
- The graphics engine and "Process RAW data on GPU" were changed, and FastRawViewer was restarted after
  each engine change.
- At the start, the dialog already showed a pending change to DirectX 11. It was not made by these
  tests: the verification at 20:14 showed DirectX 9.
- Afterwards the original state was restored and verified after a restart: **Legacy DirectX 9 + None**,
  with no change pending.

Same A7R IVA files, tests and sampler:

| Configuration | Z fit ↔ 100 % | Next/prev at fit | Z 50 ms after RIGHT (from Z) | Next at 100 % | VRAM | GPU 3D max |
|---|---|---|---|---|---|---|
| DirectX 9 + CPU | 15–35 ms | 195–229 ms | 65–81 ms | 195–216 ms | 1.2 GB | 13 % |
| DirectX 9 + GPU | 18–33 | 595–662 | 455–477 | 595–616 | 4.1 GB | 17 % |
| DirectX 11 + CPU | 27–48 | 295–331 | 203–218 | 304–336 | 2.4 GB | 24 % |
| DirectX 11 + GPU | 25–47 | 210–224 | 80–98 | 211–228 | 2.2 GB | 33 % |
| **OpenGL + GPU** | **15–27** | **142–166** | **30–33, one change** | **112–165** | 2.0 GB | 6 % |

RAM (private) was 4–5 GB in the DirectX 11/OpenGL runs. Sessions differ, so RAM is not strictly
comparable.

**What this shows:**
- **[M]** The best configuration (OpenGL with GPU demosaicing) is about **25–40 % faster** than the
  default CPU mode per image change.
- **[M]** With it, a Z pressed 50 ms after RIGHT gave the sharp 100 % view **30–33 ms after Z**, in one
  change. That is about 80 ms after the image change, close to our V7 target.
- **Hypothesis [T], not verified:** FastRawViewer may process the **visible region first** when zoomed. The
  only basis is a measured fact: a Z right after arrival finished *faster* (≈ 80 ms from RIGHT) than a full
  image at fit (142–166 ms).
- **[M]** The presentation path matters as much as the demosaicing location. The same GPU processing was
  3× slower with DirectX 9 and 1.3–1.5× faster with OpenGL. Under DirectX 11 the CPU mode got slower.
  This supports measuring presenters (P2/P3) rather than assuming.
- **[M]** Even the best configuration needs **112–166 ms per image change** on 61 Mpix. ≤ 30 ms per image
  change (V1 for RAW display) therefore still requires images prepared ahead (2.3). GPU processing alone
  does not reach it.

**Effect on this ADR (proposed):**
- P6 (GPU demosaicing) moves from "optional, only if needed" to **"worth prototyping"**. It must be
  measured in our own presenter, because the result depends strongly on the presentation path.
- A further principle to test in P4: **render the visible region first** after a zoom or image change,
  then the rest.

### 9.10 Owner decisions of 2026-11-04 on section 9

- **Faster demosaicing for normal browsing: approved.** PhotoWorkflow may use a faster method than AHD
  for normal viewing, with high-quality AHD available when needed.
  - P4b compares, for A7R IVA and RW2:
    - decode time;
    - image quality and detail at 100 %;
    - colour rendition;
    - artefacts;
    - memory use.
  - **No method is chosen before the owner has reviewed the P4b results.**
- **GPU reference test in FastRawViewer: approved with conditions and carried out (9.9).** The setting was
  restored and the restore verified.
- **Display mode RAW / JPEG: decided (G4 in `requirements-image-viewer.md`).**
  - There are exactly two modes, chosen manually: **RAW** (real RAW rendering) and **JPEG** (the camera's
    embedded JPEG).
  - **There is no automatic mode.** The program never switches mode by itself.
  - Consequences for this ADR:
    - The **view model** (2.1) holds the mode. It persists across images and zoom.
    - **This removes today's M1/M2 behaviour** (embedded preview first, RAW swapped in when ready) from the
      new viewer.
    - **RAW mode:** only RAW renderings are shown. Before an image's RAW view is complete, V2 applies (the
      old view stays) and G1 says that RAW is being prepared. For the first image of a folder there is no
      old view: a neutral "preparing RAW" state is shown, never the JPEG.
    - **JPEG mode:** only embedded JPEGs; 100 % = 100 % of the JPEG. No RAW work is needed for display
      (prefetch may still prepare RAW if the budget allows, so a switch is fast).
    - The **cache** may hold both representations of the current image, so switching mode is a swap of
      resident images (target as V3, ≤ 30 ms, when both are ready).
- **Prototype approvals (2026-11-04):**
  - **P4:** investigate and prototype rendering the **visible region first**.
  - **P4b:** compare faster demosaicing with AHD, including quality and performance.
  - **P6:** investigate and prototype GPU demosaicing in our own presenter, motivated by the OpenGL result
    in 9.9a.
  - The prototypes are still **not started**. They start only after the owner has reviewed and approved
    this updated ADR (section 8).
- **Requirements unchanged:** V1–V8 and G1–G3, including V7's 30 ms target.

## 10. Open questions (owner)

1. ~~V2 timeout~~ – decided 2026-11-05: a neutral loading indicator is permitted after 1 s. There is no
   intermediate image, and the swap stays single and direct (swap rule in 2.1; V2 in the requirements).
2. ~~V7 "normal use"~~ – decided 2026-11-05: the 30 ms target also applies immediately after an image
   change. V7 is tested both directly after an image change and after a pause, and pre-rendering is measured
   during normal stepping (8.3, S1–S6 and hit rate).
3. Prototypes:
   - P4 (visible region first), P4b and P6 are approved in principle (9.10).
   - P0, P1, P2, P3 and P5: approved 2026-11-05, with the measurement additions of 8.3.
   - The ADR as a whole: approved 2026-11-05. The prototypes may start in the order of 8.4.
   - libjpeg-turbo, and any GPU library for P6, are new dependencies and need separate approval before
     installation.
4. ~~Faster demosaic than AHD for browsing~~ – approved 2026-11-04. P4b scope: 9.10. Method chosen
   2026-11-05 after the owner's blind review: PPG, with AHD as the quality reference (section 11).
5. ~~GPU reference test in FastRawViewer~~ – done 2026-11-04 (9.9).
   - Further test with DirectX 11/OpenGL: done 2026-11-04 (9.9a). Settings restored and verified.
6. ~~G4~~ – decided 2026-11-04: two manual modes, RAW and JPEG, no automatic mode (requirements
   document updated).
7. ~~Scheduling and memory budget (V8)~~ – decided 2026-11-05 after P4/P5: the strategy in 12.1, with a
   4 GB default budget.
8. ~~Presenter~~ – decided 2026-11-05: B2-res (section 13). **Superseded 2026-11-07:** the GUI and
   the image presentation are built in Rust with eframe + egui (section 16).
9. ~~P6~~ – done 2026-11-05 (section 14). ~~P6b~~ – done 2026-11-05 and approved by the owner as a
   finished prototype (section 15). libjpeg-turbo in the app still needs separate approval.
   Postponed by the owner: DPI change, moving to another screen, GPU device loss.
10. GPU PPG in production – **open**, not decided. **2026-11-07:** the WebGL2 shader cannot be used in
    the eframe + egui GUI as is. Whether a GPU demosaic step is moved into eframe's renderer is a new
    open question (section 16). CPU PPG stays the decided path. The points below were asked for the
    WebView2 presenter; the memory points must be measured again under eframe:
    - dead-pixel correction before GPU processing: a proposed step, not implemented or verified; other
      camera models must be examined too;
    - whether the 4 GB budget covers the WebView2 GPU process and its memory use (unclear);
    - how a smaller texture cache (preliminary, unverified estimate about 1.3–1.4 GB) affects P5 hit
      rates and stepping;
    - a faster CPU unpack without lower image quality.

## 11. Amendment 1: demosaic method (owner decision, 2026-11-05)

### 11.1 Decision

- **PPG is the default demosaic method** for RAW display.
- **AHD is kept as the quality reference.** New methods, and the GPU path, are measured against it (8.3).
- **The PPG choice does not close P6.** GPU demosaicing in our own presenter is still investigated and
  compared against both PPG (speed) and AHD (quality).
- This amendment records the decision only. Changing the app's renderer from AHD to PPG is implementation
  work, done in its own step, with approval and without breaking working features.

### 11.2 Basis: P4b, run 2026-11-05 [M]

- Machine: thanos, i7-9700K, LibRaw 0.22.3 with OpenMP, ADR 0006 §2 settings.
- Files: five A7R files (9568×6376) and four RW2 files (4608×3464).
- Values are medians over the files. "All" means all threads.
- Quality is measured against AHD. Raw data: `D:\PhotoWorkflowData\analysis\phase1\p4b\`.

| | A7R AHD | A7R PPG | RW2 AHD | RW2 PPG |
|---|---|---|---|---|
| Whole image, all threads | 1871 ms | 1012 ms | 518 ms | 347 ms |
| Whole image, 1 thread | 5936 ms | 1926 ms | 1641 ms | 584 ms |
| 1920×1080 region, all threads | 116 ms | 37 ms | 126 ms | 46 ms |
| Peak private memory | 764 MB | 764 MB | 211 MB | 205 MB |
| ΔE00 mean / p99 against AHD | – | 1.03 / 6.1 | – | 1.54 / 6.5 |
| SSIM / MS-SSIM against AHD | – | 0.992 / 0.9990 | – | 0.977 / 0.9968 |
| Fine detail (HF ratio; AHD = 1) | 1.00 | 0.99 | 1.00 | 0.97 |
| False colour at neutral edges | 3.28 | 4.01 | 2.31 | 2.48 |
| Zipper index (AHD = 1) | 1.00 | 1.04 | 1.00 | 0.87 |

The other methods tested were linear, VNG, DCB, DHT and AAHD:

- None was both faster than PPG and as close to AHD.
- linear loses 40–50 % of the fine detail.
- VNG, DCB and AAHD do not scale with threads, and DCB and AAHD use 2.6–3.8 times the memory.

Blind review: two 640×480 crops at 100 % per file and method, with codes shuffled per file. The owner
judged AHD and PPG to be similar in quality.

### 11.3 Consequences

- **V7 is still not met by the CPU path alone.** PPG renders a 1920×1080 region in 37–46 ms, against the
  30 ms target. The target has to be reached through pre-rendering (P4) and/or the GPU (P6), not through
  the demosaic choice alone.
- **AHD stays available** as the reference for measurement and comparison. Whether AHD is also offered to
  the user on demand is decided separately.
- **P4 and P5 use PPG** as their CPU method, with AHD as a comparison where relevant.

## 12. Amendment 2: phase 1 results, scheduling and memory budget (owner decisions, 2026-11-05)

### 12.1 Decisions

- **Scheduling strategy for the view core (2.1, 2.3), approved by the owner after P4:**
  - The current image is shown first. After a dwell of **150 ms**, the current image's other RAW view is
    pre-rendered: the zoom target at fit, or fit when zoomed.
  - Then **2 neighbours** in the direction of travel are prepared (the shown view and the zoom target),
    then the current image in the **other display mode** (after the neighbours, not before), then a ring
    around the zoom view, then **1 neighbour behind**.
  - The zoom view is rendered as **exactly the visible region** (one LibRaw call), not as 1024-px grid tiles.
  - Two workers: an interactive one for the current image only, and a background one.
  - Demosaicing is PPG (section 11).
- **Memory budget (V8): 4 GB** by default.
  - The budget covers **both unpacked RAW sessions and finished views**.
  - Eviction removes the least recently used items that the scheduler no longer wants, and never
    anything of the current image.
  - Prefetch does not start if its result would not fit.
  - The budget stays a user setting (2.1). 4 GB is the default.
- The 1 s loading indicator and the swap rule (V2) are unchanged.
- This amendment records decisions only. Changing the app (`pw-rawview`, the loupe) is implementation
  work, done in its own step, with approval and without breaking working features.

### 12.2 Basis [M]

- Machine: thanos, i7-9700K, 32 GB. Prototypes are in `prototypes/phase1`, and raw data in
  `D:\PhotoWorkflowData\analysis\phase1\`.
- Times are until the view is **ready in memory**. Presentation on screen is phase 2 (P2/P3).
- The OS file cache was warm. "Cold" means empty application caches.

**P0, embedded-preview JPEG decode, one thread (median):**

| Preview | zune-jpeg | jpeg-decoder | libjpeg-turbo | turbo 1/2 scale |
|---|---|---|---|---|
| A1 8640×5760 | 168–198 ms | 227–253 ms | 142–166 ms | 97–120 ms |
| Ricoh DNG 6000×4000 | ~85 ms | ~113 ms | 73–77 ms | 49–52 ms |
| A7R 1616×1080 / RW2 1920×1440 | 11–13 ms | 14–17 ms | 9–11 ms | 7–8 ms |

libjpeg-turbo is 15–20 % faster than zune-jpeg. Using it in the app is a separate decision; it is
still a new dependency for the app.

**P4, scheduling, viewport 1920×1080. Each cell is the share within 30 ms, then the median:**

| RAW mode, A7R | none | cur | full |
|---|---|---|---|
| Z 0 ms after an image change (S1) | 0 % / 368 ms | 0 % / 363 ms | 100 % |
| Z after a 1–5 s pause (S6) | 0 % / 46 ms | 100 % | 100 % |
| Natural stepping, image change / Z | 18 % / 0 % | 18 % / 31 % | 100 % / 100 % |
| Cold first view / cold Z (S4) | 571 / 43 ms | 574 / 40 ms | 615 / 46 ms |
| Stepping at 4–10 images/s (S2) | 0 % | 0 % | 10 % |

The A1 and RW2 results follow the same pattern.

Comparison on A7R, on top of `full`:

- **Exact region against grid tiles:** a cold Z took 46 ms against 138 ms, and peak memory was 3.8 GB
  against 5.5 GB. This confirms the "visible region first" hypothesis (9.5) on our own code.
- **AHD against PPG:** a cold Z took 85 ms against 46 ms.
- **The 150 ms dwell** cut wasted prefetch by about 25 % without lowering hits.
- **Queue order:** the other display mode placed before the neighbours made JPEG stepping at 10/s worse
  than no prefetch at all. That is why the order above puts it after the neighbours.

**P5, the decided strategy, with the total budget:**

| Budget | A7R hits on change / jump ±3 (MIX) | A1 hits on change / jump ±3 | CPU, average cores (A7R / A1) | Process peak |
|---|---|---|---|---|
| 4 GB | 94 % / 88 % | 94 % / 82 % | 1.20 / 1.29 | 4.3–4.4 GB |
| 3 GB | 94 % / 88 % | 94 % / 65 % | 1.26 / 1.46 | 3.4 GB |
| 2 GB | 91 % / 35 % | 91 % / 47 % | 1.72 / 1.86 | 2.4 GB |

- At every budget, these were 100 % within 30 ms:
  - Z right after an image change (S1);
  - Z after a pause (S6);
  - going back after 1–5 steps (S5);
  - natural stepping, Z.
- JPEG mode at 10/s on A7R: 70 % within 30 ms, against 25 % before the change in queue order.
- The budget is held exactly. The process uses 0.2–0.3 GB more, for LibRaw's working buffers and the
  program itself.

### 12.3 Consequences

- **V7 (30 ms) is met with the CPU path whenever the zoom target was prepared before Z.** That covers
  Z right after a normal-pace image change, Z after a pause, going back, and natural stepping.
- **V7 is not met by the CPU path:**
  - on a cold first view: about 40–46 ms for Z, and 270–620 ms for the first RAW view;
  - when stepping continuously at 4 images/s or faster in RAW mode, because an A7R image needs about
    0.6 s of work.
  
  These are the cases for P6 (GPU), and for the V2 rule (no intermediate image; a neutral indicator
  after 1 s).
- **JPEG mode on the A1:** the decode takes about 150 ms, so continuous stepping at 10/s is not met.
  Scaled decoding (libjpeg-turbo, 1/2 to 1/4) is the candidate for the fit view; it needs approval as an
  app dependency.
- **Phase 2 (P2/P3) uses** the finished views from this strategy, and measures the time on screen.

## 13. Amendment 3: presenter (owner decision, 2026-11-05)

> **Superseded 2026-11-07 by amendment 6 (section 16).** The presenter is no longer WebView2 with
> WebGL2 but the eframe + egui GUI in our own process. The measurements below stay valid as a record.
> One finding carries over: the gain came from keeping prepared images resident on the GPU, not from
> the graphics library.

### 13.1 Decision

- **The presenter is B2-res:**
  - The view core (section 12) writes finished pixels into **WebView2 shared buffers**
    (`CreateSharedBuffer` / `PostSharedBufferToScript`).
  - The page uploads them to **WebGL2 textures** and keeps prepared views on the GPU, as a texture
    cache. An image change or Z is then a draw only.
  - B1 (`pw://` with `fetch`) is replaced for image pixels.
- **A (wgpu) is not chosen.** wgpu is not added to the app. It remains only in prototype P3.
- **P6 (GPU demosaicing) is done inside B2**, with WebGL2 (or WebGPU where the runtime has it). That
  needs no new dependency.
- This amendment records the decision only. Changing the app is implementation work, done in its own
  steps, with approval and without breaking working features.

### 13.2 Basis [M]

Basis: `docs/analyses/2026-11-05-adr0008-decision-matrix.md`.

Times are on screen, from key press to changed pixels, median / p95. The screen is sampled every
~17 ms.

| | B1 (today) | B2 (upload on swap) | **B2-res** | A-up (wgpu) | A-res (wgpu) |
|---|---|---|---|---|---|
| 100 % view 2560×1440 | 145 / 152 ms | 30 / 49 ms | **13 / 44 ms** (19 of 20 ≤ 30 ms) | 30 / 42 ms | 17 / 33 ms (19 of 20) |
| Whole A1 image, fitted | 1599 ms | 203 ms | **16 / 36 ms** (18 of 20) | 120–129 ms | 14 / 37 ms (18 of 20) |

- B2-res and A-res are equally fast. The gain comes from keeping textures on the GPU, not from the
  graphics library.
- B2 was chosen for integration:
  - **dependencies:** none new, against wgpu plus Tauri's `unstable` child-webview feature;
  - **focus:** no problem, whereas A lost keyboard focus after a restore from the taskbar;
  - **code:** less new code.

### 13.3 Consequences and open items

- **V8:** the GPU texture cache lives in the WebView2 GPU process. It needs a budget of its own,
  alongside the 4 GB core budget. P2 grew to 3.0 GB in all. VRAM is not measured yet.
- **The page must redraw on resize**, and set its canvas size from the window (the prototype did not).
- **Not tested, postponed by the owner:** DPI change, moving to another screen, GPU device loss
  (WebGL context loss).
- **Not solved by any presenter:** RAW-mode cold start and fast stepping (V4, V5, V7 cold). That is
  P6.

## 14. Amendment 4: P6 results and orientation check (2026-11-05)

No decision changes. **PPG stays the default demosaic method and AHD the quality reference
(section 11).** The MHC shader of P6 is a measurement candidate, not a replacement.

### 14.1 What P6 did

- **Prototype:** `prototypes/phase3/p6-gpu-demosaic`. Raw data in `D:\PhotoWorkflowData\analysis\phase3\p6\`.
- **CPU side:** LibRaw opens and unpacks the file (ADR 0006 settings). The visible Bayer area (16 bit)
  goes through a WebView2 shared buffer into a WebGL2 R16UI texture, and stays on the GPU.
- **Shaders:** they repeat `dcraw_process` on the GPU, in this order:
  1. black levels per CFA position;
  2. camera white balance, with clipping;
  3. demosaicing;
  4. the camera → sRGB matrix;
  5. dcraw's output curve.
- **Demosaic methods:** `linear` (LibRaw's linear method, used to check the pipeline) and `mhc`
  (Malvar–He–Cutler, 5×5).
- **Fit view:** half-size superpixels, as LibRaw `half_size`.
- **100 % view:** only the visible region is demosaiced, straight into the canvas.
- **Files:** A7R IVA ×5 (9568×6376), Panasonic GM1 RW2 ×4 (4608×3464), A1 ×2 (8640×5760).

### 14.2 Results [M]

**Pipeline check.** GPU `linear` against LibRaw `linear` on the same 1920×1080 region gave **ΔE00
0.00 and SSIM 1.0000 in all 11 files**. The shader reproduces LibRaw exactly under the ADR 0006
settings. The differences below come only from the demosaic method.

**Quality against AHD** (median over the files; the AHD false-colour value itself is about 3.1):

| | ΔE00 mean / p99 | SSIM | Fine detail (HF) | False colour | Zipper (AHD = 1) |
|---|---|---|---|---|---|
| A7R, MHC (GPU) | 1.60 / 6.3 | 0.9921 | 0.94 | 3.89 | **1.63** |
| A7R, PPG (CPU) | 1.03 / 5.4 | 0.9935 | 0.98 | 3.56 | 0.93 |
| RW2, MHC | 1.65 / 5.0 | 0.9710 | 0.97 | 4.71 | **2.19** |
| RW2, PPG | 1.61 / 6.7 | 0.9758 | 0.95 | 3.44 | 0.88 |
| A1, MHC | 1.49 / 4.6 | 0.9842 | 0.89 | 3.53 | **1.21** |
| A1, PPG | 1.44 / 6.1 | 0.9830 | 0.94 | 3.51 | 0.95 |

MHC is close to PPG in colour and structure, but it has 1.2–2.2 times AHD's zipper artefacts, while
PPG is at 0.9. On the A7R files it also loses some fine detail. **MHC does not meet the PPG quality
decided in section 11.**

**Zoom (Z) with the Bayer data already on the GPU** (A7R, on screen, latency.ps1, 20 presses that
alternate fit and 100 %):

- 12–33 ms, median about 26 ms;
- 18 of 20 presses within 30 ms;
- no pre-rendered view is needed.

For comparison:

- on the CPU, a cold Z took 40–46 ms to memory (P4/P5), before the presenter;
- B2-res with a pre-rendered view took 13 ms (section 13).

**Cold start and image change** (unpack included):

| | In the page, total | Open + unpack (CPU) | Copy to shared buffer | Upload to GPU |
|---|---|---|---|---|
| A7R | 436 ms | 291 ms | 52 ms | 57 ms |
| RW2 | 202 ms | 164 ms | 13 ms | 12 ms |
| A1 | 364 ms | 240 ms | 43 ms | 44 ms |

- On screen, A7R, RIGHT to a new file: **533–557 ms**. That is no faster than the CPU path's first
  fit view, about 550 ms (P5 S4), because unpacking dominates.
- What the GPU removes is the CPU demosaic and half-size render. The CPU only unpacks, and a sharp
  100 % view needs no further CPU work.

**Not reliable:** the GPU times measured in the page (0.1–0.6 ms). `gl.finish` under ANGLE/D3D11 does
not wait for the GPU, so only on-screen times are used.

**Memory:** the process tree used 0.8–0.9 GB with one image on the GPU. The A7R Bayer texture is
about 122 MB, plus the half-size texture.

### 14.3 Orientation check (2026-11-05)

**Test set** (ExifTool; `D:\PhotoWorkflowData\analysis\phase3\p6\orient\exif.txt`):

| Camera | Files | Orientation |
|---|---|---|
| A7R IVA (ILCE-7RM4A) | all 5 | **vertical**, Orientation 8, "Rotate 270 CW" |
| Panasonic GM1 | P1240235 vertical (Orientation 8); P1240231, P1240233, P1240236 horizontal | mixed |
| A1 (ILCE-1) | all files | horizontal |
| Ricoh GR III DNG, Xiaomi DNG | both orientations | not in P6 |

The comparison of a vertical and a horizontal file from the same camera is the GM1 pair, P1240235
against P1240231/33/36. There is **no horizontal A7R file** in the test set.

| Question | Finding |
|---|---|
| 1. Was the orientation tag read and used? | **Read, not used.** The shim passes LibRaw's `sizes.flip` to the page, and the page ignores it. Vertical images were therefore shown on their side in P6 |
| 2. Did LibRaw and the shader get the same orientation and size? | **Yes.** Both work in sensor orientation: the references were rendered with `user_flip = 0`, and the shader indexes the sensor grid. Proof: GPU `linear` = LibRaw `linear` (ΔE00 0.00, SSIM 1.0000) for the vertical files (A7R ×5, GM1 P1240235) as for the horizontal ones (GM1 ×3, A1 ×2). A mismatch in orientation or size would have given large differences, or a failed crop |
| 3. Bayer pattern, black level and crop for vertical images? | **Correct.** The CFA pattern, black levels and visible-area crop are defined on the sensor grid, before any rotation, and orientation does not enter them. The exact `linear` match on the vertical files confirms it |
| 4. Was the result rotated after demosaicing, or did orientation affect the demosaic step? | **Neither.** The result was never rotated, and orientation did not touch the demosaic step. That is the right order; only the final rotation for display was missing |
| 5. Were the PPG and AHD comparisons made with matched orientation? | **Yes**, both in sensor orientation (`user_flip = 0`). The P4b comparisons were also matched: each method's region was rotated the same way before the metrics were computed |
| 6. Did orientation affect the performance figures? | **Not materially.** The 100 % region was 1920×1129 in sensor coordinates. For a vertical image shown upright, the region would be 1129×1920: the same number of pixels and the same shader work. The fit pass handles the whole image either way; only the final blit draws a differently sized area (the image was shown on its side). Panning was not measured in P6. P4/P5 already swapped the view size for flip 5/6 |

**Conclusions:**

- **The P6 results remain valid.** The quality comparisons do not need to be redone, because
  orientation was matched and the exact linear match on vertical and horizontal files from the same
  camera shows it.
- **The performance figures do not need to be redone.** The images were shown on their side, but the
  pixel count and GPU work are the same.
- **Missing, and needed in P6b and in the app:**
  1. rotation for display, after demosaicing (flip 3/5/6 as a UV transform when drawing);
  2. mapping the visible screen rectangle to a sensor rectangle, with the size swapped for flip 5/6
     and the position mapped (V6, relative point in the oriented image);
  3. comparisons kept in sensor orientation, or both sides rotated the same way.
- **The test set is lopsided:** the A7R set is all vertical and the A1 set all horizontal. P6b should
  include a horizontal A7R file (none is available yet), or the Ricoh GR III DNG pair.

### 14.4 Remaining uncertainties

- **VRAM:** not measured. WebGL2 exposes no VRAM figure. The process tree's private bytes do not
  show VRAM, and wgpu's allocator report (P3) was not available on Vulkan.
- **WebGL context loss:** not tested. Nothing was reconstructed after a loss, and the Bayer textures
  would then have to be uploaded again.
- **Stepping throughput with GPU demosaicing:** not measured. The CPU work per image falls to unpack
  only (about 290 ms for A7R), but the copy and upload (about 110 ms) are added.
- **Panning at 100 % with the Bayer data on the GPU:** not measured.
- **Texture budget:** each image held on the GPU costs about 122 MB for A7R and about 100 MB for A1
  (Bayer) plus the half-size texture. It has to fit the V8 budget together with the 4 GB core budget
  (section 13.3).
- **The in-page GPU times are unreliable** under ANGLE (14.2).
- **DPI change and another screen:** postponed by the owner (section 13.3).

### 14.5 Next prototype: P6b, PPG as a GPU shader (planned; done 2026-11-05, see section 15)

P6b starts only after the owner's explicit approval. The PPG shader may not enter the app before its
results have been reviewed and approved.

1. **Images and methods:**
   - the same test images as P4b/P6: A7R ×5, GM1 ×4, A1 ×2;
   - a horizontal A7R file if the owner provides one, or the Ricoh GR III DNG pair;
   - the same metrics (section 8.3).
2. **Quality:** against LibRaw PPG (target: as close to identical as P6 `linear`) and against AHD.
3. **Orientation:** rotation for display after demosaicing, screen-to-sensor mapping, and both
   horizontal and vertical files (14.3).
4. **Time:** image change, cold start, Z and panning at 100 %, on screen with latency.ps1.
5. **Stepping:** back and forth, and fast continuous stepping (as P5 S2, S5 and MIX), with GPU
   demosaicing.
6. **Memory:** VRAM and the texture budget against the 4 GB decision (section 12) and V8, as far as
   it can be measured.
7. **Context loss:** recovery after a WebGL context loss, forced with `WEBGL_lose_context`.

Rules for P6b:

- no new dependencies without approval;
- no change to the app's code;
- no commits during the investigation;
- the finished and reported prototype is committed as its own commit.


## 15. Amendment 5: P6b results, PPG as a GPU shader (2026-11-05)

> **2026-11-07 (section 16):** the GPU PPG measured here runs as a WebGL2 shader in WebView2. It is not
> carried over into the eframe + egui GUI as is. The quality results (the GPU shader equals LibRaw PPG)
> stay valid as a fact about the algorithm. The memory results are specific to WebView2/ANGLE.

**No production decision.** PPG stays the decided default demosaic method and AHD the quality
reference (section 11). The presenter stays B2-res (section 13), and the scheduling strategy and the
4 GB budget stay as decided (section 12).

The GPU version of PPG is a **promising candidate for a production implementation**. It is not
approved for the app. The open questions in 15.4 must be answered first, and the owner must decide.

### 15.1 What P6b did

- **Prototype:** `prototypes/phase3/p6b-gpu-ppg`, commit `a44643a`. It is not part of the app and
  adds no new dependencies.
- **Raw data:** `D:\PhotoWorkflowData\analysis\phase3\p6b\`:
  - `p6b-bench.json`;
  - `p6b-latency.csv`;
  - PPM crops in `crops\`.
- **Machine:** thanos, Intel Arc B580, WebGL2 through ANGLE on D3D11.
- **Pipeline:** as in P6 (14.1).
  - LibRaw unpacks on the CPU.
  - The Bayer data goes through a WebView2 shared buffer into an R16UI texture and stays on the GPU.
  - Black level, white balance, matrix and output curve are done in the shader.
- **What is new:**
  1. **PPG demosaicing on the GPU:** dcraw `ppg_interpolate` in two WebGL2 passes, only for the
     visible region.
     - Pass 1: green at R/B sites, written to an R16UI target.
     - Pass 2: R/B at G sites and at R/B sites, followed by colour conversion and the output curve.
  2. **Rotation for display:** each screen pixel is mapped to the upright image, and then through
     flip 3/5/6 to sensor coordinates (14.3).
  3. **A GPU texture cache:** a budget with LRU eviction, prefetch of ±1 and +2, and the stepping
     scenarios from P5.
  4. **GPU memory per process tree:** Windows PDH counters "GPU Process Memory", dedicated and shared,
     including the WebView2 processes.
  5. **Context loss:** forced with `WEBGL_lose_context`.
- **Files:**
  - A7R IVA ×5, 9568×6376, all vertical;
  - GM1 RW2 ×4, 4608×3464, one vertical and three horizontal;
  - A1 ×2, 8660×5784, horizontal;
  - Ricoh GR III DNG ×2, 6020×4024: TN002512 horizontal, TN002513 vertical.
- **Comparison:** a 1920×1080 region at the image centre, against LibRaw PPG and AHD rendered for
  the same sensor rectangle (`cropbox`). The region was compared in sensor orientation and, rotated,
  against LibRaw's own rotation (`user_flip = -1`).

### 15.2 Results [M]

**1. GPU PPG against LibRaw PPG**

| Files | Identical pixels | Max channel difference (8 bit) | ΔE00 mean / p99 |
|---|---|---|---|
| A7R ×5 | 99.990–100.000 % | 1–4 | ≤ 0.0001 / 0.00 |
| A1 ×2 | 100.000 % | 1 | 0.0000 / 0.00 |
| Ricoh GR III DNG ×2 | 100.000 % | 1 | 0.0000 / 0.00 |
| GM1 RW2 ×4 | 99.984–99.989 % | 115–216 | 0.0005–0.0008 / 0.00 |

- **On A7R, A1 and Ricoh, GPU PPG reproduces LibRaw PPG.** At least 99.98 % of the pixels are
  identical, and ΔE00 is in practice 0. The remaining differences are rounding of one or a few steps.
- **Against AHD:** GPU PPG has the same distance to AHD as LibRaw PPG to the third decimal on every
  file. Zipper against LibRaw PPG is 1.000 (1.006 on P1240235).
  - The quality decided in section 11 is therefore reached.
  - The zipper problem of MHC in P6 (14.2) does not occur.
- **Pipeline check:** GPU `linear` against LibRaw `linear` gives ΔE00 0.0000 on every file except
  GM1. The GM1 exception has the same cause as below.

**2. The GM1 deviation and its cause**

- **Where:** on GM1, the differing pixels gather in about 15 small clusters per region, 3×3 to 7×7
  pixels each.
- **Cause:**
  - Each cluster has a sensor pixel with the raw value 0 at its centre, a defective (dead) pixel.
  - For Panasonic files, LibRaw replaces such pixels before demosaicing (`zero_is_bad`, in
    `remove_zeroes`). The new value is the average of the non-zero pixels of the same CFA colour
    around it.
  - The prototype does not do this. The shader treats the 0 as a real value, and PPG spreads it to
    the neighbouring pixels.
- **It is not a demosaicing error.** GPU `linear` shows the same small deviation on GM1.
- **Proposed preprocessing step on the CPU, before GPU processing.** It is **not implemented and not
  verified**; P6b did not test it.
  - **What it would do:** after unpacking and before upload, replace each zero pixel in the visible
    Bayer area when LibRaw marks the file as `zero_is_bad`.
  - **How:** the same way as LibRaw, with the average of the non-zero same-colour neighbours.
  - **Expected cost:** small next to the unpack, since zero pixels are few. Not measured.
  - **Why on the CPU:** the shader would stay the same for every camera.
  - **Expected effect:** GM1 matches LibRaw PPG as closely as the other cameras. That has to be
    verified with the same comparison as in this section.
- **Other camera models must also be examined.**
  - P6b covers four models: A7R IVA, A1, GM1 and Ricoh GR III.
  - For each model the app supports, it has to be checked which corrections LibRaw makes before
    demosaicing under the ADR 0006 settings, for example zero pixels or bad-pixel maps if they are
    turned on.
  - It also has to be checked whether GPU PPG then matches LibRaw PPG.

**3. Orientation**

- **Vertical images** (A7R ×5, GM1 P1240235, Ricoh TN002513): rotated by the shader and compared
  with LibRaw `user_flip = -1`, for a 1080×1920 region in the upright image. The result is
  99.98–100.000 % identical, the same as in sensor orientation.
- **Horizontal images** (GM1 ×3, A1 ×2, Ricoh TN002512): the same result as in sensor orientation.
- On screen, vertical images display upright in both fit and 100 %. Panning follows the upright
  image.

**4. Zoom, panning and prefetched image changes: the image's Bayer data is already on the GPU**
(A7R, all vertical; on screen, latency.ps1)

These results apply only when the image is already unpacked and uploaded, either because it is shown
or because it was prefetched. A new RAW image that is not prefetched is covered in point 5.

| Action | Median | Max | ≤ 30 ms |
|---|---|---|---|
| Z, fit ↔ 100 % (PPG of the visible region) | 18.2 ms | 47 ms | 14 of 20 |
| Panning at 100 % (PPG per frame) | 20.1 ms | 36 ms | 16 of 20 |
| Next image, prefetched, fit (1500 ms gap) | 22.9 ms | 44 ms | 9 of 12 |
| Next image, prefetched, 100 % with PPG (1500 ms gap) | 28.4 ms | 37 ms | 7 of 12 |

- No pre-rendered view is needed; any position and zoom is computed from the Bayer data on the GPU.
- For comparison:
  - P6 MHC took a median of about 26 ms for Z;
  - CPU Z cold took 40–46 ms to memory, before the presenter (12.2).
- In the bench, stepping with prefetch showed every image within 30 ms:
  - natural pace, 300–1500 ms per step: 25 of 25;
  - back and forth 1–3 steps, 800 ms: 12 of 12.
- Without prefetch, only 9 of 25 steps were within 30 ms, with a median of 216 ms.
- The page's own GPU times (0.3–0.7 ms) are unreliable under ANGLE (14.2) and are not used.

**5. New RAW image change, cold (not prefetched): limited by the CPU unpack**

| | Total in the page | Open + unpack (CPU) | Upload to GPU |
|---|---|---|---|
| A7R | 483–517 ms | **321–334 ms (about 330 ms)** | 91–94 ms |
| A1 | 402–404 ms | 266–268 ms | 74 ms |
| GM1 | 212–214 ms | 167–170 ms | 23 ms |
| Ricoh DNG | 588–647 ms | 522–584 ms | 35 ms |

A new, not prefetched RAW image is **not faster than the CPU path**, about 550 ms for A7R (P5 S4).
The time is dominated by LibRaw's unpack on the CPU, about 330 ms for A7R. **GPU PPG does not solve the
slow unpack of new RAW files.** GPU demosaicing removes
the CPU demosaic and half-size render, but not the unpack.

**6. Fast RAW stepping (A7R, bench, every step a new image)**

| Pace | Images shown | Of those within 30 ms |
|---|---|---|
| 4 images/s, prefetch | 7 of 20 | 1 |
| 7 images/s, prefetch | 2 of 20 | 1 |

- Fast stepping still does not keep up. About 500 ms per new A7R image allows at most about 2
  images/s, and the "latest wins" rule skips the rest.
- In RAW mode, fast stepping still needs the embedded preview, as in strategy p5 (section 12).
- The on-screen run at a 400 ms gap (median 43 ms, max 81 ms) cycled through five images that were all
  in the cache. It is optimistic and is not used as evidence.

**7. Memory: textures use both GPU and system memory**

| State | Texture cache | GPU dedicated | GPU shared | Process tree, private |
|---|---|---|---|---|
| Start | 0 | 56 MB | 3 MB | 0.2 GB |
| One A7R image | 175 MB | 259 MB | 316 MB | 0.8–0.9 GB |
| Cache full (2 GB texture budget) | 1.9–2.0 GB | 2.0–2.4 GB | 2.0–2.3 GB | **4.8–5.4 GB** |
| Back to one image | 175 MB | 422 MB | 332 MB | 1.4 GB |

- **Under ANGLE/D3D11 each texture also occupies system memory.** The "shared" counter is about the
  size of the cache, and the process tree grows by about 2.3 times the cache size.
- **A 2 GB texture cache gives about 5.4 GB in total** for the process tree [M].
- **It is unclear whether the 4 GB budget (section 12) should cover the WebView2 GPU process and its
  memory use.**
  - Section 12 set the budget for the core's caches.
  - Whether the presenter's textures, and the GPU and system memory they take in the WebView2 GPU
    process, count in the same 4 GB is not decided (15.4).
- **Preliminary estimate [T], not verified:** if the whole process tree must stay within 4 GB, the
  texture cache would need to be about **1.3–1.4 GB**.
  - The figure is (4 GB − about 0.65 GB base) / 2.3, with a margin.
  - That is room for about 7–8 A7R images.
  - It extrapolates from one run with one cache size. A cache of that size was not run, and the core's
    own caches were not loaded at the same time.
- Memory is returned when the cache shrinks.
- One A7R image takes about 175 MB in the cache: the Bayer texture plus the half-size texture.

**8. WebGL context loss** (forced, zoomed vertical image)

- The `webglcontextrestored` event arrives after 30 ms.
- Rebuilding the shaders, reloading the current image (unpack and upload) and drawing it takes 509 ms.
- **The view is back after about 0.5 s (539 ms in all), at the same zoom and position.**
- All textures are lost. The cache is filled again by prefetch as the user steps on.

### 15.3 Assessment (not a decision)

- **Quality:** on A7R, A1 and Ricoh, GPU PPG gives the same image as LibRaw PPG.
  - GM1 deviates at zero pixels. The proposed preprocessing step (15.2 point 2) is expected to remove
    that, but it is not implemented or verified.
  - Other camera models are not examined.
- **Orientation:** correct for vertical and horizontal images.
- **Performance, image already on the GPU** (shown or prefetched): better than the CPU path for Z,
  panning and image changes, without any pre-rendered zoom view.
- **Performance, new RAW image (cold) and fast stepping:** no better than the CPU path. The unpack of
  about 330 ms per A7R limits, and GPU PPG does not solve that.
- **Memory:** this is the main obstacle.
  - The texture budget must be set from measured total use, not from texture bytes.
  - Whether the 4 GB covers the WebView2 GPU process is undecided.
- **Robustness:** context loss is handled. DPI change, moving to another screen and GPU device loss
  are still postponed by the owner (13.3).

### 15.4 Open questions before a production decision

1. **Dead-pixel correction before GPU processing:**
   - implement the proposed preprocessing step of 15.2 point 2;
   - verify that GM1 then matches LibRaw PPG like the other cameras;
   - examine other camera models for LibRaw corrections made before demosaicing.
2. **How system and GPU memory count in the common 4 GB budget:**
   - It is unclear whether the budget covers the WebView2 GPU process and its memory use, both GPU
     memory and system memory.
   - How are the core's caches (section 12) and the texture cache divided?
   - Can GPU memory be measured reliably in the app (PDH)?
3. **A smaller texture cache:** the preliminary 1.3–1.4 GB is not verified.
   - Measure the total use at that cache size.
   - Measure how it affects P5 hit rates and stepping performance, especially back and forth and
     natural stepping with prefetch.
4. **Faster CPU unpack without lower image quality:** for example, unpacking in parallel with the
   previous image's upload, prefetching more images, or a faster unpack per format. Any route must
   give the same Bayer data as today's LibRaw unpack.

Rules that still apply:

- no new dependencies without approval;
- the PPG shader enters the app only after an owner decision;
- requirements V1–V8 and G1–G4 are unchanged.

## 16. Amendment 6: GUI in pure Rust with eframe + egui (owner decision, 2026-11-07)

### 16.1 Decision

- **The GUI is built in Rust with eframe + egui** (`docs/GUI_REQUIREMENTS.md`). That covers the loupe
  and image presentation as well as all other UI.
- **No Tauri, WebView2, HTML, CSS or JavaScript** in the new GUI.
- **GPU use only through eframe's rendering backend.** No separate wgpu device, surface or compute
  pipeline is built for the UI.

### 16.2 What it supersedes

| Earlier | Now |
|---|---|
| Section 3: options A, B and C on a Tauri/WebView2 shell; C3 (native UI) rejected | A native Rust UI (eframe + egui) is chosen |
| Section 7, points 2–3: choose A or B2; B1 as the fallback | The egui GUI is the presenter. The current Tauri app is the fallback until the egui GUI passes acceptance (GUI_REQUIREMENTS.md, GR8) |
| Section 9.7: GPU RAW processing later through wgpu or WebGPU compute shaders | Not planned |
| Section 13: presenter B2-res (WebView2 shared buffer into WebGL2) | Superseded. Images are egui textures in our own process. No shared buffers or IPC |
| Sections 14–15: GPU PPG as a WebGL2 shader | Not carried over as is (16.4) |

### 16.3 What stays

- **The core (section 2):**
  - the view model (V6);
  - the decoded-image and tile caches;
  - prefetch and scheduling;
  - the swap rule (V2), including the 1 s loading indicator.
- **PPG as the default demosaic method, with AHD as the quality reference** (section 11).
- **Strategy p5 and the 4 GB budget** (section 12).
- **V1–V8 and G1–G4.** They are the acceptance criteria for the egui GUI, measured on screen with
  `latency.ps1`.
- **All measurements** in sections 9–15 and in the analyses, as a record of what was measured where.
  - The phase 2 finding still applies: prepared images resident on the GPU are what make a swap fast.
  - P0, P4b, P4 and P5 measure the core and do not depend on the GUI.

### 16.4 Consequences and open questions

1. **Image presentation in egui.** Prepared views are uploaded as egui textures and kept resident within
   a budget, so that an image change or Z is a draw only. Whether egui textures alone reach the 30 ms
   targets, or a custom paint step in eframe's renderer is needed, must be measured [P].
2. **eframe rendering backend** (OpenGL through glow, or wgpu): open [T]. To be chosen after a measured
   comparison on the Arc B580.
3. **GPU PPG.** The WebGL2 shader of P6b/P6c is not usable in egui as is. CPU PPG stays the decided path.
   Moving a demosaic shader into eframe's renderer would be a display shader, not a compute pipeline.
   It needs its own owner decision.
4. **Memory budget.** Under eframe the textures belong to our own process, so they can be counted in
   the same budget as the core's caches. The P6c figures (WebView2 GPU process, ANGLE: 2.33× RAM per
   texture byte) do not apply. Texture memory must be measured again.
5. **Robustness** (resize, DPI change, another screen, GPU device loss) is part of acceptance for the
   GUI (GUI_REQUIREMENTS.md, GR7).
6. **Dependencies.** eframe, egui and any helper crates need the owner's approval of the concrete
   versions and a licence check before they are added.
7. **Migration.** The current Tauri app is not broken. It is replaced only when the egui GUI covers its
   features and passes the acceptance tests.

## 17. Amendment 7: P6c results, memory budget and texture cache (2026-11-07)

**No decision.** P6c was run on 2026-11-05, before amendment 6. It measured GPU PPG in the WebView2
presenter (B2-res), which has since been superseded by the eframe + egui GUI (section 16).

- **Specific to WebView2/ANGLE:** the memory factors below. They describe the WebView2 GPU process, not
  the egui GUI.
- **Carried over:** the findings on cache size, hit rates, eviction and measurement method, as
  questions to measure again under eframe.
- **Unchanged:** PPG/AHD (section 11), strategy p5 and the 4 GB budget (section 12), the P5 results,
  and V1–V8 and G1–G4.

### 17.1 What P6c did

- **Prototype:** `prototypes/phase3/p6c-gpu-memory`, commit `40903c0`. Built on P6b, with no new
  dependencies.
- **Raw data:** `D:\PhotoWorkflowData\analysis\phase3\p6c\`. Results:
  `claude/phase3-p6c-results.md` (project).
- **Memory in four layers, never added up blindly:**
  - L1, own accounting;
  - L2, private bytes per process, with WebView2 process kinds from `GetProcessInfos`;
  - L3, PDH "GPU Process Memory" and "GPU Adapter Memory";
  - L4, system commit, used only as a control.
- **The texture cache** used the P5 rules:
  - a byte budget;
  - the current image always loads;
  - prefetch of +1, +2 and −1 after 150 ms, only if it fits;
  - LRU eviction.
- **Variants:**
  - U1–U4: upload as one call, through a pixel unpack buffer, in strips, and with a texture pool;
  - H0: no half-size texture;
  - K2: unpacked data kept in a CPU tier.
- **Scenarios:** the P5 scenarios (S1, S2, S4, S6, S5, NAT and MIX in RAW mode), with P5's positions
  and random sequence; an overlap sweep (M0); and stress tests M1–M5 with frame hashes.
- **Size:** 22 configurations, each in a fresh process. All finished without errors.

### 17.2 Results [M]

**1. Measurement method (carries over to any GPU process).**

- The RAM cost of GPU textures shows up as private bytes of the process that owns the GPU context. The
  change in system commit equals the change in the private bytes of the process tree.
- Shared GPU memory (PDH) is part of those private bytes and must not be added to them.
- Dedicated GPU memory (VRAM) is not part of RAM and is reported separately.
- Allocating a texture without data costs nothing. Memory is committed on upload, and released about
  10 s after deletion.

**2. Memory per texture in WebView2/ANGLE (WebView2-specific).**

| Upload | RAM per MB of texture | Shared GPU memory | VRAM |
|---|---|---|---|
| U1 `texSubImage2D` | 2.33 × | 1.08 × | 1.03 × |
| U2 pixel unpack buffer | 1.40 × | 0.00 × | 1.09 × |

With A7R and U1, the whole process tree peaked at:

| Texture cache | Peak RAM |
|---|---|
| 768 MB | 3.0 GB |
| 1024 MB | 3.3 GB |
| 1300 MB | 4.0 GB |
| 1400 MB | 4.4 GB |
| 2048 MB | 5.3 GB |
| 1400 MB with U2 | 3.4 GB |

**3. Hit rate against P5** (A7R, share within 30 ms). The cause, one image's size in the cache, is not
specific to WebView2.

| Run | S1, S5, S6, NAT | MIX steps | **MIX jumps ±3** | S2 4–10/s | Z right after the first view |
|---|---|---|---|---|---|
| P5, 4 GB | 100 % | 94 % | **88 %** | 10 % | 0 % (43–46 ms) |
| P5, 2 GB | 94–100 % | 91 % | **35 %** | 11 % | 0 % |
| GPU cache 2048 MB (11 images) | 100 % | 96 % | **76 %** | 10 % | 100 % |
| GPU cache 1300–1400 MB (7–8 images) | 100 % | 96 % | **71 %** | 10 % | 100 % |
| GPU cache 1024 MB (5 images) | 97–100 % | 91 % | **35 %** | 10 % | 100 % |
| GPU cache 768 MB (4 images) | 71–100 % | 76 % | **29 %** | 10 % | 100 % |

- **Smaller caches miss on jumps.** One A7R image takes 175 MB as Bayer plus half-size texture, against
  about 60 MB as P5 views, so a smaller GPU cache holds fewer images of the 12-image MIX window. Every
  such miss was classed "evicted".
- **Without the half-size texture (H0)** the same cache holds 11 images instead of 8, and jumps rise to
  76 %.
- **A miss costs about 480 ms** (unpack + upload). With K2 it costs about 190–210 ms, but the CPU tier
  takes 1.5 GB of RAM, and the hit rate is unchanged.
- **Fast stepping (S2) is unchanged at about 10 %.** The CPU unpack limits.
- **CPU:** 0.4–0.65 cores on average, against 1.2–1.7 in P5, because there is no CPU demosaicing.
- **Same pattern for A1 and RW2.**
  - A1 at 1400 MB: jumps 76 % (P5 4 GB: 82 %).
  - RW2 at 1400 MB: jumps 94 % (P5: 94 %), because one image is small.

**4. Eviction and correctness (carries over as a requirement).**

- 562 frames were checked by hash: 0 wrong images, 0 black frames, 0 GL errors.
- Zoom, evict and return: zoom and position kept, same pixels, no intermediate image. The image showed
  after about 0.6 s.
- A tiny budget at 10 images/s never exceeded the budget.
- 400 steps at the limit showed no growth over time.
- The texture pool (U4) reused textures 110 times, gave somewhat faster uploads, and worked with mixed
  image sizes.

**5. On screen (WebView2, 1400 MB).**

| Action | Median | Max |
|---|---|---|
| Z | 22.7 ms | 194 ms, the first press only |
| Pan | 21.0 ms | 184 ms, the first press only |
| Prefetched image change | 47 ms (P6b: 23 ms) | – |

The cause of the slower image change is not determined. A probable factor is P6c's PDH sampler.

### 17.3 What this means for the eframe + egui GUI (not a decision)

- **Measure again under eframe.** Texture memory per image is to be measured with the L1–L4 method, for
  both rendering backends (glow and wgpu). The WebView2 factors (2.33 × and 1.40 ×) must not be used to
  set the budget.
- **The 4 GB question is simpler under eframe.** The textures belong to our own process, so they can be
  counted in the same budget as the core's caches (GUI_REQUIREMENTS.md §4). The split must still be set
  from measurement.
- **Cache size and hit rate.** Holding the MIX window (about 12 A7R images) as GPU textures takes about
  2.1 GB with half-size textures, or about 1.4 GB without. A smaller cache costs jump hits, as measured.
  - To be weighed: textures for the current image and its neighbours only, with views or unpacked data
    kept in RAM within the core's budget (as in P5).
- **Carried over as requirements for the GUI's image cache:**
  - the eviction rules;
  - never evict the current image;
  - frame correctness under memory pressure;
  - reuse of textures of the same size.
- **Open (section 16.4):**
  - the eframe rendering backend;
  - whether egui textures alone meet the 30 ms targets;
  - GPU PPG in eframe's renderer;
  - the budget split.
