//! The eframe + egui app (GUI_REQUIREMENTS GR1-GR3, GR6, GR9): uploads finished views as egui textures
//! and draws the current one. All decoding runs in the core's workers (GR2); the UI thread only uploads
//! and draws.
//!
//! - Swap rule (V2): the current view is drawn only when its texture is resident. Until then the last
//!   drawn view stays; after 1 s a neutral indicator is added (no image content of the new image).
//! - Orientation: textures are in sensor orientation; each corner of the upright view gets its own UV,
//!   so rotation costs nothing (`view::corner_uvs`).
//! - The status line says what is shown: a RAW rendering (PPG), never a JPEG (G1, GR9).

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::core::{Core, Kind, ViewOut, lock};
use crate::driver::{UiCmd, UiShared};
use crate::libraw::LibRaw;
use crate::view::{self, ImageSize, Rect};

const INDICATOR_AFTER: Duration = Duration::from_secs(1);

/// What was drawn last: kept on screen until the next view is ready (V2).
#[derive(Clone, Copy)]
struct Drawn {
    id: u64,
    dest: [f32; 4],
    uv: [[f32; 2]; 4],
}

pub struct App {
    core: Arc<Core<LibRaw>>,
    rx: Receiver<ViewOut>,
    ui: Arc<UiShared>,
    textures: HashMap<u64, egui::TextureHandle>,
    m0: Vec<egui::TextureHandle>,
    last: Option<Drawn>,
    interactive: bool,
    renderer: String,
    prev_frame: Option<Instant>,
}

impl App {
    pub fn new(core: Arc<Core<LibRaw>>, rx: Receiver<ViewOut>, ui: Arc<UiShared>, interactive: bool, renderer: String) -> Self {
        App { core, rx, ui, textures: HashMap::new(), m0: Vec::new(), last: None, interactive, renderer, prev_frame: None }
    }

    /// Runs the driver's requests that need the UI thread.
    fn ui_commands(&mut self, ctx: &egui::Context) {
        loop {
            let Some(cmd) = lock(&self.ui.cmds).pop_front() else { break };
            match cmd {
                UiCmd::M0Alloc { n, w, h } => {
                    while self.m0.len() < n {
                        let shade = (self.m0.len() * 37 % 200) as u8 + 30;
                        let img = egui::ColorImage::from_rgb([w, h], &vec![shade; w * h * 3]);
                        let t = ctx.load_texture(format!("m0-{}", self.m0.len()), img, egui::TextureOptions::LINEAR);
                        self.m0.push(t);
                    }
                }
                UiCmd::M0Keep { n } => self.m0.truncate(n),
                UiCmd::Close => {
                    eprintln!("p7: close command on the UI thread");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            self.ui.done.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Uploads finished views as textures, within the texture budget.
    fn upload(&mut self, ctx: &egui::Context) {
        let max_side = ctx.input(|i| i.max_texture_side);
        self.ui.max_texture_side.store(max_side as u64, Ordering::SeqCst);
        while let Ok(out) = self.rx.try_recv() {
            if out.w == 0 || out.h == 0 || out.w > max_side || out.h > max_side {
                self.ui.too_large.fetch_add(1, Ordering::SeqCst);
                self.core.state().reject(&out);
                continue;
            }
            let bytes = out.w * out.h * 4;
            let (id, dropped) = self.core.state().accept(&out, bytes);
            self.drop_ids(&dropped);
            let Some(id) = id else { continue };
            let t = Instant::now();
            let img = egui::ColorImage::from_rgb([out.w, out.h], &out.rgb);
            let opts = if out.kind == Kind::Fit { egui::TextureOptions::LINEAR } else { egui::TextureOptions::NEAREST };
            let tex = ctx.load_texture(format!("{}-{}-{}", out.pos, out.kind.name(), id), img, opts);
            lock(&self.ui.upload_ms).push(t.elapsed().as_secs_f64() * 1000.0);
            self.textures.insert(id, tex);
        }
    }

    fn drop_ids(&mut self, ids: &[u64]) {
        for id in ids {
            self.textures.remove(id);
        }
    }

    fn keys(&self, ctx: &egui::Context) {
        let pressed = |k: egui::Key| ctx.input(|i| i.key_pressed(k));
        let cur = self.core.state().cur.unwrap_or(0);
        if pressed(egui::Key::ArrowRight) {
            self.core.go(cur + 1, "key next");
        }
        if pressed(egui::Key::ArrowLeft) {
            self.core.go(cur.saturating_sub(1), "key previous");
        }
        if pressed(egui::Key::Z) {
            let z = self.core.state().zoomed;
            self.core.zoom(!z, "key z");
        }
        for (k, dx) in [(egui::Key::D, 0.03f32), (egui::Key::A, -0.03)] {
            if pressed(k) {
                // as P6b/P6c: wrap back to the middle near the edges, so a long run keeps panning
                let c = self.core.state().center[0];
                let dx = if !(0.2..=0.8).contains(&(c + dx)) { 0.5 - c } else { dx };
                self.core.pan(dx, 0.0, "key pan");
            }
        }
    }

    fn mesh(&self, painter: &egui::Painter, origin: egui::Pos2, d: &Drawn) -> bool {
        let Some(tex) = self.textures.get(&d.id) else { return false };
        let mut mesh = egui::Mesh::with_texture(tex.id());
        let pts = [(d.dest[0], d.dest[1]), (d.dest[2], d.dest[1]), (d.dest[2], d.dest[3]), (d.dest[0], d.dest[3])];
        for ((x, y), uv) in pts.into_iter().zip(d.uv) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: egui::pos2(origin.x + x, origin.y + y),
                uv: egui::pos2(uv[0], uv[1]),
                color: egui::Color32::WHITE,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
        true
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, egui::Color32::from_gray(32));
        let ppp = ui.ctx().pixels_per_point();
        let view_pts = [rect.width(), rect.height()];
        let view_px = [(rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32];
        // what to draw now
        let mut current: Option<Drawn> = None;
        let mut waiting_since: Option<Instant> = None;
        let mut status = String::new();
        {
            let mut g = self.core.state();
            g.view_px = view_px;
            if let Some(pos) = g.cur {
                let kind = if g.zoomed { Kind::Zoom } else { Kind::Fit };
                let size = g.sizes.get(&pos).copied();
                let found = g.serving(pos, kind).map(|r| (r.id, r.region));
                match (size, found) {
                    (Some(size), Some((id, region))) => {
                        if let Some(d) = placement(size, kind, region, view_pts, view_px, ppp, g.center) {
                            current = Some(Drawn { id, ..d });
                            g.touch(id);
                        }
                    }
                    _ => {
                        waiting_since = g.open.and_then(|i| g.recs.get(i)).map(|r| r.t);
                    }
                }
                status = format!(
                    "#{pos}  {}  RAW (PPG, LibRaw)  textures {:.0}/{:.0} MB  sessions {:.0} MB  renderer {}",
                    if g.zoomed { "100 %" } else { "fit" },
                    g.tex_bytes as f64 / 1048576.0,
                    g.tex_budget as f64 / 1048576.0,
                    g.sess_bytes as f64 / 1048576.0,
                    self.renderer
                );
            }
        }
        let origin = rect.min;
        match current {
            Some(d) if self.mesh(&painter, origin, &d) => {
                self.last = Some(d);
                self.core.state().mark_shown();
            }
            _ => {
                // V2: the previous view stays until the new one is complete
                if let Some(d) = self.last {
                    self.mesh(&painter, origin, &d);
                }
                if waiting_since.is_some_and(|t| t.elapsed() >= INDICATOR_AFTER) {
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Förbereder RAW …", egui::FontId::proportional(18.0), egui::Color32::LIGHT_GRAY);
                }
            }
        }
        painter.text(
            egui::pos2(rect.min.x + 8.0, rect.max.y - 6.0),
            egui::Align2::LEFT_BOTTOM,
            status,
            egui::FontId::monospace(12.0),
            egui::Color32::from_gray(200),
        );
    }
}

/// Destination and UVs for the current view (fit: the whole upright image; 100 %: the visible region).
fn placement(size: ImageSize, kind: Kind, tex: Rect, view_pts: [f32; 2], view_px: [u32; 2], ppp: f32, center: [f32; 2]) -> Option<Drawn> {
    match kind {
        Kind::Fit => {
            let (ow, oh) = size.oriented();
            let dest = view::fit_dest(size, view_pts)?;
            Some(Drawn { id: 0, dest, uv: view::corner_uvs(size, tex, [0, 0, ow, oh]) })
        }
        Kind::Zoom | Kind::Ring => {
            let vis = view::visible_oriented(size, view_px, center);
            Some(Drawn { id: 0, dest: view::actual_dest(vis, view_pts, ppp), uv: view::corner_uvs(size, tex, vis) })
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        if let Some(p) = self.prev_frame {
            lock(&self.ui.frame_ms).push((now - p).as_secs_f64() * 1000.0);
        }
        self.prev_frame = Some(now);
        self.ui.frames.fetch_add(1, Ordering::SeqCst);
        self.ui_commands(&ctx);
        if self.core.state().clear_textures {
            let ids = self.core.state().textures_cleared();
            self.drop_ids(&ids);
            self.last = None;
        }
        self.upload(&ctx);
        if self.interactive {
            self.keys(&ctx);
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.draw(ui));
        // keep frames coming while something is on its way
        let busy = {
            let g = self.core.state();
            g.open.and_then(|i| g.recs.get(i)).is_some_and(|r| r.shown.is_none())
        };
        if busy {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
