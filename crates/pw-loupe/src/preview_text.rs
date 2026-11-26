//! The texts of the preview (ADR 0017 §2.2, U3, T111; rule 13): its right column's Zoom section
//! and the filmstrip's toggle "only the marked". Each text is made here, whole; the drawing code in
//! `main.rs` holds none of them. The Info section's texts are the inspector's
//! (`inspector_text.rs`), the title line is the loupe's status line (`lib.rs`). The texts of the
//! Histogram section (ADR 0017 §2.6, U7, T142) are here too; the inspector shows the same section
//! with the same texts. The texts of the Clipping section (U7b, T148) are here as well, and the
//! marks of the preview's row of adjustment marks (ADR 0027 W2-Q5, T314; W3-Q6, T318).

use pw_viewcore::{
    AdjustState, ClippingState, Ev, ExposureBase, ExposureMode, ExposureState, FinePause,
    FineState, SharpenState, Sharpening, Zoom,
};

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

/// One mark of the preview's row of adjustment marks (ADR 0027 §2.1, W2-Q5, T314): an
/// adjustment that is on is always visible there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjustmentMark {
    /// The mark itself, for example "Fine".
    pub text: String,
    /// The adjustment is on but paused: the mark is drawn dimmed.
    pub dimmed: bool,
    /// The hover text: what the adjustment does, or why it is paused, with its key.
    pub hover: String,
}

/// The shortcuts shown in the exposure mark's hover text (ADR 0027 W5-Q9, T335); `None` = the
/// action has no key, and the hover text has no "(key)".
#[derive(Clone, Copy, Debug, Default)]
pub struct ExposureKeys {
    /// The key of Brighter.
    pub brighter: Option<KeyChord>,
    /// The key of Darker.
    pub darker: Option<KeyChord>,
    /// The key of Auto Exposure.
    pub auto: Option<KeyChord>,
    /// The key of Camera Brightness.
    pub camera: Option<KeyChord>,
}

/// The marks in row order (ADR 0027 §2.1): "Fine", "USM", the exposure, and "Shadows".
/// Off adjustments have no mark. Paused adjustments are dimmed. The shortcuts and values
/// appear in the hover texts when available.
pub fn adjustment_marks(
    fine: &FineState,
    fine_key: Option<KeyChord>,
    sharpen: &SharpenState,
    sharpen_key: Option<KeyChord>,
    values: Sharpening,
    (exposure, exposure_keys): (&ExposureState, ExposureKeys),
    (shadows, shadows_key, shadows_amount): (&AdjustState, Option<KeyChord>, u8),
) -> Vec<AdjustmentMark> {
    let mut marks = Vec::new();
    marks.extend(fine_mark(fine, fine_key));
    marks.extend(sharpen_mark(sharpen, sharpen_key, values));
    marks.extend(exposure_mark(exposure, exposure_keys));
    marks.extend(shadows_mark(*shadows, shadows_key, shadows_amount));
    marks
}

/// Fine details' mark "Fine", or `None` while it is off.
fn fine_mark(fine: &FineState, key: Option<KeyChord>) -> Option<AdjustmentMark> {
    let (dimmed, hover) = match (fine, key) {
        (FineState::Off, _) => return None,
        (FineState::Waiting | FineState::Shown, Some(chord)) => {
            let key = key_text(chord);
            (
                false,
                format!(
                    "Fine details ({key}): below 50%, the RAW view is made from the \
                     full-resolution rendering."
                ),
            )
        }
        (FineState::Waiting | FineState::Shown, None) => (
            false,
            "Fine details: below 50%, the RAW view is made from the full-resolution rendering."
                .to_owned(),
        ),
        (FineState::Paused(FinePause::Mode), Some(chord)) => {
            let key = key_text(chord);
            (
                true,
                format!("Fine details ({key}) is paused: it applies only to RAW renderings."),
            )
        }
        (FineState::Paused(FinePause::Mode), None) => (
            true,
            "Fine details is paused: it applies only to RAW renderings.".to_owned(),
        ),
        (FineState::Paused(FinePause::Zoom), Some(chord)) => {
            let key = key_text(chord);
            (
                true,
                format!("Fine details ({key}) is paused: it applies only below 50%."),
            )
        }
        (FineState::Paused(FinePause::Zoom), None) => (
            true,
            "Fine details is paused: it applies only below 50%.".to_owned(),
        ),
        (FineState::Paused(FinePause::Failed), Some(chord)) => {
            let key = key_text(chord);
            (
                true,
                format!("Fine details ({key}) could not be applied to this image."),
            )
        }
        (FineState::Paused(FinePause::Failed), None) => (
            true,
            "Fine details could not be applied to this image.".to_owned(),
        ),
    };
    Some(AdjustmentMark {
        text: "Fine".to_owned(),
        dimmed,
        hover,
    })
}

/// The text of sharpening's values, with the key the key map gives the action: "Sharpening (U):
/// amount 80%, radius 0.8 px, threshold 2." (without a key "Sharpening: amount …"). The radius is
/// written in pixels with one decimal. The shown mark and the button "USM" both use it (ADR 0027
/// W3-Q5, T323).
pub fn sharpening_values_text(key: Option<KeyChord>, values: Sharpening) -> String {
    let Sharpening {
        amount,
        radius_tenths,
        threshold,
    } = values;
    let (whole, tenth) = (radius_tenths / 10, radius_tenths % 10);
    match key.map(key_text) {
        Some(key) => format!(
            "Sharpening ({key}): amount {amount}%, radius {whole}.{tenth} px, threshold \
             {threshold}."
        ),
        None => format!(
            "Sharpening: amount {amount}%, radius {whole}.{tenth} px, threshold {threshold}."
        ),
    }
}

/// Sharpening's mark "USM", or `None` while it is off. The radius is written in pixels with one
/// decimal, for example 6 tenths as "0.6". It is never dimmed: sharpening applies at every zoom
/// level (ADR 0027 A-Q8, changed 2026-11-25).
fn sharpen_mark(
    sharpen: &SharpenState,
    key: Option<KeyChord>,
    values: Sharpening,
) -> Option<AdjustmentMark> {
    let (dimmed, hover) = match (sharpen, key) {
        (SharpenState::Off, _) => return None,
        (SharpenState::Shown, _) => (false, sharpening_values_text(key, values)),
        (SharpenState::Waiting, Some(chord)) => {
            let key = key_text(chord);
            (
                false,
                format!("Sharpening ({key}): the view is being made again."),
            )
        }
        (SharpenState::Waiting, None) => (
            false,
            "Sharpening: the view is being made again.".to_owned(),
        ),
    };
    Some(AdjustmentMark {
        text: "USM".to_owned(),
        dimmed,
        hover,
    })
}

/// The signed EV value of the exposure mark (W5-Q9), in exact thirds or one decimal.
fn ev_text(ev: Ev) -> String {
    let milli = ev.milli();
    let sign = ev_sign(f64::from(milli));
    let thirds = (f64::from(milli) * 3.0 / 1000.0).round() as i32;
    if Ev::from_thirds(thirds) == ev {
        let thirds = thirds.abs();
        let whole = thirds / 3;
        let fraction = match thirds % 3 {
            1 => "⅓",
            2 => "⅔",
            _ => "",
        };
        if whole == 0 && thirds != 0 {
            format!("{sign}{fraction}")
        } else {
            format!("{sign}{whole}{fraction}")
        }
    } else {
        let tenths = (f64::from(milli).abs() / 100.0).round();
        format!("{sign}{:.1}", tenths / 10.0)
    }
}

fn ev_sign(value: f64) -> &'static str {
    if value < 0.0 {
        "−"
    } else if value > 0.0 {
        "+"
    } else {
        ""
    }
}

fn exposure_mark(exposure: &ExposureState, keys: ExposureKeys) -> Option<AdjustmentMark> {
    if exposure.view == AdjustState::Off {
        return None;
    }
    let value = ev_text(exposure.total);
    let hover = match (exposure.view, exposure.base) {
        (AdjustState::Paused, _) => {
            "Exposure is paused: it applies only to RAW renderings.".to_owned()
        }
        (AdjustState::Waiting, base)
            if !matches!(
                base,
                ExposureBase::AutoWaiting | ExposureBase::CameraWaiting
            ) =>
        {
            "Exposure: the view is being made again.".to_owned()
        }
        _ => exposure_hover(exposure, keys, &value),
    };
    let text = match exposure.mode {
        ExposureMode::Off => format!("{value} EV"),
        ExposureMode::Auto => format!("{value} EV auto"),
        ExposureMode::Camera
            if exposure.base == ExposureBase::CameraNone && exposure.steps == 0 =>
        {
            "Camera EV: none".to_owned()
        }
        ExposureMode::Camera => format!("{value} EV camera"),
    };
    Some(AdjustmentMark {
        text,
        hover,
        dimmed: exposure.view == AdjustState::Paused,
    })
}

fn exposure_hover(exposure: &ExposureState, keys: ExposureKeys, value: &str) -> String {
    let steps = (exposure.steps != 0).then(|| ev_text(Ev::from_thirds(exposure.steps)));
    match exposure.base {
        ExposureBase::Manual => match (keys.brighter, keys.darker) {
            (Some(brighter), Some(darker)) => {
                let brighter = key_text(brighter);
                let darker = key_text(darker);
                format!("Exposure: {value} EV by hand ({brighter} brighter, {darker} darker).")
            }
            _ => format!("Exposure: {value} EV by hand."),
        },
        ExposureBase::Camera(v) => camera_hover(v, keys.camera.map(key_text), steps.as_deref()),
        ExposureBase::CameraWaiting => match keys.camera.map(key_text) {
            Some(key) => format!("Camera brightness ({key}): the file is being read."),
            None => "Camera brightness: the file is being read.".to_owned(),
        },
        ExposureBase::AutoWaiting => match keys.auto.map(key_text) {
            Some(key) => format!("Auto exposure ({key}): the histogram is being read."),
            None => "Auto exposure: the histogram is being read.".to_owned(),
        },
        ExposureBase::Auto(t) => {
            let base = ev_text(Ev::from_thirds(t));
            match (keys.auto.map(key_text), steps) {
                (Some(key), Some(steps)) => format!(
                    "Auto exposure ({key}): {base} EV from the histogram, plus {steps} EV by hand."
                ),
                (None, Some(steps)) => {
                    format!("Auto exposure: {base} EV from the histogram, plus {steps} EV by hand.")
                }
                (Some(key), None) => {
                    format!("Auto exposure ({key}): {base} EV from the histogram.")
                }
                (None, None) => format!("Auto exposure: {base} EV from the histogram."),
            }
        }
        ExposureBase::AutoClipped => match (keys.auto.map(key_text), steps) {
            (Some(key), Some(steps)) => format!(
                "Auto exposure ({key}): 0 EV, since more than 0.1% of the RAW data is clipped, plus {steps} EV by hand."
            ),
            (None, Some(steps)) => format!(
                "Auto exposure: 0 EV, since more than 0.1% of the RAW data is clipped, plus {steps} EV by hand."
            ),
            (Some(key), None) => format!(
                "Auto exposure ({key}): 0 EV, since more than 0.1% of the RAW data is clipped."
            ),
            (None, None) => {
                "Auto exposure: 0 EV, since more than 0.1% of the RAW data is clipped.".to_owned()
            }
        },
        ExposureBase::AutoNone => match (keys.auto.map(key_text), steps) {
            (Some(key), Some(steps)) => format!(
                "Auto exposure ({key}): 0 EV, since the file has no usable RAW statistics, plus {steps} EV by hand."
            ),
            (None, Some(steps)) => format!(
                "Auto exposure: 0 EV, since the file has no usable RAW statistics, plus {steps} EV by hand."
            ),
            (Some(key), None) => {
                format!("Auto exposure ({key}): 0 EV, since the file has no usable RAW statistics.")
            }
            (None, None) => {
                "Auto exposure: 0 EV, since the file has no usable RAW statistics.".to_owned()
            }
        },
        ExposureBase::CameraNone => match (keys.camera.map(key_text), steps) {
            (Some(key), Some(steps)) => format!(
                "Camera brightness ({key}): the file gives no value, so 0 EV is used, plus {steps} EV by hand."
            ),
            (None, Some(steps)) => format!(
                "Camera brightness: the file gives no value, so 0 EV is used, plus {steps} EV by hand."
            ),
            (Some(key), None) => {
                format!("Camera brightness ({key}): the file gives no value, so 0 EV is used.")
            }
            (None, None) => {
                "Camera brightness: the file gives no value, so 0 EV is used.".to_owned()
            }
        },
    }
}

fn camera_hover(v: f64, key: Option<String>, steps: Option<&str>) -> String {
    let value = format!("{}{:.2}", ev_sign(v), v.abs());
    let min = f64::from(Ev::MIN_MILLI) / 1000.0;
    let max = f64::from(Ev::MAX_MILLI) / 1000.0;
    let limit = if v < min {
        Some(ev_text(Ev::from_ev(min)))
    } else if v > max {
        Some(ev_text(Ev::from_ev(max)))
    } else {
        None
    };
    match (limit, key, steps) {
        (Some(limit), Some(key), Some(steps)) => format!(
            "Camera brightness ({key}): {value} EV from the camera, limited to {limit} EV, plus {steps} EV by hand."
        ),
        (Some(limit), None, Some(steps)) => format!(
            "Camera brightness: {value} EV from the camera, limited to {limit} EV, plus {steps} EV by hand."
        ),
        (Some(limit), Some(key), None) => {
            format!("Camera brightness ({key}): {value} EV from the camera, limited to {limit} EV.")
        }
        (Some(limit), None, None) => {
            format!("Camera brightness: {value} EV from the camera, limited to {limit} EV.")
        }
        (None, Some(key), Some(steps)) => {
            format!(
                "Camera brightness ({key}): {value} EV from the camera, plus {steps} EV by hand."
            )
        }
        (None, None, Some(steps)) => {
            format!("Camera brightness: {value} EV from the camera, plus {steps} EV by hand.")
        }
        (None, Some(key), None) => {
            format!("Camera brightness ({key}): {value} EV from the camera.")
        }
        (None, None, None) => format!("Camera brightness: {value} EV from the camera."),
    }
}

fn shadows_mark(state: AdjustState, key: Option<KeyChord>, amount: u8) -> Option<AdjustmentMark> {
    let hover = match (state, key) {
        (AdjustState::Off, _) => return None,
        (AdjustState::Shown, Some(chord)) => {
            let key = key_text(chord);
            format!("Shadows ({key}): the dark tones are lifted by {amount}.")
        }
        (AdjustState::Shown, None) => format!("Shadows: the dark tones are lifted by {amount}."),
        (AdjustState::Waiting, Some(chord)) => {
            let key = key_text(chord);
            format!("Shadows ({key}): the view is being made again.")
        }
        (AdjustState::Waiting, None) => "Shadows: the view is being made again.".to_owned(),
        (AdjustState::Paused, Some(chord)) => {
            let key = key_text(chord);
            format!("Shadows ({key}) is paused: it applies only to RAW renderings.")
        }
        (AdjustState::Paused, None) => {
            "Shadows is paused: it applies only to RAW renderings.".to_owned()
        }
    };
    Some(AdjustmentMark {
        text: "Shadows".to_owned(),
        hover,
        dimmed: state == AdjustState::Paused,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPOSURE_OFF: ExposureState = ExposureState {
        mode: ExposureMode::Off,
        steps: 0,
        base: ExposureBase::Manual,
        total: Ev::ZERO,
        view: AdjustState::Off,
    };

    const SHADOWS_OFF: (&AdjustState, Option<KeyChord>, u8) = (&AdjustState::Off, None, 50);

    fn exposure_off() -> (&'static ExposureState, ExposureKeys) {
        (&EXPOSURE_OFF, ExposureKeys::default())
    }

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

    /// The row's marks for each state of fine details, with D and with no key (ADR 0027 W2-Q5,
    /// T314).
    #[test]
    fn the_adjustment_marks_follow_fine_details() {
        let d = KeyChord {
            key: egui::Key::D,
            ctrl: false,
            shift: false,
            alt: false,
        };
        let usm_off = (SharpenState::Off, None, Sharpening::DEFAULT);
        for key in [Some(d), None] {
            assert!(
                adjustment_marks(
                    &FineState::Off,
                    key,
                    &usm_off.0,
                    usm_off.1,
                    usm_off.2,
                    exposure_off(),
                    SHADOWS_OFF
                )
                .is_empty()
            );
        }
        let on_d = "Fine details (D): below 50%, the RAW view is made from the full-resolution \
                    rendering.";
        let on =
            "Fine details: below 50%, the RAW view is made from the full-resolution rendering.";
        let cases = [
            (FineState::Waiting, false, on_d, on),
            (FineState::Shown, false, on_d, on),
            (
                FineState::Paused(FinePause::Mode),
                true,
                "Fine details (D) is paused: it applies only to RAW renderings.",
                "Fine details is paused: it applies only to RAW renderings.",
            ),
            (
                FineState::Paused(FinePause::Zoom),
                true,
                "Fine details (D) is paused: it applies only below 50%.",
                "Fine details is paused: it applies only below 50%.",
            ),
            (
                FineState::Paused(FinePause::Failed),
                true,
                "Fine details (D) could not be applied to this image.",
                "Fine details could not be applied to this image.",
            ),
        ];
        for (state, dimmed, with_key, without_key) in cases {
            for (key, hover) in [(Some(d), with_key), (None, without_key)] {
                assert_eq!(
                    adjustment_marks(
                        &state,
                        key,
                        &usm_off.0,
                        usm_off.1,
                        usm_off.2,
                        exposure_off(),
                        SHADOWS_OFF
                    ),
                    vec![AdjustmentMark {
                        text: "Fine".to_owned(),
                        dimmed,
                        hover: hover.to_owned(),
                    }],
                    "{state:?} {key:?}"
                );
            }
        }
        // The default key map's key is D.
        let map = crate::keymap::KeyMap::default();
        assert_eq!(
            map.first_key(crate::keymap::KeyAction::FineDetails),
            Some(d)
        );
    }

    /// The text of sharpening's values, with and without the key, with the default values and with
    /// others (T323).
    #[test]
    fn sharpening_values_text_gives_the_values() {
        let u = Some(KeyChord {
            key: egui::Key::U,
            ctrl: false,
            shift: false,
            alt: false,
        });
        let others = Sharpening {
            amount: 150,
            radius_tenths: 13,
            threshold: 7,
        };
        let cases = [
            (
                Sharpening::DEFAULT,
                "Sharpening (U): amount 80%, radius 0.8 px, threshold 2.",
                "Sharpening: amount 80%, radius 0.8 px, threshold 2.",
            ),
            (
                others,
                "Sharpening (U): amount 150%, radius 1.3 px, threshold 7.",
                "Sharpening: amount 150%, radius 1.3 px, threshold 7.",
            ),
        ];
        for (values, with_key, without_key) in cases {
            assert_eq!(sharpening_values_text(u, values), with_key);
            assert_eq!(sharpening_values_text(None, values), without_key);
        }
    }

    /// The row's mark "USM" for each state of sharpening, with U and with no key, with the
    /// default values and with others, alone and after "Fine" (ADR 0027 W3-Q6, T318).
    #[test]
    fn the_adjustment_marks_follow_sharpening() {
        let u = KeyChord {
            key: egui::Key::U,
            ctrl: false,
            shift: false,
            alt: false,
        };
        let others = Sharpening {
            amount: 150,
            radius_tenths: 13,
            threshold: 7,
        };
        // Both off: no mark.
        for key in [Some(u), None] {
            for values in [Sharpening::DEFAULT, others] {
                assert!(
                    adjustment_marks(
                        &FineState::Off,
                        None,
                        &SharpenState::Off,
                        key,
                        values,
                        exposure_off(),
                        SHADOWS_OFF
                    )
                    .is_empty()
                );
            }
        }
        let usm = |dimmed: bool, hover: &str| AdjustmentMark {
            text: "USM".to_owned(),
            dimmed,
            hover: hover.to_owned(),
        };
        let cases = [
            (
                SharpenState::Shown,
                Sharpening::DEFAULT,
                false,
                "Sharpening (U): amount 80%, radius 0.8 px, threshold 2.",
                "Sharpening: amount 80%, radius 0.8 px, threshold 2.",
            ),
            (
                SharpenState::Shown,
                others,
                false,
                "Sharpening (U): amount 150%, radius 1.3 px, threshold 7.",
                "Sharpening: amount 150%, radius 1.3 px, threshold 7.",
            ),
            (
                SharpenState::Waiting,
                Sharpening::DEFAULT,
                false,
                "Sharpening (U): the view is being made again.",
                "Sharpening: the view is being made again.",
            ),
            (
                SharpenState::Waiting,
                others,
                false,
                "Sharpening (U): the view is being made again.",
                "Sharpening: the view is being made again.",
            ),
        ];
        for (state, values, dimmed, with_key, without_key) in cases {
            for (key, hover) in [(Some(u), with_key), (None, without_key)] {
                // Fine details off: "USM" alone.
                assert_eq!(
                    adjustment_marks(
                        &FineState::Off,
                        None,
                        &state,
                        key,
                        values,
                        exposure_off(),
                        SHADOWS_OFF
                    ),
                    vec![usm(dimmed, hover)],
                    "{state:?} {key:?} {values:?}"
                );
            }
        }
        // Both on: "Fine" and then "USM".
        let marks = adjustment_marks(
            &FineState::Shown,
            None,
            &SharpenState::Shown,
            Some(u),
            Sharpening::DEFAULT,
            exposure_off(),
            SHADOWS_OFF,
        );
        assert_eq!(
            marks
                .iter()
                .map(|mark| mark.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Fine", "USM"]
        );
        assert_eq!(
            marks.get(1),
            Some(&usm(
                false,
                "Sharpening (U): amount 80%, radius 0.8 px, threshold 2."
            ))
        );
        // The button's hover text is the shown mark's text.
        for key in [Some(u), None] {
            for values in [Sharpening::DEFAULT, others] {
                assert_eq!(
                    adjustment_marks(
                        &FineState::Off,
                        None,
                        &SharpenState::Shown,
                        key,
                        values,
                        exposure_off(),
                        SHADOWS_OFF
                    )
                    .first()
                    .map(|mark| mark.hover.clone()),
                    Some(sharpening_values_text(key, values))
                );
            }
        }
        // The default key map's key is U.
        let map = crate::keymap::KeyMap::default();
        assert_eq!(map.first_key(crate::keymap::KeyAction::Sharpening), Some(u));
    }

    fn default_exposure_keys() -> ExposureKeys {
        use crate::keymap::{KeyAction, KeyMap};
        let map = KeyMap::default();
        ExposureKeys {
            brighter: map.first_key(KeyAction::ExposureBrighter),
            darker: map.first_key(KeyAction::ExposureDarker),
            auto: map.first_key(KeyAction::AutoExposure),
            camera: map.first_key(KeyAction::CameraBrightness),
        }
    }

    fn exposure_only(state: &ExposureState, keys: ExposureKeys) -> Vec<AdjustmentMark> {
        adjustment_marks(
            &FineState::Off,
            None,
            &SharpenState::Off,
            None,
            Sharpening::DEFAULT,
            (state, keys),
            SHADOWS_OFF,
        )
    }

    #[test]
    fn ev_text_uses_signed_exact_thirds_or_rounds_to_one_decimal() {
        for (milli, expected) in [
            (0, "0"),
            (333, "+⅓"),
            (667, "+⅔"),
            (1000, "+1"),
            (1333, "+1⅓"),
            (-333, "−⅓"),
            (-667, "−⅔"),
            (-2000, "−2"),
            (3000, "+3"),
            (-510, "−0.5"),
            (-177, "−0.2"),
            (250, "+0.3"),
            (40, "+0.0"),
        ] {
            assert_eq!(ev_text(Ev::from_ev(f64::from(milli) / 1000.0)), expected);
        }
    }

    #[test]
    fn the_manual_exposure_hover_names_both_shortcuts_when_available() {
        let state = ExposureState {
            steps: 4,
            total: Ev::from_thirds(4),
            view: AdjustState::Shown,
            ..EXPOSURE_OFF
        };
        let keys = default_exposure_keys();
        for (keys, hover) in [
            (
                keys,
                "Exposure: +1⅓ EV by hand (Ctrl+↑ brighter, Ctrl+↓ darker).",
            ),
            (
                ExposureKeys {
                    brighter: None,
                    ..keys
                },
                "Exposure: +1⅓ EV by hand.",
            ),
            (
                ExposureKeys {
                    darker: None,
                    ..keys
                },
                "Exposure: +1⅓ EV by hand.",
            ),
            (ExposureKeys::default(), "Exposure: +1⅓ EV by hand."),
        ] {
            assert_eq!(
                exposure_only(&state, keys),
                vec![AdjustmentMark {
                    text: "+1⅓ EV".to_owned(),
                    dimmed: false,
                    hover: hover.to_owned(),
                }]
            );
        }
    }

    #[test]
    fn the_auto_and_camera_marks_explain_the_base_and_manual_steps() {
        let keys = default_exposure_keys();
        for (mode, base, base_ev, no_steps, one_step) in [
            (
                ExposureMode::Auto,
                ExposureBase::Auto(3),
                1.0,
                (
                    "+1 EV auto",
                    "Auto exposure (B): +1 EV from the histogram.",
                    "Auto exposure: +1 EV from the histogram.",
                ),
                (
                    "+1⅓ EV auto",
                    "Auto exposure (B): +1 EV from the histogram, plus +⅓ EV by hand.",
                    "Auto exposure: +1 EV from the histogram, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Auto,
                ExposureBase::AutoClipped,
                0.0,
                (
                    "0 EV auto",
                    "Auto exposure (B): 0 EV, since more than 0.1% of the RAW data is clipped.",
                    "Auto exposure: 0 EV, since more than 0.1% of the RAW data is clipped.",
                ),
                (
                    "+⅓ EV auto",
                    "Auto exposure (B): 0 EV, since more than 0.1% of the RAW data is clipped, plus +⅓ EV by hand.",
                    "Auto exposure: 0 EV, since more than 0.1% of the RAW data is clipped, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Auto,
                ExposureBase::AutoNone,
                0.0,
                (
                    "0 EV auto",
                    "Auto exposure (B): 0 EV, since the file has no usable RAW statistics.",
                    "Auto exposure: 0 EV, since the file has no usable RAW statistics.",
                ),
                (
                    "+⅓ EV auto",
                    "Auto exposure (B): 0 EV, since the file has no usable RAW statistics, plus +⅓ EV by hand.",
                    "Auto exposure: 0 EV, since the file has no usable RAW statistics, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Camera,
                ExposureBase::Camera(-0.51),
                -0.51,
                (
                    "−0.5 EV camera",
                    "Camera brightness (Shift+B): −0.51 EV from the camera.",
                    "Camera brightness: −0.51 EV from the camera.",
                ),
                (
                    "−0.2 EV camera",
                    "Camera brightness (Shift+B): −0.51 EV from the camera, plus +⅓ EV by hand.",
                    "Camera brightness: −0.51 EV from the camera, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Camera,
                ExposureBase::Camera(0.70),
                0.7,
                (
                    "+0.7 EV camera",
                    "Camera brightness (Shift+B): +0.70 EV from the camera.",
                    "Camera brightness: +0.70 EV from the camera.",
                ),
                (
                    "+1.0 EV camera",
                    "Camera brightness (Shift+B): +0.70 EV from the camera, plus +⅓ EV by hand.",
                    "Camera brightness: +0.70 EV from the camera, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Camera,
                ExposureBase::Camera(0.0),
                0.0,
                (
                    "0 EV camera",
                    "Camera brightness (Shift+B): 0.00 EV from the camera.",
                    "Camera brightness: 0.00 EV from the camera.",
                ),
                (
                    "+⅓ EV camera",
                    "Camera brightness (Shift+B): 0.00 EV from the camera, plus +⅓ EV by hand.",
                    "Camera brightness: 0.00 EV from the camera, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Camera,
                ExposureBase::Camera(-2.51),
                -2.51,
                (
                    "−2 EV camera",
                    "Camera brightness (Shift+B): −2.51 EV from the camera, limited to −2 EV.",
                    "Camera brightness: −2.51 EV from the camera, limited to −2 EV.",
                ),
                (
                    "−2 EV camera",
                    "Camera brightness (Shift+B): −2.51 EV from the camera, limited to −2 EV, plus +⅓ EV by hand.",
                    "Camera brightness: −2.51 EV from the camera, limited to −2 EV, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Camera,
                ExposureBase::Camera(3.10),
                3.1,
                (
                    "+3 EV camera",
                    "Camera brightness (Shift+B): +3.10 EV from the camera, limited to +3 EV.",
                    "Camera brightness: +3.10 EV from the camera, limited to +3 EV.",
                ),
                (
                    "+3 EV camera",
                    "Camera brightness (Shift+B): +3.10 EV from the camera, limited to +3 EV, plus +⅓ EV by hand.",
                    "Camera brightness: +3.10 EV from the camera, limited to +3 EV, plus +⅓ EV by hand.",
                ),
            ),
            (
                ExposureMode::Camera,
                ExposureBase::CameraNone,
                0.0,
                (
                    "Camera EV: none",
                    "Camera brightness (Shift+B): the file gives no value, so 0 EV is used.",
                    "Camera brightness: the file gives no value, so 0 EV is used.",
                ),
                (
                    "+⅓ EV camera",
                    "Camera brightness (Shift+B): the file gives no value, so 0 EV is used, plus +⅓ EV by hand.",
                    "Camera brightness: the file gives no value, so 0 EV is used, plus +⅓ EV by hand.",
                ),
            ),
        ] {
            for (steps, (text, with_key, without_key)) in [(0, no_steps), (1, one_step)] {
                let state = ExposureState {
                    mode,
                    base,
                    steps,
                    total: Ev::from_ev(base_ev + f64::from(steps) / 3.0),
                    view: AdjustState::Shown,
                };
                for (keys, hover) in [(keys, with_key), (ExposureKeys::default(), without_key)] {
                    assert_eq!(
                        exposure_only(&state, keys),
                        vec![AdjustmentMark {
                            text: text.to_owned(),
                            dimmed: false,
                            hover: hover.to_owned(),
                        }],
                        "{state:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_exposure_mark_reports_when_waiting_and_dims_when_paused() {
        let keys = default_exposure_keys();
        for (mode, base, view, text, with_key, without_key) in [
            (
                ExposureMode::Auto,
                ExposureBase::AutoWaiting,
                AdjustState::Waiting,
                "0 EV auto",
                "Auto exposure (B): the histogram is being read.",
                "Auto exposure: the histogram is being read.",
            ),
            (
                ExposureMode::Camera,
                ExposureBase::CameraWaiting,
                AdjustState::Waiting,
                "0 EV camera",
                "Camera brightness (Shift+B): the file is being read.",
                "Camera brightness: the file is being read.",
            ),
            (
                ExposureMode::Off,
                ExposureBase::Manual,
                AdjustState::Waiting,
                "0 EV",
                "Exposure: the view is being made again.",
                "Exposure: the view is being made again.",
            ),
            (
                ExposureMode::Auto,
                ExposureBase::AutoNone,
                AdjustState::Paused,
                "0 EV auto",
                "Exposure is paused: it applies only to RAW renderings.",
                "Exposure is paused: it applies only to RAW renderings.",
            ),
        ] {
            let state = ExposureState {
                mode,
                base,
                view,
                ..EXPOSURE_OFF
            };
            for (keys, hover) in [(keys, with_key), (ExposureKeys::default(), without_key)] {
                assert_eq!(
                    exposure_only(&state, keys),
                    vec![AdjustmentMark {
                        text: text.to_owned(),
                        dimmed: view == AdjustState::Paused,
                        hover: hover.to_owned(),
                    }],
                    "{state:?}"
                );
                assert!(
                    exposure_only(
                        &ExposureState {
                            view: AdjustState::Off,
                            ..state
                        },
                        keys
                    )
                    .is_empty()
                );
            }
        }
    }

    #[test]
    fn the_shadows_mark_follows_exposure_and_reports_each_state() {
        use crate::keymap::{KeyAction, KeyMap};
        let o = KeyMap::default().first_key(KeyAction::Shadows);
        let exposure = ExposureState {
            steps: 1,
            total: Ev::from_thirds(1),
            view: AdjustState::Shown,
            ..EXPOSURE_OFF
        };
        for (state, expected) in [
            (AdjustState::Off, None),
            (
                AdjustState::Shown,
                Some((
                    "Shadows (O): the dark tones are lifted by 75.",
                    "Shadows: the dark tones are lifted by 75.",
                )),
            ),
            (
                AdjustState::Waiting,
                Some((
                    "Shadows (O): the view is being made again.",
                    "Shadows: the view is being made again.",
                )),
            ),
            (
                AdjustState::Paused,
                Some((
                    "Shadows (O) is paused: it applies only to RAW renderings.",
                    "Shadows is paused: it applies only to RAW renderings.",
                )),
            ),
        ] {
            for key in [o, None] {
                let marks = adjustment_marks(
                    &FineState::Shown,
                    None,
                    &SharpenState::Shown,
                    None,
                    Sharpening::DEFAULT,
                    (&exposure, default_exposure_keys()),
                    (&state, key, 75),
                );
                let (first, shadows) = marks.split_at(3);
                assert_eq!(
                    first
                        .iter()
                        .map(|mark| mark.text.as_str())
                        .collect::<Vec<_>>(),
                    ["Fine", "USM", "+⅓ EV"]
                );
                let expected = expected.map(|(with_key, without_key)| AdjustmentMark {
                    text: "Shadows".to_owned(),
                    dimmed: state == AdjustState::Paused,
                    hover: if key.is_some() { with_key } else { without_key }.to_owned(),
                });
                assert_eq!(shadows, expected.as_slice());
            }
        }
    }
}
