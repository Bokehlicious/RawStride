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

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use crate::edit::{Edit, EditOptions, EditReport};
use crate::exiftool::{MIN_EXIFTOOL_VERSION, ToolVersion};
use crate::journal::Journal;
use crate::read::MetadataRead;
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
    /// Starts a worker using `exiftool` from PATH. `setup` is `None` when the write journal could
    /// not be opened; rating writes then fail with [`WriteError::NoJournal`].
    pub fn start(setup: Option<WriteSetup>) -> Result<Self> {
        Self::start_with("exiftool", &[], setup)
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
    }
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
                Ok(other @ (Event::MetadataRead { .. } | Event::EditsWritten { .. })) => {
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
