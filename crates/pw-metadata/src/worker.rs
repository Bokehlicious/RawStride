//! A background thread that owns the ExifTool process and runs metadata requests one at a time.
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
//! - [`MetadataWorker::set_options`] changes the [`EditOptions`] without an event. The worker
//!   starts with the setup's options (the default without a setup); every request queued after
//!   `set_options` uses the new options for rating writes, edits and reads (the `fallback`).
//!   Requests run in order, so a request queued before it uses the old options.
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
//! Requests run in order, so a move or a delete sent after a rating write of the same image runs
//! after it. Moves and deletes do not use ExifTool; without a [`WriteSetup`] (no `checks` to
//! release the sessions) the moves and `delete_image` fail with [`NO_JOURNAL`] and nothing is
//! moved or deleted. `prepare_delete` works without a setup.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use crate::edit::{Edit, EditOptions, EditReport};
use crate::exiftool::{MIN_EXIFTOOL_VERSION, ToolVersion, exiftool_program};
use crate::journal::Journal;
use crate::read::MetadataRead;
use crate::recycle::{self, DeleteError, DeleteRefusal};
use crate::relocate::{self, Moved};
use crate::safe_write::ImageCheck;
use crate::{ExifTool, Rating, Result, WriteError};

const REQUEST_CAPACITY: usize = 8;

/// The text of [`WriteError::NoJournal`]: a write when the worker has no [`WriteSetup`].
pub const NO_JOURNAL: &str = "the write journal could not be opened";

/// The text of [`WriteError::ExifToolTooOld`]: a write while the running ExifTool is older than
/// [`MIN_EXIFTOOL_VERSION`] or its version could not be read (D9).
pub const EXIFTOOL_TOO_OLD: &str = "ExifTool is older than 12.67";

/// The first result of a start: the version, or the start or version error as text.
type VersionResult = std::result::Result<ToolVersion, String>;

/// What a rating write needs (T91): the app's write journal, its image checks and the edit
/// options. `options` are the options the worker starts with; [`MetadataWorker::set_options`]
/// changes them later (T98).
pub struct WriteSetup {
    pub journal: Journal,
    pub checks: Box<dyn ImageCheck + Send>,
    pub options: EditOptions,
}

impl fmt::Debug for WriteSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteSetup")
            .field("journal", &self.journal)
            .field("options", &self.options)
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
    },
    RatingWritten {
        id: RequestId,
        path: PathBuf,
        rating: Rating,
        /// The kind of failure is kept, so the program can show its own text (T49).
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

/// Bounded, serial metadata I/O for UI clients. The ExifTool process starts on this worker.
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
                );
            })?;
        Ok(Self {
            requests: Some(request_tx),
            events: event_rx,
            thread: Some(thread),
            next_id: 0,
            version,
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

    /// Queues a rating update without blocking. Writes run in request order.
    pub fn write_rating(
        &mut self,
        path: PathBuf,
        rating: Rating,
    ) -> std::result::Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Write { id, path, rating })?;
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
    /// with [`Event::EditsWritten`]. Writes run in request order.
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
        self.requests.take();
        // Do not wait for an in-progress ExifTool operation on the UI thread.
        self.thread.take();
    }
}

/// A started ExifTool process and whether its version allows writes (D9).
struct Running {
    tool: ExifTool,
    may_write: bool,
}

/// Starts ExifTool and asks its version. The first result (the version, the version error or
/// the start error) is kept in `first`. A process that broke while answering is dropped and its
/// error returned, so the next request starts a new one.
fn start_tool(
    program: &str,
    pre_args: &[&str],
    first: &OnceLock<VersionResult>,
) -> std::result::Result<Running, String> {
    let mut tool = match ExifTool::start_with(program, pre_args) {
        Ok(tool) => tool,
        Err(error) => {
            let text = error.to_string();
            let _ = first.set(Err(text.clone()));
            return Err(text);
        }
    };
    let version = tool.version().map_err(|error| error.to_string());
    let may_write = version
        .as_ref()
        .is_ok_and(|version| *version >= MIN_EXIFTOOL_VERSION);
    let broken = tool.is_broken();
    let error = version.as_ref().err().cloned();
    let _ = first.set(version);
    if broken {
        return Err(error.unwrap_or_else(|| "ExifTool process is unusable".into()));
    }
    Ok(Running { tool, may_write })
}

/// The worker loop: runs until the request sender is dropped or the event receiver is gone.
fn run(
    program: &str,
    pre_args: &[String],
    setup: Option<&WriteSetup>,
    first_version: &OnceLock<VersionResult>,
    requests: &Receiver<Command>,
    events: &SyncSender<Event>,
) {
    let pre_args: Vec<&str> = pre_args.iter().map(String::as_str).collect();
    // The current options (T98): the setup's, the default without a setup.
    let mut options = setup.map_or_else(EditOptions::default, |setup| setup.options);
    // Start at once, so the version is known before the first request (T94). A failed start is
    // tried again with the next request.
    let mut exiftool = start_tool(program, &pre_args, first_version).ok();
    while let Ok(command) = requests.recv() {
        let request = match command {
            Command::Run(request) => request,
            Command::SetOptions(new) => {
                options = new;
                continue;
            }
        };
        // Restart when needed, so a missing or crashed ExifTool is retried with each request.
        let mut start_error = None;
        if exiftool.is_none() {
            match start_tool(program, &pre_args, first_version) {
                Ok(running) => exiftool = Some(running),
                Err(error) => start_error = Some(error),
            }
        }
        let event = handle(exiftool.as_mut(), start_error, setup, &options, request);
        // A broken process cannot be trusted with another command; dropping it shuts it down.
        if exiftool
            .as_ref()
            .is_some_and(|running| running.tool.is_broken())
        {
            exiftool = None;
        }
        if events.send(event).is_err() {
            break;
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
            let result = match running {
                Some(running) => running
                    .tool
                    .read_ratings(&images)
                    .map_err(|e| e.to_string()),
                None => Err(unavailable()),
            };
            Event::RatingsRead { id, result }
        }
        Request::Write { id, path, rating } => {
            let result = match (setup, running) {
                (None, _) => Err(WriteError::NoJournal),
                (Some(_), Some(running)) if !running.may_write => Err(WriteError::ExifToolTooOld),
                (Some(setup), Some(running)) => running
                    .tool
                    .write_rating_safely(&path, &rating, options, &*setup.checks, &setup.journal)
                    .map(|_report| ())
                    .map_err(WriteError::from),
                (Some(_), None) => Err(WriteError::Other(unavailable())),
            };
            Event::RatingWritten {
                id,
                path,
                rating,
                result,
            }
        }
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
        Request::WriteEdits { id, path, edits } => {
            let result = match (setup, running) {
                (None, _) => Err(WriteError::NoJournal),
                (Some(_), Some(running)) if !running.may_write => Err(WriteError::ExifToolTooOld),
                (Some(setup), Some(running)) => running
                    .tool
                    .write_edits_safely(&path, &edits, options, &*setup.checks, &setup.journal)
                    .map_err(WriteError::from),
                (Some(_), None) => Err(WriteError::Other(unavailable())),
            };
            Event::EditsWritten { id, path, result }
        }
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

    /// A setup with `checks` and a journal in `folder`.
    fn setup_with(folder: &Path, checks: Box<dyn ImageCheck + Send>) -> WriteSetup {
        WriteSetup {
            journal: Journal::open(folder).expect("open the temporary journal"),
            checks,
            options: EditOptions::default(),
        }
    }

    /// A setup with `NoImageCheck` and a journal in `folder`.
    fn test_setup(folder: &Path) -> WriteSetup {
        setup_with(folder, Box::new(NoImageCheck))
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
            Some(setup_with(journal.path(), Box::new(check.clone()))),
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
            Some(setup_with(journal.path(), Box::new(check.clone()))),
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
            Some(setup_with(journal.path(), Box::new(check.clone()))),
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
                Some(setup_with(journal.path(), Box::new(check.clone()))),
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
                Some(setup_with(journal.path(), Box::new(check.clone()))),
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
                Some(setup_with(journal.path(), Box::new(check.clone()))),
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

    /// The tests below write through a real ExifTool (`exiftool` on PATH, or `PW_EXIFTOOL_PL`
    /// run through `perl`) and fail with an explanation when it is missing (docs/TESTING.md
    /// §4.5). The safe write uses Windows file operations.
    #[cfg(windows)]
    mod real_exiftool {
        use super::*;
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

        fn start(checks: Box<dyn ImageCheck + Send>, journal: &Path) -> MetadataWorker {
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
            let mut worker = start(Box::new(NoImageCheck), journal.path());

            assert_eq!(write_result(&mut worker, &image), Ok(()));

            assert_eq!(entries(journal.path()), Vec::<PathBuf>::new());
            assert_eq!(entries(folder.path()), vec![image.clone()]);
            let ratings = read_result(&mut worker, std::slice::from_ref(&image))
                .expect("the ratings should be read");
            assert_eq!(ratings.get(&image).and_then(|rating| rating.stars), Some(3));
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
            let mut worker = start(Box::new(NoImageCheck), journal.path());

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
            let mut worker = start(Box::new(FailingCheck), journal.path());

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
            let mut worker = start(Box::new(NoImageCheck), journal.path());

            let read = metadata_result(&mut worker, &image).expect("the metadata should be read");

            assert_eq!(value(&read, "photoshop:Headline"), text("Rubrik"));
        }

        #[test]
        fn read_metadata_of_a_missing_file_is_an_error() {
            let folder = TempDir::new("read-missing");
            let journal = TempDir::new("read-missing-journal");
            let mut worker = start(Box::new(NoImageCheck), journal.path());

            let result = metadata_result(&mut worker, &folder.path().join("saknas.jpg"));

            assert!(result.is_err(), "{result:?}");
        }

        #[test]
        fn a_headline_edit_is_written_and_read_back() {
            let folder = TempDir::new("edits-headline");
            let journal = TempDir::new("edits-headline-journal");
            let image = jpeg(folder.path());
            let mut worker = start(Box::new(NoImageCheck), journal.path());

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
            let mut worker = start(Box::new(NoImageCheck), journal.path());

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

        #[test]
        fn without_set_options_the_city_is_not_written_to_iim() {
            let folder = TempDir::new("options-no-iim");
            let journal = TempDir::new("options-no-iim-journal");
            let image = jpeg(folder.path());
            let mut et = direct();
            let mut worker = start(Box::new(NoImageCheck), journal.path());

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
            let mut worker = start(Box::new(NoImageCheck), journal.path());

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
