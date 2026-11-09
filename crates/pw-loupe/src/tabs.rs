use std::path::{Component, Path, PathBuf};

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
}

impl Tab {
    fn open(&mut self, folder: PathBuf, files: Vec<PathBuf>, pos: usize) {
        self.folder = Some(folder);
        self.files = files;
        self.current = self
            .files
            .len()
            .checked_sub(1)
            .map_or(0, |last| pos.min(last));
        self.mode = ViewMode::Grid;
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
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
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.folder.as_ref() == Some(&folder))
        {
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

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
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
                    .unwrap_or_else(|| "Tom".to_string())
            })
            .collect()
    }
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
    use super::*;

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
        assert_eq!(tabs.titles(), vec!["Tom"]);
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
        assert_eq!(tabs.titles(), vec!["Tom"]);
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
        assert_eq!(tabs.titles(), vec!["Tom"]);
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

    #[cfg(windows)]
    #[test]
    fn clear_inside_compares_path_components_without_case_on_windows() {
        let mut tabs = Tabs::default();
        tabs.open_in_active(PathBuf::from(r"e:\dcim\100test"), files("nested", 1), 0);

        assert!(tabs.clear_inside(Path::new(r"E:\DCIM")));
        assert_eq!(tabs.tabs(), &[Tab::default()]);
    }
}
