# T331. ADR 0027 W5: the exposure per LibRaw render and the camera's brightness — Done

- **Why:** ADR 0027 §3.4 and W5-Q2, W5-Q3, W5-Q4, W5-Q5 (answered by the owner 2026-11-25, ADR
  0027 §4.4). The temporary exposure is applied in LibRaw (`exp_correc`, `exp_shift`,
  `exp_preser`) before the demosaic, passed **with each render** and never left in the session,
  so the focus job, the statistics and the clipping map always run on ADR 0006 §2's settings
  (A-Q6). The camera's brightness (DNG `BaselineExposure`, the Fujifilm RAF value) is read once
  when the file is opened. This task makes the shim's and `pw-libraw`'s part, and measures what
  the later tasks depend on: the time of the statistics on the owner's 61 Mpix files (W5-Q4) and
  the cost of an exposed render. The first task of W5's chain **T331** → T332 → T333 → T334 →
  T335; nothing uses the new functions yet.
- **What the reviewer checked (2026-11-25):**
  - `crates/pw-libraw/csrc/pwlr.cpp`: `pwlr_open` sets every LibRaw parameter once (lines
    119–128: `no_auto_bright = 1`, `bright = 1.0`, `output_bps = 8`, …) and none of `exp_correc`,
    `exp_shift`, `exp_preser`; `pwlr_render` (line 236) sets `user_qual`, `half_size` and
    `cropbox` per render, then `dcraw_process` with the stop of T327; `pwlr_info` (line 70) is
    the struct `pwlr_open` fills.
  - LibRaw 0.22's headers (vcpkg, `libraw_types.h`): `imgdata.params.exp_correc` (int),
    `exp_shift` (float, linear: 2^EV), `exp_preser` (float, 0–1);
    `imgdata.color.dng_levels.baseline_exposure` (float) with the flag
    `LIBRAW_DNGFM_BASELINEEXPOSURE` in `dng_levels.parsedfields` (`libraw_const.h`);
    `imgdata.makernotes.fuji.ExpoMidPointShift` (float); `imgdata.idata.maker_index` and
    `LIBRAW_CAMERAMAKER_Fujifilm`. By LibRaw's documentation `exp_shift` is usable from 0.25 to
    8.0 (−2 to +3 EV) and `exp_preser` protects the highlights only when the image is made
    brighter.
  - `crates/pw-libraw/src/lib.rs`: `Session::render` (private, line 582) passes everything to
    `pwlr_render`; `render_half_stoppable` (line 675) and `render_region_stoppable` (line 711)
    are the renders the view core uses (through `pw-viewcore` `native.rs`, which keeps working:
    their signatures stay); `Info` (line 41) mirrors `pwlr_info`, and its literal is in
    `open_with_diagnostics` (lines 424–435).
  - The owner's `PW_TESTDATA` (`D:\PhotoWorkflowData\RAW`, ExifTool 13.64, the reviewer
    2026-11-25): 24 ARW, 25 RW2, 5 RAF (X-T30, `_DSF*.RAF`, `RawExposureBias` −0.7 each), 4
    `TN0025*.DNG` (RICOH GR III, `BaselineExposure` −0.5062) and 5 `IMG_*.dng` (Xiaomi,
    `BaselineExposure` 0). The headers give no "not set" value for `ExpoMidPointShift`.
  - `crates/pw-libraw/tests/stop.rs`: the model for a test on the owner's files (`PW_TESTDATA`,
    `#[ignore]`, `files_with`), and `timing_of_a_stop` for a timing printed with `--nocapture`.
  - The owner's files (ExifTool 13.64, ADR 0027 §4.4): DNG `BaselineExposure` −0.51 EV (Ricoh
    GR III) and 0 (a Xiaomi phone); Fujifilm RAF `RawExposureBias` −0.7 (X-T30); none in Sony
    ARW (A1, A7R IV) or Panasonic RW2 (GM1).
- **Crate:** `pw-libraw`. **Files:** `crates/pw-libraw/csrc/pwlr.cpp`,
  `crates/pw-libraw/src/lib.rs`, `crates/pw-libraw/tests/exposure.rs` (new),
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — the C++ shim and the FFI (README, "Model").
- **What:**
  1. **The exposure per render (shim):** `pwlr_render` gets a parameter `float exp_shift` right
     after `half` (the linear factor, 2^EV, by LibRaw's documentation; 1.0 = no correction).
     Every render sets, before `dcraw_process`: `exp_correc = (exp_shift != 1.0f) ? 1 : 0`,
     `exp_shift`, and `exp_preser = PWLR_EXP_PRESER`, a named constant 1.0f with a comment
     (W5-Q2: tuned in the hand check). So a render without a correction leaves no trace of an
     earlier one. The range check comes right after the check of the demosaic method and before
     the stop's first ask: a factor that is not finite or lies outside 0.25 to 8.0 is refused
     before LibRaw is called, with the error `"exposure shift %g is outside 0.25 to 8"`, and the
     session is untouched (not spent). The
     comment above `pwlr_render` and the file's head comment (lines 1–7, "All decodes use the same
     fixed settings") say that the exposure is the one setting passed per render.
  2. **The exposure per render (Rust):** a public type `Exposure` (`Clone, Copy, Debug,
     PartialEq`), the correction in EV:
     - `Exposure::NONE` (0 EV); `Exposure::MIN_EV = -2.0` and `Exposure::MAX_EV = 3.0` (`f32`);
     - `Exposure::from_ev(ev: f32) -> Option<Exposure>`: `None` when `ev` is not finite or lies
       outside `MIN_EV..=MAX_EV`; `ev(self) -> f32`; and a private `shift(self) -> f32`
       (`ev.exp2()`, `f32::exp2`, so −2 gives exactly 0.25 and 3 exactly 8.0; a unit test holds
       it).
     - `Session::render` gets an `exposure: Exposure` parameter and passes `shift()`.
     - Two new public renders: `render_half_exposed(&mut self, exposure: Exposure, stop: &(dyn
       Fn() -> bool + Sync)) -> Result<Rgb8>` and `render_region_exposed(&mut self, r: Rect,
       margin: u32, demosaic: Demosaic, exposure: Exposure, stop: &(dyn Fn() -> bool + Sync)) ->
       Result<Rgb8>`, documented like the stoppable ones (the same stop and its cost) plus the
       exposure. `render_half_stoppable` and `render_region_stoppable` stay with their
       signatures and call them with `Exposure::NONE`; every other render passes
       `Exposure::NONE`. `render_region_checked` gets the `exposure` parameter.
     - The module's head comment (lib.rs lines 5–8, "Every render uses the same fixed settings")
       names the exposure as the one per-render setting, and that the region identity holds
       with it (point 4's test).
  3. **The camera's brightness (W5-Q5):** `pwlr_info` gets `float camera_ev` and `int
     camera_ev_from` (0 none, 1 DNG `BaselineExposure`, 2 Fujifilm), placed after `damaged` and
     before `error`, in the C struct, in `Info` and in `Info`'s literal alike. `pwlr_open` fills
     them after a successful unpack: a DNG (`idata.dng_version != 0`) whose
     `dng_levels.parsedfields` has `LIBRAW_DNGFM_BASELINEEXPOSURE` gives 1 and
     `baseline_exposure`; a file with `idata.maker_index == LIBRAW_CAMERAMAKER_Fujifilm` that is
     not a DNG gives 2 and `makernotes.fuji.ExpoMidPointShift`, made to read as ExifTool's
     `RawExposureBias` (−0.7 on the owner's X-T30 files): first write the code with the value as
     LibRaw has it, run the `camera_exposure` test of point 5 (it prints the value of each RAF),
     then finish this point: if LibRaw gives −0.7, keep it; if +0.7, negate it (the comment says
     which was found); if anything else (for example 0, or a value that does not follow
     ExifTool's), stop and report: the owner decides (W5-Q5 then has no RAF value). Whether
     ExifTool's sign is the one to **add** to the view (as DNG `BaselineExposure` is) is the
     owner's hand check after T335 (the RAW view with the camera's brightness on beside the JPEG
     mode); a wrong sign is then a one-line fix here. Every other file gives 0. A value that is
     not finite counts as none.
     Rust: `Info` gets the two fields; `Session` keeps `camera: Option<CameraExposure>` (any
     `camera_ev_from` other than 1 and 2 gives `None`); `Session::camera_exposure(&self) ->
     Option<CameraExposure>`, a public `CameraExposure { pub ev: f32, pub from: CameraEv }` with
     `pub enum CameraEv { DngBaseline, Fujifilm }` (both `Clone, Copy, Debug, PartialEq`). The
     value is given as the file has it, not limited to the range (W5-Q5: the view core limits
     it, and the hover text gives the file's value).
  4. **The statistics, the clipping map and the focus job stay unexposed:** nothing in
     `pwlr_statistics`, `pwlr_clipping_map` or `pwlr_open` changes with the exposure; the tests
     of point 5 show it.
  5. **The measurement (W5-Q4, W5-Q5):** in `tests/exposure.rs`, an `#[ignore]` test
     `timing_of_exposure` (run with `--release --nocapture`, as `timing_of_a_stop`) that prints,
     per ARW and RAF file of `PW_TESTDATA`, the median of 5: `Session::open`; `Session::statistics`;
     their sum with a half-size render at `NONE` (a step to an image that was not prefetched
     with auto exposure on, W5-Q4); a half-size
     render at `NONE` and at +1 EV; a whole-image PPG render (`render_region_exposed` of the
     whole image) at `NONE` and at +1 EV. The RAF value of point 3 is checked against
     ExifTool's `RawExposureBias` (−0.7 on every RAF, above); no brightness comparison with the
     embedded preview is made in this task (the owner's hand check after T335 does it). The
     numbers go in the log row.
- **Tests to add** (`tests/exposure.rs`, on the owner's files, `#[ignore]` with the reason of
  `stop.rs`, `PW_TESTDATA` and the helpers copied from `stop.rs`, with lists for DNG and RW2
  beside ARW and RAF (`files_with(&["dng"])`, `files_with(&["rw2"])`; the owner's folder has
  them); the unit tests in `lib.rs`'s `mod tests`):
  - Unit: `Exposure::from_ev` gives `Some` for −2, 0, ⅓, 3 and `None` for −2.01, 3.01, NaN,
    +∞ and −∞; `from_ev(-2.0)` has the shift exactly 0.25 and `from_ev(3.0)` exactly 8.0
    (`==`); `Exposure::NONE.ev()` is 0.
  - A render at `Exposure::NONE` (half-size and a region with PPG) gives the bytes of the plain
    render of a fresh session, on an ARW and a RAF.
  - A render at +1 EV followed by one at `NONE` of the same session: the second gives the bytes
    of the plain render (nothing is left in the session).
  - The region identity with an exposure: a region of 600×400 px whose top left corner is at a
    third of the image's width and height (PPG, margin 8) at +1 EV and at −1 EV equals the same
    area of the whole-image render (`render_region_exposed` of the whole image as a region with
    margin 0, as `stop.rs` renders it) at the same exposure, on an ARW and a RAF.
  - The direction: the mean of a half-size render at +1 EV is above the one at `NONE`, and at
    −1 EV below it, on every ARW and RAF file. If −1 EV is not darker (LibRaw may scale the
    white level with the shift), stop and report: W5-Q2's range then needs the owner.
  - `statistics()` and `clipping_map()` after a render at +2 EV equal those of a fresh session.
  - `camera_exposure()`: `None` for every ARW and RW2 file; `Some` with `DngBaseline` for every
    DNG, within 0.01 of −0.51 for the files whose name starts with `TN` (the GR III) and of 0
    for those that start with `IMG_` (the Xiaomi); `Some` with `Fujifilm` for every RAF, within
    0.01 of −0.7 (each value printed).
  - Point 1's refusal is a defence the Rust API cannot reach (`from_ev` checks the range first);
    no test calls the shim directly.
- **Not in this task:** the view core's use of the exposure, the auto exposure and the shadows
  (T332, T333); keys, marks and settings (T334, T335).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text). No new
  dependency. An extra code review follows (the C++ shim).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above;
  the log holds the timings and the RAF check of point 5.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for the cold read and the chain review. Cold read: every LibRaw name found (0.22.3); fixed: the order of point 3 (code, run, then the sign) and its stop; the brightness check against the embedded preview dropped (no JPEG decoder in `pw-libraw`; moved to the owner's hand check after T335) and the RAF value checked against ExifTool's −0.7 instead; `f32::exp2` for exact 0.25 and 8.0; the new fields' place in `pwlr_info` and `Info`'s literal; the range check's place; DNG and RW2 lists; the GR III and Xiaomi files by name (the reviewer read the folder with ExifTool); the identity test's region; a stop if −1 EV is not darker; `Info`'s line |
| 2026-11-25 | Blocked | Chain review (T331–T335): the APIs line up (`Exposure` f32 ↔ `Ev` thousandths, the camera's value f32 → f64, `Thirds` ↔ `ExposureBase::Auto`, the amount `u8`). Fixed: here the open is timed too (W5-Q4 is open + statistics + render); T332 declares the public modules; T333: the dwell after a change, a spent session's failed statistics reopen the file, a failed image's states `Off`; T334 and the plan: `main` waits for T335; T335: where leaving the preview resets the steps, `ev_text` in `f64`, "no usable RAW statistics", the known limits (unexposed overlays, the compare with a non-RAW first side) in a new `known-limits.md` area at Done; the plan's W5 row: the latency series and the hand check's items. `CameraEv` kept for the tests and the log, though the view core uses only the value |
| 2026-11-25 | Open | Set to Open by the reviewer after the owner's approval of the chain; T332–T335 wait in turn |
| 2026-11-25 | Ready for review | Done as written, with two deviations in point 3 decided by the owner during the task (to be written into ADR 0027 W5-Q5 by the reviewer): (1) LibRaw 0.22.3 never sets `LIBRAW_DNGFM_BASELINEEXPOSURE` in `imgdata.color.dng_levels.parsedfields` (`metadata/identify.cpp` copies only the value), so a DNG's value is taken when `baseline_exposure` differs from LibRaw's "not set" value −999 (`utils/init_close_utils.cpp`); the same −999 is used for the Fujifilm value, which LibRaw initialises alike (a RAF without the tag gives none, not −999 EV). (2) RAF: LibRaw gives −0.72 on all 5 X-T30 files, ExifTool's unrounded `RawExposureBias` (`exiftool -n`, tag 0x9650 = −72/100; −0.7 is its display rounding); the sign agrees, so it is kept, and the test checks −0.72 ± 0.01. DNG: −0.5062 on the 4 `TN*` (GR III), 0 on the 5 `IMG_*` (Xiaomi); ARW and RW2: none. −1 EV is darker on every ARW and RAF (half-size means, e.g. TN005701.ARW 44.6 / 69.4 / 103.8 at −1 / 0 / +1 EV). Timing (`timing_of_exposure`, `--release`, median of 5, i7-9700K 8 threads, Windows 11, `PW_TESTDATA` of the owner, 24 ARW + 5 RAF): A7R IV 8660×5784: open 237–258 ms, statistics 169–184, open + statistics + half NONE 625–694, half NONE 209–267, half +1 EV 276–317, whole PPG NONE 1103–1327, +1 EV 1074–1306; A1 9568×6376: open 293–304, statistics 208–218, open + statistics + half 769–794, half NONE 257–263, +1 EV 340–345, whole PPG NONE 1340–1366, +1 EV 1307–1331; X-T30 RAF 6246×4170: open 296–343, statistics 86–91, open + statistics + half 506–567, half NONE 112–116, +1 EV 147–154, whole PPG NONE 1770–1849, +1 EV 1792–1892. An exposed half-size render costs about 30 % more (+65–85 ms); a whole-image PPG render the same as without. (The camera test ran for about 20 s during the timing of the first ARW files.) A half-size RAF render is not deterministic (T327), so the NONE and the leftover tests compare only its size there. `check.ps1` all OK, 2738 passed, 94 ignored; all 7 tests of `tests/exposure.rs` run with `--ignored` and passed. An extra code review of the shim is still to be done (Rules) |
| 2026-11-25 | Done | Reviewed (the reviewer and an independent review agent of the diff, the C++ shim, as Rules asks): no bug; the FFI signature and `pwlr_info`/`Info` match, every render sets `exp_correc`, `exp_shift` and `exp_preser`, the new fields are cleared by `pwlr_open`'s `memset` and the Rust literal. The review's nits: (1) a half-size RAF render compares only the size, so the leftover and NONE tests rest on the ARW and the PPG region there (documented in the test file's head; accepted); (2) the statistics test rendered only at +2 EV: on the owner's word the reviewer added renders at −1 EV (half-size and PPG region) before the statistics, since LibRaw scales its white level only when darkening; run on `PW_TESTDATA` with `--ignored`: passed; `docs/test-inventory.md`'s line says so; (3) the DNG test knows only the `TN` and `IMG_` files of the owner's folder (as `stop.rs` depends on it; accepted); (4) `pwlr_camera_ev` clears the fields again after the `memset` (kept as a defence). The owner's two decisions of point 3 and the timings for W5-Q4 are written into ADR 0027 §4.4 ("W5-Q5 after T331", "T331's timings for W5-Q4"); the owner's W5-Q4 choice is made before T333 is set to Open (already in T333's text). The first A7R IV timings ran beside the camera test (log row above), so their upper ends may be a little high. `.\tools\check.ps1` all OK by the reviewer after the change (2738 passed, 94 ignored, no clippy warning). No CHANGELOG line (nothing the user sees yet). Leftovers: none beyond the above |
