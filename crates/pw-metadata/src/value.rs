//! The metadata value model (ADR 0007 §3, §8, §10).
//!
//! - **One model keyed by the XMP id** (§3): a [`Metadata`] holds one [`PropertyValue`] per IPTC
//!   property, keyed by the property's XMP id from [`crate::properties`]. IIM datasets and Exif tags
//!   are only representations; each value carries the [`Origin`] it was read from.
//! - **Value types follow the spec** (§3): a value must fit the kind and occurrence of its property
//!   in the property table, and inside a structure every field must fit its field definition, at
//!   any depth.
//! - **Lang Alt** (§8, Q7): RawStride edits only `x-default`; every other language is kept as it
//!   is, even one that held the same text as the old `x-default`. Without `x-default` the first
//!   entry is shown, with its language. Clearing removes the whole property, all languages.
//! - **Cardinalities** (§8): the limits of [`crate::properties::max_entries`] apply to edits only;
//!   everything that is read is kept.
//! - **Clearing is always explicit** (§10): an edit with an empty value is an error; a property is
//!   removed only through [`Metadata::clear`].
//!
//! Conflicts are data only here; filling them is the reconciliation step (I5).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::date::IptcDate;
use crate::properties::{self, Occurrence, ValueKind};

/// The language of the default entry of a Lang Alt value.
const X_DEFAULT: &str = "x-default";

/// Where a value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// XMP embedded in the image file.
    XmpEmbedded,
    /// An XMP sidecar next to the image file.
    XmpSidecar,
    /// An IIM (IPTC-IIM) dataset.
    Iim,
    /// An Exif tag.
    Exif,
    /// Changed by the user in RawStride.
    Edited,
}

/// A Lang Alt value: `(language, text)` entries in file order (ADR 0007 §8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LangAlt {
    entries: Vec<(String, String)>,
}

impl LangAlt {
    /// The entries in file order. Languages are compared without regard to case; a repeated
    /// language keeps its first entry.
    pub fn from_entries(entries: Vec<(String, String)>) -> LangAlt {
        let mut kept: Vec<(String, String)> = Vec::with_capacity(entries.len());
        for (language, text) in entries {
            if !kept
                .iter()
                .any(|(seen, _)| seen.eq_ignore_ascii_case(&language))
            {
                kept.push((language, text));
            }
        }
        LangAlt { entries: kept }
    }

    /// The entries in file order.
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    /// Sets the text of `x-default` (Q7): if it exists only its text changes; otherwise it is
    /// inserted first. Every other entry stays as it is, including one that held the same text as
    /// the old `x-default`. An empty text or one of only spaces is `ValueError::Empty`; clearing
    /// goes through [`Metadata::clear`].
    pub fn set_default(&mut self, text: &str) -> Result<(), ValueError> {
        if is_empty_text(text) {
            return Err(ValueError::Empty(X_DEFAULT.to_string()));
        }
        match self
            .entries
            .iter_mut()
            .find(|(language, _)| language.eq_ignore_ascii_case(X_DEFAULT))
        {
            Some((_, old)) => *old = text.to_string(),
            None => self
                .entries
                .insert(0, (X_DEFAULT.to_string(), text.to_string())),
        }
        Ok(())
    }

    /// The text to show (§8 reading rule): the `x-default` text with `None`; without `x-default`
    /// the first entry with `Some(language)`; `None` when there is no entry.
    pub fn display(&self) -> Option<(&str, Option<&str>)> {
        if let Some((_, text)) = self
            .entries
            .iter()
            .find(|(language, _)| language.eq_ignore_ascii_case(X_DEFAULT))
        {
            return Some((text.as_str(), None));
        }
        self.entries
            .first()
            .map(|(language, text)| (text.as_str(), Some(language.as_str())))
    }
}

/// A value of a property or a structure field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Text, URI, URL and the `Any` field of an image region.
    Text(String),
    /// A Lang Alt value.
    LangAlt(LangAlt),
    /// A `Multi` property or field that is not a structure; Seq or Bag from
    /// [`properties::list_kind`] / [`properties::field_list_kind`].
    List(Vec<String>),
    /// A date, possibly truncated.
    Date(IptcDate),
    /// A number, the text exactly as read, so a rating keeps its full precision (§2.9, §8).
    Number(String),
    /// A `Single` structure.
    Struct(StructValue),
    /// A `Multi` structure.
    StructList(Vec<StructValue>),
}

/// The fields of a structure, keyed by the field's XMP id, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructValue {
    pub fields: Vec<(String, Value)>,
}

/// A value and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyValue {
    pub value: Value,
    pub origin: Origin,
}

/// The rule that decided a conflict between representations (ADR 0007 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictRule {
    /// The XMP value was used.
    Xmp,
    /// The IIM value was used.
    Iim,
    /// The Exif value was used.
    Exif,
    /// No rule applies: the user decides.
    Open,
}

/// Different values of one property in different representations. Data only; filling it is I5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The property's XMP id.
    pub property: String,
    pub values: Vec<(Origin, Value)>,
    pub rule: ConflictRule,
}

/// Why a value was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueError {
    /// The XMP id is not in the property table.
    UnknownProperty(String),
    /// The value does not fit the kind of the property or field. `property` is the XMP id, or for
    /// a field inside a structure the path of XMP ids separated by `/`.
    Kind { property: String, expected: String },
    /// An edit with more entries than the property allows.
    TooMany { property: String, max: usize },
    /// An edit with an empty value, or one of only spaces; holds the property.
    Empty(String),
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueError::UnknownProperty(property) => write!(f, "unknown property {property}"),
            ValueError::Kind { property, expected } => {
                write!(f, "{property}: expected {expected}")
            }
            ValueError::TooMany { property, max } => {
                write!(f, "{property}: at most {max} entries are allowed")
            }
            ValueError::Empty(property) => {
                write!(
                    f,
                    "{property}: the value is empty (clear the property instead)"
                )
            }
        }
    }
}

impl std::error::Error for ValueError {}

/// The metadata of one image: values keyed by XMP id, the conflicts, and the changed properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    values: BTreeMap<String, PropertyValue>,
    conflicts: Vec<Conflict>,
    changed: BTreeSet<String>,
    /// The list properties whose items have qualifiers in the file (ADR 0007 §8), by XMP id.
    qualified: BTreeSet<String>,
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata::default()
    }

    /// The value of a property.
    pub fn get(&self, xmp_id: &str) -> Option<&PropertyValue> {
        self.values.get(xmp_id)
    }

    /// The conflicts found when reading.
    pub fn conflicts(&self) -> &[Conflict] {
        &self.conflicts
    }

    /// Records a conflict (for the reconciliation, I5).
    pub fn add_conflict(&mut self, conflict: Conflict) {
        self.conflicts.push(conflict);
    }

    /// Stores a value that was read. The property must be known and the value must fit its kind;
    /// there is no cardinality limit (§8: everything read is kept). Not marked changed.
    pub fn insert_read(
        &mut self,
        xmp_id: &str,
        value: Value,
        origin: Origin,
    ) -> Result<(), ValueError> {
        let def = properties::property(xmp_id)
            .ok_or_else(|| ValueError::UnknownProperty(xmp_id.to_string()))?;
        check_kind(xmp_id, def.kind, def.occurrence, &value)?;
        self.values
            .insert(xmp_id.to_string(), PropertyValue { value, origin });
        Ok(())
    }

    /// Stores an edit: as [`Metadata::insert_read`], plus the cardinality limit (`TooMany`) and no
    /// empty value (`Empty`). The origin becomes `Edited` and the property is marked changed.
    ///
    /// A Lang Alt edit sets `x-default` (Q7) to the text the new value shows
    /// ([`LangAlt::display`]) through [`LangAlt::set_default`] on the existing value, so the other
    /// languages are kept. Without an existing Lang Alt the new value's entries are the start.
    pub fn set(&mut self, xmp_id: &str, value: Value) -> Result<(), ValueError> {
        let def = properties::property(xmp_id)
            .ok_or_else(|| ValueError::UnknownProperty(xmp_id.to_string()))?;
        check_kind(xmp_id, def.kind, def.occurrence, &value)?;
        if !has_content(&value) {
            return Err(ValueError::Empty(xmp_id.to_string()));
        }
        if let Some(max) = properties::max_entries(xmp_id)
            && entry_count(&value) > max
        {
            return Err(ValueError::TooMany {
                property: xmp_id.to_string(),
                max,
            });
        }
        let value = match value {
            Value::LangAlt(new) => {
                let text = match new.display() {
                    Some((text, _)) => text.to_string(),
                    None => return Err(ValueError::Empty(xmp_id.to_string())),
                };
                let mut lang_alt = match self.values.get(xmp_id) {
                    Some(PropertyValue {
                        value: Value::LangAlt(old),
                        ..
                    }) => old.clone(),
                    _ => new,
                };
                lang_alt
                    .set_default(&text)
                    .map_err(|_| ValueError::Empty(xmp_id.to_string()))?;
                Value::LangAlt(lang_alt)
            }
            other => other,
        };
        self.values.insert(
            xmp_id.to_string(),
            PropertyValue {
                value,
                origin: Origin::Edited,
            },
        );
        self.changed.insert(xmp_id.to_string());
        Ok(())
    }

    /// Removes the value of a property (all languages of a Lang Alt) and marks it changed (§8,
    /// §10).
    pub fn clear(&mut self, xmp_id: &str) -> Result<(), ValueError> {
        if properties::property(xmp_id).is_none() {
            return Err(ValueError::UnknownProperty(xmp_id.to_string()));
        }
        self.values.remove(xmp_id);
        self.changed.insert(xmp_id.to_string());
        Ok(())
    }

    /// Each changed property with its new value, or `None` when it was cleared.
    pub fn changes(&self) -> impl Iterator<Item = (&str, Option<&Value>)> {
        self.changed.iter().map(|xmp_id| {
            (
                xmp_id.as_str(),
                self.values.get(xmp_id).map(|stored| &stored.value),
            )
        })
    }

    /// Marks a list property whose items have qualifiers in the file, which RawStride cannot keep
    /// when it writes the list (ADR 0007 §8): it is shown, never written to XMP.
    pub fn mark_qualified(&mut self, xmp_id: &str) {
        self.qualified.insert(xmp_id.to_string());
    }

    /// True for a list property marked by [`Metadata::mark_qualified`] (ADR 0007 §8).
    pub fn has_qualifiers(&self, xmp_id: &str) -> bool {
        self.qualified.contains(xmp_id)
    }

    /// Puts a property back as it was read (ADR 0007 §8, a changed list with qualifiers): it gets
    /// the value and origin it has in `original`, or none if `original` has none, and is no longer
    /// marked changed. Conflicts are not touched.
    pub fn restore(&mut self, original: &Metadata, xmp_id: &str) {
        match original.values.get(xmp_id) {
            Some(stored) => {
                self.values.insert(xmp_id.to_string(), stored.clone());
            }
            None => {
                self.values.remove(xmp_id);
            }
        }
        self.changed.remove(xmp_id);
    }
}

/// True for an empty text or one of only spaces.
fn is_empty_text(text: &str) -> bool {
    text.chars().all(|c| c == ' ')
}

/// False for a value without content: an empty text, a list or Lang Alt whose entries are all
/// empty, a structure whose fields are all empty (at any depth).
fn has_content(value: &Value) -> bool {
    match value {
        Value::Text(text) | Value::Number(text) => !is_empty_text(text),
        Value::LangAlt(lang_alt) => lang_alt
            .entries()
            .iter()
            .any(|(_, text)| !is_empty_text(text)),
        Value::List(entries) => entries.iter().any(|text| !is_empty_text(text)),
        Value::Date(_) => true,
        Value::Struct(fields) => struct_has_content(fields),
        Value::StructList(list) => list.iter().any(struct_has_content),
    }
}

fn struct_has_content(fields: &StructValue) -> bool {
    fields.fields.iter().any(|(_, value)| has_content(value))
}

/// The number of entries a cardinality limit counts.
fn entry_count(value: &Value) -> usize {
    match value {
        Value::List(entries) => entries.len(),
        Value::StructList(list) => list.len(),
        _ => 1,
    }
}

/// Checks that `value` fits `kind` and `occurrence` (ADR 0007 §3); for a structure, every field
/// at any depth. `path` names the property, or the field as `property/field/…`.
pub(crate) fn check_kind(
    path: &str,
    kind: ValueKind,
    occurrence: Occurrence,
    value: &Value,
) -> Result<(), ValueError> {
    let kind_error = |expected: String| ValueError::Kind {
        property: path.to_string(),
        expected,
    };
    match (kind, occurrence) {
        (ValueKind::Struct(name), Occurrence::Single) => match value {
            Value::Struct(fields) => check_struct(path, name, fields),
            _ => Err(kind_error(format!("a {name} structure"))),
        },
        (ValueKind::Struct(name), Occurrence::Multi) => match value {
            Value::StructList(list) => list
                .iter()
                .try_for_each(|fields| check_struct(path, name, fields)),
            _ => Err(kind_error(format!("a list of {name} structures"))),
        },
        (_, Occurrence::Multi) => match value {
            Value::List(_) => Ok(()),
            _ => Err(kind_error("a list".to_string())),
        },
        (
            ValueKind::Text | ValueKind::Uri | ValueKind::Url | ValueKind::Any,
            Occurrence::Single,
        ) => match value {
            Value::Text(_) => Ok(()),
            _ => Err(kind_error("a text".to_string())),
        },
        (ValueKind::LangAlt, Occurrence::Single) => match value {
            Value::LangAlt(_) => Ok(()),
            _ => Err(kind_error("a Lang Alt value".to_string())),
        },
        (ValueKind::Date, Occurrence::Single) => match value {
            Value::Date(_) => Ok(()),
            _ => Err(kind_error("a date".to_string())),
        },
        (ValueKind::Integer, Occurrence::Single) => match value {
            Value::Number(text) if text.trim().parse::<i64>().is_ok() => Ok(()),
            _ => Err(kind_error("an integer".to_string())),
        },
        (ValueKind::Number, Occurrence::Single) => match value {
            Value::Number(text) if text.trim().parse::<f64>().is_ok_and(f64::is_finite) => Ok(()),
            _ => Err(kind_error("a number".to_string())),
        },
    }
}

/// Checks every field of a structure value against the structure `name`.
fn check_struct(path: &str, name: &str, fields: &StructValue) -> Result<(), ValueError> {
    let def = properties::struct_def(name).ok_or_else(|| ValueError::Kind {
        property: path.to_string(),
        expected: format!("a known structure (not {name})"),
    })?;
    for (field_id, field_value) in &fields.fields {
        let field_path = format!("{path}/{field_id}");
        let field = def
            .fields
            .iter()
            .find(|field| field.xmp_id == field_id)
            .ok_or_else(|| ValueError::Kind {
                property: field_path.clone(),
                expected: format!("a field of {name}"),
            })?;
        check_kind(&field_path, field.kind, field.occurrence, field_value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang_alt(entries: &[(&str, &str)]) -> LangAlt {
        LangAlt::from_entries(
            entries
                .iter()
                .map(|(language, text)| (language.to_string(), text.to_string()))
                .collect(),
        )
    }

    fn pairs(lang_alt: &LangAlt) -> Vec<(&str, &str)> {
        lang_alt
            .entries()
            .iter()
            .map(|(language, text)| (language.as_str(), text.as_str()))
            .collect()
    }

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    fn list(entries: &[&str]) -> Value {
        Value::List(entries.iter().map(|entry| entry.to_string()).collect())
    }

    fn structure(fields: Vec<(&str, Value)>) -> StructValue {
        StructValue {
            fields: fields
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect(),
        }
    }

    fn kind_error_path(result: Result<(), ValueError>) -> String {
        match result {
            Err(ValueError::Kind { property, .. }) => property,
            other => panic!("expected a Kind error, got {other:?}"),
        }
    }

    #[test]
    fn set_default_changes_only_x_default() -> Result<(), ValueError> {
        // Q7: the `sv` entry that held the old `x-default` text is kept as it is.
        let mut value = lang_alt(&[("x-default", "A"), ("sv", "A"), ("en", "B")]);
        value.set_default("C")?;
        assert_eq!(
            pairs(&value),
            [("x-default", "C"), ("sv", "A"), ("en", "B")]
        );

        let mut value = lang_alt(&[("sv", "A")]);
        value.set_default("C")?;
        assert_eq!(pairs(&value), [("x-default", "C"), ("sv", "A")]);
        Ok(())
    }

    #[test]
    fn display_follows_the_reading_rule() {
        assert_eq!(lang_alt(&[("sv", "A")]).display(), Some(("A", Some("sv"))));
        assert_eq!(
            lang_alt(&[("en", "B"), ("x-default", "A")]).display(),
            Some(("A", None))
        );
        assert_eq!(LangAlt::default().display(), None);
    }

    #[test]
    fn languages_are_compared_without_case_and_the_first_is_kept() -> Result<(), ValueError> {
        let mut value = lang_alt(&[("X-Default", "A"), ("sv", "B")]);
        assert_eq!(value.display(), Some(("A", None)));
        value.set_default("C")?;
        assert_eq!(pairs(&value), [("X-Default", "C"), ("sv", "B")]);

        let value = lang_alt(&[("sv", "first"), ("en", "B"), ("SV", "second")]);
        assert_eq!(pairs(&value), [("sv", "first"), ("en", "B")]);
        Ok(())
    }

    #[test]
    fn set_default_with_an_empty_text_is_empty() {
        let mut value = lang_alt(&[("x-default", "A")]);
        assert!(matches!(value.set_default(""), Err(ValueError::Empty(_))));
        assert!(matches!(
            value.set_default("   "),
            Err(ValueError::Empty(_))
        ));
        assert_eq!(pairs(&value), [("x-default", "A")]);
    }

    #[test]
    fn lang_alt_set_keeps_the_other_languages() -> Result<(), ValueError> {
        let mut metadata = Metadata::new();
        metadata.insert_read(
            "dc:description",
            Value::LangAlt(lang_alt(&[("x-default", "A"), ("sv", "A")])),
            Origin::XmpEmbedded,
        )?;
        metadata.set(
            "dc:description",
            Value::LangAlt(lang_alt(&[("x-default", "B")])),
        )?;
        let stored = metadata.get("dc:description");
        assert_eq!(
            stored.map(|stored| &stored.value),
            Some(&Value::LangAlt(lang_alt(&[
                ("x-default", "B"),
                ("sv", "A")
            ])))
        );
        assert_eq!(stored.map(|stored| stored.origin), Some(Origin::Edited));
        Ok(())
    }

    #[test]
    fn set_and_clear_of_a_lang_alt_removes_all_languages() -> Result<(), ValueError> {
        let mut metadata = Metadata::new();
        metadata.insert_read(
            "dc:description",
            Value::LangAlt(lang_alt(&[("x-default", "A"), ("sv", "B"), ("en", "C")])),
            Origin::XmpSidecar,
        )?;
        metadata.set(
            "dc:description",
            Value::LangAlt(lang_alt(&[("x-default", "D")])),
        )?;
        metadata.clear("dc:description")?;
        assert_eq!(metadata.get("dc:description"), None);
        let changes: Vec<(&str, Option<&Value>)> = metadata.changes().collect();
        assert_eq!(changes, [("dc:description", None)]);
        Ok(())
    }

    #[test]
    fn cardinality_limits_apply_to_edits_only() -> Result<(), ValueError> {
        let creator = |name: &str| structure(vec![("plus:ImageCreatorName", text(name))]);
        let four = Value::StructList(vec![creator("A"), creator("B"), creator("C"), creator("D")]);
        let mut metadata = Metadata::new();
        assert_eq!(
            metadata.set("plus:ImageCreator", four.clone()),
            Err(ValueError::TooMany {
                property: "plus:ImageCreator".to_string(),
                max: 3
            })
        );
        metadata.insert_read("plus:ImageCreator", four.clone(), Origin::XmpEmbedded)?;
        assert_eq!(
            metadata
                .get("plus:ImageCreator")
                .map(|stored| &stored.value),
            Some(&four)
        );
        Ok(())
    }

    #[test]
    fn values_that_do_not_fit_the_kind() {
        let mut metadata = Metadata::new();
        for (xmp_id, value) in [
            ("dc:subject", text("a")),
            ("photoshop:City", list(&["Göteborg"])),
            ("xmp:Rating", Value::Number("abc".to_string())),
            (
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(structure(vec![("Iptc4xmpExt:City", text("Göteborg"))])),
            ),
        ] {
            assert!(
                matches!(
                    metadata.insert_read(xmp_id, value.clone(), Origin::XmpEmbedded),
                    Err(ValueError::Kind { .. })
                ),
                "insert_read {xmp_id}"
            );
            assert!(
                matches!(metadata.set(xmp_id, value), Err(ValueError::Kind { .. })),
                "set {xmp_id}"
            );
        }
        assert_eq!(
            kind_error_path(metadata.set(
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(structure(vec![("Iptc4xmpExt:City", text("Göteborg"))])),
            )),
            "Iptc4xmpCore:CreatorContactInfo/Iptc4xmpExt:City"
        );
        assert_eq!(metadata.changes().count(), 0);
    }

    #[test]
    fn fields_inside_structures_are_checked() {
        let mut metadata = Metadata::new();
        assert_eq!(
            metadata.set(
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(structure(vec![(
                    "Iptc4xmpCore:CiAdrCity",
                    text("Göteborg")
                )])),
            ),
            Ok(())
        );
        assert_eq!(
            kind_error_path(metadata.set(
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(structure(vec![(
                    "Iptc4xmpCore:CiAdrCity",
                    list(&["Göteborg"])
                )])),
            )),
            "Iptc4xmpCore:CreatorContactInfo/Iptc4xmpCore:CiAdrCity"
        );
        assert_eq!(
            metadata.set(
                "Iptc4xmpExt:LocationCreated",
                Value::StructList(vec![structure(vec![(
                    "Iptc4xmpExt:City",
                    text("Göteborg")
                )])]),
            ),
            Ok(())
        );
        // Two levels down: ImageRegion → RegionBoundary → rbVertices must be a list of points.
        let region = Value::StructList(vec![structure(vec![(
            "Iptc4xmpExt:RegionBoundary",
            Value::Struct(structure(vec![("Iptc4xmpExt:rbVertices", text("0,0"))])),
        )])]);
        assert_eq!(
            kind_error_path(metadata.insert_read(
                "Iptc4xmpExt:ImageRegion",
                region,
                Origin::XmpEmbedded
            )),
            "Iptc4xmpExt:ImageRegion/Iptc4xmpExt:RegionBoundary/Iptc4xmpExt:rbVertices"
        );
    }

    #[test]
    fn a_number_keeps_its_text() -> Result<(), ValueError> {
        let mut metadata = Metadata::new();
        metadata.set("xmp:Rating", Value::Number("2.5".to_string()))?;
        assert_eq!(
            metadata.get("xmp:Rating").map(|stored| &stored.value),
            Some(&Value::Number("2.5".to_string()))
        );
        Ok(())
    }

    #[test]
    fn set_with_an_empty_value_is_empty() {
        let mut metadata = Metadata::new();
        for (xmp_id, value) in [
            ("photoshop:City", text("")),
            ("photoshop:City", text("   ")),
            ("dc:subject", list(&[])),
            ("dc:description", Value::LangAlt(LangAlt::default())),
            (
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(StructValue::default()),
            ),
        ] {
            assert_eq!(
                metadata.set(xmp_id, value.clone()),
                Err(ValueError::Empty(xmp_id.to_string())),
                "{xmp_id} {value:?}"
            );
        }
        assert_eq!(metadata.changes().count(), 0);
    }

    #[test]
    fn an_unknown_property_is_refused() {
        let mut metadata = Metadata::new();
        let unknown = Err(ValueError::UnknownProperty("foo:Bar".to_string()));
        assert_eq!(metadata.set("foo:Bar", text("a")), unknown);
        assert_eq!(
            metadata.insert_read("foo:Bar", text("a"), Origin::Iim),
            unknown
        );
        assert_eq!(metadata.clear("foo:Bar"), unknown);
        assert_eq!(metadata.changes().count(), 0);
    }

    #[test]
    fn insert_read_marks_nothing_changed_and_set_is_edited() -> Result<(), ValueError> {
        let mut metadata = Metadata::new();
        metadata.insert_read("photoshop:City", text("Göteborg"), Origin::Iim)?;
        assert_eq!(metadata.changes().count(), 0);
        assert_eq!(
            metadata.get("photoshop:City").map(|stored| stored.origin),
            Some(Origin::Iim)
        );

        metadata.set("photoshop:City", text("Malmö"))?;
        assert_eq!(
            metadata.get("photoshop:City"),
            Some(&PropertyValue {
                value: text("Malmö"),
                origin: Origin::Edited
            })
        );
        let changes: Vec<(&str, Option<&Value>)> = metadata.changes().collect();
        assert_eq!(changes, [("photoshop:City", Some(&text("Malmö")))]);
        Ok(())
    }

    #[test]
    fn restore_puts_back_the_read_value_and_unmarks_the_change() -> Result<(), ValueError> {
        let mut metadata = Metadata::new();
        metadata.insert_read("dc:subject", list(&["a", "b"]), Origin::XmpSidecar)?;
        let original = metadata.clone();
        metadata.set("dc:subject", list(&["c"]))?;
        metadata.restore(&original, "dc:subject");
        assert_eq!(
            metadata.get("dc:subject"),
            Some(&PropertyValue {
                value: list(&["a", "b"]),
                origin: Origin::XmpSidecar
            })
        );
        assert_eq!(metadata.changes().count(), 0);
        Ok(())
    }

    #[test]
    fn restore_of_a_property_the_file_lacked_removes_it() -> Result<(), ValueError> {
        let original = Metadata::new();
        let mut metadata = original.clone();
        metadata.set("dc:subject", list(&["c"]))?;
        metadata.restore(&original, "dc:subject");
        assert_eq!(metadata.get("dc:subject"), None);
        assert_eq!(metadata.changes().count(), 0);
        Ok(())
    }

    #[test]
    fn error_texts() {
        assert_eq!(
            ValueError::UnknownProperty("foo:Bar".to_string()).to_string(),
            "unknown property foo:Bar"
        );
        assert_eq!(
            ValueError::TooMany {
                property: "plus:ImageCreator".to_string(),
                max: 3
            }
            .to_string(),
            "plus:ImageCreator: at most 3 entries are allowed"
        );
    }
}
