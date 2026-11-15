//! The texts of the preview (ADR 0017 §2.2, U3, T111; rule 13): its right column's Zoom section
//! and the filmstrip's toggle "only the marked". Each text is made here, whole; the drawing code in
//! `main.rs` holds none of them. The Info section's texts are the inspector's
//! (`inspector_text.rs`), the title line is the loupe's status line (`lib.rs`). The texts of the
//! Histogram section (ADR 0017 §2.6, U7, T142) are here too; the inspector shows the same section
//! with the same texts.

use pw_viewcore::Zoom;

/// A fixed text of the preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewText {
    /// The heading of the right column's Zoom section.
    ZoomSection,
    /// The Zoom section's buttons and check box, each with its key (U3-Q5).
    Fit,
    ToggleZoom,
    ZoomIn,
    ZoomOut,
    ZoomAtPointer,
    /// The toggle at the left end of the filmstrip (`S`, U3-Q3) and its hover text.
    OnlyMarked,
    OnlyMarkedHover,
    /// The hover texts of the right column's collapse and open buttons.
    CollapseColumn,
    ExpandColumn,
}

/// The text of `text`.
pub fn preview_text(text: PreviewText) -> &'static str {
    match text {
        PreviewText::ZoomSection => "Zoom",
        PreviewText::Fit => "Fit (F)",
        PreviewText::ToggleZoom => "Fit ↔ 100% (Z)",
        PreviewText::ZoomIn => "Zoom In (+)",
        PreviewText::ZoomOut => "Zoom Out (−)",
        PreviewText::ZoomAtPointer => "Zoom at Pointer (A)",
        PreviewText::OnlyMarked => "Marked only (S)",
        PreviewText::OnlyMarkedHover => {
            "Show and step only between the marked images, when at least two are marked."
        }
        PreviewText::CollapseColumn => "Collapse Column (I)",
        PreviewText::ExpandColumn => "Show Column (I)",
    }
}

/// A fixed text of the Histogram section (U7-Q8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistogramText {
    /// The section's heading.
    Section,
    /// Where the numbers come from (G1): the unpacked RAW data, the embedded JPEG, or the TIFF
    /// file's own image.
    RawData,
    EmbeddedJpeg,
    Tiff,
    /// The statistics have not come yet.
    Reading,
    /// The file has no statistics.
    NoHistogram,
}

/// The text of `text`.
pub fn histogram_text(text: HistogramText) -> &'static str {
    match text {
        HistogramText::Section => "Histogram",
        HistogramText::RawData => "RAW data",
        HistogramText::EmbeddedJpeg => "Embedded JPEG",
        HistogramText::Tiff => "TIFF",
        HistogramText::Reading => "Reading …",
        HistogramText::NoHistogram => "No histogram for this file",
    }
}

/// The share of each channel's pixels at the clipping level, under the histogram (U7-Q7):
/// "Clipped: R 0.12 %  G 3.40 %  B 0.00 %". The shares are from 0 to 1 and are shown in percent
/// with two decimals.
pub fn clipped_text(r: f64, g: f64, b: f64) -> String {
    format!(
        "Clipped: R {:.2} %  G {:.2} %  B {:.2} %",
        r * 100.0,
        g * 100.0,
        b * 100.0
    )
}

/// The share of the pixels in the deepest shadows, under the histogram (U7-Q7): "Deep shadows:
/// 1.25 %". The share is from 0 to 1 and is shown in percent with two decimals.
pub fn deep_shadows_text(share: f64) -> String {
    format!("Deep shadows: {:.2} %", share * 100.0)
}

/// The zoom of the shown image in the Zoom section (U3-Q5): "Fit (34%)" at fit (only
/// "Fit" before the image's size is known), otherwise "100%", "200%". `scale` is the
/// effective scale with fit resolved (`Frame::scale`).
pub fn zoom_value_text(zoom: Zoom, scale: Option<f64>) -> String {
    match (zoom, scale) {
        (Zoom::Fit, Some(scale)) => format!("Fit ({:.0}%)", scale * 100.0),
        (Zoom::Fit, None) => "Fit".to_owned(),
        (Zoom::Scale(scale), _) => format!("{:.0}%", scale * 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixed_text_of_the_preview() {
        let texts = [
            (PreviewText::ZoomSection, "Zoom"),
            (PreviewText::Fit, "Fit (F)"),
            (PreviewText::ToggleZoom, "Fit ↔ 100% (Z)"),
            (PreviewText::ZoomIn, "Zoom In (+)"),
            (PreviewText::ZoomOut, "Zoom Out (−)"),
            (PreviewText::ZoomAtPointer, "Zoom at Pointer (A)"),
            (PreviewText::OnlyMarked, "Marked only (S)"),
            (
                PreviewText::OnlyMarkedHover,
                "Show and step only between the marked images, when at least two are marked.",
            ),
            (PreviewText::CollapseColumn, "Collapse Column (I)"),
            (PreviewText::ExpandColumn, "Show Column (I)"),
        ];
        for (text, expected) in texts {
            assert_eq!(preview_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn every_fixed_text_of_the_histogram_section() {
        let texts = [
            (HistogramText::Section, "Histogram"),
            (HistogramText::RawData, "RAW data"),
            (HistogramText::EmbeddedJpeg, "Embedded JPEG"),
            (HistogramText::Tiff, "TIFF"),
            (HistogramText::Reading, "Reading …"),
            (HistogramText::NoHistogram, "No histogram for this file"),
        ];
        for (text, expected) in texts {
            assert_eq!(histogram_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn the_clipped_shares_are_percents_with_two_decimals() {
        assert_eq!(
            clipped_text(0.0012, 0.034, 0.0),
            "Clipped: R 0.12 %  G 3.40 %  B 0.00 %"
        );
        assert_eq!(
            clipped_text(1.0, 0.5, 0.123456),
            "Clipped: R 100.00 %  G 50.00 %  B 12.35 %"
        );
        // a share too small for two decimals is shown as zero
        assert_eq!(
            clipped_text(0.00004, 0.0, 0.00006),
            "Clipped: R 0.00 %  G 0.00 %  B 0.01 %"
        );
    }

    #[test]
    fn the_deep_shadow_share_is_a_percent_with_two_decimals() {
        assert_eq!(deep_shadows_text(0.0125), "Deep shadows: 1.25 %");
        assert_eq!(deep_shadows_text(0.0), "Deep shadows: 0.00 %");
        assert_eq!(deep_shadows_text(1.0), "Deep shadows: 100.00 %");
        assert_eq!(deep_shadows_text(0.141733), "Deep shadows: 14.17 %");
    }

    #[test]
    fn the_zoom_value_is_fitted_with_its_percent_or_a_percent() {
        assert_eq!(zoom_value_text(Zoom::Fit, Some(0.3404)), "Fit (34%)");
        assert_eq!(zoom_value_text(Zoom::Fit, None), "Fit");
        assert_eq!(zoom_value_text(Zoom::Scale(1.0), Some(1.0)), "100%");
        assert_eq!(zoom_value_text(Zoom::Scale(2.0), None), "200%");
        assert_eq!(zoom_value_text(Zoom::Scale(0.126), Some(0.126)), "13%");
    }
}
