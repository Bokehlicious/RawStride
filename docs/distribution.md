# Distributing PhotoWorkflow to another Windows 11 PC

This file describes the state after M2 step 5 (2026-11-04), verified with `tools\m2-step5.cmd`
and `tools\verify-dist.ps1`. The analysis is in `docs/analyses/2026-11-04-m2-pipeline-steps.md`
(step 5).

## What the program folder contains

`tools\package-dist.ps1` builds `dist\PhotoWorkflow\`. Copying this folder is the whole
installation; there is no installer and nothing is registered.

| File | What it is | Source |
|---|---|---|
| `photoworkflow.exe` | The app (Tauri 2 + Rust). The web UI is embedded in the exe. LibRaw 0.22.3 with OpenMP, LittleCMS 2, JasPer, libjpeg-turbo and zlib are linked statically (vcpkg `x64-windows-static-md`). | `cargo build --release` |
| `VCOMP140.DLL` | Microsoft OpenMP runtime; LibRaw's multithreaded demosaicing needs it | VS Build Tools `VC\Redist\MSVC\14.51.36231\x64\Microsoft.VC145.OpenMP` |
| `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`, `MSVCP140.dll` | Microsoft Visual C++ runtime (app-local copies) | VS Build Tools `VC\Redist\MSVC\14.51.36231\x64\Microsoft.VC145.CRT` |
| `README.txt`, `dist-manifest.txt` | Requirements; every import and SHA-256 | `package-dist.ps1` |

- Every DLL import of every file resolves either to this folder or to Windows itself (System32 or
  the API sets `api-ms-win-*`).
- At run time the four MSVC DLLs are loaded **from the program folder**. This holds even on a
  machine where the Visual C++ Redistributable is installed system-wide.
- `VCOMP140.DLL` is always loaded, because the exe imports it, even with RAW rendering off.

## What the target PC needs

| Requirement | Needed for | On Windows 11 |
|---|---|---|
| Windows 10/11 **x64** | everything | yes |
| **Microsoft Edge WebView2 Runtime** (Evergreen) | the whole UI | included in Windows 11 and kept up to date by Windows; verified with version 156.0.4314.58 |
| **ExifTool** (`exiftool.exe` on PATH) | stars, colour labels, IPTC on ingest | **not included**; see below |
| Nothing else | – | no Visual C++ Redistributable, Rust, vcpkg, Visual Studio or Node needed |

**Without ExifTool** the viewer, contact sheet, loupe, RAW rendering and copying on ingest all
work. The app shows "Kunde inte läsa betyg: ExifTool: could not start exiftool: program not found
(is ExifTool on PATH?)", and ratings, labels and IPTC cannot be written. This was verified with a
cleaned environment.

To install ExifTool, use the Windows package from exiftool.org (for example under
`%LOCALAPPDATA%\Programs\ExifTool`) and put its folder on the user's PATH. ExifTool has its own
licence (Perl Artistic / GPL) and is deliberately not shipped with PhotoWorkflow.

## What the app writes, and where

- **Program folder:** nothing. After all the runs it was unchanged, so it can be read-only (for
  example under `C:\Program Files`).
- **Per user:** `%LOCALAPPDATA%\com.photoworkflow.app\`, the WebView2 profile (about 70 MB after
  the tests). Remember-settings such as "RAW-rendering" live in it.
- **Next to the images:** only what the user asks for. That is XMP sidecars for ratings and labels
  on RAW files, embedded XMP in DNG, JPEG and TIFF, and copied files on ingest.

## No development dependencies

- **Environment:** the verification ran the copied folder (`D:\PhotoWorkflowData\dist-test\`)
  with only Windows' own environment variables. PATH held only the Windows folders, so there was
  no cargo, vcpkg, Visual Studio, ExifTool or `VCPKG_ROOT`.
- **Loaded files:** besides the folder's own files and Windows, the process loaded only the
  WebView2 runtime. The antivirus injected two hook DLLs, which are not dependencies
  of the app.
- **Child processes:** only `msedgewebview2.exe`. The app also starts `exiftool` when ratings are
  used, and Windows PowerShell for "eject card" on ingest; PowerShell is part of Windows.
- **Text in the exe:**
  - The exe still contains source paths of Rust crates (`C:\Users\…\.cargo\registry\…`). These
    are panic and backtrace messages only, never opened.
  - The debug directory names `photoworkflow.pdb` without a path.
  - Neither is a dependency. A smaller exe and no user name in it would need `debug = false` and
    `--remap-path-prefix` in a release profile (not done).

## Before giving it to someone else (open points)

1. **Licence notices** have to go with the folder (a `THIRD-PARTY-NOTICES.txt`):
   - LibRaw: LGPL-2.1 or CDDL-1.0, linked statically; the choice is still open (ADR 0002). With
     LGPL and static linking, relinking must be possible.
   - LittleCMS (MIT), JasPer (JasPer License 2.0), libjpeg-turbo (IJG/BSD-3/zlib) and zlib.
   - Rust crates and Tauri (MIT/Apache-2.0).
   - The MSVC runtime files, which are redistributable under the Visual Studio licence.
2. **ExifTool:** document the installation, or decide whether to bundle it (a licence question).
3. **Code signing:** the exe is unsigned. Expect SmartScreen to warn the first time, and the
   virus scanner delays the first start of a new exe by 4–12 s (measured).
4. **A clean PC:** the verification ran on the development machine with a cleaned environment and
   the program in another folder. A final check on a PC (or Windows Sandbox) that has never had
   Visual Studio installed is recommended. It needs no installation on that PC; just copy the
   folder and start `photoworkflow.exe`.
