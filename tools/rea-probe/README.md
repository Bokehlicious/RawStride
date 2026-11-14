# rea-probe

A small binary with known source code. It is used to check what REA and Ghidra
report on Windows (functions, strings, imports, decompiled loops) before we rely
on them for real RawStride binaries. See `docs/rea-workflow.md`.

```powershell
cargo build -p rea-probe --profile profiling
.\target\profiling\rea-probe.exe
```

Expected output: `PW_REA_PROBE_MARKER_v1 64x48 blur-hash=<16 hex digits>`
