# Distributing RawStride: the portable zip and the installer (MSI)

RawStride is given out in two forms, both in every release (ADR 0031 RL9-Q8):

- **The portable zip** (ADR 0031 §3.4, RL-Q4, step RL4). Unpacking it is the installation;
  nothing is registered. Deleting the folder and `%LOCALAPPDATA%\RawStride` removes it.
- **The installer, an MSI** (ADR 0031 RL-Q5, step RL9). It installs the same files for the current
  user, with a Start menu shortcut and an entry in Installed apps, which uninstalls it.

## The zip

`RawStride-<version>-windows-x64.zip` holds one folder, `RawStride-<version>-windows-x64\`, with
exactly these files at its top level:

| File | What it is | Where it comes from |
|---|---|---|
| `rawstride.exe` | The program (eframe + egui). LibRaw with OpenMP, LittleCMS, JasPer, libjpeg-turbo, libspng and zlib are linked statically (vcpkg `x64-windows-static-md`). | `cargo build --release --locked -p pw-loupe` |
| `MSVCP140.dll`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` | The Microsoft Visual C++ runtime | The Visual Studio or Build Tools redistributable folder, `VC\Redist\MSVC\<version>\x64\Microsoft.VC<nnn>.CRT` |
| `VCOMP140.DLL` | The Microsoft OpenMP runtime; LibRaw's multithreaded demosaicing needs it | The same folder's `Microsoft.VC<nnn>.OpenMP` |
| `exiftool.exe`, `exiftool_files\` | ExifTool 13.64, unmodified (`exiftool(-k).exe` renamed) | `exiftool-13.64_64.zip` from SourceForge, the same file and SHA-256 as CI |
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

## The installer (MSI)

`RawStride-<version>-windows-x64.msi` installs the files of the zip's folder (ADR 0031 RL9-Q3 to
RL9-Q11):

- **What and where:** every file of the zip's folder, unchanged, except `README.txt` (it describes
  the zip: "unpack this folder", "delete this folder"; RL9-Q9), per user into
  `%LOCALAPPDATA%\Programs\RawStride`. **No administrator rights** are needed.
- **One shortcut**, in the Start menu; none on the desktop, and no file types are associated.
- **No dialogs:** only Windows' progress bar, and no licence page to accept (the GPL asks for
  none); `LICENSE` and `THIRD-PARTY-NOTICES.txt` are in the program folder as in the zip.
- **Upgrade:** a newer version replaces the older one. An MSI of the **same** version replaces the
  installed one too, so there is always one RawStride in Installed apps (RL9-Q10). With a newer
  version already installed the MSI stops with "A newer version of RawStride is already
  installed." An installation that fails halfway is rolled back by Windows.
- **Close RawStride before upgrading.** Otherwise Windows Installer asks to close RawStride and
  ExifTool, and then goes on (RL9-Q11, checked by the owner on 2026-11-24).
- **Uninstall** in Installed apps: the program folder and the shortcut are removed.
  `%LOCALAPPDATA%\RawStride` (settings, field values, the write journal) is kept, and images and
  sidecars are never touched (RL9-Q6).
- **A zip copy and an installed copy share** `%LOCALAPPDATA%\RawStride`: the same settings, field
  values and write journal (RL9-Q8).
- **Not signed** yet, as the zip (below): SmartScreen may warn.

## Making it

From the repository root:

```powershell
.\tools\package-dist.ps1                      # builds the release exe first
.\tools\package-dist.ps1 -NoBuild             # uses the existing target\release\rawstride.exe
.\tools\package-dist.ps1 -ExifToolZip <path>  # uses a local exiftool-13.64_64.zip
.\tools\package-msi.ps1                       # the MSI, from the folder package-dist.ps1 made
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
- **`package-msi.ps1`** makes `target\dist\RawStride-<version>-windows-x64.msi` from the program
  folder `package-dist.ps1` made (run that first) and `tools\msi\RawStride.wxs`. It needs the .NET
  SDK and WiX 7.0.1 as a .NET tool (`dotnet tool install --global wix --version 7.0.1`); the
  script accepts WiX's EULA for each run only. It checks its result without installing anything:
  it validates the MSI (the ICE checks) and compares an administrative image of it with the
  program folder. `-Version <x.y.z>` gives the same files another version, for the upgrade test
  only.
- **A zip built on a developer's machine is for testing only.** Its exe holds the build machine's
  source paths in panic messages: the release profile has `debug = "line-tables-only"` and no path
  remapping, kept for latency measurements (ADR 0031 RL-Q12). A release is built by the release
  workflow `.github/workflows/release.yml` (RL6, below): its `tools/release-build.ps1` builds the
  exe with the paths remapped, checks it, and then calls this script with `-NoBuild`.

## Making a release

A release is built by CI on a pushed tag, not on a developer's machine (ADR 0031 RL6, RL7):

1. The `[Unreleased]` section of [`CHANGELOG.md`](../CHANGELOG.md) is renamed to
   `[<version>] - <date>`, with an empty `[Unreleased]` above it.
2. `main` is fast-forwarded to that commit and pushed, and its CI run is green. The release
   workflow runs no tests of its own.
3. The tag `v<version>` (the workspace version, for example `v0.1.0`) is put on that `main`
   commit and pushed, on the owner's yes.
4. The workflow `Release` checks that the tag is on `main` and names `pw-loupe`'s version, runs
   `tools/release-build.ps1 -Section <version>`, and creates a **draft** GitHub Release (a
   pre-release while the version starts with `0.`) with the zip, the MSI, `SHA256SUMS.txt` and
   the notes. The runner gets WiX 7.0.1 as a .NET tool first.
   If a release for the tag already exists the run fails; the draft is deleted and the run started
   again.
5. The owner reads the draft and publishes it by hand.

**A test run:** Actions ▸ Release ▸ Run workflow, on any branch (GitHub offers the button only once
the workflow is on the default branch), with the input `section` (`Unreleased` by default; a
version once `Unreleased` is empty). It makes the same files as the artifact `release` but no
release.

**What `tools/release-build.ps1 -Section <name>` checks**, stopping at the first failure:

- a `-Section` other than `Unreleased` equals `pw-loupe`'s version, and `CHANGELOG.md` has exactly
  one non-empty section of that name (its text becomes `target\dist\release-notes.md`, followed by
  a fixed Download section);
- `THIRD-PARTY-NOTICES.txt` is what `tools/third-party-notices.ps1` makes today (after
  `cargo fetch --locked`; it needs cargo-about 0.9.3; `-SkipNotices` leaves this out for a local
  run without it);
- the exe, built with `--remap-path-prefix` for the repository root and the cargo home folder
  (through `CARGO_ENCODED_RUSTFLAGS`, for that build only; it stops if `RUSTFLAGS` or
  `CARGO_ENCODED_RUSTFLAGS` is already set), holds neither of them nor `%USERPROFILE%`, with `\`
  or `/`, as ASCII or UTF-16LE, and names its PDB as `rawstride.pdb` without a folder;
- `tools/package-dist.ps1 -NoBuild` succeeds, and then `tools/package-msi.ps1` (with the
  release's version) makes `RawStride-<version>-windows-x64.msi` (an old MSI of that name is
  removed at the start, as the old checksum file is); then it writes
  `target\dist\SHA256SUMS.txt` (`sha256sum` form) with two lines, the zip's and the MSI's, in that
  order. Neither file is byte-for-byte reproducible (the zip's entry times, the MSI's package
  code), so the checksums that count are those of the files the release run uploads.

A local run of `release-build.ps1` rebuilds `target\release\` with the remapping; a later plain
`cargo build --release` rebuilds it back.

## What the PC needs

Windows 10 or 11, x64. Nothing else: no Visual C++ Redistributable, no ExifTool, no WebView2, no
Rust, vcpkg or Visual Studio. This holds for the zip and the MSI alike: the MSI installs the same
files.

RL5 (the owner, 2026-11-20) ran the zip in a new standard Windows account on a development PC:
it started, rendered a RAW, wrote a rating through the shipped ExifTool and left its folder
unchanged. That PC has the Build Tools and its own graphics driver, so two points stay open:

- **[unverified]** That it runs on a PC that has never had Visual Studio installed.
- **[unverified]** That it needs no more than Windows' own OpenGL (eframe's Glow renderer).

## Installing and removing

- **With the MSI:** run it; RawStride is then in the Start menu. Remove it in Installed apps; that
  keeps `%LOCALAPPDATA%\RawStride`, which can be deleted by hand afterwards.
- **With the zip:** unpack it anywhere and start `rawstride.exe`. Remove it by deleting the folder
  and `%LOCALAPPDATA%\RawStride` (`settings.txt`, `field-values.txt` and the write journal
  `journal\`).

## What it writes, and where

- **The program folder:** nothing (checked in RL5, 2026-11-20), so it may be read-only (for example
  under `C:\Program Files`).
- **Per user:** only `%LOCALAPPDATA%\RawStride\`.
- **Next to the images:** only what the user asks for: XMP sidecars, embedded metadata, copies on
  import, and deleting to the Recycle Bin.

## Not signed

Neither the exe nor the MSI is signed yet, so Windows SmartScreen warns the first time either is
started ("More info", then "Run anyway"; `README.txt` and the release notes say so). The release
page gives the SHA-256 checksums of both files in `SHA256SUMS.txt`.
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
