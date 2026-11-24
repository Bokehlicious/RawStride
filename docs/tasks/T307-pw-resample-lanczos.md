# T307. ADR 0027 W1: the crate pw-resample, a Lanczos-3 reduction in linear light — Open

- **Why:** ADR 0027 (accepted 2026-11-18), step W1, §3.2, A-Q7 and W1-Q1/W1-Q2 (answered by the
  owner 2026-11-24, ADR 0027 §4.1): the view's box filter is replaced by a separable Lanczos-3
  filter in linear light, in a workspace crate of its own without dependencies, so the view
  (`pw-viewcore`) and later the export (`pw-export`, ADR 0025 X2) use the same filter. This task
  makes the crate and its tests only; nothing uses it yet. The chain: T307 → T308 (the conversion
  to the monitor profile moved after the reduction; Blocked until this task is Done) → T309 (the
  filter used in the view; Blocked until T308 is Done).
- **What the reviewer checked (2026-11-24):**
  - The workspace takes every folder under `crates/` as a member (`Cargo.toml`, `members =
    ["crates/*", …]`); internal crates are also listed under `[workspace.dependencies]`, for example
    `pw-tiff = { path = "crates/pw-tiff" }`, and every crate's `Cargo.toml` uses the
    `version.workspace = true` … `authors.workspace = true` lines and `[lints] workspace = true`
    (see `crates/pw-tiff/Cargo.toml`).
  - The view's images are interleaved 8-bit RGB, three bytes per pixel, row by row, no padding
    (`pw-viewcore` `backend.rs`, `Rgb`). The box filter it replaces is `pw-viewcore` `scale.rs`,
    `downscale`; it stays until T309.
  - Unit tests live in a crate's `src/tests.rs` (`#[cfg(test)] mod tests;` in `lib.rs`).
- **Crates:** `pw-resample` (new). **Files:** `crates/pw-resample/Cargo.toml` (new),
  `crates/pw-resample/src/lib.rs` (new), `crates/pw-resample/src/tests.rs` (new), `Cargo.toml`
  (the root: one line under `[workspace.dependencies]`, point 1), `Cargo.lock` (written by cargo),
  `docs/architecture.md` (point 6), `docs/test-inventory.md` (rule 12), and this task's own file
  (status and log).
- **What:**
  1. **The crate.** `crates/pw-resample/Cargo.toml`: `name = "pw-resample"`, `description =
     "Image reduction with a separable Lanczos-3 filter in linear light (ADR 0027)"`, the workspace
     lines as `pw-tiff` has them, `[lints] workspace = true`, **no `[dependencies]` and no
     `[dev-dependencies]`** (A-Q7: a crate without dependencies). In the root `Cargo.toml`, under
     `[workspace.dependencies]` after `pw-imagecheck`: `pw-resample = { path =
     "crates/pw-resample" }` (approved by the owner 2026-11-18, A-Q7). No other crate depends on
     it in this task.
  2. **The public functions** in `lib.rs`, with a module comment naming ADR 0027 §3.2, A-Q7 and
     W1-Q1:
     ```rust
     pub fn reduce_rgb8(src: &[u8], width: u32, height: u32, out_w: u32, out_h: u32) -> Option<Vec<u8>>
     pub fn reduce_rgb16(src: &[u16], width: u32, height: u32, out_w: u32, out_h: u32) -> Option<Vec<u16>>
     ```
     Interleaved RGB, three samples per pixel, row by row, no padding. They return `None` (never
     panic) when `width`, `height`, `out_w` or `out_h` is 0, when `out_w > width` or
     `out_h > height` (no enlargement), when `src.len()` is not `width × height × 3`, or when a
     size computation would overflow (checked arithmetic). The same size returns a copy of `src`.
  3. **The filter** (W1-Q1):
     - Lanczos-3: `L(x) = sinc(x)·sinc(x/3)` for `|x| < 3`, else 0, `sinc(x) = sin(πx)/(πx)`,
       `sinc(0) = 1`.
     - Separable: a horizontal pass, then a vertical pass, each with its own reduction factor
       (`f = out_w/width` and `out_h/height`, so a non-uniform reduction works). The centre of
       output pixel `i` is source position `c = (i + 0.5)/f − 0.5`; source pixel `j` gets the
       weight `L((j − c)·f)` for `|j − c| < 3/f`, else none (the kernel stretched to `3/f` source
       pixels each side). A pass whose factor is 1 copies its input (no tiny off-centre weights
       from `sin(πk)` in `f32`).
     - The weights of each output pixel are computed once per pass (per output column or row),
       not per pixel, and **normalised to sum 1**. Source pixels outside the image are left out
       (not mirrored, not repeated), and the remaining weights renormalised, so the edges, a
       1×n or n×1 image and a 1×1 output all work.
     - In **linear light**: each sample is made linear with the sRGB transfer curve (IEC
       61966-2-1: `v ≤ 0.04045 → v/12.92`, else `((v + 0.055)/1.055)^2.4`, with `v` the sample
       divided by 255 or 65535) and filtered in `f32`; the horizontally filtered rows are kept as
       linear `f32`, not quantised between the passes. A private function returns the filtered
       linear values before the clamp; `reduce_rgb8/16` use it, then **clamp each value to
       0.0–1.0**, encode it with the inverse curve, and round to 0–255 (0–65535), so Lanczos'
       negative lobes never wrap and never reach the curve as a negative number (NaN). The 8-bit
       decode uses a 256-entry table, the 16-bit decode a 65536-entry table (built per call, or
       once with `std::sync::LazyLock`, which is std); the encode may use a table or the formula,
       as long as the tests under **Tests to add** hold (a coarse encode table breaks the exact
       8-bit round trip of dark values; the formula in `f32` keeps it).
     - **Memory:** the horizontally filtered rows are kept only while the vertical kernel needs them
       (a ring of rows), not as a whole intermediate image; besides the output, the filter holds at
       most about `(2·⌈3/f⌉ + 2)` such rows, or the image's height if that is smaller. Write this
       in a comment.
     - Plain Rust, no `unsafe`, no `.unwrap()`/`.expect()`/`panic!`/unchecked indexing in non-test
       code (`AGENTS.md` §3); iterators or `get` instead of indexing.
  4. **A time measurement**, an `#[ignore]` test in `tests.rs` named
     `timing_of_a_61_mpix_half_size_render_reduced_to_fit` that makes a synthetic 4784×3188 8-bit
     image (a pattern, not flat), reduces it to 3241×2160 (about the half-size render of a
     9568×6376 image at fit on a 2160-px-high view), and prints the time. Run it once with
     `cargo test --workspace --release -- --ignored --nocapture timing_of_a_61_mpix` (a debug
     build's time means nothing) and put the printed time and the PC's CPU in the log row (W1-Q2:
     T309 decides from it whether the rows are split over threads). It asserts only the output
     size. **The `--release` run is approved by the owner for this one measurement
     (owner, 2026-11-24)**; it builds in the project's `target\release`; if `rawstride.exe`
     is running from there, ask the owner to close it.
  5. **No other code changes.** `pw-viewcore`, `pw-tiff` and `pw-thumbcache` are not touched.
  6. **`docs/architecture.md`:** one row in the crate table after `pw-colour`: `pw-resample` — image
     reduction with a separable Lanczos-3 filter in linear light, 8 and 16 bit, without
     dependencies; shared by the view and the export (ADR 0027 §3.2, A-Q7).
- **Tests to add** (in `crates/pw-resample/src/tests.rs`, each for both `reduce_rgb8` and
  `reduce_rgb16` unless it says otherwise):
  - `None` for each refused input of point 2 (a 0 size each way, a larger output each way, a buffer
    one sample short and one sample long); the same size gives back the input unchanged.
  - A flat image keeps its value exactly: for **every** 8-bit value 0–255, a 9×7 flat image reduced
    to 4×3 gives that value in every sample (the curve's round trip is exact for 8 bit); for 16 bit,
    a sample of values (0, 1, 1000, 32768, 65534, 65535) within ±1.
  - Linear light: a 2×2 image of black and white pixels in a checkerboard reduced to 1×1 gives the
    linear mean, 188 in 8 bit (linear 0.5 encodes to 187.51), not 128; in 16 bit the encoding of
    0.5 within ±2.
  - Clamping: a 12×1 image, columns 0–5 black and 6–11 white, reduced to 8×1 (f = 2/3): the
    private unclamped function gives output column 5 above 1.0 and column 2 below 0.0, and the
    clamped result is exactly the maximum (255 / 65535) at column 5 and exactly 0 at column 2 (the
    over- and undershoot clamped, not wrapped). A comment says why Lanczos overshoots there.
  - Symmetry: a left–right mirrored input gives the mirrored output; the same for top–bottom
    (within ±1, unless the weights are computed symmetrically, then exactly); the three channels
    are filtered independently (an image with different R, G and B patterns gives each channel's
    own result as when it is the only non-zero channel).
  - Edges and small sizes: 7×5 → 3×2, 1×9 → 1×4, 9×1 → 4×1, 5×5 → 1×1 return the right sizes and
    a flat image stays flat; a 1×1 input to 1×1 is a copy.
  - The 8-bit and 16-bit versions agree: an 8-bit image and the same image scaled to 16 bit
    (`v·257`) give results within ±1 of each other after scaling back (`round(r16 / 257)`).
  - The `#[ignore]` timing test of point 4.
- **Not in this task:** using the crate anywhere (T309); the conversion to the monitor profile
  (T308); threads (T309 decides, W1-Q2); other transfer curves (ADR 0025 X2, W1-Q1); `pw-tiff`'s
  box filter and the thumbnails (W1-Q6, unchanged).
- **Rules:** `docs/tasks/README.md` rules 1–12; no dependency in the new crate; the only change to
  the root `Cargo.toml` is the one line of point 1.
- **Done when:** `.\tools\check.ps1` is all OK, the timing of point 4 is in the log, and
  `git status` and `git diff --stat` show only the files above (the new crate's files untracked).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-24 | Open | Added (ADR 0027 W1, W1-Q1/W1-Q2) |
| 2026-11-24 | Open | Cold read: the timing command gets `--nocapture` and the owner's approval of `--release`; the kernel's weight formula written out, a factor of 1 copies; clamp the linear value before the encode (no NaN), rows kept as `f32`; the clamp test's private unclamped function and its exact columns (5 over, 2 under, worked through); the mirror test ±1; the checkerboard tightened to 188; the ring of rows capped by the height; `git status` in Done when; `--release` approved by the owner 2026-11-24 |
| 2026-11-24 | Open | Chain review (T307–T309): the APIs and the core's order match across the tasks, and each task leaves a working app. Fixed: T308's core test renders a flat colour so T309 does not break it; T309 treats a view-size change as a gesture step (else every resize frame plans a render), names `docs/architecture.md`, the test-inventory removals, the threshold for threads (point 10), the extra code review, the CHANGELOG line and the ADR 0016/0009 notes at Done, and more hand-check points; ADR 0027 §4.1's chain sentence and the W1 row in `plans/` (a follow-up if `latency.ps1` fails) |
