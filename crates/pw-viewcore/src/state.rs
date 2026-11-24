//! The rules of the view core, without threads (ADR 0008 §12, ADR 0009 §3.4, §5): what is wanted,
//! in which order, what may be evicted, and which resident view serves the screen. [`crate::ViewCore`]
//! runs them on two workers; the tests here drive them directly.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::backend::{
    ClippingMap, DecodeError, FocusMap, ImageStatistics, Rgb, Source, Statistics,
};
use crate::geometry::{self, ImageSize, Rect, RectF};

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
/// taken from the retired Tauri viewer's `zoomTo`.
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
    /// The whole image below 50 %: the half-size render reduced on the worker to the view's
    /// density with the Lanczos filter (ADR 0027 §3.2, W1-Q3).
    Reduced,
    /// The whole image below 50 % while fine details is on: the full-resolution render (PPG)
    /// reduced on the worker to the view's density with the Lanczos filter (ADR 0027 §3.1, W2).
    Fine,
}

impl ViewKind {
    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Half => "half",
            ViewKind::Zoom => "zoom",
            ViewKind::Ring => "ring",
            ViewKind::Scaled => "scaled",
            ViewKind::Reduced => "reduced",
            ViewKind::Fine => "fine",
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

/// The clipping map of the current image in its display mode, for the highlight and shadow
/// overlays (ADR 0017 §2.6, U7b).
#[derive(Clone, Debug, PartialEq)]
pub enum ClippingState {
    /// No overlay wants a map ([`crate::ViewCore::set_clipping_wanted`]); none is asked for.
    NotWanted,
    /// Wanted and not there yet: the map is being made, or the view on the screen is still an
    /// earlier image's.
    Reading,
    /// This file has none (its data is of no kind a map is made from, making it failed, the file
    /// cannot be shown, or there is no image).
    Unavailable,
    /// The map, to be drawn over the image of [`Frame::draw`].
    Ready {
        /// The same `Arc` for as long as the map is kept, so a new map is told from the one
        /// already uploaded with `Arc::ptr_eq`.
        map: Arc<ClippingMap>,
        /// The map's texture coordinates at the corners of the drawn image ([`Draw::dest`]): top
        /// left, top right, bottom right, bottom left, as [`Draw::uv`] (orientation, zoom and
        /// position are in these). 1.0 is the map's right or bottom edge, which may lie past the
        /// image's by less than a cell.
        uv: [[f32; 2]; 4],
    },
}

/// The focus map of the current image in its display mode, for the focus marking (ADR 0017 §2.6,
/// U9a). Given by the rules of [`ClippingState`].
#[derive(Clone, Debug, PartialEq)]
pub enum FocusState {
    /// The focus marking is off ([`crate::ViewCore::set_focus_wanted`]); no map is asked for.
    NotWanted,
    /// Wanted and not there yet: the map is being made, or the view on the screen is still an
    /// earlier image's.
    Reading,
    /// This file has none (its half-size render failed or is too small, the file cannot be
    /// shown, or there is no image).
    Unavailable,
    /// The map, to be drawn over the image of [`Frame::draw`].
    Ready {
        /// The same `Arc` for as long as the map is kept, so a new map is told from the one
        /// already uploaded with `Arc::ptr_eq`.
        map: Arc<FocusMap>,
        /// The map's texture coordinates at the corners of the drawn image ([`Draw::dest`]): top
        /// left, top right, bottom right, bottom left, as [`Draw::uv`] (orientation, zoom and
        /// position are in these). The map covers the whole image, as the half-size view does,
        /// so 1.0 is the image's right or bottom edge.
        uv: [[f32; 2]; 4],
    },
}

/// Why fine details does not apply to the current image (ADR 0027 W2-Q3, W2-Q6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FinePause {
    /// The current image is shown from its embedded JPEG, or is a JPEG or TIFF file.
    Mode,
    /// Its scale is 50 % or more.
    Zoom,
    /// Its fine view failed, was refused by the texture budget or rejected by the presentation,
    /// or the image cannot be shown.
    Failed,
}

/// The state of fine details for the current image (ADR 0027 §3.1, W2), for the shell's mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FineState {
    /// Fine details is off, or there is no image.
    Off,
    /// It applies, and the drawn view is not a fine view of the current image yet.
    Waiting,
    /// The frame's view is ready and is a fine view of the current image.
    Shown,
    /// It does not apply to the current image now.
    Paused(FinePause),
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
    /// The position of the view as the core holds it: the relative point of the upright image at
    /// the view's centre, 0–1 on each axis (V6). At fit, and where the image is smaller than the
    /// view, it is held but does not move the image.
    pub centre: [f64; 2],
    /// The current image's embedded colour profile was present but could not be used, so its
    /// colours rest on a fallback (ADR 0016 §5). Known once the image is opened.
    pub profile_unusable: bool,
    /// The histogram and the exposure statistics of the current image in its display mode (ADR
    /// 0017 §2.6, U7-Q6): `None` until they are ready. They are asked once per opened image,
    /// after its first view is on the screen.
    pub statistics: Option<ImageStatistics>,
    /// The clipping map of the current image in its display mode, when an overlay wants one
    /// (ADR 0017 §2.6, U7b).
    pub clipping: ClippingState,
    /// The focus map of the current image in its display mode, when the focus marking is on
    /// (ADR 0017 §2.6, U9a).
    pub focus: FocusState,
    /// The state of fine details for the current image (ADR 0027 §3.1, W2).
    pub fine: FineState,
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
    /// How many times an image's statistics were asked of the backend, and how long each took.
    pub statistics: u64,
    pub statistics_ms: Vec<f64>,
    /// How many times an image's clipping map was asked of the backend, and how long each took.
    pub clipping_maps: u64,
    pub clipping_ms: Vec<f64>,
    /// How many times an image's focus map was made, and how long each took.
    pub focus_maps: u64,
    pub focus_ms: Vec<f64>,
}

/// How many clipping maps are kept: those of the images shown last (ADR 0017 U7b-Q8).
pub(crate) const KEPT_CLIPPING_MAPS: usize = 4;

/// How many focus maps are kept: those of the images shown last (ADR 0017 U9a, as U7b-Q8).
pub(crate) const KEPT_FOCUS_MAPS: usize = 4;

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
    /// The fine view of the current image below 50 %, while fine details is on (ADR 0027 W2).
    Fine,
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

/// Whether a reduced view of `region` whose density is `density` (as [`State::finish_render`]
/// gives it: the smaller of its two sides' ratios) has the size `job` asks for. The density of a
/// view of the job's size lies between the job's density and the next size's, so the size is
/// found again from it; a margin far below one pixel keeps rounding from moving it.
fn same_reduction(region: Rect, density: f64, job: JobView) -> bool {
    use crate::scale::scaled_size;
    scaled_size(region.w, region.h, density - 1e-12)
        == scaled_size(region.w, region.h, job.density())
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
    /// The statistics of an opened image (ADR 0017 U7-Q6).
    Statistics(Key),
    /// The clipping map of an opened image (ADR 0017 U7b-Q8).
    Clipping(Key),
    /// The focus map of an opened image, from its half-size render (ADR 0017 U9a).
    Focus(Key),
}

impl Job {
    pub fn key(self) -> Key {
        match self {
            Job::Open(k)
            | Job::Render(k, _)
            | Job::Statistics(k)
            | Job::Clipping(k)
            | Job::Focus(k) => k,
        }
    }
}

/// What a session lent for its statistics, its clipping map or its focus map is booked under where
/// a lent session is booked by the view it renders (`lent_paths`, `released`): no view, which no
/// render job has. The three share it: an image's session is lent to one job at a time, and no new
/// session of the image is opened while a job of it runs, so no two of them are ever booked at
/// the same time.
const STATISTICS_LEND: JobView = JobView {
    kind: ViewKind::Half,
    region: Rect::new(0, 0, 0, 0),
    density_q: 0,
};

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
    Statistics {
        generation: u64,
        key: Key,
        session: S,
    },
    Clipping {
        generation: u64,
        key: Key,
        session: S,
    },
    Focus {
        generation: u64,
        key: Key,
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
    /// The backend's own flip of each opened image (for a RAW render LibRaw's flip), kept beside
    /// `sizes` so an orientation told and taken back again ([`State::set_orientation`]) brings it
    /// back without opening the file again. Inserted, cleared and moved where `sizes` is.
    own_flips: HashMap<Key, u8>,
    /// The orientations told by the shell, by path (ADR 0024 §2.4: an XMP orientation is used
    /// before the file's own). Not cleared by a new folder or list of files: the shell sends a
    /// value once (T232).
    orientations: HashMap<PathBuf, u16>,
    /// The opened images whose embedded colour profile could not be used (ADR 0016 §5). Kept,
    /// cleared and moved like `sizes`.
    pub profile_unusable: HashSet<Key>,
    /// The statistics of the opened images that were asked for them (ADR 0017 U7-Q6): once per
    /// image and display mode. Kept, cleared and moved like `sizes`.
    pub statistics: HashMap<Key, ImageStatistics>,
    /// An overlay wants the current image's clipping map (ADR 0017 U7b-Q8). Set by the shell;
    /// a new folder or list of files does not change it.
    pub clipping_wanted: bool,
    /// The clipping maps of the images shown last, the one shown longest ago first; at most
    /// [`KEPT_CLIPPING_MAPS`]. Cleared and moved like `statistics`.
    pub clipping_maps: Vec<(Key, Arc<ClippingMap>)>,
    /// The opened images that were asked for a clipping map and have none (or making it
    /// failed): not asked again. Cleared and moved like `statistics`.
    pub clipping_none: HashSet<Key>,
    /// The focus marking wants the current image's focus map (ADR 0017 U9a). Set by the shell; a
    /// new folder or list of files does not change it.
    pub focus_wanted: bool,
    /// The focus maps of the images shown last, the one shown longest ago first; at most
    /// [`KEPT_FOCUS_MAPS`]. Cleared and moved like `clipping_maps`.
    pub focus_maps: Vec<(Key, Arc<FocusMap>)>,
    /// The opened images that were asked for a focus map and have none (or making it failed):
    /// not asked again. Cleared and moved like `clipping_none`.
    pub focus_none: HashSet<Key>,
    /// Fine details is on (ADR 0027 §3.1, W2). Set by the shell, which holds it for the session
    /// (W2-Q4); a new folder or list of files does not change it.
    pub fine_details: bool,
    /// The images whose fine view failed, was refused by the texture budget or was rejected by
    /// the presentation (W2-Q6): their fine view is not asked again. Kept as `failed` is (moved
    /// by [`State::set_files`], emptied by a new folder).
    fine_failed: HashSet<Key>,
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
    /// The upright rectangle of the image that `last_draw` shows (set with it).
    last_vis: Option<RectF>,
    waiting_since: Option<Instant>,
    /// Whether [`State::frame`] has been called: the first frame's view size is not a change.
    framed: bool,
    /// The images, by the generation they were rendered in, whose half-size render came back
    /// smaller than a reduced view's size: they get the half-size view below 50 % (T309 R1).
    unreducible: HashSet<(u64, Key)>,
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
            own_flips: HashMap::new(),
            orientations: HashMap::new(),
            profile_unusable: HashSet::new(),
            statistics: HashMap::new(),
            clipping_wanted: false,
            clipping_maps: Vec::new(),
            clipping_none: HashSet::new(),
            focus_wanted: false,
            focus_maps: Vec::new(),
            focus_none: HashSet::new(),
            fine_details: false,
            fine_failed: HashSet::new(),
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
            last_vis: None,
            waiting_since: None,
            framed: false,
            unreducible: HashSet::new(),
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

    /// Sets the view model's zoom level and position from outside, as if the user had zoomed and
    /// dragged there (ADR 0017 U6: a second core that follows the first). Returns whether
    /// anything changed.
    ///
    /// `Zoom::Fit` is taken as it is. A scale that is not finite or not above 0, or a centre
    /// with a coordinate that is not finite, is ignored: nothing changes. A scale is clamped as
    /// [`State::set_zoom`] clamps it when the current image's size is known, and to
    /// [`MAX_SCALE`] only when it is not (the view model is not clamped again when the size
    /// becomes known). Each coordinate of the centre is clamped to 0–1 and stored as it is: it
    /// is not moved to the middle where the image fits the view; [`geometry::visible`] keeps the
    /// drawn region inside the image.
    ///
    /// A change counts as one step of a zoom gesture: the views that depend on the position are
    /// asked for only when the calls pause (`Config::gesture_pause`). Values equal to what is
    /// held (after the rules above) change nothing and start no new step, so it may be called
    /// every frame.
    pub fn set_view(&mut self, zoom: Zoom, centre: [f64; 2]) -> bool {
        let zoom = match zoom {
            Zoom::Fit => Zoom::Fit,
            Zoom::Scale(s) if s.is_finite() && s > 0.0 => {
                let size = self.cur_key().and_then(|k| self.sizes.get(&k).copied());
                let s = match size {
                    Some(size) => s.max(self.min_scale(Some(size))),
                    None => s,
                };
                Zoom::Scale(s.min(MAX_SCALE))
            }
            Zoom::Scale(_) => return false,
        };
        if !(centre[0].is_finite() && centre[1].is_finite()) {
            return false;
        }
        let centre = [centre[0].clamp(0.0, 1.0), centre[1].clamp(0.0, 1.0)];
        if zoom == self.zoom && centre == self.centre {
            return false;
        }
        self.zoom = zoom;
        self.centre = centre;
        self.gesture_at = Some(Instant::now());
        true
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
        self.own_flips.clear();
        self.profile_unusable.clear();
        self.statistics.clear();
        self.clipping_maps.clear();
        self.clipping_none.clear();
        self.focus_maps.clear();
        self.focus_none.clear();
        self.failed.clear();
        self.fine_failed.clear();
        self.inflight.clear();
        self.outbox.clear();
        self.dropped.extend(self.resident.drain(..).map(|r| r.id));
        self.texture_bytes = 0;
        self.last_draw = None;
        self.last_vis = None;
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
        self.own_flips = self
            .own_flips
            .drain()
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        self.profile_unusable = self.profile_unusable.drain().filter_map(to).collect();
        self.statistics = self
            .statistics
            .drain()
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        // in the same order: the one shown longest ago stays first
        self.clipping_maps = self
            .clipping_maps
            .drain(..)
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        self.clipping_none = self.clipping_none.drain().filter_map(to).collect();
        self.focus_maps = self
            .focus_maps
            .drain(..)
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        self.focus_none = self.focus_none.drain().filter_map(to).collect();
        self.failed = self
            .failed
            .drain()
            .filter_map(|(k, v)| to(k).map(|n| (n, v)))
            .collect();
        self.fine_failed = self.fine_failed.drain().filter_map(to).collect();
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
            self.last_vis = None;
        }

        if self.files.is_empty() {
            self.cur = None;
            self.last_draw = None;
            self.last_vis = None;
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
        // free levels of 50 % or more are not prefetched: neighbours get the fit view and the
        // 100 % target. Below 50 % the view at the current scale is of the whole image, so it is
        // prefetched instead of the fit view, and stepping at such a level stays quick (ADR 0027
        // W1-Q8).
        let (primary, other) = match self.zoom {
            Zoom::Scale(s) if s >= 1.0 - EPS => (Want::Target, Want::Fit),
            Zoom::Scale(s) if s < HALF_DENSITY - EPS => (Want::Shown, Want::Target),
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
        // the fine view last, after everything else (ADR 0027 W2-Q2)
        if self.fine_applies(ck) {
            add((ck, Want::Fine));
        }
        v
    }

    /// Whether fine details applies to `key` (ADR 0027 W2-Q3): it is on, `key` is the current
    /// image in RAW (never in JPEG mode, never a JPEG or TIFF file), its size is known, its scale
    /// is below 50 %, and its fine view has not failed (W2-Q6).
    fn fine_applies(&self, key: Key) -> bool {
        self.fine_details
            && self.cur_key() == Some(key)
            && key.1 == Source::Raw
            && self.scale_of(key).is_some_and(|s| s < HALF_DENSITY - EPS)
            && !self.fine_failed.contains(&key)
    }

    /// The concrete requirement for a want, once the image's size is known.
    pub fn req(&self, key: Key, want: Want) -> Option<Req> {
        let size = *self.sizes.get(&key)?;
        if !size.is_valid() {
            return None;
        }
        let scale = match want {
            Want::Shown => self.scale_of(key)?,
            Want::Fine => {
                // the whole image, reduced from the full-resolution render to the current scale;
                // it covers what the shown view covers (ADR 0027 W2)
                if !self.fine_applies(key) {
                    return None;
                }
                let scale = self.scale_of(key)?;
                let vis = geometry::visible(size, self.view_px, scale, self.centre);
                return Some(Req {
                    key,
                    cover: geometry::sensor_box(size, vis),
                    density: scale,
                    job: JobView {
                        kind: ViewKind::Fine,
                        region: geometry::whole(size),
                        density_q: quantise(scale),
                    },
                });
            }
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
        // Below 50 % the half-size render reduced to the exact scale (ADR 0027 W1-Q3), except
        // for the shown view during a zoom gesture, which uses the half-size view as it is, so a
        // gesture never starts a render per frame. At 50 % the half-size view is drawn at 1:1, and
        // so it is just below, where the reduced size would not fit in a half-size render that
        // rounds down (an odd side; less than a pixel apart; owner 2026-11-24, T309).
        let reduced_fits = {
            let (w, h) = crate::scale::scaled_size(size.w, size.h, quantise(scale) as f64 / 1e6);
            w <= size.w / 2 && h <= size.h / 2
        } && !self.unreducible.contains(&(self.generation, key));
        let job = if scale < HALF_DENSITY - EPS
            && reduced_fits
            && !(want == Want::Shown && self.in_gesture())
        {
            JobView {
                kind: ViewKind::Reduced,
                region: geometry::whole(size),
                density_q: quantise(scale),
            }
        } else if scale <= HALF_DENSITY + EPS {
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

    /// Whether a view of `kind` over `region` at `density` serves `req`. A reduced view has no
    /// mipmaps, so it does not serve a requirement below half its own density: the GPU would
    /// reduce it more than 2:1 (aliasing); the mipmapped half-size view is used there instead
    /// (ADR 0027 W1, T309 R2). A fine view is the same, and serves nothing while fine details is
    /// off (ADR 0027 W2-Q8).
    fn serves(
        &self,
        generation: u64,
        key: Key,
        kind: ViewKind,
        region: Rect,
        density: f64,
        req: &Req,
    ) -> bool {
        let need = req.density.min(1.0);
        let reduced = matches!(kind, ViewKind::Reduced | ViewKind::Fine);
        generation == self.generation
            && key == req.key
            && !(kind == ViewKind::Fine && !self.fine_details)
            && region.contains(req.cover)
            && density + EPS >= need
            && !(reduced && density > 2.0 * need + EPS)
    }

    /// The resident view that serves a requirement: a fine view first while fine details is on
    /// (ADR 0027 W2-Q8: it replaces the reduced view in one swap, and an earlier fine view that
    /// still serves stays until the new one replaces it), then the least dense one, then the
    /// smallest.
    pub fn serving(&self, req: &Req) -> Option<&Resident> {
        self.serving_of(req, true)
    }

    /// [`State::serving`]; with `fine` false a fine view is not considered.
    fn serving_of(&self, req: &Req, fine: bool) -> Option<&Resident> {
        self.resident
            .iter()
            .filter(|r| fine || r.kind != ViewKind::Fine)
            .filter(|r| {
                self.serves(
                    r.generation,
                    (r.pos, r.source),
                    r.kind,
                    r.region,
                    r.density,
                    req,
                )
            })
            .min_by(|a, b| {
                (a.kind != ViewKind::Fine)
                    .cmp(&(b.kind != ViewKind::Fine))
                    .then(a.density.total_cmp(&b.density))
                    .then(a.region.area().cmp(&b.region.area()))
            })
    }

    /// The resident view that is exactly what a reduced or fine requirement asks for: of its kind
    /// and region, at its size.
    fn exact(&self, req: &Req) -> Option<&Resident> {
        let job = req.job;
        self.resident.iter().find(|r| {
            r.generation == self.generation
                && (r.pos, r.source) == req.key
                && r.kind == job.kind
                && r.region == job.region
                && same_reduction(r.region, r.density, job)
        })
    }

    /// Whether the plan counts a requirement as done. A reduced view (ADR 0027 W1-Q3) and a fine
    /// view (W2) count only as themselves, resident or in flight: a denser view (the half-size
    /// view after a gesture, a reduced view after zooming out) is drawn meanwhile
    /// ([`State::serving`]) but does not stop the exact one from being made. A fine view
    /// satisfies no other requirement, so the shown view is still made while a fine render runs.
    fn satisfied(&self, req: &Req) -> bool {
        if matches!(req.job.kind, ViewKind::Reduced | ViewKind::Fine) {
            let job = req.job;
            return self.exact(req).is_some()
                || self
                    .inflight
                    .iter()
                    .any(|f| f.generation == self.generation && f.key == req.key && f.view == job);
        }
        self.serving_of(req, false).is_some()
            || self.inflight.iter().any(|f| {
                f.view.kind != ViewKind::Fine
                    && self.serves(
                        f.generation,
                        f.key,
                        f.view.kind,
                        f.view.region,
                        f.view.density(),
                        req,
                    )
            })
    }

    /// The jobs the plan asks for now, in order.
    pub fn plan(&self) -> Vec<Job> {
        self.plan_of(true)
    }

    /// [`State::plan`]; with `fine` false without the fine view's want (so [`State::pick`] can
    /// tell an open asked only for the fine view).
    fn plan_of(&self, fine: bool) -> Vec<Job> {
        let mut v = Vec::new();
        for (key, want) in self.wanted(false) {
            if !fine && want == Want::Fine {
                continue;
            }
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

    /// The job for the current image's statistics, when it is due (ADR 0017 U7-Q6): they are
    /// asked once per opened image, only after its view is on the screen (so the first image is
    /// never delayed), after the dwell and not during a zoom gesture (so stepping through images
    /// starts none). An image whose session was closed meanwhile is opened again first.
    ///
    /// The 100 % view first (ADR 0017 U7c-Q5, T144): while the plan wants a view of the current
    /// image at 100 % or more (the shown view when zoomed in, or the 100 % view prefetched at
    /// fit) that is not on the screen yet, the statistics wait, so Z right then does not wait for
    /// them.
    fn statistics_job(&self) -> Option<Job> {
        let key = self.cur_key()?;
        if self.statistics.contains_key(&key) || self.failed.contains_key(&key) {
            return None;
        }
        if self.in_gesture() || self.since.elapsed() < self.config.dwell {
            return None;
        }
        let shown = self.req(key, Want::Shown)?;
        self.serving(&shown)?;
        if self.wants_a_full_size_view(key) {
            return None;
        }
        Some(if self.sessions.contains_key(&key) {
            Job::Statistics(key)
        } else {
            Job::Open(key)
        })
    }

    /// The job for the current image's clipping map, when it is due (ADR 0017 U7b-Q8): only
    /// while an overlay wants a map, once per image and display mode, never for another image
    /// (no prefetch). It is due when the image's statistics are (after its view is on the
    /// screen, after the dwell, not during a zoom gesture, and after a wanted 100 % view;
    /// U7-Q6, U7c-Q5), and it comes after them: [`State::pick`] puts it behind the statistics'
    /// job, and it waits while that job of the same image runs. A map that was kept and has been
    /// dropped since (more than [`KEPT_CLIPPING_MAPS`] images shown) is asked again; an image
    /// that has none is not.
    fn clipping_job(&self) -> Option<Job> {
        if !self.clipping_wanted {
            return None;
        }
        let key = self.cur_key()?;
        if self.clipping_map(key).is_some()
            || self.clipping_none.contains(&key)
            || self.failed.contains_key(&key)
        {
            return None;
        }
        if self
            .running
            .contains(&(self.generation, Job::Statistics(key)))
        {
            return None;
        }
        if self.in_gesture() || self.since.elapsed() < self.config.dwell {
            return None;
        }
        let shown = self.req(key, Want::Shown)?;
        self.serving(&shown)?;
        if self.wants_a_full_size_view(key) {
            return None;
        }
        Some(if self.sessions.contains_key(&key) {
            Job::Clipping(key)
        } else {
            Job::Open(key)
        })
    }

    /// The kept clipping map of `key`.
    fn clipping_map(&self, key: Key) -> Option<&Arc<ClippingMap>> {
        self.clipping_maps
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, map)| map)
    }

    /// Keeps `map` as the map of the image shown last; of more than [`KEPT_CLIPPING_MAPS`], the
    /// one shown longest ago is dropped (U7b-Q8).
    fn keep_clipping_map(&mut self, key: Key, map: Arc<ClippingMap>) {
        self.clipping_maps.retain(|(k, _)| *k != key);
        self.clipping_maps.push((key, map));
        let over = self.clipping_maps.len().saturating_sub(KEPT_CLIPPING_MAPS);
        self.clipping_maps.drain(..over);
    }

    /// The job for the current image's focus map, when it is due (ADR 0017 U9a): by the rules of
    /// [`State::clipping_job`] (only while the focus marking wants a map, once per image and
    /// display mode, never for another image, when the image's statistics are due), and after
    /// both: [`State::pick`] puts it behind the statistics' and the clipping job, and it waits
    /// while either of them runs for the same image. A map that was kept and has been dropped since
    /// (more than [`KEPT_FOCUS_MAPS`] images shown) is asked again; an image that has none is not.
    fn focus_job(&self) -> Option<Job> {
        if !self.focus_wanted {
            return None;
        }
        let key = self.cur_key()?;
        if self.focus_map(key).is_some()
            || self.focus_none.contains(&key)
            || self.failed.contains_key(&key)
        {
            return None;
        }
        if [Job::Statistics(key), Job::Clipping(key)]
            .into_iter()
            .any(|job| self.running.contains(&(self.generation, job)))
        {
            return None;
        }
        if self.in_gesture() || self.since.elapsed() < self.config.dwell {
            return None;
        }
        let shown = self.req(key, Want::Shown)?;
        self.serving(&shown)?;
        if self.wants_a_full_size_view(key) {
            return None;
        }
        Some(if self.sessions.contains_key(&key) {
            Job::Focus(key)
        } else {
            Job::Open(key)
        })
    }

    /// The kept focus map of `key`.
    fn focus_map(&self, key: Key) -> Option<&Arc<FocusMap>> {
        self.focus_maps
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, map)| map)
    }

    /// Keeps `map` as the focus map of the image shown last; of more than [`KEPT_FOCUS_MAPS`],
    /// the one shown longest ago is dropped.
    fn keep_focus_map(&mut self, key: Key, map: Arc<FocusMap>) {
        self.focus_maps.retain(|(k, _)| *k != key);
        self.focus_maps.push((key, map));
        let over = self.focus_maps.len().saturating_sub(KEPT_FOCUS_MAPS);
        self.focus_maps.drain(..over);
    }

    /// Whether the plan wants a view of `key` at 100 % or more (a 100 % region) that no resident
    /// view serves yet (U7c-Q5).
    fn wants_a_full_size_view(&self, key: Key) -> bool {
        self.wanted(false).into_iter().any(|(k, want)| {
            k == key
                && self.req(k, want).is_some_and(|req| {
                    req.job.kind == ViewKind::Zoom && self.serving(&req).is_none()
                })
        })
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
    /// screen. While fine details is on, the current image's fine view and its exact reduced
    /// view are both kept, so neither is evicted for the other (ADR 0027 W2-Q8); a fine view of
    /// another image is not protected.
    fn protected_views(&self) -> HashSet<u64> {
        let mut p: HashSet<u64> = HashSet::new();
        let ck = self.cur_key();
        for (key, want) in self.wanted(true) {
            let Some(req) = self.req(key, want) else {
                continue;
            };
            if let Some(r) = self.serving_of(&req, want == Want::Fine) {
                p.insert(r.id);
            }
            if self.fine_details
                && want == Want::Shown
                && Some(key) == ck
                && req.job.kind == ViewKind::Reduced
                && let Some(r) = self.exact(&req)
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
            ViewKind::Scaled | ViewKind::Reduced | ViewKind::Fine => {
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

    // ---------- an orientation told by the shell (ADR 0024 §2.4) ----------

    /// The flip of the orientation told for the path of `key`, if one was told.
    fn told_flip(&self, key: Key) -> Option<u8> {
        let path = self.files.get(key.0)?;
        self.orientations
            .get(path)
            .map(|o| geometry::flip_of_orientation(*o))
    }

    /// Tells the orientation of `path` (ADR 0024 §2.4): `Some` value in `1..=8` is used before the
    /// file's own; `None` (or a value outside `1..=8`) takes it back, and the file's own applies
    /// again. Every opened image of `path` (both sources) gets the new flip; no view is released
    /// and no render in flight is dropped, since views are kept in sensor orientation and the flip
    /// is applied only when drawing. When the current image's flip changes at a scale, the centre
    /// is turned with the image, so the same image point stays in the centre (ADR 0024 §2.5), and
    /// the scale is raised to the turned size's minimum when it is below it; at fit the zoom and
    /// the centre are kept and the scale follows the turned size. This holds for every flip change
    /// of the current image, whatever told it. Returns whether a flip changed.
    pub fn set_orientation(&mut self, path: &Path, orientation: Option<u16>) -> bool {
        let cur = self
            .cur_key()
            .and_then(|k| self.sizes.get(&k).copied().map(|size| (k, size)));
        match orientation.filter(|o| (1..=8).contains(o)) {
            Some(o) => {
                self.orientations.insert(path.to_path_buf(), o);
            }
            None => {
                self.orientations.remove(path);
            }
        }
        let keys: Vec<Key> = self
            .sizes
            .keys()
            .filter(|k| self.files.get(k.0).is_some_and(|p| p.as_path() == path))
            .copied()
            .collect();
        let mut changed = false;
        for key in keys {
            let own = self.own_flips.get(&key).copied();
            let told = self.told_flip(key);
            if let Some(size) = self.sizes.get_mut(&key) {
                let flip = told.or(own).unwrap_or(size.flip);
                if size.flip != flip {
                    size.flip = flip;
                    changed = true;
                }
            }
        }
        if let Some((key, old_size)) = cur {
            self.turn_view_with_image(key, old_size);
        }
        changed
    }

    /// After a flip change of the current image `key`, whose size with the old flip was
    /// `old_size`: at a scale, the point drawn in the middle of the view before the turn is
    /// turned with the image and becomes the centre, and the scale is raised to the turned size's
    /// minimum (ADR 0024 §2.5). At fit, or when the flip did not change, nothing changes.
    fn turn_view_with_image(&mut self, key: Key, old_size: ImageSize) {
        let Some(new_size) = self.sizes.get(&key).copied() else {
            return;
        };
        let Zoom::Scale(s) = self.zoom else {
            return;
        };
        if new_size.flip == old_size.flip {
            return;
        }
        let old = geometry::quarter_turns_of_flip(old_size.flip);
        let new = geometry::quarter_turns_of_flip(new_size.flip);
        let turns = (4 + new - old) % 4;
        let drawn = geometry::centre_of(
            old_size,
            geometry::visible(old_size, self.view_px, s, self.centre),
        );
        let c = geometry::turned_centre(drawn, turns);
        self.centre = [c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0)];
        self.zoom = Zoom::Scale(s.max(self.min_scale(Some(new_size))).min(MAX_SCALE));
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
        let mut jobs = self.plan();
        // The statistics come first once they are due: the shown view and a wanted 100 % view of
        // the current image are on the screen by then (U7c-Q5).
        let mut first = 0;
        if let Some(job) = self.statistics_job()
            && !jobs.contains(&job)
        {
            jobs.insert(0, job);
            first = 1;
        }
        // The clipping map, when an overlay wants it, right after them (U7b-Q8).
        if let Some(job) = self.clipping_job()
            && !jobs.contains(&job)
        {
            jobs.insert(first, job);
            first += 1;
        }
        // The focus map, when the focus marking wants it, after both (U9a).
        let focus_job = self.focus_job();
        if let Some(job) = focus_job
            && !jobs.contains(&job)
        {
            jobs.insert(first, job);
        }
        // the opens asked for something other than the fine view: the interactive worker takes
        // no open asked only for it (ADR 0027 W2-Q2)
        let not_only_fine: Vec<Job> = if interactive {
            let mut v = self.plan_of(false);
            v.extend(self.statistics_job());
            v.extend(self.clipping_job());
            v.extend(focus_job);
            v
        } else {
            Vec::new()
        };
        for job in jobs {
            let key = job.key();
            if self.running.contains(&(generation, job)) || (interactive && Some(key) != ck) {
                continue;
            }
            let is_cur = Some(key) == ck;
            match job {
                Job::Open(key) => {
                    if interactive && !not_only_fine.contains(&job) {
                        continue;
                    }
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
                Job::Render(key, view) if view.kind == ViewKind::Fine => {
                    // the fine view (ADR 0027 W2-Q2, W2-Q6): only on the background worker, never
                    // while its file is being released for a metadata write, and refused when it
                    // does not fit, also for the current image; a refusal pauses fine details for
                    // the image (it is not the shown view's miss)
                    if interactive
                        || self
                            .files
                            .get(key.0)
                            .is_some_and(|p| self.releasing.contains(p))
                    {
                        continue;
                    }
                    let bytes = self.estimate_bytes(key, view);
                    if !self.textures_fit(bytes) {
                        self.stats.refused += 1;
                        self.fine_failed.insert(key);
                        continue;
                    }
                    if let Some(task) = self.lend_render(key, view, bytes) {
                        return Some(task);
                    }
                }
                Job::Render(key, view) => {
                    let bytes = self.estimate_bytes(key, view);
                    if !is_cur && !self.textures_fit(bytes) {
                        self.stats.refused += 1;
                        self.refused.insert((self.generation, key));
                        continue;
                    }
                    if let Some(task) = self.lend_render(key, view, bytes) {
                        return Some(task);
                    }
                }
                Job::Statistics(key) => {
                    let Some(slot) = self.sessions.get_mut(&key) else {
                        continue;
                    };
                    let Some(session) = slot.take() else {
                        continue;
                    };
                    if let Some(path) = self.files.get(key.0) {
                        self.lent_paths
                            .insert((generation, key, STATISTICS_LEND), path.clone());
                    }
                    self.lent.insert(key, generation);
                    self.running.insert((generation, job));
                    return Some(Task::Statistics {
                        generation,
                        key,
                        session,
                    });
                }
                Job::Clipping(key) => {
                    let Some(slot) = self.sessions.get_mut(&key) else {
                        continue;
                    };
                    let Some(session) = slot.take() else {
                        continue;
                    };
                    if let Some(path) = self.files.get(key.0) {
                        self.lent_paths
                            .insert((generation, key, STATISTICS_LEND), path.clone());
                    }
                    self.lent.insert(key, generation);
                    self.running.insert((generation, job));
                    return Some(Task::Clipping {
                        generation,
                        key,
                        session,
                    });
                }
                Job::Focus(key) => {
                    let Some(slot) = self.sessions.get_mut(&key) else {
                        continue;
                    };
                    let Some(session) = slot.take() else {
                        continue;
                    };
                    if let Some(path) = self.files.get(key.0) {
                        self.lent_paths
                            .insert((generation, key, STATISTICS_LEND), path.clone());
                    }
                    self.lent.insert(key, generation);
                    self.running.insert((generation, job));
                    return Some(Task::Focus {
                        generation,
                        key,
                        session,
                    });
                }
            }
        }
        None
    }

    /// Lends the session of `key` for a render of `view` (`bytes` its estimate), or `None` when
    /// the session is not there or is lent.
    fn lend_render(&mut self, key: Key, view: JobView, bytes: u64) -> Option<Task<S>> {
        let generation = self.generation;
        let session = self.sessions.get_mut(&key)?.take()?;
        if let Some(path) = self.files.get(key.0) {
            self.lent_paths
                .insert((generation, key, view), path.clone());
        }
        self.lent.insert(key, generation);
        self.running.insert((generation, Job::Render(key, view)));
        self.inflight.push(Inflight {
            generation,
            key,
            view,
            bytes,
        });
        Some(Task::Render {
            generation,
            key,
            view,
            session,
        })
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
                self.own_flips.insert(key, size.flip);
                let flip = self.told_flip(key).unwrap_or(size.flip);
                self.sizes.insert(key, ImageSize { flip, ..size });
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
        self.stats.renders += 1;
        self.stats
            .render_ms
            .entry(view.kind.name())
            .or_default()
            .push(ms);
        let released = self.released.contains_key(&(generation, key, view));
        if !self.return_session(generation, key, view, session) {
            self.stats.discarded += 1;
            return;
        }
        match result {
            Ok(image) => {
                let hash = self.config.record.then(|| fnv(&image.data));
                let target =
                    crate::scale::scaled_size(view.region.w, view.region.h, view.density());
                if view.kind == ViewKind::Reduced && (image.width, image.height) != target {
                    // The half-size render was smaller than the reduced size and came back as it
                    // is (`core.rs`): it is the image's half-size view, and the image gets the
                    // half-size view below 50 % from now on, so the plan counts it as done and
                    // does not render it again (T309 R1). Its job stays in flight as the
                    // half-size view it now is, until that view is accepted, so the plan does not
                    // ask for one meanwhile.
                    let half = JobView {
                        kind: ViewKind::Half,
                        region: view.region,
                        density_q: quantise(HALF_DENSITY),
                    };
                    if let Some(f) = self
                        .inflight
                        .iter_mut()
                        .find(|f| f.generation == generation && f.key == key && f.view == view)
                    {
                        f.view = half;
                    }
                    self.unreducible.retain(|(g, _)| *g == generation);
                    self.unreducible.insert((generation, key));
                    self.outbox.push(ViewOut {
                        generation,
                        pos: key.0,
                        source: key.1,
                        kind: ViewKind::Half,
                        region: view.region,
                        density: HALF_DENSITY,
                        image,
                        hash,
                    });
                    return;
                }
                let density = match view.kind {
                    ViewKind::Half => HALF_DENSITY,
                    ViewKind::Zoom | ViewKind::Ring => 1.0,
                    ViewKind::Scaled | ViewKind::Reduced | ViewKind::Fine => {
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
            Err(_) if view.kind == ViewKind::Fine => {
                // no error is shown: the reduced view stays, and fine details is paused for the
                // image (ADR 0027 W2-Q6). A failed LibRaw call ends the session, so it is dropped
                // (unless it was released and is gone already); the plan opens a new one when
                // one is needed.
                self.remove_inflight(generation, key, view.kind, view.region, view.density());
                self.stats.failures += 1;
                self.fine_failed.insert(key);
                if !released {
                    self.drop_session(key);
                }
            }
            Err(e) => {
                self.remove_inflight(generation, key, view.kind, view.region, view.density());
                self.stats.failures += 1;
                self.failed.insert(key, e);
            }
        }
    }

    /// A session lent in `generation` for `view` (or [`STATISTICS_LEND`]) comes back: it returns
    /// to its slot, or is dropped if [`State::release`] marked it or the list of files changed
    /// meanwhile. Returns whether the work it was lent for belongs to the current list of files.
    fn return_session(&mut self, generation: u64, key: Key, view: JobView, session: S) -> bool {
        self.lent_paths.remove(&(generation, key, view));
        if let Some(bytes) = self.released.remove(&(generation, key, view)) {
            // released for a metadata write: the session is dropped and its slot (gone, or
            // holding a newer session) is left alone; the view itself is still good
            self.session_bytes = self.session_bytes.saturating_sub(bytes);
            drop(session);
            generation == self.generation
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
            false
        } else {
            self.lent.remove(&key);
            if let Some(slot) = self.sessions.get_mut(&key) {
                *slot = Some(session);
            }
            self.session_lru.retain(|x| *x != key);
            self.session_lru.push(key);
            true
        }
    }

    /// The statistics of an opened image were computed (`ms` = how long it took; `None` = the
    /// file has none). The session comes back as after a render; the result is kept for the
    /// image, so it is not asked again, unless the list of files changed meanwhile.
    pub fn finish_statistics(
        &mut self,
        generation: u64,
        key: Key,
        session: S,
        result: Option<Statistics>,
        ms: f64,
    ) {
        self.running.remove(&(generation, Job::Statistics(key)));
        self.stats.statistics += 1;
        self.stats.statistics_ms.push(ms);
        if !self.return_session(generation, key, STATISTICS_LEND, session) {
            return;
        }
        let statistics = match result {
            Some(statistics) => ImageStatistics::Available(Arc::new(statistics)),
            None => ImageStatistics::Unavailable,
        };
        self.statistics.insert(key, statistics);
    }

    /// The clipping map of an opened image was made (`ms` = how long it took; `None` = the file
    /// has none, or making it failed). The session comes back as after a render; the map is kept
    /// as the newest of the kept ones, and "none" is remembered so it is not asked again, unless
    /// the list of files changed meanwhile.
    pub fn finish_clipping(
        &mut self,
        generation: u64,
        key: Key,
        session: S,
        result: Option<ClippingMap>,
        ms: f64,
    ) {
        self.running.remove(&(generation, Job::Clipping(key)));
        self.stats.clipping_maps += 1;
        self.stats.clipping_ms.push(ms);
        if !self.return_session(generation, key, STATISTICS_LEND, session) {
            return;
        }
        match result {
            Some(map) => self.keep_clipping_map(key, Arc::new(map)),
            None => {
                self.clipping_none.insert(key);
            }
        }
    }

    /// The focus map of an opened image was made (`ms` = how long it took; `None` = its half-size
    /// render failed or gave no map). The session comes back as after a render; the map is kept
    /// as the newest of the kept ones, and "none" is remembered so it is not asked again, unless
    /// the list of files changed meanwhile.
    pub fn finish_focus(
        &mut self,
        generation: u64,
        key: Key,
        session: S,
        result: Option<FocusMap>,
        ms: f64,
    ) {
        self.running.remove(&(generation, Job::Focus(key)));
        self.stats.focus_maps += 1;
        self.stats.focus_ms.push(ms);
        if !self.return_session(generation, key, STATISTICS_LEND, session) {
            return;
        }
        match result {
            Some(map) => self.keep_focus_map(key, Arc::new(map)),
            None => {
                self.focus_none.insert(key);
            }
        }
    }

    /// Removes the in-flight view of `kind` over `region`. Two reduced (or fine) views of one
    /// image differ only by their density, so for them it must match too (`density` as the view
    /// has it).
    fn remove_inflight(
        &mut self,
        generation: u64,
        key: Key,
        kind: ViewKind,
        region: Rect,
        density: f64,
    ) {
        if let Some(i) = self.inflight.iter().position(|f| {
            f.generation == generation
                && f.key == key
                && f.view.kind == kind
                && f.view.region == region
                && (!matches!(kind, ViewKind::Reduced | ViewKind::Fine)
                    || same_reduction(region, density, f.view))
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
        self.remove_inflight(out.generation, key, out.kind, out.region, out.density);
        if out.generation != self.generation {
            self.stats.discarded += 1;
            return None;
        }
        if out.kind == ViewKind::Fine {
            // kept while fine details is on, also when its image is no longer current (stepping
            // back shows it at once); dropped while it is off (ADR 0027 W2-Q8)
            if !self.fine_details {
                self.stats.discarded += 1;
                return None;
            }
        } else if matches!(out.kind, ViewKind::Scaled | ViewKind::Reduced) {
            let wanted = self.wanted(true).into_iter().any(|(k, want)| {
                self.req(k, want).is_some_and(|req| {
                    self.serves(out.generation, key, out.kind, out.region, out.density, &req)
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
    /// as failed in that mode with `reason`; for a fine view only fine details is paused for the
    /// image, and no error is shown (ADR 0027 W2-Q6).
    pub fn reject(&mut self, out: &ViewOut, reason: &str) {
        let key = (out.pos, out.source);
        self.remove_inflight(out.generation, key, out.kind, out.region, out.density);
        self.stats.discarded += 1;
        if out.generation == self.generation {
            if out.kind == ViewKind::Fine {
                self.fine_failed.insert(key);
            } else {
                self.failed.insert(key, DecodeError::new(reason));
            }
        }
    }

    /// Texture ids the presentation must release.
    pub fn take_dropped(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.dropped)
    }

    /// What to draw now, for a view of `view_px` physical pixels.
    pub fn frame(&mut self, view_px: [u32; 2]) -> Frame {
        let view_px = [view_px[0].max(1), view_px[1].max(1)];
        // A changed view size (the window resized, a panel shown or hidden) changes the fit
        // scale every frame: it counts as a step of a zoom gesture, so the reduced view is asked
        // for once it pauses, not once per size (ADR 0027 W1). The first frame's size is not a
        // change.
        if self.framed && view_px != self.view_px {
            self.gesture_at = Some(Instant::now());
        }
        self.framed = true;
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
            centre: self.centre,
            profile_unusable: key.is_some_and(|k| self.profile_unusable.contains(&k)),
            statistics: key.and_then(|k| self.statistics.get(&k).cloned()),
            // no image, or one that cannot be shown, has no map
            clipping: if self.clipping_wanted {
                ClippingState::Unavailable
            } else {
                ClippingState::NotWanted
            },
            focus: if self.focus_wanted {
                FocusState::Unavailable
            } else {
                FocusState::NotWanted
            },
            fine: FineState::Off,
        };
        let Some(key) = key else {
            self.last_draw = None;
            self.last_vis = None;
            return frame;
        };
        if let Some(e) = self.failed.get(&key) {
            // G3: an unreadable file never leaves the previous image on screen; G4: a file
            // without an embedded JPEG says so in JPEG mode
            frame.status = Status::Error(e.clone());
            if self.fine_details {
                frame.fine = FineState::Paused(FinePause::Failed);
            }
            self.last_draw = None;
            self.last_vis = None;
            self.waiting_since = None;
            return frame;
        }
        // a fine view is never drawn while fine details is off, also not as the kept last frame
        // (ADR 0027 W2-Q8)
        if !self.fine_details
            && let Some(d) = self.last_draw
            && self
                .resident
                .iter()
                .any(|r| r.id == d.id && r.kind == ViewKind::Fine)
        {
            self.last_draw = None;
            self.last_vis = None;
        }
        let ready = self.req(key, Want::Shown).and_then(|req| {
            let size = *self.sizes.get(&key)?;
            let scale = self.scale_of(key)?;
            let r = self.serving(&req)?;
            let vis = geometry::visible(size, view_px, scale, self.centre);
            let d = geometry::dest(vis, view_px, scale);
            let draw = Draw {
                id: r.id,
                dest: [d[0] as f32, d[1] as f32, d[2] as f32, d[3] as f32],
                uv: geometry::corner_uvs(size, r.region, vis),
                nearest: scale > 1.0 + EPS,
            };
            Some((draw, vis))
        });
        match ready {
            Some((draw, vis)) => {
                self.touch(draw.id);
                self.last_draw = Some(draw);
                self.last_vis = Some(vis);
                self.waiting_since = None;
                self.mark_shown();
                frame.draw = Some(draw);
                frame.status = Status::Ready;
            }
            None => {
                let since = *self.waiting_since.get_or_insert_with(Instant::now);
                if let Some((draw, vis)) = self.gesture_stand_in(key, view_px) {
                    // not the finished view: the status stays Preparing and the shown time is
                    // not marked
                    self.touch(draw.id);
                    self.last_draw = Some(draw);
                    self.last_vis = Some(vis);
                }
                frame.draw = self.last_draw;
                frame.status = Status::Preparing {
                    since,
                    indicator: since.elapsed() >= self.config.indicator_after,
                };
            }
        }
        if self.clipping_wanted {
            frame.clipping = self.clipping_state(key, frame.draw);
        }
        if self.focus_wanted {
            frame.focus = self.focus_state(key, frame.draw);
        }
        frame.fine = self.fine_state(key, &frame);
        frame
    }

    /// The state of fine details for the current image `key`, which can be shown, in `frame`
    /// (ADR 0027 W2): the mode is checked before the zoom, the zoom before a failure.
    fn fine_state(&self, key: Key, frame: &Frame) -> FineState {
        if !self.fine_details {
            return FineState::Off;
        }
        if key.1 != Source::Raw {
            return FineState::Paused(FinePause::Mode);
        }
        let Some(scale) = self.scale_of(key) else {
            return FineState::Waiting;
        };
        if scale >= HALF_DENSITY - EPS {
            return FineState::Paused(FinePause::Zoom);
        }
        if self.fine_failed.contains(&key) {
            return FineState::Paused(FinePause::Failed);
        }
        let shown = frame.status == Status::Ready
            && frame.draw.is_some_and(|d| {
                self.resident.iter().any(|r| {
                    r.id == d.id
                        && r.kind == ViewKind::Fine
                        && r.generation == self.generation
                        && (r.pos, r.source) == key
                })
            });
        if shown {
            FineState::Shown
        } else {
            FineState::Waiting
        }
    }

    /// What stands in for the current image `key` during a zoom gesture at 50 % or less while no
    /// resident view serves it (the first gesture after a step, before the half-size view
    /// arrives; ADR 0027 W1-Q3, W1-Q9): its densest resident reduced view of the whole image,
    /// placed at the gesture's scale and position (enlarged). Only a view of the current image
    /// and generation (V2), and none the GPU would reduce more than 2:1 (it has no mipmaps; R2).
    /// While fine details is on, its fine views count too, and at an equal density the fine view
    /// is taken (ADR 0027 W2-Q8).
    fn gesture_stand_in(&self, key: Key, view_px: [u32; 2]) -> Option<(Draw, RectF)> {
        if !self.in_gesture() {
            return None;
        }
        let size = *self.sizes.get(&key)?;
        let scale = self.scale_of(key)?;
        if scale > HALF_DENSITY + EPS {
            return None;
        }
        let whole = geometry::whole(size);
        let r = self
            .resident
            .iter()
            .filter(|r| {
                r.generation == self.generation
                    && (r.pos, r.source) == key
                    && (r.kind == ViewKind::Reduced
                        || (r.kind == ViewKind::Fine && self.fine_details))
                    && r.region == whole
                    && r.density <= 2.0 * scale + EPS
            })
            .max_by(|a, b| {
                a.density
                    .total_cmp(&b.density)
                    .then((a.kind == ViewKind::Fine).cmp(&(b.kind == ViewKind::Fine)))
            })?;
        let vis = geometry::visible(size, view_px, scale, self.centre);
        let d = geometry::dest(vis, view_px, scale);
        let draw = Draw {
            id: r.id,
            dest: [d[0] as f32, d[1] as f32, d[2] as f32, d[3] as f32],
            uv: geometry::corner_uvs(size, r.region, vis),
            nearest: false,
        };
        Some((draw, vis))
    }

    /// The clipping state of the current image `key` while a map is wanted and the image can be
    /// shown, for a frame that draws `draw`. The map is given whenever the drawn view is of the
    /// current image, also the kept one while a new zoom or position of it is prepared (the
    /// overlay does not flicker while zooming), placed by the rectangle that view was drawn
    /// with; it is not given while an earlier image's view is kept on the screen (V2).
    fn clipping_state(&mut self, key: Key, draw: Option<Draw>) -> ClippingState {
        if self.clipping_none.contains(&key) {
            return ClippingState::Unavailable;
        }
        let of_current = |d: &Draw| {
            self.resident.iter().any(|r| {
                r.id == d.id && r.generation == self.generation && (r.pos, r.source) == key
            })
        };
        let placed = draw
            .filter(of_current)
            .and(self.last_vis)
            .zip(self.sizes.get(&key).copied())
            .zip(self.clipping_map(key).cloned());
        let Some(((vis, size), map)) = placed else {
            return ClippingState::Reading;
        };
        // the map's grid in pixels; it may reach past the image's right and bottom edges by
        // less than a cell
        let grid = Rect::new(
            0,
            0,
            map.width.saturating_mul(map.cell),
            map.height.saturating_mul(map.cell),
        );
        let uv = geometry::corner_uvs(size, grid, vis);
        // shown now: the last of the kept maps to be dropped
        self.keep_clipping_map(key, map.clone());
        ClippingState::Ready { map, uv }
    }

    /// The focus state of the current image `key` while a map is wanted and the image can be
    /// shown, for a frame that draws `draw`, by the rules of [`State::clipping_state`]. The map
    /// covers the whole image, as the half-size view it is made from does, whatever that
    /// render's rounding.
    fn focus_state(&mut self, key: Key, draw: Option<Draw>) -> FocusState {
        if self.focus_none.contains(&key) {
            return FocusState::Unavailable;
        }
        let of_current = |d: &Draw| {
            self.resident.iter().any(|r| {
                r.id == d.id && r.generation == self.generation && (r.pos, r.source) == key
            })
        };
        let placed = draw
            .filter(of_current)
            .and(self.last_vis)
            .zip(self.sizes.get(&key).copied())
            .zip(self.focus_map(key).cloned());
        let Some(((vis, size), map)) = placed else {
            return FocusState::Reading;
        };
        let uv = geometry::corner_uvs(size, Rect::new(0, 0, size.w, size.h), vis);
        // shown now: the last of the kept maps to be dropped
        self.keep_focus_map(key, map.clone());
        FocusState::Ready { map, uv }
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
            ViewKind::Scaled | ViewKind::Reduced | ViewKind::Fine => job.density(),
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

    /// The half-size view of image `pos` (asked for during a zoom gesture below 50 %).
    fn half_out(s: &State<u32>, pos: usize) -> ViewOut {
        ViewOut {
            kind: ViewKind::Half,
            region: geometry::whole(SIZE),
            density: HALF_DENSITY,
            ..out(s, pos, Want::Fit)
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
        assert_eq!(kind(&mut s, Zoom::Fit), Some(ViewKind::Reduced));
        assert_eq!(kind(&mut s, Zoom::Scale(0.4)), Some(ViewKind::Reduced));
        assert_eq!(kind(&mut s, Zoom::Scale(0.5)), Some(ViewKind::Half));
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
        // the fit view (reduced to 0.25, ADR 0027 W1) is not drawn enlarged at 0.4 at rest
        let fit = s.accept(&out(&s, 10, Want::Fit), 100);
        assert!(fit.is_some());
        s.zoom = Zoom::Scale(0.4);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw, None);
        let reduced = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(reduced.is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), reduced);
        s.zoom = Zoom::Scale(0.7);
        let f = s.frame(VIEW);
        assert!(matches!(
            f.status,
            Status::Preparing {
                indicator: false,
                ..
            }
        ));
        // the previous (reduced) view stays on screen until the sharp one arrives (V2)
        assert_eq!(f.draw.map(|d| d.id), reduced);
        let scaled = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(scaled.is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), scaled);
    }

    // ---------- the reduced view below 50 % (ADR 0027 W1, T309) ----------

    /// The job of a reduced view of the whole image at `density_q` millionths.
    fn reduced_job(density_q: u64) -> JobView {
        JobView {
            kind: ViewKind::Reduced,
            region: geometry::whole(SIZE),
            density_q,
        }
    }

    /// The job of the half-size view.
    fn half_job() -> JobView {
        JobView {
            kind: ViewKind::Half,
            region: geometry::whole(SIZE),
            density_q: 500_000,
        }
    }

    /// The finished render of `view`, at the size the worker reduces it to.
    fn rendered(view: JobView) -> Rgb {
        let (w, h) = crate::scale::scaled_size(view.region.w, view.region.h, view.density());
        Rgb {
            width: w,
            height: h,
            data: vec![0; (w * h * 3) as usize],
        }
    }

    fn pause_the_gesture(s: &mut State<u32>) {
        s.gesture_at = Some(Instant::now() - Duration::from_secs(1));
    }

    #[test]
    fn below_50_percent_the_view_is_reduced_to_the_scale_and_a_gesture_uses_the_half_size_view() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        let job = |s: &State<u32>| s.req(k(10), Want::Shown).map(|r| r.job);
        // at rest (the fit scale of 4000×3000 in 1000×800 is 0.25)
        s.zoom = Zoom::Fit;
        assert_eq!(job(&s), Some(reduced_job(250_000)));
        s.zoom = Zoom::Scale(0.4);
        assert_eq!(job(&s), Some(reduced_job(400_000)));
        s.zoom = Zoom::Scale(0.5);
        assert_eq!(job(&s), Some(half_job()));
        s.zoom = Zoom::Scale(0.6);
        assert_eq!(job(&s).map(|j| j.kind), Some(ViewKind::Scaled));
        // during a zoom gesture the half-size view, so no render per step
        s.set_zoom(Zoom::Fit, None, false);
        s.set_zoom(Zoom::Scale(0.25), None, true);
        assert_eq!(job(&s), Some(half_job()));
        s.set_zoom(Zoom::Scale(0.4), None, true);
        assert_eq!(job(&s), Some(half_job()));
        // only the shown view switches: the fit view stays the reduced one
        assert_eq!(
            s.req(k(10), Want::Fit).map(|r| r.job),
            Some(reduced_job(250_000))
        );
        assert_eq!(s.plan(), vec![Job::Render(k(10), half_job())]);
        let half = s.accept(&half_out(&s, 10), 100);
        assert!(half.is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), half);
        // the gesture pauses: the half-size view does not satisfy the plan, the reduced view at
        // the gesture's last scale is made, and the half-size view is drawn until it arrives
        pause_the_gesture(&mut s);
        assert_eq!(s.plan()[0], Job::Render(k(10), reduced_job(400_000)));
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), half);
        let reduced = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(reduced.is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), reduced);
        assert!(!s.plan().contains(&Job::Render(k(10), reduced_job(400_000))));
    }

    #[test]
    fn just_below_50_percent_an_odd_image_gets_the_half_size_view_when_the_reduced_one_would_not_fit()
     {
        let mut s = state(1024);
        let odd = ImageSize {
            w: 6001,
            h: 4001,
            flip: 0,
        };
        s.sizes.insert(k(10), odd);
        let kind = |s: &mut State<u32>, scale: f64| {
            s.zoom = Zoom::Scale(scale);
            s.req(k(10), Want::Shown).map(|r| r.job.kind)
        };
        // 6001 × 0.49995 rounds up to 3001, more than a half-size render of 3000 that rounds down
        assert_eq!(kind(&mut s, 0.49995), Some(ViewKind::Half));
        // 4001 × 0.4999 rounds up to 2001, more than 2000
        assert_eq!(kind(&mut s, 0.4999), Some(ViewKind::Half));
        // 6001 × 0.4998 and 4001 × 0.4998 round up to 3000 and 2000: it fits
        assert_eq!(kind(&mut s, 0.4998), Some(ViewKind::Reduced));
        assert_eq!(kind(&mut s, 0.4), Some(ViewKind::Reduced));
    }

    #[test]
    fn a_gesture_below_50_percent_draws_the_reduced_view_enlarged_until_the_half_size_view_arrives()
    {
        let mut s = state(1024);
        s.config.record = true;
        known(&mut s, 10);
        // image 10 at fit: its reduced view (0.25) is drawn
        let fit = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(fit.is_some());
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        // the first gesture step to 0.4: the reduced view stands in, enlarged to the gesture's
        // scale and position, while the half-size view is made
        s.set_zoom(Zoom::Scale(0.4), Some([200.0, 100.0]), true);
        s.record("zoom");
        let f = s.frame(VIEW);
        assert!(
            matches!(f.status, Status::Preparing { .. }),
            "{:?}",
            f.status
        );
        let vis = geometry::visible(SIZE, VIEW, 0.4, s.centre);
        let d = geometry::dest(vis, VIEW, 0.4);
        let want = Draw {
            id: fit.unwrap_or(0),
            dest: [d[0] as f32, d[1] as f32, d[2] as f32, d[3] as f32],
            uv: geometry::corner_uvs(SIZE, geometry::whole(SIZE), vis),
            nearest: false,
        };
        assert_eq!(f.draw, Some(want));
        assert_eq!(s.last_vis, Some(vis));
        // not the finished view: the request is not marked shown
        assert!(s.records.last().is_some_and(|r| r.shown.is_none()));
        // the half-size view replaces it
        let half = s.accept(&half_out(&s, 10), 100);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), half);
        assert!(s.records.last().is_some_and(|r| r.shown.is_some()));
    }

    #[test]
    fn without_a_reduced_view_of_the_current_image_a_gesture_keeps_the_last_frame() {
        let mut s = state(1024);
        known(&mut s, 10);
        known(&mut s, 11);
        // image 10 at 100 %, and a reduced view of image 11 (prefetched)
        s.zoom = Zoom::Scale(1.0);
        let zoom = s.accept(&out(&s, 10, Want::Shown), 100);
        let other = s.accept(&out(&s, 11, Want::Fit), 100);
        assert!(zoom.is_some() && other.is_some());
        let before = s.frame(VIEW);
        assert_eq!(before.draw.map(|d| d.id), zoom);
        // a gesture to 0.4: no whole-image view of image 10, so the last frame stays, and image
        // 11's view is never drawn (V2)
        s.set_zoom(Zoom::Scale(0.4), None, true);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw, before.draw);
    }

    #[test]
    fn a_gesture_above_50_percent_keeps_the_last_frame_as_before() {
        let mut s = state(1024);
        known(&mut s, 10);
        let fit = s.accept(&out(&s, 10, Want::Shown), 100);
        let before = s.frame(VIEW);
        assert_eq!(before.draw.map(|d| d.id), fit);
        s.set_zoom(Zoom::Scale(0.7), None, true);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        // drawn as it was, not moved to the gesture's scale
        assert_eq!(f.draw, before.draw);
    }

    #[test]
    fn zooming_out_draws_the_denser_reduced_view_and_plans_the_exact_one() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        s.zoom = Zoom::Scale(0.4);
        let r4 = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(r4.is_some());
        s.zoom = Zoom::Scale(0.3);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), r4);
        assert_eq!(s.plan()[0], Job::Render(k(10), reduced_job(300_000)));
        // the half-size view serves 0.3 too: the least dense serving view is drawn
        let half = s.accept(&half_out(&s, 10), 100);
        assert!(half.is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), r4);
    }

    #[test]
    fn a_gesture_far_below_a_reduced_view_draws_the_half_size_view_not_the_reduced_one() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        // at rest at 0.4: the reduced view of 0.4
        s.zoom = Zoom::Scale(0.4);
        let r4 = s.accept(&out(&s, 10, Want::Shown), 100);
        assert!(r4.is_some());
        let before = s.frame(VIEW);
        assert_eq!(before.draw.map(|d| d.id), r4);
        // a gesture down to 0.15: the 0.4 view would be reduced more than 2:1 without mipmaps,
        // so it does not serve and is no stand-in; the half-size view is planned
        s.set_zoom(Zoom::Scale(0.15), None, true);
        assert_eq!(s.plan(), vec![Job::Render(k(10), half_job())]);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw, before.draw);
        // the half-size view arrives and is drawn
        let half = s.accept(&half_out(&s, 10), 100);
        assert!(half.is_some());
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), half);
    }

    #[test]
    fn a_half_size_render_smaller_than_the_reduced_view_becomes_the_half_size_view_once() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        let (generation, key, view, session) = match s.pick(true) {
            Some(Task::Render {
                generation,
                key,
                view,
                session,
            }) => (generation, key, view, session),
            other => panic!("expected a render, got {other:?}"),
        };
        assert_eq!(view, reduced_job(250_000));
        // the backend's half-size render is 700 × 500, smaller than the 1000 × 750 asked for,
        // and comes back unreduced
        let small = Rgb {
            width: 700,
            height: 500,
            data: vec![0; 700 * 500 * 3],
        };
        s.finish_render(generation, key, view, session, Ok(small), 1.0);
        // in flight as the half-size view until it is accepted: no half-size render meanwhile
        assert_eq!(
            s.inflight.iter().map(|f| f.view).collect::<Vec<_>>(),
            vec![half_job()]
        );
        assert!(!s.plan().iter().any(|j| matches!(
            j,
            Job::Render(key, v) if *key == k(10) && matches!(v.kind, ViewKind::Half | ViewKind::Reduced)
        )));
        let outs = std::mem::take(&mut s.outbox);
        assert_eq!(outs.len(), 1);
        assert_eq!(outs[0].kind, ViewKind::Half);
        assert!(s.accept(&outs[0], 100).is_some());
        assert!(s.inflight.is_empty());
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        // the image now gets the half-size view below 50 %: nothing of it is rendered again
        assert_eq!(s.req(k(10), Want::Shown).map(|r| r.job), Some(half_job()));
        assert!(!s.plan().iter().any(|j| matches!(
            j,
            Job::Render(key, v) if *key == k(10) && matches!(v.kind, ViewKind::Half | ViewKind::Reduced)
        )));
    }

    #[test]
    fn two_reduced_views_of_one_image_in_flight_are_each_accepted_and_removed_as_their_own() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        let render = |s: &mut State<u32>| match s.pick(true) {
            Some(Task::Render {
                generation,
                key,
                view,
                session,
            }) => {
                s.finish_render(generation, key, view, session, Ok(rendered(view)), 1.0);
                view
            }
            other => panic!("expected a render, got {other:?}"),
        };
        s.zoom = Zoom::Scale(0.4);
        assert_eq!(render(&mut s), reduced_job(400_000));
        // zoomed out before the 0.4 view is accepted: the 0.3 view is made too
        s.zoom = Zoom::Scale(0.3);
        assert_eq!(render(&mut s), reduced_job(300_000));
        assert_eq!(s.inflight.len(), 2);
        let outs = std::mem::take(&mut s.outbox);
        let (o4, o3) = (&outs[0], &outs[1]);
        // the 0.3 view first: only its own job leaves the in-flight list
        let id3 = s.accept(o3, 100);
        assert!(id3.is_some());
        assert_eq!(
            s.inflight.iter().map(|f| f.view).collect::<Vec<_>>(),
            vec![reduced_job(400_000)]
        );
        // the 0.4 view still serves 0.3: accepted
        let id4 = s.accept(o4, 100);
        assert!(id4.is_some());
        assert!(s.inflight.is_empty());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), id3);
        assert!(!s.plan().contains(&Job::Render(k(10), reduced_job(300_000))));
    }

    #[test]
    fn below_50_percent_the_neighbours_prefetch_the_view_at_the_current_scale() {
        let mut s = state(1024);
        known(&mut s, 11);
        let prefetch = |s: &State<u32>| {
            s.wanted(false)
                .into_iter()
                .filter(|(key, _)| *key == k(11))
                .filter_map(|(key, want)| s.req(key, want))
                .map(|r| r.job)
                .collect::<Vec<_>>()
        };
        // a free level of 0.4: the view at 0.4 first, then the 100 % target
        s.zoom = Zoom::Scale(0.4);
        assert!(s.wanted(false).contains(&(k(11), Want::Shown)));
        assert!(!s.wanted(false).contains(&(k(11), Want::Fit)));
        let jobs = prefetch(&s);
        assert_eq!(jobs.first(), Some(&reduced_job(400_000)));
        assert_eq!(jobs.get(1).map(|j| j.kind), Some(ViewKind::Zoom));
        // at fit, the fit view as before (reduced to the fit scale)
        s.zoom = Zoom::Fit;
        assert!(s.wanted(false).contains(&(k(11), Want::Fit)));
        assert_eq!(prefetch(&s).first(), Some(&reduced_job(250_000)));
        // at 50 % or more, unchanged: the fit view
        s.zoom = Zoom::Scale(0.6);
        assert!(s.wanted(false).contains(&(k(11), Want::Fit)));
        assert!(!s.wanted(false).contains(&(k(11), Want::Shown)));
    }

    #[test]
    fn a_run_of_view_size_changes_at_fit_asks_for_the_half_size_view_then_one_reduced_view() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        // the first frame's size is not a change
        let _ = s.frame(VIEW);
        assert!(!s.in_gesture());
        let reduced_of_10 = |s: &State<u32>| {
            s.plan()
                .into_iter()
                .filter(|j| matches!(j, Job::Render(key, v) if *key == k(10) && v.kind == ViewKind::Reduced))
                .collect::<Vec<_>>()
        };
        assert_eq!(reduced_of_10(&s).len(), 1);
        // the window's edge is dragged: a new size every frame
        let mut last = VIEW;
        for i in 1..=10 {
            last = [1000 + 20 * i, 800];
            let _ = s.frame(last);
            assert!(s.in_gesture());
            assert_eq!(s.plan(), vec![Job::Render(k(10), half_job())]);
            assert!(reduced_of_10(&s).is_empty());
        }
        // it pauses: one reduced view, at the last size's fit scale
        pause_the_gesture(&mut s);
        let fit = geometry::fit_scale(SIZE, last);
        assert_eq!(
            reduced_of_10(&s),
            vec![Job::Render(k(10), reduced_job(quantise(fit)))]
        );
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
        assert!(matches!(s.plan()[0], Job::Render((10, _), v) if v.kind == ViewKind::Reduced));
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
        // image 3's half-size view (left from a gesture there): an unwanted reduced view would
        // be discarded at once (ADR 0027 W1), a half-size view is kept until evicted
        let a = s.accept(&half_out(&s, 3), 600 << 10);
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

    // ---------- an orientation told by the shell (ADR 0024 §2.4, T231) ----------

    /// Opens image `pos` through the open path, as the core does.
    fn opened(s: &mut State<u32>, key: Key) {
        let generation = s.generation;
        s.finish_open(
            generation,
            key,
            Ok((key.0 as u32, SIZE)),
            |_, _| 1 << 20,
            1.0,
        );
    }

    fn flip(s: &State<u32>, key: Key) -> Option<u8> {
        s.sizes.get(&key).map(|z| z.flip)
    }

    #[test]
    fn a_told_orientation_turns_the_opened_images_of_its_path_and_none_takes_it_back() {
        let mut s = state(1024);
        let jpeg = (10, Source::Jpeg);
        opened(&mut s, k(10));
        opened(&mut s, jpeg);
        opened(&mut s, k(11));
        assert_eq!(flip(&s, k(10)), Some(0));
        assert!(s.set_orientation(&path(10), Some(6)));
        assert_eq!(flip(&s, k(10)), Some(6));
        assert_eq!(flip(&s, jpeg), Some(6), "both sources");
        assert_eq!(flip(&s, k(11)), Some(0), "another path is not touched");
        assert!(
            !s.set_orientation(&path(10), Some(6)),
            "the same value again"
        );
        assert!(s.set_orientation(&path(10), None));
        assert_eq!(flip(&s, k(10)), Some(0));
        assert_eq!(flip(&s, jpeg), Some(0));
        assert!(!s.orientations.contains_key(&path(10)));
        // a value outside 1..=8 takes it back as None does
        assert!(s.set_orientation(&path(10), Some(8)));
        assert_eq!(flip(&s, k(10)), Some(5));
        assert!(s.set_orientation(&path(10), Some(9)));
        assert_eq!(flip(&s, k(10)), Some(0));
    }

    #[test]
    fn the_backends_own_flip_comes_back_when_the_orientation_is_taken_back() {
        let mut s = state(1024);
        let generation = s.generation;
        let own = ImageSize { flip: 3, ..SIZE };
        s.finish_open(generation, k(10), Ok((10, own)), |_, _| 1 << 20, 1.0);
        assert!(s.set_orientation(&path(10), Some(6)));
        assert_eq!(flip(&s, k(10)), Some(6));
        assert!(s.set_orientation(&path(10), None));
        assert_eq!(flip(&s, k(10)), Some(3));
    }

    #[test]
    fn an_orientation_told_before_the_open_is_used_when_it_opens_and_reopens() {
        let mut s = state(1024);
        assert!(!s.set_orientation(&path(10), Some(8)), "nothing opened yet");
        opened(&mut s, k(10));
        assert_eq!(flip(&s, k(10)), Some(5));
        assert_eq!(s.own_flips.get(&k(10)), Some(&0));
        // evicted and opened again
        s.drop_session(k(10));
        opened(&mut s, k(10));
        assert_eq!(flip(&s, k(10)), Some(5));
    }

    #[test]
    fn a_told_orientation_survives_a_new_folder_and_a_new_list_and_moves_with_its_image() {
        let mut s = state(1024);
        s.set_orientation(&path(10), Some(6));
        s.open_folder(files(20));
        s.go(10);
        assert_eq!(s.orientations.get(&path(10)), Some(&6));
        let generation = s.generation;
        let own = ImageSize { flip: 3, ..SIZE };
        s.finish_open(generation, k(10), Ok((10, own)), |_, _| 1 << 20, 1.0);
        assert_eq!(flip(&s, k(10)), Some(6));
        // image 3 drops out: image 10 is now at position 9
        s.set_files(without(20, &[3]), 9);
        assert_eq!(s.orientations.get(&path(10)), Some(&6));
        assert_eq!(flip(&s, k(9)), Some(6));
        assert_eq!(s.own_flips.get(&k(9)), Some(&3));
        assert!(!s.own_flips.contains_key(&k(10)));
        assert!(s.set_orientation(&path(10), None));
        assert_eq!(flip(&s, k(9)), Some(3), "the moved own flip comes back");
    }

    #[test]
    fn a_turn_keeps_every_resident_view_of_the_image() {
        let mut s = state(1024);
        opened(&mut s, k(10));
        let fit = s.accept(&out(&s, 10, Want::Fit), 100);
        s.zoom = Zoom::Scale(1.0);
        let ring = s.accept(&out(&s, 10, Want::Ring), 100);
        assert!(fit.is_some() && ring.is_some());
        let before: Vec<u64> = s.resident.iter().map(|r| r.id).collect();
        assert!(s.set_orientation(&path(10), Some(6)));
        let _ = s.frame(VIEW);
        assert!(s.take_dropped().is_empty());
        let after: Vec<u64> = s.resident.iter().map(|r| r.id).collect();
        assert_eq!(before, after);
        assert_eq!(s.zoom, Zoom::Scale(1.0), "the zoom is kept");
    }

    #[test]
    fn at_fit_the_scale_after_a_turn_fits_the_turned_size() {
        let mut s = state(1024);
        opened(&mut s, k(10));
        let centre = s.centre;
        // 4000×3000 in 1000×800: the width decides
        let before = s.scale_of(k(10));
        assert!(before.is_some_and(|v| (v - 0.25).abs() < 1e-9));
        assert!(s.set_orientation(&path(10), Some(6)));
        // turned 3000×4000: the height decides
        let after = s.scale_of(k(10));
        assert!(after.is_some_and(|v| (v - 0.2).abs() < 1e-9));
        let turned = ImageSize { flip: 6, ..SIZE };
        assert_eq!(after, Some(geometry::fit_scale(turned, VIEW)));
        assert_eq!(s.zoom, Zoom::Fit);
        assert_eq!(s.centre, centre);
    }

    // ---------- a turn keeps the same image point in the centre (ADR 0024 §2.5, T235) ----------

    fn assert_centre(s: &State<u32>, want: [f64; 2]) {
        assert!(
            (s.centre[0] - want[0]).abs() < 1e-9 && (s.centre[1] - want[1]).abs() < 1e-9,
            "centre {:?}, want {want:?}",
            s.centre
        );
    }

    #[test]
    fn a_turn_at_100_percent_keeps_the_same_image_point_in_the_centre() {
        let mut s = state(1024);
        opened(&mut s, k(10));
        s.zoom = Zoom::Scale(1.0);
        s.centre = [0.3, 0.4];
        assert!(s.set_orientation(&path(10), Some(6)));
        assert_centre(&s, [0.6, 0.3]);
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        // from 6 to 3: one more clockwise quarter turn
        assert!(s.set_orientation(&path(10), Some(3)));
        assert_centre(&s, [0.7, 0.6]);
        // taken back: from 3 to the file's own 0
        assert!(s.set_orientation(&path(10), None));
        assert_centre(&s, [0.3, 0.4]);
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        assert!(s.gesture_at.is_none(), "a turn is not a zoom gesture");
    }

    #[test]
    fn a_turn_near_an_edge_turns_the_drawn_centre() {
        let mut s = state(1024);
        opened(&mut s, k(10));
        s.zoom = Zoom::Scale(1.0);
        s.centre = [0.25, 0.1];
        // the view's half height is 800 / 3000 / 2, so the drawn centre is [0.25, 0.4 / 3]
        let drawn = geometry::centre_of(SIZE, geometry::visible(SIZE, VIEW, 1.0, s.centre));
        assert!((drawn[0] - 0.25).abs() < 1e-9 && (drawn[1] - 0.4 / 3.0).abs() < 1e-9);
        assert!(s.set_orientation(&path(10), Some(6)));
        assert_centre(&s, [1.0 - 0.4 / 3.0, 0.25]);
    }

    #[test]
    fn a_turn_at_fit_stays_fit_and_keeps_the_centre() {
        let mut s = state(1024);
        opened(&mut s, k(10));
        s.zoom = Zoom::Fit;
        s.centre = [0.3, 0.4];
        assert!(s.set_orientation(&path(10), Some(6)));
        assert_eq!(s.zoom, Zoom::Fit);
        assert_eq!(s.centre, [0.3, 0.4]);
    }

    #[test]
    fn a_turn_of_an_image_that_is_not_current_keeps_the_view() {
        let mut s = state(1024);
        opened(&mut s, k(10));
        opened(&mut s, k(11));
        s.zoom = Zoom::Scale(1.0);
        s.centre = [0.3, 0.4];
        assert!(s.set_orientation(&path(11), Some(6)));
        assert_eq!(flip(&s, k(11)), Some(6));
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        assert_eq!(s.centre, [0.3, 0.4]);
    }

    #[test]
    fn a_turn_raises_a_scale_below_the_turned_minimum() {
        let mut s = state(1024);
        s.view_px = [800, 1000];
        opened(&mut s, k(10));
        // 4000×3000 in 800×1000: fit 0.2, minimum 0.1; turned 3000×4000: fit 0.25, minimum 0.125
        assert!((s.min_scale(Some(SIZE)) - 0.1).abs() < 1e-9);
        let turned = ImageSize { flip: 6, ..SIZE };
        assert!((s.min_scale(Some(turned)) - 0.125).abs() < 1e-9);
        s.zoom = Zoom::Scale(0.1);
        assert!(s.set_orientation(&path(10), Some(6)));
        assert_eq!(s.zoom, Zoom::Scale(0.125));
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

    // ---------- statistics (ADR 0017 §2.6, U7-Q6, T142) ----------

    use crate::backend::StatisticsOrigin;

    fn statistics(share: f64) -> Statistics {
        Statistics {
            origin: StatisticsOrigin::RawData,
            histogram: [[0; 256]; 3],
            clipped: [share; 3],
            deep_shadows: share,
        }
    }

    fn available(share: f64) -> ImageStatistics {
        ImageStatistics::Available(Arc::new(statistics(share)))
    }

    /// Opens image `pos` in RAW mode and puts its shown (fit) view on the screen, and its 100 %
    /// view, which the plan wants before the statistics (U7c-Q5, T144).
    fn open_and_show(s: &mut State<u32>, pos: usize) {
        s.finish_open(s.generation, k(pos), Ok((pos as u32, SIZE)), mb24, 1.0);
        let view = out(s, pos, Want::Shown);
        assert!(s.accept(&view, 1 << 20).is_some());
        let target = out(s, pos, Want::Target);
        assert!(s.accept(&target, 1 << 20).is_some());
    }

    /// The statistics task the interactive worker gets now.
    fn lend_for_statistics(s: &mut State<u32>) -> (u64, Key, u32) {
        match s.pick(true) {
            Some(Task::Statistics {
                generation,
                key,
                session,
            }) => (generation, key, session),
            other => panic!("expected a statistics task, got {other:?}"),
        }
    }

    fn is_statistics(task: &Option<Task<u32>>) -> bool {
        matches!(task, Some(Task::Statistics { .. }))
    }

    #[test]
    fn the_statistics_are_asked_after_the_first_view_is_shown_and_only_once() {
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        // opened but nothing shown yet: the view comes first
        assert_eq!(s.statistics_job(), None);
        let (generation, key, view, session) = match s.pick(true) {
            Some(Task::Render {
                generation,
                key,
                view,
                session,
            }) => (generation, key, view, session),
            other => panic!("expected the shown view's render, got {other:?}"),
        };
        assert_eq!(view.kind, ViewKind::Reduced);
        // the reduced view's size, so its density is the job's (ADR 0027 W1)
        let (w, h) = crate::scale::scaled_size(view.region.w, view.region.h, view.density());
        let reduced = Rgb {
            width: w,
            height: h,
            data: vec![0; (w * h * 3) as usize],
        };
        s.finish_render(generation, key, view, session, Ok(reduced), 1.0);
        // rendered, but not on the screen until the presentation accepted it
        assert_eq!(s.statistics_job(), None);
        let views = std::mem::take(&mut s.outbox);
        assert!(s.accept(&views[0], 1 << 20).is_some());
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.frame(VIEW).statistics, None);
        // the wanted 100 % view comes before them (U7c-Q5, T144)
        assert_eq!(s.statistics_job(), None);
        let target = out(&s, 10, Want::Target);
        assert!(s.accept(&target, 1 << 20).is_some());

        // now they are asked, before anything else, and of the current image
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
        let (generation, key, session) = lend_for_statistics(&mut s);
        assert_eq!((key, session), (k(10), 10));
        // while they are computed they are not asked a second time, by either worker
        assert!(!is_statistics(&s.pick(true)));
        assert!(!is_statistics(&s.pick(false)));
        assert_eq!(s.frame(VIEW).statistics, None);

        s.finish_statistics(generation, key, session, Some(statistics(0.25)), 3.0);
        assert_eq!(s.frame(VIEW).statistics, Some(available(0.25)));
        // the session is back, and the statistics are never asked again for this image
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert!(s.lent.is_empty() && s.lent_paths.is_empty());
        assert_eq!(s.statistics_job(), None);
        for _ in 0..8 {
            assert!(!is_statistics(&s.pick(true)));
            assert!(!is_statistics(&s.pick(false)));
        }
        assert_eq!(s.stats.statistics, 1);
        assert_eq!(s.stats.statistics_ms, vec![3.0]);
    }

    /// The render task the interactive worker gets now, finished and put on the screen.
    fn render_and_show(s: &mut State<u32>) -> JobView {
        let (generation, key, view, session) = match s.pick(true) {
            Some(Task::Render {
                generation,
                key,
                view,
                session,
            }) => (generation, key, view, session),
            other => panic!("expected a render, got {other:?}"),
        };
        s.finish_render(generation, key, view, session, Ok(img()), 1.0);
        for view in std::mem::take(&mut s.outbox) {
            assert!(s.accept(&view, 1 << 20).is_some());
        }
        view
    }

    #[test]
    fn a_wanted_100_percent_view_is_picked_before_the_statistics_which_follow_once_it_is_shown() {
        // at fit: the fit view is on the screen, the 100 % view the plan prefetches is not
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        let view = out(&s, 10, Want::Shown);
        assert!(s.accept(&view, 1 << 20).is_some());
        assert_eq!(s.statistics_job(), None);
        let picked = render_and_show(&mut s);
        assert_eq!(picked.kind, ViewKind::Zoom);
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
        assert!(is_statistics(&s.pick(true)));

        // zoomed in to 100 % (Z): the shown 100 % view first, then the statistics
        let mut s = state(1024);
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        let view = out(&s, 10, Want::Shown);
        assert!(s.accept(&view, 1 << 20).is_some());
        s.toggle_zoom(None);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        assert_eq!(s.statistics_job(), None);
        let picked = render_and_show(&mut s);
        assert_eq!(picked.kind, ViewKind::Zoom);
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
        assert!(is_statistics(&s.pick(true)));

        // a 100 % view of another image does not hold back the current image's statistics
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        assert!(
            s.wanted(false)
                .iter()
                .any(|(key, want)| key.0 == 11 && *want == Want::Target)
        );
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
    }

    #[test]
    fn the_statistics_wait_for_the_dwell_and_for_a_zoom_gesture_to_end() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        // the image just became current: stepping through images starts no statistics
        s.since = Instant::now();
        assert_eq!(s.statistics_job(), None);
        assert!(!is_statistics(&s.pick(true)));
        s.since = Instant::now() - Duration::from_secs(1);
        // a zoom gesture under way
        s.gesture_at = Some(Instant::now());
        assert_eq!(s.statistics_job(), None);
        s.gesture_at = None;
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
        assert!(is_statistics(&s.pick(true)));
    }

    #[test]
    fn the_background_worker_may_compute_the_statistics_of_the_current_image() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        match s.pick(false) {
            Some(Task::Statistics { key, .. }) => assert_eq!(key, k(10)),
            other => panic!("expected a statistics task, got {other:?}"),
        }
    }

    #[test]
    fn the_frame_has_the_statistics_of_its_own_image_and_mode_only() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        let (generation, key, session) = lend_for_statistics(&mut s);
        s.finish_statistics(generation, key, session, Some(statistics(0.5)), 1.0);
        assert_eq!(s.frame(VIEW).statistics, Some(available(0.5)));
        // another image has none (it is not even opened)
        s.go(11);
        assert_eq!(s.frame(VIEW).statistics, None);
        // back: still there, not asked again
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).statistics, Some(available(0.5)));
        assert_eq!(s.statistics_job(), None);
        // the same image in the other display mode is another opened image
        s.set_mode(Source::Jpeg);
        assert_eq!(s.frame(VIEW).statistics, None);
        s.set_mode(Source::Raw);
        assert_eq!(s.frame(VIEW).statistics, Some(available(0.5)));
        assert_eq!(s.stats.statistics, 1);
    }

    #[test]
    fn a_file_without_statistics_says_so_and_is_not_asked_again() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        let (generation, key, session) = lend_for_statistics(&mut s);
        s.finish_statistics(generation, key, session, None, 1.0);
        assert_eq!(s.frame(VIEW).statistics, Some(ImageStatistics::Unavailable));
        assert_eq!(s.statistics_job(), None);
        assert!(!is_statistics(&s.pick(true)));
    }

    #[test]
    fn a_failed_image_has_no_statistics_job() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        s.failed.insert(k(10), DecodeError::new("too large"));
        assert_eq!(s.statistics_job(), None);
    }

    #[test]
    fn an_image_whose_session_was_closed_is_opened_again_for_its_statistics() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        s.drop_session(k(10));
        assert_eq!(s.statistics_job(), Some(Job::Open(k(10))));
        assert!(matches!(
            s.pick(true),
            Some(Task::Open {
                key: (10, Source::Raw),
                ..
            })
        ));
    }

    #[test]
    fn statistics_follow_the_path_through_set_files_and_a_new_folder_clears_them() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        let (generation, key, session) = lend_for_statistics(&mut s);
        s.finish_statistics(generation, key, session, Some(statistics(0.125)), 1.0);
        // image 3 drops out: image 10 is now at position 9
        s.set_files(without(20, &[3]), 9);
        assert_eq!(s.statistics.get(&k(9)), Some(&available(0.125)));
        assert!(!s.statistics.contains_key(&k(10)));
        assert_eq!(s.frame(VIEW).statistics, Some(available(0.125)));
        // the image itself drops out: its statistics go with it
        s.set_files(without(20, &[3, 10]), 9);
        assert!(s.statistics.is_empty());
        assert_eq!(s.frame(VIEW).statistics, None);

        let mut s = state(1024);
        s.statistics.insert(k(10), available(0.5));
        s.open_folder(files(20));
        assert!(s.statistics.is_empty());
    }

    #[test]
    fn statistics_finished_after_set_files_are_discarded_and_their_session_is_dropped() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        let (generation, key, session) = lend_for_statistics(&mut s);
        assert_eq!(s.session_bytes, 24 << 20);
        s.set_files(without(20, &[3]), 9);
        s.finish_statistics(generation, key, session, Some(statistics(0.5)), 1.0);
        // neither the old key nor the image's new one got them, and the session is gone
        assert!(s.statistics.is_empty());
        assert_eq!(s.session_bytes, 0);
        assert!(s.detached.is_empty() && s.lent_paths.is_empty());
        assert!(!s.sessions.contains_key(&k(9)));
    }

    #[test]
    fn a_session_lent_for_statistics_is_released_when_it_comes_back() {
        let mut s = state(1024);
        open_and_show(&mut s, 10);
        let (generation, key, session) = lend_for_statistics(&mut s);
        // a metadata write of the file waits for the session
        assert_eq!(s.release(&path(10)), (true, false));
        assert!(!s.sessions.contains_key(&k(10)));
        assert_eq!(s.session_bytes, 24 << 20);
        assert_eq!(s.release(&path(10)), (false, false));
        s.finish_statistics(generation, key, session, Some(statistics(0.5)), 1.0);
        assert_eq!(s.session_bytes, 0);
        assert!(!s.sessions.contains_key(&k(10)));
        assert!(s.released.is_empty() && s.lent.is_empty() && s.lent_paths.is_empty());
        assert_eq!(s.release(&path(10)), (false, true));
        // the statistics themselves are still good
        assert_eq!(s.frame(VIEW).statistics, Some(available(0.5)));
    }

    // ---------- the clipping map (ADR 0017 §2.6, U7b-Q8, T147) ----------

    /// A map of an image of `SIZE` (cells of 2 × 2) whose first cell is `first`, to tell maps
    /// apart.
    fn map(first: u8) -> ClippingMap {
        let (width, height) = (SIZE.w / 2, SIZE.h / 2);
        let mut data = vec![0; (width * height) as usize];
        data[0] = first;
        ClippingMap {
            cell: 2,
            width,
            height,
            data,
        }
    }

    fn is_clipping(task: &Option<Task<u32>>) -> bool {
        matches!(task, Some(Task::Clipping { .. }))
    }

    /// The clipping task the interactive worker gets now.
    fn lend_for_clipping(s: &mut State<u32>) -> (u64, Key, u32) {
        match s.pick(true) {
            Some(Task::Clipping {
                generation,
                key,
                session,
            }) => (generation, key, session),
            other => panic!("expected a clipping task, got {other:?}"),
        }
    }

    /// [`open_and_show`], and the image's statistics are there.
    fn show_with_statistics(s: &mut State<u32>, pos: usize) {
        open_and_show(s, pos);
        let (generation, key, session) = lend_for_statistics(s);
        s.finish_statistics(generation, key, session, Some(statistics(0.5)), 1.0);
    }

    /// Goes to image `pos`, shows it, and makes its clipping map `map(first)` as the workers do.
    fn show_with_map(s: &mut State<u32>, pos: usize, first: u8) {
        s.go(pos);
        s.since = Instant::now() - Duration::from_secs(1);
        show_with_statistics(s, pos);
        let (generation, key, session) = lend_for_clipping(s);
        assert_eq!(key, k(pos));
        s.finish_clipping(generation, key, session, Some(map(first)), 1.0);
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(first))));
    }

    /// The map of a frame whose clipping state is "ready".
    fn ready_map(frame: &Frame) -> Option<Arc<ClippingMap>> {
        match &frame.clipping {
            ClippingState::Ready { map, .. } => Some(map.clone()),
            _ => None,
        }
    }

    /// The keys of the kept maps, the one shown longest ago first.
    fn kept(s: &State<u32>) -> Vec<Key> {
        s.clipping_maps.iter().map(|(key, _)| *key).collect()
    }

    #[test]
    fn no_clipping_map_is_asked_while_no_overlay_wants_one() {
        let mut s = state(1024);
        assert!(!s.clipping_wanted);
        show_with_statistics(&mut s, 10);
        assert_eq!(s.clipping_job(), None);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::NotWanted);
        // a map kept from before is not given either
        s.keep_clipping_map(k(10), Arc::new(map(1)));
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::NotWanted);
        s.clipping_maps.clear();
        for _ in 0..8 {
            assert!(!is_clipping(&s.pick(true)));
            assert!(!is_clipping(&s.pick(false)));
        }
        assert_eq!(s.stats.clipping_maps, 0);
        assert!(s.clipping_maps.is_empty() && s.clipping_none.is_empty());
    }

    #[test]
    fn a_wanted_clipping_map_is_asked_once_after_the_first_view_and_the_statistics() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        // opened but nothing shown yet: the view comes first
        assert_eq!(s.clipping_job(), None);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Reading);
        let view = out(&s, 10, Want::Shown);
        assert!(s.accept(&view, 1 << 20).is_some());
        // the wanted 100 % view comes before the statistics and the map (U7c-Q5)
        assert_eq!(s.clipping_job(), None);
        let target = out(&s, 10, Want::Target);
        assert!(s.accept(&target, 1 << 20).is_some());

        // both are due now; the statistics are picked first
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
        assert_eq!(s.clipping_job(), Some(Job::Clipping(k(10))));
        let (generation, key, session) = lend_for_statistics(&mut s);
        // while the statistics are computed the map waits, on both workers
        assert_eq!(s.clipping_job(), None);
        assert!(!is_clipping(&s.pick(true)));
        assert!(!is_clipping(&s.pick(false)));
        s.finish_statistics(generation, key, session, Some(statistics(0.5)), 1.0);

        // then the map, of the current image, with its session lent as for the statistics
        assert_eq!(s.clipping_job(), Some(Job::Clipping(k(10))));
        let (generation, key, session) = lend_for_clipping(&mut s);
        assert_eq!((key, session), (k(10), 10));
        assert_eq!(
            s.lent_paths.get(&(generation, key, STATISTICS_LEND)),
            Some(&path(10))
        );
        // while it is made it is not asked a second time, by either worker
        assert!(!is_clipping(&s.pick(true)));
        assert!(!is_clipping(&s.pick(false)));
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Reading);

        s.finish_clipping(generation, key, session, Some(map(1)), 2.0);
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(1))));
        // the session is back, and the map is not asked again for this image
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert!(s.lent.is_empty() && s.lent_paths.is_empty());
        assert_eq!(s.clipping_job(), None);
        for _ in 0..8 {
            assert!(!is_clipping(&s.pick(true)));
            assert!(!is_clipping(&s.pick(false)));
        }
        assert_eq!(s.stats.clipping_maps, 1);
        assert_eq!(s.stats.clipping_ms, vec![2.0]);
    }

    #[test]
    fn the_clipping_map_waits_for_the_dwell_and_for_a_zoom_gesture_to_end() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        // the image just became current: stepping through images starts no map
        s.since = Instant::now();
        assert_eq!(s.clipping_job(), None);
        assert!(!is_clipping(&s.pick(true)));
        s.since = Instant::now() - Duration::from_secs(1);
        s.gesture_at = Some(Instant::now());
        assert_eq!(s.clipping_job(), None);
        s.gesture_at = None;
        assert_eq!(s.clipping_job(), Some(Job::Clipping(k(10))));
        // the background worker may make it too
        assert!(is_clipping(&s.pick(false)));
    }

    #[test]
    fn the_overlay_turned_on_later_asks_the_map_of_the_image_already_shown() {
        let mut s = state(1024);
        show_with_statistics(&mut s, 10);
        assert_eq!(s.clipping_job(), None);
        s.clipping_wanted = true;
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Reading);
        let (generation, key, session) = lend_for_clipping(&mut s);
        s.finish_clipping(generation, key, session, Some(map(3)), 1.0);
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(3))));
        // turned off: not given; turned on again: the kept map, not asked again
        s.clipping_wanted = false;
        assert_eq!(s.frame(VIEW).clipping, ClippingState::NotWanted);
        s.clipping_wanted = true;
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(3))));
        assert_eq!(s.clipping_job(), None);
        assert_eq!(s.stats.clipping_maps, 1);
    }

    #[test]
    fn no_clipping_map_is_asked_for_a_neighbour() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        // the neighbours the plan prefetches are opened and have their views, as the current one
        for pos in [9, 11, 12] {
            open_and_show(&mut s, pos);
        }
        let mut asked = Vec::new();
        for turn in 0..64 {
            match s.pick(turn % 2 == 0) {
                Some(Task::Clipping {
                    generation,
                    key,
                    session,
                }) => {
                    asked.push(key);
                    s.finish_clipping(generation, key, session, Some(map(1)), 1.0);
                }
                Some(_) => {}
                None => {}
            }
        }
        assert_eq!(asked, vec![k(10)]);
        assert_eq!(kept(&s), vec![k(10)]);
    }

    #[test]
    fn a_file_without_a_clipping_map_says_so_and_is_not_asked_again() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        let (generation, key, session) = lend_for_clipping(&mut s);
        // no map for this file, or making it failed
        s.finish_clipping(generation, key, session, None, 1.0);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Unavailable);
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert_eq!(s.clipping_job(), None);
        for _ in 0..8 {
            assert!(!is_clipping(&s.pick(true)));
            assert!(!is_clipping(&s.pick(false)));
        }
        assert_eq!(s.stats.clipping_maps, 1);
        // remembered for that image and mode only, also when it is shown again
        s.go(11);
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Unavailable);
        assert_eq!(s.clipping_job(), None);
        s.set_mode(Source::Jpeg);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Reading);

        // a file that cannot be shown has no map and no job
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        s.failed.insert(k(10), DecodeError::new("too large"));
        assert_eq!(s.clipping_job(), None);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Unavailable);
    }

    #[test]
    fn an_image_whose_session_was_closed_is_opened_again_for_its_clipping_map() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        s.drop_session(k(10));
        assert_eq!(s.clipping_job(), Some(Job::Open(k(10))));
        assert!(matches!(
            s.pick(true),
            Some(Task::Open {
                key: (10, Source::Raw),
                ..
            })
        ));
    }

    #[test]
    fn the_fifth_images_clipping_map_drops_the_one_shown_longest_ago_which_is_asked_again() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        for (pos, first) in [(10, 1), (11, 2), (12, 3), (13, 4)] {
            show_with_map(&mut s, pos, first);
        }
        assert_eq!(kept(&s), vec![k(10), k(11), k(12), k(13)]);
        // the fifth: image 10 was shown longest ago
        show_with_map(&mut s, 14, 5);
        assert_eq!(kept(&s), vec![k(11), k(12), k(13), k(14)]);

        // image 11 is shown again: its map is still there and is now the one shown last
        s.go(11);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.clipping_job(), None);
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(2))));
        assert_eq!(kept(&s), vec![k(12), k(13), k(14), k(11)]);

        // image 10 is shown again: its map is asked again, and image 12's is dropped
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Reading);
        assert_eq!(s.clipping_job(), Some(Job::Clipping(k(10))));
        let (generation, key, session) = lend_for_clipping(&mut s);
        s.finish_clipping(generation, key, session, Some(map(6)), 1.0);
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(6))));
        assert_eq!(kept(&s), vec![k(13), k(14), k(11), k(10)]);
        assert_eq!(s.stats.clipping_maps, 6);

        // with the overlay off, a dropped map is not asked again
        s.clipping_wanted = false;
        s.go(12);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.clipping_job(), None);
    }

    #[test]
    fn clipping_maps_follow_the_path_through_set_files_and_a_new_folder_empties_them() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_map(&mut s, 10, 1);
        show_with_map(&mut s, 11, 2);
        s.clipping_none.insert(k(12));
        // image 3 drops out: images 10, 11 and 12 are now at positions 9, 10 and 11
        s.set_files(without(20, &[3]), 9);
        assert_eq!(
            s.clipping_maps,
            vec![(k(9), Arc::new(map(1))), (k(10), Arc::new(map(2)))]
        );
        assert_eq!(s.clipping_none, HashSet::from([k(11)]));
        let f = s.frame(VIEW);
        assert_eq!(f.path, Some(path(10)));
        assert_eq!(ready_map(&f), Some(Arc::new(map(1))));
        assert_eq!(s.clipping_job(), None);
        s.go(11);
        assert_eq!(s.frame(VIEW).path, Some(path(12)));
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Unavailable);
        // image 10 itself drops out: its map goes with it
        s.set_files(without(20, &[3, 10]), 9);
        assert_eq!(s.clipping_maps, vec![(k(9), Arc::new(map(2)))]);
        assert_eq!(s.clipping_none, HashSet::from([k(10)]));

        // another folder empties them; the wish stays
        s.open_folder(files(20));
        assert!(s.clipping_maps.is_empty() && s.clipping_none.is_empty());
        assert!(s.clipping_wanted);
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Reading);
        // no image: no map
        s.open_folder(Vec::new());
        assert_eq!(s.frame(VIEW).clipping, ClippingState::Unavailable);
        s.clipping_wanted = false;
        assert_eq!(s.frame(VIEW).clipping, ClippingState::NotWanted);
    }

    #[test]
    fn a_clipping_map_finished_after_set_files_is_discarded_and_its_session_is_dropped() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        let (generation, key, session) = lend_for_clipping(&mut s);
        assert_eq!(s.session_bytes, 24 << 20);
        s.set_files(without(20, &[3]), 9);
        s.finish_clipping(generation, key, session, Some(map(1)), 1.0);
        // neither the old key nor the image's new one got it, and the session is gone
        assert!(s.clipping_maps.is_empty() && s.clipping_none.is_empty());
        assert_eq!(s.session_bytes, 0);
        assert!(s.detached.is_empty() && s.lent_paths.is_empty());
        assert!(!s.sessions.contains_key(&k(9)));
    }

    #[test]
    fn a_session_lent_for_the_clipping_map_is_released_when_it_comes_back() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        let (generation, key, session) = lend_for_clipping(&mut s);
        // a metadata write of the file waits for the session
        assert_eq!(s.release(&path(10)), (true, false));
        assert!(!s.sessions.contains_key(&k(10)));
        assert_eq!(s.session_bytes, 24 << 20);
        assert_eq!(s.release(&path(10)), (false, false));
        s.finish_clipping(generation, key, session, Some(map(1)), 1.0);
        assert_eq!(s.session_bytes, 0);
        assert!(!s.sessions.contains_key(&k(10)));
        assert!(s.released.is_empty() && s.lent.is_empty() && s.lent_paths.is_empty());
        assert_eq!(s.release(&path(10)), (false, true));
        // the map itself is still good
        assert_eq!(ready_map(&s.frame(VIEW)), Some(Arc::new(map(1))));
    }

    #[test]
    fn the_frame_gives_the_map_placed_on_the_drawn_view_of_the_current_image_only() {
        let mut s = state(1024);
        s.clipping_wanted = true;
        // an image turned on its side (flip 6) with an odd width and height: the map's grid of
        // 2 × 2 cells is one pixel wider and higher than the image
        let size = ImageSize {
            w: 4001,
            h: 3001,
            flip: 6,
        };
        s.sizes.insert(k(10), size);
        let half = s.accept(&out(&s, 10, Want::Fit), 100);
        assert!(half.is_some());
        let map = Arc::new(ClippingMap {
            cell: 2,
            width: 2001,
            height: 1501,
            data: vec![0; 2001 * 1501],
        });
        s.keep_clipping_map(k(10), map.clone());
        let grid = Rect::new(0, 0, 4002, 3002);

        // at fit: the whole image, with the coordinates of `corner_uvs`
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        let draw = f.draw.unwrap();
        assert_eq!(draw.id, half.unwrap());
        let fit_vis = geometry::visible(size, VIEW, geometry::fit_scale(size, VIEW), s.centre);
        let fit_uv = geometry::corner_uvs(size, grid, fit_vis);
        let ClippingState::Ready { map: given, uv } = &f.clipping else {
            panic!("expected the map, got {:?}", f.clipping);
        };
        // the kept map itself, not a copy
        assert!(Arc::ptr_eq(given, &map));
        assert_eq!(*uv, fit_uv);
        // the orientation is in them: the upright top left corner is the sensor's bottom left,
        // and the image ends 1 pixel before the map's right and bottom edges
        let (right, bottom) = (4001.0 / 4002.0, 3001.0 / 3002.0);
        let expected = [[0.0, bottom], [0.0, 0.0], [right, 0.0], [right, bottom]];
        for (corner, (got, want)) in fit_uv.iter().zip(expected).enumerate() {
            for axis in 0..2 {
                assert!(
                    (f64::from(got[axis]) - want[axis]).abs() < 1e-6,
                    "corner {corner}: {got:?}, expected {want:?}"
                );
            }
        }
        // the same corners as the image's own texture, which ends at the image's edges
        assert_eq!(
            draw.uv,
            [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            "the half-size view of the whole image"
        );

        // a new zoom level is being prepared: the fit view stays on the screen (V2), and the
        // map stays with it, placed as that view is
        s.zoom = Zoom::Scale(0.7);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw, Some(draw));
        assert_eq!(
            f.clipping,
            ClippingState::Ready {
                map: map.clone(),
                uv: fit_uv
            }
        );
        // the sharp view arrives: the map is placed on the zoomed view
        let scaled = s.accept(&out(&s, 10, Want::Shown), 100);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), scaled);
        let zoomed_uv =
            geometry::corner_uvs(size, grid, geometry::visible(size, VIEW, 0.7, s.centre));
        assert_ne!(zoomed_uv, fit_uv);
        assert_eq!(
            f.clipping,
            ClippingState::Ready {
                map: map.clone(),
                uv: zoomed_uv
            }
        );
        // a new position (a pan) is being prepared: still the map, placed as the kept view is
        s.pan([-3000.0, 0.0]);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw.map(|d| d.id), scaled);
        assert_eq!(
            f.clipping,
            ClippingState::Ready {
                map: map.clone(),
                uv: zoomed_uv
            }
        );

        // the next image: the earlier image's view is kept on the screen (V2), so no map is
        // given, not even the next image's own, until its view is drawn
        s.go(11);
        s.sizes.insert(k(11), SIZE);
        s.keep_clipping_map(k(11), Arc::new(self::map(2)));
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw.map(|d| d.id), scaled);
        assert_eq!(f.clipping, ClippingState::Reading);
        let next = s.accept(&out(&s, 11, Want::Shown), 100);
        let f = s.frame(VIEW);
        assert_eq!(f.draw.map(|d| d.id), next);
        assert_eq!(ready_map(&f), Some(Arc::new(self::map(2))));

        // the other display mode: the earlier mode's view is kept, so no map either
        s.set_mode(Source::Jpeg);
        s.sizes.insert((11, Source::Jpeg), SIZE);
        s.keep_clipping_map((11, Source::Jpeg), Arc::new(self::map(3)));
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw.map(|d| d.id), next);
        assert_eq!(f.clipping, ClippingState::Reading);
        // an error: nothing is drawn, and there is no map
        s.failed
            .insert((11, Source::Jpeg), DecodeError::new("no embedded JPEG"));
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Error(_)));
        assert_eq!(f.clipping, ClippingState::Unavailable);
    }

    // ---------- the focus map (ADR 0017 §2.6, U9a, T167) ----------

    /// A focus map of the half-size render of an image of `SIZE` (one cell of 2 × 2 per pixel of
    /// the render) whose first cell is `first`, to tell maps apart.
    fn edges(first: u8) -> FocusMap {
        let (width, height) = (SIZE.w / 2, SIZE.h / 2);
        let mut data = vec![0; (width * height) as usize];
        data[0] = first;
        FocusMap {
            cell: 2,
            width,
            height,
            data,
        }
    }

    fn is_focus(task: &Option<Task<u32>>) -> bool {
        matches!(task, Some(Task::Focus { .. }))
    }

    /// The focus task the interactive worker gets now.
    fn lend_for_focus(s: &mut State<u32>) -> (u64, Key, u32) {
        match s.pick(true) {
            Some(Task::Focus {
                generation,
                key,
                session,
            }) => (generation, key, session),
            other => panic!("expected a focus task, got {other:?}"),
        }
    }

    /// Goes to image `pos`, shows it, and makes its focus map `edges(first)` as the workers do.
    fn show_with_focus_map(s: &mut State<u32>, pos: usize, first: u8) {
        s.go(pos);
        s.since = Instant::now() - Duration::from_secs(1);
        show_with_statistics(s, pos);
        let (generation, key, session) = lend_for_focus(s);
        assert_eq!(key, k(pos));
        s.finish_focus(generation, key, session, Some(edges(first)), 1.0);
        assert_eq!(ready_focus(&s.frame(VIEW)), Some(Arc::new(edges(first))));
    }

    /// The map of a frame whose focus state is "ready".
    fn ready_focus(frame: &Frame) -> Option<Arc<FocusMap>> {
        match &frame.focus {
            FocusState::Ready { map, .. } => Some(map.clone()),
            _ => None,
        }
    }

    /// The keys of the kept focus maps, the one shown longest ago first.
    fn kept_focus(s: &State<u32>) -> Vec<Key> {
        s.focus_maps.iter().map(|(key, _)| *key).collect()
    }

    #[test]
    fn no_focus_map_is_asked_while_the_focus_marking_is_off() {
        let mut s = state(1024);
        assert!(!s.focus_wanted);
        // the clipping map wanted does not ask for a focus map
        s.clipping_wanted = true;
        show_with_statistics(&mut s, 10);
        assert_eq!(s.focus_job(), None);
        assert_eq!(s.frame(VIEW).focus, FocusState::NotWanted);
        // a map kept from before is not given either
        s.keep_focus_map(k(10), Arc::new(edges(1)));
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.frame(VIEW).focus, FocusState::NotWanted);
        s.focus_maps.clear();
        for _ in 0..8 {
            let task = s.pick(true);
            assert!(!is_focus(&task));
            if let Some(Task::Clipping {
                generation,
                key,
                session,
            }) = task
            {
                s.finish_clipping(generation, key, session, Some(map(1)), 1.0);
            }
            assert!(!is_focus(&s.pick(false)));
        }
        assert_eq!(s.stats.focus_maps, 0);
        assert!(s.focus_maps.is_empty() && s.focus_none.is_empty());
    }

    #[test]
    fn a_wanted_focus_map_is_asked_once_after_the_first_view_the_statistics_and_the_clipping_map() {
        let mut s = state(1024);
        s.focus_wanted = true;
        s.clipping_wanted = true;
        s.finish_open(s.generation, k(10), Ok((10, SIZE)), mb24, 1.0);
        // opened but nothing shown yet: the view comes first
        assert_eq!(s.focus_job(), None);
        assert_eq!(s.frame(VIEW).focus, FocusState::Reading);
        let view = out(&s, 10, Want::Shown);
        assert!(s.accept(&view, 1 << 20).is_some());
        // the wanted 100 % view comes before the statistics and both maps (U7c-Q5)
        assert_eq!(s.focus_job(), None);
        let target = out(&s, 10, Want::Target);
        assert!(s.accept(&target, 1 << 20).is_some());

        // all three are due now; the statistics are picked first
        assert_eq!(s.statistics_job(), Some(Job::Statistics(k(10))));
        assert_eq!(s.clipping_job(), Some(Job::Clipping(k(10))));
        assert_eq!(s.focus_job(), Some(Job::Focus(k(10))));
        let (generation, key, session) = lend_for_statistics(&mut s);
        // while the statistics are computed the focus map waits, on both workers
        assert_eq!(s.focus_job(), None);
        assert!(!is_focus(&s.pick(true)));
        assert!(!is_focus(&s.pick(false)));
        s.finish_statistics(generation, key, session, Some(statistics(0.5)), 1.0);

        // then the clipping map, before the focus map
        assert_eq!(s.focus_job(), Some(Job::Focus(k(10))));
        let (generation, key, session) = lend_for_clipping(&mut s);
        // while it is made the focus map waits, on both workers
        assert_eq!(s.focus_job(), None);
        assert!(!is_focus(&s.pick(true)));
        assert!(!is_focus(&s.pick(false)));
        s.finish_clipping(generation, key, session, Some(map(1)), 1.0);

        // then the focus map, of the current image, with its session lent as for the statistics
        assert_eq!(s.focus_job(), Some(Job::Focus(k(10))));
        let (generation, key, session) = lend_for_focus(&mut s);
        assert_eq!((key, session), (k(10), 10));
        assert_eq!(
            s.lent_paths.get(&(generation, key, STATISTICS_LEND)),
            Some(&path(10))
        );
        // while it is made it is not asked a second time, by either worker
        assert!(!is_focus(&s.pick(true)));
        assert!(!is_focus(&s.pick(false)));
        assert_eq!(s.frame(VIEW).focus, FocusState::Reading);

        s.finish_focus(generation, key, session, Some(edges(1)), 2.0);
        assert_eq!(ready_focus(&s.frame(VIEW)), Some(Arc::new(edges(1))));
        // the session is back, and the map is not asked again for this image
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert!(s.lent.is_empty() && s.lent_paths.is_empty());
        assert_eq!(s.focus_job(), None);
        for _ in 0..8 {
            assert!(!is_focus(&s.pick(true)));
            assert!(!is_focus(&s.pick(false)));
        }
        assert_eq!(s.stats.focus_maps, 1);
        assert_eq!(s.stats.focus_ms, vec![2.0]);
    }

    #[test]
    fn the_focus_map_waits_for_the_dwell_and_for_a_zoom_gesture_and_is_not_asked_for_a_neighbour() {
        let mut s = state(1024);
        s.focus_wanted = true;
        show_with_statistics(&mut s, 10);
        // the image just became current: stepping through images starts no map
        s.since = Instant::now();
        assert_eq!(s.focus_job(), None);
        assert!(!is_focus(&s.pick(true)));
        s.since = Instant::now() - Duration::from_secs(1);
        s.gesture_at = Some(Instant::now());
        assert_eq!(s.focus_job(), None);
        s.gesture_at = None;
        assert_eq!(s.focus_job(), Some(Job::Focus(k(10))));

        // the neighbours the plan prefetches are opened and have their views; only the current
        // image's map is asked, by either worker
        for pos in [9, 11, 12] {
            open_and_show(&mut s, pos);
        }
        let mut asked = Vec::new();
        for turn in 0..64 {
            if let Some(Task::Focus {
                generation,
                key,
                session,
            }) = s.pick(turn % 2 == 1)
            {
                asked.push(key);
                s.finish_focus(generation, key, session, Some(edges(1)), 1.0);
            }
        }
        assert_eq!(asked, vec![k(10)]);
        assert_eq!(kept_focus(&s), vec![k(10)]);
    }

    #[test]
    fn a_file_without_a_focus_map_says_so_and_is_not_asked_again() {
        let mut s = state(1024);
        s.focus_wanted = true;
        show_with_statistics(&mut s, 10);
        let (generation, key, session) = lend_for_focus(&mut s);
        // its half-size render failed, or gave no map
        s.finish_focus(generation, key, session, None, 1.0);
        assert_eq!(s.frame(VIEW).focus, FocusState::Unavailable);
        assert!(matches!(s.sessions.get(&k(10)), Some(Some(10))));
        assert_eq!(s.focus_job(), None);
        for _ in 0..8 {
            assert!(!is_focus(&s.pick(true)));
            assert!(!is_focus(&s.pick(false)));
        }
        assert_eq!(s.stats.focus_maps, 1);
        // remembered for that image and mode only, also when it is shown again
        s.go(11);
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).focus, FocusState::Unavailable);
        assert_eq!(s.focus_job(), None);
        s.set_mode(Source::Jpeg);
        assert_eq!(s.frame(VIEW).focus, FocusState::Reading);

        // a file that cannot be shown has no map and no job
        let mut s = state(1024);
        s.focus_wanted = true;
        show_with_statistics(&mut s, 10);
        s.failed.insert(k(10), DecodeError::new("too large"));
        assert_eq!(s.focus_job(), None);
        assert_eq!(s.frame(VIEW).focus, FocusState::Unavailable);

        // an image whose session was closed is opened again for its map
        let mut s = state(1024);
        s.focus_wanted = true;
        show_with_statistics(&mut s, 10);
        s.drop_session(k(10));
        assert_eq!(s.focus_job(), Some(Job::Open(k(10))));
    }

    #[test]
    fn the_fifth_images_focus_map_drops_the_one_shown_longest_ago_which_is_asked_again() {
        let mut s = state(1024);
        s.focus_wanted = true;
        for (pos, first) in [(10, 1), (11, 2), (12, 3), (13, 4)] {
            show_with_focus_map(&mut s, pos, first);
        }
        assert_eq!(kept_focus(&s), vec![k(10), k(11), k(12), k(13)]);
        // the fifth: image 10 was shown longest ago
        show_with_focus_map(&mut s, 14, 5);
        assert_eq!(kept_focus(&s), vec![k(11), k(12), k(13), k(14)]);

        // image 11 is shown again: its map is still there and is now the one shown last
        s.go(11);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.focus_job(), None);
        assert_eq!(ready_focus(&s.frame(VIEW)), Some(Arc::new(edges(2))));
        assert_eq!(kept_focus(&s), vec![k(12), k(13), k(14), k(11)]);

        // image 10 is shown again: its map is asked again, and image 12's is dropped
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.frame(VIEW).focus, FocusState::Reading);
        assert_eq!(s.focus_job(), Some(Job::Focus(k(10))));
        let (generation, key, session) = lend_for_focus(&mut s);
        s.finish_focus(generation, key, session, Some(edges(6)), 1.0);
        assert_eq!(ready_focus(&s.frame(VIEW)), Some(Arc::new(edges(6))));
        assert_eq!(kept_focus(&s), vec![k(13), k(14), k(11), k(10)]);
        assert_eq!(s.stats.focus_maps, 6);

        // with the focus marking off, a dropped map is not asked again
        s.focus_wanted = false;
        s.go(12);
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(s.frame(VIEW).status, Status::Ready);
        assert_eq!(s.focus_job(), None);
    }

    #[test]
    fn focus_maps_follow_the_path_through_set_files_and_a_new_folder_empties_them() {
        let mut s = state(1024);
        s.focus_wanted = true;
        show_with_focus_map(&mut s, 10, 1);
        show_with_focus_map(&mut s, 11, 2);
        s.focus_none.insert(k(12));
        // image 3 drops out: images 10, 11 and 12 are now at positions 9, 10 and 11
        s.set_files(without(20, &[3]), 9);
        assert_eq!(
            s.focus_maps,
            vec![(k(9), Arc::new(edges(1))), (k(10), Arc::new(edges(2)))]
        );
        assert_eq!(s.focus_none, HashSet::from([k(11)]));
        let f = s.frame(VIEW);
        assert_eq!(f.path, Some(path(10)));
        assert_eq!(ready_focus(&f), Some(Arc::new(edges(1))));
        assert_eq!(s.focus_job(), None);
        s.go(11);
        assert_eq!(s.frame(VIEW).path, Some(path(12)));
        assert_eq!(s.frame(VIEW).focus, FocusState::Unavailable);
        // image 10 itself drops out: its map goes with it
        s.set_files(without(20, &[3, 10]), 9);
        assert_eq!(s.focus_maps, vec![(k(9), Arc::new(edges(2)))]);
        assert_eq!(s.focus_none, HashSet::from([k(10)]));

        // another folder empties them; the wish stays
        s.open_folder(files(20));
        assert!(s.focus_maps.is_empty() && s.focus_none.is_empty());
        assert!(s.focus_wanted);
        assert_eq!(s.frame(VIEW).focus, FocusState::Reading);
        // no image: no map
        s.open_folder(Vec::new());
        assert_eq!(s.frame(VIEW).focus, FocusState::Unavailable);
        s.focus_wanted = false;
        assert_eq!(s.frame(VIEW).focus, FocusState::NotWanted);
    }

    #[test]
    fn a_focus_map_finished_after_set_files_is_discarded_and_its_session_is_dropped() {
        let mut s = state(1024);
        s.focus_wanted = true;
        show_with_statistics(&mut s, 10);
        let (generation, key, session) = lend_for_focus(&mut s);
        assert_eq!(s.session_bytes, 24 << 20);
        s.set_files(without(20, &[3]), 9);
        s.finish_focus(generation, key, session, Some(edges(1)), 1.0);
        // neither the old key nor the image's new one got it, and the session is gone
        assert!(s.focus_maps.is_empty() && s.focus_none.is_empty());
        assert_eq!(s.session_bytes, 0);
        assert!(s.detached.is_empty() && s.lent_paths.is_empty());
        assert!(!s.sessions.contains_key(&k(9)));
    }

    #[test]
    fn the_frame_gives_the_focus_map_over_the_whole_drawn_image_of_the_current_image_only() {
        let mut s = state(1024);
        s.focus_wanted = true;
        // an image turned on its side (flip 6) with an odd width and height: its half-size render
        // rounds up, so the map's cells reach one pixel past the image; the map still covers the
        // whole image, as the half-size view does
        let size = ImageSize {
            w: 4001,
            h: 3001,
            flip: 6,
        };
        s.sizes.insert(k(10), size);
        let half = s.accept(&out(&s, 10, Want::Fit), 100);
        assert!(half.is_some());
        let map = Arc::new(FocusMap {
            cell: 2,
            width: 2001,
            height: 1501,
            data: vec![0; 2001 * 1501],
        });
        s.keep_focus_map(k(10), map.clone());
        let whole = Rect::new(0, 0, 4001, 3001);

        // at fit: the whole image, with the same corners as the half-size view's own texture
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        let draw = f.draw.unwrap();
        assert_eq!(draw.id, half.unwrap());
        let fit_vis = geometry::visible(size, VIEW, geometry::fit_scale(size, VIEW), s.centre);
        let fit_uv = geometry::corner_uvs(size, whole, fit_vis);
        let FocusState::Ready { map: given, uv } = &f.focus else {
            panic!("expected the map, got {:?}", f.focus);
        };
        // the kept map itself, not a copy
        assert!(Arc::ptr_eq(given, &map));
        assert_eq!(*uv, fit_uv);
        assert_eq!(*uv, draw.uv);
        // the orientation is in them, and they span the whole image: 0 to 1 on both axes
        assert_eq!(*uv, [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]);

        // a new zoom level is being prepared: the fit view stays on the screen (V2), and the
        // map stays with it, placed as that view is
        s.zoom = Zoom::Scale(0.7);
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw, Some(draw));
        assert_eq!(
            f.focus,
            FocusState::Ready {
                map: map.clone(),
                uv: fit_uv
            }
        );
        // the sharp view arrives: the map is placed on the zoomed view
        let scaled = s.accept(&out(&s, 10, Want::Shown), 100);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), scaled);
        let zoomed_uv =
            geometry::corner_uvs(size, whole, geometry::visible(size, VIEW, 0.7, s.centre));
        assert_ne!(zoomed_uv, fit_uv);
        assert_eq!(
            f.focus,
            FocusState::Ready {
                map: map.clone(),
                uv: zoomed_uv
            }
        );
        // turned off: not given; on again: the kept map
        s.focus_wanted = false;
        assert_eq!(s.frame(VIEW).focus, FocusState::NotWanted);
        s.focus_wanted = true;
        assert_eq!(ready_focus(&s.frame(VIEW)), Some(map.clone()));

        // the next image: the earlier image's view is kept on the screen (V2), so no map is
        // given, not even the next image's own, until its view is drawn
        s.go(11);
        s.sizes.insert(k(11), SIZE);
        s.keep_focus_map(k(11), Arc::new(edges(2)));
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Preparing { .. }));
        assert_eq!(f.draw.map(|d| d.id), scaled);
        assert_eq!(f.focus, FocusState::Reading);
        let next = s.accept(&out(&s, 11, Want::Shown), 100);
        let f = s.frame(VIEW);
        assert_eq!(f.draw.map(|d| d.id), next);
        assert_eq!(ready_focus(&f), Some(Arc::new(edges(2))));

        // an error: nothing is drawn, and there is no map
        s.failed.insert(k(11), DecodeError::new("damaged"));
        let f = s.frame(VIEW);
        assert!(matches!(f.status, Status::Error(_)));
        assert_eq!(f.focus, FocusState::Unavailable);
    }

    // ---------- a view set from outside (ADR 0017 U6, T150) ----------

    #[test]
    fn the_frame_gives_the_centre_after_a_pan_and_a_zoom_around_an_anchor() {
        let mut s = state(1024);
        known(&mut s, 10);
        assert_eq!(s.frame(VIEW).centre, [0.5, 0.5]);
        s.set_zoom(Zoom::Scale(1.0), None, false);
        s.pan([100.0, -50.0]);
        let panned = geometry::pan_centre(SIZE, VIEW, 1.0, [0.5, 0.5], [100.0, -50.0]);
        assert_ne!(panned, [0.5, 0.5]);
        assert_eq!(s.frame(VIEW).centre, panned);
        s.set_zoom(Zoom::Scale(2.0), Some([0.0, 0.0]), false);
        let zoomed = geometry::zoom_centre(SIZE, VIEW, 1.0, 2.0, panned, Some([0.0, 0.0]));
        assert_ne!(zoomed, panned);
        assert_eq!(s.frame(VIEW).centre, zoomed);
    }

    #[test]
    fn set_view_sets_the_zoom_and_the_centre() {
        let mut s = state(1024);
        known(&mut s, 10);
        assert!(s.set_view(Zoom::Scale(1.0), [0.3, 0.6]));
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        assert_eq!(s.centre, [0.3, 0.6]);
        let f = s.frame(VIEW);
        assert_eq!(f.zoom, Zoom::Scale(1.0));
        assert_eq!(f.scale, Some(1.0));
        assert_eq!(f.centre, [0.3, 0.6]);
        // fit is taken as it is; the centre is stored and does not move the fitted image
        assert!(s.set_view(Zoom::Fit, [0.2, 0.9]));
        assert_eq!(s.zoom, Zoom::Fit);
        assert_eq!(s.centre, [0.2, 0.9]);
        // the view model's, not one image's: it stays when stepping (V6)
        s.go(11);
        assert_eq!(s.zoom, Zoom::Fit);
        assert_eq!(s.centre, [0.2, 0.9]);
    }

    #[test]
    fn set_view_clamps_the_scale_as_set_zoom_when_the_size_is_known() {
        let mut s = state(1024);
        known(&mut s, 10);
        assert!(s.set_view(Zoom::Scale(100.0), [0.5, 0.5]));
        assert_eq!(s.zoom, Zoom::Scale(MAX_SCALE));
        // the smallest scale is half of fit (25 % for 4000 × 3000 in 1000 × 800)
        assert!(s.set_view(Zoom::Scale(0.01), [0.5, 0.5]));
        assert_eq!(s.zoom, Zoom::Scale(0.125));
        let mut by_set_zoom = state(1024);
        known(&mut by_set_zoom, 10);
        by_set_zoom.set_zoom(Zoom::Scale(0.01), None, false);
        assert_eq!(s.zoom, by_set_zoom.zoom);
    }

    #[test]
    fn set_view_keeps_a_small_scale_while_the_size_is_not_known() {
        let mut s = state(1024);
        assert!(s.set_view(Zoom::Scale(0.2), [0.5, 0.5]));
        assert_eq!(s.zoom, Zoom::Scale(0.2));
        // only the largest scale applies
        assert!(s.set_view(Zoom::Scale(100.0), [0.5, 0.5]));
        assert_eq!(s.zoom, Zoom::Scale(MAX_SCALE));
        // not clamped again when the size becomes known
        assert!(s.set_view(Zoom::Scale(0.05), [0.5, 0.5]));
        known(&mut s, 10);
        assert_eq!(s.zoom, Zoom::Scale(0.05));
        assert_eq!(s.frame(VIEW).zoom, Zoom::Scale(0.05));
    }

    #[test]
    fn set_view_ignores_a_bad_scale_and_a_bad_centre() {
        let mut s = state(1024);
        known(&mut s, 10);
        assert!(s.set_view(Zoom::Scale(1.0), [0.3, 0.6]));
        let step = s.gesture_at;
        for zoom in [
            Zoom::Scale(f64::NAN),
            Zoom::Scale(f64::INFINITY),
            Zoom::Scale(-1.0),
            Zoom::Scale(0.0),
        ] {
            assert!(!s.set_view(zoom, [0.1, 0.1]), "{zoom:?}");
        }
        for centre in [[f64::NAN, 0.5], [0.5, f64::NAN], [f64::INFINITY, 0.5]] {
            // neither the centre nor the zoom given with it is taken
            assert!(!s.set_view(Zoom::Fit, centre), "{centre:?}");
        }
        assert_eq!(s.zoom, Zoom::Scale(1.0));
        assert_eq!(s.centre, [0.3, 0.6]);
        assert_eq!(s.gesture_at, step);
    }

    #[test]
    fn set_view_clamps_the_centre_to_the_image_and_does_not_move_it_to_the_middle() {
        let mut s = state(1024);
        known(&mut s, 10);
        assert!(s.set_view(Zoom::Scale(1.0), [-1.0, 2.0]));
        assert_eq!(s.centre, [0.0, 1.0]);
        // where the image fits the view, the centre is stored as given (not reset to 0.5)
        assert!(s.set_view(Zoom::Scale(0.2), [0.3, 0.6]));
        assert_eq!(s.centre, [0.3, 0.6]);
    }

    #[test]
    fn set_view_with_the_values_held_starts_no_new_gesture_step() {
        let mut s = state(1024);
        known(&mut s, 10);
        assert!(s.set_view(Zoom::Scale(1.0), [0.3, 0.6]));
        assert!(s.gesture_at.is_some());
        // as if the calls had paused
        let step = Some(Instant::now() - Duration::from_secs(1));
        s.gesture_at = step;
        let generation = s.generation;
        assert!(!s.set_view(Zoom::Scale(1.0), [0.3, 0.6]));
        assert_eq!(s.gesture_at, step);
        assert_eq!(s.generation, generation);
        // equal after the rules: both clamped to what is held
        assert!(s.set_view(Zoom::Scale(100.0), [-1.0, 2.0]));
        s.gesture_at = step;
        assert!(!s.set_view(Zoom::Scale(50.0), [-3.0, 7.0]));
        assert_eq!(s.gesture_at, step);
        assert_eq!(s.generation, generation);
        // a new value is a new step
        assert!(s.set_view(Zoom::Scale(1.0), [0.3, 0.6]));
        assert_ne!(s.gesture_at, step);
        assert_eq!(s.generation, generation);
    }

    #[test]
    fn the_views_of_a_set_view_are_asked_for_after_the_gesture_pause() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        assert!(s.set_view(Zoom::Scale(1.0), [0.3, 0.6]));
        // while the calls go on, nothing that depends on the position is asked for, but it is
        // protected
        assert!(s.wanted(false).is_empty());
        assert!(s.plan().is_empty());
        assert!(s.wanted(true).contains(&(k(10), Want::Shown)));
        assert!(s.next_deadline().is_some());
        // the calls paused: the exact view of the new position
        s.gesture_at = Some(Instant::now() - Duration::from_secs(1));
        assert_eq!(s.wanted(false)[0], (k(10), Want::Shown));
        let region = geometry::sensor_box(SIZE, geometry::visible(SIZE, VIEW, 1.0, [0.3, 0.6]));
        assert_eq!(
            s.plan()[0],
            Job::Render(
                k(10),
                JobView {
                    kind: ViewKind::Zoom,
                    region,
                    density_q: 1_000_000,
                }
            )
        );
    }

    // ---------- fine details (ADR 0027 W2, T313) ----------

    /// The job of a fine view of the whole image at `density_q` millionths.
    fn fine_job(density_q: u64) -> JobView {
        JobView {
            kind: ViewKind::Fine,
            region: geometry::whole(SIZE),
            density_q,
        }
    }

    fn is_fine(job: &Job) -> bool {
        matches!(job, Job::Render(_, v) if v.kind == ViewKind::Fine)
    }

    /// Fine details on, image 10 (RAW, at fit, 0.25) opened, past the dwell, with its shown, 100 %
    /// and ring views resident and its statistics known, so of the current image only the fine
    /// view is left. Returns the reduced view's id.
    fn ready_for_fine(s: &mut State<u32>) -> u64 {
        s.fine_details = true;
        with_session(s, 10);
        s.statistics.insert(k(10), ImageStatistics::Unavailable);
        let reduced = s.accept(&out(s, 10, Want::Shown), 100);
        assert!(s.accept(&out(s, 10, Want::Target), 100).is_some());
        assert!(s.accept(&out(s, 10, Want::Ring), 100).is_some());
        let Some(reduced) = reduced else {
            panic!("the reduced view was not accepted");
        };
        reduced
    }

    /// Picks on the background worker until it gives a fine render (the jobs before it are
    /// handed out and left running).
    fn pick_fine(s: &mut State<u32>) -> Option<(u64, Key, JobView, u32)> {
        for _ in 0..30 {
            match s.pick(false) {
                Some(Task::Render {
                    generation,
                    key,
                    view,
                    session,
                }) if view.kind == ViewKind::Fine => return Some((generation, key, view, session)),
                Some(_) => {}
                None => return None,
            }
        }
        None
    }

    #[test]
    fn with_fine_details_off_no_plan_holds_a_fine_render() {
        let mut s = state(1024);
        with_session(&mut s, 10);
        for zoom in [Zoom::Fit, Zoom::Scale(0.3)] {
            s.zoom = zoom;
            assert_eq!(s.mode, Source::Raw);
            assert!(!s.plan().iter().any(is_fine), "{zoom:?}");
            assert!(!s.wanted(true).iter().any(|(_, w)| *w == Want::Fine));
        }
    }

    #[test]
    fn with_fine_details_on_the_fine_render_is_last_after_the_dwell_and_not_in_a_gesture() {
        let mut s = state(1024);
        s.fine_details = true;
        with_session(&mut s, 10);
        // before the dwell only the shown view
        s.since = Instant::now();
        assert!(!s.plan().iter().any(is_fine));
        // after the dwell the fine view of the whole image at the fit scale, last
        s.since = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            s.plan().last(),
            Some(&Job::Render(k(10), fine_job(250_000)))
        );
        // still last once the other wants of the current image are satisfied
        let _ = ready_for_fine(&mut s);
        let plan = s.plan();
        assert_eq!(plan.last(), Some(&Job::Render(k(10), fine_job(250_000))));
        assert_eq!(plan.iter().filter(|j| j.key() == k(10)).count(), 1);
        // during a zoom gesture there is none
        s.set_zoom(Zoom::Scale(0.3), None, true);
        assert!(!s.plan().iter().any(is_fine));
    }

    #[test]
    fn only_the_background_worker_takes_a_fine_render() {
        let mut s = state(1024);
        let _ = ready_for_fine(&mut s);
        assert!(s.plan().iter().any(is_fine));
        // nothing else of the current image is left: the interactive worker takes nothing
        assert!(s.pick(true).is_none());
        let fine = pick_fine(&mut s);
        assert!(
            matches!(fine, Some((_, key, view, _)) if key == k(10) && view == fine_job(250_000))
        );
    }

    #[test]
    fn no_fine_render_in_jpeg_mode_for_a_jpeg_file_or_at_50_percent_or_more() {
        // JPEG mode
        let mut s = state(1024);
        s.fine_details = true;
        s.set_mode(Source::Jpeg);
        let jk = (10, Source::Jpeg);
        s.sizes.insert(jk, SIZE);
        s.sessions.insert(jk, Some(10));
        assert!(!s.plan().iter().any(is_fine));
        assert_eq!(s.frame(VIEW).fine, FineState::Paused(FinePause::Mode));
        // a plain JPEG file in RAW mode
        let mut s = state(1024);
        s.fine_details = true;
        let mut list = files(20);
        list[10] = PathBuf::from("img10.jpg");
        s.open_folder(list);
        s.go(10);
        s.since = Instant::now() - Duration::from_secs(1);
        s.sizes.insert(jk, SIZE);
        s.sessions.insert(jk, Some(10));
        assert_eq!(s.cur_key(), Some(jk));
        assert!(!s.plan().iter().any(is_fine));
        assert_eq!(s.frame(VIEW).fine, FineState::Paused(FinePause::Mode));
        // RAW at 50 % and at 100 %
        for scale in [0.5, 1.0] {
            let mut s = state(1024);
            s.fine_details = true;
            with_session(&mut s, 10);
            s.zoom = Zoom::Scale(scale);
            assert!(!s.plan().iter().any(is_fine), "{scale}");
            assert_eq!(s.frame(VIEW).fine, FineState::Paused(FinePause::Zoom));
        }
    }

    #[test]
    fn the_fine_view_replaces_the_reduced_view_once_it_is_accepted() {
        let mut s = state(1024);
        let reduced = ready_for_fine(&mut s);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), Some(reduced));
        assert_eq!(f.fine, FineState::Waiting);
        let fine = s.accept(&out(&s, 10, Want::Fine), 100);
        assert!(fine.is_some());
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), fine);
        assert_eq!(f.fine, FineState::Shown);
    }

    #[test]
    fn turning_fine_details_off_draws_the_reduced_view_at_once() {
        let mut s = state(1024);
        let reduced = ready_for_fine(&mut s);
        let fine = s.accept(&out(&s, 10, Want::Fine), 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), fine);
        s.fine_details = false;
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), Some(reduced));
        assert_eq!(f.fine, FineState::Off);
        // a fine view coming in now is not kept
        s.fine_details = true;
        let o = out(&s, 10, Want::Fine);
        s.fine_details = false;
        assert!(s.accept(&o, 100).is_none());
    }

    #[test]
    fn a_failed_fine_render_shows_no_error_and_pauses_fine_details_for_the_image() {
        let mut s = state(1024);
        let reduced = ready_for_fine(&mut s);
        let Some((generation, key, view, session)) = pick_fine(&mut s) else {
            panic!("no fine render");
        };
        s.finish_render(
            generation,
            key,
            view,
            session,
            Err(DecodeError::new("render failed")),
            1.0,
        );
        assert!(!s.failed.contains_key(&k(10)));
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), Some(reduced));
        assert_eq!(f.fine, FineState::Paused(FinePause::Failed));
        assert!(!s.plan().iter().any(is_fine));
        assert!(!s.inflight.iter().any(|f| f.view.kind == ViewKind::Fine));
    }

    #[test]
    fn a_failed_fine_render_drops_the_session_and_a_later_zoom_opens_a_new_one() {
        let mut s = state(1024);
        let _ = ready_for_fine(&mut s);
        let Some((generation, key, view, session)) = pick_fine(&mut s) else {
            panic!("no fine render");
        };
        s.finish_render(
            generation,
            key,
            view,
            session,
            Err(DecodeError::new("render failed")),
            1.0,
        );
        assert!(!s.sessions.contains_key(&k(10)));
        // a zoom to 100 % at the top left corner, outside the resident 100 % view and ring, needs
        // a new render, so a new session
        s.zoom = Zoom::Scale(1.0);
        s.centre = [0.1, 0.1];
        let Some(Task::Open {
            generation, key, ..
        }) = s.pick(true)
        else {
            panic!("expected an open");
        };
        assert_eq!(key, k(10));
        s.finish_open(generation, key, Ok((10, SIZE)), mb24, 1.0);
        let Some(Task::Render {
            generation,
            key,
            view,
            session,
        }) = s.pick(true)
        else {
            panic!("expected a render");
        };
        assert_eq!(view.kind, ViewKind::Zoom);
        let region = view.region;
        let image = Rgb {
            width: region.w,
            height: region.h,
            data: vec![0; (region.w * region.h * 3) as usize],
        };
        s.finish_render(generation, key, view, session, Ok(image), 1.0);
        let outs: Vec<ViewOut> = std::mem::take(&mut s.outbox);
        for o in &outs {
            assert!(s.accept(o, 100).is_some());
        }
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.fine, FineState::Paused(FinePause::Zoom));
    }

    #[test]
    fn a_fine_render_refused_by_the_texture_budget_pauses_fine_details() {
        // 1 MiB: the fine view of 4000×3000 at 0.25 (about 3 MB) does not fit
        let mut s = state(1);
        let reduced = ready_for_fine(&mut s);
        let refused = s.stats.refused;
        assert!(pick_fine(&mut s).is_none());
        assert!(s.stats.refused > refused);
        assert!(!s.refused.contains(&(s.generation, k(10))));
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), Some(reduced));
        assert_eq!(f.fine, FineState::Paused(FinePause::Failed));
        assert!(!s.plan().iter().any(is_fine));
    }

    #[test]
    fn a_rejected_fine_view_shows_no_error_and_pauses_fine_details() {
        let mut s = state(1024);
        let reduced = ready_for_fine(&mut s);
        let o = out(&s, 10, Want::Fine);
        s.reject(&o, "too large for the GPU");
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), Some(reduced));
        assert_eq!(f.fine, FineState::Paused(FinePause::Failed));
    }

    #[test]
    fn an_image_that_failed_for_fine_details_stays_paused_after_set_files_moves_it() {
        let mut s = state(1024);
        let _ = ready_for_fine(&mut s);
        let o = out(&s, 10, Want::Fine);
        s.reject(&o, "too large for the GPU");
        assert_eq!(s.frame(VIEW).fine, FineState::Paused(FinePause::Failed));
        // image 3 drops out: image 10 is now at position 9
        s.set_files(without(20, &[3]), 9);
        assert_eq!(s.cur_key(), Some(k(9)));
        assert_eq!(s.frame(VIEW).fine, FineState::Paused(FinePause::Failed));
        // a new folder starts again
        s.open_folder(files(20));
        s.go(9);
        known(&mut s, 9);
        assert_eq!(s.frame(VIEW).fine, FineState::Waiting);
    }

    #[test]
    fn no_fine_render_is_picked_while_the_current_file_is_being_released() {
        let mut s = state(1024);
        let _ = ready_for_fine(&mut s);
        s.begin_release(&path(10));
        assert!(pick_fine(&mut s).is_none());
        s.end_release(&path(10));
        assert!(pick_fine(&mut s).is_some());
    }

    #[test]
    fn a_fine_view_finished_after_a_step_is_kept_and_drawn_when_stepping_back() {
        let mut s = state(1024);
        let _ = ready_for_fine(&mut s);
        let Some((generation, key, view, session)) = pick_fine(&mut s) else {
            panic!("no fine render");
        };
        s.go(11);
        s.finish_render(generation, key, view, session, Ok(rendered(view)), 1.0);
        let outs: Vec<ViewOut> = std::mem::take(&mut s.outbox);
        assert_eq!(outs.len(), 1);
        let fine = outs.first().and_then(|o| s.accept(o, 100));
        assert!(fine.is_some());
        // not of the current image: never drawn for image 11 (V2)
        assert_ne!(s.frame(VIEW).draw.map(|d| d.id), fine);
        s.go(10);
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), fine);
        assert_eq!(f.fine, FineState::Shown);
    }

    #[test]
    fn after_a_zoom_gesture_the_earlier_fine_view_is_drawn_until_the_new_one_arrives() {
        let mut s = state(1024);
        s.fine_details = true;
        with_session(&mut s, 10);
        s.zoom = Zoom::Scale(0.4);
        assert!(s.accept(&out(&s, 10, Want::Shown), 100).is_some());
        let fine4 = s.accept(&out(&s, 10, Want::Fine), 100);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), fine4);
        // a gesture out to 0.3: the fine view of 0.4 still serves (not below half its density)
        s.set_zoom(Zoom::Scale(0.3), None, true);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), fine4);
        pause_the_gesture(&mut s);
        assert!(s.plan().contains(&Job::Render(k(10), fine_job(300_000))));
        let f = s.frame(VIEW);
        assert_eq!(f.status, Status::Ready);
        assert_eq!(f.draw.map(|d| d.id), fine4);
        // the new fine view replaces it in one swap
        assert!(s.accept(&out(&s, 10, Want::Shown), 100).is_some());
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), fine4);
        let fine3 = s.accept(&out(&s, 10, Want::Fine), 100);
        assert!(fine3.is_some());
        let f = s.frame(VIEW);
        assert_eq!(f.draw.map(|d| d.id), fine3);
        assert_eq!(f.fine, FineState::Shown);
    }

    #[test]
    fn the_drawn_fine_view_and_the_reduced_view_are_both_protected() {
        let mut s = state(1);
        s.fine_details = true;
        with_session(&mut s, 10);
        known(&mut s, 11);
        let reduced = s.accept(&out(&s, 10, Want::Shown), 400 << 10);
        let fine = s.accept(&out(&s, 10, Want::Fine), 400 << 10);
        assert_eq!(s.frame(VIEW).draw.map(|d| d.id), fine);
        let (Some(reduced), Some(fine)) = (reduced, fine) else {
            panic!("not accepted");
        };
        let p = s.protected_views();
        assert!(p.contains(&reduced) && p.contains(&fine));
        // filling the budget evicts neither: the neighbour's view is discarded instead
        assert!(s.accept(&out(&s, 11, Want::Fit), 400 << 10).is_none());
        assert!(s.take_dropped().is_empty());
        assert!(s.resident.iter().any(|r| r.id == reduced));
        assert!(s.resident.iter().any(|r| r.id == fine));
        assert!(!s.plan().contains(&Job::Render(k(10), reduced_job(250_000))));
    }

    #[test]
    fn the_interactive_worker_takes_no_open_asked_only_for_the_fine_view() {
        let mut s = state(1024);
        let _ = ready_for_fine(&mut s);
        s.drop_session(k(10));
        assert!(s.plan().contains(&Job::Open(k(10))));
        assert!(s.pick(true).is_none());
        let mut opened = false;
        for _ in 0..30 {
            match s.pick(false) {
                Some(Task::Open { key, .. }) if key == k(10) => {
                    opened = true;
                    break;
                }
                Some(_) => {}
                None => break,
            }
        }
        assert!(opened, "the background worker opens it for the fine view");
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_ne!(fnv(b"a"), fnv(b"b"));
    }
}
