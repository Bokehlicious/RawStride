# Building RawStride

RawStride builds and runs on **Windows 10 or 11, x64** only for now. This guide assumes nothing
about your machine beyond that.

## 1. Install the tools

| Tool | Why | How |
|---|---|---|
| Visual Studio Build Tools (2022 or newer) | the MSVC compiler and linker, the Windows SDK | the "Desktop development with C++" workload |
| Rust | the compiler | [rustup](https://rustup.rs). The version is pinned in `rust-toolchain.toml`; rustup installs it, with `rustfmt` and `clippy`, the first time you build |
| vcpkg | builds LibRaw and libjpeg-turbo, the native libraries | clone [vcpkg](https://github.com/microsoft/vcpkg), run its `bootstrap-vcpkg.bat`, and put the folder on `PATH` (or set `VCPKG_ROOT`) |
| Git | the source | enable long paths: `git config --global core.longpaths true` |
| ExifTool | reading and writing metadata, at run time and in the tests | the Windows package from [exiftool.org](https://exiftool.org), version 12.67 or newer; put `exiftool.exe` (with its `exiftool_files` folder) on `PATH` |

Windows' long-path support (`LongPathsEnabled`) helps with vcpkg's deep build folders. Some
antivirus programs slow the build down a lot; that is a choice for your own machine, not a
requirement.

The old Tauri app (`src-tauri`, `ui/`) is still in the workspace until the switch to the new
program. `cargo` builds it without Node.js or the Tauri CLI.

## 2. Get the source and the native libraries

```powershell
git clone <repository URL>
cd <repository folder>
vcpkg install
```

`vcpkg install` reads `vcpkg.json` (versions pinned by its baseline) and builds the libraries into
`.\vcpkg_installed`, with the triplet `x64-windows-static-md` set in `.cargo\config.toml`. The first
run takes a while, because LibRaw is built from source.

The build looks for the libraries in `.\vcpkg_installed`. To use them from another folder (for
example a second Git worktree), set `VCPKG_INSTALLED_DIR` to that folder's `vcpkg_installed`.

## 3. Build and run

```powershell
cargo run --release -p pw-loupe
```

`pw-loupe` is the RawStride program (window title "RawStride"). A folder can be given on the command
line: `cargo run --release -p pw-loupe -- "D:\Photos\2026-11-14"`.

## 4. Check your changes

```powershell
.\tools\check.ps1
```

It runs `cargo fmt --all --check`, `cargo test --workspace` and
`cargo clippy --workspace --all-targets`, and writes the whole output to `test-output.txt`. It must
end with every step OK before a change is committed (`CONTRIBUTING.md`). If PowerShell refuses to
run the script, run it with `powershell -ExecutionPolicy Bypass -File .\tools\check.ps1`.

Some tests need more than the source:

- **ExifTool:** the `pw-metadata` tests and a few import tests start ExifTool. Without it they
  fail on purpose instead of passing silently. `PW_EXIFTOOL_PL` can point to ExifTool's Perl
  script (`exiftool` from the Perl package) instead; the tests then start it through `perl`.
- **Real camera files:** tests that need them are ignored by default. They read the files from the
  folder in `PW_TESTDATA`; the repository does not ship such files (`tests/fixtures/README.md`).
  With your own files: `$env:PW_TESTDATA = '<folder>'; .\tools\check.ps1 -Ignored`.

`docs/TESTING.md` explains the tests and `docs/test-inventory.md` lists them.

## 5. When something goes wrong

| Message | Cause |
|---|---|
| `VCPKG_INSTALLED_DIR is not set and neither … exists` | `vcpkg install` has not been run in the repository root |
| a linker error naming `raw_r.lib` or `turbojpeg` | the libraries were built for another triplet or are missing; run `vcpkg install` again |
| `ExifTool is missing` in RawStride, or the metadata tests fail at start | `exiftool.exe` is not on `PATH` |

`docs/dev-setup.md` describes the maintainer's own machine (exact versions and local paths); you do
not need it to build.
