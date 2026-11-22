//! The turns of images in RawStride (ADR 0024 §2.5, O-Q8; T236): the optimistic store of the
//! orientations shown, its ordered write queue, and the reader of a file's own Exif orientation.
//! A turn works as a rating key: the shown value changes at once, its write goes into an ordered
//! queue, and a failed write turns the image back. Plain Rust without drawing (GR11); the keys,
//! the menu, the metadata cache and the view are wired to it in T237. Its texts are in
//! `rating_text.rs`.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use pw_metadata::{MetadataWorker, RequestId, SubmitError, supports_embedded, supports_rating};
use pw_viewcore::geometry::turned;

use crate::rating_text::{rotation_failure_message, rotation_failures_text, worker_stopped_text};
use crate::ratings::{RatingRefused, absolute_path};

/// A metadata request writer for orientations, separated from ExifTool so the queue can be tested
/// directly (as `RatingWriter`).
pub trait OrientationWriter {
    fn write_orientation(
        &mut self,
        path: PathBuf,
        orientation: u16,
    ) -> Result<RequestId, SubmitError>;
}

impl OrientationWriter for MetadataWorker {
    fn write_orientation(
        &mut self,
        path: PathBuf,
        orientation: u16,
    ) -> Result<RequestId, SubmitError> {
        MetadataWorker::write_orientation(self, path, orientation)
    }
}

#[derive(Clone, Debug)]
struct OrientationWrite {
    path: PathBuf,
    orientation: u16,
}

#[derive(Clone, Debug, Default)]
struct OrientationEntry {
    /// Read from XMP, or written by RawStride and answered `Ok`.
    confirmed: Option<u16>,
    /// The confirmed value was written by RawStride into a file that holds its own metadata
    /// (JPEG, TIFF, DNG). Such a value lives in IFD0, which the folder read does not read, so a
    /// read without an XMP value keeps it. It must stay: the thumbnail cache's key has no file
    /// time and the view core keeps the flip it read before the write, so a written value going
    /// back to "the file's own" would show a stale thumbnail and flip until a restart.
    confirmed_by_write: bool,
    /// The value told to the view cores and the thumbnails; `None` means "the file's own".
    shown: Option<u16>,
    /// The file's own Exif orientation, once read.
    embedded: Option<u16>,
    /// Clockwise quarter turns (0–3) pressed before the value to start from was known.
    waiting: u8,
    /// The embedded value was asked for and has not come.
    requested: bool,
}

impl OrientationEntry {
    /// The value a turn starts from: the shown one, else the confirmed one, else the embedded one.
    fn start(&self) -> Option<u16> {
        self.shown.or(self.confirmed).or(self.embedded)
    }
}

/// The file name and the error text of the last failed write.
#[derive(Clone, Debug)]
struct WriteFailure {
    name: String,
    error: String,
}

/// Optimistic per-image orientations and their ordered, non-blocking write queue (ADR 0024 §2.5),
/// keyed by absolute path, with the shown-versus-confirmed rule of `RatingStore`.
#[derive(Clone, Debug, Default)]
pub struct OrientationStore {
    entries: HashMap<PathBuf, OrientationEntry>,
    queued: VecDeque<OrientationWrite>,
    sent: HashMap<RequestId, OrientationWrite>,
    /// The changes of the shown values since the last `take_changes`, in order.
    changes: Vec<(PathBuf, Option<u16>)>,
    /// Write failures since the last accepted turn.
    failure_count: usize,
    last_failure: Option<WriteFailure>,
}

/// A value outside `1..=8` is taken as missing.
fn valid(orientation: u16) -> Option<u16> {
    (1..=8).contains(&orientation).then_some(orientation)
}

/// Sets the shown value of `entry` and records a change when it changed.
fn show(
    entry: &mut OrientationEntry,
    changes: &mut Vec<(PathBuf, Option<u16>)>,
    path: &Path,
    value: Option<u16>,
) {
    if entry.shown != value {
        entry.shown = value;
        changes.push((path.to_path_buf(), value));
    }
}

impl OrientationStore {
    /// Turns the images 90° clockwise (`clockwise`) or anticlockwise. Files that cannot carry
    /// metadata are skipped; when none is left, the turn is refused and nothing changes. A path
    /// given twice is turned once. An image with a known value turns at once and its write is
    /// queued; an image without one keeps the turn waiting until its embedded value comes
    /// (`embedded_read`). Returns the paths whose embedded value the caller must ask for.
    pub fn turn_many(
        &mut self,
        paths: &[PathBuf],
        clockwise: bool,
    ) -> Result<Vec<PathBuf>, RatingRefused> {
        let supported: Vec<&PathBuf> = paths.iter().filter(|path| supports_rating(path)).collect();
        if supported.is_empty() {
            return Err(RatingRefused::Unsupported);
        }
        let mut targets: Vec<PathBuf> = Vec::with_capacity(supported.len());
        for path in supported {
            let path = absolute_path(path)?;
            if !targets.contains(&path) {
                targets.push(path);
            }
        }

        self.reset_failures();
        let mut ask = Vec::new();
        for path in targets {
            let entry = self.entries.entry(path.clone()).or_default();
            match entry.start() {
                Some(start) => self.put(path, turned(start, clockwise)),
                None => {
                    entry.waiting = (entry.waiting + if clockwise { 1 } else { 3 }) % 4;
                    if !entry.requested {
                        entry.requested = true;
                        ask.push(path);
                    }
                }
            }
        }
        Ok(ask)
    }

    /// The answer of the embedded-orientation reader for `path` (absolute). A value outside
    /// `1..=8`, or none, is taken as 1. Turns that waited for it are applied now. A path not in
    /// the store is ignored.
    pub fn embedded_read(&mut self, path: &Path, orientation: Option<u16>) {
        let Some(entry) = self.entries.get_mut(path) else {
            return;
        };
        let embedded = orientation.and_then(valid).unwrap_or(1);
        entry.embedded = Some(embedded);
        entry.requested = false;
        if entry.waiting != 0 {
            let mut value = entry.shown.or(entry.confirmed).unwrap_or(embedded);
            for _ in 0..entry.waiting {
                value = turned(value, true);
            }
            entry.waiting = 0;
            self.put(path.to_path_buf(), value);
        }
    }

    /// Shows `orientation` at once and queues it, replacing an unsent queued write for the path.
    fn put(&mut self, path: PathBuf, orientation: u16) {
        let entry = self.entries.entry(path.clone()).or_default();
        show(entry, &mut self.changes, &path, Some(orientation));

        if let Some(queued) = self.queued.iter_mut().find(|write| write.path == path) {
            queued.orientation = orientation;
        } else {
            self.queued
                .push_back(OrientationWrite { path, orientation });
        }
    }

    fn reset_failures(&mut self) {
        self.failure_count = 0;
        self.last_failure = None;
    }

    /// Sends writes in turn order. Back-pressure keeps the remaining writes queued.
    pub fn pump(&mut self, writer: &mut impl OrientationWriter) -> Vec<String> {
        let mut failures = Vec::new();
        while let Some(write) = self.queued.front().cloned() {
            match writer.write_orientation(write.path.clone(), write.orientation) {
                Ok(id) => {
                    let _ = self.queued.pop_front();
                    let _ = self.sent.insert(id, write);
                }
                Err(SubmitError::QueueFull) => break,
                Err(SubmitError::Stopped) => {
                    let _ = self.queued.pop_front();
                    if let Some(message) =
                        self.fail_write(&write.path, None, worker_stopped_text().to_owned())
                    {
                        failures.push(message);
                    }
                }
            }
        }
        failures
    }

    /// Completes a sent write and returns a user-facing message when it failed (then the image
    /// turns back unless a newer write of it is queued or sent).
    pub fn written(
        &mut self,
        id: RequestId,
        path: &Path,
        result: Result<(), String>,
    ) -> Option<String> {
        let write = self.sent.remove(&id)?;
        if write.path != path {
            let _ = self.sent.insert(id, write);
            return None;
        }
        match result {
            Ok(()) => {
                let has_newer = self.has_newer_pending(path, id);
                let own_metadata = supports_embedded(path);
                if let Some(entry) = self.entries.get_mut(path) {
                    entry.confirmed = Some(write.orientation);
                    if own_metadata {
                        // Now the file's own Exif value; a RAW's sidecar is read again instead.
                        entry.confirmed_by_write = true;
                        entry.embedded = Some(write.orientation);
                    }
                    if !has_newer {
                        let confirmed = entry.confirmed;
                        show(entry, &mut self.changes, path, confirmed);
                    }
                }
                None
            }
            Err(error) => self.fail_write(path, Some(id), error),
        }
    }

    /// Applies a folder read's XMP orientations without replacing a newer optimistic value: a
    /// path with a queued write, or with a write sent after `read_id` and not answered, keeps
    /// its shown value. A path missing from the map loses its confirmed value unless RawStride
    /// wrote it into the file itself.
    pub fn read(
        &mut self,
        read_id: RequestId,
        orientations: &HashMap<PathBuf, u16>,
        paths: &[PathBuf],
    ) {
        for path in paths {
            let entry = self.entries.entry(path.clone()).or_default();
            match orientations.get(path).copied().and_then(valid) {
                Some(value) => {
                    entry.confirmed = Some(value);
                    entry.confirmed_by_write = false;
                }
                None => {
                    if !entry.confirmed_by_write {
                        entry.confirmed = None;
                    }
                }
            }
            let queued = self.queued.iter().any(|write| write.path == *path);
            let newer_sent = self
                .sent
                .iter()
                .any(|(id, write)| *id > read_id && write.path == *path);
            if !queued && !newer_sent {
                let confirmed = entry.confirmed;
                show(entry, &mut self.changes, path, confirmed);
            }
        }
    }

    /// "Reload Folder" (ADR 0026 §3.1, T250): forgets the embedded value of each of `paths`
    /// (absolute) whose entry has no write queued or sent (`is_pending`) and no turn waiting for
    /// its embedded value (`is_waiting`), so the next turn of a file turned by another program
    /// asks for its value again. The confirmed value, `confirmed_by_write` and the shown value
    /// are not changed: a value RawStride wrote is kept (see `confirmed_by_write`), and a turn
    /// starts from it before the embedded one.
    pub fn forget_embedded(&mut self, paths: &[PathBuf]) {
        for path in paths {
            if self.is_pending(path) || self.is_waiting(path) {
                continue;
            }
            if let Some(entry) = self.entries.get_mut(path) {
                entry.embedded = None;
            }
        }
    }

    /// The shown value of `path` (absolute); `None` means "the file's own".
    pub fn orientation(&self, path: &Path) -> Option<u16> {
        self.entries.get(path)?.shown
    }

    /// The changes of the shown values since the last call, in order; empties the list.
    pub fn take_changes(&mut self) -> Vec<(PathBuf, Option<u16>)> {
        std::mem::take(&mut self.changes)
    }

    /// The status-line text for the write failures since the last turn: `None` without
    /// failures, the message of the failed image for one, and the count plus the last failure
    /// for more.
    pub fn failure_text(&self) -> Option<String> {
        let last = self.last_failure.as_ref()?;
        match self.failure_count {
            0 => None,
            1 => Some(rotation_failure_message(&last.name, &last.error)),
            count => Some(rotation_failures_text(count, &last.name, &last.error)),
        }
    }

    /// Whether a write for `path` (absolute) is queued or sent and has no answer yet. A path
    /// that only waits for its embedded value has no write yet and is not pending.
    pub fn is_pending(&self, path: &Path) -> bool {
        self.queued.iter().any(|write| write.path == path)
            || self.sent.values().any(|write| write.path == path)
    }

    /// The number of writes queued or sent and not answered.
    pub fn pending(&self) -> usize {
        self.queued.len() + self.sent.len()
    }

    /// The paths with a pending write: the sent ones in request order, then the queued ones.
    pub fn pending_paths(&self) -> Vec<PathBuf> {
        let mut writes: Vec<_> = self.sent.iter().collect();
        writes.sort_by_key(|(id, _)| **id);
        let mut paths = Vec::new();
        for (_, write) in writes {
            if !paths.contains(&write.path) {
                paths.push(write.path.clone());
            }
        }
        for write in &self.queued {
            if !paths.contains(&write.path) {
                paths.push(write.path.clone());
            }
        }
        paths
    }

    /// Whether a turn of `path` (absolute) waits for its embedded value: asked for and not
    /// come, or turns kept that are not applied yet.
    pub fn is_waiting(&self, path: &Path) -> bool {
        self.entries
            .get(path)
            .is_some_and(|entry| entry.requested || entry.waiting != 0)
    }

    /// The paths that wait for their embedded value, sorted.
    pub fn waiting_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.requested || entry.waiting != 0)
            .map(|(path, _)| path.clone())
            .collect();
        paths.sort();
        paths
    }

    /// Drops the paths not kept and not pending. Each dropped path that had a shown value
    /// records `(path, None)`, the dropped paths sorted. A path that only waits for its embedded
    /// value is dropped with its waiting turns.
    pub fn prune(&mut self, keep: impl Fn(&Path) -> bool) {
        let pending = self.pending_paths();
        let mut dropped = Vec::new();
        self.entries.retain(|path, entry| {
            let kept = keep(path) || pending.contains(path);
            if !kept && entry.shown.is_some() {
                dropped.push(path.clone());
            }
            kept
        });
        dropped.sort();
        self.changes
            .extend(dropped.into_iter().map(|path| (path, None)));
    }

    /// Fails every sent write, as when the metadata worker has stopped.
    pub fn fail_sent(&mut self) -> Vec<String> {
        let mut ids: Vec<_> = self.sent.keys().copied().collect();
        ids.sort_unstable();
        ids.into_iter()
            .filter_map(|id| {
                let write = self.sent.remove(&id)?;
                self.fail_write(&write.path, Some(id), worker_stopped_text().to_owned())
            })
            .collect()
    }

    fn has_newer_pending(&self, path: &Path, id: RequestId) -> bool {
        self.queued.iter().any(|write| write.path == path)
            || self
                .sent
                .iter()
                .any(|(sent_id, write)| *sent_id > id && write.path == path)
    }

    fn fail_write(
        &mut self,
        path: &Path,
        newer_than: Option<RequestId>,
        error: String,
    ) -> Option<String> {
        let has_newer = newer_than.is_some_and(|id| self.has_newer_pending(path, id));
        if let Some(entry) = self.entries.get_mut(path)
            && !has_newer
        {
            let confirmed = entry.confirmed;
            show(entry, &mut self.changes, path, confirmed);
        }
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let message = rotation_failure_message(&name, &error);
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_failure = Some(WriteFailure { name, error });
        Some(message)
    }
}

/// The embedded-orientation reader has stopped: no answer will come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReaderStopped;

/// The reader of a file's own Exif orientation: one thread it owns, which reads one file at a
/// time with `pw_raw::inspect` (as `CaptureReader`). Asked only for an image turned without a
/// known value. The thread ends when the reader is dropped.
#[derive(Debug)]
pub struct EmbeddedOrientationReader {
    requests: Sender<PathBuf>,
    results: Receiver<(PathBuf, Option<u16>)>,
}

impl EmbeddedOrientationReader {
    pub fn start() -> Self {
        let (requests, request_receiver) = mpsc::channel::<PathBuf>();
        let (result_sender, results) = mpsc::channel();
        let started = std::thread::Builder::new()
            .name("pw-orientation-reader".into())
            .spawn(move || {
                for path in request_receiver {
                    let orientation = pw_raw::inspect(&path).ok().map(|info| info.orientation);
                    if result_sender.send((path, orientation)).is_err() {
                        return;
                    }
                }
            });
        if let Err(error) = started {
            eprintln!("pw-loupe: the orientation reader could not be started: {error}");
        }
        Self { requests, results }
    }

    /// Asks for the embedded orientation of `path`; false when the reader has stopped (the
    /// caller then reads the value itself).
    pub fn request(&self, path: PathBuf) -> bool {
        self.requests.send(path).is_ok()
    }

    /// An answer that has come back: `Ok(None)` when none has come yet, `Err(ReaderStopped)`
    /// when the thread has ended and no answer will come.
    pub fn try_recv(&self) -> Result<Option<(PathBuf, Option<u16>)>, ReaderStopped> {
        match self.results.try_recv() {
            Ok(answer) => Ok(Some(answer)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(ReaderStopped),
        }
    }
}

/// The rotation keys (ADR 0024 §2.1, O-Q1; T237): Ctrl+← turns 90° anticlockwise
/// (`Some(false)`), Ctrl+→ 90° clockwise (`Some(true)`). With Shift or Alt, or any other key,
/// `None`.
pub fn rotation_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<bool> {
    if !ctrl || shift || alt {
        return None;
    }
    match key {
        egui::Key::ArrowLeft => Some(false),
        egui::Key::ArrowRight => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmbeddedOrientationReader, OrientationStore, OrientationWriter, rotation_key_action,
    };
    use crate::ratings::RatingRefused;
    use pw_metadata::{RequestId, SubmitError};
    use std::collections::{HashMap, VecDeque};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum WriteResponse {
        Accept,
        Stopped,
    }

    struct FakeWriter {
        responses: VecDeque<WriteResponse>,
        next_id: RequestId,
        requests: Vec<(RequestId, PathBuf, u16)>,
    }

    impl Default for FakeWriter {
        fn default() -> Self {
            Self {
                responses: VecDeque::new(),
                next_id: 1,
                requests: Vec::new(),
            }
        }
    }

    impl OrientationWriter for FakeWriter {
        fn write_orientation(
            &mut self,
            path: PathBuf,
            orientation: u16,
        ) -> Result<RequestId, SubmitError> {
            match self.responses.pop_front().unwrap_or(WriteResponse::Accept) {
                WriteResponse::Accept => {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.requests.push((id, path, orientation));
                    Ok(id)
                }
                WriteResponse::Stopped => Err(SubmitError::Stopped),
            }
        }
    }

    fn image(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn read_value(store: &mut OrientationStore, path: &Path, id: RequestId, value: u16) {
        let orientations = [(path.to_path_buf(), value)].into();
        store.read(id, &orientations, &[path.to_path_buf()]);
    }

    fn read_none(store: &mut OrientationStore, path: &Path, id: RequestId) {
        store.read(id, &HashMap::new(), &[path.to_path_buf()]);
    }

    fn turn(store: &mut OrientationStore, path: &Path, clockwise: bool) -> Vec<PathBuf> {
        store
            .turn_many(&[path.to_path_buf()], clockwise)
            .expect("the turn is accepted")
    }

    fn written_values(writer: &FakeWriter) -> Vec<u16> {
        writer.requests.iter().map(|(_, _, value)| *value).collect()
    }

    #[test]
    fn a_turn_from_a_read_value_is_shown_at_once_and_queued() {
        let path = image("pw-loupe-turn-read.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 6);
        assert_eq!(store.take_changes(), [(path.clone(), Some(6))]);

        assert!(turn(&mut store, &path, true).is_empty());

        assert_eq!(store.orientation(&path), Some(3));
        assert_eq!(store.take_changes(), [(path.clone(), Some(3))]);
        assert!(store.is_pending(&path));
        assert!(store.pump(&mut writer).is_empty());
        assert_eq!(writer.requests, [(1, path, 3)]);
    }

    #[test]
    fn a_turn_without_a_known_value_asks_for_the_embedded_one_once() {
        let path = image("pw-loupe-turn-unknown.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();

        assert_eq!(turn(&mut store, &path, true), std::slice::from_ref(&path));
        assert!(turn(&mut store, &path, true).is_empty());
        assert_eq!(store.orientation(&path), None);
        assert_eq!(store.pending(), 0);
        assert!(store.take_changes().is_empty());

        store.embedded_read(&path, Some(1));

        assert_eq!(store.orientation(&path), Some(3));
        assert_eq!(store.take_changes(), [(path.clone(), Some(3))]);
        assert_eq!(store.pending(), 1);
        store.pump(&mut writer);
        assert_eq!(written_values(&writer), [3]);
        assert_eq!(store.entries[&path].waiting, 0);
    }

    #[test]
    fn turns_that_cancel_before_the_embedded_value_comes_write_nothing() {
        let path = image("pw-loupe-turn-cancel.ARW");
        let mut store = OrientationStore::default();
        assert_eq!(turn(&mut store, &path, true), std::slice::from_ref(&path));
        assert!(turn(&mut store, &path, false).is_empty());

        store.embedded_read(&path, Some(6));

        assert_eq!(store.orientation(&path), None);
        assert_eq!(store.pending(), 0);
        assert!(store.take_changes().is_empty());
        assert!(!store.is_waiting(&path));
    }

    #[test]
    fn a_path_waits_from_the_turn_until_its_embedded_value_comes() {
        let path = image("pw-loupe-turn-waits.ARW");
        let other = image("pw-loupe-turn-waits-other.ARW");
        let mut store = OrientationStore::default();
        read_value(&mut store, &other, 1, 1);
        assert!(!store.is_waiting(&path));
        assert!(store.waiting_paths().is_empty());

        assert_eq!(turn(&mut store, &path, true), std::slice::from_ref(&path));
        assert!(store.is_waiting(&path));
        assert!(!store.is_waiting(&other));
        assert_eq!(store.waiting_paths(), std::slice::from_ref(&path));
        assert!(!store.is_pending(&path), "no write exists yet");

        store.embedded_read(&path, Some(1));
        assert!(!store.is_waiting(&path));
        assert!(store.waiting_paths().is_empty());
    }

    #[test]
    fn a_failed_embedded_read_starts_from_1() {
        let path = image("pw-loupe-turn-unreadable.ARW");
        let mut store = OrientationStore::default();
        assert_eq!(turn(&mut store, &path, true), std::slice::from_ref(&path));

        store.embedded_read(&path, None);

        assert_eq!(store.orientation(&path), Some(6));
    }

    #[test]
    fn an_embedded_value_for_a_path_not_in_the_store_is_ignored() {
        let path = image("pw-loupe-turn-ignored.ARW");
        let mut store = OrientationStore::default();
        store.embedded_read(&path, Some(6));
        assert!(!store.entries.contains_key(&path));
        assert!(store.take_changes().is_empty());
    }

    #[test]
    fn a_mirrored_value_stays_mirrored() {
        let path = image("pw-loupe-turn-mirrored.ARW");
        let mut store = OrientationStore::default();
        read_value(&mut store, &path, 1, 2);

        let _ = turn(&mut store, &path, true);

        assert_eq!(store.orientation(&path), Some(7));
    }

    #[test]
    fn queued_turns_coalesce_into_the_last_value() {
        let path = image("pw-loupe-turn-coalesce.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 1);
        for _ in 0..3 {
            let _ = turn(&mut store, &path, true);
        }

        assert_eq!(store.pending(), 1);
        store.pump(&mut writer);
        assert_eq!(written_values(&writer), [8]);
        assert_eq!(store.orientation(&path), Some(8));
    }

    #[test]
    fn a_path_given_twice_is_turned_once() {
        let path = image("pw-loupe-turn-twice.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 1);

        assert_eq!(
            store.turn_many(&[path.clone(), path.clone()], true),
            Ok(vec![])
        );

        assert_eq!(store.orientation(&path), Some(6));
        store.pump(&mut writer);
        assert_eq!(written_values(&writer), [6]);
    }

    #[test]
    fn a_failed_write_turns_back_to_the_confirmed_value() {
        let path = image("pw-loupe-turn-failed.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 6);
        let _ = turn(&mut store, &path, true);
        store.pump(&mut writer);
        let _ = store.take_changes();
        let id = writer.requests[0].0;

        assert_eq!(
            store.written(id, &path, Err("write failed".to_owned())),
            Some(
                "Could not save the rotation for pw-loupe-turn-failed.ARW: write failed".to_owned()
            )
        );

        assert_eq!(store.orientation(&path), Some(6));
        assert_eq!(store.take_changes(), [(path.clone(), Some(6))]);
        assert_eq!(
            store.failure_text().as_deref(),
            Some("Could not save the rotation for pw-loupe-turn-failed.ARW: write failed")
        );
    }

    #[test]
    fn a_failed_write_of_an_image_without_a_read_value_turns_back_to_its_own() {
        let path = image("pw-loupe-turn-failed-own.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        let _ = turn(&mut store, &path, true);
        store.embedded_read(&path, Some(1));
        assert_eq!(store.orientation(&path), Some(6));
        store.pump(&mut writer);
        let _ = store.take_changes();
        let id = writer.requests[0].0;

        assert!(
            store
                .written(id, &path, Err("write failed".to_owned()))
                .is_some()
        );

        assert_eq!(store.orientation(&path), None);
        assert_eq!(store.take_changes(), [(path, None)]);
    }

    #[test]
    fn a_failed_write_keeps_a_newer_queued_value() {
        let path = image("pw-loupe-turn-newer.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 1);
        let _ = turn(&mut store, &path, true);
        store.pump(&mut writer);
        let _ = turn(&mut store, &path, true);
        let id = writer.requests[0].0;

        assert_eq!(
            store.written(id, &path, Err("write failed".to_owned())),
            Some(
                "Could not save the rotation for pw-loupe-turn-newer.ARW: write failed".to_owned()
            )
        );
        assert_eq!(store.orientation(&path), Some(3));
    }

    #[test]
    fn a_failed_write_keeps_a_newer_sent_value() {
        let path = image("pw-loupe-turn-newer-sent.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 1);
        let _ = turn(&mut store, &path, true);
        store.pump(&mut writer);
        let _ = turn(&mut store, &path, true);
        store.pump(&mut writer);
        let first_id = writer.requests[0].0;

        assert!(
            store
                .written(first_id, &path, Err("write failed".to_owned()))
                .is_some()
        );
        assert_eq!(store.orientation(&path), Some(3));
    }

    #[test]
    fn a_stopped_worker_turns_back_and_reports_the_rotation_text() {
        let path = image("pw-loupe-turn-stopped.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter {
            responses: [WriteResponse::Stopped].into(),
            ..FakeWriter::default()
        };
        read_value(&mut store, &path, 1, 1);
        let _ = turn(&mut store, &path, true);

        assert_eq!(
            store.pump(&mut writer),
            [
                "Could not save the rotation for pw-loupe-turn-stopped.ARW: metadata writing has stopped"
            ]
        );
        assert_eq!(store.orientation(&path), Some(1));
        assert_eq!(store.pending(), 0);

        // A sent write failed by `fail_sent` turns back as well.
        let _ = turn(&mut store, &path, false);
        store.pump(&mut writer);
        assert_eq!(store.orientation(&path), Some(8));
        assert_eq!(store.fail_sent().len(), 1);
        assert_eq!(store.orientation(&path), Some(1));
        assert_eq!(store.pending(), 0);
    }

    #[test]
    fn a_folder_read_does_not_undo_a_queued_or_newer_sent_turn() {
        let path = image("pw-loupe-turn-read-order.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter {
            next_id: 10,
            ..FakeWriter::default()
        };
        read_value(&mut store, &path, 1, 6);
        let _ = turn(&mut store, &path, true);
        assert_eq!(store.orientation(&path), Some(3));

        read_value(&mut store, &path, 2, 6);
        assert_eq!(store.orientation(&path), Some(3), "queued");

        store.pump(&mut writer);
        assert_eq!(writer.requests[0].0, 10);
        read_value(&mut store, &path, 5, 6);
        assert_eq!(store.orientation(&path), Some(3), "sent after the read");

        read_value(&mut store, &path, 11, 6);
        assert_eq!(store.orientation(&path), Some(6));
    }

    #[test]
    fn a_written_value_is_kept_when_a_later_read_has_no_xmp_value() {
        let path = image("pw-loupe-turn-written.jpg");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        let _ = turn(&mut store, &path, true);
        store.embedded_read(&path, Some(1));
        assert_eq!(store.orientation(&path), Some(6));
        store.pump(&mut writer);
        let id = writer.requests[0].0;

        assert_eq!(store.written(id, &path, Ok(())), None);
        assert_eq!(store.entries[&path].confirmed, Some(6));
        assert_eq!(store.entries[&path].embedded, Some(6));
        assert!(store.entries[&path].confirmed_by_write);

        read_none(&mut store, &path, id + 1);
        assert_eq!(store.orientation(&path), Some(6));
        assert_eq!(store.entries[&path].confirmed, Some(6));

        read_value(&mut store, &path, id + 2, 8);
        assert_eq!(store.orientation(&path), Some(8));
    }

    #[test]
    fn a_written_sidecar_value_is_taken_away_by_a_read_without_it() {
        let path = image("pw-loupe-turn-written.arw");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        let _ = turn(&mut store, &path, true);
        store.embedded_read(&path, Some(1));
        store.pump(&mut writer);
        let id = writer.requests[0].0;

        assert_eq!(store.written(id, &path, Ok(())), None);
        assert_eq!(store.entries[&path].confirmed, Some(6));
        assert_eq!(
            store.entries[&path].embedded,
            Some(1),
            "the RAW is unchanged"
        );
        assert!(!store.entries[&path].confirmed_by_write);

        read_none(&mut store, &path, id + 1);
        assert_eq!(store.orientation(&path), None);
    }

    #[test]
    fn a_successful_write_records_a_change_only_when_the_shown_value_changes() {
        let path = image("pw-loupe-turn-ok-change.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 1);
        let _ = turn(&mut store, &path, true);
        store.pump(&mut writer);
        let _ = store.take_changes();

        assert_eq!(store.written(writer.requests[0].0, &path, Ok(())), None);

        assert_eq!(store.orientation(&path), Some(6));
        assert!(store.take_changes().is_empty());
        assert!(!store.is_pending(&path));
    }

    #[test]
    fn a_read_without_a_value_takes_a_read_value_away() {
        let path = image("pw-loupe-turn-read-away.ARW");
        let mut store = OrientationStore::default();
        read_value(&mut store, &path, 1, 6);
        let _ = store.take_changes();

        read_none(&mut store, &path, 2);

        assert_eq!(store.orientation(&path), None);
        assert_eq!(store.take_changes(), [(path, None)]);
    }

    #[test]
    fn a_read_value_outside_1_to_8_is_taken_as_missing() {
        let path = image("pw-loupe-turn-read-invalid.ARW");
        let mut store = OrientationStore::default();
        read_value(&mut store, &path, 1, 9);
        assert_eq!(store.orientation(&path), None);
        read_value(&mut store, &path, 2, 0);
        assert_eq!(store.orientation(&path), None);
        assert!(store.take_changes().is_empty());
    }

    #[test]
    fn prune_drops_shown_values_as_changes_to_none_and_keeps_pending_paths() {
        let a = image("pw-loupe-turn-prune-a.ARW");
        let b = image("pw-loupe-turn-prune-b.ARW");
        let c = image("pw-loupe-turn-prune-c.ARW");
        let waiting = image("pw-loupe-turn-prune-waiting.ARW");
        let mut store = OrientationStore::default();
        let orientations = [(c.clone(), 8), (a.clone(), 6), (b.clone(), 1)].into();
        store.read(1, &orientations, &[c.clone(), a.clone(), b.clone()]);
        let _ = turn(&mut store, &b, true);
        let _ = turn(&mut store, &waiting, true);
        let _ = store.take_changes();

        store.prune(|_| false);

        assert_eq!(store.take_changes(), [(a.clone(), None), (c.clone(), None)]);
        assert_eq!(store.orientation(&b), Some(6));
        assert!(store.is_pending(&b));
        assert!(!store.entries.contains_key(&a));
        assert!(!store.entries.contains_key(&c));
        assert!(!store.is_waiting(&waiting), "its waiting turns are lost");
        store.embedded_read(&waiting, Some(1));
        assert_eq!(store.orientation(&waiting), None);
    }

    #[test]
    fn unsupported_files_are_skipped_and_only_unsupported_is_refused() {
        let a = image("pw-loupe-turn-skip-a.ARW");
        let png = image("pw-loupe-turn-skip.png");
        let b = image("pw-loupe-turn-skip-b.DNG");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        let orientations = [(a.clone(), 1), (b.clone(), 1)].into();
        store.read(1, &orientations, &[a.clone(), b.clone()]);

        assert_eq!(
            store.turn_many(&[a.clone(), png.clone(), b.clone()], true),
            Ok(vec![])
        );
        store.pump(&mut writer);
        let written: Vec<&PathBuf> = writer.requests.iter().map(|(_, path, _)| path).collect();
        assert_eq!(written, [&a, &b]);
        assert!(!store.entries.contains_key(&png));

        let mut empty = OrientationStore::default();
        assert_eq!(
            empty.turn_many(&[png.clone(), PathBuf::from("notes.txt")], true),
            Err(RatingRefused::Unsupported)
        );
        assert_eq!(empty.turn_many(&[], false), Err(RatingRefused::Unsupported));
        assert_eq!(empty.pending(), 0);
        assert!(empty.entries.is_empty());
        assert!(empty.take_changes().is_empty());
        assert!(empty.waiting_paths().is_empty());
    }

    #[test]
    fn a_turn_resets_the_failures_also_when_it_only_waits() {
        let path = image("pw-loupe-turn-reset.ARW");
        let unknown = image("pw-loupe-turn-reset-unknown.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        read_value(&mut store, &path, 1, 1);
        let _ = turn(&mut store, &path, true);
        store.pump(&mut writer);
        assert!(
            store
                .written(writer.requests[0].0, &path, Err("disk full".to_owned()))
                .is_some()
        );
        assert!(store.failure_text().is_some());

        // A refused turn changes nothing, the failures included.
        assert_eq!(
            store.turn_many(&[image("x.png")], true),
            Err(RatingRefused::Unsupported)
        );
        assert!(store.failure_text().is_some());

        assert_eq!(
            turn(&mut store, &unknown, true),
            std::slice::from_ref(&unknown)
        );
        assert_eq!(store.failure_text(), None);
    }

    #[test]
    fn failure_text_counts_the_failures_since_the_last_turn() {
        let a = image("pw-loupe-turn-failures-a.ARW");
        let b = image("pw-loupe-turn-failures-b.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();
        let orientations = [(a.clone(), 1), (b.clone(), 1)].into();
        store.read(1, &orientations, &[a.clone(), b.clone()]);
        assert_eq!(store.turn_many(&[a.clone(), b.clone()], true), Ok(vec![]));
        store.pump(&mut writer);
        assert_eq!(store.failure_text(), None);

        let _ = store.written(writer.requests[0].0, &a, Err("fel A".to_owned()));
        let _ = store.written(writer.requests[1].0, &b, Err("fel B".to_owned()));

        assert_eq!(
            store.failure_text().as_deref(),
            Some(
                "Could not save the rotation for 2 images. Last: pw-loupe-turn-failures-b.ARW: fel B"
            )
        );
    }

    #[test]
    fn the_reader_reads_a_missing_file_as_none() {
        let reader = EmbeddedOrientationReader::start();
        let missing = std::env::temp_dir().join(format!(
            "pw-loupe-orientation-missing-{}.arw",
            std::process::id()
        ));
        assert!(reader.request(missing.clone()));
        let started = std::time::Instant::now();
        let answer = loop {
            if let Some(answer) = reader.try_recv().expect("the reader runs") {
                break answer;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "no answer");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(answer, (missing, None));
    }

    /// ADR 0024 §2.1, O-Q1 (T237): Ctrl+← anticlockwise, Ctrl+→ clockwise; nothing else.
    #[test]
    fn rotation_keys_match_adr_0024() {
        use egui::Key as K;

        assert_eq!(
            rotation_key_action(K::ArrowLeft, true, false, false),
            Some(false)
        );
        assert_eq!(
            rotation_key_action(K::ArrowRight, true, false, false),
            Some(true)
        );
        for key in [K::ArrowLeft, K::ArrowRight] {
            assert_eq!(rotation_key_action(key, false, false, false), None);
            assert_eq!(rotation_key_action(key, true, true, false), None);
            assert_eq!(rotation_key_action(key, true, false, true), None);
            assert_eq!(rotation_key_action(key, false, true, false), None);
            assert_eq!(rotation_key_action(key, false, false, true), None);
        }
        for key in [K::ArrowUp, K::ArrowDown] {
            assert_eq!(rotation_key_action(key, true, false, false), None);
        }
    }

    /// T250: "Reload Folder" forgets the embedded value of a path without a write, so its next
    /// turn asks for it again; a path with a queued, sent or waiting turn keeps its entry as it
    /// was; no confirmed or shown value changes; a path not given is not touched.
    #[test]
    fn forget_embedded_forgets_only_values_without_a_write() {
        let plain = image("pw-loupe-forget-plain.ARW");
        let queued = image("pw-loupe-forget-queued.ARW");
        let sent = image("pw-loupe-forget-sent.ARW");
        let waiting = image("pw-loupe-forget-waiting.ARW");
        let read = image("pw-loupe-forget-read.ARW");
        let other = image("pw-loupe-forget-other.ARW");
        let mut store = OrientationStore::default();
        let mut writer = FakeWriter::default();

        // `plain`: its embedded value read for turns that cancelled; nothing pending.
        assert_eq!(turn(&mut store, &plain, true), std::slice::from_ref(&plain));
        assert!(turn(&mut store, &plain, false).is_empty());
        store.embedded_read(&plain, Some(6));
        // `sent`: a turn written and not answered.
        read_value(&mut store, &sent, 1, 6);
        let _ = turn(&mut store, &sent, true);
        assert!(store.pump(&mut writer).is_empty());
        // `queued`: a turn not sent yet.
        read_value(&mut store, &queued, 2, 6);
        let _ = turn(&mut store, &queued, true);
        // `waiting`: a turn waiting for its embedded value.
        let _ = turn(&mut store, &waiting, true);
        // `read`: an XMP value read; `other`: not given to `forget_embedded`.
        read_value(&mut store, &read, 3, 8);
        for path in [&sent, &queued, &read, &other] {
            store.entries.entry(path.clone()).or_default().embedded = Some(5);
        }
        let _ = store.take_changes();
        let before: Vec<(Option<u16>, Option<u16>, bool, u8)> = [&queued, &sent, &waiting]
            .iter()
            .map(|path| {
                let entry = &store.entries[*path];
                (entry.embedded, entry.shown, entry.requested, entry.waiting)
            })
            .collect();

        store.forget_embedded(&[
            plain.clone(),
            queued.clone(),
            sent.clone(),
            waiting.clone(),
            read.clone(),
        ]);

        assert_eq!(store.entries[&plain].embedded, None);
        assert_eq!(store.entries[&read].embedded, None);
        assert_eq!(store.entries[&read].confirmed, Some(8));
        assert_eq!(store.orientation(&read), Some(8));
        assert_eq!(store.entries[&other].embedded, Some(5));
        let after: Vec<(Option<u16>, Option<u16>, bool, u8)> = [&queued, &sent, &waiting]
            .iter()
            .map(|path| {
                let entry = &store.entries[*path];
                (entry.embedded, entry.shown, entry.requested, entry.waiting)
            })
            .collect();
        assert_eq!(after, before);
        assert!(store.take_changes().is_empty());

        // The next turn of `plain` asks for its embedded value again.
        assert_eq!(turn(&mut store, &plain, true), std::slice::from_ref(&plain));
    }

    /// What a batch of turns leaves (T268): each image's shown, confirmed and embedded value and
    /// whether RawStride wrote it, the writes pending, the failure count, the failure messages
    /// sorted, the changes since the turn sorted, and the name of the last failure.
    type TurnBatchResult = (
        Vec<(Option<u16>, Option<u16>, Option<u16>, bool)>,
        usize,
        usize,
        Vec<String>,
        Vec<(PathBuf, Option<u16>)>,
        Option<String>,
    );

    /// One clockwise turn of three different images read as 1 (one write succeeds, two fail),
    /// its answers given in `order` (indexes of the images).
    fn turn_batch_answered_in(order: [usize; 3]) -> TurnBatchResult {
        let paths = [
            image("pw-loupe-turn-order-a.jpg"),
            image("pw-loupe-turn-order-b.ARW"),
            image("pw-loupe-turn-order-c.DNG"),
        ];
        let mut store = OrientationStore::default();
        let read: HashMap<PathBuf, u16> = paths.iter().map(|path| (path.clone(), 1)).collect();
        store.read(1, &read, &paths);
        let mut writer = FakeWriter::default();
        assert!(
            store
                .turn_many(&paths, true)
                .expect("the turn is accepted")
                .is_empty()
        );
        store.take_changes();
        assert!(store.pump(&mut writer).is_empty());
        assert_eq!(writer.requests.len(), 3);
        let results = [Ok(()), Err("fel B".to_owned()), Err("fel C".to_owned())];

        let mut messages: Vec<String> = order
            .iter()
            .filter_map(|&index| {
                let (id, path, _) = &writer.requests[index];
                store.written(*id, path, results[index].clone())
            })
            .collect();

        messages.sort();
        let mut changes = store.take_changes();
        changes.sort();
        let values = paths
            .iter()
            .map(|path| {
                let entry = &store.entries[path];
                (
                    entry.shown,
                    entry.confirmed,
                    entry.embedded,
                    entry.confirmed_by_write,
                )
            })
            .collect();
        let last = store.last_failure.as_ref().map(|last| last.name.clone());
        (
            values,
            store.pending(),
            store.failure_count,
            messages,
            changes,
            last,
        )
    }

    /// ADR 0007 §9.6 point 2 (T268): with several write processes the metadata worker may answer
    /// a batch's turns of different images in another order than they were sent; the store ends
    /// the same, the lists filled as answers come and the last failure compared without regard
    /// to order.
    #[test]
    fn a_batchs_turn_answers_in_reverse_order_give_the_same_result() {
        let in_order = turn_batch_answered_in([0, 1, 2]);
        let reverse = turn_batch_answered_in([2, 1, 0]);

        assert_eq!(in_order.0, reverse.0);
        assert_eq!(
            in_order.0,
            [
                (Some(6), Some(6), Some(6), true),
                (Some(1), Some(1), None, false),
                (Some(1), Some(1), None, false),
            ]
        );
        assert_eq!((in_order.1, in_order.2), (0, 2));
        assert_eq!((reverse.1, reverse.2), (0, 2));
        assert_eq!(in_order.3, reverse.3);
        assert_eq!(in_order.4, reverse.4);
        let failed = [
            Some("pw-loupe-turn-order-b.ARW".to_owned()),
            Some("pw-loupe-turn-order-c.DNG".to_owned()),
        ];
        assert!(failed.contains(&in_order.5), "{:?}", in_order.5);
        assert!(failed.contains(&reverse.5), "{:?}", reverse.5);
    }
}
