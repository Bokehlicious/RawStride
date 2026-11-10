//! The real decoders (feature `native`): LibRaw with PPG (ADR 0008 §11) through `pw-libraw` for
//! RAW, libjpeg-turbo through `pw-libjpeg` for the embedded JPEG located by `pw-raw` (G4,
//! ADR 0009 §5.2), and `pw-tiff` for TIFF files, decoded once under the core budget (ADR 0015
//! §4.1–§4.4). Without the feature the core's tests need no native libraries.

use std::path::Path;

use pw_libjpeg::{Decoder, Scale};
use pw_libraw::{DAMAGED, Demosaic, Session};

use crate::backend::{Backend, DecodeError, Rgb, Source};
use crate::geometry::{ImageSize, Rect};
use crate::scale;

/// Context rendered around a RAW region and cut off again, so a region equals the same area of a
/// whole render (ADR 0006; checked for PPG in `pw-libraw/tests/regions.rs`).
const MARGIN: u32 = 16;

/// The TIFF memory limit of [`NativeBackend::default`]: the default core budget (4 GB).
const DEFAULT_TIFF_LIMIT: usize = 4 << 30;

const MIB: usize = 1024 * 1024;

/// LibRaw with the fixed settings of ADR 0006 and PPG, libjpeg-turbo for embedded JPEGs, and
/// `pw-tiff` for TIFF files.
#[derive(Clone, Copy, Debug)]
pub struct NativeBackend {
    /// The most memory in bytes one TIFF decode may use (the core budget, ADR 0015 Q4).
    tiff_limit: usize,
}

impl Default for NativeBackend {
    fn default() -> NativeBackend {
        NativeBackend {
            tiff_limit: DEFAULT_TIFF_LIMIT,
        }
    }
}

impl NativeBackend {
    /// A backend whose TIFF decodes may use at most `bytes` (the core budget, ADR 0015 Q4).
    pub fn with_tiff_limit(bytes: usize) -> NativeBackend {
        NativeBackend { tiff_limit: bytes }
    }
}

/// An opened file.
#[derive(Debug)]
pub enum NativeSession {
    /// An unpacked RAW file.
    Raw(Session),
    /// A JPEG: its bytes, a decoder, and the full decode once a 100 % view needed it.
    Jpeg(JpegSession),
    /// A TIFF file: the whole image, decoded once when it was opened (ADR 0015 §4.2).
    Tiff(Rgb),
}

#[derive(Debug)]
pub struct JpegSession {
    data: Vec<u8>,
    decoder: Decoder,
    full: Option<Rgb>,
}

/// LibRaw's flip code for an EXIF/TIFF orientation (mirrored orientations are shown unmirrored).
pub fn flip_of_orientation(orientation: u16) -> u8 {
    match orientation {
        3 => 3,
        6 => 6,
        8 => 5,
        _ => 0,
    }
}

fn raw_error(e: pw_libraw::Error) -> DecodeError {
    let message = e.0;
    if message.contains(DAMAGED) {
        DecodeError::damaged(message)
    } else {
        DecodeError::new(message)
    }
}

fn jpeg_error(e: pw_libjpeg::Error) -> DecodeError {
    DecodeError::new(e.to_string())
}

/// The loupe's text for a TIFF that cannot be shown (ADR 0015 §5); the loupe puts it after
/// "Filen kan inte visas: ". Never `damaged`: that is for RAW data (G3).
fn tiff_error(e: pw_tiff::TiffError) -> DecodeError {
    match e {
        pw_tiff::TiffError::Unsupported(m) => {
            DecodeError::new(format!("TIFF-varianten stöds inte än ({m})"))
        }
        pw_tiff::TiffError::TooLarge { limit } => DecodeError::new(format!(
            "TIFF-filen är för stor för minnesbudgeten ({} MB)",
            limit / MIB
        )),
        e @ (pw_tiff::TiffError::Io(_) | pw_tiff::TiffError::Decode(_)) => {
            DecodeError::new(e.to_string())
        }
    }
}

fn rgb_raw(img: pw_libraw::Rgb8) -> Rgb {
    Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    }
}

fn rgb_jpeg(img: pw_libjpeg::Rgb8) -> Rgb {
    Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    }
}

/// Opens the largest embedded JPEG of `path` (or a plain JPEG file itself).
fn open_jpeg(path: &Path) -> Result<(JpegSession, ImageSize), DecodeError> {
    let info =
        pw_raw::inspect(path).map_err(|e| DecodeError::new(format!("{}: {e}", path.display())))?;
    let Some(preview) = info.pick(pw_raw::PreviewSize::Large) else {
        return Err(DecodeError::new(format!(
            "{}: the file has no embedded JPEG",
            path.display()
        )));
    };
    let data = pw_raw::read_preview(path, preview)
        .map_err(|e| DecodeError::new(format!("{}: {e}", path.display())))?;
    let mut decoder = Decoder::new().map_err(jpeg_error)?;
    let (w, h) = decoder.header(&data).map_err(jpeg_error)?;
    // the orientation is the file's tag (ADR 0003 point 4); the JPEG's own tag is not applied
    let flip = flip_of_orientation(info.orientation);
    let session = JpegSession {
        data,
        decoder,
        full: None,
    };
    Ok((session, ImageSize { w, h, flip }))
}

/// Decodes a TIFF file whole with `pw-tiff` under `limit` bytes. The orientation is the file's
/// tag as `pw-raw` reads it, or 1 when `pw-raw` cannot inspect the file (BigTIFF, ADR 0015 §4.3).
fn open_tiff(path: &Path, limit: usize) -> Result<(Rgb, ImageSize), DecodeError> {
    let img = pw_tiff::decode_rgb8(path, Some(limit)).map_err(tiff_error)?;
    let img = Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    };
    let orientation = pw_raw::inspect(path).map_or(1, |info| info.orientation);
    let size = ImageSize {
        w: img.width,
        h: img.height,
        flip: flip_of_orientation(orientation),
    };
    Ok((img, size))
}

impl Backend for NativeBackend {
    type Session = NativeSession;

    fn open(&self, path: &Path, source: Source) -> Result<(NativeSession, ImageSize), DecodeError> {
        // the file type wins over the display mode: a TIFF is shown as itself (ADR 0015 Q3)
        if pw_raw::is_tiff_file(path) {
            let (img, size) = open_tiff(path, self.tiff_limit)?;
            return Ok((NativeSession::Tiff(img), size));
        }
        match source {
            Source::Raw => {
                let session = Session::open(path).map_err(raw_error)?;
                let size = ImageSize {
                    w: session.width(),
                    h: session.height(),
                    flip: session.flip(),
                };
                Ok((NativeSession::Raw(session), size))
            }
            Source::Jpeg => {
                let (s, size) = open_jpeg(path)?;
                Ok((NativeSession::Jpeg(s), size))
            }
        }
    }

    fn render_half(&self, session: &mut NativeSession) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(s) => s.render_half().map(rgb_raw).map_err(raw_error),
            NativeSession::Jpeg(j) => j
                .decoder
                .decode(&j.data, Scale::Half)
                .map(rgb_jpeg)
                .map_err(jpeg_error),
            NativeSession::Tiff(img) => {
                let (w, h) = scale::scaled_size(img.width, img.height, 0.5);
                scale::downscale(img, w, h).ok_or_else(|| {
                    DecodeError::new(format!(
                        "the TIFF could not be reduced to {w}x{h} (it is {}x{})",
                        img.width, img.height
                    ))
                })
            }
        }
    }

    fn render_region(&self, session: &mut NativeSession, r: Rect) -> Result<Rgb, DecodeError> {
        match session {
            NativeSession::Raw(s) => {
                let r = pw_libraw::Rect::new(r.x, r.y, r.w, r.h);
                s.render_region_with(r, MARGIN, Demosaic::Ppg)
                    .map(rgb_raw)
                    .map_err(raw_error)
            }
            NativeSession::Jpeg(j) => {
                if j.full.is_none() {
                    let full = j
                        .decoder
                        .decode(&j.data, Scale::Full)
                        .map(rgb_jpeg)
                        .map_err(jpeg_error)?;
                    j.full = Some(full);
                }
                let full = j
                    .full
                    .as_ref()
                    .ok_or_else(|| DecodeError::new("the JPEG was not decoded"))?;
                scale::crop(full, r.x, r.y, r.w, r.h)
                    .ok_or_else(|| DecodeError::new(format!("region {r:?} is outside the JPEG")))
            }
            NativeSession::Tiff(img) => scale::crop(img, r.x, r.y, r.w, r.h)
                .ok_or_else(|| DecodeError::new(format!("region {r:?} is outside the TIFF"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_tags_map_to_libraw_flips() {
        assert_eq!(flip_of_orientation(1), 0);
        assert_eq!(flip_of_orientation(3), 3);
        assert_eq!(flip_of_orientation(6), 6);
        assert_eq!(flip_of_orientation(8), 5);
        assert_eq!(flip_of_orientation(2), 0);
        assert_eq!(flip_of_orientation(0), 0);
    }

    // ---------- TIFF (ADR 0015 D2) ----------

    use std::io::Cursor;
    use std::path::PathBuf;

    use tiff::encoder::{TiffEncoder, colortype};
    use tiff::tags::Tag;

    /// Writes `bytes` to `name` in a test folder of its own.
    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pw-viewcore-native-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// An 8-bit RGB TIFF, with an Orientation tag when `orientation` is given.
    fn rgb_tiff(w: u32, h: u32, data: &[u8], orientation: Option<u16>) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut encoder = TiffEncoder::new(&mut out).unwrap();
        let mut image = encoder.new_image::<colortype::RGB8>(w, h).unwrap();
        if let Some(o) = orientation {
            image.encoder().write_tag(Tag::Orientation, o).unwrap();
        }
        image.write_data(data).unwrap();
        out.into_inner()
    }

    /// Pixel (x, y) of the test image.
    fn pixel(x: u32, y: u32) -> [u8; 3] {
        [(x * 10) as u8, (y * 10) as u8, (x + y * 6) as u8]
    }

    fn pixels(w: u32, h: u32) -> Vec<u8> {
        (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| pixel(x, y)))
            .collect()
    }

    /// A 2×2 8-bit palette TIFF (PhotometricInterpretation 3 with a ColorMap), built by hand.
    /// Copied from `crates/pw-tiff/src/tests.rs`.
    fn palette_tiff() -> Vec<u8> {
        const ENTRIES: u16 = 10;
        let ifd_end = 8 + 2 + 12 * u32::from(ENTRIES) + 4;
        let colormap_at = ifd_end;
        let pixels_at = colormap_at + 3 * 256 * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"II");
        b.extend_from_slice(&42u16.to_le_bytes());
        b.extend_from_slice(&8u32.to_le_bytes());
        b.extend_from_slice(&ENTRIES.to_le_bytes());
        let mut entry = |tag: u16, kind: u16, count: u32, value: u32| {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&kind.to_le_bytes());
            b.extend_from_slice(&count.to_le_bytes());
            b.extend_from_slice(&value.to_le_bytes());
        };
        const SHORT: u16 = 3;
        const LONG: u16 = 4;
        entry(256, SHORT, 1, 2); // ImageWidth
        entry(257, SHORT, 1, 2); // ImageLength
        entry(258, SHORT, 1, 8); // BitsPerSample
        entry(259, SHORT, 1, 1); // Compression: none
        entry(262, SHORT, 1, 3); // PhotometricInterpretation: palette
        entry(273, LONG, 1, pixels_at); // StripOffsets
        entry(277, SHORT, 1, 1); // SamplesPerPixel
        entry(278, SHORT, 1, 2); // RowsPerStrip
        entry(279, LONG, 1, 4); // StripByteCounts
        entry(320, SHORT, 3 * 256, colormap_at); // ColorMap
        b.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        assert_eq!(b.len() as u32, colormap_at);
        for channel in 0..3u32 {
            for i in 0..256u32 {
                let v = ((i * (channel + 1)) % 256) as u16 * 257;
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        assert_eq!(b.len() as u32, pixels_at);
        b.extend_from_slice(&[0, 1, 2, 3]);
        b
    }

    #[test]
    fn a_tiff_opens_whole_and_renders_half_and_regions() {
        let path = temp_file("rgb-6x4.tif", &rgb_tiff(6, 4, &pixels(6, 4), None));
        let backend = NativeBackend::default();
        let (mut session, size) = backend.open(&path, Source::Jpeg).unwrap();
        assert!(matches!(session, NativeSession::Tiff(_)), "{session:?}");
        assert_eq!(
            size,
            ImageSize {
                w: 6,
                h: 4,
                flip: 0
            }
        );

        let half = backend.render_half(&mut session).unwrap();
        assert_eq!((half.width, half.height), (3, 2));
        assert!(half.is_consistent());

        let region = backend
            .render_region(&mut session, Rect::new(1, 2, 3, 2))
            .unwrap();
        assert_eq!((region.width, region.height), (3, 2));
        let expected: Vec<u8> = (2..4)
            .flat_map(|y| (1..4).flat_map(move |x| pixel(x, y)))
            .collect();
        assert_eq!(region.data, expected);

        assert!(
            backend
                .render_region(&mut session, Rect::new(4, 0, 3, 1))
                .is_err()
        );
    }

    #[test]
    fn a_tiff_opened_as_raw_is_the_same_tiff_session() {
        let path = temp_file("as-raw.tif", &rgb_tiff(6, 4, &pixels(6, 4), None));
        let backend = NativeBackend::default();
        let (mut session, size) = backend.open(&path, Source::Raw).unwrap();
        assert!(matches!(session, NativeSession::Tiff(_)), "{session:?}");
        assert_eq!(
            size,
            ImageSize {
                w: 6,
                h: 4,
                flip: 0
            }
        );
        let all = backend
            .render_region(&mut session, Rect::new(0, 0, 6, 4))
            .unwrap();
        assert_eq!(all.data, pixels(6, 4));
    }

    #[test]
    fn a_tiff_with_orientation_6_turns() {
        let path = temp_file("orientation-6.tif", &rgb_tiff(6, 4, &pixels(6, 4), Some(6)));
        let (_, size) = NativeBackend::default().open(&path, Source::Jpeg).unwrap();
        assert_eq!(
            size,
            ImageSize {
                w: 6,
                h: 4,
                flip: 6
            }
        );
    }

    #[test]
    fn a_palette_tiff_is_not_supported_yet_and_not_damaged() {
        let path = temp_file("palette.tif", &palette_tiff());
        let err = NativeBackend::default()
            .open(&path, Source::Jpeg)
            .unwrap_err();
        assert!(
            err.message.starts_with("TIFF-varianten stöds inte än ("),
            "{err:?}"
        );
        assert!(!err.damaged);
    }

    #[test]
    fn a_tiff_larger_than_the_limit_is_too_large_for_the_budget() {
        let (w, h) = (64, 64);
        let path = temp_file("limit.tif", &rgb_tiff(w, h, &pixels(w, h), None));
        let err = NativeBackend::with_tiff_limit(1024)
            .open(&path, Source::Jpeg)
            .unwrap_err();
        assert!(
            err.message.contains("för stor för minnesbudgeten"),
            "{err:?}"
        );
        assert!(!err.damaged);
        // the default limit decodes it
        assert!(NativeBackend::default().open(&path, Source::Jpeg).is_ok());
    }

    #[test]
    fn tiff_errors_get_the_loupe_texts() {
        use pw_tiff::TiffError;
        let e = tiff_error(TiffError::Unsupported("TIFF: palette".into()));
        assert_eq!(
            e,
            DecodeError::new("TIFF-varianten stöds inte än (TIFF: palette)")
        );
        let e = tiff_error(TiffError::TooLarge { limit: 512 << 20 });
        assert_eq!(
            e,
            DecodeError::new("TIFF-filen är för stor för minnesbudgeten (512 MB)")
        );
        let e = tiff_error(TiffError::Io("x.tif: not found".into()));
        assert_eq!(e, DecodeError::new("x.tif: not found"));
        let e = tiff_error(TiffError::Decode("TIFF: bad strip".into()));
        assert_eq!(e, DecodeError::new("TIFF: bad strip"));
    }
}
