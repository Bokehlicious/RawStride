# T338. ADR 0027 W5: the camera's brightness with its sign turned — Open

- **Why:** W5's hand check (owner, 2026-11-26; ADR 0027 §4.4 "W5's hand check", §3.4). With
  the camera's brightness on (Shift+B), the view of an X-T30 RAF (file value −0.72) and of a
  Ricoh GR III DNG (−0.51) is darker than the RAW view at 0 EV, while the embedded JPEG is
  brighter than both. The file's value says how far the camera kept the RAW data below its own
  rendering; the view must apply it **with the sign turned** (+0.72, +0.51), for both formats.
  The hover text then names the value as the camera's brightness, not as "from the file". The
  first task of the chain T338 → T339 → T340 from W5's hand check.
- **What the reviewer checked (2026-11-26):**
  - `crates/pw-viewcore/src/native.rs`: `NativeBackend::camera_exposure` (about lines 508–515)
    returns `s.camera_exposure().map(|c| f64::from(c.ev))`, the file's value as `pw-libraw`
    gives it (`CameraExposure::ev`, "as the file has it"). The test module starts at about line
    563.
  - `crates/pw-viewcore/src/backend.rs`: the trait's doc comment of `camera_exposure` (about
    lines 495–497) says "as the file gives it".
  - The view core (`state.rs`, `ExposureBase::Camera(f64)`, `exposure_of`) adds whatever the
    backend returns and limits the sum to −2…+3 EV; its tests use the backend's value directly,
    so nothing in `state.rs` changes.
  - `crates/pw-loupe/src/preview_text.rs`: `camera_hover` (about lines 500–533) builds eight
    texts that end in "… EV from the file…"; the test
    `the_auto_and_camera_marks_explain_the_base_and_manual_steps` (about line 1106) checks them
    with `ExposureBase::Camera(-0.51)`, `(0.70)`, `(0.0)`, `(-2.51)`, `(3.10)` (about lines
    1154–1225). The mark ("−0.5 EV camera") is made from the value the core gives and needs no
    change.
  - `crates/pw-viewcore/src/state.rs`: the doc comment of `ExposureMode::Camera` (about line
    266) says "The camera's own brightness value from the file (W5-Q5)".
  - `pw-libraw` keeps the file's value as it is (its doc and its test
    `the_camera_exposure_comes_only_from_a_known_source`); it does not change.
- **Crates:** `pw-viewcore`, `pw-loupe`. **Files:** `crates/pw-viewcore/src/native.rs`,
  `crates/pw-viewcore/src/backend.rs` and `crates/pw-viewcore/src/state.rs` (doc comments
  only), `crates/pw-loupe/src/preview_text.rs`,
  `docs/glossary.md`, `docs/test-inventory.md` (rule 12), and this task's own file (status and
  log).
- **Model:** Opus — two crates (README, "Model"), though each change is small.
- **What:**
  1. **The sign:** in `native.rs`, a private function `fn camera_brightness(file_ev: f32) ->
     f64` returns `-f64::from(file_ev)`, with a doc comment: the brightness the view adds is the
     file's value with its sign turned, since the value says how far the camera kept the RAW
     data below its own rendering (ADR 0027 §4.4, W5's hand check, owner 2026-11-26).
     `camera_exposure` uses it: `s.camera_exposure().map(|c| camera_brightness(c.ev))`. Its doc
     comment says the same in one line. No rounding or other change of the value (a tiny value
     shown as "−0.00" is an older, separate matter, not this task's).
  2. **The trait's doc** in `backend.rs`: "The camera's own brightness of the opened image in
     EV, the brightness the view adds (for `NativeBackend` the file's value with its sign
     turned; not limited to [`Ev`]'s range; ADR 0027 W5-Q5, §4.4). Asked once after a successful
     open. The default has none."
  3. **The mode's doc** in `state.rs` (about line 266): "The camera's own brightness: the
     backend's value, for `NativeBackend` the file's value with its sign turned (W5-Q5, §4.4)."
     Nothing else in `state.rs` changes (its test
     `camera_exposure_preserves_the_file_value_and_limits_the_render` keeps its name and values:
     it feeds the core the backend's value).
  4. **The hover text:** in `camera_hover`, every "EV from the file" becomes "EV from the
     camera" (eight texts; the rest of each sentence, the limit and the steps unchanged). The
     texts for a file without a value ("the file gives no value, so 0 EV is used") and while it
     is read ("the file is being read") stay as they are.
  5. **The glossary:** the row "Camera brightness, Camera Brightness" (about line 291) says the
     file's value is applied with its sign turned (ADR 0027 §4.4, owner 2026-11-26; T338).
- **Tests to add:**
  - `native.rs`: `camera_brightness` turns the sign: `-0.72 → 0.72` (within 1e-6, the `f32`
    value), `-0.5062 → 0.5062` (within 1e-6), `0.0` gives a value `== 0.0`, `1.2 → -1.2`
    (within 1e-6). The real-file test `crates/pw-viewcore/tests/native_files.rs` (about line
    262, ignored) checks only `is_some()` and does not change.
  - `preview_text.rs`: the existing test
    `the_auto_and_camera_marks_explain_the_base_and_manual_steps` is **updated** to expect "from
    the camera" in its camera rows (asked for here, a changed text, not a weakening, rule 5); its
    values and marks stay.
- **`docs/test-inventory.md`:** the `pw-viewcore` unit row (counts, and the new test named).
- **Not in this task:** the buttons (T339), the setting "Auto exposure at start" (T340), and an
  offset worked out from the embedded JPEG (`backlog.md`).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–13 (rule 13: the texts stay in
  `preview_text.rs`). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.
  At its Done the reviewer adds a `CHANGELOG.md` line (Fixed: the camera's brightness made the
  view darker instead of brighter).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-26 | Open | Added after W5's hand check (ADR 0027 §4.4). Cold read: all names and lines found; fixed: the −0.0 point dropped (`ev_sign` gives no sign to −0.0 and the mark goes through milli values, so it never showed a minus; a tiny value showing "−0.00" is older and out of scope); `state.rs`'s doc comment of `ExposureMode::Camera` added (doc only); the ignored real-file test and the inventory row named. Opus kept (two crates) |
