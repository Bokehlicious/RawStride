//! The text and metadata filter model (ADR 0022 §2). Plain Rust, no I/O (GR11).

use std::collections::HashMap;
use std::fmt;
use std::slice;

/// The fixed choices for the file type condition.
pub const FILE_TYPES: [&str; 4] = ["RAW", "JPEG", "TIFF", "DNG"];

/// Where a field's value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// The file name, which is always known.
    Name,
    /// File information and Exif metadata.
    Exif,
    /// IPTC metadata.
    Iptc,
}

/// A field available to the extended filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// The file name including its extension.
    FileName,
    /// The camera.
    Camera,
    /// The lens.
    Lens,
    /// The focal length.
    FocalLength,
    /// The aperture.
    Aperture,
    /// The exposure time.
    ExposureTime,
    /// The ISO sensitivity.
    Iso,
    /// The capture date, corrected where available.
    CaptureDate,
    /// The file type.
    FileType,
    /// The IPTC title.
    Title,
    /// The IPTC headline.
    Headline,
    /// The IPTC description.
    Description,
    /// The IPTC keywords.
    Keywords,
    /// The people shown in the image.
    PersonShown,
    /// The IPTC creator.
    Creator,
    /// The IPTC copyright notice.
    CopyrightNotice,
    /// The IPTC city.
    City,
    /// The IPTC country.
    Country,
    /// The IPTC sublocation.
    Location,
}

impl Field {
    /// All fields, in name, Exif and IPTC order.
    pub const ALL: [Field; 19] = [
        Self::FileName,
        Self::Camera,
        Self::Lens,
        Self::FocalLength,
        Self::Aperture,
        Self::ExposureTime,
        Self::Iso,
        Self::CaptureDate,
        Self::FileType,
        Self::Title,
        Self::Headline,
        Self::Description,
        Self::Keywords,
        Self::PersonShown,
        Self::Creator,
        Self::CopyrightNotice,
        Self::City,
        Self::Country,
        Self::Location,
    ];

    /// The group that supplies this field.
    pub fn group(self) -> Group {
        match self {
            Self::FileName => Group::Name,
            Self::Camera
            | Self::Lens
            | Self::FocalLength
            | Self::Aperture
            | Self::ExposureTime
            | Self::Iso
            | Self::CaptureDate
            | Self::FileType => Group::Exif,
            Self::Title
            | Self::Headline
            | Self::Description
            | Self::Keywords
            | Self::PersonShown
            | Self::Creator
            | Self::CopyrightNotice
            | Self::City
            | Self::Country
            | Self::Location => Group::Iptc,
        }
    }

    /// The conditions this field takes, in the order shown in the filter window.
    pub fn kinds(self) -> &'static [ConditionKind] {
        use ConditionKind::*;
        match self {
            Self::FileName
            | Self::Headline
            | Self::PersonShown
            | Self::Creator
            | Self::Location => &[Contains, Is, IsNot],
            Self::Camera | Self::Lens | Self::City | Self::Country => &[Contains, Is, IsNot, OneOf],
            Self::Title | Self::Description | Self::CopyrightNotice => {
                &[Contains, Is, IsNot, IsEmpty]
            }
            Self::Keywords => &[Contains, Is, IsNot, OneOf, IsEmpty],
            Self::FocalLength
            | Self::Aperture
            | Self::ExposureTime
            | Self::Iso
            | Self::CaptureDate => &[AtLeast, AtMost, Between],
            Self::FileType => &[OneOf],
        }
    }

    /// Whether this field offers choices collected from the folder's metadata.
    pub fn has_value_list(self) -> bool {
        matches!(
            self,
            Self::Camera | Self::Lens | Self::Keywords | Self::City | Self::Country
        )
    }

    fn allows(self, condition: &Condition) -> bool {
        self.kinds().contains(&condition.kind())
    }

    fn has_value(self, value: &Value) -> bool {
        match self {
            Self::FocalLength | Self::Aperture | Self::ExposureTime | Self::Iso => {
                matches!(value, Value::Number(_))
            }
            Self::CaptureDate => matches!(value, Value::Date(_)),
            _ => match value {
                Value::Text(text) => !text.trim().is_empty(),
                Value::List(texts) => texts.iter().any(|text| !text.trim().is_empty()),
                Value::Number(_) | Value::Date(_) => false,
            },
        }
    }
}

/// One metadata value from an image.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A single text, compared like a list with one element.
    Text(String),
    /// Multiple texts, compared element by element.
    List(Vec<String>),
    /// A numeric measurement.
    Number(f64),
    /// Whole seconds since 1970-01-01 00:00:00 with no time zone.
    Date(i64),
}

/// The comparison and its operand, if any, for a filter row.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// At least one text contains the operand.
    Contains(String),
    /// At least one text equals the operand.
    Is(String),
    /// No text equals the operand.
    IsNot(String),
    /// At least one text equals one of the operands.
    OneOf(Vec<String>),
    /// A number or date at or above the bound.
    AtLeast(f64),
    /// A number or date at or below the bound.
    AtMost(f64),
    /// A number or date between the two bounds, inclusive.
    Between(f64, f64),
    /// The field has no value.
    HasNo,
}

/// The comparison offered by a filter row, without its operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionKind {
    Contains,
    Is,
    IsNot,
    OneOf,
    AtLeast,
    AtMost,
    Between,
    IsEmpty,
}

impl Condition {
    /// Returns the comparison without its operands.
    pub fn kind(&self) -> ConditionKind {
        match self {
            Self::Contains(_) => ConditionKind::Contains,
            Self::Is(_) => ConditionKind::Is,
            Self::IsNot(_) => ConditionKind::IsNot,
            Self::OneOf(_) => ConditionKind::OneOf,
            Self::AtLeast(_) => ConditionKind::AtLeast,
            Self::AtMost(_) => ConditionKind::AtMost,
            Self::Between(_, _) => ConditionKind::Between,
            Self::HasNo => ConditionKind::IsEmpty,
        }
    }
}

/// Why a filter row could not be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowError {
    /// The field does not support the condition.
    NotAllowed,
    /// The condition requires at least one non-empty text.
    EmptyValue,
    /// A numeric bound is NaN.
    NotANumber,
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAllowed => "condition is not allowed for this field",
            Self::EmptyValue => "condition requires a non-empty text value",
            Self::NotANumber => "condition has a NaN bound",
        })
    }
}

impl std::error::Error for RowError {}

/// A field and a validated condition for that field.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    field: Field,
    condition: Condition,
}

impl Row {
    /// Validates the field and condition, preserving the texts and ordering range bounds.
    pub fn new(field: Field, condition: Condition) -> Result<Self, RowError> {
        if !field.allows(&condition) {
            return Err(RowError::NotAllowed);
        }
        let condition = match condition {
            Condition::Contains(ref text)
            | Condition::Is(ref text)
            | Condition::IsNot(ref text)
                if text.trim().is_empty() =>
            {
                return Err(RowError::EmptyValue);
            }
            Condition::OneOf(ref texts) if texts.iter().all(|text| text.trim().is_empty()) => {
                return Err(RowError::EmptyValue);
            }
            Condition::AtLeast(bound) | Condition::AtMost(bound) if bound.is_nan() => {
                return Err(RowError::NotANumber);
            }
            Condition::Between(a, b) if a.is_nan() || b.is_nan() => {
                return Err(RowError::NotANumber);
            }
            Condition::Between(a, b) if a > b => Condition::Between(b, a),
            _ => condition,
        };
        Ok(Self { field, condition })
    }

    /// The field this row compares.
    pub fn field(&self) -> Field {
        self.field
    }

    /// The validated condition, with the original texts.
    pub fn condition(&self) -> &Condition {
        &self.condition
    }

    fn outcome(&self, image: &ImageValues) -> Outcome {
        if !image.is_read(self.field.group()) {
            return Outcome::Unknown;
        }
        let matches = match &self.condition {
            Condition::Contains(text) => {
                let text = normalized(text);
                image.texts(self.field).any(|value| value.contains(&text))
            }
            Condition::Is(text) | Condition::IsNot(text) => {
                let text = normalized(text);
                let equal = image.texts(self.field).any(|value| value == text);
                match self.condition {
                    Condition::IsNot(_) => !equal,
                    _ => equal,
                }
            }
            Condition::OneOf(texts) => {
                let choices: Vec<_> = texts.iter().map(|text| normalized(text)).collect();
                image
                    .texts(self.field)
                    .any(|value| choices.contains(&value))
            }
            Condition::AtLeast(bound) => image.number(self.field).is_some_and(|n| n >= *bound),
            Condition::AtMost(bound) => image.number(self.field).is_some_and(|n| n <= *bound),
            Condition::Between(a, b) => image
                .number(self.field)
                .is_some_and(|n| (*a..=*b).contains(&n)),
            Condition::HasNo => image.value(self.field).is_none(),
        };
        if matches {
            Outcome::Match
        } else {
            Outcome::NoMatch
        }
    }
}

/// The result of applying a filter or one of its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The image satisfies every applicable condition.
    Match,
    /// At least one condition has failed.
    NoMatch,
    /// A needed group is unread and no condition has failed.
    Unknown,
}

/// The known file name and the metadata groups read for one image.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageValues {
    file_name: String,
    exif: Option<HashMap<Field, Value>>,
    iptc: Option<HashMap<Field, Value>>,
}

impl ImageValues {
    /// Starts with the file name (with extension, without path) and both metadata groups unread.
    pub fn new(file_name: &str) -> Self {
        Self {
            file_name: file_name.to_owned(),
            exif: None,
            iptc: None,
        }
    }

    /// Replaces the Exif group and marks it read, ignoring fields from other groups.
    pub fn set_exif(&mut self, values: Vec<(Field, Value)>) {
        self.exif = Some(Self::collect_group(Group::Exif, values));
    }

    /// Replaces the IPTC group and marks it read, ignoring fields from other groups.
    pub fn set_iptc(&mut self, values: Vec<(Field, Value)>) {
        self.iptc = Some(Self::collect_group(Group::Iptc, values));
    }

    fn collect_group(group: Group, values: Vec<(Field, Value)>) -> HashMap<Field, Value> {
        values
            .into_iter()
            .filter(|(field, _)| field.group() == group)
            .collect()
    }

    /// Whether this group is read. The file name is always known.
    pub fn is_read(&self, group: Group) -> bool {
        match group {
            Group::Name => true,
            Group::Exif => self.exif.is_some(),
            Group::Iptc => self.iptc.is_some(),
        }
    }

    /// Sets or removes a value in a read metadata group. Returns false for the name or an unread group.
    pub fn set_value(&mut self, field: Field, value: Option<Value>) -> bool {
        let values = match field.group() {
            Group::Name => None,
            Group::Exif => self.exif.as_mut(),
            Group::Iptc => self.iptc.as_mut(),
        };
        let Some(values) = values else {
            return false;
        };
        if let Some(value) = value {
            values.insert(field, value);
        } else {
            values.remove(&field);
        }
        true
    }

    /// Returns a present metadata value. The file name is held separately.
    pub fn value(&self, field: Field) -> Option<&Value> {
        let values = match field.group() {
            Group::Name => None,
            Group::Exif => self.exif.as_ref(),
            Group::Iptc => self.iptc.as_ref(),
        };
        values?.get(&field).filter(|value| field.has_value(value))
    }

    fn texts(&self, field: Field) -> impl Iterator<Item = String> + '_ {
        let texts = if field == Field::FileName {
            slice::from_ref(&self.file_name)
        } else {
            match self.value(field) {
                Some(Value::Text(text)) => slice::from_ref(text),
                Some(Value::List(texts)) => texts,
                _ => &[],
            }
        };
        texts
            .iter()
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
            .map(str::to_lowercase)
    }

    fn number(&self, field: Field) -> Option<f64> {
        match self.value(field)? {
            Value::Number(number) => Some(*number),
            Value::Date(seconds) => Some(*seconds as f64),
            _ => None,
        }
    }
}

pub(crate) fn normalized(text: &str) -> String {
    text.trim().to_lowercase()
}

const SEARCH_FIELDS: [Field; 10] = [
    Field::FileName,
    Field::Title,
    Field::Headline,
    Field::Description,
    Field::Keywords,
    Field::PersonShown,
    Field::Location,
    Field::City,
    Field::Country,
    Field::Creator,
];

/// A quick search and extended filter rows, combined with AND.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Filter {
    search: String,
    rows: Vec<Row>,
}

impl Filter {
    /// Replaces the quick search text.
    pub fn set_search(&mut self, text: &str) {
        self.search = text.to_owned();
    }

    /// The quick search text as provided.
    pub fn search(&self) -> &str {
        &self.search
    }

    /// Appends a validated row.
    pub fn push(&mut self, row: Row) {
        self.rows.push(row);
    }

    /// Replaces the extended filter rows, keeping the quick search.
    pub fn set_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
    }

    /// Removes the row at the index, returning false if the index is out of range.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.rows.len() {
            return false;
        }
        self.rows.remove(index);
        true
    }

    /// Clears the search and every row.
    pub fn clear(&mut self) {
        self.search.clear();
        self.rows.clear();
    }

    /// The extended filter rows in their current order.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Whether there is at least one search word or extended filter row.
    pub fn is_active(&self) -> bool {
        self.has_search() || !self.rows.is_empty()
    }

    /// The number of extended filter rows.
    pub fn condition_count(&self) -> usize {
        self.rows.len()
    }

    /// Whether an active search or row needs this metadata group to be read.
    pub fn needs(&self, group: Group) -> bool {
        group != Group::Name
            && ((group == Group::Iptc && self.has_search())
                || self.rows.iter().any(|row| row.field().group() == group))
    }

    /// Combines the search and rows, giving failed conditions precedence over unread values.
    pub fn outcome(&self, image: &ImageValues) -> Outcome {
        let mut combined = self.search_outcome(image);
        if combined == Outcome::NoMatch {
            return combined;
        }
        for row in &self.rows {
            match row.outcome(image) {
                Outcome::NoMatch => return Outcome::NoMatch,
                Outcome::Unknown => combined = Outcome::Unknown,
                Outcome::Match => {}
            }
        }
        combined
    }

    /// Whether the image satisfies the whole filter, with every needed value read.
    pub fn matches(&self, image: &ImageValues) -> bool {
        self.outcome(image) == Outcome::Match
    }

    fn has_search(&self) -> bool {
        self.search.split_whitespace().next().is_some()
    }

    fn search_outcome(&self, image: &ImageValues) -> Outcome {
        if !self.has_search() {
            return Outcome::Match;
        }
        if !image.is_read(Group::Iptc) {
            return Outcome::Unknown;
        }
        let texts: Vec<_> = SEARCH_FIELDS
            .into_iter()
            .flat_map(|field| image.texts(field))
            .collect();
        if self.search.split_whitespace().all(|word| {
            let word = word.to_lowercase();
            texts.iter().any(|text| text.contains(&word))
        }) {
            Outcome::Match
        } else {
            Outcome::NoMatch
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Condition::{AtLeast, AtMost, Between, Contains, HasNo, Is, IsNot, OneOf};
    use Field::*;
    use Outcome::{Match, NoMatch, Unknown};
    use RowError::{EmptyValue, NotANumber, NotAllowed};
    use Value::{Date, List, Number, Text};
    use std::collections::HashSet;

    fn read_image(file_name: &str, values: &[(Field, Value)]) -> ImageValues {
        let mut image = ImageValues::new(file_name);
        image.set_exif(Vec::new());
        image.set_iptc(Vec::new());
        for (field, value) in values {
            assert!(image.set_value(*field, Some(value.clone())));
        }
        image
    }

    fn text(value: &str) -> Value {
        Text(value.into())
    }

    fn list(values: &[&str]) -> Value {
        List(values.iter().map(|value| String::from(*value)).collect())
    }

    fn single_row(field: Field, condition: Condition) -> Filter {
        let mut filter = Filter::default();
        filter.push(Row::new(field, condition).unwrap());
        filter
    }

    fn check_row(image: &ImageValues, field: Field, condition: Condition, expected: Outcome) {
        let filter = single_row(field, condition);
        assert_eq!(filter.outcome(image), expected, "{:?}", filter.rows());
    }

    #[test]
    fn all_fields_are_distinct_and_grouped() {
        assert_eq!(Field::ALL.into_iter().collect::<HashSet<_>>().len(), 19);
        for (group, expected) in [(Group::Name, 1), (Group::Exif, 8), (Group::Iptc, 10)] {
            assert_eq!(
                Field::ALL
                    .iter()
                    .filter(|field| field.group() == group)
                    .count(),
                expected
            );
        }
    }

    #[test]
    fn kinds_follow_allowed_conditions_in_window_order() {
        let examples = [
            (Contains("x".into()), ConditionKind::Contains),
            (Is("x".into()), ConditionKind::Is),
            (IsNot("x".into()), ConditionKind::IsNot),
            (OneOf(vec!["x".into()]), ConditionKind::OneOf),
            (AtLeast(1.0), ConditionKind::AtLeast),
            (AtMost(1.0), ConditionKind::AtMost),
            (Between(1.0, 2.0), ConditionKind::Between),
            (HasNo, ConditionKind::IsEmpty),
        ];
        for (condition, kind) in &examples {
            assert_eq!(condition.kind(), *kind);
        }
        for field in Field::ALL {
            let allowed: Vec<_> = examples
                .iter()
                .filter(|(condition, _)| field.allows(condition))
                .map(|(_, kind)| *kind)
                .collect();
            assert_eq!(field.kinds(), allowed, "{field:?}");
        }
    }

    #[test]
    fn only_five_fields_have_folder_value_lists() {
        for field in Field::ALL {
            assert_eq!(
                field.has_value_list(),
                [Camera, Lens, Keywords, City, Country].contains(&field),
                "{field:?}"
            );
        }
        assert_eq!(FILE_TYPES, ["RAW", "JPEG", "TIFF", "DNG"]);
    }

    #[test]
    fn replacing_rows_keeps_the_search() {
        let mut filter = single_row(FileName, Contains("old".into()));
        filter.set_search("keep me");
        let rows = vec![
            Row::new(Camera, Is("Canon".into())).unwrap(),
            Row::new(Iso, AtLeast(1600.0)).unwrap(),
        ];
        filter.set_rows(rows.clone());
        assert_eq!(filter.rows(), rows);
        assert_eq!(filter.search(), "keep me");
        filter.set_rows(Vec::new());
        assert!(filter.rows().is_empty());
        assert_eq!(filter.search(), "keep me");
    }

    #[test]
    fn every_field_and_condition_pair_follows_the_table() {
        let conditions = [
            Contains("x".into()),
            Is("x".into()),
            IsNot("x".into()),
            OneOf(vec!["x".into()]),
            AtLeast(1.0),
            AtMost(1.0),
            Between(1.0, 2.0),
            HasNo,
        ];
        for field in Field::ALL {
            // The columns follow the order of the example conditions above.
            let allowed = match field {
                FileName | Headline | PersonShown | Creator | Location => {
                    [true, true, true, false, false, false, false, false]
                }
                Title | Description | CopyrightNotice => {
                    [true, true, true, false, false, false, false, true]
                }
                Camera | Lens | City | Country => {
                    [true, true, true, true, false, false, false, false]
                }
                Keywords => [true, true, true, true, false, false, false, true],
                FileType => [false, false, false, true, false, false, false, false],
                FocalLength | Aperture | ExposureTime | Iso | CaptureDate => {
                    [false, false, false, false, true, true, true, false]
                }
            };
            for (condition, allowed) in conditions.iter().zip(allowed) {
                let row = Row::new(field, condition.clone());
                if allowed {
                    let row = row.unwrap();
                    assert_eq!(row.field(), field);
                    assert_eq!(row.condition(), condition);
                } else {
                    assert_eq!(row, Err(NotAllowed), "{field:?}, {condition:?}");
                }
            }
        }
    }

    #[test]
    fn invalid_operands_are_rejected_and_reversed_bounds_are_ordered() {
        for condition in [Contains("  ".into()), Is("".into()), IsNot("\t".into())] {
            assert_eq!(Row::new(Title, condition), Err(EmptyValue));
        }
        assert_eq!(Row::new(Keywords, OneOf(vec![])), Err(EmptyValue));
        assert_eq!(Row::new(Keywords, OneOf(vec![" ".into()])), Err(EmptyValue));
        for condition in [
            AtLeast(f64::NAN),
            AtMost(f64::NAN),
            Between(0.0, f64::NAN),
            Between(f64::NAN, 0.0),
        ] {
            assert_eq!(Row::new(Iso, condition), Err(NotANumber));
        }
        assert_eq!(Row::new(Iso, Contains("".into())), Err(NotAllowed));
        assert_eq!(
            Row::new(Aperture, Between(5.0, 2.0)).unwrap().condition(),
            &Between(2.0, 5.0)
        );
        for condition in [
            Is("  sunset ".into()),
            OneOf(vec![" ".into(), " Beach ".into()]),
        ] {
            assert_eq!(
                Row::new(Keywords, condition.clone()).unwrap().condition(),
                &condition
            );
        }
    }

    #[test]
    fn text_is_trimmed_and_case_blind_without_accent_folding() {
        let image = read_image("image.jpg", &[(Title, text("Sunset"))]);
        check_row(&image, Title, Contains("sun".into()), Match);
        check_row(&image, Title, Is("sun".into()), NoMatch);
        let image = read_image("image.jpg", &[(Title, text("SUNSET"))]);
        check_row(&image, Title, Is("  sunset ".into()), Match);
        let image = read_image("image.jpg", &[(Title, text(" Ä "))]);
        check_row(&image, Title, Is("ä".into()), Match);
        let image = read_image("image.jpg", &[(Title, text("é"))]);
        check_row(&image, Title, Is("e".into()), NoMatch);
    }

    #[test]
    fn list_comparisons_check_each_element() {
        let image = read_image("image.jpg", &[(Keywords, list(&[" ", "Ocean", " Beach "]))]);
        for (condition, expected) in [
            (Contains("eac".into()), Match),
            (Is("beach".into()), Match),
            (Is("ocean beach".into()), NoMatch),
            (IsNot("OCEAN".into()), NoMatch),
            (IsNot("sun".into()), Match),
            (OneOf(vec![" ".into(), "sun".into(), "beach".into()]), Match),
            (OneOf(vec![" ".into(), "sun".into()]), NoMatch),
        ] {
            check_row(&image, Keywords, condition, expected);
        }
    }

    #[test]
    fn missing_blank_and_wrong_type_values_count_as_absent() {
        let mut image = read_image("image.jpg", &[]);
        for value in [
            None,
            Some(text("")),
            Some(list(&[" "])),
            Some(Number(2.0)),
            Some(Date(0)),
        ] {
            assert!(image.set_value(Keywords, value));
            for (condition, expected) in [
                (Contains("x".into()), NoMatch),
                (Is("x".into()), NoMatch),
                (OneOf(vec!["x".into()]), NoMatch),
                (IsNot("x".into()), Match),
                (HasNo, Match),
            ] {
                check_row(&image, Keywords, condition, expected);
            }
        }
        assert!(image.set_value(Iso, Some(text("100"))));
        for condition in [AtLeast(50.0), AtMost(200.0), Between(50.0, 200.0)] {
            check_row(&image, Iso, condition, NoMatch);
        }
        assert!(image.set_value(CaptureDate, Some(Number(60.0))));
        check_row(&image, CaptureDate, AtLeast(60.0), NoMatch);
    }

    #[test]
    fn has_no_matches_only_absent_values_on_its_four_fields() {
        let mut image = read_image("image.jpg", &[]);
        for field in [Title, Description, Keywords, CopyrightNotice] {
            check_row(&image, field, HasNo, Match);
            assert!(image.set_value(field, Some(text("present"))));
            check_row(&image, field, HasNo, NoMatch);
        }
    }

    #[test]
    fn numeric_and_date_conditions_include_the_bounds_and_reject_nan_values() {
        let mut image = read_image("image.jpg", &[]);
        for (field, number) in [
            (Aperture, 2.8),
            (ExposureTime, 0.004),
            (Iso, 100.0),
            (FocalLength, 55.0),
        ] {
            assert!(image.set_value(field, Some(Number(number))));
            for condition in [
                AtLeast(number),
                AtMost(number),
                Between(0.0, number),
                Between(number, number * 2.0),
            ] {
                check_row(&image, field, condition, Match);
            }
            for condition in [AtLeast(number * 2.0), AtMost(0.0), Between(0.0, 0.0)] {
                check_row(&image, field, condition, NoMatch);
            }
        }
        let earlier = 1_763_956_800;
        let later = earlier + 60;
        for seconds in [earlier, later] {
            assert!(image.set_value(CaptureDate, Some(Date(seconds))));
            for condition in [
                AtLeast(seconds as f64),
                AtMost(seconds as f64),
                Between(earlier as f64, later as f64),
            ] {
                check_row(&image, CaptureDate, condition, Match);
            }
        }
        assert!(image.set_value(Iso, Some(Number(f64::NAN))));
        for condition in [AtLeast(0.0), AtMost(100.0), Between(0.0, 100.0)] {
            check_row(&image, Iso, condition, NoMatch);
        }
    }

    #[test]
    fn file_type_matches_any_of_the_selected_choices() {
        let mut image = read_image("image.jpg", &[]);
        let filter = single_row(FileType, OneOf(vec!["raw".into(), "DNG".into()]));
        for (file_type, expected) in [("RAW", Match), ("dng", Match), ("JPEG", NoMatch)] {
            assert!(image.set_value(FileType, Some(text(file_type))));
            assert_eq!(filter.outcome(&image), expected);
        }
    }

    #[test]
    fn unread_groups_are_unknown_and_reading_a_group_replaces_its_values() {
        let mut image = ImageValues::new("Sunset.RAW");
        let camera = single_row(Camera, Is("x".into()));
        assert!(image.is_read(Group::Name));
        assert!(!image.is_read(Group::Exif));
        assert!(!image.is_read(Group::Iptc));
        assert_eq!(camera.outcome(&image), Unknown);
        check_row(&image, Title, HasNo, Unknown);
        check_row(&image, FileName, Is("sunset.raw".into()), Match);
        check_row(&image, FileName, Is("sunset".into()), NoMatch);
        assert_eq!(image.value(FileName), None);

        image.set_exif(vec![(Camera, text("x")), (Title, text("wrong group"))]);
        assert!(image.is_read(Group::Exif));
        assert!(!image.is_read(Group::Iptc));
        assert_eq!(camera.outcome(&image), Match);
        image.set_exif(vec![]);
        assert!(image.is_read(Group::Exif));
        assert_eq!(camera.outcome(&image), NoMatch);

        image.set_iptc(vec![
            (Title, text("Sunset")),
            (FileName, text("wrong group")),
        ]);
        assert!(image.is_read(Group::Iptc));
        check_row(&image, Title, HasNo, NoMatch);
        check_row(&image, FileName, Is("sunset.raw".into()), Match);
        image.set_iptc(vec![]);
        check_row(&image, Title, HasNo, Match);
    }

    #[test]
    fn single_value_updates_apply_only_to_read_metadata_groups() {
        let mut image = ImageValues::new("image.jpg");
        let filter = single_row(Title, Is("sunset".into()));
        assert!(!image.set_value(FileName, Some(text("new.jpg"))));
        assert!(!image.set_value(Title, Some(text("Sunset"))));
        assert!(!image.is_read(Group::Iptc));
        assert_eq!(filter.outcome(&image), Unknown);
        check_row(&image, FileName, Is("image.jpg".into()), Match);

        image.set_iptc(vec![(City, text("Stockholm"))]);
        assert!(image.set_value(Title, Some(text("Sunset"))));
        assert_eq!(image.value(Title), Some(&text("Sunset")));
        assert_eq!(filter.outcome(&image), Match);
        assert!(image.set_value(Title, None));
        assert_eq!(image.value(Title), None);
        assert_eq!(image.value(City), Some(&text("Stockholm")));
        assert_eq!(filter.outcome(&image), NoMatch);
    }

    #[test]
    fn quick_search_requires_each_word_in_the_name_or_a_selected_iptc_field() {
        let unread = ImageValues::new("Sun Beach.jpg");
        let mut image = read_image(
            "nameonly.jpg",
            &[
                (Title, text("Sunset")),
                (Keywords, list(&["Beach"])),
                (Location, text("seaside")),
                (City, text("Stockholm")),
                (Country, text("Sweden")),
                (Creator, text("photographer")),
                (PersonShown, list(&["sample person"])),
                (Headline, text("headline")),
                (Description, text("description")),
                (CopyrightNotice, text("copyright")),
                (Camera, text("camera")),
            ],
        );
        let mut filter = Filter::default();
        for query in ["", "   "] {
            filter.set_search(query);
            assert!(!filter.is_active());
            assert_eq!(filter.outcome(&unread), Match);
        }
        filter.set_search("SUN\u{2003}beach");
        assert_eq!(filter.outcome(&image), Match);
        assert_eq!(filter.outcome(&unread), Unknown);
        assert!(!filter.matches(&unread));
        assert!(image.set_value(Keywords, None));
        assert_eq!(filter.outcome(&image), NoMatch);

        for word in [
            "nameonly",
            "seaside",
            "stockholm",
            "sweden",
            "photographer",
            "person",
            "headline",
            "description",
        ] {
            filter.set_search(word);
            assert!(filter.matches(&image), "{word:?}");
        }
        for word in ["copyright", "camera"] {
            filter.set_search(word);
            assert_eq!(filter.outcome(&image), NoMatch);
        }
    }

    #[test]
    fn combined_outcomes_and_needed_groups_follow_active_conditions() {
        let image = read_image("sun.jpg", &[(Title, text("Sunset"))]);
        let mut filter = single_row(Title, Is("beach".into()));
        filter.set_search("sun");
        assert_eq!(filter.outcome(&image), NoMatch);

        let mut image = ImageValues::new("sun.jpg");
        image.set_iptc(vec![(Title, text("Sunset"))]);
        let camera = Row::new(Camera, Is("x".into())).unwrap();
        let mut filter = single_row(Camera, Is("x".into()));
        filter.push(Row::new(Title, Is("beach".into())).unwrap());
        assert_eq!(filter.outcome(&image), NoMatch);
        let mut filter = single_row(Title, Is("sunset".into()));
        filter.push(camera.clone());
        assert_eq!(filter.outcome(&image), Unknown);
        filter.set_search("sun");
        image.set_exif(vec![(Camera, text("x"))]);
        assert_eq!(filter.outcome(&image), Match);

        let mut filter = Filter::default();
        for group in [Group::Iptc, Group::Exif, Group::Name] {
            assert!(!filter.needs(group));
        }
        filter.set_search("sun");
        assert!(filter.needs(Group::Iptc));
        assert!(!filter.needs(Group::Exif));
        assert!(!filter.needs(Group::Name));
        filter.clear();
        filter.push(Row::new(Title, Is("sunset".into())).unwrap());
        assert!(filter.needs(Group::Iptc));
        assert!(!filter.needs(Group::Exif));
        assert!(!filter.needs(Group::Name));
        filter.clear();
        filter.push(camera);
        assert!(!filter.needs(Group::Iptc));
        assert!(filter.needs(Group::Exif));
        assert!(!filter.needs(Group::Name));
        filter.clear();
        filter.push(Row::new(FileName, Is("sun.jpg".into())).unwrap());
        for group in [Group::Iptc, Group::Exif, Group::Name] {
            assert!(!filter.needs(group));
        }
    }

    #[test]
    fn bookkeeping_counts_only_rows_and_clears_both_parts() {
        let mut filter = Filter::default();
        assert!(!filter.is_active());
        assert_eq!(filter.condition_count(), 0);
        let row = Row::new(Title, Is("sunset".into())).unwrap();
        filter.push(row.clone());
        assert!(filter.is_active());
        assert_eq!(filter.condition_count(), 1);
        assert_eq!(filter.rows(), std::slice::from_ref(&row));
        assert!(filter.remove(0));
        assert!(!filter.remove(0));
        assert!(!filter.remove(usize::MAX));
        assert!(!filter.is_active());
        filter.set_search(" sunset ");
        assert_eq!(filter.search(), " sunset ");
        assert!(filter.is_active());
        assert_eq!(filter.condition_count(), 0);
        filter.push(row.clone());
        filter.push(row);
        assert_eq!(filter.condition_count(), 2);
        filter.clear();
        assert!(!filter.is_active());
        assert!(filter.rows().is_empty());
        assert_eq!(filter.search(), "");
    }

    #[test]
    fn row_order_does_not_change_any_outcome() {
        let mut unread_iptc = ImageValues::new("image.jpg");
        unread_iptc.set_exif(vec![(Iso, Number(100.0))]);
        let images = [
            (
                read_image(
                    "image.jpg",
                    &[(Title, text("Sunset")), (Iso, Number(100.0))],
                ),
                Match,
            ),
            (read_image("image.jpg", &[(Iso, Number(800.0))]), NoMatch),
            (unread_iptc, Unknown),
        ];
        let title = Row::new(Title, Contains("sun".into())).unwrap();
        let iso = Row::new(Iso, AtLeast(50.0)).unwrap();
        let mut title_first = Filter::default();
        title_first.push(title.clone());
        title_first.push(iso.clone());
        let mut iso_first = Filter::default();
        iso_first.push(iso);
        iso_first.push(title);
        for (image, expected) in images {
            assert_eq!(title_first.outcome(&image), expected);
            assert_eq!(iso_first.outcome(&image), expected);
        }
    }
}
