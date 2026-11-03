# Clean-room policy

PhotoWorkflow is *inspired by* existing culling tools, including Photo Mechanic.
It must not contain anything derived from their code.

- Policy revised: 2026-11-03 (methods A–E, approved by the project owner)

## Methods for studying Photo Mechanic

| Level | Method | Status |
|---|---|---|
| **A** | **Normal use.** Study of normal behaviour, workflows and performance through hands-on use. Examples: image switching, 100 % zoom, contact sheet, import. Timing with a stopwatch, screen recording or our own test scripts is fine. | Allowed |
| **B** | **External process or system monitoring of Photo Mechanic.** Examples: WPR/ETW traces, Process Monitor, Process Explorer, GPU capture tools, API monitors. | **Not used** (removed from the policy) |
| **C** | **Results and file formats.** Study of what the program produces: XMP sidecars, IPTC in image files, file naming and copying, import behaviour. | Allowed |
| **D** | **Public material.** Documentation, version history and release notes, support articles, published licence notices. | Allowed |
| **E** | **Analysis of the binaries.** Decompilation, disassembly, debugger attachment, or any other analysis of Photo Mechanic's executables and libraries. This includes static inspection with REA, Ghidra or similar tools. | **Forbidden** |

Background:
- Photo Mechanic's EULA prohibits reverse engineering, decompiling and disassembling.
- EU law (Directive 2009/24/EC, art. 6) permits decompilation only when it is needed for
  interoperability. That exception does not cover studying technical solutions for a
  competing product.

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
