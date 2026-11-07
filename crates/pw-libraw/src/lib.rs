//! LibRaw binding for PhotoWorkflow's RAW rendering pipeline (ADR 0006).
//!
//! - One [`Session`] = one opened and unpacked RAW file. Unpacking (~0.3 s for a 61 MP Sony) is
//!   done once; any number of regions can then be rendered from it.
//! - Every render uses the same fixed settings (camera white balance, no auto brightness, no
//!   white-level adjustment from the image data, sensor orientation, 8-bit sRGB). With them a
//!   region is **pixel-identical** to the same area of a whole-image render, so tiles cannot show
//!   seams or differ in colour (measured: docs/analyses/2026-11-04-m2-libraw-decode.md; checked
//!   again by `tests/regions.rs`).
//! - The demosaic method is chosen per render ([`Demosaic`]): **AHD** is the default and what
//!   [`Session::render_full`] and [`Session::render_region`] use (ADR 0006, the Tauri app);
//!   **PPG** is the decided method for the egui loupe (ADR 0008 §11, ADR 0009) and is used through
//!   [`Session::render_full_with`] and [`Session::render_region_with`]. The region identity holds
//!   for both (`tests/regions.rs`).
//! - LibRaw is built with OpenMP: one render uses all cores.
//! - Images come out in **sensor orientation**; the viewer rotates them like the M1 previews.

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::fmt;
use std::path::Path;

/// Errors from LibRaw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LibRaw: {}", self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[repr(C)]
struct Info {
    width: c_int,
    height: c_int,
    flip: c_int,
    warnings: c_uint,
    data_errors: c_int,
    short_reads: i64,
    missing: i64,
    file_size: i64,
    damaged: c_int,
    error: [c_char; 200],
}

/// What LibRaw reported while unpacking a file (see `csrc/pwlr.cpp`).
///
/// - `warnings` are LibRaw's `process_warnings` (`LIBRAW_WARN_*`: camera white balance, colour
///   profiles, fallbacks). They do not mean that image data is missing and never make a file
///   count as damaged.
/// - `data_errors` (corrupt data or an unexpected end reported by a decoder) and `missing`
///   (bytes of RAW data the file does not have) do: such a file is refused as damaged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub warnings: u32,
    pub data_errors: i32,
    pub short_reads: i64,
    pub missing: i64,
    pub file_size: i64,
    pub damaged: bool,
}

/// Prefix of the error for a file whose RAW data is missing or corrupt.
pub const DAMAGED: &str = "damaged:";

unsafe extern "C" {
    fn pwlr_version() -> *const c_char;
    fn pwlr_capabilities() -> c_uint;
    fn pwlr_open(path: *const u16, info: *mut Info) -> *mut c_void;
    fn pwlr_render(
        session: *mut c_void,
        quality: c_int,
        half: c_int,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        error: *mut c_char,
    ) -> *mut c_void;
    fn pwlr_image(
        img: *const c_void,
        w: *mut c_int,
        h: *mut c_int,
        data: *mut *const u8,
        size: *mut c_uint,
    );
    fn pwlr_free(img: *mut c_void);
    fn pwlr_close(session: *mut c_void);
}

/// LibRaw version string, e.g. "0.22.3-Release".
pub fn version() -> String {
    // SAFETY: returns a pointer to a static NUL-terminated string.
    unsafe { CStr::from_ptr(pwlr_version()) }
        .to_string_lossy()
        .into_owned()
}

/// LibRaw capability flags (`libraw_capabilities()`).
pub fn capabilities() -> u32 {
    // SAFETY: plain query without arguments.
    unsafe { pwlr_capabilities() }
}

/// The demosaic method of a full-resolution render (LibRaw `user_qual`). Half-size renders
/// bin 2×2 pixels and do not demosaic, so the method does not affect them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Demosaic {
    /// AHD: the default and the quality reference (ADR 0006, ADR 0008 §11).
    #[default]
    Ahd,
    /// PPG: the decided method for the viewer core (ADR 0008 §11), about twice as fast as AHD.
    Ppg,
}

impl Demosaic {
    /// LibRaw's `user_qual` value for the method.
    fn user_qual(self) -> c_int {
        match self {
            Demosaic::Ahd => 3,
            Demosaic::Ppg => 2,
        }
    }
}

/// A rectangle in sensor pixels of the visible image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }
}

/// An 8-bit RGB image (3 bytes per pixel, rows top to bottom), in sensor orientation.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl fmt::Debug for Rgb8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rgb8({}x{}, {} bytes)",
            self.width,
            self.height,
            self.data.len()
        )
    }
}

impl Rgb8 {
    /// Copies the region `r` (must lie inside the image).
    pub fn crop(&self, r: Rect) -> Rgb8 {
        assert!(
            r.x + r.w <= self.width && r.y + r.h <= self.height,
            "crop outside image"
        );
        let row = r.w as usize * 3;
        let mut data = Vec::with_capacity(row * r.h as usize);
        for y in r.y..r.y + r.h {
            let start = (y as usize * self.width as usize + r.x as usize) * 3;
            data.extend_from_slice(&self.data[start..start + row]);
        }
        Rgb8 {
            width: r.w,
            height: r.h,
            data,
        }
    }
}

fn wide(path: &Path) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }
}

fn c_text(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// One opened and unpacked RAW file. Not shared between threads (use one per worker); it can be
/// moved to another thread.
pub struct Session {
    ptr: *mut c_void,
    width: u32,
    height: u32,
    flip: u8,
    diagnostics: Diagnostics,
}

// SAFETY: LibRaw's thread-safe build (`raw_r`) keeps all state in the instance; a Session is
// used by one thread at a time (it is Send, not Sync).
unsafe impl Send for Session {}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("flip", &self.flip)
            .finish()
    }
}

impl Session {
    /// Opens and unpacks `path` (the expensive, single-threaded part: ~0.3 s for 61 MP).
    /// A file whose RAW data is missing (truncated) or corrupt is refused with an error that
    /// contains [`DAMAGED`].
    pub fn open(path: &Path) -> Result<Session> {
        Self::open_with_diagnostics(path).0
    }

    /// Like [`open`](Self::open), and also returns what LibRaw reported (also on failure).
    pub fn open_with_diagnostics(path: &Path) -> (Result<Session>, Diagnostics) {
        let w = wide(path);
        let mut info = Info {
            width: 0,
            height: 0,
            flip: 0,
            warnings: 0,
            data_errors: 0,
            short_reads: 0,
            missing: 0,
            file_size: 0,
            damaged: 0,
            error: [0; 200],
        };
        // SAFETY: NUL-terminated UTF-16 path and a valid out-pointer.
        let ptr = unsafe { pwlr_open(w.as_ptr(), &mut info) };
        let diagnostics = Diagnostics {
            warnings: info.warnings,
            data_errors: info.data_errors,
            short_reads: info.short_reads,
            missing: info.missing,
            file_size: info.file_size,
            damaged: info.damaged != 0,
        };
        if ptr.is_null() {
            let e = Error(format!("{}: {}", path.display(), c_text(&info.error)));
            return (Err(e), diagnostics);
        }
        let s = Session {
            ptr,
            width: info.width.max(0) as u32,
            height: info.height.max(0) as u32,
            flip: info.flip.clamp(0, 7) as u8,
            diagnostics,
        };
        (Ok(s), diagnostics)
    }

    /// What LibRaw reported while unpacking this file.
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }

    /// Visible image width in sensor orientation.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Visible image height in sensor orientation.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// LibRaw's flip code for the file (0 none, 3 180°, 5 90° CCW, 6 90° CW). The pipeline does
    /// not apply it; the viewer rotates.
    pub fn flip(&self) -> u8 {
        self.flip
    }

    fn render(&mut self, half: bool, r: Option<Rect>, demosaic: Demosaic) -> Result<Rgb8> {
        let r = r.unwrap_or(Rect::new(0, 0, 0, 0));
        let mut err = [0 as c_char; 200];
        // SAFETY: live session (exclusive &mut), valid error buffer.
        let img = unsafe {
            pwlr_render(
                self.ptr,
                demosaic.user_qual(),
                c_int::from(half),
                r.x as c_int,
                r.y as c_int,
                r.w as c_int,
                r.h as c_int,
                err.as_mut_ptr(),
            )
        };
        if img.is_null() {
            return Err(Error(c_text(&err)));
        }
        let (mut w, mut h, mut data, mut size) = (0, 0, std::ptr::null(), 0);
        // SAFETY: img is a valid LibRaw image until pwlr_free below.
        unsafe { pwlr_image(img, &mut w, &mut h, &mut data, &mut size) };
        let expected = w.max(0) as usize * h.max(0) as usize * 3;
        let out = if data.is_null() || (size as usize) < expected {
            Err(Error("image buffer is smaller than its size".into()))
        } else {
            // SAFETY: data points to `size` bytes owned by img, which is still alive.
            let bytes = unsafe { std::slice::from_raw_parts(data, expected) };
            Ok(Rgb8 {
                width: w as u32,
                height: h as u32,
                data: bytes.to_vec(),
            })
        };
        // SAFETY: img came from pwlr_render and is freed exactly once.
        unsafe { pwlr_free(img) };
        out
    }

    /// The whole image at full resolution, with AHD.
    pub fn render_full(&mut self) -> Result<Rgb8> {
        self.render_full_with(Demosaic::Ahd)
    }

    /// The whole image at full resolution, with the given demosaic method.
    pub fn render_full_with(&mut self, demosaic: Demosaic) -> Result<Rgb8> {
        self.render(false, None, demosaic)
    }

    /// The whole image at half resolution (2×2 binning, no demosaicing).
    pub fn render_half(&mut self) -> Result<Rgb8> {
        self.render(true, None, Demosaic::Ahd)
    }

    /// Exactly the region `r`, rendered with AHD and with `margin` extra pixels of context on
    /// every side (clamped at the image border) that are cut off again. With a margin of at least
    /// 8 px the result equals the same area of [`render_full`](Self::render_full).
    pub fn render_region(&mut self, r: Rect, margin: u32) -> Result<Rgb8> {
        self.render_region_with(r, margin, Demosaic::Ahd)
    }

    /// Like [`render_region`](Self::render_region), with the given demosaic method. The result
    /// equals the same area of [`render_full_with`](Self::render_full_with) with that method.
    pub fn render_region_with(&mut self, r: Rect, margin: u32, demosaic: Demosaic) -> Result<Rgb8> {
        if r.w == 0 || r.h == 0 || r.x + r.w > self.width || r.y + r.h > self.height {
            return Err(Error(format!(
                "region {r:?} is outside the image ({}x{})",
                self.width, self.height
            )));
        }
        let x0 = r.x.saturating_sub(margin);
        let y0 = r.y.saturating_sub(margin);
        let x1 = (r.x + r.w + margin).min(self.width);
        let y1 = (r.y + r.h + margin).min(self.height);
        let big = self.render(false, Some(Rect::new(x0, y0, x1 - x0, y1 - y0)), demosaic)?;
        if big.width != x1 - x0 || big.height != y1 - y0 {
            return Err(Error(format!(
                "LibRaw returned {}x{} for a {}x{} region",
                big.width,
                big.height,
                x1 - x0,
                y1 - y0
            )));
        }
        Ok(big.crop(Rect::new(r.x - x0, r.y - y0, r.w, r.h)))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: the pointer came from pwlr_open and is closed exactly once.
        unsafe { pwlr_close(self.ptr) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demosaic_maps_to_libraw_user_qual() {
        assert_eq!(Demosaic::default(), Demosaic::Ahd);
        assert_eq!(Demosaic::Ahd.user_qual(), 3);
        assert_eq!(Demosaic::Ppg.user_qual(), 2);
    }

    #[test]
    fn crop_copies_rows() {
        let img = Rgb8 {
            width: 3,
            height: 2,
            data: (0..18).collect(),
        };
        let c = img.crop(Rect::new(1, 1, 2, 1));
        assert_eq!((c.width, c.height), (2, 1));
        assert_eq!(c.data, vec![12, 13, 14, 15, 16, 17]);
    }
}
