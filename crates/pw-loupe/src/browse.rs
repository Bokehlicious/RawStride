//! Browsing a tab under a filter (ADR 0014 §2.2–§2.4, §4.2, Q5, Q7): the visible list of a tab,
//! a rating key that removes the images that stop matching at once (§2.2, amendment 1), and moves
//! in the grid and the loupe that recompute the list before and after the move. Plain Rust, no I/O
//! (GR11); the shown rating of each file comes through a closure.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pw_metadata::Rating;

use crate::filter::{matches, nearest_visible};
use crate::grid::{Navigation, navigate};
use crate::meta_filter::Outcome;
use crate::selection::Selection;
use crate::sort::{CaptureTimes, Keyed, SortKey, SortOrder, key_known, place};
use crate::tabs::Tab;

/// What `Tab::relist` found (ADR 0026 F1): how many files came and went, and the current file
/// when it was among the gone ones (for F-Q5's text).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relisted {
    pub added: usize,
    pub removed: usize,
    pub current_removed: Option<PathBuf>,
    /// The new files (those counted in `added`), in the tab's form and in the listing's order, so
    /// the shell knows which files to read (T250, T251).
    pub new: Vec<PathBuf>,
    /// The files removed (those counted in `removed`), in the tab's form and its order, so the
    /// shell can forget them in the metadata cache (T250, T251).
    pub gone: Vec<PathBuf>,
}

impl Relisted {
    /// Whether a file came or went.
    pub fn changed(&self) -> bool {
        self.added != 0 || self.removed != 0
    }
}

/// Puts each of `new` among `placed` where `order` puts it (`sort::place`), the values of the
/// placed files taken once for all of them (with `Modified` each value is a disk read). Returns
/// the placed files and the new ones in their new order.
fn place_all(
    placed: &[PathBuf],
    new: Vec<PathBuf>,
    order: SortOrder,
    capture: &CaptureTimes,
    rating_of: &dyn Fn(&Path) -> Option<Rating>,
    modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
) -> Vec<PathBuf> {
    if new.is_empty() {
        return placed.to_vec();
    }
    let mut keyed: Vec<Keyed> = placed
        .iter()
        .map(|path| Keyed::new(path.clone(), order, capture, rating_of, modified_of))
        .collect();
    for path in new {
        let file = Keyed::new(path, order, capture, rating_of, modified_of);
        let at = place(&keyed, &file, order.descending);
        keyed.insert(at, file);
    }
    keyed.into_iter().map(Keyed::into_path).collect()
}

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

/// The image the metadata window moves to with "Previous" or "Next" (ADR 0017 §2.3.1, U12-Q3,
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

/// The list the metadata window on file index `current` steps over with "Previous", "Next" and
/// "Save and Next" (ADR 0017 §2.3.1 as amended 2026-11-21, U15-Q9, T244). `visible` is the shown
/// images and `marked` the marked images, both in the contact sheet's order after the filter
/// (`marked` as `Selection::marked_in(visible)` gives it). When two or more images are marked and
/// `current` is one of them, the window steps only between the marked images: `marked` is
/// returned. Otherwise it steps between all the shown images: `visible` is returned. The rule
/// holds however the window was opened (`M`, the menus, the right-click menu or a thumbnail's
/// button). The count is of every marked image, kinds without metadata support included (as
/// `marked_subjects` counts them); `metadata_neighbour` skips those kinds when it steps.
///
/// This differs on purpose from the preview's "only the marked" (`Tab::preview_list`), which
/// does not need the current image marked: the window steps between the marked images only when
/// its own image is one of them (U15-Q9).
pub fn metadata_steps(visible: &[usize], marked: &[usize], current: usize) -> Vec<usize> {
    if marked.len() >= 2 && marked.contains(&current) {
        marked.to_vec()
    } else {
        visible.to_vec()
    }
}

impl Tab {
    /// Returns the file indexes that pass the star and label filter in display order.
    /// With no star or label filter, every file passes. With one, the list is empty while
    /// ratings are loading, and files in `unread` wait for their own rating. Search and
    /// temporarily kept images do not affect this list.
    pub fn star_label_list(
        &self,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> Vec<usize> {
        if !self.filter.is_active() {
            return (0..self.files.len()).collect();
        }
        if !ratings_ready {
            return Vec::new();
        }
        let unread: HashSet<_> = self.unread.iter().collect();
        self.files
            .iter()
            .enumerate()
            .filter(|(_, path)| {
                !unread.contains(path) && matches(&self.filter, rating_of(path).as_ref())
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Shows every file when neither filter is active. Otherwise shows files that pass the
    /// star and label filter and match the metadata filter, plus the temporary `keep` image.
    /// The next refilter without `keep`, including after a read chunk, drops that exception.
    /// An active star and label filter shows nothing while ratings load, even with `keep` (Q7).
    /// Hidden files lose their marks when they fail either filter (§2.8). Marks are preserved
    /// while ratings load, for individually unread ratings (T250) under an active star and label
    /// filter (§2.8 as changed by amendment 3), and for files that pass that filter but have an
    /// unknown metadata outcome (ADR 0022 S3-Q7). If the current file is hidden, makes the
    /// nearest visible one current when available (§2.3). Returns whether `visible` changed.
    pub fn refilter(
        &mut self,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        keep: Option<usize>,
    ) -> bool {
        let stars_active = self.filter.is_active();
        let metadata_active = self.meta_filter.is_active();
        let candidates = self.star_label_list(ratings_ready, rating_of);
        let new_visible = if stars_active && !ratings_ready {
            candidates
        } else if !stars_active && !metadata_active {
            self.selection.retain_visible(&candidates);
            candidates
        } else {
            let mut candidates = candidates.into_iter().peekable();
            let unread: HashSet<_> = self.unread.iter().collect();
            let mut shown = Vec::new();
            let mut kept_marks = Vec::new();
            for (index, path) in self.files.iter().enumerate() {
                let passes_stars = candidates.next_if_eq(&index).is_some();
                let outcome = if !passes_stars {
                    Outcome::NoMatch
                } else if metadata_active {
                    self.filter_values
                        .values(path)
                        .map_or(Outcome::Unknown, |values| self.meta_filter.outcome(values))
                } else {
                    Outcome::Match
                };
                let visible = outcome == Outcome::Match || keep == Some(index);
                if visible {
                    shown.push(index);
                }
                if visible || outcome == Outcome::Unknown || (stars_active && unread.contains(path))
                {
                    kept_marks.push(index);
                }
            }
            self.selection.retain_visible(&kept_marks);
            shown
        };
        let changed = new_visible != self.visible;
        self.visible = new_visible;
        if let Some(nearest) = nearest_visible(&self.visible, self.current) {
            self.current = nearest;
        }
        changed
    }

    /// After a rating key changed the ratings (§2.2, amendment 1): `refilter` without keeping any
    /// image. Images that no longer match leave `visible` at once; if the current image left, the
    /// nearest following visible image becomes current (the last visible one if none follows;
    /// unchanged when nothing is visible, §2.3); marked images that left are unmarked, the others
    /// stay marked (while an active filter waits for the ratings nothing is shown and every mark
    /// stays, as `refilter` says). Returns whether `visible` changed.
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

    /// Gives the tab a new list of files after a move or an undo (ADR 0017 U9b, T172): `files` are
    /// paths in the tab's own form (the caller converts). The marking and the anchor are kept for
    /// the files still there, compared as given (the `Selection` is rebuilt, as `reorder` does).
    /// The current image becomes `current` when it is in `files`; otherwise the old current image
    /// when it is still there; otherwise the old file that followed it and is still there (the last
    /// file if none followed). Then `refilter`: keeping `current` when it was given and found;
    /// otherwise keeping the current image only when it is the old current image and was shown
    /// before, as `reorder` ends. A current image that replaced a removed one is not kept, so a
    /// following file the filter hides gives way to the nearest visible one. Returns whether the
    /// list of files changed.
    pub fn replace_files(
        &mut self,
        files: Vec<PathBuf>,
        current: Option<&Path>,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> bool {
        // One lookup per file, not a search of the list (review of T172: a large marked folder).
        let positions: HashMap<&PathBuf, usize> = files
            .iter()
            .enumerate()
            .rev()
            .map(|(index, file)| (file, index))
            .collect();
        let index_of = |path: &PathBuf| positions.get(path).copied();
        let new_index = |old: usize| self.files.get(old).and_then(index_of);

        let all: Vec<usize> = (0..self.files.len()).collect();
        let marked: Vec<usize> = self
            .selection
            .marked_in(&all)
            .into_iter()
            .filter_map(new_index)
            .collect();
        let mut selection = Selection::default();
        if let Some(anchor) = self.selection.anchor().and_then(new_index) {
            selection.click(anchor);
        }
        selection.all(&marked);

        let was_shown = self.position().is_some();
        let given = current.and_then(|path| files.iter().position(|file| file == path));
        let kept = new_index(self.current);
        let following = || {
            self.files
                .iter()
                .skip(self.current.saturating_add(1))
                .find_map(index_of)
                .or_else(|| files.len().checked_sub(1))
        };
        let (new_current, keep) = match (given, kept) {
            (Some(index), _) => (index, Some(index)),
            (None, Some(index)) => (index, was_shown.then_some(index)),
            (None, None) => (following().unwrap_or(0), None),
        };

        let changed = files != self.files;
        self.files = files;
        // A file gone leaves `unread` (T250).
        self.keep_unread_listed();
        self.selection = selection;
        self.current = new_current;
        self.refilter(ratings_ready, rating_of, keep);
        changed
    }

    /// Checks that the files of `unplaced` are the last files of `files` (ADR 0026 §2 point 4,
    /// T240): `replace_files` and `reorder` do not know `unplaced`, so it is checked, not
    /// trusted. The waiting files no longer in `files` are dropped; when the rest are not exactly
    /// the last files of `files`, in `unplaced`'s order, `unplaced` is emptied and `sorted` set to
    /// `None`, so the next `follow_sort` applies the whole order (as after an open).
    fn check_unplaced(&mut self) {
        if self.unplaced.is_empty() || self.files.ends_with(&self.unplaced) {
            return;
        }
        let listed: HashSet<&PathBuf> = self.files.iter().collect();
        let still: Vec<PathBuf> = self
            .unplaced
            .iter()
            .filter(|path| listed.contains(path))
            .cloned()
            .collect();
        if self.files.ends_with(&still) {
            self.unplaced = still;
        } else {
            self.unplaced.clear();
            self.sorted = None;
        }
    }

    /// Gives the tab a new listing of its folder (ADR 0026 §2, F1): `listed` is the folder's
    /// listing as `list_folder` gives it, in the tab's own form (the caller converts; a rename
    /// that changes only the case counts as one file gone and one new), compared as given; a path
    /// that comes twice is taken once. A file `pending` names (a write RawStride has queued or in
    /// flight) counts as listed even when `listed` lacks it: the safe write may rename the
    /// original away for a moment. The files still there keep their order; the new ones, in
    /// `listed`'s order, are placed where `order` puts them when the tab is in it (§2 point 4,
    /// F-Q6) and their key is known, otherwise they wait at the end in `unplaced` (a new file's
    /// rating is never known here: it has not been read). When the tab is not in `order`, the new
    /// files go at the end, nothing waits, and `sorted` becomes `None` so that the next
    /// `follow_sort` applies the whole order. The new list then goes through `replace_files` with
    /// `current` (the image an undo brings back; `None` for a reload), so the marks, the anchor,
    /// the filter and the current image follow its rules. With nothing new and nothing gone,
    /// nothing changes.
    #[allow(clippy::too_many_arguments)]
    pub fn relist(
        &mut self,
        listed: Vec<PathBuf>,
        current: Option<&Path>,
        pending: &dyn Fn(&Path) -> bool,
        order: SortOrder,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> Relisted {
        self.check_unplaced();

        let mut seen: HashSet<PathBuf> = HashSet::with_capacity(listed.len());
        let listed: Vec<PathBuf> = listed
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect();
        let still_there = |path: &PathBuf| seen.contains(path) || pending(path);

        let mut kept: Vec<PathBuf> = Vec::with_capacity(self.files.len());
        let mut gone: Vec<PathBuf> = Vec::new();
        let mut current_removed = None;
        for (index, path) in self.files.iter().enumerate() {
            if still_there(path) {
                kept.push(path.clone());
            } else {
                gone.push(path.clone());
                if index == self.current {
                    current_removed = Some(path.clone());
                }
            }
        }
        let removed = gone.len();
        let old: HashSet<&PathBuf> = self.files.iter().collect();
        let new: Vec<PathBuf> = listed
            .into_iter()
            .filter(|path| !old.contains(path))
            .collect();
        if new.is_empty() && removed == 0 {
            return Relisted::default();
        }
        let added = new.len();
        let new_files = new.clone();

        self.unplaced.retain(|path| still_there(path));
        let new_list = if self.sorted == Some(order) {
            // The waiting files are the last of `self.files` (checked above), so after the gone
            // files left they are the last of `kept`.
            let split = kept.len().saturating_sub(self.unplaced.len());
            let placed = kept.get(..split).unwrap_or(&[]);
            let mut waiting: Vec<PathBuf> = kept.get(split..).unwrap_or(&[]).to_vec();
            let mut to_place = Vec::new();
            for path in new {
                if order.key != SortKey::Rating
                    && key_known(&path, order, &self.capture, ratings_ready)
                {
                    to_place.push(path);
                } else {
                    self.unplaced.push(path.clone());
                    waiting.push(path);
                }
            }
            let mut list = place_all(
                placed,
                to_place,
                order,
                &self.capture,
                rating_of,
                modified_of,
            );
            list.append(&mut waiting);
            list
        } else {
            self.unplaced.clear();
            self.sorted = None;
            kept.extend(new);
            kept
        };

        self.replace_files(new_list, current, ratings_ready, rating_of);
        Relisted {
            added,
            removed,
            current_removed,
            new: new_files,
            gone,
        }
    }

    /// Notes that the ratings of `files` (in the tab's form) are being read on their own after a
    /// listing (ADR 0026 §2 points 3 and 4, T250): each that is in `files` and not yet in
    /// `unread` is added. Only the shell calls it (T251), after a listing the folder watcher
    /// started; it then refilters (`refilter`), so under an active filter they are hidden until
    /// their ratings are known.
    pub fn mark_unread(&mut self, files: &[PathBuf]) {
        let listed: HashSet<&PathBuf> = self.files.iter().collect();
        for file in files {
            if listed.contains(file) && !self.unread.contains(file) {
                self.unread.push(file.clone());
            }
        }
    }

    /// Takes out of `unread` every file whose rating `known` says is read (the shell passes
    /// `MetadataCache::rating_known` on the resolved path, T251) and returns whether one was
    /// taken out. It neither refilters nor places: the shell calls `refilter` and `follow_sort`
    /// next (T250).
    pub fn clear_read(&mut self, known: &dyn Fn(&Path) -> bool) -> bool {
        let before = self.unread.len();
        self.unread.retain(|path| !known(path));
        self.unread.len() != before
    }

    /// Keeps `unread` a subset of `files`, in its own order (T250).
    fn keep_unread_listed(&mut self) {
        self.filter_values.retain(&self.files);
        if self.unread.is_empty() {
            return;
        }
        let listed: HashSet<&PathBuf> = self.files.iter().collect();
        self.unread.retain(|path| listed.contains(path));
    }

    /// Places the waiting files whose key is now known (ADR 0026 §2 point 4): when the tab is in
    /// `order`, after the check of `unplaced`, each file of `unplaced` whose key is known
    /// (`sort::key_known`), in `unplaced`'s order, is taken out of the tail and put where `order`
    /// puts it among the placed files; the files still waiting stay at the end. The new order goes
    /// through `reorder` (the current image, the marks, the anchor and the filter kept as it keeps
    /// them). Returns whether a file changed place; a file whose place is the end changes nothing
    /// but still leaves `unplaced`. When the tab is not in `order`, nothing changes.
    pub fn place_waiting(
        &mut self,
        order: SortOrder,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> bool {
        self.place_known(order, ratings_ready, rating_of, modified_of)
            .unwrap_or(false)
    }

    /// `place_waiting`, telling a placement apart: `Some(moved)` when at least one file was
    /// placed, `None` when nothing was (for `follow_sort`'s `SortFollow::Placed`). Under a rating
    /// key a file of `unread` waits also when `ratings_ready` is true (T250).
    pub(crate) fn place_known(
        &mut self,
        order: SortOrder,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> Option<bool> {
        if self.sorted != Some(order) {
            return None;
        }
        self.check_unplaced();
        if self.sorted != Some(order) || self.unplaced.is_empty() {
            return None;
        }
        // Under a rating key a file whose rating is read on its own is not known yet (T250).
        let unread = |path: &PathBuf| order.key == SortKey::Rating && self.unread.contains(path);
        let (known, still): (Vec<PathBuf>, Vec<PathBuf>) =
            self.unplaced.iter().cloned().partition(|path| {
                key_known(path, order, &self.capture, ratings_ready) && !unread(path)
            });
        if known.is_empty() {
            return None;
        }
        let split = self.files.len().saturating_sub(self.unplaced.len());
        let placed = self.files.get(..split).unwrap_or(&[]);
        let mut list = place_all(placed, known, order, &self.capture, rating_of, modified_of);
        list.extend(still.iter().cloned());

        let positions: HashMap<&PathBuf, usize> = self
            .files
            .iter()
            .enumerate()
            .rev()
            .map(|(index, path)| (path, index))
            .collect();
        let new_order: Vec<usize> = list
            .iter()
            .filter_map(|path| positions.get(path).copied())
            .collect();
        let moved = self.reorder(&new_order, ratings_ready, rating_of);
        self.unplaced = still;
        Some(moved)
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

    /// A right-click on the cell of file `index`, before the right-click menu opens (ADR 0019
    /// §2.9, E-Q16): `index` becomes current and the marking and its anchor are kept as they are.
    /// With `Selection::targets` this makes a command from the menu act on every marked image when
    /// the image clicked is marked and more than one is, and on that image alone otherwise. Ends
    /// with `refilter` keeping the new current image, as `click`. A right-click on an image that is
    /// not visible changes nothing (S5). Returns the new current file index.
    pub fn right_click(
        &mut self,
        index: usize,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
    ) -> usize {
        if !self.visible.contains(&index) {
            return self.current;
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
    use crate::sort::CaptureTime;
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

    fn read_title(tab: &mut Tab, index: usize, title: &str) {
        let path = name(index);
        tab.filter_values
            .mark_in_flight(std::slice::from_ref(&path));
        tab.filter_values.record_iptc(
            &path,
            Ok(&pw_metadata::FilterValues {
                title: vec![title.into()],
                ..Default::default()
            }),
        );
    }

    #[test]
    fn search_hides_unread_images_and_preserves_their_marks() {
        let (mut tabs, ratings) = setup();
        let tab = tab(&mut tabs);
        tab.selection.all(&tab.visible);
        tab.meta_filter.set_search("needle");
        assert!(tab.refilter(true, &of(&ratings), None));
        assert!(tab.visible.is_empty());
        assert_eq!(tab.selection.count(), tab.files.len());
    }

    #[test]
    fn search_shows_matches_in_order_and_unmarks_nonmatches() {
        let (mut tabs, ratings) = setup();
        let tab = tab(&mut tabs);
        tab.selection.all(&tab.visible);
        tab.meta_filter.set_search("needle");
        read_title(tab, 3, "needle");
        read_title(tab, 1, "needle");
        read_title(tab, 2, "different");
        assert!(tab.refilter(true, &of(&ratings), None));
        assert_eq!(tab.visible, [1, 3]);
        assert!(tab.selection.is_marked(1));
        assert!(!tab.selection.is_marked(2));
        assert!(tab.selection.is_marked(3));
        assert!(tab.selection.is_marked(4));
    }

    #[test]
    fn search_and_stars_must_both_match() {
        let (mut tabs, ratings) = setup();
        let tab = tab(&mut tabs);
        tab.selection.all(&tab.visible);
        tab.filter = filter(StarFilter::Stars(3), LabelFilter::all());
        tab.meta_filter.set_search("needle");
        read_title(tab, 1, "needle");
        read_title(tab, 4, "needle");
        assert!(tab.refilter(true, &of(&ratings), None));
        assert_eq!(tab.visible, [1]);
        assert!(tab.selection.is_marked(2));
        assert!(!tab.selection.is_marked(4));
        assert!(!tab.selection.is_marked(5));
    }

    #[test]
    fn star_label_candidates_ignore_search_and_wait_for_ratings() {
        let (mut tabs, ratings) = setup();
        let tab = tab(&mut tabs);
        tab.meta_filter.set_search("needle");
        tab.unread.push(name(2));
        let rating_of = of(&ratings);
        assert_eq!(
            tab.star_label_list(false, &rating_of),
            (0..12).collect::<Vec<_>>()
        );
        tab.filter = filter(StarFilter::Stars(3), LabelFilter::all());
        assert!(tab.star_label_list(false, &rating_of).is_empty());
        assert_eq!(tab.star_label_list(true, &rating_of), [1]);
        tab.unread.clear();
        assert_eq!(tab.star_label_list(true, &rating_of), [1, 2]);
    }

    #[test]
    fn search_moves_current_to_the_nearest_match() {
        let (mut tabs, ratings) = setup();
        let tab = tab(&mut tabs);
        tab.current = 2;
        tab.meta_filter.set_search("needle");
        read_title(tab, 1, "needle");
        read_title(tab, 2, "different");
        read_title(tab, 3, "needle");
        assert!(tab.refilter(true, &of(&ratings), None));
        assert_eq!(tab.current, 3);
    }

    #[test]
    fn clearing_search_shows_every_file_again() {
        let (mut tabs, ratings) = setup();
        let tab = tab(&mut tabs);
        tab.meta_filter.set_search("needle");
        assert!(tab.refilter(true, &of(&ratings), None));
        assert!(tab.visible.is_empty());
        tab.meta_filter.set_search("");
        assert!(tab.refilter(true, &of(&ratings), None));
        assert_eq!(tab.visible, (0..12).collect::<Vec<_>>());
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
        assert_eq!(
            t.selection.count(),
            2,
            "marks are kept while the ratings are read"
        );

        assert!(t.refilter(true, &of(&ratings), None));
        assert_eq!(t.visible, [2, 4]);
        assert_eq!(t.current, 4);
        assert!(t.selection.is_marked(4));
        assert!(!t.selection.is_marked(5));
        assert_eq!(t.selection.count(), 1);

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

    /// ADR 0019 §2.9, E-Q16 (T221): a right-click on an image that is not visible changes nothing,
    /// as a left click (S5).
    #[test]
    fn a_right_click_on_a_hidden_image_changes_nothing() {
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
        assert_eq!(t.right_click(7, true, &of(&ratings)), 2);
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

    /// ADR 0019 §2.9, E-Q16 (T221): a right-click on an image that is not marked makes it current
    /// and keeps the marking and its anchor; the menu's commands then act on that image alone.
    #[test]
    fn a_right_click_makes_the_image_current_and_keeps_the_marking() {
        let (mut tabs, ratings) = setup();
        three_marked(&mut tabs, &ratings);
        let r = of(&ratings);
        let t = tab(&mut tabs);
        let anchor = t.selection.anchor();
        assert_eq!(anchor, Some(9));
        assert_eq!(t.right_click(7, true, &r), 7);
        assert_eq!(t.current, 7);
        assert_eq!(t.selection.marked_in(&t.visible), [2, 5, 9]);
        assert_eq!(t.selection.anchor(), anchor);
        assert_eq!(
            t.selection.targets(&t.visible, t.current, ViewMode::Grid),
            [7]
        );
    }

    /// ADR 0019 §2.9, E-Q16 (T221): a right-click on a marked image, with more than one marked,
    /// makes it current, and the menu's commands act on every marked image in display order.
    #[test]
    fn a_right_click_on_a_marked_image_targets_every_marked_image() {
        let (mut tabs, ratings) = setup();
        three_marked(&mut tabs, &ratings);
        let r = of(&ratings);
        let t = tab(&mut tabs);
        assert_eq!(t.preview_move(LoupeMove::To(7), false, true, &r), 7);
        assert_eq!(t.selection.marked_in(&t.visible), [2, 5, 9]);
        assert_eq!(t.right_click(5, true, &r), 5);
        assert_eq!(t.current, 5);
        assert_eq!(
            t.selection.targets(&t.visible, t.current, ViewMode::Grid),
            [2, 5, 9]
        );
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

    #[test]
    fn metadata_steps_are_the_marked_when_the_image_is_marked_and_two_or_more_are() {
        let visible = [0, 1, 2, 3, 4, 5];
        let marked = [1, 3, 5];
        assert_eq!(metadata_steps(&visible, &marked, 3), [1, 3, 5]);
    }

    #[test]
    fn metadata_steps_are_the_shown_images_when_the_image_is_not_marked() {
        let visible = [0, 1, 2, 3, 4, 5];
        let marked = [1, 3, 5];
        assert_eq!(metadata_steps(&visible, &marked, 2), visible);
    }

    #[test]
    fn metadata_steps_are_the_shown_images_with_only_one_marked() {
        let visible = [0, 1, 2, 3, 4, 5];
        assert_eq!(metadata_steps(&visible, &[3], 3), visible);
        assert_eq!(metadata_steps(&visible, &[], 3), visible);
    }

    #[test]
    fn metadata_steps_with_metadata_neighbour_step_only_between_the_marked() {
        let visible = [0, 1, 2, 3, 4, 5];
        let marked = [1, 3, 5];
        let all = |_: usize| true;
        let from_3 = metadata_steps(&visible, &marked, 3);
        assert_eq!(metadata_neighbour(&from_3, 3, true, all), Some(5));
        assert_eq!(metadata_neighbour(&from_3, 3, false, all), Some(1));
        let from_5 = metadata_steps(&visible, &marked, 5);
        assert_eq!(metadata_neighbour(&from_5, 5, true, all), None);
        // No wrap at the first marked image.
        let from_1 = metadata_steps(&visible, &marked, 1);
        assert_eq!(metadata_neighbour(&from_1, 1, false, all), None);
        // File 3 is a kind without metadata support: it is skipped, to the next marked image.
        let not_3 = |index: usize| index != 3;
        assert_eq!(metadata_neighbour(&from_1, 1, true, not_3), Some(5));
    }

    #[test]
    fn metadata_steps_keep_the_contact_sheet_order() {
        // A sort order that is not the file order; the marked in that order.
        let visible = [4, 0, 2];
        let marked = [4, 2];
        let steps = metadata_steps(&visible, &marked, 2);
        assert_eq!(steps, [4, 2]);
        assert_eq!(metadata_neighbour(&steps, 2, false, |_| true), Some(4));
    }

    #[test]
    fn metadata_steps_count_marked_kinds_without_support() {
        let visible = [0, 1, 2, 3, 4];
        let marked = [1, 3];
        let not_3 = |index: usize| index != 3;
        let steps = metadata_steps(&visible, &marked, 1);
        assert_eq!(steps, [1, 3]);
        // The window does not fall back to the images not marked.
        assert_eq!(metadata_neighbour(&steps, 1, true, not_3), None);
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

    /// The files of `t` without those named by `gone`.
    fn without(t: &Tab, gone: &[usize]) -> Vec<PathBuf> {
        t.files
            .iter()
            .filter(|path| !gone.iter().any(|&index| **path == name(index)))
            .cloned()
            .collect()
    }

    /// ADR 0017 U9b, T172: images moved away leave the list; the current image, when it left,
    /// gives way to the one that followed it, and the marks stay on the same files.
    #[test]
    fn replace_files_with_files_removed_keeps_the_marks_and_takes_the_following_image() {
        let (mut tabs, ratings) = setup();
        let t = tab(&mut tabs);
        t.click(3, false, false, true, &of(&ratings));
        t.click(5, true, false, true, &of(&ratings));
        t.click(8, true, false, true, &of(&ratings));
        assert_eq!(t.selection.marked_in(&t.visible), [3, 5, 8]);
        assert_eq!(t.current, 8);

        // 4 and 8 (the current image) were moved.
        let files = without(t, &[4, 8]);
        assert!(t.replace_files(files, None, true, &of(&ratings)));
        assert_eq!(t.files.len(), 10);
        assert_eq!(t.visible, (0..10).collect::<Vec<_>>());
        assert_eq!(
            t.files.get(t.current),
            Some(&name(9)),
            "the following image"
        );
        let marked: Vec<PathBuf> = t
            .selection
            .marked_in(&t.visible)
            .into_iter()
            .filter_map(|index| t.files.get(index).cloned())
            .collect();
        assert_eq!(marked, [name(3), name(5)]);
        // Ctrl+click on 8 made it the anchor; it is gone, so there is none.
        assert_eq!(t.selection.anchor(), None);

        // The last image removed: the last one left becomes current.
        let last = t.files.len() - 1;
        t.click(last, false, false, true, &of(&ratings));
        assert_eq!(t.files.get(t.current), Some(&name(11)));
        let files = without(t, &[11]);
        assert!(t.replace_files(files, None, true, &of(&ratings)));
        assert_eq!(t.files.get(t.current), Some(&name(10)));

        // The current image still there stays current; the anchor follows its file.
        t.click(2, false, false, true, &of(&ratings));
        let files = without(t, &[0, 1]);
        assert!(t.replace_files(files, None, true, &of(&ratings)));
        assert_eq!(t.files.get(t.current), Some(&name(2)));
        assert_eq!(t.current, 0);
        assert_eq!(t.selection.anchor(), Some(0));
        assert_eq!(t.position(), Some(0));

        // The same list changes nothing and says so.
        let same = t.files.clone();
        assert!(!t.replace_files(same, None, true, &of(&ratings)));
        assert_eq!(t.files.get(t.current), Some(&name(2)));
    }

    /// An undo puts files back: `current` (the image moved back) becomes current and is shown.
    #[test]
    fn replace_files_with_files_added_back_makes_the_given_image_current() {
        let (mut tabs, ratings) = setup();
        let t = tab(&mut tabs);
        let all = t.files.clone();
        t.click(5, false, false, true, &of(&ratings));
        t.click(6, true, false, true, &of(&ratings));
        assert!(t.replace_files(without(t, &[7, 9]), None, true, &of(&ratings)));
        assert_eq!(t.files.get(t.current), Some(&name(6)));

        let back = name(9);
        assert!(t.replace_files(all, Some(&back), true, &of(&ratings)));
        assert_eq!(t.files.len(), 12);
        assert_eq!(t.current, 9);
        assert_eq!(t.position(), Some(9));
        assert_eq!(t.selection.marked_in(&t.visible), [5, 6]);

        // A given image that is not in the list: the old current image stays.
        let missing = PathBuf::from("IMG_099.ARW");
        let files = t.files.clone();
        assert!(!t.replace_files(files, Some(&missing), true, &of(&ratings)));
        assert_eq!(t.current, 9);
    }

    /// Under a filter: an image given as `current` is kept even when the filter hides it (as the
    /// image a move lands on, §4.2); a list where nothing matches shows nothing.
    #[test]
    fn replace_files_under_a_filter() {
        let (mut tabs, mut ratings) = setup();
        let red = filter(
            StarFilter::All,
            LabelFilter::only(LabelChoice::Label("Red")),
        );
        assert_eq!(set(&mut tabs, &ratings, red), [2, 4]);
        let all = tab(&mut tabs).files.clone();
        let t = tab(&mut tabs);
        t.click(4, false, false, true, &of(&ratings));

        // 2 and 4 were moved away: nothing matches; the following file is current but not shown.
        let files = without(t, &[2, 4]);
        assert!(t.replace_files(files, None, true, &of(&ratings)));
        assert!(t.visible.is_empty());
        assert_eq!(t.files.get(t.current), Some(&name(5)));
        assert_eq!(t.position(), None);
        assert_eq!(status(t), no_match_text(10));

        // 4 comes back without its label (the undo restored another rating): kept as given.
        ratings.remove(&name(4));
        let back = name(4);
        let files: Vec<PathBuf> = all.into_iter().filter(|path| *path != name(2)).collect();
        assert!(t.replace_files(files, Some(&back), true, &of(&ratings)));
        assert_eq!(t.files.get(t.current), Some(&back));
        assert_eq!(t.visible, [3]);
        assert_eq!(t.position(), Some(0));

        // A removed current image is not kept: a hidden following file gives way to the nearest
        // visible one.
        ratings.insert(
            name(4),
            Rating {
                stars: None,
                label: Some("Red".to_owned()),
            },
        );
        let t = tab(&mut tabs);
        t.refilter(true, &of(&ratings), None);
        assert_eq!(t.visible, [3]);
        ratings.insert(
            name(9),
            Rating {
                stars: None,
                label: Some("Red".to_owned()),
            },
        );
        t.refilter(true, &of(&ratings), None);
        assert_eq!(t.visible, [3, 8]);
        let files = without(t, &[4]);
        assert!(t.replace_files(files, None, true, &of(&ratings)));
        assert_eq!(t.visible, [7]);
        assert_eq!(t.files.get(t.current), Some(&name(9)));
    }

    // --- listing a tab again and placing new files (ADR 0026 §2, F1, T240) ---

    fn by(key: SortKey, descending: bool) -> SortOrder {
        SortOrder { key, descending }
    }

    fn no_time(_: &Path) -> Option<SystemTime> {
        None
    }

    fn never(_: &Path) -> bool {
        false
    }

    /// The marked files of `t`, by path, in file order.
    fn marked_files(t: &Tab) -> Vec<PathBuf> {
        (0..t.files.len())
            .filter(|&index| t.selection.is_marked(index))
            .filter_map(|index| t.files.get(index).cloned())
            .collect()
    }

    fn current_file(t: &Tab) -> Option<PathBuf> {
        t.files.get(t.current).cloned()
    }

    /// The 12 files of `setup` and `extra`, in file-name order as `list_folder` gives them.
    fn listing(t: &Tab, extra: &[&str], gone: &[usize]) -> Vec<PathBuf> {
        let mut files = without(t, gone);
        files.extend(extra.iter().map(PathBuf::from));
        files.sort_by_key(|path| path.to_string_lossy().to_lowercase());
        files
    }

    fn time(stored: &str) -> Option<CaptureTime> {
        CaptureTime::parse(stored)
    }

    /// The `setup` folder sorted by name ascending (the order it is in already).
    fn sorted_by_name() -> (Tabs, HashMap<PathBuf, Rating>) {
        let (mut tabs, ratings) = setup();
        let order = by(SortKey::Name, false);
        tabs.follow_sort(order, true, &of(&ratings), &no_time);
        assert_eq!(tab(&mut tabs).sorted, Some(order));
        (tabs, ratings)
    }

    /// The `setup` folder sorted by capture time, every file read (14:00:00 + its index, so the
    /// order is the file-name order).
    fn sorted_by_capture() -> (Tabs, HashMap<PathBuf, Rating>) {
        let (mut tabs, ratings) = setup();
        for index in 0..12 {
            tabs.record_capture(
                &name(index),
                time(&format!("2026:10:30 14:00:{index:02}")),
                None,
            );
        }
        let order = by(SortKey::Capture, false);
        tabs.follow_sort(order, true, &of(&ratings), &no_time);
        assert_eq!(tab(&mut tabs).sorted, Some(order));
        (tabs, ratings)
    }

    #[test]
    fn relist_places_a_new_file_by_name_and_keeps_the_current_image_and_the_marks() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(3, false, false, true, &r);
        t.click(8, true, false, true, &r);
        let listed = listing(t, &["IMG_005b.ARW"], &[]);
        let relisted = t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(
            relisted,
            Relisted {
                added: 1,
                removed: 0,
                current_removed: None,
                new: vec![PathBuf::from("IMG_005b.ARW")],
                gone: Vec::new(),
            }
        );
        assert!(relisted.changed());
        assert_eq!(t.files.len(), 13);
        assert_eq!(t.files.get(6), Some(&PathBuf::from("IMG_005b.ARW")));
        assert_eq!(current_file(t), Some(name(8)));
        assert_eq!(marked_files(t), [name(3), name(8)]);
        assert!(t.unplaced.is_empty());
        assert_eq!(t.visible.len(), 13);
    }

    #[test]
    fn relist_with_the_current_and_a_marked_file_gone_takes_the_following_image() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(2, false, false, true, &r);
        t.click(4, true, false, true, &r);
        t.click(6, true, false, true, &r);
        let listed = listing(t, &[], &[4, 6]);
        let relisted = t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(
            relisted,
            Relisted {
                added: 0,
                removed: 2,
                current_removed: Some(name(6)),
                new: Vec::new(),
                gone: vec![name(4), name(6)],
            }
        );
        assert_eq!(current_file(t), Some(name(7)));
        assert_eq!(marked_files(t), [name(2)]);
    }

    #[test]
    fn relist_under_capture_waits_for_the_time_and_places_the_file_when_it_is_read() {
        let (mut tabs, ratings) = sorted_by_capture();
        let r = of(&ratings);
        let order = by(SortKey::Capture, false);
        let first = PathBuf::from("AAA.ARW");
        let second = PathBuf::from("ZZZ.ARW");
        let t = tab(&mut tabs);
        t.click(5, false, false, true, &r);

        // The time of AAA.ARW is not read: it waits at the end.
        let listed = listing(t, &["AAA.ARW"], &[]);
        t.relist(listed, None, &never, order, true, &r, &no_time);
        assert_eq!(t.files.last(), Some(&first));
        assert_eq!(t.unplaced, std::slice::from_ref(&first));
        assert_eq!(current_file(t), Some(name(5)));

        // ZZZ.ARW comes while AAA.ARW waits, with its time read (that of IMG_005): it is placed
        // before the waiting tail, after IMG_005 by name.
        t.capture
            .insert(second.clone(), time("2026:10:30 14:00:05"));
        let listed = listing(t, &["ZZZ.ARW"], &[]);
        let relisted = t.relist(listed, None, &never, order, true, &r, &no_time);
        assert_eq!(relisted.added, 1);
        assert_eq!(t.files.get(6), Some(&second));
        assert_eq!(t.files.last(), Some(&first));
        assert_eq!(t.unplaced, std::slice::from_ref(&first));

        // The time of AAA.ARW comes back (before every other): it is placed first.
        tabs.record_capture(&first, time("2026:10:30 13:00:00"), None);
        let t = tab(&mut tabs);
        assert!(t.place_waiting(order, true, &r, &no_time));
        assert_eq!(t.files.first(), Some(&first));
        assert!(t.unplaced.is_empty());
        assert_eq!(current_file(t), Some(name(5)));
        assert_eq!(t.files.len(), 14);
    }

    #[test]
    fn relist_under_rating_waits_even_with_the_ratings_ready() {
        let (mut tabs, mut ratings) = setup();
        let order = by(SortKey::Rating, true);
        tabs.follow_sort(order, true, &of(&ratings), &no_time);
        let new = PathBuf::from("NEW.ARW");
        ratings.insert(new.clone(), stars(4));
        let r = of(&ratings);
        let t = tab(&mut tabs);
        assert_eq!(t.files.first(), Some(&name(2)), "5 stars first");

        let listed = listing(t, &["NEW.ARW"], &[]);
        t.relist(listed, None, &never, order, true, &r, &no_time);
        assert_eq!(t.files.last(), Some(&new));
        assert_eq!(t.unplaced, std::slice::from_ref(&new));

        // Its rating read: after the 5 stars of IMG_002.
        assert!(t.place_waiting(order, true, &r, &no_time));
        assert_eq!(t.files.get(1), Some(&new));
        assert!(t.unplaced.is_empty());
    }

    #[test]
    fn relist_with_no_order_applied_puts_the_new_files_at_the_end_in_name_order() {
        let (mut tabs, ratings) = setup();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        assert_eq!(t.sorted, None);
        let listed = listing(t, &["B.ARW", "A.ARW"], &[]);
        let relisted = t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(relisted.added, 2);
        assert_eq!(t.files.len(), 14);
        assert_eq!(t.files.get(12), Some(&PathBuf::from("A.ARW")));
        assert_eq!(t.files.get(13), Some(&PathBuf::from("B.ARW")));
        assert_eq!(t.sorted, None);
        assert!(t.unplaced.is_empty());
    }

    #[test]
    fn relist_with_the_same_listing_changes_nothing() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(4, false, false, true, &r);
        let before = t.clone();
        let listed = listing(t, &[], &[]);
        let relisted = t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(relisted, Relisted::default());
        assert!(!relisted.changed());
        assert_eq!(*t, before);
    }

    #[test]
    fn relist_with_a_given_current_file_makes_it_current() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let back = PathBuf::from("IMG_007b.ARW");
        let t = tab(&mut tabs);
        t.click(2, false, false, true, &r);
        let listed = listing(t, &["IMG_007b.ARW"], &[]);
        t.relist(
            listed,
            Some(&back),
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(current_file(t), Some(back));
        assert_eq!(t.current, 8);
        assert!(t.position().is_some());
    }

    #[test]
    fn relist_keeps_a_file_being_written_that_the_listing_lacks() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(4, false, false, true, &r);
        t.click(6, true, false, true, &r);
        let writing = name(4);
        let pending = |path: &Path| path == writing.as_path();
        let listed = listing(t, &["IMG_010b.ARW"], &[4]);
        let relisted = t.relist(
            listed,
            None,
            &pending,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(relisted.added, 1);
        assert_eq!(relisted.removed, 0);
        assert_eq!(t.files.get(4), Some(&name(4)));
        assert_eq!(marked_files(t), [name(4), name(6)]);
    }

    #[test]
    fn relist_under_another_order_forgets_the_order_and_empties_unplaced() {
        let (mut tabs, ratings) = sorted_by_capture();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        let listed = listing(t, &["AAA.ARW"], &[]);
        t.relist(
            listed,
            None,
            &never,
            by(SortKey::Capture, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(t.unplaced.len(), 1);

        // The user chose another order before the next listing.
        let listed = listing(t, &["BBB.ARW"], &[]);
        t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(t.sorted, None);
        assert!(t.unplaced.is_empty());
        assert_eq!(t.files.last(), Some(&PathBuf::from("BBB.ARW")));
    }

    #[test]
    fn forget_sort_empties_unplaced() {
        let (mut tabs, ratings) = sorted_by_capture();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        let listed = listing(t, &["AAA.ARW"], &[]);
        t.relist(
            listed,
            None,
            &never,
            by(SortKey::Capture, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(t.unplaced.len(), 1);
        tabs.forget_sort();
        assert!(tab(&mut tabs).unplaced.is_empty());
        assert_eq!(tab(&mut tabs).sorted, None);
    }

    #[test]
    fn relist_with_every_file_gone_and_relist_of_an_empty_tab() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(3, false, false, true, &r);
        let relisted = t.relist(
            Vec::new(),
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(
            relisted,
            Relisted {
                added: 0,
                removed: 12,
                current_removed: Some(name(3)),
                new: Vec::new(),
                gone: (0..12).map(name).collect(),
            }
        );
        assert!(t.files.is_empty());
        assert!(t.visible.is_empty());
        assert_eq!(t.position(), None);
        assert_eq!(t.selection.count(), 0);

        let mut empty = Tab::default();
        let listed: Vec<PathBuf> = (0..3).map(name).collect();
        let relisted = empty.relist(
            listed.clone(),
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(relisted.added, 3);
        assert_eq!(relisted.current_removed, None);
        assert_eq!(empty.files, listed);
        assert_eq!(empty.visible, [0, 1, 2]);
        // `replace_files`' rule: no old current image and none that followed it, so the last file.
        assert_eq!(empty.position(), Some(2));
    }

    #[test]
    fn a_path_listed_twice_is_taken_once() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        let mut listed = listing(t, &["IMG_012.ARW"], &[]);
        listed.push(PathBuf::from("IMG_012.ARW"));
        let relisted = t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(relisted.added, 1);
        assert_eq!(t.files.len(), 13);
    }

    #[test]
    fn a_waiting_file_that_is_gone_leaves_unplaced() {
        let (mut tabs, ratings) = sorted_by_capture();
        let r = of(&ratings);
        let order = by(SortKey::Capture, false);
        let t = tab(&mut tabs);
        let listed = listing(t, &["AAA.ARW", "BBB.ARW"], &[]);
        t.relist(listed, None, &never, order, true, &r, &no_time);
        assert_eq!(
            t.unplaced,
            [PathBuf::from("AAA.ARW"), PathBuf::from("BBB.ARW")]
        );

        let listed: Vec<PathBuf> = t
            .files
            .iter()
            .filter(|path| path.as_path() != Path::new("AAA.ARW"))
            .cloned()
            .collect();
        let relisted = t.relist(listed, None, &never, order, true, &r, &no_time);
        assert_eq!(relisted.removed, 1);
        assert_eq!(t.unplaced, [PathBuf::from("BBB.ARW")]);
        assert_eq!(t.files.last(), Some(&PathBuf::from("BBB.ARW")));
        assert_eq!(t.sorted, Some(order));
    }

    #[test]
    fn a_broken_tail_makes_the_tab_forget_its_order() {
        let (mut tabs, ratings) = sorted_by_capture();
        let r = of(&ratings);
        let order = by(SortKey::Capture, false);
        let t = tab(&mut tabs);
        let listed = listing(t, &["AAA.ARW"], &[]);
        t.relist(listed, None, &never, order, true, &r, &no_time);
        assert_eq!(t.unplaced.len(), 1);

        // A list where the waiting file comes before a placed one.
        let mut files = t.files.clone();
        files.rotate_right(1);
        assert_eq!(files.first(), Some(&PathBuf::from("AAA.ARW")));
        t.replace_files(files, None, true, &r);
        assert!(!t.place_waiting(order, true, &r, &no_time));
        assert!(t.unplaced.is_empty());
        assert_eq!(t.sorted, None);
    }

    #[test]
    fn place_waiting_under_another_order_changes_nothing() {
        let (mut tabs, ratings) = sorted_by_capture();
        let r = of(&ratings);
        let order = by(SortKey::Capture, false);
        let t = tab(&mut tabs);
        let listed = listing(t, &["AAA.ARW"], &[]);
        t.relist(listed, None, &never, order, true, &r, &no_time);
        t.capture
            .insert(PathBuf::from("AAA.ARW"), time("2026:10:30 13:00:00"));
        let before = t.clone();
        assert!(!t.place_waiting(by(SortKey::Capture, true), true, &r, &no_time));
        assert!(!t.place_waiting(by(SortKey::Name, false), true, &r, &no_time));
        assert_eq!(*t, before);
    }

    /// ADR 0014 §2.8 as changed by amendment 3 (§16, the owner 2026-11-21): while an active
    /// filter waits for the ratings, a new listing hides every image and keeps every mark; once
    /// the ratings are read, the marked files that match stay marked and the others are unmarked.
    #[test]
    fn relist_under_a_filter_waiting_for_the_ratings_keeps_the_marks() {
        let (mut tabs, ratings) = setup();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(1, false, false, true, &r);
        t.click(2, true, false, true, &r);
        t.click(4, true, false, true, &r);
        tabs.set_filter(filter(
            StarFilter::All,
            LabelFilter::only(LabelChoice::Label("Red")),
        ));
        let t = tab(&mut tabs);
        t.refilter(false, &r, None);
        let listed = listing(t, &["IMG_020.ARW"], &[]);
        let relisted = t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            false,
            &r,
            &no_time,
        );
        assert_eq!(relisted.added, 1);
        assert!(t.visible.is_empty());
        assert_eq!(marked_files(t), [name(1), name(2), name(4)]);

        t.refilter(true, &r, None);
        assert_eq!(t.visible, [2, 4]);
        assert_eq!(marked_files(t), [name(2), name(4)]);
        assert_eq!(t.selection.count(), 2);
    }

    // --- files whose rating is read on their own after a listing (ADR 0026 F3, T250) ---

    /// T250: `relist` gives the new files in the listing's order and the removed ones in the
    /// tab's order; a file being written that the listing lacks is not gone; `unread` stays empty
    /// (only the shell fills it).
    #[test]
    fn relist_gives_the_new_and_the_gone_files_and_leaves_unread_empty() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        let listed = listing(t, &["IMG_005b.ARW", "AAA.ARW"], &[4, 5, 9]);
        let writing = |path: &Path| path == name(5);
        let relisted = t.relist(
            listed,
            None,
            &writing,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(relisted.added, 2);
        assert_eq!(relisted.removed, 2);
        assert_eq!(
            relisted.new,
            [PathBuf::from("AAA.ARW"), PathBuf::from("IMG_005b.ARW")]
        );
        assert_eq!(relisted.gone, [name(4), name(9)]);
        assert!(t.files.contains(&name(5)));
        assert!(t.unread.is_empty());
    }

    /// T250: `mark_unread` adds only files of the tab, each once; `clear_read` takes out the
    /// files said to be read and returns whether one was taken out.
    #[test]
    fn mark_unread_adds_only_files_of_the_tab_and_clear_read_reports_a_removal() {
        let (mut tabs, _) = setup();
        let t = tab(&mut tabs);
        t.mark_unread(&[name(3), PathBuf::from("OTHER.ARW"), name(5)]);
        t.mark_unread(&[name(5)]);
        assert_eq!(t.unread, [name(3), name(5)]);

        assert!(!t.clear_read(&|_| false));
        assert_eq!(t.unread, [name(3), name(5)]);
        assert!(t.clear_read(&|path| path == name(3)));
        assert_eq!(t.unread, [name(5)]);
        assert!(t.clear_read(&|_| true));
        assert!(t.unread.is_empty());
        assert!(!t.clear_read(&|_| true));
    }

    /// T250: a file gone from the listing leaves `unread`; a reordering keeps the same files.
    #[test]
    fn a_file_gone_leaves_unread() {
        let (mut tabs, ratings) = sorted_by_name();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.mark_unread(&[name(3), name(5)]);
        let listed = listing(t, &[], &[3]);
        t.relist(
            listed,
            None,
            &never,
            by(SortKey::Name, false),
            true,
            &r,
            &no_time,
        );
        assert_eq!(t.unread, [name(5)]);

        let order: Vec<usize> = (0..t.files.len()).rev().collect();
        assert!(t.reorder(&order, true, &r));
        assert_eq!(t.unread, [name(5)]);
    }

    /// T250: under an active filter an unread file is hidden while the files already read stay
    /// shown; the marks of unread files stay (amendment 3) and a read file the filter hides is
    /// unmarked; a `keep` naming an unread file shows it; once read it is filtered as any file.
    #[test]
    fn under_a_filter_an_unread_file_is_hidden_and_keeps_its_mark() {
        let (mut tabs, ratings) = setup();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.click(1, false, false, true, &r);
        t.click(2, true, false, true, &r);
        t.click(4, true, false, true, &r);
        t.mark_unread(&[name(4)]);
        tabs.set_filter(filter(
            StarFilter::All,
            LabelFilter::only(LabelChoice::Label("Red")),
        ));
        let t = tab(&mut tabs);

        t.refilter(true, &r, None);
        assert_eq!(t.visible, [2], "IMG_004 (Red) waits for its rating");
        assert_eq!(marked_files(t), [name(2), name(4)]);

        t.refilter(true, &r, Some(4));
        assert_eq!(t.visible, [2, 4], "the current image is kept");

        assert!(t.clear_read(&|path| path == name(4)));
        t.refilter(true, &r, None);
        assert_eq!(t.visible, [2, 4]);
        assert_eq!(marked_files(t), [name(2), name(4)]);
    }

    /// T250: without a filter an unread file is shown at once.
    #[test]
    fn without_a_filter_an_unread_file_is_shown() {
        let (mut tabs, ratings) = setup();
        let r = of(&ratings);
        let t = tab(&mut tabs);
        t.mark_unread(&[name(4)]);
        t.refilter(true, &r, None);
        assert_eq!(t.visible, (0..12).collect::<Vec<_>>());
    }

    /// T250: under a rating order an unread file waits in `unplaced` also with the ratings
    /// ready, and is placed once `clear_read` took it out.
    #[test]
    fn under_a_rating_order_an_unread_file_waits_until_it_is_read() {
        let (mut tabs, mut ratings) = setup();
        let order = by(SortKey::Rating, true);
        tabs.follow_sort(order, true, &of(&ratings), &no_time);
        let new = PathBuf::from("NEW.ARW");
        ratings.insert(new.clone(), stars(4));
        let r = of(&ratings);
        let t = tab(&mut tabs);

        let listed = listing(t, &["NEW.ARW"], &[]);
        let relisted = t.relist(listed, None, &never, order, true, &r, &no_time);
        t.mark_unread(&relisted.new);
        assert_eq!(t.unread, std::slice::from_ref(&new));

        assert!(!t.place_waiting(order, true, &r, &no_time));
        assert_eq!(t.files.last(), Some(&new));
        assert_eq!(t.unplaced, std::slice::from_ref(&new));

        assert!(t.clear_read(&|path| path == new));
        assert!(t.place_waiting(order, true, &r, &no_time));
        assert_eq!(t.files.get(1), Some(&new), "after the 5 stars of IMG_002");
        assert!(t.unplaced.is_empty());
    }
}
