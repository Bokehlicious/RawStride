//! Target size and area-averaging downscale. Moved from `src-tauri/src/imaging.rs` (ADR 0015 D3)
//! so the TIFF thumbnails of RawStride and the Tauri app's TIFF and RAW views share one box
//! filter. The result is unchanged bit for bit; `downscale` no longer indexes slices.

use crate::Rgb8;

/// Target size so that the long edge is at most `max_edge` (never upscales; 0 = original).
pub fn fit(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let long = width.max(height);
    if max_edge == 0 || long <= max_edge {
        return (width, height);
    }
    let scale = |v: u32| {
        ((u64::from(v) * u64::from(max_edge) + u64::from(long) / 2) / u64::from(long)).max(1) as u32
    };
    (scale(width), scale(height))
}

/// Area-averaging downscale to `tw`×`th` (each target pixel is the mean of its source block).
/// One pass over the source; cost is proportional to the source size.
///
/// A source without pixels, or a target row whose source rows are missing from a short `data`,
/// gives black target pixels instead of a panic.
pub fn downscale(src: &Rgb8, tw: u32, th: u32) -> Rgb8 {
    if (tw, th) == (src.width, src.height) || tw == 0 || th == 0 {
        return src.clone();
    }
    let (sw, sh) = (src.width as usize, src.height as usize);
    let (tw, th) = (tw as usize, th as usize);
    let mut out = vec![0u8; tw * th * 3];
    if sw == 0 || sh == 0 {
        return Rgb8 {
            width: tw as u32,
            height: th as u32,
            data: out,
        };
    }
    let row_len = sw * 3;
    // Target column of each source column.
    let col: Vec<usize> = (0..sw).map(|x| (x * tw / sw).min(tw - 1)).collect();
    // Sums per target column and channel, and the number of source pixels in each.
    let mut acc = vec![[0u64; 3]; tw];
    let mut cnt = vec![0u64; tw];
    for (ty, target_row) in out.chunks_exact_mut(tw * 3).enumerate() {
        let y0 = (ty * sh / th).min(sh - 1);
        let y1 = ((ty + 1) * sh / th).max(y0 + 1).min(sh);
        acc.fill([0; 3]);
        cnt.fill(0);
        let rows = src.data.get(y0 * row_len..y1 * row_len).unwrap_or_default();
        for row in rows.chunks_exact(row_len) {
            for (px, &t) in row.as_chunks::<3>().0.iter().zip(&col) {
                if let (Some(sum), Some(n)) = (acc.get_mut(t), cnt.get_mut(t)) {
                    for (s, &v) in sum.iter_mut().zip(px) {
                        *s += u64::from(v);
                    }
                    *n += 1;
                }
            }
        }
        for ((target, sum), &n) in target_row
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(&acc)
            .zip(&cnt)
        {
            let c = n.max(1);
            for (o, &s) in target.iter_mut().zip(sum) {
                *o = ((s + c / 2) / c) as u8;
            }
        }
    }
    Rgb8 {
        width: tw as u32,
        height: th as u32,
        data: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_aspect_and_never_upscales() {
        assert_eq!(fit(9504, 6336, 640), (640, 427));
        assert_eq!(fit(6336, 9504, 2560), (1707, 2560));
        assert_eq!(fit(500, 300, 640), (500, 300));
        assert_eq!(fit(500, 300, 0), (500, 300));
        assert_eq!(fit(10000, 1, 100), (100, 1));
    }

    #[test]
    fn downscale_averages_blocks() {
        // 4×2: left half black, right half white -> 2×1: [0, 255]
        let mut data = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                data.extend_from_slice(&[v, v, v]);
            }
        }
        let src = Rgb8 {
            width: 4,
            height: 2,
            data,
        };
        let d = downscale(&src, 2, 1);
        assert_eq!(d.data, vec![0, 0, 0, 255, 255, 255]);
        // Odd sizes cover every target pixel.
        let src = Rgb8 {
            width: 7,
            height: 5,
            data: vec![100; 7 * 5 * 3],
        };
        let d = downscale(&src, 3, 2);
        assert_eq!(d.data, vec![100; 3 * 2 * 3]);
        assert_eq!(downscale(&src, 7, 5).data.len(), 7 * 5 * 3);
    }

    /// `downscale` as it was in `src-tauri/src/imaging.rs` before the move, to pin the result.
    fn downscale_before_the_move(src: &Rgb8, tw: u32, th: u32) -> Rgb8 {
        if (tw, th) == (src.width, src.height) || tw == 0 || th == 0 {
            return src.clone();
        }
        let (sw, sh) = (src.width as usize, src.height as usize);
        let (tw, th) = (tw as usize, th as usize);
        let col: Vec<usize> = (0..sw).map(|x| (x * tw / sw).min(tw - 1)).collect();
        let mut out = vec![0u8; tw * th * 3];
        let mut acc = vec![0u32; tw * 3];
        let mut cnt = vec![0u32; tw];
        for ty in 0..th {
            let y0 = (ty * sh / th).min(sh - 1);
            let y1 = ((ty + 1) * sh / th).clamp(y0 + 1, sh);
            acc.fill(0);
            cnt.fill(0);
            for row in src.data[y0 * sw * 3..y1 * sw * 3].chunks_exact(sw * 3) {
                for (x, px) in row.as_chunks::<3>().0.iter().enumerate() {
                    let t = col[x];
                    acc[t * 3] += u32::from(px[0]);
                    acc[t * 3 + 1] += u32::from(px[1]);
                    acc[t * 3 + 2] += u32::from(px[2]);
                    cnt[t] += 1;
                }
            }
            let o = &mut out[ty * tw * 3..(ty + 1) * tw * 3];
            for t in 0..tw {
                let c = cnt[t].max(1);
                for k in 0..3 {
                    o[t * 3 + k] = ((acc[t * 3 + k] + c / 2) / c) as u8;
                }
            }
        }
        Rgb8 {
            width: tw as u32,
            height: th as u32,
            data: out,
        }
    }

    #[test]
    fn downscale_of_7x5_to_3x2_matches_the_code_before_the_move() {
        // Every byte different, so a wrong block or rounding shows.
        let src = Rgb8 {
            width: 7,
            height: 5,
            data: (0..7 * 5 * 3).map(|i| (i * 37 % 256) as u8).collect(),
        };
        let expected = downscale_before_the_move(&src, 3, 2);
        let d = downscale(&src, 3, 2);
        assert_eq!((d.width, d.height), (3, 2));
        assert_eq!((expected.width, expected.height), (3, 2));
        assert_eq!(d.data, expected.data);

        // More sizes, each compared with the code before the move.
        let src = Rgb8 {
            width: 13,
            height: 11,
            data: (0..13 * 11 * 3).map(|i| (i * 101 % 251) as u8).collect(),
        };
        for (tw, th) in [(1, 1), (2, 3), (6, 5), (12, 10), (13, 11), (13, 1), (1, 11)] {
            assert_eq!(
                downscale(&src, tw, th).data,
                downscale_before_the_move(&src, tw, th).data,
                "{tw}×{th}"
            );
        }

        // Where the code before the move panicked (no pixels, short data): black, no panic.
        let empty = Rgb8 {
            width: 0,
            height: 0,
            data: Vec::new(),
        };
        assert_eq!(downscale(&empty, 2, 1).data, vec![0; 2 * 3]);
        let short = Rgb8 {
            width: 4,
            height: 4,
            data: vec![200; 4 * 3],
        };
        assert_eq!(downscale(&short, 2, 2).data, vec![0; 2 * 2 * 3]);
    }
}
