//! P4 (ADR 0008 §8.2, §2.3): core scheduling without UI.
//!
//! A simulated user steps through a folder, zooms and switches display mode, with real timing. A
//! scheduler with two workers prepares views ahead, according to a strategy. One worker is
//! interactive and takes only jobs of the current image; the other is a background worker.
//!
//! Every request for a view (an image change, Z, or a mode switch) is logged:
//! - served at once from the cache (a hit), or
//! - finished after N ms, or
//! - not shown, because the user moved on first.
//!
//! The time measured is "view ready in memory". Presenting it on screen is P2/P3.
//!
//! Views:
//! - **Fit:** the half-size render. This is the RAW view at fit.
//! - **Zoom:** the visible 100 % region at the view position. The position is the centre (V6
//!   carries it over between images).
//! - **Ring:** the view region expanded by half a view on each side, for panning.
//! - **Jpeg:** the decoded largest embedded preview. This is the JPEG mode (G4).
//!
//! The scheduler is generic over [`Backend`], so its logic can be tested without LibRaw.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub trait Backend: Send + Sync + 'static {
    type S: Send + 'static;
    /// Opens and unpacks. Returns the session and the visible sensor size and LibRaw flip.
    fn open(&self, path: &Path) -> Result<(Self::S, u32, u32, i32), String>;
    fn half(&self, s: &mut Self::S, quality: i32) -> Result<Vec<u8>, String>;
    fn region(&self, s: &mut Self::S, quality: i32, r: [u32; 4]) -> Result<Vec<u8>, String>;
    fn jpeg(&self, path: &Path) -> Result<Vec<u8>, String>;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum View {
    Fit,
    Zoom,
    Ring,
    Jpeg,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Raw,
    Jpeg,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Job {
    Unpack(usize),
    Render(usize, View),
}

impl Job {
    fn pos(self) -> usize {
        match self {
            Job::Unpack(p) | Job::Render(p, _) => p,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Strategy {
    pub name: String,
    /// Pre-render the current image's other RAW view (the zoom target at fit; fit when zoomed).
    pub cur_prerender: bool,
    /// Neighbours prepared ahead in the direction of travel (plus one behind when > 0).
    pub neighbours: usize,
    pub ring: bool,
    /// Prepare the current image in the other display mode too (S3).
    pub cross_mode: bool,
    /// Prefetch starts only after the image has been current this long.
    pub dwell: Duration,
    /// Render the zoom view as 1024-px grid tiles (today's app) instead of exactly the visible region.
    pub tiles: bool,
    pub quality: i32,
    pub max_sessions: usize,
    /// Put the other display mode after the neighbours in the queue (P4 finding; P5).
    pub cross_late: bool,
    /// P5: the budget covers unpacked sessions as well as views. Eviction spares what the plan
    /// still wants, and prefetch does not start when it would not fit.
    pub total_budget: bool,
    /// Budget for this run, from the name suffix `@<MB>` (else the default).
    pub budget_mb: Option<usize>,
}

pub fn strategy(full_name: &str) -> Option<Strategy> {
    let (name, budget_mb) = match full_name.split_once('@') {
        Some((n, mb)) => (n, Some(mb.parse().ok()?)),
        None => (full_name, None),
    };
    let base = Strategy {
        name: full_name.into(),
        cur_prerender: false,
        neighbours: 0,
        ring: false,
        cross_mode: false,
        dwell: Duration::ZERO,
        tiles: false,
        quality: 2, // PPG (ADR 0008 §11)
        max_sessions: 1,
        cross_late: false,
        total_budget: false,
        budget_mb,
    };
    let full = Strategy { cur_prerender: true, neighbours: 2, ring: true, cross_mode: true, max_sessions: 4, ..base.clone() };
    Some(match name {
        // Only what is shown, on demand (no pre-rendering).
        "none" => base,
        // ADR 0008 §2.3 step 1: the current image's zoom target.
        "cur" => Strategy { cur_prerender: true, max_sessions: 2, ..base },
        // §2.3 steps 1–2: plus neighbours in the direction of travel, ring, other display mode.
        "full" => full,
        "full-tiles" => Strategy { tiles: true, ..full },
        "full-dwell" => Strategy { dwell: Duration::from_millis(150), ..full },
        "full-ahd" => Strategy { quality: 3, ..full },
        // P5 (owner, 2026-11-05): full + 150 ms dwell + other display mode after the neighbours +
        // exact visible region, under one budget for sessions and views.
        "p5" => Strategy {
            dwell: Duration::from_millis(150),
            cross_late: true,
            total_budget: true,
            max_sessions: usize::MAX,
            ..full
        },
        _ => return None,
    })
}

pub struct Config {
    pub view_w: u32,
    pub view_h: u32,
    pub budget_bytes: usize,
    pub tile: u32,
}

struct Cached {
    _data: Vec<u8>,
    bytes: usize,
}

struct State<S> {
    generation: u64,
    cur: usize,
    dir: isize,
    mode: Mode,
    zoomed: bool,
    since: Instant,
    /// `None` while a worker uses the session.
    sessions: HashMap<usize, Option<S>>,
    sess_lru: Vec<usize>,
    sizes: HashMap<usize, (u32, u32, i32)>,
    views: HashMap<(usize, View), Cached>,
    view_lru: Vec<(usize, View)>,
    view_bytes: usize,
    sess_bytes: HashMap<usize, usize>,
    last_sess_bytes: usize,
    last_jpeg_bytes: usize,
    evicted_views: usize,
    evicted_sessions: usize,
    peak_tracked: usize,
    running: HashSet<Job>,
    failed: HashSet<Job>,
    /// (generation, pos, view, finished at) per outstanding request.
    waits: Vec<(u64, usize, View, Option<Instant>)>,
    stop: bool,
    jobs: Vec<JobRec>,
    errors: Vec<String>,
}

#[derive(Clone, Copy)]
struct JobRec {
    generation: u64,
    job: Job,
    start: Instant,
    end: Instant,
    ok: bool,
}

struct Shared<B: Backend> {
    b: B,
    files: Vec<PathBuf>,
    len: usize,
    strat: Strategy,
    cfg: Config,
    m: Mutex<State<B::S>>,
    cv: Condvar,
}

impl<B: Backend> Shared<B> {
    fn path(&self, pos: usize) -> &Path {
        &self.files[pos % self.files.len()]
    }
    fn lock(&self) -> MutexGuard<'_, State<B::S>> {
        self.m.lock().unwrap()
    }
}

fn primary(mode: Mode, zoomed: bool) -> View {
    match (mode, zoomed) {
        (Mode::Jpeg, _) => View::Jpeg,
        (Mode::Raw, true) => View::Zoom,
        (Mode::Raw, false) => View::Fit,
    }
}

fn step(cur: usize, d: isize, len: usize) -> Option<usize> {
    let p = cur as isize + d;
    (p >= 0 && (p as usize) < len).then_some(p as usize)
}

/// Views wanted now, most important first (whether done or not).
fn wanted<B: Backend>(sh: &Shared<B>, g: &State<B::S>, ignore_dwell: bool) -> Vec<(usize, View)> {
    let st = &sh.strat;
    let mut v: Vec<(usize, View)> = Vec::new();
    if g.cur == usize::MAX {
        return v; // nothing shown yet (after a reset)
    }
    let add = |pos: usize, view: View, v: &mut Vec<(usize, View)>| {
        if !v.contains(&(pos, view)) {
            v.push((pos, view));
        }
    };
    let cur = g.cur;
    let p = primary(g.mode, g.zoomed);
    let other = if g.zoomed { View::Fit } else { View::Zoom };
    let cross = if g.mode == Mode::Raw { View::Jpeg } else { View::Fit };
    add(cur, p, &mut v);
    if ignore_dwell || g.since.elapsed() >= st.dwell {
        if st.cur_prerender && g.mode == Mode::Raw {
            add(cur, other, &mut v);
        }
        if st.cross_mode && !st.cross_late {
            add(cur, cross, &mut v);
        }
        for k in 1..=st.neighbours {
            if let Some(j) = step(cur, g.dir * k as isize, sh.len) {
                add(j, p, &mut v);
                if st.cur_prerender && g.mode == Mode::Raw {
                    add(j, other, &mut v);
                }
            }
        }
        if st.cross_mode && st.cross_late {
            add(cur, cross, &mut v);
        }
        if st.ring && g.mode == Mode::Raw {
            add(cur, View::Ring, &mut v);
        }
        if st.neighbours > 0 {
            if let Some(j) = step(cur, -g.dir, sh.len) {
                add(j, p, &mut v);
            }
        }
    }
    v
}

/// Jobs wanted now, most important first (already done or failed ones left out).
fn plan<B: Backend>(sh: &Shared<B>, g: &State<B::S>) -> Vec<Job> {
    let mut v: Vec<Job> = Vec::new();
    for (pos, view) in wanted(sh, g, false) {
        if g.views.contains_key(&(pos, view)) || g.failed.contains(&Job::Render(pos, view)) {
            continue;
        }
        if view != View::Jpeg && !g.sessions.contains_key(&pos) {
            let u = Job::Unpack(pos);
            if g.failed.contains(&u) {
                continue;
            }
            if !v.contains(&u) {
                v.push(u);
            }
        }
        v.push(Job::Render(pos, view));
    }
    v
}

fn tracked<S>(g: &State<S>) -> usize {
    g.view_bytes + g.sess_bytes.values().sum::<usize>()
}

/// Expected memory of a job's result (for admission under the total budget).
fn estimate<B: Backend>(sh: &Shared<B>, g: &State<B::S>, job: Job) -> usize {
    let (vw, vh) = (sh.cfg.view_w as usize, sh.cfg.view_h as usize);
    match job {
        Job::Unpack(_) => g.last_sess_bytes,
        Job::Render(_, View::Jpeg) => g.last_jpeg_bytes,
        Job::Render(p, View::Fit) => g.sizes.get(&p).map(|s| (s.0 as usize / 2) * (s.1 as usize / 2) * 3).unwrap_or(64 << 20),
        Job::Render(_, View::Zoom) => vw * vh * 3,
        Job::Render(_, View::Ring) => vw * vh * 12,
    }
}

/// Total budget (P5): evicts least recently used views and idle sessions that the plan does not
/// want, never anything of the current image. Returns whether the total is within the budget.
fn enforce_budget<B: Backend>(sh: &Shared<B>, g: &mut State<B::S>, extra: usize) -> bool {
    let budget = sh.cfg.budget_bytes;
    if tracked(g) + extra <= budget {
        return true;
    }
    // what the plan wants regardless of the dwell, so prepared neighbours survive an image change
    let want = wanted(sh, g, true);
    let cur = g.cur;
    // a session is still needed while a wanted RAW view of its image is not rendered
    let needs_session = |g: &State<B::S>, p: usize| want.iter().any(|w| w.0 == p && w.1 != View::Jpeg && !g.views.contains_key(w));
    while tracked(g) + extra > budget {
        if let Some(k) = g.view_lru.iter().copied().find(|k| k.0 != cur && !want.contains(k)) {
            g.view_lru.retain(|x| *x != k);
            if let Some(c) = g.views.remove(&k) {
                g.view_bytes -= c.bytes;
                g.evicted_views += 1;
            }
            continue;
        }
        let victim = g.sess_lru.iter().copied().find(|k| *k != cur && matches!(g.sessions.get(k), Some(Some(_))) && !needs_session(g, *k));
        if let Some(k) = victim {
            g.sessions.remove(&k);
            g.sess_lru.retain(|x| *x != k);
            g.sess_bytes.remove(&k);
            g.evicted_sessions += 1;
            continue;
        }
        return false;
    }
    true
}

/// Region of `view` in sensor coordinates (the view position is the centre).
fn view_rect(cfg: &Config, size: (u32, u32, i32), view: View) -> [u32; 4] {
    let (w, h, flip) = size;
    let (vw, vh) = if flip == 5 || flip == 6 { (cfg.view_h, cfg.view_w) } else { (cfg.view_w, cfg.view_h) };
    let (vw, vh) = (vw.min(w), vh.min(h));
    let (rw, rh) = if view == View::Ring { ((vw * 2).min(w), (vh * 2).min(h)) } else { (vw, vh) };
    // even origin keeps the Bayer phase
    let x = ((w - rw) / 2) & !1;
    let y = ((h - rh) / 2) & !1;
    [x, y, rw, rh]
}

enum Out<S> {
    Session(S, u32, u32, i32),
    View(Vec<u8>, Option<S>),
    Failed(String, Option<S>),
}

fn execute<B: Backend>(sh: &Shared<B>, job: Job, sess: Option<B::S>, rect: Option<[u32; 4]>) -> Out<B::S> {
    let q = sh.strat.quality;
    match job {
        Job::Unpack(p) => match sh.b.open(sh.path(p)) {
            Ok((s, w, h, f)) => Out::Session(s, w, h, f),
            Err(e) => Out::Failed(e, None),
        },
        Job::Render(p, View::Jpeg) => match sh.b.jpeg(sh.path(p)) {
            Ok(d) => Out::View(d, None),
            Err(e) => Out::Failed(e, None),
        },
        Job::Render(_, view) => {
            let mut s = sess.expect("session");
            let r = match view {
                View::Fit => sh.b.half(&mut s, q),
                _ => {
                    let r = rect.expect("rect");
                    if sh.strat.tiles {
                        render_tiles(sh, &mut s, q, r)
                    } else {
                        sh.b.region(&mut s, q, r)
                    }
                }
            };
            match r {
                Ok(d) => Out::View(d, Some(s)),
                Err(e) => Out::Failed(e, Some(s)),
            }
        }
    }
}

/// Today's app: the grid tiles (tile × tile) that cover `r`, each rendered on its own.
fn render_tiles<B: Backend>(sh: &Shared<B>, s: &mut B::S, q: i32, r: [u32; 4]) -> Result<Vec<u8>, String> {
    let t = sh.cfg.tile;
    let mut all = Vec::new();
    for ty in (r[1] / t)..=((r[1] + r[3] - 1) / t) {
        for tx in (r[0] / t)..=((r[0] + r[2] - 1) / t) {
            all.extend(sh.b.region(s, q, [tx * t, ty * t, t, t])?);
        }
    }
    Ok(all)
}

fn pick<B: Backend>(sh: &Shared<B>, g: &mut State<B::S>, interactive: bool) -> Option<(Job, Option<B::S>, Option<[u32; 4]>)> {
    for job in plan(sh, g) {
        if g.running.contains(&job) || (interactive && job.pos() != g.cur) {
            continue;
        }
        if sh.strat.total_budget && job.pos() != g.cur {
            let e = estimate(sh, g, job);
            if !enforce_budget(sh, g, e) {
                continue; // prefetch would not fit in the budget
            }
        }
        match job {
            Job::Unpack(_) | Job::Render(_, View::Jpeg) => return Some((job, None, None)),
            Job::Render(p, view) => {
                let Some(slot) = g.sessions.get_mut(&p) else { continue };
                let Some(s) = slot.take() else { continue };
                let rect = (view != View::Fit).then(|| view_rect(&sh.cfg, g.sizes[&p], view));
                return Some((job, Some(s), rect));
            }
        }
    }
    None
}

fn touch<T: PartialEq + Copy>(lru: &mut Vec<T>, k: T) {
    lru.retain(|x| *x != k);
    lru.push(k);
}

fn finish<B: Backend>(sh: &Shared<B>, g: &mut State<B::S>, job: Job, generation: u64, out: Out<B::S>, start: Instant, end: Instant) {
    let ok = !matches!(out, Out::Failed(..));
    g.jobs.push(JobRec { generation, job, start, end, ok });
    if generation != g.generation {
        return; // folder was reset meanwhile; results (and sessions) are dropped
    }
    let p = job.pos();
    match out {
        Out::Session(s, w, h, f) => {
            g.sessions.insert(p, Some(s));
            g.sizes.insert(p, (w, h, f));
            let sb = w as usize * h as usize * 2; // 16-bit raw data
            g.sess_bytes.insert(p, sb);
            g.last_sess_bytes = sb;
            touch(&mut g.sess_lru, p);
            // evict least recently used idle sessions over the limit (never the current one)
            while g.sessions.len() > sh.strat.max_sessions {
                let cur = g.cur;
                let victim = g.sess_lru.iter().copied().find(|k| *k != cur && *k != p && matches!(g.sessions.get(k), Some(Some(_))));
                let Some(k) = victim else { break };
                g.sessions.remove(&k);
                g.sess_lru.retain(|x| *x != k);
                g.sess_bytes.remove(&k);
                g.evicted_sessions += 1;
            }
        }
        Out::View(data, s) => {
            if let Some(s) = s {
                if let Some(slot) = g.sessions.get_mut(&p) {
                    *slot = Some(s);
                }
            }
            let Job::Render(_, view) = job else { unreachable!() };
            let bytes = data.len();
            g.views.insert((p, view), Cached { _data: data, bytes });
            g.view_bytes += bytes;
            touch(&mut g.view_lru, (p, view));
            for w in g.waits.iter_mut() {
                if w.0 == generation && w.1 == p && w.2 == view && w.3.is_none() {
                    w.3 = Some(end);
                }
            }
            if view == View::Jpeg {
                g.last_jpeg_bytes = bytes;
            }
            while !sh.strat.total_budget && g.view_bytes > sh.cfg.budget_bytes {
                let cur = g.cur;
                let Some(k) = g.view_lru.iter().copied().find(|k| k.0 != cur) else { break };
                g.view_lru.retain(|x| *x != k);
                if let Some(c) = g.views.remove(&k) {
                    g.view_bytes -= c.bytes;
                    g.evicted_views += 1;
                }
            }
        }
        Out::Failed(e, s) => {
            if let Some(s) = s {
                if let Some(slot) = g.sessions.get_mut(&p) {
                    *slot = Some(s);
                }
            }
            g.errors.push(format!("{job:?}: {e}"));
            g.failed.insert(job);
        }
    }
    if sh.strat.total_budget {
        enforce_budget(sh, g, 0);
    }
    let t = tracked(g);
    g.peak_tracked = g.peak_tracked.max(t);
}

fn worker<B: Backend>(sh: Arc<Shared<B>>, interactive: bool) {
    let mut g = sh.lock();
    loop {
        if g.stop {
            return;
        }
        if let Some((job, sess, rect)) = pick(&sh, &mut g, interactive) {
            let generation = g.generation;
            g.running.insert(job);
            drop(g);
            let start = Instant::now();
            let out = execute(&sh, job, sess, rect);
            let end = Instant::now();
            g = sh.lock();
            g.running.remove(&job);
            finish(&sh, &mut g, job, generation, out, start, end);
            sh.cv.notify_all();
        } else {
            g = sh.cv.wait_timeout(g, Duration::from_millis(5)).unwrap().0;
        }
    }
}

// ---------- the simulated user ----------

struct Rec {
    generation: u64,
    scen: String,
    label: String,
    pos: usize,
    view: View,
    t_req: Instant,
    hit: bool,
    wait: Option<usize>,
    left: Option<Instant>,
}

pub struct Driver<B: Backend> {
    sh: Arc<Shared<B>>,
    threads: Vec<std::thread::JoinHandle<()>>,
    recs: Vec<Rec>,
    open: Vec<usize>,
    scen: String,
    t0: Instant,
}

impl<B: Backend> Driver<B> {
    pub fn new(b: B, files: Vec<PathBuf>, len: usize, strat: Strategy, cfg: Config) -> Self {
        let sh = Arc::new(Shared {
            b,
            files,
            len,
            strat,
            cfg,
            m: Mutex::new(State {
                generation: 0,
                cur: 0,
                dir: 1,
                mode: Mode::Raw,
                zoomed: false,
                since: Instant::now(),
                sessions: HashMap::new(),
                sess_lru: Vec::new(),
                sizes: HashMap::new(),
                views: HashMap::new(),
                view_lru: Vec::new(),
                view_bytes: 0,
                sess_bytes: HashMap::new(),
                last_sess_bytes: 150 << 20,
                last_jpeg_bytes: 150 << 20,
                evicted_views: 0,
                evicted_sessions: 0,
                peak_tracked: 0,
                running: HashSet::new(),
                failed: HashSet::new(),
                waits: Vec::new(),
                stop: true, // idle until the first reset
                jobs: Vec::new(),
                errors: Vec::new(),
            }),
            cv: Condvar::new(),
        });
        let threads = [true, false]
            .into_iter()
            .map(|i| {
                let s = sh.clone();
                std::thread::spawn(move || {
                    // wait for start
                    loop {
                        if !s.lock().stop {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    worker(s, i)
                })
            })
            .collect();
        Driver { sh, threads, recs: Vec::new(), open: Vec::new(), scen: String::new(), t0: Instant::now() }
    }

    pub fn scenario(&mut self, name: &str) {
        self.scen = name.into();
    }

    fn close_open(&mut self, now: Instant) {
        for i in self.open.drain(..) {
            self.recs[i].left = Some(now);
        }
    }

    /// Logs a request for what is shown now. Call with the lock held after a state change.
    fn request(&mut self, g: &mut State<B::S>, label: &str, now: Instant) {
        let view = primary(g.mode, g.zoomed);
        let hit = g.views.contains_key(&(g.cur, view));
        if hit {
            touch(&mut g.view_lru, (g.cur, view));
        }
        let wait = (!hit).then(|| {
            g.waits.push((g.generation, g.cur, view, None));
            g.waits.len() - 1
        });
        self.recs.push(Rec { generation: g.generation, scen: self.scen.clone(), label: label.into(), pos: g.cur, view, t_req: now, hit, wait, left: None });
        if !hit {
            self.open.push(self.recs.len() - 1);
        }
    }

    /// Empties every cache (a new folder; the OS file cache cannot be emptied here).
    pub fn reset(&mut self, mode: Mode) {
        let now = Instant::now();
        self.close_open(now);
        {
            let mut g = self.sh.lock();
            g.generation += 1;
            g.stop = false;
            g.sessions.clear();
            g.sess_lru.clear();
            g.sizes.clear();
            g.views.clear();
            g.view_lru.clear();
            g.view_bytes = 0;
            g.sess_bytes.clear();
            g.failed.clear();
            g.mode = mode;
            g.zoomed = false;
            g.cur = usize::MAX; // nothing is current until `go`
        }
        // let running jobs of the old folder finish, so they don't compete with the next scenario
        loop {
            if self.sh.lock().running.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn go(&mut self, pos: usize, label: &str) {
        let now = Instant::now();
        self.close_open(now);
        let sh = self.sh.clone();
        let mut g = sh.lock();
        g.dir = if g.cur != usize::MAX && pos < g.cur { -1 } else { 1 };
        g.cur = pos;
        g.since = now;
        self.request(&mut g, label, now);
        sh.cv.notify_all();
    }

    pub fn zoom(&mut self, z: bool, label: &str) {
        let now = Instant::now();
        self.close_open(now);
        let sh = self.sh.clone();
        let mut g = sh.lock();
        g.zoomed = z;
        self.request(&mut g, label, now);
        sh.cv.notify_all();
    }

    pub fn set_mode(&mut self, m: Mode, label: &str) {
        let now = Instant::now();
        self.close_open(now);
        let sh = self.sh.clone();
        let mut g = sh.lock();
        g.mode = m;
        self.request(&mut g, label, now);
        sh.cv.notify_all();
    }

    pub fn cur(&self) -> usize {
        self.sh.lock().cur
    }

    pub fn mode(&self) -> Mode {
        self.sh.lock().mode
    }

    /// Waits until what is shown is ready, at most `max`.
    pub fn wait_ready(&mut self, max: Duration) {
        let end = Instant::now() + max;
        while Instant::now() < end {
            let ready = {
                let g = self.sh.lock();
                self.open.iter().all(|i| self.recs[*i].wait.is_none_or(|w| g.waits[w].3.is_some()))
            };
            if ready {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn sleep(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    /// Stops the workers and returns (event rows, summary line).
    pub fn finish(mut self, group: &str) -> (String, String) {
        let now = Instant::now();
        self.close_open(now);
        {
            let mut g = self.sh.lock();
            g.stop = true;
        }
        self.sh.cv.notify_all();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        let g = self.sh.lock();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let strat = &self.sh.strat.name;
        let mut rows = String::new();
        let mut requested: HashSet<(u64, usize, View)> = HashSet::new();
        for r in &self.recs {
            requested.insert((r.generation, r.pos, r.view));
            let done = r.wait.and_then(|w| g.waits[w].3);
            let (shown, lat) = if r.hit {
                (true, Some(0.0))
            } else {
                match (done, r.left) {
                    (Some(d), Some(l)) if d <= l => (true, Some(ms(d - r.t_req))),
                    (Some(d), None) => (true, Some(ms(d - r.t_req))),
                    _ => (false, None),
                }
            };
            let on_screen = r.left.map(|l| ms(l - r.t_req)).unwrap_or(f64::NAN);
            let _ = writeln!(
                rows,
                "EV,{group},{strat},{},{},{},{:?},{:.1},{},{},{},{:.1}",
                r.scen,
                r.label,
                r.pos,
                r.view,
                ms(r.t_req - self.t0),
                lat.map(|l| format!("{l:.1}")).unwrap_or_default(),
                shown as u8,
                r.hit as u8,
                on_screen
            );
        }
        // work done for views that were never requested (wasted pre-rendering)
        let (mut jobs_ms, mut wasted_ms, mut wasted_n) = (0.0, 0.0, 0usize);
        let viewed_pos: HashSet<(u64, usize)> = requested.iter().filter(|k| k.2 != View::Jpeg).map(|k| (k.0, k.1)).collect();
        for j in &g.jobs {
            let d = ms(j.end - j.start);
            jobs_ms += d;
            let wasted = match j.job {
                Job::Unpack(p) => !viewed_pos.contains(&(j.generation, p)),
                Job::Render(p, v) => !requested.contains(&(j.generation, p, v)),
            };
            if wasted && j.ok {
                wasted_ms += d;
                wasted_n += 1;
            }
        }
        let wall = ms(now - self.t0);
        let cpu = crate::sys::cpu_ms();
        let (_, peak) = crate::sys::private_mb();
        let errs = g.errors.len();
        let sum = format!(
            "SUM,{group},{strat},{wall:.0},{cpu:.0},{:.2},{peak:.0},{},{jobs_ms:.0},{wasted_n},{wasted_ms:.0},{errs},{},{},{:.0}",
            cpu / wall,
            g.jobs.len(),
            g.evicted_views,
            g.evicted_sessions,
            g.peak_tracked as f64 / 1048576.0
        );
        for e in g.errors.iter().take(5) {
            eprintln!("job error: {e}");
        }
        (rows, sum)
    }
}

// ---------- scenarios (ADR 0008 §8.3) ----------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

fn mname(m: Mode) -> &'static str {
    if m == Mode::Raw { "raw" } else { "jpeg" }
}

/// Runs S1–S6 and natural stepping. `scale` shortens all pauses (1.0 = real timing; tests use less).
pub fn run_all<B: Backend>(d: &mut Driver<B>, scale: f64) {
    let ms = |x: u64| ((x as f64) * scale) as u64;
    let settle = ms(2000);
    let mut pos = 0usize;
    let mut fresh = |n: usize| {
        let p = pos;
        pos += n;
        p
    };
    for m in [Mode::Raw, Mode::Jpeg] {
        let mn = mname(m);
        // S1: next image, then Z after 0 / 50 / 150 ms
        d.scenario("S1");
        d.reset(m);
        let mut p = fresh(40);
        d.go(p, "start");
        d.wait_ready(Duration::from_millis(ms(3000)));
        d.sleep(settle);
        for w in [0u64, 50, 150] {
            for _ in 0..3 {
                p += 1;
                d.go(p, &format!("{mn} fit change (z after {w})"));
                d.sleep(ms(w));
                d.zoom(true, &format!("{mn} zoom {w} ms after change"));
                d.wait_ready(Duration::from_millis(ms(3000)));
                d.sleep(ms(300));
                d.zoom(false, &format!("{mn} zoom out"));
                d.sleep(ms(1000));
            }
        }
        // S2: rapid stepping at 4, 7, 10 images/s, at fit and at 100 %
        d.scenario("S2");
        for zoomed in [false, true] {
            for rate in [4u64, 7, 10] {
                d.reset(m);
                let mut p = fresh(30);
                d.go(p, "start");
                if zoomed {
                    d.zoom(true, "start");
                }
                d.wait_ready(Duration::from_millis(ms(3000)));
                d.sleep(settle);
                let z = if zoomed { "100%" } else { "fit" };
                for _ in 0..20 {
                    p += 1;
                    d.go(p, &format!("{mn} {z} step {rate}/s"));
                    d.sleep(ms(1000 / rate));
                }
                d.wait_ready(Duration::from_millis(ms(3000)));
            }
        }
        // S4: cold first display (caches emptied), then Z
        d.scenario("S4");
        for _ in 0..3 {
            d.reset(m);
            let p = fresh(5);
            d.go(p, &format!("{mn} cold first view"));
            d.wait_ready(Duration::from_millis(ms(5000)));
            d.zoom(true, &format!("{mn} cold zoom after first view"));
            d.wait_ready(Duration::from_millis(ms(5000)));
            d.sleep(ms(500));
        }
        // S6: Z after a pause of 1, 3, 5 s
        d.scenario("S6");
        d.reset(m);
        let mut p = fresh(20);
        d.go(p, "start");
        d.wait_ready(Duration::from_millis(ms(3000)));
        d.sleep(settle);
        for pause in [1000u64, 3000, 5000] {
            for _ in 0..2 {
                p += 1;
                d.go(p, &format!("{mn} fit change (z after {pause})"));
                d.sleep(ms(pause));
                d.zoom(true, &format!("{mn} zoom after {} s pause", pause / 1000));
                d.wait_ready(Duration::from_millis(ms(3000)));
                d.sleep(ms(300));
                d.zoom(false, &format!("{mn} zoom out"));
                d.sleep(ms(300));
            }
        }
        // S5: back to an image after 1–5 steps
        d.scenario("S5");
        d.reset(m);
        let p0 = fresh(30);
        d.go(p0, "start");
        d.wait_ready(Duration::from_millis(ms(3000)));
        d.sleep(settle);
        for k in 1..=5usize {
            let base = d.cur();
            for i in 1..=k {
                d.go(base + i, &format!("{mn} fit step"));
                d.sleep(ms(500));
            }
            for i in (0..k).rev() {
                let label = if i == 0 { format!("{mn} fit back after {k} steps") } else { format!("{mn} fit step back") };
                d.go(base + i, &label);
                d.sleep(ms(500));
            }
            d.go(base + 1, &format!("{mn} fit step"));
            d.sleep(ms(800));
        }
    }
    // S3: RAW <-> JPEG on the shown image (after a pause) and right after an image change
    d.scenario("S3");
    for zoomed in [false, true] {
        let z = if zoomed { "100%" } else { "fit" };
        d.reset(Mode::Raw);
        let mut p = fresh(20);
        d.go(p, "start");
        if zoomed {
            d.zoom(true, "start");
        }
        d.wait_ready(Duration::from_millis(ms(3000)));
        d.sleep(settle);
        for _ in 0..3 {
            d.set_mode(Mode::Jpeg, &format!("{z} raw->jpeg after pause"));
            d.sleep(ms(1000));
            d.set_mode(Mode::Raw, &format!("{z} jpeg->raw after pause"));
            d.sleep(ms(1000));
        }
        for _ in 0..3 {
            p += 1;
            let m = d.mode();
            d.go(p, &format!("{z} {} change", mname(m)));
            let (to, label) = if m == Mode::Raw { (Mode::Jpeg, "raw->jpeg") } else { (Mode::Raw, "jpeg->raw") };
            d.set_mode(to, &format!("{z} {label} right after change"));
            d.sleep(ms(1500));
        }
    }
    // Natural stepping (RAW): 150–1500 ms between images, sometimes Z for 1–3 s, sometimes back
    d.scenario("NAT");
    d.reset(Mode::Raw);
    let mut p = fresh(100) + 5;
    d.go(p, "start");
    d.wait_ready(Duration::from_millis(ms(3000)));
    d.sleep(settle);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..50 {
        p = if rng.range(0, 9) == 0 { p.saturating_sub(1) } else { p + 1 };
        d.go(p, "raw change");
        let dwell = rng.range(150, 1500);
        if rng.range(0, 4) == 0 {
            let before = rng.range(150, dwell.max(151));
            d.sleep(ms(before));
            d.zoom(true, "raw zoom");
            d.sleep(ms(rng.range(1000, 3000)));
            d.zoom(false, "raw zoom out");
            d.sleep(ms(dwell - before.min(dwell)));
        } else {
            d.sleep(ms(dwell));
        }
    }
    // MIX (P5): back and forth within 12 images, jumps of 3, repeated zooms and mode switches.
    // Re-display hits depend on what the budget kept.
    d.scenario("MIX");
    d.reset(Mode::Raw);
    let lo = fresh(200) + 50;
    let hi = lo + 11;
    d.go(lo, "start");
    d.wait_ready(Duration::from_millis(ms(3000)));
    d.sleep(settle);
    let mut p = lo;
    let mut dir: isize = 1;
    for _ in 0..150 {
        let r = rng.range(0, 99);
        if r < 60 {
            if (dir > 0 && p >= hi) || (dir < 0 && p <= lo) {
                dir = -dir;
            }
            p = (p as isize + dir) as usize;
            let m = mname(d.mode());
            d.go(p, &format!("mix {m} change"));
        } else if r < 75 {
            let j: isize = if rng.range(0, 1) == 0 { -3 } else { 3 };
            p = (p as isize + j).clamp(lo as isize, hi as isize) as usize;
            let m = mname(d.mode());
            d.go(p, &format!("mix {m} jump"));
        } else if r < 90 {
            let m = mname(d.mode());
            d.zoom(true, &format!("mix {m} zoom"));
            d.sleep(ms(rng.range(500, 2000)));
            d.zoom(false, &format!("mix {m} zoom out"));
        } else {
            let to = if d.mode() == Mode::Raw { Mode::Jpeg } else { Mode::Raw };
            d.set_mode(to, &format!("mix to {}", mname(to)));
        }
        d.sleep(ms(rng.range(150, 1200)));
    }
}

// ---------- summary ----------

/// Aggregates the EV rows: per group, strategy, scenario and label.
pub fn summarize(events_csv: &str) -> String {
    let mut groups: Vec<(String, Vec<(bool, Option<f64>)>)> = Vec::new();
    let mut idx: HashMap<String, usize> = HashMap::new();
    for line in events_csv.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        // group, strategy, scenario, label, pos, view, t_req, latency, shown, hit, on_screen
        if f.len() < 11 || f[3] == "start" || f[3].ends_with("zoom out") {
            continue;
        }
        let key = format!("{},{},{},{}", f[0], f[1], f[2], f[3]);
        let shown = f[8] == "1";
        let lat = f[7].parse::<f64>().ok();
        let i = *idx.entry(key.clone()).or_insert_with(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        groups[i].1.push((shown, lat));
    }
    let mut out = String::from("group,strategy,scenario,label,n,shown,not_shown,le30ms_share,median_ms,p95_ms,max_ms\n");
    for (k, v) in groups {
        let n = v.len();
        let mut lats: Vec<f64> = v.iter().filter_map(|x| if x.0 { x.1 } else { None }).collect();
        lats.sort_by(|a, b| a.total_cmp(b));
        let shown = lats.len();
        let le30 = v.iter().filter(|x| x.0 && x.1.is_some_and(|l| l <= 30.0)).count();
        let q = |p: f64| if lats.is_empty() { f64::NAN } else { lats[(((lats.len() as f64) * p).ceil() as usize).clamp(1, lats.len()) - 1] };
        let _ = writeln!(
            out,
            "{k},{n},{shown},{},{:.2},{:.1},{:.1},{:.1}",
            n - shown,
            le30 as f64 / n as f64,
            q(0.5),
            q(0.95),
            lats.last().copied().unwrap_or(f64::NAN)
        );
    }
    out
}

// ---------- tests with a simulated decoder ----------

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Backend for Fake {
        type S = u32;
        fn open(&self, _: &Path) -> Result<(u32, u32, u32, i32), String> {
            std::thread::sleep(Duration::from_millis(30));
            Ok((0, 9568, 6376, 0))
        }
        fn half(&self, _: &mut u32, _: i32) -> Result<Vec<u8>, String> {
            std::thread::sleep(Duration::from_millis(25));
            Ok(vec![0; 1000])
        }
        fn region(&self, _: &mut u32, _: i32, r: [u32; 4]) -> Result<Vec<u8>, String> {
            assert!(r[2] > 0 && r[3] > 0);
            std::thread::sleep(Duration::from_millis(4));
            Ok(vec![0; 100])
        }
        fn jpeg(&self, _: &Path) -> Result<Vec<u8>, String> {
            std::thread::sleep(Duration::from_millis(2));
            Ok(vec![0; 10])
        }
    }

    fn cfg() -> Config {
        Config { view_w: 1920, view_h: 1080, budget_bytes: 1 << 30, tile: 1024 }
    }

    fn run(name: &str) -> (String, String) {
        let mut d = Driver::new(Fake, vec!["a".into(), "b".into()], 2000, strategy(name).unwrap(), cfg());
        run_all(&mut d, 0.05);
        d.finish("fake")
    }

    #[test]
    fn rect_is_centred_and_inside() {
        let c = cfg();
        assert_eq!(view_rect(&c, (9568, 6376, 0), View::Zoom), [3824, 2648, 1920, 1080]);
        let r = view_rect(&c, (9568, 6376, 6), View::Zoom);
        assert_eq!((r[2], r[3]), (1080, 1920));
        let r = view_rect(&c, (2000, 1500, 0), View::Ring);
        assert!(r[0] + r[2] <= 2000 && r[1] + r[3] <= 1500);
    }

    #[test]
    fn prefetch_beats_on_demand() {
        let (ev_none, sum_none) = run("none");
        let (ev_full, sum_full) = run("full");
        assert!(sum_none.starts_with("SUM,fake,none,"), "{sum_none}");
        assert_eq!(sum_full.split(',').nth(11), Some("0"), "errors: {sum_full}");
        let head = "group,strategy,scenario,label,pos,view,t_req_ms,latency_ms,shown,hit,on_screen_ms\n";
        let strip = |s: &str| s.lines().map(|l| l.trim_start_matches("EV,").to_string() + "\n").collect::<String>();
        let s = summarize(&format!("{head}{}{}", strip(&ev_none), strip(&ev_full)));
        let share = |strat: &str, label: &str| -> f64 {
            let l = s.lines().find(|l| l.starts_with(&format!("fake,{strat},")) && l.contains(&format!(",{label},"))).unwrap_or_else(|| panic!("{label} missing:\n{s}"));
            l.split(',').nth(7).unwrap().parse().unwrap()
        };
        // with neighbours prepared, natural stepping is mostly served at once
        assert!(share("full", "raw change") > share("none", "raw change"), "{s}");
        assert!(share("full", "raw zoom") >= 0.8, "{s}");
        assert!(share("full", "raw zoom") > share("none", "raw zoom"), "{s}");
        // cold display is never a hit
        assert_eq!(share("full", "raw cold first view"), 0.0);
    }

    #[test]
    fn p5_stays_within_total_budget() {
        // fake sessions are 9568 × 6376 × 2 ≈ 116 MB; views are tiny
        let mut d = Driver::new(Fake, vec!["a".into()], 2000, strategy("p5@400").unwrap(), Config { budget_bytes: 400 << 20, ..cfg() });
        run_all(&mut d, 0.05);
        let (ev, sum) = d.finish("fake");
        let f: Vec<&str> = sum.split(',').collect();
        let (errors, ev_sess, peak_mb): (usize, usize, f64) = (f[11].parse().unwrap(), f[13].parse().unwrap(), f[14].parse().unwrap());
        assert_eq!(errors, 0, "{sum}");
        assert!(ev_sess > 0, "sessions must be evicted under 400 MB: {sum}");
        // one current-image job may push past the budget before eviction catches up
        assert!(peak_mb <= 400.0 + 117.0, "{sum}");
        assert!(ev.contains(",MIX,mix raw change,"), "MIX scenario missing");
    }
}
