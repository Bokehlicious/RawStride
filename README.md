# PhotoWorkflow

A fast and lightweight photo culling and ingest application for Windows.
It covers card ingest, culling, rating and metadata.

**Camera support:** the goal is to support most cameras, through LibRaw.
During development we start with Sony A1 and A7R IVA (ARW), Panasonic (RW2) and DNG.

> Status: The Rust workspace includes the standalone `pw-loupe` viewer, the first milestone of the
> eframe + egui GUI migration. The existing Tauri app remains in place while the rest of its features
> are migrated.

## Stack

| Area | Choice |
|---|---|
| App | Existing Tauri 2 app; standalone eframe + egui loupe |
| Core | Rust (workspace in `crates/`) |
| RAW decoding | LibRaw (vcpkg, static), plus our own embedded-preview fast path |
| Metadata | ExifTool (one persistent `-stay_open` process) |
| Catalog | SQLite (planned; `pw-catalog` is not implemented yet) |
| Rendering | eframe renderer (glow) for the new GUI |

## Layout

```
crates/        Rust libraries and the standalone pw-loupe app
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
