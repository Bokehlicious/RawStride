//! The texts of the import (ADR 0010, T116, rule 13): the dialog, its preview and progress, the
//! finished import with the template's notices, and the messages of the import worker. The
//! drawing code in `main.rs` holds none of them.

use crate::editor::notice_text;
use crate::{ImportDone, ItemAction, Preview, PreviewLine};
use pw_core::format_bytes;
use pw_metadata::write_plan::WriteNotice;
use pw_metadata::{MIN_EXIFTOOL_VERSION, ToolVersion};
use std::path::Path;

/// A fixed text of the import dialog and of the import's messages (moved from `main.rs`,
/// `import_dialog.rs` and `import_worker.rs`, T116).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportText {
    /// The dialog's title.
    Title,
    /// The source folder's label.
    Source,
    /// The destination folder's label.
    Destination,
    /// The backup check box.
    Backup,
    /// The button that opens a folder dialog.
    Choose,
    /// The folder dialog's title for the source.
    ChooseSource,
    /// The folder dialog's title for the destination.
    ChooseDestination,
    /// The folder dialog's title for the backup.
    ChooseBackup,
    /// The folder template's label.
    FolderTemplate,
    /// The file-name template's label.
    NameTemplate,
    /// The job's label.
    Job,
    /// The first sequence number's label.
    FirstNumber,
    /// The list of the templates' variables.
    Variables,
    /// The check box that ejects the card after the import.
    Eject,
    /// The preview's label.
    Preview,
    /// The button that scans the source again.
    Rescan,
    /// Shown while the preview is made.
    Previewing,
    /// The progress bar's text before the first progress.
    Importing,
    /// The button that cancels a running import.
    Cancel,
    /// The button that hides the dialog while the import runs.
    Hide,
    /// The button that starts a new import after one finished.
    NewImport,
    /// The button that closes the dialog.
    Close,
    /// The button that starts the import.
    Start,
    /// The start button when nothing is copied and the run only adds the files at the
    /// destination already to the card's register (T166).
    MarkCard,
    /// The import worker's queue is full.
    QueueFull,
    /// The import worker has stopped (a request could not be sent).
    WorkerStopped,
    /// The preview or the import ended because the worker stopped (`import_dialog.rs`).
    StoppedMessage,
    /// The card was not ejected because the import was not complete.
    EjectSkipped,
    /// The check box that copies new images only (T158).
    Incremental,
    /// The source has no drive, so it has no ingest register (T158).
    NoRegisterRoot,
    /// Every image of the source was imported earlier (T158).
    AllImportedEarlier,
}

/// The text of `text`.
pub fn import_text(text: ImportText) -> &'static str {
    match text {
        ImportText::Title => "Import",
        ImportText::Source => "Source",
        ImportText::Destination => "Destination",
        ImportText::Backup => "Backup",
        ImportText::Choose => "Choose…",
        ImportText::ChooseSource => "Choose Source",
        ImportText::ChooseDestination => "Choose Destination",
        ImportText::ChooseBackup => "Choose Backup Destination",
        ImportText::FolderTemplate => "Folder Template",
        ImportText::NameTemplate => "File Name Template",
        ImportText::Job => "Job",
        ImportText::FirstNumber => "First Number",
        ImportText::Variables => {
            "Variables: {year} {year2} {month} {day} {hour} {minute} {second} \
             {date} {time} {make} {model} {name} {seq} {seq:4} {job}"
        }
        ImportText::Eject => {
            "Eject the card when the import is done (nothing is deleted from the card)"
        }
        ImportText::Preview => "Preview",
        ImportText::Rescan => "Search Again",
        ImportText::Previewing => "Previewing …",
        ImportText::Importing => "Importing …",
        ImportText::Cancel => "Cancel",
        ImportText::Hide => "Hide",
        ImportText::NewImport => "New Import",
        ImportText::Close => "Close",
        ImportText::Start => "Start Import",
        ImportText::MarkCard => "Mark Card as Imported",
        ImportText::QueueFull => "The import queue is full.",
        ImportText::WorkerStopped => "Import has stopped.",
        ImportText::StoppedMessage => "The import stopped before it was finished.",
        ImportText::EjectSkipped => "The card was not ejected because the import was not complete.",
        ImportText::Incremental => {
            "Copy new images only (skip what was imported from this card before)"
        }
        ImportText::NoRegisterRoot => {
            "The source is not a drive, so earlier imports cannot be recognised; every image is \
             copied."
        }
        ImportText::AllImportedEarlier => {
            "Every image in the source was imported earlier; there is nothing new to copy."
        }
    }
}

/// The status-line text of an import that runs hidden: the files copied of all.
pub fn hidden_progress_text(files_done: usize, files_total: usize) -> String {
    format!("Importing {files_done} / {files_total}")
}

/// The status-line text of a hidden import that has finished or was cancelled.
pub fn hidden_done_text(copied: usize, cancelled: bool) -> String {
    match (cancelled, copied) {
        (true, 1) => "Import cancelled: 1 file".to_owned(),
        (true, _) => format!("Import cancelled: {copied} files"),
        (false, 1) => "Import done: 1 file".to_owned(),
        (false, _) => format!("Import done: {copied} files"),
    }
}

/// The line of a finished import that tells how many files an earlier import took and were left
/// out (T158).
pub fn earlier_done_text(n: usize) -> String {
    match n {
        1 => "1 file imported earlier was left out.".to_owned(),
        _ => format!("{n} files imported earlier were left out."),
    }
}

/// The error of a folder template that cannot be parsed.
pub fn folder_template_error_text(error: &str) -> String {
    format!("Folder Template: {error}")
}

/// The error of a file-name template that cannot be parsed.
pub fn name_template_error_text(error: &str) -> String {
    format!("File Name Template: {error}")
}

/// The error when the source cannot be scanned.
pub fn scan_error_text(source: &Path, error: &str) -> String {
    format!("Could not read {}: {error}", source.display())
}

/// The line listed for the ingest template when the started ExifTool is older than
/// [`MIN_EXIFTOOL_VERSION`]; nothing is written (ADR 0007 §11, D9, T101).
pub fn exiftool_too_old_text(file: &str, version: ToolVersion) -> String {
    format!("{file}: ExifTool {version} is too old (at least {MIN_EXIFTOOL_VERSION} is needed)")
}

/// The line listed for the ingest template when no ExifTool is at hand to write it.
pub fn exiftool_unavailable_text(file: &str) -> String {
    format!("{file}: ExifTool is not available")
}

/// The line of a finished import for one notice of the template's write to `file` (ADR 0017
/// U13-Q19): the file's name and the notice's text.
pub fn template_notice_text(file: &str, notice: &WriteNotice) -> String {
    format!("{file}: {notice}", notice = notice_text(notice))
}

/// The most notices of the template's writes listed in a finished import's lines.
const MAX_NOTICE_LINES: usize = 10;

/// The most IPTC errors listed in a finished import's lines; the rest are counted in one line.
const MAX_IPTC_LINES: usize = 10;

/// Returns the preview summary as in the Tauri app.
pub fn preview_summary(p: &Preview) -> String {
    if p.shots == 0 && p.earlier > 0 {
        return import_text(ImportText::AllImportedEarlier).to_string();
    }
    if p.shots == 0 {
        return "No images were found in the source.".to_string();
    }
    let bytes = format_bytes(p.bytes_to_copy);
    let mut parts = vec![
        match p.shots {
            1 => "1 image".to_string(),
            shots => format!("{shots} images"),
        },
        match p.files_to_copy {
            1 => format!("1 file to copy ({bytes})"),
            files => format!("{files} files to copy ({bytes})"),
        },
    ];
    match p.present {
        0 => {}
        1 => parts.push("1 already exists".to_string()),
        present => parts.push(format!("{present} already exist")),
    }
    match p.earlier {
        0 => {}
        1 => parts.push("1 file imported earlier".to_string()),
        earlier => parts.push(format!("{earlier} files imported earlier")),
    }
    if p.renamed > 0 {
        parts.push(format!("{} renamed because of a name conflict", p.renamed));
    }
    if p.no_exif > 0 {
        parts.push(format!(
            "{} without an EXIF date (the file date is used)",
            p.no_exif
        ));
    }
    parts.join(" · ")
}

/// Returns one line for the preview plan.
pub fn preview_line(line: &PreviewLine) -> String {
    let mut text = format!("{}  →  {}", line.source_name, line.rel);
    match line.action {
        ItemAction::Renamed => text.push_str("  (name conflict)"),
        ItemAction::Present => text.push_str("  (already exists)"),
        ItemAction::BackupOnly => text.push_str("  (backup only)"),
        ItemAction::Copy => {}
    }
    text
}

/// Returns the "and {n} more" line if applicable.
pub fn more_lines(p: &Preview) -> Option<String> {
    if p.items_total > p.lines.len() {
        Some(format!("… and {} more", p.items_total - p.lines.len()))
    } else {
        None
    }
}

/// Returns the progress text during an import.
pub fn progress_text(p: &pw_ingest::Progress) -> String {
    format!(
        "Copying and verifying {} / {} · {} of {} · {}",
        p.files_done,
        p.files_total,
        format_bytes(p.bytes_done),
        format_bytes(p.bytes_total),
        p.current
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    )
}

/// Returns the progress text while ejecting the source card.
pub fn ejecting_progress_text(source: &Path) -> String {
    format!("Ejecting {} …", source.display())
}

#[derive(Debug)]
pub struct DoneLine {
    pub text: String,
    pub error: bool,
}

/// Returns the summary lines for a finished import.
pub fn done_lines(done: &ImportDone) -> Vec<DoneLine> {
    if let Some(error) = &done.error {
        return vec![DoneLine {
            text: error.clone(),
            error: true,
        }];
    }

    let mut lines = Vec::new();
    let seconds = done.elapsed.as_secs_f64();
    let speed = if seconds > 0.0 {
        // Rounded, as `toFixed(0)` in the Tauri dialog.
        format!(" · {:.0} MB/s", done.bytes as f64 / 1_000_000.0 / seconds)
    } else {
        "".to_string()
    };
    let bytes = format_bytes(done.bytes);
    let copied = done.copied;
    let mut summary = match (done.cancelled, copied) {
        (true, 1) => {
            format!("Cancelled: 1 file copied and verified ({bytes} in {seconds:.1} s{speed})")
        }
        (true, _) => {
            format!(
                "Cancelled: {copied} files copied and verified ({bytes} in {seconds:.1} s{speed})"
            )
        }
        (false, 1) => {
            format!("Done: 1 file copied and verified ({bytes} in {seconds:.1} s{speed})")
        }
        (false, _) => {
            format!("Done: {copied} files copied and verified ({bytes} in {seconds:.1} s{speed})")
        }
    };
    if done.present > 0 {
        summary.push_str(&format!(", {} already existed", done.present));
    }
    summary.push('.');
    lines.push(DoneLine {
        text: summary,
        error: false,
    });
    if done.earlier > 0 {
        lines.push(DoneLine {
            text: earlier_done_text(done.earlier),
            error: false,
        });
    }

    for (i, failure) in done.failed.iter().enumerate() {
        if i >= 20 {
            lines.push(DoneLine {
                text: format!("… and {} more", done.failed.len() - 20),
                error: true,
            });
            break;
        }
        lines.push(DoneLine {
            text: format!("Failed: {failure}"),
            error: true,
        });
    }

    for error in done.iptc_errors.iter().take(MAX_IPTC_LINES) {
        lines.push(DoneLine {
            text: format!("IPTC: {error}"),
            error: true,
        });
    }
    if done.iptc_errors.len() > MAX_IPTC_LINES {
        lines.push(DoneLine {
            text: format!("… and {} more", done.iptc_errors.len() - MAX_IPTC_LINES),
            error: true,
        });
    }

    for notice in done.template_notices.iter().take(MAX_NOTICE_LINES) {
        lines.push(DoneLine {
            text: notice.clone(),
            error: false,
        });
    }
    if done.template_notices.len() > MAX_NOTICE_LINES {
        lines.push(DoneLine {
            text: format!(
                "… and {} more",
                done.template_notices.len() - MAX_NOTICE_LINES
            ),
            error: false,
        });
    }

    if let Some(register_error) = &done.register_error {
        lines.push(DoneLine {
            text: register_error.clone(),
            error: true,
        });
    }

    if let Some(eject) = &done.eject {
        lines.push(DoneLine {
            text: eject.clone(),
            error: false,
        });
    }

    if done.folders.len() > 1 {
        let folder_texts: Vec<String> = done
            .folders
            .iter()
            .map(|(name, count)| format!("{name} ({count})"))
            .collect();
        lines.push(DoneLine {
            text: format!("Folders: {}", folder_texts.join(", ")),
            error: false,
        });
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn empty_preview() -> Preview {
        Preview {
            shots: 0,
            files_to_copy: 0,
            bytes_to_copy: 0,
            present: 0,
            renamed: 0,
            no_exif: 0,
            items_total: 0,
            lines: Vec::new(),
            earlier: 0,
            register_note: None,
        }
    }

    #[test]
    fn summary_with_zero_shots() {
        let p = Preview {
            shots: 0,
            ..empty_preview()
        };
        assert_eq!(preview_summary(&p), "No images were found in the source.");
    }

    #[test]
    fn summary_with_everything() {
        let p = Preview {
            shots: 10,
            files_to_copy: 15,
            bytes_to_copy: 1_048_576,
            present: 5,
            renamed: 2,
            no_exif: 3,
            ..empty_preview()
        };
        assert_eq!(
            preview_summary(&p),
            "10 images · 15 files to copy (1.0 MB) · 5 already exist · 2 renamed because of a name conflict · 3 without an EXIF date (the file date is used)"
        );
    }

    /// Singular and plural of the counted texts (T117).
    #[test]
    fn counted_texts_have_singular_and_plural() {
        let one = Preview {
            shots: 1,
            files_to_copy: 1,
            bytes_to_copy: 1024,
            present: 1,
            renamed: 1,
            no_exif: 1,
            ..empty_preview()
        };
        assert_eq!(
            preview_summary(&one),
            "1 image · 1 file to copy (1.0 KB) · 1 already exists · 1 renamed because of a name \
             conflict · 1 without an EXIF date (the file date is used)"
        );
        let two = Preview {
            shots: 2,
            files_to_copy: 2,
            bytes_to_copy: 1024,
            present: 2,
            ..empty_preview()
        };
        assert_eq!(
            preview_summary(&two),
            "2 images · 2 files to copy (1.0 KB) · 2 already exist"
        );
        assert_eq!(hidden_done_text(1, false), "Import done: 1 file");
        assert_eq!(hidden_done_text(2, false), "Import done: 2 files");
        assert_eq!(hidden_done_text(1, true), "Import cancelled: 1 file");
        assert_eq!(hidden_done_text(2, true), "Import cancelled: 2 files");
        let done = |copied, cancelled| {
            done_lines(&ImportDone {
                copied,
                cancelled,
                ..ImportDone::default()
            })[0]
                .text
                .clone()
        };
        assert_eq!(
            done(1, false),
            "Done: 1 file copied and verified (0 B in 0.0 s)."
        );
        assert_eq!(
            done(2, false),
            "Done: 2 files copied and verified (0 B in 0.0 s)."
        );
        assert_eq!(
            done(1, true),
            "Cancelled: 1 file copied and verified (0 B in 0.0 s)."
        );
        assert_eq!(
            done(2, true),
            "Cancelled: 2 files copied and verified (0 B in 0.0 s)."
        );
    }

    #[test]
    fn lines_and_more_lines() {
        let line = PreviewLine {
            source_name: "A.ARW".to_string(),
            rel: "folder/A.ARW".to_string(),
            action: ItemAction::Renamed,
        };
        assert_eq!(
            preview_line(&line),
            "A.ARW  →  folder/A.ARW  (name conflict)"
        );
        assert_eq!(
            preview_line(&PreviewLine {
                action: ItemAction::Present,
                ..line.clone()
            }),
            "A.ARW  →  folder/A.ARW  (already exists)"
        );
        assert_eq!(
            preview_line(&PreviewLine {
                action: ItemAction::BackupOnly,
                ..line.clone()
            }),
            "A.ARW  →  folder/A.ARW  (backup only)"
        );
        assert_eq!(
            preview_line(&PreviewLine {
                action: ItemAction::Copy,
                ..line.clone()
            }),
            "A.ARW  →  folder/A.ARW"
        );

        let p = Preview {
            items_total: 25,
            lines: vec![line; 20],
            ..empty_preview()
        };
        assert_eq!(more_lines(&p), Some("… and 5 more".to_string()));
        assert_eq!(more_lines(&empty_preview()), None);
    }

    #[test]
    fn progress() {
        let p = pw_ingest::Progress {
            files_done: 5,
            files_total: 10,
            bytes_done: 512 * 1024,
            bytes_total: 1024 * 1024,
            current: PathBuf::from("source/image.ARW"),
        };
        assert_eq!(
            progress_text(&p),
            "Copying and verifying 5 / 10 · 512.0 KB of 1.0 MB · image.ARW"
        );
        assert_eq!(
            progress_text(&pw_ingest::Progress::default()),
            "Copying and verifying 0 / 0 · 0 B of 0 B · "
        );
    }

    #[test]
    fn ejecting_progress_includes_the_source_path() {
        assert_eq!(
            ejecting_progress_text(Path::new(r"E:\DCIM")),
            "Ejecting E:\\DCIM …"
        );
    }

    #[test]
    fn done_success_with_one_folder() {
        let done = ImportDone {
            copied: 10,
            bytes: 10_000_000,
            elapsed: Duration::from_secs(2),
            folders: vec![("C:/Foto/2026".to_string(), 10)],
            ..ImportDone::default()
        };
        let lines = done_lines(&done);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].text,
            "Done: 10 files copied and verified (9.5 MB in 2.0 s · 5 MB/s)."
        );
        assert!(!lines[0].error);
    }

    #[test]
    fn done_success_with_zero_values_has_no_speed_or_folder_line() {
        let lines = done_lines(&ImportDone::default());
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].text,
            "Done: 0 files copied and verified (0 B in 0.0 s)."
        );
        assert!(!lines[0].error);
    }

    #[test]
    fn done_cancelled_with_failures_and_folders() {
        let done = ImportDone {
            copied: 1,
            present: 2,
            bytes: 1_000_000,
            elapsed: Duration::from_secs(1),
            cancelled: true,
            failed: (0..25).map(|i| format!("img{i}.jpg")).collect(),
            folders: vec![("F1".to_string(), 1), ("F2".to_string(), 1)],
            ..ImportDone::default()
        };
        let lines = done_lines(&done);
        assert_eq!(
            lines[0].text,
            "Cancelled: 1 file copied and verified (976.6 KB in 1.0 s · 1 MB/s), 2 already existed."
        );
        assert_eq!(lines[1].text, "Failed: img0.jpg");
        assert!(lines[1].error);
        assert_eq!(lines[21].text, "… and 5 more");
        assert_eq!(lines[22].text, "Folders: F1 (1), F2 (1)");
    }

    #[test]
    fn done_speed_is_rounded_like_the_tauri_dialog() {
        let done = ImportDone {
            copied: 1,
            bytes: 4_600_000,
            elapsed: Duration::from_secs(1),
            ..ImportDone::default()
        };
        assert_eq!(
            done_lines(&done)[0].text,
            "Done: 1 file copied and verified (4.4 MB in 1.0 s · 5 MB/s)."
        );
    }

    #[test]
    fn done_with_error() {
        let done = ImportDone {
            error: Some("Disk full".to_string()),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Disk full");
        assert!(lines[0].error);
    }

    #[test]
    fn done_includes_eject_message_after_iptc_errors_as_a_normal_line() {
        let done = ImportDone {
            iptc_errors: vec!["A001.ARW: metadata error".to_string()],
            eject: Some("E: has been ejected and can be removed.".to_string()),
            ..ImportDone::default()
        };

        let lines = done_lines(&done);

        assert_eq!(lines[1].text, "IPTC: A001.ARW: metadata error");
        assert!(lines[1].error);
        assert_eq!(lines[2].text, "E: has been ejected and can be removed.");
        assert!(!lines[2].error);
    }

    #[test]
    fn done_includes_at_most_ten_iptc_errors_after_copy_failures() {
        let done = ImportDone {
            failed: vec!["copy failure".to_string()],
            iptc_errors: (0..12)
                .map(|index| format!("image{index}.ARW: error"))
                .collect(),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);

        assert_eq!(lines[1].text, "Failed: copy failure");
        assert!(lines[1].error);
        // T294 review: the two left out are counted in one error line after the ten.
        assert_eq!(lines.len(), 13);
        assert_eq!(lines[2].text, "IPTC: image0.ARW: error");
        assert_eq!(lines[11].text, "IPTC: image9.ARW: error");
        assert_eq!(lines[12].text, "… and 2 more");
        assert!(lines[2..].iter().all(|line| line.error));
    }

    /// T294 review: exactly ten IPTC errors are all listed, with no count line.
    #[test]
    fn done_lines_count_no_iptc_errors_when_ten_are_listed() {
        let done = ImportDone {
            iptc_errors: (0..10)
                .map(|index| format!("image{index}.ARW: error"))
                .collect(),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);

        assert_eq!(lines.len(), 11);
        assert_eq!(lines[10].text, "IPTC: image9.ARW: error");
    }

    /// T294 (U13-Q19): the line of one notice of the template's write.
    #[test]
    fn template_notice_text_names_the_file_and_the_notice() {
        assert_eq!(
            template_notice_text(
                "A001.jpg",
                &WriteNotice::Truncated {
                    property: "photoshop:Headline".to_string()
                }
            ),
            "A001.jpg: Headline was truncated in IIM; XMP has the whole text"
        );
        assert_eq!(
            template_notice_text("A002.jpg", &WriteNotice::IimConvertedToUtf8),
            "A002.jpg: The IIM texts were rewritten as UTF-8"
        );
    }

    /// T294: the template's notices come after the `IPTC:` lines, as normal lines with no prefix.
    #[test]
    fn done_lines_show_the_template_notices_after_the_iptc_lines() {
        let done = ImportDone {
            iptc_errors: vec!["A001.ARW: metadata error".to_string()],
            template_notices: (0..3)
                .map(|index| format!("image{index}.jpg: notice"))
                .collect(),
            eject: Some("E: has been ejected and can be removed.".to_string()),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);
        assert_eq!(lines[1].text, "IPTC: A001.ARW: metadata error");
        assert!(lines[1].error);
        assert_eq!(
            lines[2..5]
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            [
                "image0.jpg: notice",
                "image1.jpg: notice",
                "image2.jpg: notice"
            ]
        );
        assert!(lines[2..5].iter().all(|line| !line.error));
        assert_eq!(lines[5].text, "E: has been ejected and can be removed.");
        assert_eq!(lines.len(), 6);
    }

    /// T294: at most ten notices are listed, then one normal line with the count of the rest.
    #[test]
    fn done_lines_show_at_most_ten_template_notices() {
        let done = ImportDone {
            template_notices: (0..12)
                .map(|index| format!("image{index}.jpg: notice"))
                .collect(),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);
        assert_eq!(lines.len(), 12);
        assert_eq!(lines[1].text, "image0.jpg: notice");
        assert_eq!(lines[10].text, "image9.jpg: notice");
        assert_eq!(lines[11].text, "… and 2 more");
        assert!(lines[1..].iter().all(|line| !line.error));
    }

    /// One example of each text moved here by T116, as it was before.
    #[test]
    fn every_moved_import_text_keeps_its_wording() {
        let texts = [
            (ImportText::Title, "Import"),
            (ImportText::Source, "Source"),
            (ImportText::Destination, "Destination"),
            (ImportText::Backup, "Backup"),
            (ImportText::Choose, "Choose…"),
            (ImportText::ChooseSource, "Choose Source"),
            (ImportText::ChooseDestination, "Choose Destination"),
            (ImportText::ChooseBackup, "Choose Backup Destination"),
            (ImportText::FolderTemplate, "Folder Template"),
            (ImportText::NameTemplate, "File Name Template"),
            (ImportText::Job, "Job"),
            (ImportText::FirstNumber, "First Number"),
            (
                ImportText::Variables,
                "Variables: {year} {year2} {month} {day} {hour} {minute} {second} {date} {time} \
                 {make} {model} {name} {seq} {seq:4} {job}",
            ),
            (
                ImportText::Eject,
                "Eject the card when the import is done (nothing is deleted from the card)",
            ),
            (ImportText::Preview, "Preview"),
            (ImportText::Rescan, "Search Again"),
            (ImportText::Previewing, "Previewing …"),
            (ImportText::Importing, "Importing …"),
            (ImportText::Cancel, "Cancel"),
            (ImportText::Hide, "Hide"),
            (ImportText::NewImport, "New Import"),
            (ImportText::Close, "Close"),
            (ImportText::Start, "Start Import"),
            (ImportText::QueueFull, "The import queue is full."),
            (ImportText::WorkerStopped, "Import has stopped."),
            (
                ImportText::StoppedMessage,
                "The import stopped before it was finished.",
            ),
            (
                ImportText::EjectSkipped,
                "The card was not ejected because the import was not complete.",
            ),
        ];
        for (text, expected) in texts {
            assert_eq!(import_text(text), expected, "{text:?}");
        }
        assert_eq!(hidden_progress_text(0, 0), "Importing 0 / 0");
        assert_eq!(hidden_progress_text(3, 12), "Importing 3 / 12");
        assert_eq!(hidden_done_text(5, false), "Import done: 5 files");
        assert_eq!(hidden_done_text(5, true), "Import cancelled: 5 files");
        assert_eq!(
            folder_template_error_text("ogiltig"),
            "Folder Template: ogiltig"
        );
        assert_eq!(
            name_template_error_text("ogiltig"),
            "File Name Template: ogiltig"
        );
        assert_eq!(
            scan_error_text(Path::new("E:/DCIM"), "nekad"),
            "Could not read E:/DCIM: nekad"
        );
        assert_eq!(
            exiftool_too_old_text(
                "A001.jpg",
                ToolVersion {
                    major: 12,
                    minor: 40
                }
            ),
            "A001.jpg: ExifTool 12.40 is too old (at least 13.64 is needed)"
        );
        assert_eq!(
            exiftool_unavailable_text("A001.jpg"),
            "A001.jpg: ExifTool is not available"
        );
    }

    // ---------- copy new images only (T158) ----------

    #[test]
    fn the_incremental_texts_keep_their_wording() {
        assert_eq!(
            import_text(ImportText::Incremental),
            "Copy new images only (skip what was imported from this card before)"
        );
        assert_eq!(
            import_text(ImportText::NoRegisterRoot),
            "The source is not a drive, so earlier imports cannot be recognised; every image is \
             copied."
        );
        assert_eq!(
            import_text(ImportText::AllImportedEarlier),
            "Every image in the source was imported earlier; there is nothing new to copy."
        );
        assert_eq!(import_text(ImportText::MarkCard), "Mark Card as Imported");
        assert_eq!(
            earlier_done_text(1),
            "1 file imported earlier was left out."
        );
        assert_eq!(
            earlier_done_text(3),
            "3 files imported earlier were left out."
        );
    }

    #[test]
    fn summary_counts_the_files_imported_earlier() {
        let one = Preview {
            shots: 2,
            files_to_copy: 2,
            bytes_to_copy: 1024,
            present: 1,
            earlier: 1,
            ..empty_preview()
        };
        assert_eq!(
            preview_summary(&one),
            "2 images · 2 files to copy (1.0 KB) · 1 already exists · 1 file imported earlier"
        );
        let many = Preview {
            shots: 2,
            files_to_copy: 2,
            bytes_to_copy: 1024,
            earlier: 4,
            ..empty_preview()
        };
        assert_eq!(
            preview_summary(&many),
            "2 images · 2 files to copy (1.0 KB) · 4 files imported earlier"
        );
    }

    #[test]
    fn summary_when_every_image_was_imported_earlier() {
        let p = Preview {
            shots: 0,
            earlier: 3,
            ..empty_preview()
        };
        assert_eq!(
            preview_summary(&p),
            "Every image in the source was imported earlier; there is nothing new to copy."
        );
    }

    #[test]
    fn done_lines_tell_the_files_left_out_right_after_the_summary() {
        for (earlier, text) in [
            (1, "1 file imported earlier was left out."),
            (5, "5 files imported earlier were left out."),
        ] {
            let lines = done_lines(&ImportDone {
                failed: vec!["copy failure".to_string()],
                earlier,
                ..ImportDone::default()
            });
            assert_eq!(lines[1].text, text);
            assert!(!lines[1].error);
            assert_eq!(lines[2].text, "Failed: copy failure");
        }
        assert_eq!(done_lines(&ImportDone::default()).len(), 1);
    }

    #[test]
    fn done_lines_put_the_register_error_right_before_the_eject_line() {
        let done = ImportDone {
            iptc_errors: vec!["A001.ARW: metadata error".to_string()],
            register_error: Some("The card could not be marked as imported: denied.".to_string()),
            eject: Some("E: has been ejected and can be removed.".to_string()),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);
        assert_eq!(lines[1].text, "IPTC: A001.ARW: metadata error");
        assert_eq!(
            lines[2].text,
            "The card could not be marked as imported: denied."
        );
        assert!(lines[2].error);
        assert_eq!(lines[3].text, "E: has been ejected and can be removed.");
        assert!(!lines[3].error);
        assert_eq!(lines.len(), 4);
    }
}
