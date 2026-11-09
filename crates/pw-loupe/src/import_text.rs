use crate::{ImportDone, ItemAction, Preview, PreviewLine};
use pw_core::format_bytes;
use std::path::Path;

/// Returns the Swedish preview summary as in the Tauri app.
pub fn preview_summary(p: &Preview) -> String {
    if p.shots == 0 {
        return "Inga bilder hittades i källan.".to_string();
    }
    let mut parts = vec![
        format!("{} bilder", p.shots),
        format!(
            "{} filer att kopiera ({})",
            p.files_to_copy,
            format_bytes(p.bytes_to_copy)
        ),
    ];
    if p.present > 0 {
        parts.push(format!("{} finns redan", p.present));
    }
    if p.renamed > 0 {
        parts.push(format!("{} får nytt namn pga namnkrock", p.renamed));
    }
    if p.no_exif > 0 {
        parts.push(format!("{} utan EXIF-datum (fildatum används)", p.no_exif));
    }
    parts.join(" · ")
}

/// Returns one Swedish line for the preview plan.
pub fn preview_line(line: &PreviewLine) -> String {
    let mut text = format!("{}  →  {}", line.source_name, line.rel);
    match line.action {
        ItemAction::Renamed => text.push_str("  (namnkrock)"),
        ItemAction::Present => text.push_str("  (finns redan)"),
        ItemAction::BackupOnly => text.push_str("  (bara backup)"),
        ItemAction::Copy => {}
    }
    text
}

/// Returns the Swedish "and {n} more" line if applicable.
pub fn more_lines(p: &Preview) -> Option<String> {
    if p.items_total > p.lines.len() {
        Some(format!("… och {} till", p.items_total - p.lines.len()))
    } else {
        None
    }
}

/// Returns the Swedish progress text during an import.
pub fn progress_text(p: &pw_ingest::Progress) -> String {
    format!(
        "Kopierar och verifierar {} / {} · {} av {} · {}",
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

/// Returns the Swedish progress text while writing IPTC metadata.
pub fn iptc_progress_text(done: usize, total: usize, current: &Path) -> String {
    format!(
        "Skriver IPTC {} / {} · {}",
        done.saturating_add(1),
        total,
        current
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default()
    )
}

#[derive(Debug)]
pub struct DoneLine {
    pub text: String,
    pub error: bool,
}

/// Returns Swedish summary lines for a finished import.
pub fn done_lines(done: &ImportDone) -> Vec<DoneLine> {
    if let Some(error) = &done.error {
        return vec![DoneLine {
            text: error.clone(),
            error: true,
        }];
    }

    let mut lines = Vec::new();
    let prefix = if done.cancelled {
        "Avbruten: "
    } else {
        "Klar: "
    };
    let seconds = done.elapsed.as_secs_f64();
    let speed = if seconds > 0.0 {
        // Rounded, as `toFixed(0)` in the Tauri dialog.
        format!(" · {:.0} MB/s", done.bytes as f64 / 1_000_000.0 / seconds)
    } else {
        "".to_string()
    };
    let mut summary = format!(
        "{prefix}{} filer kopierade och verifierade ({} på {seconds:.1} s{speed})",
        done.copied,
        format_bytes(done.bytes),
    );
    if done.present > 0 {
        summary.push_str(&format!(", {} fanns redan", done.present));
    }
    summary.push('.');
    lines.push(DoneLine {
        text: summary,
        error: false,
    });

    for (i, failure) in done.failed.iter().enumerate() {
        if i >= 20 {
            lines.push(DoneLine {
                text: format!("… och {} till", done.failed.len() - 20),
                error: true,
            });
            break;
        }
        lines.push(DoneLine {
            text: format!("Misslyckades: {failure}"),
            error: true,
        });
    }

    for error in done.iptc_errors.iter().take(10) {
        lines.push(DoneLine {
            text: format!("IPTC: {error}"),
            error: true,
        });
    }

    if done.folders.len() > 1 {
        let folder_texts: Vec<String> = done
            .folders
            .iter()
            .map(|(name, count)| format!("{name} ({count})"))
            .collect();
        lines.push(DoneLine {
            text: format!("Mappar: {}", folder_texts.join(", ")),
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
        }
    }

    #[test]
    fn summary_with_zero_shots() {
        let p = Preview {
            shots: 0,
            ..empty_preview()
        };
        assert_eq!(preview_summary(&p), "Inga bilder hittades i källan.");
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
            "10 bilder · 15 filer att kopiera (1.0 MB) · 5 finns redan · 2 får nytt namn pga namnkrock · 3 utan EXIF-datum (fildatum används)"
        );
    }

    #[test]
    fn lines_and_more_lines() {
        let line = PreviewLine {
            source_name: "A.ARW".to_string(),
            rel: "folder/A.ARW".to_string(),
            action: ItemAction::Renamed,
        };
        assert_eq!(preview_line(&line), "A.ARW  →  folder/A.ARW  (namnkrock)");
        assert_eq!(
            preview_line(&PreviewLine {
                action: ItemAction::Present,
                ..line.clone()
            }),
            "A.ARW  →  folder/A.ARW  (finns redan)"
        );
        assert_eq!(
            preview_line(&PreviewLine {
                action: ItemAction::BackupOnly,
                ..line.clone()
            }),
            "A.ARW  →  folder/A.ARW  (bara backup)"
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
        assert_eq!(more_lines(&p), Some("… och 5 till".to_string()));
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
            "Kopierar och verifierar 5 / 10 · 512.0 KB av 1.0 MB · image.ARW"
        );
        assert_eq!(
            progress_text(&pw_ingest::Progress::default()),
            "Kopierar och verifierar 0 / 0 · 0 B av 0 B · "
        );
    }

    #[test]
    fn iptc_progress_uses_one_based_file_number_and_name() {
        assert_eq!(
            iptc_progress_text(2, 5, Path::new("destination/image.ARW")),
            "Skriver IPTC 3 / 5 · image.ARW"
        );
        assert_eq!(
            iptc_progress_text(0, 0, Path::new("")),
            "Skriver IPTC 1 / 0 · "
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
            "Klar: 10 filer kopierade och verifierade (9.5 MB på 2.0 s · 5 MB/s)."
        );
        assert!(!lines[0].error);
    }

    #[test]
    fn done_success_with_zero_values_has_no_speed_or_folder_line() {
        let lines = done_lines(&ImportDone::default());
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].text,
            "Klar: 0 filer kopierade och verifierade (0 B på 0.0 s)."
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
            "Avbruten: 1 filer kopierade och verifierade (976.6 KB på 1.0 s · 1 MB/s), 2 fanns redan."
        );
        assert_eq!(lines[1].text, "Misslyckades: img0.jpg");
        assert!(lines[1].error);
        assert_eq!(lines[21].text, "… och 5 till");
        assert_eq!(lines[22].text, "Mappar: F1 (1), F2 (1)");
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
            "Klar: 1 filer kopierade och verifierade (4.4 MB på 1.0 s · 5 MB/s)."
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
    fn done_includes_at_most_ten_iptc_errors_after_copy_failures() {
        let done = ImportDone {
            failed: vec!["copy failure".to_string()],
            iptc_errors: (0..12)
                .map(|index| format!("image{index}.ARW: error"))
                .collect(),
            ..ImportDone::default()
        };
        let lines = done_lines(&done);

        assert_eq!(lines[1].text, "Misslyckades: copy failure");
        assert!(lines[1].error);
        assert_eq!(lines.len(), 12);
        assert_eq!(lines[2].text, "IPTC: image0.ARW: error");
        assert_eq!(lines[11].text, "IPTC: image9.ARW: error");
        assert!(lines[2..].iter().all(|line| line.error));
    }
}
