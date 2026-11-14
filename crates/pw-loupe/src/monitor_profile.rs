//! The monitor profile (ADR 0016 §4.1 "Reading the monitor profile", §5, Q4): the ICC profile
//! Windows associates with the primary monitor, read once at start; the window title that names it
//! is made in `shell_text.rs` (T116). Without a usable profile the program shows its images as
//! sRGB, as before (F3).

use std::path::PathBuf;

use pw_colour::Profile;

/// Where the monitor profile came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSource {
    /// Read from the profile file with this file name (not the path).
    File(String),
    /// No profile is associated with the monitor.
    Missing,
    /// A profile is associated but could not be used; the detail is English, for a later log, and
    /// is not shown.
    Unusable(String),
}

/// The profile the images are converted to (ADR 0016 C4) and where it came from.
#[derive(Debug, Clone)]
pub struct MonitorProfile {
    pub profile: Profile,
    pub source: ProfileSource,
}

/// Turns the result of [`primary_monitor_profile_path`] into the monitor profile: the profile file
/// read and parsed, or sRGB when there is none or it cannot be used. Never panics.
pub fn load_monitor_profile(path: Result<Option<PathBuf>, String>) -> MonitorProfile {
    let unusable = |detail: String| MonitorProfile {
        profile: Profile::srgb(),
        source: ProfileSource::Unusable(detail),
    };
    let path = match path {
        Ok(Some(path)) => path,
        Ok(None) => {
            return MonitorProfile {
                profile: Profile::srgb(),
                source: ProfileSource::Missing,
            };
        }
        Err(detail) => return unusable(detail),
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => return unusable(format!("{}: {e}", path.display())),
    };
    match Profile::from_icc(&bytes) {
        Ok(profile) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            MonitorProfile {
                profile,
                source: ProfileSource::File(name),
            }
        }
        Err(e) => unusable(format!("{}: {e}", path.display())),
    }
}

/// The path of the ICC profile Windows associates with the primary monitor (ADR 0016 §4.1).
/// `Ok(None)` when there is none; `Err` names the call that failed.
#[cfg(windows)]
pub fn primary_monitor_profile_path() -> Result<Option<PathBuf>, String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::Foundation::{GetLastError, POINT};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateDCW, DeleteDC, GetMonitorInfoW, HDC, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
        MONITORINFOEXW, MonitorFromPoint,
    };
    use windows_sys::Win32::UI::ColorSystem::GetICMProfileW;

    /// Deletes the device context on every path out of the function.
    struct Dc(HDC);
    impl Drop for Dc {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful `CreateDCW` and is deleted only here, once.
            unsafe { DeleteDC(self.0) };
        }
    }

    // SAFETY: plain call with a value argument; null means failure.
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    if monitor.is_null() {
        return Err("MonitorFromPoint failed".to_string());
    }

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = u32::try_from(std::mem::size_of::<MONITORINFOEXW>())
        .map_err(|_| "GetMonitorInfoW: structure size".to_string())?;
    // SAFETY: `info` is a MONITORINFOEXW with `cbSize` set to its size, so the function may write
    // the whole structure; MONITORINFO is its first field, so the pointer cast is valid.
    let ok = unsafe {
        GetMonitorInfoW(
            monitor,
            (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    };
    if ok == 0 {
        // SAFETY: plain call, reads the thread's last error.
        let code = unsafe { GetLastError() };
        return Err(format!("GetMonitorInfoW failed (error {code})"));
    }
    if !info.szDevice.contains(&0) {
        return Err("GetMonitorInfoW: device name not terminated".to_string());
    }

    let device = info.szDevice.as_ptr();
    // SAFETY: `device` points to a NUL-terminated UTF-16 string (checked above) that lives in
    // `info` for the whole call; the port and the mode may be null.
    let hdc = unsafe { CreateDCW(device, device, std::ptr::null(), std::ptr::null()) };
    if hdc.is_null() {
        return Err("CreateDCW failed".to_string());
    }
    let dc = Dc(hdc);

    let mut len: u32 = 0;
    // SAFETY: the device context is valid; with a null buffer the function only writes the
    // required length (in characters, with the NUL) to `len`.
    unsafe { GetICMProfileW(dc.0, &mut len, std::ptr::null_mut()) };
    if len == 0 {
        return Ok(None);
    }
    let size = usize::try_from(len).map_err(|_| "GetICMProfileW: length".to_string())?;
    let mut buffer = vec![0u16; size];
    // SAFETY: the device context is valid and `buffer` holds `len` characters, the length the
    // function asked for.
    let ok = unsafe { GetICMProfileW(dc.0, &mut len, buffer.as_mut_ptr()) };
    if ok == 0 {
        // SAFETY: plain call, reads the thread's last error.
        let code = unsafe { GetLastError() };
        return Err(format!("GetICMProfileW failed (error {code})"));
    }
    drop(dc);

    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let name = buffer.get(..end).unwrap_or_default();
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(OsString::from_wide(name))))
}

/// The path of the ICC profile of the primary monitor: none on other platforms.
#[cfg(not(windows))]
pub fn primary_monitor_profile_path() -> Result<Option<PathBuf>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use moxcms::ColorProfile;
    use pw_colour::{Profile, Transform};

    use super::{ProfileSource, load_monitor_profile, primary_monitor_profile_path};

    /// A file in the temporary folder with the process id in its name, removed when dropped.
    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str, bytes: &[u8]) -> TempFile {
            let path = std::env::temp_dir().join(format!("pw-loupe-{}-{name}", std::process::id()));
            std::fs::write(&path, bytes).unwrap();
            TempFile(path)
        }

        fn file_name(&self) -> String {
            self.0.file_name().unwrap().to_string_lossy().into_owned()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn assert_srgb(profile: &Profile) {
        assert!(
            Transform::new(&Profile::srgb(), profile)
                .unwrap()
                .is_identity()
        );
    }

    #[test]
    fn an_adobe_rgb_profile_file_is_read_and_converts() {
        let file = TempFile::new(
            "adobe.icc",
            &ColorProfile::new_adobe_rgb().encode().unwrap(),
        );
        let monitor = load_monitor_profile(Ok(Some(file.0.clone())));
        assert_eq!(monitor.source, ProfileSource::File(file.file_name()));

        let transform = Transform::new(&Profile::srgb(), &monitor.profile).unwrap();
        assert!(!transform.is_identity());
        let mut pixel = [255u8, 0, 0];
        transform.convert_rgb8(&mut pixel).unwrap();
        for (got, want) in pixel.iter().zip([219u8, 0, 0]) {
            assert!(
                got.abs_diff(want) <= 1,
                "{pixel:?} is not within 1 of (219, 0, 0)"
            );
        }
    }

    #[test]
    fn no_profile_is_missing_and_srgb() {
        let monitor = load_monitor_profile(Ok(None));
        assert_eq!(monitor.source, ProfileSource::Missing);
        assert_srgb(&monitor.profile);
    }

    #[test]
    fn a_profile_that_cannot_be_used_is_unusable_and_srgb() {
        let missing = std::env::temp_dir().join(format!(
            "pw-loupe-{}-does-not-exist.icc",
            std::process::id()
        ));
        let garbage = TempFile::new("garbage.icc", &[0x5a; 100]);
        let gray = TempFile::new(
            "gray.icc",
            &ColorProfile::new_gray_with_gamma(2.2).encode().unwrap(),
        );
        for path in [
            Ok(Some(missing)),
            Ok(Some(garbage.0.clone())),
            Ok(Some(gray.0.clone())),
            Err("x".to_string()),
        ] {
            let monitor = load_monitor_profile(path.clone());
            assert!(
                matches!(monitor.source, ProfileSource::Unusable(_)),
                "{path:?} gave {:?}",
                monitor.source
            );
            assert_srgb(&monitor.profile);
        }
        assert_eq!(
            load_monitor_profile(Err("x".to_string())).source,
            ProfileSource::Unusable("x".to_string())
        );
    }

    #[test]
    fn reading_the_primary_monitor_profile_path_does_not_panic() {
        // The value depends on the machine; only that it returns.
        let _ = primary_monitor_profile_path();
    }
}
