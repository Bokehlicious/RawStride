//! Deleting an image with its companion files to the Windows Recycle Bin (ADR 0018, step B1;
//! T185).
//!
//! - [`drive_allows_delete`] says whether the drive of a path allows a delete: a fixed drive with
//!   a Recycle Bin that is turned on; never a memory card, a network drive or a drive whose
//!   Recycle Bin settings cannot be read (ADR 0018 §2.6).
//! - [`inspect_group`] and [`check_group`] check every file of one image's group before anything
//!   is deleted (ADR 0018 §2.4): one folder, each file there and not a folder, a path the Recycle
//!   Bin takes, the total size within the bin's capacity; [`check_group`] also that no other
//!   program holds a file open.
//! - [`delete_group`] sends the group to the Recycle Bin in one Windows call (ADR 0018 §2.9).
//!
//! RawStride never deletes permanently: every case in which Windows would delete instead of using
//! the Recycle Bin is refused before the call (ADR 0018 §2.6). It never empties the Recycle Bin and
//! never deletes a folder.

use std::fmt;
use std::path::{Path, PathBuf};

// The drive types of `GetDriveTypeW`. `windows-sys` has them in the
// `Win32_System_WindowsProgramming` feature, which `pw-metadata` does not enable; the decision
// that uses them is tested on every platform, so they are not behind `cfg(windows)`. The decision
// names only the two it acts on; the others are named by its tests.
/// The drive type cannot be determined.
#[cfg_attr(not(test), allow(dead_code))]
const DRIVE_UNKNOWN: u32 = 0;
/// The root path is invalid.
#[cfg_attr(not(test), allow(dead_code))]
const DRIVE_NO_ROOT_DIR: u32 = 1;
/// A removable drive (a memory card, a USB stick).
const DRIVE_REMOVABLE: u32 = 2;
/// A fixed drive.
const DRIVE_FIXED: u32 = 3;
/// A network drive.
#[cfg_attr(not(test), allow(dead_code))]
const DRIVE_REMOTE: u32 = 4;
/// An optical drive.
#[cfg_attr(not(test), allow(dead_code))]
const DRIVE_CDROM: u32 = 5;
/// A RAM disk.
#[cfg_attr(not(test), allow(dead_code))]
const DRIVE_RAMDISK: u32 = 6;

/// Bytes in one of `MaxCapacity`'s megabytes.
const MEGABYTE: u64 = 1_048_576;

/// The full paths `SHFileOperationW` takes are shorter than this many UTF-16 units.
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_PATH_UNITS: usize = 260;

/// Why a file or its drive may not be deleted. `Display` gives the reason as an English phrase
/// without a full stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteRefusal {
    /// A removable drive, or a drive with a `DCIM` folder at its root (ADR 0018 §2.6).
    Card,
    /// A network drive or share, an optical or RAM disk, a drive of unknown type, or a drive
    /// without a volume name.
    NoRecycleBin,
    /// `NukeOnDelete` is not 0 for the drive, or the policy `NoRecycleFiles` is not 0.
    RecycleBinOff,
    /// The group's total size is larger than the drive's Recycle Bin capacity.
    TooLarge { bytes: u64, capacity: u64 },
    /// The Recycle Bin's settings for the drive could not be read; holds the Windows error text.
    RecycleBinUnknown(String),
    /// The file does not exist.
    Missing,
    /// The path is not a file (a folder, a link).
    NotAFile,
    /// Another program holds the file open.
    OpenElsewhere,
    /// The files of the group are not in one folder.
    NotOneFolder,
    /// The full path is 260 UTF-16 units or longer.
    PathTooLong,
    /// Not on Windows.
    Unsupported,
    /// Anything else; holds the system's error text.
    Failed(String),
}

impl fmt::Display for DeleteRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeleteRefusal::Card => f.write_str("the drive is a memory card"),
            DeleteRefusal::NoRecycleBin => f.write_str("the drive has no Recycle Bin"),
            DeleteRefusal::RecycleBinOff => {
                f.write_str("the Recycle Bin is turned off for the drive")
            }
            DeleteRefusal::TooLarge { bytes, capacity } => write!(
                f,
                "the files are too large for the Recycle Bin ({bytes} bytes, the Recycle Bin \
                 holds {capacity})"
            ),
            DeleteRefusal::RecycleBinUnknown(text) => write!(
                f,
                "the Recycle Bin's settings for the drive could not be read: {text}"
            ),
            DeleteRefusal::Missing => f.write_str("the file does not exist"),
            DeleteRefusal::NotAFile => f.write_str("it is not a file"),
            DeleteRefusal::OpenElsewhere => f.write_str("the file is open in another program"),
            DeleteRefusal::NotOneFolder => f.write_str("the files are not in one folder"),
            DeleteRefusal::PathTooLong => f.write_str("the path is too long for the Recycle Bin"),
            DeleteRefusal::Unsupported => {
                f.write_str("deleting to the Recycle Bin is only available on Windows")
            }
            DeleteRefusal::Failed(text) => f.write_str(text),
        }
    }
}

/// A group that could not be deleted, or was deleted only in part. Every path is the caller's
/// path as given, so the caller can match it with its own lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteError {
    /// The file that could not be deleted; for a refusal of the drive, the group's first file.
    pub file: PathBuf,
    /// Why.
    pub reason: DeleteRefusal,
    /// The files of the group still in place when some of the group already went to the Recycle
    /// Bin (ADR 0018 §2.4); empty when nothing of the group was deleted.
    pub left: Vec<PathBuf>,
    /// The files of the group that went to the Recycle Bin before the failure; empty when nothing
    /// was deleted.
    pub deleted: Vec<PathBuf>,
}

impl DeleteError {
    fn new(file: &Path, reason: DeleteRefusal) -> Self {
        DeleteError {
            file: file.to_path_buf(),
            reason,
            left: Vec::new(),
            deleted: Vec::new(),
        }
    }
}

impl fmt::Display for DeleteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = name_of(&self.file);
        let reason = &self.reason;
        if self.left.is_empty() {
            write!(f, "Could not move {name} to the Recycle Bin: {reason}.")
        } else {
            let left = self
                .left
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                f,
                "Could not move {name} to the Recycle Bin: {reason}. Some files are already in \
                 the Recycle Bin; these were left: {left}."
            )
        }
    }
}

impl std::error::Error for DeleteError {}

/// The Recycle Bin settings of one volume (`BitBucket\Volume\{GUID}` in `HKEY_CURRENT_USER`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
struct BinSettings {
    /// Not 0: files are deleted at once instead of going to the Recycle Bin.
    nuke_on_delete: u32,
    /// The bin's capacity in megabytes.
    max_capacity: u32,
}

/// The drive decision (ADR 0018 §2.6, B-Q5, B-Q6, B-Q14) from its inputs, without any Windows
/// call: the bin's capacity in bytes, or why the drive refuses a delete. `policy` is `Ok(true)`
/// when `NoRecycleFiles` is not 0 in either hive, `Err` with the Windows error text when it could
/// not be read; `settings` likewise for the volume's `NukeOnDelete` and `MaxCapacity`.
#[cfg_attr(not(windows), allow(dead_code))]
fn decide(
    drive_type: u32,
    has_dcim: bool,
    volume_guid: Option<&str>,
    policy: Result<bool, String>,
    settings: Result<BinSettings, String>,
) -> Result<u64, DeleteRefusal> {
    match drive_type {
        DRIVE_REMOVABLE => return Err(DeleteRefusal::Card),
        DRIVE_FIXED => {}
        _ => return Err(DeleteRefusal::NoRecycleBin),
    }
    if has_dcim {
        return Err(DeleteRefusal::Card);
    }
    if volume_guid.is_none() {
        return Err(DeleteRefusal::NoRecycleBin);
    }
    match policy {
        Ok(true) => return Err(DeleteRefusal::RecycleBinOff),
        Ok(false) => {}
        Err(text) => return Err(DeleteRefusal::RecycleBinUnknown(text)),
    }
    let settings = settings.map_err(DeleteRefusal::RecycleBinUnknown)?;
    if settings.nuke_on_delete != 0 {
        return Err(DeleteRefusal::RecycleBinOff);
    }
    Ok(u64::from(settings.max_capacity).saturating_mul(MEGABYTE))
}

/// The `{GUID}` part of a volume name such as `\\?\Volume{GUID}\`, braces included; `None` when
/// the name has no `{…}` part.
#[cfg_attr(not(windows), allow(dead_code))]
fn guid_of(volume_name: &str) -> Option<&str> {
    let start = volume_name.find('{')?;
    let rest = volume_name.get(start..)?;
    let end = rest.find('}')?;
    let guid = rest.get(..=end)?;
    (guid.len() > 2).then_some(guid)
}

/// Whether the drive of `path` (a file or a folder) allows a delete, without touching any file
/// (ADR 0018 §2.6): a fixed drive, not a memory card, with a volume name, the Recycle Bin turned
/// on for it and its settings readable. The path is made absolute, then resolved, so a junction
/// or a folder link is judged by the drive it points to.
pub fn drive_allows_delete(path: &Path) -> Result<(), DeleteRefusal> {
    imp::drive_allows_delete(path)
}

/// Every check of ADR 0018 §2.4 and §2.6 for one image's group before anything is deleted;
/// returns the group's total size in bytes. Nothing is changed on disk. An empty list is `Ok(0)`.
/// The error names the caller's path as given.
pub fn check_group(files: &[PathBuf]) -> Result<u64, DeleteError> {
    imp::inspect(files, true).map(|(_, total)| total)
}

/// The checks of [`check_group`] without the check that no other program holds a file open, for
/// the confirmation before a delete (the view cores still hold the current image open then).
pub fn inspect_group(files: &[PathBuf]) -> Result<u64, DeleteError> {
    imp::inspect(files, false).map(|(_, total)| total)
}

/// Sends `files`, one image's group, to the Recycle Bin in one Windows call, after
/// [`check_group`]; a failed check deletes nothing. An empty list is `Ok(())`.
///
/// When the call fails part of the way, the error's `left` holds the files still in place and
/// `deleted` those already in the Recycle Bin. Known limit (ADR 0018 §2.6): if the Recycle Bin's
/// settings change between the check and the call, Windows would delete permanently; the check
/// right before the call is the only guard.
pub fn delete_group(files: &[PathBuf]) -> Result<(), DeleteError> {
    imp::delete_group(files)
}

/// The file name of `path`, or the whole path when it has none.
fn name_of(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

/// One file of a group: the caller's path, and the path used for the checks and the call.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Prepared {
    given: PathBuf,
    normal: PathBuf,
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::path::{Component, Path, PathBuf, Prefix};

    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
    };

    use super::{
        BinSettings, DRIVE_FIXED, DeleteError, DeleteRefusal, MAX_PATH_UNITS, Prepared, decide,
        guid_of,
    };

    /// The policy key that holds `NoRecycleFiles`, in both hives.
    const POLICY_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";
    /// The key of each volume's Recycle Bin settings, followed by `\{GUID}`.
    const BIT_BUCKET_KEY: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume";

    pub(super) fn drive_allows_delete(path: &Path) -> Result<(), DeleteRefusal> {
        let absolute = std::path::absolute(path).map_err(|e| failed(&e))?;
        let normal = normal_form(&absolute)?;
        drive_of(&normal).map(|_| ())
    }

    /// `path` resolved (`canonicalize`) and in the normal form, and the capacity of its drive's
    /// Recycle Bin in bytes (ADR 0018 §2.6; T185 points 4 and 4a).
    pub(super) fn drive_of(path: &Path) -> Result<(PathBuf, u64), DeleteRefusal> {
        let canonical = std::fs::canonicalize(path).map_err(|e| failed(&e))?;
        let canonical = normal_form(&canonical)?;
        let root = volume_root(&canonical)?;
        let root_w = wide(&root)?;
        // SAFETY: `root_w` is a NUL-terminated UTF-16 string that lives for the call.
        let drive_type =
            unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root_w.as_ptr()) };
        let fixed = drive_type == DRIVE_FIXED;
        let has_dcim = fixed && root.join("DCIM").is_dir();
        let guid = if fixed && !has_dcim {
            volume_name(&root_w).and_then(|name| guid_of(&name).map(str::to_string))
        } else {
            None
        };
        let (policy, settings) = match &guid {
            Some(guid) => (read_policy(), read_settings(guid)),
            None => (Ok(false), Err(String::new())),
        };
        let capacity = decide(drive_type, has_dcim, guid.as_deref(), policy, settings)?;
        Ok((canonical, capacity))
    }

    /// `path` in the form `SHFileOperationW` takes (T185 point 4a): a verbatim disk path
    /// (`\\?\C:\…`) becomes `C:\…`; any other verbatim or device form is `NoRecycleBin`; anything
    /// else is kept.
    pub(super) fn normal_form(path: &Path) -> Result<PathBuf, DeleteRefusal> {
        let mut components = path.components();
        match components.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::VerbatimDisk(letter) => {
                    let mut normal = PathBuf::from(format!("{}:\\", char::from(letter)));
                    for component in components {
                        if !matches!(component, Component::RootDir) {
                            normal.push(component);
                        }
                    }
                    Ok(normal)
                }
                Prefix::Verbatim(_) | Prefix::VerbatimUNC(..) | Prefix::DeviceNS(_) => {
                    Err(DeleteRefusal::NoRecycleBin)
                }
                Prefix::UNC(..) | Prefix::Disk(_) => Ok(path.to_path_buf()),
            },
            _ => Ok(path.to_path_buf()),
        }
    }

    /// The checks of `check_group` (`lock_check`) or `inspect_group`: the group's files in the
    /// form for the call, and their total size.
    pub(super) fn inspect(
        files: &[PathBuf],
        lock_check: bool,
    ) -> Result<(Vec<Prepared>, u64), DeleteError> {
        let Some(first) = files.first() else {
            return Ok((Vec::new(), 0));
        };

        // Each path absolute and in the normal form, all in one folder, none twice.
        let mut normals: Vec<PathBuf> = Vec::with_capacity(files.len());
        let mut seen: Vec<String> = Vec::with_capacity(files.len());
        let mut folder: Option<(PathBuf, String)> = None;
        for file in files {
            let absolute =
                std::path::absolute(file).map_err(|e| DeleteError::new(file, failed(&e)))?;
            let normal = normal_form(&absolute).map_err(|reason| DeleteError::new(file, reason))?;
            let parent = normal.parent().map(Path::to_path_buf).unwrap_or_default();
            let parent_key = lower(&parent);
            match &folder {
                None => folder = Some((parent, parent_key)),
                Some((_, key)) if *key != parent_key => {
                    return Err(DeleteError::new(file, DeleteRefusal::NotOneFolder));
                }
                Some(_) => {}
            }
            let key = lower(&normal);
            if seen.contains(&key) {
                return Err(DeleteError::new(
                    file,
                    DeleteRefusal::Failed("the file is listed twice".to_string()),
                ));
            }
            seen.push(key);
            normals.push(normal);
        }
        let Some((folder, _)) = folder else {
            return Ok((Vec::new(), 0));
        };

        // The drive, judged by the folder it really is (a junction or link resolved).
        let (canonical, capacity) =
            drive_of(&folder).map_err(|reason| DeleteError::new(first, reason))?;

        // Each file.
        let mut prepared: Vec<Prepared> = Vec::with_capacity(files.len());
        let mut total: u64 = 0;
        for (file, normal) in files.iter().zip(&normals) {
            let Some(name) = normal.file_name() else {
                return Err(DeleteError::new(file, DeleteRefusal::NotAFile));
            };
            let path = canonical.join(name);
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| {
                let reason = if e.kind() == io::ErrorKind::NotFound {
                    DeleteRefusal::Missing
                } else {
                    failed(&e)
                };
                DeleteError::new(file, reason)
            })?;
            if !metadata.is_file() {
                return Err(DeleteError::new(file, DeleteRefusal::NotAFile));
            }
            if units(&path) >= MAX_PATH_UNITS {
                return Err(DeleteError::new(file, DeleteRefusal::PathTooLong));
            }
            if lock_check {
                open_elsewhere(&path).map_err(|reason| DeleteError::new(file, reason))?;
            }
            total = total.saturating_add(metadata.len());
            prepared.push(Prepared {
                given: file.clone(),
                normal: path,
            });
        }

        if total > capacity {
            return Err(DeleteError::new(
                first,
                DeleteRefusal::TooLarge {
                    bytes: total,
                    capacity,
                },
            ));
        }
        Ok((prepared, total))
    }

    pub(super) fn delete_group(files: &[PathBuf]) -> Result<(), DeleteError> {
        let (prepared, _) = inspect(files, true)?;
        if prepared.is_empty() {
            return Ok(());
        }
        recycle(&prepared)
    }

    /// Sends `group` to the Recycle Bin in one `SHFileOperationW` call, without any check first,
    /// and looks at every file afterwards.
    pub(super) fn recycle(group: &[Prepared]) -> Result<(), DeleteError> {
        use windows_sys::Win32::UI::Shell::{
            FO_DELETE, FOF_ALLOWUNDO, FOF_NO_UI, SHFILEOPSTRUCTW, SHFileOperationW,
        };

        let Some(first) = group.first() else {
            return Ok(());
        };
        // Each path ended by a NUL, the list by a second NUL.
        let mut from: Vec<u16> = Vec::new();
        for file in group {
            let mut path_w =
                wide(&file.normal).map_err(|reason| DeleteError::new(&file.given, reason))?;
            from.append(&mut path_w);
        }
        from.push(0);
        let flags = FOF_ALLOWUNDO | FOF_NO_UI;
        let flags = u16::try_from(flags).map_err(|_| {
            DeleteError::new(
                &first.given,
                DeleteRefusal::Failed(format!("the operation's flags {flags:#x} do not fit")),
            )
        })?;
        let mut operation = SHFILEOPSTRUCTW {
            hwnd: std::ptr::null_mut(),
            wFunc: FO_DELETE,
            pFrom: from.as_ptr(),
            pTo: std::ptr::null(),
            fFlags: flags,
            fAnyOperationsAborted: 0,
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: std::ptr::null(),
        };
        // SAFETY: `operation` is a valid `SHFILEOPSTRUCTW` that lives for the call; `pFrom` points
        // to `from`, a list of NUL-terminated UTF-16 paths ended by a second NUL, which lives
        // until after the call; `pTo`, `hwnd`, the progress title and the name mappings are null,
        // which `FO_DELETE` without `FOF_WANTMAPPINGHANDLE` allows.
        let code = unsafe { SHFileOperationW(&mut operation) };
        let aborted = operation.fAnyOperationsAborted != 0;

        let (remaining, gone): (Vec<&Prepared>, Vec<&Prepared>) =
            group.iter().partition(|file| !is_gone(&file.normal));
        let Some(first_remaining) = remaining.first() else {
            return Ok(());
        };
        let reason = if code == 0 && aborted {
            "Windows stopped the operation".to_string()
        } else {
            format!("Windows could not move the files (code {code:#x})")
        };
        let given = |files: &[&Prepared]| files.iter().map(|f| f.given.clone()).collect();
        let (left, deleted) = if gone.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            (given(&remaining), given(&gone))
        };
        Err(DeleteError {
            file: first_remaining.given.clone(),
            reason: DeleteRefusal::Failed(reason),
            left,
            deleted,
        })
    }

    /// True when `path` is known to be gone; any other result, an error included, counts as
    /// still there.
    fn is_gone(path: &Path) -> bool {
        matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound)
    }

    /// `Err(OpenElsewhere)` when another program holds `path` open: opening it for reading
    /// without sharing fails with a sharing or lock violation. Opening for reading only, so a
    /// read-only file is not refused.
    fn open_elsewhere(path: &Path) -> Result<(), DeleteRefusal> {
        use std::os::windows::fs::OpenOptionsExt;

        match std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
        {
            Ok(_) => Ok(()),
            Err(e) if is_code(&e, ERROR_SHARING_VIOLATION) || is_code(&e, ERROR_LOCK_VIOLATION) => {
                Err(DeleteRefusal::OpenElsewhere)
            }
            Err(e) => Err(failed(&e)),
        }
    }

    /// The volume root of `path` (`GetVolumePathNameW`), with its trailing backslash.
    pub(super) fn volume_root(path: &Path) -> Result<PathBuf, DeleteRefusal> {
        use std::os::windows::ffi::OsStringExt;

        let path_w = wide(path)?;
        // The volume path is never longer than the path itself plus a trailing backslash.
        let capacity = path_w
            .len()
            .saturating_add(2)
            .max(MAX_PATH_UNITS.saturating_add(1));
        let capacity_u32 = u32::try_from(capacity)
            .map_err(|_| DeleteRefusal::Failed(format!("path too long: {}", path.display())))?;
        let mut root = vec![0u16; capacity];
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
            return Err(failed(&io::Error::last_os_error()));
        }
        let end = root
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(root.len());
        let units = root.get(..end).unwrap_or_default();
        Ok(PathBuf::from(std::ffi::OsString::from_wide(units)))
    }

    /// The volume name (`\\?\Volume{GUID}\`) of the volume mounted at `root_w`, a NUL-terminated
    /// root with its trailing backslash; `None` when Windows gives none.
    fn volume_name(root_w: &[u16]) -> Option<String> {
        // A volume name is 49 units with its NUL; room to spare.
        let mut name = [0u16; 64];
        let length = u32::try_from(name.len()).ok()?;
        // SAFETY: `root_w` is NUL-terminated UTF-16 that lives for the call; `name` is a writable
        // buffer of exactly `length` UTF-16 units.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW(
                root_w.as_ptr(),
                name.as_mut_ptr(),
                length,
            )
        };
        if ok == 0 {
            return None;
        }
        let end = name.iter().position(|&unit| unit == 0)?;
        String::from_utf16(name.get(..end)?).ok()
    }

    /// `Ok(true)` when `NoRecycleFiles` is not 0 in `HKEY_CURRENT_USER` or `HKEY_LOCAL_MACHINE`;
    /// a missing key or value counts as 0; any other error is `Err` with its text.
    fn read_policy() -> Result<bool, String> {
        use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

        let values = [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].map(|hive| {
            match read_dword(hive, POLICY_KEY, "NoRecycleFiles") {
                Ok(value) => Ok(value),
                Err(code) if code == ERROR_FILE_NOT_FOUND => Ok(0),
                Err(code) => Err(error_text(code)),
            }
        });
        if values.iter().any(|value| matches!(value, Ok(v) if *v != 0)) {
            return Ok(true);
        }
        for value in values {
            value?;
        }
        Ok(false)
    }

    /// `NukeOnDelete` and `MaxCapacity` of the volume `guid` (braces included); a missing key or
    /// value, or any other error, is `Err` with the Windows error's text.
    fn read_settings(guid: &str) -> Result<BinSettings, String> {
        use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;

        let key = format!(r"{BIT_BUCKET_KEY}\{guid}");
        let nuke_on_delete =
            read_dword(HKEY_CURRENT_USER, &key, "NukeOnDelete").map_err(error_text)?;
        let max_capacity =
            read_dword(HKEY_CURRENT_USER, &key, "MaxCapacity").map_err(error_text)?;
        Ok(BinSettings {
            nuke_on_delete,
            max_capacity,
        })
    }

    /// The DWORD `value` of `hive\key` (`RegGetValueW` with `RRF_RT_REG_DWORD`), or the Windows
    /// error code.
    fn read_dword(
        hive: windows_sys::Win32::System::Registry::HKEY,
        key: &str,
        value: &str,
    ) -> Result<u32, u32> {
        use windows_sys::Win32::Foundation::ERROR_SUCCESS;
        use windows_sys::Win32::System::Registry::{RRF_RT_REG_DWORD, RegGetValueW};

        let key_w = wide_str(key);
        let value_w = wide_str(value);
        let mut data: u32 = 0;
        let mut size = u32::try_from(std::mem::size_of::<u32>()).unwrap_or(4);
        // SAFETY: `key_w` and `value_w` are NUL-terminated UTF-16 strings that live for the call;
        // `data` is a writable `u32` and `size` holds its size in bytes; the type out-pointer may
        // be null; `hive` is a predefined key.
        let code = unsafe {
            RegGetValueW(
                hive,
                key_w.as_ptr(),
                value_w.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                std::ptr::addr_of_mut!(data).cast(),
                &mut size,
            )
        };
        if code == ERROR_SUCCESS {
            Ok(data)
        } else {
            Err(code)
        }
    }

    /// The text of a Windows error code.
    fn error_text(code: u32) -> String {
        match i32::try_from(code) {
            Ok(code) => io::Error::from_raw_os_error(code).to_string(),
            Err(_) => format!("Windows error {code:#x}"),
        }
    }

    /// `Failed` with an I/O error's own text.
    fn failed(error: &io::Error) -> DeleteRefusal {
        DeleteRefusal::Failed(error.to_string())
    }

    fn is_code(error: &io::Error, code: u32) -> bool {
        error.raw_os_error() == i32::try_from(code).ok()
    }

    /// The length of `path` in UTF-16 units.
    fn units(path: &Path) -> usize {
        use std::os::windows::ffi::OsStrExt;

        path.as_os_str().encode_wide().count()
    }

    /// `path` as a NUL-terminated UTF-16 string for a Windows call. A path with a NUL in it is
    /// refused (Windows would read only the part before it).
    fn wide(path: &Path) -> Result<Vec<u16>, DeleteRefusal> {
        use std::os::windows::ffi::OsStrExt;

        let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
        if units.contains(&0) {
            return Err(DeleteRefusal::Failed(format!(
                "the path contains a NUL character: {}",
                path.display()
            )));
        }
        units.push(0);
        Ok(units)
    }

    /// `text` as a NUL-terminated UTF-16 string (a fixed name without NUL).
    fn wide_str(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `path` in lower case, for comparing paths without regard to case.
    fn lower(path: &Path) -> String {
        path.to_string_lossy().to_lowercase()
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};

    use super::{DeleteError, DeleteRefusal, Prepared};

    pub(super) fn drive_allows_delete(_path: &Path) -> Result<(), DeleteRefusal> {
        Err(DeleteRefusal::Unsupported)
    }

    pub(super) fn inspect(
        files: &[PathBuf],
        _lock_check: bool,
    ) -> Result<(Vec<Prepared>, u64), DeleteError> {
        Err(unsupported(files))
    }

    pub(super) fn delete_group(files: &[PathBuf]) -> Result<(), DeleteError> {
        Err(unsupported(files))
    }

    /// `Unsupported`, naming the first file, or an empty path for an empty list.
    fn unsupported(files: &[PathBuf]) -> DeleteError {
        let file = files.first().cloned().unwrap_or_default();
        DeleteError::new(&file, DeleteRefusal::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(nuke_on_delete: u32, max_capacity: u32) -> Result<BinSettings, String> {
        Ok(BinSettings {
            nuke_on_delete,
            max_capacity,
        })
    }

    const GUID: Option<&str> = Some("{0c2d6e5a-1b3f-4a77-9d1e-2f5c8b7a6e40}");

    #[test]
    fn decide_refuses_a_removable_drive_as_a_card() {
        assert_eq!(
            decide(DRIVE_REMOVABLE, false, GUID, Ok(false), settings(0, 100)),
            Err(DeleteRefusal::Card)
        );
    }

    #[test]
    fn decide_refuses_a_fixed_drive_with_dcim_at_the_root_as_a_card() {
        assert_eq!(
            decide(DRIVE_FIXED, true, GUID, Ok(false), settings(0, 100)),
            Err(DeleteRefusal::Card)
        );
    }

    #[test]
    fn decide_refuses_other_drive_types_as_without_a_recycle_bin() {
        for drive_type in [
            DRIVE_REMOTE,
            DRIVE_CDROM,
            DRIVE_RAMDISK,
            DRIVE_UNKNOWN,
            DRIVE_NO_ROOT_DIR,
        ] {
            assert_eq!(
                decide(drive_type, false, GUID, Ok(false), settings(0, 100)),
                Err(DeleteRefusal::NoRecycleBin),
                "drive type {drive_type}"
            );
        }
    }

    #[test]
    fn decide_refuses_a_fixed_drive_without_a_volume_guid() {
        assert_eq!(
            decide(DRIVE_FIXED, false, None, Ok(false), settings(0, 100)),
            Err(DeleteRefusal::NoRecycleBin)
        );
    }

    #[test]
    fn decide_refuses_when_the_policy_turns_the_bin_off() {
        assert_eq!(
            decide(DRIVE_FIXED, false, GUID, Ok(true), settings(0, 100)),
            Err(DeleteRefusal::RecycleBinOff)
        );
    }

    #[test]
    fn decide_refuses_when_the_policy_could_not_be_read() {
        assert_eq!(
            decide(
                DRIVE_FIXED,
                false,
                GUID,
                Err("Access is denied. (os error 5)".to_string()),
                settings(0, 100)
            ),
            Err(DeleteRefusal::RecycleBinUnknown(
                "Access is denied. (os error 5)".to_string()
            ))
        );
    }

    #[test]
    fn decide_refuses_nuke_on_delete() {
        assert_eq!(
            decide(DRIVE_FIXED, false, GUID, Ok(false), settings(1, 100)),
            Err(DeleteRefusal::RecycleBinOff)
        );
    }

    #[test]
    fn decide_refuses_settings_that_could_not_be_read_with_the_text() {
        let text = "The system cannot find the file specified. (os error 2)";
        assert_eq!(
            decide(DRIVE_FIXED, false, GUID, Ok(false), Err(text.to_string())),
            Err(DeleteRefusal::RecycleBinUnknown(text.to_string()))
        );
    }

    #[test]
    fn decide_gives_the_capacity_in_bytes_for_a_fixed_drive_with_the_bin_on() {
        assert_eq!(
            decide(DRIVE_FIXED, false, GUID, Ok(false), settings(0, 100)),
            Ok(100 * 1_048_576)
        );
        assert_eq!(
            decide(DRIVE_FIXED, false, GUID, Ok(false), settings(0, u32::MAX)),
            Ok(u64::from(u32::MAX) * 1_048_576)
        );
    }

    #[test]
    fn guid_of_takes_the_braced_part_of_a_volume_name() {
        assert_eq!(
            guid_of(r"\\?\Volume{0c2d6e5a-1b3f-4a77-9d1e-2f5c8b7a6e40}\"),
            Some("{0c2d6e5a-1b3f-4a77-9d1e-2f5c8b7a6e40}")
        );
        assert_eq!(guid_of(r"\\?\Volume\"), None);
        assert_eq!(guid_of(r"\\?\Volume{\"), None);
        assert_eq!(guid_of(r"\\?\Volume{}\"), None);
    }

    #[test]
    fn every_refusal_is_shown_as_its_reason() {
        let cases = [
            (DeleteRefusal::Card, "the drive is a memory card"),
            (DeleteRefusal::NoRecycleBin, "the drive has no Recycle Bin"),
            (
                DeleteRefusal::RecycleBinOff,
                "the Recycle Bin is turned off for the drive",
            ),
            (
                DeleteRefusal::TooLarge {
                    bytes: 2_000,
                    capacity: 1_000,
                },
                "the files are too large for the Recycle Bin (2000 bytes, the Recycle Bin holds \
                 1000)",
            ),
            (
                DeleteRefusal::RecycleBinUnknown("access denied".to_string()),
                "the Recycle Bin's settings for the drive could not be read: access denied",
            ),
            (DeleteRefusal::Missing, "the file does not exist"),
            (DeleteRefusal::NotAFile, "it is not a file"),
            (
                DeleteRefusal::OpenElsewhere,
                "the file is open in another program",
            ),
            (
                DeleteRefusal::NotOneFolder,
                "the files are not in one folder",
            ),
            (
                DeleteRefusal::PathTooLong,
                "the path is too long for the Recycle Bin",
            ),
            (
                DeleteRefusal::Unsupported,
                "deleting to the Recycle Bin is only available on Windows",
            ),
            (
                DeleteRefusal::Failed("the file is listed twice".to_string()),
                "the file is listed twice",
            ),
        ];
        for (refusal, text) in cases {
            assert_eq!(refusal.to_string(), text);
        }
    }

    #[test]
    fn the_locked_reason_has_the_words_of_a_move() {
        assert_eq!(
            DeleteRefusal::OpenElsewhere.to_string(),
            "the file is open in another program"
        );
    }

    #[test]
    fn delete_error_is_one_sentence_naming_the_file() {
        let error = DeleteError {
            file: PathBuf::from("a").join("TN0001.ARW"),
            reason: DeleteRefusal::Card,
            left: Vec::new(),
            deleted: Vec::new(),
        };
        assert_eq!(
            error.to_string(),
            "Could not move TN0001.ARW to the Recycle Bin: the drive is a memory card."
        );
    }

    #[test]
    fn delete_error_names_the_files_left_in_full() {
        let left = [
            PathBuf::from("a").join("TN0001.ARW.dop"),
            PathBuf::from("a").join("TN0001.XMP"),
        ];
        let error = DeleteError {
            file: left[0].clone(),
            reason: DeleteRefusal::Failed("Windows could not move the files (code 0x20)".into()),
            left: left.to_vec(),
            deleted: vec![PathBuf::from("a").join("TN0001.ARW")],
        };
        assert_eq!(
            error.to_string(),
            format!(
                "Could not move TN0001.ARW.dop to the Recycle Bin: Windows could not move the \
                 files (code 0x20). Some files are already in the Recycle Bin; these were left: \
                 {}, {}.",
                left[0].display(),
                left[1].display()
            )
        );
    }

    #[test]
    #[cfg(not(windows))]
    fn off_windows_everything_is_unsupported_and_nothing_is_touched() {
        let file = PathBuf::from("TN0001.ARW");
        assert_eq!(
            drive_allows_delete(Path::new(".")),
            Err(DeleteRefusal::Unsupported)
        );
        for result in [
            check_group(std::slice::from_ref(&file)),
            inspect_group(std::slice::from_ref(&file)),
        ] {
            let error = result.unwrap_err();
            assert_eq!(error.file, file);
            assert_eq!(error.reason, DeleteRefusal::Unsupported);
        }
        let error = delete_group(&[]).unwrap_err();
        assert_eq!(error.file, PathBuf::new());
        assert_eq!(error.reason, DeleteRefusal::Unsupported);
    }

    #[cfg(windows)]
    mod windows {
        use super::super::imp;
        use super::super::*;
        use crate::fileops;
        use std::os::windows::fs::OpenOptionsExt;

        // FILE_SHARE_READ | FILE_SHARE_WRITE, without FILE_SHARE_DELETE.
        const SHARE_READ_WRITE: u32 = 0x1 | 0x2;

        /// A fresh folder under the system temporary folder, removed when dropped.
        struct TempFolder(PathBuf);

        impl TempFolder {
            fn new(name: &str) -> Self {
                let path = std::env::temp_dir().join(format!(
                    "pw-metadata-recycle-{name}-{}-{:016x}",
                    std::process::id(),
                    fileops::unique_number()
                ));
                std::fs::create_dir_all(&path).unwrap();
                TempFolder(path)
            }

            fn file(&self, name: &str) -> PathBuf {
                let path = self.0.join(name);
                std::fs::write(&path, name.as_bytes()).unwrap();
                path
            }
        }

        impl Drop for TempFolder {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn group(dir: &TempFolder) -> Vec<PathBuf> {
            vec![
                dir.file("TN0020.ARW"),
                dir.file("TN0020.ARW.dop"),
                dir.file("TN0020.XMP"),
            ]
        }

        fn size_of(files: &[PathBuf]) -> u64 {
            files
                .iter()
                .map(|f| std::fs::metadata(f).unwrap().len())
                .sum()
        }

        fn all_exist(files: &[PathBuf]) -> bool {
            files.iter().all(|f| f.is_file())
        }

        fn set_read_only(path: &Path, read_only: bool) {
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_readonly(read_only);
            std::fs::set_permissions(path, permissions).unwrap();
        }

        #[test]
        fn the_temporary_folder_allows_a_delete() {
            let dir = TempFolder::new("drive");
            let result = drive_allows_delete(&dir.0);
            assert!(
                result.is_ok(),
                "the temporary folder {} is refused: {}",
                dir.0.display(),
                result.unwrap_err()
            );
        }

        #[test]
        fn check_group_gives_the_total_size_of_three_files() {
            let dir = TempFolder::new("total");
            let files = group(&dir);
            let total = check_group(&files).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(total, size_of(&files));
            assert_eq!(total, 10 + 14 + 10);
            assert!(all_exist(&files));
        }

        #[test]
        fn an_empty_group_is_nothing_to_check() {
            assert_eq!(check_group(&[]), Ok(0));
            assert_eq!(inspect_group(&[]), Ok(0));
            assert_eq!(delete_group(&[]), Ok(()));
        }

        #[test]
        fn a_missing_file_is_named() {
            let dir = TempFolder::new("missing");
            let files = group(&dir);
            let missing = dir.0.join("TN0020.JPG");
            let list = vec![files[0].clone(), missing.clone(), files[2].clone()];
            let error = check_group(&list).unwrap_err();
            assert_eq!(error, DeleteError::new(&missing, DeleteRefusal::Missing));
            assert!(all_exist(&files));
        }

        #[test]
        fn a_folder_in_the_group_is_not_a_file() {
            let dir = TempFolder::new("folder");
            let files = group(&dir);
            let folder = dir.0.join("TN0020.dop");
            std::fs::create_dir(&folder).unwrap();
            let list = vec![files[0].clone(), folder.clone(), files[1].clone()];
            let error = check_group(&list).unwrap_err();
            assert_eq!(error, DeleteError::new(&folder, DeleteRefusal::NotAFile));
            assert!(all_exist(&files));
            assert!(folder.is_dir());
        }

        #[test]
        fn files_from_two_folders_are_refused() {
            let dir = TempFolder::new("two-folders");
            let files = group(&dir);
            std::fs::create_dir(dir.0.join("other")).unwrap();
            let other = dir.file(r"other\TN0020.xmp");
            let list = vec![files[0].clone(), other.clone(), files[1].clone()];
            let error = check_group(&list).unwrap_err();
            assert_eq!(error, DeleteError::new(&other, DeleteRefusal::NotOneFolder));
            assert!(all_exist(&files) && other.is_file());
        }

        #[test]
        fn a_file_held_open_is_refused_by_check_but_not_by_inspect() {
            let dir = TempFolder::new("open");
            let files = group(&dir);
            let held = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(SHARE_READ_WRITE)
                .open(&files[1])
                .unwrap();
            let checked = check_group(&files);
            let inspected = inspect_group(&files);
            drop(held);
            assert_eq!(
                checked,
                Err(DeleteError::new(&files[1], DeleteRefusal::OpenElsewhere))
            );
            assert_eq!(inspected, Ok(size_of(&files)));
            assert!(all_exist(&files));
        }

        #[test]
        fn a_read_only_file_passes() {
            let dir = TempFolder::new("read-only");
            let files = group(&dir);
            set_read_only(&files[0], true);
            let result = check_group(&files);
            set_read_only(&files[0], false);
            assert_eq!(result, Ok(size_of(&files)));
        }

        #[test]
        fn the_same_file_twice_is_refused() {
            let dir = TempFolder::new("twice");
            let files = group(&dir);
            let again = dir.0.join("tn0020.arw");
            let list = vec![files[0].clone(), files[1].clone(), again.clone()];
            let error = check_group(&list).unwrap_err();
            assert_eq!(
                error,
                DeleteError::new(
                    &again,
                    DeleteRefusal::Failed("the file is listed twice".to_string())
                )
            );
            assert_eq!(error.reason.to_string(), "the file is listed twice");
            assert!(all_exist(&files));
        }

        #[test]
        fn a_verbatim_path_is_accepted_in_the_normal_form() {
            let dir = TempFolder::new("verbatim");
            let files = group(&dir);
            let plain = std::fs::canonicalize(&files[0]).unwrap();
            let plain = imp::normal_form(&plain).unwrap();
            let verbatim = PathBuf::from(format!(r"\\?\{}", plain.display()));
            assert_eq!(imp::normal_form(&verbatim), Ok(plain.clone()));

            let list = [verbatim.clone()];
            assert_eq!(check_group(&list), Ok(size_of(&files[..1])));
            let (prepared, _) = imp::inspect(&list, true).unwrap();
            assert_eq!(
                prepared,
                [Prepared {
                    given: verbatim,
                    normal: plain,
                }]
            );
        }

        #[test]
        fn other_verbatim_and_device_forms_have_no_recycle_bin() {
            for path in [
                r"\\?\UNC\server\share\TN0020.ARW",
                r"\\?\Volume{0c2d6e5a-1b3f-4a77-9d1e-2f5c8b7a6e40}\TN0020.ARW",
                r"\\.\C:\TN0020.ARW",
            ] {
                assert_eq!(
                    imp::normal_form(Path::new(path)),
                    Err(DeleteRefusal::NoRecycleBin),
                    "{path}"
                );
            }
        }

        // The path resolves against the working folder, whose drive is judged first, so the reason
        // depends on the machine (CI's D: had no Recycle Bin settings, 2026-11-20); the file's name
        // is what is tested.
        #[test]
        fn a_relative_path_to_a_missing_file_is_named_as_given() {
            let relative = PathBuf::from("pw-metadata-recycle-no-such-file.ARW");
            let error = check_group(std::slice::from_ref(&relative)).unwrap_err();
            assert_eq!(error.file, relative);
            assert!(error.left.is_empty(), "{error:?}");
            assert!(error.deleted.is_empty(), "{error:?}");
            assert!(
                matches!(
                    error.reason,
                    DeleteRefusal::Missing
                        | DeleteRefusal::RecycleBinUnknown(_)
                        | DeleteRefusal::NoRecycleBin
                        | DeleteRefusal::Card
                        | DeleteRefusal::RecycleBinOff
                ),
                "unexpected refusal: {error:?}"
            );
        }

        #[test]
        fn a_folder_reached_through_a_junction_is_judged_by_its_target() {
            let dir = TempFolder::new("junction-target");
            let files = group(&dir);
            let links = TempFolder::new("junction");
            let link = links.0.join("link");
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&dir.0)
                .output()
                .unwrap();
            assert!(
                status.status.success(),
                "mklink /J failed: {}",
                String::from_utf8_lossy(&status.stdout)
            );
            let through: Vec<PathBuf> = files
                .iter()
                .map(|f| link.join(f.file_name().unwrap()))
                .collect();

            let target_drive = drive_allows_delete(&dir.0);
            let link_drive = drive_allows_delete(&link);
            let checked = check_group(&through);
            let (prepared, _) = imp::inspect(&through, true).unwrap();
            let target = imp::normal_form(&std::fs::canonicalize(&dir.0).unwrap()).unwrap();
            let _ = std::fs::remove_dir(&link);

            assert_eq!(link_drive, target_drive);
            assert_eq!(checked, Ok(size_of(&files)));
            // The call would get the target's paths; the error would name the caller's.
            for (file, prepared) in through.iter().zip(&prepared) {
                assert_eq!(&prepared.given, file);
                assert_eq!(prepared.normal, target.join(file.file_name().unwrap()));
            }
        }

        /// Runs `work` on a thread of its own and waits up to 30 seconds for its answer.
        fn within_30_seconds<T: Send + 'static>(
            work: impl FnOnce() -> T + Send + 'static,
        ) -> Option<T> {
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = sender.send(work());
            });
            receiver
                .recv_timeout(std::time::Duration::from_secs(30))
                .ok()
        }

        /// The number of items in the Recycle Bin of the drive of `path`.
        fn items_in_bin(path: &Path) -> i64 {
            use windows_sys::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};

            let root = imp::volume_root(path).unwrap();
            let root_w: Vec<u16> = root
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut info = SHQUERYRBINFO {
                cbSize: u32::try_from(std::mem::size_of::<SHQUERYRBINFO>()).unwrap(),
                i64Size: 0,
                i64NumItems: 0,
            };
            // SAFETY: `root_w` is NUL-terminated UTF-16; `info` is writable with `cbSize` set.
            let result = unsafe { SHQueryRecycleBinW(root_w.as_ptr(), &mut info) };
            assert_eq!(result, 0, "SHQueryRecycleBinW failed: {result:#x}");
            info.i64NumItems
        }

        #[test]
        #[ignore = "puts three small files in the real Recycle Bin (B-Q15); run by hand"]
        fn delete_group_puts_the_three_files_in_the_recycle_bin() {
            let dir = TempFolder::new("real");
            let files = vec![
                dir.file("pw-metadata-recycle-test-TN0030.ARW"),
                dir.file("pw-metadata-recycle-test-TN0030.ARW.dop"),
                dir.file("pw-metadata-recycle-test-TN0030.XMP"),
            ];
            let before = items_in_bin(&dir.0);
            let list = files.clone();
            let result = within_30_seconds(move || delete_group(&list)).unwrap_or_else(|| {
                panic!("no answer within 30 seconds: a Windows dialog may be open")
            });
            assert_eq!(result, Ok(()));
            assert!(files.iter().all(|f| !f.exists()));
            let after = items_in_bin(&dir.0);
            assert!(
                after >= before + 3,
                "the Recycle Bin held {before} items before and {after} after"
            );
        }

        #[test]
        #[ignore = "puts small files in the real Recycle Bin (B-Q15); run by hand"]
        fn a_held_file_stops_the_windows_call_and_left_holds_what_still_exists() {
            let dir = TempFolder::new("partial");
            let files = vec![
                dir.file("pw-metadata-recycle-test-TN0031.ARW"),
                dir.file("pw-metadata-recycle-test-TN0031.ARW.dop"),
                dir.file("pw-metadata-recycle-test-TN0031.XMP"),
            ];
            let held = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(SHARE_READ_WRITE)
                .open(&files[2])
                .unwrap();
            let prepared: Vec<Prepared> = files
                .iter()
                .map(|f| Prepared {
                    given: f.clone(),
                    normal: f.clone(),
                })
                .collect();
            let Some(result) = within_30_seconds(move || imp::recycle(&prepared)) else {
                // The stuck thread and the held handle are left alone until the process ends.
                std::mem::forget(held);
                std::mem::forget(dir);
                panic!("no answer within 30 seconds: a Windows dialog may be open");
            };
            drop(held);

            let error = result.unwrap_err();
            assert!(
                matches!(error.reason, DeleteRefusal::Failed(_)),
                "{error:?}"
            );
            if error.left.is_empty() {
                assert!(all_exist(&files), "{error:?}");
                assert!(error.deleted.is_empty(), "{error:?}");
            } else {
                assert!(error.left.contains(&files[2]), "{error:?}");
                assert!(error.left.contains(&error.file), "{error:?}");
                for file in &files {
                    assert_eq!(file.exists(), error.left.contains(file), "{error:?}");
                    assert_eq!(!file.exists(), error.deleted.contains(file), "{error:?}");
                }
            }
        }
    }
}
