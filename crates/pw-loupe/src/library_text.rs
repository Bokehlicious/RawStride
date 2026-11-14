//! The texts of the library in the left column (ADR 0017 §2.1, U4, T135; rule 13): the sections'
//! headings, the right-click menus, the empty sections, the drives' names and the line of an
//! expand that failed. Each text is made here, whole; the drawing code in `main.rs` holds none of
//! them. Folder names and paths are data and are shown as they are.

use std::io;

/// A fixed text of the library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryText {
    /// The folder tree's section heading.
    Folders,
    /// The favourites' section heading.
    Favourites,
    /// The recent folders' section heading.
    Recent,
    /// The button that reads the drives again, and its hover text.
    Refresh,
    RefreshHover,
    /// The right-click menu of a folder in the tree, of a folder tab and the File menu item.
    AddToFavourites,
    /// The right-click menu of a favourite.
    RemoveFromFavourites,
    /// The right-click menu of the Recent section.
    ClearRecent,
    /// The empty Favourites section.
    NoFavourites,
    /// The empty Recent section.
    NoRecent,
    /// The hover text of a favourite or a recent folder that no longer exists.
    FolderNotFound,
}

/// The text of `text`.
pub fn library_text(text: LibraryText) -> &'static str {
    match text {
        LibraryText::Folders => "Folders",
        LibraryText::Favourites => "Favourites",
        LibraryText::Recent => "Recent",
        LibraryText::Refresh => "Refresh",
        LibraryText::RefreshHover => "Read the drives again (F5)",
        LibraryText::AddToFavourites => "Add to Favourites",
        LibraryText::RemoveFromFavourites => "Remove from Favourites",
        LibraryText::ClearRecent => "Clear Recent Folders",
        LibraryText::NoFavourites => "Right-click a folder to add it.",
        LibraryText::NoRecent => "No folders opened yet.",
        LibraryText::FolderNotFound => "Folder not found",
    }
}

/// A drive in the tree: "{label} ({letter}:)", "{letter}:" without a label, and
/// "{letter}: (no disk)" for an empty card reader or DVD drive.
pub fn drive_text(letter: char, label: Option<&str>, has_disk: bool) -> String {
    match (has_disk, label) {
        (false, _) => format!("{letter}: (no disk)"),
        (true, Some(label)) => format!("{label} ({letter}:)"),
        (true, None) => format!("{letter}:"),
    }
}

/// The greyed line under a folder whose subfolders could not be read. The system's own message
/// is not shown: it is in the system's language, not the program's (GR14).
pub fn expand_error_text(error: &io::Error) -> String {
    match (error.kind(), error.raw_os_error()) {
        (io::ErrorKind::PermissionDenied, _) => "No access to this folder".to_string(),
        (io::ErrorKind::NotFound, _) => "Folder not found".to_string(),
        (_, Some(code)) => format!("Cannot read this folder (error {code})"),
        (_, None) => "Cannot read this folder".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixed_text_of_the_library() {
        let texts = [
            (LibraryText::Folders, "Folders"),
            (LibraryText::Favourites, "Favourites"),
            (LibraryText::Recent, "Recent"),
            (LibraryText::Refresh, "Refresh"),
            (LibraryText::RefreshHover, "Read the drives again (F5)"),
            (LibraryText::AddToFavourites, "Add to Favourites"),
            (LibraryText::RemoveFromFavourites, "Remove from Favourites"),
            (LibraryText::ClearRecent, "Clear Recent Folders"),
            (LibraryText::NoFavourites, "Right-click a folder to add it."),
            (LibraryText::NoRecent, "No folders opened yet."),
            (LibraryText::FolderNotFound, "Folder not found"),
        ];
        for (text, expected) in texts {
            assert_eq!(library_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn a_drive_is_named_by_its_label_and_letter_without_a_label_or_without_a_disk() {
        assert_eq!(drive_text('C', Some("System"), true), "System (C:)");
        assert_eq!(drive_text('D', None, true), "D:");
        assert_eq!(drive_text('E', None, false), "E: (no disk)");
        // A label left from an earlier disk is not shown when there is none now.
        assert_eq!(drive_text('E', Some("CARD"), false), "E: (no disk)");
    }

    #[test]
    fn an_expand_error_gives_an_english_reason_without_the_system_message() {
        let denied = io::Error::new(io::ErrorKind::PermissionDenied, "Åtkomst nekad.");
        assert_eq!(expand_error_text(&denied), "No access to this folder");
        let gone = io::Error::new(io::ErrorKind::NotFound, "Det går inte att hitta sökvägen.");
        assert_eq!(expand_error_text(&gone), "Folder not found");
        let other = io::Error::from_raw_os_error(1117);
        assert_eq!(
            expand_error_text(&other),
            "Cannot read this folder (error 1117)"
        );
        let plain = io::Error::other("anything");
        assert_eq!(expand_error_text(&plain), "Cannot read this folder");
    }
}
