# RawStride

A fast and lightweight photo culling and ingest application for Windows.

The project was called PhotoWorkflow until 2026-11-09 (ADR 0012). Folder, branch and crate names
(`pw-*`) keep the old name for now; the existing Tauri app is still titled PhotoWorkflow.
It covers card ingest, culling, rating and metadata.

**Camera support:** the goal is to support most cameras, through LibRaw.
During development we start with Sony A1 and A7R IVA (ARW), Panasonic (RW2) and DNG.

> Status: the new eframe + egui program (binary `pw-loupe`, window title "RawStride") has the loupe,
> the contact sheet, folder tabs, the full import dialog, ratings and labels, filter and
> multi-selection, and TIFF viewing. The existing Tauri app remains in place until the switch
> (GUI_REQUIREMENTS GR8).

## Stack

| Area | Choice |
|---|---|
| App | RawStride: eframe + egui (`pw-loupe`); the existing Tauri 2 app until the switch |
| Core | Rust (workspace in `crates/`) |
| RAW decoding | LibRaw (vcpkg, static), plus our own embedded-preview fast path |
| Metadata | ExifTool (one persistent `-stay_open` process) |
| Catalog | SQLite (planned; `pw-catalog` is not implemented yet) |
| Rendering | eframe renderer (glow) for the new GUI |

## Layout

```
crates/        Rust libraries and the RawStride egui program (pw-loupe)
src-tauri/     Existing Tauri app shell
ui/            Existing Tauri frontend
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
