//! Viewing plain TIFF files (`.tif`/`.tiff`).
//!
//! Unlike RAW files, TIFFs from scanners and editors usually carry no embedded JPEG preview, so
//! the image data is decoded with the `tiff` crate, scaled down (`imaging`) and served to the
//! WebView as JPEG (`jpeg-encoder`), at the same three sizes as RAW previews:
//! small ≤ 640 px, medium ≤ 2560 px (long edge) and large = full resolution.
//!
//! - Decoding is serialised (one TIFF at a time) so a grid full of large TIFFs cannot exhaust
//!   memory; a 61 MP 16-bit RGB TIFF needs about 360 MB while decoding.
//! - Encoded JPEGs are kept in a small FIFO cache (by bytes), keyed by folder generation, index
//!   and size, so switching back and forth does not decode again.
//! - Orientation is not applied here: the viewer rotates with CSS from the TIFF's IFD0
//!   Orientation, exactly as for RAW previews.
//! - Colour management (embedded ICC profiles) is not applied yet; CMYK is converted naively.

use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use pw_raw::PreviewSize;
use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, Limits};

use crate::imaging::{self, Layout, Rgb8};

/// Long-edge limits of the generated sizes (0 = full resolution).
pub const SMALL_EDGE: u32 = 640;
pub const MEDIUM_EDGE: u32 = 2560;
const CACHE_BYTES: usize = 256 * 1024 * 1024;
const JPEG_QUALITY: u8 = 90;

pub fn max_edge(size: PreviewSize) -> u32 {
    match size {
        PreviewSize::Small => SMALL_EDGE,
        PreviewSize::Medium => MEDIUM_EDGE,
        PreviewSize::Large => 0,
    }
}

fn size_key(size: PreviewSize) -> u8 {
    match size {
        PreviewSize::Small => 0,
        PreviewSize::Medium => 1,
        PreviewSize::Large => 2,
    }
}

type Key = (u64, usize, u8);

#[derive(Debug, Default)]
struct Cache {
    entries: VecDeque<(Key, Arc<Vec<u8>>)>,
    bytes: usize,
}

impl Cache {
    fn get(&self, key: Key) -> Option<Arc<Vec<u8>>> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
    }

    fn put(&mut self, key: Key, jpeg: Arc<Vec<u8>>) {
        if self.get(key).is_some() {
            return;
        }
        self.bytes += jpeg.len();
        self.entries.push_back((key, jpeg));
        while self.bytes > CACHE_BYTES && self.entries.len() > 1 {
            if let Some((_, old)) = self.entries.pop_front() {
                self.bytes -= old.len();
            }
        }
    }
}

/// Decoded-TIFF JPEG cache plus the decode lock.
#[derive(Debug, Default)]
pub struct TiffView {
    cache: Mutex<Cache>,
    decode: Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl TiffView {
    /// JPEG bytes of `path` at `size`. Decodes the TIFF on a cache miss; the small and medium
    /// sizes are produced together because the decode dominates the cost.
    pub fn jpeg(
        &self,
        path: &Path,
        generation: u64,
        index: usize,
        size: PreviewSize,
    ) -> Result<Arc<Vec<u8>>, String> {
        let key = (generation, index, size_key(size));
        if let Some(j) = lock(&self.cache).get(key) {
            return Ok(j);
        }
        let _decoding = lock(&self.decode);
        // Another request may have produced it while we waited.
        if let Some(j) = lock(&self.cache).get(key) {
            return Ok(j);
        }
        let full = decode_rgb8(path)?;
        let wanted: &[PreviewSize] = match size {
            PreviewSize::Large => &[PreviewSize::Large],
            _ => &[PreviewSize::Small, PreviewSize::Medium],
        };
        let mut result = None;
        for &s in wanted {
            let (tw, th) = imaging::fit(full.width, full.height, max_edge(s));
            let jpeg = Arc::new(if (tw, th) == (full.width, full.height) {
                encode_jpeg(&full)?
            } else {
                encode_jpeg(&imaging::downscale(&full, tw, th))?
            });
            lock(&self.cache).put((generation, index, size_key(s)), jpeg.clone());
            if s == size {
                result = Some(jpeg);
            }
        }
        result.ok_or_else(|| "internal: size not produced".to_string())
    }
}

fn open(path: &Path) -> Result<Decoder<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let dec =
        Decoder::new(BufReader::with_capacity(1 << 20, file)).map_err(|e| format!("TIFF: {e}"))?;
    // Default limits (256 MB) reject large 16-bit files from high-resolution cameras.
    Ok(dec.with_limits(Limits::unlimited()))
}

/// Image size from the TIFF decoder (used when pw-raw cannot inspect the file, e.g. BigTIFF).
pub fn dimensions(path: &Path) -> Result<(u32, u32), String> {
    open(path)?.dimensions().map_err(|e| format!("TIFF: {e}"))
}

/// Decodes the first image of a TIFF to 8-bit RGB.
pub fn decode_rgb8(path: &Path) -> Result<Rgb8, String> {
    let mut dec = open(path)?;
    let (w, h) = dec.dimensions().map_err(|e| format!("TIFF: {e}"))?;
    let color = dec.colortype().map_err(|e| format!("TIFF: {e}"))?;
    let layout = match color {
        ColorType::Gray(_) => Layout::Gray,
        ColorType::GrayA(_) => Layout::GrayAlpha,
        ColorType::RGB(_) => Layout::Rgb,
        ColorType::RGBA(_) => Layout::Rgba,
        ColorType::CMYK(_) => Layout::Cmyk,
        other => return Err(format!("TIFF colour type {other:?} is not supported yet")),
    };
    let samples = match dec.read_image().map_err(|e| format!("TIFF: {e}"))? {
        DecodingResult::U8(v) => v,
        DecodingResult::U16(v) => imaging::u16_to_u8(&v),
        DecodingResult::F32(v) => imaging::f32_to_u8(&v),
        _ => return Err("TIFF sample format is not supported yet".into()),
    };
    imaging::to_rgb8(&samples, layout, w, h)
}

fn encode_jpeg(img: &Rgb8) -> Result<Vec<u8>, String> {
    let (Ok(w), Ok(h)) = (u16::try_from(img.width), u16::try_from(img.height)) else {
        return Err(format!(
            "image {}×{} is too large for JPEG",
            img.width, img.height
        ));
    };
    let mut out = Vec::with_capacity(img.data.len() / 8);
    jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY)
        .encode(&img.data, w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| format!("JPEG: {e}"))?;
    Ok(out)
}
