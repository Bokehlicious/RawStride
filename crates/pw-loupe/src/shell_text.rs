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

use crate::external_text::{open_in_another_text, program_number_text};
use crate::monitor_profile::ProfileSource;
use crate::rating_text::label_name;
use crate::shell::{Command, KeyChord, MenuKind, SubmenuKind, Theme, UNKNOWN_COMMIT};
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
    /// The heading of the section with the list of external programs (ADR 0019 E-Q2, T200).
    ExternalPrograms,
    /// The button under the list that chooses a program to add.
    AddProgram,
    /// The button of a row that moves the program one place up.
    MoveUp,
    /// The button of a row that takes the program away from the list.
    RemoveProgram,
    /// The line shown in place of the rows when the list is empty.
    NoPrograms,
    /// The title of the file dialog that chooses a program.
    ChooseProgram,
    /// The name of the file dialog's filter for `.exe` files.
    ProgramsFilter,
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
        SettingsText::ExternalPrograms => "External Programs",
        SettingsText::AddProgram => "Add…",
        SettingsText::MoveUp => "Move Up",
        SettingsText::RemoveProgram => "Remove",
        SettingsText::NoPrograms => "No programs added.",
        SettingsText::ChooseProgram => "Choose a Program",
        SettingsText::ProgramsFilter => "Programs",
    }
}

/// A text of the window "About RawStride": the version, the commit and the licence notice GPL-3.0
/// §0 calls "Appropriate Legal Notices" (ADR 0031 RL-Q9, T225). A `String`, since some variants
/// carry values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutText<'a> {
    /// The window's title.
    Title,
    /// The program's version (`shell::VERSION`).
    Version { version: &'a str },
    /// The commit the program was built from (`shell::COMMIT`), or `shell::UNKNOWN_COMMIT`.
    Commit { commit: &'a str },
    /// The copyright line; `authors` as Cargo gives them, separated by `:` (`shell::AUTHORS`).
    Copyright { authors: &'a str },
    /// The licence's grant.
    Licence,
    /// The disclaimer of warranty.
    Warranty,
    /// Where the licence is.
    LicenceFile,
    /// Where the source is (`shell::REPOSITORY`).
    Source { repository: &'a str },
    /// Where the third-party notices are (made by RL3, put in the program folder by RL4).
    ThirdParty,
    /// The button that closes the window.
    Close,
}

/// The text of `text` in the window "About RawStride" (ADR 0031 RL-Q9, GPL-3.0 §0, T225).
pub fn about_text(text: AboutText<'_>) -> String {
    match text {
        AboutText::Title => "About RawStride".to_owned(),
        AboutText::Version { version } => format!("Version {version}"),
        AboutText::Commit { commit } if commit == UNKNOWN_COMMIT => {
            "Built from an unknown commit".to_owned()
        }
        AboutText::Commit { commit } => format!("Built from commit {commit}"),
        AboutText::Copyright { authors } => {
            let authors = authors.split(':').collect::<Vec<_>>().join(", ");
            format!("Copyright © 2026 {authors}")
        }
        AboutText::Licence => "RawStride is free software: you can redistribute it and/or modify \
             it under the terms of the GNU General Public License as published by the Free \
             Software Foundation, either version 3 of the License, or (at your option) any later \
             version."
            .to_owned(),
        AboutText::Warranty => "RawStride comes with ABSOLUTELY NO WARRANTY, to the extent \
             permitted by law. See the GNU General Public License for details."
            .to_owned(),
        AboutText::LicenceFile => "The licence is in the file LICENSE in the program folder, and \
             at https://www.gnu.org/licenses/gpl-3.0.html."
            .to_owned(),
        AboutText::Source { repository } => format!("The source code is at {repository}."),
        AboutText::ThirdParty => "RawStride includes software by others under their own \
             licences; their notices are in the file THIRD-PARTY-NOTICES.txt in the program \
             folder."
            .to_owned(),
        AboutText::Close => "Close".to_owned(),
    }
}

/// The tooltip of the disabled "Add…" button when the list of external programs is full
/// (ADR 0019 E-Q1, T200).
pub fn external_program_limit_text(n: usize) -> String {
    if n == 1 {
        "At most 1 program".to_string()
    } else {
        format!("At most {n} programs")
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
        MenuKind::Help => "Help",
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
        Command::MovePicked => "Move to Picked".to_owned(),
        Command::MoveToSubfolders => "Move Rejected and Picked Images to Subfolders".to_owned(),
        Command::UndoMove => "Undo Move".to_owned(),
        Command::Label(label) => label_name(label).to_owned(),
        Command::ClearLabel => "No label".to_owned(),
        Command::RotateLeft => "Rotate Left".to_owned(),
        Command::RotateRight => "Rotate Right".to_owned(),
        Command::ToggleMode => "RAW ↔ JPEG".to_owned(),
        Command::DeleteToRecycleBin => "Delete to Recycle Bin".to_owned(),
        Command::DeleteRejected => "Delete Rejected Images".to_owned(),
        // Never shown in the menu, which draws `open_in_text(name)`; `command_text` takes no name.
        Command::OpenIn(index) => program_number_text(usize::from(index) + 1),
        Command::OpenInAnother => open_in_another_text().to_owned(),
        Command::ShowInExplorer => "Show in File Explorer".to_owned(),
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
        Command::About => "About RawStride…".to_owned(),
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
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
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
        assert_eq!(menu_text(MenuKind::Help), "Help");
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
                "Undo Move",
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
                "Move to Picked",
                "Move Rejected and Picked Images to Subfolders",
                "Red",
                "Yellow",
                "Green",
                "Blue",
                "Purple",
                "No label",
                "Rotate Left",
                "Rotate Right",
                "RAW ↔ JPEG",
                "Show in File Explorer",
                "Delete to Recycle Bin",
                "Delete Rejected Images",
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
                "About RawStride…",
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
            Some("Ctrl+Z"),
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
            Some("P"),
            None,
            Some("6"),
            Some("7"),
            Some("8"),
            Some("9"),
            Some("Ctrl+5"),
            Some("Ctrl+0"),
            Some("Ctrl+←"),
            Some("Ctrl+→"),
            Some("R"),
            None,
            Some("Delete"),
            None,
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

    /// Rotate Left and Rotate Right (ADR 0024 §2.1, O-Q1, T237): their texts, and their keys
    /// shown with arrows, not egui's key names.
    #[test]
    fn the_rotate_texts_and_keys() {
        assert_eq!(command_text(Command::RotateLeft), "Rotate Left");
        assert_eq!(command_text(Command::RotateRight), "Rotate Right");
        let ctrl = |key| KeyChord {
            key,
            ctrl: true,
            shift: false,
            alt: false,
        };
        assert_eq!(key_text(ctrl(Key::ArrowLeft)), "Ctrl+←");
        assert_eq!(key_text(ctrl(Key::ArrowRight)), "Ctrl+→");
    }

    /// The texts of the two commands of the external programs (ADR 0019 §2.2, T202): a program
    /// by its number (never shown in the menu, which draws `open_in_text(name)`) and "Open in
    /// Another Program…"; the key shows as "Ctrl+E".
    #[test]
    fn the_open_in_command_texts_and_key() {
        use crate::shell::OPEN_IN_KEY;
        assert_eq!(command_text(Command::OpenIn(0)), "Open in Program 1");
        assert_eq!(command_text(Command::OpenIn(4)), "Open in Program 5");
        assert_eq!(
            command_text(Command::OpenInAnother),
            "Open in Another Program…"
        );
        assert_eq!(key_text(OPEN_IN_KEY), "Ctrl+E");
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
            window_title(&ProfileSource::File("Monitor 1.icc".to_string())),
            "RawStride  ·  Display profile: Monitor 1.icc"
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
    fn the_external_programs_section_has_the_texts_of_t200() {
        let texts = [
            (SettingsText::ExternalPrograms, "External Programs"),
            (SettingsText::AddProgram, "Add…"),
            (SettingsText::MoveUp, "Move Up"),
            (SettingsText::RemoveProgram, "Remove"),
            (SettingsText::NoPrograms, "No programs added."),
            (SettingsText::ChooseProgram, "Choose a Program"),
            (SettingsText::ProgramsFilter, "Programs"),
        ];
        for (text, expected) in texts {
            assert_eq!(settings_text(text), expected, "{text:?}");
        }
        assert_eq!(external_program_limit_text(1), "At most 1 program");
        assert_eq!(external_program_limit_text(5), "At most 5 programs");
    }

    /// Every text of the window "About RawStride", with sample values, not the real authors
    /// (ADR 0031 RL-Q9, T225).
    #[test]
    fn the_about_window_has_the_texts_of_the_task() {
        assert_eq!(about_text(AboutText::Title), "About RawStride");
        assert_eq!(
            about_text(AboutText::Version { version: "1.2.3" }),
            "Version 1.2.3"
        );
        assert_eq!(
            about_text(AboutText::Commit {
                commit: "0123456789ab"
            }),
            "Built from commit 0123456789ab"
        );
        assert_eq!(
            about_text(AboutText::Commit {
                commit: UNKNOWN_COMMIT
            }),
            "Built from an unknown commit"
        );
        assert_eq!(
            about_text(AboutText::Copyright { authors: "A" }),
            "Copyright © 2026 A"
        );
        assert_eq!(
            about_text(AboutText::Copyright { authors: "A:B" }),
            "Copyright © 2026 A, B"
        );
        assert_eq!(
            about_text(AboutText::Licence),
            "RawStride is free software: you can redistribute it and/or modify it under the terms \
             of the GNU General Public License as published by the Free Software Foundation, \
             either version 3 of the License, or (at your option) any later version."
        );
        assert_eq!(
            about_text(AboutText::Warranty),
            "RawStride comes with ABSOLUTELY NO WARRANTY, to the extent permitted by law. See the \
             GNU General Public License for details."
        );
        assert_eq!(
            about_text(AboutText::LicenceFile),
            "The licence is in the file LICENSE in the program folder, and at \
             https://www.gnu.org/licenses/gpl-3.0.html."
        );
        assert_eq!(
            about_text(AboutText::Source {
                repository: "https://example.com/source"
            }),
            "The source code is at https://example.com/source."
        );
        assert_eq!(
            about_text(AboutText::ThirdParty),
            "RawStride includes software by others under their own licences; their notices are in \
             the file THIRD-PARTY-NOTICES.txt in the program folder."
        );
        assert_eq!(about_text(AboutText::Close), "Close");
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
