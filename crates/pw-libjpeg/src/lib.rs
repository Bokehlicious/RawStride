//! A small, safe wrapper over libjpeg-turbo's TurboJPEG 3 API (ADR 0009 §6.2, option B: our own
//! wrapper, no new crates). libjpeg-turbo 3.2 comes from the repository's vcpkg install and is
//! linked statically (`build.rs`).
//!
//! - [`Decoder`] owns one TurboJPEG instance. It can be moved to another thread, but is used by
//!   one thread at a time (it is `Send`, not `Sync`).
//! - [`Decoder::decode`] gives 8-bit RGB at full size or scaled by 1/2, 1/4 or 1/8 (TurboJPEG
//!   decodes scaled directly, which is faster than decoding and reducing).
//! - The pixels are the JPEG's own grid. EXIF orientation is **not** applied (ADR 0003 point 4:
//!   the viewer rotates by the RAW file's tag).
//! - Every call returns a [`Result`]; nothing panics.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt;

/// Errors from TurboJPEG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "libjpeg-turbo: {}", self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

type Handle = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct TjScalingFactor {
    num: c_int,
    denom: c_int,
}

// turbojpeg.h (libjpeg-turbo 3.2): enum TJINIT, TJPARAM, TJPF and TJERR values
const TJINIT_DECOMPRESS: c_int = 1;
const TJPARAM_JPEGWIDTH: c_int = 5;
const TJPARAM_JPEGHEIGHT: c_int = 6;
const TJPF_RGB: c_int = 0;
const TJERR_WARNING: c_int = 0;

unsafe extern "C" {
    fn tj3Init(init_type: c_int) -> Handle;
    fn tj3Destroy(handle: Handle);
    fn tj3GetErrorStr(handle: Handle) -> *mut c_char;
    fn tj3GetErrorCode(handle: Handle) -> c_int;
    fn tj3Get(handle: Handle, param: c_int) -> c_int;
    fn tj3DecompressHeader(handle: Handle, jpeg_buf: *const u8, jpeg_size: usize) -> c_int;
    fn tj3SetScalingFactor(handle: Handle, factor: TjScalingFactor) -> c_int;
    fn tj3Decompress8(
        handle: Handle,
        jpeg_buf: *const u8,
        jpeg_size: usize,
        dst_buf: *mut u8,
        pitch: c_int,
        pixel_format: c_int,
    ) -> c_int;
}

/// A decoding scale. TurboJPEG decodes these directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scale {
    Full,
    Half,
    Quarter,
    Eighth,
}

impl Scale {
    fn factor(self) -> TjScalingFactor {
        let denom = match self {
            Scale::Full => 1,
            Scale::Half => 2,
            Scale::Quarter => 4,
            Scale::Eighth => 8,
        };
        TjScalingFactor { num: 1, denom }
    }

    /// The decoded size of a `w × h` JPEG at this scale (TurboJPEG's `TJSCALED`: rounded up).
    pub fn size(self, w: u32, h: u32) -> (u32, u32) {
        let d = self.factor().denom as u32;
        (w.div_ceil(d), h.div_ceil(d))
    }
}

/// An 8-bit RGB image (3 bytes per pixel, rows top to bottom).
#[derive(Clone, PartialEq, Eq)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl fmt::Debug for Rgb8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgb8({}x{})", self.width, self.height)
    }
}

/// One TurboJPEG decompression instance.
pub struct Decoder {
    handle: Handle,
}

// SAFETY: a TurboJPEG instance keeps all its state in the handle; the library is thread-safe as
// long as one handle is used by one thread at a time, which `&mut self` on every call ensures
// (the type is Send, not Sync).
unsafe impl Send for Decoder {}

impl fmt::Debug for Decoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Decoder")
    }
}

impl Decoder {
    pub fn new() -> Result<Decoder> {
        // SAFETY: plain constructor; null means failure.
        let handle = unsafe { tj3Init(TJINIT_DECOMPRESS) };
        if handle.is_null() {
            Err(Error("cannot create a TurboJPEG instance".into()))
        } else {
            Ok(Decoder { handle })
        }
    }

    fn error(&self) -> Error {
        // SAFETY: the handle is valid; TurboJPEG returns a pointer to a NUL-terminated string it
        // owns (never null for a valid handle; checked anyway).
        let p = unsafe { tj3GetErrorStr(self.handle) };
        if p.is_null() {
            return Error("unknown error".into());
        }
        // SAFETY: p is a valid NUL-terminated string (see above).
        Error(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }

    /// Whether the last error was only a warning (the image was decoded, possibly with damage).
    fn last_was_warning(&self) -> bool {
        // SAFETY: valid handle.
        unsafe { tj3GetErrorCode(self.handle) == TJERR_WARNING }
    }

    /// Reads the header of `data`: the image's width and height.
    pub fn header(&mut self, data: &[u8]) -> Result<(u32, u32)> {
        if data.is_empty() {
            return Err(Error("empty JPEG".into()));
        }
        // SAFETY: data is valid for data.len() bytes; the handle is valid.
        let rc = unsafe { tj3DecompressHeader(self.handle, data.as_ptr(), data.len()) };
        if rc != 0 {
            return Err(self.error());
        }
        // SAFETY: valid handle, after a successful header read.
        let (w, h) = unsafe {
            (
                tj3Get(self.handle, TJPARAM_JPEGWIDTH),
                tj3Get(self.handle, TJPARAM_JPEGHEIGHT),
            )
        };
        match (u32::try_from(w), u32::try_from(h)) {
            (Ok(w), Ok(h)) if w > 0 && h > 0 => Ok((w, h)),
            _ => Err(Error(format!("invalid JPEG size {w}x{h}"))),
        }
    }

    /// Decodes `data` to 8-bit RGB at `scale`. A non-fatal warning (for example a JPEG that ends
    /// early) still gives the image, as browsers show it; a fatal error is returned.
    pub fn decode(&mut self, data: &[u8], scale: Scale) -> Result<Rgb8> {
        let (w, h) = self.header(data)?;
        // SAFETY: valid handle; the factor is a plain struct passed by value.
        if unsafe { tj3SetScalingFactor(self.handle, scale.factor()) } != 0 {
            return Err(self.error());
        }
        let (sw, sh) = scale.size(w, h);
        let pitch = (sw as usize)
            .checked_mul(3)
            .ok_or_else(|| Error("JPEG too large".into()))?;
        let len = pitch
            .checked_mul(sh as usize)
            .ok_or_else(|| Error("JPEG too large".into()))?;
        let pitch_c = c_int::try_from(pitch).map_err(|_| Error("JPEG too large".into()))?;
        let mut out = vec![0u8; len];
        // SAFETY: data is valid for data.len() bytes; out holds exactly sh rows of `pitch` bytes,
        // which is what TurboJPEG writes for an RGB image of the scaled size (TJSCALED).
        let rc = unsafe {
            tj3Decompress8(
                self.handle,
                data.as_ptr(),
                data.len(),
                out.as_mut_ptr(),
                pitch_c,
                TJPF_RGB,
            )
        };
        if rc != 0 && !self.last_was_warning() {
            return Err(self.error());
        }
        Ok(Rgb8 {
            width: sw,
            height: sh,
            data: out,
        })
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the handle came from tj3Init and is destroyed exactly once.
        unsafe { tj3Destroy(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_sizes_round_up_like_tjscaled() {
        assert_eq!(Scale::Full.size(8640, 5760), (8640, 5760));
        assert_eq!(Scale::Half.size(1617, 1081), (809, 541));
        assert_eq!(Scale::Quarter.size(5, 3), (2, 1));
        assert_eq!(Scale::Eighth.size(1, 1), (1, 1));
    }
}
