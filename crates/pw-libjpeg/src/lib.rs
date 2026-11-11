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
//! - [`Decoder::icc_profile`] gives the ICC profile in the JPEG's APP2 segments, if any (ADR 0016
//!   §4.1). Nothing calls it yet (that is ADR 0016 C4).
//! - Every call returns a [`Result`]; nothing panics.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt;
use std::ptr;

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
/// `TJPARAM_SAVEMARKERS` is entry 25 of `enum TJPARAM` in the `turbojpeg.h` of libjpeg-turbo 3.2.0
/// (counted from `TJPARAM_STOPONWARNING` = 0).
const TJPARAM_SAVEMARKERS: c_int = 25;
/// `TJPARAM_SAVEMARKERS` value 2: keep all extra markers, so `tj3DecompressHeader` extracts the
/// ICC profile (APP2) for `tj3GetICCProfile`.
const SAVEMARKERS_ALL: c_int = 2;

unsafe extern "C" {
    fn tj3Init(init_type: c_int) -> Handle;
    fn tj3Destroy(handle: Handle);
    fn tj3Set(handle: Handle, param: c_int, value: c_int) -> c_int;
    fn tj3GetICCProfile(handle: Handle, icc_buf: *mut *mut u8, icc_size: *mut usize) -> c_int;
    fn tj3Free(buffer: *mut c_void);
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

/// A buffer TurboJPEG allocated; freed with `tj3Free` when dropped, so every path frees it.
struct TjBuffer(*mut u8);

impl Drop for TjBuffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from tj3GetICCProfile, which allocates it for the caller to
            // free with tj3Free; it is freed exactly once, here.
            unsafe { tj3Free(self.0.cast::<c_void>()) };
        }
    }
}

/// The last error of `handle` (a valid TurboJPEG instance).
fn error_of(handle: Handle) -> Error {
    // SAFETY: the handle is valid; TurboJPEG returns a pointer to a NUL-terminated string it
    // owns (never null for a valid handle; checked anyway).
    let p = unsafe { tj3GetErrorStr(handle) };
    if p.is_null() {
        return Error("unknown error".into());
    }
    // SAFETY: p is a valid NUL-terminated string (see above).
    Error(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// A new decompression instance that keeps ICC markers when it reads a header.
fn new_handle() -> Result<Handle> {
    // SAFETY: plain constructor; null means failure.
    let handle = unsafe { tj3Init(TJINIT_DECOMPRESS) };
    if handle.is_null() {
        return Err(Error("cannot create a TurboJPEG instance".into()));
    }
    // SAFETY: valid handle; TJPARAM_SAVEMARKERS accepts the values 0 to 4.
    if unsafe { tj3Set(handle, TJPARAM_SAVEMARKERS, SAVEMARKERS_ALL) } != 0 {
        let error = error_of(handle);
        // SAFETY: the handle came from tj3Init above, is not stored anywhere and is destroyed
        // exactly once, here.
        unsafe { tj3Destroy(handle) };
        return Err(error);
    }
    Ok(handle)
}

impl Decoder {
    pub fn new() -> Result<Decoder> {
        Ok(Decoder {
            handle: new_handle()?,
        })
    }

    fn error(&self) -> Error {
        error_of(self.handle)
    }

    /// libjpeg-turbo 3.2.0 replaces an extracted ICC profile only when the next header has one;
    /// it never clears it (`tj3DecompressHeader` in `turbojpeg.c`). So a JPEG without a profile
    /// would report the profile of an earlier JPEG read by this instance (through `icc_profile`
    /// or `decode`). When the instance holds a profile, it is replaced by a fresh one.
    fn forget_extracted_icc(&mut self) -> Result<()> {
        let mut size: usize = 0;
        // SAFETY: valid handle; a null buffer pointer asks only for the size, so TurboJPEG
        // allocates nothing. Without a profile it returns -1 (a warning) and size 0, which is
        // what this check wants to see.
        unsafe { tj3GetICCProfile(self.handle, ptr::null_mut(), &mut size) };
        if size == 0 {
            return Ok(());
        }
        let fresh = new_handle()?;
        // SAFETY: the old handle came from tj3Init, is replaced right after and is destroyed
        // exactly once, here.
        unsafe { tj3Destroy(self.handle) };
        self.handle = fresh;
        Ok(())
    }

    /// The ICC profile stored in the APP2 segments of `data` (joined when it is split over
    /// several), or `None` when there is none. Reads only the header; `data` is not decoded.
    pub fn icc_profile(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>> {
        self.forget_extracted_icc()?;
        self.header(data)?;
        let mut buf: *mut u8 = ptr::null_mut();
        let mut size: usize = 0;
        // SAFETY: valid handle, after a successful header read; buf and size are valid places
        // for TurboJPEG to write the buffer pointer and its length.
        let rc = unsafe { tj3GetICCProfile(self.handle, &mut buf, &mut size) };
        let owned = TjBuffer(buf);
        if rc != 0 && !self.last_was_warning() {
            return Err(self.error());
        }
        if owned.0.is_null() || size == 0 {
            return Ok(None);
        }
        // SAFETY: TurboJPEG allocated `size` bytes at owned.0 and filled them with the profile;
        // the buffer stays alive until `owned` is dropped after the copy.
        let bytes = unsafe { std::slice::from_raw_parts(owned.0, size) }.to_vec();
        Ok(Some(bytes))
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

    // Inside the inline `tests` module the path is relative to `src/tests/`, hence three `..`.
    #[path = "../../../pw-thumbcache/src/jpeg_fixture.rs"]
    mod jpeg_fixture;

    /// 100 test bytes for the ICC payload (not a real profile; TurboJPEG does not parse it).
    fn icc_test_bytes() -> Vec<u8> {
        (0..100u32).map(|i| (i * 7 + 3) as u8).collect()
    }

    /// The fixture JPEG with one APP2 `ICC_PROFILE` segment per part inserted after SOI; part `k`
    /// (from 1) of `n` carries sequence number `k` and count `n`.
    fn with_icc(parts: &[&[u8]]) -> Vec<u8> {
        let jpeg = jpeg_fixture::SMALL;
        let mut out = jpeg[..2].to_vec();
        let count = parts.len() as u8;
        for (k, part) in parts.iter().enumerate() {
            let len = (2 + 12 + 2 + part.len()) as u16;
            out.extend_from_slice(&[0xFF, 0xE2]);
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(b"ICC_PROFILE\0");
            out.extend_from_slice(&[k as u8 + 1, count]);
            out.extend_from_slice(part);
        }
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    #[test]
    fn scaled_sizes_round_up_like_tjscaled() {
        assert_eq!(Scale::Full.size(8640, 5760), (8640, 5760));
        assert_eq!(Scale::Half.size(1617, 1081), (809, 541));
        assert_eq!(Scale::Quarter.size(5, 3), (2, 1));
        assert_eq!(Scale::Eighth.size(1, 1), (1, 1));
    }

    #[test]
    fn jpeg_without_icc_has_no_profile() {
        let mut d = Decoder::new().unwrap();
        assert_eq!(d.icc_profile(jpeg_fixture::SMALL).unwrap(), None);
    }

    #[test]
    fn icc_profile_in_one_app2_segment() {
        let icc = icc_test_bytes();
        let mut d = Decoder::new().unwrap();
        assert_eq!(d.icc_profile(&with_icc(&[&icc])).unwrap(), Some(icc));
    }

    #[test]
    fn icc_profile_split_over_two_app2_segments_is_joined() {
        let icc = icc_test_bytes();
        let (first, second) = icc.split_at(60);
        let mut d = Decoder::new().unwrap();
        assert_eq!(
            d.icc_profile(&with_icc(&[first, second])).unwrap(),
            Some(icc)
        );
    }

    #[test]
    fn decode_with_app2_gives_the_same_pixels() {
        let icc = icc_test_bytes();
        let mut d = Decoder::new().unwrap();
        let plain = d.decode(jpeg_fixture::SMALL, Scale::Full).unwrap();
        let tagged = d.decode(&with_icc(&[&icc]), Scale::Full).unwrap();
        assert_eq!((plain.width, plain.height), (16, 8));
        assert_eq!(tagged, plain);
    }

    #[test]
    fn icc_profile_of_garbage_is_an_error() {
        let mut d = Decoder::new().unwrap();
        assert!(d.icc_profile(b"not a jpeg at all").is_err());
        assert!(d.icc_profile(&[]).is_err());
    }

    #[test]
    fn a_reused_decoder_does_not_report_an_earlier_profile() {
        let icc = icc_test_bytes();
        let tagged = with_icc(&[&icc]);
        let mut d = Decoder::new().unwrap();
        // after icc_profile of a JPEG with a profile
        assert_eq!(d.icc_profile(&tagged).unwrap(), Some(icc.clone()));
        assert_eq!(d.icc_profile(jpeg_fixture::SMALL).unwrap(), None);
        // after decode of a JPEG with a profile (its header read extracts the profile too)
        d.decode(&tagged, Scale::Full).unwrap();
        assert_eq!(d.icc_profile(jpeg_fixture::SMALL).unwrap(), None);
        // and the decoder still works afterwards
        assert_eq!(d.icc_profile(&tagged).unwrap(), Some(icc));
        assert_eq!(
            d.decode(jpeg_fixture::SMALL, Scale::Full).unwrap().width,
            16
        );
    }
}
