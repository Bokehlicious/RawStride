# GUI requirements: pure Rust with eframe + egui

- Status: **decided by the owner (2026-11-07).** Versions and features approved the same day (§6).
  The renderer and the memory budget were decided after P7, also the same day (§7). `wgpu` was
  removed from the feature list on 2026-11-07 (ADR 0009, step S0).
- Scope: the whole graphical user interface of RawStride. That covers the window, the loupe and
  stepping, the contact sheet, ratings and labels, the ingest dialog, the metadata views and settings.
- Related:
  - ADR 0001 (technology stack, amended);
  - ADR 0003 (M1 viewer, amended);
  - ADR 0008 (image presentation, amendment 6, §16, and amendment 8, §18);
  - ADR 0010 (milestone 2: import and folder tabs; GR12);
  - ADR 0011 (milestone 3: the contact sheet; GR13);
  - `requirements-image-viewer.md` (V1–V8, G1–G4), which is unchanged.

## 1. Decision

**The GUI is built in Rust with eframe and egui.**

- No Tauri, no WebView2, no HTML, CSS or JavaScript, and no Node.js tooling in the new GUI.
- The GUI runs in the same process as the Rust core. There is no IPC bridge, no `pw://` scheme and no
  shared buffers between processes for the GUI.
- GPU access for the GUI goes only through eframe's own rendering backend.
- **No separate or complex wgpu compute pipelines are built for the UI.**

This replaces earlier statements that the app shell is Tauri 2 with a WebView2 frontend (ADR 0001,
ADR 0003) and the presenter decision B2-res (WebView2 shared buffer into WebGL2, ADR 0008 §13).

## 2. Requirements

| # | Requirement |
|---|---|
| GR1 | **Pure Rust UI.** All windows, panels, dialogs and overlays are written in Rust with egui, hosted by eframe. No embedded web view. |
| GR2 | **One process.** The GUI calls the core crates directly. Long work never runs on the UI thread (G2). |
| GR3 | **Image presentation inside egui.** Images are shown as egui textures, or, if measurement shows it is needed, drawn through eframe's renderer as a custom paint step. Either way, the view model, cache and swap rule of the core (ADR 0008 §2) are used unchanged. |
| GR4 | **GPU use is limited to eframe's renderer.** No separate wgpu device, surface or compute pipeline for the UI. A shader for image display may be considered only as part of eframe's rendering, and only after an owner decision (§4). |
| GR5 | **The performance and behaviour requirements apply unchanged.** V1–V8 and G1–G4 (`requirements-image-viewer.md`) are the acceptance criteria for the new GUI, measured on screen with `latency.ps1` as before. |
| GR6 | **Keyboard first.** Every culling action (next/previous, Z, ratings, labels, RAW/JPEG mode) works from the keyboard. Focus must not be lost after minimise, restore or a dialog. |
| GR7 | **Windows 11 is the target.** The GUI must handle resize, DPI changes and a move to another screen without wrong scaling. The owner postponed these tests for the prototypes; they are part of acceptance for the GUI. |
| GR8 | **Feature parity before the switch.** The current Tauri app keeps working until the egui GUI covers its features and passes the acceptance tests. Features: the loupe, contact sheet, ratings and labels, filter and multi-selection, the ingest dialog, IPTC metadata, and TIFF viewing. Status 2026-11-11: ratings and labels (ADR 0013), filter and multi-selection (ADR 0014) and TIFF viewing (ADR 0015) are done in the egui program. IPTC metadata: the Tauri app's only IPTC feature, the import template, is in the egui program (ADR 0010, T39); full IPTC handling (ADR 0007) is done in the egui program as milestone 8 (2026-11-14) and was not a parity item. No feature of the Tauri app is left; the switch waits for the acceptance tests. S6 (ADR 0009) partly measured on 2026-11-11 (runs 11–13 open): V3, V4 (RAW mode) and V7 not met; V5, V6, V8, G1–G4, GR6 and GR7 not measured yet. The open owner question about colour management (ADR 0015 §9.1) is answered: done in ADR 0016 (2026-11-11). |
| GR9 | **Honest source (G1) stays visible.** The UI says whether an image is the embedded JPEG or a RAW rendering, as today. |
| GR10 | **Dependencies need approval.** eframe, egui and any helper crates are added only after the owner approves the concrete crates and versions, with a licence check (ADR 0002). Nothing is installed by this document. |
| GR11 | **Testable.** UI logic (view model, key handling, dialog state) is kept in plain Rust that can be tested with `cargo test`, apart from drawing. |
| GR12 | **Imported folders open in a new tab.** When an import finishes, the imported folder opens automatically in a new tab in the viewer (owner requirement, 2026-11-08; ADR 0010). If that folder is already open in a tab, that tab is activated instead. |
| GR13 | **Opened folders start as a contact sheet.** A folder dropped on the window or opened by an import is shown as a grid of thumbnails first, not in the loupe (owner requirement, 2026-11-08; ADR 0011). |
| GR14 | **English is the interface's base language** (owner decision, 2026-11-13). Every text the user sees is English, with the terms of `glossary.md` (approved the same day, G1–G8). Texts are made by text functions (`docs/tasks/README.md`, rule 13), so another language can be added later; there is no language choice for now. Today's Swedish texts are moved and translated by T116–T118. |

## 3. What stays

- **The core crates:** `pw-raw`, `pw-libraw`, `pw-rawview`, `pw-metadata`, `pw-ingest`, `pw-catalog`
  and the planned `pw-viewcore`.
- **ADR 0008 decisions about the core:**
  - the view model, caches and swap rule (§2);
  - PPG as the default demosaic method, with AHD as the quality reference (§11);
  - the scheduling strategy p5 and the 4 GB budget (§12).
- **All measurements** in ADR 0008 and the analyses. They remain a record of what was measured, in the
  setup they were measured in.

## 4. What changes, and what is open

| Topic | Status |
|---|---|
| Presenter B2-res (WebView2 + WebGL2) | **Superseded.** The image is drawn by egui in our own process. |
| GPU PPG (P6b/P6c, a WebGL2 shader) | **Not carried over as is.** CPU PPG stays the decided path. Whether a GPU demosaic step is ever moved into eframe's renderer is an open question for the owner. It would be a display shader, not a compute pipeline (GR4). |
| P6c memory figures (WebView2 GPU process, ANGLE) | **Specific to WebView2.** They do not describe an egui GUI. **Measured again in P7:** under eframe a texture costs about 1.2× its size in RAM (§7). |
| The 4 GB budget and texture memory | **Decided for now (§7):** the core's 4 GB and the texture budget are counted separately. |
| eframe rendering backend (OpenGL through glow, or wgpu) | **Decided: glow (§7)**, after the measured comparison in P7. |
| Today's UI tests (Playwright against a mocked Tauri backend) | Remain valid for the current app until it is replaced. New tests follow GR11. `docs/TESTING.md` is updated when the GUI work starts. |
| Distribution (`distribution.md`) | The WebView2 runtime and profile folder are no longer needed once the egui GUI ships. To be updated then. |

## 5. Not decided here

- Any helper crates (GR10). The eframe and egui versions are decided in §6, and the rendering backend
  in §7.
- The order of migration, and whether the first egui milestone is the loupe alone.
- Whether any prototype is built before the app is changed. Rule: the working app is not broken.

## 6. Approved versions and features (owner decision, 2026-11-07)

**Versions:**

- **eframe 0.37.0** and **egui 0.37.0**, released 2026-10-15.
- Licence MIT OR Apache-2.0.
- They need Rust 1.95; the project toolchain is 1.99.1.
- The version is pinned exactly, as `=0.37.0`.

**Features:** `default-features = false`, plus only these:

| Feature | Why |
|---|---|
| `glow` | **The chosen renderer** (OpenGL, §7) |
| `accesskit` | Windows accessibility |
| `default_fonts` | Bundled fonts. Kept by the owner's decision. The fonts carry OFL-1.1 and the Ubuntu Font Licence, so their notices ship later with THIRD-PARTY-NOTICES (ADR 0002) |
| `clipboard` | Copy and paste |

Left out:

- **`wgpu`** (owner decision, 2026-11-07, ADR 0009): glow is the only renderer (§7). The P7 prototype
  built both renderers for its comparison; the app uses glow only.
- `wayland`, `x11`, `web_screen_reader` and `links` (Linux and web only).
- `persistence` (until there is a real settings format).

### 6.1 Folder dialog (owner decision, 2026-11-09, ADR 0010 M3)

- **rfd 0.17.2** (Rust File Dialog), for the Windows folder dialog of the import, in `pw-loupe` only.
- Licence MIT. Pinned exactly: `rfd = { version = "=0.17.2", default-features = false }`. The default
  features (`xdg-portal`, `wayland`) are for Linux only; no feature is enabled.
- On Windows it needs `windows-sys` 0.61, `raw-window-handle` 0.6 and `log` 0.4, which are already in
  the workspace through eframe (0.61.2, 0.6.2 and 0.4.35; all MIT OR Apache-2.0, `raw-window-handle`
  also Zlib). `rfd` 0.17.2 itself is also already in `Cargo.lock`, through the Tauri app's `tauri-plugin-dialog` (noted
  2026-11-09), so adding it to `pw-loupe` adds no new crate; `Cargo.lock` must show no new entry.
- 0.18.0 was released a day before the decision and was not chosen; an update is a separate decision.
- `rfd` is listed in THIRD-PARTY-NOTICES before the first release (ADR 0002).

**Licence review (accepted by the owner, 2026-11-07):**

- **Scope:** all 162 sub-dependencies for Windows x64, with `wgpu` still included, read from
  `cargo metadata` in a throwaway crate outside the repository. The list is in `D:\PhotoWorkflowData\analysis\licprobe\eframe-0.37-licences.md`.
- **Result:** all are permissive. Most are MIT OR Apache-2.0; others are Zlib, BSD, ISC, BSL-1.0 or
  Unlicense, or offer one of these.
- **`self_cell`** is offered as "Apache-2.0 OR GPL-2.0-only" and is used under **Apache-2.0**.
- **Notices required in the distribution:**
  - the Apache-2.0 and MIT licence texts;
  - the OFL-1.1 and Ubuntu Font Licence, for the fonts;
  - Unicode-3.0, for `unicode-ident` and `unicode-properties`.

  These are collected in THIRD-PARTY-NOTICES before a first release (ADR 0002).

**Still to do:**

- **Adding eframe to the code** is a separate step, with its own approval. This decision approves the
  versions and features, not a code change.
- **A newer version** needs a new decision and a new licence review.
- **The tree without `wgpu`** is checked again before eframe is first built in the workspace (ADR 0009
  §6.1). It is expected to be a subset of the 162 crates reviewed, with no new licence.

## 7. Renderer and memory budget (owner decision, 2026-11-07)

**Decisions:**

- **The renderer is glow** (OpenGL), set with `NativeOptions::renderer = Renderer::Glow`. wgpu is
  not chosen.
- **The core budget and the texture budget are counted separately for now.**
  - The core keeps its 4 GB (ADR 0008 §12).
  - The egui textures have their own budget, about 1 GB.
  - The whole process can therefore use more than 4 GB. A shared budget may be decided later.

**Basis: P7** (ADR 0008 §18; `prototypes/gui/p7-egui-presenter`).

- **On screen, glow:** Z 11.6 ms, prefetched image change 12.8 ms and pan 17.3 ms (medians). 63 of 64
  presses were within 30 ms; the other was 30.5 ms. wgpu was equally fast on screen.
- **Hit rates match P5** with a 1–2 GB texture cache.
- **Why glow and not wgpu:**
  - Under load, glow drew resident views within 75 ms at most. wgpu had unexplained stalls of up to
    285 ms.
  - glow used less GPU memory: a baseline of 47 MB against 403 MB.
  - glow exited cleanly; the wgpu process hung on exit.
- **A texture costs about 1.2× its size in RAM** under eframe, against 2.33× under WebView2.
- **Checked by hand:** resize, minimise and restore, and keyboard focus passed (GR6, GR7).
  - DPI change and a move to another screen are still postponed.
  - GPU device loss is not tested.
