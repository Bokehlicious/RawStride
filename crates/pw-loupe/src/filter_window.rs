//! The editable filter rows and value lists (ADR 0022 S4), without UI or I/O.

use std::collections::HashSet;

use crate::filter_values::{SECONDS_PER_DAY, civil_date, day_number};
use crate::meta_filter::{
    Condition, ConditionKind, Field, Filter, Group, Row, RowError, normalized,
};

fn plain_number(text: &str) -> Option<f64> {
    let mut parts = text.split(['.', ',']);
    let whole = parts.next()?;
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number = text.replace(',', ".").parse::<f64>().ok()?;
    (number.is_finite() && number > 0.0).then_some(number)
}

/// Parses a positive measurement in the form accepted for the field.
pub fn parse_number(field: Field, text: &str) -> Option<f64> {
    let text = text.trim();
    match field {
        Field::Aperture => plain_number(
            text.strip_prefix("f/")
                .or_else(|| text.strip_prefix("F/"))
                .unwrap_or(text)
                .trim_start(),
        ),
        Field::ExposureTime => match text.split_once('/') {
            Some((numerator, denominator)) => {
                let number = plain_number(numerator.trim())? / plain_number(denominator.trim())?;
                (number.is_finite() && number > 0.0).then_some(number)
            }
            None => plain_number(text),
        },
        Field::FocalLength => plain_number(text),
        Field::Iso if text.bytes().all(|b| b.is_ascii_digit()) => plain_number(text),
        _ => None,
    }
}

/// Parses a calendar date as the seconds at the start of that day.
pub fn parse_date(text: &str) -> Option<i64> {
    let (year, rest) = text.trim().split_once('-')?;
    let (month, day) = rest.split_once('-')?;
    let digits = |part: &str, width| {
        if part.len() != width || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse::<i64>().ok()
    };
    Some(day_number(digits(year, 4)?, digits(month, 2)?, digits(day, 2)?)? * SECONDS_PER_DAY)
}

/// What a draft row still needs before it can be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Value,
    Pick,
    Number,
    Date,
}

impl From<RowError> for Missing {
    fn from(error: RowError) -> Self {
        match error {
            RowError::EmptyValue | RowError::NotAllowed => Self::Value,
            RowError::NotANumber => Self::Number,
        }
    }
}

/// One editable field, condition and set of operands.
#[derive(Debug, Clone)]
pub struct DraftRow {
    field: Field,
    kind: ConditionKind,
    text: String,
    text2: String,
    picks: Vec<String>,
}

impl DraftRow {
    /// Starts an empty row with the field's first condition kind.
    pub fn new(field: Field) -> Self {
        Self {
            field,
            kind: field
                .kinds()
                .first()
                .copied()
                .unwrap_or(ConditionKind::Contains),
            text: String::new(),
            text2: String::new(),
            picks: Vec::new(),
        }
    }

    /// The selected field.
    pub fn field(&self) -> Field {
        self.field
    }

    /// The selected comparison.
    pub fn kind(&self) -> ConditionKind {
        self.kind
    }

    /// The typed operand, or the first bound of a range.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The second bound of a range.
    pub fn text2(&self) -> &str {
        &self.text2
    }

    /// The selected list values, in the order picked.
    pub fn picks(&self) -> &[String] {
        &self.picks
    }

    /// Changes the field, clearing operands and retaining the kind if it still applies.
    pub fn set_field(&mut self, field: Field) {
        if self.field != field {
            let kind = self.kind;
            *self = Self::new(field);
            self.set_kind(kind);
        }
    }

    /// Selects a supported kind, keeping the operands for further edits.
    pub fn set_kind(&mut self, kind: ConditionKind) {
        if self.field.kinds().contains(&kind) {
            self.kind = kind;
        }
    }

    /// Edits the first text field.
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    /// Edits the second text field.
    pub fn text2_mut(&mut self) -> &mut String {
        &mut self.text2
    }

    /// Adds a choice or removes matching choices using the filter's comparison form.
    pub fn toggle_pick(&mut self, value: &str) {
        let key = normalized(value);
        let before = self.picks.len();
        self.picks.retain(|pick| normalized(pick) != key);
        if self.picks.len() == before {
            self.picks.push(value.to_owned());
        }
    }

    /// Validates the operands and builds a filter row.
    pub fn to_row(&self) -> Result<Row, Missing> {
        let condition = match self.kind {
            ConditionKind::Contains => Condition::Contains(self.text.clone()),
            ConditionKind::Is => Condition::Is(self.text.clone()),
            ConditionKind::IsNot => Condition::IsNot(self.text.clone()),
            ConditionKind::OneOf if self.picks.is_empty() => return Err(Missing::Pick),
            ConditionKind::OneOf => Condition::OneOf(self.picks.clone()),
            ConditionKind::IsEmpty => Condition::HasNo,
            kind if self.field == Field::CaptureDate => self.date_condition(kind)?,
            kind => {
                let first = self.number_bound(&self.text)?;
                match kind {
                    ConditionKind::AtLeast => Condition::AtLeast(first),
                    ConditionKind::AtMost => Condition::AtMost(first),
                    ConditionKind::Between => {
                        Condition::Between(first, self.number_bound(&self.text2)?)
                    }
                    _ => return Err(Missing::Value),
                }
            }
        };
        Row::new(self.field, condition).map_err(Missing::from)
    }

    fn number_bound(&self, text: &str) -> Result<f64, Missing> {
        parse_number(self.field, text).ok_or(Missing::Number)
    }

    fn date_condition(&self, kind: ConditionKind) -> Result<Condition, Missing> {
        let first = parse_date(&self.text).ok_or(Missing::Date)?;
        let end = |day| (day + SECONDS_PER_DAY - 1) as f64;
        match kind {
            ConditionKind::AtLeast => Ok(Condition::AtLeast(first as f64)),
            ConditionKind::AtMost => Ok(Condition::AtMost(end(first))),
            ConditionKind::Between => {
                let second = parse_date(&self.text2).ok_or(Missing::Date)?;
                Ok(Condition::Between(
                    first.min(second) as f64,
                    end(first.max(second)),
                ))
            }
            _ => Err(Missing::Value),
        }
    }

    /// Opens an applied row for editing, formatting its bounds for the input fields.
    pub fn from_row(row: &Row) -> Self {
        let field = row.field();
        let mut draft = Self::new(field);
        draft.kind = row.condition().kind();
        match row.condition() {
            Condition::Contains(text) | Condition::Is(text) | Condition::IsNot(text) => {
                draft.text.clone_from(text)
            }
            Condition::OneOf(picks) => draft.picks.clone_from(picks),
            Condition::AtLeast(bound) | Condition::AtMost(bound) => {
                draft.text = bound_text(field, *bound)
            }
            Condition::Between(first, second) => {
                draft.text = bound_text(field, *first);
                draft.text2 = bound_text(field, *second);
            }
            Condition::HasNo => {}
        }
        draft
    }
}

fn bound_text(field: Field, bound: f64) -> String {
    if field == Field::CaptureDate {
        let day = (bound.floor() as i64).div_euclid(SECONDS_PER_DAY);
        let (year, month, date) = civil_date(day);
        return format!("{year:04}-{month:02}-{date:02}");
    }
    if field == Field::ExposureTime && (0.0..1.0).contains(&bound) {
        let inverse = bound.recip();
        let rounded = inverse.round();
        if (inverse - rounded).abs() <= 1e-6 {
            return format!("1/{rounded}");
        }
    }
    format!("{bound}")
}

/// The ordered draft rows in the filter window.
#[derive(Debug, Clone, Default)]
pub struct FilterDraft {
    rows: Vec<DraftRow>,
}

impl FilterDraft {
    /// Copies the applied conditions, leaving the quick search outside the draft.
    pub fn from_filter(filter: &Filter) -> Self {
        Self {
            rows: filter.rows().iter().map(DraftRow::from_row).collect(),
        }
    }

    /// The draft rows in display order.
    pub fn rows(&self) -> &[DraftRow] {
        &self.rows
    }

    /// The draft rows for editing.
    pub fn rows_mut(&mut self) -> &mut [DraftRow] {
        &mut self.rows
    }

    /// Appends an empty File Name row.
    pub fn add(&mut self) {
        self.rows.push(DraftRow::new(Field::FileName));
    }

    /// Removes a row if its index is present.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.rows.len() {
            return false;
        }
        self.rows.remove(index);
        true
    }

    /// Removes all draft rows.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Builds all rows or reports the index and error of the first incomplete row.
    pub fn result(&self) -> Result<Vec<Row>, (usize, Missing)> {
        self.rows
            .iter()
            .enumerate()
            .map(|(index, row)| row.to_row().map_err(|error| (index, error)))
            .collect()
    }

    /// The metadata groups needed by open value lists, in field-list order.
    pub fn list_groups(&self) -> Vec<Group> {
        [Group::Exif, Group::Iptc]
            .into_iter()
            .filter(|&group| {
                self.rows.iter().any(|row| {
                    row.kind == ConditionKind::OneOf
                        && row.field.has_value_list()
                        && row.field.group() == group
                })
            })
            .collect()
    }
}

/// Marks counted choices as picked and appends picks that have no count.
pub fn list_entries(counts: &[(String, usize)], picked: &[String]) -> Vec<(String, usize, bool)> {
    let picked_keys: HashSet<_> = picked.iter().map(|pick| normalized(pick)).collect();
    let mut counted_keys = HashSet::new();
    let mut entries = Vec::new();
    for (value, count) in counts {
        let key = normalized(value);
        entries.push((value.clone(), *count, picked_keys.contains(&key)));
        counted_keys.insert(key);
    }
    entries.extend(
        picked
            .iter()
            .filter(|pick| !counted_keys.contains(&normalized(pick)))
            .map(|pick| (pick.clone(), 0, true)),
    );
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use ConditionKind::*;
    use Field::*;

    #[test]
    fn parses_positive_numbers_in_each_fields_forms() {
        for (field, text, expected) in [
            (ExposureTime, "1/250", 0.004),
            (ExposureTime, " 2,8 / 4 ", 0.7),
            (ExposureTime, "0.5", 0.5),
            (ExposureTime, "2", 2.0),
            (Aperture, "2,8", 2.8),
            (Aperture, "f/2.8", 2.8),
            (Aperture, "F/ 4", 4.0),
            (FocalLength, "35", 35.0),
            (Iso, " 1600 ", 1600.0),
        ] {
            assert_eq!(
                parse_number(field, text),
                Some(expected),
                "{field:?} {text}"
            );
        }
        for (field, text) in [
            (ExposureTime, ""),
            (ExposureTime, "abc"),
            (ExposureTime, "0"),
            (ExposureTime, "-1"),
            (ExposureTime, "1/0"),
            (ExposureTime, "1/2/3"),
            (ExposureTime, " 2 ,8 / 4 "),
            (FocalLength, "+2"),
            (FocalLength, "2e2"),
            (FocalLength, "inf"),
            (FocalLength, "NaN"),
            (FocalLength, "2,.8"),
            (FocalLength, "３５"),
            (FocalLength, "f/2.8"),
            (Iso, "2.5"),
            (Iso, "1600.0"),
            (Title, "2"),
        ] {
            assert_eq!(parse_number(field, text), None, "{field:?} {text}");
        }
    }

    #[test]
    fn dates_require_exact_width_and_a_real_calendar_day() {
        assert_eq!(parse_date(" 2024-02-29 "), Some(1_709_164_800));
        assert_eq!(parse_date("1969-12-31"), Some(-86_400));
        for text in [
            "2023-02-29",
            "2024-13-01",
            "24-01-01",
            "2024-1-1",
            "+024-01-01",
            "2024-01-01 00:00:00",
        ] {
            assert_eq!(parse_date(text), None, "{text}");
        }
    }

    #[test]
    fn rows_round_trip_every_kind_and_keep_the_original_texts() {
        for (field, condition, first, second) in [
            (FileName, Condition::Contains(" raw ".into()), " raw ", ""),
            (Camera, Condition::Is(" Canon ".into()), " Canon ", ""),
            (City, Condition::IsNot(" Rome ".into()), " Rome ", ""),
            (
                Keywords,
                Condition::OneOf(vec!["Bird".into(), " Nature ".into()]),
                "",
                "",
            ),
            (ExposureTime, Condition::AtLeast(0.004), "1/250", ""),
            (ExposureTime, Condition::AtMost(0.3), "0.3", ""),
            (FocalLength, Condition::Between(35.0, 50.0), "35", "50"),
            (Title, Condition::HasNo, "", ""),
            (CaptureDate, Condition::AtLeast(-86_400.0), "1969-12-31", ""),
            (CaptureDate, Condition::AtMost(-1.0), "1969-12-31", ""),
            (
                CaptureDate,
                Condition::Between(-86_400.0, 86_399.0),
                "1969-12-31",
                "1970-01-01",
            ),
        ] {
            let row = Row::new(field, condition).unwrap();
            let draft = DraftRow::from_row(&row);
            assert_eq!(draft.field(), field);
            assert_eq!(draft.kind(), row.condition().kind());
            assert_eq!((draft.text(), draft.text2()), (first, second));
            assert_eq!(draft.to_row(), Ok(row));
        }
    }

    #[test]
    fn incomplete_rows_report_each_missing_operand() {
        let mut row = DraftRow::new(Title);
        *row.text_mut() = "  ".into();
        for kind in [Contains, Is, IsNot] {
            row.set_kind(kind);
            assert_eq!(row.to_row(), Err(Missing::Value));
        }
        row.set_kind(IsEmpty);
        assert_eq!(row.to_row().unwrap().condition(), &Condition::HasNo);

        row.set_field(FileType);
        assert_eq!(row.to_row(), Err(Missing::Pick));
        row.toggle_pick("JPEG");
        assert_eq!(
            row.to_row().unwrap().condition(),
            &Condition::OneOf(vec!["JPEG".into()])
        );
        row.toggle_pick(" jpeg ");
        assert_eq!(row.to_row(), Err(Missing::Pick));

        row.set_field(Iso);
        assert_eq!(row.to_row(), Err(Missing::Number));
        row.set_kind(Between);
        *row.text_mut() = "1600".into();
        assert_eq!(row.to_row(), Err(Missing::Number));
        *row.text2_mut() = "200".into();
        assert_eq!(
            row.to_row().unwrap().condition(),
            &Condition::Between(200.0, 1600.0)
        );

        row.set_field(CaptureDate);
        assert_eq!(row.to_row(), Err(Missing::Date));
        *row.text_mut() = "2024-02-29".into();
        assert_eq!(row.to_row(), Err(Missing::Date));
    }

    #[test]
    fn date_bounds_include_the_last_day_regardless_of_input_order() {
        let mut row = DraftRow::new(CaptureDate);
        let early = "2024-02-28";
        let late = "2024-02-29";
        let start = parse_date(early).unwrap() as f64;
        let end = (parse_date(late).unwrap() + SECONDS_PER_DAY - 1) as f64;
        *row.text_mut() = early.into();
        assert_eq!(
            row.to_row().unwrap().condition(),
            &Condition::AtLeast(start)
        );

        row.set_kind(AtMost);
        *row.text_mut() = late.into();
        assert_eq!(row.to_row().unwrap().condition(), &Condition::AtMost(end));

        row.set_kind(Between);
        for (first, second) in [(early, late), (late, early)] {
            *row.text_mut() = first.into();
            *row.text2_mut() = second.into();
            assert_eq!(
                row.to_row().unwrap().condition(),
                &Condition::Between(start, end)
            );
        }
    }

    #[test]
    fn changing_field_clears_operands_and_keeps_only_a_supported_kind() {
        let mut row = DraftRow::new(Camera);
        row.set_kind(OneOf);
        row.toggle_pick("Canon");
        *row.text_mut() = "Canon".into();
        *row.text2_mut() = "second".into();
        row.set_kind(Between);
        row.set_field(Camera);
        assert_eq!(row.kind(), OneOf);
        assert_eq!(
            (row.text(), row.text2(), row.picks()),
            ("Canon", "second", &["Canon".to_owned()][..])
        );
        row.set_kind(Contains);
        row.set_kind(OneOf);
        assert_eq!(row.picks(), ["Canon"]);

        row.set_field(Lens);
        assert_eq!(row.kind(), OneOf);
        assert_eq!((row.text(), row.text2(), row.picks()), ("", "", &[][..]));
        row.toggle_pick("Zoom");
        *row.text_mut() = "Zoom".into();
        *row.text2_mut() = "second".into();
        row.set_field(Iso);
        assert_eq!(row.kind(), AtLeast);
        assert_eq!((row.text(), row.text2(), row.picks()), ("", "", &[][..]));
    }

    #[test]
    fn draft_reports_the_first_incomplete_row_and_supports_row_edits() {
        let first = Row::new(FileName, Condition::Contains("jpg".into())).unwrap();
        let mut filter = Filter::default();
        filter.set_search("keep me");
        filter.push(first.clone());
        let mut draft = FilterDraft::from_filter(&filter);
        assert_eq!(draft.result(), Ok(vec![first.clone()]));
        draft.add();
        draft.add();
        assert_eq!(draft.rows()[1].field(), FileName);
        assert_eq!(draft.rows()[1].kind(), Contains);
        draft.rows_mut()[2].set_field(Iso);
        assert_eq!(draft.result(), Err((1, Missing::Value)));
        assert!(draft.remove(1));
        assert_eq!(draft.result(), Err((1, Missing::Number)));
        assert!(!draft.remove(3));
        *draft.rows_mut()[1].text_mut() = "1600".into();
        assert_eq!(
            draft.result().unwrap(),
            [first, Row::new(Iso, Condition::AtLeast(1600.0)).unwrap()]
        );
        draft.clear();
        assert_eq!(draft.result(), Ok(Vec::new()));
    }

    #[test]
    fn value_lists_request_only_the_groups_they_need() {
        let mut draft = FilterDraft::default();
        draft.add();
        draft.rows_mut()[0].set_field(Camera);
        assert!(draft.list_groups().is_empty());
        draft.rows_mut()[0].set_kind(OneOf);
        assert_eq!(draft.list_groups(), [Group::Exif]);
        draft.clear();
        for (index, field) in [Keywords, City, FileType].into_iter().enumerate() {
            draft.add();
            let row = &mut draft.rows_mut()[index];
            row.set_field(field);
            row.set_kind(OneOf);
        }
        assert_eq!(draft.list_groups(), [Group::Iptc]);
    }

    #[test]
    fn entries_use_counted_spelling_and_keep_absent_picks_in_order() {
        let counts = [("Bird".into(), 2), ("Tree".into(), 1)];
        let picks = ["Zoo".into(), " bird ".into(), "Ant".into()];
        assert_eq!(
            list_entries(&counts, &picks),
            [
                ("Bird".into(), 2, true),
                ("Tree".into(), 1, false),
                ("Zoo".into(), 0, true),
                ("Ant".into(), 0, true)
            ]
        );
    }
}
