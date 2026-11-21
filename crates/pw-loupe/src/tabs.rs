use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use pw_metadata::Rating;

use crate::filter::Filter;
use crate::selection::Selection;
use crate::shell_text::{ShellText, shell_text};
use crate::sort::{CaptureTime, CaptureTimes, SortOrder, SortPlan, plan};

/// The per-tab view shown for a folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Grid,
    Loupe,
}

/// One open folder and the image it should return to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tab {
    pub folder: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub current: usize,
    pub mode: ViewMode,
    /// The filter of this tab (ADR 0014 Q1: per tab, reset when the folder is opened again).
    pub filter: Filter,
    /// The marked images of this tab (ADR 0014 §2.5); cleared when the folder is opened again.
    pub selection: Selection,
    /// The file indexes shown, in display order (ADR 0014 §4.2). Set only by `refilter`, `rated`
    /// (a rating key removes the images that stop matching at once, §2.2) and the moves of
    /// `browse.rs`, never by drawing.
    pub visible: Vec<usize>,
    /// The sort order `files` are in (ADR 0017 U14, T138); `None` until one has been applied
    /// since the folder was opened. The order is applied only when it differs from the chosen one
    /// (U14-Q5), so a rating, a write or a new read never moves an image.
    pub sorted: Option<SortOrder>,
    /// The capture times read for this tab (U14-Q4), kept while it is open.
    pub capture: CaptureTimes,
    /// Files listed since the order was applied that wait for their key's value (a capture time
    /// or a rating) before they are placed (ADR 0026 §2 point 4, T240). They are always the last
    /// files of `files`, in the order they came (`relist` and `place_waiting` keep it so, and
    /// check it first, since `replace_files` and `reorder` do not know this list).
    pub unplaced: Vec<PathBuf>,
}

impl Tab {
    fn open(&mut self, folder: PathBuf, files: Vec<PathBuf>, pos: usize) {
        self.filter = Filter::default();
        self.selection = Selection::default();
        // The folder in file-name order from `list_folder`: the sort is applied again (U14-Q5).
        self.sorted = None;
        self.capture.clear();
        self.unplaced.clear();
        self.folder = Some(folder);
        self.files = files;
        self.current = self
            .files
            .len()
            .checked_sub(1)
            .map_or(0, |last| pos.min(last));
        self.mode = ViewMode::Grid;
        // Without a filter every file is shown; no rating is needed for that.
        self.refilter(true, &|_| None, None);
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    /// Applies `order` once when the files are not in it yet (U14-Q4, U14-Q5): when its keys are
    /// all read, the files are reordered (`reorder`, keeping the current image, the marking and
    /// the filter) and the order is remembered; until then the tab keeps its order. When the
    /// files are in the order already, the waiting files whose key is now known are placed
    /// (`place_waiting`, ADR 0026 §2 point 4). Returns what happened.
    pub fn follow_sort(
        &mut self,
        order: SortOrder,
        ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> SortFollow {
        if self.sorted == Some(order) {
            if let Some(moved) = self.place_known(order, ratings_ready, rating_of, modified_of) {
                return SortFollow::Placed { moved };
            }
            // A broken tail of waiting files makes the tab forget its order (`place_known`):
            // the whole order is applied below.
            if self.sorted == Some(order) {
                return SortFollow::Sorted;
            }
        }
        match plan(
            &self.files,
            order,
            &self.capture,
            ratings_ready,
            rating_of,
            modified_of,
        ) {
            SortPlan::Order(new_order) => {
                let moved = self.reorder(&new_order, ratings_ready, rating_of);
                self.sorted = Some(order);
                // The whole order now holds for every file.
                self.unplaced.clear();
                SortFollow::Reordered { moved }
            }
            SortPlan::ReadingCaptureTimes { read, total } => {
                SortFollow::ReadingCaptureTimes { read, total }
            }
            SortPlan::WaitingForRatings => SortFollow::WaitingForRatings,
        }
    }
}

/// What `Tab::follow_sort` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortFollow {
    /// The files were already in the order.
    Sorted,
    /// The order was applied now; `moved` when a file changed place.
    Reordered { moved: bool },
    /// The files were in the order and waiting files were placed in it (ADR 0026 §2 point 4):
    /// the contact sheet keeps its top image; `moved` when a file changed place.
    Placed { moved: bool },
    /// Waiting for the capture times: `read` of `total` have come back.
    ReadingCaptureTimes { read: usize, total: usize },
    /// Waiting for the ratings.
    WaitingForRatings,
}

/// A collection of folder tabs sharing one view core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tabs {
    tabs: Vec<Tab>,
    active: usize,
}

impl Default for Tabs {
    fn default() -> Self {
        Self {
            tabs: vec![Tab::default()],
            active: 0,
        }
    }
}

impl Tabs {
    /// Opens a folder in the active tab.
    pub fn open_in_active(&mut self, folder: PathBuf, files: Vec<PathBuf>, pos: usize) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.open(folder, files, pos);
        }
    }

    /// Opens a folder in a new tab. If the folder is already open, that tab is activated and
    /// refreshed instead (GR12); if the active tab is empty, the folder fills it instead of leaving
    /// an empty tab behind.
    pub fn open_new(&mut self, folder: PathBuf, files: Vec<PathBuf>, pos: usize) {
        if let Some(index) = self.tabs.iter().position(|tab| {
            tab.folder
                .as_deref()
                .is_some_and(|open| same_folder(open, &folder))
        }) {
            self.active = index;
            if let Some(tab) = self.tabs.get_mut(index) {
                tab.open(folder, files, pos);
            }
            return;
        }
        if self
            .tabs
            .get(self.active)
            .is_some_and(|tab| tab.folder.is_none())
        {
            self.open_in_active(folder, files, pos);
            return;
        }

        self.tabs.push(Tab::default());
        self.active = self.tabs.len().saturating_sub(1);
        self.open_in_active(folder, files, pos);
    }

    /// Activates tab `index`; returns false when the index is out of range.
    pub fn activate(&mut self, index: usize) -> bool {
        if let Some(_tab) = self.tabs.get(index) {
            self.active = index;
            true
        } else {
            false
        }
    }

    /// Activates the next tab, wrapping at the end.
    pub fn next(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Activates the previous tab, wrapping at the beginning.
    pub fn previous(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    /// Closes the active tab, leaving one empty tab when it was the last one.
    pub fn close_active(&mut self) {
        if self.tabs.len() == 1 {
            if let Some(tab) = self.tabs.get_mut(0) {
                tab.clear();
            }
            self.active = 0;
            return;
        }

        if self.active < self.tabs.len() {
            self.tabs.remove(self.active);
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
    }

    /// Clears tabs rooted at `root` or below it without touching the filesystem.
    pub fn clear_inside(&mut self, root: &Path) -> bool {
        let mut active_cleared = false;
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if tab
                .folder
                .as_deref()
                .is_some_and(|folder| path_starts_with(folder, root))
            {
                if index == self.active {
                    active_cleared = true;
                }
                tab.clear();
            }
        }
        active_cleared
    }

    /// Remembers the current image of the active tab.
    pub fn set_current(&mut self, pos: usize) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.current = tab
                .files
                .len()
                .checked_sub(1)
                .map_or(0, |last| pos.min(last));
        }
    }

    pub fn set_mode(&mut self, mode: ViewMode) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.mode = mode;
        }
    }

    /// Sets the filter of the active tab; other tabs keep their own filter (ADR 0014 Q1).
    pub fn set_filter(&mut self, filter: Filter) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.filter = filter;
        }
    }

    /// Applies `order` to every tab whose keys are read (U14-Q3): the active tab with
    /// `active_ratings_ready`, the others without ratings (theirs are read when they are
    /// activated). Returns what happened to the active tab.
    pub fn follow_sort(
        &mut self,
        order: SortOrder,
        active_ratings_ready: bool,
        rating_of: &dyn Fn(&Path) -> Option<Rating>,
        modified_of: &dyn Fn(&Path) -> Option<SystemTime>,
    ) -> SortFollow {
        let mut active = SortFollow::Sorted;
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            let ready = index == self.active && active_ratings_ready;
            let follow = tab.follow_sort(order, ready, rating_of, modified_of);
            if index == self.active {
                active = follow;
            }
        }
        active
    }

    /// Forgets the order every tab is in, so the next `follow_sort` applies the chosen one again:
    /// choosing the sort the tabs are in already (U14-Q5). The capture times are kept; no file
    /// waits any more, since the whole order is applied next.
    pub fn forget_sort(&mut self) {
        for tab in &mut self.tabs {
            tab.sorted = None;
            tab.unplaced.clear();
        }
    }

    /// Records a capture time that has come back in every tab listing `path` (U14-Q4).
    pub fn record_capture(&mut self, path: &Path, time: Option<CaptureTime>) {
        for tab in &mut self.tabs {
            if tab.files.iter().any(|file| file == path) {
                tab.capture.insert(path.to_path_buf(), time.clone());
            }
        }
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// The tabs whose folder is `folder`, compared by `same_folder`: after a move or an undo every
    /// tab of the folder gets its new list (ADR 0017 U9b, T172).
    pub fn tabs_of_folder_mut(&mut self, folder: &Path) -> impl Iterator<Item = &mut Tab> {
        self.tabs.iter_mut().filter(move |tab| {
            tab.folder
                .as_deref()
                .is_some_and(|open| same_folder(open, folder))
        })
    }

    pub fn titles(&self) -> Vec<String> {
        self.tabs
            .iter()
            .map(|tab| {
                tab.folder
                    .as_deref()
                    .and_then(|folder| folder.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| {
                        tab.folder
                            .as_deref()
                            .map(|folder| folder.display().to_string())
                    })
                    .unwrap_or_else(|| shell_text(ShellText::EmptyTab).to_string())
            })
            .collect()
    }
}

/// Whether `a` and `b` name the same folder, however they are written (GR12): each is made
/// absolute and then resolved with `pw_ingest::resolve`, so case, trailing separators, `\\?\`
/// and `subst` drives do not matter. A path whose absolute or resolved form cannot be made is
/// compared as it is. Only the comparison uses the resolved forms; the tab keeps the path given.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    resolved_folder(a) == resolved_folder(b)
}

fn resolved_folder(path: &Path) -> PathBuf {
    std::path::absolute(path)
        .ok()
        .and_then(|absolute| pw_ingest::resolve(&absolute, "folder").ok())
        .unwrap_or_else(|| path.to_path_buf())
}

fn path_starts_with(path: &Path, root: &Path) -> bool {
    let path_components = normalized_components(path);
    let root_components = normalized_components(root);
    path_components.len() >= root_components.len()
        && path_components.iter().zip(root_components.iter()).all(
            |(path_component, root_component)| components_equal(*path_component, *root_component),
        )
}

fn normalized_components(path: &Path) -> Vec<Component<'_>> {
    let mut normalized = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(normalized.last(), Some(Component::Normal(_))) {
                    normalized.pop();
                } else if !normalized
                    .iter()
                    .any(|part| matches!(part, Component::RootDir))
                {
                    normalized.push(component);
                }
            }
            _ => normalized.push(component),
        }
    }
    normalized
}

fn components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;
    use crate::shell_text::sorting_text;
    use crate::sort::SortKey;

    fn files(prefix: &str, count: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|index| PathBuf::from(format!("{prefix}-{index}.arw")))
            .collect()
    }

    #[test]
    fn starts_with_one_empty_tab_and_opens_in_active() {
        let mut tabs = Tabs::default();
        assert_eq!(tabs.tabs(), &[Tab::default()]);

        tabs.open_in_active(PathBuf::from("first"), files("first", 3), 2);
        let tab = tabs.active().expect("active tab exists");
        assert_eq!(tab.folder, Some(PathBuf::from("first")));
        assert_eq!(tab.current, 2);
        assert_eq!(tabs.tabs().len(), 1);
    }

    #[test]
    fn opens_new_tabs_and_reuses_an_already_open_folder() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), files("first", 2), 0);
        tabs.open_new(PathBuf::from("second"), files("second", 3), 1);
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.tabs().len(), 2);

        tabs.open_new(PathBuf::from("first"), files("updated", 4), 3);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(
            tabs.active().and_then(|tab| tab.files.first()),
            Some(&PathBuf::from("updated-0.arw"))
        );
        assert_eq!(tabs.active().map(|tab| tab.current), Some(3));
    }

    #[test]
    fn a_new_tab_fills_an_empty_active_tab_first() {
        let mut tabs = Tabs::default();
        tabs.open_new(PathBuf::from("first"), files("first", 2), 0);
        assert_eq!(tabs.tabs().len(), 1, "the empty start tab is filled");
        tabs.open_new(PathBuf::from("second"), files("second", 2), 0);
        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active_index(), 1);
        tabs.close_active();
        tabs.close_active();
        assert_eq!(tabs.titles(), vec!["Empty"]);
        tabs.open_new(PathBuf::from("third"), files("third", 1), 0);
        assert_eq!(tabs.titles(), vec!["third"]);
    }

    #[test]
    fn next_and_previous_wrap_and_activation_checks_the_index() {
        let mut tabs = Tabs::default();
        // Fill the start tab first: `open_new` would otherwise reuse it (it is empty).
        tabs.open_in_active(PathBuf::from("first"), files("first", 1), 0);
        tabs.open_new(PathBuf::from("second"), files("second", 1), 0);
        tabs.open_new(PathBuf::from("third"), files("third", 1), 0);
        assert!(tabs.activate(0));
        tabs.previous();
        assert_eq!(tabs.active_index(), 2);
        tabs.next();
        assert_eq!(tabs.active_index(), 0);
        assert!(!tabs.activate(3));
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn closing_last_tab_leaves_it_empty() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), files("first", 2), 1);
        tabs.close_active();
        assert_eq!(tabs.tabs(), &[Tab::default()]);
        assert_eq!(tabs.titles(), vec!["Empty"]);
    }

    #[test]
    fn closing_a_middle_tab_activates_the_next_tab_at_its_index() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), files("first", 1), 0);
        tabs.open_new(PathBuf::from("middle"), files("middle", 1), 0);
        tabs.open_new(PathBuf::from("last"), files("last", 1), 0);
        assert!(tabs.activate(1));
        tabs.close_active();
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(
            tabs.active().and_then(|tab| tab.folder.as_deref()),
            Some(std::path::Path::new("last"))
        );
        assert_eq!(tabs.tabs().len(), 2);
    }

    #[test]
    fn each_tab_remembers_its_current_image_and_titles() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), files("first", 3), 0);
        tabs.set_current(2);
        tabs.set_mode(ViewMode::Loupe);
        tabs.open_new(PathBuf::from("second"), files("second", 4), 1);
        tabs.set_current(3);
        assert_eq!(tabs.active().map(|tab| tab.mode), Some(ViewMode::Grid));
        assert_eq!(tabs.titles(), vec!["first", "second"]);
        tabs.previous();
        assert_eq!(tabs.active().map(|tab| tab.current), Some(2));
        assert_eq!(tabs.active().map(|tab| tab.mode), Some(ViewMode::Loupe));
        tabs.next();
        assert_eq!(tabs.active().map(|tab| tab.current), Some(3));
        assert_eq!(tabs.active().map(|tab| tab.mode), Some(ViewMode::Grid));

        tabs.close_active();
        assert_eq!(tabs.titles(), vec!["first"]);
        tabs.close_active();
        assert_eq!(tabs.titles(), vec!["Empty"]);
    }

    #[test]
    fn clear_inside_clears_root_and_subfolders_but_not_similar_names() {
        let root = PathBuf::from(r"E:\DCIM");
        let mut tabs = Tabs::default();
        tabs.open_in_active(root.clone(), files("root", 1), 0);
        tabs.open_new(root.join("100TEST"), files("nested", 1), 0);
        tabs.open_new(PathBuf::from(r"E:\DCIM2"), files("similar", 1), 0);
        assert!(tabs.activate(1));

        assert!(tabs.clear_inside(&root));

        assert_eq!(tabs.tabs()[0], Tab::default());
        assert_eq!(tabs.tabs()[1], Tab::default());
        assert_eq!(tabs.tabs()[2].folder, Some(PathBuf::from(r"E:\DCIM2")));
        assert_eq!(tabs.active_index(), 1);
    }

    #[test]
    fn clear_inside_uses_lexical_parent_components_without_file_access() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(
            PathBuf::from("card").join("..").join("other"),
            files("other", 1),
            0,
        );

        assert!(!tabs.clear_inside(Path::new("card")));
        let expected = PathBuf::from("card").join("..").join("other");
        assert_eq!(
            tabs.active().and_then(|tab| tab.folder.as_deref()),
            Some(expected.as_path())
        );
    }

    fn unrated() -> Filter {
        Filter {
            stars: crate::filter::StarFilter::Unrated,
            condition: crate::filter::StarCondition::AtLeast,
            label: crate::filter::LabelFilter::all(),
        }
    }

    fn red() -> Filter {
        Filter {
            stars: crate::filter::StarFilter::All,
            condition: crate::filter::StarCondition::AtLeast,
            label: crate::filter::LabelFilter::only(crate::filter::LabelChoice::Label("Red")),
        }
    }

    #[test]
    fn a_new_tab_and_tab_open_start_without_a_filter() {
        let mut tabs = Tabs::default();
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(Filter::default()));

        tabs.open_in_active(PathBuf::from("first"), files("first", 2), 0);
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(Filter::default()));

        tabs.set_filter(unrated());
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(unrated()));
        // Opening a folder again in the same tab (a refresh) resets the filter (Q1).
        tabs.open_in_active(PathBuf::from("first"), files("first", 2), 0);
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(Filter::default()));

        tabs.set_filter(unrated());
        // `open_new` on a folder that is already open refreshes that tab.
        tabs.open_new(PathBuf::from("first"), files("first", 3), 0);
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(Filter::default()));

        tabs.open_new(PathBuf::from("second"), files("second", 1), 0);
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(Filter::default()));
    }

    #[test]
    fn set_filter_changes_only_the_active_tab() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), files("first", 1), 0);
        tabs.open_new(PathBuf::from("second"), files("second", 1), 0);
        tabs.open_new(PathBuf::from("third"), files("third", 1), 0);
        assert!(tabs.activate(1));

        tabs.set_filter(red());

        let filters: Vec<Filter> = tabs.tabs().iter().map(|tab| tab.filter).collect();
        assert_eq!(filters, vec![Filter::default(), red(), Filter::default()]);
    }

    #[test]
    fn two_tabs_keep_different_filters_across_next_and_previous() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), files("first", 1), 0);
        tabs.set_filter(unrated());
        tabs.open_new(PathBuf::from("second"), files("second", 1), 0);
        tabs.set_filter(red());

        tabs.previous();
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(unrated()));
        tabs.next();
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(red()));
        tabs.next();
        assert_eq!(tabs.active().map(|tab| tab.filter), Some(unrated()));
    }

    #[test]
    fn tab_open_clears_the_selection_and_active_mut_changes_only_the_active_tab() {
        let mut tabs = Tabs::default();
        assert_eq!(
            tabs.active().map(|tab| tab.selection.clone()),
            Some(Selection::default())
        );
        tabs.open_in_active(PathBuf::from("first"), files("first", 4), 0);
        tabs.open_new(PathBuf::from("second"), files("second", 4), 0);

        if let Some(tab) = tabs.active_mut() {
            tab.selection.ctrl_click(1, 3);
        }
        assert_eq!(tabs.active().map(|tab| tab.selection.count()), Some(2));
        assert_eq!(tabs.tabs()[0].selection, Selection::default());

        // Opening the folder again (a refresh) clears the marking and the anchor.
        tabs.open_new(PathBuf::from("second"), files("second", 4), 0);
        let selection = tabs.active().map(|tab| tab.selection.clone());
        assert_eq!(selection, Some(Selection::default()));
        assert_eq!(selection.and_then(|selection| selection.anchor()), None);
    }

    #[cfg(windows)]
    #[test]
    fn clear_inside_compares_path_components_without_case_on_windows() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from(r"e:\dcim\100test"), files("nested", 1), 0);

        assert!(tabs.clear_inside(Path::new(r"E:\DCIM")));
        assert_eq!(tabs.tabs(), &[Tab::default()]);
    }

    /// A temporary folder made by the test, removed when dropped.
    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("pw-loupe-tabs-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("Foto")).expect("temporary folder is made");
            Self(root)
        }

        fn foto(&self) -> PathBuf {
            self.0.join("Foto")
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Opens `first` in the start tab, `other` in a second tab and then `again` with `open_new`.
    fn open_twice(first: &Path, again: &Path) -> Tabs {
        let mut tabs = Tabs::default();
        tabs.open_in_active(first.to_path_buf(), files("first", 2), 0);
        tabs.open_new(PathBuf::from("other"), files("other", 1), 0);
        tabs.open_new(again.to_path_buf(), files("again", 3), 2);
        tabs
    }

    #[cfg(windows)]
    #[test]
    fn the_same_folder_in_other_letter_case_opens_in_one_tab() {
        let temp = TempFolder::new("case");
        let first = temp.foto();
        let again = PathBuf::from(first.to_string_lossy().to_uppercase());
        assert_ne!(first, again);

        let tabs = open_twice(&first, &again);

        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active_index(), 0, "the first tab is active");
        // The tab keeps the path given the second time (point 2), not the resolved form.
        assert_eq!(tabs.tabs()[0].folder, Some(again));
        assert_eq!(tabs.tabs()[0].current, 2);
        assert_eq!(
            tabs.tabs()[0].files.first(),
            Some(&PathBuf::from("again-0.arw"))
        );
    }

    #[test]
    fn the_same_folder_with_a_trailing_separator_opens_in_one_tab() {
        let temp = TempFolder::new("trailing");
        let first = temp.foto();
        let again = PathBuf::from(format!("{}{}", first.display(), std::path::MAIN_SEPARATOR));
        assert_ne!(first.as_os_str(), again.as_os_str());

        let tabs = open_twice(&first, &again);

        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.tabs()[0].folder, Some(again));
    }

    /// The relative path is made from the current directory without changing it: changing the
    /// current directory would affect the other tests running at the same time, and writing a
    /// folder there would write into the source tree. `cargo test` runs this test in the crate's
    /// folder, so its existing `src` folder is used, read only.
    #[test]
    fn a_relative_path_to_an_open_folder_opens_in_one_tab() {
        let current = std::env::current_dir().expect("current directory is known");
        let relative = PathBuf::from("src");
        let absolute = current.join(&relative);
        assert!(absolute.is_dir(), "the test runs in the crate's folder");

        let tabs = open_twice(&absolute, &relative);

        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.tabs()[0].folder, Some(relative));
    }

    #[test]
    fn two_different_folders_open_in_two_tabs() {
        let temp = TempFolder::new("different");
        let first = temp.foto();
        let second = temp.0.join("Other");
        std::fs::create_dir_all(&second).expect("second folder is made");

        let mut tabs = Tabs::default();
        tabs.open_new(first.clone(), files("first", 1), 0);
        tabs.open_new(second.clone(), files("second", 1), 0);

        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.tabs()[0].folder, Some(first));
        assert_eq!(tabs.tabs()[1].folder, Some(second));
    }

    #[test]
    fn a_folder_that_does_not_exist_compared_with_itself_opens_in_one_tab() {
        let temp = TempFolder::new("missing");
        let missing = temp.0.join("missing").join("Foto");
        assert!(!missing.exists());

        let tabs = open_twice(&missing, &missing);
        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.tabs()[0].folder, Some(missing));

        // `pw_ingest::resolve` refuses a path with `..` (kept by `absolute` outside Windows); such a
        // path is then compared as it is.
        let parent = temp.0.join("missing").join("..").join("gone");
        assert!(same_folder(&parent, &parent));
    }

    /// After a move every tab of the folder gets its new list (ADR 0017 U9b, T172): two tabs of
    /// one folder, one with a trailing separator, are found, and a third of another folder is not.
    #[test]
    fn tabs_of_folder_mut_finds_both_tabs_of_a_folder_and_not_a_third() {
        let temp = TempFolder::new("of-folder");
        let foto = temp.foto();
        let other = temp.0.join("Other");
        std::fs::create_dir_all(&other).expect("other folder is made");
        let again = PathBuf::from(format!("{}{}", foto.display(), std::path::MAIN_SEPARATOR));

        let mut tabs = Tabs::default();
        tabs.open_in_active(foto.clone(), files("first", 2), 0);
        tabs.open_new(other.clone(), files("other", 1), 0);
        // `open_new` would activate the first tab again; a third tab is opened in place.
        tabs.open_new(temp.0.join("third"), files("third", 1), 0);
        tabs.open_in_active(again.clone(), files("again", 3), 0);
        assert_eq!(tabs.tabs().len(), 3);

        let found: Vec<Option<PathBuf>> = tabs
            .tabs_of_folder_mut(&foto)
            .map(|tab| tab.folder.clone())
            .collect();
        assert_eq!(found, vec![Some(foto.clone()), Some(again)]);

        // The tabs are changed in place.
        for tab in tabs.tabs_of_folder_mut(&foto) {
            tab.current = 1;
        }
        let currents: Vec<usize> = tabs.tabs().iter().map(|tab| tab.current).collect();
        assert_eq!(currents, vec![1, 0, 1]);
        assert_eq!(tabs.tabs_of_folder_mut(&temp.0.join("none")).count(), 0);
    }

    // --- the sort order (ADR 0017 U14, T138) ---

    fn by(key: SortKey, descending: bool) -> SortOrder {
        SortOrder { key, descending }
    }

    fn no_time(_: &Path) -> Option<SystemTime> {
        None
    }

    fn no_rating(_: &Path) -> Option<Rating> {
        None
    }

    fn named(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    fn file_names(tab: &Tab) -> Vec<String> {
        tab.files
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    }

    fn stars(stars: i8) -> Rating {
        Rating {
            stars: Some(stars),
            label: None,
        }
    }

    fn active(tabs: &Tabs) -> &Tab {
        match tabs.active() {
            Some(tab) => tab,
            None => panic!("there is always an active tab"),
        }
    }

    #[test]
    fn the_order_is_applied_once_and_a_rating_change_does_not_reorder() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(
            PathBuf::from("first"),
            named(&["a.arw", "b.arw", "c.arw"]),
            0,
        );
        let ratings = RefCell::new(HashMap::from([
            (PathBuf::from("a.arw"), stars(1)),
            (PathBuf::from("b.arw"), stars(5)),
            (PathBuf::from("c.arw"), stars(3)),
        ]));
        let rating_of = |path: &Path| ratings.borrow().get(path).cloned();
        let best_first = by(SortKey::Rating, true);

        assert_eq!(
            tabs.follow_sort(best_first, true, &rating_of, &no_time),
            SortFollow::Reordered { moved: true }
        );
        let tab = active(&tabs);
        assert_eq!(file_names(tab), ["b.arw", "c.arw", "a.arw"]);
        assert_eq!(tab.files.get(tab.current), Some(&PathBuf::from("a.arw")));
        assert_eq!(tab.sorted, Some(best_first));

        // Culling: a.arw gets 5 stars. Neither the rating nor the next frame moves it (U14-Q5).
        ratings
            .borrow_mut()
            .insert(PathBuf::from("a.arw"), stars(5));
        if let Some(tab) = tabs.active_mut() {
            tab.rated(true, &rating_of);
        }
        assert_eq!(
            tabs.follow_sort(best_first, true, &rating_of, &no_time),
            SortFollow::Sorted
        );
        assert_eq!(file_names(active(&tabs)), ["b.arw", "c.arw", "a.arw"]);

        // Choosing the order again applies it again.
        tabs.forget_sort();
        assert_eq!(
            tabs.follow_sort(best_first, true, &rating_of, &no_time),
            SortFollow::Reordered { moved: true }
        );
        assert_eq!(file_names(active(&tabs)), ["a.arw", "b.arw", "c.arw"]);
    }

    #[test]
    fn a_tab_waiting_for_its_capture_times_keeps_its_order_and_reports_how_many_are_read() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(
            PathBuf::from("first"),
            named(&["a.arw", "b.arw", "c.arw"]),
            1,
        );
        let capture = by(SortKey::Capture, false);

        tabs.record_capture(
            Path::new("a.arw"),
            CaptureTime::parse("2026:10:30 14:03:15"),
        );
        let follow = tabs.follow_sort(capture, true, &no_rating, &no_time);
        assert_eq!(
            follow,
            SortFollow::ReadingCaptureTimes { read: 1, total: 3 }
        );
        let SortFollow::ReadingCaptureTimes { read, total } = follow else {
            panic!("the tab waits");
        };
        assert_eq!(sorting_text(read, total), "Sorting by capture time: 1 of 3");
        assert_eq!(file_names(active(&tabs)), ["a.arw", "b.arw", "c.arw"]);
        assert_eq!(active(&tabs).sorted, None);

        tabs.record_capture(Path::new("b.arw"), None);
        tabs.record_capture(
            Path::new("c.arw"),
            CaptureTime::parse("2026:10:30 14:03:12"),
        );
        assert_eq!(
            tabs.follow_sort(capture, true, &no_rating, &no_time),
            SortFollow::Reordered { moved: true }
        );
        let tab = active(&tabs);
        // b.arw has no date: last.
        assert_eq!(file_names(tab), ["c.arw", "a.arw", "b.arw"]);
        assert_eq!(tab.files.get(tab.current), Some(&PathBuf::from("b.arw")));
    }

    #[test]
    fn every_open_tab_follows_the_order_and_opening_a_folder_again_applies_it_again() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), named(&["a.jpg", "b.arw"]), 0);
        tabs.open_new(PathBuf::from("second"), named(&["c.jpg", "d.arw"]), 0);
        let by_type = by(SortKey::Type, false);

        assert_eq!(
            tabs.follow_sort(by_type, false, &no_rating, &no_time),
            SortFollow::Reordered { moved: true }
        );
        let names: Vec<Vec<String>> = tabs.tabs().iter().map(file_names).collect();
        assert_eq!(names, [["b.arw", "a.jpg"], ["d.arw", "c.jpg"]]);

        // A rating order waits for the ratings, which only the active tab has.
        let rated = by(SortKey::Rating, false);
        assert_eq!(
            tabs.follow_sort(rated, true, &no_rating, &no_time),
            SortFollow::Reordered { moved: true }
        );
        assert_eq!(tabs.tabs()[0].sorted, Some(by_type), "the other tab waits");
        assert_eq!(tabs.tabs()[1].sorted, Some(rated));

        // Opening the folder again lists it by name and applies the order again.
        tabs.open_new(
            PathBuf::from("second"),
            named(&["c.jpg", "d.arw", "e.arw"]),
            0,
        );
        assert_eq!(active(&tabs).sorted, None);
        assert_eq!(
            tabs.follow_sort(by_type, false, &no_rating, &no_time),
            SortFollow::Reordered { moved: true }
        );
        assert_eq!(file_names(active(&tabs)), ["d.arw", "e.arw", "c.jpg"]);
    }

    /// ADR 0026 §2 point 4 (T240): a file listed after the order was applied waits for its
    /// capture time, then `follow_sort` places it and says so; a new order or an open empties the
    /// waiting files.
    #[test]
    fn follow_sort_places_a_waiting_file_once_its_capture_time_is_read() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(
            PathBuf::from("first"),
            named(&["a.arw", "b.arw", "c.arw"]),
            0,
        );
        tabs.record_capture(
            Path::new("a.arw"),
            CaptureTime::parse("2026:10:30 12:00:02"),
        );
        tabs.record_capture(
            Path::new("b.arw"),
            CaptureTime::parse("2026:10:30 12:00:03"),
        );
        tabs.record_capture(
            Path::new("c.arw"),
            CaptureTime::parse("2026:10:30 12:00:01"),
        );
        let capture = by(SortKey::Capture, false);
        assert_eq!(
            tabs.follow_sort(capture, true, &no_rating, &no_time),
            SortFollow::Reordered { moved: true }
        );
        assert_eq!(file_names(active(&tabs)), ["c.arw", "a.arw", "b.arw"]);

        let never = |_: &Path| false;
        let listed = named(&["a.arw", "b.arw", "c.arw", "d.arw"]);
        if let Some(tab) = tabs.active_mut() {
            tab.relist(listed, None, &never, capture, true, &no_rating, &no_time);
        }
        assert_eq!(active(&tabs).unplaced, named(&["d.arw"]));
        assert_eq!(
            tabs.follow_sort(capture, true, &no_rating, &no_time),
            SortFollow::Sorted,
            "d.arw still waits"
        );
        assert_eq!(
            file_names(active(&tabs)),
            ["c.arw", "a.arw", "b.arw", "d.arw"]
        );

        tabs.record_capture(
            Path::new("d.arw"),
            CaptureTime::parse("2026:10:30 12:00:00"),
        );
        assert_eq!(
            tabs.follow_sort(capture, true, &no_rating, &no_time),
            SortFollow::Placed { moved: true }
        );
        assert_eq!(
            file_names(active(&tabs)),
            ["d.arw", "c.arw", "a.arw", "b.arw"]
        );
        assert!(active(&tabs).unplaced.is_empty());
        assert_eq!(active(&tabs).sorted, Some(capture));

        // Another file waits; applying a new order empties `unplaced`.
        let listed = named(&["a.arw", "b.arw", "c.arw", "d.arw", "e.arw"]);
        if let Some(tab) = tabs.active_mut() {
            tab.relist(
                listed.clone(),
                None,
                &never,
                capture,
                true,
                &no_rating,
                &no_time,
            );
        }
        assert_eq!(active(&tabs).unplaced, named(&["e.arw"]));

        // Opening the folder again empties it.
        let mut reopened = tabs.clone();
        reopened.open_new(PathBuf::from("first"), listed, 0);
        assert!(active(&reopened).unplaced.is_empty());

        let by_name = by(SortKey::Name, false);
        assert_eq!(
            tabs.follow_sort(by_name, true, &no_rating, &no_time),
            SortFollow::Reordered { moved: true }
        );
        assert!(active(&tabs).unplaced.is_empty());
        assert_eq!(
            file_names(active(&tabs)),
            ["a.arw", "b.arw", "c.arw", "d.arw", "e.arw"]
        );
    }

    #[test]
    fn a_capture_time_is_kept_by_the_tabs_listing_the_file_until_the_folder_is_opened_again() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from("first"), named(&["a.arw"]), 0);
        tabs.open_new(PathBuf::from("second"), named(&["b.arw"]), 0);
        tabs.record_capture(
            Path::new("a.arw"),
            CaptureTime::parse("2026:10:30 14:03:12"),
        );
        assert!(tabs.tabs()[0].capture.is_read(Path::new("a.arw")));
        assert!(!tabs.tabs()[1].capture.is_read(Path::new("a.arw")));
        // Switching tabs keeps it; opening the folder again empties it.
        tabs.next();
        assert!(tabs.tabs()[0].capture.is_read(Path::new("a.arw")));
        tabs.open_new(PathBuf::from("first"), named(&["a.arw"]), 0);
        assert!(!tabs.tabs()[0].capture.is_read(Path::new("a.arw")));
    }
}
