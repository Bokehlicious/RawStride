//! The texts of the preview (ADR 0017 §2.2, U3, T111; rule 13): its right column's Zoom section
//! and the filmstrip's toggle "only the marked". Each text is made here, whole; the drawing code in
//! `main.rs` holds none of them. The Info section's texts are the inspector's
//! (`inspector_text.rs`), the title line is the loupe's status line (`lib.rs`). The texts of the
//! Histogram section (ADR 0017 §2.6, U7, T142) are here too; the inspector shows the same section
//! with the same texts. The texts of the Clipping section (U7b, T148) are here as well.

use pw_viewcore::{ClippingState, Zoom};

use crate::shell::KeyChord;
use crate::shell_text::key_text;

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

/// The text of `text` with the key the key map gives its action (ADR 0017 U8-Q8, T280): "Fit
/// (F)", "Fit ↔ 100% (Z)", "Zoom In (+)", "Zoom Out (−)", "Zoom at Pointer (A)", "Marked only
/// (S)" and "Collapse Column (I)" with the key given, without the parentheses when the action has
/// no key. A text without a key (the section's heading, the hover text and "Show Column (I)",
/// which is drawn nowhere) is [`preview_text`]'s.
pub fn preview_text_with_key(text: PreviewText, key: Option<KeyChord>) -> String {
    let name = match text {
        PreviewText::Fit => "Fit",
        PreviewText::ToggleZoom => "Fit ↔ 100%",
        PreviewText::ZoomIn => "Zoom In",
        PreviewText::ZoomOut => "Zoom Out",
        PreviewText::ZoomAtPointer => "Zoom at Pointer",
        PreviewText::OnlyMarked => "Marked only",
        PreviewText::CollapseColumn => "Collapse Column",
        PreviewText::ZoomSection | PreviewText::OnlyMarkedHover | PreviewText::ExpandColumn => {
            return preview_text(text).to_owned();
        }
    };
    match key {
        Some(chord) => {
            let key = key_text(chord);
            format!("{name} ({key})")
        }
        None => name.to_owned(),
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

/// A fixed text of the Clipping section of the preview's right column (ADR 0017 §2.6, U7b-Q9,
/// U7b-Q10, T148).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClippingText {
    /// The section's heading.
    Section,
    /// The check boxes of the two overlays, each with its key.
    Highlights,
    Shadows,
    /// The file has no clipping map.
    NoMap,
}

/// The text of `text`.
pub fn clipping_text(text: ClippingText) -> &'static str {
    match text {
        ClippingText::Section => "Clipping",
        ClippingText::Highlights => "Clipped highlights (J)",
        ClippingText::Shadows => "Deep shadows (Shift+J)",
        ClippingText::NoMap => "No clipping map for this file",
    }
}

/// The text of `text` with the key the key map gives its action (ADR 0017 U8-Q8, T280):
/// "Clipped highlights (J)" and "Deep shadows (Shift+J)" with the key given, without the
/// parentheses when the action has no key. The heading and the line without a map are
/// [`clipping_text`]'s.
pub fn clipping_text_with_key(text: ClippingText, key: Option<KeyChord>) -> String {
    let name = match text {
        ClippingText::Highlights => "Clipped highlights",
        ClippingText::Shadows => "Deep shadows",
        ClippingText::Section | ClippingText::NoMap => return clipping_text(text).to_owned(),
    };
    match key {
        Some(chord) => {
            let key = key_text(chord);
            format!("{name} ({key})")
        }
        None => name.to_owned(),
    }
}

/// The line under the Clipping section's check boxes for the clipping state of the last frame
/// (U7b-Q9): "Reading …" (the Histogram section's text) while the map is being made, "No clipping
/// map for this file" when the file has none, and nothing when no overlay wants a map or the map
/// is ready.
pub fn clipping_state_text(state: &ClippingState) -> Option<&'static str> {
    match state {
        ClippingState::NotWanted | ClippingState::Ready { .. } => None,
        ClippingState::Reading => Some(histogram_text(HistogramText::Reading)),
        ClippingState::Unavailable => Some(clipping_text(ClippingText::NoMap)),
    }
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
    fn every_fixed_text_of_the_clipping_section() {
        let texts = [
            (ClippingText::Section, "Clipping"),
            (ClippingText::Highlights, "Clipped highlights (J)"),
            (ClippingText::Shadows, "Deep shadows (Shift+J)"),
            (ClippingText::NoMap, "No clipping map for this file"),
        ];
        for (text, expected) in texts {
            assert_eq!(clipping_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn the_clipping_state_has_a_line_only_while_reading_or_without_a_map() {
        use pw_viewcore::ClippingMap;
        use std::sync::Arc;
        assert_eq!(clipping_state_text(&ClippingState::NotWanted), None);
        assert_eq!(
            clipping_state_text(&ClippingState::Reading),
            Some("Reading …")
        );
        assert_eq!(
            clipping_state_text(&ClippingState::Unavailable),
            Some("No clipping map for this file")
        );
        let map = ClippingMap::of_rgb8(2, 2, &[255; 12]).map(Arc::new);
        let Some(map) = map else {
            panic!("a 2 × 2 image has a clipping map");
        };
        let ready = ClippingState::Ready {
            map,
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        };
        assert_eq!(clipping_state_text(&ready), None);
    }

    #[test]
    fn the_zoom_value_is_fitted_with_its_percent_or_a_percent() {
        assert_eq!(zoom_value_text(Zoom::Fit, Some(0.3404)), "Fit (34%)");
        assert_eq!(zoom_value_text(Zoom::Fit, None), "Fit");
        assert_eq!(zoom_value_text(Zoom::Scale(1.0), Some(1.0)), "100%");
        assert_eq!(zoom_value_text(Zoom::Scale(2.0), None), "200%");
        assert_eq!(zoom_value_text(Zoom::Scale(0.126), Some(0.126)), "13%");
    }

    /// The default map's keys give exactly today's texts; a moved key is shown, and an action
    /// without a key has no parentheses (T280).
    #[test]
    fn the_preview_texts_show_the_maps_keys() {
        use crate::keymap::{KeyAction, KeyMap};
        let map = KeyMap::default();
        let preview = [
            (PreviewText::Fit, KeyAction::Fit),
            (PreviewText::ToggleZoom, KeyAction::ZoomToggle),
            (PreviewText::ZoomIn, KeyAction::ZoomIn),
            (PreviewText::ZoomOut, KeyAction::ZoomOut),
            (PreviewText::ZoomAtPointer, KeyAction::ZoomAtPointer),
            (PreviewText::OnlyMarked, KeyAction::MarkedOnly),
            (PreviewText::CollapseColumn, KeyAction::Inspector),
        ];
        for (text, action) in preview {
            assert_eq!(
                preview_text_with_key(text, map.first_key(action)),
                preview_text(text),
                "{text:?}"
            );
        }
        for (text, action) in [
            (ClippingText::Highlights, KeyAction::ClippedHighlights),
            (ClippingText::Shadows, KeyAction::DeepShadows),
        ] {
            assert_eq!(
                clipping_text_with_key(text, map.first_key(action)),
                clipping_text(text),
                "{text:?}"
            );
        }
        let q = KeyChord {
            key: egui::Key::Q,
            ctrl: false,
            shift: false,
            alt: false,
        };
        let ctrl_q = KeyChord { ctrl: true, ..q };
        assert_eq!(preview_text_with_key(PreviewText::Fit, Some(q)), "Fit (Q)");
        assert_eq!(preview_text_with_key(PreviewText::Fit, None), "Fit");
        assert_eq!(
            preview_text_with_key(PreviewText::ToggleZoom, Some(ctrl_q)),
            "Fit ↔ 100% (Ctrl+Q)"
        );
        assert_eq!(preview_text_with_key(PreviewText::ZoomIn, None), "Zoom In");
        assert_eq!(
            preview_text_with_key(PreviewText::ZoomOut, None),
            "Zoom Out"
        );
        assert_eq!(
            preview_text_with_key(PreviewText::ZoomAtPointer, None),
            "Zoom at Pointer"
        );
        assert_eq!(
            preview_text_with_key(PreviewText::OnlyMarked, Some(q)),
            "Marked only (Q)"
        );
        assert_eq!(
            preview_text_with_key(PreviewText::CollapseColumn, None),
            "Collapse Column"
        );
        // Texts without a key stay as they are, "Show Column (I)" included.
        assert_eq!(
            preview_text_with_key(PreviewText::ExpandColumn, None),
            "Show Column (I)"
        );
        assert_eq!(
            preview_text_with_key(PreviewText::ZoomSection, Some(q)),
            "Zoom"
        );
        assert_eq!(
            clipping_text_with_key(ClippingText::Highlights, Some(q)),
            "Clipped highlights (Q)"
        );
        assert_eq!(
            clipping_text_with_key(ClippingText::Shadows, None),
            "Deep shadows"
        );
        assert_eq!(
            clipping_text_with_key(ClippingText::Section, Some(q)),
            "Clipping"
        );
    }
}
