//! RAW and JPEG inspection for RawStride.
//!
//! Fast path for culling: locate the embedded JPEG previews and the basic EXIF fields by
//! walking the TIFF structure, without decoding any image data. The previews are returned
//! as byte ranges so callers can read exactly the bytes they need.
//!
//! Supported containers (development phase): Sony ARW, Panasonic RW2, DNG, Fujifilm RAF and plain
//! JPEG.
//! Other TIFF-based RAW formats usually work as well. Full RAW decoding (LibRaw) comes later.

mod jpeg;
mod raf;
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
    /// Fujifilm RAF: an own header with an embedded JPEG.
    Raf,
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

/// The colour space the Exif data names (ADR 0016 §3.1, §4.1). For a RAW file it is the camera's
/// colour-space setting, which its embedded JPEG previews follow; for a JPEG file it comes from
/// its APP1 Exif.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExifColourSpace {
    /// No ColorSpace tag, or a value other than 1 and 0xFFFF.
    #[default]
    Unknown,
    /// ColorSpace 1.
    Srgb,
    /// ColorSpace 0xFFFF with InteroperabilityIndex "R03" (the DCF rule for Adobe RGB).
    AdobeRgb,
    /// ColorSpace 0xFFFF without the "R03" index.
    Uncalibrated,
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
    /// Exif ColorSpace (0xA001) with the Interoperability index, from the first Exif IFD that
    /// has it.
    pub colour_space: ExifColourSpace,
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
    if src.len() >= raf::MAGIC.len() as u64 {
        let mut head = [0u8; 16];
        src.read_at(0, &mut head)?;
        if &head == raf::MAGIC {
            return raf::inspect_raf(src);
        }
    }
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
    "arw", "rw2", "dng", "nef", "cr2", "orf", "pef", "srw", "raf", "jpg", "jpeg", "tif", "tiff",
];

/// True for plain TIFF files (`.tif`/`.tiff`). These usually carry no embedded JPEG preview,
/// so the app decodes the image data itself.
pub fn is_tiff_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal baseline JPEG header: SOI, optional APP1 Exif, SOF0 with the given size, EOI.
    pub(crate) fn tiny_jpeg(w: u16, h: u16, orientation: Option<u16>) -> Vec<u8> {
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

    /// Writes one little-endian IFD at `at` (entries as tag, type, count, inline value or offset).
    fn put_ifd(f: &mut [u8], at: usize, entries: &[(u16, u16, u32, u32)]) {
        f[at..at + 2].copy_from_slice(&(entries.len() as u16).to_le_bytes());
        for (k, &(tag, typ, count, val)) in entries.iter().enumerate() {
            let e = at + 2 + 12 * k;
            f[e..e + 2].copy_from_slice(&tag.to_le_bytes());
            f[e + 2..e + 4].copy_from_slice(&typ.to_le_bytes());
            f[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
            f[e + 8..e + 12].copy_from_slice(&val.to_le_bytes());
        }
        let n = at + 2 + 12 * entries.len();
        f[n..n + 4].copy_from_slice(&0u32.to_le_bytes());
    }

    /// The Interoperability IFD of [`exif_colour_tiff`].
    #[derive(Clone, Copy)]
    enum Interop {
        None,
        /// An Interop IFD with this InteroperabilityIndex (3 characters).
        Index(&'static [u8; 3]),
        /// An Interop offset pointing outside the file.
        Outside,
    }

    /// A little-endian TIFF: IFD0 (4×4, Exif IFD) → Exif IFD (ISO, optional ColorSpace, optional
    /// Interop IFD) → Interop IFD (InteroperabilityIndex).
    fn exif_colour_tiff(colour_space: Option<u16>, interop: Interop) -> Vec<u8> {
        let (exif_at, interop_at) = (100u32, 200u32);
        let mut f = vec![0u8; 256];
        f[0..4].copy_from_slice(b"II*\0");
        f[4..8].copy_from_slice(&8u32.to_le_bytes());
        put_ifd(
            &mut f,
            8,
            &[
                (0x0100, 4, 1, 4),
                (0x0101, 4, 1, 4),
                (0x8769, 4, 1, exif_at),
            ],
        );
        let mut exif = vec![(0x8827, 3, 1, 100)];
        if let Some(cs) = colour_space {
            exif.push((0xA001, 3, 1, u32::from(cs)));
        }
        match interop {
            Interop::None => {}
            Interop::Index(index) => {
                exif.push((0xA005, 4, 1, interop_at));
                let value = u32::from_le_bytes([index[0], index[1], index[2], 0]);
                put_ifd(&mut f, interop_at as usize, &[(0x0001, 2, 4, value)]);
            }
            Interop::Outside => exif.push((0xA005, 4, 1, 0x00FF_FFF0)),
        }
        put_ifd(&mut f, exif_at as usize, &exif);
        f
    }

    fn colour_space_of(data: &[u8]) -> ExifColourSpace {
        inspect_source(&mut SliceSource(data)).unwrap().colour_space
    }

    #[test]
    fn exif_colour_space_srgb() {
        assert_eq!(
            colour_space_of(&exif_colour_tiff(Some(1), Interop::None)),
            ExifColourSpace::Srgb
        );
    }

    #[test]
    fn exif_colour_space_uncalibrated_with_r03_is_adobe_rgb() {
        assert_eq!(
            colour_space_of(&exif_colour_tiff(Some(0xFFFF), Interop::Index(b"R03"))),
            ExifColourSpace::AdobeRgb
        );
    }

    #[test]
    fn exif_colour_space_uncalibrated_with_r98_is_uncalibrated() {
        assert_eq!(
            colour_space_of(&exif_colour_tiff(Some(0xFFFF), Interop::Index(b"R98"))),
            ExifColourSpace::Uncalibrated
        );
    }

    #[test]
    fn exif_colour_space_uncalibrated_without_interop_ifd() {
        assert_eq!(
            colour_space_of(&exif_colour_tiff(Some(0xFFFF), Interop::None)),
            ExifColourSpace::Uncalibrated
        );
    }

    #[test]
    fn exif_colour_space_missing_or_other_value_is_unknown() {
        assert_eq!(
            colour_space_of(&exif_colour_tiff(None, Interop::None)),
            ExifColourSpace::Unknown
        );
        assert_eq!(
            colour_space_of(&exif_colour_tiff(Some(2), Interop::Index(b"R03"))),
            ExifColourSpace::Unknown
        );
        // tiny_tiff has an Exif IFD without ColorSpace
        assert_eq!(colour_space_of(&tiny_tiff()), ExifColourSpace::Unknown);
    }

    #[test]
    fn exif_colour_space_interop_outside_the_file_is_uncalibrated() {
        assert_eq!(
            colour_space_of(&exif_colour_tiff(Some(0xFFFF), Interop::Outside)),
            ExifColourSpace::Uncalibrated
        );
    }

    #[test]
    fn exif_colour_space_of_a_jpeg_file_from_app1() {
        let tiff = exif_colour_tiff(Some(1), Interop::None);
        let mut j = vec![0xFF, 0xD8, 0xFF, 0xE1];
        j.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
        j.extend_from_slice(b"Exif\0\0");
        j.extend_from_slice(&tiff);
        j.extend_from_slice(&tiny_jpeg(64, 48, None)[2..]);
        let info = inspect_source(&mut SliceSource(&j)).unwrap();
        assert_eq!(info.format, Some(Format::Jpeg));
        assert_eq!(info.colour_space, ExifColourSpace::Srgb);
        // a JPEG without Exif
        assert_eq!(
            colour_space_of(&tiny_jpeg(64, 48, None)),
            ExifColourSpace::Unknown
        );
    }

    /// Prints the Exif colour space of every file in `PW_TESTDATA` that `pw-raw` lists; asserts
    /// nothing (the reviewer reads the output). Run with `-- --ignored --nocapture`.
    #[test]
    #[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
    fn real_files_print_exif_colour_space() {
        let directory = std::env::var_os("PW_TESTDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| panic!("PW_TESTDATA is not set"));
        let mut files: Vec<_> = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.is_file()
                    && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())
                    })
            })
            .collect();
        files.sort();
        assert!(
            !files.is_empty(),
            "no supported file in PW_TESTDATA: {}",
            directory.display()
        );
        for path in files {
            match inspect(&path) {
                Ok(info) => println!("{}: {:?}", path.display(), info.colour_space),
                Err(error) => println!("{}: inspect failed: {error}", path.display()),
            }
        }
    }

    #[test]
    fn exposure_formatting() {
        assert_eq!(ExposureTime { num: 1, den: 500 }.to_string(), "1/500 s");
        assert_eq!(ExposureTime { num: 10, den: 4000 }.to_string(), "1/400 s");
        assert_eq!(ExposureTime { num: 2, den: 1 }.to_string(), "2 s");
        assert_eq!(ExposureTime { num: 13, den: 10 }.to_string(), "1.3 s");
    }
}
