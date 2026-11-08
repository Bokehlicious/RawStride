//! PhotoWorkflow Loupe (ADR 0009, milestone 1): the parts of the program that are plain Rust and
//! tested without a window (GR11): command-line options, the folder listing, the key map and the
//! texts of the status line. `main.rs` wires them to eframe, `pw-viewcore` and `pw-render`.

mod grid;
mod tabs;

use std::path::{Path, PathBuf};

use pw_viewcore::{DecodeError, Frame, Source, Status, Zoom};

pub use grid::{
    DEFAULT_THUMBNAIL_SIZE, GridKeyAction, LoupeKeyAction, MAX_THUMBNAIL_SIZE, MIN_THUMBNAIL_SIZE,
    Navigation, THUMBNAIL_SIZE_STEP, clamp_thumbnail_size, columns, grid_key_action,
    loupe_key_action, navigate, row, step_thumbnail_size, visible_range,
};
pub use tabs::{Tab, Tabs, ViewMode};

/// The window title; `latency.ps1` finds the window by it (ADR 0009 §11, decision 4).
pub const WINDOW_TITLE: &str = "PhotoWorkflow Loupe";

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
     --core-mb     memory for RAW sessions and decoded JPEGs (default 4096)\n\
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

/// Whether the loupe lists `path`: the file types of the current app, except TIFF (ADR 0009 §3.1).
pub fn is_listed(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    pw_raw::SUPPORTED_EXTENSIONS.contains(&ext.as_str()) && !pw_raw::is_tiff_file(path)
}

/// The images of a folder in display order (by name, ignoring case), and the position of
/// `selected` among them. A file path opens its folder with that file selected.
pub fn list_folder(path: &Path) -> Result<(Vec<PathBuf>, usize), String> {
    let (dir, selected) = if path.is_dir() {
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
    /// I: show or hide the information.
    ToggleInfo,
    /// R: RAW ↔ JPEG (G4).
    ToggleMode,
}

/// The action of a key press. Keys that are reserved for later milestones (Escape, G, Enter,
/// Ctrl+O, Ctrl+A, digits) give `None`.
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

/// What is shown (G1, honest source): RAW rendering, embedded JPEG or a JPEG file, with size.
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

/// The status line.
pub fn status_line(frame: &Frame, anchor_at_pointer: bool) -> String {
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
            Source::Jpeg => "förbereder JPEG".to_string(),
        },
        _ => source_text(frame),
    };
    format!(
        "{} / {}  ·  {}  ·  {}  ·  {}  ·  {}",
        pos + 1,
        frame.count,
        file_name(frame),
        what,
        zoom_text(frame),
        anchor
    )
}

/// The neutral loading indicator (V2): no image content, only what is being prepared.
pub fn indicator_text(frame: &Frame) -> String {
    match frame.source {
        Source::Raw => "Förbereder RAW …".to_string(),
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
    fn the_folder_lists_supported_files_by_name_without_tiff() {
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
        assert_eq!(names, vec!["a.jpg", "b.ARW", "C.dng", "f.rw2"]);
        assert_eq!(pos, 0);
        // a file opens its folder with that file selected
        let (_, pos) = list_folder(&dir.join("C.dng")).unwrap_or_default();
        assert_eq!(pos, 2);
        assert!(list_folder(&dir.join("missing-folder").join("x.arw")).is_err());
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
        let s = status_line(&f, true);
        assert!(s.starts_with("3 / 31"), "{s}");
        assert!(s.contains("TN100423.ARW"));
        assert!(s.contains("RAW-rendering (LibRaw, PPG) 6376 × 9568"), "{s}");
        assert!(s.contains("Zoom 11 % (anpassad)"));
        assert!(s.contains("zoom mot pekaren"));
        let j = frame(Status::Ready, Source::Jpeg);
        assert!(status_line(&j, false).contains("Inbäddad JPEG 6376 × 9568 (kamerans rendering"));
        let mut plain = frame(Status::Ready, Source::Jpeg);
        plain.path = Some(PathBuf::from("photo.JPG"));
        assert!(source_text(&plain).starts_with("JPEG-fil"));
        let mut z = frame(Status::Ready, Source::Raw);
        z.zoom = Zoom::Scale(2.0);
        assert_eq!(zoom_text(&z), "Zoom 200 %");
        let mut empty = frame(Status::Empty, Source::Raw);
        empty.pos = None;
        assert!(status_line(&empty, true).starts_with("Ingen mapp"));
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
        assert!(status_line(&f, true).contains("förbereder RAW"));
        assert_eq!(indicator_text(&f), "Förbereder RAW …");
        let damaged = DecodeError::damaged("damaged: cut");
        assert!(error_text(&damaged, Source::Raw).contains("JPEG-läget"));
        let none = DecodeError::new("x: the file has no embedded JPEG");
        assert!(error_text(&none, Source::Jpeg).contains("ingen inbäddad JPEG"));
        assert!(error_text(&DecodeError::new("boom"), Source::Raw).contains("boom"));
        assert_eq!(mode_label(Source::Jpeg), "Läge: JPEG (R)");
    }
}
