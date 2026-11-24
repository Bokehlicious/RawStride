# tools

Developer scripts (PowerShell) and benchmarks: preview extraction speed,
ingest throughput per drive, and catalog query timings.
`iptc-table.ps1 -Json <TechReference JSON>` regenerates `crates/pw-metadata/src/iptc_table.rs` (ADR 0007 §3).
`third-party-notices.ps1` makes `THIRD-PARTY-NOTICES.txt` in the repository root (ADR 0031 RL3) from
the vcpkg packages' copyright files, egui's font licences and cargo-about's output (`about.toml`,
`third-party-notices.hbs`); it needs cargo-about 0.9.3 and the vcpkg packages, and is run again, and
its result committed, when `Cargo.lock`, `vcpkg.json` or ExifTool's version changes.
`package-dist.ps1` makes the portable zip `RawStride-<version>-windows-x64.zip` and its folder in
`target\dist\` (ADR 0031 RL4; `docs/distribution.md`): the release exe, the four MSVC runtime DLLs,
ExifTool, `LICENSE`, `THIRD-PARTY-NOTICES.txt` and `README.txt`. A zip made by hand is for testing;
a release zip is made by the release workflow, not by hand.
`package-msi.ps1` makes the installer `RawStride-<version>-windows-x64.msi` in `target\dist\` (ADR 0031
RL9) from the program folder `package-dist.ps1` makes (run that first) and `msi\RawStride.wxs`: per
user, into `%LOCALAPPDATA%\Programs\RawStride`, without `README.txt`. It needs WiX 7.0.1
(`dotnet tool install --global wix --version 7.0.1`), validates the MSI and compares an
administrative image of it with the folder; it installs nothing. `-Version <x.y.z>` gives the same
files another version, for the upgrade test only. A release MSI is made by the release workflow,
not by hand.
`release-build.ps1 -Section <name>` builds a release (ADR 0031 RL6; `docs/distribution.md`): it
checks that `THIRD-PARTY-NOTICES.txt` is up to date, builds the release exe with the build machine's
paths remapped and checks it, runs `package-dist.ps1 -NoBuild` and `package-msi.ps1` (the zip and
the MSI), and writes `SHA256SUMS.txt` (both files) and `release-notes.md` (from `CHANGELOG.md`'s
section `<name>`) in `target\dist\`. The release workflow
`.github/workflows/release.yml` runs it on a `v*` tag; a local run rebuilds `target\release\`.
