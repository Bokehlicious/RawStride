//! RawStride (ADR 0012): the egui program, with the loupe (ADR 0009), folder tabs and the import
//! (ADR 0010) and the contact sheet (ADR 0011).
//!
//! eframe with glow (GUI_REQUIREMENTS §7) hosts one image area. `pw-viewcore` decides what to show
//! and prepares it on its workers; `pw-render` uploads and draws. Metadata reads and writes are
//! queued to a separate worker; the UI only polls completed results (G2).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use inspector::{ExifInspector, ExifReader, ExifShown, exif_state_text};
use inspector_text::{InspectorText, exif_label, inspector_text};
use metadata::{
    DeleteAnswer, EditorEvent, InspectorState, MetadataCache, MetadataSubmit, MoveAnswer,
};
use pw_imagecheck::RawImageCheck;
use pw_loupe::SavedLibrary;
use pw_loupe::clipping::{ClipChoice, OverlayCache, overlay_uv};
use pw_loupe::file_stamp;
use pw_loupe::focus::FocusCache;
use pw_loupe::library::{
    FolderList, Library, TreeRowKind, folder_key, read_drives, suppress_no_disk_dialogs,
};
use pw_loupe::library_text::{LibraryText, library_text};
use pw_loupe::{AUTHORS, AboutText, COMMIT, REPOSITORY, VERSION, about_text};
use pw_loupe::{
    Action, AddedFiles, CardRefresh, CellState, CopiedFields, DEFAULT_THUMBNAIL_SIZE, DialogKey,
    DialogRequest, EditorField, EditorForm, EditorItem, EditorKeyAction, EditorLayout,
    EditorSection, EditorText, Field, Filter, GridKeyAction, ImportDialog, ImportDialogState,
    ImportEvent, ImportForm, ImportWorker, IptcField, LABEL_CHOICES, LabelFilter, Language,
    LoupeMove, MAX_THUMBNAIL_SIZE, MIN_THUMBNAIL_SIZE, MonitorProfile, Navigation, Options,
    PanelText, READING_RATINGS_TEXT, RatingButton, RatingRefused, STAR_CONDITIONS, STAR_FILTERS,
    Settings, StarFilter, SubmitError, Tab, TabAction, Tabs, ThumbGrid, UNSUPPORTED_TEXT, ViewMode,
    WINDOW_TITLE, allowed_modes, apply_confirmation, carried_map, clamp_thumbnail_size,
    clear_and_import_text, clear_cancel_text, clear_confirmation_text, clear_confirmation_title,
    color_image, columns, dialog_key_action, done_lines, editor_key_action, editor_rating_key,
    editor_text, ejecting_progress_text, error_text, files_gone, grid_key_action, indicator_text,
    info_panel, key_action, label_choice_text, label_filter_text, label_name, list_folder,
    load_monitor_profile, load_settings_and_library, loupe_key_action, marked_text,
    metadata_neighbour, metadata_steps, mode_label, mode_text, more_lines, no_match_text,
    notice_text, opens_import, other_values, panel_text, parse_args, position_text, preview_line,
    preview_summary, primary_monitor_profile_path, progress_text, property_label,
    rating_key_action, request_cell, save_settings_with_library, settings_path,
    star_condition_text, star_filter_text, star_glyphs, status_line, status_text, step_allowed,
    step_thumbnail_size, tab_action, take_wheel_steps, visible_range, wheel_factor, window_title,
};
use pw_loupe::{
    Batch, MoveKey, MoveKind, MovedImage, UndoMoves, move_failed_text, move_key_action,
    move_running_text, moved_to_folders, moves_failed_text, nothing_to_undo_text, same_folder,
    target_folder, undo_failed_text, undone_text, worker_stopped_text,
};
use pw_loupe::{
    ButtonFacts, THUMB_BUTTONS, ThumbButton, button_at, button_rect, menu_items,
    preview_only_marked, thumb_button_hover_text,
};
use pw_loupe::{
    COLLAPSED_WIDTH, ColumnState, Command, ExternalProgram, IMAGE_CONTEXT_MENU,
    MAX_EXTERNAL_PROGRAMS, MENUS, MenuEntry, MenuItem, RatingKey, STATUS_TEXT_SIZE, SettingsPage,
    SettingsText, ShellAction, ShellSettings, ShellText, StartWindow, THEMES, Theme, Where,
    WindowRect, apply_text_sizes, clamp_column_width, column_width_range, command_text,
    exiftool_message, external_program_limit_text, fallback_text, font_definitions, image_area,
    key_text, menu_text, no_images_text, open_error_text, palette, path_resolution_error_text,
    prune_error_text, recovery_text, rect_on_a_screen, settings_save_error_text, settings_text,
    shell_key_action, shell_text, start_window, status_line_text, submenu_text, theme_text,
    tidied_programs, visuals,
};
use pw_loupe::{
    CaptureReader, SORT_KEYS, SortFollow, SortKey, SortOrder, modified_time, read_order,
    sort_button_text, sort_direction_hover, sort_direction_text, sort_key_text, sorting_text,
};
use pw_loupe::{ClippingText, clipping_state_text, clipping_text};
use pw_loupe::{
    ClockTime, ImportRow, ImportStep, PendingWrite, Tasks, TasksText, error_entry_text,
    errors_heading_text, import_done_text, import_route_text, importing_text, pending_write_text,
    pending_writes, tasks_text,
};
use pw_loupe::{Compare, Layout, compare_side_text, follow, open_indexes, sides};
use pw_loupe::{
    DeleteBatch, DeleteStep, delete_button_text, delete_cancel_text, delete_confirm_text,
    delete_failed_text, delete_key_action, delete_partly_failed_text, delete_partners_text,
    delete_refused_text, delete_running_text, delete_title_text, delete_while_moving_text,
    deleted_text, deletes_failed_text, image_partners, no_rejected_text, plan_delete,
    ratings_not_ready_text, refuse_unrejected_partners, rejected_images, same_path,
};
use pw_loupe::{
    EmbeddedOrientationReader, ROTATION_UNSUPPORTED_TEXT, ReaderStopped, rotation_key_action,
};
use pw_loupe::{
    FILMSTRIP_CELL_WIDTH, FILMSTRIP_IMAGE_HEIGHT, FILMSTRIP_LABEL_HEIGHT, FILMSTRIP_MARGIN,
    FILMSTRIP_SPACING, PreviewKey, PreviewText, Restore, ThumbGate, WindowBefore, clamp_offset,
    filmstrip_range, hit_test, preview_info_shown, preview_key_action, preview_parts, preview_text,
    scroll_target, thumbnail_gate, toggle_preview_column, toggle_preview_hidden, zoom_value_text,
};
use pw_loupe::{
    FieldValues, ValueChoice, ValueList, field_values_path, load_field_values, save_field_values,
    value_choices,
};
use pw_loupe::{FolderWatcher, WatchEvent};
use pw_loupe::{HISTOGRAM_BINS, HistogramPanel, HistogramText, histogram_panel, histogram_text};
use pw_loupe::{HistogramInspector, HistogramReader, HistogramReads};
use pw_loupe::{ImportPosition, ImportTabs, PlacedInFolder, with_placed};
use pw_loupe::{
    ImportText, failed_cell_text, hidden_done_text, hidden_progress_text, import_text,
    no_preview_text,
};
use pw_loupe::{
    OPEN_IN_KEY, OpenPlan, OpenRequest, OpenStep, StartError, explorer_failed_text,
    explorer_missing_text, image_gone_text, nothing_to_open_text, open_button_text,
    open_cancel_text, open_in_another_text, open_in_key_action, open_in_text,
    open_many_question_text, opened_text, plan_open, program_missing_text, show_in_explorer,
    start_failed_text, start_program, too_long_text, waiting_for_writes_text,
};
use pw_loupe::{
    Relisted, anchored_offset, folder_changed_text, folder_gone_text, inside_lexically,
    reload_key_action, removed_by_another_program_text, top_position,
};
use pw_metadata::edit::{Edit, EditOptions};
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Journal;
use pw_metadata::read::MetadataRead;
use pw_metadata::template::Mode;
use pw_metadata::{
    DeleteError, DeleteRefusal, MetadataWorker, PreparedDelete, Rating, WriteSetup, supports_rating,
};
use pw_render::{Textures, view_px};
use pw_thumbcache::{ThumbWorker, ThumbnailRequest};
use pw_viewcore::native::NativeBackend;
use pw_viewcore::{
    ClippingState, Config, FocusState, Frame, ImageStatistics, Source, Status, ViewCore, Zoom,
};

mod inspector;
mod inspector_text;
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
/// The folder dialog's starting folder as text when there is none (not shown; `pick_folder` then
/// starts in the open tab's folder).
const NO_FOLDER_TEXT: &str = "";
/// How long the image check waits for the view core to close the RAW sessions of a file (T91).
const RELEASE_TIMEOUT: Duration = Duration::from_secs(5);
/// The number of ExifTool write processes of the metadata worker and of the import's template
/// (ADR 0007 §9.6 point 2, Q18): four, fixed by the owner on 2026-11-22 after T267's measurement
/// (T268).
const WRITE_PROCESSES: usize = 4;
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
/// How often a frame is asked for while a batch of moves or an undo runs (ADR 0017 U9b, T174).
const MOVE_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// The widest the confirmation of a delete to the Recycle Bin grows; its texts wrap within it
/// (ADR 0018 B4, T189).
const DELETE_WINDOW_WIDTH: f32 = 420.0;
/// The widest the question of "Open in {name}" above 100 images grows; its text wraps within it
/// (ADR 0019 §2.5, T202).
const OPEN_WINDOW_WIDTH: f32 = 420.0;
/// How often a frame is asked for while capture times are being read for the sort (T138).
const CAPTURE_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// How long an import with "Eject" waits for the watches on the card's drive to end before it
/// starts anyway (ADR 0026 §3.2, T248); the window is blocked meanwhile, once per import.
const EJECT_WATCH_WAIT: Duration = Duration::from_secs(2);
/// The space around the line at the top of a side of the compare, and the plate behind it (T153).
const SIDE_TEXT_MARGIN: f32 = 6.0;
const SIDE_TEXT_PLATE: egui::Color32 = egui::Color32::from_black_alpha(170);
/// The dashed frame of the held image's cell in the filmstrip (U6-Q10, T153): how far inside the
/// cell it is drawn (inside the current cell's frame), and its dashes and gaps.
const HELD_FRAME_INSET: f32 = 4.0;
const HELD_FRAME_DASH: f32 = 6.0;
const HELD_FRAME_GAP: f32 = 4.0;
/// How many frames the window state is not taken after the preview closed, at most, while the
/// system has not yet reported the window out of the preview's full screen (T111).
const WINDOW_SETTLE_FRAMES: u32 = 30;

/// The image the metadata window opens for (ADR 0007 I10n, D16, T107), as the metadata cache keys
/// it (`cwd` resolves a relative path): the active tab's current image when it is shown. With two
/// or more images marked, in the contact sheet or the preview (ADR 0017 U12c-Q1, T126), the
/// current image when it is marked, else the first marked image in the shown order.
fn metadata_subject(tab: Option<&Tab>, cwd: Option<&Path>) -> Option<PathBuf> {
    let tab = tab?;
    let marked = tab.selection.marked_in(&tab.visible);
    let index = if marked.len() > 1 && !marked.contains(&tab.current) {
        *marked.first()?
    } else {
        tab.position().map(|_| tab.current)?
    };
    let path = tab.files.get(index)?;
    Some(match cwd {
        Some(cwd) => resolve_path(cwd, path),
        None => path.clone(),
    })
}

/// The marked images of the active tab in the shown order, as the metadata cache keys them, when
/// two or more are marked (ADR 0017 U12c-Q1, T126); empty otherwise. Kinds without metadata
/// support are included: "Apply to All Marked" names them as skipped.
fn marked_subjects(tab: Option<&Tab>, cwd: Option<&Path>) -> Vec<PathBuf> {
    let Some(tab) = tab else {
        return Vec::new();
    };
    let marked = tab.selection.marked_in(&tab.visible);
    if marked.len() < 2 {
        return Vec::new();
    }
    marked
        .iter()
        .filter_map(|&index| tab.files.get(index))
        .map(|path| match cwd {
            Some(cwd) => resolve_path(cwd, path),
            None => path.clone(),
        })
        .collect()
}

/// Whether "Apply to {n} Marked" can be used for the form of the window on `path` (U12c-Q2,
/// T126, review): two or more images marked, the window's image one of them (Previous / Next can
/// move the window to an image that is not marked), no field error, and something to write to
/// the other images: a changed field, or a list field's added or removed entries as an
/// `Edit::Entries` (U12d-Q1, T180); not only a chosen conflict value, an emptied copyright field
/// or removed Creator entries, which stay on the window's image (U12d-Q2), nor a list field
/// whose entries were only reordered; the other images would otherwise get a write without
/// edits.
fn apply_allowed(form: &EditorForm, path: &Path, marked: &[PathBuf]) -> bool {
    marked.len() > 1
        && marked.iter().any(|image| image.as_path() == path)
        && form
            .batch_edits()
            .is_ok_and(|batch| !batch.others.is_empty())
}

/// "Apply to All Marked" as the confirmation asked it (U12c-Q3, U12c-Q4, T126): the edits of the
/// window's image, the other marked images with metadata support and their edits, and the lines
/// of the question.
struct ApplyPlan {
    this_image: Vec<Edit>,
    others: Vec<(PathBuf, Vec<Edit>)>,
    lines: Vec<String>,
}

impl ApplyPlan {
    /// The plan for the form of the window on `path` with `marked` (the marked images, shown
    /// order); every other marked image with metadata support gets the changed fields (a list
    /// field as an `Edit::Entries` of its added and removed entries, U12d-Q1, T180), the kinds
    /// without support are counted as skipped. The field errors when a field has one.
    fn new(
        form: &EditorForm,
        path: &Path,
        marked: &[PathBuf],
    ) -> Result<ApplyPlan, Vec<(EditorField, String)>> {
        let batch = form.batch_edits()?;
        let others: Vec<(PathBuf, Vec<Edit>)> = marked
            .iter()
            .filter(|other| other.as_path() != path && supports_rating(other))
            .map(|other| (other.clone(), batch.others.clone()))
            .collect();
        let skipped = marked
            .iter()
            .filter(|other| !supports_rating(other))
            .count();
        let lines = apply_confirmation(&batch, others.len() + 1, skipped);
        Ok(ApplyPlan {
            this_image: batch.this_image,
            others,
            lines,
        })
    }
}

/// The inspector's image (ADR 0017 §2.1, §2.2, T110): the active tab's current image when it is
/// shown, as the metadata cache keys it (`cwd` resolves a relative path). In the contact sheet it
/// is the current cell also when several images are marked.
fn inspector_subject(tab: Option<&Tab>, cwd: Option<&Path>) -> Option<PathBuf> {
    let tab = tab?;
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
    /// The summary of "Apply to All Marked" (U12c-Q6, T126), each line as `BatchSummary::lines`
    /// gives it, and a failed read after the batch.
    Batch(Vec<String>),
    /// A failed write, or a failed read after a write; the whole text.
    Failed(String),
}

/// The status when OK, Save and Next, Save or "Apply to {n} Marked" wrote nothing because of
/// field errors (U11c-Q9, T181): the fields of `errors` in their order and the structure lists
/// `lists` that have an error.
fn not_saved_status(errors: &[(EditorField, String)], lists: Vec<&'static str>) -> EditorStatus {
    let fields = errors.iter().map(|(field, _)| *field).collect();
    EditorStatus::Failed(editor_text(&EditorText::NotSaved { fields, lists }))
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

/// What takes the metadata window away from its image (ADR 0017 §2.3.1, T112).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowLeave {
    /// "Föregående" (`forward` false) or "Nästa" / "Spara och nästa" (`forward` true).
    Move { forward: bool },
    /// OK, Escape or the window's "Avbryt".
    Close,
}

/// The answer to "Spara ändringarna?" (U12-Q4), or what "Spara och nästa" and OK do (`Save`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SaveAnswer {
    Save,
    Discard,
    Cancel,
}

/// How the window's write of its edits ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriteOutcome {
    /// Written without notices.
    Success,
    /// Written, with notices to show (§2.3.1, U12-Q5).
    Notices,
    /// The write failed or could not be sent.
    Failure,
    /// A field has an error: nothing was written.
    FieldErrors,
}

/// What the metadata window does about a move or a close (§2.3.1, U12-Q4, U12-Q5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeaveDecision {
    /// The move or the close is done.
    Leave,
    /// The edits are written first; the decision follows from how the write ends.
    Write,
    /// The window stays on the image with the edits kept.
    StayWithEdits,
    /// The window stays on the image, refilled from the new read, showing "Sparat." and the
    /// notices; the next press of a move moves on.
    StayShowingNotices,
}

/// The decision of a move or a close of the metadata window (ADR 0017 §2.3.1, U12-Q4, U12-Q5,
/// T112): `has_edits` whether the form holds unsaved edits, `answer` the answer to "Spara
/// ändringarna?" (or `Save` for "Spara och nästa"), `outcome` how the write ended, `None` while
/// no write has been done. Without edits the move or the close is done at once.
fn leave_decision(
    has_edits: bool,
    answer: SaveAnswer,
    outcome: Option<WriteOutcome>,
) -> LeaveDecision {
    if !has_edits {
        return LeaveDecision::Leave;
    }
    match (answer, outcome) {
        (SaveAnswer::Discard, _) | (SaveAnswer::Save, Some(WriteOutcome::Success)) => {
            LeaveDecision::Leave
        }
        (SaveAnswer::Cancel, _)
        | (SaveAnswer::Save, Some(WriteOutcome::Failure | WriteOutcome::FieldErrors)) => {
            LeaveDecision::StayWithEdits
        }
        (SaveAnswer::Save, None) => LeaveDecision::Write,
        (SaveAnswer::Save, Some(WriteOutcome::Notices)) => LeaveDecision::StayShowingNotices,
    }
}

/// What the active tab does about ratings chosen in the metadata window (ADR 0017 U10c-Q4, T124).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowRefilter {
    /// Nothing is refiltered: nothing was rated in the window, or the window stays on its image.
    Nothing,
    /// The window moved to file `index`, found in the visible list as it was: the tab is
    /// refiltered keeping it.
    KeepNeighbour(usize),
    /// The window closed: the tab is refiltered as after a rating key in the contact sheet.
    AsAfterKey,
}

/// The top-left corner the metadata window opens at (T131): its centre on the centre of
/// `image_area` (the main window's central panel), then moved inside `screen` (the program
/// window) as far as it fits. A window larger than `screen` keeps its top-left corner inside, so
/// its title bar is never above the program window's top edge.
fn metadata_window_position(
    image_area: egui::Rect,
    window_size: egui::Vec2,
    screen: egui::Rect,
) -> egui::Pos2 {
    let centred = image_area.center() - window_size / 2.0;
    // The far edges first, then the near ones, so the near ones win when it does not fit.
    let x = centred
        .x
        .min(screen.max.x - window_size.x)
        .max(screen.min.x);
    let y = centred
        .y
        .min(screen.max.y - window_size.y)
        .max(screen.min.y);
    egui::pos2(x, y)
}

/// The refilter of the active tab when the metadata window leaves its image (ADR 0017 U10c-Q4,
/// T124). A rating chosen in the window does not refilter the tab at once, so the window's image
/// stays in the visible list and "Previous" / "Next" have something to start from. `rated`: a
/// rating or label was chosen in the window on its image; `leave`: the move or close done, `None`
/// while the window stays; `neighbour`: the file index the move found in the visible list before
/// any refilter, `None` when the move found nothing (the window stays).
fn window_refilter(
    rated: bool,
    leave: Option<WindowLeave>,
    neighbour: Option<usize>,
) -> WindowRefilter {
    match (rated, leave, neighbour) {
        (false, _, _) | (true, None, _) | (true, Some(WindowLeave::Move { .. }), None) => {
            WindowRefilter::Nothing
        }
        (true, Some(WindowLeave::Move { .. }), Some(index)) => WindowRefilter::KeepNeighbour(index),
        (true, Some(WindowLeave::Close), _) => WindowRefilter::AsAfterKey,
    }
}

/// The metadata window (ADR 0007 I10n, D16, T107): one image's metadata seen and edited in one
/// floating window. It replaces the info panel of T103 and the editor window of T105. Since T112
/// (ADR 0017 §2.3.1) it is not locked to its image: "Föregående", "Nästa" and "Spara och nästa"
/// move it over the shown images that have metadata support.
struct MetadataWindow {
    /// The image, as the metadata cache keys it.
    path: PathBuf,
    file: String,
    body: WindowBody,
    /// The field errors of the last write asked for.
    errors: Vec<(EditorField, String)>,
    status: EditorStatus,
    /// "Spara ändringarna?" is shown for this move or close (unsaved edits, U12-Q4).
    question: Option<WindowLeave>,
    /// The move or close done once the window's write has succeeded without notices (OK, "Spara"
    /// in the question, "Spara och nästa").
    after_save: Option<WindowLeave>,
    /// Escape was pressed this frame; the keys are read before the window is drawn.
    escape: bool,
    /// The window's write succeeded with notices: the read after it refills the form.
    expect_reload: bool,
    /// A rating or label was chosen in the window on this image (T124): the tab is refiltered
    /// when the window moves or closes (`window_refilter`).
    rated: bool,
    /// The marked images of the active tab when the window opened, two or more, in the shown
    /// order (U12c-Q1, T126); empty with one or none. The marking does not change while the
    /// window is open, so this is kept when it moves.
    marked: Vec<PathBuf>,
    /// The confirmation of "Apply to All Marked" is shown with this plan (U12c-Q4, T126).
    apply_question: Option<ApplyPlan>,
    /// The open "▼" list of recent and own values (ADR 0017 U12a-Q5, T141), if any.
    value_list: Option<ValueList>,
    /// Escape was pressed this frame while the "▼" list was open: it closes only the list.
    value_list_escape: bool,
    /// What the write sent (OK, "Save and Next", "Apply to All Marked") puts in the fields with a
    /// "▼" (U12a-Q2, T141); moved to `recorded` when it succeeds.
    written: Vec<(EditorField, Vec<String>)>,
    /// The values of a write that succeeded, not yet added to the recent values.
    recorded: Vec<(EditorField, Vec<String>)>,
}

impl MetadataWindow {
    fn new(path: PathBuf, marked: Vec<PathBuf>) -> Self {
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
            question: None,
            after_save: None,
            escape: false,
            expect_reload: false,
            rated: false,
            marked,
            apply_question: None,
            value_list: None,
            value_list_escape: false,
            written: Vec::new(),
            recorded: Vec::new(),
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

    /// Applies what the metadata cache says about the window's write. The move or close to do
    /// when a write without notices succeeded (`after_save`; a close if none was asked for).
    fn apply(&mut self, event: EditorEvent) -> Option<WindowLeave> {
        match event {
            EditorEvent::Saved { path, notices } if path == self.path => {
                // The values written become recent values (U12a-Q2, T141), notices or not.
                self.recorded.append(&mut self.written);
                let outcome = if notices.is_empty() {
                    WriteOutcome::Success
                } else {
                    WriteOutcome::Notices
                };
                let then = self.after_save.take();
                match leave_decision(true, SaveAnswer::Save, Some(outcome)) {
                    LeaveDecision::Leave => return Some(then.unwrap_or(WindowLeave::Close)),
                    LeaveDecision::Write
                    | LeaveDecision::StayWithEdits
                    | LeaveDecision::StayShowingNotices => {}
                }
                // The asked-for move is not done; the next press of a move moves on (U12-Q5).
                self.status = EditorStatus::Saved(notices.iter().map(notice_text).collect());
                self.expect_reload = true;
            }
            EditorEvent::Failed { path, message } if path == self.path => {
                // The window stays on the image with the edits kept (§2.3.1).
                self.after_save = None;
                self.written.clear();
                self.status = EditorStatus::Failed(message);
            }
            EditorEvent::Reloaded { path, result } if path == self.path && self.expect_reload => {
                self.expect_reload = false;
                match result {
                    Ok(read) => self.fill(read),
                    // After a batch the summary stays, with the read's error under it.
                    Err(error) => match &mut self.status {
                        EditorStatus::Batch(lines) => {
                            lines.push(panel_text(&PanelText::ReadError(error)));
                        }
                        _ => {
                            self.status =
                                EditorStatus::Failed(panel_text(&PanelText::ReadError(error)));
                        }
                    },
                }
            }
            // Every write of "Apply to All Marked" answered (U12c-Q6, T126): the summary is
            // shown and the read of the window's image after it refills the form.
            EditorEvent::BatchDone(summary) => {
                // Every image got the changed fields: when any write succeeded, they were used
                // (U12a-Q2, T141).
                if summary.written > 0 {
                    self.recorded.append(&mut self.written);
                } else {
                    self.written.clear();
                }
                self.status = EditorStatus::Batch(summary.lines());
                self.expect_reload = true;
            }
            EditorEvent::Saved { .. }
            | EditorEvent::Failed { .. }
            | EditorEvent::Reloaded { .. } => {}
        }
        None
    }
}

/// The open compare (ADR 0017 §2.2, U6, T153): the held image, where it was opened, and what the
/// second view core has been told.
struct CompareOpen {
    /// The held image, as the tab's list of files names it.
    compare: Compare,
    /// The active tab's index and folder when the compare opened; it closes when either differs
    /// (U6-Q11).
    tab: usize,
    folder: Option<PathBuf>,
    /// The held side has not yet been given the current side's zoom and centre: set when the
    /// compare opens and when the held image changes, done once at the next drawn frame.
    view_pending: bool,
    /// The display mode last given to the second view core (U6-Q9); `None` before the first.
    mode_sent: Option<Source>,
    /// The zoom and centre last given to the second view core while locked (U6-Q8, T154, T151's
    /// `follow`); `None` when forgotten: at open, when the held image changes, when the lock is
    /// turned on, and when the held side's image size first becomes known.
    view_sent: Option<(Zoom, [f64; 2])>,
    /// The held side's frame has given its image's size since the held image was last set
    /// (T154): the view sent before that is sent again once.
    held_size_known: bool,
}

/// The images of one batch of moves: `(image, target folder, rating to restore)`, absolute paths
/// (ADR 0017 U9b, T172, T174).
type MoveImages = Vec<(PathBuf, PathBuf, Option<Rating>)>;

/// The moves to subfolders and their undo (ADR 0017 §2.6, U9b, U9-Q5–U9-Q9, T174): one batch or
/// one undo runs at a time; a later key's batch waits behind it.
#[derive(Default)]
struct Moves {
    /// The batches of later keys, in key order.
    waiting: VecDeque<Batch>,
    running: Option<MoveRun>,
    /// The steps that can be undone (U9-Q8).
    undo: UndoMoves,
}

impl Moves {
    /// Whether a batch or an undo runs or waits.
    fn busy(&self) -> bool {
        self.running.is_some() || !self.waiting.is_empty()
    }
}

/// The batch or the undo that runs.
enum MoveRun {
    /// A batch of moves, with the ids of the requests sent for it.
    Batch {
        batch: Batch,
        ids: HashSet<pw_metadata::RequestId>,
    },
    /// An undo step: each image's move back.
    Undo(Vec<UndoImage>),
}

/// One image of a running undo: its moves, the id of its move back once sent, and the answer.
struct UndoImage {
    moved: MovedImage,
    id: Option<pw_metadata::RequestId>,
    answer: Option<Result<(), String>>,
}

impl MoveRun {
    /// Whether every image has its answer.
    fn done(&self) -> bool {
        match self {
            MoveRun::Batch { batch, .. } => batch.done(),
            MoveRun::Undo(images) => images.iter().all(|image| image.answer.is_some()),
        }
    }
}

struct Loupe {
    core: ViewCore<NativeBackend>,
    /// The second view core, for the held image of the compare (ADR 0017 §2.2, U6-Q2, U6-Q3,
    /// T152), with its own backend, budgets, textures and clipping marks. It holds an empty list
    /// until the compare opens a file in it. It is emptied with `set_files(vec![], 0)`, never by
    /// being given another `Textures`.
    second_core: ViewCore<NativeBackend>,
    second_textures: Textures,
    second_clip_overlay: OverlayCache<egui::TextureHandle>,
    /// The texture of the focus marking on the held image of the compare (U9-Q4, T168).
    second_focus_overlay: FocusCache<egui::TextureHandle>,
    /// The compare while it is open (T153); not remembered after a restart (U6-Q11).
    compare: Option<CompareOpen>,
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
    /// The Settings window's page shown (ADR 0017 U10b-Q6, T278): set when the window opens, kept
    /// while it is open, not saved.
    settings_page: SettingsPage,
    /// The window "About RawStride" is open (ADR 0031 RL-Q9, T225). It blocks nothing, so it is not
    /// part of `dialog_open`; only its Close button closes it.
    about_window: bool,
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
    /// While the import dialog is open, when it asks for the cards again (T161).
    card_refresh: Option<CardRefresh>,
    import_submit_error: Option<String>,
    import_hidden: bool,
    /// The Tasks area under the library (ADR 0017 §2.1, U5, T140): the import's row and the
    /// error list.
    tasks: Tasks,
    thumb_worker: ThumbWorker,
    thumb_grid: ThumbGrid,
    /// The active tab's files (in the tab's own form) the current `thumb_grid` was made for, so
    /// a change of those files carries the made thumbnails to their new places (ADR 0026 §3.5,
    /// T257). Empty when no tab is open.
    thumb_files: Vec<PathBuf>,
    /// The open tabs' files another program added while they were listed, so a thumbnail made
    /// from a file still being written is made anew once it changed (ADR 0026 §3.2, the addition
    /// to F-Q7, T254). One for all tabs: a file stays kept across a tab switch or F5 while its
    /// copy goes on.
    added_files: AddedFiles,
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
    /// The inspector's Exif reader and summary (ADR 0017 §2.1, T110).
    exif_reader: ExifReader,
    exif: ExifInspector,
    /// The inspector's histogram reader and its kept results (ADR 0017 §2.6, U7c, T144).
    histogram_reader: HistogramReader,
    histograms: HistogramInspector,
    /// The capture-time reader of the sort (ADR 0017 U14-Q4, T138) and the files asked for whose
    /// answer has not come back.
    capture_reader: CaptureReader,
    capture_in_flight: HashSet<PathBuf>,
    /// The reader of a file's own Exif orientation, for a turn of an image without a known value
    /// (ADR 0024 §2.5, T236, T237), and the number of its reads asked for whose answer has not
    /// come back.
    orientation_reader: EmbeddedOrientationReader,
    orientation_reads: usize,
    /// The status bar's content of the last frame, and the one made this frame (T109).
    status_bar: StatusBar,
    next_status: StatusBar,
    /// A position the contact sheet scrolls to on the next frame: a menu command or the command
    /// bar changed the current image or the filter (T109).
    scroll_request: Option<usize>,
    /// What the contact sheet showed in its last frame (ADR 0026 §2 point 5, T242), made by
    /// `draw_grid` every frame it runs.
    grid_note: Option<GridNote>,
    /// The note taken at a relist or a placement of the active tab, waiting for the next
    /// `draw_grid` of that tab to keep its top image where it was (T242).
    grid_anchor: Option<GridNote>,
    /// Whether the left column took F5 for the library's "Refresh" this frame (U4-Q1, T242), so
    /// the key loop does not reload the tab too; reset at the start of every frame.
    library_took_f5: bool,
    /// When a button on a thumbnail was last clicked (`InputState::time`; ADR 0017 §2.8, T245):
    /// egui counts a double click per pointer, so a cell's double click soon after it is a plain
    /// click.
    thumb_button_clicked_at: Option<f64>,
    /// The metadata window (M, D16, T107); at most one. While it is open it holds the program.
    metadata_window: Option<MetadataWindow>,
    /// Whether the metadata window was drawn in the last frame: when it was not, the window has
    /// just opened and is placed in the middle of the image area (T131). A move to another image
    /// keeps it set, so the window stays where the user put it.
    metadata_window_shown: bool,
    /// The rectangle of the main window's central panel (the contact sheet or the loupe) in this
    /// frame (T131); `None` before it is drawn and in the preview.
    image_area: Option<egui::Rect>,
    /// The fields of the last "Copy All Fields" (U12b-Q1, T125): kept here, not in the window, so
    /// they outlive closing and moving it; until the next copy or the end of the program.
    copied_fields: Option<CopiedFields>,
    /// The recent and own values of the metadata window's fields (ADR 0017 U12a, T141), read at
    /// start and saved to `field_values_path` when a value is added or removed.
    field_values: FieldValues,
    /// `field-values.txt`; `None` without LOCALAPPDATA (nothing can be saved).
    field_values_path: Option<PathBuf>,
    message: Option<String>,
    /// The moves to subfolders, their undo list and the batch or undo that runs (T174).
    moves: Moves,
    /// Where a delete to the Recycle Bin is (ADR 0018 §2.10, B4, T189); one at a time.
    delete: DeleteStep,
    /// The active tab's folder when the delete was asked for; `None` while `delete` is `Idle`.
    /// The confirmation is not shown for another tab (T189 point 5).
    delete_folder: Option<PathBuf>,
    /// Whether the delete under way is Image ▸ Delete Rejected Images (T190): the images no longer
    /// rejected are left out when the prepare's answer comes and skipped while the batch runs.
    /// Set false by a plain Delete and whenever `delete` is `Idle` again (`delete_idle`).
    delete_rejected: bool,
    /// For Delete Rejected Images, each kept image's partners (`image_partners` of its group in
    /// the prepare's answer) as the tab's resolved paths (T190 point 6): while the batch runs an
    /// image waits while one of them has a rating write pending and is skipped when one of them
    /// is no longer rejected. Kept from the prepare's answer, since the plan does not hold the
    /// groups; empty for a plain Delete and cleared in `delete_idle`.
    delete_partners: ImagePartners,
    /// The ids of the deletes sent for the running batch; an answer with another id is ignored.
    /// Cleared when `delete` is `Idle` again.
    delete_ids: HashSet<pw_metadata::RequestId>,
    /// The confirmation's Cancel gets the focus on the first frame it is shown (ADR 0018 B-Q4):
    /// set when `delete` becomes `Confirming`, cleared when the window is first drawn.
    delete_focus_cancel: bool,
    /// The id of the confirmation's Delete button as last drawn, so Enter deletes only while it
    /// has the focus.
    delete_button: Option<egui::Id>,
    /// Where "Open in {name}" is (ADR 0019 §2.3–§2.6, T202): nothing, the question above 100
    /// images, or a request waiting for the images' writes; one request at a time.
    open: OpenStep,
    /// The question's Cancel gets the focus on the first frame it is shown (ADR 0019 §2.5): set
    /// when `open` becomes `Confirming`, cleared when the window is first drawn or the step
    /// leaves `Confirming`.
    open_focus_cancel: bool,
    /// The id of the question's Open button as last drawn, so Enter opens only while it has the
    /// focus; `None` when the step is not `Confirming`.
    open_button: Option<egui::Id>,
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
    /// The zoom and the effective scale of the last rendered preview frame, for the preview
    /// column's Zoom section (T111).
    last_zoom: Option<(Zoom, Option<f64>)>,
    /// The image of the last rendered preview frame, as `inspector_subject` names it, and its
    /// statistics once they are ready, for the Histogram sections (ADR 0017 §2.6, U7, T142).
    last_statistics: Option<(PathBuf, Option<ImageStatistics>)>,
    /// The clipping state of the last rendered preview frame, for the preview column's Clipping
    /// section (ADR 0017 §2.6, U7b-Q9, T148); not wanted before the first frame.
    last_clipping: ClippingState,
    /// The texture of the clipping marks drawn over the preview's image (ADR 0017 §2.6, U7b,
    /// T149): one at a time, made again only for another map or overlay choice and released when
    /// no overlay is shown. Its memory is outside the view's texture budget (U7b-Q8).
    clip_overlay: OverlayCache<egui::TextureHandle>,
    /// The texture of the focus marking drawn over the preview's image (ADR 0017 §2.6, U9a,
    /// T168): one at a time, made again only for another map and released when the marking is
    /// not shown.
    focus_overlay: FocusCache<egui::TextureHandle>,
    /// The display mode (RAW or JPEG) of the last rendered preview frame, which the inspector's
    /// Histogram section follows (U7c-Q1, T144); RAW before the first frame, as the view core.
    display_mode: Source,
    /// The window's state from before the preview made it full screen; `Some` while the preview
    /// is shown (ADR 0017 §2.2, U3-Q1, U3-Q6, T111).
    window_before: Option<WindowBefore>,
    /// After the preview closed: the full-screen state the window returns to and how many frames
    /// are left to wait for the system to report it; the window state is not taken meanwhile.
    window_settling: Option<(bool, u32)>,
    /// Commands for the window, sent at the end of the frame (the preview's full screen).
    viewport_commands: Vec<egui::ViewportCommand>,
    /// How far the filmstrip is scrolled, in points, and the current image and list length it
    /// was last scrolled for (T111).
    filmstrip_offset: f32,
    filmstrip_followed: Option<(usize, usize)>,
    /// The monitor profile, read once at start (ADR 0016 §4.1, Q4); not used for pixels yet.
    #[allow(dead_code)] // used by ADR 0016 C4
    monitor_profile: MonitorProfile,
    /// The library in the left column (ADR 0017 §2.1, U4, T135): the folder tree, the favourites
    /// and the recent folders; the lists are saved in `settings.txt` with the settings.
    library: Library,
    /// The folder watcher (ADR 0026 §3.2, T248): watches the tabs' folders.
    folder_watcher: FolderWatcher,
    /// The last set of folders given to `folder_watcher`.
    watched_folders: Vec<PathBuf>,
    /// The drive (`"E:"`) an import started with "Eject" will eject, while that import runs: no
    /// folder on it is watched, so no watch keeps the card busy (T248).
    ejecting_drive: Option<String>,
    /// The destination of the running import, as written in its form (trimmed): a tab inside it
    /// is not listed from disk while the import runs (`disk_listing_held`, ADR 0026 F3, T251).
    import_destination: Option<PathBuf>,
    /// The running import's images and folders (ADR 0026 §3.4, T260): set at its `Planned`,
    /// filled at each `Placed` (`import_placed`), and `None` at its `Done`, when its worker has
    /// stopped, and while no import runs. While it is `Some`, a listing of a folder leaves out the
    /// images the import has not shown yet (`leave_out_waiting`).
    import_tabs: Option<ImportTabs>,
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

    /// Opens the settings window with the settings in use, on `page` (T278). Not while the
    /// metadata window holds the program (D16), and not while it is open already, so an open
    /// window keeps its page.
    fn open_settings_window(&mut self, page: SettingsPage) {
        if self.metadata_window.is_none() && self.settings_window.is_none() {
            self.settings_window = Some(self.settings.clone());
            self.settings_page = page;
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
                ..self.settings.shell.clone()
            },
            ..chosen
        };
        self.settings = chosen.clone();
        self.pending_options = Some(chosen.edit_options());
        self.send_pending_options();
        let saved = match &self.settings_path {
            Some(path) => save_settings_with_library(path, &chosen, &self.library.saved())
                .map_err(|error| error.to_string()),
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

    /// The window "About RawStride" (ADR 0031 RL-Q9, T225): the version, the commit and the
    /// licence notice of GPL-3.0 §0. Every text comes from `about_text` (`shell_text.rs`); the
    /// rows are selectable labels that wrap. No ✕ and no key: only Close closes it. Nothing is
    /// opened, read from disk or from the network.
    fn draw_about_window(&mut self, ctx: &egui::Context) {
        /// The window's fixed width in points, so the long sentences wrap (T225).
        const ABOUT_WINDOW_WIDTH: f32 = 440.0;
        if !self.about_window {
            return;
        }
        let mut close = false;
        egui::Window::new(about_text(AboutText::Title))
            .collapsible(false)
            .resizable(false)
            .default_width(ABOUT_WINDOW_WIDTH)
            .max_width(ABOUT_WINDOW_WIDTH)
            .show(ctx, |ui| {
                let label = |ui: &mut egui::Ui, text: AboutText<'_>| {
                    ui.add(egui::Label::new(about_text(text)).wrap());
                };
                label(ui, AboutText::Version { version: VERSION });
                label(ui, AboutText::Commit { commit: COMMIT });
                ui.add_space(PANEL_ROW_SPACING);
                label(ui, AboutText::Copyright { authors: AUTHORS });
                label(ui, AboutText::Licence);
                label(ui, AboutText::Warranty);
                label(ui, AboutText::LicenceFile);
                label(
                    ui,
                    AboutText::Source {
                        repository: REPOSITORY,
                    },
                );
                label(ui, AboutText::ThirdParty);
                ui.add_space(PANEL_ROW_SPACING);
                close = ui.button(about_text(AboutText::Close)).clicked();
            });
        if close {
            self.about_window = false;
        }
    }

    /// The settings window "Inställningar" (ADR 0007 §5.2, §5.4/Q10, T106). Every text comes
    /// from `settings_text`, `fallback_text` or `editor_text` (`shell_text.rs`, `editor.rs`); an
    /// encoding is chosen by its value (T116). The buttons at the top choose a page (ADR 0017
    /// U10b-Q6, T278): General (the IIM check box, the encoding, the theme), External Programs (the
    /// list of programs, ADR 0019 E-Q2, T200) and Metadata Window (the sections and items of the
    /// metadata window, shown or hidden and in their order, in a scroll area, then "Restore
    /// Defaults"). Save and Cancel are under every page; like every choice, the programs and the
    /// layout are applied and saved only by "Save".
    fn draw_settings_window(&mut self, ctx: &egui::Context) {
        /// The window's fixed width in points, so a long program path wraps (T200).
        const SETTINGS_WINDOW_WIDTH: f32 = 440.0;
        /// The greatest height in points of the page Metadata Window's scroll area (T278); never
        /// more than half the program's window, so the Settings window stays on the screen.
        const LAYOUT_PAGE_MAX_HEIGHT: f32 = 420.0;
        let Some(mut chosen) = self.settings_window.clone() else {
            return;
        };
        let mut page = self.settings_page;
        let mut save = false;
        let mut cancel = false;
        // The clicks of the program rows and of "Add…", applied after the rows are drawn.
        let mut move_up = None;
        let mut remove = None;
        let mut add_program = false;
        // The clicks of the page Metadata Window, applied after its rows are drawn (T278).
        let mut layout_clicks: Vec<LayoutClick> = Vec::new();
        let mut restore_defaults = false;
        let layout_height = LAYOUT_PAGE_MAX_HEIGHT.min(ctx.content_rect().height() / 2.0);
        egui::Window::new(settings_text(SettingsText::Title))
            .collapsible(false)
            .resizable(false)
            .default_width(SETTINGS_WINDOW_WIDTH)
            .max_width(SETTINGS_WINDOW_WIDTH)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for each in SettingsPage::ALL {
                        let text = settings_text(SettingsText::Page(each));
                        if ui.selectable_label(page == each, text).clicked() {
                            page = each;
                        }
                    }
                });
                ui.separator();
                match page {
                    SettingsPage::General => {
                        ui.checkbox(
                            &mut chosen.create_iim,
                            settings_text(SettingsText::CreateIim),
                        );
                        ui.label(
                            egui::RichText::new(settings_text(SettingsText::CreateIimNote)).small(),
                        );
                        ui.add_space(PANEL_ROW_SPACING);
                        ui.label(settings_text(SettingsText::Encoding));
                        egui::ComboBox::from_id_salt("fallback-encoding")
                            .selected_text(fallback_text(chosen.fallback))
                            .show_ui(ui, |ui| {
                                for fallback in FALLBACK_CHOICES {
                                    ui.selectable_value(
                                        &mut chosen.fallback,
                                        fallback,
                                        fallback_text(fallback),
                                    );
                                }
                            });
                        ui.add_space(PANEL_ROW_SPACING);
                        ui.label(shell_text(ShellText::ThemeLabel));
                        egui::ComboBox::from_id_salt("theme")
                            .selected_text(theme_text(chosen.shell.theme))
                            .show_ui(ui, |ui| {
                                for theme in THEMES {
                                    ui.selectable_value(
                                        &mut chosen.shell.theme,
                                        theme,
                                        theme_text(theme),
                                    );
                                }
                            });
                    }
                    SettingsPage::ExternalPrograms => {
                        if chosen.external_programs.is_empty() {
                            ui.label(settings_text(SettingsText::NoPrograms));
                        }
                        for (index, program) in chosen.external_programs.iter_mut().enumerate() {
                            ui.push_id(index, |ui| {
                                ui.horizontal(|ui| {
                                    ui.text_edit_singleline(&mut program.name);
                                    let up = egui::Button::new(settings_text(SettingsText::MoveUp));
                                    if ui.add_enabled(index > 0, up).clicked() {
                                        move_up = Some(index);
                                    }
                                    if ui
                                        .button(settings_text(SettingsText::RemoveProgram))
                                        .clicked()
                                    {
                                        remove = Some(index);
                                    }
                                });
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(program.path.display().to_string())
                                            .small(),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                        let room = chosen.external_programs.len() < MAX_EXTERNAL_PROGRAMS;
                        add_program = ui
                            .add_enabled(
                                room,
                                egui::Button::new(settings_text(SettingsText::AddProgram)),
                            )
                            .on_disabled_hover_text(external_program_limit_text(
                                MAX_EXTERNAL_PROGRAMS,
                            ))
                            .clicked();
                    }
                    SettingsPage::MetadataWindow => {
                        egui::ScrollArea::vertical()
                            .id_salt("settings-metadata-window")
                            .max_height(layout_height)
                            .show(ui, |ui| {
                                layout_clicks = draw_layout_rows(ui, &chosen.editor_layout);
                            });
                        ui.add_space(PANEL_ROW_SPACING);
                        restore_defaults = ui
                            .button(settings_text(SettingsText::RestoreDefaults))
                            .clicked();
                    }
                }
                ui.add_space(PANEL_ROW_SPACING);
                ui.horizontal(|ui| {
                    save = ui.button(settings_text(SettingsText::Save)).clicked();
                    cancel = ui.button(settings_text(SettingsText::Cancel)).clicked();
                });
            });
        self.settings_page = page;
        // Only the layout; the sections' open or collapsed state (the `editor_*` keys) stays.
        if restore_defaults {
            chosen.editor_layout = EditorLayout::default();
        }
        for click in layout_clicks {
            let layout = &mut chosen.editor_layout;
            match click {
                LayoutClick::SectionShown(section, shown) => {
                    layout.set_section_shown(section, shown);
                }
                LayoutClick::SectionUp(section) => layout.move_section_up(section),
                LayoutClick::ItemShown(item, shown) => layout.set_item_shown(item, shown),
                LayoutClick::ItemUp(item) => layout.move_item_up(item),
            }
        }
        let programs = &mut chosen.external_programs;
        if let Some(index) = move_up
            && let Some(above) = index.checked_sub(1)
            && index < programs.len()
        {
            programs.swap(above, index);
        }
        if let Some(index) = remove
            && index < programs.len()
        {
            programs.remove(index);
        }
        if add_program
            && programs.len() < MAX_EXTERNAL_PROGRAMS
            && let Some(path) = pick_program()
        {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            programs.push(ExternalProgram { name, path });
        }
        if save {
            // A name typed empty or with a `|` never reaches the file (T200).
            chosen.external_programs = tidied_programs(&chosen.external_programs);
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

    /// A right-click on the cell of file `index`, before its menu opens (`Tab::right_click`,
    /// ADR 0019 §2.9, E-Q16): the image becomes current, the marking is kept.
    fn right_click(&mut self, index: usize) {
        let ready = self.ratings_ready();
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        if let Some(tab) = self.tabs.active_mut() {
            tab.right_click(index, ready, &rating_of);
        }
    }

    /// A click on a button on the thumbnail of file `index` in the contact sheet (ADR 0017 §2.8,
    /// U15-Q4, T245). The image becomes current with the marking kept, as at a right click; its
    /// refilter can hide or unmark images, so the rest reads the tab as it is after it and acts
    /// only if the image is then current and shown. The rotations act as their keys do (on the
    /// marked when the image is marked); Preview opens with "only the marked" on when the image
    /// is marked and two or more are, else off, and the choice stays; Metadata… opens the window
    /// on the clicked image, also when it is not marked and others are. The marking is never
    /// changed.
    fn thumb_button_click(&mut self, ctx: &egui::Context, index: usize, button: ThumbButton) {
        self.right_click(index);
        let Some(tab) = self.tabs.active() else {
            return;
        };
        if tab.current != index || tab.position().is_none() {
            return;
        }
        let marked = tab.selection.marked_in(&tab.visible);
        let image_marked = marked.contains(&index);
        let file = tab.files.get(index).cloned();
        match button {
            ThumbButton::RotateLeft | ThumbButton::RotateRight => {
                self.run_command(ctx, button.command());
            }
            ThumbButton::Preview => {
                self.settings.shell.filmstrip_marked =
                    preview_only_marked(marked.len(), image_marked);
                self.switch_mode(ViewMode::Loupe);
            }
            ThumbButton::Metadata => {
                let Some(file) = file else {
                    return;
                };
                // Resolved as `metadata_subject` resolves it: the cache keys by resolved path.
                let path = match std::env::current_dir() {
                    Ok(cwd) => resolve_path(&cwd, &file),
                    Err(_) => file,
                };
                self.open_metadata_window_on(path);
            }
        }
    }

    /// A step in the preview over the visible list, or between the marked images with "only the
    /// marked" (`Tab::preview_move`, ADR 0014 §4.4 way C, ADR 0017 §2.2, T111); the marking is
    /// kept. The core follows the tab: `set_files` when the visible list changed, so the views of
    /// images still in the list are kept, otherwise `go`. With nothing visible, back to the grid
    /// (§2.3).
    fn loupe_move(&mut self, to: LoupeMove) {
        let ready = self.ratings_ready();
        let only_marked = self.settings.shell.filmstrip_marked;
        let cwd = std::env::current_dir().ok();
        let (changed, position) = {
            let rating_of = tab_rating(&self.metadata, cwd.as_deref());
            let Some(tab) = self.tabs.active_mut() else {
                return;
            };
            let before = tab.visible.clone();
            tab.preview_move(to, only_marked, ready, &rating_of);
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
    /// filter when the read fails. A preview left with nothing to show once the ratings are ready
    /// goes back to the contact sheet also when the list did not change (empty while they were
    /// read, empty after: a filter no file matches after a reload, ADR 0014 §2.3, T242 point 9.1).
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
            if self.refilter_active(None) || preview_must_leave(self.tabs.active(), ready) {
                self.loupe_follows_visible();
            }
        }
        // A tab an import made active before its ratings were read: the refilter above may have
        // moved its current image or view without the user, so its position is kept again
        // (ADR 0026 F-Q10, T262).
        if ready
            && let Some(folder) = self.tabs.active().and_then(|tab| tab.folder.clone())
            && self
                .import_tabs
                .as_ref()
                .is_some_and(|import| import.settling_in(&folder))
            && let Some(position) = self.import_position()
            && let Some(import) = self.import_tabs.as_mut()
        {
            import.shown(position, false);
        }
    }

    /// A sort chosen in the command bar or View ▸ Sort By (ADR 0017 U14, T138): remembered in
    /// `settings.txt` for every tab (U14-Q3) and applied at once to every tab whose keys are read
    /// (U14-Q5). Choosing the order the tabs are in already applies it again. No widget keeps the
    /// keyboard focus, as after a filter.
    fn set_sort(&mut self, ctx: &egui::Context, order: SortOrder) {
        if self.settings.shell.sort == order {
            self.tabs.forget_sort();
        } else {
            self.settings.shell.sort = order;
            if let Some(path) = &self.settings_path
                && let Err(error) =
                    save_settings_with_library(path, &self.settings, &self.library.saved())
            {
                eprintln!("pw-loupe: the sort order could not be saved: {error}");
            }
        }
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        self.follow_sort(ctx);
    }

    /// The sort's follow-up of a frame (U14-Q4): takes the capture times that came back, asks
    /// for the active tab's missing ones when sorting by capture time, and applies the order to
    /// every tab whose keys are all read. A frame is asked for while reads are on their way.
    fn follow_sort(&mut self, ctx: &egui::Context) {
        while let Some(read) = self.capture_reader.try_recv() {
            self.capture_in_flight.remove(&read.path);
            self.tabs.record_capture(&read.path, read.time);
        }
        if self.settings.shell.sort.key == SortKey::Capture {
            self.request_capture_times();
        }
        match self.sort_tabs(self.ratings_ready()) {
            SortChange::Unchanged => {}
            SortChange::Reordered => self.active_reordered(),
            // The contact sheet keeps its top image (ADR 0026 §2 point 5, T242).
            SortChange::Placed => {
                self.anchor_from_note();
                self.active_changed(false);
            }
        }
        if !self.capture_in_flight.is_empty() {
            ctx.request_repaint_after(CAPTURE_POLL_INTERVAL);
        }
    }

    /// Asks the reader for the capture times `capture_requests` names for the active tab. When
    /// the reader has stopped, a file counts as having no date, so the sort still completes.
    fn request_capture_times(&mut self) {
        let Some(tab) = self.tabs.active() else {
            return;
        };
        let wanted = capture_requests(tab, self.settings.shell.sort, &self.capture_in_flight);
        for path in wanted {
            if self.capture_reader.request(path.clone()) {
                self.capture_in_flight.insert(path);
            } else {
                self.tabs.record_capture(&path, None);
            }
        }
    }

    /// `Tabs::follow_sort` with the chosen order, the shown ratings and the files' modification
    /// times. Returns what happened to the active tab: a file changed place by a new order, or by
    /// a placement of waiting files (T242), or neither.
    fn sort_tabs(&mut self, ratings_ready: bool) -> SortChange {
        let order = self.settings.shell.sort;
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        match self
            .tabs
            .follow_sort(order, ratings_ready, &rating_of, &modified_time)
        {
            SortFollow::Reordered { moved: true } => SortChange::Reordered,
            SortFollow::Placed { moved: true } => SortChange::Placed,
            _ => SortChange::Unchanged,
        }
    }

    /// After the active tab was reordered (U14-Q6), or its files changed by a move, an undo or a
    /// delete: the thumbnails already made follow their files to their new places and only the
    /// others are asked for (`active_changed`, ADR 0026 §3.5, T257), a shown preview gets the new
    /// list through `set_files` (its decoded images are kept, as after a filter), and the contact
    /// sheet scrolls to the current image.
    fn active_reordered(&mut self) {
        self.active_changed(true);
    }

    /// `active_reordered`; with `scroll` false (a placement of waiting files, ADR 0026 §2
    /// point 5, T242, or a listing that found files come or go, T251) the contact sheet does not
    /// scroll to the current image but keeps its top image. A preview that returned to the
    /// contact sheet still scrolls there.
    ///
    /// The thumbnails already made are kept (ADR 0026 §3.5, F4a, T257): a new generation is given
    /// to the worker without emptying its cache (`set_generation_keeping_cache`), the grid's
    /// ready, failed and no-preview cells and the textures move with their files
    /// (`ThumbGrid::carried`, `carried_map`), and a requested or new cell is asked for again
    /// (answered from the kept cache when it has the image). A file gone from the tab is
    /// forgotten in the cache, so a later file of the same name is not shown with the old image.
    /// The added files' stamps are kept (T254): the stamp taken before a file's first request
    /// stays the one to compare with, also when it is answered from the kept cache.
    fn active_changed(&mut self, scroll: bool) {
        let files = self
            .tabs
            .active()
            .map(|tab| tab.files.clone())
            .unwrap_or_default();
        self.thumb_generation = self.thumb_generation.wrapping_add(1);
        self.thumb_worker
            .set_generation_keeping_cache(self.thumb_generation);
        let (grid, moves) =
            self.thumb_grid
                .carried(self.thumb_generation, &self.thumb_files, &files);
        self.thumb_grid = grid;
        self.thumb_textures = carried_map(std::mem::take(&mut self.thumb_textures), &moves);
        for gone in files_gone(&self.thumb_files, &files) {
            self.thumb_worker.forget_path(&gone);
        }
        self.thumb_files = files;
        if self.tabs.active().map(|tab| tab.mode) == Some(ViewMode::Loupe) {
            if let Some(position) = self.loupe_follows_visible() {
                self.scroll_request = Some(position);
            }
        } else if scroll {
            self.scroll_request = self.tabs.active().and_then(Tab::position);
        }
    }

    /// Turns the last frame's note of the contact sheet into the pending anchor (ADR 0026 §2
    /// point 5, T242): before a relist or a placement of the active tab, when the note is that
    /// tab's, the tab shows the contact sheet and the note names images. Otherwise a pending
    /// anchor stays as it is (a note made while nothing was shown has no image to keep).
    fn anchor_from_note(&mut self) {
        let Some(tab) = self.tabs.active() else {
            return;
        };
        if tab.mode != ViewMode::Grid {
            return;
        }
        if let Some(note) = &self.grid_note
            && note.tab == self.tabs.active_index()
            && note.folder == tab.folder
            && !note.from_top.is_empty()
        {
            self.grid_anchor = Some(note.clone());
        }
    }

    /// The status bar's part while the active tab waits for its capture times (U14-Q4).
    fn sort_status(&self) -> Option<String> {
        let order = self.settings.shell.sort;
        let tab = self.tabs.active()?;
        if order.key != SortKey::Capture || tab.sorted == Some(order) || tab.files.is_empty() {
            return None;
        }
        Some(sorting_text(
            tab.capture.read_count(&tab.files),
            tab.files.len(),
        ))
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
        self.import_dialog = Some(ImportDialog::new(self.last_form.clone()));
        self.import_submit_error = None;
        let mut card_refresh = CardRefresh::new(Instant::now());
        if let Err(error) = self.import_worker.cards() {
            // Counted as answered, so the timer asks again after the interval (T161).
            card_refresh.answered();
            self.import_submit_error = Some(submit_error_text(error));
        }
        self.card_refresh = Some(card_refresh);
    }

    fn close_import(&mut self) {
        if let Some(dialog) = self.import_dialog.take() {
            self.last_form = dialog.form().clone();
            // "Copy new images only" is remembered in the settings (T158).
            self.settings.shell.import_incremental = self.last_form.incremental;
        }
        self.import_hidden = false;
        self.import_clear_confirm = false;
        self.card_refresh = None;
    }

    /// The start button or Enter (D14): with fields in Clear the window "Rensa fält?" asks first,
    /// once for this import; otherwise the import starts at once. A run that only marks the card
    /// as imported (T166) copies nothing and writes no IPTC field, so it never asks.
    fn request_import_start(&mut self) {
        let Some(dialog) = self.import_dialog.as_ref() else {
            return;
        };
        if !dialog.can_start() {
            return;
        }
        if dialog.marks_only() || dialog.form().iptc.cleared_fields().is_empty() {
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
        let (source, destination) = (form.source.clone(), form.dest.clone());
        let active_cleared = form.eject && self.tabs.clear_inside(Path::new(form.source.trim()));
        if active_cleared {
            self.load_active_tab();
        }
        if form.eject {
            // The drive the worker ejects, from the source as written (T248): its watches end
            // before the import starts, so they do not keep the card busy.
            self.ejecting_drive = pw_ingest::drive_of(Path::new(form.source.trim()));
            self.follow_tab_folders();
            if !self.folder_watcher.wait_until_applied(EJECT_WATCH_WAIT) {
                eprintln!(
                    "pw-loupe: the watches on the drive to eject did not end within {} s",
                    EJECT_WATCH_WAIT.as_secs()
                );
            }
        }
        match self.import_worker.run(form, self.settings.edit_options()) {
            Ok(id) => {
                if let Some(dialog) = &mut self.import_dialog {
                    dialog.sent(id);
                }
                self.tasks.import_started(id, &source, &destination);
                // A tab inside the destination is not listed from disk while the import runs
                // (`disk_listing_held`, T251).
                let written = destination.trim();
                self.import_destination = (!written.is_empty()).then(|| PathBuf::from(written));
                self.import_submit_error = None;
            }
            Err(error) => {
                self.tabs = previous_tabs;
                self.ejecting_drive = None;
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

    /// Gives the folder watcher the tabs' folders when they differ from the last set given
    /// (ADR 0026 §3.2, T248), leaving out the drive of an import that will eject it. Does not
    /// wait.
    fn follow_tab_folders(&mut self) {
        let wanted = self.tabs.folders_to_watch(self.ejecting_drive.as_deref());
        if wanted != self.watched_folders {
            self.folder_watcher.set_folders(wanted.clone());
            self.watched_folders = wanted;
        }
    }

    /// Notes the folder watcher's events on the tabs they concern (T248): `Changed` and `Started`
    /// set `changed_on_disk`, `Ended` sets `unwatched`. The active tab acts on them in
    /// `follow_disk_changes`, another tab when it becomes active (`load_activated_tab`, ADR 0026
    /// F3, T251).
    fn poll_folder_watcher(&mut self) {
        while let Some(event) = self.folder_watcher.try_recv() {
            match event {
                // The watch started after the tab's listing: a file may have come in between
                // (T249), so it counts as a change.
                WatchEvent::Started(folder) | WatchEvent::Changed(folder) => {
                    self.tabs.note_changed_on_disk(&folder)
                }
                WatchEvent::Ended { folder, reason } => {
                    self.tabs.note_unwatched(&folder);
                    eprintln!(
                        "pw-loupe: the folder {} is not watched: {reason}",
                        folder.display()
                    );
                }
            }
        }
    }

    /// `disk_listing_held` with the program's state (T251).
    fn disk_listing_held(&self, folder: &Path) -> bool {
        disk_listing_held(
            self.moves.busy(),
            self.delete.busy(),
            self.dialog_open(),
            self.import_destination.as_deref(),
            folder,
        )
    }

    /// Lists the active tab's folder from disk when a watch noted a change (`changed_on_disk`)
    /// and the listing is not held (`disk_listing_held`), and applies it (`list_into_tab`, the
    /// files RawStride is writing counting as listed); the note is cleared. Used by
    /// `follow_disk_changes` (the active tab follows at once, with the real `ratings_ready`) and
    /// by `load_activated_tab` (a tab that becomes active, with `ratings_ready` false, since the
    /// value there is still the previous tab's) (ADR 0026 F3, F-Q8, T251). `None` when nothing
    /// was listed; `Err` with `folder_gone_text` when the listing failed (the tab unchanged).
    fn list_active_from_disk(&mut self, ratings_ready: bool) -> Option<Result<Relisted, String>> {
        let folder = self
            .tabs
            .active()
            .filter(|tab| tab.changed_on_disk)
            .and_then(|tab| tab.folder.clone())?;
        if self.disk_listing_held(&folder) {
            return None;
        }
        let order = self.settings.shell.sort;
        let cwd = std::env::current_dir().ok();
        let listing = list_folder(&folder).map(|(files, _)| files);
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        let pending = tab_pending(&self.metadata, cwd.as_deref());
        let tab = self.tabs.active_mut()?;
        tab.changed_on_disk = false;
        Some(list_into_tab(
            tab,
            listing,
            &pending,
            order,
            ratings_ready,
            &rating_of,
            &modified_time,
        ))
    }

    /// The active tab follows its folder on disk at once (ADR 0026 §3.2, §3.3, F3, T251): when a
    /// watch noted a change and the listing is not held, the folder is listed on the UI thread,
    /// as Reload Folder does. A listing that fails keeps the tab and says `folder_gone_text`
    /// (F-Q4). When nothing came or went, nothing more happens. Otherwise: the contact sheet's
    /// note becomes the anchor; the removed files are forgotten in the metadata cache
    /// (`forget_deleted`), so a file that comes back under the same name is read anew; the new
    /// files' ratings are read on their own (`add_files`) and only those sent wait as `unread`;
    /// the tab is refiltered, so under a filter the new files wait for their rating; the
    /// thumbnails already made follow their files and only the new files' are asked for, with
    /// the contact sheet keeping its top image (`active_changed(false)`, T257); the sort is
    /// followed; and the status line says what changed (`disk_change_text`), unless a step above
    /// failed, whose error it shows instead. The Exif summary and the histograms are kept, and no
    /// file already shown is read again (F-Q7).
    ///
    /// The files another program added are kept in `added_files` (ADR 0026 §3.2, the addition to
    /// F-Q7, T254), and at each listing only the open tabs' files stay kept. At a listing where
    /// nothing came or went, each kept file of the active tab whose size or write time differs
    /// from the stamp taken before its thumbnail was asked for has its thumbnail made anew
    /// (`made_anew_if_stale`); one still being made is marked and checked when it comes back
    /// (`poll_thumbnails`). So a file still being written is read again at most once per listing.
    /// RawStride's own writes do not count (T255): a file it is writing is skipped, and its stamp
    /// is renewed when the write answers.
    fn follow_disk_changes(&mut self, ctx: &egui::Context) {
        let ready = self.ratings_ready();
        let relisted = match self.list_active_from_disk(ready) {
            None => return,
            Some(Err(text)) => {
                self.message = Some(text);
                return;
            }
            Some(Ok(relisted)) => relisted,
        };
        if !relisted.changed() {
            self.retain_added_files();
            self.made_anew_if_stale(ctx);
            return;
        }
        self.anchor_from_note();
        let error = self.follow_relisted_active(&relisted);
        self.added_files.add(&relisted.new);
        self.retain_added_files();
        self.follow_sort(ctx);
        self.message = error.or_else(|| disk_change_text(&relisted));
    }

    /// The active tab follows a relist that changed its files (`follow_disk_changes`, T251, and
    /// an import's `Placed`, `import_placed`, ADR 0026 §3.4, T260): the removed files are
    /// forgotten in the metadata cache (`forget_deleted`), the new files' ratings are read on
    /// their own (`add_files`) and only those sent wait as `unread`, the tab is refiltered, and the
    /// thumbnails already made follow their files with the contact sheet keeping its top image
    /// (`active_changed(false)`, T257). The anchor, `added_files`, the sort and the status line are
    /// the caller's. Returns the error of a step that failed (the later steps still run).
    fn follow_relisted_active(&mut self, relisted: &Relisted) -> Option<String> {
        let mut error = None;
        if !relisted.gone.is_empty() {
            match resolve_paths(&relisted.gone) {
                Ok(gone) => self.metadata.forget_deleted(&gone),
                Err(text) => error = Some(text),
            }
        }
        if !relisted.new.is_empty() {
            let added = resolve_paths(&relisted.new).and_then(|resolved| {
                let added: HashSet<PathBuf> = self
                    .metadata
                    .add_files(resolved.clone(), &mut self.metadata_worker)?
                    .into_iter()
                    .collect();
                Ok(relisted
                    .new
                    .iter()
                    .zip(&resolved)
                    .filter(|(_, path)| added.contains(*path))
                    .map(|(file, _)| file.clone())
                    .collect::<Vec<PathBuf>>())
            });
            match added {
                Ok(sent) => {
                    if let Some(tab) = self.tabs.active_mut() {
                        tab.mark_unread(&sent);
                    }
                }
                Err(text) => error = Some(text),
            }
        }
        self.refilter_active(None);
        // The thumbnail cache is kept, so the added files keep their stamps (T254, T257).
        self.active_changed(false);
        error
    }

    /// Keeps in `added_files` only the paths some open tab's `files` holds, so files gone or in
    /// closed tabs are dropped (T254).
    fn retain_added_files(&mut self) {
        let tabs = self.tabs.tabs();
        self.added_files.retain(|path| {
            tabs.iter()
                .any(|tab| tab.files.iter().any(|file| file == path))
        });
    }

    /// At a listing where nothing came or went (T254): each kept file of the active tab whose
    /// stamp differs from the one taken before its thumbnail was asked for has its thumbnail made
    /// anew (the cached ones forgotten, the cell set to missing, its texture kept until the new
    /// one replaces it, as after a turn); a cell still requested (its read may have begun before
    /// the last write) is marked, and checked when it comes back (`poll_thumbnails`). Kept files
    /// of other tabs are not read. RawStride's own writes do not count (T255): a kept file
    /// RawStride is writing (`tab_pending`) is skipped, neither compared nor marked; its stamp is
    /// renewed when the write answers (`draw`), and the next listing compares against that.
    fn made_anew_if_stale(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tabs.active() else {
            return;
        };
        let cwd = std::env::current_dir().ok();
        let pending = tab_pending(&self.metadata, cwd.as_deref());
        let kept: Vec<(usize, PathBuf)> = self
            .added_files
            .paths()
            .into_iter()
            .filter_map(|path| {
                if pending(&path) {
                    return None;
                }
                let index = tab.files.iter().position(|file| *file == path)?;
                Some((index, path))
            })
            .collect();
        let mut any = false;
        for (index, path) in kept {
            if !self.added_files.is_stale(&path, file_stamp(&path)) {
                continue;
            }
            if self.thumb_grid.state(index) == CellState::Requested {
                self.added_files.mark_check(&path);
            } else {
                self.thumb_worker.forget_path(&path);
                self.thumb_grid.forget(index);
                self.added_files.made_anew(&path);
                any = true;
            }
        }
        if any {
            ctx.request_repaint();
        }
    }

    /// The new files of the active tab are shown when their ratings are read (ADR 0026 §2
    /// points 3 and 4, T251): `Tab::clear_read` with `MetadataCache::rating_known`, and when a
    /// file left `unread`, the tab is refiltered and a shown preview follows its visible list.
    /// Their placement under a rating order follows in `follow_sort`.
    fn follow_unread(&mut self) {
        let cwd = std::env::current_dir().ok();
        let metadata = &self.metadata;
        let Some(tab) = self.tabs.active_mut() else {
            return;
        };
        if tab.unread.is_empty() {
            return;
        }
        let known = |path: &Path| match cwd.as_deref() {
            Some(cwd) if !path.is_absolute() => metadata.rating_known(&resolve_path(cwd, path)),
            _ => metadata.rating_known(path),
        };
        if tab.clear_read(&known) && self.refilter_active(None) {
            self.loupe_follows_visible();
        }
    }

    /// A tab became active through a click on its tab or `tab_action` (ADR 0026 F-Q8, F-Q3,
    /// T251): a tab whose watch could not start or has ended also counts as changed, so it is
    /// listed on each activation; when it changed on disk and the listing is not held, its folder
    /// is listed first (`list_active_from_disk`, with no anchor, no `add_files` and no follow-up
    /// of its own, since `load_tab` reads the whole tab next). The status text (`disk_change_text`
    /// or `folder_gone_text`) is set after the load, which clears the message; when the tab has
    /// no files, the load's "no images" text stays. When held, the note stays and
    /// `follow_disk_changes` lists the tab once the hold ends.
    fn load_activated_tab(&mut self) {
        if let Some(tab) = self.tabs.active_mut()
            && tab.unwatched
        {
            tab.changed_on_disk = true;
        }
        let listed = self.list_active_from_disk(false);
        if !self.load_tab(false) {
            return;
        }
        let text = match listed {
            None => None,
            Some(Err(text)) => Some(text),
            Some(Ok(relisted)) => {
                self.added_files.add(&relisted.new);
                if self.tabs.active().is_some_and(|tab| tab.files.is_empty()) {
                    None
                } else {
                    disk_change_text(&relisted)
                }
            }
        };
        if let Some(text) = text {
            self.message = Some(text);
        }
    }

    fn hidden_import_status(&self) -> Option<String> {
        if !self.import_hidden {
            return None;
        }
        match self.import_dialog.as_ref()?.state() {
            ImportDialogState::Running { progress } => Some(progress.as_ref().map_or_else(
                || hidden_progress_text(0, 0),
                |progress| hidden_progress_text(progress.files_done, progress.files_total),
            )),
            ImportDialogState::Ejecting { source } => Some(ejecting_progress_text(source)),
            ImportDialogState::Done { done } => {
                if let Some(error) = &done.error {
                    Some(error.clone())
                } else {
                    Some(hidden_done_text(done.copied, done.cancelled))
                }
            }
            _ => None,
        }
    }

    fn poll_import_worker(&mut self, ctx: &egui::Context) {
        loop {
            match self.import_worker.try_recv() {
                Ok(ImportEvent::Cards { cards, .. }) => {
                    if let Some(card_refresh) = &mut self.card_refresh {
                        card_refresh.answered();
                    }
                    self.detected_cards = cards;
                }
                Ok(event) => {
                    // The Tasks area's row, and an import's failures in its error list (U5-Q5).
                    self.tasks.apply(&event, ClockTime::now());
                    let refresh_cards = matches!(
                        &event,
                        ImportEvent::Done { done, .. } if done.eject.is_some()
                    );
                    if matches!(event, ImportEvent::Done { .. }) {
                        // The import has ended: the card's drive may be watched again (T248), and
                        // the tabs inside its destination listed (T251).
                        self.ejecting_drive = None;
                        self.import_destination = None;
                    }
                    if let Some(dialog) = &mut self.import_dialog {
                        dialog.apply(&event);
                    }
                    // The destination tabs open on the first image and fill while importing
                    // (ADR 0026 §3.4, T260).
                    match event {
                        ImportEvent::Planned { images, .. } => {
                            self.import_tabs = Some(ImportTabs::new(images));
                        }
                        ImportEvent::Placed { images, .. } => self.import_placed(ctx, &images),
                        ImportEvent::Done { .. } => self.import_tabs = None,
                        _ => {}
                    }
                    if refresh_cards {
                        let asked = self.import_worker.cards();
                        if let Some(card_refresh) = &mut self.card_refresh {
                            card_refresh.asked(Instant::now());
                            if asked.is_err() {
                                card_refresh.answered();
                            }
                        }
                        if let Err(error) = asked {
                            self.import_submit_error = Some(submit_error_text(error));
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.ejecting_drive = None;
                    self.import_destination = None;
                    self.import_tabs = None;
                    if let Some(dialog) = &mut self.import_dialog {
                        dialog.disconnected();
                    }
                    self.tasks
                        .import_stopped(import_text(ImportText::StoppedMessage), ClockTime::now());
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

    /// The active tab's position, kept when the import makes a tab active and compared when a
    /// later folder opens, to tell whether the user has moved (ADR 0026 F-Q10, T262): its folder,
    /// its current file (`files[current]`, `None` when it has no files) and its view; `None`
    /// without an active tab with a folder.
    fn import_position(&self) -> Option<ImportPosition> {
        let tab = self.tabs.active()?;
        Some(ImportPosition {
            folder: tab.folder.clone()?,
            current: tab.files.get(tab.current).cloned(),
            mode: tab.mode,
        })
    }

    /// An import's `Placed` event (ADR 0026 §3.4, F-Q10, T260, T262): nothing without
    /// `import_tabs`. Otherwise `ImportTabs::place` groups the images by folder, and each group, in
    /// order, goes to its folder's tabs (`placed_tabs`): a folder with tabs has them filled
    /// (`fill_import_tabs`); a folder without one is opened at its first group
    /// (`open_import_folder`), as the active tab when it is made active and otherwise behind it; a
    /// folder whose tab the user closed during the import is not opened again. A group is made
    /// active when it is its folder's first and either it is the import's first shown folder (no
    /// position kept yet and the event's first group) or the user has not moved since the import
    /// last made a tab active (`ImportTabs::unmoved` with the active tab's position taken before
    /// the group's fill or open). A filled folder made active whose tabs do not include the active
    /// one has its first tab activated and read as a click on it does (`load_activated_tab`), its
    /// view kept. After a group made active, the active tab's position is kept
    /// (`ImportTabs::shown`), settling while its ratings are not read yet
    /// (`refilter_when_ratings_change` keeps it again once they are). So two new folders in a row
    /// both become active in turn when the user has not moved, and the last one stays active. The
    /// import dialog is not hidden.
    fn import_placed(&mut self, ctx: &egui::Context, images: &[PathBuf]) {
        let Some(import) = self.import_tabs.as_mut() else {
            return;
        };
        let first_pending = !import.position_kept();
        let groups = import.place(images);
        for (index, group) in groups.iter().enumerate() {
            let now = self.import_position();
            let unmoved = self
                .import_tabs
                .as_ref()
                .is_some_and(|import| import.unmoved(now.as_ref()));
            let make_active = group.first && ((first_pending && index == 0) || unmoved);
            let has_tab = self.tabs.index_of_folder(&group.folder).is_some();
            match placed_tabs(has_tab, group.first, make_active) {
                PlacedTabs::Fill => {
                    self.fill_import_tabs(ctx, group);
                    if make_active
                        && !self.active_tab_of(&group.folder)
                        && let Some(index) = self.tabs.index_of_folder(&group.folder)
                    {
                        self.tabs.activate(index);
                        self.load_activated_tab();
                    }
                }
                PlacedTabs::OpenActive => self.open_import_folder(group, true),
                PlacedTabs::OpenBehind => self.open_import_folder(group, false),
                PlacedTabs::Closed => continue,
            }
            if make_active && let Some(position) = self.import_position() {
                let settling = !self.ratings_ready();
                if let Some(import) = self.import_tabs.as_mut() {
                    import.shown(position, settling);
                }
            }
        }
    }

    /// Whether the active tab's folder is `folder` (`same_folder`).
    fn active_tab_of(&self, folder: &Path) -> bool {
        self.tabs
            .active()
            .and_then(|tab| tab.folder.as_deref())
            .is_some_and(|open| same_folder(open, folder))
    }

    /// The tabs of a placed group's folder get its images (ADR 0026 §3.4, T260): each tab's files
    /// become `with_placed` of its own folder, files and the group's images, through `relist_tab`
    /// (no file made current; the chosen order, the files RawStride is writing counting as
    /// listed). The active tab gets the real `ratings_ready` and is followed as a listing's new
    /// files are (`follow_relisted_active`, with the contact sheet's note as the anchor before
    /// the relist, and the sort followed); an error is shown in the status line, but no text says
    /// what came, and the files are not kept in `added_files` (the import put them in place
    /// whole). Every other tab gets `ratings_ready` false and is only relisted; it is read in full
    /// when it becomes active (`load_tab`).
    fn fill_import_tabs(&mut self, ctx: &egui::Context, group: &PlacedInFolder) {
        let order = self.settings.shell.sort;
        let cwd = std::env::current_dir().ok();
        let active = self.active_tab_of(&group.folder);
        if active {
            self.anchor_from_note();
        }
        let ready = self.ratings_ready();
        let relisted = {
            let rating_of = tab_rating(&self.metadata, cwd.as_deref());
            let pending = tab_pending(&self.metadata, cwd.as_deref());
            let relisted = if active {
                self.tabs.active_mut().and_then(|tab| {
                    let folder = tab.folder.clone()?;
                    let listed = with_placed(&folder, &tab.files, &group.images);
                    Some(relist_tab(
                        tab,
                        listed,
                        None,
                        &pending,
                        order,
                        ready,
                        &rating_of,
                        &modified_time,
                    ))
                })
            } else {
                None
            };
            // The other tabs of the folder; the active one, relisted above, has every image now,
            // so its listing here is its own files and changes nothing.
            for tab in self.tabs.tabs_of_folder_mut(&group.folder) {
                let Some(folder) = tab.folder.clone() else {
                    continue;
                };
                let listed = with_placed(&folder, &tab.files, &group.images);
                relist_tab(
                    tab,
                    listed,
                    None,
                    &pending,
                    order,
                    false,
                    &rating_of,
                    &modified_time,
                );
            }
            relisted
        };
        let Some(relisted) = relisted.filter(Relisted::changed) else {
            return;
        };
        let error = self.follow_relisted_active(&relisted);
        self.follow_sort(ctx);
        if let Some(error) = error {
            self.message = Some(error);
        }
    }

    /// Opens the folder of a placed group that has no tab, at the folder's first group (ADR 0026
    /// §3.4, GR12, T260): its files are its listing without the images the import has not shown
    /// yet (`leave_out_waiting`; files of earlier imports stay) with the group's images added
    /// (`with_placed`), or the group's images alone when the listing fails. The folder, in the
    /// import's form, becomes the first recent folder and the library is saved, as `open` does.
    /// With `active` (the group is made active, F-Q10) the tab is opened with `Tabs::open_new`,
    /// the first file current, shown as the contact sheet (GR13) and loaded; otherwise it is
    /// opened behind the active tab (`Tabs::open_behind`), the active tab unchanged and nothing
    /// loaded.
    fn open_import_folder(&mut self, group: &PlacedInFolder, active: bool) {
        let folder = group.folder.clone();
        let listing = match list_folder(&folder) {
            Ok((files, _)) => leave_out_waiting(self.import_tabs.as_ref(), files),
            Err(_) => Vec::new(),
        };
        let files = with_placed(&folder, &listing, &group.images);
        self.library
            .recent
            .record(std::path::absolute(&folder).unwrap_or_else(|_| folder.clone()));
        if active {
            self.tabs.open_new(folder, files, 0);
            self.tabs.set_mode(ViewMode::Grid);
            self.load_active_tab();
        } else {
            self.tabs.open_behind(folder, files);
        }
        self.save_library();
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
        let shown_preview = dialog.shown_preview().cloned();
        let preview_is_current = dialog.preview_is_current();
        let start_text = if dialog.marks_only() {
            ImportText::MarkCard
        } else {
            ImportText::Start
        };
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

        egui::Window::new(import_text(ImportText::Title))
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!running && !confirming, |ui| {
                    ui.label(import_text(ImportText::Source));
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut form.source);
                        if ui.button(import_text(ImportText::Choose)).clicked()
                            && let Some(path) = pick_folder(
                                import_text(ImportText::ChooseSource),
                                &form.source,
                                cards.first(),
                            )
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
                    ui.label(import_text(ImportText::Destination));
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut form.dest);
                        if ui.button(import_text(ImportText::Choose)).clicked()
                            && let Some(path) = pick_folder(
                                import_text(ImportText::ChooseDestination),
                                &form.dest,
                                None,
                            )
                        {
                            form.dest = path.to_string_lossy().into_owned();
                        }
                    });
                    if let Some(message) = field_error(&state, Field::Dest) {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut form.backup_on, import_text(ImportText::Backup));
                        ui.add_enabled_ui(form.backup_on, |ui| {
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut form.backup);
                                if ui.button(import_text(ImportText::Choose)).clicked()
                                    && let Some(path) = pick_folder(
                                        import_text(ImportText::ChooseBackup),
                                        &form.backup,
                                        None,
                                    )
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
                    ui.label(import_text(ImportText::FolderTemplate));
                    ui.text_edit_singleline(&mut form.folder);
                    if let Some(message) = field_error(&state, Field::Folder) {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }

                    ui.add_space(8.0);
                    ui.label(import_text(ImportText::NameTemplate));
                    ui.text_edit_singleline(&mut form.name);
                    if let Some(message) = field_error(&state, Field::Name) {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(import_text(ImportText::Job));
                        ui.text_edit_singleline(&mut form.job);
                        ui.label(import_text(ImportText::FirstNumber));
                        ui.add(egui::DragValue::new(&mut form.first_seq).range(0..=999_999));
                    });
                    ui.label(
                        egui::RichText::new(import_text(ImportText::Variables))
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );

                    ui.collapsing(import_text(ImportText::IptcTemplate), |ui| {
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
                                        edit =
                                            edit.hint_text(import_text(ImportText::CopyrightHint));
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
                            egui::RichText::new(import_text(ImportText::IptcNote))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.checkbox(&mut form.eject, import_text(ImportText::Eject));
                    ui.checkbox(&mut form.incremental, import_text(ImportText::Incremental));

                    ui.horizontal(|ui| {
                        ui.label(import_text(ImportText::Preview));
                        if ui.button(import_text(ImportText::Rescan)).clicked() {
                            rescan = true;
                        }
                        if matches!(state, ImportDialogState::Previewing) {
                            ui.label(import_text(ImportText::Previewing));
                        }
                    });
                    // The last list stays, dimmed, until the next preview comes (T276).
                    if let Some(preview) = &shown_preview {
                        ui.add_enabled_ui(preview_is_current, |ui| {
                            ui.label(preview_summary(preview));
                            if let Some(note) = &preview.register_note {
                                ui.label(note.as_str());
                            }
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
                        });
                    }
                });

                match &state {
                    ImportDialogState::Running { progress } => {
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
                            .unwrap_or_else(|| import_text(ImportText::Importing).to_string());
                        ui.add(egui::ProgressBar::new(fraction).text(text));
                        ui.horizontal(|ui| {
                            if ui.button(import_text(ImportText::Cancel)).clicked() {
                                cancel = true;
                            }
                            if ui.button(import_text(ImportText::Hide)).clicked() {
                                hide = true;
                            }
                        });
                    }
                    ImportDialogState::Ejecting { source } => {
                        ui.add(egui::ProgressBar::new(0.0).text(ejecting_progress_text(source)));
                        if ui.button(import_text(ImportText::Hide)).clicked() {
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
                        if ui.button(import_text(ImportText::NewImport)).clicked() {
                            new_import = true;
                        }
                        if ui.button(import_text(ImportText::Close)).clicked() {
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
                                    egui::Button::new(import_text(start_text)),
                                )
                                .clicked()
                            {
                                start = true;
                            }
                            if ui
                                .add_enabled(
                                    can_close && !confirming,
                                    egui::Button::new(import_text(ImportText::Close)),
                                )
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
        // The card buttons kept up to date by themselves (T161): ask again on a timer while the
        // dialog is idle, so a card put in or taken out shows. An error is not shown here (the
        // user did nothing); a stopped worker is shown by the other paths.
        let idle = !self.import_clear_confirm
            && self.import_dialog.as_ref().is_some_and(|dialog| {
                matches!(
                    dialog.state(),
                    ImportDialogState::Editing { .. } | ImportDialogState::Done { .. }
                )
            });
        let now = Instant::now();
        if self
            .card_refresh
            .as_ref()
            .is_some_and(|card_refresh| card_refresh.due(now, idle))
        {
            let asked = self.import_worker.cards();
            if let Some(card_refresh) = &mut self.card_refresh {
                card_refresh.asked(now);
                if asked.is_err() {
                    card_refresh.answered();
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn open(&mut self, path: &Path, new_tab: bool) {
        match list_folder(path) {
            Ok((files, pos)) => {
                // While an import runs, the images it has not shown yet are left out (T260).
                let (files, pos) =
                    opened_during_import(self.import_tabs.as_ref(), files, pos, !path.is_dir());
                let folder = if path.is_dir() {
                    path.to_path_buf()
                } else {
                    path.parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or(Path::new("."))
                        .to_path_buf()
                };
                // Every folder opened, in any way, is the first recent folder (U4-Q4, T135); a
                // relative one from the command line as its full path.
                self.library
                    .recent
                    .record(std::path::absolute(&folder).unwrap_or_else(|_| folder.clone()));
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
                self.save_library();
            }
            Err(e) => self.message = Some(open_error_text(&e)),
        }
    }

    /// Saves the settings with the library's lists after a change of a list (T135); a failure is
    /// only logged, as a theme change's.
    fn save_library(&self) {
        if let Some(path) = &self.settings_path
            && let Err(error) =
                save_settings_with_library(path, &self.settings, &self.library.saved())
        {
            eprintln!("pw-loupe: the library could not be saved: {error}");
        }
    }

    fn load_active_tab(&mut self) {
        self.load_tab(false);
    }

    /// The body of `load_active_tab` and of "Reload Folder"'s reads (ADR 0026 §3.1, T242): with
    /// `reload` the metadata is read with `begin_reload` (the inspector's metadata always
    /// emptied) and the Exif summary and the histograms are left to the caller, which has called
    /// their `folder_reloaded`. Returns false when the reads could not begin (the error is in the
    /// status line).
    fn load_tab(&mut self, reload: bool) -> bool {
        // A folder opened (or opened again) or another tab: the order is applied now when its keys
        // need nothing read (U14-Q5), so the thumbnails below are asked for in it. The ratings
        // of this tab are not read yet; a rating or capture-time order follows in `follow_sort`.
        self.sort_tabs(false);
        self.thumb_generation = self.thumb_generation.wrapping_add(1);
        self.thumb_worker.set_generation(self.thumb_generation);
        // Every cell is asked for again, so the added files' stamps are taken anew (T254).
        self.added_files.forget_requests();
        let Some(tab) = self.tabs.active().cloned() else {
            self.thumb_grid = ThumbGrid::new(self.thumb_generation, 0);
            self.thumb_files.clear();
            self.thumb_textures.clear();
            return true;
        };
        self.thumb_grid = ThumbGrid::new(self.thumb_generation, tab.files.len());
        self.thumb_files = tab.files.clone();
        self.thumb_textures.clear();
        if tab.mode == ViewMode::Grid {
            self.thumb_worker.resume();
        } else {
            self.thumb_worker.pause();
        }
        let loaded = if reload {
            self.metadata
                .begin_reload(tab.files, &mut self.metadata_worker)
        } else {
            self.metadata
                .begin_load(tab.files, &mut self.metadata_worker)
        };
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
                return false;
            }
        };
        // Another folder empties the inspector's Exif summary, as its metadata (T110), and its
        // kept histograms (T144); a reload has emptied them already.
        if !reload {
            self.exif.folder_loaded(&files);
            self.histograms.folder_loaded(&files);
        }
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
            Some(no_images_text(tab.folder.as_deref()))
        } else {
            None
        };
        true
    }

    /// "Reload Folder" (F5, ADR 0026 §2, §3.1, F1, T242) on the active tab: its folder is listed
    /// again and `reload_tab` applies the listing (the current image, the marks, the filter and
    /// the sort kept, new files placed; the contact sheet keeps its top image through the
    /// anchor). A folder that is gone or unreadable leaves the tab as it was and says so (F-Q4).
    /// Otherwise the Exif summary and the histograms are emptied, every image's rating and
    /// metadata is read again and the thumbnails are made anew (`load_tab` with `reload`; in the
    /// preview the view core opens the current image again from the file), the status line says
    /// a removed current image (F-Q5) unless the reads could not begin, a preview left with
    /// nothing to show goes back to the contact sheet (`preview_must_leave`, point 9.1), and the
    /// sort is followed. A tab without a folder: nothing. The tab's notes of the folder watcher
    /// are cleared (T251): `changed_on_disk`, since the folder is listed now, and `unwatched`, so
    /// its watch is tried again (`follow_tab_folders`; a watch that still cannot start marks it
    /// again). Before the listing, the folder's watch is started anew every time
    /// (`FolderWatcher::watch_again`, T252), whatever the `unwatched` note was, since a watch can
    /// be lost without an `Ended` (a USB stick pulled out and put back) or kept as ended by
    /// another tab of the same folder. A folder the `unwatched` note left out of the watcher's set
    /// is not touched by it; the cleared note puts it back into the next set, which opens a new
    /// watch (when that set is drained with the `Again`, the open it began is replaced once: one
    /// more open, still one `Started`). The new watch's `Started` lists the folder once more
    /// (T249); that second listing is expected.
    fn reload_active_tab(&mut self, ctx: &egui::Context) {
        let Some(folder) = self.tabs.active().and_then(|tab| tab.folder.clone()) else {
            return;
        };
        if let Some(tab) = self.tabs.active_mut() {
            tab.changed_on_disk = false;
            tab.unwatched = false;
        }
        self.folder_watcher.watch_again(&folder);
        // While an import runs, the images it has not shown yet are left out (T260).
        let listing = list_folder(&folder)
            .map(|(files, _)| leave_out_waiting(self.import_tabs.as_ref(), files));
        if listing.is_ok() {
            self.anchor_from_note();
        }
        let order = self.settings.shell.sort;
        let ready = self.ratings_ready();
        let cwd = std::env::current_dir().ok();
        let reloaded = {
            let rating_of = tab_rating(&self.metadata, cwd.as_deref());
            let pending = tab_pending(&self.metadata, cwd.as_deref());
            let Some(tab) = self.tabs.active_mut() else {
                return;
            };
            reload_tab(
                tab,
                listing,
                &pending,
                order,
                ready,
                &rating_of,
                &modified_time,
            )
        };
        let removed = match reloaded {
            Ok(removed) => removed,
            Err(text) => {
                self.message = Some(text);
                return;
            }
        };
        self.exif.folder_reloaded();
        self.histograms.folder_reloaded();
        if self.load_tab(true)
            && let Some(text) = removed
        {
            self.message = Some(text);
        }
        // A preview left with nothing to show (every image removed) goes back to the contact
        // sheet (ADR 0014 §2.3, T242 point 9.1); under a filter it waits for the ratings
        // (`refilter_when_ratings_change`).
        if preview_must_leave(self.tabs.active(), self.ratings_ready())
            && let Some(position) = self.loupe_follows_visible()
        {
            self.scroll_request = Some(position);
        }
        self.follow_sort(ctx);
    }

    /// Switches the active tab between the contact sheet and the preview (the loupe) without
    /// reloading the tab, so the thumbnails and their cache survive a visit to the preview (ADR
    /// 0011 §4.2). The preview opens on the visible images (`open_folder`), on the first marked
    /// image when "only the marked" is on and the current one is not marked; with nothing visible
    /// it does not open. The window follows in `sync_preview_window` (full screen, T111). Returns
    /// the current image's position in the visible list, so the grid can scroll to it.
    fn switch_mode(&mut self, mode: ViewMode) -> Option<usize> {
        let only_marked = self.settings.shell.filmstrip_marked;
        if mode == ViewMode::Loupe
            && let Some(tab) = self.tabs.active_mut()
            && !tab.visible.is_empty()
            && tab.mode != ViewMode::Loupe
        {
            // With "only the marked" the preview opens on a marked image (U3-Q3, T111).
            tab.preview_start(only_marked);
        }
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

    /// Tells the XMP orientations that changed to both view cores (the compare's too) and makes
    /// the active tab's thumbnails of those files again (ADR 0024 §2.4, T232). The cores keep the
    /// values per path, so nothing is sent again on a mode, tab or folder switch; a file of
    /// another tab gets its thumbnail with the value when that tab is shown. The old texture
    /// stays until the new thumbnail replaces it.
    fn follow_orientations(&mut self) {
        let changes = self.metadata.take_orientation_changes();
        if changes.is_empty() {
            return;
        }
        // The tab's files may be relative; the metadata cache keys by resolved path.
        let active: HashMap<PathBuf, usize> = self
            .tabs
            .active()
            .and_then(|tab| resolve_paths(&tab.files).ok())
            .map(|files| {
                files
                    .into_iter()
                    .enumerate()
                    .map(|(index, path)| (path, index))
                    .collect()
            })
            .unwrap_or_default();
        for (path, value) in changes {
            self.core.set_orientation(path.clone(), value);
            self.second_core.set_orientation(path.clone(), value);
            if let Some(&index) = active.get(&path) {
                self.thumb_grid.forget(index);
            }
        }
    }

    /// Gives the embedded orientations that came back to the metadata cache (ADR 0024 §2.5,
    /// T237). When the reader has stopped, every turn still waiting is answered on the UI thread,
    /// so moves, deletes and "Open in" do not wait for ever. A frame is asked for while reads are
    /// on their way.
    fn follow_embedded_reads(&mut self, ctx: &egui::Context) {
        loop {
            match self.orientation_reader.try_recv() {
                Ok(Some((path, orientation))) => {
                    self.orientation_reads = self.orientation_reads.saturating_sub(1);
                    self.metadata.embedded_read(&path, orientation);
                }
                Ok(None) => break,
                Err(ReaderStopped) => {
                    for path in self.metadata.waiting_orientations() {
                        self.metadata
                            .embedded_read(&path, embedded_orientation(&path));
                    }
                    self.orientation_reads = 0;
                    break;
                }
            }
        }
        if self.orientation_reads > 0 {
            ctx.request_repaint_after(CAPTURE_POLL_INTERVAL);
        }
    }

    /// Takes the worker's finished thumbnails. A thumbnail of a file another program added, marked
    /// at a listing that found the file changed while it was being made, is checked once more
    /// when it comes back, and made anew when the file still differs from the stamp taken before
    /// it was asked for (ADR 0026 §3.2, the addition to F-Q7, T254); so it is asked for again even
    /// when no later listing comes, but not after every answer. Other paths are not read.
    /// RawStride's own writes do not count (T255): while RawStride is writing the file
    /// (`tab_pending`), the mark is taken but nothing is made anew.
    fn poll_thumbnails(&mut self, ctx: &egui::Context) {
        let cwd = std::env::current_dir().ok();
        loop {
            match self.thumb_worker.try_recv() {
                Ok(event) => {
                    // A thumbnail made with an orientation that is no longer the read one is
                    // dropped; the request made after `forget` brings the right one (T232).
                    if event.orientation
                        != read_orientation(&self.metadata, cwd.as_deref(), &event.path)
                    {
                        continue;
                    }
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
                                self.message =
                                    Some(shell_text(ShellText::InvalidThumbnail).to_string());
                            }
                        }
                        Err(_) => {
                            self.thumb_textures.remove(&event.index);
                        }
                    }
                    if self.added_files.contains(&event.path)
                        && self.added_files.take_check(&event.path)
                        && !tab_pending(&self.metadata, cwd.as_deref())(&event.path)
                        && self
                            .added_files
                            .is_stale(&event.path, file_stamp(&event.path))
                    {
                        self.thumb_worker.forget_path(&event.path);
                        self.thumb_grid.forget(event.index);
                        self.added_files.made_anew(&event.path);
                        ctx.request_repaint();
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.message = Some(shell_text(ShellText::ThumbnailWorkerStopped).to_string());
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
        commands: &mut Vec<Command>,
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
        let mut right_clicked_cell = None;
        let mut clicked_button_cell = None;
        // No right-click menu while a dialog holds the program (ADR 0019 §2.9, T221).
        let context_menu = !self.dialog_open();
        // The buttons on a thumbnail (ADR 0017 §2.8, U15, T245): only with the setting on and no
        // dialog open; an open menu or popup is checked per cell below.
        let thumb_buttons = self.settings.shell.thumbnail_buttons && context_menu;
        // A cell's double click this soon after a button click is a plain click: egui counts a
        // double click per pointer, not per widget. The delay is egui's own (`InputOptions`,
        // reached through the context's options; `InputState::options` is private).
        let now = ctx.input(|input| input.time);
        let double_click_delay =
            ctx.options(|options| options.input_options.max_double_click_delay);
        let after_button_click = self
            .thumb_button_clicked_at
            .is_some_and(|at| now - at <= double_click_delay);
        // The tab's files may be relative; the metadata cache keys by resolved path.
        let cwd = std::env::current_dir().ok();
        // A pending anchor of this tab sets the offset once (ADR 0026 §2 point 5, T242); a scroll
        // of this frame wins over it, and another tab drops it.
        let tab_index = self.tabs.active_index();
        let mut anchored = None;
        if let Some(anchor) = &self.grid_anchor {
            let step =
                if anchor.tab != tab_index || anchor.folder != tab.folder || scroll_to.is_some() {
                    AnchorStep::Drop
                } else {
                    anchor_step(
                        anchor,
                        &tab,
                        self.ratings_ready(),
                        stride,
                        self.grid_columns,
                    )
                };
            match step {
                AnchorStep::Set(offset) => {
                    anchored = Some(offset);
                    self.grid_anchor = None;
                }
                AnchorStep::Wait => {}
                AnchorStep::Drop => self.grid_anchor = None,
            }
        }

        // The grid uses the image area only, so it never runs under the status line.
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            let ctrl_held = ui.input(|input| input.modifiers.ctrl || input.modifiers.command);
            let mut area = egui::ScrollArea::vertical()
                .id_salt("contact-sheet")
                .scroll_source(egui::containers::scroll_area::ScrollSource {
                    mouse_wheel: !ctrl_held,
                    ..Default::default()
                })
                .auto_shrink([false, false]);
            if let Some(offset) = anchored {
                area = area.vertical_scroll_offset(offset);
            }
            area.show_viewport(ui, |ui, viewport| {
                let width = ui.available_width().max(0.0);
                let column_count = columns(
                    width.min(u32::MAX as f32) as u32,
                    self.thumb_size,
                    GRID_SPACING as u32,
                );
                self.grid_columns = column_count;
                self.grid_note = Some(grid_note(
                    &tab,
                    tab_index,
                    GridView {
                        offset: viewport.min.y,
                        height: viewport.height(),
                        columns: column_count,
                        cell_size: self.thumb_size,
                    },
                ));
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
                        if self.added_files.contains(path) {
                            self.added_files.note_requested(path, file_stamp(path));
                        }
                        let orientation = read_orientation(&self.metadata, cwd.as_deref(), path);
                        self.thumb_worker.request(ThumbnailRequest {
                            generation: self.thumb_generation,
                            index,
                            path: path.clone(),
                            cell,
                            orientation,
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
                let mut right_clicked = None;
                let mut clicked_button = None;
                let pointer = ui.input(|input| input.pointer.hover_pos());
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
                    let label_rect = egui::Rect::from_min_max(
                        egui::pos2(cell_rect.min.x, cell_rect.max.y - GRID_LABEL_HEIGHT),
                        cell_rect.max,
                    );
                    paint_cell(
                        &painter,
                        CellRects {
                            cell: cell_rect,
                            image: image_rect,
                            label: label_rect,
                        },
                        path,
                        shown_rating(&self.metadata, cwd.as_deref(), path),
                        tab.selection.is_marked(index),
                        index == tab.current,
                        self.thumb_grid.state(index),
                        self.thumb_textures.get(&index),
                    );
                    let response = ui.interact(
                        cell_rect,
                        ui.make_persistent_id(("grid-cell", index)),
                        egui::Sense::click(),
                    );
                    if response.double_clicked() && !after_button_click {
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
                    // The right-click menu on a cell only, not on the empty part of the
                    // contact sheet (ADR 0019 §2.9, E-Q15, E-Q16, T221).
                    if context_menu {
                        if response.secondary_clicked() {
                            right_clicked = Some(index);
                        }
                        response.context_menu(|ui| self.image_context_menu(ui, commands));
                    }
                    // The four buttons while the pointer is over the cell (`contains_pointer`:
                    // `hovered` is false over a button) and no menu or popup is open; the
                    // check reads egui's memory, so it also sees a right-click menu of a cell
                    // drawn later in the frame (ADR 0017 §2.8, U15-Q3, T245).
                    if thumb_buttons {
                        let draw =
                            response.contains_pointer() && !egui::Popup::is_any_open(ui.ctx());
                        // The cell's image as if it were current, by the menu items' rules
                        // (U15-Q5): a cell in the contact sheet is a shown image, so the rotations
                        // (`tab.position()`) and Preview (a visible list) can run; Metadata… only
                        // for a kind with metadata support, which `open_window` needs.
                        let facts = ButtonFacts {
                            rotate: true,
                            metadata: supports_rating(path),
                            preview: true,
                        };
                        let under_pointer = pointer.and_then(|at| button_at(image_rect, at));
                        for button in THUMB_BUTTONS {
                            let button_id =
                                ui.make_persistent_id(("grid-cell-button", index, button));
                            // While this button's right-click menu is open, its area and menu
                            // are kept each frame, or egui closes the popup as abandoned; the
                            // button itself is not drawn while a popup is open (T245 point 11).
                            let menu_open =
                                egui::Popup::is_id_open(ui.ctx(), button_id.with("popup"));
                            let drawn = draw && button.shown(&facts);
                            if !drawn && !menu_open {
                                continue;
                            }
                            let button_rect = button_rect(image_rect, button);
                            // Added after the cell, so a click on it is the button's alone.
                            let mut button_response =
                                ui.interact(button_rect, button_id, egui::Sense::click());
                            if drawn {
                                let disc = if under_pointer == Some(button) {
                                    image_area::THUMB_BUTTON_DISC_HOVERED
                                } else {
                                    image_area::THUMB_BUTTON_DISC
                                };
                                painter.circle_filled(
                                    button_rect.center(),
                                    button_rect.width() * 0.5,
                                    disc,
                                );
                                paint_thumb_button_symbol(&painter, button_rect, button);
                                let command = button.command();
                                let key = menu_items()
                                    .into_iter()
                                    .find(|item| item.command == command)
                                    .and_then(|item| item.key);
                                button_response = button_response
                                    .on_hover_text(thumb_button_hover_text(command, key));
                                if button_response.clicked() {
                                    clicked_button = Some((index, button));
                                    self.thumb_button_clicked_at = Some(now);
                                }
                            }
                            // A right click on a button is a right click on the cell.
                            if button_response.secondary_clicked() {
                                right_clicked = Some(index);
                            }
                            button_response
                                .context_menu(|ui| self.image_context_menu(ui, commands));
                        }
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
                if let Some(index) = right_clicked {
                    right_clicked_cell = Some(index);
                }
                if let Some(click) = clicked_button {
                    clicked_button_cell = Some(click);
                }
            });
        });
        if let Some((index, ctrl, shift)) = clicked_cell {
            self.click(index, ctrl, shift);
        }
        if let Some(index) = right_clicked_cell {
            self.right_click(index);
        }
        if let Some((index, button)) = clicked_button_cell {
            self.thumb_button_click(&ctx, index, button);
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
                    .unwrap_or_else(|| shell_text(ShellText::DropHere).to_string())
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
        // The status line is always shown in full (T110: `I` toggles the inspector now).
        {
            let current_path = tab
                .position()
                .and_then(|_| tab.files.get(tab.current))
                .cloned();
            let mut parts = if let (Some(position), Some(path)) = (tab.position(), &current_path) {
                // "3 / 12 (av 31) · 12 markerade · TN100423.ARW": the marking before the name
                // (owner decision 2026-11-10; ADR 0014 §5.3 lists the parts).
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let mut parts = vec![position_text(position, tab.visible.len(), tab.files.len())];
                if let Some(marked) = marked_text(tab.selection.count()) {
                    parts.push(marked);
                }
                parts.push(name);
                parts
            } else if tab.files.is_empty() {
                vec![shell_text(ShellText::NoImageSelected).to_string()]
            } else {
                vec![self.nothing_shown_text(&tab)]
            };
            if let Some(message) = &self.message {
                parts.push(message.clone());
            }
            if let Some(sorting) = self.sort_status() {
                parts.push(sorting);
            }
            if let Some(notice) = self.metadata.notice(current_path.as_deref()) {
                parts.push(notice);
            }
            if let Some(status) = import_status {
                // An empty line takes the import status alone, without a separator.
                if let [only] = parts.as_slice()
                    && only.is_empty()
                {
                    parts.clear();
                }
                parts.push(status);
            }
            let line = status_line_text(&parts);
            // The thumbnail size moved to the command bar (T109).
            self.next_status = StatusBar { line, mode: None };
        }
    }

    /// Why a tab with files shows nothing (ADR 0014 §2.4, §5.3, Q7): the read error, the ratings
    /// still being read under an active filter, or no image matching the filter.
    fn nothing_shown_text(&self, tab: &Tab) -> String {
        if tab.filter.is_active() {
            if let Some(error) = self.metadata.read_error() {
                return editor_text(&EditorText::MetadataError { error });
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
                self.load_activated_tab();
                self.prune_metadata_to_open_tabs();
                return;
            }
        }
        self.load_activated_tab();
    }

    fn prune_metadata_to_open_tabs(&mut self) {
        let current_dir = match std::env::current_dir() {
            Ok(current_dir) => current_dir,
            Err(error) => {
                self.message = Some(prune_error_text(&error.to_string()));
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
        zoom_anchor(self.anchor_at_pointer, pointer, rect, ppp)
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
            // The inspector open ↔ collapsed, as Visa ▸ Inspektör (U2-Q3, T110).
            Action::ToggleInfo => self.toggle_column(false),
            Action::ToggleMode => self.core.toggle_mode(),
            Action::OpenMetadata => self.open_metadata_window(),
        }
    }

    /// Opens the metadata window for the current image (ADR 0007 I10n, D16, T107) and asks for
    /// its metadata at once; with two or more images marked on the current image if it is marked,
    /// else on the first marked one (U12c-Q1, T126). Nothing happens with no image shown, a kind
    /// without metadata support, or a window already open.
    fn open_metadata_window(&mut self) {
        let cwd = std::env::current_dir().ok();
        let Some(path) = metadata_subject(self.tabs.active(), cwd.as_deref()) else {
            return;
        };
        self.open_metadata_window_on(path);
    }

    /// Opens the metadata window on `path` (as the metadata cache keys it) and asks for its
    /// metadata at once: from `open_metadata_window`, and from the Metadata… button on a
    /// thumbnail with the clicked image (ADR 0017 §2.8, U15-Q4, T245). "Apply to {n} Marked"
    /// gets the marked images of the active tab as before. Nothing happens for a kind without
    /// metadata support or while a metadata or settings window is open.
    fn open_metadata_window_on(&mut self, path: PathBuf) {
        // Not while the settings window is open: the window's read must use the saved settings.
        if self.metadata_window.is_some() || self.settings_window.is_some() {
            return;
        }
        let cwd = std::env::current_dir().ok();
        let marked = marked_subjects(self.tabs.active(), cwd.as_deref());
        if self.metadata.open_window(&path, &mut self.metadata_worker) {
            self.metadata_window = Some(MetadataWindow::new(path, marked));
        }
    }

    /// Closes the metadata window without writing.
    fn close_metadata_window(&mut self) {
        self.metadata_window = None;
        // The next opening is placed in the middle of the image area again (T131). Reset here,
        // not only when a frame is drawn without the window: `M` may open it again in the very
        // next frame, before the window is drawn (reviewer's fix).
        self.metadata_window_shown = false;
        self.metadata.close_window();
    }

    /// The image the metadata window on `path` moves to with "Previous" (`forward` false) or
    /// "Next" (ADR 0017 §2.3.1, U12-Q3, T112): its file index in the active tab and its path as
    /// the metadata cache keys it. It steps between the marked images when the window's image is
    /// marked and two or more are, else between the shown images of the active tab
    /// (`metadata_steps`, U15-Q9, T244), skipping kinds without metadata support; `None` at the
    /// first or last such image. The marking is read from the tab each time, so it is the marking
    /// as it is at the step (the window is modal; a rating chosen in the window can refilter the
    /// tab when the window moves, U10c-Q4, and the refilter may unmark images it hides,
    /// `Tab::refilter`).
    fn window_neighbour(&self, path: &Path, forward: bool) -> Option<(usize, PathBuf)> {
        let tab = self.tabs.active()?;
        let cwd = std::env::current_dir().ok();
        let key = |file: &PathBuf| match cwd.as_deref() {
            Some(cwd) => resolve_path(cwd, file),
            None => file.clone(),
        };
        let current = tab.files.iter().position(|file| key(file) == path)?;
        let steps = metadata_steps(
            &tab.visible,
            &tab.selection.marked_in(&tab.visible),
            current,
        );
        let index = metadata_neighbour(&steps, current, forward, |index| {
            tab.files
                .get(index)
                .is_some_and(|file| supports_rating(file))
        })?;
        Some((index, key(tab.files.get(index)?)))
    }

    /// Does a move or a close of the metadata window (`window`, taken out of `self`). A move
    /// that finds no image in its direction keeps the window on its image.
    fn leave_metadata_window(
        &mut self,
        ctx: &egui::Context,
        mut window: MetadataWindow,
        leave: WindowLeave,
    ) {
        match leave {
            WindowLeave::Close => {
                let refilter = window_refilter(window.rated, Some(leave), None);
                self.close_metadata_window();
                if refilter == WindowRefilter::AsAfterKey
                    && let Some(Some(position)) = self.after_rating()
                {
                    self.scroll_request = Some(position);
                }
            }
            WindowLeave::Move { forward } => {
                // Found in the visible list as it is, before any refilter (U10c-Q4).
                let target = self.window_neighbour(&window.path, forward);
                if let Some((index, path)) = target
                    && self.metadata.move_window(&path, &mut self.metadata_worker)
                {
                    let refilter = window_refilter(window.rated, Some(leave), Some(index));
                    // The window stays open at its place and size (its id does not change); the
                    // form is emptied while the new image's read runs. The marking is kept.
                    let marked = std::mem::take(&mut window.marked);
                    self.metadata_window = Some(MetadataWindow::new(path, marked));
                    self.current_follows_window(index, refilter);
                } else {
                    // Nowhere to go (the shown images changed meanwhile). After a write that
                    // succeeded, the read after it refills the form.
                    if self.metadata.editor_reloading() {
                        window.expect_reload = true;
                    }
                    self.metadata_window = Some(window);
                }
                ctx.request_repaint();
            }
        }
    }

    /// After the metadata window moved to file `index` (ADR 0017 §2.3.1, U12-Q2, T112): it is the
    /// current image of the active tab, scrolled into view in the contact sheet or shown in the
    /// preview as a step shows it. The marking is not changed (§2.2, review finding 6). With
    /// `KeepNeighbour` (a rating was chosen in the window, U10c-Q4, T124) the tab is first
    /// refiltered keeping that image; a shown preview then gets the new visible list.
    fn current_follows_window(&mut self, index: usize, refilter: WindowRefilter) {
        let Some(tab) = self.tabs.active_mut() else {
            return;
        };
        tab.current = index;
        let changed = match refilter {
            WindowRefilter::KeepNeighbour(keep) => self.refilter_active(Some(keep)),
            WindowRefilter::Nothing | WindowRefilter::AsAfterKey => false,
        };
        let Some(tab) = self.tabs.active() else {
            return;
        };
        let (mode, position) = (tab.mode, tab.position());
        match (mode, position) {
            (ViewMode::Grid, _) => self.scroll_request = position,
            (ViewMode::Loupe, _) if changed => {
                if let Some(position) = self.loupe_follows_visible() {
                    self.scroll_request = Some(position);
                }
            }
            (ViewMode::Loupe, Some(position)) => self.core.go(position),
            (ViewMode::Loupe, None) => {}
        }
    }

    /// A rating or label for the metadata window's image (ADR 0017 U10c-Q1–Q3, T124), from its
    /// buttons or its keys: queued at once through the path of the keys (`set_ratings` with the
    /// one image), refused as the keys refuse it. Not part of the form's edits, and the tab is not
    /// refiltered until the window moves or closes (U10c-Q4).
    fn rate_window(&mut self, window: &mut MetadataWindow, action: RatingKey) {
        match self
            .metadata
            .set_ratings(std::slice::from_ref(&window.path), action)
        {
            Ok(_) => window.rated = true,
            Err(RatingRefused::Unsupported) => {
                self.message = Some(UNSUPPORTED_TEXT.to_owned());
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    /// Adds the values of the metadata window's write that succeeded to the recent values (ADR
    /// 0017 U12a-Q2, T141), and saves them when they changed.
    fn record_window_values(&mut self, window: &mut MetadataWindow) {
        let recorded = std::mem::take(&mut window.recorded);
        if self.field_values.add_written(&recorded) {
            self.save_field_values();
        }
    }

    /// Saves `field-values.txt` (U12a-Q6, T141); a failure is written to the log.
    fn save_field_values(&self) {
        if let Some(path) = &self.field_values_path
            && let Err(error) = save_field_values(path, &self.field_values)
        {
            eprintln!("pw-loupe: the field values could not be saved: {error}");
        }
    }

    /// Writes the window's edits as OK writes them (T105, T107); `then` is the move or close to
    /// do once the write succeeded without notices. Field errors are shown and nothing is written;
    /// a write that cannot be sent shows why. Returns `then` when there is nothing to write.
    fn write_window(
        &mut self,
        ctx: &egui::Context,
        window: &mut MetadataWindow,
        then: WindowLeave,
    ) -> Option<WindowLeave> {
        let (edits, written, lists_with_errors) = match &window.body {
            WindowBody::Form(state) => (
                state.form.edits(),
                state.form.written_values(),
                state.form.lists_with_errors(),
            ),
            WindowBody::Loading | WindowBody::ReadError(_) => return Some(then),
        };
        let outcome = match edits {
            Ok(edits) if edits.is_empty() => return Some(then),
            // Field errors: nothing is written, the errors are shown under their fields and the
            // status line names them (U11c-Q9, T181).
            Err(errors) => {
                window.status = not_saved_status(&errors, lists_with_errors);
                window.errors = errors;
                WriteOutcome::FieldErrors
            }
            Ok(edits) => {
                window.errors.clear();
                window.status = EditorStatus::Idle;
                match self
                    .metadata
                    .save_edits(&window.path, edits, &mut self.metadata_worker)
                {
                    Ok(()) => {
                        window.after_save = Some(then);
                        // Recent values once the write has succeeded (U12a-Q2, T141).
                        window.written = written;
                        ctx.request_repaint();
                        return None;
                    }
                    Err(message) => {
                        window.status = EditorStatus::Failed(message);
                        WriteOutcome::Failure
                    }
                }
            }
        };
        match leave_decision(true, SaveAnswer::Save, Some(outcome)) {
            LeaveDecision::Leave => Some(then),
            LeaveDecision::Write
            | LeaveDecision::StayWithEdits
            | LeaveDecision::StayShowingNotices => None,
        }
    }

    /// The metadata window (ADR 0007 I10n, D16, T107, ADR 0017 §2.3.1, T112): modal, movable and
    /// resizable, its contents in a scroll area and the buttons below it. Every text comes from
    /// `editor_text`, the form (labels, notes, field errors, warnings), `notice_text`,
    /// `panel_text` or `info_panel.rs`, or is data (the file name, values).
    fn draw_metadata_window(&mut self, ctx: &egui::Context) {
        // Taken every frame, so nothing is left over from a window that was closed meanwhile.
        let events = self.metadata.take_editor_events();
        let read = self.metadata.take_window_read();
        let Some(mut window) = self.metadata_window.take() else {
            self.metadata_window_shown = false;
            return;
        };
        for event in events {
            if let Some(leave) = window.apply(event) {
                self.record_window_values(&mut window);
                self.leave_metadata_window(ctx, window, leave);
                return;
            }
        }
        self.record_window_values(&mut window);
        // The batch of "Apply to All Marked" locks the window as its own write does (U12c-Q5).
        let batch_progress = self.metadata.batch_progress();
        let saving = self.metadata.editor_saving() || batch_progress.is_some();
        let locked = saving || self.metadata.editor_reloading();
        if let Some(result) = read {
            window.read_arrived(result, locked);
        }
        let waiting = self.metadata.window_waiting();
        if locked || waiting {
            ctx.request_repaint_after(WINDOW_POLL_INTERVAL);
        }
        // The window's keys (U12-Q1) and its rating keys (U10c-Q2, T124) are taken before the
        // form is drawn, so they work while a field has the keyboard and never reach it.
        let (keys, rating_keys): (Vec<EditorKeyAction>, Vec<RatingKey>) = ctx.input_mut(|input| {
            let mut keys = Vec::new();
            let mut rating_keys = Vec::new();
            input.events.retain(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let ctrl = modifiers.ctrl || modifiers.command;
                    if let Some(action) =
                        editor_key_action(*key, ctrl, modifiers.shift, modifiers.alt)
                    {
                        keys.push(action);
                        false
                    } else if let Some(action) =
                        editor_rating_key(*key, ctrl, modifiers.shift, modifiers.alt)
                    {
                        rating_keys.push(action);
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            });
            (keys, rating_keys)
        });
        let escape = std::mem::take(&mut window.escape);
        if std::mem::take(&mut window.value_list_escape) {
            window.value_list = None;
        }
        let asking_save = window.question.is_some();
        // The confirmation of "Apply to All Marked" (U12c-Q4, T126) is asked as the save question.
        let apply_lines = window
            .apply_question
            .as_ref()
            .map(|plan| plan.lines.clone());
        let asking = asking_save || apply_lines.is_some();
        let has_form = matches!(window.body, WindowBody::Form(_));
        // "Apply to {n} Marked" with two or more marked, enabled when `apply_allowed` (the
        // window's image is marked, no field errors, something to write to the other images), no
        // write runs and no question is asked (U12c-Q2).
        let marked_count = window.marked.len();
        let can_apply = !locked
            && !asking
            && match &window.body {
                WindowBody::Form(state) => apply_allowed(&state.form, &window.path, &window.marked),
                WindowBody::Loading | WindowBody::ReadError(_) => false,
            };
        let mut apply = false;
        let mut apply_answer = None;
        // No move while the read or a write runs, nor at the first or last supported image.
        let busy = locked || waiting;
        let can_previous = !busy && self.window_neighbour(&window.path, false).is_some();
        let can_next = !busy && self.window_neighbour(&window.path, true).is_some();
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
        // The sections whose heading was clicked (T123), and their state as it is now.
        let mut toggled_sections = Vec::new();
        // The rating and label buttons show the store's value, as the badges do (T124).
        let rating = self.metadata.shown_rating(&window.path).cloned();
        let mut rating_choice = None;
        let shell = self.settings.shell.clone();
        // The sections and items the form shows, in their order (U10b, T278); the Settings window
        // is never open with this window, so it does not change under the form.
        let layout = self.settings.editor_layout.clone();
        let mut ok = false;
        let mut cancel = false;
        let mut previous = false;
        let mut next = false;
        let mut save_and_next = false;
        let mut answer = None;
        // "Copy All Fields" while the form is shown; "Paste All Fields" when something is copied
        // and the form is enabled (U12b-Q3, T125). Both buttons are hidden while asking.
        let can_copy = has_form;
        let can_paste = self.copied_fields.is_some() && has_form && !locked && !asking;
        let paste_tooltip = self.copied_fields.as_ref().map(|copied| {
            editor_text(&EditorText::PasteTooltip {
                fields: copied.count(),
                file: copied.file().to_string(),
            })
        });
        let mut copy = false;
        let mut paste = false;
        // While the question is shown the window's keys do nothing.
        if !asking {
            for key in keys {
                match key {
                    EditorKeyAction::Previous => previous |= can_previous,
                    EditorKeyAction::Next => next |= can_next,
                    EditorKeyAction::SaveAndNext => save_and_next |= can_next,
                    EditorKeyAction::CopyAllFields => copy |= can_copy,
                    EditorKeyAction::PasteAllFields => paste |= can_paste,
                }
            }
        }

        // Modal (D16): the program behind takes no clicks, drags or wheel while the window is open.
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("metadata-window-backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let response = ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
                ui.painter().rect_filled(response.rect, 0.0, BACKDROP);
            });

        let window_id = egui::Id::unique("metadata-window");
        let mut metadata_window = egui::Window::new(editor_text(&EditorText::WindowTitle {
            file: window.file.clone(),
        }))
        .id(window_id)
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(true)
        .default_size(WINDOW_SIZE);
        // Placed in the middle of the image area on the first frame it is drawn (T131), and left
        // to the user after that: egui remembers the position by the window's id, so it is set
        // here once rather than given as a default, which the remembered position would win over.
        if !std::mem::replace(&mut self.metadata_window_shown, true) {
            // The size the window had when it was last open, else the size it opens with.
            let size = ctx
                .memory(|memory| memory.area_rect(window_id))
                .map_or_else(|| egui::Vec2::from(WINDOW_SIZE), |rect| rect.size());
            let image_area = self.image_area.unwrap_or(screen);
            let position = metadata_window_position(image_area, size, screen);
            metadata_window = metadata_window.current_pos(position);
            // egui 0.37 puts a window that is moved by its title bar back at the position it
            // remembers after `current_pos` was applied (`Window::show_dyn`,
            // `pivot_pos_before_begin`), so the remembered position is replaced too (reviewer's
            // fix after the owner's hand check).
            if let Some(mut state) = egui::AreaState::load(ctx, window_id) {
                state.set_left_top_pos(position);
                ctx.memory_mut(|memory| {
                    memory.areas_mut().set_state(
                        egui::LayerId::new(egui::Order::Foreground, window_id),
                        state,
                    );
                });
            }
        }
        // The "▼" lists of recent and own values (U12a, T141).
        let mut value_ui = ValueUi {
            values: &self.field_values,
            list: &mut window.value_list,
            actions: Vec::new(),
        };
        metadata_window.show(ctx, |ui| {
            let error_color = ui.visuals().error_fg_color;
            egui::Panel::bottom("metadata-window-buttons")
                .resizable(false)
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    ui.add_space(WINDOW_BUTTON_SPACING);
                    if let Some((current, total)) = batch_progress {
                        ui.label(editor_text(&EditorText::Writing { current, total }));
                    } else if saving {
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
                            EditorStatus::Batch(lines) => {
                                for line in lines {
                                    ui.label(line);
                                }
                            }
                            EditorStatus::Failed(message) => {
                                ui.label(egui::RichText::new(message).color(error_color));
                            }
                        }
                    }
                    if let Some(lines) = &apply_lines {
                        ui.label(
                            egui::RichText::new(editor_text(&EditorText::ApplyTitle)).strong(),
                        );
                        for line in lines {
                            ui.label(line);
                        }
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(editor_text(&EditorText::Write)).clicked() {
                                apply_answer = Some(true);
                            }
                            if ui.button(editor_text(&EditorText::Cancel)).clicked() {
                                apply_answer = Some(false);
                            }
                        });
                    } else if asking_save {
                        ui.label(editor_text(&EditorText::SaveQuestion));
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(editor_text(&EditorText::Save)).clicked() {
                                answer = Some(SaveAnswer::Save);
                            }
                            if ui.button(editor_text(&EditorText::Discard)).clicked() {
                                answer = Some(SaveAnswer::Discard);
                            }
                            if ui.button(editor_text(&EditorText::Cancel)).clicked() {
                                answer = Some(SaveAnswer::Cancel);
                            }
                        });
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            copy |= ui
                                .add_enabled(
                                    can_copy,
                                    egui::Button::new(editor_text(&EditorText::CopyAllFields)),
                                )
                                .clicked();
                            let mut paste_button = ui.add_enabled(
                                can_paste,
                                egui::Button::new(editor_text(&EditorText::PasteAllFields)),
                            );
                            if let Some(tooltip) = &paste_tooltip {
                                paste_button = paste_button
                                    .on_hover_text(tooltip)
                                    .on_disabled_hover_text(tooltip);
                            }
                            paste |= paste_button.clicked();
                            previous |= ui
                                .add_enabled(
                                    can_previous,
                                    egui::Button::new(editor_text(&EditorText::Previous)),
                                )
                                .clicked();
                            next |= ui
                                .add_enabled(
                                    can_next,
                                    egui::Button::new(editor_text(&EditorText::Next)),
                                )
                                .clicked();
                            save_and_next |= ui
                                .add_enabled(
                                    can_next,
                                    egui::Button::new(editor_text(&EditorText::SaveAndNext)),
                                )
                                .clicked();
                            if marked_count > 1 {
                                apply |= ui
                                    .add_enabled(
                                        can_apply,
                                        egui::Button::new(editor_text(
                                            &EditorText::ApplyToMarked {
                                                images: marked_count,
                                            },
                                        )),
                                    )
                                    .clicked();
                            }
                            // Until the read is back, or when it failed, no "OK".
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
                    // Under the title: how many images are marked (U12c-Q1), and that OK and
                    // Save write this image only (U12d-Q4).
                    if marked_count > 1 {
                        ui.label(editor_text(&EditorText::MarkedImages {
                            images: marked_count,
                        }));
                        ui.label(editor_text(&EditorText::ThisImageOnly));
                    }
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
                                let response = draw_window_form(
                                    ui,
                                    state,
                                    &window.errors,
                                    &warnings,
                                    !locked && !asking,
                                    &shell,
                                    &layout,
                                    rating.as_ref(),
                                    !asking,
                                    &mut value_ui,
                                );
                                choice = response.choice;
                                toggled_sections = response.toggled;
                                rating_choice = response.rating;
                            }
                        });
                });
        });

        // "Add to My Values" and "×" (U12a-Q3, T141), saved at once.
        let ValueUi { actions, .. } = value_ui;
        let mut values_changed = false;
        for action in actions {
            values_changed |= match action {
                ValueAction::AddOwn(field, values) => self.field_values.add_own(field, &values),
                ValueAction::RemoveOwn(field, value) => self.field_values.remove_own(field, &value),
            };
        }
        if values_changed {
            self.save_field_values();
        }
        // Remembered in settings.txt at exit, as the inspector's sections (U10-Q6).
        for section in toggled_sections {
            self.settings.shell.toggle_editor_section(section);
        }
        // A rating or label is queued at once (U10c-Q3); while the question is shown the rating
        // keys do nothing (the buttons are disabled then).
        if !asking {
            let chosen: Vec<RatingKey> = rating_keys.into_iter().chain(rating_choice).collect();
            if !chosen.is_empty() {
                for action in chosen {
                    self.rate_window(&mut window, action);
                }
                ctx.request_repaint();
            }
        }
        if let (Some((conflict_index, value_index)), WindowBody::Form(state)) =
            (choice, &mut window.body)
        {
            state.form.choose(conflict_index, value_index);
        }
        // Copy, then paste, as the buttons and keys asked (T125). A paste is unsaved edits, as
        // typing gives; the status below is cleared when it changed the form. Only what the
        // layout shows is copied and pasted (U10b-Q4, T278).
        if let WindowBody::Form(state) = &mut window.body {
            if copy {
                self.copied_fields = Some(state.form.copy_shown(&window.file, &layout));
            }
            if paste && let Some(copied) = &self.copied_fields {
                state.form.paste_shown(copied, &layout);
                ctx.request_repaint();
            }
        }
        let form_after = match &window.body {
            WindowBody::Form(state) => Some(&state.form),
            WindowBody::Loading | WindowBody::ReadError(_) => None,
        };
        if form_after != form_before.as_ref() {
            // "Sparat." or an error no longer describes the form.
            window.status = EditorStatus::Idle;
        }
        // Escape while asking is the question's "Avbryt" (back to the window).
        if escape && asking_save && answer.is_none() {
            answer = Some(SaveAnswer::Cancel);
        }
        // The answer to the confirmation of "Apply to All Marked" (U12c-Q4, U12c-Q5, T126):
        // "Write" starts the batch, "Cancel" and Escape go back with the edits kept.
        if apply_lines.is_some() {
            if escape && apply_answer.is_none() {
                apply_answer = Some(false);
            }
            match apply_answer {
                Some(true) => {
                    if let Some(plan) = window.apply_question.take() {
                        window.errors.clear();
                        window.status = EditorStatus::Idle;
                        // Recent values once a write of the batch has succeeded (U12a-Q2, T141).
                        window.written = match &window.body {
                            WindowBody::Form(state) => state.form.written_values(),
                            WindowBody::Loading | WindowBody::ReadError(_) => Vec::new(),
                        };
                        let started = self.metadata.start_batch(
                            &window.path,
                            plan.this_image,
                            plan.others,
                            &mut self.metadata_worker,
                        );
                        if !started {
                            window.written.clear();
                        }
                        ctx.request_repaint();
                    }
                }
                Some(false) => window.apply_question = None,
                None => {}
            }
            self.metadata_window = Some(window);
            return;
        }
        // "Apply to {n} Marked": the confirmation is asked before anything is written; a field
        // error is shown under its field and named in the status line instead (U11c-Q9, T181).
        if apply && let WindowBody::Form(state) = &window.body {
            match ApplyPlan::new(&state.form, &window.path, &window.marked) {
                Ok(plan) => {
                    window.status = EditorStatus::Idle;
                    window.apply_question = Some(plan);
                }
                Err(errors) => {
                    window.status = not_saved_status(&errors, state.form.lists_with_errors());
                    window.errors = errors;
                }
            }
            ctx.request_repaint();
            self.metadata_window = Some(window);
            return;
        }
        // The answer to "Spara ändringarna?" (U12-Q4).
        if let (Some(answer), Some(leave)) = (answer, window.question) {
            window.question = None;
            let leave_now = match leave_decision(window.has_unsaved_edits(), answer, None) {
                LeaveDecision::Leave => Some(leave),
                LeaveDecision::Write => self.write_window(ctx, &mut window, leave),
                LeaveDecision::StayWithEdits | LeaveDecision::StayShowingNotices => None,
            };
            match leave_now {
                Some(leave) => self.leave_metadata_window(ctx, window, leave),
                None => self.metadata_window = Some(window),
            }
            return;
        }
        // A move, Escape or "Avbryt": with unsaved edits the question is asked first; without, the
        // move or the close is done at once. Escape and "Avbryt" do nothing while the write runs.
        let leave = if previous {
            Some(WindowLeave::Move { forward: false })
        } else if next {
            Some(WindowLeave::Move { forward: true })
        } else if (escape || cancel) && !saving && !asking {
            Some(WindowLeave::Close)
        } else {
            None
        };
        if let Some(leave) = leave {
            if window.has_unsaved_edits() {
                window.question = Some(leave);
                self.metadata_window = Some(window);
            } else {
                self.leave_metadata_window(ctx, window, leave);
            }
            return;
        }
        // "Spara och nästa" never asks: it writes the edits (when there are any) and moves on
        // success; with field errors, a failure or notices it stays as "Spara" does. OK writes and
        // closes (T107, unchanged).
        let write = if save_and_next {
            Some(WindowLeave::Move { forward: true })
        } else if ok {
            Some(WindowLeave::Close)
        } else {
            None
        };
        if let Some(then) = write
            && let Some(leave) = self.write_window(ctx, &mut window, then)
        {
            self.leave_metadata_window(ctx, window, leave);
            return;
        }
        self.metadata_window = Some(window);
    }

    /// Whether a dialog holds the program: the metadata window (modal, D16), the settings window,
    /// a shown import dialog, the question of "Open in {name}" above 100 images (T202), or the
    /// confirmation of a delete (T189), the last two also while they wait for another dialog to
    /// close.
    fn dialog_open(&self) -> bool {
        self.other_dialog_open() || matches!(self.delete, DeleteStep::Confirming(_))
    }

    /// Whether a dialog other than the confirmation of a delete is open: the metadata window, the
    /// settings window, a shown import dialog, or the question of "Open in {name}" (its step
    /// `Confirming`, T202). The confirmation waits while one is (T189).
    fn other_dialog_open(&self) -> bool {
        self.metadata_window.is_some()
            || self.settings_window.is_some()
            || (self.import_dialog.is_some() && !self.import_hidden)
            || matches!(self.open, OpenStep::Confirming(_))
    }

    /// Whether the question of "Open in {name}" is shown this frame (ADR 0019 §2.5, T202): the
    /// step is `Confirming` and neither the metadata window, the settings window nor a shown
    /// import dialog is open. Written out on its own: `other_dialog_open` is true while the
    /// question itself is `Confirming`. Unlike the delete's, it does not depend on the active tab
    /// (the request holds its own paths).
    fn open_window_shown(&self) -> bool {
        matches!(self.open, OpenStep::Confirming(_))
            && self.metadata_window.is_none()
            && self.settings_window.is_none()
            && !(self.import_dialog.is_some() && !self.import_hidden)
    }

    /// Whether the confirmation of a delete is shown this frame: the step is `Confirming`, no
    /// other dialog is open, and the active tab is still of the delete's folder (T189 point 5).
    fn delete_window_shown(&self) -> bool {
        matches!(self.delete, DeleteStep::Confirming(_))
            && !self.other_dialog_open()
            && self.delete_tab_active()
    }

    /// Whether the active tab is of the folder the delete was asked for in (`same_folder`); the
    /// confirmation never asks about a tab that is not shown (T189 point 5).
    fn delete_tab_active(&self) -> bool {
        match (
            self.tabs.active().and_then(|tab| tab.folder.as_deref()),
            self.delete_folder.as_deref(),
        ) {
            (Some(open), Some(folder)) => same_folder(open, folder),
            _ => false,
        }
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
        // The preview's full screen is not the window's state (T111): nothing is taken while the
        // preview is shown, nor after it closed until the system reports the window as it was.
        if self.window_before.is_some() {
            return;
        }
        if let Some((expected, frames)) = self.window_settling {
            if fullscreen != Some(expected) && frames > 0 {
                self.window_settling = Some((expected, frames - 1));
                return;
            }
            self.window_settling = None;
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
        if let Some(folder) = pick_folder(
            shell_text(ShellText::OpenFolderTitle),
            NO_FOLDER_TEXT,
            start.as_ref(),
        ) {
            self.open(&folder, true);
        }
    }

    /// Visa ▸ Tema (ADR 0017 §2.5, T109): applied at once and saved.
    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.shell.theme = theme;
        ctx.set_theme(egui_theme(theme));
        if let Some(path) = &self.settings_path
            && let Err(error) =
                save_settings_with_library(path, &self.settings, &self.library.saved())
        {
            eprintln!("pw-loupe: the theme could not be saved: {error}");
        }
    }

    /// Visa ▸ Vänster kolumn, and Visa ▸ Inspektör and `I` for the right column (T110): opens or
    /// collapses the column; a hidden pair is shown again with that column open.
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

    /// Follows the inspector's image every frame, also while the inspector is collapsed, so the
    /// Exif summary and the metadata cache know how long the image has been the current one
    /// (ADR 0017 §2.1, T110). Each section asks for its read only while the right column and the
    /// section are open (in the preview: the preview's column and its Info, T111); the metadata
    /// waits while the metadata window is open. Called after the frame's keys, so an image changed
    /// by a key is followed in the same frame.
    fn follow_inspector(&mut self, ctx: &egui::Context) {
        let cwd = std::env::current_dir().ok();
        let image = inspector_subject(self.tabs.active(), cwd.as_deref());
        let shell = self.settings.shell.clone();
        let column_open = !shell.columns_hidden && shell.right_column == ColumnState::Open;
        // In the preview its own column's Info is read with the same rules, and there is no
        // metadata section (T111).
        let in_preview = self.in_preview();
        let info_shown = if in_preview {
            preview_info_shown(&shell)
        } else {
            column_open && shell.inspector_info == ColumnState::Open
        };
        let now = Instant::now();
        let exif_wait = self
            .exif
            .request(image.as_deref(), info_shown, now, &mut self.exif_reader);
        let metadata_wait = self.metadata.inspector_request(
            image.as_deref(),
            !in_preview && column_open && shell.inspector_metadata == ColumnState::Open,
            self.metadata_window.is_some(),
            now,
            &mut self.metadata_worker,
        );
        // The Histogram section reads with the same rules, in the tab's display mode (T144).
        let histogram_wait = self.histograms.request(
            image.as_deref(),
            self.display_mode,
            !in_preview && column_open && shell.inspector_histogram == ColumnState::Open,
            now,
            &mut self.histogram_reader,
        );
        if let Some(wait) = exif_wait
            .into_iter()
            .chain(metadata_wait)
            .chain(histogram_wait)
            .min()
        {
            ctx.request_repaint_after(wait);
        }
    }

    /// What the Histogram section shows for `image` (ADR 0017 §2.6, U7-Q7, T142): the statistics
    /// the view core gave in the last rendered preview frame, when that frame showed this image;
    /// "Reading …" until then.
    fn histogram_for(&self, image: Option<&Path>) -> HistogramPanel {
        let statistics = match (&self.last_statistics, image) {
            (Some((path, statistics)), Some(image)) if path == image => statistics.as_ref(),
            _ => None,
        };
        histogram_panel(statistics)
    }

    /// Whether the active tab shows the preview (ADR 0017 §2.2, T111).
    fn in_preview(&self) -> bool {
        self.tabs
            .active()
            .is_some_and(|tab| tab.mode == ViewMode::Loupe)
    }

    /// The window follows the preview (U3-Q1, U3-Q6, T111): when the preview opens, the window's
    /// state is kept and the window goes full screen; when it closes, the window returns to that
    /// state (full screen if F11 had made it so, else maximized or its position and size).
    /// `switch_mode` opens and closes the preview; this follows it at the end of the frame, also
    /// when a file path opened the preview.
    fn sync_preview_window(&mut self) {
        match (self.in_preview(), self.window_before) {
            (true, None) => {
                self.window_before = Some(WindowBefore::of(&self.settings.shell));
                self.window_settling = None;
                self.viewport_commands
                    .push(egui::ViewportCommand::Fullscreen(true));
            }
            (false, Some(before)) => {
                self.window_before = None;
                self.window_settling = Some((before.fullscreen, WINDOW_SETTLE_FRAMES));
                match before.restore() {
                    Restore::StayFullscreen => {}
                    Restore::Maximized => {
                        self.viewport_commands
                            .push(egui::ViewportCommand::Fullscreen(false));
                        self.viewport_commands
                            .push(egui::ViewportCommand::Maximized(true));
                    }
                    Restore::Placed(window) => {
                        self.viewport_commands
                            .push(egui::ViewportCommand::Fullscreen(false));
                        self.viewport_commands
                            .push(egui::ViewportCommand::OuterPosition(egui::pos2(
                                window.x as f32,
                                window.y as f32,
                            )));
                        self.viewport_commands
                            .push(egui::ViewportCommand::InnerSize(egui::vec2(
                                window.width as f32,
                                window.height as f32,
                            )));
                    }
                    Restore::Normal => {
                        self.viewport_commands
                            .push(egui::ViewportCommand::Fullscreen(false));
                    }
                }
            }
            _ => {}
        }
    }

    /// `S` and the filmstrip's toggle: "only the marked" on ↔ off (U3-Q3, T111).
    fn toggle_only_marked(&mut self) {
        let shell = &mut self.settings.shell;
        shell.filmstrip_marked = !shell.filmstrip_marked;
    }

    /// `J`, View ▸ Show Clipped Highlights and the Clipping section's check box: the overlay
    /// that marks the clipped highlights on the image, on ↔ off (ADR 0017 §2.6, U7b-Q5, T148).
    fn toggle_clip_highlights(&mut self) {
        let shell = &mut self.settings.shell;
        shell.clip_highlights = !shell.clip_highlights;
    }

    /// `Shift+J`, View ▸ Show Deep Shadows and the Clipping section's check box: the overlay
    /// that marks the deepest shadows on the image, on ↔ off (U7b-Q5, T148).
    fn toggle_clip_shadows(&mut self) {
        let shell = &mut self.settings.shell;
        shell.clip_shadows = !shell.clip_shadows;
    }

    /// `E` and View ▸ Show Focus Marking: the edges of high local contrast marked in green on the
    /// image, on ↔ off (ADR 0017 §2.6, U9-Q1, T168).
    fn toggle_focus_marking(&mut self) {
        let shell = &mut self.settings.shell;
        shell.focus_marking = !shell.focus_marking;
    }

    /// View ▸ Buttons on Thumbnails: the four buttons on a thumbnail in the contact sheet, on ↔
    /// off (ADR 0017 §2.8, U15-Q7, T245). The settings file is written when the program closes.
    fn toggle_thumbnail_buttons(&mut self) {
        let shell = &mut self.settings.shell;
        shell.thumbnail_buttons = !shell.thumbnail_buttons;
    }

    /// Tells the view cores whether a clipping map is wanted (U7b-Q8, T148): the first exactly
    /// when the preview is shown and at least one overlay is on, the second when the compare is
    /// open as well (U6-Q9: the marks on both sides, T153). Called every frame.
    fn wish_clipping_map(&self) {
        let shell = &self.settings.shell;
        let wanted = self.in_preview() && (shell.clip_highlights || shell.clip_shadows);
        self.core.set_clipping_wanted(wanted);
        self.second_core
            .set_clipping_wanted(wanted && self.compare.is_some());
    }

    /// Tells the view cores whether a focus map is wanted (U9a, T168): the first exactly when the
    /// preview is shown and the focus marking is on, the second when the compare is open as well
    /// (U9-Q4: the marks on both sides). Called every frame.
    fn wish_focus_map(&self) {
        let wanted = self.in_preview() && self.settings.shell.focus_marking;
        self.core.set_focus_wanted(wanted);
        self.second_core
            .set_focus_wanted(wanted && self.compare.is_some());
    }

    /// The indexes the compare would open with, `(held, current)` (U6-Q4, T151's
    /// `open_indexes` on the stepped list); `None` outside the preview, with fewer than two
    /// images in the stepped list, or when the current image is not in it.
    fn compare_open_indexes(&self) -> Option<(usize, usize)> {
        let only_marked = self.settings.shell.filmstrip_marked;
        let tab = self
            .tabs
            .active()
            .filter(|tab| tab.mode == ViewMode::Loupe)?;
        open_indexes(&tab.preview_list(only_marked), tab.current)
    }

    /// `C` and View ▸ Compare: opens the compare, or closes it (U6-Q4, U6-Q11, T153).
    fn toggle_compare(&mut self) {
        if self.compare.is_some() {
            self.close_compare();
        } else {
            self.open_compare();
        }
    }

    /// Opens the compare (U6-Q4): first the current image moves to the next of the stepped list
    /// (the previous at the end) as a filmstrip click moves it; then, only if that step happened,
    /// the image it left is held and given to the second view core. When the step did not happen
    /// (the refilter before it dropped the target), the compare stays closed (U6-Q11, T165).
    /// Nothing with fewer than two images in the stepped list.
    fn open_compare(&mut self) {
        let Some((held, current)) = self.compare_open_indexes() else {
            return;
        };
        let Some((held, folder)) = self
            .tabs
            .active()
            .and_then(|tab| Some((tab.files.get(held)?.clone(), tab.folder.clone())))
        else {
            return;
        };
        self.loupe_move(LoupeMove::To(current));
        if self.tabs.active().is_none_or(|tab| tab.current != current) {
            return;
        }
        self.hold_in_second_core(&held);
        self.compare = Some(CompareOpen {
            compare: Compare::new(held),
            tab: self.tabs.active_index(),
            folder,
            view_pending: true,
            mode_sent: None,
            view_sent: None,
            held_size_known: false,
        });
    }

    /// Closes the compare (U6-Q11): the current image stays in the preview, the second view core
    /// gets an empty list and its clipping marks are released (as the contact sheet does). Its
    /// dropped textures are taken back at the next sync of `second_textures`.
    fn close_compare(&mut self) {
        if self.compare.take().is_none() {
            return;
        }
        self.second_core.set_files(Vec::new(), 0);
        self.second_core.set_clipping_wanted(false);
        self.second_clip_overlay.release();
        self.second_core.set_focus_wanted(false);
        self.second_focus_overlay.release();
    }

    /// Closes the compare by itself (U6-Q11), checked every frame: when the preview is no longer
    /// shown, when the active tab's index or folder differs from those it was opened in, or when
    /// the held image is no longer among the tab's files.
    fn follow_compare(&mut self) {
        let Some(open) = &self.compare else {
            return;
        };
        let valid = self.tabs.active_index() == open.tab
            && self.tabs.active().is_some_and(|tab| {
                tab.mode == ViewMode::Loupe
                    && tab.folder == open.folder
                    && open.compare.still_valid(&tab.files)
            });
        if !valid {
            self.close_compare();
        }
    }

    /// Gives the second view core the held image, `held` as the tab names it: the path is
    /// resolved as the first core's are (`resolve_paths`), so the rating badge and the image
    /// check find it.
    fn hold_in_second_core(&self, held: &Path) {
        let path = match std::env::current_dir() {
            Ok(cwd) => resolve_path(&cwd, held),
            Err(_) => held.to_path_buf(),
        };
        self.second_core.set_files(vec![path], 0);
    }

    /// `K` and View ▸ Hold Current Image (U6-Q6, T153): the current image becomes the held one
    /// and the second view core gets it, with the current side's zoom and centre at the next
    /// frame. Nothing outside the compare, or when the current image is already held.
    fn hold_current_image(&mut self) {
        let Some(current) = self
            .tabs
            .active()
            .and_then(|tab| tab.files.get(tab.current).cloned())
        else {
            return;
        };
        let Some(open) = &mut self.compare else {
            return;
        };
        if open.compare.held() == current {
            return;
        }
        open.compare.hold(&current);
        open.view_pending = true;
        open.view_sent = None;
        open.held_size_known = false;
        self.hold_in_second_core(&current);
    }

    /// `Shift+K` and View ▸ Swap Images (U6-Q6, T153): T151's swap on the stepped list. First the
    /// current image moves to the held image's index as a filmstrip click moves it; then, only if
    /// that step happened, the old current image becomes the held one and is given to the second
    /// view core. When the step did not happen (the refilter before it dropped the target), the
    /// held image stays as it was (U6-Q11, T165). Nothing outside the compare or when
    /// `swap_target` gives `None` (the held image is not in the stepped list, or is the current
    /// image).
    fn swap_images(&mut self) {
        let only_marked = self.settings.shell.filmstrip_marked;
        let Some(tab) = self.tabs.active() else {
            return;
        };
        let Some(open) = &self.compare else {
            return;
        };
        let list: Vec<(usize, &Path)> = tab
            .preview_list(only_marked)
            .into_iter()
            .filter_map(|index| Some((index, tab.files.get(index)?.as_path())))
            .collect();
        let Some((to, new_held)) = open.compare.swap_target(&list, tab.current) else {
            return;
        };
        self.loupe_move(LoupeMove::To(to));
        if self.tabs.active().is_none_or(|tab| tab.current != to) {
            return;
        }
        let Some(open) = &mut self.compare else {
            return;
        };
        open.compare.hold(&new_held);
        open.view_pending = true;
        open.view_sent = None;
        open.held_size_known = false;
        self.hold_in_second_core(&new_held);
    }

    /// `V` and View ▸ Above and Below (U6-Q7, T153): the compare's layout switched and
    /// remembered (`compare_above_below`). Nothing outside the compare.
    fn toggle_compare_layout(&mut self) {
        if self.compare.is_some() {
            let shell = &mut self.settings.shell;
            shell.compare_above_below = !shell.compare_above_below;
        }
    }

    /// `L` and View ▸ Lock Zoom and Position (U6-Q8, T154): the lock on ↔ off, remembered
    /// (`compare_locked`). Turned on, what was sent to the held side is forgotten, so it takes the
    /// current side's view at the next frame. Nothing outside the compare.
    fn toggle_compare_lock(&mut self) {
        let Some(open) = &mut self.compare else {
            return;
        };
        let shell = &mut self.settings.shell;
        shell.compare_locked = !shell.compare_locked;
        if shell.compare_locked {
            open.view_sent = None;
        }
    }

    /// The image area's two halves while comparing, `(held, current)`, in the remembered layout
    /// (U6-Q5, U6-Q7, T151's `sides`); `None` outside the compare.
    fn compare_sides(&self, area: egui::Rect) -> Option<(egui::Rect, egui::Rect)> {
        self.compare.as_ref()?;
        let layout = if self.settings.shell.compare_above_below {
            Layout::AboveBelow
        } else {
            Layout::SideBySide
        };
        Some(sides(area, layout))
    }

    /// The thumbnail worker in the preview (§5, U3-Q4, T111): it runs only while the filmstrip is
    /// shown and the shown image is drawn (`thumbnail_gate`); a step pauses it until the new image
    /// is drawn. Called at the end of the frame, with the status of the frame just drawn.
    fn gate_thumbnails(&mut self) {
        if !self.in_preview() {
            return;
        }
        let run = preview_parts(&self.settings.shell).filmstrip
            && thumbnail_gate(&self.last_frame_status) == ThumbGate::Run;
        if run {
            self.thumb_worker.resume();
        } else {
            self.thumb_worker.pause();
        }
    }

    /// The preview (ADR 0017 §2.2, U3-Q1, T111): only the title line (today's loupe status line
    /// with the RAW/JPEG control), the image area, the preview's right column and the filmstrip
    /// under both; `H` leaves the title line and the image.
    fn draw_preview(&mut self, ui: &mut egui::Ui, blocked: bool, commands: &mut Vec<Command>) {
        let parts = preview_parts(&self.settings.shell);
        egui::Panel::top("preview-title").show(ui, |ui| {
            self.draw_status_bar(ui, blocked, commands);
        });
        if parts.filmstrip {
            egui::Panel::bottom("filmstrip")
                .resizable(false)
                .show(ui, |ui| self.draw_filmstrip(ui, blocked));
        }
        match parts.column {
            Some(ColumnState::Open) => self.draw_preview_column(ui, blocked),
            // The strip is drawn whenever the column is collapsed; its button opens the column.
            Some(ColumnState::Collapsed)
                if collapsed_column(ui, "preview-column-collapsed", false) && !blocked =>
            {
                self.settings.shell.preview_column = ColumnState::Open;
            }
            Some(ColumnState::Collapsed) | None => {}
        }
        let fill = ui.visuals().panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(fill))
            .show(ui, |ui| self.draw(ui, commands));
    }

    /// The preview's right column (§2.2, U3-Q2, U3-Q5, T111): resizable and collapsible as the
    /// main window's columns, with the shown image's file name, Info (T110's Exif rows from the
    /// same reader), Zoom (the zoom as a text, and buttons and a check box that call the same
    /// function as their keys), Histogram (the shown image's histogram and exposure
    /// statistics, §2.6, U7-Q7, T142) and Clipping (the two overlays' check boxes, which call the
    /// same functions as their keys, and the state of the clipping map, U7b-Q9, T148). After a
    /// click no widget keeps the keyboard.
    fn draw_preview_column(&mut self, ui: &mut egui::Ui, blocked: bool) {
        let shell = self.settings.shell.clone();
        let window_width = ui.ctx().content_rect().width();
        let (min, max) = column_width_range(window_width);
        let cwd = std::env::current_dir().ok();
        let image = inspector_subject(self.tabs.active(), cwd.as_deref());
        let zoom = self.last_zoom;
        let histogram = self.histogram_for(image.as_deref());
        let mut anchor_at_pointer = self.anchor_at_pointer;
        // The check boxes show the overlays as they are; a click is done after the column is
        // drawn, by the keys' functions. The line under them is the last frame's clipping state,
        // and nothing when both overlays are off.
        let mut clip = [shell.clip_highlights, shell.clip_shadows];
        let mut clip_clicked = [false; 2];
        let clipping_state = if clip.contains(&true) {
            clipping_state_text(&self.last_clipping)
        } else {
            None
        };
        let exif = &self.exif;
        let mut actions = Vec::new();
        let mut toggled = [false; 4];
        let shown = egui::Panel::right("preview-column")
            .resizable(true)
            .default_size(clamp_column_width(
                shell.preview_column_width as f32,
                window_width,
            ))
            .size_range(min..=max)
            .show(ui, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    let collapse = ui
                        .horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.small_button(shell_text(ShellText::CollapseRight))
                                    .on_hover_text(preview_text(PreviewText::CollapseColumn))
                                    .clicked()
                            })
                            .inner
                        })
                        .inner;
                    egui::ScrollArea::vertical()
                        .id_salt("preview-column-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let Some(path) = image.as_deref() else {
                                column_note(ui, &inspector_text(&InspectorText::NoImage));
                                return;
                            };
                            let file_name = path.file_name().map_or_else(
                                || path.display().to_string(),
                                |name| name.to_string_lossy().into_owned(),
                            );
                            ui.add(
                                egui::Label::new(egui::RichText::new(file_name).strong()).wrap(),
                            );
                            ui.add_space(PANEL_ROW_SPACING);
                            let info =
                                egui::CollapsingHeader::new(inspector_text(&InspectorText::Info))
                                    .id_salt("preview-info")
                                    .open(Some(shell.preview_info == ColumnState::Open))
                                    .show(ui, |ui| draw_exif_rows(ui, &exif.shown(path)));
                            let zoom_section =
                                egui::CollapsingHeader::new(preview_text(PreviewText::ZoomSection))
                                    .id_salt("preview-zoom")
                                    .open(Some(shell.preview_zoom == ColumnState::Open))
                                    .show(ui, |ui| {
                                        if let Some((zoom, scale)) = zoom {
                                            ui.add(
                                                egui::Label::new(zoom_value_text(zoom, scale))
                                                    .wrap(),
                                            );
                                        }
                                        ui.horizontal_wrapped(|ui| {
                                            for (text, action) in [
                                                (PreviewText::Fit, Action::Fit),
                                                (PreviewText::ToggleZoom, Action::ToggleZoom),
                                                (PreviewText::ZoomIn, Action::ZoomStep(1)),
                                                (PreviewText::ZoomOut, Action::ZoomStep(-1)),
                                            ] {
                                                if ui.button(preview_text(text)).clicked() {
                                                    actions.push(action);
                                                }
                                            }
                                        });
                                        if ui
                                            .checkbox(
                                                &mut anchor_at_pointer,
                                                preview_text(PreviewText::ZoomAtPointer),
                                            )
                                            .changed()
                                        {
                                            actions.push(Action::ToggleAnchor);
                                        }
                                    });
                            let histogram_section =
                                egui::CollapsingHeader::new(histogram_text(HistogramText::Section))
                                    .id_salt("preview-histogram")
                                    .open(Some(shell.preview_histogram == ColumnState::Open))
                                    .show(ui, |ui| draw_histogram(ui, &histogram));
                            let clipping_section =
                                egui::CollapsingHeader::new(clipping_text(ClippingText::Section))
                                    .id_salt("preview-clipping")
                                    .open(Some(shell.preview_clipping == ColumnState::Open))
                                    .show(ui, |ui| {
                                        for ((on, clicked), text) in clip
                                            .iter_mut()
                                            .zip(clip_clicked.iter_mut())
                                            .zip([ClippingText::Highlights, ClippingText::Shadows])
                                        {
                                            *clicked =
                                                ui.checkbox(on, clipping_text(text)).changed();
                                        }
                                        if let Some(text) = clipping_state {
                                            ui.add(egui::Label::new(text).wrap());
                                        }
                                    });
                            toggled = [
                                info.header_response.clicked(),
                                zoom_section.header_response.clicked(),
                                histogram_section.header_response.clicked(),
                                clipping_section.header_response.clicked(),
                            ];
                        });
                    collapse
                })
                .inner
            });
        self.settings.shell.preview_column_width =
            shown.response.rect.width().round().max(0.0) as u32;
        if blocked {
            return;
        }
        let clicked = shown.inner
            || toggled.contains(&true)
            || !actions.is_empty()
            || clip_clicked.contains(&true);
        if shown.inner {
            self.settings.shell.preview_column = ColumnState::Collapsed;
        }
        if toggled[0] {
            self.settings.shell.preview_info = self.settings.shell.preview_info.toggled();
        }
        if toggled[1] {
            self.settings.shell.preview_zoom = self.settings.shell.preview_zoom.toggled();
        }
        if toggled[2] {
            self.settings.shell.preview_histogram = self.settings.shell.preview_histogram.toggled();
        }
        if toggled[3] {
            self.settings.shell.preview_clipping = self.settings.shell.preview_clipping.toggled();
        }
        // The check boxes call the same functions as J and Shift+J.
        if clip_clicked[0] {
            self.toggle_clip_highlights();
        }
        if clip_clicked[1] {
            self.toggle_clip_shadows();
        }
        // Each button and the check box call the same function as their key; a zoom uses the
        // image's centre, as from the menu (T109).
        for action in actions {
            self.act(action, None);
        }
        if clicked {
            let ctx = ui.ctx();
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
        }
    }

    /// The filmstrip (§2.2, §5, U3-Q3, U3-Q4, T111): the toggle "only the marked" at its left
    /// end, then one row of cells of the tab's visible list (or only the marked images) in the
    /// contact sheet's order, drawn as the contact sheet draws them, scrolled so the current image
    /// is in view after every step; the mouse wheel scrolls it sideways and a click shows that
    /// image (a step: the marking is kept). Thumbnails come from the contact sheet's worker, grid
    /// and textures, for the cells in view plus a margin, and only while the thumbnail gate lets
    /// the worker run.
    fn draw_filmstrip(&mut self, ui: &mut egui::Ui, blocked: bool) {
        let ctx = ui.ctx().clone();
        let only_marked = self.settings.shell.filmstrip_marked;
        let Some((current, list)) = self
            .tabs
            .active()
            .map(|tab| (tab.current, tab.preview_list(only_marked)))
        else {
            return;
        };
        let count = list.len();
        let cwd = std::env::current_dir().ok();
        // The held image of the compare, whose cell gets a dashed frame (U6-Q10, T153).
        let held = self
            .compare
            .as_ref()
            .map(|open| open.compare.held().to_path_buf());
        let mut toggle = false;
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!blocked, |ui| {
                toggle = ui
                    .selectable_label(only_marked, preview_text(PreviewText::OnlyMarked))
                    .on_hover_text(preview_text(PreviewText::OnlyMarkedHover))
                    .clicked();
            });
            let height = FILMSTRIP_IMAGE_HEIGHT + FILMSTRIP_LABEL_HEIGHT;
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().max(0.0), height),
                egui::Sense::click(),
            );
            let width = rect.width();
            // Scrolled to the current image after every step, the list's change included; the
            // wheel scrolls sideways in between.
            let followed = (current, count);
            if self.filmstrip_followed != Some(followed) {
                self.filmstrip_followed = Some(followed);
                if let Some(position) = list.iter().position(|&index| index == current) {
                    self.filmstrip_offset =
                        scroll_target(count, position, self.filmstrip_offset, width);
                }
            }
            if response.hovered() && !blocked {
                let delta = ctx.input(|input| input.smooth_scroll_delta);
                self.filmstrip_offset -= delta.x + delta.y;
            }
            self.filmstrip_offset = clamp_offset(self.filmstrip_offset, count, width);
            let offset = self.filmstrip_offset;

            // The thumbnails of the cells in view plus a margin (§5), only while the worker may
            // run (the status of the last drawn frame; the gate is set again after this frame).
            let keep = filmstrip_range(count, offset, width, FILMSTRIP_MARGIN);
            let keep_files = list.get(keep).unwrap_or(&[]);
            // Only these cells are copied out of the tab (file index, path, marked), not its
            // whole list of files: this runs on every frame of a step.
            let cells: Vec<(usize, PathBuf, bool)> = self
                .tabs
                .active()
                .map(|tab| {
                    keep_files
                        .iter()
                        .filter_map(|&index| {
                            let path = tab.files.get(index)?.clone();
                            Some((index, path, tab.selection.is_marked(index)))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let cell_of = |index: usize| cells.iter().find(|(file, _, _)| *file == index);
            if thumbnail_gate(&self.last_frame_status) == ThumbGate::Run {
                self.thumb_worker
                    .set_visible(index_span(keep_files.iter().copied()));
                // The contact sheet's cell size, so the same cached thumbnails are used.
                let cell = request_cell(self.thumb_size, ctx.pixels_per_point());
                for index in self.thumb_grid.to_request_indexes(keep_files, cell) {
                    if let Some((_, path, _)) = cell_of(index) {
                        if self.added_files.contains(path) {
                            self.added_files.note_requested(path, file_stamp(path));
                        }
                        let orientation = read_orientation(&self.metadata, cwd.as_deref(), path);
                        self.thumb_worker.request(ThumbnailRequest {
                            generation: self.thumb_generation,
                            index,
                            path: path.clone(),
                            cell,
                            orientation,
                        });
                        self.thumb_grid.mark_requested(index, cell);
                    }
                }
                self.poll_thumbnails(&ctx);
                for index in self.thumb_grid.forget_not_in(keep_files) {
                    self.thumb_textures.remove(&index);
                }
                if keep_files
                    .iter()
                    .any(|&index| matches!(self.thumb_grid.state(index), CellState::Requested))
                {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, BACKGROUND);
            let stride = FILMSTRIP_CELL_WIDTH + FILMSTRIP_SPACING;
            let shown = filmstrip_range(count, offset, width, 0);
            for (position, &index) in shown.clone().zip(list.get(shown).unwrap_or(&[])) {
                let Some((_, path, marked)) = cell_of(index) else {
                    continue;
                };
                let left = rect.min.x + position as f32 * stride - offset;
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(left, rect.min.y),
                    egui::vec2(FILMSTRIP_CELL_WIDTH, height),
                );
                let image_rect = egui::Rect::from_min_size(
                    cell_rect.min,
                    egui::vec2(FILMSTRIP_CELL_WIDTH, FILMSTRIP_IMAGE_HEIGHT),
                );
                let label_rect = egui::Rect::from_min_max(
                    egui::pos2(cell_rect.min.x, image_rect.max.y),
                    cell_rect.max,
                );
                paint_cell(
                    &painter,
                    CellRects {
                        cell: cell_rect,
                        image: image_rect,
                        label: label_rect,
                    },
                    path,
                    shown_rating(&self.metadata, cwd.as_deref(), path),
                    *marked,
                    index == current,
                    self.thumb_grid.state(index),
                    self.thumb_textures.get(&index),
                );
                if held.as_deref() == Some(path.as_path()) {
                    let frame = cell_rect.shrink(HELD_FRAME_INSET);
                    painter.extend(egui::Shape::dashed_line(
                        &[
                            frame.left_top(),
                            frame.right_top(),
                            frame.right_bottom(),
                            frame.left_bottom(),
                            frame.left_top(),
                        ],
                        egui::Stroke::new(2.0, MARKED_BLUE),
                        HELD_FRAME_DASH,
                        HELD_FRAME_GAP,
                    ));
                }
            }
            if response.clicked()
                && !blocked
                && let Some(pointer) = response.interact_pointer_pos()
            {
                clicked = hit_test(count, offset, pointer.x - rect.min.x)
                    .and_then(|position| list.get(position).copied());
            }
        });
        if toggle {
            self.toggle_only_marked();
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
        }
        // A click is a step to that image (point 7 of T111): it never changes the marking.
        if let Some(index) = clicked
            && index != current
        {
            self.loupe_move(LoupeMove::To(index));
            ctx.request_repaint();
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

    /// The images a rating key or a move acts on: the marked images in the grid, else the current
    /// one (ADR 0014 §2.6, §2.7), as the tab names them; none when the current image is not shown.
    fn key_targets(&self) -> Vec<PathBuf> {
        self.tabs
            .active()
            .filter(|tab| tab.position().is_some())
            .map(|tab| {
                tab.selection
                    .targets(&tab.visible, tab.current, tab.mode)
                    .into_iter()
                    .filter_map(|index| tab.files.get(index).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A rating or label key, or its menu item: the marked images in the grid, else the current
    /// one (ADR 0014 §2.6); nothing when the current image is not shown. An accepted key removes
    /// the images that stopped matching at once (§2.2, amendment 1); a refused key changes
    /// nothing. Returns `Some` with the position the contact sheet scrolls to when the visible
    /// list changed, and, for X with "Move Rejected and Picked Images to Subfolders" on (ADR 0017
    /// U9-Q6, T174), the images to move: those now rejected that have a `_rejected` target, each
    /// with its rating before the key. An X that clears a reject moves nothing.
    fn rate(&mut self, action: RatingKey) -> (Option<Option<usize>>, MoveImages) {
        let paths = self.key_targets();
        if paths.is_empty() {
            return (None, Vec::new());
        }
        // The targets as the metadata cache keys them, with their ratings before the key.
        let before: Vec<(PathBuf, Rating)> =
            if action == RatingKey::Reject && self.settings.shell.move_to_subfolders {
                resolve_paths(&paths)
                    .map(|resolved| {
                        resolved
                            .into_iter()
                            .map(|path| {
                                let rating = self.metadata.current_rating(&path);
                                (path, rating)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
        match self.metadata.set_ratings(&paths, action) {
            Ok(_) => {
                let moves = before
                    .into_iter()
                    .filter(|(path, _)| self.metadata.current_rating(path).stars == Some(-1))
                    .filter_map(|(path, rating)| {
                        target_folder(&path, MoveKind::Reject)
                            .map(|target| (path, target, Some(rating)))
                    })
                    .collect();
                return (self.after_rating(), moves);
            }
            Err(RatingRefused::Unsupported) => {
                self.message = Some(UNSUPPORTED_TEXT.to_owned());
            }
            Err(error) => self.message = Some(error.to_string()),
        }
        (None, Vec::new())
    }

    /// Ctrl+← or Ctrl+→, or Image ▸ Rotate Left or Rotate Right (ADR 0024 §2.1, §2.5, O-Q1, O-Q8;
    /// T237): the targets of a rating key turn 90° anticlockwise or clockwise (`clockwise`), shown
    /// at once and written through the ordered queue; nothing when there is no target. The
    /// images whose own value is not known yet are read by the orientation reader (on the UI
    /// thread when it has stopped). A turn changes no marking, filter or current image, and
    /// moves nothing; a refused turn says why.
    fn rotate(&mut self, ctx: &egui::Context, clockwise: bool) {
        let targets = self.key_targets();
        if targets.is_empty() {
            return;
        }
        match self.metadata.turn(&targets, clockwise) {
            Ok(asks) => {
                for path in asks {
                    if self.orientation_reader.request(path.clone()) {
                        self.orientation_reads = self.orientation_reads.saturating_add(1);
                    } else {
                        self.metadata
                            .embedded_read(&path, embedded_orientation(&path));
                    }
                }
            }
            Err(RatingRefused::Unsupported) => {
                self.message = Some(ROTATION_UNSUPPORTED_TEXT.to_owned());
            }
            Err(error) => self.message = Some(error.to_string()),
        }
        // Keys are handled after `follow_orientations`: the turn reaches the view and the
        // thumbnail in the next frame, at once.
        ctx.request_repaint();
    }

    /// P or Image ▸ Move to Picked (ADR 0017 §2.6, U9-Q6, T174): with the setting on, the key's
    /// targets with a `_picked` target are moved there, with no rating change; with it off,
    /// nothing.
    fn move_picked(&mut self) {
        if !self.settings.shell.move_to_subfolders {
            return;
        }
        let paths = self.key_targets();
        if paths.is_empty() {
            return;
        }
        match resolve_paths(&paths) {
            Ok(resolved) => {
                let images = resolved
                    .into_iter()
                    .filter_map(|path| {
                        target_folder(&path, MoveKind::Pick).map(|target| (path, target, None))
                    })
                    .collect();
                self.queue_moves(MoveKind::Pick, images);
            }
            Err(error) => self.message = Some(error),
        }
    }

    /// A batch of moves of `kind` waits behind the running batch or undo; it is sent by
    /// `follow_moves`. An image a waiting or the running batch holds already is left out (T178
    /// point 7), and nothing is queued for no images. A running undo is not checked: its images
    /// are in `_rejected` or `_picked`, where `target_folder` gives no target.
    fn queue_moves(&mut self, kind: MoveKind, images: MoveImages) {
        let running = match &self.moves.running {
            Some(MoveRun::Batch { batch, .. }) => Some(batch),
            _ => None,
        };
        let images: MoveImages = images
            .into_iter()
            .filter(|(image, _, _)| {
                !running
                    .into_iter()
                    .chain(&self.moves.waiting)
                    .any(|batch| batch.contains(image))
            })
            .collect();
        if !images.is_empty() {
            self.moves.waiting.push_back(Batch::new(kind, images));
        }
    }

    /// Image ▸ "Move Rejected and Picked Images to Subfolders" on ↔ off (U9-Q5, T174), saved at
    /// once as the sort is.
    fn toggle_move_to_subfolders(&mut self) {
        self.settings.shell.move_to_subfolders = !self.settings.shell.move_to_subfolders;
        if let Some(path) = &self.settings_path
            && let Err(error) =
                save_settings_with_library(path, &self.settings, &self.library.saved())
        {
            eprintln!("pw-loupe: the move setting could not be saved: {error}");
        }
    }

    /// Ctrl+Z or Edit ▸ Undo Move (U9-Q8, T174): the newest step of the undo list is moved back
    /// by `follow_moves`. While a batch or an undo runs or waits, the status line says so and
    /// nothing is queued (U9-Q11, T183); nothing to undo shows so.
    fn undo_move(&mut self) {
        // Nothing is moved back while a delete is under way (ADR 0018 §2.10, T189 point 7).
        if self.delete.busy() {
            self.message = Some(delete_running_text().to_owned());
            return;
        }
        if self.moves.busy() {
            self.message = Some(move_running_text().to_owned());
            return;
        }
        match self.moves.undo.pop() {
            Some(step) => {
                self.moves.running = Some(MoveRun::Undo(
                    step.into_iter()
                        .map(|moved| UndoImage {
                            moved,
                            id: None,
                            answer: None,
                        })
                        .collect(),
                ));
            }
            None => self.message = Some(nothing_to_undo_text().to_owned()),
        }
    }

    /// The moves' follow-up of a frame, right after the metadata cache's `poll` (T174): the
    /// answers go to the running batch or undo (an answer of another request is ignored); a
    /// finished one is acted on and the next waiting batch starts; the images not yet sent are
    /// sent. A frame is asked for while one runs.
    fn follow_moves(&mut self, ctx: &egui::Context) {
        for answer in self.metadata.take_moves() {
            match (answer, &mut self.moves.running) {
                (MoveAnswer::Moved { id, image, result }, Some(MoveRun::Batch { batch, ids }))
                    if ids.contains(&id) =>
                {
                    batch.answered(&image, result)
                }
                (MoveAnswer::MovedBack { id, result, .. }, Some(MoveRun::Undo(images))) => {
                    if let Some(entry) = images
                        .iter_mut()
                        .find(|entry| entry.id == Some(id) && entry.answer.is_none())
                    {
                        entry.answer = Some(result);
                    }
                }
                _ => {}
            }
        }
        loop {
            if self.moves.running.is_none() {
                // A waiting batch waits while a delete is under way (ADR 0018 §2.10, T189).
                if self.delete.busy() {
                    break;
                }
                match self.moves.waiting.pop_front() {
                    Some(batch) => {
                        self.moves.running = Some(MoveRun::Batch {
                            batch,
                            ids: HashSet::new(),
                        });
                    }
                    None => break,
                }
            }
            self.send_moves();
            if !self.moves.running.as_ref().is_some_and(MoveRun::done) {
                break;
            }
            match self.moves.running.take() {
                Some(MoveRun::Batch { batch, .. }) => self.batch_done(ctx, batch),
                Some(MoveRun::Undo(images)) => self.undo_done(ctx, images),
                None => break,
            }
        }
        if self.moves.busy() {
            ctx.request_repaint_after(MOVE_POLL_INTERVAL);
        }
    }

    /// Sends what the running batch or undo has not sent yet (T174 point 4): a batch's image waits
    /// while a rating write for it is queued or sent without its answer (`Batch::to_send` with
    /// `is_pending`, T178 point 6; the answers, and a failed write's rollback, are applied in the
    /// metadata cache's `poll`, before this); an image of an X batch that is no longer rejected
    /// (its write failed, or a later key cleared the reject) is skipped, with no message; a full
    /// queue leaves the rest for the next frame; a stopped worker answers at once as failed.
    fn send_moves(&mut self) {
        let worker = &mut self.metadata_worker;
        match &mut self.moves.running {
            Some(MoveRun::Batch { batch, ids }) => {
                let metadata = &self.metadata;
                let extensions: Vec<String> = pw_raw::SUPPORTED_EXTENSIONS
                    .iter()
                    .map(|extension| (*extension).to_owned())
                    .collect();
                for (image, target) in batch.to_send(|path| metadata.is_pending(path)) {
                    if batch.kind() == MoveKind::Reject
                        && metadata.current_rating(&image).stars != Some(-1)
                    {
                        batch.skip(&image);
                        continue;
                    }
                    match MetadataSubmit::move_image(
                        worker,
                        image.clone(),
                        target,
                        extensions.clone(),
                        self.settings.shell.companion_suffixes.clone(),
                    ) {
                        Ok(id) => {
                            ids.insert(id);
                            batch.sent(&image);
                        }
                        Err(pw_metadata::SubmitError::QueueFull) => break,
                        Err(pw_metadata::SubmitError::Stopped) => {
                            batch.answered(&image, Err(worker_stopped_text().to_owned()));
                        }
                    }
                }
            }
            Some(MoveRun::Undo(images)) => {
                for entry in images
                    .iter_mut()
                    .filter(|entry| entry.id.is_none() && entry.answer.is_none())
                {
                    match MetadataSubmit::move_back(worker, entry.moved.moved.clone()) {
                        Ok(id) => entry.id = Some(id),
                        Err(pw_metadata::SubmitError::QueueFull) => break,
                        Err(pw_metadata::SubmitError::Stopped) => {
                            entry.answer = Some(Err(worker_stopped_text().to_owned()));
                        }
                    }
                }
            }
            None => {}
        }
    }

    /// Gives every tab of each of `folders` a new list of files (ADR 0017 U9b, T174): `list` makes
    /// a tab's new files, in the tab's own form, and the file to make current. The active tab,
    /// found by `same_folder`, gets the real `ratings_ready`, the others `false` (as
    /// `Tabs::follow_sort` does). Returns whether a folder was the active tab's, and the errors.
    fn replace_tab_files(
        &mut self,
        folders: &[PathBuf],
        list: impl Fn(&Tab) -> Result<(Vec<PathBuf>, Option<PathBuf>), String>,
    ) -> (bool, Vec<String>) {
        let ready = self.ratings_ready();
        let active_folder = self.tabs.active().and_then(|tab| tab.folder.clone());
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        let mut active = false;
        let mut errors = Vec::new();
        for folder in folders {
            let is_active = active_folder
                .as_deref()
                .is_some_and(|open| same_folder(open, folder));
            for tab in self.tabs.tabs_of_folder_mut(folder) {
                match list(tab) {
                    Ok((files, current)) => {
                        tab.replace_files(
                            files,
                            current.as_deref(),
                            is_active && ready,
                            &rating_of,
                        );
                    }
                    Err(error) => errors.push(error),
                }
            }
            active |= is_active;
        }
        (active, errors)
    }

    /// Lists every tab of each of `folders` again with `Tab::relist` (ADR 0026 §2 point 4, the
    /// owner 2026-11-21, T242): the files still there keep their order and the new ones are
    /// placed in the sort (or wait for their key), instead of the whole order being applied
    /// again. `current` (a resolved path: the image an undo brings back) is made current in each
    /// tab that lists it, also when the listing brought nothing new (`relist_tab`). The active
    /// tab's folder, found by `same_folder`, gets the real `ratings_ready`, the others `false`
    /// (as `replace_tab_files`), and the contact sheet's note becomes the anchor first. A tab
    /// without a folder keeps its files; a listing that fails leaves its tab as it was. Returns
    /// whether a folder was the active tab's, and the errors.
    fn relist_folder_tabs(
        &mut self,
        folders: &[PathBuf],
        current: Option<&Path>,
    ) -> (bool, Vec<String>) {
        let active_folder = self.tabs.active().and_then(|tab| tab.folder.clone());
        let is_active = |folder: &Path| {
            active_folder
                .as_deref()
                .is_some_and(|open| same_folder(open, folder))
        };
        let active = folders.iter().any(|folder| is_active(folder));
        if active {
            self.anchor_from_note();
        }
        let ready = self.ratings_ready();
        let order = self.settings.shell.sort;
        let cwd = std::env::current_dir().ok();
        let rating_of = tab_rating(&self.metadata, cwd.as_deref());
        let pending = tab_pending(&self.metadata, cwd.as_deref());
        let mut errors = Vec::new();
        for folder in folders {
            let ready = is_active(folder) && ready;
            for tab in self.tabs.tabs_of_folder_mut(folder) {
                let Some(tab_folder) = tab.folder.clone() else {
                    continue;
                };
                // While an import runs, the images it has not shown yet are left out (T260).
                let listed = match list_folder(&tab_folder) {
                    Ok((files, _)) => leave_out_waiting(self.import_tabs.as_ref(), files),
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                // The image to make current, in the tab's own form.
                let tab_current = match current {
                    Some(current) => match resolve_paths(&listed) {
                        Ok(resolved) => listed
                            .iter()
                            .zip(&resolved)
                            .find(|(_, path)| path.as_path() == current)
                            .map(|(file, _)| file.clone()),
                        Err(error) => {
                            errors.push(error);
                            continue;
                        }
                    },
                    None => None,
                };
                relist_tab(
                    tab,
                    listed,
                    tab_current.as_deref(),
                    &pending,
                    order,
                    ready,
                    &rating_of,
                    &modified_time,
                );
            }
        }
        (active, errors)
    }

    /// Lists every tab open on one of `folders` again (ADR 0017 U9b, T183): after a batch or an
    /// undo, the tabs of the `_rejected` and `_picked` folders the files went to or left. Listing
    /// again, not dropping single files, also covers a folder spelt differently from the moved
    /// paths and files a failed move back left behind. Each tab gets the listing through
    /// `relist_folder_tabs` (ADR 0026 §2 point 4, T242): the files it shows keep their order and
    /// the new ones are placed in the sort; no file is made current; a tab without a folder keeps
    /// its files. Then the active tab is loaded again when it was one of them (its new files'
    /// ratings are not in the metadata cache), and the sort is followed. With no tab open on any
    /// of `folders`, nothing is done. Returns the errors.
    fn relist_tabs(&mut self, ctx: &egui::Context, folders: &[PathBuf]) -> Vec<String> {
        let open: Vec<PathBuf> = folders
            .iter()
            .filter(|folder| {
                self.tabs.tabs().iter().any(|tab| {
                    tab.folder
                        .as_deref()
                        .is_some_and(|open| same_folder(open, folder))
                })
            })
            .cloned()
            .collect();
        if open.is_empty() {
            return Vec::new();
        }
        let (active, errors) = self.relist_folder_tabs(&open, None);
        if active {
            self.load_active_tab();
        }
        self.follow_sort(ctx);
        errors
    }

    /// A batch of moves has all its answers (T174 point 5): every tab of the moved images' folders
    /// drops the moved files, the active one's view cores and thumbnails follow, the tabs open on
    /// the folders the files went to are listed again (T183), the step goes on the undo list, and
    /// the failures are shown and listed. A failed image keeps its new rating. With nothing to
    /// show and no other batch waiting (`running` is already taken, so `busy` means a waiting
    /// batch), the text of Ctrl+Z pressed while the moves ran is cleared; while one waits, it
    /// stays, since Ctrl+Z still does nothing.
    fn batch_done(&mut self, ctx: &egui::Context, batch: Batch) {
        let result = batch.finish();
        let moved_from: HashSet<PathBuf> = result
            .moved
            .iter()
            .flat_map(|image| image.moved.iter().map(|moved| moved.from.clone()))
            .collect();
        let folders = image_folders(result.moved.iter().map(|image| image.image.as_path()));
        let (active, mut errors) = self.replace_tab_files(&folders, |tab| {
            let resolved = resolve_paths(&tab.files)?;
            let files = tab
                .files
                .iter()
                .zip(resolved)
                .filter(|(_, path)| !moved_from.contains(path))
                .map(|(file, _)| file.clone())
                .collect();
            Ok((files, None))
        });
        if active {
            self.active_reordered();
        }
        errors.extend(self.relist_tabs(ctx, &moved_to_folders(&result.moved)));
        self.moves.undo.push(result.moved);
        let now = ClockTime::now();
        for error in &errors {
            self.tasks.add_error(error, now);
        }
        for (image, reason) in &result.failed {
            self.tasks
                .add_error(&move_failed_text(&image_name(image), reason), now);
        }
        match result.failed.as_slice() {
            [] => {
                if let Some(error) = errors.last() {
                    self.message = Some(error.clone());
                } else if !self.moves.busy() && self.message.as_deref() == Some(move_running_text())
                {
                    self.message = None;
                }
            }
            [(image, reason)] => {
                self.message = Some(move_failed_text(&image_name(image), reason));
            }
            [.., (image, reason)] => {
                self.message = Some(moves_failed_text(
                    result.failed.len(),
                    &image_name(image),
                    reason,
                ));
            }
        }
    }

    /// An undo has all its answers (T174 point 6): the ratings before an X are restored, every
    /// tab of the images' folders lists its folder again with the first restored image current
    /// (`relist_folder_tabs`: the files brought back are placed in the sort and nothing else
    /// moves, ADR 0026 §2 point 4, T242), and the active tab's view cores and thumbnails follow (its
    /// ratings are read again when a file of the tab is not in the metadata cache, T251), and the
    /// contact sheet always scrolls to the image brought back (the owner 2026-11-21, T242 point
    /// 9.2); then the tabs open on the folders the files left are listed again (T183), keeping
    /// their top image, while the undo's scroll wins in its frame. A failed image stays where
    /// it is, and the step is not pushed back. The status line is set last.
    fn undo_done(&mut self, ctx: &egui::Context, images: Vec<UndoImage>) {
        let mut restored = Vec::new();
        let mut failed = Vec::new();
        for entry in images {
            match entry.answer {
                Some(Ok(())) => restored.push(entry.moved),
                Some(Err(reason)) => failed.push((entry.moved.image, reason)),
                None => {}
            }
        }
        let mut errors = Vec::new();
        for image in &restored {
            if let Some(rating) = &image.restore
                && let Err(error) = self.metadata.restore_rating(&image.image, rating.clone())
            {
                errors.push(error.to_string());
            }
        }
        let folders = image_folders(restored.iter().map(|image| image.image.as_path()));
        let first = restored.first().map(|image| image.image.clone());
        // The files brought back are placed in each tab's order and nothing else moves (ADR 0026
        // §2 point 4, T242); a file whose key is not read yet waits at the end.
        let (active, list_errors) = self.relist_folder_tabs(&folders, first.as_deref());
        errors.extend(list_errors);
        if active {
            // The relist listed the folder from disk, so it may also have brought in files
            // another program added while the listing was held (T251): any file of the tab not
            // in the metadata cache means its ratings are read again.
            let unknown = self.tabs.active().is_some_and(|tab| {
                resolve_paths(&tab.files).map_or(true, |files| {
                    files.iter().any(|file| !self.metadata.has_image(file))
                })
            });
            if unknown {
                // The tab was loaded again since the move, or a file came: its ratings are read
                // again.
                self.load_active_tab();
            } else {
                self.active_reordered();
            }
            // An undo always scrolls to the image brought back (the owner 2026-11-21, ADR 0026
            // §2 point 5, T242 point 9.2): the anchor taken by the relist is dropped, in both
            // branches. A new anchor taken by `relist_tabs` below loses to this scroll in its
            // frame. In the preview nothing changes.
            self.grid_anchor = None;
            if self
                .tabs
                .active()
                .is_some_and(|tab| tab.mode == ViewMode::Grid)
            {
                self.scroll_request = self.tabs.active().and_then(Tab::position);
            }
        }
        if !folders.is_empty() {
            self.follow_sort(ctx);
        }
        errors.extend(self.relist_tabs(ctx, &moved_to_folders(&restored)));
        let now = ClockTime::now();
        for error in &errors {
            self.tasks.add_error(error, now);
        }
        for (image, reason) in &failed {
            self.tasks
                .add_error(&undo_failed_text(&image_name(image), reason), now);
        }
        // A restored rating or a folder listing that failed is shown as `batch_done` does (T178).
        self.message = Some(match (failed.last(), errors.last()) {
            (Some((image, reason)), _) => undo_failed_text(&image_name(image), reason),
            (None, Some(error)) => error.clone(),
            (None, None) => undone_text(restored.len()),
        });
    }

    /// Delete or Image ▸ Delete to Recycle Bin (ADR 0018 §2.1, B4, T189): while a move or an undo
    /// runs or waits, or a delete is under way, the status line says so and nothing else is done;
    /// otherwise the key's targets (`key_targets`, B-Q2) are prepared by `follow_deletes`, and the
    /// active tab's folder is kept. No targets, nothing.
    fn delete_images(&mut self) {
        if self.moves.busy() {
            self.message = Some(delete_while_moving_text().to_owned());
            return;
        }
        if self.delete.busy() {
            self.message = Some(delete_running_text().to_owned());
            return;
        }
        let paths = self.key_targets();
        if paths.is_empty() {
            return;
        }
        let Some(folder) = self.tabs.active().and_then(|tab| tab.folder.clone()) else {
            return;
        };
        match resolve_paths(&paths) {
            Ok(images) => {
                self.delete = DeleteStep::Preparing { images, id: None };
                self.delete_folder = Some(folder);
                self.delete_rejected = false;
            }
            Err(error) => self.message = Some(error),
        }
    }

    /// Image ▸ Delete Rejected Images (ADR 0018 §2.1, B-Q9, B5, T190): the same guards as
    /// `delete_images`; then, when the active tab's folder read failed, that error in the status
    /// line; when its ratings are otherwise not ready, `ratings_not_ready_text`; with no rejected
    /// image in the tab's whole list (a filter hides none), `no_rejected_text`. Otherwise the
    /// rejected images are prepared as Delete's targets are, with `delete_rejected` set.
    fn delete_rejected(&mut self) {
        if self.moves.busy() {
            self.message = Some(delete_while_moving_text().to_owned());
            return;
        }
        if self.delete.busy() {
            self.message = Some(delete_running_text().to_owned());
            return;
        }
        let Some(tab) = self.tabs.active() else {
            return;
        };
        let Some(folder) = tab.folder.clone() else {
            return;
        };
        if let Some(error) = self.metadata.read_error() {
            self.message = Some(editor_text(&EditorText::MetadataError { error }));
            return;
        }
        if !self.ratings_ready() {
            self.message = Some(ratings_not_ready_text().to_owned());
            return;
        }
        let files = match resolve_paths(&tab.files) {
            Ok(files) => files,
            Err(error) => {
                self.message = Some(error);
                return;
            }
        };
        let metadata = &self.metadata;
        let images = rejected_images(&files, |path| {
            metadata.current_rating(path).stars == Some(-1)
        });
        if images.is_empty() {
            self.message = Some(no_rejected_text().to_owned());
            return;
        }
        self.delete = DeleteStep::Preparing { images, id: None };
        self.delete_folder = Some(folder);
        self.delete_rejected = true;
    }

    /// The delete is over or dropped: the step is `Idle`, and what belongs to it is cleared.
    fn delete_idle(&mut self) {
        self.delete = DeleteStep::Idle;
        self.delete_folder = None;
        self.delete_rejected = false;
        self.delete_partners.clear();
        self.delete_ids.clear();
        self.delete_focus_cancel = false;
        self.delete_button = None;
    }

    /// The delete's follow-up of a frame, right after `follow_moves` (T189 point 5): the answers
    /// go to the step (an answer of another request is ignored); a prepare not yet taken is sent;
    /// the running batch's images with no rating write waiting are sent, and a finished batch is
    /// acted on. A frame is asked for while the step is `Preparing` or `Running`.
    fn follow_deletes(&mut self, ctx: &egui::Context) {
        for answer in self.metadata.take_deletes() {
            match answer {
                DeleteAnswer::Prepared { id, images } => {
                    if matches!(
                        self.delete,
                        DeleteStep::Preparing { id: Some(sent), .. } if sent == id
                    ) {
                        self.delete_prepared(images);
                    }
                }
                DeleteAnswer::Deleted { id, image, result } => {
                    if let DeleteStep::Running(batch) = &mut self.delete
                        && self.delete_ids.contains(&id)
                    {
                        batch.answered(&image, result);
                    }
                }
            }
        }
        let suffixes = &self.settings.shell.companion_suffixes;
        let worker = &mut self.metadata_worker;
        let mut stopped = false;
        match &mut self.delete {
            DeleteStep::Preparing {
                images,
                id: id @ None,
            } => {
                // The images are cloned, so a full queue keeps them for the next frame.
                match MetadataSubmit::prepare_delete(
                    worker,
                    images.clone(),
                    image_extensions(),
                    suffixes.clone(),
                ) {
                    Ok(sent) => *id = Some(sent),
                    Err(pw_metadata::SubmitError::QueueFull) => {}
                    Err(pw_metadata::SubmitError::Stopped) => stopped = true,
                }
            }
            DeleteStep::Running(batch) => {
                let metadata = &self.metadata;
                let rejected_only = self.delete_rejected;
                let partners = self.delete_partners.as_slice();
                // Delete Rejected Images (T190 point 6): an image also waits while one of its
                // partners has a rating write pending, so its reject is known before it is sent.
                let waiting = |path: &Path| {
                    metadata.is_pending(path)
                        || (rejected_only
                            && partners_of(partners, path)
                                .iter()
                                .any(|partner| metadata.is_pending(partner)))
                };
                for image in batch.to_send(waiting) {
                    // Delete Rejected Images: an image whose reject, or a partner's, was cleared
                    // since (a key during the delete, or its write failed and was rolled back) is
                    // not sent (T190 points 4 and 6), as `send_moves` skips an X image no longer
                    // rejected. The batch's images and the partners are the tab's own resolved
                    // paths.
                    if rejected_only
                        && std::iter::once(&image)
                            .chain(partners_of(partners, &image))
                            .any(|path| metadata.current_rating(path).stars != Some(-1))
                    {
                        batch.skip(&image);
                        continue;
                    }
                    match MetadataSubmit::delete_image(
                        worker,
                        image.clone(),
                        image_extensions(),
                        suffixes.clone(),
                    ) {
                        Ok(id) => {
                            self.delete_ids.insert(id);
                            batch.sent(&image);
                        }
                        Err(pw_metadata::SubmitError::QueueFull) => break,
                        Err(pw_metadata::SubmitError::Stopped) => {
                            let error = DeleteError {
                                file: image.clone(),
                                reason: DeleteRefusal::Failed(worker_stopped_text().to_owned()),
                                left: Vec::new(),
                                deleted: Vec::new(),
                            };
                            batch.answered(&image, Err(error));
                        }
                    }
                }
            }
            DeleteStep::Idle | DeleteStep::Preparing { .. } | DeleteStep::Confirming(_) => {}
        }
        if stopped {
            self.delete_idle();
            self.message = Some(worker_stopped_text().to_owned());
        }
        if matches!(&self.delete, DeleteStep::Running(batch) if batch.done())
            && let DeleteStep::Running(batch) = std::mem::take(&mut self.delete)
        {
            self.deletes_done(ctx, batch);
        }
        if matches!(
            self.delete,
            DeleteStep::Preparing { .. } | DeleteStep::Running(_)
        ) {
            ctx.request_repaint_after(MOVE_POLL_INTERVAL);
        }
    }

    /// The worker's answer to the prepare (T189 point 5): dropped, with nothing shown, when the
    /// active tab is no longer of the delete's folder; otherwise its plan. With no image to delete
    /// the status line names the refused ones (or is left as it is when there are none) and the
    /// step is `Idle`; else the confirmation asks. For Delete Rejected Images (T190) the images no
    /// longer rejected are left out first (none left: `no_rejected_text`), and a rejected image
    /// whose RAW+JPEG partner is not rejected is refused (`refuse_unrejected_partners`, B-Q16).
    fn delete_prepared(&mut self, prepared: Vec<PreparedDelete>) {
        if !self.delete_tab_active() {
            self.delete_idle();
            return;
        }
        let prepared = if self.delete_rejected {
            match self.still_rejected(prepared) {
                Ok((prepared, _)) if prepared.is_empty() => {
                    self.message = Some(no_rejected_text().to_owned());
                    self.delete_idle();
                    return;
                }
                Ok((prepared, partners)) => {
                    self.delete_partners = partners;
                    prepared
                }
                Err(error) => {
                    self.message = Some(error);
                    self.delete_idle();
                    return;
                }
            }
        } else {
            prepared
        };
        let plan = plan_delete(prepared, is_image_file);
        if plan.images.is_empty() {
            if let Some((image, reason)) = plan.refused.first() {
                self.message = Some(delete_refused_text(
                    plan.refused.len(),
                    &image_name(image),
                    reason,
                ));
            }
            self.delete_idle();
            return;
        }
        self.delete = DeleteStep::Confirming(plan);
        self.delete_focus_cancel = true;
    }

    /// The prepare's answers of Delete Rejected Images that are still rejected, with the partners
    /// not rejected refused (T190 points 3 and 4). Whether a file is rejected is read through the
    /// active tab's own path for it: the tab file whose resolved path is `same_path` with it; a
    /// file not in the tab's list counts as not rejected. Also each kept image's partners
    /// (`image_partners` of its `Ok` group) as the tab's own resolved paths, for the check while
    /// the batch runs (point 6); a partner not found in the list (not possible after point 3) is
    /// kept as the worker gave it and so counts as not rejected then. An error when the paths
    /// cannot be resolved.
    fn still_rejected(
        &self,
        prepared: Vec<PreparedDelete>,
    ) -> Result<(Vec<PreparedDelete>, ImagePartners), String> {
        let files = match self.tabs.active() {
            Some(tab) => resolve_paths(&tab.files)?,
            None => Vec::new(),
        };
        let tab_path = |path: &Path| files.iter().find(|file| same_path(file, path));
        let metadata = &self.metadata;
        let is_rejected = |path: &Path| {
            tab_path(path).is_some_and(|file| metadata.current_rating(file).stars == Some(-1))
        };
        let prepared: Vec<PreparedDelete> = prepared
            .into_iter()
            .filter(|answer| is_rejected(&answer.image))
            .collect();
        let prepared = refuse_unrejected_partners(prepared, is_rejected, is_image_file);
        let partners = prepared
            .iter()
            .filter_map(|answer| {
                let group = answer.result.as_ref().ok()?;
                let partners = image_partners(&group.files, is_image_file)
                    .into_iter()
                    .map(|partner| tab_path(&partner).cloned().unwrap_or(partner))
                    .collect();
                Some((answer.image.clone(), partners))
            })
            .collect();
        Ok((prepared, partners))
    }

    /// The confirmation of a delete (T189 point 4), an `egui::Modal` drawn while the step is
    /// `Confirming` and no other dialog is open, and dropped when the active tab is no longer of
    /// the delete's folder: the question, the refused images and the image files that go along,
    /// and Delete and Cancel. Cancel has the focus on the first frame (B-Q4); Escape or a click
    /// outside is Cancel. Cancel → `Idle`; Delete → `Running`.
    fn draw_delete_window(&mut self, ctx: &egui::Context) {
        if !matches!(self.delete, DeleteStep::Confirming(_)) || self.other_dialog_open() {
            return;
        }
        // Another tab was made active while the confirmation waited for another dialog (which is
        // not modal) or on the frame it began: the confirmation is dropped, nothing shown.
        if !self.delete_tab_active() {
            self.delete_idle();
            return;
        }
        let DeleteStep::Confirming(plan) = &self.delete else {
            return;
        };
        let question = delete_confirm_text(plan.images.len(), plan.files, plan.bytes);
        let refused = plan.refused.first().map(|(image, reason)| {
            delete_refused_text(plan.refused.len(), &image_name(image), reason)
        });
        let partners = (!plan.partners.is_empty()).then(|| {
            let names: Vec<String> = plan.partners.iter().map(|file| image_name(file)).collect();
            delete_partners_text(&names)
        });
        let focus_cancel = std::mem::take(&mut self.delete_focus_cancel);
        let modal = egui::Modal::new(egui::Id::unique("delete-confirmation")).show(ctx, |ui| {
            ui.set_max_width(DELETE_WINDOW_WIDTH);
            ui.heading(delete_title_text());
            ui.add_space(PANEL_ROW_SPACING);
            ui.label(question);
            if let Some(text) = refused {
                ui.add_space(PANEL_ROW_SPACING);
                ui.label(text);
            }
            if let Some(text) = partners {
                ui.add_space(PANEL_ROW_SPACING);
                ui.label(text);
            }
            ui.add_space(PANEL_ROW_SPACING);
            ui.horizontal(|ui| {
                let delete = ui
                    .push_id("delete-confirmation-delete", |ui| {
                        ui.button(delete_button_text())
                    })
                    .inner;
                let cancel = ui
                    .push_id("delete-confirmation-cancel", |ui| {
                        ui.button(delete_cancel_text())
                    })
                    .inner;
                if focus_cancel {
                    cancel.request_focus();
                }
                (delete.id, delete.clicked(), cancel.clicked())
            })
            .inner
        });
        let (delete_id, delete, cancel) = modal.inner;
        self.delete_button = Some(delete_id);
        if delete {
            if let DeleteStep::Confirming(plan) = std::mem::take(&mut self.delete) {
                self.delete = DeleteStep::Running(DeleteBatch::new(plan.images));
            }
            ctx.request_repaint();
        } else if cancel || modal.should_close() {
            self.delete_idle();
        }
    }

    /// Image ▸ Open in {name} or Ctrl+E (ADR 0019 §2.2–§2.6, T202): the targets (`key_targets`:
    /// in the contact sheet the marked images, otherwise the current one, E-Q4), resolved, are
    /// opened in the program at `index` of the list. Only the image files as the tab lists them
    /// (E-Q5). A path error goes to the status line; a missing program to the status line and
    /// the Tasks error list (E-Q9); a command line too long to the status line (E-Q8); none of
    /// these touches a request that waits. Above 100 images the question asks first; otherwise
    /// the request waits for the images' writes (`follow_open`). A new request replaces one
    /// that waits (§2.4).
    fn open_in(&mut self, index: usize) {
        let Some(program) = self.settings.external_programs.get(index).cloned() else {
            return;
        };
        let paths = self.key_targets();
        if paths.is_empty() {
            return;
        }
        let images = match resolve_paths(&paths) {
            Ok(images) => images,
            Err(error) => {
                self.message = Some(error);
                return;
            }
        };
        if images.is_empty() {
            return;
        }
        if !program.path.is_file() {
            let text = program_missing_text(&program.name, &program.path.display().to_string());
            self.tasks.add_error(&text, ClockTime::now());
            self.message = Some(text);
            return;
        }
        let name = program.name.clone();
        match plan_open(program, images) {
            OpenPlan::TooLong { count, fit } => {
                self.message = Some(too_long_text(count, &name, fit));
            }
            OpenPlan::Ask(request) => {
                self.open = OpenStep::Confirming(request);
                self.open_focus_cancel = true;
                self.open_button = None;
            }
            OpenPlan::Start(request) => self.open_wait(request),
        }
    }

    /// The request waits for its images' writes (ADR 0019 §2.4): the step becomes `Waiting`,
    /// replacing a request that waited (it is dropped, not started), and while a write is waiting
    /// the status line says so, once here.
    fn open_wait(&mut self, request: OpenRequest) {
        let ready = request.ready(|path| self.metadata.write_pending(path));
        self.open = OpenStep::Waiting(request);
        self.open_focus_cancel = false;
        self.open_button = None;
        if !ready {
            self.message = Some(waiting_for_writes_text().to_owned());
        }
    }

    /// The question is cancelled (Cancel, Escape, Enter without the Open button's focus, or a
    /// click outside): the step is `Idle` and nothing is started. A "Waiting for writes …" left
    /// by a request the question replaced is cleared.
    fn open_cancelled(&mut self) {
        self.open = OpenStep::Idle;
        self.open_focus_cancel = false;
        self.open_button = None;
        if self.message.as_deref() == Some(waiting_for_writes_text()) {
            self.message = None;
        }
    }

    /// The question of "Open in {name}" above 100 images (ADR 0019 §2.5, E-Q8, T202), an
    /// `egui::Modal` drawn while `open_window_shown`: the question, and Open and Cancel. Cancel is
    /// the default and has the focus on the first frame; Escape or a click outside is Cancel.
    /// Cancel → `Idle`; Open → the request waits for its writes.
    fn draw_open_window(&mut self, ctx: &egui::Context) {
        if !self.open_window_shown() {
            return;
        }
        let OpenStep::Confirming(request) = &self.open else {
            return;
        };
        let question = open_many_question_text(request.images.len(), &request.program.name);
        let focus_cancel = std::mem::take(&mut self.open_focus_cancel);
        let modal = egui::Modal::new(egui::Id::unique("open-in-question")).show(ctx, |ui| {
            ui.set_max_width(OPEN_WINDOW_WIDTH);
            ui.label(question);
            ui.add_space(PANEL_ROW_SPACING);
            ui.horizontal(|ui| {
                let open = ui
                    .push_id("open-in-question-open", |ui| ui.button(open_button_text()))
                    .inner;
                let cancel = ui
                    .push_id("open-in-question-cancel", |ui| {
                        ui.button(open_cancel_text())
                    })
                    .inner;
                if focus_cancel {
                    cancel.request_focus();
                }
                (open.id, open.clicked(), cancel.clicked())
            })
            .inner
        });
        let (open_id, open, cancel) = modal.inner;
        self.open_button = Some(open_id);
        if open {
            if let OpenStep::Confirming(request) = std::mem::take(&mut self.open) {
                self.open_wait(request);
            }
            ctx.request_repaint();
        } else if cancel || modal.should_close() {
            self.open_cancelled();
        }
    }

    /// The follow-up of "Open in {name}" every frame, right after `follow_deletes` (ADR 0019
    /// §2.4, §2.6, T202). While a request waits it also waits while a move or an undo runs or
    /// waits, or a delete is under way (they wait for the same writes and run earlier in the
    /// frame); then, once no image has a write waiting, the request is taken. The images no
    /// longer files (moved or deleted meanwhile) are left out, each named in the Tasks error
    /// list; with none left nothing is started. Otherwise the program is started (E-Q7) and the
    /// status line says so, or the error goes to the status line and the Tasks error list (E-Q9).
    /// A write that failed meanwhile was reported by its own path; the program is started anyway.
    fn follow_open(&mut self, ctx: &egui::Context) {
        let OpenStep::Waiting(request) = &self.open else {
            return;
        };
        let held = self.moves.busy() || self.delete.busy();
        if held || !request.ready(|path| self.metadata.write_pending(path)) {
            ctx.request_repaint_after(MOVE_POLL_INTERVAL);
            return;
        }
        let OpenStep::Waiting(request) = std::mem::take(&mut self.open) else {
            return;
        };
        let OpenRequest { program, images } = request;
        let now = ClockTime::now();
        let mut left = Vec::with_capacity(images.len());
        for image in images {
            if image.is_file() {
                left.push(image);
            } else {
                self.tasks
                    .add_error(&image_gone_text(&image_name(&image)), now);
            }
        }
        if left.is_empty() {
            self.message = Some(nothing_to_open_text().to_owned());
            return;
        }
        let request = OpenRequest {
            program,
            images: left,
        };
        let name = &request.program.name;
        match start_program(&request) {
            Ok(()) => self.message = Some(opened_text(request.images.len(), name)),
            Err(error) => {
                let text = match error {
                    StartError::Missing => {
                        program_missing_text(name, &request.program.path.display().to_string())
                    }
                    StartError::Failed(reason) => start_failed_text(name, &reason),
                };
                self.tasks.add_error(&text, now);
                self.message = Some(text);
            }
        }
    }

    /// Image ▸ Show in File Explorer (ADR 0019 §2.8, E3, T203): the current image only, also with
    /// several images marked. A file that is no longer a file is said in the status line only and
    /// starts nothing, since Explorer would open another folder without a word; a failed start
    /// goes to the status line and the Tasks error list. Nothing is said when it starts.
    fn show_current_in_explorer(&mut self) {
        let Some(file) = self
            .tabs
            .active()
            .and_then(|tab| tab.files.get(tab.current))
        else {
            return;
        };
        let path = match resolve_paths(std::slice::from_ref(file)) {
            Ok(paths) => paths.into_iter().next(),
            Err(error) => {
                self.message = Some(error);
                return;
            }
        };
        let Some(path) = path else {
            return;
        };
        if !path.is_file() {
            self.message = Some(explorer_missing_text(&image_name(&path)));
            return;
        }
        if let Err(reason) = show_in_explorer(&path) {
            let text = explorer_failed_text(&reason);
            self.tasks.add_error(&text, ClockTime::now());
            self.message = Some(text);
        }
    }

    /// A batch of deletes has all its answers (T189 point 6): every tab of the deleted files'
    /// folders drops them, the active one's view cores and thumbnails follow, the metadata cache
    /// and the undo list of moves forget them, the failures and listing errors go to the Tasks
    /// list, and the status line is set last. The step is `Idle` after.
    fn deletes_done(&mut self, ctx: &egui::Context, batch: DeleteBatch) {
        let result = batch.finish();
        let deleted = &result.files;
        let folders = image_folders(deleted.iter().map(PathBuf::as_path));
        let (active, mut errors) = self.replace_tab_files(&folders, |tab| {
            let resolved = resolve_paths(&tab.files)?;
            let files = tab
                .files
                .iter()
                .zip(resolved)
                .filter(|(_, path)| !deleted.iter().any(|file| same_path(file, path)))
                .map(|(file, _)| file.clone())
                .collect();
            Ok((files, None))
        });
        if active {
            self.active_reordered();
        }
        self.metadata.forget_deleted(&result.files);
        self.moves.undo.forget_deleted(&result.files);
        let failure_text = |error: &DeleteError| {
            let name = image_name(&error.file);
            let reason = error.reason.to_string();
            if error.left.is_empty() {
                delete_failed_text(&name, &reason)
            } else {
                let left: Vec<String> = error
                    .left
                    .iter()
                    .map(|file| file.display().to_string())
                    .collect();
                delete_partly_failed_text(&name, &reason, &left)
            }
        };
        let now = ClockTime::now();
        for error in &errors {
            self.tasks.add_error(error, now);
        }
        for (_, error) in &result.failed {
            self.tasks.add_error(&failure_text(error), now);
        }
        // A folder listing that failed is shown as `batch_done` does when nothing failed.
        self.message = Some(match result.failed.as_slice() {
            [] => errors
                .pop()
                .unwrap_or_else(|| deleted_text(result.images.len())),
            [(_, error)] => failure_text(error),
            [.., (_, error)] => deletes_failed_text(
                result.failed.len(),
                &image_name(&error.file),
                &error.reason.to_string(),
            ),
        });
        self.delete_idle();
        ctx.request_repaint();
    }

    /// After accepted ratings: the images that stopped matching leave at once (§2.2, amendment
    /// 1). Returns `Some` with the position the contact sheet scrolls to when the visible list
    /// changed. Used by `rate` and when the metadata window closes after a rating (T124).
    fn after_rating(&mut self) -> Option<Option<usize>> {
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
            Command::Stars(_)
            | Command::Reject
            | Command::Label(_)
            | Command::ClearLabel
            | Command::RotateLeft
            | Command::RotateRight => tab.is_some_and(|tab| tab.position().is_some()),
            // Greyed while the setting is off (U9-Q6, T174).
            Command::MovePicked => {
                self.settings.shell.move_to_subfolders
                    && tab.is_some_and(|tab| tab.position().is_some())
            }
            // Greyed while the undo list is empty or a batch or an undo runs (U9-Q8).
            Command::UndoMove => !self.moves.undo.is_empty() && !self.moves.busy(),
            // Greyed without a current image, and in a tab without a folder, whose list is not
            // followed after a delete (T189). A running move or delete is said when chosen.
            Command::DeleteToRecycleBin => {
                tab.is_some_and(|tab| tab.position().is_some() && tab.folder.is_some())
            }
            // Greyed while the active tab has no folder (T190); a running move or delete, ratings
            // not read yet and no rejected image are said when chosen.
            Command::DeleteRejected => tab.is_some_and(|tab| tab.folder.is_some()),
            // Enabled with a current image and a program at that index (ADR 0019 §2.2, T202); a
            // missing `.exe` is said when chosen by its key and greyed in the menu.
            Command::OpenIn(index) => {
                tab.is_some_and(|tab| tab.position().is_some())
                    && usize::from(index) < self.settings.external_programs.len()
            }
            // Greyed while the active tab shows no current image (ADR 0019 §2.8, T203).
            Command::ShowInExplorer => tab.is_some_and(|tab| tab.position().is_some()),
            Command::ToggleView => tab.is_some_and(|tab| !tab.visible.is_empty()),
            // Compare opens with two images in the stepped list, and closes an open compare; the
            // other four act only in the compare (T153, T154).
            Command::Compare => self.compare.is_some() || self.compare_open_indexes().is_some(),
            Command::HoldImage | Command::SwapImages | Command::AboveBelow | Command::LockZoom => {
                self.compare.is_some()
            }
            // Greyed without a folder and while files are moved or deleted (ADR 0026 F1, T242):
            // a reload then could list a file on its way.
            Command::ReloadFolder => reload_enabled(tab, self.moves.busy() || self.delete.busy()),
            _ => true,
        }
    }

    /// Whether a check or radio item is on; `None` for a plain item.
    fn command_checked(&self, command: Command) -> Option<bool> {
        let shell = &self.settings.shell;
        match command {
            Command::ToggleAnchor => Some(self.anchor_at_pointer),
            Command::ClipHighlights => Some(shell.clip_highlights),
            Command::ClipShadows => Some(shell.clip_shadows),
            Command::FocusMarking => Some(shell.focus_marking),
            Command::MoveToSubfolders => Some(shell.move_to_subfolders),
            Command::AboveBelow => Some(shell.compare_above_below),
            Command::LockZoom => Some(shell.compare_locked),
            Command::ToggleInfo => {
                Some(!shell.columns_hidden && shell.right_column == ColumnState::Open)
            }
            Command::ToggleColumns => Some(!shell.columns_hidden),
            Command::LeftColumn => {
                Some(!shell.columns_hidden && shell.left_column == ColumnState::Open)
            }
            Command::Fullscreen => Some(shell.fullscreen),
            Command::ThumbnailButtons => Some(shell.thumbnail_buttons),
            Command::Theme(theme) => Some(shell.theme == theme),
            Command::Sort(key) => Some(shell.sort.key == key),
            Command::SortDescending => Some(shell.sort.descending),
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
            Command::ReloadFolder => self.reload_active_tab(ctx),
            Command::Import => self.open_import(),
            Command::CloseTab => self.tab_action(TabAction::Close),
            Command::NextTab => self.tab_action(TabAction::Next),
            Command::PreviousTab => self.tab_action(TabAction::Previous),
            Command::Settings => self.open_settings_window(SettingsPage::General),
            Command::About => self.about_window = true,
            Command::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Command::SelectAll => self.select_all(),
            Command::ClearSelection => self.clear_selection(),
            Command::Metadata => self.act(Action::OpenMetadata, None),
            Command::Stars(stars) => self.rate_from_menu(RatingKey::Stars(stars)),
            Command::Reject => self.rate_from_menu(RatingKey::Reject),
            Command::MovePicked => self.move_picked(),
            Command::MoveToSubfolders => self.toggle_move_to_subfolders(),
            Command::UndoMove => self.undo_move(),
            Command::Label(label) => self.rate_from_menu(RatingKey::Label(label)),
            Command::ClearLabel => self.rate_from_menu(RatingKey::ClearLabel),
            Command::RotateLeft => self.rotate(ctx, false),
            Command::RotateRight => self.rotate(ctx, true),
            Command::ToggleMode => self.act(Action::ToggleMode, None),
            Command::DeleteToRecycleBin => self.delete_images(),
            Command::DeleteRejected => self.delete_rejected(),
            Command::OpenIn(index) => self.open_in(usize::from(index)),
            // The Settings window opens on its page "External Programs", where a program is added
            // (ADR 0019 §2.2, E-Q3; its pages, U10b-Q6, T278).
            Command::OpenInAnother => self.open_settings_window(SettingsPage::ExternalPrograms),
            Command::ShowInExplorer => self.show_current_in_explorer(),
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
            Command::ClipHighlights => self.toggle_clip_highlights(),
            Command::ClipShadows => self.toggle_clip_shadows(),
            Command::FocusMarking => self.toggle_focus_marking(),
            Command::Compare => self.toggle_compare(),
            Command::HoldImage => self.hold_current_image(),
            Command::SwapImages => self.swap_images(),
            Command::AboveBelow => self.toggle_compare_layout(),
            Command::LockZoom => self.toggle_compare_lock(),
            Command::ToggleInfo => self.act(Action::ToggleInfo, None),
            Command::ToggleColumns => self.shell_action(ctx, ShellAction::ToggleColumns),
            Command::LeftColumn => self.toggle_column(true),
            Command::Fullscreen => self.shell_action(ctx, ShellAction::ToggleFullscreen),
            Command::ThumbnailButtons => self.toggle_thumbnail_buttons(),
            Command::Theme(theme) => self.set_theme(ctx, theme),
            Command::Sort(key) => self.set_sort(
                ctx,
                SortOrder {
                    key,
                    ..self.settings.shell.sort
                },
            ),
            Command::SortDescending => self.set_sort(
                ctx,
                SortOrder {
                    descending: !self.settings.shell.sort.descending,
                    ..self.settings.shell.sort
                },
            ),
        }
    }

    /// A rating item of the Bild menu: `rate`, with the contact sheet scrolled as after its key.
    fn rate_from_menu(&mut self, action: RatingKey) {
        let (scroll, moves) = self.rate(action);
        if let Some(Some(position)) = scroll {
            self.scroll_request = Some(position);
        }
        self.queue_moves(MoveKind::Reject, moves);
    }

    /// The menu bar (ADR 0017 §2.1, U1-Q1): the table `MENUS`, each item with its key.
    /// A choice in Visa ▸ Filter is returned in `filter_choice`. File ▸ "Add to Favourites" (after
    /// "Close Tab", U4-Q3, T135) returns the active tab's folder in `favourite`.
    fn draw_menu_bar(
        &self,
        ui: &mut egui::Ui,
        commands: &mut Vec<Command>,
        filter_choice: &mut Option<Filter>,
        favourite: &mut Option<PathBuf>,
    ) {
        egui::MenuBar::new().ui(ui, |ui| {
            for menu in MENUS {
                ui.menu_button(menu_text(menu.kind), |ui| {
                    for entry in menu.entries {
                        match entry {
                            MenuEntry::Item(_)
                            | MenuEntry::Separator
                            | MenuEntry::Submenu(..)
                            | MenuEntry::ExternalPrograms => {
                                self.menu_entry(ui, *entry, commands);
                                if matches!(
                                    entry,
                                    MenuEntry::Item(item) if item.command == Command::CloseTab
                                ) {
                                    let folder =
                                        self.tabs.active().and_then(|tab| tab.folder.clone());
                                    let button = egui::Button::new(library_text(
                                        LibraryText::AddToFavourites,
                                    ));
                                    if ui.add_enabled(folder.is_some(), button).clicked() {
                                        *favourite = folder;
                                    }
                                }
                            }
                            MenuEntry::Sort => {
                                ui.menu_button(shell_text(ShellText::SortMenu), |ui| {
                                    for key in SORT_KEYS {
                                        let item = MenuItem {
                                            command: Command::Sort(key),
                                            key: None,
                                        };
                                        self.menu_item(ui, item, commands);
                                    }
                                    ui.separator();
                                    let item = MenuItem {
                                        command: Command::SortDescending,
                                        key: None,
                                    };
                                    self.menu_item(ui, item, commands);
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

    /// One entry of the menu bar or of the right-click menu on images (ADR 0019 §2.9, T221): an
    /// item, a separator, a submenu or the external programs, drawn the same in both menus.
    fn menu_entry(&self, ui: &mut egui::Ui, entry: MenuEntry, commands: &mut Vec<Command>) {
        match entry {
            MenuEntry::Item(item) => self.menu_item(ui, item, commands),
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Submenu(kind, items) => {
                ui.menu_button(submenu_text(kind), |ui| {
                    for item in items {
                        self.menu_item(ui, *item, commands);
                    }
                });
            }
            MenuEntry::ExternalPrograms => self.external_programs_menu(ui, commands),
            // The menu bar draws the filter and the sort itself; the right-click menu has neither.
            MenuEntry::Filter | MenuEntry::Sort => {}
        }
    }

    /// The right-click menu on an image in the contact sheet or the preview (ADR 0019 §2.9,
    /// E-Q17, E-Q18): every entry of `IMAGE_CONTEXT_MENU`, with the menu bar's texts, keys and
    /// greying. Its commands go into the frame's `commands`, as the menu bar's do; the popup
    /// closes by itself after a click.
    fn image_context_menu(&self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        for entry in IMAGE_CONTEXT_MENU {
            self.menu_entry(ui, *entry, commands);
        }
    }

    /// Image ▸ the external programs (ADR 0019 §2.2, E-Q3, E-Q9, T202): with an empty list "Open
    /// in Another Program…" with Ctrl+E on its right; otherwise "Open in {name}" per program in
    /// the list's order, the first with Ctrl+E, greyed while its command is not enabled or its
    /// path is not a file (checked only while the menu is open).
    fn external_programs_menu(&self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        let programs = &self.settings.external_programs;
        if programs.is_empty() {
            let button =
                egui::Button::new(open_in_another_text()).shortcut_text(key_text(OPEN_IN_KEY));
            if ui
                .add_enabled(self.command_enabled(Command::OpenInAnother), button)
                .clicked()
            {
                commands.push(Command::OpenInAnother);
            }
            return;
        }
        for (index, program) in programs.iter().enumerate() {
            // At most `MAX_EXTERNAL_PROGRAMS` (5) programs, so this never fails.
            let Ok(number) = u8::try_from(index) else {
                continue;
            };
            let command = Command::OpenIn(number);
            let mut button = egui::Button::new(open_in_text(&program.name));
            if index == 0 {
                button = button.shortcut_text(key_text(OPEN_IN_KEY));
            }
            let enabled = self.command_enabled(command) && program.path.is_file();
            if ui.add_enabled(enabled, button).clicked() {
                commands.push(command);
            }
        }
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
            // The sort order of every tab (ADR 0017 U14-Q7, T138): the key and the direction.
            let sort = self.settings.shell.sort;
            egui::ComboBox::from_id_salt("sort-key")
                .selected_text(sort_button_text(sort.key))
                .show_ui(ui, |ui| {
                    for key in SORT_KEYS {
                        if ui
                            .selectable_label(sort.key == key, sort_key_text(key))
                            .clicked()
                        {
                            commands.push(Command::Sort(key));
                        }
                    }
                });
            if ui
                .button(sort_direction_text(sort.descending))
                .on_hover_text(sort_direction_hover(sort.descending))
                .clicked()
            {
                commands.push(Command::SortDescending);
            }
            ui.separator();
            let slider = ui.add(
                egui::Slider::new(
                    &mut self.thumb_size,
                    MIN_THUMBNAIL_SIZE..=MAX_THUMBNAIL_SIZE,
                )
                .text(shell_text(ShellText::ThumbnailSize))
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

    /// Does what was clicked in the library, a folder tab's right-click menu or File ▸ "Add to
    /// Favourites" (U4, T135). A changed list is saved at once; a section's state at exit, as the
    /// other sections'.
    fn library_action(&mut self, action: LibraryAction) {
        match action {
            LibraryAction::Open(folder) => {
                // As Ctrl+O (U4-Q6): not while a dialog holds the program.
                if !self.dialog_open() {
                    self.open(&folder, true);
                    // A folder that turned out to be gone is greyed now.
                    self.library.favourites.check_folders();
                    self.library.recent.check_folders();
                }
            }
            LibraryAction::Toggle(path) => {
                self.library.toggle(&path);
            }
            LibraryAction::AddFavourite(folder) => {
                if self.library.favourites.add(folder) {
                    self.save_library();
                }
            }
            LibraryAction::RemoveFavourite(folder) => {
                if self.library.favourites.remove(&folder) {
                    self.save_library();
                }
            }
            LibraryAction::ClearRecent => {
                self.library.recent.clear();
                self.save_library();
            }
            LibraryAction::Refresh => self.library.refresh(&read_drives()),
            LibraryAction::ToggleSection(section) => {
                let shell = &mut self.settings.shell;
                let state = match section {
                    LibrarySection::Folders => &mut shell.library_folders,
                    LibrarySection::Favourites => &mut shell.library_favourites,
                    LibrarySection::Recent => &mut shell.library_recent,
                };
                *state = state.toggled();
            }
        }
    }

    /// Does what was clicked in the Tasks area (U5, T140).
    fn task_action(&mut self, action: TaskAction) {
        match action {
            TaskAction::ToggleSection => {
                let state = &mut self.settings.shell.library_tasks;
                *state = state.toggled();
            }
            // The import dialog again (U5-Q2): the running or finished import's, or a new one
            // when it was closed.
            TaskAction::ShowImport => self.open_import(),
            // As the dialog's Cancel, without a question (U5-Q3): the files copied so far stay.
            TaskAction::Stop => {
                if self
                    .import_dialog
                    .as_ref()
                    .is_some_and(ImportDialog::cancel)
                {
                    self.import_worker.cancel();
                }
            }
            TaskAction::DismissImport => self.tasks.dismiss_import(),
            TaskAction::ClearErrors => self.tasks.clear_errors(),
        }
    }

    /// The side columns (ADR 0017 §2.1, U1-Q3, T109): each resizable between the minimum and 40 %
    /// of the window, collapsible to a strip with its open button, and both hidden with the hide
    /// key. The right one is the inspector (U2, T110); the left one is the library (U4, T135)
    /// and, under it, the Tasks area (U5, T140).
    fn draw_columns(&mut self, ui: &mut egui::Ui) {
        let shell = self.settings.shell.clone();
        if shell.columns_hidden {
            return;
        }
        let window_width = ui.ctx().content_rect().width();
        let (min, max) = column_width_range(window_width);
        let pending = pending_writes(
            self.metadata.pending_rating_writes(),
            self.metadata.pending_rotation_writes(),
            self.metadata.batch_progress(),
            self.metadata.editor_saving(),
        );
        match shell.left_column {
            ColumnState::Open => {
                // The library (U4, T135): what was clicked is done after the column is drawn.
                let mut library_actions = Vec::new();
                let mut task_actions = Vec::new();
                let active_folder = self
                    .tabs
                    .active()
                    .and_then(|tab| tab.folder.as_deref())
                    .map(folder_key);
                let library = &self.library;
                let tasks = &self.tasks;
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
                        draw_library(
                            ui,
                            library,
                            &shell,
                            active_folder.as_deref(),
                            &mut library_actions,
                        );
                        ui.add_space(PANEL_ROW_SPACING * 2.0);
                        draw_tasks(ui, tasks, &shell, &pending, &mut task_actions);
                        collapse
                    });
                // F5 while the library has the pointer reads the drives again (U4-Q1).
                let ctx = ui.ctx().clone();
                let pointer_in_column = ctx
                    .pointer_hover_pos()
                    .is_some_and(|pos| shown.response.rect.contains(pos));
                if pointer_in_column
                    && !ctx.egui_wants_keyboard_input()
                    && ctx.input(|input| input.key_pressed(egui::Key::F5))
                {
                    library_actions.push(LibraryAction::Refresh);
                    // The central area is drawn after this column: its key loop leaves this F5
                    // to the library (T242).
                    self.library_took_f5 = true;
                }
                self.settings.shell.left_column_width =
                    shown.response.rect.width().round().max(0.0) as u32;
                if shown.inner {
                    self.settings.shell.left_column = ColumnState::Collapsed;
                }
                for action in library_actions {
                    self.library_action(action);
                }
                for action in task_actions {
                    self.task_action(action);
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
                // The inspector (ADR 0017 §2.1, T110): the current image's file name, then Info,
                // Metadata and Histogram (T142), the whole column in one scroll area.
                let cwd = std::env::current_dir().ok();
                let image = inspector_subject(self.tabs.active(), cwd.as_deref());
                // The inspector's own statistics, the preview's included (U7c-Q1, T144).
                let histogram = image.as_deref().map_or(HistogramPanel::Reading, |image| {
                    self.histograms.shown(image, self.display_mode)
                });
                let (exif, metadata) = (&self.exif, &self.metadata);
                let mut toggled = [false; 3];
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
                        egui::ScrollArea::vertical()
                            .id_salt("inspector-scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                toggled = draw_inspector(
                                    ui,
                                    image.as_deref(),
                                    &shell,
                                    exif,
                                    metadata,
                                    &histogram,
                                );
                            });
                        collapse
                    });
                self.settings.shell.right_column_width =
                    shown.response.rect.width().round().max(0.0) as u32;
                if shown.inner {
                    self.settings.shell.right_column = ColumnState::Collapsed;
                }
                let sections = &mut self.settings.shell;
                if toggled[0] {
                    sections.inspector_info = sections.inspector_info.toggled();
                }
                if toggled[1] {
                    sections.inspector_metadata = sections.inspector_metadata.toggled();
                }
                if toggled[2] {
                    sections.inspector_histogram = sections.inspector_histogram.toggled();
                }
            }
            ColumnState::Collapsed => {
                if collapsed_column(ui, "right-column-collapsed", false) {
                    self.settings.shell.right_column = ColumnState::Open;
                }
            }
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        let ctx = ui.ctx().clone();
        let failures = self.metadata.poll(&mut self.metadata_worker);
        // The embedded orientations read for turns that waited for them (T237).
        self.follow_embedded_reads(&ctx);
        // The XMP orientations read with the ratings, and the turns, go to the view and the
        // thumbnails (T232, T237).
        self.follow_orientations();
        // A kept histogram follows RawStride's own metadata writes of its file (U7c-Q7, T145).
        // RawStride's own write of a file another program added renews its stamp, so the write
        // does not make its thumbnail anew; a cell read while the safe write had renamed the
        // file away is made anew (T255). A failed write left the file as it was.
        let cwd = std::env::current_dir().ok();
        for written in self.metadata.take_written() {
            self.histograms.written(
                &written.path,
                written.succeeded,
                self.histogram_reader.reads(),
            );
            if !written.succeeded {
                continue;
            }
            let Some(kept) = self.added_files.kept_as(&written.path, cwd.as_deref()) else {
                continue;
            };
            self.added_files.restamp(&kept, file_stamp(&written.path));
            let index = self
                .tabs
                .active()
                .and_then(|tab| tab.files.iter().position(|file| *file == kept));
            if let Some(index) = index
                && matches!(self.thumb_grid.state(index), CellState::Failed(_))
            {
                self.thumb_worker.forget_path(&kept);
                self.thumb_grid.forget(index);
                self.added_files.made_anew(&kept);
                ctx.request_repaint();
            }
        }
        // Every failure also goes to the Tasks area's error list (U5-Q5).
        if !failures.is_empty() {
            let now = ClockTime::now();
            for failure in &failures {
                self.tasks.add_error(failure, now);
            }
        }
        if let Some(message) = failures.last() {
            // One line for every failure since the last rating key (ADR 0014 Q6).
            self.message = Some(
                self.metadata
                    .failure_text()
                    .unwrap_or_else(|| message.clone()),
            );
        }
        // The moves to subfolders and their undo follow the answers and are sent (T174).
        self.follow_moves(&ctx);
        // The delete to the Recycle Bin follows its answers and is sent (ADR 0018 B4, T189).
        self.follow_deletes(&ctx);
        // "Open in {name}" starts its program once the images' writes are done (ADR 0019 §2.4,
        // T202).
        self.follow_open(&ctx);
        // A rolled-back write does not refilter (ADR 0014 §2.2; the next move drops the image); the
        // ratings becoming ready does. New files are shown when their ratings are read (T251).
        self.follow_unread();
        self.refilter_when_ratings_change();
        // The sort order follows its keys as they are read (U14-Q4, T138).
        self.follow_sort(&ctx);
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
        // Not while the metadata window holds the program (D16): its image must not change. A drop
        // on the preview closes it first, then opens as in the contact sheet (T111).
        if let Some(path) = dropped
            && self.metadata_window.is_none()
            && !matches!(self.delete, DeleteStep::Confirming(_))
            && !matches!(self.open, OpenStep::Confirming(_))
        {
            if self.in_preview() {
                self.switch_mode(ViewMode::Grid);
            }
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
        // A folder tab's right-click menu: "Add to Favourites" (U4-Q3, T135).
        let mut favourite = None;
        // The preview shows no folder tabs (U3-Q1, T111).
        let mode_before = self.tabs.active().map(|tab| tab.mode);
        if mode_before != Some(ViewMode::Loupe) {
            let tabs = self.tabs.tabs();
            egui::ScrollArea::horizontal()
                .id_salt("folder-tabs")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (index, title) in titles.iter().enumerate() {
                            let tab = ui.selectable_label(index == active, title);
                            if tab.clicked() {
                                selected = Some(index);
                            }
                            if let Some(folder) = tabs.get(index).and_then(|t| t.folder.as_ref()) {
                                tab.context_menu(|ui| {
                                    if ui
                                        .button(library_text(LibraryText::AddToFavourites))
                                        .clicked()
                                    {
                                        favourite = Some(folder.clone());
                                        ui.close();
                                    }
                                });
                            }
                        }
                    });
                });
        }
        if let Some(folder) = favourite {
            self.library_action(LibraryAction::AddFavourite(folder));
        }
        // The command bar or a menu command may have asked for a scroll (T109).
        let mut scroll_to = self.scroll_request.take();
        if let Some(index) = selected
            && index != self.tabs.active_index()
            && self.tabs.activate(index)
        {
            self.load_activated_tab();
        }
        let full = ui.available_rect_before_wrap();
        let import_status = self.hidden_import_status();
        // The status line is its own panel now (T109): the image area is the whole central area.
        let rect = full;
        let ppp = ctx.pixels_per_point();
        // The zoom keys act on the current image (U6-Q8): while comparing, around the pointer
        // only when it is on the current side. While locked, the held side follows it below
        // (T154).
        let key_rect = self
            .compare_sides(rect)
            .map_or(rect, |(_, current)| current);
        let key_anchor = self.anchor(ctx.input(|input| input.pointer.hover_pos()), key_rect, ppp);
        let mut dialog_consumes_keys = self.import_dialog.is_some() && !self.import_hidden;
        let mut start_import = false;
        for (tab_action, key, ctrl, shift, alt, repeat) in key_actions {
            // The confirmation of a delete takes every key while it is shown (T189 point 4):
            // Escape cancels, Enter cancels unless the Delete button has the focus (then egui
            // clicks it), every other key does nothing.
            if self.delete_window_shown() {
                let delete_focused = self
                    .delete_button
                    .is_some_and(|id| ctx.memory(|memory| memory.has_focus(id)));
                if key == egui::Key::Escape || (key == egui::Key::Enter && !delete_focused) {
                    self.delete_idle();
                }
                continue;
            }
            // The question of "Open in {name}" above 100 images takes every key while it is shown
            // (ADR 0019 §2.5, T202), as the delete's confirmation: Escape cancels, Enter cancels
            // unless the Open button has the focus (then egui clicks it), every other key does
            // nothing.
            if self.open_window_shown() {
                let open_focused = self
                    .open_button
                    .is_some_and(|id| ctx.memory(|memory| memory.has_focus(id)));
                if key == egui::Key::Escape || (key == egui::Key::Enter && !open_focused) {
                    self.open_cancelled();
                }
                continue;
            }
            // The metadata window is modal (D16, T107): no key reaches the loupe, the contact
            // sheet or the tabs, and keys typed into its fields reach nothing else. Escape is its
            // "Avbryt"; Enter does not press OK.
            if let Some(window) = &mut self.metadata_window {
                if key == egui::Key::Escape && !ctrl && !alt {
                    // An open "▼" list takes Escape; the window's own Escape is not done (T141).
                    if window.value_list.is_some() {
                        window.value_list_escape = true;
                    } else {
                        window.escape = true;
                    }
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
            // The preview's own keys first (ADR 0017 §2.2, §2.4, U3-Q1–Q3, T111): S, H and I,
            // and nothing for Ctrl+O, Ctrl+I, Ctrl+W, the tab keys and F11. Not while a text
            // field has the keyboard or a dialog holds the program.
            if self.in_preview()
                && let Some(preview_key) = preview_key_action(key, ctrl, shift, alt)
            {
                if !ctx.egui_wants_keyboard_input() && !self.dialog_open() {
                    match preview_key {
                        PreviewKey::OnlyMarked => self.toggle_only_marked(),
                        PreviewKey::HideParts => toggle_preview_hidden(&mut self.settings.shell),
                        PreviewKey::ToggleColumn => {
                            toggle_preview_column(&mut self.settings.shell);
                        }
                        PreviewKey::ClipHighlights => self.toggle_clip_highlights(),
                        PreviewKey::ClipShadows => self.toggle_clip_shadows(),
                        PreviewKey::FocusMarking => self.toggle_focus_marking(),
                        PreviewKey::Compare => self.toggle_compare(),
                        PreviewKey::HoldImage => self.hold_current_image(),
                        PreviewKey::SwapImages => self.swap_images(),
                        PreviewKey::AboveBelow => self.toggle_compare_layout(),
                        PreviewKey::LockZoom => self.toggle_compare_lock(),
                        PreviewKey::Nothing => {}
                    }
                    ctx.request_repaint();
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
            // P and Ctrl+Z (ADR 0017 U9-Q6, U9-Q8, T174) in the contact sheet and the preview; not
            // while a text field has the keyboard or a dialog holds the program. P goes through its
            // command (nothing while the setting is off); Ctrl+Z with nothing to undo says so.
            if !ctx.egui_wants_keyboard_input()
                && let Some(move_key) = move_key_action(key, ctrl, shift, alt)
            {
                if !self.dialog_open() {
                    match move_key {
                        MoveKey::MovePicked => self.run_command(&ctx, Command::MovePicked),
                        MoveKey::UndoMove => self.undo_move(),
                    }
                    ctx.request_repaint();
                }
                continue;
            }
            // Delete (ADR 0018 §2.1, B4, T189) in the contact sheet and the preview, through its
            // command; not while a text field has the keyboard or a dialog holds the program.
            if !ctx.egui_wants_keyboard_input() && delete_key_action(key, ctrl, shift, alt) {
                if !self.dialog_open() {
                    self.run_command(&ctx, Command::DeleteToRecycleBin);
                    ctx.request_repaint();
                }
                continue;
            }
            // F5 (ADR 0026 §3.1, F-Q1, T242) in the contact sheet and the preview: "Reload
            // Folder" through its command; not while a text field has the keyboard or a dialog
            // holds the program, not on a repeat (a held F5 reloads once), and not when the left
            // column took it this frame for the library's "Refresh" (U4-Q1).
            if !ctx.egui_wants_keyboard_input() && reload_key_action(key, ctrl, shift, alt) {
                if !repeat && !self.dialog_open() && !self.library_took_f5 {
                    self.run_command(&ctx, Command::ReloadFolder);
                    ctx.request_repaint();
                }
                continue;
            }
            // Ctrl+E (ADR 0019 §2.2, T202) in the contact sheet and the preview: the first program
            // of the list, or the Settings window while the list is empty, through its command;
            // not while a text field has the keyboard or a dialog holds the program, and not on a
            // repeat (a held Ctrl+E starts the program once).
            if !ctx.egui_wants_keyboard_input() && open_in_key_action(key, ctrl, shift, alt) {
                if !repeat && !self.dialog_open() {
                    let command = if self.settings.external_programs.is_empty() {
                        Command::OpenInAnother
                    } else {
                        Command::OpenIn(0)
                    };
                    self.run_command(&ctx, command);
                    ctx.request_repaint();
                }
                continue;
            }

            if !ctx.egui_wants_keyboard_input()
                && let Some(action) = rating_key_action(key, ctrl, shift, alt)
            {
                let (scroll, moves) = self.rate(action);
                if let Some(position) = scroll {
                    scroll_to = position;
                }
                self.queue_moves(MoveKind::Reject, moves);
                continue;
            }

            // Ctrl+← and Ctrl+→ turn the images a rating key acts on (ADR 0024 §2.1, O-Q1, T237);
            // a held key turns once.
            if !ctx.egui_wants_keyboard_input()
                && let Some(clockwise) = rotation_key_action(key, ctrl, shift, alt)
            {
                if !repeat {
                    self.rotate(&ctx, clockwise);
                }
                continue;
            }

            // M opens the metadata window of the current image in the loupe and the contact sheet
            // (D16); with several images marked on the current or the first marked image (U12c-Q1,
            // T126); with a kind without metadata support nothing happens.
            if !ctx.egui_wants_keyboard_input()
                && key_action(key, ctrl, alt) == Some(Action::OpenMetadata)
            {
                self.act(Action::OpenMetadata, key_anchor);
                ctx.request_repaint();
                continue;
            }

            // I opens and collapses the inspector in the loupe and the contact sheet (ADR 0017
            // §2.4, U2-Q3, T110); not while a text field has the keyboard or a dialog holds the
            // program.
            if !ctx.egui_wants_keyboard_input()
                && key_action(key, ctrl, alt) == Some(Action::ToggleInfo)
            {
                if !self.dialog_open() {
                    self.act(Action::ToggleInfo, None);
                    ctx.request_repaint();
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
                    Some(GridKeyAction::SelectAll) => self.select_all(),
                    Some(GridKeyAction::ClearSelection) => self.clear_selection(),
                    None => {}
                },
                Some(ViewMode::Loupe) => {
                    if self.compare.is_some()
                        && key == egui::Key::Escape
                        && loupe_key_action(key, ctrl, alt).is_some()
                    {
                        // Escape closes an open compare first and the current image stays in
                        // the preview (U6-Q11, T153); G and Enter close the preview, and the
                        // compare with it (`follow_compare`).
                        self.close_compare();
                        ctx.request_repaint();
                    } else if loupe_key_action(key, ctrl, alt).is_some() {
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
        // The compare closes by itself when its preview, tab or held image is gone (T153); checked
        // here every frame, before anything of it is drawn.
        self.follow_compare();
        // The clipping map is wanted exactly when the preview is shown with an overlay on (T148).
        self.wish_clipping_map();
        // The focus map likewise, with the focus marking on (T168).
        self.wish_focus_map();
        // The preview opened or closed by a key this frame (T111): the next frame draws the new
        // layout, and the contact sheet scrolls to the last shown image there.
        let mode_after = self.tabs.active().map(|tab| tab.mode);
        if mode_after != mode_before {
            if mode_after == Some(ViewMode::Grid) {
                self.scroll_request = scroll_to;
            }
            ui.painter().rect_filled(full, 0.0, BACKGROUND);
            ctx.request_repaint();
            return;
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
            // Not behind the metadata window, the confirmation of a delete (T189 point 4) or the
            // question of "Open in {name}" (T202).
            if self.metadata_window.is_none()
                && !self.delete_window_shown()
                && !self.open_window_shown()
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
            // No clipping marks in the contact sheet: their texture is released (T149), and so
            // the focus marking's (T168).
            self.clip_overlay.release();
            self.second_clip_overlay.release();
            self.focus_overlay.release();
            self.second_focus_overlay.release();
            // The second view core's textures are synced here too (T153): a compare that closed
            // with the preview emptied the core, and its dropped textures are taken back and its
            // views drained. With an empty list and nothing dropped it does nothing.
            let max_side = ctx.input(|i| i.max_texture_side);
            self.second_textures.sync(&ctx, &self.second_core, max_side);
            self.draw_grid(ui, full, rect, scroll_to, commands);
            return;
        }
        // The background first, then the image area (T152): the function draws one view core's
        // image into its rectangle; what belongs to the current image only is kept here.
        ui.painter().rect_filled(full, 0.0, BACKGROUND);
        // The tab's current image comes from the tab, not from `frame.pos`: every step goes through
        // the tab and the core follows it (ADR 0014 §4.4). The status line counts the whole folder.
        let total = self.tabs.active().map_or(0, |tab| tab.files.len());
        let shown = ImageAreaShown {
            anchor_at_pointer: self.anchor_at_pointer,
            choice: ClipChoice {
                highlights: self.settings.shell.clip_highlights,
                shadows: self.settings.shell.clip_shadows,
            },
            focus: self.settings.shell.focus_marking,
            metadata: &self.metadata,
            indicator_after: self.indicator_after,
            message: self.message.as_deref(),
            total,
        };
        // While comparing, the image area is two halves (U6-Q5, T153): the held image left or on
        // top from the second view core, the current image right or below from the first.
        let halves = self.compare_sides(rect);
        let current_rect = halves.map_or(rect, |(_, current)| current);
        // The right-click menu on the image, on both sides of the compare, acting on the current
        // image as the keys do; none while a dialog holds the program (ADR 0019 §2.9, E-Q15,
        // E-Q16, T221).
        let context_menu = !self.dialog_open();
        let (frame, response) = draw_image_area(
            ui,
            &self.core,
            &self.core,
            &mut self.textures,
            &mut self.clip_overlay,
            &mut self.focus_overlay,
            current_rect,
            ui.scope_id().with("image"),
            &shown,
        );
        if context_menu {
            response.context_menu(|ui| self.image_context_menu(ui, commands));
        }
        // The held side's menu is attached after the `match`: `open` borrows `self.compare`.
        let mut held_response = None;
        match (halves, &mut self.compare) {
            (Some((held_rect, _)), Some(open)) => {
                // The held side starts as the current one is shown: the display mode when the
                // compare opens and whenever `R` changes it (U6-Q9), the zoom and centre once
                // when it opens and when the held image changes. Unlocked, each side has its
                // own zoom and position after that (U6-Q8, free); locked, the held side is
                // given the current side's zoom and centre whenever they differ from what was
                // sent last (T151's `follow`, T154).
                if open.mode_sent != Some(frame.mode) {
                    self.second_core.set_mode(frame.mode);
                    open.mode_sent = Some(frame.mode);
                }
                let locked = self.settings.shell.compare_locked;
                if let Some((zoom, centre)) =
                    follow(locked, (frame.zoom, frame.centre), open.view_sent)
                {
                    self.second_core.set_view(zoom, centre);
                    open.view_sent = Some((zoom, centre));
                    open.view_pending = false;
                } else if open.view_pending {
                    self.second_core.set_view(frame.zoom, frame.centre);
                    open.view_pending = false;
                }
                // Locked, the mouse on the held side acts on the current side's core, at the same
                // offset in its half (the halves are equal), and the held side follows (T154).
                let held_mouse = if locked {
                    &self.core
                } else {
                    &self.second_core
                };
                let (held_frame, held) = draw_image_area(
                    ui,
                    &self.second_core,
                    held_mouse,
                    &mut self.second_textures,
                    &mut self.second_clip_overlay,
                    &mut self.second_focus_overlay,
                    held_rect,
                    ui.scope_id().with("held-image"),
                    &shown,
                );
                held_response = Some(held);
                // The held image's size became known: the view sent before it was clamped without
                // it (T150), so it is sent again at the next frame (T154).
                if !open.held_size_known && held_frame.size.is_some() {
                    open.held_size_known = true;
                    if locked {
                        open.view_sent = None;
                        ui.ctx().request_repaint();
                    }
                }
                // Each side's line at its top (U6-Q10), and the current side's frame.
                paint_side_text(ui, held_rect, &compare_side_text(&held_frame));
                paint_side_text(ui, current_rect, &compare_side_text(&frame));
                ui.painter().rect_stroke(
                    current_rect,
                    0.0,
                    egui::Stroke::new(2.0, MARKED_BLUE),
                    egui::StrokeKind::Inside,
                );
            }
            _ => {
                // Outside the compare the second view core holds nothing: its textures are
                // synced as the first's (T152), and it has no clipping or focus marks to keep.
                let max_side = ctx.input(|i| i.max_texture_side);
                self.second_textures.sync(&ctx, &self.second_core, max_side);
                self.second_clip_overlay.release();
                self.second_focus_overlay.release();
            }
        }
        if let Some(held) = held_response
            && context_menu
        {
            held.context_menu(|ui| self.image_context_menu(ui, commands));
        }
        self.last_frame_status = frame.status.clone();
        self.last_zoom = Some((frame.zoom, frame.scale));
        // The clipping state, for the preview column's Clipping section (T148).
        self.last_clipping = frame.clipping.clone();
        // The statistics of the shown image, for the Histogram sections (T142); the path as
        // `inspector_subject` resolves it, so the sections can tell whose they are.
        let cwd = std::env::current_dir().ok();
        self.last_statistics = frame.path.as_deref().map(|path| {
            let path = match cwd.as_deref() {
                Some(cwd) => resolve_path(cwd, path),
                None => path.to_path_buf(),
            };
            (path, frame.statistics.clone())
        });
        // The inspector keeps them too, so it reads nothing for this image (U7c-Q1, T144).
        self.display_mode = frame.mode;
        if let Some((path, Some(statistics))) = &self.last_statistics {
            self.histograms
                .preview_result(path, frame.source, statistics);
        }
        let metadata_path = frame.path.as_deref();
        let metadata_notice = self.metadata.notice(metadata_path);
        if self.metadata.is_loading(metadata_path) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        // The status line is always shown in full, with the RAW/JPEG control (T110).
        let mut parts = vec![status_line(&frame, self.anchor_at_pointer, total)];
        if let Some(m) = &self.message {
            parts.push(m.clone());
        }
        if let Some(sorting) = self.sort_status() {
            parts.push(sorting);
        }
        if let Some(notice) = &metadata_notice {
            parts.push(notice.clone());
        }
        if let Some(rating) = self.metadata.rating_summary(frame.path.as_deref()) {
            parts.push(rating);
        }
        if let Some(status) = import_status {
            parts.push(status);
        }
        let line = status_line_text(&parts);
        // The display-mode control (G4: a key and a control) sits in the status bar.
        self.next_status = StatusBar {
            line,
            mode: Some(mode_label(frame.mode)),
        };
    }
}

/// What the image area shows besides the view core's image (T152): the caller's state the
/// drawing reads.
struct ImageAreaShown<'a> {
    /// Zooming goes around the pointer (otherwise around the view's centre).
    anchor_at_pointer: bool,
    /// The clipping overlays that are on.
    choice: ClipChoice,
    /// The focus marking is on (T168).
    focus: bool,
    /// The shown ratings, for the rating badge.
    metadata: &'a MetadataCache,
    /// The loading indicator appears after this long (V2).
    indicator_after: Duration,
    /// The message shown in place of the status line when the area has no image.
    message: Option<&'a str>,
    /// The number of files the status line of an empty area counts.
    total: usize,
}

/// The pointer's place in the image area in view pixels, which a zoom goes around; `None` (the
/// view's centre) when `anchor_at_pointer` is off or the pointer is outside `rect`.
fn zoom_anchor(
    anchor_at_pointer: bool,
    pointer: Option<egui::Pos2>,
    rect: egui::Rect,
    ppp: f32,
) -> Option<[f64; 2]> {
    if !anchor_at_pointer {
        return None;
    }
    let p = pointer.filter(|p| rect.contains(*p))?;
    Some([
        f64::from((p.x - rect.min.x) * ppp),
        f64::from((p.y - rect.min.y) * ppp),
    ])
}

/// Draws one view core's image area into `rect` (ADR 0017 §2.2, U6, T152): the mouse
/// (double-click = Z, drag = pan, wheel = zoom) under the interaction `id`, acting on
/// `mouse_core` (the same core, or the current side's while the compare is locked, T154), the
/// sync of the core's `textures`, the frame asked of the core, the image, the focus marking from
/// `focus_overlay` and over it the clipping marks from `clip_overlay`, the rating badge, and the error, indicator and empty-area texts. The
/// background is the caller's. Returns the core's frame and the area's response, to which the
/// caller attaches the right-click menu (ADR 0019 §2.9, T221).
#[allow(clippy::too_many_arguments)] // one core's area, the core its mouse acts on, its state
fn draw_image_area(
    ui: &egui::Ui,
    core: &ViewCore<NativeBackend>,
    mouse_core: &ViewCore<NativeBackend>,
    textures: &mut Textures,
    clip_overlay: &mut OverlayCache<egui::TextureHandle>,
    focus_overlay: &mut FocusCache<egui::TextureHandle>,
    rect: egui::Rect,
    id: egui::Id,
    shown: &ImageAreaShown<'_>,
) -> (Frame, egui::Response) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let response = ui.interact(rect, id, egui::Sense::click_and_drag());
    let pointer = response.hover_pos();
    let anchor = zoom_anchor(shown.anchor_at_pointer, pointer, rect, ppp);
    // mouse: double-click = Z, drag = pan, wheel = zoom
    if response.double_clicked() {
        mouse_core.toggle_zoom(anchor);
    } else if response.dragged() {
        let d = response.drag_delta();
        if d != egui::Vec2::ZERO {
            mouse_core.pan([f64::from(d.x * ppp), f64::from(d.y * ppp)]);
        }
    }
    if response.hovered() {
        let dy = ctx.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            mouse_core.zoom_by(wheel_factor(dy), anchor);
        }
    }
    // views to textures, then the frame
    let max_side = ctx.input(|i| i.max_texture_side);
    textures.sync(&ctx, core, max_side);
    let frame = core.frame(view_px(rect, ppp));
    // Clipped to `rect`, so one side of the compare never draws on the other (T153).
    let painter = ui.painter_at(rect);
    let image_shown = textures.paint(&painter, rect, ppp, &frame);
    // The focus marking over the image (ADR 0017 §2.6, U9a, T168), drawn as the clipping marks
    // below and before them, so a clipping mark is on top; from this frame's map and `Draw` only.
    match (&frame.focus, frame.draw) {
        (FocusState::Ready { map, uv }, Some(draw)) if image_shown && shown.focus => {
            let marks = focus_overlay.texture(map, max_side, |overlay| {
                ctx.load_texture(
                    "focus-marks",
                    egui::ColorImage::from_rgba_unmultiplied(overlay.size, &overlay.rgba),
                    egui::TextureOptions::NEAREST,
                )
            });
            if let Some((texture, uv_scale)) = marks {
                let draw = pw_viewcore::Draw {
                    uv: overlay_uv(*uv, uv_scale),
                    ..draw
                };
                painter.add(egui::Shape::mesh(pw_render::mesh(
                    &draw,
                    rect.min,
                    ppp,
                    texture.id(),
                )));
            }
        }
        _ => focus_overlay.release(),
    }
    // The clipping marks over the image (ADR 0017 §2.6, U7b, T149), from this frame's map and
    // `Draw` only, so they are never drawn on another image: the image's quad once more, with
    // the map's texture coordinates and nearest-neighbour sampling (sharp blocks, U7b-Q7).
    let choice = shown.choice;
    match (&frame.clipping, frame.draw) {
        (ClippingState::Ready { map, uv }, Some(draw)) if image_shown && choice.any() => {
            let marks = clip_overlay.texture(map, choice, max_side, |overlay| {
                ctx.load_texture(
                    "clipping-marks",
                    egui::ColorImage::from_rgba_unmultiplied(overlay.size, &overlay.rgba),
                    egui::TextureOptions::NEAREST,
                )
            });
            if let Some((texture, uv_scale)) = marks {
                let draw = pw_viewcore::Draw {
                    uv: overlay_uv(*uv, uv_scale),
                    ..draw
                };
                painter.add(egui::Shape::mesh(pw_render::mesh(
                    &draw,
                    rect.min,
                    ppp,
                    texture.id(),
                )));
            }
        }
        _ => clip_overlay.release(),
    }
    // The rating badge is always shown (T110).
    if let Some(rating) = frame
        .path
        .as_deref()
        .and_then(|path| shown.metadata.shown_rating(path))
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
            let due = shown.indicator_after.saturating_sub(since.elapsed());
            ctx.request_repaint_after(due.max(Duration::from_millis(20)));
        }
        Status::Empty => match shown.message {
            Some(message) => centre_text(message),
            None => centre_text(&status_line(&frame, true, shown.total)),
        },
        Status::Ready => {}
    }
    (frame, response)
}

/// The line at the top of one side of the compare (U6-Q10, T153): `text` centred at the top of
/// `rect`, on a dark plate so it can be read over the image; a text wider than the side wraps.
fn paint_side_text(ui: &egui::Ui, rect: egui::Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);
    let galley = painter.layout(
        text.to_owned(),
        egui::FontId::proportional(STATUS_TEXT_SIZE),
        image_area::TEXT,
        (rect.width() - 4.0 * SIDE_TEXT_MARGIN).max(0.0),
    );
    let size = galley.size();
    let plate = egui::Rect::from_min_size(
        egui::pos2(
            rect.center().x - size.x / 2.0 - SIDE_TEXT_MARGIN,
            rect.min.y + SIDE_TEXT_MARGIN,
        ),
        size + egui::vec2(2.0 * SIDE_TEXT_MARGIN, SIDE_TEXT_MARGIN),
    );
    painter.rect_filled(plate, 3.0, SIDE_TEXT_PLATE);
    painter.galley(
        egui::pos2(
            plate.min.x + SIDE_TEXT_MARGIN,
            plate.min.y + SIDE_TEXT_MARGIN / 2.0,
        ),
        galley,
        image_area::TEXT,
    );
}

/// The range from the smallest to the largest file index (+1), for the thumbnail worker's
/// priority (ADR 0014 §4.2); empty without indexes.
fn index_span(indexes: impl Iterator<Item = usize> + Clone) -> std::ops::Range<usize> {
    match (indexes.clone().min(), indexes.max()) {
        (Some(first), Some(last)) => first..last.saturating_add(1),
        _ => 0..0,
    }
}

/// The folders of `images`, each once (compared as given), in the order first met (T174).
fn image_folders<'a>(images: impl Iterator<Item = &'a Path>) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    for folder in images.filter_map(Path::parent) {
        if !folders.iter().any(|known| known == folder) {
            folders.push(folder.to_path_buf());
        }
    }
    folders
}

/// The file name of `image` as the status line names it, or the whole path without one (T174).
fn image_name(image: &Path) -> String {
    image.file_name().map_or_else(
        || image.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Each image of a Delete Rejected Images batch with its partners, as the tab's resolved paths
/// (`Loupe::delete_partners`, T190 point 6).
type ImagePartners = Vec<(PathBuf, Vec<PathBuf>)>;

/// The partners kept for `image` (compared as `DeleteBatch` compares its images); none for an
/// image not listed.
fn partners_of<'a>(partners: &'a [(PathBuf, Vec<PathBuf>)], image: &Path) -> &'a [PathBuf] {
    partners
        .iter()
        .find(|(known, _)| known == image)
        .map_or(&[], |(_, files)| files.as_slice())
}

/// Whether `path` is an image file, one of `pw_raw::SUPPORTED_EXTENSIONS` without regard to case,
/// and not a companion file (T189's `plan_delete`).
fn is_image_file(path: &Path) -> bool {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .is_some_and(|extension| pw_raw::SUPPORTED_EXTENSIONS.contains(&extension.as_str()))
}

/// `pw_raw::SUPPORTED_EXTENSIONS` as the metadata worker's move and delete requests take them.
fn image_extensions() -> Vec<String> {
    pw_raw::SUPPORTED_EXTENSIONS
        .iter()
        .map(|extension| (*extension).to_owned())
        .collect()
}

/// The paths as the metadata cache keys them (`MetadataCache::begin_load`): relative paths are
/// taken from the current folder.
fn resolve_paths(files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let cwd = std::env::current_dir().map_err(|e| path_resolution_error_text(&e.to_string()))?;
    Ok(files.iter().map(|path| resolve_path(&cwd, path)).collect())
}

/// The properties the default layout's form shows, left out of the window's other metadata (D16,
/// T107, T123; U10b-Q3, T278): every field's property once, also
/// `Iptc4xmpCore:CreatorContactInfo`, the read-only Urgency, Category and Supplemental Categories,
/// the structure lists (T136, T139, T143), `xmp:Rating` and `xmp:Label` (T124). Only the tests use
/// it; the window takes `shown_properties` of the layout in use.
#[cfg(test)]
fn window_editor_properties() -> Vec<&'static str> {
    EditorLayout::default().shown_properties()
}

/// A click on the Settings window's page Metadata Window (T278), applied to the chosen layout
/// after the rows are drawn.
enum LayoutClick {
    /// A section's check box: shown or hidden.
    SectionShown(EditorSection, bool),
    /// A section's Move Up.
    SectionUp(EditorSection),
    /// An item's check box: shown or hidden.
    ItemShown(EditorItem, bool),
    /// An item's Move Up, inside its section.
    ItemUp(EditorItem),
}

/// The rows of the Settings window's page Metadata Window (ADR 0017 U10b-Q6, T278): for each
/// section of `layout` in its order, Move Up (greyed on the first) and a check box named by the
/// section's heading; under it, indented, each of the section's items in its order, Move Up (greyed
/// on the section's first item) and a check box named by the item's label. The items of a hidden
/// section are greyed, their own checks kept. Move Up comes first, so a long name wraps beside it.
/// Returns the clicks.
fn draw_layout_rows(ui: &mut egui::Ui, layout: &EditorLayout) -> Vec<LayoutClick> {
    let mut clicks = Vec::new();
    for (index, (section, shown)) in layout.sections().into_iter().enumerate() {
        ui.push_id(("settings-layout-section", section), |ui| {
            ui.horizontal_wrapped(|ui| {
                let up = egui::Button::new(settings_text(SettingsText::MoveUp));
                if ui.add_enabled(index > 0, up).clicked() {
                    clicks.push(LayoutClick::SectionUp(section));
                }
                let mut checked = shown;
                let heading = editor_text(&EditorText::SectionHeading(section));
                if ui.checkbox(&mut checked, heading).changed() {
                    clicks.push(LayoutClick::SectionShown(section, checked));
                }
            });
            ui.indent("items", |ui| {
                ui.add_enabled_ui(shown, |ui| {
                    for (item_index, (item, item_shown)) in
                        layout.items(section).into_iter().enumerate()
                    {
                        ui.push_id(("settings-layout-item", item), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let up = egui::Button::new(settings_text(SettingsText::MoveUp));
                                if ui.add_enabled(item_index > 0, up).clicked() {
                                    clicks.push(LayoutClick::ItemUp(item));
                                }
                                let mut checked = item_shown;
                                if ui.checkbox(&mut checked, item.label()).changed() {
                                    clicks.push(LayoutClick::ItemShown(item, checked));
                                }
                            });
                        });
                    }
                });
            });
        });
    }
    clicks
}

/// What the metadata window's form reports after it was drawn.
struct FormResponse {
    /// The conflict value chosen: the conflict's and the value's index.
    choice: Option<(usize, usize)>,
    /// The sections whose heading was clicked.
    toggled: Vec<EditorSection>,
    /// The rating or label button pressed (T124).
    rating: Option<RatingKey>,
}

/// The contents of the metadata window's form (D16, T107, T123, T124, T128; U10b, T278), top to
/// bottom: the sections `layout` shows, in its order, each a collapsible heading open or collapsed
/// as `shell` has it, and in each the items the layout shows, in its order: a field (the Licence
/// (PLUS) section's choice lists, T128), the contact fields under the heading of their structure,
/// "Creator's Contact Info" (T114), or a structure list (`draw_structure_list`, T136, T139, T143).
/// Then the open conflicts with a choice each and the warnings (editable when `enabled`), every
/// open conflict listed whatever is hidden (U10b-Q4); then read-only, in small text, the other
/// metadata (without an origin line, T114; what the layout hides is there, U10b-Q3), the
/// conflicts a rule decided (each value with its origin) and the notices, each only when it has
/// rows. Labels and values wrap at the window's width. The section Rating and Label (T124) holds
/// the buttons of the image's shown `rating` instead of fields, usable when `rating_enabled` (also
/// while the form's write runs, not while "Save changes?" is asked); while it is hidden, the other
/// metadata starts with that rating's status line (U10b-Q9).
#[allow(clippy::too_many_arguments)] // the window's form, its state, its layout and two flags
fn draw_window_form(
    ui: &mut egui::Ui,
    state: &mut FormState,
    errors: &[(EditorField, String)],
    warnings: &[String],
    enabled: bool,
    shell: &ShellSettings,
    layout: &EditorLayout,
    rating: Option<&Rating>,
    rating_enabled: bool,
    values: &mut ValueUi<'_>,
) -> FormResponse {
    let warning = ui.visuals().warn_fg_color;
    let mut choice = None;
    let mut toggled = Vec::new();
    let mut rating_choice = None;
    let sections = layout.shown_sections();
    for section in sections.iter().copied() {
        let section_enabled = if section == EditorSection::RatingLabel {
            rating_enabled
        } else {
            enabled
        };
        let shown = ui
            .add_enabled_ui(section_enabled, |ui| {
                egui::CollapsingHeader::new(editor_text(&EditorText::SectionHeading(section)))
                    .id_salt(("metadata-window-section", section))
                    .open(Some(shell.editor_section(section) == ColumnState::Open))
                    .show(ui, |ui| {
                        if section == EditorSection::RatingLabel {
                            rating_choice = draw_rating_buttons(ui, rating);
                        }
                        for item in layout.shown_items(section) {
                            match item {
                                EditorItem::Field(_) | EditorItem::ContactInfo => {
                                    // `draw_window_field` draws "Creator's Contact Info" once,
                                    // before the first contact field.
                                    for field in item.fields() {
                                        draw_window_field(
                                            ui,
                                            &mut state.form,
                                            field,
                                            errors,
                                            values,
                                        );
                                    }
                                }
                                EditorItem::List(list) => {
                                    draw_structure_list(ui, &mut state.form, list);
                                }
                            }
                        }
                    })
            })
            .inner;
        if shown.header_response.clicked() {
            toggled.push(section);
        }
    }

    ui.add_enabled_ui(enabled, |ui| {
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
    let grey = ui.visuals().weak_text_color();
    let others = other_values(&state.read, &layout.shown_properties());
    // With Rating and Label hidden, the rating keys still rate; the current rating is shown here
    // so a key press is seen at once (U10b-Q9).
    let rating_line =
        (!sections.contains(&EditorSection::RatingLabel)).then(|| status_text(rating));
    if !others.is_empty() || rating_line.is_some() {
        ui.separator();
        ui.label(egui::RichText::new(editor_text(&EditorText::OtherValuesHeading)).strong());
        if let Some(line) = rating_line {
            ui.label(egui::RichText::new(line).small());
            ui.add_space(PANEL_ROW_SPACING);
        }
        for row in &others {
            ui.label(egui::RichText::new(&row.label).strong().small());
            ui.label(egui::RichText::new(&row.value).small());
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
    FormResponse {
        choice,
        toggled,
        rating: rating_choice,
    }
}

/// The buttons of the Rating and Label section (ADR 0017 U10c-Q1, T124): a row of no stars, 1–5
/// and Reject, then a row of the five labels and no label, the button of the shown `rating`
/// selected. The rows wrap at the window's width. Returns the rating key of the button pressed.
fn draw_rating_buttons(ui: &mut egui::Ui, rating: Option<&Rating>) -> Option<RatingKey> {
    let mut chosen = None;
    for row in [&RatingButton::STARS[..], &RatingButton::LABELS[..]] {
        ui.horizontal_wrapped(|ui| {
            for button in row {
                let text = editor_text(&EditorText::RatingButton(*button));
                if ui.selectable_label(button.selected(rating), text).clicked() {
                    chosen = Some(button.key());
                }
            }
        });
        ui.add_space(PANEL_ROW_SPACING);
    }
    chosen
}

/// One field of the metadata window's form (T107, T123): its name (with the full IPTC name as a
/// tooltip where the name shortens it, T117), the note of a Lang Alt without `x-default`, then a
/// text box, a check box (Copyrighted, U10-Q3), a drop-down (a choice list, T128) or, for a
/// read-only field, its text; then its
/// errors. The heading "Creator's Contact Info" comes before the first contact field (T114). A
/// deprecated field the file does not have is not drawn.
fn draw_window_field(
    ui: &mut egui::Ui,
    form: &mut EditorForm,
    field: EditorField,
    errors: &[(EditorField, String)],
    values: &mut ValueUi<'_>,
) {
    let read_only = form.read_only_text(field).map(str::to_string);
    if read_only.is_none() && !field.editable() {
        return;
    }
    let first_contact_field = EditorField::ALL
        .into_iter()
        .find(|field| field.struct_field().is_some());
    if Some(field) == first_contact_field {
        ui.label(egui::RichText::new(property_label(field.property(), Language::En)).strong());
    }
    if field.check_box() && read_only.is_none() {
        let mut checked = form.checked(field);
        let response = ui.checkbox(&mut checked, field.label());
        if response.changed() {
            form.set_checked(field, checked);
        }
    } else {
        let label = ui.label(egui::RichText::new(field.label()).strong());
        // The full IPTC name of a shortened name (glossary G1, T117).
        if let Some(tooltip) = field.tooltip() {
            label.on_hover_text(tooltip);
        }
        if let Some(note) = form.note(field) {
            let grey = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(note).color(grey).small());
        }
        match read_only {
            Some(text) => {
                ui.add(egui::Label::new(text).wrap());
            }
            None if field.choice_list().is_some() => {
                draw_choice_list(ui, form, field);
            }
            None if form.values_shown(field) => {
                draw_values_field(ui, form, field, values);
            }
            None => {
                if let Some(text) = form.text_mut(field) {
                    let edit = if field.multiline() {
                        egui::TextEdit::multiline(text).desired_rows(3)
                    } else {
                        egui::TextEdit::singleline(text)
                    };
                    ui.add(edit.desired_width(f32::INFINITY));
                }
            }
        }
    }
    let error_color = ui.visuals().error_fg_color;
    for (_, error) in errors.iter().filter(|(f, _)| *f == field) {
        ui.label(egui::RichText::new(error).color(error_color));
    }
    ui.add_space(PANEL_ROW_SPACING);
}

/// The "▼" lists of the metadata window's form while it is drawn (ADR 0017 U12a, T141): the
/// recent and own values, the open list, and what was pressed in it.
struct ValueUi<'a> {
    values: &'a FieldValues,
    list: &'a mut Option<ValueList>,
    /// "Add to My Values" and "×" pressed this frame, applied after the form is drawn.
    actions: Vec<ValueAction>,
}

/// What was pressed in a "▼" list that changes the own values (U12a-Q3, T141).
enum ValueAction {
    /// "Add to My Values": the field's present values.
    AddOwn(EditorField, Vec<String>),
    /// "×" beside an own value.
    RemoveOwn(EditorField, String),
}

/// A one-line field with a "▼" (ADR 0017 U12a-Q5, T141): the text box as wide as the window
/// beside a small "▼" (tooltip "Recent and saved values") that opens and closes its list; Alt+↓
/// in the field opens it too. The open list is drawn under the field: "Recent" and "My Values",
/// each only with values ("No values yet" when both are empty), a "×" beside each own value, then
/// "Add to My Values" (disabled when the field is empty). A click, or ↑/↓ and Enter, chooses a
/// value into the field ([`EditorForm::use_value`]) and closes the list; Escape closes it (taken
/// before the window's own Escape).
fn draw_values_field(
    ui: &mut egui::Ui,
    form: &mut EditorForm,
    field: EditorField,
    values: &mut ValueUi<'_>,
) {
    let store = values.values;
    let id = ui.make_persistent_id(("metadata-window-field", field));
    // Alt+↓ in the field opens its list (the Windows custom for a drop-down); before the text
    // box is drawn, so the key does not reach it.
    let focused = ui.memory(|memory| memory.has_focus(id));
    if focused
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown))
    {
        *values.list = Some(ValueList::new(field));
    }
    let choices: Vec<ValueChoice> = value_choices(store, field);
    let mut chosen: Option<String> = None;
    let mut chosen_by_key = false;
    if let Some(list) = values.list.as_mut().filter(|list| list.field == field) {
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                list.step(true, choices.len());
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                list.step(false, choices.len());
            }
            if let Some(value) = list.chosen(&choices)
                && input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
            {
                chosen = Some(value.to_string());
                chosen_by_key = true;
            }
        });
    }
    let open = values.list.is_some_and(|list| list.field == field);
    let mut toggle = false;
    // One row as high as a text box: a right-to-left layout alone would take the rest of the
    // section's height (the owner's hand check of T141).
    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        row,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            toggle = values_button(ui)
                .on_hover_text(editor_text(&EditorText::ValuesTooltip))
                .clicked();
            if let Some(text) = form.text_mut(field) {
                ui.add(
                    egui::TextEdit::singleline(text)
                        .id(id)
                        .desired_width(f32::INFINITY),
                );
            }
        },
    );
    if toggle {
        *values.list = if open {
            None
        } else {
            Some(ValueList::new(field))
        };
    }
    let highlighted = match *values.list {
        Some(list) if list.field == field => Some(list.highlighted),
        _ => None,
    };
    if let Some(highlighted) = highlighted {
        let recent = store.recent(field).len();
        let present = form.present_values(field);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            if choices.is_empty() {
                let grey = ui.visuals().weak_text_color();
                ui.label(egui::RichText::new(editor_text(&EditorText::NoValuesYet)).color(grey));
            }
            for (index, choice) in choices.iter().enumerate() {
                if index == 0 && recent > 0 {
                    ui.label(egui::RichText::new(editor_text(&EditorText::Recent)).strong());
                }
                if index == recent {
                    ui.label(egui::RichText::new(editor_text(&EditorText::MyValues)).strong());
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(highlighted == Some(index), &choice.value)
                        .clicked()
                    {
                        chosen = Some(choice.value.clone());
                    }
                    if choice.own
                        && ui
                            .small_button(editor_text(&EditorText::RemoveValue))
                            .clicked()
                    {
                        values
                            .actions
                            .push(ValueAction::RemoveOwn(field, choice.value.clone()));
                    }
                });
            }
            ui.separator();
            let add = ui.add_enabled(
                !present.is_empty(),
                egui::Button::new(editor_text(&EditorText::AddToMyValues)),
            );
            if add.clicked() {
                values.actions.push(ValueAction::AddOwn(field, present));
            }
        });
    }
    if let Some(value) = chosen {
        form.use_value(field, &value);
        *values.list = None;
        if chosen_by_key {
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }
}

/// The "▼" button of a field with values (U12a-Q5): a square button with a downward triangle
/// painted on it, as the default fonts have no "▼" glyph (the owner's hand check of T141).
fn values_button(ui: &mut egui::Ui) -> egui::Response {
    let side = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        ui.painter().rect(
            rect,
            visuals.corner_radius,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let centre = rect.center();
        let half = side * 0.2;
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                centre + egui::vec2(-half, -half * 0.5),
                centre + egui::vec2(half, -half * 0.5),
                centre + egui::vec2(0.0, half * 0.7),
            ],
            visuals.fg_stroke.color,
            egui::Stroke::NONE,
        ));
    }
    response
}

/// One structure list of the metadata window's form, drawn where the layout puts it (T136, T139,
/// T143; U10b, T278). The list's name, then one framed group per entry with its sub-fields (names
/// from `property_label`, one-line text boxes as wide as the window, a drop-down for a Licensor's
/// telephone types, a Licensor's address under a collapsed "Address" line, an Artwork or Object's
/// further sub-fields under a collapsed "More" line) and "Remove", then "Add Location" or
/// "Add {list}", disabled at the list's limit with the tooltip "At most {n} entries"; under
/// Location Shown also "Copy from Location Created" (U11a-Q4), under Image Creator "Copy from
/// Creator" (U11b-Q4), under Person Shown with Details "Copy from Person Shown" (U11c-Q6). A list
/// with an undecided conflict, or a value the form cannot show, is its read-only text instead.
fn draw_structure_list(ui: &mut egui::Ui, form: &mut EditorForm, list: &'static str) {
    ui.label(egui::RichText::new(property_label(list, Language::En)).strong());
    if let Some(text) = form.list_read_only(list).map(str::to_string) {
        ui.add(egui::Label::new(text).wrap());
        ui.add_space(PANEL_ROW_SPACING);
        return;
    }
    let mut removed = None;
    for entry in 0..form.list_count(list) {
        ui.push_id(("metadata-window-list", list, entry), |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                for field in EditorForm::list_fields(list) {
                    draw_list_field(ui, form, list, entry, field);
                }
                let address = EditorForm::list_address_fields(list);
                if !address.is_empty() {
                    egui::CollapsingHeader::new(editor_text(&EditorText::Address))
                        .id_salt("address")
                        .default_open(false)
                        .show(ui, |ui| {
                            for field in address {
                                draw_list_field(ui, form, list, entry, field);
                            }
                        });
                }
                // An Artwork or Object's eleven further sub-fields (U11c-Q3, T143).
                let more = EditorForm::list_more_fields(list);
                if !more.is_empty() {
                    egui::CollapsingHeader::new(editor_text(&EditorText::More))
                        .id_salt("more")
                        .default_open(false)
                        .show(ui, |ui| {
                            for field in more {
                                draw_list_field(ui, form, list, entry, field);
                            }
                        });
                }
                if ui
                    .button(editor_text(&EditorText::RemoveLocation))
                    .clicked()
                {
                    removed = Some(entry);
                }
            });
        });
        ui.add_space(PANEL_ROW_SPACING);
    }
    if let Some(entry) = removed {
        form.remove_entry(list, entry);
    }
    ui.horizontal_wrapped(|ui| {
        let add_text = if EditorForm::LOCATION_LISTS.contains(&list) {
            editor_text(&EditorText::AddLocation)
        } else {
            editor_text(&EditorText::AddEntry { list })
        };
        let add = ui.add_enabled(form.can_add_entry(list), egui::Button::new(add_text));
        let add = match EditorForm::max_entries(list) {
            Some(max) => {
                add.on_disabled_hover_text(editor_text(&EditorText::AtMostEntries { max }))
            }
            None => add,
        };
        if add.clicked() {
            form.add_entry(list);
        }
        if list == EditorForm::LOCATION_SHOWN {
            let copy = ui.add_enabled(
                form.can_copy_location_created(),
                egui::Button::new(editor_text(&EditorText::CopyFromLocationCreated)),
            );
            if copy.clicked() {
                form.copy_location_created();
            }
        }
        if list == EditorForm::IMAGE_CREATOR {
            let copy = ui.add_enabled(
                form.can_copy_creator(),
                egui::Button::new(editor_text(&EditorText::CopyFromCreator)),
            );
            if copy.clicked() {
                form.copy_creator();
            }
        }
        if list == EditorForm::PERSON_DETAILS {
            let copy = ui.add_enabled(
                form.can_copy_person_shown(),
                egui::Button::new(editor_text(&EditorText::CopyFromPersonShown)),
            );
            if copy.clicked() {
                form.copy_person_shown();
            }
        }
    });
    ui.add_space(PANEL_ROW_SPACING);
}

/// One sub-field of a structure list's entry (T136, T139): its name from `property_label`, then a
/// one-line text box as wide as the window, or a drop-down for a choice (a Licensor's telephone
/// types, U11b-Q3): the empty entry, the PLUS values with their full URIs as tooltips, and an
/// unknown value of the file as "Unknown value: {uri}" (U10d-Q4). Under a text box its error, in
/// the error colour, while it has one (an Artwork or Object's Date Created, U11c-Q4, T143).
fn draw_list_field(
    ui: &mut egui::Ui,
    form: &mut EditorForm,
    list: &'static str,
    entry: usize,
    field: &'static str,
) {
    ui.label(egui::RichText::new(property_label(field, Language::En)).strong());
    let entries = form.list_choice_entries(list, entry, field);
    if entries.is_empty() {
        if let Some(text) = form.list_text_mut(list, entry, field) {
            ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
        }
        if let Some(error) = form.list_error(list, entry, field) {
            let error_color = ui.visuals().error_fg_color;
            ui.add(egui::Label::new(egui::RichText::new(error).color(error_color)).wrap());
        }
        return;
    }
    let current = form.list_text(list, entry, field).to_string();
    let selected = entries
        .iter()
        .find(|choice| choice.value == current)
        .map(|choice| choice.text.clone())
        .unwrap_or_default();
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("metadata-window-list-choice", field))
        .selected_text(selected)
        .width(ui.available_width())
        .wrap_mode(egui::TextWrapMode::Wrap)
        .show_ui(ui, |ui| {
            for choice in &entries {
                let response = ui.selectable_label(choice.value == current, &choice.text);
                let response = match &choice.tooltip {
                    Some(tooltip) => response.on_hover_text(tooltip),
                    None => response,
                };
                if response.clicked() {
                    chosen = Some(choice.value.clone());
                }
            }
        });
    if let Some(value) = chosen {
        form.set_list_choice(list, entry, field, &value);
    }
}

/// The drop-down of a choice-list field (ADR 0017 U10d-Q3, T128): the form's entries (the empty
/// entry, the vocabulary's values, an unknown or retired value of the file), the field's value
/// selected, each entry's full URI as its tooltip. As wide as the window; the entries wrap.
fn draw_choice_list(ui: &mut egui::Ui, form: &mut EditorForm, field: EditorField) {
    let entries = form.choice_entries(field);
    let current = form.text(field).to_string();
    let selected = entries
        .iter()
        .find(|entry| entry.value == current)
        .map(|entry| entry.text.clone())
        .unwrap_or_default();
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("metadata-window-choice", field))
        .selected_text(selected)
        .width(ui.available_width())
        .wrap_mode(egui::TextWrapMode::Wrap)
        .show_ui(ui, |ui| {
            for entry in &entries {
                let response = ui.selectable_label(entry.value == current, &entry.text);
                let response = match &entry.tooltip {
                    Some(tooltip) => response.on_hover_text(tooltip),
                    None => response,
                };
                if response.clicked() {
                    chosen = Some(entry.value.clone());
                }
            }
        });
    if let Some(value) = chosen {
        form.set_choice(field, &value);
    }
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

/// The file's own Exif orientation of `path`, read on the UI thread (one header read) when the
/// orientation reader cannot answer (T237); `None` only when the read fails.
fn embedded_orientation(path: &Path) -> Option<u16> {
    pw_raw::inspect(path).ok().map(|info| info.orientation)
}

/// The XMP orientation read for a tab file (ADR 0024 §2.4, T232), looked up by the resolved path
/// as `shown_rating` does.
fn read_orientation(metadata: &MetadataCache, cwd: Option<&Path>, path: &Path) -> Option<u16> {
    match cwd {
        Some(cwd) if !path.is_absolute() => metadata.orientation(&resolve_path(cwd, path)),
        _ => metadata.orientation(path),
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

/// The `pending` of `Tab::relist` (ADR 0026 F1, T240 point 5, T242, T251): whether RawStride is
/// writing a tab file (`MetadataCache::write_pending`: a rating or rotation write, an editor
/// save or a write of "Apply to All Marked"), looked up by the resolved path as `shown_rating`
/// does. Such a file counts as listed, since the safe write may rename it away for a moment. It is
/// used by every listing of a tab: Reload Folder, `relist_tabs` after a batch or an undo, and the
/// listings the folder watcher starts. Accepted edge (T251): a move waits only for `is_pending`,
/// so a file moved while "Apply to All Marked" still has it pending counts as listed and stays in
/// the source tab until the next listing.
fn tab_pending<'a>(
    metadata: &'a MetadataCache,
    cwd: Option<&'a Path>,
) -> impl Fn(&Path) -> bool + 'a {
    move |path| match cwd {
        Some(cwd) if !path.is_absolute() => metadata.write_pending(&resolve_path(cwd, path)),
        _ => metadata.write_pending(path),
    }
}

/// What `sort_tabs` did to the active tab (T242).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortChange {
    /// No file of the active tab changed place.
    Unchanged,
    /// A new order moved a file (`SortFollow::Reordered { moved: true }`).
    Reordered,
    /// Waiting files were placed and a file moved (`SortFollow::Placed { moved: true }`): the
    /// contact sheet keeps its top image (ADR 0026 §2 point 5).
    Placed,
}

/// Whether "Reload Folder" can be chosen (ADR 0026 F1, T242): the active tab has a folder, also
/// one with no images (F5 is how it shows new files), and no batch of moves, undo or delete runs
/// (`busy`), since a reload then could list a file on its way.
fn reload_enabled(tab: Option<&Tab>, busy: bool) -> bool {
    !busy && tab.is_some_and(|tab| tab.folder.is_some())
}

/// Whether the preview must go back to the contact sheet (ADR 0014 §2.3, T242 point 9.1): the
/// active tab shows the preview, nothing is visible, and the ratings are ready, so nothing will be
/// shown by them (while they are read, an active filter shows nothing and the preview waits).
fn preview_must_leave(tab: Option<&Tab>, ratings_ready: bool) -> bool {
    ratings_ready && tab.is_some_and(|tab| tab.mode == ViewMode::Loupe && tab.visible.is_empty())
}

/// `Tab::relist`, and the image an undo brings back made current also when the listing brought
/// nothing new and took nothing away (T242): `relist` then returns early without using
/// `current`, so `replace_files` with the same files makes it current, as the undo did before
/// the placement. `current` is in the tab's own form.
#[allow(clippy::too_many_arguments)]
fn relist_tab(
    tab: &mut Tab,
    listed: Vec<PathBuf>,
    current: Option<&Path>,
    pending: &dyn Fn(&Path) -> bool,
    order: SortOrder,
    ratings_ready: bool,
    rating_of: &dyn Fn(&Path) -> Option<Rating>,
    modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
) -> Relisted {
    let relisted = tab.relist(
        listed,
        current,
        pending,
        order,
        ratings_ready,
        rating_of,
        modified_of,
    );
    if !relisted.changed()
        && let Some(current) = current
        && tab.files.iter().any(|file| file == current)
    {
        let files = tab.files.clone();
        tab.replace_files(files, Some(current), ratings_ready, rating_of);
    }
    relisted
}

/// The decision of "Reload Folder" on the active tab (ADR 0026 §2, §3.1, T242): `listing` is
/// `list_folder`'s result for the tab's folder, in the tab's own form. A listing that failed
/// leaves the tab unchanged and gives `Err` with `folder_gone_text` (F-Q4). Otherwise the tab is
/// relisted (`relist_tab`, no file made current; `pending` names the files RawStride is writing,
/// which count as listed) and `Ok` holds the status text to show when the current image was
/// removed (`removed_by_another_program_text`, F-Q5), else `None`. A tab without a folder:
/// `Ok(None)`, nothing changed.
#[allow(clippy::too_many_arguments)]
fn reload_tab(
    tab: &mut Tab,
    listing: Result<Vec<PathBuf>, String>,
    pending: &dyn Fn(&Path) -> bool,
    order: SortOrder,
    ratings_ready: bool,
    rating_of: &dyn Fn(&Path) -> Option<Rating>,
    modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
) -> Result<Option<String>, String> {
    let relisted = list_into_tab(
        tab,
        listing,
        pending,
        order,
        ratings_ready,
        rating_of,
        modified_of,
    )?;
    Ok(relisted
        .current_removed
        .as_deref()
        .map(removed_by_another_program_text))
}

/// One listing of a tab's folder applied to the tab (ADR 0026 F3, T251), used by Reload Folder
/// (`reload_tab`) and by the listings the folder watcher starts (`list_active_from_disk`):
/// `listing` is `list_folder`'s result for the tab's folder, in the tab's own form. A listing that
/// failed leaves the tab unchanged and gives `Err` with `folder_gone_text` (F-Q4; also a folder
/// renamed or moved while open). Otherwise the tab is relisted with `relist_tab` (no file made
/// current; a file `pending` names counts as listed) and its `Relisted` returned. A tab without a
/// folder: `Ok` with nothing changed.
#[allow(clippy::too_many_arguments)]
fn list_into_tab(
    tab: &mut Tab,
    listing: Result<Vec<PathBuf>, String>,
    pending: &dyn Fn(&Path) -> bool,
    order: SortOrder,
    ratings_ready: bool,
    rating_of: &dyn Fn(&Path) -> Option<Rating>,
    modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
) -> Result<Relisted, String> {
    let Some(folder) = tab.folder.clone() else {
        return Ok(Relisted::default());
    };
    let Ok(listed) = listing else {
        return Err(folder_gone_text(&folder));
    };
    Ok(relist_tab(
        tab,
        listed,
        None,
        pending,
        order,
        ratings_ready,
        rating_of,
        modified_of,
    ))
}

/// Whether RawStride's own work holds back a listing of `folder` from disk (ADR 0026 F3, T251):
/// a batch of moves or an undo runs or waits (`moves_busy`), a delete runs (`delete_busy`), a
/// dialog that holds the program is open (`dialog_open`), or `folder` is the destination of a
/// running import or inside it (`inside_lexically`, no file access). These change the tabs
/// themselves when they end (`replace_tab_files`, `relist_tabs`, an import's `Placed`), and
/// while they run a file can be missing for a moment (a file on its way in a move, an import's
/// `.pwpart` renamed and its template written with ExifTool's `-overwrite_original`), which a
/// listing would report as removed by another program. While held, the tab's `changed_on_disk`
/// is kept, and the folder is listed once the hold ends (`follow_disk_changes` runs every
/// frame).
fn disk_listing_held(
    moves_busy: bool,
    delete_busy: bool,
    dialog_open: bool,
    import_destination: Option<&Path>,
    folder: &Path,
) -> bool {
    moves_busy
        || delete_busy
        || dialog_open
        || import_destination.is_some_and(|destination| inside_lexically(folder, destination))
}

/// What an import's placed group does to its folder's tabs (ADR 0026 §3.4, T260).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlacedTabs {
    /// The folder has tabs: each gets the group's images.
    Fill,
    /// The folder has no tab and this is its first group, made active (F-Q10): it opens as the
    /// active tab.
    OpenActive,
    /// The folder has no tab and this is its first group, not made active: it opens behind the
    /// active tab.
    OpenBehind,
    /// The folder has no tab and had images placed before: the user closed its tab during the
    /// import, so it is not opened again.
    Closed,
}

/// The choice of `PlacedTabs` for one placed group (ADR 0026 §3.4, T260, T262): `has_tab` when a
/// tab shows its folder, `first_in_folder` when no image of the folder was placed earlier in the
/// import (`PlacedInFolder::first`), `make_active` when the group is made active (F-Q10).
fn placed_tabs(has_tab: bool, first_in_folder: bool, make_active: bool) -> PlacedTabs {
    match (has_tab, first_in_folder, make_active) {
        (true, _, _) => PlacedTabs::Fill,
        (false, true, true) => PlacedTabs::OpenActive,
        (false, true, false) => PlacedTabs::OpenBehind,
        (false, false, _) => PlacedTabs::Closed,
    }
}

/// A folder's listing while an import runs (ADR 0026 §3.4, T260): without the images `import`
/// has not shown yet (`ImportTabs::leave_out_waiting`); unchanged without an import.
fn leave_out_waiting(import: Option<&ImportTabs>, listing: Vec<PathBuf>) -> Vec<PathBuf> {
    match import {
        Some(import) => import.leave_out_waiting(listing),
        None => listing,
    }
}

/// `list_folder`'s files and position for `open` while an import may run (ADR 0026 §3.4, T260):
/// the files without the images `import` has not shown yet, and the position found again in
/// them. With `file_opened` (a file was opened, `pos` its index in `files`), the same path's new
/// index, or the first file when it was left out; otherwise `pos` within the new files.
fn opened_during_import(
    import: Option<&ImportTabs>,
    files: Vec<PathBuf>,
    pos: usize,
    file_opened: bool,
) -> (Vec<PathBuf>, usize) {
    let Some(import) = import else {
        return (files, pos);
    };
    let opened = file_opened.then(|| files.get(pos).cloned()).flatten();
    let files = import.leave_out_waiting(files);
    let pos = match opened {
        Some(opened) => files.iter().position(|file| *file == opened).unwrap_or(0),
        None => files.len().checked_sub(1).map_or(0, |last| pos.min(last)),
    };
    (files, pos)
}

/// The status text after a listing the folder watcher started (ADR 0026 §3.3, F-Q5, F-Q9, T251):
/// the current image removed has its own text (`removed_by_another_program_text`, as Reload
/// Folder), otherwise how many images came and went (`folder_changed_text`); `None` when nothing
/// changed. No sound and no dialog.
fn disk_change_text(relisted: &Relisted) -> Option<String> {
    match &relisted.current_removed {
        Some(file) => Some(removed_by_another_program_text(file)),
        None => folder_changed_text(relisted.added, relisted.removed),
    }
}

/// The capture times to ask for in the active tab under `order` (U14-Q4, ADR 0026 F-Q6, T242):
/// when the tab is in `order` already, only its waiting (`unplaced`) files not read or asked for
/// yet, so they are placed when they come; otherwise every file not read or asked for yet, the
/// shown images first (`read_order`).
fn capture_requests(tab: &Tab, order: SortOrder, in_flight: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let wanted = |path: &PathBuf| !tab.capture.is_read(path) && !in_flight.contains(path);
    if tab.sorted == Some(order) {
        return tab
            .unplaced
            .iter()
            .filter(|path| wanted(path))
            .cloned()
            .collect();
    }
    read_order(tab.files.len(), &tab.visible, tab.current, |index| {
        tab.files.get(index).is_none_or(|path| !wanted(path))
    })
    .into_iter()
    .filter_map(|index| tab.files.get(index).cloned())
    .collect()
}

/// The contact sheet as `draw_grid` laid it out in one frame (T242).
#[derive(Clone, Copy, Debug, PartialEq)]
struct GridView {
    /// The scroll offset (`viewport.min.y`).
    offset: f32,
    /// The view's height.
    height: f32,
    /// The column count used.
    columns: usize,
    /// The cell size (`thumb_size`).
    cell_size: u32,
}

/// What the contact sheet showed of one tab in a frame (ADR 0026 §2 point 5, T242): the tab's
/// index and folder, the scroll offset, the column count, the position in `visible` of the image
/// at the top, and the paths of the shown images from that position on (two screens' worth).
#[derive(Clone, Debug, Default, PartialEq)]
struct GridNote {
    tab: usize,
    folder: Option<PathBuf>,
    offset: f32,
    columns: usize,
    top: usize,
    from_top: Vec<PathBuf>,
}

/// The note of `tab` (at index `index`) drawn as `view` says (T242): the top position is the
/// start of `visible_range` with no margin; the paths taken from it are two screens' worth (the
/// rows the view's height holds, plus one, twice, and at least two rows). Nothing shown: no paths.
fn grid_note(tab: &Tab, index: usize, view: GridView) -> GridNote {
    let columns = view.columns.max(1);
    let top = visible_range(
        tab.visible.len(),
        columns,
        view.cell_size,
        GRID_SPACING as u32,
        view.offset,
        view.height,
        0,
    )
    .start;
    let stride = view.cell_size.max(1) as f32 + GRID_SPACING;
    let rows = if view.height.is_finite() && view.height > 0.0 {
        (view.height / stride).ceil() as usize
    } else {
        0
    };
    let count = rows
        .saturating_add(1)
        .saturating_mul(2)
        .max(2)
        .saturating_mul(columns);
    let from_top = tab
        .visible
        .iter()
        .skip(top)
        .take(count)
        .filter_map(|&file| tab.files.get(file).cloned())
        .collect();
    GridNote {
        tab: index,
        folder: tab.folder.clone(),
        offset: view.offset,
        columns,
        top,
        from_top,
    }
}

/// What a pending anchor does in a frame of the contact sheet (T242).
#[derive(Clone, Copy, Debug, PartialEq)]
enum AnchorStep {
    /// Set this scroll offset once; the anchor is dropped.
    Set(f32),
    /// Nothing is shown yet (an active filter while the ratings are read): keep waiting.
    Wait,
    /// None of its images is shown: drop it.
    Drop,
}

/// The offset a pending anchor sets on `tab` (ADR 0026 §2 point 5, T242): the first image of
/// `anchor.from_top` still shown (`top_position`) keeps the height it had on the screen
/// (`anchored_offset` with `stride`). The rows are taken with each layout's own column count
/// (the note's for the old top, `columns` for the new one), so `anchored_offset` gets them with
/// one column; with the same count this is `anchored_offset(offset, stride, columns, old_top,
/// new_top)`. While nothing is shown and the ratings are not ready it waits; otherwise, with
/// none of its images shown, it is dropped.
fn anchor_step(
    anchor: &GridNote,
    tab: &Tab,
    ratings_ready: bool,
    stride: f32,
    columns: usize,
) -> AnchorStep {
    let shown: Vec<PathBuf> = tab
        .visible
        .iter()
        .filter_map(|&file| tab.files.get(file).cloned())
        .collect();
    match top_position(&anchor.from_top, &shown) {
        Some(new_top) => AnchorStep::Set(anchored_offset(
            anchor.offset,
            stride,
            1,
            anchor.top / anchor.columns.max(1),
            new_top / columns.max(1),
        )),
        None if shown.is_empty() && !ratings_ready => AnchorStep::Wait,
        None => AnchorStep::Drop,
    }
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

/// The text of a cell without a thumbnail, in the middle of its image area.
fn cell_text(painter: &egui::Painter, image_rect: egui::Rect, text: String) {
    painter.text(
        image_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.0),
        image_area::TEXT,
    );
}

/// The parts of one thumbnail cell: the whole cell, the image area and the label bar.
#[derive(Clone, Copy)]
struct CellRects {
    cell: egui::Rect,
    image: egui::Rect,
    label: egui::Rect,
}

/// The symbol of a button on a thumbnail in its disc's square `rect` (ADR 0017 §2.8, U15-Q6,
/// T245), drawn with lines and shapes in `THUMB_BUTTON_SYMBOL`: no icon set or font glyph.
fn paint_thumb_button_symbol(painter: &egui::Painter, rect: egui::Rect, button: ThumbButton) {
    match button {
        ThumbButton::RotateLeft => paint_rotate_symbol(painter, rect, false),
        ThumbButton::RotateRight => paint_rotate_symbol(painter, rect, true),
        ThumbButton::Metadata => paint_info_symbol(painter, rect),
        ThumbButton::Preview => paint_magnifier_symbol(painter, rect),
    }
}

/// ↻ (`clockwise`) or ↺: an open arc of 270° with an arrow head at its end, near the top.
fn paint_rotate_symbol(painter: &egui::Painter, rect: egui::Rect, clockwise: bool) {
    let colour = image_area::THUMB_BUTTON_SYMBOL;
    let scale = rect.width() / 20.0;
    let centre = rect.center();
    let radius = 5.0 * scale;
    // Drawn clockwise (screen angles grow clockwise, y down) from the lower right to the upper
    // right; the anticlockwise symbol is its mirror image.
    let start = 30.0_f32.to_radians();
    let sweep = 270.0_f32.to_radians();
    let mirror = |offset: egui::Vec2| {
        if clockwise {
            centre + offset
        } else {
            centre + egui::vec2(-offset.x, offset.y)
        }
    };
    const STEPS: u16 = 24;
    let points: Vec<egui::Pos2> = (0..=STEPS)
        .map(|step| {
            let angle = start + sweep * f32::from(step) / f32::from(STEPS);
            mirror(egui::vec2(angle.cos(), angle.sin()) * radius)
        })
        .collect();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.8 * scale, colour),
    ));
    let end = start + sweep;
    let radial = egui::vec2(end.cos(), end.sin());
    let tangent = egui::vec2(-end.sin(), end.cos());
    let tip = radial * radius + tangent * 3.0 * scale;
    let back = radial * radius - tangent * 1.0 * scale;
    let head = vec![
        mirror(tip),
        mirror(back + radial * 3.0 * scale),
        mirror(back - radial * 3.0 * scale),
    ];
    painter.add(egui::Shape::convex_polygon(
        head,
        colour,
        egui::Stroke::NONE,
    ));
}

/// "i": a dot above a short vertical bar.
fn paint_info_symbol(painter: &egui::Painter, rect: egui::Rect) {
    let colour = image_area::THUMB_BUTTON_SYMBOL;
    let scale = rect.width() / 20.0;
    let centre = rect.center();
    painter.circle_filled(centre + egui::vec2(0.0, -4.5 * scale), 1.5 * scale, colour);
    painter.line_segment(
        [
            centre + egui::vec2(0.0, -1.5 * scale),
            centre + egui::vec2(0.0, 5.5 * scale),
        ],
        egui::Stroke::new(2.2 * scale, colour),
    );
}

/// A magnifying glass: a ring with a short handle to the lower right.
fn paint_magnifier_symbol(painter: &egui::Painter, rect: egui::Rect) {
    let colour = image_area::THUMB_BUTTON_SYMBOL;
    let scale = rect.width() / 20.0;
    let ring = rect.center() + egui::vec2(-1.5, -1.5) * scale;
    let radius = 4.0 * scale;
    painter.circle_stroke(ring, radius, egui::Stroke::new(1.8 * scale, colour));
    let diagonal = egui::vec2(1.0, 1.0) / 2.0_f32.sqrt();
    painter.line_segment(
        [
            ring + diagonal * radius,
            rect.center() + egui::vec2(5.5, 5.5) * scale,
        ],
        egui::Stroke::new(2.4 * scale, colour),
    );
}

/// One thumbnail cell of the contact sheet or the filmstrip (ADR 0011, ADR 0013, ADR 0014 §5.1):
/// the faint fill of a marked cell, the thumbnail fitted into the image area (aspect kept), or the
/// error of a failed one, the dimming of a rejected image, the label bar (label pill, stars, file
/// name), then the frames: a marked cell has its outline outside the cell, the current cell
/// inside, so a current marked cell shows both (owner, 2026-11-13).
#[allow(clippy::too_many_arguments)] // one cell, with everything it shows
fn paint_cell(
    painter: &egui::Painter,
    rects: CellRects,
    path: &Path,
    rating: Option<&Rating>,
    marked: bool,
    current: bool,
    state: CellState,
    texture: Option<&egui::TextureHandle>,
) {
    let CellRects {
        cell: cell_rect,
        image: image_rect,
        label: label_rect,
    } = rects;
    if marked {
        painter.rect_filled(cell_rect, 0.0, MARKED_FILL);
    }
    painter.rect_filled(image_rect, 0.0, image_area::CELL);
    match state {
        CellState::Ready => {
            if let Some(texture) = texture {
                let size = texture.size_vec2();
                let scale = (image_rect.width() / size.x.max(1.0))
                    .min(image_rect.height() / size.y.max(1.0));
                let fitted = size * scale;
                let fitted_rect = egui::Rect::from_center_size(image_rect.center(), fitted);
                painter.image(
                    texture.id(),
                    fitted_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        // A file without a preview has its own state (T116); a failed one shows its error.
        CellState::NoPreview => cell_text(painter, image_rect, no_preview_text().to_string()),
        CellState::Failed(error) => cell_text(painter, image_rect, failed_cell_text(&error)),
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
        painter,
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

    if marked {
        painter.rect_stroke(
            cell_rect,
            0.0,
            egui::Stroke::new(2.0, MARKED_BLUE),
            egui::StrokeKind::Outside,
        );
    }
    if current {
        painter.rect_stroke(
            cell_rect,
            0.0,
            egui::Stroke::new(2.0, MARKED_BLUE),
            egui::StrokeKind::Inside,
        );
    }
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
            '×' => {
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

/// The Windows file dialog that chooses a program's `.exe` for the list of external programs
/// (ADR 0019 E-Q2, T200); `None` when it is cancelled.
fn pick_program() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(settings_text(SettingsText::ChooseProgram))
        .add_filter(settings_text(SettingsText::ProgramsFilter), &["exe"])
        .pick_file()
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

/// A section of the library (U4-Q7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LibrarySection {
    Folders,
    Favourites,
    Recent,
}

/// What was clicked in the library, done after the column is drawn (`Loupe::library_action`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum LibraryAction {
    /// A folder in the tree, a favourite or a recent folder (U4-Q6).
    Open(PathBuf),
    /// A triangle in the tree.
    Toggle(PathBuf),
    AddFavourite(PathBuf),
    RemoveFavourite(PathBuf),
    ClearRecent,
    /// "Refresh" or F5.
    Refresh,
    ToggleSection(LibrarySection),
}

/// How far each level of the tree is indented, in points.
const TREE_INDENT: f32 = 12.0;
/// The share of the column's height each section's scroll area may take when it is long: the
/// tree, then each list.
const TREE_HEIGHT_SHARE: f32 = 0.45;
const LIST_HEIGHT_SHARE: f32 = 0.2;

/// The library (ADR 0017 §2.1, U4-Q7, T135): the sections Folders, Favourites and Recent, each
/// collapsible as `shell` says and in its own scroll area when long. `active` is the
/// [`folder_key`] of the active tab's folder, highlighted in the tree when it is visible. Every
/// text comes from `library_text.rs`; names and paths are data.
fn draw_library(
    ui: &mut egui::Ui,
    library: &Library,
    shell: &ShellSettings,
    active: Option<&str>,
    actions: &mut Vec<LibraryAction>,
) {
    let height = ui.available_height().max(0.0);
    let folders = egui::CollapsingHeader::new(library_text(LibraryText::Folders))
        .id_salt("library-folders")
        .open(Some(shell.library_folders == ColumnState::Open))
        .show(ui, |ui| {
            if ui
                .small_button(library_text(LibraryText::Refresh))
                .on_hover_text(library_text(LibraryText::RefreshHover))
                .clicked()
            {
                actions.push(LibraryAction::Refresh);
            }
            egui::ScrollArea::both()
                .id_salt("library-tree")
                .max_height(height * TREE_HEIGHT_SHARE)
                .auto_shrink([false, true])
                .show(ui, |ui| draw_tree(ui, library, active, actions));
        });
    if folders.header_response.clicked() {
        actions.push(LibraryAction::ToggleSection(LibrarySection::Folders));
    }
    let favourites = egui::CollapsingHeader::new(library_text(LibraryText::Favourites))
        .id_salt("library-favourites")
        .open(Some(shell.library_favourites == ColumnState::Open))
        .show(ui, |ui| {
            draw_folder_list(
                ui,
                &library.favourites,
                LibrarySection::Favourites,
                height * LIST_HEIGHT_SHARE,
                actions,
            );
        });
    if favourites.header_response.clicked() {
        actions.push(LibraryAction::ToggleSection(LibrarySection::Favourites));
    }
    let recent = egui::CollapsingHeader::new(library_text(LibraryText::Recent))
        .id_salt("library-recent")
        .open(Some(shell.library_recent == ColumnState::Open))
        .show(ui, |ui| {
            draw_folder_list(
                ui,
                &library.recent,
                LibrarySection::Recent,
                height * LIST_HEIGHT_SHARE,
                actions,
            );
        });
    if recent.header_response.clicked() {
        actions.push(LibraryAction::ToggleSection(LibrarySection::Recent));
    }
    recent.header_response.context_menu(|ui| {
        if ui.button(library_text(LibraryText::ClearRecent)).clicked() {
            actions.push(LibraryAction::ClearRecent);
            ui.close();
        }
    });
}

/// What was clicked in the Tasks area, done after the column is drawn (`Loupe::task_action`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskAction {
    ToggleSection,
    /// A click on the import's row.
    ShowImport,
    Stop,
    /// "×" on a finished import's row.
    DismissImport,
    ClearErrors,
}

/// The least height of the error list's scroll area, in points, when the column is full.
const TASKS_ERRORS_MIN_HEIGHT: f32 = 80.0;

/// The Tasks area (ADR 0017 §2.1, U5, T140), a collapsible section under the library as `shell`
/// says: the import's row, the pending writes and the error list, or "No tasks" with nothing to
/// show. Every text comes from `tasks_text.rs` and `import_text.rs`; paths and errors are data.
fn draw_tasks(
    ui: &mut egui::Ui,
    tasks: &Tasks,
    shell: &ShellSettings,
    pending: &[PendingWrite],
    actions: &mut Vec<TaskAction>,
) {
    let section = egui::CollapsingHeader::new(tasks_text(TasksText::Tasks))
        .id_salt("library-tasks")
        .open(Some(shell.library_tasks == ColumnState::Open))
        .show(ui, |ui| {
            if tasks.is_empty() && pending.is_empty() {
                column_note(ui, tasks_text(TasksText::NoTasks));
                return;
            }
            if let Some(row) = tasks.import() {
                draw_import_row(ui, row, actions);
            }
            for write in pending {
                column_note(ui, &pending_write_text(*write));
            }
            let count = tasks.errors().len();
            if count > 0 {
                ui.add_space(PANEL_ROW_SPACING);
                ui.horizontal_wrapped(|ui| {
                    ui.strong(errors_heading_text(count));
                    if ui.small_button(tasks_text(TasksText::Clear)).clicked() {
                        actions.push(TaskAction::ClearErrors);
                    }
                });
                let error_colour = ui.visuals().error_fg_color;
                egui::ScrollArea::vertical()
                    .id_salt("tasks-errors")
                    .max_height(ui.available_height().max(TASKS_ERRORS_MIN_HEIGHT))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for error in tasks.errors() {
                            let text = error_entry_text(error.time, &error.text);
                            ui.add(
                                egui::Label::new(egui::RichText::new(text).color(error_colour))
                                    .wrap(),
                            );
                        }
                    });
            }
        });
    if section.header_response.clicked() {
        actions.push(TaskAction::ToggleSection);
    }
}

/// The import's row (U5-Q2, U5-Q3): source → destination, the progress bar by bytes and the
/// step's line, "Stop" while it runs and "×" when it is done; a click on the texts shows the
/// import dialog.
fn draw_import_row(ui: &mut egui::Ui, row: &ImportRow, actions: &mut Vec<TaskAction>) {
    let step = match &row.step {
        ImportStep::Copying {
            files_done,
            files_total,
            current,
            ..
        } => importing_text(*files_done, *files_total, current.as_deref()),
        ImportStep::Ejecting => ejecting_progress_text(Path::new(&row.source)),
        ImportStep::Done {
            error: Some(error), ..
        } => error.clone(),
        ImportStep::Done {
            copied,
            failed,
            cancelled,
            error: None,
        } => import_done_text(*copied, *failed, *cancelled),
    };
    let route = ui.add(
        egui::Label::new(import_route_text(&row.source, &row.destination))
            .sense(egui::Sense::click())
            .wrap(),
    );
    let mut show = route
        .on_hover_text(tasks_text(TasksText::ShowImport))
        .clicked();
    if row.running() {
        ui.add(egui::ProgressBar::new(row.fraction()));
    }
    ui.horizontal_wrapped(|ui| {
        let line = ui.add(egui::Label::new(step).sense(egui::Sense::click()).wrap());
        show |= line
            .on_hover_text(tasks_text(TasksText::ShowImport))
            .clicked();
        if row.running() {
            if ui.small_button(tasks_text(TasksText::Stop)).clicked() {
                actions.push(TaskAction::Stop);
            }
        } else if ui
            .small_button(tasks_text(TasksText::Dismiss))
            .on_hover_text(tasks_text(TasksText::DismissHover))
            .clicked()
        {
            actions.push(TaskAction::DismissImport);
        }
    });
    if show {
        actions.push(TaskAction::ShowImport);
    }
}

/// The folder tree's rows (U4-Q2): a triangle that only expands or collapses, and the name, whose
/// click opens the folder and whose right-click menu adds it to the favourites. A drive without a
/// disk has neither triangle nor click; a failed expand is one greyed line.
fn draw_tree(
    ui: &mut egui::Ui,
    library: &Library,
    active: Option<&str>,
    actions: &mut Vec<LibraryAction>,
) {
    let icon_size = egui::vec2(ui.spacing().icon_width, ui.spacing().icon_width);
    for row in library.tree.visible_rows() {
        ui.horizontal(|ui| {
            ui.add_space(row.depth as f32 * TREE_INDENT);
            match row.kind {
                TreeRowKind::Failed(reason) => {
                    ui.add_space(icon_size.x);
                    ui.label(egui::RichText::new(reason).weak());
                }
                TreeRowKind::Folder(node) if !node.expandable => {
                    ui.add_space(icon_size.x);
                    ui.label(egui::RichText::new(node.text.as_str()).weak());
                }
                TreeRowKind::Folder(node) => {
                    let (_, icon) = ui.allocate_exact_size(icon_size, egui::Sense::click());
                    let openness = if node.expanded { 1.0 } else { 0.0 };
                    egui::collapsing_header::paint_default_icon(ui, openness, &icon);
                    if icon.clicked() {
                        actions.push(LibraryAction::Toggle(node.path.clone()));
                    }
                    let highlighted = active.is_some_and(|key| node.is_folder(key));
                    let name = ui.selectable_label(highlighted, node.text.as_str());
                    if name.clicked() {
                        actions.push(LibraryAction::Open(node.path.clone()));
                    }
                    name.context_menu(|ui| {
                        if ui
                            .button(library_text(LibraryText::AddToFavourites))
                            .clicked()
                        {
                            actions.push(LibraryAction::AddFavourite(node.path.clone()));
                            ui.close();
                        }
                    });
                }
            }
        });
    }
}

/// The favourites or the recent folders (U4-Q3, U4-Q4): each by its name with the full path as
/// hover text; one that no longer exists is greyed with "Folder not found" and cannot be opened.
/// The right-click menu removes a favourite, or clears the recent folders.
fn draw_folder_list(
    ui: &mut egui::Ui,
    list: &FolderList,
    section: LibrarySection,
    max_height: f32,
    actions: &mut Vec<LibraryAction>,
) {
    if list.is_empty() {
        let empty = if section == LibrarySection::Favourites {
            LibraryText::NoFavourites
        } else {
            LibraryText::NoRecent
        };
        column_note(ui, library_text(empty));
        return;
    }
    let salt = if section == LibrarySection::Favourites {
        "library-favourites-list"
    } else {
        "library-recent-list"
    };
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .max_height(max_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for entry in list.entries() {
                let item = if entry.exists {
                    ui.selectable_label(false, entry.name.as_str())
                        .on_hover_text(entry.path.display().to_string())
                } else {
                    ui.add(
                        egui::Label::new(egui::RichText::new(entry.name.as_str()).weak())
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_text(library_text(LibraryText::FolderNotFound))
                };
                if entry.exists && item.clicked() {
                    actions.push(LibraryAction::Open(entry.path.clone()));
                }
                item.context_menu(|ui| {
                    let (text, action) = if section == LibrarySection::Favourites {
                        (
                            LibraryText::RemoveFromFavourites,
                            LibraryAction::RemoveFavourite(entry.path.clone()),
                        )
                    } else {
                        (LibraryText::ClearRecent, LibraryAction::ClearRecent)
                    };
                    if ui.button(library_text(text)).clicked() {
                        actions.push(action);
                        ui.close();
                    }
                });
            }
        });
}

/// The inspector's contents (ADR 0017 §2.1, T110): the file name of the current image (wrapped,
/// never cut), then the sections Info, Metadata and Histogram (§2.6, U7-Q7, T142), open or
/// collapsed as `shell` says. Every text comes from `inspector_text.rs`, `info_panel.rs`
/// (`panel_text`, `info_panel`), `preview_text.rs` (`histogram_text`) or `UNSUPPORTED_TEXT`, or
/// is data; labels and values wrap at the column's width. Returns which section headers were
/// clicked (Info, Metadata, Histogram).
fn draw_inspector(
    ui: &mut egui::Ui,
    image: Option<&Path>,
    shell: &ShellSettings,
    exif: &ExifInspector,
    metadata: &MetadataCache,
    histogram: &HistogramPanel,
) -> [bool; 3] {
    let Some(path) = image else {
        column_note(ui, &inspector_text(&InspectorText::NoImage));
        return [false; 3];
    };
    let file_name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    ui.add(egui::Label::new(egui::RichText::new(file_name).strong()).wrap());
    ui.add_space(PANEL_ROW_SPACING);
    let info = egui::CollapsingHeader::new(inspector_text(&InspectorText::Info))
        .id_salt("inspector-info")
        .open(Some(shell.inspector_info == ColumnState::Open))
        .show(ui, |ui| draw_exif_rows(ui, &exif.shown(path)));
    let values = egui::CollapsingHeader::new(inspector_text(&InspectorText::Metadata))
        .id_salt("inspector-metadata")
        .open(Some(shell.inspector_metadata == ColumnState::Open))
        .show(ui, |ui| draw_inspector_metadata(ui, path, metadata));
    let histogram_section = egui::CollapsingHeader::new(histogram_text(HistogramText::Section))
        .id_salt("inspector-histogram")
        .open(Some(shell.inspector_histogram == ColumnState::Open))
        .show(ui, |ui| draw_histogram(ui, histogram));
    [
        info.header_response.clicked(),
        values.header_response.clicked(),
        histogram_section.header_response.clicked(),
    ]
}

/// The height of the histogram in points.
const HISTOGRAM_HEIGHT: f32 = 96.0;
/// The histogram's background: dark in both themes, so the channels' colours mix on it.
const HISTOGRAM_BACKGROUND: egui::Color32 = egui::Color32::from_gray(22);
/// The colours of the R, G and B channels: partly transparent (premultiplied), so where the
/// channels overlap their colours mix.
const HISTOGRAM_COLOURS: [egui::Color32; 3] = [
    egui::Color32::from_rgba_premultiplied(200, 45, 45, 110),
    egui::Color32::from_rgba_premultiplied(45, 175, 45, 110),
    egui::Color32::from_rgba_premultiplied(55, 80, 225, 110),
];

/// The Histogram section (ADR 0017 §2.6, U7-Q7, T142), in the preview's column and in the
/// inspector: the three channels drawn over each other at the column's width, and under them
/// where the numbers come from, the clipped share per channel and the deep-shadow share; or the
/// text that the statistics are being read, or that the file has none. `panel` holds the bar
/// heights and the finished texts (`histogram.rs`, `preview_text.rs`); the lines wrap.
fn draw_histogram(ui: &mut egui::Ui, panel: &HistogramPanel) {
    let view = match panel {
        HistogramPanel::Reading => {
            ui.add(egui::Label::new(histogram_text(HistogramText::Reading)).wrap());
            return;
        }
        HistogramPanel::NoHistogram => {
            ui.add(egui::Label::new(histogram_text(HistogramText::NoHistogram)).wrap());
            return;
        }
        HistogramPanel::Ready(view) => view,
    };
    let width = ui.available_width().max(1.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(width, HISTOGRAM_HEIGHT), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, HISTOGRAM_BACKGROUND);
    let bin_width = rect.width() / HISTOGRAM_BINS as f32;
    for (bars, colour) in view.bars.iter().zip(HISTOGRAM_COLOURS) {
        let mut mesh = egui::Mesh::default();
        for (bin, height) in bars.iter().enumerate() {
            if *height <= 0.0 {
                continue;
            }
            let left = rect.left() + bin as f32 * bin_width;
            let bar = egui::Rect::from_min_max(
                egui::pos2(left, rect.bottom() - height * rect.height()),
                egui::pos2(left + bin_width, rect.bottom()),
            );
            mesh.add_colored_rect(bar, colour);
        }
        painter.add(egui::Shape::mesh(mesh));
    }
    ui.add_space(PANEL_ROW_SPACING);
    ui.add(egui::Label::new(egui::RichText::new(histogram_text(view.source)).weak()).wrap());
    ui.add(egui::Label::new(view.clipped.as_str()).wrap());
    ui.add(egui::Label::new(view.deep_shadows.as_str()).wrap());
}

/// The Info section: the Exif rows (label, value), or the text of loading, an error or no values.
fn draw_exif_rows(ui: &mut egui::Ui, shown: &ExifShown<'_>) {
    if let Some(text) = exif_state_text(shown) {
        ui.add(egui::Label::new(text).wrap());
        return;
    }
    if let ExifShown::Rows(rows) = shown {
        for row in *rows {
            ui.add(egui::Label::new(egui::RichText::new(exif_label(row.label)).weak()).wrap());
            ui.add(egui::Label::new(row.value.as_str()).wrap());
            ui.add_space(PANEL_ROW_SPACING);
        }
    }
}

/// The Metadata section, as the metadata window shows the values (T103's panel, D16): the values
/// without an origin line (T114), the conflicts (open and decided, each value with its origin) in
/// the warning colour and the notices, each of the last two only when there are any; read-only.
fn draw_inspector_metadata(ui: &mut egui::Ui, path: &Path, metadata: &MetadataCache) {
    let read = match metadata.inspector_state(path) {
        InspectorState::Unsupported => {
            ui.add(egui::Label::new(UNSUPPORTED_TEXT).wrap());
            return;
        }
        InspectorState::Loading => {
            ui.add(egui::Label::new(panel_text(&PanelText::Loading)).wrap());
            return;
        }
        InspectorState::Error(error) => {
            ui.add(egui::Label::new(panel_text(&PanelText::ReadError(error.to_owned()))).wrap());
            return;
        }
        InspectorState::Ready(read) => read,
    };
    let panel = info_panel(read);
    let grey = ui.visuals().weak_text_color();
    let warning = ui.visuals().warn_fg_color;
    let wrapped = |ui: &mut egui::Ui, text: egui::RichText| {
        ui.add(egui::Label::new(text).wrap());
    };

    wrapped(
        ui,
        egui::RichText::new(panel_text(&PanelText::Values)).strong(),
    );
    for row in &panel.values {
        wrapped(ui, egui::RichText::new(&row.label).strong());
        wrapped(ui, egui::RichText::new(&row.value));
        ui.add_space(PANEL_ROW_SPACING);
    }

    if !panel.conflicts.is_empty() {
        ui.separator();
        wrapped(
            ui,
            egui::RichText::new(panel_text(&PanelText::Conflicts))
                .strong()
                .color(warning),
        );
        for conflict in &panel.conflicts {
            wrapped(
                ui,
                egui::RichText::new(&conflict.label).strong().color(warning),
            );
            for value in &conflict.values {
                wrapped(ui, egui::RichText::new(&value.value).color(warning));
                wrapped(ui, egui::RichText::new(&value.origin).color(grey).small());
            }
            wrapped(
                ui,
                egui::RichText::new(&conflict.rule).color(warning).italics(),
            );
            ui.add_space(PANEL_ROW_SPACING);
        }
    }

    if !panel.notices.is_empty() {
        ui.separator();
        wrapped(
            ui,
            egui::RichText::new(panel_text(&PanelText::Notices)).strong(),
        );
        for notice in &panel.notices {
            wrapped(ui, egui::RichText::new(notice));
            ui.add_space(PANEL_ROW_SPACING);
        }
    }
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
        SubmitError::QueueFull => import_text(ImportText::QueueFull).to_string(),
        SubmitError::Stopped => import_text(ImportText::WorkerStopped).to_string(),
    }
}

impl eframe::App for Loupe {
    /// The main window (ADR 0017 §2.1, T109): the menu bar, the command bar, the status bar, the
    /// side columns and, in the middle, the folder tabs with the contact sheet or the loupe.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_import_worker(&ctx);
        self.poll_folder_watcher();
        // The active tab follows its folder on disk at once (ADR 0026 F3, T251).
        self.follow_disk_changes(&ctx);
        self.follow_tab_folders();
        self.check_exiftool(&ctx);
        if self.send_pending_options() {
            ctx.request_repaint_after(WINDOW_POLL_INTERVAL);
        }
        self.remember_window(&ctx);
        // The bars take no clicks while a dialog holds the program: the metadata window is modal
        // (D16), and the import and settings windows keep the program as their keys do.
        let blocked = self.dialog_open();
        let mut commands = Vec::new();
        // Set by the left column when it takes F5 this frame (T242); in the preview, or with the
        // column collapsed, it is not drawn.
        self.library_took_f5 = false;
        // Set again below when the central panel is drawn (T131).
        self.image_area = None;
        if self.in_preview() {
            // Only the preview: no menu bar, command bar, tabs, status bar or side columns
            // (U3-Q1, T111).
            self.draw_preview(ui, blocked, &mut commands);
        } else {
            let mut filter_choice = None;
            let mut favourite = None;
            egui::Panel::top("menu-bar").show(ui, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    self.draw_menu_bar(ui, &mut commands, &mut filter_choice, &mut favourite);
                });
            });
            if let Some(folder) = favourite {
                self.library_action(LibraryAction::AddFavourite(folder));
            }
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
            let central = egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(fill))
                .show(ui, |ui| self.draw(ui, &mut commands));
            self.image_area = Some(central.response.rect);
        }
        self.draw_import_dialog(&ctx);
        self.draw_settings_window(&ctx);
        self.draw_about_window(&ctx);
        self.draw_metadata_window(&ctx);
        self.draw_delete_window(&ctx);
        self.draw_open_window(&ctx);
        if !commands.is_empty() {
            for command in commands {
                self.run_command(&ctx, command);
            }
            ctx.request_repaint();
        }
        // A tab opened, closed or cleared this frame is watched or ended at once (T248).
        self.follow_tab_folders();
        self.follow_inspector(&ctx);
        self.gate_thumbnails();
        self.sync_preview_window();
        for command in self.viewport_commands.drain(..) {
            ctx.send_viewport_cmd(command);
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
        // Closed in the preview: the window as it was before the preview, not its full screen
        // (U3-Q6, T111).
        if let Some(before) = self.window_before {
            self.settings.shell = before.saved(&self.settings.shell);
        }
        // Quit with the import dialog open: its "copy new images only" is kept (T158).
        if let Some(dialog) = &self.import_dialog {
            self.settings.shell.import_incremental = dialog.form().incremental;
        }
        // The window, full screen and the columns as they are now (ADR 0017 §2.5, T109); a
        // failure is only logged, no dialog at exit.
        if let Some(path) = &self.settings_path
            && let Err(error) =
                save_settings_with_library(path, &self.settings, &self.library.saved())
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
                    .set_title(shell_text(ShellText::RecoveryTitle))
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

/// The fallback encodings of the settings window's drop-down, in its order.
const FALLBACK_CHOICES: [Fallback; 2] = [Fallback::Windows1252, Fallback::Latin1];

/// The settings and the library's lists at start (T106, T135): read from `path` with each problem
/// written to the log; the defaults without a path.
fn start_settings(path: Option<&Path>) -> (Settings, SavedLibrary) {
    let Some(path) = path else {
        eprintln!("pw-loupe: LOCALAPPDATA is not set; the default settings are used");
        return (Settings::default(), SavedLibrary::default());
    };
    let (settings, library, problems) = load_settings_and_library(path);
    for problem in problems {
        eprintln!("pw-loupe: {problem}");
    }
    (settings, library)
}

/// The recent and own values of the metadata window's fields (ADR 0017 U12a-Q6, T141) from
/// `field-values.txt`, each problem written to the log; none without a path.
fn start_field_values(path: Option<&Path>) -> FieldValues {
    let Some(path) = path else {
        return FieldValues::default();
    };
    let (values, problems) = load_field_values(path);
    for problem in problems {
        eprintln!("pw-loupe: {problem}");
    }
    values
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

/// The image check of the safe write (I9d): it closes both view cores' RAW sessions of the file
/// first, the second core's before the first's (T89, T152), and waits for the inspector's
/// histogram read of it (U7c-Q4, T144), each at most [`RELEASE_TIMEOUT`]. With `note_stamp`
/// (the metadata worker's writes, whose answers reach the inspector) the file's stamp just before
/// the write is noted as well (U7c-Q7, T145); the import's writes make new files and note nothing.
fn image_check(
    core: &ViewCore<NativeBackend>,
    second_core: &ViewCore<NativeBackend>,
    histogram_reads: &HistogramReads,
    note_stamp: bool,
) -> RawImageCheck {
    let releaser = core.releaser();
    let second_releaser = second_core.releaser();
    release_before_write(
        release_in_both(
            move |path| {
                let _ = second_releaser.release(path, RELEASE_TIMEOUT);
            },
            move |path| {
                let _ = releaser.release(path, RELEASE_TIMEOUT);
            },
        ),
        histogram_reads.clone(),
        note_stamp,
    )
}

/// One `release_core` for [`release_before_write`] out of the two view cores' releases (T152):
/// the file is released in the second core first, then in the first.
fn release_in_both(
    release_second: impl Fn(&Path) + Send + Sync + 'static,
    release_first: impl Fn(&Path) + Send + Sync + 'static,
) -> impl Fn(&Path) + Send + Sync + 'static {
    move |path| {
        release_second(path);
        release_first(path);
    }
}

/// The image check whose release first calls `release_core` (the view cores' sessions) and then
/// waits until the histogram reader no longer holds the file (U7c-Q4, T144), so a metadata write
/// never meets a file the reader has open. The read's result is still used: a metadata write
/// does not change the pixels. With `note_stamp`, the file's stamp just before the write is
/// noted in `histogram_reads` before the release returns, so a kept histogram follows the write
/// (U7c-Q7, T145). The release before a move (ADR 0017 §2.6, T177) does the same but never notes
/// the stamp, whatever `note_stamp` is: a move has no write answer that would take it back.
fn release_before_write(
    release_core: impl Fn(&Path) + Send + Sync + 'static,
    histogram_reads: HistogramReads,
    note_stamp: bool,
) -> RawImageCheck {
    let release_core = std::sync::Arc::new(release_core);
    let move_core = std::sync::Arc::clone(&release_core);
    let move_reads = histogram_reads.clone();
    RawImageCheck::new(move |path| {
        release_core(path);
        let _ = histogram_reads.wait_for(path, RELEASE_TIMEOUT);
        if note_stamp {
            histogram_reads.note_before_write(path);
        }
    })
    .with_move_release(move |path| {
        move_core(path);
        let _ = move_reads.wait_for(path, RELEASE_TIMEOUT);
    })
}

fn run(options: Options) -> Result<(), String> {
    // First: no Windows "There is no disk in the drive" dialog for an empty card reader or DVD
    // drive anywhere in the program, the drive list included (U4-Q1, T135).
    suppress_no_disk_dialogs();
    let journal = open_journal();
    let settings_path = settings_path();
    let (settings, saved_library) = start_settings(settings_path.as_deref());
    let field_values_path = field_values_path();
    let field_values = start_field_values(field_values_path.as_deref());
    let library = Library::new(&read_drives(), &saved_library);
    let config = Config {
        core_budget: options.core_mb << 20,
        texture_budget: options.texture_mb << 20,
        ..Config::default()
    };
    let indicator_after = config.indicator_after;
    let monitor_profile = load_monitor_profile(primary_monitor_profile_path());
    let title = window_title(&monitor_profile.source);
    // One backend for the view core and the inspector's histogram reader (U7c-Q2, T144).
    let backend =
        NativeBackend::with_tiff_limit(usize::try_from(config.core_budget).unwrap_or(usize::MAX))
            .with_monitor_profile(monitor_profile.profile.clone());
    // The second view core, for the held image of the compare (U6-Q2, U6-Q3, T152): a quarter of
    // the budgets, its own backend (the same monitor profile, the TIFF limit of its own session
    // budget). Started before the workers, whose image check releases a file in both cores.
    let second_config = Config {
        core_budget: config.core_budget / 4,
        texture_budget: config.texture_budget / 4,
        ..config.clone()
    };
    let second_backend = NativeBackend::with_tiff_limit(
        usize::try_from(second_config.core_budget).unwrap_or(usize::MAX),
    )
    .with_monitor_profile(monitor_profile.profile.clone());
    let core = ViewCore::start(backend.clone(), config)?;
    let second_core = ViewCore::start(second_backend, second_config)?;
    let histogram_reads = HistogramReads::default();
    let setup = journal.clone().map(|journal| WriteSetup {
        journal,
        checks: std::sync::Arc::new(image_check(&core, &second_core, &histogram_reads, true)),
        options: settings.edit_options(),
        write_processes: WRITE_PROCESSES,
    });
    let metadata_worker = MetadataWorker::start(setup).map_err(|e| e.to_string())?;
    let import_worker = ImportWorker::start_with_processes(
        journal,
        std::sync::Arc::new(image_check(&core, &second_core, &histogram_reads, false)),
        WRITE_PROCESSES,
    )
    .map_err(|e| e.to_string())?;
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
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
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
            // The fonts (Hack last for the arrows, T155), both themes and the text sizes before
            // the first frame (ADR 0017 §2.5, T109).
            cc.egui_ctx.set_fonts(font_definitions());
            cc.egui_ctx
                .set_visuals_of(egui::Theme::Dark, visuals(Theme::Dark));
            cc.egui_ctx
                .set_visuals_of(egui::Theme::Light, visuals(Theme::Light));
            cc.egui_ctx.all_styles_mut(apply_text_sizes);
            cc.egui_ctx.set_theme(egui_theme(settings.shell.theme));
            let ctx = cc.egui_ctx.clone();
            core.set_wake({
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            });
            second_core.set_wake({
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            });
            let folder_watcher = FolderWatcher::start(Box::new(move || ctx.request_repaint()));
            let mut loupe = Loupe {
                core,
                second_core,
                second_textures: Textures::new(),
                second_clip_overlay: OverlayCache::default(),
                second_focus_overlay: FocusCache::default(),
                compare: None,
                metadata_worker,
                exiftool_checked: false,
                settings: settings.clone(),
                settings_path,
                settings_window: None,
                settings_page: SettingsPage::General,
                about_window: false,
                pending_options: None,
                metadata: MetadataCache::default(),
                import_worker,
                import_dialog: None,
                tasks: Tasks::new(),
                last_form: ImportForm {
                    incremental: settings.shell.import_incremental,
                    ..ImportForm::default()
                },
                import_clear_confirm: false,
                detected_cards: Vec::new(),
                card_refresh: None,
                import_submit_error: None,
                import_hidden: false,
                thumb_worker,
                thumb_grid: ThumbGrid::new(0, 0),
                thumb_files: Vec::new(),
                added_files: AddedFiles::default(),
                thumb_generation: 0,
                thumb_textures: HashMap::new(),
                thumb_size: DEFAULT_THUMBNAIL_SIZE,
                grid_columns: 1,
                wheel_notches: 0.0,
                textures: Textures::new(),
                anchor_at_pointer: true,
                exif_reader: ExifReader::start(),
                exif: ExifInspector::default(),
                histogram_reader: HistogramReader::start(backend, histogram_reads),
                histograms: HistogramInspector::new(
                    inspector::INSPECTOR_READ_DELAY,
                    inspector::INSPECTOR_POLL_INTERVAL,
                ),
                capture_reader: CaptureReader::start(),
                capture_in_flight: HashSet::new(),
                orientation_reader: EmbeddedOrientationReader::start(),
                orientation_reads: 0,
                status_bar: StatusBar::default(),
                next_status: StatusBar::default(),
                scroll_request: None,
                grid_note: None,
                grid_anchor: None,
                library_took_f5: false,
                thumb_button_clicked_at: None,
                metadata_window: None,
                metadata_window_shown: false,
                image_area: None,
                copied_fields: None,
                field_values,
                field_values_path,
                message: None,
                moves: Moves::default(),
                delete: DeleteStep::Idle,
                delete_folder: None,
                delete_rejected: false,
                delete_partners: Vec::new(),
                delete_ids: HashSet::new(),
                delete_focus_cancel: false,
                delete_button: None,
                open: OpenStep::Idle,
                open_focus_cancel: false,
                open_button: None,
                indicator_after,
                tabs: Tabs::default(),
                ratings_read: false,
                last_ratings_ready: false,
                last_frame_status: Status::Empty,
                last_zoom: None,
                last_statistics: None,
                last_clipping: ClippingState::NotWanted,
                clip_overlay: OverlayCache::default(),
                focus_overlay: FocusCache::default(),
                display_mode: Source::Raw,
                window_before: None,
                window_settling: None,
                viewport_commands: Vec::new(),
                filmstrip_offset: 0.0,
                filmstrip_followed: None,
                monitor_profile,
                library,
                folder_watcher,
                watched_folders: Vec::new(),
                ejecting_drive: None,
                import_destination: None,
                import_tabs: None,
            };
            if let Some(p) = path {
                loupe.open(&p, false);
            }
            Ok(Box::new(loupe))
        }),
    )
    .map_err(|e| format!("{e}"))
}

/// RawStride's icon for the title bar, the taskbar and Alt+Tab (ADR 0031 RL-Q11, T226), or
/// `None` when the PNG cannot be decoded; eframe then keeps its own icon.
fn window_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/rawstride-128.png")).ok()
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
    use super::{ApplyPlan, apply_allowed, marked_subjects, metadata_subject};
    use super::{
        FALLBACK_CHOICES, LABEL_TEXT, fallback_text, index_span, label_color, label_pill,
        other_values, resolve_path, resolve_paths, window_editor_properties,
    };
    use super::{LeaveDecision, SaveAnswer, WriteOutcome, leave_decision};
    use super::{WindowLeave, WindowRefilter, metadata_window_position, window_refilter};
    use egui::{Color32, Rect, pos2, vec2};
    use pw_loupe::{EditorField, EditorForm, Tab, ViewMode};
    use pw_loupe::{EditorItem, EditorLayout, EditorSection, Language, property_label};
    use pw_loupe::{TEXT_CONTRAST, contrast_ratio};
    use pw_metadata::iim::Fallback;
    use std::path::{Path, PathBuf};

    // --- A metadata write waits for the inspector's histogram read (U7c-Q4, T144) ---

    mod write_waits_for_the_histogram_read {
        use super::super::{release_before_write, release_in_both};
        use pw_loupe::{HistogramReader, HistogramReads, HistogramSubmit, file_stamp};
        use pw_metadata::safe_write::ImageCheck;
        use pw_viewcore::{
            Backend, DecodeError, ImageSize, Rect, Rgb, Source, Statistics, StatisticsOrigin,
        };
        use std::path::{Path, PathBuf};
        use std::sync::{Arc, Condvar, Mutex, mpsc};
        use std::time::{Duration, Instant};

        type Log = Arc<Mutex<Vec<&'static str>>>;

        fn note(log: &Log, step: &'static str) {
            log.lock().unwrap_or_else(|e| e.into_inner()).push(step);
        }

        fn steps(log: &Log) -> Vec<&'static str> {
            log.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }

        /// A test reader's backend that records the order of its steps; its statistics wait at a
        /// gate until the test opens it.
        #[derive(Clone)]
        struct Recording {
            log: Log,
            gate: Arc<(Mutex<(bool, bool)>, Condvar)>,
        }

        impl Recording {
            fn wait_until_reading(&self) {
                let (state, changed) = &*self.gate;
                let end = Instant::now() + Duration::from_secs(10);
                let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                while !state.0 {
                    let now = Instant::now();
                    assert!(now < end, "the read did not start");
                    state = match changed.wait_timeout(state, end - now) {
                        Ok((state, _)) => state,
                        Err(e) => e.into_inner().0,
                    };
                }
            }

            fn open_gate(&self) {
                let (state, changed) = &*self.gate;
                state.lock().unwrap_or_else(|e| e.into_inner()).1 = true;
                changed.notify_all();
            }
        }

        /// The session notes when it is dropped, so the test sees the file let go.
        struct Session(Log);

        impl Drop for Session {
            fn drop(&mut self) {
                note(&self.0, "session dropped");
            }
        }

        impl Backend for Recording {
            type Session = Session;
            fn open(&self, _: &Path, _: Source) -> Result<(Session, ImageSize), DecodeError> {
                note(&self.log, "opened");
                Ok((
                    Session(self.log.clone()),
                    ImageSize {
                        w: 2,
                        h: 2,
                        flip: 0,
                    },
                ))
            }
            fn render_half(&self, _: &mut Session) -> Result<Rgb, DecodeError> {
                Err(DecodeError::new("nothing is rendered"))
            }
            fn render_region(&self, _: &mut Session, _: Rect) -> Result<Rgb, DecodeError> {
                Err(DecodeError::new("nothing is rendered"))
            }
            fn statistics(&self, _: &mut Session) -> Option<Statistics> {
                let (state, changed) = &*self.gate;
                let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                state.0 = true;
                changed.notify_all();
                while !state.1 {
                    state = match changed.wait(state) {
                        Ok(state) => state,
                        Err(e) => e.into_inner(),
                    };
                }
                drop(state);
                note(&self.log, "statistics");
                Some(Statistics::of_rgb8(StatisticsOrigin::Tiff, &[0; 12]))
            }
        }

        #[test]
        fn a_write_of_the_file_being_read_waits_for_the_read() {
            let log: Log = Arc::default();
            let backend = Recording {
                log: log.clone(),
                gate: Arc::default(),
            };
            let reads = HistogramReads::default();
            let mut reader = HistogramReader::start(backend.clone(), reads.clone());
            let path = std::env::temp_dir().join("pw-loupe-write-waits.DNG");
            assert!(reader.read_histogram(path.clone(), Source::Raw).is_ok());
            backend.wait_until_reading();

            let core_log = log.clone();
            let check = release_before_write(
                move |_: &Path| note(&core_log, "view core released"),
                reads,
                true,
            );
            let (done, finished) = mpsc::channel();
            let write_log = log.clone();
            let write_path: PathBuf = path.clone();
            let writer = std::thread::spawn(move || {
                check.release(&write_path);
                note(&write_log, "written");
                let _ = done.send(());
            });
            // the write waits while the read holds the file
            assert_eq!(
                finished.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            backend.open_gate();
            assert_eq!(finished.recv_timeout(Duration::from_secs(10)), Ok(()));
            assert!(writer.join().is_ok());
            assert_eq!(
                steps(&log),
                [
                    "opened",
                    "view core released",
                    "statistics",
                    "session dropped",
                    "written"
                ]
            );
            // the read's result still comes back
            let end = Instant::now() + Duration::from_secs(10);
            let read = loop {
                if let Ok(read) = reader.try_recv() {
                    break read;
                }
                assert!(Instant::now() < end, "no result");
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(read.path, path);
            assert!(matches!(
                read.statistics,
                pw_viewcore::ImageStatistics::Available(_)
            ));

            // a write of another file does not wait
            let other = release_before_write(|_: &Path| {}, HistogramReads::default(), true);
            let t0 = Instant::now();
            other.release(Path::new("elsewhere.DNG"));
            assert!(t0.elapsed() < Duration::from_secs(1));
        }

        /// The release before a move waits for the histogram read too (review of T177): a move
        /// must not meet a file the reader has open.
        #[test]
        fn a_move_of_the_file_being_read_waits_for_the_read() {
            let log: Log = Arc::default();
            let backend = Recording {
                log: log.clone(),
                gate: Arc::default(),
            };
            let reads = HistogramReads::default();
            let mut reader = HistogramReader::start(backend.clone(), reads.clone());
            let path = std::env::temp_dir().join("pw-loupe-move-waits.DNG");
            assert!(reader.read_histogram(path.clone(), Source::Raw).is_ok());
            backend.wait_until_reading();

            let core_log = log.clone();
            let check = release_before_write(
                move |_: &Path| note(&core_log, "view core released"),
                reads.clone(),
                true,
            );
            let (done, finished) = mpsc::channel();
            let move_log = log.clone();
            let move_path: PathBuf = path.clone();
            let mover = std::thread::spawn(move || {
                check.release_for_move(&move_path);
                note(&move_log, "moved");
                let _ = done.send(());
            });
            // the move waits while the read holds the file
            assert_eq!(
                finished.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            backend.open_gate();
            assert_eq!(finished.recv_timeout(Duration::from_secs(10)), Ok(()));
            assert!(mover.join().is_ok());
            assert_eq!(
                steps(&log),
                [
                    "opened",
                    "view core released",
                    "statistics",
                    "session dropped",
                    "moved"
                ]
            );
            // and notes no stamp
            assert_eq!(reads.take_before_write(&path), None);
        }

        #[test]
        fn a_write_releases_the_file_in_both_view_cores_before_it_waits_for_the_read() {
            let log: Log = Arc::default();
            let backend = Recording {
                log: log.clone(),
                gate: Arc::default(),
            };
            let reads = HistogramReads::default();
            let reader_reads = reads.clone();
            let mut reader = HistogramReader::start(backend.clone(), reader_reads);
            let path = std::env::temp_dir().join("pw-loupe-write-releases-both.DNG");
            assert!(reader.read_histogram(path.clone(), Source::Raw).is_ok());
            backend.wait_until_reading();

            let second_log = log.clone();
            let first_log = log.clone();
            let check = release_before_write(
                release_in_both(
                    move |_: &Path| note(&second_log, "second view core released"),
                    move |_: &Path| note(&first_log, "view core released"),
                ),
                reads,
                true,
            );
            let (done, finished) = mpsc::channel();
            let write_log = log.clone();
            let write_path: PathBuf = path.clone();
            let writer = std::thread::spawn(move || {
                check.release(&write_path);
                note(&write_log, "written");
                let _ = done.send(());
            });
            // both cores have let go, the second first, while the write still waits for the read
            assert_eq!(
                finished.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            assert_eq!(
                steps(&log),
                ["opened", "second view core released", "view core released"]
            );
            backend.open_gate();
            assert_eq!(finished.recv_timeout(Duration::from_secs(10)), Ok(()));
            assert!(writer.join().is_ok());
            assert_eq!(
                steps(&log),
                [
                    "opened",
                    "second view core released",
                    "view core released",
                    "statistics",
                    "session dropped",
                    "written"
                ]
            );
        }

        #[test]
        fn the_release_notes_the_stamp_before_the_write_when_asked_to() {
            let folder =
                std::env::temp_dir().join(format!("pw-loupe-release-notes-{}", std::process::id()));
            assert!(std::fs::create_dir_all(&folder).is_ok());
            let path = folder.join("noted.DNG");
            assert!(std::fs::write(&path, b"before").is_ok());
            let before = file_stamp(&path);
            assert!(before.is_some());

            let log: Log = Arc::default();
            let core_log = log.clone();
            let reads = HistogramReads::default();
            let check = release_before_write(
                move |_: &Path| note(&core_log, "view core released"),
                reads.clone(),
                true,
            );
            check.release(&path);
            // noted when the release returns, after the view core let go
            assert_eq!(steps(&log), ["view core released"]);
            // the write changes the file; the noted stamp is the one before it
            assert!(std::fs::write(&path, b"after the metadata write").is_ok());
            assert_eq!(reads.take_before_write(&path), Some(before));
            assert_eq!(reads.take_before_write(&path), None);

            // the import's check notes nothing
            let import = release_before_write(|_: &Path| {}, reads.clone(), false);
            import.release(&path);
            assert_eq!(reads.take_before_write(&path), None);
            let _ = std::fs::remove_dir_all(&folder);
        }

        #[test]
        fn the_release_before_a_move_notes_no_stamp() {
            let folder =
                std::env::temp_dir().join(format!("pw-loupe-move-release-{}", std::process::id()));
            assert!(std::fs::create_dir_all(&folder).is_ok());
            let path = folder.join("moved.DNG");
            assert!(std::fs::write(&path, b"before").is_ok());
            let before = file_stamp(&path);
            assert!(before.is_some());

            let log: Log = Arc::default();
            let core_log = log.clone();
            let reads = HistogramReads::default();
            let check = release_before_write(
                move |_: &Path| note(&core_log, "view core released"),
                reads.clone(),
                true,
            );
            check.release_for_move(&path);
            // the view core let go, but no stamp is noted: no write answer would take it
            assert_eq!(steps(&log), ["view core released"]);
            assert_eq!(reads.take_before_write(&path), None);

            // a write's release afterwards still notes one
            check.release(&path);
            assert_eq!(steps(&log), ["view core released", "view core released"]);
            assert_eq!(reads.take_before_write(&path), Some(before));
            let _ = std::fs::remove_dir_all(&folder);
        }
    }

    // --- Where the metadata window opens (T131) ---

    /// The program window the placement tests use: 1600 × 1000 at the origin.
    fn placement_screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0))
    }

    #[test]
    fn metadata_window_opens_centred_on_a_larger_image_area() {
        let screen = placement_screen();
        let image_area = Rect::from_min_max(pos2(0.0, 60.0), pos2(1600.0, 970.0));
        let size = vec2(460.0, 640.0);
        let position = metadata_window_position(image_area, size, screen);
        let window = Rect::from_min_size(position, size);
        assert_eq!(window.center(), image_area.center());
        assert_eq!(position, pos2(570.0, 195.0));
    }

    #[test]
    fn metadata_window_opens_centred_on_an_image_area_beside_a_left_column() {
        let screen = placement_screen();
        // A left column 300 wide: the image area's centre is not the screen's.
        let image_area = Rect::from_min_max(pos2(300.0, 60.0), pos2(1600.0, 970.0));
        let size = vec2(460.0, 640.0);
        let position = metadata_window_position(image_area, size, screen);
        let window = Rect::from_min_size(position, size);
        assert_eq!(window.center(), image_area.center());
        assert_ne!(window.center().x, screen.center().x);
        assert_eq!(position, pos2(720.0, 195.0));
    }

    #[test]
    fn metadata_window_taller_than_the_image_area_keeps_its_top_inside_the_program_window() {
        // An image area 400 high in a program window 500 high: the window (640 high) is taller
        // than both, so centring alone would put its title bar above the program window.
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 500.0));
        let image_area = Rect::from_min_max(pos2(300.0, 60.0), pos2(1600.0, 460.0));
        let size = vec2(460.0, 640.0);
        let position = metadata_window_position(image_area, size, screen);
        assert!(
            position.y >= screen.top(),
            "top {} above the screen",
            position.y
        );
        assert_eq!(position.y, 0.0);
        // Still centred across.
        assert_eq!(position.x + size.x / 2.0, image_area.center().x);
    }

    #[test]
    fn metadata_window_lower_than_the_window_stays_centred_on_the_image_area() {
        // An image area lower than the window but a program window tall enough: the window is
        // centred on the area and stays inside the program window.
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 800.0));
        let image_area = Rect::from_min_max(pos2(0.0, 300.0), pos2(1600.0, 700.0));
        let size = vec2(460.0, 640.0);
        let position = metadata_window_position(image_area, size, screen);
        let window = Rect::from_min_size(position, size);
        assert!(window.top() >= screen.top());
        assert!(screen.contains_rect(window));
        // Centring alone would end at 820, below the program window: moved up to fit.
        assert_eq!(position.y, 160.0);
    }

    // --- `M` and "Apply to All Marked" with several images marked (ADR 0017 U12c, T126) ---

    /// A tab of six files shown in reverse order (5, 4, … 0), `current` the current one and
    /// `marked` Ctrl+clicked in that order.
    fn tab_with(mode: ViewMode, current: usize, marked: &[usize]) -> Tab {
        let mut tab = Tab {
            files: (0..6)
                .map(|n| PathBuf::from(format!("img{n}.jpg")))
                .collect(),
            current,
            mode,
            visible: (0..6).rev().collect(),
            ..Tab::default()
        };
        for &index in marked {
            tab.selection.ctrl_click(index, index);
        }
        tab
    }

    #[test]
    fn with_several_marked_m_opens_on_the_current_image_if_marked_else_the_first_marked_shown() {
        for mode in [ViewMode::Grid, ViewMode::Loupe] {
            // The current image is marked: it is the window's image.
            let tab = tab_with(mode, 3, &[1, 3, 4]);
            assert_eq!(tab.selection.count(), 3);
            assert_eq!(
                metadata_subject(Some(&tab), None),
                Some(PathBuf::from("img3.jpg")),
                "{mode:?}"
            );
            // It is not: the first marked image in the shown order (4, not 1).
            let tab = tab_with(mode, 0, &[1, 3, 4]);
            assert_eq!(
                metadata_subject(Some(&tab), None),
                Some(PathBuf::from("img4.jpg")),
                "{mode:?}"
            );
            // The marked images in the shown order.
            assert_eq!(
                marked_subjects(Some(&tab), None),
                [
                    PathBuf::from("img4.jpg"),
                    PathBuf::from("img3.jpg"),
                    PathBuf::from("img1.jpg")
                ],
                "{mode:?}"
            );
        }
    }

    #[test]
    fn with_one_marked_or_none_m_opens_on_the_current_image_as_before() {
        for mode in [ViewMode::Grid, ViewMode::Loupe] {
            let one = tab_with(mode, 2, &[4]);
            assert_eq!(one.selection.count(), 1);
            assert_eq!(
                metadata_subject(Some(&one), None),
                Some(PathBuf::from("img2.jpg")),
                "{mode:?}"
            );
            assert!(marked_subjects(Some(&one), None).is_empty());
            let none = tab_with(mode, 5, &[]);
            assert_eq!(
                metadata_subject(Some(&none), None),
                Some(PathBuf::from("img5.jpg")),
                "{mode:?}"
            );
            assert!(marked_subjects(Some(&none), None).is_empty());
            // A current image that is not shown opens nothing.
            let mut hidden = tab_with(mode, 5, &[]);
            hidden.visible.retain(|&index| index != 5);
            assert_eq!(metadata_subject(Some(&hidden), None), None, "{mode:?}");
        }
        assert_eq!(metadata_subject(None, None), None);
        assert!(marked_subjects(None, None).is_empty());
    }

    #[test]
    fn the_apply_plan_writes_the_other_supported_marked_images_and_counts_the_skipped() {
        let read = pw_metadata::read::MetadataRead {
            reconciled: pw_metadata::reconcile::Reconciled {
                metadata: pw_metadata::value::Metadata::new(),
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: pw_metadata::write_plan::FileState {
                kind: pw_metadata::reconcile::FileKind::Jpeg,
                iim_present: false,
                iim_charset: pw_metadata::iim::CodedCharset::Absent,
                iim_values: Vec::new(),
                digest: pw_metadata::reconcile::IimDigest::Missing,
                exif_present: pw_metadata::write_plan::ExifPresent::default(),
                xmp_present: pw_metadata::write_plan::XmpPresent::default(),
            },
        };
        let mut form = EditorForm::from_read(&read);
        match form.text_mut(EditorField::Headline) {
            Some(text) => *text = "Hav".to_owned(),
            None => panic!("the form has a headline field"),
        }
        let marked = [
            PathBuf::from("a.jpg"),
            PathBuf::from("b.png"),
            PathBuf::from("c.ARW"),
            PathBuf::from("d.DNG"),
        ];

        let plan = match ApplyPlan::new(&form, Path::new("a.jpg"), &marked) {
            Ok(plan) => plan,
            Err(errors) => panic!("no field error expected: {errors:?}"),
        };

        let others: Vec<&PathBuf> = plan.others.iter().map(|(path, _)| path).collect();
        assert_eq!(others, [&marked[2], &marked[3]]);
        assert!(
            plan.others
                .iter()
                .all(|(_, edits)| *edits == plan.this_image)
        );
        assert_eq!(
            plan.lines,
            [
                "Write 1 changed field to 3 images?",
                "Changed: Headline",
                "Skipped: 1 file",
            ]
        );

        // A field error gives no plan.
        match form.text_mut(EditorField::DateCreated) {
            Some(text) => *text = "igår".to_owned(),
            None => panic!("the form has a date field"),
        }
        assert!(ApplyPlan::new(&form, Path::new("a.jpg"), &marked).is_err());
    }

    /// "Apply to {n} Marked" only for a marked window image with something to write to the other
    /// images (review of T126).
    #[test]
    fn apply_needs_a_marked_window_image_and_something_for_the_other_images() {
        let mut metadata = pw_metadata::value::Metadata::new();
        for (id, value) in [("photoshop:Headline", "Hav"), ("dc:rights", "Copyright")] {
            let value = if id == "dc:rights" {
                pw_metadata::value::Value::LangAlt(pw_metadata::value::LangAlt::from_entries(vec![
                    ("x-default".to_owned(), value.to_owned()),
                ]))
            } else {
                pw_metadata::value::Value::Text(value.to_owned())
            };
            if let Err(error) =
                metadata.insert_read(id, value, pw_metadata::value::Origin::XmpEmbedded)
            {
                panic!("{id} inserted: {error:?}");
            }
        }
        let read = pw_metadata::read::MetadataRead {
            reconciled: pw_metadata::reconcile::Reconciled {
                metadata,
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: pw_metadata::write_plan::FileState {
                kind: pw_metadata::reconcile::FileKind::Jpeg,
                iim_present: false,
                iim_charset: pw_metadata::iim::CodedCharset::Absent,
                iim_values: Vec::new(),
                digest: pw_metadata::reconcile::IimDigest::Missing,
                exif_present: pw_metadata::write_plan::ExifPresent::default(),
                xmp_present: pw_metadata::write_plan::XmpPresent::default(),
            },
        };
        let marked = [PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let set_text =
            |form: &mut EditorForm, field: EditorField, text: &str| match form.text_mut(field) {
                Some(target) => *target = text.to_owned(),
                None => panic!("{field:?} is editable"),
            };

        // Nothing changed: nothing to apply.
        let untouched = EditorForm::from_read(&read);
        assert!(!apply_allowed(&untouched, Path::new("a.jpg"), &marked));

        // A changed field on a marked window image: allowed.
        let mut changed = EditorForm::from_read(&read);
        set_text(&mut changed, EditorField::Headline, "Kväll");
        assert!(apply_allowed(&changed, Path::new("a.jpg"), &marked));
        // The window moved to an image that is not marked: not allowed.
        assert!(!apply_allowed(&changed, Path::new("c.jpg"), &marked));
        // One image marked: not allowed.
        assert!(!apply_allowed(&changed, Path::new("a.jpg"), &marked[..1]));

        // Only an emptied copyright field, which stays on the window's image: nothing to write to
        // the other images, so not allowed (no write without edits to them).
        let mut copyright_only = EditorForm::from_read(&read);
        set_text(&mut copyright_only, EditorField::Copyright, "");
        assert!(copyright_only.edits().is_ok_and(|edits| !edits.is_empty()));
        assert!(!apply_allowed(&copyright_only, Path::new("a.jpg"), &marked));

        // A field error: not allowed.
        let mut bad_date = EditorForm::from_read(&read);
        set_text(&mut bad_date, EditorField::Headline, "Kväll");
        set_text(&mut bad_date, EditorField::DateCreated, "igår");
        assert!(!apply_allowed(&bad_date, Path::new("a.jpg"), &marked));
    }

    // --- no user-visible string literal in the drawing code (T116, rule 13) ---

    /// The calls whose text argument must come from a text module, not a string literal.
    const TEXT_CALLS: [&str; 9] = [
        "label(",
        "button(",
        "Button::new(",
        "checkbox(",
        "on_hover_text(",
        "hint_text(",
        "selectable_label(",
        "selectable_value(",
        "pick_folder(",
    ];

    /// The lines of `source` (before its `#[cfg(test)]` module, comment lines left out) where a
    /// string literal is passed straight to one of [`TEXT_CALLS`]: as one of the call's own
    /// arguments, also on a later line of the call. A literal inside a nested call (for example
    /// `format!`) is not passed straight. Each finding names its line.
    fn literal_text_calls(source: &str) -> Vec<String> {
        let code = source
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or(source);
        // The non-comment lines as one text, and the 1-based line number of each byte in it.
        let mut text = String::new();
        let mut line_of = Vec::new();
        for (index, line) in code.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            text.push_str(line);
            text.push('\n');
            line_of.extend(std::iter::repeat_n(index + 1, line.len() + 1));
        }
        let bytes = text.as_bytes();
        let mut found = Vec::new();
        for call in TEXT_CALLS {
            for (start, _) in text.match_indices(call) {
                let mut at = start + call.len();
                let mut depth = 1;
                while at < bytes.len() && depth > 0 {
                    match bytes[at] {
                        b'"' if depth == 1 => {
                            found.push(format!(
                                "main.rs:{}: a string literal is passed to `{call}`",
                                line_of[start]
                            ));
                            break;
                        }
                        b'"' => {
                            // Skip a nested string, with its escapes.
                            at += 1;
                            while at < bytes.len() && bytes[at] != b'"' {
                                if bytes[at] == b'\\' {
                                    at += 1;
                                }
                                at += 1;
                            }
                        }
                        b'\'' if bytes.get(at + 2) == Some(&b'\'') => at += 2,
                        b'(' | b'[' | b'{' => depth += 1,
                        b')' | b']' | b'}' => depth -= 1,
                        _ => {}
                    }
                    at += 1;
                }
            }
        }
        found
    }

    #[test]
    fn the_check_finds_a_literal_passed_to_a_text_call() {
        let source = "fn draw(ui: &mut egui::Ui) {\n\
                      ui.label(\"Källa\");\n\
                      ui.checkbox(\n    &mut on,\n    \"Backup\",\n);\n\
                      // ui.button(\"Stäng\");\n\
                      ui.label(text);\n\
                      ui.label(format!(\"{n}\"));\n\
                      }\n\
                      #[cfg(test)]\nmod tests {\n    ui.label(\"test\");\n}\n";
        assert_eq!(
            literal_text_calls(source),
            [
                "main.rs:2: a string literal is passed to `label(`",
                "main.rs:3: a string literal is passed to `checkbox(`",
            ]
        );
    }

    #[test]
    fn the_drawing_code_passes_no_string_literal_to_a_text_call() {
        let found = literal_text_calls(include_str!("main.rs"));
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    // --- the metadata window's move and close (ADR 0017 §2.3.1, U12-Q4, U12-Q5, T112) ---

    #[test]
    fn without_edits_a_move_or_close_is_done_at_once_whatever_the_answer_or_outcome() {
        let outcomes = [
            None,
            Some(WriteOutcome::Success),
            Some(WriteOutcome::Notices),
            Some(WriteOutcome::Failure),
            Some(WriteOutcome::FieldErrors),
        ];
        for answer in [SaveAnswer::Save, SaveAnswer::Discard, SaveAnswer::Cancel] {
            for outcome in outcomes {
                assert_eq!(
                    leave_decision(false, answer, outcome),
                    LeaveDecision::Leave,
                    "{answer:?} {outcome:?}"
                );
            }
        }
    }

    #[test]
    fn with_edits_spara_writes_first_and_its_outcome_decides() {
        assert_eq!(
            leave_decision(true, SaveAnswer::Save, None),
            LeaveDecision::Write
        );
        assert_eq!(
            leave_decision(true, SaveAnswer::Save, Some(WriteOutcome::Success)),
            LeaveDecision::Leave,
            "success without notices: the move or the close is done"
        );
        assert_eq!(
            leave_decision(true, SaveAnswer::Save, Some(WriteOutcome::Failure)),
            LeaveDecision::StayWithEdits,
            "a failure keeps the window on the image with the edits"
        );
        assert_eq!(
            leave_decision(true, SaveAnswer::Save, Some(WriteOutcome::FieldErrors)),
            LeaveDecision::StayWithEdits,
            "field errors: nothing written, no move"
        );
        assert_eq!(
            leave_decision(true, SaveAnswer::Save, Some(WriteOutcome::Notices)),
            LeaveDecision::StayShowingNotices,
            "notices: the window stays and shows them"
        );
    }

    #[test]
    fn with_edits_slang_leaves_and_avbryt_stays_with_the_edits() {
        let outcomes = [
            None,
            Some(WriteOutcome::Success),
            Some(WriteOutcome::Notices),
            Some(WriteOutcome::Failure),
            Some(WriteOutcome::FieldErrors),
        ];
        for outcome in outcomes {
            assert_eq!(
                leave_decision(true, SaveAnswer::Discard, outcome),
                LeaveDecision::Leave,
                "{outcome:?}"
            );
            assert_eq!(
                leave_decision(true, SaveAnswer::Cancel, outcome),
                LeaveDecision::StayWithEdits,
                "{outcome:?}"
            );
        }
    }

    #[test]
    fn every_encoding_choice_has_a_text() {
        assert_eq!(FALLBACK_CHOICES, [Fallback::Windows1252, Fallback::Latin1]);
        assert_eq!(
            fallback_text(Fallback::Windows1252),
            "Windows-1252 (Western European)"
        );
        assert_eq!(fallback_text(Fallback::Latin1), "ISO 8859-1 (Latin-1)");
        for fallback in FALLBACK_CHOICES {
            assert!(!fallback_text(fallback).is_empty(), "{fallback:?}");
        }
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
    fn the_windows_other_metadata_leaves_out_every_field_and_the_contact_info() {
        let properties = window_editor_properties();

        // 47 fields, eight of them (the address, Email, Phone, Web URL) in the contact structure,
        // which is now left out as a whole (T123, point 7); and the rating and the label, shown in
        // the Rating and Label section (T124, point 6). The seven fields of the Licence (PLUS)
        // section (T128) are seven of them. Location Created and Location Shown, edited in Time
        // and Place (T136), and Genre, Image Creator, Copyright Owner, Licensor and Image
        // Supplier, edited in Assignment and Licence (PLUS) (T139), are left out too, and so are
        // Person Shown with Details, Product Shown, Artwork or Object, Contributor, CV-Term About
        // Image and Image Registry Entry (T143).
        assert_eq!(properties.len(), 55);
        for id in [
            "dc:title",
            "dc:description",
            "dc:subject",
            "photoshop:City",
            "Iptc4xmpCore:CreatorContactInfo",
            "Iptc4xmpCore:ExtDescrAccessibility",
            "photoshop:CaptionWriter",
            "Iptc4xmpExt:PersonInImage",
            "Iptc4xmpExt:OrganisationInImageName",
            "Iptc4xmpExt:OrganisationInImageCode",
            "Iptc4xmpExt:Event",
            "xmpRights:WebStatement",
            "xmpRights:Marked",
            "Iptc4xmpCore:SubjectCode",
            "Iptc4xmpCore:Scene",
            "photoshop:Urgency",
            "photoshop:Category",
            "photoshop:SupplementalCategories",
            "xmp:Rating",
            "xmp:Label",
            "Iptc4xmpExt:LocationCreated",
            "Iptc4xmpExt:LocationShown",
            "Iptc4xmpExt:Genre",
            "plus:ImageCreator",
            "plus:CopyrightOwner",
            "plus:Licensor",
            "plus:ImageSupplier",
            "Iptc4xmpExt:PersonInImageWDetails",
            "Iptc4xmpExt:ProductInImage",
            "Iptc4xmpExt:ArtworkOrObject",
            "Iptc4xmpExt:Contributor",
            "Iptc4xmpExt:AboutCvTerm",
            "Iptc4xmpExt:RegistryId",
        ] {
            assert!(properties.contains(&id), "{id}");
        }
        // Each property once.
        let mut unique = properties.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), properties.len());
        // The other structures (Image Region and the rights expressions, not in U11c-Q1) and
        // Intellectual Genre (U10-Q4) stay under Other Metadata.
        for id in [
            "Iptc4xmpCore:IntellectualGenre",
            "Iptc4xmpExt:ImageRegion",
            "Iptc4xmpExt:EmbdEncRightsExpr",
            "Iptc4xmpExt:LinkedEncRightsExpr",
        ] {
            assert!(!properties.contains(&id), "{id}");
        }
    }

    /// Other Metadata of a read with new fields, the deprecated fields, the contact structure and
    /// one property that is not a field: only that one is listed (T123, point 7).
    #[test]
    fn other_metadata_lists_no_new_field() {
        use pw_metadata::iim::CodedCharset;
        use pw_metadata::read::MetadataRead;
        use pw_metadata::reconcile::{FileKind, IimDigest, Reconciled};
        use pw_metadata::value::{Metadata, Origin, StructValue, Value};
        use pw_metadata::write_plan::{ExifPresent, FileState, XmpPresent};

        let text = |s: &str| Value::Text(s.to_string());
        let mut metadata = Metadata::new();
        for (id, value) in [
            ("photoshop:CaptionWriter", text("Anna")),
            ("Iptc4xmpExt:PersonInImage", Value::List(vec!["Per".into()])),
            ("Iptc4xmpCore:Scene", Value::List(vec!["011900".into()])),
            ("xmpRights:WebStatement", text("https://example.com")),
            ("xmpRights:Marked", text("True")),
            ("photoshop:Urgency", Value::Number("5".into())),
            ("photoshop:Category", text("SPO")),
            (
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(StructValue {
                    fields: vec![("Iptc4xmpCore:CiAdrCity".into(), text("Umeå"))],
                }),
            ),
            ("Iptc4xmpCore:IntellectualGenre", text("Nyhet")),
            // Shown by the Rating and Label section (T124, point 6).
            ("xmp:Rating", Value::Number("3".into())),
            ("xmp:Label", text("Red")),
        ] {
            metadata
                .insert_read(id, value, Origin::XmpEmbedded)
                .unwrap();
        }
        let read = MetadataRead {
            reconciled: Reconciled {
                metadata,
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: FileState {
                kind: FileKind::Jpeg,
                iim_present: false,
                iim_charset: CodedCharset::Absent,
                iim_values: Vec::new(),
                digest: IimDigest::Missing,
                exif_present: ExifPresent::default(),
                xmp_present: XmpPresent::default(),
            },
        };
        let rows = other_values(&read, &window_editor_properties());
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Intellectual Genre"]);
    }

    /// The Licence (PLUS) section's properties are fields, left out of Other Metadata (T128,
    /// point 7), and so are its PLUS structures, edited as lists (T139, point 5); Model Age stays
    /// there.
    #[test]
    fn other_metadata_leaves_out_the_licence_fields() {
        use pw_metadata::iim::CodedCharset;
        use pw_metadata::read::MetadataRead;
        use pw_metadata::reconcile::{FileKind, IimDigest, Reconciled};
        use pw_metadata::value::{Metadata, Origin, StructValue, Value};
        use pw_metadata::write_plan::{ExifPresent, FileState, XmpPresent};

        let properties = window_editor_properties();
        let licence = [
            "plus:ModelReleaseStatus",
            "plus:ModelReleaseID",
            "plus:MinorModelAgeDisclosure",
            "plus:PropertyReleaseStatus",
            "plus:PropertyReleaseID",
            "plus:DataMining",
            "Iptc4xmpExt:DigitalSourceType",
        ];
        for id in licence {
            assert!(properties.contains(&id), "{id}");
        }
        for id in [
            "plus:ImageCreator",
            "plus:CopyrightOwner",
            "plus:Licensor",
            "plus:ImageSupplier",
        ] {
            assert!(properties.contains(&id), "{id}");
        }
        assert!(!properties.contains(&"Iptc4xmpExt:ModelAge"));

        let text = |s: &str| Value::Text(s.to_string());
        let mut metadata = Metadata::new();
        for (id, value) in [
            ("plus:ModelReleaseStatus", text("MR-NAP")),
            ("plus:ModelReleaseID", Value::List(vec!["MR-1".into()])),
            ("plus:MinorModelAgeDisclosure", text("AG-A25")),
            ("plus:PropertyReleaseStatus", text("PR-NON")),
            ("plus:PropertyReleaseID", Value::List(vec!["PR-1".into()])),
            ("plus:DataMining", text("DMI-UNSPECIFIED")),
            (
                "Iptc4xmpExt:DigitalSourceType",
                text("http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture"),
            ),
            ("Iptc4xmpExt:ModelAge", Value::List(vec!["25".into()])),
            (
                "plus:Licensor",
                Value::StructList(vec![StructValue {
                    fields: vec![("plus:LicensorName".into(), text("Bildbyrån"))],
                }]),
            ),
        ] {
            metadata
                .insert_read(id, value, Origin::XmpEmbedded)
                .unwrap();
        }
        let read = MetadataRead {
            reconciled: Reconciled {
                metadata,
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: FileState {
                kind: FileKind::Jpeg,
                iim_present: false,
                iim_charset: CodedCharset::Absent,
                iim_values: Vec::new(),
                digest: IimDigest::Missing,
                exif_present: ExifPresent::default(),
                xmp_present: XmpPresent::default(),
            },
        };
        let rows = other_values(&read, &properties);
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Model Age"]);
    }

    /// A JPEG's read holding `values`, read from embedded XMP (T278's tests).
    fn read_with(
        values: Vec<(&str, pw_metadata::value::Value)>,
    ) -> pw_metadata::read::MetadataRead {
        use pw_metadata::iim::CodedCharset;
        use pw_metadata::read::MetadataRead;
        use pw_metadata::reconcile::{FileKind, IimDigest, Reconciled};
        use pw_metadata::value::{Metadata, Origin};
        use pw_metadata::write_plan::{ExifPresent, FileState, XmpPresent};

        let mut metadata = Metadata::new();
        for (id, value) in values {
            metadata
                .insert_read(id, value, Origin::XmpEmbedded)
                .unwrap();
        }
        MetadataRead {
            reconciled: Reconciled {
                metadata,
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: FileState {
                kind: FileKind::Jpeg,
                iim_present: false,
                iim_charset: CodedCharset::Absent,
                iim_values: Vec::new(),
                digest: IimDigest::Missing,
                exif_present: ExifPresent::default(),
                xmp_present: XmpPresent::default(),
            },
        }
    }

    /// The labels of the form's Other Metadata rows for `read` with `layout` (U10b-Q3, T278).
    fn other_labels(read: &pw_metadata::read::MetadataRead, layout: &EditorLayout) -> Vec<String> {
        other_values(read, &layout.shown_properties())
            .into_iter()
            .map(|row| row.label)
            .collect()
    }

    /// A hidden field's value is shown read-only under Other Metadata; with the default layout it
    /// is not (U10b-Q3, T278).
    #[test]
    fn other_metadata_shows_a_hidden_fields_value() {
        use pw_metadata::value::{LangAlt, Value};

        let read = read_with(vec![(
            "dc:title",
            Value::LangAlt(LangAlt::from_entries(vec![(
                "x-default".to_owned(),
                "Hav".to_owned(),
            )])),
        )]);
        let title = property_label("dc:title", Language::En);
        assert!(other_labels(&read, &EditorLayout::default()).is_empty());
        let mut layout = EditorLayout::default();
        layout.set_item_shown(EditorItem::Field(EditorField::Title), false);
        assert_eq!(other_labels(&read, &layout), [title]);
    }

    /// The field of a hidden section (Instructions in a hidden Assignment) is shown read-only
    /// under Other Metadata; with the default layout it is not (U10b-Q3, T278).
    #[test]
    fn other_metadata_shows_a_hidden_sections_field() {
        use pw_metadata::value::Value;

        let read = read_with(vec![(
            "photoshop:Instructions",
            Value::Text("Embargo".to_owned()),
        )]);
        let instructions = property_label("photoshop:Instructions", Language::En);
        assert!(other_labels(&read, &EditorLayout::default()).is_empty());
        let mut layout = EditorLayout::default();
        layout.set_section_shown(EditorSection::Assignment, false);
        assert!(
            layout
                .items(EditorSection::Assignment)
                .iter()
                .all(|(_, shown)| *shown)
        );
        assert_eq!(other_labels(&read, &layout), [instructions]);
    }

    /// With Rating and Label hidden, the rating read when the window opened is not an Other
    /// Metadata row (U10b-Q9, T278); the window's own line of the current rating is checked by
    /// hand.
    #[test]
    fn other_metadata_holds_no_rating_row_with_rating_and_label_hidden() {
        use pw_metadata::value::Value;

        let read = read_with(vec![
            ("xmp:Rating", Value::Number("3".into())),
            ("xmp:Label", Value::Text("Red".to_owned())),
        ]);
        let mut layout = EditorLayout::default();
        layout.set_section_shown(EditorSection::RatingLabel, false);
        assert!(
            !layout
                .shown_sections()
                .contains(&EditorSection::RatingLabel)
        );
        assert!(other_labels(&read, &layout).is_empty());
    }

    // --- the filter while the metadata window is open (ADR 0017 U10c-Q4, T124) ---

    #[test]
    fn a_rating_in_the_window_refilters_only_when_the_window_moves_or_closes() {
        let previous = Some(WindowLeave::Move { forward: false });
        let next = Some(WindowLeave::Move { forward: true });
        let close = Some(WindowLeave::Close);
        // Rated: a move keeps the neighbour it found in the list as it was; a close refilters
        // as after a key in the contact sheet.
        assert_eq!(
            window_refilter(true, next, Some(7)),
            WindowRefilter::KeepNeighbour(7)
        );
        assert_eq!(
            window_refilter(true, previous, Some(2)),
            WindowRefilter::KeepNeighbour(2)
        );
        assert_eq!(
            window_refilter(true, close, None),
            WindowRefilter::AsAfterKey
        );
        // While the window stays (no move or close, or a move that found nothing): nothing.
        assert_eq!(window_refilter(true, None, None), WindowRefilter::Nothing);
        assert_eq!(window_refilter(true, next, None), WindowRefilter::Nothing);
        // Nothing rated in the window: nothing, whatever the window does.
        for (leave, neighbour) in [
            (None, None),
            (next, Some(7)),
            (previous, Some(2)),
            (close, None),
        ] {
            assert_eq!(
                window_refilter(false, leave, neighbour),
                WindowRefilter::Nothing,
                "{leave:?}"
            );
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

    // --- The window's icon (ADR 0031 RL-Q11, T226) ---

    #[test]
    fn window_icon_is_the_128_px_png() {
        let icon = super::window_icon().unwrap();
        assert_eq!((icon.width, icon.height), (128, 128));
        assert_eq!(icon.rgba.len(), 128 * 128 * 4);
    }

    // --- "Reload Folder" and the tabs listed again after a move or an undo (ADR 0026 F1, T242) ---

    mod reload_folder {
        use super::super::{
            AnchorStep, GRID_SPACING, GridView, PlacedTabs, anchor_step, capture_requests,
            disk_change_text, disk_listing_held, grid_note, leave_out_waiting, list_into_tab,
            opened_during_import, placed_tabs, preview_must_leave, relist_tab, reload_enabled,
            reload_tab,
        };
        use pw_loupe::{
            ImportTabs, Relisted, SortKey, SortOrder, Tab, Tabs, ViewMode, anchored_offset,
            folder_changed_text, folder_gone_text, list_folder, removed_by_another_program_text,
        };
        use std::collections::{HashMap, HashSet};
        use std::path::{Path, PathBuf};
        use std::time::{Duration, SystemTime};

        const NAME: SortOrder = SortOrder {
            key: SortKey::Name,
            descending: false,
        };
        const MODIFIED: SortOrder = SortOrder {
            key: SortKey::Modified,
            descending: false,
        };
        const CAPTURE: SortOrder = SortOrder {
            key: SortKey::Capture,
            descending: false,
        };

        fn nothing_pending(_: &Path) -> bool {
            false
        }

        /// A new temporary folder `name` (under a folder of its own) holding `files`.
        fn folder(name: &str, files: &[&str]) -> PathBuf {
            let root =
                std::env::temp_dir().join(format!("pw-loupe-t242-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            for file in files {
                std::fs::write(dir.join(file), b"x").unwrap();
            }
            dir
        }

        fn remove_folder(dir: &Path) {
            if let Some(root) = dir.parent() {
                let _ = std::fs::remove_dir_all(root);
            }
        }

        fn listing(dir: &Path) -> Result<Vec<PathBuf>, String> {
            list_folder(dir).map(|(files, _)| files)
        }

        /// The folder opened in a tab with file `current` current and `order` applied, as
        /// opening it and `follow_sort` leave it.
        fn opened(
            dir: &Path,
            current: usize,
            order: SortOrder,
            modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
        ) -> Tab {
            let mut tabs = Tabs::default();
            tabs.open_in_active(dir.to_path_buf(), listing(dir).unwrap(), current);
            tabs.follow_sort(order, true, &|_| None, modified_of);
            let tab = tabs.active().cloned().unwrap();
            assert_eq!(tab.sorted, Some(order));
            tab
        }

        fn names(tab: &Tab) -> Vec<String> {
            tab.files
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect()
        }

        fn current_name(tab: &Tab) -> String {
            names(tab).get(tab.current).cloned().unwrap_or_default()
        }

        fn marked_names(tab: &Tab) -> Vec<String> {
            let all: Vec<usize> = (0..tab.files.len()).collect();
            let names = names(tab);
            tab.selection
                .marked_in(&all)
                .into_iter()
                .filter_map(|index| names.get(index).cloned())
                .collect()
        }

        /// Modification times by file name, in seconds.
        fn times(seconds: &[(&str, u64)]) -> impl Fn(&Path) -> Option<SystemTime> {
            let times: HashMap<String, SystemTime> = seconds
                .iter()
                .map(|(name, seconds)| {
                    (
                        (*name).to_owned(),
                        SystemTime::UNIX_EPOCH + Duration::from_secs(*seconds),
                    )
                })
                .collect();
            move |path: &Path| {
                path.file_name()
                    .and_then(|name| times.get(name.to_string_lossy().as_ref()).copied())
            }
        }

        #[test]
        fn a_reload_places_a_new_file_by_name_drops_a_removed_one_and_keeps_current_and_marks() {
            let dir = folder("added", &["a.jpg", "c.jpg", "e.jpg"]);
            let mut tab = opened(&dir, 2, NAME, &|_| None);
            tab.selection.all(&[0, 2]);
            std::fs::write(dir.join("b.jpg"), b"x").unwrap();
            std::fs::remove_file(dir.join("c.jpg")).unwrap();

            let text = reload_tab(
                &mut tab,
                listing(&dir),
                &nothing_pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            );

            assert_eq!(text, Ok(None));
            assert_eq!(names(&tab), ["a.jpg", "b.jpg", "e.jpg"]);
            assert_eq!(current_name(&tab), "e.jpg");
            assert_eq!(marked_names(&tab), ["a.jpg", "e.jpg"]);
            assert_eq!(tab.visible, [0, 1, 2]);
            remove_folder(&dir);
        }

        #[test]
        fn a_reload_whose_current_file_was_removed_makes_the_following_file_current_and_says_so() {
            let dir = folder("current-removed", &["a.jpg", "c.jpg", "e.jpg"]);
            let mut tab = opened(&dir, 1, NAME, &|_| None);
            std::fs::remove_file(dir.join("c.jpg")).unwrap();

            let text = reload_tab(
                &mut tab,
                listing(&dir),
                &nothing_pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            );

            assert_eq!(
                text,
                Ok(Some(removed_by_another_program_text(&dir.join("c.jpg"))))
            );
            assert_eq!(names(&tab), ["a.jpg", "e.jpg"]);
            assert_eq!(current_name(&tab), "e.jpg");
            remove_folder(&dir);
        }

        #[test]
        fn a_reload_of_a_deleted_folder_keeps_the_tab_and_says_the_folder_is_gone() {
            let dir = folder("gone", &["a.jpg", "b.jpg"]);
            let mut tab = opened(&dir, 1, NAME, &|_| None);
            tab.selection.all(&[0]);
            let before = tab.clone();
            remove_folder(&dir);

            let text = reload_tab(
                &mut tab,
                listing(&dir),
                &nothing_pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            );

            assert_eq!(text, Err(folder_gone_text(&dir)));
            assert_eq!(tab, before);
        }

        #[test]
        fn a_reload_keeps_a_file_being_written_that_the_listing_lacks() {
            let dir = folder("pending", &["a.jpg", "b.jpg", "c.jpg"]);
            let mut tab = opened(&dir, 1, NAME, &|_| None);
            // The safe write has renamed the original away for a moment.
            std::fs::remove_file(dir.join("b.jpg")).unwrap();
            let written = dir.join("b.jpg");
            let pending = |path: &Path| path == written;

            let text = reload_tab(
                &mut tab,
                listing(&dir),
                &pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            );

            assert_eq!(text, Ok(None));
            assert_eq!(names(&tab), ["a.jpg", "b.jpg", "c.jpg"]);
            assert_eq!(current_name(&tab), "b.jpg");
            remove_folder(&dir);
        }

        #[test]
        fn reload_is_enabled_for_a_folder_tab_with_or_without_images_and_not_while_busy() {
            let empty_folder = Tab {
                folder: Some(PathBuf::from("D:/x/empty")),
                ..Tab::default()
            };
            let with_images = Tab {
                folder: Some(PathBuf::from("D:/x/shoot")),
                files: vec![PathBuf::from("D:/x/shoot/a.jpg")],
                visible: vec![0],
                ..Tab::default()
            };
            let without_folder = Tab {
                files: vec![PathBuf::from("D:/x/shoot/a.jpg")],
                visible: vec![0],
                ..Tab::default()
            };
            assert!(!reload_enabled(None, false));
            assert!(!reload_enabled(Some(&without_folder), false));
            assert!(!reload_enabled(Some(&with_images), true));
            assert!(!reload_enabled(Some(&empty_folder), true));
            assert!(reload_enabled(Some(&with_images), false));
            assert!(reload_enabled(Some(&empty_folder), false));
        }

        #[test]
        fn the_relist_of_a_picked_tab_sorted_by_modified_places_the_new_file_and_moves_nothing_else()
         {
            let dir = folder("_picked", &["a.jpg", "b.jpg", "c.jpg"]);
            let before = times(&[("a.jpg", 30), ("b.jpg", 10), ("c.jpg", 20)]);
            let mut tab = opened(&dir, 0, MODIFIED, &before);
            assert_eq!(names(&tab), ["b.jpg", "c.jpg", "a.jpg"]);
            let current = current_name(&tab);
            // A move to `_picked` brings d.jpg; a.jpg's time changed since the order was applied,
            // and it still does not move.
            std::fs::write(dir.join("d.jpg"), b"x").unwrap();
            let after = times(&[("a.jpg", 5), ("b.jpg", 10), ("c.jpg", 20), ("d.jpg", 15)]);

            let relisted = relist_tab(
                &mut tab,
                listing(&dir).unwrap(),
                None,
                &nothing_pending,
                MODIFIED,
                false,
                &|_| None,
                &after,
            );

            assert_eq!(relisted.added, 1);
            assert_eq!(relisted.removed, 0);
            assert_eq!(names(&tab), ["b.jpg", "d.jpg", "c.jpg", "a.jpg"]);
            assert_eq!(current_name(&tab), current);
            assert_eq!(tab.sorted, Some(MODIFIED));
            assert!(tab.unplaced.is_empty());
            remove_folder(&dir);
        }

        #[test]
        fn the_relist_of_an_undo_sorted_by_modified_places_the_file_brought_back_and_makes_it_current()
         {
            let dir = folder("undo", &["a.jpg", "c.jpg"]);
            let modified = times(&[("a.jpg", 30), ("b.jpg", 10), ("c.jpg", 20)]);
            let mut tab = opened(&dir, 1, MODIFIED, &modified);
            assert_eq!(names(&tab), ["c.jpg", "a.jpg"]);
            tab.selection.all(&[0]);
            // The undo brings b.jpg back.
            std::fs::write(dir.join("b.jpg"), b"x").unwrap();
            let brought_back = dir.join("b.jpg");

            let relisted = relist_tab(
                &mut tab,
                listing(&dir).unwrap(),
                Some(&brought_back),
                &nothing_pending,
                MODIFIED,
                true,
                &|_| None,
                &modified,
            );

            assert_eq!(relisted.added, 1);
            assert_eq!(names(&tab), ["b.jpg", "c.jpg", "a.jpg"]);
            assert_eq!(current_name(&tab), "b.jpg");
            assert_eq!(marked_names(&tab), ["c.jpg"]);

            // A file brought back that the tab already lists (for example as a pending write):
            // nothing new, nothing gone, and it is still made current.
            let again = dir.join("c.jpg");
            let relisted = relist_tab(
                &mut tab,
                listing(&dir).unwrap(),
                Some(&again),
                &nothing_pending,
                MODIFIED,
                true,
                &|_| None,
                &modified,
            );
            assert!(!relisted.changed());
            assert_eq!(names(&tab), ["b.jpg", "c.jpg", "a.jpg"]);
            assert_eq!(current_name(&tab), "c.jpg");
            remove_folder(&dir);
        }

        #[test]
        fn capture_requests_ask_only_for_the_waiting_files_once_the_tab_is_in_the_order() {
            let path = |name: &str| PathBuf::from(format!("D:/x/{name}"));
            let mut tab = Tab {
                folder: Some(PathBuf::from("D:/x")),
                files: vec![path("a.jpg"), path("b.jpg"), path("c.jpg")],
                visible: vec![0, 1, 2],
                sorted: Some(CAPTURE),
                unplaced: vec![path("c.jpg")],
                ..Tab::default()
            };
            tab.capture.insert(path("a.jpg"), None);
            let none = HashSet::new();
            assert_eq!(capture_requests(&tab, CAPTURE, &none), [path("c.jpg")]);
            let in_flight: HashSet<PathBuf> = [path("c.jpg")].into_iter().collect();
            assert!(capture_requests(&tab, CAPTURE, &in_flight).is_empty());

            // Not in the order yet: every file not read or asked for, from the current one on.
            tab.sorted = None;
            tab.unplaced.clear();
            assert_eq!(
                capture_requests(&tab, CAPTURE, &none),
                [path("b.jpg"), path("c.jpg")]
            );
            assert_eq!(capture_requests(&tab, CAPTURE, &in_flight), [path("b.jpg")]);
        }

        #[test]
        fn the_preview_leaves_with_nothing_visible_once_the_ratings_are_ready() {
            let path = |name: &str| PathBuf::from(format!("D:/x/{name}"));
            let preview = |files: Vec<PathBuf>, visible: Vec<usize>| Tab {
                folder: Some(PathBuf::from("D:/x")),
                files,
                visible,
                mode: ViewMode::Loupe,
                ..Tab::default()
            };
            // Every image removed: the tab has no files.
            let emptied = preview(Vec::new(), Vec::new());
            assert!(preview_must_leave(Some(&emptied), true));
            // A filter no file matches: files, nothing visible; it waits while the ratings are
            // read.
            let unmatched = preview(vec![path("a.jpg"), path("b.jpg")], Vec::new());
            assert!(!preview_must_leave(Some(&unmatched), false));
            assert!(preview_must_leave(Some(&unmatched), true));
            // Something shown, the contact sheet, or no tab: it stays.
            let shown = preview(vec![path("a.jpg")], vec![0]);
            assert!(!preview_must_leave(Some(&shown), true));
            let sheet = Tab {
                mode: ViewMode::Grid,
                ..unmatched.clone()
            };
            assert!(!preview_must_leave(Some(&sheet), true));
            assert!(!preview_must_leave(None, true));
        }

        #[test]
        fn the_anchor_keeps_the_top_image_or_the_next_one_and_waits_while_nothing_is_shown() {
            let cell_size = 100;
            let stride = cell_size as f32 + GRID_SPACING;
            let columns = 4;
            let path = |name: String| PathBuf::from(format!("D:/x/{name}.jpg"));
            let tab_of = |names: Vec<String>| Tab {
                folder: Some(PathBuf::from("D:/x")),
                visible: (0..names.len()).collect(),
                files: names.into_iter().map(path).collect(),
                ..Tab::default()
            };
            let old: Vec<String> = (0..20).map(|i| format!("f{i:02}")).collect();
            let offset = 2.0 * stride + 5.0;
            let anchor = grid_note(
                &tab_of(old.clone()),
                0,
                GridView {
                    offset,
                    height: 2.0 * stride,
                    columns,
                    cell_size,
                },
            );
            assert_eq!(anchor.top, 8);
            assert_eq!(anchor.from_top.first(), Some(&path("f08".to_owned())));

            // Four files added above the top image: one row lower.
            let mut added: Vec<String> = (0..4).map(|i| format!("a{i}")).collect();
            added.extend(old.iter().cloned());
            assert_eq!(
                anchor_step(&anchor, &tab_of(added), true, stride, columns),
                AnchorStep::Set(anchored_offset(offset, stride, columns, 8, 12))
            );
            assert_eq!(
                anchored_offset(offset, stride, columns, 8, 12),
                offset + stride
            );

            // The top image and the first row gone: the next file, f09, now at 4, decides.
            let gone: Vec<String> = old
                .iter()
                .enumerate()
                .filter(|(index, _)| *index >= 4 && *index != 8)
                .map(|(_, name)| name.clone())
                .collect();
            assert_eq!(
                anchor_step(&anchor, &tab_of(gone), true, stride, columns),
                AnchorStep::Set(anchored_offset(offset, stride, columns, 8, 4))
            );

            // Nothing shown (an active filter while the ratings are read): no offset yet; once
            // the ratings are ready with none of its files shown, it is dropped.
            let mut hidden = tab_of(old.clone());
            hidden.visible.clear();
            assert_eq!(
                anchor_step(&anchor, &hidden, false, stride, columns),
                AnchorStep::Wait
            );
            assert_eq!(
                anchor_step(&anchor, &hidden, true, stride, columns),
                AnchorStep::Drop
            );
            let others = tab_of((0..4).map(|i| format!("o{i}")).collect());
            assert_eq!(
                anchor_step(&anchor, &others, false, stride, columns),
                AnchorStep::Drop
            );
        }

        // --- The tabs follow their folders on disk (ADR 0026 F3, T251) ---

        #[test]
        fn a_disk_listing_is_held_while_rawstride_moves_deletes_shows_a_dialog_or_imports_into_it()
        {
            let folder = Path::new("D:/Photos/2026/shoot");
            // nothing running
            assert!(!disk_listing_held(false, false, false, None, folder));
            // moves or an undo, a delete, a dialog
            assert!(disk_listing_held(true, false, false, None, folder));
            assert!(disk_listing_held(false, true, false, None, folder));
            assert!(disk_listing_held(false, false, true, None, folder));
            // inside or equal to the import's destination
            let destination = Path::new("D:/Photos/2026");
            assert!(disk_listing_held(
                false,
                false,
                false,
                Some(destination),
                folder
            ));
            assert!(disk_listing_held(false, false, false, Some(folder), folder));
            // outside it: a sibling with a longer name
            assert!(!disk_listing_held(
                false,
                false,
                false,
                Some(folder),
                Path::new("D:/Photos/2026/shoot-2")
            ));
            // no import
            assert!(!disk_listing_held(false, false, false, None, destination));
            assert!(!disk_listing_held(
                false,
                false,
                false,
                Some(Path::new("D:/Photos/2026/shoot-2")),
                folder
            ));
        }

        // --- The destination tabs fill while importing (ADR 0026 §3.4, T260) ---

        #[test]
        fn a_placed_group_fills_its_folders_tabs_or_opens_one_at_its_first_group() {
            // tabs of the folder exist: filled, whatever else holds
            for first_in_folder in [false, true] {
                for make_active in [false, true] {
                    assert_eq!(
                        placed_tabs(true, first_in_folder, make_active),
                        PlacedTabs::Fill
                    );
                }
            }
            // no tab, the folder's first group: a folder made active opens active, another one
            // behind
            assert_eq!(placed_tabs(false, true, true), PlacedTabs::OpenActive);
            assert_eq!(placed_tabs(false, true, false), PlacedTabs::OpenBehind);
            // no tab, images placed before: the user closed it, not opened again
            assert_eq!(placed_tabs(false, false, false), PlacedTabs::Closed);
            assert_eq!(placed_tabs(false, false, true), PlacedTabs::Closed);
        }

        fn import_of(planned: &[&str], placed: &[&str]) -> ImportTabs {
            let mut import = ImportTabs::new(planned.iter().map(PathBuf::from).collect());
            import.place(&placed.iter().map(PathBuf::from).collect::<Vec<_>>());
            import
        }

        fn paths(names: &[&str]) -> Vec<PathBuf> {
            names.iter().map(PathBuf::from).collect()
        }

        #[test]
        fn a_listing_leaves_out_the_images_an_import_has_not_shown_and_is_kept_without_one() {
            let listing = paths(&["D:/Shoot/old.nef", "D:/Shoot/a.nef", "D:/Shoot/b.nef"]);
            assert_eq!(leave_out_waiting(None, listing.clone()), listing);
            let import = import_of(&["D:/Shoot/a.nef", "D:/Shoot/b.nef"], &["D:/Shoot/a.nef"]);
            assert_eq!(
                leave_out_waiting(Some(&import), listing),
                paths(&["D:/Shoot/old.nef", "D:/Shoot/a.nef"])
            );
        }

        #[test]
        fn a_file_opened_during_an_import_keeps_its_position_in_the_filtered_listing() {
            let listing = paths(&[
                "D:/Shoot/a.nef",
                "D:/Shoot/b.nef",
                "D:/Shoot/c.nef",
                "D:/Shoot/d.nef",
            ]);
            // no import: unchanged
            assert_eq!(
                opened_during_import(None, listing.clone(), 2, true),
                (listing.clone(), 2)
            );
            let import = import_of(&["D:/Shoot/b.nef", "D:/Shoot/c.nef"], &["D:/Shoot/c.nef"]);
            let kept = paths(&["D:/Shoot/a.nef", "D:/Shoot/c.nef", "D:/Shoot/d.nef"]);
            // the opened file found again by its path
            assert_eq!(
                opened_during_import(Some(&import), listing.clone(), 2, true),
                (kept.clone(), 1)
            );
            assert_eq!(
                opened_during_import(Some(&import), listing.clone(), 3, true),
                (kept.clone(), 2)
            );
            // the opened file left out: the first file
            assert_eq!(
                opened_during_import(Some(&import), listing.clone(), 1, true),
                (kept.clone(), 0)
            );
            // a folder opened: the position kept within the files
            assert_eq!(
                opened_during_import(Some(&import), listing.clone(), 0, false),
                (kept.clone(), 0)
            );
            assert_eq!(
                opened_during_import(Some(&import), listing, 3, false),
                (kept, 2)
            );
            // everything left out: an empty listing at 0
            let all = import_of(&["D:/Shoot/a.nef"], &[]);
            assert_eq!(
                opened_during_import(Some(&all), paths(&["D:/Shoot/a.nef"]), 0, true),
                (Vec::new(), 0)
            );
        }

        #[test]
        fn the_disk_change_text_names_a_removed_current_image_else_counts_new_and_removed() {
            let current = PathBuf::from("D:/x/shoot/c.jpg");
            let current_gone = Relisted {
                added: 2,
                removed: 1,
                current_removed: Some(current.clone()),
                new: vec![PathBuf::from("D:/x/shoot/n1.jpg")],
                gone: vec![current.clone()],
            };
            assert_eq!(
                disk_change_text(&current_gone),
                Some(removed_by_another_program_text(&current))
            );
            let others = Relisted {
                added: 3,
                removed: 2,
                current_removed: None,
                ..Relisted::default()
            };
            assert_eq!(disk_change_text(&others), folder_changed_text(3, 2));
            assert!(disk_change_text(&others).is_some());
            assert_eq!(disk_change_text(&Relisted::default()), None);
        }

        #[test]
        fn a_failed_disk_listing_keeps_the_tab_and_says_the_folder_is_gone() {
            let dir = folder("disk-gone", &["a.jpg", "b.jpg"]);
            let mut tab = opened(&dir, 1, NAME, &|_| None);
            let before = tab.clone();
            remove_folder(&dir);

            let relisted = list_into_tab(
                &mut tab,
                listing(&dir),
                &nothing_pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            );

            assert_eq!(relisted, Err(folder_gone_text(&dir)));
            assert_eq!(tab, before);
        }

        #[test]
        fn a_disk_listing_reports_the_new_and_the_gone_files_and_nothing_when_unchanged() {
            let dir = folder("disk-changed", &["a.jpg", "c.jpg", "e.jpg"]);
            let mut tab = opened(&dir, 0, NAME, &|_| None);
            std::fs::write(dir.join("d.jpg"), b"x").unwrap();
            std::fs::remove_file(dir.join("c.jpg")).unwrap();

            let relisted = list_into_tab(
                &mut tab,
                listing(&dir),
                &nothing_pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            )
            .unwrap();

            assert_eq!(relisted.added, 1);
            assert_eq!(relisted.removed, 1);
            assert_eq!(relisted.current_removed, None);
            assert_eq!(relisted.new, [dir.join("d.jpg")]);
            assert_eq!(relisted.gone, [dir.join("c.jpg")]);
            assert_eq!(names(&tab), ["a.jpg", "d.jpg", "e.jpg"]);

            // the same files again: nothing changes
            let before = tab.clone();
            let again = list_into_tab(
                &mut tab,
                listing(&dir),
                &nothing_pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            )
            .unwrap();
            assert!(!again.changed());
            assert_eq!(again, Relisted::default());
            assert_eq!(tab, before);
            remove_folder(&dir);
        }

        #[test]
        fn a_disk_listing_counts_a_file_being_written_as_listed() {
            let dir = folder("disk-pending", &["a.jpg", "b.jpg", "c.jpg"]);
            let mut tab = opened(&dir, 0, NAME, &|_| None);
            // The safe write has renamed the original away for a moment.
            std::fs::remove_file(dir.join("b.jpg")).unwrap();
            let written = dir.join("b.jpg");
            let pending = |path: &Path| path == written;

            let relisted = list_into_tab(
                &mut tab,
                listing(&dir),
                &pending,
                NAME,
                true,
                &|_| None,
                &|_| None,
            )
            .unwrap();

            assert!(!relisted.changed());
            assert_eq!(names(&tab), ["a.jpg", "b.jpg", "c.jpg"]);
            remove_folder(&dir);
        }
    }
}
