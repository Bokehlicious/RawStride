# T67. ADR 0015 D6–D7: documentation after milestone 6 — Done

- **Review 2026-11-11: accepted.** Waiting for the owner's commit.

- **Can start now.** T63–T66 and the owner's acceptance (D5, 2026-11-11) are done. Documentation
  only; no code, no `Cargo.toml`. For a Claude agent.
- **Files:** `docs/adr/0015-egui-tiff-viewing.md`, `docs/adr/0004-ratings-and-labels.md`,
  `docs/adr/0009-egui-loupe-migration.md`, `docs/adr/0007-metadata-iptc.md`, `docs/architecture.md`,
  `docs/GUI_REQUIREMENTS.md`, `docs/README.md`, the root `README.md`, `docs/test-inventory.md` (only
  if it does not match the latest `test-output.txt`). This file: rule 11.
- **Source of truth:** the log rows of T63–T67 and D5 in this file, and ADR 0015 §2, §7, §9 and §9.1
  as they are now; write only what they record. Do not change ADR 0015 §7 or §9.1, which the reviewer
  wrote from the log.
- **What:**
  1. **ADR 0015:** after the status line, an "Implemented (2026-11-11)" line as in ADR 0014: steps
     D1–D4 done (agent tasks T63–T66), D5 accepted by the owner, D6–D7 this documentation update.
     §9: D6 and D7 "**Done:** T67". §12 (Consequences): keep the text; add one point that colour
     management (ICC) is not applied and is an open owner question before the switch (§9.1).
  2. **ADR 0004** addendum "viewing TIFF": a note "2026-11-11: implemented in the egui program (ADR
     0015)" with the differences: the decoder is the shared crate `pw-tiff`; RawStride keeps the
     decoded image instead of JPEGs at three sizes; one decode at a time in the whole process; a
     memory limit (the core budget in the loupe, 4 GiB for thumbnails); a TIFF is shown as itself in
     both display modes; thumbnails are decoded from the image data, embedded previews not used.
  3. **ADR 0009:** §3.1 table row "Open a folder": replace "except TIFF (later)" with "TIFF since
     ADR 0015"; §3.2: the item "TIFF viewing." gets "(done: ADR 0015, 2026-11-11)". Nothing else.
  4. **ADR 0007:** one new line in the status list, after the first: "Planned as milestone 7 of the
     egui program after its approval (owner, 2026-11-10; ADR 0015 §2)." Nothing else.
  5. **architecture.md**, crate table only (the diagram stays):
     - a new row `pw-tiff` after `pw-thumbcache`: TIFF decoding to 8-bit RGB (`tiff` 0.9.1), one
       decode at a time in the process, an optional memory limit, and the box filter `fit` /
       `downscale`; shared by RawStride and the Tauri app (ADR 0015);
     - `pw-thumbcache`: "TIFF files are decoded from their image data with `pw-tiff` (ADR 0015)";
     - `pw-viewcore`: "a TIFF is opened whole with `pw-tiff` and shown as itself in both modes (ADR
       0015)";
     - `pw-loupe`: "and TIFF files (ADR 0015)" where it lists what the program has.
  6. **GUI_REQUIREMENTS.md** GR8: the status becomes "Status 2026-11-11: ratings and labels (ADR
     0013), filter and multi-selection (ADR 0014) and TIFF viewing (ADR 0015) are done in the egui
     program. IPTC metadata: the Tauri app's only IPTC feature, the import template, is in the egui
     program (ADR 0010, T39); full IPTC handling (ADR 0007) is milestone 7 and not a parity item. No
     feature of the Tauri app is left; the switch waits for the acceptance tests. Open owner question
     before the switch: colour management (ADR 0015 §9.1)." Keep the rest of GR8 as it is.
  7. **Root `README.md`** status: the program also has TIFF viewing; replace "until the egui program
     covers the rest of its features (IPTC metadata, TIFF viewing; GUI_REQUIREMENTS GR8)" with "until
     the switch (GUI_REQUIREMENTS GR8)".
  8. **docs/README.md:** "0015 milestone 6: TIFF viewing in egui – done 2026-11-11" (instead of
     "accepted 2026-11-10"); 0007 stays "proposed".
- **Rules:** English, the style of the existing documents (short sentences, no marketing words);
  change only the lines needed; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
