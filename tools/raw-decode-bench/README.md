# raw-decode-bench

This tool measures LibRaw decode time, memory use and parallel throughput, for milestone 2.
It is a development tool, not application code. LibRaw is linked statically from the vcpkg
manifest install (`vcpkg install --triplet x64-windows-static-md` in the repo root), through a
small C shim (`csrc/shim.c`).

```powershell
.\tools\raw-decode-bench\run.ps1      # builds, then runs both measurements on D:\PhotoWorkflowData\RAW
```

- **`single`:** each configuration runs in its own child process, 3 times. Peak memory is
  therefore per configuration.

  | Config | What it does |
  |---|---|
  | `unpack_only` | Opens and decompresses the RAW data. This is the floor for any RAW pixels. |
  | `half_size` | 2×2 binning, no demosaic. Gives half resolution. |
  | `full_linear` | Full resolution, bilinear demosaic. |
  | `full_ahd` | Full resolution, AHD demosaic (LibRaw's default quality). |
  | `crop2048_ahd` | A centred 2048 × 2048 region with AHD, which simulates one tile. |

- **`parallel`:** decodes N files at the same time (full AHD) and reports wall time, images per
  second, CPU/wall ratio and peak memory.
- **Files are pre-read**, so timings are with a warm OS file cache and exclude disk reads.
  The disk read cost (~110 ms per file on D:) is in `docs/analyses/2026-11-03-pw-raw-preview-timing.md`.

## M2 follow-up (2026-11-04)

- **`tiles`:** opens and unpacks a file once, then processes 6 regions at different places and
  the first region again. Sizes 512², 1024², 2048², 2560 × 1440 and 3840 × 2160 (a 100 %
  viewport on QHD and 4K), linear and AHD. Reports the unpack, the **time to the first visible
  region** (open + unpack + first region), the median of the following regions, and whether
  re-processing the first region gives identical pixels (FNV hash).
- **`ptiles`:** one 2560 × 1440 AHD viewport split into 1, 2 or 4 strips, each decoded by its
  own LibRaw instance (own unpack) at the same time.
- **`parallel --config half_size`:** background half-size decoding with 1–4 decoders.

Each script is run from the repository root as
`powershell -NoProfile -ExecutionPolicy Bypass -File tools\raw-decode-bench\<script>.ps1`. The
scripts write their results under the owner's data folder (`D:\PhotoWorkflowData\analysis\`, as
their own comments say), not into the repository.

| Script | What |
|---|---|
| `run.ps1` | The M2 LibRaw decode measurements (`single` and `parallel`). Needs the normal `vcpkg install --triplet x64-windows-static-md` of `docs/building.md` first. |
| `run-tiles.ps1` | `tiles`, `ptiles`, parallel half-size with the baseline LibRaw. Installs nothing. |
| `run-openmp.ps1` | Builds the bench twice (baseline into `target\`, OpenMP into `target-openmp\`) and runs the full suite for both, interleaved. Needs `libraw[openmp]` in the separate `vcpkg_installed_openmp\` (below). |
| `run-verify.ps1` | Pixel check: do the baseline and the OpenMP LibRaw give identical pixels, and which tile overlap makes a tile identical to the same pixels of a full decode? |
| `run-crops.ps1` | Which crop origins and margins reproduce the full decode exactly, with the OpenMP build. |

`run-openmp.ps1`, `run-verify.ps1` and `run-crops.ps1` need the OpenMP variant of LibRaw, installed
once from the repository root into the separate `vcpkg_installed_openmp\` (the baseline
`vcpkg_installed\` is not touched); `docs/libraw-build.md` says how to undo it:

```powershell
& "$env:VCPKG_ROOT\vcpkg.exe" install --triplet x64-windows-static-md --x-manifest-root=tools\raw-decode-bench\openmp --x-install-root=vcpkg_installed_openmp
```

The LibRaw configuration, and how to undo the OpenMP variant, is in `docs/libraw-build.md`.
