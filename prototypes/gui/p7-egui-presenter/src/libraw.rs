//! LibRaw through the prototype shim (`csrc/proto.cpp`, a copy of the phase 1 shim): open + unpack,
//! then PPG renders of the half-size image or a full-size sensor region (ADR 0006 settings,
//! `user_flip = 0`, ADR 0008 §11: PPG).

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use crate::core::Backend;
use crate::view::{ImageSize, Rect};

/// LibRaw `user_qual` for PPG.
const PPG: c_int = 2;

#[repr(C)]
struct PpInfo {
    open_ms: f64,
    unpack_ms: f64,
    width: c_int,
    height: c_int,
    flip: c_int,
    error: [c_char; 200],
}

#[repr(C)]
struct PpRenderInfo {
    process_ms: f64,
    mkimg_ms: f64,
    width: c_int,
    height: c_int,
    error: [c_char; 200],
}

unsafe extern "C" {
    fn pp_open(path: *const u16, info: *mut PpInfo) -> *mut c_void;
    fn pp_render(s: *mut c_void, quality: c_int, half: c_int, x: c_int, y: c_int, w: c_int, h: c_int, ri: *mut PpRenderInfo) -> *mut c_void;
    fn pp_image(img: *const c_void, data: *mut *const u8, size: *mut c_uint);
    fn pp_free(img: *mut c_void);
    fn pp_close(s: *mut c_void);
}

fn c_str(e: &[c_char]) -> String {
    let b: Vec<u8> = e.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&b).into_owned()
}

/// An opened and unpacked RAW.
pub struct Session(*mut c_void);

// SAFETY: a LibRaw instance may be used from any thread as long as one thread uses it at a time; the
// core hands each session to exactly one worker at a time (taken out of the map while rendering).
unsafe impl Send for Session {}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: opened by pp_open, closed exactly once.
        unsafe { pp_close(self.0) };
    }
}

pub struct LibRaw;

impl Backend for LibRaw {
    type S = Session;

    fn open(&self, path: &Path) -> Result<(Session, ImageSize), String> {
        let w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: zeroed POD out-struct and a NUL-terminated wide path.
        let mut info: PpInfo = unsafe { std::mem::zeroed() };
        let s = unsafe { pp_open(w.as_ptr(), &mut info) };
        if s.is_null() {
            return Err(format!("{}: {}", path.display(), c_str(&info.error)));
        }
        let size = ImageSize { w: info.width.max(0) as u32, h: info.height.max(0) as u32, flip: info.flip };
        Ok((Session(s), size))
    }

    fn render(&self, s: &mut Session, half: bool, region: Option<Rect>) -> Result<(Vec<u8>, usize, usize), String> {
        let r = region.unwrap_or([0; 4]);
        let c = |v: u32| c_int::try_from(v).unwrap_or(c_int::MAX);
        // SAFETY: zeroed POD out-struct; the session is open and used by this thread only.
        let mut ri: PpRenderInfo = unsafe { std::mem::zeroed() };
        let img = unsafe { pp_render(s.0, PPG, c_int::from(half), c(r[0]), c(r[1]), c(r[2]), c(r[3]), &mut ri) };
        if img.is_null() {
            return Err(format!("render: {}", c_str(&ri.error)));
        }
        let (mut data, mut size) = (std::ptr::null(), 0u32);
        // SAFETY: img is a valid processed image until pp_free; data/size describe its pixels.
        let rgb = unsafe {
            pp_image(img, &mut data, &mut size);
            let v = if data.is_null() { Vec::new() } else { std::slice::from_raw_parts(data, size as usize).to_vec() };
            pp_free(img);
            v
        };
        let (w, h) = (ri.width.max(0) as usize, ri.height.max(0) as usize);
        if rgb.len() != w * h * 3 {
            return Err(format!("unexpected image size {w}x{h} ({} bytes)", rgb.len()));
        }
        Ok((rgb, w, h))
    }
}
