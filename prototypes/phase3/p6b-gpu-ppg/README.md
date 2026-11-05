# prototypes/phase3/p6b-gpu-ppg

ADR 0008 **P6b: PPG demosaicing on the GPU** in presenter B2-res (ADR 0008 §13, plan in §14.5).
**Not part of the app.** The crate has its own workspace and builds into `target\prototypes`. It
adds no new dependencies: WebGL2 is part of WebView2, LibRaw comes from the repository's vcpkg
install, and PDH (GPU memory counters) is part of Windows.

PPG is the decided default method (ADR 0008 §11). P6b shows whether a GPU version reaches the same
quality as LibRaw PPG with better performance. The shader must not enter the app before the owner has
reviewed and approved the results.

## Pipeline

1. **Rust:** LibRaw opens and unpacks the file on the CPU. The visible Bayer data (16 bit) goes to
   the page through a WebView2 shared buffer, with black levels, white-balance scale, camera → sRGB
   matrix and the file's flip (the same parameters as P6).
2. **Page:** the Bayer data stays on the GPU as an R16UI texture (LRU cache, 2 GB budget inside the
   4 GB total of §12).
   - **Fit view:** half-size superpixels, rotated by flip.
   - **100 % view:** the visible region is demosaiced with PPG in two passes:
     1. green at R/B sites (dcraw `ppg_interpolate` pass 1) into an R16UI target;
     2. R/B at G sites and at R/B sites (passes 2 and 3), black, scale, matrix and the output curve.
   - Each screen pixel maps to the upright image and then, through flip 3/5/6, to sensor
     coordinates. Vertical and 180° images are displayed upright.
3. **References:** `p6_ref` renders LibRaw PPG (`user_qual` 2) and AHD (3) for the same sensor rect,
   either in sensor orientation (`user_flip` 0) or oriented (`user_flip` -1).

## Running it

Use `run-p6b.cmd`, which takes about 15 minutes. Don't use the computer while it runs.

- `p6b-bench.json`:
  - per file (A7R ×5, RW2 ×4, A1 ×2, Ricoh GR III DNG ×2): 3 cold image changes, 10 zooms and
    10 pans;
  - quality in three modes: `linear` and `ppg` in sensor orientation, and `ppg-oriented`. Each is
    measured against LibRaw PPG and AHD (ΔE00, SSIM, PSNR, zipper, false colour, identical share,
    max channel difference);
  - stepping on the A7R files:
    - natural pace, with and without prefetch;
    - back and forth;
    - continuous at 4/s and 7/s;
  - process-tree memory and GPU memory (dedicated and shared, PDH);
  - WebGL context loss: restore and reload times.
- `p6b-latency.csv` (latency.ps1, on screen):
  - Z;
  - RIGHT at 1500 ms and at 400 ms gaps;
  - pan D at 100 %, and RIGHT while zoomed.
- `crops\`: PPM crops for visual comparison.

Results go to `D:\PhotoWorkflowData\analysis\phase3\p6b\`.
