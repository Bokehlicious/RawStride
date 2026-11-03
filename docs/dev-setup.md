# Developer setup (Windows 11)

Verified on the reference machine "thanos" on 2026-11-03.

## Toolchain

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.99.1, MSVC | pinned in `rust-toolchain.toml` |
| VS Build Tools 2026 | 18.11 | C++ workload, Windows SDK 10.0.26100 |
| LLVM/Clang | 23.1.4 | `LIBCLANG_PATH=C:\Program Files\LLVM\bin` (bindgen) |
| CMake / Ninja | 4.4.5 / 1.13.2 | |
| vcpkg | 2026-10-29 | `VCPKG_ROOT=C:\Development\vcpkg` |
| ExifTool | 13.59 | on PATH |
| Tauri CLI | 2.12.4 | `cargo install tauri-cli --version "^2" --locked` |
| Node.js | 26.13 | frontend tooling |
| Git | 2.56 | `core.longpaths=true` |

## System settings

- `LongPathsEnabled = 1`
- PowerShell execution policy (CurrentUser): `RemoteSigned`
- Antivirus exclusions for `target\`, `vcpkg\buildtrees\` and `%USERPROFILE%\.cargo\registry`
- GPU driver: Intel Arc 32.0.101.9150 or newer

## Native dependencies

LibRaw is built by vcpkg in **manifest mode** from `vcpkg.json`.
The triplet is `x64-windows-static-md`, set in `.cargo/config.toml`.
The vcpkg baseline is pinned in `vcpkg.json` (`builtin-baseline`).

```powershell
vcpkg install            # run in the repo root; output goes to .\vcpkg_installed
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
