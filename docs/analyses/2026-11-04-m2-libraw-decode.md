# 2026-11-04 – pw-raw (slow path): LibRaw decode time, memory and parallelism

- Module: `pw-raw` (LibRaw full decode), `pw-render` (100 % zoom)
- Question (M2): How long does a RAW decode take for the A7R IVA (primary) and the A1
  (comparison): full image, reduced size, first usable RAW pixels, memory, parallel decoding?
  Should the slow path decode whole images, tiles or a combination?
- Build: `tools/raw-decode-bench` (C shim → LibRaw), profile `profiling`, MSVC 14.51.
- **LibRaw 0.22.3-Release** from vcpkg (`x64-windows-static-md`), thread-safe `raw_r.lib`, linked
  statically, `LIBRAW_NODLL`. No default features: **no OpenMP** (single-threaded per image),
  with lcms2, jasper, libjpeg-turbo and zlib (`zs.lib`). Capabilities `0x40`.
- **Licence:** LibRaw is dual-licensed **LGPL-2.1 or CDDL-1.0**. The choice for distribution is
  postponed (ADR 0002). Static linking under LGPL has relinking obligations; CDDL does not.
- **Note (2026-11-19, T216):** the `tools\m2-*.cmd` scripts named below were removed; `tools\raw-decode-bench\README.md` shows how to run the same PowerShell scripts.
- Machine: thanos (i7-9700K, 8 cores without HT, 32 GB). Test files on D: (SATA SSD).
  Warm file cache (each configuration run 3 times; median shown, spread < 2 %).
- Settings: 8-bit output, camera white balance defaults, `user_qual` 0 = linear, 3 = AHD.
- Raw data: `D:\PhotoWorkflowData\analysis\raw-decode-20261104-062336\` (outside the repo).

## Measured / observed

### One image, one thread (ms)

| Camera, file | Configuration | Unpack | Demosaic etc. | To RGB | **Total** | Output | Peak memory |
|---|---|---|---|---|---|---|---|
| A7R IVA, TN100423 (61 MP) | unpack only | 294 | – | – | **295** | – | 122 MB |
| | half size | 289 | 363 | 104 | **755** | 4784 × 3188 | 282 MB |
| | full, linear | 293 | 1 598 | 536 | **2 427** | 9568 × 6376 | 763 MB |
| | full, AHD | 290 | 5 565 | 528 | **6 387** | 9568 × 6376 | 763 MB |
| | 2048² crop (one tile), AHD | 294 | 382 | 66 | **743** | 2048 × 2048 | 166 MB |
| A1, TN005956 (50 MP) | unpack only | 233 | – | – | **233** | – | 101 MB |
| | half size | 234 | 301 | 32 | **570** | 4330 × 2892 | 233 MB |
| | full, linear | 237 | 1 309 | 124 | **1 672** | 8660 × 5784 | 628 MB |
| | full, AHD | 240 | 4 584 | 124 | **4 944** | 8660 × 5784 | 628 MB |
| | 2048² crop (one tile), AHD | 234 | 383 | 11 | **629** | 2048 × 2048 | 145 MB |

CPU time ≈ wall time in every row (cpu/wall 1.00–1.02): each decode uses one core.

### Parallel decoding, A7R IVA full AHD

| Threads | Wall time per image | Images/s | Peak private memory |
|---|---|---|---|
| 1 | 6.35 s | 0.16 | 0.76 GB |
| 2 | 6.54 s | 0.30 | 1.52 GB |
| 4 | 7.01 s | 0.56 | 3.04 GB |
| 8 | 8.46 s | 0.92 | 6.08 GB |

## Interpretation

1. **A full AHD decode is far too slow for culling** (6.4 s for A7R IVA, 4.9 s for A1), and needs
   ~0.76 GB per image. *Inference.* The embedded JPEG stays the culling path (ADR 0003).
2. **Unpacking is a fixed cost of ~0.3 s** (A7R IVA) and is not reduced by cropping.
   Demosaicing scales with the pixels processed: AHD ≈ 90 ns/pixel
   (61 MP → 5.6 s; 2048² → 0.38 s), linear ≈ 26 ns/pixel. *Inference from the numbers above.*
3. **First usable RAW pixels:** about **0.75 s** for a half-size image (4784 px long edge, more
   than any screen) or for one 2048² AHD tile. *Measured.*
4. **Parallel decoding scales almost linearly** (5.8× at 8 threads), but each decoder costs its
   own ~0.76 GB, and per-image latency grows by a third at 8 threads (memory bandwidth).
   *Measured.* For interactive work, 2–3 decoders in parallel is the sensible limit, which leaves
   cores for the UI.
5. **Where RAW decoding is actually needed:** the A1 already has an 8640 × 5760 embedded JPEG
   (full resolution), but the A7R IVA only has a 1616 × 1080 preview. So the A7R IVA needs RAW
   pixels for every zoom above ~25 % (and for a sharp fit view on a 4K screen). *Inference from
   the preview survey (2026-11-03).*

## Recommendation: a combination

| Need | Method | Cost (A7R IVA) |
|---|---|---|
| Culling, grid, switching images | embedded JPEG (unchanged) | ~2 ms |
| Sharp fit view, zoom up to ~50 % | **half-size decode** in the background for the current image and its neighbours, 2–3 decoders | ~0.75 s, 0.28 GB each |
| 100 % zoom and above | **tiles**: unpack once and keep the raw data (~120 MB) for the current image, then demosaic only the visible region with AHD (linear first if it is faster to show) | 0.3 s once + ~0.4 s per 2048² tile |
| Export / full-quality image | whole-image AHD, in the background | 6.4 s, 0.76 GB |

Not recommended: decoding the whole image at full resolution for viewing. It costs 6.4 s and
0.76 GB to show a region that is at most 4 MP on screen.

## Follow-up (needs the owner's go-ahead before changing the architecture)

1. **Repeated processing after one unpack.** LibRaw allows `dcraw_process` to be called again with
   a different crop box after one `unpack`. Measure tile 2, 3 … on the same instance, to confirm
   the 0.3 s is paid only once per image. This is a small bench change.
2. **OpenMP build** (`vcpkg` LibRaw feature `openmp`). This would let one image's AHD use all
   8 cores, which might bring a full AHD under 1.5 s. It changes an installed package, so it needs
   approval. Measure it against our own tile-level parallelism.
3. **GPU demosaicing** (Arc B580, `pw-render`) is the long-term option for 100 % zoom. It is not
   needed to make M3/M4 usable.

---

## Follow-up 1: several regions after one shared unpack (2026-11-04, baseline LibRaw)

- Approved by the owner together with the recommendation (half-size in the background, tiles on
  demand at 100 % and above).
- Bench `tiles` (`tools\m2-tiles.cmd`): one `open` + `unpack`, then 6 regions at different
  places, then the first region again. 3 runs per configuration, each in a fresh process. Same
  files and build as above (baseline LibRaw, ABI `e9e50a2e…`, no OpenMP).
- Raw data: `D:\PhotoWorkflowData\analysis\raw-tiles-20261104-064152\`.
- **Note:** this run measured unpack at ~320 ms (A7R IVA) against 294 ms in the first run, and
  half-size at 0.86 s against 0.76 s. The machine was ~10 % slower this time. Compare within one
  run, not across runs.

### Correctness

Re-processing the first region after 6 other regions gave **identical pixels** (FNV hash) in all
120 cases (2 files × 2 demosaics × 5 sizes × 3 runs). `dcraw_process` can therefore be called
repeatedly on one unpacked image. *Measured.*

### A7R IVA (TN100423), ms, median of 3 runs

| Region | Demosaic | Unpack (once) | **First visible** = open + unpack + 1st region | Each further region | Peak memory |
|---|---|---|---|---|---|
| 512 × 512 | linear | 321 | **330** | 8 | 125 MB |
| 512 × 512 | AHD | 321 | **349** | 26 | 132 MB |
| 1024 × 1024 | AHD | 326 | **432** | 102 | 137 MB |
| 2048 × 2048 | AHD | 319 | **769** | 448 | 166 MB |
| 2560 × 1440 (QHD viewport) | linear | 319 | **439** | 119 | 161 MB |
| 2560 × 1440 (QHD viewport) | AHD | 320 | **688** | 362 | 161 MB |
| 3840 × 2160 (4K viewport) | linear | 322 | **605** | 276 | 209 MB |
| 3840 × 2160 (4K viewport) | AHD | 327 | **1 148** | 822 | 209 MB |

A1 (TN005956): unpack ~262 ms; QHD viewport AHD first visible 613 ms, further 350 ms; 512² AHD
first visible 289 ms, further 26 ms.

Spread across the 6 further regions is small (512² AHD 25–27 ms; QHD AHD 354–371 ms).

### Parallel

| What (A7R IVA) | 1 | 2 | 4 |
|---|---|---|---|
| QHD AHD viewport split into N strips, N instances (each unpacks), done after | 708 ms | 527 ms | 444 ms |
| … peak memory | 161 MB | 281 MB | 531 MB |
| Half-size, N images at once: time per image | 0.86 s | 0.87 s | 0.90 s |
| … images per second / peak memory | 1.15 / 0.28 GB | 2.26 / 0.56 GB | 4.26 / 1.12 GB |

(3 decoders: 0.89 s per image, 3.29 images/s, 0.84 GB.)

### Interpretation

1. **The unpack (~0.32 s) dominates the time to the first RAW pixels.** After it, AHD costs
   ≈ 98 ns per pixel and linear ≈ 32 ns per pixel, whatever the region size. A 512² AHD region
   takes 26 ms. *Measured.*
2. **Small regions have no overhead.** 14 regions of 512² (≈ 3.7 MP) cost ~360 ms, the same as
   one 2560 × 1440 region. So a grid of small tiles can be decoded in order of importance, the
   one under the cursor first, without a cost penalty. *Inference from items 1 and the table.*
   Not yet checked: AHD needs a few pixels of context, so tiles need an overlap margin
   (8–16 px, +3–6 % for 512² tiles) to avoid visible seams. This must be verified on the image.
3. **Unpacking ahead of time hides most of the wait.** If the current image is unpacked in the
   background while it is shown (0.32 s, ~125 MB kept), zooming to 100 % shows the first AHD
   tile after ~26 ms and a full QHD viewport after ~0.36 s. Without it: 0.35 s to the first
   tile and 0.69 s to the full viewport. *Inference.*
4. **Splitting a viewport over several LibRaw instances helps only a little** (0.71 → 0.44 s with
   4), because each instance pays its own unpack and its own ~125 MB. Parallelism inside one
   instance (OpenMP, follow-up 2), or tiles over one shared unpack, is the better direction.
   *Measured / inference.*
5. **Half-size in the background scales almost perfectly up to 4 decoders** (4.3 images/s,
   1.1 GB). 2–3 decoders are enough to keep the neighbours ready while culling at normal speed.
   *Measured.*

---

## Follow-up 2: OpenMP vs single-threaded LibRaw (2026-11-04)

- Approved by the owner as a separate measurement. The baseline was documented first
  (`docs/libraw-build.md`) and left untouched. The OpenMP build is isolated in
  `vcpkg_installed_openmp\` and `target-openmp\`.
- **Both builds:** LibRaw 0.22.3, same baseline, triplet, compiler and files.
  - Baseline: ABI `e9e50a2e…`, features `core`. Bench exe SHA-256 `0F844AA0…E55E`.
  - OpenMP: features `core;openmp` (confirmed in the vcpkg status). Bench exe SHA-256
    `C072CDA8…3DC6`. `vcomp140.dll` came from `C:\Windows\System32` (nothing was copied).
    `OMP_NUM_THREADS` was not set, so all 8 cores were used.
- **Method:** every step ran on the baseline build and then on the OpenMP build, back to back
  (`tools\m2-openmp.cmd`). Raw data: `D:\PhotoWorkflowData\analysis\raw-openmp-20261104-064806\`.
- The baseline numbers in this run match the earlier runs (A7R IVA unpack 320 ms, full AHD 6.4 s).

### One image (A7R IVA, TN100423), ms

| | Baseline | OpenMP | Speed-up | CPU/wall OpenMP |
|---|---|---|---|---|
| Unpack only | 328 | 317 | – (not parallel) | 1.03 |
| Half size | 847 | **618** | 1.4× | 2.6 |
| Full, linear | 2 468 | 2 245 | 1.1× | 1.5 |
| Full, AHD | 6 417 | **2 284** | 2.8× (demosaic step 3.9×) | 3.7 |
| 2048² region, AHD | 773 | 492 | 1.6× | 3.3 |

A1: full AHD 5.0 → 1.65 s, half size 0.64 → 0.45 s.

### Regions after one unpack (A7R IVA, AHD)

| Region | First visible, baseline | First visible, OpenMP | Further region, baseline | Further region, OpenMP |
|---|---|---|---|---|
| 512² | 347 | 342 | 25 | 26 |
| 1024² | 419 | **375** | 102 | **62** |
| 2048² | 767 | 493 | 452 | 177 |
| 2560 × 1440 (QHD) | 678 | **492** | 362 | **175** |
| 3840 × 2160 (4K) | 1 138 | **627** | 816 | **315** |

- Linear demosaic gains only 0–20 % from OpenMP.
- Re-processing gave identical pixels in every case, for both builds.
- Peak private memory with OpenMP is ~40–50 MB higher (per-thread buffers), e.g. QHD AHD
  161 → 203 MB.

### Parallel (A7R IVA)

| What | Baseline | OpenMP |
|---|---|---|
| Half size, 1 decoder: time per image / images/s | 0.84 s / 1.17 | **0.62 s / 1.58** |
| Half size, 2 decoders | 0.86 s / 2.28 | 0.68 s / 2.86 |
| Half size, 4 decoders | 0.91 s / 4.24 / 1.12 GB | 0.76 s / 4.99 / 1.12 GB |
| Full AHD, 1 image | 6.4 s / 0.15 | **2.3 s / 0.42** |
| Full AHD, 8 at once | 8.7 s / 0.89 | 8.5 s / 0.91 |
| QHD viewport over 1 / 2 / 4 instances | 704 / 528 / 442 ms | 513 / 422 / 407 ms |
| … peak memory with 4 instances | 531 MB | 715 MB |

### Interpretation

1. **OpenMP cuts the latency of AHD regions of 1024² and larger by 1.6–2.8×.** A QHD viewport
   at 100 % takes 0.49 s instead of 0.68 s from scratch, and 0.18 s instead of 0.36 s after the
   unpack. *Measured.*
2. **It does not help where the floor is.** The unpack (single-threaded, ~0.32 s for A7R IVA),
   tiny 512² regions, and linear demosaicing gain nothing or little. *Measured.*
3. **It does not raise total throughput.** When all cores are already busy (8 images at once),
   both builds give ~0.9 images/s. OpenMP moves work from "many images in parallel" to "one image
   faster", which is what interactive viewing needs. *Measured.*
4. **Several LibRaw instances on one viewport are no longer worth it** with OpenMP: 4 instances
   save 0.1 s and cost +0.5 GB. *Measured.*
5. **Not verified yet:** that OpenMP AHD gives exactly the same pixels as the single-threaded
   build (the bench compared repeats within one build, not across builds). AHD works on
   independent sub-tiles, so they should be equal. This must be checked before OpenMP is adopted
   (a one-line bench change: print the hash).
6. **Contention:** OpenMP uses all 8 cores per decode. Background half-size decoders and an
   interactive 100 % decode will compete. The pipeline must give interactive tiles priority, for
   example by pausing background work and capping background decoders to fewer OpenMP threads
   (`omp_set_num_threads` per worker thread). *Hypothesis; to be measured in the implementation.*

---

## Follow-up 3: pixel check before implementation (2026-11-04)

Bench `verify` (`tools\m2-verify.cmd`). Settings for every decode: sensor orientation
(`user_flip = 0`, the viewer rotates), camera white balance, 8-bit. Files: A7R IVA, A1, GM1 RW2,
Xiaomi DNG. Raw data: `D:\PhotoWorkflowData\analysis\raw-verify-20261104-071138\`.

1. **OpenMP and baseline give identical pixels: 58 of 58 hashes are equal.** This covers whole
   images (AHD and linear, half size, with and without auto-brightness) and all tiles, on all
   four files. *Measured.*
2. **LibRaw's auto-brightness depends on the region.** With it on, a tile differs from the same
   pixels of the full image everywhere (mean difference 7–64 levels of 255). Tiles must therefore
   use `no_auto_bright` and one brightness shared by the whole image. *Measured.*
3. **With `no_auto_bright`, a margin of 8 px makes a 1024² AHD tile identical to the full
   decode** on all three files that could be tested. Without a margin, up to 1.9 % of pixels
   differ, all within 16 px of the edge (max 38–81 levels). These would be visible seams.
   *Measured.* (The DNG is too small for a margin at the test position, so only margin 0 ran
   there.)
4. **Open question: A7R IVA, margins 16 and 32.** The tile came out one row shifted, and 97 % of
   pixels differed slightly (mean 2.3 levels). This did not happen on the A1 or RW2. LibRaw's
   crop code (`raw2image_ex`) does not round Bayer crop origins and re-phases the CFA pattern
   (read from the LibRaw source). The cause is therefore not yet known. All margins ran in one
   LibRaw instance one after another, so state carried between crops is also possible.
   **`tools\m2-crops.cmd` tests this**: every crop in a fresh instance, at varied origins, and
   prints the CFA pattern and margins.

---

## Follow-up 4: cause of the A7R IVA crop anomaly (2026-11-04)

Bench `crops` (`tools\m2-crops.cmd`, OpenMP build). Every crop ran in a **fresh** LibRaw instance,
then the series was repeated in one shared instance. Each run was done with LibRaw's
`adjust_maximum_thr` at its default (0.75) and with it disabled (0). Raw data:
`D:\PhotoWorkflowData\analysis\raw-crops-20261104-074337\` and `…-074725\`.

- **Not carried-over state.** Fresh instances fail exactly like the shared one. *Measured.*
- **The cause is `adjust_maximum_thr`.** With the default 0.75, LibRaw raises the white level to
  the brightest value found in the processed area when that value is above 75 % of the camera
  maximum. A7R IVA tiles that include a bright highlight near (3080, 3080) therefore got a
  different white level, and so slightly different values everywhere (97 % of pixels differ,
  mean 2.3 levels; the "one row shift" was just the best match the comparison found). The A1 and
  RW2 images have no such highlights near the test area. *Measured.*
- **With `adjust_maximum_thr = 0`, every case on every file gives a tile identical to the full
  decode** (margins 8–32, 13 origins, fresh and shared instances). *Measured.*
- The CFA pattern is `0xb4b4b4b4` for both Sonys and `0x1e1e1e1e` for the RW2. Visible areas
  start at (0, 0). Crops are not rounded.

### Settings for the RAW pipeline (decided by measurement)

| Setting | Value | Why |
|---|---|---|
| `no_auto_bright` | 1 | Auto-brightness is computed per region (follow-up 3) |
| `adjust_maximum_thr` | 0 | The white level would otherwise depend on the region (this follow-up) |
| `use_camera_wb` / `use_auto_wb` | 1 / 0 | Fixed white balance for the whole image; auto WB would be per region |
| `user_flip` | 0 | Sensor orientation; the viewer rotates the whole image, as in M1 |
| Demosaic | AHD (`user_qual = 3`) | Quality; OpenMP makes it fast enough |
| Tile overlap | **16 px** (8 px measured as sufficient) | Margin of safety for other cameras; costs ~3 % on 1024² tiles |
| LibRaw | 0.22.3 with OpenMP | Identical pixels to the baseline (follow-up 3) |

With these settings, a tile is pixel-identical to the same area of a full-image decode, so tiles
cannot show seams, and brightness, white balance and orientation are the same for every tile.
