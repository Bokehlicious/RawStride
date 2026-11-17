//! Comparing two images in the preview (ADR 0017 §2.2, §6.1 U6-Q4–U6-Q11, T151): which images, on
//! which side, hold, swap, the layout, when the compare closes and what the locked side is sent.
//! Plain Rust, no I/O (GR11); nothing is drawn and no key is read here.
//!
//! "The stepped list" is the list the preview steps through (`browse.rs`, `LoupeMove`): the tab's
//! visible images, or with "only the marked" on the marked ones among them, in that order. The
//! functions take it as file indexes and give file indexes, so the caller moves through the tab
//! with `LoupeMove::To(index)`, as a filmstrip click does.

use std::path::{Path, PathBuf};

use pw_viewcore::Zoom;

/// How the two images share the image area (U6-Q7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// The held image left, the current image right.
    #[default]
    SideBySide,
    /// The held image on top, the current image below.
    AboveBelow,
}

/// The image area split in two equal halves, `(held, current)`: the held image left or on top,
/// the current image right or below (U6-Q5). The halves share one edge, so there is no gap and no
/// overlap, and together they are the area. An area of zero or negative size (or one that is not
/// finite) gives two empty rectangles.
pub fn sides(area: egui::Rect, layout: Layout) -> (egui::Rect, egui::Rect) {
    let has_size = area.is_finite() && area.width() > 0.0 && area.height() > 0.0;
    if !has_size {
        let corner = if area.min.x.is_finite() && area.min.y.is_finite() {
            area.min
        } else {
            egui::Pos2::ZERO
        };
        let empty = egui::Rect::from_min_max(corner, corner);
        return (empty, empty);
    }
    match layout {
        Layout::SideBySide => {
            let middle = area.min.x + area.width() / 2.0;
            (
                egui::Rect::from_min_max(area.min, egui::pos2(middle, area.max.y)),
                egui::Rect::from_min_max(egui::pos2(middle, area.min.y), area.max),
            )
        }
        Layout::AboveBelow => {
            let middle = area.min.y + area.height() / 2.0;
            (
                egui::Rect::from_min_max(area.min, egui::pos2(area.max.x, middle)),
                egui::Rect::from_min_max(egui::pos2(area.min.x, middle), area.max),
            )
        }
    }
}

/// Opening the compare (U6-Q4), as `(held, current)` file indexes: the current image is held and
/// the next image of the stepped list becomes current, or the previous one when the current image
/// is the last. `None` when the list has fewer than two images or `current` is not in it. With
/// "only the marked" on, `list` is the marked images, so two marked images give those two.
pub fn open_indexes(list: &[usize], current: usize) -> Option<(usize, usize)> {
    let position = list.iter().position(|&index| index == current)?;
    let other = match list.get(position + 1) {
        Some(&next) => next,
        None => *list.get(position.checked_sub(1)?)?,
    };
    Some((current, other))
}

/// The state while two images are compared: the held image. The compare is open while a `Compare`
/// exists; it is not remembered after a restart (U6-Q11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compare {
    held: PathBuf,
}

impl Compare {
    /// A compare with `held` as the held image.
    pub fn new(held: PathBuf) -> Self {
        Self { held }
    }

    /// The held image's path.
    pub fn held(&self) -> &Path {
        &self.held
    }

    /// Hold (U6-Q6, K): the held image becomes the current image.
    pub fn hold(&mut self, current: &Path) {
        self.held = current.to_path_buf();
    }

    /// Swap (U6-Q6, Shift+K): `list` is the stepped list as file indexes with their paths and
    /// `current` the current image's file index. Gives the file index the current image moves to
    /// (the held image's), and the old current image becomes the held one. `None`, and nothing is
    /// changed, when the held image is not in the stepped list (a filter or "only the marked"
    /// left it out), when it is the current image, or when `current` is not in the list.
    /// `swap_target` gives the same without changing anything.
    pub fn swap(&mut self, list: &[(usize, &Path)], current: usize) -> Option<usize> {
        let (to, new_held) = self.swap_target(list, current)?;
        self.held = new_held;
        Some(to)
    }

    /// What `swap` would do, without changing anything (T165): the file index the current image
    /// moves to (the held image's) and the path that would become held (the current image's).
    /// `None` in the same cases as `swap`. The caller steps first and holds the new path only
    /// when the step happened.
    pub fn swap_target(&self, list: &[(usize, &Path)], current: usize) -> Option<(usize, PathBuf)> {
        let (_, current_path) = list.iter().find(|(index, _)| *index == current)?;
        let (held_index, _) = list.iter().find(|(_, path)| *path == self.held)?;
        if *held_index == current {
            return None;
        }
        Some((*held_index, current_path.to_path_buf()))
    }

    /// Whether the compare stays open after the tab's list of files changed (U6-Q11): `files` is
    /// every file of the tab, not only the shown ones, so a held image that the filter leaves out
    /// stays held; the compare closes when the held image is no longer among them.
    pub fn still_valid<P: AsRef<Path>>(&self, files: &[P]) -> bool {
        files.iter().any(|file| file.as_ref() == self.held)
    }
}

/// The lock (U6-Q8): what to send the held side's view core (`ViewCore::set_view`) this frame.
/// `current` is the current side's zoom and centre (`Frame::zoom`, `Frame::centre`) and `sent`
/// what was sent last. Nothing when unlocked, or when `current` equals what was sent last;
/// otherwise `current`. The values are compared as they are, without rounding, so a drag sends
/// every change.
pub fn follow(
    locked: bool,
    current: (Zoom, [f64; 2]),
    sent: Option<(Zoom, [f64; 2])>,
) -> Option<(Zoom, [f64; 2])> {
    if !locked || sent == Some(current) {
        None
    } else {
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(810.0, 520.0))
    }

    #[test]
    fn the_default_layout_is_side_by_side() {
        assert_eq!(Layout::default(), Layout::SideBySide);
    }

    #[test]
    fn side_by_side_puts_the_held_image_left_and_the_current_image_right() {
        let (held, current) = sides(area(), Layout::SideBySide);
        assert_eq!(
            held,
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(410.0, 520.0))
        );
        assert_eq!(
            current,
            egui::Rect::from_min_max(egui::pos2(410.0, 20.0), egui::pos2(810.0, 520.0))
        );
        // equal halves
        assert_eq!(held.size(), current.size());
        // no gap and no overlap: they share one edge
        assert_eq!(held.max.x, current.min.x);
        // together the area
        assert_eq!(held.union(current), area());
    }

    #[test]
    fn above_and_below_puts_the_held_image_on_top_and_the_current_image_below() {
        let (held, current) = sides(area(), Layout::AboveBelow);
        assert_eq!(
            held,
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(810.0, 270.0))
        );
        assert_eq!(
            current,
            egui::Rect::from_min_max(egui::pos2(10.0, 270.0), egui::pos2(810.0, 520.0))
        );
        assert_eq!(held.size(), current.size());
        assert_eq!(held.max.y, current.min.y);
        assert_eq!(held.union(current), area());
    }

    #[test]
    fn an_odd_sized_area_is_still_two_equal_halves_that_are_the_area() {
        let odd = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(801.0, 455.0));
        for layout in [Layout::SideBySide, Layout::AboveBelow] {
            let (held, current) = sides(odd, layout);
            assert_eq!(held.size(), current.size(), "{layout:?}");
            assert_eq!(held.union(current), odd, "{layout:?}");
            assert_eq!(held.intersect(current).area(), 0.0, "{layout:?}");
        }
    }

    #[test]
    fn an_empty_area_gives_two_empty_rectangles() {
        let corner = egui::pos2(10.0, 20.0);
        let empty = egui::Rect::from_min_max(corner, corner);
        let areas = [
            // zero size
            empty,
            // zero width, zero height
            egui::Rect::from_min_max(corner, egui::pos2(10.0, 520.0)),
            egui::Rect::from_min_max(corner, egui::pos2(810.0, 20.0)),
            // negative width, negative height, both
            egui::Rect::from_min_max(corner, egui::pos2(5.0, 520.0)),
            egui::Rect::from_min_max(corner, egui::pos2(810.0, 0.0)),
            egui::Rect::from_min_max(corner, egui::pos2(0.0, 0.0)),
        ];
        for area in areas {
            for layout in [Layout::SideBySide, Layout::AboveBelow] {
                let (held, current) = sides(area, layout);
                assert_eq!(held, empty, "{area:?} {layout:?}");
                assert_eq!(current, empty, "{area:?} {layout:?}");
                assert_eq!(held.area(), 0.0);
            }
        }
        // an area that is not finite: empty rectangles with finite corners
        for area in [egui::Rect::NOTHING, egui::Rect::EVERYTHING, egui::Rect::NAN] {
            let (held, current) = sides(area, Layout::SideBySide);
            assert_eq!(held, egui::Rect::ZERO, "{area:?}");
            assert_eq!(current, egui::Rect::ZERO, "{area:?}");
        }
    }

    #[test]
    fn opening_holds_the_current_image_and_steps_to_the_next() {
        // in the middle
        assert_eq!(open_indexes(&[0, 1, 2, 3], 1), Some((1, 2)));
        // at the start
        assert_eq!(open_indexes(&[0, 1, 2, 3], 0), Some((0, 1)));
        // the list's order, not the file indexes', decides what is next
        assert_eq!(open_indexes(&[3, 0, 2, 1], 0), Some((0, 2)));
    }

    #[test]
    fn opening_at_the_end_steps_to_the_previous_image() {
        assert_eq!(open_indexes(&[0, 1, 2, 3], 3), Some((3, 2)));
        assert_eq!(open_indexes(&[3, 0, 2, 1], 1), Some((1, 2)));
    }

    #[test]
    fn opening_with_two_images_compares_those_two() {
        assert_eq!(open_indexes(&[4, 5], 4), Some((4, 5)));
        assert_eq!(open_indexes(&[4, 5], 5), Some((5, 4)));
    }

    #[test]
    fn opening_with_one_image_or_none_does_nothing() {
        assert_eq!(open_indexes(&[4], 4), None);
        assert_eq!(open_indexes(&[], 0), None);
    }

    #[test]
    fn opening_does_nothing_when_the_current_image_is_not_in_the_list() {
        assert_eq!(open_indexes(&[0, 1, 2], 5), None);
        assert_eq!(open_indexes(&[2, 7], 3), None);
    }

    #[test]
    fn opening_on_the_marked_images_gives_those_with_their_gaps() {
        assert_eq!(open_indexes(&[2, 7], 2), Some((2, 7)));
        assert_eq!(open_indexes(&[2, 7], 7), Some((7, 2)));
        assert_eq!(open_indexes(&[2, 7, 11], 7), Some((7, 11)));
    }

    fn paths() -> Vec<PathBuf> {
        ["a.ARW", "b.ARW", "c.ARW", "d.ARW"]
            .iter()
            .map(|name| Path::new("D:/RAW").join(name))
            .collect()
    }

    /// The stepped list of the given file indexes of `files`.
    fn stepped<'a>(files: &'a [PathBuf], indexes: &[usize]) -> Vec<(usize, &'a Path)> {
        indexes
            .iter()
            .map(|&index| (index, files[index].as_path()))
            .collect()
    }

    #[test]
    fn hold_makes_the_current_image_the_held_one() {
        let files = paths();
        let mut compare = Compare::new(files[0].clone());
        assert_eq!(compare.held(), files[0].as_path());
        compare.hold(&files[2]);
        assert_eq!(compare.held(), files[2].as_path());
        // holding the held image again changes nothing
        compare.hold(&files[2]);
        assert_eq!(compare, Compare::new(files[2].clone()));
    }

    #[test]
    fn swap_moves_the_current_image_to_the_held_one_and_holds_the_old_current() {
        let files = paths();
        let list = stepped(&files, &[0, 1, 2, 3]);
        let mut compare = Compare::new(files[0].clone());
        // held a (0), current c (2): the current image moves to 0 and c is held
        assert_eq!(compare.swap(&list, 2), Some(0));
        assert_eq!(compare.held(), files[2].as_path());
        // swapped again from the new current image (0): back where it was
        assert_eq!(compare.swap(&list, 0), Some(2));
        assert_eq!(compare.held(), files[0].as_path());
    }

    #[test]
    fn swap_gives_file_indexes_on_a_list_with_gaps() {
        let files = paths();
        // "only the marked": b (1) and d (3)
        let list = stepped(&files, &[1, 3]);
        let mut compare = Compare::new(files[3].clone());
        assert_eq!(compare.swap(&list, 1), Some(3));
        assert_eq!(compare.held(), files[1].as_path());
    }

    #[test]
    fn swap_does_nothing_when_the_held_image_is_filtered_out() {
        let files = paths();
        // the filter leaves a (0) out
        let list = stepped(&files, &[1, 2, 3]);
        let mut compare = Compare::new(files[0].clone());
        assert_eq!(compare.swap(&list, 2), None);
        assert_eq!(compare, Compare::new(files[0].clone()));
    }

    #[test]
    fn swap_does_nothing_when_the_held_image_is_the_current_one() {
        let files = paths();
        let list = stepped(&files, &[0, 1, 2, 3]);
        let mut compare = Compare::new(files[1].clone());
        assert_eq!(compare.swap(&list, 1), None);
        assert_eq!(compare, Compare::new(files[1].clone()));
    }

    #[test]
    fn swap_does_nothing_when_the_current_image_is_not_in_the_list() {
        let files = paths();
        let list = stepped(&files, &[0, 1]);
        let mut compare = Compare::new(files[0].clone());
        assert_eq!(compare.swap(&list, 3), None);
        assert_eq!(compare, Compare::new(files[0].clone()));
    }

    #[test]
    fn swap_target_gives_what_swap_would_without_changing_the_held_image() {
        let files = paths();
        for (members, held, current) in [(&[0, 1, 2, 3][..], 0, 2), (&[1, 3][..], 3, 1)] {
            let list = stepped(&files, members);
            let compare = Compare::new(files[held].clone());
            let target = compare.swap_target(&list, current);
            assert_eq!(target, Some((held, files[current].clone())));
            // nothing changed
            assert_eq!(compare.held(), files[held].as_path());
            // the same index as swap gives, and swap holds the same path
            let mut swapped = compare.clone();
            assert_eq!(swapped.swap(&list, current), Some(held));
            assert_eq!(swapped.held(), files[current].as_path());
        }
    }

    #[test]
    fn swap_target_gives_none_in_the_cases_swap_does_and_changes_nothing() {
        let files = paths();
        // held image filtered out, held image is the current one, current not in the list
        let cases = [
            (&[1, 2, 3][..], 0, 2),
            (&[0, 1, 2, 3][..], 1, 1),
            (&[0, 1][..], 0, 3),
        ];
        for (members, held, current) in cases {
            let list = stepped(&files, members);
            let compare = Compare::new(files[held].clone());
            assert_eq!(compare.swap_target(&list, current), None);
            assert_eq!(compare, Compare::new(files[held].clone()));
        }
    }

    #[test]
    fn the_compare_stays_open_while_the_held_file_is_in_the_tab() {
        let files = paths();
        let compare = Compare::new(files[0].clone());
        // The filter leaves the held image out of the shown list, but `still_valid` is given
        // every file of the tab: it stays held.
        assert!(compare.still_valid(&files));
        // the order of the files does not matter (a new sort order)
        let mut sorted = files.clone();
        sorted.reverse();
        assert!(compare.still_valid(&sorted));
    }

    #[test]
    fn the_compare_closes_when_the_held_file_left_the_tab() {
        let files = paths();
        let compare = Compare::new(files[0].clone());
        assert!(!compare.still_valid(&files[1..]));
        assert!(!compare.still_valid::<PathBuf>(&[]));
        // the same file name in another folder is another file
        assert!(!compare.still_valid(&[Path::new("D:/Other/a.ARW")]));
    }

    const CENTRE: [f64; 2] = [0.5, 0.5];

    #[test]
    fn unlocked_sends_nothing() {
        assert_eq!(follow(false, (Zoom::Fit, CENTRE), None), None);
        assert_eq!(
            follow(
                false,
                (Zoom::Scale(1.0), [0.25, 0.75]),
                Some((Zoom::Fit, CENTRE))
            ),
            None
        );
    }

    #[test]
    fn locked_sends_the_current_view_the_first_time() {
        assert_eq!(
            follow(true, (Zoom::Fit, CENTRE), None),
            Some((Zoom::Fit, CENTRE))
        );
        assert_eq!(
            follow(true, (Zoom::Scale(2.0), [0.25, 0.75]), None),
            Some((Zoom::Scale(2.0), [0.25, 0.75]))
        );
    }

    #[test]
    fn locked_and_unchanged_sends_nothing() {
        let view = (Zoom::Scale(1.0), [0.25, 0.75]);
        assert_eq!(follow(true, view, Some(view)), None);
        assert_eq!(
            follow(true, (Zoom::Fit, CENTRE), Some((Zoom::Fit, CENTRE))),
            None
        );
    }

    #[test]
    fn locked_sends_the_new_centre_after_a_pan() {
        let sent = (Zoom::Scale(1.0), [0.25, 0.75]);
        let panned = (Zoom::Scale(1.0), [0.26, 0.75]);
        assert_eq!(follow(true, panned, Some(sent)), Some(panned));
        // no rounding: the smallest change is sent
        let nudged = (Zoom::Scale(1.0), [0.25, 0.75 + f64::EPSILON]);
        assert_eq!(follow(true, nudged, Some(sent)), Some(nudged));
    }

    #[test]
    fn locked_sends_the_new_zoom_after_a_zoom() {
        let sent = (Zoom::Scale(1.0), [0.25, 0.75]);
        let zoomed = (Zoom::Scale(1.25), [0.25, 0.75]);
        assert_eq!(follow(true, zoomed, Some(sent)), Some(zoomed));
        // from a scale to fit and back
        let fit = (Zoom::Fit, [0.25, 0.75]);
        assert_eq!(follow(true, fit, Some(sent)), Some(fit));
        assert_eq!(follow(true, sent, Some(fit)), Some(sent));
    }
}
