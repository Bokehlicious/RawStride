# preview-bench

This tool measures how fast the embedded RAW previews can be read and decoded.
It is a development tool, not application code. See `docs/analyses/` for the results.

Its input is the `metadata.csv` written by `tools/raw-preview-survey.ps1`.

```powershell
cargo run -p preview-bench --profile profiling -- D:\PhotoWorkflowData\analysis\raw-previews-<ts>\metadata.csv --reps 5
```

The results are written to `preview-bench.csv`, next to the input CSV.

| Metric | Meaning |
|---|---|
| `file_uncached` | Reading the whole RAW file, bypassing the Windows file cache (8 MiB chunks) |
| `read_uncached` | Opening the file and reading only the preview byte range, bypassing the file cache |
| `read_cached` | The same range read through the warm file cache |
| `decode_zune` | Full-size decode with zune-jpeg (single thread) |
| `decode_jd_1/N` | Decode with jpeg-decoder at scale 1/1, 1/2, 1/4 or 1/8 (scaled IDCT). Note: without rayon, jpeg-decoder may still spawn one thread per colour component, which is not yet verified |

Times are wall-clock milliseconds, reported as median, min and max over the repetitions.
The uncached reads use `FILE_FLAG_NO_BUFFERING`. Drive-internal caches still apply.
