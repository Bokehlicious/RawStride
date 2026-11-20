//! Star ratings and colour labels in RawStride (ADR 0013): the key map and the optimistic rating
//! store; their texts are in `rating_text.rs` (T116). Plain Rust without I/O (GR11); drawing comes
//! in step B3.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use pw_metadata::{MetadataWorker, Rating, RequestId, SubmitError, supports_rating};

use crate::rating_text::{
    UNSUPPORTED_TEXT, failure_message, failures_text, path_resolution_text, worker_stopped_text,
};

/// A rating or label change requested by a keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatingKey {
    Stars(u8),
    Label(&'static str),
    ClearLabel,
    Reject,
}

/// Maps rating shortcuts without changing the existing loupe or grid key maps.
pub fn rating_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<RatingKey> {
    use egui::Key as K;

    if shift || (ctrl && alt) {
        return None;
    }

    if ctrl {
        return match key {
            K::Num0 => Some(RatingKey::ClearLabel),
            K::Num1 => Some(RatingKey::Label("Red")),
            K::Num2 => Some(RatingKey::Label("Yellow")),
            K::Num3 => Some(RatingKey::Label("Green")),
            K::Num4 => Some(RatingKey::Label("Blue")),
            K::Num5 => Some(RatingKey::Label("Purple")),
            _ => None,
        };
    }

    if alt {
        return match key {
            K::Num0 => Some(RatingKey::Stars(0)),
            K::Num1 => Some(RatingKey::Stars(1)),
            K::Num2 => Some(RatingKey::Stars(2)),
            K::Num3 => Some(RatingKey::Stars(3)),
            K::Num4 => Some(RatingKey::Stars(4)),
            K::Num5 => Some(RatingKey::Stars(5)),
            _ => None,
        };
    }

    match key {
        K::Num0 => Some(RatingKey::Stars(0)),
        K::Num1 => Some(RatingKey::Stars(1)),
        K::Num2 => Some(RatingKey::Stars(2)),
        K::Num3 => Some(RatingKey::Stars(3)),
        K::Num4 => Some(RatingKey::Stars(4)),
        K::Num5 => Some(RatingKey::Stars(5)),
        K::Num6 => Some(RatingKey::Label("Red")),
        K::Num7 => Some(RatingKey::Label("Yellow")),
        K::Num8 => Some(RatingKey::Label("Green")),
        K::Num9 => Some(RatingKey::Label("Blue")),
        K::X => Some(RatingKey::Reject),
        _ => None,
    }
}

/// Applies a rating shortcut while preserving the other half of the rating.
pub fn apply(current: &Rating, key: RatingKey) -> Rating {
    let mut next = current.clone();
    match key {
        RatingKey::Stars(0) => next.stars = None,
        RatingKey::Stars(stars @ 1..=5) => next.stars = Some(stars as i8),
        RatingKey::Stars(_) => {}
        RatingKey::Label(label) => {
            if current.label.as_deref() == Some(label) {
                next.label = None;
            } else {
                next.label = Some(label.to_owned());
            }
        }
        RatingKey::ClearLabel => next.label = None,
        RatingKey::Reject => {
            next.stars = if current.stars == Some(-1) {
                None
            } else {
                Some(-1)
            };
        }
    }
    next
}

/// A metadata request writer, separated from ExifTool so the rating queue can be tested directly.
pub trait RatingWriter {
    fn write_rating(&mut self, path: PathBuf, rating: Rating) -> Result<RequestId, SubmitError>;
}

impl RatingWriter for MetadataWorker {
    fn write_rating(&mut self, path: PathBuf, rating: Rating) -> Result<RequestId, SubmitError> {
        MetadataWorker::write_rating(self, path, rating)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RatingRefused {
    Unsupported,
    PathResolution(String),
}

impl fmt::Display for RatingRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str(UNSUPPORTED_TEXT),
            Self::PathResolution(error) => formatter.write_str(&path_resolution_text(error)),
        }
    }
}

impl std::error::Error for RatingRefused {}

#[derive(Clone, Debug)]
struct PendingWrite {
    path: PathBuf,
    rating: Rating,
}

#[derive(Clone, Debug)]
struct SentWrite {
    path: PathBuf,
    rating: Rating,
}

#[derive(Clone, Debug, Default)]
struct RatingEntry {
    confirmed: Option<Rating>,
    shown: Option<Rating>,
}

/// The file name and the error text of the last failed write.
#[derive(Clone, Debug)]
struct WriteFailure {
    name: String,
    error: String,
}

/// Optimistic per-image ratings and the ordered, non-blocking metadata write queue.
#[derive(Clone, Debug, Default)]
pub struct RatingStore {
    entries: HashMap<PathBuf, RatingEntry>,
    queued: VecDeque<PendingWrite>,
    sent: HashMap<RequestId, SentWrite>,
    /// Write failures since the last accepted `set` or `set_many` (ADR 0014 Q6).
    failure_count: usize,
    last_failure: Option<WriteFailure>,
}

impl RatingStore {
    /// Applies a rating key immediately and queues its value for the metadata worker.
    pub fn set(&mut self, path: &Path, key: RatingKey) -> Result<(), RatingRefused> {
        if !supports_rating(path) {
            return Err(RatingRefused::Unsupported);
        }
        let path = absolute_path(path)?;
        let rating = apply(&self.current(&path), key);
        self.reset_failures();
        self.put(path, rating);
        Ok(())
    }

    /// Applies one rating key to several images with one decision for the whole batch
    /// (ADR 0014 §2.7). Files that cannot carry ratings are skipped. A label or a reject is
    /// cleared only when every image already has it, otherwise it is set on every image; stars
    /// and clearing the label work as `apply`. The other half of each rating is kept. Returns the
    /// number of images set; nothing changes when the batch is refused.
    pub fn set_many(&mut self, paths: &[PathBuf], key: RatingKey) -> Result<usize, RatingRefused> {
        let supported: Vec<&PathBuf> = paths.iter().filter(|path| supports_rating(path)).collect();
        if supported.is_empty() {
            return Err(RatingRefused::Unsupported);
        }
        let targets = supported
            .into_iter()
            .map(|path| absolute_path(path))
            .collect::<Result<Vec<_>, _>>()?;
        let currents: Vec<Rating> = targets.iter().map(|path| self.current(path)).collect();

        let ratings: Vec<Rating> = match key {
            RatingKey::Label(label) => {
                let clear = currents
                    .iter()
                    .all(|rating| rating.label.as_deref() == Some(label));
                currents
                    .into_iter()
                    .map(|mut rating| {
                        rating.label = if clear { None } else { Some(label.to_owned()) };
                        rating
                    })
                    .collect()
            }
            RatingKey::Reject => {
                let clear = currents.iter().all(|rating| rating.stars == Some(-1));
                currents
                    .into_iter()
                    .map(|mut rating| {
                        rating.stars = if clear { None } else { Some(-1) };
                        rating
                    })
                    .collect()
            }
            RatingKey::Stars(_) | RatingKey::ClearLabel => {
                currents.iter().map(|rating| apply(rating, key)).collect()
            }
        };

        self.reset_failures();
        let count = targets.len();
        for (path, rating) in targets.into_iter().zip(ratings) {
            self.put(path, rating);
        }
        Ok(count)
    }

    /// The status-line text for the write failures since the last key (ADR 0014 Q6): `None`
    /// without failures, the message of the failed image for one, and the count plus the last
    /// failure for more.
    pub fn failure_text(&self) -> Option<String> {
        let last = self.last_failure.as_ref()?;
        match self.failure_count {
            0 => None,
            1 => Some(failure_message(&last.name, &last.error)),
            count => Some(failures_text(count, &last.name, &last.error)),
        }
    }

    /// Whether a write for `path` is queued and not yet sent to the metadata worker. The store's
    /// keys are absolute paths, so `path` is passed absolute.
    pub fn is_queued(&self, path: &Path) -> bool {
        self.queued.iter().any(|write| write.path == path)
    }

    /// Whether a write for `path` is queued or sent and has no answer yet (`path` absolute): a move
    /// of the image waits until this is false, so a failed write is rolled back first (T178).
    pub fn is_pending(&self, path: &Path) -> bool {
        self.is_queued(path) || self.sent.values().any(|write| write.path == path)
    }

    /// The rating a key would start from (`path` absolute): the shown value, otherwise the
    /// confirmed one, otherwise the default. A move by X keeps it, so the undo can restore it
    /// (ADR 0017 U9-Q8, T172).
    pub fn current_rating(&self, path: &Path) -> Rating {
        self.current(path)
    }

    /// Shows and queues exactly `rating` for `path`, as `set` does after its key is applied, with
    /// the same checks of the path: used by the undo of an X (ADR 0017 U9-Q8, T172).
    pub fn restore(&mut self, path: &Path, rating: Rating) -> Result<(), RatingRefused> {
        if !supports_rating(path) {
            return Err(RatingRefused::Unsupported);
        }
        let path = absolute_path(path)?;
        self.reset_failures();
        self.put(path, rating);
        Ok(())
    }

    /// The value a key starts from: the shown value, otherwise the confirmed one.
    fn current(&self, path: &Path) -> Rating {
        self.entries
            .get(path)
            .and_then(|entry| entry.shown.as_ref().or(entry.confirmed.as_ref()))
            .cloned()
            .unwrap_or_default()
    }

    /// Shows `rating` at once and queues it, replacing an unsent queued write for the path.
    fn put(&mut self, path: PathBuf, rating: Rating) {
        let entry = self.entries.entry(path.clone()).or_default();
        entry.shown = Some(rating.clone());

        if let Some(queued) = self.queued.iter_mut().find(|write| write.path == path) {
            queued.rating = rating;
        } else {
            self.queued.push_back(PendingWrite { path, rating });
        }
    }

    fn reset_failures(&mut self) {
        self.failure_count = 0;
        self.last_failure = None;
    }

    /// Sends writes in key order. Back-pressure keeps the remaining writes queued.
    pub fn pump(&mut self, writer: &mut impl RatingWriter) -> Vec<String> {
        let mut failures = Vec::new();
        while let Some(write) = self.queued.front().cloned() {
            match writer.write_rating(write.path.clone(), write.rating.clone()) {
                Ok(id) => {
                    let _ = self.queued.pop_front();
                    let _ = self.sent.insert(
                        id,
                        SentWrite {
                            path: write.path,
                            rating: write.rating,
                        },
                    );
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

    /// Completes a sent write and returns a user-facing message when it failed.
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
                if let Some(entry) = self.entries.get_mut(path) {
                    entry.confirmed = Some(write.rating);
                    if !has_newer {
                        entry.shown = entry.confirmed.clone();
                    }
                }
                None
            }
            Err(error) => self.fail_write(path, Some(id), error),
        }
    }

    /// Applies a folder read without replacing a newer optimistic value.
    pub fn read(
        &mut self,
        read_id: RequestId,
        ratings: &HashMap<PathBuf, Rating>,
        paths: &[PathBuf],
    ) {
        for path in paths {
            let entry = self.entries.entry(path.clone()).or_default();
            entry.confirmed = ratings.get(path).cloned();
            let queued = self.queued.iter().any(|write| write.path == *path);
            let newer_sent = self
                .sent
                .iter()
                .any(|(id, write)| *id > read_id && write.path == *path);
            if !queued && !newer_sent {
                entry.shown = entry.confirmed.clone();
            }
        }
    }

    pub fn shown(&self, path: &Path) -> Option<&Rating> {
        self.entries.get(path)?.shown.as_ref()
    }

    pub fn pending(&self) -> usize {
        self.queued.len() + self.sent.len()
    }

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

    pub fn prune(&mut self, keep: impl Fn(&Path) -> bool) {
        let pending = self.pending_paths();
        self.entries
            .retain(|path, _| keep(path) || pending.contains(path));
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

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
            entry.shown = entry.confirmed.clone();
        }
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let message = failure_message(&name, &error);
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_failure = Some(WriteFailure { name, error });
        Some(message)
    }
}

pub(crate) fn absolute_path(path: &Path) -> Result<PathBuf, RatingRefused> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current_dir| current_dir.join(path))
            .map_err(|error| RatingRefused::PathResolution(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{RatingKey, RatingRefused, RatingStore, RatingWriter, apply, rating_key_action};
    use egui::Key as K;
    use pw_metadata::{Rating, RequestId, SubmitError};
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};

    const DIGITS: [K; 10] = [
        K::Num0,
        K::Num1,
        K::Num2,
        K::Num3,
        K::Num4,
        K::Num5,
        K::Num6,
        K::Num7,
        K::Num8,
        K::Num9,
    ];

    fn rating(stars: Option<i8>, label: Option<&str>) -> Rating {
        Rating {
            stars,
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn rating_shortcuts_match_the_accepted_key_map() {
        for (key, expected) in [
            (K::Num0, RatingKey::Stars(0)),
            (K::Num1, RatingKey::Stars(1)),
            (K::Num2, RatingKey::Stars(2)),
            (K::Num3, RatingKey::Stars(3)),
            (K::Num4, RatingKey::Stars(4)),
            (K::Num5, RatingKey::Stars(5)),
            (K::Num6, RatingKey::Label("Red")),
            (K::Num7, RatingKey::Label("Yellow")),
            (K::Num8, RatingKey::Label("Green")),
            (K::Num9, RatingKey::Label("Blue")),
            (K::X, RatingKey::Reject),
        ] {
            assert_eq!(rating_key_action(key, false, false, false), Some(expected));
        }

        for (key, expected) in [
            (K::Num0, RatingKey::Stars(0)),
            (K::Num1, RatingKey::Stars(1)),
            (K::Num2, RatingKey::Stars(2)),
            (K::Num3, RatingKey::Stars(3)),
            (K::Num4, RatingKey::Stars(4)),
            (K::Num5, RatingKey::Stars(5)),
        ] {
            assert_eq!(rating_key_action(key, false, false, true), Some(expected));
        }

        for (key, expected) in [
            (K::Num0, RatingKey::ClearLabel),
            (K::Num1, RatingKey::Label("Red")),
            (K::Num2, RatingKey::Label("Yellow")),
            (K::Num3, RatingKey::Label("Green")),
            (K::Num4, RatingKey::Label("Blue")),
            (K::Num5, RatingKey::Label("Purple")),
        ] {
            assert_eq!(rating_key_action(key, true, false, false), Some(expected));
        }
    }

    #[test]
    fn shift_and_ctrl_alt_rating_keys_are_ignored_and_existing_maps_are_unchanged() {
        for key in [
            K::Num0,
            K::Num1,
            K::Num2,
            K::Num3,
            K::Num4,
            K::Num5,
            K::Num6,
            K::Num7,
            K::Num8,
            K::Num9,
            K::X,
        ] {
            assert_eq!(rating_key_action(key, false, true, false), None, "{key:?}");
            assert_eq!(rating_key_action(key, true, false, true), None, "{key:?}");
        }

        assert_eq!(crate::key_action(K::Num1, false, false), None);
        assert_eq!(crate::grid_key_action(K::Num1, false, false, false), None);
    }

    #[test]
    fn applying_rating_keys_preserves_and_toggles_the_other_value() {
        let current = Rating {
            stars: Some(3),
            label: Some("Red".to_owned()),
        };
        assert_eq!(
            apply(&current, RatingKey::Stars(0)),
            Rating {
                stars: None,
                label: Some("Red".to_owned())
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Stars(5)),
            Rating {
                stars: Some(5),
                label: Some("Red".to_owned())
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Label("Red")),
            Rating {
                stars: Some(3),
                label: None
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Label("Blue")),
            Rating {
                stars: Some(3),
                label: Some("Blue".to_owned())
            }
        );
        assert_eq!(
            apply(&current, RatingKey::ClearLabel),
            Rating {
                stars: Some(3),
                label: None
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Reject),
            Rating {
                stars: Some(-1),
                label: Some("Red".to_owned())
            }
        );
        assert_eq!(
            apply(
                &Rating {
                    stars: Some(-1),
                    label: None
                },
                RatingKey::Reject
            ),
            Rating::default()
        );
    }

    #[test]
    fn modifiers_limit_the_rating_keys() {
        for key in [K::Num6, K::Num7, K::Num8, K::Num9, K::X] {
            assert_eq!(
                rating_key_action(key, false, false, true),
                None,
                "Alt+{key:?}"
            );
            assert_eq!(
                rating_key_action(key, true, false, false),
                None,
                "Ctrl+{key:?}"
            );
        }
        for key in [K::Num0, K::Num5, K::X] {
            assert_eq!(
                rating_key_action(key, true, true, false),
                None,
                "Ctrl+Shift+{key:?}"
            );
            assert_eq!(
                rating_key_action(key, false, true, true),
                None,
                "Alt+Shift+{key:?}"
            );
        }
        for (ctrl, alt) in [(false, false), (true, false), (false, true)] {
            assert_eq!(rating_key_action(K::Q, ctrl, false, alt), None);
            assert_eq!(rating_key_action(K::Enter, ctrl, false, alt), None);
        }
    }

    #[test]
    fn digits_and_x_keep_no_meaning_in_the_loupe_and_grid_key_maps() {
        for key in DIGITS.into_iter().chain([K::X]) {
            assert_eq!(crate::key_action(key, false, false), None, "{key:?}");
            assert_eq!(
                crate::grid_key_action(key, false, false, false),
                None,
                "{key:?}"
            );
            assert_eq!(crate::loupe_key_action(key, false, false), None, "{key:?}");
        }
    }

    #[test]
    fn zero_clears_a_reject_and_stars_replace_it() {
        let rejected = rating(Some(-1), Some("Green"));
        assert_eq!(
            apply(&rejected, RatingKey::Stars(0)),
            rating(None, Some("Green"))
        );
        assert_eq!(
            apply(&rejected, RatingKey::Stars(4)),
            rating(Some(4), Some("Green"))
        );
        assert_eq!(
            apply(&Rating::default(), RatingKey::Stars(0)),
            Rating::default()
        );
    }

    #[test]
    fn labels_toggle_on_an_image_without_a_label_or_with_stars_only() {
        let starred = rating(Some(2), None);
        let labelled = apply(&starred, RatingKey::Label("Purple"));
        assert_eq!(labelled, rating(Some(2), Some("Purple")));
        assert_eq!(apply(&labelled, RatingKey::Label("Purple")), starred);
        assert_eq!(apply(&starred, RatingKey::ClearLabel), starred);
    }

    #[derive(Clone, Copy)]
    enum WriteResponse {
        Accept,
        QueueFull,
        Stopped,
    }

    struct FakeWriter {
        responses: VecDeque<WriteResponse>,
        next_id: RequestId,
        requests: Vec<(RequestId, PathBuf, Rating)>,
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

    impl FakeWriter {
        fn with_responses(responses: impl IntoIterator<Item = WriteResponse>) -> Self {
            Self {
                responses: responses.into_iter().collect(),
                next_id: 1,
                requests: Vec::new(),
            }
        }
    }

    impl RatingWriter for FakeWriter {
        fn write_rating(
            &mut self,
            path: PathBuf,
            rating: Rating,
        ) -> Result<RequestId, SubmitError> {
            match self.responses.pop_front().unwrap_or(WriteResponse::Accept) {
                WriteResponse::Accept => {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.requests.push((id, path, rating));
                    Ok(id)
                }
                WriteResponse::QueueFull => Err(SubmitError::QueueFull),
                WriteResponse::Stopped => Err(SubmitError::Stopped),
            }
        }
    }

    fn image(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn loaded_rating(stars: i8) -> Rating {
        Rating {
            stars: Some(stars),
            label: None,
        }
    }

    fn read_rating(store: &mut RatingStore, path: &Path, id: RequestId, stars: i8) {
        let ratings = [(path.to_path_buf(), loaded_rating(stars))].into();
        store.read(id, &ratings, &[path.to_path_buf()]);
    }

    #[test]
    fn set_shows_the_rating_immediately() {
        let path = image("pw-loupe-rating-immediate.ARW");
        let mut store = RatingStore::default();

        assert_eq!(store.set(&path, RatingKey::Stars(3)), Ok(()));

        assert_eq!(store.shown(&path).and_then(|rating| rating.stars), Some(3));
        assert_eq!(store.pending(), 1);
    }

    #[test]
    fn unsupported_rating_is_refused_without_queueing() {
        let mut store = RatingStore::default();

        assert_eq!(
            store.set(Path::new("notes.txt"), RatingKey::Stars(3)),
            Err(RatingRefused::Unsupported)
        );
        assert_eq!(store.pending(), 0);
        assert!(store.pending_paths().is_empty());
    }

    #[test]
    fn queued_key_updates_coalesce_and_keep_the_latest_value() {
        let path = image("pw-loupe-rating-coalesced.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        for stars in 1..=5 {
            assert_eq!(store.set(&path, RatingKey::Stars(stars)), Ok(()));
        }

        assert_eq!(store.pending(), 1);
        store.pump(&mut writer);
        assert_eq!(writer.requests.len(), 1);
        assert_eq!(writer.requests[0].2, loaded_rating(5));
        assert_eq!(store.shown(&path), Some(&loaded_rating(5)));
    }

    #[test]
    fn writes_keep_key_order_across_paths() {
        let first = image("pw-loupe-rating-order-1.ARW");
        let second = image("pw-loupe-rating-order-2.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        assert_eq!(store.set(&first, RatingKey::Stars(1)), Ok(()));
        assert_eq!(store.set(&second, RatingKey::Stars(2)), Ok(()));

        store.pump(&mut writer);

        assert_eq!(writer.requests.len(), 2);
        assert_eq!(writer.requests[0].1, first);
        assert_eq!(writer.requests[1].1, second);
    }

    #[test]
    fn queue_full_leaves_the_rest_for_the_next_pump() {
        let first = image("pw-loupe-rating-full-1.ARW");
        let second = image("pw-loupe-rating-full-2.ARW");
        let mut store = RatingStore::default();
        let mut writer =
            FakeWriter::with_responses([WriteResponse::Accept, WriteResponse::QueueFull]);
        assert_eq!(store.set(&first, RatingKey::Stars(1)), Ok(()));
        assert_eq!(store.set(&second, RatingKey::Stars(2)), Ok(()));

        assert!(store.pump(&mut writer).is_empty());
        assert_eq!(writer.requests.len(), 1);
        assert_eq!(store.pending(), 2);
        assert!(store.pump(&mut writer).is_empty());
        assert_eq!(writer.requests.len(), 2);
        assert_eq!(writer.requests[1].1, second);
        assert_eq!(store.pending(), 2);
    }

    #[test]
    fn stopped_write_rolls_back_and_reports_the_error() {
        let path = image("pw-loupe-rating-stopped.ARW");
        let mut store = RatingStore::default();
        read_rating(&mut store, &path, 1, 2);
        assert_eq!(store.set(&path, RatingKey::Stars(4)), Ok(()));
        let mut writer = FakeWriter::with_responses([WriteResponse::Stopped]);

        let failures = store.pump(&mut writer);

        assert_eq!(store.shown(&path), Some(&loaded_rating(2)));
        assert_eq!(store.pending(), 0);
        assert_eq!(
            failures,
            [
                "Could not save the rating for pw-loupe-rating-stopped.ARW: metadata writing has stopped"
            ]
        );
    }

    #[test]
    fn failed_write_keeps_a_newer_queued_value() {
        let path = image("pw-loupe-rating-newer.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        read_rating(&mut store, &path, 1, 1);
        assert_eq!(store.set(&path, RatingKey::Stars(2)), Ok(()));
        store.pump(&mut writer);
        assert_eq!(store.set(&path, RatingKey::Stars(3)), Ok(()));
        let id = writer.requests[0].0;

        assert_eq!(
            store.written(id, &path, Err("write failed".to_owned())),
            Some(
                "Could not save the rating for pw-loupe-rating-newer.ARW: write failed".to_owned()
            )
        );
        assert_eq!(store.shown(&path), Some(&loaded_rating(3)));
    }

    #[test]
    fn failed_write_keeps_a_newer_sent_value() {
        let path = image("pw-loupe-rating-newer-sent.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        read_rating(&mut store, &path, 1, 1);
        assert_eq!(store.set(&path, RatingKey::Stars(2)), Ok(()));
        store.pump(&mut writer);
        assert_eq!(store.set(&path, RatingKey::Stars(3)), Ok(()));
        store.pump(&mut writer);
        let first_id = writer.requests[0].0;

        assert!(
            store
                .written(first_id, &path, Err("write failed".to_owned()))
                .is_some()
        );
        assert_eq!(store.shown(&path), Some(&loaded_rating(3)));
    }

    #[test]
    fn successful_write_becomes_confirmed_for_a_later_rollback() {
        let path = image("pw-loupe-rating-confirmed.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        read_rating(&mut store, &path, 1, 1);
        assert_eq!(store.set(&path, RatingKey::Stars(2)), Ok(()));
        store.pump(&mut writer);
        let first_id = writer.requests[0].0;
        assert_eq!(store.written(first_id, &path, Ok(())), None);

        assert_eq!(store.set(&path, RatingKey::Stars(3)), Ok(()));
        store.pump(&mut writer);
        let second_id = writer.requests[1].0;
        assert!(
            store
                .written(second_id, &path, Err("write failed".to_owned()))
                .is_some()
        );

        assert_eq!(store.shown(&path), Some(&loaded_rating(2)));
    }

    #[test]
    fn failed_write_without_a_newer_value_rolls_back_to_confirmed() {
        let path = image("pw-loupe-rating-rollback.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        read_rating(&mut store, &path, 1, 2);
        assert_eq!(store.set(&path, RatingKey::Stars(4)), Ok(()));
        store.pump(&mut writer);
        let id = writer.requests[0].0;

        assert!(
            store
                .written(id, &path, Err("write failed".to_owned()))
                .is_some()
        );
        assert_eq!(store.shown(&path), Some(&loaded_rating(2)));
    }

    #[test]
    fn read_updates_shown_only_when_its_id_is_newer_than_sent_writes() {
        let path = image("pw-loupe-rating-read-order.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter {
            next_id: 5,
            ..FakeWriter::default()
        };
        read_rating(&mut store, &path, 1, 1);
        assert_eq!(store.set(&path, RatingKey::Stars(2)), Ok(()));
        store.pump(&mut writer);

        read_rating(&mut store, &path, 4, 4);
        assert_eq!(store.shown(&path), Some(&loaded_rating(2)));
        read_rating(&mut store, &path, 6, 5);
        assert_eq!(store.shown(&path), Some(&loaded_rating(5)));
    }

    #[test]
    fn read_does_not_change_a_path_with_a_queued_write() {
        let path = image("pw-loupe-rating-read-queued.ARW");
        let mut store = RatingStore::default();
        read_rating(&mut store, &path, 1, 1);
        assert_eq!(store.set(&path, RatingKey::Stars(2)), Ok(()));

        read_rating(&mut store, &path, 3, 4);

        assert_eq!(store.shown(&path), Some(&loaded_rating(2)));
    }

    #[test]
    fn prune_keeps_entries_with_pending_writes() {
        let sent = image("pw-loupe-rating-prune-sent.ARW");
        let queued = image("pw-loupe-rating-prune-queued.ARW");
        let unused = image("pw-loupe-rating-prune-unused.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        assert_eq!(store.set(&sent, RatingKey::Stars(2)), Ok(()));
        store.pump(&mut writer);
        assert_eq!(store.set(&queued, RatingKey::Stars(4)), Ok(()));
        read_rating(&mut store, &unused, 1, 3);

        store.prune(|_| false);

        assert!(store.contains(&sent));
        assert!(store.contains(&queued));
        assert!(!store.contains(&unused));
    }

    /// Loads the given ratings as a folder read, so they are both confirmed and shown.
    fn read_ratings(store: &mut RatingStore, values: &[(&Path, Rating)]) {
        let ratings = values
            .iter()
            .map(|(path, rating)| (path.to_path_buf(), rating.clone()))
            .collect();
        let paths: Vec<PathBuf> = values.iter().map(|(path, _)| path.to_path_buf()).collect();
        store.read(1, &ratings, &paths);
    }

    fn shown(store: &RatingStore, path: &Path) -> Rating {
        store.shown(path).cloned().unwrap_or_default()
    }

    #[test]
    fn batch_label_on_a_mix_sets_it_on_all_and_keeps_the_stars() {
        let a = image("pw-loupe-batch-label-mix-a.ARW");
        let b = image("pw-loupe-batch-label-mix-b.ARW");
        let c = image("pw-loupe-batch-label-mix-c.DNG");
        let mut store = RatingStore::default();
        read_ratings(
            &mut store,
            &[
                (&a, rating(Some(3), None)),
                (&b, rating(None, Some("Yellow"))),
                (&c, rating(Some(-1), Some("Red"))),
            ],
        );

        assert_eq!(
            store.set_many(
                &[a.clone(), b.clone(), c.clone()],
                RatingKey::Label("Yellow")
            ),
            Ok(3)
        );

        assert_eq!(shown(&store, &a), rating(Some(3), Some("Yellow")));
        assert_eq!(shown(&store, &b), rating(None, Some("Yellow")));
        assert_eq!(shown(&store, &c), rating(Some(-1), Some("Yellow")));
        assert_eq!(store.pending(), 3);
    }

    #[test]
    fn batch_label_on_all_that_have_it_clears_it_on_all() {
        let a = image("pw-loupe-batch-label-all-a.ARW");
        let b = image("pw-loupe-batch-label-all-b.ARW");
        let mut store = RatingStore::default();
        read_ratings(
            &mut store,
            &[
                (&a, rating(Some(2), Some("Yellow"))),
                (&b, rating(None, Some("Yellow"))),
            ],
        );

        assert_eq!(
            store.set_many(&[a.clone(), b.clone()], RatingKey::Label("Yellow")),
            Ok(2)
        );

        assert_eq!(shown(&store, &a), rating(Some(2), None));
        assert_eq!(shown(&store, &b), rating(None, None));
    }

    #[test]
    fn batch_reject_on_a_mix_rejects_all_and_keeps_the_labels() {
        let a = image("pw-loupe-batch-reject-mix-a.ARW");
        let b = image("pw-loupe-batch-reject-mix-b.ARW");
        let c = image("pw-loupe-batch-reject-mix-c.ARW");
        let mut store = RatingStore::default();
        read_ratings(
            &mut store,
            &[
                (&a, rating(Some(3), Some("Red"))),
                (&b, rating(Some(-1), None)),
                (&c, rating(None, Some("Blue"))),
            ],
        );

        assert_eq!(
            store.set_many(&[a.clone(), b.clone(), c.clone()], RatingKey::Reject),
            Ok(3)
        );

        assert_eq!(shown(&store, &a), rating(Some(-1), Some("Red")));
        assert_eq!(shown(&store, &b), rating(Some(-1), None));
        assert_eq!(shown(&store, &c), rating(Some(-1), Some("Blue")));
    }

    #[test]
    fn batch_reject_on_all_rejected_clears_the_stars_and_keeps_the_labels() {
        let a = image("pw-loupe-batch-reject-all-a.ARW");
        let b = image("pw-loupe-batch-reject-all-b.ARW");
        let mut store = RatingStore::default();
        read_ratings(
            &mut store,
            &[
                (&a, rating(Some(-1), Some("Green"))),
                (&b, rating(Some(-1), None)),
            ],
        );

        assert_eq!(
            store.set_many(&[a.clone(), b.clone()], RatingKey::Reject),
            Ok(2)
        );

        assert_eq!(shown(&store, &a), rating(None, Some("Green")));
        assert_eq!(shown(&store, &b), rating(None, None));
    }

    #[test]
    fn batch_stars_and_clear_label_work_as_apply_on_each_image() {
        let a = image("pw-loupe-batch-stars-a.ARW");
        let b = image("pw-loupe-batch-stars-b.ARW");
        let mut store = RatingStore::default();
        read_ratings(
            &mut store,
            &[
                (&a, rating(Some(-1), Some("Red"))),
                (&b, rating(None, None)),
            ],
        );
        let paths = [a.clone(), b.clone()];

        assert_eq!(store.set_many(&paths, RatingKey::Stars(3)), Ok(2));
        assert_eq!(shown(&store, &a), rating(Some(3), Some("Red")));
        assert_eq!(shown(&store, &b), rating(Some(3), None));

        assert_eq!(store.set_many(&paths, RatingKey::ClearLabel), Ok(2));
        assert_eq!(shown(&store, &a), rating(Some(3), None));
        assert_eq!(shown(&store, &b), rating(Some(3), None));

        assert_eq!(store.set_many(&paths, RatingKey::Stars(0)), Ok(2));
        assert_eq!(shown(&store, &a), rating(None, None));
        assert_eq!(shown(&store, &b), rating(None, None));
    }

    #[test]
    fn batch_skips_files_that_cannot_carry_ratings() {
        let a = image("pw-loupe-batch-skip-a.ARW");
        let png = image("x.png");
        let b = image("pw-loupe-batch-skip-b.DNG");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();

        assert_eq!(
            store.set_many(&[a.clone(), png.clone(), b.clone()], RatingKey::Stars(2)),
            Ok(2)
        );
        store.pump(&mut writer);

        let written: Vec<&PathBuf> = writer.requests.iter().map(|(_, path, _)| path).collect();
        assert_eq!(written, [&a, &b]);
        assert!(!store.contains(&png));
    }

    #[test]
    fn batch_of_only_unsupported_files_is_refused_and_nothing_is_queued() {
        let mut store = RatingStore::default();

        assert_eq!(
            store.set_many(
                &[image("x.png"), PathBuf::from("notes.txt")],
                RatingKey::Label("Red")
            ),
            Err(RatingRefused::Unsupported)
        );
        assert_eq!(
            store.set_many(&[], RatingKey::Stars(1)),
            Err(RatingRefused::Unsupported)
        );

        assert_eq!(store.pending(), 0);
        assert!(store.pending_paths().is_empty());
        assert!(!store.contains(&image("x.png")));
    }

    #[test]
    fn batch_writes_follow_the_order_of_the_paths() {
        let a = image("pw-loupe-batch-order-a.ARW");
        let b = image("pw-loupe-batch-order-b.ARW");
        let c = image("pw-loupe-batch-order-c.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();

        assert_eq!(
            store.set_many(&[c.clone(), a.clone(), b.clone()], RatingKey::Stars(4)),
            Ok(3)
        );
        store.pump(&mut writer);

        let written: Vec<&PathBuf> = writer.requests.iter().map(|(_, path, _)| path).collect();
        assert_eq!(written, [&c, &a, &b]);
    }

    #[test]
    fn two_batches_before_a_pump_coalesce_per_path() {
        let a = image("pw-loupe-batch-coalesce-a.ARW");
        let b = image("pw-loupe-batch-coalesce-b.ARW");
        let c = image("pw-loupe-batch-coalesce-c.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();

        assert_eq!(
            store.set_many(&[a.clone(), b.clone()], RatingKey::Stars(2)),
            Ok(2)
        );
        assert_eq!(
            store.set_many(&[b.clone(), c.clone()], RatingKey::Label("Red")),
            Ok(2)
        );
        assert_eq!(store.pending(), 3);
        store.pump(&mut writer);

        let requests: Vec<(PathBuf, Rating)> = writer
            .requests
            .iter()
            .map(|(_, path, rating)| (path.clone(), rating.clone()))
            .collect();
        assert_eq!(
            requests,
            [
                (a, rating(Some(2), None)),
                (b, rating(Some(2), Some("Red"))),
                (c, rating(None, Some("Red"))),
            ]
        );
    }

    #[test]
    fn one_failed_image_of_a_batch_rolls_back_only_that_image() {
        let a = image("pw-loupe-batch-rollback-a.ARW");
        let b = image("pw-loupe-batch-rollback-b.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        read_ratings(
            &mut store,
            &[(&a, loaded_rating(1)), (&b, loaded_rating(1))],
        );
        assert_eq!(
            store.set_many(&[a.clone(), b.clone()], RatingKey::Stars(4)),
            Ok(2)
        );
        store.pump(&mut writer);
        let (id_a, id_b) = (writer.requests[0].0, writer.requests[1].0);

        assert!(
            store
                .written(id_a, &a, Err("write failed".to_owned()))
                .is_some()
        );
        assert_eq!(store.written(id_b, &b, Ok(())), None);

        assert_eq!(store.shown(&a), Some(&loaded_rating(1)));
        assert_eq!(store.shown(&b), Some(&loaded_rating(4)));
        assert_eq!(store.pending(), 0);
    }

    #[test]
    fn batch_with_one_path_equals_set_for_every_key() {
        let path = image("pw-loupe-batch-single.ARW");
        let mut keys: Vec<RatingKey> = (0..=5).map(RatingKey::Stars).collect();
        keys.extend(pw_metadata::LABELS.iter().copied().map(RatingKey::Label));
        keys.extend([RatingKey::ClearLabel, RatingKey::Reject]);
        let starts = [
            Rating::default(),
            rating(Some(3), None),
            rating(Some(-1), Some("Red")),
            rating(None, Some("Yellow")),
            rating(Some(5), Some("Purple")),
            rating(Some(-1), None),
        ];

        for start in &starts {
            for key in &keys {
                let mut single = RatingStore::default();
                let mut batch = RatingStore::default();
                read_ratings(&mut single, &[(&path, start.clone())]);
                read_ratings(&mut batch, &[(&path, start.clone())]);

                assert_eq!(single.set(&path, *key), Ok(()));
                assert_eq!(batch.set_many(std::slice::from_ref(&path), *key), Ok(1));
                assert_eq!(batch.shown(&path), single.shown(&path), "{start:?} {key:?}");

                let mut single_writer = FakeWriter::default();
                let mut batch_writer = FakeWriter::default();
                single.pump(&mut single_writer);
                batch.pump(&mut batch_writer);
                assert_eq!(
                    batch_writer.requests, single_writer.requests,
                    "{start:?} {key:?}"
                );
            }
        }
    }

    #[test]
    fn failure_text_for_zero_one_and_three_failures_and_reset_by_the_next_key() {
        let a = image("pw-loupe-batch-failure-a.ARW");
        let b = image("pw-loupe-batch-failure-b.ARW");
        let c = image("pw-loupe-batch-failure-c.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        assert_eq!(
            store.set_many(&[a.clone(), b.clone(), c.clone()], RatingKey::Stars(2)),
            Ok(3)
        );
        store.pump(&mut writer);
        let ids: Vec<RequestId> = writer.requests.iter().map(|(id, _, _)| *id).collect();

        assert_eq!(store.failure_text(), None);

        assert_eq!(
            store.written(ids[0], &a, Err("fel A".to_owned())),
            Some("Could not save the rating for pw-loupe-batch-failure-a.ARW: fel A".to_owned())
        );
        assert_eq!(
            store.failure_text().as_deref(),
            Some("Could not save the rating for pw-loupe-batch-failure-a.ARW: fel A")
        );

        // The per-write messages are unchanged; only `failure_text` sums them up.
        assert_eq!(
            store.written(ids[1], &b, Err("fel B".to_owned())),
            Some("Could not save the rating for pw-loupe-batch-failure-b.ARW: fel B".to_owned())
        );
        assert_eq!(
            store.written(ids[2], &c, Err("fel C".to_owned())),
            Some("Could not save the rating for pw-loupe-batch-failure-c.ARW: fel C".to_owned())
        );
        assert_eq!(
            store.failure_text().as_deref(),
            Some(
                "Could not save the rating for 3 images. Last: pw-loupe-batch-failure-c.ARW: fel C"
            )
        );

        // A refused key changes nothing, the failures included.
        assert_eq!(
            store.set_many(&[image("x.png")], RatingKey::Stars(1)),
            Err(RatingRefused::Unsupported)
        );
        assert!(store.failure_text().is_some());

        assert_eq!(store.set(&a, RatingKey::Stars(1)), Ok(()));
        assert_eq!(store.failure_text(), None);

        // `set_many` resets the count as well; a stopped worker counts as a failure.
        let mut stopped = FakeWriter::with_responses([WriteResponse::Stopped]);
        assert_eq!(store.pump(&mut stopped).len(), 1);
        assert!(store.failure_text().is_some());
        assert_eq!(
            store.set_many(std::slice::from_ref(&b), RatingKey::Stars(5)),
            Ok(1)
        );
        assert_eq!(store.failure_text(), None);
    }

    #[test]
    fn is_queued_until_the_write_is_pumped() {
        let path = image("pw-loupe-rating-is-queued.ARW");
        let other = image("pw-loupe-rating-is-queued-other.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::with_responses([WriteResponse::QueueFull]);
        assert!(!store.is_queued(&path));

        assert_eq!(store.set(&path, RatingKey::Stars(2)), Ok(()));
        assert!(store.is_queued(&path));
        assert!(!store.is_queued(&other));

        // A full worker queue keeps it queued; the next pump sends it.
        assert!(store.pump(&mut writer).is_empty());
        assert!(store.is_queued(&path));
        assert!(store.pump(&mut writer).is_empty());
        assert!(!store.is_queued(&path));
        assert_eq!(store.pending(), 1, "sent, waiting for its answer");
    }

    #[test]
    fn is_pending_until_the_write_has_its_answer() {
        let path = image("pw-loupe-rating-is-pending.ARW");
        let other = image("pw-loupe-rating-is-pending-other.ARW");
        for result in [Ok(()), Err("disk full".to_owned())] {
            let mut store = RatingStore::default();
            let mut writer = FakeWriter::default();
            assert!(!store.is_pending(&path));

            assert_eq!(store.set(&path, RatingKey::Reject), Ok(()));
            assert!(store.is_pending(&path), "queued");
            assert!(!store.is_pending(&other));
            assert!(store.pump(&mut writer).is_empty());
            assert!(!store.is_queued(&path));
            assert!(store.is_pending(&path), "sent, waiting for its answer");

            let failed = result.is_err();
            let (id, written_path, _) = writer.requests[0].clone();
            assert_eq!(store.written(id, &written_path, result).is_some(), failed);
            assert!(!store.is_pending(&path), "answered, failed: {failed}");
        }
    }

    #[test]
    fn current_rating_is_the_shown_then_the_confirmed_then_the_default() {
        let unread = image("pw-loupe-rating-current-unread.ARW");
        let path = image("pw-loupe-rating-current.ARW");
        let mut store = RatingStore::default();
        assert_eq!(store.current_rating(&unread), Rating::default());

        read_rating(&mut store, &path, 1, 2);
        assert_eq!(store.current_rating(&path), loaded_rating(2));
        assert_eq!(store.set(&path, RatingKey::Reject), Ok(()));
        assert_eq!(store.current_rating(&path), loaded_rating(-1));
    }

    #[test]
    fn restore_shows_and_queues_the_exact_rating() {
        let path = image("pw-loupe-rating-restore.ARW");
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();
        read_rating(&mut store, &path, 1, 4);
        assert_eq!(store.set(&path, RatingKey::Reject), Ok(()));
        store.pump(&mut writer);

        let before = rating(Some(4), Some("Green"));
        assert_eq!(store.restore(&path, before.clone()), Ok(()));
        assert_eq!(store.shown(&path), Some(&before));
        assert!(store.is_queued(&path));
        store.pump(&mut writer);
        assert_eq!(writer.requests.len(), 2);
        assert_eq!(writer.requests[1].1, path);
        assert_eq!(writer.requests[1].2, before);

        // The same checks of the path as `set`.
        assert_eq!(
            store.restore(Path::new("notes.txt"), Rating::default()),
            Err(RatingRefused::Unsupported)
        );
    }

    /// The `app.js` case: five marked images, one of them unsupported.
    #[test]
    fn yellow_on_five_marked_images_sets_then_clears_it_on_the_four_ratable_ones() {
        let paths = [
            image("pw-loupe-batch-app-1.ARW"),
            image("pw-loupe-batch-app-2.RW2"),
            image("pw-loupe-batch-app-3.png"),
            image("pw-loupe-batch-app-4.DNG"),
            image("pw-loupe-batch-app-5.JPG"),
        ];
        let ratable: Vec<&PathBuf> = paths
            .iter()
            .filter(|path| path.extension().is_some_and(|ext| ext != "png"))
            .collect();
        let mut store = RatingStore::default();
        let mut writer = FakeWriter::default();

        assert_eq!(store.set_many(&paths, RatingKey::Label("Yellow")), Ok(4));
        store.pump(&mut writer);
        assert_eq!(writer.requests.len(), 4);
        for ((id, path, written), expected) in writer.requests.clone().into_iter().zip(&ratable) {
            assert_eq!(&&path, expected);
            assert_eq!(written, rating(None, Some("Yellow")));
            assert_eq!(store.written(id, &path, Ok(())), None);
        }

        assert_eq!(store.set_many(&paths, RatingKey::Label("Yellow")), Ok(4));
        store.pump(&mut writer);
        assert_eq!(writer.requests.len(), 8);
        for ((_, path, written), expected) in writer.requests[4..].iter().zip(&ratable) {
            assert_eq!(&path, expected);
            assert_eq!(written, &rating(None, None));
        }
    }
}
