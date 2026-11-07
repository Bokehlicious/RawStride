# ADR 0001: Technology stack

- Status: accepted; **amended 2026-11-07** (GUI: pure Rust with eframe + egui, see
  [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md))
- Date: 2026-11-03

## Context

We need a fast, lightweight Windows culling and ingest tool for Sony A1 and A7R IVA
ARW files, developed by a small team.

## Decision

- ~~**Tauri 2** for the app shell. It uses the system WebView2, so the app stays small.~~
  **Superseded 2026-11-07 (owner):** the GUI is built in **Rust with eframe + egui**, in the same
  process as the core. No Tauri, WebView2, HTML or JavaScript in the new GUI. The current Tauri app
  keeps working until the egui GUI replaces it (GUI_REQUIREMENTS.md, GR8).
- **Rust** for all core logic, organised as a Cargo workspace of small crates.
- **LibRaw** (via vcpkg, statically linked) for full RAW decoding. Our own code
  extracts embedded ARW previews for the culling fast path.
- **ExifTool** as an external process in `-stay_open` mode for metadata read and write.
- **SQLite** for the catalog.
- ~~**wgpu** for GPU rendering (DX12 and Vulkan on Intel Arc B580).~~
  **Superseded 2026-11-07:** GPU drawing goes only through eframe's own rendering backend. No
  separate wgpu pipelines (render or compute) are built for the UI. The backend choice is open
  (GUI_REQUIREMENTS.md §4).

## Consequences

- We need a C/C++ toolchain (MSVC, CMake, Clang for bindgen) as well as Rust.
- ExifTool must be shipped alongside the app or located at runtime.
- ~~How the GPU renderer integrates with the webview needs its own ADR.~~ No longer applies: there is
  no webview in the new GUI (2026-11-07).
- New crates for the GUI (eframe, egui) need the owner's approval of the concrete versions before they
  are added (GUI_REQUIREMENTS.md, GR10).
