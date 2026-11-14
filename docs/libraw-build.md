# LibRaw build and installation

Recorded **2026-11-04, before the OpenMP test**, so the working configuration can always be
restored. Nothing in the app (M1 viewer, ratings, ingest) links LibRaw yet; only
`tools/raw-decode-bench` does.

## Baseline (the working version) – unchanged by the OpenMP test

| Item | Value |
|---|---|
| Source of truth | `vcpkg.json` in the repo root (manifest mode) |
| vcpkg baseline | `builtin-baseline` `afa249f12f52ed8a3b919f195495ba8b330288d1` |
| Manifest dependencies | `libjpeg-turbo`, `libraw` (no features) |
| Triplet | `x64-windows-static-md` (static libraries, dynamic MSVC runtime `/MD`) |
| Install root | `C:\Development\PhotoWorkflow\vcpkg_installed\` (git-ignored) |
| Compiler | MSVC 14.51.36231, `cl.exe` from VS 18 Build Tools, Hostx64/x64 |
| LibRaw | **0.22.3**, vcpkg ABI `e9e50a2e1fe0e0f23804f2c876012b44665507697849b7366f9e9ec0ec283ca1` |
| LibRaw features | none → **no OpenMP**, no DNG lossy codec; `libraw_capabilities()` = `0x40` (zlib) |
| LibRaw libraries | `lib\raw_r.lib` (thread-safe, 30 890 120 bytes, used), `lib\manual-link\raw.lib` |
| LibRaw dependencies | jasper 4.2.9 (ABI `1f1607ea…`), lcms 2.19.1#1 (`bbf4aef6…`), zlib 1.3.2#2 (`e363a108…`, `zs.lib`) |
| Also installed | libjpeg-turbo 3.2.0#1 (`d2132288…`), libspng 0.7.4#1 (`39205b51…`) |
| Licence | LibRaw: LGPL-2.1 **or** CDDL-1.0; used under **LGPL-2.1** (owner, 2026-11-13, ADR 0002) |
| Binary cache | `%LOCALAPPDATA%\vcpkg\archives` (restoring all 10 packages took 0.6 s) |

How `raw-decode-bench` links it (`tools/raw-decode-bench/build.rs`):

- Install root: `$VCPKG_INSTALLED_DIR` or `<repo>\vcpkg_installed`, triplet folder
  `x64-windows-static-md`.
- Links `raw_r` statically, then `lcms2`, `jasper`, `jpeg`, `zs` (zlib), `ws2_32`.
- The C shim is compiled with `LIBRAW_NODLL`.

### Restore the baseline

The baseline is never modified. If anything is in doubt:

```
cd C:\Development\PhotoWorkflow
"%VCPKG_ROOT%\vcpkg.exe" install --triplet x64-windows-static-md
```

This re-creates `vcpkg_installed\` from `vcpkg.json` and the binary cache (no rebuild). Check
that `vcpkg_installed\vcpkg\status` lists `libraw 0.22.3` with ABI `e9e50a2e…` and no
`Feature:` lines.

## OpenMP variant (measurement only, 2026-11-04)

Approved by the owner for a separate performance measurement.

- **Separate manifest:** `tools/raw-decode-bench/openmp/vcpkg.json` with `libraw[openmp]` and the
  same baseline.
- **Separate install root:** `C:\Development\PhotoWorkflow\vcpkg_installed_openmp\` (git-ignored).
- **Separate build output:** `target-openmp\` (git-ignored). The bench is built with
  `VCPKG_INSTALLED_DIR` pointing at the OpenMP root.
- With MSVC, OpenMP needs `vcomp140.dll` at run time. If Windows does not already have it, the
  script copies it from the VS Build Tools redist folder **next to the bench executable only**.
- **Undo:** delete `vcpkg_installed_openmp\` and `target-openmp\`. Nothing else is touched.

### Result of the OpenMP measurement (2026-11-04)

- The install worked as planned: `vcpkg_installed_openmp\vcpkg\status` lists `libraw` with
  `Feature: openmp` (ABI features `core;openmp`). `vcomp140.dll` was already in
  `C:\Windows\System32`, so nothing was copied.
- The baseline `vcpkg_installed\` was not modified (status file identical, same mtime).
- Results: `docs/analyses/2026-11-04-m2-libraw-decode.md`, follow-up 2.
- **Adopting OpenMP for the app is a separate decision.** It would mean `libraw[openmp]` in the
  root `vcpkg.json` and shipping `vcomp140.dll` (part of the MSVC redistributable) with the app.

## The app's LibRaw (from 2026-11-04, ADR 0006)

The owner approved OpenMP for the app.

- **Manifest:** the root `vcpkg.json` asks for `{ "name": "libraw", "features": ["openmp"] }`,
  with the same baseline and triplet. `vcpkg_installed\` therefore contains LibRaw 0.22.3 with
  OpenMP, which is the configuration measured as `vcpkg_installed_openmp\` (identical pixels to the
  baseline, follow-up 3). `crates/pw-libraw/build.rs` refuses to build unless the vcpkg status
  lists `libraw` with `Feature: openmp`.
- **Linking:** static `raw_r.lib` + `lcms2`, `jasper`, `jpeg`, `zs`, `ws2_32`. LibRaw's objects are
  compiled with `/openmp`, so the exe imports `vcomp140.dll`.
- **Run-time DLLs:** the exe needs the MSVC runtime, including the OpenMP runtime:
  `vcomp140.dll`, plus `vcruntime140.dll`, `vcruntime140_1.dll` and `msvcp140.dll` as reported by
  `dumpbin`. On a development machine they are in `C:\Windows\System32`.
  `tools\package-dist.ps1` builds `dist\PhotoWorkflow\` and copies every non-Windows DLL the exe
  imports from the VS Build Tools redist folder (`VC\Redist\MSVC\<ver>\x64\Microsoft.VC*.CRT` /
  `…OpenMP`), app-locally. It writes `dist-manifest.txt` (each import: system / shipped /
  missing, with SHA-256) and fails if anything is missing.
- **Back to the single-threaded LibRaw** (only if ever needed): change `vcpkg.json` back to
  `"libraw"` and relax the check in `crates/pw-libraw/build.rs`. Pixels are identical either way.
- **`vcpkg_installed_openmp\` / `target-openmp\`** are no longer needed for the app. They are kept
  for the bench until the measurements are closed, and can be deleted at any time.
