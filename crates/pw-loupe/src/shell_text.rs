//! The texts of the main window's shell (ADR 0017 U1, T109; rule 13): the menu bar, the command
//! bar, the side columns and the folder dialog. Each text is made here, whole; the drawing code in
//! `main.rs` holds none of them.

use egui::Key;

use crate::ratings::label_name;
use crate::shell::{Command, KeyChord, MenuKind, SubmenuKind, Theme};

/// A fixed text of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellText {
    /// The left column's upper heading.
    Library,
    /// The left column's lower heading.
    Tasks,
    /// The right column's heading.
    Inspector,
    /// The line under "Bibliotek" until U4 fills it.
    LibraryNote,
    /// The line under "Uppgifter" when no import runs hidden.
    NoTasks,
    /// The folder dialog's title (Öppna mapp, Ctrl+O).
    OpenFolderTitle,
    /// Visa ▸ Filter.
    FilterMenu,
    /// Visa ▸ Filter ▸ Visa alla (clears the filter).
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
}

/// The text of `text`.
pub fn shell_text(text: ShellText) -> &'static str {
    match text {
        ShellText::Library => "Bibliotek",
        ShellText::Tasks => "Uppgifter",
        ShellText::Inspector => "Inspektör",
        ShellText::LibraryNote => "Öppna en mapp med Ctrl+O eller dra den till fönstret.",
        ShellText::NoTasks => "Inga pågående uppgifter.",
        ShellText::OpenFolderTitle => "Öppna mapp",
        ShellText::FilterMenu => "Filter",
        ShellText::ShowAll => "Visa alla",
        ShellText::ThemeLabel => "Tema",
        ShellText::CollapseLeft => "«",
        ShellText::CollapseRight => "»",
        ShellText::CollapseHover => "Fäll ihop kolumnen",
        ShellText::ExpandLeft => "»",
        ShellText::ExpandRight => "«",
        ShellText::ExpandHover => "Visa kolumnen",
    }
}

/// The name of a menu in the menu bar.
pub fn menu_text(menu: MenuKind) -> &'static str {
    match menu {
        MenuKind::File => "Arkiv",
        MenuKind::Edit => "Redigera",
        MenuKind::Image => "Bild",
        MenuKind::View => "Visa",
    }
}

/// The name of a submenu.
pub fn submenu_text(submenu: SubmenuKind) -> &'static str {
    match submenu {
        SubmenuKind::Rating => "Betyg",
        SubmenuKind::Label => "Etikett",
        SubmenuKind::Theme => "Tema",
    }
}

/// The name of a theme (the menu and the settings window).
pub fn theme_text(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "Mörkt",
        Theme::Light => "Ljust",
    }
}

/// The text of a menu item (also the command bar's buttons "Importera…" and "Öppna mapp…").
pub fn command_text(command: Command) -> String {
    match command {
        Command::OpenFolder => "Öppna mapp…".to_owned(),
        Command::Import => "Importera…".to_owned(),
        Command::CloseTab => "Stäng flik".to_owned(),
        Command::Settings => "Inställningar…".to_owned(),
        Command::Quit => "Avsluta".to_owned(),
        Command::SelectAll => "Markera alla".to_owned(),
        Command::ClearSelection => "Avmarkera".to_owned(),
        Command::Metadata => "Metadata…".to_owned(),
        Command::Stars(0) => "Inga stjärnor".to_owned(),
        Command::Stars(1) => "1 stjärna".to_owned(),
        Command::Stars(stars) => format!("{stars} stjärnor"),
        Command::Reject => "Avvisa".to_owned(),
        Command::Label(label) => label_name(label).to_owned(),
        Command::ClearLabel => "Ingen etikett".to_owned(),
        Command::ToggleMode => "RAW ↔ JPEG".to_owned(),
        Command::ToggleView => "Kontaktark ↔ lupp".to_owned(),
        Command::Fit => "Anpassa".to_owned(),
        Command::ToggleZoom => "Anpassa ↔ 100 %".to_owned(),
        Command::ZoomIn => "Zooma in".to_owned(),
        Command::ZoomOut => "Zooma ut".to_owned(),
        Command::ToggleAnchor => "Zooma vid pekaren".to_owned(),
        Command::ToggleInfo => "Inspektör".to_owned(),
        Command::NextTab => "Nästa flik".to_owned(),
        Command::PreviousTab => "Föregående flik".to_owned(),
        Command::ToggleColumns => "Sidokolumner".to_owned(),
        Command::LeftColumn => "Vänster kolumn".to_owned(),
        Command::Fullscreen => "Helskärm".to_owned(),
        Command::Theme(theme) => theme_text(theme).to_owned(),
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

    #[test]
    fn every_fixed_text_of_the_shell() {
        let texts = [
            (ShellText::Library, "Bibliotek"),
            (ShellText::Tasks, "Uppgifter"),
            (ShellText::Inspector, "Inspektör"),
            (
                ShellText::LibraryNote,
                "Öppna en mapp med Ctrl+O eller dra den till fönstret.",
            ),
            (ShellText::NoTasks, "Inga pågående uppgifter."),
            (ShellText::OpenFolderTitle, "Öppna mapp"),
            (ShellText::FilterMenu, "Filter"),
            (ShellText::ShowAll, "Visa alla"),
            (ShellText::ThemeLabel, "Tema"),
            (ShellText::CollapseLeft, "«"),
            (ShellText::CollapseRight, "»"),
            (ShellText::CollapseHover, "Fäll ihop kolumnen"),
            (ShellText::ExpandLeft, "»"),
            (ShellText::ExpandRight, "«"),
            (ShellText::ExpandHover, "Visa kolumnen"),
        ];
        for (text, expected) in texts {
            assert_eq!(shell_text(text), expected, "{text:?}");
        }
        assert_eq!(menu_text(MenuKind::File), "Arkiv");
        assert_eq!(menu_text(MenuKind::Edit), "Redigera");
        assert_eq!(menu_text(MenuKind::Image), "Bild");
        assert_eq!(menu_text(MenuKind::View), "Visa");
        assert_eq!(submenu_text(SubmenuKind::Rating), "Betyg");
        assert_eq!(submenu_text(SubmenuKind::Label), "Etikett");
        assert_eq!(submenu_text(SubmenuKind::Theme), "Tema");
        assert_eq!(theme_text(Theme::Dark), "Mörkt");
        assert_eq!(theme_text(Theme::Light), "Ljust");
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
                "Öppna mapp…",
                "Importera…",
                "Stäng flik",
                "Inställningar…",
                "Avsluta",
                "Markera alla",
                "Avmarkera",
                "Metadata…",
                "Inga stjärnor",
                "1 stjärna",
                "2 stjärnor",
                "3 stjärnor",
                "4 stjärnor",
                "5 stjärnor",
                "Avvisa",
                "Röd",
                "Gul",
                "Grön",
                "Blå",
                "Lila",
                "Ingen etikett",
                "RAW ↔ JPEG",
                "Kontaktark ↔ lupp",
                "Anpassa",
                "Anpassa ↔ 100 %",
                "Zooma in",
                "Zooma ut",
                "Zooma vid pekaren",
                "Nästa flik",
                "Föregående flik",
                "Sidokolumner",
                "Vänster kolumn",
                "Inspektör",
                "Helskärm",
                "Mörkt",
                "Ljust",
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
}
