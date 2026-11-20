use std::collections::HashSet;
use std::ops::Range;

use egui::ColorImage;
use pw_thumbcache::{Rgb8, ThumbError, ThumbnailEvent};

/// The bookkeeping state of one thumbnail cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellState {
    Missing,
    Requested,
    Ready,
    /// The file has no embedded preview to make a thumbnail from (T116: its own state, so the
    /// drawing code does not compare an error text).
    NoPreview,
    Failed(String),
}

#[derive(Clone, Debug)]
enum Entry {
    Missing,
    Requested(u32),
    Ready(Option<u32>),
    NoPreview,
    Failed(String),
}

impl Entry {
    fn cell(&self) -> Option<u32> {
        match self {
            Entry::Requested(cell) => Some(*cell),
            Entry::Ready(cell) => *cell,
            Entry::Missing | Entry::NoPreview | Entry::Failed(_) => None,
        }
    }

    fn state(&self) -> CellState {
        match self {
            Entry::Missing => CellState::Missing,
            Entry::Requested(_) => CellState::Requested,
            Entry::Ready(_) => CellState::Ready,
            Entry::NoPreview => CellState::NoPreview,
            Entry::Failed(message) => CellState::Failed(message.clone()),
        }
    }

    /// Whether this entry should be submitted for `cell`: missing, requested (a retry), or ready at
    /// another cell size. A failed entry (also one without a preview) is not retried.
    fn wants(&self, cell: u32) -> bool {
        match self {
            Entry::Missing | Entry::Requested(_) => true,
            Entry::Ready(ready_cell) => *ready_cell != Some(cell),
            Entry::NoPreview | Entry::Failed(_) => false,
        }
    }

    /// Resets a ready or failed entry to missing; returns whether it did (its texture is released).
    fn forget(&mut self) -> bool {
        if matches!(self, Entry::Ready(_) | Entry::NoPreview | Entry::Failed(_)) {
            *self = Entry::Missing;
            true
        } else {
            false
        }
    }
}

/// Tracks thumbnail state for one folder generation without holding images or textures.
#[derive(Clone, Debug)]
pub struct ThumbGrid {
    generation: u64,
    entries: Vec<Entry>,
}

impl ThumbGrid {
    /// Creates missing-thumbnail state for every image in the folder.
    pub fn new(generation: u64, count: usize) -> Self {
        Self {
            generation,
            entries: vec![Entry::Missing; count],
        }
    }

    /// Returns indexes that should be submitted for this cell size.
    pub fn to_request(&self, range: Range<usize>, cell: u32) -> Vec<usize> {
        let start = range.start.min(self.entries.len());
        let end = range.end.min(self.entries.len());
        if start >= end {
            return Vec::new();
        }

        self.entries
            .iter()
            .enumerate()
            .take(end)
            .skip(start)
            .filter_map(|(index, entry)| entry.wants(cell).then_some(index))
            .collect()
    }

    /// As `to_request`, for a list of file indexes (the visible cells under a filter, ADR 0014
    /// §4.2), in the given order. Indexes outside this folder are left out.
    pub fn to_request_indexes(&self, indexes: &[usize], cell: u32) -> Vec<usize> {
        indexes
            .iter()
            .copied()
            .filter(|&index| self.entries.get(index).is_some_and(|e| e.wants(cell)))
            .collect()
    }

    /// Marks an in-folder thumbnail as requested at `cell` pixels.
    pub fn mark_requested(&mut self, index: usize, cell: u32) {
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = Entry::Requested(cell);
        }
    }

    /// Records a completion from the worker if it belongs to this folder.
    pub fn apply(&mut self, event: &ThumbnailEvent) -> bool {
        if event.generation != self.generation {
            return false;
        }
        let Some(entry) = self.entries.get_mut(event.index) else {
            return false;
        };
        let cell = entry.cell();
        *entry = match &event.result {
            Ok(_) => Entry::Ready(cell),
            Err(ThumbError::NoPreview) => Entry::NoPreview,
            Err(error) => Entry::Failed(error.to_string()),
        };
        true
    }

    /// Sets the entry at `index` to missing from any state, a requested one included, so its
    /// thumbnail is asked for again (ADR 0024 §2.4, T232: its orientation changed). A request in
    /// flight was sent with the old orientation; its late event lands on the missing entry as
    /// ready without a cell size, which is asked for again. Returns whether the entry was ready
    /// (a texture to release); an index outside this folder changes nothing and returns `false`.
    pub fn forget(&mut self, index: usize) -> bool {
        let Some(entry) = self.entries.get_mut(index) else {
            return false;
        };
        let was_ready = matches!(entry, Entry::Ready(_));
        *entry = Entry::Missing;
        was_ready
    }

    /// Returns the visible state of an index, or `Missing` when it is outside this folder.
    pub fn state(&self, index: usize) -> CellState {
        self.entries
            .get(index)
            .map(Entry::state)
            .unwrap_or(CellState::Missing)
    }

    /// Resets ready and failed entries outside `keep`, returning their indexes for texture release.
    pub fn forget_outside(&mut self, keep: Range<usize>) -> Vec<usize> {
        let start = keep.start.min(self.entries.len());
        let end = keep.end.min(self.entries.len());
        self.entries
            .iter_mut()
            .enumerate()
            .filter_map(|(index, entry)| {
                let outside = index < start || index >= end || start >= end;
                (outside && entry.forget()).then_some(index)
            })
            .collect()
    }

    /// As `forget_outside`, for a list of file indexes to keep (ADR 0014 §4.2): resets ready and
    /// failed entries not in `keep`, returning their indexes in ascending order.
    pub fn forget_not_in(&mut self, keep: &[usize]) -> Vec<usize> {
        let keep: HashSet<usize> = keep.iter().copied().collect();
        self.entries
            .iter_mut()
            .enumerate()
            .filter_map(|(index, entry)| {
                (!keep.contains(&index) && entry.forget()).then_some(index)
            })
            .collect()
    }
}

/// Converts a cell size in points to physical pixels, rounded up to a 128-pixel step.
pub fn request_cell(size_points: u32, pixels_per_point: f32) -> u32 {
    let scale = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        f64::from(pixels_per_point)
    } else {
        1.0
    };
    let pixels = f64::from(size_points) * scale;
    let max_steps = u32::MAX / 128;
    let steps = ((pixels / 128.0).ceil().max(1.0)).min(f64::from(max_steps)) as u32;
    steps * 128
}

/// Converts an RGB thumbnail to egui's image type when its pixel buffer has the expected length.
pub fn color_image(image: &Rgb8) -> Option<ColorImage> {
    let width = usize::try_from(image.width).ok()?;
    let height = usize::try_from(image.height).ok()?;
    let expected_len = width.checked_mul(height)?.checked_mul(3)?;
    if image.data.len() != expected_len {
        return None;
    }
    Some(ColorImage::from_rgb([width, height], &image.data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn event(generation: u64, index: usize, result: Result<Rgb8, ThumbError>) -> ThumbnailEvent {
        ThumbnailEvent {
            generation,
            index,
            path: PathBuf::from("image.jpg"),
            orientation: None,
            result,
        }
    }

    #[test]
    fn requests_missing_cells_and_retries_requested_cells() {
        let mut grid = ThumbGrid::new(1, 4);
        assert_eq!(grid.to_request(1..10, 256), vec![1, 2, 3]);

        grid.mark_requested(1, 256);
        assert_eq!(grid.to_request(1..2, 256), vec![1]);
        grid.apply(&event(
            1,
            1,
            Ok(Rgb8 {
                width: 1,
                height: 1,
                data: vec![0, 0, 0],
            }),
        ));
        assert_eq!(grid.state(1), CellState::Ready);
        assert!(grid.to_request(1..2, 256).is_empty());
        assert_eq!(grid.to_request(1..2, 384), vec![1]);
    }

    #[test]
    fn failed_cells_are_not_retried() {
        let mut grid = ThumbGrid::new(1, 2);
        grid.mark_requested(0, 128);
        assert!(grid.apply(&event(1, 0, Err(ThumbError::NoPreview))));
        assert_eq!(grid.state(0), CellState::NoPreview);
        assert!(grid.to_request(0..1, 128).is_empty());
    }

    /// A file without a preview gets its own state (T116); any other error keeps its text.
    #[test]
    fn only_a_missing_preview_gives_no_preview() {
        let mut grid = ThumbGrid::new(1, 2);
        grid.mark_requested(0, 128);
        grid.mark_requested(1, 128);
        assert!(grid.apply(&event(1, 0, Err(ThumbError::NoPreview))));
        assert!(grid.apply(&event(
            1,
            1,
            Err(ThumbError::Unreadable("broken".to_string()))
        )));
        assert_eq!(grid.state(0), CellState::NoPreview);
        assert_eq!(
            grid.state(1),
            CellState::Failed("unreadable thumbnail: broken".to_string())
        );
        assert!(grid.to_request(0..2, 128).is_empty());
    }

    #[test]
    fn ignores_events_from_other_generations_and_out_of_folder_indexes() {
        let mut grid = ThumbGrid::new(2, 1);
        grid.mark_requested(0, 128);
        let success = Ok(Rgb8 {
            width: 1,
            height: 1,
            data: vec![0, 0, 0],
        });
        assert!(!grid.apply(&event(1, 0, success.clone())));
        assert_eq!(grid.state(0), CellState::Requested);
        assert!(!grid.apply(&event(2, 1, success)));
        assert_eq!(grid.state(0), CellState::Requested);
    }

    #[test]
    fn forgets_ready_and_failed_entries_outside_keep() {
        let mut grid = ThumbGrid::new(1, 5);
        let success = Ok(Rgb8 {
            width: 1,
            height: 1,
            data: vec![0, 0, 0],
        });
        for index in [0, 2, 4] {
            grid.mark_requested(index, 128);
            assert!(grid.apply(&event(1, index, success.clone())));
        }
        grid.mark_requested(3, 128);
        assert!(grid.apply(&event(1, 3, Err(ThumbError::NoPreview))));

        assert_eq!(grid.forget_outside(1..4), vec![0, 4]);
        assert_eq!(grid.state(0), CellState::Missing);
        assert_eq!(grid.state(2), CellState::Ready);
        assert_eq!(grid.state(3), CellState::NoPreview);
        assert_eq!(grid.state(4), CellState::Missing);
    }

    /// A grid of six cells: 0 missing, 1 requested at 128, 2 ready at 128, 3 ready at 256,
    /// 4 failed, 5 missing.
    fn mixed_grid() -> ThumbGrid {
        let mut grid = ThumbGrid::new(1, 6);
        let success = Ok(Rgb8 {
            width: 1,
            height: 1,
            data: vec![0, 0, 0],
        });
        grid.mark_requested(1, 128);
        grid.mark_requested(2, 128);
        assert!(grid.apply(&event(1, 2, success.clone())));
        grid.mark_requested(3, 256);
        assert!(grid.apply(&event(1, 3, success)));
        grid.mark_requested(4, 128);
        assert!(grid.apply(&event(1, 4, Err(ThumbError::NoPreview))));
        grid
    }

    #[test]
    fn to_request_indexes_follows_the_rules_of_to_request_in_the_given_order() {
        let grid = mixed_grid();
        // Missing and requested are (re)requested, ready at another size is requested again,
        // ready at this size and failed are not; out-of-folder indexes are left out.
        assert_eq!(
            grid.to_request_indexes(&[5, 4, 3, 2, 1, 0, 9], 128),
            vec![5, 3, 1, 0]
        );
        assert_eq!(grid.to_request_indexes(&[2, 3], 256), vec![2]);
        assert!(grid.to_request_indexes(&[], 128).is_empty());
        // The same answer as the range method for a contiguous list.
        assert_eq!(
            grid.to_request_indexes(&[0, 1, 2, 3, 4, 5], 128),
            grid.to_request(0..6, 128)
        );
    }

    #[test]
    fn forget_not_in_resets_ready_and_failed_entries_outside_the_list() {
        let mut grid = mixed_grid();
        // Keep 3 and 4 (not contiguous with the others); 2 (ready) is forgotten, the requested and
        // missing ones are left alone.
        assert_eq!(grid.forget_not_in(&[4, 3, 9]), vec![2]);
        assert_eq!(grid.state(1), CellState::Requested);
        assert_eq!(grid.state(2), CellState::Missing);
        assert_eq!(grid.state(3), CellState::Ready);
        assert_eq!(grid.state(4), CellState::NoPreview);
        // Keeping nothing forgets the ready and failed rest.
        assert_eq!(grid.forget_not_in(&[]), vec![3, 4]);
        assert_eq!(grid.state(4), CellState::Missing);
        assert!(grid.forget_not_in(&[]).is_empty());
    }

    /// T232: a ready cell forgotten (its orientation changed) is asked for again at the same size.
    #[test]
    fn forget_resets_a_ready_cell_so_it_is_asked_for_again() {
        let mut grid = mixed_grid();
        assert!(grid.to_request_indexes(&[2], 128).is_empty());
        assert!(grid.forget(2));
        assert_eq!(grid.state(2), CellState::Missing);
        assert_eq!(grid.to_request_indexes(&[2], 128), vec![2]);
    }

    /// T232: a requested cell is forgotten too; the late event of the old request leaves it
    /// asked for again.
    #[test]
    fn forget_resets_a_requested_cell_and_its_late_event_leaves_it_asked_for() {
        let mut grid = mixed_grid();
        assert!(!grid.forget(1));
        assert_eq!(grid.state(1), CellState::Missing);
        let success = Ok(Rgb8 {
            width: 1,
            height: 1,
            data: vec![0, 0, 0],
        });
        assert!(grid.apply(&event(1, 1, success)));
        assert_eq!(grid.to_request_indexes(&[1], 128), vec![1]);
    }

    /// T232: an index outside the grid changes nothing.
    #[test]
    fn forget_outside_the_grid_returns_false() {
        let mut grid = mixed_grid();
        assert!(!grid.forget(6));
        assert!(!grid.forget(usize::MAX));
        assert_eq!(grid.state(2), CellState::Ready);
        assert_eq!(grid.state(1), CellState::Requested);
    }

    #[test]
    fn request_cell_rounds_up_by_128_and_sanitizes_scale() {
        assert_eq!(request_cell(200, 1.0), 256);
        assert_eq!(request_cell(200, 1.5), 384);
        assert_eq!(request_cell(96, 1.0), 128);
        assert_eq!(request_cell(200, f32::NAN), 256);
        assert_eq!(request_cell(200, 0.0), 256);
        assert_eq!(request_cell(200, -1.0), 256);
    }

    #[test]
    fn converts_valid_rgb_data_and_rejects_wrong_length() {
        let image = Rgb8 {
            width: 2,
            height: 1,
            data: vec![255, 0, 0, 0, 255, 0],
        };
        let converted = color_image(&image).expect("valid RGB image");
        assert_eq!(converted.size, [2, 1]);
        assert_eq!(
            converted.pixels,
            vec![
                egui::Color32::from_rgb(255, 0, 0),
                egui::Color32::from_rgb(0, 255, 0)
            ]
        );

        let malformed = Rgb8 {
            width: 2,
            height: 1,
            data: vec![255, 0, 0],
        };
        assert!(color_image(&malformed).is_none());
    }
}
