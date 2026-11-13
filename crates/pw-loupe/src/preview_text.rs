//! The texts of the preview (ADR 0017 §2.2, U3, T111; rule 13): its right column's Zoom section
//! and the filmstrip's toggle "only the marked". Each text is made here, whole; the drawing code in
//! `main.rs` holds none of them. The Info section's texts are the inspector's
//! (`inspector_text.rs`), the title line is the loupe's status line (`lib.rs`).

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
        PreviewText::Fit => "Anpassa (F)",
        PreviewText::ToggleZoom => "Anpassa ↔ 100 % (Z)",
        PreviewText::ZoomIn => "Zooma in (+)",
        PreviewText::ZoomOut => "Zooma ut (−)",
        PreviewText::ZoomAtPointer => "Zooma vid pekaren (A)",
        PreviewText::OnlyMarked => "Bara markerade (S)",
        PreviewText::OnlyMarkedHover => {
            "Visa och stega bara mellan de markerade bilderna, när minst två är markerade."
        }
        PreviewText::CollapseColumn => "Fäll ihop kolumnen (I)",
        PreviewText::ExpandColumn => "Visa kolumnen (I)",
    }
}

/// The zoom of the shown image in the Zoom section (U3-Q5): "Anpassad (34 %)" at fit (only
/// "Anpassad" before the image's size is known), otherwise "100 %", "200 %". `scale` is the
/// effective scale with fit resolved (`Frame::scale`).
pub fn zoom_value_text(zoom: Zoom, scale: Option<f64>) -> String {
    match (zoom, scale) {
        (Zoom::Fit, Some(scale)) => format!("Anpassad ({:.0} %)", scale * 100.0),
        (Zoom::Fit, None) => "Anpassad".to_owned(),
        (Zoom::Scale(scale), _) => format!("{:.0} %", scale * 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixed_text_of_the_preview() {
        let texts = [
            (PreviewText::ZoomSection, "Zoom"),
            (PreviewText::Fit, "Anpassa (F)"),
            (PreviewText::ToggleZoom, "Anpassa ↔ 100 % (Z)"),
            (PreviewText::ZoomIn, "Zooma in (+)"),
            (PreviewText::ZoomOut, "Zooma ut (−)"),
            (PreviewText::ZoomAtPointer, "Zooma vid pekaren (A)"),
            (PreviewText::OnlyMarked, "Bara markerade (S)"),
            (
                PreviewText::OnlyMarkedHover,
                "Visa och stega bara mellan de markerade bilderna, när minst två är markerade.",
            ),
            (PreviewText::CollapseColumn, "Fäll ihop kolumnen (I)"),
            (PreviewText::ExpandColumn, "Visa kolumnen (I)"),
        ];
        for (text, expected) in texts {
            assert_eq!(preview_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn the_zoom_value_is_fitted_with_its_percent_or_a_percent() {
        assert_eq!(zoom_value_text(Zoom::Fit, Some(0.3404)), "Anpassad (34 %)");
        assert_eq!(zoom_value_text(Zoom::Fit, None), "Anpassad");
        assert_eq!(zoom_value_text(Zoom::Scale(1.0), Some(1.0)), "100 %");
        assert_eq!(zoom_value_text(Zoom::Scale(2.0), None), "200 %");
        assert_eq!(zoom_value_text(Zoom::Scale(0.126), Some(0.126)), "13 %");
    }
}
