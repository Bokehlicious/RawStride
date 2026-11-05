# ADR 0008 decision matrix: requirements against the measured combinations

- Date: 2026-11-05
- Basis:
  - ADR 0008 §8.3 (decision matrix), §11 (PPG) and §12 (scheduling, 4 GB);
  - phase 1 (P0, P4b, P4, P5);
  - phase 2 (P2, P3) and the robustness checks by hand;
  - phase 3 (P6, P6b), ADR 0008 §14 and §15.
- Raw data: `D:\PhotoWorkflowData\analysis\phase1\`, `...\phase2\` and `...\phase3\`.
- Status: **for the owner's decision.** The matrix shows measured values against the targets. It
  does not choose.

## How to read it

- **The core is the same in every column.** It is the scheduling strategy of §12 with PPG and a 4 GB
  budget. The columns differ only in the presenter, which carries finished pixels to the screen.
- **Totals are added up.** Core values are "view ready in memory" (P4/P5). Presenter values are
  on-screen times from key press to changed pixels (latency.ps1, P2/P3). Both are medians, with p95
  where it matters. The screen is sampled every ~17 ms, so on-screen values are upper bounds.
- **Markings:**
  - **[M]** measured;
  - **[M+M]** two measured parts added;
  - **[T]** estimate from measured parts that were never run together;
  - **–** not measured.
- **Verdicts:**
  - ✓ meets the target;
  - ≈ at the limit (median at the target, p95 above);
  - ✗ misses it;
  - ? not known.
- **Columns:**
  - **B1:** today's `pw://` fetch, as in the app now.
  - **B2:** WebView2 shared buffer, filled ahead (B2-pool), drawn with WebGL2. Each image is
    uploaded to the GPU on the swap.
  - **B2-res:** like B2, but prepared views stay as WebGL textures (a VRAM cache), so the swap is a
    draw only. Measured on 2026-11-05 at 17:34 to close gap 1.
  - **A-up:** wgpu surface. Each image is uploaded from RAM to the GPU on the swap.
  - **A-res:** wgpu surface. Prepared views are already GPU textures (a VRAM cache), so the swap is
    a draw only.
- **CPU and GPU demosaicing:** every column of the main matrix uses CPU demosaicing (PPG). The rows that
  GPU demosaicing could change are marked "P6". GPU demosaicing is measured separately in the P6/P6b
  section below.

## Presenter alone, data ready in RAM (on screen, median / p95)

| | B1 | B2 | B2-res | A-up | A-res |
|---|---|---|---|---|---|
| 100 % view 2560×1440 | 145 / 152 ms | 30 / 49 ms | **13 / 44 ms** (19 of 20 ≤ 30 ms) | 30 / 42 ms | **17 / 33 ms** (19 of 20 ≤ 30 ms) |
| Whole A1 image 8640×5760, fitted | 1599 ms | 203 ms | **16 / 36 ms** (18 of 20) | 120–129 ms | **14 / 37 ms** (18 of 20; the p95 of 159 ms is the first use) |
| In the page or in Rust, until present | 119 ms | 15 ms | **0.5 ms** | 4 ms | **0.7 ms** |

The B1, B2 and B2-res rows are from the second P2 run (2026-11-05 17:34). B1 and B2 repeat the first
run within a few ms.

## Matrix

| Requirement (target) | B1 | B2 | B2-res | A-up | A-res |
|---|---|---| --- |---|---|
| **V1** next/previous ≤ 30 ms, fit and 100 %, embedded-preview formats | ✗ 146 ms for a 100 % view [M]; 1.6 s for a whole A1 [M] | ≈ 30 / 49 ms for a 100 % view [M+M; the core is 0 ms when prefetched]. ✗ 201 ms for a whole A1 at fit [M]. Smaller previews such as A7R 1616 px: in-page 13 ms [M], on screen – | ✓ 13 ms for a 100 % view, 16 ms for a whole A1 [M+M] | ≈ 30 / 42 ms [M+M]. ✗ 120–129 ms for a whole A1 [M] | ✓ 17 ms for a 100 % view, 14 ms for a whole A1 [M+M] |
| **V2** no intermediate frames, one swap (indicator after 1 s) | ✓ by design. Today's app shows blur and is ✗; the new core removes that | ✓ one change per key press [M] | ✓ one change per key press [M] | ✓ one change per key press [M] | ✓ one change per key press [M] |
| **V3** fit ↔ 100 % with a full-size embedded JPEG ≤ 30 ms, also the first time | ✗ | ≈ for a 100 % crop (30 / 49 ms); ✗ back to fit for a whole A1 (201 ms) [M] | ✓ the image is one texture, so fit and 100 % are only draws (0.5 ms in the page [M], on screen 13–16 ms [M]) | ≈ for a 100 % crop; ✗ back to fit for a whole A1 (120–129 ms) [M] | ✓ the image is one texture, so fit and 100 % are only draws (0.7 ms to present [M], on screen 14–17 ms [M]) |
| **V4** about 7 images/s at 100 %: each step ≤ 70 ms, never another image | ✗ | JPEG mode, A7R/RW2: the core serves 70–85 % of steps at 10/s within 30 ms [M], plus 30–49 ms presenter, so probably ✓ [T]. A1 JPEG ✗ (150 ms decode [M]). RAW mode ✗: the CPU does not keep up at ≥ 4/s [M] (P6) | JPEG A7R/RW2 ✓ [T]. A1 JPEG ✗ (decode). RAW ✗ (CPU) (P6) | Same as B2 [T] | JPEG A7R/RW2 ✓ [T]. A1 JPEG ✗ (decode). RAW ✗ (CPU) (P6) |
| **V5** first image of a folder ≤ 150 ms | ✗ | JPEG A7R/RW2 ✓, about 10–14 ms decode plus 30 ms [T]. JPEG A1 ✗: 150 ms decode plus 201 ms [M+M]. RAW ✗: 270–620 ms to the first view [M] | JPEG A7R/RW2 ✓ [T]. JPEG A1 ≈ ✗: 150 ms decode plus upload [T]. RAW ✗ | JPEG A7R/RW2 ✓ [T]. JPEG A1 ✗: 150 plus 125 ms [M+M]. RAW ✗ | JPEG A7R/RW2 ✓ [T]. JPEG A1 ≈ ✗: 150 ms decode plus upload [T]. RAW ✗ |
| **V6** zoom and position persist | Core (view model): the same in all columns. Designed, not yet measured | Same | Same | Same | Same |
| **V7** real, sharp 100 % from RAW ≤ 30 ms after Z, also immediately | ✗ | Prepared (S1 0/50/150 ms, S6, natural stepping: 100 % ready in memory [M]): ≈ 30 / 49 ms [M+M]. Cold: ✗ 43–46 ms plus 30 ms [M+M]. Continuous stepping at ≥ 4/s: ✗ (P6) | Prepared ✓ 13 / 44 ms [M+M] (the zoom view is uploaded ahead). Cold and fast stepping ✗ (P6) | Prepared ≈ 30 / 42 ms [M+M]. Cold and fast stepping ✗ (P6) | Prepared ✓ 17 / 33 ms [M+M] (the zoom view is uploaded ahead). Cold and fast stepping ✗ (P6) |
| **V8** a configurable memory budget that is never exceeded | Core: 4 GB held exactly; the process adds 0.2–0.3 GB [M] | Core ✓. The presenter's memory is not budgeted: the WebView2 processes grew to 2.2 GB in P2 [M]. Shared-buffer lifetime is not measurable with private bytes | Core ✓. The texture cache lives in the WebView2 GPU process and needs a budget of its own: P2 grew to 3.0 GB in all [M]. VRAM was not measured | Core ✓. The presenter's memory is not budgeted. VRAM was not measured (Vulkan) | Core ✓. A VRAM cache needs a budget of its own. VRAM was not measured |
| **G1** honest source | UI in the webview: the same in all columns | Same | Same | Same | Same |
| **G2** responsiveness | Fetch is asynchronous | Post on the UI thread takes 0.1 ms [M]; the upload runs in the page | Same as B2 | Rendering runs off the UI thread [M] | Same as A-up |
| **G3** damaged files | Core: the same in all columns (M2 behaviour) | Same | Same | Same | Same |
| **G4** two display modes, chosen by hand | Core: switching served from the cache in most cases (P5 S3) [M]. The presenter cost is that of one swap | Same plus a swap | Same plus a draw | Same plus a swap | Same plus a draw |

## Integration and risk (P2/P3 and the checks by hand)

| | B1 / B2 (WebView2) | A (wgpu) |
|---|---|---|
| New dependencies | None. B2 uses webview2-com and windows-core, which are already in the tree | wgpu, approved for the prototype only. Tauri's `unstable` feature (child webviews) |
| Platform | B2 is a WebView2 API, so Windows only. The app is Windows only | Cross-platform library. wgpu chose **Vulkan**; DX12 is untested |
| Focus and keyboard | ✓ also after minimise and restore [M] | ✗ keys lost after a restore from the taskbar until the user clicks [M]. Small fix: give the webview focus on `Focused(true)` |
| Resize and move | ✓ (the prototype only scales the canvas) [M] | ✓ (the prototype redraws only on the next swap) [M] |
| Transparent webview over the image | – | ✓ works and is as fast as the strip layout [M] |
| DPI change, another screen, GPU device loss | – (postponed by the owner) | – (postponed by the owner) |
| Code that must be written | WebGL2 drawing in the page; Rust fills shared buffers | Native GPU presenter, window and webview layering, focus, resize and DPI handling |

## Gaps that may affect the choice

1. ~~GPU-resident textures in B2 are not measured.~~ **Closed** on 2026-11-05: B2-res is as fast as
   A-res (100 % view 13 against 17 ms in the median; whole A1 16 against 14 ms; 18–19 of 20 swaps
   within 30 ms in both).
2. **A fit-sized rendition for the A1** (for example libjpeg-turbo scaled to 1/4, as in P0) would
   make the fit swap a 1920 px upload instead of 199 MB in B2 and A-up. That is not measured on screen.
   libjpeg-turbo in the app needs approval.
3. **P6 and P6b (GPU demosaicing)** are measured; see "P6 and P6b" below and ADR 0008 §14–§15.
   - GPU PPG gives the same image as LibRaw PPG.
   - When the image is already on the GPU (shown or prefetched), it helps zoom, panning and image
     changes without a pre-rendered view.
   - It does not help a cold change to a new RAW file or fast stepping. The CPU unpack, about 330 ms
     for A7R, limits.
4. **VRAM and presenter memory under the V8 budget:** measured for B2-res with GPU textures in P6b.
   - Textures also take system memory.
   - A 2 GB texture cache gives about 5.4 GB in total.
   - It is unclear whether the 4 GB budget covers the WebView2 GPU process and its memory use (ADR
     0008 §15.4).
   - Not measured for the other presenters.
5. **The three robustness points** (DPI change, another screen, GPU device loss) are postponed by the
   owner.

## P6 and P6b: GPU demosaicing inside B2-res (added 2026-11-05, ADR 0008 §14; P6b column added 2026-11-05, §15)

The presenter is B2-res in all columns below. The columns differ only in where demosaicing runs and
with which method. The P6b times are on screen, A7R (vertical), latency.ps1, median / max.

| Requirement (target) | CPU PPG, prepared views (§12) | GPU demosaic, Bayer data on the GPU (P6, MHC) | GPU PPG, Bayer data on the GPU (P6b) |
|---|---|---|---|
| **V3 / V7** Z ≤ 30 ms, prepared | ✓ 13 / 44 ms (pre-rendered view) [M] | ✓ for any position, no pre-render needed: on screen 12–33 ms, median about 26, 18 of 20 within 30 ms [M] | ✓ 18 / 47 ms, 14 of 20 within 30 ms, any position, no pre-render [M] |
| **V7** Z cold or at a new position | ✗ 40–46 ms to memory, plus the presenter [M] | ✓ / ≈ as above, once the Bayer data is on the GPU [M] | New position in an image already on the GPU: ✓ / ≈ as above [M]. A new RAW image that is not prefetched has to be unpacked first: ✗, see the next row |
| **V5 / V4** cold start, image change (RAW) | ✗ about 550 ms to the first view (A7R) [M] | ✗ 533–557 ms on screen (A7R); unpack dominates [M] | ✗ 483–517 ms in the page (A7R); unpack about 330 ms dominates. GPU PPG does not solve this [M] |
| Fast stepping (RAW) | ✗ the CPU does not keep up at ≥ 4/s [M] | ? CPU work per image is unpack only, plus about 110 ms of copy and upload; not measured | ✗ 4/s: 7 of 20 images shown; 7/s: 2 of 20 [M]. With prefetch at a natural pace: 25 of 25 within 30 ms [M] |
| Quality against AHD | PPG: ΔE00 1.03 (A7R), zipper 0.9 [M] | MHC: ΔE00 1.60 (A7R), zipper 1.2–2.2. **Below PPG** [M]. P6b: see the next column | = LibRaw PPG: ≥ 99.98 % identical pixels, ΔE00 ≈ 0 on A7R, A1 and Ricoh; the same distance to AHD as CPU PPG. GM1 deviates at dead (zero) pixels that LibRaw corrects. A preprocessing step before upload is proposed, not implemented or verified. Other camera models are not examined [M] |
| **V8** memory | 4 GB core budget [M] | Plus about 122 MB of Bayer texture per A7R image on the GPU. VRAM is not measurable in WebGL2 | ✗ / ? textures also use system memory: a 2 GB texture cache gives 5.4 GB in total. Whether the 4 GB budget covers the WebView2 GPU process is unclear. If it does, about 1.3–1.4 GB of cache would fit: a preliminary, unverified estimate [M]/[T] |
| Orientation | P4/P5 swap the view for flip 5/6 [M] | Demosaicing in sensor orientation, correct; rotation for display missing in the prototype (§14.3) | ✓ rotation for display and screen-to-sensor mapping; vertical and horizontal equal to LibRaw with `user_flip = -1` [M] |
| Panning at 100 % | – (pre-rendered region only) | – | ✓ / ≈ 20 / 36 ms, 16 of 20 within 30 ms [M] |
| Image change, prefetched (image already on the GPU) | ✓ 13 / 44 ms (B2-res, §13) [M] | – | ✓ / ≈ fit 23 / 44 ms; 100 % with PPG 28 / 37 ms [M]. Applies only to prefetched images; cold changes are in the V5/V4 row |
| WebGL context loss | – | – | ✓ view back after about 0.5 s at the same zoom and position [M] |

## Summary of what the matrix shows (not a decision)

- **B1 (today) misses V1, V3, V5 and V7** in every case. Leaving it is settled by the measurements
  alone.
- **B2 and A-up are about equal.** Both are at the limit for 100 % views (median 30 ms, p95 42–49 ms)
  and too slow for whole large images at fit.
- **Only the two variants with a VRAM texture cache, B2-res and A-res, meet V1, V3 and prepared V7.**
  They are equally fast: medians of 13–17 ms, with 18–19 of 20 swaps within 30 ms. The difference
  between B2 and A is therefore not speed but integration (see the table above). B2 needs no new
  dependencies and had no focus problem. A needs wgpu, Tauri's `unstable` feature and a focus fix.
- **Upload-on-swap variants (B2, A-up) are at the limit**, at 30 ms in the median, and too slow for
  whole large images.
- **No presenter fixes cold start or fast stepping in RAW mode.** P6 and P6b confirm that the CPU
  unpack is the limit, about 330 ms for A7R.
- **GPU PPG (P6b) keeps the PPG quality on the cameras tested**, except at GM1 zero pixels. When
  the image is already on the GPU, it makes zoom, panning and prefetched image changes fast without
  pre-rendering.
- **It does not solve the slow unpack of new RAW files.**
  - It is a promising candidate for production, but there is no production decision.
  - Open (ADR 0008 §15.4):
    - dead-pixel correction before upload: a proposed step, not implemented or verified; other
      camera models must be examined too;
    - whether the 4 GB budget covers the WebView2 GPU process and its memory use (unclear);
    - the effect of a smaller texture cache (preliminary, unverified estimate about 1.3–1.4 GB) on hit
      rates;
    - a faster unpack without lower quality.
