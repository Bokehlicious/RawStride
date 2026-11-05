//! RAW and JPEG inspection for PhotoWorkflow.
//!
//! Fast path for culling: locate the embedded JPEG previews and the basic EXIF fields by
//! walking the TIFF structure, without decoding any image data. The previews are returned
//! as byte ranges so callers can read exactly the bytes they need.
//!
//! Supported containers (development phase): Sony ARW, Panasonic RW2, DNG and plain JPEG.
//! Other TIFF-based RAW formats usually work as well. Full RAW decoding (LibRaw) comes later.

mod jpeg;
mod source;
mod tiff;

use std::fmt;
use std::io;
use std::path::Path;

pub use jpeg::neutralize_orientation;
pub use source::{FileSource, SliceSource, Source};

/// Errors from inspecting or reading a file.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// The file is not in a supported format or is malformed.
    Format(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Format(m) => write!(f, "format error: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Container format detected from the file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// TIFF-based RAW (ARW, DNG, NEF, CR2, …) or plain TIFF.
    Tiff,
    /// Panasonic RW2/RAW (TIFF variant with magic 0x55).
    Panasonic,
    /// A plain JPEG file.
    Jpeg,
}

/// One embedded JPEG image (thumbnail, preview or full-size JPEG).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRef {
    /// Absolute byte offset of the JPEG (starts with FF D8).
    pub offset: u64,
    /// Length of the JPEG in bytes.
    pub length: u64,
    pub width: u32,
    pub height: u32,
    /// Where it was found, for diagnostics (e.g. "IFD2/JPEGInterchangeFormat").
    pub source: String,
}

/// Which embedded preview a caller wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSize {
    /// Grid thumbnails: the smallest preview at least 320 px wide (else the largest).
    Small,
    /// Screen-sized: the largest preview at most 2560 px wide (else the smallest).
    Medium,
    /// The largest preview available.
    Large,
}

/// Exposure time as a rational number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExposureTime {
    pub num: u32,
    pub den: u32,
}

impl fmt::Display for ExposureTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.num == 0 || self.den == 0 {
            return write!(f, "?");
        }
        let secs = f64::from(self.num) / f64::from(self.den);
        if secs >= 0.3 {
            let s = format!("{secs:.1}");
            write!(f, "{} s", s.trim_end_matches(".0"))
        } else {
            write!(f, "1/{} s", (1.0 / secs).round() as u64)
        }
    }
}

/// What the viewer needs to know about one image file.
#[derive(Debug, Clone, Default)]
pub struct ImageInfo {
    pub format: Option<Format>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    /// EXIF DateTimeOriginal as stored ("YYYY:MM:DD HH:MM:SS").
    pub date_time_original: Option<String>,
    pub exposure_time: Option<ExposureTime>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub focal_length: Option<f64>,
    pub exposure_bias: Option<f64>,
    /// EXIF/TIFF orientation (1 = normal, 3 = 180°, 6 = 90° CW, 8 = 270° CW). Defaults to 1.
    pub orientation: u16,
    /// Dimensions of the sensor image (RAW) or of the JPEG itself.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Embedded JPEGs, sorted by width (smallest first).
    pub previews: Vec<PreviewRef>,
}

impl ImageInfo {
    /// Picks the embedded preview best suited for `size`.
    pub fn pick(&self, size: PreviewSize) -> Option<&PreviewRef> {
        let p = &self.previews;
        match size {
            PreviewSize::Small => p.iter().find(|x| x.width >= 320).or(p.last()),
            PreviewSize::Medium => p.iter().rev().find(|x| x.width <= 2560).or(p.first()),
            PreviewSize::Large => p.last(),
        }
    }
}

/// Inspects a file on disk: format, EXIF summary and embedded previews.
pub fn inspect(path: &Path) -> Result<ImageInfo> {
    let mut src = FileSource::open(path)?;
    inspect_source(&mut src)
}

/// Inspects any byte source (used by `inspect` and by tests).
pub fn inspect_source<S: Source>(src: &mut S) -> Result<ImageInfo> {
    let mut magic = [0u8; 4];
    src.read_at(0, &mut magic)?;
    if magic[0] == 0xFF && magic[1] == 0xD8 {
        return jpeg::inspect_jpeg_file(src);
    }
    tiff::inspect_tiff(src)
}

/// Reads the bytes of one embedded preview.
pub fn read_preview(path: &Path, preview: &PreviewRef) -> Result<Vec<u8>> {
    let mut src = FileSource::open(path)?;
    let len =
        usize::try_from(preview.length).map_err(|_| Error::Format("preview too large".into()))?;
    let mut buf = vec![0u8; len];
    src.read_at(preview.offset, &mut buf)?;
    Ok(buf)
}

/// File extensions (lower case, without dot) the viewer lists.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "arw", "rw2", "dng", "nef", "cr2", "orf", "pef", "srw", "jpg", "jpeg", "tif", "tiff",
];

/// True for plain TIFF files (`.tif`/`.tiff`). These usually carry no embedded JPEG preview,
/// so the app decodes the image data itself.
pub fn is_tiff_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal baseline JPEG header: SOI, optional APP1 Exif, SOF0 with the given size, EOI.
    fn tiny_jpeg(w: u16, h: u16, orientation: Option<u16>) -> Vec<u8> {
        let mut j = vec![0xFF, 0xD8];
        if let Some(o) = orientation {
            // TIFF (LE): header + IFD0 with one entry (Orientation, SHORT, 1, o)
            let mut t = b"II*\0".to_vec();
            t.extend_from_slice(&8u32.to_le_bytes());
            t.extend_from_slice(&1u16.to_le_bytes());
            t.extend_from_slice(&0x0112u16.to_le_bytes());
            t.extend_from_slice(&3u16.to_le_bytes());
            t.extend_from_slice(&1u32.to_le_bytes());
            t.extend_from_slice(&o.to_le_bytes());
            t.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
            let len = (2 + 6 + t.len()) as u16;
            j.extend_from_slice(&[0xFF, 0xE1]);
            j.extend_from_slice(&len.to_be_bytes());
            j.extend_from_slice(b"Exif\0\0");
            j.extend_from_slice(&t);
        }
        j.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        j.extend_from_slice(&h.to_be_bytes());
        j.extend_from_slice(&w.to_be_bytes());
        j.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        j.extend_from_slice(&[0xFF, 0xD9]);
        j
    }

    /// A little-endian TIFF with IFD0 (Make, Orientation, ExifIFD, JPEG preview) -> IFD1 (thumb).
    fn tiny_tiff() -> Vec<u8> {
        let preview = tiny_jpeg(1616, 1080, None);
        let thumb = tiny_jpeg(160, 120, None);
        let mut f = vec![0u8; 512];
        f[0..4].copy_from_slice(b"II*\0");
        f[4..8].copy_from_slice(&8u32.to_le_bytes());
        let put = |f: &mut Vec<u8>, at: usize, entries: &[(u16, u16, u32, u32)], next: u32| {
            f[at..at + 2].copy_from_slice(&(entries.len() as u16).to_le_bytes());
            for (k, &(tag, typ, count, val)) in entries.iter().enumerate() {
                let e = at + 2 + 12 * k;
                f[e..e + 2].copy_from_slice(&tag.to_le_bytes());
                f[e + 2..e + 4].copy_from_slice(&typ.to_le_bytes());
                f[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
                f[e + 8..e + 12].copy_from_slice(&val.to_le_bytes());
            }
            let n = at + 2 + 12 * entries.len();
            f[n..n + 4].copy_from_slice(&next.to_le_bytes());
        };
        let (make_at, exif_at, ifd1_at, rat_at) = (200u32, 220u32, 300u32, 400u32);
        let prev_at = 512u32;
        let thumb_at = prev_at + preview.len() as u32;
        put(
            &mut f,
            8,
            &[
                (0x010F, 2, 5, make_at),
                (0x0112, 3, 1, 8),
                (0x0201, 4, 1, prev_at),
                (0x0202, 4, 1, preview.len() as u32),
                (0x8769, 4, 1, exif_at),
            ],
            ifd1_at,
        );
        f[make_at as usize..make_at as usize + 5].copy_from_slice(b"SONY\0");
        put(
            &mut f,
            exif_at as usize,
            &[(0x829D, 5, 1, rat_at), (0x8827, 3, 1, 400)],
            0,
        );
        f[rat_at as usize..rat_at as usize + 8].copy_from_slice(&[28, 0, 0, 0, 10, 0, 0, 0]);
        put(
            &mut f,
            ifd1_at as usize,
            &[(0x0201, 4, 1, thumb_at), (0x0202, 4, 1, thumb.len() as u32)],
            0,
        );
        f.extend_from_slice(&preview);
        f.extend_from_slice(&thumb);
        f
    }

    #[test]
    fn tiff_previews_and_exif() {
        let data = tiny_tiff();
        let info = inspect_source(&mut SliceSource(&data)).unwrap();
        assert_eq!(info.format, Some(Format::Tiff));
        assert_eq!(info.make.as_deref(), Some("SONY"));
        assert_eq!(info.orientation, 8);
        assert_eq!(info.iso, Some(400));
        assert_eq!(info.f_number, Some(2.8));
        let sizes: Vec<_> = info.previews.iter().map(|p| (p.width, p.height)).collect();
        assert_eq!(sizes, vec![(160, 120), (1616, 1080)]);
        assert_eq!(info.pick(PreviewSize::Small).unwrap().width, 1616);
        assert_eq!(info.pick(PreviewSize::Large).unwrap().width, 1616);
    }

    /// A plain 8-bit RGB TIFF (no previews): IFD0 300×200, Orientation 6, one strip.
    fn plain_rgb_tiff() -> Vec<u8> {
        let (w, h) = (300u32, 200u32);
        let mut f = b"II*\0".to_vec();
        f.extend_from_slice(&8u32.to_le_bytes());
        let entries: [(u16, u16, u32, u32); 8] = [
            (0x0100, 4, 1, w),         // ImageWidth
            (0x0101, 4, 1, h),         // ImageLength
            (0x0103, 3, 1, 1),         // Compression: none
            (0x0106, 3, 1, 2),         // Photometric: RGB
            (0x0111, 4, 1, 0),         // StripOffsets (patched below)
            (0x0112, 3, 1, 6),         // Orientation
            (0x0115, 3, 1, 3),         // SamplesPerPixel
            (0x0117, 4, 1, w * h * 3), // StripByteCounts
        ];
        f.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let data_at = 8 + 2 + entries.len() as u32 * 12 + 4;
        for (tag, typ, count, val) in entries {
            f.extend_from_slice(&tag.to_le_bytes());
            f.extend_from_slice(&typ.to_le_bytes());
            f.extend_from_slice(&count.to_le_bytes());
            let val = if tag == 0x0111 { data_at } else { val };
            if typ == 3 {
                f.extend_from_slice(&(val as u16).to_le_bytes());
                f.extend_from_slice(&[0, 0]);
            } else {
                f.extend_from_slice(&val.to_le_bytes());
            }
        }
        f.extend_from_slice(&[0, 0, 0, 0]);
        f.resize(f.len() + (w * h * 3) as usize, 0x80);
        f
    }

    #[test]
    fn plain_tiff_file() {
        let data = plain_rgb_tiff();
        let info = inspect_source(&mut SliceSource(&data)).unwrap();
        assert_eq!(info.format, Some(Format::Tiff));
        assert_eq!((info.width, info.height), (Some(300), Some(200)));
        assert_eq!(info.orientation, 6);
        assert!(info.previews.is_empty());
        assert!(is_tiff_file(Path::new("x/Scan 01.TIF")));
        assert!(!is_tiff_file(Path::new("x/a.dng")));
    }

    #[test]
    fn plain_jpeg_file() {
        let data = tiny_jpeg(4000, 3000, Some(6));
        let info = inspect_source(&mut SliceSource(&data)).unwrap();
        assert_eq!(info.format, Some(Format::Jpeg));
        assert_eq!((info.width, info.height), (Some(4000), Some(3000)));
        assert_eq!(info.orientation, 6);
        assert_eq!(info.previews.len(), 1);
    }

    #[test]
    fn orientation_is_neutralized() {
        let mut data = tiny_jpeg(100, 50, Some(6));
        assert!(neutralize_orientation(&mut data));
        let info = inspect_source(&mut SliceSource(&data)).unwrap();
        assert_eq!(info.orientation, 1);
        assert!(
            !neutralize_orientation(&mut data),
            "second call changes nothing"
        );
        let mut plain = tiny_jpeg(100, 50, None);
        assert!(!neutralize_orientation(&mut plain));
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(inspect_source(&mut SliceSource(b"hello world")).is_err());
        assert!(inspect_source(&mut SliceSource(b"II*\0\xff\xff\xff\xff")).is_err());
    }

    #[test]
    fn exposure_formatting() {
        assert_eq!(ExposureTime { num: 1, den: 500 }.to_string(), "1/500 s");
        assert_eq!(ExposureTime { num: 10, den: 4000 }.to_string(), "1/400 s");
        assert_eq!(ExposureTime { num: 2, den: 1 }.to_string(), "2 s");
        assert_eq!(ExposureTime { num: 13, den: 10 }.to_string(), "1.3 s");
    }
}
