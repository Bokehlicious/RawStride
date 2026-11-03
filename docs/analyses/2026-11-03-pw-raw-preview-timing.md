# 2026-11-03 – pw-raw: read and decode time of embedded previews

- Module: `pw-raw` (fast path), `pw-thumbcache`, `pw-render` (100 % zoom)
- Question: How long does it take to get an embedded preview from disk to decoded pixels, and is a
  scaled decode of the A1's full-size JPEG a cheap way to get a sharp fit-to-screen view?
- Build: `tools/preview-bench`, profile `profiling`, rustc 1.99.1 MSVC, zune-jpeg 0.4.x,
  jpeg-decoder 0.3.x (`default-features = false`). Binary SHA-256 `0b69f556…15ac`.
- Machine: thanos (i7-9700K, 32 GB). Test files on **D: (Samsung 850 PRO, SATA)**.
- Input: `raw-previews-20261103-203827\metadata.csv` (20 files; see the embedded-previews analysis).
  Each measurement was repeated 5 times; whole-file reads 3 times. Raw data is in
  `D:\PhotoWorkflowData\analysis\raw-previews-20261103-203827\preview-bench.csv` (outside the repo).
- RW2: only the thumbnail was measured, because the survey did not capture the 1920 × 1440 preview offset.

## Measured / observed

The values below are the median of the per-file medians, in milliseconds. "Uncached" means the read
bypassed the Windows file cache (`FILE_FLAG_NO_BUFFERING`) and includes opening the file.

### Reading

| Camera | What | Bytes (median) | Uncached | Cached |
|---|---|---|---|---|
| A1 | whole file | 55.3 MB | **105.7** | – |
| A1 | PreviewImage 1616 × 1080 | 235 KB | **0.60** | 0.07 |
| A1 | JpgFromRaw 8640 × 5760 | 7.0 MB | 14.3 | 2.1 |
| A7R IVA | whole file | 59.4 MB | **113.3** | – |
| A7R IVA | PreviewImage 1616 × 1080 | 899 KB | **1.86** | 0.11 |
| GM1 | whole file | 18.7 MB | 36.6 | – |
| Xiaomi DNG | whole file | 11.6 MB | 23.1 | – |
| Xiaomi DNG | PreviewImage 4096 × 3072 (at end of file) | 5.2 MB | 10.7 | 1.6 |

Whole-file reads ran at about 530–550 MB/s.

### Decoding (one call per decode; see the threading note under Limitations)

| Camera | Preview | zune-jpeg full | jpeg-decoder 1/1 | 1/2 | 1/4 | 1/8 |
|---|---|---|---|---|---|---|
| A1 | PreviewImage 1616 × 1080 | **5.97** | 8.71 | 5.82 | 5.04 | 4.79 |
| A1 | JpgFromRaw 8640 × 5760 | **173.9** | 218.1 | 138.7 | 118.1 (→ 2160 × 1440) | 113.2 |
| A7R IVA | PreviewImage 1616 × 1080 | **10.69** | 14.24 | 11.75 | 10.58 | 10.34 |
| Xiaomi DNG | PreviewImage 4096 × 3072 | **67.4** | 82.0 | 61.6 | 57.8 | 56.7 |
| all | Thumbnail 160 × 120 | 0.10–0.12 | 0.34–0.44 | – | – | – |

Spread: per-file medians varied by roughly ±10 % for the Sony files. The DNG files showed up to +50 % on
the slowest file, in both reading and decoding.

### REA (targeted)

- **Why:** to check two assumptions behind the numbers: that the benchmark binary is a normal optimised
  build with no unexpected DLLs, and that the decoders run on a single thread.
- **`inspect_pe_imports`** (`ev_c5d46e41…`): 10 DLLs and 100 functions, all standard Windows and CRT
  imports. No delay-load imports. **`KERNEL32!CreateThread` is imported**, but our own code never starts a thread.
- **Ghidra (`open_binary` + `binary_overview` / `search_strings`)** on the 565 KB binary: **did not finish**.
  The first query after opening the binary exceeded the device bridge's 60-second limit three times, and
  twice the REA session was lost. The analysis was **stopped** because it had become disproportionate to
  the question. No Ghidra results were obtained.

## Interpretation

- **Inference: the Sony culling fast path is well under a 50 ms budget, even with a cold OS cache.**
  Reading plus decoding the 1616 × 1080 preview takes about **6.6 ms for the A1** and about
  **12.6 ms for the A7R IVA**, on one core, from a SATA SSD.
- **Inference: reading only the preview is about 175× (A1) and 60× (A7R IVA) faster than reading the
  whole file** (0.60 vs 105.7 ms and 1.86 vs 113.3 ms). The fast path must not read the whole RAW file.
- **Hypothesis refuted: a scaled decode of the A1's JpgFromRaw is not cheap.** At 1/4 scale it still takes
  ~118 ms, and even 1/8 takes ~113 ms. The likely cause (inference) is that entropy decoding of the 7 MB
  bitstream dominates, and scaling only saves IDCT and colour-conversion work. A sharp fit view from
  JpgFromRaw therefore costs ~130 ms (read plus decode). It has to be prepared in the background,
  not produced on keypress.
- **Inference: 100 % zoom on the A1 from the embedded JPEG costs ~190 ms** (14 ms read + 174 ms decode)
  on one core. For instant zoom it has to be pre-decoded or prefetched for the next and previous images,
  or decoded in tiles. This is a requirement for `pw-thumbcache` and `pw-render`.
- **Inference: zune-jpeg is faster than jpeg-decoder for full-size decodes** (−30 % for 1616 px and −20 %
  for 8640 px). Its README says it uses AVX2/SSE and decodes single-threaded. We have **not verified this
  with REA** because the Ghidra session did not complete.
- **Hypothesis: jpeg-decoder may have used extra threads.** Its source has a "multithreaded" worker that
  spawns one std thread per colour component when rayon is disabled. This would explain the
  `CreateThread` import. If so, the jpeg-decoder numbers are not single-core numbers, and its comparison
  with zune-jpeg is not like for like. **Not verified.**

## Limitations

- Only one drive (D:, SATA SSD) was measured. NVMe (C:), the HDDs and the card reader have not been measured yet.
- `FILE_FLAG_NO_BUFFERING` bypasses the OS cache only; the SSD's own cache still applies.
- Decoders produced RGB output in their default format. Upload to the GPU is not included.
- RW2's 1920 × 1440 preview was not measured, because its offset is missing.
- libjpeg-turbo, the likely production candidate, has not been measured yet.
- The threading of jpeg-decoder and the SIMD use of zune-jpeg are not verified (see the REA section).

## Follow-up

- [ ] Find out how many threads jpeg-decoder uses, by a measurement (thread count or CPU time vs wall
      time) rather than Ghidra. Rerun its numbers single-threaded if possible.
- [ ] Add libjpeg-turbo (vcpkg) to the benchmark; it supports scaled decoding and SIMD.
- [ ] Measure on C: (NVMe), an HDD and a card reader.
- [ ] Capture the RW2 preview offset in the survey script, and measure it.
- [ ] Multi-core throughput: previews decoded per second across 8 threads, for contact-sheet generation.
