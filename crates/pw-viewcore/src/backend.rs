//! The decoder seen by the core. The real one is LibRaw with PPG ([`crate::libraw`], feature
//! `libraw`); the tests use a simulated one, so the core is tested without native libraries.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::geometry::{ImageSize, Rect};

/// An 8-bit RGB image (3 bytes per pixel, rows top to bottom), in sensor orientation.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgb {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl fmt::Debug for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgb({}x{})", self.width, self.height)
    }
}

impl Rgb {
    /// Whether the buffer holds exactly `width × height` RGB pixels.
    pub fn is_consistent(&self) -> bool {
        u64::from(self.width) * u64::from(self.height) * 3 == self.data.len() as u64
    }
}

/// Why a file or a view could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    pub message: String,
    /// The file's RAW data is missing or corrupt (G3). An intact preview is then never replaced.
    pub damaged: bool,
    /// The file has no embedded JPEG to show in JPEG mode (G4; T118), so a program can say so
    /// without reading `message`.
    pub no_embedded_jpeg: bool,
}

impl DecodeError {
    pub fn new(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: false,
            no_embedded_jpeg: false,
        }
    }

    pub fn damaged(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: true,
            no_embedded_jpeg: false,
        }
    }

    /// A file without an embedded JPEG, opened in JPEG mode (T118).
    pub fn no_embedded_jpeg(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: false,
            no_embedded_jpeg: true,
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DecodeError {}

/// Where an image's pixels come from: the display mode (G4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// A RAW rendering (LibRaw, PPG).
    Raw,
    /// The camera's largest embedded JPEG, or a plain JPEG file itself.
    Jpeg,
}

impl Source {
    /// The other display mode.
    pub fn other(self) -> Source {
        match self {
            Source::Raw => Source::Jpeg,
            Source::Jpeg => Source::Raw,
        }
    }
}

/// Where an image's statistics were computed from; the panel names it (G1, ADR 0017 U7-Q5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatisticsOrigin {
    /// The unpacked RAW data, before any rendering.
    RawData,
    /// The embedded JPEG (or a plain JPEG file) as decoded, before the conversion to the screen
    /// profile.
    EmbeddedJpeg,
    /// A TIFF file's own decoded image, before the conversion to the screen profile.
    Tiff,
}

/// The histogram and the exposure statistics of an opened image (ADR 0017 §2.6, U7), per colour
/// channel in the order R, G, B.
#[derive(Clone, PartialEq)]
pub struct Statistics {
    pub origin: StatisticsOrigin,
    /// 256 bins per channel. RAW data: the values after the black level is subtracted and the
    /// camera's white balance applied, scaled so the white level is the top bin, on an sRGB-like
    /// curve (U7-Q3). JPEG and TIFF: the 8-bit values themselves.
    pub histogram: [[u64; 256]; 3],
    /// The share (0 to 1) of each channel's pixels at the clipping level (U7-Q4, U7-Q5): RAW data
    /// at or above the white level before white balance; JPEG and TIFF at 255.
    pub clipped: [f64; 3],
    /// The share (0 to 1) of the pixels in the deepest shadows: RAW data with every channel below
    /// 1/256 of the white level above black; JPEG and TIFF with every channel at 0.
    pub deep_shadows: f64,
}

impl fmt::Debug for Statistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Statistics")
            .field("origin", &self.origin)
            .field("clipped", &self.clipped)
            .field("deep_shadows", &self.deep_shadows)
            .finish_non_exhaustive()
    }
}

impl Statistics {
    /// The statistics of 8-bit RGB pixels (3 bytes per pixel) as they are: a bin per value,
    /// clipped at 255 and deepest shadows at 0 in every channel (U7-Q5). Bytes after the last
    /// whole pixel are not counted.
    pub fn of_rgb8(origin: StatisticsOrigin, data: &[u8]) -> Statistics {
        let mut histogram = [[0u64; 256]; 3];
        let mut dark = 0u64;
        let mut pixels = 0u64;
        for &[r, g, b] in data.as_chunks::<3>().0 {
            for (bins, value) in histogram.iter_mut().zip([r, g, b]) {
                if let Some(bin) = bins.get_mut(usize::from(value)) {
                    *bin += 1;
                }
            }
            if r == 0 && g == 0 && b == 0 {
                dark += 1;
            }
            pixels += 1;
        }
        let share = |count: u64| {
            if pixels == 0 {
                0.0
            } else {
                count as f64 / pixels as f64
            }
        };
        Statistics {
            origin,
            clipped: [
                share(histogram[0][255]),
                share(histogram[1][255]),
                share(histogram[2][255]),
            ],
            deep_shadows: share(dark),
            histogram,
        }
    }
}

/// What is known of an opened image's statistics once they were asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageStatistics {
    Available(Arc<Statistics>),
    /// The file has none (for example RAW data that is not from an RGB sensor).
    Unavailable,
}

/// A decoder: open once, then render any number of views from the session. For RAW, opening
/// unpacks the file; for JPEG it reads the JPEG and its header, and the full decode is made when
/// a view needs it.
pub trait Backend: Send + Sync + 'static {
    /// An opened file. Used by one thread at a time.
    type Session: Send + 'static;

    /// Opens `path` as `source`. Returns the session and the size of its pixel grid (sensor
    /// orientation for RAW, the JPEG's own grid for JPEG) with the orientation to apply.
    fn open(&self, path: &Path, source: Source) -> Result<(Self::Session, ImageSize), DecodeError>;

    /// The whole image at half size.
    fn render_half(&self, session: &mut Self::Session) -> Result<Rgb, DecodeError>;

    /// Exactly the region `r` at full resolution (RAW: demosaiced with PPG).
    fn render_region(&self, session: &mut Self::Session, r: Rect) -> Result<Rgb, DecodeError>;

    /// Whether the file's embedded colour profile was present but could not be used, so its
    /// colours rest on a fallback (ADR 0016 §5). Asked once after a successful open.
    fn profile_unusable(&self, _session: &Self::Session) -> bool {
        false
    }

    /// The histogram and the exposure statistics of the opened image (ADR 0017 §2.6, U7): RAW
    /// from the unpacked RAW data, JPEG and TIFF from the decoded image before the conversion to
    /// the screen profile. The core asks once per opened image, after its first view is on the
    /// screen (U7-Q6), so this may take as long as a render. `None` when the file has none.
    fn statistics(&self, _session: &mut Self::Session) -> Option<Statistics> {
        None
    }

    /// The memory one open session holds, for the core budget.
    fn session_bytes(&self, size: ImageSize, source: Source) -> u64 {
        let px = u64::from(size.w) * u64::from(size.h);
        match source {
            // unpacked Bayer data, 16 bits per pixel (P5's estimate)
            Source::Raw => px * 2,
            // the full decode, kept for 100 % views
            Source::Jpeg => px * 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb8_statistics_count_every_value_and_the_two_ends() {
        // four pixels: white, black, a red clipped only, and a mid grey; one byte left over
        let data = [255, 255, 255, 0, 0, 0, 255, 10, 0, 128, 128, 128, 77];
        let s = Statistics::of_rgb8(StatisticsOrigin::Tiff, &data);
        assert_eq!(s.origin, StatisticsOrigin::Tiff);
        for channel in &s.histogram {
            assert_eq!(channel.iter().sum::<u64>(), 4);
        }
        assert_eq!(s.histogram[0][255], 2);
        assert_eq!(s.histogram[1][255], 1);
        assert_eq!(s.histogram[1][10], 1);
        assert_eq!(s.histogram[2][0], 2);
        assert_eq!(s.histogram[2][128], 1);
        assert_eq!(s.clipped, [0.5, 0.25, 0.25]);
        // only the black pixel has every channel at 0
        assert_eq!(s.deep_shadows, 0.25);
    }

    #[test]
    fn rgb8_statistics_of_no_pixels_are_zero() {
        let s = Statistics::of_rgb8(StatisticsOrigin::EmbeddedJpeg, &[]);
        assert_eq!(s.clipped, [0.0; 3]);
        assert_eq!(s.deep_shadows, 0.0);
        assert!(s.histogram.iter().flatten().all(|n| *n == 0));
    }

    #[test]
    fn a_backend_without_statistics_has_none() {
        struct Plain;
        impl Backend for Plain {
            type Session = ();
            fn open(&self, _: &Path, _: Source) -> Result<((), ImageSize), DecodeError> {
                Err(DecodeError::new("no files"))
            }
            fn render_half(&self, _: &mut ()) -> Result<Rgb, DecodeError> {
                Err(DecodeError::new("no image"))
            }
            fn render_region(&self, _: &mut (), _: Rect) -> Result<Rgb, DecodeError> {
                Err(DecodeError::new("no image"))
            }
        }
        assert_eq!(Plain.statistics(&mut ()), None);
    }
}
