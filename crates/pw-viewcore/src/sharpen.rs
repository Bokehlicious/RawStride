//! Sharpening of a view (ADR 0027 §3.3, W3-Q2): an unsharp mask on luminance, applied to a view at
//! screen resolution after the reduction. The luminance `Y′` of the 8-bit values as they are is
//! blurred with a separable Gaussian, and where `Y′` differs from its blur by at least the
//! threshold, the same offset is added to R′, G′ and B′, so no colour fringe is made.

use std::ops::RangeInclusive;

use crate::backend::Rgb;

/// The parameters of the sharpening (ADR 0027 W3-Q7). Integers only, so it can be part of a hashed
/// job key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sharpening {
    /// The strength in percent.
    pub amount: u16,
    /// The radius (the Gaussian's sigma) in tenths of a pixel.
    pub radius_tenths: u8,
    /// The smallest difference of luminance, in levels of 0–255, that is sharpened.
    pub threshold: u8,
}

impl Sharpening {
    pub const AMOUNT_RANGE: RangeInclusive<u16> = 10..=300;
    pub const RADIUS_TENTHS_RANGE: RangeInclusive<u8> = 3..=20;
    pub const THRESHOLD_RANGE: RangeInclusive<u8> = 0..=20;
    /// Amount 80 %, radius 0.8 px, threshold 2 (ADR 0027 §3.3).
    pub const DEFAULT: Sharpening = Sharpening {
        amount: 80,
        radius_tenths: 8,
        threshold: 2,
    };

    /// All three values lie inside their ranges.
    pub fn is_valid(self) -> bool {
        Self::AMOUNT_RANGE.contains(&self.amount)
            && Self::RADIUS_TENTHS_RANGE.contains(&self.radius_tenths)
            && Self::THRESHOLD_RANGE.contains(&self.threshold)
    }

    /// Each value moved into its range.
    pub fn clamped(self) -> Self {
        Sharpening {
            amount: self
                .amount
                .clamp(*Self::AMOUNT_RANGE.start(), *Self::AMOUNT_RANGE.end()),
            radius_tenths: self.radius_tenths.clamp(
                *Self::RADIUS_TENTHS_RANGE.start(),
                *Self::RADIUS_TENTHS_RANGE.end(),
            ),
            threshold: self
                .threshold
                .clamp(*Self::THRESHOLD_RANGE.start(), *Self::THRESHOLD_RANGE.end()),
        }
    }

    /// The radius in pixels.
    pub fn radius(self) -> f32 {
        f32::from(self.radius_tenths) / 10.0
    }
}

impl Default for Sharpening {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Sharpens `img` with `s`. Returns `None` for an inconsistent image or parameters that are not
/// valid; an image without pixels is copied.
pub fn sharpen(img: &Rgb, s: Sharpening) -> Option<Rgb> {
    let threads = thread_count(available_cores(), usize::try_from(img.height).ok()?);
    sharpen_with_threads(img, s, threads)
}

/// The most threads one sharpening uses, as for a reduction in `pw-resample`: `pw-viewcore` has two
/// workers that may work at the same time, so one sharpening does not take every core of a large PC.
const MAX_THREADS: usize = 8;

/// The fewest rows a band gets from [`thread_count`], so a thread has enough work to be worth
/// starting.
const MIN_ROWS_PER_BAND: usize = 32;

/// The number of cores, or 1 if the system cannot tell.
fn available_cores() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

/// The number of threads for an image of `rows` rows on a PC with `cores` cores: `cores`, at most
/// [`MAX_THREADS`] and at most `⌊rows / 32⌋`, but at least 1.
fn thread_count(cores: usize, rows: usize) -> usize {
    cores.min(MAX_THREADS).min(rows / MIN_ROWS_PER_BAND).max(1)
}

/// [`sharpen`] with its rows split into `threads` bands (0 counts as 1), in both passes: the rows
/// of the horizontal blur, then the rows of the vertical blur and the output. Every number of
/// threads gives the same bytes.
fn sharpen_with_threads(img: &Rgb, s: Sharpening, threads: usize) -> Option<Rgb> {
    if !img.is_consistent() || !s.is_valid() {
        return None;
    }
    let width = usize::try_from(img.width).ok()?;
    let height = usize::try_from(img.height).ok()?;
    if width == 0 || height == 0 {
        return Some(img.clone());
    }
    let kernel = kernel(s);
    let row_bytes = width.checked_mul(3)?;
    let src_rows = |first: usize| img.data.get(first.checked_mul(row_bytes)?..);
    // The rows of `Y′`, each blurred horizontally.
    let mut rows_blurred = vec![0.0f32; width.checked_mul(height)?];
    in_bands(
        &mut rows_blurred,
        width,
        threads,
        &|first, part: &mut [f32]| {
            // One buffer per band, reused for every row of it.
            let mut padded = Vec::with_capacity(width.checked_add(kernel.len())?);
            for (src_row, out_row) in src_rows(first)?
                .chunks_exact(row_bytes)
                .zip(part.chunks_exact_mut(width))
            {
                blur_row(src_row, &kernel, &mut padded, out_row)?;
            }
            Some(())
        },
    )?;
    let rows_blurred = &rows_blurred;
    let mut data = vec![0u8; img.data.len()];
    in_bands(&mut data, row_bytes, threads, &|first, part: &mut [u8]| {
        // One buffer per band, reused for every row of it.
        let mut blur = vec![0.0f32; width];
        for (i, (src_row, out_row)) in src_rows(first)?
            .chunks_exact(row_bytes)
            .zip(part.chunks_exact_mut(row_bytes))
            .enumerate()
        {
            blur_column(rows_blurred, width, height, first + i, &kernel, &mut blur)?;
            apply_row(src_row, &blur, s, out_row);
        }
        Some(())
    })?;
    Some(Rgb {
        width: img.width,
        height: img.height,
        data,
    })
}

/// How the bands of one pass ended.
enum Outcome {
    Done,
    /// A band failed or panicked.
    Failed,
    /// A thread could not be started; the bands already started were joined.
    SpawnFailed,
}

/// Runs `work(first_row, part)` on `out` (rows of `row_len` values) split into `bands` bands of
/// whole rows, as even as possible (their sizes differ by at most one row; at most one band per
/// row), `bands − 1` of them on threads of their own and the last on the calling thread. If a
/// thread cannot be started, the whole of `out` is done again here as one band.
fn in_bands<T, W>(out: &mut [T], row_len: usize, bands: usize, work: &W) -> Option<()>
where
    T: Send,
    W: Fn(usize, &mut [T]) -> Option<()> + Sync,
{
    let rows = out.len().checked_div(row_len)?;
    let bands = bands.min(rows).max(1);
    let base = rows.checked_div(bands)?;
    let extra = rows.checked_rem(bands)?;
    let mut parts = Vec::with_capacity(bands);
    let mut rest: &mut [T] = &mut *out;
    let mut first = 0;
    for index in 0..bands {
        let n = base + usize::from(index < extra);
        let (part, next) = rest.split_at_mut_checked(n.checked_mul(row_len)?)?;
        parts.push((first, part));
        first += n;
        rest = next;
    }
    let (last_first, last_part) = parts.pop()?;
    let outcome = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(parts.len());
        let mut spawn_failed = false;
        for (band_first, part) in parts {
            let started =
                std::thread::Builder::new().spawn_scoped(scope, move || work(band_first, part));
            match started {
                Ok(handle) => handles.push(handle),
                Err(_) => {
                    spawn_failed = true;
                    break;
                }
            }
        }
        let mut ok = spawn_failed || work(last_first, last_part).is_some();
        // Every started band is joined here, so the scope never sees an unjoined panic.
        for handle in handles {
            ok &= matches!(handle.join(), Ok(Some(())));
        }
        if !ok {
            Outcome::Failed
        } else if spawn_failed {
            Outcome::SpawnFailed
        } else {
            Outcome::Done
        }
    });
    match outcome {
        Outcome::Done => Some(()),
        Outcome::Failed => None,
        Outcome::SpawnFailed => work(0, out),
    }
}

/// The normalised Gaussian of sigma `s.radius()`, `2·half + 1` taps with `half = ⌈3σ⌉`.
fn kernel(s: Sharpening) -> Vec<f32> {
    let half = (3 * usize::from(s.radius_tenths)).div_ceil(10);
    let sigma = s.radius();
    let weights: Vec<f32> = (0..=2 * half)
        .map(|i| {
            let d = i as f32 - half as f32;
            (-(d * d) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    weights.into_iter().map(|w| w / sum).collect()
}

/// The luminance `Y′` of one 8-bit pixel, as it is (not linear light).
fn luma(p: &[u8; 3]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

/// Blurs the luminance of one row of pixels (`src`) horizontally into `out`, repeating the edge
/// pixels. `padded` is a buffer the caller reuses from row to row: it is cleared and filled with
/// the row's luminance and `half` copies of the edge value at each end. `out` is set to 0.0 and
/// then summed with the taps in the outer loop and the pixels in the inner one, so each pixel gets
/// the same sums in the same order as one tap after another.
fn blur_row(src: &[u8], kernel: &[f32], padded: &mut Vec<f32>, out: &mut [f32]) -> Option<()> {
    let half = kernel.len() / 2;
    let (pixels, _) = src.as_chunks::<3>();
    let first = luma(pixels.first()?);
    let last = luma(pixels.last()?);
    padded.clear();
    padded.extend(std::iter::repeat_n(first, half));
    padded.extend(pixels.iter().map(luma));
    padded.extend(std::iter::repeat_n(last, half));
    out.fill(0.0);
    for (i, k) in kernel.iter().enumerate() {
        let window = padded.get(i..)?;
        for (o, v) in out.iter_mut().zip(window) {
            *o += v * k;
        }
    }
    Some(())
}

/// Blurs row `y` of the horizontally blurred rows vertically into `out` (`width` values), repeating
/// the edge rows. `out` is a buffer the caller reuses from row to row; it is set to 0.0 first.
fn blur_column(
    rows: &[f32],
    width: usize,
    height: usize,
    y: usize,
    kernel: &[f32],
    out: &mut [f32],
) -> Option<()> {
    let half = kernel.len() / 2;
    out.fill(0.0);
    for (i, k) in kernel.iter().enumerate() {
        let row = (y + i).saturating_sub(half).min(height - 1);
        let start = row.checked_mul(width)?;
        let src = rows.get(start..start.checked_add(width)?)?;
        for (o, v) in out.iter_mut().zip(src) {
            *o += v * k;
        }
    }
    Some(())
}

/// Writes one sharpened row: where `|Y′ − blur| ≥ threshold`, the offset
/// `round(amount / 100 × (Y′ − blur))` is added to R′, G′ and B′, each clamped to 0–255.
fn apply_row(src: &[u8], blur: &[f32], s: Sharpening, out: &mut [u8]) {
    let amount = f32::from(s.amount) / 100.0;
    let threshold = f32::from(s.threshold);
    let (pixels, _) = src.as_chunks::<3>();
    let (outs, _) = out.as_chunks_mut::<3>();
    for ((p, b), o) in pixels.iter().zip(blur).zip(outs.iter_mut()) {
        let d = luma(p) - b;
        if d.abs() < threshold {
            *o = *p;
            continue;
        }
        let k = (amount * d).round() as i32;
        *o = p.map(|c| (i32::from(c) + k).clamp(0, 255) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Rgb {
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&f(x, y));
            }
        }
        Rgb {
            width: w,
            height: h,
            data,
        }
    }

    fn at(img: &Rgb, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * img.width + x) * 3) as usize;
        [img.data[i], img.data[i + 1], img.data[i + 2]]
    }

    fn s(amount: u16, radius_tenths: u8, threshold: u8) -> Sharpening {
        Sharpening {
            amount,
            radius_tenths,
            threshold,
        }
    }

    fn sharpened(src: &Rgb, p: Sharpening) -> Rgb {
        sharpen(src, p).unwrap_or_else(|| img(0, 0, |_, _| [0; 3]))
    }

    /// A vertical step at x = 20 of a 40×8 image: `left` for x < 20, `right` from x = 20.
    fn step(left: [u8; 3], right: [u8; 3]) -> Rgb {
        img(40, 8, |x, _| if x < 20 { left } else { right })
    }

    const RADIUS_2: Sharpening = Sharpening {
        amount: 60,
        radius_tenths: 20,
        threshold: 2,
    };

    #[test]
    fn a_uniform_image_is_unchanged() {
        for colour in [[0, 0, 0], [255, 255, 255], [200, 60, 40], [17, 130, 250]] {
            let src = img(23, 17, |_, _| colour);
            for p in [Sharpening::DEFAULT, RADIUS_2] {
                assert_eq!(sharpened(&src, p), src, "{colour:?} {p:?}");
            }
        }
    }

    #[test]
    fn a_grey_step_gets_darker_and_brighter_next_to_it_and_stays_grey() {
        let src = step([100; 3], [150; 3]);
        let out = sharpened(&src, Sharpening::DEFAULT);
        assert_eq!((out.width, out.height), (src.width, src.height));
        for y in 0..out.height {
            assert!(at(&out, 19, y)[0] < 100, "{:?}", at(&out, 19, y));
            assert!(at(&out, 20, y)[0] > 150, "{:?}", at(&out, 20, y));
            for x in (0..10).chain(30..40) {
                assert_eq!(at(&out, x, y), at(&src, x, y), "x {x}");
            }
            for x in 0..out.width {
                let p = at(&out, x, y);
                assert!(p[0] == p[1] && p[1] == p[2], "x {x}: {p:?}");
            }
        }
    }

    #[test]
    fn a_coloured_step_keeps_the_differences_between_channels() {
        let src = step([200, 60, 40], [120, 30, 20]);
        for p in [Sharpening::DEFAULT, RADIUS_2, s(300, 20, 0)] {
            let out = sharpened(&src, p);
            let mut changed = false;
            for y in 0..out.height {
                for x in 0..out.width {
                    let (a, b) = (at(&src, x, y), at(&out, x, y));
                    changed |= a != b;
                    if b.iter().any(|&c| c == 0 || c == 255) {
                        continue;
                    }
                    let diff = |q: [u8; 3]| {
                        (
                            i32::from(q[0]) - i32::from(q[1]),
                            i32::from(q[1]) - i32::from(q[2]),
                        )
                    };
                    assert_eq!(diff(a), diff(b), "x {x} {p:?}");
                }
            }
            assert!(changed, "{p:?}");
        }
    }

    #[test]
    fn the_threshold_leaves_a_small_step_alone() {
        let src = step([100; 3], [110; 3]);
        assert_eq!(sharpened(&src, s(100, 6, 12)), src);
        let out = sharpened(&src, s(100, 6, 0));
        for y in 0..out.height {
            assert!(at(&out, 19, y)[0] < 100, "{:?}", at(&out, 19, y));
            assert!(at(&out, 20, y)[0] > 110, "{:?}", at(&out, 20, y));
        }
    }

    #[test]
    fn a_larger_amount_gives_a_larger_overshoot() {
        let src = step([100; 3], [150; 3]);
        let small = sharpened(&src, s(40, 6, 2));
        let large = sharpened(&src, s(150, 6, 2));
        assert!(at(&large, 19, 0)[0] < at(&small, 19, 0)[0]);
        assert!(at(&large, 20, 0)[0] > at(&small, 20, 0)[0]);
    }

    #[test]
    fn small_and_empty_images_keep_their_size() {
        for (w, h) in [(1, 1), (1, 5), (5, 1), (0, 0), (0, 5), (5, 0)] {
            let uniform = img(w, h, |_, _| [90, 120, 30]);
            for p in [Sharpening::DEFAULT, RADIUS_2, s(300, 20, 0)] {
                assert_eq!(sharpened(&uniform, p), uniform, "{w}x{h} {p:?}");
                let varied = img(w, h, |x, y| [(x * 50) as u8, (y * 40) as u8, 7]);
                let out = sharpened(&varied, p);
                assert_eq!((out.width, out.height), (w, h));
                assert!(out.is_consistent());
            }
        }
    }

    #[test]
    fn values_are_clamped_and_never_wrapped() {
        let src = step([250; 3], [255; 3]);
        let out = sharpened(&src, s(300, 20, 0));
        for y in 0..out.height {
            // The bright side would go above 255, so it stays at 255 instead of wrapping to a
            // small value; the dark side goes down from 250.
            assert_eq!(at(&out, 20, y), [255; 3]);
            assert!(at(&out, 19, y)[0] < 250);
            for x in 0..out.width {
                assert!(at(&out, x, y)[0] >= 200, "x {x}: {:?}", at(&out, x, y));
            }
        }
        let dark = step([0; 3], [5; 3]);
        let out = sharpened(&dark, s(300, 20, 0));
        for x in 0..out.width {
            assert!(at(&out, x, 0)[0] <= 55, "x {x}: {:?}", at(&out, x, 0));
        }
        assert_eq!(at(&out, 19, 0), [0; 3]);
    }

    #[test]
    fn bad_input_and_parameters_give_none() {
        let mut bad = img(4, 4, |_, _| [1, 2, 3]);
        bad.data.pop();
        assert!(sharpen(&bad, Sharpening::DEFAULT).is_none());
        let good = img(4, 4, |_, _| [1, 2, 3]);
        let outside = [
            s(9, 6, 2),
            s(301, 6, 2),
            s(60, 2, 2),
            s(60, 21, 2),
            s(60, 6, 21),
        ];
        for p in outside {
            assert!(!p.is_valid(), "{p:?}");
            assert!(sharpen(&good, p).is_none(), "{p:?}");
            assert!(p.clamped().is_valid(), "{p:?}");
            assert!(sharpen(&good, p.clamped()).is_some(), "{p:?}");
        }
        assert_eq!(s(9, 2, 21).clamped(), s(10, 3, 20));
        assert_eq!(s(301, 21, 0).clamped(), s(300, 20, 0));
        assert_eq!(Sharpening::default(), s(80, 8, 2));
        assert_eq!(Sharpening::DEFAULT.radius(), 0.8);
    }

    /// A pattern with edges everywhere, as `pw-resample`'s test images.
    fn pattern(x: u32, y: u32, c: u32) -> u8 {
        let mut v =
            x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(2_246_822_519) ^ c.wrapping_mul(97);
        v ^= v >> 13;
        v = v.wrapping_mul(1_274_126_177);
        ((v ^ (v >> 16)) & 0xFF) as u8
    }

    fn patterned(w: u32, h: u32) -> Rgb {
        img(w, h, |x, y| {
            [pattern(x, y, 0), pattern(x, y, 1), pattern(x, y, 2)]
        })
    }

    #[test]
    fn threads_give_the_same_bytes_as_one() {
        // 300 rows: more than one band of 32 rows, and not divisible by every band count.
        let src = patterned(37, 300);
        for p in [Sharpening::DEFAULT, RADIUS_2, s(300, 20, 0)] {
            let one = sharpen_with_threads(&src, p, 1);
            assert!(one.as_ref().is_some_and(|o| *o != src), "{p:?}");
            for threads in [0, 2, 3, 7, 8, 300, 1000] {
                assert_eq!(
                    sharpen_with_threads(&src, p, threads),
                    one,
                    "{threads} {p:?}"
                );
            }
            assert_eq!(sharpen(&src, p), one, "{p:?}");
        }
    }

    /// T315's two passes as they were before T319, on one thread: a new `Vec` per row and the
    /// horizontal taps summed in a `fold`.
    fn reference_sharpen(img: &Rgb, p: Sharpening) -> Rgb {
        fn blur_row(src: &[u8], kernel: &[f32], out: &mut [f32]) -> Option<()> {
            let half = kernel.len() / 2;
            let (pixels, _) = src.as_chunks::<3>();
            let first = luma(pixels.first()?);
            let last = luma(pixels.last()?);
            let mut padded = Vec::with_capacity(pixels.len() + 2 * half);
            padded.extend(std::iter::repeat_n(first, half));
            padded.extend(pixels.iter().map(luma));
            padded.extend(std::iter::repeat_n(last, half));
            for (window, o) in padded.windows(kernel.len()).zip(out.iter_mut()) {
                *o = window.iter().zip(kernel).fold(0.0, |a, (v, k)| a + v * k);
            }
            Some(())
        }

        fn blur_column(
            rows: &[f32],
            width: usize,
            height: usize,
            y: usize,
            kernel: &[f32],
        ) -> Option<Vec<f32>> {
            let half = kernel.len() / 2;
            let mut out = vec![0.0f32; width];
            for (i, k) in kernel.iter().enumerate() {
                let row = (y + i).saturating_sub(half).min(height - 1);
                let start = row.checked_mul(width)?;
                let src = rows.get(start..start.checked_add(width)?)?;
                for (o, v) in out.iter_mut().zip(src) {
                    *o += v * k;
                }
            }
            Some(out)
        }

        let (width, height) = (img.width as usize, img.height as usize);
        let kernel = kernel(p);
        let row_bytes = width * 3;
        let mut rows_blurred = vec![0.0f32; width * height];
        for (src_row, out_row) in img
            .data
            .chunks_exact(row_bytes)
            .zip(rows_blurred.chunks_exact_mut(width))
        {
            blur_row(src_row, &kernel, out_row).unwrap();
        }
        let mut data = vec![0u8; img.data.len()];
        for (y, (src_row, out_row)) in img
            .data
            .chunks_exact(row_bytes)
            .zip(data.chunks_exact_mut(row_bytes))
            .enumerate()
        {
            let blur = blur_column(&rows_blurred, width, height, y, &kernel).unwrap();
            apply_row(src_row, &blur, p, out_row);
        }
        Rgb {
            width: img.width,
            height: img.height,
            data,
        }
    }

    #[test]
    fn the_faster_passes_give_the_same_bytes_as_t315s() {
        for (w, h) in [(37, 300), (1, 1), (1, 5), (5, 1), (64, 65)] {
            let src = patterned(w, h);
            for p in [Sharpening::DEFAULT, RADIUS_2, s(300, 20, 0)] {
                let reference = reference_sharpen(&src, p);
                // One pixel's blur is the pixel itself, so only 1×1 stays as it is.
                assert_eq!(reference == src, (w, h) == (1, 1), "{w}x{h} {p:?}");
                assert_eq!(sharpen(&src, p), Some(reference), "{w}x{h} {p:?}");
            }
        }
    }

    #[test]
    fn the_thread_count_follows_cores_and_rows() {
        assert_eq!(thread_count(16, 2160), 8);
        assert_eq!(thread_count(4, 2160), 4);
        assert_eq!(thread_count(16, 100), 3);
        assert_eq!(thread_count(16, 10), 1);
        assert_eq!(thread_count(0, 2160), 1);
    }

    #[test]
    #[ignore = "a time measurement; run with --release (ADR 0027 W3-Q3)"]
    fn timing_of_sharpening() {
        for (w, h) in [(3840, 2160), (1920, 1200), (4800, 3200)] {
            let src = patterned(w, h);
            for p in [Sharpening::DEFAULT, RADIUS_2] {
                let mut times = Vec::new();
                for _ in 0..5 {
                    let started = std::time::Instant::now();
                    let out = sharpen(&src, p);
                    times.push(started.elapsed().as_secs_f64() * 1000.0);
                    assert_eq!(out.map(|o| (o.width, o.height)), Some((w, h)));
                }
                times.sort_by(f64::total_cmp);
                println!(
                    "{w}×{h}, radius {:.1} px: median {:.1} ms, max {:.1} ms on {} threads",
                    p.radius(),
                    times[2],
                    times[4],
                    thread_count(available_cores(), h as usize)
                );
            }
        }
    }
}
