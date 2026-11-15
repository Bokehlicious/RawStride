//! Browsing a tab under a filter (ADR 0014 §2.2–§2.4, §4.2, Q5, Q7): the visible list of a tab,
//! a rating key that removes the images that stop matching at once (§2.2, amendment 1), and moves
//! in the grid and the loupe that recompute the list before and after the move. Plain Rust, no I/O
//! (GR11); the shown rating of each file comes through a closure.

use std::path::{Path, PathBuf};

use pw_metadata::Rating;

use crate::filter::{matches, nearest_visible, visible};
use crate::grid::{Navigation, navigate};
use crate::selection::Selection;
use crate::tabs::Tab;

/// A step in the preview (the loupe), over the visible list or, with "only the marked", over the
/// marked images (ADR 0017 §2.2, T111).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoupeMove {
    Next,
    Previous,
    First,
    Last,
    /// A click in the filmstrip: the image of file index `0`, if it is visible.
    To(usize),
}

/// The image the metadata window moves to with "Föregående" or "Nästa" (ADR 0017 §2.3.1, U12-Q3,
/// T112), as a file index: the nearest image after (`forward`) or before `current` in `visible`
/// (the shown images in the contact sheet's order) whose kind has metadata support (`supported`
/// of a file index). Unsupported kinds are skipped; there is no wrap. `None` at the first or last
/// supported image, when no other shown image is supported, or when `current` is not shown.
pub fn metadata_neighbour(
    visible: &[usize],
    current: usize,
    forward: bool,
    supported: impl Fn(usize) -> bool,
) -> Option<usize> {
    let position = visible.iter().position(|&index| index == current)?;
    if forward {
        visible
            .iter()
            .skip(position.saturating_add(1))
            .copied()
            .find(|&index| supported(index))
    } else {
        visible
            .iter()
            .take(position)
            .rev()
            .copied()
            .find(|&index| supported(index))
    }
}

impl Tab {
    /// Recomputes `visible`: every file without an active filter; nothing with an active filter
    /// while the ratings are not read yet (Q7); otherwise the files that match, plus `keep` (the
    /// image a move lands on, §4.2). Then unmarks the images the filter hides (§2.8), and if the
    /// current image is hidden, makes the nearest visible one current (§2.3; unchanged when nothing
    /// is visible). Returns whether `visible` changed.
    pub fn refilter(
        &mut self,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        keep: Option<usize>,
    ) -> bool {
        let count = self.files.len();
        let new_visible = if !self.filter.is_active() {
            (0..count).collect()
        } else if !ratings_ready {
            Vec::new()
        } else {
            let files = &self.files;
            let filter = self.filter;
            visible(count, keep, |index| {
                files
                    .get(index)
                    .is_some_and(|path| matches(&filter, rating_of(path).as_ref()))
            })
        };
        let changed = new_visible != self.visible;
        self.visible = new_visible;
        self.selection.retain_visible(&self.visible);
        if let Some(nearest) = nearest_visible(&self.visible, self.current) {
            self.current = nearest;
        }
        changed
    }

    /// After a rating key changed the ratings (§2.2, amendment 1): `refilter` without keeping any
    /// image. Images that no longer match leave `visible` at once; if the current image left, the
    /// nearest following visible image becomes current (the last visible one if none follows;
    /// unchanged when nothing is visible, §2.3); marked images that left are unmarked, the others
    /// stay marked. Returns whether `visible` changed.
    pub fn rated(
        &mut self,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> bool {
        self.refilter(ratings_ready, rating_of, None)
    }

    /// Puts the files in a new order (ADR 0017 U14-Q6, T138): entry `i` of `order` is the old
    /// file index that comes `i`th. The current image stays current (and shown, when it was), the
    /// marked images and the anchor stay marked, and the filter is applied again to the new order
    /// (`refilter`). An `order` that is not a reordering of every file changes nothing. Returns
    /// whether a file changed place.
    pub fn reorder(
        &mut self,
        order: &[usize],
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> bool {
        let count = self.files.len();
        if order.len() != count {
            return false;
        }
        // The new index of each old file index.
        let mut new_of_old = vec![usize::MAX; count];
        for (new, &old) in order.iter().enumerate() {
            match new_of_old.get_mut(old) {
                Some(slot) if *slot == usize::MAX => *slot = new,
                _ => return false,
            }
        }
        if order.iter().enumerate().all(|(new, &old)| new == old) {
            return false;
        }
        let moved = |old: usize| new_of_old.get(old).copied();
        let all: Vec<usize> = (0..count).collect();
        let marked: Vec<usize> = self
            .selection
            .marked_in(&all)
            .into_iter()
            .filter_map(moved)
            .collect();
        let mut selection = Selection::default();
        if let Some(anchor) = self.selection.anchor().and_then(moved) {
            selection.click(anchor);
        }
        selection.all(&marked);
        let was_shown = self.position().is_some();
        let files: Vec<PathBuf> = order
            .iter()
            .filter_map(|&old| self.files.get(old).cloned())
            .collect();
        self.files = files;
        self.selection = selection;
        self.current = moved(self.current).unwrap_or(0);
        self.refilter(ratings_ready, rating_of, was_shown.then_some(self.current));
        true
    }

    /// The position of the current image in `visible`, if it is shown.
    pub fn position(&self) -> Option<usize> {
        self.visible.iter().position(|&index| index == self.current)
    }

    /// The first half of a move, as `go()` in `app.js` (§4.2): `refilter` keeping the **current**
    /// image, so images that stopped matching drop out before the move instead of becoming current.
    /// A rating key already removes them (`rated`); this matters when a rating changes without a
    /// key, for example a rolled-back write. Returns false when nothing is visible: then the move
    /// changes nothing, since keeping the current image would show an image the filter hides.
    fn refilter_before_move(
        &mut self,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> bool {
        if self.visible.is_empty() {
            return false;
        }
        self.refilter(ratings_ready, rating_of, Some(self.current));
        !self.visible.is_empty()
    }

    /// A key move in the grid, as `go()` in `app.js`: first `refilter` keeping the current image
    /// (it drops images whose rating changed without a key, see `refilter_before_move`), then the
    /// new position from `grid::navigate` over that visible list. With `extend` (Shift), the
    /// marking becomes the range from the anchor; otherwise the marking is cleared. Ends with
    /// `refilter` keeping the new current image (§4.2). With nothing visible, nothing changes (as
    /// the keys in `app.js`). Returns the new current file index.
    pub fn grid_move(
        &mut self,
        nav: Navigation,
        extend: bool,
        columns: usize,
        visible_rows: usize,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> usize {
        if !self.refilter_before_move(ratings_ready, rating_of) {
            return self.current;
        }
        let from = self.position().unwrap_or(0);
        let to = navigate(self.visible.len(), columns, from, nav, visible_rows);
        let Some(&index) = self.visible.get(to) else {
            return self.current;
        };
        if extend {
            self.selection.extend_to(&self.visible, self.current, index);
        } else {
            self.selection.click(index);
        }
        self.current = index;
        self.refilter(ratings_ready, rating_of, Some(self.current));
        self.current
    }

    /// A click on the cell of file `index`: Shift marks the range from the anchor, Ctrl toggles the
    /// image, a plain click clears the marking; then `index` becomes current. Ends with `refilter`
    /// keeping the new current image (§4.2). A click on an image that is not visible changes
    /// nothing (S5). Returns the new current file index.
    pub fn click(
        &mut self,
        index: usize,
        ctrl: bool,
        shift: bool,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> usize {
        if !self.visible.contains(&index) {
            return self.current;
        }
        if shift {
            self.selection.extend_to(&self.visible, self.current, index);
        } else if ctrl {
            self.selection.ctrl_click(self.current, index);
        } else {
            self.selection.click(index);
        }
        self.current = index;
        self.refilter(ratings_ready, rating_of, Some(self.current));
        self.current
    }

    /// A step in the preview, as `go()` in `app.js`: `preview_move` without "only the marked".
    pub fn loupe_move(
        &mut self,
        to: LoupeMove,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> usize {
        self.preview_move(to, false, ratings_ready, rating_of)
    }

    /// A step in the preview (arrows, Page Up/Down, Home, End, a click in the filmstrip), as `go()`
    /// in `app.js`: first `refilter` keeping the current image (it drops images whose rating
    /// changed without a key, see `refilter_before_move`), then the step over that visible list.
    /// With `only_marked` and at least two visible images marked, the step goes only between the
    /// marked images (Home and End to the first and last marked); otherwise over every visible
    /// image. A step does not change the marking (ADR 0017 §2.2, owner 2026-11-13; this replaces
    /// ADR 0014 Q5 for the preview). Ends with `refilter` keeping the new current image (§4.2).
    /// With nothing visible, or a click on an image that is not visible, nothing changes. Returns
    /// the new current file index.
    pub fn preview_move(
        &mut self,
        to: LoupeMove,
        only_marked: bool,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> usize {
        if !self.refilter_before_move(ratings_ready, rating_of) {
            return self.current;
        }
        let Some(index) = self.step_target(to, only_marked) else {
            return self.current;
        };
        self.current = index;
        self.refilter(ratings_ready, rating_of, Some(self.current));
        self.current
    }

    /// The images the preview steps through and its filmstrip shows (ADR 0017 §2.2, U3-Q3), as
    /// file indexes in display order: with `only_marked` and at least two visible images marked,
    /// the marked ones; otherwise every visible image.
    pub fn preview_list(&self, only_marked: bool) -> Vec<usize> {
        if only_marked {
            let marked = self.selection.marked_in(&self.visible);
            if marked.len() >= 2 {
                return marked;
            }
        }
        self.visible.clone()
    }

    /// The image a step lands on, as a file index; `None` when nothing is visible or a click hits
    /// an image that is not in the list.
    fn step_target(&self, to: LoupeMove, only_marked: bool) -> Option<usize> {
        let list = self.preview_list(only_marked);
        if let LoupeMove::To(index) = to {
            return list.contains(&index).then_some(index);
        }
        let last = list.len().checked_sub(1)?;
        let position = match list.iter().position(|&index| index == self.current) {
            Some(position) => match to {
                LoupeMove::Next => position.saturating_add(1).min(last),
                LoupeMove::Previous => position.saturating_sub(1),
                LoupeMove::First => 0,
                LoupeMove::Last => last,
                LoupeMove::To(_) => position,
            },
            // The current image is not in the list (it is not marked): the nearest marked image
            // in the direction of the step, by display order; none there, no step (no wrap).
            None => {
                let from = self.position().unwrap_or(0);
                let shown = |index: &usize| self.visible.iter().position(|v| v == index);
                match to {
                    LoupeMove::Next => list
                        .iter()
                        .position(|index| shown(index).is_some_and(|at| at > from))?,
                    LoupeMove::Previous => list
                        .iter()
                        .rposition(|index| shown(index).is_some_and(|at| at < from))?,
                    LoupeMove::First | LoupeMove::To(_) => 0,
                    LoupeMove::Last => last,
                }
            }
        };
        list.get(position).copied()
    }

    /// The image the preview opens on (ADR 0017 §2.2, U3-Q3): when the current image is not in
    /// `preview_list` (with `only_marked`, at least two visible images marked and the current one
    /// not marked), the first marked image in the visible order becomes current; otherwise the
    /// current image stays. Returns the current file
    /// index.
    pub fn preview_start(&mut self, only_marked: bool) -> usize {
        let list = self.preview_list(only_marked);
        if !list.contains(&self.current)
            && let Some(&first) = list.first()
        {
            self.current = first;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::filter::{Filter, LabelChoice, LabelFilter, StarCondition, StarFilter};
    use crate::tabs::{Tabs, ViewMode};
    use crate::{marked_text, no_match_text, position_text};

    fn name(index: usize) -> PathBuf {
        PathBuf::from(format!("IMG_{index:03}.ARW"))
    }

    /// The `filter-select.test.js` setup (ADR 0014 S6): 12 files, and the ratings 1: 3 stars,
    /// 2: 5 stars Red, 3: rejected, 4: Red only.
    fn setup() -> (Tabs, HashMap<PathBuf, Rating>) {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("D:/x"), (0..12).map(name).collect(), 0);
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
        (tabs, ratings)
    }

    fn of(ratings: &HashMap<PathBuf, Rating>) -> impl Fn(&Path) -> Option<Rating> + '_ {
        |path| ratings.get(path).cloned()
    }

    fn filter(stars: StarFilter, label: LabelFilter) -> Filter {
        Filter {
            stars,
            condition: StarCondition::default(),
            label,
        }
    }

    fn tab(tabs: &mut Tabs) -> &mut Tab {
        match tabs.active_mut() {
            Some(tab) => tab,
            None => panic!("there is always an active tab"),
        }
    }

    /// Sets the filter of the active tab and refilters without keep, as `setFilter` in `app.js`.
    fn set(tabs: &mut Tabs, ratings: &HashMap<PathBuf, Rating>, f: Filter) -> Vec<usize> {
        tabs.set_filter(f);
        let t = tab(tabs);
        t.refilter(true, &of(ratings), None);
        t.visible.clone()
    }

    fn status(tab: &Tab) -> String {
        match tab.position() {
            Some(pos) => position_text(pos, tab.visible.len(), tab.files.len()),
            None => no_match_text(tab.files.len()),
        }
    }

    #[test]
    fn the_filter_select_cases_of_app_js() {
        use LabelFilter as L;
        use StarFilter as S;
        let (mut tabs, mut ratings) = setup();
        assert_eq!(
            tab(&mut tabs).visible,
            (0..12).collect::<Vec<_>>(),
            "all visible"
        );

        assert_eq!(
            set(&mut tabs, &ratings, filter(S::Stars(3), L::all())),
            [1, 2]
        );
        assert_eq!(tab(&mut tabs).current, 1);
        assert_eq!(status(tab(&mut tabs)), "1 / 2 (of 12)");
        assert_eq!(
            set(
                &mut tabs,
                &ratings,
                filter(S::Stars(3), L::only(LabelChoice::Label("Red")))
            ),
            [2]
        );
        assert_eq!(
            set(
                &mut tabs,
                &ratings,
                filter(S::All, L::only(LabelChoice::Label("Red")))
            ),
            [2, 4]
        );
        assert!(
            set(
                &mut tabs,
                &ratings,
                filter(S::All, L::only(LabelChoice::Label("Purple")))
            )
            .is_empty()
        );
        assert_eq!(
            status(tab(&mut tabs)),
            "No images match the filter (12 in the folder)"
        );
        assert_eq!(
            tab(&mut tabs).current,
            2,
            "unchanged when nothing is visible"
        );
        assert_eq!(set(&mut tabs, &ratings, filter(S::Rejected, L::all())), [3]);
        assert_eq!(
            set(&mut tabs, &ratings, filter(S::Unrated, L::all())),
            [0, 4, 5, 6, 7, 8, 9, 10, 11]
        );
        assert_eq!(tab(&mut tabs).current, 4, "the nearest following image");

        // Under "unrated", rating the current image (2 stars) removes it at once, and the next one
        // is current (§2.2, amendment 1).
        if let Some(rating) = ratings.get_mut(&name(4)) {
            rating.stars = Some(2);
        }
        assert!(tab(&mut tabs).rated(true, &of(&ratings)));
        let current = tab(&mut tabs).current;
        assert_eq!(current, 5);
        assert_eq!(tab(&mut tabs).visible, [0, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(tab(&mut tabs).files.get(current), Some(&name(5)));

        // Multi-selection with every image shown.
        set(&mut tabs, &ratings, Filter::default());
        let t = tab(&mut tabs);
        t.click(5, false, false, true, &of(&ratings));
        t.click(8, false, true, true, &of(&ratings));
        assert_eq!(t.selection.count(), 4, "range 4 marked");
        assert_eq!(
            marked_text(t.selection.count()).as_deref(),
            Some("4 marked")
        );
        t.click(10, true, false, true, &of(&ratings));
        assert_eq!(t.selection.count(), 5, "ctrl adds");
        assert_eq!(t.current, 10);
        t.selection.clear();
        assert_eq!(t.selection.count(), 0, "escape clears");
        assert_eq!(marked_text(t.selection.count()), None);
        t.grid_move(Navigation::Left, true, 4, 3, true, &of(&ratings));
        t.grid_move(Navigation::Left, true, 4, 3, true, &of(&ratings));
        assert_eq!(t.selection.count(), 3, "shift+arrow extends");
        assert_eq!(t.selection.marked_in(&t.visible), [8, 9, 10]);
        let visible = t.visible.clone();
        t.selection.all(&visible);
        assert_eq!(t.selection.count(), 12, "ctrl+a");
        t.grid_move(Navigation::Left, false, 4, 3, true, &of(&ratings));
        assert_eq!(t.selection.count(), 0, "plain arrow clears");
        assert_eq!(t.current, 7);
    }

    #[test]
    fn a_new_folder_shows_every_image() {
        let (mut tabs, _) = setup();
        let t = tab(&mut tabs);
        assert_eq!(t.visible, (0..12).collect::<Vec<_>>());
        assert_eq!(t.position(), Some(0));
        assert_eq!(status(t), "1 / 12");
        assert_eq!(t.mode, ViewMode::Grid);
    }

    #[test]
    fn an_active_filter_shows_nothing_until_the_ratings_are_read() {
        let (mut tabs, ratings) = setup();
        tabs.set_filter(filter(
            StarFilter::All,
            LabelFilter::only(LabelChoice::Label("Red")),
        ));
        let t = tab(&mut tabs);
        t.current = 4;
        t.selection.ctrl_click(4, 5);
        assert!(t.refilter(false, &of(&ratings), None), "the list changed");
        assert!(t.visible.is_empty());
        assert_eq!(t.position(), None);
        assert_eq!(t.current, 4, "unchanged while nothing is visible");
        assert_eq!(t.selection.count(), 0, "hidden images are unmarked");

        assert!(t.refilter(true, &of(&ratings), None));
        assert_eq!(t.visible, [2, 4]);
        assert_eq!(t.current, 4);

        // Without an active filter, every image is shown even before the ratings are read.
        tabs.set_filter(Filter::default());
        let t = tab(&mut tabs);
        assert!(t.refilter(false, &of(&ratings), None));
        assert_eq!(t.visible.len(), 12);
        assert!(!t.refilter(false, &of(&ratings), None), "nothing changed");
    }

    #[test]
    fn when_the_filter_hides_the_current_image_the_nearest_following_one_is_current() {
        let (mut tabs, ratings) = setup();
        tab(&mut tabs).current = 3;
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Red")),
            ),
        );
        assert_eq!(tab(&mut tabs).current, 4, "the next visible image after 3");

        set(&mut tabs, &ratings, Filter::default());
        tab(&mut tabs).current = 7;
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Red")),
            ),
        );
        assert_eq!(
            tab(&mut tabs).current,
            4,
            "the last visible one when none follows"
        );

        // `keep` keeps a current image that does not match.
        set(&mut tabs, &ratings, Filter::default());
        tabs.set_filter(filter(
            StarFilter::All,
            LabelFilter::only(LabelChoice::Label("Red")),
        ));
        let t = tab(&mut tabs);
        t.current = 6;
        t.refilter(true, &of(&ratings), Some(6));
        assert_eq!(t.visible, [2, 4, 6]);
        assert_eq!(t.current, 6);
    }

    #[test]
    fn a_marked_image_that_the_filter_hides_is_unmarked() {
        let (mut tabs, ratings) = setup();
        let t = tab(&mut tabs);
        t.click(1, false, false, true, &of(&ratings));
        t.click(4, false, true, true, &of(&ratings));
        assert_eq!(t.selection.marked_in(&t.visible), [1, 2, 3, 4]);

        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::NotRejected, LabelFilter::all()),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.selection.count(), 3);
        assert!(!t.selection.is_marked(3));
        assert_eq!(t.selection.marked_in(&t.visible), [1, 2, 4]);
    }

    #[test]
    fn a_click_on_a_hidden_image_changes_nothing() {
        let (mut tabs, ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Red")),
            ),
        );
        let t = tab(&mut tabs);
        t.click(2, true, false, true, &of(&ratings));
        let before = t.clone();
        assert_eq!(t.click(7, false, false, true, &of(&ratings)), 2);
        assert_eq!(t.click(7, true, false, true, &of(&ratings)), 2);
        assert_eq!(t.click(7, false, true, true, &of(&ratings)), 2);
        assert_eq!(*t, before);
    }

    #[test]
    fn grid_moves_follow_positions_in_the_visible_list() {
        let (mut tabs, ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::NotRejected, LabelFilter::all()),
        );
        // Visible: 0, 1, 2, 4, 5, …, 11 (11 images); 4 columns, pages of 2 rows.
        let t = tab(&mut tabs);
        let r = of(&ratings);
        assert_eq!(t.current, 0);
        assert_eq!(t.grid_move(Navigation::Down, false, 4, 2, true, &r), 5);
        assert_eq!(t.grid_move(Navigation::Up, false, 4, 2, true, &r), 0);
        assert_eq!(t.grid_move(Navigation::End, false, 4, 2, true, &r), 11);
        assert_eq!(t.grid_move(Navigation::Right, false, 4, 2, true, &r), 11);
        // A page is 4 × 2 positions: position 10 → 2.
        assert_eq!(t.grid_move(Navigation::PageUp, false, 4, 2, true, &r), 2);
        // Shift+Down from position 2 to position 6 (file 7).
        assert_eq!(t.grid_move(Navigation::Down, true, 4, 2, true, &r), 7);
        assert_eq!(
            t.selection.marked_in(&t.visible),
            [2, 4, 5, 6, 7],
            "the range skips the hidden 3"
        );

        // Nothing visible: a move changes nothing.
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Purple")),
            ),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.current, 7);
        assert_eq!(t.grid_move(Navigation::Right, false, 4, 2, true, &r), 7);
        assert!(t.visible.is_empty());
        assert_eq!(t.selection.count(), 0);
    }

    #[test]
    fn loupe_moves_step_within_the_filter_and_keep_the_marking() {
        let (mut tabs, ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Red")),
            ),
        );
        let t = tab(&mut tabs);
        let r = of(&ratings);
        assert_eq!(t.loupe_move(LoupeMove::First, true, &r), 2);
        assert_eq!(t.loupe_move(LoupeMove::Next, true, &r), 4);
        assert_eq!(t.loupe_move(LoupeMove::Next, true, &r), 4, "no wrap");
        assert_eq!(t.loupe_move(LoupeMove::Previous, true, &r), 2);
        assert_eq!(t.loupe_move(LoupeMove::Previous, true, &r), 2);
        assert_eq!(t.loupe_move(LoupeMove::Last, true, &r), 4);

        t.click(2, true, false, true, &r);
        assert_eq!(t.selection.count(), 2);
        t.loupe_move(LoupeMove::Previous, true, &r);
        assert_eq!(
            t.selection.count(),
            2,
            "a step in the preview keeps the marking (ADR 0017 §2.2, T111)"
        );

        // Nothing visible: a step changes nothing and shows no hidden image.
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Purple")),
            ),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.loupe_move(LoupeMove::Next, true, &r), 2);
        assert!(t.visible.is_empty());
    }

    #[test]
    fn a_loupe_step_drops_the_rated_image_it_leaves() {
        let (mut tabs, mut ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::Unrated, LabelFilter::all()),
        );
        assert_eq!(
            tab(&mut tabs).loupe_move(LoupeMove::First, true, &of(&ratings)),
            0
        );
        // The rating changes without `rated`, as after a rolled-back write: the image stays until
        // the step.
        ratings.insert(
            name(0),
            Rating {
                stars: Some(4),
                label: None,
            },
        );
        assert!(tab(&mut tabs).visible.contains(&0), "kept until the step");
        let t = tab(&mut tabs);
        assert_eq!(t.loupe_move(LoupeMove::Next, true, &of(&ratings)), 4);
        assert!(!t.visible.contains(&0));
        assert_eq!(t.visible, [4, 5, 6, 7, 8, 9, 10, 11]);
    }

    /// Under "Ej betygsatt": current 5, marked 5–7, and all three get stars. The ratings change
    /// without `rated` (as after a rolled-back write), so the list is not recomputed here. Returns
    /// the tab state and the ratings.
    fn batch_rated_under_unrated() -> (Tabs, HashMap<PathBuf, Rating>) {
        let (mut tabs, mut ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::Unrated, LabelFilter::all()),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.visible, [0, 4, 5, 6, 7, 8, 9, 10, 11]);
        t.click(7, false, false, true, &of(&ratings));
        t.click(5, false, true, true, &of(&ratings));
        assert_eq!(t.current, 5);
        assert_eq!(t.selection.marked_in(&t.visible), [5, 6, 7]);
        for index in [5, 6, 7] {
            ratings.insert(
                name(index),
                Rating {
                    stars: Some(3),
                    label: None,
                },
            );
        }
        assert_eq!(
            tab(&mut tabs).visible,
            [0, 4, 5, 6, 7, 8, 9, 10, 11],
            "a rating key does not refilter"
        );
        (tabs, ratings)
    }

    #[test]
    fn a_grid_move_after_a_batch_rating_skips_the_other_rated_images() {
        let (mut tabs, ratings) = batch_rated_under_unrated();
        let t = tab(&mut tabs);
        assert_eq!(
            t.grid_move(Navigation::Right, false, 4, 3, true, &of(&ratings)),
            8
        );
        assert_eq!(t.current, 8);
        assert_eq!(t.visible, [0, 4, 8, 9, 10, 11]);
        assert_eq!(t.selection.count(), 0);
    }

    #[test]
    fn a_loupe_step_after_a_batch_rating_skips_the_other_rated_images() {
        let (mut tabs, ratings) = batch_rated_under_unrated();
        let t = tab(&mut tabs);
        assert_eq!(t.loupe_move(LoupeMove::Next, true, &of(&ratings)), 8);
        assert_eq!(t.current, 8);
        assert_eq!(t.visible, [0, 4, 8, 9, 10, 11]);
        assert_eq!(t.selection.count(), 0);
    }

    fn stars(stars: i8) -> Rating {
        Rating {
            stars: Some(stars),
            label: None,
        }
    }

    #[test]
    fn rated_removes_a_rated_marking_at_once() {
        let (mut tabs, ratings) = batch_rated_under_unrated();
        let t = tab(&mut tabs);
        assert!(t.rated(true, &of(&ratings)));
        assert_eq!(t.visible, [0, 4, 8, 9, 10, 11]);
        assert_eq!(t.current, 8);
        assert_eq!(t.selection.count(), 0, "nothing marked");
    }

    #[test]
    fn rated_unmarks_only_the_images_that_left() {
        let (mut tabs, mut ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::NotRejected, LabelFilter::all()),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.visible, [0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11]);
        t.click(1, false, false, true, &of(&ratings));
        t.click(4, true, false, true, &of(&ratings));
        t.click(2, true, false, true, &of(&ratings));
        assert_eq!(t.current, 2);
        assert_eq!(t.selection.marked_in(&t.visible), [1, 2, 4]);

        ratings.insert(name(2), stars(-1));
        let t = tab(&mut tabs);
        assert!(t.rated(true, &of(&ratings)));
        assert!(!t.visible.contains(&2), "the rejected image is hidden");
        assert_eq!(t.visible, [0, 1, 4, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(t.current, 4, "the nearest following visible image");
        assert_eq!(t.selection.marked_in(&t.visible), [1, 4]);
        assert_eq!(t.selection.count(), 2);
    }

    #[test]
    fn rated_out_of_the_last_place_makes_the_previous_visible_image_current() {
        let (mut tabs, mut ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::Unrated, LabelFilter::all()),
        );
        let t = tab(&mut tabs);
        t.click(11, false, false, true, &of(&ratings));
        assert_eq!(t.current, 11);

        ratings.insert(name(11), stars(1));
        let t = tab(&mut tabs);
        assert!(t.rated(true, &of(&ratings)));
        assert_eq!(t.visible, [0, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(t.current, 10);
    }

    #[test]
    fn rated_with_nothing_left_keeps_the_current_image_and_unmarks_all() {
        let (mut tabs, mut ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Red")),
            ),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.visible, [2, 4]);
        let visible = t.visible.clone();
        t.selection.all(&visible);
        assert_eq!(t.selection.count(), 2);
        let current = t.current;

        for index in [2, 4] {
            if let Some(rating) = ratings.get_mut(&name(index)) {
                rating.label = None;
            }
        }
        let t = tab(&mut tabs);
        assert!(t.rated(true, &of(&ratings)));
        assert!(t.visible.is_empty());
        assert_eq!(t.position(), None);
        assert_eq!(t.current, current, "unchanged when nothing is visible");
        assert_eq!(t.selection.count(), 0);
    }

    #[test]
    fn rated_when_nothing_stopped_matching_changes_nothing() {
        let (mut tabs, mut ratings) = setup();
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::Stars(3), LabelFilter::all()),
        );
        let t = tab(&mut tabs);
        assert_eq!(t.visible, [1, 2]);
        t.click(1, false, false, true, &of(&ratings));
        t.click(2, false, true, true, &of(&ratings));
        assert_eq!(t.current, 2);

        // Image 1 goes from 3 to 4 stars: it still matches "≥ 3".
        ratings.insert(name(1), stars(4));
        let t = tab(&mut tabs);
        let before = t.clone();
        assert!(!t.rated(true, &of(&ratings)));
        assert_eq!(*t, before);
        assert_eq!(t.current, 2);
        assert_eq!(t.selection.marked_in(&t.visible), [1, 2]);

        // Without an active filter a rating never changes the list.
        set(&mut tabs, &ratings, Filter::default());
        ratings.insert(name(0), stars(-1));
        let t = tab(&mut tabs);
        assert!(!t.rated(true, &of(&ratings)));
        assert_eq!(t.visible.len(), 12);
    }

    #[test]
    fn the_star_conditions_on_the_filter_select_setup() {
        use LabelFilter as L;
        use StarCondition as C;
        use StarFilter as S;
        let with = |stars, condition, label| Filter {
            stars,
            condition,
            label,
        };
        let (mut tabs, ratings) = setup();
        assert_eq!(
            set(&mut tabs, &ratings, with(S::Stars(3), C::Exactly, L::all())),
            [1],
            "= 3"
        );
        assert_eq!(
            set(&mut tabs, &ratings, with(S::Stars(5), C::Exactly, L::all())),
            [2],
            "= 5"
        );
        assert_eq!(
            set(&mut tabs, &ratings, with(S::Stars(3), C::AtMost, L::all())),
            [0, 1, 4, 5, 6, 7, 8, 9, 10, 11],
            "≤ 3"
        );
        assert_eq!(
            set(
                &mut tabs,
                &ratings,
                with(S::Stars(3), C::AtMost, L::only(LabelChoice::Label("Red")))
            ),
            [4],
            "≤ 3 and Red"
        );
        assert_eq!(
            set(&mut tabs, &ratings, with(S::Stars(3), C::AtLeast, L::all())),
            [1, 2],
            "≥ 3 (unchanged)"
        );
    }

    #[test]
    fn several_labels_on_the_filter_select_setup() {
        use LabelChoice as C;
        use StarFilter as S;
        let labels = |choices: &[LabelChoice]| {
            let mut set = LabelFilter::all();
            for choice in choices {
                set.toggle(*choice);
            }
            set
        };
        let (mut tabs, ratings) = setup();
        assert_eq!(
            set(
                &mut tabs,
                &ratings,
                filter(S::All, labels(&[C::Label("Red"), C::Label("Yellow")]))
            ),
            [2, 4],
            "Red + Yellow"
        );
        assert_eq!(
            set(&mut tabs, &ratings, filter(S::All, labels(&[C::NoLabel]))),
            [0, 1, 3, 5, 6, 7, 8, 9, 10, 11],
            "Ingen etikett"
        );
        assert_eq!(
            set(
                &mut tabs,
                &ratings,
                filter(S::Stars(3), labels(&[C::Label("Red"), C::NoLabel]))
            ),
            [1, 2],
            "≥ 3 and Red + Ingen etikett"
        );
    }

    /// The `setup` folder of 12 images with 2, 5 and 9 marked (a click, then Ctrl+clicks), current 9.
    fn three_marked(tabs: &mut Tabs, ratings: &HashMap<PathBuf, Rating>) {
        let r = of(ratings);
        let t = tab(tabs);
        t.click(2, false, false, true, &r);
        t.click(5, true, false, true, &r);
        t.click(9, true, false, true, &r);
        assert_eq!(t.selection.marked_in(&t.visible), [2, 5, 9]);
    }

    #[test]
    fn a_step_in_the_preview_keeps_the_marking() {
        let (mut tabs, ratings) = setup();
        three_marked(&mut tabs, &ratings);
        let r = of(&ratings);
        let t = tab(&mut tabs);
        for to in [
            LoupeMove::Next,
            LoupeMove::Previous,
            LoupeMove::Last,
            LoupeMove::First,
            LoupeMove::To(7),
        ] {
            t.preview_move(to, false, true, &r);
            assert_eq!(t.selection.marked_in(&t.visible), [2, 5, 9], "{to:?}");
        }
        assert_eq!(t.current, 7, "a click in the filmstrip shows that image");
        // A click on an image that is not visible changes nothing.
        set(
            &mut tabs,
            &ratings,
            filter(
                StarFilter::All,
                LabelFilter::only(LabelChoice::Label("Red")),
            ),
        );
        let t = tab(&mut tabs);
        let current = t.current;
        assert_eq!(t.preview_move(LoupeMove::To(7), false, true, &r), current);
    }

    #[test]
    fn with_only_the_marked_steps_go_only_between_the_marked() {
        let (mut tabs, ratings) = setup();
        three_marked(&mut tabs, &ratings);
        let r = of(&ratings);
        let t = tab(&mut tabs);
        assert_eq!(t.preview_list(true), [2, 5, 9]);
        assert_eq!(t.preview_move(LoupeMove::First, true, true, &r), 2);
        assert_eq!(t.preview_move(LoupeMove::Next, true, true, &r), 5);
        assert_eq!(t.preview_move(LoupeMove::Next, true, true, &r), 9);
        assert_eq!(
            t.preview_move(LoupeMove::Next, true, true, &r),
            9,
            "no wrap"
        );
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 5);
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 2);
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 2);
        assert_eq!(t.preview_move(LoupeMove::Last, true, true, &r), 9);
        assert_eq!(t.preview_move(LoupeMove::First, true, true, &r), 2);
        // A click on an unmarked image while only the marked are shown changes nothing.
        assert_eq!(t.preview_move(LoupeMove::To(3), true, true, &r), 2);
        assert_eq!(t.selection.marked_in(&t.visible), [2, 5, 9]);
        // From an unmarked current image, the next and previous marked ones by display order.
        t.preview_move(LoupeMove::To(7), false, true, &r);
        assert_eq!(t.preview_move(LoupeMove::Next, true, true, &r), 9);
        t.preview_move(LoupeMove::To(7), false, true, &r);
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 5);
        // No marked image after it: Next stays (no wrap); Previous goes to the last marked.
        t.preview_move(LoupeMove::To(11), false, true, &r);
        assert_eq!(t.preview_move(LoupeMove::Next, true, true, &r), 11);
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 9);
        t.preview_move(LoupeMove::To(0), false, true, &r);
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 0);
    }

    #[test]
    fn with_fewer_than_two_marked_only_the_marked_steps_through_all() {
        let (mut tabs, ratings) = setup();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(4, false, false, true, &r);
        t.click(6, true, false, true, &r);
        t.click(4, true, false, true, &r);
        assert_eq!(t.selection.marked_in(&t.visible), [6]);
        assert_eq!(t.preview_list(true), t.visible);
        assert_eq!(t.preview_move(LoupeMove::First, true, true, &r), 0);
        assert_eq!(t.preview_move(LoupeMove::Next, true, true, &r), 1);
        assert_eq!(t.preview_move(LoupeMove::Next, true, true, &r), 2);
        assert_eq!(t.preview_move(LoupeMove::Last, true, true, &r), 11);
        assert_eq!(t.preview_move(LoupeMove::Previous, true, true, &r), 10);
        assert_eq!(t.preview_move(LoupeMove::To(3), true, true, &r), 3);
    }

    #[test]
    fn the_preview_opens_on_the_first_marked_image_when_the_current_one_is_not_marked() {
        let (mut tabs, ratings) = setup();
        three_marked(&mut tabs, &ratings);
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.preview_move(LoupeMove::To(7), false, true, &r);
        assert_eq!(
            t.preview_start(false),
            7,
            "without only the marked it stays"
        );
        assert_eq!(
            t.preview_start(true),
            2,
            "the first marked in the visible order"
        );
        t.preview_move(LoupeMove::To(5), false, true, &r);
        assert_eq!(t.preview_start(true), 5, "a marked current image stays");
        assert_eq!(t.selection.marked_in(&t.visible), [2, 5, 9]);
    }

    // --- the metadata window's previous / next (ADR 0017 §2.3.1, U12-Q3, T112) ---

    #[test]
    fn metadata_neighbour_steps_forward_and_back_over_the_shown_images() {
        // A filtered visible list in the contact sheet's order; every kind supported.
        let visible = [1, 2, 4, 7];
        let all = |_: usize| true;
        assert_eq!(metadata_neighbour(&visible, 2, true, all), Some(4));
        assert_eq!(metadata_neighbour(&visible, 4, true, all), Some(7));
        assert_eq!(metadata_neighbour(&visible, 4, false, all), Some(2));
        assert_eq!(metadata_neighbour(&visible, 2, false, all), Some(1));
        // A hidden file between them (3) is never visited.
        assert_ne!(metadata_neighbour(&visible, 2, true, all), Some(3));
    }

    #[test]
    fn metadata_neighbour_skips_kinds_without_metadata_support() {
        let visible = [0, 1, 2, 3, 4];
        let supported = |index: usize| index != 1 && index != 3;
        assert_eq!(metadata_neighbour(&visible, 0, true, supported), Some(2));
        assert_eq!(metadata_neighbour(&visible, 2, true, supported), Some(4));
        assert_eq!(metadata_neighbour(&visible, 4, false, supported), Some(2));
        assert_eq!(metadata_neighbour(&visible, 2, false, supported), Some(0));
    }

    #[test]
    fn metadata_neighbour_has_none_at_the_first_and_last_supported_image() {
        // The ends are unsupported kinds: the first and last supported images are 1 and 3.
        let visible = [0, 1, 2, 3, 4];
        let supported = |index: usize| (1..=3).contains(&index);
        assert_eq!(metadata_neighbour(&visible, 1, false, supported), None);
        assert_eq!(metadata_neighbour(&visible, 3, true, supported), None);
        assert_eq!(metadata_neighbour(&visible, 1, true, supported), Some(2));
        assert_eq!(metadata_neighbour(&visible, 3, false, supported), Some(2));
        // No wrap at the ends of the list.
        let all = |_: usize| true;
        assert_eq!(metadata_neighbour(&visible, 0, false, all), None);
        assert_eq!(metadata_neighbour(&visible, 4, true, all), None);
    }

    #[test]
    fn metadata_neighbour_has_none_without_a_supported_image_or_a_shown_current_one() {
        let visible = [0, 1, 2];
        let none = |_: usize| false;
        assert_eq!(metadata_neighbour(&visible, 1, true, none), None);
        assert_eq!(metadata_neighbour(&visible, 1, false, none), None);
        assert_eq!(metadata_neighbour(&[], 0, true, |_| true), None);
        // The current image is not shown (hidden by the filter).
        assert_eq!(metadata_neighbour(&visible, 5, true, |_| true), None);
    }

    #[test]
    fn metadata_neighbour_moves_on_from_an_unsupported_current_image() {
        let visible = [0, 1, 2, 3];
        let supported = |index: usize| index != 1;
        assert_eq!(metadata_neighbour(&visible, 1, true, supported), Some(2));
        assert_eq!(metadata_neighbour(&visible, 1, false, supported), Some(0));
    }

    // --- reordering a tab (ADR 0017 U14-Q6, T138) ---

    #[test]
    fn reordering_keeps_the_current_image_and_the_marking_and_reapplies_the_filter() {
        let (mut tabs, ratings) = setup();
        // Under "3 stars or more" (1 and 2 shown): a click on 1, then Shift+click on 2 marks both;
        // 2 is current and 1 the anchor.
        set(
            &mut tabs,
            &ratings,
            filter(StarFilter::Stars(3), LabelFilter::all()),
        );
        let t = tab(&mut tabs);
        t.click(1, false, false, true, &of(&ratings));
        t.click(2, false, true, true, &of(&ratings));
        assert_eq!(t.selection.marked_in(&t.visible), [1, 2]);

        // The reverse order: old file i becomes 11 − i.
        let order: Vec<usize> = (0..12).rev().collect();
        assert!(t.reorder(&order, true, &of(&ratings)));

        assert_eq!(t.files.first(), Some(&name(11)));
        assert_eq!(t.files.get(t.current), Some(&name(2)), "the same image");
        // The filter applied to the new order: IMG_002 now comes before IMG_001.
        assert_eq!(t.visible, [9, 10]);
        assert_eq!(t.position(), Some(0));
        assert_eq!(t.selection.marked_in(&t.visible), [9, 10]);
        assert_eq!(t.selection.anchor(), Some(10), "the anchor follows IMG_001");
        assert_eq!(status(t), "1 / 2 (of 12)");
    }

    #[test]
    fn reordering_without_a_filter_keeps_every_image_shown_and_the_current_one_in_view() {
        let (mut tabs, ratings) = setup();
        let t = tab(&mut tabs);
        t.click(5, false, false, true, &of(&ratings));
        t.click(7, true, false, true, &of(&ratings));
        // The last file first.
        let mut order: Vec<usize> = vec![11];
        order.extend(0..11);
        assert!(t.reorder(&order, true, &of(&ratings)));
        assert_eq!(t.visible, (0..12).collect::<Vec<_>>());
        assert_eq!(t.files.get(t.current), Some(&name(7)));
        assert_eq!(t.current, 8);
        // Ctrl+click marked the current image 5 first, then 7.
        assert_eq!(t.selection.marked_in(&t.visible), [6, 8]);
    }

    #[test]
    fn a_reorder_that_moves_nothing_or_is_not_a_reordering_changes_nothing() {
        let (mut tabs, ratings) = setup();
        let t = tab(&mut tabs);
        t.click(3, false, false, true, &of(&ratings));
        let before = t.clone();
        assert!(!t.reorder(&(0..12).collect::<Vec<_>>(), true, &of(&ratings)));
        assert!(!t.reorder(&[0, 1, 2], true, &of(&ratings)), "too short");
        let mut twice: Vec<usize> = (0..12).collect();
        twice[11] = 0;
        assert!(!t.reorder(&twice, true, &of(&ratings)), "a file twice");
        let mut outside: Vec<usize> = (0..12).collect();
        outside[11] = 12;
        assert!(
            !t.reorder(&outside, true, &of(&ratings)),
            "an index outside"
        );
        assert_eq!(*t, before);
    }
}
