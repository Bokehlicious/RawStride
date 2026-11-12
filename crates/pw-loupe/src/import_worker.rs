use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::{Field, FieldError, ImportForm, Preview};
use pw_metadata::edit::EditOptions;
use pw_metadata::journal::Journal;
use pw_metadata::safe_write::ImageCheck;
use pw_metadata::{ExifTool, Iptc};

const REQUEST_CAPACITY: usize = 8;

/// The reason listed for the ingest template when the write journal could not be opened at the
/// start; nothing is written (T91).
const NO_JOURNAL_TEXT: &str = "skrivloggen kunde inte öppnas";

/// What the ingest template's safe write needs (ADR 0007 §9.5, T91): the ExifTool program
/// (`None` for `exiftool` on PATH), the app's write journal (`None` when it could not be opened)
/// and its image checks.
struct WriteContext {
    exiftool_program: Option<String>,
    journal: Option<Journal>,
    checks: Box<dyn ImageCheck + Send>,
}

pub type RequestId = u64;

#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError {
    QueueFull,
    Stopped,
}

#[derive(Debug, Clone)]
pub enum ImportEvent {
    Previewed {
        id: RequestId,
        result: Result<Preview, FieldError>,
    },
    Progress {
        id: RequestId,
        progress: pw_ingest::Progress,
    },
    IptcProgress {
        id: RequestId,
        done: usize,
        total: usize,
        current: PathBuf,
    },
    Ejecting {
        id: RequestId,
    },
    Cards {
        id: RequestId,
        cards: Vec<PathBuf>,
    },
    Done {
        id: RequestId,
        done: ImportDone,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportDone {
    pub copied: usize,
    pub present: usize,
    pub bytes: u64,
    pub failed: Vec<String>,
    pub iptc_errors: Vec<String>,
    pub eject: Option<String>,
    pub cancelled: bool,
    pub elapsed: Duration,
    pub folders: Vec<(String, usize)>,
    pub error: Option<String>,
}

#[derive(Debug)]
enum Request {
    Preview {
        id: RequestId,
        form: ImportForm,
        rescan: bool,
    },
    Run {
        id: RequestId,
        form: ImportForm,
    },
    Cards {
        id: RequestId,
    },
}

/// Performs import scans and copies away from the UI thread.
pub struct ImportWorker {
    requests: Option<SyncSender<Request>>,
    events: Receiver<ImportEvent>,
    thread: Option<JoinHandle<()>>,
    next_id: RequestId,
    cancel: Arc<AtomicBool>,
    run_gate: Arc<Mutex<()>>,
}

impl std::fmt::Debug for ImportWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImportWorker")
            .field("thread_running", &self.thread.is_some())
            .finish_non_exhaustive()
    }
}

impl ImportWorker {
    /// Starts the single import worker thread. The ingest template is written safely with
    /// `journal` and `checks`; when `journal` is `None` (it could not be opened), every template
    /// write fails and nothing is written.
    pub fn start(journal: Option<Journal>, checks: Box<dyn ImageCheck + Send>) -> io::Result<Self> {
        Self::start_inner(WriteContext {
            exiftool_program: None,
            journal,
            checks,
        })
    }

    /// Starts the worker with a specific ExifTool program, primarily for tests.
    pub fn start_with_exiftool(
        program: &str,
        journal: Option<Journal>,
        checks: Box<dyn ImageCheck + Send>,
    ) -> io::Result<Self> {
        Self::start_inner(WriteContext {
            exiftool_program: Some(program.to_string()),
            journal,
            checks,
        })
    }

    fn start_inner(write: WriteContext) -> io::Result<Self> {
        let (request_sender, request_receiver) = mpsc::sync_channel(REQUEST_CAPACITY);
        let (event_sender, event_receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let run_gate = Arc::new(Mutex::new(()));
        let worker_run_gate = Arc::clone(&run_gate);
        let thread = std::thread::Builder::new()
            .name("pw-import".into())
            .spawn(move || {
                run(
                    request_receiver,
                    event_sender,
                    worker_cancel,
                    worker_run_gate,
                    write,
                )
            })?;

        Ok(Self {
            requests: Some(request_sender),
            events: event_receiver,
            thread: Some(thread),
            next_id: 0,
            cancel,
            run_gate,
        })
    }

    /// Queues a preview, optionally reusing the last scan for this source.
    pub fn preview(&mut self, form: ImportForm, rescan: bool) -> Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Preview { id, form, rescan })?;
        Ok(id)
    }

    /// Queues a fresh scan and copy. A previous cancellation is cleared for this run.
    pub fn run(&mut self, form: ImportForm) -> Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        // The worker waits for this gate before it starts a run, so it cannot start copying
        // before the cancel flag of an earlier run is cleared below.
        let _gate = lock_recover(&self.run_gate);
        self.submit(Request::Run { id, form })?;
        self.cancel.store(false, Ordering::Relaxed);
        Ok(id)
    }

    /// Queues a card detection.
    pub fn cards(&mut self) -> Result<RequestId, SubmitError> {
        let id = self.next_request_id();
        self.submit(Request::Cards { id })?;
        Ok(id)
    }

    /// Requests cancellation of the current or next queued copy.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Returns the next completed event without waiting.
    pub fn try_recv(&self) -> Result<ImportEvent, TryRecvError> {
        self.events.try_recv()
    }

    fn next_request_id(&mut self) -> RequestId {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    fn submit(&self, request: Request) -> Result<(), SubmitError> {
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

impl Drop for ImportWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.requests.take();
        // Do not wait for a scan or copy on the UI thread.
        self.thread.take();
    }
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

fn run(
    requests: Receiver<Request>,
    events: mpsc::Sender<ImportEvent>,
    cancel: Arc<AtomicBool>,
    run_gate: Arc<Mutex<()>>,
    write: WriteContext,
) {
    let mut cached_scan: Option<(PathBuf, Arc<Vec<pw_ingest::Shot>>)> = None;
    let mut exiftool: Option<ExifTool> = None;
    while let Ok(request) = requests.recv() {
        let event = match request {
            Request::Preview { id, form, rescan } => ImportEvent::Previewed {
                id,
                result: preview(&form, rescan, &mut cached_scan),
            },
            Request::Run { id, form } => {
                // Wait until `ImportWorker::run` has cleared the cancel flag for this run.
                drop(lock_recover(&run_gate));
                run_import(
                    id,
                    &form,
                    &cancel,
                    &mut cached_scan,
                    &events,
                    &mut exiftool,
                    &write,
                )
            }
            Request::Cards { id } => ImportEvent::Cards {
                id,
                cards: pw_ingest::card_volumes(),
            },
        };
        if events.send(event).is_err() {
            break;
        }
    }
}

fn shots(
    source: &Path,
    rescan: bool,
    cached_scan: &mut Option<(PathBuf, Arc<Vec<pw_ingest::Shot>>)>,
) -> Result<Arc<Vec<pw_ingest::Shot>>, String> {
    if !rescan
        && let Some((cached_source, cached_shots)) = cached_scan.as_ref()
        && cached_source == source
    {
        return Ok(Arc::clone(cached_shots));
    }

    let scanned = Arc::new(
        pw_ingest::scan(source)
            .map_err(|error| format!("Kunde inte läsa {}: {error}", source.display()))?,
    );
    *cached_scan = Some((source.to_path_buf(), Arc::clone(&scanned)));
    Ok(scanned)
}

fn preview(
    form: &ImportForm,
    rescan: bool,
    cached_scan: &mut Option<(PathBuf, Arc<Vec<pw_ingest::Shot>>)>,
) -> Result<Preview, FieldError> {
    let validated = form.validate()?;
    let shots = shots(&validated.source, rescan, cached_scan).map_err(|message| FieldError {
        field: Field::Source,
        message,
    })?;
    let plan = pw_ingest::plan(&shots, &validated.options);
    Ok(Preview::new(&shots, &plan))
}

fn run_import(
    id: RequestId,
    form: &ImportForm,
    cancel: &AtomicBool,
    cached_scan: &mut Option<(PathBuf, Arc<Vec<pw_ingest::Shot>>)>,
    events: &mpsc::Sender<ImportEvent>,
    exiftool: &mut Option<ExifTool>,
    write: &WriteContext,
) -> ImportEvent {
    let started = Instant::now();
    let validated = match form.validate() {
        Ok(validated) => validated,
        Err(error) => {
            return ImportEvent::Done {
                id,
                done: ImportDone {
                    elapsed: Duration::ZERO,
                    error: Some(error.message),
                    ..ImportDone::default()
                },
            };
        }
    };
    let shots = match shots(&validated.source, true, cached_scan) {
        Ok(shots) => shots,
        Err(error) => {
            return ImportEvent::Done {
                id,
                done: ImportDone {
                    elapsed: started.elapsed(),
                    error: Some(error),
                    ..ImportDone::default()
                },
            };
        }
    };

    let plan = pw_ingest::plan(&shots, &validated.options);
    let report = pw_ingest::execute(&plan, &validated.options, cancel, |progress| {
        let _ = events.send(ImportEvent::Progress {
            id,
            progress: progress.clone(),
        });
    });
    let destination_items: Vec<&pw_ingest::PlanItem> = plan
        .items
        .iter()
        .filter(|item| validated.options.dest.join(&item.rel).exists())
        .collect();

    let mut done = ImportDone {
        copied: report.copied,
        present: report.skipped_present,
        bytes: report.bytes,
        failed: report
            .failed
            .iter()
            .map(|(path, error)| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{name}: {error}")
            })
            .collect(),
        cancelled: report.cancelled,
        elapsed: started.elapsed(),
        folders: pw_ingest::folder_summary(&destination_items, &validated.options.dest),
        error: None,
        ..ImportDone::default()
    };
    let mut iptc_runtime = IptcRuntime {
        id,
        cancel,
        events,
        exiftool,
        exiftool_program: write.exiftool_program.as_deref(),
        write,
    };
    apply_iptc(
        &plan,
        &report.failed,
        &validated.options,
        &form.iptc.to_iptc(),
        &mut iptc_runtime,
        &mut done,
    );
    if form.eject {
        if done.cancelled || !done.failed.is_empty() || done.error.is_some() {
            done.eject =
                Some("Kortet matades inte ut eftersom importen inte blev komplett.".to_string());
        } else {
            *cached_scan = None;
            let _ = events.send(ImportEvent::Ejecting { id });
            done.eject = Some(pw_ingest::eject(&validated.source));
        }
    }
    done.elapsed = started.elapsed();

    ImportEvent::Done { id, done }
}

struct IptcRuntime<'a> {
    id: RequestId,
    cancel: &'a AtomicBool,
    events: &'a mpsc::Sender<ImportEvent>,
    exiftool: &'a mut Option<ExifTool>,
    exiftool_program: Option<&'a str>,
    write: &'a WriteContext,
}

fn iptc_targets(
    plan: &pw_ingest::Plan,
    failed: &[(PathBuf, String)],
    options: &pw_ingest::Options,
) -> Vec<PathBuf> {
    let failed_sources: HashSet<PathBuf> =
        failed.iter().map(|(source, _)| source.clone()).collect();
    let mut targets = Vec::new();
    for item in &plan.items {
        if !(item.to_dest || item.to_backup) || failed_sources.contains(&item.source) {
            continue;
        }
        let is_companion = item
            .rel
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
            .is_some_and(|extension| pw_ingest::COMPANION_EXTENSIONS.contains(&extension.as_str()));
        if is_companion {
            continue;
        }
        if item.to_dest {
            targets.push(options.dest.join(&item.rel));
        }
        if item.to_backup
            && let Some(backup) = &options.backup
        {
            targets.push(backup.join(&item.rel));
        }
    }
    targets
}

fn apply_iptc(
    plan: &pw_ingest::Plan,
    failed: &[(PathBuf, String)],
    options: &pw_ingest::Options,
    iptc: &Iptc,
    runtime: &mut IptcRuntime<'_>,
    done: &mut ImportDone,
) {
    if done.cancelled || iptc.is_empty() {
        return;
    }

    let targets = iptc_targets(plan, failed, options);
    let template = iptc.to_template();
    for (index, current) in targets.iter().enumerate() {
        if runtime.cancel.load(Ordering::Relaxed) {
            done.cancelled = true;
            break;
        }
        // Without the journal nothing may be written (§9.5); listed once, as a failed start.
        let Some(journal) = runtime.write.journal.as_ref() else {
            done.iptc_errors
                .push(format!("{}: {NO_JOURNAL_TEXT}", file_name(current)));
            break;
        };
        if runtime.exiftool.is_none() {
            let started = match runtime.exiftool_program {
                Some(program) => ExifTool::start_with(program, &[]),
                None => ExifTool::start(),
            };
            match started {
                Ok(tool) => *runtime.exiftool = Some(tool),
                Err(error) => {
                    done.iptc_errors
                        .push(format!("{}: {error}", file_name(current)));
                    break;
                }
            }
        }
        let Some(tool) = runtime.exiftool.as_mut() else {
            done.iptc_errors.push(format!(
                "{}: ExifTool worker is unavailable",
                file_name(current)
            ));
            break;
        };
        let _ = runtime.events.send(ImportEvent::IptcProgress {
            id: runtime.id,
            done: index,
            total: targets.len(),
            current: current.clone(),
        });
        // §9.5: the template is written to the verified copy with the safe write.
        let written = tool.apply_template_safely(
            current,
            &template,
            &EditOptions::default(),
            &*runtime.write.checks,
            journal,
        );
        if let Err(error) = written {
            done.iptc_errors
                .push(format!("{}: {error}", file_name(current)));
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_metadata::safe_write::NoImageCheck;
    use std::fs;

    /// The small baseline JPEG of `pw-thumbcache`, shared as `pw-imagecheck` and `pw-libjpeg` do.
    #[cfg(windows)]
    #[path = "../../../../pw-thumbcache/src/jpeg_fixture.rs"]
    mod jpeg_fixture;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDirs {
        root: PathBuf,
        source: PathBuf,
        dest: PathBuf,
    }

    impl TempDirs {
        fn new(tag: &str) -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "pw-loupe-import-worker-{tag}-{}-{id}",
                std::process::id()
            ));
            let source = root.join("source");
            let dest = root.join("destination");
            fs::create_dir_all(&source)
                .unwrap_or_else(|error| panic!("could not create {}: {error}", source.display()));
            Self { root, source, dest }
        }

        fn write_image(&self, name: &str, bytes: &[u8]) {
            let folder = self.source.join("DCIM").join("100TEST");
            fs::create_dir_all(&folder)
                .unwrap_or_else(|error| panic!("could not create {}: {error}", folder.display()));
            fs::write(folder.join(name), bytes)
                .unwrap_or_else(|error| panic!("could not write image: {error}"));
        }

        fn options(&self, folder: &str) -> pw_ingest::Options {
            pw_ingest::Options {
                dest: self.dest.clone(),
                backup: None,
                folder: pw_ingest::Template::parse(folder).unwrap(),
                name: pw_ingest::Template::parse("{name}").unwrap(),
                job: "Test".into(),
                first_seq: 1,
            }
        }

        fn form(&self, folder: &str) -> ImportForm {
            ImportForm {
                source: self.source.to_string_lossy().into_owned(),
                dest: self.dest.to_string_lossy().into_owned(),
                folder: folder.to_string(),
                name: "{name}".to_string(),
                ..ImportForm::default()
            }
        }
    }

    impl Drop for TempDirs {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// A temporary write journal (T91), shared by the tests of this process.
    fn test_journal() -> Journal {
        let folder =
            std::env::temp_dir().join(format!("pw-loupe-import-journal-{}", std::process::id()));
        Journal::open(&folder)
            .unwrap_or_else(|error| panic!("could not open {}: {error}", folder.display()))
    }

    /// The files in `folder` and below, as paths relative to it.
    fn files_below(folder: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut folders = vec![folder.to_path_buf()];
        while let Some(current) = folders.pop() {
            for entry in fs::read_dir(&current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else {
                    files.push(path.strip_prefix(folder).unwrap().to_path_buf());
                }
            }
        }
        files.sort();
        files
    }

    fn next_event(worker: &ImportWorker) -> ImportEvent {
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

    fn done_event(worker: &ImportWorker, id: RequestId) -> ImportDone {
        loop {
            match next_event(worker) {
                ImportEvent::Progress { id: event_id, .. } => assert_eq!(event_id, id),
                ImportEvent::IptcProgress { id: event_id, .. } => assert_eq!(event_id, id),
                ImportEvent::Ejecting { id: event_id } => {
                    panic!("unexpected ejection event for request {event_id}")
                }
                ImportEvent::Done { id: event_id, done } => {
                    assert_eq!(event_id, id);
                    return done;
                }
                ImportEvent::Previewed { .. } => panic!("unexpected preview event"),
                ImportEvent::Cards { .. } => panic!("unexpected cards event"),
            }
        }
    }

    #[test]
    fn preview_reports_counts_and_rescan_controls_cache_reuse() {
        let dirs = TempDirs::new("preview-cache");
        dirs.write_image("A001.ARW", b"first image");
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();

        let id = worker.preview(dirs.form(""), false).unwrap();
        match next_event(&worker) {
            ImportEvent::Previewed {
                id: event_id,
                result: Ok(preview),
            } => {
                assert_eq!(event_id, id);
                assert_eq!((preview.shots, preview.files_to_copy), (1, 1));
            }
            event => panic!("unexpected preview result: {event:?}"),
        }

        dirs.write_image("A002.ARW", b"second image");
        let cached_id = worker.preview(dirs.form(""), false).unwrap();
        match next_event(&worker) {
            ImportEvent::Previewed {
                id,
                result: Ok(preview),
            } => {
                assert_eq!(id, cached_id);
                assert_eq!(preview.shots, 1);
            }
            event => panic!("unexpected cached preview: {event:?}"),
        }

        let rescan_id = worker.preview(dirs.form(""), true).unwrap();
        match next_event(&worker) {
            ImportEvent::Previewed {
                id,
                result: Ok(preview),
            } => {
                assert_eq!(id, rescan_id);
                assert_eq!(preview.shots, 2);
            }
            event => panic!("unexpected rescanned preview: {event:?}"),
        }
    }

    #[test]
    fn run_copies_reports_progress_and_summarizes_destination_folders() {
        let dirs = TempDirs::new("run");
        dirs.write_image("A001.ARW", b"image bytes");
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        let options = dirs.options("imported");

        let id = worker.run(dirs.form("imported")).unwrap();
        let mut progress_count = 0;
        let done = loop {
            match next_event(&worker) {
                ImportEvent::Progress {
                    id: event_id,
                    progress,
                } => {
                    assert_eq!(event_id, id);
                    assert_eq!(progress.files_total, 1);
                    progress_count += 1;
                }
                ImportEvent::IptcProgress { .. } => {
                    panic!("unexpected IPTC progress for an empty template")
                }
                ImportEvent::Ejecting { id: event_id } => {
                    panic!("unexpected ejection event for request {event_id}")
                }
                ImportEvent::Done { id: event_id, done } => {
                    assert_eq!(event_id, id);
                    break done;
                }
                ImportEvent::Previewed { .. } => panic!("unexpected preview event"),
                ImportEvent::Cards { .. } => panic!("unexpected cards event"),
            }
        };

        assert_eq!(progress_count, 1);
        assert_eq!((done.copied, done.present, done.bytes), (1, 0, 11));
        assert!(done.failed.is_empty());
        assert!(!done.cancelled);
        assert!(done.eject.is_none());
        assert!(done.error.is_none());
        assert_eq!(
            done.folders,
            vec![(
                options.dest.join("imported").to_string_lossy().into_owned(),
                1
            )]
        );
        assert_eq!(
            fs::read(options.dest.join("imported").join("A001.ARW")).unwrap(),
            b"image bytes"
        );
    }

    #[test]
    fn subsequent_run_reports_files_already_present() {
        let dirs = TempDirs::new("present");
        dirs.write_image("A001.ARW", b"image bytes");
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();

        let first = worker.run(dirs.form("imported")).unwrap();
        let first_done = done_event(&worker, first);
        assert_eq!(first_done.copied, 1);

        let second = worker.run(dirs.form("imported")).unwrap();
        let second_done = done_event(&worker, second);
        assert_eq!((second_done.copied, second_done.present), (0, 1));
    }

    #[test]
    fn missing_source_returns_validation_error() {
        let dirs = TempDirs::new("missing");
        let missing = dirs.root.join("missing");
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        let mut form = dirs.form("");
        form.source = missing.to_string_lossy().into_owned();
        let id = worker.preview(form, true).unwrap();

        match next_event(&worker) {
            ImportEvent::Previewed {
                id: event_id,
                result: Err(error),
            } => {
                assert_eq!(event_id, id);
                assert_eq!(error.field, Field::Source);
                assert_eq!(
                    error.message,
                    format!("Källan {} finns inte.", missing.display())
                );
            }
            event => panic!("unexpected missing-source event: {event:?}"),
        }
    }

    #[test]
    fn invalid_form_returns_destination_error_from_preview() {
        let dirs = TempDirs::new("invalid-preview");
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        let mut form = dirs.form("");
        form.dest.clear();
        let id = worker.preview(form, true).unwrap();

        match next_event(&worker) {
            ImportEvent::Previewed {
                id: event_id,
                result: Err(error),
            } => {
                assert_eq!(event_id, id);
                assert_eq!(error.field, Field::Dest);
                assert_eq!(error.message, "Välj ett mål.");
            }
            event => panic!("unexpected invalid preview event: {event:?}"),
        }
    }

    #[test]
    fn invalid_run_finishes_with_validation_error_without_copying() {
        let dirs = TempDirs::new("invalid-run");
        dirs.write_image("A001.ARW", b"image bytes");
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        let mut form = dirs.form("");
        form.dest.clear();
        let id = worker.run(form).unwrap();

        let done = done_event(&worker, id);
        assert_eq!(done.error.as_deref(), Some("Välj ett mål."));
        assert_eq!(done.copied, 0);
        assert!(!dirs.dest.exists());
    }

    #[test]
    fn empty_iptc_template_does_not_start_exiftool() {
        let dirs = TempDirs::new("empty-iptc");
        dirs.write_image("A001.ARW", b"image bytes");
        let missing_exiftool = dirs
            .root
            .join("missing-exiftool")
            .to_string_lossy()
            .into_owned();
        let mut worker = ImportWorker::start_with_exiftool(
            &missing_exiftool,
            Some(test_journal()),
            Box::new(NoImageCheck),
        )
        .unwrap();
        let id = worker.run(dirs.form("")).unwrap();

        let done = done_event(&worker, id);
        assert_eq!(done.copied, 1);
        assert!(done.iptc_errors.is_empty());
        assert!(dirs.dest.join("A001.ARW").exists());
    }

    #[test]
    fn unavailable_exiftool_reports_one_iptc_error_after_copying() {
        let dirs = TempDirs::new("iptc-start-error");
        dirs.write_image("A001.ARW", b"image bytes");
        let missing_exiftool = dirs
            .root
            .join("missing-exiftool")
            .to_string_lossy()
            .into_owned();
        let mut worker = ImportWorker::start_with_exiftool(
            &missing_exiftool,
            Some(test_journal()),
            Box::new(NoImageCheck),
        )
        .unwrap();
        let mut form = dirs.form("");
        form.iptc.creator = "Fotograf".to_string();
        let id = worker.run(form).unwrap();

        let done = done_event(&worker, id);
        assert_eq!(done.copied, 1);
        assert!(done.failed.is_empty());
        assert_eq!(done.iptc_errors.len(), 1);
        assert!(
            done.iptc_errors[0].starts_with("A001.ARW: ExifTool: could not start "),
            "{:?}",
            done.iptc_errors
        );
        assert!(dirs.dest.join("A001.ARW").exists());
    }

    #[test]
    fn without_a_journal_the_template_is_not_written() {
        let dirs = TempDirs::new("iptc-no-journal");
        dirs.write_image("A001.jpg", b"image bytes");
        let missing_exiftool = dirs
            .root
            .join("missing-exiftool")
            .to_string_lossy()
            .into_owned();
        let mut worker =
            ImportWorker::start_with_exiftool(&missing_exiftool, None, Box::new(NoImageCheck))
                .unwrap();
        let mut form = dirs.form("");
        form.iptc.creator = "Fotograf".to_string();
        let id = worker.run(form).unwrap();

        let done = done_event(&worker, id);
        assert_eq!(done.copied, 1);
        assert_eq!(
            done.iptc_errors,
            ["A001.jpg: skrivloggen kunde inte öppnas"]
        );
        assert_eq!(
            fs::read(dirs.dest.join("A001.jpg")).unwrap(),
            b"image bytes"
        );
        assert_eq!(files_below(&dirs.dest), [PathBuf::from("A001.jpg")]);
    }

    /// Needs ExifTool on PATH (the import worker starts `exiftool`); fails with an explanation
    /// when it is missing (docs/TESTING.md §4.5). The safe write uses Windows file operations.
    #[cfg(windows)]
    #[test]
    fn the_template_is_written_safely_to_an_imported_jpeg() {
        let mut exiftool = ExifTool::start()
            .and_then(|mut tool| tool.execute(&["-ver".to_string()]).map(|_| tool))
            .unwrap_or_else(|error| {
                panic!(
                    "ExifTool is required for this test but `exiftool` on PATH does not answer: \
                     {error}. Install ExifTool (docs/TESTING.md §4.5)."
                )
            });
        let dirs = TempDirs::new("iptc-safe-jpeg");
        dirs.write_image("A001.jpg", jpeg_fixture::SMALL);
        let journal_folder = dirs.root.join("journal");
        let journal = Journal::open(&journal_folder).unwrap();
        let mut worker = ImportWorker::start(Some(journal), Box::new(NoImageCheck)).unwrap();
        let mut form = dirs.form("");
        form.iptc.creator = "Fotograf".to_string();
        let id = worker.run(form).unwrap();

        let done = done_event(&worker, id);
        assert_eq!(done.copied, 1);
        assert!(done.iptc_errors.is_empty(), "{:?}", done.iptc_errors);
        let image = dirs.dest.join("A001.jpg");
        let read = exiftool
            .read_metadata(&image, pw_metadata::iim::Fallback::Windows1252)
            .unwrap();
        assert_eq!(
            read.reconciled
                .metadata
                .get("dc:creator")
                .map(|stored| stored.value.clone()),
            Some(pw_metadata::value::Value::List(vec![
                "Fotograf".to_string()
            ]))
        );
        let files = files_below(&dirs.dest);
        assert_eq!(files, [PathBuf::from("A001.jpg")]);
        assert!(
            !files
                .iter()
                .any(|file| file.to_string_lossy().contains("pwtmp")),
            "{files:?}"
        );
        assert_eq!(files_below(&journal_folder), Vec::<PathBuf>::new());
    }

    #[test]
    fn iptc_targets_images_in_destination_and_backup_but_not_companions_or_failures() {
        let dirs = TempDirs::new("iptc-targets");
        let mut options = dirs.options("");
        let backup = dirs.root.join("backup");
        options.backup = Some(backup.clone());
        let copied_source = dirs.source.join("A001.ARW");
        let companion_source = dirs.source.join("A001.xmp");
        let failed_source = dirs.source.join("A002.ARW");
        let plan = pw_ingest::Plan {
            items: vec![
                pw_ingest::PlanItem {
                    source: copied_source.clone(),
                    size: 10,
                    modified: std::time::SystemTime::UNIX_EPOCH,
                    rel: PathBuf::from("A001.ARW"),
                    to_dest: true,
                    to_backup: true,
                    renamed_for_conflict: false,
                },
                pw_ingest::PlanItem {
                    source: companion_source,
                    size: 10,
                    modified: std::time::SystemTime::UNIX_EPOCH,
                    rel: PathBuf::from("A001.xmp"),
                    to_dest: true,
                    to_backup: true,
                    renamed_for_conflict: false,
                },
                pw_ingest::PlanItem {
                    source: failed_source.clone(),
                    size: 10,
                    modified: std::time::SystemTime::UNIX_EPOCH,
                    rel: PathBuf::from("A002.ARW"),
                    to_dest: true,
                    to_backup: false,
                    renamed_for_conflict: false,
                },
            ],
        };

        assert_eq!(
            iptc_targets(
                &plan,
                &[(failed_source, "copy failed".to_string())],
                &options
            ),
            vec![options.dest.join("A001.ARW"), backup.join("A001.ARW")]
        );
    }

    #[test]
    fn cancellation_before_iptc_writes_nothing() {
        let dirs = TempDirs::new("iptc-cancel");
        dirs.write_image("A001.ARW", b"image bytes");
        let options = dirs.options("");
        let image = options.dest.join("A001.ARW");
        fs::create_dir_all(&options.dest).unwrap();
        fs::write(&image, b"image bytes").unwrap();
        let plan = pw_ingest::Plan {
            items: vec![pw_ingest::PlanItem {
                source: dirs.source.join("DCIM").join("100TEST").join("A001.ARW"),
                size: 11,
                modified: std::time::SystemTime::UNIX_EPOCH,
                rel: PathBuf::from("A001.ARW"),
                to_dest: true,
                to_backup: false,
                renamed_for_conflict: false,
            }],
        };
        let cancel = AtomicBool::new(true);
        let mut exiftool = None;
        let (events, _receiver) = mpsc::channel();
        let write = WriteContext {
            exiftool_program: None,
            journal: Some(test_journal()),
            checks: Box::new(NoImageCheck),
        };
        let mut runtime = IptcRuntime {
            id: 1,
            cancel: &cancel,
            events: &events,
            exiftool: &mut exiftool,
            exiftool_program: Some("missing-exiftool"),
            write: &write,
        };
        let mut done = ImportDone {
            copied: 1,
            ..ImportDone::default()
        };

        apply_iptc(
            &plan,
            &[],
            &options,
            &crate::IptcForm {
                creator: "Fotograf".to_string(),
                ..crate::IptcForm::default()
            }
            .to_iptc(),
            &mut runtime,
            &mut done,
        );

        assert!(done.cancelled);
        assert!(done.iptc_errors.is_empty());
        assert!(exiftool.is_none());
        assert!(image.exists());
        assert!(!image.with_extension("xmp").exists());
    }

    #[test]
    fn cancelled_run_with_eject_requested_does_not_attempt_ejection() {
        let dirs = TempDirs::new("eject-cancelled");
        dirs.write_image("A001.ARW", b"image bytes");
        let mut form = dirs.form("");
        form.eject = true;
        let cancel = AtomicBool::new(true);
        let (events, receiver) = mpsc::channel();
        let mut cached_scan = None;
        let mut exiftool = None;

        let ImportEvent::Done { done, .. } = run_import(
            7,
            &form,
            &cancel,
            &mut cached_scan,
            &events,
            &mut exiftool,
            &WriteContext {
                exiftool_program: None,
                journal: Some(test_journal()),
                checks: Box::new(NoImageCheck),
            },
        ) else {
            panic!("run_import did not return Done");
        };

        assert!(done.cancelled);
        assert_eq!(
            done.eject.as_deref(),
            Some("Kortet matades inte ut eftersom importen inte blev komplett.")
        );
        assert!(
            receiver
                .try_iter()
                .all(|event| !matches!(event, ImportEvent::Ejecting { .. }))
        );
    }

    #[test]
    fn failed_run_with_eject_requested_does_not_attempt_ejection() {
        let dirs = TempDirs::new("eject-failed");
        dirs.write_image("A001.ARW", b"image bytes");
        fs::write(&dirs.dest, b"not a directory").unwrap();
        let mut form = dirs.form("");
        form.eject = true;
        let cancel = AtomicBool::new(false);
        let (events, receiver) = mpsc::channel();
        let mut cached_scan = None;
        let mut exiftool = None;

        let ImportEvent::Done { done, .. } = run_import(
            8,
            &form,
            &cancel,
            &mut cached_scan,
            &events,
            &mut exiftool,
            &WriteContext {
                exiftool_program: None,
                journal: Some(test_journal()),
                checks: Box::new(NoImageCheck),
            },
        ) else {
            panic!("run_import did not return Done");
        };

        assert!(!done.failed.is_empty());
        assert_eq!(
            done.eject.as_deref(),
            Some("Kortet matades inte ut eftersom importen inte blev komplett.")
        );
        assert!(
            receiver
                .try_iter()
                .all(|event| !matches!(event, ImportEvent::Ejecting { .. }))
        );
    }

    #[test]
    fn cancel_during_many_file_run_leaves_no_partial_named_files() {
        let dirs = TempDirs::new("cancel");
        let bytes = vec![7; 1024 * 1024];
        for index in 0..96 {
            dirs.write_image(&format!("A{index:03}.ARW"), &bytes);
        }
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        let options = dirs.options("");
        let id = worker.run(dirs.form("")).unwrap();
        worker.cancel();

        let done = done_event(&worker, id);
        assert!(done.cancelled);
        assert!(done.error.is_none());
        let mut stack = vec![options.dest.clone()];
        while let Some(folder) = stack.pop() {
            if !folder.exists() {
                continue;
            }
            for entry in fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    assert!(
                        !path.to_string_lossy().ends_with(".pwpart"),
                        "partial copy remains: {}",
                        path.display()
                    );
                    assert_eq!(
                        fs::metadata(&path).unwrap().len(),
                        bytes.len() as u64,
                        "a final-named file must be complete"
                    );
                }
            }
        }
    }

    #[test]
    fn cards_answers_with_its_id() {
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        let id = worker.cards().unwrap();
        match next_event(&worker) {
            ImportEvent::Cards { id: event_id, .. } => assert_eq!(event_id, id),
            event => panic!("unexpected cards event: {event:?}"),
        }
    }

    #[test]
    fn dropping_worker_does_not_wait_for_a_copy() {
        let dirs = TempDirs::new("drop");
        dirs.write_image("A001.ARW", &vec![3; 16 * 1024 * 1024]);
        let mut worker = ImportWorker::start(Some(test_journal()), Box::new(NoImageCheck)).unwrap();
        worker.run(dirs.form("")).unwrap();

        let started = Instant::now();
        drop(worker);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "dropping the worker waited for the background operation"
        );
    }
}
