//! A background thread that owns the ExifTool process and runs metadata requests one at a time.
//!
//! A rating is written through the write plan and the safe write (ADR 0007 §9, §15; T91):
//! [`ExifTool::write_rating_safely`] with the [`WriteSetup`] given at the start. Without a setup
//! (the program could not open its write journal) every rating write fails with [`NO_JOURNAL`]
//! and nothing is written; reading ratings works either way.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;

use crate::edit::EditOptions;
use crate::journal::Journal;
use crate::safe_write::ImageCheck;
use crate::{ExifTool, Rating, Result, WriteError};

const REQUEST_CAPACITY: usize = 8;

/// The text of [`WriteError::Other`] for a rating write when the worker has no [`WriteSetup`].
pub const NO_JOURNAL: &str = "the write journal could not be opened";

/// What a rating write needs (T91): the app's write journal, its image checks and the edit
/// options.
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
}

/// Bounded, serial metadata I/O for UI clients. The ExifTool process starts on this worker.
///
/// The worker recovers on its own: if ExifTool cannot be started, the start is retried with the
/// next request, and if it crashes or its output stream breaks, the process is discarded and a
/// new one is started for the next request. Each failed request is still reported as an error.
#[derive(Debug)]
pub struct MetadataWorker {
    requests: Option<SyncSender<Request>>,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
    next_id: RequestId,
}

impl MetadataWorker {
    /// Starts a worker using `exiftool` from PATH. `setup` is `None` when the write journal could
    /// not be opened; rating writes then fail with [`NO_JOURNAL`].
    pub fn start(setup: Option<WriteSetup>) -> Result<Self> {
        Self::start_with("exiftool", &[], setup)
    }

    /// Starts a worker using an explicit ExifTool program and leading arguments.
    pub fn start_with(program: &str, pre_args: &[&str], setup: Option<WriteSetup>) -> Result<Self> {
        let (request_tx, request_rx) = mpsc::sync_channel(REQUEST_CAPACITY);
        let (event_tx, event_rx) = mpsc::sync_channel(REQUEST_CAPACITY);
        let program = program.to_string();
        let pre_args: Vec<String> = pre_args.iter().map(|arg| (*arg).to_string()).collect();
        let thread = std::thread::Builder::new()
            .name("pw-metadata".into())
            .spawn(move || run(&program, &pre_args, setup.as_ref(), &request_rx, &event_tx))?;
        Ok(Self {
            requests: Some(request_tx),
            events: event_rx,
            thread: Some(thread),
            next_id: 0,
        })
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

    /// Returns the next completed operation, if one is available.
    pub fn try_recv(&self) -> std::result::Result<Event, TryRecvError> {
        self.events.try_recv()
    }

    fn next_request_id(&mut self) -> RequestId {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    fn submit(&self, request: Request) -> std::result::Result<(), SubmitError> {
        let Some(requests) = &self.requests else {
            return Err(SubmitError::Stopped);
        };
        match requests.try_send(request) {
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

/// The worker loop: runs until the request sender is dropped or the event receiver is gone.
fn run(
    program: &str,
    pre_args: &[String],
    setup: Option<&WriteSetup>,
    requests: &Receiver<Request>,
    events: &SyncSender<Event>,
) {
    let pre_args: Vec<&str> = pre_args.iter().map(String::as_str).collect();
    let mut exiftool: Option<ExifTool> = None;
    while let Ok(request) = requests.recv() {
        // (Re)start lazily, so a missing or crashed ExifTool is retried with each request.
        let mut start_error = None;
        if exiftool.is_none() {
            match ExifTool::start_with(program, &pre_args) {
                Ok(tool) => exiftool = Some(tool),
                Err(error) => start_error = Some(error.to_string()),
            }
        }
        let event = handle(exiftool.as_mut(), start_error, setup, request);
        // A broken process cannot be trusted with another command; dropping it shuts it down.
        if exiftool.as_ref().is_some_and(ExifTool::is_broken) {
            exiftool = None;
        }
        if events.send(event).is_err() {
            break;
        }
    }
}

fn handle(
    tool: Option<&mut ExifTool>,
    start_error: Option<String>,
    setup: Option<&WriteSetup>,
    request: Request,
) -> Event {
    let unavailable = || start_error.unwrap_or_else(|| "ExifTool worker is unavailable".into());
    match request {
        Request::Read { id, images } => {
            let result = match tool {
                Some(tool) => tool.read_ratings(&images).map_err(|e| e.to_string()),
                None => Err(unavailable()),
            };
            Event::RatingsRead { id, result }
        }
        Request::Write { id, path, rating } => {
            let result = match (setup, tool) {
                (None, _) => Err(WriteError::Other(NO_JOURNAL.to_string())),
                (Some(setup), Some(tool)) => tool
                    .write_rating_safely(
                        &path,
                        &rating,
                        &setup.options,
                        &*setup.checks,
                        &setup.journal,
                    )
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
        let (program, args) = fake_exiftool("exit-after-first");
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

        assert_eq!(
            write_result(&mut worker, &raw),
            Err(WriteError::Other(NO_JOURNAL.to_string()))
        );
        assert!(!folder.path().join("TN0001.xmp").exists());
    }

    /// The tests below write through a real ExifTool (`exiftool` on PATH, or `PW_EXIFTOOL_PL`
    /// run through `perl`) and fail with an explanation when it is missing (docs/TESTING.md
    /// §4.5). The safe write uses Windows file operations.
    #[cfg(windows)]
    mod real_exiftool {
        use super::*;
        use crate::reconcile::FileKind;
        use crate::safe_write::{OriginalState, Stage};

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
    }
}
