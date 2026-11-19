# Image viewer – performance and behaviour requirements (V1–V8)

- Status: **approved by the owner (2026-11-04).** Any architecture work against these requirements
  starts with an ADR. Changes to targets need the owner's approval.
- Revision 2026-11-04 (owner's decision): **G4 added** (two manually chosen display modes, RAW and JPEG).
  V1–V8 and G1–G3 are unchanged.
- Revision 2026-11-05 (owner's decision): **V2 clarified** with a permitted loading indicator after 1 s
  (no intermediate image; one direct swap). The targets are unchanged.
- Scope: the loupe/preview of a single image and stepping between images. The contact sheet, ingest
  and metadata have their own requirements later.
- Goal (the owner's): **step between images and zoom to real 100 % without any visible delay in
  normal use.**
- Sources:
  - our own measurements of a reference workflow on the development PC, recorded outside this
    repository under the [clean-room](clean-room.md) rules;
  - our own baseline (below).

## Definitions

| Term | Meaning |
|---|---|
| **Response time** | Time from the key going down until the image area on screen stops changing (last change). Measured on screen pixels (see "Method") |
| **Intermediate frame** | Any frame between the old and the final new view: blank, wrong image, low resolution or blurred upscale |
| **Embedded preview** | The largest JPEG the camera stored in the file |
| **Real 100 %** | One sensor pixel per screen pixel, from RAW data or from an embedded JPEG of full sensor size |
| **Fit** | The whole image fitted to the image area |
| **Normal use** | Stepping at up to about 7 images/s, zooming, panning; the image shown is in a folder of up to 270 images (larger folders: see "Not covered") |

## Requirements

| # | Requirement | Target | Our baseline 2026-11-04 (dist build, M2) |
|---|---|---|---|
| **V1** | Next/previous image | Median response time **≤ 30 ms**, at fit **and** at 100 %, for every format whose embedded preview is shown | At fit, per format (ranges over runs `pw-compare` 3 and `pw-raw100`, not averages):<br>- Sony A1 ARW: 16–34 ms<br>- Panasonic RW2: 28–34 ms<br>- Sony A7R IVA ARW: first change 32 ms, still after 81–97 ms (2 changes)<br>- Xiaomi 17 Ultra DNG (4096 px preview): **67–99 ms**<br>- Ricoh DNG (6000 × 4000 preview): **141–185 ms**<br>At 100 %: not measured (zoom is reset, V6) |
| **V2** | No intermediate frames | The previous image stays on screen until the new view is complete at the current zoom and position. One swap, no blur-to-sharp step. A file that cannot be read may show its error message instead<br>**Permitted exception: loading indicator after 1 s** (owner, 2026-11-05):<br>- No blurred, low-resolution or unfinished image may ever be shown as an intermediate state.<br>- The view changes directly to the finished image as soon as it is ready.<br>- If the finished image is not ready after **1 s**, a **neutral loading indicator** may be shown. It contains no image content of the new image.<br>- The indicator never blocks stepping, zooming or other user actions.<br>- The indicator does not change the requirement of one single, direct swap.<br>This is a permitted exception to V2, not a lowering of the image-quality requirement | Not met at the first zoom: a blurred upscale first, then sharp |
| **V3** | Fit ↔ 100 % with a full-size embedded JPEG | Response time **≤ 30 ms**, also the first time for an image | Latest measurement of sharp 100 %: **682–699 ms** (Sony A7R IVA, RAW tiles, first change 17–33 ms). For a full-size embedded JPEG (Sony A1): first time 435 ms (blurred after 33 ms); later 15–31 ms |
| **V4** | Rapid stepping | At about 7 images/s and 100 %: each step **≤ 70 ms**; never an image other than the current one | Not measured at 100 % (zoom is reset, V6) |
| **V5** | First image of a folder | Shown at the current zoom in **≤ 150 ms** after the folder is opened or the preview starts | Not measured with the pixel method |
| **V6** | Zoom and position persist | The zoom level and the position in the image stay when stepping to the next/previous image.<br>The position is adapted so that it is valid and natural for the next image:<br>- it is kept as the same relative point of the image (for example the middle of the left third), taking each image's orientation into account;<br>- it is moved as little as needed so that the view stays inside the image;<br>- an image smaller than the window at that zoom is centred.<br>"Fit" stays fit, and 100 % means 100 % of the new image | **Not met**: every image change returns to fit |
| **V7** | Real 100 % from RAW | For formats whose embedded preview is smaller than the sensor (Sony A7R IVA, Panasonic RW2, phone DNG):<br>- real, **sharp** 100 % from RAW data within **≤ 30 ms** after the zoom command;<br>- this also applies when the user zooms at once, without waiting on the image.<br>This is a performance target. It does not prescribe a technical solution | **Measured:** real 100 % exists (M2 tiles). After a 3–4 s pause at fit: first change 17–33 ms (upscaled, not sharp); **sharp after 682–699 ms**.<br>**Not measured:** zooming immediately on arrival |
| **V8** | Memory budget | The memory for decoded images and RAW data has a user-visible, configurable limit (for example a share of RAM). The viewer never exceeds it, and V1–V7 apply within it | Fixed budgets in code (ADR 0006); not configurable |

General:

- **G1 – Honest source.** The viewer always says what is shown: embedded preview (with size) or RAW,
  and when RAW data is still being prepared. This already exists (M1/M2) and must be kept.
- **G2 – Responsiveness.** Slow work never blocks key input, zooming or panning: decoding, RAW
  rendering, metadata writes, ingest. Animations and panning keep the display frame rate.
- **G3 – Damaged files.** The behaviour of M2 is kept: an intact preview is never replaced by bad RAW
  data, and an unreadable file never leaves the previous image on screen.
- **G4 – Display mode, chosen by the user.** There are exactly two display modes:
  - **RAW** – real RAW rendering;
  - **JPEG** – the camera's embedded JPEG.
  
  Rules:
  - The user switches between them manually, with a key and a control in the viewer.
  - **The program never chooses or changes the display mode by itself.** There is no automatic mode.
  - The chosen mode stays when stepping between images and when zooming (as V6).
  - **RAW:** only RAW renderings are shown. Until the RAW view of an image is complete, the rules of V2
    apply, and G1 says that the RAW view is being prepared. The embedded JPEG is not shown as a substitute.
  - **JPEG:** only the embedded JPEG is shown, and 100 % means 100 % of that JPEG. G1 states its size.
    A file without an embedded JPEG shows a message saying so, not the RAW rendering.
  - V7 applies in RAW mode. V1 and V3 apply in both modes.

## Method

- **Tool:** a script outside the repository (`D:\PhotoWorkflowData\pm-study\tools\latency.ps1`).
  The measurement tools and their results are the owner's own and are not published (owner's
  decision, 2026-11-19, `docs/tasks/plans/public-release.md` PR-Q6); this section describes what the
  script does, so the measurement can be repeated with another tool. The paths below are the
  owner's machine.
  - It sends a key to the window under test.
  - It samples the image area's screen pixels every capture (about 10–20 ms).
  - It reports the first and the last change, and the number of changes, until the screen has been
    still for a set time.
  - It can double-click a thumbnail first and checks that the view changed.
- **Machine:** the development PC (i7-9700K, 32 GB, Arc B580, Windows 11), one 1909 px wide screen,
  window maximised.
- **Test files:** `D:\PhotoWorkflowData\RAW`, 31 files:
  - Sony A1, Sony A7R IVA (compressed RAW);
  - Panasonic GM1 RW2;
  - Xiaomi 17 Ultra DNG;
  - Ricoh DNG.
- **Runs:** `pw-compare.cmd` (stepping and Z) and `pw-raw100.cmd` (A7R at 100 %). Results are in
  `D:\PhotoWorkflowData\pm-study\results\`.
- **Resolution:** one capture interval. Values below about 15 ms cannot be separated.
- **Limits:**
  - The window title carries no file name, so steps are mapped to files by their order.
  - The memory and CPU of the app are not recorded, because the drawing happens in the WebView2
    processes.

## Not covered (later)

- **Large folders:** 1 000, 5 000 and 10 000 images. These tests are deferred, and no conclusion about
  large folders is drawn from the measurements above.
- **Contact sheet:** thumbnail speed and the grid.
- **Window management:** floating windows and user-controlled window and panel placement (for
  example on a second screen) are a **future product requirement**. They are not part of V1–V8.
- **Other machines:** HDD sources and other GPUs.
