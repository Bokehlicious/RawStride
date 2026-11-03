# Architecture (draft)

## Goals

1. **Culling speed above everything.** Moving to the next image must feel instant.
   To get there, we show the embedded JPEG preview first and only decode the RAW when the user asks for it.
2. **Safe ingest.** Every copy is verified, and the source card is never modified.
3. **Portable metadata.** User-facing metadata is written to XMP, so other tools can read it.
   The SQLite catalog is an index and cache, not the only copy.
4. **Lightweight.** The app starts fast, uses little memory when idle, and needs no background services.

## Crates

```
                 ┌──────────────┐
                 │  src-tauri   │  commands, windows, events
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
| `pw-ingest` | Volume detection, copy and verify, renaming, backup |
| `pw-thumbcache` | Content-addressed thumbnail and preview cache |
| `pw-render` | GPU texture upload and presentation |

## Storage on the dev machine

| Drive | Use |
|---|---|
| C: (NVMe, Kingston NV3) | Repo, build output, thumbnail cache, catalog |
| D: (SATA SSD) | `D:\PhotoWorkflowData` – test RAWs, cache experiments |
| E: (SATA SSD), F/G/H (HDD) | Ingest destinations and backup targets for testing |

Note: the NV3 is DRAM-less, and its sustained writes drop once the SLC cache is full.
Benchmark large ingests before choosing default destinations.

## Open questions

- How GPU rendering integrates with the Tauri webview: native child window,
  shared texture, or webview-only rendering. This gets an ADR before any code is written.
- Thumbnail cache format and sizes.
- Catalog per volume or global.
