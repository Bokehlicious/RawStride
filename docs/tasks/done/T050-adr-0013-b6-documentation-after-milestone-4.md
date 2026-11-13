# T50. ADR 0013 B6: documentation after milestone 4 — Done

- **Documentation only; no code, no `Cargo.toml`.** For a Claude agent.
- **Files:** `docs/adr/0013-egui-ratings-and-labels.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/architecture.md`, `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`,
  `docs/test-inventory.md` (only if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T41–T49 in this file; write only what they record. Do not
  invent measurements or results.
- **What:**
  1. **ADR 0013:**
     - Status: add "**Implemented (2026-11-10):** steps B1–B5 done (agent tasks T41–T49); B6 is this
       documentation update." (as ADR 0011's status line).
     - §9: the Result column of B1–B6 with their tasks (B1 T41; B2 T42, T45; B3 T43, T46; B4 T44, T47;
       B5 the owner's hand checks; B6 T50), and one line under the table that T47–T49 were found in the
       hand checks.
     - §7: a short result per R1–R6 from the log (R1–R5 checked by hand as recorded; R3 with the Swedish
       messages of T49; R4 with the exit wait of T44). **R6 (Lightroom / Photo Mechanic) is not in the
       log:** mark it "not checked yet" — do not mark it passed.
     - A new short section "Findings during implementation": the Common Controls 6 manifest (T47:
       `rfd` features are merged with the Tauri app, `check.ps1` cannot catch a binary that fails to
       load), refused writes (T48: ExifTool's "0 image files updated"), the Swedish reasons (T49:
       `WriteError`), and the label name kept in the loupe badge (owner, T43).
  2. **ADR 0004:** a note "2026-11-10: implemented in the egui program (ADR 0013)". Under "Open", keep
     the clean-room level C check open (it is the same as ADR 0013 R6) and add: "A write that ExifTool
     refuses is now reported (T48); a read-only file gives its own error (T49)."
  3. **architecture.md:** in the crate table, `pw-metadata` also reports refused writes by kind
     (`WriteError`), and `pw-loupe` also has ratings and labels (key map, the rating store with its
     ordered write queue, the exit wait; ADR 0013). Mention `crates/pw-loupe/build.rs` (manifest) in one
     line.
  4. **GUI_REQUIREMENTS.md** GR8 and the root `README.md` status: ratings and labels are done in the
     egui program; still left before the switch: filter and multi-selection, IPTC metadata, TIFF
     viewing. Keep the rest of GR8 as it is.
  5. **docs/README.md:** the ADR list says "0013 milestone 4: ratings and labels in egui – done
     2026-11-10".
- **Rules:** English, the style of the existing documents (short sentences, no marketing words);
  change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK (formatting and tests are unaffected) and
  `git diff --stat` shows only the files above.
