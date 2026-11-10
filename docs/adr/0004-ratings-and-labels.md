# ADR 0004: Star ratings and colour labels

- Status: accepted (choices made by the project owner, 2026-11-03)
- Date: 2026-11-03
- 2026-11-10: implemented in the egui program (ADR 0013).

## Context

Rating images is the core of culling. Here is how Photo Mechanic handles it, according to Camera
Bits' public documentation (clean-room level D):

- **Storage.** For RAW files it writes to XMP sidecars by default, "the best option for speed and
  compatibility with other programs". The RAW file is only modified if the user explicitly allows it.
- **DNG.** For DNG, metadata is written into the file itself.
- **Colour classes.** These can be synchronised with the XMP *Label* field "for better Adobe compatibility".
- **Windows shortcuts.** Alt+1–5 sets the rating and Ctrl+1–8 the colour class. Single-key shortcuts
  are optional.

## Decision

1. **Storage.** Ratings and labels go to an XMP sidecar `<stem>.xmp` next to the RAW file, as
   `xmp:Rating` (1–5, -1 = rejected) and `xmp:Label` (Adobe label names: Red, Yellow, Green, Blue,
   Purple). The RAW file is never modified.
2. **Writing through ExifTool** (the owner's choice). One persistent `exiftool -stay_open` process
   for the whole session (`pw-metadata::ExifTool`). It edits existing sidecars in place, so other
   metadata is preserved, and leaves no `_original` backup files.
3. **DNG, JPEG and TIFF: embedded in the file** (the owner's choice, updated 2026-11-03; it
   replaces "no ratings for DNG or JPEG yet"). Lightroom ignores sidecars for these formats, and
   Photo Mechanic also writes DNG metadata into the file. ExifTool runs with
   `-overwrite_original_in_place -P`: only the metadata is rewritten, the image data is untouched,
   the file keeps its identity (creation date, attributes, links) and its modification date
   (to the second). No backup file is left behind.
   - Rewriting can move embedded previews inside the file (measured on a DNG: the preview offset
     moved from 6 484 588 to 9 126). The app therefore forgets its cached inspection of the file
     after each embedded write.
4. **Keys.**

   | Keys | Effect |
   |---|---|
   | 0–5, or Alt+0–5 | Stars (0 clears) |
   | 6–9 | Red, Yellow, Green, Blue |
   | Ctrl+1–5 | Red … Purple |
   | Ctrl+0 | No label |

   Pressing an image's current label again removes it. Because of this, the loupe's
   "fit" key moved from `0` to `F`, and its "100 %" key from `1` to `Z` (which already toggles).
5. **Writing order.** The UI updates at once, and writes are queued in key order. A failed write is
   rolled back in the UI and reported.

## Verification (2026-11-03)

`crates/pw-metadata/tests/sidecar.rs` was run against ExifTool 13.59, the same version as on the
development PC, using its Perl modules:

- Writing creates the sidecar; reading, updating, rejecting and clearing all work.
- An existing sidecar keeps its title and keywords.
- Paths with Swedish characters and spaces work.
- JPEG and TIFF (built in the test) and the real DNG from the test folder get the values
  embedded: the ExifTool `ImageDataHash` is unchanged, no sidecar or backup is created, the
  modification date is kept, and clearing removes both values.
- No backup files are left behind.

Measured in the sandbox, so Windows numbers may differ:

| Operation | Time |
|---|---|
| ExifTool start | ~90 ms |
| First write | ~160 ms (modules load) |
| Each later write | ~4 ms (median) |
| Reading 20 sidecars | ~50 ms |

## Open

- ~~**Clean-room level C check.**~~ **Done 2026-11-10** with Photo Mechanic (ADR 0013 R6, §14): the
  same `xmp:Rating` and `xmp:Label` fields; Photo Mechanic names its sidecar `.XMP` and does not show
  a reject (−1). Lightroom Classic is not checked.
- A write that ExifTool refuses is now reported (T48); a read-only file gives its own error (T49).
- **Concurrent reads during an embedded write.** A preview request that arrives while ExifTool
  replaces a DNG can read stale offsets once; the next request is correct. Acceptable for now.

## Addendum: viewing TIFF (2026-11-03)

Plain TIFFs (`.tif`/`.tiff`) rarely carry an embedded JPEG, so the app decodes the image data
(`tiff` 0.9, pure Rust) and serves JPEG (`jpeg-encoder` 0.7, quality 90) at the same three sizes
as RAW previews: small ≤ 640 px, medium ≤ 2560 px, large = full resolution (decoded only when
zooming in). Decodes are serialised, so a folder of large TIFFs cannot exhaust memory, and the
encoded JPEGs are cached (FIFO, 256 MB). Orientation comes from IFD0 and is applied by CSS, as for
RAW. (Current Tauri UI. In the eframe + egui GUI, orientation is applied when the image is drawn;
see `docs/GUI_REQUIREMENTS.md`.) Supported: 8/16-bit and float Gray, Gray+alpha, RGB, RGBA and CMYK (naive conversion);
palette and YCbCr TIFFs are reported as unsupported. ICC profiles are not applied yet.

## Addendum: filter and multi-selection (2026-11-04)

UI only; no backend change.

- **Filter** (toolbar): stars "≥ 1 … ≥ 4", "5", "unrated", "rejected", combined with a label
  (one colour or "no label"). An active filter is outlined in the toolbar and the status bar
  shows "n / shown (of total)". The filter is remembered between sessions (browser storage).
  An image you rate so that it no longer matches stays visible until you move on, as in
  Lightroom, so it never vanishes under the cursor.
- **Multi-selection** (contact sheet): Ctrl+click toggles, Shift+click and Shift+arrows mark a
  range, Ctrl+A marks all shown images, Esc or a plain arrow clears. Rating and label keys then
  apply to every marked image (one ExifTool write each, queued in order); files that cannot carry
  ratings are skipped silently. Toggling a label removes it only if all marked images have it.
  In the loupe, keys always act on the shown image.
- Test: `ui/tests/filter-select.test.js` (Playwright, mocked backend), 22 checks.
- **2026-11-10: implemented in the egui program (ADR 0014).** The rules above are carried over,
  with these differences: the filter is per folder tab and is not saved between sessions; the star
  list has an extra choice "Alla utom avvisade"; and ADR 0014 amendment 1 (§13): (point 1) a
  condition ≥, =, ≤ beside the star list, where "≤ n" also takes unrated images and a reject never
  matches a number of stars, and (point 2) an image you rate so that it no longer matches leaves at
  once, in the grid and the loupe. ADR 0014 amendment 2 (§14): the label filter takes several labels
  at once. The Tauri app keeps the behaviour described above.

## Note (2026-11-04): ADR 0007

The owner confirmed this ADR's storage rules in ADR 0007 (metadata per IPTC):

- manufacturer RAW → sidecar only
- DNG embedded (an explicit exception)
- JPEG/TIFF embedded
- XMP as the primary representation

ADR 0007 adds one thing: existing IPTC-IIM in JPEG/TIFF/DNG is kept in sync with XMP. Ratings and
labels have no IIM dataset, so this ADR's behaviour is unchanged.
