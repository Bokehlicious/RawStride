# ADR 0009: Migration to the eframe + egui GUI – milestone 1, the loupe program

- Status: **accepted by the owner (2026-11-07, step S0).** The six questions of section 11 are
  decided. Nothing is built, installed or added as a dependency by this ADR. Each step in section 9 is
  its own work part, with its own approval and commit.
- Date: 2026-11-07
- Related:
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) (GR1–GR11; §6 versions and features; §7 renderer and
    budget);
  - ADR 0008 (image presentation): §2 the common core, §11 PPG/AHD, §12 strategy p5 and the 4 GB
    budget, §16 the eframe + egui decision, §18 P7 and the renderer decision;
  - ADR 0003 (M1 viewer, amended), ADR 0006 (RAW pipeline M2);
  - [requirements-image-viewer.md](../requirements-image-viewer.md) (V1–V8, G1–G4), which is
    unchanged;
  - [TESTING.md](../TESTING.md) and AGENTS.md.
- Basis: P7 (`prototypes/gui/p7-egui-presenter`, commit `1dff27c`); phase 1 (P0, P4, P5).

## 0. How claims are marked

As in ADR 0008:

- **[M]** measured;
- **[D]** documented by the supplier;
- **[T]** estimate or reasoning;
- **[P]** must be shown in this milestone, by a test or a measurement.

## 1. Context

- **The owner has decided** that the GUI is pure Rust with eframe + egui (ADR 0008 §16), with glow as
  the renderer and separate budgets for the core and the textures (ADR 0008 §18).
- **P7 showed that the approach works on screen [M]:**
  - views prepared by P5's strategy and drawn as egui textures met 30 ms in 63 of 64 presses with glow
    (Z, a prefetched image change, pan);
  - the hit rates matched P5;
  - a texture cost about 1.2× its size in RAM.
- **P7 is a prototype, not a product.**
  - It measures RAW mode only, with no JPEG mode (G4).
  - It takes its files from the command line.
  - It has no error display for damaged files (G3).
  - It uses its own LibRaw shim instead of `pw-libraw`.
  - Its budgets are fixed in code (V8).
- **The current Tauri app must keep working** until the egui GUI covers its features and passes
  acceptance (GR8).
- **GUI_REQUIREMENTS §5 left the migration order open.** This ADR settles it for the first milestone
  and describes how the core is moved from the prototype into production crates.

## 2. Decisions fixed by the owner (2026-11-07)

These three points are given. This ADR plans their implementation and does not reopen them. The
owner's answers to the questions of section 11 are recorded there.

1. **Milestone 1 is the loupe only.** It is a separate program beside the Tauri app. The Tauri app is
   not changed.
2. **wgpu is removed entirely from the eframe feature list.** Only glow is used.
3. **The view core becomes its own production crate, `pw-viewcore`,** built on the P7 logic.

## 3. Scope of milestone 1

### 3.1 In scope

| Area | Content | Requirement |
|---|---|---|
| Open a folder | One folder at a time. The file types of `pw_raw::SUPPORTED_EXTENSIONS`, as in the current app, except TIFF (later). The folder comes from the command line or is dropped on the window. There is no dialog (§11, decision 2) | V5 |
| Stepping | Next, previous, first and last, with the keys of section 3.3 | V1, V4 |
| Zoom | Fit ↔ 100 % with Z. The 100 % view is real sensor pixels in RAW mode | V3, V7 |
| Pan | Dragging with the mouse at 100 %, as in the current viewer (section 3.3) | V6, G2 |
| Zoom and position persist | Kept as the relative point in the oriented image, clamped, and centred for small images | V6 |
| Display mode | RAW or JPEG, switched by the user with R and a control. Never changed by the program | G4 |
| Swap rule | One direct swap. The previous view stays until the new one is complete, with a neutral indicator after 1 s | V2 |
| Honest source | A status line or label: RAW rendering (PPG), or embedded JPEG with its size; and "RAW being prepared" | G1 |
| Damaged files | An error message instead of an image. An intact preview is never replaced by bad RAW data | G3 |
| Orientation | Always from the RAW file's orientation tag, applied once, as texture coordinates per corner | ADR 0003 point 4 |
| Memory | Core budget 4 GB and texture budget about 1 GB, counted separately. Both are user settings, given as command-line arguments (§11, decision 3) | V8, ADR 0008 §18 |
| Keyboard | The current viewer's keys (section 3.3). Every culling action works from the keyboard (GR6). Focus survives minimise, restore and resize | GR6, GR7 |
| Window | Binary `pw-loupe`. The window title is **"PhotoWorkflow Loupe"**, which `latency.ps1` targets (§11, decision 4) | – |

### 3.2 Out of scope (later milestones)

- The contact sheet and thumbnails.
- Ratings, labels, filter and multiple selection (ADR 0004).
- The ingest dialog (ADR 0005).
- IPTC metadata (ADR 0007), apart from the few EXIF fields `pw-raw` already reads, which the status
  line may show.
- TIFF viewing.
- A settings window and a settings file. The budgets are command-line arguments in milestone 1
  (§11, decision 3).
- A folder dialog (§11, decision 2).
- DPI change and a move to another screen. These are postponed by the owner, but they are still part
  of acceptance for the whole GUI (GR7).
- GPU demosaicing (ADR 0008 §16.4, point 3).
- Removing or changing the Tauri app, `pw-rawview` or `ui/web`.

### 3.3 Keys (§11, decision 5)

The loupe keeps the keys of the current viewer (`ui/web/app.js`, loupe mode).

| Key | Action in milestone 1 |
|---|---|
| Right, Down, Page Down | Next image |
| Left, Up, Page Up | Previous image |
| Home / End | First / last image |
| Z, double-click | Fit ↔ 100 % |
| F | Fit |
| A | Zoom anchor: at the pointer or at the image centre |
| I | Show or hide the information |
| Mouse drag at 100 % | Pan |

Notes:

- **Keys without a function in milestone 1 are reserved and do nothing.** Their meaning stays the same
  for later milestones:
  - Escape, G and Enter (to the contact sheet);
  - Ctrl+O (folder dialog);
  - Ctrl+A (select all);
  - digits and Ctrl+digits (ratings and labels).
- **Free zoom** (+, − and the mouse wheel) exists in the current viewer. Milestone 1 has fit and 100 %
  only, the views of ADR 0008 §12. Free zoom levels need a view type of their own and are planned
  with a later milestone.
- **The display mode (G4) needs a key** (G4: "with a key and a control"). The current viewer has only
  a button for it. The loupe uses **R**, which is free in the current viewer, plus a control in the
  window.
- **Pan by keyboard** does not exist in the current viewer. P7 measured pan with D and A, but A is the
  anchor key here. The on-screen pan measurement (section 8.2) therefore uses mouse drags, or a key
  that is active only in the measurement mode of section 5.2.

## 4. Architecture

### 4.1 Crates and the program

| Crate | Status | Content in milestone 1 |
|---|---|---|
| **`pw-viewcore`** | **New** library | The view core from P7, at production quality (section 5). No egui, no FFI, no GUI types. The decoders are traits, so it can be tested without native libraries |
| `pw-libraw` | Extended, **additive only** | A demosaic choice: **PPG** for the egui loupe, while today's AHD stays the default for existing callers (section 7). The fixed settings of ADR 0006 are otherwise unchanged |
| `pw-raw` | Used as is | Locates embedded JPEG previews and reads orientation and EXIF (ADR 0003) |
| **`pw-render`** | **Repurposed** (today an empty stub described as wgpu) | Image presentation in egui: texture upload, residency within the texture budget, and placement and texture coordinates per corner. Depends on `egui` only, not eframe (section 6) |
| **`pw-loupe`** | **New** program (binary crate) | The window (eframe with glow), keyboard and mouse input, the status line, the indicator and error display, and the settings. It wires `pw-viewcore` and `pw-render` together |
| `pw-rawview` | **Unchanged** | Stays the engine of the Tauri app (ADR 0006) |
| `src-tauri`, `ui/web` | **Unchanged** | The current app (GR8) |

- **Location:** `crates/pw-viewcore`, `crates/pw-render` and `crates/pw-loupe`. The workspace already
  takes `crates/*`, so no change to the workspace member list is needed. The new external crates are
  pinned once in `[workspace.dependencies]`.
- **Deviation from ADR 0008 §2.1.** That section planned `pw-viewcore` "built on `pw-rawview`".
  - The owner's decision 3 takes the P7 logic instead: views per P5, exact regions, and two workers
    with one budget.
  - `pw-rawview` uses grid tiles and JPEG-encoded output for WebView2, which ADR 0008 §12 replaced
    with exact regions as raw RGB.
  - `pw-rawview` therefore stays with the Tauri app. Whether it is removed after the switch is decided
    with GR8.

### 4.2 Data flow

1. **`pw-loupe` turns input into core calls.** A key press or a mouse drag becomes a call on
   `pw-viewcore`: `go`, `zoom`, `pan`, `set_mode` or `open_folder`. It returns at once (G2).
2. **The core decides what each view needs (P5's plan).**
   - It decodes and renders on two worker threads: one interactive, for the current image only, and
     one background.
   - The decoders are `pw-libraw` with PPG for RAW and a JPEG decoder for embedded previews
     (section 6).
3. **The core hands over a finished view.** That is an RGB image with its kind (fit, zoom or ring),
   its region, its image and its generation. It is sent through a channel, and the core wakes the UI
   with `request_repaint`.
4. **`pw-render` uploads finished views on the UI thread,** within the texture budget.
   - It asks the core which views are still wanted, so the current image is never evicted.
   - It draws the current view only when its texture is resident. Otherwise the previous view stays
     (V2).
5. **`pw-loupe` draws the frame:** the image, the status line, and the indicator or error.

## 5. `pw-viewcore`: from the P7 prototype to production

### 5.1 Carried over from P7 [M]

These parts were measured in P7. They are rewritten to production quality, not copied as they are.

- **The view model:**
  - the current image;
  - zoom (fit or 100 %);
  - the position as a relative point in the oriented image, clamped (V6);
  - orientation by flip 0/3/5/6, with the screen-to-sensor mapping (`view.rs`).
- **The views:** fit at half size, the exact visible 100 % region, and the ring of twice that region
  for panning. Their sensor boxes have an even origin, to keep the Bayer phase.
- **Scheduling, strategy p5:**
  - the 150 ms dwell;
  - the order: current view, the other view, +1 and +2 in the direction of travel, ring, −1;
  - admission: prefetch only if it fits;
  - LRU eviction of what is no longer wanted; the current image is never evicted;
  - folder generations, so that old results are thrown away;
  - miss classification, used for measurement.
- **Two workers**, interactive and background, as in P5 and P7.
- **The texture-side accounting:** `accept` and `reject`, protected positions, and an overshoot only
  for the current image.
- **The P7 tests** of view geometry and scheduling: 18 tests, as the starting point.

### 5.2 New in production

1. **JPEG mode (G4)** [P].
   - The embedded preview is found by `pw-raw` and decoded off the UI thread with libjpeg-turbo
     (§11, decision 1). For the fit view it uses scaled decoding (1/2 to 1/8). The decoded image
     counts in the core budget.
   - The views are derived from it: a fit view scaled down to the window, and a 100 % region of the
     JPEG itself. 100 % means 100 % of the JPEG.
   - A file without an embedded JPEG shows a message, never the RAW rendering.
   - The scheduling order follows ADR 0008 §12: the other display mode comes after the neighbours.
   - Plain JPEG files are shown in both modes as what they are.
2. **Damaged and unreadable files (G3).**
   - `pw-libraw` reports damaged files through `open_with_diagnostics`. The core reports them as an
     error view.
   - An unreadable file never leaves the previous image on screen.
3. **Settings (V8).**
   - Both budgets are user settings, given as command-line arguments: the core with 4 GB by default,
     and the textures with about 1 GB. There is no settings file yet.
   - The worker count and the dwell stay constants unless the owner decides otherwise.
4. **Errors instead of crashes (AGENTS.md).**
   - No `unwrap`, `expect` or `panic!`.
   - A poisoned lock is taken over and logged, as in P7.
   - Worker errors become error views, not lost threads.
   - Shutdown is orderly: the workers stop and the LibRaw sessions close. P7's hard exit was a
     workaround for wgpu and is not carried over.
5. **Measurement hooks.** The event record of P7 (request, hit or miss, shown) stays behind a
   measurement switch, so that P5-style runs can be repeated against the product [T].
6. **A JPEG decoder trait.** It mirrors the RAW backend trait, so that both decoders can be simulated
   in tests.

### 5.3 What stays out of `pw-viewcore`

- egui and eframe types, textures and drawing. Those go in `pw-render` and `pw-loupe`.
- Files and FFI. Those go through `pw-libraw` and `pw-raw`, behind traits.
- Thumbnails, the catalog and metadata writes.

## 6. Dependencies

### 6.1 eframe and egui (owner decisions 2026-11-07)

- **eframe `=0.37.0`, `default-features = false`.** Features: **`glow`, `accesskit`, `default_fonts`,
  `clipboard`**. `wgpu` is removed (decision 2).
- **egui `=0.37.0`**, the same version that eframe uses. It is used in `pw-render` with default features
  off, and only what `pw-render` needs. It brings no new crate into the tree beyond eframe's.
- **GUI_REQUIREMENTS §6** is amended in step S0: `wgpu` is removed from the feature list.
- **The licence review is checked again** for the smaller tree, without wgpu and its dependencies.
  - Expected: a subset of the 162 crates reviewed, with no new licence [T].
  - It is done with the same throwaway-crate method, outside the repository, before the first build
    with eframe in the workspace [P].

### 6.2 Other dependencies (owner decisions, 2026-11-07; section 11)

| Need | Decision | Note |
|---|---|---|
| **JPEG decoding** of embedded previews (G4) | **libjpeg-turbo, through a safe Rust wrapper.** Fastest in P0, with scaled decoding [M] | The concrete wrapper crate and version, and how libjpeg-turbo is built or linked (vcpkg static, as LibRaw, or the wrapper's own build), are proposed with a licence check in step S3, before they are added (GR10, ADR 0002). libjpeg-turbo carries the IJG, BSD-3-Clause and zlib licences [D]; their notices go to THIRD-PARTY-NOTICES |
| Folder dialog | **None in milestone 1.** Command line and drag-and-drop, which eframe supports itself | No `rfd` |
| Settings file | **None.** Command-line arguments only | eframe's `persistence` feature stays off (GUI_REQUIREMENTS §6) |

No other new crates are planned. `cc` is already used by `pw-libraw`.

## 7. Coexistence with the Tauri app (GR8)

- **Nothing in `src-tauri`, `ui/web` or `pw-rawview` changes in milestone 1.**
- **`pw-libraw` is extended only additively.**
  - A new demosaic parameter (PPG for the loupe) leaves today's functions and defaults as they are, so
    the Tauri app keeps AHD as it does today.
  - Switching the Tauri app to PPG is not part of this milestone.
- **Two programs in one workspace.**
  - `cargo build -p pw-loupe` builds only the loupe; `cargo run -p photoworkflow` builds only the
    Tauri app.
  - Both link LibRaw statically, as today.
- **No shared state between the programs.** The loupe writes no sidecars, catalogs or caches in
  milestone 1, so the two cannot conflict.
- **The distribution folder (`distribution.md`) is unchanged.** The loupe is a development build until
  the owner decides how it ships.

## 8. Testing and acceptance

**What the acceptance criteria are.** They are V1–V8 and G1–G4, and GR6 and GR7 as far as the owner
has not postponed them. P7's figures are **not** acceptance criteria (TESTING.md §2). They show what
can be expected.

### 8.1 Automated (GR11; `cargo test`, `cargo clippy`)

| Level | What | Tested without |
|---|---|---|
| Unit tests, `pw-viewcore` | View geometry and orientation (flip 0/3/5/6); V6 clamping and centring; the scheduling order; dwell; admission and eviction; the current image never evicted; folder generations; display-mode rules (G4); error views (G3) | Native libraries: the decoders are simulated |
| Unit tests, `pw-render` | Texture-budget accounting, protected views, and texture coordinates per corner | A GPU: the logic is kept apart from the egui calls |
| Integration tests, `pw-libraw` | PPG regions with a margin equal the same area of a whole PPG render (the ADR 0006 identity, checked again for PPG) [P] | – (real files, LibRaw) |
| Real-file tests, `pw-viewcore` with LibRaw | A7R (flip 5), A1, RW2 and DNG: views have the expected sizes and orientation; damaged files are refused | – |
| Key handling | Input to core calls, without a window | eframe |

### 8.2 On screen and by hand

| Check | Method |
|---|---|
| V1, V3, V7: image change, Z and pan | `latency.ps1` against the window title "PhotoWorkflow Loupe", in RAW and JPEG mode, as in P7 part D |
| V2: one swap, no intermediate frame | `latency.ps1`, which counts the changes per press |
| V4: about 7 images/s at 100 % | `latency.ps1` at a 140 ms gap |
| V5: first image after opening | Measured from opening, in both modes |
| V6, G1, G3, G4 | Checked by hand against a list, with screenshots |
| V8 | Process memory against the two budgets, with the L1–L4 method of P6c/P7 |
| GR6 and GR7 (resize, minimise and restore, focus) | By hand, as in P7. DPI and a second screen stay postponed |

### 8.3 Known limits that acceptance will show [M]

These are not new findings. Milestone 1 is not expected to fix them, and the report will say so.

- **Cold RAW start and continuous fast stepping in RAW mode** (V4, V5) are limited by the CPU unpack,
  about 337 ms for A7R, and PPG. P5 and P7 measured about 10 % hits at 4–10 images/s.
- **JPEG mode on the A1:** a full decode takes about 150 ms with libjpeg-turbo [M, P0]. Scaled
  decoding for the fit view is expected to shorten this (97–120 ms at 1/2 in P0); JPEG stepping at
  10/s is measured in S6.

## 9. Work steps

Each step is a bounded work part. It ends with the review flow in `claude/working-agreements.md`:
summary, git diff and git status, and the owner's approval before commit. Nothing is pushed.

| Step | Content | Result |
|---|---|---|
| **S0** | Accept this ADR. Amend GUI_REQUIREMENTS §6 (remove `wgpu`). Decide the questions in section 11 | **Done 2026-11-07:** documentation commit |
| **S1** | `pw-libraw`: PPG as an additive option, with the region identity test for PPG | Code + tests |
| **S2** | `pw-viewcore`: view model, views, scheduling, budgets and errors, with simulated decoders; then the RAW backend on `pw-libraw` | Code + tests |
| **S3** | JPEG mode in `pw-viewcore`: libjpeg-turbo through a safe wrapper (decided in S0), behind its trait. The wrapper crate and its licence check are approved first | Code + tests |
| **S4** | `pw-render`: textures within the budget, and placement | Code + tests |
| **S5** | `pw-loupe`: window, input, status line, indicator, errors and settings. The licence check for the eframe tree without wgpu comes before the first build | Code + tests, first runnable program |
| **S6** | Acceptance measurement (section 8.2) and a results report | Results document; no code changes unless the owner asks |
| **S7** | Documentation: architecture.md, TESTING.md (the loupe's tests), docs/README.md | Documentation commit |

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| PPG regions are not pixel-identical to a whole PPG render with the current margin | Seams in the 100 % view | S1 tests it first. The margin can grow; it is the same mechanism as for AHD (ADR 0006) |
| JPEG mode changes the hit rates measured in P5/P7 (RAW only) | Fewer hits in mixed use | The P5 order is kept (the other mode after the neighbours), and S6 measures it |
| glow's driver pools (about 230 MB that stays) grow over a long session | Memory above the budgets | S6 runs a long session with the L1–L4 method |
| Two programs share `pw-libraw` | A change for the loupe affects the Tauri app | Additive changes only, and the existing `pw-libraw` tests stay green |
| Production rewrite differs from the measured prototype | Results worse than P7 | The P7 tests are carried over, and S6 repeats the P7 on-screen series |
| eframe 0.37 with glow on other GPUs | Not tested | Out of scope; only thanos (Arc B580) is the target machine for now |

## 11. Owner decisions on the open questions (2026-11-07)

1. **JPEG decoder:** **libjpeg-turbo, through a safe Rust wrapper**, for speed and scaled decoding.
   The wrapper crate is approved with a licence check in step S3.
2. **Opening a folder:** **command-line arguments and native drag-and-drop** in milestone 1. No dialog
   boxes.
3. **Budget settings (V8):** **command-line arguments only.** No settings file yet.
4. **Names:** the binary is **`pw-loupe`**. The window title is **"PhotoWorkflow Loupe"**, and
   `latency.ps1` targets it.
5. **Keys:** **the same classic keys as the current viewer** (section 3.3).
6. **`pw-rawview`:** **left untouched** in the Tauri app, as an isolated legacy component.

## 12. Consequences

- **The loupe can be built and accepted without touching the working app.**
- **The core becomes testable production code.** That code also carries the later milestones: the
  contact sheet, ratings and ingest.
- **The dependency tree shrinks:** eframe without wgpu.
- **ADR 0008 §2.1 is changed on one point:** `pw-viewcore` is built on the P7 logic, not on
  `pw-rawview`. The rest of the common core (view model, swap rule, budget, decoders) applies as
  written.
- **GUI_REQUIREMENTS §5:** the order of migration is settled for milestone 1, with the loupe first. The
  later milestones are planned in their own ADRs or amendments.
