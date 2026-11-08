//! PhotoWorkflow Loupe (ADR 0009, milestone 1): the loupe as its own program beside the Tauri app.
//!
//! eframe with glow (GUI_REQUIREMENTS §7) hosts one image area. `pw-viewcore` decides what to show
//! and prepares it on its workers; `pw-render` uploads and draws. Metadata reads and writes are
//! queued to a separate worker; the UI only polls completed results (G2).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use eframe::egui;
use metadata::MetadataCache;
use pw_loupe::{
    Action, CellState, DEFAULT_THUMBNAIL_SIZE, GridKeyAction, Options, TabAction, Tabs, ThumbGrid,
    ViewMode, WINDOW_TITLE, color_image, columns, error_text, grid_key_action, indicator_text,
    key_action, list_folder, mode_label, navigate, parse_args, request_cell, status_line,
    tab_action, visible_range, wheel_factor,
};
use pw_metadata::MetadataWorker;
use pw_render::{Textures, view_px};
use pw_thumbcache::{ThumbWorker, ThumbnailRequest};
use pw_viewcore::native::NativeBackend;
use pw_viewcore::{Config, Status, ViewCore};

mod metadata;

const BACKGROUND: egui::Color32 = egui::Color32::from_gray(32);
const STATUS_HEIGHT: f32 = 24.0;
const GRID_SPACING: f32 = 8.0;
const GRID_LABEL_HEIGHT: f32 = 18.0;

struct Loupe {
    core: ViewCore<NativeBackend>,
    metadata_worker: MetadataWorker,
    metadata: MetadataCache,
    thumb_worker: ThumbWorker,
    thumb_grid: ThumbGrid,
    thumb_generation: u64,
    thumb_textures: HashMap<usize, egui::TextureHandle>,
    thumb_size: u32,
    /// The column count of the last drawn grid, so key navigation uses the same layout as the
    /// drawing (the scroll bar makes the drawing area narrower than the image area).
    grid_columns: usize,
    textures: Textures,
    anchor_at_pointer: bool,
    show_info: bool,
    message: Option<String>,
    indicator_after: Duration,
    tabs: Tabs,
}

impl Loupe {
    fn open(&mut self, path: &Path, new_tab: bool) {
        match list_folder(path) {
            Ok((files, pos)) => {
                let folder = if path.is_dir() {
                    path.to_path_buf()
                } else {
                    path.parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or(Path::new("."))
                        .to_path_buf()
                };
                if new_tab {
                    self.tabs.open_new(folder, files, pos);
                } else {
                    self.tabs.open_in_active(folder, files, pos);
                }
                self.load_active_tab();
            }
            Err(e) => self.message = Some(format!("Kan inte öppna: {e}")),
        }
    }

    fn load_active_tab(&mut self) {
        self.thumb_generation = self.thumb_generation.wrapping_add(1);
        self.thumb_worker.set_generation(self.thumb_generation);
        let Some(tab) = self.tabs.active().cloned() else {
            self.thumb_grid = ThumbGrid::new(self.thumb_generation, 0);
            self.thumb_textures.clear();
            return;
        };
        self.thumb_grid = ThumbGrid::new(self.thumb_generation, tab.files.len());
        self.thumb_textures.clear();
        let files = match self
            .metadata
            .begin_load(tab.files, &mut self.metadata_worker)
        {
            Ok(files) => files,
            Err(error) => {
                self.message = Some(error);
                return;
            }
        };
        self.core.open_folder(files);
        self.core.go(tab.current);
        self.message = if self
            .tabs
            .active()
            .is_some_and(|active| active.files.is_empty())
        {
            Some(format!(
                "Inga bilder i {}",
                tab.folder
                    .as_deref()
                    .map(|folder| folder.display().to_string())
                    .unwrap_or_else(|| "mappen".to_string())
            ))
        } else {
            None
        };
    }

    fn poll_thumbnails(&mut self, ctx: &egui::Context) {
        loop {
            match self.thumb_worker.try_recv() {
                Ok(event) => {
                    if !self.thumb_grid.apply(&event) {
                        continue;
                    }
                    match event.result {
                        Ok(image) => {
                            if let Some(image) = color_image(&image) {
                                let texture = ctx.load_texture(
                                    format!("thumbnail-{}-{}", self.thumb_generation, event.index),
                                    image,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.thumb_textures.insert(event.index, texture);
                            } else {
                                self.message = Some("Ogiltig miniatyrdata".to_string());
                            }
                        }
                        Err(_) => {
                            self.thumb_textures.remove(&event.index);
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.message = Some("Förhandsvisningsarbetaren avslutades".to_string());
                    break;
                }
            }
        }
    }

    fn draw_grid(
        &mut self,
        ui: &mut egui::Ui,
        full: egui::Rect,
        rect: egui::Rect,
        scroll_to: Option<usize>,
    ) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        painter.rect_filled(full, 0.0, BACKGROUND);
        let Some(mut tab) = self.tabs.active().cloned() else {
            return;
        };
        let cell_size = self.thumb_size as f32;
        let stride = cell_size + GRID_SPACING;

        // The grid uses the image area only, so it never runs under the status line.
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("contact-sheet")
                .auto_shrink([false, false])
                .show_viewport(ui, |ui, viewport| {
                    let width = ui.available_width().max(0.0);
                    let column_count = columns(
                        width.min(u32::MAX as f32) as u32,
                        self.thumb_size,
                        GRID_SPACING as u32,
                    );
                    self.grid_columns = column_count;
                    let rows = tab.files.len().div_ceil(column_count);
                    let content_height = rows as f32 * stride;
                    ui.set_min_size(egui::vec2(width, content_height));

                    let visible = visible_range(
                        tab.files.len(),
                        column_count,
                        self.thumb_size,
                        GRID_SPACING as u32,
                        viewport.min.y,
                        viewport.height(),
                        1,
                    );
                    self.thumb_worker.set_visible(visible.clone());
                    let cell = request_cell(self.thumb_size, ctx.pixels_per_point());
                    for index in self.thumb_grid.to_request(visible.clone(), cell) {
                        if let Some(path) = tab.files.get(index) {
                            self.thumb_worker.request(ThumbnailRequest {
                                generation: self.thumb_generation,
                                index,
                                path: path.clone(),
                                cell,
                            });
                            self.thumb_grid.mark_requested(index, cell);
                        }
                    }
                    self.poll_thumbnails(&ctx);

                    let keep = visible_range(
                        tab.files.len(),
                        column_count,
                        self.thumb_size,
                        GRID_SPACING as u32,
                        viewport.min.y,
                        viewport.height(),
                        3,
                    );
                    for index in self.thumb_grid.forget_outside(keep) {
                        self.thumb_textures.remove(&index);
                    }

                    let origin = ui.min_rect().min;
                    let painter = ui.painter().clone();
                    let mut clicked = None;
                    for index in visible.clone() {
                        let Some(path) = tab.files.get(index) else {
                            continue;
                        };
                        let row = index / column_count;
                        let column = index % column_count;
                        let cell_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                origin.x + column as f32 * stride,
                                origin.y + row as f32 * stride,
                            ),
                            egui::vec2(cell_size, cell_size),
                        );
                        let image_size = (cell_size - GRID_LABEL_HEIGHT).max(1.0);
                        let image_rect = egui::Rect::from_center_size(
                            egui::pos2(cell_rect.center().x, cell_rect.min.y + image_size * 0.5),
                            egui::vec2(image_size, image_size),
                        );
                        painter.rect_filled(image_rect, 0.0, egui::Color32::from_gray(64));
                        match self.thumb_grid.state(index) {
                            CellState::Ready => {
                                if let Some(texture) = self.thumb_textures.get(&index) {
                                    let size = texture.size_vec2();
                                    let scale = (image_size / size.x.max(1.0))
                                        .min(image_size / size.y.max(1.0));
                                    let fitted = size * scale;
                                    let image_rect =
                                        egui::Rect::from_center_size(image_rect.center(), fitted);
                                    painter.image(
                                        texture.id(),
                                        image_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        egui::Color32::WHITE,
                                    );
                                }
                            }
                            CellState::Failed(error) => {
                                let text = if error == "ingen förhandsvisning" {
                                    error
                                } else {
                                    let shortened: String = error.chars().take(32).collect();
                                    if error.chars().count() > 32 {
                                        format!("{shortened}…")
                                    } else {
                                        shortened
                                    }
                                };
                                painter.text(
                                    image_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    text,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::LIGHT_GRAY,
                                );
                            }
                            CellState::Missing | CellState::Requested => {}
                        }

                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        let label_rect = egui::Rect::from_min_max(
                            egui::pos2(cell_rect.min.x, cell_rect.max.y - GRID_LABEL_HEIGHT),
                            cell_rect.max,
                        );
                        painter.with_clip_rect(label_rect).text(
                            label_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            name,
                            egui::FontId::proportional(12.0),
                            egui::Color32::LIGHT_GRAY,
                        );

                        if index == tab.current {
                            painter.rect_stroke(
                                cell_rect,
                                0.0,
                                egui::Stroke::new(2.0, egui::Color32::YELLOW),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let response = ui.interact(
                            cell_rect,
                            ui.make_persistent_id(("grid-cell", index)),
                            egui::Sense::click(),
                        );
                        if response.clicked() {
                            clicked = Some(index);
                        }
                    }

                    if let Some(index) = scroll_to {
                        let row = index / column_count;
                        let column = index % column_count;
                        let target = egui::Rect::from_min_size(
                            egui::pos2(
                                origin.x + column as f32 * stride,
                                origin.y + row as f32 * stride,
                            ),
                            egui::vec2(cell_size, cell_size),
                        );
                        ui.scroll_to_rect(target, Some(egui::Align::Center));
                    }
                    if let Some(index) = clicked {
                        self.tabs.set_current(index);
                        tab.current = index;
                    }
                    if visible
                        .clone()
                        .any(|index| matches!(self.thumb_grid.state(index), CellState::Requested))
                    {
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                });
        });

        let painter = ui.painter().clone();
        if tab.files.is_empty() {
            let text = self
                .message
                .as_deref()
                .unwrap_or("Dra en mapp eller en bild hit");
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(18.0),
                egui::Color32::LIGHT_GRAY,
            );
        }
        if self.show_info {
            let bar = egui::Rect::from_min_max(egui::pos2(full.min.x, rect.max.y), full.max);
            let mut line = if let Some(path) = tab.files.get(tab.current) {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                format!(
                    "{}  {} / {}",
                    name,
                    tab.current.saturating_add(1),
                    tab.files.len()
                )
            } else {
                "Ingen bild vald".to_string()
            };
            if let Some(message) = &self.message {
                line = format!("{line}  ·  {message}");
            }
            if let Some(notice) = self
                .tabs
                .active()
                .and_then(|active| active.files.get(active.current))
                .and_then(|path| self.metadata.notice(Some(path)))
            {
                line = format!("{line}  ·  {notice}");
            }
            painter.text(
                egui::pos2(bar.min.x + 8.0, bar.center().y),
                egui::Align2::LEFT_CENTER,
                line,
                egui::FontId::monospace(12.0),
                egui::Color32::from_gray(210),
            );
        }
    }

    fn tab_action(&mut self, action: TabAction) {
        match action {
            TabAction::Next => self.tabs.next(),
            TabAction::Previous => self.tabs.previous(),
            TabAction::Close => self.tabs.close_active(),
        }
        self.load_active_tab();
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
        // A folder or a file dropped on the window opens in a new tab (owner decision 2026-11-08,
        // ADR 0010 Q3). An empty active tab is filled instead, and an already open folder is
        // activated (GR12). Modifier keys are not used: Windows does not report them reliably
        // while dragging from Explorer.
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .find(|p| !p.as_os_str().is_empty())
        });
        if let Some(path) = dropped {
            self.open(&path, true);
        }
        let key_actions: Vec<(Option<TabAction>, egui::Key, bool, bool)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        let ctrl = modifiers.ctrl || modifiers.command;
                        let tab = tab_action(*key, ctrl, modifiers.shift, modifiers.alt);
                        Some((tab, *key, ctrl, modifiers.alt))
                    }
                    _ => None,
                })
                .collect()
        });
        let titles = self.tabs.titles();
        let active = self.tabs.active_index();
        let mut selected = None;
        egui::ScrollArea::horizontal()
            .id_salt("folder-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, title) in titles.iter().enumerate() {
                        if ui.selectable_label(index == active, title).clicked() {
                            selected = Some(index);
                        }
                    }
                });
            });
        if let Some(index) = selected
            && index != self.tabs.active_index()
            && self.tabs.activate(index)
        {
            self.load_active_tab();
        }
        let full = ui.available_rect_before_wrap();
        let status_h = if self.show_info { STATUS_HEIGHT } else { 0.0 };
        let rect = egui::Rect::from_min_max(
            full.min,
            egui::pos2(full.max.x, (full.max.y - status_h).max(full.min.y + 1.0)),
        );
        if self
            .tabs
            .active()
            .is_some_and(|tab| tab.mode == ViewMode::Grid)
        {
            let mut scroll_to = None;
            for (tab, key, ctrl, alt) in key_actions {
                if let Some(action) = tab {
                    self.tab_action(action);
                } else if let Some(GridKeyAction::Navigate(action)) =
                    grid_key_action(key, ctrl, alt)
                {
                    let count = self.tabs.active().map_or(0, |tab| tab.files.len());
                    let column_count = self.grid_columns;
                    let visible_rows = ((rect.height() / (self.thumb_size as f32 + GRID_SPACING))
                        .floor() as usize)
                        .max(1);
                    let current = self.tabs.active().map_or(0, |tab| tab.current);
                    self.tabs.set_current(navigate(
                        count,
                        column_count,
                        current,
                        action,
                        visible_rows,
                    ));
                    scroll_to = self.tabs.active().map(|tab| tab.current);
                }
            }
            self.draw_grid(ui, full, rect, scroll_to);
            return;
        }
        let ppp = ctx.pixels_per_point();
        let response = ui.interact(
            rect,
            ui.scope_id().with("image"),
            egui::Sense::click_and_drag(),
        );
        let pointer = response.hover_pos();
        let anchor = self.anchor(pointer, rect, ppp);
        for (tab, key, ctrl, alt) in key_actions {
            if let Some(action) = tab {
                self.tab_action(action);
            } else if let Some(action) = key_action(key, ctrl, alt) {
                self.act(action, anchor);
            }
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
        if let Some(pos) = frame.pos {
            self.tabs.set_current(pos);
        }
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
        let metadata_path = frame.path.as_deref();
        let metadata_notice = self.metadata.notice(metadata_path);
        if self.metadata.is_loading(metadata_path) {
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
            && self.metadata.is_error(frame.path.as_deref())
            && let Some(notice) = metadata_notice
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
    let thumb_worker = ThumbWorker::start().map_err(|e| e.to_string())?;
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
                thumb_worker,
                thumb_grid: ThumbGrid::new(0, 0),
                thumb_generation: 0,
                thumb_textures: HashMap::new(),
                thumb_size: DEFAULT_THUMBNAIL_SIZE,
                grid_columns: 1,
                textures: Textures::new(),
                anchor_at_pointer: true,
                show_info: true,
                message: None,
                indicator_after,
                tabs: Tabs::default(),
            };
            if let Some(p) = path {
                loupe.open(&p, false);
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
