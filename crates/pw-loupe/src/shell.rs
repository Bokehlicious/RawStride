//! The shell of RawStride's main window (ADR 0017 §2.1, §2.5, U1, T109): the menu table, the
//! two themes with their contrast check (WCAG AA), the side columns' widths and the window the
//! program starts with. Plain Rust, tested without a window; `main.rs` draws it. Every text is in
//! `shell_text.rs` (rule 13).

use egui::{Color32, Key};

use crate::settings::{ShellSettings, WindowRect};

/// The theme of the menus, bars, columns and dialogs (ADR 0017 §2.5, U-Q3). The contact sheet and
/// the image area keep the same neutral dark grey in both (U1-Q4), so an image is always judged
/// against the same surround.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// The themes in the order of the menu and the settings window.
pub const THEMES: [Theme; 2] = [Theme::Dark, Theme::Light];

/// Whether a side column is shown in full or as a thin strip with its open button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnState {
    #[default]
    Open,
    Collapsed,
}

impl ColumnState {
    /// The other state (the button in the column's header).
    pub fn toggled(self) -> Self {
        match self {
            Self::Open => Self::Collapsed,
            Self::Collapsed => Self::Open,
        }
    }
}

/// The narrowest a side column can be dragged, in points.
pub const MIN_COLUMN_WIDTH: f32 = 160.0;
/// The widest a side column can be dragged, as a share of the window's width.
pub const MAX_COLUMN_SHARE: f32 = 0.4;
/// The left column's width before the user has changed it.
pub const DEFAULT_LEFT_WIDTH: u32 = 240;
/// The right column's width before the user has changed it.
pub const DEFAULT_RIGHT_WIDTH: u32 = 300;
/// The width of a collapsed column (only its open button).
pub const COLLAPSED_WIDTH: f32 = 28.0;

/// The widths a side column may have in a window `window_width` points wide: at least
/// [`MIN_COLUMN_WIDTH`], at most [`MAX_COLUMN_SHARE`] of the window, but never a range below the
/// minimum (a window narrower than two minimum columns keeps the minimum).
pub fn column_width_range(window_width: f32) -> (f32, f32) {
    let max = if window_width.is_finite() {
        (window_width * MAX_COLUMN_SHARE).max(MIN_COLUMN_WIDTH)
    } else {
        MIN_COLUMN_WIDTH
    };
    (MIN_COLUMN_WIDTH, max)
}

/// `width` inside [`column_width_range`].
pub fn clamp_column_width(width: f32, window_width: f32) -> f32 {
    let (min, max) = column_width_range(window_width);
    if width.is_finite() {
        width.clamp(min, max)
    } else {
        min
    }
}

/// The smallest saved window that is used at start, in points; a smaller one gives a maximized
/// window.
pub const MIN_START_SIZE: [u32; 2] = [400, 300];

/// How the window opens at start (§2.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StartWindow {
    Maximized,
    /// The saved outer position and inner size, in points.
    At {
        position: [f32; 2],
        size: [f32; 2],
    },
}

/// The window at start: maximized when nothing is saved, when it was maximized, when it is
/// smaller than [`MIN_START_SIZE`], or when `on_screen` says that its rectangle in pixels
/// (`[left, top, right, bottom]`, from the saved scale) lies on no screen; otherwise the saved
/// position and size. Full screen is applied on top of this by the caller.
pub fn start_window(shell: &ShellSettings, on_screen: impl Fn([i32; 4]) -> bool) -> StartWindow {
    let Some(rect) = shell.window else {
        return StartWindow::Maximized;
    };
    if shell.window_maximized || rect.width < MIN_START_SIZE[0] || rect.height < MIN_START_SIZE[1] {
        return StartWindow::Maximized;
    }
    if !on_screen(pixel_rect(&rect)) {
        return StartWindow::Maximized;
    }
    StartWindow::At {
        position: [rect.x as f32, rect.y as f32],
        size: [rect.width as f32, rect.height as f32],
    }
}

/// The saved rectangle in pixels, `[left, top, right, bottom]`.
fn pixel_rect(rect: &WindowRect) -> [i32; 4] {
    let scale = if rect.scale.is_finite() && rect.scale > 0.0 {
        rect.scale
    } else {
        1.0
    };
    let px = |points: f32| (points * scale).round() as i32;
    let (x, y) = (rect.x as f32, rect.y as f32);
    [
        px(x),
        px(y),
        px(x + rect.width as f32),
        px(y + rect.height as f32),
    ]
}

/// Whether the rectangle in pixels lies at least partly on a screen (Windows:
/// `MonitorFromRect` with `MONITOR_DEFAULTTONULL`).
#[cfg(windows)]
pub fn rect_on_a_screen(rect: [i32; 4]) -> bool {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect};
    let rect = RECT {
        left: rect[0],
        top: rect[1],
        right: rect[2],
        bottom: rect[3],
    };
    // SAFETY: `rect` is a valid RECT on the stack for the duration of the call; the function only
    // reads it and returns a handle (null when no monitor intersects it).
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONULL) };
    !monitor.is_null()
}

/// Off Windows the screens are not looked up: a saved rectangle is used as it is.
#[cfg(not(windows))]
pub fn rect_on_a_screen(_rect: [i32; 4]) -> bool {
    true
}

/// The colours of a theme. Backgrounds and text are colour-neutral greys (R = G = B); the accent is
/// the one colour for selection and active controls (§2.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Panels, bars, columns and windows.
    pub panel: Color32,
    /// Text fields and the inside of lists.
    pub extreme: Color32,
    /// A button or list at rest, when hovered, and when pressed or open.
    pub inactive: Color32,
    pub hovered: Color32,
    pub active: Color32,
    pub text: Color32,
    pub weak_text: Color32,
    /// Widget borders and separators.
    pub border: Color32,
    /// Frames of marked and active controls, the focus outline and links. (The contact sheet's
    /// marked cells use [`image_area::MARKED`], the same in both themes.)
    pub accent: Color32,
    /// The background behind selected text (a selected tab, a chosen list entry).
    pub selection: Color32,
    pub selection_text: Color32,
    pub warning: Color32,
    pub error: Color32,
}

const fn grey(value: u8) -> Color32 {
    Color32::from_rgb(value, value, value)
}

/// The palette of `theme`.
pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            panel: grey(0x26),
            extreme: grey(0x1a),
            inactive: grey(0x3a),
            hovered: grey(0x46),
            active: grey(0x52),
            text: grey(0xe6),
            weak_text: grey(0xa8),
            border: grey(0x78),
            accent: Color32::from_rgb(0x4c, 0x8d, 0xff),
            selection: Color32::from_rgb(0x2a, 0x5d, 0xb0),
            selection_text: grey(0xff),
            warning: Color32::from_rgb(0xff, 0xc2, 0x4d),
            error: Color32::from_rgb(0xff, 0x7a, 0x70),
        },
        Theme::Light => Palette {
            panel: grey(0xf2),
            extreme: grey(0xff),
            inactive: grey(0xe0),
            hovered: grey(0xd4),
            active: grey(0xc8),
            text: grey(0x1e),
            weak_text: grey(0x5a),
            border: grey(0x8a),
            accent: Color32::from_rgb(0x1f, 0x5f, 0xc8),
            selection: Color32::from_rgb(0x1f, 0x5f, 0xc8),
            selection_text: grey(0xff),
            warning: Color32::from_rgb(0x8a, 0x5a, 0x00),
            error: Color32::from_rgb(0xb3, 0x26, 0x1e),
        },
    }
}

/// The contact sheet's and the image area's colours, the same in both themes (U1-Q4).
pub mod image_area {
    use egui::Color32;

    /// The background of the contact sheet and the loupe.
    pub const BACKGROUND: Color32 = Color32::from_gray(0x20);
    /// The square behind a thumbnail.
    pub const CELL: Color32 = Color32::from_gray(0x40);
    /// Text drawn on the background or a cell (file names, "ingen förhandsvisning", the loupe's
    /// centre texts).
    pub const TEXT: Color32 = Color32::from_gray(0xbe);
    /// The frame of a marked cell (ADR 0014 §5.1: the label blue, the dark theme's accent).
    pub const MARKED: Color32 = Color32::from_rgb(0x4c, 0x8d, 0xff);
}

/// The text sizes in points (U1-Q4): body text, small text and headings; the status bar uses the
/// proportional font at [`STATUS_TEXT_SIZE`].
pub const BODY_TEXT_SIZE: f32 = 14.0;
pub const SMALL_TEXT_SIZE: f32 = 11.0;
pub const HEADING_TEXT_SIZE: f32 = 16.0;
pub const STATUS_TEXT_SIZE: f32 = 13.0;

/// The text sizes of [`BODY_TEXT_SIZE`] and its siblings set in `style` (spacing stays egui's).
pub fn apply_text_sizes(style: &mut egui::Style) {
    use egui::{FontFamily, FontId, TextStyle};
    let sizes = [
        (TextStyle::Body, BODY_TEXT_SIZE, FontFamily::Proportional),
        (TextStyle::Button, BODY_TEXT_SIZE, FontFamily::Proportional),
        (TextStyle::Small, SMALL_TEXT_SIZE, FontFamily::Proportional),
        (
            TextStyle::Heading,
            HEADING_TEXT_SIZE,
            FontFamily::Proportional,
        ),
        (TextStyle::Monospace, BODY_TEXT_SIZE, FontFamily::Monospace),
    ];
    for (style_name, size, family) in sizes {
        style
            .text_styles
            .insert(style_name, FontId::new(size, family));
    }
}

/// The egui visuals of `theme`, built from its [`palette`].
pub fn visuals(theme: Theme) -> egui::Visuals {
    let p = palette(theme);
    let mut v = match theme {
        Theme::Dark => egui::Visuals::dark(),
        Theme::Light => egui::Visuals::light(),
    };
    v.override_text_color = None;
    v.weak_text_color = Some(p.weak_text);
    v.hyperlink_color = p.accent;
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.window_stroke = egui::Stroke::new(1.0, p.border);
    v.faint_bg_color = p.extreme;
    v.extreme_bg_color = p.extreme;
    v.text_edit_bg_color = Some(p.extreme);
    v.code_bg_color = p.extreme;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.error;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = egui::Stroke::new(1.0, p.selection_text);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.text);
    for (state, fill) in [
        (&mut w.inactive, p.inactive),
        (&mut w.hovered, p.hovered),
        (&mut w.active, p.active),
        (&mut w.open, p.active),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = egui::Stroke::new(1.0, p.border);
        state.fg_stroke = egui::Stroke::new(1.0, p.text);
    }
    w.hovered.bg_stroke = egui::Stroke::new(1.0, p.accent);
    w.active.bg_stroke = egui::Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = egui::Stroke::new(2.0, p.accent);
    v
}

/// The contrast ratio of two colours (WCAG 2.x: relative luminance with the sRGB curve, alpha
/// ignored), from 1 to 21.
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

fn luminance(colour: Color32) -> f32 {
    let channel = |value: u8| {
        let c = f32::from(value) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

/// WCAG AA for normal text.
pub const TEXT_CONTRAST: f32 = 4.5;
/// WCAG AA for borders, focus outlines and other parts of controls (1.4.11).
pub const NON_TEXT_CONTRAST: f32 = 3.0;

/// Every text colour of `theme` on every background it is drawn on: `(what, text, background)`.
/// Each must reach [`TEXT_CONTRAST`]. Text over an image (the rating badge) is not here.
pub fn text_pairs(theme: Theme) -> Vec<(&'static str, Color32, Color32)> {
    let p = palette(theme);
    let mut pairs = vec![
        ("text on panel", p.text, p.panel),
        ("text in a field", p.text, p.extreme),
        ("text on a button", p.text, p.inactive),
        ("text on a hovered button", p.text, p.hovered),
        ("text on a pressed button", p.text, p.active),
        ("weak text on panel", p.weak_text, p.panel),
        ("weak text in a field", p.weak_text, p.extreme),
        ("warning on panel", p.warning, p.panel),
        ("warning in a field", p.warning, p.extreme),
        ("error on panel", p.error, p.panel),
        ("error in a field", p.error, p.extreme),
        ("link on panel", p.accent, p.panel),
        ("selected text", p.selection_text, p.selection),
    ];
    pairs.extend([
        (
            "text on the image area",
            image_area::TEXT,
            image_area::BACKGROUND,
        ),
        (
            "text on a thumbnail cell",
            image_area::TEXT,
            image_area::CELL,
        ),
    ]);
    pairs
}

/// Every non-text part of `theme` against its background: `(what, colour, background)`. Each must
/// reach [`NON_TEXT_CONTRAST`].
pub fn non_text_pairs(theme: Theme) -> Vec<(&'static str, Color32, Color32)> {
    let p = palette(theme);
    vec![
        ("accent on panel", p.accent, p.panel),
        ("accent in a field", p.accent, p.extreme),
        ("accent on a button", p.accent, p.inactive),
        ("border on panel", p.border, p.panel),
        ("border around a field", p.border, p.extreme),
        (
            "marked cell frame on the contact sheet",
            image_area::MARKED,
            image_area::BACKGROUND,
        ),
    ]
}

/// A key with its modifiers, as a menu item shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyChord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

const fn plain(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: false,
        shift: false,
        alt: false,
    })
}

const fn ctrl(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: true,
        shift: false,
        alt: false,
    })
}

/// The key that hides and shows both side columns (U1-Q2: `H`; not Tab, which egui uses to move
/// the keyboard focus and which changes tab with Ctrl).
pub const HIDE_KEY: Key = Key::H;

/// A command of the menu bar (§2.1). Each does what its key does, through the same function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    OpenFolder,
    Import,
    CloseTab,
    Settings,
    Quit,
    SelectAll,
    ClearSelection,
    Metadata,
    /// 0–5 stars (0 clears the stars).
    Stars(u8),
    Reject,
    /// A label by its XMP name ("Red", …).
    Label(&'static str),
    ClearLabel,
    ToggleMode,
    /// The contact sheet ↔ the loupe.
    ToggleView,
    Fit,
    ToggleZoom,
    ZoomIn,
    ZoomOut,
    ToggleAnchor,
    /// The inspector (the right column) open ↔ collapsed, as `I` (ADR 0017 §2.4, U2-Q3, T110).
    ToggleInfo,
    NextTab,
    PreviousTab,
    ToggleColumns,
    LeftColumn,
    Fullscreen,
    Theme(Theme),
}

/// The view a command acts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    /// The contact sheet and the loupe.
    Both,
    Grid,
    Loupe,
}

impl Command {
    /// Where the command does something; elsewhere its menu item is disabled (not hidden).
    pub fn acts_in(self) -> Where {
        match self {
            Self::SelectAll | Self::ClearSelection => Where::Grid,
            Self::ToggleMode
            | Self::Fit
            | Self::ToggleZoom
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ToggleAnchor => Where::Loupe,
            _ => Where::Both,
        }
    }
}

/// One item of a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub command: Command,
    /// The key the item shows on its right (the main key where a command has several).
    pub key: Option<KeyChord>,
}

const fn item(command: Command, key: Option<KeyChord>) -> MenuEntry {
    MenuEntry::Item(MenuItem { command, key })
}

/// The menus of the menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    File,
    Edit,
    Image,
    View,
}

/// The submenus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmenuKind {
    Rating,
    Label,
    Theme,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu(SubmenuKind, &'static [MenuItem]),
    /// Visa ▸ Filter: the command bar's three lists as radio items and "Visa alla", drawn from the
    /// filter's own choices (ADR 0014).
    Filter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    pub kind: MenuKind,
    pub entries: &'static [MenuEntry],
}

const RATING_ITEMS: &[MenuItem] = &[
    MenuItem {
        command: Command::Stars(0),
        key: plain(Key::Num0),
    },
    MenuItem {
        command: Command::Stars(1),
        key: plain(Key::Num1),
    },
    MenuItem {
        command: Command::Stars(2),
        key: plain(Key::Num2),
    },
    MenuItem {
        command: Command::Stars(3),
        key: plain(Key::Num3),
    },
    MenuItem {
        command: Command::Stars(4),
        key: plain(Key::Num4),
    },
    MenuItem {
        command: Command::Stars(5),
        key: plain(Key::Num5),
    },
];

const LABEL_ITEMS: &[MenuItem] = &[
    MenuItem {
        command: Command::Label("Red"),
        key: plain(Key::Num6),
    },
    MenuItem {
        command: Command::Label("Yellow"),
        key: plain(Key::Num7),
    },
    MenuItem {
        command: Command::Label("Green"),
        key: plain(Key::Num8),
    },
    MenuItem {
        command: Command::Label("Blue"),
        key: plain(Key::Num9),
    },
    MenuItem {
        command: Command::Label("Purple"),
        key: ctrl(Key::Num5),
    },
    MenuItem {
        command: Command::ClearLabel,
        key: ctrl(Key::Num0),
    },
];

const THEME_ITEMS: &[MenuItem] = &[
    MenuItem {
        command: Command::Theme(Theme::Dark),
        key: None,
    },
    MenuItem {
        command: Command::Theme(Theme::Light),
        key: None,
    },
];

/// The menu bar (U1-Q1): Arkiv, Redigera, Bild, Visa.
pub const MENUS: &[Menu] = &[
    Menu {
        kind: MenuKind::File,
        entries: &[
            item(Command::OpenFolder, ctrl(Key::O)),
            item(Command::Import, ctrl(Key::I)),
            item(Command::CloseTab, ctrl(Key::W)),
            MenuEntry::Separator,
            item(Command::Settings, None),
            MenuEntry::Separator,
            item(
                Command::Quit,
                Some(KeyChord {
                    key: Key::F4,
                    ctrl: false,
                    shift: false,
                    alt: true,
                }),
            ),
        ],
    },
    Menu {
        kind: MenuKind::Edit,
        entries: &[
            item(Command::SelectAll, ctrl(Key::A)),
            item(Command::ClearSelection, plain(Key::Escape)),
            MenuEntry::Separator,
            item(Command::Metadata, plain(Key::M)),
        ],
    },
    Menu {
        kind: MenuKind::Image,
        entries: &[
            MenuEntry::Submenu(SubmenuKind::Rating, RATING_ITEMS),
            item(Command::Reject, plain(Key::X)),
            MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS),
            MenuEntry::Separator,
            item(Command::ToggleMode, plain(Key::R)),
        ],
    },
    Menu {
        kind: MenuKind::View,
        entries: &[
            item(Command::ToggleView, plain(Key::G)),
            MenuEntry::Separator,
            item(Command::Fit, plain(Key::F)),
            item(Command::ToggleZoom, plain(Key::Z)),
            item(Command::ZoomIn, plain(Key::Plus)),
            item(Command::ZoomOut, plain(Key::Minus)),
            item(Command::ToggleAnchor, plain(Key::A)),
            MenuEntry::Separator,
            MenuEntry::Filter,
            MenuEntry::Separator,
            item(Command::NextTab, ctrl(Key::Tab)),
            item(
                Command::PreviousTab,
                Some(KeyChord {
                    key: Key::Tab,
                    ctrl: true,
                    shift: true,
                    alt: false,
                }),
            ),
            MenuEntry::Separator,
            item(Command::ToggleColumns, plain(HIDE_KEY)),
            item(Command::LeftColumn, None),
            item(Command::ToggleInfo, plain(Key::I)),
            item(Command::Fullscreen, plain(Key::F11)),
            MenuEntry::Separator,
            MenuEntry::Submenu(SubmenuKind::Theme, THEME_ITEMS),
        ],
    },
];

/// Every item of the menu bar, submenus included, in menu order.
pub fn menu_items() -> Vec<MenuItem> {
    let mut items = Vec::new();
    for menu in MENUS {
        for entry in menu.entries {
            match entry {
                MenuEntry::Item(item) => items.push(*item),
                MenuEntry::Submenu(_, sub) => items.extend_from_slice(sub),
                MenuEntry::Separator | MenuEntry::Filter => {}
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ShellSettings;

    fn rect(x: i32, y: i32, width: u32, height: u32, scale: f32) -> WindowRect {
        WindowRect {
            x,
            y,
            width,
            height,
            scale,
        }
    }

    #[test]
    fn contrast_ratio_of_known_colours() {
        let black = Color32::BLACK;
        let white = Color32::WHITE;
        assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, black) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 0.001);
        // The classic limit case: #777777 on white is just under 4.5:1.
        let ratio = contrast_ratio(Color32::from_gray(0x77), white);
        assert!((ratio - 4.48).abs() < 0.01, "{ratio}");
        assert!(ratio < TEXT_CONTRAST);
    }

    #[test]
    fn every_text_pair_of_both_themes_reaches_4_5() {
        for theme in THEMES {
            for (what, text, background) in text_pairs(theme) {
                let ratio = contrast_ratio(text, background);
                assert!(
                    ratio >= TEXT_CONTRAST,
                    "{theme:?}: {what} is {ratio:.2}:1, below 4.5:1"
                );
            }
        }
    }

    #[test]
    fn every_non_text_pair_of_both_themes_reaches_3() {
        for theme in THEMES {
            for (what, colour, background) in non_text_pairs(theme) {
                let ratio = contrast_ratio(colour, background);
                assert!(
                    ratio >= NON_TEXT_CONTRAST,
                    "{theme:?}: {what} is {ratio:.2}:1, below 3:1"
                );
            }
        }
    }

    #[test]
    fn backgrounds_and_text_are_neutral_greys() {
        for theme in THEMES {
            let p = palette(theme);
            for colour in [
                p.panel,
                p.extreme,
                p.inactive,
                p.hovered,
                p.active,
                p.text,
                p.weak_text,
                p.border,
                image_area::BACKGROUND,
                image_area::CELL,
                image_area::TEXT,
            ] {
                assert!(
                    colour.r() == colour.g() && colour.g() == colour.b(),
                    "{theme:?}: {colour:?} is not neutral"
                );
            }
        }
    }

    #[test]
    fn visuals_use_the_palette() {
        for theme in THEMES {
            let p = palette(theme);
            let v = visuals(theme);
            assert_eq!(v.dark_mode, theme == Theme::Dark);
            assert_eq!(v.panel_fill, p.panel);
            assert_eq!(v.widgets.noninteractive.fg_stroke.color, p.text);
            assert_eq!(v.widgets.inactive.fg_stroke.color, p.text);
            assert_eq!(v.weak_text_color, Some(p.weak_text));
            assert_eq!(v.selection.bg_fill, p.selection);
            assert_eq!(v.warn_fg_color, p.warning);
            assert_eq!(v.error_fg_color, p.error);
        }
    }

    #[test]
    fn text_sizes_are_set() {
        let mut style = egui::Style::default();
        apply_text_sizes(&mut style);
        let size = |name| style.text_styles.get(&name).map(|font| font.size);
        assert_eq!(size(egui::TextStyle::Body), Some(14.0));
        assert_eq!(size(egui::TextStyle::Button), Some(14.0));
        assert_eq!(size(egui::TextStyle::Small), Some(11.0));
        assert_eq!(size(egui::TextStyle::Heading), Some(16.0));
    }

    #[test]
    fn column_widths_are_clamped_to_the_minimum_and_40_percent() {
        assert_eq!(column_width_range(1000.0), (160.0, 400.0));
        assert_eq!(clamp_column_width(100.0, 1000.0), 160.0);
        assert_eq!(clamp_column_width(250.0, 1000.0), 250.0);
        assert_eq!(clamp_column_width(600.0, 1000.0), 400.0);
        // A window narrower than two minimum columns keeps the minimum.
        assert_eq!(column_width_range(300.0), (160.0, 160.0));
        assert_eq!(clamp_column_width(250.0, 300.0), 160.0);
        assert_eq!(clamp_column_width(f32::NAN, 1000.0), 160.0);
        assert_eq!(column_width_range(f32::INFINITY), (160.0, 160.0));
    }

    #[test]
    fn column_state_toggles() {
        assert_eq!(ColumnState::Open.toggled(), ColumnState::Collapsed);
        assert_eq!(ColumnState::Collapsed.toggled(), ColumnState::Open);
    }

    #[test]
    fn start_window_without_a_saved_window_is_maximized() {
        let shell = ShellSettings::default();
        assert_eq!(start_window(&shell, |_| true), StartWindow::Maximized);
    }

    #[test]
    fn start_window_when_it_was_maximized_is_maximized() {
        let shell = ShellSettings {
            window: Some(rect(100, 80, 1200, 800, 1.0)),
            window_maximized: true,
            ..ShellSettings::default()
        };
        assert_eq!(start_window(&shell, |_| true), StartWindow::Maximized);
    }

    #[test]
    fn start_window_on_a_screen_uses_the_saved_rectangle() {
        let shell = ShellSettings {
            window: Some(rect(100, 80, 1200, 800, 1.5)),
            ..ShellSettings::default()
        };
        let seen = std::cell::Cell::new(None);
        let start = start_window(&shell, |pixels| {
            seen.set(Some(pixels));
            true
        });
        assert_eq!(
            start,
            StartWindow::At {
                position: [100.0, 80.0],
                size: [1200.0, 800.0],
            }
        );
        // Checked in pixels from the saved scale.
        assert_eq!(seen.get(), Some([150, 120, 1950, 1320]));
    }

    #[test]
    fn start_window_off_every_screen_is_maximized() {
        let shell = ShellSettings {
            window: Some(rect(-5000, -5000, 1200, 800, 1.0)),
            ..ShellSettings::default()
        };
        assert_eq!(start_window(&shell, |_| false), StartWindow::Maximized);
    }

    #[test]
    fn start_window_smaller_than_400_by_300_is_maximized() {
        for (width, height) in [(399, 800), (1200, 299), (0, 0)] {
            let shell = ShellSettings {
                window: Some(rect(100, 80, width, height, 1.0)),
                ..ShellSettings::default()
            };
            assert_eq!(
                start_window(&shell, |_| true),
                StartWindow::Maximized,
                "{width} × {height}"
            );
        }
        let shell = ShellSettings {
            window: Some(rect(100, 80, 400, 300, 1.0)),
            ..ShellSettings::default()
        };
        assert!(matches!(
            start_window(&shell, |_| true),
            StartWindow::At { .. }
        ));
    }

    #[test]
    fn a_bad_saved_scale_counts_as_one() {
        let shell = ShellSettings {
            window: Some(rect(10, 20, 500, 400, 0.0)),
            ..ShellSettings::default()
        };
        let seen = std::cell::Cell::new(None);
        let _ = start_window(&shell, |pixels| {
            seen.set(Some(pixels));
            true
        });
        assert_eq!(seen.get(), Some([10, 20, 510, 420]));
    }

    #[test]
    fn the_menus_are_arkiv_redigera_bild_visa_and_every_command_is_listed_once() {
        let kinds: Vec<MenuKind> = MENUS.iter().map(|menu| menu.kind).collect();
        assert_eq!(
            kinds,
            vec![
                MenuKind::File,
                MenuKind::Edit,
                MenuKind::Image,
                MenuKind::View
            ]
        );
        let items = menu_items();
        for (index, item) in items.iter().enumerate() {
            assert!(
                !items[index + 1..]
                    .iter()
                    .any(|other| other.command == item.command),
                "{:?} is listed twice",
                item.command
            );
        }
        // Every key is shown on at most one item.
        for (index, item) in items.iter().enumerate() {
            if let Some(key) = item.key {
                assert!(
                    !items[index + 1..]
                        .iter()
                        .any(|other| other.key == Some(key)),
                    "{key:?} is shown twice"
                );
            }
        }
        assert!(
            MENUS
                .iter()
                .any(|menu| menu.entries.contains(&MenuEntry::Filter))
        );
    }

    #[test]
    fn commands_act_in_the_right_view() {
        assert_eq!(Command::SelectAll.acts_in(), Where::Grid);
        assert_eq!(Command::ClearSelection.acts_in(), Where::Grid);
        assert_eq!(Command::Fit.acts_in(), Where::Loupe);
        assert_eq!(Command::ZoomIn.acts_in(), Where::Loupe);
        assert_eq!(Command::ToggleMode.acts_in(), Where::Loupe);
        assert_eq!(Command::Stars(3).acts_in(), Where::Both);
        assert_eq!(Command::Metadata.acts_in(), Where::Both);
        assert_eq!(Command::ToggleInfo.acts_in(), Where::Both);
        assert_eq!(Command::OpenFolder.acts_in(), Where::Both);
    }
}
