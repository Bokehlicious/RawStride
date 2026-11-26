//! The sort order of the contact sheet (ADR 0017 §2.1, §6.1, U14, T138): the sort keys, the
//! ordering of a tab's files by one of them, the capture times a tab has read, and the reader
//! that reads them with `pw_raw::inspect` on a thread of its own. The ordering is plain Rust
//! (GR11): the values a key needs come through its arguments.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::SystemTime;

use pw_metadata::{LABELS, Rating};

/// What the files are sorted by (U14-Q1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    /// The file name, ignoring case: the order `list_folder` gives (the default).
    #[default]
    Name,
    /// Exif DateTimeOriginal, read with `pw_raw::inspect`.
    Capture,
    /// The file's modification time.
    Modified,
    /// The extension, ignoring case, then the name.
    Type,
    /// The stars, then the label in the order of the label keys; no rating last.
    Rating,
}

/// The keys in the order of the command bar's drop-down and View ▸ Sort By.
pub const SORT_KEYS: [SortKey; 5] = [
    SortKey::Name,
    SortKey::Capture,
    SortKey::Modified,
    SortKey::Type,
    SortKey::Rating,
];

/// The sort order of every tab (U14-Q2, U14-Q3): a key and a direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SortOrder {
    pub key: SortKey,
    pub descending: bool,
}

/// A capture time as Exif stores it, `YYYY:MM:DD HH:MM:SS`, checked so that two of them compare
/// in time order as text.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaptureTime(String);

impl CaptureTime {
    /// The capture time of a stored DateTimeOriginal; `None` for a value that is not
    /// `YYYY:MM:DD HH:MM:SS` (anything after the seconds is ignored) or is all zeros, which some
    /// cameras write when the clock was not set.
    pub fn parse(stored: &str) -> Option<Self> {
        let stored = stored.trim();
        let head = stored.get(..19)?;
        let well_formed = head.char_indices().all(|(at, c)| match at {
            4 | 7 => c == ':',
            10 => c == ' ',
            13 | 16 => c == ':',
            _ => c.is_ascii_digit(),
        });
        let zero = head.chars().all(|c| matches!(c, '0' | ':' | ' '));
        (well_formed && !zero).then(|| Self(head.to_owned()))
    }
}

/// The capture times a tab has read (U14-Q4), by the file path as the tab lists it: `None` for a
/// file whose date could not be read or that has none. Kept while the tab is open; emptied when
/// its folder is opened again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureTimes {
    times: HashMap<PathBuf, Option<CaptureTime>>,
}

impl CaptureTimes {
    /// Records the read of `path`.
    pub fn insert(&mut self, path: PathBuf, time: Option<CaptureTime>) {
        self.times.insert(path, time);
    }

    /// Whether the read of `path` has come back.
    pub fn is_read(&self, path: &Path) -> bool {
        self.times.contains_key(path)
    }

    /// The capture time of `path`; `None` when it has none or is not read.
    pub fn get(&self, path: &Path) -> Option<&CaptureTime> {
        self.times.get(path).and_then(Option::as_ref)
    }

    /// How many of `files` have been read.
    pub fn read_count(&self, files: &[PathBuf]) -> usize {
        files.iter().filter(|path| self.is_read(path)).count()
    }

    pub fn clear(&mut self) {
        self.times.clear();
    }
}

/// What a tab's files can be ordered by now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortPlan {
    /// The new order: entry `i` is the file index that comes `i`th.
    Order(Vec<usize>),
    /// Sorting by capture time: `read` of the `total` capture times have come back (U14-Q4).
    ReadingCaptureTimes { read: usize, total: usize },
    /// Sorting by rating: the tab's ratings are not read yet.
    WaitingForRatings,
}

/// The order of `files` under `order`, or what it waits for (U14-Q4): every capture time read
/// (`capture`), or the ratings ready (`ratings_ready`, as the filter's, ADR 0014 Q7).
/// `rating_of` gives the shown rating and `modified_of` the modification time of a file; each is
/// asked only for its own key.
pub fn plan(
    files: &[PathBuf],
    order: SortOrder,
    capture: &CaptureTimes,
    ratings_ready: bool,
    rating_of: &dyn Fn(&Path) -> Option<Rating>,
    modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
) -> SortPlan {
    match order.key {
        SortKey::Capture => {
            let read = capture.read_count(files);
            if read < files.len() {
                return SortPlan::ReadingCaptureTimes {
                    read,
                    total: files.len(),
                };
            }
        }
        SortKey::Rating if !ratings_ready => return SortPlan::WaitingForRatings,
        SortKey::Name | SortKey::Modified | SortKey::Type | SortKey::Rating => {}
    }
    let keyed: Vec<Keyed> = files
        .iter()
        .map(|path| Keyed::new(path.clone(), order, capture, rating_of, modified_of))
        .collect();
    SortPlan::Order(sorted_order(&keyed, order.descending))
}

/// Where `new` goes in `files` under `order` (ADR 0026 §2 point 4, F-Q6): the index before the
/// first file that `order` puts after `new`; `files.len()` when there is none. The comparison is
/// the one `plan` uses (the key's value in the order's direction, no value last in both
/// directions, then the file name ignoring case, then the path as it is). `files` need not be in
/// the order (a rating or a modification time may have changed since it was applied): they are
/// scanned from the start. `rating_of` and `modified_of` are asked only for their own key.
pub fn insert_position(
    files: &[PathBuf],
    new: &Path,
    order: SortOrder,
    capture: &CaptureTimes,
    rating_of: &dyn Fn(&Path) -> Option<Rating>,
    modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
) -> usize {
    let placed: Vec<Keyed> = files
        .iter()
        .map(|path| Keyed::new(path.clone(), order, capture, rating_of, modified_of))
        .collect();
    let new = Keyed::new(new.to_path_buf(), order, capture, rating_of, modified_of);
    place(&placed, &new, order.descending)
}

/// Whether `insert_position` can place `path` under `order` now: `Name`, `Type` and `Modified`
/// always; `Capture` when its capture time is read; `Rating` when the ratings are ready.
pub fn key_known(
    path: &Path,
    order: SortOrder,
    capture: &CaptureTimes,
    ratings_ready: bool,
) -> bool {
    match order.key {
        SortKey::Name | SortKey::Type | SortKey::Modified => true,
        SortKey::Capture => capture.is_read(path),
        SortKey::Rating => ratings_ready,
    }
}

/// The value a key compares first; `None` sorts last in both directions (U14-Q4).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Primary {
    Text(String),
    Capture(CaptureTime),
    Time(SystemTime),
    Rank((i8, usize)),
}

/// A file with the values the comparison needs, taken once (with `Modified` a value is a disk
/// read): `plan` and the placement of new files (`insert_position`, `Tab::relist`,
/// `Tab::place_waiting`) compare with these.
#[derive(Clone, Debug)]
pub(crate) struct Keyed {
    path: PathBuf,
    name: String,
    primary: Option<Primary>,
}

impl Keyed {
    /// `path` with its value under `order.key`; `rating_of` and `modified_of` are asked only for
    /// their own key.
    pub(crate) fn new(
        path: PathBuf,
        order: SortOrder,
        capture: &CaptureTimes,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> Self {
        let primary = match order.key {
            SortKey::Name => Some(Primary::Text(lower_name(&path))),
            SortKey::Type => Some(Primary::Text(lower_extension(&path))),
            SortKey::Capture => capture.get(&path).cloned().map(Primary::Capture),
            SortKey::Modified => modified_of(&path).map(Primary::Time),
            SortKey::Rating => rating_rank(rating_of(&path).as_ref()).map(Primary::Rank),
        };
        let name = lower_name(&path);
        Self {
            path,
            name,
            primary,
        }
    }

    pub(crate) fn into_path(self) -> PathBuf {
        self.path
    }
}

/// The order of two files: the key's value (its direction from `descending`, no value last in
/// both directions), then the file name ignoring case, then the path as it is.
fn compare(a: &Keyed, b: &Keyed, descending: bool) -> Ordering {
    let first = match (a.primary.as_ref(), b.primary.as_ref()) {
        (Some(x), Some(y)) if descending => y.cmp(x),
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    first
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.path.cmp(&b.path))
}

/// The index in `placed` before the first file that the order puts after `new`; `placed.len()`
/// when there is none. Scanned from the start, so `placed` need not be in the order.
pub(crate) fn place(placed: &[Keyed], new: &Keyed, descending: bool) -> usize {
    placed
        .iter()
        .position(|file| compare(file, new, descending) == Ordering::Greater)
        .unwrap_or(placed.len())
}

/// The file indexes ordered by `compare`, ties then by the old position, so the order is the
/// same every time.
fn sorted_order(keyed: &[Keyed], descending: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..keyed.len()).collect();
    order.sort_by(|&a, &b| {
        let by_value = match (keyed.get(a), keyed.get(b)) {
            (Some(x), Some(y)) => compare(x, y, descending),
            _ => Ordering::Equal,
        };
        by_value.then_with(|| a.cmp(&b))
    });
    order
}

fn lower_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn lower_extension(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// The rank of a rating for Rating (U14-Q1): the stars (rejected −1 lowest, no stars as 0), then
/// the label in the order of the label keys (`LABELS`), another label text after them and no
/// label after that. `None` for no rating at all (no stars and no label).
fn rating_rank(rating: Option<&Rating>) -> Option<(i8, usize)> {
    let rating = rating?;
    if rating.stars.is_none() && rating.label.is_none() {
        return None;
    }
    let label = match rating.label.as_deref() {
        Some(label) => LABELS
            .iter()
            .position(|known| *known == label)
            .unwrap_or(LABELS.len()),
        None => LABELS.len() + 1,
    };
    // A label ranks as a value does: the first label key (Red) highest, no label lowest. So,
    // as with the stars, descending (the best first) gives the label keys' order within the same
    // stars, and ascending reverses it.
    let label_rank = LABELS.len() + 1 - label;
    Some((rating.stars.unwrap_or(0), label_rank))
}

/// The modification time of a file; `None` when it cannot be read.
pub fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// The order a tab's capture times are asked for (U14-Q4): the shown images first, from the
/// current one on, then the shown ones before it, then the hidden files. File indexes; those
/// `skip` says are read or asked for already are left out.
pub fn read_order(
    count: usize,
    visible: &[usize],
    current: usize,
    skip: impl Fn(usize) -> bool,
) -> Vec<usize> {
    let from = visible
        .iter()
        .position(|&index| index == current)
        .unwrap_or(0);
    let mut seen = vec![false; count];
    let mut order = Vec::with_capacity(count);
    let shown = visible
        .iter()
        .skip(from)
        .chain(visible.iter().take(from))
        .copied();
    for index in shown.chain(0..count) {
        if let Some(done) = seen.get_mut(index)
            && !*done
        {
            *done = true;
            if !skip(index) {
                order.push(index);
            }
        }
    }
    order
}

/// The answer of one capture-time read.
#[derive(Debug)]
pub struct CaptureRead {
    pub exif: Option<crate::filter_values::ExifSummary>,
    pub path: PathBuf,
    pub time: Option<CaptureTime>,
}

/// The capture-time reader: one thread it owns, which reads one file at a time with
/// `pw_raw::inspect` (as the inspector's Info, U2-Q1). The thread ends when the reader is
/// dropped. When it cannot be started, `request` fails and the caller counts the file as having
/// no date, so the sort still completes.
#[derive(Debug)]
pub struct CaptureReader {
    requests: Sender<PathBuf>,
    results: Receiver<CaptureRead>,
}

impl CaptureReader {
    pub fn start() -> Self {
        let (requests, request_receiver) = mpsc::channel::<PathBuf>();
        let (result_sender, results) = mpsc::channel();
        let started = std::thread::Builder::new()
            .name("pw-capture-reader".into())
            .spawn(move || {
                for path in request_receiver {
                    let info = pw_raw::inspect(&path).ok();
                    let exif = info.as_ref().map(crate::filter_values::exif_summary);
                    let time = info
                        .as_ref()
                        .and_then(|i| i.date_time_original.as_deref())
                        .and_then(CaptureTime::parse);
                    if result_sender
                        .send(CaptureRead { path, time, exif })
                        .is_err()
                    {
                        return;
                    }
                }
            });
        if let Err(error) = started {
            eprintln!("pw-loupe: the capture-time reader could not be started: {error}");
        }
        Self { requests, results }
    }

    /// Asks for the capture time of `path`; false when the reader has stopped.
    pub fn request(&self, path: PathBuf) -> bool {
        self.requests.send(path).is_ok()
    }

    /// A read that has come back, if any.
    pub fn try_recv(&self) -> Option<CaptureRead> {
        self.results.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    fn names(files: &[PathBuf], order: &[usize]) -> Vec<String> {
        order
            .iter()
            .map(|&index| files[index].display().to_string())
            .collect()
    }

    fn no_rating(_: &Path) -> Option<Rating> {
        None
    }

    fn no_time(_: &Path) -> Option<SystemTime> {
        None
    }

    /// The names in the order `plan` gives, with no capture times and no ratings or times unless
    /// the arguments give them.
    fn ordered(
        files: &[PathBuf],
        key: SortKey,
        descending: bool,
        capture: &CaptureTimes,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> Vec<String> {
        match plan(
            files,
            SortOrder { key, descending },
            capture,
            true,
            rating_of,
            modified_of,
        ) {
            SortPlan::Order(order) => names(files, &order),
            other => panic!("not ordered: {other:?}"),
        }
    }

    #[test]
    fn file_name_ascending_and_descending_ignores_case() {
        let files = paths(&["b.ARW", "a.jpg", "C.dng", "A.arw"]);
        let capture = CaptureTimes::default();
        assert_eq!(
            ordered(&files, SortKey::Name, false, &capture, &no_rating, &no_time),
            ["A.arw", "a.jpg", "b.ARW", "C.dng"]
        );
        assert_eq!(
            ordered(&files, SortKey::Name, true, &capture, &no_rating, &no_time),
            ["C.dng", "b.ARW", "a.jpg", "A.arw"]
        );
    }

    #[test]
    fn file_type_sorts_by_extension_then_name_both_ways() {
        let files = paths(&["b.jpg", "a.jpg", "c.ARW", "a.arw", "d.dng"]);
        let capture = CaptureTimes::default();
        assert_eq!(
            ordered(&files, SortKey::Type, false, &capture, &no_rating, &no_time),
            ["a.arw", "c.ARW", "d.dng", "a.jpg", "b.jpg"]
        );
        // Descending reverses the extensions; ties stay by name.
        assert_eq!(
            ordered(&files, SortKey::Type, true, &capture, &no_rating, &no_time),
            ["a.jpg", "b.jpg", "d.dng", "a.arw", "c.ARW"]
        );
    }

    fn capture_of(pairs: &[(&str, Option<&str>)]) -> CaptureTimes {
        let mut capture = CaptureTimes::default();
        for (name, time) in pairs {
            capture.insert(PathBuf::from(name), time.and_then(CaptureTime::parse));
        }
        capture
    }

    #[test]
    fn capture_time_both_ways_with_ties_by_name_and_no_date_last() {
        // Two cameras: the names do not follow the capture order; a RAW and its JPEG share a time.
        let files = paths(&[
            "B_002.JPG",
            "A_100.ARW",
            "B_002.ARW",
            "NODATE.TIF",
            "A_101.ARW",
            "BROKEN.DNG",
        ]);
        let capture = capture_of(&[
            ("B_002.JPG", Some("2026:10:30 14:03:13")),
            ("A_100.ARW", Some("2026:10:30 14:03:12")),
            ("B_002.ARW", Some("2026:10:30 14:03:13")),
            ("NODATE.TIF", None),
            ("A_101.ARW", Some("2026:10:30 14:03:14")),
            ("BROKEN.DNG", Some("    :  :     :  :  ")),
        ]);
        assert_eq!(
            ordered(
                &files,
                SortKey::Capture,
                false,
                &capture,
                &no_rating,
                &no_time
            ),
            [
                "A_100.ARW",
                "B_002.ARW",
                "B_002.JPG",
                "A_101.ARW",
                "BROKEN.DNG",
                "NODATE.TIF"
            ]
        );
        assert_eq!(
            ordered(
                &files,
                SortKey::Capture,
                true,
                &capture,
                &no_rating,
                &no_time
            ),
            [
                "A_101.ARW",
                "B_002.ARW",
                "B_002.JPG",
                "A_100.ARW",
                "BROKEN.DNG",
                "NODATE.TIF"
            ]
        );
    }

    #[test]
    fn capture_times_are_checked_before_they_are_compared() {
        assert_eq!(
            CaptureTime::parse("2026:10:30 14:03:12"),
            Some(CaptureTime("2026:10:30 14:03:12".to_owned()))
        );
        // Sub-seconds or a zone after the seconds are ignored.
        assert_eq!(
            CaptureTime::parse("2026:10:30 14:03:12.45+01:00"),
            CaptureTime::parse("2026:10:30 14:03:12")
        );
        assert_eq!(CaptureTime::parse("0000:00:00 00:00:00"), None);
        assert_eq!(CaptureTime::parse("2026-10-30 14:03:12"), None);
        assert_eq!(CaptureTime::parse("2026:10:30"), None);
        assert_eq!(CaptureTime::parse(""), None);
    }

    #[test]
    fn modified_time_both_ways_and_an_unreadable_time_last() {
        let files = paths(&["c.arw", "a.arw", "b.arw", "gone.arw"]);
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let modified = |path: &Path| -> Option<SystemTime> {
            let seconds = match path.to_str()? {
                "c.arw" => 1,
                "a.arw" => 3,
                "b.arw" => 1,
                _ => return None,
            };
            Some(base + Duration::from_secs(seconds))
        };
        let capture = CaptureTimes::default();
        assert_eq!(
            ordered(
                &files,
                SortKey::Modified,
                false,
                &capture,
                &no_rating,
                &modified
            ),
            ["b.arw", "c.arw", "a.arw", "gone.arw"]
        );
        assert_eq!(
            ordered(
                &files,
                SortKey::Modified,
                true,
                &capture,
                &no_rating,
                &modified
            ),
            ["a.arw", "b.arw", "c.arw", "gone.arw"]
        );
    }

    fn rating(stars: Option<i8>, label: Option<&str>) -> Rating {
        Rating {
            stars,
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn rating_sorts_by_stars_then_label_keys_with_no_rating_last() {
        let files = paths(&[
            "none.arw",
            "five_red.arw",
            "three.arw",
            "rejected.arw",
            "five.arw",
            "red_only.arw",
            "five_purple.arw",
            "empty.arw",
            "five_yellow.arw",
            "three_orange.arw",
            "three_green.arw",
        ]);
        let ratings: HashMap<&str, Rating> = HashMap::from([
            ("five_red.arw", rating(Some(5), Some("Red"))),
            ("three.arw", rating(Some(3), None)),
            ("rejected.arw", rating(Some(-1), None)),
            ("five.arw", rating(Some(5), None)),
            ("red_only.arw", rating(None, Some("Red"))),
            ("five_purple.arw", rating(Some(5), Some("Purple"))),
            ("empty.arw", rating(None, None)),
            ("five_yellow.arw", rating(Some(5), Some("Yellow"))),
            ("three_orange.arw", rating(Some(3), Some("Orange"))),
            ("three_green.arw", rating(Some(3), Some("Green"))),
        ]);
        let rating_of = |path: &Path| path.to_str().and_then(|name| ratings.get(name).cloned());
        let capture = CaptureTimes::default();
        // Descending: the best-rated first; within the stars the label keys' order reversed.
        assert_eq!(
            ordered(
                &files,
                SortKey::Rating,
                true,
                &capture,
                &rating_of,
                &no_time
            ),
            [
                "five_red.arw",
                "five_yellow.arw",
                "five_purple.arw",
                "five.arw",
                "three_green.arw",
                "three_orange.arw",
                "three.arw",
                "red_only.arw",
                "rejected.arw",
                "empty.arw",
                "none.arw",
            ]
        );
        assert_eq!(
            ordered(
                &files,
                SortKey::Rating,
                false,
                &capture,
                &rating_of,
                &no_time
            ),
            [
                "rejected.arw",
                "red_only.arw",
                "three.arw",
                "three_orange.arw",
                "three_green.arw",
                "five.arw",
                "five_purple.arw",
                "five_yellow.arw",
                "five_red.arw",
                "empty.arw",
                "none.arw",
            ]
        );
    }

    #[test]
    fn a_tab_waits_for_every_capture_time_and_for_its_ratings() {
        let files = paths(&["a.arw", "b.arw", "c.arw"]);
        let mut capture = capture_of(&[("a.arw", Some("2026:10:30 14:03:12")), ("c.arw", None)]);
        let capture_order = SortOrder {
            key: SortKey::Capture,
            descending: false,
        };
        assert_eq!(
            plan(&files, capture_order, &capture, true, &no_rating, &no_time),
            SortPlan::ReadingCaptureTimes { read: 2, total: 3 }
        );
        capture.insert(
            PathBuf::from("b.arw"),
            CaptureTime::parse("2026:10:30 14:03:11"),
        );
        assert_eq!(
            plan(&files, capture_order, &capture, true, &no_rating, &no_time),
            SortPlan::Order(vec![1, 0, 2])
        );

        let rating_order = SortOrder {
            key: SortKey::Rating,
            descending: true,
        };
        assert_eq!(
            plan(&files, rating_order, &capture, false, &no_rating, &no_time),
            SortPlan::WaitingForRatings
        );
        // The other keys need nothing first.
        for key in [SortKey::Name, SortKey::Modified, SortKey::Type] {
            let order = SortOrder {
                key,
                descending: false,
            };
            let empty = CaptureTimes::default();
            assert!(matches!(
                plan(&files, order, &empty, false, &no_rating, &no_time),
                SortPlan::Order(_)
            ));
        }
    }

    #[test]
    fn capture_times_count_only_the_files_asked_about() {
        let mut capture = CaptureTimes::default();
        capture.insert(PathBuf::from("a.arw"), None);
        capture.insert(
            PathBuf::from("other.arw"),
            CaptureTime::parse("2026:01:01 00:00:00"),
        );
        assert_eq!(capture.read_count(&paths(&["a.arw", "b.arw"])), 1);
        assert!(capture.is_read(Path::new("a.arw")));
        assert_eq!(capture.get(Path::new("a.arw")), None);
        assert!(capture.get(Path::new("other.arw")).is_some());
        capture.clear();
        assert_eq!(capture.read_count(&paths(&["a.arw"])), 0);
    }

    #[test]
    fn read_order_takes_the_shown_images_from_the_current_one_first() {
        // Files 0..8; the filter shows 1, 3, 4, 6; the current image is 4; 6 is read already.
        assert_eq!(
            read_order(8, &[1, 3, 4, 6], 4, |index| index == 6),
            [4, 1, 3, 0, 2, 5, 7]
        );
        // The current image not shown: from the first shown one.
        assert_eq!(read_order(4, &[2, 3], 0, |_| false), [2, 3, 0, 1]);
        // Indexes outside the folder are ignored.
        assert_eq!(read_order(2, &[5, 1], 1, |_| false), [1, 0]);
    }

    // --- placing a new file (ADR 0026 §2 point 4, F-Q6, T240) ---

    fn by(key: SortKey, descending: bool) -> SortOrder {
        SortOrder { key, descending }
    }

    /// `insert_position` with no capture times, ratings or modification times.
    fn position_by_name(files: &[&str], new: &str, descending: bool) -> usize {
        insert_position(
            &paths(files),
            Path::new(new),
            by(SortKey::Name, descending),
            &CaptureTimes::default(),
            &no_rating,
            &no_time,
        )
    }

    #[test]
    fn insert_position_by_name_both_ways_and_into_an_empty_list() {
        let files = ["a.arw", "C.arw", "e.arw"];
        assert_eq!(position_by_name(&files, "B.arw", false), 1, "between");
        assert_eq!(position_by_name(&files, "0.arw", false), 0, "first");
        assert_eq!(position_by_name(&files, "f.arw", false), 3, "last");
        assert_eq!(
            position_by_name(&["e.arw", "C.arw", "a.arw"], "b.arw", true),
            2,
            "descending"
        );
        assert_eq!(position_by_name(&[], "a.arw", false), 0, "an empty list");
    }

    #[test]
    fn insert_position_by_capture_puts_a_file_without_a_time_last_in_both_directions() {
        let capture = capture_of(&[
            ("t1.arw", Some("2026:10:30 14:00:01")),
            ("t2.arw", Some("2026:10:30 14:00:02")),
            ("t3.arw", Some("2026:10:30 14:00:03")),
            ("m_none.arw", None),
            ("z_none.arw", None),
            ("a_none.arw", None),
        ]);
        let at = |files: &[&str], new: &str, descending: bool| {
            insert_position(
                &paths(files),
                Path::new(new),
                by(SortKey::Capture, descending),
                &capture,
                &no_rating,
                &no_time,
            )
        };
        let ascending = ["t1.arw", "t3.arw", "m_none.arw"];
        assert_eq!(at(&ascending, "t2.arw", false), 1);
        assert_eq!(at(&ascending, "a_none.arw", false), 2, "no time: last");
        assert_eq!(at(&ascending, "z_none.arw", false), 3);
        let descending = ["t3.arw", "t1.arw", "m_none.arw"];
        assert_eq!(at(&descending, "t2.arw", true), 1);
        assert_eq!(at(&descending, "a_none.arw", true), 2, "no time: last");
        assert_eq!(at(&descending, "z_none.arw", true), 3);
    }

    #[test]
    fn insert_position_scans_a_list_that_is_not_in_the_order_from_the_start() {
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let modified = |path: &Path| -> Option<SystemTime> {
            let seconds = match path.to_str()? {
                "a.arw" => 1,
                "b.arw" => 2,
                "c.arw" => 30,
                "new.arw" => 20,
                _ => return None,
            };
            Some(base + Duration::from_secs(seconds))
        };
        // c.arw was modified after the order was applied: it comes first and is after new.arw.
        assert_eq!(
            insert_position(
                &paths(&["c.arw", "a.arw", "b.arw"]),
                Path::new("new.arw"),
                by(SortKey::Modified, false),
                &CaptureTimes::default(),
                &no_rating,
                &modified,
            ),
            0
        );
        // A list in the order: after b.arw.
        assert_eq!(
            insert_position(
                &paths(&["a.arw", "b.arw", "c.arw"]),
                Path::new("new.arw"),
                by(SortKey::Modified, false),
                &CaptureTimes::default(),
                &no_rating,
                &modified,
            ),
            2
        );
    }

    #[test]
    fn insert_position_by_rating() {
        let ratings: HashMap<&str, Rating> = HashMap::from([
            ("five.arw", rating(Some(5), None)),
            ("three.arw", rating(Some(3), None)),
            ("four.arw", rating(Some(4), None)),
        ]);
        let rating_of = |path: &Path| path.to_str().and_then(|name| ratings.get(name).cloned());
        let files = paths(&["five.arw", "three.arw", "none.arw"]);
        let at = |new: &str| {
            insert_position(
                &files,
                Path::new(new),
                by(SortKey::Rating, true),
                &CaptureTimes::default(),
                &rating_of,
                &no_time,
            )
        };
        assert_eq!(at("four.arw"), 1);
        assert_eq!(
            at("other.arw"),
            3,
            "no rating: last, after none.arw by name"
        );
    }

    #[test]
    fn a_new_file_whose_value_ties_is_placed_by_name_then_by_path() {
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let same = |_: &Path| Some(base);
        let at = |files: &[&str], new: &str| {
            insert_position(
                &paths(files),
                Path::new(new),
                by(SortKey::Modified, false),
                &CaptureTimes::default(),
                &no_rating,
                &same,
            )
        };
        assert_eq!(at(&["a.arw", "c.arw"], "b.arw"), 1, "by name");
        assert_eq!(at(&["a.arw", "c.arw"], "C.arw"), 1, "C.arw before c.arw");
        assert_eq!(at(&["x/b.arw", "z/b.arw"], "y/b.arw"), 1, "then by path");
    }

    /// For each key and direction a list is put in `plan`'s order; each file then placed into
    /// the list without it goes back to its own place, so `plan` and `insert_position` compare
    /// alike.
    #[test]
    fn insert_position_puts_each_file_where_plan_puts_it() {
        let files = paths(&[
            "d.jpg", "B.arw", "a.dng", "c.ARW", "e.tif", "none.jpg", "x/f.arw", "y/f.arw",
        ]);
        let capture = capture_of(&[
            ("d.jpg", Some("2026:10:30 14:00:03")),
            ("B.arw", Some("2026:10:30 14:00:01")),
            ("a.dng", Some("2026:10:30 14:00:03")),
            ("c.ARW", None),
            ("e.tif", Some("2026:10:30 14:00:02")),
            ("none.jpg", None),
            ("x/f.arw", Some("2026:10:30 14:00:01")),
            ("y/f.arw", Some("2026:10:30 14:00:01")),
        ]);
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let modified = |path: &Path| -> Option<SystemTime> {
            let seconds = match path.to_str()? {
                "d.jpg" => 2,
                "B.arw" => 5,
                "a.dng" => 2,
                "c.ARW" => 1,
                "x/f.arw" => 4,
                "y/f.arw" => 4,
                _ => return None,
            };
            Some(base + Duration::from_secs(seconds))
        };
        let ratings: HashMap<&str, Rating> = HashMap::from([
            ("d.jpg", rating(Some(3), Some("Red"))),
            ("B.arw", rating(Some(5), None)),
            ("a.dng", rating(Some(3), None)),
            ("c.ARW", rating(Some(-1), None)),
            ("x/f.arw", rating(Some(3), None)),
            ("y/f.arw", rating(None, Some("Green"))),
        ]);
        let rating_of = |path: &Path| path.to_str().and_then(|name| ratings.get(name).cloned());
        for key in SORT_KEYS {
            for descending in [false, true] {
                let order = by(key, descending);
                let SortPlan::Order(planned) =
                    plan(&files, order, &capture, true, &rating_of, &modified)
                else {
                    panic!("not ordered: {order:?}");
                };
                let sorted: Vec<PathBuf> = planned.iter().map(|&i| files[i].clone()).collect();
                for (i, file) in sorted.iter().enumerate() {
                    let mut without = sorted.clone();
                    without.remove(i);
                    assert_eq!(
                        insert_position(&without, file, order, &capture, &rating_of, &modified),
                        i,
                        "{order:?}: {}",
                        file.display()
                    );
                }
            }
        }
    }

    #[test]
    fn key_known_for_each_key() {
        let capture = capture_of(&[("read.arw", None)]);
        let read = Path::new("read.arw");
        let unread = Path::new("unread.arw");
        for key in [SortKey::Name, SortKey::Type, SortKey::Modified] {
            assert!(
                key_known(unread, by(key, false), &capture, false),
                "{key:?}"
            );
        }
        assert!(key_known(
            read,
            by(SortKey::Capture, false),
            &capture,
            false
        ));
        assert!(!key_known(
            unread,
            by(SortKey::Capture, true),
            &capture,
            true
        ));
        assert!(key_known(
            unread,
            by(SortKey::Rating, false),
            &capture,
            true
        ));
        assert!(!key_known(
            read,
            by(SortKey::Rating, false),
            &capture,
            false
        ));
    }

    #[test]
    fn the_reader_reads_a_file_without_a_date_as_none() {
        let reader = CaptureReader::start();
        let missing =
            std::env::temp_dir().join(format!("pw-loupe-sort-missing-{}.arw", std::process::id()));
        assert!(reader.request(missing.clone()));
        let started = std::time::Instant::now();
        let read = loop {
            if let Some(read) = reader.try_recv() {
                break read;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "no answer");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(read.path, missing);
        assert_eq!(read.time, None);
    }
}
