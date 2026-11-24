# T311. ADR 0027 W1: pw-resample's output rows split over threads — Open

- **Why:** ADR 0027 W1-Q2 and T309 point 10: the reduction must fit in about 100 ms. That
  threshold is T309's: at about 7 images/s (V4), a neighbour's half-size render and its reduction
  must fit in about 140 ms. T307's timing was 440–443 ms on one thread (4784×3188 → 3241×2160,
  8 bit, Intel Core i7-9700K, 8 cores). So the output rows are split over threads with
  `std::thread::scope` (W1-Q2: no new dependency). The owner decided on 2026-11-24 that this is a
  task of its own, before T309, so T309 does not grow by a third crate. The chain: T307 → T308 →
  **T311** → T309 (Blocked until this task is Done).
- **What the reviewer checked (2026-11-24):**
  - `crates/pw-resample/src/lib.rs` (T307): `reduce_rgb8`/`reduce_rgb16` call the generic
    `reduce`, which checks the sizes, copies on the same size, and otherwise calls `filter_linear`
    with an `emit` closure that extends the output `Vec`. `filter_linear` computes the horizontal
    and vertical taps itself (`taps`), walks the vertical taps top to bottom with a ring of
    horizontally filtered rows, and gives each output row's linear values to `emit`. The test-only
    `reduce_linear_rgb8/16` call `filter_linear` too. Each output row depends only on its own
    vertical taps and the source rows they name, so the rows can be computed in any order and
    give the same bits.
  - `pw-viewcore` runs two workers (`core.rs`: an interactive worker and a prefetch worker), so
    two reductions can run at the same time.
  - `std::thread::Builder::spawn_scoped` is stable (Rust 1.63; the workspace is on 1.99) and
    returns `io::Result<ScopedJoinHandle>`; it takes its closure by value and drops it on `Err`.
  - The tests of T307 are in `crates/pw-resample/src/tests.rs`; the timing test is
    `timing_of_a_61_mpix_half_size_render_reduced_to_fit`.
- **Crates:** `pw-resample`. **Files:** `crates/pw-resample/src/lib.rs`,
  `crates/pw-resample/src/tests.rs`, `docs/architecture.md` (point 7), `docs/test-inventory.md`
  (rule 12), and this task's own file (status and log).
- **What:**
  1. **The public functions stay as they are:** the same signatures, the same `None` cases, the
     same results to the bit. Only the work inside is split.
  2. **The private function** `reduce_with_threads(src, size, threads, decode, encode)` does the
     work; `reduce` checks and copies as today and then calls it with the number of point 4.
     `threads` is used as given, except: 0 counts as 1, and it is at most `out_h`. The rows are
     split as evenly as possible into exactly that many bands of contiguous output rows (their
     sizes differ by at most one row; never an empty band).
  3. **The bands.**
     - `filter_linear` takes the precomputed horizontal taps and one band's slice of the vertical
       taps (`&[Taps]`) instead of computing them; the taps are computed once per reduction and
       shared by every band. `reduce_linear_rgb8/16` are changed to match (still one band).
     - Each band runs today's row loop with its own ring of rows and its own decoded source row,
       beginning its ring at its first row's `start`; the source rows where two bands' windows
       overlap (about one window length) are filtered horizontally by both.
     - The output is allocated once at full length (`vec![encode(0.0); out_len]`, so `T` needs no
       `Default`) and split into the bands' disjoint parts (`split_at_mut`, with sizes from
       `checked_mul`; a size is never 0, so `chunks_mut(0)` cannot occur). Each band's `emit`
       encodes its rows straight into its own part (for example through
       `chunks_exact_mut(out_row_len)`). No band's result is copied into the output afterwards,
       and no whole intermediate image is made.
     - With N bands, N − 1 are started on threads and the last runs on the calling thread. With
       one band no thread is started.
     - Bounds: `T: Copy + Send + Sync`, `decode: Fn(T) -> f32 + Sync`,
       `encode: Fn(f32) -> T + Sync`; `emit` stays `FnMut`, one per band.
  4. **The number of threads** for the public functions, from a function of its own that takes
     the number of cores as an argument: `cores` (from `std::thread::available_parallelism()`,
     1 if it returns an error), at most 8, and at most `⌊out_h / 32⌋` but at least 1, so every
     band has at least 32 output rows. Examples: (8 cores, `out_h` 1) → 1, (8, 63) → 1,
     (8, 64) → 2, (8, 10 000) → 8, (16, 10 000) → 8, (1, 10 000) → 1, (0, 10 000) → 1. Write in a
     comment why the cap is 8: `pw-viewcore` has two workers that may reduce at the same time.
  5. **Threads without a panic path:** bands are started with
     `std::thread::Builder::new().spawn_scoped(scope, …)`, not `scope.spawn`, which panics when
     the system cannot start a thread. If a spawn fails, the scope is finished (the bands already
     started are joined) and the whole reduction is done again on the calling thread as one band
     (a failed spawn drops its closure and with it the band's part of the output, so that band
     cannot be run on its own). A `join` that returns `Err` (only if a band panicked, which
     non-test code does not do) makes the reduction return `None`. One band's `filter_linear`
     returning `None` makes the reduction return `None`. Both paths are defensive and need no
     test. No `unsafe`, `.unwrap()`, `.expect()`, `panic!` or unchecked indexing in non-test code
     (`AGENTS.md` §3).
  6. **The time measurement again:** the timing test calls `reduce_rgb8` as before and also prints
     the number of threads (the function of point 4 with `available_parallelism`); it still
     asserts only the output size. Run
     `cargo test --workspace --release -- --ignored --nocapture timing_of_a_61_mpix` as in T307
     (approved by the owner for this measurement, 2026-11-24; if `rawstride.exe` is running from
     `target\release`, ask the owner to close it). Put the printed time, the number of threads and
     the PC's CPU in the log row. If the time is still above 100 ms, say so in the log and do not
     tune further (W1-Q2: the owner decides).
  7. **`docs/architecture.md`:** the `pw-resample` row also says "the output rows split over up
     to 8 threads".
- **Tests to add** (in `crates/pw-resample/src/tests.rs`, for both 8 and 16 bit):
  - **The same bits on any number of threads:** for the sizes 23×17 → 9×6, 4784×40 → 3241×27
    (wide, few rows), 97×300 → 40×200 (many rows), 9×7 → 4×3 (fewer rows than 7 or 8 threads),
    50×40 → 20×40 (vertical factor 1: one row per window, no overlap) and 40×50 → 40×20
    (horizontal factor 1), `reduce_with_threads` with 1, 2, 3, 7 and 8 threads gives exactly the
    result of 1 thread (`assert_eq!`, not within ±1).
  - **The public functions equal one thread:** `reduce_rgb8`/`reduce_rgb16` on 97×300 → 40×200
    equal `reduce_with_threads` with 1 thread.
  - **The number of threads (point 4):** every example of point 4.
  - Every test of T307 stays as it is and passes (rule 5).
- **Not in this task:** using the crate in the view (T309); a thread pool or a new dependency;
  splitting the columns; SIMD; any change to the taps, the curve or the clamp; a test of the
  failed-spawn and failed-join paths (point 5).
- **Rules:** `docs/tasks/README.md` rules 1–12 (rule 13 does not apply: no text the user sees); no
  `Cargo.toml` change (no dependency, the root untouched).
- **Done when:** `.\tools\check.ps1` is all OK, the timing of point 6 is in the log, and
  `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Open | Added (ADR 0027 W1-Q2; T307's timing 440 ms above T309's 100 ms; a task of its own before T309, owner 2026-11-24) |
| 2026-11-24 | Open | Cold read: the output allocated at full length with `encode(0.0)` and split with `split_at_mut` (`T` has no `Default`); a failed spawn redoes the whole reduction on one band (the closure, and the band's output part, is dropped); the bounds written out; the thread count `⌊out_h/32⌋` (the ceiling broke "at least 32 rows") with an example per core count; the private function's `threads` used as given (else the tests ran one band); even bands, N − 1 spawned and the last on the calling thread; `filter_linear` takes the taps and a band slice; test sizes with a vertical and a horizontal factor 1; the overlap "about one window length" |
