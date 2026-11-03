# PhotoWorkflow

A fast and lightweight photo culling and ingest application for Windows.
It covers card ingest, culling, rating and metadata.

**Camera support:** the goal is to support most cameras, through LibRaw.
During development we start with Sony A1 and A7R IV (ARW), Panasonic (RW2) and DNG.

> Status: **project skeleton**. There is no application code yet.

## Stack

| Area | Choice |
|---|---|
| App shell | Tauri 2 |
| Core | Rust (workspace in `crates/`) |
| RAW decoding | LibRaw (vcpkg, static), plus our own embedded-preview fast path |
| Metadata | ExifTool (persistent `-stay_open` process pool) |
| Catalog | SQLite |
| Rendering | GPU via wgpu (DX12 / Vulkan) |

## Layout

```
crates/        Rust libraries (core, raw, metadata, catalog, ingest, thumbcache, render)
src-tauri/     Tauri app shell (generated in the implementation phase)
ui/            Frontend (contact sheet, loupe, panels)
docs/          Architecture, ADRs, clean-room policy, developer setup
third_party/   Notes, pins and licences for native dependencies
tools/         Developer scripts and benchmarks
tests/fixtures Small, licence-clean test files only
```

Large RAW test sets, caches and catalogs live outside the repo, in `D:\PhotoWorkflowData`.

## Getting started

See [docs/dev-setup.md](docs/dev-setup.md).

## Important

This project is developed under a **clean-room policy** (see [docs/clean-room.md](docs/clean-room.md)).
No code, assets or decompiled material from other products may enter this repository.
