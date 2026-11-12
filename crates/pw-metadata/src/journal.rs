//! The write journal (ADR 0007 §9.5; I8b, T85).
//!
//! Before the safe write makes its temporary copy, it records the transaction (original, copy,
//! backup) in a journal file and flushes it; after the replace the file is removed. If the program
//! crashes or the power goes in between, [`Journal::recover`] resolves the open entries at the
//! next start, so at most an extra `.pwtmp-*` or `.pwbak-*` file is ever left, and the original is
//! never lost:
//!
//! - original present → the copy and the backup are deleted (the backup can only exist after a
//!   completed replace or a completed second rename);
//! - no original had existed (a new sidecar) → the copy is deleted;
//! - original missing and backup present → the backup is renamed back, the copy deleted;
//! - anything else (original and backup missing) → nothing is deleted, the entry is kept and
//!   reported as [`Recovered::NeedsUser`].
//!
//! Each transaction is one file `<id>.json` in the journal folder.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::fileops;
use crate::{Error, Result};

/// One write transaction, as recorded before the copy is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The file that is replaced (the image, or the sidecar of a RAW).
    pub original: PathBuf,
    /// The temporary copy that is written and verified (`.pwtmp-…`).
    pub copy: PathBuf,
    /// The name the original gets during the replace (`.pwbak-…`).
    pub backup: PathBuf,
    /// Whether the original existed when the transaction began (`false` for a new sidecar).
    pub original_existed: bool,
}

/// The id of an open journal entry, returned by [`Journal::begin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryId(String);

/// What [`Journal::recover`] did with one open entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovered {
    /// The original is in place (or there was none, for a new sidecar): the files listed were
    /// deleted (empty when there was nothing left over). The entry is removed.
    Cleaned { entry: Entry, deleted: Vec<PathBuf> },
    /// The original was missing and its backup present: the backup was renamed back to the
    /// original's name and the files listed (the copy, if it was there) deleted. The entry is
    /// removed.
    Restored { entry: Entry, deleted: Vec<PathBuf> },
    /// The original and its backup are both missing: nothing was deleted (the copy, if present,
    /// may be the only remaining version). The entry is kept for the user.
    NeedsUser { entry: Entry },
    /// A step of the recovery failed; holds the error. The entry is kept and tried again at the
    /// next start.
    Failed { entry: Entry, error: String },
    /// The entry file could not be read or is not a journal entry; it is kept, not deleted.
    Unreadable { file: PathBuf, error: String },
}

/// The journal folder (in the app's data folder).
#[derive(Debug, Clone)]
pub struct Journal {
    folder: PathBuf,
}

impl Journal {
    /// Opens the journal in `folder`, creating the folder if needed.
    pub fn open(folder: &Path) -> Result<Journal> {
        std::fs::create_dir_all(folder)?;
        Ok(Journal {
            folder: folder.to_path_buf(),
        })
    }

    /// The journal folder.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Records `entry` in a new file `<id>.json` and flushes it to disk. Call this before the copy
    /// is made (§9.5).
    pub fn begin(&self, entry: &Entry) -> Result<EntryId> {
        let text = entry_to_json(entry)?;
        loop {
            let id = format!("{:016x}", fileops::unique_number());
            let path = self.entry_path(&id);
            let mut file = match File::options().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Error::Io(e)),
            };
            let written = file
                .write_all(text.as_bytes())
                .and_then(|()| file.sync_all());
            if let Err(e) = written {
                drop(file);
                let _ = std::fs::remove_file(&path);
                return Err(Error::Io(e));
            }
            return Ok(EntryId(id));
        }
    }

    /// Removes the entry: the transaction is complete.
    pub fn end(&self, id: EntryId) -> Result<()> {
        std::fs::remove_file(self.entry_path(&id.0))?;
        Ok(())
    }

    /// Resolves every open entry (§9.5) and says what was done with each, in the order of the
    /// entry file names. Fails only if the journal folder cannot be read.
    pub fn recover(&self) -> Result<Vec<Recovered>> {
        let mut files = Vec::new();
        for item in std::fs::read_dir(&self.folder)? {
            let path = item?.path();
            if path.extension().is_some_and(|e| e == "json") && path.is_file() {
                files.push(path);
            }
        }
        files.sort();
        Ok(files.iter().map(|file| recover_one(file)).collect())
    }

    fn entry_path(&self, id: &str) -> PathBuf {
        self.folder.join(format!("{id}.json"))
    }
}

/// Resolves one entry file.
fn recover_one(file: &Path) -> Recovered {
    let entry = match std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| entry_from_json(&text))
    {
        Ok(entry) => entry,
        Err(error) => {
            return Recovered::Unreadable {
                file: file.to_path_buf(),
                error,
            };
        }
    };
    match resolve(&entry) {
        Ok(Some(done)) => match std::fs::remove_file(file) {
            Ok(()) => done,
            Err(e) => Recovered::Failed {
                entry,
                error: format!("could not remove the journal entry {}: {e}", file.display()),
            },
        },
        Ok(None) => Recovered::NeedsUser { entry },
        Err(error) => Recovered::Failed { entry, error },
    }
}

/// Does what §9.5 says for `entry`. `Ok(None)` means nothing may be done: the user has to decide.
fn resolve(entry: &Entry) -> std::result::Result<Option<Recovered>, String> {
    let original_present = exists(&entry.original)?;
    if original_present {
        let mut deleted = Vec::new();
        delete_if_present(&entry.copy, &mut deleted)?;
        delete_if_present(&entry.backup, &mut deleted)?;
        return Ok(Some(Recovered::Cleaned {
            entry: entry.clone(),
            deleted,
        }));
    }
    if !entry.original_existed {
        let mut deleted = Vec::new();
        delete_if_present(&entry.copy, &mut deleted)?;
        return Ok(Some(Recovered::Cleaned {
            entry: entry.clone(),
            deleted,
        }));
    }
    if exists(&entry.backup)? {
        fileops::move_new(&entry.backup, &entry.original).map_err(|e| {
            format!(
                "could not rename {} back to {}: {e}",
                entry.backup.display(),
                entry.original.display()
            )
        })?;
        let mut deleted = Vec::new();
        delete_if_present(&entry.copy, &mut deleted)?;
        return Ok(Some(Recovered::Restored {
            entry: entry.clone(),
            deleted,
        }));
    }
    Ok(None)
}

fn exists(path: &Path) -> std::result::Result<bool, String> {
    path.try_exists()
        .map_err(|e| format!("could not check {}: {e}", path.display()))
}

fn delete_if_present(path: &Path, deleted: &mut Vec<PathBuf>) -> std::result::Result<(), String> {
    if exists(path)? {
        std::fs::remove_file(path)
            .map_err(|e| format!("could not delete {}: {e}", path.display()))?;
        deleted.push(path.to_path_buf());
    }
    Ok(())
}

fn entry_to_json(entry: &Entry) -> Result<String> {
    let text = |path: &Path| {
        path.to_str().map(str::to_owned).ok_or_else(|| {
            Error::Unsupported(format!(
                "the path is not valid Unicode and cannot be journalled: {}",
                path.display()
            ))
        })
    };
    let value = json!({
        "original": text(&entry.original)?,
        "copy": text(&entry.copy)?,
        "backup": text(&entry.backup)?,
        "original_existed": entry.original_existed,
    });
    Ok(value.to_string())
}

fn entry_from_json(text: &str) -> std::result::Result<Entry, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let path = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| format!("no text \"{key}\" in the entry"))
    };
    Ok(Entry {
        original: path("original")?,
        copy: path("copy")?,
        backup: path("backup")?,
        original_existed: value
            .get("original_existed")
            .and_then(Value::as_bool)
            .ok_or_else(|| "no true/false \"original_existed\" in the entry".to_owned())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temporary folder, removed when dropped.
    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pw-metadata-journal-{name}-{}-{:016x}",
                std::process::id(),
                fileops::unique_number()
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempFolder(path)
        }

        fn file(&self, name: &str, content: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, content).unwrap();
            path
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn entry(folder: &TempFolder, original_existed: bool) -> Entry {
        Entry {
            original: folder.path("a.jpg"),
            copy: folder.path("a.pwtmp-1.jpg"),
            backup: folder.path("a.pwbak-1.jpg"),
            original_existed,
        }
    }

    fn journal_files(journal: &Journal) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(journal.folder())
            .unwrap()
            .map(|item| item.unwrap().path())
            .collect();
        files.sort();
        files
    }

    #[test]
    fn open_creates_the_folder() {
        let folder = TempFolder::new("open");
        let journal_folder = folder.path("data").join("journal");
        let journal = Journal::open(&journal_folder).unwrap();
        assert!(journal_folder.is_dir());
        assert_eq!(journal.folder(), journal_folder);
    }

    #[test]
    fn begin_writes_an_entry_file_and_end_removes_it() {
        let folder = TempFolder::new("begin-end");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let entry = entry(&folder, true);

        let id = journal.begin(&entry).unwrap();

        let files = journal_files(&journal);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].extension().and_then(|e| e.to_str()), Some("json"));
        let text = std::fs::read_to_string(&files[0]).unwrap();
        assert_eq!(entry_from_json(&text).unwrap(), entry);

        journal.end(id).unwrap();
        assert!(journal_files(&journal).is_empty());
    }

    #[test]
    fn two_entries_get_two_files() {
        let folder = TempFolder::new("two");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let first = journal.begin(&entry(&folder, true)).unwrap();
        let second = journal.begin(&entry(&folder, false)).unwrap();
        assert_ne!(first, second);
        assert_eq!(journal_files(&journal).len(), 2);
    }

    #[test]
    fn recover_with_the_original_present_deletes_the_copy_and_the_backup() {
        let folder = TempFolder::new("recover-present");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let entry = entry(&folder, true);
        folder.file("a.jpg", b"new content");
        folder.file("a.pwtmp-1.jpg", b"copy");
        folder.file("a.pwbak-1.jpg", b"old content");
        journal.begin(&entry).unwrap();

        let recovered = journal.recover().unwrap();

        assert_eq!(
            recovered,
            vec![Recovered::Cleaned {
                entry: entry.clone(),
                deleted: vec![entry.copy.clone(), entry.backup.clone()],
            }]
        );
        assert_eq!(std::fs::read(&entry.original).unwrap(), b"new content");
        assert!(!entry.copy.exists());
        assert!(!entry.backup.exists());
        assert!(journal_files(&journal).is_empty());
    }

    #[test]
    fn recover_with_the_original_present_and_nothing_left_over_deletes_nothing() {
        let folder = TempFolder::new("recover-clean");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let entry = entry(&folder, true);
        folder.file("a.jpg", b"content");
        journal.begin(&entry).unwrap();

        let recovered = journal.recover().unwrap();

        assert_eq!(
            recovered,
            vec![Recovered::Cleaned {
                entry: entry.clone(),
                deleted: vec![],
            }]
        );
        assert!(entry.original.exists());
        assert!(journal_files(&journal).is_empty());
    }

    #[test]
    #[cfg(windows)]
    fn recover_with_the_original_missing_and_the_backup_present_renames_it_back() {
        let folder = TempFolder::new("recover-restore");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let entry = entry(&folder, true);
        folder.file("a.pwtmp-1.jpg", b"copy");
        folder.file("a.pwbak-1.jpg", b"old content");
        journal.begin(&entry).unwrap();

        let recovered = journal.recover().unwrap();

        assert_eq!(
            recovered,
            vec![Recovered::Restored {
                entry: entry.clone(),
                deleted: vec![entry.copy.clone()],
            }]
        );
        assert_eq!(std::fs::read(&entry.original).unwrap(), b"old content");
        assert!(!entry.copy.exists());
        assert!(!entry.backup.exists());
        assert!(journal_files(&journal).is_empty());
    }

    #[test]
    fn recover_of_a_new_sidecar_deletes_the_copy() {
        let folder = TempFolder::new("recover-new");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let entry = entry(&folder, false);
        folder.file("a.pwtmp-1.jpg", b"copy");
        journal.begin(&entry).unwrap();

        let recovered = journal.recover().unwrap();

        assert_eq!(
            recovered,
            vec![Recovered::Cleaned {
                entry: entry.clone(),
                deleted: vec![entry.copy.clone()],
            }]
        );
        assert!(!entry.copy.exists());
        assert!(!entry.original.exists());
        assert!(journal_files(&journal).is_empty());
    }

    #[test]
    fn recover_with_the_original_and_the_backup_missing_keeps_everything_for_the_user() {
        let folder = TempFolder::new("recover-user");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let entry = entry(&folder, true);
        folder.file("a.pwtmp-1.jpg", b"copy");
        journal.begin(&entry).unwrap();

        let recovered = journal.recover().unwrap();

        assert_eq!(
            recovered,
            vec![Recovered::NeedsUser {
                entry: entry.clone()
            }]
        );
        assert_eq!(std::fs::read(&entry.copy).unwrap(), b"copy");
        assert_eq!(journal_files(&journal).len(), 1);
        // A second start reports it again.
        assert_eq!(
            journal.recover().unwrap(),
            vec![Recovered::NeedsUser { entry }]
        );
    }

    #[test]
    fn recover_reports_an_unreadable_entry_file_and_keeps_it() {
        let folder = TempFolder::new("recover-unreadable");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        let bad = journal.folder().join("0000000000000000.json");
        std::fs::write(&bad, b"not a journal entry").unwrap();
        let incomplete = journal.folder().join("0000000000000001.json");
        std::fs::write(&incomplete, br#"{"original": "a.jpg"}"#).unwrap();

        let recovered = journal.recover().unwrap();

        assert_eq!(recovered.len(), 2);
        assert!(
            matches!(&recovered[0], Recovered::Unreadable { file, .. } if *file == bad),
            "{recovered:?}"
        );
        assert!(
            matches!(&recovered[1], Recovered::Unreadable { file, .. } if *file == incomplete),
            "{recovered:?}"
        );
        assert!(bad.exists());
        assert!(incomplete.exists());
    }

    #[test]
    fn recover_ignores_files_that_are_not_entries() {
        let folder = TempFolder::new("recover-other");
        let journal = Journal::open(&folder.path("journal")).unwrap();
        std::fs::write(journal.folder().join("notes.txt"), b"x").unwrap();
        assert!(journal.recover().unwrap().is_empty());
    }
}
