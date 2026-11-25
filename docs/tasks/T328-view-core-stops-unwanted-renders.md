# T328. ADR 0027 W5-Q1: the view core stops a render that is no longer wanted — Blocked

- **Why:** ADR 0027 W5-Q1 (answered by the owner 2026-11-25, ADR 0027 §4.4). A render under way
  runs to its end today even when nothing wants its view any more, so the next image waits for it:
  the pan view of the image just left (T326), the fine render (T313), and from W5 a view of an
  exposure value already changed again. T327 gave `pw-libraw` renders that a check can stop; this
  task makes the view core stop the renders it no longer wants. The second and last task of the
  chain T327 → **T328**. Blocked: waits for T327, and for the owner's decision if T327's time from
  a stop to the return is above 100 ms (W5-Q1).
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
     an error with `is_cancelled()` becomes `DecodeError::cancelled()`. `session_usable` is
     `!is_spent()` for a RAW session, `true` otherwise. JPEG and TIFF sessions are not stopped.
  3. **The worker** (`core.rs`): `Task::Render` carries a `Stop`; the render uses the stoppable
     methods; a cancelled result skips the reduction, sharpening and `to_display` and goes to
     `finish_render`, which gains the backend's `session_usable` answer as an argument (passed
     in by the core, as `finish_open_checked` gets its answers). `Task::Focus`, the statistics
     and the clipping map are not stopped.
  4. **What is stopped** (`state.rs`): `lend_render` makes a new `Stop` and keeps a clone in a
     map of its own beside `running`, keyed as `running` is, together with the `Want` the job
     was lent for (`inflight` stays `Copy` and is not used for it). The map is emptied only as
     the jobs finish, not by `open_folder` or `set_files`. One state method, `stop_unwanted`,
     stops every job of that map that is still in `running`, not yet stopped, and for which
     one of these holds:
     - it was lent in an older generation (a new folder or a new file list);
     - its session is marked for release (`Releaser::release`, a metadata write waiting);
     - the full plan (`wanted(true)`) wants **neither** its exact `JobView` for its key (under
       any `Want`) **nor**, under the same `Want` it was lent for, a view of its key of the same
       `ViewKind` and the same `sharpen`. For this comparison `Half` and `Reduced` count as one
       kind, so the start of a gesture stops nothing. So a pan or a zoom step of the same image
       keeps its ring, its 100 % view or its pan view running (same `Want`, same kind, another
       region); stepping on stops the pan view of the image left (−1 wants no `Ring`) and its
       fine view; stepping forward keeps +1's render (its exact view is still wanted); a change
       of sharpening stops the views made with the old one.
     `stop_unwanted` is called, under the lock, at the end of `act()`, in `set_fine_details`,
     `set_sharpening`, `set_view` and `set_orientation` where they change something, after each
     `release(path)` in `Releaser::release`, in `frame()` and `reject()`, and in the worker after
     `finish_open_checked` and `finish_render`. Every stop is counted once in `Stats` as
     `cancelled`.
  5. **A stopped render finishes** (`finish_render`, result `is_cancelled()`): it gives no view
     and no failure, and is not counted as `discarded`. Its `inflight` entry is removed, so the
     plan may ask for the view again. When the session is usable it goes back as today
     (`return_session`); when it is not, it is dropped as `drop_session` does (its bytes off the
     budget, without `return_session` first), so the plan opens the file again if it is still
     wanted; a session marked for release is handled as today. A render that finishes normally
     although its stop was set is handled as today (its view is booked or discarded by today's
     rules).
  6. **A real file** (`tests/native_files.rs`, `#[ignore]`, `PW_TESTDATA`): open a folder of
     the owner's ARW files at fit below 50 % (fine views are made only there), turn fine details
     on, wait until the current image's fine render is under way, step to the next image, and
     check that the stats count **at least one** cancelled render and that the next image's view
     arrives; then step back and check that the first image is shown again (its session
     reopened if it was spent). Run it with
     `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace -- --ignored <the test's name>`
     (not `cargo test -p`, whatever the file's header says, `AGENTS.md` §5).
- **Tests to add** (unit tests in `state.rs` driving `State<u32>` as the existing ones do, and in
  `core.rs` with its fakes):
  - between 50 % and 100 %, after a drag, stepping forward while the current image's pan view is
    in flight stops it; while +1's render is in flight it does not;
  - a pan and a zoom step of the same image do not stop its ring or pan view in flight; the start
    of a gesture does not stop a reduced view in flight;
  - `set_files` with a different list and `open_folder` stop everything in flight (the stop is
    still found after `inflight` is cleared); a release stops the render of that path's session;
  - a change of sharpening stops a view in flight with the old sharpening;
  - a stopped render that comes back: no view, no failure, not `discarded`, `cancelled` counted
    once, its `inflight` entry gone and the view asked for again when still wanted; its session
    back when usable, dropped and opened again when wanted when not;
  - a core test with a fake backend whose stoppable render waits until its `Stop` is set (with
    a time limit, so the test cannot hang): stepping away returns the worker within that limit.
- **Not in this task:** asking for the pan view again without a drag (T326; a question of its
  own once this chain is hand-checked, W5-Q1); stopping the open, the statistics, the clipping
  map or the focus job; W5's exposure (its own chain after this one).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12. No new dependency. No `unwrap`,
  `expect` or panic in non-test code. An extra code review (the core's workers and their state,
  which run while the user keeps working).
- **Done when:** `.\tools\check.ps1` is all OK, the test of point 6 has been run with
  `PW_TESTDATA` and its result is in the log, and `git diff --stat` shows only the files above.
  Then the owner's hand check (fine details on and quick steps; a drag between 50 % and 100 % and
  a step at once) and `latency.ps1` at fit and at 51.2 %, as after T326.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added (ADR 0027 W5-Q1); waits for T327 and its cold read |
| 2026-11-25 | Blocked | Cold read: the rule of point 4 did not stop the pan view of the image left (it is a `Scaled` view as −1's shown view): now the exact view, or the same `Want` with the same kind and sharpening; `Half` and `Reduced` one kind (a gesture stops nothing); the stops kept beside `running`, not in `inflight` (`Copy`, cleared by `open_folder` and `set_files`, and holding finished views); only running jobs stopped, once; a stopped render's `inflight` entry removed, not `discarded`; `finish_render` gets `session_usable` from the core; one method `stop_unwanted` at named places (`act`, the four setters, `release`, `frame`, `reject`, after the worker's finishes); `DecodeError` without a new field (`pw-loupe`'s test builds it as a literal); the tests drive `State<u32>`; `set_files` with a different list; point 6 below 50 %, "at least one", and its run command |
