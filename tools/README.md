# tools

Developer scripts (PowerShell) and benchmarks: preview extraction speed,
ingest throughput per drive, and catalog query timings.
`iptc-table.ps1 -Json <TechReference JSON>` regenerates `crates/pw-metadata/src/iptc_table.rs` (ADR 0007 §3).
`third-party-notices.ps1` makes `THIRD-PARTY-NOTICES.txt` in the repository root (ADR 0031 RL3) from
the vcpkg packages' copyright files, egui's font licences and cargo-about's output (`about.toml`,
`third-party-notices.hbs`); it needs cargo-about 0.9.3 and the vcpkg packages, and is run again, and
its result committed, when `Cargo.lock`, `vcpkg.json` or ExifTool's version changes.
