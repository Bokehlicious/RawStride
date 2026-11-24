# T319. ADR 0027 W3: the sharpening's two blur passes made faster — Open

- **Why:** ADR 0027 W3-Q3 and the owner's decision of 2026-11-24 (ADR 0027 §4.3, the note after
  the table). T315's `sharpen` was above 10 ms on one thread, so its rows were split over threads;
  on 8 threads (Intel Core i7-9700K, `--release`, median) it takes 21.2 ms at radius 0.6 and
  27.5 ms at radius 2.0 on 3840×2160, 6.5 / 7.4 ms on 1920×1200, and 40.3 / 69.8 ms on 4800×3200,
  the half-size view T316 sharpens at 50 %. Two causes were found and not tried in T315: the
  horizontal blur sums each pixel's taps one after another in a `fold`, which the compiler cannot
  vectorise, and a new `Vec` is made for every row in both passes. This task removes them, with
  the same output to the byte. The chain is now T315 → **T319** → T316 → T317 → T318.
- **What the reviewer checked (2026-11-24):**
  - `crates/pw-viewcore/src/sharpen.rs` (T315): `sharpen_with_threads` runs two passes through
    `in_bands(out, row_len, bands, work)`, whose `work(first_row, part)` is called once per band.
    Pass 1 calls `blur_row(src_row, kernel, out_row)` for each row: it builds a new `padded`
    `Vec<f32>` of the row's luminance with `half` copies of the edge value at each end, then for
    each output pixel `window.iter().zip(kernel).fold(0.0, |a, (v, k)| a + v * k)`. Pass 2 calls
    `blur_column(rows, width, height, y, kernel)` for each row, which returns a new `Vec<f32>` of
    `width` (its loop already has the taps outside and the pixels inside), and then
    `apply_row(src_row, &blur, s, out_row)`.
  - Rust does not fuse `a + v * k` into one instruction (no FMA without asking for it), so
    `out[x] = 0.0; out[x] += v[x + i] * k[i]` for `i = 0, 1, …` gives each pixel the same sums in
    the same order as the `fold`, and the same bits.
  - The tests of T315 are in `sharpen.rs`'s `mod tests`; `threads_give_the_same_bytes_as_one`
    compares 1 thread with others; `timing_of_sharpening` is the ignored timing test.
- **Crate:** `pw-viewcore`. **Files:** `crates/pw-viewcore/src/sharpen.rs`,
  `docs/test-inventory.md` (rule 12: the `pw-viewcore` unit-test row's count and description, and
  the total on its line 4), and this task's own file (status and log).
- **What:**
  1. **The horizontal pass:** the luminance of the row with its padding is written into one
     `Vec<f32>` made once per band (in `work`, reused for every row of the band, not one per
     row), and the output row is summed with the taps in the outer loop and the pixels in the
     inner loop: the output row is set to 0.0, then for each tap `i` in order, `out[x] +=
     padded[x + i] * kernel[i]` for every `x`. The sums and their order per pixel stay as in T315.
     The signature becomes `blur_row(src, kernel, padded: &mut Vec<f32>, out) -> Option<()>`
     (`padded` cleared and filled for each row), and its doc comment says so.
  2. **The vertical pass:** `blur_column` writes into a buffer of `width` made once per band and
     passed in, instead of returning a new `Vec` per row:
     `blur_column(rows, width, height, y, kernel, out: &mut [f32]) -> Option<()>`, its doc comment
     updated. Its order of taps stays as it is.
     **The reset to 0.0 is needed in both passes, not only tidy:** when a thread cannot be started,
     `in_bands` runs `work(0, out)` again over the whole output, on top of bands already written,
     so a sum that adds into what is there would be doubled. The buffers are made inside `work`
     (one per band), never shared between bands.
  3. **Nothing else changes:** the public `Sharpening` and `sharpen`, `sharpen_with_threads`,
     `in_bands`, `thread_count`, the kernel, `luma` and `apply_row` work as in T315; the luminance
     is still worked out in both passes (a plane of it would cost 4 bytes a pixel more). No
     `unsafe`, no new dependency, no SIMD by hand; the indexing stays checked (slices, `zip`,
     `get`), as in T315.
  4. **The measurement:** run
     `cargo test --workspace --release -- --ignored --nocapture timing_of_sharpening`
     (the `--release` run is approved by the owner for this measurement (2026-11-24), as in T315;
     it builds in the project's `target\release`; if `rawstride.exe` is running and locks it, ask
     the owner to close it). Put the printed lines in the log row, with T315's for comparison.
     Do not optimise further than points 1 and 2, whatever the result: the owner decides on it.
     Order: the measurement first, then the log row, then `.	ools\check.ps1` last (rule 6: its
     `test-output.txt` newer than the last change).
- **Tests to add** (in `sharpen.rs`'s `mod tests`):
  - A test-only copy of T315's two passes, `reference_sharpen(img: &Rgb, p: Sharpening)`: one thread, T315's
    `blur_row` with its `fold` and T315's `blur_column` returning a `Vec`, then `apply_row`, copied
    from the file as it is at the start of this task. `sharpen` gives the same bytes as
    `reference_sharpen` on `patterned` images of 37×300, 1×1, 1×5, 5×1 and 64×65, for
    `Sharpening::DEFAULT`, radius 2.0 and `s(300, 20, 0)`; it also asserts that the reference's
    output differs from the input (so the test compares a real sharpening).
  - T315's tests stay as they are and pass unchanged.
- **Not in this task:** the view core (T316); a plane of the luminance; threads beyond T315's
  rule.
- **Rules:** `AGENTS.md`; `docs/tasks/README.md` rules 1–12 (no user-visible text is added). No
  new dependency.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Blocked | Added (ADR 0027 W3-Q3; T315's 27.5 ms at 3840×2160 and 69.8 ms at 4800×3200 on 8 threads; a task of its own before T316, owner 2026-11-24); Open after its cold read |
| 2026-11-24 | Open | Cold read: the reset to 0.0 explained (a failed spawn runs the whole output again on top of written bands) and the buffers made per band; the new signatures of `blur_row` and `blur_column` given; the order measurement → log row → check; the reference's parameter named `p` (not hiding `s()`) and an assert that it changes the image. Set Open with T315's Done commit |
