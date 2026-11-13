//! The preview (ADR 0017 §2.2, §5, §7 U3, T111): the full-screen mode of the main window with a
//! title line, the image area (today's loupe), its own right column and the filmstrip. Plain Rust,
//! no I/O (GR11): which parts are shown, the filmstrip's range, scroll target and hit test, the
//! thumbnail gate of §5 and the window state the preview returns to. `main.rs` draws.

use std::ops::Range;

use pw_viewcore::Status;

use crate::settings::{ShellSettings, WindowRect};
use crate::shell::ColumnState;

/// The height of a filmstrip thumbnail in points (U3-Q3); the aspect is kept inside the cell.
pub const FILMSTRIP_IMAGE_HEIGHT: f32 = 96.0;
/// The width of a filmstrip cell in points: a 3:2 image at [`FILMSTRIP_IMAGE_HEIGHT`].
pub const FILMSTRIP_CELL_WIDTH: f32 = 144.0;
/// The height of a cell's label bar (ADR 0013), as in the contact sheet.
pub const FILMSTRIP_LABEL_HEIGHT: f32 = 18.0;
/// The space between two cells, as in the contact sheet.
pub const FILMSTRIP_SPACING: f32 = 8.0;
/// Cells beyond the strip's edges, on each side, whose thumbnails are asked for (§5).
pub const FILMSTRIP_MARGIN: usize = 3;

/// The distance from one cell's left edge to the next one's.
fn stride() -> f32 {
    FILMSTRIP_CELL_WIDTH + FILMSTRIP_SPACING
}

/// Whether the thumbnail worker may run in the preview (§5, U3-Q4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbGate {
    Run,
    Pause,
}

/// The thumbnail gate (§5, U3-Q4): the worker runs only while the shown image is drawn (`Ready`)
/// or known to fail (`Error`); while a step's image is prepared, and with nothing shown, it
/// pauses, so the filmstrip never slows a step.
pub fn thumbnail_gate(status: &Status) -> ThumbGate {
    match status {
        Status::Ready | Status::Error(_) => ThumbGate::Run,
        Status::Preparing { .. } | Status::Empty => ThumbGate::Pause,
    }
}

/// The width of the filmstrip's cells side by side, in points.
pub fn filmstrip_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * stride() - FILMSTRIP_SPACING
    }
}

/// `offset` (how far the strip is scrolled, in points) kept between the start and the last
/// offset that still fills a strip `width` points wide; 0 when the list is shorter than the strip.
pub fn clamp_offset(offset: f32, count: usize, width: f32) -> f32 {
    let max = (filmstrip_width(count) - width.max(0.0)).max(0.0);
    if offset.is_finite() {
        offset.clamp(0.0, max)
    } else {
        0.0
    }
}

/// The positions in the list of the cells in a strip `width` points wide scrolled by `offset`,
/// plus `margin` cells on each side, inside `0..count`.
pub fn filmstrip_range(count: usize, offset: f32, width: f32, margin: usize) -> Range<usize> {
    if count == 0 || !offset.is_finite() || !width.is_finite() {
        return 0..0;
    }
    let first = (offset.max(0.0) / stride()).floor() as usize;
    let end = ((offset.max(0.0) + width.max(0.0)) / stride()).ceil() as usize;
    let start = first.saturating_sub(margin).min(count);
    let end = end.saturating_add(margin).min(count);
    start..end.max(start)
}

/// The offset after a step to `position`: unchanged when its cell is already in view, otherwise
/// the nearest offset that shows the whole cell (its left edge when the strip is narrower than a
/// cell); clamped as `clamp_offset`.
pub fn scroll_target(count: usize, position: usize, offset: f32, width: f32) -> f32 {
    let offset = clamp_offset(offset, count, width);
    if position >= count {
        return offset;
    }
    let start = position as f32 * stride();
    let end = start + FILMSTRIP_CELL_WIDTH;
    let target = if start < offset || width < FILMSTRIP_CELL_WIDTH {
        start
    } else if end > offset + width {
        end - width
    } else {
        offset
    };
    clamp_offset(target, count, width)
}

/// The position in the list of the cell under `x` (points from the strip's left edge) in a strip
/// scrolled by `offset`; `None` between two cells or past the last one.
pub fn hit_test(count: usize, offset: f32, x: f32) -> Option<usize> {
    let at = x + offset;
    if !at.is_finite() || at < 0.0 {
        return None;
    }
    let position = (at / stride()).floor() as usize;
    let inside = at - position as f32 * stride();
    (position < count && inside <= FILMSTRIP_CELL_WIDTH).then_some(position)
}

/// The parts of the preview beside the title line and the image (§2.2, U3-Q2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewParts {
    /// The right column, open or collapsed; `None` when hidden with `H`.
    pub column: Option<ColumnState>,
    /// Whether the filmstrip is drawn.
    pub filmstrip: bool,
}

/// What `shell` shows in the preview: `H` (`preview_hidden`) hides the right column and the
/// filmstrip together and leaves their own states as they were.
pub fn preview_parts(shell: &ShellSettings) -> PreviewParts {
    if shell.preview_hidden {
        PreviewParts {
            column: None,
            filmstrip: false,
        }
    } else {
        PreviewParts {
            column: Some(shell.preview_column),
            filmstrip: shell.filmstrip,
        }
    }
}

/// Whether the preview column's Info section is shown, so its Exif summary may be read (T110's
/// rules): the column shown and open and the section open.
pub fn preview_info_shown(shell: &ShellSettings) -> bool {
    preview_parts(shell).column == Some(ColumnState::Open)
        && shell.preview_info == ColumnState::Open
}

/// `H` in the preview: hides the right column and the filmstrip, or brings them back as they were.
pub fn toggle_preview_hidden(shell: &mut ShellSettings) {
    shell.preview_hidden = !shell.preview_hidden;
}

/// `I` in the preview: opens or collapses the preview's right column, as `I` does for the main
/// window's right column; from the hidden parts (`H`) it brings them back with the column open.
pub fn toggle_preview_column(shell: &mut ShellSettings) {
    if shell.preview_hidden {
        shell.preview_hidden = false;
        shell.preview_column = ColumnState::Open;
    } else {
        shell.preview_column = shell.preview_column.toggled();
    }
}

/// The main window's state before the preview made it full screen (U3-Q1, U3-Q6): what closing
/// the preview returns to and what `settings.txt` gets when RawStride is closed in the preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowBefore {
    pub fullscreen: bool,
    pub maximized: bool,
    pub window: Option<WindowRect>,
}

impl WindowBefore {
    /// The window's remembered state when the preview opens.
    pub fn of(shell: &ShellSettings) -> Self {
        Self {
            fullscreen: shell.fullscreen,
            maximized: shell.window_maximized,
            window: shell.window,
        }
    }

    /// The settings to save while the preview is shown: `shell` with the window as it was before
    /// the preview, not the preview's full screen.
    pub fn saved(&self, shell: &ShellSettings) -> ShellSettings {
        ShellSettings {
            fullscreen: self.fullscreen,
            window_maximized: self.maximized,
            window: self.window,
            ..*shell
        }
    }

    /// What closing the preview does to the window.
    pub fn restore(&self) -> Restore {
        if self.fullscreen {
            Restore::StayFullscreen
        } else if self.maximized {
            Restore::Maximized
        } else if let Some(window) = self.window {
            Restore::Placed(window)
        } else {
            Restore::Normal
        }
    }
}

/// What closing the preview does to the window (U3-Q6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Restore {
    /// F11 had made the window full screen: it stays so.
    StayFullscreen,
    /// Out of full screen, maximized.
    Maximized,
    /// Out of full screen, at its position and size.
    Placed(WindowRect),
    /// Out of full screen, as the system had it (no normal size was known).
    Normal,
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use pw_viewcore::DecodeError;

    use super::*;

    #[test]
    fn the_gate_runs_on_ready_and_error_and_pauses_on_preparing_and_empty() {
        assert_eq!(thumbnail_gate(&Status::Ready), ThumbGate::Run);
        let error = DecodeError {
            message: "broken".to_owned(),
            damaged: true,
        };
        assert_eq!(thumbnail_gate(&Status::Error(error)), ThumbGate::Run);
        for indicator in [false, true] {
            let preparing = Status::Preparing {
                since: Instant::now(),
                indicator,
            };
            assert_eq!(thumbnail_gate(&preparing), ThumbGate::Pause);
        }
        assert_eq!(thumbnail_gate(&Status::Empty), ThumbGate::Pause);
    }

    /// A strip five cells wide: five cells and their spaces, 5 × 152 = 760 points.
    const WIDTH: f32 = 760.0;

    #[test]
    fn the_range_is_the_cells_in_view_plus_the_margin() {
        assert_eq!(filmstrip_range(100, 0.0, WIDTH, 0), 0..5);
        assert_eq!(filmstrip_range(100, 0.0, WIDTH, 3), 0..8);
        // Scrolled by ten cells: 10..15 in view, 7..18 with the margin.
        assert_eq!(filmstrip_range(100, 1520.0, WIDTH, 3), 7..18);
        // The end is cut at the list's length; an empty list gives nothing.
        assert_eq!(filmstrip_range(4, 0.0, WIDTH, 3), 0..4);
        assert_eq!(filmstrip_range(0, 0.0, WIDTH, 3), 0..0);
        assert_eq!(filmstrip_range(10, f32::NAN, WIDTH, 3), 0..0);
    }

    #[test]
    fn a_step_scrolls_the_current_cell_into_view() {
        let count = 100;
        // In view: unchanged.
        assert_eq!(scroll_target(count, 2, 0.0, WIDTH), 0.0);
        // A step right past the edge: the cell's right edge at the strip's right edge.
        let offset = scroll_target(count, 5, 0.0, WIDTH);
        assert_eq!(offset, 5.0 * 152.0 + 144.0 - WIDTH);
        assert!(filmstrip_range(count, offset, WIDTH, 0).contains(&5));
        // A step left past the edge: the cell's left edge at the strip's left edge.
        assert_eq!(scroll_target(count, 3, 1520.0, WIDTH), 3.0 * 152.0);
        // Every step keeps the current cell wholly in view.
        let mut offset = 0.0;
        for position in (0..count).chain((0..count).rev()) {
            offset = scroll_target(count, position, offset, WIDTH);
            let start = position as f32 * 152.0;
            assert!(
                start >= offset && start + 144.0 <= offset + WIDTH,
                "{position}"
            );
        }
    }

    #[test]
    fn the_first_and_last_cells_scroll_to_the_ends() {
        let count = 100;
        assert_eq!(scroll_target(count, 0, 5000.0, WIDTH), 0.0);
        let last = scroll_target(count, count - 1, 0.0, WIDTH);
        assert_eq!(last, filmstrip_width(count) - WIDTH);
        assert_eq!(clamp_offset(last + 500.0, count, WIDTH), last);
        assert_eq!(filmstrip_range(count, last, WIDTH, 0).end, count);
    }

    #[test]
    fn a_list_shorter_than_the_strip_never_scrolls() {
        for position in 0..3 {
            assert_eq!(scroll_target(3, position, 0.0, WIDTH), 0.0);
            assert_eq!(scroll_target(3, position, 400.0, WIDTH), 0.0);
        }
        assert_eq!(clamp_offset(50.0, 3, WIDTH), 0.0);
        assert_eq!(filmstrip_range(3, 0.0, WIDTH, 3), 0..3);
        // A strip narrower than one cell shows the cell's left edge.
        assert_eq!(scroll_target(10, 4, 0.0, 100.0), 4.0 * 152.0);
    }

    #[test]
    fn a_click_hits_its_cell_and_a_click_between_cells_hits_none() {
        assert_eq!(hit_test(10, 0.0, 0.0), Some(0));
        assert_eq!(hit_test(10, 0.0, 143.0), Some(0));
        assert_eq!(hit_test(10, 0.0, 148.0), None, "between cells 0 and 1");
        assert_eq!(hit_test(10, 0.0, 152.0), Some(1));
        assert_eq!(hit_test(10, 304.0, 10.0), Some(2), "scrolled by two cells");
        assert_eq!(hit_test(10, 0.0, 9.0 * 152.0 + 20.0), Some(9));
        assert_eq!(
            hit_test(10, 0.0, 10.0 * 152.0 + 20.0),
            None,
            "past the last cell"
        );
        assert_eq!(hit_test(10, 0.0, -1.0), None);
    }

    #[test]
    fn h_hides_the_column_and_the_filmstrip_and_brings_them_back_as_they_were() {
        let mut shell = ShellSettings {
            preview_column: ColumnState::Collapsed,
            ..ShellSettings::default()
        };
        let before = preview_parts(&shell);
        assert_eq!(
            before,
            PreviewParts {
                column: Some(ColumnState::Collapsed),
                filmstrip: true
            }
        );
        toggle_preview_hidden(&mut shell);
        assert_eq!(
            preview_parts(&shell),
            PreviewParts {
                column: None,
                filmstrip: false
            }
        );
        toggle_preview_hidden(&mut shell);
        assert_eq!(preview_parts(&shell), before);
        // A hidden filmstrip stays hidden when the column comes back.
        shell.filmstrip = false;
        toggle_preview_hidden(&mut shell);
        toggle_preview_hidden(&mut shell);
        assert!(!preview_parts(&shell).filmstrip);
    }

    #[test]
    fn i_opens_and_collapses_the_preview_column_and_brings_back_the_hidden_parts() {
        let mut shell = ShellSettings::default();
        assert_eq!(shell.preview_column, ColumnState::Open);
        assert!(preview_info_shown(&shell));
        toggle_preview_column(&mut shell);
        assert_eq!(shell.preview_column, ColumnState::Collapsed);
        assert!(!preview_info_shown(&shell));
        toggle_preview_column(&mut shell);
        assert_eq!(shell.preview_column, ColumnState::Open);
        // The main window's right column is not touched.
        assert_eq!(shell.right_column, ColumnState::Collapsed);
        // From H, I brings the column back open (and the filmstrip with it).
        shell.preview_column = ColumnState::Collapsed;
        toggle_preview_hidden(&mut shell);
        assert!(!preview_info_shown(&shell));
        toggle_preview_column(&mut shell);
        assert!(!shell.preview_hidden);
        assert_eq!(
            preview_parts(&shell),
            PreviewParts {
                column: Some(ColumnState::Open),
                filmstrip: true
            }
        );
        // A collapsed Info section is not read.
        shell.preview_info = ColumnState::Collapsed;
        assert!(!preview_info_shown(&shell));
    }

    fn rect() -> WindowRect {
        WindowRect {
            x: 100,
            y: 80,
            width: 1400,
            height: 900,
            scale: 1.5,
        }
    }

    #[test]
    fn closing_returns_to_maximized_and_saves_maximized() {
        let shell = ShellSettings {
            window_maximized: true,
            window: Some(rect()),
            ..ShellSettings::default()
        };
        let before = WindowBefore::of(&shell);
        assert_eq!(before.restore(), Restore::Maximized);
        // While the preview is full screen, the saved state is the one from before.
        let in_preview = ShellSettings {
            fullscreen: true,
            window_maximized: false,
            ..shell
        };
        let saved = before.saved(&in_preview);
        assert!(!saved.fullscreen);
        assert!(saved.window_maximized);
        assert_eq!(saved.window, Some(rect()));
    }

    #[test]
    fn closing_after_f11_stays_full_screen_and_saves_full_screen() {
        let shell = ShellSettings {
            fullscreen: true,
            window: Some(rect()),
            ..ShellSettings::default()
        };
        let before = WindowBefore::of(&shell);
        assert_eq!(before.restore(), Restore::StayFullscreen);
        let saved = before.saved(&shell);
        assert!(saved.fullscreen);
        assert_eq!(saved.window, Some(rect()));
    }

    #[test]
    fn closing_returns_to_a_saved_size_and_saves_it() {
        let shell = ShellSettings {
            window: Some(rect()),
            preview_column_width: 333,
            ..ShellSettings::default()
        };
        let before = WindowBefore::of(&shell);
        assert_eq!(before.restore(), Restore::Placed(rect()));
        let in_preview = ShellSettings {
            fullscreen: true,
            window_maximized: true,
            window: None,
            preview_hidden: true,
            ..shell
        };
        let saved = before.saved(&in_preview);
        assert!(!saved.fullscreen);
        assert!(!saved.window_maximized);
        assert_eq!(saved.window, Some(rect()));
        // Everything else is saved as it is now.
        assert!(saved.preview_hidden);
        assert_eq!(saved.preview_column_width, 333);
        // Without a known size the system places the window.
        let unknown = WindowBefore::of(&ShellSettings::default());
        assert_eq!(unknown.restore(), Restore::Normal);
    }
}
