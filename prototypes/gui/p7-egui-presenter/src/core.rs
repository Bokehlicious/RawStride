//! The view core for P7 (ADR 0008 §2, §12): P5's scheduling rules with CPU PPG, and egui textures as
//! the cache of finished views.
//!
//! - Two workers, as P5: an interactive one for the current image only, and a background one for the
//!   plan. LibRaw sessions (unpacked RAWs) live here, under the core budget.
//! - A finished view (fit = half-size PPG of the whole image; zoom = the exact 100 % region; ring = twice
//!   that, for panning) is handed to the UI, which uploads it as a texture. The core keeps no RGB copy:
//!   the texture *is* the view cache, under its own byte budget (the egui analogue of B2-res).
//! - Plan (P5 `p5`): the current image's shown view; after a 150 ms dwell its other view, then +1 and
//!   +2 in the direction of travel (both views), the ring of the current image, and -1 behind.
//!   Prefetch starts only if it fits after evicting what the plan does not want. The current image is
//!   never evicted.
//!
//! No egui types here, so the rules are tested with a fake backend.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::view::{self, ImageSize, Rect};

pub const DWELL: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Fit,
    Zoom,
    Ring,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Fit => "Fit",
            Kind::Zoom => "Zoom",
            Kind::Ring => "Ring",
        }
    }
}

/// What decodes images. The real one is LibRaw (`crate::libraw`); tests use a fake.
pub trait Backend: Send + Sync + 'static {
    type S: Send + 'static;
    fn open(&self, path: &Path) -> Result<(Self::S, ImageSize), String>;
    /// Renders with PPG: half size of the whole image, or a full-size sensor region. Returns
    /// (RGB, width, height).
    fn render(&self, s: &mut Self::S, half: bool, region: Option<Rect>) -> Result<(Vec<u8>, usize, usize), String>;
}

/// A finished view, on its way to the UI.
pub struct ViewOut {
    pub generation: u64,
    pub pos: usize,
    pub kind: Kind,
    /// The sensor region the pixels cover (the whole image for a fit view).
    pub region: Rect,
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
    /// FNV-1a over the pixels (correctness checks: same file and region give the same hash).
    pub hash: u64,
}

/// A view resident as a texture in the UI.
#[derive(Clone, Debug)]
pub struct Resident {
    pub id: u64,
    pub generation: u64,
    pub pos: usize,
    pub kind: Kind,
    pub region: Rect,
    pub bytes: usize,
    pub used: u64,
    pub hash: u64,
}

/// One user request (P5's event log).
#[derive(Clone, Debug)]
pub struct Rec {
    pub scen: String,
    pub label: String,
    pub pos: usize,
    pub zoomed: bool,
    pub t: Instant,
    pub hit: bool,
    pub shown: Option<Instant>,
    pub left: Option<Instant>,
    pub miss: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub unpacks: usize,
    pub renders: usize,
    pub failures: usize,
    pub refused: usize,
    pub discarded: usize,
    pub evicted_textures: usize,
    pub evicted_sessions: usize,
    pub peak_tex_bytes: usize,
    pub peak_sess_bytes: usize,
    pub overshoot_max_bytes: i64,
    pub unpack_ms: Vec<f64>,
    pub render_ms: HashMap<&'static str, Vec<f64>>,
    pub last_error: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Job {
    Unpack(usize),
    Render(usize, Kind),
}

impl Job {
    fn pos(self) -> usize {
        match self {
            Job::Unpack(p) | Job::Render(p, _) => p,
        }
    }
}

struct Pending {
    generation: u64,
    pos: usize,
    kind: Kind,
    region: Rect,
    bytes: usize,
}

pub struct State<S> {
    pub generation: u64,
    pub cur: Option<usize>,
    pub dir: i8,
    pub since: Instant,
    pub zoomed: bool,
    pub center: [f32; 2],
    /// Size of the image area in physical pixels (set by the UI).
    pub view_px: [u32; 2],
    pub sizes: HashMap<usize, ImageSize>,
    sessions: HashMap<usize, Option<S>>,
    sess_lru: Vec<usize>,
    sess_sizes: HashMap<usize, usize>,
    pub sess_bytes: usize,
    pub core_budget: usize,
    last_sess_bytes: usize,
    pub resident: Vec<Resident>,
    pub tex_bytes: usize,
    pub tex_budget: usize,
    tick: u64,
    next_id: u64,
    pending: Vec<Pending>,
    running: HashSet<Job>,
    failed: HashSet<usize>,
    ever_resident: HashSet<(u64, usize)>,
    refused: HashSet<(u64, usize)>,
    pub recs: Vec<Rec>,
    pub open: Option<usize>,
    pub scen: String,
    pub stop: bool,
    /// Set by `reset`; the UI drops all textures and clears it.
    pub clear_textures: bool,
    pub stats: Stats,
}

pub struct Core<B: Backend> {
    backend: B,
    files: Vec<PathBuf>,
    m: Mutex<State<B::S>>,
    cv: Condvar,
    out: Mutex<Option<Sender<ViewOut>>>,
    wake: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

/// Locks a mutex; a poisoned lock (a panicked thread) is taken over rather than propagated.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn fnv(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl<S> State<S> {
    pub fn new(core_budget: usize, tex_budget: usize) -> Self {
        State {
            generation: 0,
            cur: None,
            dir: 1,
            since: Instant::now(),
            zoomed: false,
            center: [0.5, 0.5],
            view_px: [1920, 1080],
            sizes: HashMap::new(),
            sessions: HashMap::new(),
            sess_lru: Vec::new(),
            sess_sizes: HashMap::new(),
            sess_bytes: 0,
            core_budget,
            last_sess_bytes: 128 << 20,
            resident: Vec::new(),
            tex_bytes: 0,
            tex_budget,
            tick: 0,
            next_id: 1,
            pending: Vec::new(),
            running: HashSet::new(),
            failed: HashSet::new(),
            ever_resident: HashSet::new(),
            refused: HashSet::new(),
            recs: Vec::new(),
            open: None,
            scen: String::new(),
            stop: false,
            clear_textures: false,
            stats: Stats::default(),
        }
    }

    /// Views wanted now, most important first (P5 `p5`). `ignore_dwell` gives the full plan (used to
    /// decide what must not be evicted).
    pub fn wanted(&self, ignore_dwell: bool) -> Vec<(usize, Kind)> {
        let Some(cur) = self.cur else { return Vec::new() };
        let (p, other) = if self.zoomed { (Kind::Zoom, Kind::Fit) } else { (Kind::Fit, Kind::Zoom) };
        let mut v = vec![(cur, p)];
        let mut add = |x: (usize, Kind)| {
            if !v.contains(&x) {
                v.push(x);
            }
        };
        if ignore_dwell || self.since.elapsed() >= DWELL {
            add((cur, other));
            for k in 1..=2usize {
                let j = if self.dir >= 0 { cur.checked_add(k) } else { cur.checked_sub(k) };
                if let Some(j) = j {
                    add((j, p));
                    add((j, other));
                }
            }
            add((cur, Kind::Ring));
            let behind = if self.dir >= 0 { cur.checked_sub(1) } else { cur.checked_add(1) };
            if let Some(b) = behind {
                add((b, p));
            }
        }
        v
    }

    /// The sensor region a view of `pos` must cover now (fit: the whole image).
    fn needed_region(&self, pos: usize, kind: Kind) -> Option<Rect> {
        let size = *self.sizes.get(&pos)?;
        Some(match kind {
            Kind::Fit => [0, 0, size.w, size.h],
            Kind::Zoom => view::zoom_regions(size, self.view_px, self.center).0,
            Kind::Ring => view::zoom_regions(size, self.view_px, self.center).1,
        })
    }

    /// A resident texture that serves `kind` of `pos` now. A zoom need is also served by the ring.
    pub fn serving(&self, pos: usize, kind: Kind) -> Option<&Resident> {
        let need = self.needed_region(pos, kind)?;
        let fits = |r: &Resident| {
            r.generation == self.generation
                && r.pos == pos
                && match kind {
                    Kind::Fit => r.kind == Kind::Fit,
                    Kind::Zoom => matches!(r.kind, Kind::Zoom | Kind::Ring) && view::contains(r.region, need),
                    Kind::Ring => r.kind == Kind::Ring && view::contains(r.region, need),
                }
        };
        // prefer the exact zoom view over the ring
        self.resident.iter().filter(|r| fits(r)).min_by_key(|r| if r.kind == Kind::Zoom { 0 } else { 1 })
    }

    fn satisfied(&self, pos: usize, kind: Kind) -> bool {
        if self.serving(pos, kind).is_some() {
            return true;
        }
        let Some(need) = self.needed_region(pos, kind) else { return false };
        self.pending.iter().any(|p| {
            p.generation == self.generation
                && p.pos == pos
                && match kind {
                    Kind::Fit => p.kind == Kind::Fit,
                    Kind::Zoom => matches!(p.kind, Kind::Zoom | Kind::Ring) && view::contains(p.region, need),
                    Kind::Ring => p.kind == Kind::Ring && view::contains(p.region, need),
                }
        })
    }

    fn plan(&self) -> Vec<Job> {
        let mut v = Vec::new();
        for (pos, kind) in self.wanted(false) {
            if self.failed.contains(&pos) || self.satisfied(pos, kind) {
                continue;
            }
            if self.sessions.contains_key(&pos) {
                v.push(Job::Render(pos, kind));
            } else if !v.contains(&Job::Unpack(pos)) {
                v.push(Job::Unpack(pos));
            }
        }
        v
    }

    fn protected_positions(&self) -> HashSet<usize> {
        self.wanted(true).into_iter().map(|w| w.0).chain(self.cur).collect()
    }

    fn est_view_bytes(&self, pos: usize, kind: Kind) -> usize {
        match (self.sizes.get(&pos), kind) {
            (Some(s), Kind::Fit) => (s.w as usize / 2) * (s.h as usize / 2) * 4,
            (_, Kind::Fit) => 64 << 20,
            (_, Kind::Zoom) => self.view_px[0] as usize * self.view_px[1] as usize * 4,
            (_, Kind::Ring) => self.view_px[0] as usize * self.view_px[1] as usize * 16,
        }
    }

    /// Whether `need` texture bytes fit after evicting everything the plan does not protect.
    fn tex_fits(&self, need: usize) -> bool {
        let protect = self.protected_positions();
        let pending: usize = self.pending.iter().map(|p| p.bytes).sum();
        let evictable: usize = self.resident.iter().filter(|r| !protect.contains(&r.pos)).map(|r| r.bytes).sum();
        self.tex_bytes + pending + need <= self.tex_budget + evictable
    }

    /// Evicts idle sessions the plan does not protect until `need` more bytes fit.
    fn sessions_fit(&mut self, need: usize) -> bool {
        let protect = self.protected_positions();
        while self.sess_bytes + need > self.core_budget {
            let victim = self
                .sess_lru
                .iter()
                .copied()
                .find(|k| !protect.contains(k) && matches!(self.sessions.get(k), Some(Some(_))));
            let Some(k) = victim else { return false };
            self.drop_session(k);
            self.stats.evicted_sessions += 1;
        }
        true
    }

    fn drop_session(&mut self, k: usize) {
        self.sessions.remove(&k);
        self.sess_lru.retain(|x| *x != k);
        if let Some(b) = self.sess_sizes.remove(&k) {
            self.sess_bytes = self.sess_bytes.saturating_sub(b);
        }
    }

    /// Picks the next job (P5 `pick`). Returns the job, the session for a render, and its region.
    fn pick(&mut self, interactive: bool) -> Option<(Job, Option<S>, Rect)> {
        for job in self.plan() {
            if self.running.contains(&job) || (interactive && Some(job.pos()) != self.cur) {
                continue;
            }
            let is_cur = Some(job.pos()) == self.cur;
            match job {
                Job::Unpack(p) => {
                    let need = self.sizes.get(&p).map(|s| s.w as usize * s.h as usize * 2).unwrap_or(self.last_sess_bytes);
                    if !self.sessions_fit(need) && !is_cur {
                        self.stats.refused += 1;
                        self.refused.insert((self.generation, p));
                        continue;
                    }
                    return Some((job, None, [0; 4]));
                }
                Job::Render(p, kind) => {
                    if !is_cur && !self.tex_fits(self.est_view_bytes(p, kind)) {
                        self.stats.refused += 1;
                        self.refused.insert((self.generation, p));
                        continue;
                    }
                    let Some(region) = self.needed_region(p, kind) else { continue };
                    let Some(slot) = self.sessions.get_mut(&p) else { continue };
                    let Some(s) = slot.take() else { continue };
                    return Some((job, Some(s), region));
                }
            }
        }
        None
    }

    /// Records a request for what is shown now (P5 `request`).
    pub fn request(&mut self, label: &str) {
        let now = Instant::now();
        if let Some(i) = self.open.take()
            && let Some(r) = self.recs.get_mut(i)
        {
            r.left = Some(now);
        }
        let Some(pos) = self.cur else { return };
        let kind = if self.zoomed { Kind::Zoom } else { Kind::Fit };
        let hit = self.serving(pos, kind).is_some();
        let miss = if hit {
            ""
        } else if self.running.iter().any(|j| j.pos() == pos) {
            "inflight"
        } else if self.ever_resident.contains(&(self.generation, pos)) {
            "evicted"
        } else if self.refused.contains(&(self.generation, pos)) {
            "refused"
        } else {
            "not-started"
        };
        self.recs.push(Rec {
            scen: self.scen.clone(),
            label: label.to_string(),
            pos,
            zoomed: self.zoomed,
            t: now,
            hit,
            shown: None,
            left: None,
            miss,
        });
        self.open = Some(self.recs.len() - 1);
    }

    /// The UI uploaded (or will upload) a view. Returns its texture id and the ids to drop, or `None`
    /// if it is discarded (old folder, or no room for a non-current image).
    pub fn accept(&mut self, out: &ViewOut, bytes: usize) -> (Option<u64>, Vec<u64>) {
        if let Some(i) = self
            .pending
            .iter()
            .position(|p| p.generation == out.generation && p.pos == out.pos && p.kind == out.kind && p.region == out.region)
        {
            self.pending.swap_remove(i);
        }
        if out.generation != self.generation {
            self.stats.discarded += 1;
            return (None, Vec::new());
        }
        let protect = self.protected_positions();
        let mut dropped = Vec::new();
        while self.tex_bytes + bytes > self.tex_budget {
            let victim = self
                .resident
                .iter()
                .enumerate()
                .filter(|(_, r)| !protect.contains(&r.pos))
                .min_by_key(|(_, r)| r.used)
                .map(|(i, _)| i);
            let Some(i) = victim else { break };
            let r = self.resident.swap_remove(i);
            self.tex_bytes = self.tex_bytes.saturating_sub(r.bytes);
            self.stats.evicted_textures += 1;
            dropped.push(r.id);
        }
        if self.tex_bytes + bytes > self.tex_budget && Some(out.pos) != self.cur {
            self.stats.discarded += 1;
            return (None, dropped);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tick += 1;
        self.resident.push(Resident {
            id,
            generation: out.generation,
            pos: out.pos,
            kind: out.kind,
            region: out.region,
            bytes,
            used: self.tick,
            hash: out.hash,
        });
        self.tex_bytes += bytes;
        self.ever_resident.insert((self.generation, out.pos));
        self.stats.peak_tex_bytes = self.stats.peak_tex_bytes.max(self.tex_bytes);
        self.stats.overshoot_max_bytes = self.stats.overshoot_max_bytes.max(self.tex_bytes as i64 - self.tex_budget as i64);
        (Some(id), dropped)
    }

    /// A view the UI cannot upload (larger than the GPU's texture limit): dropped and counted.
    pub fn reject(&mut self, out: &ViewOut) {
        if let Some(i) = self
            .pending
            .iter()
            .position(|p| p.generation == out.generation && p.pos == out.pos && p.kind == out.kind && p.region == out.region)
        {
            self.pending.swap_remove(i);
        }
        self.stats.discarded += 1;
        self.failed.insert(out.pos);
    }

    /// Marks a resident texture as used now (LRU).
    pub fn touch(&mut self, id: u64) {
        self.tick += 1;
        let t = self.tick;
        if let Some(r) = self.resident.iter_mut().find(|r| r.id == id) {
            r.used = t;
        }
    }

    /// The UI has drawn the current image's requested view: the open request is served.
    pub fn mark_shown(&mut self) {
        if let Some(i) = self.open
            && let Some(r) = self.recs.get_mut(i)
            && r.shown.is_none()
            && Some(r.pos) == self.cur
            && r.zoomed == self.zoomed
        {
            r.shown = Some(Instant::now());
        }
    }

    /// Clears what belongs to the previous folder (P5 `reset`); textures are dropped by the UI.
    fn clear_folder(&mut self) {
        self.generation += 1;
        self.cur = None;
        self.zoomed = false;
        self.sessions.retain(|_, s| s.is_none()); // sessions in use come back and are dropped then
        self.sess_lru.clear();
        self.sess_sizes.clear();
        self.sess_bytes = 0;
        self.sizes.clear();
        self.failed.clear();
        self.pending.clear();
        self.clear_textures = true;
    }

    /// The UI dropped all textures.
    pub fn textures_cleared(&mut self) -> Vec<u64> {
        let ids = self.resident.iter().map(|r| r.id).collect();
        self.resident.clear();
        self.tex_bytes = 0;
        self.clear_textures = false;
        ids
    }

    fn finish_unpack(&mut self, generation: u64, pos: usize, res: Result<(S, ImageSize), String>, ms: f64) {
        self.stats.unpacks += 1;
        self.stats.unpack_ms.push(ms);
        match res {
            Ok((s, size)) => {
                if generation != self.generation {
                    return;
                }
                let bytes = size.w as usize * size.h as usize * 2;
                self.sizes.insert(pos, size);
                self.sessions.insert(pos, Some(s));
                self.sess_sizes.insert(pos, bytes);
                self.sess_bytes += bytes;
                self.last_sess_bytes = bytes;
                self.sess_lru.retain(|x| *x != pos);
                self.sess_lru.push(pos);
                self.stats.peak_sess_bytes = self.stats.peak_sess_bytes.max(self.sess_bytes);
            }
            Err(e) => {
                self.stats.failures += 1;
                self.stats.last_error = e;
                self.failed.insert(pos);
            }
        }
    }
}

impl<B: Backend> Core<B> {
    pub fn new(backend: B, files: Vec<PathBuf>, core_budget: usize, tex_budget: usize) -> Arc<Self> {
        Arc::new(Core {
            backend,
            files,
            m: Mutex::new(State::new(core_budget, tex_budget)),
            cv: Condvar::new(),
            out: Mutex::new(None),
            wake: Mutex::new(None),
        })
    }

    pub fn state(&self) -> MutexGuard<'_, State<B::S>> {
        lock(&self.m)
    }

    pub fn set_output(&self, tx: Sender<ViewOut>, wake: Box<dyn Fn() + Send + Sync>) {
        *lock(&self.out) = Some(tx);
        *lock(&self.wake) = Some(wake);
    }

    fn wake_ui(&self) {
        if let Some(w) = lock(&self.wake).as_ref() {
            w();
        }
    }

    /// Starts the two workers (interactive + background, as P5).
    pub fn start(self: &Arc<Self>) -> Result<Vec<std::thread::JoinHandle<()>>, String> {
        let mut v = Vec::new();
        for interactive in [true, false] {
            let me = self.clone();
            let name = if interactive { "p7-interactive" } else { "p7-background" };
            let h = std::thread::Builder::new()
                .name(name.into())
                .spawn(move || me.worker(interactive))
                .map_err(|e| format!("spawn {name}: {e}"))?;
            v.push(h);
        }
        Ok(v)
    }

    fn worker(&self, interactive: bool) {
        let mut g = self.state();
        loop {
            if g.stop {
                return;
            }
            let Some((job, sess, region)) = g.pick(interactive) else {
                g = self.cv.wait_timeout(g, Duration::from_millis(5)).map(|r| r.0).unwrap_or_else(|e| e.into_inner().0);
                continue;
            };
            let generation = g.generation;
            g.running.insert(job);
            let path = self.files.get(job.pos() % self.files.len().max(1)).cloned();
            drop(g);
            let t0 = Instant::now();
            match job {
                Job::Unpack(pos) => {
                    let res = match &path {
                        Some(p) => self.backend.open(p),
                        None => Err("no files".into()),
                    };
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    g = self.state();
                    g.running.remove(&job);
                    g.finish_unpack(generation, pos, res, ms);
                }
                Job::Render(pos, kind) => {
                    let mut sess = sess;
                    let res = match sess.as_mut() {
                        Some(s) => self.backend.render(s, kind == Kind::Fit, (kind != Kind::Fit).then_some(region)),
                        None => Err("no session".into()),
                    };
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    g = self.state();
                    g.running.remove(&job);
                    g.stats.renders += 1;
                    g.stats.render_ms.entry(kind.name()).or_default().push(ms);
                    // the session goes back (or is dropped with an old folder)
                    if generation == g.generation {
                        if let (Some(slot), Some(s)) = (g.sessions.get_mut(&pos), sess) {
                            *slot = Some(s);
                        }
                    } else {
                        g.sessions.remove(&pos);
                    }
                    match res {
                        Ok((rgb, w, h)) if generation == g.generation => {
                            let bytes = w * h * 4;
                            g.pending.push(Pending { generation, pos, kind, region, bytes });
                            let out = ViewOut { generation, pos, kind, region, w, h, hash: fnv(&rgb), rgb };
                            if let Some(tx) = lock(&self.out).as_ref()
                                && tx.send(out).is_err()
                            {
                                g.stop = true;
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            g.stats.failures += 1;
                            g.stats.last_error = e;
                            g.failed.insert(pos);
                        }
                    }
                }
            }
            self.cv.notify_all();
            drop(g);
            self.wake_ui();
            g = self.state();
        }
    }

    // ---------- the user (driver, keys) ----------

    pub fn go(&self, pos: usize, label: &str) {
        {
            let mut g = self.state();
            g.dir = match g.cur {
                Some(c) if pos < c => -1,
                _ => 1,
            };
            g.cur = Some(pos);
            g.since = Instant::now();
            g.request(label);
        }
        self.cv.notify_all();
        self.wake_ui();
    }

    pub fn zoom(&self, z: bool, label: &str) {
        {
            let mut g = self.state();
            g.zoomed = z;
            g.request(label);
        }
        self.cv.notify_all();
        self.wake_ui();
    }

    pub fn pan(&self, dx: f32, dy: f32, label: &str) {
        {
            let mut g = self.state();
            if g.zoomed && dx.is_finite() && dy.is_finite() {
                g.center = [(g.center[0] + dx).clamp(0.0, 1.0), (g.center[1] + dy).clamp(0.0, 1.0)];
            }
            g.request(label);
        }
        self.cv.notify_all();
        self.wake_ui();
    }

    /// A new folder: every cache is emptied (P5 `reset`). Waits until running jobs and the UI's
    /// textures are gone.
    pub fn reset(&self, scen: &str) {
        {
            let mut g = self.state();
            let now = Instant::now();
            if let Some(i) = g.open.take()
                && let Some(r) = g.recs.get_mut(i)
            {
                r.left = Some(now);
            }
            g.clear_folder();
            g.scen = scen.to_string();
        }
        self.wake_ui();
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            {
                let g = self.state();
                if g.running.is_empty() && !g.clear_textures {
                    return;
                }
            }
            self.wake_ui();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Waits until the open request has been shown, at most `max`.
    pub fn wait_ready(&self, max: Duration) {
        let end = Instant::now() + max;
        while Instant::now() < end {
            {
                let g = self.state();
                let done = g.open.and_then(|i| g.recs.get(i)).is_none_or(|r| r.shown.is_some());
                if done {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: ImageSize = ImageSize { w: 4000, h: 3000, flip: 0 };

    fn state(tex_mb: usize) -> State<u32> {
        let mut s = State::new(4096 << 20, tex_mb << 20);
        s.view_px = [1000, 800];
        s.cur = Some(10);
        s.since = Instant::now() - Duration::from_secs(1);
        s
    }

    fn out(s: &State<u32>, pos: usize, kind: Kind) -> ViewOut {
        let region = s.needed_region(pos, kind).unwrap_or([0; 4]);
        ViewOut { generation: s.generation, pos, kind, region, w: 1, h: 1, rgb: Vec::new(), hash: 0 }
    }

    #[test]
    fn wanted_follows_p5_order() {
        let mut s = state(1024);
        assert_eq!(
            s.wanted(false),
            vec![(10, Kind::Fit), (10, Kind::Zoom), (11, Kind::Fit), (11, Kind::Zoom), (12, Kind::Fit), (12, Kind::Zoom), (10, Kind::Ring), (9, Kind::Fit)]
        );
        s.since = Instant::now();
        assert_eq!(s.wanted(false), vec![(10, Kind::Fit)]);
        s.dir = -1;
        s.zoomed = true;
        assert_eq!(s.wanted(true)[..3], [(10, Kind::Zoom), (10, Kind::Fit), (9, Kind::Zoom)]);
    }

    #[test]
    fn plan_unpacks_first_then_renders() {
        let mut s = state(1024);
        assert_eq!(s.plan()[0], Job::Unpack(10));
        s.sessions.insert(10, Some(0));
        s.sizes.insert(10, SIZE);
        assert_eq!(s.plan()[0], Job::Render(10, Kind::Fit));
        // the interactive worker only takes the current image
        let picked = s.pick(true).map(|p| p.0);
        assert_eq!(picked, Some(Job::Render(10, Kind::Fit)));
    }

    #[test]
    fn a_ring_serves_a_zoom_inside_it() {
        let mut s = state(1024);
        s.sizes.insert(10, SIZE);
        let ring = out(&s, 10, Kind::Ring);
        let (id, _) = s.accept(&ring, 100);
        assert!(id.is_some());
        assert_eq!(s.serving(10, Kind::Zoom).map(|r| r.kind), Some(Kind::Ring));
        // panned far away: the ring no longer serves
        s.center = [0.95, 0.95];
        assert!(s.serving(10, Kind::Zoom).is_none());
    }

    #[test]
    fn the_current_image_is_never_evicted_and_unwanted_ones_go_first() {
        let mut s = state(1);
        for p in [3, 10] {
            s.sizes.insert(p, SIZE);
        }
        let (a, _) = s.accept(&out(&s, 3, Kind::Fit), 600 << 10);
        let (b, dropped) = s.accept(&out(&s, 10, Kind::Fit), 600 << 10);
        assert!(a.is_some() && b.is_some());
        assert_eq!(dropped, a.into_iter().collect::<Vec<_>>());
        // over budget with only the current image: it still stays
        let (c, dropped) = s.accept(&out(&s, 10, Kind::Zoom), 900 << 10);
        assert!(c.is_some());
        assert!(dropped.is_empty());
        assert!(s.stats.overshoot_max_bytes > 0);
    }

    #[test]
    fn prefetch_that_does_not_fit_is_refused() {
        let mut s = state(1);
        s.sizes.insert(11, SIZE);
        s.sessions.insert(11, Some(0));
        s.sessions.insert(10, None); // in use
        s.sizes.insert(10, SIZE);
        // fit of 11 needs 2000*1500*4 bytes, far over 1 MB
        let picked = s.pick(false).map(|p| p.0);
        assert!(picked != Some(Job::Render(11, Kind::Fit)));
        assert!(s.stats.refused > 0);
    }

    #[test]
    fn requests_classify_misses() {
        let mut s = state(1024);
        s.request("a");
        assert_eq!(s.recs[0].miss, "not-started");
        s.sizes.insert(10, SIZE);
        let _ = s.accept(&out(&s, 10, Kind::Fit), 100);
        s.request("b");
        assert!(s.recs[1].hit);
        assert!(s.recs[0].left.is_some());
        s.mark_shown();
        assert!(s.recs[1].shown.is_some());
    }

    #[test]
    fn old_folder_views_are_discarded() {
        let mut s = state(1024);
        s.sizes.insert(10, SIZE);
        let o = out(&s, 10, Kind::Fit);
        s.clear_folder();
        let (id, _) = s.accept(&o, 100);
        assert!(id.is_none());
        assert_eq!(s.stats.discarded, 1);
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_ne!(fnv(b"a"), fnv(b"b"));
    }
}
