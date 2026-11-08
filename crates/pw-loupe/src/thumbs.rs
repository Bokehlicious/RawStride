use std::ops::Range;

use egui::ColorImage;
use pw_thumbcache::{Rgb8, ThumbnailEvent};

/// The bookkeeping state of one thumbnail cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellState {
    Missing,
    Requested,
    Ready,
    Failed(String),
}

#[derive(Clone, Debug)]
enum Entry {
    Missing,
    Requested(u32),
    Ready(Option<u32>),
    Failed(String),
}

impl Entry {
    fn cell(&self) -> Option<u32> {
        match self {
            Entry::Requested(cell) => Some(*cell),
            Entry::Ready(cell) => *cell,
            Entry::Missing | Entry::Failed(_) => None,
        }
    }

    fn state(&self) -> CellState {
        match self {
            Entry::Missing => CellState::Missing,
            Entry::Requested(_) => CellState::Requested,
            Entry::Ready(_) => CellState::Ready,
            Entry::Failed(message) => CellState::Failed(message.clone()),
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
            .filter_map(|(index, entry)| match entry {
                Entry::Missing | Entry::Requested(_) => Some(index),
                Entry::Ready(ready_cell) if *ready_cell != Some(cell) => Some(index),
                Entry::Ready(_) | Entry::Failed(_) => None,
            })
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
            Err(error) => Entry::Failed(error.to_string()),
        };
        true
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
                if outside && matches!(entry, Entry::Ready(_) | Entry::Failed(_)) {
                    *entry = Entry::Missing;
                    Some(index)
                } else {
                    None
                }
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
    use pw_thumbcache::ThumbError;
    use std::path::PathBuf;

    fn event(generation: u64, index: usize, result: Result<Rgb8, ThumbError>) -> ThumbnailEvent {
        ThumbnailEvent {
            generation,
            index,
            path: PathBuf::from("image.jpg"),
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
        assert_eq!(
            grid.state(0),
            CellState::Failed("ingen förhandsvisning".to_string())
        );
        assert!(grid.to_request(0..1, 128).is_empty());
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
        assert_eq!(
            grid.state(3),
            CellState::Failed("ingen förhandsvisning".to_string())
        );
        assert_eq!(grid.state(4), CellState::Missing);
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
