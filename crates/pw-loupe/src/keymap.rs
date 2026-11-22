//! The key map of the contact sheet and the preview (ADR 0017 §2.4, §6.4 U8-Q1–U8-Q13, T279): the
//! changeable actions ([`KeyAction`]) with up to two keys each, their default keys (the program's
//! keys before the map), the fixed keys that no action can take ([`fixed_key`], [`is_fixed`]), the
//! lookup of a key in a view ([`KeyMap::action_of`]), the rules for a new key ([`KeyMap::allowed`],
//! [`KeyMap::can_set`]) and the file `keys.txt`.
//!
//! `keys.txt` is kept beside `settings.txt` (`%LOCALAPPDATA%\RawStride\`), for this user on this
//! computer only. It is UTF-8 text: the comment line `# RawStride keys`, then one line per action
//! whose keys differ from its defaults, `name = key[, key]`, a key written as its modifiers (Ctrl,
//! Shift, Alt in that order) and egui's key name joined by `+` (`Ctrl+Shift+K`, `F5`, `Alt+3`); an
//! action with no key is written `name =`. Loading never fails: a line that cannot be used is
//! skipped with a problem text in English for the log, as for `settings.txt`. Saving writes a
//! temporary file and renames it onto the file, so a crash never leaves half a file. Plain Rust
//! without drawing (GR11).

use std::io;
use std::path::{Path, PathBuf};

use egui::Key;

use crate::field_values::write_synced;
use crate::grid::Navigation;
use crate::settings::settings_path;
use crate::shell::{Command, KeyChord};

/// The file's name, in the folder of `settings.txt` (U8-Q6).
pub const KEYS_FILE: &str = "keys.txt";

/// The first line of `keys.txt`.
const FILE_HEADER: &str = "# RawStride keys";

/// The menu an action is in, the group the Settings page lists it under (U8-Q7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyGroup {
    File,
    Edit,
    Image,
    View,
}

/// Where an action acts, and the view a key is looked up in (`Sheet` or `Preview`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyView {
    /// The contact sheet.
    Sheet,
    /// The preview (the loupe and the compare).
    Preview,
    /// The contact sheet and the preview.
    Both,
}

impl KeyView {
    /// Whether an action that acts in `self` and one that acts in `other` share a view.
    fn overlaps(self, other: KeyView) -> bool {
        self == KeyView::Both || other == KeyView::Both || self == other
    }
}

/// A changeable action of the key map (U8-Q1), in the order of [`KeyAction::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyAction {
    OpenFolder,
    ReloadFolder,
    Import,
    CloseTab,
    UndoMove,
    SelectAll,
    Metadata,
    Stars0,
    Stars1,
    Stars2,
    Stars3,
    Stars4,
    Stars5,
    LabelRed,
    LabelYellow,
    LabelGreen,
    LabelBlue,
    LabelPurple,
    ClearLabel,
    Reject,
    MovePicked,
    RotateLeft,
    RotateRight,
    RawJpeg,
    /// "Open in {name}" of the first program, or "Open in Another Program…" while the list is
    /// empty.
    OpenIn,
    Delete,
    Preview,
    Fit,
    ZoomToggle,
    ZoomIn,
    ZoomOut,
    ZoomAtPointer,
    ClippedHighlights,
    DeepShadows,
    FocusMarking,
    Compare,
    HoldImage,
    SwapImages,
    AboveBelow,
    LockZoom,
    /// The preview's "Marked only"; not a menu command.
    MarkedOnly,
    /// The hide key: both side columns, in the preview the right column and the filmstrip.
    HideColumns,
    /// The inspector, in the preview its right column.
    Inspector,
    FullScreen,
}

const fn plain(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: false,
        shift: false,
        alt: false,
    })
}

const fn with_ctrl(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: true,
        shift: false,
        alt: false,
    })
}

const fn with_shift(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: false,
        shift: true,
        alt: false,
    })
}

const fn with_alt(key: Key) -> Option<KeyChord> {
    Some(KeyChord {
        key,
        ctrl: false,
        shift: false,
        alt: true,
    })
}

impl KeyAction {
    /// Every action, in the order the Settings page lists them (T281) and `keys.txt` writes them.
    pub const ALL: [KeyAction; 44] = [
        Self::OpenFolder,
        Self::ReloadFolder,
        Self::Import,
        Self::CloseTab,
        Self::UndoMove,
        Self::SelectAll,
        Self::Metadata,
        Self::Stars0,
        Self::Stars1,
        Self::Stars2,
        Self::Stars3,
        Self::Stars4,
        Self::Stars5,
        Self::LabelRed,
        Self::LabelYellow,
        Self::LabelGreen,
        Self::LabelBlue,
        Self::LabelPurple,
        Self::ClearLabel,
        Self::Reject,
        Self::MovePicked,
        Self::RotateLeft,
        Self::RotateRight,
        Self::RawJpeg,
        Self::OpenIn,
        Self::Delete,
        Self::Preview,
        Self::Fit,
        Self::ZoomToggle,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomAtPointer,
        Self::ClippedHighlights,
        Self::DeepShadows,
        Self::FocusMarking,
        Self::Compare,
        Self::HoldImage,
        Self::SwapImages,
        Self::AboveBelow,
        Self::LockZoom,
        Self::MarkedOnly,
        Self::HideColumns,
        Self::Inspector,
        Self::FullScreen,
    ];

    /// The action's place in [`KeyAction::ALL`] (the declaration order).
    fn index(self) -> usize {
        self as usize
    }

    /// The action's fixed name in `keys.txt`.
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenFolder => "open_folder",
            Self::ReloadFolder => "reload_folder",
            Self::Import => "import",
            Self::CloseTab => "close_tab",
            Self::UndoMove => "undo_move",
            Self::SelectAll => "select_all",
            Self::Metadata => "metadata",
            Self::Stars0 => "stars_0",
            Self::Stars1 => "stars_1",
            Self::Stars2 => "stars_2",
            Self::Stars3 => "stars_3",
            Self::Stars4 => "stars_4",
            Self::Stars5 => "stars_5",
            Self::LabelRed => "label_red",
            Self::LabelYellow => "label_yellow",
            Self::LabelGreen => "label_green",
            Self::LabelBlue => "label_blue",
            Self::LabelPurple => "label_purple",
            Self::ClearLabel => "clear_label",
            Self::Reject => "reject",
            Self::MovePicked => "move_picked",
            Self::RotateLeft => "rotate_left",
            Self::RotateRight => "rotate_right",
            Self::RawJpeg => "raw_jpeg",
            Self::OpenIn => "open_in",
            Self::Delete => "delete",
            Self::Preview => "preview",
            Self::Fit => "fit",
            Self::ZoomToggle => "zoom_toggle",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::ZoomAtPointer => "zoom_at_pointer",
            Self::ClippedHighlights => "clipped_highlights",
            Self::DeepShadows => "deep_shadows",
            Self::FocusMarking => "focus_marking",
            Self::Compare => "compare",
            Self::HoldImage => "hold_image",
            Self::SwapImages => "swap_images",
            Self::AboveBelow => "above_below",
            Self::LockZoom => "lock_zoom",
            Self::MarkedOnly => "marked_only",
            Self::HideColumns => "hide_columns",
            Self::Inspector => "inspector",
            Self::FullScreen => "full_screen",
        }
    }

    /// The action with this name in `keys.txt` (exact, case matters).
    pub fn from_name(name: &str) -> Option<KeyAction> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }

    /// The menu the action is in.
    pub fn group(self) -> KeyGroup {
        match self {
            Self::OpenFolder | Self::ReloadFolder | Self::Import | Self::CloseTab => KeyGroup::File,
            Self::UndoMove | Self::SelectAll | Self::Metadata => KeyGroup::Edit,
            Self::Stars0
            | Self::Stars1
            | Self::Stars2
            | Self::Stars3
            | Self::Stars4
            | Self::Stars5
            | Self::LabelRed
            | Self::LabelYellow
            | Self::LabelGreen
            | Self::LabelBlue
            | Self::LabelPurple
            | Self::ClearLabel
            | Self::Reject
            | Self::MovePicked
            | Self::RotateLeft
            | Self::RotateRight
            | Self::RawJpeg
            | Self::OpenIn
            | Self::Delete => KeyGroup::Image,
            Self::Preview
            | Self::Fit
            | Self::ZoomToggle
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomAtPointer
            | Self::ClippedHighlights
            | Self::DeepShadows
            | Self::FocusMarking
            | Self::Compare
            | Self::HoldImage
            | Self::SwapImages
            | Self::AboveBelow
            | Self::LockZoom
            | Self::MarkedOnly
            | Self::HideColumns
            | Self::Inspector
            | Self::FullScreen => KeyGroup::View,
        }
    }

    /// Where the action acts. Open Folder, Import, Close Tab and Full Screen act in both: the
    /// preview takes their keys and does nothing with them (U3-Q1).
    pub fn acts_in(self) -> KeyView {
        match self {
            Self::SelectAll => KeyView::Sheet,
            Self::RawJpeg
            | Self::Fit
            | Self::ZoomToggle
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomAtPointer
            | Self::ClippedHighlights
            | Self::DeepShadows
            | Self::FocusMarking
            | Self::Compare
            | Self::HoldImage
            | Self::SwapImages
            | Self::AboveBelow
            | Self::LockZoom
            | Self::MarkedOnly => KeyView::Preview,
            _ => KeyView::Both,
        }
    }

    /// The action's default keys, the program's keys before the map; the first is the one the
    /// menus show (U8-Q2).
    pub fn default_keys(self) -> [Option<KeyChord>; 2] {
        match self {
            Self::OpenFolder => [with_ctrl(Key::O), None],
            Self::ReloadFolder => [plain(Key::F5), None],
            Self::Import => [with_ctrl(Key::I), None],
            Self::CloseTab => [with_ctrl(Key::W), None],
            Self::UndoMove => [with_ctrl(Key::Z), None],
            Self::SelectAll => [with_ctrl(Key::A), None],
            Self::Metadata => [plain(Key::M), None],
            Self::Stars0 => [plain(Key::Num0), with_alt(Key::Num0)],
            Self::Stars1 => [plain(Key::Num1), with_alt(Key::Num1)],
            Self::Stars2 => [plain(Key::Num2), with_alt(Key::Num2)],
            Self::Stars3 => [plain(Key::Num3), with_alt(Key::Num3)],
            Self::Stars4 => [plain(Key::Num4), with_alt(Key::Num4)],
            Self::Stars5 => [plain(Key::Num5), with_alt(Key::Num5)],
            Self::LabelRed => [plain(Key::Num6), with_ctrl(Key::Num1)],
            Self::LabelYellow => [plain(Key::Num7), with_ctrl(Key::Num2)],
            Self::LabelGreen => [plain(Key::Num8), with_ctrl(Key::Num3)],
            Self::LabelBlue => [plain(Key::Num9), with_ctrl(Key::Num4)],
            Self::LabelPurple => [with_ctrl(Key::Num5), None],
            Self::ClearLabel => [with_ctrl(Key::Num0), None],
            Self::Reject => [plain(Key::X), None],
            Self::MovePicked => [plain(Key::P), None],
            Self::RotateLeft => [with_ctrl(Key::ArrowLeft), None],
            Self::RotateRight => [with_ctrl(Key::ArrowRight), None],
            Self::RawJpeg => [plain(Key::R), None],
            Self::OpenIn => [with_ctrl(Key::E), None],
            Self::Delete => [plain(Key::Delete), None],
            Self::Preview => [plain(Key::G), None],
            Self::Fit => [plain(Key::F), None],
            Self::ZoomToggle => [plain(Key::Z), None],
            Self::ZoomIn => [plain(Key::Plus), None],
            Self::ZoomOut => [plain(Key::Minus), None],
            Self::ZoomAtPointer => [plain(Key::A), None],
            Self::ClippedHighlights => [plain(Key::J), None],
            Self::DeepShadows => [with_shift(Key::J), None],
            Self::FocusMarking => [plain(Key::E), None],
            Self::Compare => [plain(Key::C), None],
            Self::HoldImage => [plain(Key::K), None],
            Self::SwapImages => [with_shift(Key::K), None],
            Self::AboveBelow => [plain(Key::V), None],
            Self::LockZoom => [plain(Key::L), None],
            Self::MarkedOnly => [plain(Key::S), None],
            Self::HideColumns => [plain(crate::shell::HIDE_KEY), None],
            Self::Inspector => [plain(Key::I), None],
            Self::FullScreen => [plain(Key::F11), None],
        }
    }

    /// The action of a menu command, for the menus' keys (T280). `OpenIn(0)` and `OpenInAnother`
    /// both give `OpenIn`, since its key runs `OpenInAnother` while the program list is empty.
    /// Commands whose key is fixed (Clear Selection, Next Tab, Previous Tab, Quit) and commands
    /// without a key give `None`.
    pub fn of_command(command: Command) -> Option<KeyAction> {
        Some(match command {
            Command::OpenFolder => Self::OpenFolder,
            Command::ReloadFolder => Self::ReloadFolder,
            Command::Import => Self::Import,
            Command::CloseTab => Self::CloseTab,
            Command::UndoMove => Self::UndoMove,
            Command::SelectAll => Self::SelectAll,
            Command::Metadata => Self::Metadata,
            Command::Stars(0) => Self::Stars0,
            Command::Stars(1) => Self::Stars1,
            Command::Stars(2) => Self::Stars2,
            Command::Stars(3) => Self::Stars3,
            Command::Stars(4) => Self::Stars4,
            Command::Stars(5) => Self::Stars5,
            Command::Label("Red") => Self::LabelRed,
            Command::Label("Yellow") => Self::LabelYellow,
            Command::Label("Green") => Self::LabelGreen,
            Command::Label("Blue") => Self::LabelBlue,
            Command::Label("Purple") => Self::LabelPurple,
            Command::ClearLabel => Self::ClearLabel,
            Command::Reject => Self::Reject,
            Command::MovePicked => Self::MovePicked,
            Command::RotateLeft => Self::RotateLeft,
            Command::RotateRight => Self::RotateRight,
            Command::ToggleMode => Self::RawJpeg,
            Command::OpenIn(0) | Command::OpenInAnother => Self::OpenIn,
            Command::DeleteToRecycleBin => Self::Delete,
            Command::ToggleView => Self::Preview,
            Command::Fit => Self::Fit,
            Command::ToggleZoom => Self::ZoomToggle,
            Command::ZoomIn => Self::ZoomIn,
            Command::ZoomOut => Self::ZoomOut,
            Command::ToggleAnchor => Self::ZoomAtPointer,
            Command::ClipHighlights => Self::ClippedHighlights,
            Command::ClipShadows => Self::DeepShadows,
            Command::FocusMarking => Self::FocusMarking,
            Command::Compare => Self::Compare,
            Command::HoldImage => Self::HoldImage,
            Command::SwapImages => Self::SwapImages,
            Command::AboveBelow => Self::AboveBelow,
            Command::LockZoom => Self::LockZoom,
            Command::ToggleColumns => Self::HideColumns,
            Command::ToggleInfo => Self::Inspector,
            Command::Fullscreen => Self::FullScreen,
            _ => return None,
        })
    }

    /// The menu command of the action; `OpenIn`'s is `OpenIn(0)`, `MarkedOnly` has none.
    pub fn command(self) -> Option<Command> {
        Some(match self {
            Self::OpenFolder => Command::OpenFolder,
            Self::ReloadFolder => Command::ReloadFolder,
            Self::Import => Command::Import,
            Self::CloseTab => Command::CloseTab,
            Self::UndoMove => Command::UndoMove,
            Self::SelectAll => Command::SelectAll,
            Self::Metadata => Command::Metadata,
            Self::Stars0 => Command::Stars(0),
            Self::Stars1 => Command::Stars(1),
            Self::Stars2 => Command::Stars(2),
            Self::Stars3 => Command::Stars(3),
            Self::Stars4 => Command::Stars(4),
            Self::Stars5 => Command::Stars(5),
            Self::LabelRed => Command::Label("Red"),
            Self::LabelYellow => Command::Label("Yellow"),
            Self::LabelGreen => Command::Label("Green"),
            Self::LabelBlue => Command::Label("Blue"),
            Self::LabelPurple => Command::Label("Purple"),
            Self::ClearLabel => Command::ClearLabel,
            Self::Reject => Command::Reject,
            Self::MovePicked => Command::MovePicked,
            Self::RotateLeft => Command::RotateLeft,
            Self::RotateRight => Command::RotateRight,
            Self::RawJpeg => Command::ToggleMode,
            Self::OpenIn => Command::OpenIn(0),
            Self::Delete => Command::DeleteToRecycleBin,
            Self::Preview => Command::ToggleView,
            Self::Fit => Command::Fit,
            Self::ZoomToggle => Command::ToggleZoom,
            Self::ZoomIn => Command::ZoomIn,
            Self::ZoomOut => Command::ZoomOut,
            Self::ZoomAtPointer => Command::ToggleAnchor,
            Self::ClippedHighlights => Command::ClipHighlights,
            Self::DeepShadows => Command::ClipShadows,
            Self::FocusMarking => Command::FocusMarking,
            Self::Compare => Command::Compare,
            Self::HoldImage => Command::HoldImage,
            Self::SwapImages => Command::SwapImages,
            Self::AboveBelow => Command::AboveBelow,
            Self::LockZoom => Command::LockZoom,
            Self::MarkedOnly => return None,
            Self::HideColumns => Command::ToggleColumns,
            Self::Inspector => Command::ToggleInfo,
            Self::FullScreen => Command::Fullscreen,
        })
    }
}

/// What a fixed key does (U8-Q1, U8-Q2): the keys no action of the map can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedKey {
    /// An arrow, Page Up/Down, Home or End in the contact sheet.
    Navigate(Navigation),
    /// The same with Shift in the contact sheet: extends the marking.
    Extend(Navigation),
    /// Enter and Space in the contact sheet.
    OpenPreview,
    /// Escape in the contact sheet.
    ClearMarking,
    /// Ctrl+Tab in the contact sheet.
    NextTab,
    /// Ctrl+Shift+Tab in the contact sheet.
    PreviousTab,
    /// →, ↓ and Page Down in the preview, with or without Shift.
    NextImage,
    /// ←, ↑ and Page Up in the preview, with or without Shift.
    PreviousImage,
    /// Home in the preview, with or without Shift.
    FirstImage,
    /// End in the preview, with or without Shift.
    LastImage,
    /// Escape and Enter in the preview, with or without Shift (the shell closes a compare first).
    ClosePreview,
    /// `=` with or without Shift, and Shift+`+`, in the preview.
    ZoomIn,
    /// A key that does nothing in this view: Ctrl+Tab and Ctrl+Shift+Tab in the preview (U3-Q1).
    Taken,
}

/// The navigation of an arrow, Page Up/Down, Home or End.
fn navigation(key: Key) -> Option<Navigation> {
    Some(match key {
        Key::ArrowLeft => Navigation::Left,
        Key::ArrowRight => Navigation::Right,
        Key::ArrowUp => Navigation::Up,
        Key::ArrowDown => Navigation::Down,
        Key::PageUp => Navigation::PageUp,
        Key::PageDown => Navigation::PageDown,
        Key::Home => Navigation::Home,
        Key::End => Navigation::End,
        _ => return None,
    })
}

/// The fixed key of a key press in a view, asked for what is not in the map (T280). `Both` gives
/// the contact sheet's fixed key, else the preview's.
pub fn fixed_key(chord: KeyChord, view: KeyView) -> Option<FixedKey> {
    match view {
        KeyView::Sheet => fixed_in_sheet(chord),
        KeyView::Preview => fixed_in_preview(chord),
        KeyView::Both => fixed_in_sheet(chord).or_else(|| fixed_in_preview(chord)),
    }
}

fn fixed_in_sheet(chord: KeyChord) -> Option<FixedKey> {
    let KeyChord {
        key,
        ctrl,
        shift,
        alt,
    } = chord;
    if ctrl || alt {
        return (ctrl && !alt && key == Key::Tab).then_some(if shift {
            FixedKey::PreviousTab
        } else {
            FixedKey::NextTab
        });
    }
    if let Some(navigation) = navigation(key) {
        return Some(if shift {
            FixedKey::Extend(navigation)
        } else {
            FixedKey::Navigate(navigation)
        });
    }
    if shift {
        return None;
    }
    match key {
        Key::Enter | Key::Space => Some(FixedKey::OpenPreview),
        Key::Escape => Some(FixedKey::ClearMarking),
        _ => None,
    }
}

fn fixed_in_preview(chord: KeyChord) -> Option<FixedKey> {
    let KeyChord {
        key,
        ctrl,
        shift,
        alt,
    } = chord;
    if alt {
        return None;
    }
    if ctrl {
        return (key == Key::Tab).then_some(FixedKey::Taken);
    }
    match key {
        Key::ArrowRight | Key::ArrowDown | Key::PageDown => Some(FixedKey::NextImage),
        Key::ArrowLeft | Key::ArrowUp | Key::PageUp => Some(FixedKey::PreviousImage),
        Key::Home => Some(FixedKey::FirstImage),
        Key::End => Some(FixedKey::LastImage),
        Key::Escape | Key::Enter => Some(FixedKey::ClosePreview),
        Key::Equals => Some(FixedKey::ZoomIn),
        Key::Plus if shift => Some(FixedKey::ZoomIn),
        _ => None,
    }
}

/// Whether a key can never be given to an action: fixed in either view, Tab or Shift+Tab (egui
/// moves the keyboard focus with them), or Alt+F4.
pub fn is_fixed(chord: KeyChord) -> bool {
    let KeyChord {
        key,
        ctrl,
        shift,
        alt,
    } = chord;
    fixed_key(chord, KeyView::Both).is_some()
        || (key == Key::Tab && !ctrl && !alt)
        || (key == Key::F4 && alt && !ctrl && !shift)
}

/// Whether the key is a modifier itself (Shift, Ctrl, Alt or the Windows key, left or right),
/// which egui reports as a key of its own; it is never an action's key (U8-Q5, "not a modifier
/// alone").
pub fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
}

/// Why a key cannot be given to an action (U8-Q4, U8-Q5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRefusal {
    /// With Ctrl+Alt (with or without Shift), which is AltGr on many keyboards.
    CtrlAlt,
    /// A modifier key itself ([`is_modifier_key`]), with or without other modifiers.
    Modifier,
    /// A fixed key ([`is_fixed`]).
    Fixed,
    /// Another action (or the action's other key) has it in a view both act in.
    Taken(KeyAction),
}

/// The keys of every action, up to two each (U8-Q2). The keys of an action are kept packed: its
/// second key is set only while it has a first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyMap {
    /// The keys per action, in the order of [`KeyAction::ALL`].
    keys: Vec<[Option<KeyChord>; 2]>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            keys: KeyAction::ALL
                .into_iter()
                .map(KeyAction::default_keys)
                .collect(),
        }
    }
}

impl KeyMap {
    /// The action's keys, the first `Some` first.
    pub fn keys(&self, action: KeyAction) -> [Option<KeyChord>; 2] {
        self.keys
            .get(action.index())
            .copied()
            .unwrap_or([None, None])
    }

    /// The action's first key, the one the menus show (U8-Q2).
    pub fn first_key(&self, action: KeyAction) -> Option<KeyChord> {
        self.keys(action).into_iter().flatten().next()
    }

    /// The action whose key is exactly this chord (all four modifiers compared, U8-Q12) and that
    /// acts in `view`.
    pub fn action_of(&self, chord: KeyChord, view: KeyView) -> Option<KeyAction> {
        KeyAction::ALL.into_iter().find(|action| {
            action.acts_in().overlaps(view) && self.keys(*action).contains(&Some(chord))
        })
    }

    /// Whether a key may be given to an action at all (U8-Q5): a key alone or with Ctrl, Shift,
    /// Alt, Ctrl+Shift or Shift+Alt, not a modifier key itself and not a fixed key.
    pub fn allowed(chord: KeyChord) -> Result<(), KeyRefusal> {
        if chord.ctrl && chord.alt {
            Err(KeyRefusal::CtrlAlt)
        } else if is_modifier_key(chord.key) {
            Err(KeyRefusal::Modifier)
        } else if is_fixed(chord) {
            Err(KeyRefusal::Fixed)
        } else {
            Ok(())
        }
    }

    /// Whether the chord can become the action's key in `slot`: [`KeyMap::allowed`], and no other
    /// action (nor the action's other slot) has it in a view both act in (U8-Q3, U8-Q4).
    pub fn can_set(
        &self,
        action: KeyAction,
        slot: usize,
        chord: KeyChord,
    ) -> Result<(), KeyRefusal> {
        Self::allowed(chord)?;
        for other in KeyAction::ALL {
            if !other.acts_in().overlaps(action.acts_in()) {
                continue;
            }
            for (other_slot, key) in self.keys(other).into_iter().enumerate() {
                if key == Some(chord) && !(other == action && other_slot == slot) {
                    return Err(KeyRefusal::Taken(other));
                }
            }
        }
        Ok(())
    }

    /// Gives the action the chord in `slot` (0 or 1) after [`KeyMap::can_set`]; nothing changes
    /// when it is refused. Slot 1 while slot 0 is empty fills slot 0; another slot does nothing.
    pub fn set(
        &mut self,
        action: KeyAction,
        slot: usize,
        chord: KeyChord,
    ) -> Result<(), KeyRefusal> {
        self.can_set(action, slot, chord)?;
        if let Some(keys) = self.keys.get_mut(action.index()) {
            match slot {
                0 => keys[0] = Some(chord),
                1 if keys[0].is_none() => keys[0] = Some(chord),
                1 => keys[1] = Some(chord),
                _ => {}
            }
        }
        Ok(())
    }

    /// Takes the key in `slot` (0 or 1) from the action; slot 1's key moves up when slot 0 is
    /// cleared. Another slot does nothing.
    pub fn clear(&mut self, action: KeyAction, slot: usize) {
        if let Some(keys) = self.keys.get_mut(action.index()) {
            match slot {
                0 => *keys = [keys[1], None],
                1 => keys[1] = None,
                _ => {}
            }
        }
    }

    /// Every action back to its default keys.
    pub fn restore_defaults(&mut self) {
        *self = Self::default();
    }

    /// Both keys of the action taken away (while reading `keys.txt`).
    fn clear_all(&mut self, action: KeyAction) {
        if let Some(keys) = self.keys.get_mut(action.index()) {
            *keys = [None, None];
        }
    }

    /// The text of `keys.txt`: the comment line, then a line per action whose keys differ from
    /// its defaults.
    fn file_text(&self) -> String {
        let mut text = format!("{FILE_HEADER}\n");
        for action in KeyAction::ALL {
            let keys = self.keys(action);
            if keys == action.default_keys() {
                continue;
            }
            let written: Vec<String> = keys.into_iter().flatten().map(chord_text).collect();
            if written.is_empty() {
                text.push_str(&format!("{} =\n", action.name()));
            } else {
                text.push_str(&format!("{} = {}\n", action.name(), written.join(", ")));
            }
        }
        text
    }
}

/// A key as `keys.txt` writes it: its modifiers in the order Ctrl, Shift, Alt and egui's key name,
/// joined by `+`.
fn chord_text(chord: KeyChord) -> String {
    let mut parts = Vec::new();
    if chord.ctrl {
        parts.push("Ctrl");
    }
    if chord.shift {
        parts.push("Shift");
    }
    if chord.alt {
        parts.push("Alt");
    }
    parts.push(chord.key.name());
    parts.join("+")
}

/// A key as `keys.txt` reads it: modifiers in any order and case, each at most once, then a key
/// name as egui's `Key::from_name` reads it; spaces around `+` ignored.
fn parse_chord(text: &str) -> Option<KeyChord> {
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let (key_name, modifiers) = parts.split_last()?;
    let mut chord = KeyChord {
        key: Key::from_name(key_name)?,
        ctrl: false,
        shift: false,
        alt: false,
    };
    for modifier in modifiers {
        let flag = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" => &mut chord.ctrl,
            "shift" => &mut chord.shift,
            "alt" => &mut chord.alt,
            _ => return None,
        };
        if *flag {
            return None;
        }
        *flag = true;
    }
    Some(chord)
}

/// Why a key is refused, for a problem text of the log.
fn refusal_text(chord: KeyChord, refusal: KeyRefusal) -> String {
    let key = chord_text(chord);
    match refusal {
        KeyRefusal::CtrlAlt => format!("key {key} uses Ctrl+Alt"),
        KeyRefusal::Modifier => format!("key {key} is a modifier alone"),
        KeyRefusal::Fixed => format!("key {key} is a fixed key"),
        KeyRefusal::Taken(other) => format!("key {key} is taken by {}", other.name()),
    }
}

/// `%LOCALAPPDATA%\RawStride\keys.txt`; `None` without `LOCALAPPDATA`.
pub fn keys_path() -> Option<PathBuf> {
    settings_path().map(|path| path.with_file_name(KEYS_FILE))
}

/// A line of `keys.txt` accepted by the first pass.
struct KeyLine {
    number: usize,
    action: KeyAction,
    keys: Vec<KeyChord>,
}

/// Reads `keys.txt` (U8-Q6). A missing file gives the default map and no problem; a file that
/// cannot be read gives the default map and one problem. The lines are read in two passes, so a
/// map this program saved always reads back. First every line is parsed: a line with no `=`, an
/// unknown action, a key that cannot be read or is not allowed, more than two keys, or a second
/// line for the same action is skipped with a problem naming its line number. Then every action
/// not named keeps its default keys, the named ones start with none and get their lines' keys in
/// file order; a line with a key that is taken is skipped with a problem, and its action gets
/// back those of its default keys that are free. The problems are English, for the log.
pub fn load_key_map(path: &Path) -> (KeyMap, Vec<String>) {
    let mut map = KeyMap::default();
    let mut problems = Vec::new();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (map, problems),
        Err(error) => {
            problems.push(format!(
                "keys file {} could not be read, the default keys used: {error}",
                path.display()
            ));
            return (map, problems);
        }
    };
    // A byte order mark written by an editor is not part of the first line.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    let mut lines: Vec<KeyLine> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let problem = |what: &str| {
            format!(
                "keys file {}, line {number}: {what} in \"{trimmed}\", line ignored",
                path.display()
            )
        };
        let Some((name, keys_text)) = trimmed.split_once('=') else {
            problems.push(problem("no '='"));
            continue;
        };
        let Some(action) = KeyAction::from_name(name.trim()) else {
            problems.push(problem("unknown action"));
            continue;
        };
        let keys_text = keys_text.trim();
        let mut keys = Vec::new();
        let mut refused = None;
        if !keys_text.is_empty() {
            for part in keys_text.split(',') {
                match parse_chord(part) {
                    Some(chord) => match KeyMap::allowed(chord) {
                        Ok(()) => keys.push(chord),
                        Err(refusal) => {
                            refused = Some(refusal_text(chord, refusal));
                            break;
                        }
                    },
                    None => {
                        refused = Some(format!("key \"{}\" cannot be read", part.trim()));
                        break;
                    }
                }
            }
        }
        if let Some(what) = refused {
            problems.push(problem(&what));
            continue;
        }
        if keys.len() > 2 {
            problems.push(problem("more than two keys"));
            continue;
        }
        if lines.iter().any(|kept| kept.action == action) {
            problems.push(problem(&format!("a second line for {}", action.name())));
            continue;
        }
        lines.push(KeyLine {
            number,
            action,
            keys,
        });
    }

    for line in &lines {
        map.clear_all(line.action);
    }
    for line in &lines {
        let action = line.action;
        let mut clash = None;
        for (slot, chord) in line.keys.iter().enumerate() {
            if let Err(refusal) = map.set(action, slot, *chord) {
                clash = Some(refusal_text(*chord, refusal));
                break;
            }
        }
        let Some(what) = clash else {
            continue;
        };
        map.clear_all(action);
        for chord in action.default_keys().into_iter().flatten() {
            // Slot 1 fills slot 0 while it is empty; a default key that is taken is left out.
            let _ = map.set(action, 1, chord);
        }
        let result = if map.first_key(action).is_some() {
            format!(
                "{} keeps those of its default keys that are free",
                action.name()
            )
        } else {
            format!("{} has no key", action.name())
        };
        problems.push(format!(
            "keys file {}, line {}: {what}, line ignored; {result}",
            path.display(),
            line.number
        ));
    }
    (map, problems)
}

/// Writes `keys.txt`: creates the folder, writes the map to `<file name>.tmp` in the same folder,
/// then renames it onto the file (which replaces it on Windows).
pub fn save_key_map(path: &Path, map: &KeyMap) -> io::Result<()> {
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("keys path {} has no file name", path.display()),
        ));
    };
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut temp_name = file_name.to_os_string();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let written =
        write_synced(&temp, map.file_text().as_bytes()).and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        // Best effort: the error of the write or the rename is the one reported.
        let _ = std::fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{GridKeyAction, LoupeKeyAction, grid_key_action, loupe_key_action};
    use crate::orientations::rotation_key_action;
    use crate::ratings::{RatingKey, rating_key_action};
    // `menu_items()` holds every item of `MENUS`, its rating and label submenus included.
    use crate::shell::{IMAGE_CONTEXT_MENU, MenuEntry, menu_items};
    use crate::{
        Action as ViewAction, MoveKey, PreviewKey, ShellAction, TabAction, delete_key_action,
        key_action, move_key_action, open_in_key_action, opens_import, preview_key_action,
        reload_key_action, shell_key_action, tab_action,
    };
    use std::collections::HashSet;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "pw-loupe-keymap-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> PathBuf {
            self.0.join(KEYS_FILE)
        }

        fn with_file(text: &str) -> (Self, PathBuf) {
            let dir = Self::new();
            let path = dir.path();
            std::fs::write(&path, text).unwrap();
            (dir, path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn chord(key: Key, ctrl: bool, shift: bool, alt: bool) -> KeyChord {
        KeyChord {
            key,
            ctrl,
            shift,
            alt,
        }
    }

    fn key(key: Key) -> KeyChord {
        chord(key, false, false, false)
    }

    /// Every key with each of the eight modifier combinations.
    fn every_chord() -> Vec<KeyChord> {
        let mut chords = Vec::new();
        for &key in Key::ALL {
            for bits in 0..8u8 {
                chords.push(chord(key, bits & 1 != 0, bits & 2 != 0, bits & 4 != 0));
            }
        }
        chords
    }

    /// What a key press does, as the map and today's key functions both give it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Outcome {
        Action(KeyAction),
        Fixed(FixedKey),
        Nothing,
    }

    /// The outcome with the map: the map's action, else the fixed key (`Taken` is nothing).
    fn with_map(map: &KeyMap, chord: KeyChord, view: KeyView) -> Outcome {
        if let Some(action) = map.action_of(chord, view) {
            return Outcome::Action(action);
        }
        match fixed_key(chord, view) {
            None | Some(FixedKey::Taken) => Outcome::Nothing,
            Some(fixed) => Outcome::Fixed(fixed),
        }
    }

    /// The outcome of today's key functions, asked in the order of `main.rs`'s key loop.
    fn today(chord: KeyChord, view: KeyView) -> Outcome {
        use KeyAction as A;
        use Outcome::{Action, Fixed, Nothing};
        let KeyChord {
            key,
            ctrl,
            shift,
            alt,
        } = chord;
        if view == KeyView::Preview
            && let Some(preview_key) = preview_key_action(key, ctrl, shift, alt)
        {
            return match preview_key {
                PreviewKey::OnlyMarked => Action(A::MarkedOnly),
                PreviewKey::HideParts => Action(A::HideColumns),
                PreviewKey::ToggleColumn => Action(A::Inspector),
                PreviewKey::ClipHighlights => Action(A::ClippedHighlights),
                PreviewKey::ClipShadows => Action(A::DeepShadows),
                PreviewKey::FocusMarking => Action(A::FocusMarking),
                PreviewKey::Compare => Action(A::Compare),
                PreviewKey::HoldImage => Action(A::HoldImage),
                PreviewKey::SwapImages => Action(A::SwapImages),
                PreviewKey::AboveBelow => Action(A::AboveBelow),
                PreviewKey::LockZoom => Action(A::LockZoom),
                // The map's actions that the shell takes in the preview and does nothing with.
                PreviewKey::Nothing => match (key, ctrl) {
                    (Key::O, true) => Action(A::OpenFolder),
                    (Key::I, true) => Action(A::Import),
                    (Key::W, true) => Action(A::CloseTab),
                    (Key::F11, false) => Action(A::FullScreen),
                    _ => Nothing,
                },
            };
        }
        if opens_import(key, ctrl, shift, alt) {
            return Action(A::Import);
        }
        if let Some(tab) = tab_action(key, ctrl, shift, alt) {
            return match tab {
                TabAction::Next => Fixed(FixedKey::NextTab),
                TabAction::Previous => Fixed(FixedKey::PreviousTab),
                TabAction::Close => Action(A::CloseTab),
            };
        }
        if let Some(shell) = shell_key_action(key, ctrl, shift, alt) {
            return Action(match shell {
                ShellAction::OpenFolder => A::OpenFolder,
                ShellAction::ToggleFullscreen => A::FullScreen,
                ShellAction::ToggleColumns => A::HideColumns,
            });
        }
        if let Some(move_key) = move_key_action(key, ctrl, shift, alt) {
            return Action(match move_key {
                MoveKey::MovePicked => A::MovePicked,
                MoveKey::UndoMove => A::UndoMove,
            });
        }
        if delete_key_action(key, ctrl, shift, alt) {
            return Action(A::Delete);
        }
        if reload_key_action(key, ctrl, shift, alt) {
            return Action(A::ReloadFolder);
        }
        if open_in_key_action(key, ctrl, shift, alt) {
            return Action(A::OpenIn);
        }
        if let Some(rating) = rating_key_action(key, ctrl, shift, alt) {
            return Action(match rating {
                RatingKey::Stars(0) => A::Stars0,
                RatingKey::Stars(1) => A::Stars1,
                RatingKey::Stars(2) => A::Stars2,
                RatingKey::Stars(3) => A::Stars3,
                RatingKey::Stars(4) => A::Stars4,
                RatingKey::Stars(5) => A::Stars5,
                RatingKey::Label("Red") => A::LabelRed,
                RatingKey::Label("Yellow") => A::LabelYellow,
                RatingKey::Label("Green") => A::LabelGreen,
                RatingKey::Label("Blue") => A::LabelBlue,
                RatingKey::Label("Purple") => A::LabelPurple,
                RatingKey::ClearLabel => A::ClearLabel,
                RatingKey::Reject => A::Reject,
                other => panic!("no action for {other:?}"),
            });
        }
        if let Some(clockwise) = rotation_key_action(key, ctrl, shift, alt) {
            return Action(if clockwise {
                A::RotateRight
            } else {
                A::RotateLeft
            });
        }
        match key_action(key, ctrl, alt) {
            Some(ViewAction::OpenMetadata) => return Action(A::Metadata),
            Some(ViewAction::ToggleInfo) => return Action(A::Inspector),
            _ => {}
        }
        match view {
            KeyView::Sheet => match grid_key_action(key, ctrl, shift, alt) {
                Some(GridKeyAction::OpenLoupe) if key == Key::G => Action(A::Preview),
                Some(GridKeyAction::OpenLoupe) => Fixed(FixedKey::OpenPreview),
                Some(GridKeyAction::Navigate(navigation)) => Fixed(FixedKey::Navigate(navigation)),
                Some(GridKeyAction::Extend(navigation)) => Fixed(FixedKey::Extend(navigation)),
                Some(GridKeyAction::SelectAll) => Action(A::SelectAll),
                Some(GridKeyAction::ClearSelection) => Fixed(FixedKey::ClearMarking),
                None => Nothing,
            },
            KeyView::Preview => {
                if loupe_key_action(key, ctrl, alt) == Some(LoupeKeyAction::ShowGrid) {
                    return if key == Key::G {
                        Action(A::Preview)
                    } else {
                        Fixed(FixedKey::ClosePreview)
                    };
                }
                match key_action(key, ctrl, alt) {
                    Some(ViewAction::Next) => Fixed(FixedKey::NextImage),
                    Some(ViewAction::Previous) => Fixed(FixedKey::PreviousImage),
                    Some(ViewAction::First) => Fixed(FixedKey::FirstImage),
                    Some(ViewAction::Last) => Fixed(FixedKey::LastImage),
                    Some(ViewAction::ToggleZoom) => Action(A::ZoomToggle),
                    Some(ViewAction::Fit) => Action(A::Fit),
                    Some(ViewAction::ZoomStep(1)) if key == Key::Equals || shift => {
                        Fixed(FixedKey::ZoomIn)
                    }
                    Some(ViewAction::ZoomStep(1)) => Action(A::ZoomIn),
                    Some(ViewAction::ZoomStep(-1)) => Action(A::ZoomOut),
                    Some(ViewAction::ToggleAnchor) => Action(A::ZoomAtPointer),
                    Some(ViewAction::ToggleInfo) => Action(A::Inspector),
                    Some(ViewAction::ToggleMode) => Action(A::RawJpeg),
                    Some(ViewAction::OpenMetadata) => Action(A::Metadata),
                    Some(other) => panic!("no outcome for {other:?}"),
                    None => Nothing,
                }
            }
            KeyView::Both => panic!("a lookup is made in the sheet or the preview"),
        }
    }

    fn is_letter_digit_or_minus(key: Key) -> bool {
        let name = key.name();
        key == Key::Minus
            || (name.len() == 1
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
    }

    /// The only differences allowed between today's keys and the default map (U8-Q12's exact
    /// keys): a Shift on a letter, a digit or `−` that today's `key_action` ignores in the
    /// preview, and Shift+M and Shift+I in both views.
    fn allowed_difference(chord: KeyChord, view: KeyView, today: Outcome, map: Outcome) -> bool {
        let shift_only = chord.shift && !chord.ctrl && !chord.alt;
        if !shift_only || map != Outcome::Nothing || !matches!(today, Outcome::Action(_)) {
            return false;
        }
        (view == KeyView::Preview && is_letter_digit_or_minus(chord.key))
            || matches!(chord.key, Key::M | Key::I)
    }

    #[test]
    fn the_default_map_is_todays_keys() {
        let map = KeyMap::default();
        let mut differences = Vec::new();
        let mut allowed = 0;
        for view in [KeyView::Sheet, KeyView::Preview] {
            for chord in every_chord() {
                let today = today(chord, view);
                let new = with_map(&map, chord, view);
                if today == new {
                    continue;
                }
                if allowed_difference(chord, view, today, new) {
                    allowed += 1;
                } else {
                    differences.push(format!("{view:?} {chord:?}: today {today:?}, map {new:?}"));
                }
            }
        }
        assert!(differences.is_empty(), "{differences:#?}");
        // Shift+Z, Shift+F, … in the preview and Shift+M, Shift+I in both views.
        assert!(allowed > 0);
        assert_eq!(
            with_map(&map, chord(Key::Z, false, true, false), KeyView::Preview),
            Outcome::Nothing
        );
    }

    #[test]
    fn every_menu_item_with_an_action_shows_its_default_first_key() {
        let map = KeyMap::default();
        let mut items = menu_items();
        for entry in IMAGE_CONTEXT_MENU {
            match entry {
                MenuEntry::Item(item) => items.push(*item),
                MenuEntry::Submenu(_, sub) => items.extend_from_slice(sub),
                _ => {}
            }
        }
        let mut checked = 0;
        for item in items {
            if let Some(action) = KeyAction::of_command(item.command) {
                assert_eq!(item.key, map.first_key(action), "{:?}", item.command);
                checked += 1;
            }
        }
        assert!(checked > 40, "{checked}");
    }

    #[test]
    fn commands_and_actions_correspond() {
        for action in KeyAction::ALL {
            match action.command() {
                Some(command) => assert_eq!(KeyAction::of_command(command), Some(action)),
                None => assert_eq!(action, KeyAction::MarkedOnly),
            }
        }
        assert_eq!(
            KeyAction::of_command(Command::OpenInAnother),
            Some(KeyAction::OpenIn)
        );
        assert_eq!(KeyAction::of_command(Command::OpenIn(1)), None);
        for fixed in [
            Command::ClearSelection,
            Command::NextTab,
            Command::PreviousTab,
            Command::Quit,
        ] {
            assert_eq!(KeyAction::of_command(fixed), None);
        }
    }

    #[test]
    fn names_round_trip_and_are_unique() {
        let mut names = HashSet::new();
        for (index, action) in KeyAction::ALL.into_iter().enumerate() {
            assert_eq!(action.index(), index);
            assert_eq!(KeyAction::from_name(action.name()), Some(action));
            assert!(names.insert(action.name()), "{}", action.name());
        }
        assert_eq!(KeyAction::from_name("Reject"), None);
        assert_eq!(KeyAction::from_name("nothing"), None);
    }

    #[test]
    fn the_default_map_passes_can_set_for_every_key_it_holds() {
        let map = KeyMap::default();
        for action in KeyAction::ALL {
            let keys = map.keys(action);
            assert!(keys[0].is_some(), "{action:?}");
            for (slot, key) in keys.into_iter().enumerate() {
                if let Some(key) = key {
                    assert_eq!(map.can_set(action, slot, key), Ok(()), "{action:?} {key:?}");
                }
            }
        }
    }

    #[test]
    fn allowed_refuses_ctrl_alt_and_fixed_keys() {
        assert_eq!(
            KeyMap::allowed(chord(Key::K, true, false, true)),
            Err(KeyRefusal::CtrlAlt)
        );
        assert_eq!(
            KeyMap::allowed(chord(Key::K, true, true, true)),
            Err(KeyRefusal::CtrlAlt)
        );
        assert_eq!(
            KeyMap::allowed(chord(Key::F4, false, false, true)),
            Err(KeyRefusal::Fixed)
        );
        assert_eq!(KeyMap::allowed(key(Key::Tab)), Err(KeyRefusal::Fixed));
        assert_eq!(
            KeyMap::allowed(chord(Key::Tab, false, true, false)),
            Err(KeyRefusal::Fixed)
        );
        let mut fixed = 0;
        for chord in every_chord() {
            let in_a_view = fixed_key(chord, KeyView::Sheet).is_some()
                || fixed_key(chord, KeyView::Preview).is_some();
            if in_a_view {
                assert!(is_fixed(chord), "{chord:?}");
                assert!(KeyMap::allowed(chord).is_err(), "{chord:?}");
                fixed += 1;
            }
        }
        // The sheet: 8 navigation keys with and without Shift, Enter, Space, Escape, Ctrl+Tab,
        // Ctrl+Shift+Tab (21); the preview adds Shift+Escape, Shift+Enter, `=` with and without
        // Shift and Shift+`+` (5).
        assert_eq!(fixed, 26);
        for allowed in [
            key(Key::K),
            chord(Key::K, true, false, false),
            chord(Key::K, false, true, false),
            chord(Key::K, false, false, true),
            chord(Key::K, true, true, false),
            chord(Key::K, false, true, true),
            chord(Key::F4, false, false, false),
            chord(Key::ArrowLeft, true, true, false),
        ] {
            assert_eq!(KeyMap::allowed(allowed), Ok(()), "{allowed:?}");
        }
    }

    #[test]
    fn allowed_refuses_a_modifier_key_alone() {
        let modifiers: Vec<Key> = Key::ALL
            .iter()
            .copied()
            .filter(|key| is_modifier_key(*key))
            .collect();
        // Shift, Ctrl, Alt and the Windows key, each left and right.
        assert_eq!(modifiers.len(), 8, "{modifiers:?}");
        for chord in every_chord() {
            if is_modifier_key(chord.key) && !(chord.ctrl && chord.alt) {
                assert_eq!(
                    KeyMap::allowed(chord),
                    Err(KeyRefusal::Modifier),
                    "{chord:?}"
                );
            }
        }
        assert!(!is_modifier_key(Key::K));
        assert_eq!(
            KeyMap::default().set(KeyAction::Fit, 1, key(Key::ShiftLeft)),
            Err(KeyRefusal::Modifier)
        );
    }

    #[test]
    fn can_set_refuses_a_key_taken_in_the_same_view() {
        let map = KeyMap::default();
        assert_eq!(
            map.can_set(KeyAction::MarkedOnly, 0, key(Key::C)),
            Err(KeyRefusal::Taken(KeyAction::Compare))
        );
        assert_eq!(
            map.can_set(KeyAction::Fit, 1, key(Key::X)),
            Err(KeyRefusal::Taken(KeyAction::Reject))
        );
        assert_eq!(
            map.can_set(KeyAction::SelectAll, 1, key(Key::M)),
            Err(KeyRefusal::Taken(KeyAction::Metadata))
        );
        // The action's other slot counts as taken.
        assert_eq!(
            map.can_set(KeyAction::Stars3, 0, chord(Key::Num3, false, false, true)),
            Err(KeyRefusal::Taken(KeyAction::Stars3))
        );
        // A sheet-only action may share A with the preview-only zoom_at_pointer (U8-Q3).
        assert_eq!(map.can_set(KeyAction::SelectAll, 1, key(Key::A)), Ok(()));
        let mut changed = map.clone();
        assert_eq!(changed.set(KeyAction::SelectAll, 1, key(Key::A)), Ok(()));
        assert_eq!(
            changed.action_of(key(Key::A), KeyView::Sheet),
            Some(KeyAction::SelectAll)
        );
        assert_eq!(
            changed.action_of(key(Key::A), KeyView::Preview),
            Some(KeyAction::ZoomAtPointer)
        );
    }

    #[test]
    fn set_changes_nothing_when_refused() {
        let mut map = KeyMap::default();
        let before = map.clone();
        assert_eq!(
            map.set(KeyAction::Fit, 0, key(Key::X)),
            Err(KeyRefusal::Taken(KeyAction::Reject))
        );
        assert_eq!(
            map.set(KeyAction::Fit, 0, chord(Key::K, true, false, true)),
            Err(KeyRefusal::CtrlAlt)
        );
        assert_eq!(
            map.set(KeyAction::Fit, 0, key(Key::Escape)),
            Err(KeyRefusal::Fixed)
        );
        assert_eq!(map, before);
    }

    #[test]
    fn a_slot_other_than_0_or_1_does_nothing() {
        let mut map = KeyMap::default();
        let before = map.clone();
        let _ = map.set(KeyAction::Fit, 2, key(Key::Y));
        map.clear(KeyAction::Fit, 2);
        map.clear(KeyAction::Stars0, 7);
        assert_eq!(map, before);
    }

    #[test]
    fn keys_stay_packed() {
        let mut map = KeyMap::default();
        map.clear(KeyAction::Fit, 0);
        assert_eq!(map.keys(KeyAction::Fit), [None, None]);
        assert_eq!(map.set(KeyAction::Fit, 1, key(Key::Y)), Ok(()));
        assert_eq!(map.keys(KeyAction::Fit), [Some(key(Key::Y)), None]);
        map.clear(KeyAction::LabelRed, 0);
        assert_eq!(
            map.keys(KeyAction::LabelRed),
            [Some(chord(Key::Num1, true, false, false)), None]
        );
        assert_eq!(
            map.first_key(KeyAction::LabelRed),
            Some(chord(Key::Num1, true, false, false))
        );
        map.restore_defaults();
        assert_eq!(map, KeyMap::default());
    }

    #[test]
    fn every_key_text_reads_back() {
        for chord in every_chord() {
            assert_eq!(parse_chord(&chord_text(chord)), Some(chord), "{chord:?}");
        }
        assert_eq!(chord_text(chord(Key::K, true, true, false)), "Ctrl+Shift+K");
        assert_eq!(chord_text(chord(Key::Num3, false, false, true)), "Alt+3");
        assert_eq!(
            chord_text(chord(Key::ArrowLeft, true, false, false)),
            "Ctrl+Left"
        );
        assert_eq!(parse_chord("Ctrl+Ctrl+K"), None);
        assert_eq!(parse_chord("Super+K"), None);
        assert_eq!(parse_chord("Ctrl+"), None);
        assert_eq!(parse_chord(""), None);
    }

    #[test]
    fn a_default_map_writes_only_the_comment_line_and_reads_back() {
        let dir = TempDir::new();
        let path = dir.path();
        save_key_map(&path, &KeyMap::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# RawStride keys\n"
        );
        assert_eq!(load_key_map(&path), (KeyMap::default(), Vec::new()));
        assert!(!path.with_file_name("keys.txt.tmp").exists());
    }

    #[test]
    fn a_changed_map_reads_back_equal() {
        let mut map = KeyMap::default();
        // A key moved from a later action to an earlier one.
        map.clear(KeyAction::FullScreen, 0);
        map.set(KeyAction::OpenFolder, 1, key(Key::F11)).unwrap();
        // Two keys swapped between a later and an earlier action.
        map.clear(KeyAction::Metadata, 0);
        map.clear(KeyAction::Reject, 0);
        map.set(KeyAction::Metadata, 0, key(Key::X)).unwrap();
        map.set(KeyAction::Reject, 0, key(Key::M)).unwrap();
        // A second key added.
        map.set(KeyAction::Fit, 1, chord(Key::F, false, true, false))
            .unwrap();
        // A key cleared.
        map.clear(KeyAction::Stars0, 1);
        // Slot 0 cleared with slot 1 set.
        map.clear(KeyAction::LabelRed, 0);

        let dir = TempDir::new();
        let path = dir.path();
        save_key_map(&path, &map).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "# RawStride keys\n\
             open_folder = Ctrl+O, F11\n\
             metadata = X\n\
             stars_0 = 0\n\
             label_red = Ctrl+1\n\
             reject = M\n\
             fit = F, Shift+F\n\
             full_screen =\n"
        );
        assert_eq!(load_key_map(&path), (map, Vec::new()));
    }

    /// Reads a file with the text and returns the map and the problems.
    fn read(text: &str) -> (KeyMap, Vec<String>) {
        let (_dir, path) = TempDir::with_file(text);
        load_key_map(&path)
    }

    fn assert_one_problem(problems: &[String], line: usize) {
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains(&format!("line {line}:")),
            "{problems:?}"
        );
    }

    #[test]
    fn each_problem_line_gives_one_problem_and_leaves_the_default() {
        for line in [
            "reject X",
            "nothing = K",
            "reject = Ctrl+Nokey",
            "reject = Hyper+X",
            "reject = Y, U, T",
            "reject = Ctrl+Alt+Y",
            "reject = Escape",
            "reject = Alt+F4",
            "reject = ShiftLeft",
            "reject = Y,",
        ] {
            let (map, problems) = read(&format!("# RawStride keys\n\n{line}\n"));
            assert_one_problem(&problems, 3);
            assert_eq!(map, KeyMap::default(), "{line}");
        }
    }

    #[test]
    fn a_second_line_for_the_same_action_is_skipped() {
        let (map, problems) = read("# RawStride keys\nreject = Y\nreject = U\n");
        assert_one_problem(&problems, 3);
        assert_eq!(map.keys(KeyAction::Reject), [Some(key(Key::Y)), None]);
    }

    #[test]
    fn a_clashing_line_keeps_the_default_keys_that_are_free() {
        // M is metadata's (not named), so reject keeps its default X.
        let (map, problems) = read("# RawStride keys\nreject = M\n");
        assert_one_problem(&problems, 2);
        assert!(problems[0].contains("taken by metadata"), "{problems:?}");
        assert_eq!(map, KeyMap::default());
        // fit takes X first; reject's line clashes on X, and its default X is taken: no key.
        let (map, problems) = read("# RawStride keys\nfit = X\nreject = F, X\n");
        assert_one_problem(&problems, 3);
        assert!(problems[0].contains("reject has no key"), "{problems:?}");
        assert_eq!(map.keys(KeyAction::Fit), [Some(key(Key::X)), None]);
        assert_eq!(map.keys(KeyAction::Reject), [None, None]);
    }

    #[test]
    fn modifiers_are_read_in_any_order_and_case_with_spaces() {
        let (map, problems) =
            read("  rotate_left  =  shift + CTRL + Left ,  alt+ k \nfit=F\n# a comment\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            map.keys(KeyAction::RotateLeft),
            [
                Some(chord(Key::ArrowLeft, true, true, false)),
                Some(chord(Key::K, false, false, true))
            ]
        );
        assert_eq!(map.keys(KeyAction::Fit), KeyAction::Fit.default_keys());
    }

    #[test]
    fn an_action_with_no_key_reads_back() {
        let (map, problems) = read("reject =\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(map.keys(KeyAction::Reject), [None, None]);
        assert_eq!(map.action_of(key(Key::X), KeyView::Sheet), None);
    }

    #[test]
    fn a_missing_file_gives_the_default_map_and_no_problem() {
        let dir = TempDir::new();
        assert_eq!(load_key_map(&dir.path()), (KeyMap::default(), Vec::new()));
    }

    #[test]
    fn an_unreadable_file_gives_the_default_map_and_one_problem() {
        let dir = TempDir::new();
        // A folder where the file should be cannot be read as text.
        std::fs::create_dir_all(dir.path()).unwrap();
        let (map, problems) = load_key_map(&dir.path());
        assert_eq!(map, KeyMap::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn a_byte_order_mark_is_skipped() {
        let (map, problems) = read("\u{feff}reject = Y\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(map.keys(KeyAction::Reject), [Some(key(Key::Y)), None]);
    }

    #[test]
    fn keys_path_is_beside_the_settings_file() {
        // Read only: the environment is not changed (other tests run at the same time).
        assert_eq!(
            keys_path(),
            settings_path().map(|path| path.with_file_name("keys.txt"))
        );
    }
}
