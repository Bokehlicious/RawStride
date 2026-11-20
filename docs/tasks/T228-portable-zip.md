# T228. ADR 0031 RL4: the portable zip, made by tools/package-dist.ps1, and docs/distribution.md rewritten (tools, docs) — Open

- **Why:** ADR 0031 (accepted by the owner 2026-11-20), step RL4, §3.4 and RL-Q4, RL-Q13: the
  first package is a portable zip, `RawStride-<version>-windows-x64.zip`, holding
  `rawstride.exe`, the four Microsoft DLLs, ExifTool unmodified (ADR 0002 §2.5), `LICENSE`,
  `THIRD-PARTY-NOTICES.txt` and a short `README.txt`. Unpacking it is the installation; deleting
  the folder (and `%LOCALAPPDATA%\RawStride`) removes it. Today there is no script that makes it
  (the Tauri app's `tools/package-dist.ps1` was removed by T208), and `docs/distribution.md` still
  describes the Tauri app. RL5 (the owner runs the zip on a clean PC or in Windows Sandbox) and
  RL6 (the release workflow, which calls this script after a release build with
  `--remap-path-prefix`) come after this task.
- **What the reviewer checked (2026-11-20):**
  - `target\debug\rawstride.exe` imports, besides Windows' own DLLs and the `api-ms-win-*` API
    sets, exactly `MSVCP140.dll`, `VCOMP140.DLL`, `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`
    (`dumpbin /dependents`); `THIRD-PARTY-NOTICES.txt` names the same four. On the owner's machine
    the Build Tools are at `vswhere -latest -products * -property installationPath`
    (Visual Studio 18 Build Tools), the redistributable DLLs under
    `VC\Redist\MSVC\14.51.36231\x64\Microsoft.VC145.CRT` and `...\Microsoft.VC145.OpenMP`, and
    `dumpbin.exe` under `VC\Tools\MSVC\<newest>\bin\Hostx64\x64`. The old script
    (`git show d00647b~1:tools/package-dist.ps1`) found them the same way; it may be read as a
    starting point.
  - ExifTool is looked for as `exiftool.exe` in the running program's folder, before PATH
    (`crates/pw-metadata/src/exiftool.rs`, `exiftool_program`, T120); its `exiftool_files` folder
    must be next to it. CI (`.github/workflows/ci.yml`, step "Install ExifTool") downloads
    `exiftool-13.59_64.zip` from SourceForge, checks SHA-256
    `44b512b25af500724ba579d0a53c8fc5851628b692dd5e5d94ae4a15c2cba9ec`, and renames
    `exiftool(-k).exe` to `exiftool.exe`.
  - The About window (T225, `shell_text.rs`) says the licence is "the file LICENSE in the program
    folder" and the notices are in `THIRD-PARTY-NOTICES.txt` there: the file names in the zip are
    exactly `LICENSE` and `THIRD-PARTY-NOTICES.txt`.
  - RawStride writes per user only under `%LOCALAPPDATA%\RawStride\`: `settings.txt`,
    `field-values.txt` and the write journal `journal\` (`settings.rs`, `field_values.rs`,
    `main.rs`).
  - `/target/` is in `.gitignore`; `/dist/` no longer is (T208). The main worktree still has an
    untracked `dist\` folder from the Tauri app; it is not touched.
- **Crates:** none (no Rust code changes). **Files:** `tools/package-dist.ps1` (new, points 1–6),
  `docs/distribution.md` (rewritten, point 7), `tools/README.md` (point 8), `docs/README.md`
  (point 8), `docs/adr/0002-licensing.md` (point 8), and this task's own file (status and log).
- **What:**
  1. **`tools/package-dist.ps1`** (Windows PowerShell 5.1, run from the repository root; a
     comment header naming ADR 0031 RL4, T228, what it makes and how to run it;
     `$ErrorActionPreference = 'Stop'`, `$LASTEXITCODE` checked after every native program). The
     file holds only ASCII (Windows PowerShell reads a `.ps1` without a BOM as ANSI). Every native
     program (`cargo`, `curl.exe`, `dumpbin.exe`, `exiftool.exe`) is run through one helper that
     sets `$ErrorActionPreference = 'Continue'` around the call and restores it after (as
     `Invoke-Cargo` in `tools/third-party-notices.ps1` does): in Windows PowerShell 5.1 a line on
     a native program's stderr becomes a terminating error under `Stop` when output is redirected
     (an agent's shell, the Actions runner), and `cargo build` writes its progress there. Temporary
     folders are removed in a `finally`. Parameters:
     - `-NoBuild`: use the existing `target\release\rawstride.exe` instead of building (RL6's
       workflow builds it itself with path remapping, just before); stop with a clear message if
       it is missing, and print its last-write time so a stale exe is seen;
     - `-ExifToolZip <path>`: use this local copy of `exiftool-13.59_64.zip` instead of
       downloading it (it is still checked by SHA-256).

     Constants at the top: `$exifToolVersion = '13.59'` and `$exifToolSha256` (the value above),
     with a comment that both must match `EXIFTOOL_VERSION` / `EXIFTOOL_SHA256` in
     `.github/workflows/ci.yml` and the ExifTool version in `tools/third-party-notices.ps1`; the
     four DLL names (point 3).
  2. **Build and inputs:** the version and the repository URL are `pw-loupe`'s from
     `cargo metadata --format-version 1 --no-deps --locked --offline` (`--no-deps`: only the
     workspace's own packages are needed, and on a clean runner a full `--offline` lookup can fail
     on packages for other platforms that were never downloaded). Unless `-NoBuild`, run
     `cargo build --release --locked -p pw-loupe`; building only `pw-loupe` is right here (it is
     the program's crate, as in `docs/building.md`'s release command), unlike the workspace-wide
     check of `AGENTS.md` §5. Stop with a
     clear message if `LICENSE` or `THIRD-PARTY-NOTICES.txt` is missing in the repository root
     (whether the notices are up to date is RL6's check, not this task's). Everything the script
     writes goes under `target\dist\` and nowhere else: the folder
     `target\dist\RawStride-<version>-windows-x64\` and the zip
     `target\dist\RawStride-<version>-windows-x64.zip`, both removed first if they exist, plus
     the download cache and a temporary unpack folder (point 4) under `target\dist\`.
  3. **The Microsoft DLLs:** find Visual Studio or the Build Tools with
     `%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe -latest -products *
     -property installationPath`; `dumpbin.exe` under the newest `VC\Tools\MSVC\<version>`
     folder's `bin\Hostx64\x64`; the redistributable folder as the
     highest `VC\Redist\MSVC\<version>` (names compared as `[version]`; names that are not a
     version, such as `v145`, skipped) that has an `x64` subfolder, and under that `x64` folder
     the subfolders whose names end in `.CRT` or `.OpenMP` (`Microsoft.VC145.CRT` here,
     `Microsoft.VC143.CRT` on a runner with Visual Studio 2022). Walk the imports (`dumpbin /nologo /dependents`) of `rawstride.exe` and
     of every DLL copied, as the old script did: a DLL is Windows' own when its name starts with
     `api-ms-win-` or `ext-ms-`, or when it is in `%WINDIR%\System32` and its name does not start
     with `vcruntime`, `msvcp`, `vcomp`, `concrt` or `vccorlib`; every other import is copied from
     the redistributable folders, keeping the file's own name there (lower case, `msvcp140.dll`;
     Windows does not care). `Get-Imports` checks `dumpbin`'s exit code. Stop if an import is not
     found there. Stop also if the set copied is not exactly the four names `MSVCP140.dll`,
     `VCOMP140.DLL`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` (every name comparison in the script
     ignores case), with a message that `THIRD-PARTY-NOTICES.txt`'s Microsoft paragraph and this
     script's list must then be updated.
  4. **ExifTool:** with `-ExifToolZip`, that file; otherwise
     `target\dist\download\exiftool-13.59_64.zip`, downloaded only if it is not there already,
     with `curl.exe -L --fail --silent --show-error --retry 3 -o <file>.part <url>` from CI's URL
     (`https://sourceforge.net/projects/exiftool/files/exiftool-13.59_64.zip/download`) and
     renamed to the zip's name only when `curl.exe` succeeded; a `.part` file left from a broken
     download is deleted first. The task agent may run this download (the same file and source as
     CI; no tool is installed). Check its SHA-256 (compared without regard to case) and stop
     on a mismatch, naming both values (a cached file with the wrong hash is deleted, so the next
     run downloads it again). Unpack it with `Expand-Archive` into a temporary folder under
     `target\dist\` (removed afterwards, also when the script stops), and copy only
     `exiftool(-k).exe` (as `exiftool.exe`) and the whole `exiftool_files` folder from its
     `exiftool-13.59_64` folder into the program folder, unchanged; stop if either is missing.
     Then run the copied `exiftool.exe -ver` and stop unless its first output line, trimmed, is
     `13.59`.
  5. **`README.txt`** (written by the script with `[System.IO.File]::WriteAllText` and
     `UTF8Encoding($false)`: UTF-8 without BOM, ASCII only so the script stays ASCII, and CRLF line
     endings, since it is read in Notepad on the user's PC and is not stored in the repository).
     The text is a **single-quoted** here-string (so `%LOCALAPPDATA%` and `$` stay as they are),
     its line breaks made CRLF explicitly (first every CRLF to LF, then every LF to CRLF; the
     `.ps1` itself may be checked out with either), with `<version>` and `<repository>` replaced
     afterwards (`repository` from the
     same `cargo metadata`, `https://github.com/Bokehlicious/RawStride` today):
     ```
     RawStride <version>

     A photo browser and metadata editor for Windows 10 and 11 (x64).

     Installing: unpack this folder anywhere you like and start rawstride.exe. Nothing else
     is needed; ExifTool and the Microsoft Visual C++ runtime files are included.

     The program is not signed yet, so Windows SmartScreen may warn the first time it is
     started ("Windows protected your PC": choose "More info", then "Run anyway"). Compare
     the zip's SHA-256 checksum with the one on the release page first.

     Settings are kept in %LOCALAPPDATA%\RawStride. To remove RawStride, delete this folder
     and that one. The program writes nothing into its own folder.

     RawStride is free software under the GNU General Public License, version 3 or later;
     see LICENSE. It comes with no warranty. The notices of the software it includes are in
     THIRD-PARTY-NOTICES.txt. Source code, releases and the change log:
     <repository>
     ```
  6. **Check and zip:** before zipping, stop unless the program folder's top level is exactly
     `rawstride.exe`, the four DLLs, `exiftool.exe`, the folder `exiftool_files`, `LICENSE`,
     `THIRD-PARTY-NOTICES.txt` and `README.txt`. Make the zip with
     `System.IO.Compression.ZipArchive` (`Add-Type -AssemblyName System.IO.Compression,
     System.IO.Compression.FileSystem`), one entry per file, every entry name starting with
     `RawStride-<version>-windows-x64/` and using `/` only (Windows PowerShell's
     `Compress-Archive` and `ZipFile.CreateFromDirectory` may write `\`), compression `Optimal`.
     No directory entries are written, so an empty folder would be lost: stop before zipping if
     the program folder holds an empty folder. Then open the zip again and stop unless it has
     exactly as many entries as the folder has files and no entry name holds `\`. Print the folder, the zip's path, its size and its
     SHA-256 (lower case). (`SHA256SUMS.txt` and the release are RL6's.)
  7. **`docs/distribution.md`** rewritten for RawStride (RL-Q13), in English like the other
     docs, with these sections:
     - **The zip:** its name and a table of what it holds and where each file comes from
       (as point 6's list); the four DLLs app-local, as Microsoft permits; ExifTool 13.59
       unmodified, found by RawStride in its own folder before PATH (T120).
     - **Making it:** `.\tools\package-dist.ps1` (and `-NoBuild`, `-ExifToolZip`), what it needs
       (the build tools of `docs/building.md`, `THIRD-PARTY-NOTICES.txt` up to date, the network
       for the first ExifTool download), where it writes (`target\dist\`), and that **a zip built
       on a developer's machine is for testing only**: its exe holds the build machine's source
       paths in panic messages (the release profile has `debug = "line-tables-only"` and no path
       remapping, kept for latency measurements, RL-Q12); a release is built by the release
       workflow (RL6) with the paths remapped.
     - **What the PC needs:** Windows 10 or 11 x64, nothing else (no Visual C++
       Redistributable, no ExifTool, no WebView2). Mark as **[unverified until RL5]** that it runs
       on a PC that never had Visual Studio, and that it needs no more than Windows' own OpenGL
       (eframe's Glow renderer).
     - **Installing and removing:** unpack and start; delete the folder and
       `%LOCALAPPDATA%\RawStride` (`settings.txt`, `field-values.txt`, `journal\`).
     - **What it writes, and where:** the program folder nothing **[unverified until RL5]**, so
       it may be read-only; per user `%LOCALAPPDATA%\RawStride\`; next to the images only what the
       user asks for (XMP sidecars, embedded metadata, copies on import, deleting to the Recycle
       Bin).
     - **Not signed:** SmartScreen warns on first start; the release page gives SHA-256
       checksums; signing after `0.1.0` (RL-Q6, RL8).
     - **History (the Tauri app, 2026-11-04):** a short section keeping what still holds or was
       learnt: the four MSVC DLLs were loaded from the program folder even with the
       Redistributable installed; `VCOMP140.DLL` is always loaded since the exe imports it; the
       virus scanner delayed the first start of a new exe by 4–12 s; the WebView2 runtime and its
       profile folder are no longer needed. The rest of the old file (its tables, ExifTool on
       PATH, the open points now done by ADR 0002 §2.5, T227 and this task) is left out.
  8. **Other docs:** `tools/README.md` gets a line for `package-dist.ps1` (what it makes, where,
     and that a release zip is made by the release workflow, not by hand). `docs/README.md`'s
     line for `distribution.md` reads "the portable zip: contents, how it is made, what a PC
     needs". `docs/adr/0002-licensing.md` §4: the first item ("A RawStride distribution folder")
     checked `[x]` with "Done by T228 (2026-11-20): `tools/package-dist.ps1` makes the portable
     zip (ADR 0031 RL4); `docs/distribution.md` describes it."
- **Tests to add:** none in Rust. The script checks its own result (points 3, 4 and 6). The log
  shows: the script's output from a full run (with the build), the zip's entry list
  (top level and the number of entries under `exiftool_files/`), and a second run with
  `-NoBuild` that uses the cached ExifTool download (no new download).
- **Not in this task:** the release workflow, `--remap-path-prefix`, `SHA256SUMS.txt`,
  `CHANGELOG.md` (RL6); running the zip on a clean PC (RL5, the owner); the MSI (RL9); signing;
  any Rust code, `Cargo.toml`, `.gitignore`, `.github/`, `tools/check.ps1`,
  `tools/third-party-notices.ps1`, `THIRD-PARTY-NOTICES.txt`; the untracked `dist\` folder in the
  main worktree. Other documents that call `distribution.md` the Tauri folder or say ExifTool is
  not shipped (ADR 0002 §2.5's "today it is not shipped", ADR 0031's Related list,
  `GUI_REQUIREMENTS.md`'s Distribution row, `backlog.md`'s macOS distribution item) are updated
  by the reviewer at Done; ADR 0009's line is history and stays.
- **Rules:** `docs/tasks/README.md` rules 1–12 (rule 13 does not apply: no text in the program).
  No path into a user folder and no e-mail address in any file (the personal-data step of
  `check.ps1`).
- **Hand check:** none of its own; the owner's check of the zip is RL5 (in the plan).
- **Done when:** `.\tools\check.ps1` is all OK, the script has made the zip as in the log, and
  `git status` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Written by the reviewer from ADR 0031 RL4 / §3.4, RL-Q4 and RL-Q13; the exe's DLL imports, the Build Tools' folders, CI's ExifTool step, the About window's file names and the per-user folder checked by the reviewer. Cold read (fresh agent, 2026-11-20): can be done with the named files; the reviewer's facts confirmed (also the release exe's four DLL imports). Fixed in the text: `cargo metadata --no-deps` (a clean runner); native programs run with `Continue` around them (stderr under `Stop` in PowerShell 5.1); the redist folder by `[version]` and its subfolders under `x64`; the DLLs keep their lower-case names, every comparison ignores case; `-p pw-loupe` stated as right here; `-NoBuild` prints the exe's time; the download through a `.part` file and allowed for the agent; `exiftool -ver`'s first line trimmed; `README.txt` from a single-quoted here-string with explicit CRLF; no empty folders in the zip; `finally` cleanup; the stale lines in other documents left to the reviewer at Done |
