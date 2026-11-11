//! Small, dependency-free pixel helpers: sample conversion to 8-bit RGB. Moved unchanged in
//! behaviour and messages from `src-tauri/src/imaging.rs` (ADR 0015 D1).

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
    let short = || format!("image data is short: {} of {} bytes", samples.len(), n * ch);
    let samples = samples.get(..n * ch).ok_or_else(short)?;
    let data = match layout {
        Layout::Rgb => samples.to_vec(),
        Layout::Gray => samples.iter().flat_map(|&g| [g, g, g]).collect(),
        Layout::GrayAlpha => samples
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0]])
            .collect(),
        Layout::Rgba => samples
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect(),
        Layout::Cmyk => samples
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

/// Converts interleaved 16-bit RGB or RGBA samples to 16-bit RGB (alpha is dropped), the 16-bit
/// twin of [`to_rgb8`] for the colour conversion before the reduction (ADR 0016 §4.1). RGB
/// samples of exactly the image's length are returned without a copy. Other layouts give an
/// error.
pub(crate) fn to_rgb16(
    samples: Vec<u16>,
    layout: Layout,
    width: u32,
    height: u32,
) -> Result<Vec<u16>, String> {
    let n = width as usize * height as usize;
    let ch = layout.channels();
    let short = || format!("image data is short: {} of {} bytes", samples.len(), n * ch);
    match layout {
        Layout::Rgb if samples.len() == n * ch => Ok(samples),
        Layout::Rgb => Ok(samples.get(..n * ch).ok_or_else(short)?.to_vec()),
        Layout::Rgba => Ok(samples
            .get(..n * ch)
            .ok_or_else(short)?
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect()),
        Layout::Gray | Layout::GrayAlpha | Layout::Cmyk => Err(format!(
            "16-bit RGB conversion supports RGB and RGBA only, not {layout:?}"
        )),
    }
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
    fn rgb16_conversions() {
        assert_eq!(
            to_rgb16(vec![1, 2, 3, 4, 5, 6], Layout::Rgb, 2, 1).unwrap(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            to_rgb16(vec![1, 2, 3, 4, 5, 6, 7], Layout::Rgb, 2, 1).unwrap(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            to_rgb16(vec![1, 2, 3, 999, 4, 5, 6, 999], Layout::Rgba, 2, 1).unwrap(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            to_rgb16(vec![1, 2], Layout::Rgb, 1, 1).unwrap_err(),
            to_rgb8(&[1, 2], Layout::Rgb, 1, 1).unwrap_err()
        );
        assert!(to_rgb16(vec![1, 2, 3], Layout::Rgba, 1, 1).is_err());
        assert!(to_rgb16(vec![1, 2, 3], Layout::Gray, 3, 1).is_err());
    }
}
