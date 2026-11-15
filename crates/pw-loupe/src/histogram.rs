//! The Histogram section of the preview's right column and of the inspector (ADR 0017 §2.6, U7,
//! T142): what the panel draws, made from the view core's statistics as plain data. The drawing
//! code in `main.rs` only paints the bars and the texts it is given; the texts themselves are
//! made in `preview_text.rs` (rule 13).
//!
//! The inspector's statistics (U7c, T144): the inspector is drawn in the contact sheet, where the
//! view core has no image open, so it has a reader of its own ([`HistogramReader`]) with the
//! rules of the Exif reader (T110): a read only while the section is shown and after the image
//! has been the current one for the inspector's read delay; one read in flight; the newest image
//! asked for when it comes back; a result for another image dropped. The last
//! [`KEPT_HISTOGRAMS`] results are kept by path and display mode ([`HistogramInspector`]), also
//! those the preview gave, so nothing is read for an image the preview showed. A metadata write
//! by RawStride does not change the pixels, so a kept result follows it (U7c-Q7, T145).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use pw_viewcore::{Backend, ImageStatistics, Source, Statistics, StatisticsOrigin};

use crate::preview_text::{HistogramText, clipped_text, deep_shadows_text};

/// The bins of one channel.
pub const HISTOGRAM_BINS: usize = 256;

/// The bar heights of the three channels (R, G, B), each from 0 (empty) to 1 (the panel's full
/// height), one per bin.
pub type HistogramBars = [[f32; HISTOGRAM_BINS]; 3];

/// What the Histogram section shows.
#[derive(Clone, Debug, PartialEq)]
pub enum HistogramPanel {
    /// The statistics have not come yet.
    Reading,
    /// The file has none.
    NoHistogram,
    /// The histogram with the lines under it.
    Ready(Box<HistogramView>),
}

/// A histogram and the lines under it.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramView {
    pub bars: HistogramBars,
    /// Where the numbers come from (G1): RAW data, Embedded JPEG or TIFF.
    pub source: HistogramText,
    /// "Clipped: R 0.12 %  G 3.40 %  B 0.00 %".
    pub clipped: String,
    /// "Deep shadows: 1.25 %".
    pub deep_shadows: String,
}

/// The panel for the statistics of the shown image: `None` while they are not ready.
pub fn histogram_panel(statistics: Option<&ImageStatistics>) -> HistogramPanel {
    match statistics {
        None => HistogramPanel::Reading,
        Some(ImageStatistics::Unavailable) => HistogramPanel::NoHistogram,
        Some(ImageStatistics::Available(statistics)) => {
            HistogramPanel::Ready(Box::new(histogram_view(statistics)))
        }
    }
}

/// The histogram and the lines under it for `statistics`.
pub fn histogram_view(statistics: &Statistics) -> HistogramView {
    let [r, g, b] = statistics.clipped;
    HistogramView {
        bars: histogram_bars(&statistics.histogram),
        source: histogram_source_text(statistics.origin),
        clipped: clipped_text(r, g, b),
        deep_shadows: deep_shadows_text(statistics.deep_shadows),
    }
}

/// The text that names where the statistics come from (G1).
pub fn histogram_source_text(origin: StatisticsOrigin) -> HistogramText {
    match origin {
        StatisticsOrigin::RawData => HistogramText::RawData,
        StatisticsOrigin::EmbeddedJpeg => HistogramText::EmbeddedJpeg,
        StatisticsOrigin::Tiff => HistogramText::Tiff,
    }
}

/// The bar heights of a histogram (U7-Q7). Each bin is taken as its share of its own channel's
/// pixels, so the green channel of a Bayer sensor (twice the photosites) is not drawn twice as
/// high. The heights are scaled to the highest bin of the three channels with **the two end bins
/// left out of the scale**, so a clipped image (or one with crushed shadows) does not flatten the
/// rest; an end bin higher than that is drawn at the full height. A histogram with pixels only in
/// its end bins is scaled to those.
pub fn histogram_bars(histogram: &[[u64; HISTOGRAM_BINS]; 3]) -> HistogramBars {
    let mut shares = [[0.0f64; HISTOGRAM_BINS]; 3];
    for (channel, shares) in histogram.iter().zip(shares.iter_mut()) {
        let pixels: u64 = channel.iter().sum();
        if pixels == 0 {
            continue;
        }
        for (count, share) in channel.iter().zip(shares.iter_mut()) {
            *share = *count as f64 / pixels as f64;
        }
    }
    let highest = |bins: &[f64]| bins.iter().copied().fold(0.0f64, f64::max);
    let inner = shares
        .iter()
        .map(|channel| highest(channel.get(1..HISTOGRAM_BINS - 1).unwrap_or(&[])))
        .fold(0.0f64, f64::max);
    let scale = if inner > 0.0 {
        inner
    } else {
        shares
            .iter()
            .map(|channel| highest(channel))
            .fold(0.0f64, f64::max)
    };
    let mut bars = [[0.0f32; HISTOGRAM_BINS]; 3];
    if scale <= 0.0 {
        return bars;
    }
    for (shares, bars) in shares.iter().zip(bars.iter_mut()) {
        for (share, bar) in shares.iter().zip(bars.iter_mut()) {
            *bar = (share / scale).min(1.0) as f32;
        }
    }
    bars
}

// ---------- the inspector's statistics (U7c, T144) ----------

/// How many results the inspector keeps (U7c-Q3).
pub const KEPT_HISTOGRAMS: usize = 64;

/// The display mode an image's statistics are of: a JPEG or TIFF file is itself in both modes,
/// as the view core keys it.
pub fn histogram_source(path: &Path, mode: Source) -> Source {
    if pw_viewcore::is_shown_as_itself(path) {
        Source::Jpeg
    } else {
        mode
    }
}

/// A file's size and modification time, so a kept result of a file changed since is read again
/// (U7c-Q3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

/// The stamp of `path` now; `None` when it cannot be read.
pub fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(FileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

/// The answer of one read.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramRead {
    pub path: PathBuf,
    pub source: Source,
    /// The file's stamp just before it was opened.
    pub stamp: Option<FileStamp>,
    /// `Unavailable` also when the file could not be opened (U7c-Q6).
    pub statistics: ImageStatistics,
}

/// The reader has stopped: no read can be asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistogramReaderStopped;

/// The reader as the inspector uses it, so the tests use a fake.
pub trait HistogramSubmit {
    /// Asks for the statistics of one image in one display mode.
    fn read_histogram(
        &mut self,
        path: PathBuf,
        source: Source,
    ) -> Result<(), HistogramReaderStopped>;

    fn try_recv(&mut self) -> Result<HistogramRead, TryRecvError>;
}

/// Locks a mutex; a poisoned lock is taken over (the data is a plain list).
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

/// What [`HistogramReads`] holds.
#[derive(Debug, Default)]
struct ReadsState {
    /// The files asked for and not let go of yet.
    held: Vec<PathBuf>,
    /// Each metadata write's file and its stamp just before the write, in the order of the
    /// writes, until its answer comes (U7c-Q7).
    before_write: Vec<(PathBuf, Option<FileStamp>)>,
}

/// The files the reader has been asked to read and has not let go of yet, so a metadata write
/// of one of them waits until the read is over (U7c-Q4; as the view core's `release`, ADR 0014
/// §4.4). A path is listed from the moment it is asked for until its session is dropped. It also
/// holds the stamp of each file just before RawStride's metadata write of it, until the write's
/// answer comes (U7c-Q7, T145). It can be cloned and used from any thread.
#[derive(Clone, Debug, Default)]
pub struct HistogramReads {
    inner: Arc<(Mutex<ReadsState>, Condvar)>,
}

impl HistogramReads {
    fn asked(&self, path: &Path) {
        lock(&self.inner.0).held.push(path.to_path_buf());
    }

    fn finished(&self, path: &Path) {
        let (state, changed) = &*self.inner;
        let mut state = lock(state);
        if let Some(index) = state.held.iter().position(|listed| listed == path) {
            state.held.swap_remove(index);
        }
        changed.notify_all();
    }

    /// Notes the stamp of `path` now, just before RawStride's metadata write of it (U7c-Q7).
    pub fn note_before_write(&self, path: &Path) {
        let stamp = file_stamp(path);
        lock(&self.inner.0)
            .before_write
            .push((path.to_path_buf(), stamp));
    }

    /// Takes the stamp noted before the oldest write of `path` not yet answered, which is then
    /// forgotten: `None` when none is noted; `Some(None)` when the stamp could not be read then.
    pub fn take_before_write(&self, path: &Path) -> Option<Option<FileStamp>> {
        let mut state = lock(&self.inner.0);
        let index = state
            .before_write
            .iter()
            .position(|(noted, _)| noted == path)?;
        Some(state.before_write.remove(index).1)
    }

    /// Waits until no read of `path` is asked for or running, at most `timeout`. Returns `true`
    /// when none is left, at once when there is none.
    pub fn wait_for(&self, path: &Path, timeout: Duration) -> bool {
        let deadline = Instant::now().checked_add(timeout);
        let (state, changed) = &*self.inner;
        let mut state = lock(state);
        loop {
            if !state.held.iter().any(|listed| listed == path) {
                return true;
            }
            let wait = match deadline {
                Some(deadline) => {
                    let now = Instant::now();
                    if deadline <= now {
                        return false;
                    }
                    deadline - now
                }
                None => Duration::from_secs(1),
            };
            state = match changed.wait_timeout(state, wait) {
                Ok((state, _)) => state,
                Err(error) => error.into_inner().0,
            };
        }
    }
}

/// The statistics of `path` as `source`: the file is opened with `backend`, asked for its
/// statistics and closed at once; nothing is rendered (U7c-Q2). A file that cannot be opened has
/// none.
pub fn read_statistics<B: Backend>(backend: &B, path: &Path, source: Source) -> ImageStatistics {
    let Ok((mut session, _)) = backend.open(path, source) else {
        return ImageStatistics::Unavailable;
    };
    let statistics = backend.statistics(&mut session);
    drop(session);
    statistics.map_or(ImageStatistics::Unavailable, |statistics| {
        ImageStatistics::Available(Arc::new(statistics))
    })
}

/// The inspector's reader (U7c-Q2): one thread it owns, which reads one image at a time with the
/// view core's backend. The thread ends when the reader is dropped.
#[derive(Debug)]
pub struct HistogramReader {
    requests: Sender<(PathBuf, Source)>,
    results: Receiver<HistogramRead>,
    reads: HistogramReads,
}

impl HistogramReader {
    /// Starts the thread with `backend` (a clone of the view core's, so the same LibRaw build
    /// and limits); `reads` lists what it holds, for the metadata writes. When the thread cannot
    /// be started, every read fails with `HistogramReaderStopped` (the section then says the
    /// file has no histogram); RawStride still runs.
    pub fn start<B: Backend>(backend: B, reads: HistogramReads) -> Self {
        let (requests, request_receiver) = mpsc::channel::<(PathBuf, Source)>();
        let (result_sender, results) = mpsc::channel();
        let held = reads.clone();
        let started = std::thread::Builder::new()
            .name("pw-histogram-reader".into())
            .spawn(move || {
                for (path, source) in request_receiver {
                    let stamp = file_stamp(&path);
                    let statistics = read_statistics(&backend, &path, source);
                    held.finished(&path);
                    let read = HistogramRead {
                        path,
                        source,
                        stamp,
                        statistics,
                    };
                    if result_sender.send(read).is_err() {
                        return;
                    }
                }
            });
        if let Err(error) = started {
            eprintln!("pw-loupe: the histogram reader could not be started: {error}");
        }
        Self {
            requests,
            results,
            reads,
        }
    }
}

impl HistogramSubmit for HistogramReader {
    fn read_histogram(
        &mut self,
        path: PathBuf,
        source: Source,
    ) -> Result<(), HistogramReaderStopped> {
        // listed before it is sent, so a write asked for meanwhile already waits for it
        self.reads.asked(&path);
        self.requests
            .send((path, source))
            .map_err(|mpsc::SendError((path, _))| {
                self.reads.finished(&path);
                HistogramReaderStopped
            })
    }

    fn try_recv(&mut self) -> Result<HistogramRead, TryRecvError> {
        self.results.try_recv()
    }
}

impl HistogramReader {
    /// The list shared with the metadata writes (the stamps noted before them, U7c-Q7).
    pub fn reads(&self) -> &HistogramReads {
        &self.reads
    }
}

/// One kept result.
#[derive(Clone, Debug)]
struct Kept {
    path: PathBuf,
    source: Source,
    stamp: Option<FileStamp>,
    statistics: ImageStatistics,
}

/// The inspector's Histogram section (U7c, T144): its image, the read in flight and the kept
/// results.
#[derive(Debug)]
pub struct HistogramInspector {
    /// How long an image must have been the current one before it is read, and how often a frame
    /// is asked for while a read is in flight (the inspector's, T110).
    delay: Duration,
    poll: Duration,
    /// The current image (absolute), its display mode and when it became the current one.
    image: Option<(PathBuf, Source, Instant)>,
    /// `image` must be read: it has no fresh kept result.
    needs_read: bool,
    /// A read is in flight, for whichever image it was asked for.
    in_flight: bool,
    /// The current image's result when the reader has stopped; not kept.
    stopped: Option<ImageStatistics>,
    /// The kept results, the oldest first.
    kept: VecDeque<Kept>,
}

impl HistogramInspector {
    /// An inspector that reads an image after it has been the current one for `delay`, and asks
    /// for a frame every `poll` while a read is in flight.
    pub fn new(delay: Duration, poll: Duration) -> Self {
        Self {
            delay,
            poll,
            image: None,
            needs_read: false,
            in_flight: false,
            stopped: None,
            kept: VecDeque::new(),
        }
    }

    fn is_image(&self, path: &Path, source: Source) -> bool {
        self.image
            .as_ref()
            .is_some_and(|(image, image_source, _)| image == path && *image_source == source)
    }

    fn kept_index(&self, path: &Path, source: Source) -> Option<usize> {
        self.kept
            .iter()
            .position(|kept| kept.path == path && kept.source == source)
    }

    /// Keeps a result, the newest last; the oldest goes when there are more than
    /// `KEPT_HISTOGRAMS`.
    fn keep(&mut self, kept: Kept) {
        if let Some(index) = self.kept_index(&kept.path, kept.source) {
            self.kept.remove(index);
        }
        self.kept.push_back(kept);
        while self.kept.len() > KEPT_HISTOGRAMS {
            self.kept.pop_front();
        }
    }

    /// Whether a kept result of `path` as `source` is there and its file has not changed since;
    /// a changed file's result is dropped.
    fn has_fresh(&mut self, path: &Path, source: Source) -> bool {
        let Some(index) = self.kept_index(path, source) else {
            return false;
        };
        let fresh = self
            .kept
            .get(index)
            .is_some_and(|kept| kept.stamp == file_stamp(path));
        if !fresh {
            self.kept.remove(index);
        }
        fresh
    }

    /// The statistics the preview's last frame gave for `path` (absolute) as `source` (U7c-Q1):
    /// kept like a read's result, so the inspector reads nothing for that image. Called every
    /// frame; an unchanged result is not stored again.
    pub fn preview_result(&mut self, path: &Path, source: Source, statistics: &ImageStatistics) {
        if let Some(index) = self.kept_index(path, source)
            && self
                .kept
                .get(index)
                .is_some_and(|kept| kept.statistics == *statistics)
        {
            return;
        }
        self.keep(Kept {
            path: path.to_path_buf(),
            source,
            stamp: file_stamp(path),
            statistics: statistics.clone(),
        });
        if self.is_image(path, source) {
            self.needs_read = false;
        }
    }

    /// Called every frame with the inspector's image (absolute path), the tab's display mode and
    /// whether the Histogram section is shown (the inspector and the section open). Takes the
    /// results that came back, then asks for the image's statistics when it is shown, has no
    /// fresh kept result, no read is in flight and it has been the current one for the delay.
    /// Returns how soon a frame is needed to go on (the rest of the delay, or a read in flight),
    /// if at all.
    pub fn request(
        &mut self,
        image: Option<&Path>,
        mode: Source,
        shown: bool,
        now: Instant,
        reader: &mut impl HistogramSubmit,
    ) -> Option<Duration> {
        self.drain(reader);
        let subject = image.map(|path| (path, histogram_source(path, mode)));
        let current = self
            .image
            .as_ref()
            .map(|(path, source, _)| (path.as_path(), *source));
        if current != subject {
            self.stopped = None;
            self.needs_read = match subject {
                Some((path, source)) => !self.has_fresh(path, source),
                None => false,
            };
            self.image = subject.map(|(path, source)| (path.to_path_buf(), source, now));
        }
        if !shown {
            return None;
        }
        if self.in_flight {
            return Some(self.poll);
        }
        if !self.needs_read {
            return None;
        }
        let (path, source, since) = self.image.clone()?;
        let waited = now.saturating_duration_since(since);
        if waited < self.delay {
            return Some(self.delay - waited);
        }
        self.needs_read = false;
        match reader.read_histogram(path, source) {
            Ok(()) => {
                self.in_flight = true;
                Some(self.poll)
            }
            Err(HistogramReaderStopped) => {
                self.stopped = Some(ImageStatistics::Unavailable);
                None
            }
        }
    }

    /// Takes every result that came back; a result for another image is dropped. A stopped
    /// reader fails the read in flight.
    fn drain(&mut self, reader: &mut impl HistogramSubmit) {
        loop {
            match reader.try_recv() {
                Ok(read) => {
                    self.in_flight = false;
                    if self.is_image(&read.path, read.source) {
                        self.needs_read = false;
                        self.keep(Kept {
                            path: read.path,
                            source: read.source,
                            stamp: read.stamp,
                            statistics: read.statistics,
                        });
                    }
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    if std::mem::take(&mut self.in_flight) && self.image.is_some() {
                        self.stopped = Some(ImageStatistics::Unavailable);
                        self.needs_read = false;
                    }
                    return;
                }
            }
        }
    }

    /// RawStride's metadata write of `path` (absolute) has answered (U7c-Q7, T145): the stamp
    /// noted just before it is taken from `reads` and forgotten. When the write succeeded, the
    /// kept results of `path` (both display modes) whose stamp equals the noted one take the
    /// file's stamp as it is now, so they are not read again: the write did not change the
    /// pixels. A kept result with another stamp (the file was changed outside RawStride before
    /// the write) is left as it is and read again; a failed write changes nothing.
    pub fn written(&mut self, path: &Path, succeeded: bool, reads: &HistogramReads) {
        let Some(before) = reads.take_before_write(path) else {
            return;
        };
        if !succeeded {
            return;
        }
        let mut now = None;
        for kept in self
            .kept
            .iter_mut()
            .filter(|kept| kept.path == path && kept.stamp == before)
        {
            kept.stamp = *now.get_or_insert_with(|| file_stamp(path));
        }
    }

    /// A folder was loaded (`files`, absolute): another folder empties the kept results, as the
    /// Exif reader does (T110); the same folder loaded again (a switch between the contact sheet
    /// and the loupe) keeps them. A read in flight stays in flight, so a second one is not sent
    /// before it is back.
    pub fn folder_loaded(&mut self, files: &[PathBuf]) {
        let listed: std::collections::HashSet<&Path> = files.iter().map(PathBuf::as_path).collect();
        let same_folder = self
            .image
            .as_ref()
            .is_some_and(|(path, _, _)| listed.contains(path.as_path()))
            || self
                .kept
                .iter()
                .any(|kept| listed.contains(kept.path.as_path()));
        if !same_folder {
            *self = Self {
                in_flight: self.in_flight,
                ..Self::new(self.delay, self.poll)
            };
        }
    }

    /// What the Histogram section shows for `image` (absolute path) in the tab's display mode:
    /// its kept result, or "Reading …" until it comes; a failed read and a stopped reader give
    /// "No histogram for this file" (U7c-Q6).
    pub fn shown(&self, image: &Path, mode: Source) -> HistogramPanel {
        let source = histogram_source(image, mode);
        if let Some(kept) = self
            .kept
            .iter()
            .find(|kept| kept.path == image && kept.source == source)
        {
            return histogram_panel(Some(&kept.statistics));
        }
        if self.is_image(image, source) {
            return histogram_panel(self.stopped.as_ref());
        }
        HistogramPanel::Reading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> [[u64; HISTOGRAM_BINS]; 3] {
        [[0; HISTOGRAM_BINS]; 3]
    }

    fn statistics(origin: StatisticsOrigin) -> Statistics {
        let mut histogram = empty();
        histogram[0][10] = 50;
        histogram[0][20] = 50;
        histogram[1][10] = 100;
        histogram[2][255] = 100;
        Statistics {
            origin,
            histogram,
            clipped: [0.0012, 0.034, 0.0],
            deep_shadows: 0.0125,
        }
    }

    #[test]
    fn the_bars_are_scaled_to_the_highest_bin_without_the_two_end_bins() {
        let mut histogram = empty();
        // red: a huge clipped end bin and two inner bins
        histogram[0][255] = 800;
        histogram[0][100] = 150;
        histogram[0][101] = 50;
        // green: crushed shadows in the first bin and one inner bin
        histogram[1][0] = 900;
        histogram[1][50] = 100;
        // blue: inner bins only
        histogram[2][30] = 20;
        histogram[2][31] = 80;
        let bars = histogram_bars(&histogram);
        // the highest inner share is blue's 80 % (bin 31): the scale
        assert_eq!(bars[2][31], 1.0);
        assert_eq!(bars[2][30], 0.25);
        // red's inner bins are 15 % and 5 % of its pixels
        assert!((bars[0][100] - 0.1875).abs() < 1e-6, "{}", bars[0][100]);
        assert!((bars[0][101] - 0.0625).abs() < 1e-6, "{}", bars[0][101]);
        // the end bins do not set the scale; higher than it, they are drawn at the full height
        assert_eq!(bars[0][255], 1.0);
        assert_eq!(bars[1][0], 1.0);
        assert!((bars[1][50] - 0.125).abs() < 1e-6, "{}", bars[1][50]);
        // every other bin is empty
        assert_eq!(bars[0][0], 0.0);
        assert_eq!(bars[2][255], 0.0);
        assert!(bars.iter().flatten().all(|h| (0.0..=1.0).contains(h)));
    }

    #[test]
    fn an_end_bin_lower_than_the_scale_keeps_its_own_height() {
        let mut histogram = empty();
        histogram[0][0] = 10;
        histogram[0][128] = 40;
        histogram[0][255] = 50;
        let bars = histogram_bars(&histogram);
        assert_eq!(bars[0][128], 1.0);
        assert_eq!(bars[0][0], 0.25);
        // higher than the scale: the full height, not more
        assert_eq!(bars[0][255], 1.0);
    }

    #[test]
    fn each_channel_is_taken_as_a_share_of_its_own_pixels() {
        // a Bayer sensor: twice as many green photosites, the same distribution
        let mut histogram = empty();
        histogram[0][40] = 100;
        histogram[1][40] = 200;
        histogram[2][40] = 100;
        let bars = histogram_bars(&histogram);
        assert_eq!([bars[0][40], bars[1][40], bars[2][40]], [1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_histogram_with_pixels_only_in_its_end_bins_is_scaled_to_them() {
        let mut histogram = empty();
        histogram[0][255] = 75;
        histogram[0][0] = 25;
        histogram[1][0] = 10;
        let bars = histogram_bars(&histogram);
        // green has all its pixels in bin 0: the highest share
        assert_eq!(bars[1][0], 1.0);
        assert_eq!(bars[0][255], 0.75);
        assert_eq!(bars[0][0], 0.25);
    }

    #[test]
    fn an_empty_histogram_has_no_bars() {
        let bars = histogram_bars(&empty());
        assert!(bars.iter().flatten().all(|h| *h == 0.0));
    }

    #[test]
    fn the_source_text_names_where_the_numbers_come_from() {
        assert_eq!(
            histogram_source_text(StatisticsOrigin::RawData),
            HistogramText::RawData
        );
        assert_eq!(
            histogram_source_text(StatisticsOrigin::EmbeddedJpeg),
            HistogramText::EmbeddedJpeg
        );
        assert_eq!(
            histogram_source_text(StatisticsOrigin::Tiff),
            HistogramText::Tiff
        );
    }

    #[test]
    fn the_view_has_the_bars_the_source_and_the_share_texts() {
        let view = histogram_view(&statistics(StatisticsOrigin::RawData));
        assert_eq!(view.source, HistogramText::RawData);
        assert_eq!(view.clipped, "Clipped: R 0.12 %  G 3.40 %  B 0.00 %");
        assert_eq!(view.deep_shadows, "Deep shadows: 1.25 %");
        // red has half its pixels in each of two bins, green all in one: the scale
        assert_eq!(view.bars[1][10], 1.0);
        assert_eq!(view.bars[0][10], 0.5);
        assert_eq!(view.bars[0][20], 0.5);
        // blue is only in the top bin, drawn at the full height
        assert_eq!(view.bars[2][255], 1.0);

        let view = histogram_view(&statistics(StatisticsOrigin::EmbeddedJpeg));
        assert_eq!(view.source, HistogramText::EmbeddedJpeg);
        let view = histogram_view(&statistics(StatisticsOrigin::Tiff));
        assert_eq!(view.source, HistogramText::Tiff);
    }

    // ---------- the inspector's reader (U7c, T144), with a fake as the Exif reader's tests ----------

    use crate::preview_text::histogram_text;
    use pw_viewcore::{DecodeError, ImageSize, Rect, Rgb};

    const DELAY: Duration = Duration::from_millis(200);
    const POLL: Duration = Duration::from_millis(100);

    #[derive(Default)]
    struct FakeReader {
        reads: Vec<(PathBuf, Source)>,
        results: VecDeque<HistogramRead>,
        stopped: bool,
    }

    impl FakeReader {
        fn answer(&mut self, path: &Path, source: Source, statistics: ImageStatistics) {
            self.results.push_back(HistogramRead {
                path: path.to_path_buf(),
                source,
                stamp: file_stamp(path),
                statistics,
            });
        }
    }

    impl HistogramSubmit for FakeReader {
        fn read_histogram(
            &mut self,
            path: PathBuf,
            source: Source,
        ) -> Result<(), HistogramReaderStopped> {
            if self.stopped {
                return Err(HistogramReaderStopped);
            }
            self.reads.push((path, source));
            Ok(())
        }

        fn try_recv(&mut self) -> Result<HistogramRead, TryRecvError> {
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

    fn inspector() -> HistogramInspector {
        HistogramInspector::new(DELAY, POLL)
    }

    fn found(share: f64) -> ImageStatistics {
        let mut statistics = statistics(StatisticsOrigin::RawData);
        statistics.deep_shadows = share;
        ImageStatistics::Available(Arc::new(statistics))
    }

    fn ready(share: f64) -> HistogramPanel {
        histogram_panel(Some(&found(share)))
    }

    /// Asks for `path` and lets the delay pass, so its read is sent; returns the time after.
    fn read_now(
        histograms: &mut HistogramInspector,
        reader: &mut FakeReader,
        path: &Path,
        at: Instant,
    ) -> Instant {
        histograms.request(Some(path), Source::Raw, true, at, reader);
        let after = at + DELAY;
        histograms.request(Some(path), Source::Raw, true, after, reader);
        after
    }

    #[test]
    fn nothing_is_read_while_the_section_is_hidden_or_before_the_delay() {
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let path = image("pw-loupe-histogram-wait.ARW");
        let start = Instant::now();

        // hidden: nothing, however long the image is current
        let hidden = |histograms: &mut HistogramInspector, reader: &mut FakeReader, at| {
            histograms.request(Some(&path), Source::Raw, false, at, reader)
        };
        assert_eq!(hidden(&mut histograms, &mut reader, start), None);
        let later = start + Duration::from_secs(5);
        assert_eq!(hidden(&mut histograms, &mut reader, later), None);
        assert!(reader.reads.is_empty());

        // another image, shown: nothing before the delay, and the rest of the wait is returned
        let other = image("pw-loupe-histogram-wait.RW2");
        assert_eq!(
            histograms.request(Some(&other), Source::Raw, true, later, &mut reader),
            Some(DELAY)
        );
        let almost = later + Duration::from_millis(199);
        assert_eq!(
            histograms.request(Some(&other), Source::Raw, true, almost, &mut reader),
            Some(Duration::from_millis(1))
        );
        assert!(reader.reads.is_empty());
        assert_eq!(
            histograms.shown(&other, Source::Raw),
            HistogramPanel::Reading
        );

        // after the delay the one read is sent, in the tab's mode
        assert_eq!(
            histograms.request(Some(&other), Source::Raw, true, later + DELAY, &mut reader),
            Some(POLL)
        );
        assert_eq!(reader.reads, [(other, Source::Raw)]);

        // moving on quickly reads nothing more; no image asks for nothing
        let mut histograms = inspector();
        let mut reader = FakeReader::default();
        for step in 0..5u32 {
            let path = image(&format!("pw-loupe-histogram-step-{step}.DNG"));
            let at = start + Duration::from_millis(100) * step;
            histograms.request(Some(&path), Source::Raw, true, at, &mut reader);
        }
        assert!(reader.reads.is_empty());
        let end = start + Duration::from_secs(9);
        assert_eq!(
            histograms.request(None, Source::Raw, true, end, &mut reader),
            None
        );
        assert!(reader.reads.is_empty());
    }

    #[test]
    fn one_read_in_flight_the_newest_image_next_and_a_result_for_another_image_dropped() {
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let first = image("pw-loupe-histogram-first.ARW");
        let second = image("pw-loupe-histogram-second.RW2");
        let third = image("pw-loupe-histogram-third.NEF");
        let t = read_now(&mut histograms, &mut reader, &first, Instant::now());
        assert_eq!(reader.reads, [(first.clone(), Source::Raw)]);

        // the image changes twice while the read runs: nothing more is sent
        let t = t + Duration::from_millis(300);
        histograms.request(Some(&second), Source::Raw, true, t, &mut reader);
        let t = t + Duration::from_millis(300);
        assert_eq!(
            histograms.request(Some(&third), Source::Raw, true, t, &mut reader),
            Some(POLL)
        );
        let t = t + Duration::from_secs(1);
        histograms.request(Some(&third), Source::Raw, true, t, &mut reader);
        assert_eq!(reader.reads.len(), 1, "one read in flight");

        // the first image's result comes back: it is dropped, and the newest image is asked for
        reader.answer(&first, Source::Raw, found(0.1));
        histograms.request(Some(&third), Source::Raw, true, t, &mut reader);
        assert_eq!(
            reader.reads,
            [(first.clone(), Source::Raw), (third.clone(), Source::Raw)]
        );
        assert_eq!(
            histograms.shown(&first, Source::Raw),
            HistogramPanel::Reading
        );
        assert_eq!(
            histograms.shown(&third, Source::Raw),
            HistogramPanel::Reading
        );

        // its result is shown and not read again
        reader.answer(&third, Source::Raw, found(0.3));
        assert_eq!(
            histograms.request(Some(&third), Source::Raw, true, t, &mut reader),
            None
        );
        assert_eq!(histograms.shown(&third, Source::Raw), ready(0.3));
        histograms.request(Some(&third), Source::Raw, true, t, &mut reader);
        assert_eq!(reader.reads.len(), 2, "a read image is not read again");

        // a result of the same image in the other mode is another image's result
        histograms.request(Some(&second), Source::Jpeg, true, t, &mut reader);
        let t = t + DELAY;
        histograms.request(Some(&second), Source::Jpeg, true, t, &mut reader);
        assert_eq!(reader.reads.last(), Some(&(second.clone(), Source::Jpeg)));
        reader.answer(&second, Source::Raw, found(0.2));
        histograms.request(Some(&second), Source::Jpeg, true, t, &mut reader);
        assert_eq!(
            histograms.shown(&second, Source::Jpeg),
            HistogramPanel::Reading
        );
        assert_eq!(
            histograms.shown(&second, Source::Raw),
            HistogramPanel::Reading
        );
    }

    #[test]
    fn a_jpeg_or_tiff_file_is_read_as_itself_in_both_modes() {
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let tiff = image("pw-loupe-histogram-itself.tif");
        let start = Instant::now();
        histograms.request(Some(&tiff), Source::Raw, true, start, &mut reader);
        histograms.request(Some(&tiff), Source::Raw, true, start + DELAY, &mut reader);
        assert_eq!(reader.reads, [(tiff.clone(), Source::Jpeg)]);
        reader.answer(&tiff, Source::Jpeg, found(0.4));
        histograms.request(Some(&tiff), Source::Raw, true, start + DELAY, &mut reader);
        assert_eq!(histograms.shown(&tiff, Source::Raw), ready(0.4));
        assert_eq!(histograms.shown(&tiff, Source::Jpeg), ready(0.4));
        assert_eq!(
            histogram_source(Path::new("a.JPG"), Source::Raw),
            Source::Jpeg
        );
        assert_eq!(
            histogram_source(Path::new("a.ARW"), Source::Raw),
            Source::Raw
        );
        assert_eq!(
            histogram_source(Path::new("a.ARW"), Source::Jpeg),
            Source::Jpeg
        );
    }

    #[test]
    fn a_failed_read_and_a_stopped_reader_say_there_is_no_histogram() {
        let no_histogram = HistogramPanel::NoHistogram;
        assert_eq!(
            histogram_text(HistogramText::NoHistogram),
            "No histogram for this file"
        );
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let path = image("pw-loupe-histogram-failed.ARW");
        let t = read_now(&mut histograms, &mut reader, &path, Instant::now());
        // the real reader answers a file it cannot open with `Unavailable`
        reader.answer(&path, Source::Raw, ImageStatistics::Unavailable);
        histograms.request(Some(&path), Source::Raw, true, t, &mut reader);
        assert_eq!(histograms.shown(&path, Source::Raw), no_histogram);
        assert_eq!(reader.reads.len(), 1, "not read again");

        // the reader stops while a read is in flight
        let other = image("pw-loupe-histogram-stopped.ARW");
        let t = read_now(
            &mut histograms,
            &mut reader,
            &other,
            t + Duration::from_secs(1),
        );
        reader.stopped = true;
        histograms.request(Some(&other), Source::Raw, true, t, &mut reader);
        assert_eq!(histograms.shown(&other, Source::Raw), no_histogram);

        // a read asked of a stopped reader
        let last = image("pw-loupe-histogram-last.ARW");
        read_now(
            &mut histograms,
            &mut reader,
            &last,
            t + Duration::from_secs(1),
        );
        assert_eq!(histograms.shown(&last, Source::Raw), no_histogram);
    }

    #[test]
    fn a_result_of_the_preview_is_used_without_a_read() {
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let path = image("pw-loupe-histogram-preview.ARW");
        // the preview showed the image in RAW mode and had its statistics
        histograms.preview_result(&path, Source::Raw, &found(0.6));
        let start = Instant::now();
        assert_eq!(
            histograms.request(Some(&path), Source::Raw, true, start, &mut reader),
            None
        );
        let later = start + Duration::from_secs(1);
        histograms.request(Some(&path), Source::Raw, true, later, &mut reader);
        assert!(reader.reads.is_empty());
        assert_eq!(histograms.shown(&path, Source::Raw), ready(0.6));
        // in JPEG mode it is another image
        assert_eq!(
            histograms.shown(&path, Source::Jpeg),
            HistogramPanel::Reading
        );

        // a result the preview gives while the image waits for its read: no read either
        let other = image("pw-loupe-histogram-preview-later.ARW");
        histograms.request(Some(&other), Source::Raw, true, later, &mut reader);
        histograms.preview_result(&other, Source::Raw, &found(0.7));
        histograms.request(Some(&other), Source::Raw, true, later + DELAY, &mut reader);
        assert!(reader.reads.is_empty());
        assert_eq!(histograms.shown(&other, Source::Raw), ready(0.7));
    }

    #[test]
    fn the_last_64_results_are_kept_and_the_oldest_goes_first() {
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let paths: Vec<PathBuf> = (0..=KEPT_HISTOGRAMS)
            .map(|n| image(&format!("pw-loupe-histogram-kept-{n}.ARW")))
            .collect();
        let mut t = Instant::now();
        for (n, path) in paths.iter().enumerate() {
            t = read_now(&mut histograms, &mut reader, path, t);
            reader.answer(path, Source::Raw, found(n as f64));
            histograms.request(Some(path), Source::Raw, true, t, &mut reader);
        }
        assert_eq!(reader.reads.len(), KEPT_HISTOGRAMS + 1);
        // the oldest went; the others are shown at once and not read again
        let Some((oldest, kept)) = paths.split_first() else {
            panic!("no paths");
        };
        assert_eq!(
            histograms.shown(oldest, Source::Raw),
            HistogramPanel::Reading
        );
        for (n, path) in kept.iter().enumerate() {
            assert_eq!(histograms.shown(path, Source::Raw), ready((n + 1) as f64));
        }
        let Some(back) = kept.first() else {
            panic!("no kept path");
        };
        let t = t + Duration::from_secs(1);
        histograms.request(Some(back), Source::Raw, true, t, &mut reader);
        histograms.request(Some(back), Source::Raw, true, t + DELAY, &mut reader);
        assert_eq!(
            reader.reads.len(),
            KEPT_HISTOGRAMS + 1,
            "a kept result is not read"
        );
        // the oldest is read again
        histograms.request(Some(oldest), Source::Raw, true, t, &mut reader);
        histograms.request(Some(oldest), Source::Raw, true, t + DELAY, &mut reader);
        assert_eq!(reader.reads.last(), Some(&(oldest.clone(), Source::Raw)));
    }

    #[test]
    fn another_folder_empties_the_kept_results_and_the_same_folder_keeps_them() {
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let path = image("pw-loupe-histogram-folder.ARW");
        let t = read_now(&mut histograms, &mut reader, &path, Instant::now());
        reader.answer(&path, Source::Raw, found(0.5));
        histograms.request(Some(&path), Source::Raw, true, t, &mut reader);
        assert_eq!(histograms.shown(&path, Source::Raw), ready(0.5));

        // the same folder loaded again (contact sheet ↔ loupe) keeps them
        histograms.folder_loaded(std::slice::from_ref(&path));
        assert_eq!(histograms.shown(&path, Source::Raw), ready(0.5));

        // another folder empties them
        histograms.folder_loaded(&[image("pw-loupe-histogram-elsewhere.ARW")]);
        assert_eq!(
            histograms.shown(&path, Source::Raw),
            HistogramPanel::Reading
        );
    }

    #[test]
    fn a_kept_result_of_a_file_changed_since_is_read_again() {
        let folder =
            std::env::temp_dir().join(format!("pw-loupe-histogram-changed-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&folder).is_ok());
        let path = folder.join("changed.ARW");
        let other = folder.join("other.ARW");
        assert!(std::fs::write(&path, b"one").is_ok());

        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        let t = read_now(&mut histograms, &mut reader, &path, Instant::now());
        reader.answer(&path, Source::Raw, found(0.1));
        histograms.request(Some(&path), Source::Raw, true, t, &mut reader);
        // away and back, unchanged: not read again
        histograms.request(Some(&other), Source::Raw, false, t, &mut reader);
        histograms.request(Some(&path), Source::Raw, true, t, &mut reader);
        histograms.request(Some(&path), Source::Raw, true, t + DELAY, &mut reader);
        assert_eq!(reader.reads.len(), 1);

        // away, the file changes size, back: read again
        histograms.request(Some(&other), Source::Raw, false, t, &mut reader);
        assert!(std::fs::write(&path, b"one and more").is_ok());
        let t = t + Duration::from_secs(1);
        histograms.request(Some(&path), Source::Raw, true, t, &mut reader);
        histograms.request(Some(&path), Source::Raw, true, t + DELAY, &mut reader);
        assert_eq!(reader.reads.len(), 2);
        assert_eq!(
            histograms.shown(&path, Source::Raw),
            HistogramPanel::Reading
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A folder of its own for one test, with `name` written as a file of three bytes.
    fn written_file(test: &str, name: &str) -> (PathBuf, PathBuf) {
        let folder =
            std::env::temp_dir().join(format!("pw-loupe-histogram-{test}-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&folder).is_ok());
        let path = folder.join(name);
        assert!(std::fs::write(&path, b"one").is_ok());
        (folder, path)
    }

    /// Kept results of `path` in both display modes, as the preview gives them.
    fn kept_in_both_modes(histograms: &mut HistogramInspector, path: &Path) {
        histograms.preview_result(path, Source::Raw, &found(0.1));
        histograms.preview_result(path, Source::Jpeg, &found(0.2));
    }

    /// Moves to `path` in `mode` after another image and lets the delay pass; returns how many
    /// reads have been asked for.
    fn visit(
        histograms: &mut HistogramInspector,
        reader: &mut FakeReader,
        other: &Path,
        path: &Path,
        mode: Source,
        at: Instant,
    ) -> usize {
        histograms.request(Some(other), mode, false, at, reader);
        histograms.request(Some(path), mode, true, at, reader);
        histograms.request(Some(path), mode, true, at + DELAY, reader);
        reader.reads.len()
    }

    #[test]
    fn a_kept_result_follows_rawstrides_own_write_in_both_modes() {
        let (folder, path) = written_file("own-write", "own.DNG");
        let other = folder.join("other.DNG");
        let reads = HistogramReads::default();
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        kept_in_both_modes(&mut histograms, &path);

        // the stamp noted before the write, the file changed by it, the write answered
        reads.note_before_write(&path);
        assert!(std::fs::write(&path, b"one with metadata").is_ok());
        histograms.written(&path, true, &reads);

        let t = Instant::now();
        assert_eq!(
            visit(&mut histograms, &mut reader, &other, &path, Source::Raw, t),
            0
        );
        assert_eq!(histograms.shown(&path, Source::Raw), ready(0.1));
        assert_eq!(
            visit(&mut histograms, &mut reader, &other, &path, Source::Jpeg, t),
            0
        );
        assert_eq!(histograms.shown(&path, Source::Jpeg), ready(0.2));
        // the noted stamp is forgotten
        assert_eq!(reads.take_before_write(&path), None);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_file_changed_outside_rawstride_before_the_write_is_read_again() {
        let (folder, path) = written_file("outside", "outside.DNG");
        let other = folder.join("other.DNG");
        let reads = HistogramReads::default();
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        kept_in_both_modes(&mut histograms, &path);

        // changed by another program, then written by RawStride
        assert!(std::fs::write(&path, b"one, edited elsewhere").is_ok());
        reads.note_before_write(&path);
        assert!(std::fs::write(&path, b"one, edited elsewhere, with metadata").is_ok());
        histograms.written(&path, true, &reads);

        let t = Instant::now();
        assert_eq!(
            visit(&mut histograms, &mut reader, &other, &path, Source::Raw, t),
            1
        );
        // the read comes back, so the other mode's read can be sent
        reader.answer(&path, Source::Raw, found(0.3));
        assert_eq!(
            visit(&mut histograms, &mut reader, &other, &path, Source::Jpeg, t),
            2
        );
        assert_eq!(reads.take_before_write(&path), None);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_failed_write_changes_nothing_and_its_noted_stamp_is_forgotten() {
        let (folder, path) = written_file("failed", "failed.DNG");
        let other = folder.join("other.DNG");
        let reads = HistogramReads::default();
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        kept_in_both_modes(&mut histograms, &path);

        // a failed write of an unchanged file: still fresh
        reads.note_before_write(&path);
        histograms.written(&path, false, &reads);
        assert_eq!(reads.take_before_write(&path), None);
        let t = Instant::now();
        assert_eq!(
            visit(&mut histograms, &mut reader, &other, &path, Source::Raw, t),
            0
        );

        // a failed write while the file changed: the kept stamp is not moved on
        reads.note_before_write(&path);
        assert!(std::fs::write(&path, b"one and more").is_ok());
        histograms.written(&path, false, &reads);
        assert_eq!(reads.take_before_write(&path), None);
        assert_eq!(
            visit(&mut histograms, &mut reader, &other, &path, Source::Raw, t),
            1
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_write_of_a_path_with_no_kept_result_does_nothing() {
        let (folder, path) = written_file("not-kept", "not-kept.DNG");
        let kept = folder.join("kept.DNG");
        assert!(std::fs::write(&kept, b"kept").is_ok());
        let reads = HistogramReads::default();
        let mut reader = FakeReader::default();
        let mut histograms = inspector();
        kept_in_both_modes(&mut histograms, &kept);

        reads.note_before_write(&path);
        assert!(std::fs::write(&path, b"one with metadata").is_ok());
        histograms.written(&path, true, &reads);
        assert_eq!(reads.take_before_write(&path), None);
        // nothing is kept for the written file, the other's result is as it was
        assert_eq!(
            histograms.shown(&path, Source::Raw),
            HistogramPanel::Reading
        );
        assert_eq!(histograms.shown(&kept, Source::Raw), ready(0.1));
        let t = Instant::now();
        assert_eq!(
            visit(&mut histograms, &mut reader, &path, &kept, Source::Raw, t),
            0
        );
        // a write answered with nothing noted does nothing either
        histograms.written(&kept, true, &reads);
        assert_eq!(histograms.shown(&kept, Source::Raw), ready(0.1));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A backend that fails to open one file and has statistics for every other.
    struct Fake;

    impl Backend for Fake {
        type Session = ();
        fn open(&self, path: &Path, _: Source) -> Result<((), ImageSize), DecodeError> {
            if path.ends_with("broken.ARW") {
                return Err(DecodeError::damaged("broken"));
            }
            Ok((
                (),
                ImageSize {
                    w: 4,
                    h: 2,
                    flip: 0,
                },
            ))
        }
        fn render_half(&self, _: &mut ()) -> Result<Rgb, DecodeError> {
            Err(DecodeError::new("nothing is rendered"))
        }
        fn render_region(&self, _: &mut (), _: Rect) -> Result<Rgb, DecodeError> {
            Err(DecodeError::new("nothing is rendered"))
        }
        fn statistics(&self, _: &mut ()) -> Option<Statistics> {
            Some(statistics(StatisticsOrigin::RawData))
        }
    }

    #[test]
    fn the_reader_opens_reads_and_lets_go_and_a_file_it_cannot_open_has_no_histogram() {
        let reads = HistogramReads::default();
        let mut reader = HistogramReader::start(Fake, reads.clone());
        let good = image("pw-loupe-histogram-good.ARW");
        let broken = image("broken.ARW");
        assert_eq!(reader.read_histogram(good.clone(), Source::Raw), Ok(()));
        assert_eq!(reader.read_histogram(broken.clone(), Source::Jpeg), Ok(()));
        let mut answers = Vec::new();
        let end = Instant::now() + Duration::from_secs(10);
        while answers.len() < 2 && Instant::now() < end {
            match reader.try_recv() {
                Ok(read) => answers.push(read),
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        let expected = ImageStatistics::Available(Arc::new(statistics(StatisticsOrigin::RawData)));
        assert_eq!(
            answers
                .iter()
                .map(|read| (read.path.clone(), read.source, read.statistics.clone()))
                .collect::<Vec<_>>(),
            [
                (good.clone(), Source::Raw, expected),
                (broken.clone(), Source::Jpeg, ImageStatistics::Unavailable),
            ]
        );
        // nothing is held any more: a write waits for nothing
        assert!(reads.wait_for(&good, Duration::ZERO));
        assert!(reads.wait_for(&broken, Duration::ZERO));
    }

    #[test]
    fn a_write_waits_for_a_read_that_is_asked_for_and_not_over() {
        let reads = HistogramReads::default();
        let path = image("pw-loupe-histogram-held.ARW");
        reads.asked(&path);
        let t0 = Instant::now();
        assert!(!reads.wait_for(&path, Duration::from_millis(100)));
        assert!(t0.elapsed() >= Duration::from_millis(100));
        // another file does not wait
        assert!(reads.wait_for(&image("pw-loupe-histogram-free.ARW"), Duration::ZERO));
        let waiter = {
            let reads = reads.clone();
            let path = path.clone();
            std::thread::spawn(move || reads.wait_for(&path, Duration::from_secs(10)))
        };
        std::thread::sleep(Duration::from_millis(50));
        reads.finished(&path);
        assert!(matches!(waiter.join(), Ok(true)));
    }

    #[test]
    fn the_panel_reads_until_the_statistics_come_and_says_when_there_are_none() {
        assert_eq!(histogram_panel(None), HistogramPanel::Reading);
        assert_eq!(
            histogram_panel(Some(&ImageStatistics::Unavailable)),
            HistogramPanel::NoHistogram
        );
        let statistics = statistics(StatisticsOrigin::Tiff);
        let available = ImageStatistics::Available(Arc::new(statistics.clone()));
        assert_eq!(
            histogram_panel(Some(&available)),
            HistogramPanel::Ready(Box::new(histogram_view(&statistics)))
        );
    }
}
