# T350. ADR 0027 W6: the white balance's mode in the view core's state — Open

- **Why:** ADR 0027 §2.1, §3.5 and W6-Q3, W6-Q4, W6-Q6, W6-Q7, W6-Q8 (answered by the owner
  2026-11-28, ADR 0027 §4.6). T349 made the render path carry a balance (`JobView::wb`, a `Wb`)
  and gave the backend `white_balance_info`, `auto_white_balance` and `grey_white_balance`. This
  task makes the state ask for a balance: the mode (as shot, auto, daylight, a preset of the
  file), the info read once at the open, the auto balance made **before the first view** while
  auto is on, the views told apart by their balance in the cache, and the frame's state for the
  shell's mark and menu. **The click on gray is T352's** (its job, its result, its note), so
  `WbMode` has a place for `Grey` here but nothing sets it yet. The third task of W6's chain
  T348 → T349 → **T350** → T351 → T352 → T353. Waits for T349. It follows T333 (exposure) and
  T346 (the channel) place by place.
- **What the reviewer checked (2026-11-28, and by a cold read the same day):**
  - `crates/pw-viewcore/src/state.rs`: `Look` (about line 481: `sharpen`, `exposure`, `shadows`,
    `channel`) with `look()` on `Resident`, `ViewOut` and `JobView`; every comparison of looks
    (`serving_of`, `exact`, `satisfied`, `replaced_fine`, `remove_inflight`, `gesture_stand_in`)
    goes through it; `accept`'s fine branch (about lines 3018–3031: it does not decide whether a
    fine view is taken, it lets the new fine view evict the image's protected fine views of
    another look) compares `sharpen`, exposure, shadows and channel directly; `exposure_of(key)
    -> Option<Ev>` (`None` only while auto exposure waits for the statistics) and `shadows_of`;
    the exposure's per-key maps (`camera_ev`, kept, cleared and moved where `own_flips` is) and
    `statistics: HashMap<Key, ImageStatistics>` with `finish_statistics` (about line 2889, which
    takes `ms` and a `usable: bool`); `Stats` (about lines 417–418: `statistics` and
    `statistics_ms`); `plan_wants` (about line 1846) with `waiting_for_auto` (about lines 1857 and
    1868: the statistics job before the first render of a key, also when a resident view
    satisfies it); `clipping_job` and `focus_job` (about lines 1938 and 1996, which check
    `waiting_for_auto`); `pick` (about lines 2494–2550: lends the session for a `Job::Statistics`,
    the interactive worker only for the current image); `go`, `open_folder`, `set_files` (about
    line 1258; where the current image changes and `exposure_steps` is reset); `set_mode` (the
    display mode, the same image); `adjust_state` (about line 3288: `Shown` also needs
    `frame.exposure.base != AutoWaiting`, which the white balance's view inherits; accepted),
    `exposure_state`, `shadows_state`, `channel_state`; `Frame` (line 341, derives `Clone, Debug,
    PartialEq`) and `frame_of`'s literal and early returns; `finish_open_checked` with the
    closures `profile_unusable` and `camera_exposure`, called by `finish_open`, by the test helper
    `open_checked` and by the exposure tests (about lines 8790 and 8841); `own_flips` and
    `told_flip`; the test helper `o_job` (about line 4443); the `STATISTICS_LEND` constant (a
    `JobView` booked for the short loan of a session; its doc comment says who shares it).
  - `crates/pw-viewcore/src/core.rs`: the worker's `Task::Open`, `Task::Render`,
    `Task::Statistics` (asks `session_usable` after a `None` and drops an unusable session),
    `Task::Clipping`, `Task::Focus`; `ViewCore::set_shadows`, `set_channel`,
    `set_exposure_mode`, `step_exposure` (each through `update_adjustment`, about line 474, which
    wakes nothing when its closure returns `false`; not user requests, nothing recorded).
  - `crates/pw-viewcore/src/lib.rs`: the exports of the states (line 47: `AdjustState`,
    `ExposureState`, `ExposureBase`, `ExposureMode`).
  - `crates/pw-viewcore/tests/engine.rs`: the backend `Fake` (about line 31), which a full-run
    test overrides.
  - `crates/pw-loupe/src/loupe_text.rs`: its test helpers `frame` and `tiff_frame` build `Frame`
    literals and need the new field; no other crate builds a `Frame`, `JobView`, `ViewOut` or
    `Resident`.
  - `Key` is `(usize, Source)` (the position in the list and the display mode); `cur` the current
    position. The state's tests use `State<u32>` without a backend: values reach it through
    `finish_open_checked`'s closures and through `finish_balance` directly.
- **Crates:** `pw-viewcore`, `pw-loupe` (test helpers only). **Files:**
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/core.rs`,
  `crates/pw-viewcore/src/lib.rs`, `crates/pw-viewcore/tests/engine.rs`,
  `crates/pw-loupe/src/loupe_text.rs` (the new `Frame` field in its test helpers, off; nothing
  else), `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — a state machine in the view core and more than one crate (README, "Model").
- **What:**
  1. **The mode the shell gives** (A-Q2, W6-Q6): a public enum `WbMode` (`Clone, Debug,
     PartialEq, Eq, Default` with `Shot` the default): `Shot`, `Auto`, `Daylight`,
     `Preset(String)` (a preset of the file by its name, W6-Q4) and `Grey` (set only by T352's
     click, never by the shell). `State` gets `wb_mode: WbMode`. A new folder or list does not
     change it. Setters, each returning whether something changed (and setting `since =
     Instant::now()` on a change, as a zoom step does; W6-Q7: the shown view comes first, the
     rest after the dwell):
     - `set_white_balance(mode: WbMode)`: setting `Grey` from outside is refused (`false`);
       `Shot` is as shot (when the mode was `Grey`, T352 also forgets the grey);
     - `reset_white_balance()`: mode `Shot`.
  2. **What a file gives, once** (W6-Q4): `State` gets `wb_info: HashMap<Key, WbInfo>`, kept,
     cleared and moved exactly where `camera_ev` is; `finish_open_checked` gets a closure
     `white_balance: impl Fn(&S) -> WbInfo` after `camera_exposure`, called once on a successful
     open and stored; `core.rs` passes `|session| self.backend.white_balance_info(session)`;
     `finish_open`, the test helper `open_checked` and the two exposure tests that call
     `finish_open_checked` directly (about lines 8790 and 8841) pass `|_| WbInfo::default()` (a
     test that needs a value passes its own).
  3. **An image's balance** (W6-Q3, W6-Q4): `fn wb_of(&self, key: Key) -> Option<Wb>`; it returns
     `Some(Wb::Shot)` **first of all** for the mode `Shot` and for a key whose source is not
     `Source::Raw` (JPEG mode, a JPEG or TIFF file), before any map is looked at, so as-shot
     stepping costs nothing; `None` only while auto waits for the image's auto balance:
     - `Auto` → by `wb_auto.get(&key)` (point 4): not there → `None`; `Unavailable` → `Shot`;
       `Available(wb)` → `wb`;
     - `Daylight` → `wb_info`'s `daylight` (`Shot` without one, or while the info is not there);
     - `Preset(name)` → the preset of that name in `wb_info.get(&key)` (`Shot` when the file has
       none of that name, or while its info is not there yet);
     - `Grey` → `Shot` in this task (T352 gives the click's balance for the current image's key and
       `Shot` for every other, a neighbour is prefetched as shot, W6-Q3).
  4. **The auto balance, made before the first view** (W6-Q1, W6-Q7): `State` gets `wb_auto:
     HashMap<Key, AutoWb>` with `pub(crate) enum AutoWb { Available(Wb), Unavailable }`, kept,
     cleared and moved as `statistics` is; a new `Job::Balance(key)` and `Task::Balance
     { generation, key, session }` in `core.rs`, whose worker calls `backend.auto_white_balance`,
     asks `session_usable` after a `None` as `Task::Statistics` does, and calls a new
     `finish_balance(generation, key, session, result: Option<Wb>, usable: bool, ms: f64)` that
     stores `Available` for `Some` and `Unavailable` for `None` (an unusable session is dropped and
     nothing stored, so the file is asked again), and adds to `Stats` the fields `balances: u64`
     and `balance_ms: Vec<f64>` as `statistics` and `statistics_ms` are (so `ms` has a home). In
     `plan_wants` a key with a session (not failed) whose `wb_of` is `None` gets `Job::Balance(key)`
     once, in the place of its first render job and none of its render jobs (**after** a
     `Job::Statistics` of the same key when `exposure_of` is `None` too: `plan_wants` pushes the
     statistics job and goes on, so the balance job follows on the next plan; each is asked in
     turn); a key without a session gets its `Open`, also when a resident view satisfies its
     requirement, **only while `wb_of` is `None`** (`waiting_for_auto` is extended to say so), so
     as-shot stepping plans exactly as it does today. While the current image waits for either,
     `clipping_job` and `focus_job` give `None`. `pick` lends the session for a `Job::Balance` as
     for a `Job::Statistics` (the interactive worker only for the current image) and books it
     under `STATISTICS_LEND`, whose doc comment is updated to name the balance.
  5. **Every view asks for it:** in `req` and `pan_view` every `JobView` gets `wb:
     self.wb_of(key).unwrap_or(Wb::Shot)`, for every kind (the ring, the 100 % and pan views too,
     W6-Q3). `STATISTICS_LEND` keeps `Wb::Shot`.
  6. **The views are told apart by their look** (W6-Q7): `ViewOut` and `Resident` get `wb: Wb`,
     copied where `channel` is (`finish_render_checked`'s `ViewOut` literals, `accept`'s
     `Resident`, the test helpers, `o_job` copying `o.wb`); `Look` gains `wb`, so every comparison
     through `look()` includes it; `accept`'s fine branch also asks `Some(out.wb) ==
     self.wb_of(key)` (a key waiting for its auto balance, `None`, matches nothing), which only
     decides whether the new fine view may evict the image's protected fine views of another
     look; `serves` is unchanged; `gesture_stand_in` already prefers the wanted look. The views of
     an earlier balance are kept as the budget allows (V8) and one is drawn at once when its
     balance comes back.
  7. **The frame's state** (W6-Q8; exported from `lib.rs` with `WbMode`): `pub enum WbBase { Shot,
     Auto, AutoNone, AutoWaiting, Daylight, DaylightNone, Preset(String), PresetNone(String),
     Grey }` (`Clone, Debug, PartialEq, Eq`). By the mode and the current key:
     - mode `Shot` → `Shot`; `Grey` → `Grey` (nothing sets it in this task);
     - `Auto` → `Auto` when the auto balance is available, `AutoNone` for `Unavailable`,
       `AutoWaiting` while it is not there;
     - `Daylight` → `Daylight` while the key's `wb_info` is not there yet (the view `Waiting`, so
       no "none" mark flashes before the open has finished), then `Daylight` when the file has one
       and `DaylightNone` when it has none; `Preset(n)` the same with `Preset(n)` and
       `PresetNone(n)`;
     - for a current key whose source is not `Source::Raw` the base of the mode as if the file
       were open (`Auto` → `AutoNone`, `Daylight` → `DaylightNone`, `Preset(n)` → `PresetNone(n)`,
       `Shot` → `Shot`) and the view `Paused`.
     `pub struct WhiteBalanceState { pub mode: WbMode, pub base: WbBase, pub wb: Wb, pub view:
     AdjustState, pub presets: std::sync::Arc<[WbPreset]>, pub has_daylight: bool }` (`Clone, Debug,
     PartialEq, Default` with an `off()` constructor the `loupe_text.rs` helpers use): `mode` is
     always the state's `wb_mode`, also on `frame_of`'s early returns; `wb` is `wb_of(cur key)`
     (`Wb::Shot` while waiting; the hover text reads its multipliers); `presets` the current
     image's file list from its `wb_info` (the `Arc`, not a copy, so the frame clones cheaply;
     empty before its info is there); `has_daylight` whether the file has LibRaw's daylight. T352
     adds the click's fields. `Frame` gets `white_balance: WhiteBalanceState`; its `view` is worked
     out beside `channel_state` by `adjust_state`'s rules: `Off` when the mode is `Shot` and the
     drawn view of this image has as-shot balance (else `Waiting`: the mark stays while a balanced
     picture is still drawn after turning it off), `Paused` when it is on and the current key's
     source is not `Source::Raw`, `Waiting` during a gesture, while auto waits, while `wb_info` is
     not there for a mode that needs it, or until the drawn view (of this image, this generation,
     `Status::Ready`) has the wanted `wb`; then `Shown`. On `frame_of`'s early returns: no image
     → `Off`, base by the mode as for a non-RAW key but `Off`, nothing listed; a failed image →
     `Off`.
  8. **What does not change:** the focus job, the statistics and the clipping map (A-Q6, §2.1,
     W6-Q5: the histogram and the overlays stay as shot; auto exposure is not worked out again for
     a balance); the stop of T328 (a new balance of the same image stops nothing, as for the
     exposure); the budgets (V8).
  9. **The neighbours and the compare** (W6-Q3): a neighbour's view gets its own mode's balance
     (its auto balance made as point 4 says, its preset looked up by name in its own info,
     daylight its own); the other display mode of the current image gets `Wb::Shot` (paused
     there). The compare's second side is its own `ViewCore` with its own state: the shell sets
     the same mode on both (T351).
  10. **The core's setters** (`core.rs`): `ViewCore::set_white_balance(mode: WbMode) -> bool` and
      `reset_white_balance()`, each as `set_channel` (through `update_adjustment`, `stop_unwanted()`,
      `notify_all` and `wake_ui` on a change; not user requests, nothing recorded).
  11. **Cases and what they do** (no new code beyond the points above): a session released for a
      metadata write while an auto balance is made keeps the result and drops the session, and
      the image waits (`Waiting`) while the release lasts; a new folder or list while it is made
      drops the result (another generation) and asks again; a change of mode while it is made
      stores it as any result; a session lent to another job delays the job until it is back; a
      failed auto balance (`None`) gives `Unavailable` and the mark "WB auto: none" (T351), except
      on an unusable session (dropped, asked again).
- **Tests to add** (in `state.rs`'s tests with `State<u32>`, unless named otherwise; values come
  through `finish_open_checked`'s closures and `finish_balance`; helpers of a chosen look are
  added as `out_with` for sharpening was; `Wb::Mul { r, b }` below is written `Mul(r, b)`):
  - With everything off, every job of the plan has `wb: Wb::Shot`, `wb_of` is `Some(Shot)`
    without a map lookup (a test with empty maps), and the frame's `white_balance` is `Shot`, base
    `Shot`, `Off`.
  - Auto: with the mode `Auto` and a RAW image opened, the plan holds `Job::Balance` for it and no
    render of it; after `finish_balance(.., Some(Mul(2000, 1500)), ..)`, its views (fit, 50 %,
    75 %, 100 %, the ring) ask for it and the frame's base is `Auto`; `None` gives `Shot` and
    `AutoNone`; a prefetched neighbour gets its own balance job first too, also when its session
    was dropped and a shot view of it is resident (it is opened), and **with the mode `Shot` the
    same neighbour is planned exactly as before** (no open for a resident view); while the
    current image waits, no clipping or focus job is picked; with auto exposure on as well the
    statistics job comes first and the balance job after it, before the first render; a balance
    job whose session came back unusable drops the session and stores nothing.
  - Daylight and presets: after an open with a `WbInfo` of presets "Daylight" (`Mul(2415,
    1568)`) and "Shade" and a daylight `Mul(2300, 1600)`, the mode `Daylight` asks for it; the mode
    `Preset("Shade".into())` asks for Shade's; a neighbour whose info lacks "Shade" is asked as
    shot and its frame (when it is the current image) says `PresetNone("Shade")`; a file without a
    daylight says `DaylightNone`; before the open has finished the frame's base is `Daylight` or
    `Preset(n)` with the view `Waiting`, not a "none"; the frame lists the presets in the file's
    order and `has_daylight` is right.
  - `set_white_balance(WbMode::Grey)` returns `false`; the same mode again returns `false`;
    `reset_white_balance` returns to `Shot`; modes `Auto`, `Daylight` and `Preset` survive `go`,
    `open_folder` and `set_files` (a new folder or list does not change the mode).
  - JPEG mode and a TIFF file: every mode asks for `Wb::Shot`, the frame's `view` is `Paused`
    (and `Off` with the mode `Shot`), the bases are as the point 7 table says.
  - The swap (W6-Q7): with a shot view resident and drawn, a change of mode keeps drawing its
    id with `view == Waiting` and plans a render at the new balance; once it is accepted the
    new view is drawn and `view` is `Shown`; a press back to `Shot` finds the kept view at once.
  - The gesture's stand-in prefers a view of the wanted balance; a late fine view of an old
    balance is accepted but does not evict the drawn fine view of the wanted balance.
  - `tests/engine.rs`: the `Fake` backend gets an `auto_white_balance` that counts its calls and
    gives a `Wb`; a full run through `ViewCore` with the mode `Auto`: the current image's first
    view is rendered with that balance and the balance job ran before its first render (the
    counter says so).
- **Not in this task:** the click on gray (T352); the keys, the menu, the marks and their texts
  (T351); the Settings page (W6-Q6 gives no setting).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text: the frame
  carries values and names the file gives, not sentences). No new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added (as T350 with the click); waits for T349 |
| 2026-11-28 | Blocked | Cold read and chain review: the click split out to T352; added the key-first `wb_of` for `Shot`, the `waiting_for_auto` extension, `Stats` fields for the balance's time, `finish_open_checked`'s other callers, the `WbBase` rules before the info is there and for a non-RAW key, `Arc` for the preset list, the exports and derives, the engine test's `Fake` |
| 2026-11-28 | Open | T349 done; set to Open by the reviewer (the cold read and the chain review are above) |
