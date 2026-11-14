//! Windows file operations for the safe write (ADR 0007 §9.1 point 2, §9.3 point 5, §9.4; I8b,
//! T85).
//!
//! - [`can_write`] checks that a file may be written: not read-only, not on a read-only volume,
//!   not locked by another program, and enough free space on its volume (§9.1 point 2).
//! - [`flush`] and [`set_modified`] flush a written copy and set its modification time (§9.3
//!   point 5, §9.4).
//! - [`replace`] swaps the verified copy in for the original with `ReplaceFileW`, keeping the
//!   original as a backup; [`replace_by_rename`] does the same with two renames on volumes without
//!   `ReplaceFileW` (§9.4). [`move_new`] moves a new file (a new sidecar) into place without ever
//!   overwriting a file.
//! - [`temp_name`] makes the `.pwtmp-…`/`.pwbak-…` names next to the original.
//!
//! The Windows calls are behind `#[cfg(windows)]`; on other platforms those functions return
//! [`Error::Unsupported`]. Nothing uses this module until the safe write transaction (T86).
#![cfg_attr(not(test), allow(dead_code))]

use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

/// Why a file may not be written (§9.1 point 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The file has the read-only attribute.
    ReadOnlyFile,
    /// The file is on a read-only or write-protected volume.
    ReadOnlyVolume,
    /// Another program holds the file open so it cannot be opened exclusively.
    Locked,
    /// The volume has less free space than the file size + 10 % (+ the extra bytes asked for).
    NoSpace { needed: u64, free: u64 },
    /// One of the checks could not be made (the file is missing, a Windows call failed, or the
    /// platform is not Windows); holds the error text.
    Error(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::ReadOnlyFile => write!(f, "the file is read-only"),
            Refusal::ReadOnlyVolume => write!(f, "the volume is read-only or write-protected"),
            Refusal::Locked => write!(f, "the file is open in another program"),
            Refusal::NoSpace { needed, free } => write!(
                f,
                "not enough free space on the volume: {needed} bytes needed, {free} free"
            ),
            Refusal::Error(message) => write!(f, "the file could not be checked: {message}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// How a failed replace left the files (§9.5). Each holds a text that names the files and the
/// Windows error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplaceError {
    /// The original is still under its own name and unchanged; the copy was not moved.
    OriginalIntact(String),
    /// The original was moved to the backup name and is not back under its own name.
    OriginalAtBackup(String),
}

impl fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplaceError::OriginalIntact(m) => write!(f, "the original was not replaced: {m}"),
            ReplaceError::OriginalAtBackup(m) => {
                write!(f, "the original is under the backup name: {m}")
            }
        }
    }
}

impl std::error::Error for ReplaceError {}

/// Bytes available to the current user on the volume of `folder` (`GetDiskFreeSpaceExW`).
#[cfg(windows)]
pub(crate) fn free_space(folder: &Path) -> Result<u64> {
    let folder_w = wide(folder)?;
    let mut available: u64 = 0;
    // SAFETY: `folder_w` is a NUL-terminated UTF-16 string that lives for the call; `available`
    // is a valid `u64` to write to; the two other out-pointers may be null.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            folder_w.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    Ok(available)
}

#[cfg(not(windows))]
pub(crate) fn free_space(_folder: &Path) -> Result<u64> {
    Err(unsupported("free_space"))
}

/// `FILE_READ_ONLY_VOLUME` from `winnt.h`. `windows-sys` has it in the
/// `Win32_System_SystemServices` feature, which `pw-metadata` does not enable (D3).
#[cfg(windows)]
const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;

/// Whether `path` is on a read-only volume (`GetVolumePathNameW`, then `GetVolumeInformationW` and
/// `FILE_READ_ONLY_VOLUME`).
#[cfg(windows)]
pub(crate) fn is_read_only_volume(path: &Path) -> Result<bool> {
    let path_w = wide(path)?;
    // The volume path is never longer than the path itself plus a trailing backslash.
    let capacity = path_w.len().saturating_add(2).max(261);
    let mut root = vec![0u16; capacity];
    let capacity_u32 = u32::try_from(capacity)
        .map_err(|_| Error::Unsupported(format!("path too long: {}", path.display())))?;
    // SAFETY: `path_w` is NUL-terminated UTF-16; `root` is a writable buffer of exactly
    // `capacity_u32` UTF-16 units that lives for the call.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW(
            path_w.as_ptr(),
            root.as_mut_ptr(),
            capacity_u32,
        )
    };
    if ok == 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    // Windows writes a NUL-terminated string into `root`; make sure the buffer ends in NUL even
    // if it filled up.
    if let Some(last) = root.last_mut() {
        *last = 0;
    }
    let mut flags: u32 = 0;
    // SAFETY: `root` is NUL-terminated UTF-16 (ensured above); `flags` is a valid `u32` to write
    // to; the other buffers are null with size 0 and the other out-pointers are null, which the
    // function allows.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut flags,
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    Ok(flags & FILE_READ_ONLY_VOLUME != 0)
}

#[cfg(not(windows))]
pub(crate) fn is_read_only_volume(_path: &Path) -> Result<bool> {
    Err(unsupported("is_read_only_volume"))
}

/// Whether another program holds `path` open: opening it for reading and writing without sharing
/// fails with a sharing or lock violation. Any other failure is an error.
#[cfg(windows)]
pub(crate) fn is_locked(path: &Path) -> Result<bool> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Foundation::{ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION};

    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
    {
        Ok(_) => Ok(false),
        Err(e) if is_code(&e, ERROR_SHARING_VIOLATION) || is_code(&e, ERROR_LOCK_VIOLATION) => {
            Ok(true)
        }
        Err(e) => Err(Error::Io(e)),
    }
}

#[cfg(not(windows))]
pub(crate) fn is_locked(_path: &Path) -> Result<bool> {
    Err(unsupported("is_locked"))
}

/// Whether `original` may be written (§9.1 point 2): not read-only, not on a read-only volume,
/// not locked, and its volume has at least the file size + 10 % + `extra_bytes` free.
pub(crate) fn can_write(original: &Path, extra_bytes: u64) -> std::result::Result<(), Refusal> {
    let refusal = |e: Error| Refusal::Error(e.to_string());
    let metadata = std::fs::metadata(original).map_err(|e| refusal(Error::Io(e)))?;
    if metadata.permissions().readonly() {
        return Err(Refusal::ReadOnlyFile);
    }
    if is_read_only_volume(original).map_err(refusal)? {
        return Err(Refusal::ReadOnlyVolume);
    }
    if is_locked(original).map_err(refusal)? {
        return Err(Refusal::Locked);
    }
    let size = metadata.len();
    let needed = size
        .saturating_add(size.div_ceil(10))
        .saturating_add(extra_bytes);
    let free = free_space(&folder_of(original)).map_err(refusal)?;
    if free < needed {
        return Err(Refusal::NoSpace { needed, free });
    }
    Ok(())
}

/// Sets the modification time of `file` (`File::set_modified`).
pub(crate) fn set_modified(file: &Path, time: SystemTime) -> Result<()> {
    File::options().write(true).open(file)?.set_modified(time)?;
    Ok(())
}

/// Flushes `file` to disk (`File::sync_all`, which calls `FlushFileBuffers` on Windows).
pub(crate) fn flush(file: &Path) -> Result<()> {
    File::options().write(true).open(file)?.sync_all()?;
    Ok(())
}

/// Replaces `original` with `copy` in one step with `ReplaceFileW`, keeping the old original as
/// `backup` (§9.4). `ReplaceFileW` keeps the original's creation time, attributes, ACL and
/// alternate data streams; the modification time is the copy's. On a volume without
/// `ReplaceFileW` (`ERROR_INVALID_FUNCTION`, `ERROR_NOT_SUPPORTED`) it falls back to
/// [`replace_by_rename`].
///
/// When the original has an alternate data stream, `ReplaceFileW` writes it into the copy and so
/// sets the modification time to now (measured on NTFS, 2026-11-12). The copy's modification time
/// is therefore read before the call and set on the result again afterwards. If only that last
/// step fails the result is still `Ok`: the original has been replaced with the verified copy and
/// neither [`ReplaceError`] would describe the files truthfully.
#[cfg(windows)]
pub(crate) fn replace(
    original: &Path,
    copy: &Path,
    backup: &Path,
) -> std::result::Result<(), ReplaceError> {
    use windows_sys::Win32::Foundation::{
        ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED, ERROR_UNABLE_TO_MOVE_REPLACEMENT_2,
    };

    let intact = |e: Error| ReplaceError::OriginalIntact(e.to_string());
    let copy_modified = std::fs::metadata(copy)
        .and_then(|m| m.modified())
        .map_err(|e| intact(Error::Io(e)))?;
    let original_w = wide(original).map_err(intact)?;
    let copy_w = wide(copy).map_err(intact)?;
    let backup_w = wide(backup).map_err(intact)?;
    // SAFETY: the three names are NUL-terminated UTF-16 strings that live for the call; no flags;
    // `lpexclude` and `lpreserved` must be null.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            original_w.as_ptr(),
            copy_w.as_ptr(),
            backup_w.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok != 0 {
        // See above: the replace is done; a failure here leaves a correct file with a newer time.
        let _ = set_modified(original, copy_modified);
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if is_code(&error, ERROR_INVALID_FUNCTION) || is_code(&error, ERROR_NOT_SUPPORTED) {
        return replace_by_rename(original, copy, backup);
    }
    let message = format!(
        "replacing {} with {} failed: {error}",
        original.display(),
        copy.display()
    );
    if is_code(&error, ERROR_UNABLE_TO_MOVE_REPLACEMENT_2) {
        return Err(ReplaceError::OriginalAtBackup(format!(
            "{message}; the original is {}",
            backup.display()
        )));
    }
    // ERROR_UNABLE_TO_MOVE_REPLACEMENT and every other failure: where is the original now?
    if !original.exists() && backup.exists() {
        Err(ReplaceError::OriginalAtBackup(format!(
            "{message}; the original is {}",
            backup.display()
        )))
    } else {
        Err(ReplaceError::OriginalIntact(message))
    }
}

#[cfg(not(windows))]
pub(crate) fn replace(
    _original: &Path,
    _copy: &Path,
    _backup: &Path,
) -> std::result::Result<(), ReplaceError> {
    Err(ReplaceError::OriginalIntact(
        unsupported("replace").to_string(),
    ))
}

/// Replaces `original` with `copy` by two renames (§9.4, for volumes without `ReplaceFileW`):
/// `original` → `backup`, then `copy` → `original`, both with `MOVEFILE_WRITE_THROUGH`. If the
/// second rename fails the first is undone ([`ReplaceError::OriginalIntact`]); if that fails too
/// the original stays under `backup` ([`ReplaceError::OriginalAtBackup`]).
pub(crate) fn replace_by_rename(
    original: &Path,
    copy: &Path,
    backup: &Path,
) -> std::result::Result<(), ReplaceError> {
    if let Err(e) = move_new(original, backup) {
        return Err(ReplaceError::OriginalIntact(format!(
            "moving {} to {} failed: {e}",
            original.display(),
            backup.display()
        )));
    }
    let Err(e) = move_new(copy, original) else {
        return Ok(());
    };
    let message = format!(
        "moving {} to {} failed: {e}",
        copy.display(),
        original.display()
    );
    match move_new(backup, original) {
        Ok(()) => Err(ReplaceError::OriginalIntact(message)),
        Err(undo) => Err(ReplaceError::OriginalAtBackup(format!(
            "{message}; moving it back failed: {undo}; the original is {}",
            backup.display()
        ))),
    }
}

/// Moves `copy` to `target` with `MoveFileExW` and `MOVEFILE_WRITE_THROUGH`, **without**
/// `MOVEFILE_REPLACE_EXISTING`: an existing `target` is never overwritten (a new sidecar never
/// replaces a file that appeared meanwhile).
#[cfg(windows)]
pub(crate) fn move_new(copy: &Path, target: &Path) -> Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let copy_w = wide(copy)?;
    let target_w = wide(target)?;
    // SAFETY: both names are NUL-terminated UTF-16 strings that live for the call.
    let ok = unsafe { MoveFileExW(copy_w.as_ptr(), target_w.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn move_new(_copy: &Path, _target: &Path) -> Result<()> {
    Err(unsupported("move_new"))
}

/// A free name next to `original`: `<stem>.<tag>-<16 hex>.<ext>` (`<stem>.<tag>-<16 hex>` when
/// there is no extension), with `tag` `pwtmp` or `pwbak`. The hex comes from the time, the process
/// id and a counter; a name that exists is never returned.
pub(crate) fn temp_name(original: &Path, tag: &str) -> PathBuf {
    let folder = original.parent().unwrap_or_else(|| Path::new(""));
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    let extension = original.extension().map(|e| e.to_string_lossy());
    loop {
        let name = match &extension {
            Some(ext) => format!("{stem}.{tag}-{:016x}.{ext}", unique_number()),
            None => format!("{stem}.{tag}-{:016x}", unique_number()),
        };
        let candidate = folder.join(name);
        // Only a name known to exist is skipped; the counter makes every attempt different.
        if !matches!(candidate.try_exists(), Ok(true)) {
            return candidate;
        }
    }
}

/// A number that differs on every call in this process and between processes: the time in
/// nanoseconds, the process id and a counter, mixed.
pub(crate) fn unique_number() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = u64::from(std::process::id());
    nanos ^ pid.rotate_left(40) ^ count.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// The folder of `file` for `free_space`; `.` for a bare file name.
fn folder_of(file: &Path) -> PathBuf {
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// `path` as a NUL-terminated UTF-16 string for a Windows call. A path with a NUL in it is refused
/// (Windows would read only the part before it).
#[cfg(windows)]
fn wide(path: &Path) -> Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.contains(&0) {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path contains a NUL character: {}", path.display()),
        )));
    }
    units.push(0);
    Ok(units)
}

#[cfg(windows)]
fn is_code(error: &std::io::Error, code: u32) -> bool {
    error.raw_os_error() == i32::try_from(code).ok()
}

#[cfg(not(windows))]
fn unsupported(function: &str) -> Error {
    Error::Unsupported(format!("{function} is only available on Windows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temporary folder, removed when dropped.
    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pw-metadata-fileops-{name}-{}-{:016x}",
                std::process::id(),
                unique_number()
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempFolder(path)
        }

        fn file(&self, name: &str, content: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, content).unwrap();
            path
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            // Clear read-only attributes so the folder can be removed.
            if let Ok(entries) = std::fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        let mut permissions = metadata.permissions();
                        #[allow(clippy::permissions_set_readonly_false)]
                        permissions.set_readonly(false);
                        let _ = std::fs::set_permissions(entry.path(), permissions);
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    fn open_exclusive(path: &Path) -> File {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(path)
            .unwrap()
    }

    #[test]
    #[cfg(windows)]
    fn free_space_of_the_temporary_folder_is_above_zero() {
        let folder = TempFolder::new("free");
        assert!(free_space(&folder.0).unwrap() > 0);
    }

    #[test]
    #[cfg(windows)]
    fn the_temporary_folder_is_not_on_a_read_only_volume() {
        let folder = TempFolder::new("volume");
        let file = folder.file("a.jpg", b"jpeg");
        assert!(!is_read_only_volume(&folder.0).unwrap());
        assert!(!is_read_only_volume(&file).unwrap());
    }

    #[test]
    #[cfg(windows)]
    fn a_file_is_locked_while_held_without_sharing() {
        let folder = TempFolder::new("locked");
        let file = folder.file("a.jpg", b"jpeg");
        assert!(!is_locked(&file).unwrap());
        let held = open_exclusive(&file);
        assert!(is_locked(&file).unwrap());
        drop(held);
        assert!(!is_locked(&file).unwrap());
    }

    #[test]
    #[cfg(windows)]
    fn is_locked_of_a_missing_file_is_an_error() {
        let folder = TempFolder::new("locked-missing");
        assert!(is_locked(&folder.0.join("missing.jpg")).is_err());
    }

    #[test]
    #[cfg(windows)]
    fn can_write_accepts_a_normal_file() {
        let folder = TempFolder::new("can-write");
        let file = folder.file("a.jpg", b"jpeg");
        assert_eq!(can_write(&file, 0), Ok(()));
    }

    #[test]
    #[cfg(windows)]
    fn can_write_refuses_a_read_only_file() {
        let folder = TempFolder::new("can-write-ro");
        let file = folder.file("a.jpg", b"jpeg");
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions).unwrap();
        assert_eq!(can_write(&file, 0), Err(Refusal::ReadOnlyFile));
    }

    #[test]
    #[cfg(windows)]
    fn can_write_refuses_a_locked_file() {
        let folder = TempFolder::new("can-write-locked");
        let file = folder.file("a.jpg", b"jpeg");
        let held = open_exclusive(&file);
        assert_eq!(can_write(&file, 0), Err(Refusal::Locked));
        drop(held);
        assert_eq!(can_write(&file, 0), Ok(()));
    }

    #[test]
    #[cfg(windows)]
    fn can_write_refuses_when_the_extra_bytes_exceed_the_free_space() {
        let folder = TempFolder::new("can-write-space");
        let file = folder.file("a.jpg", &[0u8; 100]);
        // A margin of 1 TiB above the free space read here, so that no change of the free space
        // between this reading and the one inside `can_write` changes the result. Not `u64::MAX`:
        // `needed` would saturate and the sum would no longer be checked.
        const MARGIN: u64 = 1 << 40;
        let extra = free_space(&folder.0).unwrap().saturating_add(MARGIN);
        match can_write(&file, extra) {
            Err(Refusal::NoSpace {
                needed,
                free: reported,
            }) => {
                // 100 bytes + 10 % + the extra bytes.
                assert_eq!(needed, 110 + extra);
                assert!(reported < needed);
            }
            other => panic!("expected NoSpace, got {other:?}"),
        }
    }

    #[test]
    fn can_write_of_a_missing_file_is_an_error() {
        let folder = TempFolder::new("can-write-missing");
        assert!(matches!(
            can_write(&folder.0.join("missing.jpg"), 0),
            Err(Refusal::Error(_))
        ));
    }

    #[test]
    #[cfg(windows)]
    fn replace_keeps_creation_time_attributes_and_streams_and_takes_the_copys_modification_time() {
        use std::os::windows::fs::{FileTimesExt, MetadataExt};
        use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, SetFileAttributesW};

        let folder = TempFolder::new("replace");
        let original = folder.file("a.jpg", b"old content");
        let copy = folder.file("a.pwtmp-1.jpg", b"new content");
        let backup = folder.0.join("a.pwbak-1.jpg");

        // Creation time 2020-01-01 00:00:00 UTC.
        let created = UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800);
        File::options()
            .write(true)
            .open(&original)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_created(created))
            .unwrap();
        assert_eq!(
            std::fs::metadata(&original).unwrap().created().unwrap(),
            created
        );
        // An alternate data stream on the original.
        let stream = PathBuf::from(format!("{}:pw.test", original.display()));
        std::fs::write(&stream, b"stream content").unwrap();
        // The hidden attribute.
        let original_w = wide(&original).unwrap();
        // SAFETY: `original_w` is NUL-terminated UTF-16 and lives for the call.
        let ok = unsafe { SetFileAttributesW(original_w.as_ptr(), FILE_ATTRIBUTE_HIDDEN) };
        assert_ne!(ok, 0, "{}", std::io::Error::last_os_error());

        let modified = UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        set_modified(&copy, modified).unwrap();
        flush(&copy).unwrap();

        replace(&original, &copy, &backup).unwrap();

        assert_eq!(std::fs::read(&original).unwrap(), b"new content");
        assert_eq!(std::fs::read(&backup).unwrap(), b"old content");
        assert!(!copy.exists());
        let metadata = std::fs::metadata(&original).unwrap();
        assert_eq!(metadata.created().unwrap(), created);
        assert_eq!(metadata.modified().unwrap(), modified);
        assert_ne!(metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN, 0);
        assert_eq!(std::fs::read(&stream).unwrap(), b"stream content");
    }

    #[test]
    #[cfg(windows)]
    fn replace_of_a_locked_original_leaves_it_intact() {
        let folder = TempFolder::new("replace-locked");
        let original = folder.file("a.jpg", b"old content");
        let copy = folder.file("a.pwtmp-1.jpg", b"new content");
        let backup = folder.0.join("a.pwbak-1.jpg");

        let held = open_exclusive(&original);
        let result = replace(&original, &copy, &backup);
        drop(held);

        assert!(
            matches!(result, Err(ReplaceError::OriginalIntact(_))),
            "{result:?}"
        );
        assert_eq!(std::fs::read(&original).unwrap(), b"old content");
        assert_eq!(std::fs::read(&copy).unwrap(), b"new content");
    }

    #[test]
    #[cfg(windows)]
    fn replace_by_rename_puts_the_copy_in_place_and_keeps_the_backup() {
        let folder = TempFolder::new("rename");
        let original = folder.file("a.jpg", b"old content");
        let copy = folder.file("a.pwtmp-1.jpg", b"new content");
        let backup = folder.0.join("a.pwbak-1.jpg");

        replace_by_rename(&original, &copy, &backup).unwrap();

        assert_eq!(std::fs::read(&original).unwrap(), b"new content");
        assert_eq!(std::fs::read(&backup).unwrap(), b"old content");
        assert!(!copy.exists());
    }

    #[test]
    #[cfg(windows)]
    fn replace_by_rename_without_the_copy_puts_the_original_back() {
        let folder = TempFolder::new("rename-missing");
        let original = folder.file("a.jpg", b"old content");
        let copy = folder.0.join("a.pwtmp-1.jpg");
        let backup = folder.0.join("a.pwbak-1.jpg");

        let result = replace_by_rename(&original, &copy, &backup);

        assert!(
            matches!(result, Err(ReplaceError::OriginalIntact(_))),
            "{result:?}"
        );
        assert_eq!(std::fs::read(&original).unwrap(), b"old content");
        assert!(!backup.exists());
    }

    #[test]
    #[cfg(windows)]
    fn move_new_moves_to_a_free_name_and_refuses_an_existing_target() {
        let folder = TempFolder::new("move-new");
        let copy = folder.file("a.pwtmp-1.xmp", b"new sidecar");
        let target = folder.0.join("a.xmp");

        move_new(&copy, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new sidecar");
        assert!(!copy.exists());

        let second = folder.file("a.pwtmp-2.xmp", b"second sidecar");
        assert!(move_new(&second, &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"new sidecar");
        assert_eq!(std::fs::read(&second).unwrap(), b"second sidecar");
    }

    #[test]
    fn temp_name_keeps_the_folder_and_extension() {
        let folder = TempFolder::new("temp-name");
        let original = folder.0.join("IMG_0001.dng");

        let name = temp_name(&original, "pwtmp");

        assert_eq!(name.parent(), Some(folder.0.as_path()));
        assert_eq!(name.extension().and_then(|e| e.to_str()), Some("dng"));
        let file_name = name.file_name().and_then(|n| n.to_str()).unwrap();
        let hex = file_name
            .strip_prefix("IMG_0001.pwtmp-")
            .and_then(|rest| rest.strip_suffix(".dng"))
            .unwrap();
        assert_eq!(hex.len(), 16);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!name.exists());

        let backup = temp_name(&original, "pwbak");
        assert!(
            backup
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap()
                .starts_with("IMG_0001.pwbak-")
        );
    }

    #[test]
    fn temp_name_never_returns_an_existing_name() {
        let folder = TempFolder::new("temp-name-unique");
        let original = folder.0.join("a.jpg");
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let name = temp_name(&original, "pwtmp");
            assert!(!name.exists(), "{}", name.display());
            // Create it, so a later call that came up with the same name would have to skip it.
            std::fs::write(&name, b"").unwrap();
            assert!(seen.insert(name));
        }
    }
}
