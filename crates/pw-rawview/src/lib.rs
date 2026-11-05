//! RAW rendering jobs for the viewer (ADR 0006): priorities, cancellation and a hard memory
//! budget. Independent of Tauri and of LibRaw: the decoder is a [`Backend`] (the real one is
//! [`libraw::LibRawBackend`] behind the `libraw` feature; the tests use a simulated one).
//!
//! # Jobs and priorities
//!
//! | Priority | Job | Worker |
//! |---|---|---|
//! | 1 | **Tile** of the current image, in the order last requested | interactive |
//! | 2 | **Unpack** of the current image (kept for tiles): after `dwell` with `unpack_ahead`, or at
//! once after [`Engine::prepare`] | interactive |
//! | 3 | **Half size** of the current image | background |
//! | 4 | **Half size** of the neighbours | background |
//!
//! - Two worker threads. The interactive worker only does tiles and the current unpack, so a
//!   zoom never waits for a half-size render. The background worker starts no new job while a
//!   tile is waiting.
//! - Unpack and half-size jobs start only after the image has been current for `dwell` (150 ms),
//!   so flicking through images starts no RAW work at all. Tiles start at once.
//! - **Cancellation:** changing the current image removes every queued job for images that are no
//!   longer wanted. A LibRaw call that is already running cannot be interrupted; it is allowed to
//!   finish and its result is thrown away if the image is no longer wanted.
//!
//! # Memory budget
//!
//! - **Unpacked RAW sessions** kept for tiles: at most `max_sessions` (2). The least recently used
//!   is closed before a new one is opened, so there are never more than `max_sessions` kept
//!   sessions plus the one temporary session of the background worker.
//! - **Encoded tiles:** at most `max_tile_bytes` (192 MB), least recently used first.
//! - **Encoded half-size images:** at most `max_halves` (6), least recently used first.
//! - With `reuse_half_session`, the background worker's session of the *current* image is kept
//!   for tiles after its half-size render (counted against `max_sessions`), so a later zoom does
//!   not unpack the file again. Sessions of neighbours are always closed.
//! - A folder change ([`Engine::set_folder`]) drops all of it. Sessions of images that are left
//!   stay until evicted (going back is common); their tiles go with them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(feature = "libraw")]
pub mod libraw;

/// A rectangle in sensor pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// An 8-bit RGB image in sensor orientation.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}x{})", self.width, self.height)
    }
}

/// The decoder used by the engine.
pub trait Backend: Send + Sync + 'static {
    /// An opened and unpacked RAW file.
    type Session: Send + 'static;
    fn open(&self, path: &Path) -> Result<Self::Session, String>;
    /// Visible size in sensor orientation.
    fn size(&self, s: &Self::Session) -> (u32, u32);
    fn render_half(&self, s: &mut Self::Session) -> Result<Image, String>;
    /// Exactly `r`, rendered with `margin` pixels of context that are cut off again.
    fn render_region(&self, s: &mut Self::Session, r: Rect, margin: u32) -> Result<Image, String>;
    /// Encodes for the viewer (JPEG).
    fn encode(&self, img: &Image) -> Result<Vec<u8>, String>;
}

/// Limits and timings (defaults from ADR 0006).
#[derive(Debug, Clone)]
pub struct Config {
    pub tile_size: u32,
    pub margin: u32,
    pub max_sessions: usize,
    pub max_tile_bytes: usize,
    pub max_halves: usize,
    pub dwell: Duration,
    /// Unpack the current image ahead of any zoom request (after `dwell`), so the first tile is
    /// quick. Only useful once tiles are shown; off = sessions are opened only by tile requests.
    pub unpack_ahead: bool,
    /// Keep the half-size job's session of the current image for tiles (ADR 0006 step 4).
    pub reuse_half_session: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            tile_size: 1024,
            margin: 16,
            max_sessions: 2,
            max_tile_bytes: 192 * 1024 * 1024,
            max_halves: 6,
            dwell: Duration::from_millis(150),
            unpack_ahead: true,
            reuse_half_session: true,
        }
    }
}

/// An image of the open folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub generation: u64,
    pub index: usize,
}

/// A tile in the grid of `tile_size` squares (column, row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    pub col: u32,
    pub row: u32,
}

/// An image the viewer wants RAW work for.
#[derive(Debug, Clone)]
pub struct Target {
    pub key: ImageKey,
    pub path: PathBuf,
    /// Whether a half-size render is wanted (the embedded preview is too small).
    pub want_half: bool,
}

/// The tile grid of an image (known once it has been unpacked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub tile: u32,
    pub cols: u32,
    pub rows: u32,
}

impl Grid {
    fn new(width: u32, height: u32, tile: u32) -> Grid {
        Grid {
            width,
            height,
            tile,
            cols: width.div_ceil(tile),
            rows: height.div_ceil(tile),
        }
    }

    /// The pixels of a tile (smaller at the right and bottom border), or None outside the grid.
    pub fn rect(&self, t: TileId) -> Option<Rect> {
        if t.col >= self.cols || t.row >= self.rows {
            return None;
        }
        let x = t.col * self.tile;
        let y = t.row * self.tile;
        Some(Rect {
            x,
            y,
            w: self.tile.min(self.width - x),
            h: self.tile.min(self.height - y),
        })
    }
}

/// Result of looking up a tile or half-size image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Ready(Arc<Vec<u8>>),
    /// Queued or running; not ready within the timeout.
    Pending,
    /// The image is no longer wanted (left, or folder changed): nothing will be produced.
    Gone,
    /// The decoder failed for this image; the viewer keeps the M1 preview.
    Failed(String),
}

/// Something became ready (for notifying the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Half(ImageKey),
    Tile(ImageKey, TileId),
    Grid(ImageKey, Grid),
    Failed(ImageKey, String),
}

/// Counters for tests and measurements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub sessions_kept: usize,
    pub tiles: usize,
    pub tile_bytes: usize,
    pub halves: usize,
    pub half_bytes: usize,
    pub queued: usize,
    /// Decoder calls running now.
    pub running: usize,
    /// Jobs removed from the queue because their image was left.
    pub jobs_dropped: u64,
    /// Results of finished jobs thrown away because their image was left.
    pub results_discarded: u64,
    pub sessions_opened: u64,
    pub sessions_evicted: u64,
    pub tiles_evicted: u64,
    pub halves_evicted: u64,
    /// Sessions of half-size jobs kept for tiles instead of being closed.
    pub sessions_reused: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Job {
    Tile(ImageKey, TileId),
    Unpack(ImageKey),
    Half(ImageKey),
}

type TileKey = (ImageKey, TileId);
type Encoded = Arc<Vec<u8>>;

enum Slot<S> {
    Idle(S),
    InUse,
}

struct State<B: Backend> {
    generation: u64,
    current: Option<ImageKey>,
    /// When the current image became current (dwell for unpack/half-size).
    current_since: Instant,
    targets: HashMap<ImageKey, Target>,
    /// Tile jobs of the current image, in request order.
    tile_queue: VecDeque<TileId>,
    /// Half-size jobs in priority order (current first).
    half_queue: VecDeque<ImageKey>,
    unpack_wanted: bool,
    /// The viewer asked to unpack the current image now ([`Engine::prepare`]).
    prepare: Option<ImageKey>,
    running: HashSet<Job>,
    sessions: Vec<(ImageKey, Slot<B::Session>)>, // most recently used last
    grids: HashMap<ImageKey, Grid>,
    tiles: VecDeque<(TileKey, Encoded)>, // LRU: most recent last
    halves: VecDeque<(ImageKey, Encoded)>,
    failed: HashMap<ImageKey, String>,
    stats: Stats,
    shutdown: bool,
}

type Listener = Box<dyn Fn(Event) + Send + Sync>;

struct Inner<B: Backend> {
    backend: B,
    config: Config,
    state: Mutex<State<B>>,
    /// Wakes workers (new jobs, current image changed, shutdown).
    work: Condvar,
    /// Wakes callers waiting for a result.
    done: Condvar,
    listener: Mutex<Option<Listener>>,
}

/// The RAW job engine. Dropping it stops the workers (after their current LibRaw call).
pub struct Engine<B: Backend> {
    inner: Arc<Inner<B>>,
    workers: Vec<JoinHandle<()>>,
}

impl<B: Backend> std::fmt::Debug for Engine<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("stats", &self.stats())
            .finish()
    }
}

impl<B: Backend> Inner<B> {
    fn lock(&self) -> MutexGuard<'_, State<B>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        let l = self.listener.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(f) = l.as_ref() {
            for e in events {
                f(e);
            }
        }
    }
}

impl<B: Backend> State<B> {
    fn tile_bytes(&self) -> usize {
        self.tiles.iter().map(|(_, b)| b.len()).sum()
    }

    fn wanted(&self, key: ImageKey) -> bool {
        key.generation == self.generation && self.targets.contains_key(&key)
    }

    fn is_current(&self, key: ImageKey) -> bool {
        self.current == Some(key) && key.generation == self.generation
    }

    fn touch_tile(&mut self, k: (ImageKey, TileId)) -> Option<Arc<Vec<u8>>> {
        let pos = self.tiles.iter().position(|(kk, _)| *kk == k)?;
        let e = self.tiles.remove(pos)?;
        let v = e.1.clone();
        self.tiles.push_back(e);
        Some(v)
    }

    fn touch_half(&mut self, k: ImageKey) -> Option<Arc<Vec<u8>>> {
        let pos = self.halves.iter().position(|(kk, _)| *kk == k)?;
        let e = self.halves.remove(pos)?;
        let v = e.1.clone();
        self.halves.push_back(e);
        Some(v)
    }

    fn insert_tile(&mut self, k: (ImageKey, TileId), bytes: Arc<Vec<u8>>, max: usize) {
        if bytes.len() > max || self.tiles.iter().any(|(kk, _)| *kk == k) {
            return;
        }
        let mut total = self.tile_bytes() + bytes.len();
        while total > max {
            match self.tiles.pop_front() {
                Some((_, b)) => {
                    total -= b.len();
                    self.stats.tiles_evicted += 1;
                }
                None => break,
            }
        }
        self.tiles.push_back((k, bytes));
    }

    fn insert_half(&mut self, k: ImageKey, bytes: Arc<Vec<u8>>, max: usize) {
        if max == 0 || self.halves.iter().any(|(kk, _)| *kk == k) {
            return;
        }
        while self.halves.len() >= max {
            self.halves.pop_front();
            self.stats.halves_evicted += 1;
        }
        self.halves.push_back((k, bytes));
    }

    /// Drops the tiles of an image (when its session is evicted or the folder changes).
    fn drop_tiles_of(&mut self, key: ImageKey) {
        self.tiles.retain(|((k, _), _)| *k != key);
    }

    fn refresh_stats(&mut self) {
        self.stats.sessions_kept = self.sessions.len();
        self.stats.tiles = self.tiles.len();
        self.stats.tile_bytes = self.tile_bytes();
        self.stats.halves = self.halves.len();
        self.stats.half_bytes = self.halves.iter().map(|(_, b)| b.len()).sum();
        self.stats.queued =
            self.tile_queue.len() + self.half_queue.len() + usize::from(self.unpack_wanted);
        self.stats.running = self.running.len();
    }
}

impl<B: Backend> Engine<B> {
    /// Starts the engine with its two worker threads.
    pub fn new(backend: B, config: Config) -> Engine<B> {
        let inner = Arc::new(Inner {
            backend,
            config,
            state: Mutex::new(State {
                generation: 0,
                current: None,
                current_since: Instant::now(),
                targets: HashMap::new(),
                tile_queue: VecDeque::new(),
                half_queue: VecDeque::new(),
                unpack_wanted: false,
                prepare: None,
                running: HashSet::new(),
                sessions: Vec::new(),
                grids: HashMap::new(),
                tiles: VecDeque::new(),
                halves: VecDeque::new(),
                failed: HashMap::new(),
                stats: Stats::default(),
                shutdown: false,
            }),
            work: Condvar::new(),
            done: Condvar::new(),
            listener: Mutex::new(None),
        });
        let workers = vec![
            {
                let i = inner.clone();
                std::thread::Builder::new()
                    .name("raw-interactive".into())
                    .spawn(move || interactive_worker(&i))
                    .expect("spawn worker")
            },
            {
                let i = inner.clone();
                std::thread::Builder::new()
                    .name("raw-background".into())
                    .spawn(move || background_worker(&i))
                    .expect("spawn worker")
            },
        ];
        Engine { inner, workers }
    }

    /// Called for every event (outside the engine's lock).
    pub fn set_listener(&self, f: impl Fn(Event) + Send + Sync + 'static) {
        *self
            .inner
            .listener
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    /// A new folder was opened: everything of the old one is dropped (queue, results, sessions).
    pub fn set_folder(&self, generation: u64) {
        let mut s = self.inner.lock();
        if s.generation == generation {
            return;
        }
        s.generation = generation;
        s.current = None;
        s.targets.clear();
        let dropped = s.tile_queue.len() + s.half_queue.len() + usize::from(s.unpack_wanted);
        s.stats.jobs_dropped += dropped as u64;
        s.tile_queue.clear();
        s.half_queue.clear();
        s.unpack_wanted = false;
        // Kept sessions are closed now (outside the lock); one in use is dropped when returned.
        // A session in use is dropped by its worker when returned (generation mismatch).
        let old: Vec<_> = std::mem::take(&mut s.sessions)
            .into_iter()
            .filter_map(|(_, slot)| match slot {
                Slot::Idle(sess) => Some(sess),
                Slot::InUse => None,
            })
            .collect();
        s.grids.clear();
        s.tiles.clear();
        s.halves.clear();
        s.failed.clear();
        s.refresh_stats();
        drop(s);
        drop(old);
        self.inner.work.notify_all();
        self.inner.done.notify_all();
    }

    /// The image shown now, and its neighbours. Removes every queued job for other images.
    pub fn set_current(&self, current: Target, neighbours: &[Target]) {
        let mut s = self.inner.lock();
        if current.key.generation != s.generation {
            return;
        }
        let changed = s.current != Some(current.key);
        let mut targets = HashMap::new();
        for t in neighbours.iter().chain(std::iter::once(&current)) {
            if t.key.generation == s.generation {
                targets.insert(t.key, t.clone());
            }
        }
        s.targets = targets;
        if changed {
            s.current = Some(current.key);
            s.current_since = Instant::now();
            s.stats.jobs_dropped += s.tile_queue.len() as u64;
            s.tile_queue.clear();
            // Tiles of images that are not current are not kept (they would only be shown again
            // after a new zoom request, which re-renders from the kept session).
            let cur = current.key;
            let before = s.tiles.len();
            s.tiles.retain(|((k, _), _)| *k == cur);
            s.stats.tiles_evicted += (before - s.tiles.len()) as u64;
        }
        // Half-size jobs: current first, then neighbours in the given order.
        let mut hq = VecDeque::new();
        for t in std::iter::once(&current).chain(neighbours) {
            if t.want_half
                && t.key.generation == s.generation
                && !s.halves.iter().any(|(k, _)| *k == t.key)
                && !s.failed.contains_key(&t.key)
                && !s.running.contains(&Job::Half(t.key))
                && !hq.contains(&t.key)
            {
                hq.push_back(t.key);
            }
        }
        let removed = s.half_queue.iter().filter(|k| !hq.contains(k)).count();
        s.stats.jobs_dropped += removed as u64;
        s.half_queue = hq;
        let has_session = s.sessions.iter().any(|(k, _)| *k == current.key);
        let was = s.unpack_wanted;
        if changed {
            s.prepare = None;
        }
        s.unpack_wanted = (self.inner.config.unpack_ahead || s.prepare == Some(current.key))
            && !has_session
            && !s.failed.contains_key(&current.key)
            && !s.running.contains(&Job::Unpack(current.key));
        if was && changed && !s.unpack_wanted {
            s.stats.jobs_dropped += 1;
        }
        s.refresh_stats();
        drop(s);
        self.inner.work.notify_all();
        self.inner.done.notify_all();
    }

    /// No image is shown in a RAW-capable view any more (e.g. back to the contact sheet): every
    /// queued job is dropped and no new work starts. Results and kept sessions stay in the caches
    /// (within the budget) for when the user comes back.
    pub fn clear_current(&self) {
        let mut s = self.inner.lock();
        let dropped = s.tile_queue.len() + s.half_queue.len() + usize::from(s.unpack_wanted);
        s.stats.jobs_dropped += dropped as u64;
        s.tile_queue.clear();
        s.half_queue.clear();
        s.unpack_wanted = false;
        s.prepare = None;
        s.current = None;
        s.targets.clear();
        s.refresh_stats();
        drop(s);
        self.inner.work.notify_all();
        self.inner.done.notify_all();
    }

    /// The viewer will soon need tiles of the current image (first zoom action): unpack it now,
    /// without waiting for the dwell time. Does nothing if a session is kept or being opened.
    pub fn prepare(&self, key: ImageKey) {
        let mut s = self.inner.lock();
        if !s.is_current(key) || s.failed.contains_key(&key) {
            return;
        }
        s.prepare = Some(key);
        let has_session = s.sessions.iter().any(|(k, _)| *k == key);
        if !has_session && !s.running.contains(&Job::Unpack(key)) {
            s.unpack_wanted = true;
        }
        s.refresh_stats();
        drop(s);
        self.inner.work.notify_all();
    }

    /// The viewer needs these tiles of the current image, most important first. Replaces the
    /// previous tile request (a new viewport); tiles already rendered are not queued again.
    pub fn request_tiles(&self, key: ImageKey, tiles: &[TileId]) {
        let mut s = self.inner.lock();
        if !s.is_current(key) {
            return;
        }
        let mut q = VecDeque::new();
        for &t in tiles {
            if !q.contains(&t)
                && !s.tiles.iter().any(|((k, tt), _)| *k == key && *tt == t)
                && !s.running.contains(&Job::Tile(key, t))
            {
                q.push_back(t);
            }
        }
        let removed = s.tile_queue.iter().filter(|t| !q.contains(t)).count();
        s.stats.jobs_dropped += removed as u64;
        s.tile_queue = q;
        s.refresh_stats();
        drop(s);
        self.inner.work.notify_all();
    }

    /// A tile of the current image: from the cache, or queued first and waited for up to
    /// `timeout`.
    pub fn tile(&self, key: ImageKey, t: TileId, timeout: Duration) -> Lookup {
        let deadline = Instant::now() + timeout;
        let mut s = self.inner.lock();
        loop {
            if let Some(b) = s.touch_tile((key, t)) {
                return Lookup::Ready(b);
            }
            if let Some(e) = s.failed.get(&key) {
                return Lookup::Failed(e.clone());
            }
            if !s.is_current(key) || s.shutdown {
                return Lookup::Gone;
            }
            if let Some(g) = s.grids.get(&key)
                && g.rect(t).is_none()
            {
                return Lookup::Failed(format!("tile {t:?} is outside the image"));
            }
            if !s.running.contains(&Job::Tile(key, t)) && !s.tile_queue.contains(&t) {
                // Asked for directly: it is visible now, so it goes first.
                s.tile_queue.push_front(t);
                s.refresh_stats();
                self.inner.work.notify_all();
            }
            let now = Instant::now();
            if now >= deadline {
                return Lookup::Pending;
            }
            s = self
                .inner
                .done
                .wait_timeout(s, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// The half-size image of a wanted image, waiting up to `timeout`.
    pub fn half(&self, key: ImageKey, timeout: Duration) -> Lookup {
        let deadline = Instant::now() + timeout;
        let mut s = self.inner.lock();
        loop {
            if let Some(b) = s.touch_half(key) {
                return Lookup::Ready(b);
            }
            if let Some(e) = s.failed.get(&key) {
                return Lookup::Failed(e.clone());
            }
            if !s.wanted(key) || s.shutdown {
                return Lookup::Gone;
            }
            if !s.half_queue.contains(&key) && !s.running.contains(&Job::Half(key)) {
                let pos = usize::from(!s.is_current(key)).min(s.half_queue.len());
                s.half_queue.insert(pos, key);
                s.refresh_stats();
                self.inner.work.notify_all();
            }
            let now = Instant::now();
            if now >= deadline {
                return Lookup::Pending;
            }
            s = self
                .inner
                .done
                .wait_timeout(s, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// The tile grid of an image, once it has been unpacked.
    pub fn grid(&self, key: ImageKey) -> Option<Grid> {
        self.inner.lock().grids.get(&key).copied()
    }

    pub fn stats(&self) -> Stats {
        let mut s = self.inner.lock();
        s.refresh_stats();
        s.stats.clone()
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }
}

impl<B: Backend> Drop for Engine<B> {
    fn drop(&mut self) {
        {
            let mut s = self.inner.lock();
            s.shutdown = true;
        }
        self.inner.work.notify_all();
        self.inner.done.notify_all();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}

// ---------- workers ----------

/// Sleeps until new work is signalled, or for at most `d`. The guard is released while waiting.
fn wait_work<B: Backend>(inner: &Inner<B>, s: MutexGuard<'_, State<B>>, d: Option<Duration>) {
    let _guard = match d {
        Some(d) => inner
            .work
            .wait_timeout(s, d)
            .map(|r| r.0)
            .unwrap_or_else(|e| e.into_inner().0),
        None => inner.work.wait(s).unwrap_or_else(|e| e.into_inner()),
    };
}

/// What the interactive worker should do next.
enum Next {
    Tile(ImageKey, TileId),
    Unpack(ImageKey),
    Wait(Option<Duration>),
    Stop,
}

fn next_interactive<B: Backend>(inner: &Inner<B>, s: &mut State<B>) -> Next {
    if s.shutdown {
        return Next::Stop;
    }
    let Some(cur) = s.current else {
        return Next::Wait(None);
    };
    if let Some(t) = s.tile_queue.pop_front() {
        return Next::Tile(cur, t);
    }
    if s.unpack_wanted {
        let ready_at = if s.prepare == Some(cur) {
            s.current_since
        } else {
            s.current_since + inner.config.dwell
        };
        let now = Instant::now();
        if now < ready_at {
            return Next::Wait(Some(ready_at - now));
        }
        s.unpack_wanted = false;
        return Next::Unpack(cur);
    }
    Next::Wait(None)
}

/// Takes the kept session of `key` (marking it in use), or None.
fn take_session<B: Backend>(s: &mut State<B>, key: ImageKey) -> Option<B::Session> {
    let pos = s.sessions.iter().position(|(k, _)| *k == key)?;
    let (k, slot) = s.sessions.remove(pos);
    match slot {
        Slot::Idle(sess) => {
            s.sessions.push((k, Slot::InUse));
            Some(sess)
        }
        Slot::InUse => {
            s.sessions.insert(pos, (k, Slot::InUse));
            None
        }
    }
}

/// Makes room for one more kept session: closes least recently used idle sessions (returned so
/// they are dropped outside the lock) until fewer than `max` remain.
fn evict_sessions<B: Backend>(s: &mut State<B>, max: usize) -> Vec<B::Session> {
    let mut out = Vec::new();
    while s.sessions.len() >= max.max(1) {
        let Some(pos) = s
            .sessions
            .iter()
            .position(|(_, slot)| matches!(slot, Slot::Idle(_)))
        else {
            break;
        };
        let (k, slot) = s.sessions.remove(pos);
        s.drop_tiles_of(k);
        s.grids.remove(&k);
        s.stats.sessions_evicted += 1;
        if let Slot::Idle(sess) = slot {
            out.push(sess);
        }
    }
    out
}

/// Returns a session after use (or drops it if the folder changed meanwhile).
fn return_session<B: Backend>(
    s: &mut State<B>,
    key: ImageKey,
    sess: B::Session,
) -> Option<B::Session> {
    if let Some(pos) = s.sessions.iter().position(|(k, _)| *k == key) {
        s.sessions.remove(pos);
    }
    if key.generation != s.generation || s.shutdown {
        return Some(sess);
    }
    s.sessions.push((key, Slot::Idle(sess)));
    None
}

fn interactive_worker<B: Backend>(inner: &Inner<B>) {
    loop {
        let mut s = inner.lock();
        let next = next_interactive(inner, &mut s);
        match next {
            Next::Stop => return,
            Next::Wait(d) => {
                wait_work(inner, s, d);
            }
            Next::Unpack(key) => {
                if s.sessions.iter().any(|(k, _)| *k == key) || s.failed.contains_key(&key) {
                    continue;
                }
                let Some(path) = s.targets.get(&key).map(|t| t.path.clone()) else {
                    continue;
                };
                s.running.insert(Job::Unpack(key));
                let evicted = evict_sessions(&mut s, inner.config.max_sessions);
                // Reserve the slot so the session count never exceeds the budget.
                s.sessions.push((key, Slot::InUse));
                s.stats.sessions_opened += 1;
                drop(s);
                drop(evicted);
                let result = inner.backend.open(&path);
                let mut s = inner.lock();
                s.running.remove(&Job::Unpack(key));
                let mut events = Vec::new();
                let mut to_drop = None;
                match result {
                    Ok(sess) => {
                        let (w, h) = inner.backend.size(&sess);
                        if key.generation == s.generation {
                            let g = Grid::new(w, h, inner.config.tile_size);
                            s.grids.insert(key, g);
                            events.push(Event::Grid(key, g));
                        }
                        to_drop = return_session(&mut s, key, sess);
                    }
                    Err(e) => {
                        s.sessions.retain(|(k, _)| *k != key);
                        if key.generation == s.generation {
                            s.failed.insert(key, e.clone());
                            events.push(Event::Failed(key, e));
                        }
                    }
                }
                s.refresh_stats();
                drop(s);
                drop(to_drop);
                inner.done.notify_all();
                inner.emit(events);
            }
            Next::Tile(key, t) => {
                if s.tiles.iter().any(|((k, tt), _)| *k == key && *tt == t) {
                    continue;
                }
                if s.failed.contains_key(&key) {
                    inner.done.notify_all();
                    continue;
                }
                let Some(path) = s.targets.get(&key).map(|tg| tg.path.clone()) else {
                    continue;
                };
                s.running.insert(Job::Tile(key, t));
                // Use the kept session, or open one (the unpack job may not have run yet).
                let (sess, evicted) = match take_session(&mut s, key) {
                    Some(sess) => (Some(sess), Vec::new()),
                    None => {
                        let ev = evict_sessions(&mut s, inner.config.max_sessions);
                        s.sessions.push((key, Slot::InUse));
                        s.stats.sessions_opened += 1;
                        s.unpack_wanted = false;
                        (None, ev)
                    }
                };
                drop(s);
                drop(evicted);
                let mut events = Vec::new();
                let sess = match sess {
                    Some(sess) => Ok(sess),
                    None => inner.backend.open(&path).inspect(|sess| {
                        let (w, h) = inner.backend.size(sess);
                        let g = Grid::new(w, h, inner.config.tile_size);
                        events.push(Event::Grid(key, g));
                    }),
                };
                let mut sess = match sess {
                    Ok(sess) => sess,
                    Err(e) => {
                        let mut s = inner.lock();
                        s.running.remove(&Job::Tile(key, t));
                        s.sessions.retain(|(k, _)| *k != key);
                        if key.generation == s.generation {
                            s.failed.insert(key, e.clone());
                        }
                        s.refresh_stats();
                        drop(s);
                        inner.done.notify_all();
                        inner.emit(vec![Event::Failed(key, e)]);
                        continue;
                    }
                };
                let (w, h) = inner.backend.size(&sess);
                let grid = Grid::new(w, h, inner.config.tile_size);
                // A tile outside the grid is not an error of the image: the caller gets
                // `Failed` from `tile()` once the grid is known.
                let rendered = grid.rect(t).map(|r| {
                    inner
                        .backend
                        .render_region(&mut sess, r, inner.config.margin)
                        .and_then(|img| inner.backend.encode(&img))
                });
                let mut s = inner.lock();
                s.running.remove(&Job::Tile(key, t));
                if key.generation == s.generation {
                    s.grids.insert(key, grid);
                }
                let to_drop = return_session(&mut s, key, sess);
                let Some(rendered) = rendered else {
                    s.refresh_stats();
                    drop(s);
                    drop(to_drop);
                    inner.done.notify_all();
                    inner.emit(events);
                    continue;
                };
                match rendered {
                    Ok(bytes) if s.is_current(key) => {
                        let max = inner.config.max_tile_bytes;
                        s.insert_tile((key, t), Arc::new(bytes), max);
                        events.push(Event::Tile(key, t));
                    }
                    Ok(_) => s.stats.results_discarded += 1,
                    Err(e) => {
                        if key.generation == s.generation {
                            s.failed.insert(key, e.clone());
                            events.push(Event::Failed(key, e));
                        }
                    }
                }
                s.refresh_stats();
                drop(s);
                drop(to_drop);
                inner.done.notify_all();
                inner.emit(events);
            }
        }
    }
}

/// Hands the half-size job's session over as the kept session of `key` if `key` is the current
/// image and has none yet. Returns what must be dropped outside the lock.
fn keep_half_session<B: Backend>(
    inner: &Inner<B>,
    key: ImageKey,
    sess: B::Session,
    size: Option<(u32, u32)>,
) -> (Option<B::Session>, Vec<B::Session>) {
    let mut s = inner.lock();
    if !s.is_current(key) || s.sessions.iter().any(|(k, _)| *k == key) {
        return (Some(sess), Vec::new());
    }
    let evicted = evict_sessions(&mut s, inner.config.max_sessions);
    if s.sessions.len() >= inner.config.max_sessions.max(1) {
        return (Some(sess), evicted); // every kept session is in use
    }
    if let Some((w, h)) = size {
        s.grids.insert(key, Grid::new(w, h, inner.config.tile_size));
    }
    s.sessions.push((key, Slot::Idle(sess)));
    s.stats.sessions_reused += 1;
    // An unpack queued for this image is no longer needed.
    if s.current == Some(key) {
        s.unpack_wanted = false;
    }
    s.refresh_stats();
    drop(s);
    inner.work.notify_all();
    (None, evicted)
}

fn background_worker<B: Backend>(inner: &Inner<B>) {
    loop {
        let mut s = inner.lock();
        if s.shutdown {
            return;
        }
        // Tiles go first: start nothing while one is waiting.
        let wait_for: Option<Option<Duration>> = if !s.tile_queue.is_empty() {
            Some(Some(Duration::from_millis(20)))
        } else if s.half_queue.is_empty() || s.current.is_none() {
            Some(None)
        } else {
            let ready_at = s.current_since + inner.config.dwell;
            let now = Instant::now();
            (now < ready_at).then(|| Some(ready_at - now))
        };
        if let Some(d) = wait_for {
            wait_work(inner, s, d);
            continue;
        }
        let Some(key) = s.half_queue.pop_front() else {
            continue;
        };
        let Some(path) = s.targets.get(&key).map(|t| t.path.clone()) else {
            continue;
        };
        if s.halves.iter().any(|(k, _)| *k == key) || s.failed.contains_key(&key) {
            continue;
        }
        s.running.insert(Job::Half(key));
        s.refresh_stats();
        drop(s);
        // A temporary session of its own, so it never holds a session the tiles need. After the
        // render it may be handed over for tiles (current image only), before encoding.
        let mut size = None;
        let result = inner.backend.open(&path).and_then(|mut sess| {
            size = Some(inner.backend.size(&sess));
            let r = inner.backend.render_half(&mut sess);
            let leftover = if r.is_ok() && inner.config.reuse_half_session {
                keep_half_session(inner, key, sess, size)
            } else {
                (Some(sess), Vec::new())
            };
            drop(leftover);
            r.and_then(|img| inner.backend.encode(&img))
        });
        let mut s = inner.lock();
        s.running.remove(&Job::Half(key));
        let mut events = Vec::new();
        match result {
            Ok(bytes) if s.wanted(key) => {
                let max = inner.config.max_halves;
                s.insert_half(key, Arc::new(bytes), max);
                // The full-size grid, so the viewer knows what 100 % is for this half-size image.
                if let Some((w, h)) = size {
                    let g = Grid::new(w, h, inner.config.tile_size);
                    s.grids.insert(key, g);
                    events.push(Event::Grid(key, g));
                }
                events.push(Event::Half(key));
            }
            Ok(_) => s.stats.results_discarded += 1,
            Err(e) => {
                if key.generation == s.generation {
                    s.failed.insert(key, e.clone());
                    events.push(Event::Failed(key, e));
                }
            }
        }
        s.refresh_stats();
        drop(s);
        inner.done.notify_all();
        inner.emit(events);
    }
}
