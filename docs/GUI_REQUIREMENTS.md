# GUI requirements: pure Rust with eframe + egui

- Status: **decided by the owner (2026-11-07).** Versions and features approved the same day (§6).
- Scope: the whole graphical user interface of PhotoWorkflow. That covers the window, the loupe and
  stepping, the contact sheet, ratings and labels, the ingest dialog, the metadata views and settings.
- Related:
  - ADR 0001 (technology stack, amended);
  - ADR 0003 (M1 viewer, amended);
  - ADR 0008 (image presentation, amendment 6, §16);
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
| GR8 | **Feature parity before the switch.** The current Tauri app keeps working until the egui GUI covers its features and passes the acceptance tests. Features: the loupe, contact sheet, ratings and labels, filter and multi-selection, the ingest dialog, IPTC metadata, and TIFF viewing. |
| GR9 | **Honest source (G1) stays visible.** The UI says whether an image is the embedded JPEG or a RAW rendering, as today. |
| GR10 | **Dependencies need approval.** eframe, egui and any helper crates are added only after the owner approves the concrete crates and versions, with a licence check (ADR 0002). Nothing is installed by this document. |
| GR11 | **Testable.** UI logic (view model, key handling, dialog state) is kept in plain Rust that can be tested with `cargo test`, apart from drawing. |

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
| P6c memory figures (WebView2 GPU process, ANGLE) | **Specific to WebView2.** They do not describe an egui GUI. Texture memory under eframe must be measured again. |
| The 4 GB budget and texture memory | **Open.** Under eframe the textures belong to our own process, so they can be counted in one budget. The split must be decided after measurement. |
| eframe rendering backend (OpenGL through glow, or wgpu) | **Open [T].** To be chosen after a measured comparison on the Arc B580 against V1–V7. |
| Today's UI tests (Playwright against a mocked Tauri backend) | Remain valid for the current app until it is replaced. New tests follow GR11. `docs/TESTING.md` is updated when the GUI work starts. |
| Distribution (`distribution.md`) | The WebView2 runtime and profile folder are no longer needed once the egui GUI ships. To be updated then. |

## 5. Not decided here

- The rendering backend (§4) and any helper crates (GR10). The eframe and egui versions are decided
  in §6.
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
| `wgpu` | Renderer candidate, and eframe's default renderer since 0.34 |
| `glow` | Renderer candidate (OpenGL). Kept until the measured comparison picks one (§4) |
| `accesskit` | Windows accessibility |
| `default_fonts` | Bundled fonts. Kept by the owner's decision. The fonts carry OFL-1.1 and the Ubuntu Font Licence, so their notices ship later with THIRD-PARTY-NOTICES (ADR 0002) |
| `clipboard` | Copy and paste |

Left out: `wayland`, `x11`, `web_screen_reader` and `links` (Linux and web only), and `persistence`
(until there is a real settings format).

**Licence review (accepted by the owner, 2026-11-07):**

- **Scope:** all 162 sub-dependencies for Windows x64, read from `cargo metadata` in a throwaway crate
  outside the repository. The list is in `D:\PhotoWorkflowData\analysis\licprobe\eframe-0.37-licences.md`.
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
