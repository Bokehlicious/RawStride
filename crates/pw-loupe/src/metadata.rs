use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use pw_loupe::{
    RatingKey, RatingRefused, RatingStore, RatingWriter, UNSUPPORTED_TEXT, status_text,
};
use pw_metadata::read::MetadataRead;
use pw_metadata::safe_write::OriginalState;
use pw_metadata::{
    EXIFTOOL_TOO_OLD, MIN_EXIFTOOL_VERSION, MetadataWorker, NO_JOURNAL, Rating, RequestId,
    SubmitError, WorkerEvent, WriteError, supports_rating,
};

pub(crate) trait MetadataSubmit: RatingWriter {
    fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError>;

    /// All metadata of one image, for the info panel (T98, T103).
    fn read_metadata(&mut self, image: PathBuf) -> Result<RequestId, SubmitError>;

    fn try_recv(&mut self) -> Result<WorkerEvent, TryRecvError>;
}

impl MetadataSubmit for MetadataWorker {
    fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError> {
        MetadataWorker::read_ratings(self, images)
    }

    fn read_metadata(&mut self, image: PathBuf) -> Result<RequestId, SubmitError> {
        MetadataWorker::read_metadata(self, image)
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

/// How long an image must have been the current one before the info panel asks for its metadata,
/// so moving quickly with the arrow keys queues nothing (ADR 0007 I10j, D15, T103).
pub(crate) const PANEL_READ_DELAY: Duration = Duration::from_millis(200);
/// How often a frame is asked for while the panel's read is in flight or waits to be sent.
const PANEL_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// What the info panel shows for its image (T103).
#[derive(Debug, PartialEq)]
pub(crate) enum PanelState<'a> {
    /// The file kind has no metadata support.
    Unsupported,
    /// No result yet: the read waits for `PANEL_READ_DELAY`, is in flight or not yet sent.
    Loading,
    Error(&'a str),
    Ready(&'a MetadataRead),
}

/// The metadata of the one image shown in the info panel (ADR 0007 §5.1, D15, T103). At most one
/// `read_metadata` is in flight; its result is kept only if it is for the panel's image.
#[derive(Default)]
struct PanelRead {
    /// The panel's image (absolute, supported) and when it became the current one.
    image: Option<(PathBuf, Instant)>,
    /// The result for `image`, once one has come back.
    result: Option<Result<MetadataRead, String>>,
    /// `image` must be read (again): it is new, or RawStride wrote to it.
    needs_read: bool,
    /// The one read in flight, for whichever image it was asked for.
    in_flight: Option<RequestId>,
}

impl PanelRead {
    fn is_image(&self, path: &Path) -> bool {
        self.image.as_ref().is_some_and(|(image, _)| image == path)
    }

    /// Empties the panel's metadata. A read in flight stays in flight, so a second one is not
    /// sent before it is back; its result is then dropped.
    fn clear(&mut self) {
        *self = Self {
            in_flight: self.in_flight,
            ..Self::default()
        };
    }
}

#[derive(Default)]
pub(crate) struct MetadataCache {
    images: HashMap<PathBuf, ImageMetadata>,
    load_request: Option<RequestId>,
    pending_read: Option<Vec<PathBuf>>,
    /// Why the last folder read failed (ADR 0014 Q7); cleared by the next `begin_load`.
    read_error: Option<String>,
    ratings: RatingStore,
    panel: PanelRead,
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
        // A loaded folder change empties the panel; the same folder loaded again (a switch between
        // grid and loupe) keeps it.
        if !files.iter().any(|path| self.panel.is_image(path)) {
            self.panel.clear();
        }
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
                    // The panel's read in flight can never come back either.
                    if self.panel.in_flight.take().is_some() && self.panel.image.is_some() {
                        self.panel.result = Some(Err(message));
                        self.panel.needs_read = false;
                    }
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

    /// Called every frame with the info panel's image (absolute path) and whether the panel is
    /// shown (ADR 0007 I10j, D15, T103). Asks for the image's metadata when the panel is shown, no
    /// read is in flight and the image has been the current one for `PANEL_READ_DELAY`. Returns
    /// how soon a frame is needed to go on (the rest of the delay, or a read in flight), if at all.
    pub fn panel_request(
        &mut self,
        image: Option<&Path>,
        shown: bool,
        now: Instant,
        worker: &mut impl MetadataSubmit,
    ) -> Option<Duration> {
        let image = image.filter(|path| supports_rating(path));
        if self.panel.image.as_ref().map(|(path, _)| path.as_path()) != image {
            self.panel.image = image.map(|path| (path.to_path_buf(), now));
            self.panel.result = None;
            self.panel.needs_read = image.is_some();
        }
        if !shown {
            return None;
        }
        if self.panel.in_flight.is_some() {
            return Some(PANEL_POLL_INTERVAL);
        }
        if !self.panel.needs_read {
            return None;
        }
        let (path, since) = self.panel.image.clone()?;
        let waited = now.saturating_duration_since(since);
        if waited < PANEL_READ_DELAY {
            return Some(PANEL_READ_DELAY - waited);
        }
        match worker.read_metadata(path) {
            Ok(id) => {
                self.panel.in_flight = Some(id);
                self.panel.needs_read = false;
                Some(PANEL_POLL_INTERVAL)
            }
            Err(SubmitError::QueueFull) => Some(PANEL_POLL_INTERVAL),
            Err(error) => {
                self.panel.result = Some(Err(submit_error(error)));
                self.panel.needs_read = false;
                None
            }
        }
    }

    /// What the info panel shows for `image` (absolute path).
    pub fn panel_state(&self, image: &Path) -> PanelState<'_> {
        if !supports_rating(image) {
            return PanelState::Unsupported;
        }
        if !self.panel.is_image(image) {
            return PanelState::Loading;
        }
        match &self.panel.result {
            None => PanelState::Loading,
            Some(Ok(read)) => PanelState::Ready(read),
            Some(Err(error)) => PanelState::Error(error),
        }
    }

    /// RawStride wrote to `path`: the panel reads it again if it is the panel's image. The values
    /// read before stay shown until the new ones come back.
    fn panel_written(&mut self, path: &Path) {
        if self.panel.is_image(path) {
            self.panel.needs_read = true;
        }
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
                self.panel_written(&path);
                if let Some(message) =
                    self.ratings
                        .written(id, &path, result.map_err(write_error_text))
                {
                    failures.push(message);
                }
            }
            WorkerEvent::RatingsRead { .. } => {}
            WorkerEvent::MetadataRead { id, path, result } => {
                if self.panel.in_flight == Some(id) {
                    self.panel.in_flight = None;
                    // A result for another image than the panel's is dropped.
                    if self.panel.is_image(&path) {
                        self.panel.result = Some(result);
                    }
                }
            }
            // The editor (T105) sends the writes; the panel reads the image again after one.
            WorkerEvent::EditsWritten { path, .. } => self.panel_written(&path),
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
        // The metadata worker refuses rating writes while ExifTool is too old or its version
        // could not be read (ADR 0007 §11, D9, T101).
        WriteError::Other(text) if text == EXIFTOOL_TOO_OLD => {
            format!("ExifTool är för gammal (minst {MIN_EXIFTOOL_VERSION})")
        }
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
        /// The images of every `read_metadata` call, with the id each was given.
        metadata_reads: Vec<(RequestId, PathBuf)>,
        next_metadata_id: RequestId,
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
                metadata_reads: Vec::new(),
                next_metadata_id: 500,
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

        fn read_metadata(&mut self, image: PathBuf) -> Result<RequestId, SubmitError> {
            self.operations.push("read_metadata");
            let id = self.next_metadata_id;
            self.next_metadata_id += 1;
            self.metadata_reads.push((id, image));
            Ok(id)
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
    fn a_write_refused_for_an_old_exiftool_gives_the_swedish_text() {
        assert_eq!(
            write_error_text(WriteError::Other(EXIFTOOL_TOO_OLD.into())),
            "ExifTool är för gammal (minst 12.67)"
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

    // --- the info panel's metadata (ADR 0007 I10j, D15, T103) ---

    /// An empty read of a JPEG without IIM, as the worker could answer it.
    fn empty_read() -> MetadataRead {
        use pw_metadata::iim::CodedCharset;
        use pw_metadata::reconcile::{FileKind, IimDigest, Reconciled};
        use pw_metadata::value::Metadata;
        use pw_metadata::write_plan::{ExifPresent, FileState};
        MetadataRead {
            reconciled: Reconciled {
                metadata: Metadata::new(),
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: FileState {
                kind: FileKind::Jpeg,
                iim_present: false,
                iim_charset: CodedCharset::Absent,
                iim_values: Vec::new(),
                digest: IimDigest::Missing,
                exif_present: ExifPresent::default(),
            },
        }
    }

    fn panel_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn metadata_read(
        id: RequestId,
        path: &Path,
        result: Result<MetadataRead, String>,
    ) -> WorkerEvent {
        WorkerEvent::MetadataRead {
            id,
            path: path.to_path_buf(),
            result,
        }
    }

    #[test]
    fn the_panel_asks_for_nothing_before_200_ms_or_while_hidden() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = panel_path("pw-loupe-panel-wait.ARW");
        let start = Instant::now();

        // Hidden: nothing is asked for, however long the image is current.
        assert_eq!(
            cache.panel_request(Some(&image), false, start, &mut worker),
            None
        );
        let later = start + Duration::from_secs(5);
        assert_eq!(
            cache.panel_request(Some(&image), false, later, &mut worker),
            None
        );
        assert!(worker.metadata_reads.is_empty());

        // Another image, shown: nothing before 200 ms, and the rest of the wait is returned.
        let other = panel_path("pw-loupe-panel-wait.jpg");
        assert_eq!(
            cache.panel_request(Some(&other), true, later, &mut worker),
            Some(PANEL_READ_DELAY)
        );
        let almost = later + Duration::from_millis(199);
        assert_eq!(
            cache.panel_request(Some(&other), true, almost, &mut worker),
            Some(Duration::from_millis(1))
        );
        assert!(worker.metadata_reads.is_empty());
        assert_eq!(cache.panel_state(&other), PanelState::Loading);

        // At 200 ms the one read is sent.
        cache.panel_request(Some(&other), true, later + PANEL_READ_DELAY, &mut worker);
        assert_eq!(worker.metadata_reads, [(500, other.clone())]);
    }

    #[test]
    fn moving_quickly_queues_nothing_and_an_unsupported_file_is_never_read() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let start = Instant::now();
        // A new image every 100 ms: none is current for 200 ms.
        for step in 0..5u32 {
            let image = panel_path(&format!("pw-loupe-panel-step-{step}.DNG"));
            let now = start + Duration::from_millis(100) * step;
            cache.panel_request(Some(&image), true, now, &mut worker);
        }
        assert!(worker.metadata_reads.is_empty());

        let text = panel_path("pw-loupe-panel-notes.txt");
        assert_eq!(
            cache.panel_request(Some(&text), true, start, &mut worker),
            None
        );
        let later = start + Duration::from_secs(1);
        assert_eq!(
            cache.panel_request(Some(&text), true, later, &mut worker),
            None
        );
        assert!(worker.metadata_reads.is_empty());
        assert_eq!(cache.panel_state(&text), PanelState::Unsupported);
    }

    #[test]
    fn one_read_in_flight_then_the_newest_image_and_a_stale_result_is_dropped() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let first = panel_path("pw-loupe-panel-first.ARW");
        let second = panel_path("pw-loupe-panel-second.ARW");
        let third = panel_path("pw-loupe-panel-third.ARW");
        let start = Instant::now();
        cache.panel_request(Some(&first), true, start, &mut worker);
        cache.panel_request(Some(&first), true, start + PANEL_READ_DELAY, &mut worker);
        assert_eq!(worker.metadata_reads, [(500, first.clone())]);

        // The image changes twice while the read runs: nothing more is sent.
        let t = start + Duration::from_millis(300);
        cache.panel_request(Some(&second), true, t, &mut worker);
        let t = t + Duration::from_millis(300);
        assert_eq!(
            cache.panel_request(Some(&third), true, t, &mut worker),
            Some(PANEL_POLL_INTERVAL)
        );
        let t = t + Duration::from_secs(1);
        cache.panel_request(Some(&third), true, t, &mut worker);
        assert_eq!(worker.metadata_reads.len(), 1, "one read in flight");

        // The first image's result comes back: it is dropped.
        worker
            .events
            .push_back(metadata_read(500, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.panel_state(&third), PanelState::Loading);
        assert_eq!(cache.panel_state(&first), PanelState::Loading);

        // Then the newest image is asked for, and its result is kept.
        cache.panel_request(Some(&third), true, t, &mut worker);
        assert_eq!(
            worker.metadata_reads,
            [(500, first.clone()), (501, third.clone())]
        );
        worker
            .events
            .push_back(metadata_read(501, &third, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.panel_state(&third), PanelState::Ready(&empty_read()));
        assert_eq!(
            cache.panel_request(Some(&third), true, t, &mut worker),
            None
        );
        assert_eq!(
            worker.metadata_reads.len(),
            2,
            "a read image is not read again"
        );
    }

    #[test]
    fn a_result_with_another_id_is_dropped_and_an_error_is_shown() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = panel_path("pw-loupe-panel-error.jpg");
        let start = Instant::now();
        cache.panel_request(Some(&image), true, start, &mut worker);
        cache.panel_request(Some(&image), true, start + PANEL_READ_DELAY, &mut worker);

        worker
            .events
            .push_back(metadata_read(77, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.panel_state(&image), PanelState::Loading);

        worker
            .events
            .push_back(metadata_read(500, &image, Err("filen saknas".to_owned())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.panel_state(&image), PanelState::Error("filen saknas"));
    }

    #[test]
    fn the_panel_reads_its_image_again_after_a_rating_write() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Ok(()));
        let mut cache = MetadataCache::default();
        let image = panel_path("pw-loupe-panel-rated.ARW");
        let other = panel_path("pw-loupe-panel-other.ARW");
        let start = Instant::now();
        cache.panel_request(Some(&image), true, start, &mut worker);
        let t = start + PANEL_READ_DELAY;
        cache.panel_request(Some(&image), true, t, &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.panel_request(Some(&image), true, t, &mut worker),
            None
        );

        // A rating write of another image changes nothing.
        if let Err(error) = cache.set_rating(&other, RatingKey::Stars(2)) {
            panic!("supported rating path should be accepted: {error}");
        }
        assert!(cache.poll(&mut worker).is_empty(), "the write is sent");
        assert!(cache.poll(&mut worker).is_empty(), "the write is confirmed");
        assert_eq!(
            cache.panel_request(Some(&image), true, t, &mut worker),
            None
        );
        assert_eq!(worker.metadata_reads.len(), 1);

        // A rating write of the panel's image: read again, the old values shown meanwhile.
        if let Err(error) = cache.set_rating(&image, RatingKey::Stars(4)) {
            panic!("supported rating path should be accepted: {error}");
        }
        assert!(cache.poll(&mut worker).is_empty(), "the write is sent");
        assert!(cache.poll(&mut worker).is_empty(), "the write is confirmed");
        assert_eq!(cache.panel_state(&image), PanelState::Ready(&empty_read()));
        cache.panel_request(Some(&image), true, t, &mut worker);
        assert_eq!(
            worker.metadata_reads,
            [(500, image.clone()), (501, image.clone())]
        );
    }

    #[test]
    fn a_folder_change_empties_the_panel_and_the_same_folder_keeps_it() {
        let mut worker = FakeSubmit::new([Ok(1), Ok(2), Ok(3)]);
        let mut cache = MetadataCache::default();
        let image = panel_path("pw-loupe-panel-folder.ARW");
        if let Err(error) = cache.begin_load(vec![image.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        let start = Instant::now();
        cache.panel_request(Some(&image), true, start, &mut worker);
        let t = start + PANEL_READ_DELAY;
        cache.panel_request(Some(&image), true, t, &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());

        // The same folder loaded again (grid ↔ loupe) keeps the values.
        if let Err(error) = cache.begin_load(vec![image.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(cache.panel_state(&image), PanelState::Ready(&empty_read()));

        // Another folder empties them.
        let elsewhere = panel_path("pw-loupe-panel-elsewhere.ARW");
        if let Err(error) = cache.begin_load(vec![elsewhere], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(cache.panel_state(&image), PanelState::Loading);
    }
}
