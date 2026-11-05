# prototypes/phase3/p6-gpu-demosaic

ADR 0008 **P6: GPU demosaicing in presenter B2** (ADR 0008 §13). **Not part of the app.** The crate
has its own workspace and builds into `target\prototypes`. It adds no new dependencies: WebGL2 is
part of WebView2, and LibRaw comes from the repository's vcpkg install.

## Pipeline

1. **Rust:** LibRaw opens and unpacks the file on the CPU. The visible Bayer data (16 bit) is copied
   into a WebView2 shared buffer and posted to the page, together with the parameters
   `dcraw_process` would use under the ADR 0006 settings:
   - the black levels per CFA colour;
   - the camera white-balance scale (highlight 0, clipped);
   - the camera → sRGB matrix.
2. **Page:** the Bayer data becomes an integer texture (R16UI) and stays on the GPU. The shaders then
   do black subtraction, scaling, demosaicing, the matrix and dcraw's output curve (gamma 0.45, slope
   4.5, as an 8-bit lookup table).
   - **Fit view:** half-size superpixels, like LibRaw `half_size`, drawn scaled.
   - **100 % view:** only the visible region is demosaiced, straight into the canvas.
3. **Demosaic methods:**
   - `linear` reproduces LibRaw's linear method. Its difference from LibRaw is a check of the
     pipeline.
   - `mhc` (Malvar–He–Cutler, 5×5) is the GPU candidate. It is compared with AHD, the quality
     reference, and with PPG, the CPU method.

## Running it

Use `run-p6.cmd`, which takes about 10 minutes.

- `p6-bench.json`: per file (A7R ×5, RW2 ×4, A1 ×2):
  - image change: unpack, copy, post, upload and fit render;
  - Z: GPU demosaic of the visible region;
  - quality of the GPU region against LibRaw AHD, PPG and linear on the same pixels, with the phase 1
    metrics. The crops are written to `crops\` as PPM.
- `p6-latency.csv`: on-screen latency for Z, with the raw data already on the GPU, and for RIGHT (a
  new file, unpack included).

Orientation (flip) is ignored. Everything is in sensor orientation, as in the LibRaw references.
