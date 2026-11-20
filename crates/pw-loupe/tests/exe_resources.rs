//! `rawstride.exe` carries a Windows version resource (ADR 0031 RL2 / RL-Q11, T226), written by
//! `build.rs` and compiled with the Windows SDK's `rc.exe`. This test fails when `build.rs` could
//! not find or run `rc.exe` (it only warns then). The icon resource is checked by hand.

#![cfg(all(windows, target_env = "msvc"))]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};

/// `text` as a NUL-terminated UTF-16 string.
fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// The whole version resource of the file at `path`.
fn version_info(path: &Path) -> Vec<u8> {
    let name = wide(path.as_os_str());
    let mut handle = 0u32;
    // SAFETY: `name` is a NUL-terminated UTF-16 string that lives across the call, and `handle`
    // is a valid `u32` to write to.
    let size = unsafe { GetFileVersionInfoSizeW(name.as_ptr(), &mut handle) };
    assert!(size > 0, "{} has no version resource", path.display());
    let mut data = vec![0u8; size as usize];
    // SAFETY: `name` is as above, and `data` is a writable buffer of exactly `size` bytes.
    let ok = unsafe { GetFileVersionInfoW(name.as_ptr(), 0, size, data.as_mut_ptr().cast()) };
    assert_ne!(ok, 0, "GetFileVersionInfoW failed");
    data
}

/// The bytes `VerQueryValueW` gives for `sub_block` in `data`, or `None` when it is missing.
fn query<'a>(data: &'a [u8], sub_block: &str) -> Option<&'a [u8]> {
    let sub_block = wide(std::ffi::OsStr::new(sub_block));
    let mut buffer: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `data` holds a version resource read by `GetFileVersionInfoW`, `sub_block` is a
    // NUL-terminated UTF-16 string, and `buffer` and `len` are valid places to write to.
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr().cast(),
            sub_block.as_ptr(),
            &mut buffer,
            &mut len,
        )
    };
    if ok == 0 || buffer.is_null() {
        return None;
    }
    // The root block's `len` counts bytes; a string value's counts UTF-16 characters, its
    // terminating NUL included.
    let bytes = if sub_block == [u16::from(b'\\'), 0] {
        len as usize
    } else {
        len as usize * 2
    };
    // SAFETY: `buffer` points into `data` at `bytes` bytes (a `VS_FIXEDFILEINFO` or a string
    // value), which live as long as `data`.
    Some(unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), bytes) })
}

/// The string value `name` of the US English, Unicode table, read up to its first NUL.
fn string_value(data: &[u8], name: &str) -> String {
    let bytes = query(data, &format!("\\StringFileInfo\\040904b0\\{name}"))
        .unwrap_or_else(|| panic!("{name} is missing"));
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16(&units).unwrap()
}

#[test]
fn rawstride_exe_has_the_version_resource() {
    let data = version_info(Path::new(env!("CARGO_BIN_EXE_rawstride")));

    assert_eq!(
        string_value(&data, "ProductVersion"),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(string_value(&data, "ProductName"), "RawStride");
    assert_eq!(string_value(&data, "FileDescription"), "RawStride");
    assert_eq!(string_value(&data, "OriginalFilename"), "rawstride.exe");
    let copyright = string_value(&data, "LegalCopyright");
    assert!(copyright.starts_with("Copyright © 2026 "), "{copyright}");

    let fixed = query(&data, "\\").unwrap();
    assert!(fixed.len() >= std::mem::size_of::<VS_FIXEDFILEINFO>());
    // SAFETY: `fixed` holds at least one `VS_FIXEDFILEINFO`, read without assuming alignment.
    let fixed: VS_FIXEDFILEINFO = unsafe { std::ptr::read_unaligned(fixed.as_ptr().cast()) };
    assert_eq!(fixed.dwSignature, 0xFEEF_04BD);
    let part = |name: &str| name.parse::<u32>().unwrap();
    let major = part(env!("CARGO_PKG_VERSION_MAJOR"));
    let minor = part(env!("CARGO_PKG_VERSION_MINOR"));
    let patch = part(env!("CARGO_PKG_VERSION_PATCH"));
    assert_eq!(
        (
            fixed.dwFileVersionMS >> 16,
            fixed.dwFileVersionMS & 0xFFFF,
            fixed.dwFileVersionLS >> 16,
            fixed.dwFileVersionLS & 0xFFFF,
        ),
        (major, minor, patch, 0)
    );
}
