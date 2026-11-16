//! The texts of the loupe and its status line (T116, rule 13): the source and zoom of the shown
//! image, the position and filter texts, the loading indicator, the errors of a file that cannot be
//! shown, the RAW/JPEG control, and the text of a contact-sheet cell without a preview. Moved here
//! from `lib.rs` and `main.rs` word for word; the drawing code holds none of them.

use pw_viewcore::{DecodeError, Frame, Source, Status, Zoom};

use crate::shell_text::status_line_text;

fn file_name(frame: &Frame) -> String {
    frame
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether the frame shows a TIFF file. The core keys a TIFF `Source::Jpeg` (ADR 0015 §4.2), so the
/// texts look at the path.
fn is_tiff(frame: &Frame) -> bool {
    frame.path.as_deref().is_some_and(pw_raw::is_tiff_file)
}

/// What is shown (G1, honest source): RAW rendering, embedded JPEG, a JPEG file or a TIFF file,
/// with size.
pub fn source_text(frame: &Frame) -> String {
    let size = frame
        .size
        .map(|s| {
            let (w, h) = s.oriented();
            format!(" {w} × {h}")
        })
        .unwrap_or_default();
    let plain = frame
        .path
        .as_deref()
        .is_some_and(pw_viewcore::is_plain_jpeg);
    match frame.source {
        Source::Raw => format!("RAW rendering (LibRaw, PPG){size}"),
        Source::Jpeg if is_tiff(frame) => format!("TIFF file{size}"),
        Source::Jpeg if plain => format!("JPEG file{size}"),
        Source::Jpeg => format!("Embedded JPEG{size} (camera rendering, not RAW)"),
    }
}

/// The zoom part of the status line: "Zoom 37%", and "(Fit)" at fit.
pub fn zoom_text(frame: &Frame) -> String {
    match (frame.zoom, frame.scale) {
        (Zoom::Fit, Some(s)) => format!("Zoom {:.0}% (Fit)", s * 100.0),
        (Zoom::Fit, None) => "Zoom: Fit".to_string(),
        (Zoom::Scale(s), _) => format!("Zoom {:.0}%", s * 100.0),
    }
}

/// The line at the top of each side of the compare (ADR 0017 §2.2, U6-Q10, U6-Q12): the file name
/// (not the folder), what is shown (G1, told apart as [`source_text`] tells it, without the pixel
/// size) and the zoom in percent, with "(Fit)" at fit. Each line is one whole template (rule 13);
/// with no path, or no scale yet at fit, the template is the one without the missing part.
pub fn compare_side_text(frame: &Frame) -> String {
    let name = frame
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy());
    // The zoom in percent, and whether it is fit; `None` at fit before the scale is known.
    let zoom = match (frame.zoom, frame.scale) {
        (Zoom::Fit, Some(s)) => Some((s * 100.0, true)),
        (Zoom::Fit, None) => None,
        (Zoom::Scale(s), _) => Some((s * 100.0, false)),
    };
    let plain = frame
        .path
        .as_deref()
        .is_some_and(pw_viewcore::is_plain_jpeg);
    match (name, frame.source) {
        (Some(file), Source::Raw) => match zoom {
            Some((zoom, true)) => format!("{file} · RAW rendering · {zoom:.0}% (Fit)"),
            Some((zoom, false)) => format!("{file} · RAW rendering · {zoom:.0}%"),
            None => format!("{file} · RAW rendering"),
        },
        (Some(file), Source::Jpeg) if is_tiff(frame) => match zoom {
            Some((zoom, true)) => format!("{file} · TIFF file · {zoom:.0}% (Fit)"),
            Some((zoom, false)) => format!("{file} · TIFF file · {zoom:.0}%"),
            None => format!("{file} · TIFF file"),
        },
        (Some(file), Source::Jpeg) if plain => match zoom {
            Some((zoom, true)) => format!("{file} · JPEG file · {zoom:.0}% (Fit)"),
            Some((zoom, false)) => format!("{file} · JPEG file · {zoom:.0}%"),
            None => format!("{file} · JPEG file"),
        },
        (Some(file), Source::Jpeg) => match zoom {
            Some((zoom, true)) => format!("{file} · Embedded JPEG · {zoom:.0}% (Fit)"),
            Some((zoom, false)) => format!("{file} · Embedded JPEG · {zoom:.0}%"),
            None => format!("{file} · Embedded JPEG"),
        },
        // Without a path the file cannot be told to be a JPEG or TIFF file.
        (None, Source::Raw) => match zoom {
            Some((zoom, true)) => format!("RAW rendering · {zoom:.0}% (Fit)"),
            Some((zoom, false)) => format!("RAW rendering · {zoom:.0}%"),
            None => "RAW rendering".to_string(),
        },
        (None, Source::Jpeg) => match zoom {
            Some((zoom, true)) => format!("Embedded JPEG · {zoom:.0}% (Fit)"),
            Some((zoom, false)) => format!("Embedded JPEG · {zoom:.0}%"),
            None => "Embedded JPEG".to_string(),
        },
    }
}

/// The status-line part after the source text when the image's embedded colour profile could not
/// be used (ADR 0016 §5).
pub const PROFILE_UNUSABLE_TEXT: &str = "The embedded color profile could not be used";

/// The status line of the loupe. It starts with `position_text`, as in the grid (ADR 0014 §5.3,
/// amendment 1): the core holds the visible list, so `frame.count` is the number shown, and `total`
/// is the number of images in the folder ("3 / 12 (of 31)", or "3 / 31" without a filter).
pub fn status_line(frame: &Frame, anchor_at_pointer: bool, total: usize) -> String {
    let Some(pos) = frame.pos else {
        return "No folder open. Drag a folder or an image here, or give it on the command line."
            .to_string();
    };
    let anchor = if anchor_at_pointer {
        "zoom to pointer"
    } else {
        "zoom to center"
    };
    let what = match &frame.status {
        Status::Error(_) => "cannot be shown".to_string(),
        Status::Preparing { .. } if frame.size.is_none() => match frame.source {
            Source::Raw => "preparing RAW".to_string(),
            Source::Jpeg if is_tiff(frame) => "preparing TIFF".to_string(),
            Source::Jpeg => "preparing JPEG".to_string(),
        },
        // ADR 0016 §5: the colours rest on a fallback because the file's own profile was broken
        _ if frame.profile_unusable => {
            status_line_text(&[source_text(frame), PROFILE_UNUSABLE_TEXT.to_string()])
        }
        _ => source_text(frame),
    };
    status_line_text(&[
        position_text(pos, frame.count, total),
        file_name(frame),
        what,
        zoom_text(frame),
        anchor.to_string(),
    ])
}

/// The position part of the status line (ADR 0014 §5.3): "3 / 12 (of 31)" under a filter, or
/// "3 / 31" when every image is shown. `pos` is the zero-based position in the visible list.
pub fn position_text(pos: usize, shown: usize, total: usize) -> String {
    let number = pos.saturating_add(1);
    if shown == total {
        format!("{number} / {total}")
    } else {
        format!("{number} / {shown} (of {total})")
    }
}

/// The text when the filter lets nothing through (ADR 0014 §2.4, §5.3), in the status line and the
/// empty grid.
pub fn no_match_text(total: usize) -> String {
    format!("No images match the filter ({total} in the folder)")
}

/// The marking part of the status line (ADR 0014 §5.3): only when more than one image is marked.
pub fn marked_text(n: usize) -> Option<String> {
    (n > 1).then(|| format!("{n} marked"))
}

/// Shown in the grid and the status line while an active filter waits for the ratings (Q7).
pub const READING_RATINGS_TEXT: &str = "Reading ratings …";

/// The neutral loading indicator (V2): no image content, only what is being prepared.
pub fn indicator_text(frame: &Frame) -> String {
    match frame.source {
        Source::Raw => "Preparing RAW …".to_string(),
        Source::Jpeg if is_tiff(frame) => "Preparing TIFF …".to_string(),
        Source::Jpeg => "Preparing JPEG …".to_string(),
    }
}

/// The message for a file that cannot be shown (G3, G4).
pub fn error_text(e: &DecodeError, mode: Source) -> String {
    if e.damaged {
        "RAW data is missing or damaged. Switch to JPEG mode (R) to see the camera's preview."
            .to_string()
    } else if mode == Source::Jpeg && e.no_embedded_jpeg {
        "The file has no embedded JPEG. Switch to RAW mode (R).".to_string()
    } else {
        format!("The file cannot be shown: {}", e.message)
    }
}

/// The label of the display-mode control (G4: "with a key and a control").
pub fn mode_label(mode: Source) -> &'static str {
    match mode {
        Source::Raw => "Mode: RAW (R)",
        Source::Jpeg => "Mode: JPEG (R)",
    }
}

/// The text of a contact-sheet or filmstrip cell whose file has no embedded preview
/// (`CellState::NoPreview`).
pub fn no_preview_text() -> &'static str {
    "no preview"
}

/// The text of a cell whose thumbnail failed: the error, shortened to 32 characters with "…".
pub fn failed_cell_text(error: &str) -> String {
    let shortened: String = error.chars().take(32).collect();
    if error.chars().count() > 32 {
        format!("{shortened}…")
    } else {
        shortened
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_viewcore::ImageSize;
    use std::path::PathBuf;
    use std::time::Instant;

    fn frame(status: Status, source: Source) -> Frame {
        Frame {
            draw: None,
            status,
            pos: Some(2),
            count: 31,
            path: Some(PathBuf::from("D:/RAW/TN100423.ARW")),
            mode: source,
            source,
            size: Some(ImageSize {
                w: 9568,
                h: 6376,
                flip: 5,
            }),
            zoom: Zoom::Fit,
            scale: Some(0.1129),
            centre: [0.5, 0.5],
            profile_unusable: false,
            statistics: None,
            clipping: pw_viewcore::ClippingState::NotWanted,
        }
    }

    #[test]
    fn the_status_line_says_what_is_shown() {
        let f = frame(Status::Ready, Source::Raw);
        let s = status_line(&f, true, 31);
        assert!(s.starts_with("3 / 31  ·  "), "{s}");
        assert!(s.contains("TN100423.ARW"));
        assert!(s.contains("RAW rendering (LibRaw, PPG) 6376 × 9568"), "{s}");
        assert!(s.contains("Zoom 11% (Fit)"));
        assert!(s.contains("zoom to pointer"));
        let j = frame(Status::Ready, Source::Jpeg);
        assert!(status_line(&j, false, 31).contains("Embedded JPEG 6376 × 9568 (camera rendering"));
        let mut plain = frame(Status::Ready, Source::Jpeg);
        plain.path = Some(PathBuf::from("photo.JPG"));
        assert!(source_text(&plain).starts_with("JPEG file"));
        let mut z = frame(Status::Ready, Source::Raw);
        z.zoom = Zoom::Scale(2.0);
        assert_eq!(zoom_text(&z), "Zoom 200%");
        let mut empty = frame(Status::Empty, Source::Raw);
        empty.pos = None;
        assert!(status_line(&empty, true, 31).starts_with("No folder"));
    }

    #[test]
    fn the_loupe_status_line_starts_with_the_position_in_the_filter() {
        // Without a filter the core shows the whole folder: "3 / 31".
        let all = frame(Status::Ready, Source::Raw);
        assert_eq!(
            status_line(&all, true, 31),
            "3 / 31  ·  TN100423.ARW  ·  RAW rendering (LibRaw, PPG) 6376 × 9568  ·  \
             Zoom 11% (Fit)  ·  zoom to pointer"
        );
        // Under a filter the core shows the 12 visible images of 31 (ADR 0014 §5.3).
        let mut filtered = frame(Status::Ready, Source::Raw);
        filtered.count = 12;
        let s = status_line(&filtered, false, 31);
        assert!(s.starts_with("3 / 12 (of 31)  ·  TN100423.ARW  ·  "), "{s}");
        assert!(s.ends_with("zoom to center"), "{s}");
    }

    #[test]
    fn preparing_and_errors_are_described_without_image_content() {
        let mut f = frame(
            Status::Preparing {
                since: Instant::now(),
                indicator: true,
            },
            Source::Raw,
        );
        f.size = None;
        assert!(status_line(&f, true, 31).contains("preparing RAW"));
        assert_eq!(indicator_text(&f), "Preparing RAW …");
        let damaged = DecodeError::damaged("damaged: cut");
        assert!(error_text(&damaged, Source::Raw).contains("JPEG mode"));
        let none = DecodeError::new("x: the file has no embedded JPEG");
        assert!(error_text(&none, Source::Jpeg).contains("no embedded JPEG"));
        assert!(error_text(&DecodeError::new("boom"), Source::Raw).contains("boom"));
        assert_eq!(mode_label(Source::Jpeg), "Mode: JPEG (R)");
    }

    /// The message for a file without an embedded JPEG follows the typed kind of `pw-viewcore`
    /// (T118), not the error's text; the texts are the same as before.
    #[test]
    fn the_no_embedded_jpeg_message_follows_the_typed_kind() {
        let none = DecodeError::no_embedded_jpeg("x: the file has no embedded JPEG");
        assert_eq!(
            error_text(&none, Source::Jpeg),
            "The file has no embedded JPEG. Switch to RAW mode (R)."
        );
        assert_eq!(
            error_text(&none, Source::Raw),
            "The file cannot be shown: x: the file has no embedded JPEG"
        );
        // the same words without the kind are an ordinary error
        let text_only = DecodeError::new("x: the file has no embedded JPEG");
        assert_eq!(
            error_text(&text_only, Source::Jpeg),
            "The file cannot be shown: x: the file has no embedded JPEG"
        );
    }

    fn tiff_frame(status: Status, mode: Source) -> Frame {
        Frame {
            draw: None,
            status,
            pos: Some(0),
            count: 9,
            path: Some(PathBuf::from("D:/Scans/Scan 01.tif")),
            mode,
            source: Source::Jpeg,
            size: Some(ImageSize {
                w: 7741,
                h: 5161,
                flip: 6,
            }),
            zoom: Zoom::Fit,
            scale: Some(0.2),
            centre: [0.5, 0.5],
            profile_unusable: false,
            statistics: None,
            clipping: pw_viewcore::ClippingState::NotWanted,
        }
    }

    #[test]
    fn a_tiff_frame_is_named_tiff_file_with_its_oriented_size_in_both_modes() {
        for mode in [Source::Raw, Source::Jpeg] {
            let f = tiff_frame(Status::Ready, mode);
            assert_eq!(source_text(&f), "TIFF file 5161 × 7741", "{mode:?}");
            let s = status_line(&f, true, 9);
            assert!(
                s.contains("Scan 01.tif  ·  TIFF file 5161 × 7741  ·  "),
                "{s}"
            );
        }
    }

    #[test]
    fn a_preparing_tiff_frame_says_tiff() {
        let mut f = tiff_frame(
            Status::Preparing {
                since: Instant::now(),
                indicator: true,
            },
            Source::Raw,
        );
        f.size = None;
        let s = status_line(&f, true, 9);
        assert!(s.contains("preparing TIFF"), "{s}");
        assert!(!s.contains("JPEG"), "{s}");
        assert_eq!(indicator_text(&f), "Preparing TIFF …");
    }

    #[test]
    fn a_jpeg_file_keeps_its_texts() {
        let mut f = frame(
            Status::Preparing {
                since: Instant::now(),
                indicator: true,
            },
            Source::Jpeg,
        );
        f.path = Some(PathBuf::from("D:/Photos/photo.JPG"));
        assert_eq!(indicator_text(&f), "Preparing JPEG …");
        assert_eq!(source_text(&f), "JPEG file 6376 × 9568");
        f.size = None;
        assert!(status_line(&f, true, 31).contains("preparing JPEG"));
    }

    #[test]
    fn the_filter_texts_of_the_status_line() {
        assert_eq!(position_text(2, 12, 31), "3 / 12 (of 31)");
        assert_eq!(position_text(2, 31, 31), "3 / 31");
        assert_eq!(position_text(0, 2, 12), "1 / 2 (of 12)");
        assert_eq!(
            no_match_text(31),
            "No images match the filter (31 in the folder)"
        );
        assert_eq!(
            no_match_text(1),
            "No images match the filter (1 in the folder)"
        );
        assert_eq!(marked_text(0), None);
        assert_eq!(marked_text(1), None);
        assert_eq!(marked_text(2).as_deref(), Some("2 marked"));
        assert_eq!(marked_text(12).as_deref(), Some("12 marked"));
        assert_eq!(READING_RATINGS_TEXT, "Reading ratings …");
    }

    // ---------- an embedded profile that cannot be used (ADR 0016 §5, T73) ----------

    fn small_tiff_frame(status: Status) -> Frame {
        let mut f = tiff_frame(status, Source::Jpeg);
        f.size = Some(ImageSize {
            w: 6,
            h: 4,
            flip: 0,
        });
        f
    }

    #[test]
    fn the_status_line_says_when_the_embedded_profile_cannot_be_used() {
        let mut f = small_tiff_frame(Status::Ready);
        let without = status_line(&f, true, 9);
        f.profile_unusable = true;
        let with = status_line(&f, true, 9);
        assert!(
            with.contains("TIFF file 6 × 4  ·  The embedded color profile could not be used  ·  "),
            "{with}"
        );
        // nothing else in the line changes
        assert_eq!(
            with.replace("  ·  The embedded color profile could not be used", ""),
            without
        );

        let mut j = frame(Status::Ready, Source::Jpeg);
        j.path = Some(PathBuf::from("D:/RAW/TN005701_DxO.jpg"));
        j.profile_unusable = true;
        let s = status_line(&j, false, 31);
        assert!(
            s.contains(
                "JPEG file 6376 × 9568  ·  The embedded color profile could not be used  ·  "
            ),
            "{s}"
        );
    }

    #[test]
    fn without_the_flag_the_status_line_is_unchanged() {
        let f = small_tiff_frame(Status::Ready);
        assert!(!f.profile_unusable);
        assert_eq!(
            status_line(&f, true, 9),
            "1 / 9  ·  Scan 01.tif  ·  TIFF file 6 × 4  ·  Zoom 20% (Fit)  ·  zoom to pointer"
        );
    }

    #[test]
    fn preparing_and_error_frames_do_not_show_the_profile_part() {
        let mut preparing = small_tiff_frame(Status::Preparing {
            since: Instant::now(),
            indicator: true,
        });
        preparing.size = None;
        preparing.profile_unusable = true;
        let s = status_line(&preparing, true, 9);
        assert!(s.contains("preparing TIFF"), "{s}");
        assert!(!s.contains(PROFILE_UNUSABLE_TEXT), "{s}");

        let mut error = small_tiff_frame(Status::Error(DecodeError::new("boom")));
        error.profile_unusable = true;
        let s = status_line(&error, true, 9);
        assert!(s.contains("cannot be shown"), "{s}");
        assert!(!s.contains(PROFILE_UNUSABLE_TEXT), "{s}");
    }

    #[test]
    fn the_cell_texts() {
        assert_eq!(no_preview_text(), "no preview");
        assert_eq!(failed_cell_text("boom"), "boom");
        let long = "x".repeat(40);
        assert_eq!(failed_cell_text(&long), format!("{}…", "x".repeat(32)));
        assert_eq!(failed_cell_text(&"x".repeat(32)), "x".repeat(32));
    }

    // ---------- the side line of the compare (ADR 0017 U6-Q10, T151) ----------

    /// A frame of each of the four sources (RAW rendering, embedded JPEG, JPEG file, TIFF file)
    /// with the start of its side line.
    fn four_sources() -> [(Frame, &'static str); 4] {
        let mut plain = frame(Status::Ready, Source::Jpeg);
        plain.path = Some(PathBuf::from("D:/Photos/photo.JPG"));
        [
            (
                frame(Status::Ready, Source::Raw),
                "TN100423.ARW · RAW rendering",
            ),
            (
                frame(Status::Ready, Source::Jpeg),
                "TN100423.ARW · Embedded JPEG",
            ),
            (plain, "photo.JPG · JPEG file"),
            (
                tiff_frame(Status::Ready, Source::Raw),
                "Scan 01.tif · TIFF file",
            ),
        ]
    }

    #[test]
    fn the_side_line_names_the_file_the_source_and_the_zoom_at_fit() {
        let [raw, embedded, plain, tiff] = four_sources();
        assert_eq!(
            compare_side_text(&raw.0),
            "TN100423.ARW · RAW rendering · 11% (Fit)"
        );
        assert_eq!(
            compare_side_text(&embedded.0),
            "TN100423.ARW · Embedded JPEG · 11% (Fit)"
        );
        assert_eq!(
            compare_side_text(&plain.0),
            "photo.JPG · JPEG file · 11% (Fit)"
        );
        assert_eq!(
            compare_side_text(&tiff.0),
            "Scan 01.tif · TIFF file · 20% (Fit)"
        );
    }

    #[test]
    fn the_side_line_at_100_percent_has_no_fit() {
        for (mut f, start) in four_sources() {
            f.zoom = Zoom::Scale(1.0);
            assert_eq!(compare_side_text(&f), format!("{start} · 100%"));
            f.zoom = Zoom::Scale(0.375);
            assert_eq!(compare_side_text(&f), format!("{start} · 38%"));
        }
    }

    #[test]
    fn the_side_line_has_the_file_name_without_the_folder_and_no_pixel_size() {
        for (f, _) in four_sources() {
            let s = compare_side_text(&f);
            assert!(!s.contains("D:"), "{s}");
            assert!(!s.contains('/'), "{s}");
            assert!(!s.contains('×'), "{s}");
            assert!(!s.contains("6376"), "{s}");
            assert!(!s.contains("Zoom"), "{s}");
        }
    }

    #[test]
    fn the_side_line_without_a_path_has_no_file_name() {
        let mut raw = frame(Status::Empty, Source::Raw);
        raw.path = None;
        assert_eq!(compare_side_text(&raw), "RAW rendering · 11% (Fit)");
        raw.zoom = Zoom::Scale(1.0);
        assert_eq!(compare_side_text(&raw), "RAW rendering · 100%");
        let mut jpeg = frame(Status::Empty, Source::Jpeg);
        jpeg.path = None;
        assert_eq!(compare_side_text(&jpeg), "Embedded JPEG · 11% (Fit)");
        jpeg.zoom = Zoom::Scale(1.0);
        assert_eq!(compare_side_text(&jpeg), "Embedded JPEG · 100%");
    }

    #[test]
    fn the_side_line_without_a_scale_has_no_zoom() {
        for (mut f, start) in four_sources() {
            f.scale = None;
            assert_eq!(compare_side_text(&f), start);
            // a chosen scale is known without the image's size
            f.zoom = Zoom::Scale(2.0);
            assert_eq!(compare_side_text(&f), format!("{start} · 200%"));
        }
        // neither a path nor a scale
        let mut raw = frame(Status::Empty, Source::Raw);
        raw.path = None;
        raw.scale = None;
        assert_eq!(compare_side_text(&raw), "RAW rendering");
        let mut jpeg = frame(Status::Empty, Source::Jpeg);
        jpeg.path = None;
        jpeg.scale = None;
        assert_eq!(compare_side_text(&jpeg), "Embedded JPEG");
    }
}
