# Clean-room policy

RawStride is *inspired by* existing culling tools, including Photo Mechanic.
It must not contain anything derived from their code.

- Policy revised: 2026-11-03 (methods A–E, approved by the project owner)
- Revised 2026-11-04 by the owner: **level B allowed** within the limits below; E unchanged.
  The legal review behind the policy is kept outside the repository.
- Revised 2026-11-17 by the owner: when another product may be named in the repository (below,
  "Naming other products").

## Methods for studying other programs

| Level | Method | Status |
|---|---|---|
| **A** | **Normal use.** Study of normal behaviour, workflows and performance through hands-on use. Examples: image switching, 100 % zoom, contact sheet, import. Timing with a stopwatch, screen recording or our own test scripts is fine. | Allowed |
| **B** | **Passive observation through the operating system while the program runs.** Examples: Task Manager and Resource Monitor (CPU, memory, disk, GPU load), the files it opens, process counters read by our own scripts. **Limits:** watch only. Nothing is injected into or changed in the program's process; no memory dumps; no call stacks or symbols from its code; no tools that show its instructions; no GPU frame-capture tools that inject. | Allowed (owner's decision 2026-11-04) |
| **C** | **Results and file formats.** Study of what the program produces: XMP sidecars, IPTC in image files, file naming and copying, import behaviour. | Allowed |
| **D** | **Public material.** Documentation, version history and release notes, support articles, published licence notices. | Allowed |
| **E** | **Analysis of the binaries.** Decompilation, disassembly, debugger attachment, or any other analysis of another program's executables and libraries. This includes static inspection with REA, Ghidra or similar tools. | **Forbidden** |

The licence terms of every program studied are followed.

Not used (uncertain or of no benefit; ask the owner first): reading non-executable resources in a
program's install folder (styles, localisation, built-in templates), reading its plain-text
scripts, and static inspection of its executable headers. Reading the published licence notices
(Credits) is level D.

## Our own software

REA, Ghidra, WPR and other analysis tools may be used freely on:
- RawStride's own builds
- open-source dependencies under compatible licences (for example LibRaw)

See [rea-workflow.md](rea-workflow.md).

## Rules for all material

- **No proprietary material in this repository.** That means no code, binaries, assets, icons,
  texts, UI graphics or copied file contents from other products.
- **Observations about other products are kept outside the repository.**
- **Only neutral, self-formulated requirements enter `docs/`.** Requirements derived from
  observations are rewritten as our own specifications. Example: "the next image must
  appear in under 50 ms".
- **Interoperability work (level C)** describes file formats from standards and from
  observed output files, never from program internals.
- When unsure whether a method is allowed, do not use it. Ask the project owner and
  record the decision here.

## Naming other products (owner, 2026-11-17)

Text in this repository (documents, ADRs, tasks, code, comments, tests and commit messages)
describes RawStride in its own words. Another product is named only for:

1. **Interoperability (level C):** files another program writes or reads, and what RawStride does
   with them: names, fields and values seen in its output files, and checks that its files and
   RawStride's work both ways. Example: "a sidecar `<stem>.XMP` in upper case, as Photo Mechanic
   writes it, is read and written under that name."
2. **Public documentation as a reference (level D)** for a file format, a field or a value, or
   for another program's documented technical design (decoding, caching, colour handling) cited
   as background for a RawStride architecture decision; listed in [references.md](references.md)
   or cited where it is used. The decision itself is stated in RawStride's own terms.
3. **Where a test file came from:** the program and version that wrote a level-C sample.
4. **This policy, the agent rules and the trademark notice below.**

Not allowed: another program's behaviour, interface, menus, keys, workflow or speed described as a
model or a reason for a RawStride feature ("as X does", "X's shortcut is …"). A wish inspired by
another program is written as RawStride's own requirement, without the name; the observation
behind it stays outside the repository (above).

Finished tasks in `docs/tasks/done/` are an archive and are kept as they were written; this rule
applies to new text and to the current documents.

## Trademarks

Photo Mechanic, Lightroom, FastRawViewer and other product names named in this repository are
trademarks of their respective owners. RawStride is not affiliated with, endorsed by or sponsored by any of them; their names are used
only to describe compatibility and the background of design decisions.
