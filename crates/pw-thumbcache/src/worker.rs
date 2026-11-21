use std::collections::{HashMap, HashSet};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use pw_colour::{Profile, TransformCache};
use pw_libjpeg::{Decoder, Rgb8};

use crate::{ThumbError, thumbnail};

const DEFAULT_WORKER_COUNT: usize = 2;
const DEFAULT_CACHE_LIMIT: usize = 64 * 1024 * 1024;
const VISIBLE_MARGIN: usize = 32;

/// A thumbnail source owned by one worker thread.
pub trait ThumbSource: Send {
    /// Builds a thumbnail from a path on its owning background thread. `orientation` is used
    /// instead of the file's own when given (ADR 0024 §2.4).
    fn make(
        &mut self,
        path: &Path,
        cell: u32,
        orientation: Option<u16>,
    ) -> Result<Rgb8, ThumbError>;
}

/// Thumbnails from files, converted to `destination` (ADR 0016 §4.1 "Thumbnails").
struct FileThumbSource {
    decoder: Result<Decoder, String>,
    destination: Profile,
    /// Shared by the worker threads, so each transform is built once.
    cache: Arc<TransformCache>,
}

impl FileThumbSource {
    fn new(destination: Profile, cache: Arc<TransformCache>) -> Self {
        Self {
            decoder: Decoder::new().map_err(|error| error.to_string()),
            destination,
            cache,
        }
    }
}

impl ThumbSource for FileThumbSource {
    fn make(
        &mut self,
        path: &Path,
        cell: u32,
        orientation: Option<u16>,
    ) -> Result<Rgb8, ThumbError> {
        match &mut self.decoder {
            Ok(decoder) => thumbnail(
                path,
                cell,
                orientation,
                decoder,
                &self.destination,
                &self.cache,
            ),
            Err(error) => Err(ThumbError::Unreadable(error.clone())),
        }
    }
}

/// One thumbnail job submitted by the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailRequest {
    /// The folder generation the request belongs to.
    pub generation: u64,
    /// The image's position in that folder.
    pub index: usize,
    /// The image path.
    pub path: PathBuf,
    /// The requested cell size in pixels.
    pub cell: u32,
    /// The orientation to use instead of the file's own (ADR 0024 §2.4: an XMP orientation before
    /// the embedded one); `None` uses the file's own. A request with another orientation is a job
    /// of its own.
    pub orientation: Option<u16>,
}

/// A completed thumbnail job, including failures for placeholder display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailEvent {
    /// The folder generation the result belongs to.
    pub generation: u64,
    /// The image's position in that folder.
    pub index: usize,
    /// The image path.
    pub path: PathBuf,
    /// The orientation the thumbnail was made with (the request's, also for a cache hit), so a
    /// late result made with an old value can be dropped.
    pub orientation: Option<u16>,
    /// The decoded thumbnail or its extraction error.
    pub result: Result<Rgb8, ThumbError>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct CacheKey {
    path: PathBuf,
    cell: u32,
    orientation: Option<u16>,
}

/// What two requests are compared by: generation, index, cell and orientation.
type RequestKey = (u64, usize, u32, Option<u16>);

fn request_key(request: &ThumbnailRequest) -> RequestKey {
    (
        request.generation,
        request.index,
        request.cell,
        request.orientation,
    )
}

struct CacheEntry {
    image: Rgb8,
    last_used: u64,
}

struct State {
    generation: u64,
    visible: Option<Range<usize>>,
    queue: Vec<QueuedRequest>,
    in_flight: HashSet<RequestKey>,
    next_sequence: u64,
    paused: bool,
    stopped: bool,
    cache: HashMap<CacheKey, CacheEntry>,
    cache_bytes: usize,
    cache_limit: usize,
    cache_clock: u64,
    /// Raised by `set_generation` and `cancel` only (the cache emptied), not by
    /// `set_generation_keeping_cache`: a result is cached only when the epoch it was queued in is
    /// still the current one.
    cache_epoch: u64,
}

struct QueuedRequest {
    request: ThumbnailRequest,
    sequence: u64,
    /// The cache epoch when the request was queued.
    epoch: u64,
}

type SourceFactory = Arc<dyn Fn() -> Box<dyn ThumbSource> + Send + Sync>;

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    results: Sender<ThumbnailEvent>,
    source_factory: SourceFactory,
}

/// A non-blocking interface to a small pool of thumbnail workers.
pub struct ThumbWorker {
    shared: Arc<Shared>,
    events: Receiver<ThumbnailEvent>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for ThumbWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThumbWorker")
            .field("worker_count", &self.threads.len())
            .finish_non_exhaustive()
    }
}

impl ThumbWorker {
    /// Starts the default two-thread pool with a 64 MiB thumbnail cache; thumbnails in sRGB.
    pub fn start() -> io::Result<Self> {
        Self::start_with_limits(DEFAULT_WORKER_COUNT, DEFAULT_CACHE_LIMIT)
    }

    /// Starts the default two-thread pool with a 64 MiB thumbnail cache; thumbnails converted to
    /// `profile`, the monitor profile (ADR 0016 §4.1).
    pub fn start_with_monitor_profile(profile: Profile) -> io::Result<Self> {
        Self::start_with_profile(DEFAULT_WORKER_COUNT, DEFAULT_CACHE_LIMIT, profile)
    }

    /// Starts a pool with a chosen thread count and in-memory cache limit; thumbnails in sRGB.
    pub fn start_with_limits(worker_count: usize, cache_limit: usize) -> io::Result<Self> {
        Self::start_with_profile(worker_count, cache_limit, Profile::srgb())
    }

    fn start_with_profile(
        worker_count: usize,
        cache_limit: usize,
        destination: Profile,
    ) -> io::Result<Self> {
        let cache = Arc::new(TransformCache::default());
        Self::with_source_factory(
            worker_count,
            cache_limit,
            Arc::new(move || {
                Box::new(FileThumbSource::new(
                    destination.clone(),
                    Arc::clone(&cache),
                ))
            }),
        )
    }

    fn with_source_factory(
        worker_count: usize,
        cache_limit: usize,
        source_factory: SourceFactory,
    ) -> io::Result<Self> {
        if worker_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "thumbnail worker count must be greater than zero",
            ));
        }

        let (result_sender, events) = mpsc::channel();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                generation: 0,
                visible: None,
                queue: Vec::new(),
                in_flight: HashSet::new(),
                next_sequence: 0,
                paused: false,
                stopped: false,
                cache: HashMap::new(),
                cache_bytes: 0,
                cache_limit,
                cache_clock: 0,
                cache_epoch: 0,
            }),
            wake: Condvar::new(),
            results: result_sender,
            source_factory,
        });

        let mut threads = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let worker_shared = Arc::clone(&shared);
            match std::thread::Builder::new()
                .name(format!("pw-thumbnail-{index}"))
                .spawn(move || run_worker(worker_shared))
            {
                Ok(thread) => threads.push(thread),
                Err(error) => {
                    let mut state = lock_state(&shared);
                    state.stopped = true;
                    shared.wake.notify_all();
                    drop(state);
                    drop(threads);
                    return Err(error);
                }
            }
        }

        Ok(Self {
            shared,
            events,
            threads,
        })
    }

    /// Queues a current-generation thumbnail request; cache hits are posted as results directly.
    pub fn request(&self, request: ThumbnailRequest) {
        let cached = {
            let mut state = lock_state(&self.shared);
            if state.stopped || request.generation != state.generation {
                return;
            }
            if is_far_from_visible(state.visible.as_ref(), request.index) {
                return;
            }
            let key_of_request = request_key(&request);
            if state.in_flight.contains(&key_of_request)
                || state
                    .queue
                    .iter()
                    .any(|queued| request_key(&queued.request) == key_of_request)
            {
                return;
            }

            let key = CacheKey {
                path: request.path.clone(),
                cell: request.cell,
                orientation: request.orientation,
            };
            if state.cache.contains_key(&key) {
                state.cache_clock = state.cache_clock.saturating_add(1);
                let last_used = state.cache_clock;
                let image = state.cache.get_mut(&key).map(|entry| {
                    entry.last_used = last_used;
                    entry.image.clone()
                });
                image.map(|image| ThumbnailEvent {
                    generation: request.generation,
                    index: request.index,
                    path: request.path.clone(),
                    orientation: request.orientation,
                    result: Ok(image),
                })
            } else {
                state.next_sequence = state.next_sequence.saturating_add(1);
                let sequence = state.next_sequence;
                let epoch = state.cache_epoch;
                state.queue.push(QueuedRequest {
                    request,
                    sequence,
                    epoch,
                });
                self.shared.wake.notify_one();
                None
            }
        };

        if let Some(event) = cached {
            let _ = self.shared.results.send(event);
        }
    }

    /// Sets the active folder generation and discards queued and cached work from earlier folders.
    pub fn set_generation(&self, generation: u64) {
        let mut state = lock_state(&self.shared);
        state.generation = generation;
        state.visible = None;
        state.queue.clear();
        state.cache.clear();
        state.cache_bytes = 0;
        state.cache_epoch = state.cache_epoch.wrapping_add(1);
        self.shared.wake.notify_all();
    }

    /// Sets the active generation as `set_generation` does (the generation set, the visible range
    /// cleared, the queue cleared, the workers woken), but keeps the thumbnail cache and its bytes
    /// (ADR 0026 §3.5): the cache is kept by path, cell size and orientation, not by place, so its
    /// entries stay true when the files of the active tab only move or more files come.
    ///
    /// A request of an older generation still being made when it comes back is not sent (its
    /// index is stale), but its thumbnail is cached when no `set_generation` or `cancel` came since
    /// it was queued, so a slow thumbnail is finished even while the generation changes often.
    pub fn set_generation_keeping_cache(&self, generation: u64) {
        let mut state = lock_state(&self.shared);
        state.generation = generation;
        state.visible = None;
        state.queue.clear();
        self.shared.wake.notify_all();
    }

    /// Cancels queued work and invalidates any in-flight results.
    ///
    /// The worker moves to a new internal generation, so requests are refused until the UI calls
    /// `set_generation` with the generation of the folder it shows next.
    pub fn cancel(&self) {
        let mut state = lock_state(&self.shared);
        state.generation = state.generation.wrapping_add(1);
        state.visible = None;
        state.queue.clear();
        state.cache.clear();
        state.cache_bytes = 0;
        state.cache_epoch = state.cache_epoch.wrapping_add(1);
        self.shared.wake.notify_all();
    }

    /// Forgets the cached thumbnails of one file (ADR 0026 §3.2), so its next request is made
    /// anew instead of being answered from the cache.
    ///
    /// Every cache entry whose path equals `path` (compared as a `PathBuf`, as the cache compares
    /// it), at any cell size and orientation, is removed and its bytes no longer count against
    /// the cache limit. Queued and in-flight requests are not touched: a result of one that comes
    /// in later is cached as usual. Takes only the state lock and does not wake the workers.
    pub fn forget_path(&self, path: &Path) {
        let mut state = lock_state(&self.shared);
        let mut freed = 0usize;
        state.cache.retain(|key, entry| {
            if key.path.as_path() == path {
                freed = freed.saturating_add(entry.image.data.len());
                false
            } else {
                true
            }
        });
        state.cache_bytes = state.cache_bytes.saturating_sub(freed);
    }

    /// Sets the visible index range; nearby requests are prefetched, distant ones are dropped.
    pub fn set_visible(&self, visible: Range<usize>) {
        let mut state = lock_state(&self.shared);
        state.visible = Some(visible);
        let active_range = state.visible.clone();
        state
            .queue
            .retain(|queued| !is_far_from_visible(active_range.as_ref(), queued.request.index));
        self.shared.wake.notify_all();
    }

    /// Pauses workers before they take another queued job.
    pub fn pause(&self) {
        lock_state(&self.shared).paused = true;
    }

    /// Resumes queued work.
    pub fn resume(&self) {
        let mut state = lock_state(&self.shared);
        state.paused = false;
        self.shared.wake.notify_all();
    }

    /// Returns the next non-stale result if one is available.
    pub fn try_recv(&self) -> Result<ThumbnailEvent, TryRecvError> {
        loop {
            let event = self.events.try_recv()?;
            if event.generation == lock_state(&self.shared).generation {
                return Ok(event);
            }
        }
    }
}

impl Drop for ThumbWorker {
    fn drop(&mut self) {
        let mut state = lock_state(&self.shared);
        state.stopped = true;
        state.queue.clear();
        self.shared.wake.notify_all();
        drop(state);
        // Dropping JoinHandles detaches workers instead of waiting on file I/O or decoding.
        self.threads.clear();
    }
}

fn lock_state(shared: &Shared) -> MutexGuard<'_, State> {
    shared
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn is_far_from_visible(visible: Option<&Range<usize>>, index: usize) -> bool {
    let Some(visible) = visible else {
        return false;
    };
    distance_to_visible(visible, index) > VISIBLE_MARGIN
}

fn distance_to_visible(visible: &Range<usize>, index: usize) -> usize {
    if index < visible.start {
        visible.start - index
    } else if index >= visible.end {
        index.saturating_sub(visible.end)
    } else {
        0
    }
}

fn run_worker(shared: Arc<Shared>) {
    let mut source = (shared.source_factory)();
    loop {
        let (request, epoch) = {
            let mut state = lock_state(&shared);
            loop {
                if state.stopped {
                    return;
                }
                let selected = if state.paused {
                    None
                } else {
                    state.queue.iter().enumerate().min_by_key(|(_, queued)| {
                        let visible = state.visible.as_ref();
                        (
                            !visible.is_some_and(|range| {
                                queued.request.index >= range.start
                                    && queued.request.index < range.end
                            }),
                            visible.map_or(0, |range| {
                                distance_to_visible(range, queued.request.index)
                            }),
                            queued.sequence,
                        )
                    })
                };
                if let Some((queue_index, _)) = selected {
                    let queued = state.queue.remove(queue_index);
                    if queued.request.generation == state.generation
                        && !is_far_from_visible(state.visible.as_ref(), queued.request.index)
                    {
                        state.in_flight.insert(request_key(&queued.request));
                        break (queued.request, queued.epoch);
                    }
                    continue;
                }
                state = shared
                    .wake
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner());
            }
        };

        let result = source.make(&request.path, request.cell, request.orientation);
        let mut state = lock_state(&shared);
        state.in_flight.remove(&request_key(&request));
        if state.stopped {
            continue;
        }
        // Cached while the cache it was asked for is still the current one (also across
        // `set_generation_keeping_cache`, since the cache is kept by path); sent only to the
        // current generation, where its index is still true.
        if epoch == state.cache_epoch
            && let Ok(image) = &result
        {
            cache_insert(
                &mut state,
                CacheKey {
                    path: request.path.clone(),
                    cell: request.cell,
                    orientation: request.orientation,
                },
                image.clone(),
            );
        }
        if request.generation == state.generation
            && !is_far_from_visible(state.visible.as_ref(), request.index)
        {
            let _ = shared.results.send(ThumbnailEvent {
                generation: request.generation,
                index: request.index,
                path: request.path,
                orientation: request.orientation,
                result,
            });
        }
    }
}

fn cache_insert(state: &mut State, key: CacheKey, image: Rgb8) {
    let bytes = image.data.len();
    if bytes > state.cache_limit {
        return;
    }
    if let Some(previous) = state.cache.remove(&key) {
        state.cache_bytes = state.cache_bytes.saturating_sub(previous.image.data.len());
    }
    while state.cache_bytes.saturating_add(bytes) > state.cache_limit {
        let oldest = state
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone());
        let Some(oldest) = oldest else {
            break;
        };
        if let Some(entry) = state.cache.remove(&oldest) {
            state.cache_bytes = state.cache_bytes.saturating_sub(entry.image.data.len());
        }
    }

    state.cache_clock = state.cache_clock.saturating_add(1);
    let last_used = state.cache_clock;
    state.cache_bytes = state.cache_bytes.saturating_add(bytes);
    state.cache.insert(key, CacheEntry { image, last_used });
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use pw_libjpeg::Rgb8;

    use super::*;
    use crate::ThumbError;

    #[derive(Default)]
    struct FakeState {
        calls: Vec<PathBuf>,
        /// The orientation of each call, in the order of `calls`.
        orientations: Vec<Option<u16>>,
        failures: Vec<PathBuf>,
    }

    #[derive(Default)]
    struct GateState {
        started: bool,
        released: bool,
    }

    #[derive(Default)]
    struct Gate {
        state: Mutex<GateState>,
        changed: Condvar,
    }

    struct FakeSource {
        state: Arc<Mutex<FakeState>>,
        gate: Option<Arc<Gate>>,
    }

    impl ThumbSource for FakeSource {
        fn make(
            &mut self,
            path: &Path,
            _cell: u32,
            orientation: Option<u16>,
        ) -> Result<Rgb8, ThumbError> {
            if let Some(gate) = &self.gate {
                let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
                gate_state.started = true;
                gate.changed.notify_all();
                while !gate_state.released {
                    gate_state = gate
                        .changed
                        .wait(gate_state)
                        .unwrap_or_else(|error| error.into_inner());
                }
            }

            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.calls.push(path.to_path_buf());
            state.orientations.push(orientation);
            if state.failures.iter().any(|failure| failure == path) {
                return Err(ThumbError::Unreadable("simulated failure".to_owned()));
            }
            Ok(Rgb8 {
                width: 1,
                height: 1,
                data: vec![path.to_string_lossy().len() as u8; 3],
            })
        }
    }

    fn worker(threads: usize, cache_limit: usize) -> (ThumbWorker, Arc<Mutex<FakeState>>) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let source_state = Arc::clone(&state);
        let factory: Arc<dyn Fn() -> Box<dyn ThumbSource> + Send + Sync> = Arc::new(move || {
            Box::new(FakeSource {
                state: Arc::clone(&source_state),
                gate: None,
            })
        });
        let worker = ThumbWorker::with_source_factory(threads, cache_limit, factory)
            .unwrap_or_else(|error| panic!("start test worker: {error}"));
        (worker, state)
    }

    fn gated_worker() -> (ThumbWorker, Arc<Mutex<FakeState>>, Arc<Gate>) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let gate = Arc::new(Gate::default());
        let source_state = Arc::clone(&state);
        let source_gate = Arc::clone(&gate);
        let factory: Arc<dyn Fn() -> Box<dyn ThumbSource> + Send + Sync> = Arc::new(move || {
            Box::new(FakeSource {
                state: Arc::clone(&source_state),
                gate: Some(Arc::clone(&source_gate)),
            })
        });
        let worker = ThumbWorker::with_source_factory(1, 1024, factory)
            .unwrap_or_else(|error| panic!("start gated test worker: {error}"));
        (worker, state, gate)
    }

    fn request(generation: u64, index: usize, path: &str) -> ThumbnailRequest {
        ThumbnailRequest {
            generation,
            index,
            path: PathBuf::from(path),
            cell: 200,
            orientation: None,
        }
    }

    fn oriented(index: usize, path: &str, orientation: Option<u16>) -> ThumbnailRequest {
        ThumbnailRequest {
            orientation,
            ..request(0, index, path)
        }
    }

    fn orientations(state: &Mutex<FakeState>) -> Vec<Option<u16>> {
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .orientations
            .clone()
    }

    fn wait_until_started(gate: &Gate) {
        let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
        while !gate_state.started {
            gate_state = gate
                .changed
                .wait(gate_state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    fn open_gate(gate: &Gate) {
        let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
        gate_state.released = true;
        gate.changed.notify_all();
    }

    fn next_event(worker: &ThumbWorker) -> ThumbnailEvent {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match worker.try_recv() {
                Ok(event) => return event,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    panic!("thumbnail worker disconnected")
                }
                Err(std::sync::mpsc::TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    panic!("timed out waiting for thumbnail")
                }
            }
        }
    }

    fn calls(state: &Mutex<FakeState>) -> Vec<PathBuf> {
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .calls
            .clone()
    }

    #[test]
    fn every_request_gets_exactly_one_result() {
        let (worker, _) = worker(2, 1024);
        worker.request(request(0, 0, "first"));
        worker.request(request(0, 1, "second"));

        let first = next_event(&worker);
        let second = next_event(&worker);
        let results = HashMap::from([
            (first.index, first.result.is_ok()),
            (second.index, second.result.is_ok()),
        ]);
        assert_eq!(results, HashMap::from([(0, true), (1, true)]));
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn duplicate_queued_request_gets_one_source_call_and_one_result() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 0, "same"));
        worker.request(request(0, 0, "same"));
        worker.resume();

        assert_eq!(next_event(&worker).index, 0);
        assert_eq!(calls(&state), vec![PathBuf::from("same")]);
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn duplicate_in_flight_request_gets_one_source_call_and_one_result() {
        let (worker, state, gate) = gated_worker();
        worker.request(request(0, 0, "same"));
        {
            let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
            while !gate_state.started {
                gate_state = gate
                    .changed
                    .wait(gate_state)
                    .unwrap_or_else(|error| error.into_inner());
            }
        }
        worker.request(request(0, 0, "same"));
        {
            let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
            gate_state.released = true;
            gate.changed.notify_all();
        }

        assert_eq!(next_event(&worker).index, 0);
        assert_eq!(calls(&state), vec![PathBuf::from("same")]);
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn same_index_with_different_cell_sizes_is_not_a_duplicate() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 0, "same"));
        let mut larger = request(0, 0, "same");
        larger.cell = 256;
        worker.request(larger);
        worker.resume();

        assert_eq!(next_event(&worker).index, 0);
        assert_eq!(next_event(&worker).index, 0);
        assert_eq!(
            calls(&state),
            vec![PathBuf::from("same"), PathBuf::from("same")]
        );
    }

    #[test]
    fn completed_request_is_answered_again_from_the_cache() {
        let (worker, state) = worker(1, 1024);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);

        assert_eq!(calls(&state), vec![PathBuf::from("same")]);
    }

    #[test]
    fn visible_work_precedes_nearby_work_and_far_work_is_dropped() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 10, "near"));
        worker.request(request(0, 2, "visible"));
        worker.request(request(0, 1000, "far"));
        worker.set_visible(2..3);
        worker.resume();

        assert_eq!(next_event(&worker).index, 2);
        assert_eq!(next_event(&worker).index, 10);
        assert_eq!(
            calls(&state),
            vec![PathBuf::from("visible"), PathBuf::from("near")]
        );
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn queued_results_from_an_old_generation_are_discarded() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 0, "old"));
        worker.set_generation(1);
        worker.request(request(1, 1, "new"));
        worker.resume();

        let event = next_event(&worker);
        assert_eq!(event.generation, 1);
        assert_eq!(event.index, 1);
        assert_eq!(calls(&state), vec![PathBuf::from("new")]);
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn in_flight_results_from_an_old_generation_are_discarded() {
        let (worker, state, gate) = gated_worker();
        worker.request(request(0, 0, "old"));
        {
            let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
            while !gate_state.started {
                gate_state = gate
                    .changed
                    .wait(gate_state)
                    .unwrap_or_else(|error| error.into_inner());
            }
        }
        worker.set_generation(1);
        {
            let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
            gate_state.released = true;
            gate.changed.notify_all();
        }

        let deadline = Instant::now() + Duration::from_secs(3);
        while calls(&state).is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(calls(&state), vec![PathBuf::from("old")]);
        std::thread::sleep(Duration::from_millis(20));
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn dropping_the_worker_does_not_wait_for_an_in_flight_decode() {
        let (worker, _, gate) = gated_worker();
        worker.request(request(0, 0, "blocked"));
        {
            let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
            while !gate_state.started {
                gate_state = gate
                    .changed
                    .wait(gate_state)
                    .unwrap_or_else(|error| error.into_inner());
            }
        }

        let (dropped_sender, dropped_receiver) = std::sync::mpsc::channel();
        let drop_thread = std::thread::spawn(move || {
            drop(worker);
            let _ = dropped_sender.send(());
        });
        assert!(
            dropped_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_ok()
        );
        {
            let mut gate_state = gate.state.lock().unwrap_or_else(|error| error.into_inner());
            gate_state.released = true;
            gate.changed.notify_all();
        }
        assert!(drop_thread.join().is_ok());
    }

    #[test]
    fn pause_stops_work_until_resumed() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 0, "paused"));
        std::thread::sleep(Duration::from_millis(40));
        assert!(calls(&state).is_empty());
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        worker.resume();
        assert_eq!(next_event(&worker).index, 0);
        assert_eq!(calls(&state), vec![PathBuf::from("paused")]);
    }

    #[test]
    fn cache_hit_returns_without_calling_the_source_again() {
        let (worker, state) = worker(1, 1024);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);
        worker.request(request(0, 1, "same"));
        let cached = next_event(&worker);

        assert_eq!(cached.index, 1);
        assert_eq!(calls(&state), vec![PathBuf::from("same")]);
    }

    #[test]
    fn changing_generation_clears_the_thumbnail_cache() {
        let (worker, state) = worker(1, 1024);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);
        worker.set_generation(1);
        worker.request(request(1, 1, "same"));
        assert_eq!(next_event(&worker).index, 1);

        assert_eq!(
            calls(&state),
            vec![PathBuf::from("same"), PathBuf::from("same")]
        );
    }

    #[test]
    fn cache_evicts_the_least_recently_used_thumbnail_to_respect_its_limit() {
        let (worker, state) = worker(1, 6);
        worker.request(request(0, 0, "first"));
        let _ = next_event(&worker);
        worker.request(request(0, 1, "second"));
        let _ = next_event(&worker);
        worker.request(request(0, 2, "first"));
        let _ = next_event(&worker);
        worker.request(request(0, 3, "third"));
        let _ = next_event(&worker);
        worker.request(request(0, 4, "second"));
        let _ = next_event(&worker);

        assert_eq!(
            calls(&state),
            vec![
                PathBuf::from("first"),
                PathBuf::from("second"),
                PathBuf::from("third"),
                PathBuf::from("second"),
            ]
        );
    }

    #[test]
    fn source_errors_are_delivered_as_results() {
        let (worker, state) = worker(1, 1024);
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .failures
            .push(PathBuf::from("broken"));
        worker.request(request(0, 0, "broken"));

        let event = next_event(&worker);
        assert_eq!(
            event.result,
            Err(ThumbError::Unreadable("simulated failure".to_owned()))
        );
    }

    #[test]
    fn cancelling_invalidates_in_flight_results() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 0, "cancelled"));
        worker.cancel();
        worker.resume();
        std::thread::sleep(Duration::from_millis(40));

        assert!(calls(&state).is_empty());
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    // ---------- an orientation per request (ADR 0024 §2.4, T231) ----------

    #[test]
    fn a_queued_request_with_another_orientation_is_a_job_of_its_own() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(oriented(0, "same", None));
        worker.request(oriented(0, "same", Some(6)));
        worker.resume();

        let first = next_event(&worker);
        let second = next_event(&worker);
        assert_eq!((first.index, second.index), (0, 0));
        assert_eq!(first.orientation, None);
        assert_eq!(second.orientation, Some(6));
        assert_eq!(orientations(&state), vec![None, Some(6)]);
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn an_in_flight_request_with_another_orientation_is_a_job_of_its_own() {
        let (worker, state, gate) = gated_worker();
        worker.request(oriented(0, "same", None));
        wait_until_started(&gate);
        worker.request(oriented(0, "same", Some(8)));
        open_gate(&gate);

        let first = next_event(&worker);
        let second = next_event(&worker);
        assert_eq!(first.orientation, None);
        assert_eq!(second.orientation, Some(8));
        assert_eq!(orientations(&state), vec![None, Some(8)]);
    }

    #[test]
    fn a_cached_thumbnail_is_not_given_out_for_another_orientation() {
        let (worker, state) = worker(1, 1024);
        worker.request(oriented(0, "same", Some(6)));
        assert_eq!(next_event(&worker).orientation, Some(6));
        worker.request(oriented(0, "same", None));
        let other = next_event(&worker);
        assert_eq!(other.orientation, None);
        assert_eq!(orientations(&state), vec![Some(6), None]);
    }

    #[test]
    fn a_cache_hit_carries_the_orientation_it_was_made_with() {
        let (worker, state) = worker(1, 1024);
        worker.request(oriented(0, "same", Some(3)));
        assert_eq!(next_event(&worker).orientation, Some(3));
        worker.request(oriented(1, "same", Some(3)));
        let hit = next_event(&worker);
        assert_eq!(hit.index, 1);
        assert_eq!(hit.orientation, Some(3));
        assert_eq!(
            orientations(&state),
            vec![Some(3)],
            "answered from the cache"
        );
    }

    // ---------- forgetting one file's thumbnails (ADR 0026 §3.2, T253) ----------

    fn cell(index: usize, path: &str, cell: u32) -> ThumbnailRequest {
        ThumbnailRequest {
            cell,
            ..request(0, index, path)
        }
    }

    #[test]
    fn a_forgotten_thumbnail_is_made_anew() {
        let (worker, state) = worker(1, 1024);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);
        worker.forget_path(Path::new("same"));
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);

        assert_eq!(
            calls(&state),
            vec![PathBuf::from("same"), PathBuf::from("same")]
        );
    }

    #[test]
    fn forget_path_drops_every_cell_size_and_orientation_of_that_path_only() {
        let (worker, state) = worker(1, 1024);
        let requests = [
            cell(0, "same", 200),
            cell(0, "same", 256),
            oriented(0, "same", Some(6)),
        ];
        for request in &requests {
            worker.request(request.clone());
            let _ = next_event(&worker);
        }
        worker.request(request(0, 1, "other"));
        let _ = next_event(&worker);
        assert_eq!(calls(&state).len(), 4);

        worker.forget_path(Path::new("same"));
        for request in &requests {
            worker.request(request.clone());
            let _ = next_event(&worker);
        }
        worker.request(request(0, 1, "other"));
        assert_eq!(next_event(&worker).index, 1);

        assert_eq!(
            calls(&state),
            vec![
                PathBuf::from("same"),
                PathBuf::from("same"),
                PathBuf::from("same"),
                PathBuf::from("other"),
                PathBuf::from("same"),
                PathBuf::from("same"),
                PathBuf::from("same"),
            ],
            "every entry of \"same\" made anew, \"other\" answered from the cache"
        );
        assert_eq!(
            orientations(&state)[4..],
            [None, None, Some(6)],
            "made anew at each cell size and orientation"
        );
    }

    #[test]
    fn forget_path_frees_the_bytes_of_the_forgotten_thumbnails() {
        // The fake thumbnails are 3 bytes, so a limit of 6 holds two.
        let (worker, state) = worker(1, 6);
        worker.request(request(0, 0, "first"));
        let _ = next_event(&worker);
        worker.request(request(0, 1, "second"));
        let _ = next_event(&worker);
        worker.forget_path(Path::new("first"));
        worker.request(request(0, 2, "third"));
        let _ = next_event(&worker);
        worker.request(request(0, 1, "second"));
        assert_eq!(next_event(&worker).index, 1);

        assert_eq!(
            calls(&state),
            vec![
                PathBuf::from("first"),
                PathBuf::from("second"),
                PathBuf::from("third"),
            ],
            "\"second\" still answered from the cache: nothing was evicted for \"third\""
        );
    }

    #[test]
    fn forget_path_of_a_path_not_in_the_cache_changes_nothing() {
        let (worker, state) = worker(1, 1024);
        worker.request(request(0, 0, "same"));
        let _ = next_event(&worker);
        let before = {
            let state = lock_state(&worker.shared);
            (state.cache.len(), state.cache_bytes)
        };

        worker.forget_path(Path::new("other"));

        let after = {
            let state = lock_state(&worker.shared);
            (state.cache.len(), state.cache_bytes)
        };
        assert_eq!(after, before);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);
        assert_eq!(calls(&state), vec![PathBuf::from("same")]);
    }

    // ---------- a generation change that keeps the cache (ADR 0026 §3.5, T256) ----------

    /// Waits until the one request being made has come back and been handled by the worker.
    fn wait_until_nothing_in_flight(worker: &ThumbWorker, state: &Mutex<FakeState>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while (calls(state).is_empty() || !lock_state(&worker.shared).in_flight.is_empty())
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            lock_state(&worker.shared).in_flight.is_empty(),
            "the request came back in time"
        );
    }

    #[test]
    fn a_generation_change_keeping_the_cache_answers_from_the_cache() {
        let (worker, state) = worker(1, 1024);
        worker.request(request(0, 0, "same"));
        assert_eq!(next_event(&worker).index, 0);

        worker.set_generation_keeping_cache(1);
        worker.request(request(1, 5, "same"));
        let hit = next_event(&worker);

        assert_eq!((hit.generation, hit.index), (1, 5));
        assert_eq!(hit.path, PathBuf::from("same"));
        assert!(hit.result.is_ok());
        assert_eq!(calls(&state), vec![PathBuf::from("same")], "from the cache");
    }

    #[test]
    fn a_generation_change_keeping_the_cache_drops_queued_requests_of_the_old_one() {
        let (worker, state) = worker(1, 1024);
        worker.pause();
        worker.request(request(0, 0, "old"));
        worker.set_generation_keeping_cache(1);
        worker.request(request(1, 1, "new"));
        worker.resume();

        let event = next_event(&worker);
        assert_eq!((event.generation, event.index), (1, 1));
        assert_eq!(calls(&state), vec![PathBuf::from("new")]);
        assert!(matches!(
            worker.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn an_old_result_after_a_generation_change_keeping_the_cache_is_cached_not_sent() {
        let (worker, state, gate) = gated_worker();
        worker.request(request(0, 0, "same"));
        wait_until_started(&gate);
        worker.set_generation_keeping_cache(1);
        open_gate(&gate);
        wait_until_nothing_in_flight(&worker, &state);

        assert!(
            matches!(worker.events.try_recv(), Err(TryRecvError::Empty)),
            "the old generation's event is not sent"
        );
        worker.request(request(1, 3, "same"));
        let hit = next_event(&worker);
        assert_eq!((hit.generation, hit.index), (1, 3));
        assert_eq!(
            calls(&state),
            vec![PathBuf::from("same")],
            "the source called once in all"
        );
    }

    #[test]
    fn an_old_result_after_set_generation_is_neither_cached_nor_sent() {
        let (worker, state, gate) = gated_worker();
        worker.request(request(0, 0, "same"));
        wait_until_started(&gate);
        worker.set_generation(1);
        open_gate(&gate);
        wait_until_nothing_in_flight(&worker, &state);

        assert!(
            matches!(worker.events.try_recv(), Err(TryRecvError::Empty)),
            "the old generation's event is not sent"
        );
        worker.request(request(1, 3, "same"));
        let event = next_event(&worker);
        assert_eq!((event.generation, event.index), (1, 3));
        assert_eq!(
            calls(&state),
            vec![PathBuf::from("same"), PathBuf::from("same")],
            "made anew: the old result was not cached"
        );
    }

    #[test]
    fn an_old_result_after_cancel_is_neither_cached_nor_sent() {
        let (worker, state, gate) = gated_worker();
        worker.request(request(0, 0, "same"));
        wait_until_started(&gate);
        worker.cancel();
        worker.set_generation_keeping_cache(2);
        open_gate(&gate);
        wait_until_nothing_in_flight(&worker, &state);

        assert!(
            matches!(worker.events.try_recv(), Err(TryRecvError::Empty)),
            "the old generation's event is not sent"
        );
        worker.request(request(2, 3, "same"));
        let event = next_event(&worker);
        assert_eq!((event.generation, event.index), (2, 3));
        assert_eq!(
            calls(&state),
            vec![PathBuf::from("same"), PathBuf::from("same")],
            "made anew: a cancel since the request empties the cache for it"
        );
    }
}
