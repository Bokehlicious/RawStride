# Clean-room policy

PhotoWorkflow is *inspired by* existing culling tools, including Photo Mechanic.
It must not contain anything derived from their code.

## Allowed

- Using other products as a normal licensed user and observing their **behaviour**:
  UX, workflows, keyboard shortcuts, timings, and which files and metadata they produce.
- Reading **public documentation**, file-format specifications, and open-source code
  under compatible licences (LibRaw, ExifTool, Exiv2 docs and similar).
- Inspecting **output files** that other products write (for example XMP sidecars),
  so that our files stay interoperable with theirs.
- Using reverse-engineering tools (REA, Ghidra) on **our own builds** and on
  open-source dependencies.

## Not allowed

- Decompiling, disassembling or debugging another vendor's binaries.
- Copying another product's code, assets, icons, texts or UI graphics.
- Storing investigation notes about other products in this repository.

## Where notes live

Behavioural observations are kept **outside the repo**, in `D:\pm clean room\`.
Requirements that come from them are rewritten as our own neutral specifications
(for example "the next image must appear in under 50 ms"), and only those enter `docs/`.
