//! Small, dependency-free pixel helpers for the TIFF viewer: sample conversion to 8-bit RGB and
//! area-averaging downscale. Kept separate from the decoder glue so they can be unit tested
//! without the `tiff`/`jpeg-encoder` crates.

/// Channel layout of decoded samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Gray,
    GrayAlpha,
    Rgb,
    Rgba,
    Cmyk,
}

impl Layout {
    pub fn channels(self) -> usize {
        match self {
            Layout::Gray => 1,
            Layout::GrayAlpha => 2,
            Layout::Rgb => 3,
            Layout::Rgba | Layout::Cmyk => 4,
        }
    }
}

/// An 8-bit RGB image.
#[derive(Debug, Clone)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Converts interleaved 8-bit samples to RGB (alpha is dropped, CMYK converted naively).
pub fn to_rgb8(samples: &[u8], layout: Layout, width: u32, height: u32) -> Result<Rgb8, String> {
    let n = width as usize * height as usize;
    let ch = layout.channels();
    if samples.len() < n * ch {
        return Err(format!(
            "image data is short: {} of {} bytes",
            samples.len(),
            n * ch
        ));
    }
    let data = match layout {
        Layout::Rgb => samples[..n * 3].to_vec(),
        Layout::Gray => samples[..n].iter().flat_map(|&g| [g, g, g]).collect(),
        Layout::GrayAlpha => samples[..n * 2]
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0]])
            .collect(),
        Layout::Rgba => samples[..n * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect(),
        Layout::Cmyk => samples[..n * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let k = 255 - u32::from(p[3]);
                let c = |v: u8| ((255 - u32::from(v)) * k / 255) as u8;
                [c(p[0]), c(p[1]), c(p[2])]
            })
            .collect(),
    };
    Ok(Rgb8 {
        width,
        height,
        data,
    })
}

/// 16-bit samples to 8-bit (top byte; TIFFs from editors are gamma encoded).
pub fn u16_to_u8(samples: &[u16]) -> Vec<u8> {
    samples.iter().map(|&s| (s >> 8) as u8).collect()
}

/// Float samples (nominal range 0–1) to 8-bit.
pub fn f32_to_u8(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .map(|&s| (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
        .collect()
}

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
pub fn downscale(src: &Rgb8, tw: u32, th: u32) -> Rgb8 {
    if (tw, th) == (src.width, src.height) || tw == 0 || th == 0 {
        return src.clone();
    }
    let (sw, sh) = (src.width as usize, src.height as usize);
    let (tw, th) = (tw as usize, th as usize);
    // Target column of each source column.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        let g = to_rgb8(&[10, 20], Layout::Gray, 2, 1).unwrap();
        assert_eq!(g.data, vec![10, 10, 10, 20, 20, 20]);
        let a = to_rgb8(&[1, 2, 3, 99], Layout::Rgba, 1, 1).unwrap();
        assert_eq!(a.data, vec![1, 2, 3]);
        let k = to_rgb8(&[0, 255, 0, 0], Layout::Cmyk, 1, 1).unwrap();
        assert_eq!(k.data, vec![255, 0, 255]);
        assert!(to_rgb8(&[1, 2], Layout::Rgb, 1, 1).is_err());
        assert_eq!(u16_to_u8(&[0xFFFF, 0x0100]), vec![255, 1]);
        assert_eq!(f32_to_u8(&[1.5, 0.0, 0.5]), vec![255, 0, 128]);
    }

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
}
