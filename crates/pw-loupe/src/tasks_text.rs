//! The texts of the Tasks area in the left column (ADR 0017 §2.1, U5, T140): its buttons, the
//! import row, the pending writes and the error list. English (GR14), with the terms of
//! `docs/glossary.md`; each text a whole sentence from one template (`docs/tasks/README.md`,
//! rule 13). The import's ejecting step keeps the import dialog's text
//! (`ejecting_progress_text`), and a finished import without failures `hidden_done_text`.

use std::path::Path;

use crate::hidden_done_text;
use crate::shell_text::{ShellText, shell_text};
use crate::tasks::{ClockTime, PendingWrite};

/// The fixed texts of the Tasks area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TasksText {
    /// The section's heading (the column title of the glossary, §3).
    Tasks,
    /// The line under the heading with nothing to show (as before T140).
    NoTasks,
    /// The button on the running import's row (U5-Q3).
    Stop,
    /// The button that empties the error list (U5-Q5).
    Clear,
    /// The button that removes a finished import's row (U5-Q2).
    Dismiss,
    /// The hover text of the import row: a click shows the import dialog (U5-Q2).
    ShowImport,
    /// The hover text of "×".
    DismissHover,
    /// The line of the metadata window's own write (U5-Q4).
    SavingMetadata,
}

/// The fixed text `text`.
pub fn tasks_text(text: TasksText) -> &'static str {
    match text {
        TasksText::Tasks => shell_text(ShellText::Tasks),
        TasksText::NoTasks => shell_text(ShellText::NoTasks),
        TasksText::Stop => "Stop",
        TasksText::Clear => "Clear",
        TasksText::Dismiss => "×",
        TasksText::ShowImport => "Show the import window",
        TasksText::DismissHover => "Remove this line",
        TasksText::SavingMetadata => "Saving metadata",
    }
}

/// The import row's first line: where it copies from and to.
pub fn import_route_text(source: &str, destination: &str) -> String {
    format!("{source} → {destination}")
}

/// The import row's progress while files are copied: the files done of all and the last file
/// copied; before the first file only the counts.
pub fn importing_text(done: usize, total: usize, file: Option<&Path>) -> String {
    match file.and_then(Path::file_name) {
        Some(name) => format!("Importing {done} of {total}: {}", name.to_string_lossy()),
        None => format!("Importing {done} of {total}"),
    }
}

/// The finished import's row: the files copied and, when some failed, how many.
pub fn import_done_text(copied: usize, failed: usize, cancelled: bool) -> String {
    let files = if copied == 1 { "file" } else { "files" };
    match (cancelled, failed) {
        (_, 0) => hidden_done_text(copied, cancelled),
        (true, _) => format!("Import cancelled: {copied} {files}, {failed} failed"),
        (false, _) => format!("Import done: {copied} {files}, {failed} failed"),
    }
}

/// The line of a pending write (U5-Q4).
pub fn pending_write_text(write: PendingWrite) -> String {
    match write {
        PendingWrite::Ratings(count) => format!("Writing ratings: {count}"),
        PendingWrite::Rotations(count) => format!("Writing rotations: {count}"),
        PendingWrite::Batch { current, total } => {
            format!("Saving metadata: {current} of {total}")
        }
        PendingWrite::Editor => tasks_text(TasksText::SavingMetadata).to_owned(),
    }
}

/// The error list's heading with the number of errors kept (U5-Q5).
pub fn errors_heading_text(count: usize) -> String {
    format!("Errors ({count})")
}

/// One line of the error list: its time (`HH:MM`) and the text.
pub fn error_entry_text(time: ClockTime, text: &str) -> String {
    format!("{:02}:{:02} {text}", time.hour, time.minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_texts_are_the_glossary_terms() {
        for (text, expected) in [
            (TasksText::Tasks, "Tasks"),
            (TasksText::NoTasks, "No tasks in progress."),
            (TasksText::Stop, "Stop"),
            (TasksText::Clear, "Clear"),
            (TasksText::Dismiss, "×"),
            (TasksText::ShowImport, "Show the import window"),
            (TasksText::DismissHover, "Remove this line"),
            (TasksText::SavingMetadata, "Saving metadata"),
        ] {
            assert_eq!(tasks_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn the_import_row_texts() {
        assert_eq!(import_route_text("E:\\", "D:\\Bilder"), "E:\\ → D:\\Bilder");
        assert_eq!(
            importing_text(3, 12, Some(Path::new("E:\\DCIM\\TN100423.ARW"))),
            "Importing 3 of 12: TN100423.ARW"
        );
        assert_eq!(importing_text(0, 0, None), "Importing 0 of 0");
    }

    #[test]
    fn the_done_text_counts_the_failed_files_in_one_sentence() {
        assert_eq!(import_done_text(12, 0, false), "Import done: 12 files");
        assert_eq!(import_done_text(1, 0, false), "Import done: 1 file");
        assert_eq!(import_done_text(4, 0, true), "Import cancelled: 4 files");
        assert_eq!(
            import_done_text(10, 2, false),
            "Import done: 10 files, 2 failed"
        );
        assert_eq!(
            import_done_text(1, 1, false),
            "Import done: 1 file, 1 failed"
        );
        assert_eq!(
            import_done_text(3, 1, true),
            "Import cancelled: 3 files, 1 failed"
        );
    }

    #[test]
    fn the_pending_write_texts() {
        assert_eq!(
            pending_write_text(PendingWrite::Ratings(4)),
            "Writing ratings: 4"
        );
        assert_eq!(
            pending_write_text(PendingWrite::Rotations(3)),
            "Writing rotations: 3"
        );
        assert_eq!(
            pending_write_text(PendingWrite::Batch {
                current: 2,
                total: 9
            }),
            "Saving metadata: 2 of 9"
        );
        assert_eq!(pending_write_text(PendingWrite::Editor), "Saving metadata");
    }

    #[test]
    fn the_error_list_texts() {
        assert_eq!(errors_heading_text(0), "Errors (0)");
        assert_eq!(errors_heading_text(17), "Errors (17)");
        assert_eq!(
            error_entry_text(
                ClockTime { hour: 9, minute: 5 },
                "Could not write the rating."
            ),
            "09:05 Could not write the rating."
        );
        assert_eq!(
            error_entry_text(
                ClockTime {
                    hour: 23,
                    minute: 59
                },
                "x"
            ),
            "23:59 x"
        );
    }
}
