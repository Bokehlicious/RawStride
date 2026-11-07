//! The real decoders (feature `native`): LibRaw with PPG (ADR 0008 §11) through `pw-libraw` for
//! RAW, and libjpeg-turbo through `pw-libjpeg` for the embedded JPEG located by `pw-raw` (G4,
//! ADR 0009 §5.2). Without the feature the core's tests need no native libraries.

use std::path::Path;

use pw_libjpeg::{Decoder, Scale};
use pw_libraw::{DAMAGED, Demosaic, Session};

use crate::backend::{Backend, DecodeError, Rgb, Source};
use crate::geometry::{ImageSize, Rect};
use crate::scale;

/// Context rendered around a RAW region and cut off again, so a region equals the same area of a
/// whole render (ADR 0006; checked for PPG in `pw-libraw/tests/regions.rs`).
const MARGIN: u32 = 16;

/// LibRaw with the fixed settings of ADR 0006 and PPG, and libjpeg-turbo for embedded JPEGs.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeBackend;

/// An opened file.
#[derive(Debug)]
pub enum NativeSession {
    /// An unpacked RAW file.
    Raw(Session),
    /// A JPEG: its bytes, a decoder, and the full decode once a 100 % view needed it.
    Jpeg(JpegSession),
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

impl Backend for NativeBackend {
    type Session = NativeSession;

    fn open(&self, path: &Path, source: Source) -> Result<(NativeSession, ImageSize), DecodeError> {
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
}
