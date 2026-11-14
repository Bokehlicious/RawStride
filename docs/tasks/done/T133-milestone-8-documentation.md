# T133. ADR 0007 I13: documentation after milestone 8 (IPTC metadata) — Done

- **Can start now:** I1–I12 are done (the last, I12, by T127 with T129, T130 and T132 on
  2026-11-14). Documentation only; no code and no tests change.
- **Files:** `docs/adr/0007-metadata-iptc.md`; `docs/adr/0004-ratings-and-labels.md` (one note);
  `docs/adr/0005-ingest-engine.md` (one sentence); `docs/adr/0010-egui-import-and-tabs.md` (one line of
  §3.4); `docs/adr/0013-egui-ratings-and-labels.md` (one line); `docs/architecture.md` (the crate
  table only); `docs/GUI_REQUIREMENTS.md` (GR8 only); `docs/README.md` (the ADR list line only).
  Change only the lines needed; rules 10–12. Read `docs/tasks/plans/adr-0007-milestone-8-steps.md`
  and `docs/test-inventory.md` as sources; do not change them.
- **Why:** the last step of milestone 8 (I13 in the step list), as T67 and T74 did for milestones 6
  and 7: ADR 0007 says what was built and what is left, and the documents that point at ADR 0007
  no longer call it planned or future.
- **What:**
  1. **ADR 0007, status.** Under the status bullet list, add one bullet "**Implemented
     (2026-11-14):**" with the steps and their tasks, taken from the step list's table: I1 T76,
     I2 T77, I3 T78, I4 T79, I5 T80–T81, I6 T82, I7 T83, I8 T84–T86, I9 T87–T93, I10 T94–T108,
     I11 T128 (with ADR 0017 U10d), I12 T127 (with T129, T130, T132), I13 T133; the owner's
     decisions D1–D16 in §17. Keep "accepted by the owner" as the status word; change "what is left
     before the implementation is in §16" to point at the new §18 (point 3).
  2. **ADR 0007, §12 tests.** Under the **Tests:** list, add a table "Where each test is" with one
     row per bullet of that list (round trip per format, IIM = XMP after every write, … fault
     injection) and the test files that cover it, **taken from the rows of `docs/test-inventory.md`**
     (for example `crates/pw-metadata/tests/edit.rs`, `src/reconcile.rs`, `tests/level_c.rs`). A
     bullet no row covers gets "not covered" — do not guess; list such bullets in the report.
  3. **ADR 0007, a new §18 "What is left after milestone 8"** before the revision history, as a
     list, each item with where it now lives:
     - `Cleared` (§6.4, Q2): not read or written; waits for D1 (`docs/tasks/backlog.md`);
     - §6.1 Exif ImageEditor ↔ Contributor and GPS → Location Created (backlog; GPS is the next
       metadata ADR, ADR 0017);
     - keyword qualifiers (§8; backlog);
     - from §10: template sub-fields of lists of structures, variables, template files in XMP
       (backlog; ADR 0017 U13);
     - the other controlled vocabularies of §11 (backlog);
     - lists of structures and editing several images (ADR 0017 U11, U12);
     - §16 point 5: XMP Part 1, MWG §5.2, XMP Part 3 and IIM 4.2 not read, so Q3, Q7 and Q8 and
       the Lang Alt row of §13 stay "to verify";
     - the Tauri app keeps its in-place write (D7) until it is retired (GR8).
     Update §16's heading line or first sentence only so that it points to §18 for what is left.
  4. **ADR 0007, revision history:** revision 18, 2026-11-14: "I13 (T133): implemented status,
     where each §12 test is, §18 what is left; no new decision".
  5. **ADR 0004:** at the end of the note "Note (2026-11-04): ADR 0007", one sentence: in
     RawStride, embedded ratings and labels (DNG, JPEG, TIFF) are written through ADR 0007's safe
     write since T87/T91 (2026-11-12); the Tauri app keeps the in-place write (ADR 0007 D7).
  6. **ADR 0005:** after point 4 "IPTC template on ingest", one sentence: in RawStride the template
     has the modes of ADR 0007 §10 (Keep, Replace, Append, Prefix, Clear; T88, T100) and is
     written through the safe write (T91); the Tauri app is unchanged (D7).
  7. **ADR 0010 §3.4:** after the bullet "Writing IPTC through the safe write procedure of
     ADR 0007 §9 …" add "(done: ADR 0007 I9, T91, 2026-11-12)", nothing else.
  8. **ADR 0013:** after the out-of-scope bullet "Keywords, title, description and other IPTC
     editing (ADR 0007)" add "(done: ADR 0007 milestone 8, 2026-11-14; the metadata window of
     ADR 0017)", nothing else.
  9. **architecture.md, crate table:**
     - the `pw-metadata` row: what milestone 8 added, in one or two sentences: the IPTC property
       table, the value model, the reader with IIM/XMP/Exif reconciliation and conflicts and the
       sidecar rules, the write plan, the safe write (snapshot, verification, Windows file
       operations, the write journal), the ingest template modes, edits, validation warnings, the
       PLUS vocabularies, and the ExifTool version check (ADR 0007);
     - a new `pw-imagecheck` row after `pw-metadata`: the real image check of the safe write
       (previews byte for byte with `pw-raw`, the DNG opened with LibRaw; ADR 0007 §9.3, D4);
     - the `pw-viewcore` row: closing the RAW sessions of a file before it is written (T89);
     - the `pw-loupe` row: the metadata worker, journal recovery at start, the ExifTool check at
       start, the settings file and window (D10), and the metadata window (ADR 0007 D16,
       ADR 0017).
     Keep each row's existing text; diagrams unchanged.
  10. **GR8:** in the status sentence, "full IPTC handling (ADR 0007) is milestone 8 and not a
      parity item" becomes: full IPTC handling (ADR 0007) is done in the egui program as
      milestone 8 (2026-11-14) and was not a parity item. Nothing else in GR8.
  11. **docs/README.md:** in the ADR list, "0007 metadata per IPTC (milestone 8) – accepted
      2026-11-11" gets ", done 2026-11-14".
- **Not in this task:** the step list and other files under `docs/tasks/` (the reviewer marks I13
  done there), the root `README.md` (already lists the metadata), ADR 0017, `docs/test-inventory.md`
  and any code.
- **Rules:** English; facts only from the files named above and git history, no new decisions. If a
  sentence above does not fit the file as it is, stop and ask (rule 10).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-14 | Open | Added |
| 2026-11-14 | Ready for review | Points 1–11 done. §12 "not covered": hierarchical keywords kept, and deprecated IIM datasets kept in a written file (only the write plan's unit tests cover them); `.\tools\check.ps1` run |
| 2026-11-14 | Done | Reviewed; reviewer fix: the GPS item of §18 names the next metadata ADR after ADR 0017 (its §7.2). Owner approved. check.ps1 all OK (1325 passed, 53 ignored) |
