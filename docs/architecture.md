# Architecture (draft)

> **2026-11-09 (owner):** the product is called **RawStride** (ADR 0012). Crate names keep the `pw-`
> prefix.

> **2026-11-07 (owner):** the GUI is built in pure Rust with eframe + egui
> ([GUI_REQUIREMENTS.md](GUI_REQUIREMENTS.md)). The app crate below replaces `src-tauri` when the egui GUI
> takes over. Until then, the current Tauri app keeps working.

## Goals

1. **Culling speed above everything.** Moving to the next image must feel instant.
   To get there, we show the embedded JPEG preview first and only decode the RAW when the user asks for it.
2. **Safe ingest.** Every copy is verified, and the source card is never modified.
3. **Portable metadata.** User-facing metadata is written to XMP, so other tools can read it.
   The SQLite catalog is an index and cache, not the only copy.
4. **Lightweight.** The app starts fast, uses little memory when idle, and needs no background services.

## Camera support

- **Long term:** most cameras that LibRaw supports.
- **Development phase:** Sony A1 (ILCE-1) and A7R IVA (ILCE-7RM4A) (`.ARW`), Panasonic (`.RW2`) and DNG (`.dng`).
  These formats define the test set in `D:\PhotoWorkflowData\RAW`.
- Sony files are tested as **Compressed RAW** only, since that is the format in use. LibRaw still
  decodes Lossless Compressed and Uncompressed, but we neither test nor optimise for them in this phase.
- The fast path, which uses the embedded previews, is designed per format with a generic fallback.
  This means a camera without a usable preview still works, just more slowly.

## Crates

```
                 ┌──────────────┐
                 │  app (egui)  │  eframe window, egui UI, view model
                 └──────┬───────┘
     ┌────────┬─────────┼──────────┬────────────┐
     ▼        ▼         ▼          ▼            ▼
 pw-ingest pw-catalog pw-metadata pw-thumbcache pw-render
     │        │         │          │            │
     └────────┴────┬────┴──────────┴─────┬──────┘
                   ▼                     ▼
                pw-core               pw-raw (LibRaw FFI)
```

| Crate | Responsibility |
|---|---|
| `pw-core` | IDs, errors, config, paths. No I/O. |
| `pw-raw` | ARW preview extraction (fast path) and the LibRaw full decode (slow path) |
| `pw-metadata` | ExifTool process pool, EXIF/IPTC/XMP read and write |
| `pw-catalog` | SQLite schema, migrations and queries (WAL, single writer) |
| `pw-ingest` | Card detection, request validation, scan, plan, verified copy, renaming, backup, eject; shared by the Tauri app and the egui program |
| `pw-thumbcache` | Thumbnails from the embedded previews (never a RAW rendering), a small pool of background workers with visible-first priority, and an in-memory cache with a byte limit (ADR 0011). A disk cache is not built: the first measurement showed it is not needed for the test set |
| `pw-loupe` | The egui program (until it is renamed): loupe, contact sheet, folder tabs and the import dialog with its worker (ADR 0009, 0010, 0011). UI logic is plain Rust in its library and tested; `main.rs` only draws |
| `pw-render` | Image presentation in egui: texture upload and residency within the budget, through eframe's renderer only (no separate wgpu pipelines) |

## Storage on the dev machine

| Drive | Use |
|---|---|
| C: (NVMe, Kingston NV3) | Repo, build output, thumbnail cache, catalog |
| D: (SATA SSD) | `D:\PhotoWorkflowData` – test RAWs, cache experiments |
| E: (SATA SSD), F/G/H (HDD) | Ingest destinations and backup targets for testing |

Note: the NV3 is DRAM-less, and its sustained writes drop once the SLC cache is full.
Benchmark large ingests before choosing default destinations.

## Open questions

- ~~How GPU rendering integrates with the Tauri webview.~~ Settled 2026-11-07: there is no webview; the
  GUI and image presentation are eframe + egui (ADR 0008 §16).
- ~~eframe rendering backend (OpenGL through glow, or wgpu).~~ Settled 2026-11-07: **glow**, after the
  P7 measurements (ADR 0008 §18, GUI_REQUIREMENTS.md §7).
- ~~Thumbnail cache format and sizes.~~ Settled 2026-11-09 for now: thumbnails are made from the embedded
  previews on demand and kept in memory only; sizes 96–480 pt, default 200 (ADR 0011 §11, K5). A disk
  cache is reconsidered only if larger folders need it.
- Catalog per volume or global.
