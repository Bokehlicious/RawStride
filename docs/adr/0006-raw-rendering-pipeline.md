# ADR 0006: RAW rendering pipeline (M2)

- Status: **proposed**. The owner approved the direction (OpenMP LibRaw and points 1–6, 2026-11-04);
  this is the design for the implementation, shown before it starts.
- Date: 2026-11-04
- Basis: `docs/analyses/2026-11-04-m2-libraw-decode.md` (measurements and follow-ups 1–4),
  `docs/libraw-build.md`.

## Requirements from the owner

- Memory stays bounded, and old RAW buffers are freed automatically.
- Cancelled or outdated decode jobs never block the current image.
- Tile overlap must not produce visible seams.
- Colour, white balance, orientation and quality are consistent between tiles.
- The working M1 display is always there as a fallback.
- OpenMP is documented, and a standalone build ships the DLLs it needs.
- Build it step by step and measure after each part against M1: switching images, the contact
  sheet and the existing zoom must not get slower.
- No unrelated changes.

## Decisions

### 1. Where the code lives

| Part | Place | Contents |
|---|---|---|
| LibRaw binding | new crate `crates/pw-libraw` | C shim (as in the bench) + a safe Rust API. Links `libraw[openmp]` statically from the root vcpkg manifest. |
| Scheduling and caches | new crate `crates/pw-rawview` | Job queue, workers, cancellation, memory budget. No Tauri, so it can be tested on its own. |
| App wiring | `src-tauri` | Commands and the `pw://` routes for half-size images and tiles |
| Display | `ui/web` | Swaps in the half-size image, and lays tiles over the loupe image |

`pw-raw` (the M1 embedded-preview reader) is not changed.

### 2. Decode settings (identical everywhere)

These are the settings measured in follow-up 4:

- `no_auto_bright = 1`, `adjust_maximum_thr = 0`
- camera white balance, no auto white balance
- `user_flip = 0`, AHD, 8-bit sRGB output

With them, a tile is pixel-identical to the same area of a full decode. Brightness is therefore
fixed per image (LibRaw `bright = 1.0`). Matching the camera JPEG's look is a separate, later
question, and it would also be one value per image, never per tile.

### 3. Orientation

Tiles and half-size images are produced in sensor orientation. The loupe already rotates the whole
image with the EXIF orientation (M1). RAW pixels go in the same rotated layer, so they cannot be
oriented differently from the preview underneath.

### 4. Jobs, priority and cancellation

There are four kinds of job, in this priority order:

1. **Tile** of the current image at ≥ 100 %. The one under the cursor goes first, then the rest
   of the viewport.
2. **Unpack** of the current image. Started after the image has been shown for ~150 ms.
3. **Half-size** of the current image. Only when the largest embedded preview is smaller than the
   screen needs (A7R IVA: 1616 px; not A1).
4. **Half-size** of the neighbours (±1).

How jobs are handled:

- **Every job carries the image's generation.** When the user moves on, all queued jobs for other
  images are dropped before they start.
- A LibRaw call cannot be interrupted, but none lasts long: unpack ≈ 0.32 s, tile ≈ 0.06 s, and
  half-size ≈ 0.62 s, the longest. A running stale job is finished and its result thrown away.
- **Workers:** one interactive worker for tiles and the current unpack, and one background worker
  for half-size. With OpenMP, each uses all cores.
- **The background worker yields** while tile jobs are waiting. It checks before every job, and
  starts no new job while one is waiting.

### 5. Memory budget (automatic freeing)

| What | Budget | Freed |
|---|---|---|
| Unpacked RAW sessions | at most 2 images (~125 MB each) | Least recently used first; also when the folder changes |
| Tiles (encoded) | 192 MB | Least recently used first; all tiles of an image go when its session goes |
| Half-size images (encoded) | 6 images | Least recently used first |
| LibRaw working memory | ~170 MB while a tile is processed, ~280 MB while a half-size image is | Released when the call returns |

Expected peak is ~0.9 GB on top of M1, and below that at rest. Each figure is measured in its step.

### 6. Tiles

- **Grid:** 1024 × 1024 in sensor pixels, plus a 16 px margin on each side, which is cut off
  before encoding.
- **Encoding:** JPEG, quality 95, served through `pw://` like the previews:
  `/<gen>/<index>/tile/<col>/<row>` and `/<gen>/<index>/half`.
- **Display:** the loupe keeps the M1 image (embedded preview, or half-size once ready) at the
  bottom, and lays the tiles over it as they arrive. A missing tile simply shows the preview, so
  nothing ever goes blank.

### 7. Fallback to M1

- **A setting:** "RAW-rendering" (on by default). Off means exactly the M1 code paths: no
  LibRaw, no jobs.
- If LibRaw fails on a file (unsupported camera, damaged file), that image falls back to M1
  silently, and the info panel says why.
- The M1 previews are always requested first. RAW work is only ever added on top.

### 8. OpenMP and distribution

- The root `vcpkg.json` gets `libraw` with feature `openmp`, and `docs/libraw-build.md` is
  updated.
- A standalone build must ship `vcomp140.dll`, the MSVC OpenMP runtime from the Visual C++
  redistributable, next to the exe. A build script copies it from the VS Build Tools redist
  folder and checks that it is there.
- The Tauri bundle (when enabled) lists it as a resource.

## Steps, each measured against M1

| Step | Contents | Measured |
|---|---|---|
| 1 | `pw-libraw` + vcpkg switch + `vcomp140.dll` copy; app links but does not call it yet | Build OK; start-up time and idle memory unchanged vs M1 |
| 2 | `pw-rawview`: jobs, priorities, cancellation, budgets; tests without UI | Unit tests; stale-job latency; memory never above budget |
| 3 | Half-size in the loupe (A7R IVA) + setting | Image-switch time and contact-sheet scrolling vs M1 (unchanged); time to sharp fit view |
| 4 | Tiles at ≥ 100 % | First tile after zoom; full viewport; visual seam check; memory |
| 5 | Standalone distribution check | Runs on the build output folder with only the shipped files |

## Consequences

- `pw-libraw` adds a native dependency (LibRaw, LGPL-2.1 / CDDL-1.0, choice still postponed;
  ADR 0002) and `vcomp140.dll`.
- RAW pixels will look darker or flatter than the camera JPEG, because there is no tone curve
  yet. That is expected and identical across tiles.

## Amendments during implementation (approved by the owner, 2026-11-04)

Measured and documented in `docs/analyses/2026-11-04-m2-pipeline-steps.md`.

- **Step 3:** half-size images are scaled to the screen's long edge before JPEG encoding.
- **Step 3:** unpacking on dwell (point 2 of the chain) is off for every image.
- **Step 4:** tiles start as soon as the shown image (screen-sized half-size image or embedded
  preview) has too little detail for the zoom level, not only at ≥ 100 %.
- **Step 4:** the file is unpacked on the first zoom action. Before that, the half-size job's
  session of the current image is reused, so a pause on the image prepares it.
- **Step 4:** only files whose embedded preview is smaller than the RAW get tiles (not the A1).
- **Error handling (after step 5):**
  - A file whose RAW data is shorter than declared (truncated) or that LibRaw reports as corrupt is
    refused before any RAW pixels exist (`pw-libraw`, C++ shim with a counting data stream; no
    tolerance).
  - LibRaw's processing warnings never count as damage.
  - The viewer keeps the embedded preview with a warning chip, and empties the loupe for
    unreadable files.
