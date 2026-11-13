# T74. ADR 0016 C6: documentation after milestone 7 (colour management) — Done

- **Can start after T73 is done.** Documentation only; no code, no tests change.
- **Files:** `docs/adr/0016-colour-management.md`; `docs/adr/0015-egui-tiff-viewing.md`;
  `docs/adr/0007-metadata-iptc.md` (one status line); `docs/architecture.md`;
  `docs/GUI_REQUIREMENTS.md` (GR8 only); `docs/README.md` (the ADR list line only); the root
  `README.md`. Change only the lines needed; rules 10–12.
- **Why:** ADR 0016 §9 C6, as T67 did for ADR 0015.
- **What:**
  1. **ADR 0016:** after the status, an "Implemented (date)" line: C1–C4 = T68–T72, the status-line
     note = T73, C5 accepted by the owner on 2026-11-11, C6 = T74. §7: a results table under the F1–F6
     table, in the style of ADR 0015 §7, filled **from the log rows of T68–T73 and C5** in
     `docs/agent-tasks.md` (F1: T68 within 1; F2: T70/T71 tests and the real-file results; F3:
     unchanged tests and identity; F4: the fallback tests; F5: the objective patch check within 1 and
     the owner's DxO check; F6: the `latency.ps1` result **if a log row gives one**, otherwise
     "not measured yet; the render timings of the T72 review show the conversion within the noise").
     §9: C6 "**Done:** T74". §12: one point that milestone 7 is complete.
  2. **ADR 0015:** §2 "ADR 0007 becomes milestone 7" and §9 D7 and §12 "milestone 7": add
     "(moved to milestone 8 by ADR 0016 Q1, 2026-11-11)" once each, nothing else; §3.2 and §12 colour
     points: add "(done: ADR 0016, 2026-11-11)".
  3. **ADR 0007:** its "Planned as milestone 7" status line becomes milestone 8 with
     "(ADR 0016 Q1, 2026-11-11)"; status still "proposed".
  4. **architecture.md:** a new `pw-colour` row (ICC profiles and transforms with `moxcms` 0.8.1,
     the transform cache, the JPEG source rule; ADR 0016) after `pw-tiff`; one short addition each in
     the `pw-tiff`, `pw-thumbcache`, `pw-viewcore`, `pw-loupe`, `pw-libjpeg` and `pw-raw` rows (what
     T69–T73 added there). Diagrams unchanged.
  5. **GR8:** the status sentence: the open owner question about colour management is answered —
     done in ADR 0016 (2026-11-11); full IPTC handling (ADR 0007) is now milestone 8.
  6. **docs/README.md:** add "0016 colour management – done 2026-11-11" to the ADR list.
  7. **Root README.md:** list colour management (conversion to the monitor profile) with the other
     done features of RawStride.
- **Done when:** `.\tools\check.ps1` is all OK (no code changed) and `git diff --stat` shows only the
  files above.
