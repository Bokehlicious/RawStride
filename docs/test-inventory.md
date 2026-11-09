# Test inventory

This inventory describes Rust tests included by `cargo test --workspace`. It was checked against
a Windows run on 2026-11-09 (after ADR 0013 B2): **330 passed, 27 ignored by default, 0 failed**.
Test counts include
ignored tests. Unit-test rows aggregate the inline `#[cfg(test)]` modules in each crate; for
`pw-loupe`, the shared metadata module is included in both its library and binary test targets.
Crates and targets with no tests are omitted. This inventory does not claim that a product
requirement is fully verified.

| Crate | File(s) | What it verifies | Category | Prerequisites | Tests | Ignored |
|---|---|---|---|---|---:|---:|
| `photoworkflow` | `src-tauri/src/imaging.rs`, `src-tauri/src/ingest.rs` | Image-buffer conversion and fit sizing; ingest action labels and conversion of dialog text to IPTC. | Unit | vcpkg libraries | 5 | 0 |
| `preview-bench` | `tools/preview-bench/src/main.rs` | CSV parsing, timing statistics, and reading byte ranges including an unaligned end-of-file range. | Unit | None | 4 | 0 |
| `pw-core` | `crates/pw-core/src/lib.rs` | Human-readable byte formatting and unit-boundary rounding. | Unit | None | 14 | 0 |
| `pw-ingest` | `crates/pw-ingest/src/eject.rs`, `paths.rs`, `request.rs`, `summary.rs`, `template.rs`, `volumes.rs` | Eject polling/scripts, path and import-request validation (with its exact messages), card detection over candidate roots, folder counts, template rendering, capture dates, and safe path segments. | Unit | None | 30 | 0 |
| `pw-libjpeg` | `crates/pw-libjpeg/src/lib.rs` | Scaled JPEG output dimensions. | Unit | vcpkg libraries | 1 | 0 |
| `pw-libraw` | `crates/pw-libraw/src/lib.rs` | Demosaic quality mapping and RGB crop behavior. | Unit | vcpkg libraries | 2 | 0 |
| `pw-loupe` | `crates/pw-loupe/src/lib.rs`, `src/grid.rs`, `src/tabs.rs`, `src/thumbs.rs`, `src/import.rs`, `src/import_worker.rs`, `src/import_dialog.rs`, `src/import_text.rs`, `src/ratings.rs`, `src/main.rs` (shared `src/metadata.rs` module) | Viewer options, file listing, keyboard actions for loupe, grid, import dialog and ratings; rating key mapping, optimistic rating updates, ordered writes, rollback, stale-read protection and the Swedish rating texts; folder-tab state, titles, view mode and clearing tabs on a card; grid columns, visible range, navigation and thumbnail size steps; thumbnail bookkeeping, request cell sizes and image conversion; import form validation by field (backup, job, number, IPTC template), preview summary, the import worker (cached and fresh scans, copy, progress, cancel, IPTC step, eject rules, card detection) on temporary folders, the dialog state machine, and the Swedish dialog texts; metadata-cache loading, preserved ratings, and error states. | Unit / integration | vcpkg libraries | 124 | 0 |
| `pw-metadata` | `crates/pw-metadata/src/exiftool.rs`, `lib.rs`, `test_support.rs`, `worker.rs` | ExifTool protocol handling, localized IPTC values, worker failures/restarts, and its ignored fake-process entry point. | Unit | None | 14 | 1 |
| `pw-raw` | `crates/pw-raw/src/lib.rs` | Synthetic JPEG/TIFF parsing, preview extraction, EXIF orientation, and exposure formatting. | Unit | None | 6 | 0 |
| `pw-thumbcache` | `crates/pw-thumbcache/src/extract.rs`, `worker.rs` | Embedded-preview and decode-scale choice, all eight orientations, thumbnails from a generated JPEG and from invalid files; the worker's priority, generations, cancel, pause, duplicate requests, LRU cache and error results with a simulated source; thumbnails from camera RAWs. | Unit / real-file | vcpkg libraries; `PW_TESTDATA` (4 ignored) | 29 | 4 |
| `pw-render` | `crates/pw-render/src/lib.rs` | View sizing, mesh placement/UVs, texture filtering, and texture memory accounting. | Unit | None | 3 | 0 |
| `pw-viewcore` | `crates/pw-viewcore/src/geometry.rs`, `native.rs`, `scale.rs`, `state.rs` | Geometry/orientation, image scaling and cropping, native orientation mapping, scheduling, zoom, and memory-budget state. | Unit | vcpkg libraries | 44 | 0 |
| `pw-ingest` | `crates/pw-ingest/tests/ingest.rs` | Scanning synthetic card contents, planning and verifying copies, conflict naming, backups, cancellation, and a real-RAW EXIF-time case. | Integration / real-file | `PW_TESTDATA` (1 ignored) | 8 | 1 |
| `pw-libjpeg` | `crates/pw-libjpeg/tests/decode.rs` | Decoding a generated JPEG at multiple scales, invalid input handling and thread transfer; embedded previews from camera RAWs. | Integration / real-file | vcpkg libraries; `PW_TESTDATA` (1 ignored) | 4 | 1 |
| `pw-libraw` | `crates/pw-libraw/tests/damaged.rs` | Distinguishing unreadable files from damaged RAW data, and detecting truncation in camera files. | Integration / real-file | vcpkg libraries; `PW_TESTDATA` (2 ignored) | 3 | 2 |
| `pw-libraw` | `crates/pw-libraw/tests/regions.rs` | Comparing rendered RAW regions with full renders, including PPG/AHD selection and session reuse; generated invalid-file cases. | Real-file | vcpkg libraries; `PW_TESTDATA` (8 ignored) | 10 | 8 |
| `pw-metadata` | `crates/pw-metadata/tests/sidecar.rs` | Rating and IPTC writes/reads, sidecar preservation, Unicode paths, embedded JPEG/TIFF updates, and a real DNG copy. | External tool / real-file | ExifTool; `PW_TESTDATA` (1 ignored) | 7 | 1 |
| `pw-raw` | `crates/pw-raw/tests/real_files.rs` | Camera RAW metadata, embedded-preview sizes/offsets, and orientation handling. | Real-file | `PW_TESTDATA` (4 ignored) | 4 | 4 |
| `pw-rawview` | `crates/pw-rawview/tests/engine.rs` | Simulated engine queue priority, cancellation, concurrency, session reuse, tile caching, and memory limits. | Integration | vcpkg libraries in the workspace feature-unified build | 24 | 0 |
| `pw-rawview` | `crates/pw-rawview/tests/libraw_engine.rs` | Windows LibRaw first-tile timing and process-memory behavior on camera RAWs. | Real-file | Windows; vcpkg libraries; `PW_TESTDATA` (3 ignored) | 3 | 3 |
| `pw-render` | `crates/pw-render/tests/sync.rs` | Synchronizing simulated view output with headless egui textures, including texture limits and folder changes. | Integration | None | 3 | 0 |
| `pw-viewcore` | `crates/pw-viewcore/tests/engine.rs` | Simulated view scheduling, zoom/rotation, error presentation, texture residency, worker responsiveness, and JPEG mode. | Integration | vcpkg libraries in the workspace feature-unified build | 12 | 0 |
| `pw-viewcore` | `crates/pw-viewcore/tests/native_files.rs` | Native decoder behavior on camera RAWs and a generated non-RAW file. | Integration / real-file | vcpkg libraries; `PW_TESTDATA` (2 ignored) | 3 | 2 |
