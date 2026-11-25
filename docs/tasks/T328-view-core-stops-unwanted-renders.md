# T328. ADR 0027 W5-Q1: the view core stops a render that is no longer wanted — Open

- **Why:** ADR 0027 W5-Q1 (answered by the owner 2026-11-25, ADR 0027 §4.4). A render under way
  runs to its end today even when nothing wants its view any more, so the next image waits for it:
  the pan view of the image just left (T326) and the fine render (T313). T327 gave `pw-libraw`
  renders that a check can stop; this task makes the view core stop the renders it no longer
  wants. The second and last task of the chain T327 → **T328**. T327 is Done (2026-11-25): a stop
  takes effect only between LibRaw's stages (220–350 ms on a whole-image PPG render of an ARW,
  up to 1.3 s on a RAF) and a stop while LibRaw works **always** spends the session, so the file
  must be opened again (about 0.3 s for 61 Mpix). **The owner decided after that measurement
  (2026-11-25, ADR 0027 after W5-Q1's chain): a render is stopped only when its image is no
  longer wanted** (a step to another image, a new folder or file list, a release), never for a
  changed setting of the same image (sharpening, fine details, and later W5's exposure), whose
  render under way finishes first.
- **What the reviewer checked (2026-11-25):**
  - `crates/pw-viewcore/src/core.rs`: the worker loop takes a `Task` from `State::pick` and, for
    `Task::Render { generation, key, view, session }`, calls `Backend::render_half` (`Half`,
    `Reduced`) or `Backend::render_region` (`Zoom`, `Ring`, `Scaled`, `Fine`), then the reduction,
    sharpening and `to_display`, and gives the result to `State::finish_render`. `Task::Focus`
    also calls `render_half`. `act()` takes the lock for `open_folder` … `pan`;
    `set_fine_details`, `set_sharpening`, `set_view` and `set_orientation` have lock blocks of
    their own (with early returns); `Releaser::release` loops and calls `release(path)`;
    `frame()` can change the view's size and the gesture time; `reject()` can set `fine_failed`.
    `finish_open_checked` already gets answers from the backend through closures passed in by
    the core (the state never calls the backend). The core's tests have a `Gate` fake backend.
  - `crates/pw-viewcore/src/state.rs`: `lend_render` takes the session out of `sessions`, books
    the job in `running` (`(generation, Job::Render(key, view))`), in `lent_paths` and in
    `inflight` (`Inflight { generation, key, view, bytes }`, which is `Copy`, and also holds
    views that are finished but not yet accepted; `satisfied()` counts an in-flight `Reduced`,
    `Fine` or `Scaled` view as done). `open_folder` and `set_files` clear `inflight`;
    `set_files` with the same list only goes to the position (no new generation). `wanted(full)`
    gives the plan as `(Key, Want)` pairs (`Want::{Shown, Fit, Target, Ring, Fine}`); `req(key,
    want)` makes the requirement and its `JobView { kind, region, density_q, sharpen }`; during a
    gesture (`in_gesture()`) `Shown` gives `Half` instead of `Reduced`. Between 50 % and 100 %
    the pan view is a `Scaled` view asked for as `Want::Ring` (`pan_view`), the same kind as the
    shown view there. `drop_session` removes a session and its bytes, after which the plan opens
    the file again (a failed fine render already does this). The unit tests drive `State<u32>`
    directly (for example `pick_fine`).
  - `crates/pw-viewcore/src/backend.rs`: the `Backend` trait (several methods with default
    bodies); `DecodeError` with `new`, `damaged` and `no_embedded_jpeg`. `pw-loupe`'s
    `preview.rs` builds a `DecodeError` as a struct literal in a test, so a new field would
    break another crate. `pw-libraw` is an optional dependency (the native backend), so the
    state and the trait cannot use its types.
  - `crates/pw-viewcore/src/native.rs`: `NativeBackend` and `NativeSession::{Raw, Jpeg, Tiff}`;
    the RAW renders call `Session::render_half` and `render_region_with(r, MARGIN, Ppg)`.
  - `crates/pw-viewcore/tests/native_files.rs`: the real-file tests, `#[ignore]`, `PW_TESTDATA`.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/backend.rs`,
  `crates/pw-viewcore/src/state.rs`, `crates/pw-viewcore/src/core.rs`,
  `crates/pw-viewcore/src/native.rs`, `crates/pw-viewcore/src/lib.rs` (the export of `Stop`),
  `crates/pw-viewcore/tests/native_files.rs`, `docs/test-inventory.md` (rule 12), and this task's
  own file (status and log).
- **What:**
  1. **The stop** (`backend.rs`): a type `Stop` (`Clone`, `Send`, `Sync`, `Debug`; an
     `Arc<AtomicBool>` inside) with `new()`, `stop(&self)` and `is_stopped(&self) -> bool`.
     `DecodeError::cancelled()` and `is_cancelled()`, **without a new field** (the error's
     message is a constant text, `is_cancelled` compares it, as the other kinds of the crate do
     where they can; the struct literal in `pw-loupe` keeps compiling). The `Backend` trait gains
     three methods with default bodies, so the other backends (the tests' fakes, `pw-loupe`,
     `pw-render`) need no change: `render_half_stoppable(&self, s, stop: &Stop)` (default:
     `render_half`), `render_region_stoppable(&self, s, r, stop: &Stop)` (default:
     `render_region`) and `session_usable(&self, s) -> bool` (default `true`).
  2. **The native backend** (`native.rs`): for a RAW session the two stoppable renders call
     T327's `render_half_stoppable` and `render_region_stoppable` with `&|| stop.is_stopped()`;
     an error for which `pw_libraw::Error::is_cancelled()` is true (it tests the start of the
     text; never `contains`, as `raw_error` does for `DAMAGED`) becomes `DecodeError::cancelled()`.
     `session_usable` is `!is_spent()` for a RAW session, `true` otherwise (since T327 a session
     can also be spent by a render that failed for another reason). JPEG and TIFF sessions are not
     stopped (their renders are short); the doc comments say so, and that a stop while LibRaw
     works spends the session (T327). A
     spent session is never lent again (point 5), so `native.rs`'s statistics, clipping map and
     focus never see one (they turn an `Err` into "none", which would be kept for good).
  3. **The worker** (`core.rs`): `Task::Render` carries a `Stop`; the render uses the stoppable
     methods. After the render, before the reduction, the worker looks at the `Stop` once more:
     if it is set, an `Ok` result counts as cancelled (the session is not spent by that); an
     `Err` that is not cancelled (for example a damaged file) stays the failure it is today. The
     reduction is chained inside the render's `match` today (`core.rs`, the `Task::Render` arm),
     so that block is split for this look. A cancelled result skips the reduction, sharpening
     and `to_display` and goes to the state through a new `finish_render_checked`, which gains
     the backend's `session_usable` answer as an argument (passed in by the core, as
     `finish_open_checked` gets its answers); `finish_render` stays, with its signature, as a
     `#[cfg(test)]` wrapper of it (`session_usable` true), as `finish_open` wraps
     `finish_open_checked`, so the existing tests' calls do not change. `Task::Focus`, the
     statistics and the clipping map are not stopped.
  4. **What is stopped** (`state.rs`): `lend_render` makes a new `Stop` and keeps a clone in a
     map of its own beside `running`, keyed as `running` is, together with the `Want`s the job
     was lent for (`inflight` stays `Copy` and is not used for it). The `Want` is lost today in
     `plan_of` (it gives `Job::Render(key, view)`); it is carried to `lend_render` through an
     internal plan of the state that keeps the `Want`s beside each job, while the public
     `plan()` and `Job` stay as they are (the existing tests use both). When one `JobView` is
     wanted under several `Want`s at the lend (rare: for example `Shown` and `Fit` when the scale
     is the fit scale), every `Want` whose requirement gave exactly that `JobView` is recorded,
     from the same plan `pick` uses (`wanted(false)`), before `plan_of` drops the duplicate job.
     `Want` may derive `Hash` if a set needs it. The map is emptied only as
     the jobs finish, not by `open_folder` or `set_files`. One state method, `stop_unwanted`,
     stops every job of that map that is still in `running`, not yet stopped, and for which
     one of these holds:
     - it was lent in an older generation (a new folder or a new file list);
     - its session is marked for release (`Releaser::release`, a metadata write waiting);
     - its key is **not the current image** any more: its position (`Key` is `(usize, Source)`)
       is not the current position, in **either** display mode (`ck` in `wanted` is the current
       position in the current mode, for which `Ring` and `Fine` are made; `Shown` is also asked
       for the neighbours, the other display mode and the image behind, so the rule looks at the
       position, not at the `Want`; owner, 2026-11-25: a change of display mode, `toggle_mode` or
       `set_mode`, is the same image and stops nothing), **and** the full plan
       (`wanted(true)`) wants **neither** its `JobView` for its key under any `Want` (compared
       without `sharpen`) **nor**, under one of the `Want`s it was lent for, a view of its key
       of the same `ViewKind`. For this comparison `Half` and `Reduced` count as one kind. A job
       of the current image is therefore never stopped by this rule (owner, 2026-11-25: a
       changed setting of the same image lets its render finish), and a neighbour's job is not
       stopped by a change of sharpening either. So the start of a gesture, a pan or a zoom
       step, a change of sharpening, fine details turned off and a new orientation stop
       nothing; stepping on stops the pan view of the image left (−1 is not the current image
       and wants no `Ring`), its fine view, and at fit its 100 % view asked for in advance
       (`Target`); its shown view is kept when the plan still wants it (as `Fit` or `Shown`);
       stepping forward keeps +1's render (its view is still wanted); a neighbour that falls out
       of the plan is stopped.
     `stop_unwanted` is called, under the lock, at the end of `act()`, in `set_fine_details`,
     `set_sharpening`, `set_view` and `set_orientation` where they change something, after each
     `release(path)` in `Releaser::release`, in `frame()` and `reject()`, and in the worker after
     `finish_open_checked` and `finish_render_checked`; it returns at once when the stop map is
     empty (it is called on every `frame()`). `Stats` gains three counters: `stops` (a `Stop`
     set, once per job), `cancelled` (a render that came back cancelled) and `spent` (a session
     dropped because it was spent). Each core of the compare (`pw-loupe` has a second
     `ViewCore`) stops only its own renders; that is intended.
  5. **A render comes back** (`finish_render_checked`), in this order: its entry in the stop map is
     removed first, on every path. A render of an older generation is then handled as today
     (`discarded`), cancelled or not. Otherwise, a cancelled result (`is_cancelled()`) gives no
     view and no failure, is not counted as `discarded`, and its time is not added to
     `render_ms` (so W3's and W5's timings are not skewed); `cancelled` is counted, and its
     `inflight` entry is removed, so the plan may ask for the view again. The session goes back
     with `return_session` first, always (it cleans up `lent` and `lent_paths`, which
     `release()` waits on, and the detached bytes of an old folder); only when that returns true,
     the session was **not released** while it was lent (the same `!released` guard as today's
     path for a failed fine render; else `drop_session` would drop a session opened after a
     release timed out) and the session is not usable is it then dropped with `drop_session`
     and `spent` counted, so the plan opens the file again if it is still wanted and a spent
     session is never lent again. A render whose stop was unset at the worker's second look
     (point 3) is handled as today (its view is booked or discarded by today's rules), even if
     the stop is set later.
  6. **A real file** (`tests/native_files.rs`, `#[ignore]`, `PW_TESTDATA`): open a folder of
     the owner's ARW files at fit below 50 % (fine views are made only there), turn fine details
     on, wait until `frame()` shows the fine view waiting (nothing public tells a lent render
     from one not yet picked, so this is the start), then 450 ms more (half of T327's
     unstopped whole-image PPG render of a 61 Mpix ARW, about 900 ms in a release build; a debug
     build is slower, which the repeats below cover), step to the next
     image, and check that the stats count **at least one** `cancelled` render (not only a
     `stop`) and that the next image's view arrives. If the render had already finished, the
     test repeats on the next image, up to five times, and fails only if none was cancelled; then step back and check that the first image is shown again (its session
     reopened if it was spent). Run it with
     `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace -- --ignored <the test's name>`
     (not `cargo test -p`, whatever the file's header says, `AGENTS.md` §5).
- **Existing tests:** an existing test whose pattern on `Task::Render` lists every field may gain
  `stop: _` (or `..`), as the new field requires; nothing else in an existing test changes
  (rule 5).
- **Tests to add** (unit tests in `state.rs` driving `State<u32>` as the existing ones do, and in
  `core.rs` with its fakes):
  - between 50 % and 100 %, after a drag, stepping forward while the current image's pan view is
    in flight stops it; while +1's render is in flight it does not;
  - a pan and a zoom step of the same image do not stop its ring or pan view in flight; the start
    of a gesture does not stop a reduced view in flight;
  - `set_files` with a different list and `open_folder` stop everything in flight (the stop is
    still found after `inflight` is cleared); a release stops the render of that path's session;
  - a change of sharpening does not stop the current image's view in flight with the old
    sharpening, and turning fine details off does not stop its fine render in flight (both
    finish, by today's rules); stepping away then stops that fine render;
  - a stopped render that comes back: no view, no failure, not `discarded`, no time in
    `render_ms`, `cancelled` counted once, its `inflight` entry gone and the view asked for again
    when still wanted; its session back when usable; when not, `lent_paths` cleaned (a release
    of that path then finishes at once), the session dropped, `spent` counted, and the file
    opened again when wanted; a cancelled render of an older generation does not drop the new
    folder's session at the same position; a cancelled render whose session was released while
    it was lent does not drop a session opened after the release;
  - a change of display mode does not stop the current image's fine or pan view in flight in
    the mode left;
  - a sharpening change does not stop a neighbour's view in flight; at fit, stepping on stops
    the image left's 100 % view asked for in advance;
  - a stop set after the render finished: `stops` counted, `cancelled` not, the view booked;
  - the pan view of the image left is stopped although −1's shown view is also `Scaled` (the
    `Want` carried to the lend);
  - a core test with a fake backend whose stoppable render waits until its `Stop` is set (with
    a time limit, so the test cannot hang): stepping away returns the worker within that limit.
- **Not in this task:** asking for the pan view again without a drag (T326; a question of its
  own once this chain is hand-checked, W5-Q1); stopping the open, the statistics, the clipping
  map or the focus job; stopping a render of the current image for a changed setting (the
  owner, 2026-11-25); LibRaw's `setCancelFlag()` (`backlog.md`); W5's exposure (its own chain
  after this one).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12. No new dependency. No `unwrap`,
  `expect` or panic in non-test code. An extra code review (the core's workers and their state,
  which run while the user keeps working).
- **Done when:** `.\tools\check.ps1` is all OK, the test of point 6 has been run with
  `PW_TESTDATA` and its result is in the log, and `git diff --stat` shows only the files above.
  Then the owner's hand check (fine details on and quick steps; a drag between 50 % and 100 % and
  a step at once; the compare) and `latency.ps1` (the owner's, `D:\PhotoWorkflowData\pm-study\tools\latency.ps1`,
  not in the repository) at fit and at 51.2 %, as after T326, with the `spent` counter read:
  every stop spends the session (T327), so an image left whose render was stopped is opened again
  when the plan wants it (about 0.3 s, for example a step back), and the owner weighs that.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added (ADR 0027 W5-Q1); waits for T327 and its cold read |
| 2026-11-25 | Blocked | Cold read: the rule of point 4 did not stop the pan view of the image left (it is a `Scaled` view as −1's shown view): now the exact view, or the same `Want` with the same kind and sharpening; `Half` and `Reduced` one kind (a gesture stops nothing); the stops kept beside `running`, not in `inflight` (`Copy`, cleared by `open_folder` and `set_files`, and holding finished views); only running jobs stopped, once; a stopped render's `inflight` entry removed, not `discarded`; `finish_render` gets `session_usable` from the core; one method `stop_unwanted` at named places (`act`, the four setters, `release`, `frame`, `reject`, after the worker's finishes); `DecodeError` without a new field (`pw-loupe`'s test builds it as a literal); the tests drive `State<u32>`; `set_files` with a different list; point 6 below 50 %, "at least one", and its run command |
| 2026-11-25 | Blocked | Chain review (T327 and T328): `return_session` always first, `drop_session` only when it returned true (else `lent_paths` stays and a release waits until its timeout, and an old generation's key could drop the new folder's session); the order in `finish_render` (stop entry removed, old generation as today, then the cancelled path); the `Want` carried through `plan_of` → `pick` → `lend_render`; counters `stops`, `cancelled` and `spent`, no `render_ms` for a cancelled render; the worker looks at the stop again before the reduction; `is_cancelled` by the start of the text; a spent session never lent again; each compare core stops its own; point 6 waits for the lend and repeats up to five images; the hand check names `latency.ps1`'s place and reads `spent`. Waits for T327 |
| 2026-11-25 | Blocked | T327 Done. Changed after the owner's decision on T327's measurement (2026-11-25, "Ja, bara när bilden lämnas", ADR 0027 after W5-Q1's chain): a render is stopped only when its image is no longer wanted; point 4's third rule now needs the key to be no longer the current image, and `sharpen` is not compared (a change of sharpening, fine details off, a new orientation stop nothing of the current image); the tests and Not in this task follow; point 2 and the Why updated with T327's findings (every stop while LibRaw works spends the session; the spent text no longer holds "cancelled"); the hand check's note on `spent` rewritten. Open after a new cold read |
| 2026-11-25 | Open | Cold read (after the change for the owner's decision): point 5 gets the `!released` guard before `drop_session` (else a session opened after a timed-out release could be dropped); the `Want` carried through an internal plan, `plan()` and `Job` unchanged, `finish_render` kept as a `#[cfg(test)]` wrapper of `finish_render_checked`, existing tests may only gain `stop: _` in a pattern on `Task::Render`; point 4's rule looks at the position (not the `Want`: `Shown` is also asked for neighbours), compares a neighbour's view without `sharpen`, names the image left's `Target` at fit; points 3 and 5 split at the worker's second look, a non-cancelled `Err` stays a failure; the reduction block in `core.rs` is split; `stop_unwanted` returns at once with an empty map; point 6 waits from `frame()`'s waiting fine view plus 450 ms (T327's figure stated); `Want` may derive `Hash`; the several-`Want`s example corrected. **Owner, 2026-11-25 ("Ja, samma bild"): a change of display mode is the same image and stops nothing** (ADR 0027, after W5-Q1's chain; a test added). Set Open |
