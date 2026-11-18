# 2026-11-18 – Image decoding: would the GPU make it faster?

- Module: `pw-libraw`, `pw-libjpeg`, `pw-render`, `pw-viewcore`
- Question: the CPU in thanos is old (i7-9700K) and the graphics card is fairly new (Arc B580). Would
  decoding images on the GPU make RawStride faster?
- Kind: a discussion with the owner, written down so it can be picked up later. **No new measurement.**
  Every figure below comes from the earlier measurements it cites.
- Machine: thanos (i7-9700K, 8 cores without HT, 32 GB; Arc B580)
- Status: no decision and no task. The owner has no slow situation to fix at the moment.

## Where decoding runs today

- **RAW:** LibRaw 0.22.3 with OpenMP and PPG demosaicing on the CPU (`pw-libraw`, `pw-raw`).
- **JPEG:** libjpeg-turbo (TurboJPEG 3) on the CPU (`pw-libjpeg`).
- **TIFF and colour:** on the CPU (`pw-tiff`, `pw-colour`).
- **The GPU only presents.** `pw-render` uploads finished views as egui textures, and eframe's glow
  renderer draws them with linear filtering and mipmaps. The GPU's largest texture side is a limit:
  a larger view is rejected and shown as an error.
- GPU demosaicing exists only in the prototypes `prototypes/phase3/p6-gpu-demosaic`, `p6b-gpu-ppg` and
  `p6c-gpu-memory` (WebGL2 shaders in the WebView2 presenter). They are not part of the app.
  `docs/GUI_REQUIREMENTS.md` (the "GPU PPG" row) keeps CPU PPG as the decided path and leaves open
  whether a GPU demosaic step is ever moved into eframe's renderer, as a display shader (GR4).

## Measured earlier

| What | CPU PPG (today) | GPU PPG (P6b) | Source |
|---|---|---|---|
| New RAW image to first view (A7R) | about 550 ms | 483–517 ms | decision matrix, V5/V4 row [M] |
| Of which RAW unpack (CPU in both) | about 330 ms | about 330 ms | the same [M] |
| Fast stepping, 4 images/s | the CPU does not keep up | 7 of 20 shown | the same [M] |
| Zoom to 100 % at any position | only if pre-rendered | 18 / 47 ms, no pre-render | the same, V3/V7 row [M] |
| Panning at 100 % | pre-rendered region only | 20 / 36 ms | the same [M] |
| Quality | LibRaw PPG | ≥ 99.98 % identical pixels to LibRaw PPG; GM1 differs at dead pixels | the same [M] |

Source: [2026-11-05-adr0008-decision-matrix.md](2026-11-05-adr0008-decision-matrix.md), section "P6 and
P6b". Background: ADR 0008 §2.3 (pre-rendering and prefetch) and §14–§15 (GPU demosaicing).

## Interpretation

1. **The GPU does not remove the main cost of a new RAW image.** *Inference from the measurements.*
   Unpacking is the decompression of the RAW file's sensor data. It is serial work that suits a CPU, not
   a GPU, and it runs before demosaicing wherever demosaicing runs. Moving PPG to the GPU saves about
   40–60 ms of about 550 ms.
2. **JPEG would not get faster either.** *Inference.* libjpeg-turbo is already fast, and the costly part
   of JPEG decoding (entropy decoding) is serial too.
3. **The GPU helps where the image is already unpacked.** *Measured in P6b.* Zoom to 100 % at any
   position and panning become fast without pre-rendering.
4. **An old CPU may gain more from GPU PPG than a new one.** *Hypothesis.* With PPG off the CPU, more CPU
   time is left for unpacking the next image ahead. Confirm by measuring stepping with prefetch in the
   real app, with CPU PPG and with GPU PPG on the same machine. With prefetch at a natural pace, P6b
   showed 25 of 25 images within 30 ms; CPU PPG was not measured in the same scenario.
5. **The owner notices no slow situation today.** *Observation by the owner, 2026-11-18.* This suggests
   that prefetch and pre-rendered views (ADR 0008 §2.3) already hide most of the cost at the owner's
   normal pace.

## Options

| Option | New RAW image | Fast stepping | Zoom and panning at 100 % | Cost |
|---|---|---|---|---|
| Tune prefetch (dwell, how far ahead) | hidden at a calm pace | a little | – | small |
| Faster unpack without lower quality | yes, the only option that helps here | yes | – | unknown, needs profiling |
| GPU PPG as a display shader in glow | marginal (40–60 ms) | a little (CPU time freed) | yes, clearly | large: dead-pixel correction before upload, more camera models checked, a texture-memory budget (ADR 0008 §15.4) |
| Show the embedded JPEG preview first | feels immediate | yes | – | small; not checked whether the loupe already does this |
| A newer CPU | yes, unpack is CPU-bound | yes | a little | money, no code |

## Follow-up

The recommendation from the discussion, in this order, **only when a slow situation appears**:

1. Name the situation: opening a new image, fast stepping, or zoom and panning at 100 %.
2. Measure prefetch in the real app: is the next image unpacked when it is reached at a normal pace?
   If not, tune the dwell or how far ahead it reaches.
3. Profile LibRaw's unpack for the owner's cameras and look for a faster setting or version without
   lower quality (open point in ADR 0008 §15.4).
4. Only if zoom and panning at 100 % are still too slow: GPU PPG in glow, starting from P6b, which
   already showed the same image as LibRaw PPG.
