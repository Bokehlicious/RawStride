# T327. ADR 0027 W5-Q1: a LibRaw render that can be stopped, and its timing — Blocked

- **Why:** ADR 0027 W5-Q1 (answered by the owner 2026-11-25, ADR 0027 §4.4): a render under way
  cannot be cancelled today, so a long render on either worker makes the next image wait (the pan
  view after a drag and a step at once, T326; the fine render, T313), and W5 will render the
  current image again for every changed exposure value. This task gives `pw-libraw` a render that
  a check from another thread can stop, and measures how quickly it stops on the owner's files.
  The first task of the chain **T327** → T328 (the view core uses it). Nothing in the program
  changes yet: the existing render functions keep their behaviour.
- **What the reviewer checked (2026-11-25):**
  - `crates/pw-libraw/csrc/pwlr.cpp`: `pwlr_open` sets every LibRaw parameter and unpacks;
    `pwlr_render(session, quality, half, x, y, w, h, &ox, &oy, &ow, &oh, error)` sets
    `user_qual`, `half_size` and the crop box, calls `dcraw_process` and `dcraw_make_mem_image`,
    and returns NULL with `error` filled on failure. `PwSession` holds `lr`, `stream` and
    `data_errors`.
  - LibRaw 0.22 (vcpkg, `libraw_types.h`, `libraw.h`): `progress_callback` is
    `int (*)(void *data, enum LibRaw_progress stage, int iteration, int expected)`, set with
    `LibRaw::set_progress_handler(cb, data)`; a non-zero return makes LibRaw throw
    `LIBRAW_EXCEPTION_CANCELLED_BY_CALLBACK`, and `dcraw_process` returns
    `LIBRAW_CANCELLED_BY_CALLBACK` (−100010). How often LibRaw calls it during a demosaic, and
    whether LibRaw then recycles the session (frees the unpacked RAW data, as it does for
    `LIBRAW_EXCEPTION_IO_CORRUPT`, see the X-Trans comment in `pwlr.cpp`), is **not known** [T]:
    this task finds out.
  - `crates/pw-libraw/src/lib.rs`: `Session::render` (private) calls `pwlr_render`;
    `render_half`, `render_full_with` and `render_region_with` are the public renders;
    `Error(pub String)`; the constant `DAMAGED` marks a damaged file's error text.
  - The real-file tests in `crates/pw-libraw/tests/` are `#[ignore]` and read `PW_TESTDATA`
    (`statistics.rs` has the helpers). The owner's folder `D:\PhotoWorkflowData\RAW` holds Sony
    ARW files (`TN…`, 61 Mpix A7R IVA and 50 Mpix A1; which is which is not in the name) and
    Fujifilm RAF files (`_DSF…`, X-Trans).
- **Crate:** `pw-libraw` (with its C++ shim). **Files:** `crates/pw-libraw/csrc/pwlr.cpp`,
  `crates/pw-libraw/src/lib.rs`, `crates/pw-libraw/tests/stop.rs` (new),
  `docs/test-inventory.md` (rule 12), and this task's own file (status and log).
- **What:**
  1. **The shim** (`pwlr.cpp`):
     - `pwlr_render` gains two last parameters, `int (*stop)(void *ctx)` and `void *ctx`; `stop`
       may be NULL (no check, today's behaviour).
     - If `stop` is set and `stop(ctx)` returns non-zero **before** `dcraw_process` is called, the
       render returns NULL at once with an error text that starts with `cancelled`, and LibRaw is
       not called (the session is untouched).
     - `pwlr_open` sets a progress handler (`LibRaw::set_progress_handler`) on the LibRaw object
       with the session as its data, after the unpack has succeeded. `PwSession` gains the
       current render's `stop` and `ctx` and the spent flag (its `new PwSession{…}` initialises
       them). The session's `stop` and `ctx` are set at the start of `pwlr_render` and cleared
       before it returns on every path; the handler returns `stop(ctx)` when one is set, else 0.
       As far as is known LibRaw calls the handler on the thread that called `dcraw_process`
       (between its stages, `LibRaw_progress`), not on an OpenMP worker; the Rust side does not
       rely on it (point 2's `Sync` bound).
     - When `dcraw_process` returns `LIBRAW_CANCELLED_BY_CALLBACK`, the error text starts with
       `cancelled`. If LibRaw has freed the unpacked RAW data (`imgdata.rawdata.raw_alloc` is NULL
       afterwards, or another check the agent finds in LibRaw's headers and names in the report),
       the session is marked **spent**: every later `pwlr_render` (NULL), `pwlr_statistics` and
       `pwlr_clipping_map` (both −1, so `lib.rs` gives `Err`, never 0, which it reads as "no
       statistics") on it fails with the text `spent: the render was cancelled; open the file
       again`, without calling LibRaw. A new function `int pwlr_spent(void *session)` tells it.
       (After LibRaw's `recycle()` the saved sizes are 0 too, so without the flag a region would
       be refused as "outside the image (0x0)" and the statistics read as none.)
  2. **`pw-libraw`** (`lib.rs`):
     - the `extern` declaration of `pwlr_render` follows the shim; the existing renders pass NULL
       and behave as today;
     - `pub fn render_half_stoppable(&mut self, stop: &(dyn Fn() -> bool + Sync)) -> Result<Rgb8>`
       (the same demosaic value as `render_half`) and `pub fn render_region_stoppable(&mut self,
       r: Rect, margin: u32, demosaic: Demosaic, stop: &(dyn Fn() -> bool + Sync)) ->
       Result<Rgb8>` (the same checks and crop as `render_region_with`). `stop` is passed to the
       shim through an `extern "C"` trampoline: `ctx` points at the reference itself (a
       `&dyn Fn` is a fat pointer, so `ctx` is a `*const &(dyn Fn() -> bool + Sync)` that lives
       on the caller's stack for the whole call). The trampoline calls `stop` inside
       `std::panic::catch_unwind(AssertUnwindSafe(…))` and answers "stop" on a panic, so no panic
       crosses into C++ (it would abort the process);
     - `pub const CANCELLED: &str = "cancelled"` and `Error::is_cancelled(&self) -> bool` (the
       text starts with it, as `DAMAGED` is used), and `pub fn is_spent(&self) -> bool` on
       `Session` (from `pwlr_spent`). A spent session's error starts with `spent:`, so
       `is_cancelled()` is false for it; a caller asks `is_spent()` (T328 does);
     - doc comments say what a stop costs: none before LibRaw starts; after it, whether the
       session is spent (what point 3 finds).
  3. **Tests on real files** (`tests/stop.rs`, `#[ignore]`, `PW_TESTDATA`, one Sony ARW and one
     RAF). The helpers of `statistics.rs` are private to that test file: `stop.rs` has its own
     small helpers (the folder from `PW_TESTDATA`, the ARW files, the RAF files), in the same
     style; no shared `tests/common` module.
     - a stop that answers "stop" at once: both stoppable renders return `is_cancelled()`, the
       session is not spent, and a following `render_half` gives the same bytes as on a fresh
       session;
     - a stop that never answers "stop": the stoppable renders give the same bytes as
       `render_half` and `render_region_with` (PPG) of the same region;
     - a stop set from another thread during a whole-image PPG render
       (`render_region_stoppable` of the whole image), after about half of an unstopped render's
       time: the render returns `is_cancelled()`; then either the session is spent and a further
       render fails with the spent text, or it is not spent and a further render gives the same
       bytes as on a fresh session. The test asserts whichever LibRaw does and the report says
       which;
     - the same for `render_half_stoppable`, if LibRaw calls the handler during a half-size
       render at all (if it never does, the test says so in a comment and asserts that the
       render finishes normally).
  4. **The timing** (`tests/stop.rs`):
     `#[ignore = "a time measurement on real files; run with --release (ADR 0027 W5-Q1)"]`
     `fn timing_of_a_stop()`. For the first file of every distinct size among the ARW files
     (so the 61 and the 50 Mpix are both measured; every ARW is opened to learn its size, which
     takes some seconds) and one RAF: the unstopped time of a
     whole-image PPG region render and of a half-size render (median of three), then for each,
     stops set from another thread at 10 %, 50 % and 90 % of that time, three runs each (a fresh
     session every run; the open not timed). It prints, per file and render, the median and
     the largest **time from the stop to the return**, and how many of the runs ended
     cancelled and how many finished anyway.
  5. **The measurement:** run
     `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --workspace --release -- --ignored --nocapture timing_of_a_stop`
     (a debug build measures the wrong thing; **the `--release` run is approved by the owner for
     this one measurement**, see the log). It builds in the project's `target\release`; if
     `rawstride.exe` is running and locks it, ask the owner to close it. Put every printed line in
     the log row, and name the CPU (`Get-CimInstance Win32_Processor`). Also run the other tests
     of point 3 with `-- --ignored` (debug is enough) and give their result.
- **Tests to add:** the `#[ignore]` tests of point 3 and the timing of point 4 (none is run by
  `.\tools\check.ps1`, which has no `PW_TESTDATA`); a unit test in `lib.rs` that
  `Error("cancelled: …").is_cancelled()` is true and a `DAMAGED` text's is false.
- **Not in this task:** the view core (T328); a stop during the open (unpack), the statistics or
  the clipping map; deciding whether the time is good enough (above 100 ms from the stop to the
  return, the owner decides whether T328 goes on, W5-Q1). LibRaw also has `setCancelFlag()`, a
  flag it checks inside some of its loops (`checkCancel`, `libraw.h`), which might stop sooner
  than a handler called between stages; it is not used here. If the timing is slow because the
  handler is called only between stages, the report says so, and using that flag becomes the
  owner's question.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12. No new dependency. No `unwrap`,
  `expect` or panic in non-test code; the trampoline never lets a panic cross into C++. The C++
  code allocates nothing new per render and never throws out of an `extern "C"` function. An
  extra code review (the C++ shim, `docs/tasks/README.md`).
- **Done when:** `.\tools\check.ps1` is all OK, the measurement of point 5 is in the log, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-25 | Blocked | Added (ADR 0027 W5-Q1); Open after its cold read, the chain review and the owner's yes to the `--release` run |
| 2026-11-25 | Blocked | Cold read: the handler set after the unpack and `PwSession`'s new fields initialised; a spent session's statistics and clipping map fail with −1 (an `Err`, never "no statistics"); `stop` gets a `Sync` bound, `ctx` points at the fat reference, `catch_unwind` with `AssertUnwindSafe`; `render_half_stoppable` uses `render_half`'s demosaic value; `stop.rs` has its own helpers (those of `statistics.rs` are private); every ARW is opened to learn its size; a spent error is not `is_cancelled()`; LibRaw's `setCancelFlag` named as the alternative if the handler is called only between stages. Confirmed against LibRaw 0.22's headers: the callback type, `set_progress_handler`, `LIBRAW_CANCELLED_BY_CALLBACK`, `imgdata.rawdata.raw_alloc` |
