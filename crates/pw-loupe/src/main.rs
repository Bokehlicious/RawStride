//! PhotoWorkflow Loupe (ADR 0009, milestone 1): the loupe as its own program beside the Tauri app.
//!
//! eframe with glow (GUI_REQUIREMENTS §7) hosts one image area. `pw-viewcore` decides what to show
//! and prepares it on its workers; `pw-render` uploads and draws. Metadata reads and writes are
//! queued to a separate worker; the UI only polls completed results (G2).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use eframe::egui;
use metadata::MetadataCache;
use pw_loupe::{
    Action, CellState, DEFAULT_THUMBNAIL_SIZE, DialogKey, DialogRequest, Field, GridKeyAction,
    ImportDialog, ImportDialogState, ImportEvent, ImportForm, ImportWorker, MAX_THUMBNAIL_SIZE,
    MIN_THUMBNAIL_SIZE, Options, SubmitError, TabAction, Tabs, ThumbGrid, ViewMode, WINDOW_TITLE,
    clamp_thumbnail_size, color_image, columns, dialog_key_action, done_lines, error_text,
    grid_key_action, indicator_text, iptc_progress_text, key_action, list_folder, loupe_key_action,
    mode_label, more_lines, navigate, opens_import, parse_args, preview_line, preview_summary,
    progress_text, request_cell, status_line, step_thumbnail_size, tab_action, take_wheel_steps,
    visible_range, wheel_factor,
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
    import_worker: ImportWorker,
    import_dialog: Option<ImportDialog>,
    last_form: ImportForm,
    detected_cards: Vec<PathBuf>,
    import_submit_error: Option<String>,
    import_hidden: bool,
    import_result_opened: bool,
    thumb_worker: ThumbWorker,
    thumb_grid: ThumbGrid,
    thumb_generation: u64,
    thumb_textures: HashMap<usize, egui::TextureHandle>,
    thumb_size: u32,
    /// The column count of the last drawn grid, so key navigation uses the same layout as the
    /// drawing (the scroll bar makes the drawing area narrower than the image area).
    grid_columns: usize,
    /// Ctrl + wheel movement not yet turned into a size step.
    wheel_notches: f32,
    textures: Textures,
    anchor_at_pointer: bool,
    show_info: bool,
    message: Option<String>,
    indicator_after: Duration,
    tabs: Tabs,
}

impl Loupe {
    fn open_import(&mut self) {
        if self.import_dialog.is_some() {
            self.import_hidden = false;
            return;
        }
        self.import_hidden = false;
        self.import_result_opened = false;
        self.import_dialog = Some(ImportDialog::new(self.last_form.clone()));
        self.import_submit_error = None;
        if let Err(error) = self.import_worker.cards() {
            self.import_submit_error = Some(submit_error_text(error));
        }
    }

    fn close_import(&mut self) {
        if let Some(dialog) = self.import_dialog.take() {
            self.last_form = dialog.form().clone();
        }
        self.import_hidden = false;
    }

    fn start_import(&mut self) {
        let request = self.import_dialog.as_mut().and_then(ImportDialog::start);
        let Some(DialogRequest::Run { form }) = request else {
            return;
        };
        match self.import_worker.run(form) {
            Ok(id) => {
                if let Some(dialog) = &mut self.import_dialog {
                    dialog.sent(id);
                }
                self.import_result_opened = false;
                self.import_submit_error = None;
            }
            Err(error) => {
                if let Some(dialog) = &mut self.import_dialog {
                    dialog.disconnected();
                }
                self.import_submit_error = Some(submit_error_text(error));
            }
        }
    }

    fn hidden_import_status(&self) -> Option<String> {
        if !self.import_hidden {
            return None;
        }
        match self.import_dialog.as_ref()?.state() {
            ImportDialogState::Running { progress, iptc } => Some(
                iptc.as_ref()
                    .map(|(done, total, current)| iptc_progress_text(*done, *total, current))
                    .unwrap_or_else(|| {
                        progress.as_ref().map_or_else(
                            || "Importerar 0 / 0".to_string(),
                            |progress| {
                                format!(
                                    "Importerar {} / {}",
                                    progress.files_done, progress.files_total
                                )
                            },
                        )
                    }),
            ),
            ImportDialogState::Done { done } => {
                if let Some(error) = &done.error {
                    Some(error.clone())
                } else if done.cancelled {
                    Some(format!("Importen avbruten: {} filer", done.copied))
                } else {
                    Some(format!("Importen klar: {} filer", done.copied))
                }
            }
            _ => None,
        }
    }

    fn poll_import_worker(&mut self, ctx: &egui::Context) {
        loop {
            match self.import_worker.try_recv() {
                Ok(ImportEvent::Cards { cards, .. }) => self.detected_cards = cards,
                Ok(event) => {
                    let folder_to_open = if let Some(dialog) = &mut self.import_dialog {
                        let was_running = dialog.cancel();
                        dialog.apply(&event);
                        if was_running
                            && matches!(dialog.state(), ImportDialogState::Done { .. })
                            && !self.import_result_opened
                        {
                            self.import_result_opened = true;
                            dialog.result_folder()
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(folder) = folder_to_open {
                        self.open(&folder, true);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if let Some(dialog) = &mut self.import_dialog {
                        dialog.disconnected();
                    }
                    break;
                }
            }
        }
        if self
            .import_dialog
            .as_ref()
            .is_some_and(ImportDialog::cancel)
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn draw_import_dialog(&mut self, ctx: &egui::Context) {
        if self.import_hidden {
            return;
        }
        let Some(dialog) = self.import_dialog.as_ref() else {
            return;
        };
        let mut form = dialog.form().clone();
        let original_form = form.clone();
        let state = dialog.state().clone();
        let can_close = dialog.can_close();
        let can_start = dialog.can_start();
        let running = dialog.cancel();
        let cards = self.detected_cards.clone();
        let mut rescan = false;
        let mut close = false;
        let mut start = false;
        let mut cancel = false;
        let mut hide = false;
        let mut new_import = false;

        egui::Window::new("Importera")
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!running, |ui| {
                    ui.label("Källa");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut form.source);
                        if ui.button("Välj…").clicked()
                            && let Some(path) =
                                pick_folder("Välj källa", &form.source, cards.first())
                        {
                            form.source = path.to_string_lossy().into_owned();
                        }
                    });
                    if let Some(message) = field_error(&state, Field::Source) {
                        ui.colored_label(egui::Color32::RED, message);
                    }
                    ui.horizontal_wrapped(|ui| {
                        for card in &cards {
                            if ui.button(card.display().to_string()).clicked() {
                                form.source = card.to_string_lossy().into_owned();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.label("Mål");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut form.dest);
                        if ui.button("Välj…").clicked()
                            && let Some(path) = pick_folder("Välj mål", &form.dest, None)
                        {
                            form.dest = path.to_string_lossy().into_owned();
                        }
                    });
                    if let Some(message) = field_error(&state, Field::Dest) {
                        ui.colored_label(egui::Color32::RED, message);
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut form.backup_on, "Backup");
                        ui.add_enabled_ui(form.backup_on, |ui| {
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut form.backup);
                                if ui.button("Välj…").clicked()
                                    && let Some(path) =
                                        pick_folder("Välj backupmål", &form.backup, None)
                                {
                                    form.backup = path.to_string_lossy().into_owned();
                                }
                            });
                        });
                    });
                    if let Some(message) = field_error(&state, Field::Backup) {
                        ui.colored_label(egui::Color32::RED, message);
                    }

                    ui.add_space(8.0);
                    ui.label("Mappmall");
                    ui.text_edit_singleline(&mut form.folder);
                    if let Some(message) = field_error(&state, Field::Folder) {
                        ui.colored_label(egui::Color32::RED, message);
                    }

                    ui.add_space(8.0);
                    ui.label("Filnamnsmall");
                    ui.text_edit_singleline(&mut form.name);
                    if let Some(message) = field_error(&state, Field::Name) {
                        ui.colored_label(egui::Color32::RED, message);
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Jobb");
                        ui.text_edit_singleline(&mut form.job);
                        ui.label("Första nr");
                        ui.add(egui::DragValue::new(&mut form.first_seq).range(0..=999_999));
                    });
                    ui.label(
                        egui::RichText::new(
                            "Variabler: {year} {year2} {month} {day} {hour} {minute} {second} \
                             {date} {time} {make} {model} {name} {seq} {seq:4} {job}",
                        )
                        .small()
                        .color(egui::Color32::GRAY),
                    );

                    ui.collapsing("IPTC-mall", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Fotograf");
                            ui.text_edit_singleline(&mut form.iptc.creator);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Copyright");
                            ui.add(
                                egui::TextEdit::singleline(&mut form.iptc.copyright)
                                    .hint_text("© 2026 Namn"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Credit");
                            ui.text_edit_singleline(&mut form.iptc.credit);
                        });
                        ui.horizontal(|ui| {
                            ui.label("E-post");
                            ui.text_edit_singleline(&mut form.iptc.email);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Telefon");
                            ui.text_edit_singleline(&mut form.iptc.phone);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Webb");
                            ui.text_edit_singleline(&mut form.iptc.url);
                        });
                        ui.label(
                            egui::RichText::new(
                                "Skrivs till .xmp-sidofilen för RAW och in i DNG/JPG/TIF. \
                                 Tomma fält lämnas orörda.",
                            )
                            .small()
                            .color(egui::Color32::GRAY),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Förhandsvisning");
                        if ui.button("Sök igen").clicked() {
                            rescan = true;
                        }
                    });
                    match &state {
                        ImportDialogState::Previewing => {
                            ui.label("Förhandsvisar …");
                        }
                        ImportDialogState::Editing {
                            preview: Some(preview),
                            ..
                        } => {
                            ui.label(preview_summary(preview));
                            egui::ScrollArea::vertical()
                                .max_height(160.0)
                                .show(ui, |ui| {
                                    for line in &preview.lines {
                                        ui.label(preview_line(line));
                                    }
                                    if let Some(more) = more_lines(preview) {
                                        ui.label(more);
                                    }
                                });
                        }
                        ImportDialogState::Editing { .. } => {}
                        ImportDialogState::Running { .. } | ImportDialogState::Done { .. } => {}
                    }
                });

                match &state {
                    ImportDialogState::Running { progress, iptc } => {
                        let (fraction, text) = if let Some((done, total, current)) = iptc {
                            let files_done = done.saturating_add(1);
                            let fraction = if *total == 0 {
                                0.0
                            } else {
                                (files_done as f32 / *total as f32).clamp(0.0, 1.0)
                            };
                            (fraction, iptc_progress_text(*done, *total, current))
                        } else {
                            let fraction = progress.as_ref().map_or(0.0, |progress| {
                                if progress.bytes_total == 0 {
                                    0.0
                                } else {
                                    (progress.bytes_done as f32 / progress.bytes_total as f32)
                                        .clamp(0.0, 1.0)
                                }
                            });
                            let text = progress
                                .as_ref()
                                .map(progress_text)
                                .unwrap_or_else(|| "Importerar …".to_string());
                            (fraction, text)
                        };
                        ui.add(egui::ProgressBar::new(fraction).text(text));
                        ui.horizontal(|ui| {
                            if ui.button("Avbryt").clicked() {
                                cancel = true;
                            }
                            if ui.button("Dölj").clicked() {
                                hide = true;
                            }
                        });
                    }
                    ImportDialogState::Done { done } => {
                        for line in done_lines(done) {
                            if line.error {
                                ui.colored_label(egui::Color32::RED, line.text);
                            } else {
                                ui.label(line.text);
                            }
                        }
                        if ui.button("Ny import").clicked() {
                            new_import = true;
                        }
                        if ui.button("Stäng").clicked() {
                            close = true;
                        }
                    }
                    ImportDialogState::Editing { .. } | ImportDialogState::Previewing => {
                        if let Some(error) = &self.import_submit_error {
                            ui.colored_label(egui::Color32::RED, error);
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(can_start, egui::Button::new("Starta import"))
                                .clicked()
                            {
                                start = true;
                            }
                            if ui
                                .add_enabled(can_close, egui::Button::new("Stäng"))
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    }
                }
                if matches!(state, ImportDialogState::Done { .. })
                    && let Some(error) = &self.import_submit_error
                {
                    ui.colored_label(egui::Color32::RED, error);
                }
            });

        if let Some(dialog) = &mut self.import_dialog
            && form != original_form
        {
            dialog.edit(|current| *current = form, Instant::now());
            self.import_submit_error = None;
        }
        if rescan && let Some(dialog) = &mut self.import_dialog {
            dialog.rescan(Instant::now());
            self.import_submit_error = None;
        }
        if start {
            self.start_import();
        }
        if cancel
            && self
                .import_dialog
                .as_ref()
                .is_some_and(ImportDialog::cancel)
        {
            self.import_worker.cancel();
        }
        if hide
            && self
                .import_dialog
                .as_ref()
                .is_some_and(ImportDialog::cancel)
        {
            self.import_hidden = true;
        }
        if new_import && let Some(dialog) = &mut self.import_dialog {
            dialog.back_to_editing();
            self.import_result_opened = false;
        }
        if close
            && self
                .import_dialog
                .as_ref()
                .is_some_and(ImportDialog::can_close)
        {
            self.close_import();
            return;
        }

        if let Some(request) = self
            .import_dialog
            .as_mut()
            .and_then(|dialog| dialog.tick(Instant::now()))
        {
            match request {
                DialogRequest::Preview { form, rescan } => {
                    match self.import_worker.preview(form, rescan) {
                        Ok(id) => {
                            if let Some(dialog) = &mut self.import_dialog {
                                dialog.sent(id);
                            }
                            self.import_submit_error = None;
                        }
                        Err(error) => {
                            if let Some(dialog) = &mut self.import_dialog {
                                match &error {
                                    SubmitError::QueueFull => dialog.rescan(Instant::now()),
                                    SubmitError::Stopped => dialog.disconnected(),
                                }
                            }
                            self.import_submit_error = Some(submit_error_text(error));
                        }
                    }
                }
                DialogRequest::Run { .. } => {}
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

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
                self.tabs.set_mode(if path.is_dir() {
                    ViewMode::Grid
                } else {
                    ViewMode::Loupe
                });
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
        if tab.mode == ViewMode::Grid {
            self.thumb_worker.resume();
        } else {
            self.thumb_worker.pause();
        }
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
        if tab.mode == ViewMode::Loupe {
            self.core.open_folder(files);
            self.core.go(tab.current);
        } else {
            self.core.open_folder(Vec::new());
        }
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

    /// Switches the active tab between grid and loupe without reloading the tab, so the
    /// thumbnails and their cache survive a visit to the loupe (ADR 0011 §4.2). Returns the
    /// current image, so the grid can scroll to it.
    fn switch_mode(&mut self, mode: ViewMode) -> Option<usize> {
        let tab = self.tabs.active().cloned()?;
        self.tabs.set_mode(mode);
        match mode {
            ViewMode::Loupe => {
                self.thumb_worker.pause();
                match self
                    .metadata
                    .begin_load(tab.files, &mut self.metadata_worker)
                {
                    Ok(files) => {
                        self.core.open_folder(files);
                        self.core.go(tab.current);
                    }
                    Err(error) => self.message = Some(error),
                }
            }
            ViewMode::Grid => {
                self.core.open_folder(Vec::new());
                self.thumb_worker.resume();
            }
        }
        Some(tab.current)
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
        let mut open_loupe = false;

        // The grid uses the image area only, so it never runs under the status line.
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            let ctrl_held = ui.input(|input| input.modifiers.ctrl || input.modifiers.command);
            egui::ScrollArea::vertical()
                .id_salt("contact-sheet")
                .scroll_source(egui::containers::scroll_area::ScrollSource {
                    mouse_wheel: !ctrl_held,
                    ..Default::default()
                })
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
                        if response.double_clicked() {
                            clicked = Some(index);
                            open_loupe = true;
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
        if open_loupe {
            self.switch_mode(ViewMode::Loupe);
        }

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
        let import_status = self.hidden_import_status();
        if self.show_info || import_status.is_some() {
            let bar = egui::Rect::from_min_max(egui::pos2(full.min.x, rect.max.y), full.max);
            let mut line = if !self.show_info {
                String::new()
            } else if let Some(path) = tab.files.get(tab.current) {
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
            if self.show_info
                && let Some(message) = &self.message
            {
                line = format!("{line}  ·  {message}");
            }
            if self.show_info
                && let Some(notice) = self
                    .tabs
                    .active()
                    .and_then(|active| active.files.get(active.current))
                    .and_then(|path| self.metadata.notice(Some(path)))
            {
                line = format!("{line}  ·  {notice}");
            }
            if let Some(status) = import_status {
                line = if line.is_empty() {
                    status
                } else {
                    format!("{line}  ·  {status}")
                };
            }
            if self.show_info {
                let slider = egui::Rect::from_min_max(
                    egui::pos2((bar.max.x - 180.0).max(bar.min.x), bar.min.y + 2.0),
                    egui::pos2(bar.max.x - 8.0, bar.max.y - 2.0),
                );
                painter
                    .with_clip_rect(egui::Rect::from_min_max(
                        bar.min,
                        egui::pos2((slider.min.x - 8.0).max(bar.min.x), bar.max.y),
                    ))
                    .text(
                        egui::pos2(bar.min.x + 8.0, bar.center().y),
                        egui::Align2::LEFT_CENTER,
                        line,
                        egui::FontId::monospace(12.0),
                        egui::Color32::from_gray(210),
                    );
                ui.put(
                    slider,
                    egui::Slider::new(
                        &mut self.thumb_size,
                        MIN_THUMBNAIL_SIZE..=MAX_THUMBNAIL_SIZE,
                    )
                    .text("Storlek")
                    .show_value(true),
                );
                self.thumb_size = clamp_thumbnail_size(self.thumb_size);
            } else {
                painter.text(
                    egui::pos2(bar.min.x + 8.0, bar.center().y),
                    egui::Align2::LEFT_CENTER,
                    line,
                    egui::FontId::monospace(12.0),
                    egui::Color32::from_gray(210),
                );
            }
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
        let key_actions: Vec<(Option<TabAction>, egui::Key, bool, bool, bool)> = ctx.input(|i| {
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
                        Some((tab, *key, ctrl, modifiers.shift, modifiers.alt))
                    }
                    _ => None,
                })
                .collect()
        });
        let titles = self.tabs.titles();
        let active = self.tabs.active_index();
        let mut selected = None;
        let mut open_import_clicked = false;
        egui::ScrollArea::horizontal()
            .id_salt("folder-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, title) in titles.iter().enumerate() {
                        if ui.selectable_label(index == active, title).clicked() {
                            selected = Some(index);
                        }
                    }
                    open_import_clicked = ui.button("Importera…").clicked();
                });
            });
        if open_import_clicked {
            self.open_import();
        }
        if let Some(index) = selected
            && index != self.tabs.active_index()
            && self.tabs.activate(index)
        {
            self.load_active_tab();
        }
        let full = ui.available_rect_before_wrap();
        let import_status = self.hidden_import_status();
        let status_h = if self.show_info || import_status.is_some() {
            STATUS_HEIGHT
        } else {
            0.0
        };
        let rect = egui::Rect::from_min_max(
            full.min,
            egui::pos2(full.max.x, (full.max.y - status_h).max(full.min.y + 1.0)),
        );
        let ppp = ctx.pixels_per_point();
        let key_anchor = self.anchor(ctx.input(|input| input.pointer.hover_pos()), rect, ppp);
        let mut scroll_to = None;
        let mut dialog_consumes_keys = self.import_dialog.is_some() && !self.import_hidden;
        let mut start_import = false;
        for (tab_action, key, ctrl, shift, alt) in key_actions {
            if dialog_consumes_keys {
                match dialog_key_action(key, ctrl, alt) {
                    Some(DialogKey::Close)
                        if self
                            .import_dialog
                            .as_ref()
                            .is_some_and(ImportDialog::can_close) =>
                    {
                        self.close_import();
                    }
                    Some(DialogKey::Start) => start_import = true,
                    _ => {}
                }
                continue;
            }
            if opens_import(key, ctrl, shift, alt) {
                self.open_import();
                dialog_consumes_keys = true;
                continue;
            }
            if let Some(action) = tab_action {
                self.tab_action(action);
                continue;
            }

            match self.tabs.active().map(|tab| tab.mode) {
                Some(ViewMode::Grid) => match grid_key_action(key, ctrl, alt) {
                    Some(GridKeyAction::OpenLoupe) => {
                        self.switch_mode(ViewMode::Loupe);
                    }
                    Some(GridKeyAction::Navigate(action)) => {
                        let count = self.tabs.active().map_or(0, |tab| tab.files.len());
                        let visible_rows = ((rect.height()
                            / (self.thumb_size as f32 + GRID_SPACING))
                            .floor() as usize)
                            .max(1);
                        let current = self.tabs.active().map_or(0, |tab| tab.current);
                        self.tabs.set_current(navigate(
                            count,
                            self.grid_columns,
                            current,
                            action,
                            visible_rows,
                        ));
                        scroll_to = self.tabs.active().map(|tab| tab.current);
                    }
                    None => {}
                },
                Some(ViewMode::Loupe) => {
                    if loupe_key_action(key, ctrl, alt).is_some() {
                        scroll_to = self.switch_mode(ViewMode::Grid);
                    } else if let Some(action) = key_action(key, ctrl, alt) {
                        self.act(action, key_anchor);
                    }
                }
                None => {}
            }
        }
        if start_import {
            self.start_import();
        }
        if self
            .tabs
            .active()
            .is_some_and(|tab| tab.mode == ViewMode::Grid)
        {
            // Ctrl + mouse wheel changes the size by one step per wheel notch. The raw wheel
            // events are used, not `zoom_delta()`, because egui spreads one notch over several
            // frames and each frame would otherwise be a step (owner feedback 2026-11-09).
            let (wheel, pointer) = ctx.input(|input| {
                let wheel: f32 = input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::MouseWheel {
                            unit,
                            delta,
                            modifiers,
                            ..
                        } if modifiers.ctrl || modifiers.command => Some(match unit {
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Point => delta.y / 50.0,
                            egui::MouseWheelUnit::Page => delta.y * 3.0,
                        }),
                        _ => None,
                    })
                    .sum();
                (wheel, input.pointer.hover_pos())
            });
            if pointer.is_some_and(|pointer| rect.contains(pointer)) {
                self.wheel_notches += wheel;
                let steps = take_wheel_steps(&mut self.wheel_notches);
                for _ in 0..steps.unsigned_abs() {
                    self.thumb_size = step_thumbnail_size(self.thumb_size, steps.signum());
                }
            } else {
                self.wheel_notches = 0.0;
            }
            self.draw_grid(ui, full, rect, scroll_to);
            return;
        }
        let response = ui.interact(
            rect,
            ui.scope_id().with("image"),
            egui::Sense::click_and_drag(),
        );
        let pointer = response.hover_pos();
        let anchor = self.anchor(pointer, rect, ppp);
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
        if self.show_info || import_status.is_some() {
            let bar = egui::Rect::from_min_max(egui::pos2(full.min.x, rect.max.y), full.max);
            let mut line = if self.show_info {
                status_line(&frame, self.anchor_at_pointer)
            } else {
                String::new()
            };
            if self.show_info
                && let Some(m) = &self.message
            {
                line = format!("{line}  ·  {m}");
            }
            if self.show_info
                && let Some(notice) = &metadata_notice
            {
                line = format!("{line}  ·  {notice}");
            }
            if self.show_info
                && let Some(rating) = self.metadata.rating_summary(frame.path.as_deref())
            {
                line = format!("{line}  ·  {rating}");
            }
            if let Some(status) = import_status {
                line = if line.is_empty() {
                    status
                } else {
                    format!("{line}  ·  {status}")
                };
            }
            painter.text(
                egui::pos2(bar.min.x + 8.0, bar.center().y),
                egui::Align2::LEFT_CENTER,
                line,
                egui::FontId::monospace(12.0),
                egui::Color32::from_gray(210),
            );
            // the display-mode control (G4: a key and a control)
            if self.show_info {
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
            }
        } else if matches!(frame.status, Status::Ready)
            && self.metadata.is_error(frame.path.as_deref())
            && let Some(notice) = metadata_notice
        {
            centre_text(&notice);
        }
    }
}

fn field_error(state: &ImportDialogState, field: Field) -> Option<&str> {
    match state {
        ImportDialogState::Editing {
            error: Some(error), ..
        } if error.field == field => Some(&error.message),
        _ => None,
    }
}

/// Opens the Windows folder dialog. It starts in the folder already typed in the field, else in
/// `fallback` (the first detected card for the source); Windows itself falls back to its default
/// folder when the start folder does not exist, so nothing is checked on disk here.
fn pick_folder(title: &str, current: &str, fallback: Option<&PathBuf>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    let current = current.trim();
    if !current.is_empty() {
        dialog = dialog.set_directory(current);
    } else if let Some(fallback) = fallback {
        dialog = dialog.set_directory(fallback);
    }
    dialog.pick_folder()
}

fn submit_error_text(error: SubmitError) -> String {
    match error {
        SubmitError::QueueFull => "Importarbetarens kö är full.".to_string(),
        SubmitError::Stopped => "Importarbetaren har stoppat.".to_string(),
    }
}

impl eframe::App for Loupe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_import_worker(ui.ctx());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.draw(ui);
                self.draw_import_dialog(ui.ctx());
            });
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
    let import_worker = ImportWorker::start().map_err(|e| e.to_string())?;
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
                import_worker,
                import_dialog: None,
                last_form: ImportForm::default(),
                detected_cards: Vec::new(),
                import_submit_error: None,
                import_hidden: false,
                import_result_opened: false,
                thumb_worker,
                thumb_grid: ThumbGrid::new(0, 0),
                thumb_generation: 0,
                thumb_textures: HashMap::new(),
                thumb_size: DEFAULT_THUMBNAIL_SIZE,
                grid_columns: 1,
                wheel_notches: 0.0,
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
