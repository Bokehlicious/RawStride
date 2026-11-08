//! PhotoWorkflow Loupe (ADR 0009, milestone 1): the loupe as its own program beside the Tauri app.
//!
//! eframe with glow (GUI_REQUIREMENTS §7) hosts one image area. `pw-viewcore` decides what to show
//! and prepares it on its workers; `pw-render` uploads and draws. Metadata reads and writes are
//! queued to a separate worker; the UI only polls completed results (G2).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use eframe::egui;
use metadata::MetadataCache;
use pw_loupe::{
    Action, Options, WINDOW_TITLE, error_text, indicator_text, key_action, list_folder, mode_label,
    parse_args, status_line, wheel_factor,
};
use pw_metadata::MetadataWorker;
use pw_render::{Textures, view_px};
use pw_viewcore::native::NativeBackend;
use pw_viewcore::{Config, Status, ViewCore};

mod metadata;

const BACKGROUND: egui::Color32 = egui::Color32::from_gray(32);
const STATUS_HEIGHT: f32 = 24.0;

struct Loupe {
    core: ViewCore<NativeBackend>,
    metadata_worker: MetadataWorker,
    metadata: MetadataCache,
    textures: Textures,
    anchor_at_pointer: bool,
    show_info: bool,
    message: Option<String>,
    indicator_after: Duration,
}

impl Loupe {
    fn open(&mut self, path: &Path) {
        match list_folder(path) {
            Ok((files, _)) if files.is_empty() => {
                self.message = Some(format!("Inga bilder i {}", path.display()));
            }
            Ok((files, pos)) => {
                self.message = None;
                let files = match self.metadata.begin_load(files, &mut self.metadata_worker) {
                    Ok(files) => files,
                    Err(error) => {
                        self.message = Some(error);
                        return;
                    }
                };
                self.core.open_folder(files);
                if pos > 0 {
                    self.core.go(pos);
                }
            }
            Err(e) => self.message = Some(format!("Kan inte öppna: {e}")),
        }
    }

    /// The anchor of a zoom: the pointer (physical pixels inside the image area) or the centre.
    fn anchor(&self, pointer: Option<egui::Pos2>, rect: egui::Rect, ppp: f32) -> Option<[f64; 2]> {
        if !self.anchor_at_pointer {
            return None;
        }
        let p = pointer.filter(|p| rect.contains(*p))?;
        Some([
            f64::from((p.x - rect.min.x) * ppp),
            f64::from((p.y - rect.min.y) * ppp),
        ])
    }

    fn act(&mut self, action: Action, anchor: Option<[f64; 2]>) {
        match action {
            Action::Next => self.core.step(1),
            Action::Previous => self.core.step(-1),
            Action::First => self.core.go(0),
            Action::Last => self.core.last(),
            Action::ToggleZoom => self.core.toggle_zoom(anchor),
            Action::Fit => self.core.fit(),
            Action::ZoomStep(n) => self.core.zoom_steps(n, anchor),
            Action::ToggleAnchor => self.anchor_at_pointer = !self.anchor_at_pointer,
            Action::ToggleInfo => self.show_info = !self.show_info,
            Action::ToggleMode => self.core.toggle_mode(),
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.metadata.poll(&self.metadata_worker);
        // a folder or a file dropped on the window (ADR 0009 §11, decision 2)
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .find(|p| !p.as_os_str().is_empty())
        });
        if let Some(path) = dropped {
            self.open(&path);
        }
        let full = ui.max_rect();
        let status_h = if self.show_info { STATUS_HEIGHT } else { 0.0 };
        let rect = egui::Rect::from_min_max(
            full.min,
            egui::pos2(full.max.x, (full.max.y - status_h).max(full.min.y + 1.0)),
        );
        let ppp = ctx.pixels_per_point();
        let response = ui.interact(
            rect,
            ui.scope_id().with("image"),
            egui::Sense::click_and_drag(),
        );
        let pointer = response.hover_pos();
        let anchor = self.anchor(pointer, rect, ppp);
        // keys
        let actions: Vec<Action> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => key_action(*key, modifiers.ctrl || modifiers.command, modifiers.alt),
                    _ => None,
                })
                .collect()
        });
        for a in actions {
            self.act(a, anchor);
        }
        // mouse: double-click = Z, drag = pan, wheel = zoom
        if response.double_clicked() {
            self.core.toggle_zoom(anchor);
        } else if response.dragged() {
            let d = response.drag_delta();
            if d != egui::Vec2::ZERO {
                self.core.pan([f64::from(d.x * ppp), f64::from(d.y * ppp)]);
            }
        }
        if response.hovered() {
            let dy = ctx.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.core.zoom_by(wheel_factor(dy), anchor);
            }
        }
        // views to textures, then the frame
        let max_side = ctx.input(|i| i.max_texture_side);
        self.textures.sync(&ctx, &self.core, max_side);
        let frame = self.core.frame(view_px(rect, ppp));
        let painter = ui.painter().clone();
        painter.rect_filled(full, 0.0, BACKGROUND);
        self.textures.paint(&painter, rect, ppp, &frame);
        let centre_text = |text: &str| {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(18.0),
                egui::Color32::LIGHT_GRAY,
            );
        };
        match &frame.status {
            Status::Error(e) => centre_text(&error_text(e, frame.mode)),
            Status::Preparing {
                indicator: true, ..
            } => centre_text(&indicator_text(&frame)),
            Status::Preparing {
                since,
                indicator: false,
            } => {
                // wake up when the indicator is due (V2)
                let due = self.indicator_after.saturating_sub(since.elapsed());
                ctx.request_repaint_after(due.max(Duration::from_millis(20)));
            }
            Status::Empty => {
                let text = self
                    .message
                    .clone()
                    .unwrap_or_else(|| status_line(&frame, true));
                centre_text(&text);
            }
            Status::Ready => {}
        }
        let metadata_notice = self.metadata.notice(frame.path.as_deref());
        if metadata_notice
            .as_deref()
            .is_some_and(|notice| notice == "Metadata loading…")
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.show_info {
            let bar = egui::Rect::from_min_max(egui::pos2(full.min.x, rect.max.y), full.max);
            let mut line = status_line(&frame, self.anchor_at_pointer);
            if let Some(m) = &self.message {
                line = format!("{line}  ·  {m}");
            }
            if let Some(notice) = &metadata_notice {
                line = format!("{line}  ·  {notice}");
            }
            if let Some(rating) = self.metadata.rating_summary(frame.path.as_deref()) {
                line = format!("{line}  ·  {rating}");
            }
            painter.text(
                egui::pos2(bar.min.x + 8.0, bar.center().y),
                egui::Align2::LEFT_CENTER,
                line,
                egui::FontId::monospace(12.0),
                egui::Color32::from_gray(210),
            );
            // the display-mode control (G4: a key and a control)
            let button = egui::Rect::from_min_size(
                egui::pos2(bar.max.x - 130.0, bar.min.y + 2.0),
                egui::vec2(124.0, STATUS_HEIGHT - 4.0),
            );
            if ui
                .put(button, egui::Button::new(mode_label(frame.mode)))
                .clicked()
            {
                self.core.toggle_mode();
            }
        } else if matches!(frame.status, Status::Ready)
            && let Some(notice) = metadata_notice
            && notice.starts_with("Metadata error")
        {
            centre_text(&notice);
        }
    }
}

impl eframe::App for Loupe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.draw(ui));
    }
}

fn run(options: Options) -> Result<(), String> {
    let config = Config {
        core_budget: options.core_mb << 20,
        texture_budget: options.texture_mb << 20,
        ..Config::default()
    };
    let indicator_after = config.indicator_after;
    let core = ViewCore::start(NativeBackend, config)?;
    let metadata_worker = MetadataWorker::start().map_err(|e| e.to_string())?;
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_maximized(true)
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let path = options.path;
    eframe::run_native(
        WINDOW_TITLE,
        native,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            core.set_wake(move || ctx.request_repaint());
            let mut loupe = Loupe {
                core,
                metadata_worker,
                metadata: MetadataCache::default(),
                textures: Textures::new(),
                anchor_at_pointer: true,
                show_info: true,
                message: None,
                indicator_after,
            };
            if let Some(p) = path {
                loupe.open(&p);
            }
            Ok(Box::new(loupe))
        }),
    )
    .map_err(|e| format!("{e}"))
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    match run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pw-loupe: {e}");
            ExitCode::FAILURE
        }
    }
}
