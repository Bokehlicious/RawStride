# ADR 0025: Exporting images

- Status: **accepted by the owner (2026-11-17):** every question answered, all proposals approved
  (§3). Nothing is built, installed or added as a dependency by this ADR. Each step in §4 becomes
  its own task, with its own approval and commit.
- Date: 2026-11-17
- Related:
  - ADR 0002 and GUI_REQUIREMENTS GR10 (dependencies and licences): this ADR adds a crate of the
    workspace and dependency lines between existing crates, no new package (§2.9; approved by the
    owner, X-Q15);
  - ADR 0006 (the RAW rendering pipeline), ADR 0016 (colour management, `moxcms`), ADR 0009 step
    S3 (`pw-libjpeg`, TurboJPEG 3);
  - ADR 0007 (metadata, ExifTool), ADR 0024 (the XMP rotation), ADR 0021 (the name template,
    collisions and target folder), ADR 0005 (the import's verified write);
  - ADR 0017 U5 (the Tasks area).

## 1. Context

- RawStride culls and captions, but a client, a web page or a printer often needs a JPEG or TIFF
  of a chosen size with the captions inside. Today that needs another program.
- What exists: libjpeg-turbo's TurboJPEG 3 is linked for decoding (`pw-libjpeg`), and the same
  library encodes (`tj3Compress8`); the `tiff` crate =0.9.1 has an encoder; `moxcms` =0.8.1 converts
  colour; `pw-libraw` renders a full RAW. Missing: good scaling, writing metadata into a new file,
  and a place in the code for the whole.

## 2. Decision

### 2.1 Images and command

- The marked images, otherwise the current one. **"Export…"** in the File menu, **Ctrl+Shift+E**
  (X-Q1).

### 2.2 Source

- For a RAW the user chooses (X-Q2):
  - **Embedded JPEG** (the default): the camera's own preview, fast;
  - **RAW**: rendered by LibRaw with the same settings as the view's RAW mode (with every display
    adjustment of ADR 0027 off, that is ADR 0006 §2; owner, 2026-11-18), at full size and in
    16 bits.
- For a JPEG or TIFF the source is the file itself.
- **Copied as it is** (X-Q3): an embedded JPEG exported as JPEG at its original size is copied
  byte for byte, with no new encoding and no loss; its orientation is then written as a tag instead
  of turning the pixels.

### 2.3 Format

- **JPEG** (X-Q4): quality 1–100, 90 by default; no chroma subsampling (4:4:4) from quality 90 up,
  4:2:0 below; encoded by TurboJPEG through `pw-libjpeg`.
- **TIFF** (X-Q5): 8 or 16 bits (16 bits only from a RAW or a 16-bit TIFF), uncompressed or
  Deflate, Deflate by default.

### 2.4 Size and scaling

- Original size, long side in pixels, short side in pixels, or a percentage. An image is never
  enlarged (X-Q6).
- Scaled by RawStride's own separable Lanczos filter in plain Rust, in linear light; no new crate,
  no sharpening (X-Q7).

### 2.5 Colour and orientation

- The output is always **sRGB with its ICC profile embedded**. A RAW is rendered to sRGB; a source
  with another profile is converted with `moxcms` (X-Q8).
- The pixels are turned by the image's orientation, the XMP rotation of ADR 0024 included, and the
  output's Orientation is 1; except the copy of §2.2 (X-Q9).

### 2.6 Metadata in the exported file

- Four choices (X-Q10), "All" the default:
  - **All**: Exif, IPTC and XMP from the file and its sidecar, XMP winning (ADR 0007 rule 4),
    rating and label included;
  - **All except GPS and serial numbers**;
  - **Copyright and contact only**;
  - **None**.
- Written by ExifTool into the new file after it is encoded, before it gets its final name.

### 2.7 Target, name and collisions

- The target folder is chosen with the folder dialog, the favourites and the recent folders. The
  name is a template in ADR 0021's syntax, `{name}` by default; the extension follows the format.
  Collisions are shown in the plan and never overwritten, as in ADR 0021 (X-Q11).
- Each file is written to `.pwpart`, flushed and then renamed without overwriting, as the import's
  copy (X-Q12).

### 2.8 Presets and running

- **Presets** (X-Q13): named sets of every choice (for example "Web 2048", "Print TIFF"), kept in
  `settings.txt` and chosen in the window.
- **Running** (X-Q14): in the background as a task in the Tasks area, with progress per file and
  Stop; files already done are kept. Embedded JPEGs are handled on several threads (half the
  cores); RAWs are rendered one at a time, as LibRaw already uses several threads and much memory.

### 2.9 Code and dependencies

- A new crate **`pw-export`**, depending on `pw-raw`, `pw-libraw`, `pw-libjpeg`, `pw-colour`,
  `pw-metadata` and `tiff` =0.9.1, all already in the workspace; and an encoding function in
  `pw-libjpeg`. No new package, no new licence (X-Q15; owner, 2026-11-17).
- **2026-11-18 (ADR 0027 A-Q7):** the Lanczos filter lives in a new workspace crate without
  dependencies (for example `pw-resample`), shared with `pw-viewcore`; `pw-export` depends on it.
  Made by X2 or ADR 0027's A1, whichever comes first.

## 3. Questions (answered by the owner, 2026-11-17: every proposal approved)

| # | Question | Answer (as proposed) |
|---|---|---|
| X-Q1 | Images and command | The marked, otherwise the current one; "Export…" in the File menu, Ctrl+Shift+E |
| X-Q2 | Source for RAW | Embedded JPEG (default) or RAW rendered by LibRaw as the view's RAW mode, full size, 16 bits; JPEG and TIFF are their own source |
| X-Q3 | Copy as it is | An embedded JPEG at original size to JPEG is copied byte for byte, the orientation as a tag |
| X-Q4 | JPEG | Quality 1–100, 90 default; 4:4:4 from 90, 4:2:0 below; TurboJPEG |
| X-Q5 | TIFF | 8 or 16 bits (16 from RAW or a 16-bit TIFF); none or Deflate, Deflate default |
| X-Q6 | Size | Original, long side, short side or percent; never enlarged |
| X-Q7 | Scaling | Own separable Lanczos in linear light; no sharpening |
| X-Q8 | Colour | sRGB with the ICC profile embedded; other profiles converted with `moxcms` |
| X-Q9 | Orientation | Pixels turned (ADR 0024 included), Orientation 1; except the copy of X-Q3 |
| X-Q10 | Metadata | All (default) / All except GPS and serial numbers / Copyright and contact only / None; by ExifTool |
| X-Q11 | Target and name | Folder dialog, favourites, recent; ADR 0021's template, `{name}` default; collisions shown, never overwritten |
| X-Q12 | Safe write | `.pwpart`, flushed, renamed without overwriting |
| X-Q13 | Presets | Named, in `settings.txt` |
| X-Q14 | Running | A task with progress and Stop; embedded on half the cores, RAW one at a time |
| X-Q15 | Code and dependencies | A new crate `pw-export` of existing workspace crates and `tiff`; an encoder in `pw-libjpeg`; no new package |
| X-Q16 | Steps | X1–X5 as §4 |
| X-Q17 | Not included | As §5 |

## 4. Steps

Each step ends with `.\tools\check.ps1`; the owner's hand check comes after X5.

| Step | Content |
|---|---|
| **X1** | JPEG encoding in `pw-libjpeg` (TurboJPEG 3: quality, subsampling, ICC profile), tested by decoding the result |
| **X2** | `pw-export`: scaling (Lanczos, linear light), orientation and colour conversion in plain Rust, with tests |
| **X3** | The sources (embedded, the byte copy, RAW in 16 bits, JPEG and TIFF) and the JPEG and TIFF writing through `.pwpart` |
| **X4** | Metadata into the exported file with ExifTool, the four choices |
| **X5** | The window, the presets, the task in the Tasks area, the menu item and key; texts and glossary; the owner's hand check (embedded and RAW, JPEG and TIFF, a size, each metadata choice read back in another program, a turned image, Stop) |

## 5. Not included

- Sharpening and watermarks.
- Colour spaces other than sRGB.
- PNG, WebP, HEIF and DNG output.
- Cropping.
- Uploading, e-mail, printing and slide shows.
- Video export.

## 6. Consequences

- Delivery files are made from RawStride with their captions, sized and in sRGB, without another
  program.
- The workspace gets one more crate; no new package or licence.
- A RAW export renders the full image with LibRaw, one at a time, so a large RAW batch takes time;
  the embedded JPEG is the fast default.
