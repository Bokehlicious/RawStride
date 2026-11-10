use std::ops::Range;

pub const DEFAULT_THUMBNAIL_SIZE: u32 = 200;
pub const MIN_THUMBNAIL_SIZE: u32 = 96;
pub const MAX_THUMBNAIL_SIZE: u32 = 480;
pub const THUMBNAIL_SIZE_STEP: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridKeyAction {
    Navigate(Navigation),
    OpenLoupe,
    /// Shift + a navigation key: extends the marking (ADR 0014 §4.5).
    Extend(Navigation),
    /// Ctrl+A: marks every visible image.
    SelectAll,
    /// Escape: clears the marking.
    ClearSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoupeKeyAction {
    ShowGrid,
}

/// Number of square cells that fit in `available_width`, with at least one column.
pub fn columns(available_width: u32, cell_size: u32, spacing: u32) -> usize {
    let cell = u64::from(cell_size.max(1));
    let gap = u64::from(spacing);
    let count = (u64::from(available_width) + gap) / (cell + gap);
    usize::try_from(count.max(1)).unwrap_or(usize::MAX)
}

/// Zero-based row containing `index`.
pub fn row(index: usize, columns: usize) -> usize {
    index / columns.max(1)
}

/// Index range containing visible cells and `margin_rows` on either side.
pub fn visible_range(
    count: usize,
    columns: usize,
    cell_size: u32,
    spacing: u32,
    scroll_offset: f32,
    view_height: f32,
    margin_rows: usize,
) -> Range<usize> {
    let columns = columns.max(1);
    if count == 0 || !view_height.is_finite() || view_height <= 0.0 {
        return 0..0;
    }

    let total_rows = count.div_ceil(columns);
    let stride = f64::from(cell_size.max(1)) + f64::from(spacing);
    let offset = if scroll_offset.is_finite() {
        f64::from(scroll_offset.max(0.0))
    } else {
        0.0
    };
    let height = f64::from(view_height);
    let first_visible_row = ((offset / stride).floor() as usize).min(total_rows);
    let end_visible_row = (((offset + height) / stride).ceil() as usize)
        .max(first_visible_row)
        .min(total_rows);
    let first_row = first_visible_row.saturating_sub(margin_rows);
    let end_row = end_visible_row.saturating_add(margin_rows).min(total_rows);

    first_row.saturating_mul(columns)..end_row.saturating_mul(columns).min(count)
}

/// Moves the selected index without wrapping; vertical moves clamp to a short final row.
pub fn navigate(
    count: usize,
    columns: usize,
    current: usize,
    action: Navigation,
    visible_rows: usize,
) -> usize {
    if count == 0 {
        return 0;
    }

    let last = count - 1;
    let current = current.min(last);
    let columns = columns.max(1);
    let page = columns.saturating_mul(visible_rows.max(1));
    match action {
        Navigation::Left => current.saturating_sub(1),
        Navigation::Right => current.saturating_add(1).min(last),
        Navigation::Up => current.saturating_sub(columns),
        Navigation::Down => current.saturating_add(columns).min(last),
        Navigation::PageUp => current.saturating_sub(page),
        Navigation::PageDown => current.saturating_add(page).min(last),
        Navigation::Home => 0,
        Navigation::End => last,
    }
}

/// Maps keys handled while the contact sheet is active. Shift + a navigation key extends the
/// marking, Ctrl+A marks all and Escape clears the marking (ADR 0014 §4.5); every other Ctrl or Alt
/// combination, and Shift with any other key, does nothing.
pub fn grid_key_action(
    key: egui::Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> Option<GridKeyAction> {
    use egui::Key as K;
    if ctrl || alt {
        return (key == K::A && ctrl && !shift && !alt).then_some(GridKeyAction::SelectAll);
    }

    let navigation = match key {
        K::ArrowLeft => Some(Navigation::Left),
        K::ArrowRight => Some(Navigation::Right),
        K::ArrowUp => Some(Navigation::Up),
        K::ArrowDown => Some(Navigation::Down),
        K::PageUp => Some(Navigation::PageUp),
        K::PageDown => Some(Navigation::PageDown),
        K::Home => Some(Navigation::Home),
        K::End => Some(Navigation::End),
        _ => None,
    };
    if let Some(navigation) = navigation {
        return Some(if shift {
            GridKeyAction::Extend(navigation)
        } else {
            GridKeyAction::Navigate(navigation)
        });
    }
    if shift {
        return None;
    }
    match key {
        K::Enter | K::G | K::Space => Some(GridKeyAction::OpenLoupe),
        K::Escape => Some(GridKeyAction::ClearSelection),
        _ => None,
    }
}

/// Maps keys handled while the loupe is active.
pub fn loupe_key_action(key: egui::Key, ctrl: bool, alt: bool) -> Option<LoupeKeyAction> {
    use egui::Key as K;
    if ctrl || alt {
        return None;
    }
    match key {
        K::Escape | K::G | K::Enter => Some(LoupeKeyAction::ShowGrid),
        _ => None,
    }
}

pub fn clamp_thumbnail_size(size: u32) -> u32 {
    size.clamp(MIN_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE)
}

/// Adjusts the thumbnail size by one step in the direction of a Ctrl+wheel movement.
pub fn step_thumbnail_size(size: u32, direction: i32) -> u32 {
    let size = clamp_thumbnail_size(size);
    match direction.cmp(&0) {
        std::cmp::Ordering::Less => size
            .saturating_sub(THUMBNAIL_SIZE_STEP)
            .max(MIN_THUMBNAIL_SIZE),
        std::cmp::Ordering::Equal => size,
        std::cmp::Ordering::Greater => size
            .saturating_add(THUMBNAIL_SIZE_STEP)
            .min(MAX_THUMBNAIL_SIZE),
    }
}

/// Takes whole mouse-wheel notches out of an accumulated wheel movement (in lines; positive is
/// away from the user) and returns them as size steps; the remainder stays for the next frame.
/// One notch of a mouse wheel is one step, however many frames egui spreads it over.
pub fn take_wheel_steps(accumulated: &mut f32) -> i32 {
    if !accumulated.is_finite() {
        *accumulated = 0.0;
        return 0;
    }
    let steps = accumulated.trunc();
    *accumulated -= steps;
    // At most one full range of sizes per frame; protects the cast from huge values.
    steps.clamp(-64.0, 64.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_width_and_always_include_at_least_one() {
        assert_eq!(columns(0, 100, 10), 1);
        assert_eq!(columns(109, 100, 10), 1);
        assert_eq!(columns(110, 100, 10), 1);
        assert_eq!(columns(209, 100, 10), 1);
        assert_eq!(columns(210, 100, 10), 2);
        assert_eq!(columns(440, 100, 10), 4);
        assert_eq!(columns(u32::MAX, 0, 0), u32::MAX as usize);
    }

    #[test]
    fn index_rows_use_the_column_count() {
        assert_eq!(row(0, 4), 0);
        assert_eq!(row(7, 4), 1);
        assert_eq!(row(8, 4), 2);
        assert_eq!(row(7, 0), 7);
    }

    #[test]
    fn visible_range_includes_rows_around_top_middle_and_end() {
        assert_eq!(visible_range(100, 4, 100, 10, 0.0, 200.0, 1), 0..12);
        assert_eq!(visible_range(100, 4, 100, 10, 330.0, 220.0, 1), 8..24);
        assert_eq!(visible_range(10, 4, 100, 10, 230.0, 100.0, 1), 4..10);
        assert_eq!(visible_range(0, 4, 100, 10, 0.0, 100.0, 1), 0..0);
    }

    #[test]
    fn navigation_clamps_all_directions_and_short_final_rows() {
        let count = 10;
        let columns = 4;
        assert_eq!(navigate(count, columns, 0, Navigation::Left, 2), 0);
        assert_eq!(navigate(count, columns, 0, Navigation::Up, 2), 0);
        assert_eq!(navigate(count, columns, 8, Navigation::Right, 2), 9);
        assert_eq!(navigate(count, columns, 5, Navigation::Down, 2), 9);
        assert_eq!(navigate(count, columns, 9, Navigation::Up, 2), 5);
        assert_eq!(navigate(count, columns, 9, Navigation::Down, 2), 9);
        assert_eq!(navigate(count, columns, 5, Navigation::PageUp, 2), 0);
        assert_eq!(navigate(count, columns, 0, Navigation::PageDown, 2), 8);
        assert_eq!(navigate(count, columns, 7, Navigation::Home, 2), 0);
        assert_eq!(navigate(count, columns, 0, Navigation::End, 2), 9);
        assert_eq!(navigate(0, columns, 0, Navigation::End, 2), 0);
    }

    #[test]
    fn keys_are_mapped_separately_for_grid_and_loupe() {
        use egui::Key as K;
        for (key, navigation) in [
            (K::ArrowLeft, Navigation::Left),
            (K::ArrowRight, Navigation::Right),
            (K::ArrowUp, Navigation::Up),
            (K::ArrowDown, Navigation::Down),
            (K::PageUp, Navigation::PageUp),
            (K::PageDown, Navigation::PageDown),
            (K::Home, Navigation::Home),
            (K::End, Navigation::End),
        ] {
            assert_eq!(
                grid_key_action(key, false, false, false),
                Some(GridKeyAction::Navigate(navigation))
            );
        }
        for key in [K::Enter, K::G, K::Space] {
            assert_eq!(
                grid_key_action(key, false, false, false),
                Some(GridKeyAction::OpenLoupe)
            );
        }
        assert_eq!(
            grid_key_action(K::Escape, false, false, false),
            Some(GridKeyAction::ClearSelection)
        );
        assert_eq!(grid_key_action(K::G, true, false, false), None);
        for key in [K::Escape, K::G, K::Enter] {
            assert_eq!(
                loupe_key_action(key, false, false),
                Some(LoupeKeyAction::ShowGrid)
            );
        }
        assert_eq!(loupe_key_action(K::Space, false, false), None);
        assert_eq!(loupe_key_action(K::G, false, true), None);
    }

    #[test]
    fn grid_marking_keys_shift_extends_ctrl_a_marks_all_escape_clears() {
        use egui::Key as K;
        for (key, navigation) in [
            (K::ArrowLeft, Navigation::Left),
            (K::ArrowRight, Navigation::Right),
            (K::ArrowUp, Navigation::Up),
            (K::ArrowDown, Navigation::Down),
            (K::PageUp, Navigation::PageUp),
            (K::PageDown, Navigation::PageDown),
            (K::Home, Navigation::Home),
            (K::End, Navigation::End),
        ] {
            assert_eq!(
                grid_key_action(key, false, true, false),
                Some(GridKeyAction::Extend(navigation))
            );
            // Ctrl or Alt with Shift + navigation still does nothing.
            assert_eq!(grid_key_action(key, true, true, false), None);
            assert_eq!(grid_key_action(key, false, true, true), None);
        }
        assert_eq!(
            grid_key_action(K::A, true, false, false),
            Some(GridKeyAction::SelectAll)
        );
        assert_eq!(grid_key_action(K::A, true, true, false), None);
        assert_eq!(grid_key_action(K::A, false, false, true), None);
        assert_eq!(grid_key_action(K::A, true, false, true), None);
        assert_eq!(grid_key_action(K::A, false, false, false), None);
        assert_eq!(
            grid_key_action(K::Escape, false, false, false),
            Some(GridKeyAction::ClearSelection)
        );
        assert_eq!(grid_key_action(K::Escape, true, false, false), None);
        assert_eq!(grid_key_action(K::Escape, false, true, false), None);
        assert_eq!(grid_key_action(K::Escape, false, false, true), None);
        for key in [K::Enter, K::G, K::Space] {
            assert_eq!(grid_key_action(key, false, true, false), None);
        }
        // The loupe key map is unchanged: Ctrl+A does nothing there.
        assert_eq!(loupe_key_action(K::A, true, false), None);
    }

    #[test]
    fn thumbnail_size_is_clamped_and_steps_at_the_limits() {
        assert_eq!(clamp_thumbnail_size(0), MIN_THUMBNAIL_SIZE);
        assert_eq!(clamp_thumbnail_size(200), 200);
        assert_eq!(clamp_thumbnail_size(u32::MAX), MAX_THUMBNAIL_SIZE);
        assert_eq!(step_thumbnail_size(200, 1), 208);
        assert_eq!(step_thumbnail_size(200, -1), 192);
        assert_eq!(
            step_thumbnail_size(MIN_THUMBNAIL_SIZE, -1),
            MIN_THUMBNAIL_SIZE
        );
        assert_eq!(
            step_thumbnail_size(MAX_THUMBNAIL_SIZE, 1),
            MAX_THUMBNAIL_SIZE
        );
        assert_eq!(step_thumbnail_size(200, 0), 200);
    }

    #[test]
    fn wheel_steps_are_whole_notches_and_keep_the_remainder() {
        let mut accumulated = 0.4;
        assert_eq!(take_wheel_steps(&mut accumulated), 0);
        accumulated += 0.7;
        assert_eq!(take_wheel_steps(&mut accumulated), 1);
        assert!((accumulated - 0.1).abs() < 1e-6);
        accumulated = -2.5;
        assert_eq!(take_wheel_steps(&mut accumulated), -2);
        assert!((accumulated + 0.5).abs() < 1e-6);
        accumulated = f32::NAN;
        assert_eq!(take_wheel_steps(&mut accumulated), 0);
        assert_eq!(accumulated, 0.0);
        accumulated = 1.0e9;
        assert_eq!(take_wheel_steps(&mut accumulated), 64);
    }
}
