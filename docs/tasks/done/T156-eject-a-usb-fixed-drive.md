# T156. Eject a card reader that Windows sees as a fixed USB disk — Done

- **Why:** the owner's job test (2026-11-16): with a CFexpress reader the eject after an import always
  fails with "I: could not be ejected (is it in use by another program?). Eject it in File Explorer.
  Attempts: 3." The reviewer checked with the card in (read-only): the reader ("ABT CFX Card
  Reader", USB, UAS) is a **fixed** disk to Windows (`Get-Volume`: DriveType Fixed), and its shell
  object has **no "Eject" verb**. `eject_script` calls `InvokeVerb('Eject')`, which then does
  nothing, so `I:\DCIM` never goes away. A fixed USB disk is removed with "Safely Remove Hardware",
  whose API is `CM_Request_Device_EjectW`. **Owner, 2026-11-16** (ADR 0005, choice 3): a removable
  drive keeps the verb; a fixed drive on USB is ejected through the device; any other drive is not
  ejected. `windows-sys` 0.61.2 (already used by `pw-loupe` and `pw-metadata`) is approved as a
  dependency of `pw-ingest`.
- **Crates:** `pw-ingest`. **Files:** `crates/pw-ingest/Cargo.toml`, `Cargo.lock` (updated by
  cargo only), `crates/pw-ingest/src/eject.rs`, `docs/glossary.md`, `docs/test-inventory.md`.
- **What:**
  1. **Dependency.** In `crates/pw-ingest/Cargo.toml` add, as `pw-loupe` does,
     `[target.'cfg(windows)'.dependencies]` `windows-sys = { version = "=0.61.2", features = [...] }`
     with exactly the features the code needs, at most: `Win32_Foundation`,
     `Win32_Storage_FileSystem`, `Win32_System_IO`, `Win32_System_Ioctl`,
     `Win32_System_WindowsProgramming`, `Win32_Devices_DeviceAndDriverInstallation`,
     `Win32_Devices_Properties`. No other dependency change.
  2. **Which way** (pure, in `eject.rs`, private, `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`,
     `#[cfg(any(windows, test))]` like the helpers there today): `enum DriveKind { Removable, Fixed,
     Other }`, `enum EjectWay { Verb, Device, NotEjected }` and
     `fn eject_way(kind: DriveKind, on_usb: bool) -> EjectWay`: `Removable` → `Verb`; `Fixed` with
     `on_usb` → `Device`; everything else → `NotEjected`. On Windows, `eject` gets the drive type with
     `GetDriveTypeW("I:\\")` and maps `DRIVE_REMOVABLE` → `Removable`, `DRIVE_FIXED` → `Fixed`, any
     other value → `Other` (so the pure part and its tests use no Windows constants). `on_usb` comes from `IOCTL_STORAGE_QUERY_PROPERTY`
     (`StorageDeviceProperty`, `STORAGE_DEVICE_DESCRIPTOR::BusType == BusTypeUsb`) on the volume
     `\\.\I:`. Volumes and interfaces are opened with `std::fs::OpenOptions` and
     `std::os::windows::fs::OpenOptionsExt` (`access_mode(0)`, `share_mode(FILE_SHARE_READ |
     FILE_SHARE_WRITE)`), and `as_raw_handle()` is passed to `DeviceIoControl`; `File` closes the
     handle. No `CreateFileW` (it needs the feature `Win32_Security`). If the query fails, `on_usb`
     is false. An internal SATA or NVMe disk is therefore never ejected, even if Windows lists it as
     hot-pluggable.
  3. **The Verb way** is today's code, unchanged: `eject_script`, PowerShell, `retry_until_gone`,
     and the same messages.
  4. **The Device way** (Windows only):
     - the volume's `STORAGE_DEVICE_NUMBER` from `IOCTL_STORAGE_GET_DEVICE_NUMBER` on `\\.\I:`;
     - the present disk interfaces from `CM_Get_Device_Interface_List_SizeW` and
       `CM_Get_Device_Interface_ListW` (`GUID_DEVINTERFACE_DISK`,
       `CM_GET_DEVICE_INTERFACE_LIST_PRESENT`). Each one is opened as above and the one with the same
       `DeviceType` and `DeviceNumber` is chosen;
     - its device instance from `CM_Get_Device_Interface_PropertyW` (`DEVPKEY_Device_InstanceId`)
       and `CM_Locate_DevNodeW`, then its parent (the USB storage device) from `CM_Get_Parent`;
     - one attempt is `CM_Request_Device_EjectW` on the parent, with a `PNP_VETO_TYPE` out value and
       a veto-name buffer (`MAX_PATH` UTF-16; both are passed so that Windows shows no dialog of its
       own; the veto name is not used further). A refusal (`CR_REMOVE_VETOED`, any other `CR_*`
       result than `CR_SUCCESS`, or a veto type other than `PNP_VetoTypeUnknown`) ends that attempt
       at once, without the 5 s wait for `I:\DCIM`; the next attempt follows after a pause of 1 s.
       Otherwise the existing wait for `I:\DCIM` to go away follows. At most `EJECT_ATTEMPTS` (3)
       attempts, as today;
     - **the retry:** a new private `enum Attempt { Started, Refused }` and a new function
       `retry_until_gone_or_refused(attempts, invoke: impl FnMut() -> Result<Attempt, String>,
       still_present, timeout, interval, pause_after_refusal: Duration) -> Result<bool, String>`.
       `Started` waits as today; `Refused` does not ask `still_present` and sleeps
       `pause_after_refusal` before the next attempt (not after the last); `Err` stops at once as
       today. All attempts refused or still present gives `Ok(false)` (the "in use" text below).
       `retry_until_gone` keeps its signature and calls the new function (`Ok(())` →
       `Attempt::Started`, pause zero), so its existing tests stay unchanged and keep passing;
     - the parent is the whole reader, so a reader with several slots loses all of them. That is
       what "Safely Remove Hardware" does too.
  5. **Pure helpers** (so the parts without Windows can be tested):
     `fn split_multi_sz(list: &[u16]) -> Vec<Vec<u16>>` (the `\0`-separated, `\0\0`-ended interface
     list; an empty or unended list gives what it holds, no panic), and
     `fn matching_interface(interfaces: &[Vec<u16>], wanted: (u32, u32), number_of: impl FnMut(&[u16]) -> Option<(u32, u32)>) -> Option<usize>`.
  6. **Unsafe code:** every `unsafe` block is as small as possible and has a `// SAFETY:` comment.
     Buffer sizes come from the API's size calls, never assumed: if `CM_Get_Device_Interface_ListW`
     returns `CR_BUFFER_SMALL` (a device arrived in between), the size is asked again, at most three
     times. `CM_Get_Device_Interface_PropertyW` is called first with size 0, where `CR_BUFFER_SMALL`
     and the size in bytes are the expected answer, not an error; the `Vec<u16>` length is computed
     from the bytes with checked arithmetic. No `.unwrap()`, `.expect()` or unchecked
     indexing (AGENTS.md §3).
  7. **Texts** (rule 13; made in `eject.rs`, where the eject texts are today, each one whole
     sentence from one `format!`):
     - success, both ways: unchanged, "{drive} has been ejected and can be removed.";
     - Device way, still there after all attempts:
       "{drive} could not be ejected (is it in use by another program?). Use Safely Remove Hardware in the taskbar. Attempts: {EJECT_ATTEMPTS}.";
     - `NotEjected`: "{drive} was not ejected: it is neither a removable drive nor a USB drive.";
     - a failing Windows call (no handle, no device number, no matching disk, a `CR_*` error outside
       the attempt): "{drive} could not be ejected: Windows reported error {code}." (the
       `GetLastError` value or the `CR_*` value; one text for both is deliberate, the number is for
       a bug report);
     - the not-Windows text and "The source is not a drive, so nothing was ejected." are unchanged.
  8. **Glossary:** add "Safely Remove Hardware" (Windows' own English name for "Säker borttagning av
     maskinvara") to `docs/glossary.md`, next to "mata ut", and name it in the report.
- **Tests to add** (in `eject.rs`):
  - `eject_way`: `Removable` gives `Verb` (with and without USB); `Fixed` with USB gives `Device`;
    `Fixed` without USB and `Other` (with and without USB) give `NotEjected`.
  - `split_multi_sz`: two entries; one entry; the empty list (`[0]`); a list without its final
    `\0\0`.
  - `matching_interface`: picks the right index among three; `None` when none matches; skips an
    interface whose `number_of` gives `None`.
  - `retry_until_gone_or_refused`: three refusals call `invoke` three times, never ask
    `still_present` and give `Ok(false)`; a refusal then a `Started` whose `still_present` answers
    false gives `Ok(true)` after two calls; an `Err` after a refusal stops at once with that error.
    The pause is a few milliseconds in the tests.
  - Each new text, exactly as in point 7.
- **Not in this task:** naming the program that holds the card (`pw-ingest` has no logging; the veto
  name is not shown); ejecting other drive kinds; changing `pw-loupe` (the result line already shows
  whatever `pw_ingest::eject` returns).
- **Rules:** AGENTS.md §3 (no panics; the dependency is approved for this task only, with the
  features above); `docs/tasks/README.md` rule 13. This task adds FFI code with `unsafe`, so it gets
  the independent code review of the README ("An extra code review").
- **Hand check (owner):** with the CFexpress reader, an import with "Eject the card" ends with
  "I: has been ejected and can be removed." and the reader disappears from File Explorer. With an SD
  card in a reader whose drive is removable, the eject still works as before. With a file on the
  card open in another program, the "in use" text appears within a few seconds, not after 15.
- **Done when:** `.\tools\check.ps1` is all OK and `git diff --stat` shows only the files above.

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-16 | Open | Added after the owner's job test (bug 1 of 5). Diagnosis with the card in: fixed USB disk without an "Eject" verb. Owner approved `windows-sys` in `pw-ingest` and the decision in ADR 0005 choice 3 |
| 2026-11-16 | Open | Cold read: `CreateFileW` needs `Win32_Security` (now `std::fs::OpenOptions` + `access_mode(0)`); `retry_until_gone` cannot change without editing its tests (now a new `retry_until_gone_or_refused` that it calls); `DriveKind` keeps the pure part free of Windows constants; `CR_BUFFER_SMALL` handling, `CR_REMOVE_VETOED`, a 1 s pause after a refusal and the shared error text are stated; the veto name is not logged. The owner's decision is in ADR 0005 choice 3 |
| 2026-11-16 | Ready for review | Done as written: `eject_way` chooses the verb (removable), the device (fixed on USB) or no eject; the Device way uses `CM_Request_Device_EjectW` on the parent of the matching disk interface through `retry_until_gone_or_refused`; `retry_until_gone` calls it unchanged for the verb. 14 new tests in `eject.rs` (pw-ingest unit 30 → 44); `check.ps1` all OK, 1624 passed, 63 ignored, 0 failed. Glossary: "Safely Remove Hardware". The Windows calls are untested without the reader (hand check) |
| 2026-11-16 | Done | Reviewed, with the independent review the README asks for (FFI and `unsafe`): no bugs. Its findings are not changed here and are in `backlog.md` ("Eject through the device: known limits"): the direct parent only (a composite USB reader would be refused; the owner's CFexpress reader's parent USB device is itself removable), a bus-type buffer of exactly the struct size, `CR_FAILURE` (19) reused for "no matching disk", and the veto buffer length given by `MAX_PATH`. Owner's hand check approved on 2026-11-16: the CFexpress reader and an SD card (removable) were both ejected. Committed |
