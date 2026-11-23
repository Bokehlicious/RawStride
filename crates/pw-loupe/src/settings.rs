//! RawStride's settings file (ADR 0007 §5.2 and §5.4/Q10; owner decision D10): the setting "Skapa
//! IPTC-IIM för kompatibilitet med äldre system" (off by default) and the fallback encoding for IIM
//! and Exif text (Windows-1252 by default). The file is `%LOCALAPPDATA%\RawStride\settings.txt`,
//! next to the write journal, for this user on this computer only: UTF-8 text with one
//! `key = value` per line. Loading never fails: what cannot be read keeps its default and gives a
//! problem text in English for the log. Saving writes a temporary file and renames it onto the
//! settings file, so a crash never leaves half a file. The main window's remembered state (ADR
//! 0017 §2.5, U1, T109: theme, window, full screen, side columns; the metadata window's sections,
//! T123) is kept in the same file, and so are the library's favourites and recent folders and
//! its sections' states (ADR 0017 §2.1, U4, T135), and the sort order of the tabs (ADR 0017
//! §6.1, U14, T138).

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pw_metadata::edit::EditOptions;
use pw_metadata::iim::Fallback;
use pw_metadata::write_plan::WriteSettings;

use crate::editor::EditorSection;
use crate::editor_layout::{EditorItem, EditorLayout, section_from_name, section_name};
use crate::shell::{
    ColumnState, DEFAULT_LEFT_WIDTH, DEFAULT_RIGHT_WIDTH, KEY_ACTION_WIDTH_DEFAULT, Theme,
};
use crate::sort::{SortKey, SortOrder};

/// The key of the setting "Skapa IPTC-IIM för kompatibilitet med äldre system".
const CREATE_IIM: &str = "create_iim";
/// The key of the fallback encoding.
const FALLBACK_ENCODING: &str = "fallback_encoding";
/// The value of [`FALLBACK_ENCODING`] for Windows-1252.
const WINDOWS_1252: &str = "windows-1252";
/// The value of [`FALLBACK_ENCODING`] for ISO 8859-1 (Latin-1).
const ISO_8859_1: &str = "iso-8859-1";
/// The keys of the main window's remembered state (T109).
const THEME: &str = "theme";
const WINDOW: &str = "window";
const WINDOW_MAXIMIZED: &str = "window_maximized";
const FULLSCREEN: &str = "fullscreen";
const LEFT_COLUMN_WIDTH: &str = "left_column_width";
const RIGHT_COLUMN_WIDTH: &str = "right_column_width";
const LEFT_COLUMN: &str = "left_column";
const RIGHT_COLUMN: &str = "right_column";
const COLUMNS_HIDDEN: &str = "columns_hidden";
/// The keys of the inspector's sections, open or collapsed (ADR 0017 §2.1, §2.5, T110).
const INSPECTOR_INFO: &str = "inspector_info";
const INSPECTOR_METADATA: &str = "inspector_metadata";
/// The keys of the preview's remembered state (ADR 0017 §2.2, U3, T111).
const PREVIEW_COLUMN: &str = "preview_column";
const PREVIEW_COLUMN_WIDTH: &str = "preview_column_width";
const PREVIEW_INFO: &str = "preview_info";
const PREVIEW_ZOOM: &str = "preview_zoom";
const FILMSTRIP: &str = "filmstrip";
const FILMSTRIP_MARKED: &str = "filmstrip_marked";
const PREVIEW_HIDDEN: &str = "preview_hidden";
/// The key of the Action column's width on the Settings window's page Keyboard Shortcuts (ADR 0017
/// U8-Q15, T286).
const KEY_ACTION_WIDTH: &str = "key_action_width";
/// The keys of the Histogram sections of the preview's column and of the inspector, open or
/// collapsed (ADR 0017 §2.6, U7-Q7, T142).
const PREVIEW_HISTOGRAM: &str = "preview_histogram";
const INSPECTOR_HISTOGRAM: &str = "inspector_histogram";
/// The keys of the clipping overlays of the preview, on or off, and of the Clipping section of the
/// preview's column, open or collapsed (ADR 0017 §2.6, U7b-Q6, U7b-Q9, T148).
const CLIP_HIGHLIGHTS: &str = "clip_highlights";
const CLIP_SHADOWS: &str = "clip_shadows";
const PREVIEW_CLIPPING: &str = "preview_clipping";
/// The key of the preview's focus marking, on or off (ADR 0017 §2.6, U9-Q1, T168).
const FOCUS_MARKING: &str = "focus_marking";
/// The keys of the compare's layout (above and below instead of side by side) and of its lock of
/// zoom and position (ADR 0017 §2.2, U6-Q7, U6-Q8, T151).
const COMPARE_ABOVE_BELOW: &str = "compare_above_below";
const COMPARE_LOCKED: &str = "compare_locked";
/// The key of the import dialog's "copy new images only" check box (ADR 0005 choice 6, T158).
const IMPORT_INCREMENTAL: &str = "import_incremental";
/// The keys of whether `X` and `P` move images into subfolders and of the suffixes of the
/// companion files that move with an image (ADR 0017 §2.6, U9-Q5, U9-Q10, T171).
const MOVE_TO_SUBFOLDERS: &str = "move_to_subfolders";
const COMPANION_SUFFIXES: &str = "companion_suffixes";
/// The key of the four buttons on a thumbnail in the contact sheet, on or off (ADR 0017 §2.8,
/// U15-Q7, T245).
const THUMBNAIL_BUTTONS: &str = "thumbnail_buttons";
/// The companion suffixes by default (U9-Q10).
const DEFAULT_COMPANION_SUFFIXES: [&str; 3] = ["dop", "pp3", "on1"];
/// The keys of the metadata window's sections, open or collapsed (ADR 0017 U10-Q6, T123).
const EDITOR_CONTENT: &str = "editor_content";
const EDITOR_CREATOR_RIGHTS: &str = "editor_creator_rights";
const EDITOR_TIME_PLACE: &str = "editor_time_place";
const EDITOR_ASSIGNMENT: &str = "editor_assignment";
/// The key of the metadata window's Rating and Label section (U10c-Q5, T124).
const EDITOR_RATING_LABEL: &str = "editor_rating_label";
/// The key of the metadata window's Licence (PLUS) section (U10d, T128).
const EDITOR_LICENCE: &str = "editor_licence";
/// The keys of the metadata window's layout (ADR 0017 U10b-Q7, T277): the order of the sections
/// and of every section's items, and the hidden sections and items, each a list of names
/// separated by `, `, written only when it differs from the default.
const EDITOR_SECTION_ORDER: &str = "editor_section_order";
const EDITOR_ITEM_ORDER: &str = "editor_item_order";
const EDITOR_HIDDEN_SECTIONS: &str = "editor_hidden_sections";
const EDITOR_HIDDEN_ITEMS: &str = "editor_hidden_items";
/// The keys of the library's sections (T135, U4-Q7).
const LIBRARY_FOLDERS: &str = "library_folders";
const LIBRARY_FAVOURITES: &str = "library_favourites";
const LIBRARY_RECENT: &str = "library_recent";
/// The key of the Tasks section under the library (T140, U5-Q1).
const LIBRARY_TASKS: &str = "library_tasks";
/// The keys of the sort order of every tab (U14-Q3, T138).
const SORT: &str = "sort";
const SORT_DESCENDING: &str = "sort_descending";
/// The keys of a favourite and of a recent folder: one line per folder, in the list's order
/// (T135, U4-Q3, U4-Q4).
const FAVOURITE: &str = "favourite";
const RECENT: &str = "recent";
/// The key of an external program, `<name> | <path>`: one line per program, in the list's order
/// (ADR 0019 §2.1, E-Q1, T200).
const EXTERNAL_PROGRAM: &str = "external_program";
/// The values of [`FILMSTRIP`] and [`FILMSTRIP_MARKED`].
const SHOWN: &str = "shown";
const HIDDEN: &str = "hidden";
const ALL: &str = "all";
const MARKED: &str = "marked";

/// RawStride's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Create IIM in a JPEG, TIFF or DNG that has none (§5.2).
    pub create_iim: bool,
    /// The encoding for IIM and Exif text that is neither ASCII nor UTF-8 (§5.4, Q10).
    pub fallback: Fallback,
    /// The main window's remembered state (ADR 0017 §2.5, T109).
    pub shell: ShellSettings,
    /// The external programs in the list's order, the first the default program; at most
    /// [`MAX_EXTERNAL_PROGRAMS`] (ADR 0019 §2.1, E-Q1, T200).
    pub external_programs: Vec<ExternalProgram>,
    /// The metadata window's sections and items, their order and which are shown (ADR 0017
    /// §2.3.1, U10b, T277).
    pub editor_layout: EditorLayout,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            create_iim: false,
            fallback: Fallback::Windows1252,
            shell: ShellSettings::default(),
            external_programs: Vec::new(),
            editor_layout: EditorLayout::default(),
        }
    }
}

/// The most external programs the list holds (ADR 0019 §2.1, E-Q1).
pub const MAX_EXTERNAL_PROGRAMS: usize = 5;

/// A program the chosen images can be opened in: the name shown to the user and the path to its
/// `.exe`, which need not exist (ADR 0019 §2.1, E-Q9, T200).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProgram {
    pub name: String,
    pub path: PathBuf,
}

/// The window's last normal (not maximized, not full screen) place: the outer position and the
/// inner size in points, as eframe reports them, and the points-to-pixels scale at that time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

/// The main window's remembered state (ADR 0017 §2.5, U1, T109).
#[derive(Clone, Debug, PartialEq)]
pub struct ShellSettings {
    pub theme: Theme,
    /// `None` until the window has had a normal size; the window then opens maximized.
    pub window: Option<WindowRect>,
    pub window_maximized: bool,
    pub fullscreen: bool,
    /// The side columns' widths in points.
    pub left_column_width: u32,
    pub right_column_width: u32,
    pub left_column: ColumnState,
    pub right_column: ColumnState,
    /// Both side columns hidden with the hide key.
    pub columns_hidden: bool,
    /// The inspector's Info and Metadata sections (T110).
    pub inspector_info: ColumnState,
    pub inspector_metadata: ColumnState,
    /// The preview's right column, remembered apart from the main window's (T111, U3-Q2).
    pub preview_column: ColumnState,
    /// The preview's right column's width in points.
    pub preview_column_width: u32,
    /// The preview column's Info and Zoom sections.
    pub preview_info: ColumnState,
    pub preview_zoom: ColumnState,
    /// The filmstrip shown under the preview (`filmstrip = shown`).
    pub filmstrip: bool,
    /// "Only the marked" in the filmstrip (`filmstrip_marked = marked`, `S`, U3-Q3).
    pub filmstrip_marked: bool,
    /// The right column and the filmstrip hidden with `H` in the preview.
    pub preview_hidden: bool,
    /// The Action column's width in points on the Settings window's page Keyboard Shortcuts,
    /// dragged by the user (ADR 0017 U8-Q15, T286).
    pub key_action_width: u32,
    /// The metadata window's sections (T123, U10-Q6): Content, Creator and Rights, Time and
    /// Place, Assignment; Rating and Label (T124, U10c-Q5); Licence (PLUS) (T128, U10d).
    pub editor_content: ColumnState,
    pub editor_creator_rights: ColumnState,
    pub editor_time_place: ColumnState,
    pub editor_assignment: ColumnState,
    pub editor_rating_label: ColumnState,
    pub editor_licence: ColumnState,
    /// The library's sections Folders, Favourites and Recent (T135, U4-Q7).
    pub library_folders: ColumnState,
    pub library_favourites: ColumnState,
    pub library_recent: ColumnState,
    /// The Tasks section under the library (T140, U5-Q1).
    pub library_tasks: ColumnState,
    /// The sort order of every tab (U14-Q3, T138): `sort` and `sort_descending`.
    pub sort: SortOrder,
    /// The Histogram sections of the preview's column and of the inspector (T142, U7-Q7).
    pub preview_histogram: ColumnState,
    pub inspector_histogram: ColumnState,
    /// The preview's overlays that mark the clipped highlights (`J`) and the deepest shadows
    /// (`Shift+J`) on the image (T148, U7b-Q5, U7b-Q6); they stay as they are from image to image.
    pub clip_highlights: bool,
    pub clip_shadows: bool,
    /// The preview's focus marking (`E`): the edges of high local contrast marked in green on the
    /// image (T168, U9-Q1); it stays as it is from image to image.
    pub focus_marking: bool,
    /// The Clipping section of the preview's column (T148, U7b-Q9).
    pub preview_clipping: ColumnState,
    /// The compare's two images above and below each other (`V`) instead of side by side (T151,
    /// U6-Q7), and its zoom and position locked together (`L`, U6-Q8). The compare itself is not
    /// remembered (U6-Q11).
    pub compare_above_below: bool,
    pub compare_locked: bool,
    /// The import dialog's "copy new images only" check box (ADR 0005 choice 6, T158).
    pub import_incremental: bool,
    /// `X` and `P` move images into subfolders (ADR 0017 §2.6, U9-Q5, T171); off by default.
    pub move_to_subfolders: bool,
    /// The suffixes of the companion files that move with an image, without the dot, trimmed and
    /// in lower case (U9-Q10, T171).
    pub companion_suffixes: Vec<String>,
    /// The four buttons on a thumbnail in the contact sheet, shown while the pointer is over the
    /// cell (View ▸ Buttons on Thumbnails; ADR 0017 §2.8, U15-Q7, T245); on by default.
    pub thumbnail_buttons: bool,
}

/// The library's lists as `settings.txt` keeps them (T135, U4-Q3, U4-Q4): the favourites in the
/// order they were added and the recent folders, most recent first. Kept apart from [`Settings`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedLibrary {
    pub favourites: Vec<PathBuf>,
    pub recent: Vec<PathBuf>,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            window: None,
            window_maximized: false,
            fullscreen: false,
            left_column_width: DEFAULT_LEFT_WIDTH,
            right_column_width: DEFAULT_RIGHT_WIDTH,
            left_column: ColumnState::Open,
            // The main window starts without the inspector; it is opened from Visa (owner,
            // 2026-11-13).
            right_column: ColumnState::Collapsed,
            columns_hidden: false,
            // Both sections open (U2-Q5).
            inspector_info: ColumnState::Open,
            inspector_metadata: ColumnState::Open,
            // The preview's column starts open, with both sections (U3-Q2); the filmstrip is
            // shown with every visible image (U3-Q3).
            preview_column: ColumnState::Open,
            preview_column_width: DEFAULT_RIGHT_WIDTH,
            preview_info: ColumnState::Open,
            preview_zoom: ColumnState::Open,
            filmstrip: true,
            filmstrip_marked: false,
            preview_hidden: false,
            // 220 points (U8-Q15).
            key_action_width: KEY_ACTION_WIDTH_DEFAULT as u32,
            // Every section of the metadata window open (U10-Q6).
            editor_content: ColumnState::Open,
            editor_creator_rights: ColumnState::Open,
            editor_time_place: ColumnState::Open,
            editor_assignment: ColumnState::Open,
            editor_rating_label: ColumnState::Open,
            editor_licence: ColumnState::Open,
            // Every section of the library open.
            library_folders: ColumnState::Open,
            library_favourites: ColumnState::Open,
            library_recent: ColumnState::Open,
            // The Tasks section open (U5-Q1).
            library_tasks: ColumnState::Open,
            // File-name order, ascending: the order before T138 (U14-Q1).
            sort: SortOrder::default(),
            // Both Histogram sections open, as the other sections of their columns.
            preview_histogram: ColumnState::Open,
            inspector_histogram: ColumnState::Open,
            // Both overlays off (U7b-Q6); the Clipping section open (U7b-Q9).
            clip_highlights: false,
            clip_shadows: false,
            // The focus marking off (T168).
            focus_marking: false,
            preview_clipping: ColumnState::Open,
            // The compare side by side (U6-Q7) and locked (U6-Q8).
            compare_above_below: false,
            compare_locked: true,
            // New images only (owner, 2026-11-16, ADR 0005 choice 6).
            import_incremental: true,
            // Images are not moved (U9-Q5); the companion suffixes of U9-Q10.
            move_to_subfolders: false,
            companion_suffixes: default_companion_suffixes(),
            // The buttons on a thumbnail shown (U15-Q7).
            thumbnail_buttons: true,
        }
    }
}

impl ShellSettings {
    /// The state of a section of the metadata window (T123).
    pub fn editor_section(&self, section: EditorSection) -> ColumnState {
        match section {
            EditorSection::Content => self.editor_content,
            EditorSection::CreatorRights => self.editor_creator_rights,
            EditorSection::TimePlace => self.editor_time_place,
            EditorSection::Assignment => self.editor_assignment,
            EditorSection::Licence => self.editor_licence,
            EditorSection::RatingLabel => self.editor_rating_label,
        }
    }

    /// Opens a collapsed section of the metadata window, or collapses an open one.
    pub fn toggle_editor_section(&mut self, section: EditorSection) {
        let state = match section {
            EditorSection::Content => &mut self.editor_content,
            EditorSection::CreatorRights => &mut self.editor_creator_rights,
            EditorSection::TimePlace => &mut self.editor_time_place,
            EditorSection::Assignment => &mut self.editor_assignment,
            EditorSection::Licence => &mut self.editor_licence,
            EditorSection::RatingLabel => &mut self.editor_rating_label,
        };
        *state = state.toggled();
    }
}

impl Settings {
    /// The options the metadata writes and reads use with these settings.
    pub fn edit_options(&self) -> EditOptions {
        EditOptions {
            settings: WriteSettings {
                create_iim: self.create_iim,
            },
            fallback: self.fallback,
        }
    }
}

/// `%LOCALAPPDATA%\RawStride\settings.txt`; `None` without `LOCALAPPDATA` (or when it is empty).
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|data| !data.is_empty())
        .map(|data| PathBuf::from(data).join("RawStride").join("settings.txt"))
}

/// Reads the settings file without the library's lists ([`load_settings_and_library`]).
pub fn load_settings(path: &Path) -> (Settings, Vec<String>) {
    let (settings, _, problems) = load_settings_and_library(path);
    (settings, problems)
}

/// Reads the settings file. A missing file gives the defaults and no problem. An unknown key, a
/// bad value or a line without `=` keeps the default for it and gives one problem naming the line
/// number; a file that cannot be read gives the defaults and one problem. The problems are English,
/// for the log (`eprintln!` by the caller). The `favourite` and `recent` lines give the library's
/// lists in their order; a path that is empty or not a full path is skipped with a problem.
pub fn load_settings_and_library(path: &Path) -> (Settings, SavedLibrary, Vec<String>) {
    let mut settings = Settings::default();
    let mut library = SavedLibrary::default();
    let mut problems = Vec::new();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (settings, library, problems);
        }
        Err(error) => {
            problems.push(format!(
                "settings file {} could not be read, defaults used: {error}",
                path.display()
            ));
            return (settings, library, problems);
        }
    };
    // A byte order mark written by an editor is not part of the first key.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!(
                "settings file {}, line {number}: no '=' in \"{line}\", line ignored",
                path.display()
            ));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let bad = |expected: &str| {
            format!(
                "settings file {}, line {number}: bad value \"{value}\" for {key} (expected \
                 {expected}), default used",
                path.display()
            )
        };
        let shell = &mut settings.shell;
        match key {
            CREATE_IIM => match parse_bool(value) {
                Some(on) => settings.create_iim = on,
                None => problems.push(format!(
                    "settings file {}, line {number}: bad value \"{value}\" for {CREATE_IIM} \
                     (expected true or false), default used",
                    path.display()
                )),
            },
            FALLBACK_ENCODING => match parse_fallback(value) {
                Some(fallback) => settings.fallback = fallback,
                None => problems.push(format!(
                    "settings file {}, line {number}: bad value \"{value}\" for \
                     {FALLBACK_ENCODING} (expected {WINDOWS_1252} or {ISO_8859_1}), default used",
                    path.display()
                )),
            },
            THEME => match parse_theme(value) {
                Some(theme) => shell.theme = theme,
                None => problems.push(bad("dark or light")),
            },
            WINDOW => match parse_window(value) {
                Some(window) => shell.window = Some(window),
                None => problems.push(bad("x,y,width,height,scale")),
            },
            SORT => match parse_sort_key(value) {
                Some(key) => shell.sort.key = key,
                None => problems.push(bad("name, capture, modified, type or rating")),
            },
            WINDOW_MAXIMIZED | FULLSCREEN | COLUMNS_HIDDEN | PREVIEW_HIDDEN | SORT_DESCENDING
            | CLIP_HIGHLIGHTS | CLIP_SHADOWS | FOCUS_MARKING | COMPARE_ABOVE_BELOW
            | COMPARE_LOCKED | IMPORT_INCREMENTAL | MOVE_TO_SUBFOLDERS | THUMBNAIL_BUTTONS => {
                match parse_bool(value) {
                    Some(on) => match key {
                        WINDOW_MAXIMIZED => shell.window_maximized = on,
                        FULLSCREEN => shell.fullscreen = on,
                        PREVIEW_HIDDEN => shell.preview_hidden = on,
                        SORT_DESCENDING => shell.sort.descending = on,
                        CLIP_HIGHLIGHTS => shell.clip_highlights = on,
                        CLIP_SHADOWS => shell.clip_shadows = on,
                        FOCUS_MARKING => shell.focus_marking = on,
                        COMPARE_ABOVE_BELOW => shell.compare_above_below = on,
                        COMPARE_LOCKED => shell.compare_locked = on,
                        IMPORT_INCREMENTAL => shell.import_incremental = on,
                        MOVE_TO_SUBFOLDERS => shell.move_to_subfolders = on,
                        THUMBNAIL_BUTTONS => shell.thumbnail_buttons = on,
                        _ => shell.columns_hidden = on,
                    },
                    None => problems.push(bad("true or false")),
                }
            }
            LEFT_COLUMN_WIDTH | RIGHT_COLUMN_WIDTH | PREVIEW_COLUMN_WIDTH | KEY_ACTION_WIDTH => {
                match value.parse::<u32>() {
                    Ok(width) if key == LEFT_COLUMN_WIDTH => shell.left_column_width = width,
                    Ok(width) if key == PREVIEW_COLUMN_WIDTH => shell.preview_column_width = width,
                    Ok(width) if key == KEY_ACTION_WIDTH => shell.key_action_width = width,
                    Ok(width) => shell.right_column_width = width,
                    Err(_) => problems.push(bad("a width in points")),
                }
            }
            LEFT_COLUMN | RIGHT_COLUMN => match parse_column(value) {
                Some(state) if key == LEFT_COLUMN => shell.left_column = state,
                Some(state) => shell.right_column = state,
                None => problems.push(bad("open or collapsed")),
            },
            INSPECTOR_INFO | INSPECTOR_METADATA => match parse_column(value) {
                Some(state) if key == INSPECTOR_INFO => shell.inspector_info = state,
                Some(state) => shell.inspector_metadata = state,
                None => problems.push(bad("open or collapsed")),
            },
            EDITOR_CONTENT
            | EDITOR_CREATOR_RIGHTS
            | EDITOR_TIME_PLACE
            | EDITOR_ASSIGNMENT
            | EDITOR_LICENCE
            | EDITOR_RATING_LABEL => match parse_column(value) {
                Some(state) => match key {
                    EDITOR_CONTENT => shell.editor_content = state,
                    EDITOR_CREATOR_RIGHTS => shell.editor_creator_rights = state,
                    EDITOR_TIME_PLACE => shell.editor_time_place = state,
                    EDITOR_ASSIGNMENT => shell.editor_assignment = state,
                    EDITOR_LICENCE => shell.editor_licence = state,
                    _ => shell.editor_rating_label = state,
                },
                None => problems.push(bad("open or collapsed")),
            },
            PREVIEW_COLUMN | PREVIEW_INFO | PREVIEW_ZOOM => match parse_column(value) {
                Some(state) => match key {
                    PREVIEW_COLUMN => shell.preview_column = state,
                    PREVIEW_INFO => shell.preview_info = state,
                    _ => shell.preview_zoom = state,
                },
                None => problems.push(bad("open or collapsed")),
            },
            PREVIEW_HISTOGRAM | INSPECTOR_HISTOGRAM => match parse_column(value) {
                Some(state) if key == PREVIEW_HISTOGRAM => shell.preview_histogram = state,
                Some(state) => shell.inspector_histogram = state,
                None => problems.push(bad("open or collapsed")),
            },
            PREVIEW_CLIPPING => match parse_column(value) {
                Some(state) => shell.preview_clipping = state,
                None => problems.push(bad("open or collapsed")),
            },
            // A line without a valid entry gives the default; it is not reported (T171).
            COMPANION_SUFFIXES => shell.companion_suffixes = parse_companion_suffixes(value),
            FILMSTRIP => match value {
                SHOWN => shell.filmstrip = true,
                HIDDEN => shell.filmstrip = false,
                _ => problems.push(bad("shown or hidden")),
            },
            FILMSTRIP_MARKED => match value {
                MARKED => shell.filmstrip_marked = true,
                ALL => shell.filmstrip_marked = false,
                _ => problems.push(bad("all or marked")),
            },
            LIBRARY_FOLDERS | LIBRARY_FAVOURITES | LIBRARY_RECENT | LIBRARY_TASKS => {
                match parse_column(value) {
                    Some(state) => match key {
                        LIBRARY_FOLDERS => shell.library_folders = state,
                        LIBRARY_FAVOURITES => shell.library_favourites = state,
                        LIBRARY_RECENT => shell.library_recent = state,
                        _ => shell.library_tasks = state,
                    },
                    None => problems.push(bad("open or collapsed")),
                }
            }
            FAVOURITE | RECENT => match parse_folder(value) {
                Some(folder) if key == FAVOURITE => library.favourites.push(folder),
                Some(folder) => library.recent.push(folder),
                None => problems.push(format!(
                    "settings file {}, line {number}: bad value \"{value}\" for {key} (expected \
                     a full folder path), line ignored",
                    path.display()
                )),
            },
            // The value is checked before the count, so a bad line is reported as bad wherever
            // it is (T200).
            EXTERNAL_PROGRAM => match parse_external_program(value) {
                Some(_) if settings.external_programs.len() >= MAX_EXTERNAL_PROGRAMS => {
                    problems.push(format!(
                        "settings file {}, line {number}: more than {MAX_EXTERNAL_PROGRAMS} \
                         external programs, line ignored",
                        path.display()
                    ));
                }
                Some(program) => settings.external_programs.push(program),
                None => problems.push(format!(
                    "settings file {}, line {number}: bad value \"{value}\" for {key} (expected \
                     a name, '|' and a full path to a program), line ignored",
                    path.display()
                )),
            },
            // Each unknown name is skipped with a problem; the rest of the line is used (T277).
            // A hidden line names every hidden one, so it replaces an earlier line.
            EDITOR_SECTION_ORDER | EDITOR_HIDDEN_SECTIONS => {
                let (sections, unknown) = layout_names(value, section_from_name);
                problems.extend(
                    unknown
                        .iter()
                        .map(|name| unknown_name(path, number, key, name)),
                );
                let layout = &mut settings.editor_layout;
                if key == EDITOR_SECTION_ORDER {
                    layout.set_section_order(&sections);
                } else {
                    for section in EditorSection::ALL {
                        layout.set_section_shown(section, !sections.contains(&section));
                    }
                }
            }
            EDITOR_ITEM_ORDER | EDITOR_HIDDEN_ITEMS => {
                let (items, unknown) = layout_names(value, EditorItem::from_name);
                problems.extend(
                    unknown
                        .iter()
                        .map(|name| unknown_name(path, number, key, name)),
                );
                let layout = &mut settings.editor_layout;
                if key == EDITOR_ITEM_ORDER {
                    layout.set_item_order(&items);
                } else {
                    for section in EditorSection::ALL {
                        for (item, _) in layout.items(section) {
                            layout.set_item_shown(item, !items.contains(&item));
                        }
                    }
                }
            }
            _ => problems.push(format!(
                "settings file {}, line {number}: unknown key \"{key}\", line ignored",
                path.display()
            )),
        }
    }
    (settings, library, problems)
}

/// Writes the settings without favourites or recent folders ([`save_settings_with_library`]).
pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    save_settings_with_library(path, settings, &SavedLibrary::default())
}

/// Writes the settings: creates the folder, writes every key (after the comment line
/// `# RawStride`; `window` only when there is one) to `<file name>.tmp` in the same folder, then renames it onto the settings file
/// (which replaces it on Windows), so a crash never leaves half a file. The library's favourites
/// and recent folders follow, one line each in their order; a path that could not be read back
/// as it is (not UTF-8, a line break in it, spaces at either end, not a full path) is left out.
pub fn save_settings_with_library(
    path: &Path,
    settings: &Settings,
    library: &SavedLibrary,
) -> io::Result<()> {
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("settings path {} has no file name", path.display()),
        ));
    };
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut temp_name = file_name.to_os_string();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let mut text = format!(
        "# RawStride\n{CREATE_IIM} = {}\n{FALLBACK_ENCODING} = {}\n",
        settings.create_iim,
        fallback_value(settings.fallback)
    );
    text.push_str(&shell_lines(&settings.shell));
    text.push_str(&editor_layout_lines(&settings.editor_layout));
    text.push_str(&external_program_lines(&settings.external_programs));
    text.push_str(&library_lines(library));
    let written = write_synced(&temp, text.as_bytes()).and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        // Best effort: the error of the write or the rename is the one reported.
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Writes the bytes to a new file and waits until they are on the disk, so the rename that follows
/// never puts an incomplete file in place.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The lines of the main window's remembered state.
fn shell_lines(shell: &ShellSettings) -> String {
    let mut text = format!("{THEME} = {}\n", theme_value(shell.theme));
    if let Some(w) = shell.window {
        text.push_str(&format!(
            "{WINDOW} = {},{},{},{},{}\n",
            w.x, w.y, w.width, w.height, w.scale
        ));
    }
    text.push_str(&format!(
        "{WINDOW_MAXIMIZED} = {}\n{FULLSCREEN} = {}\n{LEFT_COLUMN_WIDTH} = {}\n\
         {RIGHT_COLUMN_WIDTH} = {}\n{LEFT_COLUMN} = {}\n{RIGHT_COLUMN} = {}\n\
         {COLUMNS_HIDDEN} = {}\n{INSPECTOR_INFO} = {}\n{INSPECTOR_METADATA} = {}\n\
         {PREVIEW_COLUMN} = {}\n{PREVIEW_COLUMN_WIDTH} = {}\n{PREVIEW_INFO} = {}\n\
         {PREVIEW_ZOOM} = {}\n{FILMSTRIP} = {}\n{FILMSTRIP_MARKED} = {}\n{PREVIEW_HIDDEN} = {}\n\
         {KEY_ACTION_WIDTH} = {}\n",
        shell.window_maximized,
        shell.fullscreen,
        shell.left_column_width,
        shell.right_column_width,
        column_value(shell.left_column),
        column_value(shell.right_column),
        shell.columns_hidden,
        column_value(shell.inspector_info),
        column_value(shell.inspector_metadata),
        column_value(shell.preview_column),
        shell.preview_column_width,
        column_value(shell.preview_info),
        column_value(shell.preview_zoom),
        if shell.filmstrip { SHOWN } else { HIDDEN },
        if shell.filmstrip_marked { MARKED } else { ALL },
        shell.preview_hidden,
        shell.key_action_width
    ));
    text.push_str(&format!(
        "{EDITOR_CONTENT} = {}\n{EDITOR_CREATOR_RIGHTS} = {}\n{EDITOR_TIME_PLACE} = {}\n\
         {EDITOR_ASSIGNMENT} = {}\n{EDITOR_RATING_LABEL} = {}\n{EDITOR_LICENCE} = {}\n",
        column_value(shell.editor_content),
        column_value(shell.editor_creator_rights),
        column_value(shell.editor_time_place),
        column_value(shell.editor_assignment),
        column_value(shell.editor_rating_label),
        column_value(shell.editor_licence)
    ));
    text.push_str(&format!(
        "{LIBRARY_FOLDERS} = {}\n{LIBRARY_FAVOURITES} = {}\n{LIBRARY_RECENT} = {}\n\
         {LIBRARY_TASKS} = {}\n",
        column_value(shell.library_folders),
        column_value(shell.library_favourites),
        column_value(shell.library_recent),
        column_value(shell.library_tasks)
    ));
    text.push_str(&format!(
        "{SORT} = {}\n{SORT_DESCENDING} = {}\n",
        sort_key_value(shell.sort.key),
        shell.sort.descending
    ));
    text.push_str(&format!(
        "{PREVIEW_HISTOGRAM} = {}\n{INSPECTOR_HISTOGRAM} = {}\n",
        column_value(shell.preview_histogram),
        column_value(shell.inspector_histogram)
    ));
    text.push_str(&format!(
        "{CLIP_HIGHLIGHTS} = {}\n{CLIP_SHADOWS} = {}\n{FOCUS_MARKING} = {}\n{PREVIEW_CLIPPING} = {}\n",
        shell.clip_highlights,
        shell.clip_shadows,
        shell.focus_marking,
        column_value(shell.preview_clipping)
    ));
    text.push_str(&format!(
        "{COMPARE_ABOVE_BELOW} = {}\n{COMPARE_LOCKED} = {}\n",
        shell.compare_above_below, shell.compare_locked
    ));
    text.push_str(&format!(
        "{IMPORT_INCREMENTAL} = {}\n",
        shell.import_incremental
    ));
    text.push_str(&format!(
        "{MOVE_TO_SUBFOLDERS} = {}\n{COMPANION_SUFFIXES} = {}\n",
        shell.move_to_subfolders,
        shell.companion_suffixes.join(", ")
    ));
    text.push_str(&format!(
        "{THUMBNAIL_BUTTONS} = {}\n",
        shell.thumbnail_buttons
    ));
    text
}

/// The lines of the metadata window's layout (T277), each only when it differs from the
/// default, so a file with the default layout has none: the sections in the layout's order; every
/// item of every section, the sections in `EditorSection::ALL` order and each section's items in
/// the layout's order; the hidden sections and the hidden items in the same orders.
fn editor_layout_lines(layout: &EditorLayout) -> String {
    let mut text = String::new();
    let sections = layout.sections();
    let section_order: Vec<EditorSection> = sections.iter().map(|(section, _)| *section).collect();
    if section_order != EditorSection::ALL {
        let names: Vec<&str> = section_order.iter().map(|s| section_name(*s)).collect();
        text.push_str(&format!("{EDITOR_SECTION_ORDER} = {}\n", names.join(", ")));
    }
    let items: Vec<(EditorItem, bool)> = EditorSection::ALL
        .into_iter()
        .flat_map(|section| layout.items(section))
        .collect();
    let default_items: Vec<EditorItem> = EditorSection::ALL
        .into_iter()
        .flat_map(EditorItem::defaults)
        .collect();
    if items.iter().map(|(item, _)| *item).ne(default_items) {
        let names: Vec<&str> = items.iter().map(|(item, _)| item.name()).collect();
        text.push_str(&format!("{EDITOR_ITEM_ORDER} = {}\n", names.join(", ")));
    }
    let hidden_sections: Vec<&str> = sections
        .iter()
        .filter(|(_, shown)| !*shown)
        .map(|(section, _)| section_name(*section))
        .collect();
    if !hidden_sections.is_empty() {
        text.push_str(&format!(
            "{EDITOR_HIDDEN_SECTIONS} = {}\n",
            hidden_sections.join(", ")
        ));
    }
    let hidden_items: Vec<&str> = items
        .iter()
        .filter(|(_, shown)| !*shown)
        .map(|(item, _)| item.name())
        .collect();
    if !hidden_items.is_empty() {
        text.push_str(&format!(
            "{EDITOR_HIDDEN_ITEMS} = {}\n",
            hidden_items.join(", ")
        ));
    }
    text
}

/// The entries of a layout line (T277), separated by commas and trimmed, an empty entry left
/// out: those `from_name` knows in their order, and the names it does not know.
fn layout_names<T>(value: &str, from_name: impl Fn(&str) -> Option<T>) -> (Vec<T>, Vec<String>) {
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    for name in value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        match from_name(name) {
            Some(entry) => known.push(entry),
            None => unknown.push(name.to_string()),
        }
    }
    (known, unknown)
}

/// The problem of an unknown name on a layout line (T277).
fn unknown_name(path: &Path, number: usize, key: &str, name: &str) -> String {
    format!(
        "settings file {}, line {number}: unknown name \"{name}\" for {key}, name skipped",
        path.display()
    )
}

/// The companion suffixes by default: `dop`, `pp3`, `on1` (U9-Q10).
fn default_companion_suffixes() -> Vec<String> {
    DEFAULT_COMPANION_SUFFIXES
        .iter()
        .map(|suffix| (*suffix).to_string())
        .collect()
}

/// The suffixes of a `companion_suffixes` line, separated by commas: each trimmed and in lower
/// case; an empty entry, or one holding `.`, `/`, `\`, `:` or a space, is dropped. A line with no
/// valid entry gives the default. The Settings window's field Companion File Suffixes is read the
/// same way (ADR 0017 U8-Q10, T281).
pub fn parse_companion_suffixes(value: &str) -> Vec<String> {
    let suffixes: Vec<String> = value
        .split(',')
        .map(|entry| entry.trim().to_lowercase())
        .filter(|entry| !entry.is_empty() && !entry.contains(['.', '/', '\\', ':', ' ']))
        .collect();
    if suffixes.is_empty() {
        default_companion_suffixes()
    } else {
        suffixes
    }
}

/// The `favourite` and `recent` lines, each list in its order; a path that [`parse_folder`] would
/// not give back as it is, is left out.
fn library_lines(library: &SavedLibrary) -> String {
    let mut text = String::new();
    for (key, folders) in [(FAVOURITE, &library.favourites), (RECENT, &library.recent)] {
        for folder in folders {
            if let Some(value) = folder.to_str()
                && parse_folder(value).as_deref() == Some(folder.as_path())
            {
                text.push_str(&format!("{key} = {value}\n"));
            }
        }
    }
    text
}

/// The `external_program` lines in the list's order (T200): a program that
/// [`parse_external_program`] would not give back as it is, is left out; of those left, the first
/// [`MAX_EXTERNAL_PROGRAMS`] are written.
fn external_program_lines(programs: &[ExternalProgram]) -> String {
    let mut text = String::new();
    for (name, path) in programs
        .iter()
        .filter_map(|program| {
            let path = program.path.to_str()?;
            let line_value = format!("{} | {path}", program.name);
            (parse_external_program(&line_value).as_ref() == Some(program))
                .then_some((&program.name, path))
        })
        .take(MAX_EXTERNAL_PROGRAMS)
    {
        text.push_str(&format!("{EXTERNAL_PROGRAM} = {name} | {path}\n"));
    }
    text
}

/// The program of an `external_program` line, `<name> | <path>` (T200): exactly one `|`, the
/// name and the path each trimmed; the name not empty, the path a full path without a line break
/// ([`parse_folder`]). The file need not exist.
fn parse_external_program(value: &str) -> Option<ExternalProgram> {
    let (name, path) = value.split_once('|')?;
    if path.contains('|') {
        return None;
    }
    let (name, path) = (name.trim(), path.trim());
    if name.is_empty() || name.contains(['\n', '\r']) {
        return None;
    }
    Some(ExternalProgram {
        name: name.to_string(),
        path: parse_folder(path)?,
    })
}

/// The list the Settings window saves (T200): each name without `|` and line breaks and trimmed;
/// an empty name becomes the file name of its path without the extension, and a program with no
/// such name either is dropped; at most [`MAX_EXTERNAL_PROGRAMS`] are kept. The paths are not
/// changed.
pub fn tidied_programs(programs: &[ExternalProgram]) -> Vec<ExternalProgram> {
    let cleaned = |name: &str| {
        name.chars()
            .filter(|c| !matches!(c, '|' | '\n' | '\r'))
            .collect::<String>()
            .trim()
            .to_string()
    };
    programs
        .iter()
        .filter_map(|program| {
            let mut name = cleaned(&program.name);
            if name.is_empty() {
                name = program
                    .path
                    .file_stem()
                    .map(|stem| cleaned(&stem.to_string_lossy()))
                    .unwrap_or_default();
            }
            (!name.is_empty()).then(|| ExternalProgram {
                name,
                path: program.path.clone(),
            })
        })
        .take(MAX_EXTERNAL_PROGRAMS)
        .collect()
}

/// The folder of a `favourite` or `recent` line: a full path without a line break or spaces at
/// either end (they would be lost when the line is read).
fn parse_folder(value: &str) -> Option<PathBuf> {
    let folder = Path::new(value);
    let readable = !value.is_empty()
        && value.trim() == value
        && !value.contains(['\n', '\r'])
        && folder.is_absolute();
    readable.then(|| folder.to_path_buf())
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn theme_value(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "dark",
        Theme::Light => "light",
    }
}

fn parse_theme(value: &str) -> Option<Theme> {
    match value {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    }
}

fn sort_key_value(key: SortKey) -> &'static str {
    match key {
        SortKey::Name => "name",
        SortKey::Capture => "capture",
        SortKey::Modified => "modified",
        SortKey::Type => "type",
        SortKey::Rating => "rating",
    }
}

fn parse_sort_key(value: &str) -> Option<SortKey> {
    match value {
        "name" => Some(SortKey::Name),
        "capture" => Some(SortKey::Capture),
        "modified" => Some(SortKey::Modified),
        "type" => Some(SortKey::Type),
        "rating" => Some(SortKey::Rating),
        _ => None,
    }
}

fn column_value(state: ColumnState) -> &'static str {
    match state {
        ColumnState::Open => "open",
        ColumnState::Collapsed => "collapsed",
    }
}

fn parse_column(value: &str) -> Option<ColumnState> {
    match value {
        "open" => Some(ColumnState::Open),
        "collapsed" => Some(ColumnState::Collapsed),
        _ => None,
    }
}

/// `x,y,width,height,scale`: whole points and a positive, finite scale.
fn parse_window(value: &str) -> Option<WindowRect> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    let [x, y, width, height, scale] = parts.as_slice() else {
        return None;
    };
    let scale: f32 = scale.parse().ok()?;
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some(WindowRect {
        x: x.parse().ok()?,
        y: y.parse().ok()?,
        width: width.parse().ok()?,
        height: height.parse().ok()?,
        scale,
    })
}

/// The file value of an encoding.
fn fallback_value(fallback: Fallback) -> &'static str {
    match fallback {
        Fallback::Windows1252 => WINDOWS_1252,
        Fallback::Latin1 => ISO_8859_1,
    }
}

/// The encoding of a file value; `None` for an unknown value.
fn parse_fallback(value: &str) -> Option<Fallback> {
    match value {
        WINDOWS_1252 => Some(Fallback::Windows1252),
        ISO_8859_1 => Some(Fallback::Latin1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use pw_metadata::edit::EditOptions;
    use pw_metadata::iim::Fallback;
    use pw_metadata::write_plan::WriteSettings;

    use super::{
        ExternalProgram, MAX_EXTERNAL_PROGRAMS, SavedLibrary, Settings, ShellSettings, WindowRect,
        load_settings, load_settings_and_library, save_settings, save_settings_with_library,
        settings_path, tidied_programs,
    };
    use crate::editor::EditorSection;
    use crate::editor_layout::{EditorItem, EditorLayout};
    use crate::shell::{ColumnState, Theme};
    use crate::sort::{SORT_KEYS, SortKey, SortOrder};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    /// A new temporary folder (not created), removed with its contents when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pw-loupe-settings-{}-{id}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        /// Creates the folder and writes `settings.txt` in it.
        fn with_file(text: &str) -> (Self, PathBuf) {
            let dir = Self::new();
            std::fs::create_dir_all(&dir.0).unwrap();
            let path = dir.0.join("settings.txt");
            std::fs::write(&path, text).unwrap();
            (dir, path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn file_names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Asserts exactly one problem, naming the line.
    fn assert_one_problem_on_line(problems: &[String], line: usize) {
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains(&format!("line {line}:")),
            "{:?} does not name line {line}",
            problems[0]
        );
    }

    #[test]
    fn defaults_are_no_iim_and_windows_1252() {
        let settings = Settings::default();
        assert!(!settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Windows1252);
    }

    #[test]
    fn missing_file_gives_defaults_and_no_problem() {
        let dir = TempDir::new();
        let (settings, problems) = load_settings(&dir.0.join("settings.txt"));
        assert_eq!(settings, Settings::default());
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn save_then_load_gives_the_same_settings_for_all_four_combinations() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        // The same file each time: every save after the first replaces an existing file.
        for create_iim in [false, true] {
            for fallback in [Fallback::Windows1252, Fallback::Latin1] {
                let settings = Settings {
                    create_iim,
                    fallback,
                    ..Settings::default()
                };
                save_settings(&path, &settings).unwrap();
                let (loaded, problems) = load_settings(&path);
                assert_eq!(loaded, settings);
                assert!(problems.is_empty(), "{problems:?}");
            }
        }
    }

    #[test]
    fn saved_file_has_the_comment_line_and_every_key() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let settings = Settings {
            create_iim: true,
            fallback: Fallback::Latin1,
            ..Settings::default()
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# RawStride\ncreate_iim = true\nfallback_encoding = iso-8859-1\ntheme = dark\n\
             window_maximized = false\nfullscreen = false\nleft_column_width = 240\n\
             right_column_width = 300\nleft_column = open\nright_column = collapsed\n\
             columns_hidden = false\ninspector_info = open\ninspector_metadata = open\n\
             preview_column = open\npreview_column_width = 300\npreview_info = open\n\
             preview_zoom = open\nfilmstrip = shown\nfilmstrip_marked = all\n\
             preview_hidden = false\nkey_action_width = 220\neditor_content = open\n\
             editor_creator_rights = open\n\
             editor_time_place = open\neditor_assignment = open\n\
             editor_rating_label = open\neditor_licence = open\nlibrary_folders = open\n\
             library_favourites = open\nlibrary_recent = open\nlibrary_tasks = open\n\
             sort = name\n\
             sort_descending = false\npreview_histogram = open\ninspector_histogram = open\n\
             clip_highlights = false\nclip_shadows = false\nfocus_marking = false\n\
             preview_clipping = open\n\
             compare_above_below = false\ncompare_locked = true\n\
             import_incremental = true\n\
             move_to_subfolders = false\ncompanion_suffixes = dop, pp3, on1\n\
             thumbnail_buttons = true\n"
        );
    }

    #[test]
    fn comments_empty_lines_and_spaces_are_allowed() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\r\n\r\n   \n  # another comment\n\tcreate_iim\t=   true  \n\n  \
             fallback_encoding=iso-8859-1\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings,
            Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn unknown_key_gives_one_problem_and_the_other_keys_are_read() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ncreate_iim = true\ncolour = blue\nfallback_encoding = iso-8859-1\n",
        );
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 3);
        assert!(problems[0].contains("colour"), "{problems:?}");
        assert_eq!(
            settings,
            Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn bad_create_iim_value_keeps_its_default_and_the_other_key_is_read() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = yes\nfallback_encoding = iso-8859-1\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 2);
        assert!(problems[0].contains("yes"), "{problems:?}");
        assert!(!settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Latin1);
    }

    #[test]
    fn bad_fallback_value_keeps_its_default_and_the_other_key_is_read() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\n\ncreate_iim = true\nfallback_encoding = utf-16\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 4);
        assert!(problems[0].contains("utf-16"), "{problems:?}");
        assert!(settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Windows1252);
    }

    #[test]
    fn line_without_equals_gives_one_problem_and_the_other_key_is_read() {
        let (_dir, path) = TempDir::with_file("create_iim true\nfallback_encoding = iso-8859-1\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 1);
        assert!(!settings.create_iim);
        assert_eq!(settings.fallback, Fallback::Latin1);
    }

    #[test]
    fn byte_order_mark_is_not_part_of_the_first_key() {
        let (_dir, path) = TempDir::with_file("\u{feff}create_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.create_iim);
    }

    #[test]
    fn file_that_cannot_be_read_gives_defaults_and_one_problem() {
        // A folder where the file should be: it exists but cannot be read as text.
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        std::fs::create_dir_all(&path).unwrap();
        let (settings, problems) = load_settings(&path);
        assert_eq!(settings, Settings::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn save_creates_a_missing_folder() {
        let dir = TempDir::new();
        let path = dir.0.join("RawStride").join("settings.txt");
        assert!(!dir.0.exists());
        save_settings(&path, &Settings::default()).unwrap();
        assert!(path.is_file());
        assert_eq!(load_settings(&path), (Settings::default(), Vec::new()));
    }

    #[test]
    fn save_leaves_no_temporary_file() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        save_settings(&path, &Settings::default()).unwrap();
        save_settings(
            &path,
            &Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
                ..Settings::default()
            },
        )
        .unwrap();
        assert_eq!(file_names(&dir.0), vec!["settings.txt".to_string()]);
    }

    #[test]
    fn edit_options_carry_both_settings() {
        assert_eq!(Settings::default().edit_options(), EditOptions::default());
        let settings = Settings {
            create_iim: true,
            fallback: Fallback::Latin1,
            ..Settings::default()
        };
        assert_eq!(
            settings.edit_options(),
            EditOptions {
                settings: WriteSettings { create_iim: true },
                fallback: Fallback::Latin1,
            }
        );
    }

    #[test]
    fn settings_path_is_in_the_rawstride_folder_of_local_app_data() {
        // Read only: the environment is not changed (other tests run at the same time).
        match std::env::var_os("LOCALAPPDATA").filter(|data| !data.is_empty()) {
            Some(data) => assert_eq!(
                settings_path(),
                Some(PathBuf::from(data).join("RawStride").join("settings.txt"))
            ),
            None => assert_eq!(settings_path(), None),
        }
    }

    fn changed_shell() -> ShellSettings {
        ShellSettings {
            theme: Theme::Light,
            window: Some(WindowRect {
                x: -8,
                y: 40,
                width: 1600,
                height: 900,
                scale: 1.25,
            }),
            window_maximized: true,
            fullscreen: true,
            left_column_width: 210,
            right_column_width: 355,
            left_column: ColumnState::Collapsed,
            right_column: ColumnState::Collapsed,
            columns_hidden: true,
            inspector_info: ColumnState::Collapsed,
            inspector_metadata: ColumnState::Collapsed,
            preview_column: ColumnState::Collapsed,
            preview_column_width: 420,
            preview_info: ColumnState::Collapsed,
            preview_zoom: ColumnState::Collapsed,
            filmstrip: false,
            filmstrip_marked: true,
            preview_hidden: true,
            key_action_width: 300,
            editor_content: ColumnState::Collapsed,
            editor_creator_rights: ColumnState::Collapsed,
            editor_time_place: ColumnState::Collapsed,
            editor_assignment: ColumnState::Collapsed,
            editor_rating_label: ColumnState::Collapsed,
            editor_licence: ColumnState::Collapsed,
            library_folders: ColumnState::Collapsed,
            library_favourites: ColumnState::Collapsed,
            library_recent: ColumnState::Collapsed,
            library_tasks: ColumnState::Collapsed,
            sort: SortOrder {
                key: SortKey::Capture,
                descending: true,
            },
            preview_histogram: ColumnState::Collapsed,
            inspector_histogram: ColumnState::Collapsed,
            clip_highlights: true,
            clip_shadows: true,
            focus_marking: true,
            preview_clipping: ColumnState::Collapsed,
            compare_above_below: true,
            compare_locked: false,
            import_incremental: false,
            move_to_subfolders: true,
            companion_suffixes: vec!["xmp2".to_string(), "dop".to_string()],
            thumbnail_buttons: false,
        }
    }

    #[test]
    fn shell_defaults_are_dark_left_column_open_and_right_collapsed() {
        let shell = Settings::default().shell;
        assert_eq!(shell.theme, Theme::Dark);
        assert_eq!(shell.window, None);
        assert!(!shell.window_maximized);
        assert!(!shell.fullscreen);
        assert_eq!(shell.left_column_width, 240);
        assert_eq!(shell.right_column_width, 300);
        assert_eq!(shell.left_column, ColumnState::Open);
        assert_eq!(shell.right_column, ColumnState::Collapsed);
        assert!(!shell.columns_hidden);
        assert_eq!(shell.inspector_info, ColumnState::Open);
        assert_eq!(shell.inspector_metadata, ColumnState::Open);
    }

    #[test]
    fn the_inspector_sections_round_trip_one_at_a_time() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for (info, metadata) in [
            (ColumnState::Collapsed, ColumnState::Open),
            (ColumnState::Open, ColumnState::Collapsed),
        ] {
            let settings = Settings {
                shell: ShellSettings {
                    inspector_info: info,
                    inspector_metadata: metadata,
                    ..ShellSettings::default()
                },
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
    }

    #[test]
    fn a_file_without_the_inspector_keys_gives_both_sections_open_and_no_problem() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ncreate_iim = false\ntheme = light\nright_column = open\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell.inspector_info, ColumnState::Open);
        assert_eq!(settings.shell.inspector_metadata, ColumnState::Open);
        assert_eq!(settings.shell.right_column, ColumnState::Open);
    }

    #[test]
    fn a_bad_inspector_value_keeps_open_and_names_the_line() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ninspector_info = collapsed\ninspector_metadata = closed\n",
        );
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 3);
        assert!(problems[0].contains("inspector_metadata"), "{problems:?}");
        assert_eq!(settings.shell.inspector_info, ColumnState::Collapsed);
        assert_eq!(settings.shell.inspector_metadata, ColumnState::Open);
    }

    #[test]
    fn every_shell_key_round_trips_through_save_and_load() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for shell in [ShellSettings::default(), changed_shell()] {
            let settings = Settings {
                create_iim: true,
                fallback: Fallback::Latin1,
                shell,
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("window = -8,40,1600,900,1.25\n")
        );
    }

    #[test]
    fn a_file_with_only_the_two_metadata_keys_gives_the_shell_defaults_and_no_problem() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = true\nfallback_encoding = iso-8859-1\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell, ShellSettings::default());
        assert!(settings.create_iim);
    }

    #[test]
    fn a_bad_value_of_each_shell_key_keeps_its_default_and_names_the_line() {
        let bad = [
            ("theme", "blue"),
            ("window", "1,2,3,4"),
            ("window", "1,2,3,4,0"),
            ("window", "1,2,-3,4,1"),
            ("window", "a,2,3,4,1"),
            ("window_maximized", "yes"),
            ("fullscreen", "1"),
            ("left_column_width", "-5"),
            ("right_column_width", "wide"),
            ("key_action_width", "wide"),
            ("key_action_width", "-5"),
            ("left_column", "hidden"),
            ("right_column", ""),
            ("columns_hidden", "no"),
            ("inspector_info", "shut"),
            ("inspector_metadata", "true"),
        ];
        for (key, value) in bad {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\n{key} = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key} = {value}");
            assert!(settings.create_iim, "{key} = {value}");
        }
    }

    #[test]
    fn the_preview_keys_default_to_an_open_column_and_a_shown_filmstrip_of_all() {
        let shell = ShellSettings::default();
        assert_eq!(shell.preview_column, ColumnState::Open);
        assert_eq!(shell.preview_column_width, 300);
        assert_eq!(shell.preview_info, ColumnState::Open);
        assert_eq!(shell.preview_zoom, ColumnState::Open);
        assert!(shell.filmstrip);
        assert!(!shell.filmstrip_marked);
        assert!(!shell.preview_hidden);
        // The main window's right column still starts collapsed; the two are apart.
        assert_eq!(shell.right_column, ColumnState::Collapsed);
    }

    #[test]
    fn the_preview_keys_round_trip_one_at_a_time() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let base = ShellSettings::default();
        for shell in [
            ShellSettings {
                preview_column: ColumnState::Collapsed,
                ..base.clone()
            },
            ShellSettings {
                preview_column_width: 512,
                ..base.clone()
            },
            ShellSettings {
                preview_info: ColumnState::Collapsed,
                ..base.clone()
            },
            ShellSettings {
                preview_zoom: ColumnState::Collapsed,
                ..base.clone()
            },
            ShellSettings {
                filmstrip: false,
                ..base.clone()
            },
            ShellSettings {
                filmstrip_marked: true,
                ..base.clone()
            },
            ShellSettings {
                preview_hidden: true,
                ..base.clone()
            },
        ] {
            let settings = Settings {
                shell,
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("preview_hidden = true\n"), "{text}");
        assert!(text.contains("filmstrip = shown\n"), "{text}");
        assert!(text.contains("filmstrip_marked = all\n"), "{text}");
    }

    #[test]
    fn the_preview_keys_are_read_from_their_file_values() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\npreview_column = collapsed\npreview_column_width = 260\n\
             preview_info = collapsed\npreview_zoom = collapsed\nfilmstrip = hidden\n\
             filmstrip_marked = marked\npreview_hidden = true\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        let shell = settings.shell;
        assert_eq!(shell.preview_column, ColumnState::Collapsed);
        assert_eq!(shell.preview_column_width, 260);
        assert_eq!(shell.preview_info, ColumnState::Collapsed);
        assert_eq!(shell.preview_zoom, ColumnState::Collapsed);
        assert!(!shell.filmstrip);
        assert!(shell.filmstrip_marked);
        assert!(shell.preview_hidden);
    }

    /// The Action column's width on the page Keyboard Shortcuts (ADR 0017 U8-Q15, T286): read
    /// from its line; 220 without one.
    #[test]
    fn the_key_action_width_is_read_and_is_220_without_a_line() {
        let (_dir, path) = TempDir::with_file("# RawStride\nkey_action_width = 300\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell.key_action_width, 300);

        let (_dir, path) = TempDir::with_file("# RawStride\ncreate_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell.key_action_width, 220);
    }

    #[test]
    fn a_file_without_the_preview_keys_gives_their_defaults_and_no_problem() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ncreate_iim = true\ntheme = light\nright_column = open\n\
             inspector_info = collapsed\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        let defaults = ShellSettings::default();
        let shell = settings.shell;
        assert_eq!(shell.preview_column, defaults.preview_column);
        assert_eq!(shell.preview_column_width, defaults.preview_column_width);
        assert_eq!(shell.preview_info, defaults.preview_info);
        assert_eq!(shell.preview_zoom, defaults.preview_zoom);
        assert_eq!(shell.filmstrip, defaults.filmstrip);
        assert_eq!(shell.filmstrip_marked, defaults.filmstrip_marked);
        assert_eq!(shell.preview_hidden, defaults.preview_hidden);
    }

    #[test]
    fn a_bad_value_of_each_preview_key_keeps_its_default_and_names_the_line() {
        let bad = [
            ("preview_column", "hidden"),
            ("preview_column_width", "wide"),
            ("preview_column_width", "-1"),
            ("preview_info", "shut"),
            ("preview_zoom", "true"),
            ("filmstrip", "open"),
            ("filmstrip", "true"),
            ("filmstrip_marked", "some"),
            ("preview_hidden", "yes"),
        ];
        for (key, value) in bad {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\n{key} = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key} = {value}");
            assert!(settings.create_iim, "{key} = {value}");
        }
    }

    // --- the metadata window's sections (T123, U10-Q6) ---

    #[test]
    fn the_editor_sections_are_open_by_default() {
        let shell = ShellSettings::default();
        for section in EditorSection::ALL {
            assert_eq!(
                shell.editor_section(section),
                ColumnState::Open,
                "{section:?}"
            );
        }
    }

    #[test]
    fn each_editor_section_toggles_alone_and_round_trips_through_its_key() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let keys = [
            (EditorSection::Content, "editor_content"),
            (EditorSection::CreatorRights, "editor_creator_rights"),
            (EditorSection::TimePlace, "editor_time_place"),
            (EditorSection::Assignment, "editor_assignment"),
            (EditorSection::Licence, "editor_licence"),
            (EditorSection::RatingLabel, "editor_rating_label"),
        ];
        for (section, key) in keys {
            let mut shell = ShellSettings::default();
            shell.toggle_editor_section(section);
            for other in EditorSection::ALL {
                let expected = if other == section {
                    ColumnState::Collapsed
                } else {
                    ColumnState::Open
                };
                assert_eq!(
                    shell.editor_section(other),
                    expected,
                    "{section:?}/{other:?}"
                );
            }
            let settings = Settings {
                shell: shell.clone(),
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains(&format!("{key} = collapsed\n")), "{text}");
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
            // Toggled again: open.
            shell.toggle_editor_section(section);
            assert_eq!(shell.editor_section(section), ColumnState::Open);
        }
    }

    #[test]
    fn the_editor_section_keys_are_read_and_a_missing_key_is_open() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\neditor_content = collapsed\neditor_time_place = collapsed\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        let shell = settings.shell;
        assert_eq!(shell.editor_content, ColumnState::Collapsed);
        assert_eq!(shell.editor_creator_rights, ColumnState::Open);
        assert_eq!(shell.editor_time_place, ColumnState::Collapsed);
        assert_eq!(shell.editor_assignment, ColumnState::Open);
        assert_eq!(shell.editor_rating_label, ColumnState::Open);
        assert_eq!(shell.editor_licence, ColumnState::Open);
    }

    /// The Licence (PLUS) section's key (T128, U10d): read, written, and open when missing.
    #[test]
    fn the_licence_section_key_is_read_and_written_and_a_missing_key_is_open() {
        let (_dir, path) = TempDir::with_file("# RawStride\neditor_licence = collapsed\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell.editor_section(EditorSection::Licence),
            ColumnState::Collapsed
        );
        save_settings(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("editor_licence = collapsed\n"), "{text}");
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded, settings);

        // A file from before T128, without the key: open.
        let (_dir, path) = TempDir::with_file("# RawStride\neditor_assignment = collapsed\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell.editor_section(EditorSection::Licence),
            ColumnState::Open
        );
    }

    #[test]
    fn the_rating_and_label_section_key_is_read_and_written() {
        let (_dir, path) = TempDir::with_file("# RawStride\neditor_rating_label = collapsed\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell.editor_section(EditorSection::RatingLabel),
            ColumnState::Collapsed
        );
        save_settings(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("editor_rating_label = collapsed\n"), "{text}");
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded, settings);
    }

    #[test]
    fn a_bad_value_of_each_editor_section_key_keeps_open_and_names_the_line() {
        for key in [
            "editor_content",
            "editor_creator_rights",
            "editor_time_place",
            "editor_assignment",
            "editor_rating_label",
            "editor_licence",
        ] {
            let (_dir, path) =
                TempDir::with_file(&format!("# RawStride\ncreate_iim = true\n{key} = closed\n"));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key}");
        }
    }

    // --- The library (T135, U4-Q3, U4-Q4, U4-Q7) ---

    /// A full path that need not exist.
    fn full(name: &str) -> PathBuf {
        std::env::temp_dir().join("pw-loupe-library").join(name)
    }

    #[test]
    fn the_library_sections_are_open_by_default_and_round_trip_one_at_a_time() {
        let shell = ShellSettings::default();
        assert_eq!(shell.library_folders, ColumnState::Open);
        assert_eq!(shell.library_favourites, ColumnState::Open);
        assert_eq!(shell.library_recent, ColumnState::Open);
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for (key, shell) in [
            (
                "library_folders",
                ShellSettings {
                    library_folders: ColumnState::Collapsed,
                    ..ShellSettings::default()
                },
            ),
            (
                "library_favourites",
                ShellSettings {
                    library_favourites: ColumnState::Collapsed,
                    ..ShellSettings::default()
                },
            ),
            (
                "library_recent",
                ShellSettings {
                    library_recent: ColumnState::Collapsed,
                    ..ShellSettings::default()
                },
            ),
        ] {
            let settings = Settings {
                shell,
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains(&format!("{key} = collapsed\n")), "{text}");
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings, "{key}");
        }
    }

    #[test]
    fn a_bad_value_of_each_library_section_key_keeps_open_and_names_the_line() {
        for key in ["library_folders", "library_favourites", "library_recent"] {
            let (_dir, path) =
                TempDir::with_file(&format!("# RawStride\ncreate_iim = true\n{key} = closed\n"));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key}");
        }
    }

    #[test]
    fn the_tasks_section_is_open_by_default_and_its_state_is_saved_and_loaded() {
        assert_eq!(ShellSettings::default().library_tasks, ColumnState::Open);
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let settings = Settings {
            shell: ShellSettings {
                library_tasks: ColumnState::Collapsed,
                ..ShellSettings::default()
            },
            ..Settings::default()
        };
        save_settings(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("library_tasks = collapsed\n"), "{text}");
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded, settings);
    }

    #[test]
    fn an_unknown_value_of_the_tasks_section_is_warned_and_ignored() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = true\nlibrary_tasks = hidden\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 3);
        assert!(problems[0].contains("library_tasks"), "{problems:?}");
        assert_eq!(settings.shell.library_tasks, ColumnState::Open);
        assert_eq!(settings.shell, ShellSettings::default());
    }

    #[test]
    fn favourites_and_recent_folders_are_saved_and_loaded_in_their_order() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let library = SavedLibrary {
            favourites: vec![full("Zebra"), full("alpha"), full("Mitt arkiv 2026")],
            recent: vec![full("Last"), full("Zebra"), full("First")],
        };
        let settings = Settings {
            create_iim: true,
            ..Settings::default()
        };
        save_settings_with_library(&path, &settings, &library).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let expected_lines = format!(
            "favourite = {}\nfavourite = {}\nfavourite = {}\nrecent = {}\nrecent = {}\n\
             recent = {}\n",
            full("Zebra").display(),
            full("alpha").display(),
            full("Mitt arkiv 2026").display(),
            full("Last").display(),
            full("Zebra").display(),
            full("First").display()
        );
        assert!(text.ends_with(&expected_lines), "{text}");
        let (loaded, loaded_library, problems) = load_settings_and_library(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded, settings);
        assert_eq!(loaded_library, library);
        // `load_settings` reads the same file and leaves the lists out.
        let (only_settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(only_settings, settings);
    }

    #[test]
    fn a_file_without_library_lines_gives_empty_lists_and_save_settings_writes_none() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        save_settings(&path, &Settings::default()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text
                .lines()
                .any(|line| line.starts_with("favourite =") || line.starts_with("recent =")),
            "{text}"
        );
        let (_, library, problems) = load_settings_and_library(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(library, SavedLibrary::default());
    }

    #[test]
    fn an_unreadable_favourite_or_recent_line_is_skipped_with_a_problem_naming_the_line() {
        let good = full("Good");
        let (_dir, path) = TempDir::with_file(&format!(
            "# RawStride\nfavourite = {}\nfavourite = relative\\folder\nfavourite =\n\
             recent = not full\nrecent = {}\n",
            good.display(),
            good.display()
        ));
        let (settings, library, problems) = load_settings_and_library(&path);
        assert_eq!(problems.len(), 3, "{problems:?}");
        for (problem, line) in problems.iter().zip([3, 4, 5]) {
            assert!(problem.contains(&format!("line {line}:")), "{problem}");
            assert!(problem.contains("line ignored"), "{problem}");
        }
        assert!(problems[0].contains("favourite"), "{problems:?}");
        assert!(problems[2].contains("recent"), "{problems:?}");
        assert_eq!(library.favourites, vec![good.clone()]);
        assert_eq!(library.recent, vec![good]);
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn a_path_that_could_not_be_read_back_is_left_out_when_saving() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let kept = full("Kept");
        let library = SavedLibrary {
            favourites: vec![
                full("line\nbreak"),
                kept.clone(),
                PathBuf::from("relative"),
                // A space inside a name is kept; only one at the end of the line would be lost.
                full("with space inside"),
            ],
            recent: vec![full("trailing space ")],
        };
        save_settings_with_library(&path, &Settings::default(), &library).unwrap();
        let (_, loaded, problems) = load_settings_and_library(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded.favourites, vec![kept, full("with space inside")]);
        assert!(loaded.recent.is_empty(), "{:?}", loaded.recent);
    }

    #[test]
    fn the_sort_order_is_file_name_ascending_by_default_and_round_trips_for_every_key() {
        assert_eq!(
            Settings::default().shell.sort,
            SortOrder {
                key: SortKey::Name,
                descending: false
            }
        );
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for (key, value) in SORT_KEYS
            .into_iter()
            .zip(["name", "capture", "modified", "type", "rating"])
        {
            for descending in [false, true] {
                let settings = Settings {
                    shell: ShellSettings {
                        sort: SortOrder { key, descending },
                        ..ShellSettings::default()
                    },
                    ..Settings::default()
                };
                save_settings(&path, &settings).unwrap();
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(text.contains(&format!("\nsort = {value}\n")), "{text}");
                assert!(
                    text.contains(&format!("\nsort_descending = {descending}\n")),
                    "{text}"
                );
                let (loaded, problems) = load_settings(&path);
                assert!(problems.is_empty(), "{problems:?}");
                assert_eq!(loaded, settings);
            }
        }
    }

    #[test]
    fn the_sort_keys_are_read_and_a_file_without_them_gives_the_default() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\nsort = rating\nsort_descending = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell.sort,
            SortOrder {
                key: SortKey::Rating,
                descending: true
            }
        );
        let (_dir, path) = TempDir::with_file("# RawStride\ntheme = light\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell.sort, SortOrder::default());
    }

    #[test]
    fn an_unknown_sort_value_is_warned_about_and_ignored() {
        let (_dir, path) = TempDir::with_file("# RawStride\nsort = date\nsort_descending = true\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 2);
        assert!(problems[0].contains("\"date\" for sort"), "{problems:?}");
        assert_eq!(settings.shell.sort.key, SortKey::Name);
        assert!(settings.shell.sort.descending);

        let (_dir, path) = TempDir::with_file("# RawStride\nsort = type\nsort_descending = yes\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 3);
        assert!(problems[0].contains("sort_descending"), "{problems:?}");
        assert_eq!(
            settings.shell.sort,
            SortOrder {
                key: SortKey::Type,
                descending: false
            }
        );
    }

    // ---------- the Histogram sections (ADR 0017 §2.6, U7-Q7, T142) ----------

    #[test]
    fn the_histogram_sections_default_to_open() {
        let shell = ShellSettings::default();
        assert_eq!(shell.preview_histogram, ColumnState::Open);
        assert_eq!(shell.inspector_histogram, ColumnState::Open);
    }

    #[test]
    fn the_histogram_sections_round_trip_one_at_a_time() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let base = ShellSettings::default();
        for (shell, preview, inspector) in [
            (
                ShellSettings {
                    preview_histogram: ColumnState::Collapsed,
                    ..base.clone()
                },
                "collapsed",
                "open",
            ),
            (
                ShellSettings {
                    inspector_histogram: ColumnState::Collapsed,
                    ..base.clone()
                },
                "open",
                "collapsed",
            ),
            (base, "open", "open"),
        ] {
            let settings = Settings {
                shell,
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.contains(&format!("\npreview_histogram = {preview}\n")),
                "{text}"
            );
            assert!(
                text.contains(&format!("\ninspector_histogram = {inspector}\n")),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
    }

    #[test]
    fn the_histogram_sections_are_read_from_their_file_values() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\npreview_histogram = collapsed\ninspector_histogram = collapsed\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell.preview_histogram, ColumnState::Collapsed);
        assert_eq!(settings.shell.inspector_histogram, ColumnState::Collapsed);
        // only the two sections changed
        assert_eq!(
            ShellSettings {
                preview_histogram: ColumnState::Open,
                inspector_histogram: ColumnState::Open,
                ..settings.shell
            },
            ShellSettings::default()
        );
    }

    #[test]
    fn a_file_without_the_histogram_sections_gives_their_defaults_and_no_problem() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ncreate_iim = true\npreview_info = collapsed\ninspector_info = collapsed\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.shell.preview_histogram, ColumnState::Open);
        assert_eq!(settings.shell.inspector_histogram, ColumnState::Open);
    }

    #[test]
    fn a_bad_value_of_a_histogram_section_keeps_its_default_and_names_the_line() {
        for (key, value) in [
            ("preview_histogram", "hidden"),
            ("preview_histogram", "true"),
            ("inspector_histogram", "shut"),
            ("inspector_histogram", ""),
        ] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\n{key} = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key} = {value}");
            assert!(settings.create_iim, "{key} = {value}");
        }
    }

    // ---------- the clipping overlays and the Clipping section (ADR 0017 §2.6, U7b, T148) ----------

    #[test]
    fn the_clipping_overlays_default_to_off_and_the_clipping_section_to_open() {
        let shell = ShellSettings::default();
        assert!(!shell.clip_highlights);
        assert!(!shell.clip_shadows);
        assert_eq!(shell.preview_clipping, ColumnState::Open);
    }

    #[test]
    fn the_clipping_keys_round_trip_one_at_a_time() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let base = ShellSettings::default();
        for (shell, highlights, shadows, section) in [
            (
                ShellSettings {
                    clip_highlights: true,
                    ..base.clone()
                },
                "true",
                "false",
                "open",
            ),
            (
                ShellSettings {
                    clip_shadows: true,
                    ..base.clone()
                },
                "false",
                "true",
                "open",
            ),
            (
                ShellSettings {
                    preview_clipping: ColumnState::Collapsed,
                    ..base.clone()
                },
                "false",
                "false",
                "collapsed",
            ),
            (base, "false", "false", "open"),
        ] {
            let settings = Settings {
                shell,
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.contains(&format!("\nclip_highlights = {highlights}\n")),
                "{text}"
            );
            assert!(
                text.contains(&format!("\nclip_shadows = {shadows}\n")),
                "{text}"
            );
            assert!(
                text.contains(&format!("\npreview_clipping = {section}\n")),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
    }

    #[test]
    fn the_clipping_keys_are_read_from_their_file_values() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\nclip_highlights = true\nclip_shadows = true\n\
             preview_clipping = collapsed\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.shell.clip_highlights);
        assert!(settings.shell.clip_shadows);
        assert_eq!(settings.shell.preview_clipping, ColumnState::Collapsed);
        // only the three changed
        assert_eq!(
            ShellSettings {
                clip_highlights: false,
                clip_shadows: false,
                preview_clipping: ColumnState::Open,
                ..settings.shell
            },
            ShellSettings::default()
        );
    }

    #[test]
    fn a_file_without_the_clipping_keys_gives_their_defaults_and_no_problem() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = true\npreview_histogram = collapsed\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!settings.shell.clip_highlights);
        assert!(!settings.shell.clip_shadows);
        assert_eq!(settings.shell.preview_clipping, ColumnState::Open);
    }

    #[test]
    fn a_bad_value_of_a_clipping_key_keeps_its_default_and_names_the_line() {
        for (key, value) in [
            ("clip_highlights", "on"),
            ("clip_highlights", "open"),
            ("clip_shadows", "1"),
            ("clip_shadows", ""),
            ("preview_clipping", "true"),
            ("preview_clipping", "hidden"),
        ] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\n{key} = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key} = {value}");
            assert!(settings.create_iim, "{key} = {value}");
        }
    }

    // ---------- the focus marking (ADR 0017 §2.6, U9-Q1, T168) ----------

    #[test]
    fn the_focus_marking_defaults_to_off() {
        assert!(!ShellSettings::default().focus_marking);
    }

    #[test]
    fn the_focus_marking_is_written_and_read() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for on in [true, false] {
            let settings = Settings {
                shell: ShellSettings {
                    focus_marking: on,
                    ..ShellSettings::default()
                },
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.contains(&format!("\nfocus_marking = {on}\n")),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
        // read from the file's value; nothing else changes
        let (_dir, path) = TempDir::with_file("# RawStride\nfocus_marking = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell,
            ShellSettings {
                focus_marking: true,
                ..ShellSettings::default()
            }
        );
    }

    #[test]
    fn a_file_without_the_focus_marking_gives_off_and_no_problem() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = true\nclip_highlights = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!settings.shell.focus_marking);
    }

    #[test]
    fn a_bad_value_of_the_focus_marking_keeps_off_and_names_the_line() {
        for value in ["on", "open", "1", ""] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\nfocus_marking = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains("focus_marking"), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{value}");
            assert!(settings.create_iim, "{value}");
        }
    }

    // ---------- the buttons on a thumbnail (ADR 0017 §2.8, U15-Q7, T245) ----------

    #[test]
    fn the_thumbnail_buttons_default_to_on() {
        assert!(ShellSettings::default().thumbnail_buttons);
    }

    #[test]
    fn the_thumbnail_buttons_are_written_and_read() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for on in [true, false] {
            let settings = Settings {
                shell: ShellSettings {
                    thumbnail_buttons: on,
                    ..ShellSettings::default()
                },
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.ends_with(&format!("\nthumbnail_buttons = {on}\n")),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
        // read from the file's value; nothing else changes
        let (_dir, path) = TempDir::with_file("# RawStride\nthumbnail_buttons = false\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell,
            ShellSettings {
                thumbnail_buttons: false,
                ..ShellSettings::default()
            }
        );
    }

    #[test]
    fn a_file_without_the_thumbnail_buttons_gives_on_and_no_problem() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = true\nfocus_marking = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.shell.thumbnail_buttons);
    }

    #[test]
    fn a_bad_value_of_the_thumbnail_buttons_keeps_on_and_names_the_line() {
        for value in ["on", "off", "shown", "0", ""] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\nthumbnail_buttons = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains("thumbnail_buttons"), "{problems:?}");
            assert!(settings.shell.thumbnail_buttons, "{value}");
            assert_eq!(settings.shell, ShellSettings::default(), "{value}");
            assert!(settings.create_iim, "{value}");
        }
    }

    // ---------- the compare's layout and lock (ADR 0017 §2.2, U6-Q7, U6-Q8, T151) ----------

    #[test]
    fn the_compare_defaults_to_side_by_side_and_locked() {
        let shell = ShellSettings::default();
        assert!(!shell.compare_above_below);
        assert!(shell.compare_locked);
    }

    #[test]
    fn the_compare_keys_round_trip_one_at_a_time() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let base = ShellSettings::default();
        for (shell, above_below, locked) in [
            (
                ShellSettings {
                    compare_above_below: true,
                    ..base.clone()
                },
                "true",
                "true",
            ),
            (
                ShellSettings {
                    compare_locked: false,
                    ..base.clone()
                },
                "false",
                "false",
            ),
            (base, "false", "true"),
        ] {
            let settings = Settings {
                shell,
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.contains(&format!("\ncompare_above_below = {above_below}\n")),
                "{text}"
            );
            assert!(
                text.contains(&format!("\ncompare_locked = {locked}\n")),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
    }

    #[test]
    fn the_compare_keys_are_read_from_their_file_values() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncompare_above_below = true\ncompare_locked = false\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.shell.compare_above_below);
        assert!(!settings.shell.compare_locked);
        // only the two changed
        assert_eq!(
            ShellSettings {
                compare_above_below: false,
                compare_locked: true,
                ..settings.shell
            },
            ShellSettings::default()
        );
    }

    #[test]
    fn a_file_without_the_compare_keys_gives_their_defaults_and_no_problem() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\ncreate_iim = true\nclip_highlights = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!settings.shell.compare_above_below);
        assert!(settings.shell.compare_locked);
    }

    #[test]
    fn a_bad_value_of_a_compare_key_keeps_its_default_and_names_the_line() {
        for (key, value) in [
            ("compare_above_below", "yes"),
            ("compare_above_below", "above"),
            ("compare_locked", "0"),
            ("compare_locked", ""),
            ("compare_locked", "unlocked"),
        ] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncreate_iim = true\n{key} = {value}\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 3);
            assert!(problems[0].contains(key), "{problems:?}");
            assert!(problems[0].contains("true or false"), "{problems:?}");
            assert_eq!(settings.shell, ShellSettings::default(), "{key} = {value}");
            assert!(settings.create_iim, "{key} = {value}");
        }
    }

    // ---------- the import's "copy new images only" (ADR 0005 choice 6, T158) ----------

    #[test]
    fn import_incremental_defaults_to_on() {
        assert!(ShellSettings::default().import_incremental);
    }

    #[test]
    fn import_incremental_is_written_and_read() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for on in [false, true] {
            let settings = Settings {
                shell: ShellSettings {
                    import_incremental: on,
                    ..ShellSettings::default()
                },
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.contains(&format!("\nimport_incremental = {on}\n")),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
        let (_dir, path) = TempDir::with_file("# RawStride\nimport_incremental = false\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!settings.shell.import_incremental);
    }

    #[test]
    fn a_file_without_import_incremental_gives_true_and_no_problem() {
        let (_dir, path) = TempDir::with_file("# RawStride\ncreate_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.shell.import_incremental);
    }

    fn default_suffixes() -> Vec<String> {
        vec!["dop".to_string(), "pp3".to_string(), "on1".to_string()]
    }

    #[test]
    fn the_move_settings_default_to_off_and_dop_pp3_on1() {
        let shell = ShellSettings::default();
        assert!(!shell.move_to_subfolders);
        assert_eq!(shell.companion_suffixes, default_suffixes());
    }

    #[test]
    fn the_move_settings_are_written_and_read() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for (on, suffixes, line) in [
            (
                false,
                default_suffixes(),
                "companion_suffixes = dop, pp3, on1",
            ),
            (true, vec!["xmp2".to_string()], "companion_suffixes = xmp2"),
            (
                true,
                vec!["on1".to_string(), "dop".to_string()],
                "companion_suffixes = on1, dop",
            ),
        ] {
            let settings = Settings {
                shell: ShellSettings {
                    move_to_subfolders: on,
                    companion_suffixes: suffixes,
                    ..ShellSettings::default()
                },
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            // The two lines come last but `thumbnail_buttons` (T245), `move_to_subfolders` first.
            assert!(
                text.ends_with(&format!(
                    "\nimport_incremental = true\nmove_to_subfolders = {on}\n{line}\n\
                     thumbnail_buttons = true\n"
                )),
                "{text}"
            );
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded, settings);
        }
    }

    #[test]
    fn the_move_settings_are_read_from_a_written_file() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\nmove_to_subfolders = true\ncompanion_suffixes =  DOP ,Xmp2,on1  \n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.shell.move_to_subfolders);
        assert_eq!(
            settings.shell.companion_suffixes,
            vec!["dop".to_string(), "xmp2".to_string(), "on1".to_string()]
        );
    }

    #[test]
    fn a_file_without_the_move_keys_gives_the_defaults_and_no_problem() {
        let (_dir, path) = TempDir::with_file("# RawStride\ncreate_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!settings.shell.move_to_subfolders);
        assert_eq!(settings.shell.companion_suffixes, default_suffixes());
    }

    #[test]
    fn a_bad_move_to_subfolders_value_keeps_its_default_with_one_problem() {
        let (_dir, path) =
            TempDir::with_file("# RawStride\nmove_to_subfolders = yes\ncreate_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 2);
        assert!(problems[0].contains("yes"), "{problems:?}");
        assert!(!settings.shell.move_to_subfolders);
        assert!(settings.create_iim);
    }

    #[test]
    fn bad_companion_suffix_entries_are_dropped_without_a_problem() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\ncompanion_suffixes = dop, , .xmp, a/b, a\\b, c:d, two words, PP3,\n",
        );
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.shell.companion_suffixes,
            vec!["dop".to_string(), "pp3".to_string()]
        );
    }

    #[test]
    fn a_companion_suffixes_line_without_a_valid_entry_gives_the_default() {
        for value in ["", " , ", ".dop", "a/b, c:d, x y, a\\b"] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\ncompanion_suffixes = {value}\nmove_to_subfolders = true\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{value:?}: {problems:?}");
            assert_eq!(
                settings.shell.companion_suffixes,
                default_suffixes(),
                "{value:?}"
            );
            assert!(settings.shell.move_to_subfolders, "{value:?}");
        }
    }

    // --- The external programs (ADR 0019 E1, E-Q1, E-Q2, T200) ---

    /// A program with this name and `path`.
    fn program(name: &str, path: &str) -> ExternalProgram {
        ExternalProgram {
            name: name.to_string(),
            path: PathBuf::from(path),
        }
    }

    /// Five programs with spaces inside their names and paths.
    fn five_programs() -> Vec<ExternalProgram> {
        vec![
            program(
                "Capture One",
                r"C:\Program Files\Capture One\CaptureOne.exe",
            ),
            program(
                "Photo Editor 2",
                r"C:\Program Files\Photo Editor\editor.exe",
            ),
            program("Raw Developer", r"D:\Apps\Raw Developer 8\RawDeveloper.exe"),
            program("Viewer", r"C:\Tools\viewer.exe"),
            program("Last one", r"E:\Portable apps\last one.exe"),
        ]
    }

    #[test]
    fn the_external_programs_are_empty_by_default_and_without_lines() {
        assert!(Settings::default().external_programs.is_empty());
        let (_dir, path) = TempDir::with_file("# RawStride\ncreate_iim = true\n");
        let (settings, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(settings.external_programs.is_empty());
    }

    #[test]
    fn one_and_five_external_programs_round_trip_in_their_order() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        for programs in [five_programs()[..1].to_vec(), five_programs()] {
            let settings = Settings {
                external_programs: programs.clone(),
                ..Settings::default()
            };
            save_settings(&path, &settings).unwrap();
            let (loaded, problems) = load_settings(&path);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(loaded.external_programs, programs);
            assert_eq!(loaded, settings);
        }
    }

    #[test]
    fn the_external_program_line_is_after_the_shell_lines_and_before_the_library() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let settings = Settings {
            external_programs: five_programs()[..1].to_vec(),
            ..Settings::default()
        };
        let library = SavedLibrary {
            favourites: vec![full("Favourite")],
            recent: Vec::new(),
        };
        save_settings_with_library(&path, &settings, &library).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let expected = format!(
            "companion_suffixes = dop, pp3, on1\nthumbnail_buttons = true\n\
             external_program = Capture One | C:\\Program Files\\Capture One\\CaptureOne.exe\n\
             favourite = {}\n",
            full("Favourite").display()
        );
        assert!(text.ends_with(&expected), "{text}");
    }

    #[test]
    fn a_bad_external_program_line_gives_one_problem_and_a_good_line_after_it_is_read() {
        for bad in [
            "Capture One C:\\CaptureOne.exe",
            "Capture | One | C:\\CaptureOne.exe",
            " | C:\\CaptureOne.exe",
            "Capture One |",
            "Capture One | Programs\\CaptureOne.exe",
        ] {
            let (_dir, path) = TempDir::with_file(&format!(
                "# RawStride\nexternal_program = {bad}\n\
                 external_program = Viewer | C:\\Tools\\viewer.exe\n"
            ));
            let (settings, problems) = load_settings(&path);
            assert_one_problem_on_line(&problems, 2);
            assert!(
                problems[0].contains(
                    "for external_program (expected a name, '|' and a full path to a program), \
                     line ignored"
                ),
                "{bad:?}: {problems:?}"
            );
            assert_eq!(
                settings.external_programs,
                vec![program("Viewer", r"C:\Tools\viewer.exe")],
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_sixth_external_program_gives_one_problem_and_the_first_five_are_kept() {
        let mut text = String::from("# RawStride\n");
        for program in five_programs() {
            text.push_str(&format!(
                "external_program = {} | {}\n",
                program.name,
                program.path.display()
            ));
        }
        text.push_str("external_program = Sixth | C:\\Tools\\sixth.exe\n");
        let (_dir, path) = TempDir::with_file(&text);
        let (settings, problems) = load_settings(&path);
        assert_one_problem_on_line(&problems, 7);
        assert!(
            problems[0].contains("more than 5 external programs, line ignored"),
            "{problems:?}"
        );
        assert_eq!(settings.external_programs, five_programs());
    }

    #[test]
    fn an_external_program_that_could_not_be_read_back_is_left_out_when_saving() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        let kept = program("Kept", r"C:\Tools\kept.exe");
        let also_kept = program("Also kept", r"C:\Tools\also kept.exe");
        let settings = Settings {
            external_programs: vec![
                program("A | B", r"C:\Tools\ab.exe"),
                kept.clone(),
                program("Trailing ", r"C:\Tools\trailing.exe"),
                also_kept.clone(),
            ],
            ..Settings::default()
        };
        save_settings(&path, &settings).unwrap();
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded.external_programs, vec![kept, also_kept]);
    }

    /// The item named `name`.
    fn item(name: &str) -> EditorItem {
        EditorItem::from_name(name).unwrap()
    }

    #[test]
    fn a_changed_editor_layout_saves_and_loads_back_equal() {
        let mut layout = EditorLayout::default();
        layout.move_section_up(EditorSection::Licence);
        layout.move_item_up(item("contact_info"));
        layout.set_section_shown(EditorSection::Assignment, false);
        layout.set_item_shown(item("city"), false);
        let settings = Settings {
            editor_layout: layout.clone(),
            ..Settings::default()
        };
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        save_settings(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains(
                "editor_section_order = content, creator_rights, time_place, licence, \
                 assignment, rating_label\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "\neditor_item_order = title, headline, description, alt_text, ext_description, \
                 description_writer, keywords, person_shown, person_details, product_shown, \
                 artwork, organisation_name, organisation_code, event, creator, contact_info, \
                 creator_jobtitle, copyright,"
            ),
            "{text}"
        );
        assert!(
            text.contains("editor_hidden_sections = assignment\n"),
            "{text}"
        );
        assert!(text.contains("editor_hidden_items = city\n"), "{text}");
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(loaded.editor_layout, layout);
        assert_eq!(loaded, settings);
    }

    #[test]
    fn the_default_editor_layout_writes_none_of_its_keys() {
        let dir = TempDir::new();
        let path = dir.0.join("settings.txt");
        save_settings(&path, &Settings::default()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        for key in [
            "editor_section_order",
            "editor_item_order",
            "editor_hidden_sections",
            "editor_hidden_items",
        ] {
            assert!(!text.contains(key), "{key}: {text}");
        }
        // Only hidden items: the order lines stay out.
        let mut layout = EditorLayout::default();
        layout.set_item_shown(item("event"), false);
        let settings = Settings {
            editor_layout: layout,
            ..Settings::default()
        };
        save_settings(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("editor_hidden_items = event\n"), "{text}");
        for key in [
            "editor_section_order",
            "editor_item_order",
            "editor_hidden_sections",
        ] {
            assert!(!text.contains(key), "{key}: {text}");
        }
        // A file without the lines gives the default layout.
        let (_dir, path) = TempDir::with_file("# RawStride\ncreate_iim = true\n");
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(loaded.editor_layout.is_default());
    }

    #[test]
    fn an_unknown_layout_name_gives_one_problem_and_the_rest_of_the_line_is_used() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\neditor_hidden_items = city, rating, event, nope\n\
             editor_section_order = licence, middle\n",
        );
        let (loaded, problems) = load_settings(&path);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].contains("line 2"), "{problems:?}");
        assert!(problems[0].contains("\"rating\""), "{problems:?}");
        assert!(problems[0].contains("editor_hidden_items"), "{problems:?}");
        assert!(problems[1].contains("line 2"), "{problems:?}");
        assert!(problems[1].contains("\"nope\""), "{problems:?}");
        assert!(problems[2].contains("line 3"), "{problems:?}");
        assert!(problems[2].contains("\"middle\""), "{problems:?}");
        let mut expected = EditorLayout::default();
        expected.set_item_shown(item("city"), false);
        expected.set_item_shown(item("event"), false);
        expected.set_section_order(&[EditorSection::Licence]);
        assert_eq!(loaded.editor_layout, expected);
    }

    #[test]
    fn an_item_order_line_leaving_out_an_item_gives_it_its_default_place() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\neditor_item_order = sublocation, date_created, state, country, \
             country_code, location_created, location_shown\n",
        );
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        let order: Vec<&str> = loaded
            .editor_layout
            .items(EditorSection::TimePlace)
            .iter()
            .map(|(item, _)| item.name())
            .collect();
        // City comes back right after Sublocation, the field before it by default.
        assert_eq!(
            order,
            [
                "sublocation",
                "city",
                "date_created",
                "state",
                "country",
                "country_code",
                "location_created",
                "location_shown",
            ]
        );
    }

    #[test]
    fn a_hidden_line_before_an_order_line_gives_the_same_layout_as_after_it() {
        let hidden = "editor_hidden_items = city, genre\neditor_hidden_sections = licence\n";
        let order = "editor_item_order = genre, instructions, job_id, subject_code, scene_code, \
                     about_cv_term, urgency, category, supplemental_categories, registry_entry, \
                     city, sublocation\n\
                     editor_section_order = licence, content, creator_rights, time_place, \
                     assignment, rating_label\n";
        let (_before_dir, before) = TempDir::with_file(&format!("# RawStride\n{hidden}{order}"));
        let (_after_dir, after) = TempDir::with_file(&format!("# RawStride\n{order}{hidden}"));
        let (from_before, problems) = load_settings(&before);
        assert!(problems.is_empty(), "{problems:?}");
        let (from_after, problems) = load_settings(&after);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(from_before.editor_layout, from_after.editor_layout);
        let layout = &from_before.editor_layout;
        assert!(!layout.is_shown(item("city")));
        assert!(!layout.is_shown(item("genre")));
        assert!(!layout.is_shown(item("image_creator")));
        assert_eq!(
            layout.sections().first(),
            Some(&(EditorSection::Licence, false))
        );
        assert_eq!(
            layout.items(EditorSection::Assignment).first(),
            Some(&(item("genre"), false))
        );
    }

    #[test]
    fn an_empty_layout_value_gives_the_default_and_the_last_line_wins() {
        let (_dir, path) = TempDir::with_file(
            "# RawStride\neditor_hidden_items =\neditor_hidden_sections =\n\
             editor_item_order =\neditor_section_order =\n",
        );
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(loaded.editor_layout.is_default());

        // A key given twice: the last line wins.
        let (_dir, path) = TempDir::with_file(
            "# RawStride\neditor_hidden_items = city\neditor_hidden_items = event\n\
             editor_section_order = licence\neditor_section_order =\n",
        );
        let (loaded, problems) = load_settings(&path);
        assert!(problems.is_empty(), "{problems:?}");
        let mut expected = EditorLayout::default();
        expected.set_item_shown(item("event"), false);
        assert_eq!(loaded.editor_layout, expected);
    }

    #[test]
    fn tidied_programs_clean_the_names_and_keep_at_most_five() {
        let tidied = tidied_programs(&[
            program(
                "  Capture | One  ",
                r"C:\Program Files\Capture One\CaptureOne.exe",
            ),
            program(" | ", r"C:\Tools\My Viewer.exe"),
            program("", r"C:\"),
        ]);
        assert_eq!(
            tidied,
            vec![
                program(
                    "Capture  One",
                    r"C:\Program Files\Capture One\CaptureOne.exe"
                ),
                program("My Viewer", r"C:\Tools\My Viewer.exe"),
            ]
        );
        let seven: Vec<ExternalProgram> = (1..=7)
            .map(|n| program(&format!("Program {n}"), &format!(r"C:\Tools\p{n}.exe")))
            .collect();
        let tidied = tidied_programs(&seven);
        assert_eq!(tidied.len(), MAX_EXTERNAL_PROGRAMS);
        assert_eq!(tidied, seven[..5].to_vec());
    }
}
