# T75. ADR 0009 S6: acceptance measurement of RawStride and a results report — Done

- **Review 2026-11-11: accepted.** Waiting for the owner's commit.

- **Starts after the owner's runs (S6a below).** The report itself is documentation only; no code, no
  `Cargo.toml`. For a Claude agent.
- **Why:** ADR 0009 §9 step S6 was never done. There is no log row, no `D:\PhotoWorkflowData\analysis\s6`
  and no report; ADR 0009's status still ends at S0. GR8 says the switch from the Tauri app waits for
  the acceptance tests, and S6 is those tests. Since S5 the program has gained the contact sheet,
  tabs, import, ratings, filter, TIFF and colour management (ADR 0010–0016), so S6 measures
  RawStride as it is now, not the milestone 1 loupe alone. The earlier runs in
  `latency-rawstride.csv` (`t57-…`, `t66-…`, `c4-…`) cover next/previous only.
- **S7** (TESTING.md: the loupe's tests) is a separate task after this one.

**S6a – the owner's runs** (not an agent task). The method is ADR 0009 §8.2, with these changes:

- the window title is **"RawStride"** (ADR 0012), not "PhotoWorkflow Loupe";
- RawStride is started **with a file path**, which opens the loupe; a folder opens the grid (F6 log
  row, 2026-11-11);
- a release build of the committed HEAD; the window maximised on the main screen; default budgets;
- `latency.ps1 -Title RawStride -Region 0.12,0.1,0.5,0.5 -Out
  D:\PhotoWorkflowData\pm-study\results\latency-rawstride.csv`, with `-Proc` set to RawStride's
  process name (check it in Task Manager) so the memory columns are filled;
- every label starts with `s6-`; screenshots and notes go to `D:\PhotoWorkflowData\analysis\s6\`;
- each run gets one row in the Log ("S6a"), as for F6; an invalid run is marked invalid with the
  reason, and repeated.

| # | Check | Requirement | Run | Label |
|---|---|---|---|---|
| 1 | Next at fit, RAW mode, per camera (A7R IV, A1, RW2, DNG) | V1, V2 | `-Key RIGHT -Count 20 -Gap 1500` | `s6-raw-fit-next-<camera>` |
| 2 | The same in JPEG mode | V1, V2 | as 1 | `s6-jpg-fit-next-<camera>` |
| 3 | Next at 100 % (Z first), RAW and JPEG, A7R IV | V1, V6 | as 1 | `s6-raw-100-next`, `s6-jpg-100-next` |
| 4 | Z after a pause on a shown image (ADR 0008 S6), A7R IV and RW2; Z toggles, so the odd presses are fit → 100 % | V3, V7 | `-Key Z -Count 10 -Gap 3000` | `s6-raw-z-pause-<camera>` |
| 5 | Z right after an image change (ADR 0008 S1); Z toggles, so the odd presses are fit → 100 %; with a short delay the image change itself can be the first change, so the time to the sharp 100 % view is `settled_ms` | V7 | `-PreKey RIGHT -PreDelay 0`, `50` and `150`, `-Key Z` | `s6-raw-z-next-<delay>` |
| 6 | Z in JPEG mode on the A1 (full-size embedded JPEG) | V3 | `-Key Z -Count 10 -Gap 3000` | `s6-jpg-z-a1` |
| 7 | Rapid stepping at 100 %, about 7 images/s, RAW and JPEG. `latency.ps1` waits `-Quiet` ms without change after each press, so the defaults cannot reach 7/s; with these values a press without a change within 70 ms ends after 70 ms (empty `first_ms` = over the V4 limit) | V4, V2 | `-Key RIGHT -Count 30 -Quiet 1 -Timeout 70 -Gap 70` | `s6-raw-100-7fps`, `s6-jpg-100-7fps` |
| 8 | R on a shown image, and right after an image change | G4 (ADR 0008 S3) | `-Key R`; `-PreKey RIGHT -Key R` | `s6-r-shown`, `s6-r-next` |
| 9 | Free zoom from fit, anchor at the centre (A) | V2, §3.4 | `-Key ADD -Count 5`; then `-Key SUBTRACT -Count 5` | `s6-zoom-plus`, `s6-zoom-minus` |
| 10 | Back after three steps (ADR 0008 S5) | V1 | `-Key LEFT` after RIGHT × 3 | `s6-raw-back` |
| 11 | First image after opening, cold (after a restart) and warm, both modes | V5 | by hand or a screen recording; the note names the method | – |
| 12 | Long session: about 15 min of mixed stepping and Z in RAW mode on the A7R IV files; private memory at start (L1), after the first image (L2), after 10 min (L3), after closing the tab (L4); a clean exit | V8 | the `-Proc` columns of runs 1–10, plus Task Manager | `s6-mem` (note) |
| 13 | By hand, with screenshots: zoom and position persist and small images are centred (V6); the status line in both modes, for RAW, JPEG and TIFF (G1); a truncated RAW file shows an error, never bad data (G3); the mode never changes by itself (G4); focus after minimise, restore, resize and the import dialog (GR6, GR7) | V6, G1–G4, GR6, GR7 | checklist | – |

TIFF and RAW next with colour management are already measured (`c4-tiff-next-2`; the last 20 rows of
`c4-raw-next`, F6) and are not repeated. DPI change, a second screen and GPU device loss stay postponed
(GR7, ADR 0008 §18.4).

**T75 – the report** (Claude agent, after S6a):

- **Files:** a new `docs/analyses/<date of the runs>-adr0009-s6-acceptance.md` (from
  `docs/analyses/TEMPLATE.md`); `docs/adr/0009-egui-loupe-migration.md` (the status list and §9
  only); `docs/GUI_REQUIREMENTS.md` (the GR8 status sentence only). This file: rule 11.
- **Source of truth:** the `s6-` rows of `latency-rawstride.csv`, the S6a log rows and the notes in
  `D:\PhotoWorkflowData\analysis\s6\`; for TIFF/RAW with colour management, the F6 log rows. Write only
  what they record. The CSV uses decimal commas.
- **What:**
  1. **The report:** one table row per requirement (V1–V8, G1–G4, GR6, GR7) with the target, what
     was measured (median, 95th percentile, maximum and n; never a median alone, ADR 0008 §8.3), the
     number of screen changes per press for V2, and **met / not met / not measured**. A column with
     the P7 figure (ADR 0008 §18.3) for comparison; P7 is not an acceptance criterion (TESTING.md §2).
     The known limits of ADR 0009 §8.3 in their own section, stated as expected. Invalid runs listed
     with the reason, not used. Measured values marked [M], interpretations [T], as in the ADRs.
  2. **ADR 0009:** a status line "S6 done (<date>): results in `docs/analyses/…`" after amendment 1;
     §9 S6 "**Done:** T75". Nothing else.
  3. **GR8:** after "the switch waits for the acceptance tests", add "S6 (ADR 0009) measured on
     <date>: <all met / the requirements not met>." Nothing else.
- **Runs 11–13 were deferred by the owner (2026-11-11).** The report marks V5, V8 over a long session
  (L1–L4) and the checks of run 13 (V6, G1, G3, G4, GR6, GR7) as **not measured**, says that S6 is
  partly done, and that `s6a.ps1 -Run 11,12,13` runs them later. The per-run memory columns
  (`proc_private_mb`) are reported as they are, without an L1–L4 claim. ADR 0009 §9 S6 then reads
  "**Partly done:** T75 (runs 11–13 open)" instead of "**Done:** T75", and the GR8 sentence says the same.
- **Rules:** the report draws no conclusion about the switch; that is the owner's decision. English,
  the style of the existing documents; rules 10–12.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
