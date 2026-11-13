//! RawStride (ADR 0012; the egui program of ADR 0009–0011): the parts of the program that are plain
//! Rust and tested without a window (GR11): command-line options, the folder listing, the key map
//! and the texts of the status line. `main.rs` wires them to eframe, `pw-viewcore` and
//! `pw-render`.

mod browse;
mod editor;
mod filter;
mod grid;
mod import;
mod import_dialog;
mod import_text;
mod import_worker;
mod info_panel;
mod monitor_profile;
mod preview;
mod preview_text;
mod ratings;
mod selection;
mod settings;
mod shell;
mod shell_text;
mod tabs;
mod thumbs;

use std::path::{Path, PathBuf};

use pw_viewcore::{DecodeError, Frame, Source, Status, Zoom};

pub use browse::LoupeMove;
pub use editor::{EditorField, EditorForm, EditorText, OpenConflict, editor_text, notice_text};
pub use filter::{
    Filter, LABEL_CHOICES, LabelChoice, LabelFilter, STAR_CONDITIONS, STAR_FILTERS, StarCondition,
    StarFilter, label_choice_text, label_filter_text, matches, nearest_visible,
    star_condition_text, star_filter_text, visible,
};
pub use grid::{
    DEFAULT_THUMBNAIL_SIZE, GridKeyAction, LoupeKeyAction, MAX_THUMBNAIL_SIZE, MIN_THUMBNAIL_SIZE,
    Navigation, THUMBNAIL_SIZE_STEP, clamp_thumbnail_size, columns, grid_key_action,
    loupe_key_action, navigate, row, step_thumbnail_size, take_wheel_steps, visible_range,
};
pub use import::{
    Field, FieldError, ImportForm, IptcField, IptcForm, IptcModes, ItemAction, Preview,
    PreviewLine, Validated, allowed_modes, clear_and_import_text, clear_cancel_text,
    clear_confirmation_text, clear_confirmation_title, mode_text, result_folder,
};
pub use import_dialog::{DialogRequest, ImportDialog, ImportDialogState};
pub use import_text::{
    DoneLine, done_lines, ejecting_progress_text, iptc_progress_text, more_lines, preview_line,
    preview_summary, progress_text,
};
pub use import_worker::{ImportDone, ImportEvent, ImportWorker, RequestId, SubmitError};
pub use info_panel::{
    ConflictRow, ConflictValue, InfoPanel, Language, PanelText, ValueRow, info_panel, origin_text,
    other_values, panel_text, property_label, rule_text, value_text,
};
pub use monitor_profile::{
    MonitorProfile, ProfileSource, load_monitor_profile, primary_monitor_profile_path, window_title,
};
pub use preview::{
    FILMSTRIP_CELL_WIDTH, FILMSTRIP_IMAGE_HEIGHT, FILMSTRIP_LABEL_HEIGHT, FILMSTRIP_MARGIN,
    FILMSTRIP_SPACING, PreviewParts, Restore, ThumbGate, WindowBefore, clamp_offset,
    filmstrip_range, filmstrip_width, hit_test, preview_info_shown, preview_parts, scroll_target,
    thumbnail_gate, toggle_preview_column, toggle_preview_hidden,
};
pub use preview_text::{PreviewText, preview_text, zoom_value_text};
pub use ratings::{
    RatingKey, RatingRefused, RatingStore, RatingWriter, UNSUPPORTED_TEXT, apply, label_name,
    rating_key_action, star_glyphs, status_text,
};
pub use selection::Selection;
pub use settings::{
    Settings, ShellSettings, WindowRect, load_settings, save_settings, settings_path,
};
pub use shell::{
    COLLAPSED_WIDTH, ColumnState, Command, HIDE_KEY, KeyChord, MENUS, Menu, MenuEntry, MenuItem,
    MenuKind, NON_TEXT_CONTRAST, Palette, STATUS_TEXT_SIZE, StartWindow, SubmenuKind,
    TEXT_CONTRAST, THEMES, Theme, Where, apply_text_sizes, clamp_column_width, column_width_range,
    contrast_ratio, image_area, menu_items, non_text_pairs, palette, rect_on_a_screen,
    start_window, text_pairs, visuals,
};
pub use shell_text::{
    ShellText, command_text, key_text, menu_text, shell_text, submenu_text, theme_text,
};
pub use tabs::{Tab, Tabs, ViewMode};
pub use thumbs::{CellState, ThumbGrid, color_image, request_cell};

/// The window title: the product name (ADR 0012). `latency.ps1` finds the window by it (ADR 0009
/// §11, decision 4, amended by ADR 0012).
pub const WINDOW_TITLE: &str = "RawStride";

/// Command-line options (ADR 0009 §11, decisions 2 and 3: no settings file, no dialog).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// A folder, or a file in a folder (that file is shown first).
    pub path: Option<PathBuf>,
    /// Core budget (LibRaw sessions and decoded JPEGs), in MiB.
    pub core_mb: u64,
    /// Texture budget, in MiB.
    pub texture_mb: u64,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            path: None,
            core_mb: 4096,
            texture_mb: 1024,
        }
    }
}

pub const USAGE: &str = "pw-loupe [--core-mb <MiB>] [--texture-mb <MiB>] [<folder or file>]\n\
     \n\
     --core-mb     memory for RAW sessions and decoded JPEGs and TIFFs (default 4096)\n\
     --texture-mb  memory for finished views as textures (default 1024)";

/// Parses the arguments after the program name.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut o = Options::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut number = |name: &str| -> Result<u64, String> {
            let v = it.next().ok_or_else(|| format!("{name} needs a value"))?;
            match v.parse::<u64>() {
                Ok(n) if (16..=1_048_576).contains(&n) => Ok(n),
                _ => Err(format!(
                    "{name}: not a size in MiB between 16 and 1048576: {v}"
                )),
            }
        };
        match a.as_str() {
            "--core-mb" => o.core_mb = number("--core-mb")?,
            "--texture-mb" => o.texture_mb = number("--texture-mb")?,
            "-h" | "--help" => return Err(USAGE.to_string()),
            s if s.starts_with("--") => return Err(format!("unknown option {s}\n\n{USAGE}")),
            _ if o.path.is_some() => return Err(format!("only one folder or file\n\n{USAGE}")),
            _ => o.path = Some(PathBuf::from(a)),
        }
    }
    Ok(o)
}

/// Whether the loupe lists `path`: the file types of the current app (TIFF since ADR 0015).
pub fn is_listed(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    pw_raw::SUPPORTED_EXTENSIONS.contains(&ext.as_str())
}

/// The images of a folder in display order (by name, ignoring case), and the position of
/// `selected` among them. A file path opens its folder with that file selected. A path that
/// does not exist is an error naming that path, not its folder.
pub fn list_folder(path: &Path) -> Result<(Vec<PathBuf>, usize), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (dir, selected) = if meta.is_dir() {
        (path.to_path_buf(), None)
    } else {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        (dir, Some(path.to_path_buf()))
    };
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_listed(p))
        .collect();
    files.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    let pos = selected
        .and_then(|s| files.iter().position(|f| f.file_name() == s.file_name()))
        .unwrap_or(0);
    Ok((files, pos))
}

/// What a key does (ADR 0009 §3.3; the current viewer's keys, plus R).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Next,
    Previous,
    First,
    Last,
    /// Z: fit ↔ 100 %, around the anchor.
    ToggleZoom,
    Fit,
    /// + and −: one step in (1) or out (−1), around the anchor.
    ZoomStep(i32),
    /// A: zoom around the pointer or the image centre.
    ToggleAnchor,
    /// I: open or collapse the inspector (the right column), in the loupe and the contact sheet;
    /// from the hidden side columns (`H`) it brings both back with the inspector open (ADR 0017
    /// §2.4, U2-Q3).
    ToggleInfo,
    /// R: RAW ↔ JPEG (G4).
    ToggleMode,
    /// M: open the metadata window of the current image, in the loupe and the contact sheet
    /// (ADR 0007 I10n, D16).
    OpenMetadata,
}

/// Whether a loupe action is allowed for this key event (ADR 0009 §13).
pub fn step_allowed(action: Action, repeat: bool, status: &Status) -> bool {
    !(repeat
        && matches!(action, Action::Next | Action::Previous)
        && matches!(status, Status::Preparing { .. }))
}

/// The action of a key press in the preview (the loupe's keys, ADR 0017 §2.2, T111). `M` (the
/// metadata window) and `I` (the inspector open ↔ collapsed, ADR 0017 §2.4, U2-Q3) act in the
/// contact sheet as well; in the preview `I` is taken first by [`preview_key_action`] for the
/// preview's own right column. Keys handled elsewhere (Escape, G, Enter, Ctrl+O, Ctrl+A, Ctrl+I,
/// digits, and the preview's `S` and `H`) give `None`.
pub fn key_action(key: egui::Key, ctrl: bool, alt: bool) -> Option<Action> {
    use egui::Key as K;
    if ctrl || alt {
        return None;
    }
    Some(match key {
        K::ArrowRight | K::ArrowDown | K::PageDown => Action::Next,
        K::ArrowLeft | K::ArrowUp | K::PageUp => Action::Previous,
        K::Home => Action::First,
        K::End => Action::Last,
        K::Z => Action::ToggleZoom,
        K::F => Action::Fit,
        K::Plus | K::Equals => Action::ZoomStep(1),
        K::Minus => Action::ZoomStep(-1),
        K::A => Action::ToggleAnchor,
        K::I => Action::ToggleInfo,
        K::R => Action::ToggleMode,
        K::M => Action::OpenMetadata,
        _ => return None,
    })
}

/// What a tab shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    Next,
    Previous,
    Close,
}

/// Maps tab shortcuts separately from the loupe key map.
pub fn tab_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<TabAction> {
    use egui::Key as K;
    if !ctrl || alt {
        return None;
    }
    match (key, shift) {
        (K::Tab, false) => Some(TabAction::Next),
        (K::Tab, true) => Some(TabAction::Previous),
        (K::W, false) => Some(TabAction::Close),
        _ => None,
    }
}

/// The zoom factor of a mouse-wheel movement of `dy` points (as the current viewer: about ×1.16
/// per notch of 100).
pub fn wheel_factor(dy: f32) -> f64 {
    let f = (f64::from(dy) * 0.0015).exp();
    if f.is_finite() && f > 0.0 { f } else { 1.0 }
}

fn file_name(frame: &Frame) -> String {
    frame
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether the frame shows a TIFF file. The core keys a TIFF `Source::Jpeg` (ADR 0015 §4.2), so the
/// texts look at the path.
fn is_tiff(frame: &Frame) -> bool {
    frame.path.as_deref().is_some_and(pw_raw::is_tiff_file)
}

/// What is shown (G1, honest source): RAW rendering, embedded JPEG, a JPEG file or a TIFF file,
/// with size.
pub fn source_text(frame: &Frame) -> String {
    let size = frame
        .size
        .map(|s| {
            let (w, h) = s.oriented();
            format!(" {w} × {h}")
        })
        .unwrap_or_default();
    let plain = frame
        .path
        .as_deref()
        .is_some_and(pw_viewcore::is_plain_jpeg);
    match frame.source {
        Source::Raw => format!("RAW-rendering (LibRaw, PPG){size}"),
        Source::Jpeg if is_tiff(frame) => format!("TIFF-fil{size}"),
        Source::Jpeg if plain => format!("JPEG-fil{size}"),
        Source::Jpeg => format!("Inbäddad JPEG{size} (kamerans rendering, inte RAW)"),
    }
}

/// The zoom part of the status line: "Zoom 37 %", and "(anpassad)" at fit.
pub fn zoom_text(frame: &Frame) -> String {
    match (frame.zoom, frame.scale) {
        (Zoom::Fit, Some(s)) => format!("Zoom {:.0} % (anpassad)", s * 100.0),
        (Zoom::Fit, None) => "Zoom anpassad".to_string(),
        (Zoom::Scale(s), _) => format!("Zoom {:.0} %", s * 100.0),
    }
}

/// The status-line part after the source text when the image's embedded colour profile could not
/// be used (ADR 0016 §5).
pub const PROFILE_UNUSABLE_TEXT: &str = "Inbäddad färgprofil kunde inte användas";

/// The status line of the loupe. It starts with `position_text`, as in the grid (ADR 0014 §5.3,
/// amendment 1): the core holds the visible list, so `frame.count` is the number shown, and `total`
/// is the number of images in the folder ("3 / 12 (av 31)", or "3 / 31" without a filter).
pub fn status_line(frame: &Frame, anchor_at_pointer: bool, total: usize) -> String {
    let Some(pos) = frame.pos else {
        return "Ingen mapp öppen. Dra en mapp eller en bild hit, eller ange den på kommandoraden."
            .to_string();
    };
    let anchor = if anchor_at_pointer {
        "zoom mot pekaren"
    } else {
        "zoom mot mitten"
    };
    let what = match &frame.status {
        Status::Error(_) => "kan inte visas".to_string(),
        Status::Preparing { .. } if frame.size.is_none() => match frame.source {
            Source::Raw => "förbereder RAW".to_string(),
            Source::Jpeg if is_tiff(frame) => "förbereder TIFF".to_string(),
            Source::Jpeg => "förbereder JPEG".to_string(),
        },
        // ADR 0016 §5: the colours rest on a fallback because the file's own profile was broken
        _ if frame.profile_unusable => {
            format!("{}  ·  {PROFILE_UNUSABLE_TEXT}", source_text(frame))
        }
        _ => source_text(frame),
    };
    format!(
        "{}  ·  {}  ·  {}  ·  {}  ·  {}",
        position_text(pos, frame.count, total),
        file_name(frame),
        what,
        zoom_text(frame),
        anchor
    )
}

/// The position part of the status line (ADR 0014 §5.3): "3 / 12 (av 31)" under a filter, or
/// "3 / 31" when every image is shown. `pos` is the zero-based position in the visible list.
pub fn position_text(pos: usize, shown: usize, total: usize) -> String {
    let number = pos.saturating_add(1);
    if shown == total {
        format!("{number} / {total}")
    } else {
        format!("{number} / {shown} (av {total})")
    }
}

/// The text when the filter lets nothing through (ADR 0014 §2.4, §5.3), in the status line and the
/// empty grid.
pub fn no_match_text(total: usize) -> String {
    format!("Inga bilder matchar filtret ({total} i mappen)")
}

/// The marking part of the status line (ADR 0014 §5.3): only when more than one image is marked.
pub fn marked_text(n: usize) -> Option<String> {
    (n > 1).then(|| format!("{n} markerade"))
}

/// Shown in the grid and the status line while an active filter waits for the ratings (Q7).
pub const READING_RATINGS_TEXT: &str = "Läser betyg …";

/// The neutral loading indicator (V2): no image content, only what is being prepared.
pub fn indicator_text(frame: &Frame) -> String {
    match frame.source {
        Source::Raw => "Förbereder RAW …".to_string(),
        Source::Jpeg if is_tiff(frame) => "Förbereder TIFF …".to_string(),
        Source::Jpeg => "Förbereder JPEG …".to_string(),
    }
}

/// The message for a file that cannot be shown (G3, G4).
pub fn error_text(e: &DecodeError, mode: Source) -> String {
    if e.damaged {
        "RAW-data saknas eller är skadad. Byt till JPEG-läget (R) för att se kamerans förhandsbild."
            .to_string()
    } else if mode == Source::Jpeg && e.message.contains("no embedded JPEG") {
        "Filen har ingen inbäddad JPEG. Byt till RAW-läget (R).".to_string()
    } else {
        format!("Filen kan inte visas: {}", e.message)
    }
}

/// The label of the display-mode control (G4: "with a key and a control").
pub fn mode_label(mode: Source) -> &'static str {
    match mode {
        Source::Raw => "Läge: RAW (R)",
        Source::Jpeg => "Läge: JPEG (R)",
    }
}

/// A key of the main window's shell (ADR 0017 §2.4, U1, T109).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellAction {
    /// Ctrl+O: the folder dialog ("Öppna mapp").
    OpenFolder,
    /// F11: full screen for the main window.
    ToggleFullscreen,
    /// The hide key ([`HIDE_KEY`], U1-Q2): hides both side columns, or brings them back.
    ToggleColumns,
}

/// The shell action of a key press: Ctrl+O, F11 and the hide key, each with exactly those
/// modifiers. Tab is not used (Ctrl+Tab changes tab, and egui moves the focus with Tab).
pub fn shell_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<ShellAction> {
    match (key, ctrl, shift, alt) {
        (egui::Key::O, true, false, false) => Some(ShellAction::OpenFolder),
        (egui::Key::F11, false, false, false) => Some(ShellAction::ToggleFullscreen),
        (key, false, false, false) if key == HIDE_KEY => Some(ShellAction::ToggleColumns),
        _ => None,
    }
}

/// A key of the preview's own (ADR 0017 §2.2, §2.4, U3-Q1–Q3, T111), asked before every other key
/// map while the preview is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKey {
    /// `S`: "only the marked" on ↔ off in the filmstrip and the steps.
    OnlyMarked,
    /// `H`: hides the right column and the filmstrip together, or brings them back.
    HideParts,
    /// `I`: the preview's right column open ↔ collapsed.
    ToggleColumn,
    /// Ctrl+O, Ctrl+I, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab and F11: nothing until the preview is
    /// closed (U3-Q1). The key is taken so no other key map acts on it.
    Nothing,
}

/// The preview's key of a key press, each with exactly those modifiers; `None` for the keys the
/// preview shares with the loupe and the rest of the program (steps, zoom, R, M, ratings, Escape,
/// G, Enter).
pub fn preview_key_action(
    key: egui::Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> Option<PreviewKey> {
    use egui::Key as K;
    match (key, ctrl, shift, alt) {
        (K::S, false, false, false) => Some(PreviewKey::OnlyMarked),
        (key, false, false, false) if key == HIDE_KEY => Some(PreviewKey::HideParts),
        (K::I, false, false, false) => Some(PreviewKey::ToggleColumn),
        (K::O | K::I | K::W, true, false, false)
        | (K::Tab, true, _, false)
        | (K::F11, false, false, false) => Some(PreviewKey::Nothing),
        _ => None,
    }
}

/// Whether the key combination opens the import dialog (Ctrl+I, no shift or alt).
pub fn opens_import(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> bool {
    key == egui::Key::I && ctrl && !shift && !alt
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKey {
    Close,
    Start,
}

/// The action of a key press while the import dialog is open.
pub fn dialog_key_action(key: egui::Key, ctrl: bool, alt: bool) -> Option<DialogKey> {
    if ctrl || alt {
        return None;
    }
    match key {
        egui::Key::Escape => Some(DialogKey::Close),
        egui::Key::Enter => Some(DialogKey::Start),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_viewcore::ImageSize;
    use std::time::Instant;

    fn args(v: &[&str]) -> Result<Options, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    fn frame(status: Status, source: Source) -> Frame {
        Frame {
            draw: None,
            status,
            pos: Some(2),
            count: 31,
            path: Some(PathBuf::from("D:/RAW/TN100423.ARW")),
            mode: source,
            source,
            size: Some(ImageSize {
                w: 9568,
                h: 6376,
                flip: 5,
            }),
            zoom: Zoom::Fit,
            scale: Some(0.1129),
            profile_unusable: false,
        }
    }

    #[test]
    fn step_actions_wait_only_for_preparing_frames_on_repeats() {
        let statuses = [
            Status::Ready,
            Status::Preparing {
                since: Instant::now(),
                indicator: false,
            },
            Status::Error(DecodeError::new("damaged")),
            Status::Empty,
        ];
        for action in [Action::Next, Action::Previous] {
            for status in &statuses {
                assert!(step_allowed(action, false, status));
                assert_eq!(
                    step_allowed(action, true, status),
                    !matches!(status, Status::Preparing { .. })
                );
            }
        }
    }

    #[test]
    fn repeated_non_step_actions_are_always_allowed() {
        for action in [
            Action::First,
            Action::Last,
            Action::ToggleZoom,
            Action::ZoomStep(1),
        ] {
            let preparing = Status::Preparing {
                since: Instant::now(),
                indicator: false,
            };
            assert!(step_allowed(action, true, &preparing));
        }
    }

    #[test]
    fn options_have_defaults_and_validate_sizes() {
        assert_eq!(args(&[]), Ok(Options::default()));
        let o = args(&["--core-mb", "3072", "D:/RAW", "--texture-mb", "512"]);
        assert_eq!(
            o,
            Ok(Options {
                path: Some(PathBuf::from("D:/RAW")),
                core_mb: 3072,
                texture_mb: 512
            })
        );
        assert!(args(&["--core-mb"]).is_err());
        assert!(args(&["--core-mb", "lots"]).is_err());
        assert!(args(&["--texture-mb", "0"]).is_err());
        assert!(args(&["--nonsense"]).is_err());
        assert!(args(&["a", "b"]).is_err());
        assert_eq!(args(&["--help"]), Err(USAGE.to_string()));
    }

    #[test]
    fn the_folder_lists_supported_files_by_name() {
        let dir = std::env::temp_dir().join(format!("pw-loupe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join("sub.arw"));
        for n in [
            "b.ARW",
            "a.jpg",
            "C.dng",
            "d.tif",
            "e.xmp",
            "f.rw2",
            "notes.txt",
        ] {
            let _ = std::fs::write(dir.join(n), b"x");
        }
        let (files, pos) = list_folder(&dir).unwrap_or_default();
        let names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.ARW", "C.dng", "d.tif", "f.rw2"]);
        assert_eq!(pos, 0);
        // a file opens its folder with that file selected
        let (_, pos) = list_folder(&dir.join("C.dng")).unwrap_or_default();
        assert_eq!(pos, 2);
        // a missing path is an error naming the path itself, not its folder
        let missing = dir.join("missing-folder").join("x.arw");
        let error = list_folder(&missing).err().unwrap_or_default();
        assert!(error.starts_with(&missing.display().to_string()), "{error}");
        let missing = dir.join("missing.arw");
        let error = list_folder(&missing).err().unwrap_or_default();
        assert!(error.starts_with(&missing.display().to_string()), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keys_follow_the_current_viewer_and_reserved_keys_do_nothing() {
        use egui::Key as K;
        assert_eq!(key_action(K::ArrowRight, false, false), Some(Action::Next));
        assert_eq!(key_action(K::PageUp, false, false), Some(Action::Previous));
        assert_eq!(key_action(K::End, false, false), Some(Action::Last));
        assert_eq!(key_action(K::Z, false, false), Some(Action::ToggleZoom));
        assert_eq!(key_action(K::Plus, false, false), Some(Action::ZoomStep(1)));
        assert_eq!(
            key_action(K::Minus, false, false),
            Some(Action::ZoomStep(-1))
        );
        assert_eq!(key_action(K::R, false, false), Some(Action::ToggleMode));
        for k in [K::Escape, K::G, K::Enter, K::Num1, K::Space] {
            assert_eq!(key_action(k, false, false), None, "{k:?}");
        }
        // Ctrl+O and Ctrl+A are reserved; A alone toggles the anchor
        assert_eq!(key_action(K::O, true, false), None);
        assert_eq!(key_action(K::A, true, false), None);
        assert_eq!(key_action(K::A, false, false), Some(Action::ToggleAnchor));
    }

    #[test]
    fn m_opens_the_metadata_window_without_ctrl_or_alt() {
        use egui::Key as K;
        assert_eq!(key_action(K::M, false, false), Some(Action::OpenMetadata));
        assert_eq!(key_action(K::M, true, false), None);
        assert_eq!(key_action(K::M, false, true), None);
        // M is not a key of the contact sheet or of the loupe's way back to it.
        assert_eq!(grid_key_action(K::M, false, false, false), None);
        assert_eq!(loupe_key_action(K::M, false, false), None);
    }

    #[test]
    fn i_toggles_the_inspector_without_ctrl_or_alt_and_ctrl_i_stays_import() {
        use egui::Key as K;
        assert_eq!(key_action(K::I, false, false), Some(Action::ToggleInfo));
        assert_eq!(key_action(K::I, true, false), None);
        assert_eq!(key_action(K::I, false, true), None);
        assert!(opens_import(K::I, true, false, false));
        assert!(!opens_import(K::I, false, false, false));
        // I is not a key of the contact sheet or of the loupe's way back to it.
        assert_eq!(grid_key_action(K::I, false, false, false), None);
        assert_eq!(loupe_key_action(K::I, false, false), None);
        // From the contact sheet and the loupe alike, I gives Visa ▸ Inspektör.
        let i = KeyChord {
            key: K::I,
            ctrl: false,
            shift: false,
            alt: false,
        };
        assert_eq!(command_of_key(i, ViewMode::Grid), Some(Command::ToggleInfo));
        assert_eq!(
            command_of_key(i, ViewMode::Loupe),
            Some(Command::ToggleInfo)
        );
        let ctrl_i = KeyChord { ctrl: true, ..i };
        assert_eq!(
            command_of_key(ctrl_i, ViewMode::Grid),
            Some(Command::Import)
        );
        assert_eq!(
            command_of_key(ctrl_i, ViewMode::Loupe),
            Some(Command::Import)
        );
    }

    #[test]
    fn e_does_nothing() {
        use egui::Key as K;
        // E opened the editor before D16; the metadata window is opened with M only.
        assert_eq!(key_action(K::E, false, false), None);
        assert_eq!(key_action(K::E, true, false), None);
        assert_eq!(key_action(K::E, false, true), None);
        // E is not a key of the contact sheet or of the loupe's way back to it.
        assert_eq!(grid_key_action(K::E, false, false, false), None);
        assert_eq!(loupe_key_action(K::E, false, false), None);
    }

    #[test]
    fn tab_keys_are_separate_from_viewer_keys() {
        use egui::Key as K;
        assert_eq!(
            tab_action(K::Tab, true, false, false),
            Some(TabAction::Next)
        );
        assert_eq!(
            tab_action(K::Tab, true, true, false),
            Some(TabAction::Previous)
        );
        assert_eq!(tab_action(K::W, true, false, false), Some(TabAction::Close));
        assert_eq!(tab_action(K::Tab, false, false, false), None);
        assert_eq!(tab_action(K::W, true, true, false), None);
        assert_eq!(tab_action(K::W, true, false, true), None);
        assert_eq!(key_action(K::Tab, true, false), None);
        assert_eq!(key_action(K::W, true, false), None);
    }

    #[test]
    fn the_wheel_zooms_like_the_current_viewer() {
        assert!((wheel_factor(100.0) - (0.15f64).exp()).abs() < 1e-9);
        assert!(wheel_factor(-100.0) < 1.0);
        assert_eq!(wheel_factor(f32::NAN), 1.0);
    }

    #[test]
    fn the_status_line_says_what_is_shown() {
        let f = frame(Status::Ready, Source::Raw);
        let s = status_line(&f, true, 31);
        assert!(s.starts_with("3 / 31  ·  "), "{s}");
        assert!(s.contains("TN100423.ARW"));
        assert!(s.contains("RAW-rendering (LibRaw, PPG) 6376 × 9568"), "{s}");
        assert!(s.contains("Zoom 11 % (anpassad)"));
        assert!(s.contains("zoom mot pekaren"));
        let j = frame(Status::Ready, Source::Jpeg);
        assert!(
            status_line(&j, false, 31).contains("Inbäddad JPEG 6376 × 9568 (kamerans rendering")
        );
        let mut plain = frame(Status::Ready, Source::Jpeg);
        plain.path = Some(PathBuf::from("photo.JPG"));
        assert!(source_text(&plain).starts_with("JPEG-fil"));
        let mut z = frame(Status::Ready, Source::Raw);
        z.zoom = Zoom::Scale(2.0);
        assert_eq!(zoom_text(&z), "Zoom 200 %");
        let mut empty = frame(Status::Empty, Source::Raw);
        empty.pos = None;
        assert!(status_line(&empty, true, 31).starts_with("Ingen mapp"));
    }

    #[test]
    fn the_loupe_status_line_starts_with_the_position_in_the_filter() {
        // Without a filter the core shows the whole folder: "3 / 31".
        let all = frame(Status::Ready, Source::Raw);
        assert_eq!(
            status_line(&all, true, 31),
            "3 / 31  ·  TN100423.ARW  ·  RAW-rendering (LibRaw, PPG) 6376 × 9568  ·  \
             Zoom 11 % (anpassad)  ·  zoom mot pekaren"
        );
        // Under a filter the core shows the 12 visible images of 31 (ADR 0014 §5.3).
        let mut filtered = frame(Status::Ready, Source::Raw);
        filtered.count = 12;
        let s = status_line(&filtered, false, 31);
        assert!(s.starts_with("3 / 12 (av 31)  ·  TN100423.ARW  ·  "), "{s}");
        assert!(s.ends_with("zoom mot mitten"), "{s}");
    }

    /// The `app.js` case "loupe within filter" (`ui/tests/filter-select.test.js`) on
    /// `Tab::loupe_move`: with the `browse.rs` setup (1: 3 stars, 2: 5 stars Red, 3: rejected,
    /// 4: Red only), filter Red, Home, Enter, step right → IMG_002, then IMG_004.
    #[test]
    fn the_loupe_steps_within_the_filter() {
        use pw_metadata::Rating;
        use std::collections::HashMap;

        let name = |index: usize| PathBuf::from(format!("IMG_{index:03}.ARW"));
        let rating = |stars: Option<i8>, label: Option<&str>| Rating {
            stars,
            label: label.map(str::to_owned),
        };
        let ratings = HashMap::from([
            (name(1), rating(Some(3), None)),
            (name(2), rating(Some(5), Some("Red"))),
            (name(3), rating(Some(-1), None)),
            (name(4), rating(None, Some("Red"))),
        ]);
        let rating_of = |path: &Path| ratings.get(path).cloned();
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("D:/x"), (0..12).map(name).collect(), 7);
        tabs.set_filter(Filter {
            stars: StarFilter::All,
            condition: StarCondition::AtLeast,
            label: LabelFilter::only(LabelChoice::Label("Red")),
        });
        let Some(tab) = tabs.active_mut() else {
            panic!("there is always an active tab");
        };
        tab.refilter(true, &rating_of, None);
        assert_eq!(tab.visible, [2, 4]);
        // Home in the grid, then Enter opens the loupe on the current image.
        tab.grid_move(Navigation::Home, false, 4, 3, true, &rating_of);
        tab.mode = ViewMode::Loupe;
        let shown = |tab: &Tab| tab.files.get(tab.current).cloned();
        assert_eq!(shown(tab), Some(name(2)));
        assert_eq!(tab.position(), Some(0));
        // A step right skips IMG_003 (hidden) and shows IMG_004.
        tab.loupe_move(LoupeMove::Next, true, &rating_of);
        assert_eq!(shown(tab), Some(name(4)));
        assert_eq!(tab.position(), Some(1));
        assert_eq!(
            tab.visible,
            [2, 4],
            "a step does not change the visible list"
        );
        // Stepping stays within the filter at both ends.
        tab.loupe_move(LoupeMove::Next, true, &rating_of);
        assert_eq!(shown(tab), Some(name(4)));
        tab.loupe_move(LoupeMove::Previous, true, &rating_of);
        assert_eq!(shown(tab), Some(name(2)));
        tab.loupe_move(LoupeMove::Last, true, &rating_of);
        assert_eq!(shown(tab), Some(name(4)));
        tab.loupe_move(LoupeMove::First, true, &rating_of);
        assert_eq!(shown(tab), Some(name(2)));
    }

    #[test]
    fn preparing_and_errors_are_described_without_image_content() {
        let mut f = frame(
            Status::Preparing {
                since: Instant::now(),
                indicator: true,
            },
            Source::Raw,
        );
        f.size = None;
        assert!(status_line(&f, true, 31).contains("förbereder RAW"));
        assert_eq!(indicator_text(&f), "Förbereder RAW …");
        let damaged = DecodeError::damaged("damaged: cut");
        assert!(error_text(&damaged, Source::Raw).contains("JPEG-läget"));
        let none = DecodeError::new("x: the file has no embedded JPEG");
        assert!(error_text(&none, Source::Jpeg).contains("ingen inbäddad JPEG"));
        assert!(error_text(&DecodeError::new("boom"), Source::Raw).contains("boom"));
        assert_eq!(mode_label(Source::Jpeg), "Läge: JPEG (R)");
    }

    #[test]
    fn tiff_files_are_listed() {
        for n in ["a.tif", "b.TIFF", "c.tiff"] {
            assert!(is_listed(Path::new(n)), "{n}");
        }
    }

    fn tiff_frame(status: Status, mode: Source) -> Frame {
        Frame {
            draw: None,
            status,
            pos: Some(0),
            count: 9,
            path: Some(PathBuf::from("D:/Scans/Scan 01.tif")),
            mode,
            source: Source::Jpeg,
            size: Some(ImageSize {
                w: 7741,
                h: 5161,
                flip: 6,
            }),
            zoom: Zoom::Fit,
            scale: Some(0.2),
            profile_unusable: false,
        }
    }

    #[test]
    fn a_tiff_frame_is_named_tiff_file_with_its_oriented_size_in_both_modes() {
        for mode in [Source::Raw, Source::Jpeg] {
            let f = tiff_frame(Status::Ready, mode);
            assert_eq!(source_text(&f), "TIFF-fil 5161 × 7741", "{mode:?}");
            let s = status_line(&f, true, 9);
            assert!(
                s.contains("Scan 01.tif  ·  TIFF-fil 5161 × 7741  ·  "),
                "{s}"
            );
        }
    }

    #[test]
    fn a_preparing_tiff_frame_says_tiff() {
        let mut f = tiff_frame(
            Status::Preparing {
                since: Instant::now(),
                indicator: true,
            },
            Source::Raw,
        );
        f.size = None;
        let s = status_line(&f, true, 9);
        assert!(s.contains("förbereder TIFF"), "{s}");
        assert!(!s.contains("JPEG"), "{s}");
        assert_eq!(indicator_text(&f), "Förbereder TIFF …");
    }

    #[test]
    fn a_jpeg_file_keeps_its_texts() {
        let mut f = frame(
            Status::Preparing {
                since: Instant::now(),
                indicator: true,
            },
            Source::Jpeg,
        );
        f.path = Some(PathBuf::from("D:/Photos/photo.JPG"));
        assert_eq!(indicator_text(&f), "Förbereder JPEG …");
        assert_eq!(source_text(&f), "JPEG-fil 6376 × 9568");
        f.size = None;
        assert!(status_line(&f, true, 31).contains("förbereder JPEG"));
    }

    #[test]
    fn the_filter_texts_of_the_status_line() {
        assert_eq!(position_text(2, 12, 31), "3 / 12 (av 31)");
        assert_eq!(position_text(2, 31, 31), "3 / 31");
        assert_eq!(position_text(0, 2, 12), "1 / 2 (av 12)");
        assert_eq!(
            no_match_text(31),
            "Inga bilder matchar filtret (31 i mappen)"
        );
        assert_eq!(marked_text(0), None);
        assert_eq!(marked_text(1), None);
        assert_eq!(marked_text(2).as_deref(), Some("2 markerade"));
        assert_eq!(marked_text(12).as_deref(), Some("12 markerade"));
        assert_eq!(READING_RATINGS_TEXT, "Läser betyg …");
    }

    // ---------- an embedded profile that cannot be used (ADR 0016 §5, T73) ----------

    fn small_tiff_frame(status: Status) -> Frame {
        let mut f = tiff_frame(status, Source::Jpeg);
        f.size = Some(ImageSize {
            w: 6,
            h: 4,
            flip: 0,
        });
        f
    }

    #[test]
    fn the_status_line_says_when_the_embedded_profile_cannot_be_used() {
        let mut f = small_tiff_frame(Status::Ready);
        let without = status_line(&f, true, 9);
        f.profile_unusable = true;
        let with = status_line(&f, true, 9);
        assert!(
            with.contains("TIFF-fil 6 × 4  ·  Inbäddad färgprofil kunde inte användas  ·  "),
            "{with}"
        );
        // nothing else in the line changes
        assert_eq!(
            with.replace("  ·  Inbäddad färgprofil kunde inte användas", ""),
            without
        );

        let mut j = frame(Status::Ready, Source::Jpeg);
        j.path = Some(PathBuf::from("D:/RAW/TN005701_DxO.jpg"));
        j.profile_unusable = true;
        let s = status_line(&j, false, 31);
        assert!(
            s.contains("JPEG-fil 6376 × 9568  ·  Inbäddad färgprofil kunde inte användas  ·  "),
            "{s}"
        );
    }

    #[test]
    fn without_the_flag_the_status_line_is_unchanged() {
        let f = small_tiff_frame(Status::Ready);
        assert!(!f.profile_unusable);
        assert_eq!(
            status_line(&f, true, 9),
            "1 / 9  ·  Scan 01.tif  ·  TIFF-fil 6 × 4  ·  Zoom 20 % (anpassad)  ·  zoom mot pekaren"
        );
    }

    #[test]
    fn preparing_and_error_frames_do_not_show_the_profile_part() {
        let mut preparing = small_tiff_frame(Status::Preparing {
            since: Instant::now(),
            indicator: true,
        });
        preparing.size = None;
        preparing.profile_unusable = true;
        let s = status_line(&preparing, true, 9);
        assert!(s.contains("förbereder TIFF"), "{s}");
        assert!(!s.contains(PROFILE_UNUSABLE_TEXT), "{s}");

        let mut error = small_tiff_frame(Status::Error(DecodeError::new("boom")));
        error.profile_unusable = true;
        let s = status_line(&error, true, 9);
        assert!(s.contains("kan inte visas"), "{s}");
        assert!(!s.contains(PROFILE_UNUSABLE_TEXT), "{s}");
    }

    #[test]
    fn opens_import_only_on_ctrl_i() {
        use egui::Key as K;
        assert!(opens_import(K::I, true, false, false));
        assert!(!opens_import(K::I, false, false, false));
        assert!(!opens_import(K::I, true, true, false));
        assert!(!opens_import(K::I, true, false, true));
        assert!(!opens_import(K::I, false, false, true));
        assert!(!opens_import(K::O, true, false, false));
    }

    #[test]
    fn dialog_keys_match_spec() {
        use egui::Key as K;
        assert_eq!(
            dialog_key_action(K::Escape, false, false),
            Some(DialogKey::Close)
        );
        assert_eq!(
            dialog_key_action(K::Enter, false, false),
            Some(DialogKey::Start)
        );
        assert_eq!(dialog_key_action(K::Escape, true, false), None);
        assert_eq!(dialog_key_action(K::Enter, true, false), None);
        assert_eq!(dialog_key_action(K::Escape, false, true), None);
        assert_eq!(dialog_key_action(K::Enter, false, true), None);
        assert_eq!(dialog_key_action(K::A, false, false), None);
    }

    #[test]
    fn shell_keys_are_ctrl_o_f11_and_h() {
        use egui::Key as K;
        assert_eq!(
            shell_key_action(K::O, true, false, false),
            Some(ShellAction::OpenFolder)
        );
        assert_eq!(
            shell_key_action(K::F11, false, false, false),
            Some(ShellAction::ToggleFullscreen)
        );
        assert_eq!(
            shell_key_action(K::H, false, false, false),
            Some(ShellAction::ToggleColumns)
        );
        for (ctrl, shift, alt) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
            (false, true, false),
        ] {
            assert_eq!(shell_key_action(K::O, ctrl, shift, alt), None);
        }
        for (ctrl, shift, alt) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            assert_eq!(shell_key_action(K::F11, ctrl, shift, alt), None);
            assert_eq!(shell_key_action(K::H, ctrl, shift, alt), None);
        }
        // Tab is not the hide key, with or without Ctrl; Ctrl+Tab still changes tab.
        for (ctrl, shift) in [(false, false), (true, false), (false, true), (true, true)] {
            assert_eq!(shell_key_action(K::Tab, ctrl, shift, false), None);
        }
        assert_eq!(
            tab_action(K::Tab, true, false, false),
            Some(TabAction::Next)
        );
        // The shell keys do nothing in the loupe's key map.
        assert_eq!(key_action(K::H, false, false), None);
        assert_eq!(key_action(K::F11, false, false), None);
        assert_eq!(key_action(K::O, true, false), None);
    }

    /// The command a key gives in `view`, through the key functions in the order `main.rs` asks
    /// them (shell keys, import, tabs, ratings, `M`, `I`, then the view's own keys).
    fn command_of_key(chord: KeyChord, view: ViewMode) -> Option<Command> {
        let KeyChord {
            key,
            ctrl,
            shift,
            alt,
        } = chord;
        if let Some(action) = shell_key_action(key, ctrl, shift, alt) {
            return Some(match action {
                ShellAction::OpenFolder => Command::OpenFolder,
                ShellAction::ToggleFullscreen => Command::Fullscreen,
                ShellAction::ToggleColumns => Command::ToggleColumns,
            });
        }
        if opens_import(key, ctrl, shift, alt) {
            return Some(Command::Import);
        }
        if let Some(action) = tab_action(key, ctrl, shift, alt) {
            return Some(match action {
                TabAction::Next => Command::NextTab,
                TabAction::Previous => Command::PreviousTab,
                TabAction::Close => Command::CloseTab,
            });
        }
        if let Some(rating) = rating_key_action(key, ctrl, shift, alt) {
            return Some(match rating {
                RatingKey::Stars(stars) => Command::Stars(stars),
                RatingKey::Label(label) => Command::Label(label),
                RatingKey::ClearLabel => Command::ClearLabel,
                RatingKey::Reject => Command::Reject,
            });
        }
        if key_action(key, ctrl, alt) == Some(Action::OpenMetadata) {
            return Some(Command::Metadata);
        }
        if key_action(key, ctrl, alt) == Some(Action::ToggleInfo) {
            return Some(Command::ToggleInfo);
        }
        match view {
            ViewMode::Grid => match grid_key_action(key, ctrl, shift, alt)? {
                GridKeyAction::OpenLoupe => Some(Command::ToggleView),
                GridKeyAction::SelectAll => Some(Command::SelectAll),
                GridKeyAction::ClearSelection => Some(Command::ClearSelection),
                GridKeyAction::Navigate(_) | GridKeyAction::Extend(_) => None,
            },
            ViewMode::Loupe => {
                if shift {
                    return None;
                }
                if loupe_key_action(key, ctrl, alt).is_some() {
                    return Some(Command::ToggleView);
                }
                match key_action(key, ctrl, alt)? {
                    Action::Fit => Some(Command::Fit),
                    Action::ToggleZoom => Some(Command::ToggleZoom),
                    Action::ZoomStep(1) => Some(Command::ZoomIn),
                    Action::ZoomStep(-1) => Some(Command::ZoomOut),
                    Action::ToggleAnchor => Some(Command::ToggleAnchor),
                    Action::ToggleMode => Some(Command::ToggleMode),
                    _ => None,
                }
            }
        }
    }

    #[test]
    fn every_menu_key_gives_its_items_command() {
        let mut checked = 0;
        for item in shell::menu_items() {
            let Some(chord) = item.key else {
                continue;
            };
            // Alt+F4 is Windows' own key for closing the window, not RawStride's.
            if item.command == Command::Quit {
                continue;
            }
            let views: &[ViewMode] = match item.command.acts_in() {
                Where::Both => &[ViewMode::Grid, ViewMode::Loupe],
                Where::Grid => &[ViewMode::Grid],
                Where::Loupe => &[ViewMode::Loupe],
            };
            for &view in views {
                assert_eq!(
                    command_of_key(chord, view),
                    Some(item.command),
                    "{} shows {} in {view:?}",
                    command_text(item.command),
                    key_text(chord)
                );
                checked += 1;
            }
        }
        assert!(checked > 30, "only {checked} keys checked");
    }

    #[test]
    fn the_preview_keys_are_s_h_and_i_and_the_shell_keys_do_nothing() {
        use egui::Key as K;
        assert_eq!(
            preview_key_action(K::S, false, false, false),
            Some(PreviewKey::OnlyMarked)
        );
        assert_eq!(
            preview_key_action(K::H, false, false, false),
            Some(PreviewKey::HideParts)
        );
        assert_eq!(
            preview_key_action(K::I, false, false, false),
            Some(PreviewKey::ToggleColumn)
        );
        // Ctrl+O, Ctrl+I, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab and F11 give nothing in the preview.
        for (key, ctrl, shift) in [
            (K::O, true, false),
            (K::I, true, false),
            (K::W, true, false),
            (K::Tab, true, false),
            (K::Tab, true, true),
            (K::F11, false, false),
        ] {
            assert_eq!(
                preview_key_action(key, ctrl, shift, false),
                Some(PreviewKey::Nothing),
                "{key:?} ctrl {ctrl} shift {shift}"
            );
        }
        // S, H and I with a modifier are not the preview's (Ctrl+I is taken as nothing above).
        for (ctrl, shift, alt) in [
            (false, true, false),
            (false, false, true),
            (true, true, false),
        ] {
            assert_eq!(preview_key_action(K::S, ctrl, shift, alt), None);
            assert_eq!(preview_key_action(K::H, ctrl, shift, alt), None);
        }
        // The loupe's keys, the ratings, M, R and the way back stay with their own key maps.
        for key in [
            K::ArrowRight,
            K::ArrowLeft,
            K::PageDown,
            K::Home,
            K::End,
            K::Z,
            K::F,
            K::Plus,
            K::Minus,
            K::A,
            K::R,
            K::M,
            K::Num3,
            K::X,
            K::Escape,
            K::G,
            K::Enter,
        ] {
            assert_eq!(
                preview_key_action(key, false, false, false),
                None,
                "{key:?}"
            );
        }
        // S is no other key of the loupe or of the ratings.
        assert_eq!(key_action(K::S, false, false), None);
        assert_eq!(rating_key_action(K::S, false, false, false), None);
    }
}
