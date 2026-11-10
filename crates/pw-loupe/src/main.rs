//! RawStride (ADR 0012): the egui program beside the Tauri app, with the loupe (ADR 0009), folder
//! tabs and the import (ADR 0010) and the contact sheet (ADR 0011).
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
    Action, CellState, DEFAULT_THUMBNAIL_SIZE, DialogKey, DialogRequest, Field, Filter,
    GridKeyAction, ImportDialog, ImportDialogState, ImportEvent, ImportForm, ImportWorker,
    LABEL_CHOICES, LabelFilter, LoupeMove, MAX_THUMBNAIL_SIZE, MIN_THUMBNAIL_SIZE, Navigation,
    Options, READING_RATINGS_TEXT, RatingRefused, STAR_CONDITIONS, STAR_FILTERS, StarFilter,
    SubmitError, Tab, TabAction, Tabs, ThumbGrid, UNSUPPORTED_TEXT, ViewMode, WINDOW_TITLE,
    clamp_thumbnail_size, color_image, columns, dialog_key_action, done_lines,
    ejecting_progress_text, error_text, grid_key_action, indicator_text, iptc_progress_text,
    key_action, label_choice_text, label_filter_text, label_name, list_folder, loupe_key_action,
    marked_text, mode_label, more_lines, no_match_text, opens_import, parse_args, position_text,
    preview_line, preview_summary, progress_text, rating_key_action, request_cell,
    star_condition_text, star_filter_text, star_glyphs, status_line, step_allowed,
    step_thumbnail_size, tab_action, take_wheel_steps, visible_range, wheel_factor,
};
use pw_metadata::MetadataWorker;
use pw_metadata::Rating;
use pw_render::{Textures, view_px};
use pw_thumbcache::{ThumbWorker, ThumbnailRequest};
use pw_viewcore::native::NativeBackend;
use pw_viewcore::{Config, Status, ViewCore};

mod metadata;

const BACKGROUND: egui::Color32 = egui::Color32::from_gray(32);
const STATUS_HEIGHT: f32 = 24.0;
/// Width of the narrow star-condition list ("≥", "=", "≤", ADR 0014 §5.2, amendment 1).
const CONDITION_COMBO_WIDTH: f32 = 40.0;
const GRID_SPACING: f32 = 8.0;
const GRID_LABEL_HEIGHT: f32 = 18.0;
/// The outline and, faint, the fill of a marked cell (ADR 0014 §5.1: the label blue).
const MARKED_BLUE: egui::Color32 = egui::Color32::from_rgb(0x4c, 0x8d, 0xff);
const MARKED_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x0f, 0x1c, 0x33, 0x33);

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
    /// Whether the active tab's ratings have been read since the tab was loaded. A later read of
    /// the same folder (`switch_mode` reads again) does not clear it: the rating store keeps the
    /// shown values meanwhile, so the filter keeps working.
    ratings_read: bool,
    /// The value of `ratings_ready` when the active tab was last refiltered for it, so a change
    /// refilters once (ADR 0014 Q7).
    last_ratings_ready: bool,
    /// Status of the last rendered loupe frame (ADR 0009 §13).
    last_frame_status: Status,
}

impl Loupe {
    /// Whether the filter of the active tab can use the ratings (ADR 0014 Q7): they have been read,
    /// and the folder read did not fail. A read error counts as not ready, so an active filter
    /// shows nothing.
    fn ratings_ready(&self) -> bool {
        self.ratings_read && self.metadata.read_error().is_none()
    }

    /// `Tab::refilter` on the active tab with the shown ratings. Returns whether `visible` changed.
    fn refilter_active(&mut self, keep: Option<usize>) -> bool {
        let ready = self.ratings_ready();
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        self.tabs
            .active_mut()
            .is_some_and(|tab| tab.refilter(ready, &rating_of, keep))
    }

    /// `Tab::rated` on the active tab after an accepted rating key (ADR 0014 §2.2, amendment 1):
    /// the images that stopped matching leave at once. Returns whether `visible` changed.
    fn rated_active(&mut self) -> bool {
        let ready = self.ratings_ready();
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        self.tabs
            .active_mut()
            .is_some_and(|tab| tab.rated(ready, &rating_of))
    }

    /// A key move in the grid over the visible list (`Tab::grid_move`).
    fn grid_move(&mut self, nav: Navigation, extend: bool, visible_rows: usize) {
        let ready = self.ratings_ready();
        let columns = self.grid_columns;
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        if let Some(tab) = self.tabs.active_mut() {
            tab.grid_move(nav, extend, columns, visible_rows, ready, &rating_of);
        }
    }

    /// A click on the cell of file `index` (`Tab::click`).
    fn click(&mut self, index: usize, ctrl: bool, shift: bool) {
        let ready = self.ratings_ready();
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        if let Some(tab) = self.tabs.active_mut() {
            tab.click(index, ctrl, shift, ready, &rating_of);
        }
    }

    /// A step in the loupe over the visible list (`Tab::loupe_move`, ADR 0014 §4.4 way C). The core
    /// follows the tab: `set_files` when the visible list changed, so the views of images still in
    /// the list are kept, otherwise `go`. With nothing visible, back to the grid (§2.3).
    fn loupe_move(&mut self, to: LoupeMove) {
        let ready = self.ratings_ready();
        let cwd = std::env::current_dir().ok();
        let (changed, position) = {
            let rating_of = tab_rating(&self.metadata, cwd.as_deref());
            let Some(tab) = self.tabs.active_mut() else {
                return;
            };
            let before = tab.visible.clone();
            tab.loupe_move(to, ready, &rating_of);
            (tab.visible != before, tab.position())
        };
        match position {
            Some(position) if !changed => self.core.go(position),
            _ => {
                self.loupe_follows_visible();
            }
        }
    }

    /// A new filter for the active tab, as `setFilter` in `app.js`: refilter without keeping the
    /// current image. A shown loupe gets the new visible list (ADR 0014 §4.4), or returns to the grid
    /// when nothing is visible (§2.3).
    fn set_filter(&mut self, filter: Filter) {
        if self.tabs.active().is_none_or(|tab| tab.filter == filter) {
            return;
        }
        self.tabs.set_filter(filter);
        self.refilter_active(None);
        self.loupe_follows_visible();
    }

    /// Recomputes the active tab's visible list when `ratings_ready` has changed since the last
    /// time (ADR 0014 Q7): the list fills when the ratings are read, and empties under an active
    /// filter when the read fails.
    fn refilter_when_ratings_change(&mut self) {
        if !self.ratings_read
            && self.metadata.ratings_ready()
            && self.metadata.read_error().is_none()
        {
            self.ratings_read = true;
        }
        let ready = self.ratings_ready();
        if ready != self.last_ratings_ready {
            self.last_ratings_ready = ready;
            if self.refilter_active(None) {
                self.loupe_follows_visible();
            }
        }
    }

    /// Opens the view core on the visible images of the active tab and goes to the current one:
    /// opening the loupe from the grid and a tab load. A later change of the visible list goes
    /// through `loupe_follows_visible` instead. `files` are the tab's files resolved as the
    /// metadata cache keys them.
    fn open_loupe_core(&mut self, files: &[PathBuf]) {
        let Some(tab) = self.tabs.active() else {
            self.core.open_folder(Vec::new());
            return;
        };
        let paths: Vec<PathBuf> = tab
            .visible
            .iter()
            .filter_map(|&index| files.get(index).cloned())
            .collect();
        let position = tab.position().unwrap_or(0);
        self.core.open_folder(paths);
        self.core.go(position);
    }

    /// After the active tab's visible list changed while it shows the loupe (a filter change, the
    /// ratings becoming ready, a rating key, a step): `core.set_files` with the visible paths and the
    /// current position, so the views of images still in the list are kept and the next image
    /// comes from the prefetched views (ADR 0014 §4.4 way C). With nothing visible, back to the grid
    /// (§2.3). Returns the position the grid should scroll to when it returned to the grid.
    fn loupe_follows_visible(&mut self) -> Option<usize> {
        let (mode, position) = self.tabs.active().map(|tab| (tab.mode, tab.position()))?;
        if mode != ViewMode::Loupe {
            return None;
        }
        let Some(position) = position else {
            return self.switch_mode(ViewMode::Grid);
        };
        let tab = self.tabs.active()?;
        match resolve_paths(&tab.files) {
            Ok(files) => {
                let paths: Vec<PathBuf> = tab
                    .visible
                    .iter()
                    .filter_map(|&index| files.get(index).cloned())
                    .collect();
                self.core.set_files(paths, position);
            }
            Err(error) => self.message = Some(error),
        }
        None
    }

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
        let previous_tabs = self.tabs.clone();
        let active_cleared = form.eject && self.tabs.clear_inside(Path::new(form.source.trim()));
        if active_cleared {
            self.load_active_tab();
        }
        match self.import_worker.run(form) {
            Ok(id) => {
                if let Some(dialog) = &mut self.import_dialog {
                    dialog.sent(id);
                }
                self.import_result_opened = false;
                self.import_submit_error = None;
            }
            Err(error) => {
                self.tabs = previous_tabs;
                if active_cleared {
                    self.load_active_tab();
                }
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
            ImportDialogState::Ejecting { source } => Some(ejecting_progress_text(source)),
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
                    let refresh_cards = matches!(
                        &event,
                        ImportEvent::Done { done, .. } if done.eject.is_some()
                    );
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
                    if refresh_cards && let Err(error) = self.import_worker.cards() {
                        self.import_submit_error = Some(submit_error_text(error));
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
                    ui.checkbox(
                        &mut form.eject,
                        "Mata ut kortet när importen är klar (inget raderas från kortet)",
                    );

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
                        ImportDialogState::Running { .. }
                        | ImportDialogState::Ejecting { .. }
                        | ImportDialogState::Done { .. } => {}
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
                    ImportDialogState::Ejecting { source } => {
                        ui.add(egui::ProgressBar::new(0.0).text(ejecting_progress_text(source)));
                        if ui.button("Dölj").clicked() {
                            hide = true;
                        }
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
        let loaded = self
            .metadata
            .begin_load(tab.files, &mut self.metadata_worker);
        // A tab load refilters without keeping the current image, as a folder load in `app.js`;
        // under an active filter nothing is shown until the ratings are read (Q7).
        self.ratings_read =
            loaded.is_ok() && self.metadata.ratings_ready() && self.metadata.read_error().is_none();
        self.last_ratings_ready = self.ratings_ready();
        self.refilter_active(None);
        let files = match loaded {
            Ok(files) => files,
            Err(error) => {
                self.message = Some(error);
                return;
            }
        };
        if tab.mode == ViewMode::Loupe {
            self.open_loupe_core(&files);
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
    /// thumbnails and their cache survive a visit to the loupe (ADR 0011 §4.2). The loupe opens on
    /// the visible images (`open_folder`); with nothing visible it does not open. Returns
    /// the current image's position in the visible list, so the grid can scroll to it.
    fn switch_mode(&mut self, mode: ViewMode) -> Option<usize> {
        let tab = self.tabs.active().cloned()?;
        if mode == ViewMode::Loupe && tab.visible.is_empty() {
            return None;
        }
        self.tabs.set_mode(mode);
        match mode {
            ViewMode::Loupe => {
                self.thumb_worker.pause();
                match self
                    .metadata
                    .begin_load(tab.files, &mut self.metadata_worker)
                {
                    Ok(files) => self.open_loupe_core(&files),
                    Err(error) => self.message = Some(error),
                }
            }
            ViewMode::Grid => {
                self.core.open_folder(Vec::new());
                self.thumb_worker.resume();
            }
        }
        self.tabs.active().and_then(Tab::position)
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
        let mut clicked_cell = None;
        // The tab's files may be relative; the metadata cache keys by resolved path.
        let cwd = std::env::current_dir().ok();

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
                    // Layout, rows and scrolling use positions in `tab.visible`; cells, thumbnails
                    // and textures use file indexes (ADR 0014 §4.2).
                    let shown = tab.visible.len();
                    let rows = shown.div_ceil(column_count);
                    let content_height = rows as f32 * stride;
                    ui.set_min_size(egui::vec2(width, content_height));

                    let positions = visible_range(
                        shown,
                        column_count,
                        self.thumb_size,
                        GRID_SPACING as u32,
                        viewport.min.y,
                        viewport.height(),
                        1,
                    );
                    let visible_files = tab.visible.get(positions.clone()).unwrap_or(&[]);
                    self.thumb_worker
                        .set_visible(index_span(visible_files.iter().copied()));
                    let cell = request_cell(self.thumb_size, ctx.pixels_per_point());
                    for index in self.thumb_grid.to_request_indexes(visible_files, cell) {
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
                        shown,
                        column_count,
                        self.thumb_size,
                        GRID_SPACING as u32,
                        viewport.min.y,
                        viewport.height(),
                        3,
                    );
                    let keep_files = tab.visible.get(keep).unwrap_or(&[]);
                    for index in self.thumb_grid.forget_not_in(keep_files) {
                        self.thumb_textures.remove(&index);
                    }

                    let origin = ui.min_rect().min;
                    let painter = ui.painter().clone();
                    let mut clicked = None;
                    for (position, &index) in positions.clone().zip(visible_files) {
                        let Some(path) = tab.files.get(index) else {
                            continue;
                        };
                        let row = position / column_count;
                        let column = position % column_count;
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
                        let rating = shown_rating(&self.metadata, cwd.as_deref(), path);
                        let marked = tab.selection.is_marked(index);
                        if marked {
                            painter.rect_filled(cell_rect, 0.0, MARKED_FILL);
                        }
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
                        if rating.is_some_and(|rating| rating.stars == Some(-1)) {
                            painter.rect_filled(
                                image_rect,
                                0.0,
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 102),
                            );
                        }

                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        let label_rect = egui::Rect::from_min_max(
                            egui::pos2(cell_rect.min.x, cell_rect.max.y - GRID_LABEL_HEIGHT),
                            cell_rect.max,
                        );
                        let pill = label_pill(label_rect);
                        let label = rating.and_then(|rating| rating.label.as_deref());
                        let color = label.and_then(label_color);
                        if let Some(color) = color {
                            painter.rect_filled(pill, pill.height() * 0.5, color);
                        }
                        let text_color = if matches!(label, Some("Yellow" | "Green")) {
                            egui::Color32::from_gray(24)
                        } else {
                            egui::Color32::LIGHT_GRAY
                        };
                        let glyphs = star_glyphs(rating.and_then(|rating| rating.stars));
                        let glyph_width = paint_rating_glyphs(
                            &painter,
                            egui::pos2(pill.min.x + 4.0, label_rect.min.y),
                            label_rect.height(),
                            &glyphs,
                            text_color,
                        );
                        painter.with_clip_rect(label_rect).text(
                            egui::pos2(pill.min.x + 4.0 + glyph_width + 4.0, label_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(12.0),
                            text_color,
                        );

                        // A marked cell has a blue outline outside the cell; the current cell keeps
                        // its yellow outline inside, so a current marked cell shows both (§5.1).
                        if marked {
                            painter.rect_stroke(
                                cell_rect,
                                0.0,
                                egui::Stroke::new(2.0, MARKED_BLUE),
                                egui::StrokeKind::Outside,
                            );
                        }
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
                        if response.double_clicked() {
                            // The first click of the pair already made this image current.
                            open_loupe = true;
                        } else if response.clicked() {
                            let (ctrl, shift) = ui.input(|input| {
                                (
                                    input.modifiers.ctrl || input.modifiers.command,
                                    input.modifiers.shift,
                                )
                            });
                            clicked = Some((index, ctrl, shift));
                        }
                    }

                    if let Some(position) = scroll_to {
                        let row = position / column_count;
                        let column = position % column_count;
                        let target = egui::Rect::from_min_size(
                            egui::pos2(
                                origin.x + column as f32 * stride,
                                origin.y + row as f32 * stride,
                            ),
                            egui::vec2(cell_size, cell_size),
                        );
                        ui.scroll_to_rect(target, Some(egui::Align::Center));
                    }
                    if visible_files
                        .iter()
                        .any(|&index| matches!(self.thumb_grid.state(index), CellState::Requested))
                    {
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                    if let Some(click) = clicked {
                        clicked_cell = Some(click);
                    }
                });
        });
        if let Some((index, ctrl, shift)) = clicked_cell {
            self.click(index, ctrl, shift);
        }
        if open_loupe {
            self.switch_mode(ViewMode::Loupe);
        }
        // The status line shows the tab as it is after this frame's click.
        if let Some(active) = self.tabs.active() {
            tab = active.clone();
        }

        let painter = ui.painter().clone();
        if tab.visible.is_empty() {
            let text = if tab.files.is_empty() {
                self.message
                    .clone()
                    .unwrap_or_else(|| "Dra en mapp eller en bild hit".to_string())
            } else {
                self.nothing_shown_text(&tab)
            };
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
            let current_path = tab
                .position()
                .and_then(|_| tab.files.get(tab.current))
                .cloned();
            let mut line = if !self.show_info {
                String::new()
            } else if let (Some(position), Some(path)) = (tab.position(), &current_path) {
                // "3 / 12 (av 31) · 12 markerade · TN100423.ARW": the marking before the name
                // (owner decision 2026-11-10; ADR 0014 §5.3 lists the parts).
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let mut line = position_text(position, tab.visible.len(), tab.files.len());
                if let Some(marked) = marked_text(tab.selection.count()) {
                    line = format!("{line}  ·  {marked}");
                }
                format!("{line}  ·  {name}")
            } else if tab.files.is_empty() {
                "Ingen bild vald".to_string()
            } else {
                self.nothing_shown_text(&tab)
            };
            if self.show_info
                && let Some(message) = &self.message
            {
                line = format!("{line}  ·  {message}");
            }
            if self.show_info
                && let Some(notice) = self.metadata.notice(current_path.as_deref())
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

    /// Why a tab with files shows nothing (ADR 0014 §2.4, §5.3, Q7): the read error, the ratings
    /// still being read under an active filter, or no image matching the filter.
    fn nothing_shown_text(&self, tab: &Tab) -> String {
        if tab.filter.is_active() {
            if let Some(error) = self.metadata.read_error() {
                return format!("Metadatafel: {error}");
            }
            if !self.ratings_ready() {
                return READING_RATINGS_TEXT.to_string();
            }
        }
        no_match_text(tab.files.len())
    }

    fn tab_action(&mut self, action: TabAction) {
        match action {
            TabAction::Next => self.tabs.next(),
            TabAction::Previous => self.tabs.previous(),
            TabAction::Close => {
                self.tabs.close_active();
                self.load_active_tab();
                self.prune_metadata_to_open_tabs();
                return;
            }
        }
        self.load_active_tab();
    }

    fn prune_metadata_to_open_tabs(&mut self) {
        let current_dir = match std::env::current_dir() {
            Ok(current_dir) => current_dir,
            Err(error) => {
                self.message = Some(format!("Kan inte städa metadata: {error}"));
                return;
            }
        };
        let open_files: Vec<PathBuf> = self
            .tabs
            .tabs()
            .iter()
            .flat_map(|tab| &tab.files)
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    current_dir.join(path)
                }
            })
            .collect();
        self.metadata
            .prune(|path| open_files.iter().any(|open_path| open_path == path));
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
            // Steps go through the tab's visible list; the core follows (ADR 0014 §4.4).
            Action::Next => self.loupe_move(LoupeMove::Next),
            Action::Previous => self.loupe_move(LoupeMove::Previous),
            Action::First => self.loupe_move(LoupeMove::First),
            Action::Last => self.loupe_move(LoupeMove::Last),
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
        let failures = self.metadata.poll(&mut self.metadata_worker);
        if let Some(message) = failures.last() {
            // One line for every failure since the last rating key (ADR 0014 Q6).
            self.message = Some(
                self.metadata
                    .failure_text()
                    .unwrap_or_else(|| message.clone()),
            );
        }
        // A rolled-back write does not refilter (ADR 0014 §2.2; the next move drops the image); the
        // ratings becoming ready does.
        self.refilter_when_ratings_change();
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
        let key_actions: Vec<(Option<TabAction>, egui::Key, bool, bool, bool, bool)> =
            ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            repeat,
                            ..
                        } => {
                            let ctrl = modifiers.ctrl || modifiers.command;
                            let tab = tab_action(*key, ctrl, modifiers.shift, modifiers.alt);
                            Some((tab, *key, ctrl, modifiers.shift, modifiers.alt, *repeat))
                        }
                        _ => None,
                    })
                    .collect()
            });
        let titles = self.tabs.titles();
        let active = self.tabs.active_index();
        let mut selected = None;
        let mut open_import_clicked = false;
        let filter = self.tabs.active().map(|tab| tab.filter).unwrap_or_default();
        let mut new_filter = None;
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
                    // The filter of the active tab (ADR 0014 §5.2, amendment 1): condition, stars,
                    // label. The condition applies only to a number of stars; it keeps its value
                    // while disabled.
                    let counts_stars = matches!(filter.stars, StarFilter::Stars(_));
                    let condition = ui
                        .add_enabled_ui(counts_stars, |ui| {
                            filter_combo(
                                ui,
                                "star-condition",
                                filter.condition,
                                &STAR_CONDITIONS,
                                star_condition_text,
                                counts_stars,
                                Some(CONDITION_COMBO_WIDTH),
                            )
                        })
                        .inner;
                    if let Some(condition) = condition {
                        new_filter = Some(Filter {
                            condition,
                            ..filter
                        });
                    }
                    if let Some(stars) = filter_combo(
                        ui,
                        "star-filter",
                        filter.stars,
                        &STAR_FILTERS,
                        star_filter_text,
                        filter.stars != StarFilter::All,
                        None,
                    ) {
                        new_filter = Some(Filter { stars, ..filter });
                    }
                    if let Some(label) = label_combo(ui, "label-filter", filter.label) {
                        new_filter = Some(Filter { label, ..filter });
                    }
                });
            });
        if open_import_clicked {
            self.open_import();
        }
        let mut scroll_to = None;
        if let Some(filter) = new_filter {
            self.set_filter(filter);
            // No widget keeps the keyboard focus, so arrows and digits work at once (§4.5).
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
            scroll_to = self.tabs.active().and_then(Tab::position);
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
        let mut dialog_consumes_keys = self.import_dialog.is_some() && !self.import_hidden;
        let mut start_import = false;
        for (tab_action, key, ctrl, shift, alt, repeat) in key_actions {
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

            if !ctx.egui_wants_keyboard_input()
                && let Some(action) = rating_key_action(key, ctrl, shift, alt)
            {
                // The marked images in the grid, else the current one (ADR 0014 §2.6); nothing
                // when the current image is not shown. An accepted key removes the images that
                // stopped matching at once (§2.2, amendment 1); a refused key changes nothing.
                let paths: Vec<PathBuf> = self
                    .tabs
                    .active()
                    .filter(|tab| tab.position().is_some())
                    .map(|tab| {
                        tab.selection
                            .targets(&tab.visible, tab.current, tab.mode)
                            .into_iter()
                            .filter_map(|index| tab.files.get(index).cloned())
                            .collect()
                    })
                    .unwrap_or_default();
                if !paths.is_empty() {
                    match self.metadata.set_ratings(&paths, action) {
                        Ok(_) => {
                            if self.rated_active() {
                                let tab = self.tabs.active();
                                let position = tab.and_then(Tab::position);
                                match tab.map(|tab| tab.mode) {
                                    Some(ViewMode::Grid) => scroll_to = position,
                                    // The core gets the new visible list, so the next image
                                    // comes from the prefetched views (§4.4); nothing left to
                                    // show: back to the grid (§2.3).
                                    Some(ViewMode::Loupe) => {
                                        scroll_to = self.loupe_follows_visible();
                                    }
                                    None => {}
                                }
                            }
                        }
                        Err(RatingRefused::Unsupported) => {
                            self.message = Some(UNSUPPORTED_TEXT.to_owned());
                        }
                        Err(error) => self.message = Some(error.to_string()),
                    }
                }
                continue;
            }

            match self.tabs.active().map(|tab| tab.mode) {
                Some(ViewMode::Grid) => match grid_key_action(key, ctrl, shift, alt) {
                    Some(GridKeyAction::OpenLoupe) => {
                        self.switch_mode(ViewMode::Loupe);
                    }
                    Some(action @ (GridKeyAction::Navigate(nav) | GridKeyAction::Extend(nav))) => {
                        let extend = matches!(action, GridKeyAction::Extend(_));
                        let visible_rows = ((rect.height()
                            / (self.thumb_size as f32 + GRID_SPACING))
                            .floor() as usize)
                            .max(1);
                        self.grid_move(nav, extend, visible_rows);
                        scroll_to = self.tabs.active().and_then(Tab::position);
                    }
                    Some(GridKeyAction::SelectAll) => {
                        if let Some(tab) = self.tabs.active_mut() {
                            tab.selection.all(&tab.visible);
                        }
                    }
                    Some(GridKeyAction::ClearSelection) => {
                        if let Some(tab) = self.tabs.active_mut() {
                            tab.selection.clear();
                        }
                    }
                    None => {}
                },
                Some(ViewMode::Loupe) => {
                    if loupe_key_action(key, ctrl, alt).is_some() {
                        scroll_to = self.switch_mode(ViewMode::Grid);
                    } else if let Some(action) = key_action(key, ctrl, alt)
                        && step_allowed(action, repeat, &self.last_frame_status)
                    {
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
        self.last_frame_status = frame.status.clone();
        // The tab's current image comes from the tab, not from `frame.pos`: every step goes through
        // the tab and the core follows it (ADR 0014 §4.4). The status line counts the whole folder.
        let total = self.tabs.active().map_or(0, |tab| tab.files.len());
        let painter = ui.painter().clone();
        painter.rect_filled(full, 0.0, BACKGROUND);
        self.textures.paint(&painter, rect, ppp, &frame);
        if self.show_info
            && let Some(rating) = frame
                .path
                .as_deref()
                .and_then(|path| self.metadata.shown_rating(path))
            && (rating.stars.is_some() || rating.label.is_some())
        {
            paint_rating_badge(&painter, rect, rating);
        }
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
                    .unwrap_or_else(|| status_line(&frame, true, total));
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
                status_line(&frame, self.anchor_at_pointer, total)
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

/// The range from the smallest to the largest file index (+1), for the thumbnail worker's
/// priority (ADR 0014 §4.2); empty without indexes.
fn index_span(indexes: impl Iterator<Item = usize> + Clone) -> std::ops::Range<usize> {
    match (indexes.clone().min(), indexes.max()) {
        (Some(first), Some(last)) => first..last.saturating_add(1),
        _ => 0..0,
    }
}

/// The paths as the metadata cache keys them (`MetadataCache::begin_load`): relative paths are
/// taken from the current folder.
fn resolve_paths(files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot resolve image paths: {e}"))?;
    Ok(files.iter().map(|path| resolve_path(&cwd, path)).collect())
}

/// One path as `resolve_paths` resolves it: a relative path is taken from `cwd`, an absolute one
/// stays as it is.
fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    cwd.join(path)
}

/// The shown rating of a tab file, looked up by the resolved path the metadata cache keys by (a
/// folder given as a relative path on the command line has relative files). Without a current
/// folder (`cwd` is `None`) the path is looked up as it is.
fn shown_rating<'a>(
    metadata: &'a MetadataCache,
    cwd: Option<&Path>,
    path: &Path,
) -> Option<&'a Rating> {
    match cwd {
        Some(cwd) if !path.is_absolute() => metadata.shown_rating(&resolve_path(cwd, path)),
        _ => metadata.shown_rating(path),
    }
}

/// The `rating_of` of the filter (`Tab::refilter`, `rated`, `grid_move`, `click`): `shown_rating`
/// of each tab file.
fn tab_rating<'a>(
    metadata: &'a MetadataCache,
    cwd: Option<&'a Path>,
) -> impl Fn(&Path) -> Option<Rating> + 'a {
    move |path| shown_rating(metadata, cwd, path).cloned()
}

/// A filter drop-down list in the tab bar (ADR 0014 §5.2): an active list (not "Alla …") has a
/// yellow outline. Returns the chosen value, if one was picked this frame.
fn filter_combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    current: T,
    choices: &[T],
    text: impl Fn(T) -> &'static str,
    active: bool,
    width: Option<f32>,
) -> Option<T> {
    let mut chosen = current;
    let mut picked = false;
    ui.scope(|ui| {
        if active {
            let stroke = egui::Stroke::new(1.5, egui::Color32::YELLOW);
            let widgets = &mut ui.visuals_mut().widgets;
            widgets.inactive.bg_stroke = stroke;
            widgets.hovered.bg_stroke = stroke;
            widgets.active.bg_stroke = stroke;
            widgets.open.bg_stroke = stroke;
        }
        let mut combo = egui::ComboBox::from_id_salt(id).selected_text(text(current));
        if let Some(width) = width {
            combo = combo.width(width);
        }
        combo.show_ui(ui, |ui| {
            for &choice in choices {
                if ui
                    .selectable_value(&mut chosen, choice, text(choice))
                    .clicked()
                {
                    picked = true;
                }
            }
        });
    });
    picked.then_some(chosen)
}

/// The label drop-down list in the tab bar (ADR 0014 §5.2, amendment 2, Q10): "Alla etiketter"
/// clears the set, and one check box per label choice. The popup stays open while ticking and closes
/// on a click outside it. Yellow outline when the set is not empty. Returns the new set, if it
/// changed this frame.
fn label_combo(ui: &mut egui::Ui, id: &str, current: LabelFilter) -> Option<LabelFilter> {
    let mut chosen = current;
    ui.scope(|ui| {
        if !current.is_all() {
            let stroke = egui::Stroke::new(1.5, egui::Color32::YELLOW);
            let widgets = &mut ui.visuals_mut().widgets;
            widgets.inactive.bg_stroke = stroke;
            widgets.hovered.bg_stroke = stroke;
            widgets.active.bg_stroke = stroke;
            widgets.open.bg_stroke = stroke;
        }
        egui::ComboBox::from_id_salt(id)
            .selected_text(label_filter_text(&current))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                let all = LabelFilter::all();
                if ui
                    .selectable_label(chosen.is_all(), label_filter_text(&all))
                    .clicked()
                {
                    chosen = all;
                }
                for choice in LABEL_CHOICES {
                    let mut ticked = chosen.contains(choice);
                    if ui
                        .checkbox(&mut ticked, label_choice_text(choice))
                        .changed()
                    {
                        chosen.toggle(choice);
                    }
                }
            });
    });
    (chosen != current).then_some(chosen)
}

fn field_error(state: &ImportDialogState, field: Field) -> Option<&str> {
    match state {
        ImportDialogState::Editing {
            error: Some(error), ..
        } if error.field == field => Some(&error.message),
        _ => None,
    }
}

fn label_color(label: &str) -> Option<egui::Color32> {
    let (red, green, blue) = match label {
        "Red" => (0xe5, 0x48, 0x4d),
        "Yellow" => (0xf5, 0xc4, 0x00),
        "Green" => (0x3f, 0xb9, 0x50),
        "Blue" => (0x4c, 0x8d, 0xff),
        "Purple" => (0xa3, 0x71, 0xf7),
        _ => return None,
    };
    Some(egui::Color32::from_rgb(red, green, blue))
}

/// Horizontal inset of the label pill from the cell edges, in points.
const LABEL_PILL_INSET_X: f32 = 6.0;
/// Vertical inset of the label pill from the top and bottom of the row, in points.
const LABEL_PILL_INSET_Y: f32 = 2.0;

/// The rounded label bar inside the row under a thumbnail: inset from the cell edges so it does
/// not touch them. Its corner radius is half its height (a pill).
fn label_pill(row: egui::Rect) -> egui::Rect {
    row.shrink2(egui::vec2(LABEL_PILL_INSET_X, LABEL_PILL_INSET_Y))
}

fn paint_rating_glyphs(
    painter: &egui::Painter,
    origin: egui::Pos2,
    height: f32,
    glyphs: &str,
    color: egui::Color32,
) -> f32 {
    let mut x = origin.x;
    let center_y = origin.y + height * 0.5;
    for glyph in glyphs.chars() {
        match glyph {
            '★' => {
                let center = egui::pos2(x + 5.5, center_y);
                let radius = height.min(14.0) * 0.44;
                // A star is concave, so it is drawn as ten triangles from the centre (each convex)
                // instead of one convex polygon, which would fill the notches between the tips.
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(center, color);
                for point in 0..10u32 {
                    let angle =
                        -std::f32::consts::FRAC_PI_2 + point as f32 * std::f32::consts::PI / 5.0;
                    let point_radius = if point % 2 == 0 {
                        radius
                    } else {
                        radius * 0.43
                    };
                    mesh.colored_vertex(
                        egui::pos2(
                            center.x + angle.cos() * point_radius,
                            center.y + angle.sin() * point_radius,
                        ),
                        color,
                    );
                    mesh.add_triangle(0, 1 + point, 1 + (point + 1) % 10);
                }
                painter.add(egui::Shape::mesh(mesh));
                x += 11.0;
            }
            '•' => {
                painter.circle_filled(
                    egui::pos2(x + 5.5, center_y),
                    height.min(14.0) * 0.12,
                    color,
                );
                x += 11.0;
            }
            '✕' => {
                let center = egui::pos2(x + 6.0, center_y);
                let radius = height.min(14.0) * 0.34;
                let stroke = egui::Stroke::new(1.8, color);
                painter.line_segment(
                    [
                        egui::pos2(center.x - radius, center.y - radius),
                        egui::pos2(center.x + radius, center.y + radius),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(center.x - radius, center.y + radius),
                        egui::pos2(center.x + radius, center.y - radius),
                    ],
                    stroke,
                );
                x += 12.0;
            }
            _ => {}
        }
    }
    x - origin.x
}

fn paint_rating_badge(painter: &egui::Painter, image_rect: egui::Rect, rating: &Rating) {
    let glyphs = star_glyphs(rating.stars);
    let text_color = if matches!(rating.label.as_deref(), Some("Yellow" | "Green")) {
        egui::Color32::from_gray(24)
    } else {
        egui::Color32::WHITE
    };
    let label = rating.label.as_deref().map(|label| {
        painter.layout_no_wrap(
            label_name(label).to_owned(),
            egui::FontId::proportional(14.0),
            text_color,
        )
    });
    let label_width = label.as_ref().map_or(0.0, |galley| galley.rect.width());
    let glyph_width = glyphs.chars().count() as f32 * 11.0;
    let gap = if !glyphs.is_empty() && label.is_some() {
        6.0
    } else {
        0.0
    };
    let height = 24.0;
    let width = 8.0 + glyph_width + gap + label_width + 8.0;
    let badge = egui::Rect::from_min_size(
        egui::pos2(image_rect.min.x + 8.0, image_rect.max.y - height - 8.0),
        egui::vec2(width, height),
    );
    let color = rating
        .label
        .as_deref()
        .and_then(label_color)
        .unwrap_or_else(|| egui::Color32::from_black_alpha(190));
    painter.rect_filled(badge, height * 0.5, color);
    let glyph_width = paint_rating_glyphs(
        painter,
        egui::pos2(badge.min.x + 4.0, badge.min.y),
        height,
        &glyphs,
        text_color,
    );
    if let Some(galley) = label {
        let position = egui::pos2(
            badge.min.x + 4.0 + glyph_width + gap,
            badge.center().y - galley.rect.height() * 0.5,
        );
        painter.galley(position, galley, text_color);
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

    /// Waits up to 5 s for rating writes still on their way to disk, then lists the ones that
    /// were not saved (ADR 0013 Q6, R4).
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let unsaved = metadata::finish_pending_writes(
            &mut self.metadata,
            &mut self.metadata_worker,
            Instant::now,
            std::thread::sleep,
        );
        if !unsaved.is_empty() {
            let _ = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title(metadata::UNSAVED_TITLE)
                .set_description(metadata::unsaved_list_text(&unsaved.lines()))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        }
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
                ratings_read: false,
                last_ratings_ready: false,
                last_frame_status: Status::Empty,
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

#[cfg(test)]
mod tests {
    use super::{index_span, label_color, label_pill, resolve_path, resolve_paths};
    use egui::{Color32, Rect, pos2};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_relative_path_is_resolved_from_the_current_folder_and_an_absolute_one_is_kept() {
        let cwd = std::env::temp_dir().join("pw-loupe-cwd");
        assert_eq!(
            resolve_path(&cwd, Path::new("shoot/IMG_001.ARW")),
            cwd.join("shoot").join("IMG_001.ARW")
        );
        assert_eq!(
            resolve_path(&cwd, Path::new("IMG_001.ARW")),
            cwd.join("IMG_001.ARW")
        );
        let absolute = std::env::temp_dir().join("other").join("IMG_002.ARW");
        assert!(absolute.is_absolute());
        assert_eq!(resolve_path(&cwd, &absolute), absolute);

        // The same result as `resolve_paths`, which the view core and the metadata cache get.
        let files = [PathBuf::from("shoot/IMG_001.ARW"), absolute.clone()];
        let current_dir = match std::env::current_dir() {
            Ok(current_dir) => current_dir,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        let resolved = match resolve_paths(&files) {
            Ok(resolved) => resolved,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        assert_eq!(
            resolved,
            [resolve_path(&current_dir, &files[0]), absolute.clone()]
        );
        assert!(resolved.iter().all(|path| path.is_absolute()));
    }

    #[test]
    fn thumbnail_priority_span_runs_from_the_smallest_to_the_largest_file_index() {
        // The visible cells under a filter (ADR 0014 §4.2): file indexes with gaps.
        assert_eq!(index_span([0, 4, 8, 9].into_iter()), 0..10);
        assert_eq!(index_span([9, 2, 5].into_iter()), 2..10);
        assert_eq!(index_span([7].into_iter()), 7..8);
        assert_eq!(index_span(std::iter::empty()), 0..0);
        assert_eq!(index_span([usize::MAX].into_iter()), usize::MAX..usize::MAX);
    }

    #[test]
    fn label_pill_is_inset_six_points_sideways_and_two_vertically() {
        let row = Rect::from_min_max(pos2(100.0, 282.0), pos2(300.0, 300.0));

        let pill = label_pill(row);

        assert_eq!(
            pill,
            Rect::from_min_max(pos2(106.0, 284.0), pos2(294.0, 298.0))
        );
        assert!(row.contains_rect(pill));
    }

    #[test]
    fn rating_label_colours_match_the_accepted_palette() {
        for (label, expected) in [
            ("Red", Color32::from_rgb(0xe5, 0x48, 0x4d)),
            ("Yellow", Color32::from_rgb(0xf5, 0xc4, 0x00)),
            ("Green", Color32::from_rgb(0x3f, 0xb9, 0x50)),
            ("Blue", Color32::from_rgb(0x4c, 0x8d, 0xff)),
            ("Purple", Color32::from_rgb(0xa3, 0x71, 0xf7)),
        ] {
            assert_eq!(label_color(label), Some(expected), "{label}");
        }
        assert_eq!(label_color("unknown"), None);
    }
}
