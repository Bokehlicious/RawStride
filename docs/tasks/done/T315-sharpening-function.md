# T315. ADR 0027 W3: the sharpening function and its timing — Done

- **Why:** ADR 0027 §3.3 and W3-Q2, W3-Q3 (answered by the owner 2026-11-24, ADR 0027 §4.3).
  Sharpening in the view is an unsharp mask on luminance, applied to a view at screen resolution
  after the reduction. This task makes the function and its parameters as plain, testable Rust in
  `pw-viewcore`, and times it on screen-sized images. Nothing in the program uses it yet; the view
  core takes it up in T316. The first task of the chain **T315** → T316 → T317 → T318.
- **What the reviewer checked (2026-11-24):**
  - `crates/pw-viewcore/src/backend.rs`: `Rgb { width, height, data }` (8-bit RGB, interleaved,
    no padding); `Rgb::is_consistent()` checks `width × height × 3 == data.len()`.
  - `crates/pw-viewcore/src/scale.rs` is the model of a small public module: a module comment,
    `reduce(img: &Rgb, …) -> Option<Rgb>` that refuses an inconsistent input with `None`, and its
    own `#[cfg(test)] mod tests` at the end of the file.
  - `crates/pw-viewcore/src/lib.rs` declares `pub mod scale;` and re-exports the public types in
    `pub use` lines.
  - `crates/pw-resample/src/lib.rs` splits its output rows over threads with `std::thread::scope`
    (`available_cores`, `thread_count`, `MIN_ROWS_PER_BAND`, `MAX_THREADS`, and its handling of a
    thread that cannot be started); `crates/pw-resample/src/tests.rs` has an
    `#[ignore = "a time measurement; run with --release (ADR 0027 W1-Q2)"]` timing test.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/sharpen.rs` (new),
  `crates/pw-viewcore/src/lib.rs` (the module and the re-export), `docs/test-inventory.md`
  (rule 12: the `pw-viewcore` unit-test row gets `sharpen.rs` in its file list, its count and one
  more ignored test), and this task's own file (status and log).
- **What:**
  1. **The parameters:** in `sharpen.rs`, a public type
     `Sharpening { pub amount: u16, pub radius_tenths: u8, pub threshold: u8 }`, deriving
     `Clone, Copy, Debug, PartialEq, Eq, Hash` (T316 puts it in a hashed job key, so no floats):
     - `amount` in percent, **10–300**; `radius_tenths` the radius in tenths of a pixel, **3–20**
       (0.3–2.0 px); `threshold` in levels of 0–255, **0–20** (W3-Q7's ranges);
     - associated `pub const`s for the three ranges (`Sharpening::AMOUNT_RANGE`,
       `RADIUS_TENTHS_RANGE`, `THRESHOLD_RANGE`, as `RangeInclusive`), `Sharpening::DEFAULT` = amount 60, radius 6
       (0.6 px), threshold 2 (§3.3), and `impl Default` giving it;
     - `pub fn is_valid(self) -> bool` (all three inside their ranges), `pub fn clamped(self) ->
       Self` (each value moved into its range), and `pub fn radius(self) -> f32`
       (`radius_tenths / 10`).
  2. **The function:** `pub fn sharpen(img: &Rgb, s: Sharpening) -> Option<Rgb>`, the unsharp mask
     of W3-Q2:
     - `None` for an inconsistent image or an `s` that is not valid; nothing panics (an image of
       0×0, 0×5 or 5×0 with empty data is consistent and gives a copy);
     - the luminance of each pixel, `Y′ = 0.2126 R′ + 0.7152 G′ + 0.0722 B′`, of the 8-bit values
       as they are (sRGB-encoded, not linear light), in `f32`;
     - `Y′` blurred with a **separable Gaussian** (rows, then columns), sigma = `s.radius()`, the
       kernel `2·half + 1` taps wide with `half = (3·radius_tenths + 9) / 10` in integers (⌈3σ⌉
       exactly), its weights normalised to 1; at the edges the edge pixel is
       repeated, so an image of 1×1, 1×n or n×1 works;
     - per pixel `d = Y′ − blur`: where `|d| < threshold` the pixel is copied unchanged; elsewhere
       the offset `k = round(amount / 100 × d)` is worked out **once** and added to each of R′, G′
       and B′ in a wider type (`i32`), each clamped to 0–255, never wrapped (the same `k` for all
       three, so no colour fringe is made and a grey stays grey);
     - the output has the input's size.
  3. **Its speed (W3-Q3):** first written on one thread. The timing test of the tests below is run
     in a release build (point 4). If the **median** time at radius 2.0 on 3840×2160 is **more than
     10 ms**, the rows are split over threads with `std::thread::scope` in this task, as
     `pw_resample::reduce_rgb8` does (its rules for the number of threads and for a thread that
     cannot be started, written again here, since `available_cores`, `thread_count`,
     `MIN_ROWS_PER_BAND` and `MAX_THREADS` are private there), in both passes (the rows of the
     horizontal blur, then the rows of the vertical blur and the output), through a private
     `sharpen_with_threads(img, s, threads)` that `sharpen` calls (as `reduce_with_threads`), and
     the timing is run again. If it is still above 10 ms, report it in the log and do not
     optimise further: the owner decides. The result must be the same, byte for byte, with one thread and
     with several.
  4. **The measurement:** run
     `cargo test --workspace --release -- --ignored --nocapture timing_of_sharpening`
     (a debug build measures the wrong thing; **the `--release` run is approved by the owner for this
     one measurement (2026-11-24)**, as in T307 and T312; it builds in the project's `target\release`; if
     `rawstride.exe` is running and locks it, ask the owner to close it). Put the printed lines in
     the log row, with the CPU (`Get-CimInstance Win32_Processor`), and say whether point 3's
     split was needed.
  5. **`lib.rs`:** `pub mod sharpen;` beside `pub mod scale;`, and `Sharpening` re-exported from
     the crate root.
- **Tests to add** (in `sharpen.rs`'s own `#[cfg(test)] mod tests`):
  - A uniform image (any colour) is returned unchanged, for the default and for radius 2.0.
  - A vertical step from grey 100 to grey 150: after sharpening the pixels next to the step on
    the dark side are darker than 100 and on the bright side brighter than 150; pixels far from
    the step are unchanged; every pixel stays grey (R = G = B).
  - A coloured step (for example (200, 60, 40) next to (120, 30, 20)): where no value is clamped,
    each pixel's R − G and G − B are the same as before (the same value added to the three).
  - Threshold: a grey step of 10 levels (100 to 110) with amount 100, radius 0.6 and threshold 12
    is unchanged; with threshold 0 the pixels next to the step change.
  - Amount: a larger amount gives a larger overshoot at the same step.
  - An image of 1×1, 1×5, 5×1, 0×0, 0×5 and 5×0 is returned with its size and does not panic; a uniform one
    is unchanged.
  - Clamping: a step from 250 to 255 with amount 300 gives no value above 255 and none wrapped.
  - An inconsistent image (data one byte short) and each parameter one step outside its range give
    `None`; `clamped()` moves each into its range and `is_valid()` is then true;
    `Sharpening::default()` is 60, 6, 2.
  - If point 3's split is made: `sharpen_with_threads` gives the same output, byte for byte, with
    one thread and with several, on an image of more rows than one band.
  - `#[ignore = "a time measurement; run with --release (ADR 0027 W3-Q3)"] fn
    timing_of_sharpening()`: a synthetic image of 3840×2160, one of 1920×1200 and one of
    4800×3200 (a 61 Mpix image's half-size view, which T316 sharpens at 50 %; a pattern with
    edges, as `pw-resample`'s `image8`), each sharpened with the default and with radius 2.0, five
    runs, the median and the maximum printed in ms with the size and the radius. It asserts only
    the output's size.
- **Not in this task:** the view core (T316); the key, the mark and the settings (T317, T318).
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text is added). No
  new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W3, §4.3); Open after its cold read, the chain review and the owner's approval of the `--release` run |
| 2026-11-24 | Blocked | Cold read: the range constants and the methods named as associated and `pub`; the kernel's half width in integers; the offset rounded once and added in `i32` (so R − G stays exact); the 10 ms limit is the median, the split is done in both passes through `sharpen_with_threads`, and above 10 ms after it the owner decides; the threshold test changed (a 3-level step gives no change at the defaults even with threshold 0) to a 10-level step with amount 100 and threshold 12 against 0; 0×5 and 5×0 added; `test-inventory.md`'s row named. Waits for the chain review and the `--release` approval |
| 2026-11-24 | Open | Chain review (whole chain T315–T318; noted here as the first task): the half-size view (4800×3200) added to the timing, since T316 sharpens it. The owner approved the `--release` run 2026-11-24. Set Open |
| 2026-11-24 | Ready for review | `sharpen.rs` (new): `Sharpening` with its ranges, `DEFAULT`, `is_valid`, `clamped`, `radius`; `sharpen` as written (luma, separable Gaussian with `half = (3·radius_tenths).div_ceil(10)`, which clippy asked for instead of `(3·r + 9) / 10`, the same number; edge repeated; one `k` per pixel in `i32`, clamped). `lib.rs`: `pub mod sharpen;` and `Sharpening` re-exported. 11 tests (10 + 1 ignored). CPU: Intel Core i7-9700K @ 3.60 GHz, 8 cores, 8 logical processors. **One thread** (`cargo test --workspace --release -- --ignored --nocapture timing_of_sharpening`): 3840×2160 r 0.6 median 129.6 ms, max 147.1; r 2.0 median 175.4, max 196.9; 1920×1200 r 0.6 36.0 / 36.2; r 2.0 48.1 / 48.2; 4800×3200 r 0.6 239.3 / 240.8; r 2.0 324.4 / 325.2. Above 10 ms, so **point 3's split was made**: `sharpen_with_threads` with `in_bands` (both passes; thread count and spawn-failure rule as `pw_resample`, `MAX_THREADS` 8, `MIN_ROWS_PER_BAND` 32), plus a byte-for-byte test (1 thread against 0, 2, 3, 7, 8, 300, 1000 on 37×300) and a thread-count test. **After the split** (same command, 8 threads): 3840×2160 r 0.6 median 21.2 ms, max 23.3; r 2.0 median 27.5, max 40.6; 1920×1200 r 0.6 6.5 / 9.2; r 2.0 7.4 / 9.2; 4800×3200 r 0.6 40.3 / 57.6; r 2.0 69.8 / 79.2. **Still above 10 ms at 3840×2160, radius 2.0 (27.5 ms): not optimised further, the owner decides.** Not tried (for the owner): the horizontal pass sums each pixel's taps one after another, which the compiler cannot vectorise (looping over taps outside and pixels inside keeps the same sum order and would vectorise); the luminance is worked out twice per pixel; one `Vec` per output row. `test-inventory.md`'s row 27: `sharpen.rs`, 208 tests, 1 ignored, and a description; the total on its line 4 was not changed (the row only, as the task names) |
| 2026-11-24 | Done | Reviewed: the diff holds only the named files; points 1–5 and every test asked for are there, plus the byte-for-byte test over threads and the thread-count test; no panic paths; `test-output.txt` (20:27, after the last change) all OK, `pw_viewcore` 207 passed and 1 ignored, no clippy warnings. The reviewer set the total on `test-inventory.md`'s line 4 (2640 passed, 81 ignored). Still above 10 ms after the split (27.5 ms at 3840×2160, 69.8 ms at 4800×3200, radius 2.0, 8 threads): the owner decided on 2026-11-24 (ADR 0027 §4.3, the note after the table) on a task of its own before T316, **T319** (the horizontal pass with the taps outside, no `Vec` per row, the same bytes). Noted, not changed: the luminance is worked out in both passes (T319 point 3); a failed spawn together with a failed band gives `None` instead of a retry, and the failed-spawn path has no test (as in T311). No hand check of its own (nothing in the program uses it; W3's hand check after T318). No CHANGELOG line. Approved by the owner |
