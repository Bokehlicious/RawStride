//! The real RAW decoder: LibRaw with PPG (ADR 0008 §11) through `pw-libraw`. Behind the `libraw`
//! feature, so the core's tests need no native libraries.

use std::path::Path;

use pw_libraw::{DAMAGED, Demosaic, Session};

use crate::backend::{DecodeError, RawBackend, Rgb};
use crate::geometry::{ImageSize, Rect};

/// Context rendered around a region and cut off again, so a region equals the same area of a whole
/// render (ADR 0006; checked for PPG in `pw-libraw/tests/regions.rs`).
const MARGIN: u32 = 16;

/// LibRaw with the fixed settings of ADR 0006 and PPG demosaicing.
#[derive(Clone, Copy, Debug, Default)]
pub struct LibRawBackend;

fn error(e: pw_libraw::Error) -> DecodeError {
    let message = e.0;
    if message.contains(DAMAGED) {
        DecodeError::damaged(message)
    } else {
        DecodeError::new(message)
    }
}

fn rgb(img: pw_libraw::Rgb8) -> Rgb {
    Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    }
}

impl RawBackend for LibRawBackend {
    type Session = Session;

    fn open(&self, path: &Path) -> Result<(Session, ImageSize), DecodeError> {
        let session = Session::open(path).map_err(error)?;
        let size = ImageSize {
            w: session.width(),
            h: session.height(),
            flip: session.flip(),
        };
        Ok((session, size))
    }

    fn render_half(&self, session: &mut Session) -> Result<Rgb, DecodeError> {
        session.render_half().map(rgb).map_err(error)
    }

    fn render_region(&self, session: &mut Session, r: Rect) -> Result<Rgb, DecodeError> {
        let r = pw_libraw::Rect::new(r.x, r.y, r.w, r.h);
        session
            .render_region_with(r, MARGIN, Demosaic::Ppg)
            .map(rgb)
            .map_err(error)
    }
}
