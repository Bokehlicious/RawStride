//! Moving an image with its companion files (ADR 0017 §2.6, step U9b; T169).
//!
//! - [`companions`] lists the files that belong to an image in its own folder: the image, its
//!   RAW+JPEG partners, and the sidecars of both (`.xmp` and the given suffixes such as `dop`).
//! - [`move_group`] moves such a group into a folder (for example `_rejected`), all or nothing,
//!   with the safe write's `move_new`, which never overwrites a file.
//! - [`move_back`] moves the files back where they came from, by the same rules.
//!
//! Extensions and suffixes are given without the leading dot, in any case; every name is compared
//! without regard to case. There is no journal: a crash between two moves leaves the moved files
//! in the target folder, where the user finds them (reviewer 2026-11-17; a move never replaces a
//! file's content).

use std::collections::HashSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::fileops;

/// The reason when the name is already taken in the folder a file is moved to.
const NAME_TAKEN: &str = "the name is taken in the target folder";
/// The reason when another program holds the file open (Windows errors 32 and 33).
#[cfg_attr(not(windows), allow(dead_code))]
const OPEN_ELSEWHERE: &str = "the file is open in another program";
/// The reason when the target folder's name is taken by something that is not a folder.
const NOT_A_FOLDER: &str = "it exists and is not a folder";

/// One file moved: from where and to where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// A group move that failed. Every file the call had moved was moved back, except those in
/// `left`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveError {
    /// The file that could not be moved (or the target folder, when it could not be made).
    pub file: PathBuf,
    /// Why, in English: the name is taken in the target folder, the file is open in another
    /// program, or the system's error text.
    pub reason: String,
    /// Moves that could not be undone; each file is still at its `to`. Empty when the rollback
    /// was complete.
    pub left: Vec<Moved>,
}

impl MoveError {
    fn new(file: &Path, reason: impl Into<String>) -> Self {
        MoveError {
            file: file.to_path_buf(),
            reason: reason.into(),
            left: Vec::new(),
        }
    }
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = name_of(&self.file);
        let reason = &self.reason;
        if self.left.is_empty() {
            write!(f, "Could not move {name}: {reason}.")
        } else {
            let left = self
                .left
                .iter()
                .map(|m| m.to.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                f,
                "Could not move {name}: {reason}. Some files could not be moved back and are \
                 now at: {left}."
            )
        }
    }
}

impl std::error::Error for MoveError {}

/// The files in `image`'s folder that move with it, the image first, then its partners, then the
/// other files by name (lower case). With `base` the image's name without its extension:
/// - partners: every other file `<base>.<e>` with `e` in `image_extensions`;
/// - for the image and each partner, with `m` its whole name: `<m>.xmp` and `<m>.<s>` for each
///   suffix `s`;
/// - `<base>.xmp` and `<base>.<s>` for each suffix `s`.
///
/// Only files are listed, each once; names are compared without regard to case. An image that
/// does not exist is an error.
pub fn companions(
    image: &Path,
    image_extensions: &[&str],
    suffixes: &[&str],
) -> io::Result<Vec<PathBuf>> {
    if !std::fs::metadata(image)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a file", image.display()),
        ));
    }
    let image_name = image
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} has no file name", image.display()),
            )
        })?;
    let base = image
        .file_stem()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let folder = image.parent().unwrap_or_else(|| Path::new(""));
    let read_from = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };

    // Every other file in the folder, with its name in lower case, sorted by that name.
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(read_from)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let lower = file_name.to_string_lossy().to_lowercase();
        if lower == image_name {
            continue;
        }
        entries.push((lower, folder.join(file_name)));
    }
    entries.sort();

    let partner_names: HashSet<String> = image_extensions
        .iter()
        .map(|e| format!("{base}.{}", e.to_lowercase()))
        .collect();
    let (partners, rest): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(lower, _)| partner_names.contains(lower));

    let mut sidecar_endings: Vec<String> = vec!["xmp".to_string()];
    sidecar_endings.extend(suffixes.iter().map(|s| s.to_lowercase()));
    let owners = std::iter::once(&image_name)
        .chain(partners.iter().map(|(lower, _)| lower))
        .chain(std::iter::once(&base));
    let mut wanted: HashSet<String> = HashSet::new();
    for owner in owners {
        for ending in &sidecar_endings {
            wanted.insert(format!("{owner}.{ending}"));
        }
    }

    let mut files = vec![image.to_path_buf()];
    files.extend(partners.into_iter().map(|(_, path)| path));
    files.extend(
        rest.into_iter()
            .filter(|(lower, _)| wanted.contains(lower))
            .map(|(_, path)| path),
    );
    Ok(files)
}

/// Moves `files` into `target`, all or nothing, and returns the moves in order.
///
/// `target` is created if it does not exist (one level); it is an error if it exists and is not a
/// folder. Every `target/<file name>` is checked first: a name that is taken fails the group with
/// nothing moved. The files are then moved in order without ever overwriting a file; if one
/// fails, the files already moved are moved back in reverse order. A `target` this call created
/// is removed again after a failure if it is empty.
pub fn move_group(files: &[PathBuf], target: &Path) -> Result<Vec<Moved>, MoveError> {
    let created = prepare_target(target)?;
    let result = plan_into(files, target).and_then(|plan| run(&plan));
    if result.is_err() && created {
        // Fails when the folder is not empty; then it is left for the user.
        let _ = std::fs::remove_dir(target);
    }
    result
}

/// Moves the files of `moved` back from `to` to `from`, by the same rules as [`move_group`]:
/// every `from` is checked free first, then the files are moved in reverse order; on a failure,
/// the files already moved back are moved to `to` again. The target folder is left as it is.
///
/// In the error, `left` holds the moves back that could not be undone (`from` in the target
/// folder, `to` the original place, where the file still is).
pub fn move_back(moved: &[Moved]) -> Result<(), MoveError> {
    let plan: Vec<Moved> = moved
        .iter()
        .rev()
        .map(|m| Moved {
            from: m.to.clone(),
            to: m.from.clone(),
        })
        .collect();
    for step in &plan {
        check_free(&step.from, &step.to)?;
    }
    run(&plan).map(|_| ())
}

/// Makes sure `target` is a folder; `true` when this call created it.
fn prepare_target(target: &Path) -> Result<bool, MoveError> {
    match std::fs::metadata(target) {
        Ok(metadata) if metadata.is_dir() => Ok(false),
        Ok(_) => Err(MoveError::new(target, NOT_A_FOLDER)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => std::fs::create_dir(target)
            .map(|()| true)
            .map_err(|e| MoveError::new(target, reason_of_io(&e))),
        Err(e) => Err(MoveError::new(target, reason_of_io(&e))),
    }
}

/// The moves of `files` into `target`, after checking that every name is free there and that no
/// two files get the same name (without regard to case; a file given twice counts as two).
fn plan_into(files: &[PathBuf], target: &Path) -> Result<Vec<Moved>, MoveError> {
    let mut plan = Vec::with_capacity(files.len());
    let mut planned_names: HashSet<String> = HashSet::with_capacity(files.len());
    for file in files {
        let name = file
            .file_name()
            .ok_or_else(|| MoveError::new(file, "it has no file name"))?;
        if !planned_names.insert(name.to_string_lossy().to_lowercase()) {
            return Err(MoveError::new(file, NAME_TAKEN));
        }
        let to = target.join(name);
        check_free(file, &to)?;
        plan.push(Moved {
            from: file.clone(),
            to,
        });
    }
    Ok(plan)
}

/// An error naming `file` when `destination` exists (or cannot be checked).
fn check_free(file: &Path, destination: &Path) -> Result<(), MoveError> {
    match std::fs::symlink_metadata(destination) {
        Ok(_) => Err(MoveError::new(file, NAME_TAKEN)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(MoveError::new(file, reason_of_io(&e))),
    }
}

/// Carries out `plan` in order; on a failure, undoes the moves already made.
fn run(plan: &[Moved]) -> Result<Vec<Moved>, MoveError> {
    let mut done: Vec<Moved> = Vec::with_capacity(plan.len());
    for step in plan {
        if let Err(e) = fileops::move_new(&step.from, &step.to) {
            return Err(MoveError {
                file: step.from.clone(),
                reason: reason_of(&e),
                left: undo(&done),
            });
        }
        done.push(step.clone());
    }
    Ok(done)
}

/// Moves the files of `done` back in reverse order; returns the moves that could not be undone,
/// in the order they were made.
fn undo(done: &[Moved]) -> Vec<Moved> {
    let mut left: Vec<Moved> = done
        .iter()
        .rev()
        .filter(|m| fileops::move_new(&m.to, &m.from).is_err())
        .cloned()
        .collect();
    left.reverse();
    left
}

fn reason_of(error: &crate::Error) -> String {
    match error {
        crate::Error::Io(e) => reason_of_io(e),
        other => other.to_string(),
    }
}

/// The reason for a failed file operation: the two causes a user can act on in words of their
/// own, anything else the system's text.
fn reason_of_io(error: &io::Error) -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{
            ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
        };

        let is = |code: u32| error.raw_os_error() == i32::try_from(code).ok();
        if is(ERROR_SHARING_VIOLATION) || is(ERROR_LOCK_VIOLATION) {
            return OPEN_ELSEWHERE.to_string();
        }
        if is(ERROR_FILE_EXISTS) || is(ERROR_ALREADY_EXISTS) {
            return NAME_TAKEN.to_string();
        }
    }
    error.to_string()
}

/// The file name of `path`, or the whole path when it has none.
fn name_of(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temporary folder, removed when dropped.
    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pw-metadata-relocate-{name}-{}-{:016x}",
                std::process::id(),
                fileops::unique_number()
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempFolder(path)
        }

        fn file(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, name.as_bytes()).unwrap();
            path
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const IMAGES: &[&str] = &["arw", "jpg", "dng"];
    const SUFFIXES: &[&str] = &["dop", "pp3"];

    fn names(files: &[PathBuf]) -> Vec<String> {
        files.iter().map(|f| name_of(f)).collect()
    }

    #[test]
    fn companions_of_an_arw_with_a_dop_and_an_upper_case_xmp() {
        let dir = TempFolder::new("arw");
        let image = dir.file("TN0001.ARW");
        dir.file("TN0001.ARW.dop");
        dir.file("TN0001.XMP");
        let files = companions(&image, IMAGES, SUFFIXES).unwrap();
        assert_eq!(
            names(&files),
            ["TN0001.ARW", "TN0001.ARW.dop", "TN0001.XMP"]
        );
        assert_eq!(files[0], image);
    }

    #[test]
    fn companions_include_a_jpg_partner_and_its_sidecars() {
        let dir = TempFolder::new("partner");
        let image = dir.file("TN0002.ARW");
        dir.file("TN0002.JPG");
        dir.file("TN0002.JPG.xmp");
        dir.file("TN0002.dop");
        let files = companions(&image, IMAGES, SUFFIXES).unwrap();
        assert_eq!(
            names(&files),
            ["TN0002.ARW", "TN0002.JPG", "TN0002.dop", "TN0002.JPG.xmp"]
        );
    }

    #[test]
    fn companions_are_found_in_any_case() {
        let dir = TempFolder::new("case");
        let image = dir.file("tn0003.arw");
        dir.file("TN0003.Jpg");
        dir.file("tn0003.ARW.DOP");
        dir.file("Tn0003.jpg.Xmp");
        dir.file("TN0003.Pp3");
        let files = companions(&image, &["ARW", "JPG"], &["DOP", "pp3"]).unwrap();
        assert_eq!(
            names(&files),
            [
                "tn0003.arw",
                "TN0003.Jpg",
                "tn0003.ARW.DOP",
                "Tn0003.jpg.Xmp",
                "TN0003.Pp3"
            ]
        );
    }

    #[test]
    fn companions_leave_out_other_images_folders_and_unknown_suffixes() {
        let dir = TempFolder::new("others");
        let image = dir.file("TN0004.ARW");
        dir.file("TN0004.xmp");
        dir.file("TN00042.ARW");
        dir.file("TN00042.xmp");
        dir.file("TN0004.ARW.bak");
        dir.file("TN0004.txt");
        std::fs::create_dir(dir.0.join("TN0004.ARW.dop")).unwrap();
        std::fs::create_dir(dir.0.join("TN0004.JPG")).unwrap();
        let files = companions(&image, IMAGES, SUFFIXES).unwrap();
        assert_eq!(names(&files), ["TN0004.ARW", "TN0004.xmp"]);
    }

    #[test]
    fn companions_list_each_file_once() {
        let dir = TempFolder::new("once");
        let image = dir.file("TN0005.ARW");
        dir.file("TN0005.JPG");
        dir.file("TN0005.xmp");
        dir.file("TN0005.ARW.xmp");
        dir.file("TN0005.JPG.xmp");
        // The image's own extension in the list of image types does not list it twice.
        let files = companions(&image, IMAGES, &["xmp", "dop"]).unwrap();
        assert_eq!(
            names(&files),
            [
                "TN0005.ARW",
                "TN0005.JPG",
                "TN0005.ARW.xmp",
                "TN0005.JPG.xmp",
                "TN0005.xmp"
            ]
        );
        let unique: HashSet<&PathBuf> = files.iter().collect();
        assert_eq!(unique.len(), files.len());
    }

    #[test]
    fn companions_of_a_missing_image_is_an_error() {
        let dir = TempFolder::new("missing");
        dir.file("TN0006.xmp");
        let error = companions(&dir.0.join("TN0006.ARW"), IMAGES, SUFFIXES).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[cfg(windows)]
    fn group(dir: &TempFolder) -> Vec<PathBuf> {
        vec![
            dir.file("TN0010.ARW"),
            dir.file("TN0010.ARW.dop"),
            dir.file("TN0010.XMP"),
        ]
    }

    #[cfg(windows)]
    fn all_exist(files: &[PathBuf]) -> bool {
        files.iter().all(|f| f.is_file())
    }

    #[test]
    #[cfg(windows)]
    fn move_group_moves_all_files_into_a_new_folder() {
        let dir = TempFolder::new("group");
        let files = group(&dir);
        let target = dir.0.join("_rejected");
        let moved = move_group(&files, &target).unwrap();
        let expected: Vec<Moved> = files
            .iter()
            .map(|f| Moved {
                from: f.clone(),
                to: target.join(f.file_name().unwrap()),
            })
            .collect();
        assert_eq!(moved, expected);
        assert!(files.iter().all(|f| !f.exists()));
        assert!(moved.iter().all(|m| m.to.is_file()));
        assert_eq!(std::fs::read(&moved[1].to).unwrap(), b"TN0010.ARW.dop");
    }

    #[test]
    #[cfg(windows)]
    fn move_group_with_a_taken_name_moves_nothing() {
        let dir = TempFolder::new("taken");
        let files = group(&dir);
        let target = dir.0.join("_rejected");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("tn0010.xmp"), b"other").unwrap();
        let error = move_group(&files, &target).unwrap_err();
        assert_eq!(error.file, files[2]);
        assert_eq!(error.reason, NAME_TAKEN);
        assert!(error.left.is_empty());
        assert_eq!(
            error.to_string(),
            "Could not move TN0010.XMP: the name is taken in the target folder."
        );
        assert!(all_exist(&files));
        // The existing target folder is kept, with its file unchanged.
        assert!(target.is_dir());
        assert_eq!(std::fs::read(target.join("tn0010.xmp")).unwrap(), b"other");
    }

    #[test]
    #[cfg(windows)]
    fn move_group_with_a_file_held_open_rolls_back() {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_WRITE, without FILE_SHARE_DELETE.
        const SHARE_READ_WRITE: u32 = 0x1 | 0x2;

        let dir = TempFolder::new("open");
        let files = group(&dir);
        let target = dir.0.join("_rejected");
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(SHARE_READ_WRITE)
            .open(&files[1])
            .unwrap();
        let error = move_group(&files, &target).unwrap_err();
        drop(held);
        assert_eq!(error.file, files[1]);
        assert_eq!(error.reason, OPEN_ELSEWHERE);
        assert!(error.left.is_empty());
        assert!(error.to_string().contains("TN0010.ARW.dop"));
        assert!(all_exist(&files));
        assert!(!target.exists());
    }

    #[test]
    #[cfg(windows)]
    fn move_group_keeps_an_existing_target_folder() {
        let dir = TempFolder::new("existing");
        let files = group(&dir);
        let target = dir.0.join("_rejected");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("TN0099.ARW"), b"other").unwrap();
        let moved = move_group(&files, &target).unwrap();
        assert_eq!(moved.len(), 3);
        assert!(target.join("TN0099.ARW").is_file());
        assert!(moved.iter().all(|m| m.to.is_file()));
    }

    #[test]
    #[cfg(windows)]
    fn move_back_returns_the_files() {
        let dir = TempFolder::new("back");
        let files = group(&dir);
        let target = dir.0.join("_picked");
        let moved = move_group(&files, &target).unwrap();
        move_back(&moved).unwrap();
        assert!(all_exist(&files));
        assert!(moved.iter().all(|m| !m.to.exists()));
        assert!(target.is_dir());
    }

    #[test]
    #[cfg(windows)]
    fn move_back_with_a_taken_name_moves_nothing() {
        let dir = TempFolder::new("back-taken");
        let files = group(&dir);
        let target = dir.0.join("_rejected");
        let moved = move_group(&files, &target).unwrap();
        std::fs::write(&files[0], b"new").unwrap();
        let error = move_back(&moved).unwrap_err();
        assert_eq!(error.file, moved[0].to);
        assert_eq!(error.reason, NAME_TAKEN);
        assert!(error.left.is_empty());
        assert!(moved.iter().all(|m| m.to.is_file()));
        assert_eq!(std::fs::read(&files[0]).unwrap(), b"new");
    }

    #[test]
    fn move_group_with_two_files_of_the_same_name_moves_nothing() {
        let dir = TempFolder::new("same-name");
        let first = dir.file("TN0011.ARW");
        std::fs::create_dir(dir.0.join("other")).unwrap();
        let second = dir.file("other/tn0011.arw");
        let target = dir.0.join("_rejected");
        let error = move_group(&[first.clone(), second.clone()], &target).unwrap_err();
        assert_eq!(error.file, second);
        assert_eq!(error.reason, NAME_TAKEN);
        assert!(error.left.is_empty());
        assert!(first.is_file() && second.is_file());
        assert!(!target.exists());
    }

    #[test]
    fn move_group_with_one_file_given_twice_moves_nothing() {
        let dir = TempFolder::new("twice");
        let image = dir.file("TN0012.ARW");
        let sidecar = dir.file("TN0012.xmp");
        let target = dir.0.join("_rejected");
        let files = [image.clone(), sidecar.clone(), image.clone()];
        let error = move_group(&files, &target).unwrap_err();
        assert_eq!(error.file, image);
        assert_eq!(error.reason, NAME_TAKEN);
        assert!(image.is_file() && sidecar.is_file());
        assert!(!target.exists());
    }

    #[test]
    fn move_group_into_a_file_names_only_the_cause() {
        let dir = TempFolder::new("not-a-folder");
        let image = dir.file("TN0013.ARW");
        let target = dir.file("_rejected");
        let error = move_group(std::slice::from_ref(&image), &target).unwrap_err();
        assert_eq!(error.file, target);
        assert_eq!(error.reason, NOT_A_FOLDER);
        assert_eq!(
            error.to_string(),
            "Could not move _rejected: it exists and is not a folder."
        );
        assert!(image.is_file() && target.is_file());
    }

    #[test]
    fn move_error_names_the_files_left_where_they_are() {
        let error = MoveError {
            file: PathBuf::from("a").join("TN0014.XMP"),
            reason: OPEN_ELSEWHERE.to_string(),
            left: vec![Moved {
                from: PathBuf::from("a").join("_rejected").join("TN0014.ARW"),
                to: PathBuf::from("a").join("TN0014.ARW"),
            }],
        };
        let text = error.to_string();
        let left = PathBuf::from("a").join("TN0014.ARW");
        assert_eq!(
            text,
            format!(
                "Could not move TN0014.XMP: the file is open in another program. Some files \
                 could not be moved back and are now at: {}.",
                left.display()
            )
        );
        assert!(!text.contains("target folder"));
    }

    #[test]
    #[cfg(windows)]
    fn reason_of_io_names_the_causes_a_user_can_act_on() {
        let reason = |code: i32| reason_of_io(&io::Error::from_raw_os_error(code));
        assert_eq!(reason(32), OPEN_ELSEWHERE);
        assert_eq!(reason(33), OPEN_ELSEWHERE);
        assert_eq!(reason(80), NAME_TAKEN);
        assert_eq!(reason(183), NAME_TAKEN);
        // ERROR_ACCESS_DENIED: the system's own text.
        assert_eq!(reason(5), io::Error::from_raw_os_error(5).to_string());
    }
}
