use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use pw_loupe::{
    EditorText, RatingKey, RatingRefused, RatingStore, RatingWriter, UNSUPPORTED_TEXT, WriteReason,
    editor_text, notice_text, path_resolution_error_text, status_text, write_reason_text,
};
use pw_metadata::edit::Edit;
use pw_metadata::read::MetadataRead;
use pw_metadata::safe_write::OriginalState;
use pw_metadata::write_plan::WriteNotice;
use pw_metadata::{
    DeleteError, MetadataWorker, Moved, PreparedDelete, Rating, RequestId, SubmitError,
    WorkerEvent, WriteError, supports_rating,
};

use crate::inspector::{INSPECTOR_POLL_INTERVAL, INSPECTOR_READ_DELAY};

pub(crate) trait MetadataSubmit: RatingWriter {
    fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError>;

    /// All metadata of one image, for the metadata window (T98, T107).
    fn read_metadata(&mut self, image: PathBuf) -> Result<RequestId, SubmitError>;

    /// The editor's edits of one image (T98, T105).
    fn write_edits(&mut self, image: PathBuf, edits: Vec<Edit>) -> Result<RequestId, SubmitError>;

    /// A move of `image` and its companion files into `target` (ADR 0017 U9b, T170, T173).
    fn move_image(
        &mut self,
        image: PathBuf,
        target: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> Result<RequestId, SubmitError>;

    /// The files of `moved` moved back where they came from (ADR 0017 U9b, T170, T173).
    fn move_back(&mut self, moved: Vec<Moved>) -> Result<RequestId, SubmitError>;

    /// The groups of `images` and whether each can be deleted, for the confirmation (ADR 0018
    /// B2, T186, T188).
    fn prepare_delete(
        &mut self,
        images: Vec<PathBuf>,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> Result<RequestId, SubmitError>;

    /// `image` and its companion files sent to the Recycle Bin (ADR 0018 B2, T186, T188).
    fn delete_image(
        &mut self,
        image: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> Result<RequestId, SubmitError>;

    fn try_recv(&mut self) -> Result<WorkerEvent, TryRecvError>;
}

impl MetadataSubmit for MetadataWorker {
    fn read_ratings(&mut self, images: Vec<PathBuf>) -> Result<RequestId, SubmitError> {
        MetadataWorker::read_ratings(self, images)
    }

    fn read_metadata(&mut self, image: PathBuf) -> Result<RequestId, SubmitError> {
        MetadataWorker::read_metadata(self, image)
    }

    fn write_edits(&mut self, image: PathBuf, edits: Vec<Edit>) -> Result<RequestId, SubmitError> {
        MetadataWorker::write_edits(self, image, edits)
    }

    fn move_image(
        &mut self,
        image: PathBuf,
        target: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> Result<RequestId, SubmitError> {
        MetadataWorker::move_image(self, image, target, image_extensions, suffixes)
    }

    fn move_back(&mut self, moved: Vec<Moved>) -> Result<RequestId, SubmitError> {
        MetadataWorker::move_back(self, moved)
    }

    fn prepare_delete(
        &mut self,
        images: Vec<PathBuf>,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> Result<RequestId, SubmitError> {
        MetadataWorker::prepare_delete(self, images, image_extensions, suffixes)
    }

    fn delete_image(
        &mut self,
        image: PathBuf,
        image_extensions: Vec<String>,
        suffixes: Vec<String>,
    ) -> Result<RequestId, SubmitError> {
        MetadataWorker::delete_image(self, image, image_extensions, suffixes)
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

/// The metadata of the one image of the metadata window (ADR 0007 §5.1, D16, T107). It is asked
/// for at once when the window opens or moves to another image (ADR 0017 §2.3.1, T112), never
/// while moving between images behind a closed window. At most one
/// `read_metadata` is in flight; its result is kept only if it is for the window's image.
#[derive(Default)]
struct WindowRead {
    /// The window's image (absolute, supported), while the window is open.
    image: Option<PathBuf>,
    /// A result for `image` not yet taken by the window.
    result: Option<Result<MetadataRead, String>>,
    /// `image` must be read (again): the window opened, or RawStride wrote to it.
    needs_read: bool,
    /// The one read in flight, for whichever image it was asked for.
    in_flight: Option<RequestId>,
}

impl WindowRead {
    fn is_image(&self, path: &Path) -> bool {
        self.image.as_deref() == Some(path)
    }
}

/// What the inspector's Metadata section shows for its image (ADR 0017 §2.1, T110).
#[derive(Debug, PartialEq)]
pub(crate) enum InspectorState<'a> {
    /// The file kind has no metadata support.
    Unsupported,
    /// No result yet: the read waits for `INSPECTOR_READ_DELAY`, is in flight or not yet sent.
    Loading,
    Error(&'a str),
    Ready(&'a MetadataRead),
}

/// The metadata of the inspector's one image (ADR 0017 §2.1, T110), read with the rules of T103:
/// only while the Metadata section is shown, after the image has been the current one for
/// `INSPECTOR_READ_DELAY`, at most one `read_metadata` in flight, its result kept only if it is
/// for the inspector's image. It is apart from the metadata window's read (T107).
#[derive(Default)]
struct InspectorRead {
    /// The inspector's image (absolute, supported) and when it became the current one.
    image: Option<(PathBuf, Instant)>,
    /// The result for `image`, once one has come back.
    result: Option<Result<MetadataRead, String>>,
    /// `image` must be read (again): it is new, or RawStride wrote to it.
    needs_read: bool,
    /// The one read in flight, for whichever image it was asked for.
    in_flight: Option<RequestId>,
    /// `result` changed since the last `inspector_request` (it came back, or the worker
    /// stopped): the section was drawn before the results were taken in that frame, so one more
    /// frame is asked for at once (found with the Histogram section, T144).
    arrived: bool,
}

impl InspectorRead {
    fn is_image(&self, path: &Path) -> bool {
        self.image.as_ref().is_some_and(|(image, _)| image == path)
    }

    /// Empties the inspector's metadata. A read in flight stays in flight, so a second one is not
    /// sent before it is back; its result is then dropped.
    fn clear(&mut self) {
        *self = Self {
            in_flight: self.in_flight,
            ..Self::default()
        };
    }
}

/// What the editor window learns about its write (ADR 0007 I10l, D12, T105).
#[derive(Debug, PartialEq)]
pub(crate) enum EditorEvent {
    /// The write of `path` succeeded; a new read of it is asked for.
    Saved {
        path: PathBuf,
        notices: Vec<WriteNotice>,
    },
    /// The write of `path` failed; `message` is the text the window shows.
    Failed { path: PathBuf, message: String },
    /// The new read of `path` after a write that succeeded, or after a batch.
    Reloaded {
        path: PathBuf,
        result: Result<MetadataRead, String>,
    },
    /// Every write of "Apply to All Marked" has answered (U12c-Q6, T126); a new read of the
    /// window's image is asked for.
    BatchDone(BatchSummary),
}

/// What a batch of "Apply to All Marked" gave (ADR 0017 §2.3.1, U12c-Q6, T126).
#[derive(Debug, PartialEq)]
pub(crate) struct BatchSummary {
    /// How many images were written.
    pub written: usize,
    /// How many images the batch had.
    pub total: usize,
    /// The failed files, each with its reason ("Could not save metadata for <file>: …").
    pub failed: Vec<String>,
    /// The other images where an unresolved conflict was kept ("Unresolved conflict kept: …") or a
    /// list with qualifiers was not written ("List with qualifiers not written: …"), in the order
    /// the answers and their notices came.
    pub kept: Vec<String>,
    /// The notices of the window's own image, shown as after its own write.
    pub notices: Vec<WriteNotice>,
}

impl BatchSummary {
    /// The lines the window shows: the written count, the failed files, the kept conflicts and
    /// lists with qualifiers not written, then the window's image's notices.
    pub fn lines(&self) -> Vec<String> {
        let written = editor_text(&EditorText::BatchWritten {
            written: self.written,
            total: self.total,
        });
        std::iter::once(written)
            .chain(self.failed.iter().cloned())
            .chain(self.kept.iter().cloned())
            .chain(self.notices.iter().map(notice_text))
            .collect()
    }
}

/// A batch of "Apply to All Marked" (ADR 0007 §9.5, U12c-Q5, T126): each image its own
/// `write_edits`, sent as the worker's queue takes them; one failure stops nothing else.
struct EditorBatch {
    /// The window's image, read again when every write has answered.
    window_image: PathBuf,
    /// How many images the batch has.
    total: usize,
    /// The writes not yet sent, in order.
    queue: VecDeque<(PathBuf, Vec<Edit>)>,
    /// The writes sent and not yet answered.
    in_flight: Vec<(RequestId, PathBuf)>,
    written: usize,
    failed: Vec<String>,
    kept: Vec<String>,
    notices: Vec<WriteNotice>,
}

impl EditorBatch {
    /// How many writes have answered (a write that could not be sent counts as answered).
    fn answered(&self) -> usize {
        self.total
            .saturating_sub(self.queue.len() + self.in_flight.len())
    }

    /// The files of the writes not yet answered, the sent ones first.
    fn pending_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.in_flight
            .iter()
            .map(|(_, path)| path)
            .chain(self.queue.iter().map(|(path, _)| path))
    }
}

/// The editor's write and the read after it (T105). One write at a time: the window is read-only
/// while it runs.
#[derive(Default)]
struct EditorSave {
    /// The write in flight.
    writing: Option<(RequestId, PathBuf)>,
    /// The image to read again after a write, while the read waits to be sent (queue full).
    reload: Option<PathBuf>,
    /// The read in flight after a write.
    reading: Option<(RequestId, PathBuf)>,
    /// The batch of "Apply to All Marked" while it runs (T126); never at the same time as
    /// `writing`.
    batch: Option<EditorBatch>,
    /// What happened since the window last asked.
    events: Vec<EditorEvent>,
}

#[derive(Default)]
pub(crate) struct MetadataCache {
    images: HashMap<PathBuf, ImageMetadata>,
    load_request: Option<RequestId>,
    pending_read: Option<Vec<PathBuf>>,
    /// Why the last folder read failed (ADR 0014 Q7); cleared by the next `begin_load`.
    read_error: Option<String>,
    ratings: RatingStore,
    window: WindowRead,
    inspector: InspectorRead,
    editor: EditorSave,
    /// The answers of RawStride's metadata writes since the inspector's Histogram last asked, in
    /// the order they came (U7c-Q7, T145).
    written: Vec<WriteAnswer>,
    /// The answers of the moves and the moves back since `main.rs` last took them, in the order
    /// they came (ADR 0017 U9b, T173).
    moves: Vec<MoveAnswer>,
    /// The answers of the delete requests since `main.rs` last took them, in the order they came
    /// (ADR 0018 B3, T188).
    deletes: Vec<DeleteAnswer>,
}

/// One answer of the metadata worker's delete requests (ADR 0018 B2, T186, T188), with the fields
/// of its event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeleteAnswer {
    /// The answer to `MetadataSubmit::prepare_delete`: one entry per image, in the order sent.
    Prepared {
        id: RequestId,
        images: Vec<PreparedDelete>,
    },
    /// The answer to `MetadataSubmit::delete_image`: the files sent to the Recycle Bin, or why
    /// the delete failed.
    Deleted {
        id: RequestId,
        image: PathBuf,
        result: Result<Vec<PathBuf>, DeleteError>,
    },
}

/// One answer of the metadata worker's move or move back (ADR 0017 U9b, T170, T173), with the
/// fields of its event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MoveAnswer {
    /// The answer to `MetadataSubmit::move_image`: the files moved, or why the move failed.
    Moved {
        id: RequestId,
        image: PathBuf,
        result: Result<Vec<Moved>, String>,
    },
    /// The answer to `MetadataSubmit::move_back`, with `moved` as it was sent.
    MovedBack {
        id: RequestId,
        moved: Vec<Moved>,
        result: Result<(), String>,
    },
}

/// One answer of RawStride's metadata write (a rating or label key, an editor save, a write of
/// "Apply to All Marked"): its file and whether it succeeded (U7c-Q7, T145).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriteAnswer {
    pub path: PathBuf,
    pub succeeded: bool,
}

impl MetadataCache {
    /// Keys metadata by absolute path so it matches the paths sent to the view core.
    pub fn begin_load(
        &mut self,
        files: Vec<PathBuf>,
        worker: &mut impl MetadataSubmit,
    ) -> Result<Vec<PathBuf>, String> {
        let cwd =
            std::env::current_dir().map_err(|e| path_resolution_error_text(&e.to_string()))?;
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
        // A loaded folder change empties the inspector; the same folder loaded again (a switch
        // between the contact sheet and the loupe) keeps it (T110).
        if !files.iter().any(|path| self.inspector.is_image(path)) {
            self.inspector.clear();
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
        self.send_batch(worker);
        self.send_editor_reload(worker);
        self.send_window_read(worker);
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

    /// Drops what is kept for images no longer open: their ratings, and the inspector's image.
    pub fn prune(&mut self, keep: impl Fn(&Path) -> bool) {
        if self
            .inspector
            .image
            .as_ref()
            .is_some_and(|(image, _)| !keep(image))
        {
            self.inspector.clear();
        }
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
                    // The window's read in flight can never come back either.
                    if self.window.in_flight.take().is_some() && self.window.image.is_some() {
                        self.window.result = Some(Err(message.clone()));
                        self.window.needs_read = false;
                    }
                    // Nor can the inspector's.
                    if self.inspector.in_flight.take().is_some() && self.inspector.image.is_some() {
                        self.inspector.result = Some(Err(message.clone()));
                        self.inspector.needs_read = false;
                        self.inspector.arrived = true;
                    }
                    // Nor can the editor's write or the read after it.
                    if let Some((_, path)) = self.editor.writing.take() {
                        let message = save_failed_text(&path, message.clone());
                        self.editor
                            .events
                            .push(EditorEvent::Failed { path, message });
                    }
                    // Nor can the batch's writes, sent or not (T126); the batch then ends.
                    if let Some(batch) = self.editor.batch.as_mut() {
                        let paths: Vec<PathBuf> = batch
                            .in_flight
                            .drain(..)
                            .map(|(_, path)| path)
                            .chain(batch.queue.drain(..).map(|(path, _)| path))
                            .collect();
                        for path in paths {
                            batch.failed.push(save_failed_text(&path, message.clone()));
                        }
                    }
                    self.finish_batch();
                    let reload = self.editor.reading.take().map(|(_, path)| path);
                    if let Some(path) = reload.or_else(|| self.editor.reload.take()) {
                        self.editor.events.push(EditorEvent::Reloaded {
                            path,
                            result: Err(message),
                        });
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
            LoadState::Loading => Some(editor_text(&EditorText::ReadingMetadata)),
            LoadState::Ready => None,
            LoadState::Unsupported => Some(UNSUPPORTED_TEXT.into()),
            LoadState::Error(error) => Some(editor_text(&EditorText::MetadataError {
                error: error.clone(),
            })),
        }
    }

    pub fn is_loading(&self, path: Option<&Path>) -> bool {
        path.and_then(|path| self.images.get(path))
            .is_some_and(|metadata| matches!(&metadata.state, LoadState::Loading))
    }

    // Since T110 the status line is always shown, so the loupe no longer puts a metadata error
    // in the middle of the image; the tests still use this one.
    #[cfg_attr(not(test), allow(dead_code))]
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

    /// Opens the metadata window's read of `image` (absolute path, ADR 0007 I10n, D16, T107): the
    /// read is asked for at once, unless a read is still in flight (its result is then dropped
    /// and this one sent by the next `poll`). False, and nothing asked for, when the file kind
    /// has no metadata support.
    pub fn open_window(&mut self, image: &Path, worker: &mut impl MetadataSubmit) -> bool {
        if !supports_rating(image) {
            return false;
        }
        self.set_window_image(image, worker);
        true
    }

    /// Moves the open metadata window's read to `image` (absolute path, ADR 0017 §2.3.1, T112)
    /// without closing it: the new image's read is asked for at once, as when the window opens,
    /// unless a read is still in flight (its result, for the earlier image, is then dropped and
    /// this one sent by the next `poll`). The editor's write of the earlier image is unaffected
    /// and still reads that image back. False, and nothing changed, when no window is open or the
    /// file kind has no metadata support.
    pub fn move_window(&mut self, image: &Path, worker: &mut impl MetadataSubmit) -> bool {
        if self.window.image.is_none() || !supports_rating(image) {
            return false;
        }
        self.set_window_image(image, worker);
        true
    }

    /// The window's image is now `image`: a result for the earlier one is dropped and the read of
    /// this one asked for at once. A read in flight stays in flight (one at a time).
    fn set_window_image(&mut self, image: &Path, worker: &mut impl MetadataSubmit) {
        self.window = WindowRead {
            image: Some(image.to_path_buf()),
            result: None,
            needs_read: true,
            in_flight: self.window.in_flight,
        };
        self.send_window_read(worker);
    }

    /// The window closed: a result still to come is dropped. A read in flight stays in flight,
    /// so a second one is not sent before it is back.
    pub fn close_window(&mut self) {
        self.window = WindowRead {
            in_flight: self.window.in_flight,
            ..WindowRead::default()
        };
    }

    /// Whether the window's read waits to be sent or is in flight (a frame is needed to go on).
    pub fn window_waiting(&self) -> bool {
        self.window.image.is_some() && (self.window.needs_read || self.window.in_flight.is_some())
    }

    /// The newest result for the window's image, once: the window keeps it.
    pub fn take_window_read(&mut self) -> Option<Result<MetadataRead, String>> {
        self.window.result.take()
    }

    /// Sends the window's read when it is wanted and no read is in flight.
    fn send_window_read(&mut self, worker: &mut impl MetadataSubmit) {
        if !self.window.needs_read || self.window.in_flight.is_some() {
            return;
        }
        let Some(path) = self.window.image.clone() else {
            return;
        };
        match worker.read_metadata(path) {
            Ok(id) => {
                self.window.in_flight = Some(id);
                self.window.needs_read = false;
            }
            Err(SubmitError::QueueFull) => {}
            Err(error) => {
                self.window.result = Some(Err(submit_error(error)));
                self.window.needs_read = false;
            }
        }
    }

    /// Called every frame with the inspector's image (absolute path), whether its Metadata section
    /// is shown (the right column and the section open) and whether the metadata window is open
    /// (ADR 0017 §2.1, T110). Asks for the image's metadata when the section is shown, no read of
    /// the inspector is in flight and the image has been the current one for
    /// `INSPECTOR_READ_DELAY`. While the window is open the inspector keeps its image and asks for
    /// no new read. Returns how soon a frame is needed to go on (the rest of the delay, or a read
    /// in flight), if at all; at once (`Duration::ZERO`) when the shown section's result changed
    /// since the last call, since the section was drawn before the results were taken in that
    /// frame and would otherwise wait for the next input.
    pub fn inspector_request(
        &mut self,
        image: Option<&Path>,
        shown: bool,
        window_open: bool,
        now: Instant,
        worker: &mut impl MetadataSubmit,
    ) -> Option<Duration> {
        let arrived = std::mem::take(&mut self.inspector.arrived);
        let wait = self.inspector_ask(image, shown, window_open, now, worker);
        if shown && arrived {
            Some(Duration::ZERO)
        } else {
            wait
        }
    }

    /// The part of [`Self::inspector_request`] that follows the image and asks for its read when
    /// it is due.
    fn inspector_ask(
        &mut self,
        image: Option<&Path>,
        shown: bool,
        window_open: bool,
        now: Instant,
        worker: &mut impl MetadataSubmit,
    ) -> Option<Duration> {
        let image = image.filter(|path| supports_rating(path));
        let inspector = &mut self.inspector;
        if !window_open && inspector.image.as_ref().map(|(path, _)| path.as_path()) != image {
            inspector.image = image.map(|path| (path.to_path_buf(), now));
            inspector.result = None;
            inspector.needs_read = image.is_some();
        }
        if !shown {
            return None;
        }
        if inspector.in_flight.is_some() {
            return Some(INSPECTOR_POLL_INTERVAL);
        }
        if window_open || !inspector.needs_read {
            return None;
        }
        let (path, since) = inspector.image.clone()?;
        let waited = now.saturating_duration_since(since);
        if waited < INSPECTOR_READ_DELAY {
            return Some(INSPECTOR_READ_DELAY - waited);
        }
        match worker.read_metadata(path) {
            Ok(id) => {
                inspector.in_flight = Some(id);
                inspector.needs_read = false;
                Some(INSPECTOR_POLL_INTERVAL)
            }
            Err(SubmitError::QueueFull) => Some(INSPECTOR_POLL_INTERVAL),
            Err(error) => {
                inspector.result = Some(Err(submit_error(error)));
                inspector.needs_read = false;
                Some(Duration::ZERO)
            }
        }
    }

    /// What the inspector's Metadata section shows for `image` (absolute path).
    pub fn inspector_state(&self, image: &Path) -> InspectorState<'_> {
        if !supports_rating(image) {
            return InspectorState::Unsupported;
        }
        if !self.inspector.is_image(image) {
            return InspectorState::Loading;
        }
        match &self.inspector.result {
            None => InspectorState::Loading,
            Some(Ok(read)) => InspectorState::Ready(read),
            Some(Err(error)) => InspectorState::Error(error),
        }
    }

    /// Sends the editor's edits of `path` (absolute) to the worker (T105). An error is the text
    /// the window shows: the write could not be sent. Nothing is sent while a write runs.
    pub fn save_edits(
        &mut self,
        path: &Path,
        edits: Vec<Edit>,
        worker: &mut impl MetadataSubmit,
    ) -> Result<(), String> {
        if self.editor.writing.is_some() || self.editor.batch.is_some() {
            return Ok(());
        }
        match worker.write_edits(path.to_path_buf(), edits) {
            Ok(id) => {
                self.editor.writing = Some((id, path.to_path_buf()));
                Ok(())
            }
            Err(error) => Err(save_failed_text(path, submit_error(error))),
        }
    }

    /// Whether the editor's write is in flight.
    pub fn editor_saving(&self) -> bool {
        self.editor.writing.is_some()
    }

    /// Starts "Apply to All Marked" (ADR 0017 §2.3.1, U12c-Q5, T126): `this_image` written to the
    /// window's image (absolute) and each of `others` (absolute path, edits) to its image, each
    /// one its own `write_edits` (ADR 0007 §9.5), sent as the queue takes them, the window's image
    /// first. A write that cannot be sent fails that image only. When every write has answered,
    /// `EditorEvent::BatchDone` is given and the window's image is read again. False, and nothing
    /// sent, while the editor's write or another batch runs.
    pub fn start_batch(
        &mut self,
        window_image: &Path,
        this_image: Vec<Edit>,
        others: Vec<(PathBuf, Vec<Edit>)>,
        worker: &mut impl MetadataSubmit,
    ) -> bool {
        if self.editor.writing.is_some() || self.editor.batch.is_some() {
            return false;
        }
        let queue: VecDeque<(PathBuf, Vec<Edit>)> =
            std::iter::once((window_image.to_path_buf(), this_image))
                .chain(others)
                .collect();
        self.editor.batch = Some(EditorBatch {
            window_image: window_image.to_path_buf(),
            total: queue.len(),
            queue,
            in_flight: Vec::new(),
            written: 0,
            failed: Vec::new(),
            kept: Vec::new(),
            notices: Vec::new(),
        });
        self.send_batch(worker);
        true
    }

    /// Whether a batch of "Apply to All Marked" runs.
    pub fn batch_running(&self) -> bool {
        self.editor.batch.is_some()
    }

    /// The progress of the running batch for "Writing {i} of {n} …": the write being waited for
    /// (the answered ones plus one, at most the total) and the total.
    pub fn batch_progress(&self) -> Option<(usize, usize)> {
        self.editor
            .batch
            .as_ref()
            .map(|batch| ((batch.answered() + 1).min(batch.total), batch.total))
    }

    /// Sends the batch's writes not yet sent until the queue is full; a write the worker refuses
    /// otherwise fails its image. Ends the batch when every write has answered.
    fn send_batch(&mut self, worker: &mut impl MetadataSubmit) {
        let Some(batch) = self.editor.batch.as_mut() else {
            return;
        };
        while let Some((path, edits)) = batch.queue.front() {
            match worker.write_edits(path.clone(), edits.clone()) {
                Ok(id) => {
                    batch.in_flight.push((id, path.clone()));
                    batch.queue.pop_front();
                }
                Err(SubmitError::QueueFull) => break,
                Err(error) => {
                    let message = save_failed_text(path, submit_error(error));
                    batch.failed.push(message);
                    batch.queue.pop_front();
                }
            }
        }
        self.finish_batch();
    }

    /// Ends the batch when every write has answered: its summary is given and the window's image
    /// is read again (as after OK).
    fn finish_batch(&mut self) {
        let done = self
            .editor
            .batch
            .as_ref()
            .is_some_and(|batch| batch.queue.is_empty() && batch.in_flight.is_empty());
        if !done {
            return;
        }
        let Some(batch) = self.editor.batch.take() else {
            return;
        };
        self.editor.reload = Some(batch.window_image);
        self.editor
            .events
            .push(EditorEvent::BatchDone(BatchSummary {
                written: batch.written,
                total: batch.total,
                failed: batch.failed,
                kept: batch.kept,
                notices: batch.notices,
            }));
    }

    /// One of the batch's writes answered (T126): counted as written or failed; an unresolved
    /// conflict kept or a list with qualifiers not written (T194) in another image than the
    /// window's is listed.
    fn batch_written(
        &mut self,
        id: RequestId,
        path: &Path,
        result: Result<pw_metadata::edit::EditReport, WriteError>,
    ) {
        let Some(batch) = self.editor.batch.as_mut() else {
            return;
        };
        let Some(index) = batch.in_flight.iter().position(|(sent, _)| *sent == id) else {
            return;
        };
        batch.in_flight.remove(index);
        match result {
            Ok(report) => {
                batch.written += 1;
                if path == batch.window_image {
                    batch.notices = report.notices;
                } else {
                    for notice in &report.notices {
                        match notice {
                            WriteNotice::ConflictKept { property } => {
                                batch.kept.push(editor_text(&EditorText::ConflictKept {
                                    file: file_name(path),
                                    property: property.clone(),
                                }));
                            }
                            WriteNotice::QualifiersKept { property } => {
                                batch.kept.push(editor_text(&EditorText::QualifiersKept {
                                    file: file_name(path),
                                    property: property.clone(),
                                }));
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(error) => batch
                .failed
                .push(save_failed_text(path, write_error_text(error))),
        }
        self.finish_batch();
    }

    /// The answers of the metadata writes since the last call, in the order they came, so the
    /// inspector's kept histograms follow RawStride's own writes (U7c-Q7, T145).
    pub fn take_written(&mut self) -> Vec<WriteAnswer> {
        std::mem::take(&mut self.written)
    }

    /// The answers of the moves and the moves back since the last call, in the order they came
    /// (ADR 0017 U9b, T173).
    pub(crate) fn take_moves(&mut self) -> Vec<MoveAnswer> {
        std::mem::take(&mut self.moves)
    }

    /// The answers of the delete requests since the last call, in the order they came (ADR 0018
    /// B3, T188).
    pub(crate) fn take_deletes(&mut self) -> Vec<DeleteAnswer> {
        std::mem::take(&mut self.deletes)
    }

    /// Forgets `files` (absolute), which were sent to the Recycle Bin (ADR 0018 §2.7, T188):
    /// they leave the list of loaded images, the inspector's image is cleared when it is one of
    /// them, the rating store drops them, and a folder read waiting to be sent again no longer
    /// asks for them. A rating write still queued or sent is kept, as by `prune`; its answer for
    /// the missing file fails and is shown as any failed rating write.
    pub(crate) fn forget_deleted(&mut self, files: &[PathBuf]) {
        let deleted = |path: &Path| files.iter().any(|file| file == path);
        self.images.retain(|path, _| !deleted(path));
        if let Some(pending) = self.pending_read.as_mut() {
            pending.retain(|path| !deleted(path));
            if pending.is_empty() {
                self.pending_read = None;
            }
        }
        self.prune(|path| !deleted(path));
    }

    /// Whether a rating write for `path` (absolute) is queued, or sent and not yet answered; a
    /// move of the image waits for it (T172, T173, T178).
    pub(crate) fn is_pending(&self, path: &Path) -> bool {
        self.ratings.is_pending(path)
    }

    /// The rating a key would start from for `path` (absolute), which the undo of a move restores
    /// (ADR 0017 U9-Q8, T172, T173).
    pub(crate) fn current_rating(&self, path: &Path) -> Rating {
        self.ratings.current_rating(path)
    }

    /// Shows and queues exactly `rating` for `path`, for the undo of a move (ADR 0017 U9-Q8,
    /// T172, T173).
    pub(crate) fn restore_rating(
        &mut self,
        path: &Path,
        rating: Rating,
    ) -> Result<(), RatingRefused> {
        self.ratings.restore(path, rating)
    }

    /// Whether `path` (absolute) is in the list of loaded images. A moved image stays in it until
    /// the tab is loaded again, so after an undo in the same tab its rating is still known (T173).
    pub(crate) fn has_image(&self, path: &Path) -> bool {
        self.images.contains_key(path)
    }

    /// Whether the read after the editor's write is waiting or in flight.
    pub fn editor_reloading(&self) -> bool {
        self.editor.reload.is_some() || self.editor.reading.is_some()
    }

    /// What happened to the editor's write since the last call.
    pub fn take_editor_events(&mut self) -> Vec<EditorEvent> {
        std::mem::take(&mut self.editor.events)
    }

    /// Sends the read after an editor write that succeeded, once the queue takes it.
    fn send_editor_reload(&mut self, worker: &mut impl MetadataSubmit) {
        if self.editor.reading.is_some() {
            return;
        }
        let Some(path) = self.editor.reload.clone() else {
            return;
        };
        match worker.read_metadata(path.clone()) {
            Ok(id) => {
                self.editor.reading = Some((id, path));
                self.editor.reload = None;
            }
            Err(SubmitError::QueueFull) => {}
            Err(error) => {
                self.editor.reload = None;
                self.editor.events.push(EditorEvent::Reloaded {
                    path,
                    result: Err(submit_error(error)),
                });
            }
        }
    }

    /// RawStride wrote to `path`: the window reads it again if it is the window's image. The
    /// values read before stay shown until the new ones come back.
    fn window_written(&mut self, path: &Path) {
        if self.window.is_image(path) {
            self.window.needs_read = true;
        }
    }

    /// RawStride wrote to `path`: the inspector reads it again if it is the inspector's image
    /// (T110). The values read before stay shown until the new ones come back.
    fn inspector_written(&mut self, path: &Path) {
        if self.inspector.is_image(path) {
            self.inspector.needs_read = true;
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
                self.window_written(&path);
                self.inspector_written(&path);
                self.written.push(WriteAnswer {
                    path: path.clone(),
                    succeeded: result.is_ok(),
                });
                if let Some(message) =
                    self.ratings
                        .written(id, &path, result.map_err(write_error_text))
                {
                    failures.push(message);
                }
            }
            WorkerEvent::RatingsRead { .. } => {}
            WorkerEvent::MetadataRead { id, path, result }
                if self
                    .editor
                    .reading
                    .as_ref()
                    .is_some_and(|(reading, _)| *reading == id) =>
            {
                self.editor.reading = None;
                self.editor
                    .events
                    .push(EditorEvent::Reloaded { path, result });
            }
            WorkerEvent::MetadataRead { id, path, result } => {
                if self.window.in_flight == Some(id) {
                    self.window.in_flight = None;
                    // A result for another image than the window's is dropped.
                    if self.window.is_image(&path) {
                        self.window.result = Some(result);
                    }
                } else if self.inspector.in_flight == Some(id) {
                    self.inspector.in_flight = None;
                    // A result for another image than the inspector's is dropped.
                    if self.inspector.is_image(&path) {
                        self.inspector.result = Some(result);
                        self.inspector.arrived = true;
                    }
                }
            }
            // The editor's write (T105): the window reads the image again after it, and after one
            // that succeeded the editor's form is filled from a new read.
            WorkerEvent::EditsWritten { id, path, result } => {
                self.window_written(&path);
                self.inspector_written(&path);
                self.written.push(WriteAnswer {
                    path: path.clone(),
                    succeeded: result.is_ok(),
                });
                if self
                    .editor
                    .batch
                    .as_ref()
                    .is_some_and(|batch| batch.in_flight.iter().any(|(sent, _)| *sent == id))
                {
                    self.batch_written(id, &path, result);
                } else if self
                    .editor
                    .writing
                    .as_ref()
                    .is_some_and(|(writing, _)| *writing == id)
                {
                    self.editor.writing = None;
                    match result {
                        Ok(report) => {
                            self.editor.reload = Some(path.clone());
                            self.editor.events.push(EditorEvent::Saved {
                                path,
                                notices: report.notices,
                            });
                        }
                        Err(error) => {
                            let message = save_failed_text(&path, write_error_text(error));
                            self.editor
                                .events
                                .push(EditorEvent::Failed { path, message });
                        }
                    }
                }
            }
            // The moves (T170): `main.rs` takes their answers with `take_moves` (T173).
            WorkerEvent::Moved { id, image, result } => {
                self.moves.push(MoveAnswer::Moved { id, image, result });
            }
            WorkerEvent::MovedBack { id, moved, result } => {
                self.moves.push(MoveAnswer::MovedBack { id, moved, result });
            }
            // The deletes (T186): `main.rs` takes their answers with `take_deletes` (T188, T189).
            WorkerEvent::DeletePrepared { id, images } => {
                self.deletes.push(DeleteAnswer::Prepared { id, images });
            }
            WorkerEvent::Deleted { id, image, result } => {
                self.deletes
                    .push(DeleteAnswer::Deleted { id, image, result });
            }
        }
    }
}

/// The longest wait for rating writes, the editor's write and the batch's writes when the program
/// exits (ADR 0013 Q6, T105, T126).
pub(crate) const EXIT_WAIT_LIMIT: Duration = Duration::from_secs(5);
/// The pause between two polls while waiting at exit.
pub(crate) const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The most lines listed in the exit message box before "… och {n} till".
const UNSAVED_LIST_LIMIT: usize = 20;
/// What is left after waiting for the rating writes and the editor's write at exit.
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

/// Keeps sending and confirming rating writes until none is pending, and the editor's write and
/// the batch of "Apply to All Marked" are done, or `EXIT_WAIT_LIMIT` has passed (T44, T105,
/// T126). The clock and the sleep are passed in so the wait can be tested without real time.
pub(crate) fn finish_pending_writes(
    cache: &mut MetadataCache,
    worker: &mut impl MetadataSubmit,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> UnsavedRatings {
    let mut unsaved = UnsavedRatings::default();
    let waiting = |cache: &MetadataCache| {
        cache.pending_writes() > 0 || cache.editor_saving() || cache.batch_running()
    };
    if !waiting(cache) {
        return unsaved;
    }
    let start = now();
    loop {
        unsaved.failed.extend(cache.poll(worker));
        for event in cache.take_editor_events() {
            match event {
                EditorEvent::Failed { message, .. } => unsaved.failed.push(message),
                EditorEvent::BatchDone(summary) => unsaved.failed.extend(summary.failed),
                EditorEvent::Saved { .. } | EditorEvent::Reloaded { .. } => {}
            }
        }
        if !waiting(cache) || now().saturating_duration_since(start) >= EXIT_WAIT_LIMIT {
            break;
        }
        sleep(EXIT_POLL_INTERVAL);
    }
    let editor = cache.editor.writing.as_ref().map(|(_, path)| path);
    unsaved.pending = cache
        .ratings
        .pending_paths()
        .iter()
        .chain(editor)
        .map(|path| file_name(path))
        .collect();
    if let Some(batch) = &cache.editor.batch {
        unsaved
            .pending
            .extend(batch.pending_paths().map(|path| file_name(path)));
    }
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
        let _ = write!(text, "\n{}", editor_text(&EditorText::MoreUnsaved { more }));
    }
    text
}

/// The reason in the message for a failed rating write (its texts are in `editor.rs`, T116).
/// Other errors keep their own text.
fn write_error_text(error: WriteError) -> String {
    let reason = match error {
        WriteError::ReadOnly => WriteReason::ReadOnly,
        WriteError::Locked => WriteReason::Locked,
        WriteError::NotWritten(_) => WriteReason::NotWritten,
        WriteError::NoJournal => WriteReason::NoJournal,
        // The metadata worker refuses rating writes while ExifTool is too old or its version
        // could not be read (ADR 0007 §11, D9, T101).
        WriteError::ExifToolTooOld => WriteReason::ExifToolTooOld,
        WriteError::Other(text) => return text,
        WriteError::NotSafe { original, .. } => match original {
            OriginalState::Intact => WriteReason::NotSafeIntact,
            OriginalState::AtBackup(backup) => {
                let name = backup.file_name().map_or_else(
                    || backup.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                WriteReason::NotSafeAtBackup { name }
            }
            OriginalState::Replaced => WriteReason::NotSafeReplaced,
        },
    };
    write_reason_text(&reason)
}

/// The file name of `path`, or the whole path when it has none.
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// "Could not save metadata for <file>: <reason>" (T105).
fn save_failed_text(path: &Path, reason: String) -> String {
    editor_text(&EditorText::SaveFailed {
        file: file_name(path),
        reason,
    })
}

fn submit_error(error: SubmitError) -> String {
    match error {
        SubmitError::QueueFull => write_reason_text(&WriteReason::QueueFull),
        SubmitError::Stopped => write_reason_text(&WriteReason::WorkerStopped),
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

    /// One `move_image` call of `FakeSubmit`: its id, image, target, image extensions and
    /// suffixes (T173).
    type MoveImageCall = (RequestId, PathBuf, PathBuf, Vec<String>, Vec<String>);

    /// One `prepare_delete` call of `FakeSubmit`: its id, images, image extensions and suffixes
    /// (T188).
    type PrepareDeleteCall = (RequestId, Vec<PathBuf>, Vec<String>, Vec<String>);

    /// One `delete_image` call of `FakeSubmit`: its id, image, image extensions and suffixes
    /// (T188).
    type DeleteImageCall = (RequestId, PathBuf, Vec<String>, Vec<String>);

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
        /// The image and edits of every `write_edits` call, with the id each was given.
        edits_writes: Vec<(RequestId, PathBuf, Vec<Edit>)>,
        next_edits_id: RequestId,
        /// When set, `write_edits` answers `QueueFull` once this many writes were accepted (T126).
        edits_capacity: Option<usize>,
        /// When true, `write_edits` answers `Stopped` (T126).
        edits_stopped: bool,
        /// The image, target, image extensions and suffixes of every `move_image` call, with the
        /// id each was given (T173).
        move_images: Vec<MoveImageCall>,
        /// The files of every `move_back` call, with the id each was given (T173).
        move_backs: Vec<(RequestId, Vec<Moved>)>,
        next_move_id: RequestId,
        /// Every `prepare_delete` call, with the id each was given (T188).
        prepare_deletes: Vec<PrepareDeleteCall>,
        /// Every `delete_image` call, with the id each was given (T188).
        delete_images: Vec<DeleteImageCall>,
        next_delete_id: RequestId,
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
                edits_writes: Vec::new(),
                next_edits_id: 900,
                edits_capacity: None,
                edits_stopped: false,
                move_images: Vec::new(),
                move_backs: Vec::new(),
                next_move_id: 1300,
                prepare_deletes: Vec::new(),
                delete_images: Vec::new(),
                next_delete_id: 1700,
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

        fn write_edits(
            &mut self,
            image: PathBuf,
            edits: Vec<Edit>,
        ) -> Result<RequestId, SubmitError> {
            if self.edits_stopped {
                return Err(SubmitError::Stopped);
            }
            if self
                .edits_capacity
                .is_some_and(|capacity| self.edits_writes.len() >= capacity)
            {
                return Err(SubmitError::QueueFull);
            }
            self.operations.push("write_edits");
            let id = self.next_edits_id;
            self.next_edits_id += 1;
            self.edits_writes.push((id, image, edits));
            Ok(id)
        }

        fn move_image(
            &mut self,
            image: PathBuf,
            target: PathBuf,
            image_extensions: Vec<String>,
            suffixes: Vec<String>,
        ) -> Result<RequestId, SubmitError> {
            self.operations.push("move_image");
            let id = self.next_move_id;
            self.next_move_id += 1;
            self.move_images
                .push((id, image, target, image_extensions, suffixes));
            Ok(id)
        }

        fn move_back(&mut self, moved: Vec<Moved>) -> Result<RequestId, SubmitError> {
            self.operations.push("move_back");
            let id = self.next_move_id;
            self.next_move_id += 1;
            self.move_backs.push((id, moved));
            Ok(id)
        }

        fn prepare_delete(
            &mut self,
            images: Vec<PathBuf>,
            image_extensions: Vec<String>,
            suffixes: Vec<String>,
        ) -> Result<RequestId, SubmitError> {
            self.operations.push("prepare_delete");
            let id = self.next_delete_id;
            self.next_delete_id += 1;
            self.prepare_deletes
                .push((id, images, image_extensions, suffixes));
            Ok(id)
        }

        fn delete_image(
            &mut self,
            image: PathBuf,
            image_extensions: Vec<String>,
            suffixes: Vec<String>,
        ) -> Result<RequestId, SubmitError> {
            self.operations.push("delete_image");
            let id = self.next_delete_id;
            self.next_delete_id += 1;
            self.delete_images
                .push((id, image, image_extensions, suffixes));
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
            Some("Reading metadata…")
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
    fn disconnected_worker_turns_loading_into_an_english_error() {
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
            Some("Metadata error: metadata writing has stopped")
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
    fn rating_summary_uses_the_english_rating_text() {
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
            Some("Rating: 3 stars, Red")
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
            Some("Reading metadata…")
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
            Some("Metadata error: metadata writing has stopped")
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
            ["Could not save the rating for pw-loupe-exit-e.ARW: skrivskyddad"]
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
        assert_eq!(shown.last(), Some(&"… and 3 more"));
        assert_eq!(
            unsaved_list_text(&lines[..20]).lines().count(),
            20,
            "exactly twenty lines need no count"
        );
    }

    #[test]
    fn write_error_texts_are_english_for_read_only_and_refused_writes() {
        assert_eq!(
            write_error_text(WriteError::ReadOnly),
            "the file is read-only"
        );
        assert_eq!(
            write_error_text(WriteError::NotWritten(
                "0 image files updated\n1 files weren't updated due to errors".into()
            )),
            "the file could not be written"
        );
        assert_eq!(
            write_error_text(WriteError::Other("ExifTool exited unexpectedly".into())),
            "ExifTool exited unexpectedly"
        );
    }

    #[test]
    fn write_error_texts_are_english_for_writes_that_were_not_safe() {
        assert_eq!(
            write_error_text(WriteError::NotSafe {
                stage: Stage::ImageCheck,
                original: OriginalState::Intact,
                reason: "preview 2 differs".into(),
            }),
            "the metadata could not be written safely; the file is unchanged"
        );
        let backup = std::env::temp_dir().join("bild.pwbak-0123.jpg");
        assert_eq!(
            write_error_text(WriteError::NotSafe {
                stage: Stage::Replace,
                original: OriginalState::AtBackup(backup),
                reason: "access denied".into(),
            }),
            "the metadata could not be written safely; the original is now bild.pwbak-0123.jpg"
        );
        assert_eq!(
            write_error_text(WriteError::NotSafe {
                stage: Stage::FinalCheck,
                original: OriginalState::Replaced,
                reason: "Rating is 2".into(),
            }),
            "the file was written but the final check failed; the earlier version is kept as a \
             backup"
        );
    }

    #[test]
    fn a_write_to_a_file_open_in_another_program_gives_the_reason() {
        assert_eq!(
            write_error_text(WriteError::Locked),
            "the file is open in another program"
        );
        // The editor's message (T107's window).
        assert_eq!(
            save_failed_text(
                Path::new("konflikt-stad.jpg"),
                write_error_text(WriteError::Locked)
            ),
            "Could not save metadata for konflikt-stad.jpg: the file is open in another program"
        );
    }

    #[test]
    fn a_locked_rating_write_gives_the_reason_through_poll() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = cache_with_writes(&["pw-loupe-locked.JPG"]);
        assert!(cache.poll(&mut worker).is_empty(), "the first poll sends");
        assert_eq!(worker.write_attempts.len(), 1);
        let id = worker.next_write_id - 1;
        worker.events.push_back(WorkerEvent::RatingWritten {
            id,
            path: std::env::temp_dir().join("pw-loupe-locked.JPG"),
            rating: Rating {
                stars: Some(2),
                label: None,
            },
            result: Err(WriteError::Locked),
        });

        let failures = cache.poll(&mut worker);

        assert_eq!(
            failures,
            [
                "Could not save the rating for pw-loupe-locked.JPG: the file is open in another program"
            ]
        );
        assert_eq!(cache.pending_writes(), 0);
    }

    #[test]
    fn a_write_without_the_journal_gives_the_english_text() {
        assert_eq!(
            write_error_text(WriteError::NoJournal),
            "the write log could not be opened"
        );
    }

    #[test]
    fn a_write_refused_for_an_old_exiftool_gives_the_english_text() {
        assert_eq!(
            write_error_text(WriteError::ExifToolTooOld),
            "ExifTool is too old (at least 12.67 is needed)"
        );
    }

    #[test]
    fn a_read_only_write_result_gives_the_english_message_through_poll() {
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
            ["Could not save the rating for pw-loupe-read-only.ARW: the file is read-only"]
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
                "Could not save the rating for 2 images. Last: pw-loupe-cache-batch-c.DNG: skrivskyddad"
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
            Some("metadata writing has stopped")
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
            Some("metadata writing has stopped")
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

    // --- the metadata window's read (ADR 0007 I10n, D16, T107) ---

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

    fn image_path(name: &str) -> PathBuf {
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
    fn opening_the_window_sends_its_read_at_once() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-window-open.ARW");

        // No window: polling asks for nothing.
        assert!(cache.poll(&mut worker).is_empty());
        assert!(!cache.window_waiting());
        assert!(worker.metadata_reads.is_empty());

        // Opened: the read is sent in the same call, without a wait.
        assert!(cache.open_window(&image, &mut worker));
        assert_eq!(worker.metadata_reads, [(500, image.clone())]);
        assert!(cache.window_waiting());
        assert_eq!(cache.take_window_read(), None, "nothing is back yet");

        // Polling while the read runs sends nothing more.
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.metadata_reads.len(), 1);

        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert!(!cache.window_waiting());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));
        assert_eq!(
            cache.take_window_read(),
            None,
            "a result is handed over once"
        );

        // A read image is not read again.
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.metadata_reads.len(), 1);
    }

    #[test]
    fn an_unsupported_file_opens_no_window_and_is_never_read() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let text = image_path("pw-loupe-window-notes.txt");

        assert!(!cache.open_window(&text, &mut worker));
        assert!(cache.poll(&mut worker).is_empty());

        assert!(worker.metadata_reads.is_empty());
        assert!(!cache.window_waiting());
        assert_eq!(cache.take_window_read(), None);
    }

    #[test]
    fn one_read_in_flight_and_a_stale_result_is_dropped() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let first = image_path("pw-loupe-window-first.ARW");
        let second = image_path("pw-loupe-window-second.ARW");
        assert!(cache.open_window(&first, &mut worker));
        assert_eq!(worker.metadata_reads, [(500, first.clone())]);

        // Closed and opened on another image while the first read runs: nothing more is sent.
        cache.close_window();
        assert!(cache.open_window(&second, &mut worker));
        assert_eq!(worker.metadata_reads.len(), 1, "one read in flight");
        assert!(cache.window_waiting());

        // The first image's result comes back: it is dropped, and the second image is read.
        worker
            .events
            .push_back(metadata_read(500, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), None);
        assert_eq!(
            worker.metadata_reads,
            [(500, first.clone()), (501, second.clone())]
        );

        // Its result is kept.
        worker
            .events
            .push_back(metadata_read(501, &second, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));
    }

    #[test]
    fn a_closed_window_drops_its_result() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-window-closed.jpg");
        assert!(cache.open_window(&image, &mut worker));
        cache.close_window();
        assert!(!cache.window_waiting());

        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());

        assert_eq!(cache.take_window_read(), None);
        assert_eq!(worker.metadata_reads.len(), 1, "nothing is read again");
    }

    #[test]
    fn a_result_with_another_id_is_dropped_and_an_error_is_handed_over() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-window-error.jpg");
        assert!(cache.open_window(&image, &mut worker));

        worker
            .events
            .push_back(metadata_read(77, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), None);

        worker
            .events
            .push_back(metadata_read(500, &image, Err("filen saknas".to_owned())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_window_read(),
            Some(Err("filen saknas".to_owned()))
        );
    }

    #[test]
    fn a_stopped_worker_fails_the_windows_read() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-window-stopped.DNG");
        assert!(cache.open_window(&image, &mut worker));
        let mut failures = Vec::new();

        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);

        assert!(!cache.window_waiting());
        assert_eq!(
            cache.take_window_read(),
            Some(Err("metadata writing has stopped".to_owned()))
        );
    }

    #[test]
    fn the_window_reads_its_image_again_after_a_rating_write() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Ok(()));
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-window-rated.ARW");
        let other = image_path("pw-loupe-window-other.ARW");
        assert!(cache.open_window(&image, &mut worker));
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));

        // A rating write of another image changes nothing.
        if let Err(error) = cache.set_rating(&other, RatingKey::Stars(2)) {
            panic!("supported rating path should be accepted: {error}");
        }
        assert!(cache.poll(&mut worker).is_empty(), "the write is sent");
        assert!(cache.poll(&mut worker).is_empty(), "the write is confirmed");
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.metadata_reads.len(), 1);

        // A rating write of the window's image: read again.
        if let Err(error) = cache.set_rating(&image, RatingKey::Stars(4)) {
            panic!("supported rating path should be accepted: {error}");
        }
        assert!(cache.poll(&mut worker).is_empty(), "the write is sent");
        assert!(cache.poll(&mut worker).is_empty(), "the write is confirmed");
        assert_eq!(
            worker.metadata_reads,
            [(500, image.clone()), (501, image.clone())]
        );
    }

    #[test]
    fn a_folder_load_keeps_the_windows_image() {
        let mut worker = FakeSubmit::new([Ok(1), Ok(2)]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-window-folder.ARW");
        if let Err(error) = cache.begin_load(vec![image.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert!(cache.open_window(&image, &mut worker));

        // The window is locked to its image: another folder loaded meanwhile does not drop it.
        let elsewhere = image_path("pw-loupe-window-elsewhere.ARW");
        if let Err(error) = cache.begin_load(vec![elsewhere], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));
    }

    // --- moving the window to another image (ADR 0017 §2.3.1, T112) ---

    #[test]
    fn moving_the_window_asks_for_the_new_images_read_at_once() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let first = image_path("pw-loupe-move-first.jpg");
        let second = image_path("pw-loupe-move-second.ARW");

        // No window open: nothing is moved or read.
        assert!(!cache.move_window(&second, &mut worker));
        assert!(worker.metadata_reads.is_empty());
        assert!(!cache.window_waiting());

        assert!(cache.open_window(&first, &mut worker));
        worker
            .events
            .push_back(metadata_read(500, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));

        // Moved: the new image's read is sent in the same call, without a wait.
        assert!(cache.move_window(&second, &mut worker));
        assert_eq!(
            worker.metadata_reads,
            [(500, first.clone()), (501, second.clone())]
        );
        assert!(cache.window_waiting());
        assert_eq!(cache.take_window_read(), None, "nothing is back yet");

        worker
            .events
            .push_back(metadata_read(501, &second, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));

        // A kind without metadata support: no move, the window keeps its image.
        let text = image_path("pw-loupe-move-notes.txt");
        assert!(!cache.move_window(&text, &mut worker));
        assert_eq!(worker.metadata_reads.len(), 2);
        assert!(cache.window.is_image(&second));
    }

    #[test]
    fn moving_the_window_drops_a_result_for_the_earlier_image() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let first = image_path("pw-loupe-move-stale-first.jpg");
        let second = image_path("pw-loupe-move-stale-second.jpg");
        assert!(cache.open_window(&first, &mut worker));

        // Moved while the first read runs: one read in flight, nothing more is sent.
        assert!(cache.move_window(&second, &mut worker));
        assert_eq!(worker.metadata_reads, [(500, first.clone())]);
        assert!(cache.window_waiting());

        // The first image's result is dropped, and the second image is read.
        worker
            .events
            .push_back(metadata_read(500, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), None);
        assert_eq!(
            worker.metadata_reads,
            [(500, first.clone()), (501, second.clone())]
        );

        worker
            .events
            .push_back(metadata_read(501, &second, Err("filen saknas".to_owned())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_window_read(),
            Some(Err("filen saknas".to_owned()))
        );
    }

    #[test]
    fn the_write_of_the_earlier_image_still_reads_that_image_back_after_a_move() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let first = image_path("pw-loupe-move-write-first.jpg");
        let second = image_path("pw-loupe-move-write-second.ARW");
        assert!(cache.open_window(&first, &mut worker));
        worker
            .events
            .push_back(metadata_read(500, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));

        // "Spara och nästa": the first image's edits are written, then the window moves.
        assert_eq!(cache.save_edits(&first, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(900, &first, Ok(report(Vec::new()))));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Saved {
                path: first.clone(),
                notices: Vec::new(),
            }]
        );
        // The write asked for the read after it (501) and, as the window still showed the first
        // image, the window's new read of it (502).
        assert_eq!(
            worker.metadata_reads,
            [
                (500, first.clone()),
                (501, first.clone()),
                (502, first.clone())
            ]
        );
        assert!(cache.move_window(&second, &mut worker));
        assert_eq!(worker.metadata_reads.len(), 3, "one window read in flight");

        // The read after the write still reads the first image back for the editor; the window's
        // result for the first image is dropped, and the second image is read.
        worker
            .events
            .push_back(metadata_read(501, &first, Ok(empty_read())));
        worker
            .events
            .push_back(metadata_read(502, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Reloaded {
                path: first.clone(),
                result: Ok(empty_read()),
            }]
        );
        assert!(!cache.editor_reloading());
        assert_eq!(cache.take_window_read(), None);
        assert_eq!(
            worker.metadata_reads,
            [
                (500, first.clone()),
                (501, first.clone()),
                (502, first),
                (503, second.clone())
            ]
        );
        worker
            .events
            .push_back(metadata_read(503, &second, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));
    }

    // --- the editor's write (ADR 0007 I10l, D12, T105) ---

    /// The edits of a form of `empty_read` with a title typed in.
    fn title_edits() -> Vec<Edit> {
        let mut form = pw_loupe::EditorForm::from_read(&empty_read());
        match form.text_mut(pw_loupe::EditorField::Title) {
            Some(text) => *text = "Solnedgång".to_owned(),
            None => panic!("the form has a title field"),
        }
        match form.edits() {
            Ok(edits) => edits,
            Err(errors) => panic!("a title has no field error: {errors:?}"),
        }
    }

    fn edits_written(
        id: RequestId,
        path: &Path,
        result: Result<pw_metadata::edit::EditReport, WriteError>,
    ) -> WorkerEvent {
        WorkerEvent::EditsWritten {
            id,
            path: path.to_path_buf(),
            result,
        }
    }

    fn report(notices: Vec<WriteNotice>) -> pw_metadata::edit::EditReport {
        pw_metadata::edit::EditReport {
            report: pw_metadata::safe_write::WriteReport::Unchanged,
            notices,
        }
    }

    #[test]
    fn an_edits_request_is_sent_with_the_forms_edits() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-editor-send.jpg");
        let edits = title_edits();
        assert!(!edits.is_empty());

        assert_eq!(cache.save_edits(&image, edits.clone(), &mut worker), Ok(()));

        assert_eq!(worker.edits_writes, [(900, image.clone(), edits.clone())]);
        assert!(cache.editor_saving());
        // A second save while the write runs sends nothing.
        assert_eq!(cache.save_edits(&image, edits, &mut worker), Ok(()));
        assert_eq!(worker.edits_writes.len(), 1);
        assert!(cache.take_editor_events().is_empty());
    }

    #[test]
    fn edits_written_ok_asks_for_a_new_read_of_the_image() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-editor-ok.DNG");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker.events.push_back(edits_written(
            900,
            &image,
            Ok(report(vec![WriteNotice::DigestNotWritten])),
        ));

        assert!(cache.poll(&mut worker).is_empty());

        assert!(!cache.editor_saving());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Saved {
                path: image.clone(),
                notices: vec![WriteNotice::DigestNotWritten],
            }]
        );
        assert_eq!(worker.metadata_reads, [(500, image.clone())]);
        assert!(cache.editor_reloading());

        // The new read fills the form again.
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert!(!cache.editor_reloading());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Reloaded {
                path: image,
                result: Ok(empty_read()),
            }]
        );
    }

    #[test]
    fn edits_written_ok_also_renews_the_windows_read_of_the_image() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-editor-window.jpg");
        assert!(cache.open_window(&image, &mut worker));
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(900, &image, Ok(report(Vec::new()))));

        assert!(cache.poll(&mut worker).is_empty());

        // The window's first read, the read after the write (the form), the window's new read.
        assert_eq!(
            worker.metadata_reads,
            [
                (500, image.clone()),
                (501, image.clone()),
                (502, image.clone())
            ]
        );
    }

    #[test]
    fn edits_written_err_clears_the_pending_state_and_gives_the_english_text() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-editor-err.ARW");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(900, &image, Err(WriteError::ReadOnly)));

        assert!(cache.poll(&mut worker).is_empty());

        assert!(!cache.editor_saving());
        assert!(!cache.editor_reloading());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Failed {
                path: image,
                message: "Could not save metadata for pw-loupe-editor-err.ARW: the file is \
                          read-only"
                    .to_owned(),
            }]
        );
        assert!(worker.metadata_reads.is_empty(), "nothing is read again");
    }

    #[test]
    fn an_edits_result_with_another_id_is_not_the_editors() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-editor-other-id.jpg");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(17, &image, Err(WriteError::ReadOnly)));

        assert!(cache.poll(&mut worker).is_empty());

        assert!(cache.editor_saving());
        assert!(cache.take_editor_events().is_empty());
    }

    #[test]
    fn a_stopped_worker_fails_the_editors_write() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-editor-stopped.jpg");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        let mut failures = Vec::new();

        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);

        assert!(!cache.editor_saving());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Failed {
                path: image,
                message: "Could not save metadata for pw-loupe-editor-stopped.jpg: \
                          metadata writing has stopped"
                    .to_owned(),
            }]
        );
    }

    #[test]
    fn the_exit_wait_counts_a_running_edits_write() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-exit-editor.jpg");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        assert_eq!(cache.pending_writes(), 0, "no rating write is pending");
        let clock = std::cell::Cell::new(Instant::now());
        let start = clock.get();

        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
        );

        assert_eq!(
            clock.get() - start,
            EXIT_WAIT_LIMIT,
            "it waited the full limit"
        );
        assert!(unsaved.failed.is_empty());
        assert_eq!(unsaved.pending, ["pw-loupe-exit-editor.jpg"]);
    }

    #[test]
    fn the_exit_wait_ends_with_the_editors_write_and_lists_its_failure() {
        // Confirmed: nothing to list.
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-exit-editor-ok.jpg");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(900, &image, Ok(report(Vec::new()))));
        let clock = std::cell::Cell::new(Instant::now());
        let start = clock.get();
        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
        );
        assert_eq!(unsaved, UnsavedRatings::default());
        assert_eq!(clock.get(), start, "no pause was needed");

        // Failed: listed with its reason.
        let image = image_path("pw-loupe-exit-editor-err.jpg");
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(901, &image, Err(WriteError::ReadOnly)));
        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
        );
        assert!(unsaved.pending.is_empty());
        assert_eq!(
            unsaved.failed,
            ["Could not save metadata for pw-loupe-exit-editor-err.jpg: the file is read-only"]
        );
    }

    // --- the inspector's metadata (ADR 0017 §2.1, T110; the rules of T103) ---

    /// Makes `image` the inspector's image at `start` and sends its read at `start + 200 ms`;
    /// returns that time.
    fn inspector_read_sent(
        cache: &mut MetadataCache,
        image: &Path,
        start: Instant,
        worker: &mut FakeSubmit,
    ) -> Instant {
        cache.inspector_request(Some(image), true, false, start, worker);
        let t = start + INSPECTOR_READ_DELAY;
        cache.inspector_request(Some(image), true, false, t, worker);
        t
    }

    #[test]
    fn the_inspector_asks_for_nothing_before_200_ms_or_while_collapsed() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-wait.ARW");
        let start = Instant::now();

        // Collapsed (the column or the section): nothing, however long the image is current.
        assert_eq!(
            cache.inspector_request(Some(&image), false, false, start, &mut worker),
            None
        );
        let later = start + Duration::from_secs(5);
        assert_eq!(
            cache.inspector_request(Some(&image), false, false, later, &mut worker),
            None
        );
        assert!(worker.metadata_reads.is_empty());

        // Another image, shown: nothing before 200 ms, and the rest of the wait is returned.
        let other = image_path("pw-loupe-inspector-wait.jpg");
        assert_eq!(
            cache.inspector_request(Some(&other), true, false, later, &mut worker),
            Some(INSPECTOR_READ_DELAY)
        );
        let almost = later + Duration::from_millis(199);
        assert_eq!(
            cache.inspector_request(Some(&other), true, false, almost, &mut worker),
            Some(Duration::from_millis(1))
        );
        assert!(worker.metadata_reads.is_empty());
        assert_eq!(cache.inspector_state(&other), InspectorState::Loading);

        // At 200 ms the one read is sent.
        cache.inspector_request(
            Some(&other),
            true,
            false,
            later + INSPECTOR_READ_DELAY,
            &mut worker,
        );
        assert_eq!(worker.metadata_reads, [(500, other.clone())]);
    }

    #[test]
    fn moving_quickly_queues_nothing_and_an_unsupported_file_is_never_read_by_the_inspector() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let start = Instant::now();
        // A new image every 100 ms: none is current for 200 ms.
        for step in 0..5u32 {
            let image = image_path(&format!("pw-loupe-inspector-step-{step}.DNG"));
            let now = start + Duration::from_millis(100) * step;
            cache.inspector_request(Some(&image), true, false, now, &mut worker);
        }
        assert!(worker.metadata_reads.is_empty());

        let text = image_path("pw-loupe-inspector-notes.txt");
        cache.inspector_request(Some(&text), true, false, start, &mut worker);
        let later = start + Duration::from_secs(1);
        assert_eq!(
            cache.inspector_request(Some(&text), true, false, later, &mut worker),
            None
        );
        assert!(worker.metadata_reads.is_empty());
        assert_eq!(cache.inspector_state(&text), InspectorState::Unsupported);
    }

    #[test]
    fn the_inspector_has_one_read_in_flight_then_the_newest_image_and_drops_a_stale_result() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let first = image_path("pw-loupe-inspector-first.ARW");
        let second = image_path("pw-loupe-inspector-second.ARW");
        let third = image_path("pw-loupe-inspector-third.ARW");
        let start = Instant::now();
        inspector_read_sent(&mut cache, &first, start, &mut worker);
        assert_eq!(worker.metadata_reads, [(500, first.clone())]);

        // The image changes twice while the read runs: nothing more is sent.
        let t = start + Duration::from_millis(300);
        cache.inspector_request(Some(&second), true, false, t, &mut worker);
        let t = t + Duration::from_millis(300);
        assert_eq!(
            cache.inspector_request(Some(&third), true, false, t, &mut worker),
            Some(INSPECTOR_POLL_INTERVAL)
        );
        let t = t + Duration::from_secs(1);
        cache.inspector_request(Some(&third), true, false, t, &mut worker);
        assert_eq!(worker.metadata_reads.len(), 1, "one read in flight");

        // The first image's result comes back: it is dropped.
        worker
            .events
            .push_back(metadata_read(500, &first, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.inspector_state(&third), InspectorState::Loading);
        assert_eq!(cache.inspector_state(&first), InspectorState::Loading);

        // Then the newest image is asked for, and its result is kept.
        cache.inspector_request(Some(&third), true, false, t, &mut worker);
        assert_eq!(
            worker.metadata_reads,
            [(500, first.clone()), (501, third.clone())]
        );
        worker
            .events
            .push_back(metadata_read(501, &third, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.inspector_state(&third),
            InspectorState::Ready(&empty_read())
        );
        // one more frame at once, so the result is drawn
        assert_eq!(
            cache.inspector_request(Some(&third), true, false, t, &mut worker),
            Some(Duration::ZERO)
        );
        assert_eq!(
            cache.inspector_request(Some(&third), true, false, t, &mut worker),
            None
        );
        assert_eq!(
            worker.metadata_reads.len(),
            2,
            "a read image is not read again"
        );
    }

    /// The section is drawn before the worker's answers are taken in a frame, so the next
    /// request after the inspector's result came back (or the worker stopped) asks for one more
    /// frame at once, and only then; without it the result waited for the next input (found with
    /// the Histogram section, T144).
    #[test]
    fn the_inspector_asks_for_a_frame_at_once_when_its_result_comes() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-repaint.ARW");
        let t = inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        assert_eq!(
            cache.inspector_request(Some(&image), true, false, t, &mut worker),
            Some(INSPECTOR_POLL_INTERVAL)
        );
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.inspector_request(Some(&image), true, false, t, &mut worker),
            Some(Duration::ZERO)
        );
        assert_eq!(
            cache.inspector_request(Some(&image), true, false, t, &mut worker),
            None
        );

        // a result for another image (dropped) asks for nothing
        let stale = image_path("pw-loupe-inspector-repaint-stale.ARW");
        let newest = image_path("pw-loupe-inspector-repaint-newest.ARW");
        let t = inspector_read_sent(&mut cache, &stale, t, &mut worker);
        cache.inspector_request(Some(&newest), true, false, t, &mut worker);
        worker
            .events
            .push_back(metadata_read(501, &stale, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_ne!(
            cache.inspector_request(Some(&newest), true, false, t, &mut worker),
            Some(Duration::ZERO)
        );

        // collapsed: nothing is asked for
        let hidden = image_path("pw-loupe-inspector-repaint-hidden.ARW");
        let t = t + Duration::from_secs(1);
        let t = inspector_read_sent(&mut cache, &hidden, t, &mut worker);
        let Some(&(id, _)) = worker.metadata_reads.last() else {
            panic!("no read sent");
        };
        worker
            .events
            .push_back(metadata_read(id, &hidden, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.inspector_request(Some(&hidden), false, false, t, &mut worker),
            None
        );

        // the worker stops while the inspector's read is in flight
        let stopped = image_path("pw-loupe-inspector-repaint-stopped.ARW");
        let t = t + Duration::from_secs(1);
        let t = inspector_read_sent(&mut cache, &stopped, t, &mut worker);
        let mut failures = Vec::new();
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert_eq!(
            cache.inspector_request(Some(&stopped), true, false, t, &mut worker),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn the_inspector_shows_an_error_and_a_stopped_worker_fails_its_read() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-error.jpg");
        inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        worker
            .events
            .push_back(metadata_read(77, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.inspector_state(&image), InspectorState::Loading);
        worker
            .events
            .push_back(metadata_read(500, &image, Err("filen saknas".to_owned())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.inspector_state(&image),
            InspectorState::Error("filen saknas")
        );

        let other = image_path("pw-loupe-inspector-stopped.jpg");
        inspector_read_sent(&mut cache, &other, Instant::now(), &mut worker);
        let mut failures = Vec::new();
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert_eq!(
            cache.inspector_state(&other),
            InspectorState::Error("metadata writing has stopped")
        );
    }

    #[test]
    fn the_inspector_reads_its_image_again_after_a_rating_write() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Ok(()));
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-rated.ARW");
        let other = image_path("pw-loupe-inspector-other.ARW");
        let t = inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        // the frame after the result asks for one more at once, so it is drawn
        assert_eq!(
            cache.inspector_request(Some(&image), true, false, t, &mut worker),
            Some(Duration::ZERO)
        );

        // A rating write of another image changes nothing.
        if let Err(error) = cache.set_rating(&other, RatingKey::Stars(2)) {
            panic!("supported rating path should be accepted: {error}");
        }
        assert!(cache.poll(&mut worker).is_empty(), "the write is sent");
        assert!(cache.poll(&mut worker).is_empty(), "the write is confirmed");
        assert_eq!(
            cache.inspector_request(Some(&image), true, false, t, &mut worker),
            None
        );
        assert_eq!(worker.metadata_reads.len(), 1);

        // A rating write of the inspector's image: read again, the old values shown meanwhile.
        if let Err(error) = cache.set_rating(&image, RatingKey::Stars(4)) {
            panic!("supported rating path should be accepted: {error}");
        }
        assert!(cache.poll(&mut worker).is_empty(), "the write is sent");
        assert!(cache.poll(&mut worker).is_empty(), "the write is confirmed");
        assert_eq!(
            cache.inspector_state(&image),
            InspectorState::Ready(&empty_read())
        );
        cache.inspector_request(Some(&image), true, false, t, &mut worker);
        assert_eq!(
            worker.metadata_reads,
            [(500, image.clone()), (501, image.clone())]
        );
    }

    #[test]
    fn the_inspector_reads_again_after_the_windows_save_once_the_window_closes() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-saved.jpg");
        let t = inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        // the frame after the result asks for one more at once, so it is drawn
        assert_eq!(
            cache.inspector_request(Some(&image), true, false, t, &mut worker),
            Some(Duration::ZERO)
        );

        // The window opens on the image: its own read is sent at once, the inspector's is not.
        assert!(cache.open_window(&image, &mut worker));
        assert_eq!(worker.metadata_reads.len(), 2);
        worker
            .events
            .push_back(metadata_read(501, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_window_read(),
            Some(Ok(empty_read())),
            "the window's read"
        );

        // A save in the window: the editor's read after it; the inspector asks for nothing while
        // the window is open, and keeps showing its image even when asked about another.
        assert_eq!(cache.save_edits(&image, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(900, &image, Ok(report(Vec::new()))));
        assert!(cache.poll(&mut worker).is_empty());
        let reads_while_open = worker.metadata_reads.len();
        let elsewhere = image_path("pw-loupe-inspector-elsewhere.jpg");
        let later = t + Duration::from_secs(1);
        assert_eq!(
            cache.inspector_request(Some(&elsewhere), true, true, later, &mut worker),
            None
        );
        assert_eq!(
            cache.inspector_request(Some(&image), true, true, later, &mut worker),
            None
        );
        assert_eq!(worker.metadata_reads.len(), reads_while_open);
        assert_eq!(
            cache.inspector_state(&image),
            InspectorState::Ready(&empty_read())
        );

        // The window closes: the inspector reads its image again at once.
        cache.close_window();
        cache.inspector_request(Some(&image), true, false, later, &mut worker);
        assert_eq!(worker.metadata_reads.len(), reads_while_open + 1);
        assert_eq!(
            worker.metadata_reads.last().map(|(_, path)| path),
            Some(&image)
        );
    }

    #[test]
    fn the_windows_read_is_unaffected_by_the_inspectors() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let shown = image_path("pw-loupe-inspector-beside-a.ARW");
        let window = image_path("pw-loupe-inspector-beside-b.ARW");
        inspector_read_sent(&mut cache, &shown, Instant::now(), &mut worker);
        assert_eq!(worker.metadata_reads, [(500, shown.clone())]);

        // The window's read is sent at once although the inspector's is in flight.
        assert!(cache.open_window(&window, &mut worker));
        assert_eq!(
            worker.metadata_reads,
            [(500, shown.clone()), (501, window.clone())]
        );

        // Each result goes to its own reader.
        worker
            .events
            .push_back(metadata_read(501, &window, Err("fönstrets fel".to_owned())));
        worker
            .events
            .push_back(metadata_read(500, &shown, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_window_read(),
            Some(Err("fönstrets fel".to_owned()))
        );
        assert_eq!(
            cache.inspector_state(&shown),
            InspectorState::Ready(&empty_read())
        );
    }

    #[test]
    fn a_folder_change_empties_the_inspector_and_the_same_folder_keeps_it() {
        let mut worker = FakeSubmit::new([Ok(1), Ok(2), Ok(3)]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-folder.ARW");
        if let Err(error) = cache.begin_load(vec![image.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());

        // The same folder loaded again (contact sheet ↔ loupe) keeps the values.
        if let Err(error) = cache.begin_load(vec![image.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(
            cache.inspector_state(&image),
            InspectorState::Ready(&empty_read())
        );

        // Pruning keeps an image still open.
        cache.prune(|path| path == image);
        assert_eq!(
            cache.inspector_state(&image),
            InspectorState::Ready(&empty_read())
        );

        // Another folder empties them.
        let elsewhere = image_path("pw-loupe-inspector-elsewhere.ARW");
        if let Err(error) = cache.begin_load(vec![elsewhere], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        assert_eq!(cache.inspector_state(&image), InspectorState::Loading);
    }

    // --- "Apply to All Marked" (ADR 0017 §2.3.1, U12c, T126) ---

    /// The edits the other marked images get in these tests.
    fn headline_edits() -> Vec<Edit> {
        vec![Edit::Set {
            property: "photoshop:Headline".to_owned(),
            field: None,
            value: pw_metadata::value::Value::Text("Hav".to_owned()),
        }]
    }

    /// The three images of a batch: the window's JPEG, an ARW and a DNG.
    fn batch_images() -> (PathBuf, PathBuf, PathBuf) {
        (
            image_path("pw-loupe-batch-a.jpg"),
            image_path("pw-loupe-batch-b.ARW"),
            image_path("pw-loupe-batch-c.DNG"),
        )
    }

    fn start_three(
        cache: &mut MetadataCache,
        worker: &mut FakeSubmit,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let (a, b, c) = batch_images();
        let others = vec![(b.clone(), headline_edits()), (c.clone(), headline_edits())];
        assert!(cache.start_batch(&a, title_edits(), others, worker));
        (a, b, c)
    }

    #[test]
    fn a_batch_sends_one_write_per_file_as_the_queue_takes_them() {
        let mut worker = FakeSubmit::new([]);
        worker.edits_capacity = Some(2);
        let mut cache = MetadataCache::default();

        let (a, b, c) = start_three(&mut cache, &mut worker);

        // The window's image first with its own edits, then the others with the changed fields.
        assert_eq!(
            worker.edits_writes,
            [
                (900, a.clone(), title_edits()),
                (901, b.clone(), headline_edits())
            ]
        );
        assert!(cache.batch_running());
        assert_eq!(cache.batch_progress(), Some((1, 3)));
        // The queue is still full: nothing more is sent.
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.edits_writes.len(), 2);
        // The queue takes the third.
        worker.edits_capacity = None;
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            worker.edits_writes.get(2),
            Some(&(902, c.clone(), headline_edits()))
        );

        // Neither the window's own write nor a second batch is sent while it runs.
        assert_eq!(cache.save_edits(&a, title_edits(), &mut worker), Ok(()));
        assert!(!cache.start_batch(&a, title_edits(), Vec::new(), &mut worker));
        assert_eq!(worker.edits_writes.len(), 3);
        assert!(cache.take_editor_events().is_empty());
        assert!(cache.batch_running());
    }

    #[test]
    fn no_batch_starts_while_the_windows_write_runs() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let (a, b, _) = batch_images();
        assert_eq!(cache.save_edits(&a, title_edits(), &mut worker), Ok(()));

        assert!(!cache.start_batch(&a, title_edits(), vec![(b, headline_edits())], &mut worker));

        assert_eq!(worker.edits_writes.len(), 1);
        assert!(!cache.batch_running());
    }

    #[test]
    fn one_failure_stops_nothing_and_the_summary_counts_failures_and_kept_conflicts() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let (a, b, c) = start_three(&mut cache, &mut worker);
        assert_eq!(worker.edits_writes.len(), 3);
        assert_eq!(cache.batch_progress(), Some((1, 3)));

        // The ARW fails; the others still answer.
        worker
            .events
            .push_back(edits_written(901, &b, Err(WriteError::ReadOnly)));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.batch_progress(), Some((2, 3)));
        let window_notices = vec![
            WriteNotice::DigestNotWritten,
            WriteNotice::ConflictKept {
                property: "photoshop:State".to_owned(),
            },
        ];
        worker
            .events
            .push_back(edits_written(900, &a, Ok(report(window_notices.clone()))));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.batch_progress(), Some((3, 3)));
        assert!(cache.take_editor_events().is_empty());
        assert!(
            worker.metadata_reads.is_empty(),
            "the window's image is read again only when every write has answered"
        );

        worker.events.push_back(edits_written(
            902,
            &c,
            Ok(report(vec![
                WriteNotice::ConflictKept {
                    property: "photoshop:City".to_owned(),
                },
                WriteNotice::DigestNotWritten,
            ])),
        ));
        assert!(cache.poll(&mut worker).is_empty());

        assert!(!cache.batch_running());
        assert_eq!(cache.batch_progress(), None);
        let summary = BatchSummary {
            written: 2,
            total: 3,
            failed: vec![
                "Could not save metadata for pw-loupe-batch-b.ARW: the file is read-only"
                    .to_owned(),
            ],
            kept: vec!["Unresolved conflict kept: pw-loupe-batch-c.DNG (City)".to_owned()],
            notices: window_notices,
        };
        assert_eq!(
            summary.lines(),
            [
                "Written to 2 of 3 images.",
                "Could not save metadata for pw-loupe-batch-b.ARW: the file is read-only",
                "Unresolved conflict kept: pw-loupe-batch-c.DNG (City)",
                "No new IIM checksum was written, because a conflict is unresolved",
                "Province or State: the conflict is unresolved and was not written",
            ]
        );
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::BatchDone(summary)]
        );

        // Then the window's image is read again and the read refills the form.
        assert_eq!(worker.metadata_reads, [(500, a.clone())]);
        assert!(cache.editor_reloading());
        worker
            .events
            .push_back(metadata_read(500, &a, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert!(!cache.editor_reloading());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::Reloaded {
                path: a,
                result: Ok(empty_read()),
            }]
        );
    }

    #[test]
    fn a_list_with_qualifiers_kept_in_another_image_is_a_summary_line() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let (a, b, c) = start_three(&mut cache, &mut worker);
        assert_eq!(worker.edits_writes.len(), 3);

        let window_notices = vec![WriteNotice::QualifiersKept {
            property: "dc:subject".to_owned(),
        }];
        worker
            .events
            .push_back(edits_written(900, &a, Ok(report(window_notices.clone()))));
        assert!(cache.poll(&mut worker).is_empty());
        // The second image answers before the first other one.
        worker.events.push_back(edits_written(
            902,
            &c,
            Ok(report(vec![
                WriteNotice::ConflictKept {
                    property: "photoshop:City".to_owned(),
                },
                WriteNotice::QualifiersKept {
                    property: "dc:subject".to_owned(),
                },
            ])),
        ));
        assert!(cache.poll(&mut worker).is_empty());
        assert!(cache.take_editor_events().is_empty());
        worker.events.push_back(edits_written(
            901,
            &b,
            Ok(report(vec![WriteNotice::QualifiersKept {
                property: "dc:subject".to_owned(),
            }])),
        ));
        assert!(cache.poll(&mut worker).is_empty());

        assert!(!cache.batch_running());
        let summary = BatchSummary {
            written: 3,
            total: 3,
            failed: Vec::new(),
            kept: vec![
                "Unresolved conflict kept: pw-loupe-batch-c.DNG (City)".to_owned(),
                "List with qualifiers not written: pw-loupe-batch-c.DNG (Keywords)".to_owned(),
                "List with qualifiers not written: pw-loupe-batch-b.ARW (Keywords)".to_owned(),
            ],
            notices: window_notices,
        };
        assert_eq!(
            summary.lines(),
            [
                "Written to 3 of 3 images.",
                "Unresolved conflict kept: pw-loupe-batch-c.DNG (City)",
                "List with qualifiers not written: pw-loupe-batch-c.DNG (Keywords)",
                "List with qualifiers not written: pw-loupe-batch-b.ARW (Keywords)",
                "Keywords was not written: it has qualifiers RawStride cannot keep",
            ]
        );
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::BatchDone(summary)]
        );
    }

    #[test]
    fn the_window_reads_its_image_again_after_the_batch_as_after_its_own_write() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let (a, b, _) = batch_images();
        assert!(cache.open_window(&a, &mut worker));
        worker
            .events
            .push_back(metadata_read(500, &a, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(cache.take_window_read(), Some(Ok(empty_read())));
        assert!(cache.start_batch(
            &a,
            title_edits(),
            vec![(b.clone(), headline_edits())],
            &mut worker
        ));

        // Another image written: the window reads nothing.
        worker
            .events
            .push_back(edits_written(901, &b, Ok(report(Vec::new()))));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.metadata_reads.len(), 1);

        // The window's image written, the last answer: the read after the batch (501) and the
        // window's own new read (502).
        worker
            .events
            .push_back(edits_written(900, &a, Ok(report(Vec::new()))));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            worker.metadata_reads,
            [(500, a.clone()), (501, a.clone()), (502, a)]
        );
    }

    #[test]
    fn a_stopped_worker_fails_the_rest_of_the_batch() {
        // Refused when sent: every image fails at once, and the batch ends.
        let mut worker = FakeSubmit::new([]);
        worker.edits_stopped = true;
        let mut cache = MetadataCache::default();
        let (a, _, _) = start_three(&mut cache, &mut worker);
        assert!(!cache.batch_running());
        assert_eq!(
            cache.take_editor_events(),
            [EditorEvent::BatchDone(BatchSummary {
                written: 0,
                total: 3,
                failed: vec![
                    "Could not save metadata for pw-loupe-batch-a.jpg: metadata writing has \
                     stopped"
                        .to_owned(),
                    "Could not save metadata for pw-loupe-batch-b.ARW: metadata writing has \
                     stopped"
                        .to_owned(),
                    "Could not save metadata for pw-loupe-batch-c.DNG: metadata writing has \
                     stopped"
                        .to_owned(),
                ],
                kept: Vec::new(),
                notices: Vec::new(),
            })]
        );
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.metadata_reads, [(500, a)]);

        // Stopped while one is sent and two wait: all three fail.
        let mut worker = FakeSubmit::new([]);
        worker.edits_capacity = Some(1);
        let mut cache = MetadataCache::default();
        let (a, _, _) = start_three(&mut cache, &mut worker);
        assert_eq!(worker.edits_writes.len(), 1);
        let mut failures = Vec::new();
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert!(!cache.batch_running());
        let events = cache.take_editor_events();
        let Some(EditorEvent::BatchDone(summary)) = events.first() else {
            panic!("the batch should end: {events:?}");
        };
        assert_eq!(summary.written, 0);
        assert_eq!(summary.failed.len(), 3);
        assert_eq!(
            events.get(1),
            Some(&EditorEvent::Reloaded {
                path: a,
                result: Err("metadata writing has stopped".to_owned()),
            })
        );
    }

    #[test]
    fn the_exit_wait_counts_the_batch_and_lists_its_pending_and_failed_files() {
        // One sent and one waiting: the full wait, then both listed.
        let mut worker = FakeSubmit::new([]);
        worker.edits_capacity = Some(1);
        let mut cache = MetadataCache::default();
        let (a, b, _) = batch_images();
        assert!(cache.start_batch(
            &a,
            title_edits(),
            vec![(b.clone(), headline_edits())],
            &mut worker
        ));
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
            ["pw-loupe-batch-a.jpg", "pw-loupe-batch-b.ARW"]
        );

        // Both answered, one failed: it is listed with its reason and the wait ends.
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        assert!(cache.start_batch(
            &a,
            title_edits(),
            vec![(b.clone(), headline_edits())],
            &mut worker
        ));
        worker
            .events
            .push_back(edits_written(900, &a, Ok(report(Vec::new()))));
        worker
            .events
            .push_back(edits_written(901, &b, Err(WriteError::ReadOnly)));
        let clock = std::cell::Cell::new(Instant::now());
        let start = clock.get();
        let unsaved = finish_pending_writes(
            &mut cache,
            &mut worker,
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
        );
        assert_eq!(clock.get(), start, "no pause was needed");
        assert!(unsaved.pending.is_empty());
        assert_eq!(
            unsaved.failed,
            ["Could not save metadata for pw-loupe-batch-b.ARW: the file is read-only"]
        );
    }

    #[test]
    fn pruning_a_closed_image_empties_the_inspector() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-inspector-pruned.ARW");
        inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());

        cache.prune(|_| false);

        assert_eq!(cache.inspector_state(&image), InspectorState::Loading);
    }

    // --- the answers of the writes for the inspector's Histogram (U7c-Q7, T145) ---

    fn answer(path: &Path, succeeded: bool) -> WriteAnswer {
        WriteAnswer {
            path: path.to_path_buf(),
            succeeded,
        }
    }

    #[test]
    fn the_writes_of_an_editor_save_and_a_rating_key_hand_on_their_paths() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        assert!(cache.take_written().is_empty());

        // an editor save that succeeded, then one that failed
        let saved = image_path("pw-loupe-written-saved.DNG");
        assert_eq!(cache.save_edits(&saved, title_edits(), &mut worker), Ok(()));
        worker
            .events
            .push_back(edits_written(900, &saved, Ok(report(Vec::new()))));
        assert!(cache.poll(&mut worker).is_empty());
        let refused = image_path("pw-loupe-written-refused.jpg");
        assert_eq!(
            cache.save_edits(&refused, title_edits(), &mut worker),
            Ok(())
        );
        worker
            .events
            .push_back(edits_written(901, &refused, Err(WriteError::ReadOnly)));
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_written(),
            [answer(&saved, true), answer(&refused, false)]
        );
        // taken once
        assert!(cache.take_written().is_empty());

        // a rating key that succeeded, then one that failed
        let rated = std::env::temp_dir().join("pw-loupe-written-rated.DNG");
        assert!(cache.set_rating(&rated, RatingKey::Stars(3)).is_ok());
        worker.write_result = Some(Ok(()));
        let _ = cache.poll(&mut worker);
        let _ = cache.poll(&mut worker);
        assert_eq!(cache.take_written(), [answer(&rated, true)]);
        let failed = std::env::temp_dir().join("pw-loupe-written-failed.DNG");
        assert!(cache.set_rating(&failed, RatingKey::Stars(1)).is_ok());
        worker.write_result = Some(Err("refused".into()));
        let _ = cache.poll(&mut worker);
        let _ = cache.poll(&mut worker);
        let written = cache.take_written();
        assert_eq!(written, [answer(&failed, false)]);
        assert!(!written.iter().any(|written| written.succeeded));
    }

    #[test]
    fn each_write_of_apply_to_all_marked_hands_on_its_path() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let (a, b, c) = start_three(&mut cache, &mut worker);
        for (id, path, result) in [
            (900, &a, Ok(report(Vec::new()))),
            (901, &b, Err(WriteError::ReadOnly)),
            (902, &c, Ok(report(Vec::new()))),
        ] {
            worker.events.push_back(edits_written(id, path, result));
        }
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(
            cache.take_written(),
            [answer(&a, true), answer(&b, false), answer(&c, true)]
        );
    }

    // --- the moves and their undo through the cache (ADR 0017 U9b, T173) ---

    fn moved(from: &Path, to: &Path) -> Moved {
        Moved {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        }
    }

    #[test]
    fn the_fake_records_move_image_and_move_back() {
        let mut worker = FakeSubmit::new([]);
        let image = image_path("pw-loupe-move-record.ARW");
        let target = image_path("_rejected");
        let extensions = vec!["arw".to_owned(), "jpg".to_owned()];
        let suffixes = vec!["dop".to_owned()];
        assert_eq!(
            MetadataSubmit::move_image(
                &mut worker,
                image.clone(),
                target.clone(),
                extensions.clone(),
                suffixes.clone()
            ),
            Ok(1300)
        );
        let files = vec![moved(&image, &target.join("pw-loupe-move-record.ARW"))];
        assert_eq!(
            MetadataSubmit::move_back(&mut worker, files.clone()),
            Ok(1301)
        );
        assert_eq!(
            worker.move_images,
            [(1300, image, target, extensions, suffixes)]
        );
        assert_eq!(worker.move_backs, [(1301, files)]);
        assert_eq!(worker.operations, ["move_image", "move_back"]);
    }

    #[test]
    fn a_move_and_a_move_back_reach_take_moves_in_order_and_once() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        assert!(cache.take_moves().is_empty());
        let image = image_path("pw-loupe-move-answer.ARW");
        let target = image_path("_rejected");
        let files = vec![moved(&image, &target.join("pw-loupe-move-answer.ARW"))];
        worker.events.push_back(WorkerEvent::Moved {
            id: 1300,
            image: image.clone(),
            result: Ok(files.clone()),
        });
        worker.events.push_back(WorkerEvent::MovedBack {
            id: 1301,
            moved: files.clone(),
            result: Err("the file is open in another program".to_owned()),
        });

        assert!(cache.poll(&mut worker).is_empty());

        assert_eq!(
            cache.take_moves(),
            [
                MoveAnswer::Moved {
                    id: 1300,
                    image,
                    result: Ok(files.clone()),
                },
                MoveAnswer::MovedBack {
                    id: 1301,
                    moved: files,
                    result: Err("the file is open in another program".to_owned()),
                },
            ]
        );
        // taken once
        assert!(cache.take_moves().is_empty());
        // a move answer is no write answer
        assert!(cache.take_written().is_empty());
    }

    #[test]
    fn is_pending_and_current_rating_follow_the_rating_store() {
        let mut worker = FakeSubmit::new([]);
        // each write's answer is queued when it is sent
        worker.write_result = Some(Ok(()));
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-move-queued.ARW");
        assert!(!cache.is_pending(&image));
        assert_eq!(cache.current_rating(&image), Rating::default());

        assert!(cache.set_rating(&image, RatingKey::Stars(4)).is_ok());
        assert!(cache.is_pending(&image));
        let _ = cache.poll(&mut worker);
        assert!(cache.is_pending(&image), "sent, waiting for its answer");
        let _ = cache.poll(&mut worker);
        assert!(!cache.is_pending(&image), "answered");
        assert!(cache.set_rating(&image, RatingKey::Reject).is_ok());

        // an X shows `is_pending` until `poll` applies its answer
        assert!(cache.is_pending(&image));
        assert_eq!(
            cache.current_rating(&image),
            Rating {
                stars: Some(-1),
                label: None,
            }
        );
        assert!(cache.poll(&mut worker).is_empty());
        assert!(cache.is_pending(&image), "sent, waiting for its answer");
        assert!(cache.poll(&mut worker).is_empty());
        assert!(!cache.is_pending(&image), "answered");
        assert_eq!(cache.current_rating(&image).stars, Some(-1));
        assert_eq!(worker.write_attempts, [image.clone(), image]);
    }

    #[test]
    fn is_pending_lasts_until_a_failed_x_is_polled_and_rolled_back() {
        let mut worker = FakeSubmit::new([]);
        worker.write_result = Some(Err("read-only".to_owned()));
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-move-pending-failed.ARW");

        assert!(cache.set_rating(&image, RatingKey::Reject).is_ok());
        assert!(cache.is_pending(&image));
        assert!(cache.poll(&mut worker).is_empty());
        assert!(cache.is_pending(&image), "sent, waiting for its answer");
        assert_eq!(cache.current_rating(&image).stars, Some(-1));

        // the answer is polled: no longer pending, and the reject is rolled back
        assert_eq!(cache.poll(&mut worker).len(), 1);
        assert!(!cache.is_pending(&image));
        assert_ne!(cache.current_rating(&image).stars, Some(-1));
    }

    #[test]
    fn restore_rating_queues_the_rating() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-move-restored.ARW");
        let rating = Rating {
            stars: Some(3),
            label: Some("Green".to_owned()),
        };

        assert_eq!(cache.restore_rating(&image, rating.clone()), Ok(()));

        assert!(cache.is_pending(&image));
        assert_eq!(cache.current_rating(&image), rating);
        assert!(cache.poll(&mut worker).is_empty());
        assert!(cache.is_pending(&image));
        assert_eq!(worker.write_attempts, [image]);
        // an unsupported file is refused, as by a rating key
        assert_eq!(
            cache.restore_rating(&image_path("pw-loupe-move-notes.txt"), rating),
            Err(RatingRefused::Unsupported)
        );
    }

    #[test]
    fn has_image_is_true_for_a_loaded_image_and_false_for_another() {
        let mut worker = FakeSubmit::new([Ok(7)]);
        let mut cache = MetadataCache::default();
        let loaded = image_path("pw-loupe-move-loaded.ARW");
        let files = match cache.begin_load(vec![loaded.clone()], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        assert_eq!(files, std::slice::from_ref(&loaded));

        assert!(cache.has_image(&loaded));
        assert!(!cache.has_image(&image_path("pw-loupe-move-other.ARW")));
    }

    // --- the deletes through the cache (ADR 0018 B3, T188) ---

    #[test]
    fn the_fake_records_prepare_delete_and_delete_image() {
        let mut worker = FakeSubmit::new([]);
        let a = image_path("pw-loupe-delete-record-a.ARW");
        let b = image_path("pw-loupe-delete-record-b.ARW");
        let extensions = vec!["arw".to_owned(), "jpg".to_owned()];
        let suffixes = vec!["dop".to_owned()];
        assert_eq!(
            MetadataSubmit::prepare_delete(
                &mut worker,
                vec![a.clone(), b.clone()],
                extensions.clone(),
                suffixes.clone()
            ),
            Ok(1700)
        );
        assert_eq!(
            MetadataSubmit::delete_image(
                &mut worker,
                a.clone(),
                extensions.clone(),
                suffixes.clone()
            ),
            Ok(1701)
        );
        assert_eq!(
            worker.prepare_deletes,
            [(
                1700,
                vec![a.clone(), b],
                extensions.clone(),
                suffixes.clone()
            )]
        );
        assert_eq!(worker.delete_images, [(1701, a, extensions, suffixes)]);
        assert_eq!(worker.operations, ["prepare_delete", "delete_image"]);
    }

    #[test]
    fn a_delete_prepared_and_a_deleted_reach_take_deletes_in_order_and_once() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        assert!(cache.take_deletes().is_empty());
        let image = image_path("pw-loupe-delete-answer.ARW");
        let sidecar = image_path("pw-loupe-delete-answer.ARW.dop");
        let card = image_path("pw-loupe-delete-card.ARW");
        let prepared = vec![
            PreparedDelete {
                image: image.clone(),
                result: Ok(pw_metadata::DeleteGroup {
                    files: vec![image.clone(), sidecar.clone()],
                    bytes: 1234,
                }),
            },
            PreparedDelete {
                image: card.clone(),
                result: Err(DeleteError {
                    file: card,
                    reason: pw_metadata::DeleteRefusal::Card,
                    left: Vec::new(),
                    deleted: Vec::new(),
                }),
            },
        ];
        worker.events.push_back(WorkerEvent::DeletePrepared {
            id: 1700,
            images: prepared.clone(),
        });
        worker.events.push_back(WorkerEvent::Deleted {
            id: 1701,
            image: image.clone(),
            result: Ok(vec![image.clone(), sidecar.clone()]),
        });

        assert!(cache.poll(&mut worker).is_empty());

        assert_eq!(
            cache.take_deletes(),
            [
                DeleteAnswer::Prepared {
                    id: 1700,
                    images: prepared,
                },
                DeleteAnswer::Deleted {
                    id: 1701,
                    image: image.clone(),
                    result: Ok(vec![image, sidecar]),
                },
            ]
        );
        // taken once
        assert!(cache.take_deletes().is_empty());
        // a delete answer is no move answer and no write answer
        assert!(cache.take_moves().is_empty());
        assert!(cache.take_written().is_empty());
    }

    /// A cache with `files` loaded through `worker` (T188).
    fn loaded_cache(files: &[PathBuf], worker: &mut FakeSubmit) -> MetadataCache {
        let mut cache = MetadataCache::default();
        if let Err(error) = cache.begin_load(files.to_vec(), worker) {
            panic!("current directory should be available: {error}");
        }
        cache
    }

    #[test]
    fn forget_deleted_removes_a_loaded_image_and_keeps_the_others() {
        let mut worker = FakeSubmit::new([Ok(7)]);
        let deleted = image_path("pw-loupe-forget-loaded.ARW");
        let kept = image_path("pw-loupe-forget-kept.ARW");
        let mut cache = loaded_cache(&[deleted.clone(), kept.clone()], &mut worker);
        assert!(cache.has_image(&deleted));

        cache.forget_deleted(std::slice::from_ref(&deleted));

        assert!(!cache.has_image(&deleted));
        assert!(cache.has_image(&kept));
    }

    #[test]
    fn forget_deleted_clears_the_inspectors_image_only_when_it_is_deleted() {
        let mut worker = FakeSubmit::new([]);
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-forget-inspector.ARW");
        inspector_read_sent(&mut cache, &image, Instant::now(), &mut worker);
        worker
            .events
            .push_back(metadata_read(500, &image, Ok(empty_read())));
        assert!(cache.poll(&mut worker).is_empty());

        // another image deleted: the inspector keeps its image and its result
        cache.forget_deleted(&[image_path("pw-loupe-forget-other.ARW")]);
        assert!(cache.inspector.image.is_some());
        assert!(matches!(
            cache.inspector_state(&image),
            InspectorState::Ready(_)
        ));

        // its own image deleted: cleared
        cache.forget_deleted(std::slice::from_ref(&image));
        assert!(cache.inspector.image.is_none());
    }

    #[test]
    fn forget_deleted_drops_the_rating_of_an_image_without_a_pending_write() {
        let mut worker = FakeSubmit::new([Ok(7)]);
        let deleted = image_path("pw-loupe-forget-rated.ARW");
        let kept = image_path("pw-loupe-forget-rated-kept.ARW");
        let mut cache = loaded_cache(&[deleted.clone(), kept.clone()], &mut worker);
        let ratings = HashMap::from([
            (deleted.clone(), Rating::default()),
            (kept.clone(), Rating::default()),
        ]);
        worker.events.push_back(WorkerEvent::RatingsRead {
            id: 7,
            result: Ok(ratings),
        });
        assert!(cache.poll(&mut worker).is_empty());
        assert!(cache.ratings.contains(&deleted));
        assert!(!cache.is_pending(&deleted));

        cache.forget_deleted(std::slice::from_ref(&deleted));

        assert!(!cache.ratings.contains(&deleted));
        assert!(cache.ratings.contains(&kept));
    }

    #[test]
    fn forget_deleted_keeps_a_rating_write_that_is_still_queued() {
        let mut cache = MetadataCache::default();
        let image = image_path("pw-loupe-forget-queued.ARW");
        assert!(cache.set_rating(&image, RatingKey::Stars(2)).is_ok());
        assert!(cache.is_pending(&image));

        cache.forget_deleted(std::slice::from_ref(&image));

        assert!(cache.is_pending(&image));
        assert!(cache.ratings.contains(&image));
    }

    #[test]
    fn forget_deleted_takes_a_deleted_path_out_of_a_waiting_folder_read() {
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull), Ok(8)]);
        let deleted = image_path("pw-loupe-forget-pending-a.ARW");
        let kept = image_path("pw-loupe-forget-pending-b.ARW");
        let mut cache = loaded_cache(&[deleted.clone(), kept.clone()], &mut worker);
        assert!(cache.pending_read.is_some());

        cache.forget_deleted(std::slice::from_ref(&deleted));

        assert_eq!(cache.pending_read, Some(vec![kept.clone()]));
        // the retry asks only for the image that is left
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.read_attempts.last(), Some(&vec![kept]));
        assert!(cache.pending_read.is_none());

        // when every waiting path is deleted, nothing waits to be sent
        let mut worker = FakeSubmit::new([Err(SubmitError::QueueFull)]);
        let mut cache = loaded_cache(std::slice::from_ref(&deleted), &mut worker);
        cache.forget_deleted(std::slice::from_ref(&deleted));
        assert!(cache.pending_read.is_none());
        assert!(cache.poll(&mut worker).is_empty());
        assert_eq!(worker.read_attempts.len(), 1, "no empty read is sent");
    }
}
