# T327. ADR 0027 W5-Q1: a LibRaw render that can be stopped, and its timing — Done

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
       them). The session's `stop` and `ctx` are set right before `dcraw_process` and cleared
       right after it returns, in one place, so they are never left set; the handler returns
       `stop(ctx)` when one is set, else 0. When `dcraw_process` fails (cancelled or not) and the
       session is not spent, `free_image()` is called as after a success, so a kept session never
       holds a processed buffer beyond its budget.
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
     cancelled and how many finished anyway, and how many left the session spent. The report
     says plainly whether a stop during LibRaw's work always spends the session (then every stop
     costs opening the file again, about 0.3 s for 61 Mpix, if the image is still wanted; T328
     and the owner weigh that).
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
| 2026-11-25 | Open | Chain review (T327 and T328): `stop`/`ctx` set right before `dcraw_process` and cleared right after, in one place; `free_image()` after a failed `dcraw_process` when the session is not spent; the report says whether a stop always spends the session (each stop then costs a reopen, weighed in T328). T328 changed for the rest (its log). **The `--release` run of point 5 approved by the owner on 2026-11-25** ("Ja till båda"). Set Open |
| 2026-11-25 | Ready for review | Done as written. **Shim:** `pwlr_render` takes `stop`/`ctx` (NULL = as before); a stop before `dcraw_process` returns `cancelled: …` at once with LibRaw untouched; `pwlr_open` sets the progress handler after a good unpack; `PwSession` has `stop`, `ctx`, `spent`, set right before `dcraw_process` and cleared right after; a failed `dcraw_process` on a session that is not spent calls `free_image()`; `LIBRAW_CANCELLED_BY_CALLBACK` gives `cancelled: …` and marks the session spent when `rawdata.raw_alloc` is NULL; `pwlr_spent`; spent sessions fail `pwlr_render`, `pwlr_statistics`, `pwlr_clipping_map` with `spent: the render was cancelled; open the file again`. **Rust:** `CANCELLED`, `Error::is_cancelled`, `Session::is_spent`, `render_half_stoppable`, `render_region_stoppable`, `stop_trampoline` (`catch_unwind`, a panic means stop). **Finding 1: a stop during LibRaw's work spent the session in every one of the 24 stopped runs** (`raw_alloc` NULL after the cancel), so each such stop costs a reopen (about 0.3 s for 61 Mpix) if the image is still wanted. **Finding 2: the handler is called only between LibRaw's stages**, so a stop is felt late: a whole-image PPG render on the 61 Mpix ARW needs 213 ms from a stop at 10 %, and the Fujifilm RAF 1205 ms (its demosaic is one stage); stops late in a render (90 %) found the render finished anyway for both ARW sizes. `setCancelFlag()` (LibRaw's `checkCancel` inside loops) is the owner's question (Not in this task). **Finding 3 (not caused by this task): a half-size render of an X-Trans file (`_DSF4243.RAF`) is not deterministic**: two fresh sessions differ in about half the bytes (1 715 915 and 3 147 708 of 19.5 M in two runs); a whole-image PPG render of the same file, and every ARW render, are identical; `stop.rs` compares only the size there. Needs a place: known-limits or a task (owner). **Debug run of the other tests** (`cargo test -p pw-libraw --test stop -- --ignored --test-threads=1 --skip timing_of_a_stop`): 4 passed. **Measurement** (CPU Intel Core i7-9700K 3.60 GHz; `cargo test --workspace --release -- --ignored --nocapture timing_of_a_stop`; unstopped = median of 3; per stop point 3 runs; times stop to return): TN005701.ARW (61 Mpix) WholePpg unstopped 902.6 ms: 10 % (90.3 ms) cancelled 3, finished 0, spent 3, median 213.3 / largest 216.7 ms; 50 % (451.3 ms) cancelled 3, spent 3, median 43.7 / largest 283.1; 90 % (812.3 ms) cancelled 0, finished 3, spent 0, median 134.0 / largest 174.2. TN005701.ARW Half unstopped 219.2 ms: 10 % (21.9) cancelled 3, spent 3, 22.5 / 23.4; 50 % (109.6) cancelled 3, spent 3, 61.7 / 63.5; 90 % (197.2) finished 3, spent 0, 20.9 / 22.6. TN100423.ARW (50 Mpix) WholePpg unstopped 1102.4 ms: 10 % (110.2) cancelled 3, spent 3, 260.5 / 260.9; 50 % (551.2) cancelled 3, spent 3, 56.5 / 302.0; 90 % (992.2) finished 3, spent 0, 97.1 / 155.7. TN100423.ARW Half unstopped 258.5 ms: 10 % (25.9) cancelled 3, spent 3, 18.7 / 23.1; 50 % (129.3) cancelled 3, spent 3, 80.8 / 80.9; 90 % (232.7) finished 3, spent 0, 22.4 / 24.3. _DSF4243.RAF WholePpg unstopped 1609.9 ms: 10 % (161.0) cancelled 3, spent 3, 1205.0 / 1221.8; 50 % (804.9) cancelled 3, spent 3, 560.2 / 573.4; 90 % (1448.9) cancelled 3, spent 3, 45.0 / 69.1. _DSF4243.RAF Half unstopped 110.1 ms: 10 % (11.0) cancelled 3, spent 3, 34.3 / 34.8; 50 % (55.0) cancelled 3, spent 3, 35.6 / 35.6; 90 % (99.1) finished 3, spent 0, 10.7 / 11.3. Above 100 ms from stop to return: the whole-image PPG renders (up to 1.2 s on the RAF), so the owner decides whether T328 goes on (W5-Q1). `.\tools\check.ps1` all OK; a clippy `redundant_closure` it first showed was fixed. Extra code review of the shim and `lib.rs` still to do (README) |
| 2026-11-25 | Open | Review (reviewer, and the extra code review by a fresh agent against LibRaw 0.22.3's sources): no bug found; the shim, the trampoline and the `ctx` pointer are sound, no exception leaves an `extern "C"` function (AHD's progress call inside its OpenMP region runs only on thread 0 and throws after the region). `.\tools\check.ps1` all OK (2700 passed, 86 ignored); the reviewer's own run of the four tests of point 3 (`cargo test --workspace -- --ignored a_stop_`): 4 passed, every stop during LibRaw's work spent. The agent ran `cargo test -p pw-libraw`, which `AGENTS.md` §5 forbids: use `--workspace` with a test-name filter. **Sent back, all five points approved by the owner on 2026-11-25** ("Ja, alla fem"): (1) LibRaw's exception handler calls `recycle()` on *every* exception in `dcraw_process`, not only a cancel (IO_CORRUPT, out of memory, the X-Trans case of the comment above `PWLR_XTRANS_*`); mark the session spent after *any* failed `dcraw_process` that leaves `imgdata.rawdata.raw_alloc` NULL, and change the spent text so it fits both (it must still start with `spent:` and not with `cancelled`); the doc comments of `is_spent` and the renders say so; a test of a non-cancel failure that spends the session if one can be made from a file in `PW_TESTDATA` or a generated file, else the report says why not. (2) `timing_of_a_stop`: only cancelled runs go into the stop-to-return median and largest; a finished-anyway run is counted and, if wanted, printed as its own figure (the remaining render time), never mixed in; a stop set after the render returned (the race between `returned` and `done`) counts as not set. (3) The thread that sets the stop does not busy-wait for the whole delay (it takes a core from LibRaw): it sleeps until shortly before the delay and spins only the last ~1 ms. (4) The doc comments (`render_half_stoppable`, `render_region_stoppable`, the shim's comment at the cancel) and `stop.rs`'s module comment say what was found: in LibRaw 0.22 a stop while LibRaw works always spends the session; the not-spent branch of `check_after_cancel` stays as a defence and says it is not expected. (5) `a_stop_from_another_thread_ends_a_whole_image_ppg_render` accepts a render that finished anyway (its bytes then equal a fresh render's), as `stop_half_way` does, since an X-Trans PPG demosaic has no progress call and a stop may come after the last one. Then run `.\tools\check.ps1`, the four tests of point 3 with `--workspace`, and **the timing of point 5 again (a second `--release` run, approved by the owner on 2026-11-25)**; the new figures replace the old ones in the report. `docs/test-inventory.md` follows any new test. Finding 3 (X-Trans half-size not deterministic) and the questions for the owner (T328, `setCancelFlag`) are taken up at the Done, not by the agent |
| 2026-11-25 | Ready for review | Sent-back points done. (1) The shim marks the session spent after **any** failed `dcraw_process` that leaves `raw_alloc` NULL; the spent text is now `spent: a render failed or was stopped and LibRaw dropped the RAW data; open the file again` (still `spent:`, not `cancelled`); doc comments of `is_spent` and the stoppable renders say so. **No test of a non-cancel failure that spends the session:** the only such case known is X-Trans below 512 px, and the shim refuses that before LibRaw is called (T197); no file in `PW_TESTDATA` and no generated file I could make fails inside `dcraw_process`, and a corrupt file is refused at the open. (2) `timing_of_a_stop`: only cancelled runs go into the stop-to-return median/largest; a finished-anyway run is counted and its render time left at the stop printed apart; a stop set after the return counts as not set. (3) The stop thread sleeps to 1 ms before the delay and spins only then. (4) Doc comments, the shim's comment and `stop.rs`'s module comment say a stop while LibRaw works spends the session in LibRaw 0.22; the not-spent branch stays as a defence. (5) The whole-image PPG test now goes through `stop_half_way` and accepts a render that finished anyway. **Runs:** `.\tools\check.ps1` all OK (2700 passed, 86 ignored, no clippy warning); `cargo test --workspace -- --ignored --nocapture --test-threads=1 a_stop_`: 4 passed (every stop spent the session). **Second `--release` timing (i7-9700K; replaces the first; times in ms; unstopped median of 3, then per stop point: cancelled/finished/spent; stop-to-return median/largest of the cancelled runs; for finished-anyway the median time left):** TN005701.ARW (61 Mpix) WholePpg 960.6: 10 % (96.1) 3/0/3, 222.7/223.9; 50 % (480.3) 3/0/3, 274.1/277.3; 90 % (864.5) 0/3/0, left 88.6. Half 239.5: 10 % (24.0) 3/0/3, 38.4/48.4; 50 % (119.8) 3/0/3, 71.5/73.0; 90 % (215.6) 0/3/0, left 14.1. TN100423.ARW (50 Mpix) WholePpg 1159.2: 10 % (115.9) 3/0/3, 271.6/276.7; 50 % (579.6) 3/0/3, 349.1/349.7; 90 % (1043.3) 0/3/0, left 119.0. Half 286.9: 10 % (28.7) 3/0/3, 45.3/45.8; 50 % (143.4) 3/0/3, 87.6/93.2; 90 % (258.2) 0/3/0, left 22.2. _DSF4243.RAF WholePpg 1715.9: 10 % (171.6) 3/0/3, 1296.7/1493.4; 50 % (858.0) 3/0/3, 551.7/579.6; 90 % (1544.3) 1/2/1, 44.8/44.8, left 66.2. Half 113.6: 10 % (11.4) 3/0/3, 35.6/35.7; 50 % (56.8) 3/0/3, 35.6/40.1; 90 % (102.2) 0/3/0, left 9.8. Every stop that took effect spent the session; stops at 90 % were too late to take effect |
| 2026-11-25 | Done | Second review (reviewer): all five points done as asked; no test of a non-cancel failure accepted (none can be made, see known-limits). The reviewer fixed two shim comments that still named only a stop (`pwlr_render`'s header, `pwlr_spent`) and wrapped the long `PWLR_SPENT_TEXT` line. `.\tools\check.ps1` all OK (2700 passed, 86 ignored, no clippy warning); the reviewer's run of the four tests of point 3 with `--workspace`: 4 passed, every stop spent. **Owner's decision 2026-11-25** ("Ja, bara när bilden lämnas"): the chain goes on, and T328 stops a render only when its image is no longer wanted, not for a new exposure value of the same image; written in ADR 0027 (after W5-Q1's chain). Leftovers: `setCancelFlag()` in `backlog.md` (Technical work); the X-Trans half-size render that is not deterministic, and the untested non-cancel spent session, in `known-limits.md` (LibRaw). No user-visible change, so no `CHANGELOG.md` line |
