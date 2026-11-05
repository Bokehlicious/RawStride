# Clean-room policy

PhotoWorkflow is *inspired by* existing culling tools, including Photo Mechanic.
It must not contain anything derived from their code.

- Policy revised: 2026-11-03 (methods A–E, approved by the project owner)
- Revised 2026-11-04 by the owner: **level B allowed** within the limits below; E unchanged.
  Legal review: Photo Mechanic study report, section H (kept outside the repo).

## Methods for studying Photo Mechanic

| Level | Method | Status |
|---|---|---|
| **A** | **Normal use.** Study of normal behaviour, workflows and performance through hands-on use. Examples: image switching, 100 % zoom, contact sheet, import. Timing with a stopwatch, screen recording or our own test scripts is fine. | Allowed |
| **B** | **Passive OS-level observation of Photo Mechanic while it runs.** Examples: Task Manager / Resource Monitor (CPU, memory, disk, GPU load), WPR/ETW traces of file I/O, threads and GPU queues, Process Monitor (file and registry access), the list of loaded modules, process memory/CPU counters read by our own scripts. **Limits:** watch only. No code injection into PM's process; no modification of the process; no memory dumps; no call stacks or symbol resolution into PM's code; no tools that show its instructions; no GPU frame-capture tools that inject (PIX, RenderDoc). | **Allowed** (owner's decision 2026-11-04; basis: Directive 2009/24/EC art. 5(3), URL 26 g § 4 st., a contract cannot exclude it) |
| **C** | **Results and file formats.** Study of what the program produces: XMP sidecars, IPTC in image files, file naming and copying, import behaviour. | Allowed |
| **D** | **Public material.** Documentation, version history and release notes, support articles, published licence notices. | Allowed |
| **E** | **Analysis of the binaries.** Decompilation, disassembly, debugger attachment, or any other analysis of Photo Mechanic's executables and libraries. This includes static inspection with REA, Ghidra or similar tools. | **Forbidden** |

Background:
- Photo Mechanic's EULA prohibits reverse engineering, decompiling and disassembling, except
  decompiling for interoperability as far as the law permits. It says nothing about monitoring or
  benchmarking.
- EU law (Directive 2009/24/EC, art. 6) permits decompilation only when it is needed for
  interoperability. That exception does not cover studying technical solutions for a
  competing product.

Not used (uncertain or of no benefit; ask the owner first): reading non-executable resources in the
install folder (styles, localisation, built-in templates), reading its plain-text scripts, and
static inspection of its executable headers. Reading the published licence notices (Credits) is
level D.

## Our own software

REA, Ghidra, WPR and other analysis tools may be used freely on:
- PhotoWorkflow's own builds
- open-source dependencies under compatible licences (for example LibRaw)

See [rea-workflow.md](rea-workflow.md).

## Rules for all material

- **No proprietary material in this repository.** That means no code, binaries, assets, icons,
  texts, UI graphics or copied file contents from Photo Mechanic or other products.
- **Observations about other products are kept outside the repo,** in `D:\pm clean room\`.
- **Only neutral, self-formulated requirements enter `docs/`.** Requirements derived from
  observations are rewritten as our own specifications. Example: "the next image must
  appear in under 50 ms".
- **Interoperability work (level C)** describes file formats from standards and from
  observed output files, never from program internals.
- When unsure whether a method is allowed, do not use it. Ask the project owner and
  record the decision here.
