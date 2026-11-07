# ADR 0003: Milestone 1 viewer architecture

- Status: accepted (approved by the project owner, 2026-11-03); **amended 2026-11-07**
- Date: 2026-11-03

> **Amendment 2026-11-07 (owner):** the GUI moves to pure Rust with eframe + egui
> ([GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md)). This ADR describes the M1 app as built, and it stays
> valid for the current Tauri app until the egui GUI replaces it.
> - **Still valid in the new GUI:** points 3 (an embedded JPEG is labelled as such), 4 (rotation always
>   from the RAW file's orientation tag; never applied twice) and 5 (EXIF from `pw-raw`).
> - **Superseded for the new GUI:** point 1 (pixels served to WebView2 through `pw://`), the CSS part of
>   point 4, and point 6 (HTML/CSS/JS frontend in `ui/web`). In egui the images are decoded in Rust and
>   drawn as textures in the same process.

## Context

We want a runnable Windows viewer within 48 hours. Measurements on the test set
(`docs/analyses/2026-11-03-pw-raw-preview-timing.md`) showed that reading only the embedded
preview takes 0.6–1.9 ms, while reading a whole RAW file takes about 110 ms.

## Decision

1. **Images.** `pw-raw` locates the embedded JPEG previews. The Tauri shell serves their bytes
   unchanged through a custom URI scheme (`pw://`, which WebView2 sees as `http://pw.localhost/`).
   WebView2 decodes and composites them (GPU-accelerated through Direct3D on the Arc B580).
   Rust does no pixel decoding in M1.
2. **The scheme only serves files the user opened.** URLs have the form `<generation>/<index>/<size>`.
   No file path is ever accepted, so the scheme cannot read arbitrary files.
3. **An embedded JPEG is not a RAW decode, and the UI says so.** The loupe shows "Inbäddad JPEG …
   (kamerans rendering)". Where the largest embedded JPEG is smaller than the RAW, it states that full
   resolution needs RAW decoding, which is milestone 2.
   100 % zoom uses the largest embedded JPEG. On the A1 that is 8640 × 5760, practically the full
   resolution. On the A7R IVA, RW2 and the DNG files it is not.
4. **Orientation.** Rotation always comes from the RAW file's orientation tag. Some embedded JPEGs carry
   their own EXIF orientation: the test DNG preview has 6. Two measures stop it being applied twice:
   - The server sets that tag to 1 before serving the bytes (`pw_raw::neutralize_orientation`).
   - The frontend uses CSS `image-orientation: none`, with `crossorigin="anonymous"` and a CORS header,
     so that browsers honour it for images from another origin.
5. **EXIF.** `pw-raw` reads the few fields M1 shows (camera, lens, date, exposure, aperture, ISO, focal
   length, exposure compensation, orientation) in the same TIFF walk that finds the previews.
6. **Frontend.** Plain HTML, CSS and JS in `ui/web`, with no npm and no build step. The assets are
   embedded with Tauri's `custom-protocol` feature, so `cargo run -p photoworkflow` is the whole build.

## EXIF crates considered (documentation review, 2026-11-03)

| Crate | Finding |
|---|---|
| kamadak-exif 0.6.1 | TIFF/JPEG/HEIF/PNG/WebP. Documents only IFD0 (primary) and IFD1 (thumbnail). SubIFDs (0x014A), IFD2 and RW2's 0x55 magic are not documented. |
| nom-exif 3.8 | Names CR3, RAF and IIQ, but not ARW, RW2 or DNG. Brings 7 dependencies. |
| little_exif 0.6 | Aimed at writing; "a few dozen tags in IFD0 and ExifIFD"; no RAW formats. |
| rexif 0.7 | JPEG and TIFF; describes itself as "very early stages". |

None documents what M1 needs most: finding the previews in ARW **IFD2** (the A1's 8640 px JPEG), DNG
**SubIFDs**, and Panasonic **RW2** (magic 0x55, tag 0x002E). A TIFF walker was needed anyway, and the
EXIF fields come from the same walk, so `pw-raw` reads them itself with no new dependencies.
It is tested against ExifTool's results on the real test files (`crates/pw-raw/tests/real_files.rs`).
Full metadata, and writing XMP/IPTC, will use ExifTool from milestone 3.
The crates were not tried in code, because the build sandbox has no access to crates.io.

## Consequences

- **More decode work for WebView2.** The A1's 8640 px JPEG takes noticeable time to decode in WebView2.
  It is loaded only when zooming in past the 1616 px preview.
- **No miniature cache yet.** Grids of DNG files decode 4096 px previews; a thumbnail cache comes in
  milestone 2.
- ~~**wgpu postponed.** A native wgpu renderer is not needed for M1. It is reconsidered when RAW-decoded
  tiles (milestone 2) need GPU upload.~~ **2026-11-07:** no separate wgpu renderer is planned. GPU
  drawing goes through eframe's backend (GUI_REQUIREMENTS.md, GR4).
