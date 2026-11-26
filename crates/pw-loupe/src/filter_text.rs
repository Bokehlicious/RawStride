//! English texts for the filter window (ADR 0022 S4).

use crate::filter_window::Missing;
use crate::meta_filter::{ConditionKind, Field, Group};
use crate::{Language, property_label};

/// The name of a field in the filter window.
pub fn field_label(field: Field) -> String {
    match field {
        Field::FileName => "File Name",
        Field::Camera => "Camera",
        Field::Lens => "Lens",
        Field::FocalLength => "Focal Length",
        Field::Aperture => "Aperture",
        Field::ExposureTime => "Exposure Time",
        Field::Iso => "ISO",
        Field::CaptureDate => "Capture Date",
        Field::FileType => "File Type",
        Field::Title => return property_label("dc:title", Language::En),
        Field::Headline => return property_label("photoshop:Headline", Language::En),
        Field::Description => return property_label("dc:description", Language::En),
        Field::Keywords => return property_label("dc:subject", Language::En),
        Field::PersonShown => return property_label("Iptc4xmpExt:PersonInImage", Language::En),
        Field::Creator => return property_label("dc:creator", Language::En),
        Field::CopyrightNotice => return property_label("dc:rights", Language::En),
        Field::City => return property_label("photoshop:City", Language::En),
        Field::Country => return property_label("photoshop:Country", Language::En),
        Field::Location => return property_label("Iptc4xmpCore:Location", Language::En),
    }
    .to_owned()
}

/// The heading for a group of fields.
pub fn group_label(group: Group) -> String {
    match group {
        Group::Name => "File",
        Group::Exif => "Exif",
        Group::Iptc => "IPTC",
    }
    .to_owned()
}

/// The label of a condition kind.
pub fn condition_text(kind: ConditionKind) -> String {
    match kind {
        ConditionKind::Contains => "contains",
        ConditionKind::Is => "is",
        ConditionKind::IsNot => "is not",
        ConditionKind::OneOf => "is one of",
        ConditionKind::AtLeast => "≥",
        ConditionKind::AtMost => "≤",
        ConditionKind::Between => "between",
        ConditionKind::IsEmpty => "is empty",
    }
    .to_owned()
}

/// The full template for the range operands, with placeholders for the inputs.
pub fn between_template() -> String {
    "{low} and {high}".to_owned()
}

/// A text or input in the range template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BetweenPart {
    Text(String),
    Low,
    High,
}

/// The text and input fields in the template's display order.
pub fn between_parts() -> Vec<BetweenPart> {
    let template = between_template();
    let mut parts = Vec::new();
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        let (text, candidate) = rest.split_at(start);
        let (part, after) = if let Some(after) = candidate.strip_prefix("{low}") {
            (BetweenPart::Low, after)
        } else if let Some(after) = candidate.strip_prefix("{high}") {
            (BetweenPart::High, after)
        } else {
            break;
        };
        if !text.is_empty() {
            parts.push(BetweenPart::Text(text.to_owned()));
        }
        parts.push(part);
        rest = after;
    }
    if !rest.is_empty() {
        parts.push(BetweenPart::Text(rest.to_owned()));
    }
    parts
}

/// An example for an empty number or date field.
pub fn value_hint(field: Field) -> Option<String> {
    Some(
        match field {
            Field::ExposureTime => "1/250",
            Field::Aperture => "2.8",
            Field::FocalLength => "35",
            Field::Iso => "1600",
            Field::CaptureDate => "YYYY-MM-DD",
            _ => return None,
        }
        .to_owned(),
    )
}

/// The instruction for completing an invalid or unfinished row.
pub fn missing_text(missing: Missing) -> String {
    match missing {
        Missing::Value => "Enter a value",
        Missing::Pick => "Pick at least one value",
        Missing::Number => "Enter a number",
        Missing::Date => "Enter a date as YYYY-MM-DD",
    }
    .to_owned()
}

/// The command bar's filter button label for the number of applied conditions.
pub fn filter_button_text(conditions: usize) -> String {
    match conditions {
        0 => "Filter…".to_owned(),
        1 => "Filter: 1 condition".to_owned(),
        n => format!("Filter: {n} conditions"),
    }
}

/// A choice with the number of images that have that value.
pub fn value_count_text(value: &str, count: usize) -> String {
    format!("{value} ({count})")
}

/// A fixed text in the filter window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterText {
    Title,
    AddCondition,
    Remove,
    Clear,
    Ok,
    Cancel,
    NoConditions,
    NoValues,
}

/// The English text for a fixed filter window label.
pub fn filter_text(text: FilterText) -> String {
    match text {
        FilterText::Title => "Filter",
        FilterText::AddCondition => "Add Condition",
        FilterText::Remove => "Remove",
        FilterText::Clear => "Clear",
        FilterText::Ok => "OK",
        FilterText::Cancel => "Cancel",
        FilterText::NoConditions => "No conditions: every image is shown.",
        FilterText::NoValues => "No values in the folder",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_exif_and_group_labels_match_the_glossary() {
        for (field, label) in [
            (Field::FileName, "File Name"),
            (Field::Camera, "Camera"),
            (Field::Lens, "Lens"),
            (Field::FocalLength, "Focal Length"),
            (Field::Aperture, "Aperture"),
            (Field::ExposureTime, "Exposure Time"),
            (Field::Iso, "ISO"),
            (Field::CaptureDate, "Capture Date"),
            (Field::FileType, "File Type"),
        ] {
            assert_eq!(field_label(field), label);
        }
        for (group, label) in [
            (Group::Name, "File"),
            (Group::Exif, "Exif"),
            (Group::Iptc, "IPTC"),
        ] {
            assert_eq!(group_label(group), label);
        }
    }

    #[test]
    fn iptc_labels_come_from_the_corresponding_properties() {
        for (field, id) in [
            (Field::Title, "dc:title"),
            (Field::Headline, "photoshop:Headline"),
            (Field::Description, "dc:description"),
            (Field::Keywords, "dc:subject"),
            (Field::PersonShown, "Iptc4xmpExt:PersonInImage"),
            (Field::Creator, "dc:creator"),
            (Field::CopyrightNotice, "dc:rights"),
            (Field::City, "photoshop:City"),
            (Field::Country, "photoshop:Country"),
            (Field::Location, "Iptc4xmpCore:Location"),
        ] {
            assert_eq!(field_label(field), property_label(id, Language::En));
        }
    }

    #[test]
    fn comparison_texts_and_range_template_follow_window_order() {
        use ConditionKind::*;
        for (kind, label) in [
            (Contains, "contains"),
            (Is, "is"),
            (IsNot, "is not"),
            (OneOf, "is one of"),
            (AtLeast, "≥"),
            (AtMost, "≤"),
            (Between, "between"),
            (IsEmpty, "is empty"),
        ] {
            assert_eq!(condition_text(kind), label);
        }
        assert_eq!(between_template(), "{low} and {high}");
        assert_eq!(
            between_parts(),
            [
                BetweenPart::Low,
                BetweenPart::Text(" and ".into()),
                BetweenPart::High
            ]
        );
    }

    #[test]
    fn each_field_has_the_expected_hint_or_no_hint() {
        for field in Field::ALL {
            let expected = match field {
                Field::ExposureTime => Some("1/250"),
                Field::Aperture => Some("2.8"),
                Field::FocalLength => Some("35"),
                Field::Iso => Some("1600"),
                Field::CaptureDate => Some("YYYY-MM-DD"),
                _ => None,
            };
            assert_eq!(value_hint(field).as_deref(), expected);
        }
    }

    #[test]
    fn missing_operands_have_distinct_instructions() {
        for (missing, label) in [
            (Missing::Value, "Enter a value"),
            (Missing::Pick, "Pick at least one value"),
            (Missing::Number, "Enter a number"),
            (Missing::Date, "Enter a date as YYYY-MM-DD"),
        ] {
            assert_eq!(missing_text(missing), label);
        }
    }

    #[test]
    fn button_and_value_counts_use_complete_labels() {
        assert_eq!(filter_button_text(0), "Filter…");
        assert_eq!(filter_button_text(1), "Filter: 1 condition");
        assert_eq!(filter_button_text(2), "Filter: 2 conditions");
        assert_eq!(value_count_text("Canon", 3), "Canon (3)");
    }

    #[test]
    fn fixed_window_texts_match_the_glossary() {
        use FilterText::*;
        for (text, label) in [
            (Title, "Filter"),
            (AddCondition, "Add Condition"),
            (Remove, "Remove"),
            (Clear, "Clear"),
            (Ok, "OK"),
            (Cancel, "Cancel"),
            (NoConditions, "No conditions: every image is shown."),
            (NoValues, "No values in the folder"),
        ] {
            assert_eq!(filter_text(text), label);
        }
    }
}
