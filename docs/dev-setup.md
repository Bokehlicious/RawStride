# Developer setup (the maintainer's machine)

To build RawStride on your own machine, follow [building.md](building.md). This file records the
maintainer's reference machine "thanos" (verified 2026-11-03): exact tool versions, local paths and
settings, and the setup of the Git worktrees.

## Toolchain

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.99.1, MSVC | pinned in `rust-toolchain.toml` |
| VS Build Tools 2026 | 18.11 | C++ workload, Windows SDK 10.0.26100 |
| LLVM/Clang | 23.1.4 | `LIBCLANG_PATH=C:\Program Files\LLVM\bin` (bindgen) |
| CMake / Ninja | 4.4.5 / 1.13.2 | |
| vcpkg | 2026-10-29 | `VCPKG_ROOT=C:\Development\vcpkg` |
| ExifTool | 13.64 | on PATH |
| Tauri CLI | 2.12.4 | `cargo install tauri-cli --version "^2" --locked` (current Tauri app only) |
| Node.js | 26.13 | frontend tooling and UI tests of the current Tauri app |
| Git | 2.56 | `core.longpaths=true` |

The Tauri CLI and Node.js are only needed for the current Tauri app. The planned GUI (eframe + egui,
`docs/GUI_REQUIREMENTS.md`) is built with Cargo alone.

## System settings

- `LongPathsEnabled = 1`
- PowerShell execution policy (CurrentUser): `RemoteSigned`
- Antivirus exclusions for `target\`, `vcpkg\buildtrees\` and `%USERPROFILE%\.cargo\registry`
- GPU driver: Intel Arc 32.0.101.9150 or newer

## Git worktrees

One repository, two working folders (`git worktree`):

| Folder | Branch | Use |
|---|---|---|
| `C:\Development\PhotoWorkflow` | `main` | the main checkout; not edited |
| `C:\Development\PhotoWorkflow-GUI` | `feature/gui-workspace` | all work: tasks, review, commits |

Why the work does not happen directly in the main checkout:

1. **`main` holds only reviewed and approved work.** Task agents leave their changes uncommitted,
   and a commit on `feature/gui-workspace` comes only after the owner's approval and an all-OK
   `.\tools\check.ps1` (`AGENTS.md` §2). Unfinished work, work under review and work awaiting the
   owner's hand check therefore never touch `main`; the owner moves `main` forward after review.
2. **Two folders instead of switching branches in one.** No `git checkout` back and forth, so
   uncommitted changes cannot follow along to the wrong branch, and `target\` is not rebuilt on
   every switch. The approved state stays untouched in its own folder and can be built and run
   next to the development version.
3. **The main checkout holds the shared native libraries.** `VCPKG_INSTALLED_DIR` points the
   worktree at the main checkout's `vcpkg_installed` (see "Running the tests"), so the vcpkg
   libraries are installed and built once.
4. **It matches the pull request flow.** An approved pull request is merged into
   `feature/gui-workspace`, checked there, and reaches `main` from it (`CONTRIBUTING.md`).

## Native dependencies

LibRaw is built by vcpkg in **manifest mode** from `vcpkg.json`.
The triplet is `x64-windows-static-md`; vcpkg does not read `.cargo/config.toml`, so it is given
on the command line (`.cargo/config.toml` sets it only for the Cargo build).
The vcpkg baseline is pinned in `vcpkg.json` (`builtin-baseline`).

```powershell
vcpkg install --triplet x64-windows-static-md   # run in the repo root; output goes to .\vcpkg_installed
```

## Running the tests

Each Git worktree has its own `vcpkg_installed` directory; it is not shared with the main
checkout. To reuse the main checkout's installed libraries, point the worktree at that directory.
For this setup, the main checkout is `C:\Development\PhotoWorkflow`:

```powershell
$env:VCPKG_INSTALLED_DIR = 'C:\Development\PhotoWorkflow\vcpkg_installed'
[Environment]::SetEnvironmentVariable('VCPKG_INSTALLED_DIR', 'C:\Development\PhotoWorkflow\vcpkg_installed', 'User')
```

The first command applies to the current PowerShell window; the second sets a permanent user
variable. Adjust the path if the main checkout is elsewhere.

The `pw-metadata` integration tests require ExifTool on `PATH`. Alternatively, set
`PW_EXIFTOOL_PL` to the ExifTool Perl script path; the tests run it through `perl`. If neither
option works, those tests intentionally fail rather than silently passing.

Tests that need real camera files are ignored by default and read the files from `PW_TESTDATA`
(see `docs/test-inventory.md`). Set it to the external test-data folder, then run the checks with
the ignored tests enabled:

```powershell
$env:PW_TESTDATA = 'D:\PhotoWorkflowData\RAW'
.\tools\check.ps1 -Ignored
```

The `-Ignored` switch passes `-- --ignored` to the workspace test run. To run the regular formatting,
test and clippy checks without the ignored real-file tests, use:

```powershell
.\tools\check.ps1
```

## Quick check

```powershell
cargo check --workspace
cargo clippy --workspace
cargo fmt --all --check
```

## Reverse-engineering tools (own builds only)

REA 6.8.0 is installed as an MCP server, with Ghidra 12.1.4 and JDK 21.
The setup details, including the Claude desktop config location, are in the project notes.
See [rea-workflow.md](rea-workflow.md) for how it is used and [clean-room.md](clean-room.md) for what it may be used on.
