//! The rules of the view core, without threads (ADR 0008 §12, ADR 0009 §3.4, §5): what is wanted,
//! in which order, what may be evicted, and which resident view serves the screen. [`crate::ViewCore`]
//! runs them on two workers; the tests here drive them directly.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::backend::{DecodeError, Rgb, Source};
use crate::geometry::{self, ImageSize, Rect};

/// Settings of the core. Both budgets are user settings (V8); in milestone 1 they come from the
/// command line (ADR 0009 §11).
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// LibRaw sessions (unpacked RAW data), in bytes. Default 4 GiB (ADR 0008 §12).
    pub core_budget: u64,
    /// Finished views held as textures, in bytes. Default 1 GiB (ADR 0008 §18).
    pub texture_budget: u64,
    /// Prefetch starts after the image has been current this long (150 ms, ADR 0008 §12).
    pub dwell: Duration,
    /// A free-zoom view is requested when the zoom gesture has paused this long (ADR 0009 §3.4).
    pub gesture_pause: Duration,
    /// The neutral loading indicator appears after this long (V2).
    pub indicator_after: Duration,
    /// Keep a record of every user request (measurement, ADR 0009 §5.2).
    pub record: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            core_budget: 4 << 30,
            texture_budget: 1 << 30,
            dwell: Duration::from_millis(150),
            gesture_pause: Duration::from_millis(100),
            indicator_after: Duration::from_secs(1),
            record: false,
        }
    }
}

/// The zoom level of the view model (V6, ADR 0009 §3.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    /// The whole image fitted to the view.
    Fit,
    /// Physical screen pixels per image pixel: 1.0 is 100 %.
    Scale(f64),
}

/// Smallest and largest free zoom: half of fit (or of 100 %, whichever is smaller) up to 800 %, as
/// the current viewer (`ui/web/app.js`, `zoomTo`).
pub const MAX_SCALE: f64 = 8.0;
/// One + or − step.
pub const ZOOM_STEP: f64 = 1.25;

/// What a texture holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewKind {
    /// The whole image at half size (2×2 binning): fit and small zoom levels.
    Half,
    /// The visible region at 100 %.
    Zoom,
    /// Twice the visible region at 100 %, for panning.
    Ring,
    /// The visible region at a free level between 50 % and 100 %, scaled down on the worker.
    Scaled,
}

impl ViewKind {
    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Half => "half",
            ViewKind::Zoom => "zoom",
            ViewKind::Ring => "ring",
            ViewKind::Scaled => "scaled",
        }
    }
}

/// The density of a half-size view: half an output pixel per image pixel.
pub const HALF_DENSITY: f64 = 0.5;
const EPS: f64 = 1e-9;

/// A finished view on its way to the presentation, which uploads it as a texture.
#[derive(Clone, Debug)]
pub struct ViewOut {
    pub generation: u64,
    pub pos: usize,
    pub source: Source,
    pub kind: ViewKind,
    /// The region of the pixel grid the pixels cover.
    pub region: Rect,
    /// Output pixels per image pixel (0.5 for half size, 1.0 at 100 %).
    pub density: f64,
    pub image: Rgb,
    /// FNV-1a over the pixels, only when recording (correctness checks).
    pub hash: Option<u64>,
}

/// A view held as a texture by the presentation.
#[derive(Clone, Debug)]
pub struct Resident {
    pub id: u64,
    pub generation: u64,
    pub pos: usize,
    pub source: Source,
    pub kind: ViewKind,
    pub region: Rect,
    pub density: f64,
    pub bytes: u64,
    pub used: u64,
    pub hash: Option<u64>,
}

/// What to draw this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw {
    /// The texture (from [`crate::ViewCore::accept`]).
    pub id: u64,
    /// Destination in physical view pixels: `[x0, y0, x1, y1]`.
    pub dest: [f32; 4],
    /// Texture coordinates of the destination's corners: top left, top right, bottom right,
    /// bottom left (orientation is in these).
    pub uv: [[f32; 2]; 4],
    /// Magnified beyond 100 %: draw with nearest-neighbour sampling (real pixels, no blur).
    pub nearest: bool,
}

/// The state of the current image.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// No folder, or an empty one.
    Empty,
    /// The current view is drawn.
    Ready,
    /// The current view is being prepared. The previous view stays (V2); `indicator` says whether
    /// the neutral indicator should show.
    Preparing { since: Instant, indicator: bool },
    /// The file cannot be shown (G3). Nothing of an earlier image is drawn.
    Error(DecodeError),
}

/// Everything the presentation needs for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub draw: Option<Draw>,
    pub status: Status,
    pub pos: Option<usize>,
    pub count: usize,
    pub path: Option<PathBuf>,
    /// The display mode chosen by the user (G4).
    pub mode: Source,
    /// Where the current image's pixels come from: the mode, except for a plain JPEG file, which
    /// is always shown as what it is.
    pub source: Source,
    /// Size of the pixel grid (sensor orientation for RAW; the JPEG's own grid), once known.
    pub size: Option<ImageSize>,
    pub zoom: Zoom,
    /// The effective scale (fit resolved), once the size is known.
    pub scale: Option<f64>,
    /// The current image's embedded colour profile was present but could not be used, so its
    /// colours rest on a fallback (ADR 0016 §5). Known once the image is opened.
    pub profile_unusable: bool,
}

/// One user request, for measurement (ADR 0009 §5.2; the P5 event format).
#[derive(Clone, Debug)]
pub struct Record {
    pub label: String,
    pub pos: usize,
    pub source: Source,
    pub zoom: Zoom,
    pub at: Instant,
    pub hit: bool,
    /// Why it was not a hit: "inflight", "evicted", "refused" or "not-started".
    pub miss: &'static str,
    pub shown: Option<Instant>,
    pub left: Option<Instant>,
}

/// Counters for measurement and tests.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub unpacks: u64,
    pub renders: u64,
    pub failures: u64,
    pub refused: u64,
    pub discarded: u64,
    pub evicted_textures: u64,
    pub evicted_sessions: u64,
    pub peak_texture_bytes: u64,
    pub peak_session_bytes: u64,
    pub render_ms: HashMap<&'static str, Vec<f64>>,
    pub unpack_ms: Vec<f64>,
}

/// An image in one display mode: a position in the folder and where the pixels come from.
pub(crate) type Key = (usize, Source);

/// What the plan wants of an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Want {
    /// The view for the current zoom level.
    Shown,
    /// The fit view.
    Fit,
    /// The 100 % view at the current position (the zoom target, V7).
    Target,
    /// The ring for panning at 100 %.
    Ring,
}

/// A view to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct JobView {
    pub kind: ViewKind,
    pub region: Rect,
    /// Density in millionths (hashable).
    pub density_q: u64,
}

impl JobView {
    pub fn density(self) -> f64 {
        self.density_q as f64 / 1e6
    }
}

fn quantise(d: f64) -> u64 {
    (d * 1e6).ceil().max(0.0) as u64
}

/// A requirement: a view that covers `cover` at `density` or more, and how to render one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Req {
    pub key: Key,
    pub cover: Rect,
    pub density: f64,
    pub job: JobView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Job {
    Open(Key),
    Render(Key, JobView),
}

impl Job {
    pub fn key(self) -> Key {
        match self {
            Job::Open(k) | Job::Render(k, _) => k,
        }
    }
}

/// A view being rendered, or rendered and not yet accepted.
#[derive(Clone, Copy, Debug)]
struct Inflight {
    generation: u64,
    key: Key,
    view: JobView,
    bytes: u64,
}

/// What a worker should do.
#[derive(Debug)]
pub(crate) enum Task<S> {
    Open {
        generation: u64,
        key: Key,
        path: PathBuf,
    },
    Render {
        generation: u64,
        key: Key,
        view: JobView,
        session: S,
    },
}

/// Whether `path` is a plain JPEG file (shown as itself in both modes, ADR 0009 §5.2).
pub fn is_plain_jpeg(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
}

/// Whether `path` is shown as itself in both display modes: a plain JPEG file (ADR 0009 §5.2) or
/// a TIFF file (ADR 0015 Q3). Such a file is always keyed [`Source::Jpeg`].
pub fn is_shown_as_itself(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["jpg", "jpeg", "tif", "tiff"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// The core's state.
#[derive(Debug)]
pub(crate) struct State<S> {
    pub config: Config,
    pub generation: u64,
    pub files: Vec<PathBuf>,
    pub cur: Option<usize>,
    dir: i8,
    since: Instant,
    /// The display mode chosen by the user (G4). Never changed by the program.
    pub mode: Source,
    pub zoom: Zoom,
    gesture_at: Option<Instant>,
    pub centre: [f64; 2],
    pub view_px: [u32; 2],
    pub sizes: HashMap<Key, ImageSize>,
    /// The opened images whose embedded colour profile could not be used (ADR 0016 §5). Kept,
    /// cleared and moved like `sizes`.
    pub profile_unusable: HashSet<Key>,
    sessions: HashMap<Key, Option<S>>,
    /// The generation each lent session (`None` in `sessions`) was lent in.
    lent: HashMap<Key, u64>,
    /// Sessions lent under an older order of the files ([`State::set_files`]), by the generation
    /// and key they were lent under, with their bytes. They still count in `session_bytes` and are
    /// dropped when they come back.
    detached: HashMap<(u64, Key), u64>,
    /// The file of every session lent to a worker, by the generation, key and view it was lent
    /// under (a view is rendered once at a time), so [`State::release`] knows which sessions
    /// still hold a file, also those lent under an older list of files.
    lent_paths: HashMap<(u64, Key, JobView), PathBuf>,
    /// Sessions lent in their generation that [`State::release`] marked to be dropped when they
    /// come back, by the generation, key and view they were lent under, with their bytes. They
    /// still count in `session_bytes`.
    released: HashMap<(u64, Key, JobView), u64>,
    /// The paths a [`crate::Releaser::release`] is waiting for (once per call): no new session
    /// of them is opened meanwhile.
    releasing: Vec<PathBuf>,
    session_bytes_of: HashMap<Key, u64>,
    session_lru: Vec<Key>,
    pub session_bytes: u64,
    last_session_bytes: u64,
    pub resident: Vec<Resident>,
    pub texture_bytes: u64,
    dropped: Vec<u64>,
    tick: u64,
    next_id: u64,
    inflight: Vec<Inflight>,
    /// Jobs handed to a worker, with the generation they were handed out in.
    running: HashSet<(u64, Job)>,
    failed: HashMap<Key, DecodeError>,
    ever_resident: HashSet<(u64, Key)>,
    refused: HashSet<(u64, Key)>,
    pub outbox: Vec<ViewOut>,
    last_draw: Option<Draw>,
    waiting_since: Option<Instant>,
    pub records: Vec<Record>,
    open_record: Option<usize>,
    pub stop: bool,
    pub stats: Stats,
}

impl<S> State<S> {
    pub fn new(config: Config) -> State<S> {
        State {
            config,
            generation: 0,
            files: Vec::new(),
            cur: None,
            dir: 1,
            since: Instant::now(),
            mode: Source::Raw,
            zoom: Zoom::Fit,
            gesture_at: None,
            centre: [0.5, 0.5],
            view_px: [1920, 1080],
            sizes: HashMap::new(),
            profile_unusable: HashSet::new(),
            sessions: HashMap::new(),
            lent: HashMap::new(),
            detached: HashMap::new(),
            lent_paths: HashMap::new(),
            released: HashMap::new(),
            releasing: Vec::new(),
            session_bytes_of: HashMap::new(),
            session_lru: Vec::new(),
            session_bytes: 0,
            last_session_bytes: 128 << 20,
            resident: Vec::new(),
            texture_bytes: 0,
            dropped: Vec::new(),
            tick: 0,
            next_id: 1,
            inflight: Vec::new(),
            running: HashSet::new(),
            failed: HashMap::new(),
            ever_resident: HashSet::new(),
            refused: HashSet::new(),
            outbox: Vec::new(),
            last_draw: None,
            waiting_since: None,
            records: Vec::new(),
            open_record: None,
            stop: false,
            stats: Stats::default(),
        }
    }

    // ---------- the view model ----------

    /// Where image `pos` comes from in display mode `mode`: plain JPEG and TIFF files are always
    /// `Source::Jpeg` (shown as themselves).
    pub fn source_in(&self, pos: usize, mode: Source) -> Source {
        match self.files.get(pos) {
            Some(p) if is_shown_as_itself(p) => Source::Jpeg,
            _ => mode,
        }
    }

    /// The key of image `pos` in the current mode.
    pub fn key(&self, pos: usize) -> Key {
        (pos, self.source_in(pos, self.mode))
    }

    fn cur_key(&self) -> Option<Key> {
        self.cur.map(|c| self.key(c))
    }

    /// The effective scale for `key` at the current zoom, once its size is known.
    pub fn scale_of(&self, key: Key) -> Option<f64> {
        let size = *self.sizes.get(&key)?;
        Some(match self.zoom {
            Zoom::Fit => geometry::fit_scale(size, self.view_px),
            Zoom::Scale(s) => s,
        })
    }

    fn min_scale(&self, size: Option<ImageSize>) -> f64 {
        let fit = size.map_or(1.0, |s| geometry::fit_scale(s, self.view_px));
        fit.min(1.0) * 0.5
    }

    /// Sets a new zoom level for the current image, keeping the point under `anchor` still.
    /// `gesture` marks a step of a zoom gesture (wheel, + and −): views that depend on the
    /// position are then asked for only when the gesture pauses (ADR 0009 §3.4).
    pub fn set_zoom(&mut self, zoom: Zoom, anchor: Option<[f64; 2]>, gesture: bool) {
        let key = self.cur_key();
        let size = key.and_then(|k| self.sizes.get(&k).copied());
        let zoom = match zoom {
            Zoom::Scale(s) if s.is_finite() && s > 0.0 => {
                Zoom::Scale(s.clamp(self.min_scale(size), MAX_SCALE))
            }
            Zoom::Scale(_) => return,
            Zoom::Fit => Zoom::Fit,
        };
        if let (Some(size), Some(from)) = (size, key.and_then(|k| self.scale_of(k))) {
            let to = match zoom {
                Zoom::Fit => geometry::fit_scale(size, self.view_px),
                Zoom::Scale(s) => s,
            };
            if zoom != Zoom::Fit {
                self.centre =
                    geometry::zoom_centre(size, self.view_px, from, to, self.centre, anchor);
            }
        }
        self.zoom = zoom;
        self.gesture_at = gesture.then(Instant::now);
    }

    /// Whether a zoom gesture is under way (its last step is younger than the pause).
    fn in_gesture(&self) -> bool {
        self.gesture_at
            .is_some_and(|t| t.elapsed() < self.config.gesture_pause)
    }

    /// Multiplies the current scale by `factor` (steps and the mouse wheel). Ignored before the
    /// current image's size is known when at fit.
    pub fn zoom_by(&mut self, factor: f64, anchor: Option<[f64; 2]>) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let known = self.cur_key().and_then(|k| self.scale_of(k));
        let Some(from) = known.or(match self.zoom {
            Zoom::Scale(s) => Some(s),
            Zoom::Fit => None,
        }) else {
            return;
        };
        self.set_zoom(Zoom::Scale(from * factor), anchor, true);
    }

    /// Z: fit ↔ 100 %.
    pub fn toggle_zoom(&mut self, anchor: Option<[f64; 2]>) {
        let size = self.cur_key().and_then(|k| self.sizes.get(&k).copied());
        let at_fit = match (self.zoom, size) {
            (Zoom::Fit, _) => true,
            (Zoom::Scale(s), Some(size)) => s <= geometry::fit_scale(size, self.view_px) * 1.01,
            (Zoom::Scale(_), None) => false,
        };
        if at_fit {
            self.set_zoom(Zoom::Scale(1.0), anchor, false);
        } else {
            self.set_zoom(Zoom::Fit, None, false);
        }
    }

    /// R: switches the display mode (G4). Zoom and position stay (V6); 100 % then means 100 % of
    /// the other source.
    pub fn set_mode(&mut self, mode: Source) {
        self.mode = mode;
    }

    /// Drags the image by `delta` view pixels.
    pub fn pan(&mut self, delta: [f64; 2]) {
        let Some(key) = self.cur_key() else { return };
        let (Some(size), Some(scale)) = (self.sizes.get(&key).copied(), self.scale_of(key)) else {
            return;
        };
        if self.zoom == Zoom::Fit {
            return;
        }
        self.centre = geometry::pan_centre(size, self.view_px, scale, self.centre, delta);
    }

    /// Makes `pos` current (clamped to the folder).
    pub fn go(&mut self, pos: usize) {
        if self.files.is_empty() {
            self.cur = None;
            return;
        }
        let pos = pos.min(self.files.len() - 1);
        self.dir = match self.cur {
            Some(c) if pos < c => -1,
            _ => 1,
        };
        self.cur = Some(pos);
        self.since = Instant::now();
    }

    /// A new folder: every cache is emptied and the first image becomes current. Textures are
    /// handed back through [`State::take_dropped`]; sessions in use are dropped when they return.
    /// The display mode stays (G4).
    pub fn open_folder(&mut self, files: Vec<PathBuf>) {
        self.generation += 1;
        self.files = files;
        self.cur = None;
        self.zoom = Zoom::Fit;
        self.centre = [0.5, 0.5];
        self.sessions.retain(|_, s| s.is_none());
        self.detached.clear();
        self.released.clear();
        self.session_bytes_of.clear();
        self.session_lru.clear();
        self.session_bytes = 0;
        self.sizes.clear();
        self.profile_unusable.clear();
        self.failed.clear();
        self.inflight.clear();
        self.outbox.clear();
        self.dropped.extend(self.resident.drain(..).map(|r| r.id));
        self.texture_bytes = 0;
        self.last_draw = None;
        self.waiting_since = None;
        if !self.files.is_empty() {
            self.go(0);
        }
    }

    /// The list of files changed, and `current` becomes current (clamped) (ADR 0014 §4.4, way C).
    /// The same list only goes to `current`. Otherwise the finished state of every path still in
    /// the list moves to its new position: sizes, failures, owned sessions and resident views
    /// (which keep their ids). The state of paths no longer in the list is released as eviction
    /// does. Work in flight belongs to the old order: a new generation starts, so none of its
    /// results is stored, and a session lent before is dropped when it comes back. Zoom, centre,
    /// direction and display mode are kept.
    pub fn set_files(&mut self, files: Vec<PathBuf>, current: usize) {
        if files == self.files {
            self.go(current);
            return;
        }
        let before = self.cur.and_then(|c| self.files.get(c).cloned());
        let old_generation = self.generation;
        let old_files = std::mem::replace(&mut self.files, files);
        // old position → new position, by path; each new position is taken at most once
        let moved: Vec<Option<usize>> = {
            let mut index: HashMap<&PathBuf, usize> = HashMap::new();
            for (i, p) in self.files.iter().enumerate() {
                index.entry(p).or_insert(i);
            }
            let mut taken = HashSet::new();
            old_files
                .iter()
                .map(|p| index.get(p).copied().filter(|n| taken.insert(*n)))
                .collect()
        };
        let to = |key: Key| moved.get(key.0).copied().flatten().map(|p| (p, key.1));
        self.generation += 1;
        let generation = self.generation;

        // work in flight belongs to the old order
        self.inflight.clear();
        self.outbox.clear();
        let lent: Vec<Key> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.is_none())
            .map(|(k, _)| *k)
            .collect();
        for key in lent {
            self.sessions.remove(&key);
            let lent_in = self.lent.remove(&key).unwrap_or(old_generation);
            let bytes = self.session_bytes_of.remove(&key).unwrap_or(0);
            self.detached.insert((lent_in, key), bytes);
        }
        self.lent.clear();

        // owned sessions move or are dropped
        let mut sessions = HashMap::new();
        let mut bytes_of = HashMap::new();
        for (key, slot) in self.sessions.drain() {
            let bytes = self.session_bytes_of.remove(&key);
            match (to(key), slot) {
                (Some(n), Some(s)) => {
                    sessions.insert(n, Some(s));
                    if let Some(b) = bytes {
                        bytes_of.insert(n, b);
                    }
                }
                _ => {
                    if let Some(b) = bytes {
                        self.session_bytes = self.session_bytes.saturating_sub(b);
                    }
                }
            }
        }
        for (_, b) in self.session_bytes_of.drain() {
            self.session_bytes = self.session_bytes.saturating_sub(b);
        }
        self.sessions = sessions;
        self.session_bytes_of = bytes_of;
        self.session_lru = std::mem::take(&mut self.session_lru)
            .into_iter()
            .filter_map(to)
            .filter(|k| self.sessions.contains_key(k))
            .collect();

        self.sizes = self
            .sizes
            .drain()
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        self.profile_unusable = self.profile_unusable.drain().filter_map(to).collect();
        self.failed = self
            .failed
            .drain()
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        self.ever_resident = self
            .ever_resident
            .drain()
            .filter(|(g, _)| *g == old_generation)
            .filter_map(|(_, k)| to(k).map(|n| (generation, n)))
            .collect();

        // resident views keep their ids
        let mut kept = Vec::with_capacity(self.resident.len());
        for mut r in self.resident.drain(..) {
            match to((r.pos, r.source)) {
                Some((pos, _)) if r.generation == old_generation => {
                    r.pos = pos;
                    r.generation = generation;
                    kept.push(r);
                }
                _ => {
                    self.texture_bytes = self.texture_bytes.saturating_sub(r.bytes);
                    self.dropped.push(r.id);
                }
            }
        }
        self.resident = kept;
        if let Some(d) = self.last_draw
            && !self.resident.iter().any(|r| r.id == d.id)
        {
            self.last_draw = None;
        }

        if self.files.is_empty() {
            self.cur = None;
            self.last_draw = None;
            self.waiting_since = None;
            return;
        }
        let pos = current.min(self.files.len() - 1);
        self.cur = Some(pos);
        if self.files.get(pos) != before.as_ref() {
            self.since = Instant::now();
        }
    }

    // ---------- the plan ----------

    fn neighbour(&self, cur: usize, k: usize, forward: bool) -> Option<usize> {
        let j = if forward {
            cur.checked_add(k)?
        } else {
            cur.checked_sub(k)?
        };
        (j < self.files.len()).then_some(j)
    }

    /// What is wanted now, most important first (strategy p5, ADR 0008 §12, with free zoom, ADR
    /// 0009 §3.4): the shown view; after the dwell the other view, +1 and +2 in the direction of
    /// travel, the current image in the other display mode, the ring, and −1 behind. `full` gives
    /// the whole plan regardless of the dwell and the zoom gesture; it decides what must not be
    /// evicted.
    pub fn wanted(&self, full: bool) -> Vec<(Key, Want)> {
        let Some(cur) = self.cur else {
            return Vec::new();
        };
        let ck = self.key(cur);
        let mut v: Vec<(Key, Want)> = Vec::new();
        let mut add = |x: (Key, Want)| {
            if !v.contains(&x) {
                v.push(x);
            }
        };
        if !full && self.in_gesture() {
            // during a zoom gesture only a view that does not depend on the position is asked for
            if self.scale_of(ck).is_some_and(|s| s <= HALF_DENSITY + EPS) {
                add((ck, Want::Shown));
            }
            return v;
        }
        add((ck, Want::Shown));
        if !(full || self.since.elapsed() >= self.config.dwell) {
            return v;
        }
        // free levels are not prefetched: neighbours get the fit view and the 100 % target
        let (primary, other) = match self.zoom {
            Zoom::Scale(s) if s >= 1.0 - EPS => (Want::Target, Want::Fit),
            _ => (Want::Fit, Want::Target),
        };
        let other_view = if self.zoom == Zoom::Fit {
            Want::Target
        } else {
            Want::Fit
        };
        add((ck, other_view));
        let forward = self.dir >= 0;
        for k in 1..=2 {
            if let Some(j) = self.neighbour(cur, k, forward) {
                add((self.key(j), primary));
                add((self.key(j), other));
            }
        }
        // the other display mode after the neighbours, not before (ADR 0008 §12)
        let other_mode = (cur, self.source_in(cur, self.mode.other()));
        if other_mode != ck {
            add((other_mode, primary));
        }
        add((ck, Want::Ring));
        if let Some(b) = self.neighbour(cur, 1, !forward) {
            add((self.key(b), primary));
        }
        v
    }

    /// The concrete requirement for a want, once the image's size is known.
    pub fn req(&self, key: Key, want: Want) -> Option<Req> {
        let size = *self.sizes.get(&key)?;
        if !size.is_valid() {
            return None;
        }
        let scale = match want {
            Want::Shown => self.scale_of(key)?,
            Want::Fit => geometry::fit_scale(size, self.view_px),
            Want::Target => 1.0,
            Want::Ring => {
                let vis = geometry::visible(size, self.view_px, 0.5, self.centre);
                let r = geometry::sensor_box(size, vis);
                let job = JobView {
                    kind: ViewKind::Ring,
                    region: r,
                    density_q: quantise(1.0),
                };
                return Some(Req {
                    key,
                    cover: r,
                    density: 1.0,
                    job,
                });
            }
        };
        let vis = geometry::visible(size, self.view_px, scale, self.centre);
        let cover = geometry::sensor_box(size, vis);
        let job = if scale <= HALF_DENSITY + EPS {
            JobView {
                kind: ViewKind::Half,
                region: geometry::whole(size),
                density_q: quantise(HALF_DENSITY),
            }
        } else if scale >= 1.0 - EPS {
            let vis1 = geometry::visible(size, self.view_px, 1.0, self.centre);
            JobView {
                kind: ViewKind::Zoom,
                region: geometry::sensor_box(size, vis1),
                density_q: quantise(1.0),
            }
        } else {
            JobView {
                kind: ViewKind::Scaled,
                region: cover,
                density_q: quantise(scale),
            }
        };
        Some(Req {
            key,
            cover,
            density: scale,
            job,
        })
    }

    fn serves(&self, generation: u64, key: Key, region: Rect, density: f64, req: &Req) -> bool {
        generation == self.generation
            && key == req.key
            && region.contains(req.cover)
            && density + EPS >= req.density.min(1.0)
    }

    /// The resident view that serves a requirement: the least dense one, then the smallest.
    pub fn serving(&self, req: &Req) -> Option<&Resident> {
        self.resident
            .iter()
            .filter(|r| self.serves(r.generation, (r.pos, r.source), r.region, r.density, req))
            .min_by(|a, b| {
                a.density
                    .total_cmp(&b.density)
                    .then(a.region.area().cmp(&b.region.area()))
            })
    }

    fn satisfied(&self, req: &Req) -> bool {
        self.serving(req).is_some()
            || self
                .inflight
                .iter()
                .any(|f| self.serves(f.generation, f.key, f.view.region, f.view.density(), req))
    }

    /// The jobs the plan asks for now, in order.
    pub fn plan(&self) -> Vec<Job> {
        let mut v = Vec::new();
        for (key, want) in self.wanted(false) {
            if self.failed.contains_key(&key) {
                continue;
            }
            if !self.sessions.contains_key(&key) {
                // a view whose size is known and is already served needs no session
                if let Some(req) = self.req(key, want)
                    && self.satisfied(&req)
                {
                    continue;
                }
                if !v.contains(&Job::Open(key)) {
                    v.push(Job::Open(key));
                }
                continue;
            }
            let Some(req) = self.req(key, want) else {
                continue;
            };
            if self.satisfied(&req) {
                continue;
            }
            let job = Job::Render(key, req.job);
            if !v.contains(&job) {
                v.push(job);
            }
        }
        v
    }

    // ---------- budgets ----------

    fn protected_keys(&self) -> HashSet<Key> {
        self.wanted(true)
            .into_iter()
            .map(|w| w.0)
            .chain(self.cur_key())
            .collect()
    }

    /// Resident views that must not be evicted: those serving the full plan, and the one on
    /// screen.
    fn protected_views(&self) -> HashSet<u64> {
        let mut p: HashSet<u64> = HashSet::new();
        for (key, want) in self.wanted(true) {
            if let Some(req) = self.req(key, want)
                && let Some(r) = self.serving(&req)
            {
                p.insert(r.id);
            }
        }
        if let Some(d) = self.last_draw {
            p.insert(d.id);
        }
        p
    }

    fn estimate_bytes(&self, key: Key, view: JobView) -> u64 {
        let area = view.region.area();
        match view.kind {
            ViewKind::Half => self.sizes.get(&key).map_or(64 << 20, |s| {
                u64::from(s.w.div_ceil(2)) * u64::from(s.h.div_ceil(2)) * 4
            }),
            ViewKind::Zoom | ViewKind::Ring => area * 4,
            ViewKind::Scaled => {
                let d = view.density();
                ((area as f64) * d * d * 4.0).ceil() as u64
            }
        }
    }

    fn textures_fit(&self, need: u64) -> bool {
        let protect = self.protected_views();
        let inflight: u64 = self.inflight.iter().map(|f| f.bytes).sum();
        let evictable: u64 = self
            .resident
            .iter()
            .filter(|r| !protect.contains(&r.id))
            .map(|r| r.bytes)
            .sum();
        self.texture_bytes + inflight + need <= self.config.texture_budget + evictable
    }

    /// Closes idle sessions the plan does not protect until `need` more bytes fit.
    fn sessions_fit(&mut self, need: u64) -> bool {
        let protect = self.protected_keys();
        while self.session_bytes + need > self.config.core_budget {
            let victim =
                self.session_lru.iter().copied().find(|k| {
                    !protect.contains(k) && matches!(self.sessions.get(k), Some(Some(_)))
                });
            let Some(k) = victim else { return false };
            self.drop_session(k);
            self.stats.evicted_sessions += 1;
        }
        true
    }

    fn drop_session(&mut self, k: Key) {
        self.sessions.remove(&k);
        self.session_lru.retain(|x| *x != k);
        if let Some(b) = self.session_bytes_of.remove(&k) {
            self.session_bytes = self.session_bytes.saturating_sub(b);
        }
    }

    // ---------- releasing a file (ADR 0007 §9.1 point 2) ----------

    /// Closes the sessions of `path` before its metadata is written: every owned session of it is
    /// dropped (its bytes taken off as an eviction does), and every session of it lent in the
    /// current generation is marked to be dropped when it comes back (sessions lent under an
    /// older generation are dropped then anyway). Paths are compared as stored in the file list.
    /// The views of `path` are kept. Returns `(changed, none_left)`: whether a session was dropped
    /// or marked, and whether no session of `path` is lent any more.
    pub fn release(&mut self, path: &std::path::Path) -> (bool, bool) {
        let mut changed = false;
        let owned: Vec<Key> = self
            .sessions
            .iter()
            .filter(|(k, slot)| {
                slot.is_some() && self.files.get(k.0).is_some_and(|p| p.as_path() == path)
            })
            .map(|(k, _)| *k)
            .collect();
        for k in owned {
            self.drop_session(k);
            changed = true;
        }
        let generation = self.generation;
        let lent: Vec<(u64, Key, JobView)> = self
            .lent_paths
            .iter()
            .filter(|((g, key, _), p)| {
                *g == generation
                    && p.as_path() == path
                    && self.lent.get(key) == Some(&generation)
                    && matches!(self.sessions.get(key), Some(None))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in lent {
            let key = id.1;
            self.sessions.remove(&key);
            self.lent.remove(&key);
            self.session_lru.retain(|x| *x != key);
            let bytes = self.session_bytes_of.remove(&key).unwrap_or(0);
            self.released.insert(id, bytes);
            changed = true;
        }
        let none_left = !self.lent_paths.values().any(|p| p.as_path() == path);
        (changed, none_left)
    }

    /// A release of `path` starts waiting: until [`State::end_release`], no new session of it is
    /// opened (the plan would otherwise open it again while the release waits).
    pub fn begin_release(&mut self, path: &std::path::Path) {
        self.releasing.push(path.to_path_buf());
    }

    /// The release of `path` is over: its sessions are opened again as usual.
    pub fn end_release(&mut self, path: &std::path::Path) {
        if let Some(i) = self.releasing.iter().position(|p| p.as_path() == path) {
            self.releasing.swap_remove(i);
        }
    }

    // ---------- the workers' side ----------

    /// Picks the next task. The interactive worker only takes the current image in the current
    /// mode.
    pub fn pick(&mut self, interactive: bool) -> Option<Task<S>> {
        let ck = self.cur_key();
        let generation = self.generation;
        for job in self.plan() {
            let key = job.key();
            if self.running.contains(&(generation, job)) || (interactive && Some(key) != ck) {
                continue;
            }
            let is_cur = Some(key) == ck;
            match job {
                Job::Open(key) => {
                    if self
                        .running
                        .iter()
                        .any(|(g, j)| *g == generation && j.key() == key)
                    {
                        continue;
                    }
                    // a file being released for a metadata write is not opened meanwhile
                    if self
                        .files
                        .get(key.0)
                        .is_some_and(|p| self.releasing.contains(p))
                    {
                        continue;
                    }
                    let need = self.last_session_bytes;
                    if !self.sessions_fit(need) && !is_cur {
                        self.stats.refused += 1;
                        self.refused.insert((self.generation, key));
                        continue;
                    }
                    let Some(path) = self.files.get(key.0).cloned() else {
                        continue;
                    };
                    self.running.insert((generation, job));
                    return Some(Task::Open {
                        generation: self.generation,
                        key,
                        path,
                    });
                }
                Job::Render(key, view) => {
                    let bytes = self.estimate_bytes(key, view);
                    if !is_cur && !self.textures_fit(bytes) {
                        self.stats.refused += 1;
                        self.refused.insert((self.generation, key));
                        continue;
                    }
                    let Some(slot) = self.sessions.get_mut(&key) else {
                        continue;
                    };
                    let Some(session) = slot.take() else {
                        continue;
                    };
                    if let Some(path) = self.files.get(key.0) {
                        self.lent_paths
                            .insert((generation, key, view), path.clone());
                    }
                    self.lent.insert(key, generation);
                    self.running.insert((generation, job));
                    self.inflight.push(Inflight {
                        generation: self.generation,
                        key,
                        view,
                        bytes,
                    });
                    return Some(Task::Render {
                        generation: self.generation,
                        key,
                        view,
                        session,
                    });
                }
            }
        }
        None
    }

    /// An open (RAW unpack, or JPEG read) finished (`ms` = how long it took), for a backend whose
    /// profiles are always usable. The core calls [`State::finish_open_checked`].
    #[cfg(test)]
    pub fn finish_open(
        &mut self,
        generation: u64,
        key: Key,
        result: Result<(S, ImageSize), DecodeError>,
        session_bytes: impl Fn(ImageSize, Source) -> u64,
        ms: f64,
    ) {
        self.finish_open_checked(generation, key, result, session_bytes, |_| false, ms);
    }

    /// [`State::finish_open`], and `profile_unusable` (the backend's
    /// [`profile_unusable`](crate::backend::Backend::profile_unusable)) is asked of the session
    /// after a successful open whose result is stored (ADR 0016 §5).
    pub fn finish_open_checked(
        &mut self,
        generation: u64,
        key: Key,
        result: Result<(S, ImageSize), DecodeError>,
        session_bytes: impl Fn(ImageSize, Source) -> u64,
        profile_unusable: impl Fn(&S) -> bool,
        ms: f64,
    ) {
        self.running.remove(&(generation, Job::Open(key)));
        self.stats.unpacks += 1;
        self.stats.unpack_ms.push(ms);
        if generation != self.generation {
            return;
        }
        match result {
            Ok((session, size)) => {
                let bytes = session_bytes(size, key.1);
                self.sizes.insert(key, size);
                if profile_unusable(&session) {
                    self.profile_unusable.insert(key);
                } else {
                    self.profile_unusable.remove(&key);
                }
                self.sessions.insert(key, Some(session));
                self.session_bytes_of.insert(key, bytes);
                self.session_bytes += bytes;
                self.last_session_bytes = bytes;
                self.session_lru.retain(|x| *x != key);
                self.session_lru.push(key);
                self.stats.peak_session_bytes =
                    self.stats.peak_session_bytes.max(self.session_bytes);
            }
            Err(e) => {
                self.stats.failures += 1;
                self.failed.insert(key, e);
            }
        }
    }

    /// A render finished. The session comes back (or is dropped if [`State::release`] marked
    /// it); the view goes to the outbox.
    pub fn finish_render(
        &mut self,
        generation: u64,
        key: Key,
        view: JobView,
        session: S,
        result: Result<Rgb, DecodeError>,
        ms: f64,
    ) {
        self.running.remove(&(generation, Job::Render(key, view)));
        self.lent_paths.remove(&(generation, key, view));
        self.stats.renders += 1;
        self.stats
            .render_ms
            .entry(view.kind.name())
            .or_default()
            .push(ms);
        if let Some(bytes) = self.released.remove(&(generation, key, view)) {
            // released for a metadata write: the session is dropped and its slot (gone, or
            // holding a newer session) is left alone; the view itself is still good
            self.session_bytes = self.session_bytes.saturating_sub(bytes);
            drop(session);
            if generation != self.generation {
                self.stats.discarded += 1;
                return;
            }
        } else if generation != self.generation {
            // the session is dropped; only the slot it was lent from is cleared, never a session
            // that now has the same key (ADR 0014 §4.4)
            if let Some(bytes) = self.detached.remove(&(generation, key)) {
                self.session_bytes = self.session_bytes.saturating_sub(bytes);
            } else if self.lent.get(&key) == Some(&generation)
                && matches!(self.sessions.get(&key), Some(None))
            {
                self.sessions.remove(&key);
                self.lent.remove(&key);
            }
            self.stats.discarded += 1;
            return;
        } else {
            self.lent.remove(&key);
            if let Some(slot) = self.sessions.get_mut(&key) {
                *slot = Some(session);
            }
            self.session_lru.retain(|x| *x != key);
            self.session_lru.push(key);
        }
        match result {
            Ok(image) => {
                let hash = self.config.record.then(|| fnv(&image.data));
                let density = match view.kind {
                    ViewKind::Half => HALF_DENSITY,
                    ViewKind::Zoom | ViewKind::Ring => 1.0,
                    ViewKind::Scaled => {
                        let r = view.region;
                        (f64::from(image.width) / f64::from(r.w.max(1)))
                            .min(f64::from(image.height) / f64::from(r.h.max(1)))
                    }
                };
                self.outbox.push(ViewOut {
                    generation,
                    pos: key.0,
                    source: key.1,
                    kind: view.kind,
                    region: view.region,
                    density,
                    image,
                    hash,
                });
            }
            Err(e) => {
                self.remove_inflight(generation, key, view.kind, view.region);
                self.stats.failures += 1;
                self.failed.insert(key, e);
            }
        }
    }

    fn remove_inflight(&mut self, generation: u64, key: Key, kind: ViewKind, region: Rect) {
        if let Some(i) = self.inflight.iter().position(|f| {
            f.generation == generation
                && f.key == key
                && f.view.kind == kind
                && f.view.region == region
        }) {
            self.inflight.swap_remove(i);
        }
    }

    // ---------- the presentation's side ----------

    /// The presentation uploads `out` as a texture of `bytes`. Returns its id, or `None` if it is
    /// discarded (an old folder, a free-zoom level no longer wanted, or no room for a view that is
    /// not of the current image). Evicted ids go to [`State::take_dropped`].
    pub fn accept(&mut self, out: &ViewOut, bytes: u64) -> Option<u64> {
        let key = (out.pos, out.source);
        self.remove_inflight(out.generation, key, out.kind, out.region);
        if out.generation != self.generation {
            self.stats.discarded += 1;
            return None;
        }
        if out.kind == ViewKind::Scaled {
            let wanted = self.wanted(true).into_iter().any(|(k, want)| {
                self.req(k, want).is_some_and(|req| {
                    self.serves(out.generation, key, out.region, out.density, &req)
                })
            });
            if !wanted {
                self.stats.discarded += 1;
                return None;
            }
        }
        let protect = self.protected_views();
        while self.texture_bytes + bytes > self.config.texture_budget {
            let victim = self
                .resident
                .iter()
                .enumerate()
                .filter(|(_, r)| !protect.contains(&r.id))
                .min_by_key(|(_, r)| r.used)
                .map(|(i, _)| i);
            let Some(i) = victim else { break };
            let r = self.resident.swap_remove(i);
            self.texture_bytes = self.texture_bytes.saturating_sub(r.bytes);
            self.stats.evicted_textures += 1;
            self.dropped.push(r.id);
        }
        if self.texture_bytes + bytes > self.config.texture_budget && Some(key) != self.cur_key() {
            self.stats.discarded += 1;
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tick += 1;
        self.resident.push(Resident {
            id,
            generation: out.generation,
            pos: out.pos,
            source: out.source,
            kind: out.kind,
            region: out.region,
            density: out.density,
            bytes,
            used: self.tick,
            hash: out.hash,
        });
        self.texture_bytes += bytes;
        self.ever_resident.insert((self.generation, key));
        self.stats.peak_texture_bytes = self.stats.peak_texture_bytes.max(self.texture_bytes);
        Some(id)
    }

    /// A view the presentation cannot upload (larger than the GPU allows). The image is marked
    /// as failed in that mode with `reason`.
    pub fn reject(&mut self, out: &ViewOut, reason: &str) {
        let key = (out.pos, out.source);
        self.remove_inflight(out.generation, key, out.kind, out.region);
        self.stats.discarded += 1;
        if out.generation == self.generation {
            self.failed.insert(key, DecodeError::new(reason));
        }
    }

    /// Texture ids the presentation must release.
    pub fn take_dropped(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.dropped)
    }

    /// What to draw now, for a view of `view_px` physical pixels.
    pub fn frame(&mut self, view_px: [u32; 2]) -> Frame {
        let view_px = [view_px[0].max(1), view_px[1].max(1)];
        self.view_px = view_px;
        let key = self.cur_key();
        let mut frame = Frame {
            draw: None,
            status: Status::Empty,
            pos: self.cur,
            count: self.files.len(),
            path: self.cur.and_then(|c| self.files.get(c).cloned()),
            mode: self.mode,
            source: key.map_or(self.mode, |k| k.1),
            size: key.and_then(|k| self.sizes.get(&k).copied()),
            zoom: self.zoom,
            scale: key.and_then(|k| self.scale_of(k)),
            profile_unusable: key.is_some_and(|k| self.profile_unusable.contains(&k)),
        };
        let Some(key) = key else {
            self.last_draw = None;
            return frame;
        };
        if let Some(e) = self.failed.get(&key) {
            // G3: an unreadable file never leaves the previous image on screen; G4: a file
            // without an embedded JPEG says so in JPEG mode
            frame.status = Status::Error(e.clone());
            self.last_draw = None;
            self.waiting_since = None;
            return frame;
        }
        let ready = self.req(key, Want::Shown).and_then(|req| {
            let size = *self.sizes.get(&key)?;
            let scale = self.scale_of(key)?;
            let r = self.serving(&req)?;
            let vis = geometry::visible(size, view_px, scale, self.centre);
            let d = geometry::dest(vis, view_px, scale);
            Some(Draw {
                id: r.id,
                dest: [d[0] as f32, d[1] as f32, d[2] as f32, d[3] as f32],
                uv: geometry::corner_uvs(size, r.region, vis),
                nearest: scale > 1.0 + EPS,
            })
        });
        match ready {
            Some(draw) => {
                self.touch(draw.id);
                self.last_draw = Some(draw);
                self.waiting_since = None;
                self.mark_shown();
                frame.draw = Some(draw);
                frame.status = Status::Ready;
            }
            None => {
                let since = *self.waiting_since.get_or_insert_with(Instant::now);
                frame.draw = self.last_draw;
                frame.status = Status::Preparing {
                    since,
                    indicator: since.elapsed() >= self.config.indicator_after,
                };
            }
        }
        frame
    }

    fn touch(&mut self, id: u64) {
        self.tick += 1;
        let t = self.tick;
        if let Some(r) = self.resident.iter_mut().find(|r| r.id == id) {
            r.used = t;
        }
    }

    /// Something changed what is wanted: the old waiting time no longer applies.
    pub fn changed(&mut self) {
        self.waiting_since = None;
    }

    // ---------- measurement ----------

    /// Records a user request (only with `Config::record`).
    pub fn record(&mut self, label: &str) {
        if !self.config.record {
            return;
        }
        let now = Instant::now();
        if let Some(i) = self.open_record.take()
            && let Some(r) = self.records.get_mut(i)
        {
            r.left = Some(now);
        }
        let Some(key) = self.cur_key() else { return };
        let hit = self
            .req(key, Want::Shown)
            .is_some_and(|req| self.serving(&req).is_some());
        let miss = if hit {
            ""
        } else if self
            .running
            .iter()
            .any(|(g, j)| *g == self.generation && j.key() == key)
        {
            "inflight"
        } else if self.ever_resident.contains(&(self.generation, key)) {
            "evicted"
        } else if self.refused.contains(&(self.generation, key)) {
            "refused"
        } else {
            "not-started"
        };
        self.records.push(Record {
            label: label.to_string(),
            pos: key.0,
            source: key.1,
            zoom: self.zoom,
            at: now,
            hit,
            miss,
            shown: None,
            left: None,
        });
        self.open_record = Some(self.records.len() - 1);
    }

    fn mark_shown(&mut self) {
        let key = self.cur_key();
        if let Some(i) = self.open_record
            && let Some(r) = self.records.get_mut(i)
            && r.shown.is_none()
            && Some((r.pos, r.source)) == key
        {
            r.shown = Some(Instant::now());
        }
    }

    /// How long a worker may sleep before the plan can change by itself (dwell or gesture pause).
    pub fn next_deadline(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut d: Option<Duration> = None;
        let mut consider = |t: Instant| {
            if t > now {
                let left = t - now;
                d = Some(d.map_or(left, |x| x.min(left)));
            }
        };
        if self.cur.is_some() {
            consider(self.since + self.config.dwell);
            if let Some(t) = self.gesture_at {
                consider(t + self.config.gesture_pause);
            }
        }
        d
    }
}

/// FNV-1a over a byte slice (correctness checks in measurement runs).
pub fn fnv(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: ImageSize = ImageSize {
        w: 4000,
        h: 3000,
        flip: 0,
    };
    const VIEW: [u32; 2] = [1000, 800];

    fn files(n: usize) -> Vec<PathBuf> {
        (0..n)
            .map(|i| PathBuf::from(format!("img{i}.raw")))
            .collect()
    }

    /// A folder of 20 images with image 10 current, past the dwell.
    fn state(tex_mb: u64) -> State<u32> {
        let config = Config {
            texture_budget: tex_mb << 20,
            ..Config::default()
        };
        let mut s = State::new(config);
        s.open_folder(files(20));
        s.view_px = VIEW;
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        s
    }

    fn k(pos: usize) -> Key {
        (pos, Source::Raw)
    }

    fn known(s: &mut State<u32>, pos: usize) {
        s.sizes.insert(k(pos), SIZE);
    }

    fn with_session(s: &mut State<u32>, pos: usize) {
        known(s, pos);
        s.sessions.insert(k(pos), Some(pos as u32));
    }

    fn out(s: &State<u32>, pos: usize, want: Want) -> ViewOut {
        out_in(s, k(pos), want)
    }

    fn out_in(s: &State<u32>, key: Key, want: Want) -> ViewOut {
        let pos = key.0;
        let req = s.req(key, want);
        let job = req.map(|r| r.job).unwrap_or(JobView {
            kind: ViewKind::Half,
            region: Rect::new(0, 0, 1, 1),
            density_q: 500_000,
        });
        let density = match job.kind {
            ViewKind::Half => HALF_DENSITY,
            ViewKind::Zoom | ViewKind::Ring => 1.0,
            ViewKind::Scaled => job.density(),
        };
        ViewOut {
            generation: s.generation,
            pos,
            source: key.1,
            kind: job.kind,
            region: job.region,
            density,
            image: Rgb {
                width: 1,
                height: 1,
                data: vec![0; 3],
            },
            hash: None,
        }
    }

    #[test]
    fn wanted_follows_p5_order() {
        let mut s = state(1024);
        use Want::*;
        assert_eq!(
            s.wanted(false),
            vec![
                (k(10), Shown),
                (k(10), Target),
                (k(11), Fit),
                (k(11), Target),
                (k(12), Fit),
                (k(12), Target),
                ((10, Source::Jpeg), Fit),
                (k(10), Ring),
                (k(9), Fit)
            ]
        );
        s.since = Instant::now();
        assert_eq!(s.wanted(false), vec![(k(10), Shown)]);
        s.go(9); // backwards
        s.since = Instant::now() - Duration::from_secs(1);
        s.zoom = Zoom::Scale(1.0);
        assert_eq!(
            s.wanted(false)[..4],
            [(k(9), Shown), (k(9), Fit), (k(8), Target), (k(8), Fit)]
        );
    }

    #[test]
    fn the_plan_stays_inside_the_folder() {
        let mut s = state(1024);
        s.go(19);
        s.since = Instant::now() - Duration::from_secs(1);
        assert!(s.wanted(false).iter().all(|(key, _)| key.0 < 20));
        s.go(0);
        s.since = Instant::now() - Duration::from_secs(1);
        assert!(s.wanted(false).iter().all(|(key, _)| key.0 < 20));
    }

    #[test]
    fn free_zoom_waits_for_the_gesture_to_pause_and_is_not_prefetched() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        s.set_zoom(Zoom::Scale(0.7), None, true);
        // during the gesture nothing that depends on the position is asked for, but it is
        // protected
        assert!(s.wanted(false).is_empty());
        assert!(s.wanted(true).contains(&(k(10), Want::Shown)));
        // a gesture step at a level served by the half-size view asks for it at once
        s.set_zoom(Zoom::Scale(0.3), None, true);
        assert_eq!(s.wanted(false), vec![(k(10), Want::Shown)]);
        // Z is not a gesture: no waiting
        s.set_zoom(Zoom::Scale(1.0), None, false);
        assert_eq!(s.wanted(false)[0], (k(10), Want::Shown));
        s.set_zoom(Zoom::Scale(0.7), None, true);
        s.gesture_at = Some(Instant::now() - Duration::from_secs(1));
        let w = s.wanted(false);
        assert_eq!(w[0], (k(10), Want::Shown));
        assert!(
            w.iter()
                .filter(|(key, _)| key.0 != 10)
                .all(|(_, x)| matches!(x, Want::Fit | Want::Target))
        );
        // the job is a scaled view of the visible region
        assert!(matches!(s.plan()[0], Job::Render((10, _), v) if v.kind == ViewKind::Scaled));
    }

    #[test]
    fn the_job_follows_the_scale() {
        let mut s = state(1024);
        known(&mut s, 10);
        let kind = |s: &mut State<u32>, z: Zoom| {
            s.zoom = z;
            s.req(k(10), Want::Shown).map(|r| r.job.kind)
        };
        assert_eq!(kind(&mut s, Zoom::Fit), Some(ViewKind::Half));
        assert_eq!(kind(&mut s, Zoom::Scale(0.4)), Some(ViewKind::Half));
        assert_eq!(kind(&mut s, Zoom::Scale(0.75)), Some(ViewKind::Scaled));
        assert_eq!(kind(&mut s, Zoom::Scale(1.0)), Some(ViewKind::Zoom));
        assert_eq!(kind(&mut s, Zoom::Scale(4.0)), Some(ViewKind::Zoom));
        // a tiny image whose fit scale is above 50 % gets a sharp view, not the half-size one
        s.sizes.insert(
            k(10),
            ImageSize {
                w: 900,
                h: 600,
                flip: 0,
            },
        );
        assert_eq!(kind(&mut s, Zoom::Fit), Some(ViewKind::Zoom));
    }

    #[test]
    fn a_view_is_never_drawn_upscaled_below_100_percent() {
        let mut s = state(1024);
        known(&mut s, 10);
        let half = s.accept(&out(&s, 10, Want::Fit), 100);
        assert!(half.is_some());
        s.zoom = Zoom::Scale(0.4);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), half);
        s.zoom = Zoom::Scale(0.7);
        let f = s.frame(VIEW);
        assert!(matches!(
            f.status,
            Status::Preparing {
                indicator: false,
                ..
            }
        ));
        // the previous (half-size) view stays on screen until the sharp one arrives (V2)
        assert_eq!(f.draw.map(|d| d.id), half);
        let scaled = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(scaled.is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), scaled);
    }

    #[test]
    fn a_100_percent_view_serves_higher_levels_with_nearest_sampling() {
        let mut s = state(1024);
        known(&mut s, 10);
        s.zoom = Zoom::Scale(1.0);
        let id = s.accept(&out(&s, 10, Want::Shown), 100);
        s.zoom = Zoom::Scale(3.0);
        let f = s.frame(VIEW);
        assert_eq!(f.draw.map(|d| d.id), id);
        assert!(f.draw.is_some_and(|d| d.nearest));
        // the drawn area fills the view
        let d = f.draw.map(|d| d.dest).unwrap_or([0.0; 4]);
        assert!((d[2] - d[0] - 1000.0).abs() < 0.01 && (d[3] - d[1] - 800.0).abs() < 0.01);
    }

    #[test]
    fn a_ring_serves_a_pan_inside_it() {
        let mut s = state(1024);
        known(&mut s, 10);
        s.zoom = Zoom::Scale(1.0);
        let ring = s.accept(&out(&s, 10, Want::Ring), 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), ring);
        s.pan([200.0, 0.0]);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), ring);
        s.pan([5000.0, 5000.0]);
        assert_ne!(s.frame(VIEW).status, Status::Ready);
    }

    #[test]
    fn zoom_limits_steps_and_toggle() {
        let mut s = state(1024);
        known(&mut s, 10);
        let fit = geometry::fit_scale(SIZE, VIEW);
        s.toggle_zoom(None);
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        s.toggle_zoom(None);
        assert_eq!(s.zoom, Zoom::Fit);
        s.zoom_by(ZOOM_STEP, None);
        assert_eq!(s.zoom, Zoom::Scale(fit * ZOOM_STEP));
        s.zoom_by(1000.0, None);
        assert_eq!(s.zoom, Zoom::Scale(MAX_SCALE));
        s.zoom_by(1e-6, None);
        assert_eq!(s.zoom, Zoom::Scale(fit * 0.5));
        s.zoom_by(f64::NAN, None);
        assert_eq!(s.zoom, Zoom::Scale(fit * 0.5));
        // the level persists when stepping (V6)
        s.zoom = Zoom::Scale(2.0);
        s.go(11);
        assert_eq!(s.zoom, Zoom::Scale(2.0));
    }

    #[test]
    fn zooming_before_the_size_is_known() {
        let mut s = state(1024);
        s.zoom_by(ZOOM_STEP, None);
        assert_eq!(s.zoom, Zoom::Fit);
        s.toggle_zoom(None);
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        s.pan([10.0, 10.0]);
        assert_eq!(s.centre, [0.5, 0.5]);
    }

    #[test]
    fn plan_unpacks_first_then_renders() {
        let mut s = state(1024);
        assert_eq!(s.plan()[0], Job::Open(k(10)));
        with_session(&mut s, 10);
        assert!(matches!(s.plan()[0], Job::Render((10, _), v) if v.kind == ViewKind::Half));
        // the interactive worker only takes the current image
        assert!(matches!(
            s.pick(true),
            Some(Task::Render { key: (10, _), .. })
        ));
        // the session is in use: the same job is not handed out twice
        assert!(!matches!(
            s.pick(true),
            Some(Task::Render { key: (10, _), .. })
        ));
    }

    #[test]
    fn the_current_image_is_never_evicted_and_unwanted_ones_go_first() {
        let mut s = state(1);
        for p in [3, 10] {
            known(&mut s, p);
        }
        let a = s.accept(&out(&s, 3, Want::Fit), 600 << 10);
        let b = s.accept(&out(&s, 10, Want::Fit), 600 << 10);
        assert!(a.is_some() && b.is_some());
        assert_eq!(s.take_dropped(), a.into_iter().collect::<Vec<_>>());
        // over budget with only the current image: it still stays
        let c = s.accept(&out(&s, 10, Want::Target), 900 << 10);
        assert!(c.is_some());
        assert!(s.take_dropped().is_empty());
        assert!(s.texture_bytes > s.config.texture_budget);
    }

    #[test]
    fn the_view_on_screen_is_kept_while_the_next_is_prepared() {
        let mut s = state(1);
        known(&mut s, 10);
        known(&mut s, 12);
        let shown = s.accept(&out(&s, 10, Want::Fit), 900 << 10);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), shown);
        // a jump far ahead: image 10 is no longer in the plan, but its view is still on screen
        s.go(14);
        let _ = s.accept(&out(&s, 12, Want::Fit), 900 << 10);
        assert!(s.resident.iter().any(|r| Some(r.id) == shown));
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), shown);
    }

    #[test]
    fn prefetch_that_does_not_fit_is_refused() {
        let mut s = state(1);
        with_session(&mut s, 11);
        known(&mut s, 10);
        s.sessions.insert(k(10), None); // in use
        assert!(!matches!(
            s.pick(false),
            Some(Task::Render { key: (11, _), .. })
        ));
        assert!(s.stats.refused > 0);
    }

    #[test]
    fn sessions_stay_within_the_core_budget() {
        let config = Config {
            core_budget: 50 << 20,
            ..Config::default()
        };
        let mut s: State<u32> = State::new(config);
        s.open_folder(files(20));
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        let bytes = |_: ImageSize, _: Source| 24u64 << 20;
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), bytes, 1.0);
        s.finish_open(s.generation, k(11), Ok((11, SIZE)), bytes, 1.0);
        assert_eq!(s.session_bytes, 48 << 20);
        // both sessions are in the plan: nothing can go, so another one does not fit
        assert!(!s.sessions_fit(24 << 20));
        assert_eq!(s.stats.evicted_sessions, 0);
        // a session outside the plan is closed to make room
        s.finish_open(s.generation, k(3), Ok((3, SIZE)), |_, _| 1 << 20, 1.0);
        assert!(s.sessions_fit(2 << 20));
        assert_eq!(s.stats.evicted_sessions, 1);
        assert!(!s.sessions.contains_key(&k(3)));
        assert_eq!(s.session_bytes, 48 << 20);
    }

    #[test]
    fn a_failed_file_shows_an_error_and_not_the_previous_image() {
        let mut s = state(1024);
        known(&mut s, 10);
        let _ = s.accept(&out(&s, 10, Want::Fit), 100);
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        s.finish_open(
            s.generation,
            k(11),
            Err(DecodeError::damaged("damaged: cut short")),
            |_, _| 0,
            1.0,
        );
        s.go(11);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Error(ref e) if e.damaged));
        assert!(f.draw.is_none());
        assert!(!s.plan().iter().any(|j| j.key().0 == 11));
    }

    #[test]
    fn old_folder_results_are_discarded() {
        let mut s = state(1024);
        known(&mut s, 10);
        let o = out(&s, 10, Want::Fit);
        let id = s.accept(&o, 100);
        s.open_folder(files(3));
        assert_eq!(s.take_dropped(), id.into_iter().collect::<Vec<_>>());
        assert!(s.accept(&o, 100).is_none());
        // a session coming back from the old folder is dropped
        s.finish_render(
            s.generation - 1,
            k(10),
            o_job(&o),
            7,
            Ok(o.image.clone()),
            1.0,
        );
        assert!(!s.sessions.contains_key(&k(10)));
        assert!(s.outbox.is_empty());
        assert_eq!(s.cur, Some(0));
    }

    fn o_job(o: &ViewOut) -> JobView {
        JobView {
            kind: o.kind,
            region: o.region,
            density_q: quantise(o.density),
        }
    }

    /// `running` holds the generation (T54): after `open_folder`, an open of a position whose
    /// open from the old folder is still running starts at once instead of waiting for it, and
    /// the old one coming back neither stores anything nor frees the new one's place.
    #[test]
    fn a_new_folder_does_not_wait_for_an_open_of_the_old_folder() {
        let mut s = state(1024);
        let old = s.generation;
        assert!(matches!(s.pick(true), Some(Task::Open { key, .. }) if key == k(10)));
        s.open_folder(
            (0..20)
                .map(|i| PathBuf::from(format!("new{i}.raw")))
                .collect(),
        );
        s.go(10);
        let new = s.generation;
        assert!(matches!(
            s.pick(true),
            Some(Task::Open { generation, key, .. }) if generation == new && key == k(10)
        ));
        s.finish_open(old, k(10), Ok((99, SIZE)), mb24, 1.0);
        assert!(s.sessions.is_empty() && s.sizes.is_empty());
        assert_eq!(s.session_bytes, 0);
        // the new open is still running: it is not handed out twice
        assert!(s.pick(true).is_none());
        s.finish_open(new, k(10), Ok((10, SIZE)), mb24, 1.0);
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert_eq!(s.session_bytes, 24 << 20);
    }

    #[test]
    fn a_stale_free_zoom_view_is_discarded() {
        let mut s = state(1024);
        known(&mut s, 10);
        s.zoom = Zoom::Scale(0.7);
        let o = out(&s, 10, Want::Shown);
        s.zoom = Zoom::Scale(0.9);
        assert!(s.accept(&o, 100).is_none());
        assert_eq!(s.stats.discarded, 1);
    }

    #[test]
    fn a_finished_render_returns_its_session_and_queues_the_view() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        let Some(Task::Render {
            generation,
            key,
            view,
            session,
        }) = s.pick(true)
        else {
            panic!("expected a render task");
        };
        assert!(
            s.plan().iter().all(|j| *j != Job::Render(key, view)),
            "in flight counts as satisfied"
        );
        let img = Rgb {
            width: 2000,
            height: 1500,
            data: vec![0; 2000 * 1500 * 3],
        };
        s.finish_render(generation, key, view, session, Ok(img), 1.0);
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(_))));
        assert_eq!(s.outbox.len(), 1);
        assert_eq!(s.outbox[0].density, HALF_DENSITY);
    }

    #[test]
    fn records_classify_misses_and_mark_the_shown_time() {
        let config = Config {
            record: true,
            ..Config::default()
        };
        let mut s: State<u32> = State::new(config);
        s.open_folder(files(20));
        s.view_px = VIEW;
        s.go(10);
        s.record("a");
        assert_eq!(s.records[0].miss, "not-started");
        known(&mut s, 10);
        let _ = s.accept(&out(&s, 10, Want::Shown), 100);
        s.record("b");
        assert!(s.records[1].hit);
        assert!(s.records[0].left.is_some());
        let _ = s.frame(VIEW);
        assert!(s.records[1].shown.is_some());
    }

    #[test]
    fn the_indicator_appears_after_the_configured_time() {
        let mut s = state(1024);
        let f = s.frame(VIEW);
        assert!(matches!(
            f.status,
            Status::Preparing {
                indicator: false,
                ..
            }
        ));
        s.waiting_since = Some(Instant::now() - Duration::from_secs(2));
        assert!(matches!(
            s.frame(VIEW).status,
            Status::Preparing {
                indicator: true,
                ..
            }
        ));
        s.changed();
        assert!(matches!(
            s.frame(VIEW).status,
            Status::Preparing {
                indicator: false,
                ..
            }
        ));
    }

    #[test]
    fn an_empty_folder_is_empty() {
        let mut s: State<u32> = State::new(Config::default());
        s.open_folder(Vec::new());
        assert_eq!(s.frame(VIEW).status, Status::Empty);
        assert!(s.plan().is_empty());
        s.go(3);
        assert_eq!(s.cur, None);
    }

    #[test]
    fn the_display_mode_selects_the_source_and_never_changes_by_itself() {
        let mut s = state(1024);
        assert_eq!(s.key(10), k(10));
        s.set_mode(Source::Jpeg);
        assert_eq!(s.key(10), (10, Source::Jpeg));
        // the plan follows the mode; the RAW rendering comes after the neighbours
        let w = s.wanted(false);
        assert_eq!(w[0], ((10, Source::Jpeg), Want::Shown));
        let raw = w.iter().position(|x| x.0 == k(10));
        let next = w.iter().position(|x| x.0 == (11, Source::Jpeg));
        assert!(raw.is_some() && next.is_some() && raw > next, "{w:?}");
        // a folder change keeps the mode
        s.open_folder(files(3));
        assert_eq!(s.mode, Source::Jpeg);
    }

    #[test]
    fn a_plain_jpeg_file_is_shown_as_itself_in_both_modes() {
        let mut s: State<u32> = State::new(Config::default());
        s.open_folder(vec![PathBuf::from("a.JPG"), PathBuf::from("b.arw")]);
        assert_eq!(s.key(0), (0, Source::Jpeg));
        assert_eq!(s.key(1), (1, Source::Raw));
        s.set_mode(Source::Jpeg);
        assert_eq!(s.key(0), (0, Source::Jpeg));
        s.since = Instant::now() - Duration::from_secs(1);
        // no "other mode" entry for a plain JPEG
        assert!(
            s.wanted(false)
                .iter()
                .all(|(key, _)| *key != (0, Source::Raw))
        );
    }

    #[test]
    fn jpeg_and_tiff_files_are_shown_as_themselves() {
        use std::path::Path;
        assert!(is_shown_as_itself(Path::new("a.tif")));
        assert!(is_shown_as_itself(Path::new("b.TIFF")));
        assert!(is_shown_as_itself(Path::new("c.jpg")));
        assert!(!is_shown_as_itself(Path::new("d.arw")));
        assert!(!is_shown_as_itself(Path::new("e.dng")));
        assert!(!is_shown_as_itself(Path::new("f")));
    }

    #[test]
    fn a_tiff_file_is_shown_as_itself_in_both_modes() {
        let mut s: State<u32> = State::new(Config::default());
        s.open_folder(vec![PathBuf::from("a.tif"), PathBuf::from("b.arw")]);
        assert_eq!(s.key(0), (0, Source::Jpeg));
        assert_eq!(s.key(1), (1, Source::Raw));
        s.set_mode(Source::Jpeg);
        assert_eq!(s.key(0), (0, Source::Jpeg));
        s.since = Instant::now() - Duration::from_secs(1);
        // no "other mode" entry for a TIFF
        assert!(
            s.wanted(false)
                .iter()
                .all(|(key, _)| *key != (0, Source::Raw))
        );
    }

    #[test]
    fn a_damaged_raw_keeps_its_embedded_jpeg_and_a_missing_jpeg_says_so() {
        let mut s = state(1024);
        s.finish_open(
            s.generation,
            k(10),
            Err(DecodeError::damaged("damaged: cut short")),
            |_, _| 0,
            1.0,
        );
        assert!(matches!(s.frame(VIEW).status, Status::Error(_)));
        // G3: the intact embedded preview is still shown in JPEG mode
        s.set_mode(Source::Jpeg);
        s.sizes.insert((10, Source::Jpeg), SIZE);
        let id = s.accept(&out_in(&s, (10, Source::Jpeg), Want::Shown), 100);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), id);
        assert_eq!(f.source, Source::Jpeg);
        // G4: a file without an embedded JPEG shows a message, not the RAW rendering
        s.finish_open(
            s.generation,
            (11, Source::Jpeg),
            Err(DecodeError::new("no embedded JPEG")),
            |_, _| 0,
            1.0,
        );
        s.go(11);
        assert!(matches!(s.frame(VIEW).status, Status::Error(ref e) if !e.damaged));
    }

    #[test]
    fn switching_mode_keeps_the_previous_view_until_the_new_one_is_ready() {
        let mut s = state(1024);
        known(&mut s, 10);
        let raw = s.accept(&out(&s, 10, Want::Shown), 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), raw);
        s.set_mode(Source::Jpeg);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw.map(|d| d.id), raw);
    }

    // ---------- set_files (ADR 0014 §4.4, T54) ----------

    fn path(i: usize) -> PathBuf {
        PathBuf::from(format!("img{i}.raw"))
    }

    /// `files(n)` without the images in `gone`.
    fn without(n: usize, gone: &[usize]) -> Vec<PathBuf> {
        (0..n).filter(|i| !gone.contains(i)).map(path).collect()
    }

    /// `session_bytes` is the sum of the sessions held and the detached ones still lent.
    fn exact(s: &State<u32>) -> bool {
        s.session_bytes
            == s.session_bytes_of.values().sum::<u64>() + s.detached.values().sum::<u64>()
    }

    fn mb24(_: ImageSize, _: Source) -> u64 {
        24 << 20
    }

    #[test]
    fn set_files_moves_a_resident_view_to_its_new_position_with_the_same_id() {
        let mut s = state(1024);
        known(&mut s, 10);
        let id = s.accept(&out(&s, 10, Want::Fit), 100);
        assert!(id.is_some());
        // image 3 drops out: image 10 is now at position 9
        s.set_files(without(20, &[3]), 9);
        assert!(s.take_dropped().is_empty());
        let r = s.resident.iter().find(|r| Some(r.id) == id);
        assert!(r.is_some_and(|r| r.pos == 9 && r.generation == s.generation));
        assert_eq!(s.sizes.get(&k(9)), Some(&SIZE));
        assert!(!s.sizes.contains_key(&k(10)));
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), id);
        assert_eq!(f.path, Some(path(10)));
        // drawn without a new render: nothing is planned for the shown view
        s.since = Instant::now();
        assert!(s.plan().is_empty());
    }

    #[test]
    fn set_files_releases_the_view_and_session_of_a_removed_image() {
        let mut s = state(1024);
        s.finish_open(s.generation, k(11), Ok((11, SIZE)), mb24, 1.0);
        s.finish_open(s.generation, k(12), Ok((12, SIZE)), mb24, 1.0);
        let gone = s.accept(&out(&s, 12, Want::Fit), 100);
        let stays = s.accept(&out(&s, 11, Want::Fit), 50);
        assert!(gone.is_some() && stays.is_some());
        s.set_files(without(20, &[12]), 10);
        assert_eq!(s.take_dropped(), gone.into_iter().collect::<Vec<_>>());
        assert_eq!(s.texture_bytes, 50);
        assert!(
            s.resident
                .iter()
                .any(|r| Some(r.id) == stays && r.pos == 11)
        );
        assert_eq!(s.session_bytes, 24 << 20);
        assert!(matches!(s.sessions.get(&k(11)), Some(Some(11))));
        // image 13 is now at position 12 and has no session of image 12
        assert!(!s.sessions.contains_key(&k(12)));
        assert_eq!(s.session_lru, vec![k(11)]);
        assert!(exact(&s));
    }

    #[test]
    fn after_set_files_the_plan_follows_the_new_order() {
        let mut s = state(1024);
        s.set_files(vec![path(5), path(10), path(17), path(3)], 1);
        assert_eq!(s.files.get(2), Some(&path(17)));
        use Want::*;
        assert_eq!(
            s.wanted(false),
            vec![
                (k(1), Shown),
                (k(1), Target),
                (k(2), Fit),
                (k(2), Target),
                (k(3), Fit),
                (k(3), Target),
                ((1, Source::Jpeg), Fit),
                (k(1), Ring),
                (k(0), Fit)
            ]
        );
        let plan = s.plan();
        assert_eq!(plan.first(), Some(&Job::Open(k(1))));
        assert!(plan.contains(&Job::Open(k(2))) && plan.contains(&Job::Open(k(3))));
        // +1 is the next path in the new order
        assert!(matches!(s.pick(true), Some(Task::Open { key, .. }) if key == k(1)));
        assert!(
            matches!(s.pick(false), Some(Task::Open { key, path: p, .. }) if key == k(2) && p == path(17))
        );
    }

    #[test]
    fn a_render_lent_before_set_files_is_discarded_and_leaves_the_new_key_alone() {
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        s.finish_open(s.generation, k(11), Ok((11, SIZE)), mb24, 1.0);
        let Some(Task::Render {
            generation,
            key,
            view,
            session,
        }) = s.pick(true)
        else {
            panic!("expected a render task");
        };
        assert_eq!(key, k(10));
        // image 3 drops out: image 11 is now at position 10, image 10 at 9
        s.set_files(without(20, &[3]), 9);
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(11))));
        assert!(exact(&s));
        let discarded = s.stats.discarded;
        let img = Rgb {
            width: 1,
            height: 1,
            data: vec![0; 3],
        };
        s.finish_render(generation, key, view, session, Ok(img), 1.0);
        assert!(s.outbox.is_empty());
        assert_eq!(s.stats.discarded, discarded + 1);
        // the session of image 11, now under the same key, is untouched
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(11))));
        assert!(!s.sessions.contains_key(&k(9)));
        assert_eq!(s.session_bytes, 24 << 20);
        assert!(s.detached.is_empty());
        assert!(exact(&s));
    }

    #[test]
    fn an_open_finished_after_set_files_is_discarded() {
        let mut s = state(1024);
        let Some(Task::Open {
            generation,
            key,
            path: p,
        }) = s.pick(true)
        else {
            panic!("expected an open task");
        };
        assert_eq!((key, p), (k(10), path(10)));
        s.set_files(without(20, &[3]), 9);
        s.finish_open(generation, key, Ok((10, SIZE)), mb24, 1.0);
        // nothing is stored under the old key, which now belongs to image 11
        assert!(s.sessions.is_empty());
        assert!(s.sizes.is_empty());
        assert_eq!(s.session_bytes, 0);
        // the image is opened again at its new position
        assert!(
            matches!(s.pick(true), Some(Task::Open { key, path: p, .. }) if key == k(9) && p == path(10))
        );
    }

    #[test]
    fn set_files_with_the_same_list_only_goes_to_the_current_image() {
        let mut s = state(1024);
        known(&mut s, 10);
        let id = s.accept(&out(&s, 10, Want::Fit), 100);
        let generation = s.generation;
        s.set_files(files(20), 12);
        assert_eq!(s.generation, generation);
        assert!(s.take_dropped().is_empty());
        assert_eq!(s.cur, Some(12));
        assert!(s.resident.iter().any(|r| Some(r.id) == id && r.pos == 10));
        assert_eq!(s.sizes.get(&k(10)), Some(&SIZE));
    }

    #[test]
    fn set_files_keeps_the_view_of_an_unchanged_current_image_and_the_zoom() {
        let mut s = state(1024);
        known(&mut s, 10);
        s.zoom = Zoom::Scale(1.0);
        s.centre = [0.3, 0.6];
        s.dir = -1;
        s.set_mode(Source::Raw);
        let id = s.accept(&out(&s, 10, Want::Shown), 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), id);
        s.set_files(without(20, &[0, 1]), 8);
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        assert_eq!(s.centre, [0.3, 0.6]);
        assert_eq!(s.dir, -1);
        assert_eq!(s.mode, Source::Raw);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), id);
        assert_eq!(f.pos, Some(8));
        assert_eq!(f.path, Some(path(10)));
    }

    #[test]
    fn set_files_with_an_empty_list_releases_everything() {
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        let id = s.accept(&out(&s, 10, Want::Fit), 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), id);
        s.set_files(Vec::new(), 0);
        assert_eq!(s.cur, None);
        assert_eq!(s.take_dropped(), id.into_iter().collect::<Vec<_>>());
        assert_eq!((s.session_bytes, s.texture_bytes), (0, 0));
        assert!(s.sessions.is_empty() && s.sizes.is_empty() && s.resident.is_empty());
        assert!(s.plan().is_empty());
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Empty);
        assert!(f.draw.is_none());
    }

    // ---------- releasing a file before a metadata write (ADR 0007 §9.1, T89) ----------

    /// `session_bytes` is the sum of the sessions held, the detached ones and the released ones
    /// still lent.
    fn exact_released(s: &State<u32>) -> bool {
        s.session_bytes
            == s.session_bytes_of.values().sum::<u64>()
                + s.detached.values().sum::<u64>()
                + s.released.values().sum::<u64>()
    }

    fn img() -> Rgb {
        Rgb {
            width: 1,
            height: 1,
            data: vec![0; 3],
        }
    }

    /// Opens image `pos` (the current one) in RAW mode and lends its session to a render.
    fn lend(s: &mut State<u32>, pos: usize) -> (u64, Key, JobView, u32) {
        s.finish_open(s.generation, k(pos), Ok((pos as u32, SIZE)), mb24, 1.0);
        match s.pick(true) {
            Some(Task::Render {
                generation,
                key,
                view,
                session,
            }) => (generation, key, view, session),
            other => panic!("expected a render task, got {other:?}"),
        }
    }

    #[test]
    fn release_drops_the_owned_sessions_of_the_path_and_keeps_its_views_and_other_sessions() {
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        s.finish_open(s.generation, (10, Source::Jpeg), Ok((10, SIZE)), mb24, 1.0);
        s.finish_open(s.generation, k(11), Ok((11, SIZE)), mb24, 1.0);
        let id = s.accept(&out(&s, 10, Want::Fit), 100);
        assert!(id.is_some());
        assert_eq!(s.session_bytes, 72 << 20);
        assert_eq!(s.release(&path(10)), (true, true));
        // both sessions of the path are gone and their bytes with them
        assert!(!s.sessions.contains_key(&k(10)));
        assert!(!s.sessions.contains_key(&(10, Source::Jpeg)));
        assert_eq!(s.session_bytes, 24 << 20);
        assert!(!s.session_lru.contains(&k(10)));
        assert!(exact_released(&s));
        // another path keeps its session
        assert!(matches!(s.sessions.get(&k(11)), Some(Some(11))));
        assert_eq!(s.session_lru, vec![k(11)]);
        // the views of the path are kept and still drawn
        assert!(s.take_dropped().is_empty());
        assert!(s.resident.iter().any(|r| Some(r.id) == id && r.pos == 10));
        assert_eq!(s.texture_bytes, 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), id);
        assert_eq!(s.sizes.get(&k(10)), Some(&SIZE));
    }

    #[test]
    fn release_of_a_path_the_core_does_not_know_changes_nothing() {
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        assert_eq!(s.release(std::path::Path::new("other.raw")), (false, true));
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert_eq!(s.session_bytes, 24 << 20);
        // compared as stored: another spelling of the path is another path
        assert_eq!(
            s.release(std::path::Path::new("./img10.raw")),
            (false, true)
        );
        assert_eq!(s.session_bytes, 24 << 20);
    }

    #[test]
    fn a_lent_session_is_dropped_when_it_comes_back_and_its_view_is_still_delivered() {
        let mut s = state(1024);
        let (generation, key, view, session) = lend(&mut s, 10);
        assert_eq!(key, k(10));
        assert_eq!(s.release(&path(10)), (true, false));
        // marked, still counted until it comes back
        assert!(!s.sessions.contains_key(&k(10)));
        assert_eq!(s.session_bytes, 24 << 20);
        assert!(exact_released(&s));
        // a second release while it is out finds nothing new to do and still waits
        assert_eq!(s.release(&path(10)), (false, false));
        s.finish_render(generation, key, view, session, Ok(img()), 1.0);
        assert_eq!(s.session_bytes, 0);
        assert!(!s.sessions.contains_key(&k(10)));
        assert!(s.released.is_empty() && s.lent.is_empty());
        assert_eq!(s.outbox.len(), 1);
        assert_eq!(s.release(&path(10)), (false, true));
    }

    #[test]
    fn a_released_session_coming_back_leaves_a_newer_session_of_the_same_key_alone() {
        let mut s = state(1024);
        let (generation, key, view, session) = lend(&mut s, 10);
        assert_eq!(s.release(&path(10)), (true, false));
        // the image is opened again after the release
        s.finish_open(s.generation, k(10), Ok((99, SIZE)), mb24, 1.0);
        assert_eq!(s.session_bytes, 48 << 20);
        s.finish_render(generation, key, view, session, Ok(img()), 1.0);
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(99))));
        assert_eq!(s.session_bytes, 24 << 20);
        assert!(exact_released(&s));
    }

    #[test]
    fn release_waits_for_a_session_lent_under_an_older_list_of_files() {
        let mut s = state(1024);
        let (generation, key, view, session) = lend(&mut s, 10);
        // image 3 drops out: image 10 is now at position 9; its lent session is detached
        s.set_files(without(20, &[3]), 9);
        assert_eq!(s.release(&path(10)), (false, false));
        assert_eq!(s.release(&path(11)), (false, true));
        s.finish_render(generation, key, view, session, Ok(img()), 1.0);
        assert_eq!(s.release(&path(10)), (false, true));
        assert_eq!(s.session_bytes, 0);
        // and after a new folder too
        let mut s = state(1024);
        let (generation, key, view, session) = lend(&mut s, 10);
        s.open_folder(files(20));
        assert_eq!(s.release(&path(10)), (false, false));
        s.finish_render(generation, key, view, session, Ok(img()), 1.0);
        assert_eq!(s.release(&path(10)), (false, true));
        assert!(s.lent_paths.is_empty());
    }

    #[test]
    fn a_file_being_released_is_not_opened_until_the_release_ends() {
        let mut s = state(1024);
        s.begin_release(&path(10));
        let picked: Vec<Key> = std::iter::from_fn(|| match s.pick(false) {
            Some(Task::Open { key, .. }) => Some(key),
            _ => None,
        })
        .collect();
        assert!(!picked.is_empty());
        assert!(!picked.contains(&k(10)));
        assert!(!picked.contains(&(10, Source::Jpeg)));
        s.end_release(&path(10));
        assert!(matches!(s.pick(true), Some(Task::Open { key, .. }) if key == k(10)));
    }

    // ---------- an embedded profile that cannot be used (ADR 0016 §5, T73) ----------

    /// A simulated backend's `profile_unusable`: true for the session of `img10.raw` only (the
    /// test sessions are the position the image was opened at in `files(20)`).
    fn unusable_for_img10(session: &u32) -> bool {
        files(20).get(*session as usize) == Some(&path(10))
    }

    fn open_checked(s: &mut State<u32>, pos: usize) {
        s.finish_open_checked(
            s.generation,
            k(pos),
            Ok((pos as u32, SIZE)),
            mb24,
            unusable_for_img10,
            1.0,
        );
    }

    #[test]
    fn the_frame_says_when_the_current_images_profile_cannot_be_used() {
        let mut s = state(1024);
        open_checked(&mut s, 10);
        open_checked(&mut s, 11);
        let f = s.frame(VIEW);
        assert_eq!(f.path, Some(path(10)));
        assert!(f.profile_unusable);
        s.go(11);
        assert!(!s.frame(VIEW).profile_unusable);
        s.go(12);
        assert!(!s.frame(VIEW).profile_unusable);
    }

    #[test]
    fn the_unusable_profile_flag_follows_the_path_through_set_files() {
        let mut s = state(1024);
        open_checked(&mut s, 10);
        open_checked(&mut s, 11);
        // image 3 drops out: img10 is now at position 9, img11 at 10
        s.set_files(without(20, &[3]), 9);
        let f = s.frame(VIEW);
        assert_eq!(f.path, Some(path(10)));
        assert!(f.profile_unusable);
        s.go(10);
        let f = s.frame(VIEW);
        assert_eq!(f.path, Some(path(11)));
        assert!(!f.profile_unusable);
        assert_eq!(s.profile_unusable, HashSet::from([k(9)]));
    }

    #[test]
    fn open_folder_clears_the_unusable_profile_flags() {
        let mut s = state(1024);
        open_checked(&mut s, 10);
        assert!(s.frame(VIEW).profile_unusable);
        s.open_folder(files(20));
        s.go(10);
        assert!(s.profile_unusable.is_empty());
        let f = s.frame(VIEW);
        assert_eq!(f.path, Some(path(10)));
        assert!(!f.profile_unusable);
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_ne!(fnv(b"a"), fnv(b"b"));
    }
}
