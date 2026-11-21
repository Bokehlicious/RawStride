//! The folder watcher (ADR 0026 step F2, §3.2): one thread that follows the open folders' files
//! coming and going, through Windows' change notifications (`ReadDirectoryChangesW` on one I/O
//! completion port), and says "this folder changed" after a quiet moment.
//!
//! Everything the thread decides is plain Rust, tested on its own (ADR 0026 §7): the quiet moment
//! ([`QuietMoment`]), which notifications count ([`counts_as_change`]), which folders to open and
//! end ([`plan_set`]) and the applied sets' numbers ([`Applied`]). Only the `sys` module calls
//! Windows.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::tabs::same_folder;

/// How long no notification must have come for a folder before it is said to have changed
/// (ADR 0026 §3.2, the quiet moment: a copy of many files gives one listing, not one per file).
const QUIET: Duration = Duration::from_millis(500);

/// The longest a folder waits while notifications keep coming, counted from its first
/// notification of the round (ADR 0026 §3.2: a long copy still shows its files as it goes).
const MOST: Duration = Duration::from_secs(2);

/// How long `Drop` waits for the thread to close every handle.
const DROP_WAIT: Duration = Duration::from_secs(2);

/// How long the thread waits for a cancelled read's completion before it gives up on it (leaks
/// what Windows may still write into and closes the handle), so a read that never completes does
/// not hold back later sets; shorter than [`DROP_WAIT`], so at the thread's end it has given up
/// before `Drop` stops waiting for it.
#[cfg_attr(not(windows), allow(dead_code))]
const STOP_WAIT: Duration = Duration::from_millis(1500);

/// What the watcher tells the shell. Each watch starts with `Started` or `Ended`; a live watch
/// then gives `Changed` until it ends. The shell lists the folder again at `Started` (a file that
/// arrived between the tab's listing and the watch's start would otherwise never be shown) and
/// at `Changed` (ADR 0026 F3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchEvent {
    /// The folder's watch has started (its first read is issued): a change made from now on is
    /// seen. Sent once per watch, so again when the folder has left the set and been given again.
    /// The shell lists the folder once more (ADR 0026 F3: the gap between a tab's listing and its
    /// watch).
    Started(PathBuf),
    /// Files came or went in the folder (named as it was first given), after a quiet moment.
    Changed(PathBuf),
    /// The folder's watch could not start (the folder does not exist, cannot be opened, or its
    /// volume does not support notifications) or stopped (the drive is gone, the folder was
    /// deleted). `reason` is the system's error text, for the log only. The folder is not watched
    /// again until it has left the set and been given again.
    Ended { folder: PathBuf, reason: String },
}

/// The shell's wake-up call, shared by the owner's side and the thread.
type Wake = Arc<Mutex<Box<dyn Fn() + Send>>>;

fn call_wake(wake: &Wake) {
    let wake = wake.lock().unwrap_or_else(PoisonError::into_inner);
    wake();
}

/// The watcher: one thread with one directory handle per watched folder (ADR 0026 §3.2).
pub struct FolderWatcher {
    events_sender: Sender<WatchEvent>,
    events: Receiver<WatchEvent>,
    wake: Wake,
    applied: Arc<Applied>,
    /// The number of the last `set_folders` call (0 before the first).
    number: u64,
    /// The thread; `None` when it or its completion port could not be made (or it has stopped).
    running: Option<sys::Running>,
    /// Without a thread: the folders already answered with `Ended`, kept as `set_folders` keeps
    /// an ended folder, so a folder is answered again only after it has left the set.
    unwatched: Vec<PathBuf>,
    /// Without a thread: why, for the `Ended` events.
    reason: String,
}

impl std::fmt::Debug for FolderWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FolderWatcher")
            .field("number", &self.number)
            .field("running", &self.running.is_some())
            .field("unwatched", &self.unwatched)
            .finish_non_exhaustive()
    }
}

impl FolderWatcher {
    /// Starts the thread (`pw-folder-watcher`). `wake` is called on the thread after each event
    /// is sent (the shell gives it `egui::Context::request_repaint`). When the thread or its
    /// completion port cannot be made, the reason is logged and every folder given later is
    /// answered with `WatchEvent::Ended` (ADR 0026 F-Q3: the shell falls back).
    pub fn start(wake: Box<dyn Fn() + Send>) -> Self {
        let wake: Wake = Arc::new(Mutex::new(wake));
        let (events_sender, events) = mpsc::channel();
        let applied = Arc::new(Applied::default());
        let (running, reason) = match sys::Running::start(
            events_sender.clone(),
            Arc::clone(&wake),
            Arc::clone(&applied),
        ) {
            Ok(running) => (Some(running), String::new()),
            Err(reason) => {
                eprintln!("pw-loupe: {reason}");
                (None, reason)
            }
        };
        Self {
            events_sender,
            events,
            wake,
            applied,
            number: 0,
            running,
            unwatched: Vec::new(),
            reason,
        }
    }

    /// The folders to watch from now on, the whole set each time. Does not block: the thread
    /// ends the watches of folders no longer in the set, then opens the new ones. Folders
    /// `same_folder` calls the same get one watch, named by the first spelling given.
    pub fn set_folders(&mut self, folders: Vec<PathBuf>) {
        if let Some(running) = &self.running {
            self.number += 1;
            if running.send_set(self.number, folders.clone()) {
                return;
            }
            eprintln!("pw-loupe: the folder watcher has stopped");
            self.running = None;
            self.reason = "the folder watcher has stopped".to_string();
        }
        self.answer_without_thread(folders);
    }

    /// Without a thread: each folder new to the set is answered with `Ended` at once.
    fn answer_without_thread(&mut self, folders: Vec<PathBuf>) {
        let change = plan_set(&self.unwatched, &folders, same_folder);
        remove_indexes(&mut self.unwatched, change.end);
        let mut sent = false;
        for folder in change.open {
            self.unwatched.push(folder.clone());
            let event = WatchEvent::Ended {
                folder,
                reason: self.reason.clone(),
            };
            sent |= self.events_sender.send(event).is_ok();
        }
        if sent {
            call_wake(&self.wake);
        }
    }

    /// Blocks until the thread has applied the last `set_folders` made before this call (every
    /// watch of a folder that left the set has closed its handle, and every open still running
    /// for such a folder has come back and been closed) or `wait` has passed; true when applied.
    /// Opens of folders still in the set do not count: applied means "nothing left open that
    /// should not be". Without a thread, true at once (no handle is open).
    pub fn wait_until_applied(&self, wait: Duration) -> bool {
        if self.running.is_none() {
            return true;
        }
        self.applied.wait_for(self.number, wait)
    }

    /// The next event, if one is waiting; never blocks.
    pub fn try_recv(&mut self) -> Option<WatchEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for FolderWatcher {
    /// Ends every watch and the thread, and returns when the thread has closed every handle (at
    /// most [`DROP_WAIT`]). A helper thread still opening a folder is not waited for.
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop(DROP_WAIT);
        }
    }
}

/// The number of the newest set the thread has applied, and a `Condvar` to wait for it.
#[derive(Debug, Default)]
struct Applied {
    number: Mutex<u64>,
    changed: Condvar,
}

impl Applied {
    /// Publishes `number` as applied (never lowering it).
    #[cfg_attr(not(windows), allow(dead_code))]
    fn publish(&self, number: u64) {
        let mut applied = self.number.lock().unwrap_or_else(PoisonError::into_inner);
        if number > *applied {
            *applied = number;
        }
        self.changed.notify_all();
    }

    /// Waits until the published number is at least `number`, at most `wait`; true when it is.
    fn wait_for(&self, number: u64, wait: Duration) -> bool {
        let applied = self.number.lock().unwrap_or_else(PoisonError::into_inner);
        let (applied, _) = self
            .changed
            .wait_timeout_while(applied, wait, |applied| *applied < number)
            .unwrap_or_else(PoisonError::into_inner);
        *applied >= number
    }
}

/// The quiet moment (ADR 0026 §3.2): a folder is due when no notification has come for
/// [`QUIET`] since its last, or [`MOST`] has passed since its first while they keep coming.
#[derive(Debug, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct QuietMoment {
    /// Each waiting folder with its first and last notification of this round.
    waiting: Vec<(PathBuf, Instant, Instant)>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl QuietMoment {
    /// Notes a notification for `folder` at `now`.
    fn notice(&mut self, folder: &Path, now: Instant) {
        match self
            .waiting
            .iter_mut()
            .find(|(waiting, _, _)| waiting == folder)
        {
            Some((_, _, last)) => *last = now,
            None => self.waiting.push((folder.to_path_buf(), now, now)),
        }
    }

    /// Forgets `folder` (its watch has ended; it gives no event).
    fn forget(&mut self, folder: &Path) {
        self.waiting.retain(|(waiting, _, _)| waiting != folder);
    }

    /// Forgets every folder.
    fn clear(&mut self) {
        self.waiting.clear();
    }

    fn due_at(first: Instant, last: Instant) -> Instant {
        (last + QUIET).min(first + MOST)
    }

    /// The due folders, forgotten.
    fn take_due(&mut self, now: Instant) -> Vec<PathBuf> {
        let mut due = Vec::new();
        self.waiting.retain(|(folder, first, last)| {
            if now >= Self::due_at(*first, *last) {
                due.push(folder.clone());
                false
            } else {
                true
            }
        });
        due
    }

    /// How long until the next folder is due; `None` when nothing waits.
    fn next_due(&self, now: Instant) -> Option<Duration> {
        self.waiting
            .iter()
            .map(|(_, first, last)| Self::due_at(*first, *last).saturating_duration_since(now))
            .min()
    }
}

/// The size of a `FILE_NOTIFY_INFORMATION` record before its name: `NextEntryOffset`, `Action`
/// and `FileNameLength`, each a `u32`.
#[cfg_attr(not(windows), allow(dead_code))]
const RECORD_HEADER: usize = 12;

fn read_u32(buffer: &[u8], at: usize) -> Option<u32> {
    let bytes = buffer.get(at..at.checked_add(4)?)?;
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_ne_bytes(bytes))
}

/// Whether a completion's `FILE_NOTIFY_INFORMATION` records name a file `crate::is_listed` lists
/// (ADR 0026 §2 point 6: RawStride's own temporary names and sidecars start no listing). Reads
/// only the first `byte_count` bytes (clamped to the buffer). 0 bytes (Windows' buffer overflowed
/// and the records are lost) or a record that cannot be read counts as a change: a wrong
/// "changed" costs one listing, a wrong "not changed" loses a file.
#[cfg_attr(not(windows), allow(dead_code))]
fn counts_as_change(buffer: &[u8], byte_count: usize) -> bool {
    let buffer = buffer
        .get(..byte_count.min(buffer.len()))
        .unwrap_or_default();
    if buffer.is_empty() {
        return true;
    }
    let mut at = 0usize;
    loop {
        let (Some(next), Some(name_length)) = (read_u32(buffer, at), read_u32(buffer, at + 8))
        else {
            return true;
        };
        let name_start = at + RECORD_HEADER;
        let Some(name) = name_start
            .checked_add(name_length as usize)
            .and_then(|name_end| buffer.get(name_start..name_end))
        else {
            return true;
        };
        let (units, odd) = name.as_chunks::<2>();
        if !odd.is_empty() {
            return true;
        }
        let units: Vec<u16> = units.iter().map(|unit| u16::from_ne_bytes(*unit)).collect();
        let name = String::from_utf16_lossy(&units);
        if crate::is_listed(Path::new(&name)) {
            return true;
        }
        if next == 0 {
            return false;
        }
        match at.checked_add(next as usize) {
            Some(next_at) if next_at < buffer.len() => at = next_at,
            _ => return true,
        }
    }
}

/// What a new set changes: the folders to open, and the indexes (into the watched folders) of
/// the folders to end.
#[derive(Debug, Default, PartialEq, Eq)]
struct SetChange {
    open: Vec<PathBuf>,
    end: Vec<usize>,
}

/// Given the watched folders (ended ones included) and the new set, the folders to open and the
/// folders to end, with folders `same` calls equal counted once (ADR 0026 §3.2, one handle per
/// open folder); a folder already watched keeps its first spelling.
fn plan_set(
    watched: &[PathBuf],
    folders: &[PathBuf],
    same: impl Fn(&Path, &Path) -> bool,
) -> SetChange {
    let mut change = SetChange::default();
    for folder in folders {
        let known = watched.iter().any(|watched| same(watched, folder))
            || change.open.iter().any(|open| same(open, folder));
        if !known {
            change.open.push(folder.clone());
        }
    }
    change.end = watched
        .iter()
        .enumerate()
        .filter(|(_, watched)| !folders.iter().any(|folder| same(watched, folder)))
        .map(|(index, _)| index)
        .collect();
    change
}

/// Removes the items at `indexes` (any order; ones past the end ignored), returning them.
fn remove_indexes<T>(items: &mut Vec<T>, mut indexes: Vec<usize>) -> Vec<T> {
    indexes.sort_unstable();
    indexes.dedup();
    let mut removed = Vec::new();
    for index in indexes.into_iter().rev() {
        if index < items.len() {
            removed.push(items.remove(index));
        }
    }
    removed
}

/// `ERROR_NOTIFY_ENUM_DIR`: Windows had more changes than it could record and asks for the
/// folder to be listed again.
#[cfg_attr(not(windows), allow(dead_code))]
const ERROR_NOTIFY_ENUM_DIR: u32 = 1022;

/// What a read that completed with an error does to its watch.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum AfterError {
    /// The changes are lost but the watch still works: counted as a change (as 0 bytes are,
    /// [`counts_as_change`]) and the read is started again.
    ChangeAndReadAgain,
    /// The watch ends with `Ended` (the drive gone, the folder deleted, access lost).
    End,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn after_error(error: u32) -> AfterError {
    if error == ERROR_NOTIFY_ENUM_DIR {
        AfterError::ChangeAndReadAgain
    } else {
        AfterError::End
    }
}

/// Whether the newest applied set may be published: no ended watch still waits for its
/// cancelled read, and no open of a folder that left the set is still to be closed (the caller
/// may eject a card as soon as `wait_until_applied` returns true).
#[cfg_attr(not(windows), allow(dead_code))]
fn may_publish(ending: usize, opens_to_close: usize) -> bool {
    ending == 0 && opens_to_close == 0
}

/// Takes out the items whose deadline has passed.
#[cfg_attr(not(windows), allow(dead_code))]
fn take_expired<T>(items: &mut Vec<(T, Instant)>, now: Instant) -> Vec<T> {
    let mut expired = Vec::new();
    let mut kept = Vec::new();
    for (item, deadline) in std::mem::take(items) {
        if now >= deadline {
            expired.push(item);
        } else {
            kept.push((item, deadline));
        }
    }
    *items = kept;
    expired
}

/// How long until the nearest deadline; `None` without items.
#[cfg_attr(not(windows), allow(dead_code))]
fn nearest_deadline<T>(items: &[(T, Instant)], now: Instant) -> Option<Duration> {
    items
        .iter()
        .map(|(_, deadline)| deadline.saturating_duration_since(now))
        .min()
}

/// A message to the watcher thread.
#[cfg_attr(not(windows), allow(dead_code))]
enum Control {
    /// A new set of folders, numbered.
    Set { number: u64, folders: Vec<PathBuf> },
    /// A helper thread's open of the watch with this key.
    Opened {
        key: usize,
        result: std::io::Result<std::fs::File>,
    },
    /// End every watch and the thread.
    Stop,
}

#[cfg(windows)]
mod sys {
    //! The Windows part: the completion port, the directory handles and their reads.

    use super::{
        AfterError, Applied, Control, QuietMoment, STOP_WAIT, Wake, WatchEvent, after_error,
        call_wake, counts_as_change, may_publish, nearest_deadline, plan_set, remove_indexes,
        same_folder, take_expired,
    };
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_NOT_FOUND, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
        FILE_NOTIFY_CHANGE_FILE_NAME, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        ReadDirectoryChangesW,
    };
    use windows_sys::Win32::System::IO::{
        CancelIoEx, CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED,
        PostQueuedCompletionStatus,
    };

    /// `GetQueuedCompletionStatus`'s "wait without end"; `windows-sys` has it only under
    /// `Win32_System_Threading`, which is not enabled.
    const INFINITE: u32 = u32::MAX;

    /// The completion key of the control post; watches count from 1 and never reuse a key.
    const CONTROL_KEY: usize = 0;

    /// A read buffer's size in `u32`s (16 KiB; network folders allow at most 64 KiB). `u32`s keep
    /// it DWORD-aligned, as `ReadDirectoryChangesW` needs.
    const BUFFER_WORDS: usize = 4096;
    const BUFFER_BYTES: u32 = (BUFFER_WORDS * 4) as u32;

    /// The completion port's handle, shared through an `Arc` by `FolderWatcher`, the thread and
    /// the helper threads, and closed on the last drop, so nobody posts to or waits on a closed
    /// port.
    struct Port(HANDLE);

    // SAFETY: a completion port handle may be used from any thread; the handle is only closed in
    // `Drop`, when no other owner is left.
    unsafe impl Send for Port {}
    // SAFETY: as above; `PostQueuedCompletionStatus` and `GetQueuedCompletionStatus` are safe to
    // call on one port from several threads at once.
    unsafe impl Sync for Port {}

    impl Port {
        fn new() -> std::io::Result<Self> {
            // SAFETY: plain call; `INVALID_HANDLE_VALUE` with no existing port makes a new port,
            // no pointer is passed.
            let handle =
                unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, std::ptr::null_mut(), 0, 1) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self(handle))
        }

        /// Wakes the thread waiting on the port.
        fn post(&self) -> bool {
            // SAFETY: the port is open while `self` lives; no `OVERLAPPED` is passed.
            let posted =
                unsafe { PostQueuedCompletionStatus(self.0, 0, CONTROL_KEY, std::ptr::null()) };
            if posted == 0 {
                eprintln!(
                    "pw-loupe: the folder watcher could not be woken: {}",
                    std::io::Error::last_os_error()
                );
            }
            posted != 0
        }

        fn wait(&self, timeout: u32) -> Packet {
            let mut bytes = 0u32;
            let mut key = 0usize;
            let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
            // SAFETY: the port is open while `self` lives; the three out pointers point at locals
            // that outlive the call.
            let ok = unsafe {
                GetQueuedCompletionStatus(self.0, &mut bytes, &mut key, &mut overlapped, timeout)
            };
            // SAFETY: plain call, reading this thread's last error.
            let error = if ok == 0 {
                unsafe { GetLastError() }
            } else {
                0
            };
            if overlapped.is_null() {
                if ok != 0 {
                    Packet::Control
                } else if error == WAIT_TIMEOUT {
                    Packet::Timeout
                } else {
                    Packet::PortError(error)
                }
            } else {
                Packet::Completion {
                    key,
                    overlapped,
                    bytes,
                    error: (ok == 0).then_some(error),
                }
            }
        }
    }

    impl Drop for Port {
        fn drop(&mut self) {
            // SAFETY: the handle was made by `CreateIoCompletionPort` and is closed once, here,
            // when the last owner drops it.
            unsafe { CloseHandle(self.0) };
        }
    }

    enum Packet {
        Control,
        Timeout,
        PortError(u32),
        Completion {
            key: usize,
            overlapped: *mut OVERLAPPED,
            bytes: u32,
            error: Option<u32>,
        },
    }

    /// The owner's side of the running thread.
    pub(super) struct Running {
        control: Sender<Control>,
        port: Arc<Port>,
        done: Receiver<()>,
    }

    impl Running {
        pub(super) fn start(
            events: Sender<WatchEvent>,
            wake: Wake,
            applied: Arc<Applied>,
        ) -> Result<Self, String> {
            let port = Arc::new(Port::new().map_err(|error| {
                format!("the folder watcher's completion port could not be made: {error}")
            })?);
            let (control, control_receiver) = mpsc::channel();
            let (done_sender, done) = mpsc::channel();
            let thread_port = Arc::clone(&port);
            let thread_control = control.clone();
            std::thread::Builder::new()
                .name("pw-folder-watcher".into())
                .spawn(move || {
                    // Made on the thread: its watches hold raw pointers and never leave it.
                    let mut thread = WatchThread {
                        port: thread_port,
                        control: thread_control,
                        control_receiver,
                        events,
                        wake,
                        applied,
                        entries: Vec::new(),
                        ending: Vec::new(),
                        opens_to_close: Vec::new(),
                        quiet: QuietMoment::default(),
                        next_key: CONTROL_KEY + 1,
                        to_publish: None,
                        stopping: false,
                    };
                    thread.run();
                    drop(thread);
                    let _ = done_sender.send(());
                })
                .map_err(|error| format!("the folder watcher could not be started: {error}"))?;
            Ok(Self {
                control,
                port,
                done,
            })
        }

        /// Sends a numbered set; false when the thread is gone or cannot be woken (the set would
        /// wait unseen).
        pub(super) fn send_set(&self, number: u64, folders: Vec<PathBuf>) -> bool {
            if self.control.send(Control::Set { number, folders }).is_err() {
                return false;
            }
            self.port.post()
        }

        /// Ends the thread and waits for it to have closed every handle, at most `wait`.
        pub(super) fn stop(self, wait: Duration) {
            if self.control.send(Control::Stop).is_ok() && self.port.post() {
                // `Ok` (done) or `Disconnected` (the thread has ended) both mean every handle is
                // closed; a timeout is logged.
                if let Err(mpsc::RecvTimeoutError::Timeout) = self.done.recv_timeout(wait) {
                    eprintln!("pw-loupe: the folder watcher did not end in time");
                }
            }
        }
    }

    /// Opens a folder for watching (`OPEN_EXISTING`, since nothing is created), so the user can
    /// still rename or delete its files and the folder itself.
    fn open_folder(folder: &Path) -> std::io::Result<File> {
        OpenOptions::new()
            .read(false)
            .access_mode(FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED)
            .open(folder)
    }

    /// One open folder's handle and its read. The buffer and `OVERLAPPED` are owned by the
    /// watcher thread only and held as raw pointers, since Windows writes into them while a read
    /// is pending.
    struct Watch {
        key: usize,
        folder: PathBuf,
        file: File,
        buffer: *mut [u32],
        overlapped: *mut OVERLAPPED,
        /// A read is started and its completion has not been taken from the port yet.
        pending: bool,
    }

    impl Watch {
        /// Associates the folder's handle with the port under `key`.
        fn new(key: usize, folder: PathBuf, file: File, port: &Port) -> Result<Self, u32> {
            // SAFETY: the file's handle and the port are both open; the key is a plain number.
            let associated =
                unsafe { CreateIoCompletionPort(file.as_raw_handle(), port.0, key, 0) };
            if associated.is_null() {
                // SAFETY: plain call, reading this thread's last error.
                return Err(unsafe { GetLastError() });
            }
            let buffer = Box::into_raw(vec![0u32; BUFFER_WORDS].into_boxed_slice());
            let overlapped = Box::into_raw(Box::new(OVERLAPPED::default()));
            Ok(Self {
                key,
                folder,
                file,
                buffer,
                overlapped,
                pending: false,
            })
        }

        /// Starts the next read: only the folder itself, only file names coming and going (ADR
        /// 0026 §2 point 7, F-Q7).
        fn start_read(&mut self) -> Result<(), u32> {
            // SAFETY: no read is pending (`pending` is false here), so Windows does not use the
            // `OVERLAPPED`; it is a live allocation owned by this watch.
            unsafe { *self.overlapped = OVERLAPPED::default() };
            // SAFETY: the handle is open; the buffer (`BUFFER_BYTES` long, DWORD-aligned) and
            // the `OVERLAPPED` are allocations owned by this watch, freed only after the read's
            // completion has been taken from the port (or leaked, `Drop`), so they stay valid as
            // long as Windows writes into them.
            let started = unsafe {
                ReadDirectoryChangesW(
                    self.file.as_raw_handle(),
                    self.buffer.cast::<c_void>(),
                    BUFFER_BYTES,
                    0,
                    FILE_NOTIFY_CHANGE_FILE_NAME,
                    std::ptr::null_mut(),
                    self.overlapped,
                    None,
                )
            };
            if started == 0 {
                // SAFETY: plain call, reading this thread's last error.
                return Err(unsafe { GetLastError() });
            }
            self.pending = true;
            Ok(())
        }

        /// The completed read's first `count` bytes.
        fn bytes(&self, count: u32) -> Vec<u8> {
            // SAFETY: called only after the read's completion was taken from the port, so
            // Windows no longer writes into the buffer; it is a live allocation of this watch.
            let words: &[u32] = unsafe { &*self.buffer };
            let count = (count as usize).min(BUFFER_WORDS * 4);
            words
                .iter()
                .take(count.div_ceil(4))
                .flat_map(|word| word.to_ne_bytes())
                .collect()
        }

        /// Asks Windows to cancel the pending read. True when its completion is still to come
        /// (cancelled, or already queued: `ERROR_NOT_FOUND`), false when there is none.
        fn cancel(&self) -> bool {
            if !self.pending {
                return false;
            }
            // SAFETY: the handle is open and the `OVERLAPPED` is the pending read's, still live.
            let cancelled = unsafe { CancelIoEx(self.file.as_raw_handle(), self.overlapped) };
            if cancelled == 0 {
                // SAFETY: plain call, reading this thread's last error.
                let error = unsafe { GetLastError() };
                if error != ERROR_NOT_FOUND {
                    eprintln!(
                        "pw-loupe: the watch of {} could not be cancelled: {}",
                        self.folder.display(),
                        reason(error)
                    );
                }
            }
            true
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            if self.pending {
                // The thread gave up waiting for this read's completion (at its own end): Windows
                // may still write into the buffer and the `OVERLAPPED`, so both are leaked
                // instead of freed. The handle closes after this, with the `File`.
                return;
            }
            // SAFETY: both were made by `Box::into_raw` in `new` and are freed once, here; no
            // read is pending, so Windows no longer uses them.
            unsafe {
                drop(Box::from_raw(self.buffer));
                drop(Box::from_raw(self.overlapped));
            }
        }
    }

    fn reason(error: u32) -> String {
        std::io::Error::from_raw_os_error(error as i32).to_string()
    }

    /// One folder in the set, by its first spelling.
    struct Entry {
        folder: PathBuf,
        state: State,
    }

    enum State {
        /// A helper thread is opening it; its `Opened` carries this key.
        Opening(usize),
        Live(Watch),
        /// Its watch could not start or stopped; kept so it is not opened again while it stays
        /// in the set.
        Ended,
    }

    struct WatchThread {
        port: Arc<Port>,
        /// For the helper threads' `Opened`.
        control: Sender<Control>,
        control_receiver: Receiver<Control>,
        events: Sender<WatchEvent>,
        wake: Wake,
        applied: Arc<Applied>,
        entries: Vec<Entry>,
        /// Watches whose folder left the set (or every watch, after `Stop`), each waiting for its
        /// cancelled read's completion before it is closed, at most until its deadline.
        ending: Vec<(Watch, Instant)>,
        /// The keys of opens still running on a helper thread for folders that left the set;
        /// each is closed as soon as it comes back.
        opens_to_close: Vec<usize>,
        quiet: QuietMoment,
        next_key: usize,
        /// The newest set applied, published once `may_publish` allows it.
        to_publish: Option<u64>,
        /// Set by `Stop`: the thread ends when `ending` is empty.
        stopping: bool,
    }

    impl WatchThread {
        fn run(&mut self) {
            loop {
                let now = Instant::now();
                let expired = take_expired(&mut self.ending, now);
                if !expired.is_empty() {
                    eprintln!(
                        "pw-loupe: the folder watcher gave up waiting for {} cancelled reads",
                        expired.len()
                    );
                    // Each still has its read pending, so its buffer and `OVERLAPPED` are leaked
                    // and its handle closed (`Watch`'s `Drop`).
                    drop(expired);
                }
                if may_publish(self.ending.len(), self.opens_to_close.len())
                    && let Some(number) = self.to_publish.take()
                {
                    self.applied.publish(number);
                }
                if self.stopping && self.ending.is_empty() {
                    return;
                }
                let mut wait = if self.stopping {
                    None
                } else {
                    self.quiet.next_due(now)
                };
                if let Some(left) = nearest_deadline(&self.ending, now) {
                    wait = Some(wait.map_or(left, |wait| wait.min(left)));
                }
                let timeout = wait.map_or(INFINITE, milliseconds_up);
                match self.port.wait(timeout) {
                    Packet::Control => self.drain_control(),
                    Packet::Timeout => {}
                    Packet::PortError(error) => {
                        eprintln!(
                            "pw-loupe: the folder watcher's completion port failed: {}",
                            reason(error)
                        );
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Packet::Completion {
                        key,
                        overlapped,
                        bytes,
                        error,
                    } => self.completed(key, overlapped, bytes, error),
                }
                if !self.stopping {
                    for folder in self.quiet.take_due(Instant::now()) {
                        self.send(WatchEvent::Changed(folder));
                    }
                }
            }
        }

        fn send(&self, event: WatchEvent) {
            if self.events.send(event).is_ok() {
                call_wake(&self.wake);
            }
        }

        fn drain_control(&mut self) {
            let mut newest: Option<(u64, Vec<PathBuf>)> = None;
            let mut opened = Vec::new();
            let mut stop = false;
            while let Ok(message) = self.control_receiver.try_recv() {
                match message {
                    Control::Set { number, folders } => {
                        if newest.as_ref().is_none_or(|(newest, _)| number > *newest) {
                            newest = Some((number, folders));
                        }
                    }
                    Control::Opened { key, result } => opened.push((key, result)),
                    Control::Stop => stop = true,
                }
            }
            if stop {
                self.stop();
                return;
            }
            if let Some((number, folders)) = newest {
                self.apply(&folders);
                self.to_publish = Some(number);
            }
            for (key, result) in opened {
                self.opened(key, result);
            }
        }

        fn stop(&mut self) {
            for entry in std::mem::take(&mut self.entries) {
                if let State::Live(watch) = entry.state {
                    self.begin_end(watch);
                }
            }
            self.quiet.clear();
            self.stopping = true;
        }

        /// Applies a new set: ends first, then opens.
        fn apply(&mut self, folders: &[PathBuf]) {
            let watched: Vec<PathBuf> = self
                .entries
                .iter()
                .map(|entry| entry.folder.clone())
                .collect();
            let change = plan_set(&watched, folders, same_folder);
            for entry in remove_indexes(&mut self.entries, change.end) {
                self.quiet.forget(&entry.folder);
                match entry.state {
                    State::Live(watch) => self.begin_end(watch),
                    State::Opening(key) => self.opens_to_close.push(key),
                    State::Ended => {}
                }
            }
            for folder in change.open {
                let key = self.next_key;
                let Some(next_key) = key.checked_add(1) else {
                    self.entries.push(Entry {
                        folder: folder.clone(),
                        state: State::Ended,
                    });
                    self.send(WatchEvent::Ended {
                        folder,
                        reason: "the folder watcher has no completion key left".to_string(),
                    });
                    continue;
                };
                self.next_key = next_key;
                match self.spawn_open(key, folder.clone()) {
                    Ok(()) => self.entries.push(Entry {
                        folder,
                        state: State::Opening(key),
                    }),
                    Err(error) => {
                        self.entries.push(Entry {
                            folder: folder.clone(),
                            state: State::Ended,
                        });
                        self.send(WatchEvent::Ended {
                            folder,
                            reason: error.to_string(),
                        });
                    }
                }
            }
        }

        /// Opens the folder on a short-lived helper thread (an unreachable network folder can
        /// block for the network's timeout), which sends the result back and wakes the thread.
        fn spawn_open(&self, key: usize, folder: PathBuf) -> std::io::Result<()> {
            let control = self.control.clone();
            let port = Arc::clone(&self.port);
            std::thread::Builder::new()
                .name("pw-folder-open".into())
                .spawn(move || {
                    let result = open_folder(&folder);
                    // When the watcher is gone, the `File` is dropped here.
                    if control.send(Control::Opened { key, result }).is_ok() {
                        port.post();
                    }
                })
                .map(|_| ())
        }

        /// A helper's open: dropped at once when its folder left the set.
        fn opened(&mut self, key: usize, result: std::io::Result<File>) {
            if let Some(index) = self.opens_to_close.iter().position(|close| *close == key) {
                self.opens_to_close.swap_remove(index);
                // The `File` (or the error) is dropped here, closing the handle.
                drop(result);
                return;
            }
            let Some(index) = self
                .entries
                .iter()
                .position(|entry| matches!(entry.state, State::Opening(opening) if opening == key))
            else {
                return;
            };
            let Some(entry) = self.entries.get_mut(index) else {
                return;
            };
            let started = result
                .map_err(|error| error.to_string())
                .and_then(|file| {
                    Watch::new(key, entry.folder.clone(), file, &self.port).map_err(reason)
                })
                .and_then(|mut watch| watch.start_read().map(|()| watch).map_err(reason));
            match started {
                Ok(watch) => {
                    entry.state = State::Live(watch);
                    let folder = entry.folder.clone();
                    self.send(WatchEvent::Started(folder));
                }
                Err(reason) => {
                    entry.state = State::Ended;
                    let folder = entry.folder.clone();
                    self.send(WatchEvent::Ended { folder, reason });
                }
            }
        }

        /// Ends a watch: closed at once with no read pending, otherwise cancelled and kept in
        /// `ending` until its read's completion comes (Windows writes into the buffer and the
        /// `OVERLAPPED` until then), at most [`STOP_WAIT`]; past that it is dropped with its
        /// read pending (leaked, `Watch`'s `Drop`).
        fn begin_end(&mut self, watch: Watch) {
            if watch.cancel() {
                self.ending.push((watch, Instant::now() + STOP_WAIT));
            }
        }

        fn completed(
            &mut self,
            key: usize,
            overlapped: *mut OVERLAPPED,
            bytes: u32,
            error: Option<u32>,
        ) {
            if let Some(index) = self
                .ending
                .iter()
                .position(|(watch, _)| watch.key == key && watch.overlapped == overlapped)
            {
                let (mut watch, _) = self.ending.swap_remove(index);
                watch.pending = false;
                return;
            }
            let Some(index) = self.entries.iter().position(|entry| {
                matches!(&entry.state, State::Live(watch)
                    if watch.key == key && watch.overlapped == overlapped)
            }) else {
                return;
            };
            let Some(Entry {
                folder,
                state: State::Live(watch),
            }) = self.entries.get_mut(index)
            else {
                return;
            };
            watch.pending = false;
            let changed = match error {
                Some(error) => match after_error(error) {
                    AfterError::ChangeAndReadAgain => Ok(true),
                    AfterError::End => Err(error),
                },
                None => Ok(counts_as_change(&watch.bytes(bytes), bytes as usize)),
            };
            let result = changed.and_then(|changed| {
                if changed {
                    self.quiet.notice(folder, Instant::now());
                }
                watch.start_read()
            });
            if let Err(error) = result {
                self.end_with_error(index, error);
            }
        }

        /// A watch that stopped: closed (no read pending) and reported with `Ended`.
        fn end_with_error(&mut self, index: usize, error: u32) {
            let Some(entry) = self.entries.get_mut(index) else {
                return;
            };
            entry.state = State::Ended;
            let folder = entry.folder.clone();
            self.quiet.forget(&folder);
            self.send(WatchEvent::Ended {
                folder,
                reason: reason(error),
            });
        }
    }

    /// A wait in whole milliseconds, rounded up (so the thread does not wake just before a
    /// folder is due), below `INFINITE`.
    fn milliseconds_up(wait: Duration) -> u32 {
        let milliseconds = wait.as_nanos().div_ceil(1_000_000);
        u32::try_from(milliseconds).map_or(INFINITE - 1, |ms| ms.min(INFINITE - 1))
    }
}

#[cfg(not(windows))]
mod sys {
    //! No thread elsewhere: every folder given is answered with `Ended`.

    use super::{Applied, Wake, WatchEvent};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::mpsc::Sender;
    use std::time::Duration;

    pub(super) struct Running(std::convert::Infallible);

    impl Running {
        pub(super) fn start(
            _events: Sender<WatchEvent>,
            _wake: Wake,
            _applied: Arc<Applied>,
        ) -> Result<Self, String> {
            Err("folder watching is not supported on this system".to_string())
        }

        pub(super) fn send_set(&self, _number: u64, _folders: Vec<PathBuf>) -> bool {
            match self.0 {}
        }

        pub(super) fn stop(self, _wait: Duration) {
            match self.0 {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // --- The quiet moment ---

    #[test]
    fn one_notification_is_due_after_the_quiet_moment_and_not_before() {
        let start = Instant::now();
        let folder = Path::new(r"C:\a");
        let mut quiet = QuietMoment::default();
        quiet.notice(folder, start);
        assert!(
            quiet
                .take_due(start + QUIET - Duration::from_millis(1))
                .is_empty()
        );
        assert_eq!(quiet.take_due(start + QUIET), vec![folder.to_path_buf()]);
    }

    #[test]
    fn notifications_that_keep_coming_are_due_two_seconds_after_the_first() {
        let start = Instant::now();
        let folder = Path::new(r"C:\a");
        let mut quiet = QuietMoment::default();
        let mut now = start;
        while now <= start + Duration::from_millis(1800) {
            quiet.notice(folder, now);
            assert!(quiet.take_due(now).is_empty());
            now += Duration::from_millis(300);
        }
        assert!(
            quiet
                .take_due(start + MOST - Duration::from_millis(1))
                .is_empty()
        );
        assert_eq!(quiet.take_due(start + MOST), vec![folder.to_path_buf()]);
    }

    #[test]
    fn two_folders_are_due_separately() {
        let start = Instant::now();
        let a = Path::new(r"C:\a");
        let b = Path::new(r"C:\b");
        let mut quiet = QuietMoment::default();
        quiet.notice(a, start);
        quiet.notice(b, start + Duration::from_millis(300));
        assert_eq!(quiet.take_due(start + QUIET), vec![a.to_path_buf()]);
        assert!(
            quiet
                .take_due(start + Duration::from_millis(799))
                .is_empty()
        );
        assert_eq!(
            quiet.take_due(start + Duration::from_millis(800)),
            vec![b.to_path_buf()]
        );
    }

    #[test]
    fn next_due_is_none_when_nothing_waits_and_the_shortest_wait_otherwise() {
        let start = Instant::now();
        let mut quiet = QuietMoment::default();
        assert_eq!(quiet.next_due(start), None);
        quiet.notice(Path::new(r"C:\a"), start);
        quiet.notice(Path::new(r"C:\b"), start + Duration::from_millis(200));
        assert_eq!(
            quiet.next_due(start + Duration::from_millis(100)),
            Some(Duration::from_millis(400))
        );
        assert_eq!(
            quiet.next_due(start + Duration::from_millis(600)),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn a_folder_taken_is_not_due_again_until_noticed_again() {
        let start = Instant::now();
        let folder = Path::new(r"C:\a");
        let mut quiet = QuietMoment::default();
        quiet.notice(folder, start);
        assert_eq!(quiet.take_due(start + QUIET).len(), 1);
        assert!(quiet.take_due(start + Duration::from_secs(10)).is_empty());
        assert_eq!(quiet.next_due(start + Duration::from_secs(10)), None);
        let later = start + Duration::from_secs(11);
        quiet.notice(folder, later);
        assert_eq!(quiet.take_due(later + QUIET), vec![folder.to_path_buf()]);
    }

    // --- The notification buffer ---

    /// One `FILE_NOTIFY_INFORMATION` record; `next` 0 for the last.
    fn record(next: u32, name: &str) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&next.to_ne_bytes());
        bytes.extend_from_slice(&1u32.to_ne_bytes()); // FILE_ACTION_ADDED
        bytes.extend_from_slice(&((units.len() * 2) as u32).to_ne_bytes());
        for unit in units {
            bytes.extend_from_slice(&unit.to_ne_bytes());
        }
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        bytes
    }

    fn records(names: &[&str]) -> Vec<u8> {
        let mut buffer = Vec::new();
        for (index, name) in names.iter().enumerate() {
            let last = index + 1 == names.len();
            let length = record(0, name).len() as u32;
            buffer.extend(record(if last { 0 } else { length }, name));
        }
        buffer
    }

    #[test]
    fn a_record_naming_an_image_counts() {
        let buffer = records(&["a.ARW"]);
        assert!(counts_as_change(&buffer, buffer.len()));
    }

    #[test]
    fn records_naming_only_sidecars_and_temporary_files_do_not_count() {
        let buffer = records(&["a.xmp", "a.pwtmp-0f.jpg", "a.pwbak-0f.jpg"]);
        assert!(!counts_as_change(&buffer, buffer.len()));
    }

    #[test]
    fn the_second_of_two_records_counting_is_found() {
        let buffer = records(&["a.xmp", "b.jpg"]);
        assert!(counts_as_change(&buffer, buffer.len()));
        let buffer = records(&["a.xmp", "b.txt"]);
        assert!(!counts_as_change(&buffer, buffer.len()));
    }

    #[test]
    fn zero_bytes_count() {
        let buffer = records(&["a.xmp"]);
        assert!(counts_as_change(&buffer, 0));
        assert!(counts_as_change(&[], 0));
    }

    #[test]
    fn an_offset_or_length_past_the_byte_count_counts_without_reading_past_it() {
        // `NextEntryOffset` past the byte count; the bytes after it would name an image.
        let first = record(0, "a.xmp");
        let mut buffer = record(first.len() as u32 + 64, "a.xmp");
        let count = buffer.len();
        buffer.extend(records(&["x.txt"]));
        assert!(counts_as_change(&buffer, count));
        // `FileNameLength` past the byte count: the name is cut by the count.
        let buffer = records(&["a.xmp"]);
        assert!(counts_as_change(&buffer, buffer.len() - 4));
        let mut buffer = records(&["a.xmp"]);
        buffer[8..12].copy_from_slice(&1000u32.to_ne_bytes());
        assert!(counts_as_change(&buffer, buffer.len()));
        // A header cut short.
        assert!(counts_as_change(&buffer, 6));
    }

    #[test]
    fn a_byte_count_larger_than_the_buffer_is_clamped() {
        let buffer = records(&["a.xmp"]);
        assert!(!counts_as_change(&buffer, buffer.len() + 4096));
        let buffer = records(&["a.jpg"]);
        assert!(counts_as_change(&buffer, usize::MAX));
    }

    // --- The set ---

    fn same_ignoring_case(a: &Path, b: &Path) -> bool {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_folder_given_twice_in_two_spellings_is_opened_once() {
        let change = plan_set(&[], &paths(&[r"C:\A", r"c:\a"]), same_ignoring_case);
        assert_eq!(change.open, paths(&[r"C:\A"]));
        assert!(change.end.is_empty());
        let change = plan_set(&paths(&[r"C:\A"]), &paths(&[r"c:\a"]), same_ignoring_case);
        assert_eq!(change, SetChange::default());
    }

    #[test]
    fn a_folder_left_out_is_ended() {
        let watched = paths(&[r"C:\a", r"C:\b", r"C:\c"]);
        let change = plan_set(&watched, &paths(&[r"C:\B", r"C:\d"]), same_ignoring_case);
        assert_eq!(change.open, paths(&[r"C:\d"]));
        assert_eq!(change.end, vec![0, 2]);
    }

    #[test]
    fn an_unchanged_set_opens_and_ends_nothing() {
        let watched = paths(&[r"C:\a", r"C:\b"]);
        assert_eq!(
            plan_set(&watched, &watched, same_ignoring_case),
            SetChange::default()
        );
    }

    // --- Generations ---

    #[test]
    fn a_wait_is_answered_only_by_its_own_number_or_a_later_one() {
        let applied = Arc::new(Applied::default());
        applied.publish(2);
        assert!(!applied.wait_for(3, Duration::from_millis(100)));
        let publisher = Arc::clone(&applied);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            publisher.publish(3);
        });
        assert!(applied.wait_for(3, Duration::from_secs(5)));
        thread.join().unwrap();
        let applied = Applied::default();
        applied.publish(4);
        assert!(applied.wait_for(3, Duration::ZERO));
        // Never lowered.
        applied.publish(1);
        assert!(applied.wait_for(4, Duration::ZERO));
    }

    #[test]
    fn a_wait_times_out_with_false_when_nothing_is_published() {
        let applied = Applied::default();
        let start = Instant::now();
        assert!(!applied.wait_for(1, Duration::from_millis(100)));
        assert!(start.elapsed() >= Duration::from_millis(100));
    }

    #[test]
    fn wait_until_applied_with_no_change_returns_true_at_once() {
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        assert!(watcher.wait_until_applied(Duration::ZERO));
        watcher.set_folders(Vec::new());
        assert!(watcher.wait_until_applied(Duration::from_secs(2)));
        assert!(watcher.wait_until_applied(Duration::ZERO));
    }

    // --- Publishing, ending watches and error completions ---

    #[test]
    fn a_set_is_published_only_with_no_watch_ending_and_no_open_to_close() {
        assert!(may_publish(0, 0));
        assert!(!may_publish(1, 0));
        assert!(!may_publish(0, 1));
        assert!(!may_publish(2, 3));
    }

    #[test]
    fn an_ending_watch_past_its_deadline_is_taken_out() {
        let start = Instant::now();
        let mut ending = vec![
            ('a', start + Duration::from_millis(100)),
            ('b', start + STOP_WAIT),
        ];
        assert_eq!(
            nearest_deadline(&ending, start),
            Some(Duration::from_millis(100))
        );
        assert!(take_expired(&mut ending, start).is_empty());
        assert_eq!(
            take_expired(&mut ending, start + Duration::from_millis(100)),
            vec!['a']
        );
        assert_eq!(
            nearest_deadline(&ending, start + Duration::from_millis(100)),
            Some(STOP_WAIT - Duration::from_millis(100))
        );
        assert_eq!(take_expired(&mut ending, start + STOP_WAIT), vec!['b']);
        assert!(ending.is_empty());
        assert_eq!(nearest_deadline(&ending, start), None);
    }

    #[test]
    fn notify_enum_dir_is_a_change_and_other_errors_end_the_watch() {
        assert_eq!(after_error(1022), AfterError::ChangeAndReadAgain);
        assert_eq!(after_error(995), AfterError::End); // ERROR_OPERATION_ABORTED
        assert_eq!(after_error(5), AfterError::End); // ERROR_ACCESS_DENIED
        assert_eq!(after_error(64), AfterError::End); // ERROR_NETNAME_DELETED
    }

    // --- Windows, on real temporary folders ---

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pw-loupe-watcher-{}-{id}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn folder(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const POLL: Duration = Duration::from_millis(20);

    /// The events that come within `limit`, polling every 20 ms; stops at the first event `stop`
    /// accepts.
    fn events_until(
        watcher: &mut FolderWatcher,
        limit: Duration,
        stop: impl Fn(&WatchEvent) -> bool,
    ) -> Vec<WatchEvent> {
        let start = Instant::now();
        let mut events = Vec::new();
        while start.elapsed() < limit {
            while let Some(event) = watcher.try_recv() {
                let done = stop(&event);
                events.push(event);
                if done {
                    return events;
                }
            }
            std::thread::sleep(POLL);
        }
        while let Some(event) = watcher.try_recv() {
            events.push(event);
        }
        events
    }

    fn events_for(watcher: &mut FolderWatcher, limit: Duration) -> Vec<WatchEvent> {
        events_until(watcher, limit, |_| false)
    }

    fn changed(folder: &Path) -> WatchEvent {
        WatchEvent::Changed(folder.to_path_buf())
    }

    fn is_started(event: &WatchEvent) -> bool {
        matches!(event, WatchEvent::Started(_))
    }

    /// The events without `Started` (T249: for tests about `Changed` and `Ended`, where a watch
    /// that becomes live also sends `Started`).
    fn ignoring_started(events: Vec<WatchEvent>) -> Vec<WatchEvent> {
        events.into_iter().filter(|e| !is_started(e)).collect()
    }

    fn touch(path: &Path) {
        std::fs::write(path, b"").unwrap();
    }

    /// Waits until the watch of `folder` is open (it opens on a helper thread, so a file made at
    /// once could come before the read): makes probe files until a `Changed` comes, then waits
    /// until the watcher has been quiet for 1.5 s (longer than the quiet moment).
    #[cfg(windows)]
    fn wait_until_watching(watcher: &mut FolderWatcher, folder: &Path) {
        let start = Instant::now();
        let mut probe = 0;
        loop {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the watch of {} did not open",
                folder.display()
            );
            touch(&folder.join(format!("probe-{probe}.jpg")));
            probe += 1;
            let events = ignoring_started(events_until(watcher, Duration::from_secs(1), |e| {
                !is_started(e)
            }));
            if events.contains(&changed(folder)) {
                break;
            }
            assert!(events.is_empty(), "unexpected events: {events:?}");
        }
        while !events_for(watcher, Duration::from_millis(1500)).is_empty() {}
    }

    /// Retries `act` for up to 2 s (the search indexer or a virus scanner may hold a handle
    /// under `%TEMP%` for a moment).
    #[cfg(windows)]
    fn retried(act: impl Fn() -> std::io::Result<()>) -> std::io::Result<()> {
        let start = Instant::now();
        loop {
            match act() {
                Ok(()) => return Ok(()),
                Err(error) if start.elapsed() >= Duration::from_secs(2) => return Err(error),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn creating_an_image_gives_one_changed_naming_the_folder_as_given() {
        let temp = TempDir::new();
        let folder = temp.folder("given");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        wait_until_watching(&mut watcher, &folder);
        touch(&folder.join("x.jpg"));
        let events = events_until(&mut watcher, Duration::from_secs(5), |_| true);
        assert_eq!(events, vec![changed(&folder)]);
        assert_eq!(events_for(&mut watcher, Duration::from_secs(2)), vec![]);
    }

    #[cfg(windows)]
    #[test]
    fn twenty_images_give_at_most_three_changed() {
        let temp = TempDir::new();
        let folder = temp.folder("twenty");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        wait_until_watching(&mut watcher, &folder);
        let start = Instant::now();
        let mut events = Vec::new();
        let mut first_at = None;
        let mut take = |watcher: &mut FolderWatcher, events: &mut Vec<WatchEvent>| {
            while let Some(event) = watcher.try_recv() {
                first_at.get_or_insert(start.elapsed());
                events.push(event);
            }
            first_at
        };
        for index in 0..20 {
            touch(&folder.join(format!("image-{index:02}.jpg")));
            take(&mut watcher, &mut events);
        }
        let last_file = Instant::now();
        // Counted until 3 s after the last file, longer if the first has not come (up to 5 s).
        while last_file.elapsed() < Duration::from_secs(3)
            || (take(&mut watcher, &mut events).is_none()
                && start.elapsed() < Duration::from_secs(5))
        {
            take(&mut watcher, &mut events);
            std::thread::sleep(POLL);
        }
        let first_at = take(&mut watcher, &mut events);
        assert!(
            first_at.is_some_and(|at| at <= Duration::from_secs(5)),
            "no Changed within 5 s: {events:?}"
        );
        assert!(
            events.iter().all(|event| *event == changed(&folder)),
            "{events:?}"
        );
        assert!(events.len() <= 3, "{} Changed: {events:?}", events.len());
    }

    #[cfg(windows)]
    #[test]
    fn deleting_or_renaming_an_image_gives_changed() {
        let temp = TempDir::new();
        let folder = temp.folder("delete-rename");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        wait_until_watching(&mut watcher, &folder);
        // `wait_until_watching`'s probes are listed images.
        std::fs::remove_file(folder.join("probe-0.jpg")).unwrap();
        let events = events_until(&mut watcher, Duration::from_secs(5), |_| true);
        assert_eq!(events, vec![changed(&folder)]);
        while !events_for(&mut watcher, Duration::from_millis(1500)).is_empty() {}
        touch(&folder.join("x.jpg"));
        assert_eq!(
            events_until(&mut watcher, Duration::from_secs(5), |_| true),
            vec![changed(&folder)]
        );
        while !events_for(&mut watcher, Duration::from_millis(1500)).is_empty() {}
        std::fs::rename(folder.join("x.jpg"), folder.join("y.jpg")).unwrap();
        assert_eq!(
            events_until(&mut watcher, Duration::from_secs(5), |_| true),
            vec![changed(&folder)]
        );
    }

    #[cfg(windows)]
    #[test]
    fn sidecars_and_temporary_files_give_no_changed() {
        let temp = TempDir::new();
        let folder = temp.folder("sidecars");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        wait_until_watching(&mut watcher, &folder);
        touch(&folder.join("x.xmp"));
        touch(&folder.join("x.pwtmp-0f.jpg"));
        assert_eq!(
            events_for(&mut watcher, Duration::from_millis(1500)),
            vec![]
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_file_in_a_subfolder_gives_no_changed() {
        let temp = TempDir::new();
        let folder = temp.folder("parent-of-sub");
        let sub = temp.folder("parent-of-sub/sub");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        wait_until_watching(&mut watcher, &folder);
        touch(&sub.join("x.jpg"));
        assert_eq!(
            events_for(&mut watcher, Duration::from_millis(1500)),
            vec![]
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_that_does_not_exist_gives_ended_and_nothing_else() {
        let temp = TempDir::new();
        let folder = temp.0.join("missing");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        let events = events_until(&mut watcher, Duration::from_secs(5), |_| true);
        assert!(
            matches!(events.as_slice(), [WatchEvent::Ended { folder: ended, .. }] if *ended == folder),
            "{events:?}"
        );
        // Given again while still in the set, it is not opened again.
        watcher.set_folders(vec![folder.clone()]);
        assert_eq!(events_for(&mut watcher, Duration::from_secs(1)), vec![]);
    }

    /// Tries to rename the watched folder's parent once while it is watched and logs the result
    /// (to report, ADR 0026 F-Q4; not an assertion). Renames it back if it worked.
    #[cfg(windows)]
    fn report_parent_rename(parent: &Path) {
        let renamed = parent.with_extension("renamed");
        match std::fs::rename(parent, &renamed) {
            Ok(()) => {
                eprintln!(
                    "pw-loupe watcher test: renaming the watched folder's parent worked while watched"
                );
                std::fs::rename(&renamed, parent).unwrap();
            }
            Err(error) => eprintln!(
                "pw-loupe watcher test: renaming the watched folder's parent failed while watched: {error}"
            ),
        }
    }

    #[cfg(windows)]
    #[test]
    fn an_ended_watch_lets_the_parent_be_renamed_and_the_folder_deleted() {
        let temp = TempDir::new();
        for (round, by_drop) in [(0, false), (1, true)] {
            let parent = temp.folder(&format!("parent-{round}"));
            let folder = temp.folder(&format!("parent-{round}/watched"));
            let mut watcher = FolderWatcher::start(Box::new(|| {}));
            watcher.set_folders(vec![folder.clone()]);
            wait_until_watching(&mut watcher, &folder);
            report_parent_rename(&parent);
            if by_drop {
                drop(watcher);
            } else {
                watcher.set_folders(Vec::new());
                assert!(watcher.wait_until_applied(Duration::from_secs(2)));
            }
            let moved = temp.0.join(format!("moved-{round}"));
            retried(|| std::fs::rename(&parent, &moved)).unwrap();
            retried(|| std::fs::remove_dir_all(moved.join("watched"))).unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_watched_folder_deleted_gives_ended() {
        let temp = TempDir::new();
        let folder = temp.folder("deleted");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        wait_until_watching(&mut watcher, &folder);
        retried(|| std::fs::remove_dir_all(&folder)).unwrap();
        let events = events_until(&mut watcher, Duration::from_secs(5), |event| {
            matches!(event, WatchEvent::Ended { .. })
        });
        let (last, before) = events.split_last().expect("no event within 5 s");
        assert!(
            matches!(last, WatchEvent::Ended { folder: ended, .. } if *ended == folder),
            "no Ended within 5 s: {events:?}"
        );
        assert!(
            before.iter().all(|event| *event == changed(&folder)),
            "{events:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_removed_at_once_while_opening_can_be_deleted_once_applied() {
        let temp = TempDir::new();
        let parent = temp.folder("removed-at-once");
        let folder = temp.folder("removed-at-once/watched");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        watcher.set_folders(Vec::new());
        assert!(watcher.wait_until_applied(Duration::from_secs(2)));
        // The share mode lets a watched folder be deleted, but not its parent be renamed (see
        // `report_parent_rename`), so the rename shows that no handle is left open.
        let moved = temp.0.join("removed-at-once-moved");
        retried(|| std::fs::rename(&parent, &moved)).unwrap();
        retried(|| std::fs::remove_dir_all(moved.join("watched"))).unwrap();
        assert_eq!(
            ignoring_started(events_for(&mut watcher, Duration::from_millis(500))),
            vec![]
        );
    }

    #[cfg(windows)]
    #[test]
    fn quick_sets_apply_only_the_newest() {
        let temp = TempDir::new();
        let a = temp.folder("a");
        let b = temp.folder("b");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![a.clone()]);
        watcher.set_folders(vec![a.clone(), b.clone()]);
        watcher.set_folders(vec![b.clone()]);
        assert!(watcher.wait_until_applied(Duration::from_secs(2)));
        wait_until_watching(&mut watcher, &b);
        touch(&a.join("x.jpg"));
        assert_eq!(
            ignoring_started(events_for(&mut watcher, Duration::from_millis(1500))),
            vec![]
        );
        touch(&b.join("x.jpg"));
        assert_eq!(
            events_until(&mut watcher, Duration::from_secs(5), |_| true),
            vec![changed(&b)]
        );
    }

    // --- `Started` (T249) ---

    #[cfg(windows)]
    #[test]
    fn a_folder_given_gets_started_before_any_changed_and_then_sees_a_new_file() {
        let temp = TempDir::new();
        let folder = temp.folder("started");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        let events = events_until(&mut watcher, Duration::from_secs(5), |_| true);
        assert_eq!(events, vec![WatchEvent::Started(folder.clone())]);
        // A file made right after `Started` is seen.
        touch(&folder.join("x.jpg"));
        assert_eq!(
            events_until(&mut watcher, Duration::from_secs(5), |_| true),
            vec![changed(&folder)]
        );
        assert_eq!(events_for(&mut watcher, Duration::from_secs(1)), vec![]);
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_that_does_not_exist_gets_no_started() {
        let temp = TempDir::new();
        let folder = temp.0.join("missing-started");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        let events = events_for(&mut watcher, Duration::from_secs(2));
        assert!(
            matches!(events.as_slice(), [WatchEvent::Ended { folder: ended, .. }] if *ended == folder),
            "{events:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_taken_out_and_given_again_gets_a_second_started() {
        let temp = TempDir::new();
        let folder = temp.folder("again");
        let mut watcher = FolderWatcher::start(Box::new(|| {}));
        watcher.set_folders(vec![folder.clone()]);
        assert_eq!(
            events_until(&mut watcher, Duration::from_secs(5), |_| true),
            vec![WatchEvent::Started(folder.clone())]
        );
        watcher.set_folders(Vec::new());
        assert!(watcher.wait_until_applied(Duration::from_secs(2)));
        assert_eq!(events_for(&mut watcher, Duration::from_millis(500)), vec![]);
        watcher.set_folders(vec![folder.clone()]);
        assert_eq!(
            events_until(&mut watcher, Duration::from_secs(5), |_| true),
            vec![WatchEvent::Started(folder.clone())]
        );
    }

    #[test]
    fn without_a_thread_only_ended_is_sent() {
        let (events_sender, events) = mpsc::channel();
        let mut watcher = FolderWatcher {
            events_sender,
            events,
            wake: Arc::new(Mutex::new(Box::new(|| {}))),
            applied: Arc::new(Applied::default()),
            number: 0,
            running: None,
            unwatched: Vec::new(),
            reason: "no thread".to_string(),
        };
        let temp = TempDir::new();
        let folder = temp.folder("no-thread");
        watcher.set_folders(vec![folder.clone()]);
        let mut received = Vec::new();
        while let Some(event) = watcher.try_recv() {
            received.push(event);
        }
        assert_eq!(
            received,
            vec![WatchEvent::Ended {
                folder,
                reason: "no thread".to_string()
            }]
        );
    }
}
