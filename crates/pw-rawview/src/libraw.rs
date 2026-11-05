//! The real decoder: LibRaw with OpenMP (`pw-libraw`) and JPEG encoding (quality 95).

use std::path::Path;

use crate::{Backend, Image, Rect};

/// JPEG quality for tiles and half-size images (ADR 0006).
pub const JPEG_QUALITY: u8 = 95;

/// [`Backend`] using LibRaw. Stateless; sessions are LibRaw instances.
#[derive(Debug, Clone, Copy, Default)]
pub struct LibRawBackend;

fn image(i: pw_libraw::Rgb8) -> Image {
    Image {
        width: i.width,
        height: i.height,
        data: i.data,
    }
}

impl Backend for LibRawBackend {
    type Session = pw_libraw::Session;

    fn open(&self, path: &Path) -> Result<Self::Session, String> {
        pw_libraw::Session::open(path).map_err(|e| e.to_string())
    }

    fn size(&self, s: &Self::Session) -> (u32, u32) {
        (s.width(), s.height())
    }

    fn render_half(&self, s: &mut Self::Session) -> Result<Image, String> {
        s.render_half().map(image).map_err(|e| e.to_string())
    }

    fn render_region(&self, s: &mut Self::Session, r: Rect, margin: u32) -> Result<Image, String> {
        s.render_region(pw_libraw::Rect::new(r.x, r.y, r.w, r.h), margin)
            .map(image)
            .map_err(|e| e.to_string())
    }

    fn encode(&self, img: &Image) -> Result<Vec<u8>, String> {
        let (Ok(w), Ok(h)) = (u16::try_from(img.width), u16::try_from(img.height)) else {
            return Err(format!(
                "{}x{} is too large for JPEG",
                img.width, img.height
            ));
        };
        let mut out = Vec::with_capacity(img.data.len() / 6);
        jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY)
            .encode(&img.data, w, h, jpeg_encoder::ColorType::Rgb)
            .map_err(|e| format!("JPEG: {e}"))?;
        Ok(out)
    }
}
