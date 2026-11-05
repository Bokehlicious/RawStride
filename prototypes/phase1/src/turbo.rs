//! Minimal FFI to the TurboJPEG 2.x API (still provided by libjpeg-turbo 3.x), as in
//! tools/preview-bench. Linked statically from the vcpkg manifest install (build.rs).
use std::ffi::{CStr, c_char, c_int, c_ulong, c_void};

type Handle = *mut c_void;
const TJPF_RGB: c_int = 0;
const TJERR_WARNING: c_int = 0;

unsafe extern "C" {
    fn tjInitDecompress() -> Handle;
    fn tjDecompressHeader3(
        handle: Handle,
        jpeg_buf: *const u8,
        jpeg_size: c_ulong,
        width: *mut c_int,
        height: *mut c_int,
        subsamp: *mut c_int,
        colorspace: *mut c_int,
    ) -> c_int;
    fn tjDecompress2(
        handle: Handle,
        jpeg_buf: *const u8,
        jpeg_size: c_ulong,
        dst_buf: *mut u8,
        width: c_int,
        pitch: c_int,
        height: c_int,
        pixel_format: c_int,
        flags: c_int,
    ) -> c_int;
    fn tjGetErrorStr2(handle: Handle) -> *mut c_char;
    fn tjGetErrorCode(handle: Handle) -> c_int;
    fn tjDestroy(handle: Handle) -> c_int;
}

#[derive(Debug)]
pub struct Decompressor(Handle);

impl Decompressor {
    pub fn new() -> Result<Self, String> {
        // SAFETY: plain constructor; null means failure.
        let h = unsafe { tjInitDecompress() };
        if h.is_null() {
            Err("tjInitDecompress failed".into())
        } else {
            Ok(Self(h))
        }
    }

    fn error(&self) -> String {
        // SAFETY: the handle is valid and the library returns a NUL-terminated string.
        unsafe { CStr::from_ptr(tjGetErrorStr2(self.0)) }
            .to_string_lossy()
            .into_owned()
    }

    /// Decodes `data` to RGB at scale 1/`div`. Returns (width, height, pixels).
    pub fn decode(&mut self, data: &[u8], div: u32) -> Result<(u32, u32, Vec<u8>), String> {
        let size = c_ulong::try_from(data.len()).map_err(|_| "JPEG too large".to_string())?;
        let (mut w, mut h, mut ss, mut cs) = (0, 0, 0, 0);
        // SAFETY: data is valid for `size` bytes; out-pointers are valid locals.
        let rc = unsafe {
            tjDecompressHeader3(
                self.0,
                data.as_ptr(),
                size,
                &mut w,
                &mut h,
                &mut ss,
                &mut cs,
            )
        };
        if rc != 0 {
            return Err(format!("tjDecompressHeader3: {}", self.error()));
        }
        // TJSCALED(dim, 1/div) == ceil(dim / div); TurboJPEG then picks exactly 1/div.
        let sw = (w as u32).div_ceil(div);
        let sh = (h as u32).div_ceil(div);
        let mut out = vec![0u8; sw as usize * sh as usize * 3];
        // SAFETY: out holds sw*sh*3 bytes for an RGB image of the requested scaled size.
        let rc = unsafe {
            tjDecompress2(
                self.0,
                data.as_ptr(),
                size,
                out.as_mut_ptr(),
                sw as c_int,
                0,
                sh as c_int,
                TJPF_RGB,
                0,
            )
        };
        // SAFETY: the handle is valid.
        if rc != 0 && unsafe { tjGetErrorCode(self.0) } != TJERR_WARNING {
            return Err(format!("tjDecompress2: {}", self.error()));
        }
        Ok((sw, sh, out))
    }
}

impl Drop for Decompressor {
    fn drop(&mut self) {
        // SAFETY: the handle came from tjInitDecompress and is destroyed exactly once.
        unsafe { tjDestroy(self.0) };
    }
}
