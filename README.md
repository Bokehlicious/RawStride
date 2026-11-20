# RawStride

A fast and lightweight photo culling and ingest program for Windows: copy the photos off the card,
step through them quickly, rate and label them, and add IPTC metadata.

> **Status: in development, not released.** The new program (`rawstride.exe`, built from the crate
> `pw-loupe`; window title "RawStride", eframe + egui) has the loupe and preview, the contact sheet,
> folder tabs, the import dialog, ratings and labels, filter and multi-selection, TIFF viewing, colour
> management and the metadata window. There are no installers yet.

The project was called PhotoWorkflow until 2026-11-09 ([ADR 0012](docs/adr/0012-product-name-rawstride.md)).
Folder and crate names (`pw-*`) keep the old name for now.

## What it does

- **Import:** copies from a memory card to a destination and an optional backup, with folder and
  file name templates, and can write an IPTC template into the copies and eject the card.
- **Culling:** a contact sheet and a loupe built for speed: RAW files are shown from their embedded
  preview or rendered with LibRaw, and the next image is prepared ahead.
- **Ratings and labels:** stars, reject and colour labels in XMP (`xmp:Rating`, `xmp:Label`), read
  and written the way other photo programs do.
- **Metadata:** IPTC Core and Extension in XMP, and the older IIM, following the IPTC standards. A
  proprietary RAW file is never modified: its metadata goes into an XMP sidecar. DNG, JPEG and TIFF
  get the metadata inside the file, written safely (a backup and a check after every write).

**Cameras:** the goal is most cameras, through LibRaw. Development and testing use Sony (ARW),
Panasonic (RW2), Fujifilm (RAF) and DNG files.

## Building

Windows 10 or 11, x64. RawStride runs only on Windows today; if you would like it on Linux or
macOS, open an issue to say so. See [docs/building.md](docs/building.md): Visual Studio Build Tools, Rust,
vcpkg and ExifTool, then

```powershell
vcpkg install --triplet x64-windows-static-md
cargo run --release -p pw-loupe
```

The tests that read real camera files need the maintainer's test files and are skipped without
them, also in CI: a green CI run does not mean a RAW file was decoded or written
([docs/TESTING.md](docs/TESTING.md) §11).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Open an issue before a larger change.

## How RawStride is developed

RawStride is written with AI coding agents, directed and reviewed by its maintainer.

- **The maintainer decides.** Requirements, architecture and every design decision are the
  maintainer's, recorded with their reasons in the [ADRs](docs/adr/) and
  [requirements](docs/GUI_REQUIREMENTS.md).
- **Agents do small, written tasks.** Each change starts as a task file under
  [`docs/tasks/`](docs/tasks/README.md) that states what to change, which files, and which tests
  to add. An agent carries out one task and leaves its change uncommitted.
- **Nothing goes in unreviewed.** Every change is reviewed against its task and must pass
  `tools/check.ps1` (formatting, personal data, `cargo audit`, all tests, clippy without warnings) before the
  maintainer approves it. Changes that affect what the user sees or what is written to files are
  also checked by hand in the running program.
- **The same rules bind people and agents.** [`AGENTS.md`](AGENTS.md) holds the rules every agent
  follows: no panics in non-test code, no new dependencies without approval, no weakened tests,
  and the [clean-room policy](docs/clean-room.md).

The task files are kept as part of the project's history: [`docs/tasks/done/`](docs/tasks/done/)
shows how each feature was specified and built. You do not need to read them to contribute; the
ADRs and [CONTRIBUTING.md](CONTRIBUTING.md) are the place to start. Contributions written with or
without AI tools are welcome under the same review.

## Layout

```
crates/        Rust libraries and the RawStride program (crate pw-loupe, rawstride.exe)
docs/          Architecture, requirements, ADRs, building, testing
third_party/   Notes, pins and licences for native dependencies
tools/         Developer scripts and benchmarks
tests/fixtures Small, licence-clean test files only
```

## Stack

| Area | Choice |
|---|---|
| Program | Rust, eframe + egui (renderer glow) |
| RAW decoding | LibRaw (vcpkg, static), plus a fast path for the embedded preview |
| JPEG decoding | libjpeg-turbo (vcpkg, static) |
| Metadata | ExifTool (one persistent `-stay_open` process) |

## Licence

RawStride is licensed under the GNU General Public License, version 3 or later
([LICENSE](LICENSE), [ADR 0002](docs/adr/0002-licensing.md)). LibRaw is used under the LGPL-2.1;
ExifTool is a separate program, used unmodified.

## Clean room

RawStride is developed under a [clean-room policy](docs/clean-room.md). No code, assets or
decompiled material from other products may enter this repository. Product names mentioned in the
repository are trademarks of their respective owners; RawStride is not affiliated with them.
