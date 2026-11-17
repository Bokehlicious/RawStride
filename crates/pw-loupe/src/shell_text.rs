//! The texts of the main window's shell (ADR 0017 U1, T109; rule 13): the menu bar, the command
//! bar, the side columns, the folder dialog, the tabs, the window title, the settings window, the
//! messages at start (the recovery after an interrupted metadata write, ExifTool) and the general
//! status messages (T116). Each text is made here, whole; the drawing code in `main.rs` holds none
//! of them.

use std::borrow::Borrow;
use std::path::Path;

use egui::Key;
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Recovered;
use pw_metadata::{MIN_EXIFTOOL_VERSION, ToolVersion};

use crate::monitor_profile::ProfileSource;
use crate::rating_text::label_name;
use crate::shell::{Command, KeyChord, MenuKind, SubmenuKind, Theme};
use crate::sort::SortKey;

/// The separator between the parts of the status line.
const STATUS_SEPARATOR: &str = "  ·  ";

/// A fixed text of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellText {
    /// The left column's upper heading.
    Library,
    /// The left column's lower heading.
    Tasks,
    /// The right column's heading.
    Inspector,
    /// The line under "Tasks" when no import runs hidden.
    NoTasks,
    /// The folder dialog's title (Open Folder, Ctrl+O).
    OpenFolderTitle,
    /// View ▸ Filter.
    FilterMenu,
    /// View ▸ Filter ▸ Show All (clears the filter).
    ShowAll,
    /// The settings window's theme list.
    ThemeLabel,
    /// The collapse button of the left column (a glyph in the bundled font) and its hover text.
    CollapseLeft,
    /// The collapse button of the right column (a glyph).
    CollapseRight,
    CollapseHover,
    /// The open button of a collapsed left column (a glyph).
    ExpandLeft,
    /// The open button of a collapsed right column (a glyph).
    ExpandRight,
    ExpandHover,
    /// The title of a tab without a folder (T116, from `tabs.rs`).
    EmptyTab,
    /// The empty contact sheet without a folder (T116, from `main.rs`).
    DropHere,
    /// The status line of a tab without images (T116, from `main.rs`).
    NoImageSelected,
    /// A thumbnail whose pixels could not be used (T116, from `main.rs`).
    InvalidThumbnail,
    /// The thumbnail worker has stopped (T116, from `main.rs`).
    ThumbnailWorkerStopped,
    /// The thumbnail-size slider in the command bar (T116, from `main.rs`).
    ThumbnailSize,
    /// The title of the message about writes left unfinished at the last exit (T91; T116, from
    /// `main.rs`).
    RecoveryTitle,
    /// View ▸ Sort By (ADR 0017 U14-Q7, T138).
    SortMenu,
}

/// The text of `text`.
pub fn shell_text(text: ShellText) -> &'static str {
    match text {
        ShellText::Library => "Library",
        ShellText::Tasks => "Tasks",
        ShellText::Inspector => "Inspector",
        ShellText::NoTasks => "No tasks in progress.",
        ShellText::OpenFolderTitle => "Open Folder",
        ShellText::FilterMenu => "Filter",
        ShellText::ShowAll => "Show All",
        ShellText::ThemeLabel => "Theme",
        ShellText::CollapseLeft => "«",
        ShellText::CollapseRight => "»",
        ShellText::CollapseHover => "Collapse Column",
        ShellText::ExpandLeft => "»",
        ShellText::ExpandRight => "«",
        ShellText::ExpandHover => "Show Column",
        ShellText::EmptyTab => "Empty",
        ShellText::DropHere => "Drag a folder or an image here",
        ShellText::NoImageSelected => "No image selected",
        ShellText::InvalidThumbnail => "Invalid thumbnail data",
        ShellText::ThumbnailWorkerStopped => "Thumbnail loading has stopped",
        ShellText::ThumbnailSize => "Size",
        ShellText::RecoveryTitle => "Metadata writing was interrupted",
        ShellText::SortMenu => "Sort By",
    }
}

/// The name of a sort key (View ▸ Sort By and the command bar's drop-down; U14-Q1, T138).
pub fn sort_key_text(key: SortKey) -> &'static str {
    match key {
        SortKey::Name => "File Name",
        SortKey::Capture => "Capture Time",
        SortKey::Modified => "Modified",
        SortKey::Type => "File Type",
        SortKey::Rating => "Rating",
    }
}

/// The command bar's sort drop-down when closed: "Sort: {key}" (U14-Q7).
pub fn sort_button_text(key: SortKey) -> String {
    format!("Sort: {}", sort_key_text(key))
}

/// The command bar's direction button: "↑" ascending, "↓" descending (U14-Q7).
pub fn sort_direction_text(descending: bool) -> &'static str {
    if descending { "↓" } else { "↑" }
}

/// The hover text of the direction button: the direction it shows.
pub fn sort_direction_hover(descending: bool) -> &'static str {
    if descending {
        "Descending"
    } else {
        "Ascending"
    }
}

/// The status bar while a tab waits for its capture times (U14-Q4): "{read} of {total}" read.
pub fn sorting_text(read: usize, total: usize) -> String {
    format!("Sorting by capture time: {read} of {total}")
}

/// The status line (the contact sheet's and the loupe's): its parts in order, joined with the
/// separator " · " with two spaces on each side.
pub fn status_line_text<S: Borrow<str>>(parts: &[S]) -> String {
    parts.join(STATUS_SEPARATOR)
}

/// The status message when a folder or a file cannot be opened.
pub fn open_error_text(error: &str) -> String {
    format!("Cannot open: {error}")
}

/// The status message when the opened folder has no images; `None` names no folder ("the folder").
pub fn no_images_text(folder: Option<&Path>) -> String {
    match folder {
        Some(folder) => format!("No images in {}", folder.display()),
        None => "No images in the folder".to_string(),
    }
}

/// The status message when the metadata of closed tabs cannot be cleaned up.
pub fn prune_error_text(error: &str) -> String {
    format!("Cannot clean up metadata: {error}")
}

/// The status message when the current folder is unknown, so relative image paths cannot be
/// resolved (`main.rs` and `metadata.rs`).
pub fn path_resolution_error_text(error: &str) -> String {
    format!("cannot resolve image paths: {error}")
}

/// The window title naming the monitor profile (ADR 0016 §5; T116, from `monitor_profile.rs`).
pub fn window_title(source: &ProfileSource) -> String {
    match source {
        ProfileSource::File(name) => format!("RawStride  ·  Display profile: {name}"),
        ProfileSource::Missing => "RawStride  ·  No display profile – shown as sRGB".to_string(),
        ProfileSource::Unusable(_) => {
            "RawStride  ·  The display profile could not be used – shown as sRGB".to_string()
        }
    }
}

/// The first line of the recovery message (T91; T116, from `main.rs`).
/// The first line of that message.
const RECOVERY_HEADING: &str =
    "RawStride closed in the middle of a metadata write. Check these files:";

/// The message about the writes [`Journal::recover`](pw_metadata::journal::Journal::recover) could not resolve (`NeedsUser`, `Failed`,
/// `Unreadable`): one line per entry with the original and, when the backup file exists
/// (`exists`), the backup. `Cleaned` and `Restored` are not shown; `None` when nothing is left.
pub fn recovery_text(recovered: &[Recovered], exists: impl Fn(&Path) -> bool) -> Option<String> {
    let lines: Vec<String> = recovered
        .iter()
        .filter_map(|item| match item {
            Recovered::Cleaned { .. } | Recovered::Restored { .. } => None,
            Recovered::NeedsUser { entry } | Recovered::Failed { entry, .. } => {
                Some(if exists(&entry.backup) {
                    format!(
                        "{} (backup: {})",
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
pub fn exiftool_message(result: &Result<ToolVersion, String>) -> Option<(&'static str, String)> {
    match result {
        Err(text) => Some((
            "ExifTool is missing",
            format!(
                "ExifTool could not be started: {text}. Ratings and metadata cannot be read or \
                 saved."
            ),
        )),
        Ok(version) if *version < MIN_EXIFTOOL_VERSION => Some((
            "ExifTool is too old",
            format!(
                "ExifTool {version} is too old. RawStride needs version \
                 {MIN_EXIFTOOL_VERSION} or later. Ratings and metadata are not saved until \
                 ExifTool has been updated."
            ),
        )),
        Ok(_) => None,
    }
}

/// A fixed text of the settings window (ADR 0007 §5.2, §5.4/Q10, T106).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsText {
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
pub fn settings_text(text: SettingsText) -> &'static str {
    match text {
        SettingsText::Title => "Settings",
        SettingsText::CreateIim => "Create IPTC-IIM for compatibility with older systems",
        SettingsText::CreateIimNote => {
            "Applies to JPEG, TIFF and DNG files without IIM. Off by default."
        }
        SettingsText::Encoding => "Character encoding for older IPTC and Exif text",
        SettingsText::Save => "Save",
        SettingsText::Cancel => "Cancel",
        SettingsText::NoFolder => "the settings folder is unknown (LOCALAPPDATA is missing)",
    }
}

/// The message shown when the settings file could not be saved; the settings still apply until
/// RawStride closes (T106).
pub fn settings_save_error_text(error: &str) -> String {
    format!("The settings could not be saved: {error}")
}

/// The text of a fallback encoding in the settings window's drop-down.
pub fn fallback_text(fallback: Fallback) -> &'static str {
    match fallback {
        Fallback::Windows1252 => "Windows-1252 (Western European)",
        Fallback::Latin1 => "ISO 8859-1 (Latin-1)",
    }
}

/// The name of a menu in the menu bar.
pub fn menu_text(menu: MenuKind) -> &'static str {
    match menu {
        MenuKind::File => "File",
        MenuKind::Edit => "Edit",
        MenuKind::Image => "Image",
        MenuKind::View => "View",
    }
}

/// The name of a submenu.
pub fn submenu_text(submenu: SubmenuKind) -> &'static str {
    match submenu {
        SubmenuKind::Rating => "Rating",
        SubmenuKind::Label => "Label",
        SubmenuKind::Theme => "Theme",
    }
}

/// The name of a theme (the menu and the settings window).
pub fn theme_text(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "Dark",
        Theme::Light => "Light",
    }
}

/// The text of a menu item (also the command bar's buttons "Import…" and "Open Folder…").
pub fn command_text(command: Command) -> String {
    match command {
        Command::OpenFolder => "Open Folder…".to_owned(),
        Command::Import => "Import…".to_owned(),
        Command::CloseTab => "Close Tab".to_owned(),
        Command::Settings => "Settings…".to_owned(),
        Command::Quit => "Exit".to_owned(),
        Command::SelectAll => "Mark All".to_owned(),
        Command::ClearSelection => "Unmark All".to_owned(),
        Command::Metadata => "Metadata…".to_owned(),
        Command::Stars(0) => "No stars".to_owned(),
        Command::Stars(1) => "1 star".to_owned(),
        Command::Stars(stars) => format!("{stars} stars"),
        Command::Reject => "Reject".to_owned(),
        Command::Label(label) => label_name(label).to_owned(),
        Command::ClearLabel => "No label".to_owned(),
        Command::ToggleMode => "RAW ↔ JPEG".to_owned(),
        Command::ToggleView => "Preview".to_owned(),
        Command::Fit => "Fit".to_owned(),
        Command::ToggleZoom => "Fit ↔ 100%".to_owned(),
        Command::ZoomIn => "Zoom In".to_owned(),
        Command::ZoomOut => "Zoom Out".to_owned(),
        Command::ToggleAnchor => "Zoom at Pointer".to_owned(),
        Command::ClipHighlights => "Show Clipped Highlights".to_owned(),
        Command::ClipShadows => "Show Deep Shadows".to_owned(),
        Command::FocusMarking => "Show Focus Marking".to_owned(),
        Command::Compare => "Compare".to_owned(),
        Command::HoldImage => "Hold Current Image".to_owned(),
        Command::SwapImages => "Swap Images".to_owned(),
        Command::AboveBelow => "Above and Below".to_owned(),
        Command::LockZoom => "Lock Zoom and Position".to_owned(),
        Command::ToggleInfo => "Inspector".to_owned(),
        Command::NextTab => "Next Tab".to_owned(),
        Command::PreviousTab => "Previous Tab".to_owned(),
        Command::ToggleColumns => "Side Columns".to_owned(),
        Command::LeftColumn => "Left Column".to_owned(),
        Command::Fullscreen => "Full Screen".to_owned(),
        Command::Theme(theme) => theme_text(theme).to_owned(),
        Command::Sort(key) => sort_key_text(key).to_owned(),
        Command::SortDescending => "Descending".to_owned(),
    }
}

/// The key a menu item shows, for example "Ctrl+Shift+Tab", "F11", "5" or "−".
pub fn key_text(chord: KeyChord) -> String {
    let key = match chord.key {
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::Plus => "+",
        Key::Minus => "−",
        Key::Escape => "Esc",
        other => other.name(),
    };
    match (chord.ctrl, chord.shift, chord.alt) {
        (false, false, false) => key.to_owned(),
        (true, false, false) => format!("Ctrl+{key}"),
        (true, true, false) => format!("Ctrl+Shift+{key}"),
        (false, false, true) => format!("Alt+{key}"),
        (false, true, false) => format!("Shift+{key}"),
        (true, false, true) => format!("Ctrl+Alt+{key}"),
        (false, true, true) => format!("Shift+Alt+{key}"),
        (true, true, true) => format!("Ctrl+Shift+Alt+{key}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{MENUS, MenuEntry, menu_items};
    use pw_metadata::journal::Entry;

    #[test]
    fn every_fixed_text_of_the_shell() {
        let texts = [
            (ShellText::Library, "Library"),
            (ShellText::Tasks, "Tasks"),
            (ShellText::Inspector, "Inspector"),
            (ShellText::NoTasks, "No tasks in progress."),
            (ShellText::OpenFolderTitle, "Open Folder"),
            (ShellText::FilterMenu, "Filter"),
            (ShellText::ShowAll, "Show All"),
            (ShellText::ThemeLabel, "Theme"),
            (ShellText::CollapseLeft, "«"),
            (ShellText::CollapseRight, "»"),
            (ShellText::CollapseHover, "Collapse Column"),
            (ShellText::ExpandLeft, "»"),
            (ShellText::ExpandRight, "«"),
            (ShellText::ExpandHover, "Show Column"),
        ];
        for (text, expected) in texts {
            assert_eq!(shell_text(text), expected, "{text:?}");
        }
        assert_eq!(menu_text(MenuKind::File), "File");
        assert_eq!(menu_text(MenuKind::Edit), "Edit");
        assert_eq!(menu_text(MenuKind::Image), "Image");
        assert_eq!(menu_text(MenuKind::View), "View");
        assert_eq!(submenu_text(SubmenuKind::Rating), "Rating");
        assert_eq!(submenu_text(SubmenuKind::Label), "Label");
        assert_eq!(submenu_text(SubmenuKind::Theme), "Theme");
        assert_eq!(theme_text(Theme::Dark), "Dark");
        assert_eq!(theme_text(Theme::Light), "Light");
    }

    #[test]
    fn every_menu_item_text_in_menu_order() {
        let texts: Vec<String> = menu_items()
            .into_iter()
            .map(|item| command_text(item.command))
            .collect();
        assert_eq!(
            texts,
            vec![
                "Open Folder…",
                "Import…",
                "Close Tab",
                "Settings…",
                "Exit",
                "Mark All",
                "Unmark All",
                "Metadata…",
                "No stars",
                "1 star",
                "2 stars",
                "3 stars",
                "4 stars",
                "5 stars",
                "Reject",
                "Red",
                "Yellow",
                "Green",
                "Blue",
                "Purple",
                "No label",
                "RAW ↔ JPEG",
                "Preview",
                "Fit",
                "Fit ↔ 100%",
                "Zoom In",
                "Zoom Out",
                "Zoom at Pointer",
                "Show Clipped Highlights",
                "Show Deep Shadows",
                "Show Focus Marking",
                "Compare",
                "Hold Current Image",
                "Swap Images",
                "Above and Below",
                "Lock Zoom and Position",
                "Next Tab",
                "Previous Tab",
                "Side Columns",
                "Left Column",
                "Inspector",
                "Full Screen",
                "Dark",
                "Light",
            ]
        );
    }

    #[test]
    fn every_menu_key_text_in_menu_order() {
        let keys: Vec<Option<String>> = menu_items()
            .into_iter()
            .map(|item| item.key.map(key_text))
            .collect();
        let expected: Vec<Option<&str>> = vec![
            Some("Ctrl+O"),
            Some("Ctrl+I"),
            Some("Ctrl+W"),
            None,
            Some("Alt+F4"),
            Some("Ctrl+A"),
            Some("Esc"),
            Some("M"),
            Some("0"),
            Some("1"),
            Some("2"),
            Some("3"),
            Some("4"),
            Some("5"),
            Some("X"),
            Some("6"),
            Some("7"),
            Some("8"),
            Some("9"),
            Some("Ctrl+5"),
            Some("Ctrl+0"),
            Some("R"),
            Some("G"),
            Some("F"),
            Some("Z"),
            Some("+"),
            Some("−"),
            Some("A"),
            Some("J"),
            Some("Shift+J"),
            Some("E"),
            Some("C"),
            Some("K"),
            Some("Shift+K"),
            Some("V"),
            Some("L"),
            Some("Ctrl+Tab"),
            Some("Ctrl+Shift+Tab"),
            Some("H"),
            None,
            Some("I"),
            Some("F11"),
            None,
            None,
        ];
        let expected: Vec<Option<String>> = expected
            .into_iter()
            .map(|key| key.map(str::to_owned))
            .collect();
        assert_eq!(keys, expected);
        assert!(
            MENUS
                .iter()
                .any(|menu| menu.entries.contains(&MenuEntry::Filter))
        );
    }

    #[test]
    fn the_sort_texts_of_the_command_bar_the_menu_and_the_status_bar() {
        use crate::sort::SORT_KEYS;
        let names: Vec<&str> = SORT_KEYS.into_iter().map(sort_key_text).collect();
        assert_eq!(
            names,
            [
                "File Name",
                "Capture Time",
                "Modified",
                "File Type",
                "Rating"
            ]
        );
        for key in SORT_KEYS {
            assert_eq!(command_text(Command::Sort(key)), sort_key_text(key));
        }
        assert_eq!(sort_button_text(SortKey::Capture), "Sort: Capture Time");
        assert_eq!(sort_button_text(SortKey::Name), "Sort: File Name");
        assert_eq!(shell_text(ShellText::SortMenu), "Sort By");
        assert_eq!(command_text(Command::SortDescending), "Descending");
        assert_eq!(sort_direction_text(false), "↑");
        assert_eq!(sort_direction_text(true), "↓");
        assert_eq!(sort_direction_hover(false), "Ascending");
        assert_eq!(sort_direction_hover(true), "Descending");
        assert_eq!(sorting_text(3, 120), "Sorting by capture time: 3 of 120");
        assert!(
            MENUS
                .iter()
                .any(|menu| menu.entries.contains(&MenuEntry::Sort))
        );
    }

    /// The clipping overlays' menu items (ADR 0017 U7b-Q10, T148).
    #[test]
    fn the_clipping_menu_items_and_their_keys() {
        assert_eq!(
            command_text(Command::ClipHighlights),
            "Show Clipped Highlights"
        );
        assert_eq!(command_text(Command::ClipShadows), "Show Deep Shadows");
        let key_of = |command| {
            menu_items()
                .into_iter()
                .find(|item| item.command == command)
                .and_then(|item| item.key)
                .map(key_text)
        };
        assert_eq!(key_of(Command::ClipHighlights).as_deref(), Some("J"));
        assert_eq!(key_of(Command::ClipShadows).as_deref(), Some("Shift+J"));
    }

    /// The focus marking's menu item (ADR 0017 §2.6, U9-Q1, T168).
    #[test]
    fn the_focus_marking_menu_item_and_its_key() {
        assert_eq!(command_text(Command::FocusMarking), "Show Focus Marking");
        let key = menu_items()
            .into_iter()
            .find(|item| item.command == Command::FocusMarking)
            .and_then(|item| item.key)
            .map(key_text);
        assert_eq!(key.as_deref(), Some("E"));
    }

    /// The compare's menu items (ADR 0017 U6-Q12, T153, T154).
    #[test]
    fn the_compare_menu_items_and_their_keys() {
        let key_of = |command| {
            menu_items()
                .into_iter()
                .find(|item| item.command == command)
                .and_then(|item| item.key)
                .map(key_text)
        };
        for (command, text, key) in [
            (Command::Compare, "Compare", "C"),
            (Command::HoldImage, "Hold Current Image", "K"),
            (Command::SwapImages, "Swap Images", "Shift+K"),
            (Command::AboveBelow, "Above and Below", "V"),
            (Command::LockZoom, "Lock Zoom and Position", "L"),
        ] {
            assert_eq!(command_text(command), text);
            assert_eq!(key_of(command).as_deref(), Some(key), "{text}");
        }
    }

    #[test]
    fn key_text_of_every_modifier_combination() {
        let chord = |ctrl, shift, alt| KeyChord {
            key: Key::K,
            ctrl,
            shift,
            alt,
        };
        assert_eq!(key_text(chord(false, false, false)), "K");
        assert_eq!(key_text(chord(true, false, false)), "Ctrl+K");
        assert_eq!(key_text(chord(true, true, false)), "Ctrl+Shift+K");
        assert_eq!(key_text(chord(false, false, true)), "Alt+K");
        assert_eq!(key_text(chord(false, true, false)), "Shift+K");
        assert_eq!(key_text(chord(true, false, true)), "Ctrl+Alt+K");
        assert_eq!(key_text(chord(false, true, true)), "Shift+Alt+K");
        assert_eq!(key_text(chord(true, true, true)), "Ctrl+Shift+Alt+K");
    }

    /// Visa ▸ Förhandsvisning (G) opens and closes the preview (ADR 0017 §2.2, T111); the old
    /// "Kontaktark ↔ lupp" is gone.
    #[test]
    fn the_view_menu_opens_the_preview_with_g() {
        let Some(view) = MENUS.iter().find(|menu| menu.kind == MenuKind::View) else {
            panic!("there is a Visa menu");
        };
        let Some(MenuEntry::Item(first)) = view.entries.first() else {
            panic!("Visa starts with an item");
        };
        assert_eq!(first.command, Command::ToggleView);
        assert_eq!(command_text(first.command), "Preview");
        assert_eq!(first.key.map(key_text).as_deref(), Some("G"));
        assert!(
            menu_items()
                .iter()
                .all(|item| !command_text(item.command).contains("lupp"))
        );
    }

    #[test]
    fn the_window_title_names_the_profile() {
        assert_eq!(
            window_title(&ProfileSource::File(
                "CX240(…)08CAL (DP).icc".to_string()
            )),
            "RawStride  ·  Display profile: CX240(…)08CAL (DP).icc"
        );
        assert_eq!(
            window_title(&ProfileSource::Missing),
            "RawStride  ·  No display profile – shown as sRGB"
        );
        assert_eq!(
            window_title(&ProfileSource::Unusable("detail".to_string())),
            "RawStride  ·  The display profile could not be used – shown as sRGB"
        );
    }

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
                "ExifTool is missing",
                "ExifTool could not be started: ExifTool: could not start exiftool: program not \
                 found. Ratings and metadata cannot be read or saved."
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
                "ExifTool is too old",
                "ExifTool 12.40 is too old. RawStride needs version 12.67 or later. \
                 Ratings and metadata are not saved until ExifTool has been updated."
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
    fn the_settings_window_has_the_texts_of_the_task() {
        assert_eq!(settings_text(SettingsText::Title), "Settings");
        assert_eq!(
            settings_text(SettingsText::CreateIim),
            "Create IPTC-IIM for compatibility with older systems"
        );
        assert_eq!(
            settings_text(SettingsText::CreateIimNote),
            "Applies to JPEG, TIFF and DNG files without IIM. Off by default."
        );
        assert_eq!(
            settings_text(SettingsText::Encoding),
            "Character encoding for older IPTC and Exif text"
        );
        assert_eq!(settings_text(SettingsText::Save), "Save");
        assert_eq!(settings_text(SettingsText::Cancel), "Cancel");
        assert_eq!(
            settings_save_error_text("åtkomst nekad"),
            "The settings could not be saved: åtkomst nekad"
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
                "RawStride closed in the middle of a metadata write. Check these files:\n{}",
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
                "RawStride closed in the middle of a metadata write. Check these \
                 files:\n{} (backup: {})",
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
                "RawStride closed in the middle of a metadata write. Check these \
                 files:\n{}\n{}",
                folder.join("bild.jpg").display(),
                journal_file.display()
            ))
        );
    }

    /// One example of each text moved here by T116, as it was before.
    #[test]
    fn every_moved_shell_text_keeps_its_wording() {
        let texts = [
            (ShellText::EmptyTab, "Empty"),
            (ShellText::DropHere, "Drag a folder or an image here"),
            (ShellText::NoImageSelected, "No image selected"),
            (ShellText::InvalidThumbnail, "Invalid thumbnail data"),
            (
                ShellText::ThumbnailWorkerStopped,
                "Thumbnail loading has stopped",
            ),
            (ShellText::ThumbnailSize, "Size"),
            (ShellText::RecoveryTitle, "Metadata writing was interrupted"),
        ];
        for (text, expected) in texts {
            assert_eq!(shell_text(text), expected, "{text:?}");
        }
        assert_eq!(
            status_line_text(&["3 / 31", "TN100423.ARW", "Läser betyg …"]),
            "3 / 31  ·  TN100423.ARW  ·  Läser betyg …"
        );
        assert_eq!(
            status_line_text(&["Ingen bild vald".to_string()]),
            "Ingen bild vald"
        );
        assert_eq!(open_error_text("D:/x: nekad"), "Cannot open: D:/x: nekad");
        assert_eq!(
            no_images_text(Some(Path::new("D:/Foto"))),
            "No images in D:/Foto"
        );
        assert_eq!(no_images_text(None), "No images in the folder");
        assert_eq!(prune_error_text("nekad"), "Cannot clean up metadata: nekad");
        assert_eq!(
            path_resolution_error_text("denied"),
            "cannot resolve image paths: denied"
        );
        assert_eq!(fallback_text(Fallback::Latin1), "ISO 8859-1 (Latin-1)");
        assert_eq!(
            settings_text(SettingsText::NoFolder),
            "the settings folder is unknown (LOCALAPPDATA is missing)"
        );
    }
}
