//! A background thread that owns the ExifTool process and runs metadata requests.
//!
//! Several write processes (ADR 0007 §9.6 point 2, Q18; T266): [`WriteSetup::write_processes`]
//! is the number of ExifTool processes that write (1..=4). With 1 the worker thread runs every
//! request itself, one at a time and in order. With more, and a setup, the worker thread starts
//! a [`WritePool`] of that many processes at its first write and gives it the rating, orientation
//! and edit writes, each its own safe-write transaction, keyed by its image ([`write_key`]); it
//! keeps its own process for everything else. Writes of different images may then answer in
//! another order than they were asked; writes of one image keep their order, and a request that
//! is not a write starts only when every earlier write has answered and its event has been sent.
//!
//! A rating is written through the write plan and the safe write (ADR 0007 §9, §15; T91):
//! [`ExifTool::write_rating_safely`] with the [`WriteSetup`] given at the start. Without a setup
//! (the program could not open its write journal) every rating write fails with
//! [`WriteError::NoJournal`] and nothing is written; reading ratings works either way.
//!
//! The ExifTool version (ADR 0007 §11, D9; T94): the worker thread starts ExifTool at once and
//! asks [`ExifTool::version`] each time it starts a process. The first result is kept for
//! [`MetadataWorker::exiftool_version`]. While the running process is older than
//! [`MIN_EXIFTOOL_VERSION`] or its version could not be read, every rating write fails with
//! [`WriteError::ExifToolTooOld`] and nothing is written; reading ratings works as before.
//!
//! All metadata, edits and options (ADR 0007 I10e; T98):
//! - [`MetadataWorker::read_metadata`] reads and reconciles all metadata of one image
//!   ([`ExifTool::read_metadata`]) and answers with [`Event::MetadataRead`].
//! - [`MetadataWorker::write_edits`] writes the editor's edits through
//!   [`ExifTool::write_edits_safely`] and answers with [`Event::EditsWritten`]. It fails as a
//!   rating write does: [`WriteError::NoJournal`] without a setup, [`WriteError::ExifToolTooOld`]
//!   with a too old ExifTool.
//! - [`MetadataWorker::write_orientation`] writes an Exif orientation through
//!   [`ExifTool::write_orientation_safely`] in the same queue as the ratings (ADR 0024 §2.5) and
//!   answers with [`Event::OrientationWritten`]; it fails as a rating write does.
//! - [`MetadataWorker::set_options`] changes the [`EditOptions`] without an event. The worker
//!   starts with the setup's options (the default without a setup); every request queued after
//!   `set_options` uses the new options for rating writes, edits and reads (the `fallback`).
//!   Requests are taken from the queue in order, so a request queued before it uses the old
//!   options (with several write processes too: a write takes the options current when it is
//!   taken from the queue).
//!
//! Moving an image with its companion files (ADR 0017 §2.6, step U9b; T170):
//! - [`MetadataWorker::move_image`] finds the image's group with [`relocate::companions`],
//!   releases the view cores' sessions of the image files in it
//!   ([`ImageCheck::release_for_move`], T177: no write follows, so nothing is noted for one) and
//!   moves the group with [`relocate::move_group`]; answered with [`Event::Moved`].
//! - [`MetadataWorker::move_back`] releases every moved file (also with
//!   [`ImageCheck::release_for_move`]) and moves the files back with
//!   [`relocate::move_back`]; answered with [`Event::MovedBack`].
//!
//! Deleting an image with its companion files to the Recycle Bin (ADR 0018 §2.3, step B2; T186):
//! - [`MetadataWorker::prepare_delete`] finds each image's group with [`relocate::companions`]
//!   and its size with [`recycle::inspect_group`], for the confirmation; it releases nothing and
//!   changes nothing on disk. Answered with [`Event::DeletePrepared`].
//! - [`MetadataWorker::delete_image`] finds the image's group, releases the view cores' sessions
//!   of the image files in it as a move does ([`ImageCheck::release_for_move`]) and sends the
//!   group to the Recycle Bin with [`recycle::delete_group`]; answered with [`Event::Deleted`].
//!
//! Both answer with a structured [`DeleteError`].
//!
//! A move or a delete sent after a rating write of the same image runs after it, and its event
//! comes after the write's: with one write process because requests run in order, with several
//! because a request that is not a write waits for every earlier write's event. Moves and deletes
//! do not use ExifTool; without a [`WriteSetup`] (no `checks` to release the sessions) the moves
//! and `delete_image` fail with [`NO_JOURNAL`] and nothing is moved or deleted. `prepare_delete`
//! works without a setup.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::edit::{Edit, EditOptions, EditReport};
use crate::exiftool::{ToolVersion, exiftool_program};
use crate::journal::Journal;
use crate::read::MetadataRead;
use crate::recycle::{self, DeleteError, DeleteRefusal};
use crate::relocate::{self, Moved};
use crate::safe_write::ImageCheck;
use crate::write_pool::{
    JobAnswer, Running, Ticket, VersionResult, WritePool, start_tool, write_key,
};
use crate::{ExifTool, Rating, Result, WriteError};

const REQUEST_CAPACITY: usize = 8;

/// The most write processes of a worker (ADR 0007 §9.6 point 2).
const MAX_WRITE_PROCESSES: usize = 4;

/// How long the worker waits for a pool answer before it looks at the request queue again, while
/// writes are outstanding (T266).
const ANSWER_POLL: Duration = Duration::from_millis(5);

/// The text of [`WriteError::NoJournal`]: a write when the worker has no [`WriteSetup`].
pub const NO_JOURNAL: &str = "the write journal could not be opened";

/// The text of [`WriteError::ExifToolTooOld`]: a write while the running ExifTool is older than
/// [`MIN_EXIFTOOL_VERSION`] or its version could not be read (D9).
pub const EXIFTOOL_TOO_OLD: &str = "ExifTool is older than 13.64";

/// What a rating write needs (T91): the app's write journal, its image checks and the edit
/// options. `options` are the options the worker starts with; [`MetadataWorker::set_options`]
/// changes them later (T98).
pub struct WriteSetup {
    pub journal: Journal,
    /// Shared by the write processes' threads when there are several (T266).
    pub checks: Arc<dyn ImageCheck + Send + Sync>,
    pub options: EditOptions,
    /// The number of ExifTool processes that write (ADR 0007 §9.6 point 2, Q18; T266), clamped
    /// to 1..=4 when the worker starts (0 counts as 1). With 1 the worker writes with its own
    /// process, one request at a time, as before; with more it gives its writes to a
    /// [`WritePool`] of that many processes.
    pub write_processes: usize,
}

impl fmt::Debug for WriteSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteSetup")
            .field("journal", &self.journal)
            .field("options", &self.options)
            .field("write_processes", &self.write_processes)
            .finish_non_exhaustive()
    }
}

pub type RequestId = u64;

/// One image's group for a delete (T186): its files as [`relocate::companions`] gives them, the
/// image first, and their total size in bytes from [`recycle::inspect_group`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteGroup {
    pub files: Vec<PathBuf>,
    pub bytes: u64,
}

/// The answer for one image of [`MetadataWorker::prepare_delete`] (T186): the image as sent, and
/// its group or why it cannot be deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedDelete {
    pub image: PathBuf,
    pub result: std::result::Result<DeleteGroup, DeleteError>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError {
    QueueFull,
    Stopped,
}

#[derive(Debug)]
pub enum Event {
    RatingsRead {
        id: RequestId,
        result: std::result::Result<HashMap<PathBuf, Rating>, String>,
        /// The `XMP-tiff:Orientation` (1–8) of the images that have one, read in the same call
        /// (ADR 0024 §2.4); empty when the read failed.
        orientations: HashMap<PathBuf, u16>,
    },
    RatingWritten {
        id: RequestId,
        path: PathBuf,
        rating: Rating,
        /// The kind of failure is kept, so the program can show its own text (T49).
        result: std::result::Result<(), WriteError>,
    },
    /// The answer to [`MetadataWorker::write_orientation`] (ADR 0024 §2.5, T233), with `path` and
    /// `orientation` as they were sent. `Ok` also when the value was already in the file.
    OrientationWritten {
        id: RequestId,
        path: PathBuf,
        orientation: u16,
        result: std::result::Result<(), WriteError>,
    },
    /// The answer to [`MetadataWorker::read_metadata`] (T98).
    MetadataRead {
        id: RequestId,
        path: PathBuf,
        result: std::result::Result<MetadataRead, String>,
    },
    /// The answer to [`MetadataWorker::write_edits`] (T98).
    EditsWritten {
        id: RequestId,
        path: PathBuf,
        result: std::result::Result<EditReport, WriteError>,
    },
    /// The answer to [`MetadataWorker::move_image`] (T170): the moves in order, or the error as
    /// text (the folder read's error with the image's name, or [`relocate::MoveError`]'s text).
    Moved {
        id: RequestId,
        image: PathBuf,
        result: std::result::Result<Vec<Moved>, String>,
    },
    /// The answer to [`MetadataWorker::move_back`] (T170), with `moved` as it was sent.
    MovedBack {
        id: RequestId,
        moved: Vec<Moved>,
        result: std::result::Result<(), String>,
    },
    /// The answer to [`MetadataWorker::prepare_delete`] (T186): one entry per image, in the order
    /// sent.
    DeletePrepared {
        id: RequestId,
        images: Vec<PreparedDelete>,
    },
    /// The answer to [`MetadataWorker::delete_image`] (T186), with `image` as it was sent: the
    /// files that went to the Recycle Bin, or the error.
    Deleted {
        id: RequestId,
        image: PathBuf,
        result: std::result::Result<Vec<PathBuf>, DeleteError>,
    },
}

/// What the worker thread receives: a request with an event, or new options without one (T98).
#[derive(Debug)]
enum Command {
    Run(Request),
    SetOptions(EditOptions),
}

#[derive(Debug)]
enum Request {
    Read {
        id: RequestId,
        images: Vec<PathBuf>,
    },
    Write {
        id: RequestId,
        path: PathBuf,
        rating: Rating,
    },
    WriteOrientation {
        id: RequestId,
        path: PathBuf,
        orientation: u16,
    },
    ReadMetadata {
        id: RequestId,
        path: PathBuf,
    },
    WriteEdits {
        id: RequestId,
        path: PathBuf,
        edits: Vec<Edit>,
    },
    Move {
        id: RequestId,
        image: PathBuf,
        target: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    },
    MoveBack {
        id: RequestId,
        moved: Vec<Moved>,
    },
    PrepareDelete {
        id: RequestId,
        images: Vec<PathBuf>,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    },
    Delete {
        id: RequestId,
        image: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    },
}

/// Bounded metadata I/O for UI clients. The ExifTool process starts on this worker.
///
/// With one write process ([`WriteSetup::write_processes`]) every request runs in order. With
/// several (T266), writes of different images may answer in another order than they were asked,
/// while one image's writes, and everything sent after a write, keep their order.
///
/// The worker recovers on its own: if ExifTool cannot be started, the start is retried with the
/// next request, and if it crashes or its output stream breaks, the process is discarded and a
/// new one is started for the next request. Each failed request is still reported as an error.
#[derive(Debug)]
pub struct MetadataWorker {
    requests: Option<SyncSender<Command>>,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
    next_id: RequestId,
    /// Set once by the worker thread after its first start (T94).
    version: Arc<OnceLock<VersionResult>>,
    /// Set when the worker is dropped: with several write processes the worker thread takes no
    /// further request from the queue (T266).
    stopped: Arc<AtomicBool>,
}

impl MetadataWorker {
    /// Starts a worker using the ExifTool given by [`exiftool_program`]: the copy in the program
    /// folder, or `exiftool` from PATH. `setup` is `None` when the write journal could not be
    /// opened; rating writes then fail with [`WriteError::NoJournal`].
    pub fn start(setup: Option<WriteSetup>) -> Result<Self> {
        Self::start_with(&exiftool_program(), &[], setup)
    }

    /// Starts a worker using an explicit ExifTool program and leading arguments.
    pub fn start_with(program: &str, pre_args: &[&str], setup: Option<WriteSetup>) -> Result<Self> {
        let (request_tx, request_rx) = mpsc::sync_channel(REQUEST_CAPACITY);
        let (event_tx, event_rx) = mpsc::sync_channel(REQUEST_CAPACITY);
        let program = program.to_string();
        let pre_args: Vec<String> = pre_args.iter().map(|arg| (*arg).to_string()).collect();
        let version = Arc::new(OnceLock::new());
        let first_version = Arc::clone(&version);
        let stopped = Arc::new(AtomicBool::new(false));
        let thread_stopped = Arc::clone(&stopped);
        let thread = std::thread::Builder::new()
            .name("pw-metadata".into())
            .spawn(move || {
                run(
                    &program,
                    &pre_args,
                    setup.as_ref(),
                    &first_version,
                    &request_rx,
                    &event_tx,
                    &thread_stopped,
                );
            })?;
        Ok(Self {
            requests: Some(request_tx),
            events: event_rx,
            thread: Some(thread),
            next_id: 0,
            version,
            stopped,
        })
    }

    /// The version of the first ExifTool process the worker started: `None` until the first
    /// start has been tried, then the version, or the start or version error as text. Later
    /// restarts do not change it.
    pub fn exiftool_version(&self) -> Option<std::result::Result<ToolVersion, String>> {
        self.version.get().cloned()
    }

    /// Queues a batch read without blocking. The bounded queue reports back-pressure explicitly.
    pub fn read_ratings(
        &mut self,
        images: Vec<PathBuf>,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Read { id, images })?;
        Ok(id)
    }

    /// Queues a rating update without blocking. Writes of one image run in request order (with
    /// one write process, all writes do).
    pub fn write_rating(
        &mut self,
        path: PathBuf,
        rating: Rating,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Write { id, path, rating })?;
        Ok(id)
    }

    /// Queues an orientation write without blocking. Writes of one image run in request order
    /// (ADR 0024 §2.5); answered with [`Event::OrientationWritten`].
    pub fn write_orientation(
        &mut self,
        path: PathBuf,
        orientation: u16,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::WriteOrientation {
            id,
            path,
            orientation,
        })?;
        Ok(id)
    }

    /// Queues a read of all metadata of `path` without blocking (T98). It is read with the
    /// current options' `fallback` and answered with [`Event::MetadataRead`].
    pub fn read_metadata(&mut self, path: PathBuf) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::ReadMetadata { id, path })?;
        Ok(id)
    }

    /// Queues a safe write of the editor's `edits` to `path` without blocking (T98), answered
    /// with [`Event::EditsWritten`]. Writes of one image run in request order.
    pub fn write_edits(
        &mut self,
        path: PathBuf,
        edits: Vec<Edit>,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::WriteEdits { id, path, edits })?;
        Ok(id)
    }

    /// Queues a move of `image` and its companion files into `target` without blocking (T170),
    /// answered with [`Event::Moved`]. `image_extensions` are the image formats' extensions (the
    /// image's RAW+JPEG partners and the files whose sessions are released), `suffixes` the
    /// sidecar suffixes besides `xmp`; both without the leading dot, in any case.
    pub fn move_image(
        &mut self,
        image: PathBuf,
        target: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Move {
            id,
            image,
            target,
            image_extensions,
            suffixes,
        })?;
        Ok(id)
    }

    /// Queues moving the files of `moved` back where they came from without blocking (T170),
    /// answered with [`Event::MovedBack`].
    pub fn move_back(&mut self, moved: Vec<Moved>) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::MoveBack { id, moved })?;
        Ok(id)
    }

    /// Queues finding the groups of `images` and their sizes without blocking (T186), for the
    /// confirmation before a delete; answered with [`Event::DeletePrepared`], one entry per image
    /// in the order given. Nothing is released or changed on disk, and no [`WriteSetup`] is
    /// needed. `image_extensions` and `suffixes` are as for [`MetadataWorker::move_image`].
    pub fn prepare_delete(
        &mut self,
        images: Vec<PathBuf>,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::PrepareDelete {
            id,
            images,
            image_extensions,
            suffixes,
        })?;
        Ok(id)
    }

    /// Queues sending `image` and its companion files to the Recycle Bin without blocking (T186),
    /// answered with [`Event::Deleted`]. The sessions of the group's image files are released
    /// first, as for a move; `image_extensions` and `suffixes` are as for
    /// [`MetadataWorker::move_image`]. Without a [`WriteSetup`] it fails with [`NO_JOURNAL`] and
    /// nothing is deleted.
    pub fn delete_image(
        &mut self,
        image: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Delete {
            id,
            image,
            image_extensions,
            suffixes,
        })?;
        Ok(id)
    }

    /// Queues new options without blocking (T98). There is no event; every request queued after
    /// this one uses `options`, a request queued before it the old ones.
    pub fn set_options(&mut self, options: EditOptions) -> std::result::Result<(), SubmitError> {
        self.send(Command::SetOptions(options))
    }

    /// Returns the next completed operation, if one is available.
    pub fn try_recv(&self) -> std::result::Result<Event, TryRecvError> {
        self.events.try_recv()
    }

    /// Drops the worker as `Drop` does but keeps its events (T266, tests only), so the worker
    /// thread can still send them and only the stop flag keeps it from running queued requests.
    #[cfg(test)]
    fn drop_keeping_events(mut self) -> Receiver<Event> {
        let (_unused, empty) = mpsc::sync_channel(0);
        std::mem::replace(&mut self.events, empty)
    }

    fn next_request_id(&mut self) -> RequestId {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    fn submit(&self, request: Request) -> std::result::Result<(), SubmitError> {
        self.send(Command::Run(request))
    }

    fn send(&self, command: Command) -> std::result::Result<(), SubmitError> {
        let Some(requests) = &self.requests else {
            return Err(SubmitError::Stopped);
        };
        match requests.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::Stopped),
        }
    }
}

impl Drop for MetadataWorker {
    fn drop(&mut self) {
        // Requests still in the queue are not run (T266); set before the queue is closed.
        self.stopped.store(true, Ordering::Release);
        self.requests.take();
        // Do not wait for an in-progress ExifTool operation on the UI thread.
        self.thread.take();
    }
}

/// The worker thread's own ExifTool process (T266: kept apart from the pool's).
struct OwnProcess<'a> {
    program: &'a str,
    pre_args: Vec<&'a str>,
    first_version: &'a OnceLock<VersionResult>,
    exiftool: Option<Running>,
}

impl OwnProcess<'_> {
    /// Runs `request` with the worker thread's process, as the worker always has: the process is
    /// started again when needed, so a missing or crashed ExifTool is retried with each request,
    /// and a broken one is dropped after the request.
    fn handle(
        &mut self,
        setup: Option<&WriteSetup>,
        options: &EditOptions,
        request: Request,
    ) -> Event {
        let mut start_error = None;
        if self.exiftool.is_none() {
            match start_tool(self.program, &self.pre_args, Some(self.first_version)) {
                Ok(running) => self.exiftool = Some(running),
                Err(error) => start_error = Some(error),
            }
        }
        let event = handle(self.exiftool.as_mut(), start_error, setup, options, request);
        // A broken process cannot be trusted with another command; dropping it shuts it down.
        if self
            .exiftool
            .as_ref()
            .is_some_and(|running| running.tool.is_broken())
        {
            self.exiftool = None;
        }
        event
    }
}

/// The worker loop: runs until the request sender is dropped or the event receiver is gone.
fn run(
    program: &str,
    pre_args: &[String],
    setup: Option<&WriteSetup>,
    first_version: &OnceLock<VersionResult>,
    requests: &Receiver<Command>,
    events: &SyncSender<Event>,
    stopped: &AtomicBool,
) {
    let pre_args: Vec<&str> = pre_args.iter().map(String::as_str).collect();
    // Start at once, so the version is known before the first request (T94). A failed start is
    // tried again with the next request.
    let exiftool = start_tool(program, &pre_args, Some(first_version)).ok();
    let mut own = OwnProcess {
        program,
        pre_args,
        first_version,
        exiftool,
    };
    // Without a setup there is no pool: writes answer `NoJournal` (T266).
    let processes = setup.map_or(1, |setup| {
        setup.write_processes.clamp(1, MAX_WRITE_PROCESSES)
    });
    match setup {
        Some(setup) if processes > 1 => {
            run_pooled(&mut own, setup, processes, requests, events, stopped);
        }
        _ => run_serial(&mut own, setup, requests, events),
    }
}

/// The worker with one write process: every request in order with the worker's own process.
fn run_serial(
    own: &mut OwnProcess<'_>,
    setup: Option<&WriteSetup>,
    requests: &Receiver<Command>,
    events: &SyncSender<Event>,
) {
    // The current options (T98): the setup's, the default without a setup.
    let mut options = setup.map_or_else(EditOptions::default, |setup| setup.options);
    while let Ok(command) = requests.recv() {
        let request = match command {
            Command::Run(request) => request,
            Command::SetOptions(new) => {
                options = new;
                continue;
            }
        };
        let event = own.handle(setup, &options, request);
        if events.send(event).is_err() {
            break;
        }
    }
}

/// The worker with `processes` (2..=4) write processes (T266): writes go to a [`WritePool`]
/// started at the first write; every other request waits until every earlier write has answered
/// and its event has been sent, then runs with the worker's own process. At most `processes`
/// writes are given at once, so the request queue still fills and gives back-pressure. Once
/// `stopped` is set (the worker was dropped) no request taken from the queue or held back is run,
/// as with one process, where the next event cannot be sent. When the loop ends the pool is
/// dropped here: writes that started finish, the rest are not run.
fn run_pooled(
    own: &mut OwnProcess<'_>,
    setup: &WriteSetup,
    processes: usize,
    requests: &Receiver<Command>,
    events: &SyncSender<Event>,
    stopped: &AtomicBool,
) {
    let mut options = setup.options;
    let mut pool: Option<WritePool<Event>> = None;
    // What each outstanding write's event names, for an answer without one.
    let mut writes: HashMap<Ticket, WriteHeader> = HashMap::new();
    // A request taken from the queue that has to wait: a write while `processes` are given, or
    // anything else while a write is outstanding.
    let mut held: Option<Request> = None;
    loop {
        if let Some(pool) = pool.as_mut().filter(|pool| pool.outstanding() > 0) {
            let mut answer = pool.answer_timeout(ANSWER_POLL);
            while let Some((ticket, job)) = answer {
                let Some(header) = writes.remove(&ticket) else {
                    answer = pool.try_answer();
                    continue;
                };
                if events.send(header.answered(job)).is_err() {
                    return;
                }
                answer = pool.try_answer();
            }
        }
        let outstanding = pool.as_ref().map_or(0, WritePool::outstanding);
        let request = match held.take() {
            Some(request) => request,
            None => {
                let command = if outstanding == 0 {
                    match requests.recv() {
                        Ok(command) => command,
                        Err(_) => return,
                    }
                } else {
                    match requests.try_recv() {
                        Ok(command) => command,
                        Err(TryRecvError::Empty) => continue,
                        Err(TryRecvError::Disconnected) => return,
                    }
                };
                match command {
                    Command::Run(request) => request,
                    Command::SetOptions(new) => {
                        options = new;
                        continue;
                    }
                }
            }
        };
        // The worker was dropped: neither this request nor any later one is run.
        if stopped.load(Ordering::Acquire) {
            return;
        }
        let request = match Write::from_request(request) {
            Ok(write) => {
                if outstanding >= processes {
                    held = Some(write.into_request());
                    continue;
                }
                if pool.is_none() {
                    match WritePool::start_with(own.program, &own.pre_args, processes) {
                        Ok(started) => pool = Some(started),
                        Err(error) => {
                            // Tried again with the next write.
                            let event = write.failed(WriteError::Other(error.to_string()));
                            if events.send(event).is_err() {
                                return;
                            }
                            continue;
                        }
                    }
                }
                let Some(pool) = pool.as_mut() else {
                    continue;
                };
                let header = write.header.clone();
                let key = write_key(header.path());
                let checks = Arc::clone(&setup.checks);
                let journal = setup.journal.clone();
                let write_options = options;
                let ticket = pool.submit(
                    key,
                    Box::new(move |tool: &mut ExifTool| {
                        write.run(tool, &*checks, &journal, &write_options)
                    }),
                );
                writes.insert(ticket, header);
                continue;
            }
            Err(request) => request,
        };
        if outstanding > 0 {
            held = Some(request);
            continue;
        }
        let event = own.handle(Some(setup), &options, request);
        if events.send(event).is_err() {
            return;
        }
    }
}

/// What a write's event names besides its result (T266).
#[derive(Clone, Debug)]
enum WriteHeader {
    Rating {
        id: RequestId,
        path: PathBuf,
        rating: Rating,
    },
    Orientation {
        id: RequestId,
        path: PathBuf,
        orientation: u16,
    },
    Edits {
        id: RequestId,
        path: PathBuf,
    },
}

impl WriteHeader {
    fn path(&self) -> &Path {
        match self {
            WriteHeader::Rating { path, .. }
            | WriteHeader::Orientation { path, .. }
            | WriteHeader::Edits { path, .. } => path,
        }
    }

    /// The write's event with `error`.
    fn failed(self, error: WriteError) -> Event {
        match self {
            WriteHeader::Rating { id, path, rating } => Event::RatingWritten {
                id,
                path,
                rating,
                result: Err(error),
            },
            WriteHeader::Orientation {
                id,
                path,
                orientation,
            } => Event::OrientationWritten {
                id,
                path,
                orientation,
                result: Err(error),
            },
            WriteHeader::Edits { id, path } => Event::EditsWritten {
                id,
                path,
                result: Err(error),
            },
        }
    }

    /// The write's event from the pool's answer (T266): a too old ExifTool is
    /// [`WriteError::ExifToolTooOld`], a failed start and a lost job [`WriteError::Other`] with
    /// their text, as with the worker's own process.
    fn answered(self, answer: JobAnswer<Event>) -> Event {
        match answer {
            JobAnswer::Done(event) => event,
            JobAnswer::TooOld(_) => self.failed(WriteError::ExifToolTooOld),
            JobAnswer::StartFailed(text) | JobAnswer::Lost(text) => {
                self.failed(WriteError::Other(text))
            }
            // The worker never discards; said all the same rather than lost.
            JobAnswer::Discarded => {
                self.failed(WriteError::Other("the write was not started".into()))
            }
        }
    }
}

/// A rating, orientation or edit write taken from a request (T266).
#[derive(Debug)]
struct Write {
    header: WriteHeader,
    /// The edits of an edit write; empty for the others.
    edits: Vec<Edit>,
}

impl Write {
    /// The write in `request`, or the request back when it is not a write.
    fn from_request(request: Request) -> std::result::Result<Write, Request> {
        match request {
            Request::Write { id, path, rating } => Ok(Write {
                header: WriteHeader::Rating { id, path, rating },
                edits: Vec::new(),
            }),
            Request::WriteOrientation {
                id,
                path,
                orientation,
            } => Ok(Write {
                header: WriteHeader::Orientation {
                    id,
                    path,
                    orientation,
                },
                edits: Vec::new(),
            }),
            Request::WriteEdits { id, path, edits } => Ok(Write {
                header: WriteHeader::Edits { id, path },
                edits,
            }),
            other => Err(other),
        }
    }

    /// The request this write came from.
    fn into_request(self) -> Request {
        match self.header {
            WriteHeader::Rating { id, path, rating } => Request::Write { id, path, rating },
            WriteHeader::Orientation {
                id,
                path,
                orientation,
            } => Request::WriteOrientation {
                id,
                path,
                orientation,
            },
            WriteHeader::Edits { id, path } => Request::WriteEdits {
                id,
                path,
                edits: self.edits,
            },
        }
    }

    fn failed(self, error: WriteError) -> Event {
        self.header.failed(error)
    }

    /// Runs the write's safe write with `tool` and gives its event.
    fn run(
        self,
        tool: &mut ExifTool,
        checks: &dyn ImageCheck,
        journal: &Journal,
        options: &EditOptions,
    ) -> Event {
        match self.header {
            WriteHeader::Rating { id, path, rating } => {
                let result = tool
                    .write_rating_safely(&path, &rating, options, checks, journal)
                    .map(|_report| ())
                    .map_err(WriteError::from);
                Event::RatingWritten {
                    id,
                    path,
                    rating,
                    result,
                }
            }
            WriteHeader::Orientation {
                id,
                path,
                orientation,
            } => {
                let result = tool
                    .write_orientation_safely(&path, orientation, checks, journal)
                    .map(|_report| ())
                    .map_err(WriteError::from);
                Event::OrientationWritten {
                    id,
                    path,
                    orientation,
                    result,
                }
            }
            WriteHeader::Edits { id, path } => {
                let result = tool
                    .write_edits_safely(&path, &self.edits, options, checks, journal)
                    .map_err(WriteError::from);
                Event::EditsWritten { id, path, result }
            }
        }
    }
}

fn handle(
    running: Option<&mut Running>,
    start_error: Option<String>,
    setup: Option<&WriteSetup>,
    options: &EditOptions,
    request: Request,
) -> Event {
    let unavailable = || start_error.unwrap_or_else(|| "ExifTool worker is unavailable".into());
    match request {
        Request::Read { id, images } => {
            let read = match running {
                Some(running) => running
                    .tool
                    .read_folder_values(&images)
                    .map_err(|e| e.to_string()),
                None => Err(unavailable()),
            };
            match read {
                Ok(values) => Event::RatingsRead {
                    id,
                    result: Ok(values.ratings),
                    orientations: values.orientations,
                },
                Err(error) => Event::RatingsRead {
                    id,
                    result: Err(error),
                    orientations: HashMap::new(),
                },
            }
        }
        Request::Write { id, path, rating } => handle_write(
            running,
            unavailable,
            setup,
            options,
            Write {
                header: WriteHeader::Rating { id, path, rating },
                edits: Vec::new(),
            },
        ),
        Request::WriteOrientation {
            id,
            path,
            orientation,
        } => handle_write(
            running,
            unavailable,
            setup,
            options,
            Write {
                header: WriteHeader::Orientation {
                    id,
                    path,
                    orientation,
                },
                edits: Vec::new(),
            },
        ),
        Request::ReadMetadata { id, path } => {
            let result = match running {
                Some(running) => running
                    .tool
                    .read_metadata(&path, options.fallback)
                    .map_err(|e| e.to_string()),
                None => Err(unavailable()),
            };
            Event::MetadataRead { id, path, result }
        }
        Request::WriteEdits { id, path, edits } => handle_write(
            running,
            unavailable,
            setup,
            options,
            Write {
                header: WriteHeader::Edits { id, path },
                edits,
            },
        ),
        // A move does not use ExifTool (T170), so `running` and `start_error` do not matter.
        Request::Move {
            id,
            image,
            target,
            image_extensions,
            suffixes,
        } => {
            let result = match setup {
                None => Err(NO_JOURNAL.to_string()),
                Some(setup) => move_image(
                    &*setup.checks,
                    &image,
                    &target,
                    &image_extensions,
                    &suffixes,
                ),
            };
            Event::Moved { id, image, result }
        }
        Request::MoveBack { id, moved } => {
            let result = match setup {
                None => Err(NO_JOURNAL.to_string()),
                Some(setup) => {
                    for step in &moved {
                        setup.checks.release_for_move(&step.to);
                    }
                    relocate::move_back(&moved).map_err(|error| error.to_string())
                }
            };
            Event::MovedBack { id, moved, result }
        }
        // Neither delete request uses ExifTool (T186), as for a move.
        Request::PrepareDelete {
            id,
            images,
            image_extensions,
            suffixes,
        } => {
            let images = images
                .into_iter()
                .map(|image| {
                    let result = prepare_group(&image, &image_extensions, &suffixes);
                    PreparedDelete { image, result }
                })
                .collect();
            Event::DeletePrepared { id, images }
        }
        Request::Delete {
            id,
            image,
            image_extensions,
            suffixes,
        } => {
            let result = match setup {
                None => Err(delete_error(
                    &image,
                    DeleteRefusal::Failed(NO_JOURNAL.to_string()),
                )),
                Some(setup) => delete_image(&*setup.checks, &image, &image_extensions, &suffixes),
            };
            Event::Deleted { id, image, result }
        }
    }
}

/// A write with the worker's own process: without a setup [`WriteError::NoJournal`], with a too
/// old ExifTool [`WriteError::ExifToolTooOld`], without a process [`WriteError::Other`] with
/// `unavailable`'s text, otherwise the safe write.
fn handle_write(
    running: Option<&mut Running>,
    unavailable: impl FnOnce() -> String,
    setup: Option<&WriteSetup>,
    options: &EditOptions,
    write: Write,
) -> Event {
    match (setup, running) {
        (None, _) => write.failed(WriteError::NoJournal),
        (Some(_), Some(running)) if !running.may_write => write.failed(WriteError::ExifToolTooOld),
        (Some(setup), Some(running)) => {
            write.run(&mut running.tool, &*setup.checks, &setup.journal, options)
        }
        (Some(_), None) => write.failed(WriteError::Other(unavailable())),
    }
}

/// Finds `image`'s group, releases the sessions of its image files and moves the group into
/// `target` (T170).
fn move_image(
    checks: &dyn ImageCheck,
    image: &Path,
    target: &Path,
    image_extensions: &[String],
    suffixes: &[String],
) -> std::result::Result<Vec<Moved>, String> {
    let extensions: Vec<&str> = image_extensions.iter().map(String::as_str).collect();
    let suffixes: Vec<&str> = suffixes.iter().map(String::as_str).collect();
    let files = relocate::companions(image, &extensions, &suffixes).map_err(|error| {
        let name = image.file_name().map_or_else(
            || image.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        format!("Could not move {name}: {error}.")
    })?;
    release_image_files(checks, image, &files, &extensions);
    relocate::move_group(&files, target).map_err(|error| error.to_string())
}

/// Releases the sessions of the image files among `files`, `image`'s group, with
/// [`ImageCheck::release_for_move`]: `image` itself, and every file whose extension is in
/// `image_extensions`, without regard to case (T170, T186).
fn release_image_files(
    checks: &dyn ImageCheck,
    image: &Path,
    files: &[PathBuf],
    image_extensions: &[&str],
) {
    let lower: Vec<String> = image_extensions.iter().map(|e| e.to_lowercase()).collect();
    for file in files {
        // The image itself is always released, whether or not its extension is in the list.
        let is_image = file == image
            || file
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .is_some_and(|e| lower.contains(&e));
        if is_image {
            checks.release_for_move(file);
        }
    }
}

/// A [`DeleteError`] for `file` with nothing deleted and nothing left.
fn delete_error(file: &Path, reason: DeleteRefusal) -> DeleteError {
    DeleteError {
        file: file.to_path_buf(),
        reason,
        left: Vec::new(),
        deleted: Vec::new(),
    }
}

/// `image`'s group as [`relocate::companions`] finds it (T186). Its errors become a
/// [`DeleteError`] naming the image made absolute (as given if that fails): `NotFound` is
/// [`DeleteRefusal::Missing`], `InvalidInput` [`DeleteRefusal::NotAFile`], anything else
/// [`DeleteRefusal::Failed`] with the error's text.
fn delete_companions(
    image: &Path,
    image_extensions: &[&str],
    suffixes: &[&str],
) -> std::result::Result<Vec<PathBuf>, DeleteError> {
    relocate::companions(image, image_extensions, suffixes).map_err(|error| {
        let reason = match error.kind() {
            std::io::ErrorKind::NotFound => DeleteRefusal::Missing,
            std::io::ErrorKind::InvalidInput => DeleteRefusal::NotAFile,
            _ => DeleteRefusal::Failed(error.to_string()),
        };
        let file = std::path::absolute(image).unwrap_or_else(|_| image.to_path_buf());
        delete_error(&file, reason)
    })
}

/// Finds `image`'s group and its size for the confirmation (T186); changes nothing.
fn prepare_group(
    image: &Path,
    image_extensions: &[String],
    suffixes: &[String],
) -> std::result::Result<DeleteGroup, DeleteError> {
    let extensions: Vec<&str> = image_extensions.iter().map(String::as_str).collect();
    let suffixes: Vec<&str> = suffixes.iter().map(String::as_str).collect();
    let files = delete_companions(image, &extensions, &suffixes)?;
    let bytes = recycle::inspect_group(&files)?;
    Ok(DeleteGroup { files, bytes })
}

/// Finds `image`'s group, releases the sessions of its image files and sends the group to the
/// Recycle Bin (T186); `Ok` holds the group's files as found.
fn delete_image(
    checks: &dyn ImageCheck,
    image: &Path,
    image_extensions: &[String],
    suffixes: &[String],
) -> std::result::Result<Vec<PathBuf>, DeleteError> {
    let extensions: Vec<&str> = image_extensions.iter().map(String::as_str).collect();
    let suffixes: Vec<&str> = suffixes.iter().map(String::as_str).collect();
    let files = delete_companions(image, &extensions, &suffixes)?;
    release_image_files(checks, image, &files, &extensions);
    recycle::delete_group(&files)?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safe_write::NoImageCheck;
    use crate::test_support::{fake_args, fake_exiftool};
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// A temporary folder for one test, removed at the end.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let path = std::env::temp_dir()
                .join(format!("pw-metadata-worker-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create the temporary folder");
            TempDir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A setup with `checks`, a journal in `folder` and one write process.
    fn setup_with(folder: &Path, checks: Arc<dyn ImageCheck + Send + Sync>) -> WriteSetup {
        WriteSetup {
            journal: Journal::open(folder).expect("open the temporary journal"),
            checks,
            options: EditOptions::default(),
            write_processes: 1,
        }
    }

    /// A setup with `NoImageCheck` and a journal in `folder`.
    fn test_setup(folder: &Path) -> WriteSetup {
        setup_with(folder, Arc::new(NoImageCheck))
    }

    fn next_event(worker: &MetadataWorker) -> Event {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match worker.try_recv() {
                Ok(event) => return event,
                Err(TryRecvError::Disconnected) => panic!("worker disconnected unexpectedly"),
                Err(TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TryRecvError::Empty) => panic!("worker did not answer in time"),
            }
        }
    }

    fn write_result(
        worker: &mut MetadataWorker,
        path: &std::path::Path,
    ) -> std::result::Result<(), WriteError> {
        let rating = Rating {
            stars: Some(3),
            label: None,
        };
        let id = worker
            .write_rating(path.to_path_buf(), rating)
            .expect("request should be accepted");
        match next_event(worker) {
            Event::RatingWritten {
                id: event_id,
                result,
                ..
            } => {
                assert_eq!(event_id, id);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Writes `orientation` to `path` through the worker (T233) and checks that the answer
    /// carries the id, path and orientation as sent.
    fn orientation_result(
        worker: &mut MetadataWorker,
        path: &std::path::Path,
        orientation: u16,
    ) -> std::result::Result<(), WriteError> {
        let id = worker
            .write_orientation(path.to_path_buf(), orientation)
            .expect("request should be accepted");
        match next_event(worker) {
            Event::OrientationWritten {
                id: event_id,
                path: event_path,
                orientation: event_orientation,
                result,
            } => {
                assert_eq!(event_id, id);
                assert_eq!(event_path, path);
                assert_eq!(event_orientation, orientation);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Reads the ratings of `images` through the worker.
    fn read_result(
        worker: &mut MetadataWorker,
        images: &[PathBuf],
    ) -> std::result::Result<HashMap<PathBuf, Rating>, String> {
        let id = worker
            .read_ratings(images.to_vec())
            .expect("request should be accepted");
        match next_event(worker) {
            Event::RatingsRead {
                id: event_id,
                result,
                ..
            } => {
                assert_eq!(event_id, id);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Reads all metadata of `path` through the worker (T98).
    fn metadata_result(
        worker: &mut MetadataWorker,
        path: &Path,
    ) -> std::result::Result<MetadataRead, String> {
        let id = worker
            .read_metadata(path.to_path_buf())
            .expect("request should be accepted");
        match next_event(worker) {
            Event::MetadataRead {
                id: event_id,
                path: event_path,
                result,
            } => {
                assert_eq!(event_id, id);
                assert_eq!(event_path, path);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Writes `edits` to `path` through the worker (T98).
    fn edits_result(
        worker: &mut MetadataWorker,
        path: &Path,
        edits: Vec<Edit>,
    ) -> std::result::Result<EditReport, WriteError> {
        let id = worker
            .write_edits(path.to_path_buf(), edits)
            .expect("request should be accepted");
        match next_event(worker) {
            Event::EditsWritten {
                id: event_id,
                path: event_path,
                result,
            } => {
                assert_eq!(event_id, id);
                assert_eq!(event_path, path);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// An edit that sets the text `value` on `property`.
    fn set_text(property: &str, value: &str) -> Edit {
        Edit::Set {
            property: property.to_string(),
            field: None,
            value: crate::value::Value::Text(value.to_string()),
        }
    }

    /// A fake RAW with a sidecar in `folder`, so that a read of it runs an ExifTool command.
    fn raw_with_sidecar(folder: &Path) -> PathBuf {
        let raw = folder.join("TN0001.ARW");
        std::fs::write(&raw, b"not really a raw file").expect("write the fake RAW");
        std::fs::write(folder.join("TN0001.xmp"), b"<x:xmpmeta/>").expect("write the sidecar");
        raw
    }

    #[test]
    fn reports_exiftool_start_errors_asynchronously() {
        let journal = TempDir::new("start-errors-journal");
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-missing-exiftool-{}",
            std::process::id()
        ));
        let mut worker = match MetadataWorker::start_with(
            &program.to_string_lossy(),
            &[],
            Some(test_setup(journal.path())),
        ) {
            Ok(worker) => worker,
            Err(error) => panic!("worker thread should start: {error}"),
        };
        let id = match worker.read_ratings(Vec::new()) {
            Ok(id) => id,
            Err(error) => panic!("request should be accepted: {error:?}"),
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match worker.try_recv() {
                Ok(Event::RatingsRead {
                    id: event_id,
                    result: Err(error),
                    ..
                }) => {
                    assert_eq!(event_id, id);
                    assert!(error.contains("could not start"));
                    break;
                }
                Ok(Event::RatingsRead { result: Ok(_), .. }) => {
                    panic!("missing ExifTool unexpectedly succeeded")
                }
                Ok(Event::RatingWritten { .. }) => panic!("unexpected write result"),
                Ok(
                    other @ (Event::MetadataRead { .. }
                    | Event::OrientationWritten { .. }
                    | Event::EditsWritten { .. }
                    | Event::Moved { .. }
                    | Event::MovedBack { .. }
                    | Event::DeletePrepared { .. }
                    | Event::Deleted { .. }),
                ) => {
                    panic!("unexpected event {other:?}")
                }
                Err(TryRecvError::Disconnected) => panic!("worker disconnected unexpectedly"),
                Err(TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TryRecvError::Empty) => panic!("worker did not report startup failure"),
            }
        }
    }

    /// T230: a failed read gives an `Err` result and no orientations (ADR 0024 §2.4).
    #[test]
    fn a_failed_read_gives_no_orientations() {
        let mut worker = start_without_exiftool("read-orientations", None);
        let id = worker
            .read_ratings(Vec::new())
            .expect("request should be accepted");
        match next_event(&worker) {
            Event::RatingsRead {
                id: event_id,
                result,
                orientations,
            } => {
                assert_eq!(event_id, id);
                assert!(result.is_err(), "a read without ExifTool should fail");
                assert!(orientations.is_empty());
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    // The fake ExifTool answers every command with "1 image files updated", which a safe write
    // cannot use (it reads the file as JSON first), so the two tests below drive the worker with
    // reads of a RAW that has a sidecar (owner's decision in T91).
    #[test]
    fn restarts_exiftool_after_it_exits() {
        let folder = TempDir::new("restart");
        let journal = TempDir::new("restart-journal");
        let raw = raw_with_sidecar(folder.path());
        // T94 (owner's decision): the worker asks `-ver` when it starts a process, so this fake
        // answers `-ver` and exits at the second other command.
        let (program, args) = fake_exiftool("exit-after-first-request");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker =
            MetadataWorker::start_with(&program, &args, Some(test_setup(journal.path())))
                .expect("worker should start");
        let images = [raw];
        assert_eq!(read_result(&mut worker, &images), Ok(HashMap::new()));
        let failed = read_result(&mut worker, &images);
        assert!(failed.is_err(), "the crashed process must report an error");
        assert_eq!(
            read_result(&mut worker, &images),
            Ok(HashMap::new()),
            "a new process should serve the next request"
        );
    }

    #[test]
    fn retries_a_failed_start_with_the_next_request() {
        let folder = TempDir::new("retry");
        let journal = TempDir::new("retry-journal");
        let raw = raw_with_sidecar(folder.path());
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-late-exiftool-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        let _ = std::fs::remove_file(&program);
        let args = fake_args("normal");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker = MetadataWorker::start_with(
            &program.to_string_lossy(),
            &args,
            Some(test_setup(journal.path())),
        )
        .expect("worker should start");
        let images = [raw];
        let first = read_result(&mut worker, &images);
        assert!(
            first.as_ref().is_err_and(|e| e.contains("could not start")),
            "{first:?}"
        );
        // "Install" ExifTool: the fake is a copy of this test binary.
        let exe = std::env::current_exe().expect("test binary path");
        std::fs::copy(&exe, &program).expect("copy fake ExifTool");
        assert_eq!(read_result(&mut worker, &images), Ok(HashMap::new()));
        drop(worker);
        // Best effort: on Windows the copy may still be running for a moment.
        let _ = std::fs::remove_file(&program);
    }

    #[test]
    fn without_a_setup_a_rating_write_fails_and_writes_nothing() {
        let folder = TempDir::new("no-journal");
        let raw = folder.path().join("TN0001.ARW");
        std::fs::write(&raw, b"not really a raw file").expect("write the fake RAW");
        let (program, args) = fake_exiftool("normal");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker =
            MetadataWorker::start_with(&program, &args, None).expect("worker should start");

        assert_eq!(write_result(&mut worker, &raw), Err(WriteError::NoJournal));
        assert!(!folder.path().join("TN0001.xmp").exists());
    }

    #[test]
    fn without_a_setup_an_orientation_write_fails_and_writes_nothing() {
        let folder = TempDir::new("no-journal-orientation");
        let raw = folder.path().join("TN0001.ARW");
        std::fs::write(&raw, b"not really a raw file").expect("write the fake RAW");
        let (program, args) = fake_exiftool("normal");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker =
            MetadataWorker::start_with(&program, &args, None).expect("worker should start");

        assert_eq!(
            orientation_result(&mut worker, &raw, 6),
            Err(WriteError::NoJournal)
        );
        assert!(!folder.path().join("TN0001.xmp").exists());
    }

    #[test]
    fn without_a_setup_edits_fail_and_the_file_is_unchanged() {
        let folder = TempDir::new("no-journal-edits");
        let image = folder.path().join("bild.jpg");
        std::fs::write(&image, b"not really a jpeg").expect("write the fake JPEG");
        let (program, args) = fake_exiftool("normal");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker =
            MetadataWorker::start_with(&program, &args, None).expect("worker should start");

        assert_eq!(
            edits_result(
                &mut worker,
                &image,
                vec![set_text("photoshop:Headline", "Rubrik")]
            ),
            Err(WriteError::NoJournal)
        );
        assert_eq!(
            std::fs::read(&image).expect("read the file"),
            b"not really a jpeg"
        );
        assert_eq!(
            std::fs::read_dir(folder.path())
                .expect("read the folder")
                .count(),
            1
        );
    }

    /// Waits up to `limit` for the first start's result of `worker`.
    fn wait_for_version(
        worker: &MetadataWorker,
        limit: Duration,
    ) -> Option<std::result::Result<ToolVersion, String>> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(version) = worker.exiftool_version() {
                return Some(version);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A worker with a fake ExifTool that answers `-ver` with `answer`, and a journal in `journal`.
    fn start_with_version(answer: &str, journal: &Path) -> MetadataWorker {
        let (program, args) = fake_exiftool(&format!("version={answer}"));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        MetadataWorker::start_with(&program, &args, Some(test_setup(journal)))
            .expect("worker should start")
    }

    #[test]
    fn the_version_is_known_without_any_request() {
        let journal = TempDir::new("version-new-journal");
        let worker = start_with_version("13.59", journal.path());
        assert_eq!(
            wait_for_version(&worker, Duration::from_secs(10)),
            Some(Ok(ToolVersion {
                major: 13,
                minor: 59
            }))
        );
    }

    #[test]
    fn an_old_exiftool_refuses_rating_writes_but_reads() {
        let folder = TempDir::new("version-old");
        let journal = TempDir::new("version-old-journal");
        let raw = raw_with_sidecar(folder.path());
        let sidecar = folder.path().join("TN0001.xmp");
        let mut worker = start_with_version("12.40", journal.path());
        assert_eq!(
            wait_for_version(&worker, Duration::from_secs(10)),
            Some(Ok(ToolVersion {
                major: 12,
                minor: 40
            }))
        );

        assert_eq!(
            write_result(&mut worker, &raw),
            Err(WriteError::ExifToolTooOld)
        );
        assert_eq!(
            std::fs::read(&sidecar).expect("read the sidecar"),
            b"<x:xmpmeta/>"
        );
        assert_eq!(
            read_result(&mut worker, std::slice::from_ref(&raw)),
            Ok(HashMap::new())
        );
    }

    #[test]
    fn an_old_exiftool_refuses_orientation_writes() {
        let folder = TempDir::new("version-old-orientation");
        let journal = TempDir::new("version-old-orientation-journal");
        let raw = raw_with_sidecar(folder.path());
        let sidecar = folder.path().join("TN0001.xmp");
        let mut worker = start_with_version("12.40", journal.path());
        assert!(matches!(
            wait_for_version(&worker, Duration::from_secs(10)),
            Some(Ok(_))
        ));

        assert_eq!(
            orientation_result(&mut worker, &raw, 6),
            Err(WriteError::ExifToolTooOld)
        );
        assert_eq!(
            std::fs::read(&sidecar).expect("read the sidecar"),
            b"<x:xmpmeta/>"
        );
    }

    #[test]
    fn an_old_exiftool_refuses_edits_with_the_typed_kind() {
        let folder = TempDir::new("version-old-edits");
        let journal = TempDir::new("version-old-edits-journal");
        let image = folder.path().join("bild.jpg");
        std::fs::write(&image, b"not really a jpeg").expect("write the fake JPEG");
        let mut worker = start_with_version("12.40", journal.path());
        assert!(matches!(
            wait_for_version(&worker, Duration::from_secs(10)),
            Some(Ok(_))
        ));

        let result = edits_result(
            &mut worker,
            &image,
            vec![set_text("photoshop:Headline", "Headline")],
        );
        assert_eq!(result, Err(WriteError::ExifToolTooOld));
        assert_eq!(
            result.err().map(|error| error.to_string()).as_deref(),
            Some(EXIFTOOL_TOO_OLD)
        );
        assert_eq!(
            std::fs::read(&image).expect("read the file"),
            b"not really a jpeg"
        );
    }

    #[test]
    fn an_unreadable_version_counts_as_too_old() {
        let folder = TempDir::new("version-abc");
        let journal = TempDir::new("version-abc-journal");
        let raw = raw_with_sidecar(folder.path());
        let mut worker = start_with_version("abc", journal.path());
        match wait_for_version(&worker, Duration::from_secs(10)) {
            Some(Err(error)) => assert_eq!(error, "ExifTool: unreadable ExifTool version: abc"),
            other => panic!("expected an unreadable version, got {other:?}"),
        }

        assert_eq!(
            write_result(&mut worker, &raw),
            Err(WriteError::ExifToolTooOld)
        );
        assert_eq!(
            read_result(&mut worker, std::slice::from_ref(&raw)),
            Ok(HashMap::new())
        );
    }

    #[test]
    fn a_missing_program_gives_the_start_error_as_the_version() {
        let journal = TempDir::new("version-missing-journal");
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-no-exiftool-version-{}",
            std::process::id()
        ));
        let worker = MetadataWorker::start_with(
            &program.to_string_lossy(),
            &[],
            Some(test_setup(journal.path())),
        )
        .expect("worker thread should start");
        match wait_for_version(&worker, Duration::from_secs(10)) {
            Some(Err(error)) => assert!(error.contains("could not start"), "{error}"),
            other => panic!("expected the start error, got {other:?}"),
        }
    }

    /// An image check that records the paths given to `release` (T170) and, in a second list,
    /// those given to `release_for_move` (T177).
    #[derive(Clone, Default)]
    struct RecordingCheck(
        Arc<std::sync::Mutex<Vec<PathBuf>>>,
        Arc<std::sync::Mutex<Vec<PathBuf>>>,
    );

    impl RecordingCheck {
        fn released(&self) -> Vec<PathBuf> {
            self.0.lock().expect("the recorded paths").clone()
        }

        fn released_for_move(&self) -> Vec<PathBuf> {
            self.1.lock().expect("the recorded paths").clone()
        }
    }

    impl ImageCheck for RecordingCheck {
        fn release(&self, image: &Path) {
            self.0
                .lock()
                .expect("the recorded paths")
                .push(image.to_path_buf());
        }

        fn release_for_move(&self, image: &Path) {
            self.1
                .lock()
                .expect("the recorded paths")
                .push(image.to_path_buf());
        }

        fn check_copy(
            &self,
            _original: &Path,
            _copy: &Path,
            _kind: crate::reconcile::FileKind,
        ) -> std::result::Result<(), String> {
            Ok(())
        }
    }

    /// A worker whose ExifTool program is missing (T170: a move does not need ExifTool).
    fn start_without_exiftool(name: &str, setup: Option<WriteSetup>) -> MetadataWorker {
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-no-exiftool-{name}-{}",
            std::process::id()
        ));
        MetadataWorker::start_with(&program.to_string_lossy(), &[], setup)
            .expect("worker thread should start")
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// Moves `image` into `target` through the worker, with ARW and JPG as image extensions and
    /// no suffixes.
    fn move_result(
        worker: &mut MetadataWorker,
        image: &Path,
        target: &Path,
    ) -> std::result::Result<Vec<Moved>, String> {
        let id = worker
            .move_image(
                image.to_path_buf(),
                target.to_path_buf(),
                strings(&["ARW", "jpg"]),
                Vec::new(),
            )
            .expect("request should be accepted");
        match next_event(worker) {
            Event::Moved {
                id: event_id,
                image: event_image,
                result,
            } => {
                assert_eq!(event_id, id);
                assert_eq!(event_image, image);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Moves the files of `moved` back through the worker.
    fn move_back_result(
        worker: &mut MetadataWorker,
        moved: &[Moved],
    ) -> std::result::Result<(), String> {
        let id = worker
            .move_back(moved.to_vec())
            .expect("request should be accepted");
        match next_event(worker) {
            Event::MovedBack {
                id: event_id,
                moved: event_moved,
                result,
            } => {
                assert_eq!(event_id, id);
                assert_eq!(event_moved, moved);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// The names of the entries in `folder`, sorted.
    fn names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .expect("read the folder")
            .map(|entry| {
                entry
                    .expect("folder entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn move_image_moves_the_image_and_its_sidecar_and_releases_the_image() {
        let folder = TempDir::new("move");
        let journal = TempDir::new("move-journal");
        let raw = raw_with_sidecar(folder.path());
        let target = folder.path().join("_rejected");
        let check = RecordingCheck::default();
        let mut worker = start_without_exiftool(
            "move",
            Some(setup_with(journal.path(), Arc::new(check.clone()))),
        );

        let moved = move_result(&mut worker, &raw, &target).expect("the move should succeed");

        assert_eq!(
            moved,
            vec![
                Moved {
                    from: raw.clone(),
                    to: target.join("TN0001.ARW"),
                },
                Moved {
                    from: folder.path().join("TN0001.xmp"),
                    to: target.join("TN0001.xmp"),
                },
            ]
        );
        assert_eq!(names(&target), ["TN0001.ARW", "TN0001.xmp"]);
        assert_eq!(names(folder.path()), ["_rejected"]);
        assert_eq!(check.released_for_move(), vec![raw]);
        assert!(check.released().is_empty());
    }

    #[test]
    fn a_move_sent_after_a_rating_write_is_answered_after_it() {
        let folder = TempDir::new("move-order");
        let journal = TempDir::new("move-order-journal");
        let raw = raw_with_sidecar(folder.path());
        let target = folder.path().join("_rejected");
        let mut worker = start_without_exiftool("move-order", Some(test_setup(journal.path())));

        let write = worker
            .write_rating(
                raw.clone(),
                Rating {
                    stars: Some(1),
                    label: None,
                },
            )
            .expect("request should be accepted");
        let moving = worker
            .move_image(raw, target, strings(&["arw"]), Vec::new())
            .expect("request should be accepted");

        match next_event(&worker) {
            Event::RatingWritten { id, .. } => assert_eq!(id, write),
            other => panic!("expected the rating write first, got {other:?}"),
        }
        match next_event(&worker) {
            Event::Moved { id, result, .. } => {
                assert_eq!(id, moving);
                assert!(result.is_ok(), "{result:?}");
            }
            other => panic!("expected the move second, got {other:?}"),
        }
    }

    #[test]
    fn a_failing_move_names_the_file_and_moves_nothing() {
        let folder = TempDir::new("move-fails");
        let journal = TempDir::new("move-fails-journal");
        let raw = raw_with_sidecar(folder.path());
        let target = folder.path().join("_rejected");
        std::fs::create_dir(&target).expect("create the target");
        std::fs::write(target.join("TN0001.xmp"), b"already here").expect("write the taken name");
        let mut worker = start_without_exiftool("move-fails", Some(test_setup(journal.path())));

        let result = move_result(&mut worker, &raw, &target);

        let error = result.expect_err("the move should fail");
        assert!(error.contains("TN0001.xmp"), "{error}");
        assert_eq!(names(&target), ["TN0001.xmp"]);
        assert_eq!(
            std::fs::read(target.join("TN0001.xmp")).expect("read the taken name"),
            b"already here"
        );
        assert_eq!(
            names(folder.path()),
            ["TN0001.ARW", "TN0001.xmp", "_rejected"]
        );
    }

    #[test]
    fn move_back_returns_the_files() {
        let folder = TempDir::new("move-back");
        let journal = TempDir::new("move-back-journal");
        let raw = raw_with_sidecar(folder.path());
        let target = folder.path().join("_rejected");
        let check = RecordingCheck::default();
        let mut worker = start_without_exiftool(
            "move-back",
            Some(setup_with(journal.path(), Arc::new(check.clone()))),
        );
        let moved = move_result(&mut worker, &raw, &target).expect("the move should succeed");

        assert_eq!(move_back_result(&mut worker, &moved), Ok(()));

        assert_eq!(
            names(folder.path()),
            ["TN0001.ARW", "TN0001.xmp", "_rejected"]
        );
        assert_eq!(names(&target), Vec::<String>::new());
        let mut released = vec![raw];
        released.extend(moved.iter().map(|m| m.to.clone()));
        assert_eq!(check.released_for_move(), released);
        assert!(check.released().is_empty());
    }

    #[test]
    fn the_image_is_released_even_when_its_extension_is_not_listed() {
        let folder = TempDir::new("move-unlisted");
        let journal = TempDir::new("move-unlisted-journal");
        let image = folder.path().join("TN0001.tiff");
        std::fs::write(&image, b"image").expect("write the image");
        let target = folder.path().join("_rejected");
        let check = RecordingCheck::default();
        let mut worker = start_without_exiftool(
            "move-unlisted",
            Some(setup_with(journal.path(), Arc::new(check.clone()))),
        );

        move_result(&mut worker, &image, &target).expect("the move should succeed");

        assert_eq!(check.released_for_move(), vec![image]);
        assert!(check.released().is_empty());
        assert_eq!(names(&target), ["TN0001.tiff"]);
    }

    #[test]
    fn a_missing_image_fails_with_its_name() {
        let folder = TempDir::new("move-missing");
        let journal = TempDir::new("move-missing-journal");
        let image = folder.path().join("TN0009.ARW");
        let target = folder.path().join("_rejected");
        let mut worker = start_without_exiftool("move-missing", Some(test_setup(journal.path())));

        let error = move_result(&mut worker, &image, &target).expect_err("the move should fail");

        assert!(error.starts_with("Could not move TN0009.ARW: "), "{error}");
        assert!(!target.exists());
    }

    #[test]
    fn without_a_setup_moves_fail_and_nothing_moves() {
        let folder = TempDir::new("move-no-journal");
        let raw = raw_with_sidecar(folder.path());
        let target = folder.path().join("_rejected");
        std::fs::create_dir(&target).expect("create the target");
        let moved_file = target.join("TN0002.ARW");
        std::fs::write(&moved_file, b"moved earlier").expect("write the moved file");
        let mut worker = start_without_exiftool("move-no-journal", None);

        assert_eq!(
            move_result(&mut worker, &raw, &target),
            Err(NO_JOURNAL.to_string())
        );
        let back = [Moved {
            from: folder.path().join("TN0002.ARW"),
            to: moved_file,
        }];
        assert_eq!(
            move_back_result(&mut worker, &back),
            Err(NO_JOURNAL.to_string())
        );

        assert_eq!(
            names(folder.path()),
            ["TN0001.ARW", "TN0001.xmp", "_rejected"]
        );
        assert_eq!(names(&target), ["TN0002.ARW"]);
    }

    /// Finds the groups of `images` through the worker (T186), with ARW and JPG as image
    /// extensions and `dop` as the suffix.
    fn prepare_result(worker: &mut MetadataWorker, images: &[PathBuf]) -> Vec<PreparedDelete> {
        let id = worker
            .prepare_delete(images.to_vec(), strings(&["arw", "jpg"]), strings(&["dop"]))
            .expect("request should be accepted");
        match next_event(worker) {
            Event::DeletePrepared {
                id: event_id,
                images,
            } => {
                assert_eq!(event_id, id);
                images
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Deletes `image` through the worker (T186), with ARW and JPG as image extensions and `dop`
    /// as the suffix.
    fn delete_result(
        worker: &mut MetadataWorker,
        image: &Path,
    ) -> std::result::Result<Vec<PathBuf>, DeleteError> {
        let id = worker
            .delete_image(
                image.to_path_buf(),
                strings(&["arw", "jpg"]),
                strings(&["dop"]),
            )
            .expect("request should be accepted");
        match next_event(worker) {
            Event::Deleted {
                id: event_id,
                image: event_image,
                result,
            } => {
                assert_eq!(event_id, id);
                assert_eq!(event_image, image);
                result
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// Writes `name` in `folder` with its own name as its content and returns its path.
    fn file_in(folder: &Path, name: &str) -> PathBuf {
        let path = folder.join(name);
        std::fs::write(&path, name.as_bytes()).expect("write the file");
        path
    }

    /// The total size of `files`.
    fn size_of(files: &[PathBuf]) -> u64 {
        files
            .iter()
            .map(|file| std::fs::metadata(file).expect("the file's metadata").len())
            .sum()
    }

    #[test]
    fn without_a_setup_delete_image_fails_and_nothing_is_deleted() {
        let folder = TempDir::new("delete-no-journal");
        let raw = file_in(folder.path(), "TN0001.ARW");
        let dop = file_in(folder.path(), "TN0001.ARW.dop");
        let mut worker = start_without_exiftool("delete-no-journal", None);

        assert_eq!(
            delete_result(&mut worker, &raw),
            Err(DeleteError {
                file: raw.clone(),
                reason: DeleteRefusal::Failed(NO_JOURNAL.to_string()),
                left: Vec::new(),
                deleted: Vec::new(),
            })
        );
        assert!(raw.is_file() && dop.is_file());
    }

    /// The tests below expect a group to pass T185's drive check, so the system's temporary
    /// folder must be on a fixed drive with the Recycle Bin on; if it is not, they fail with the
    /// refusal. Nothing goes to the Recycle Bin except in the ignored test.
    #[cfg(windows)]
    mod deletes {
        use super::*;
        use std::os::windows::fs::OpenOptionsExt;

        // FILE_SHARE_READ | FILE_SHARE_WRITE, without FILE_SHARE_DELETE.
        const SHARE_READ_WRITE: u32 = 0x1 | 0x2;

        /// Opens `file` as another program would: reading and writing shared, deleting not.
        fn hold_open(file: &Path) -> std::fs::File {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(SHARE_READ_WRITE)
                .open(file)
                .expect("open the file")
        }

        #[test]
        fn prepare_delete_gives_each_group_and_its_size_and_changes_nothing() {
            let folder = TempDir::new("prepare");
            let journal = TempDir::new("prepare-journal");
            let raw = file_in(folder.path(), "TN0001.ARW");
            let dop = file_in(folder.path(), "TN0001.ARW.dop");
            let xmp = file_in(folder.path(), "TN0001.XMP");
            let jpg = file_in(folder.path(), "TN0002.jpg");
            let check = RecordingCheck::default();
            let mut worker = start_without_exiftool(
                "prepare",
                Some(setup_with(journal.path(), Arc::new(check.clone()))),
            );

            let prepared = prepare_result(&mut worker, &[jpg.clone(), raw.clone()]);

            let raw_group = vec![raw.clone(), dop.clone(), xmp.clone()];
            assert_eq!(
                prepared,
                vec![
                    PreparedDelete {
                        image: jpg.clone(),
                        result: Ok(DeleteGroup {
                            files: vec![jpg.clone()],
                            bytes: size_of(std::slice::from_ref(&jpg)),
                        }),
                    },
                    PreparedDelete {
                        image: raw.clone(),
                        result: Ok(DeleteGroup {
                            bytes: size_of(&raw_group),
                            files: raw_group,
                        }),
                    },
                ]
            );
            assert!([raw, dop, xmp, jpg].iter().all(|file| file.is_file()));
            assert!(check.released().is_empty());
            assert!(check.released_for_move().is_empty());
        }

        #[test]
        fn prepare_delete_of_a_missing_image_is_missing_and_the_other_is_answered() {
            let folder = TempDir::new("prepare-missing");
            let journal = TempDir::new("prepare-missing-journal");
            let missing = folder.path().join("TN0009.ARW");
            let jpg = file_in(folder.path(), "TN0002.jpg");
            let mut worker =
                start_without_exiftool("prepare-missing", Some(test_setup(journal.path())));

            let prepared = prepare_result(&mut worker, &[missing.clone(), jpg.clone()]);

            assert_eq!(
                prepared,
                vec![
                    PreparedDelete {
                        image: missing.clone(),
                        result: Err(DeleteError {
                            file: missing,
                            reason: DeleteRefusal::Missing,
                            left: Vec::new(),
                            deleted: Vec::new(),
                        }),
                    },
                    PreparedDelete {
                        image: jpg.clone(),
                        result: Ok(DeleteGroup {
                            files: vec![jpg.clone()],
                            bytes: size_of(std::slice::from_ref(&jpg)),
                        }),
                    },
                ]
            );
        }

        #[test]
        fn prepare_delete_works_without_a_setup() {
            let folder = TempDir::new("prepare-no-journal");
            let raw = file_in(folder.path(), "TN0001.ARW");
            let dop = file_in(folder.path(), "TN0001.ARW.dop");
            let mut worker = start_without_exiftool("prepare-no-journal", None);

            let prepared = prepare_result(&mut worker, std::slice::from_ref(&raw));

            let group = vec![raw.clone(), dop.clone()];
            assert_eq!(
                prepared,
                vec![PreparedDelete {
                    image: raw.clone(),
                    result: Ok(DeleteGroup {
                        bytes: size_of(&group),
                        files: group,
                    }),
                }]
            );
            assert!(raw.is_file() && dop.is_file());
        }

        #[test]
        fn delete_image_with_a_sidecar_held_open_is_refused_and_nothing_is_deleted() {
            let folder = TempDir::new("delete-held");
            let journal = TempDir::new("delete-held-journal");
            let raw = file_in(folder.path(), "TN0001.ARW");
            let dop = file_in(folder.path(), "TN0001.ARW.dop");
            let xmp = file_in(folder.path(), "TN0001.XMP");
            let check = RecordingCheck::default();
            let mut worker = start_without_exiftool(
                "delete-held",
                Some(setup_with(journal.path(), Arc::new(check.clone()))),
            );

            let held = hold_open(&xmp);
            let result = delete_result(&mut worker, &raw);
            drop(held);

            assert_eq!(
                result,
                Err(DeleteError {
                    file: xmp.clone(),
                    reason: DeleteRefusal::OpenElsewhere,
                    left: Vec::new(),
                    deleted: Vec::new(),
                })
            );
            assert_eq!(check.released_for_move(), vec![raw.clone()]);
            assert!(check.released().is_empty());
            assert!([raw, dop, xmp].iter().all(|file| file.is_file()));
        }

        #[test]
        fn a_delete_sent_after_a_rating_write_is_answered_after_it() {
            let folder = TempDir::new("delete-order");
            let journal = TempDir::new("delete-order-journal");
            let raw = file_in(folder.path(), "TN0001.ARW");
            let xmp = file_in(folder.path(), "TN0001.XMP");
            let mut worker =
                start_without_exiftool("delete-order", Some(test_setup(journal.path())));

            let held = hold_open(&xmp);
            let write = worker
                .write_rating(
                    raw.clone(),
                    Rating {
                        stars: Some(1),
                        label: None,
                    },
                )
                .expect("request should be accepted");
            let deleting = worker
                .delete_image(raw.clone(), strings(&["arw", "jpg"]), strings(&["dop"]))
                .expect("request should be accepted");
            let first = next_event(&worker);
            let second = next_event(&worker);
            drop(held);

            match first {
                Event::RatingWritten { id, .. } => assert_eq!(id, write),
                other => panic!("expected the rating write first, got {other:?}"),
            }
            match second {
                Event::Deleted { id, result, .. } => {
                    assert_eq!(id, deleting);
                    let reason = result.map_err(|error| error.reason);
                    assert_eq!(reason, Err(DeleteRefusal::OpenElsewhere));
                }
                other => panic!("expected the delete second, got {other:?}"),
            }
            assert!(raw.is_file() && xmp.is_file());
        }

        #[test]
        #[ignore = "puts two small files in the real Recycle Bin (B-Q15); run by hand"]
        fn delete_to_recycle_bin_sends_the_image_and_its_dop_and_releases_the_image() {
            let folder = TempDir::new("delete-real");
            let journal = TempDir::new("delete-real-journal");
            let raw = file_in(folder.path(), "pw-metadata-recycle-test-TN0040.ARW");
            let dop = file_in(folder.path(), "pw-metadata-recycle-test-TN0040.ARW.dop");
            let check = RecordingCheck::default();
            let mut worker = start_without_exiftool(
                "delete-real",
                Some(setup_with(journal.path(), Arc::new(check.clone()))),
            );

            let id = worker
                .delete_image(raw.clone(), strings(&["arw", "jpg"]), strings(&["dop"]))
                .expect("request should be accepted");
            let deadline = Instant::now() + Duration::from_secs(30);
            let event = loop {
                match worker.try_recv() {
                    Ok(event) => break event,
                    Err(TryRecvError::Disconnected) => panic!("worker disconnected unexpectedly"),
                    Err(TryRecvError::Empty) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(TryRecvError::Empty) => {
                        panic!("no answer within 30 seconds: a Windows dialog may be open")
                    }
                }
            };

            match event {
                Event::Deleted {
                    id: event_id,
                    image,
                    result,
                } => {
                    assert_eq!(event_id, id);
                    assert_eq!(image, raw);
                    assert_eq!(result, Ok(vec![raw.clone(), dop.clone()]));
                }
                other => panic!("unexpected event {other:?}"),
            }
            assert!(!raw.exists() && !dop.exists());
            assert_eq!(check.released_for_move(), vec![raw]);
            assert!(check.released().is_empty());
        }
    }

    /// T266: the worker with several write processes, over the fake ExifTool's `slow` mode (a
    /// command naming a file with [`SLOW_MARKER`] is answered after [`SLOW_WAIT`]). The fake
    /// cannot be written through, so every write answers an error; what is checked is when and
    /// in which order the events come.
    mod pooled {
        use super::*;
        use crate::test_support::{LOG_PREFIX, SLOW_MARKER, SLOW_WAIT};
        use crate::write_pool::tests::FAIL_START_WHILE;

        /// A setup with `NoImageCheck`, a journal in `folder` and `n` write processes.
        fn pooled_setup(folder: &Path, n: usize) -> WriteSetup {
            WriteSetup {
                write_processes: n,
                ..test_setup(folder)
            }
        }

        /// A worker over the fake's `slow` mode with `n` write processes and `extra` leading
        /// arguments after the fake's own.
        fn start_slow(journal: &Path, n: usize, extra: &[String]) -> MetadataWorker {
            let (program, mut args) = fake_exiftool("slow");
            args.extend(extra.iter().cloned());
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            MetadataWorker::start_with(&program, &args, Some(pooled_setup(journal, n)))
                .expect("worker should start")
        }

        /// A file `name` in `folder` (`slow` puts the marker in its name).
        fn image(folder: &Path, name: &str, slow: bool) -> PathBuf {
            let name = if slow {
                let (stem, extension) = name.rsplit_once('.').expect("a name with an extension");
                format!("{stem}-{SLOW_MARKER}.{extension}")
            } else {
                name.to_string()
            };
            let path = folder.join(name);
            std::fs::write(&path, b"not really an image").expect("write the file");
            path
        }

        fn rate(worker: &mut MetadataWorker, path: &Path) -> RequestId {
            worker
                .write_rating(
                    path.to_path_buf(),
                    Rating {
                        stars: Some(2),
                        label: None,
                    },
                )
                .expect("request should be accepted")
        }

        fn id_of(event: &Event) -> RequestId {
            match event {
                Event::RatingsRead { id, .. }
                | Event::RatingWritten { id, .. }
                | Event::OrientationWritten { id, .. }
                | Event::MetadataRead { id, .. }
                | Event::EditsWritten { id, .. }
                | Event::Moved { id, .. }
                | Event::MovedBack { id, .. }
                | Event::DeletePrepared { id, .. }
                | Event::Deleted { id, .. } => *id,
            }
        }

        /// The ids of the next `count` events, in the order they came.
        fn next_ids(worker: &MetadataWorker, count: usize) -> Vec<RequestId> {
            (0..count).map(|_| id_of(&next_event(worker))).collect()
        }

        #[test]
        fn writes_of_different_images_overlap_and_may_answer_out_of_order() {
            for n in [2, 4] {
                let folder = TempDir::new(&format!("pooled-overlap-{n}"));
                let journal = TempDir::new(&format!("pooled-overlap-{n}-journal"));
                let first = image(folder.path(), "a.jpg", true);
                let second = image(folder.path(), "b.jpg", false);
                let mut worker = start_slow(journal.path(), n, &[]);

                let slow_id = rate(&mut worker, &first);
                let quick_id = rate(&mut worker, &second);

                // The quick write, asked second, answered while the slow one still ran.
                assert_eq!(
                    next_ids(&worker, 2),
                    vec![quick_id, slow_id],
                    "{n} processes"
                );
            }
        }

        #[test]
        fn two_writes_of_one_image_answer_in_order() {
            for n in [2, 4] {
                let folder = TempDir::new(&format!("pooled-one-image-{n}"));
                let journal = TempDir::new(&format!("pooled-one-image-{n}-journal"));
                let file = image(folder.path(), "a.jpg", true);
                let mut worker = start_slow(journal.path(), n, &[]);

                let started = Instant::now();
                let first = rate(&mut worker, &file);
                let second = worker
                    .write_orientation(file.clone(), 6)
                    .expect("request should be accepted");

                assert_eq!(next_ids(&worker, 2), vec![first, second], "{n} processes");
                // One after the other: two slow commands.
                assert!(started.elapsed() >= SLOW_WAIT * 2, "{n} processes");
            }
        }

        #[test]
        fn writes_queued_when_the_worker_is_dropped_are_never_given_to_the_pool() {
            let folder = TempDir::new("pooled-drop-queued");
            let journal = TempDir::new("pooled-drop-queued-journal");
            let files: Vec<PathBuf> = ["q0.jpg", "q1.jpg", "q2.jpg", "q3.jpg"]
                .iter()
                .map(|name| image(folder.path(), name, true))
                .collect();
            // The fake logs every file a write's command names: which writes reached a process.
            let log = folder.path().join("commands.log");
            let mut worker = start_slow(
                journal.path(),
                2,
                &[format!("{LOG_PREFIX}{}", log.display())],
            );
            let ids: Vec<RequestId> = files.iter().map(|file| rate(&mut worker, file)).collect();
            // The first two are given to the pool and run; the third is held back, the fourth
            // waits in the queue.
            std::thread::sleep(SLOW_WAIT / 2);

            // The events are kept, so the worker thread can still send the first two writes'
            // events and go on: only the drop keeps it from giving the other two to the pool.
            let events = worker.drop_keeping_events();

            let mut answered = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                match events.try_recv() {
                    Ok(event) => answered.push(id_of(&event)),
                    // The worker thread has ended, after its pool's threads.
                    Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) => {
                        assert!(Instant::now() < deadline, "the worker thread did not end");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            }
            assert!(
                answered.iter().all(|id| ids[..2].contains(id)),
                "a write queued at the drop was answered: {answered:?} of {ids:?}"
            );
            let logged = std::fs::read_to_string(&log).expect("read the fake's log");
            let reached = |file: &PathBuf| logged.contains(&*file.to_string_lossy());
            assert!(reached(&files[0]) && reached(&files[1]), "{logged}");
            assert!(
                !reached(&files[2]) && !reached(&files[3]),
                "a write queued at the drop reached ExifTool: {logged}"
            );
        }

        #[test]
        fn a_move_and_a_read_after_a_write_come_after_its_event() {
            let folder = TempDir::new("pooled-after-write");
            let journal = TempDir::new("pooled-after-write-journal");
            let raw = image(folder.path(), "TN0001.ARW", true);
            let other = image(folder.path(), "TN0002.jpg", false);
            let target = folder.path().join("_rejected");
            let mut worker = start_slow(journal.path(), 2, &[]);

            let write = rate(&mut worker, &raw);
            let quick = rate(&mut worker, &other);
            let read = worker
                .read_metadata(raw.clone())
                .expect("request should be accepted");
            let moving = worker
                .move_image(raw.clone(), target.clone(), strings(&["arw"]), Vec::new())
                .expect("request should be accepted");

            let events: Vec<Event> = (0..4).map(|_| next_event(&worker)).collect();
            let ids: Vec<RequestId> = events.iter().map(id_of).collect();
            assert_eq!(ids, vec![quick, write, read, moving]);
            match &events[3] {
                Event::Moved { result, .. } => assert!(result.is_ok(), "{result:?}"),
                other => panic!("expected the move, got {other:?}"),
            }
            assert!(target.join(raw.file_name().expect("a name")).is_file());
        }

        #[test]
        fn a_failed_pool_start_answers_the_write_other_and_the_next_write_tries_again() {
            let folder = TempDir::new("pooled-start-fails");
            let journal = TempDir::new("pooled-start-fails-journal");
            let file = image(folder.path(), "a.jpg", false);
            let marker = folder.path().join("fail-the-pool-start");
            std::fs::write(&marker, b"").expect("write the marker");
            let extra = [format!("{FAIL_START_WHILE}{}", marker.display())];
            let mut worker = start_slow(journal.path(), 2, &extra);

            let first = write_result(&mut worker, &file);
            assert_eq!(
                first,
                Err(WriteError::Other(
                    "I/O error: the test asks the pool start to fail".to_string()
                ))
            );
            std::fs::remove_file(&marker).expect("remove the marker");
            let second = write_result(&mut worker, &file);
            // The pool started and its process answered: the fake's answer is not a file's
            // metadata, so the safe write stops there, with another error than the start's.
            assert!(second.is_err());
            assert_ne!(second, first);
        }

        /// Sends slow writes of different images until the queue is full; the number accepted.
        fn accepted_until_full(worker: &mut MetadataWorker, folder: &Path) -> usize {
            for number in 0..40 {
                let file = image(folder, &format!("f{number}.jpg"), true);
                match worker.write_rating(
                    file,
                    Rating {
                        stars: Some(1),
                        label: None,
                    },
                ) {
                    Ok(_) => {}
                    Err(SubmitError::QueueFull) => return number,
                    Err(SubmitError::Stopped) => panic!("the worker stopped"),
                }
            }
            panic!("the queue never filled");
        }

        #[test]
        fn more_writes_than_processes_fill_the_queue_as_with_one() {
            for n in [1, 2, 4] {
                let folder = TempDir::new(&format!("pooled-full-{n}"));
                let journal = TempDir::new(&format!("pooled-full-{n}-journal"));
                let mut worker = start_slow(journal.path(), n, &[]);

                let accepted = accepted_until_full(&mut worker, folder.path());

                // The queue, the writes given (one with one process) and one held back.
                assert!(
                    (REQUEST_CAPACITY..=REQUEST_CAPACITY + n + 1).contains(&accepted),
                    "{accepted} accepted with {n} processes"
                );
                for _ in 0..accepted {
                    next_event(&worker);
                }
            }
        }
    }

    /// The tests below write through a real ExifTool (`exiftool` on PATH, or `PW_EXIFTOOL_PL`
    /// run through `perl`) and fail with an explanation when it is missing (docs/TESTING.md
    /// §4.5). The safe write uses Windows file operations.
    #[cfg(windows)]
    mod real_exiftool {
        use super::*;
        use crate::MIN_EXIFTOOL_VERSION;
        use crate::iim::Fallback;
        use crate::reconcile::FileKind;
        use crate::safe_write::{OriginalState, Stage, WriteReport};
        use crate::write_plan::WriteSettings;

        #[path = "../../../../../pw-thumbcache/src/jpeg_fixture.rs"]
        mod jpeg_fixture;

        /// The ExifTool program and its leading arguments; panics when ExifTool does not answer.
        fn exiftool() -> (String, Vec<String>) {
            let (program, args, source) = match std::env::var("PW_EXIFTOOL_PL") {
                Ok(pl) => (
                    "perl".to_string(),
                    vec![pl.clone()],
                    format!("perl {pl} (PW_EXIFTOOL_PL)"),
                ),
                Err(_) => (
                    "exiftool".to_string(),
                    Vec::new(),
                    "exiftool on PATH".into(),
                ),
            };
            let pre_args: Vec<&str> = args.iter().map(String::as_str).collect();
            let answered = ExifTool::start_with(&program, &pre_args)
                .and_then(|mut tool| tool.execute(&["-ver".to_string()]));
            if let Err(error) = answered {
                panic!(
                    "ExifTool is required for this test but does not answer from {source}: \
                     {error}. Install ExifTool or set PW_EXIFTOOL_PL (docs/TESTING.md §4.5)."
                );
            }
            (program, args)
        }

        fn start(checks: Arc<dyn ImageCheck + Send + Sync>, journal: &Path) -> MetadataWorker {
            let (program, args) = exiftool();
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            MetadataWorker::start_with(&program, &args, Some(setup_with(journal, checks)))
                .expect("worker should start")
        }

        fn jpeg(folder: &Path) -> PathBuf {
            let path = folder.join("bild.jpg");
            std::fs::write(&path, jpeg_fixture::SMALL).expect("write the JPEG");
            path
        }

        fn entries(folder: &Path) -> Vec<PathBuf> {
            std::fs::read_dir(folder)
                .expect("read the folder")
                .map(|entry| entry.expect("folder entry").path())
                .collect()
        }

        /// An image check whose copy check always fails.
        struct FailingCheck;

        impl ImageCheck for FailingCheck {
            fn release(&self, _image: &Path) {}

            fn release_for_move(&self, _image: &Path) {}

            fn check_copy(
                &self,
                _original: &Path,
                _copy: &Path,
                _kind: FileKind,
            ) -> std::result::Result<(), String> {
                Err("the test check refuses the copy".to_string())
            }
        }

        #[test]
        fn the_real_exiftool_is_new_enough_within_five_seconds() {
            let journal = TempDir::new("real-version-journal");
            let (program, args) = exiftool();
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let worker =
                MetadataWorker::start_with(&program, &args, Some(test_setup(journal.path())))
                    .expect("worker should start");
            match wait_for_version(&worker, Duration::from_secs(5)) {
                Some(Ok(version)) => assert!(
                    version >= MIN_EXIFTOOL_VERSION,
                    "ExifTool {version} is older than {MIN_EXIFTOOL_VERSION}"
                ),
                other => panic!("expected the version within 5 s, got {other:?}"),
            }
        }

        #[test]
        fn a_rating_is_written_safely_and_leaves_an_empty_journal() {
            let folder = TempDir::new("safe-jpeg");
            let journal = TempDir::new("safe-jpeg-journal");
            let image = jpeg(folder.path());
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            assert_eq!(write_result(&mut worker, &image), Ok(()));

            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
            assert_eq!(entries(folder.path()), vec![image.clone()]);
            let ratings = read_result(&mut worker, std::slice::from_ref(&image))
                .expect("the ratings should be read");
            assert_eq!(ratings.get(&image).and_then(|rating| rating.stars), Some(3));
        }

        #[test]
        fn an_orientation_sent_after_a_rating_is_answered_after_it() {
            let folder = TempDir::new("orientation-after-rating");
            let journal = TempDir::new("orientation-after-rating-journal");
            let raw = folder.path().join("TN0001.ARW");
            std::fs::write(&raw, b"FAKE_RAW").expect("write the fake RAW");
            let sidecar = folder.path().join("TN0001.xmp");
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            let rating = Rating {
                stars: Some(3),
                label: None,
            };
            let rating_id = worker
                .write_rating(raw.clone(), rating)
                .expect("request should be accepted");
            let orientation_id = worker
                .write_orientation(raw.clone(), 6)
                .expect("request should be accepted");

            match next_event(&worker) {
                Event::RatingWritten { id, result, .. } => {
                    assert_eq!(id, rating_id);
                    assert_eq!(result, Ok(()));
                }
                other => panic!("expected the rating first, got {other:?}"),
            }
            match next_event(&worker) {
                Event::OrientationWritten {
                    id,
                    orientation,
                    result,
                    ..
                } => {
                    assert_eq!(id, orientation_id);
                    assert_eq!(orientation, 6);
                    assert_eq!(result, Ok(()));
                }
                other => panic!("expected the orientation second, got {other:?}"),
            }
            let mut et = direct();
            assert_eq!(tag(&mut et, &sidecar, "XMP-xmp:Rating"), "3");
            assert_eq!(tag(&mut et, &sidecar, "XMP-tiff:Orientation#"), "6");
            assert_eq!(std::fs::read(&raw).expect("read the RAW"), b"FAKE_RAW");
            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
        }

        #[test]
        fn a_read_only_jpeg_is_read_only() {
            let folder = TempDir::new("read-only-jpeg");
            let journal = TempDir::new("read-only-jpeg-journal");
            let image = jpeg(folder.path());
            let writable = std::fs::metadata(&image).expect("metadata").permissions();
            let mut read_only = writable.clone();
            read_only.set_readonly(true);
            std::fs::set_permissions(&image, read_only).expect("set read-only");
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            let result = write_result(&mut worker, &image);

            std::fs::set_permissions(&image, writable).expect("restore the permissions");
            assert_eq!(result, Err(WriteError::ReadOnly));
            assert_eq!(std::fs::read(&image).expect("read"), jpeg_fixture::SMALL);
        }

        #[test]
        fn a_failing_image_check_is_not_safe_and_the_file_is_intact() {
            let folder = TempDir::new("failing-check");
            let journal = TempDir::new("failing-check-journal");
            let image = jpeg(folder.path());
            let mut worker = start(Arc::new(FailingCheck), journal.path());

            let result = write_result(&mut worker, &image);

            match result {
                Err(WriteError::NotSafe {
                    stage, original, ..
                }) => {
                    assert_eq!(stage, Stage::ImageCheck);
                    assert_eq!(original, OriginalState::Intact);
                }
                other => panic!("expected NotSafe with Intact, got {other:?}"),
            }
            assert_eq!(std::fs::read(&image).expect("read"), jpeg_fixture::SMALL);
            assert_eq!(entries(folder.path()), vec![image.clone()]);
            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
        }

        /// An ExifTool of the test's own, to prepare and inspect files beside the worker's.
        fn direct() -> ExifTool {
            let (program, args) = exiftool();
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            ExifTool::start_with(&program, &args).expect("ExifTool should start")
        }

        /// Writes `tags` to `file` with `et`.
        fn write_tags(et: &mut ExifTool, file: &Path, tags: &[&str]) {
            let mut args = vec!["-overwrite_original".to_string()];
            args.extend(tags.iter().map(|tag| (*tag).to_string()));
            args.push(file.to_string_lossy().into_owned());
            let out = et.execute(&args).expect("ExifTool should answer");
            assert!(
                out.contains("1 image files updated"),
                "ExifTool did not write {}: {out}",
                file.display()
            );
        }

        /// The value of the tag `name` in `file` as ExifTool prints it, `-` when it is missing.
        fn tag(et: &mut ExifTool, file: &Path, name: &str) -> String {
            et.execute(&[
                "-s3".to_string(),
                "-f".to_string(),
                format!("-{name}"),
                file.to_string_lossy().into_owned(),
            ])
            .expect("ExifTool should answer")
            .trim()
            .to_string()
        }

        /// The value `read` holds for `property`.
        fn value(read: &MetadataRead, property: &str) -> Option<crate::value::Value> {
            read.reconciled
                .metadata
                .get(property)
                .map(|stored| stored.value.clone())
        }

        fn text(text: &str) -> Option<crate::value::Value> {
            Some(crate::value::Value::Text(text.to_string()))
        }

        #[test]
        fn read_metadata_gives_the_headline_of_a_jpeg() {
            let folder = TempDir::new("read-headline");
            let journal = TempDir::new("read-headline-journal");
            let image = jpeg(folder.path());
            write_tags(&mut direct(), &image, &["-XMP-photoshop:Headline=Rubrik"]);
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            let read = metadata_result(&mut worker, &image).expect("the metadata should be read");

            assert_eq!(value(&read, "photoshop:Headline"), text("Rubrik"));
        }

        #[test]
        fn read_metadata_of_a_missing_file_is_an_error() {
            let folder = TempDir::new("read-missing");
            let journal = TempDir::new("read-missing-journal");
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            let result = metadata_result(&mut worker, &folder.path().join("saknas.jpg"));

            assert!(result.is_err(), "{result:?}");
        }

        #[test]
        fn a_headline_edit_is_written_and_read_back() {
            let folder = TempDir::new("edits-headline");
            let journal = TempDir::new("edits-headline-journal");
            let image = jpeg(folder.path());
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            let report = edits_result(
                &mut worker,
                &image,
                vec![set_text("photoshop:Headline", "Rubrik")],
            )
            .expect("the edits should be written");

            assert_eq!(
                report.report,
                WriteReport::Written {
                    target: image.clone()
                }
            );
            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
            let read = metadata_result(&mut worker, &image).expect("the metadata should be read");
            assert_eq!(value(&read, "photoshop:Headline"), text("Rubrik"));
        }

        #[test]
        fn set_options_with_create_iim_writes_the_city_to_iim() {
            let folder = TempDir::new("options-create-iim");
            let journal = TempDir::new("options-create-iim-journal");
            let image = jpeg(folder.path());
            let mut et = direct();
            assert_eq!(
                tag(&mut et, &image, "IPTC:City"),
                "-",
                "the fixture has IIM"
            );
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            worker
                .set_options(EditOptions {
                    settings: WriteSettings { create_iim: true },
                    fallback: Fallback::default(),
                })
                .expect("the options should be accepted");
            edits_result(
                &mut worker,
                &image,
                vec![set_text("photoshop:City", "Lund")],
            )
            .expect("the edits should be written");

            assert_eq!(tag(&mut et, &image, "IPTC:City"), "Lund");
        }

        /// T266: a worker over the real ExifTool with `n` write processes.
        fn start_pooled(journal: &Path, n: usize) -> MetadataWorker {
            let (program, args) = exiftool();
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let setup = WriteSetup {
                write_processes: n,
                ..test_setup(journal)
            };
            MetadataWorker::start_with(&program, &args, Some(setup)).expect("worker should start")
        }

        #[test]
        fn eight_copies_are_written_with_four_processes() {
            let folder = TempDir::new("pooled-eight");
            let journal = TempDir::new("pooled-eight-journal");
            let images: Vec<PathBuf> = (0..8)
                .map(|number| {
                    let path = folder.path().join(format!("bild{number}.jpg"));
                    std::fs::write(&path, jpeg_fixture::SMALL).expect("write the JPEG");
                    path
                })
                .collect();
            let mut worker = start_pooled(journal.path(), 4);

            let mut ids = Vec::new();
            for (number, image) in images.iter().enumerate() {
                let stars = i8::try_from(number % 5 + 1).expect("a small number");
                let rating = Rating {
                    stars: Some(stars),
                    label: None,
                };
                let deadline = Instant::now() + Duration::from_secs(60);
                loop {
                    match worker.write_rating(image.clone(), rating.clone()) {
                        Ok(id) => {
                            ids.push((id, image.clone(), stars));
                            break;
                        }
                        Err(SubmitError::QueueFull) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        Err(error) => panic!("the write was not accepted: {error:?}"),
                    }
                }
            }
            let mut answered = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(120);
            while answered.len() < ids.len() {
                match worker.try_recv() {
                    Ok(Event::RatingWritten { id, result, .. }) => {
                        assert_eq!(result, Ok(()), "write {id}");
                        answered.push(id);
                    }
                    Ok(other) => panic!("unexpected event {other:?}"),
                    Err(TryRecvError::Empty) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("not every write answered: {error:?}"),
                }
            }

            answered.sort_unstable();
            let mut sent: Vec<RequestId> = ids.iter().map(|(id, _, _)| *id).collect();
            sent.sort_unstable();
            assert_eq!(answered, sent);
            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
            let mut et = direct();
            for (_, image, stars) in &ids {
                assert_eq!(tag(&mut et, image, "XMP-xmp:Rating"), stars.to_string());
            }
            let mut left = entries(folder.path());
            left.sort();
            assert_eq!(left, images);
        }

        #[test]
        fn dropping_the_worker_while_writes_run_leaves_each_file_unchanged_or_written() {
            let folder = TempDir::new("pooled-drop");
            let journal = TempDir::new("pooled-drop-journal");
            let images: Vec<PathBuf> = (0..8)
                .map(|number| {
                    let path = folder.path().join(format!("drop{number}.jpg"));
                    std::fs::write(&path, jpeg_fixture::SMALL).expect("write the JPEG");
                    path
                })
                .collect();
            let mut worker = start_pooled(journal.path(), 4);
            let rating = Rating {
                stars: Some(4),
                label: None,
            };
            for image in &images {
                let deadline = Instant::now() + Duration::from_secs(60);
                loop {
                    match worker.write_rating(image.clone(), rating.clone()) {
                        Ok(_) => break,
                        Err(SubmitError::QueueFull) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        Err(error) => panic!("the write was not accepted: {error:?}"),
                    }
                }
            }
            // The first answer: the pool's processes are writing now.
            match next_event(&worker) {
                Event::RatingWritten { result, .. } => assert_eq!(result, Ok(())),
                other => panic!("unexpected event {other:?}"),
            }

            drop(worker);

            // The worker thread is not joined: wait until the journal is empty and only the
            // images are left, and still so two seconds later (the started writes have ended).
            let settled = || {
                entries(journal.path()).is_empty() && entries(folder.path()).len() == images.len()
            };
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                if settled() {
                    std::thread::sleep(Duration::from_secs(2));
                    if settled() {
                        break;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "journal entries or temporary files left: {:?}, {:?}",
                    entries(journal.path()),
                    entries(folder.path())
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            let mut et = direct();
            let mut written = 0;
            for image in &images {
                let bytes = std::fs::read(image).expect("read the image");
                if bytes != jpeg_fixture::SMALL {
                    assert_eq!(
                        tag(&mut et, image, "XMP-xmp:Rating"),
                        "4",
                        "{} is changed but not written",
                        image.display()
                    );
                    written += 1;
                }
            }
            assert!(
                written >= 1,
                "the write answered before the drop is missing"
            );
        }

        #[test]
        fn set_options_between_two_writes_with_two_processes() {
            let folder = TempDir::new("pooled-options");
            let journal = TempDir::new("pooled-options-journal");
            let before = folder.path().join("before.jpg");
            let after = folder.path().join("after.jpg");
            std::fs::write(&before, jpeg_fixture::SMALL).expect("write the JPEG");
            std::fs::write(&after, jpeg_fixture::SMALL).expect("write the JPEG");
            let mut worker = start_pooled(journal.path(), 2);

            let first = worker
                .write_edits(before.clone(), vec![set_text("photoshop:City", "Lund")])
                .expect("request should be accepted");
            worker
                .set_options(EditOptions {
                    settings: WriteSettings { create_iim: true },
                    fallback: Fallback::default(),
                })
                .expect("the options should be accepted");
            let second = worker
                .write_edits(after.clone(), vec![set_text("photoshop:City", "Lund")])
                .expect("request should be accepted");
            let mut answered = Vec::new();
            for _ in 0..2 {
                match next_event(&worker) {
                    Event::EditsWritten { id, result, .. } => {
                        assert!(result.is_ok(), "{result:?}");
                        answered.push(id);
                    }
                    other => panic!("unexpected event {other:?}"),
                }
            }
            answered.sort_unstable();
            assert_eq!(answered, vec![first, second]);

            let mut et = direct();
            assert_eq!(tag(&mut et, &before, "XMP-photoshop:City"), "Lund");
            assert_eq!(tag(&mut et, &before, "IPTC:City"), "-");
            assert_eq!(tag(&mut et, &after, "XMP-photoshop:City"), "Lund");
            assert_eq!(tag(&mut et, &after, "IPTC:City"), "Lund");
            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
        }

        #[test]
        fn without_set_options_the_city_is_not_written_to_iim() {
            let folder = TempDir::new("options-no-iim");
            let journal = TempDir::new("options-no-iim-journal");
            let image = jpeg(folder.path());
            let mut et = direct();
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            edits_result(
                &mut worker,
                &image,
                vec![set_text("photoshop:City", "Lund")],
            )
            .expect("the edits should be written");

            assert_eq!(tag(&mut et, &image, "XMP-photoshop:City"), "Lund");
            assert_eq!(tag(&mut et, &image, "IPTC:City"), "-");
        }

        #[test]
        fn set_options_with_latin1_changes_how_iim_text_is_read() {
            let folder = TempDir::new("options-latin1");
            let journal = TempDir::new("options-latin1-journal");
            let image = jpeg(folder.path());
            let mut et = direct();
            // ExifTool writes IIM in Windows-1252 without 1:90 by default: `€` is the byte 0x80.
            write_tags(&mut et, &image, &["-IPTC:City=€"]);
            assert_eq!(tag(&mut et, &image, "IPTC:CodedCharacterSet"), "-");
            let mut worker = start(Arc::new(NoImageCheck), journal.path());

            let before = metadata_result(&mut worker, &image).expect("the metadata should be read");
            worker
                .set_options(EditOptions {
                    settings: WriteSettings::default(),
                    fallback: Fallback::Latin1,
                })
                .expect("the options should be accepted");
            let after = metadata_result(&mut worker, &image).expect("the metadata should be read");

            assert_eq!(value(&before, "photoshop:City"), text("€"));
            assert_eq!(value(&after, "photoshop:City"), text("\u{80}"));
        }
    }
}
