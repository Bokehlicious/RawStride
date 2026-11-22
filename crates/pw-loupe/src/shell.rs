//! The shell of RawStride's main window (ADR 0017 §2.1, §2.5, U1, T109): the menu table, the
//! two themes with their contrast check (WCAG AA), the side columns' widths and the window the
//! program starts with. Plain Rust, tested without a window; `main.rs` draws it. Every text is in
//! `shell_text.rs` (rule 13).

use egui::{Color32, Key};

use crate::settings::{ShellSettings, WindowRect};
use crate::sort::SortKey;

/// The program's version, from `[workspace.package]` in `Cargo.toml`, shown in the window "About
/// RawStride" (ADR 0031 RL-Q9, T225).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The first 12 characters of the commit the program was built from, or `UNKNOWN_COMMIT` when the
/// build had no Git repository; set by `build.rs` (ADR 0031 RL-Q9, T225).
pub const COMMIT: &str = env!("RAWSTRIDE_COMMIT");

/// The text `build.rs` sets as `COMMIT` when there is no commit (ADR 0031 RL-Q9, T225). It must be
/// the same text as the literal in `build.rs`.
pub const UNKNOWN_COMMIT: &str = "unknown";

/// The authors from `[workspace.package]`, separated by `:` as Cargo gives them (ADR 0031 RL-Q9,
/// T225).
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

/// The source repository from `[workspace.package]` (ADR 0031 RL-Q9, T225).
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

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

/// A page of the Settings window (ADR 0017 U10b-Q6, T278), chosen by the buttons at its top. The
/// chosen page is kept while the window is open, not saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPage {
    /// The IIM check box, the encoding and the theme.
    General,
    /// The list of external programs (ADR 0019 E-Q2).
    ExternalPrograms,
    /// The sections and items of the metadata window and their order (step U10b).
    MetadataWindow,
}

impl SettingsPage {
    /// The pages in the order of their buttons.
    pub const ALL: [SettingsPage; 3] = [
        SettingsPage::General,
        SettingsPage::ExternalPrograms,
        SettingsPage::MetadataWindow,
    ];
}

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
    /// The disc of a button on a thumbnail (ADR 0017 §2.8, U15-Q6, T245): black at about 55 %
    /// opacity, so the image shows through and the disc stands out on a light image.
    pub const THUMB_BUTTON_DISC: Color32 = Color32::from_black_alpha(0x8c);
    /// The disc of the button under the pointer: a lighter grey, nearly opaque.
    pub const THUMB_BUTTON_DISC_HOVERED: Color32 =
        Color32::from_rgba_unmultiplied_const(0x70, 0x70, 0x70, 0xe0);
    /// The symbol on a button's disc.
    pub const THUMB_BUTTON_SYMBOL: Color32 = Color32::from_gray(0xf0);
}

/// `colour` (premultiplied, as `Color32` holds it) drawn over the opaque `background`:
/// `premultiplied + background × (1 − alpha)` per channel, opaque.
fn over(colour: Color32, background: Color32) -> Color32 {
    let rest = 255 - u16::from(colour.a());
    let channel = |top: u8, bottom: u8| {
        let mixed = u16::from(top) + (u16::from(bottom) * rest + 127) / 255;
        u8::try_from(mixed).unwrap_or(u8::MAX)
    };
    Color32::from_rgb(
        channel(colour.r(), background.r()),
        channel(colour.g(), background.g()),
        channel(colour.b(), background.b()),
    )
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

/// egui's default fonts with `Hack` added as the last font of normal (proportional) text.
///
/// The default proportional fonts (`Ubuntu-Light` first, then `egui-icons`) have no arrows
/// `↔ → ↑ ↓`, which egui then draws as boxes (the Zoom button "Fit ↔ 100% (Z)", the menu,
/// the sort direction button and the import preview). `Hack`, which egui bundles for monospace
/// text, has them; as the last font it is used only for characters the others lack (owner,
/// 2026-11-16, T155). The fonts come from egui's `default_fonts` feature, which `pw-loupe` gets
/// through `eframe`. If the proportional family is missing, the defaults are returned unchanged.
pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("Hack".to_owned());
    }
    fonts
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
    let symbol = image_area::THUMB_BUTTON_SYMBOL;
    let disc = image_area::THUMB_BUTTON_DISC;
    let hovered = image_area::THUMB_BUTTON_DISC_HOVERED;
    let (white, black, cell) = (Color32::WHITE, Color32::BLACK, image_area::CELL);
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
        // The buttons on a thumbnail (ADR 0017 §2.8, U15-Q6, T245): the symbol on its disc over
        // a white image, a black image and the empty cell, and the disc itself on a white image.
        // On a black image the dark disc cannot stand out (1:1); the light symbol does.
        (
            "thumbnail button symbol on its disc over a white image",
            symbol,
            over(disc, white),
        ),
        (
            "thumbnail button symbol on its disc over a black image",
            symbol,
            over(disc, black),
        ),
        (
            "thumbnail button symbol on its disc over the empty cell",
            symbol,
            over(disc, cell),
        ),
        (
            "thumbnail button symbol on its hovered disc over a white image",
            symbol,
            over(hovered, white),
        ),
        (
            "thumbnail button symbol on its hovered disc over a black image",
            symbol,
            over(hovered, black),
        ),
        (
            "thumbnail button symbol on its hovered disc over the empty cell",
            symbol,
            over(hovered, cell),
        ),
        (
            "thumbnail button disc on a white image",
            over(disc, white),
            white,
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

const fn shift(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: false,
        shift: true,
        alt: false,
    })
}

/// The key that hides and shows both side columns (U1-Q2: `H`; not Tab, which egui uses to move
/// the keyboard focus and which changes tab with Ctrl).
pub const HIDE_KEY: Key = Key::H;

/// Ctrl+E: the targets opened in the first external program, or the Settings window when the
/// list is empty (ADR 0019 §2.2, E-Q1, E-Q3, T202). The first program's item (or "Open in
/// Another Program…") shows it on its right; it is not in `menu_items()`, since
/// `MenuEntry::ExternalPrograms` is drawn from the settings.
pub const OPEN_IN_KEY: KeyChord = KeyChord {
    key: Key::E,
    ctrl: true,
    shift: false,
    alt: false,
};

/// A command of the menu bar (§2.1). Each does what its key does, through the same function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    OpenFolder,
    /// The active tab's folder listed again and every image's rating and metadata read again
    /// (File ▸ Reload Folder, F5 elsewhere than over the left column; ADR 0026 §3.1, F-Q1, T241).
    ReloadFolder,
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
    /// The target images moved to `_picked` in their own folder, with no rating change (Image ▸
    /// Move to Picked, P; ADR 0017 §2.6, U9-Q6, T174). Greyed while the setting is off.
    MovePicked,
    /// The setting "Move Rejected and Picked Images to Subfolders", on ↔ off (Image menu, a check
    /// box; U9-Q5, T174). Saved at once.
    MoveToSubfolders,
    /// The last batch of moves moved back, with the ratings before an X restored (Edit ▸ Undo
    /// Move, Ctrl+Z; U9-Q8, T174).
    UndoMove,
    /// A label by its XMP name ("Red", …).
    Label(&'static str),
    ClearLabel,
    /// The target images turned 90° anticlockwise (Image ▸ Rotate Left, Ctrl+←; ADR 0024 §2.1,
    /// O-Q1, T237).
    RotateLeft,
    /// The target images turned 90° clockwise (Image ▸ Rotate Right, Ctrl+→; ADR 0024 §2.1, O-Q1,
    /// T237).
    RotateRight,
    ToggleMode,
    /// The target images and their companion files sent to the Recycle Bin after a confirmation
    /// (Image ▸ Delete to Recycle Bin, Delete; ADR 0018 §2.1, B4, T189).
    DeleteToRecycleBin,
    /// Every image of the active tab's folder whose rating is reject sent to the Recycle Bin after
    /// the same confirmation as Delete, no key (Image ▸ Delete Rejected Images; ADR 0018 §2.1,
    /// B-Q9, B5, T190). Greyed while the active tab has no folder.
    DeleteRejected,
    /// The target images opened in the external program at this index of
    /// `Settings::external_programs`, 0 the first and default (Image ▸ Open in {name}, Ctrl+E for
    /// the first; ADR 0019 §2.2–§2.6, E-Q1, T202).
    OpenIn(u8),
    /// The Settings window opened at its section "External Programs" (Image ▸ Open in Another
    /// Program…, shown while the list is empty, then with Ctrl+E; ADR 0019 E-Q3, T202).
    OpenInAnother,
    /// The current image's folder opened in File Explorer with the file selected, no key (Image ▸
    /// Show in File Explorer; ADR 0019 §2.8, E-Q11, T203). Greyed while the active tab shows no
    /// current image.
    ShowInExplorer,
    /// The preview open ↔ closed (Visa ▸ Förhandsvisning, G; ADR 0017 §2.2, T111).
    ToggleView,
    Fit,
    ToggleZoom,
    ZoomIn,
    ZoomOut,
    ToggleAnchor,
    /// The preview's overlay that marks the clipped highlights on the image, on ↔ off (View ▸
    /// Show Clipped Highlights, J; ADR 0017 §2.6, U7b-Q5, T148).
    ClipHighlights,
    /// The preview's overlay that marks the deepest shadows on the image, on ↔ off (View ▸ Show
    /// Deep Shadows, Shift+J; U7b-Q5, T148).
    ClipShadows,
    /// The preview's focus marking, the edges of high local contrast marked in green on the
    /// image, on ↔ off (View ▸ Show Focus Marking, E; ADR 0017 §2.6, U9-Q1, T168).
    FocusMarking,
    /// The compare open ↔ closed: two images in the preview, the held image beside or above the
    /// current one (View ▸ Compare, C; ADR 0017 §2.2, U6-Q4, U6-Q11, T153).
    Compare,
    /// The current image becomes the held image of the compare (View ▸ Hold Current Image, K;
    /// U6-Q6, T153).
    HoldImage,
    /// The held and the current image of the compare change places (View ▸ Swap Images, Shift+K;
    /// U6-Q6, T153).
    SwapImages,
    /// The compare's two images above and below each other ↔ side by side (View ▸ Above and
    /// Below, V; U6-Q7, T153).
    AboveBelow,
    /// The held image of the compare at the current image's zoom and position ↔ free (View ▸
    /// Lock Zoom and Position, L; U6-Q8, T154).
    LockZoom,
    /// The inspector (the right column) open ↔ collapsed, as `I` (ADR 0017 §2.4, U2-Q3, T110).
    ToggleInfo,
    NextTab,
    PreviousTab,
    ToggleColumns,
    LeftColumn,
    Fullscreen,
    /// "Buttons on Thumbnails" on ↔ off: the four buttons on a thumbnail in the contact sheet
    /// (View menu, a check item, no key; ADR 0017 §2.8, U15-Q7, T245).
    ThumbnailButtons,
    Theme(Theme),
    /// Sort every tab by a key (View ▸ Sort By and the command bar's drop-down; ADR 0017 U14,
    /// T138).
    Sort(SortKey),
    /// Descending ↔ ascending (View ▸ Sort By ▸ Descending and the command bar's ↑/↓ button).
    SortDescending,
    /// The window "About RawStride" opened: the version, the commit and the licence notice (Help ▸
    /// About RawStride…, no key; ADR 0031 RL-Q9, T225).
    About,
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
            | Self::ToggleAnchor
            | Self::ClipHighlights
            | Self::ClipShadows
            | Self::FocusMarking
            | Self::Compare
            | Self::HoldImage
            | Self::SwapImages
            | Self::AboveBelow
            | Self::LockZoom => Where::Loupe,
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
    /// The fifth menu, last, with "About RawStride…" (ADR 0031 RL-Q9, T225).
    Help,
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
    /// View ▸ Sort By: the sort keys as radio items and "Descending" as a check item (ADR 0017
    /// U14-Q7, T138), drawn from `SORT_KEYS`. No key (U8 may add one).
    Sort,
    /// Image ▸ the external programs: "Open in {name}" per program of the settings' list, the
    /// first with `OPEN_IN_KEY`, or "Open in Another Program…" while the list is empty (ADR 0019
    /// §2.2, T202), drawn from `Settings::external_programs`.
    ExternalPrograms,
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

/// The menu bar (U1-Q1): Arkiv, Redigera, Bild, Visa, and Help last (ADR 0031 RL-Q9, T225).
pub const MENUS: &[Menu] = &[
    Menu {
        kind: MenuKind::File,
        entries: &[
            item(Command::OpenFolder, ctrl(Key::O)),
            item(Command::ReloadFolder, plain(Key::F5)),
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
            item(Command::UndoMove, ctrl(Key::Z)),
            MenuEntry::Separator,
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
            MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS),
            item(Command::Reject, plain(Key::X)),
            item(Command::MovePicked, plain(Key::P)),
            item(Command::MoveToSubfolders, None),
            MenuEntry::Separator,
            item(Command::RotateLeft, ctrl(Key::ArrowLeft)),
            item(Command::RotateRight, ctrl(Key::ArrowRight)),
            MenuEntry::Separator,
            item(Command::ToggleMode, plain(Key::R)),
            MenuEntry::Separator,
            MenuEntry::ExternalPrograms,
            item(Command::ShowInExplorer, None),
            MenuEntry::Separator,
            item(Command::DeleteToRecycleBin, plain(Key::Delete)),
            item(Command::DeleteRejected, None),
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
            item(Command::ClipHighlights, plain(Key::J)),
            item(Command::ClipShadows, shift(Key::J)),
            item(Command::FocusMarking, plain(Key::E)),
            item(Command::Compare, plain(Key::C)),
            item(Command::HoldImage, plain(Key::K)),
            item(Command::SwapImages, shift(Key::K)),
            item(Command::AboveBelow, plain(Key::V)),
            item(Command::LockZoom, plain(Key::L)),
            MenuEntry::Separator,
            MenuEntry::Filter,
            MenuEntry::Sort,
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
            item(Command::ThumbnailButtons, None),
            MenuEntry::Separator,
            MenuEntry::Submenu(SubmenuKind::Theme, THEME_ITEMS),
        ],
    },
    Menu {
        kind: MenuKind::Help,
        entries: &[item(Command::About, None)],
    },
];

/// The right-click menu on an image in the contact sheet or the preview (the compare included):
/// the commands that act on images, in the order of ADR 0019 §2.9 and E-Q17. Every item has the
/// same key as its item in `MENUS` (E-Q18). Left out on purpose: Move to Subfolders, RAW ↔ JPEG,
/// Delete Rejected Images, Reload Folder (ADR 0026 §3.1, T241) and Buttons on Thumbnails (ADR 0017
/// §2.8, T245), which act on the folder or the view, not on the images clicked.
pub const IMAGE_CONTEXT_MENU: &[MenuEntry] = &[
    MenuEntry::ExternalPrograms,
    item(Command::ShowInExplorer, None),
    MenuEntry::Separator,
    MenuEntry::Submenu(SubmenuKind::Rating, RATING_ITEMS),
    MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS),
    item(Command::Reject, plain(Key::X)),
    item(Command::MovePicked, plain(Key::P)),
    MenuEntry::Separator,
    item(Command::Metadata, plain(Key::M)),
    MenuEntry::Separator,
    item(Command::DeleteToRecycleBin, plain(Key::Delete)),
];

/// Every item of the menu bar, submenus included, in menu order.
pub fn menu_items() -> Vec<MenuItem> {
    let mut items = Vec::new();
    for menu in MENUS {
        for entry in menu.entries {
            match entry {
                MenuEntry::Item(item) => items.push(*item),
                MenuEntry::Submenu(_, sub) => items.extend_from_slice(sub),
                MenuEntry::Separator
                | MenuEntry::Filter
                | MenuEntry::Sort
                | MenuEntry::ExternalPrograms => {}
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
    fn the_menus_are_arkiv_redigera_bild_visa_hjalp_and_every_command_is_listed_once() {
        let kinds: Vec<MenuKind> = MENUS.iter().map(|menu| menu.kind).collect();
        assert_eq!(
            kinds,
            vec![
                MenuKind::File,
                MenuKind::Edit,
                MenuKind::Image,
                MenuKind::View,
                MenuKind::Help
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

    /// The Help menu is the last menu and holds only "About RawStride…", with no key (ADR 0031
    /// RL-Q9, T225).
    #[test]
    fn the_help_menu_is_last_and_holds_only_about_with_no_key() {
        let Some(last) = MENUS.last() else {
            panic!("the menu bar has no menus");
        };
        assert_eq!(last.kind, MenuKind::Help);
        assert_eq!(
            last.entries,
            &[MenuEntry::Item(MenuItem {
                command: Command::About,
                key: None,
            })]
        );
    }

    /// The build facts the window "About RawStride" shows (ADR 0031 RL-Q9, T225): the commit is
    /// `UNKNOWN_COMMIT` or 12 hex digits, and the version, repository and authors come from
    /// `Cargo.toml`.
    #[test]
    fn the_build_facts_are_well_formed() {
        assert!(
            COMMIT == UNKNOWN_COMMIT
                || (COMMIT.len() == 12
                    && COMMIT.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))),
            "{COMMIT:?}"
        );
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert!(!VERSION.is_empty());
        assert!(REPOSITORY.starts_with("https://"), "{REPOSITORY:?}");
        assert!(!AUTHORS.is_empty());
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
        assert_eq!(Command::MovePicked.acts_in(), Where::Both);
        assert_eq!(Command::MoveToSubfolders.acts_in(), Where::Both);
        assert_eq!(Command::UndoMove.acts_in(), Where::Both);
        assert_eq!(Command::DeleteToRecycleBin.acts_in(), Where::Both);
    }

    /// "Reload Folder" is in the File menu right after "Open Folder…" with F5, acts in both views,
    /// and is not in the right-click menu on images (ADR 0026 §3.1, F-Q1, T241).
    #[test]
    fn reload_folder_follows_open_folder_with_f5_and_is_not_in_the_image_context_menu() {
        let Some(file) = MENUS.iter().find(|menu| menu.kind == MenuKind::File) else {
            panic!("there is a File menu");
        };
        assert_eq!(
            file.entries.get(..2),
            Some(
                &[
                    item(Command::OpenFolder, ctrl(Key::O)),
                    item(Command::ReloadFolder, plain(Key::F5)),
                ][..]
            )
        );
        assert_eq!(Command::ReloadFolder.acts_in(), Where::Both);
        for entry in IMAGE_CONTEXT_MENU {
            if let MenuEntry::Item(menu_item) = entry {
                assert_ne!(menu_item.command, Command::ReloadFolder);
            }
        }
    }

    /// Buttons on Thumbnails is a check item of the View menu right after Full Screen, with no
    /// key; it acts in both views and is not in the right-click menu (ADR 0017 §2.8, U15-Q7,
    /// T245).
    #[test]
    fn buttons_on_thumbnails_follows_full_screen_in_the_view_menu() {
        let Some(view) = MENUS.iter().find(|menu| menu.kind == MenuKind::View) else {
            panic!("there is a View menu");
        };
        let Some(at) = view
            .entries
            .iter()
            .position(|entry| *entry == item(Command::Fullscreen, plain(Key::F11)))
        else {
            panic!("Full Screen is in the View menu");
        };
        assert_eq!(
            view.entries.get(at + 1),
            Some(&item(Command::ThumbnailButtons, None))
        );
        assert_eq!(Command::ThumbnailButtons.acts_in(), Where::Both);
        for entry in IMAGE_CONTEXT_MENU {
            if let MenuEntry::Item(menu_item) = entry {
                assert_ne!(menu_item.command, Command::ThumbnailButtons);
            }
        }
    }

    /// A colour with alpha drawn over an opaque background: premultiplied + background × (1 −
    /// alpha), opaque (T245's contrast pairs).
    #[test]
    fn a_transparent_colour_over_a_background() {
        assert_eq!(
            over(Color32::from_black_alpha(0x8c), Color32::WHITE),
            Color32::from_gray(0x73)
        );
        assert_eq!(
            over(Color32::from_black_alpha(0x8c), Color32::BLACK),
            Color32::BLACK
        );
        assert_eq!(
            over(Color32::from_gray(0x40), Color32::WHITE),
            Color32::from_gray(0x40)
        );
        assert_eq!(
            over(Color32::TRANSPARENT, Color32::from_gray(0x40)),
            Color32::from_gray(0x40)
        );
        assert_eq!(
            over(
                Color32::from_rgba_premultiplied(0x62, 0x62, 0x62, 0xe0),
                Color32::WHITE
            ),
            Color32::from_gray(0x62 + 0x1f)
        );
    }

    /// Delete to Recycle Bin (Delete) and Delete Rejected Images (no key) are the last items of
    /// the Image menu, after RAW ↔ JPEG, a separator, the external programs (ADR 0019 §2.2,
    /// T202), Show in File Explorer (no key; ADR 0019 §2.8, T203) and another separator, and no
    /// other item shows Delete (ADR 0018 B4, B5, T189, T190).
    #[test]
    fn the_delete_items_are_the_last_items_of_the_image_menu() {
        let Some(image) = MENUS.iter().find(|menu| menu.kind == MenuKind::Image) else {
            panic!("there is an Image menu");
        };
        let Some(start) = image.entries.len().checked_sub(7) else {
            panic!("the Image menu has at least seven entries");
        };
        assert_eq!(
            image.entries.get(start..),
            Some(
                &[
                    item(Command::ToggleMode, plain(Key::R)),
                    MenuEntry::Separator,
                    MenuEntry::ExternalPrograms,
                    item(Command::ShowInExplorer, None),
                    MenuEntry::Separator,
                    item(Command::DeleteToRecycleBin, plain(Key::Delete)),
                    item(Command::DeleteRejected, None),
                ][..]
            )
        );
        for menu_item in menu_items() {
            if menu_item.key.is_some_and(|chord| chord.key == Key::Delete) {
                assert_eq!(menu_item.command, Command::DeleteToRecycleBin);
            }
        }
    }

    /// The Image menu begins with the Rating submenu, the Label submenu right below it, Reject,
    /// Move to Picked and Move to Subfolders (the owner's change of the order, ADR 0019 §2.9,
    /// 2026-11-21, T243); then, after a separator, Rotate Left (Ctrl+←) and Rotate Right (Ctrl+→)
    /// before the separator above RAW ↔ JPEG. The rotate items act in both views and are not in
    /// the right-click menu (ADR 0024 §2.1, O-Q1, T237).
    #[test]
    fn the_image_menu_begins_with_rating_and_label_then_the_rotate_items() {
        let Some(image) = MENUS.iter().find(|menu| menu.kind == MenuKind::Image) else {
            panic!("there is an Image menu");
        };
        assert_eq!(
            image.entries.get(..10),
            Some(
                &[
                    MenuEntry::Submenu(SubmenuKind::Rating, RATING_ITEMS),
                    MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS),
                    item(Command::Reject, plain(Key::X)),
                    item(Command::MovePicked, plain(Key::P)),
                    item(Command::MoveToSubfolders, None),
                    MenuEntry::Separator,
                    item(Command::RotateLeft, ctrl(Key::ArrowLeft)),
                    item(Command::RotateRight, ctrl(Key::ArrowRight)),
                    MenuEntry::Separator,
                    item(Command::ToggleMode, plain(Key::R)),
                ][..]
            )
        );
        assert_eq!(Command::RotateLeft.acts_in(), Where::Both);
        assert_eq!(Command::RotateRight.acts_in(), Where::Both);
        for entry in IMAGE_CONTEXT_MENU {
            if let MenuEntry::Item(menu_item) = entry {
                assert!(
                    !matches!(
                        menu_item.command,
                        Command::RotateLeft | Command::RotateRight
                    ),
                    "{menu_item:?} is in the right-click menu"
                );
            }
        }
    }

    /// The right-click menu on images holds the commands that act on images, in the order of
    /// ADR 0019 §2.9 and E-Q17 (T221), with the Label submenu right after the Rating submenu (the
    /// owner's change of the order, ADR 0019 §2.9, 2026-11-21, T243).
    #[test]
    fn the_image_context_menu_holds_the_image_commands_in_order() {
        assert_eq!(
            IMAGE_CONTEXT_MENU,
            &[
                MenuEntry::ExternalPrograms,
                item(Command::ShowInExplorer, None),
                MenuEntry::Separator,
                MenuEntry::Submenu(SubmenuKind::Rating, RATING_ITEMS),
                MenuEntry::Submenu(SubmenuKind::Label, LABEL_ITEMS),
                item(Command::Reject, plain(Key::X)),
                item(Command::MovePicked, plain(Key::P)),
                MenuEntry::Separator,
                item(Command::Metadata, plain(Key::M)),
                MenuEntry::Separator,
                item(Command::DeleteToRecycleBin, plain(Key::Delete)),
            ][..]
        );
    }

    /// Every item of the right-click menu is an item of the menu bar with the same key, every
    /// submenu is the Image menu's, and it holds neither the filter nor the sort, nor the commands
    /// that act on the folder or the view (ADR 0019 §2.9, E-Q18, T221).
    #[test]
    fn every_item_of_the_image_context_menu_is_the_menu_bars_item() {
        let items = menu_items();
        let Some(image) = MENUS.iter().find(|menu| menu.kind == MenuKind::Image) else {
            panic!("there is an Image menu");
        };
        for entry in IMAGE_CONTEXT_MENU {
            match entry {
                MenuEntry::Item(menu_item) => {
                    assert!(
                        items.iter().any(|bar| bar.command == menu_item.command
                            && bar.key == menu_item.key),
                        "{menu_item:?} is not an item of the menu bar with the same key"
                    );
                    assert!(
                        !matches!(
                            menu_item.command,
                            Command::MoveToSubfolders
                                | Command::ToggleMode
                                | Command::DeleteRejected
                        ),
                        "{menu_item:?} acts on the folder or the view"
                    );
                }
                MenuEntry::Submenu(kind, sub) => {
                    assert!(
                        image.entries.iter().any(|bar| matches!(
                            bar,
                            MenuEntry::Submenu(bar_kind, bar_sub)
                                if bar_kind == kind && bar_sub == sub
                        )),
                        "{entry:?} is not a submenu of the Image menu"
                    );
                }
                MenuEntry::Filter | MenuEntry::Sort => {
                    panic!("{entry:?} is in the right-click menu");
                }
                MenuEntry::Separator | MenuEntry::ExternalPrograms => {}
            }
        }
    }

    /// The external programs' items are drawn from the settings (ADR 0019 §2.2, T202): no
    /// `OpenIn` and no `OpenInAnother` in the static table.
    #[test]
    fn the_open_in_commands_are_not_in_the_static_menu_items() {
        for menu_item in menu_items() {
            assert!(
                !matches!(
                    menu_item.command,
                    Command::OpenIn(_) | Command::OpenInAnother
                ),
                "{menu_item:?} is in the static table"
            );
        }
        assert_eq!(Command::OpenIn(0).acts_in(), Where::Both);
        assert_eq!(Command::OpenInAnother.acts_in(), Where::Both);
    }

    /// The key of "Open in {name}" is Ctrl+E, with no Shift and no Alt (ADR 0019 §2.2, T202).
    #[test]
    fn the_open_in_key_is_ctrl_e() {
        assert_eq!(
            OPEN_IN_KEY,
            KeyChord {
                key: Key::E,
                ctrl: true,
                shift: false,
                alt: false,
            }
        );
    }

    /// The moves' items (ADR 0017 U9-Q5–U9-Q8, T174): Undo Move (Ctrl+Z) first in the Edit menu,
    /// followed by a separator before Mark All; Move to Picked (P) and the setting's check box,
    /// with no key, right after Reject in the Image menu. No other item shows P or Z with Ctrl.
    #[test]
    fn the_move_items_are_in_the_edit_and_image_menus() {
        let Some(edit) = MENUS.iter().find(|menu| menu.kind == MenuKind::Edit) else {
            panic!("there is an Edit menu");
        };
        assert_eq!(
            edit.entries.get(..3),
            Some(
                &[
                    item(Command::UndoMove, ctrl(Key::Z)),
                    MenuEntry::Separator,
                    item(Command::SelectAll, ctrl(Key::A)),
                ][..]
            )
        );
        let Some(image) = MENUS.iter().find(|menu| menu.kind == MenuKind::Image) else {
            panic!("there is an Image menu");
        };
        let Some(reject) = image
            .entries
            .iter()
            .position(|entry| *entry == item(Command::Reject, plain(Key::X)))
        else {
            panic!("the Image menu has Reject");
        };
        assert_eq!(
            image.entries.get(reject + 1..reject + 3),
            Some(
                &[
                    item(Command::MovePicked, plain(Key::P)),
                    item(Command::MoveToSubfolders, None),
                ][..]
            )
        );
        for menu_item in menu_items() {
            if menu_item.key.is_some_and(|chord| chord.key == Key::P) {
                assert_eq!(menu_item.command, Command::MovePicked);
            }
            if menu_item
                .key
                .is_some_and(|chord| chord.key == Key::Z && chord.ctrl)
            {
                assert_eq!(menu_item.command, Command::UndoMove);
            }
        }
    }

    /// View ▸ Show Clipped Highlights (J) and Show Deep Shadows (Shift+J) come after the zoom
    /// items, act in the preview, and no other item has their keys (ADR 0017 U7b-Q5, T148). The
    /// compare's four items follow them, before the separator, with C, K, Shift+K and V (U6-Q4,
    /// U6-Q6, U6-Q7, T153).
    #[test]
    fn the_clipping_commands_have_j_and_shift_j_after_the_zoom_items() {
        let j = KeyChord {
            key: Key::J,
            ctrl: false,
            shift: false,
            alt: false,
        };
        let shift_j = KeyChord { shift: true, ..j };
        assert_eq!(plain(Key::J), Some(j));
        assert_eq!(shift(Key::J), Some(shift_j));
        let items = menu_items();
        let key_of = |command| {
            items
                .iter()
                .find(|item| item.command == command)
                .and_then(|item| item.key)
        };
        assert_eq!(key_of(Command::ClipHighlights), Some(j));
        assert_eq!(key_of(Command::ClipShadows), Some(shift_j));
        // No other item shows the key J, with any modifiers.
        for item in &items {
            if item.key.is_some_and(|chord| chord.key == Key::J) {
                assert!(
                    matches!(item.command, Command::ClipHighlights | Command::ClipShadows),
                    "{:?} has J",
                    item.command
                );
            }
        }
        assert_eq!(Command::ClipHighlights.acts_in(), Where::Loupe);
        assert_eq!(Command::ClipShadows.acts_in(), Where::Loupe);
        // The compare's keys: C, K, Shift+K, V and L, and no other item shows C, K, V or L.
        let c = KeyChord { key: Key::C, ..j };
        let k = KeyChord { key: Key::K, ..j };
        let shift_k = KeyChord { shift: true, ..k };
        let v = KeyChord { key: Key::V, ..j };
        let l = KeyChord { key: Key::L, ..j };
        let compare_items = [
            (Command::Compare, c),
            (Command::HoldImage, k),
            (Command::SwapImages, shift_k),
            (Command::AboveBelow, v),
            (Command::LockZoom, l),
        ];
        for (command, chord) in compare_items {
            assert_eq!(key_of(command), Some(chord), "{command:?}");
            assert_eq!(command.acts_in(), Where::Loupe, "{command:?}");
        }
        for item in &items {
            if item
                .key
                .is_some_and(|chord| matches!(chord.key, Key::C | Key::K | Key::V | Key::L))
            {
                assert!(
                    compare_items.iter().any(
                        |(command, chord)| item.command == *command && item.key == Some(*chord)
                    ),
                    "{:?} has a key of the compare",
                    item.command
                );
            }
        }
        // In the View menu, right after the last zoom item; then the compare's five in their
        // order (Lock Zoom and Position after Above and Below, T154), before the next separator.
        let Some(view) = MENUS.iter().find(|menu| menu.kind == MenuKind::View) else {
            panic!("there is a View menu");
        };
        let Some(anchor) = view
            .entries
            .iter()
            .position(|entry| *entry == item(Command::ToggleAnchor, plain(Key::A)))
        else {
            panic!("the View menu has Zoom at Pointer");
        };
        assert_eq!(
            view.entries.get(anchor + 1..anchor + 10),
            Some(
                &[
                    item(Command::ClipHighlights, Some(j)),
                    item(Command::ClipShadows, Some(shift_j)),
                    item(Command::FocusMarking, Some(KeyChord { key: Key::E, ..j })),
                    item(Command::Compare, Some(c)),
                    item(Command::HoldImage, Some(k)),
                    item(Command::SwapImages, Some(shift_k)),
                    item(Command::AboveBelow, Some(v)),
                    item(Command::LockZoom, Some(l)),
                    MenuEntry::Separator,
                ][..]
            )
        );
    }

    /// View ▸ Show Focus Marking (E) comes right after Show Deep Shadows, acts in the preview, and
    /// no other item has the key E (ADR 0017 §2.6, U9-Q1, T168).
    #[test]
    fn the_focus_marking_command_has_e_right_after_show_deep_shadows() {
        let e = KeyChord {
            key: Key::E,
            ctrl: false,
            shift: false,
            alt: false,
        };
        assert_eq!(plain(Key::E), Some(e));
        let items = menu_items();
        for item in &items {
            if item.key.is_some_and(|chord| chord.key == Key::E) {
                assert_eq!(
                    item.command,
                    Command::FocusMarking,
                    "{:?} has E",
                    item.command
                );
            }
        }
        assert!(
            items
                .iter()
                .any(|item| item.command == Command::FocusMarking && item.key == Some(e))
        );
        assert_eq!(Command::FocusMarking.acts_in(), Where::Loupe);
        let Some(view) = MENUS.iter().find(|menu| menu.kind == MenuKind::View) else {
            panic!("there is a View menu");
        };
        let Some(shadows) = view
            .entries
            .iter()
            .position(|entry| *entry == item(Command::ClipShadows, shift(Key::J)))
        else {
            panic!("the View menu has Show Deep Shadows");
        };
        assert_eq!(
            view.entries.get(shadows + 1),
            Some(&item(Command::FocusMarking, Some(e)))
        );
    }

    #[test]
    fn font_definitions_append_hack_last_to_proportional() {
        use egui::FontFamily;
        let defaults = egui::FontDefinitions::default();
        let fonts = font_definitions();
        let default_proportional = defaults.families.get(&FontFamily::Proportional);
        let proportional = fonts.families.get(&FontFamily::Proportional);
        let mut expected = default_proportional.cloned().unwrap_or_default();
        assert!(!expected.is_empty(), "egui's default fonts are present");
        expected.push("Hack".to_owned());
        assert_eq!(proportional, Some(&expected));
        assert_eq!(
            fonts.families.get(&FontFamily::Monospace),
            defaults.families.get(&FontFamily::Monospace)
        );
    }

    /// The user-visible texts with arrows (T155), each from its own text function.
    fn texts_with_arrows() -> Vec<String> {
        use crate::import::{ItemAction, PreviewLine};
        use crate::import_text::preview_line;
        use crate::preview_text::{PreviewText, preview_text};
        use crate::shell_text::{command_text, sort_direction_text};
        use crate::tasks_text::import_route_text;
        let mut texts: Vec<String> = [
            PreviewText::ZoomSection,
            PreviewText::Fit,
            PreviewText::ToggleZoom,
            PreviewText::ZoomIn,
            PreviewText::ZoomOut,
            PreviewText::ZoomAtPointer,
            PreviewText::OnlyMarked,
            PreviewText::OnlyMarkedHover,
            PreviewText::CollapseColumn,
            PreviewText::ExpandColumn,
        ]
        .into_iter()
        .map(|text| preview_text(text).to_owned())
        .collect();
        texts.extend(
            menu_items()
                .into_iter()
                .map(|item| command_text(item.command)),
        );
        texts.push(sort_direction_text(true).to_owned());
        texts.push(sort_direction_text(false).to_owned());
        for action in [
            ItemAction::Copy,
            ItemAction::Renamed,
            ItemAction::Present,
            ItemAction::BackupOnly,
        ] {
            texts.push(preview_line(&PreviewLine {
                source_name: "A.ARW".to_owned(),
                rel: "folder/A.ARW".to_owned(),
                action,
            }));
        }
        texts.push(import_route_text("E:\\DCIM", "D:\\Photos"));
        texts
    }

    /// The texts of `texts` that miss a glyph in the proportional font at 14 points with `fonts`.
    fn texts_missing_glyphs(fonts: egui::FontDefinitions, texts: &[String]) -> Vec<String> {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
        let font_id = egui::FontId::proportional(14.0);
        texts
            .iter()
            .filter(|text| !ctx.fonts_mut(|fonts| fonts.has_glyphs(&font_id, text)))
            .cloned()
            .collect()
    }

    #[test]
    fn font_definitions_have_every_glyph_of_the_texts_with_arrows() {
        let missing = texts_missing_glyphs(font_definitions(), &texts_with_arrows());
        assert!(missing.is_empty(), "texts missing a glyph: {missing:?}");
    }

    #[test]
    fn default_fonts_miss_the_zoom_arrow() {
        let missing = texts_missing_glyphs(egui::FontDefinitions::default(), &texts_with_arrows());
        assert!(
            missing.iter().any(|text| text == "Fit ↔ 100% (Z)"),
            "egui's default fonts should miss the arrow of \"Fit ↔ 100% (Z)\"; missing: {missing:?}"
        );
    }
}
