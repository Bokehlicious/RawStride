//! The inspector's Info section (ADR 0017 §2.1, U2, T110): an Exif summary of the current image,
//! read with `pw_raw::inspect` (no ExifTool, U2-Q1) on a thread of its own, with the rules of
//! T103: a read is asked for only while the section is shown and after the image has been the
//! current one for 200 ms; at most one read is in flight; the newest image is asked for when it
//! comes back; a result for another image is dropped. The rows are made by pure functions from
//! `pw_raw::ImageInfo` (U2-Q2).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use pw_raw::{ImageInfo, SUPPORTED_EXTENSIONS};

use crate::inspector_text::{ExifLabel, ExifValue, InspectorText, exif_value_text, inspector_text};

/// How long an image must have been the current one before the inspector asks for its Exif
/// summary or its metadata, so moving quickly with the arrow keys queues nothing (T103, T110).
pub(crate) const INSPECTOR_READ_DELAY: Duration = Duration::from_millis(200);
/// How often a frame is asked for while a read of the inspector is in flight or waits.
pub(crate) const INSPECTOR_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// One row of the Exif summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExifRow {
    pub label: ExifLabel,
    pub value: String,
}

/// The rows of the Exif summary of `info`, in the order of U2-Q2, each only when the file has the
/// value. Empty when it has none of them (`InspectorText::NoExif`).
pub(crate) fn exif_rows(info: &ImageInfo) -> Vec<ExifRow> {
    let mut rows = Vec::new();
    let mut push = |label, value: Option<String>| {
        if let Some(value) = value {
            rows.push(ExifRow { label, value });
        }
    };
    push(
        ExifLabel::Camera,
        camera_text(info.make.as_deref(), info.model.as_deref()),
    );
    push(ExifLabel::Lens, text_value(info.lens.as_deref()));
    push(
        ExifLabel::FocalLength,
        positive(info.focal_length)
            .map(|mm| exif_value_text(&ExifValue::FocalLength(one_decimal(mm)))),
    );
    push(
        ExifLabel::Aperture,
        positive(info.f_number)
            .map(|number| exif_value_text(&ExifValue::Aperture(one_decimal(number)))),
    );
    push(
        ExifLabel::ExposureTime,
        info.exposure_time
            .filter(|time| time.num != 0 && time.den != 0)
            // The rounding of `ExposureTime`'s `Display` in `pw-raw`, with its decimal point.
            .map(|time| time.to_string()),
    );
    push(
        ExifLabel::Iso,
        info.iso
            .filter(|iso| *iso > 0)
            .map(|iso| exif_value_text(&ExifValue::Iso(iso))),
    );
    push(
        ExifLabel::Taken,
        info.date_time_original.as_deref().and_then(taken_text),
    );
    rows
}

/// A text value without surrounding spaces or NUL padding; `None` when nothing is left.
fn text_value(value: Option<&str>) -> Option<String> {
    let value = value?.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    (!value.is_empty()).then(|| value.to_owned())
}

/// The camera: the model alone when it already begins with the make (case-insensitive), else the
/// make and the model; either alone when the other is missing.
fn camera_text(make: Option<&str>, model: Option<&str>) -> Option<String> {
    match (text_value(make), text_value(model)) {
        (Some(make), Some(model)) => {
            if model.to_lowercase().starts_with(&make.to_lowercase()) {
                Some(model)
            } else {
                Some(exif_value_text(&ExifValue::Camera { make, model }))
            }
        }
        (Some(make), None) => Some(make),
        (None, model) => model,
    }
}

/// A finite value above zero.
fn positive(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

/// At most one decimal, `.0` dropped, with a decimal point: 35 → "35", 24.5 → "24.5",
/// 2.8 → "2.8".
fn one_decimal(value: f64) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_owned()
}

/// DateTimeOriginal as `2026-10-30 14:03:12`; a value that is not `YYYY:MM:DD HH:MM:SS` is shown
/// as stored. `None` when it is empty.
fn taken_text(stored: &str) -> Option<String> {
    let trimmed = stored.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let well_formed = bytes.len() == 19
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 | 13 | 16 => *byte == b':',
            10 => *byte == b' ',
            _ => byte.is_ascii_digit(),
        });
    if !well_formed {
        return Some(stored.to_owned());
    }
    // Every byte is ASCII here, so the slices are on character boundaries.
    let part = |range: std::ops::Range<usize>| trimmed.get(range).unwrap_or_default().to_owned();
    Some(exif_value_text(&ExifValue::Taken {
        year: part(0..4),
        month: part(5..7),
        day: part(8..10),
        time: part(11..19),
    }))
}

/// Whether `pw-raw` reads the file kind: every kind RawStride lists (`SUPPORTED_EXTENSIONS`).
fn exif_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SUPPORTED_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

/// The answer of one Exif read.
#[derive(Debug)]
pub(crate) struct ExifRead {
    pub path: PathBuf,
    /// `pw_raw::inspect`'s error as text on failure.
    pub result: Result<ImageInfo, String>,
}

/// The reader has stopped: no read can be asked for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReaderStopped;

/// The Exif reader as the inspector uses it, so the tests use a fake (as `MetadataSubmit`).
pub(crate) trait ExifSubmit {
    /// Asks for the Exif summary of one image.
    fn read_exif(&mut self, image: PathBuf) -> Result<(), ReaderStopped>;

    fn try_recv(&mut self) -> Result<ExifRead, TryRecvError>;
}

/// The Exif reader: one thread it owns, which reads one image at a time with `pw_raw::inspect`.
/// The thread ends when the reader is dropped.
pub(crate) struct ExifReader {
    requests: Sender<PathBuf>,
    results: Receiver<ExifRead>,
}

impl ExifReader {
    /// Starts the thread. When it cannot be started, every read fails with `ReaderStopped`
    /// (the inspector then shows `InspectorText::ReaderStopped`); RawStride still runs.
    pub fn start() -> Self {
        let (requests, request_receiver) = mpsc::channel::<PathBuf>();
        let (result_sender, results) = mpsc::channel();
        let started = std::thread::Builder::new()
            .name("pw-exif-reader".into())
            .spawn(move || {
                for path in request_receiver {
                    let result = pw_raw::inspect(&path).map_err(|error| error.to_string());
                    if result_sender.send(ExifRead { path, result }).is_err() {
                        return;
                    }
                }
            });
        if let Err(error) = started {
            eprintln!("pw-loupe: the Exif reader could not be started: {error}");
        }
        Self { requests, results }
    }
}

impl ExifSubmit for ExifReader {
    fn read_exif(&mut self, image: PathBuf) -> Result<(), ReaderStopped> {
        self.requests.send(image).map_err(|_| ReaderStopped)
    }

    fn try_recv(&mut self) -> Result<ExifRead, TryRecvError> {
        self.results.try_recv()
    }
}

/// What the Info section shows for its image.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ExifShown<'a> {
    /// No result yet: the read waits for `INSPECTOR_READ_DELAY`, is in flight or not yet sent.
    Loading,
    /// The read failed; the reason.
    Error(&'a str),
    /// The rows; empty for a file without any of the values, or a kind `pw-raw` does not read.
    Rows(&'a [ExifRow]),
}

/// The text of the Info section when it shows no rows: loading, an error or no values. `None`
/// when there are rows.
pub(crate) fn exif_state_text(shown: &ExifShown<'_>) -> Option<String> {
    match shown {
        ExifShown::Loading => Some(inspector_text(&InspectorText::ExifLoading)),
        ExifShown::Error(reason) => Some(inspector_text(&InspectorText::ExifError(
            (*reason).to_owned(),
        ))),
        ExifShown::Rows([]) => Some(inspector_text(&InspectorText::NoExif)),
        ExifShown::Rows(_) => None,
    }
}

/// The Exif summary of the inspector's one image (T110).
#[derive(Default)]
pub(crate) struct ExifInspector {
    /// The current image (absolute, a kind `pw-raw` reads) and when it became the current one.
    image: Option<(PathBuf, Instant)>,
    /// The rows of `image`, or why they could not be read, once a result has come back.
    result: Option<Result<Vec<ExifRow>, String>>,
    /// `image` must be read: it is new.
    needs_read: bool,
    /// A read is in flight, for whichever image it was asked for.
    in_flight: bool,
}

impl ExifInspector {
    fn is_image(&self, path: &Path) -> bool {
        self.image.as_ref().is_some_and(|(image, _)| image == path)
    }

    /// Called every frame with the current image (absolute path) and whether the Info section is
    /// shown (the right column and the section open). Takes the results that came back, then asks
    /// for the image's summary when it is shown, no read is in flight and the image has been the
    /// current one for `INSPECTOR_READ_DELAY`. Returns how soon a frame is needed to go on (the
    /// rest of the delay, or a read in flight), if at all; at once (`Duration::ZERO`) when the
    /// shown section changed in this call (the image's result came back, or the reader stopped),
    /// since the section was drawn earlier in the frame and would otherwise wait for the next
    /// input (found with the Histogram section, T144).
    pub fn request(
        &mut self,
        image: Option<&Path>,
        shown: bool,
        now: Instant,
        reader: &mut impl ExifSubmit,
    ) -> Option<Duration> {
        let arrived = self.drain(reader);
        let wait = self.ask(image, shown, now, reader);
        if shown && arrived {
            Some(Duration::ZERO)
        } else {
            wait
        }
    }

    /// The part of [`Self::request`] after the results are taken: follows the image and asks
    /// for its read when it is due.
    fn ask(
        &mut self,
        image: Option<&Path>,
        shown: bool,
        now: Instant,
        reader: &mut impl ExifSubmit,
    ) -> Option<Duration> {
        let image = image.filter(|path| exif_supported(path));
        if self.image.as_ref().map(|(path, _)| path.as_path()) != image {
            self.image = image.map(|path| (path.to_path_buf(), now));
            self.result = None;
            self.needs_read = image.is_some();
        }
        if !shown {
            return None;
        }
        if self.in_flight {
            return Some(INSPECTOR_POLL_INTERVAL);
        }
        if !self.needs_read {
            return None;
        }
        let (path, since) = self.image.clone()?;
        let waited = now.saturating_duration_since(since);
        if waited < INSPECTOR_READ_DELAY {
            return Some(INSPECTOR_READ_DELAY - waited);
        }
        self.needs_read = false;
        match reader.read_exif(path) {
            Ok(()) => {
                self.in_flight = true;
                Some(INSPECTOR_POLL_INTERVAL)
            }
            Err(ReaderStopped) => {
                self.result = Some(Err(inspector_text(&InspectorText::ReaderStopped)));
                Some(Duration::ZERO)
            }
        }
    }

    /// Takes every result that came back; a result for another image is dropped. A stopped
    /// reader fails the read in flight. Returns whether the current image's section changed (its
    /// result came back, or the reader stopped while reading).
    fn drain(&mut self, reader: &mut impl ExifSubmit) -> bool {
        let mut changed = false;
        loop {
            match reader.try_recv() {
                Ok(ExifRead { path, result }) => {
                    self.in_flight = false;
                    if self.is_image(&path) {
                        self.result = Some(result.map(|info| exif_rows(&info)));
                        self.needs_read = false;
                        changed = true;
                    }
                }
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => {
                    if std::mem::take(&mut self.in_flight) && self.image.is_some() {
                        self.result = Some(Err(inspector_text(&InspectorText::ReaderStopped)));
                        self.needs_read = false;
                        changed = true;
                    }
                    return changed;
                }
            }
        }
    }

    /// A folder was loaded (`files`, absolute): another folder empties the summary; the same
    /// folder loaded again (a switch between the contact sheet and the loupe) keeps it. A read in
    /// flight stays in flight, so a second one is not sent before it is back.
    pub fn folder_loaded(&mut self, files: &[PathBuf]) {
        if !files.iter().any(|path| self.is_image(path)) {
            *self = Self {
                in_flight: self.in_flight,
                ..Self::default()
            };
        }
    }

    /// What the Info section shows for `image` (absolute path).
    pub fn shown(&self, image: &Path) -> ExifShown<'_> {
        if !exif_supported(image) {
            return ExifShown::Rows(&[]);
        }
        if !self.is_image(image) {
            return ExifShown::Loading;
        }
        match &self.result {
            None => ExifShown::Loading,
            Some(Ok(rows)) => ExifShown::Rows(rows),
            Some(Err(error)) => ExifShown::Error(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_raw::ExposureTime;
    use std::collections::VecDeque;

    fn info() -> ImageInfo {
        ImageInfo {
            make: Some("SONY".to_owned()),
            model: Some("ILCE-7RM5".to_owned()),
            lens: Some("FE 24-70mm F2.8 GM II".to_owned()),
            focal_length: Some(35.0),
            f_number: Some(2.8),
            exposure_time: Some(ExposureTime { num: 1, den: 250 }),
            iso: Some(400),
            date_time_original: Some("2026:10:30 14:03:12".to_owned()),
            ..ImageInfo::default()
        }
    }

    fn row(label: ExifLabel, value: &str) -> ExifRow {
        ExifRow {
            label,
            value: value.to_owned(),
        }
    }

    fn value_of(info: &ImageInfo, label: ExifLabel) -> Option<String> {
        exif_rows(info)
            .into_iter()
            .find(|row| row.label == label)
            .map(|row| row.value)
    }

    #[test]
    fn every_row_in_order() {
        assert_eq!(
            exif_rows(&info()),
            [
                row(ExifLabel::Camera, "SONY ILCE-7RM5"),
                row(ExifLabel::Lens, "FE 24-70mm F2.8 GM II"),
                row(ExifLabel::FocalLength, "35 mm"),
                row(ExifLabel::Aperture, "f/2.8"),
                row(ExifLabel::ExposureTime, "1/250 s"),
                row(ExifLabel::Iso, "ISO 400"),
                row(ExifLabel::Taken, "2026-10-30 14:03:12"),
            ]
        );
    }

    #[test]
    fn the_make_is_left_out_when_the_model_begins_with_it() {
        let camera = |make: &str, model: &str| {
            value_of(
                &ImageInfo {
                    make: Some(make.to_owned()),
                    model: Some(model.to_owned()),
                    ..ImageInfo::default()
                },
                ExifLabel::Camera,
            )
        };
        // Kept when the model does not begin with it.
        assert_eq!(
            camera("SONY", "ILCE-7RM5").as_deref(),
            Some("SONY ILCE-7RM5")
        );
        assert_eq!(
            camera("Panasonic", "DC-S5M2").as_deref(),
            Some("Panasonic DC-S5M2")
        );
        // Left out, case-insensitive.
        assert_eq!(
            camera("NIKON CORPORATION", "NIKON Z 8").as_deref(),
            Some("NIKON CORPORATION NIKON Z 8"),
            "the whole make must begin the model"
        );
        assert_eq!(
            camera("Canon", "Canon EOS R5").as_deref(),
            Some("Canon EOS R5")
        );
        assert_eq!(
            camera("SONY", "Sony ILCE-7RM5").as_deref(),
            Some("Sony ILCE-7RM5")
        );
        // Either alone.
        let only_make = ImageInfo {
            make: Some("SONY".to_owned()),
            ..ImageInfo::default()
        };
        assert_eq!(
            value_of(&only_make, ExifLabel::Camera).as_deref(),
            Some("SONY")
        );
        let only_model = ImageInfo {
            model: Some("ILCE-7RM5".to_owned()),
            ..ImageInfo::default()
        };
        assert_eq!(
            value_of(&only_model, ExifLabel::Camera).as_deref(),
            Some("ILCE-7RM5")
        );
    }

    #[test]
    fn focal_length_and_aperture_have_a_decimal_point_and_at_most_one_decimal() {
        let with = |focal_length: f64, f_number: f64| ImageInfo {
            focal_length: Some(focal_length),
            f_number: Some(f_number),
            ..ImageInfo::default()
        };
        let shown = |info: &ImageInfo| {
            (
                value_of(info, ExifLabel::FocalLength),
                value_of(info, ExifLabel::Aperture),
            )
        };
        assert_eq!(
            shown(&with(35.0, 8.0)),
            (Some("35 mm".to_owned()), Some("f/8".to_owned()))
        );
        assert_eq!(
            shown(&with(24.5, 2.8)),
            (Some("24.5 mm".to_owned()), Some("f/2.8".to_owned()))
        );
        assert_eq!(
            shown(&with(70.04, 5.6)),
            (Some("70 mm".to_owned()), Some("f/5.6".to_owned()))
        );
        assert_eq!(
            shown(&with(18.66, 1.4)),
            (Some("18.7 mm".to_owned()), Some("f/1.4".to_owned()))
        );
    }

    #[test]
    fn exposure_time_follows_pw_raw_with_a_decimal_point() {
        let time = |num, den| {
            value_of(
                &ImageInfo {
                    exposure_time: Some(ExposureTime { num, den }),
                    ..ImageInfo::default()
                },
                ExifLabel::ExposureTime,
            )
        };
        assert_eq!(time(1, 250).as_deref(), Some("1/250 s"));
        assert_eq!(time(1, 2).as_deref(), Some("0.5 s"));
        assert_eq!(time(2, 1).as_deref(), Some("2 s"));
        assert_eq!(time(13, 10).as_deref(), Some("1.3 s"));
        // pw-raw's "?" for a zero value is no value.
        assert_eq!(time(0, 1), None);
        assert_eq!(time(1, 0), None);
    }

    /// English numbers (T117): focal length, aperture and exposure time have a decimal point and
    /// never a decimal comma.
    #[test]
    fn focal_length_aperture_and_exposure_time_have_a_decimal_point() {
        let info = ImageInfo {
            focal_length: Some(24.5),
            f_number: Some(2.8),
            exposure_time: Some(ExposureTime { num: 1, den: 2 }),
            ..ImageInfo::default()
        };
        let focal_length = value_of(&info, ExifLabel::FocalLength);
        let aperture = value_of(&info, ExifLabel::Aperture);
        let exposure_time = value_of(&info, ExifLabel::ExposureTime);
        assert_eq!(focal_length.as_deref(), Some("24.5 mm"));
        assert_eq!(aperture.as_deref(), Some("f/2.8"));
        assert_eq!(exposure_time.as_deref(), Some("0.5 s"));
        for value in [focal_length, aperture, exposure_time]
            .into_iter()
            .flatten()
        {
            assert!(!value.contains(','), "{value}");
        }
    }

    #[test]
    fn the_date_is_reformatted_and_a_malformed_date_shown_as_stored() {
        let taken = |stored: &str| {
            value_of(
                &ImageInfo {
                    date_time_original: Some(stored.to_owned()),
                    ..ImageInfo::default()
                },
                ExifLabel::Taken,
            )
        };
        assert_eq!(
            taken("2026:10:30 14:03:12").as_deref(),
            Some("2026-10-30 14:03:12")
        );
        assert_eq!(
            taken("2026-10-30T14:03:12").as_deref(),
            Some("2026-10-30T14:03:12")
        );
        assert_eq!(
            taken("    :  :     :  :  ").as_deref(),
            Some("    :  :     :  :  ")
        );
        assert_eq!(taken("2026:10:30").as_deref(), Some("2026:10:30"));
        assert_eq!(taken(""), None);
    }

    #[test]
    fn missing_values_give_no_row_and_no_values_give_none() {
        let some = ImageInfo {
            lens: Some("  ".to_owned()),
            iso: Some(100),
            f_number: Some(f64::NAN),
            focal_length: Some(0.0),
            ..ImageInfo::default()
        };
        assert_eq!(exif_rows(&some), [row(ExifLabel::Iso, "ISO 100")]);
        assert!(exif_rows(&ImageInfo::default()).is_empty());
        assert_eq!(
            exif_state_text(&ExifShown::Rows(&[])).as_deref(),
            Some("The file has no camera information.")
        );
        assert_eq!(
            exif_state_text(&ExifShown::Rows(&[row(ExifLabel::Iso, "ISO 100")])),
            None
        );
        assert_eq!(
            exif_state_text(&ExifShown::Loading).as_deref(),
            Some("Reading camera information…")
        );
        assert_eq!(
            exif_state_text(&ExifShown::Error("I/O error: gone")).as_deref(),
            Some("The camera information could not be read: I/O error: gone")
        );
    }

    // --- the reader with a fake (T103's rules) ---

    #[derive(Default)]
    struct FakeReader {
        reads: Vec<PathBuf>,
        results: VecDeque<ExifRead>,
        stopped: bool,
    }

    impl FakeReader {
        fn answer(&mut self, path: &Path, result: Result<ImageInfo, String>) {
            self.results.push_back(ExifRead {
                path: path.to_path_buf(),
                result,
            });
        }
    }

    impl ExifSubmit for FakeReader {
        fn read_exif(&mut self, image: PathBuf) -> Result<(), ReaderStopped> {
            if self.stopped {
                return Err(ReaderStopped);
            }
            self.reads.push(image);
            Ok(())
        }

        fn try_recv(&mut self) -> Result<ExifRead, TryRecvError> {
            match self.results.pop_front() {
                Some(read) => Ok(read),
                None if self.stopped => Err(TryRecvError::Disconnected),
                None => Err(TryRecvError::Empty),
            }
        }
    }

    fn image(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn nothing_is_read_before_200_ms_or_while_the_section_is_not_shown() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let path = image("pw-loupe-exif-wait.ARW");
        let start = Instant::now();

        // Not shown (column or section collapsed): nothing, however long the image is current.
        assert_eq!(exif.request(Some(&path), false, start, &mut reader), None);
        let later = start + Duration::from_secs(5);
        assert_eq!(exif.request(Some(&path), false, later, &mut reader), None);
        assert!(reader.reads.is_empty());

        // Another image, shown: nothing before 200 ms, and the rest of the wait is returned.
        let other = image("pw-loupe-exif-wait.jpg");
        assert_eq!(
            exif.request(Some(&other), true, later, &mut reader),
            Some(INSPECTOR_READ_DELAY)
        );
        let almost = later + Duration::from_millis(199);
        assert_eq!(
            exif.request(Some(&other), true, almost, &mut reader),
            Some(Duration::from_millis(1))
        );
        assert!(reader.reads.is_empty());
        assert_eq!(exif.shown(&other), ExifShown::Loading);

        // At 200 ms the one read is sent.
        exif.request(
            Some(&other),
            true,
            later + INSPECTOR_READ_DELAY,
            &mut reader,
        );
        assert_eq!(reader.reads, [other]);
    }

    /// The section is drawn before the results are taken in a frame, so the frame that takes the
    /// image's result (or finds the reader stopped) asks for one more frame at once; without it
    /// the result waited for the next input (found with the Histogram section, T144).
    #[test]
    fn a_frame_is_asked_for_at_once_when_the_shown_section_changes() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let path = image("pw-loupe-exif-repaint.ARW");
        let start = Instant::now();
        let t = start + INSPECTOR_READ_DELAY;
        exif.request(Some(&path), true, start, &mut reader);
        assert_eq!(
            exif.request(Some(&path), true, t, &mut reader),
            Some(INSPECTOR_POLL_INTERVAL)
        );
        reader.answer(&path, Err("gone".into()));
        assert_eq!(
            exif.request(Some(&path), true, t, &mut reader),
            Some(Duration::ZERO)
        );
        assert_eq!(exif.request(Some(&path), true, t, &mut reader), None);

        // hidden: no frame is asked for
        let hidden = image("pw-loupe-exif-repaint-hidden.ARW");
        exif.request(Some(&hidden), true, t, &mut reader);
        let t = t + INSPECTOR_READ_DELAY;
        exif.request(Some(&hidden), true, t, &mut reader);
        reader.answer(&hidden, Err("gone".into()));
        assert_eq!(exif.request(Some(&hidden), false, t, &mut reader), None);

        // the reader stops while reading, and a read asked of a stopped reader
        let stopped = image("pw-loupe-exif-repaint-stopped.ARW");
        exif.request(Some(&stopped), true, t, &mut reader);
        let t = t + INSPECTOR_READ_DELAY;
        exif.request(Some(&stopped), true, t, &mut reader);
        reader.stopped = true;
        assert_eq!(
            exif.request(Some(&stopped), true, t, &mut reader),
            Some(Duration::ZERO)
        );
        let last = image("pw-loupe-exif-repaint-last.ARW");
        exif.request(Some(&last), true, t, &mut reader);
        assert_eq!(
            exif.request(Some(&last), true, t + INSPECTOR_READ_DELAY, &mut reader),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn moving_quickly_reads_nothing() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let start = Instant::now();
        for step in 0..5u32 {
            let path = image(&format!("pw-loupe-exif-step-{step}.DNG"));
            exif.request(
                Some(&path),
                true,
                start + Duration::from_millis(100) * step,
                &mut reader,
            );
        }
        assert!(reader.reads.is_empty());
        // No image: nothing.
        assert_eq!(
            exif.request(None, true, start + Duration::from_secs(9), &mut reader),
            None
        );
        assert!(reader.reads.is_empty());
    }

    #[test]
    fn one_read_in_flight_then_the_newest_image_and_a_stale_result_is_dropped() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let first = image("pw-loupe-exif-first.ARW");
        let second = image("pw-loupe-exif-second.RW2");
        let third = image("pw-loupe-exif-third.tif");
        let start = Instant::now();
        exif.request(Some(&first), true, start, &mut reader);
        exif.request(
            Some(&first),
            true,
            start + INSPECTOR_READ_DELAY,
            &mut reader,
        );
        assert_eq!(reader.reads, std::slice::from_ref(&first));

        // The image changes twice while the read runs: nothing more is sent.
        let t = start + Duration::from_millis(300);
        exif.request(Some(&second), true, t, &mut reader);
        let t = t + Duration::from_millis(300);
        assert_eq!(
            exif.request(Some(&third), true, t, &mut reader),
            Some(INSPECTOR_POLL_INTERVAL)
        );
        let t = t + Duration::from_secs(1);
        exif.request(Some(&third), true, t, &mut reader);
        assert_eq!(reader.reads.len(), 1, "one read in flight");

        // The first image's result comes back: it is dropped, and the newest image is asked for.
        reader.answer(&first, Ok(info()));
        exif.request(Some(&third), true, t, &mut reader);
        assert_eq!(reader.reads, [first.clone(), third.clone()]);
        assert_eq!(exif.shown(&third), ExifShown::Loading);
        assert_eq!(exif.shown(&first), ExifShown::Loading);

        // Its result is kept until the image changes (one more frame at once, so it is drawn),
        // and not read again.
        reader.answer(&third, Ok(info()));
        assert_eq!(
            exif.request(Some(&third), true, t, &mut reader),
            Some(Duration::ZERO)
        );
        assert_eq!(exif.shown(&third), ExifShown::Rows(&exif_rows(&info())));
        assert_eq!(exif.request(Some(&third), true, t, &mut reader), None);
        assert_eq!(reader.reads.len(), 2, "a read image is not read again");
        exif.request(Some(&second), true, t, &mut reader);
        assert_eq!(exif.shown(&third), ExifShown::Loading);
    }

    #[test]
    fn an_error_is_shown_and_a_stopped_reader_fails_the_read() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let path = image("pw-loupe-exif-error.jpg");
        let start = Instant::now();
        exif.request(Some(&path), true, start, &mut reader);
        exif.request(Some(&path), true, start + INSPECTOR_READ_DELAY, &mut reader);
        reader.answer(&path, Err("format error: not a JPEG".to_owned()));
        exif.request(Some(&path), true, start + INSPECTOR_READ_DELAY, &mut reader);
        assert_eq!(
            exif.shown(&path),
            ExifShown::Error("format error: not a JPEG")
        );

        // The reader stops while a read is in flight.
        let other = image("pw-loupe-exif-stopped.jpg");
        let t = start + Duration::from_secs(1);
        exif.request(Some(&other), true, t, &mut reader);
        exif.request(Some(&other), true, t + INSPECTOR_READ_DELAY, &mut reader);
        reader.stopped = true;
        exif.request(Some(&other), true, t + INSPECTOR_READ_DELAY, &mut reader);
        assert_eq!(
            exif.shown(&other),
            ExifShown::Error("Exif reading has stopped")
        );

        // A read asked of a stopped reader.
        let last = image("pw-loupe-exif-last.jpg");
        let t = t + Duration::from_secs(1);
        exif.request(Some(&last), true, t, &mut reader);
        exif.request(Some(&last), true, t + INSPECTOR_READ_DELAY, &mut reader);
        assert_eq!(
            exif.shown(&last),
            ExifShown::Error("Exif reading has stopped")
        );
    }

    #[test]
    fn a_folder_change_empties_the_summary_and_the_same_folder_keeps_it() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let path = image("pw-loupe-exif-folder.ARW");
        let start = Instant::now();
        exif.request(Some(&path), true, start, &mut reader);
        let t = start + INSPECTOR_READ_DELAY;
        exif.request(Some(&path), true, t, &mut reader);
        reader.answer(&path, Ok(info()));
        exif.request(Some(&path), true, t, &mut reader);
        assert_eq!(exif.shown(&path), ExifShown::Rows(&exif_rows(&info())));

        // The same folder loaded again (contact sheet ↔ loupe) keeps it.
        exif.folder_loaded(std::slice::from_ref(&path));
        assert_eq!(exif.shown(&path), ExifShown::Rows(&exif_rows(&info())));

        // Another folder empties it.
        exif.folder_loaded(&[image("pw-loupe-exif-elsewhere.ARW")]);
        assert_eq!(exif.shown(&path), ExifShown::Loading);
    }

    #[test]
    fn a_kind_pw_raw_does_not_read_is_never_read_and_has_no_values() {
        let mut reader = FakeReader::default();
        let mut exif = ExifInspector::default();
        let text = image("pw-loupe-exif-notes.txt");
        let start = Instant::now();
        exif.request(Some(&text), true, start, &mut reader);
        exif.request(
            Some(&text),
            true,
            start + Duration::from_secs(1),
            &mut reader,
        );
        assert!(reader.reads.is_empty());
        assert_eq!(exif.shown(&text), ExifShown::Rows(&[]));
        // Every listed kind is read, in either case.
        for name in [
            "a.ARW", "a.rw2", "a.DNG", "a.nef", "a.CR2", "a.orf", "a.PEF", "a.RAF",
        ] {
            assert!(exif_supported(Path::new(name)), "{name}");
        }
        for name in ["a.srw", "a.JPG", "a.jpeg", "a.TIF", "a.tiff"] {
            assert!(exif_supported(Path::new(name)), "{name}");
        }
    }
}
