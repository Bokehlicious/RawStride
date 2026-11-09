//! Star ratings and colour labels in RawStride (ADR 0013): the key map, optimistic rating store,
//! and Swedish rating texts. Plain Rust without I/O (GR11); drawing comes in step B3.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use pw_metadata::{MetadataWorker, Rating, RequestId, SubmitError, supports_rating};

/// The status-line message when a rating key is pressed on a file that cannot carry ratings.
pub const UNSUPPORTED_TEXT: &str = "Betyg och etiketter kan inte sparas för den här filtypen.";

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
            Self::PathResolution(error) => {
                write!(formatter, "Kunde inte slå upp bildens sökväg: {error}")
            }
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

/// Optimistic per-image ratings and the ordered, non-blocking metadata write queue.
#[derive(Clone, Debug, Default)]
pub struct RatingStore {
    entries: HashMap<PathBuf, RatingEntry>,
    queued: VecDeque<PendingWrite>,
    sent: HashMap<RequestId, SentWrite>,
}

impl RatingStore {
    /// Applies a rating key immediately and queues its value for the metadata worker.
    pub fn set(&mut self, path: &Path, key: RatingKey) -> Result<(), RatingRefused> {
        if !supports_rating(path) {
            return Err(RatingRefused::Unsupported);
        }
        let path = absolute_path(path)?;
        let entry = self.entries.entry(path.clone()).or_default();
        let current = entry
            .shown
            .as_ref()
            .or(entry.confirmed.as_ref())
            .cloned()
            .unwrap_or_default();
        let rating = apply(&current, key);
        entry.shown = Some(rating.clone());

        if let Some(queued) = self.queued.iter_mut().find(|write| write.path == path) {
            queued.rating = rating;
        } else {
            self.queued.push_back(PendingWrite { path, rating });
        }
        Ok(())
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
                    if let Some(message) = self.fail_write(
                        &write.path,
                        None,
                        "metadataarbetaren har stannat".to_owned(),
                    ) {
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
                self.fail_write(
                    &write.path,
                    Some(id),
                    "metadataarbetaren har stannat".to_owned(),
                )
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
            .to_string_lossy();
        Some(format!("Kunde inte spara betyget för {name}: {error}"))
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, RatingRefused> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current_dir| current_dir.join(path))
            .map_err(|error| RatingRefused::PathResolution(error.to_string()))
    }
}

/// The Swedish name of an Adobe label name; an unknown label is returned as it is.
pub fn label_name(label: &str) -> &str {
    match label {
        "Red" => "Röd",
        "Yellow" => "Gul",
        "Green" => "Grön",
        "Blue" => "Blå",
        "Purple" => "Lila",
        other => other,
    }
}

/// The status-line text of a rating, for example "Betyg: 3 stjärnor, Röd".
pub fn status_text(rating: Option<&Rating>) -> String {
    let stars = rating.and_then(|rating| rating.stars);
    let mut text = match stars {
        Some(-1) => "Betyg: Avvisad".to_owned(),
        Some(1) => "Betyg: 1 stjärna".to_owned(),
        Some(stars) if stars > 1 => format!("Betyg: {stars} stjärnor"),
        _ => "Betyg: Ej betygsatt".to_owned(),
    };
    if let Some(label) = rating.and_then(|rating| rating.label.as_deref()) {
        text.push_str(", ");
        text.push_str(label_name(label));
    }
    text
}

/// The stars drawn under a thumbnail: "★★★••" for three stars, "✕" for rejected, nothing when
/// unrated.
pub fn star_glyphs(stars: Option<i8>) -> String {
    match stars {
        Some(-1) => "✕".to_owned(),
        Some(stars @ 1..=5) => {
            let filled = usize::from(stars.unsigned_abs());
            format!("{}{}", "★".repeat(filled), "•".repeat(5 - filled))
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RatingKey, RatingRefused, RatingStore, RatingWriter, UNSUPPORTED_TEXT, apply, label_name,
        rating_key_action, star_glyphs, status_text,
    };
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
        assert_eq!(crate::grid_key_action(K::Num1, false, false), None);
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
            assert_eq!(crate::grid_key_action(key, false, false), None, "{key:?}");
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

    #[test]
    fn swedish_label_names() {
        for (label, name) in [
            ("Red", "Röd"),
            ("Yellow", "Gul"),
            ("Green", "Grön"),
            ("Blue", "Blå"),
            ("Purple", "Lila"),
            ("Orange", "Orange"),
            ("", ""),
        ] {
            assert_eq!(label_name(label), name);
        }
        for label in pw_metadata::LABELS {
            assert_ne!(label_name(label), *label, "{label} has a Swedish name");
        }
    }

    #[test]
    fn status_texts() {
        for (value, text) in [
            (None, "Betyg: Ej betygsatt"),
            (Some(rating(None, None)), "Betyg: Ej betygsatt"),
            (Some(rating(Some(-1), None)), "Betyg: Avvisad"),
            (Some(rating(Some(1), None)), "Betyg: 1 stjärna"),
            (Some(rating(Some(3), Some("Red"))), "Betyg: 3 stjärnor, Röd"),
            (Some(rating(Some(5), None)), "Betyg: 5 stjärnor"),
            (
                Some(rating(Some(-1), Some("Yellow"))),
                "Betyg: Avvisad, Gul",
            ),
            (Some(rating(None, Some("Blue"))), "Betyg: Ej betygsatt, Blå"),
            (
                Some(rating(None, Some("Orange"))),
                "Betyg: Ej betygsatt, Orange",
            ),
        ] {
            assert_eq!(status_text(value.as_ref()), text);
        }
    }

    #[test]
    fn star_glyphs_for_each_value() {
        assert_eq!(star_glyphs(None), "");
        assert_eq!(star_glyphs(Some(-1)), "✕");
        assert_eq!(star_glyphs(Some(1)), "★••••");
        assert_eq!(star_glyphs(Some(3)), "★★★••");
        assert_eq!(star_glyphs(Some(5)), "★★★★★");
        // Values that cannot come from the keys or `read_ratings` draw nothing instead of panicking.
        assert_eq!(star_glyphs(Some(0)), "");
        assert_eq!(star_glyphs(Some(6)), "");
        assert_eq!(star_glyphs(Some(-2)), "");
    }

    #[test]
    fn unsupported_text_is_swedish() {
        assert_eq!(
            UNSUPPORTED_TEXT,
            "Betyg och etiketter kan inte sparas för den här filtypen."
        );
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
                "Kunde inte spara betyget för pw-loupe-rating-stopped.ARW: metadataarbetaren har stannat"
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
            Some("Kunde inte spara betyget för pw-loupe-rating-newer.ARW: write failed".to_owned())
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
}
