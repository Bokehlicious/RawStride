//! The texts of the inspector (ADR 0017 §2.1, U2, T110; rule 13): its section headings, its
//! states and the Exif summary's labels and value templates. Each text is made here, whole; the
//! drawing code in `main.rs` holds none of them. The Exif labels are not IPTC properties, so they
//! do not come from `property_label` (U2-Q4).

/// A text of the inspector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InspectorText {
    /// The heading of the Exif summary section.
    Info,
    /// The heading of the metadata section.
    Metadata,
    /// No current image (an empty tab, or no tab).
    NoImage,
    /// The file has none of the summary's values.
    NoExif,
    /// The Exif read failed; holds the reason.
    ExifError(String),
    /// The Exif read is not back yet.
    ExifLoading,
    /// The Exif reader's thread has stopped (the reason of `ExifError`).
    ReaderStopped,
}

/// The text of `text`.
pub(crate) fn inspector_text(text: &InspectorText) -> String {
    match text {
        InspectorText::Info => "Info".to_owned(),
        InspectorText::Metadata => "Metadata".to_owned(),
        InspectorText::NoImage => "Ingen bild vald.".to_owned(),
        InspectorText::NoExif => "Filen har ingen kamerainformation.".to_owned(),
        InspectorText::ExifError(reason) => {
            format!("Kamerainformationen kunde inte läsas: {reason}")
        }
        InspectorText::ExifLoading => "Läser kamerainformation…".to_owned(),
        InspectorText::ReaderStopped => "Exif-läsaren har stannat".to_owned(),
    }
}

/// A row of the Exif summary, in the order they are shown (U2-Q2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExifLabel {
    Camera,
    Lens,
    FocalLength,
    Aperture,
    ExposureTime,
    Iso,
    Taken,
}

/// The label of an Exif row.
pub(crate) fn exif_label(label: ExifLabel) -> &'static str {
    match label {
        ExifLabel::Camera => "Kamera",
        ExifLabel::Lens => "Objektiv",
        ExifLabel::FocalLength => "Brännvidd",
        ExifLabel::Aperture => "Bländare",
        ExifLabel::ExposureTime => "Exponeringstid",
        ExifLabel::Iso => "ISO",
        ExifLabel::Taken => "Tagen",
    }
}

/// An Exif value with its unit or form; the numbers are already written with a decimal comma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExifValue {
    /// The make and the model, when the model does not begin with the make.
    Camera {
        make: String,
        model: String,
    },
    /// The focal length in millimetres ("24,5").
    FocalLength(String),
    /// The f-number ("2,8").
    Aperture(String),
    Iso(u32),
    /// DateTimeOriginal's parts ("2026", "10", "30", "14:03:12").
    Taken {
        year: String,
        month: String,
        day: String,
        time: String,
    },
}

/// The text of an Exif value.
pub(crate) fn exif_value_text(value: &ExifValue) -> String {
    match value {
        ExifValue::Camera { make, model } => format!("{make} {model}"),
        ExifValue::FocalLength(mm) => format!("{mm} mm"),
        ExifValue::Aperture(number) => format!("f/{number}"),
        ExifValue::Iso(iso) => format!("ISO {iso}"),
        ExifValue::Taken {
            year,
            month,
            day,
            time,
        } => format!("{year}-{month}-{day} {time}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_text_of_the_inspector() {
        let texts = [
            (InspectorText::Info, "Info"),
            (InspectorText::Metadata, "Metadata"),
            (InspectorText::NoImage, "Ingen bild vald."),
            (InspectorText::NoExif, "Filen har ingen kamerainformation."),
            (
                InspectorText::ExifError("format error: not a TIFF file".to_owned()),
                "Kamerainformationen kunde inte läsas: format error: not a TIFF file",
            ),
            (InspectorText::ExifLoading, "Läser kamerainformation…"),
            (InspectorText::ReaderStopped, "Exif-läsaren har stannat"),
        ];
        for (text, expected) in texts {
            assert_eq!(inspector_text(&text), expected, "{text:?}");
        }
    }

    #[test]
    fn every_exif_label() {
        let labels = [
            (ExifLabel::Camera, "Kamera"),
            (ExifLabel::Lens, "Objektiv"),
            (ExifLabel::FocalLength, "Brännvidd"),
            (ExifLabel::Aperture, "Bländare"),
            (ExifLabel::ExposureTime, "Exponeringstid"),
            (ExifLabel::Iso, "ISO"),
            (ExifLabel::Taken, "Tagen"),
        ];
        for (label, expected) in labels {
            assert_eq!(exif_label(label), expected, "{label:?}");
        }
    }

    #[test]
    fn every_exif_value_template() {
        let values = [
            (
                ExifValue::Camera {
                    make: "Panasonic".to_owned(),
                    model: "DC-S5M2".to_owned(),
                },
                "Panasonic DC-S5M2",
            ),
            (ExifValue::FocalLength("24,5".to_owned()), "24,5 mm"),
            (ExifValue::Aperture("2,8".to_owned()), "f/2,8"),
            (ExifValue::Iso(400), "ISO 400"),
            (
                ExifValue::Taken {
                    year: "2026".to_owned(),
                    month: "10".to_owned(),
                    day: "30".to_owned(),
                    time: "14:03:12".to_owned(),
                },
                "2026-10-30 14:03:12",
            ),
        ];
        for (value, expected) in values {
            assert_eq!(exif_value_text(&value), expected, "{value:?}");
        }
    }
}
