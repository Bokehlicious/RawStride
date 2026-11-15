//! The view core with its two workers (ADR 0008 §12): an interactive worker for the current image
//! only, and a background worker for the rest of the plan. Every user action returns at once (G2);
//! the decoding runs on the workers.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::{Backend, DecodeError, Source};
use crate::scale;
use crate::state::{Config, Frame, Record, State, Stats, Task, ViewKind, ViewOut, ZOOM_STEP, Zoom};

/// The longest a worker sleeps without a reason (a safety net; changes wake it at once).
const IDLE_WAIT: Duration = Duration::from_secs(1);

type Wake = Box<dyn Fn() + Send + Sync>;

struct Shared<B: Backend> {
    backend: B,
    state: Mutex<State<B::Session>>,
    cv: Condvar,
    wake: Mutex<Option<Wake>>,
}

/// Locks a mutex. A poisoned lock (a thread panicked while holding it) is taken over: the state
/// is plain data and stays usable, and the viewer must not crash (AGENTS.md).
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl<B: Backend> Shared<B> {
    fn state(&self) -> MutexGuard<'_, State<B::Session>> {
        lock(&self.state)
    }

    fn wake_ui(&self) {
        if let Some(w) = lock(&self.wake).as_ref() {
            w();
        }
    }

    fn worker(&self, interactive: bool) {
        let mut g = self.state();
        loop {
            if g.stop {
                return;
            }
            let Some(task) = g.pick(interactive) else {
                let wait = g.next_deadline().map_or(IDLE_WAIT, |d| d.min(IDLE_WAIT));
                g = match self.cv.wait_timeout(g, wait) {
                    Ok((g, _)) => g,
                    Err(e) => e.into_inner().0,
                };
                continue;
            };
            drop(g);
            let t0 = Instant::now();
            match task {
                Task::Open {
                    generation,
                    key,
                    path,
                } => {
                    let result = self.backend.open(&path, key.1);
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let mut s = self.state();
                    s.finish_open_checked(
                        generation,
                        key,
                        result,
                        |size, source| self.backend.session_bytes(size, source),
                        |session| self.backend.profile_unusable(session),
                        ms,
                    );
                }
                Task::Render {
                    generation,
                    key,
                    view,
                    mut session,
                } => {
                    let result = match view.kind {
                        ViewKind::Half => self.backend.render_half(&mut session),
                        ViewKind::Zoom | ViewKind::Ring => {
                            self.backend.render_region(&mut session, view.region)
                        }
                        ViewKind::Scaled => self
                            .backend
                            .render_region(&mut session, view.region)
                            .and_then(|img| {
                                let (w, h) =
                                    scale::scaled_size(img.width, img.height, view.density());
                                scale::downscale(&img, w, h)
                                    .ok_or_else(|| DecodeError::new("scaling the view failed"))
                            }),
                    };
                    let result = result.and_then(|img| {
                        if img.is_consistent() {
                            Ok(img)
                        } else {
                            Err(DecodeError::new("the decoder returned a malformed image"))
                        }
                    });
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let mut s = self.state();
                    s.finish_render(generation, key, view, session, result, ms);
                }
                Task::Statistics {
                    generation,
                    key,
                    mut session,
                } => {
                    let result = self.backend.statistics(&mut session);
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let mut s = self.state();
                    s.finish_statistics(generation, key, session, result, ms);
                }
                Task::Clipping {
                    generation,
                    key,
                    mut session,
                } => {
                    let result = self.backend.clipping_map(&mut session);
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let mut s = self.state();
                    s.finish_clipping(generation, key, session, result, ms);
                }
            }
            self.cv.notify_all();
            self.wake_ui();
            g = self.state();
        }
    }
}

/// The view core (`pw-viewcore`): the view model, the plan, both budgets and the workers.
///
/// The presentation calls, once per frame:
///
/// 1. [`take_views`](Self::take_views), then [`accept`](Self::accept) (or
///    [`reject`](Self::reject)) for each, uploading the accepted ones as textures;
/// 2. [`take_dropped`](Self::take_dropped), releasing those textures;
/// 3. [`frame`](Self::frame), and draws what it says.
///
/// Dropping the core stops the workers. A LibRaw call that is running finishes first.
pub struct ViewCore<B: Backend> {
    shared: Arc<Shared<B>>,
    workers: Vec<JoinHandle<()>>,
}

impl<B: Backend> fmt::Debug for ViewCore<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.shared.state();
        f.debug_struct("ViewCore")
            .field("files", &s.files.len())
            .field("current", &s.cur)
            .field("zoom", &s.zoom)
            .field("session_bytes", &s.session_bytes)
            .field("texture_bytes", &s.texture_bytes)
            .finish()
    }
}

impl<B: Backend> ViewCore<B> {
    /// Starts the core with its two workers.
    pub fn start(backend: B, config: Config) -> Result<ViewCore<B>, String> {
        let shared = Arc::new(Shared {
            backend,
            state: Mutex::new(State::new(config)),
            cv: Condvar::new(),
            wake: Mutex::new(None),
        });
        let mut core = ViewCore {
            shared,
            workers: Vec::new(),
        };
        for (interactive, name) in [
            (true, "viewcore-interactive"),
            (false, "viewcore-background"),
        ] {
            let shared = core.shared.clone();
            let handle = std::thread::Builder::new()
                .name(name.into())
                .spawn(move || shared.worker(interactive))
                .map_err(|e| format!("cannot start {name}: {e}"))?;
            core.workers.push(handle);
        }
        Ok(core)
    }

    /// Called (from a worker) whenever a view is ready or the state changed, so the presentation
    /// draws a new frame (eframe: `request_repaint`).
    pub fn set_wake(&self, wake: impl Fn() + Send + Sync + 'static) {
        *lock(&self.shared.wake) = Some(Box::new(wake));
    }

    fn act(&self, label: &str, f: impl FnOnce(&mut State<B::Session>)) {
        {
            let mut s = self.shared.state();
            f(&mut s);
            s.changed();
            s.record(label);
        }
        self.shared.cv.notify_all();
        self.shared.wake_ui();
    }

    // ---------- user actions (return at once) ----------

    /// Opens a folder: `files` in display order. The first image becomes current.
    pub fn open_folder(&self, files: Vec<PathBuf>) {
        self.act("open", |s| s.open_folder(files));
    }

    /// The list of files changed (a filter, or an image that dropped out of it): `files` in display
    /// order, and `current` becomes current (clamped). Views and sessions of paths still in the
    /// list are kept, with the same texture ids; the rest are released through
    /// [`take_dropped`](Self::take_dropped) (ADR 0014 §4.4).
    pub fn set_files(&self, files: Vec<PathBuf>, current: usize) {
        self.act("files", |s| s.set_files(files, current));
    }

    /// Makes image `pos` current (clamped to the folder).
    pub fn go(&self, pos: usize) {
        self.act("go", |s| s.go(pos));
    }

    /// Steps by `delta` images (clamped to the folder).
    pub fn step(&self, delta: isize) {
        self.act("step", |s| {
            if let Some(cur) = s.cur {
                s.go(cur.saturating_add_signed(delta));
            }
        });
    }

    /// The last image.
    pub fn last(&self) {
        self.act("last", |s| s.go(usize::MAX));
    }

    /// Z: fit ↔ 100 %, around `anchor` (view pixels; `None` = the view centre).
    pub fn toggle_zoom(&self, anchor: Option<[f64; 2]>) {
        self.act("zoom", |s| s.toggle_zoom(anchor));
    }

    /// F: fit.
    pub fn fit(&self) {
        self.act("fit", |s| s.set_zoom(Zoom::Fit, None, false));
    }

    /// + and −: `steps` steps of ×1.25 (negative zooms out), around `anchor`.
    pub fn zoom_steps(&self, steps: i32, anchor: Option<[f64; 2]>) {
        self.act("zoom-step", |s| s.zoom_by(ZOOM_STEP.powi(steps), anchor));
    }

    /// The mouse wheel: multiplies the scale by `factor`, around `anchor`.
    pub fn zoom_by(&self, factor: f64, anchor: Option<[f64; 2]>) {
        self.act("zoom-wheel", |s| s.zoom_by(factor, anchor));
    }

    /// R: switches the display mode RAW ↔ JPEG (G4). Only the user changes it.
    pub fn toggle_mode(&self) {
        self.act("mode", |s| {
            let m = s.mode.other();
            s.set_mode(m);
        });
    }

    /// Sets the display mode (G4).
    pub fn set_mode(&self, mode: Source) {
        self.act("mode", |s| s.set_mode(mode));
    }

    /// Drags the image by `delta` view pixels.
    pub fn pan(&self, delta: [f64; 2]) {
        self.act("pan", |s| s.pan(delta));
    }

    /// Whether an overlay wants the current image's clipping map (ADR 0017 U7b-Q8): the shell
    /// sets it while the highlight or the shadow overlay is on. While it is set, the map of the
    /// current image is made after its statistics and given in [`Frame::clipping`]; while it is
    /// not, no map is asked for. Not a user request: nothing is recorded.
    pub fn set_clipping_wanted(&self, wanted: bool) {
        {
            let mut s = self.shared.state();
            if s.clipping_wanted == wanted {
                return;
            }
            s.clipping_wanted = wanted;
        }
        self.shared.cv.notify_all();
        self.shared.wake_ui();
    }

    // ---------- the presentation ----------

    /// Finished views to upload.
    pub fn take_views(&self) -> Vec<ViewOut> {
        std::mem::take(&mut self.shared.state().outbox)
    }

    /// Registers an uploaded view of `bytes` texture memory. Returns its texture id, or `None` if
    /// the view is not wanted any more (do not upload it).
    pub fn accept(&self, view: &ViewOut, bytes: u64) -> Option<u64> {
        let id = self.shared.state().accept(view, bytes);
        self.shared.cv.notify_all();
        id
    }

    /// A view that cannot be uploaded (for example larger than the GPU allows).
    pub fn reject(&self, view: &ViewOut, reason: &str) {
        self.shared.state().reject(view, reason);
        self.shared.cv.notify_all();
    }

    /// Texture ids to release.
    pub fn take_dropped(&self) -> Vec<u64> {
        let d = self.shared.state().take_dropped();
        if !d.is_empty() {
            self.shared.cv.notify_all();
        }
        d
    }

    /// What to draw now, for an image area of `view_px` physical pixels.
    pub fn frame(&self, view_px: [u32; 2]) -> Frame {
        let (frame, changed) = {
            let mut s = self.shared.state();
            let before = s.view_px;
            (s.frame(view_px), before != s.view_px)
        };
        if changed {
            self.shared.cv.notify_all();
        }
        frame
    }

    // ---------- measurement ----------

    pub fn stats(&self) -> Stats {
        self.shared.state().stats.clone()
    }

    /// The memory held now: (session bytes, texture bytes).
    pub fn memory(&self) -> (u64, u64) {
        let s = self.shared.state();
        (s.session_bytes, s.texture_bytes)
    }

    /// The request records so far (with `Config::record`).
    pub fn take_records(&self) -> Vec<Record> {
        std::mem::take(&mut self.shared.state().records)
    }

    // ---------- writing metadata (ADR 0007 §9.1 point 2) ----------

    /// A handle that closes the sessions of a file before its metadata is written. It can be
    /// cloned and sent to another thread.
    pub fn releaser(&self) -> Releaser<B> {
        Releaser {
            shared: self.shared.clone(),
        }
    }

    /// Stops the workers and waits for them. Called by `Drop`.
    pub fn shutdown(&mut self) {
        self.shared.state().stop = true;
        self.shared.cv.notify_all();
        for h in self.workers.drain(..) {
            // a worker that panicked has nothing left to clean up
            let _ = h.join();
        }
    }
}

impl<B: Backend> Drop for ViewCore<B> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Closes the sessions of a file in the view core before its metadata is written (ADR 0007 §9.1
/// point 2): a session keeps its file open for its whole life, and a write needs the file closed.
/// From [`ViewCore::releaser`]; it holds the core's shared state, so it can be cloned and used
/// from any thread.
pub struct Releaser<B: Backend> {
    shared: Arc<Shared<B>>,
}

impl<B: Backend> Clone for Releaser<B> {
    fn clone(&self) -> Self {
        Releaser {
            shared: self.shared.clone(),
        }
    }
}

impl<B: Backend> fmt::Debug for Releaser<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Releaser").finish_non_exhaustive()
    }
}

impl<B: Backend> Releaser<B> {
    /// Closes every session of `path` (compared as stored in the file list) and waits until
    /// none is left, at most `timeout`. Returns `true` when no session of `path` is left, at once
    /// when the core has none.
    ///
    /// Owned sessions are dropped and their bytes taken off the session budget, as an eviction
    /// does; a session lent to a decode is dropped when it comes back. While it waits, no new
    /// session of `path` is opened. The views of `path` are kept: writing metadata does not change
    /// the image data, and the next RAW render opens a new session as usual. Recorded as
    /// `"release"`.
    pub fn release(&self, path: &Path, timeout: Duration) -> bool {
        // a timeout too large for the clock waits without a deadline
        let deadline = Instant::now().checked_add(timeout);
        let mut changed = false;
        let mut g = self.shared.state();
        g.record("release");
        g.begin_release(path);
        let none_left = loop {
            let (dropped, none_left) = g.release(path);
            if dropped {
                changed = true;
                // a worker may now open what the plan wants
                self.shared.cv.notify_all();
            }
            if none_left {
                break true;
            }
            let wait = match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if d <= now {
                        break false;
                    }
                    d - now
                }
                None => IDLE_WAIT,
            };
            g = match self.shared.cv.wait_timeout(g, wait) {
                Ok((g, _)) => g,
                Err(e) => e.into_inner().0,
            };
        };
        g.end_release(path);
        drop(g);
        // the plan may open the file again now
        self.shared.cv.notify_all();
        if changed {
            self.shared.wake_ui();
        }
        none_left
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Rgb;
    use crate::geometry::{ImageSize, Rect};
    use std::sync::mpsc;

    /// A simulated decoder whose renders wait at a gate until the test opens it, so a session
    /// stays lent for as long as the test wants.
    #[derive(Clone, Default)]
    struct Gate {
        inner: Arc<(Mutex<GateState>, Condvar)>,
    }

    #[derive(Default)]
    struct GateState {
        open: bool,
        waiting: usize,
    }

    impl Gate {
        fn pass(&self) {
            let (m, cv) = &*self.inner;
            let mut g = lock(m);
            g.waiting += 1;
            cv.notify_all();
            while !g.open {
                g = match cv.wait(g) {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
            }
            g.waiting -= 1;
        }

        fn open(&self) {
            let (m, cv) = &*self.inner;
            lock(m).open = true;
            cv.notify_all();
        }

        /// Waits until a render is held at the gate (a session is lent).
        fn wait_for_a_render(&self) {
            let (m, cv) = &*self.inner;
            let end = Instant::now() + Duration::from_secs(10);
            let mut g = lock(m);
            while g.waiting == 0 {
                let now = Instant::now();
                assert!(now < end, "no render reached the gate");
                g = match cv.wait_timeout(g, end - now) {
                    Ok((g, _)) => g,
                    Err(e) => e.into_inner().0,
                };
            }
        }
    }

    fn rgb(w: u32, h: u32) -> Rgb {
        Rgb {
            width: w,
            height: h,
            data: vec![0; (w * h * 3) as usize],
        }
    }

    impl Backend for Gate {
        type Session = u32;

        fn open(&self, _path: &Path, _source: Source) -> Result<(u32, ImageSize), DecodeError> {
            Ok((
                0,
                ImageSize {
                    w: 400,
                    h: 300,
                    flip: 0,
                },
            ))
        }

        fn render_half(&self, _session: &mut u32) -> Result<Rgb, DecodeError> {
            self.pass();
            Ok(rgb(200, 150))
        }

        fn render_region(&self, _session: &mut u32, r: Rect) -> Result<Rgb, DecodeError> {
            self.pass();
            Ok(rgb(r.w, r.h))
        }
    }

    const A: &str = "a.raw";

    /// A core on one file whose first render is held at the gate. Dropping it opens the gate
    /// first (also after a failed assertion), so the workers can stop.
    struct Lent {
        gate: Gate,
        core: ViewCore<Gate>,
    }

    impl Drop for Lent {
        fn drop(&mut self) {
            self.gate.open();
        }
    }

    fn core_with_a_lent_session() -> Lent {
        let gate = Gate::default();
        let config = Config {
            record: true,
            ..Config::default()
        };
        let core = match ViewCore::start(gate.clone(), config) {
            Ok(c) => c,
            Err(e) => panic!("start: {e}"),
        };
        let lent = Lent { gate, core };
        lent.core.open_folder(vec![PathBuf::from(A)]);
        lent.gate.wait_for_a_render();
        assert!(
            lent.core.memory().0 > 0,
            "the session is counted while lent"
        );
        lent
    }

    #[test]
    fn a_releaser_can_be_cloned_and_sent_to_another_thread() {
        fn check<T: Clone + Send + Sync + 'static>() {}
        check::<Releaser<Gate>>();
    }

    #[test]
    fn release_returns_true_only_after_the_lent_session_came_back() {
        let lent = core_with_a_lent_session();
        let releaser = lent.core.releaser().clone();
        let (tx, rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let _ = tx.send(releaser.release(Path::new(A), Duration::from_secs(10)));
        });
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(300)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "release returned while the session was lent"
        );
        lent.gate.open();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)), Ok(true));
        assert!(waiter.join().is_ok());
        assert!(
            lent.core
                .take_records()
                .iter()
                .any(|r| r.label == "release")
        );
    }

    #[test]
    fn release_returns_false_after_the_timeout_when_the_session_does_not_come_back() {
        let lent = core_with_a_lent_session();
        let timeout = Duration::from_millis(300);
        let t0 = Instant::now();
        assert!(!lent.core.releaser().release(Path::new(A), timeout));
        let took = t0.elapsed();
        assert!(took >= timeout, "returned after {took:?}");
        assert!(
            took < timeout + Duration::from_secs(2),
            "returned after {took:?}"
        );
    }

    // ---------- the clipping map (ADR 0017 U7b, T147) ----------

    use crate::backend::{ClippingMap, ImageStatistics};
    use crate::state::ClippingState;

    /// A simulated decoder that renders at once and has a clipping map (and no statistics).
    struct Mapped;

    impl Backend for Mapped {
        type Session = u32;

        fn open(&self, _path: &Path, _source: Source) -> Result<(u32, ImageSize), DecodeError> {
            Ok((
                0,
                ImageSize {
                    w: 400,
                    h: 300,
                    flip: 0,
                },
            ))
        }

        fn render_half(&self, _session: &mut u32) -> Result<Rgb, DecodeError> {
            Ok(rgb(200, 150))
        }

        fn render_region(&self, _session: &mut u32, r: Rect) -> Result<Rgb, DecodeError> {
            Ok(rgb(r.w, r.h))
        }

        fn clipping_map(&self, _session: &mut u32) -> Option<ClippingMap> {
            Some(ClippingMap {
                cell: 2,
                width: 200,
                height: 150,
                data: vec![ClippingMap::CLIPPED_G; 200 * 150],
            })
        }
    }

    /// Frames of an 800 × 600 view, with every finished view accepted, until `done` (or 10 s).
    fn frames_until(core: &ViewCore<Mapped>, done: impl Fn(&Frame) -> bool) -> Frame {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            for view in core.take_views() {
                let _ = core.accept(&view, 1024);
            }
            let _ = core.take_dropped();
            let frame = core.frame([800, 600]);
            if done(&frame) || Instant::now() > end {
                return frame;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_workers_make_the_clipping_map_only_while_it_is_wanted() {
        let core = match ViewCore::start(Mapped, Config::default()) {
            Ok(c) => c,
            Err(e) => panic!("start: {e}"),
        };
        core.open_folder(vec![PathBuf::from(A)]);
        // not wanted: the statistics are asked (this decoder has none), no map
        let f = frames_until(&core, |f| f.statistics.is_some());
        assert_eq!(f.statistics, Some(ImageStatistics::Unavailable));
        assert_eq!(f.clipping, ClippingState::NotWanted);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(core.stats().clipping_maps, 0);

        core.set_clipping_wanted(true);
        let f = frames_until(&core, |f| f.clipping != ClippingState::Reading);
        let ClippingState::Ready { map, uv } = f.clipping else {
            panic!("expected the map, got {:?}", f.clipping);
        };
        assert_eq!((map.cell, map.width, map.height), (2, 200, 150));
        // the whole image is drawn, and the map's grid is the image's
        assert_eq!(uv, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert_eq!(core.stats().clipping_maps, 1);
        assert_eq!(core.stats().clipping_ms.len(), 1);

        // off and on again: the same map, not made again
        core.set_clipping_wanted(false);
        assert_eq!(core.frame([800, 600]).clipping, ClippingState::NotWanted);
        core.set_clipping_wanted(true);
        let ClippingState::Ready { map: again, .. } = core.frame([800, 600]).clipping else {
            panic!("the map was not kept");
        };
        assert!(Arc::ptr_eq(&map, &again));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(core.stats().clipping_maps, 1);
    }

    #[test]
    fn release_of_a_path_without_sessions_returns_true_at_once() {
        let lent = core_with_a_lent_session();
        let t0 = Instant::now();
        assert!(
            lent.core
                .releaser()
                .release(Path::new("b.raw"), Duration::from_secs(10))
        );
        assert!(t0.elapsed() < Duration::from_secs(1));
        // the session of the other file is still lent and counted
        assert!(lent.core.memory().0 > 0);
    }
}
