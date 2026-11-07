# prototypes/phase3/p6c-gpu-memory

ADR 0008 **P6c: GPU PPG memory budget and texture-cache validation**, in presenter B2-res. This is a
prototype and **not part of the app**.

- The crate has its own workspace and builds into `target\prototypes`.
- It adds no new dependencies. WebGL2 and the WebView2 process list come with WebView2, PDH comes with
  Windows, and LibRaw comes from the repository's vcpkg install.
- The PPG shader, the LibRaw shim and the orientation mapping are copied unchanged from P6b.

P6c measures. It decides nothing about the 4 GB budget (ADR 0008 §12) or about GPU PPG in the app.

## What it measures

**Memory, in four layers that are never added up blindly:**

| Layer | What | Source |
|---|---|---|
| L1 | Bytes the core and the page account for: CPU tier (K2), unpacks in progress, shared buffers, textures, pool | Own accounting |
| L2 | Private bytes and working set per process: the app, and WebView2's browser, renderer, GPU and utility processes | `GetProcessInfos`, `K32GetProcessMemoryInfo` |
| L3 | GPU memory (dedicated, shared, committed) per process and per adapter | PDH "GPU Process Memory", "GPU Adapter Memory" |
| L4 | System commit and available memory | PDH "Memory"; used only to check for double counting |

A sampler records all layers every 250 ms (`<tag>-mem.csv`). The page adds markers and settled
snapshots.

**The texture cache in the page** uses a byte budget for the Bayer R16UI texture plus the half-size
RGBA8 texture of each image, and follows the P5 rules:

- The current image always loads.
- Prefetch covers +1 and +2 in the direction of travel and −1 behind, after a 150 ms dwell. It starts
  only if it fits after evicting images the plan does not want.
- Eviction is least recently used first, and never evicts the current image.
- Two load slots stand in for P5's two workers.

**Variants:**

| Code | Variant |
|---|---|
| U1 | One `texSubImage2D` |
| U2 | Pixel unpack buffer |
| U3 | 16 strips |
| U4 | Texture pool: textures of evicted images are reused for an image of the same size |
| H0 | No stored half-size texture; the fit view is drawn straight from the Bayer texture |
| K1 | The unpacked data is dropped after upload |
| K2 | The unpacked data is kept in a CPU tier (`--tier <MB>`), so an evicted image is uploaded again without a new unpack |

**Suites:**

| Suite | What it runs |
|---|---|
| `m0` | Textures of known size, allocated, uploaded and deleted in steps, to show what each counter counts |
| `p5` | The P5 scenarios in RAW mode (S1, S2, S4, S6, S5, NAT, MIX), with the same pauses, positions and random sequence, in P5's event format plus the draw time and the miss cause |
| `natmix` | NAT and MIX only, at the same positions |
| `stress` | M1 long stepping at the limit; M2 zoom, evict, return; M3 a tiny budget at 10 images/s; M4 late loads; M5 400 steps for drift and release. Every drawn frame is checked by a hash of 64×64 pixels, plus black-frame and GL-error checks |
| `interactive` | For latency.ps1 |

## Running it

Run `run-p6c.cmd [A|B|C|all]`. Results go to `D:\PhotoWorkflowData\analysis\phase3\p6c\`. Don't use the
computer while it runs.

| Part | Contents | Time |
|---|---|---|
| A | M0, the variants, stress, mixed sizes | about 40 min |
| B | P5 comparison at 2048 / 1400 / 1300 / 1024 / 768 MB (A7R), K2, A1 and RW2 | about 60 min |
| C | On-screen latency | about 6 min |

## Known limits

- The page's GPU times are unreliable under ANGLE. Only the on-screen times (part C) are real
  presentation times.
- P5 measured "ready in memory". P6c logs "ready", meaning resident on the GPU, and "drawn", meaning
  the next animation frame after drawing.
- JPEG mode and S3 are not run, because GPU PPG concerns RAW mode only. Their positions are kept, so
  NAT and MIX use the same images as in P5. MIX logs its mode switches, but the page stays in RAW mode.
- The green scratch texture of the PPG pass, about 4 MB, is not in the budget.
