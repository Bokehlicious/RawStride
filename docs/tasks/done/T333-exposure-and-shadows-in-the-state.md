# T333. ADR 0027 W5: the exposure and the shadows in the view core's state — Done

- **Why:** ADR 0027 §2.1, §3.4 and W5-Q2, W5-Q3, W5-Q4, W5-Q5, W5-Q7 (answered by the owner
  2026-11-25, ADR 0027 §4.4). T332 made the render path carry an exposure (`JobView::exposure`,
  an `Ev`) and the shadows (`JobView::shadows`); this task makes the state ask for them: the
  exposure's mode (off, auto, the camera's brightness) and the manual steps, the shadows'
  amount, the statistics made **before** the first view while auto exposure is on, the views
  told apart by them in the cache, and the frame's states for the shell's marks. The third task
  of W5's chain T331 → T332 → **T333** → T334 → T335. Waits for T332.
- **What the reviewer checked (2026-11-25):** sharpening (T316) goes through every place a look
  of the view must go, and the exposure and the shadows follow it:
  - `crates/pw-viewcore/src/state.rs`: `State::sharpening` and `set_sharpening` (line 850);
    `ViewOut` (line 114) and `Resident` (line 132) with `sharpen`, copied in `finish_render` and
    `accept`; `same_view` (line 485, "the same view but for their sharpening"); `serving_of`
    (line 1457: a fine view first while fine details is on, then one whose `sharpen` equals the
    requirement's, then the least dense, then the smallest); `exact` (line 1488); `satisfied`
    (line 1513, with the exception for the current image's `Half` during a zoom gesture);
    `remove_inflight` (line 2671, with a `sharpen` parameter); `replaced_fine` (line 1843) and
    its use in `pick` (about line 2209); `accept`'s fine branch (line 2731:
    `out.sharpen == self.sharpening`); `sharpen_state` (line 2942) and its calls from
    `frame_of`, also on the early returns (no image, a failed image).
  - `go` (line 926, `dragged = false` when the position changes), `open_folder` (line 947),
    `set_files` (line 990): where the current image changes. `set_mode`: the display mode, the
    same image (ADR 0027 §4.4, T328's cold read).
  - `own_flips` (line 581): a per-key map kept, cleared and moved like `sizes` (lines 962, 1066,
    2372); `finish_open_checked` (line 2354) with the closure `profile_unusable`, called from
    `core.rs`'s `Task::Open`, and `finish_open` (test only) calling it.
  - `statistics: HashMap<Key, ImageStatistics>` and `finish_statistics` (line 2594), which keeps
    the statistics of any key whose session came back; `statistics_job` (line 1645: the current
    image's, after its view is on the screen); `plan_wants` (line 1594); `pick` (line 2102),
    which already lends a session for a `Job::Statistics` from its job list (the interactive
    worker only for the current image).
  - T332's `Ev`, `AutoExposure`, `auto_exposure`, `shadows::AMOUNT_RANGE`, and
    `Backend::camera_exposure`.
  - `Frame` is built as a literal outside `pw-viewcore` only in `pw-loupe` `loupe_text.rs`'s
    tests (`frame`, `tiff_frame`, `small_tiff_frame`).
- **Crates:** `pw-viewcore`, `pw-loupe` (test helpers only). **Files:**
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/core.rs` (points 2, 6 and 11),
  `crates/pw-viewcore/src/lib.rs` (the new types exported), `crates/pw-viewcore/tests/engine.rs`,
  `crates/pw-loupe/src/loupe_text.rs` (the new `Frame` fields in its test helpers, off; nothing
  else), `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — a state machine in the view core and more than one crate (README, "Model").
- **What:**
  1. **The settings the shell gives:** `State` gets `exposure_mode: ExposureMode` (public enum
     `Off`, `Auto`, `Camera`; `Clone, Copy, Debug, PartialEq, Eq, Default` with `Off` the default),
     `exposure_steps: i32` (manual steps in ⅓ EV; private, read through the frame) and
     `shadows: Option<u8>` (`None` = off). All off at the start; the shell holds the mode and
     the shadows for the session (A-Q2, W5-Q6) and sets them as it sets sharpening; a new folder
     or list of files does not change them. Setters, each returning whether something changed:
     - `set_exposure_mode(mode)`; turning one mode on is the shell's choice of the other's off
       (A-Q12): the state holds one mode only.
     - `set_shadows(amount: Option<u8>)`: a `Some` above 100 is held as 100.
     - `step_exposure(delta: i32)` (the shell passes 1 or −1): with the current image's total
       before the step, **unlimited** (point 4's sum before `Ev` limits it: whole thirds for the
       modes off and auto, an `f64` for the camera's; its base taken as 0 while it is not
       known), at or above +3 EV and `delta > 0`, or at or below −2 EV and `delta < 0`, nothing
       changes and it returns `false` (W5-Q2: a press beyond a limit changes nothing); also
       `false` when there is no current image or its key's source is not `Source::Raw` (paused,
       A-Q9). Otherwise `exposure_steps += delta`, limited to −15..=15. (A camera value beyond
       a limit, for example −2.7, so takes three presses up before the total leaves −2 EV;
       accepted.)
     - `reset_exposure_steps()`.
     - A change of mode keeps the steps (they are reset only as point 3 says).
     - Each setter that returns `true` also sets `since = Instant::now()`, as a zoom step does
       (about line 1136; W5-Q7: after a change the shown view comes first, the ring, the 100 %
       view, the neighbours and the fine view after the dwell), so a quick run of steps plans
       only the shown view at each value.
  2. **The camera's value per image:** `State` gets `camera_ev: HashMap<Key, Option<f64>>`,
     kept, cleared and moved exactly where `own_flips` is. `finish_open_checked` gets a closure
     `camera_exposure: impl Fn(&S) -> Option<f64>` after `profile_unusable`, called once on a
     successful open and stored; `core.rs` passes `|session| self.backend.camera_exposure(session)`,
     `finish_open` and the test helper `open_checked` (about line 4894) pass `|_| None` (a test
     that needs a value passes its own).
  3. **The steps are reset when another image is shown** (A-Q2): `exposure_steps = 0` in `go`
     when the position changes (beside `dragged = false`), in `open_folder`, and in `set_files`
     when the current image's path is not the one before. A change of display mode keeps them.
  4. **An image's exposure** (W5-Q3, W5-Q4, W5-Q5): `fn exposure_of(&self, key: Key) ->
     Option<Ev>`; `None` only while auto exposure waits for the image's statistics:
     - a key whose source is not `Source::Raw` (JPEG mode, a JPEG or TIFF file): `Ev::ZERO`;
     - the base: `Off` → 0; `Auto` → by `statistics.get(&key)`: not there → `None`;
       `Unavailable` → 0; `Available(s)` → `auto_exposure(&s)`: `Thirds(t)` → `t` thirds, any
       other → 0; `Camera` → `camera_ev.get(&key)`'s value, 0 without one;
     - the manual steps are added only for the current image's position (`key.0 == cur`; a
       neighbour is prefetched with the mode's value, W5-Q3);
     - the sum: with a base in thirds (off, auto) `Ev::from_thirds(base + steps)`, with the
       camera's value `Ev::from_ev(camera + f64::from(steps) / 3.0)`; both limited to −2..+3 EV
       by `Ev`.
     And `fn shadows_of(&self, key: Key) -> Option<u8>`: `self.shadows` for a `Source::Raw` key,
     `None` otherwise (W5-Q6, A-Q9).
  5. **Every view asks for them:** in `req` and `pan_view` every `JobView` gets `exposure:
     self.exposure_of(key).unwrap_or(Ev::ZERO)` and `shadows: self.shadows_of(key)`, for every
     kind (the ring, the 100 % and pan views too, W5-Q3). `STATISTICS_LEND` keeps `Ev::ZERO`
     and `None`.
  6. **The statistics before the first view while auto is on** (W5-Q4): in `plan_wants`, a key
     with a session (not failed) whose `exposure_of` is `None` gets `Job::Statistics(key)` once,
     in the place of its first render job, and none of its render jobs. A key without a session
     gets its `Open`, **also when a resident view satisfies its requirement**: the check
     "a view whose size is known and is already served needs no session" (about lines
     1603–1613) does not apply to a `Source::Raw` key while auto is on and its statistics are
     not there (else a neighbour drawn from a kept 0 EV view would never get them). So the
     current image and the prefetched neighbours get their statistics before their first view,
     in the plan's order. `pick` needs no new branch (it lends for `Job::Statistics` already);
     `statistics_job` is unchanged. While the current image waits for them, `clipping_job` and
     `focus_job` give `None` (in `pick` they would else go ahead of the plan's statistics job,
     whose place `statistics_job` does not mark); they are asked once the statistics are there.
     A failed statistics job gives `Unavailable` (core.rs about lines 97–106), so 0 EV and
     `AutoNone`, **except** on a session that can no longer be used (a LibRaw session spent by
     an earlier failed render, which the core keeps, `backend.rs` about line 506): after a
     `None`, `Task::Statistics` asks `backend.session_usable(&session)`, and when it is not
     usable `finish_statistics` (a new parameter `usable: bool`, as `finish_render_checked` has)
     drops the session and stores nothing, so the file is opened and asked again. `Frame::statistics`' doc comment (about lines 287–290) adds that while auto
     exposure is on they are made before the first view.
  7. **The views are told apart by their look:** `ViewOut` and `Resident` get `exposure: Ev`
     and `shadows: Option<u8>`, copied as `sharpen` is (`finish_render_checked`'s two `ViewOut`
     literals, about lines 2489 and 2511; the one `Resident` literal in `accept`, about line
     2761; the test helpers `out`, `out_in`, `out_with`, `half_out`, `ring_out` and the other
     `ViewOut` literals of the tests). A view's
     **look** is its `sharpen`, `exposure` and `shadows`; `same_view` becomes "the same view but
     for the look". Every place that compares `sharpen` today compares the whole look instead:
     `serving_of` (a fine view first while fine details is on, then **a view of the
     requirement's look**, then the least dense, then the smallest: the drawn view stays until
     the new one is ready, W5-Q7; and its branch after a failed fine view, about lines
     1453–1457, which orders by sharpening, orders by the look), `exact`, `satisfied` (the
     gesture's exception unchanged: a `Half` view of any look), `remove_inflight` (its `sharpen`
     parameter becomes the look), `replaced_fine` (fine views of the image with another look)
     and `accept`'s fine branch: `out.sharpen == self.sharpening && Some(out.exposure) ==
     self.exposure_of(key) && out.shadows == self.shadows_of(key)` (a key waiting for its
     statistics, `None`, matches nothing). `serves` is unchanged (region and density only), so
     the views of an earlier value are kept as the budget allows (W5-Q7) and one is drawn at
     once when its value comes back.
  8. **The frame's states** (W5-Q9; exported from `lib.rs`):
     - `pub enum AdjustState { Off, Paused, Waiting, Shown }` (`Clone, Copy, Debug, PartialEq,
       Eq`).
     - `pub enum ExposureBase { Manual, Auto(i32), AutoClipped, AutoNone, AutoWaiting,
       Camera(f64), CameraNone, CameraWaiting }` (`Clone, Copy, Debug, PartialEq`): `Manual`
       when the mode is off; `Auto(t)` the rule's thirds; `AutoClipped` for
       `AutoExposure::Clipped`; `AutoNone` for `Unavailable`, `NotRaw` and `NoData`;
       `AutoWaiting` while the statistics are not there; `Camera(v)` the file's own value (not
       limited, for the hover text); `CameraNone` for an opened file without one;
       `CameraWaiting` before the file is opened. For a current key whose source is not
       `Source::Raw` (the view `Paused`): `Manual`, `AutoNone` or `CameraNone` by the mode, and
       the total `Ev::ZERO`.
     - `pub struct ExposureState { pub mode: ExposureMode, pub steps: i32, pub base:
       ExposureBase, pub total: Ev, pub view: AdjustState }` (`Clone, Copy, Debug, PartialEq`);
       `total` is `exposure_of(cur key)` (`Ev::ZERO` while waiting).
     - `Frame` gets `exposure: ExposureState` and `shadows: AdjustState`. `view` and `shadows`
       are worked out in an `exposure_state` and a `shadows_state` beside `sharpen_state`, by
       its rules: `Off` when the adjustment is off (mode `Off` and no steps; shadows `None`) and
       the drawn view of this image is unadjusted (else `Waiting`: the mark stays while an
       exposed or lifted picture is still drawn after turning it off); `Paused` when it is on
       and the current key's source is not `Source::Raw`; `Waiting` during a gesture, while
       auto waits for the statistics, or until the drawn view (of this image, this generation,
       `Status::Ready`) has the wanted `exposure` (or `shadows`); then `Shown`. On `frame_of`'s
       early returns: no image → `Off` (and the base `Manual`, total 0); a failed image → `Off`
       (no view of it will come, so no mark says one is being made; unlike sharpening's
       `Waiting`).
  9. **What does not change:** the focus job, the statistics and the clipping map (A-Q6, §2.1);
     the stop of T328 (a new value of the same image stops nothing, ADR 0027 §4.4); the budgets
     (V8).
  10. **The neighbours and the other mode:** a neighbour's view gets its own mode's value (point
      4) and the shadows; the other display mode of the current image (JPEG) gets `Ev::ZERO` and
      `None` (paused there).
  11. **The core's setters** (`core.rs`): `ViewCore::set_exposure_mode`, `set_shadows`,
      `step_exposure` (returns the `bool`) and `reset_exposure_steps`, each as
      `set_sharpening` (core.rs about lines 425–435): under the lock, `stop_unwanted()` as it
      calls it, and on a change `cv.notify_all` and `wake_ui`; not user requests, nothing
      recorded.
  12. **Cases and what they do** (no new code beyond the points above): a session released for
      a metadata write while its statistics are made keeps the statistics and drops the
      session, and the image waits (`Waiting`) while the release lasts; a new folder or list of
      files while statistics are made drops the result (another generation) and asks again; a
      change of mode while they are made stores them as any statistics; a session lent to
      another job delays the statistics job until it is back. The histogram (`Frame::statistics`)
      stays the unexposed data (A-Q6).
- **Tests to add** (in `state.rs`'s tests with its test backend, unless named otherwise; helpers
  of a chosen look are added as `out_with` for sharpening was):
  - With everything off, every job of the plan has `exposure: Ev::ZERO` and `shadows: None` (fit,
    50 %, 75 %, 100 %, the ring), and the frame's `exposure` is `Manual`, total 0, `Off`.
  (`Ev(n)` below is short for an `Ev` whose `milli()` is `n`; the field is private, T332.)
  - Manual steps: two `step_exposure(1)` give every view of the current image `Ev(667)`; the
    neighbours' views stay at `Ev::ZERO`; `go` to another position resets the steps (the frame
    shows 0 steps) and so do `open_folder` and a `set_files` that changes the current path; a
    `set_files` that keeps it and `set_mode` keep them.
  - The limits: from 0, nine `step_exposure(1)` are accepted and the tenth returns `false` with
    the total `Ev(3000)`; from 0, six `step_exposure(-1)` are accepted and the seventh returns
    `false`; in JPEG mode and on a plain JPEG file `step_exposure` returns `false`.
  - Auto: with the mode `Auto` and a RAW image opened, the plan holds `Job::Statistics` for it
    and no render of it; after `finish_statistics` with statistics whose rule gives `Thirds(3)`,
    its views ask for `Ev(1000)` (for example `RawData` statistics with every pixel of every
    channel in bin 160: `v = 161/256`, `L ≈ 0.353`, `3e ≈ 3.50`, so `Thirds(3)`); a prefetched
    neighbour gets its own statistics first too, also when its session was dropped and a 0 EV
    view of it is resident (it is opened); while the current image waits, no clipping or focus
    job is picked;
    `Unavailable` gives `Ev::ZERO` and `AutoNone`; clipped statistics give `AutoClipped` and
    `Ev::ZERO`; one step on top gives `Ev(1333)` and the frame `Auto(3)`, steps 1.
  - Camera: with the mode `Camera`, an image opened with a camera value of −0.51 asks for
    `Ev(-510)` and the frame's base is `Camera(-0.51)`; one step gives `Ev(-177)`; without a
    value `CameraNone` and `Ev::ZERO`; a value of −2.7 is held as `Ev(-2000)` with the base
    `Camera(-2.7)`.
  - Shadows: `set_shadows(Some(50))` gives `shadows: Some(50)` to every RAW view, `None` in JPEG
    mode and for a TIFF file, where the frame's `shadows` is `Paused`; `set_shadows(Some(150))`
    holds 100; the same value again returns `false`.
  - The swap (W5-Q7): with a view at 0 EV resident and drawn, a step keeps drawing its id with
    `exposure.view == Waiting` and plans a render at the new value; once it is accepted the
    frame draws the new id and the state is `Shown`; a step back draws the kept 0 EV view at
    once and plans nothing.
  - Turning the mode off while only an exposed view is resident: `Waiting` until an unexposed
    view is drawn, then `Off`; the same for the shadows.
  - Two jobs differing only in exposure in flight at once: accepting one removes only its own
    inflight entry.
  - With fine details on, a fine view drawn and a budget that holds one fine view, a step: the
    new fine render is not refused and the old fine view is evicted when it is accepted.
  - A Frame early return: with no image, and for a failed image with every adjustment on,
    `exposure.view` and `shadows` are `Off`.
  - The dwell after a change: right after a step, before the dwell, the plan holds only the
    shown view at the new value; after the dwell the ring or the 100 % view and the neighbours
    follow, and with fine details on the fine view last.
  - A spent session: with a test backend whose `statistics` gives `None` and whose session is
    not usable, auto exposure stores no statistics, drops the session and opens the file again;
    with a usable one, `Unavailable` is stored.
  - `tests/engine.rs`: with the core running, `step_exposure(1)` **after** `open_folder` (it
    returns `false` with no current image, and `open_folder` resets the steps) gives a fit view whose
    `ViewOut::exposure` is `Ev(333)` (the engine's fake backend ignores the exposure; only the
    view's field is asserted), and `set_shadows(Some(100))` one whose pixels differ from the view
    without shadows.
- **Known and accepted (W5-Q7, W5-Q4):** every change renders the shown view again (about
  0.3 s for 61 Mpix at fit, 1.2 s with fine details on), and the neighbours after it. While auto
  is on, a step to an image that was not prefetched waits for its statistics before its first
  view: the owner's W5-Q4 choice after T331's timings (2026-11-25, ADR 0027 §4.4): the step
  waits (open, statistics and a half-size render 625–794 ms on the 61 Mpix ARW files, the
  statistics 170–220 ms of it); no first view at 0 EV replaced by the auto-exposed one. A kept
  view of another value may be
  drawn meanwhile, marked `Waiting`.
- **Not in this task:** the compare's shared steps, the keys, the marks and the reset (T335);
  the shadows' amount in the settings (T334).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text). No new
  dependency. An extra code review follows (more than one crate).
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added; waits for T332. Cold read: every cited line found; fixed: a RAW key waiting for its statistics is opened even if a kept view satisfies it (else a neighbour never gets them); clipping and focus jobs wait while the current image's statistics are due; `f64::from(steps) / 3.0`; the unlimited total defined and the −2.7 case accepted; `accept`'s fine branch spelled out; `stop_unwanted` in the setters; the `fine_failed` branch of `serving_of`; `CameraWaiting` and the base of a paused key; the mode change keeps the steps; the `ViewOut`, `Resident` literals and `open_checked`; a bin for `Thirds(3)`; the engine test's order; the cases of point 12 stated. Chain review (T331–T335, 2026-11-25): the APIs line up; fixed here: the setters set `since` (the dwell after a change, W5-Q7); a spent session's failed statistics reopen the file instead of storing `Unavailable` for the folder; a failed image's states `Off`. Fixed in the others: T331 times the open too (W5-Q4 is open + statistics + render); T332 declares the public modules; T334 and the plan: `main` waits for T335; T335: where leaving the preview resets the steps, the compare with a non-RAW first side and the unexposed overlays as known limits, `ev_text`'s rounding in `f64`, "no usable RAW statistics"; the plan's W5 row: the latency series and the hand check's items |
| 2026-11-25 | Blocked | The owner's W5-Q4 choice after T331's timings written into "Known and accepted" (the step waits for the statistics; ADR 0027 §4.4); point 6 already says so, unchanged. Still waits for T332 |
| 2026-11-25 | Open | Set to Open by the reviewer on the owner's word after T332 was Done (aa15dc1). T332 moved lines: every cited line of `state.rs` (+8 to +31), `core.rs` (`Task::Statistics` now about lines 97–106, `set_sharpening` 425–435) and `backend.rs` (`session_usable` 506) updated to the tree at aa15dc1; T332's API as this task names it; nothing else changed |
| 2026-11-26 | Ready for review | Implemented the RAW exposure modes, manual steps, shadows, statistics scheduling, cache matching and frame states. Added 13 state tests and two worker integration tests. Independent diff review found no blocking issues. Updated cache comments and removed redundant histogram scans from scheduling checks. |
| 2026-11-26 | Done | Reviewed: no bug. The reviewer's own review and an independent review agent (looks compared everywhere `sharpen` was, `stop_unwanted` unchanged, statistics before the first view, `usable: false` with a release and a new generation, `pan_view` and the half-size branch equivalent) found only missing doc comments; the reviewer added them (the new `pub` fields of `ViewOut`, `Resident` and `Frame`, the variants of `ExposureMode`, `AdjustState`, `ExposureBase`, the fields of `ExposureState`, the four setters in `core.rs`, `finish_statistics`' `usable`) and set `docs/test-inventory.md`'s header to the run (2794/98) instead of the agent's added sentence. Accepted: the spent-session test is in `tests/engine.rs` (through `core.rs`) rather than `state.rs`. `.\tools\check.ps1` all OK, 2794 passed, 98 ignored, 0 failed, no clippy warnings. No leftovers. No `CHANGELOG.md` line (nothing the user sees until T335). The owner's hand check follows after T335 |
