//! The view core with its two workers (ADR 0008 §12): an interactive worker for the current image
//! only, and a background worker for the rest of the plan. Every user action returns at once (G2);
//! the decoding runs on the workers.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::{DecodeError, RawBackend};
use crate::scale;
use crate::state::{Config, Frame, Record, State, Stats, Task, ViewKind, ViewOut, ZOOM_STEP, Zoom};

/// The longest a worker sleeps without a reason (a safety net; changes wake it at once).
const IDLE_WAIT: Duration = Duration::from_secs(1);

type Wake = Box<dyn Fn() + Send + Sync>;

struct Shared<B: RawBackend> {
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

impl<B: RawBackend> Shared<B> {
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
                Task::Unpack {
                    generation,
                    pos,
                    path,
                } => {
                    let result = self.backend.open(&path);
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let mut s = self.state();
                    s.finish_unpack(
                        generation,
                        pos,
                        result,
                        |size| self.backend.session_bytes(size),
                        ms,
                    );
                }
                Task::Render {
                    generation,
                    pos,
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
                    s.finish_render(generation, pos, view, session, result, ms);
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
pub struct ViewCore<B: RawBackend> {
    shared: Arc<Shared<B>>,
    workers: Vec<JoinHandle<()>>,
}

impl<B: RawBackend> fmt::Debug for ViewCore<B> {
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

impl<B: RawBackend> ViewCore<B> {
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

    /// Drags the image by `delta` view pixels.
    pub fn pan(&self, delta: [f64; 2]) {
        self.act("pan", |s| s.pan(delta));
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

impl<B: RawBackend> Drop for ViewCore<B> {
    fn drop(&mut self) {
        self.shutdown();
    }
}
