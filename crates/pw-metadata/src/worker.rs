//! A background thread that owns the ExifTool process and runs metadata requests one at a time.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;

use crate::{ExifTool, Rating, Result};

const REQUEST_CAPACITY: usize = 8;

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
        result: std::result::Result<(), String>,
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
    /// Starts a worker using `exiftool` from PATH.
    pub fn start() -> Result<Self> {
        Self::start_with("exiftool", &[])
    }

    /// Starts a worker using an explicit ExifTool program and leading arguments.
    pub fn start_with(program: &str, pre_args: &[&str]) -> Result<Self> {
        let (request_tx, request_rx) = mpsc::sync_channel(REQUEST_CAPACITY);
        let (event_tx, event_rx) = mpsc::sync_channel(REQUEST_CAPACITY);
        let program = program.to_string();
        let pre_args: Vec<String> = pre_args.iter().map(|arg| (*arg).to_string()).collect();
        let thread = std::thread::Builder::new()
            .name("pw-metadata".into())
            .spawn(move || run(&program, &pre_args, &request_rx, &event_tx))?;
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
        let event = handle(exiftool.as_mut(), start_error, request);
        // A broken process cannot be trusted with another command; dropping it shuts it down.
        if exiftool.as_ref().is_some_and(ExifTool::is_broken) {
            exiftool = None;
        }
        if events.send(event).is_err() {
            break;
        }
    }
}

fn handle(tool: Option<&mut ExifTool>, start_error: Option<String>, request: Request) -> Event {
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
            let result = match tool {
                Some(tool) => tool.write_rating(&path, &rating).map_err(|e| e.to_string()),
                None => Err(unavailable()),
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
    use crate::test_support::{fake_args, fake_exiftool};
    use std::time::{Duration, Instant};

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
    ) -> std::result::Result<(), String> {
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

    #[test]
    fn reports_exiftool_start_errors_asynchronously() {
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-missing-exiftool-{}",
            std::process::id()
        ));
        let mut worker = match MetadataWorker::start_with(&program.to_string_lossy(), &[]) {
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

    #[test]
    fn restarts_exiftool_after_it_exits() {
        let (program, args) = fake_exiftool("exit-after-first");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker = MetadataWorker::start_with(&program, &args).expect("worker should start");
        let raw = std::env::temp_dir().join("pw-metadata-restart-test.ARW");
        assert_eq!(write_result(&mut worker, &raw), Ok(()));
        let failed = write_result(&mut worker, &raw);
        assert!(failed.is_err(), "the crashed process must report an error");
        assert_eq!(
            write_result(&mut worker, &raw),
            Ok(()),
            "a new process should serve the next request"
        );
    }

    #[test]
    fn retries_a_failed_start_with_the_next_request() {
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-late-exiftool-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        let _ = std::fs::remove_file(&program);
        let args = fake_args("normal");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut worker = MetadataWorker::start_with(&program.to_string_lossy(), &args)
            .expect("worker should start");
        let raw = std::env::temp_dir().join("pw-metadata-retry-test.ARW");
        let first = write_result(&mut worker, &raw);
        assert!(
            first.as_ref().is_err_and(|e| e.contains("could not start")),
            "{first:?}"
        );
        // "Install" ExifTool: the fake is a copy of this test binary.
        let exe = std::env::current_exe().expect("test binary path");
        std::fs::copy(&exe, &program).expect("copy fake ExifTool");
        assert_eq!(write_result(&mut worker, &raw), Ok(()));
        drop(worker);
        // Best effort: on Windows the copy may still be running for a moment.
        let _ = std::fs::remove_file(&program);
    }
}
