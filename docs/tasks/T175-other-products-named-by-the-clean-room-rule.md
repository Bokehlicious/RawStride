# T175. Other products named only as the clean-room rule allows — Open

- **Why:** `docs/clean-room.md` keeps observations about other products out of the repository, but
  a few current documents describe another program's interface, keys or way of working as the
  model for a RawStride feature. The owner decided on 2026-11-17 when another product may be named
  (`docs/clean-room.md`, "Naming other products"; `AGENTS.md` §4): for interoperability with its
  files, public documentation as a reference (also its documented technical design), where a test
  file came from, and the policy itself. This task rewords the passages that fall outside that, in
  the current documents. Nothing in the code needs to change (its mentions are interoperability or
  test-file sources), and `docs/tasks/done/` is an archive that is not changed.
- **Crates:** none. **Files:** `docs/adr/0004-ratings-and-labels.md`,
  `docs/adr/0005-ingest-engine.md`, `docs/adr/0013-egui-ratings-and-labels.md`,
  `docs/adr/0017-interface-shell.md`, `docs/analyses/2026-11-04-iptc-metadata-requirements.md`; and
  this task's own file (status and log, `docs/tasks/README.md` rule 11).
- **What:** reword only the passages below. **No decision, rule, requirement, date, reference
  or Q/U number changes**; only how the passage is said. Keep the surrounding format (lists,
  tables, line width of 100).
  1. **ADR 0004, "Context"** (lines 9–17): the opening sentence presents "how Photo Mechanic
     handles it". Rewrite the section as the interoperability background it is: XMP sidecars for
     RAW, DNG written in the file and the colour class synchronised with `xmp:Label` stay, named
     as what other programs' files hold, with Camera Bits' public documentation as the source
     (level D); its two short quotes on storage and on the label may stay as quotes. **Remove** the
     bullet "Windows shortcuts" (another program's keys; RawStride's keys are decided in ADR 0004's
     own Decision 4, repeated in ADR 0013 §2, and need no such background). Decision 3's sentence
     on Lightroom and Photo Mechanic writing DNG metadata into the file is storage
     interoperability and stays.
  2. **ADR 0004, "Addendum: filter and multi-selection"** (about line 104): "An image you rate so
     that it no longer matches stays visible until you move on, as in Lightroom, so it never
     vanishes under the cursor." becomes the same sentence without ", as in Lightroom".
  3. **ADR 0005, choice 4 of "Decision":** "The syntax is our own, not Photo Mechanic's." becomes
     "The syntax is RawStride's own." Choice 6's sentence about the other program's file on the
     card is interoperability (it is never read) and stays.
  4. **ADR 0013 §13** (lines 271–279): "the way the owner uses FastRawViewer" and "(a checkbox, as
     in FastRawViewer)" are reworded as the owner's own wishes (for example "X moves the image
     directly into a `_rejected` folder inside the same folder"; "an option in the menus, a check
     box"), and the sentence "FastRawViewer is studied only from its public documentation and by
     using it (clean-room levels C and D, `docs/clean-room.md`)." is removed. The bullet
     "Consequence for this milestone" stays as it is.
  5. **ADR 0013 §11, Q5** (about line 255): "(Lightroom's key)" after "X on a rejected image clears
     it" is removed.
  6. **ADR 0017:** in the status line, "the FastRawViewer additions (§2.6)" becomes "the technical
     assessment additions (§2.6)"; in the line about the owner's wishes of 2026-11-15, ", as
     Photo Mechanic does" after "saved as an XMP template file" is removed; in §7.2's row "Images
     shown while importing", ", as Photo Mechanic does" is removed. §2.6's sentences on the
     upper-case `.XMP` and on the companion files of DxO PhotoLab, RawTherapee and ON1
     (interoperability) stay.
  7. **The analysis of 2026-11-04, §7 "Requirements derived for the design", requirement 6 on
     templates** (lines 229–235; not §10's owner decisions): keep the modes as written and replace
     the comparison with "Photo Mechanic's public template documentation" (lines 232–235) with one
     neutral sentence that the modes are RawStride's own. Its other mentions (the source list,
     the storage practice of Lightroom and Photo Mechanic in §5 and §10, the colour class ↔
     `xmp:Label` option, DNG embedded, Adobe's names) are interoperability with a level-D source
     and stay.
  8. **Check:** in the five files, every remaining mention of any other product or maker
     (Photo Mechanic, Camera Bits, FastRawViewer, Lightroom, Adobe, Capture One, DxO PhotoLab,
     RawTherapee, ON1 or any other) falls under one of the four allowed uses. List any you are
     unsure of in the log instead of changing them.
- **Not in this task:** ADR 0007; ADR 0008 as a whole (§9 and its other FastRawViewer mentions,
  including the reference tests of §9.9 and §9.9a, are documented technical design cited as
  background, allowed use 2); ADR 0016; the glossary, `references.md`, `test-inventory.md` (no
  change expected: no test changes), the plans and the code (their mentions are allowed uses);
  `docs/tasks/done/` (archive); the git history.
- **Rules:** AGENTS.md §4 and §6 (the ADRs are named here, so they may be edited; no decision is
  changed).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-17 | Open | Added from the backlog item "The clean-room policy and the documents disagree" (survey of 2026-11-17); the owner's rule is in `docs/clean-room.md` ("Naming other products") and `AGENTS.md` §4 |
| 2026-11-17 | Open | Cold read: doable with the named files. Changed: two passages it found added (ADR 0004 "as in Lightroom", point 2; ADR 0013 Q5 "(Lightroom's key)", point 5); ADR 0004's keys are its own Decision 4, not ADR 0013's; its quotes and Decision 3 stay; the ADR 0013 §13 sentence to remove quoted; the analysis passage is §7 requirement 6, not a decision; the check covers every product and maker; ADR 0008 as a whole left out as allowed use 2 |
