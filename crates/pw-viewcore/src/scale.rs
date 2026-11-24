//! Reduction of views on the worker (ADR 0009 §3.4, ADR 0027 §3.2): a view below 100 % is reduced
//! to the screen's resolution with the Lanczos filter of `pw-resample` (in linear light), so the GPU
//! never draws it upscaled and the texture stays small.

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

/// Reduces `img` to `out_w × out_h` with the Lanczos filter of `pw-resample` (ADR 0027 §3.2,
/// W1-Q1). Returns `None` for an inconsistent input, a 0 size or an output larger than the input in
/// either direction; the same size is a copy.
pub fn reduce(img: &Rgb, out_w: u32, out_h: u32) -> Option<Rgb> {
    if !img.is_consistent() || out_w == 0 || out_h == 0 || out_w > img.width || out_h > img.height {
        return None;
    }
    if out_w == img.width && out_h == img.height {
        return Some(img.clone());
    }
    let data = pw_resample::reduce_rgb8(&img.data, img.width, img.height, out_w, out_h)?;
    Some(Rgb {
        width: out_w,
        height: out_h,
        data,
    })
}

/// Copies the region `x, y, w, h` of `img`. Returns `None` if it does not lie inside the image or
/// the image is inconsistent.
pub fn crop(img: &Rgb, x: u32, y: u32, w: u32, h: u32) -> Option<Rgb> {
    if !img.is_consistent()
        || w == 0
        || h == 0
        || u64::from(x) + u64::from(w) > u64::from(img.width)
        || u64::from(y) + u64::from(h) > u64::from(img.height)
    {
        return None;
    }
    let row = w as usize * 3;
    let mut data = Vec::with_capacity(row * h as usize);
    for yy in y..y + h {
        let start = (yy as usize * img.width as usize + x as usize) * 3;
        data.extend_from_slice(img.data.get(start..start + row)?);
    }
    Some(Rgb {
        width: w,
        height: h,
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
    fn crop_copies_the_region_and_refuses_outside() {
        let src = img(5, 4, |x, y| [x as u8, y as u8, 0]);
        let c = crop(&src, 1, 2, 3, 2).unwrap_or_else(|| img(0, 0, |_, _| [0; 3]));
        assert_eq!((c.width, c.height), (3, 2));
        assert_eq!(&c.data[..3], &[1, 2, 0]);
        assert_eq!(&c.data[c.data.len() - 3..], &[3, 3, 0]);
        assert!(crop(&src, 3, 0, 3, 1).is_none());
        assert!(crop(&src, 0, 0, 0, 1).is_none());
    }

    #[test]
    fn reduce_refuses_what_downscale_refused_and_copies_the_same_size() {
        let src = img(4, 4, |_, _| [1, 2, 3]);
        assert!(reduce(&src, 5, 4).is_none());
        assert!(reduce(&src, 4, 5).is_none());
        assert!(reduce(&src, 0, 4).is_none());
        assert!(reduce(&src, 4, 0).is_none());
        let bad = Rgb {
            width: 4,
            height: 4,
            data: vec![0; 5],
        };
        assert!(reduce(&bad, 2, 2).is_none());
        assert_eq!(reduce(&src, 4, 4), Some(src));
    }

    #[test]
    fn reduce_gives_the_asked_size_and_a_flat_image_stays_flat() {
        let src = img(97, 61, |_, _| [10, 20, 30]);
        for (w, h) in [(50, 31), (13, 7), (1, 1), (96, 60), (97, 30)] {
            let out = reduce(&src, w, h).unwrap_or_else(|| img(0, 0, |_, _| [0; 3]));
            assert_eq!((out.width, out.height), (w, h));
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
}
