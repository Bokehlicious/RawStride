# 2026-11-03 – tools/rea-probe: REA/Ghidra verification on a known-source binary

- Module: `tools/rea-probe` (test binary, not app code)
- Question: Does REA/Ghidra on this Windows host give correct, useful results on our own MSVC Rust builds?
- Build: `cargo build -p rea-probe --profile profiling` (rustc 1.99.1, MSVC), not yet committed
- Binary: `target\profiling\rea-probe.exe`, 131 584 bytes,
  SHA-256 `f2f3591a53152d604c7e16008c241525a0d73b651e3e5cb07d964e5b681cece8`

## Measured / observed

**Static inspectors (no Ghidra):**

| Observation | Value | Tool / evidence |
|---|---|---|
| Format | PE32+, AMD64 | `inspect_pe_imports` `ev_4426d07d…` |
| Imports | 10 DLLs, 82 functions, 0 delay-load; includes `VCRUNTIME140.dll` and `api-ms-win-crt-*` | same |
| Coverage | complete, no issues | same |
| cargo-auditable section | not present (`.dep-v0` missing) | `inspect_rust_binary` `ev_9921e306…` |
| File entropy | 6.3722; no high-entropy runs; one low run (last 512 bytes) | `inspect_byte_entropy` `ev_f09623cd…` |

**Ghidra 12.1.4 via REA:**

| Observation | Value | Evidence |
|---|---|---|
| Session opened | `provider_id: ghidra`, auto-analysis completed | `open_binary` |
| Image base / segments | `0x140000000`; Headers, .text (91 136 B), .rdata, .data, .pdata, .reloc | `binary_overview` `ev_5f6f7d80…` |
| Procedures / strings | 460 / 270 | same |
| PDB | `rea_probe.pdb` referenced (RSDS, GUID b3f5bb3e…), load state "unknown" | same |
| Name search for `fnv1a64`, `box_blur`, `make_test_image`, `rea_probe` | 0 hits | `search_procedures` `ev_869bf609…` |
| Marker string `PW_REA_PROBE_MARKER_v1` | found at `0x14001a015`; referenced from `0x14001a030` (data) | `search_strings` `ev_01b01ea2…`, `xrefs` `ev_3084baf2…` |
| FNV prime bytes `b3 01 00 00 00 01 00 00` | one match at `0x140006205`, inside `FUN_1400061f0` | `search_bytes` `ev_6e63b514…` |
| `FUN_1400061f0` pseudocode | FNV-1a offset basis `0xcbf29ce484222325`, prime `0x100000001b3`; main loop processes **8 bytes per iteration**, plus a remainder loop for `len & 7` | `procedure_pseudo_code` `ev_89e7f90b…` |
| Callers of `FUN_1400061f0` | `0x140006050` | `procedure_callers` `ev_d2807c86…` |
| Callees of `0x140006050` | `0x140005970`, `0x140005fa0`, `0x1400061f0`, `0x14000f630`, 2 externals | `procedure_callees` `ev_9a579ce8…` |
| `FUN_140005fa0` pseudocode | allocates `0xc00` (3072 = 64×48) bytes; outer loop 48 rows; inner loop writes **4 pixels per iteration**; the `% 256` became plain byte (u8) wraparound arithmetic | `procedure_pseudo_code` `ev_c008226e…` |
| `FUN_140005970` dossier | 355 instructions, 21 basic blocks, 3 callees, pseudocode 26 867 code units | `analyze_function` (summary) `ev_01e34d20…` |

## Interpretation

- **Inference:** `FUN_1400061f0` is `fnv1a64`. The constants and loop structure match the source exactly.
  The 8× unrolling is LLVM's work. The loop cannot be vectorised, because each step depends on the previous hash value.
- **Inference:** `FUN_140005fa0` is `make_test_image`. The allocation size and row count match `WIDTH*HEIGHT` and `HEIGHT`.
- **Inference:** `0x140006050` is `main` (the user `main`, not the CRT entry point). It is the only caller of the hash,
  and it calls both kernels.
- **Inference:** `FUN_140005970` is `box_blur_3x3`, by elimination.
- **Hypothesis:** The blur's size (355 instructions for a ~15-line function) suggests auto-vectorisation and/or
  bounds-check paths. **Not verified.** Confirm with `procedure_assembly` (look for SIMD such as `pmovzxbw`/`paddw`)
  and with a microbenchmark.
- **Inference:** The imports show a **dynamic CRT** (`/MD`). This matches the plan of linking LibRaw with the
  `x64-windows-static-md` triplet.
- **Conclusion on the tooling:** REA/Ghidra correctly recovers the structure of our own MSVC Rust code, including
  compiler transformations. Function names are not available (PDB not loaded), so navigation has to go through
  constants, strings and the call graph.

## Follow-up

- [ ] Paste the probe output from Windows and confirm `blur-hash=c47cbaf7a035c359` (the Linux reference value).
- [ ] Verify `procedure_assembly` on `0x140005970` and record whether the blur is vectorised.
- [ ] Investigate PDB loading (for example whether Ghidra picks the PDB up from the image directory or a symbol path).
- [ ] Verify `compare_functions` / `match_native_functions` between two builds before the first real optimisation.
