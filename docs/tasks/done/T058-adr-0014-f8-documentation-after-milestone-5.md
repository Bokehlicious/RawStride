# T58. ADR 0014 F8: documentation after milestone 5 — Done

- **Review 2026-11-10: accepted.** Including amendment 2 in the ADR 0004 note (the task text named
  only amendment 1; adding it is right).

- **Start after T61 and the owner's acceptance (F7).** Documentation only; no code, no `Cargo.toml`. For a
  Claude agent.
- **Files:** `docs/adr/0014-egui-filter-and-multi-selection.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/architecture.md`, `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`,
  `docs/test-inventory.md` (only if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T51–T57, T59–T61 and the owner's F7 notes in this file; write only what
  they record.
- **What:**
  1. **ADR 0014:** "Implemented" status line; §7 results for S1–S6; §9 results per step; a short
     "Findings during implementation" section if the log has any.
  2. **ADR 0004** addendum "filter and multi-selection": a note that the egui program implements it
     (ADR 0014), with the differences: filter per tab, not saved between sessions, "Alla utom
     avvisade", and amendment 1 (ADR 0014 §13): the star condition ≥, =, ≤ (points 1) and a rated
     image that stops matching leaves at once (point 2).
  3. **architecture.md:** `pw-loupe` (filter, selection, batch ratings) and `pw-viewcore`
     (`set_files`).
  4. **GUI_REQUIREMENTS.md** GR8 and the root `README.md`: filter and multi-selection done; left
     before the switch: IPTC metadata, TIFF viewing.
  5. **docs/README.md:** "0014 milestone 5: filter and multi-selection in egui – done {date}".
- **Rules:** English, the style of the existing documents; change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
