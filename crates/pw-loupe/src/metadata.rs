use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use pw_loupe::{
    RatingKey, RatingRefused, RatingStore, RatingWriter, UNSUPPORTED_TEXT, status_text,
};
use pw_metadata::safe_write::OriginalState;
use pw_metadata::{
    MetadataWorker, NO_JOURNAL, Rating, RequestId, SubmitError, WorkerEvent, WriteError,
    supports_rating,
};

pub(crate) trait MetadataSubmit: RatingWriter {
    fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError>;

    fn try_recv(&mut self) -> Result<WorkerEvent, TryRecvError>;
}

impl MetadataSubmit for MetadataWorker {
    fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError> {
        MetadataWorker::read_ratings(self, images)
    }

    fn try_recv(&mut self) -> Result<WorkerEvent, TryRecvError> {
        MetadataWorker::try_recv(self)
    }
}

#[derive(Clone, Debug)]
enum LoadState {
    Loading,
    Ready,
    Unsupported,
    Error(String),
}

#[derive(Clone, Debug)]
struct ImageMetadata {
    state: LoadState,
}

#[derive(Default)]
pub(crate) struct MetadataCache {
    images: HashMap<PathBuf, ImageMetadata>,
    load_request: Option<RequestId>,
    pending_read: Option<Vec<PathBuf>>,
    /// Why the last folder read failed (ADR 0014 Q7); cleared by the next `begin_load`.
    read_error: Option<String>,
    ratings: RatingStore,
}

impl MetadataCache {
    /// Keys metadata by absolute path so it matches the paths sent to the view core.
    pub fn begin_load(
        &mut self,
        files: Vec<PathBuf>,
        worker: &mut impl MetadataSubmit,
    ) -> Result<Vec<PathBuf>, String> {
        let cwd =
            std::env::current_dir().map_err(|e| format!("cannot resolve image paths: {e}"))?;
        let files: Vec<PathBuf> = files
            .into_iter()
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                }
            })
            .collect();
        self.images = files
            .iter()
            .map(|path| {
                let state = if supports_rating(path) {
                    LoadState::Loading
                } else {
                    LoadState::Unsupported
                };
                (path.clone(), ImageMetadata { state })
            })
            .collect();
        self.load_request = None;
        self.pending_read = None;
        self.read_error = None;
        let supported: Vec<PathBuf> = files
            .iter()
            .filter(|path| supports_rating(path))
            .cloned()
            .collect();
        if !supported.is_empty() {
            match worker.read_ratings(supported.clone()) {
                Ok(id) => self.load_request = Some(id),
                Err(SubmitError::QueueFull) => self.pending_read = Some(supported),
                Err(error) => self.fail_read(&submit_error(error)),
            }
        }
        Ok(files)
    }

    pub fn poll(&mut self, worker: &mut impl MetadataSubmit) -> Vec<String> {
        let mut failures = Vec::new();
        self.drain(|| worker.try_recv(), &mut failures);
        self.retry_pending_read(worker);
        failures.extend(self.ratings.pump(worker));
        failures
    }

    fn retry_pending_read(&mut self, worker: &mut impl MetadataSubmit) {
        let Some(files) = self.pending_read.clone() else {
            return;
        };
        match worker.read_ratings(files) {
            Ok(id) => {
                self.load_request = Some(id);
                self.pending_read = None;
            }
            Err(SubmitError::QueueFull) => {}
            Err(error) => self.fail_read(&submit_error(error)),
        }
    }

    pub fn prune(&mut self, keep: impl Fn(&Path) -> bool) {
        self.ratings.prune(keep);
    }

    /// Applies every available event. If the worker is gone, no outstanding operation can
    /// finish, so loading entries and writes become errors instead of remaining pending.
    fn drain(
        &mut self,
        mut next: impl FnMut() -> Result<WorkerEvent, TryRecvError>,
        failures: &mut Vec<String>,
    ) {
        loop {
            match next() {
                Ok(event) => self.apply(event, failures),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    let message = submit_error(SubmitError::Stopped);
                    if self.ratings_ready() {
                        self.fail_loading(&message);
                    } else {
                        // The folder read was sent or waiting to be sent: it can never finish.
                        self.fail_read(&message);
                    }
                    failures.extend(self.ratings.fail_sent());
                    return;
                }
            }
        }
    }

    /// The folder read failed: remembers why (Q7), then fails every entry that is still loading.
    fn fail_read(&mut self, message: &str) {
        self.read_error = Some(message.to_owned());
        self.fail_loading(message);
    }

    /// Moves every entry that is still loading to the error state.
    fn fail_loading(&mut self, message: &str) {
        self.load_request = None;
        self.pending_read = None;
        for metadata in self.images.values_mut() {
            if matches!(&metadata.state, LoadState::Loading) {
                metadata.state = LoadState::Error(message.to_owned());
            }
        }
    }

    /// Whether the ratings of the loaded folder are read: false while the folder read is sent or
    /// waiting to be sent (ADR 0014 Q7), true otherwise. A failed read also ends the wait, so this
    /// is true then as well: the caller of `Tab::refilter` must treat a `read_error` as
    /// `ratings_ready == false`, so an active filter shows nothing.
    pub fn ratings_ready(&self) -> bool {
        self.load_request.is_none() && self.pending_read.is_none()
    }

    /// Why the folder read failed, if it did (ADR 0014 Q7: an active filter then shows nothing and
    /// the status line gives this error). Treat it as `ratings_ready == false` when calling
    /// `Tab::refilter`.
    pub fn read_error(&self) -> Option<String> {
        self.read_error.clone()
    }

    pub fn notice(&self, path: Option<&Path>) -> Option<String> {
        let metadata = self.images.get(path?)?;
        match &metadata.state {
            LoadState::Loading => Some("Läser metadata…".into()),
            LoadState::Ready => None,
            LoadState::Unsupported => Some(UNSUPPORTED_TEXT.into()),
            LoadState::Error(error) => Some(format!("Metadatafel: {error}")),
        }
    }

    pub fn is_loading(&self, path: Option<&Path>) -> bool {
        path.and_then(|path| self.images.get(path))
            .is_some_and(|metadata| matches!(&metadata.state, LoadState::Loading))
    }

    pub fn is_error(&self, path: Option<&Path>) -> bool {
        path.and_then(|path| self.images.get(path))
            .is_some_and(|metadata| matches!(&metadata.state, LoadState::Error(_)))
    }

    pub fn rating_summary(&self, path: Option<&Path>) -> Option<String> {
        let path = path?;
        self.images.get(path)?;
        self.ratings
            .contains(path)
            .then(|| status_text(self.ratings.shown(path)))
    }

    // Since T56 the program rates through `set_ratings` (one path equals `set`); the tests still
    // use this one.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_rating(&mut self, path: &Path, key: RatingKey) -> Result<(), RatingRefused> {
        self.ratings.set(path, key)
    }

    /// One rating key on several images (ADR 0014 §2.7); see `RatingStore::set_many`.
    pub fn set_ratings(
        &mut self,
        paths: &[PathBuf],
        key: RatingKey,
    ) -> Result<usize, RatingRefused> {
        self.ratings.set_many(paths, key)
    }

    /// The status-line text for the write failures since the last rating key (ADR 0014 Q6).
    pub fn failure_text(&self) -> Option<String> {
        self.ratings.failure_text()
    }

    pub fn shown_rating(&self, path: &Path) -> Option<&Rating> {
        self.images.get(path)?;
        self.ratings.shown(path)
    }

    /// Rating writes that are queued or sent but not yet confirmed.
    pub fn pending_writes(&self) -> usize {
        self.ratings.pending()
    }

    fn apply(&mut self, event: WorkerEvent, failures: &mut Vec<String>) {
        match event {
            WorkerEvent::RatingsRead { id, result } if Some(id) == self.load_request => {
                self.load_request = None;
                match result {
                    Ok(ratings) => {
                        let paths: Vec<PathBuf> = self
                            .images
                            .iter()
                            .filter(|(_, metadata)| matches!(&metadata.state, LoadState::Loading))
                            .map(|(path, _)| path.clone())
                            .collect();
                        self.ratings.read(id, &ratings, &paths);
                        for metadata in self.images.values_mut() {
                            if matches!(&metadata.state, LoadState::Loading) {
                                metadata.state = LoadState::Ready;
                            }
                        }
                    }
                    Err(error) => self.fail_read(&error),
                }
            }
            WorkerEvent::RatingWritten {
                id, path, result, ..
            } => {
                if let Some(message) =
                    self.ratings
                        .written(id, &path, result.map_err(write_error_text))
                {
                    failures.push(message);
                }
            }
            WorkerEvent::RatingsRead { .. } => {}
        }
    }
}

/// The longest wait for rating writes when the program exits (ADR 0013 Q6).
pub(crate) const EXIT_WAIT_LIMIT: Duration = Duration::from_secs(5);
/// The pause between two polls while waiting at exit.
pub(crate) const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The most lines listed in the exit message box before "… och {n} till".
const UNSAVED_LIST_LIMIT: usize = 20;
/// Title of the message box listing ratings that were not saved at exit.
pub(crate) const UNSAVED_TITLE: &str = "Betyg som inte sparades";

/// What is left after waiting for the rating writes at exit.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct UnsavedRatings {
    /// Messages for writes that failed during the wait; each names its file.
    pub failed: Vec<String>,
    /// File names of writes still pending when the wait ended.
    pub pending: Vec<String>,
}

impl UnsavedRatings {
    pub fn is_empty(&self) -> bool {
        self.failed.is_empty() && self.pending.is_empty()
    }

    /// The lines of the message box: the failures first, then the files still pending.
    pub fn lines(&self) -> Vec<String> {
        self.failed.iter().chain(&self.pending).cloned().collect()
    }
}

/// Keeps sending and confirming rating writes until none is pending or `EXIT_WAIT_LIMIT` has
/// passed. The clock and the sleep are passed in so the wait can be tested without real time.
pub(crate) fn finish_pending_writes(
    cache: &mut MetadataCache,
    worker: &mut impl MetadataSubmit,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> UnsavedRatings {
    let mut unsaved = UnsavedRatings::default();
    if cache.pending_writes() == 0 {
        return unsaved;
    }
    let start = now();
    loop {
        unsaved.failed.extend(cache.poll(worker));
        if cache.pending_writes() == 0 || now().saturating_duration_since(start) >= EXIT_WAIT_LIMIT
        {
            break;
        }
        sleep(EXIT_POLL_INTERVAL);
    }
    unsaved.pending = cache
        .ratings
        .pending_paths()
        .iter()
        .map(|path| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    unsaved
}

/// The text of the exit message box: at most `UNSAVED_LIST_LIMIT` lines, then how many more.
pub(crate) fn unsaved_list_text(lines: &[String]) -> String {
    let mut text = lines
        .iter()
        .take(UNSAVED_LIST_LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let more = lines.len().saturating_sub(UNSAVED_LIST_LIMIT);
    if more > 0 {
        let _ = write!(text, "\n… och {more} till");
    }
    text
}

/// Shown after "Kunde inte spara betyget för <fil>: " when the file is read-only (T49).
const READ_ONLY_TEXT: &str = "filen är skrivskyddad";
/// Shown after "Kunde inte spara betyget för <fil>: " when ExifTool refused the write (T49).
const NOT_WRITTEN_TEXT: &str = "filen kunde inte skrivas";

/// Shown after "Kunde inte spara betyget för <fil>: " when the write journal could not be opened
/// at the start, so nothing is written (T91).
pub(crate) const NO_JOURNAL_TEXT: &str = "skrivloggen kunde inte öppnas";

/// The reason in the message for a failed rating write. Other errors keep their own text.
fn write_error_text(error: WriteError) -> String {
    match error {
        WriteError::ReadOnly => READ_ONLY_TEXT.into(),
        WriteError::NotWritten(_) => NOT_WRITTEN_TEXT.into(),
        WriteError::Other(text) if text == NO_JOURNAL => NO_JOURNAL_TEXT.into(),
        WriteError::Other(text) => text,
        WriteError::NotSafe { original, .. } => match original {
            OriginalState::Intact => {
                "metadata kunde inte skrivas säkert, filen är oförändrad".into()
            }
            OriginalState::AtBackup(backup) => {
                let name = backup.file_name().map_or_else(
                    || backup.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                format!("metadata kunde inte skrivas säkert, originalet ligger nu som {name}")
            }
            OriginalState::Replaced => "filen skrevs men slutkontrollen misslyckades, den \
                                        tidigare versionen finns kvar som säkerhetskopia"
                .into(),
        },
    }
}

fn submit_error(error: SubmitError) -> String {
    match error {
        SubmitError::QueueFull => "metadatakön är full".into(),
        SubmitError::Stopped => "metadataarbetaren har stannat".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_metadata::Rating;
    use pw_metadata::WriteSetup;
    use pw_metadata::edit::EditOptions;
    use pw_metadata::journal::Journal;
    use pw_metadata::safe_write::{NoImageCheck, Stage};
    use std::collections::{HashMap, VecDeque};

    /// A write setup with `NoImageCheck` and a temporary journal (T91); these tests never write.
    fn test_setup() -> WriteSetup {
        let folder =
            std::env::temp_dir().join(format!("pw-loupe-metadata-journal-{}", std::process::id()));
        let journal = Journal::open(&folder)
            .unwrap_or_else(|error| panic!("could not open {}: {error}", folder.display()));
        WriteSetup {
            journal,
            checks: Box::new(NoImageCheck),
            options: EditOptions::default(),
        }
    }

    struct FakeSubmit {
        read_responses: VecDeque<Result<RequestId, SubmitError>>,
        read_attempts: Vec<Vec<PathBuf>>,
        write_attempts: Vec<PathBuf>,
        operations: Vec<&'static str>,
        events: VecDeque<WorkerEvent>,
        next_write_id: RequestId,
        /// When set, every accepted write is answered with this result on the next `try_recv`.
        write_result: Option<Result<(), String>>,
    }

    impl FakeSubmit {
        fn new(read_responses: impl IntoIterator<Item = Result<RequestId, SubmitError>>) -> Self {
            Self {
                read_responses: read_responses.into_iter().collect(),
                read_attempts: Vec::new(),
                write_attempts: Vec::new(),
                operations: Vec::new(),
                events: VecDeque::new(),
                next_write_id: 100,
                write_result: None,
            }
        }
    }

    impl RatingWriter for FakeSubmit {
        fn write_rating(
            &mut self,
            path: PathBuf,
            rating: Rating,
        ) -> Result<RequestId, SubmitError> {
            self.operations.push("write");
            self.write_attempts.push(path.clone());
            let id = self.next_write_id;
            self.next_write_id += 1;
            if let Some(result) = self.write_result.clone() {
                self.events.push_back(WorkerEvent::RatingWritten {
                    id,
                    path,
                    rating,
                    result: result.map_err(WriteError::Other),
                });
            }
            Ok(id)
        }
    }

    impl MetadataSubmit for FakeSubmit {
        fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError> {
            self.operations.push("read");
            self.read_attempts.push(images);
            self.read_responses
                .pop_front()
                .unwrap_or(Err(SubmitError::QueueFull))
        }

        fn try_recv(&mut self) -> Result<WorkerEvent, TryRecvError> {
            self.events.pop_front().ok_or(TryRecvError::Empty)
        }
    }

    #[test]
    fn metadata_cache_uses_absolute_paths_and_starts_loading() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-missing-exiftool-{}", std::process::id()));
        let mut worker =
            match MetadataWorker::start_with(&program.to_string_lossy(), &[], Some(test_setup())) {
                Ok(worker) => worker,
                Err(error) => panic!("worker thread should start: {error}"),
            };
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        assert!(files[0].is_absolute());
        assert!(cache.is_loading(Some(&files[0])));
        assert!(!cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Läser metadata…")
        );
    }

    #[test]
    fn metadata_state_queries_are_false_for_unsupported_files_and_missing_paths() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-unsupported-{}", std::process::id()));
        let mut worker =
            match MetadataWorker::start_with(&program.to_string_lossy(), &[], Some(test_setup())) {
                Ok(worker) => worker,
                Err(error) => panic!("worker thread should start: {error}"),
            };
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("notes.txt")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };

        assert!(!cache.is_loading(Some(&files[0])));
        assert!(!cache.is_error(Some(&files[0])));
        assert!(!cache.is_loading(None));
        assert!(!cache.is_error(None));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some(UNSUPPORTED_TEXT)
        );
    }

    #[test]
    fn disconnected_worker_turns_loading_into_a_swedish_error() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-stopped-worker-{}", std::process::id()));
        let mut worker =
            match MetadataWorker::start_with(&program.to_string_lossy(), &[], Some(test_setup())) {
                Ok(worker) => worker,
                Err(error) => panic!("worker thread should start: {error}"),
            };
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(
            vec![PathBuf::from("photo.ARW"), PathBuf::from("notes.txt")],
            &mut worker,
        ) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        let mut failures = Vec::new();
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert!(failures.is_empty());
        assert!(!cache.is_loading(Some(&files[0])));
        assert!(cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Metadatafel: metadataarbetaren har stannat")
        );
        assert_eq!(
            cache.notice(Some(&files[1])).as_deref(),
            Some(UNSUPPORTED_TEXT),
            "entries that were not loading keep their state"
        );
    }

    #[test]
    fn ratings_survive_loading_another_folder() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-preserved-rating-{}", std::process::id()));
        let mut worker =
            match MetadataWorker::start_with(&program.to_string_lossy(), &[], Some(test_setup())) {
                Ok(worker) => worker,
                Err(error) => panic!("worker thread should start: {error}"),
            };
        let mut cache = MetadataCache::default();
        let old_path = std::env::temp_dir().join("pw-loupe-preserved.ARW");
        if let Err(error) = cache.begin_load(vec![old_path.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        if let Err(error) = cache.ratings.set(&old_path, pw_loupe::RatingKey::Stars(3)) {
            panic!("supported rating path should be accepted: {error}");
        }
        if let Err(error) = cache.begin_load(vec![PathBuf::from("notes.txt")], &mut worker) {
            panic!("current directory should be available: {error}");
        }

        assert_eq!(
            cache
                .ratings
                .shown(&old_path)
                .and_then(|rating| rating.stars),
            Some(3)
        );
    }

    #[test]
    fn rating_summary_uses_the_swedish_rating_text() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-rating-summary-{}", std::process::id()));
        let mut worker =
            match MetadataWorker::start_with(&program.to_string_lossy(), &[], Some(test_setup())) {
                Ok(worker) => worker,
                Err(error) => panic!("worker thread should start: {error}"),
            };
        let mut cache = MetadataCache::default();
        let path = std::env::temp_dir().join("pw-loupe-summary.ARW");
        let files = match cache.begin_load(vec![path.clone()], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        let Some(id) = cache.load_request else {
            panic!("supported image should queue a metadata read");
        };
        let mut failures = Vec::new();
        cache.apply(
            WorkerEvent::RatingsRead {
                id,
                result: Ok(HashMap::from([(
                    files[0].clone(),
                    Rating {
                        stars: Some(3),
                        label: Some("Red".to_owned()),
                    },
                )])),
            },
            &mut failures,
        );

        assert!(failures.is_empty());
        assert_eq!(cache.notice(Some(&files[0])), None);
        assert_eq!(
            cache.rating_summary(Some(&files[0])).as_deref(),
            Some("Betyg: 3 stjärnor, Röd")
        );
    }

    #[test]
    fn queue_full_read_keeps_images_loading_without_an_error_notice() {
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull)]);
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };

        assert!(cache.is_loading(Some(&files[0])));
        assert!(!cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Läser metadata…")
        );
        assert_eq!(cache.pending_read, Some(files));
    }

    #[test]
    fn pending_read_is_retried_and_its_result_marks_images_ready() {
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull), Ok(7)]);
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };

        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.load_request, Some(7));
        worker.events.push_back(WorkerEvent::RatingsRead {
            id: 7,
            result: Ok(HashMap::new()),
        });
        assert!(cache.poll(&mut worker).is_empty());

        assert!(!cache.is_loading(Some(&files[0])));
        assert!(!cache.is_error(Some(&files[0])));
        assert_eq!(cache.notice(Some(&files[0])), None);
    }

    #[test]
    fn stopped_pending_read_fails_loading_with_the_existing_error() {
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull), Err(SubmitError::Stopped)]);
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };

        assert!(cache.poll(&mut worker).is_empty());

        assert!(!cache.is_loading(Some(&files[0])));
        assert!(cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Metadatafel: metadataarbetaren har stannat")
        );
    }

    #[test]
    fn a_new_load_replaces_the_pending_read() {
        let mut worker = FakeSubmit::new([
            Err(SubmitError::QueueFull),
            Err(SubmitError::QueueFull),
            Ok(9),
        ]);
        let mut cache = MetadataCache::default();
        let old_files = match cache.begin_load(vec![PathBuf::from("old.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        let new_files = match cache.begin_load(vec![PathBuf::from("new.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };

        assert!(cache.poll(&mut worker).is_empty());

        assert_eq!(
            worker.read_attempts,
            vec![old_files.clone(), new_files.clone(), new_files.clone(),]
        );
        assert!(cache.is_loading(Some(&new_files[0])));
        assert!(!cache.images.contains_key(&old_files[0]));
    }

    #[test]
    fn pending_read_is_submitted_before_queued_rating_writes() {
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull), Ok(11)]);
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        if let Err(error) = cache.set_rating(&files[0], RatingKey::Stars(3)) {
            panic!("supported rating path should be accepted: {error}");
        }
        let operations_before_poll = worker.operations.len();

        assert!(cache.poll(&mut worker).is_empty());

        assert_eq!(
            &worker.operations[operations_before_poll..],
            ["read", "write"]
        );
    }

    /// A cache with one rating write queued for each named file (absolute paths, no load).
    fn cache_with_writes(names: &[&str]) -> MetadataCache {
        let mut cache = MetadataCache::default();
        for name in names {
            let path = std::env::temp_dir().join(name);
            if let Err(error) = cache.set_rating(&path, RatingKey::Stars(2)) {
                panic!("supported rating path should be accepted: {error}");
            }
        }
        cache
    }

    #[test]
    fn exit_wait_returns_at_once_when_nothing_is_pending() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let mut clock_reads = 0;
        let mut sleeps = 0;

        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || {
                clock_reads += 1;
                Instant::now()
            },
            |_| sleeps += 1,
        );

        assert!(unsaved.is_empty());
        assert_eq!((clock_reads, sleeps), (0, 0));
        assert!(worker.operations.is_empty());
    }

    #[test]
    fn exit_wait_ends_when_the_last_write_is_confirmed() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Ok(()));
        let mut cache = cache_with_writes(&["pw-loupe-exit-a.ARW", "pw-loupe-exit-b.DNG"]);
        let clock = std::cell::Cell::new(Instant::now());
        let start = clock.get();
        let mut sleeps = 0;

        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| {
                sleeps += 1;
                clock.set(clock.get() + pause);
            },
        );

        assert_eq!(unsaved, UnsavedRatings::default());
        assert_eq!(cache.pending_writes(), 0);
        assert_eq!(worker.write_attempts.len(), 2);
        assert_eq!(sleeps, 1, "one poll sends, the next confirms");
        assert_eq!(clock.get() - start, EXIT_POLL_INTERVAL);
    }

    #[test]
    fn exit_wait_gives_up_after_five_seconds_and_lists_the_pending_files() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = cache_with_writes(&["pw-loupe-exit-c.ARW", "pw-loupe-exit-d.RW2"]);
        let clock = std::cell::Cell::new(Instant::now());
        let start = clock.get();

        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
        );

        assert_eq!(clock.get() - start, EXIT_WAIT_LIMIT);
        assert!(unsaved.failed.is_empty());
        assert_eq!(
            unsaved.pending,
            ["pw-loupe-exit-c.ARW", "pw-loupe-exit-d.RW2"]
        );
        assert_eq!(cache.pending_writes(), 2, "the writes are still pending");
    }

    #[test]
    fn exit_wait_reports_writes_that_fail_during_the_wait() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Err("skrivskyddad".to_owned()));
        let mut cache = cache_with_writes(&["pw-loupe-exit-e.ARW"]);
        let clock = std::cell::Cell::new(Instant::now());

        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
        );

        assert!(unsaved.pending.is_empty());
        assert_eq!(
            unsaved.failed,
            ["Kunde inte spara betyget för pw-loupe-exit-e.ARW: skrivskyddad"]
        );
        assert_eq!(unsaved.lines(), unsaved.failed);
    }

    #[test]
    fn unsaved_list_text_shows_at_most_twenty_lines_then_the_rest_as_a_count() {
        let lines: Vec<String> = (1..=23).map(|n| format!("bild{n}.ARW")).collect();

        let text = unsaved_list_text(&lines);

        let shown: Vec<&str> = text.lines().collect();
        assert_eq!(shown.len(), 21);
        assert_eq!(shown.first(), Some(&"bild1.ARW"));
        assert_eq!(shown.get(19), Some(&"bild20.ARW"));
        assert_eq!(shown.last(), Some(&"… och 3 till"));
        assert_eq!(
            unsaved_list_text(&lines[..20]).lines().count(),
            20,
            "exactly twenty lines need no count"
        );
    }

    #[test]
    fn write_error_texts_are_swedish_for_read_only_and_refused_writes() {
        assert_eq!(
            write_error_text(WriteError::ReadOnly),
            "filen är skrivskyddad"
        );
        assert_eq!(
            write_error_text(WriteError::NotWritten(
                "0 image files updated\n1 files weren't updated due to errors".into()
            )),
            "filen kunde inte skrivas"
        );
        assert_eq!(
            write_error_text(WriteError::Other("ExifTool exited unexpectedly".into())),
            "ExifTool exited unexpectedly"
        );
    }

    #[test]
    fn write_error_texts_are_swedish_for_writes_that_were_not_safe() {
        assert_eq!(
            write_error_text(WriteError::NotSafe {
                stage: Stage::ImageCheck,
                original: OriginalState::Intact,
                reason: "preview 2 differs".into(),
            }),
            "metadata kunde inte skrivas säkert, filen är oförändrad"
        );
        let backup = std::env::temp_dir().join("bild.pwbak-0123.jpg");
        assert_eq!(
            write_error_text(WriteError::NotSafe {
                stage: Stage::Replace,
                original: OriginalState::AtBackup(backup),
                reason: "access denied".into(),
            }),
            "metadata kunde inte skrivas säkert, originalet ligger nu som bild.pwbak-0123.jpg"
        );
        assert_eq!(
            write_error_text(WriteError::NotSafe {
                stage: Stage::FinalCheck,
                original: OriginalState::Replaced,
                reason: "Rating is 2".into(),
            }),
            "filen skrevs men slutkontrollen misslyckades, den tidigare versionen finns kvar som \
             säkerhetskopia"
        );
    }

    #[test]
    fn a_write_without_the_journal_gives_the_swedish_text() {
        assert_eq!(
            write_error_text(WriteError::Other(NO_JOURNAL.into())),
            "skrivloggen kunde inte öppnas"
        );
    }

    #[test]
    fn a_read_only_write_result_gives_the_swedish_message_through_poll() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = cache_with_writes(&["pw-loupe-read-only.ARW"]);
        assert!(cache.poll(&mut worker).is_empty(), "the first poll sends");
        assert_eq!(worker.write_attempts.len(), 1);
        let id = worker.next_write_id - 1;
        worker.events.push_back(WorkerEvent::RatingWritten {
            id,
            path: std::env::temp_dir().join("pw-loupe-read-only.ARW"),
            rating: Rating {
                stars: Some(2),
                label: None,
            },
            result: Err(WriteError::ReadOnly),
        });

        let failures = cache.poll(&mut worker);

        assert_eq!(
            failures,
            ["Kunde inte spara betyget för pw-loupe-read-only.ARW: filen är skrivskyddad"]
        );
        assert_eq!(cache.pending_writes(), 0);
    }

    #[test]
    fn set_ratings_and_failure_text_pass_through_to_the_store() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Err("skrivskyddad".to_owned()));
        let mut cache = MetadataCache::default();
        let paths = [
            std::env::temp_dir().join("pw-loupe-cache-batch-a.ARW"),
            std::env::temp_dir().join("pw-loupe-cache-batch-b.png"),
            std::env::temp_dir().join("pw-loupe-cache-batch-c.DNG"),
        ];

        assert_eq!(cache.set_ratings(&paths, RatingKey::Stars(4)), Ok(2));
        assert_eq!(cache.failure_text(), None);
        assert!(cache.poll(&mut worker).is_empty(), "the first poll sends");
        assert_eq!(worker.write_attempts, [paths[0].clone(), paths[2].clone()]);

        let failures = cache.poll(&mut worker);

        assert_eq!(failures.len(), 2);
        assert_eq!(
            cache.failure_text().as_deref(),
            Some(
                "Kunde inte spara betyget för 2 bilder. Senast: pw-loupe-cache-batch-c.DNG: skrivskyddad"
            )
        );
        assert_eq!(
            cache.set_ratings(&paths[1..2], RatingKey::Stars(1)),
            Err(RatingRefused::Unsupported)
        );
    }

    #[test]
    fn ratings_are_not_ready_while_the_folder_read_is_sent_or_waiting() {
        let mut cache = MetadataCache::default();
        assert!(cache.ratings_ready(), "nothing loaded, nothing to wait for");

        // Waiting to be sent (queue full), then sent, then answered.
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull), Ok(7)]);
        if let Err(error) = cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert!(!cache.ratings_ready(), "the read waits to be sent");
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.load_request, Some(7));
        assert!(!cache.ratings_ready(), "the read is sent");
        worker.events.push_back(WorkerEvent::RatingsRead {
            id: 7,
            result: Ok(HashMap::new()),
        });
        assert!(cache.poll(&mut worker).is_empty());
        assert!(cache.ratings_ready());
        assert_eq!(cache.read_error(), None);

        // A folder without ratable files has nothing to read.
        let mut worker = FakeSubmit::new([]);
        if let Err(error) = cache.begin_load(vec![PathBuf::from("notes.txt")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert!(cache.ratings_ready());
        assert!(worker.read_attempts.is_empty());
    }

    #[test]
    fn a_failed_folder_read_gives_a_read_error_until_the_next_load() {
        let mut worker = FakeSubmit::new([Ok(3), Ok(4)]);
        let mut cache = MetadataCache::default();
        if let Err(error) = cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(cache.read_error(), None);
        worker.events.push_back(WorkerEvent::RatingsRead {
            id: 3,
            result: Err("exiftool saknas".to_owned()),
        });
        assert!(cache.poll(&mut worker).is_empty());

        assert_eq!(cache.read_error().as_deref(), Some("exiftool saknas"));
        // The wait is over; the caller treats the error as "not ready" (doc comment).
        assert!(cache.ratings_ready());

        if let Err(error) = cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(cache.read_error(), None, "a new load clears the error");
        assert!(!cache.ratings_ready());
    }

    #[test]
    fn a_read_that_cannot_be_sent_or_a_stopped_worker_gives_a_read_error() {
        // The worker refuses the read at once.
        let mut worker = FakeSubmit::new([Err(SubmitError::Stopped)]);
        let mut cache = MetadataCache::default();
        if let Err(error) = cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(
            cache.read_error().as_deref(),
            Some("metadataarbetaren har stannat")
        );
        assert!(cache.ratings_ready());

        // The worker stops while the read is sent.
        let mut worker = FakeSubmit::new([Ok(5)]);
        let mut cache = MetadataCache::default();
        if let Err(error) = cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        let mut failures = Vec::new();
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert_eq!(
            cache.read_error().as_deref(),
            Some("metadataarbetaren har stannat")
        );

        // The worker stops after the read was answered: the ratings were read, no read error.
        let mut worker = FakeSubmit::new([Ok(6)]);
        let mut cache = MetadataCache::default();
        if let Err(error) = cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        worker.events.push_back(WorkerEvent::RatingsRead {
            id: 6,
            result: Ok(HashMap::new()),
        });
        assert!(cache.poll(&mut worker).is_empty());
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert_eq!(cache.read_error(), None);
        assert!(cache.ratings_ready());
    }
}
