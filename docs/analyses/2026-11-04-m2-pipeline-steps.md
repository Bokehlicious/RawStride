# 2026-11-04 – RAW pipeline (ADR 0006): results per step, compared with M1

- **M1 reference:** `D:\PhotoWorkflowData\builds\m1-reference\photoworkflow.exe`. A release build
  of the app made just before step 1 (`tools\build-m1-reference.cmd`; git HEAD and status saved
  next to it).
- **Machine:** thanos (i7-9700K, 32 GB, Windows 11).

## Step 1 – LibRaw with OpenMP linked into the app (not used yet)

Changes:

- New crate `crates/pw-libraw` (C shim + safe API, with the fixed settings of ADR 0006).
- Root `vcpkg.json` → `libraw[openmp]`.
- `src-tauri` depends on it. A `raw_engine_info` command (LibRaw version) exists so the library is
  linked; the UI does not call it.
- Distribution script `tools/package-dist.ps1`, start-up measurement `tools/measure-app.ps1`.
- Documentation in `docs/libraw-build.md`.

Run with `tools\m2-step1.cmd`. Raw data: `D:\PhotoWorkflowData\analysis\step1\`.

### Results

- **vcpkg:** `libraw[openmp]` installed from the binary cache. The status lists
  `Feature: openmp`, and `pw-libraw`'s build check passed.
- **`cargo test --release -p pw-libraw`: 6 of 6 pass on the real files.**
  - A7R IVA, A1 and GM1 RW2: 5 regions each (interior incl. the A7R IVA highlight area, origin,
    right/bottom border, an odd-sized partial tile) are byte-identical to the same area of the
    full render, with a 16 px margin.
  - Repeated renders are identical, half size has the expected dimensions, and a non-RAW file
    gives an error.
- **Start-up and memory** (5 runs each, app opened with `D:\PhotoWorkflowData\RAW`, memory read
  6 s after the window appeared):

  | | M1 | Step 1 |
  |---|---|---|
  | Window appears (runs 2–5, median) | 50 ms | 58 ms |
  | Window appears, first run (cold WebView2) | 4.2 s | 3.8 s |
  | Working set, app process | 31.6 MB | 31.6–31.9 MB |
  | Private memory, app process | 8.6–10.1 MB | 9.3–11.0 MB |
  | Exe size | 9.5 MB | 10.1 MB |

  There is no measurable difference in start-up or idle memory; the spread between runs is
  larger than the difference. The exe grew by 0.6 MB.

### Deviation to note

`dumpbin` shows that the step-1 exe imports **only Windows DLLs**: no `vcomp140.dll`, and no
`vcruntime140.dll` or `msvcp140.dll` either. The reason is that step 1 calls only LibRaw's version
functions, so the linker leaves out the processing code (and with it OpenMP). The distribution
check is therefore not yet meaningful. It is re-run when rendering is called (steps 3 and 4), and
step 5 tests the folder on its own. `package-dist.ps1` copies whatever the exe imports at that
point, and fails if a DLL is missing.

The window-time metric only shows that start-up is not delayed. Image switching and
contact-sheet speed are measured from step 3 on, where the UI changes.

## Step 2 – the job engine (`crates/pw-rawview`), tested on its own

The engine is generic over the decoder ([`Backend`]). The real decoder (`LibRawBackend`: LibRaw
with OpenMP + JPEG quality 95) is behind the `libraw` feature. The app does not use the engine
yet, so the viewer is unchanged (M1).

What the engine does (ADR 0006):

- **Two workers.** The interactive worker renders tiles and does the current image's unpack. The
  background worker renders half-size images.
- **Priority:** tiles of the current image, in the requested order. A tile asked for directly
  (visible now) jumps the tile queue. The background worker starts nothing while a tile is waiting.
- **Unpack and half-size jobs wait 150 ms (dwell).** Flicking through images starts no RAW work.
- **Changing image** removes every queued job of images no longer wanted. A running LibRaw call
  finishes, and its result is thrown away.
- **Changing folder** clears the queue and all caches, and closes all sessions. A session in use is
  closed when its call returns.
- **Budget:**
  - at most 2 kept sessions (the least recently used is closed **before** a new one is opened),
    plus the background worker's one temporary session
  - encoded tiles ≤ 192 MB (least recently used first)
  - half-size images ≤ 6
  - tiles of images that are no longer current are dropped

Run with `tools\m2-step2.cmd`. Raw data: `D:\PhotoWorkflowData\analysis\step2\`.

### Simulated decoder (`tests/engine.rs`) – 16 of 16 pass, 3 runs on thanos (and 10 runs in the sandbox)

| Area | Test | Checks |
|---|---|---|
| Priority | `zoom_tiles_go_before_queued_background_jobs` | 4 tiles are done in < 250 ms while a 300 ms half-size render runs; no queued half-size starts before the last tile has started; request order is kept |
| | `a_tile_asked_for_directly_jumps_the_tile_queue` | A visible tile goes ahead of 6 queued ones |
| | `a_new_viewport_replaces_the_queued_tiles` | Old tile requests are dropped |
| Cancellation | `flicking_through_images_starts_no_raw_work` | 30 images at 15 ms each: only the last one is opened |
| | `queued_jobs_of_a_left_image_are_removed` | Queued tiles and neighbour half-sizes of the left image never start; the new image's do |
| | `a_running_call_finishes_but_its_result_is_thrown_away` | The running tile finishes; the new image starts < 30 ms after it ends; result discarded; old tile → `Gone` |
| | `a_finished_half_size_of_a_left_image_is_discarded` | Same for half-size |
| Concurrency | `tiles_and_half_size_run_at_the_same_time_and_tiles_are_rendered_once` | A tile is rendered while a half-size runs; 4 simultaneous requests for one tile → 1 render |
| | `a_failing_file_reports_once_and_is_not_retried` | `Failed` (viewer keeps M1); no retry loop |
| | `dropping_the_engine_stops_the_workers` | Shutdown during a running call; all sessions closed |
| Memory | `budget_holds_under_fast_switching_and_repeated_zooming` | 60 switches, back and forth, zooming every other one, pauses 5–75 ms: kept sessions ≤ 2, tiles ≤ budget, half-size ≤ budget at every sample; at most 3 sessions alive at once (2 + 1 temporary) |
| | `tile_budget_evicts_least_recently_used` | The LRU tile goes; it is rendered again on demand |
| | `sessions_are_kept_for_two_images_and_freed_when_left_further` | ≤ 2 sessions; going back to the previous image reuses its session |
| | `changing_folder_frees_everything` | 0 sessions, tiles, half-sizes and queued jobs; every session closed |
| | `folder_change_while_a_call_runs_closes_its_session_afterwards` | The session in use is closed when its call returns |
| | `grid_and_border_tiles` | 10 × 7 grid for 9568 × 6376; border tile 352 × 232; tile outside → `Failed` |

### Real decoder (`tests/libraw_engine.rs`, LibRaw with OpenMP, A7R IVA)

| | Time |
|---|---|
| First tile, zoom right away (open + unpack + render + JPEG) | **430 ms** |
| Next tile (kept session) | 85 ms (render ~62 ms + JPEG ~20 ms) |
| **First tile when the unpack was done ahead** (image shown > 150 ms) | **82 ms** |
| Half-size (own open + render + JPEG, 4784 × 3188, 6.8 MB) | 542 ms |

Memory, 40 images (the 4 test RAWs under different keys), back and forth, zooming on every other
one, pauses 20–600 ms, measured as the test process's private bytes:

| | MB |
|---|---|
| Before | 5 |
| **Peak** | **567 (+561)** |
| At rest with full caches (2 sessions + 6 half-size images) | 261 |
| **After a folder change** | **5** |

Engine counters for the run: 23 sessions opened, 21 evicted, 200 queued jobs dropped,
22 finished results thrown away.

### Deviations and notes

- **Peak +561 MB** is below the ADR estimate of ~0.9 GB. After a folder change, memory returns to
  the starting value.
- **Half-size JPEGs are large** (6.8 MB for A7R IVA at quality 95), and the encoding is part of the
  542 ms. This is within budget (6 × ~4–7 MB). Step 3 measures whether decoding a 15 MP JPEG in
  WebView2 is fast enough, or whether the half-size image should be scaled to the screen first.
- **The background worker always opens its own session** for half-size images, an extra unpack
  (~0.3 s CPU). This is deliberate: it can never hold the session that tiles need. With the kept
  session it could save the unpack, but the zoom could then wait behind a half-size render.
- **App versus M1:** the app does not use the engine yet, so there is no change to measure.
  Comparison with M1 resumes in step 3.

## Step 3 – half-size RAW in the loupe

Changes:

- `src-tauri/src/rawview.rs` connects the `pw-rawview` engine (LibRaw backend) to the app with the
  commands `raw_view`, `raw_set_enabled` and `raw_stats`, the Tauri events `raw-ready` and
  `raw-failed`, and the route `pw://<gen>/<index>/rawhalf`.
- `ui/web/raw.js` handles the loupe side:
  - The embedded preview (M1) is always shown first. The half-size RAW image replaces it only
    once it is decoded, and the framing stays the same.
  - A badge shows "RAW-avkodning (LibRaw), skärmanpassad W × H (av N px bredd)".
  - There is a status-bar setting "RAW-rendering: på/av", which is remembered. Off clears the
    engine and gives exactly the M1 code paths.
- `app.js` has a few hooks:
  - `rawShow` at the end of `showLoupe`; `rawLeave` when going to the contact sheet.
  - 100 % zoom and the zoom readout use the full RAW size that the backend reports.
  - A `pw-shown` event is dispatched when an image has loaded.
- **When a half-size image is wanted:** the file is a RAW file and its largest embedded preview is
  smaller than the screen's long edge in physical pixels.
  - A7R IVA (1616 px preview) on the 1920 px screen: **yes**.
  - A1 (8640 px): **no**, so the A1 keeps its embedded JPEG.
  - The test GM1 RW2 and DNG files have previews ≥ 1920 px, so: no on this screen.
- **Engine additions** (`crates/pw-rawview`):
  - `clear_current()`, used by the contact sheet.
  - `Config::unpack_ahead` (see round 2).
  - The half-size job reports the full RAW size (a `Grid` event before `Half`).
  - 18 of 18 engine tests pass; two are new.
- **Built-in measurement run:**
  - `photoworkflow.exe <folder> --bench <out.json> --bench-raw on|off [--bench-half-max <px>]`
    runs a fixed scenario (`ui/web/bench.js`) and exits. `tools/bench-app.ps1` samples CPU and
    private memory of the app and its WebView2 processes every 250 ms.
  - Scripts: `tools\m2-step3.cmd` (round 1) and `tools\m2-step3b.cmd` (round 2).
  - Raw data: `D:\PhotoWorkflowData\analysis\step3\` and `…\step3b\`.
- **Tests:**
  - UI test `ui/tests/raw-half.test.js`: 27 of 27 pass.
  - The existing UI tests (filter-select, ingest-dialog, zoom-anchor) still pass.
  - No git commit.

**Method.** The scenario uses all 23 test images, at 1920 × 1129 CSS px with DPR 1:

1. Open the folder.
2. 20 steps in the contact sheet.
3. Every image once in the loupe, 1.2 s each.
4. 5 steps back.
5. 60 steps at 40 ms (fast browsing).
6. 20 zoom steps with image changes.
7. Cold start: RAW caches emptied, then from the contact sheet straight into an A7R IVA image,
   then on to the next one.
8. Half-size JPEG decoding at different sizes.

"RAW off" runs the same build with the setting off, which is the M1 code path. The bench cannot
instrument the M1 reference exe, so M1 itself is compared on start-up and idle memory.

### Round 1: the engine as delivered in step 2 (full half size, unpack ahead on)

- **The A7R IVA RAW image appeared 131–197 ms after arriving at the image.** It was already
  prepared as a neighbour. Almost all of that time was WebView2 decoding the 4784 × 3188 JPEG
  (126–189 ms).
- **Loupe phase CPU and memory:**

  | | RAW off | RAW on |
  |---|---|---|
  | App CPU | 0.3 s | 16 s |
  | App private memory, peak | 53 MB | 568 MB |

  - **Cause:** the engine unpacked *every* RAW file shown (the "pre-unpack on dwell" of ADR 0006
    point 2), including the 17 that never need RAW pixels (A1, RW2, DNG). In step 3, nothing uses
    those sessions; only tiles (step 4) would.
  - **Second cause:** the half-size job also opens its own session, so A7R IVA files were unpacked
    twice.
- **Decision from this round:** turn off unpacking ahead until step 4 needs it, and decide the
  half-size format from the decode measurement (below).

### Half-size JPEG: render, scale, encode, transfer, decode (A7R IVA, median of 18)

| Long edge | Size | Open + unpack | Render | Scale | JPEG encode | Fetch | WebView2 decode | Total |
|---|---|---|---|---|---|---|---|---|
| 4784 (full half) | 7.0 MB | 339 | 264 | – | 288 | 59 | 108 | **1057 ms** |
| 3840 | 4.6 MB | 338 | 264 | 84 | 186 | 36 | 70 | 978 ms |
| 2560 | 2.1 MB | 337 | 264 | 54 | 83 | 18 | 33 | 789 ms |
| **1920 (= screen)** | **1.2 MB** | 338 | 263 | 44 | 48 | 11 | **19** | **723 ms** |

- Decoding with `createImageBitmap` costs the same as `img.decode()`.
- Scaling (area average) costs less than it saves in encoding alone. Fetch and WebView2 decoding
  scale with the pixel count.
- **Result:** scaling to the screen before encoding is about 30 % faster from start to finish. It
  cuts the visible wait for a *prepared* image from ~150 ms to ~30 ms, and the half-size cache from
  ~7 MB to ~1.2 MB per image.

### Round 2: half-size scaled to the screen, no unpacking ahead (the delivered state)

Two runs of each variant. "Full" = the same build with `--bench-half-max 0` (full half size).

| | RAW off (= M1 path) | **RAW on, screen-scaled** | RAW on, full half |
|---|---|---|---|
| Contact sheet, step until info shown (median / max) | 0.6–2.5 / 1.6–4.8 ms | 0.7–2.5 / 1.2–4.8 ms | 0.6–0.7 / 1.7 ms |
| Loupe, embedded preview shown (median / max) | 2.3–2.5 / 5.5–7.4 ms | 2.7 / 8.4–18.5 ms | 2.7–2.9 / 5.1–5.9 ms |
| **A7R IVA, RAW shown after arriving (prepared as a neighbour)** | – | **27–39 ms** | 128–182 ms |
| A7R IVA, going back (cache) | – | 3–43 ms | 4–152 ms |
| **First RAW image, cold** (preview first after 3–24 ms) | – | **892–924 ms** | 1206–1245 ms |
| Next image after the cold one | – | 34–52 ms | 141–191 ms |
| Fast browsing, event-loop lag avg / max | 8.4–8.7 / 59 ms | 8.3–10.3 / 59 ms | 8.4–9.0 / 53–72 ms |
| Fast browsing, final image shown after stop | 1.7–2.1 ms | 1.9–2.1 ms | 2.1–2.5 ms |
| Zoom, frame time avg / max | 1.7–2.3 / 24–34 ms | 1.5–2.9 / 17–19 ms | 0.7–1.5 / 4–23 ms |
| App CPU, loupe phase (29 s, 5 A7R IVA) | 0.2–0.3 s | 7.9–8.6 s | 8.8–9.0 s |
| App private memory, peak in loupe | 42 MB | 256–296 MB | 332 MB |
| App private memory between jobs (revisit/zoom) | 9–78 MB | 18–68 MB | 54–112 MB |
| Half-size cache, peak | – | 5 images, 6.1 MB | 5 images, 35.8 MB |
| Kept sessions | – | 0 | 0 |

- **Unpacking ahead off halves the app's CPU and memory in the loupe** (16 s → 8 s; 568 → ~280 MB
  peak). What remains is the half-size job itself: about 0.65 s of wall time per A7R IVA image,
  multithreaded, with ~250 MB transient. It goes back to ~10–20 MB when the job ends.
- **A1, RW2 and DNG cost nothing extra**: no file is opened for them (0 sessions opened in round 2).
- **Contact sheet, fast browsing and zoom are unchanged against RAW off** within run-to-run
  spread. The contact sheet starts no RAW work (`raw_view` is not called there, and going back
  clears the engine's current image).
- **WebView2 memory** (300 MB → 1.3–1.6 GB over the run) is the same with RAW off. It is
  WebView2's own image cache from the M1 code paths, not step 3. One run (on-2) started 300 MB
  higher in WebView2 from the first sample, which is noise outside the app.
- **Memory budget:** at most 5 half-size images (all the A7R IVA files in the folder; the budget is
  6), 0 kept sessions, and 0 tiles. The engine counters show 2 results discarded and no leak:
  between jobs the app is back at 10–20 MB.
- **Start-up** (3 runs each):

  | | M1 | Step 3 |
  |---|---|---|
  | Window appears | 49–115 ms | 50–51 ms |
  | Private memory | 8.8–9.1 MB | 9.7–9.9 MB |
  | Exe size | 9.5 MB | 11 MB |

  The first run after each new build takes ~4 s; this is the virus scan of a new exe, seen in step 1
  too.
- **Distribution:** the exe now imports `VCOMP140.DLL` (OpenMP), `VCRUNTIME140(_1).dll` and
  `MSVCP140.dll`. `package-dist.ps1` copies all four into `dist\PhotoWorkflow` (see
  `dist-manifest.txt`). The bench ran from that folder. Testing it on a machine without the
  development environment is step 5.

### Deviations and compromises

1. **ADR 0006 point 2 ("pre-unpack on dwell") is off for now** (`Config::unpack_ahead = false`).
   It only pays off when tiles are shown. Step 4 must measure when to unpack: on zoom, after a
   longer dwell, or for A7R IVA only. The engine still supports it, and its tests are unchanged.
2. **Half-size images are scaled to the screen's long edge** (physical pixels) before encoding.
   - The cost: between "fit" and ~50 % zoom (A7R IVA on a 1920 px screen), the image has less
     detail than a full half-size image would give, until step 4's tiles take over. It is still
     more detail than M1's 1616 px preview at every zoom level.
   - On a 4K screen the limit is 3840, which is nearly the full half size.
   - `--bench-half-max` overrides the limit for measurement only.
3. **100 % zoom on a half-size image** now uses the full RAW size from LibRaw (backend event)
   instead of "2 × the image". Without that report it falls back to 2 ×.
4. **The first RAW image cold is ~0.9 s.** About 340 ms of that is open + unpack and 260 ms is the
   half-size render. The embedded preview is on screen within 25 ms, so culling never waits. Images
   after it are prepared in the background and appear 30–50 ms after arrival.
5. **The half-size job still opens its own session** (step 2 design), an extra unpack per A7R IVA
   image. With unpacking ahead off there is now only one unpack per image, so this costs nothing
   until step 4.
6. **Measurement script:** round 1 wrote the CSV with the Swedish decimal comma. That was fixed for
   round 2 (invariant culture), and the round-1 data were parsed with that in mind.

## Step 4 – RAW tiles in the loupe

Changes:

- **Engine (`crates/pw-rawview`)**
  - `Engine::prepare(key)` unpacks the current image at once (on the first zoom action), without
    the dwell time.
  - `Config::reuse_half_session` (on): after its half-size render, the background job hands its
    session of the *current* image over as the kept session for tiles. Sessions of neighbours are
    closed as before. A zoom during the half-size render does not wait for it: the tile opens its
    own session.
  - Stats now count `sessions_reused`.
  - 23 of 23 tests pass; 5 are new:
    - `prepare` without dwell
    - reuse
    - no reuse
    - zoom during the half-size render
    - fast zoom/pan with budget and priority checks
- **LibRaw shim (`pwlr.c`, since replaced by `pwlr.cpp`)**: after each render, `libraw_free_image` frees LibRaw's processed
  buffer, so a kept session holds only the unpacked RAW data. A7R IVA: 126 MB.
- **Backend (`rawview.rs`)**
  - Commands `raw_prepare` and `raw_tiles`.
  - Events `raw-tile` and `raw-grid`.
  - Route `pw://<gen>/<index>/tile/<col>/<row>`.
  - `raw_view` also returns `can_tile` and the grid if it is known.
- **UI (`ui/web/rawtiles.js`, new; hooks in `app.js` render, 100 % zoom, zoom readout and badge)**
  - **When tiles are used:** tiles are requested when the image is zoomed beyond fit and the shown
    image (the screen-sized half-size image, or the largest embedded preview) is upscaled by more
    than 5 %, and the RAW is larger than that image.
    - The A1 (8640 px preview) and JPEG/TIFF files never get tiles.
    - With RAW rendering on, 100 % means the full RAW for the A7R IVA and RW2.
  - **Unpacking:** the first zoom action beyond fit sends `raw_prepare` (once per image).
  - **Which tiles, in what order:**
    - The visible tiles are computed with the inverse of the image transform, so rotation and
      flip are handled.
    - The tile under the pointer goes first, then the rest by distance. The request is sent only
      when the set of missing tiles changes, and it replaces the previous one.
  - **How tiles are shown:**
    - A tile is shown only after `decode()`, on top of the image that is already there, so there
      is no black area and no blinking.
    - Each tile gets its own transform (the image transform mapped to the tile's pixels), so it is
      composited as an image layer at full quality.
    - Tiles go with the image when it changes, and when RAW rendering is turned off.
    - At most 120 tile elements are kept; tiles outside the view are dropped first.
- **Tests**
  - **Pixel identity (`pw-libraw`, real files, on thanos):** for A7R IVA, A1 and RW2, five
    regions each are byte-identical to the same area of a full render. This holds:
    - on the same session;
    - after a half-size render on that session;
    - on a fresh session that rendered the half-size image first (the reuse case).

    Tiles are JPEG-encoded at quality 95 afterwards. The 8×8 blocks line up with the 1024 px tile
    grid.
  - **UI (`ui/tests/raw-tiles.test.js`, mocked backend): 33 of 33 pass.** It checks that:
    - nothing is requested at fit, and one `raw_prepare` follows the first zoom;
    - the tile under the pointer comes first;
    - the image stays until tiles are decoded;
    - every tile lands on its image pixels (worst error 0.001 px), and neighbours meet edge to
      edge (< 0.05 px) — at zoom, after panning, at 100 %, after rapid zoom/pan, and rotated 90°;
    - after a pan only missing tiles are requested;
    - 40 rapid actions give 7 requests;
    - tiles are removed on image change;
    - the A1 gets no unpack and no tiles;
    - RAW off means no RAW work.
  - The UI tests from earlier steps still pass: raw-half, filter-select, ingest-dialog and
    zoom-anchor.
  - No git commit.
- **Measurement:** `bench.js` has new tile scenarios (below). `tools\m2-step4.cmd` runs RAW off,
  on, and on without session reuse, twice each. `tools\m2-step4b.cmd` repeats "on" twice with the
  tile-start zoom level added. Raw data: `D:\PhotoWorkflowData\analysis\step4\` and `…\step4b\`.

### Real decoder in the engine (`libraw_engine.rs`, A7R IVA)

| | Reuse on | Reuse off |
|---|---|---|
| Memory held after the half-size image (kept session) | 126 MB | 7 MB |
| **First tile after the half-size image** | **90 ms** | 412 ms |
| 5 more tiles | 434 ms | 436 ms |
| RW2 (no half-size): unpack after `prepare` / then the first tile | 172 ms / 93 ms | – |

The step-2 memory test (fast switching with zooming) now peaks at +504 MB (step 2: +561 MB) and
returns to the starting value after a folder change.

### In the app (1920 × 1129 viewport; the A7R IVA test image is portrait)

Times are from the zoom action. "Covered" means every visible tile is shown. Ranges are over 4
runs with reuse (2 for the tile-start level) and 2 without reuse.

| Scenario | Tiles | First tile (reuse on) | Covered (reuse on) | Without reuse: first / covered |
|---|---|---|---|---|
| A7R IVA, 100 % right away (half-size still rendering; unpack wait 342–375 ms) | 4 | 483–493 ms | 751–762 ms | 491–546 / 754–809 ms |
| **A7R IVA, 100 % after looking at it** | 4 | **131–157 ms** | **400–425 ms** | 457–481 / 713–737 ms (unpack 360–384) |
| A7R IVA, 200 % after a pause | 2 | 131–140 ms | 218–230 ms | 472–473 / 559–562 ms |
| A7R IVA, 50 % | 12 | 123–146 ms | 1.08–1.14 s | 126–138 ms / 1.09–1.11 s ¹ |
| **A7R IVA, where tiles start** (scale 1.2 ≈ 24 %; most of the image visible) | 35 | 118–127 ms | **2.8–2.9 s** | – |
| A7R IVA, 10 × fit → 100 % at different points | 2–6 | 37–100 ms | 16–534 ms | 16–532 ms |
| RW2, 100 % right away (unpack wait 174–196 ms) | 6 | 281–305 ms | 746–784 ms | same |
| RW2, 100 % after a small first zoom (unpack started then) | 6 | 96–115 ms | 560–586 ms | same |
| A1, 100 % | 0 | – (embedded 8640 px JPEG, no file opened) | – | – |

¹ A small zoom just before (fit + 30 %) had already started the unpack. That is the
"first zoom action" rule at work.

**Zoom and pan response** (RAW on, compared with RAW off, the M1 path):

| | RAW off | RAW on |
|---|---|---|
| Pan at 100 %, 8 drags × 10 frames: frame time avg / max | 16.7 / 17.3 ms | 15.5–16.7 / 16.8–17.5 ms |
| Pan: covered again after the drag stops | – | 16–49 ms |
| 80 fast zoom/pan actions at 20 ms: event-loop lag avg / max | 0.8 / 2.2 ms | 0.7–0.8 / 1.9–6.0 ms |
| 80 fast zoom/pan actions: frame time avg / max | 16.7 / 16.9 ms | 16.6–16.7 / 17.2–24.8 ms |
| 80 fast zoom/pan actions: covered after the last action | – | 16–17 ms |
| Existing zoom phase (step 3 scenario): frame time avg / max | 1.2–1.8 / 9–16 ms | 0.8–2.7 / 5–41 ms |

- Panning and fast interaction stay at 60 fps with or without tiles.
- The single frames up to 25–41 ms with RAW on fall within the spread seen between runs (the
  no-reuse and step-4b runs had 5–21 ms).

**Earlier phases are unchanged against step 3:**

- Contact sheet: median 0.6–0.8 ms.
- Loupe preview: median 2.3–3.6 ms.
- A7R IVA RAW image after arrival: 27–57 ms.
- Fast browsing lag: 7.8–9.2 ms average.
- Start-up: 49–60 ms (first run after a new build 11.8 s: virus scan of the new exe).
- Idle memory: 10.3–11 MB.

**CPU and memory:**

- **App memory.** During tile rendering the app peaks at about 425–470 MB: one kept session
  (126 MB) plus LibRaw's working memory for a tile and, at times, a half-size render running at
  the same time. While an A7R IVA image is zoomed, with its session and tiles kept, it sits at
  138–151 MB. With no session it is 10–20 MB.
- **WebView2 memory** in the tile phases is 550–830 MB with RAW on and 540–830 MB with RAW off.
- **CPU per tile:** about 80 ms of rendering + encoding. The 35-tile tile-start view costs about
  20 s of CPU over 2.9 s, since LibRaw's AHD runs on all cores. Reuse saves one unpack per zoomed
  image (pan phase 7.0–7.7 s of CPU with reuse, 9.9–10.3 s without).
- **Budget** (engine counters):
  - at most 1 kept session (budget 2);
  - tile cache at most 37 tiles / 12.3 MB (budget 192 MB);
  - at most 3 half-size images (budget 6);
  - no forced evictions;
  - 8 queued jobs dropped and 8–9 results thrown away in the fast zoom/pan scenarios.
  - Nothing is left queued or running after the scenarios.
- **Distribution:** the standalone folder ships `VCOMP140.DLL`, `VCRUNTIME140(_1).dll` and
  `MSVCP140.dll`, and the measurements ran from it.

### Deviations and compromises

1. **Covering the whole view where tiles start takes 2.8–2.9 s** (35 full-resolution tiles,
   ~80 ms each). The tile under the pointer is there after ~120 ms, and the rest fill in from
   it. Until then the screen-sized half-size image stays visible. This is the cost of starting
   tiles that early (approved). See the recommendation for step 5.
2. **Zooming to 100 % at once on an image just opened** waits for the unpack (~350 ms; RW2
   ~180 ms). The half-size job may be unpacking the same file at the same time; its session is
   then closed, not kept, because the tile already has one.
3. **The session reused from the half-size job stays in memory (126 MB) while the image is
   shown, even if the user never zooms.** It counts against the 2-session budget and goes when
   the budget needs room or the folder changes.
4. **Embedded preview under tiles (RW2 before any RAW image):** the preview's aspect can differ
   from LibRaw's by about 0.1 % (RW2 1920 × 1440 against 4592 × 3448). Tiles are scaled to the
   shown image, so at the edge of the covered area the preview may be offset by about 1 screen px
   until the area is covered. With the half-size RAW image underneath (A7R IVA) the geometry is
   exact.
5. **Measurement artefact:** turning RAW rendering off and on keeps the folder's generation. An
   unpack still running at that moment can come back as a kept session for the same image (a
   valid session; budget respected). This is why "50 %" without reuse looked as fast as with
   reuse.
6. **Not changed:** contact sheet, metadata, ingest and the M1 path with RAW rendering off (A1
   zoom = embedded full-size JPEG; no RAW calls).

## Step 5 – verification of the standalone distribution

**Method:**

- `tools\m2-step5.cmd` rebuilds `dist\PhotoWorkflow`, then `tools\verify-dist.ps1` copies the
  folder to `D:\PhotoWorkflowData\dist-test\PhotoWorkflow`. Everything below ran from that copy.
- **The environment:** the app was started with only Windows' own environment variables and
  `PATH = C:\WINDOWS\System32;C:\WINDOWS;…\Wbem;…\WindowsPowerShell\v1.0\`.
  - Removed from the development shell: `VCPKG_ROOT`, `LIBCLANG_PATH`, `JAVA_HOME`,
    `GHIDRA_INSTALL_DIR`, `LEVEL_ZERO_V1_SDK_PATH` and others.
  - Neither cargo nor ExifTool was reachable.
- **What was recorded:** the DLLs the process actually loaded (and from where), its child
  processes, threads, CPU and memory.
- **The runs:** RAW on and RAW off on the 23 test images, and a folder of broken files
  (`tools\m2-step5b.cmd` repeats only that one).
- **Dependencies:** listed in `docs/distribution.md`, together with what another Windows 11 PC
  needs.
- **Raw data:** `D:\PhotoWorkflowData\analysis\step5\` (`verify-dist.txt`, `verify-dist-broken.txt`,
  `bench-*.json`, `*-samples.csv`).

### Results

| # | Check | Result |
|---|---|---|
| 1 | All DLLs present | The folder holds exe + `VCOMP140.DLL`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`, `MSVCP140.dll` (byte-identical to dist). `dumpbin /dependents` on all 5 files: every import is in the folder or in Windows; **0 unresolved**. |
| 1 | Loaded at run time | The 4 MSVC DLLs are loaded **from the program folder** (not from System32, where the VC++ Redistributable also exists). The only other files are 44 Windows DLLs, the WebView2 runtime (`EmbeddedBrowserWebView.dll` 156.0.4314.58) and 2 hook DLLs injected by the antivirus. |
| 2 | LibRaw with OpenMP | `VCOMP140.DLL` is loaded from the folder. The app process has up to 87 threads with RAW on (41 with RAW off): the OpenMP pool. At the tile-start level it used 38 s of CPU in 11.5 s (≈ 3.3 cores). Half-size render 266 ms (step 3: 264 ms). |
| 3 | Half-size and tiles | A7R IVA: the RAW image appears 27–39 ms after arrival (the image was prepared as a neighbour); first RAW image cold 0.9 s. Tiles: 100 % at once 474–493 / 743–760 ms; after a pause 128–147 / 392–409 ms; tile-start level (35 tiles) 128–129 ms / 2.8 s; 200 % 126–139 / 216–227 ms. These are the same as step 4. |
| 4 | A7R IVA and RW2 | Both work. RW2 100 % at once 290–295 / 758–768 ms; after a first small zoom 98 / 565–572 ms. |
| 5 | A1 | Uses the embedded 8640 px JPEG ("large"). 0 RAW files opened, no tiles, `sessions_opened` unchanged across the A1 scenario. |
| 6 | RAW off = M1 | All engine counters stay 0. No half-size image, no tiles. A1 and A7R IVA zoom on the embedded JPEGs. The contact sheet, loupe and fast browsing behave as in earlier steps. |
| 7 | Error handling | See below: works for files LibRaw rejects; **a problem with truncated files**. |
| 8 | No development paths | The exe contains no reference to `C:\Development`, vcpkg, Visual Studio, `target\release`, a dev server or `D:\PhotoWorkflowData`. There are 275 crate source paths under `%USERPROFILE%\.cargo\registry\…` (panic and backtrace text only, never opened), and the PDB is named without a path. The program folder was **unchanged** after all runs. The app writes only its WebView2 profile in `%LOCALAPPDATA%\com.photoworkflow.app` (69.5 MB). The only child process was `msedgewebview2.exe`. |
| 9 | Memory and caches | RAW on: app private memory peaks at 426 MB, and drops back to 9–20 MB without a session. Engine counters: at most 1–2 kept sessions (budget 2), 37 tiles / 12.3 MB (budget 192 MB) and 3–5 half-size images (budget 6). Nothing is left queued or running at the end. RAW off: at most 125 MB. WebView2 is about 1.3 GB in both modes (its image cache, as in M1). |
| – | ExifTool missing | The app starts and works. The toast says "Kunde inte läsa betyg: ExifTool: could not start exiftool: program not found (is ExifTool on PATH?)". There were no script errors in any run. |

### Error handling with broken files (from the standalone copy)

| File | What happened | Assessment |
|---|---|---|
| JPEG renamed to `.ARW` (1200 px, smaller than the screen: RAW rendering is tried) | LibRaw: "open: Unsupported file format or not RAW file". `raw-failed` arrived; the image (M1 path) stayed, with no tiles and no hang. | **Correct fallback** |
| 3 MB random data as `.ARW` | The backend reports "format error: not a TIFF/RAW file". The app shows the message as a toast. | Correct, no crash. But the loupe keeps showing the **previous** image (and its tiles) while the status bar names the broken file. This is existing M1 behaviour of `showLoupe`, not new in M2. |
| A7R IVA cut to 2 %, 5 %, 10 % and 40 % of the file (preview intact, RAW data missing) | LibRaw opens, unpacks and renders **without an error**. The half-size image and tiles replace the embedded preview. | **Problem:** the RAW pixels are built from missing data, yet they replace an intact camera preview. |
| RW2 cut to 40 % | Same: tiles are rendered and no error is reported. | **Problem** (same cause) |
| The good A7R IVA file afterwards | RAW image 29–33 ms, tiles 444–449 / 698–705 ms. | Works |

**Cause:** LibRaw does not treat a short read inside the RAW data as an error. It counts it as a
"data error" and reports it through a callback (or on stderr, which a GUI app does not have), then
fills the missing data. The pipeline currently only checks LibRaw's return codes.

**Proposed fix (not made: outside step 5).** Register `libraw_set_dataerror_handler` in
`pwlr_open`. If LibRaw reports a data error during unpack or rendering, `pwlr` returns an error.
The engine then emits `raw-failed`, and the embedded preview (M1) stays with the explanation in the
badge — the existing fallback path. Also check that the file is at least as long as the RAW data
it declares. Test with the truncated files above (A7R IVA, RW2) in `pw-libraw`'s region tests and
in the broken-file run.

### Notes on the measurement itself

- The first broken-file run stopped. `bench.js` did not catch the rejected `image_info` of the
  garbage file and waited forever; the app itself was fine. Fixed in `bench.js` (only active with
  `--bench`):
  - errors are now recorded;
  - every phase writes what it has so far;
  - an unexpected error ends the run with a `fatal` entry;
  - a frame wait falls back to a timer when the window is covered.
- The frame fallback was never needed in the final runs (0).
- The test runs on the development machine, with a cleaned environment and a copy outside the
  repository. A PC that has never had Visual Studio or the VC++ Redistributable installed was not
  available. The loaded-module list shows that the app uses its own copies, so such a PC should
  behave the same. A final check by copying the folder to such a PC is still recommended.

## Error handling for damaged and unreadable RAW files (after step 5)

Step 5 found that truncated ARW and RW2 files were decoded by LibRaw **without an error**. Their
RAW pixels, built from missing data, replaced an intact embedded preview. The fix was approved on
2026-11-04. Scripts: `tools\m2-errors.cmd`, `tools\m2-errors2.cmd` and the final
`tools\m2-final.cmd`. Raw data: `D:\PhotoWorkflowData\analysis\m2-errors\`, `…\m2-errors2\` and
`…\m2-final\`.

### How LibRaw reports problems (LibRaw 0.22.3, headers from the vcpkg package)

| Channel | What it covers | Treated as |
|---|---|---|
| Return codes of open/unpack/process | Unsupported format, I/O errors, an end of file that a decoder notices (it throws internally) | Failure. It is "damaged" if a data error or missing bytes were seen as well. |
| `imgdata.process_warnings` (`LIBRAW_WARN_*`) | Camera white balance, colour profiles, dark frame, fallbacks to AHD, DNG SDK stages | **Harmless**: logged in the diagnostics, never makes a file damaged. All 24 test files: 0. |
| Data-error callback (`set_dataerror_handler`) / `error_count()` | A decoder that notices corrupt data or an unexpected end | Damaged |
| *(none)* | **Decoders that read past the end and fill the gap** (Sony ARW, Panasonic RW2 when cut inside the data, DNG lossless JPEG) | Detected by PhotoWorkflow itself (below) |

### What was changed

- **`crates/pw-libraw`: the shim is now C++ (`csrc/pwlr.cpp`; the old `pwlr.c` was removed by the owner).**
  - The file is opened through a subclass of LibRaw's own Windows data stream
    (`LibRaw_bigfile_buffered_datastream`). While `unpack()` runs, it counts every byte LibRaw
    asks for that the file does not have.
  - A file is refused with `damaged: RAW data incomplete, N bytes missing (file M bytes,
    truncated?)` if any byte is missing, or if LibRaw reports a data error.
  - `Session::open_with_diagnostics` returns warnings, data errors, short reads and missing bytes
    for tests and analysis.
  - The refusal happens at open/unpack, before any RAW pixels exist. So neither the half-size job
    nor a tile can produce anything for such a file, and the engine reports `Failed` once and
    never retries.
- **No tolerance for missing bytes.** The first version allowed 64 KiB, on the assumption that
  block-reading decoders run past the end of valid files. The measurement contradicted that:
  - All 24 complete test files (A7R IVA, A1, GM1 RW2, two kinds of DNG) lack **0 bytes**.
  - The 64 KiB tolerance missed a DNG cut to half (its decoder reads byte by byte, so only 41,662
    bytes were "missing") and cuts of 1–16 KiB at the end.
  - The tolerance is now 0.
- **`rawview.rs`: `raw_view` carries a sequence number.** A call that finishes after a later one
  (each inspects its file first) can no longer make an earlier image current in the engine again.
- **UI (`raw.js`, `rawtiles.js`, `app.js`, `style.css`)**
  - **Failed RAW (damaged or unsupported):**
    - The embedded preview stays; a `raw-ready` for a failed image is ignored.
    - There is no unpack and no tiles, and 100 % means the preview (M1).
    - A discreet chip in the loupe's top left: "⚠ Skadad RAW – visar inbäddad bild" or "⚠ RAW
      kunde inte avkodas – visar inbäddad bild". If the preview was cut off too, it says "ingen
      inbäddad bild att visa". The details are in its tooltip and in the badge.
  - **Unreadable file:** the loupe is emptied (no image or tiles of the previous file), the badge
    says "Filen kan inte läsas: …", and the RAW work for the previous image stops. Before, the
    previous image stayed on screen (M1 behaviour).
  - **Late results** (`raw-ready`, `raw-tile`, `raw-failed`, `raw-grid`) for an image other than
    the one on screen change nothing on screen. They are kept for that image.
- **Tests**
  - `pw-libraw/tests/damaged.rs` (real files):
    - all complete files open and are never called damaged;
    - A7R IVA, RW2, A1 and DNG files cut at 2–99 % or 1 KiB–1 MiB before the end are refused;
    - random data and a JPEG named `.ARW` fail without being called damaged.
  - Engine: 24/24, including a late failure of a left image.
  - UI: `raw-errors.test.js` 29/29 (damaged, unreadable, JPEG as RAW, no-preview damaged DNG, late
    events, 40 rapid switches).
  - The earlier UI tests pass.

### Results on the test RAWs (`cargo test -p pw-libraw`, thanos)

| File | Cut | LibRaw's own result | PhotoWorkflow |
|---|---|---|---|
| All 24 complete files | – | ok, 0 warnings, 0 data errors, 0 missing | **open** |
| A7R IVA (TN100426) | 2, 5, 10, 40, 90, 99 % and 1 KiB … 1 MiB before the end | success | **damaged** (9,600 … 61,046,400 bytes missing) |
| GM1 RW2 (P1240231) | 10 % | I/O error + 1 data error | **damaged** |
| GM1 RW2 | 40, 90, 99 %; 1 KiB … 1 MiB before the end | success | **damaged** |
| A1 (TN005956) | 10, 50, 90 %; 1 KiB … 1 MiB before the end | success | **damaged** |
| Xiaomi DNG | 50 % | success | **damaged** (41,662 bytes missing) |
| Xiaomi DNG | 1 KiB … 1 MiB before the end | success, 0 missing | open: only data *after* the RAW data was cut, so the image is complete |
| 3 MB random data `.ARW` | – | "Unsupported file format" | failed, not damaged (pw-raw already refuses it in the app) |
| JPEG named `.ARW` | – | "Unsupported file format" / I/O error | failed, not damaged |

### From the standalone folder (copy, cleaned environment; `verify-dist` "on" and "broken")

- **RAW on, 23 test images:**
  - **no image classed as failed or damaged**;
  - the A7R IVA RAW image appears 30–40 ms after arrival;
  - tiles: 100 % after a pause 106 / 362 ms, at once 438 / 691 ms; tile-start level 106 ms /
    2.7 s; RW2 302 / 758 ms;
  - the A1 gets no RAW work.

  These are the same as steps 4–5. No script errors, the program folder was unchanged, and every
  import is resolved.
- **Broken-file folder:**

  | File | Loupe | Chip |
  |---|---|---|
  | A7R IVA cut 2 / 5 / 10 / 40 / 99 % | embedded preview, no tiles | ⚠ Skadad RAW – visar inbäddad bild |
  | RW2 cut 40 / 90 % (found on the zoom to 100 %: RW2 needs no half-size image) | embedded preview, no tiles | ⚠ Skadad RAW – visar inbäddad bild |
  | DNG cut 50 % (its preview is at the end of the file and was cut off too) | empty, badge explains | ⚠ Skadad RAW – "ingen inbäddad bild att visa" in the final build (before: "visar inbäddad bild") |
  | JPEG named `.ARW` | the image itself | ⚠ RAW kunde inte avkodas – visar inbäddad bild |
  | Random data `.ARW` | **empty**, "Filen kan inte läsas: format error: not a TIFF/RAW file" | – |
  | Good A7R IVA afterwards | RAW image in 36 ms, tiles covered in 643 ms | none |
  | 60 rapid switches over all files, then stops on good, damaged, unreadable and RW2 files | each stop shows its own image and state; no tiles or warnings of another image | correct per file |

### Limits

- **A RAW file damaged inside its data without being shorter** (flipped bits) is caught only if
  the decoder notices it (LibRaw data error). Otherwise it shows as noise in the RAW image, as in
  any RAW converter. A checksum is not possible: RAW files carry none for their image data.
- **RW2 files** get no half-size image on a 1920 px screen, so their damage is noticed on the
  first zoom (the unpack). Until then the preview is shown, which is the correct M1 behaviour.
- **Files with formats or cameras outside the test set:** a decoder that legitimately reads past
  the end would make a complete file look damaged. None of the 24 test files does this. The result
  would be the M1 preview with a warning chip, never wrong pixels.
