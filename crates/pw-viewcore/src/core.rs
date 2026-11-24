//! The view core with its two workers (ADR 0008 §12): an interactive worker for the current image
//! only, and a background worker for the rest of the plan. Every user action returns at once (G2);
//! the decoding runs on the workers.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::{Backend, DecodeError, FocusMap, Rgb, Source};
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
                    // a view below 100 % is reduced on the worker with the Lanczos filter (ADR
                    // 0027 §3.2, W1-Q3)
                    let (w, h) = scale::scaled_size(view.region.w, view.region.h, view.density());
                    let reduce = |img: Rgb| {
                        scale::reduce(&img, w, h)
                            .ok_or_else(|| DecodeError::new("scaling the view failed"))
                    };
                    let result = match view.kind {
                        ViewKind::Half => self.backend.render_half(&mut session),
                        ViewKind::Zoom | ViewKind::Ring => {
                            self.backend.render_region(&mut session, view.region)
                        }
                        ViewKind::Reduced => {
                            self.backend.render_half(&mut session).and_then(|img| {
                                if img.width < w || img.height < h {
                                    // a backend whose half-size render is smaller than half the
                                    // image: it is used as it is rather than failing, and
                                    // `finish_render` gives it as the half-size view (T309 R1)
                                    Ok(img)
                                } else {
                                    reduce(img)
                                }
                            })
                        }
                        ViewKind::Scaled => self
                            .backend
                            .render_region(&mut session, view.region)
                            .and_then(reduce),
                    };
                    // to the display after the render and the reduction, before the check
                    // (ADR 0027 W1-Q4)
                    let result = result.and_then(|img| self.backend.to_display(&session, img));
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
                Task::Focus {
                    generation,
                    key,
                    mut session,
                } => {
                    // from the image's own half-size render, the pixels the view draws at fit
                    // (ADR 0017 U9-Q2), not converted to the display (ADR 0027 W1-Q4)
                    let result = self
                        .backend
                        .render_half(&mut session)
                        .ok()
                        .filter(Rgb::is_consistent)
                        .and_then(|img| FocusMap::of_rgb8(img.width, img.height, &img.data));
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let mut s = self.state();
                    s.finish_focus(generation, key, session, result, ms);
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

    /// Whether the focus marking wants the current image's focus map (ADR 0017 U9a): the shell
    /// sets it while the focus marking is on. While it is set, the map of the current image is
    /// made from its half-size render after its statistics and its clipping map, and given in
    /// [`Frame::focus`]; while it is not, no map is asked for. Not a user request: nothing is
    /// recorded.
    pub fn set_focus_wanted(&self, wanted: bool) {
        {
            let mut s = self.shared.state();
            if s.focus_wanted == wanted {
                return;
            }
            s.focus_wanted = wanted;
        }
        self.shared.cv.notify_all();
        self.shared.wake_ui();
    }

    /// Sets the view model's zoom level and position from outside, as if the user had zoomed and
    /// dragged there (ADR 0017 U6): `centre` is the relative point of the upright image at the
    /// view's centre, as [`Frame::centre`] gives it. It sets the view model, not one image's,
    /// so it stays when stepping (V6).
    ///
    /// A scale that is not finite or not above 0, or a centre with a coordinate that is not
    /// finite, is ignored. A scale is clamped as for the zoom actions when the current image's
    /// size is known, and to 800 % only when it is not; the centre is clamped to 0–1. A change
    /// counts as one step of a zoom gesture, so the exact view is rendered when the calls pause
    /// (`Config::gesture_pause`); values equal to those held change nothing, so the shell may
    /// call it every frame. Not a user request: nothing is recorded.
    pub fn set_view(&self, zoom: Zoom, centre: [f64; 2]) {
        {
            let mut s = self.shared.state();
            if !s.set_view(zoom, centre) {
                return;
            }
            s.changed();
        }
        self.shared.cv.notify_all();
        self.shared.wake_ui();
    }

    /// Tells the orientation of `path` (ADR 0024 §2.4): a value in `1..=8` is used before the
    /// file's own orientation, `None` takes it back. It applies to the opened images of `path`
    /// at once and to those opened later, also after a new folder or list of files. No view is
    /// released (views are kept in sensor orientation). When the current image turns at a scale,
    /// the centre is turned with the image so the same image point stays in the centre (ADR 0024
    /// §2.5), and the scale is raised to the turned size's minimum when it is below it; fit stays
    /// fit. When a flip changed, the state is marked changed, the workers are notified and the
    /// UI is woken. Not a user request: nothing is recorded.
    pub fn set_orientation(&self, path: PathBuf, orientation: Option<u16>) {
        {
            let mut s = self.shared.state();
            if !s.set_orientation(&path, orientation) {
                return;
            }
            s.changed();
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

    // ---------- the focus map (ADR 0017 U9a, T167) ----------

    use crate::state::FocusState;

    #[test]
    fn the_workers_make_the_focus_map_from_the_half_size_render_only_while_it_is_wanted() {
        let core = match ViewCore::start(Mapped, Config::default()) {
            Ok(c) => c,
            Err(e) => panic!("start: {e}"),
        };
        core.open_folder(vec![PathBuf::from(A)]);
        // not wanted: the statistics are asked (this decoder has none), no map
        let f = frames_until(&core, |f| f.statistics.is_some());
        assert_eq!(f.statistics, Some(ImageStatistics::Unavailable));
        assert_eq!(f.focus, FocusState::NotWanted);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(core.stats().focus_maps, 0);

        core.set_focus_wanted(true);
        let f = frames_until(&core, |f| f.focus != FocusState::Reading);
        let FocusState::Ready { map, uv } = f.focus else {
            panic!("expected the map, got {:?}", f.focus);
        };
        // one cell of 2 × 2 per pixel of the decoder's own half-size render, 200 × 150
        assert_eq!((map.cell, map.width, map.height), (2, 200, 150));
        let half = rgb(200, 150);
        assert_eq!(
            Some(map.as_ref()),
            FocusMap::of_rgb8(half.width, half.height, &half.data).as_ref()
        );
        // the whole image is drawn, and the map covers it
        assert_eq!(uv, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert_eq!(core.stats().focus_maps, 1);
        assert_eq!(core.stats().focus_ms.len(), 1);
        // the clipping map was not asked for
        assert_eq!(f.clipping, ClippingState::NotWanted);
        assert_eq!(core.stats().clipping_maps, 0);

        // off and on again: the same map, not made again
        core.set_focus_wanted(false);
        assert_eq!(core.frame([800, 600]).focus, FocusState::NotWanted);
        core.set_focus_wanted(true);
        let FocusState::Ready { map: again, .. } = core.frame([800, 600]).focus else {
            panic!("the map was not kept");
        };
        assert!(Arc::ptr_eq(&map, &again));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(core.stats().focus_maps, 1);
    }

    // ---------- the conversion to the display after the reduction (ADR 0027 W1-Q4, T308) ----------

    use crate::state::Status;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The value of every byte the simulated [`Flat`] decoder renders.
    const FLAT: u8 = 100;

    /// A simulated decoder of 4000 × 3000 images in one flat colour (so a reduced view is the
    /// same colour), whose `to_display` adds 1 to every byte and records the size it got, or
    /// fails when `fail` is set.
    #[derive(Clone, Default)]
    struct Flat {
        renders: Arc<AtomicUsize>,
        /// How many of the renders were half-size renders.
        halves: Arc<AtomicUsize>,
        shown: Arc<Mutex<Vec<(u32, u32)>>>,
        fail: bool,
        /// The half-size render is malformed (too few samples), so it cannot be reduced.
        malformed_half: bool,
        /// The half-size render is 700 × 500, smaller than half the image (T309 R1).
        small_half: bool,
    }

    fn flat(w: u32, h: u32) -> Rgb {
        Rgb {
            width: w,
            height: h,
            data: vec![FLAT; (w * h * 3) as usize],
        }
    }

    impl Backend for Flat {
        type Session = u32;

        fn open(&self, _path: &Path, _source: Source) -> Result<(u32, ImageSize), DecodeError> {
            Ok((
                0,
                ImageSize {
                    w: 4000,
                    h: 3000,
                    flip: 0,
                },
            ))
        }

        fn render_half(&self, _session: &mut u32) -> Result<Rgb, DecodeError> {
            self.renders.fetch_add(1, Ordering::SeqCst);
            self.halves.fetch_add(1, Ordering::SeqCst);
            let mut half = if self.small_half {
                flat(700, 500)
            } else {
                flat(2000, 1500)
            };
            if self.malformed_half {
                half.data.truncate(5);
            }
            Ok(half)
        }

        fn render_region(&self, _session: &mut u32, r: Rect) -> Result<Rgb, DecodeError> {
            self.renders.fetch_add(1, Ordering::SeqCst);
            Ok(flat(r.w, r.h))
        }

        fn to_display(&self, _session: &u32, mut img: Rgb) -> Result<Rgb, DecodeError> {
            lock(&self.shown).push((img.width, img.height));
            if self.fail {
                return Err(DecodeError::new("the conversion to the display failed"));
            }
            for byte in &mut img.data {
                *byte = byte.saturating_add(1);
            }
            Ok(img)
        }
    }

    impl Flat {
        fn shown(&self) -> Vec<(u32, u32)> {
            lock(&self.shown).clone()
        }
    }

    /// Frames of an 800 × 600 view, with every finished view accepted and kept in `views`, until
    /// `done` (or 10 s).
    fn flat_frames_until(
        core: &ViewCore<Flat>,
        views: &mut Vec<ViewOut>,
        done: impl Fn(&Frame, &[ViewOut]) -> bool,
    ) -> Frame {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            for view in core.take_views() {
                let _ = core.accept(&view, 1024);
                views.push(view);
            }
            let _ = core.take_dropped();
            let frame = core.frame([800, 600]);
            if done(&frame, views) || Instant::now() > end {
                return frame;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn every_view_is_converted_to_the_display_after_its_reduction_and_the_focus_map_is_not() {
        let backend = Flat::default();
        let core = match ViewCore::start(backend.clone(), Config::default()) {
            Ok(c) => c,
            Err(e) => panic!("start: {e}"),
        };
        let mut views = Vec::new();
        core.open_folder(vec![PathBuf::from(A)]);
        core.set_focus_wanted(true);
        let f = flat_frames_until(&core, &mut views, |f, _| {
            f.status == Status::Ready && f.focus != FocusState::Reading
        });
        assert_eq!(f.status, Status::Ready);
        // the focus map is of the unconverted half-size render (W1-Q4)
        let FocusState::Ready { map, .. } = f.focus else {
            panic!("expected the focus map, got {:?}", f.focus);
        };
        let half = flat(2000, 1500);
        assert_eq!(
            Some(map.as_ref()),
            FocusMap::of_rgb8(half.width, half.height, &half.data).as_ref()
        );

        // a free level between 50 % and 100 %: a view reduced on the worker (near a corner, where
        // the 100 % views made around the centre do not cover the visible region)
        core.set_view(Zoom::Scale(0.75), [0.15, 0.15]);
        flat_frames_until(&core, &mut views, |f, v| {
            f.status == Status::Ready && v.iter().any(|v| v.kind == ViewKind::Scaled)
        });
        // every render that was not a focus render, and nothing else, went through to_display
        let end = Instant::now() + Duration::from_secs(10);
        while backend.shown().len() as u64 != core.stats().renders && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(200));
        let stats = core.stats();
        let shown = backend.shown();
        assert_eq!(shown.len() as u64, stats.renders);
        assert_eq!(stats.focus_maps, 1);
        assert_eq!(
            backend.renders.load(Ordering::SeqCst) as u64,
            stats.renders + stats.focus_maps,
            "the focus render is not converted"
        );

        // every view given out is its render plus 1
        flat_frames_until(&core, &mut views, |_, _| true);
        assert!(!views.is_empty());
        for view in &views {
            assert!(view.image.is_consistent(), "{view:?}");
            assert!(
                view.image.data.iter().all(|b| *b == FLAT + 1),
                "{:?} {:?} is not its render plus 1",
                view.kind,
                view.region
            );
        }

        // the scaled view reached to_display at its reduced size
        let Some(scaled) = views.iter().find(|v| v.kind == ViewKind::Scaled) else {
            panic!("no scaled view in {views:?}");
        };
        let reduced = (scaled.image.width, scaled.image.height);
        let region = (scaled.region.w, scaled.region.h);
        assert!(
            reduced.0 < region.0 && reduced.1 < region.1,
            "{reduced:?} of {region:?}"
        );
        assert!(shown.contains(&reduced), "{shown:?} lacks {reduced:?}");
        assert!(!shown.contains(&region), "{shown:?} has {region:?}");
    }

    #[test]
    fn a_failed_conversion_to_the_display_is_the_frames_error() {
        let backend = Flat {
            fail: true,
            ..Flat::default()
        };
        let core = match ViewCore::start(backend.clone(), Config::default()) {
            Ok(c) => c,
            Err(e) => panic!("start: {e}"),
        };
        let mut views = Vec::new();
        core.open_folder(vec![PathBuf::from(A)]);
        let f = flat_frames_until(&core, &mut views, |f, _| {
            matches!(f.status, Status::Error(_))
        });
        let Status::Error(e) = f.status else {
            panic!("expected an error, got {:?}", f.status);
        };
        assert_eq!(e.message, "the conversion to the display failed");
        assert!(!e.damaged);
        assert!(views.is_empty(), "{views:?}");
        assert!(!backend.shown().is_empty());
        assert!(core.stats().failures >= 1);
    }

    // ---------- the reduced view below 50 % (ADR 0027 W1, T309) ----------

    /// A core on [`Flat`] whose view is 800 × 600 before the folder opens (so the first job is
    /// for that size), and which prefetches nothing (a dwell of an hour).
    fn flat_core(backend: &Flat) -> ViewCore<Flat> {
        let config = Config {
            dwell: Duration::from_secs(3600),
            ..Config::default()
        };
        let core = match ViewCore::start(backend.clone(), config) {
            Ok(c) => c,
            Err(e) => panic!("start: {e}"),
        };
        let _ = core.frame([800, 600]);
        core.open_folder(vec![PathBuf::from(A)]);
        core
    }

    #[test]
    fn a_reduced_view_is_the_half_size_render_reduced_then_converted_to_the_display() {
        let backend = Flat::default();
        let core = flat_core(&backend);
        let mut views = Vec::new();
        let f = flat_frames_until(&core, &mut views, |f, _| f.status == Status::Ready);
        assert_eq!(f.status, Status::Ready);
        // 4000 × 3000 at fit in 800 × 600: 0.2, one reduced view of 800 × 600
        assert_eq!(views.len(), 1, "{views:?}");
        let view = &views[0];
        assert_eq!(view.kind, ViewKind::Reduced);
        assert_eq!((view.image.width, view.image.height), (800, 600));
        assert_eq!(backend.halves.load(Ordering::SeqCst), 1);
        assert_eq!(core.stats().renders, 1);
        // converted once, at the reduced size, after the reduction
        assert_eq!(backend.shown(), vec![(800, 600)]);
        assert!(view.image.data.iter().all(|b| *b == FLAT + 1));
    }

    #[test]
    fn a_half_size_render_smaller_than_the_reduced_view_is_rendered_once_and_shown() {
        let backend = Flat {
            small_half: true,
            ..Flat::default()
        };
        let core = flat_core(&backend);
        let mut views = Vec::new();
        let f = flat_frames_until(&core, &mut views, |f, _| f.status == Status::Ready);
        assert_eq!(f.status, Status::Ready);
        // a while longer: no second render of the same view
        let end = Instant::now() + Duration::from_millis(300);
        while Instant::now() < end {
            let f = flat_frames_until(&core, &mut views, |_, _| true);
            assert_eq!(f.status, Status::Ready);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(backend.halves.load(Ordering::SeqCst), 1);
        assert_eq!(core.stats().renders, 1);
        // the 700 × 500 render, unreduced, shown as the half-size view and converted
        assert_eq!(views.len(), 1, "{views:?}");
        assert_eq!(views[0].kind, ViewKind::Half);
        assert_eq!((views[0].image.width, views[0].image.height), (700, 500));
        assert_eq!(backend.shown(), vec![(700, 500)]);
        assert!(f.draw.is_some());
    }

    #[test]
    fn a_failed_reduction_is_the_views_error() {
        let backend = Flat {
            malformed_half: true,
            ..Flat::default()
        };
        let core = flat_core(&backend);
        let mut views = Vec::new();
        let f = flat_frames_until(&core, &mut views, |f, _| {
            matches!(f.status, Status::Error(_))
        });
        let Status::Error(e) = f.status else {
            panic!("expected an error, got {:?}", f.status);
        };
        assert_eq!(e.message, "scaling the view failed");
        assert!(views.is_empty(), "{views:?}");
        assert!(backend.shown().is_empty(), "nothing reached to_display");
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
