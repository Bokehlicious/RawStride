//! The safe write transaction (ADR 0007 §9, §4.2; I8c, T86).
//!
//! [`ExifTool::write_safely`] writes a [`WritePlan`] so that the file holding the metadata is
//! either replaced by a verified copy or left exactly as it was:
//!
//! - **Target** (§4.2): the image for a JPEG, TIFF or DNG; for a manufacturer RAW its sidecar
//!   `<basename>.xmp`. The RAW file itself is never written.
//! - **9.1, before writing:** the caller's [`ImageCheck::release`] closes RAW engine sessions for
//!   the image; the target must be writable (not read-only, not locked, not on a read-only volume,
//!   enough free space; for a new sidecar only the volume and the free space); an existing target
//!   is recorded with [`ExifTool::snapshot`] and identified by content ([`check_file_type`]).
//! - **Journal** (§9.5): the transaction (target, copy, backup) is recorded before the copy is
//!   made and removed when it is complete.
//! - **9.2, the copy:** one ExifTool call writes the plan to `<name>.pwtmp-<hex>.<ext>` with `-o`;
//!   the target is only read. A new sidecar is made without a source file (measured: `-o x.xmp`
//!   from a RAW copies the RAW's Exif into the sidecar). ExifTool must report one file created
//!   and no warning.
//! - **9.3, verification:** the copy's snapshot against the original's ([`verify_copy`]); a new
//!   sidecar must be XMP and hold, besides `System`, `ExifTool` and `File` keys, only keys
//!   [`verify_copy`] allows for the plan (owner's decision during T86: ExifTool always adds
//!   `XMP-x:XMPToolkit`); then the caller's [`ImageCheck::check_copy`] for a JPEG, TIFF or DNG
//!   (previews and the RAW image, §9.3 points 3–4, written in I9); then the original's
//!   modification time is set on the copy and the copy is flushed.
//! - **9.4, replace:** `ReplaceFileW` with the backup `<name>.pwbak-<hex>.<ext>` (or two renames
//!   on volumes without it), or a move without overwriting for a new sidecar; then a final read
//!   (size and `ImageDataHash` equal the copy's), the backup is deleted and the journal entry
//!   ended.
//! - **Failures** ([`SafeWriteError`]) leave no `pwtmp` file. When the original is intact the
//!   journal entry is ended. When the original is under the backup name, the backup is renamed
//!   back once; if that fails the entry is kept for [`Journal::recover`]. A failed final check
//!   keeps the backup and ends the entry, so the next recovery does not delete the backup
//!   (owner's decision during T86).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::fileops::{self, ReplaceError};
use crate::journal::{Entry, EntryId, Journal};
use crate::reconcile::FileKind;
use crate::verify::{FileSnapshot, VerifyError, check_file_type, verify_copy};
use crate::write_plan::WritePlan;
use crate::{ExifTool, sidecar_path};

/// Groups whose keys a new sidecar has without the plan writing them: the file system, ExifTool's
/// own data and the file's type.
const NEW_SIDECAR_GROUPS: &[&str] = &["System", "ExifTool", "File"];
/// Bytes reserved for a new sidecar besides the plan's arguments: room for the XMP packet around
/// the values (a sidecar with one City is 380 bytes, measured with ExifTool 13.59).
const NEW_SIDECAR_RESERVE: u64 = 4096;

/// The checks of the image the transaction cannot make itself (§9.1 point 2, §9.3 points 3–4):
/// they need the RAW engine and LibRaw, which `pw-metadata` does not depend on. The real one is
/// written in I9.
pub trait ImageCheck {
    /// Closes every RAW engine session for `image` (§9.1 point 2), before the file is checked.
    fn release(&self, image: &Path);

    /// Checks the written copy of `original` (§9.3 points 3–4: the same previews, the RAW image
    /// still opens). Called only for a JPEG, TIFF or DNG, never for a sidecar. An error is the
    /// reason shown to the user.
    fn check_copy(&self, original: &Path, copy: &Path, kind: FileKind) -> Result<(), String>;
}

/// An [`ImageCheck`] that does nothing (for tests and sidecars).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoImageCheck;

impl ImageCheck for NoImageCheck {
    fn release(&self, _image: &Path) {}

    fn check_copy(&self, _original: &Path, _copy: &Path, _kind: FileKind) -> Result<(), String> {
        Ok(())
    }
}

/// What a successful [`ExifTool::write_safely`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteReport {
    /// The target (the image, or the RAW's sidecar) holds the plan's values.
    Written { target: PathBuf },
    /// The plan was empty; nothing was written.
    Unchanged,
}

/// The step of the transaction that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// §9.1: the file kind, the write checks, the snapshot of the original or its file type.
    Precheck,
    /// §9.2: the journal entry or ExifTool's write of the temporary copy.
    WriteCopy,
    /// §9.3 points 1–3 and 5: the copy's snapshot and its comparison with the original; setting
    /// the modification time and the flush.
    Verify,
    /// §9.3 points 3–4: the caller's [`ImageCheck::check_copy`].
    ImageCheck,
    /// §9.4: replacing the original with the copy (or moving a new sidecar into place).
    Replace,
    /// §9.4: the final read of the replaced file.
    FinalCheck,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Stage::Precheck => "the check before writing",
            Stage::WriteCopy => "writing the temporary copy",
            Stage::Verify => "the verification of the copy",
            Stage::ImageCheck => "the image check of the copy",
            Stage::Replace => "replacing the original",
            Stage::FinalCheck => "the final check of the written file",
        })
    }
}

/// Where the original is after a failed transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalState {
    /// The original is unchanged under its own name (for a new sidecar: none was made).
    Intact,
    /// The original is under this backup name and could not be renamed back; the journal entry is
    /// kept, so [`Journal::recover`] tries again at the next start.
    AtBackup(PathBuf),
    /// The original was replaced by the copy; the previous version is kept as the backup named in
    /// the reason.
    Replaced,
}

/// Why [`ExifTool::write_safely`] failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeWriteError {
    /// The file that was to be written: the image, or the RAW's sidecar (added by the owner's
    /// decision during T86, so the message can name it).
    pub file: PathBuf,
    pub stage: Stage,
    pub original: OriginalState,
    pub reason: String,
}

impl fmt::Display for SafeWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "metadata could not be written safely to {}: {} failed: {}; ",
            self.file.display(),
            self.stage,
            self.reason
        )?;
        match &self.original {
            OriginalState::Intact => write!(f, "the original is unchanged"),
            OriginalState::AtBackup(backup) => write!(
                f,
                "the original could not be put back and is now {}",
                backup.display()
            ),
            OriginalState::Replaced => write!(f, "the original has been replaced"),
        }
    }
}

impl std::error::Error for SafeWriteError {}

impl ExifTool {
    /// Writes `plan` to `image`'s metadata file in one safe transaction (see the module comment).
    /// `checks` are the caller's image checks; `journal` is the app's write journal.
    pub fn write_safely(
        &mut self,
        image: &Path,
        plan: &WritePlan,
        checks: &dyn ImageCheck,
        journal: &Journal,
    ) -> Result<WriteReport, SafeWriteError> {
        if plan.args.is_empty() {
            return Ok(WriteReport::Unchanged);
        }
        let Some(kind) = file_kind(image) else {
            return Err(SafeWriteError {
                file: image.to_path_buf(),
                stage: Stage::Precheck,
                original: OriginalState::Intact,
                reason: "the file is not a JPEG, TIFF, DNG or manufacturer RAW".to_string(),
            });
        };
        let target = if kind == FileKind::Raw {
            sidecar_path(image)
        } else {
            image.to_path_buf()
        };
        Transaction {
            et: self,
            target: &target,
            plan,
            journal,
        }
        .run(image, kind, checks)
    }
}

/// The kind of file by its extension (the lists in `lib.rs`, as the reader decides it).
fn file_kind(image: &Path) -> Option<FileKind> {
    if crate::supports_sidecar(image) {
        return Some(FileKind::Raw);
    }
    let extension = image
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("dng") => Some(FileKind::Dng),
        Some("jpg" | "jpeg") => Some(FileKind::Jpeg),
        Some("tif" | "tiff") => Some(FileKind::Tiff),
        _ => None,
    }
}

/// One write of one target.
struct Transaction<'a> {
    et: &'a mut ExifTool,
    target: &'a Path,
    plan: &'a WritePlan,
    journal: &'a Journal,
}

/// The names and journal entry of an open transaction.
struct Open {
    copy: PathBuf,
    backup: PathBuf,
    id: EntryId,
}

impl Transaction<'_> {
    fn fail(&self, stage: Stage, original: OriginalState, reason: String) -> SafeWriteError {
        SafeWriteError {
            file: self.target.to_path_buf(),
            stage,
            original,
            reason,
        }
    }

    fn precheck(&self, reason: String) -> SafeWriteError {
        self.fail(Stage::Precheck, OriginalState::Intact, reason)
    }

    fn run(
        &mut self,
        image: &Path,
        kind: FileKind,
        checks: &dyn ImageCheck,
    ) -> Result<WriteReport, SafeWriteError> {
        // 9.1
        checks.release(image);
        let exists = self
            .target
            .try_exists()
            .map_err(|e| self.precheck(format!("the file could not be checked: {e}")))?;
        let plan_bytes: u64 = self
            .plan
            .options
            .iter()
            .chain(&self.plan.args)
            .map(|arg| arg.len() as u64)
            .sum();
        let original = if exists {
            if let Err(refusal) = fileops::can_write(self.target, plan_bytes) {
                return Err(self.precheck(refusal.to_string()));
            }
            let snapshot = match self.et.snapshot(self.target) {
                Ok(snapshot) => snapshot,
                Err(e) => return Err(self.precheck(e.to_string())),
            };
            if let Err(e) = check_file_type(kind, &snapshot) {
                return Err(self.precheck(e.to_string()));
            }
            Some(snapshot)
        } else if kind == FileKind::Raw {
            if let Err(reason) = self.check_new_sidecar(plan_bytes) {
                return Err(self.precheck(reason));
            }
            None
        } else {
            return Err(self.precheck("the file does not exist".to_string()));
        };

        // The journal entry, before the copy is made (§9.5).
        let copy = fileops::temp_name(self.target, "pwtmp");
        let backup = fileops::temp_name(self.target, "pwbak");
        let begun = self.journal.begin(&Entry {
            original: self.target.to_path_buf(),
            copy: copy.clone(),
            backup: backup.clone(),
            original_existed: original.is_some(),
        });
        let id = match begun {
            Ok(id) => id,
            Err(e) => {
                return Err(self.fail(
                    Stage::WriteCopy,
                    OriginalState::Intact,
                    format!("the write journal could not be written: {e}"),
                ));
            }
        };
        let open = Open { copy, backup, id };

        // 9.2 and 9.3
        let written = match self.write_and_verify(&open, original.as_ref(), kind, checks) {
            Ok(written) => written,
            Err((stage, reason)) => return Err(self.abandon(open, stage, reason)),
        };

        // 9.4
        self.replace(open, original.is_some(), &written)
    }

    /// For a new sidecar (§9.1 point 2): its volume is writable and has room for it (the plan's
    /// arguments + 10 % + a reserve for the XMP packet).
    fn check_new_sidecar(&self, plan_bytes: u64) -> Result<(), String> {
        let folder = match self.target.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let unchecked = |e: crate::Error| format!("the folder could not be checked: {e}");
        if fileops::is_read_only_volume(&folder).map_err(unchecked)? {
            return Err(fileops::Refusal::ReadOnlyVolume.to_string());
        }
        let needed = plan_bytes
            .saturating_add(plan_bytes.div_ceil(10))
            .saturating_add(NEW_SIDECAR_RESERVE);
        let free = fileops::free_space(&folder).map_err(unchecked)?;
        if free < needed {
            return Err(fileops::Refusal::NoSpace { needed, free }.to_string());
        }
        Ok(())
    }

    /// Writes the copy (§9.2) and verifies it (§9.3); sets the original's modification time on it
    /// and flushes it. Returns the copy's snapshot for the final check.
    fn write_and_verify(
        &mut self,
        open: &Open,
        original: Option<&FileSnapshot>,
        kind: FileKind,
        checks: &dyn ImageCheck,
    ) -> Result<FileSnapshot, (Stage, String)> {
        let mut args = self.plan.options.clone();
        args.extend(self.plan.args.iter().cloned());
        args.push("-o".to_string());
        args.push(open.copy.to_string_lossy().into_owned());
        // A new sidecar is made from the arguments alone, without a source file.
        if original.is_some() {
            args.push(self.target.to_string_lossy().into_owned());
        }
        let answer = self
            .et
            .execute(&args)
            .map_err(|e| (Stage::WriteCopy, e.to_string()))?;
        if !copy_written(&answer) || !open.copy.is_file() {
            return Err((
                Stage::WriteCopy,
                format!("ExifTool did not write the copy: {}", answer.trim()),
            ));
        }

        let verify = |reason: String| (Stage::Verify, reason);
        let written = self
            .et
            .snapshot(&open.copy)
            .map_err(|e| verify(e.to_string()))?;
        match original {
            Some(original) => {
                verify_copy(original, &written, self.plan).map_err(|e| verify(e.to_string()))?;
                // Never for a sidecar: the RAW's sidecar is not an image (reviewer's fix).
                if kind != FileKind::Raw {
                    checks
                        .check_copy(self.target, &open.copy, kind)
                        .map_err(|reason| (Stage::ImageCheck, reason))?;
                }
                fileops::set_modified(&open.copy, original.modified)
                    .map_err(|e| verify(format!("the modification time could not be set: {e}")))?;
            }
            None => verify_new_sidecar(&written, self.plan).map_err(|e| verify(e.to_string()))?,
        }
        fileops::flush(&open.copy)
            .map_err(|e| verify(format!("the copy could not be flushed: {e}")))?;
        Ok(written)
    }

    /// Replaces the original with the verified copy, or moves a new sidecar into place (§9.4);
    /// then the final check, the backup deleted and the journal entry ended.
    fn replace(
        &mut self,
        open: Open,
        original_existed: bool,
        written: &FileSnapshot,
    ) -> Result<WriteReport, SafeWriteError> {
        let replaced = if original_existed {
            fileops::replace(self.target, &open.copy, &open.backup)
        } else {
            fileops::move_new(&open.copy, self.target)
                .map_err(|e| ReplaceError::OriginalIntact(e.to_string()))
        };
        match replaced {
            Ok(()) => {}
            Err(ReplaceError::OriginalIntact(reason)) => {
                return Err(self.abandon(open, Stage::Replace, reason));
            }
            Err(ReplaceError::OriginalAtBackup(reason)) => {
                return Err(self.put_back(open, reason));
            }
        }

        if let Err(reason) = self.final_check(written) {
            let reason = if original_existed {
                format!(
                    "{reason}; the previous version is kept as {}",
                    open.backup.display()
                )
            } else {
                reason
            };
            let note = self.journal.end(open.id).err().map(journal_note);
            return Err(self.fail(
                Stage::FinalCheck,
                OriginalState::Replaced,
                with_note(reason, note),
            ));
        }

        // Done. If the backup cannot be deleted the entry stays open, so the next recovery deletes
        // it (the original is in place); the write itself has succeeded.
        if remove_if_present(&open.backup).is_ok() {
            let _ = self.journal.end(open.id);
        }
        Ok(WriteReport::Written {
            target: self.target.to_path_buf(),
        })
    }

    /// The final read (§9.4): the target's size and image data hash are the verified copy's.
    fn final_check(&mut self, written: &FileSnapshot) -> Result<(), String> {
        let now = self
            .et
            .snapshot(self.target)
            .map_err(|e| format!("the written file could not be read: {e}"))?;
        if now.size != written.size {
            return Err(format!(
                "the written file has {} bytes, the verified copy had {}",
                now.size, written.size
            ));
        }
        if now.image_data_hash != written.image_data_hash {
            return Err(
                "the image data hash of the written file differs from the verified copy's"
                    .to_string(),
            );
        }
        Ok(())
    }

    /// A failure with the original intact (§9.5): the copy (and a backup, if one was made) is
    /// deleted and the journal entry ended. If a file cannot be deleted the entry is kept, so the
    /// next recovery deletes it.
    fn abandon(&self, open: Open, stage: Stage, reason: String) -> SafeWriteError {
        let removed = remove_if_present(&open.copy).and_then(|()| remove_if_present(&open.backup));
        let note = match removed {
            Ok(()) => self.journal.end(open.id).err().map(journal_note),
            Err(e) => Some(format!("{e}; it is deleted at the next start")),
        };
        self.fail(stage, OriginalState::Intact, with_note(reason, note))
    }

    /// A replace that left the original under the backup name (§9.5): the backup is renamed back
    /// once. If that works the original is intact; otherwise the journal entry is kept and the
    /// error names the backup. The copy is deleted in both cases (the original is the backup).
    fn put_back(&self, open: Open, reason: String) -> SafeWriteError {
        match fileops::move_new(&open.backup, self.target) {
            Ok(()) => self.abandon(open, Stage::Replace, reason),
            Err(e) => {
                let reason = format!("{reason}; renaming it back failed: {e}");
                let note = remove_if_present(&open.copy).err();
                self.fail(
                    Stage::Replace,
                    OriginalState::AtBackup(open.backup),
                    with_note(reason, note),
                )
            }
        }
    }
}

/// True if ExifTool's answer to the write of the copy reports exactly one file created, no failed
/// file and no warning.
fn copy_written(answer: &str) -> bool {
    let created: u64 = answer
        .lines()
        .filter_map(|line| line.trim().strip_suffix(" image files created"))
        .filter_map(|count| count.trim().parse::<u64>().ok())
        .sum();
    let warned = answer
        .lines()
        .any(|line| line.trim_start().starts_with("Warning"));
    crate::write_succeeded(answer) && created == 1 && !warned
}

/// The check of a new sidecar (§9.3, owner's decision during T86): it is XMP, and besides its
/// `System`, `ExifTool` and `File` keys it holds only keys [`verify_copy`] allows for the plan. It
/// is compared with an "original" that has the same type, hash and validation counts and only
/// those keys.
fn verify_new_sidecar(written: &FileSnapshot, plan: &WritePlan) -> Result<(), VerifyError> {
    check_file_type(FileKind::Raw, written)?;
    let empty = FileSnapshot {
        listing: written
            .listing
            .iter()
            .filter(|(key, _)| {
                key.split_once(':')
                    .is_some_and(|(group, _)| NEW_SIDECAR_GROUPS.contains(&group))
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        ..written.clone()
    };
    verify_copy(&empty, written, plan)
}

/// Deletes `path` if it exists.
fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{} could not be deleted: {e}", path.display())),
    }
}

fn journal_note(e: crate::Error) -> String {
    format!("the journal entry could not be removed: {e}")
}

fn with_note(reason: String, note: Option<String>) -> String {
    match note {
        Some(note) => format!("{reason} ({note})"),
        None => reason,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::SystemTime;

    use super::*;

    fn sidecar(listing: &[(&str, &str)]) -> FileSnapshot {
        FileSnapshot {
            file_type: "XMP".to_string(),
            size: 380,
            modified: SystemTime::UNIX_EPOCH,
            image_data_hash: None,
            validate: None,
            listing: listing
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn city_plan() -> WritePlan {
        WritePlan {
            args: vec!["-XMP-photoshop:City=Lund".to_string()],
            ..WritePlan::default()
        }
    }

    #[test]
    fn the_file_kind_comes_from_the_extension() {
        assert_eq!(file_kind(Path::new("a.JPG")), Some(FileKind::Jpeg));
        assert_eq!(file_kind(Path::new("a.jpeg")), Some(FileKind::Jpeg));
        assert_eq!(file_kind(Path::new("a.tif")), Some(FileKind::Tiff));
        assert_eq!(file_kind(Path::new("a.Dng")), Some(FileKind::Dng));
        assert_eq!(file_kind(Path::new("a.ARW")), Some(FileKind::Raw));
        assert_eq!(file_kind(Path::new("a.rw2")), Some(FileKind::Raw));
        assert_eq!(file_kind(Path::new("a.png")), None);
        assert_eq!(file_kind(Path::new("a")), None);
    }

    #[test]
    fn one_file_created_without_a_warning_is_a_written_copy() {
        assert!(copy_written("    1 image files created\n"));
    }

    #[test]
    fn a_copied_file_a_warning_or_an_error_is_not_a_written_copy() {
        // Measured with ExifTool 13.59: an unknown tag copies the file and warns.
        assert!(!copy_written(
            "    1 image files copied\n--- stderr ---\n\
             Warning: Tag 'XMP-photoshop:NoSuchTag' is not defined\n"
        ));
        assert!(!copy_written(
            "    1 image files created\n--- stderr ---\n\
             Warning: [Minor] IPTC:City exceeds length limit (truncated) - a.jpg\n"
        ));
        assert!(!copy_written(
            "    0 image files updated\n    1 files weren't updated due to errors\n\
             --- stderr ---\nError: 'a.pwtmp-1.jpg' already exists - a.jpg\n"
        ));
        assert!(!copy_written("    1 image files updated\n"));
        assert!(!copy_written("    2 image files created\n"));
        assert!(!copy_written(""));
    }

    /// Measured with ExifTool 13.59: a sidecar made from `-XMP-photoshop:City=Lund -o new.xmp`.
    #[test]
    fn a_new_sidecar_with_the_plan_and_exiftools_own_keys_passes() {
        let written = sidecar(&[
            ("ExifTool:ExifToolVersion", "13.59"),
            ("System:FileName", "new.xmp"),
            ("System:FileSize", "380"),
            ("File:FileType", "XMP"),
            ("File:MIMEType", "application/rdf+xml"),
            ("XMP-x:XMPToolkit", "Image::ExifTool 13.59"),
            ("XMP-photoshop:City", "Lund"),
        ]);
        assert_eq!(verify_new_sidecar(&written, &city_plan()), Ok(()));
    }

    #[test]
    fn a_new_sidecar_with_a_key_outside_the_plan_or_of_another_type_fails() {
        let written = sidecar(&[
            ("File:FileType", "XMP"),
            ("XMP-photoshop:City", "Lund"),
            ("XMP-exif:ExposureTime", "0.01"),
        ]);
        assert_eq!(
            verify_new_sidecar(&written, &city_plan()),
            Err(VerifyError::Unexpected {
                key: "XMP-exif:ExposureTime".to_string(),
                before: None,
                after: Some("0.01".to_string()),
            })
        );
        let jpeg = FileSnapshot {
            file_type: "JPEG".to_string(),
            ..sidecar(&[("XMP-photoshop:City", "Lund")])
        };
        assert_eq!(
            verify_new_sidecar(&jpeg, &city_plan()),
            Err(VerifyError::WrongFileType {
                expected: FileKind::Raw,
                found: "JPEG".to_string(),
            })
        );
    }

    #[test]
    fn the_error_names_the_file_the_stage_the_reason_and_the_original() {
        let error = SafeWriteError {
            file: PathBuf::from("C:/bilder/a.jpg"),
            stage: Stage::Verify,
            original: OriginalState::Intact,
            reason: "metadata check failed".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "metadata could not be written safely to C:/bilder/a.jpg: the verification of the \
             copy failed: metadata check failed; the original is unchanged"
        );
        let at_backup = SafeWriteError {
            original: OriginalState::AtBackup(PathBuf::from("C:/bilder/a.pwbak-1.jpg")),
            stage: Stage::Replace,
            ..error.clone()
        };
        assert!(
            at_backup.to_string().ends_with(
                "replacing the original failed: metadata check failed; the original could not \
                 be put back and is now C:/bilder/a.pwbak-1.jpg"
            ),
            "{at_backup}"
        );
        let replaced = SafeWriteError {
            original: OriginalState::Replaced,
            stage: Stage::FinalCheck,
            ..error
        };
        assert!(
            replaced.to_string().ends_with(
                "the final check of the written file failed: metadata check failed; the \
                 original has been replaced"
            ),
            "{replaced}"
        );
    }

    #[test]
    fn an_unknown_file_kind_is_refused_before_anything_is_done() {
        let mut et = crate::test_support::start_fake("normal");
        let journal_dir = std::env::temp_dir().join(format!(
            "pw-metadata-safe-write-unit-{}",
            std::process::id()
        ));
        let journal = Journal::open(&journal_dir).unwrap();
        let error = et
            .write_safely(
                Path::new("does-not-exist.png"),
                &city_plan(),
                &NoImageCheck,
                &journal,
            )
            .unwrap_err();
        assert_eq!(error.stage, Stage::Precheck);
        assert_eq!(error.original, OriginalState::Intact);
        assert_eq!(std::fs::read_dir(&journal_dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&journal_dir);
    }
}
