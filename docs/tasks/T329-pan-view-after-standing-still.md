# T329. ADR 0027: the pan view also once the view has stood still — Open

- **Why:** decided by the owner on 2026-11-25 after T328's hand check (ADR 0027 §4.4, "The pan
  view without a drag", alternative C). Since T326 the pan view (a scaled view at the shown scale
  over twice the visible region, between 50 % and 100 %) is asked for only once the current image
  has been dragged, so the first drag on every image stands still until it pauses. T328 now stops
  the render of an image that is left, so the pan view may be asked for without a drag too, but
  only once the view has **stood still for 1 s**, and then **last in the plan**: quick stepping
  never starts it, and the neighbours are prefetched first.
- **What the reviewer checked (2026-11-25):** in `crates/pw-viewcore/src/state.rs`:
  - `Config` (about line 19) has `dwell` (150 ms) and `gesture_pause`; `Config::default()` (about
    line 35) sets them. Every other `Config { … }` in the workspace (`core.rs` tests, `state.rs`
    tests, `tests/engine.rs`, `pw-loupe`'s `main.rs` about lines 13196 and 13211) ends with
    `..Config::default()` or `..config.clone()`, so a new field needs no change there.
  - `State::next_deadline` (about line 3134) tells the worker how long it may sleep before the
    plan changes by itself (the dwell, the gesture pause, the pan's pause); the worker sleeps at
    most that, and at most `IDLE_WAIT` (1 s, `core.rs`).
  - `set_files` with a list equal to the one held calls `self.go(current)` and returns (about
    line 969), so it resets `since` as `go` does.
  - `State` (about line 541) has `since: Instant` (when the image became current), `gesture_at`,
    `pan_at` and `dragged` (T326). The test helper `state()` (about line 3183) sets `since` one
    second back, so a field reset where `since` is and **not** set back by the helper keeps every
    existing test at "not yet still".
  - `set_zoom` (about line 758) stores the zoom on every call; `zoom_by` and `toggle_zoom` call it.
    `set_view` (about line 798) returns early when nothing changed. `go` (about line 904) resets
    `since` on every call; `open_folder` (about line 926) and `set_files` (where the current path
    changed, about line 1113, the place that resets `since` and `dragged`).
  - `State::wanted` (about line 1135): after the dwell, the other view of the current image, the
    forward neighbours, the other display mode, then the `match self.pan_view(ck)` (the pan view
    when `Some` and `self.dragged && useful && (full || !self.in_pan())`; the 100 % ring when
    `None`), then the backward neighbour (`self.neighbour(cur, 1, !forward)`), then the fine
    view last.
  - `State::pan_view` (about line 1233) has no limit on the source: JPEG and TIFF files and JPEG
    mode get a pan view as RAW does.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **Model:** Opus — the plan's state and its timing (the kind of T326 and T328).
- **What:**
  1. A new field of `Config`, `pan_view_dwell: Duration`, with a doc comment: without a drag, the
     pan view is asked for once the current image's view has stood still this long (ADR 0027
     §4.4, owner 2026-11-25). `Config::default()` sets it to 1 s (`Duration::from_secs(1)`).
  2. A new field of `State`, `still_since: Instant`, with a doc comment: when the current image's
     view last changed (the image became current, a new folder or file list, a zoom, a
     `set_view` that changed something). `State::new` sets it to `Instant::now()`.
  3. It is set to `Instant::now()` in `go` (every call that makes a position current, where
     `since` is reset; so also `set_files` with an equal list, which calls `go`), `open_folder`,
     `set_files` (where the current path changed, where `since` is reset), `set_zoom` (every call
     that stores a zoom, so `zoom_by` and `toggle_zoom` too) and `set_view` (only when it returns
     `true`: after its early returns, where `gesture_at` is set). `pan`, `set_mode`,
     `set_sharpening` and `frame_of` do not change it (a drag sets `dragged`, which asks for the
     pan view already).
  4. In `State::wanted`, the `Some` arm of `pan_view`:
     - when `self.dragged` holds, unchanged: the pan view is added in its place today (before the
       backward neighbour), with `useful && (full || !self.in_pan())`;
     - when `self.dragged` does not hold, the pan view is added only when
       `self.still_since.elapsed() >= self.config.pan_view_dwell` and `useful`, and **after the
       backward neighbour, before the fine view**. The full plan (`full`) does **not** skip the
       pause: inside it, neither `wanted(false)` nor `wanted(true)` holds the pan view, so one not
       yet wanted is neither protected nor kept by `accept`. (`in_pan()` cannot hold without a
       drag.) The `None` arm (the 100 % ring) is unchanged.
  5. In `State::next_deadline`, when `self.cur` is `Some` and `self.dragged` does not hold, also
     `consider(self.still_since + self.config.pan_view_dwell)`, so the worker wakes when the
     pause ends instead of up to `IDLE_WAIT` later; its doc comment names the pause.
  6. `State::req`, `pan_view`, `satisfied`, `serving_of`, `stop_unwanted`, the budgets,
     `PAN_VIEW_SIZE` and `PAN_VIEW_KEEP` are not changed.
  7. The comments of `Want::Ring` (about line 366) and of the `wanted` code that adds the pan view
     say that it is asked for after a drag, or without one once the view has stood still for
     `Config::pan_view_dwell`, then last (ADR 0027 §4.4, owner 2026-11-25).
- **Tests to add** (in `state.rs`; rule 5; set `s.still_since` back with
  `Instant::now() - Duration::from_secs(2)` to pass the pause, as the tests set `since` back):
  - `Config::default().pan_view_dwell` is 1 s.
  - At 0.512 and 0.75, the shown view resident (`shown_resident`), not dragged, `still_since`
    set back past the pause: `wanted(false)` and `wanted(true)` hold `(k(10), Want::Ring)`, and in
    `wanted(false)` it comes after `(k(9), Want::Shown)` (the backward neighbour); the plan's
    scaled renders of image 10 include the pan view's region (`pan_region`).
  - The same with `dragged` true (and `still_since` not set back): `(k(10), Want::Ring)` comes
    before `(k(9), Want::Shown)` in `wanted(false)`, as today.
  - Not dragged, `still_since` just set (inside the pause), `since` past the dwell: no
    `(k(10), Want::Ring)` in `wanted(false)` or `wanted(true)`.
  - Each of `go` to another image, `go` to the same position, `open_folder`, `set_files` with
    another path current, `zoom_by`, `toggle_zoom`, `set_zoom` and a `set_view` that changed
    something resets `still_since` (set it back first, then check
    `still_since.elapsed() < pan_view_dwell`); `set_files` with a changed list where the current
    path stays (`without(20, &[3]), 9`, as T326's test does), a `set_view` that changed nothing,
    `pan`, `set_mode` and `set_sharpening` keep it. Use `shown_resident` or a known size, since
    `zoom_by` does nothing at fit before the size is known.
  - `next_deadline`: with `state(1024)` (not dragged, `since` past the dwell, `still_since` just
    set) it is `Some` and above half of `pan_view_dwell` and at most `pan_view_dwell`; with
    `still_since` set back past the pause and no gesture or pan under way, it is `None`; with
    `dragged` true and `still_since` just set, the pause is not considered (`None` as well).
  - After `zoom_by` at 0.75 with `still_since` set back before it, and the gesture's pause passed
    (`gesture_at` set back), not dragged: no pan view in `wanted(false)`; after `still_since` is
    set back again, the pan view at the new scale is planned.
  - The existing T326 test `without_a_drag_no_pan_view_is_wanted_between_50_and_100` and every
    other existing test are not changed and must still pass (the helper does not set
    `still_since` back). Run them before and after; any that then fails is stopped at and
    reported by name (rule 10). The core test with `set_view(Zoom::Scale(0.75), …)` in `core.rs`
    (about line 1066, `every_view_is_converted_to_the_display_after_its_reduction_and_the_focus_map_is_not`)
    reads its counts about 0.2 s after the scaled view, before the pause, and `tests/engine.rs`
    `a_free_zoom_level_is_rendered_scaled_and_sharp` (about line 202) checks the first drawn view
    at 70 %: if either turns out flaky, stop and report.
- **Not in this task:** the pan view's size; the compare's second core that counts a new held
  image as dragged (`known-limits.md`, stays); a pause set by the user (the 1 s is the core's and
  is tuned in the hand check); a window resize as a change of the view; any change to T328's stop.
  No `CHANGELOG.md` line from the agent. After Done the owner hand-checks the first drag after
  standing still for 1 s at 51–59 % and at one more level between 50 % and 100 %, and quick
  stepping there, and measures `latency.ps1` at 51.2 % and 64 % with the clipping marks off and on
  (V1, V4); the result goes into ADR 0027 §4.4.
- **Rules:** `AGENTS.md`, `docs/tasks/README.md`, `docs/TESTING.md`.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Open | Added after T328's hand check: alternative C chosen by the owner (ADR 0027 §4.4, "The pan view without a drag") |
| 2026-11-25 | Open | Cold read (a fresh agent, 2026-11-25): doable with the named file, line references right, the plan's order and the helpers as assumed. Changed after it: `next_deadline` considers the pause (point 5, a test), so the worker does not wake up to 1 s late; `set_files` with an equal list resets the pause through `go` (the "keeps it" test uses a changed list); the full plan does not skip the pause, said outright; `pw-loupe`'s two `Config`s named (no change needed); the two real-time tests at risk named |
