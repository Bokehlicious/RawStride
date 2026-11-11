//! The decoder seen by the core. The real one is LibRaw with PPG ([`crate::libraw`], feature
//! `libraw`); the tests use a simulated one, so the core is tested without native libraries.

use std::fmt;
use std::path::Path;

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
}

impl DecodeError {
    pub fn new(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: false,
        }
    }

    pub fn damaged(message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            damaged: true,
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
