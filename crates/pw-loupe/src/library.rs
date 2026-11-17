//! The library in the left column (ADR 0017 §2.1, U4, T135): the folder tree of the computer's
//! drives, the favourites and the recently opened folders, as plain Rust. The drives come in as
//! data ([`Drive`]), so everything but [`read_drives`] and [`suppress_no_disk_dialogs`] is tested
//! without real drives; a folder's subfolders are read only when it is expanded (one `read_dir`,
//! on the UI thread), never in the background. Every text comes from `library_text.rs`; folder
//! names and paths are data.

use std::io;
use std::path::{Path, PathBuf};

use crate::library_text::{drive_text, expand_error_text, folder_list_name_text};
use crate::settings::SavedLibrary;

/// How many recent folders are kept (U4-Q4).
pub const RECENT_LIMIT: usize = 10;

/// A drive's kind, from `GetDriveTypeW`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveKind {
    Removable,
    Fixed,
    Network,
    CdRom,
    RamDisk,
    Unknown,
}

impl DriveKind {
    /// The kind of a `GetDriveTypeW` result (`DRIVE_REMOVABLE` = 2 … `DRIVE_RAMDISK` = 6; the
    /// values are fixed by the Windows API).
    pub fn from_drive_type(code: u32) -> Self {
        match code {
            2 => Self::Removable,
            3 => Self::Fixed,
            4 => Self::Network,
            5 => Self::CdRom,
            6 => Self::RamDisk,
            _ => Self::Unknown,
        }
    }

    /// A card reader or a DVD drive: without a disk in it, it is shown as "(no disk)".
    fn may_lack_disk(self) -> bool {
        matches!(self, Self::Removable | Self::CdRom)
    }
}

/// A drive of the computer (U4-Q1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    pub letter: char,
    pub kind: DriveKind,
    /// The volume label; `None` without one or when it could not be read.
    pub label: Option<String>,
    /// `false` for an empty card reader or DVD drive: it cannot be expanded.
    pub has_disk: bool,
}

impl Drive {
    /// A drive as the Windows calls describe it: the label is `None` when empty, and a card reader
    /// or DVD drive whose volume could not be read has no disk.
    pub fn new(letter: char, kind: DriveKind, volume: Option<String>) -> Self {
        let has_disk = volume.is_some() || !kind.may_lack_disk();
        Self {
            letter,
            kind,
            label: volume.filter(|label| !label.is_empty()),
            has_disk,
        }
    }

    /// The drive's root folder, `D:\`.
    pub fn root(&self) -> PathBuf {
        PathBuf::from(format!("{}:\\", self.letter))
    }
}

/// The subfolders of a folder in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Children {
    /// Not read: the folder has not been expanded since it was made or last collapsed.
    Unread,
    /// Read when the folder was expanded, in the tree's order.
    Read(Vec<FolderNode>),
    /// The read failed: the greyed line with the reason (`expand_error_text`).
    Failed(String),
}

/// A drive or a folder in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderNode {
    pub path: PathBuf,
    /// What the tree shows: the drive's name or the folder's name.
    pub text: String,
    /// `false` for a drive without a disk: it has no triangle.
    pub expandable: bool,
    pub expanded: bool,
    pub children: Children,
    /// [`folder_key`] of `path`, made once, so the active tab's folder is found without touching
    /// the disk in every frame.
    key: String,
}

impl FolderNode {
    fn drive(drive: &Drive) -> Self {
        let path = drive.root();
        Self {
            key: folder_key(&path),
            text: drive_text(drive.letter, drive.label.as_deref(), drive.has_disk),
            path,
            expandable: drive.has_disk,
            expanded: false,
            children: Children::Unread,
        }
    }

    fn folder(path: PathBuf) -> Self {
        Self {
            key: folder_key(&path),
            text: display_name(&path),
            path,
            expandable: true,
            expanded: false,
            children: Children::Unread,
        }
    }

    /// Whether this is the folder of `key` ([`folder_key`]).
    pub fn is_folder(&self, key: &str) -> bool {
        self.key == key
    }

    /// Expands the folder and reads its subfolders with `read`; a failed read gives its line.
    fn expand<F>(&mut self, read: &F)
    where
        F: Fn(&Path) -> io::Result<Vec<PathBuf>>,
    {
        if !self.expandable {
            return;
        }
        self.expanded = true;
        self.children = match read(&self.path) {
            Ok(folders) => Children::Read(folders.into_iter().map(Self::folder).collect()),
            Err(error) => Children::Failed(expand_error_text(&error)),
        };
    }

    /// Collapses the folder; its subfolders are read again when it is expanded again.
    fn collapse(&mut self) {
        self.expanded = false;
        self.children = Children::Unread;
    }
}

/// What a row of the tree shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeRowKind<'a> {
    Folder(&'a FolderNode),
    /// The greyed line under a folder whose subfolders could not be read.
    Failed(&'a str),
}

/// A visible row of the tree, with its depth (0 for a drive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRow<'a> {
    pub depth: usize,
    pub kind: TreeRowKind<'a>,
}

/// The folder tree (U4-Q2): the drives, then the folders of each expanded drive or folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderTree {
    roots: Vec<FolderNode>,
}

impl FolderTree {
    /// The tree of `drives`, sorted by letter, none expanded.
    pub fn from_drives(drives: &[Drive]) -> Self {
        let mut drives: Vec<&Drive> = drives.iter().collect();
        drives.sort_by_key(|drive| drive.letter);
        drives.dedup_by_key(|drive| drive.letter);
        Self {
            roots: drives.into_iter().map(FolderNode::drive).collect(),
        }
    }

    pub fn roots(&self) -> &[FolderNode] {
        &self.roots
    }

    /// The rows shown: each node, then the rows of its subfolders when it is expanded.
    pub fn visible_rows(&self) -> Vec<TreeRow<'_>> {
        let mut rows = Vec::new();
        push_rows(&self.roots, 0, &mut rows);
        rows
    }

    /// The triangle of the node at `path`: collapses an expanded node, or expands it and reads
    /// its subfolders with `read`. Returns `false` when there is no such node in the tree.
    pub fn toggle<F>(&mut self, path: &Path, read: &F) -> bool
    where
        F: Fn(&Path) -> io::Result<Vec<PathBuf>>,
    {
        let Some(node) = find_mut(&mut self.roots, path) else {
            return false;
        };
        if node.expanded {
            node.collapse();
        } else {
            node.expand(read);
        }
        true
    }

    /// "Refresh" (F5): the tree of `drives` again, with every folder that was expanded and still
    /// exists expanded again and read anew.
    pub fn refresh<F>(&mut self, drives: &[Drive], read: &F)
    where
        F: Fn(&Path) -> io::Result<Vec<PathBuf>>,
    {
        let before = std::mem::take(&mut self.roots);
        *self = Self::from_drives(drives);
        for root in &mut self.roots {
            if let Some(previous) = before.iter().find(|old| old.path == root.path) {
                reopen(root, previous, read);
            }
        }
    }
}

fn push_rows<'a>(nodes: &'a [FolderNode], depth: usize, rows: &mut Vec<TreeRow<'a>>) {
    for node in nodes {
        rows.push(TreeRow {
            depth,
            kind: TreeRowKind::Folder(node),
        });
        if !node.expanded {
            continue;
        }
        match &node.children {
            Children::Read(children) => push_rows(children, depth + 1, rows),
            Children::Failed(reason) => rows.push(TreeRow {
                depth: depth + 1,
                kind: TreeRowKind::Failed(reason),
            }),
            Children::Unread => {}
        }
    }
}

fn find_mut<'a>(nodes: &'a mut [FolderNode], path: &Path) -> Option<&'a mut FolderNode> {
    for node in nodes {
        if node.path == path {
            return Some(node);
        }
        if path.starts_with(&node.path)
            && let Children::Read(children) = &mut node.children
            && let Some(found) = find_mut(children, path)
        {
            return Some(found);
        }
    }
    None
}

/// Expands `node` again when `previous` (the same folder before a refresh) was expanded, and so
/// on down its subfolders.
fn reopen<F>(node: &mut FolderNode, previous: &FolderNode, read: &F)
where
    F: Fn(&Path) -> io::Result<Vec<PathBuf>>,
{
    if !previous.expanded {
        return;
    }
    node.expand(read);
    if let (Children::Read(children), Children::Read(before)) =
        (&mut node.children, &previous.children)
    {
        for child in children {
            if let Some(old) = before.iter().find(|old| old.path == child.path) {
                reopen(child, old, read);
            }
        }
    }
}

/// The subfolders of `folder` (U4-Q2): folders only, hidden and system folders left out, sorted
/// by name without regard to case. An entry that cannot be looked at is left out; a folder that
/// cannot be listed gives the error.
pub fn read_subfolders(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        // A link or junction to a folder counts as a folder.
        let is_folder = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());
        if !is_folder || hidden_or_system(&entry) {
            continue;
        }
        found.push((entry.file_name().to_string_lossy().into_owned(), path));
    }
    found.sort_by_cached_key(|(name, _)| (name.to_lowercase(), name.clone()));
    Ok(found.into_iter().map(|(_, path)| path).collect())
}

/// Whether the entry has the Windows attribute Hidden or System.
#[cfg(windows)]
fn hidden_or_system(entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;

    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM};

    entry.metadata().is_ok_and(|metadata| {
        metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
    })
}

/// Whether the entry is hidden: its name starts with a dot.
#[cfg(not(windows))]
fn hidden_or_system(entry: &std::fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

/// A folder's name as the library shows it: its last component, or the whole path for a drive's
/// root (`D:\`).
pub fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The names of a list of folders (Favourites or Recent; ADR 0017 U4-Q8, T164), one per path in
/// the same order. A folder whose [`display_name`] no other folder of the list has keeps it; a
/// group with the same name (without regard to case on Windows) gets, for the smallest `k ≥ 1`
/// that makes every path of the group differ, the last `k` components of each path's parent
/// ([`folder_list_name_text`]), or all of them if no `k` does. A path without a parent (a drive's
/// root) keeps its name.
pub fn list_names(paths: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = paths.iter().map(|path| display_name(path)).collect();
    // The paths that have a parent: (index, name key, parent components).
    let candidates: Vec<(usize, String, Vec<String>)> = paths
        .iter()
        .zip(&names)
        .enumerate()
        .filter_map(|(index, (path, name))| {
            let parents = parent_components(path);
            (!parents.is_empty()).then(|| (index, comparison_key(name), parents))
        })
        .collect();
    for (index, key, parents) in &candidates {
        let group: Vec<&[String]> = candidates
            .iter()
            .filter(|(_, other, _)| other == key)
            .map(|(_, _, other)| other.as_slice())
            .collect();
        if group.len() < 2 {
            continue;
        }
        let k = distinct_parent_count(&group);
        if let Some(name) = names.get_mut(*index) {
            *name = folder_list_name_text(name, &last_components(parents, k));
        }
    }
    names
}

/// The components of `path`'s parent as text: a `Prefix` as it is written (`D:`), `RootDir`
/// skipped. Empty for a path without a parent.
fn parent_components(path: &Path) -> Vec<String> {
    path.parent()
        .map(|parent| {
            parent
                .components()
                .filter(|component| !matches!(component, std::path::Component::RootDir))
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The last `k` of `components` (all of them when there are fewer), joined with `\`.
fn last_components(components: &[String], k: usize) -> String {
    components
        .iter()
        .skip(components.len().saturating_sub(k))
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\\")
}

/// The smallest `k ≥ 1` for which the last `k` parent components differ for every path of
/// `group`, or the most components any of them has when none does (then all are used).
fn distinct_parent_count(group: &[&[String]]) -> usize {
    let most = group.iter().map(|parents| parents.len()).max().unwrap_or(0);
    (1..=most)
        .find(|&k| {
            let suffixes: std::collections::HashSet<String> = group
                .iter()
                .map(|parents| comparison_key(&last_components(parents, k)))
                .collect();
            suffixes.len() == group.len()
        })
        .unwrap_or(most)
}

/// A name or part of a path as [`folder_key`] compares it: in lower case on Windows.
fn comparison_key(text: &str) -> String {
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.to_string()
    }
}

/// A cheap key for "is this the same folder" in the tree's highlight (U4-Q5): the absolute path
/// without `\\?\` and without a separator at the end, in lower case on Windows. Nothing is read
/// from the disk, so it can be made in every frame.
pub fn folder_key(path: &Path) -> String {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let text = absolute.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    let text = text.trim_end_matches(['\\', '/']);
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.to_string()
    }
}

/// Whether `a` and `b` name the same folder, compared as tabs compare folders (`tabs.rs`, GR12):
/// each made absolute and resolved with `pw_ingest::resolve`, so case, separators at the end,
/// `\\?\` and `subst` drives do not matter.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    resolved_folder(a) == resolved_folder(b)
}

fn resolved_folder(path: &Path) -> PathBuf {
    std::path::absolute(path)
        .ok()
        .and_then(|absolute| pw_ingest::resolve(&absolute, "folder").ok())
        .unwrap_or_else(|| path.to_path_buf())
}

/// A favourite or a recent folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEntry {
    pub path: PathBuf,
    /// The folder's name in the list ([`list_names`]): its [`display_name`], with as many of its
    /// parent folders as tell it apart when another folder of the list has the same name. The
    /// full path is the tooltip of a folder that exists.
    pub name: String,
    /// Whether the folder existed when the list last looked (at start, at a change of the list
    /// and at "Refresh"); a missing one is greyed and can only be removed.
    pub exists: bool,
}

impl ListEntry {
    fn new(path: PathBuf) -> Self {
        Self {
            name: display_name(&path),
            exists: path.is_dir(),
            path,
        }
    }
}

/// The favourites (in the order they were added, U4-Q3) or the recent folders (most recent first,
/// at most [`RECENT_LIMIT`], U4-Q4); a folder is in a list once ([`same_folder`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderList {
    entries: Vec<ListEntry>,
}

impl FolderList {
    /// The list of `paths` in their order, each folder once (the first kept), at most `limit`.
    pub fn from_paths(paths: &[PathBuf], limit: Option<usize>) -> Self {
        let mut list = Self::default();
        for path in paths {
            if limit.is_some_and(|limit| list.entries.len() >= limit) {
                break;
            }
            list.add(path.clone());
        }
        list.set_names();
        list
    }

    /// Sets every entry's name from [`list_names`], after each change of the list.
    fn set_names(&mut self) {
        let names = list_names(&self.paths());
        for (entry, name) in self.entries.iter_mut().zip(names) {
            entry.name = name;
        }
    }

    pub fn entries(&self) -> &[ListEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The folders in the list's order.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn position(&self, path: &Path) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| same_folder(&entry.path, path))
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.position(path).is_some()
    }

    /// Adds a favourite at the end. Returns `false` when the folder is in the list already.
    pub fn add(&mut self, path: PathBuf) -> bool {
        if self.contains(&path) {
            return false;
        }
        self.entries.push(ListEntry::new(path));
        self.set_names();
        true
    }

    /// Records an opened folder (U4-Q4): first in the list, moved there when it is in the list
    /// already, and the list cut to [`RECENT_LIMIT`].
    pub fn record(&mut self, path: PathBuf) {
        if let Some(index) = self.position(&path) {
            self.entries.remove(index);
        }
        self.entries.insert(0, ListEntry::new(path));
        self.entries.truncate(RECENT_LIMIT);
        self.set_names();
    }

    /// Removes the folder. Returns `false` when it was not in the list.
    pub fn remove(&mut self, path: &Path) -> bool {
        match self.position(path) {
            Some(index) => {
                self.entries.remove(index);
                self.set_names();
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.set_names();
    }

    /// Looks again whether each folder exists.
    pub fn check_folders(&mut self) {
        for entry in &mut self.entries {
            entry.exists = entry.path.is_dir();
        }
    }
}

/// The library: the tree, the favourites and the recent folders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub tree: FolderTree,
    pub favourites: FolderList,
    pub recent: FolderList,
}

impl Library {
    /// The library at start: the tree of `drives` and the lists of `settings.txt`.
    pub fn new(drives: &[Drive], saved: &SavedLibrary) -> Self {
        Self {
            tree: FolderTree::from_drives(drives),
            favourites: FolderList::from_paths(&saved.favourites, None),
            recent: FolderList::from_paths(&saved.recent, Some(RECENT_LIMIT)),
        }
    }

    /// "Refresh" (F5): the drives again (expanded folders read anew), and whether each favourite
    /// and recent folder exists.
    pub fn refresh(&mut self, drives: &[Drive]) {
        self.tree.refresh(drives, &read_subfolders);
        self.favourites.check_folders();
        self.recent.check_folders();
    }

    /// The triangle of a drive or folder in the tree.
    pub fn toggle(&mut self, path: &Path) -> bool {
        self.tree.toggle(path, &read_subfolders)
    }

    /// The lists as `settings.txt` keeps them.
    pub fn saved(&self) -> SavedLibrary {
        SavedLibrary {
            favourites: self.favourites.paths(),
            recent: self.recent.paths(),
        }
    }
}

/// The computer's drives (U4-Q1): the letters from `GetLogicalDrives`, each with its kind from
/// `GetDriveTypeW` and its volume label from `GetVolumeInformationW`. With
/// [`suppress_no_disk_dialogs`] called first, an empty card reader or DVD drive gives no Windows
/// dialog; its volume cannot be read, so it has no disk. A network drive's label is not read: for
/// a disconnected mapped drive `GetVolumeInformationW` can wait for the network's timeout, and
/// the drives are read at start (reviewer, T135).
#[cfg(windows)]
pub fn read_drives() -> Vec<Drive> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };

    /// The label buffer: `MAX_PATH + 1` characters, as the documentation asks.
    const LABEL_LENGTH: u32 = 261;

    // SAFETY: plain call without arguments.
    let mask = unsafe { GetLogicalDrives() };
    let mut drives = Vec::new();
    for (bit, letter) in ('A'..='Z').enumerate() {
        if mask & (1u32 << bit) == 0 {
            continue;
        }
        let root: Vec<u16> = format!("{letter}:\\")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `root` is a NUL-terminated wide string that lives through the call.
        let kind = DriveKind::from_drive_type(unsafe { GetDriveTypeW(root.as_ptr()) });
        if kind == DriveKind::Network {
            drives.push(Drive::new(letter, kind, None));
            continue;
        }
        let mut label = [0u16; LABEL_LENGTH as usize];
        // SAFETY: `root` is NUL-terminated; `label` holds `LABEL_LENGTH` characters, the size
        // given; the other out pointers are null, which the function allows.
        let read = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                label.as_mut_ptr(),
                LABEL_LENGTH,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        let volume = (read != 0).then(|| {
            let text = label.split(|&unit| unit == 0).next().unwrap_or(&[]);
            String::from_utf16_lossy(text)
        });
        drives.push(Drive::new(letter, kind, volume));
    }
    drives
}

/// No drive letters outside Windows: the tree is empty.
#[cfg(not(windows))]
pub fn read_drives() -> Vec<Drive> {
    Vec::new()
}

/// At start (U4-Q1): `SetErrorMode(SEM_FAILCRITICALERRORS)`, keeping the other flags, so an empty
/// card reader or DVD drive gives no Windows "There is no disk in the drive" dialog anywhere in
/// the program. Does nothing outside Windows.
pub fn suppress_no_disk_dialogs() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SetErrorMode,
        };

        // SAFETY: plain calls with value arguments; the first returns the mode before it, which
        // the second keeps with the flag added.
        unsafe {
            let before = SetErrorMode(SEM_FAILCRITICALERRORS);
            SetErrorMode(before | SEM_FAILCRITICALERRORS);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    /// A new, empty temporary folder, removed with its contents when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pw-loupe-library-{}-{id}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn folder(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn drives() -> Vec<Drive> {
        vec![
            Drive::new('E', DriveKind::Removable, None),
            Drive::new('C', DriveKind::Fixed, Some("System".to_string())),
            Drive::new('D', DriveKind::Fixed, Some(String::new())),
            Drive::new('F', DriveKind::CdRom, Some("FILM_2026".to_string())),
            Drive::new('Z', DriveKind::Network, None),
        ]
    }

    fn texts(tree: &FolderTree) -> Vec<(usize, String)> {
        tree.visible_rows()
            .into_iter()
            .map(|row| match row.kind {
                TreeRowKind::Folder(node) => (row.depth, node.text.clone()),
                TreeRowKind::Failed(reason) => (row.depth, format!("! {reason}")),
            })
            .collect()
    }

    /// A reader that gives fixed subfolders: `A` and `B` under a drive, `A1` under `A`.
    fn fake_read(path: &Path) -> io::Result<Vec<PathBuf>> {
        let text = path.to_string_lossy();
        if text.ends_with(":\\") {
            Ok(vec![path.join("A"), path.join("B")])
        } else if text.ends_with('A') {
            Ok(vec![path.join("A1")])
        } else {
            Ok(Vec::new())
        }
    }

    #[test]
    fn drive_types_map_to_their_kinds() {
        assert_eq!(DriveKind::from_drive_type(2), DriveKind::Removable);
        assert_eq!(DriveKind::from_drive_type(3), DriveKind::Fixed);
        assert_eq!(DriveKind::from_drive_type(4), DriveKind::Network);
        assert_eq!(DriveKind::from_drive_type(5), DriveKind::CdRom);
        assert_eq!(DriveKind::from_drive_type(6), DriveKind::RamDisk);
        assert_eq!(DriveKind::from_drive_type(0), DriveKind::Unknown);
        assert_eq!(DriveKind::from_drive_type(1), DriveKind::Unknown);
    }

    #[test]
    fn only_an_unreadable_card_reader_or_dvd_drive_has_no_disk() {
        assert!(!Drive::new('E', DriveKind::Removable, None).has_disk);
        assert!(!Drive::new('F', DriveKind::CdRom, None).has_disk);
        assert!(Drive::new('E', DriveKind::Removable, Some(String::new())).has_disk);
        // A fixed or network drive whose volume could not be read can still be expanded; the
        // expand then shows why it failed.
        assert!(Drive::new('C', DriveKind::Fixed, None).has_disk);
        assert!(Drive::new('Z', DriveKind::Network, None).has_disk);
        assert_eq!(
            Drive::new('D', DriveKind::Fixed, Some(String::new())).label,
            None
        );
        assert_eq!(
            Drive::new('C', DriveKind::Fixed, None).root(),
            PathBuf::from("C:\\")
        );
    }

    #[test]
    fn the_tree_shows_the_drives_by_letter_with_labels_and_no_disk() {
        let tree = FolderTree::from_drives(&drives());
        assert_eq!(
            texts(&tree),
            [
                (0, "System (C:)".to_string()),
                (0, "D:".to_string()),
                (0, "E: (no disk)".to_string()),
                (0, "FILM_2026 (F:)".to_string()),
                (0, "Z:".to_string()),
            ]
        );
        let expandable: Vec<bool> = tree.roots().iter().map(|root| root.expandable).collect();
        assert_eq!(expandable, [true, true, false, true, true]);
        assert!(tree.roots().iter().all(|root| !root.expanded));
        // The same letter twice is shown once.
        let twice = FolderTree::from_drives(&[
            Drive::new('C', DriveKind::Fixed, None),
            Drive::new('C', DriveKind::Fixed, None),
        ]);
        assert_eq!(twice.roots().len(), 1);
    }

    #[test]
    fn a_drive_without_a_disk_cannot_be_expanded() {
        let mut tree = FolderTree::from_drives(&drives());
        let read = |_: &Path| -> io::Result<Vec<PathBuf>> { panic!("an empty drive is not read") };
        assert!(tree.toggle(Path::new("E:\\"), &read));
        assert!(!tree.roots()[2].expanded);
        assert_eq!(tree.visible_rows().len(), 5);
    }

    #[test]
    fn expanding_reads_the_subfolders_once_and_collapsing_hides_them() {
        let mut tree = FolderTree::from_drives(&drives());
        assert!(tree.toggle(Path::new("C:\\"), &fake_read));
        assert!(tree.toggle(Path::new("C:\\A"), &fake_read));
        assert_eq!(
            texts(&tree)[..5],
            [
                (0, "System (C:)".to_string()),
                (1, "A".to_string()),
                (2, "A1".to_string()),
                (1, "B".to_string()),
                (0, "D:".to_string()),
            ]
        );
        // The triangle collapses; the subfolders are read again at the next expand.
        assert!(tree.toggle(Path::new("C:\\"), &fake_read));
        assert_eq!(tree.roots()[0].children, Children::Unread);
        assert_eq!(tree.visible_rows().len(), 5);
        // A path not in the tree does nothing.
        assert!(!tree.toggle(Path::new("C:\\A"), &fake_read));
        assert!(!tree.toggle(Path::new("Q:\\"), &fake_read));
    }

    #[test]
    fn a_failed_expand_shows_one_line_with_the_reason_under_the_folder() {
        let mut tree = FolderTree::from_drives(&drives());
        let denied = |_: &Path| -> io::Result<Vec<PathBuf>> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "nekad"))
        };
        assert!(tree.toggle(Path::new("D:\\"), &denied));
        assert_eq!(
            texts(&tree)[1..3],
            [
                (0, "D:".to_string()),
                (1, "! No access to this folder".to_string()),
            ]
        );
        assert_eq!(tree.visible_rows().len(), 6);
    }

    #[test]
    fn expanding_a_folder_that_is_gone_gives_its_line_from_the_real_reader() {
        let dir = TempDir::new();
        let gone = dir.0.join("gone");
        let mut node = FolderNode::folder(gone);
        node.expand(&read_subfolders);
        assert!(node.expanded);
        assert_eq!(
            node.children,
            Children::Failed("Folder not found".to_string())
        );
    }

    #[test]
    fn refresh_reads_the_drives_again_and_keeps_expanded_folders_expanded() {
        let mut tree = FolderTree::from_drives(&drives());
        tree.toggle(Path::new("C:\\"), &fake_read);
        tree.toggle(Path::new("C:\\A"), &fake_read);
        // The card reader now has a card, and Z: is gone.
        let now = vec![
            Drive::new('C', DriveKind::Fixed, Some("System".to_string())),
            Drive::new('E', DriveKind::Removable, Some("CARD".to_string())),
        ];
        tree.refresh(&now, &fake_read);
        assert_eq!(
            texts(&tree),
            [
                (0, "System (C:)".to_string()),
                (1, "A".to_string()),
                (2, "A1".to_string()),
                (1, "B".to_string()),
                (0, "CARD (E:)".to_string()),
            ]
        );
    }

    #[test]
    fn subfolders_are_sorted_without_regard_to_case_and_files_are_left_out() {
        let dir = TempDir::new();
        for name in ["beta", "Alpha", "gamma", "Beta2", "ÄPPLE"] {
            dir.folder(name);
        }
        std::fs::write(dir.0.join("aaa.jpg"), b"not a folder").unwrap();
        let names: Vec<String> = read_subfolders(&dir.0)
            .unwrap()
            .iter()
            .map(|path| display_name(path))
            .collect();
        assert_eq!(names, ["Alpha", "beta", "Beta2", "gamma", "ÄPPLE"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn dot_folders_are_left_out() {
        let dir = TempDir::new();
        dir.folder(".hidden");
        dir.folder("shown");
        let names: Vec<String> = read_subfolders(&dir.0)
            .unwrap()
            .iter()
            .map(|p| display_name(p))
            .collect();
        assert_eq!(names, ["shown"]);
    }

    #[cfg(windows)]
    #[test]
    fn hidden_and_system_folders_are_left_out() {
        use std::os::windows::ffi::OsStrExt;

        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM, SetFileAttributesW,
        };

        let set = |path: &Path, attributes: u32| {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
            // SAFETY: `wide` is a NUL-terminated wide string that lives through the call.
            assert_ne!(unsafe { SetFileAttributesW(wide.as_ptr(), attributes) }, 0);
        };
        let dir = TempDir::new();
        set(&dir.folder("hidden"), FILE_ATTRIBUTE_HIDDEN);
        set(&dir.folder("system"), FILE_ATTRIBUTE_SYSTEM);
        dir.folder("shown");
        // A dot does not hide a folder on Windows.
        dir.folder(".dotted");
        let names: Vec<String> = read_subfolders(&dir.0)
            .unwrap()
            .iter()
            .map(|path| display_name(path))
            .collect();
        assert_eq!(names, [".dotted", "shown"]);
    }

    #[test]
    fn the_real_reader_fills_the_tree_and_names_a_root_by_its_path() {
        let dir = TempDir::new();
        dir.folder("one");
        let mut node = FolderNode::folder(dir.0.clone());
        node.expand(&read_subfolders);
        let Children::Read(children) = &node.children else {
            panic!("{:?}", node.children);
        };
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].text, "one");
        assert_eq!(display_name(Path::new("C:\\")), "C:\\");
    }

    #[test]
    fn the_active_folder_is_found_by_its_key_however_it_is_written() {
        let dir = TempDir::new();
        let node = FolderNode::folder(dir.folder("Shoot"));
        assert!(node.is_folder(&folder_key(&dir.0.join("Shoot"))));
        let with_separator = PathBuf::from(format!("{}{}", dir.0.join("Shoot").display(), "/"));
        assert!(node.is_folder(&folder_key(&with_separator)));
        #[cfg(windows)]
        {
            let upper = PathBuf::from(dir.0.join("Shoot").to_string_lossy().to_uppercase());
            assert!(node.is_folder(&folder_key(&upper)));
            let verbatim = PathBuf::from(format!(r"\\?\{}", dir.0.join("Shoot").display()));
            assert!(node.is_folder(&folder_key(&verbatim)));
        }
        assert!(!node.is_folder(&folder_key(&dir.0)));
    }

    #[test]
    fn favourites_are_added_once_kept_in_order_and_removed() {
        let dir = TempDir::new();
        let (b, a, c) = (dir.folder("b"), dir.folder("a"), dir.folder("c"));
        let mut favourites = FolderList::default();
        assert!(favourites.is_empty());
        assert!(favourites.add(b.clone()));
        assert!(favourites.add(a.clone()));
        assert!(favourites.add(c.clone()));
        // The same folder written another way is a favourite already.
        assert!(!favourites.add(PathBuf::from(format!("{}{}", b.display(), "/"))));
        #[cfg(windows)]
        assert!(!favourites.add(PathBuf::from(a.to_string_lossy().to_uppercase())));
        assert_eq!(favourites.paths(), [b.clone(), a.clone(), c.clone()]);
        let names: Vec<&str> = favourites
            .entries()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["b", "a", "c"]);
        assert!(favourites.contains(&a));
        assert!(favourites.remove(&a));
        assert!(!favourites.remove(&a));
        assert!(!favourites.contains(&a));
        assert_eq!(favourites.paths(), [b, c]);
    }

    #[test]
    fn a_missing_favourite_is_greyed_and_found_again_after_a_check() {
        let dir = TempDir::new();
        let here = dir.folder("here");
        let later = dir.0.join("later");
        let mut favourites = FolderList::from_paths(&[here, later.clone()], None);
        let exists: Vec<bool> = favourites.entries().iter().map(|e| e.exists).collect();
        assert_eq!(exists, [true, false]);
        // It can still be removed.
        std::fs::create_dir_all(&later).unwrap();
        favourites.check_folders();
        assert!(favourites.entries().iter().all(|entry| entry.exists));
        assert!(favourites.remove(&later));
        assert_eq!(favourites.entries().len(), 1);
    }

    #[test]
    fn recent_folders_are_capped_at_ten_most_recent_first_and_a_reopened_one_moves_to_the_top() {
        let dir = TempDir::new();
        let folders: Vec<PathBuf> = (0..12).map(|n| dir.folder(&format!("f{n}"))).collect();
        let mut recent = FolderList::default();
        for folder in &folders {
            recent.record(folder.clone());
        }
        assert_eq!(recent.entries().len(), RECENT_LIMIT);
        let expected: Vec<PathBuf> = folders.iter().rev().take(10).cloned().collect();
        assert_eq!(recent.paths(), expected);
        // f5 is opened again (written another way): it moves to the top, still once.
        recent.record(PathBuf::from(format!("{}{}", folders[5].display(), "/")));
        assert_eq!(recent.entries().len(), RECENT_LIMIT);
        assert_eq!(recent.entries()[0].name, "f5");
        let f5_count = recent.entries().iter().filter(|e| e.name == "f5").count();
        assert_eq!(f5_count, 1);
        assert_eq!(recent.entries()[1].name, "f11");
        recent.clear();
        assert!(recent.is_empty());
    }

    fn names(paths: &[&str]) -> Vec<String> {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        list_names(&paths)
    }

    #[cfg(windows)]
    #[test]
    fn folders_with_the_same_name_get_as_many_parents_as_tell_them_apart() {
        assert_eq!(
            names(&[
                r"C:\Photos\import\2025\2025-07-25",
                r"D:\Data\Trip\1\2025\2025-07-25",
                r"D:\Data\Trip\1\2\2025\2025-07-25",
            ]),
            [
                r"2025-07-25 (import\2025)",
                r"2025-07-25 (1\2025)",
                r"2025-07-25 (2\2025)",
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_drive_is_a_parent_component_written_as_its_prefix() {
        assert_eq!(names(&[r"D:\a\x", r"E:\b\x"]), ["x (a)", "x (b)"]);
        assert_eq!(names(&[r"D:\x", r"E:\x"]), ["x (D:)", "x (E:)"]);
        assert_eq!(names(&[r"D:\a\x", r"E:\a\x"]), [r"x (D:\a)", r"x (E:\a)"]);
    }

    #[cfg(windows)]
    #[test]
    fn names_that_differ_only_in_case_are_the_same_name() {
        assert_eq!(
            names(&[r"D:\a\Foto", r"D:\b\foto"]),
            ["Foto (a)", "foto (b)"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_name_no_other_folder_has_and_a_drives_root_are_unchanged() {
        assert_eq!(
            names(&[r"D:\a\x", r"D:\Foto\only", r"E:\b\x", r"D:\"]),
            ["x (a)", "only", "x (b)", r"D:\"]
        );
    }

    #[test]
    fn the_names_follow_folders_with_the_same_name_coming_and_going() {
        let dir = TempDir::new();
        let (a, b) = (dir.folder("a/x"), dir.folder("b/x"));
        let mut recent = FolderList::default();
        recent.record(a.clone());
        assert_eq!(recent.entries()[0].name, "x");
        recent.record(b.clone());
        let names: Vec<&str> = recent.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["x (b)", "x (a)"]);
        assert!(recent.remove(&b));
        assert_eq!(recent.entries()[0].name, "x");
        // The other list kind, made from saved paths, names them the same way.
        let favourites = FolderList::from_paths(&[a, b], None);
        let names: Vec<&str> = favourites
            .entries()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["x (a)", "x (b)"]);
    }

    #[test]
    fn the_library_is_made_from_the_saved_lists_and_gives_them_back() {
        let dir = TempDir::new();
        let folders: Vec<PathBuf> = (0..12).map(|n| dir.folder(&format!("r{n}"))).collect();
        let fav = dir.folder("fav");
        let saved = SavedLibrary {
            favourites: vec![fav.clone(), fav.clone()],
            recent: folders.clone(),
        };
        let library = Library::new(&drives(), &saved);
        assert_eq!(library.tree.roots().len(), 5);
        assert_eq!(library.favourites.paths(), std::slice::from_ref(&fav));
        // More than ten saved by hand: the first ten are kept.
        assert_eq!(library.recent.paths(), folders[..10]);
        assert_eq!(
            library.saved(),
            SavedLibrary {
                favourites: vec![fav],
                recent: folders[..10].to_vec(),
            }
        );
    }

    #[test]
    fn library_refresh_checks_the_lists() {
        let dir = TempDir::new();
        let folder = dir.folder("x");
        let mut library = Library::new(
            &[],
            &SavedLibrary {
                favourites: vec![folder.clone()],
                recent: vec![folder.clone()],
            },
        );
        std::fs::remove_dir_all(&folder).unwrap();
        library.refresh(&[]);
        assert!(!library.favourites.entries()[0].exists);
        assert!(!library.recent.entries()[0].exists);
    }
}
