//! TIFF decoding to 8-bit RGB for RawStride (ADR 0015 §4.1).
//!
//! The decoder and the sample conversions moved here from `src-tauri` (`tiff_view.rs` and
//! `imaging.rs`) unchanged in behaviour and messages (ADR 0015 D1). Additions:
//! - an optional memory limit for one decode (`decode_rgb8(path, Some(bytes))`, ADR 0015 Q4);
//!   `None` decodes without a limit;
//! - one decode at a time in the whole process (ADR 0015 §4.4): `decode_rgb8` holds a private
//!   lock for the whole decode; `dimensions` does not.
//!
//! The target size `fit` and the area-averaging `downscale` moved here from `src-tauri`
//! (`imaging.rs`) in ADR 0015 D3, for the TIFF thumbnails of RawStride.
//!
//! `decode_rgb8_for_display` (ADR 0016 C3) decodes like `decode_rgb8` and converts the pixels from
//! the file's embedded ICC profile (tag 34675) into a destination profile; 16-bit RGB is converted
//! before the reduction to 8 bit (ADR 0016 §3.1, §4.1). Nothing calls it yet (ADR 0016 C4).
//!
//! Orientation is not applied; `decode_rgb8` does no colour management; CMYK is converted naively.

mod convert;
mod resize;

use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use pw_colour::{Profile, Transform, TransformCache};
use tiff::ColorType;
use tiff::decoder::ifd::Value;
use tiff::decoder::{Decoder, DecodingResult, Limits};
use tiff::tags::Tag;

use convert::to_rgb16;
pub use convert::{Layout, Rgb8, f32_to_u8, to_rgb8, u16_to_u8};
pub use resize::{downscale, fit};

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

/// The TIFF tag that holds an embedded ICC profile (InterColorProfile).
const ICC_PROFILE_TAG: u16 = 34675;

/// Which source profile `decode_rgb8_for_display` used for the pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceColour {
    /// The file's embedded ICC profile.
    Embedded,
    /// No embedded profile, or a layout whose profile is not applied (Gray, GrayAlpha, CMYK);
    /// the pixels are treated as sRGB.
    AssumedSrgb,
    /// The file has a profile that could not be used (the detail, e.g. the `ColourError` text);
    /// the pixels are treated as sRGB.
    Unusable(String),
}

/// Decodes the first image of a TIFF like [`decode_rgb8`] and converts it from its source profile
/// to `destination` (ADR 0016 §4.1). The source profile is the embedded ICC profile for RGB and
/// RGBA, else sRGB (see [`SourceColour`]). 16-bit RGB and RGBA are converted before the
/// reduction to 8 bit. Decode errors are the same as those of `decode_rgb8`; a profile or
/// conversion problem is never an error, only `Unusable`. Only one decode runs at a time.
pub fn decode_rgb8_for_display(
    path: &Path,
    limit: Option<usize>,
    destination: &Profile,
    cache: &TransformCache,
) -> Result<(Rgb8, SourceColour), TiffError> {
    let _decoding = decode_lock();
    let mut dec = open(path, limit)?;
    let (w, h) = dec.dimensions().map_err(|e| tiff_error(e, limit))?;
    let color = dec.colortype().map_err(|e| tiff_error(e, limit))?;
    let layout = layout_of(color)?;

    let (source, mut colour) = source_profile(&mut dec, layout);
    let transform = match cache.get(&source, destination) {
        Ok(transform) => Some(transform),
        Err(e) => {
            colour = SourceColour::Unusable(e.to_string());
            cache.get(&Profile::srgb(), destination).ok()
        }
    };

    let result = dec.read_image().map_err(|e| tiff_error(e, limit))?;
    let rgb_16_bit = matches!(layout, Layout::Rgb | Layout::Rgba);
    let (image, converted) = match result {
        DecodingResult::U16(v) if rgb_16_bit => {
            let mut data = to_rgb16(v, layout, w, h).map_err(TiffError::Decode)?;
            let converted = convert(transform.as_deref(), |t| t.convert_rgb16(&mut data));
            let image = Rgb8 {
                width: w,
                height: h,
                data: u16_to_u8(&data),
            };
            (image, converted)
        }
        result => {
            let samples = match result {
                DecodingResult::U8(v) => v,
                DecodingResult::U16(v) => u16_to_u8(&v),
                DecodingResult::F32(v) => f32_to_u8(&v),
                _ => {
                    return Err(TiffError::Unsupported(
                        "TIFF sample format is not supported yet".into(),
                    ));
                }
            };
            let mut image = to_rgb8(&samples, layout, w, h).map_err(TiffError::Decode)?;
            let converted = convert(transform.as_deref(), |t| t.convert_rgb8(&mut image.data));
            (image, converted)
        }
    };
    if let Err(detail) = converted {
        colour = SourceColour::Unusable(detail);
    }
    Ok((image, colour))
}

/// The pixel layout of a TIFF colour type, as in `decode_rgb8`.
fn layout_of(color: ColorType) -> Result<Layout, TiffError> {
    match color {
        ColorType::Gray(_) => Ok(Layout::Gray),
        ColorType::GrayA(_) => Ok(Layout::GrayAlpha),
        ColorType::RGB(_) => Ok(Layout::Rgb),
        ColorType::RGBA(_) => Ok(Layout::Rgba),
        ColorType::CMYK(_) => Ok(Layout::Cmyk),
        other => Err(TiffError::Unsupported(format!(
            "TIFF colour type {other:?} is not supported yet"
        ))),
    }
}

/// The source profile of the pixels and how it was chosen. The embedded profile is applied to
/// RGB and RGBA only: a gray or CMYK profile does not describe the RGB that `to_rgb8` makes
/// (ADR 0016 §3.2).
fn source_profile<R: std::io::Read + std::io::Seek>(
    dec: &mut Decoder<R>,
    layout: Layout,
) -> (Profile, SourceColour) {
    if !matches!(layout, Layout::Rgb | Layout::Rgba) {
        return (Profile::srgb(), SourceColour::AssumedSrgb);
    }
    let bytes = dec
        .find_tag(Tag::Unknown(ICC_PROFILE_TAG))
        .and_then(|value| value.map(icc_bytes).transpose());
    match bytes {
        Ok(None) => (Profile::srgb(), SourceColour::AssumedSrgb),
        Ok(Some(bytes)) => match Profile::from_icc(&bytes) {
            Ok(profile) => (profile, SourceColour::Embedded),
            Err(e) => (Profile::srgb(), SourceColour::Unusable(e.to_string())),
        },
        Err(e) => (
            Profile::srgb(),
            SourceColour::Unusable(format!("TIFF ICC profile tag: {e}")),
        ),
    }
}

/// The bytes of the ICC profile tag. The `tiff` crate reads type UNDEFINED (as Lightroom writes
/// it) as bytes (`into_u8_vec`), but type BYTE longer than 4 bytes as `UnsignedBig` values, so
/// those are taken one by one.
fn icc_bytes(value: Value) -> tiff::TiffResult<Vec<u8>> {
    match value {
        Value::List(items) if matches!(items.first(), Some(Value::UnsignedBig(_))) => items
            .into_iter()
            .map(|v| v.into_u64().and_then(|n| Ok(u8::try_from(n)?)))
            .collect(),
        value => value.into_u8_vec(),
    }
}

/// Runs a conversion if there is a transform; an error becomes its text. Without a transform
/// (no transform could be built at all) the data stays as it is.
fn convert(
    transform: Option<&Transform>,
    run: impl FnOnce(&Transform) -> Result<(), pw_colour::ColourError>,
) -> Result<(), String> {
    match transform {
        Some(transform) => run(transform).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
