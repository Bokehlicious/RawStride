# prototypes/phase1

Measurement prototypes for ADR 0008 phase 1 (§8.4). **Not part of the app.** The crate has its own
workspace, builds into `target\prototypes`, and nothing in `crates/` or `src-tauri/` depends on it.
It adds no new dependencies: it uses `pw-raw`, the same JPEG decoders as `tools/preview-bench`
(zune-jpeg, jpeg-decoder, libjpeg-turbo), and LibRaw with OpenMP from the repository's vcpkg install.

Run: double-click `run-phase1.cmd`. Results go to `D:\PhotoWorkflowData\analysis\phase1\`.

## P0: JPEG decode of embedded previews

`proto-phase1 p0 <out.csv> <files…>`

For every file, the largest embedded preview (`pw_raw::PreviewSize::Large`) is read and decoded 5
times with each decoder: zune-jpeg, jpeg-decoder and libjpeg-turbo (full size and 1/2 scale);
the CSV holds the medians. The second table gives parallel throughput (zune-jpeg and
libjpeg-turbo, 1/2/4/8 threads) over the previews at least 4000 px wide.

libjpeg-turbo (approved for P0 on 2026-11-05) is the `turbojpeg` feature, on by default, linked
statically from the vcpkg install like in `tools/preview-bench`. If it cannot be linked next to
LibRaw's libjpeg, `run-phase1.ps1` rebuilds with `--no-default-features` and logs it.

## P4b: demosaic methods against AHD

`proto-phase1 p4b <out-dir> <threads-list> <files…>`, for example `all,1`.

Methods: AHD (today's method, reference), linear, VNG, PPG, DCB, DHT, AAHD and half-size AHD.
The decode settings are those of ADR 0006 §2. Each (file, method, thread count) runs in a separate
child process, so CPU time and peak memory belong to that method alone.

Timings are written to `p4b-timing.csv`, and cover:

- the whole image;
- a 1920×1080 region (3 runs, median);
- open and unpack;
- CPU time;
- private memory, both after unpack and at its peak.

The region is the window with the most detail, picked once per file from a half-size AHD render
and saved as `<file>.region.txt`, so every method renders the same pixels.

Quality is written to `p4b-quality.csv`. Each method's region is compared with the AHD region of
the same file:

| Metric | What it measures |
|---|---|
| ΔE00 (mean, p99) | Colour difference |
| SSIM, MS-SSIM | Structural similarity |
| PSNR (Y) | Supplementary only |
| 10–90 % rise distance | Edge sharpness, in pixels, on strong edges, for both the reference and the test image |
| Laplacian HF ratio | Fine detail kept, relative to AHD |
| False colour | Chroma at edges that are neutral in AHD |
| Zipper index | Zipper artefacts |

AHD is the reference, not the truth: a difference from AHD does not by itself mean worse.

Blind review: `crops\` holds two 640×480 crops per file and method, at 100 %, named by a letter
code only; the codes are shuffled per file. The key is in `key-open-after-review\crop-key.csv`.
Open it only after the visual review.

No conclusion on the demosaic method is drawn before the owner's review (ADR 0008).

## P4: scheduling without UI

`proto-phase1 p4 <out-dir> <group> <strategies,list> <files…>`, then `proto-phase1 p4sum <out-dir>`.
Run it with `run-p4.cmd`, which takes about an hour.

A simulated user works through a virtual folder: the group's files are repeated, and each position
counts as its own image. The user runs scenarios S1–S6 from ADR 0008 §8.3, in RAW and JPEG mode, plus
natural stepping (150–1500 ms between images, sometimes Z for 1–3 s, sometimes back). A scheduler with
two workers prepares views, one interactive worker for the current image and one background worker.

Views:

| View | What it is |
|---|---|
| Fit | Half-size render |
| Zoom | The visible 1920×1080 region at the centre |
| Ring | Twice the view in each direction |
| Jpeg | The decoded largest embedded preview |

The demosaic method is PPG, as decided in ADR 0008 §11.

| Strategy | What is prepared |
|---|---|
| `none` | Only what is shown, on demand |
| `cur` | The current image's other view as well (the zoom target at fit) |
| `full` | `cur`, plus 2 neighbours in the direction of travel and 1 behind, the ring, and the other display mode |
| `full-tiles` | As `full`, but the zoom view is rendered as 1024-px grid tiles like today's app. This tests "visible region first" |
| `full-dwell` | As `full`, with prefetch only after 150 ms on an image |
| `full-ahd` | As `full`, with AHD instead of PPG |

The result files are:

- `p4-summary.csv`: per scenario and step type, the number of requests, the share served within 30 ms, the median, p95 and maximum time, and the number not shown (the user moved on first).
- `p4-runs.csv`: CPU (average cores), peak private memory, and wasted pre-render work.
- `p4-events.csv`: every request.

What is measured is the time until the view is ready in memory. Showing it on screen is P2/P3. The OS
file cache cannot be emptied without system changes, so "cold" (S4) means empty application caches only.

## P5: memory budget

Run it with `run-p5.cmd`, which takes about an hour. Results go to `D:\PhotoWorkflowData\analysis\phase1\p5\`.

Strategy `p5` was approved by the owner on 2026-11-05. It is `full` with three changes:

- prefetch starts only after 150 ms on an image;
- the other display mode comes after the neighbours in the queue;
- the zoom view is the exact visible region.

It runs under one budget that covers both unpacked sessions (counted as width × height × 2 bytes) and finished views.

- **Eviction:** the least recently used items go first, but only items the plan no longer wants and nothing of the current image.
- **Admission:** prefetch does not start when its result would not fit in the budget.

4 GB is the chosen budget (`p5@4096`). 2 and 3 GB are run for comparison: `p5@2048` and `p5@3072` on A7R and A1, and only `p5@4096` on RW2.

The scenarios are S1–S6 and natural stepping, as in P4, plus **MIX**: 150 actions back and forth
within 12 images, with jumps of 3, repeated zooms and mode switches. It measures hits on re-display.

`p4-runs.csv` now also has `evicted_views`, `evicted_sessions` and `peak_tracked_mb` (the budgeted
memory), next to `peak_private_mb` (the whole process). VRAM is not used in phase 1.
