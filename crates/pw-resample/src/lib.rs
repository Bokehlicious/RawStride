//! Image reduction with a separable Lanczos-3 filter in linear light (ADR 0027 §3.2).
//!
//! A workspace crate without dependencies (ADR 0027 A-Q7), so the view (`pw-viewcore`) and later
//! the export (`pw-export`, ADR 0025 X2) reduce images with the same filter. The filter follows
//! ADR 0027 W1-Q1:
//! - Lanczos-3, `L(x) = sinc(x)·sinc(x/3)` for `|x| < 3`, else 0;
//! - separable: a horizontal pass, then a vertical pass, each with its own reduction factor
//!   `f = out/in`; the kernel is stretched to `3/f` source pixels each side; a pass whose factor
//!   is 1 copies its input;
//! - the weights of each output column or row are computed once per pass and normalised to sum 1;
//!   source pixels outside the image are left out and the remaining weights renormalised;
//! - in linear light: the samples are decoded with the sRGB transfer curve (IEC 61966-2-1),
//!   filtered in `f32`, clamped to 0.0–1.0 and encoded again.
//!
//! Images are interleaved RGB, three samples per pixel, row by row, no padding.

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::sync::LazyLock;

/// The Lanczos window: `a = 3` lobes each side.
const LOBES: i128 = 3;

/// Reduces an 8-bit RGB image to `out_w` × `out_h`.
///
/// Returns `None` when a size is 0, the output is larger than the input in either direction, `src`
/// is not `width × height × 3` samples long, or a size computation overflows. The same size
/// returns a copy of `src`.
pub fn reduce_rgb8(src: &[u8], width: u32, height: u32, out_w: u32, out_h: u32) -> Option<Vec<u8>> {
    let table: &[f32] = &DECODE_8;
    reduce(
        src,
        Size::new(width, height, out_w, out_h),
        |v| table.get(usize::from(v)).copied().unwrap_or(0.0),
        // The value is rounded and within 0–255, so the cast is exact.
        |l| encode(l, 255.0) as u8,
    )
}

/// Reduces a 16-bit RGB image to `out_w` × `out_h`. The same rules as [`reduce_rgb8`].
pub fn reduce_rgb16(
    src: &[u16],
    width: u32,
    height: u32,
    out_w: u32,
    out_h: u32,
) -> Option<Vec<u16>> {
    let table: &[f32] = &DECODE_16;
    reduce(
        src,
        Size::new(width, height, out_w, out_h),
        |v| table.get(usize::from(v)).copied().unwrap_or(0.0),
        // The value is rounded and within 0–65535, so the cast is exact.
        |l| encode(l, 65535.0) as u16,
    )
}

/// The input and output sizes of one reduction.
#[derive(Debug, Clone, Copy)]
struct Size {
    width: u32,
    height: u32,
    out_w: u32,
    out_h: u32,
}

impl Size {
    fn new(width: u32, height: u32, out_w: u32, out_h: u32) -> Self {
        Self {
            width,
            height,
            out_w,
            out_h,
        }
    }

    /// Checks the sizes against `src_len` and returns the number of output samples.
    fn check(self, src_len: usize) -> Option<usize> {
        if self.width == 0 || self.height == 0 || self.out_w == 0 || self.out_h == 0 {
            return None;
        }
        if self.out_w > self.width || self.out_h > self.height {
            return None;
        }
        let src_samples = samples(self.width, self.height)?;
        if src_len != src_samples {
            return None;
        }
        samples(self.out_w, self.out_h)
    }
}

/// `width × height × 3`, or `None` on overflow.
fn samples(width: u32, height: u32) -> Option<usize> {
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(3)
}

/// Checks the sizes, then filters and encodes, or copies when the size is unchanged.
fn reduce<T: Copy>(
    src: &[T],
    size: Size,
    decode: impl Fn(T) -> f32,
    encode: impl Fn(f32) -> T,
) -> Option<Vec<T>> {
    let out_len = size.check(src.len())?;
    if size.out_w == size.width && size.out_h == size.height {
        return Some(src.to_vec());
    }
    let mut out = Vec::with_capacity(out_len);
    filter_linear(src, size, decode, |row| {
        // The clamp keeps Lanczos' over- and undershoot from wrapping and keeps negative values
        // away from the curve (a negative number to a fractional power is NaN).
        out.extend(row.iter().map(|&l| encode(l.clamp(0.0, 1.0))));
    })?;
    Some(out)
}

/// The filtered linear values of an 8-bit image, before the clamp, row by row (tests only).
#[cfg(test)]
fn reduce_linear_rgb8(
    src: &[u8],
    width: u32,
    height: u32,
    out_w: u32,
    out_h: u32,
) -> Option<Vec<f32>> {
    let size = Size::new(width, height, out_w, out_h);
    let out_len = size.check(src.len())?;
    let table: &[f32] = &DECODE_8;
    let mut out = Vec::with_capacity(out_len);
    filter_linear(
        src,
        size,
        |v| table.get(usize::from(v)).copied().unwrap_or(0.0),
        |row| out.extend_from_slice(row),
    )?;
    Some(out)
}

/// The filtered linear values of a 16-bit image, before the clamp, row by row (tests only).
#[cfg(test)]
fn reduce_linear_rgb16(
    src: &[u16],
    width: u32,
    height: u32,
    out_w: u32,
    out_h: u32,
) -> Option<Vec<f32>> {
    let size = Size::new(width, height, out_w, out_h);
    let out_len = size.check(src.len())?;
    let table: &[f32] = &DECODE_16;
    let mut out = Vec::with_capacity(out_len);
    filter_linear(
        src,
        size,
        |v| table.get(usize::from(v)).copied().unwrap_or(0.0),
        |row| out.extend_from_slice(row),
    )?;
    Some(out)
}

/// The weights of one output column (horizontal pass) or row (vertical pass): source pixels
/// `start .. start + weights.len()`, normalised to sum 1.
#[derive(Debug)]
struct Taps {
    start: usize,
    weights: Vec<f32>,
}

/// The weights of every output position of a pass from `n` source pixels to `out` (`out ≤ n`).
///
/// With `f = out/n`, output pixel `i` has its centre at source position `c = (i + 0.5)/f − 0.5`,
/// and source pixel `j` the weight `L((j − c)·f)` for `|j − c| < 3/f`. The argument is computed
/// exactly as the fraction `((2j + 1)·out − (2i + 1)·n) / 2n` in integers, so the weights are the
/// same for mirrored positions and the support test needs no rounding.
fn taps(n: u32, out: u32) -> Option<Vec<Taps>> {
    let count = usize::try_from(out).ok()?;
    if out == n {
        // A factor of 1 copies (no tiny off-centre weights from `sin(πk)` in floating point).
        return Some(
            (0..count)
                .map(|i| Taps {
                    start: i,
                    weights: vec![1.0],
                })
                .collect(),
        );
    }
    let n_i = i128::from(n);
    let out_i = i128::from(out);
    let limit = 2 * LOBES * n_i; // |x| < 3  ⇔  |numerator| < 6n
    let denominator = (2 * n_i) as f64;
    let mut all = Vec::with_capacity(count);
    for i in 0..i128::from(out) {
        let centre = (2 * i + 1) * n_i;
        // A range that surely holds the support, then the exact test for each pixel.
        let lo = ((centre - limit).div_euclid(2 * out_i) - 1).max(0);
        let hi = ((centre + limit).div_euclid(2 * out_i) + 1).min(n_i - 1);
        let mut start = None;
        let mut weights = Vec::new();
        let mut sum = 0.0_f64;
        for j in lo..=hi {
            let numerator = (2 * j + 1) * out_i - centre;
            if numerator.abs() >= limit {
                continue;
            }
            if start.is_none() {
                start = Some(usize::try_from(j).ok()?);
            }
            let weight = lanczos3(numerator as f64 / denominator);
            sum += weight;
            weights.push(weight);
        }
        // The nearest source pixel is always inside the image and has a weight near 1, so the
        // sum is positive; anything else is refused rather than divided by.
        if !(sum > 0.0 && sum.is_finite()) {
            return None;
        }
        all.push(Taps {
            start: start?,
            weights: weights.into_iter().map(|w| (w / sum) as f32).collect(),
        });
    }
    Some(all)
}

/// `L(x) = sinc(x)·sinc(x/3)` for `|x| < 3`, else 0.
fn lanczos3(x: f64) -> f64 {
    let x = x.abs();
    if x >= 3.0 {
        return 0.0;
    }
    if x == 0.0 {
        return 1.0;
    }
    let px = PI * x;
    let px3 = px / 3.0;
    (px.sin() / px) * (px3.sin() / px3)
}

/// Decodes and filters `src` and gives each output row's linear values, before the clamp, to
/// `emit`, top to bottom.
///
/// Memory: the horizontally filtered rows are kept as linear `f32` (not quantised between the
/// passes), but only while the vertical kernel needs them, in a ring of rows; besides the output,
/// at most about `2·⌈3/f⌉ + 2` such rows (`f` the vertical factor) are held, or the image's
/// height if that is smaller, plus one decoded source row. No whole intermediate image is made.
fn filter_linear<T: Copy>(
    src: &[T],
    size: Size,
    decode: impl Fn(T) -> f32,
    mut emit: impl FnMut(&[f32]),
) -> Option<()> {
    let row_len = samples(size.width, 1)?;
    let out_row_len = samples(size.out_w, 1)?;
    let h_taps = taps(size.width, size.out_w)?;
    let v_taps = taps(size.height, size.out_h)?;

    let mut linear_row = vec![0.0_f32; row_len];
    let mut ring: VecDeque<(usize, Vec<f32>)> = VecDeque::new();
    let mut spare: Vec<Vec<f32>> = Vec::new();
    let mut out_row = vec![0.0_f32; out_row_len];
    let mut next_source = 0_usize;

    for vertical in &v_taps {
        let end = vertical.start.checked_add(vertical.weights.len())?;
        // Rows above this window are not needed again (the windows only move down).
        while ring
            .front()
            .is_some_and(|(index, _)| *index < vertical.start)
        {
            if let Some((_, buffer)) = ring.pop_front() {
                spare.push(buffer);
            }
        }
        next_source = next_source.max(vertical.start);
        while next_source < end {
            let first = next_source.checked_mul(row_len)?;
            let source_row = src.get(first..first.checked_add(row_len)?)?;
            for (linear, &sample) in linear_row.iter_mut().zip(source_row) {
                *linear = decode(sample);
            }
            let mut buffer = spare.pop().unwrap_or_else(|| vec![0.0; out_row_len]);
            horizontal(&linear_row, &h_taps, &mut buffer)?;
            ring.push_back((next_source, buffer));
            next_source += 1;
        }
        if ring.len() != vertical.weights.len() {
            return None;
        }
        out_row.fill(0.0);
        for ((_, row), &weight) in ring.iter().zip(&vertical.weights) {
            for (out, &value) in out_row.iter_mut().zip(row) {
                *out += weight * value;
            }
        }
        emit(&out_row);
    }
    Some(())
}

/// The horizontal pass of one linear row into `out` (`out_w × 3` values).
fn horizontal(linear: &[f32], taps: &[Taps], out: &mut [f32]) -> Option<()> {
    let (linear, _) = linear.as_chunks::<3>();
    let (out, _) = out.as_chunks_mut::<3>();
    for (tap, out_pixel) in taps.iter().zip(out) {
        let pixels = linear.get(tap.start..tap.start.checked_add(tap.weights.len())?)?;
        let mut acc = [0.0_f32; 3];
        for (pixel, &weight) in pixels.iter().zip(&tap.weights) {
            for (sum, &value) in acc.iter_mut().zip(pixel) {
                *sum += weight * value;
            }
        }
        *out_pixel = acc;
    }
    Some(())
}

/// The sRGB transfer curve (IEC 61966-2-1) from an encoded value `v` in 0–1 to linear light.
fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// The 8-bit decode table: sample → linear value.
static DECODE_8: LazyLock<Vec<f32>> = LazyLock::new(|| {
    (0..=255_u32)
        .map(|v| srgb_to_linear(f64::from(v) / 255.0) as f32)
        .collect()
});

/// The 16-bit decode table: sample → linear value.
static DECODE_16: LazyLock<Vec<f32>> = LazyLock::new(|| {
    (0..=65535_u32)
        .map(|v| srgb_to_linear(f64::from(v) / 65535.0) as f32)
        .collect()
});

/// The inverse sRGB curve of a linear value in 0–1, scaled to `max` and rounded. The formula in
/// `f32` (not a coarse table) keeps the 8-bit round trip of dark values exact.
fn encode(linear: f32, max: f32) -> f32 {
    let v = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (v * max).round().clamp(0.0, max)
}
