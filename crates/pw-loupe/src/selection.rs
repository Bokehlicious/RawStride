//! The marking of the contact sheet (ADR 0014 §2.5, §2.6, §2.8, Q5): the rules of `clickCell`,
//! `pickRange`, `pickAll`, `clearPicked`, `go` and `ratingTargets` in `app.js` as plain Rust.
//!
//! Marked images are file indexes. Every position (ranges, display order) is taken from a
//! `visible` argument: the file indexes shown, in display order.

use std::collections::BTreeSet;

use crate::tabs::ViewMode;

/// The marked images of a tab and the anchor of a Shift range.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    marked: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    /// A plain click: clears the marking; the clicked image becomes the anchor.
    pub fn click(&mut self, index: usize) {
        self.marked.clear();
        self.anchor = Some(index);
    }

    /// Ctrl+click: toggles `index`. With nothing marked, `current` is marked first; the only marked
    /// image stays marked. The clicked image becomes the anchor.
    pub fn ctrl_click(&mut self, current: usize, index: usize) {
        if self.marked.is_empty() {
            self.marked.insert(current);
        }
        if self.marked.contains(&index) && self.marked.len() > 1 {
            self.marked.remove(&index);
        } else {
            self.marked.insert(index);
        }
        self.anchor = Some(index);
    }

    /// Shift+click and Shift+navigation: the marking becomes every visible image between the anchor
    /// and `index` (inclusive, by display position). With nothing marked, `current` becomes the
    /// anchor first. If only the anchor is not visible, only `index` is marked. If `index` is not
    /// visible, nothing changes: a hidden image is never marked (S5). The anchor does not change.
    pub fn extend_to(&mut self, visible: &[usize], current: usize, index: usize) {
        let Some(to) = visible.iter().position(|&shown| shown == index) else {
            return;
        };
        if self.marked.is_empty() {
            self.anchor = Some(current);
        }
        let from = self
            .anchor
            .and_then(|anchor| visible.iter().position(|&shown| shown == anchor));
        self.marked.clear();
        match from {
            Some(from) => {
                let range = visible.get(from.min(to)..=from.max(to)).unwrap_or(&[]);
                self.marked.extend(range.iter().copied());
            }
            None => {
                self.marked.insert(index);
            }
        }
    }

    /// Ctrl+A: the marking becomes every visible image, as `pickAll` does (`clearPicked` first);
    /// the anchor is unchanged.
    pub fn all(&mut self, visible: &[usize]) {
        self.marked.clear();
        self.marked.extend(visible.iter().copied());
    }

    /// Escape or a plain step: unmarks every image; the anchor is unchanged.
    pub fn clear(&mut self) {
        self.marked.clear();
    }

    /// Unmarks the images the filter hides (§2.8).
    pub fn retain_visible(&mut self, visible: &[usize]) {
        let shown: BTreeSet<usize> = visible.iter().copied().collect();
        self.marked.retain(|index| shown.contains(index));
    }

    /// The number of marked images.
    pub fn count(&self) -> usize {
        self.marked.len()
    }

    pub fn is_marked(&self, index: usize) -> bool {
        self.marked.contains(&index)
    }

    /// The anchor of a Shift range, if any.
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    /// The marked images in display order.
    pub fn marked_in(&self, visible: &[usize]) -> Vec<usize> {
        visible
            .iter()
            .copied()
            .filter(|index| self.marked.contains(index))
            .collect()
    }

    /// The images a rating key acts on (§2.6): in the grid, every marked image in display order if
    /// more than one is marked and `current` is one of them; otherwise, and always in the loupe,
    /// the current image.
    pub fn targets(&self, visible: &[usize], current: usize, mode: ViewMode) -> Vec<usize> {
        match mode {
            ViewMode::Grid if self.count() > 1 && self.is_marked(current) => {
                self.marked_in(visible)
            }
            _ => vec![current],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `app.js` setup: 12 images, all visible.
    fn twelve() -> Vec<usize> {
        (0..12).collect()
    }

    #[test]
    fn click_then_shift_click_marks_the_range_and_ctrl_click_adds_one() {
        let visible = twelve();
        let mut selection = Selection::default();
        selection.click(5);
        assert_eq!(selection.count(), 0);
        assert_eq!(selection.anchor(), Some(5));

        selection.extend_to(&visible, 5, 8);
        assert_eq!(selection.count(), 4);
        assert_eq!(selection.marked_in(&visible), vec![5, 6, 7, 8]);
        assert_eq!(selection.anchor(), Some(5));

        selection.ctrl_click(8, 10);
        assert_eq!(selection.count(), 5);
        assert_eq!(selection.marked_in(&visible), vec![5, 6, 7, 8, 10]);
        assert_eq!(selection.anchor(), Some(10));

        // Ctrl+click on a marked image among several unmarks it.
        selection.ctrl_click(10, 6);
        assert_eq!(selection.marked_in(&visible), vec![5, 7, 8, 10]);
    }

    #[test]
    fn ctrl_click_on_the_only_marked_image_keeps_it() {
        let visible = twelve();
        let mut selection = Selection::default();
        selection.ctrl_click(3, 3);
        assert_eq!(selection.marked_in(&visible), vec![3]);
        selection.ctrl_click(3, 3);
        assert_eq!(selection.marked_in(&visible), vec![3]);
        assert_eq!(selection.anchor(), Some(3));
    }

    #[test]
    fn ctrl_click_with_nothing_marked_marks_current_and_index() {
        let visible = twelve();
        let mut selection = Selection::default();
        selection.ctrl_click(2, 7);
        assert_eq!(selection.marked_in(&visible), vec![2, 7]);
        assert_eq!(selection.anchor(), Some(7));
    }

    #[test]
    fn shift_range_backwards() {
        let visible = twelve();
        let mut selection = Selection::default();
        selection.click(9);
        selection.extend_to(&visible, 9, 4);
        assert_eq!(selection.marked_in(&visible), vec![4, 5, 6, 7, 8, 9]);
        assert_eq!(selection.anchor(), Some(9));
    }

    #[test]
    fn shift_from_an_anchor_that_is_no_longer_visible_marks_only_the_index() {
        let mut selection = Selection::default();
        selection.ctrl_click(2, 3);
        assert_eq!(selection.marked_in(&twelve()), vec![2, 3]);
        assert_eq!(selection.anchor(), Some(3));
        // The filter now hides 3, the anchor; 2 stays marked.
        let visible = vec![0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
        selection.retain_visible(&visible);
        assert_eq!(selection.marked_in(&visible), vec![2]);
        selection.extend_to(&visible, 2, 8);
        assert_eq!(selection.marked_in(&visible), vec![8]);
        assert_eq!(selection.anchor(), Some(3), "the anchor does not change");
    }

    #[test]
    fn shift_range_follows_display_positions_under_a_filter() {
        // Visible file indexes, in display order, with gaps.
        let visible = vec![1, 3, 6, 9, 11];
        let mut selection = Selection::default();
        selection.click(3);
        selection.extend_to(&visible, 3, 9);
        assert_eq!(selection.marked_in(&visible), vec![3, 6, 9]);
        assert!(!selection.is_marked(4));
    }

    #[test]
    fn all_then_clear_keep_the_anchor() {
        let visible = vec![0, 2, 4, 6];
        let mut selection = Selection::default();
        selection.click(2);
        selection.all(&visible);
        assert_eq!(selection.count(), 4);
        assert_eq!(selection.marked_in(&visible), visible);
        assert!(!selection.is_marked(1));
        assert_eq!(selection.anchor(), Some(2));
        selection.clear();
        assert_eq!(selection.count(), 0);
        assert_eq!(selection.anchor(), Some(2));
    }

    #[test]
    fn extend_to_a_hidden_index_changes_nothing() {
        let visible = vec![0, 1, 2, 4, 5];
        let mut selection = Selection::default();
        selection.click(1);
        selection.extend_to(&visible, 1, 4);
        assert_eq!(selection.marked_in(&twelve()), vec![1, 2, 4]);

        // 3 is hidden: neither the marking nor the anchor changes.
        selection.extend_to(&visible, 4, 3);
        assert_eq!(selection.marked_in(&twelve()), vec![1, 2, 4]);
        assert!(!selection.is_marked(3));
        assert_eq!(selection.anchor(), Some(1));

        // Also with nothing marked: the anchor is not moved to current.
        let mut empty = Selection::default();
        empty.click(0);
        empty.extend_to(&visible, 2, 3);
        assert_eq!(empty.count(), 0);
        assert_eq!(empty.anchor(), Some(0));
    }

    #[test]
    fn all_replaces_the_marking() {
        let mut selection = Selection::default();
        selection.ctrl_click(2, 3);
        assert_eq!(selection.marked_in(&twelve()), vec![2, 3]);
        selection.all(&[0, 1]);
        assert_eq!(selection.marked_in(&twelve()), vec![0, 1]);
        assert_eq!(selection.count(), 2);
    }

    #[test]
    fn retain_visible_unmarks_hidden_images() {
        let mut selection = Selection::default();
        selection.all(&twelve());
        selection.retain_visible(&[1, 4, 7]);
        assert_eq!(selection.count(), 3);
        assert_eq!(selection.marked_in(&twelve()), vec![1, 4, 7]);
    }

    #[test]
    fn targets_follow_the_marking_only_in_the_grid_with_current_marked() {
        let visible = twelve();
        let mut selection = Selection::default();
        selection.click(5);
        selection.extend_to(&visible, 5, 8);

        // Grid, current marked: every marked image in display order.
        assert_eq!(
            selection.targets(&visible, 6, ViewMode::Grid),
            vec![5, 6, 7, 8]
        );
        // Grid, current not marked: the current image.
        assert_eq!(selection.targets(&visible, 2, ViewMode::Grid), vec![2]);
        // Loupe: always the shown image.
        assert_eq!(selection.targets(&visible, 6, ViewMode::Loupe), vec![6]);

        // Grid, one marked: the current image.
        let mut one = Selection::default();
        one.ctrl_click(4, 4);
        assert_eq!(one.count(), 1);
        assert_eq!(one.targets(&visible, 4, ViewMode::Grid), vec![4]);
    }

    #[test]
    fn targets_are_in_display_order() {
        let visible = vec![9, 2, 5];
        let mut selection = Selection::default();
        selection.all(&visible);
        assert_eq!(
            selection.targets(&visible, 5, ViewMode::Grid),
            vec![9, 2, 5]
        );
    }

    #[test]
    fn extend_to_twice_from_the_same_anchor_replaces_the_range() {
        let visible = twelve();
        let mut selection = Selection::default();
        selection.click(5);
        selection.extend_to(&visible, 5, 9);
        assert_eq!(selection.marked_in(&visible), vec![5, 6, 7, 8, 9]);
        selection.extend_to(&visible, 9, 3);
        assert_eq!(selection.marked_in(&visible), vec![3, 4, 5]);
        assert_eq!(selection.anchor(), Some(5));
    }

    #[test]
    fn extend_to_with_nothing_marked_starts_at_current() {
        let visible = twelve();
        let mut selection = Selection::default();
        // An old anchor with nothing marked is replaced by the current image (Shift+arrow).
        selection.click(1);
        selection.extend_to(&visible, 4, 5);
        assert_eq!(selection.marked_in(&visible), vec![4, 5]);
        assert_eq!(selection.anchor(), Some(4));
    }
}
