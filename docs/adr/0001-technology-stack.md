# ADR 0001: Technology stack

- Status: accepted
- Date: 2026-11-03

## Context

We need a fast, lightweight Windows culling and ingest tool for Sony A1 and A7R IV
ARW files, developed by a small team.

## Decision

- **Tauri 2** for the app shell. It uses the system WebView2, so the app stays small.
- **Rust** for all core logic, organised as a Cargo workspace of small crates.
- **LibRaw** (via vcpkg, statically linked) for full RAW decoding. Our own code
  extracts embedded ARW previews for the culling fast path.
- **ExifTool** as an external process in `-stay_open` mode for metadata read and write.
- **SQLite** for the catalog.
- **wgpu** for GPU rendering (DX12 and Vulkan on Intel Arc B580).

## Consequences

- We need a C/C++ toolchain (MSVC, CMake, Clang for bindgen) as well as Rust.
- ExifTool must be shipped alongside the app or located at runtime.
- How the GPU renderer integrates with the webview needs its own ADR.
