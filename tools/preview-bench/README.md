# preview-bench

This tool measures how fast the embedded RAW previews can be read and decoded.
It is a development tool, not application code. See `docs/analyses/` for the results.

Its input is the `metadata.csv` written by `tools/raw-preview-survey.ps1`.

```powershell
# Pure-Rust decoders only
cargo run -p preview-bench --profile profiling -- D:\PhotoWorkflowData\analysis\raw-previews-<ts>\metadata.csv --reps 5

# Including libjpeg-turbo (static, from the vcpkg manifest). Run this once in the repo root first:
vcpkg install --triplet x64-windows-static-md
cargo run -p preview-bench --profile profiling --features turbojpeg -- <metadata.csv> --reps 5
```

The results are written to `preview-bench.csv`, next to the input CSV.

| Metric | Meaning |
|---|---|
| `file_uncached` | Reading the whole RAW file, bypassing the Windows file cache (8 MiB chunks) |
| `read_uncached` | Opening the file and reading only the preview byte range, bypassing the file cache |
| `read_cached` | The same range read through the warm file cache |
| `decode_zune` | Full-size decode with zune-jpeg |
| `decode_jd_1/N` | Decode with jpeg-decoder at scale 1/1, 1/2, 1/4 or 1/8 (scaled IDCT) |
| `decode_tj_1/N` | Decode with libjpeg-turbo at scale 1/1 … 1/8 (only with `--features turbojpeg`) |

- **Times:** wall-clock milliseconds, reported as median, min and max over the repetitions.
- **`cpu_ratio`:** the process CPU time divided by the wall time over all repetitions of a decode metric.
  A value clearly above 1.0 means the decoder used several threads. Windows records process times with
  a resolution of about 15.6 ms, so the ratio is only meaningful for decodes whose repetitions add up to
  well over 100 ms (the A1's full-size JPEG, the DNG previews).
- **Uncached reads:** these use `FILE_FLAG_NO_BUFFERING`, so they bypass the Windows file cache only.
  Drive-internal caches still apply.
