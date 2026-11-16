use std::path::Path;
// Only the Windows eject code and the tests wait for a drive to disappear.
#[cfg(any(windows, test))]
use std::time::{Duration, Instant};

/// How many times an eject is tried before the drive is reported as in use.
#[cfg(any(windows, test))]
const EJECT_ATTEMPTS: usize = 3;

/// Returns the drive letter at the start of `source`, if present.
pub fn drive_of(source: &Path) -> Option<String> {
    let s = source.to_string_lossy();
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic() => {
            Some(format!("{}:", letter.to_ascii_uppercase()))
        }
        _ => None,
    }
}

/// The kind of drive Windows reports, without Windows' own constants.
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DriveKind {
    Removable,
    Fixed,
    Other,
}

/// How a drive is ejected (ADR 0005 choice 3).
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EjectWay {
    /// The shell's "Eject" verb, as File Explorer does.
    Verb,
    /// The USB device is asked to be removed, as Safely Remove Hardware does.
    Device,
    NotEjected,
}

#[cfg(any(windows, test))]
fn eject_way(kind: DriveKind, on_usb: bool) -> EjectWay {
    match kind {
        DriveKind::Removable => EjectWay::Verb,
        DriveKind::Fixed if on_usb => EjectWay::Device,
        DriveKind::Fixed | DriveKind::Other => EjectWay::NotEjected,
    }
}

/// The result of one eject attempt.
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempt {
    /// Windows accepted the request; the drive should go away shortly.
    Started,
    /// Windows refused the request (for example a file on the drive is open).
    Refused,
}

#[cfg(any(windows, test))]
fn ejected_text(drive: &str) -> String {
    format!("{drive} has been ejected and can be removed.")
}

#[cfg(any(windows, test))]
fn device_in_use_text(drive: &str) -> String {
    format!(
        "{drive} could not be ejected (is it in use by another program?). Use Safely Remove Hardware in the taskbar. Attempts: {EJECT_ATTEMPTS}."
    )
}

#[cfg(any(windows, test))]
fn not_ejected_text(drive: &str) -> String {
    format!("{drive} was not ejected: it is neither a removable drive nor a USB drive.")
}

#[cfg(any(windows, test))]
fn windows_error_text(drive: &str, code: u32) -> String {
    format!("{drive} could not be ejected: Windows reported error {code}.")
}

/// PowerShell that ejects `drive` and stays alive until `<drive>\DCIM` is gone (at most 5 s):
/// the eject verb runs in the background and is cancelled if PowerShell exits at once.
#[cfg(any(windows, test))]
fn eject_script(drive: &str) -> String {
    format!(
        "$d = (New-Object -ComObject Shell.Application).Namespace(17).ParseName('{drive}'); \
         $d.InvokeVerb('Eject'); \
         for ($i = 0; $i -lt 20 -and (Test-Path '{drive}\\DCIM'); $i++) {{ \
             Start-Sleep -Milliseconds 250 \
         }}"
    )
}

#[cfg(windows)]
const EJECT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(windows)]
const EJECT_POLL_INTERVAL: Duration = Duration::from_millis(250);
#[cfg(windows)]
const PAUSE_AFTER_REFUSAL: Duration = Duration::from_secs(1);

/// Ejects the drive containing `source` on Windows.
pub fn eject(source: &Path) -> String {
    let Some(drive) = drive_of(source) else {
        return "The source is not a drive, so nothing was ejected.".into();
    };
    #[cfg(windows)]
    {
        let kind = device::drive_kind(&drive);
        let on_usb = kind == DriveKind::Fixed && device::on_usb(&drive);
        match eject_way(kind, on_usb) {
            EjectWay::Verb => eject_with_verb(&drive),
            EjectWay::Device => eject_through_device(&drive),
            EjectWay::NotEjected => not_ejected_text(&drive),
        }
    }
    #[cfg(not(windows))]
    {
        format!("Ejecting {drive} is only supported on Windows.")
    }
}

/// The shell's "Eject" verb through PowerShell, for a removable drive.
#[cfg(windows)]
fn eject_with_verb(drive: &str) -> String {
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Some(system_root) = std::env::var_os("SystemRoot") else {
        return "The eject could not be started: the environment variable SystemRoot is missing."
            .into();
    };
    let powershell = PathBuf::from(system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let script = eject_script(drive);
    let dcim = PathBuf::from(format!("{drive}\\DCIM"));
    match retry_until_gone(
        EJECT_ATTEMPTS,
        || {
            let status = std::process::Command::new(&powershell)
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|error| format!("{drive} could not be ejected: {error}."))?;
            if !status.success() {
                return Err(format!(
                    "{drive} could not be ejected: PowerShell ended with an error ({status})."
                ));
            }
            Ok(())
        },
        || dcim.exists(),
        EJECT_TIMEOUT,
        EJECT_POLL_INTERVAL,
    ) {
        Ok(true) => ejected_text(drive),
        Ok(false) => format!(
            "{drive} could not be ejected (is it in use by another program?). Eject it in File Explorer. Attempts: {EJECT_ATTEMPTS}."
        ),
        Err(error) => error,
    }
}

/// The USB device behind a fixed drive is asked to be removed, as Safely Remove Hardware does.
/// The device is the whole reader, so a reader with several slots loses all of them.
#[cfg(windows)]
fn eject_through_device(drive: &str) -> String {
    let reader = match device::reader_device(drive) {
        Ok(reader) => reader,
        Err(code) => return windows_error_text(drive, code),
    };
    let dcim = std::path::PathBuf::from(format!("{drive}\\DCIM"));
    match retry_until_gone_or_refused(
        EJECT_ATTEMPTS,
        || Ok(device::request_eject(reader)),
        || dcim.exists(),
        EJECT_TIMEOUT,
        EJECT_POLL_INTERVAL,
        PAUSE_AFTER_REFUSAL,
    ) {
        Ok(true) => ejected_text(drive),
        Ok(false) => device_in_use_text(drive),
        Err(error) => error,
    }
}

/// Splits a `\0`-separated, `\0\0`-ended list of UTF-16 strings. An empty or unended list gives
/// the entries it holds.
#[cfg(any(windows, test))]
fn split_multi_sz(list: &[u16]) -> Vec<Vec<u16>> {
    list.split(|&unit| unit == 0)
        .take_while(|entry| !entry.is_empty())
        .map(<[u16]>::to_vec)
        .collect()
}

/// The index of the first interface whose (device type, device number) is `wanted`.
#[cfg(any(windows, test))]
fn matching_interface(
    interfaces: &[Vec<u16>],
    wanted: (u32, u32),
    mut number_of: impl FnMut(&[u16]) -> Option<(u32, u32)>,
) -> Option<usize> {
    interfaces
        .iter()
        .position(|interface| number_of(interface) == Some(wanted))
}

#[cfg(any(windows, test))]
fn wait_until_gone(
    mut still_present: impl FnMut() -> bool,
    timeout: Duration,
    interval: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !still_present() {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        std::thread::sleep(interval.min(deadline - now));
    }
}

#[cfg(any(windows, test))]
fn retry_until_gone(
    attempts: usize,
    mut invoke: impl FnMut() -> Result<(), String>,
    still_present: impl FnMut() -> bool,
    timeout: Duration,
    interval: Duration,
) -> Result<bool, String> {
    retry_until_gone_or_refused(
        attempts,
        || invoke().map(|()| Attempt::Started),
        still_present,
        timeout,
        interval,
        Duration::ZERO,
    )
}

/// Tries at most `attempts` times. A started attempt waits for the drive to go away; a refused
/// one does not wait for it, but pauses before the next attempt (not after the last). An error
/// stops at once. `Ok(false)` means every attempt was refused or the drive stayed.
#[cfg(any(windows, test))]
fn retry_until_gone_or_refused(
    attempts: usize,
    mut invoke: impl FnMut() -> Result<Attempt, String>,
    mut still_present: impl FnMut() -> bool,
    timeout: Duration,
    interval: Duration,
    pause_after_refusal: Duration,
) -> Result<bool, String> {
    for attempt in 1..=attempts {
        match invoke()? {
            Attempt::Started => {
                if wait_until_gone(&mut still_present, timeout, interval) {
                    return Ok(true);
                }
            }
            Attempt::Refused => {
                if attempt < attempts {
                    std::thread::sleep(pause_after_refusal);
                }
            }
        }
    }
    Ok(false)
}

/// The Windows calls of the Device way. Errors are the `GetLastError` value or the `CR_*` value.
#[cfg(windows)]
mod device {
    use super::{Attempt, DriveKind, matching_interface, split_multi_sz};
    use std::ffi::{OsString, c_void};
    use std::fs::{File, OpenOptions};
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW,
        CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_PropertyW, CM_Get_Parent,
        CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CM_Request_Device_EjectW, CONFIGRET,
        CR_BUFFER_SMALL, CR_FAILURE, CR_SUCCESS, PNP_VETO_TYPE, PNP_VetoTypeUnknown,
    };
    use windows_sys::Win32::Devices::Properties::{
        DEVPKEY_Device_InstanceId, DEVPROP_TYPE_STRING, DEVPROPTYPE,
    };
    use windows_sys::Win32::Foundation::MAX_PATH;
    use windows_sys::Win32::Storage::FileSystem::{
        BusTypeUsb, FILE_SHARE_READ, FILE_SHARE_WRITE, GetDriveTypeW,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::{
        GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY,
        PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR, STORAGE_DEVICE_NUMBER,
        STORAGE_PROPERTY_QUERY, StorageDeviceProperty,
    };
    use windows_sys::Win32::System::WindowsProgramming::{DRIVE_FIXED, DRIVE_REMOVABLE};

    /// How often the interface list is asked for when a disk arrives between the size and the
    /// list.
    const LIST_ATTEMPTS: usize = 3;
    /// UTF-16 units of the veto-name buffer.
    const VETO_NAME_UNITS: usize = MAX_PATH as usize;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The `GetLastError` value of an I/O error, or `CR_FAILURE` if it has none.
    fn os_code(error: &std::io::Error) -> u32 {
        error.raw_os_error().map_or(CR_FAILURE, i32::cast_unsigned)
    }

    /// Opens a volume or a device interface for queries only (no read or write access).
    fn open_for_query(path: &Path) -> Result<File, u32> {
        OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(path)
            .map_err(|error| os_code(&error))
    }

    fn volume_path(drive: &str) -> PathBuf {
        PathBuf::from(format!(r"\\.\{drive}"))
    }

    fn size_u32<T>() -> u32 {
        u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
    }

    pub(super) fn drive_kind(drive: &str) -> DriveKind {
        let root = wide(&format!("{drive}\\"));
        // SAFETY: `root` is a NUL-terminated UTF-16 string that lives for the whole call.
        let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
        match drive_type {
            DRIVE_REMOVABLE => DriveKind::Removable,
            DRIVE_FIXED => DriveKind::Fixed,
            _ => DriveKind::Other,
        }
    }

    /// Whether the disk holding `drive` is on USB. Any failure counts as not on USB, so an
    /// internal disk is never ejected.
    pub(super) fn on_usb(drive: &str) -> bool {
        let Ok(volume) = open_for_query(&volume_path(drive)) else {
            return false;
        };
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceProperty,
            QueryType: PropertyStandardQuery,
            AdditionalParameters: [0],
        };
        let mut descriptor = STORAGE_DEVICE_DESCRIPTOR::default();
        let mut returned = 0u32;
        // SAFETY: the handle is open while `volume` lives; the in and out pointers point to
        // values of exactly the sizes passed, which outlive the synchronous call.
        let ok = unsafe {
            DeviceIoControl(
                volume.as_raw_handle(),
                IOCTL_STORAGE_QUERY_PROPERTY,
                (&raw const query).cast::<c_void>(),
                size_u32::<STORAGE_PROPERTY_QUERY>(),
                (&raw mut descriptor).cast::<c_void>(),
                size_u32::<STORAGE_DEVICE_DESCRIPTOR>(),
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        let filled = std::mem::offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType)
            + size_of_val(&descriptor.BusType);
        ok != 0
            && usize::try_from(returned).is_ok_and(|returned| returned >= filled)
            && descriptor.BusType == BusTypeUsb
    }

    /// The (device type, device number) of an opened volume or disk.
    fn device_number(file: &File) -> Result<(u32, u32), u32> {
        let mut number = STORAGE_DEVICE_NUMBER::default();
        let mut returned = 0u32;
        // SAFETY: the handle is open while `file` lives; the out pointer points to a value of
        // exactly the size passed, which outlives the synchronous call. No input buffer.
        let ok = unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                std::ptr::null(),
                0,
                (&raw mut number).cast::<c_void>(),
                size_u32::<STORAGE_DEVICE_NUMBER>(),
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(os_code(&std::io::Error::last_os_error()));
        }
        Ok((number.DeviceType, number.DeviceNumber))
    }

    /// The present disk interfaces as a `\0`-separated, `\0\0`-ended list.
    fn disk_interface_list() -> Result<Vec<u16>, u32> {
        let mut result: CONFIGRET = CR_BUFFER_SMALL;
        for _ in 0..LIST_ATTEMPTS {
            let mut length = 0u32;
            // SAFETY: `length` is a valid out pointer and the GUID a constant that outlives the
            // call; no device ID is given (null).
            result = unsafe {
                CM_Get_Device_Interface_List_SizeW(
                    &mut length,
                    &GUID_DEVINTERFACE_DISK,
                    std::ptr::null(),
                    CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
                )
            };
            if result != CR_SUCCESS {
                return Err(result);
            }
            let units = usize::try_from(length).map_err(|_| CR_FAILURE)?;
            let mut list = vec![0u16; units];
            // SAFETY: `list` holds exactly `length` UTF-16 units, the length passed, and
            // outlives the call.
            result = unsafe {
                CM_Get_Device_Interface_ListW(
                    &GUID_DEVINTERFACE_DISK,
                    std::ptr::null(),
                    list.as_mut_ptr(),
                    length,
                    CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
                )
            };
            match result {
                CR_SUCCESS => return Ok(list),
                CR_BUFFER_SMALL => continue,
                other => return Err(other),
            }
        }
        Err(result)
    }

    /// The device instance ID (NUL-terminated) of a disk interface.
    fn instance_id(interface: &[u16]) -> Result<Vec<u16>, u32> {
        let path: Vec<u16> = interface
            .iter()
            .copied()
            .chain(std::iter::once(0))
            .collect();
        let mut property_type: DEVPROPTYPE = 0;
        let mut size = 0u32;
        // SAFETY: `path` is NUL-terminated and outlives the call; with size 0 and a null buffer
        // Windows writes no property, it only reports the size in bytes.
        let result = unsafe {
            CM_Get_Device_Interface_PropertyW(
                path.as_ptr(),
                &DEVPKEY_Device_InstanceId,
                &mut property_type,
                std::ptr::null_mut(),
                &mut size,
                0,
            )
        };
        match result {
            CR_BUFFER_SMALL => {}
            CR_SUCCESS => return Err(CR_FAILURE),
            other => return Err(other),
        }
        let units = usize::try_from(size)
            .map_err(|_| CR_FAILURE)?
            .div_ceil(size_of::<u16>());
        let capacity = units
            .checked_mul(size_of::<u16>())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(CR_FAILURE)?;
        let mut id = vec![0u16; units];
        let mut size = capacity;
        // SAFETY: `id` holds `capacity` bytes, the size passed; `path` is NUL-terminated and
        // both outlive the call.
        let result = unsafe {
            CM_Get_Device_Interface_PropertyW(
                path.as_ptr(),
                &DEVPKEY_Device_InstanceId,
                &mut property_type,
                id.as_mut_ptr().cast::<u8>(),
                &mut size,
                0,
            )
        };
        if result != CR_SUCCESS {
            return Err(result);
        }
        if property_type != DEVPROP_TYPE_STRING {
            return Err(CR_FAILURE);
        }
        let end = id.iter().position(|&unit| unit == 0).unwrap_or(id.len());
        id.truncate(end);
        id.push(0);
        Ok(id)
    }

    /// The device node of the USB storage device (the whole reader) behind `drive`.
    pub(super) fn reader_device(drive: &str) -> Result<u32, u32> {
        let volume = open_for_query(&volume_path(drive))?;
        let wanted = device_number(&volume)?;
        let interfaces = split_multi_sz(&disk_interface_list()?);
        let index = matching_interface(&interfaces, wanted, |interface| {
            let path = PathBuf::from(OsString::from_wide(interface));
            open_for_query(&path)
                .and_then(|disk| device_number(&disk))
                .ok()
        })
        .ok_or(CR_FAILURE)?;
        let interface = interfaces.get(index).ok_or(CR_FAILURE)?;
        let id = instance_id(interface)?;
        let mut disk = 0u32;
        // SAFETY: `id` is a NUL-terminated device instance ID that outlives the call; `disk` is
        // a valid out pointer.
        let result =
            unsafe { CM_Locate_DevNodeW(&mut disk, id.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) };
        if result != CR_SUCCESS {
            return Err(result);
        }
        let mut reader = 0u32;
        // SAFETY: `reader` is a valid out pointer and `disk` a device node from Windows.
        let result = unsafe { CM_Get_Parent(&mut reader, disk, 0) };
        if result != CR_SUCCESS {
            return Err(result);
        }
        Ok(reader)
    }

    /// One request to remove the device. The veto type and name are passed so that Windows
    /// shows no dialog of its own; the name is not used.
    pub(super) fn request_eject(device: u32) -> Attempt {
        let mut veto_type: PNP_VETO_TYPE = PNP_VetoTypeUnknown;
        let mut veto_name = [0u16; VETO_NAME_UNITS];
        // SAFETY: `veto_type` is a valid out pointer and `veto_name` holds `MAX_PATH` UTF-16
        // units, the length passed; both outlive the call.
        let result = unsafe {
            CM_Request_Device_EjectW(device, &mut veto_type, veto_name.as_mut_ptr(), MAX_PATH, 0)
        };
        if result == CR_SUCCESS && veto_type == PNP_VetoTypeUnknown {
            Attempt::Started
        } else {
            Attempt::Refused
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eject_script_invokes_eject_and_waits_for_the_requested_drive() {
        let script = eject_script("E:");

        assert!(script.contains("ParseName('E:')"));
        assert!(script.contains("InvokeVerb('Eject')"));
        assert!(script.contains("Test-Path 'E:\\DCIM'"));
        assert!(script.contains("Start-Sleep"));
        assert_eq!(script.matches(':').count(), 2);
    }

    #[test]
    fn drive_letter_is_found_only_at_the_start() {
        assert_eq!(drive_of(Path::new(r"e:\DCIM")).as_deref(), Some("E:"));
        assert_eq!(drive_of(Path::new("F:")).as_deref(), Some("F:"));
        assert_eq!(drive_of(Path::new(r"\\server\share")), None);
        assert_eq!(drive_of(Path::new("/media/card")), None);
        assert_eq!(drive_of(Path::new("1:")), None);
        assert_eq!(drive_of(Path::new("")), None);
    }

    #[test]
    fn wait_until_gone_succeeds_when_already_gone() {
        let mut polls = 0;
        assert!(wait_until_gone(
            || {
                polls += 1;
                false
            },
            Duration::from_millis(20),
            Duration::from_millis(1)
        ));
        assert_eq!(polls, 1);
    }

    #[test]
    fn wait_until_gone_succeeds_after_a_few_polls() {
        let mut polls = 0;
        assert!(wait_until_gone(
            || {
                polls += 1;
                polls < 3
            },
            Duration::from_millis(20),
            Duration::from_millis(1)
        ));
        assert_eq!(polls, 3);
    }

    #[test]
    fn wait_until_gone_times_out_when_still_present() {
        assert!(!wait_until_gone(
            || true,
            Duration::from_millis(20),
            Duration::from_millis(1)
        ));
    }

    #[test]
    fn retry_until_gone_succeeds_after_first_attempt() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || false,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Ok(true));
        assert_eq!(invocations.get(), 1);
    }

    #[test]
    fn retry_until_gone_succeeds_after_second_attempt() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || invocations.get() < 2,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Ok(true));
        assert_eq!(invocations.get(), 2);
    }

    #[test]
    fn retry_until_gone_returns_false_after_all_attempts() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || true,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Ok(false));
        assert_eq!(invocations.get(), 3);
    }

    #[test]
    fn retry_until_gone_returns_invoke_error_without_retrying() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Err("PowerShell failed".to_owned())
            },
            || true,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Err("PowerShell failed".to_owned()));
        assert_eq!(invocations.get(), 1);
    }

    #[test]
    fn eject_way_uses_the_verb_for_a_removable_drive() {
        assert_eq!(eject_way(DriveKind::Removable, false), EjectWay::Verb);
        assert_eq!(eject_way(DriveKind::Removable, true), EjectWay::Verb);
    }

    #[test]
    fn eject_way_uses_the_device_for_a_fixed_usb_drive() {
        assert_eq!(eject_way(DriveKind::Fixed, true), EjectWay::Device);
    }

    #[test]
    fn eject_way_does_not_eject_other_drives() {
        assert_eq!(eject_way(DriveKind::Fixed, false), EjectWay::NotEjected);
        assert_eq!(eject_way(DriveKind::Other, false), EjectWay::NotEjected);
        assert_eq!(eject_way(DriveKind::Other, true), EjectWay::NotEjected);
    }

    fn utf16(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn split_multi_sz_gives_two_entries() {
        let mut list = utf16("ab");
        list.push(0);
        list.extend(utf16("c"));
        list.extend([0, 0]);

        assert_eq!(split_multi_sz(&list), vec![utf16("ab"), utf16("c")]);
    }

    #[test]
    fn split_multi_sz_gives_one_entry() {
        let mut list = utf16("disk");
        list.extend([0, 0]);

        assert_eq!(split_multi_sz(&list), vec![utf16("disk")]);
    }

    #[test]
    fn split_multi_sz_of_the_empty_list_is_empty() {
        assert!(split_multi_sz(&[0]).is_empty());
        assert!(split_multi_sz(&[]).is_empty());
    }

    #[test]
    fn split_multi_sz_without_the_final_double_nul_gives_what_it_holds() {
        let mut list = utf16("ab");
        list.push(0);
        list.extend(utf16("c"));

        assert_eq!(split_multi_sz(&list), vec![utf16("ab"), utf16("c")]);
    }

    #[test]
    fn matching_interface_picks_the_right_index_among_three() {
        let interfaces = vec![utf16("a"), utf16("b"), utf16("c")];
        let numbers = |interface: &[u16]| match interface.first().copied() {
            Some(unit) if unit == u16::from(b'a') => Some((7, 0)),
            Some(unit) if unit == u16::from(b'b') => Some((7, 2)),
            _ => Some((7, 1)),
        };

        assert_eq!(matching_interface(&interfaces, (7, 2), numbers), Some(1));
        assert_eq!(matching_interface(&interfaces, (7, 1), numbers), Some(2));
    }

    #[test]
    fn matching_interface_gives_none_when_none_matches() {
        let interfaces = vec![utf16("a"), utf16("b"), utf16("c")];

        assert_eq!(
            matching_interface(&interfaces, (7, 9), |_| Some((7, 0))),
            None
        );
    }

    #[test]
    fn matching_interface_skips_an_interface_without_a_number() {
        let interfaces = vec![utf16("a"), utf16("b")];
        let mut asked = 0;
        let found = matching_interface(&interfaces, (7, 3), |interface| {
            asked += 1;
            if interface == utf16("a").as_slice() {
                None
            } else {
                Some((7, 3))
            }
        });

        assert_eq!(found, Some(1));
        assert_eq!(asked, 2);
    }

    #[test]
    fn retry_until_gone_or_refused_gives_up_after_three_refusals() {
        let invocations = std::cell::Cell::new(0);
        let polls = std::cell::Cell::new(0);
        let result = retry_until_gone_or_refused(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(Attempt::Refused)
            },
            || {
                polls.set(polls.get() + 1);
                true
            },
            Duration::from_millis(2),
            Duration::from_millis(1),
            Duration::from_millis(2),
        );

        assert_eq!(result, Ok(false));
        assert_eq!(invocations.get(), 3);
        assert_eq!(polls.get(), 0);
    }

    #[test]
    fn retry_until_gone_or_refused_succeeds_after_a_refusal() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone_or_refused(
            3,
            || {
                invocations.set(invocations.get() + 1);
                if invocations.get() == 1 {
                    Ok(Attempt::Refused)
                } else {
                    Ok(Attempt::Started)
                }
            },
            || false,
            Duration::from_millis(2),
            Duration::from_millis(1),
            Duration::from_millis(2),
        );

        assert_eq!(result, Ok(true));
        assert_eq!(invocations.get(), 2);
    }

    #[test]
    fn retry_until_gone_or_refused_stops_at_an_error_after_a_refusal() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone_or_refused(
            3,
            || {
                invocations.set(invocations.get() + 1);
                if invocations.get() == 1 {
                    Ok(Attempt::Refused)
                } else {
                    Err("Windows failed".to_owned())
                }
            },
            || true,
            Duration::from_millis(2),
            Duration::from_millis(1),
            Duration::from_millis(2),
        );

        assert_eq!(result, Err("Windows failed".to_owned()));
        assert_eq!(invocations.get(), 2);
    }

    #[test]
    fn eject_texts_are_exact() {
        assert_eq!(
            ejected_text("I:"),
            "I: has been ejected and can be removed."
        );
        assert_eq!(
            device_in_use_text("I:"),
            "I: could not be ejected (is it in use by another program?). Use Safely Remove Hardware in the taskbar. Attempts: 3."
        );
        assert_eq!(
            not_ejected_text("I:"),
            "I: was not ejected: it is neither a removable drive nor a USB drive."
        );
        assert_eq!(
            windows_error_text("I:", 23),
            "I: could not be ejected: Windows reported error 23."
        );
    }
}
