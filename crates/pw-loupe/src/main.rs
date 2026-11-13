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
use metadata::{EditorEvent, MetadataCache};
use pw_imagecheck::RawImageCheck;
use pw_loupe::{
    Action, CellState, DEFAULT_THUMBNAIL_SIZE, DialogKey, DialogRequest, EditorField, EditorForm,
    EditorText, Field, Filter, GridKeyAction, ImportDialog, ImportDialogState, ImportEvent,
    ImportForm, ImportWorker, IptcField, LABEL_CHOICES, LabelFilter, LoupeMove, MAX_THUMBNAIL_SIZE,
    MIN_THUMBNAIL_SIZE, MonitorProfile, Navigation, Options, PanelText, READING_RATINGS_TEXT,
    RatingRefused, STAR_CONDITIONS, STAR_FILTERS, Settings, StarFilter, SubmitError, Tab,
    TabAction, Tabs, ThumbGrid, UNSUPPORTED_TEXT, ViewMode, WINDOW_TITLE, allowed_modes,
    clamp_thumbnail_size, clear_and_import_text, clear_cancel_text, clear_confirmation_text,
    clear_confirmation_title, color_image, columns, dialog_key_action, done_lines, editor_text,
    ejecting_progress_text, error_text, grid_key_action, indicator_text, info_panel,
    iptc_progress_text, key_action, label_choice_text, label_filter_text, label_name, list_folder,
    load_monitor_profile, load_settings, loupe_key_action, marked_text, mode_label, mode_text,
    more_lines, no_match_text, notice_text, opens_import, other_values, panel_text, parse_args,
    position_text, preview_line, preview_summary, primary_monitor_profile_path, progress_text,
    rating_key_action, request_cell, save_settings, settings_path, star_condition_text,
    star_filter_text, star_glyphs, status_line, step_allowed, step_thumbnail_size, tab_action,
    take_wheel_steps, visible_range, wheel_factor, window_title,
};
use pw_loupe::{
    COLLAPSED_WIDTH, ColumnState, Command, MENUS, MenuEntry, MenuItem, RatingKey, STATUS_TEXT_SIZE,
    ShellAction, ShellSettings, ShellText, StartWindow, THEMES, Theme, Where, WindowRect,
    apply_text_sizes, clamp_column_width, column_width_range, command_text, image_area, key_text,
    menu_text, palette, rect_on_a_screen, shell_key_action, shell_text, start_window, submenu_text,
    theme_text, visuals,
};
use pw_metadata::edit::EditOptions;
use pw_metadata::iim::Fallback;
use pw_metadata::journal::{Journal, Recovered};
use pw_metadata::read::MetadataRead;
use pw_metadata::template::Mode;
use pw_metadata::{MIN_EXIFTOOL_VERSION, MetadataWorker, Rating, ToolVersion, WriteSetup};
use pw_render::{Textures, view_px};
use pw_thumbcache::{ThumbWorker, ThumbnailRequest};
use pw_viewcore::native::NativeBackend;
use pw_viewcore::{Config, Status, ViewCore};

mod metadata;

/// The contact sheet's and the loupe's background: the same neutral dark grey in both themes
/// (ADR 0017 §2.5, U1-Q4).
const BACKGROUND: egui::Color32 = image_area::BACKGROUND;
/// Width of the narrow star-condition list ("≥", "=", "≤", ADR 0014 §5.2, amendment 1).
const CONDITION_COMBO_WIDTH: f32 = 40.0;
const GRID_SPACING: f32 = 8.0;
const GRID_LABEL_HEIGHT: f32 = 18.0;
/// The outline and, faint, the fill of a marked cell (ADR 0014 §5.1: the label blue, which is
/// also the dark theme's accent, ADR 0017 §2.5).
const MARKED_BLUE: egui::Color32 = image_area::MARKED;
const MARKED_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x0f, 0x1c, 0x33, 0x33);
/// How long the image check waits for the view core to close the RAW sessions of a file (T91).
const RELEASE_TIMEOUT: Duration = Duration::from_secs(5);
/// The title of the message about writes left unfinished at the last exit (T91).
const RECOVERY_TITLE: &str = "Metadataskrivning avbröts";
/// The first line of that message.
const RECOVERY_HEADING: &str =
    "RawStride avslutades mitt i en metadataskrivning. Kontrollera de här filerna:";
/// How often a frame is asked for while the metadata worker has not yet tried to start ExifTool,
/// so the message about it is not left waiting for the next input (T101).
const EXIFTOOL_CHECK_INTERVAL: Duration = Duration::from_millis(200);
/// The space between two rows of the metadata window.
const PANEL_ROW_SPACING: f32 = 6.0;
/// The size the metadata window opens with (D16, T107); the user can resize it.
const WINDOW_SIZE: [f32; 2] = [460.0, 640.0];
/// The space above and below the metadata window's buttons.
const WINDOW_BUTTON_SPACING: f32 = 4.0;
/// How dark the program behind the metadata window is drawn while the window holds it (D16).
const BACKDROP: egui::Color32 = egui::Color32::from_black_alpha(60);
/// How often a frame is asked for while the metadata window waits for the worker.
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The image the metadata window opens for (ADR 0007 I10n, D16, T107): the active tab's current
/// image when it is shown, as the metadata cache keys it (`cwd` resolves a relative path); in the
/// contact sheet only when at most one image is marked.
fn metadata_subject(tab: Option<&Tab>, cwd: Option<&Path>) -> Option<PathBuf> {
    let tab = tab?;
    if tab.mode == ViewMode::Grid && tab.selection.marked_in(&tab.visible).len() > 1 {
        return None;
    }
    let path = tab.position().and_then(|_| tab.files.get(tab.current))?;
    Some(match cwd {
        Some(cwd) => resolve_path(cwd, path),
        None => path.clone(),
    })
}

/// The status bar's content (ADR 0017 §2.1, T109): made while the central area is drawn, shown
/// by the bottom panel. A change asks for one more frame, so the bar is never left a frame behind.
#[derive(Clone, Debug, Default, PartialEq)]
struct StatusBar {
    line: String,
    /// The RAW/JPEG control's text in the loupe (G4: a key and a control).
    mode: Option<&'static str>,
}

/// egui's theme for one of RawStride's.
fn egui_theme(theme: Theme) -> egui::Theme {
    match theme {
        Theme::Dark => egui::Theme::Dark,
        Theme::Light => egui::Theme::Light,
    }
}

/// What the metadata window says above its buttons (T105, T107).
#[derive(Debug, PartialEq)]
enum EditorStatus {
    Idle,
    /// "Sparat." and the notice texts of the write.
    Saved(Vec<String>),
    /// A failed write, or a failed read after a write; the whole text.
    Failed(String),
}

/// The read the form was filled from and the form.
struct FormState {
    /// The warnings are checked against it, and the read-only sections show it.
    read: MetadataRead,
    form: EditorForm,
}

/// What the metadata window holds.
enum WindowBody {
    /// The read is not back yet.
    Loading,
    /// The read failed; holds the reason.
    ReadError(String),
    Form(Box<FormState>),
}

/// The metadata window (ADR 0007 I10n, D16, T107): one image's metadata seen and edited in one
/// floating window, locked to its image until OK or Avbryt. It replaces the info panel of T103 and
/// the editor window of T105.
struct MetadataWindow {
    /// The image, as the metadata cache keys it.
    path: PathBuf,
    file: String,
    body: WindowBody,
    /// The field errors of the last "OK".
    errors: Vec<(EditorField, String)>,
    status: EditorStatus,
    /// "Släng ändringarna?" is shown (Escape with edits).
    confirm_discard: bool,
    /// Escape was pressed this frame; the keys are read before the window is drawn.
    escape: bool,
    /// The window's write succeeded with notices: the read after it refills the form.
    expect_reload: bool,
}

impl MetadataWindow {
    fn new(path: PathBuf) -> Self {
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        Self {
            path,
            file,
            body: WindowBody::Loading,
            errors: Vec::new(),
            status: EditorStatus::Idle,
            confirm_discard: false,
            escape: false,
            expect_reload: false,
        }
    }

    /// Whether the form holds edits that are not saved (a field with an error counts).
    fn has_unsaved_edits(&self) -> bool {
        match &self.body {
            WindowBody::Form(state) => state.form.edits().map_or(true, |edits| !edits.is_empty()),
            WindowBody::Loading | WindowBody::ReadError(_) => false,
        }
    }

    /// Fills the form from `read`.
    fn fill(&mut self, read: MetadataRead) {
        self.body = WindowBody::Form(Box::new(FormState {
            form: EditorForm::from_read(&read),
            read,
        }));
        self.errors.clear();
    }

    /// A result of the window's read. The first one fills the form or shows the error. A later
    /// one (RawStride wrote to the image) refills the form only when it holds no edits and no
    /// write runs (`locked`); otherwise the form keeps the read its edits were made against.
    fn read_arrived(&mut self, result: Result<MetadataRead, String>, locked: bool) {
        let has_form = matches!(self.body, WindowBody::Form(_));
        match result {
            Ok(read) if !has_form || (!locked && !self.has_unsaved_edits()) => self.fill(read),
            Err(error) if !has_form => self.body = WindowBody::ReadError(error),
            Ok(_) | Err(_) => {}
        }
    }

    /// Applies what the metadata cache says about the window's write. True when the window is to
    /// close (a write without notices succeeded).
    fn apply(&mut self, event: EditorEvent) -> bool {
        match event {
            EditorEvent::Saved { path, notices } if path == self.path => {
                if notices.is_empty() {
                    return true;
                }
                self.status = EditorStatus::Saved(notices.iter().map(notice_text).collect());
                self.expect_reload = true;
            }
            EditorEvent::Failed { path, message } if path == self.path => {
                self.status = EditorStatus::Failed(message);
            }
            EditorEvent::Reloaded { path, result } if path == self.path && self.expect_reload => {
                self.expect_reload = false;
                match result {
                    Ok(read) => self.fill(read),
                    Err(error) => {
                        self.status =
                            EditorStatus::Failed(panel_text(&PanelText::ReadError(error)));
                    }
                }
            }
            EditorEvent::Saved { .. }
            | EditorEvent::Failed { .. }
            | EditorEvent::Reloaded { .. } => {}
        }
        false
    }
}

struct Loupe {
    core: ViewCore<NativeBackend>,
    metadata_worker: MetadataWorker,
    /// The result of the metadata worker's first ExifTool start has been looked at, and the
    /// message about it shown when needed (ADR 0007 §11, D9, T101): it is shown at most once.
    exiftool_checked: bool,
    /// The settings in use (ADR 0007 §5.2, §5.4, T106): read at start, changed by "Spara" in the
    /// settings window even when the file could not be saved.
    settings: Settings,
    /// The settings file; `None` without LOCALAPPDATA (nothing can be saved).
    settings_path: Option<PathBuf>,
    /// The settings window's choices while it is open.
    settings_window: Option<Settings>,
    /// New options not yet taken by the metadata worker (its queue was full).
    pending_options: Option<EditOptions>,
    metadata: MetadataCache,
    import_worker: ImportWorker,
    import_dialog: Option<ImportDialog>,
    last_form: ImportForm,
    /// The window "Rensa fält?" is open: the import was started with fields in Clear and waits
    /// for the user's answer (ADR 0007 §10, D14).
    import_clear_confirm: bool,
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
    /// The status bar's content of the last frame, and the one made this frame (T109).
    status_bar: StatusBar,
    next_status: StatusBar,
    /// A position the contact sheet scrolls to on the next frame: a menu command or the command
    /// bar changed the current image or the filter (T109).
    scroll_request: Option<usize>,
    /// The metadata window (M, D16, T107); at most one. While it is open it holds the program.
    metadata_window: Option<MetadataWindow>,
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
    /// The monitor profile, read once at start (ADR 0016 §4.1, Q4); not used for pixels yet.
    #[allow(dead_code)] // used by ADR 0016 C4
    monitor_profile: MonitorProfile,
}

impl Loupe {
    /// Until it has been done once: asks the metadata worker for the result of its first ExifTool
    /// start, and when ExifTool is missing or too old shows one message window about it (ADR 0007
    /// §11, D9, T101). The window is shown from its own thread, so the frame is not held up.
    fn check_exiftool(&mut self, ctx: &egui::Context) {
        if self.exiftool_checked {
            return;
        }
        let Some(result) = self.metadata_worker.exiftool_version() else {
            ctx.request_repaint_after(EXIFTOOL_CHECK_INTERVAL);
            return;
        };
        self.exiftool_checked = true;
        let Some((title, text)) = exiftool_message(&result) else {
            return;
        };
        let shown = std::thread::Builder::new()
            .name("pw-exiftool-message".into())
            .spawn(move || {
                let _ = rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title(title)
                    .set_description(text)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            });
        if let Err(error) = shown {
            eprintln!("pw-loupe: the ExifTool message could not be shown: {error}");
        }
    }

    /// Opens the settings window with the settings in use. Not while the metadata window holds
    /// the program (D16).
    fn open_settings_window(&mut self) {
        if self.metadata_window.is_none() && self.settings_window.is_none() {
            self.settings_window = Some(self.settings);
        }
    }

    /// "Spara" in the settings window (T106): the settings apply at once (ratings, the metadata
    /// window and the next import) and are saved to the file. When they cannot be saved, they
    /// still apply until RawStride closes and a message says so. The metadata window reads its
    /// image when it opens, so no read made with the old settings is kept (it cannot be open
    /// while the settings window is).
    fn save_settings_window(&mut self, chosen: Settings) {
        // The window was opened with the layout of that moment: only its theme is taken, so the
        // file gets the current window and columns (T109).
        let chosen = Settings {
            shell: ShellSettings {
                theme: chosen.shell.theme,
                ..self.settings.shell
            },
            ..chosen
        };
        self.settings = chosen;
        self.pending_options = Some(chosen.edit_options());
        self.send_pending_options();
        let saved = match &self.settings_path {
            Some(path) => save_settings(path, &chosen).map_err(|error| error.to_string()),
            None => Err(settings_text(SettingsText::NoFolder).to_string()),
        };
        if let Err(error) = saved {
            show_warning(
                settings_text(SettingsText::Title),
                settings_save_error_text(&error),
            );
        }
    }

    /// Sends new options to the metadata worker (`set_options`, T98); kept for the next frame
    /// while its queue is full. Returns whether options still wait.
    fn send_pending_options(&mut self) -> bool {
        let Some(options) = self.pending_options else {
            return false;
        };
        match self.metadata_worker.set_options(options) {
            Ok(()) => self.pending_options = None,
            Err(pw_metadata::SubmitError::QueueFull) => {}
            Err(pw_metadata::SubmitError::Stopped) => {
                eprintln!("pw-loupe: the metadata worker has stopped; the settings were not sent");
                self.pending_options = None;
            }
        }
        self.pending_options.is_some()
    }

    /// The settings window "Inställningar" (ADR 0007 §5.2, §5.4/Q10, T106). Every text comes
    /// from `settings_text` or `fallback_text`.
    fn draw_settings_window(&mut self, ctx: &egui::Context) {
        let Some(mut chosen) = self.settings_window else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        egui::Window::new(settings_text(SettingsText::Title))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.checkbox(
                    &mut chosen.create_iim,
                    settings_text(SettingsText::CreateIim),
                );
                ui.label(egui::RichText::new(settings_text(SettingsText::CreateIimNote)).small());
                ui.add_space(PANEL_ROW_SPACING);
                ui.label(settings_text(SettingsText::Encoding));
                egui::ComboBox::from_id_salt("fallback-encoding")
                    .selected_text(fallback_text(chosen.fallback))
                    .show_ui(ui, |ui| {
                        for text in FALLBACK_CHOICES.map(fallback_text) {
                            let selected = text == fallback_text(chosen.fallback);
                            if ui.selectable_label(selected, text).clicked()
                                && let Some(fallback) = fallback_from_text(text)
                            {
                                chosen.fallback = fallback;
                            }
                        }
                    });
                ui.add_space(PANEL_ROW_SPACING);
                ui.label(shell_text(ShellText::ThemeLabel));
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(theme_text(chosen.shell.theme))
                    .show_ui(ui, |ui| {
                        for theme in THEMES {
                            ui.selectable_value(&mut chosen.shell.theme, theme, theme_text(theme));
                        }
                    });
                ui.add_space(PANEL_ROW_SPACING);
                ui.horizontal(|ui| {
                    save = ui.button(settings_text(SettingsText::Save)).clicked();
                    cancel = ui.button(settings_text(SettingsText::Cancel)).clicked();
                });
            });
        if save {
            self.settings_window = None;
            self.save_settings_window(chosen);
            ctx.set_theme(egui_theme(self.settings.shell.theme));
        } else if cancel {
            self.settings_window = None;
        } else {
            self.settings_window = Some(chosen);
        }
    }

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
        self.import_clear_confirm = false;
    }

    /// The start button or Enter (D14): with fields in Clear the window "Rensa fält?" asks first,
    /// once for this import; otherwise the import starts at once.
    fn request_import_start(&mut self) {
        let Some(dialog) = self.import_dialog.as_ref() else {
            return;
        };
        if !dialog.can_start() {
            return;
        }
        if dialog.form().iptc.cleared_fields().is_empty() {
            self.start_import();
        } else {
            self.import_clear_confirm = true;
        }
    }

    fn start_import(&mut self) {
        self.import_clear_confirm = false;
        let request = self.import_dialog.as_mut().and_then(ImportDialog::start);
        let Some(DialogRequest::Run { form }) = request else {
            return;
        };
        let previous_tabs = self.tabs.clone();
        let active_cleared = form.eject && self.tabs.clear_inside(Path::new(form.source.trim()));
        if active_cleared {
            self.load_active_tab();
        }
        match self.import_worker.run(form, self.settings.edit_options()) {
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
        // While "Rensa fält?" is open the form cannot change, so the import starts with the
        // fields the window lists (D14).
        let confirming = self.import_clear_confirm && can_start;
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
                ui.add_enabled_ui(!running && !confirming, |ui| {
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
                        ui.colored_label(ui.visuals().error_fg_color, message);
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
                        ui.colored_label(ui.visuals().error_fg_color, message);
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
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }

                    ui.add_space(8.0);
                    ui.label("Mappmall");
                    ui.text_edit_singleline(&mut form.folder);
                    if let Some(message) = field_error(&state, Field::Folder) {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }

                    ui.add_space(8.0);
                    ui.label("Filnamnsmall");
                    ui.text_edit_singleline(&mut form.name);
                    if let Some(message) = field_error(&state, Field::Name) {
                        ui.colored_label(ui.visuals().error_fg_color, message);
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
                        .color(ui.visuals().weak_text_color()),
                    );

                    ui.collapsing("IPTC-mall", |ui| {
                        // Each field with a drop-down list of its modes (D13); the text cannot
                        // be edited in Behåll or Rensa, which do not use it.
                        for field in IptcField::ALL {
                            ui.horizontal(|ui| {
                                ui.label(field.label());
                                let uses_text =
                                    !matches!(form.iptc.modes.get(field), Mode::Keep | Mode::Clear);
                                ui.add_enabled_ui(uses_text, |ui| {
                                    let mut edit =
                                        egui::TextEdit::singleline(form.iptc.text_mut(field));
                                    if field == IptcField::Copyright {
                                        edit = edit.hint_text("© 2026 Namn");
                                    }
                                    ui.add(edit);
                                });
                                let mode = form.iptc.modes.get_mut(field);
                                egui::ComboBox::from_id_salt(("import-iptc-mode", field))
                                    .selected_text(mode_text(*mode))
                                    .show_ui(ui, |ui| {
                                        for &choice in allowed_modes(field) {
                                            ui.selectable_value(mode, choice, mode_text(choice));
                                        }
                                    });
                            });
                        }
                        ui.label(
                            egui::RichText::new(
                                "Skrivs till .xmp-sidofilen för RAW och in i DNG/JPG/TIF. \
                                 Tomma fält lämnas orörda.",
                            )
                            .small()
                            .color(ui.visuals().weak_text_color()),
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
                                ui.colored_label(ui.visuals().error_fg_color, line.text);
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
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    can_start && !confirming,
                                    egui::Button::new("Starta import"),
                                )
                                .clicked()
                            {
                                start = true;
                            }
                            if ui
                                .add_enabled(can_close && !confirming, egui::Button::new("Stäng"))
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
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });

        // D14: asked once per started import, before anything is copied.
        let mut confirm_start = false;
        let mut confirm_cancel = false;
        if confirming {
            let cleared = form.iptc.cleared_fields();
            egui::Window::new(clear_confirmation_title())
                .collapsible(false)
                .resizable(false)
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(clear_confirmation_text(&cleared));
                    ui.horizontal(|ui| {
                        if ui.button(clear_and_import_text()).clicked() {
                            confirm_start = true;
                        }
                        if ui.button(clear_cancel_text()).clicked() {
                            confirm_cancel = true;
                        }
                    });
                });
        }

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
            self.request_import_start();
        }
        if confirm_start {
            self.start_import();
        }
        // "Avbryt", or the import can no longer start (for example a new preview): back to the
        // dialog, nothing started.
        if confirm_cancel || (self.import_clear_confirm && !can_start) {
            self.import_clear_confirm = false;
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
                        painter.rect_filled(image_rect, 0.0, image_area::CELL);
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
                                    image_area::TEXT,
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
                        let text_color = if color.is_some() {
                            LABEL_TEXT
                        } else {
                            image_area::TEXT
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

                        // One frame colour for the current cell and the marked cells (owner,
                        // 2026-11-13): a marked cell has its outline outside the cell, the current
                        // cell inside, so a current marked cell shows both.
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
                                egui::Stroke::new(2.0, MARKED_BLUE),
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
                image_area::TEXT,
            );
        }
        let import_status = self.hidden_import_status();
        if self.show_info || import_status.is_some() {
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
            // The thumbnail size moved to the command bar (T109).
            self.next_status = StatusBar { line, mode: None };
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
            Action::OpenMetadata => self.open_metadata_window(),
        }
    }

    /// Opens the metadata window for the current image (ADR 0007 I10n, D16, T107) and asks for
    /// its metadata at once. Nothing happens with several images marked in the contact sheet, no
    /// image shown, a kind without metadata support, or a window already open.
    fn open_metadata_window(&mut self) {
        // Not while the settings window is open: the window's read must use the saved settings.
        if self.metadata_window.is_some() || self.settings_window.is_some() {
            return;
        }
        let cwd = std::env::current_dir().ok();
        let Some(path) = metadata_subject(self.tabs.active(), cwd.as_deref()) else {
            return;
        };
        if self.metadata.open_window(&path, &mut self.metadata_worker) {
            self.metadata_window = Some(MetadataWindow::new(path));
        }
    }

    /// Closes the metadata window without writing.
    fn close_metadata_window(&mut self) {
        self.metadata_window = None;
        self.metadata.close_window();
    }

    /// The metadata window (ADR 0007 I10n, D16, T107): modal, movable and resizable, its contents
    /// in a scroll area and the buttons below it. Every text comes from `editor_text`, the form
    /// (labels, notes, field errors, warnings), `notice_text`, `panel_text` or `info_panel.rs`, or
    /// is data (the file name, values).
    fn draw_metadata_window(&mut self, ctx: &egui::Context) {
        // Taken every frame, so nothing is left over from a window that was closed meanwhile.
        let events = self.metadata.take_editor_events();
        let read = self.metadata.take_window_read();
        let Some(mut window) = self.metadata_window.take() else {
            return;
        };
        for event in events {
            if window.apply(event) {
                self.close_metadata_window();
                return;
            }
        }
        let saving = self.metadata.editor_saving();
        let locked = saving || self.metadata.editor_reloading();
        if let Some(result) = read {
            window.read_arrived(result, locked);
        }
        if locked || self.metadata.window_waiting() {
            ctx.request_repaint_after(WINDOW_POLL_INTERVAL);
        }
        let escape = std::mem::take(&mut window.escape);
        let asking = window.confirm_discard;
        let has_form = matches!(window.body, WindowBody::Form(_));
        let create_iim = self.settings.create_iim;
        let warnings = match &window.body {
            WindowBody::Form(state) => state.form.warnings(&state.read, create_iim),
            WindowBody::Loading | WindowBody::ReadError(_) => Vec::new(),
        };
        let form_before = match &window.body {
            WindowBody::Form(state) => Some(state.form.clone()),
            WindowBody::Loading | WindowBody::ReadError(_) => None,
        };
        let mut choice = None;
        let mut ok = false;
        let mut cancel = false;
        let mut discard = false;
        let mut keep_editing = false;

        // Modal (D16): the program behind takes no clicks, drags or wheel while the window is open.
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("metadata-window-backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let response = ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
                ui.painter().rect_filled(response.rect, 0.0, BACKDROP);
            });

        egui::Window::new(editor_text(&EditorText::WindowTitle {
            file: window.file.clone(),
        }))
        .id(egui::Id::unique("metadata-window"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(true)
        .default_size(WINDOW_SIZE)
        .show(ctx, |ui| {
            let error_color = ui.visuals().error_fg_color;
            egui::Panel::bottom("metadata-window-buttons")
                .resizable(false)
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    ui.add_space(WINDOW_BUTTON_SPACING);
                    if saving {
                        ui.label(editor_text(&EditorText::Saving));
                    } else {
                        match &window.status {
                            EditorStatus::Idle => {}
                            EditorStatus::Saved(notices) => {
                                ui.label(editor_text(&EditorText::Saved));
                                for notice in notices {
                                    ui.label(notice);
                                }
                            }
                            EditorStatus::Failed(message) => {
                                ui.label(egui::RichText::new(message).color(error_color));
                            }
                        }
                    }
                    if asking {
                        ui.label(editor_text(&EditorText::DiscardQuestion));
                        ui.horizontal(|ui| {
                            discard = ui.button(editor_text(&EditorText::Discard)).clicked();
                            keep_editing =
                                ui.button(editor_text(&EditorText::KeepEditing)).clicked();
                        });
                    } else {
                        ui.horizontal(|ui| {
                            // Until the read is back, or when it failed, only "Avbryt".
                            if has_form {
                                ok = ui
                                    .add_enabled(
                                        !locked,
                                        egui::Button::new(editor_text(&EditorText::Ok)),
                                    )
                                    .clicked();
                            }
                            cancel = ui
                                .add_enabled(
                                    !saving,
                                    egui::Button::new(editor_text(&EditorText::Cancel)),
                                )
                                .clicked();
                        });
                    }
                    ui.add_space(WINDOW_BUTTON_SPACING);
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("metadata-window-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| match &mut window.body {
                            WindowBody::Loading => {
                                ui.label(panel_text(&PanelText::Loading));
                            }
                            WindowBody::ReadError(error) => {
                                ui.label(
                                    egui::RichText::new(panel_text(&PanelText::ReadError(
                                        error.clone(),
                                    )))
                                    .color(error_color),
                                );
                            }
                            WindowBody::Form(state) => {
                                choice = draw_window_form(
                                    ui,
                                    state,
                                    &window.errors,
                                    &warnings,
                                    !locked && !asking,
                                );
                            }
                        });
                });
        });

        if let (Some((conflict_index, value_index)), WindowBody::Form(state)) =
            (choice, &mut window.body)
        {
            state.form.choose(conflict_index, value_index);
        }
        let form_after = match &window.body {
            WindowBody::Form(state) => Some(&state.form),
            WindowBody::Loading | WindowBody::ReadError(_) => None,
        };
        if form_after != form_before.as_ref() {
            // "Sparat." or an error no longer describes the form.
            window.status = EditorStatus::Idle;
        }
        if discard {
            self.close_metadata_window();
            return;
        }
        if keep_editing {
            window.confirm_discard = false;
        }
        // Escape is "Avbryt", but with edits it asks first; while the write runs it does nothing.
        // Escape while asking is "Fortsätt redigera".
        if escape && !saving {
            if window.confirm_discard {
                window.confirm_discard = false;
            } else if window.has_unsaved_edits() {
                window.confirm_discard = true;
            } else {
                self.close_metadata_window();
                return;
            }
        }
        if cancel {
            self.close_metadata_window();
            return;
        }
        if ok {
            let edits = match &window.body {
                WindowBody::Form(state) => Some(state.form.edits()),
                WindowBody::Loading | WindowBody::ReadError(_) => None,
            };
            match edits {
                None => {}
                // Field errors: nothing is written, the errors are shown under their fields.
                Some(Err(errors)) => window.errors = errors,
                Some(Ok(edits)) if edits.is_empty() => {
                    self.close_metadata_window();
                    return;
                }
                Some(Ok(edits)) => {
                    window.errors.clear();
                    window.status = EditorStatus::Idle;
                    match self
                        .metadata
                        .save_edits(&window.path, edits, &mut self.metadata_worker)
                    {
                        Ok(()) => ctx.request_repaint(),
                        Err(message) => window.status = EditorStatus::Failed(message),
                    }
                }
            }
        }
        self.metadata_window = Some(window);
    }

    /// Whether a dialog holds the program: the metadata window (modal, D16), the settings window,
    /// or a shown import dialog.
    fn dialog_open(&self) -> bool {
        self.metadata_window.is_some()
            || self.settings_window.is_some()
            || (self.import_dialog.is_some() && !self.import_hidden)
    }

    /// Keeps the window's state for `settings.txt` (ADR 0017 §2.5, T109): full screen, maximized,
    /// and the last normal position and size. Nothing is taken while the window is minimized.
    fn remember_window(&mut self, ctx: &egui::Context) {
        let (outer, inner, maximized, fullscreen, scale, minimized) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                viewport.outer_rect,
                viewport.inner_rect,
                viewport.maximized,
                viewport.fullscreen,
                viewport.native_pixels_per_point,
                viewport.minimized,
            )
        });
        if minimized == Some(true) {
            return;
        }
        let shell = &mut self.settings.shell;
        if let Some(fullscreen) = fullscreen {
            shell.fullscreen = fullscreen;
        }
        if shell.fullscreen {
            return;
        }
        if let Some(maximized) = maximized {
            shell.window_maximized = maximized;
        }
        if shell.window_maximized {
            return;
        }
        if let (Some(outer), Some(inner), Some(scale)) = (outer, inner, scale) {
            shell.window = Some(WindowRect {
                x: outer.min.x.round() as i32,
                y: outer.min.y.round() as i32,
                width: inner.width().round().max(0.0) as u32,
                height: inner.height().round().max(0.0) as u32,
                scale,
            });
        }
    }

    /// Ctrl+O, F11 and the hide key (ADR 0017 §2.4, T109), and their menu items.
    fn shell_action(&mut self, ctx: &egui::Context, action: ShellAction) {
        match action {
            ShellAction::OpenFolder => self.open_folder_dialog(),
            ShellAction::ToggleFullscreen => {
                let now = ctx
                    .input(|input| input.viewport().fullscreen)
                    .unwrap_or(self.settings.shell.fullscreen);
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!now));
            }
            ShellAction::ToggleColumns => {
                self.settings.shell.columns_hidden = !self.settings.shell.columns_hidden;
            }
        }
    }

    /// "Öppna mapp" (Ctrl+O, ADR 0017 §2.1, T109): the folder dialog, starting in the active
    /// tab's folder; the chosen folder opens as a dropped folder does (GR12, GR13). Not while a
    /// dialog holds the program.
    fn open_folder_dialog(&mut self) {
        if self.dialog_open() {
            return;
        }
        let start = self.tabs.active().and_then(|tab| tab.folder.clone());
        if let Some(folder) =
            pick_folder(shell_text(ShellText::OpenFolderTitle), "", start.as_ref())
        {
            self.open(&folder, true);
        }
    }

    /// Visa ▸ Tema (ADR 0017 §2.5, T109): applied at once and saved.
    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.shell.theme = theme;
        ctx.set_theme(egui_theme(theme));
        if let Some(path) = &self.settings_path
            && let Err(error) = save_settings(path, &self.settings)
        {
            eprintln!("pw-loupe: the theme could not be saved: {error}");
        }
    }

    /// Visa ▸ Vänster kolumn / Höger kolumn: opens or collapses the column; a hidden pair is
    /// shown again with that column open.
    fn toggle_column(&mut self, left: bool) {
        let shell = &mut self.settings.shell;
        let column = if left {
            &mut shell.left_column
        } else {
            &mut shell.right_column
        };
        if shell.columns_hidden {
            shell.columns_hidden = false;
            *column = ColumnState::Open;
        } else {
            *column = column.toggled();
        }
    }

    /// Ctrl+A in the contact sheet: marks every visible image (ADR 0014 §4.5).
    fn select_all(&mut self) {
        if let Some(tab) = self.tabs.active_mut() {
            tab.selection.all(&tab.visible);
        }
    }

    /// Escape in the contact sheet: clears the marking.
    fn clear_selection(&mut self) {
        if let Some(tab) = self.tabs.active_mut() {
            tab.selection.clear();
        }
    }

    /// A rating or label key, or its menu item: the marked images in the grid, else the current
    /// one (ADR 0014 §2.6); nothing when the current image is not shown. An accepted key removes
    /// the images that stopped matching at once (§2.2, amendment 1); a refused key changes
    /// nothing. Returns `Some` with the position the contact sheet scrolls to when the visible
    /// list changed.
    fn rate(&mut self, action: RatingKey) -> Option<Option<usize>> {
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
        if paths.is_empty() {
            return None;
        }
        match self.metadata.set_ratings(&paths, action) {
            Ok(_) => {
                if self.rated_active() {
                    let tab = self.tabs.active();
                    let position = tab.and_then(Tab::position);
                    match tab.map(|tab| tab.mode) {
                        Some(ViewMode::Grid) => return Some(position),
                        // The core gets the new visible list, so the next image comes from the
                        // prefetched views (§4.4); nothing left to show: back to the grid (§2.3).
                        Some(ViewMode::Loupe) => return Some(self.loupe_follows_visible()),
                        None => {}
                    }
                }
            }
            Err(RatingRefused::Unsupported) => {
                self.message = Some(UNSUPPORTED_TEXT.to_owned());
            }
            Err(error) => self.message = Some(error.to_string()),
        }
        None
    }

    /// A new filter from the command bar or Visa ▸ Filter (ADR 0014 §5.2): no widget keeps the
    /// keyboard focus, so arrows and digits work at once (§4.5), and the contact sheet scrolls to
    /// the current image.
    fn apply_filter(&mut self, ctx: &egui::Context, filter: Filter) {
        self.set_filter(filter);
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        self.scroll_request = self.tabs.active().and_then(Tab::position);
    }

    /// Whether a menu item can be chosen now (a disabled item is shown, not hidden).
    fn command_enabled(&self, command: Command) -> bool {
        let tab = self.tabs.active();
        let view = tab.map(|tab| tab.mode);
        match command.acts_in() {
            Where::Grid if view != Some(ViewMode::Grid) => return false,
            Where::Loupe if view != Some(ViewMode::Loupe) => return false,
            Where::Both | Where::Grid | Where::Loupe => {}
        }
        match command {
            Command::CloseTab
            | Command::NextTab
            | Command::PreviousTab
            | Command::SelectAll
            | Command::ClearSelection => tab.is_some(),
            Command::Metadata => {
                let cwd = std::env::current_dir().ok();
                metadata_subject(tab, cwd.as_deref()).is_some()
            }
            Command::Stars(_) | Command::Reject | Command::Label(_) | Command::ClearLabel => {
                tab.is_some_and(|tab| tab.position().is_some())
            }
            Command::ToggleView => tab.is_some_and(|tab| !tab.visible.is_empty()),
            _ => true,
        }
    }

    /// Whether a check or radio item is on; `None` for a plain item.
    fn command_checked(&self, command: Command) -> Option<bool> {
        let shell = &self.settings.shell;
        match command {
            Command::ToggleAnchor => Some(self.anchor_at_pointer),
            Command::ToggleInfo => Some(self.show_info),
            Command::ToggleColumns => Some(!shell.columns_hidden),
            Command::LeftColumn => {
                Some(!shell.columns_hidden && shell.left_column == ColumnState::Open)
            }
            Command::RightColumn => {
                Some(!shell.columns_hidden && shell.right_column == ColumnState::Open)
            }
            Command::Fullscreen => Some(shell.fullscreen),
            Command::Theme(theme) => Some(shell.theme == theme),
            _ => None,
        }
    }

    /// A command of the menu bar, the command bar or the status bar: what its key does, through
    /// the same function (ADR 0017 §2.1, T109). A zoom from the menu uses the image's centre.
    fn run_command(&mut self, ctx: &egui::Context, command: Command) {
        if !self.command_enabled(command) || self.dialog_open() {
            return;
        }
        match command {
            Command::OpenFolder => self.shell_action(ctx, ShellAction::OpenFolder),
            Command::Import => self.open_import(),
            Command::CloseTab => self.tab_action(TabAction::Close),
            Command::NextTab => self.tab_action(TabAction::Next),
            Command::PreviousTab => self.tab_action(TabAction::Previous),
            Command::Settings => self.open_settings_window(),
            Command::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Command::SelectAll => self.select_all(),
            Command::ClearSelection => self.clear_selection(),
            Command::Metadata => self.act(Action::OpenMetadata, None),
            Command::Stars(stars) => self.rate_from_menu(RatingKey::Stars(stars)),
            Command::Reject => self.rate_from_menu(RatingKey::Reject),
            Command::Label(label) => self.rate_from_menu(RatingKey::Label(label)),
            Command::ClearLabel => self.rate_from_menu(RatingKey::ClearLabel),
            Command::ToggleMode => self.act(Action::ToggleMode, None),
            Command::ToggleView => match self.tabs.active().map(|tab| tab.mode) {
                Some(ViewMode::Grid) => {
                    self.switch_mode(ViewMode::Loupe);
                }
                Some(ViewMode::Loupe) => self.scroll_request = self.switch_mode(ViewMode::Grid),
                None => {}
            },
            Command::Fit => self.act(Action::Fit, None),
            Command::ToggleZoom => self.act(Action::ToggleZoom, None),
            Command::ZoomIn => self.act(Action::ZoomStep(1), None),
            Command::ZoomOut => self.act(Action::ZoomStep(-1), None),
            Command::ToggleAnchor => self.act(Action::ToggleAnchor, None),
            Command::ToggleInfo => self.act(Action::ToggleInfo, None),
            Command::ToggleColumns => self.shell_action(ctx, ShellAction::ToggleColumns),
            Command::LeftColumn => self.toggle_column(true),
            Command::RightColumn => self.toggle_column(false),
            Command::Fullscreen => self.shell_action(ctx, ShellAction::ToggleFullscreen),
            Command::Theme(theme) => self.set_theme(ctx, theme),
        }
    }

    /// A rating item of the Bild menu: `rate`, with the contact sheet scrolled as after its key.
    fn rate_from_menu(&mut self, action: RatingKey) {
        if let Some(Some(position)) = self.rate(action) {
            self.scroll_request = Some(position);
        }
    }

    /// The menu bar (ADR 0017 §2.1, U1-Q1): the table `MENUS`, each item with its key.
    /// A choice in Visa ▸ Filter is returned in `filter_choice`.
    fn draw_menu_bar(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<Command>,
        filter_choice: &mut Option<Filter>,
    ) {
        egui::MenuBar::new().ui(ui, |ui| {
            for menu in MENUS {
                ui.menu_button(menu_text(menu.kind), |ui| {
                    for entry in menu.entries {
                        match entry {
                            MenuEntry::Item(item) => self.menu_item(ui, *item, commands),
                            MenuEntry::Separator => {
                                ui.separator();
                            }
                            MenuEntry::Submenu(kind, items) => {
                                ui.menu_button(submenu_text(*kind), |ui| {
                                    for item in *items {
                                        self.menu_item(ui, *item, commands);
                                    }
                                });
                            }
                            MenuEntry::Filter => {
                                let has_tab = self.tabs.active().is_some();
                                ui.add_enabled_ui(has_tab, |ui| {
                                    ui.menu_button(shell_text(ShellText::FilterMenu), |ui| {
                                        if let Some(filter) = self.filter_menu(ui) {
                                            *filter_choice = Some(filter);
                                        }
                                    });
                                });
                            }
                        }
                    }
                });
            }
        });
    }

    /// One menu item: its text, its key on the right, checked when it is a check or radio item
    /// that is on, disabled where it does nothing.
    fn menu_item(&self, ui: &mut egui::Ui, item: MenuItem, commands: &mut Vec<Command>) {
        let mut button = egui::Button::new(command_text(item.command));
        if let Some(key) = item.key {
            button = button.shortcut_text(key_text(key));
        }
        if let Some(on) = self.command_checked(item.command) {
            button = button.selected(on);
        }
        if ui
            .add_enabled(self.command_enabled(item.command), button)
            .clicked()
        {
            commands.push(item.command);
        }
    }

    /// Visa ▸ Filter: "Visa alla", then the command bar's three lists as radio items (the
    /// condition only for a number of stars) and the label check boxes (ADR 0014 §5.2).
    fn filter_menu(&self, ui: &mut egui::Ui) -> Option<Filter> {
        let filter = self.tabs.active().map(|tab| tab.filter)?;
        let mut chosen = None;
        if ui.button(shell_text(ShellText::ShowAll)).clicked() {
            chosen = Some(Filter {
                stars: StarFilter::All,
                label: LabelFilter::all(),
                ..filter
            });
        }
        ui.separator();
        let counts_stars = matches!(filter.stars, StarFilter::Stars(_));
        ui.add_enabled_ui(counts_stars, |ui| {
            for condition in STAR_CONDITIONS {
                if ui
                    .radio(
                        filter.condition == condition,
                        star_condition_text(condition),
                    )
                    .clicked()
                {
                    chosen = Some(Filter {
                        condition,
                        ..filter
                    });
                }
            }
        });
        ui.separator();
        for stars in STAR_FILTERS {
            if ui
                .radio(filter.stars == stars, star_filter_text(stars))
                .clicked()
            {
                chosen = Some(Filter { stars, ..filter });
            }
        }
        ui.separator();
        let all = LabelFilter::all();
        if ui
            .radio(filter.label.is_all(), label_filter_text(&all))
            .clicked()
        {
            chosen = Some(Filter {
                label: all,
                ..filter
            });
        }
        for choice in LABEL_CHOICES {
            let mut ticked = filter.label.contains(choice);
            if ui
                .checkbox(&mut ticked, label_choice_text(choice))
                .changed()
            {
                let mut label = filter.label;
                label.toggle(choice);
                chosen = Some(Filter { label, ..filter });
            }
        }
        chosen
    }

    /// The command bar (ADR 0017 §2.1, T109): "Importera…", "Öppna mapp…", the active tab's
    /// filter (ADR 0014 §5.2, amendment 1) and the thumbnail size.
    fn draw_command_bar(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        let accent = palette(self.settings.shell.theme).accent;
        let tab_filter = self.tabs.active().map(|tab| tab.filter);
        let mut new_filter = None;
        ui.horizontal_wrapped(|ui| {
            for command in [Command::Import, Command::OpenFolder] {
                if ui.button(command_text(command)).clicked() {
                    commands.push(command);
                }
            }
            ui.separator();
            ui.add_enabled_ui(tab_filter.is_some(), |ui| {
                let filter = tab_filter.unwrap_or_default();
                // The condition applies only to a number of stars; it keeps its value while
                // disabled.
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
                            accent,
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
                    accent,
                ) {
                    new_filter = Some(Filter { stars, ..filter });
                }
                if let Some(label) = label_combo(ui, "label-filter", filter.label, accent) {
                    new_filter = Some(Filter { label, ..filter });
                }
            });
            ui.separator();
            let slider = ui.add(
                egui::Slider::new(
                    &mut self.thumb_size,
                    MIN_THUMBNAIL_SIZE..=MAX_THUMBNAIL_SIZE,
                )
                .text("Storlek")
                .show_value(true),
            );
            // The slider does not keep the keyboard: arrows move in the contact sheet (§4.5).
            if slider.has_focus() && !slider.dragged() {
                slider.surrender_focus();
            }
            self.thumb_size = clamp_thumbnail_size(self.thumb_size);
        });
        if let Some(filter) = new_filter {
            self.apply_filter(ui.ctx(), filter);
        }
    }

    /// The status bar (ADR 0017 §2.1, T109): the RAW/JPEG control in the loupe (G4) and the
    /// line of the last frame, wrapped when it is long. The control is disabled while a dialog
    /// holds the program; the text stays readable.
    fn draw_status_bar(&self, ui: &mut egui::Ui, blocked: bool, commands: &mut Vec<Command>) {
        ui.horizontal_wrapped(|ui| {
            if let Some(mode) = self.status_bar.mode
                && ui.add_enabled(!blocked, egui::Button::new(mode)).clicked()
            {
                commands.push(Command::ToggleMode);
            }
            ui.label(egui::RichText::new(&self.status_bar.line).size(STATUS_TEXT_SIZE));
        });
    }

    /// The side columns (ADR 0017 §2.1, U1-Q3, T109): each resizable between the minimum and 40 %
    /// of the window, collapsible to a strip with its open button, and both hidden with the hide
    /// key. Their content comes in U2, U4 and U5; until then a heading and one line each, and the
    /// hidden import's status under "Uppgifter".
    fn draw_columns(&mut self, ui: &mut egui::Ui) {
        let shell = self.settings.shell;
        if shell.columns_hidden {
            return;
        }
        let window_width = ui.ctx().content_rect().width();
        let (min, max) = column_width_range(window_width);
        let import_status = self.hidden_import_status();
        match shell.left_column {
            ColumnState::Open => {
                let shown = egui::Panel::left("left-column")
                    .resizable(true)
                    .default_size(clamp_column_width(
                        shell.left_column_width as f32,
                        window_width,
                    ))
                    .size_range(min..=max)
                    .show(ui, |ui| {
                        let collapse =
                            column_header(ui, ShellText::Library, ShellText::CollapseLeft);
                        column_note(ui, shell_text(ShellText::LibraryNote));
                        ui.add_space(PANEL_ROW_SPACING * 2.0);
                        ui.heading(shell_text(ShellText::Tasks));
                        match &import_status {
                            Some(status) => {
                                ui.label(status.as_str());
                            }
                            None => column_note(ui, shell_text(ShellText::NoTasks)),
                        }
                        collapse
                    });
                self.settings.shell.left_column_width =
                    shown.response.rect.width().round().max(0.0) as u32;
                if shown.inner {
                    self.settings.shell.left_column = ColumnState::Collapsed;
                }
            }
            ColumnState::Collapsed => {
                if collapsed_column(ui, "left-column-collapsed", true) {
                    self.settings.shell.left_column = ColumnState::Open;
                }
            }
        }
        match shell.right_column {
            ColumnState::Open => {
                let shown = egui::Panel::right("right-column")
                    .resizable(true)
                    .default_size(clamp_column_width(
                        shell.right_column_width as f32,
                        window_width,
                    ))
                    .size_range(min..=max)
                    .show(ui, |ui| {
                        let collapse =
                            column_header(ui, ShellText::Inspector, ShellText::CollapseRight);
                        column_note(ui, shell_text(ShellText::InspectorNote));
                        collapse
                    });
                self.settings.shell.right_column_width =
                    shown.response.rect.width().round().max(0.0) as u32;
                if shown.inner {
                    self.settings.shell.right_column = ColumnState::Collapsed;
                }
            }
            ColumnState::Collapsed => {
                if collapsed_column(ui, "right-column-collapsed", false) {
                    self.settings.shell.right_column = ColumnState::Open;
                }
            }
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
        // Not while the metadata window holds the program (D16): its image must not change.
        if let Some(path) = dropped
            && self.metadata_window.is_none()
        {
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
        self.next_status = StatusBar::default();
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
        // The command bar or a menu command may have asked for a scroll (T109).
        let mut scroll_to = self.scroll_request.take();
        if let Some(index) = selected
            && index != self.tabs.active_index()
            && self.tabs.activate(index)
        {
            self.load_active_tab();
        }
        let full = ui.available_rect_before_wrap();
        let import_status = self.hidden_import_status();
        // The status line is its own panel now (T109): the image area is the whole central area.
        let rect = full;
        let ppp = ctx.pixels_per_point();
        let key_anchor = self.anchor(ctx.input(|input| input.pointer.hover_pos()), rect, ppp);
        let mut dialog_consumes_keys = self.import_dialog.is_some() && !self.import_hidden;
        let mut start_import = false;
        for (tab_action, key, ctrl, shift, alt, repeat) in key_actions {
            // The metadata window is modal (D16, T107): no key reaches the loupe, the contact
            // sheet or the tabs, and keys typed into its fields reach nothing else. Escape is its
            // "Avbryt"; Enter does not press OK.
            if let Some(window) = &mut self.metadata_window {
                if key == egui::Key::Escape && !ctrl && !alt {
                    window.escape = true;
                }
                continue;
            }
            if dialog_consumes_keys {
                match dialog_key_action(key, ctrl, alt) {
                    // Escape in "Rensa fält?" is its "Avbryt"; Enter does not answer it.
                    Some(DialogKey::Close) if self.import_clear_confirm => {
                        self.import_clear_confirm = false;
                    }
                    Some(DialogKey::Start) if self.import_clear_confirm => {}
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
            // Ctrl+O, F11 and the hide key (ADR 0017 §2.4, T109); not while a text field has the
            // keyboard.
            if !ctx.egui_wants_keyboard_input()
                && let Some(action) = shell_key_action(key, ctrl, shift, alt)
            {
                self.shell_action(&ctx, action);
                continue;
            }

            if !ctx.egui_wants_keyboard_input()
                && let Some(action) = rating_key_action(key, ctrl, shift, alt)
            {
                if let Some(position) = self.rate(action) {
                    scroll_to = position;
                }
                continue;
            }

            // M opens the metadata window of the current image in the loupe and the contact sheet
            // (D16); with several images marked, or a kind without metadata support, nothing
            // happens.
            if !ctx.egui_wants_keyboard_input()
                && key_action(key, ctrl, alt) == Some(Action::OpenMetadata)
            {
                self.act(Action::OpenMetadata, key_anchor);
                ctx.request_repaint();
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
                    Some(GridKeyAction::SelectAll) => self.select_all(),
                    Some(GridKeyAction::ClearSelection) => self.clear_selection(),
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
            self.request_import_start();
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
            if self.metadata_window.is_none()
                && pointer.is_some_and(|pointer| rect.contains(pointer))
            {
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
                image_area::TEXT,
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
            // The display-mode control (G4: a key and a control) sits in the status bar.
            self.next_status = StatusBar {
                line,
                mode: self.show_info.then(|| mode_label(frame.mode)),
            };
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

/// The properties of the editor's fields, left out of the window's other metadata (D16, T107):
/// every field's property except `Iptc4xmpCore:CreatorContactInfo` (the contact fields), whose
/// address fields are not editor fields, so the structure stays in the list.
fn window_editor_properties() -> Vec<&'static str> {
    EditorField::ALL
        .iter()
        .filter(|field| field.struct_field().is_none())
        .map(|field| field.property())
        .collect()
}

/// The contents of the metadata window's form (D16, T107), top to bottom: the 22 fields, the open
/// conflicts with a choice each and the warnings (editable when `enabled`); then read-only, in
/// small text, the other metadata, the conflicts a rule decided and the notices, each only when it
/// has rows. Labels and values wrap at the window's width. Returns the conflict value chosen.
fn draw_window_form(
    ui: &mut egui::Ui,
    state: &mut FormState,
    errors: &[(EditorField, String)],
    warnings: &[String],
    enabled: bool,
) -> Option<(usize, usize)> {
    let warning = ui.visuals().warn_fg_color;
    let error_color = ui.visuals().error_fg_color;
    let grey = ui.visuals().weak_text_color();
    let mut choice = None;
    ui.add_enabled_ui(enabled, |ui| {
        for field in EditorField::ALL {
            ui.label(egui::RichText::new(field.label()).strong());
            if let Some(note) = state.form.note(field) {
                ui.label(egui::RichText::new(note).color(grey).small());
            }
            if let Some(text) = state.form.text_mut(field) {
                let edit = if field.multiline() {
                    egui::TextEdit::multiline(text).desired_rows(3)
                } else {
                    egui::TextEdit::singleline(text)
                };
                ui.add(edit.desired_width(f32::INFINITY));
            }
            for (_, error) in errors.iter().filter(|(f, _)| *f == field) {
                ui.label(egui::RichText::new(error).color(error_color));
            }
            ui.add_space(PANEL_ROW_SPACING);
        }

        let conflicts = state.form.open_conflicts();
        if !conflicts.is_empty() {
            ui.separator();
            ui.label(
                egui::RichText::new(editor_text(&EditorText::ConflictsHeading))
                    .heading()
                    .color(warning),
            );
            for (conflict_index, conflict) in conflicts.iter().enumerate() {
                ui.label(egui::RichText::new(&conflict.label).strong());
                for (value_index, value) in conflict.values.iter().enumerate() {
                    let text = editor_text(&EditorText::ConflictChoice {
                        value: value.value.clone(),
                        origin: value.origin.clone(),
                    });
                    if ui
                        .radio(conflict.chosen == Some(value_index), text)
                        .clicked()
                    {
                        choice = Some((conflict_index, value_index));
                    }
                }
                ui.add_space(PANEL_ROW_SPACING);
            }
        }

        if !warnings.is_empty() {
            ui.separator();
            ui.label(
                egui::RichText::new(editor_text(&EditorText::WarningsHeading))
                    .heading()
                    .color(warning),
            );
            for text in warnings {
                ui.label(egui::RichText::new(text).color(warning));
            }
        }
    });

    // Read-only, from the read the form was filled from.
    let others = other_values(&state.read, &window_editor_properties());
    if !others.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(editor_text(&EditorText::OtherValuesHeading)).strong());
        for row in &others {
            ui.label(egui::RichText::new(&row.label).strong().small());
            ui.label(egui::RichText::new(&row.value).small());
            ui.label(egui::RichText::new(&row.origin).color(grey).small());
            ui.add_space(PANEL_ROW_SPACING);
        }
    }

    let panel = info_panel(&state.read);
    // Open conflicts are the choices above; their rule text is not shown.
    let decided: Vec<_> = panel
        .conflicts
        .iter()
        .filter(|conflict| !conflict.open)
        .collect();
    if !decided.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(panel_text(&PanelText::Conflicts)).strong());
        for conflict in decided {
            ui.label(egui::RichText::new(&conflict.label).strong().small());
            for value in &conflict.values {
                ui.label(egui::RichText::new(&value.value).small());
                ui.label(egui::RichText::new(&value.origin).color(grey).small());
            }
            ui.label(egui::RichText::new(&conflict.rule).color(warning).small());
            ui.add_space(PANEL_ROW_SPACING);
        }
    }

    if !panel.notices.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(panel_text(&PanelText::Notices)).strong());
        for notice in &panel.notices {
            ui.label(egui::RichText::new(notice).small());
            ui.add_space(PANEL_ROW_SPACING);
        }
    }
    choice
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
#[allow(clippy::too_many_arguments)] // one list of the command bar, with its look
fn filter_combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    current: T,
    choices: &[T],
    text: impl Fn(T) -> &'static str,
    active: bool,
    width: Option<f32>,
    accent: egui::Color32,
) -> Option<T> {
    let mut chosen = current;
    let mut picked = false;
    ui.scope(|ui| {
        if active {
            // An active list is outlined in the theme's accent (ADR 0017 §2.5: one accent for
            // active controls; the yellow of ADR 0014 had too little contrast in the light theme).
            let stroke = egui::Stroke::new(1.5, accent);
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

/// The label drop-down list in the command bar (ADR 0014 §5.2, amendment 2, Q10): "Alla
/// etiketter" clears the set, and one check box per label choice. The popup stays open while
/// ticking and closes on a click outside it. Outlined in the accent when the set is not empty.
/// Returns the new set, if it changed this frame.
fn label_combo(
    ui: &mut egui::Ui,
    id: &str,
    current: LabelFilter,
    accent: egui::Color32,
) -> Option<LabelFilter> {
    let mut chosen = current;
    ui.scope(|ui| {
        if !current.is_all() {
            let stroke = egui::Stroke::new(1.5, accent);
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

/// The text on a label pill and a labelled rating badge: dark on every label colour, at least
/// 4.5:1 (WCAG AA; owner, 2026-11-13). Light text stays below 4.5:1 on red, blue and purple.
const LABEL_TEXT: egui::Color32 = egui::Color32::from_gray(24);

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
    let text_color = if rating.label.as_deref().and_then(label_color).is_some() {
        LABEL_TEXT
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

/// A side column's heading with its collapse button on the right; returns whether the button was
/// clicked (T109).
fn column_header(ui: &mut egui::Ui, heading: ShellText, collapse: ShellText) -> bool {
    ui.horizontal(|ui| {
        ui.heading(shell_text(heading));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.small_button(shell_text(collapse))
                .on_hover_text(shell_text(ShellText::CollapseHover))
                .clicked()
        })
        .inner
    })
    .inner
}

/// One line of weak text in a side column, wrapped to the column's width.
fn column_note(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).weak()).wrap());
}

/// A collapsed side column: a thin strip with only its open button; returns whether it was
/// clicked (T109).
fn collapsed_column(ui: &mut egui::Ui, id: &str, left: bool) -> bool {
    let panel = if left {
        egui::Panel::left(id.to_owned())
    } else {
        egui::Panel::right(id.to_owned())
    };
    let glyph = if left {
        ShellText::ExpandLeft
    } else {
        ShellText::ExpandRight
    };
    panel
        .resizable(false)
        .exact_size(COLLAPSED_WIDTH)
        .show(ui, |ui| {
            ui.small_button(shell_text(glyph))
                .on_hover_text(shell_text(ShellText::ExpandHover))
                .clicked()
        })
        .inner
}

fn submit_error_text(error: SubmitError) -> String {
    match error {
        SubmitError::QueueFull => "Importarbetarens kö är full.".to_string(),
        SubmitError::Stopped => "Importarbetaren har stoppat.".to_string(),
    }
}

impl eframe::App for Loupe {
    /// The main window (ADR 0017 §2.1, T109): the menu bar, the command bar, the status bar, the
    /// side columns and, in the middle, the folder tabs with the contact sheet or the loupe.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_import_worker(&ctx);
        self.check_exiftool(&ctx);
        if self.send_pending_options() {
            ctx.request_repaint_after(WINDOW_POLL_INTERVAL);
        }
        self.remember_window(&ctx);
        // The bars take no clicks while a dialog holds the program: the metadata window is modal
        // (D16), and the import and settings windows keep the program as their keys do.
        let blocked = self.dialog_open();
        let mut commands = Vec::new();
        let mut filter_choice = None;
        egui::Panel::top("menu-bar").show(ui, |ui| {
            ui.add_enabled_ui(!blocked, |ui| {
                self.draw_menu_bar(ui, &mut commands, &mut filter_choice);
            });
        });
        egui::Panel::top("command-bar").show(ui, |ui| {
            ui.add_enabled_ui(!blocked, |ui| self.draw_command_bar(ui, &mut commands));
        });
        egui::Panel::bottom("status-bar").show(ui, |ui| {
            self.draw_status_bar(ui, blocked, &mut commands);
        });
        if let Some(filter) = filter_choice
            && !blocked
        {
            self.apply_filter(&ctx, filter);
        }
        self.draw_columns(ui);
        let fill = ui.visuals().panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(fill))
            .show(ui, |ui| self.draw(ui));
        self.draw_import_dialog(&ctx);
        self.draw_settings_window(&ctx);
        self.draw_metadata_window(&ctx);
        if !commands.is_empty() {
            for command in commands {
                self.run_command(&ctx, command);
            }
            ctx.request_repaint();
        }
        if self.next_status != self.status_bar {
            self.status_bar = self.next_status.clone();
            ctx.request_repaint();
        }
    }

    /// Waits up to 5 s for rating writes and the editor's write still on their way to disk, then
    /// lists the ones that were not saved (ADR 0013 Q6, R4, T105).
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
                .set_title(editor_text(&EditorText::UnsavedAtExitTitle))
                .set_description(metadata::unsaved_list_text(&unsaved.lines()))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        }
        // The window, full screen and the columns as they are now (ADR 0017 §2.5, T109); a
        // failure is only logged, no dialog at exit.
        if let Some(path) = &self.settings_path
            && let Err(error) = save_settings(path, &self.settings)
        {
            eprintln!("pw-loupe: the settings could not be saved at exit: {error}");
        }
    }
}

/// The write journal folder (ADR 0007 §9.5): `%LOCALAPPDATA%\RawStride\journal`.
fn journal_folder() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|data| PathBuf::from(data).join("RawStride").join("journal"))
}

/// Opens the write journal and resolves the writes left open at the last exit (§9.5), before any
/// worker starts. Files that need the user are shown in a message box. `None` when the journal
/// cannot be opened: RawStride still starts, but writes nothing (T91).
fn open_journal() -> Option<Journal> {
    let folder = journal_folder()?;
    let journal = match Journal::open(&folder) {
        Ok(journal) => journal,
        Err(error) => {
            eprintln!(
                "pw-loupe: the write journal {} could not be opened: {error}",
                folder.display()
            );
            return None;
        }
    };
    match journal.recover() {
        Ok(recovered) => {
            if let Some(text) = recovery_text(&recovered, |path| path.exists()) {
                let _ = rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title(RECOVERY_TITLE)
                    .set_description(text)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            }
        }
        Err(error) => {
            eprintln!(
                "pw-loupe: the write journal {} could not be read: {error}",
                folder.display()
            );
        }
    }
    Some(journal)
}

/// The message about the writes [`Journal::recover`] could not resolve (`NeedsUser`, `Failed`,
/// `Unreadable`): one line per entry with the original and, when the backup file exists
/// (`exists`), the backup. `Cleaned` and `Restored` are not shown; `None` when nothing is left.
fn recovery_text(recovered: &[Recovered], exists: impl Fn(&Path) -> bool) -> Option<String> {
    let lines: Vec<String> = recovered
        .iter()
        .filter_map(|item| match item {
            Recovered::Cleaned { .. } | Recovered::Restored { .. } => None,
            Recovered::NeedsUser { entry } | Recovered::Failed { entry, .. } => {
                Some(if exists(&entry.backup) {
                    format!(
                        "{} (säkerhetskopia: {})",
                        entry.original.display(),
                        entry.backup.display()
                    )
                } else {
                    entry.original.display().to_string()
                })
            }
            Recovered::Unreadable { file, .. } => Some(file.display().to_string()),
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(format!("{RECOVERY_HEADING}\n{}", lines.join("\n")))
}

/// The title and text of the message about the metadata worker's first ExifTool start (ADR 0007
/// §11, D9, T101): `None` when the version is at least [`MIN_EXIFTOOL_VERSION`].
fn exiftool_message(result: &Result<ToolVersion, String>) -> Option<(&'static str, String)> {
    match result {
        Err(text) => Some((
            "ExifTool saknas",
            format!(
                "ExifTool kunde inte startas: {text}. Betyg och metadata kan inte läsas eller \
                 sparas."
            ),
        )),
        Ok(version) if *version < MIN_EXIFTOOL_VERSION => Some((
            "ExifTool är för gammal",
            format!(
                "ExifTool {version} är för gammal. RawStride behöver version \
                 {MIN_EXIFTOOL_VERSION} eller senare. Betyg och metadata sparas inte förrän \
                 ExifTool har uppdaterats."
            ),
        )),
        Ok(_) => None,
    }
}

/// A fixed text of the settings window (ADR 0007 §5.2, §5.4/Q10, T106).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsText {
    /// The button in the top bar and the window's title.
    Title,
    /// The check box of §5.2.
    CreateIim,
    /// The small text under the check box.
    CreateIimNote,
    /// The label of the encoding drop-down (§5.4, Q10).
    Encoding,
    Save,
    Cancel,
    /// The reason the file could not be saved when there is no settings folder (no LOCALAPPDATA).
    NoFolder,
}

/// The text of `text` in the settings window.
fn settings_text(text: SettingsText) -> &'static str {
    match text {
        SettingsText::Title => "Inställningar",
        SettingsText::CreateIim => "Skapa IPTC-IIM för kompatibilitet med äldre system",
        SettingsText::CreateIimNote => "Gäller JPEG, TIFF och DNG som saknar IIM. Av som standard.",
        SettingsText::Encoding => "Teckenkodning för äldre IPTC- och Exif-text",
        SettingsText::Save => "Spara",
        SettingsText::Cancel => "Avbryt",
        SettingsText::NoFolder => "mappen för inställningar är okänd (LOCALAPPDATA saknas)",
    }
}

/// The message shown when the settings file could not be saved; the settings still apply until
/// RawStride closes (T106).
fn settings_save_error_text(error: &str) -> String {
    format!("Inställningarna kunde inte sparas: {error}")
}

/// The fallback encodings of the settings window's drop-down, in its order.
const FALLBACK_CHOICES: [Fallback; 2] = [Fallback::Windows1252, Fallback::Latin1];

/// The text of a fallback encoding in the settings window's drop-down.
fn fallback_text(fallback: Fallback) -> &'static str {
    match fallback {
        Fallback::Windows1252 => "Windows-1252 (västeuropeisk)",
        Fallback::Latin1 => "ISO 8859-1 (Latin-1)",
    }
}

/// The fallback encoding whose drop-down text is `text`; `None` for any other text.
fn fallback_from_text(text: &str) -> Option<Fallback> {
    FALLBACK_CHOICES
        .into_iter()
        .find(|fallback| fallback_text(*fallback) == text)
}

/// The settings at start (T106): read from `path` with each problem written to the log; the
/// defaults without a path.
fn start_settings(path: Option<&Path>) -> Settings {
    let Some(path) = path else {
        eprintln!("pw-loupe: LOCALAPPDATA is not set; the default settings are used");
        return Settings::default();
    };
    let (settings, problems) = load_settings(path);
    for problem in problems {
        eprintln!("pw-loupe: {problem}");
    }
    settings
}

/// Shows a warning box from its own thread, so the frame is not held up.
fn show_warning(title: &'static str, text: String) {
    let shown = std::thread::Builder::new()
        .name("pw-warning-message".into())
        .spawn(move || {
            let _ = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title(title)
                .set_description(text)
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        });
    if let Err(error) = shown {
        eprintln!("pw-loupe: a warning could not be shown: {error}");
    }
}

/// The image check of the safe write (I9d): it closes the view core's RAW sessions of the file
/// first (T89), waiting at most [`RELEASE_TIMEOUT`].
fn image_check(core: &ViewCore<NativeBackend>) -> RawImageCheck {
    let releaser = core.releaser();
    RawImageCheck::new(move |path| {
        let _ = releaser.release(path, RELEASE_TIMEOUT);
    })
}

fn run(options: Options) -> Result<(), String> {
    let journal = open_journal();
    let settings_path = settings_path();
    let settings = start_settings(settings_path.as_deref());
    let config = Config {
        core_budget: options.core_mb << 20,
        texture_budget: options.texture_mb << 20,
        ..Config::default()
    };
    let indicator_after = config.indicator_after;
    let monitor_profile = load_monitor_profile(primary_monitor_profile_path());
    let title = window_title(&monitor_profile.source);
    let core = ViewCore::start(
        NativeBackend::with_tiff_limit(usize::try_from(config.core_budget).unwrap_or(usize::MAX))
            .with_monitor_profile(monitor_profile.profile.clone()),
        config,
    )?;
    let setup = journal.clone().map(|journal| WriteSetup {
        journal,
        checks: Box::new(image_check(&core)),
        options: settings.edit_options(),
    });
    let metadata_worker = MetadataWorker::start(setup).map_err(|e| e.to_string())?;
    let import_worker =
        ImportWorker::start(journal, Box::new(image_check(&core))).map_err(|e| e.to_string())?;
    let thumb_worker = ThumbWorker::start_with_monitor_profile(monitor_profile.profile.clone())
        .map_err(|e| e.to_string())?;
    // The window as it was left (ADR 0017 §2.5, T109): maximized unless a normal size on a
    // screen was saved; full screen on top.
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(title)
        .with_drag_and_drop(true);
    viewport = match start_window(&settings.shell, rect_on_a_screen) {
        StartWindow::Maximized => viewport.with_maximized(true),
        StartWindow::At { position, size } => {
            viewport.with_position(position).with_inner_size(size)
        }
    };
    if settings.shell.fullscreen {
        viewport = viewport.with_fullscreen(true);
    }
    let native = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let path = options.path;
    eframe::run_native(
        WINDOW_TITLE,
        native,
        Box::new(move |cc| {
            // Both themes and the text sizes before the first frame (ADR 0017 §2.5, T109).
            cc.egui_ctx
                .set_visuals_of(egui::Theme::Dark, visuals(Theme::Dark));
            cc.egui_ctx
                .set_visuals_of(egui::Theme::Light, visuals(Theme::Light));
            cc.egui_ctx.all_styles_mut(apply_text_sizes);
            cc.egui_ctx.set_theme(egui_theme(settings.shell.theme));
            let ctx = cc.egui_ctx.clone();
            core.set_wake(move || ctx.request_repaint());
            let mut loupe = Loupe {
                core,
                metadata_worker,
                exiftool_checked: false,
                settings,
                settings_path,
                settings_window: None,
                pending_options: None,
                metadata: MetadataCache::default(),
                import_worker,
                import_dialog: None,
                last_form: ImportForm::default(),
                import_clear_confirm: false,
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
                status_bar: StatusBar::default(),
                next_status: StatusBar::default(),
                scroll_request: None,
                metadata_window: None,
                message: None,
                indicator_after,
                tabs: Tabs::default(),
                ratings_read: false,
                last_ratings_ready: false,
                last_frame_status: Status::Empty,
                monitor_profile,
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
    use super::{
        FALLBACK_CHOICES, LABEL_TEXT, SettingsText, exiftool_message, fallback_from_text,
        fallback_text, index_span, label_color, label_pill, recovery_text, resolve_path,
        resolve_paths, settings_save_error_text, settings_text, window_editor_properties,
    };
    use egui::{Color32, Rect, pos2};
    use pw_loupe::{TEXT_CONTRAST, contrast_ratio};
    use pw_metadata::ToolVersion;
    use pw_metadata::iim::Fallback;
    use pw_metadata::journal::{Entry, Recovered};
    use std::path::{Path, PathBuf};

    fn entry(folder: &Path) -> Entry {
        Entry {
            original: folder.join("bild.jpg"),
            copy: folder.join("bild.pwtmp-1.jpg"),
            backup: folder.join("bild.pwbak-1.jpg"),
            original_existed: true,
        }
    }

    #[test]
    fn exiftool_message_names_a_start_error() {
        let result = Err("ExifTool: could not start exiftool: program not found".to_string());
        assert_eq!(
            exiftool_message(&result),
            Some((
                "ExifTool saknas",
                "ExifTool kunde inte startas: ExifTool: could not start exiftool: program not \
                 found. Betyg och metadata kan inte läsas eller sparas."
                    .to_string()
            ))
        );
    }

    #[test]
    fn exiftool_message_says_that_12_40_is_too_old() {
        let result = Ok(ToolVersion {
            major: 12,
            minor: 40,
        });
        assert_eq!(
            exiftool_message(&result),
            Some((
                "ExifTool är för gammal",
                "ExifTool 12.40 är för gammal. RawStride behöver version 12.67 eller senare. \
                 Betyg och metadata sparas inte förrän ExifTool har uppdaterats."
                    .to_string()
            ))
        );
    }

    #[test]
    fn exiftool_message_is_none_from_12_67() {
        for (major, minor) in [(12, 67), (13, 59)] {
            assert_eq!(exiftool_message(&Ok(ToolVersion { major, minor })), None);
        }
    }

    #[test]
    fn the_two_encodings_have_their_texts_and_come_back_from_them() {
        assert_eq!(FALLBACK_CHOICES, [Fallback::Windows1252, Fallback::Latin1]);
        assert_eq!(
            fallback_text(Fallback::Windows1252),
            "Windows-1252 (västeuropeisk)"
        );
        assert_eq!(fallback_text(Fallback::Latin1), "ISO 8859-1 (Latin-1)");
        for fallback in FALLBACK_CHOICES {
            assert_eq!(fallback_from_text(fallback_text(fallback)), Some(fallback));
        }
        assert_eq!(fallback_from_text("UTF-8"), None);
        assert_eq!(fallback_from_text(""), None);
    }

    #[test]
    fn the_settings_window_has_the_texts_of_the_task() {
        assert_eq!(settings_text(SettingsText::Title), "Inställningar");
        assert_eq!(
            settings_text(SettingsText::CreateIim),
            "Skapa IPTC-IIM för kompatibilitet med äldre system"
        );
        assert_eq!(
            settings_text(SettingsText::CreateIimNote),
            "Gäller JPEG, TIFF och DNG som saknar IIM. Av som standard."
        );
        assert_eq!(
            settings_text(SettingsText::Encoding),
            "Teckenkodning för äldre IPTC- och Exif-text"
        );
        assert_eq!(settings_text(SettingsText::Save), "Spara");
        assert_eq!(settings_text(SettingsText::Cancel), "Avbryt");
        assert_eq!(
            settings_save_error_text("åtkomst nekad"),
            "Inställningarna kunde inte sparas: åtkomst nekad"
        );
    }

    #[test]
    fn recovery_lists_a_file_that_needs_the_user_without_a_backup() {
        let folder = std::env::temp_dir().join("pw-loupe-recovery");
        let recovered = [Recovered::NeedsUser {
            entry: entry(&folder),
        }];

        let text = recovery_text(&recovered, |_| false);

        assert_eq!(
            text,
            Some(format!(
                "RawStride avslutades mitt i en metadataskrivning. Kontrollera de här filerna:\n{}",
                folder.join("bild.jpg").display()
            ))
        );
    }

    #[test]
    fn recovery_names_the_backup_when_it_exists() {
        let folder = std::env::temp_dir().join("pw-loupe-recovery");
        let backup = folder.join("bild.pwbak-1.jpg");
        let recovered = [Recovered::NeedsUser {
            entry: entry(&folder),
        }];

        let text = recovery_text(&recovered, |path| path == backup);

        assert_eq!(
            text,
            Some(format!(
                "RawStride avslutades mitt i en metadataskrivning. Kontrollera de här \
                 filerna:\n{} (säkerhetskopia: {})",
                folder.join("bild.jpg").display(),
                backup.display()
            ))
        );
    }

    #[test]
    fn recovery_shows_nothing_for_cleaned_and_restored_entries() {
        let folder = std::env::temp_dir().join("pw-loupe-recovery");
        let recovered = [
            Recovered::Cleaned {
                entry: entry(&folder),
                deleted: vec![folder.join("bild.pwtmp-1.jpg")],
            },
            Recovered::Restored {
                entry: entry(&folder),
                deleted: Vec::new(),
            },
        ];

        assert_eq!(recovery_text(&recovered, |_| true), None);
        assert_eq!(recovery_text(&[], |_| true), None);
    }

    #[test]
    fn recovery_lists_failed_and_unreadable_entries_but_not_cleaned_ones() {
        let folder = std::env::temp_dir().join("pw-loupe-recovery");
        let journal_file = folder.join("journal").join("0001.json");
        let recovered = [
            Recovered::Cleaned {
                entry: entry(&folder),
                deleted: Vec::new(),
            },
            Recovered::Failed {
                entry: entry(&folder),
                error: "access denied".to_string(),
            },
            Recovered::Unreadable {
                file: journal_file.clone(),
                error: "not JSON".to_string(),
            },
        ];

        let text = recovery_text(&recovered, |_| false);

        assert_eq!(
            text,
            Some(format!(
                "RawStride avslutades mitt i en metadataskrivning. Kontrollera de här \
                 filerna:\n{}\n{}",
                folder.join("bild.jpg").display(),
                journal_file.display()
            ))
        );
    }

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
    fn the_windows_other_metadata_leaves_out_the_fields_but_keeps_the_contact_info() {
        let properties = window_editor_properties();

        // 22 fields, three of them (E-post, Telefon, Webb) in the contact structure.
        assert_eq!(properties.len(), 19);
        assert!(!properties.contains(&"Iptc4xmpCore:CreatorContactInfo"));
        for id in ["dc:title", "dc:description", "dc:subject", "photoshop:City"] {
            assert!(properties.contains(&id), "{id}");
        }
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

    #[test]
    fn label_text_meets_wcag_aa_on_every_label_colour() {
        for label in ["Red", "Yellow", "Green", "Blue", "Purple"] {
            let ratio = contrast_ratio(LABEL_TEXT, label_color(label).unwrap());
            assert!(ratio >= TEXT_CONTRAST, "{label}: {ratio:.2}");
        }
    }
}
