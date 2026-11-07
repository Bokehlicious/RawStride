//! Downscaling for free-zoom views (ADR 0009 §3.4): a full-resolution region is reduced on the
//! worker to the screen's resolution, so the GPU never draws it upscaled and the texture stays
//! small.

use crate::backend::Rgb;

/// The output size of `w × h` at `scale` (0 < scale ≤ 1), at least 1×1. Rounded up, so the result
/// never has fewer pixels per source pixel than asked for (a view must not be drawn upscaled).
pub fn scaled_size(w: u32, h: u32, scale: f64) -> (u32, u32) {
    let s = if scale.is_finite() && scale > 0.0 {
        scale.min(1.0)
    } else {
        1.0
    };
    let f = |v: u32| ((f64::from(v) * s).ceil() as u32).clamp(1, v.max(1));
    (f(w), f(h))
}

/// Reduces `img` to `out_w × out_h` with a box filter: every output pixel is the mean of the source
/// pixels it covers. Returns `None` for an inconsistent input or an output larger than the input.
pub fn downscale(img: &Rgb, out_w: u32, out_h: u32) -> Option<Rgb> {
    if !img.is_consistent() || out_w == 0 || out_h == 0 || out_w > img.width || out_h > img.height {
        return None;
    }
    if out_w == img.width && out_h == img.height {
        return Some(img.clone());
    }
    let (sw, sh) = (img.width as usize, img.height as usize);
    let (ow, oh) = (out_w as usize, out_h as usize);
    // source column range of every output column
    let cols: Vec<(usize, usize)> = (0..ow)
        .map(|x| {
            let a = x * sw / ow;
            let b = ((x + 1) * sw / ow).max(a + 1).min(sw);
            (a, b)
        })
        .collect();
    let mut data = vec![0u8; ow * oh * 3];
    let mut acc = vec![0u32; ow * 3];
    for y in 0..oh {
        let y0 = y * sh / oh;
        let y1 = ((y + 1) * sh / oh).max(y0 + 1).min(sh);
        acc.iter_mut().for_each(|a| *a = 0);
        for sy in y0..y1 {
            let row = img.data.get(sy * sw * 3..(sy + 1) * sw * 3)?;
            for (x, &(a, b)) in cols.iter().enumerate() {
                for px in row.get(a * 3..b * 3)?.as_chunks::<3>().0 {
                    acc[x * 3] += u32::from(px[0]);
                    acc[x * 3 + 1] += u32::from(px[1]);
                    acc[x * 3 + 2] += u32::from(px[2]);
                }
            }
        }
        let rows = (y1 - y0) as u32;
        for (x, &(a, b)) in cols.iter().enumerate() {
            let n = rows * (b - a) as u32;
            let o = (y * ow + x) * 3;
            for c in 0..3 {
                data[o + c] = ((acc[x * 3 + c] + n / 2) / n) as u8;
            }
        }
    }
    Some(Rgb {
        width: out_w,
        height: out_h,
        data,
    })
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

    #[test]
    fn sizes_round_and_stay_positive() {
        assert_eq!(scaled_size(1000, 500, 0.5), (500, 250));
        assert_eq!(scaled_size(3, 3, 0.01), (1, 1));
        assert_eq!(scaled_size(1001, 3, 0.5), (501, 2));
        assert_eq!(scaled_size(100, 100, 2.0), (100, 100));
        assert_eq!(scaled_size(100, 100, f64::NAN), (100, 100));
    }

    #[test]
    fn a_half_reduction_averages_2x2_blocks() {
        let src = img(4, 2, |x, _| {
            if x % 2 == 0 {
                [0, 100, 200]
            } else {
                [100, 200, 0]
            }
        });
        let out = downscale(&src, 2, 1).unwrap_or_else(|| img(0, 0, |_, _| [0; 3]));
        assert_eq!(out.width, 2);
        assert_eq!(out.data, vec![50, 150, 100, 50, 150, 100]);
    }

    #[test]
    fn a_flat_image_stays_flat_at_any_reduction() {
        let src = img(97, 61, |_, _| [10, 20, 30]);
        for (w, h) in [(50, 31), (13, 7), (1, 1), (96, 60)] {
            let out = downscale(&src, w, h).unwrap_or_else(|| img(0, 0, |_, _| [0; 3]));
            assert!(out.is_consistent());
            assert!(
                out.data
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .all(|p| *p == [10, 20, 30]),
                "{w}x{h}"
            );
        }
    }

    #[test]
    fn invalid_requests_are_refused() {
        let src = img(4, 4, |_, _| [1, 2, 3]);
        assert!(downscale(&src, 5, 4).is_none());
        assert!(downscale(&src, 0, 4).is_none());
        let bad = Rgb {
            width: 4,
            height: 4,
            data: vec![0; 5],
        };
        assert!(downscale(&bad, 2, 2).is_none());
        assert_eq!(downscale(&src, 4, 4), Some(src));
    }
}
