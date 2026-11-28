# T352. ADR 0027 W6: the click on gray in the view core's state — Blocked

- **Why:** ADR 0027 §3.5 and W6-Q2, W6-Q3, W6-Q8 (answered by the owner 2026-11-28, ADR 0027
  §4.6). T349 gave the backend `grey_white_balance(session, area)` and the pure `grey_area`;
  T350 gave the state the white balance's mode, with a place for `Grey` that nothing sets. This
  task makes the click work in the view core: the request from a point of the view, the job that
  measures it, its result (the mode `Grey` for that image, or a note that the area cannot be
  measured), and the frame's fields the shell needs (`pick`, `can_pick`). The fifth task of W6's
  chain T348 → T349 → T350 → T351 → **T352** → T353. Waits for T350 (and so for T349) and T351 is
  not needed.
- **What the reviewer checked (2026-11-28, and by a cold read and a chain review the same day):**
  - `crates/pw-viewcore/src/state.rs`: `Job` (line 550: `Open`, `Render`, `Statistics`,
    `Clipping`, `Focus`; `Job::key`, line 562, and every exhaustive match on `Job` and `Task`
    are in `state.rs` and `core.rs`); `plan_wants` (about line 1846) and `pick` (about lines
    2381–2550): `statistics_job` is inserted **at index 0** of the plan for the current image and
    `pick` lends the session for it, the interactive worker only for the current key
    (`key != ck` is skipped, about line 2411); `STATISTICS_LEND` (the `JobView` booked for a
    session lent to a statistics, balance or click job; its doc comment says who shares it);
    `finish_statistics` (about line 2889: a `None` on a session that can no longer be used drops
    the session and stores nothing, so the file is opened and asked again; a session released for
    a metadata write is dropped, `return_session` at about line 2835 handles it); `go`,
    `open_folder`, `set_files` (about line 1258, with the `to` map from old to new positions),
    `set_mode`; `scale_of` (about line 898, which uses `self.view_px`); `geometry::visible`,
    `dest` and `ImageSize::to_sensor`; `frame_of`, `Frame::scale`, `Frame::size`; T350's
    `wb_of`, `wb_info`, `WhiteBalanceState`, `WbBase`, `WbMode`.
  - `crates/pw-viewcore/src/core.rs`: `Task::Statistics`/`Task::Balance` (T350) and
    `ViewCore::update_adjustment` (about line 474, which wakes nothing when its closure returns
    `false`: a refused click still changes the note, so `pick_grey` notifies and wakes on every
    outcome).
  - `crates/pw-viewcore/src/geometry.rs` after T349: `grey_area(size, point, scale) ->
    Option<Rect>` with the point in the upright image's edge coordinates.
  - `crates/pw-viewcore/tests/engine.rs`: the backend `Fake` (T350 gave it an
    `auto_white_balance`).
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/state.rs`,
  `crates/pw-viewcore/src/core.rs`, `crates/pw-viewcore/src/lib.rs`,
  `crates/pw-viewcore/tests/engine.rs`, `docs/test-inventory.md` (rule 12), and this task's own
  file (status and log). (`WhiteBalanceState` has `off()`, so the helpers in `pw-loupe`'s
  `loupe_text.rs` need no change; if they do, add that file and say so in the log.)
- **Model:** Opus — a state machine in the view core (README, "Model").
- **What:**
  1. **What the state holds:** `State` gets `grey: Option<(usize, Wb)>` (the position of the image
     it was measured on and its balance), `pending_grey: Option<(Key, Rect)>` (the request being
     measured: the key and the sensor area) and `pick_note: PickState` with `pub enum PickState {
     Idle, Measuring, Unusable, TooBrightOrDark }` (`Clone, Copy, Debug, PartialEq, Eq, Default`,
     `Idle` the default; exported from `lib.rs`). `wb_of` for the mode `Grey` gives the grey's
     balance for the current image's key (`key.0 == cur` and `grey`'s position is `cur`) and `Shot`
     for any other key (a neighbour is prefetched as shot, W6-Q3).
  2. **The request** (W6-Q2): `State::pick_grey(point: [f64; 2], view_px: [u32; 2]) ->
     PickResult` with `pub enum PickResult { Started, Outside, Unusable }` (exported); `point` is
     the clicked point in physical view pixels from the view's top left corner, as the anchor of
     `zoom_by` is, and `view_px` the view's size, as `frame` is asked for (it is used for the
     shown scale and for `visible` and `dest`, not `self.view_px`). The state turns the point
     into the upright image's edge coordinates with the geometry it already has for the drawn view
     (`geometry::visible` of the current image's `ImageSize`, the zoom's shown scale and centre,
     and `geometry::dest`: image x = the visible rectangle's left + (px − dest's left) / scale,
     likewise y); a point outside `dest` is `Outside` (nothing changes, no note: the shell ends
     its mode silently). With no current image, a current key whose source is not `Source::Raw`,
     a failed image, or a file whose `wb_info` says it is not `measurable`, the result is
     `Unusable` and `pick_note = Unusable`. Otherwise `grey_area(size, image_point, scale)` (a
     `None` there counts as `Outside`), `pending_grey = Some((key, area))`, `pick_note =
     Measuring`, `since = Instant::now()` and the result `Started`. A new click replaces a request
     that is still being measured.
  3. **The job** (W6-Q2): `Job::Grey(Key, Rect)` and `Task::Grey { generation, key, area, session
     }` in `core.rs`, whose worker calls `backend.grey_white_balance(&mut session, area)`, asks
     `session_usable` after a `None` as `Task::Statistics` does, and calls a new
     `State::finish_grey(generation, key, area, session, result: Option<Wb>, usable: bool)`.
     `plan_wants` plans, **ahead of everything else** (as `statistics_job` is inserted at index 0
     in `pick`, and the click goes before it): for a `pending_grey` whose key is not failed and
     whose size is known, `Job::Open(key)` when `sessions` lacks the key, else
     `Job::Grey(key, area)`; `pick` lends the session for it (the interactive worker only for the
     current key) and books it under `STATISTICS_LEND`, whose doc comment is updated again.
  4. **The result** (W6-Q2, W6-Q3): `finish_grey` applies a result only when the generation is
     current, `key.0 == cur` and `pending_grey == Some((key, area))`; otherwise the session is put
     back (as `return_session` does) and **nothing changes**. Then, for `Some(wb)`: `grey =
     Some((cur, wb))`, `wb_mode = Grey`, `pending_grey = None`, `pick_note = Idle`, `since =
     Instant::now()`; for `None` with `usable`: `pick_note = TooBrightOrDark` (the mode and the
     grey unchanged) and `pending_grey = None`; for `None` with `!usable` (a session spent by an
     earlier stopped render): the session is dropped (unless released) and **the request is kept**,
     so the plan opens the file and measures again, as `finish_statistics` does. A session released
     for a metadata write while the click is measured is dropped and the request kept.
  5. **When it ends** (W6-Q2, A-Q2): `pending_grey` and `pick_note` are cleared, and `grey` is
     forgotten with a mode `Grey` becoming `Shot`, in `go` when the position changes, in
     `open_folder`, and in `set_files` when the current image's path is not the one before; in
     `set_files` that keeps the path at a new position, `grey`'s position is moved through the
     map `to` and the mode stays; `set_mode` (the same image in the other display mode) clears
     `pending_grey` and the note and keeps `grey`; `set_white_balance(mode)` (any change) and
     `reset_white_balance()` clear `pending_grey`, the note and (for `Shot` or any mode other than
     `Grey`) the grey. T350's `set_white_balance(Grey)` stays refused. A result for a request that
     was cleared is dropped by point 4.
  6. **The frame** (W6-Q8): `WhiteBalanceState` gets `pick: PickState` (`Measuring` while
     `pending_grey` is set, else the note) and `can_pick: bool` (the current key's source is
     `Source::Raw`, its `wb_info` is there and `measurable`); the `Default` and `off()` give `Idle`
     and `false`. The base `Grey` and the view `Waiting` while a click is measured (the view's
     rules of T350 point 7 otherwise); a mode that is `Shot` with a click being measured is still
     `Off`/`Waiting` by those rules (the mark only follows the mode; the shell's line, T353,
     follows `pick`).
  7. **The core:** `ViewCore::pick_grey(&self, point: [f64; 2], view_px: [u32; 2]) -> PickResult`
     under the lock, `stop_unwanted()`, and `notify_all` and `wake_ui` **on every result** (the
     note changes even when the click is refused); not a user request, nothing recorded.
  8. **What does not change:** everything T350's point 8 lists (the histogram, the overlays, auto
     exposure, the focus job); the compare's second side is its own `ViewCore` and state, so a
     click's grey belongs to the core it was made on (T353 calls the side's own core).
- **Tests to add** (in `state.rs`'s tests with `State<u32>`, unless named otherwise; `Mul(r, b)`
  is `Wb::Mul { r, b }`):
  - `pick_grey` at the view's centre at fit on an image of `ImageSize { w: 6000, h: 4000, flip: 0 }`
    and on one with `flip` 6 (upright 4000 × 6000) records `pending_grey` whose area's centre is
    the image's centre within 1 photosite and returns `Started` with the frame's `pick` =
    `Measuring`; the plan holds `Job::Open` first when the key has no session and `Job::Grey` once
    it has, ahead of a `Job::Statistics` and of every render.
  - A point in the letterbox (outside `dest`) returns `Outside` and changes nothing; a click in
    JPEG mode, on a TIFF, with no image, on a failed image and with `measurable: false` return
    `Unusable` and the frame's `pick` is `Unusable`, `can_pick` false.
  - `finish_grey` with `Some(Mul(1800, 900))`: the mode is `Grey`, every view of the current image
    asks for it, the neighbours ask as shot, the frame's base is `Grey`; with `None` and `usable`
    the mode is unchanged and `pick` is `TooBrightOrDark`, cleared by the next mode change, the
    next click, `go` and `reset_white_balance`; with `None` and `!usable` the session is dropped
    and the plan opens the file and measures again; a result for another area (a second click
    replaced the first) or another generation or a left image is dropped (mode and grey unchanged,
    the session back); a release for a metadata write keeps the request.
  - `go` to another position, `open_folder` and a `set_files` that changes the current path reset
    `Grey` to `Shot` and clear the request and the note; a `set_files` that keeps the path at a new
    position keeps the grey (its position moved) and the mode; `set_mode` keeps the grey and clears
    the request; modes `Auto`, `Daylight` and `Preset` survive a step and keep their balance; a
    click while the mode is `Auto` makes the mode `Grey`, and `set_white_balance(Auto)` then
    gives `Auto` again and forgets the grey.
  - `set_white_balance(WbMode::Grey)` still returns `false`; `Shot` from `Grey` forgets the grey.
  - `can_pick` is true for an opened RAW image with a measurable `WbInfo` and false before its
    info is there.
  - `tests/engine.rs`: the `Fake` backend gets a `grey_white_balance` that gives a `Wb` and counts
    its calls; a full run through `ViewCore`: `pick_grey` at a point, the worker measures, the
    current image's next view is rendered with the grey balance.
- **Not in this task:** the key, the menu item, the pick mode and the click's mapping from the
  screen (T353); the texts of the failures (T353); the mark (T351).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text). No new
  dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-28 | Blocked | Added (from the cold reads and the chain review of T350 and T351); waits for T350 |
