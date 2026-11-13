# 2026-11-11 – RawStride (`pw-loupe`): ADR 0009 S6 acceptance measurement

- Module: `pw-loupe` (the RawStride program), with `pw-viewcore`, `pw-render` and the decoders behind it
- Question: how does RawStride, as it is after milestone 7, measure against the acceptance criteria
  of ADR 0009 §8 (V1–V8, G1–G4, GR6, GR7)?
- Status: **S6 is partly done.** Runs 1–10 of S6a were made; runs 11–13 were deferred by the owner
  (2026-11-11) and are reported as **not measured** below. `s6a.ps1 -Run 11,12,13` runs them later.
  This report draws no conclusion about the switch from the Tauri app (GR8); that is the owner's
  decision.
- Build: a release build (`cargo build --release --workspace`, `target\release\pw-loupe.exe`). The
  S6a notes do not record the commit or a binary SHA-256. The last commit before the runs was
  `f884ed5` (13:40); the runs were made 14:33–15:01 [T: the build is of that commit].
- Machine and conditions: thanos (i7-9700K, 32 GB, Arc B580, Windows 11); the number of screens
  connected is not recorded in S6a; window maximised on the main screen; the monitor profile of the
  CX240 was active (the window title of every `s6-` row ends in "Skärmprofil: CX240(…)08CAL
  (DP).icc"); default budgets. RawStride was started on the first file of
  `D:\PhotoWorkflowData\s6\<camera>` (which opens the loupe) and restarted before each run except
  `s6-zoom-minus`, which continued in the window of `s6-zoom-plus`, so the program's caches were
  empty at the start of each other run. The state of the Windows file cache is not recorded.
- Input sets: `D:\PhotoWorkflowData\s6\`: `a7r` 5 files (Sony A7R IV), `a1` 14 (Sony A1), `rw2` 21
  (Panasonic RW2), `dng` 9 (DNG).
- Sources: the `s6-` rows of `D:\PhotoWorkflowData\pm-study\results\latency-rawstride.csv`; the
  S6a rows of the Log in `docs/tasks/done/log-2026-11-08-to-2026-11-13.md`; `D:\PhotoWorkflowData\analysis\s6\s6a-log.txt` and
  `mem.csv`. For TIFF and RAW with colour management: the F6 rows of the Log (labels
  `c4-tiff-next-2`, `c4-raw-next`).

## How to read it

- **[M]** measured; **[T]** interpretation or estimate, as in the ADRs.
- **Times** are `first_ms` of `latency.ps1`: from the key press until the image area changed on
  screen. Every press in runs 1–10 had at most one change, so `settled_ms` (the response time of
  `requirements-image-viewer.md`) equals `first_ms` in every row. The screen is sampled about every
  17 ms, so the values are upper bounds and values below about 15 ms cannot be separated.
- **Statistics:** median / 95th percentile / maximum, and n (ADR 0008 §8.3). Medians are those of
  `s6a-log.txt` (the middle value, or the mean of the two middle values); the 95th percentile is the
  nearest-rank value of the CSV rows. With n below 20 the 95th percentile is the maximum.
- **Verdicts:** met / not met / not measured, against the targets of
  `requirements-image-viewer.md` and `GUI_REQUIREMENTS.md` as written.
- **P7** (ADR 0008 §18.3, glow) is shown for comparison only. It is not an acceptance criterion
  (TESTING.md §2).

## Measured / observed

### Requirements

| Req. | Target | Measured [M] | Changes per press | P7 (glow), for comparison | Verdict |
|---|---|---|---|---|---|
| **V1** | Median response ≤ 30 ms at fit and at 100 %, for every format whose embedded preview is shown | **Fit, RAW mode** (runs 1, two rounds): A7R IV n=4 15.6 / 16.0 / 16.0 and 16.8 / 17.0 / 17.0; A1 n=13 16.7 / 17.5 / 17.5 and 16.8 / 17.4 / 17.4; RW2 n=20 16.8 / 17.3 / 17.8 and 16.8 / 17.3 / 17.3; DNG n=8 16.8 / 17.2 / 17.2 and 16.8 / 17.0 / 17.0 ms.<br>**Fit, JPEG mode** (run 2, two rounds): A7R IV n=4 17.0 / 17.4 / 17.4 and 16.9 / 17.2 / 17.2; A1 n=13 16.8 / 17.8 / 17.8 and 16.9 / 17.3 / 17.3; RW2 n=20 16.8 / 17.1 / 17.2 and 16.9 / 17.2 / 17.3; DNG n=8 16.8 / 17.1 / 17.1 and 16.8 / 17.5 / 17.5 ms.<br>All fit runs together: n=180, 16.8 / 17.3 / 17.8 ms.<br>**100 %, A7R IV** (run 3): RAW n=4 17.2 / 17.8 / 17.8; JPEG n=4 16.6 / 16.9 / 16.9 ms.<br>**Back** (run 10, LEFT after RIGHT × 3, RAW, A7R IV): n=3 17.0 / 17.0 / 17.0 ms.<br>**With colour management** (F6): TIFF `c4-tiff-next-2` n=8 17.4 / 30.6 / 30.6 ms; RAW `c4-raw-next` (last 20 rows) n=20 17.0 / 17.5 / 17.5 ms | 1 in every press (0 presses with none, 0 with more than one) | Image change, prefetched: A7R 12.8 / – / 20.4 ms (12 of 12 ≤ 30 ms); A1 12.8 / – / 18.6 ms. RAW mode at fit only | **Met.** 100 % was measured on the A7R IV only |
| **V2** | No intermediate frames: one swap per change; a neutral indicator only after 1 s | Runs 1–10: no press had more than one screen change. Free zoom from fit, anchor at the centre (run 9, A7R IV): + n=5 16.7 / 17.1 / 17.1 ms; − n=5 16.6 / 16.9 / 16.9 ms. Run 7 RAW: 16 of 20 presses showed no change within 70 ms, so V2 cannot be judged for them | ≤ 1 in every press of runs 1–10 | Every press one change; no intermediate or black frame | **Met** by the method of ADR 0009 §8.2 (changes per press) |
| **V3** | Fit ↔ 100 % with a full-size embedded JPEG: ≤ 30 ms, also the first time for an image | Run 6 (JPEG mode, A1, n=10, a 3 s gap): fit → 100 % 51.8, 16.4, 16.7, 16.9, 16.0 ms (16.7 / 51.8 / 51.8); back to fit 16.7 / 16.7 / 16.7 ms. The 51.8 ms press is the first Z after the start, the only press that is the first time for the image. Run 4 (RAW mode) is under V7: the A7R IV and RW2 previews are not full size | 1 | Z fit ↔ 100 %: 11.6 / – / 23.8 ms, 20 of 20 ≤ 30 ms (A7R, RAW mode; P7 had no JPEG mode) | **Not met**: the first time for an image took 51.8 ms. Later presses 16.0–16.9 ms. RAW mode on a camera with a full-size preview (A1) was not measured |
| **V4** | At about 7 images/s and 100 %: each step ≤ 70 ms; never an image other than the current one | Run 7, RW2, n=20 each (`-Quiet 1 -Timeout 70 -Gap 70`): **RAW: 16 of 20 presses with no change within 70 ms**; the 4 others 17.0, 17.4, 15.9, 22.6 ms. **JPEG: 20 of 20 within 70 ms**, 50.0 / 50.8 / 51.0 ms (16.9–51.0). Whether a shown image was ever other than the current one cannot be seen by the method | ≤ 1 | Not measured at 7/s on screen. Image change every 400 ms: 133 / – / 200 ms, 4 of 12 ≤ 30 ms; RAW hit rate at 4–10 images/s 10 % | **Not met** in RAW mode (16 of 20 steps over 70 ms). Met in JPEG mode for the step time |
| **V5** | First image of a folder at the current zoom in ≤ 150 ms | Not measured (run 11 deferred) | – | – (S4 cold mostly missed) | **Not measured** |
| **V6** | Zoom and position persist when stepping; small images centred | Not checked (run 13 deferred). Run 3 measured the time of steps at 100 % (V1), not the persistence | – | – | **Not measured** |
| **V7** | Sharp real 100 % from RAW ≤ 30 ms after the zoom command, also when zooming at once | Run 4 (Z after a 3 s pause, RAW, n=10): A7R IV fit → 100 % 82.6, 15.8, 16.5, 16.5, 17.0 ms (16.5 / 82.6 / 82.6), back 16.9 / 19.7 / 19.7; RW2 fit → 100 % 98.9, 16.2, 16.7, 16.2, 16.7 ms (16.7 / 98.9 / 98.9), back 16.1 / 16.6 / 16.6.<br>Run 5 (RIGHT, then Z after 0 / 50 / 150 ms, RAW, RW2, n=10 each; each fit → 100 % press is on a new image): 0 ms: 81.9, 16.3, 33.2, 33.0, 32.1 (33.0 / 81.9 / 81.9), the 100 % → fit presses 32.7 / 32.8 / 32.8; 50 ms: 99.8, 17.0, 83.9, 16.8, 17.0 (17.0 / 99.8 / 99.8); 150 ms: 231.4, 17.1, 17.3, 17.2, 17.2 (17.2 / 231.4 / 231.4) ms | 1 | Z fit ↔ 100 %: 11.6 / – / 23.8 ms, 20 of 20 ≤ 30 ms (A7R) | **Not met**: the first Z of each run took 81.9–231.4 ms, and at 0 ms delay three of five presses took 32.1–33.2 ms |
| **V8** | A configurable memory limit that is never exceeded; V1–V7 hold within it | Not measured over a long session (run 12 deferred; no L1–L4). The per-run maximum of `proc_private_mb` is listed under "All runs" (544–5157 MB) | – | P5 run, A7R, textures 1 GB and core sessions 1.9 GB: 3.3 GB RAM; stress M1: 5.5 GB | **Not measured** |
| **G1** | The status line says what is shown (embedded preview with size, or RAW, or RAW being prepared) | Not checked (run 13 deferred) | – | – | **Not measured** |
| **G2** | Slow work never blocks keys, zoom or pan | Not checked (run 13 deferred) | – | – | **Not measured** |
| **G3** | A damaged file never shows bad data; an unreadable file never leaves the previous image | Not checked (run 13 deferred) | – | – | **Not measured** |
| **G4** | Two modes chosen by the user; the program never changes the mode by itself | The rule was not checked (run 13 deferred). Time of the R switch (run 8, RAW start): on a shown image (A7R IV) n=6 17.1 / 21.1 / 21.1 ms; right after RIGHT (RW2) n=6 32.3 / 47.8 / 47.8 ms | 1 | – (P7 had no JPEG mode) | **Not measured** (G4 has no time target) |
| **GR6** | Keyboard first; focus kept after minimise, restore or a dialog | Not checked (run 13 deferred) | – | Focus kept after minimise and restore (by hand) | **Not measured** |
| **GR7** | Resize without wrong scaling (DPI and a second screen postponed) | Not checked (run 13 deferred) | – | Resize and maximise rescaled correctly (by hand) | **Not measured** |

### All runs

Times in ms, median / 95th percentile / maximum. "None" and "> 1": presses with no change and with
more than one change. Memory: the largest `proc_private_mb` of the run.

| Run | Label | Mode | Set | n | first_ms | None | > 1 | Memory (MB) |
|---|---|---|---|---|---|---|---|---|
| 1 | `s6-raw-fit-next-a7r` (round 1) | RAW, fit | A7R IV | 4 | 15.6 / 16.0 / 16.0 | 0 | 0 | 1652 |
| 1 | `s6-raw-fit-next-a1` (round 1) | RAW, fit | A1 | 13 | 16.7 / 17.5 / 17.5 | 0 | 0 | 5087 |
| 1 | `s6-raw-fit-next-rw2` (round 1) | RAW, fit | RW2 | 20 | 16.8 / 17.3 / 17.8 | 0 | 0 | 2415 |
| 1 | `s6-raw-fit-next-dng` (round 1) | RAW, fit | DNG | 8 | 16.8 / 17.2 / 17.2 | 0 | 0 | 1780 |
| 2 | `s6-jpg-fit-next-a7r` (round 1) | JPEG, fit | A7R IV | 4 | 17.0 / 17.4 / 17.4 | 0 | 0 | 1321 |
| 2 | `s6-jpg-fit-next-a1` (round 1) | JPEG, fit | A1 | 13 | 16.8 / 17.8 / 17.8 | 0 | 0 | 5111 |
| 2 | `s6-jpg-fit-next-rw2` (round 1) | JPEG, fit | RW2 | 20 | 16.8 / 17.1 / 17.2 | 0 | 0 | 1946 |
| 2 | `s6-jpg-fit-next-dng` (round 1) | JPEG, fit | DNG | 8 | 16.8 / 17.1 / 17.1 | 0 | 0 | 1885 |
| 1 | `s6-raw-fit-next-a7r` (round 2) | RAW, fit | A7R IV | 4 | 16.8 / 17.0 / 17.0 | 0 | 0 | 1538 |
| 1 | `s6-raw-fit-next-a1` (round 2) | RAW, fit | A1 | 13 | 16.8 / 17.4 / 17.4 | 0 | 0 | 3007 |
| 1 | `s6-raw-fit-next-rw2` (round 2) | RAW, fit | RW2 | 20 | 16.8 / 17.3 / 17.3 | 0 | 0 | 2318 |
| 1 | `s6-raw-fit-next-dng` (round 2) | RAW, fit | DNG | 8 | 16.8 / 17.0 / 17.0 | 0 | 0 | 1413 |
| 2 | `s6-jpg-fit-next-a7r` (round 2) | JPEG, fit | A7R IV | 4 | 16.9 / 17.2 / 17.2 | 0 | 0 | 1363 |
| 2 | `s6-jpg-fit-next-a1` (round 2) | JPEG, fit | A1 | 13 | 16.9 / 17.3 / 17.3 | 0 | 0 | 5157 |
| 2 | `s6-jpg-fit-next-rw2` (round 2) | JPEG, fit | RW2 | 20 | 16.9 / 17.2 / 17.3 | 0 | 0 | 2232 |
| 2 | `s6-jpg-fit-next-dng` (round 2) | JPEG, fit | DNG | 8 | 16.8 / 17.5 / 17.5 | 0 | 0 | 1889 |
| 3 | `s6-raw-100-next` | RAW, 100 % | A7R IV | 4 | 17.2 / 17.8 / 17.8 | 0 | 0 | 1668 |
| 3 | `s6-jpg-100-next` | JPEG, 100 % | A7R IV | 4 | 16.6 / 16.9 / 16.9 | 0 | 0 | 1378 |
| 4 | `s6-raw-z-pause-a7r` | RAW, Z | A7R IV | 10 | 16.8 / 82.6 / 82.6 | 0 | 0 | 999 |
| 4 | `s6-raw-z-pause-rw2` | RAW, Z | RW2 | 10 | 16.4 / 98.9 / 98.9 | 0 | 0 | 544 |
| 5 | `s6-raw-z-next-0` | RAW, RIGHT + Z | RW2 | 10 | 32.7 / 81.9 / 81.9 | 0 | 0 | 1669 |
| 5 | `s6-raw-z-next-50` | RAW, RIGHT + Z | RW2 | 10 | 16.8 / 99.8 / 99.8 | 0 | 0 | 1695 |
| 5 | `s6-raw-z-next-150` | RAW, RIGHT + Z | RW2 | 10 | 17.2 / 231.4 / 231.4 | 0 | 0 | 1716 |
| 6 | `s6-jpg-z-a1` | JPEG, Z | A1 | 10 | 16.7 / 51.8 / 51.8 | 0 | 0 | 1651 |
| 7 | `s6-raw-100-7fps` | RAW, 100 % | RW2 | 20 | 17.2 / 22.6 / 22.6 (the 4 presses with a change) | 16 | 0 | 1401 |
| 7 | `s6-jpg-100-7fps` | JPEG, 100 % | RW2 | 20 | 50.0 / 50.8 / 51.0 | 0 | 0 | 884 |
| 8 | `s6-r-shown` | R from RAW | A7R IV | 6 | 17.1 / 21.1 / 21.1 | 0 | 0 | 970 |
| 8 | `s6-r-next` | RIGHT + R | RW2 | 6 | 32.3 / 47.8 / 47.8 | 0 | 0 | 1086 |
| 9 | `s6-zoom-plus` | + from fit | A7R IV | 5 | 16.7 / 17.1 / 17.1 | 0 | 0 | 924 |
| 9 | `s6-zoom-minus` | − | A7R IV | 5 | 16.6 / 16.9 / 16.9 | 0 | 0 | 924 |
| 10 | `s6-raw-back` | RAW, LEFT | A7R IV | 3 | 17.0 / 17.0 / 17.0 | 0 | 0 | 1497 |

Runs 4–6 mix both directions of Z; the fit → 100 % presses alone are given under V3 and V7.

### Not measured: runs 11–13

Deferred by the owner (2026-11-11). Not measured: V5 (run 11), V8 over a long session with L1–L4
(run 12), and the hand checks of run 13: V6, G1, G3, G4, GR6, GR7. G2 was not in the S6a plan and is
not measured either. DPI change, a second screen and GPU device loss stay postponed (GR7, ADR 0008
§18.4). `s6a.ps1 -Run 11,12,13` runs them later.

### Runs not used

| Run | Reason |
|---|---|
| First attempt at run 12 (`s6-mem`, 15:02, `mem.csv`) | Not done as intended: no long session (the four L marks within about 30 s; `s6a-log.txt`: "avslutad efter 0,2 s"). Kept as a record, not used |
| `c4-raw-next`, the first 20 rows (F6) | Invalid: RawStride was started with a folder (the grid), so no press changed the screen |
| `c4-raw-next-2` (F6) | Does not measure stepping: it reached the end of the folder and cold files. Replaced by the last 20 rows of `c4-raw-next` |

No run of S6a 1–10 was invalid (S6a Log row).

### Differences from the plan

- Runs 1 and 2 were made twice; both rounds are valid and both are listed.
- Run 7 has n=20 per mode in the CSV (the plan had `-Count 30`), on the RW2 set.

## Known limits of ADR 0009 §8.3

These were expected before S6 and are not new findings. Milestone 1 was not expected to fix them.

- **Cold RAW start and continuous fast stepping in RAW mode (V4, V5)** are limited by the CPU unpack
  (about 337 ms for A7R) and PPG. **As expected [M]:** in run 7, 16 of 20 RAW steps at about 7/s showed
  no change within 70 ms. V5 was not measured.
- **Free zoom between the fit view's resolution and 100 %** is not prefetched (a region render,
  about 100–300 ms on A7R [T]). Run 9's ten presses all took ≤ 17.1 ms with one change each. The zoom
  levels reached are not recorded, so whether the presses entered that range is not known [T].
- **JPEG mode on the A1:** a full decode takes about 150 ms. The first Z in JPEG mode on the A1 took
  51.8 ms (run 6). JPEG stepping at 10/s on the A1 was not run; run 7 stepped the RW2 set in JPEG mode
  at about 7/s.

## Interpretation

- **[T]** The slow presses in runs 4–6 are the first Z of a run, after a restart, or on an image just
  reached (run 5). The later presses on the same image take one or two screen samples. This fits the
  zoom view not yet being prepared on the first press; confirming it needs the program's own render
  timings for those presses.
- **[T]** In run 5 at 0 ms delay, and in run 8 after RIGHT, several presses took about 32–33 ms, two
  screen samples. With no delay the image change and the timed key may land in the same swap; the
  method cannot separate them.
- **[T]** One change per press means the single swap was the final view, so the fit → 100 % times
  under V7 are times to the RAW 100 % view, not to a blurred step. Sharpness itself was not checked
  by eye in S6a.
- **[T]** The `proc_private_mb` values include the whole process (core, textures, egui, driver), so
  they are not compared with the core and texture budgets here; that is the L1–L4 method of run 12.

## Follow-up

- Runs 11–13 (`s6a.ps1 -Run 11,12,13`): V5, V8 over a long session, and V6, G1–G4, GR6, GR7 by hand.
- The owner decides on the switch from the Tauri app (GR8) and on the requirements not met (V3, V4 in
  RAW mode, V7).
- S7 (TESTING.md: the loupe's tests) is a separate task.
