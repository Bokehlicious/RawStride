//! TIFF decoding to 8-bit RGB, shared by RawStride and the Tauri app (ADR 0015 §4.1).
//!
//! The decoder and the sample conversions moved here from `src-tauri` (`tiff_view.rs` and
//! `imaging.rs`) unchanged in behaviour and messages (ADR 0015 D1). Additions:
//! - an optional memory limit for one decode (`decode_rgb8(path, Some(bytes))`, ADR 0015 Q4);
//!   `None` keeps the Tauri app's unlimited decoding;
//! - one decode at a time in the whole process (ADR 0015 §4.4): `decode_rgb8` holds a private
//!   lock for the whole decode; `dimensions` does not.
//!
//! Orientation and colour management are not applied; CMYK is converted naively.

mod convert;

use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, Limits};

pub use convert::{Layout, Rgb8, f32_to_u8, to_rgb8, u16_to_u8};

/// Why a TIFF could not be read. Each `String` is the complete message that `Display` shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TiffError {
    /// The file could not be opened ("{path}: {io error}").
    Io(String),
    /// The `tiff` crate refused the file ("TIFF: {error}") or the image data is short.
    Decode(String),
    /// A colour type, sample format or TIFF feature that is not supported.
    Unsupported(String),
    /// The decode needs more memory than `limit` bytes.
    TooLarge { limit: usize },
}

impl fmt::Display for TiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TiffError::Io(m) | TiffError::Decode(m) | TiffError::Unsupported(m) => f.write_str(m),
            TiffError::TooLarge { limit } => write!(
                f,
                "TIFF: the file needs more memory than the limit of {} MB",
                limit / (1024 * 1024)
            ),
        }
    }
}

impl std::error::Error for TiffError {}

/// Maps an error of the `tiff` crate; `LimitsExceeded` is `TooLarge` only when a limit was set.
fn tiff_error(e: tiff::TiffError, limit: Option<usize>) -> TiffError {
    match (e, limit) {
        (tiff::TiffError::LimitsExceeded, Some(limit)) => TiffError::TooLarge { limit },
        (e @ tiff::TiffError::UnsupportedError(_), _) => {
            TiffError::Unsupported(format!("TIFF: {e}"))
        }
        (e, _) => TiffError::Decode(format!("TIFF: {e}")),
    }
}

/// One decode at a time in the whole process (ADR 0015 §4.4).
static DECODE: Mutex<()> = Mutex::new(());

fn decode_lock() -> MutexGuard<'static, ()> {
    DECODE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Holds the decode lock, so tests can check that `decode_rgb8` waits for it.
#[cfg(test)]
fn hold_decode_lock() -> MutexGuard<'static, ()> {
    decode_lock()
}

fn open(path: &Path, limit: Option<usize>) -> Result<Decoder<BufReader<File>>, TiffError> {
    let file = File::open(path).map_err(|e| TiffError::Io(format!("{}: {e}", path.display())))?;
    let dec =
        Decoder::new(BufReader::with_capacity(1 << 20, file)).map_err(|e| tiff_error(e, limit))?;
    let limits = match limit {
        // Default limits (256 MB) reject large 16-bit files from high-resolution cameras.
        None => Limits::unlimited(),
        Some(bytes) => {
            let mut limits = Limits::default();
            limits.decoding_buffer_size = bytes;
            limits.intermediate_buffer_size = bytes;
            limits
        }
    };
    Ok(dec.with_limits(limits))
}

/// Image size from the TIFF decoder (used when pw-raw cannot inspect the file, e.g. BigTIFF).
pub fn dimensions(path: &Path) -> Result<(u32, u32), TiffError> {
    open(path, None)?
        .dimensions()
        .map_err(|e| tiff_error(e, None))
}

/// Decodes the first image of a TIFF to 8-bit RGB. `limit` is the most memory in bytes one
/// decode buffer may use (`None` = no limit). Only one decode runs at a time.
pub fn decode_rgb8(path: &Path, limit: Option<usize>) -> Result<Rgb8, TiffError> {
    let _decoding = decode_lock();
    let mut dec = open(path, limit)?;
    let (w, h) = dec.dimensions().map_err(|e| tiff_error(e, limit))?;
    let color = dec.colortype().map_err(|e| tiff_error(e, limit))?;
    let layout = match color {
        ColorType::Gray(_) => Layout::Gray,
        ColorType::GrayA(_) => Layout::GrayAlpha,
        ColorType::RGB(_) => Layout::Rgb,
        ColorType::RGBA(_) => Layout::Rgba,
        ColorType::CMYK(_) => Layout::Cmyk,
        other => {
            return Err(TiffError::Unsupported(format!(
                "TIFF colour type {other:?} is not supported yet"
            )));
        }
    };
    let samples = match dec.read_image().map_err(|e| tiff_error(e, limit))? {
        DecodingResult::U8(v) => v,
        DecodingResult::U16(v) => u16_to_u8(&v),
        DecodingResult::F32(v) => f32_to_u8(&v),
        _ => {
            return Err(TiffError::Unsupported(
                "TIFF sample format is not supported yet".into(),
            ));
        }
    };
    to_rgb8(&samples, layout, w, h).map_err(TiffError::Decode)
}

#[cfg(test)]
mod tests;
