# ADR 0015: Milestone 6 – TIFF viewing in the egui program

- Status: **accepted by the owner (2026-11-10, step D0).** The scope (§3), E1–E6 as behaviour
  requirements (§7), every proposal in §11 as written, and `tiff` =0.9.1 in `pw-tiff` (§6) are
  accepted. Also accepted: replacing the test `the_folder_lists_supported_files_by_name_without_tiff`
  in step D4 (§8.1). Nothing is built or installed by this ADR. Each step in §9 is its own work part,
  with its own approval and commit.
- **Implemented (2026-11-11):** steps D1–D4 done (agent tasks T63–T66), D5 accepted by the owner;
  D6–D7 are this documentation update.
- Date: 2026-11-10
- Related:
  - ADR 0004, addendum "viewing TIFF" (2026-11-03): today's Tauri behaviour. **Its rules are carried
    over** unless §11 says otherwise; this ADR decides how the egui program implements them;
  - ADR 0009 §3.1 and §3.2 (TIFF left out of milestone 1, "later"), §5.2 (a plain JPEG file is shown
    as itself in both modes);
  - ADR 0007 (IPTC metadata, proposed): **not part of this milestone** (§2);
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) GR8 (feature parity), GR9 (honest source), GR10
    (dependencies need approval), GR11 (testable);
  - `src-tauri/src/tiff_view.rs` and `src-tauri/src/imaging.rs` (today's decoder and pixel helpers).

## 1. Context

- GR8 lists two features left before the Tauri app can be retired: IPTC metadata and TIFF viewing.
- **IPTC in the Tauri app** is only the import template (six fields, written by ExifTool to the new
  copies). RawStride has had it since T39 (ADR 0010 M7c). The info panel of the Tauri app shows Exif
  fields from `pw-raw`, not IPTC. Everything else about IPTC is ADR 0007, which is not approved
  (Q1–Q3 and Q8–Q13 are open).
- **TIFF in the Tauri app** (ADR 0004 addendum): the image data is decoded with the `tiff` crate
  0.9.1 and served as JPEG at three sizes. Decodes run one at a time, and the JPEGs are cached
  (256 MB). Supported: 8/16-bit and float Gray, Gray+alpha, RGB, RGBA, CMYK (naive). Palette and
  YCbCr are reported as unsupported. ICC profiles are not applied. Orientation comes from IFD0. A
  file that `pw-raw` cannot inspect (BigTIFF) still opens with the decoder's size.
- **TIFF in RawStride today:** `pw_loupe::is_listed` leaves TIFF out, so a folder of scans looks
  empty. Everything after the listing already handles TIFF or nearly does:
  - `pw-raw` inspects a plain TIFF (format, size, orientation; `plain_tiff_file` test);
  - `pw-metadata` writes ratings and labels into TIFF (embedded XMP; `jpeg_and_tiff_are_written_in_place`);
  - `pw-ingest` copies TIFF and the import writes the IPTC template into it.
- **Missing:** pixels. `pw-viewcore` opens either a RAW (LibRaw) or a JPEG (embedded or the file
  itself, `pw-libjpeg`). `pw-thumbcache` makes thumbnails from embedded JPEGs only, so a TIFF gives
  `NoPreview`.

## 2. Decision: IPTC is split off (owner, 2026-11-10)

- **This milestone is TIFF viewing only.**
- **IPTC parity is already met.** The Tauri app's only IPTC feature, the import template, is in
  RawStride (T39). GR8's "IPTC metadata" is marked done for parity in step D7.
- **ADR 0007 becomes milestone 7** (moved to milestone 8 by ADR 0016 Q1, 2026-11-11). It is finished (open questions answered, ADR approved) on its
  own and then planned as reading, display and editing in RawStride. That work is new, not parity,
  so it does not block the switch from the Tauri app.

## 3. Scope

### 3.1 In scope

| Area | Content |
|---|---|
| Listing | `.tif` / `.tiff` are listed in the grid and the loupe, sorted with the other files |
| Decoder | One shared TIFF decoder for RawStride and the Tauri app (§4.1): first image of the file to 8-bit RGB, with today's colour types and rules |
| Loupe | A TIFF is shown as itself in both display modes, like a plain JPEG file (ADR 0009 §5.2). Fit, free zoom, 100 % and pan as for other files. The status line says "TIFF-fil W × H" (GR9) |
| Thumbnails | Decoded from the image data and reduced to the cell size on the thumbnail worker |
| Memory | The decoded image counts in the core budget; decodes run one at a time (§4.4) |
| Errors | An unsupported or damaged TIFF shows an error message instead of an image, in Swedish, as other files (G3) |
| Ratings and labels | Work on TIFF as soon as it is listed; no new code expected (`pw-metadata` supports it) |

### 3.2 Out of scope

- IPTC beyond the import template (§2, ADR 0007).
- Colour management: ICC profiles are not applied (as in the Tauri app). CMYK stays a naive
  conversion. Comes with a colour-management decision for all formats (done: ADR 0016, 2026-11-11).
- Palette, YCbCr and other colour types the Tauri app does not support.
- More pages than the first image (multi-page TIFF), layers and alpha display (alpha is dropped).
- A disk cache of TIFF thumbnails (the thumbnail cache is in memory for all formats).
- Upgrading the `tiff` crate (Q2).

## 4. Architecture

### 4.1 Where the code goes

| Part | Crate | Notes |
|---|---|---|
| `decode_rgb8(path, limit) -> Result<Rgb8, TiffError>`, `dimensions(path)`, the pixel helpers (`Layout`, `Rgb8`, `u16_to_u8`, `f32_to_u8`, `to_rgb8`), the decode lock | **new internal crate `pw-tiff`** (Q1) | Moved from `src-tauri/src/tiff_view.rs` and `imaging.rs` with their tests, behaviour unchanged, as T8 moved the ingest helpers. Depends on `tiff` only. `imaging::fit` and `imaging::downscale` stay in `src-tauri`: its RAW view (`rawview.rs`) uses them too |
| The Tauri app | `photoworkflow` (`src-tauri`) | Calls `pw-tiff`; keeps its JPEG encoding, its three sizes and its cache. Its tests stay green (GR8) |
| Opening a TIFF in the loupe | `pw-viewcore` (`native`) | A third session kind `NativeSession::Tiff` that holds the decoded full image. `render_half` and `render_region` reduce or crop it with the existing `scale.rs`. Chosen in `open` by the file type, not by `Source` (§4.2) |
| Which source a file has | `pw-viewcore` (`state.rs`) | `is_plain_jpeg` becomes "a file shown as itself": JPEG and TIFF. `source_in` returns `Source::Jpeg` for both, so the mode key R changes nothing for them |
| Thumbnails | `pw-thumbcache` (`extract.rs`) | `thumbnail` decodes a TIFF with `pw-tiff`, reduces it to the cell with a box filter and applies the orientation as today |
| Listing and status text | `pw-loupe` (library) | `is_listed` lists TIFF; `source_text` gives "TIFF-fil W × H" |

### 4.2 Keeping `Source` with two values (Q3)

- `Source` is the user's display mode (G4): RAW or JPEG. A TIFF has neither a RAW rendering nor an
  embedded JPEG; it is its own pixels, like a plain JPEG file.
- The cache key stays `(position, Source)`. A TIFF is keyed as `Source::Jpeg`, so switching mode
  does not decode it again and no "other mode" entry is prefetched (as for JPEG files today).
- The backend decides how to open the file from its type. The status text names the real source
  (GR9), so the shared key value is never shown to the user.

### 4.3 Orientation and size

- Orientation from IFD0 through `pw_raw::inspect`, applied once at drawing, as for every file
  (ADR 0003 point 4).
- If `pw-raw` cannot inspect the file (BigTIFF), the decoder's size is used and the orientation is 1,
  as in the Tauri app.

### 4.4 Memory and decode cost

- Decoding is slow compared with an embedded JPEG: a 61 MP 16-bit RGB TIFF needs about 360 MB
  while decoding and gives 183 MB of RGB. It takes seconds, not milliseconds.
- **One TIFF decode at a time per process** (a lock in `pw-tiff`), shared by the loupe workers and
  the thumbnail worker, as in the Tauri app. A grid of large TIFFs cannot then fill the memory.
- The decoded image of a session counts in `session_bytes`, so the core budget (V8) evicts it like a
  RAW session.
- **Decoder limit (Q4):** the `tiff` crate's buffer limit is set to the core budget instead of
  "unlimited" (the Tauri app). A file larger than that gives an error message, not an exhausted
  machine.
- A thumbnail of a TIFF costs a full decode. Thumbnails are requested for visible cells first, as
  today, so the cost follows the scroll position.

## 5. Display

- **Loupe status line:** "3 / 31 · Scan 01.tif · TIFF-fil 6000 × 4000 · …". The size is the
  decoded image's.
- **While decoding:** the previous image stays until the new one is complete, with the neutral
  indicator after 1 s (V2 swap rule). No new text.
- **Grid:** a TIFF cell shows the placeholder until its thumbnail is ready, as other files.
- **Errors (Swedish, after "Filen kan inte visas: "):** "TIFF-varianten stöds inte än (…)" with the
  decoder's detail (a palette TIFF is reported by the `tiff` crate itself, §9.1), "TIFF-filen är för
  stor för minnesbudgeten (… MB)", and the decoder's message for a damaged file.

## 6. Dependencies

- **`tiff` =0.9.1** (MIT), default features, in the new crate `pw-tiff` only (Q2). It is already in
  the workspace through the Tauri app, with its dependencies `flate2`, `jpeg-decoder` and `weezl`, so
  **no new crate is added to `Cargo.lock`**. Needs the owner's approval (GR10).
- The workspace `Cargo.toml` gains the member and workspace dependency `pw-tiff`. `pw-viewcore`
  (feature `native`), `pw-thumbcache` and `photoworkflow` gain a dependency on it; `photoworkflow`
  drops its direct `tiff` dependency.
- `jpeg-encoder` stays in the Tauri app only.

## 7. Behaviour requirements

Proposed as behaviour requirements now and targets after a first measurement, as in ADR 0013 §7 and
ADR 0014 §7:

| # | Requirement | Target |
|---|---|---|
| **E1** | A folder with TIFFs lists them, and each supported TIFF is shown in the grid and the loupe with the right orientation and size | Tests on synthetic TIFFs; by hand on the owner's set (Q5) |
| **E2** | The pixels equal the Tauri app's decode for every supported colour type and bit depth (the same `pw-tiff` code) | Tests: moved tests keep their assertions; one test per colour type through `pw-viewcore` |
| **E3** | An unsupported, damaged or too large TIFF shows an error message, never a crash and never another file's image (G3) | Tests per case |
| **E4** | Decoding never runs on the UI thread, and never more than one TIFF at a time (G2) | Tests for the lock; by hand: scroll a grid of large TIFFs and step the loupe at the same time |
| **E5** | Stepping to a **prefetched** TIFF meets V1. A cold TIFF follows the V2 swap rule (Q6) | By hand with `latency.ps1` on the owner's set |
| **E6** | Ratings and labels work on TIFF in grid and loupe, and survive a reopen | By hand; the existing `pw-metadata` test covers the write |

Results (D5, 2026-11-11, the owner's acceptance; the reviewer's checks from the log in
`docs/tasks/done/log-2026-11-08-to-2026-11-13.md`):

| # | Result |
|---|---|
| **E1** | Tests on synthetic TIFFs (T63–T66). The real-file tests of T64 (loupe, both modes, size of `pw_tiff::dimensions`) and T65 (thumbnails, long edge 256) pass on the owner's 9 TIFFs. Checked by hand (T66, D5): listed, thumbnails in the grid, shown in the loupe with "TIFF-fil W × H", zoom to 100 % and pan |
| **E2** | The decoder is the Tauri app's code, moved with its tests (T63); one test per colour type and an LZW twin in `pw-tiff`; `downscale` pinned bit for bit against the old code (T65) |
| **E3** | Tests: palette (`Unsupported`), truncated file, a file over the limit, a missing file (T63); the Swedish loupe texts, never `damaged` (T64); a palette thumbnail is `Unreadable` (T65). The owner's set has no palette or damaged TIFF (§9.1), so this rests on the automated tests |
| **E4** | Tests for the lock (T63). Checked by hand (D5): the UI stays responsive while the grid loads thumbnails and the loupe steps |
| **E5** | The owner's `latency.ps1` run (`D:\PhotoWorkflowData\pm-study\results\latency-rawstride.csv`, label `t66-tiff-next`; RAW mode; the 9 TIFFs copied alone to `D:\PhotoWorkflowData\TIFF`; RIGHT × 8, 1.5 s apart, so each next image was prefetched): first change 15.5–30.7 ms, **median 16.1 ms**, one change each. V1 (median ≤ 30 ms) and V2 are met at fit. The first press was the slowest (30.7 ms); the others 15.5–16.5 ms, about one frame. Not measured: 100 % zoom, and cold (not prefetched) TIFFs |
| **E6** | Checked by hand (D5): ratings and labels on TIFFs in grid and loupe survive a reopen |

## 8. Testing and acceptance

### 8.1 Automated (`cargo test`)

- `pw-tiff`: the moved `imaging.rs` tests unchanged; synthetic TIFFs (8-bit and 16-bit RGB, Gray,
  RGBA, CMYK, float, LZW) decode to the expected pixels; palette is refused with its colour type; a
  truncated file gives an error; the limit refuses a file over it; the lock lets one decode run.
- `pw-viewcore`: a TIFF is keyed as `Source::Jpeg` in both modes; `render_half` and `render_region`
  of a TIFF session give the expected sizes and pixels; the session counts in `session_bytes`.
- `pw-thumbcache`: a TIFF thumbnail has the cell size and the orientation applied.
- `pw-loupe`: `is_listed` lists `.tif` and `.TIFF` (the test
  `the_folder_lists_supported_files_by_name_without_tiff` is **replaced** in its step, with the
  owner's approval, because this ADR reverses the rule it pins); `source_text` for a TIFF.
- `photoworkflow`: its tests pass unchanged.

### 8.2 By hand (owner)

- The owner's TIFF set (Q5) in the grid and the loupe: sizes, orientation, zoom to 100 %, pan.
- R on a TIFF: the image does not change and the status line still says "TIFF-fil". The mode
  control ("Läge: RAW (R)" / "Läge: JPEG (R)") does switch: it shows the folder's display mode (G4),
  which applies to the next RAW file, exactly as on a plain JPEG file (ADR 0009 §5.2).
- A palette TIFF and a damaged TIFF: error message; stepping on works.
- A grid of large TIFFs while stepping the loupe: the UI stays responsive (E4).
- `latency.ps1` on prefetched TIFFs (E5).
- Rate and label TIFFs, close, reopen (E6). Open the same files in the Tauri app: same ratings.

## 9. Work steps

| Step | Content | Result |
|---|---|---|
| **D0** | Accept this ADR; answer §11; approve `tiff` =0.9.1 in `pw-tiff`; accept E1–E6 | **Done 2026-11-10** |
| **D1** | New crate `pw-tiff`: move the decoder and pixel helpers from `src-tauri` with their tests; the Tauri app calls it; add the lock and the limit (tests) | **Done:** T63 |
| **D2** | `pw-viewcore`: the TIFF session, "shown as itself" for TIFF, budget accounting, the Swedish TIFF error texts (tests) | **Done:** T64 |
| **D3** | `pw-thumbcache`: TIFF thumbnails; `fit` and `downscale` move to `pw-tiff` (tests) | **Done:** T65 |
| **D4** | `pw-loupe`: list TIFF, status text (library tests) | **Done:** T66 |
| **D5** | Acceptance by hand (§8.2) | **Done 2026-11-11** by the owner; see §7 |
| **D6** | Documentation: architecture.md (`pw-tiff`), test inventory, ADR 0004 addendum (egui note), ADR 0009 §3.2 note | **Done:** T67 |
| **D7** | GR8 and the root `README.md`: TIFF viewing done; IPTC parity met through the import template; ADR 0007 is milestone 7 (moved to milestone 8 by ADR 0016 Q1, 2026-11-11) | **Done:** T67 |

D2 and D3 need D1 and do not depend on each other. D4 needs D2 and D3. Each of D1–D4 is one agent
task.

### 9.1 Findings during implementation

- **The owner's test set (2026-11-10, `D:\PhotoWorkflowData\RAW`, 9 files).** All are Lightroom
  Classic exports: 16-bit RGB, uncompressed, one image, not tiled, orientation 1, **ProPhoto RGB** ICC
  profile, XMP ratings 1–4. Sizes 3201×2134 to 7741×5161 (40 MP).
- **Decoding them with `pw-tiff` (T63 review, a temporary probe):** all nine decode, with the same
  pixels without a limit and with a 4 GiB limit. 26–164 ms each with a warm file cache (dev profile;
  dependencies at opt-level 3). That is far faster than the "seconds" of §4.4, so E5 (Q6) may be met
  for more than prefetched files; D5 measures it with a cold cache.
- **Colour (affects D5, not D1).** Without ICC handling (§3.2), ProPhoto RGB is shown as if it were
  sRGB, so these files look clearly dull and desaturated, in the Tauri app as well. The D5 check must
  not count that as a fault of this milestone. Whether colour management comes before the switch is
  an open owner question.
- **What the set does not cover (Q5):** orientation 6, LZW or ZIP compression, 8-bit, grayscale,
  CMYK, palette, BigTIFF, and a file of about 60 MP. The synthetic tests of T63 cover the colour
  types and LZW; orientation and BigTIFF have no file yet.
- **Planning of D2 and D3 (reviewer, 2026-11-10):**
  - The Swedish TIFF error texts (§5) are made in `pw-viewcore` from the `TiffError` kind, not in
    `pw-loupe`. The loupe already shows a decode message after "Filen kan inte visas: ", so no
    message text has to be recognised later.
  - The loupe's limit is the user's core budget, given to `NativeBackend` at start. The thumbnail
    worker does not get the user's setting, so thumbnails use a fixed 4 GiB, the default core
    budget.
  - A thumbnail needs a box filter on `pw-tiff::Rgb8`. The Tauri app's `fit` and `downscale` move to
    `pw-tiff` instead of being written a second time; `src-tauri` re-exports them.
  - `session_bytes` already counts `w × h × 3` for `Source::Jpeg`, which is the decoded TIFF.

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Moving the decoder changes the Tauri app | A regression in the app that must keep working | A move with unchanged tests and messages (as T8); the Tauri decode is the reference for E2 |
| Large TIFFs and prefetch | The core budget fills with one or two decoded neighbours | Sessions count in the budget; the limit (Q4); one decode at a time |
| The decode lock is shared with thumbnails | The loupe waits behind a thumbnail decode | Thumbnails of visible cells only; measured in D5. If it hurts, the loupe gets priority (a follow-up, not now) |
| `tiff` 0.9.1 is old | Some files (CCITT fax, f16) do not decode | Reported as unsupported; upgrade is Q2 |
| Treating TIFF as `Source::Jpeg` | Code that reads `Source::Jpeg` as "a JPEG" gets it wrong | The status text and the backend use the file type; a test pins the TIFF status text |

## 11. Questions and the owner's answers (2026-11-10)

The owner accepted every proposal below as written. Notes from the review for step D5: Q7 is the
proposal most likely to change after measuring (a folder of many large scans), and the shared decode
lock (§10) is checked by stepping to a cold TIFF while thumbnails decode. Q5: the set must at least
hold a 16-bit RGB TIFF from an editor, a large file, one with orientation 6, one LZW file and a
palette TIFF; CMYK and BigTIFF can follow.

| # | Question | Decision (the proposal, accepted) |
|---|---|---|
| **Q1** | Where the shared decoder lives | **A new internal crate `pw-tiff`.** `pw-raw` reads headers only and stays without image decoding; `pw-viewcore` cannot be used by `pw-thumbcache` or the Tauri app without pulling in its workers |
| **Q2** | Which `tiff` version | **=0.9.1**, the version the Tauri app already uses: no new crate in `Cargo.lock`, and the move keeps the behaviour. 0.11.3 is in `Cargo.lock` too but not built on Windows; it would add `fax`, `half`, `quick-error` and `zune-jpeg` 0.5 to the build. An upgrade is its own later step |
| **Q3** | How a TIFF fits the display mode (§4.2) | **As a plain JPEG file:** shown as itself in both modes, keyed as `Source::Jpeg`, opened by file type. Alternative: a third `Source` value, which changes every match on `Source` |
| **Q4** | The decoder's memory limit | **The core budget** (4 GB by default) instead of "unlimited". Say "unlimited" for strict parity |
| **Q5** | Test files for acceptance (none in `D:\PhotoWorkflowData` today) | The owner provides a small set outside the repo: an 8-bit RGB scan, a 16-bit RGB TIFF from an editor, a grayscale scan, a CMYK file, an LZW and a ZIP compressed file, a BigTIFF, one large file (about 60 MP), one with orientation 6, and one palette TIFF |
| **Q6** | Speed target for TIFF stepping | **V1 for prefetched TIFFs only;** a cold TIFF follows V2 (previous image kept, indicator after 1 s). A decode of seconds cannot meet 30 ms |
| **Q7** | Thumbnails of TIFFs that carry an embedded JPEG preview | **Decode the image data, as the Tauri app does.** Using the embedded preview is faster but can be stale after an edit; can be a later option |

## 12. Consequences

- RawStride shows TIFF; GR8 then has no feature left before the switch, apart from the acceptance
  tests.
- A new crate `pw-tiff` holds the decoder that the Tauri app had inside `src-tauri`.
- `pw-viewcore` gains a third session kind; `Source` is unchanged.
- ADR 0007 (IPTC) is planned as milestone 7, after its approval (moved to milestone 8 by ADR 0016
  Q1, 2026-11-11).
- Colour management (ICC) is not applied, so a TIFF with a wide-gamut profile such as ProPhoto RGB
  looks dull. Whether it comes before the switch is an open owner question (§9.1) (done: ADR 0016,
  2026-11-11).
