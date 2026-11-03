# REA workflow

REA ("Reverse Engineer Anything") is used in PhotoWorkflow as a real analysis tool.
We use it to verify what the compiler actually produced, to find bottlenecks, and to
compare builds before and after an optimisation. It is not a box to tick:
use it when it answers a question better than reading the source or running a profiler.

All use follows [clean-room.md](clean-room.md).

## 1. Environment (verified 2026-11-03)

| Component | Version / location |
|---|---|
| REA (`rea-agents`) | 6.8.0, global npm install, MCP server via the Claude desktop app |
| Ghidra | 12.1.4 official release, `C:\tools\ghidra_12.1.4_PUBLIC` |
| JDK | Temurin 21.0.13 |
| Analysis provider | `REA_ANALYSIS_PROVIDER=ghidra` (set in the MCP registration) |

The MCP registration lives in the Claude desktop app's config file. Note its location:
because the app is an MSIX install, it is under `%LOCALAPPDATA%\Claude-Data\…`
and not under `%APPDATA%\Claude`. Quit the app fully before editing that file.

Windows Ghidra support in REA is **experimental (P0)** and is restricted as follows:
- It accepts only native PE files (x86, x86-64, ARM64): applications and DLLs.
  Managed (.NET), hostile, sensitive or mutable-path targets are not supported.
- It is **read-only**: renaming, comments and bookmarks are rejected.
- The target and the runtimes must be on local NTFS.
- Each session imports the binary into a temporary Ghidra project, which is
  deleted on `close_binary`.

## 2. Available MCP tools

This inventory comes from `binary_session` (catalog: 229 tools) and from actual calls
on 2026-11-03. "Verified" means we called the tool and checked its result against known source.

### Verified working

| Tool | Engine | Used for |
|---|---|---|
| `binary_session` | REA | Server identity, provider availability, tool availability |
| `inspect_pe_imports` | REA (no Ghidra) | DLLs and functions a build imports; CRT linkage; unexpected dependencies |
| `inspect_rust_binary` | REA (no Ghidra) | cargo-auditable dependency list, rustc commit |
| `inspect_byte_entropy` | REA (no Ghidra) | Detecting embedded compressed or encrypted blobs |
| `open_binary` / `close_binary` | Ghidra | Start and end a session (`provider_id: "ghidra"`) |
| `binary_overview` | Ghidra | Segment, procedure and string counts |
| `search_strings` | Ghidra | Locate string literals |
| `search_bytes` | Ghidra | Locate unique constants or byte patterns, then the procedure containing them |
| `search_procedures` | Ghidra | Name search (only useful when symbols are loaded, see §4) |
| `xrefs` | Ghidra | References to an address |
| `procedure_callers` / `procedure_callees` | Ghidra | Call graph navigation |
| `procedure_pseudo_code` | Ghidra | Decompiled view of one function |
| `analyze_function` (summary) | Ghidra | Instruction, block, call and reference counts for one function |

### Listed as available but not yet verified

`analyze_function` (complete detail), `procedure_assembly`, `read_function_instructions`,
`get_call_graph`, `batch_decompile`, `decompile_all`, `list_procedures`, `list_strings`,
`read_bytes`, `compare_functions`, `match_native_functions`, `compare_artifacts`,
`find_changed_behavior`, `inspect_pe_resources`, `inspect_pe_load_config`,
`inspect_pe_certificates`, `inspect_native_exports`, `inspect_with_objdump`,
`inspect_sqlite_database`, the evidence-bundle tools, and the Frida/rizin/gdb tools.
REA lists the Frida, rizin and gdb tools as available, but we have not checked that
the backing tools are installed, and none were installed as part of this setup.

Each tool is moved to the verified table the first time it is used successfully.

### Not available on this host

| Tool(s) | Reason |
|---|---|
| `capture_process_scenario`, `observe_native_calls`, `capture_native_ui_scenario`, `observe_native_ui` | Unsupported host (Windows) |
| `inspect_macho`, `inspect_plist`, `demangle_swift`, `list_architectures`, `inspect_signature` | macOS only |
| Annotation tools (`set_address_name`, `set_comment`, `annotate_native_function`, …) | Windows Ghidra P0 is read-only |
| ADB, Android and JEB tools | Provider not installed (not relevant to this project) |

**Consequence:** REA gives us **static** analysis on Windows. **Runtime** behaviour
(CPU time, waits, I/O, GPU) is measured with WPR/WPA and benchmarks, not with REA.

## 3. Which tool for which question

| Question | Primary tool | Complement |
|---|---|---|
| Where does the time go during culling or ingest? | WPR (CPU sampling, disk I/O) + WPA | REA on the hot functions WPA names |
| Did the compiler vectorise or unroll this kernel? | REA `procedure_pseudo_code` / `analyze_function` | Microbenchmark |
| Did an optimisation change the generated code? | REA on the old and new builds (same function) | Benchmark numbers |
| What does a build link against (CRT, DLLs, unexpected deps)? | `inspect_pe_imports`, `inspect_rust_binary` | `cargo tree` |
| How does LibRaw decode a given ARW variant? | LibRaw **source** first | REA/Ghidra on our statically linked build |
| Is memory or handle use growing? | WPR (heap / handle tracing), Task Manager | REA only for locating the allocation site |
| GPU frame time and upload cost | RenderDoc / PIX / WPR GPU (to be approved) | — |
| Why does our component behave unexpectedly? | Logs, tests, debugger | REA when the source is not enough (for example inlining) |

Rule of thumb: **measure with a profiler, explain with REA.** REA does not
measure time, so it must never be the only source for a performance claim.

## 4. Known limitations (observed)

- **PDB symbols are not loaded.** Ghidra sees the PDB reference in our MSVC builds
  but reports its load state as unknown, and functions show up as `FUN_<address>`.
  A name search therefore returns nothing. REA does not let us point to a PDB file.
  - Workaround: find functions through unique constants (`search_bytes`), string
    literals (`search_strings` → `xrefs`) and the call graph (`procedure_callers`/`callees`).
  - When we analyse a kernel, give it a recognisable marker, such as a unique
    constant or `#[inline(never)]` during the investigation.
- **Large outputs.** `binary_session` and complete-detail results can exceed 70 kB.
  Prefer `detail: "summary"` and page through the views instead.
- **Rust strings** may not be listed as defined strings, so an empty string search does not prove absence.

## 5. Running a performance comparison

1. **State the question and the metric.** Example: "time from key press to a visible
   preview for A1 ARW, p50/p95". Name the module (`pw-raw`, `pw-render`, …).
2. **Fix the conditions.** Same input set (`D:\PhotoWorkflowData\…`, list the files),
   same build profile (`--profile profiling`: release optimisations plus full debug info),
   AC power, no other heavy load, and caches either cold or warm as stated.
3. **Baseline.** Run the benchmark or scenario at least 5 times and record median, p95 and spread.
   Capture a WPR trace of one representative run.
4. **Explain.** Use WPA to find the hot functions, then REA on those functions in
   the same binary. Record the binary's SHA-256 and the REA evidence IDs.
5. **Change one thing.** Rebuild, then repeat steps 3–4 under identical conditions.
6. **Compare.** Report the numbers side by side. Use REA to confirm that the intended
   code change actually happened (for example a loop is now vectorised).
7. **Record it** in `docs/analyses/` (see below), including failed attempts.

## 6. Documenting analyses

- Each analysis gets a file in `docs/analyses/` named
  `YYYY-MM-DD-<module>-<topic>.md`, based on [analyses/TEMPLATE.md](analyses/TEMPLATE.md).
- **Keep these two separate:**
  - **Measured / observed:** numbers from benchmarks or WPR, and tool outputs
    with their evidence IDs and the binary SHA-256.
  - **Interpretation:** our conclusions, labelled as inference or hypothesis,
    with what would confirm them.
- Link the analysis from the module's crate documentation, or from the PR or commit that acted on it.
- Raw WPR traces (`.etl`) are large and stay outside git, under `D:\PhotoWorkflowData\profiling\`.

## 7. Analysis log

| Date | Module | Topic | File |
|---|---|---|---|
| 2026-11-03 | tools/rea-probe | REA/Ghidra verification on a known-source binary | [analyses/2026-11-03-rea-probe-baseline.md](analyses/2026-11-03-rea-probe-baseline.md) |
| 2026-11-03 | pw-raw | Embedded previews in ARW, RW2, DNG (ExifTool; REA not applicable) | [analyses/2026-11-03-pw-raw-embedded-previews.md](analyses/2026-11-03-pw-raw-embedded-previews.md) |
