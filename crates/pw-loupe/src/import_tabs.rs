//! What the destination tabs need to know about one running import (ADR 0026 §3.4, T259): the
//! images the import will put in place (`ImportEvent::Planned`) and those it has put in place
//! (`ImportEvent::Placed`), grouped by folder, so the shell can open and fill a tab per folder,
//! leave the images not shown yet out of a listing, and at the end make the newest folder's tab
//! active only if the user has not moved (F-Q10). Plain data, no file access: every folder
//! comparison uses `same_folder_lexically`, never `same_folder`, which reads the disk.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::tabs::{ViewMode, same_folder_lexically};

/// The images of one `Placed` event in one folder (`ImportTabs::place`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedInFolder {
    /// The folder, as the first of `images` writes its parent.
    pub folder: PathBuf,
    /// The images placed in `folder`, in the event's order.
    pub images: Vec<PathBuf>,
    /// No image of this folder was placed earlier in this import.
    pub first: bool,
}

/// The active tab's folder, current image (`None` for an empty tab) and view, as the shell sees
/// them; the end rule of F-Q10 compares the one kept at the first shown image with the one at the
/// end (`ImportTabs::end_target`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportPosition {
    pub folder: PathBuf,
    pub current: Option<PathBuf>,
    pub mode: ViewMode,
}

/// File names of one folder, compared without regard to ASCII case.
#[derive(Clone, Debug, Default)]
struct FolderNames {
    folder: PathBuf,
    names: HashSet<OsString>,
}

/// Adds `path` to the folder group `same_folder_lexically` finds for its parent, or to a new group;
/// false (nothing added) when `path` has no parent folder or no file name.
fn insert_name(groups: &mut Vec<FolderNames>, path: &Path) -> bool {
    let (Some(folder), Some(name)) = (parent_folder(path), path.file_name()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    if let Some(group) = groups
        .iter_mut()
        .find(|group| same_folder_lexically(&group.folder, folder))
    {
        group.names.insert(name);
    } else {
        groups.push(FolderNames {
            folder: folder.to_path_buf(),
            names: HashSet::from([name]),
        });
    }
    true
}

/// Whether `path`'s folder (by `same_folder_lexically`) and file name (ASCII case ignored) are in
/// `groups`.
fn contains_name(groups: &[FolderNames], path: &Path) -> bool {
    let (Some(folder), Some(name)) = (parent_folder(path), path.file_name()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    groups
        .iter()
        .filter(|group| same_folder_lexically(&group.folder, folder))
        .any(|group| group.names.contains(&name))
}

/// The parent folder of `path`; `None` for a path without one, also a bare file name (whose
/// parent is the empty path).
fn parent_folder(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

/// One running import's images and folders (ADR 0026 §3.4).
#[derive(Clone, Debug, Default)]
pub struct ImportTabs {
    planned: Vec<FolderNames>,
    placed: Vec<FolderNames>,
    folders: Vec<PathBuf>,
    first: Option<ImportPosition>,
}

impl ImportTabs {
    /// The destination images the import will put in place (`ImportEvent::Planned`), none placed
    /// yet.
    pub fn new(planned: Vec<PathBuf>) -> Self {
        let mut groups = Vec::new();
        for path in &planned {
            insert_name(&mut groups, path);
        }
        Self {
            planned: groups,
            ..Self::default()
        }
    }

    /// Records `images` (one `ImportEvent::Placed`) as placed and returns them grouped by their
    /// parent folder: the groups in the order their folders first come in `images`, each group's
    /// images in `images`' order. Folders are told apart by `same_folder_lexically`; a group's
    /// `folder` is the parent as its first image writes it, and its `first` is true when no image
    /// of that folder was placed earlier in this import. An image placed but never planned is
    /// recorded and grouped like the others; an image without a parent folder or a file name is
    /// left out.
    pub fn place(&mut self, images: &[PathBuf]) -> Vec<PlacedInFolder> {
        let mut groups: Vec<PlacedInFolder> = Vec::new();
        for image in images {
            let Some(folder) = parent_folder(image) else {
                continue;
            };
            if !insert_name(&mut self.placed, image) {
                continue;
            }
            if let Some(group) = groups
                .iter_mut()
                .find(|group| same_folder_lexically(&group.folder, folder))
            {
                group.images.push(image.clone());
                continue;
            }
            let first = !self
                .folders
                .iter()
                .any(|known| same_folder_lexically(known, folder));
            if first {
                self.folders.push(folder.to_path_buf());
            }
            groups.push(PlacedInFolder {
                folder: folder.to_path_buf(),
                images: vec![image.clone()],
                first,
            });
        }
        groups
    }

    /// The folders images were placed in, in the order of their first placement.
    pub fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    /// Whether `path` is a planned image not placed yet. Matched by folder with
    /// `same_folder_lexically` and by file name without regard to ASCII case, so a tab's own
    /// spelling of the folder matches the import's.
    pub fn waiting(&self, path: &Path) -> bool {
        contains_name(&self.planned, path) && !contains_name(&self.placed, path)
    }

    /// `listing` without the paths `waiting` names, in its order.
    pub fn leave_out_waiting(&self, listing: Vec<PathBuf>) -> Vec<PathBuf> {
        listing
            .into_iter()
            .filter(|path| !self.waiting(path))
            .collect()
    }

    /// Keeps `at`, the active tab's position when the first image was shown, the first time it is
    /// called; later calls are ignored (F-Q10).
    pub fn first_shown(&mut self, at: ImportPosition) {
        if self.first.is_none() {
            self.first = Some(at);
        }
    }

    /// Whether `first_shown` has kept a position.
    pub fn first_kept(&self) -> bool {
        self.first.is_some()
    }

    /// The end rule of F-Q10: the newest folder (the last of `folders()`) when a position was
    /// kept, `now` is equal to it (folder by `same_folder_lexically`, `current` exactly, `mode`
    /// equal) and the newest folder is not that same folder; otherwise `None`, so a user who has
    /// moved stays where they are.
    pub fn end_target(&self, now: Option<&ImportPosition>) -> Option<PathBuf> {
        let kept = self.first.as_ref()?;
        let now = now?;
        let unmoved = same_folder_lexically(&kept.folder, &now.folder)
            && kept.current == now.current
            && kept.mode == now.mode;
        if !unmoved {
            return None;
        }
        let newest = self.folders.last()?;
        if same_folder_lexically(newest, &kept.folder) {
            None
        } else {
            Some(newest.clone())
        }
    }
}

/// A tab's new listing after an `ImportEvent::Placed`: `files` (the tab's files, in its own form)
/// followed by each of `images` written in the tab's form (`folder` joined with the image's file
/// name) whose file name no file of `files` has (file names compared without regard to ASCII
/// case), in `images`' order. An image without a file name is left out, and an image whose name
/// came earlier in `images` is not added twice.
pub fn with_placed(folder: &Path, files: &[PathBuf], images: &[PathBuf]) -> Vec<PathBuf> {
    let mut names: HashSet<OsString> = files
        .iter()
        .filter_map(|file| file.file_name())
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut listing = files.to_vec();
    for image in images {
        let Some(name) = image.file_name() else {
            continue;
        };
        if names.insert(name.to_ascii_lowercase()) {
            listing.push(folder.join(name));
        }
    }
    listing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(items: &[&str]) -> Vec<PathBuf> {
        items.iter().map(PathBuf::from).collect()
    }

    fn position(folder: &str, current: Option<&str>, mode: ViewMode) -> ImportPosition {
        ImportPosition {
            folder: PathBuf::from(folder),
            current: current.map(PathBuf::from),
            mode,
        }
    }

    #[test]
    fn place_groups_by_folder_and_marks_each_folders_first_group() {
        let mut import = ImportTabs::new(paths(&[
            r"D:\Import\A\1.arw",
            r"D:\Import\A\2.arw",
            r"D:\Import\B\3.arw",
            r"D:\Import\A\4.arw",
        ]));

        let first = import.place(&paths(&[
            r"D:\Import\A\1.arw",
            r"D:\Import\B\3.arw",
            r"D:\Import\A\2.arw",
        ]));
        assert_eq!(
            first,
            vec![
                PlacedInFolder {
                    folder: PathBuf::from(r"D:\Import\A"),
                    images: paths(&[r"D:\Import\A\1.arw", r"D:\Import\A\2.arw"]),
                    first: true,
                },
                PlacedInFolder {
                    folder: PathBuf::from(r"D:\Import\B"),
                    images: paths(&[r"D:\Import\B\3.arw"]),
                    first: true,
                },
            ]
        );

        let second = import.place(&paths(&[r"D:\Import\C\5.arw", r"D:\Import\A\4.arw"]));
        assert_eq!(
            second,
            vec![
                PlacedInFolder {
                    folder: PathBuf::from(r"D:\Import\C"),
                    images: paths(&[r"D:\Import\C\5.arw"]),
                    first: true,
                },
                PlacedInFolder {
                    folder: PathBuf::from(r"D:\Import\A"),
                    images: paths(&[r"D:\Import\A\4.arw"]),
                    first: false,
                },
            ]
        );
        assert_eq!(
            import.folders(),
            paths(&[r"D:\Import\A", r"D:\Import\B", r"D:\Import\C"]).as_slice()
        );
    }

    #[test]
    fn place_leaves_out_an_image_without_a_parent_folder() {
        let mut import = ImportTabs::new(Vec::new());
        let groups = import.place(&paths(&["1.arw", r"D:\Import\A\2.arw", ""]));
        assert_eq!(
            groups,
            vec![PlacedInFolder {
                folder: PathBuf::from(r"D:\Import\A"),
                images: paths(&[r"D:\Import\A\2.arw"]),
                first: true,
            }]
        );
        assert_eq!(import.folders(), paths(&[r"D:\Import\A"]).as_slice());
    }

    #[test]
    fn place_groups_a_folder_spelt_two_ways_under_its_first_spelling() {
        let mut import = ImportTabs::new(Vec::new());
        let groups = import.place(&paths(&[r"D:\Import\A\1.arw", r"D:\Import\A\\2.arw"]));
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups.first().map(|group| group.folder.clone()),
            Some(PathBuf::from(r"D:\Import\A"))
        );
        assert_eq!(import.folders().len(), 1);
    }

    #[test]
    fn waiting_is_planned_and_not_placed() {
        let mut import = ImportTabs::new(paths(&[r"D:\Import\A\1.arw", r"D:\Import\A\2.arw"]));
        assert!(import.waiting(Path::new(r"D:\Import\A\1.arw")));
        assert!(import.waiting(Path::new(r"D:\Import\A\2.arw")));
        assert!(!import.waiting(Path::new(r"D:\Import\A\3.arw")));
        assert!(!import.waiting(Path::new(r"D:\Import\B\1.arw")));

        import.place(&paths(&[r"D:\Import\A\1.arw"]));
        assert!(!import.waiting(Path::new(r"D:\Import\A\1.arw")));
        assert!(import.waiting(Path::new(r"D:\Import\A\2.arw")));

        // Placed but never planned: not waiting either.
        import.place(&paths(&[r"D:\Import\A\9.arw"]));
        assert!(!import.waiting(Path::new(r"D:\Import\A\9.arw")));
    }

    #[test]
    fn waiting_matches_another_spelling_of_the_folder_and_file_name() {
        let import = ImportTabs::new(paths(&[r"D:\Import\Card A\DSC0001.ARW"]));
        // A trailing separator and a file name in other case; the folders need not exist.
        assert!(import.waiting(Path::new(r"D:\Import\Card A\\dsc0001.arw")));
        assert!(import.waiting(Path::new(r"D:\Import\Card A\.\DSC0001.arw")));
        assert!(!import.waiting(Path::new(r"D:\Import\Card B\DSC0001.ARW")));
    }

    /// On Windows a folder spelt in other case is the same folder.
    #[cfg(windows)]
    #[test]
    fn waiting_ignores_the_folders_case_on_windows() {
        let mut import = ImportTabs::new(paths(&[r"D:\Import\Card A\DSC0001.ARW"]));
        assert!(import.waiting(Path::new(r"d:\import\card a\dsc0001.arw")));
        import.place(&paths(&[r"D:\IMPORT\CARD A\dsc0001.arw"]));
        assert!(!import.waiting(Path::new(r"D:\Import\Card A\DSC0001.ARW")));
    }

    #[test]
    fn leave_out_waiting_keeps_the_order_and_drops_only_waiting_images() {
        let mut import = ImportTabs::new(paths(&[
            r"D:\Import\A\2.arw",
            r"D:\Import\A\4.arw",
            r"D:\Import\A\5.arw",
        ]));
        import.place(&paths(&[r"D:\Import\A\5.arw"]));
        let listing = paths(&[
            r"D:\Import\A\5.arw",
            r"D:\Import\A\4.arw",
            r"D:\Import\A\3.arw",
            r"D:\Import\A\2.arw",
            r"D:\Import\A\1.arw",
        ]);
        assert_eq!(
            import.leave_out_waiting(listing),
            paths(&[
                r"D:\Import\A\5.arw",
                r"D:\Import\A\3.arw",
                r"D:\Import\A\1.arw",
            ])
        );
    }

    #[test]
    fn with_placed_adds_the_new_images_in_the_tabs_form_after_its_files() {
        let folder = Path::new(r"d:\import\a");
        let files = paths(&[r"d:\import\a\1.arw", r"d:\import\a\3.arw"]);
        let images = paths(&[
            r"D:\Import\A\5.arw",
            r"D:\Import\A\3.ARW",
            r"D:\Import\A\2.arw",
            r"D:\Import\A\5.ARW",
        ]);
        assert_eq!(
            with_placed(folder, &files, &images),
            paths(&[
                r"d:\import\a\1.arw",
                r"d:\import\a\3.arw",
                r"d:\import\a\5.arw",
                r"d:\import\a\2.arw",
            ])
        );
    }

    #[test]
    fn with_placed_leaves_out_an_image_without_a_file_name() {
        let folder = Path::new(r"D:\Import\A");
        assert_eq!(
            with_placed(folder, &[], &paths(&["", r"D:\Import\A\1.arw"])),
            paths(&[r"D:\Import\A\1.arw"])
        );
    }

    #[test]
    fn first_kept_is_set_by_first_shown() {
        let mut import = ImportTabs::new(Vec::new());
        assert!(!import.first_kept());
        import.first_shown(position(r"D:\Import\A", None, ViewMode::Grid));
        assert!(import.first_kept());
    }

    #[test]
    fn end_target_is_the_newest_folder_only_when_the_user_has_not_moved() {
        let mut import = ImportTabs::new(Vec::new());
        let kept = position(r"D:\Import\A", Some(r"D:\Import\A\1.arw"), ViewMode::Grid);
        import.place(&paths(&[r"D:\Import\A\1.arw"]));

        // Before `first_shown`.
        assert_eq!(import.end_target(Some(&kept)), None);

        import.first_shown(kept.clone());
        // One folder: the newest is the kept one.
        assert_eq!(import.end_target(Some(&kept)), None);

        import.place(&paths(&[r"D:\Import\B\2.arw"]));
        assert_eq!(
            import.end_target(Some(&kept)),
            Some(PathBuf::from(r"D:\Import\B"))
        );
        // The same folder spelt with a trailing separator is the same position.
        let same = position(r"D:\Import\A\", Some(r"D:\Import\A\1.arw"), ViewMode::Grid);
        assert_eq!(
            import.end_target(Some(&same)),
            Some(PathBuf::from(r"D:\Import\B"))
        );

        let other_tab = position(r"D:\Import\B", Some(r"D:\Import\A\1.arw"), ViewMode::Grid);
        let other_image = position(r"D:\Import\A", Some(r"D:\Import\A\2.arw"), ViewMode::Grid);
        let no_image = position(r"D:\Import\A", None, ViewMode::Grid);
        let other_view = position(r"D:\Import\A", Some(r"D:\Import\A\1.arw"), ViewMode::Loupe);
        assert_eq!(import.end_target(Some(&other_tab)), None);
        assert_eq!(import.end_target(Some(&other_image)), None);
        assert_eq!(import.end_target(Some(&no_image)), None);
        assert_eq!(import.end_target(Some(&other_view)), None);
        assert_eq!(import.end_target(None), None);
    }

    #[test]
    fn a_second_first_shown_does_not_replace_the_first() {
        let mut import = ImportTabs::new(Vec::new());
        import.place(&paths(&[r"D:\Import\A\1.arw", r"D:\Import\B\2.arw"]));
        let kept = position(r"D:\Import\A", Some(r"D:\Import\A\1.arw"), ViewMode::Grid);
        let later = position(r"D:\Other", None, ViewMode::Loupe);
        import.first_shown(kept.clone());
        import.first_shown(later.clone());
        assert_eq!(
            import.end_target(Some(&kept)),
            Some(PathBuf::from(r"D:\Import\B"))
        );
        assert_eq!(import.end_target(Some(&later)), None);
    }
}
