# Distributing RawStride: the portable zip

RawStride is given out as a portable zip (ADR 0031 §3.4, RL-Q4, step RL4). Unpacking it is the
installation; there is no installer and nothing is registered. Deleting the folder and
`%LOCALAPPDATA%\RawStride` removes it.

## The zip

`RawStride-<version>-windows-x64.zip` holds one folder, `RawStride-<version>-windows-x64\`, with
exactly these files at its top level:

| File | What it is | Where it comes from |
|---|---|---|
| `rawstride.exe` | The program (eframe + egui). LibRaw with OpenMP, LittleCMS, JasPer, libjpeg-turbo, libspng and zlib are linked statically (vcpkg `x64-windows-static-md`). | `cargo build --release --locked -p pw-loupe` |
| `MSVCP140.dll`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` | The Microsoft Visual C++ runtime | The Visual Studio or Build Tools redistributable folder, `VC\Redist\MSVC\<version>\x64\Microsoft.VC<nnn>.CRT` |
| `VCOMP140.DLL` | The Microsoft OpenMP runtime; LibRaw's multithreaded demosaicing needs it | The same folder's `Microsoft.VC<nnn>.OpenMP` |
| `exiftool.exe`, `exiftool_files\` | ExifTool 13.59, unmodified (`exiftool(-k).exe` renamed) | `exiftool-13.59_64.zip` from SourceForge, the same file and SHA-256 as CI |
| `LICENSE` | RawStride's licence, GPL-3.0 | The repository root |
| `THIRD-PARTY-NOTICES.txt` | The notices of everything in the zip that is not RawStride's own | The repository root (`tools/third-party-notices.ps1`, ADR 0031 RL3) |
| `README.txt` | Installing, SmartScreen, removing, licence and where the source is | Written by `tools/package-dist.ps1` |

- The four Microsoft DLLs are deployed app-locally, as Microsoft permits for the Visual C++
  runtime's distributable files. They keep the redistributable folder's own (lower-case) names.
  The script walks the imports of `rawstride.exe` and of each DLL it copies, and stops if the set
  is not exactly these four (then `THIRD-PARTY-NOTICES.txt`'s Microsoft paragraph and the script's
  list are updated together).
- ExifTool is shipped unmodified (ADR 0002 §2.5). RawStride looks for `exiftool.exe` in its own
  folder before PATH (T120), so the copy in the zip is used even when another ExifTool is
  installed. Its `exiftool_files` folder must stay next to it.

## Making it

From the repository root:

```powershell
.\tools\package-dist.ps1                      # builds the release exe first
.\tools\package-dist.ps1 -NoBuild             # uses the existing target\release\rawstride.exe
.\tools\package-dist.ps1 -ExifToolZip <path>  # uses a local exiftool-13.59_64.zip
```

- **It needs** the build tools of [`building.md`](building.md) (Rust, vcpkg, Visual Studio or the
  Build Tools, whose `vswhere.exe`, `dumpbin.exe` and redistributable folder it uses), an up-to-date
  `THIRD-PARTY-NOTICES.txt` in the repository root, and the network for the first ExifTool
  download.
- **`-NoBuild`** prints the exe's last-write time, so a stale exe is seen. **`-ExifToolZip`** skips
  the download; the file is still checked by SHA-256.
- **It writes** only under `target\dist\`: the program folder, the zip (both replaced on each
  run), the cached ExifTool download in `target\dist\download\`, and a temporary unpack folder that
  is removed afterwards. It checks its own result: the DLL set, ExifTool's hash and `-ver`, the
  folder's top level, no empty folders, and the zip's entry count. At the end it prints the zip's
  path, size and SHA-256.
- **A zip built on a developer's machine is for testing only.** Its exe holds the build machine's
  source paths in panic messages: the release profile has `debug = "line-tables-only"` and no path
  remapping, kept for latency measurements (ADR 0031 RL-Q12). A release is built by the release
  workflow (RL6) with the paths remapped, which then calls this script with `-NoBuild`.

## What the PC needs

Windows 10 or 11, x64. Nothing else: no Visual C++ Redistributable, no ExifTool, no WebView2, no
Rust, vcpkg or Visual Studio.

RL5 (the owner, 2026-11-20) ran the zip in a new standard Windows account on a development PC:
it started, rendered a RAW, wrote a rating through the shipped ExifTool and left its folder
unchanged. That PC has the Build Tools and its own graphics driver, so two points stay open:

- **[unverified]** That it runs on a PC that has never had Visual Studio installed.
- **[unverified]** That it needs no more than Windows' own OpenGL (eframe's Glow renderer).

## Installing and removing

- **Installing:** unpack the zip anywhere and start `rawstride.exe`.
- **Removing:** delete the folder and `%LOCALAPPDATA%\RawStride` (`settings.txt`,
  `field-values.txt` and the write journal `journal\`).

## What it writes, and where

- **The program folder:** nothing (checked in RL5, 2026-11-20), so it may be read-only (for example
  under `C:\Program Files`).
- **Per user:** only `%LOCALAPPDATA%\RawStride\`.
- **Next to the images:** only what the user asks for: XMP sidecars, embedded metadata, copies on
  import, and deleting to the Recycle Bin.

## Not signed

The exe is not signed yet, so Windows SmartScreen warns the first time it is started ("More info",
then "Run anyway"; `README.txt` says so). The release page gives the zips' SHA-256 checksums.
Signing comes after `0.1.0` (ADR 0031 RL-Q6, RL8).

## History: the Tauri app's folder (2026-11-04)

The retired Tauri app (ADR 0009 §7, T207) was given out as a plain folder made by an earlier
`tools\package-dist.ps1` (removed by T208). What was learnt there and still holds:

- The four MSVC DLLs were loaded **from the program folder**, even on a machine with the Visual
  C++ Redistributable installed system-wide.
- `VCOMP140.DLL` is always loaded, because the exe imports it, even when no RAW file is rendered.
- The virus scanner delayed the first start of a new exe by 4–12 s (measured).
- The WebView2 runtime and its profile folder under `%LOCALAPPDATA%` were needed by that app only;
  RawStride needs neither.
