//! The Tasks area in the left column (ADR 0017 §2.1, step U5, T140) as plain Rust: the import's
//! row, the pending writes and the error list. `main.rs` feeds it the import worker's events and
//! the failures `metadata.poll` returns, and draws it; the texts are made in `tasks_text.rs`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{ImportDone, ImportEvent, RequestId};

/// At most this many errors are kept; the oldest is dropped first (U5-Q5).
pub const MAX_ERRORS: usize = 200;

/// A time of day as the error list shows it (`HH:MM`, local time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockTime {
    pub hour: u8,
    pub minute: u8,
}

impl ClockTime {
    /// The time of day of `unix_seconds` (seconds since the Unix epoch, UTC) moved by
    /// `offset_seconds` (the local offset from UTC).
    pub fn from_unix(unix_seconds: i64, offset_seconds: i64) -> ClockTime {
        let of_day = unix_seconds
            .saturating_add(offset_seconds)
            .rem_euclid(86_400);
        ClockTime {
            hour: (of_day / 3600) as u8,
            minute: (of_day / 60 % 60) as u8,
        }
    }

    /// The local time now. Outside Windows, or when the local offset cannot be read, UTC.
    pub fn now() -> ClockTime {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(0));
        ClockTime::from_unix(seconds, local_offset_seconds(seconds))
    }
}

/// The local offset from UTC at `unix_seconds`, from Windows' own conversion of a file time to
/// local file time (the time zone's current bias); 0 when it fails.
#[cfg(windows)]
fn local_offset_seconds(unix_seconds: i64) -> i64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Storage::FileSystem::FileTimeToLocalFileTime;

    /// Seconds from 1601-01-01 (the file-time epoch) to 1970-01-01.
    const EPOCH_DIFFERENCE: i64 = 11_644_473_600;
    /// File-time units (100 ns) per second.
    const UNITS: i64 = 10_000_000;

    let Some(units) = unix_seconds
        .checked_add(EPOCH_DIFFERENCE)
        .and_then(|seconds| seconds.checked_mul(UNITS))
        .and_then(|units| u64::try_from(units).ok())
    else {
        return 0;
    };
    let utc = FILETIME {
        dwLowDateTime: units as u32,
        dwHighDateTime: (units >> 32) as u32,
    };
    let mut local = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    // SAFETY: both pointers point to `FILETIME` values that live through the call.
    let converted = unsafe { FileTimeToLocalFileTime(&utc, &mut local) };
    if converted == 0 {
        return 0;
    }
    let local_units = (u64::from(local.dwHighDateTime) << 32) | u64::from(local.dwLowDateTime);
    let difference = i128::from(local_units) - i128::from(units);
    i64::try_from(difference / i128::from(UNITS)).unwrap_or(0)
}

#[cfg(not(windows))]
fn local_offset_seconds(_unix_seconds: i64) -> i64 {
    0
}

/// What the import row shows (U5-Q2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportStep {
    /// Copying: the files and bytes done of all, and the last file copied (`None` before the
    /// first).
    Copying {
        files_done: usize,
        files_total: usize,
        bytes_done: u64,
        bytes_total: u64,
        current: Option<PathBuf>,
    },
    /// Writing the IPTC template: `done` files written of `total`, `current` being written.
    Iptc {
        done: usize,
        total: usize,
        current: PathBuf,
    },
    /// Ejecting the source card.
    Ejecting,
    /// Finished or cancelled: the files copied and failed, or the error that stopped it.
    Done {
        copied: usize,
        failed: usize,
        cancelled: bool,
        error: Option<String>,
    },
}

/// The import's row: where it copies from and to, and its step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportRow {
    pub source: String,
    pub destination: String,
    pub step: ImportStep,
}

impl ImportRow {
    /// Whether the import still runs (copying, writing IPTC or ejecting): the row has "Stop".
    pub fn running(&self) -> bool {
        !matches!(self.step, ImportStep::Done { .. })
    }

    /// The progress bar's fraction, by bytes: 0 to 1 while copying, full after it.
    pub fn fraction(&self) -> f32 {
        match &self.step {
            ImportStep::Copying {
                bytes_done,
                bytes_total,
                ..
            } => {
                if *bytes_total == 0 {
                    0.0
                } else {
                    (*bytes_done as f64 / *bytes_total as f64).clamp(0.0, 1.0) as f32
                }
            }
            _ => 1.0,
        }
    }
}

/// A pending write, one line each while it runs (U5-Q4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingWrite {
    /// Rating writes queued or sent but not yet confirmed (`metadata.pending_rating_writes()`).
    Ratings(usize),
    /// Rotation writes queued or sent but not yet confirmed (`metadata.pending_rotation_writes()`,
    /// T239).
    Rotations(usize),
    /// "Apply to All Marked": image `current` of `total` (`metadata.batch_progress()`).
    Batch { current: usize, total: usize },
    /// The metadata window's own write.
    Editor,
}

/// The pending writes to show, in this order: ratings, rotations (T239), "Apply to All Marked",
/// the window's write. A write that does not run (a count of 0) gives no line. The window's own
/// write is not shown beside a batch, whose writes it is part of.
pub fn pending_writes(
    ratings: usize,
    rotations: usize,
    batch: Option<(usize, usize)>,
    editor_saving: bool,
) -> Vec<PendingWrite> {
    let mut lines = Vec::new();
    if ratings > 0 {
        lines.push(PendingWrite::Ratings(ratings));
    }
    if rotations > 0 {
        lines.push(PendingWrite::Rotations(rotations));
    }
    match batch {
        Some((current, total)) => lines.push(PendingWrite::Batch { current, total }),
        None if editor_saving => lines.push(PendingWrite::Editor),
        None => {}
    }
    lines
}

/// One error in the list: when it came and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskError {
    pub time: ClockTime,
    pub text: String,
}

/// The Tasks area's state: the import row and the error list. Not saved when the program ends.
#[derive(Clone, Debug, Default)]
pub struct Tasks {
    import: Option<ImportRow>,
    /// The running import's request; `None` once it is done, so its events count once.
    import_id: Option<RequestId>,
    /// Newest first.
    errors: VecDeque<TaskError>,
}

impl Tasks {
    pub fn new() -> Tasks {
        Tasks::default()
    }

    /// The import's row, if an import runs or a finished one has not been removed.
    pub fn import(&self) -> Option<&ImportRow> {
        self.import.as_ref()
    }

    /// An import was sent to the worker as request `id`: its row replaces any earlier one.
    pub fn import_started(&mut self, id: RequestId, source: &str, destination: &str) {
        self.import_id = Some(id);
        self.import = Some(ImportRow {
            source: source.trim().to_owned(),
            destination: destination.trim().to_owned(),
            step: ImportStep::Copying {
                files_done: 0,
                files_total: 0,
                bytes_done: 0,
                bytes_total: 0,
                current: None,
            },
        });
    }

    /// Applies an event of the import worker; events of other requests (previews, card lists,
    /// an earlier import) change nothing. A finished import's failed files, IPTC errors and error
    /// go to the error list at `time`, once.
    pub fn apply(&mut self, event: &ImportEvent, time: ClockTime) {
        let Some(id) = self.import_id else {
            return;
        };
        let Some(row) = self.import.as_mut() else {
            return;
        };
        match event {
            ImportEvent::Progress {
                id: event_id,
                progress,
            } if *event_id == id => {
                row.step = ImportStep::Copying {
                    files_done: progress.files_done,
                    files_total: progress.files_total,
                    bytes_done: progress.bytes_done,
                    bytes_total: progress.bytes_total,
                    current: Some(progress.current.clone()),
                };
            }
            ImportEvent::IptcProgress {
                id: event_id,
                done,
                total,
                current,
            } if *event_id == id => {
                row.step = ImportStep::Iptc {
                    done: *done,
                    total: *total,
                    current: current.clone(),
                };
            }
            ImportEvent::Ejecting { id: event_id } if *event_id == id => {
                row.step = ImportStep::Ejecting;
            }
            ImportEvent::Done { id: event_id, done } if *event_id == id => {
                row.step = ImportStep::Done {
                    copied: done.copied,
                    failed: done.failed.len(),
                    cancelled: done.cancelled,
                    error: done.error.clone(),
                };
                self.import_id = None;
                self.add_import_errors(done, time);
            }
            _ => {}
        }
    }

    /// The import worker stopped while the import ran: the row ends with `error`, which also goes
    /// to the error list.
    pub fn import_stopped(&mut self, error: &str, time: ClockTime) {
        if self.import_id.take().is_none() {
            return;
        }
        if let Some(row) = self.import.as_mut() {
            row.step = ImportStep::Done {
                copied: 0,
                failed: 0,
                cancelled: false,
                error: Some(error.to_owned()),
            };
        }
        self.add_error(error, time);
    }

    /// "×" on a finished import's row: the row goes. A running import's row stays.
    pub fn dismiss_import(&mut self) {
        if self.import.as_ref().is_some_and(|row| !row.running()) {
            self.import = None;
        }
    }

    /// Adds an error as the newest; beyond [`MAX_ERRORS`] the oldest is dropped.
    pub fn add_error(&mut self, text: &str, time: ClockTime) {
        self.errors.push_front(TaskError {
            time,
            text: text.to_owned(),
        });
        self.errors.truncate(MAX_ERRORS);
    }

    /// The errors, newest first.
    pub fn errors(&self) -> impl ExactSizeIterator<Item = &TaskError> {
        self.errors.iter()
    }

    /// "Clear": the error list is emptied.
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }

    /// Whether the area has nothing to show with no pending write ("No tasks").
    pub fn is_empty(&self) -> bool {
        self.import.is_none() && self.errors.is_empty()
    }

    fn add_import_errors(&mut self, done: &ImportDone, time: ClockTime) {
        for text in done
            .failed
            .iter()
            .chain(&done.iptc_errors)
            .chain(&done.error)
        {
            self.add_error(text, time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: ClockTime = ClockTime {
        hour: 14,
        minute: 7,
    };

    fn started() -> Tasks {
        let mut tasks = Tasks::new();
        tasks.import_started(7, " E:\\ ", "D:\\Bilder");
        tasks
    }

    fn progress(id: RequestId, done: usize, bytes_done: u64) -> ImportEvent {
        ImportEvent::Progress {
            id,
            progress: pw_ingest::Progress {
                files_done: done,
                files_total: 4,
                bytes_done,
                bytes_total: 400,
                current: PathBuf::from(format!("E:\\DCIM\\IMG_{done}.ARW")),
            },
        }
    }

    fn done(id: RequestId, done: ImportDone) -> ImportEvent {
        ImportEvent::Done { id, done }
    }

    fn step(tasks: &Tasks) -> ImportStep {
        tasks.import().map(|row| row.step.clone()).unwrap()
    }

    fn texts(tasks: &Tasks) -> Vec<String> {
        tasks.errors().map(|error| error.text.clone()).collect()
    }

    #[test]
    fn the_clock_time_is_the_local_time_of_day() {
        // 2026-11-15 13:05:00 UTC.
        let seconds = 1_763_211_900;
        assert_eq!(
            ClockTime::from_unix(seconds, 0),
            ClockTime {
                hour: 13,
                minute: 5
            }
        );
        assert_eq!(
            ClockTime::from_unix(seconds, 3600),
            ClockTime {
                hour: 14,
                minute: 5
            }
        );
        // Past midnight both ways.
        assert_eq!(
            ClockTime::from_unix(seconds, 11 * 3600),
            ClockTime { hour: 0, minute: 5 }
        );
        assert_eq!(
            ClockTime::from_unix(seconds, -14 * 3600),
            ClockTime {
                hour: 23,
                minute: 5
            }
        );
        let now = ClockTime::now();
        assert!(now.hour < 24 && now.minute < 60, "{now:?}");
    }

    #[test]
    fn a_started_import_shows_its_route_and_no_progress_yet() {
        let tasks = started();
        let row = tasks.import().unwrap();
        assert_eq!(row.source, "E:\\");
        assert_eq!(row.destination, "D:\\Bilder");
        assert!(row.running());
        assert_eq!(row.fraction(), 0.0);
        assert_eq!(
            row.step,
            ImportStep::Copying {
                files_done: 0,
                files_total: 0,
                bytes_done: 0,
                bytes_total: 0,
                current: None
            }
        );
    }

    #[test]
    fn progress_moves_the_row_and_its_bar_by_bytes() {
        let mut tasks = started();
        tasks.apply(&progress(7, 1, 100), T);
        assert_eq!(
            step(&tasks),
            ImportStep::Copying {
                files_done: 1,
                files_total: 4,
                bytes_done: 100,
                bytes_total: 400,
                current: Some(PathBuf::from("E:\\DCIM\\IMG_1.ARW"))
            }
        );
        assert_eq!(tasks.import().unwrap().fraction(), 0.25);
        // Another request's progress changes nothing.
        tasks.apply(&progress(8, 3, 300), T);
        assert_eq!(tasks.import().unwrap().fraction(), 0.25);
    }

    #[test]
    fn the_iptc_and_ejecting_steps() {
        let mut tasks = started();
        tasks.apply(
            &ImportEvent::IptcProgress {
                id: 7,
                done: 2,
                total: 4,
                current: PathBuf::from("D:\\Bilder\\IMG_3.ARW"),
            },
            T,
        );
        assert_eq!(
            step(&tasks),
            ImportStep::Iptc {
                done: 2,
                total: 4,
                current: PathBuf::from("D:\\Bilder\\IMG_3.ARW")
            }
        );
        assert!(tasks.import().unwrap().running());
        tasks.apply(&ImportEvent::Ejecting { id: 7 }, T);
        assert_eq!(step(&tasks), ImportStep::Ejecting);
        assert!(tasks.import().unwrap().running());
        assert_eq!(tasks.import().unwrap().fraction(), 1.0);
    }

    #[test]
    fn done_and_cancelled_end_the_row_with_the_failed_files_counted() {
        let mut tasks = started();
        tasks.apply(
            &done(
                7,
                ImportDone {
                    copied: 3,
                    failed: vec!["IMG_2.ARW: disk full".to_owned()],
                    ..ImportDone::default()
                },
            ),
            T,
        );
        assert_eq!(
            step(&tasks),
            ImportStep::Done {
                copied: 3,
                failed: 1,
                cancelled: false,
                error: None
            }
        );
        assert!(!tasks.import().unwrap().running());

        let mut tasks = started();
        tasks.apply(
            &done(
                7,
                ImportDone {
                    copied: 2,
                    cancelled: true,
                    ..ImportDone::default()
                },
            ),
            T,
        );
        assert_eq!(
            step(&tasks),
            ImportStep::Done {
                copied: 2,
                failed: 0,
                cancelled: true,
                error: None
            }
        );
        assert!(tasks.errors().next().is_none());
    }

    #[test]
    fn the_done_row_stays_until_dismissed_or_a_new_import() {
        let mut tasks = started();
        // "×" does nothing while the import runs.
        tasks.dismiss_import();
        assert!(tasks.import().is_some());
        tasks.apply(
            &done(
                7,
                ImportDone {
                    copied: 4,
                    ..ImportDone::default()
                },
            ),
            T,
        );
        // Later events (another request) leave it.
        tasks.apply(&progress(8, 1, 100), T);
        assert!(!tasks.import().unwrap().running());
        tasks.dismiss_import();
        assert!(tasks.import().is_none());
        assert!(tasks.is_empty());

        let mut tasks = started();
        tasks.apply(&done(7, ImportDone::default()), T);
        tasks.import_started(9, "F:\\", "D:\\Andra");
        let row = tasks.import().unwrap();
        assert_eq!(row.source, "F:\\");
        assert!(row.running());
        tasks.apply(&progress(9, 1, 100), T);
        assert_eq!(tasks.import().unwrap().fraction(), 0.25);
    }

    #[test]
    fn an_imports_failures_go_to_the_list_once() {
        let mut tasks = started();
        let event = done(
            7,
            ImportDone {
                copied: 1,
                failed: vec!["a: x".to_owned(), "b: y".to_owned()],
                iptc_errors: vec!["c: z".to_owned()],
                error: Some("stopped".to_owned()),
                ..ImportDone::default()
            },
        );
        tasks.apply(&event, T);
        assert_eq!(texts(&tasks), ["stopped", "c: z", "b: y", "a: x"]);
        assert!(tasks.errors().all(|error| error.time == T));
        // The same event again (or after the row is gone) adds nothing.
        tasks.apply(&event, T);
        tasks.dismiss_import();
        tasks.apply(&event, T);
        assert_eq!(tasks.errors().len(), 4);
    }

    #[test]
    fn a_stopped_worker_ends_the_row_with_its_error_once() {
        let mut tasks = started();
        tasks.import_stopped("The import stopped before it was finished.", T);
        assert_eq!(
            step(&tasks),
            ImportStep::Done {
                copied: 0,
                failed: 0,
                cancelled: false,
                error: Some("The import stopped before it was finished.".to_owned())
            }
        );
        tasks.import_stopped("again", T);
        assert_eq!(
            texts(&tasks),
            ["The import stopped before it was finished."]
        );
        // Nothing runs: nothing to stop.
        let mut idle = Tasks::new();
        idle.import_stopped("x", T);
        assert!(idle.is_empty());
    }

    #[test]
    fn the_pending_write_lines_appear_and_go() {
        assert_eq!(pending_writes(0, 0, None, false), []);
        assert_eq!(
            pending_writes(3, 0, None, false),
            [PendingWrite::Ratings(3)]
        );
        assert_eq!(
            pending_writes(0, 5, None, false),
            [PendingWrite::Rotations(5)]
        );
        assert_eq!(pending_writes(0, 0, None, true), [PendingWrite::Editor]);
        assert_eq!(
            pending_writes(2, 0, Some((4, 9)), true),
            [
                PendingWrite::Ratings(2),
                PendingWrite::Batch {
                    current: 4,
                    total: 9
                }
            ]
        );
        assert_eq!(
            pending_writes(2, 1, Some((4, 9)), true),
            [
                PendingWrite::Ratings(2),
                PendingWrite::Rotations(1),
                PendingWrite::Batch {
                    current: 4,
                    total: 9
                }
            ]
        );
        assert_eq!(
            pending_writes(2, 1, None, true),
            [
                PendingWrite::Ratings(2),
                PendingWrite::Rotations(1),
                PendingWrite::Editor
            ]
        );
        assert_eq!(pending_writes(0, 0, None, false), []);
    }

    #[test]
    fn errors_are_newest_first_with_their_time_and_clear_empties_them() {
        let mut tasks = Tasks::new();
        assert!(tasks.is_empty());
        let early = ClockTime { hour: 9, minute: 0 };
        tasks.add_error("first", early);
        tasks.add_error("second", T);
        assert!(!tasks.is_empty());
        let errors: Vec<&TaskError> = tasks.errors().collect();
        assert_eq!(
            errors,
            [
                &TaskError {
                    time: T,
                    text: "second".to_owned()
                },
                &TaskError {
                    time: early,
                    text: "first".to_owned()
                }
            ]
        );
        tasks.clear_errors();
        assert!(tasks.errors().next().is_none());
        assert!(tasks.is_empty());
    }

    #[test]
    fn at_most_200_errors_are_kept_dropping_the_oldest() {
        let mut tasks = Tasks::new();
        for number in 0..=MAX_ERRORS {
            tasks.add_error(&format!("error {number}"), T);
        }
        assert_eq!(MAX_ERRORS, 200);
        assert_eq!(tasks.errors().len(), MAX_ERRORS);
        let list = texts(&tasks);
        assert_eq!(list.first().map(String::as_str), Some("error 200"));
        assert_eq!(list.last().map(String::as_str), Some("error 1"));
    }
}
