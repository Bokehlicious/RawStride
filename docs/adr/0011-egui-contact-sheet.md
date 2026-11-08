# ADR 0011: Milestone 3 – the contact sheet in the egui program

- Status: **accepted by the owner (2026-11-08, step K0).** The two points of §2, the answers to §11
  (the proposals, accepted as written) and C1–C5 as behaviour requirements (§7; targets after the
  first measurement) are decided. Nothing is built, installed or added as a dependency by this ADR.
  Each step in §9 is its own work part, with its own approval and commit.
- Date: 2026-11-08
- Related:
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) (GR1–GR13; GR13 is this milestone's owner requirement);
  - ADR 0009 (milestone 1, the loupe; §3.2 lists the contact sheet as a later milestone; §3.3 reserves
    Escape, G and Enter for the contact sheet);
  - ADR 0010 (milestone 2, import and folder tabs; its steps M3–M7 now follow this milestone);
  - ADR 0003 (M1 viewer, the Tauri contact sheet) and `ui/web/app.js` (today's behaviour);
  - [requirements-image-viewer.md](../requirements-image-viewer.md): V1–V8 cover the loupe only; the
    contact sheet "has its own requirements later" (§7 here proposes them).

## 1. Context

- The egui program `pw-loupe` has folder tabs (ADR 0010 M2) and shows every opened folder in the
  loupe, one image at a time.
- The Tauri app opens a folder as a contact sheet: a grid of thumbnails, loaded as they come into view,
  with a size slider; Enter, G, Space or a double-click open the loupe, and Escape, G or Enter go back.
- The owner wants the egui program to work the same way: **a folder that is opened (dropped on the
  window or imported from a card) is shown as thumbnails first, not in the loupe.**
- The thumbnail source is already available in the core:
  - `pw-raw` finds the embedded previews of a RAW file and reads one (`inspect`, `read_preview`);
  - `pw-libjpeg` decodes a JPEG at full, half, quarter or eighth size.
- `pw-thumbcache` exists only as a placeholder ("thumbnail and preview cache, planned").

## 2. Decisions fixed by the owner (2026-11-08)

1. **An opened folder is shown as a contact sheet.** This applies to a folder dropped on the window
   and to a folder opened by the import (GUI_REQUIREMENTS GR13).
2. **The contact sheet comes before the rest of the import.** ADR 0010 steps M3–M7 follow after this
   milestone. The import then opens its result in a new tab, in the contact sheet.

## 3. Scope

### 3.1 In scope

| Area | Content |
|---|---|
| Grid | All images of the tab's folder as a grid of square cells, in the folder's order. The number of columns follows the window width and the thumbnail size |
| Thumbnails | From the **embedded preview** only (no RAW decoding): the smallest embedded preview that is at least the cell size, decoded at the smallest JPEG scale that still covers the cell, with the file's orientation applied. A plain JPEG file is decoded from the file itself |
| Missing preview | A file without a usable embedded preview shows a neutral placeholder with the text "ingen förhandsvisning" (as today), never a RAW rendering |
| Damaged files | A file that cannot be read shows an error placeholder; it never stops the other thumbnails (G3) |
| Loading order | Visible cells first, then a margin around them; work for cells that scrolled far away is dropped; a new folder or tab cancels the old work |
| Thumbnail size | A slider in the window; Ctrl + mouse wheel over the grid changes it too (§11, Q2) |
| Selection | One current image, highlighted. Clicking a cell makes it current |
| Keys in the grid | Arrows (left/right by one, up/down by one row), Page Up/Down (one screen), Home/End; **Enter, G or Space** open the loupe on the current image; a double-click does the same |
| Keys in the loupe | **Escape, G or Enter** return to the grid, with the image that was shown in the loupe as the current one (the reserved keys of ADR 0009 §3.3) |
| Tabs | Each tab remembers its mode (grid or loupe) and its current image; a newly opened folder starts in the grid |
| Information | File name of the current image, its position and the count, in the status line |
| Memory | Thumbnail textures count against the existing texture budget (about 1 GB, GUI_REQUIREMENTS §7); thumbnails of other tabs are released |

### 3.2 Out of scope

- Ratings and labels under the thumbnails, filters and multiple selection (ADR 0004; a later milestone).
- A **disk cache** of thumbnails (`pw-thumbcache`'s planned disk part). Thumbnails are kept in memory for
  the open tab only, until a measurement shows a disk cache is needed (§11, Q4).
- Folders much larger than the test set (1 000+ images); they stay deferred, as for the loupe.
- TIFF thumbnails (TIFF viewing is a later milestone).
- Sorting other than the folder's order.

## 4. Architecture

### 4.1 Where the code goes

| Part | Crate | Notes |
|---|---|---|
| Thumbnail extraction: choose the embedded preview, decode at the right scale, apply orientation | `pw-thumbcache` | Plain Rust on top of `pw-raw` and `pw-libjpeg`; testable with synthetic files and with the real-file tests (ignored by default) |
| Thumbnail worker: a small pool of background threads, a priority queue (visible first), cancel, an in-memory cache of finished thumbnails per folder | `pw-thumbcache` | Like `pw-viewcore`'s workers: the UI never decodes; results are polled |
| Grid model: columns from width and size, the visible range from the scroll position, key navigation, mode switching | `pw-loupe` (library) | Plain Rust, no drawing (GR11) |
| Drawing: the grid in a scroll area, thumbnail textures, the slider, the switch between grid and loupe | `pw-loupe` (binary) | egui |

`pw-thumbcache`'s description changes from "content-addressed thumbnail and preview cache" to
"thumbnail extraction, background generation and an in-memory cache; a disk cache later".

### 4.2 Interaction with the loupe

- The loupe's view core (`pw-viewcore`) is only given the folder when the loupe is opened, so the grid
  does not compete with the loupe for decoding. While the loupe is shown, thumbnail work pauses.
- Opening the loupe from the grid: `open_folder(files)` and `go(current)` on the view core, as a tab
  switch does today (ADR 0010).

## 5. Behaviour requirements

- **G2 / GR2:** scrolling, key presses and resizing never wait for a thumbnail. No file I/O or decoding
  on the UI thread.
- **G1:** a thumbnail is always the embedded preview (or the JPEG file itself); the grid never shows a
  RAW rendering.
- **GR6:** everything in §3.1 works from the keyboard; focus returns to the grid after the loupe.
- **GR11:** extraction, the worker protocol and the grid model have `cargo test` tests.
- **Orientation** is applied once, from the file's orientation tag, as in the loupe (ADR 0003 point 4).

## 6. Dependencies

- No new crates. `pw-thumbcache` gains dependencies on `pw-raw` and `pw-libjpeg` (internal), and
  `pw-loupe` on `pw-thumbcache` (internal).

## 7. Contact sheet requirements (accepted as behaviour requirements, 2026-11-08)

The image viewer requirements leave the contact sheet "for later". These are proposed as **behaviour
requirements now and targets after a first measurement**, so no number is invented (TESTING.md §2):

| # | Requirement | Target |
|---|---|---|
| **C1** | Opening a folder shows the grid at once, with placeholders, before any thumbnail is ready | Measured in K5; target set by the owner afterwards |
| **C2** | The visible thumbnails of a folder of the test set (31 files) are complete within a short time | Measured in K5 (time until the last visible thumbnail is shown); target set by the owner afterwards |
| **C3** | Scrolling and key navigation keep the display frame rate while thumbnails load (G2) | Checked by hand in K5 |
| **C4** | Grid → loupe on the current image, and back, follow the loupe's V1/V5 rules for the image shown | V5 applies to the first image shown in the loupe |
| **C5** | A thumbnail is never a RAW rendering, and never shows the wrong image for a cell | Tests and checked by hand |

## 8. Testing and acceptance

### 8.1 Automated

- `pw-thumbcache`: preview choice for each size; scale choice; orientation for all eight values;
  placeholder for a file without a preview; a damaged file gives an error, not a panic; the worker's
  priority and cancel with a simulated decoder; real-file tests (ignored by default) for the camera
  formats of the test set.
- `pw-loupe`: grid columns and visible range for several widths and sizes; navigation at the edges
  (first row, last row, a short last row); grid ↔ loupe with the current image kept; a tab remembers
  its mode.

### 8.2 By hand (owner)

- Drop the test folder: the grid appears at once and fills in; the time until all visible thumbnails
  are shown is noted (C1, C2).
- Scroll quickly and change the size while thumbnails load (C3).
- Enter / double-click into the loupe and Escape back, on several images and in several tabs (C4).
- Check orientation on the A7R IVA vertical images and a DNG.

## 9. Work steps

| Step | Content | Result |
|---|---|---|
| **K0** | Accept this ADR; answer §11; decide C1–C5 as behaviour requirements | **Done 2026-11-08:** documentation commit |
| **K1** | `pw-thumbcache`: thumbnail extraction (preview choice, scaled decode, orientation, placeholder, errors) | Code + tests |
| **K2** | `pw-thumbcache`: worker pool, priority queue, cancel and in-memory cache | Code + tests |
| **K3** | `pw-loupe`: grid model (layout, visible range, keys, mode per tab) in plain Rust | Code + tests |
| **K4** | `pw-loupe`: draw the grid, upload thumbnails, slider, grid ↔ loupe, dropped folders open in the grid | Code + tests |
| **K5** | Acceptance by hand and the first measurement for C1–C2 | Results noted in the log; the owner sets the targets |
| **K6** | Documentation: architecture.md (`pw-thumbcache`), test inventory, ADR 0010 (imports open in the grid) | Documentation commit |

Steps K1, K2 and K3 are on the agent task list as T23, T24 and T25; K4 is larger and may be split.

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Some formats have only a large embedded preview (e.g. a phone DNG with one 4096 px JPEG) | Slower thumbnails for those files | The eighth-scale decode keeps the work small; measured in K5 |
| Many textures for a large folder | Texture memory | Only visible cells and a margin keep textures; the rest are released |
| Thumbnails and the loupe compete for the disk or CPU | A slower loupe | Thumbnail work pauses while the loupe is shown (§4.2) |
| Orientation errors in thumbnails | Wrong-looking grid | Orientation tests for all eight values and the A7R IVA real files |

## 11. Questions and the owner's answers (2026-11-08)

The owner accepted every proposal below as written.

| # | Question | Decision (the proposal, accepted) |
|---|---|---|
| **Q1** | What does the command line open? | A folder opens in the grid; a single file opens the loupe on that file (useful for `latency.ps1`, which can also double-click a thumbnail first) |
| **Q2** | Thumbnail sizes | Long edge from 96 to 480 px, default 200 px, changed with the slider or Ctrl + mouse wheel; kept for the session only |
| **Q3** | Text under a thumbnail | The file name, shortened when it does not fit. Ratings and labels come with their milestone |
| **Q4** | Disk cache now or later | Later, only if K5 shows that thumbnails of the test set are too slow from the embedded previews |
| **Q5** | Does a tab remember grid or loupe? | Yes; a new folder always starts in the grid |
| **Q6** | Escape in the grid | Does nothing for now (it will clear a multiple selection when that exists) |

## 12. Consequences

- The egui program opens folders the way the Tauri app does, closer to GR8.
- `pw-thumbcache` becomes real code: extraction, workers and an in-memory cache.
- ADR 0010's import, when finished, opens its result in the grid of a new tab.
- The reserved keys Escape, G and Enter of ADR 0009 §3.3 get their planned meaning.
