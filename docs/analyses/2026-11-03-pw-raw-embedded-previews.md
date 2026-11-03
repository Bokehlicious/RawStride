# 2026-11-03 – pw-raw: embedded previews in ARW, RW2 and DNG

- Module: `pw-raw` (culling fast path), `pw-thumbcache`, `pw-render` (100 % zoom)
- Question: Which embedded previews do our development formats contain, where in the file are they,
  and when do we need a real RAW decode?
- Method: `tools/raw-preview-survey.ps1` (ExifTool 13.59, read-only), run 2026-11-03 20:38.
  Results are in `D:\PhotoWorkflowData\analysis\raw-previews-20261103-203827\` (outside the repo).
  Previews were extracted from up to 3 files per camera/format group; offsets come from all 20 files.
- Test set: 20 files in `D:\PhotoWorkflowData\RAW`, all taken by the project owner.
- REA/Ghidra: not used. This is a file-format question, so binary analysis adds nothing here.

## Measured / observed

### Test set

| Camera (EXIF model) | Firmware | Files | File size | RAW type | RAW dimensions |
|---|---|---|---|---|---|
| Sony ILCE-1 (A1) | v4.00 | 6 | 56.1–58.7 MB | Sony Compressed RAW, 14-bit | 8672 × 5784 |
| Sony ILCE-7RM4A (A7R IVA) | v1.10 | 5 | 62.1–62.3 MB | Sony Compressed RAW, 14-bit | 9600 × 6376 |
| Panasonic DMC-GM1 | Ver.1.1 | 4 | 19.6–19.7 MB | Panasonic RAW 1, 12-bit | 4592 × 3448 (sensor 4816 × 3464) |
| Xiaomi 17 Ultra (phone) | – | 5 | 11.4–20.4 MB | DNG 1.4, CFA 16-bit, JPEG-compressed | 4096 × 3072 |

### Embedded previews (extracted and measured)

| Camera | Preview | Dimensions | Size | Chroma | Position in file |
|---|---|---|---|---|---|
| A1 | ThumbnailImage (IFD1) | 160 × 120 | 7–12 KB | 4:2:2 | ends ≤ 0.06 MB |
| A1 | PreviewImage (IFD0) | 1616 × 1080 | 183–343 KB | 4:2:2 | ends ≤ 0.54 MB (≤ 0.9 % of file) |
| A1 | **JpgFromRaw (IFD2)** | **8640 × 5760** | 5.2–7.7 MB | 4:2:2 | ends ≤ 8.5 MB (≤ 14.5 % of file) |
| A7R IVA | ThumbnailImage (IFD1) | 160 × 120 | 8–10 KB | 4:2:2 | ends ≤ 0.05 MB |
| A7R IVA | PreviewImage (IFD0) | 1616 × 1080 | 774–917 KB | 4:2:2 | ends ≤ 1.07 MB (≤ 1.7 % of file) |
| A7R IVA | *(no larger JPEG)* | – | – | – | – |
| GM1 | ThumbnailImage (IFD1) | 160 × 120 | 5–6 KB | 4:2:2 | ends ≤ 0.02 MB |
| GM1 | JpgFromRaw | 1920 × 1440 | 545–655 KB | 4:2:2 | not reported (see limitations) |
| Xiaomi DNG | PreviewImage (SubIFD) | 4096 × 3072 | 4.7–8.3 MB | 4:2:0 | at the **end** of the file (ends at 100 %) |

All extracted previews are baseline JPEG (Huffman).

Orientation tags observed: A1 = 1 (normal) in all files. A7R IVA = 8 (rotate 270° CW) in all files.
GM1 = 1 or 8. Xiaomi = 1 or 6.

## Interpretation

- **Inference: the A1 can do 100 % zoom without a RAW decode.** Its full-resolution JPEG (8640 × 5760)
  covers practically the whole RAW frame (8672 × 5784). It is a camera-rendered JPEG, though, and not
  our own development of the RAW data.
- **Inference: the A7R IVA, GM1 and phone DNG need a LibRaw decode for 100 % zoom.** For the A7R IVA the
  largest embedded image is 1616 px wide, only about 17 % of the RAW width. That makes the RAW path
  (and caching its result) critical for the A7R IVA.
- **Inference: the culling fast path needs very little I/O for Sony files.** The 1616 × 1080 preview lies
  in the first ~1 MB of a ~60 MB file, so showing a preview means reading under 2 % of the file. For the
  A1, the full-resolution JPEG lies in the first ~8.5 MB. This matters for browsing straight from cards and HDDs.
- **Inference: DNG needs a different read pattern.** In these phone DNGs the preview sits at the end of the
  file, so it has to be read with a seek rather than by reading the start of the file.
- **Hypothesis: 1616 × 1080 is too small for a sharp fit-to-screen view on large monitors.** On a
  2560 × 1440 screen it has to be upscaled about 1.3×. For the A1, a JPEG decode of JpgFromRaw at a reduced
  scale (1/4 gives 2160 × 1440) could be a sharp and still cheap fit view. **Not measured yet.**
- **Inference: the A7R IVA embeds a higher-quality preview than the A1.** At the same 1616 × 1080 size,
  its preview is about 3× larger in bytes (≈ 0.9 MB vs ≈ 0.25 MB).
- **Inference: orientation must be applied by us.** The extracted previews are stored unrotated, and the
  orientation comes from the EXIF/TIFF tag.

## Limitations

- **All Sony files are "Sony Compressed RAW"** (lossy). That is the only Sony format used in the project,
  so Lossless Compressed and Uncompressed are out of scope for the test set.
- **One firmware version per camera.**
- **The RW2 JpgFromRaw offset was not captured.** The script asks for TIFF IFD tags, and Panasonic stores
  this preview in its own IFD. Its dimensions and size were measured from the extracted file.
- **Only one DNG source, a phone.** DNGs from Adobe DNG Converter or from cameras that write DNG natively
  may be organised differently.
- **Previews were extracted from at most 3 files per group.** The offsets and lengths cover all 20 files.
- **No timing was measured.** Read times and JPEG decode times are the next measurement.
- **LibRaw has not been checked yet.** We have not confirmed that LibRaw's `thumbs_list` reports the same
  previews (expected from LibRaw ≥ 0.21; inference from its documentation).

## Follow-up

- [ ] Add DNGs from at least one non-phone source.
- [ ] Extend the script to record the RW2 preview offset.
- [ ] Benchmark (own code): read time for the preview vs. the whole file (C:, D:, HDD, card reader);
      JPEG decode time for 1616 × 1080 and for 8640 × 5760 at full and 1/2, 1/4 and 1/8 scale.
- [ ] Verify LibRaw `thumbs_list` and `unpack_thumb_ex` against these numbers.
- [ ] Decide the per-format fast-path strategy, as an ADR in the `pw-raw` design.
