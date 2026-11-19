//! The ingest template with the modes Keep, Replace, Append, Prefix and Clear (ADR 0007 §10, §8,
//! §15; I9b, T88).
//!
//! A [`Template`] is a list of [`TemplateField`]s, each naming a property of the table by its XMP
//! id (or one field of a single structure) with a [`Mode`] and a value. [`apply`] applies them to
//! the [`Metadata`] of one file, field by field in order, through [`Metadata::set`] and
//! [`Metadata::clear`]; the write plan and the safe write then write the result
//! ([`crate::ExifTool::apply_template_safely`]).
//!
//! - **Keep** changes nothing.
//! - **Replace** sets the template's value. A Lang Alt sets `x-default` and keeps the other
//!   languages (Q7, as `set` does); a list is replaced by the template's list.
//! - **Append** and **Prefix** (§10: Prefix is a form of Append) join the old and the new text of
//!   a text or a Lang Alt (its shown text, [`LangAlt::display`]) with one space (owner decision
//!   D5): Append is old, space, new; Prefix is new, space, old. Without an old text the result is
//!   the new text. On a list, Append adds the template's entries that are missing, at the end, in
//!   order, without duplicates (exact text); Prefix on a list is not allowed. On a date, a number
//!   or a whole structure neither is allowed.
//! - **An empty field never clears** (§10): Replace, Append or Prefix without a value, or with a
//!   value without content (an empty text or one of only white space, an empty list), changes
//!   nothing.
//! - **Clear** removes the property (every language of a Lang Alt). The copyright-management
//!   properties of §8 cannot be cleared by a template, nor can their fields. Nothing to clear is
//!   no change.
//! - **A field of a single structure** (`field`, e.g. `Iptc4xmpCore:CiEmailWork` of
//!   `Iptc4xmpCore:CreatorContactInfo`): the mode is applied to that field of the structure's
//!   current value (or of an empty one), and the whole structure is set; the other fields are
//!   kept. Clearing the last field clears the property. A field of a **list** of structures
//!   (Location Created, Image Creator, …) allows only Keep for now; it comes with the template
//!   editor.
//! - **A result equal to the current value** ([`same_value`]) is not set, so a template that is
//!   already in the file gives no change and an empty write plan.
//! - The cardinality limits come from `set` (`ValueError::TooMany`).
//!
//! The confirmation before a Clear (§10, once per template) belongs to the user interface (I10);
//! variables (`{date}` …) and template files in XMP (§10) come later.

use std::fmt;

use crate::properties::{self, Occurrence, ValueKind};
use crate::reconcile::same_value;
use crate::value::{LangAlt, Metadata, StructValue, Value, ValueError};

/// The language of the default entry of a Lang Alt value.
const X_DEFAULT: &str = "x-default";

/// The separator of Append and Prefix for a text or a Lang Alt (owner decision D5).
const SEPARATOR: &str = " ";

/// The copyright-management properties of §8, which a template never clears.
const COPYRIGHT_MANAGEMENT: &[&str] = &[
    "dc:rights",
    "dc:creator",
    "photoshop:Credit",
    "xmpRights:UsageTerms",
    "xmpRights:WebStatement",
    "plus:CopyrightOwner",
    "plus:Licensor",
    "plus:ImageCreator",
];

/// How a template field changes the file's value (§10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Leave the value as it is.
    Keep,
    /// Set the template's value.
    Replace,
    /// Add the template's value after the old one.
    Append,
    /// Add the template's value before the old one.
    Prefix,
    /// Remove the value.
    Clear,
}

/// One field of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateField {
    /// The XMP id of a property of the table.
    pub property: String,
    /// The XMP id of a field when `property` is a single structure (e.g.
    /// `Iptc4xmpCore:CiEmailWork` of `Iptc4xmpCore:CreatorContactInfo`).
    pub field: Option<String>,
    pub mode: Mode,
    /// The value for Replace, Append and Prefix; ignored by Keep and Clear.
    pub value: Option<Value>,
}

/// A metadata template: its fields, applied in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Template {
    pub fields: Vec<TemplateField>,
}

/// Why a template could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The property, or the field of a structure (`property/field`), is not in the table.
    UnknownProperty(String),
    /// A copyright-management property (§8), or one of its fields, cannot be cleared by a
    /// template; holds the property (or `property/field`).
    CannotClear(String),
    /// The mode cannot be used on this property or field (`property/field`).
    ModeNotAllowed { property: String, mode: Mode },
    /// The value was not accepted.
    Value(ValueError),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::UnknownProperty(property) => {
                write!(f, "unknown property {property}")
            }
            TemplateError::CannotClear(property) => write!(
                f,
                "{property} is a copyright-management property and cannot be cleared by a template"
            ),
            TemplateError::ModeNotAllowed { property, mode } => {
                write!(f, "{mode:?} cannot be used on {property}")
            }
            TemplateError::Value(error) => write!(f, "the value was not accepted: {error}"),
        }
    }
}

impl std::error::Error for TemplateError {}

impl From<ValueError> for TemplateError {
    fn from(error: ValueError) -> Self {
        TemplateError::Value(error)
    }
}

/// Applies `template` to `metadata`, field by field in order (see the module comment). On an
/// error the fields before it have been applied; the caller writes nothing then.
pub fn apply(template: &Template, metadata: &mut Metadata) -> Result<(), TemplateError> {
    for field in &template.fields {
        apply_field(field, metadata)?;
    }
    Ok(())
}

/// Applies one template field.
fn apply_field(field: &TemplateField, metadata: &mut Metadata) -> Result<(), TemplateError> {
    let property = field.property.as_str();
    let def = properties::property(property)
        .ok_or_else(|| TemplateError::UnknownProperty(property.to_string()))?;
    match &field.field {
        None => {
            if field.mode == Mode::Clear {
                if is_copyright_management(property) {
                    return Err(TemplateError::CannotClear(property.to_string()));
                }
                if metadata.get(property).is_some() {
                    metadata.clear(property)?;
                }
                return Ok(());
            }
            let current = metadata.get(property).map(|stored| &stored.value);
            let result = combine(
                property,
                def.kind,
                def.occurrence,
                field.mode,
                current,
                field.value.as_ref(),
            )?;
            if let Some(result) = result
                && !current.is_some_and(|current| same_value(property, current, &result))
            {
                metadata.set(property, result)?;
            }
            Ok(())
        }
        Some(sub) => apply_sub_field(field, sub, def.kind, def.occurrence, metadata),
    }
}

/// Applies a template field that names a field of a structure.
fn apply_sub_field(
    field: &TemplateField,
    sub: &str,
    kind: ValueKind,
    occurrence: Occurrence,
    metadata: &mut Metadata,
) -> Result<(), TemplateError> {
    let property = field.property.as_str();
    let path = format!("{property}/{sub}");
    let ValueKind::Struct(name) = kind else {
        return Err(TemplateError::UnknownProperty(path));
    };
    let struct_def =
        properties::struct_def(name).ok_or_else(|| TemplateError::UnknownProperty(path.clone()))?;
    let field_def = struct_def
        .fields
        .iter()
        .find(|candidate| candidate.xmp_id == sub)
        .ok_or_else(|| TemplateError::UnknownProperty(path.clone()))?;
    if field.mode == Mode::Clear && is_copyright_management(property) {
        return Err(TemplateError::CannotClear(path));
    }
    if field.mode == Mode::Keep {
        return Ok(());
    }
    if occurrence == Occurrence::Multi {
        return Err(TemplateError::ModeNotAllowed {
            property: path,
            mode: field.mode,
        });
    }

    let current = match metadata.get(property).map(|stored| &stored.value) {
        Some(Value::Struct(fields)) => Some(fields),
        _ => None,
    };
    let current_field = current.and_then(|fields| {
        fields
            .fields
            .iter()
            .find(|(id, _)| id == sub)
            .map(|(_, value)| value)
    });

    if field.mode == Mode::Clear {
        let Some(fields) = current else {
            return Ok(());
        };
        if current_field.is_none() {
            return Ok(());
        }
        let rest = StructValue {
            fields: fields
                .fields
                .iter()
                .filter(|(id, _)| id != sub)
                .cloned()
                .collect(),
        };
        if rest.fields.iter().any(|(_, value)| has_content(value)) {
            metadata.set(property, Value::Struct(rest))?;
        } else {
            metadata.clear(property)?;
        }
        return Ok(());
    }

    let Some(new_field) = combine(
        &path,
        field_def.kind,
        field_def.occurrence,
        field.mode,
        current_field,
        field.value.as_ref(),
    )?
    else {
        return Ok(());
    };
    let mut result = current.cloned().unwrap_or_default();
    match result.fields.iter_mut().find(|(id, _)| id == sub) {
        Some((_, value)) => *value = new_field,
        None => result.fields.push((sub.to_string(), new_field)),
    }
    // Fields in the order of the structure, as the reader gives them, so a structure that is
    // already in the file compares equal.
    let position = |id: &str| {
        struct_def
            .fields
            .iter()
            .position(|candidate| candidate.xmp_id == id)
            .unwrap_or(usize::MAX)
    };
    result.fields.sort_by_key(|(id, _)| position(id));
    let result = Value::Struct(result);
    if current.is_some_and(|fields| Value::Struct(fields.clone()) == result) {
        return Ok(());
    }
    metadata.set(property, result)?;
    Ok(())
}

/// The value `mode` makes of `current` and the template's `new` value for a property or field of
/// `kind` and `occurrence`; `None` when nothing is to be set (Keep, or no value with content).
/// `path` names the property or field in errors. Not for Clear.
fn combine(
    path: &str,
    kind: ValueKind,
    occurrence: Occurrence,
    mode: Mode,
    current: Option<&Value>,
    new: Option<&Value>,
) -> Result<Option<Value>, TemplateError> {
    let not_allowed = || TemplateError::ModeNotAllowed {
        property: path.to_string(),
        mode,
    };
    let joins = matches!(mode, Mode::Append | Mode::Prefix);
    let shape = Shape::of(kind, occurrence);
    if joins {
        match shape {
            Shape::Text | Shape::LangAlt => {}
            Shape::List if mode == Mode::Append => {}
            Shape::List | Shape::Other => return Err(not_allowed()),
        }
    }
    let new = match (mode, new) {
        (Mode::Keep | Mode::Clear, _) | (_, None) => return Ok(None),
        (_, Some(new)) if !has_content(new) => return Ok(None),
        (_, Some(new)) => new,
    };
    let kind_error = |expected: &str| {
        TemplateError::Value(ValueError::Kind {
            property: path.to_string(),
            expected: expected.to_string(),
        })
    };

    if mode == Mode::Replace {
        return Ok(Some(match (shape, current, new) {
            // Inside a structure `set` does not merge a Lang Alt, so do it here: `x-default`
            // changes, the other languages stay (Q7). At the top level `set` gives the same.
            (Shape::LangAlt, Some(Value::LangAlt(old)), Value::LangAlt(new_alt)) => {
                let Some((text, _)) = new_alt.display() else {
                    return Ok(None);
                };
                let mut merged = old.clone();
                merged.set_default(text)?;
                Value::LangAlt(merged)
            }
            _ => new.clone(),
        }));
    }

    match shape {
        Shape::Text => {
            let Value::Text(new_text) = new else {
                return Err(kind_error("a text"));
            };
            let old = match current {
                Some(Value::Text(old)) => Some(old.as_str()),
                _ => None,
            };
            Ok(Some(Value::Text(join(mode, old, new_text))))
        }
        Shape::LangAlt => {
            let Value::LangAlt(new_alt) = new else {
                return Err(kind_error("a Lang Alt value"));
            };
            let Some((new_text, _)) = new_alt.display() else {
                return Ok(None);
            };
            let old = match current {
                Some(Value::LangAlt(old)) => Some(old),
                _ => None,
            };
            let old_text = old.and_then(LangAlt::display).map(|(text, _)| text);
            let joined = join(mode, old_text, new_text);
            let mut result = old.cloned().unwrap_or_default();
            result.set_default(&joined)?;
            Ok(Some(Value::LangAlt(result)))
        }
        Shape::List => {
            let Value::List(entries) = new else {
                return Err(kind_error("a list"));
            };
            let mut result = match current {
                Some(Value::List(old)) => old.clone(),
                _ => Vec::new(),
            };
            for entry in entries {
                if !is_blank(entry) && !result.contains(entry) {
                    result.push(entry.clone());
                }
            }
            Ok(Some(Value::List(result)))
        }
        Shape::Other => Err(not_allowed()),
    }
}

/// What Append and Prefix see of a property or field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A single text, URI, URL or `Any`.
    Text,
    /// A single Lang Alt.
    LangAlt,
    /// A list that is not of structures.
    List,
    /// A date, a number, a structure or a list of structures.
    Other,
}

impl Shape {
    fn of(kind: ValueKind, occurrence: Occurrence) -> Shape {
        match (kind, occurrence) {
            (ValueKind::Struct(_), _) => Shape::Other,
            (_, Occurrence::Multi) => Shape::List,
            (ValueKind::Text | ValueKind::Uri | ValueKind::Url | ValueKind::Any, _) => Shape::Text,
            (ValueKind::LangAlt, _) => Shape::LangAlt,
            (ValueKind::Date | ValueKind::Integer | ValueKind::Number, _) => Shape::Other,
        }
    }
}

/// Append: old, separator, new; Prefix: new, separator, old. Without an old text (or a blank
/// one), the new text.
fn join(mode: Mode, old: Option<&str>, new: &str) -> String {
    match old.filter(|old| !is_blank(old)) {
        None => new.to_string(),
        Some(old) if mode == Mode::Prefix => format!("{new}{SEPARATOR}{old}"),
        Some(old) => format!("{old}{SEPARATOR}{new}"),
    }
}

/// True for a copyright-management property of §8.
fn is_copyright_management(xmp_id: &str) -> bool {
    COPYRIGHT_MANAGEMENT.contains(&xmp_id)
}

/// True for an empty text or one of only white space.
fn is_blank(text: &str) -> bool {
    text.chars().all(char::is_whitespace)
}

/// False for a value without content: a blank text, a list or Lang Alt whose entries are all
/// blank, a structure whose fields are all without content (at any depth).
fn has_content(value: &Value) -> bool {
    match value {
        Value::Text(text) | Value::Number(text) => !is_blank(text),
        Value::LangAlt(lang_alt) => lang_alt.entries().iter().any(|(_, text)| !is_blank(text)),
        Value::List(entries) => entries.iter().any(|text| !is_blank(text)),
        Value::Date(_) => true,
        Value::Struct(fields) => fields.fields.iter().any(|(_, value)| has_content(value)),
        Value::StructList(list) => list
            .iter()
            .any(|fields| fields.fields.iter().any(|(_, value)| has_content(value))),
    }
}

/// A Lang Alt with only `x-default`.
pub(crate) fn x_default(text: &str) -> Value {
    Value::LangAlt(LangAlt::from_entries(vec![(
        X_DEFAULT.to_string(),
        text.to_string(),
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::IptcDate;
    use crate::value::Origin;

    const CONTACT: &str = "Iptc4xmpCore:CreatorContactInfo";
    const EMAIL: &str = "Iptc4xmpCore:CiEmailWork";
    const PHONE: &str = "Iptc4xmpCore:CiTelWork";

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    fn list(entries: &[&str]) -> Value {
        Value::List(entries.iter().map(|entry| entry.to_string()).collect())
    }

    fn lang_alt(entries: &[(&str, &str)]) -> Value {
        Value::LangAlt(LangAlt::from_entries(
            entries
                .iter()
                .map(|(language, text)| (language.to_string(), text.to_string()))
                .collect(),
        ))
    }

    fn structure(fields: &[(&str, Value)]) -> Value {
        Value::Struct(StructValue {
            fields: fields
                .iter()
                .map(|(id, value)| (id.to_string(), value.clone()))
                .collect(),
        })
    }

    fn field(property: &str, mode: Mode, value: Option<Value>) -> TemplateField {
        TemplateField {
            property: property.to_string(),
            field: None,
            mode,
            value,
        }
    }

    fn sub_field(property: &str, sub: &str, mode: Mode, value: Option<Value>) -> TemplateField {
        TemplateField {
            field: Some(sub.to_string()),
            ..field(property, mode, value)
        }
    }

    fn template(fields: Vec<TemplateField>) -> Template {
        Template { fields }
    }

    /// Metadata as read from a file: the values, nothing changed.
    fn read(values: &[(&str, Value)]) -> Metadata {
        let mut metadata = Metadata::new();
        for (xmp_id, value) in values {
            if let Err(error) = metadata.insert_read(xmp_id, value.clone(), Origin::XmpEmbedded) {
                panic!("{xmp_id}: {error}");
            }
        }
        metadata
    }

    fn value_of<'a>(metadata: &'a Metadata, xmp_id: &str) -> Option<&'a Value> {
        metadata.get(xmp_id).map(|stored| &stored.value)
    }

    /// Applies one field to `metadata` and returns the new value of its property.
    fn after(metadata: &mut Metadata, one: TemplateField) -> Option<Value> {
        let property = one.property.clone();
        if let Err(error) = apply(&template(vec![one]), metadata) {
            panic!("{property}: {error}");
        }
        value_of(metadata, &property).cloned()
    }

    fn changed(metadata: &Metadata) -> Vec<String> {
        metadata
            .changes()
            .map(|(xmp_id, _)| xmp_id.to_string())
            .collect()
    }

    // --- Text ---

    #[test]
    fn every_mode_on_a_text() {
        let city = || read(&[("photoshop:City", text("Göteborg"))]);
        let new = || Some(text("Lund"));

        let mut metadata = city();
        assert_eq!(
            after(&mut metadata, field("photoshop:City", Mode::Keep, new())),
            Some(text("Göteborg"))
        );
        assert!(changed(&metadata).is_empty());

        let mut metadata = city();
        assert_eq!(
            after(&mut metadata, field("photoshop:City", Mode::Replace, new())),
            Some(text("Lund"))
        );
        assert_eq!(changed(&metadata), ["photoshop:City"]);

        let mut metadata = city();
        assert_eq!(
            after(&mut metadata, field("photoshop:City", Mode::Append, new())),
            Some(text("Göteborg Lund"))
        );

        let mut metadata = city();
        assert_eq!(
            after(&mut metadata, field("photoshop:City", Mode::Prefix, new())),
            Some(text("Lund Göteborg"))
        );

        let mut metadata = city();
        assert_eq!(
            after(&mut metadata, field("photoshop:City", Mode::Clear, None)),
            None
        );
        assert_eq!(changed(&metadata), ["photoshop:City"]);
    }

    #[test]
    fn append_and_prefix_without_an_old_text_give_the_new_text() {
        for mode in [Mode::Append, Mode::Prefix] {
            let mut metadata = Metadata::new();
            assert_eq!(
                after(
                    &mut metadata,
                    field("photoshop:City", mode, Some(text("Lund")))
                ),
                Some(text("Lund")),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn clear_without_a_value_changes_nothing() {
        let mut metadata = Metadata::new();
        assert_eq!(
            after(&mut metadata, field("photoshop:City", Mode::Clear, None)),
            None
        );
        assert!(changed(&metadata).is_empty());
    }

    // --- Lang Alt ---

    #[test]
    fn every_mode_on_a_lang_alt() {
        let description = || {
            read(&[(
                "dc:description",
                lang_alt(&[("x-default", "Hamnen"), ("sv", "Hamnen på svenska")]),
            )])
        };
        let new = || Some(lang_alt(&[("x-default", "i dimma")]));

        let mut metadata = description();
        assert_eq!(
            after(&mut metadata, field("dc:description", Mode::Keep, new())),
            Some(lang_alt(&[
                ("x-default", "Hamnen"),
                ("sv", "Hamnen på svenska")
            ]))
        );
        assert!(changed(&metadata).is_empty());

        // Replace and Append change `x-default` only; the `sv` entry is kept (Q7).
        let mut metadata = description();
        assert_eq!(
            after(&mut metadata, field("dc:description", Mode::Replace, new())),
            Some(lang_alt(&[
                ("x-default", "i dimma"),
                ("sv", "Hamnen på svenska")
            ]))
        );

        let mut metadata = description();
        assert_eq!(
            after(&mut metadata, field("dc:description", Mode::Append, new())),
            Some(lang_alt(&[
                ("x-default", "Hamnen i dimma"),
                ("sv", "Hamnen på svenska")
            ]))
        );

        let mut metadata = description();
        assert_eq!(
            after(&mut metadata, field("dc:description", Mode::Prefix, new())),
            Some(lang_alt(&[
                ("x-default", "i dimma Hamnen"),
                ("sv", "Hamnen på svenska")
            ]))
        );

        // Clear removes every language.
        let mut metadata = description();
        assert_eq!(
            after(&mut metadata, field("dc:description", Mode::Clear, None)),
            None
        );
        assert_eq!(changed(&metadata), ["dc:description"]);
    }

    #[test]
    fn append_to_a_lang_alt_without_x_default_joins_the_shown_text() {
        let mut metadata = read(&[("dc:title", lang_alt(&[("sv", "Hamnen")]))]);
        assert_eq!(
            after(
                &mut metadata,
                field(
                    "dc:title",
                    Mode::Append,
                    Some(lang_alt(&[("x-default", "2026")]))
                )
            ),
            Some(lang_alt(&[("x-default", "Hamnen 2026"), ("sv", "Hamnen")]))
        );
    }

    // --- Lists ---

    #[test]
    fn every_mode_on_a_list() {
        let keywords = || read(&[("dc:subject", list(&["hav", "båt"]))]);

        let mut metadata = keywords();
        assert_eq!(
            after(
                &mut metadata,
                field("dc:subject", Mode::Keep, Some(list(&["sol"])))
            ),
            Some(list(&["hav", "båt"]))
        );
        assert!(changed(&metadata).is_empty());

        let mut metadata = keywords();
        assert_eq!(
            after(
                &mut metadata,
                field("dc:subject", Mode::Replace, Some(list(&["sol", "hav"])))
            ),
            Some(list(&["sol", "hav"]))
        );

        // Append keeps the order and adds only the missing entries, once each (exact text).
        let mut metadata = keywords();
        assert_eq!(
            after(
                &mut metadata,
                field(
                    "dc:subject",
                    Mode::Append,
                    Some(list(&["sol", "båt", "Hav", "sol", "moln"]))
                )
            ),
            Some(list(&["hav", "båt", "sol", "Hav", "moln"]))
        );

        let mut metadata = keywords();
        assert_eq!(
            after(&mut metadata, field("dc:subject", Mode::Clear, None)),
            None
        );
        assert_eq!(changed(&metadata), ["dc:subject"]);
    }

    #[test]
    fn append_of_entries_that_are_all_there_changes_nothing() {
        let mut metadata = read(&[("dc:subject", list(&["hav", "båt"]))]);
        after(
            &mut metadata,
            field("dc:subject", Mode::Append, Some(list(&["båt"]))),
        );
        assert!(changed(&metadata).is_empty());
    }

    // --- A field of a single structure ---

    #[test]
    fn every_mode_on_a_field_of_creator_contact_info() {
        let contact = || {
            read(&[(
                CONTACT,
                structure(&[(EMAIL, text("a@example.com")), (PHONE, text("031-12"))]),
            )])
        };

        let mut metadata = contact();
        assert_eq!(
            after(
                &mut metadata,
                sub_field(CONTACT, EMAIL, Mode::Keep, Some(text("b@example.com")))
            ),
            Some(structure(&[
                (EMAIL, text("a@example.com")),
                (PHONE, text("031-12"))
            ]))
        );
        assert!(changed(&metadata).is_empty());

        // The other field is kept in every mode.
        let mut metadata = contact();
        assert_eq!(
            after(
                &mut metadata,
                sub_field(CONTACT, EMAIL, Mode::Replace, Some(text("b@example.com")))
            ),
            Some(structure(&[
                (EMAIL, text("b@example.com")),
                (PHONE, text("031-12"))
            ]))
        );

        let mut metadata = contact();
        assert_eq!(
            after(
                &mut metadata,
                sub_field(CONTACT, PHONE, Mode::Append, Some(text("34")))
            ),
            Some(structure(&[
                (EMAIL, text("a@example.com")),
                (PHONE, text("031-12 34"))
            ]))
        );

        let mut metadata = contact();
        assert_eq!(
            after(
                &mut metadata,
                sub_field(CONTACT, PHONE, Mode::Prefix, Some(text("+46")))
            ),
            Some(structure(&[
                (EMAIL, text("a@example.com")),
                (PHONE, text("+46 031-12"))
            ]))
        );

        let mut metadata = contact();
        assert_eq!(
            after(&mut metadata, sub_field(CONTACT, EMAIL, Mode::Clear, None)),
            Some(structure(&[(PHONE, text("031-12"))]))
        );
        assert_eq!(changed(&metadata), [CONTACT]);
    }

    #[test]
    fn clearing_the_last_field_clears_the_structure() {
        let mut metadata = read(&[(CONTACT, structure(&[(EMAIL, text("a@example.com"))]))]);
        assert_eq!(
            after(&mut metadata, sub_field(CONTACT, EMAIL, Mode::Clear, None)),
            None
        );
        assert_eq!(changed(&metadata), [CONTACT]);
    }

    #[test]
    fn a_field_of_a_missing_structure_creates_it_in_the_order_of_the_structure() {
        let mut metadata = Metadata::new();
        let applied = apply(
            &template(vec![
                sub_field(CONTACT, PHONE, Mode::Replace, Some(text("031-12"))),
                sub_field(CONTACT, EMAIL, Mode::Replace, Some(text("a@example.com"))),
            ]),
            &mut metadata,
        );
        assert_eq!(applied, Ok(()));
        // `CiEmailWork` comes before `CiTelWork` in the structure.
        assert_eq!(
            value_of(&metadata, CONTACT),
            Some(&structure(&[
                (EMAIL, text("a@example.com")),
                (PHONE, text("031-12"))
            ]))
        );
    }

    // --- Keep and empty values ---

    #[test]
    fn keep_and_empty_values_change_nothing() {
        let original = read(&[
            ("photoshop:City", text("Göteborg")),
            ("dc:subject", list(&["hav"])),
            ("dc:title", lang_alt(&[("x-default", "Hamnen")])),
            (CONTACT, structure(&[(EMAIL, text("a@example.com"))])),
        ]);
        let mut fields = Vec::new();
        for mode in [Mode::Replace, Mode::Append, Mode::Prefix] {
            fields.push(field("photoshop:City", mode, None));
            fields.push(field("photoshop:City", mode, Some(text(""))));
            fields.push(field("photoshop:City", mode, Some(text("   "))));
            fields.push(field(
                "dc:title",
                mode,
                Some(lang_alt(&[("x-default", " ")])),
            ));
            fields.push(sub_field(CONTACT, EMAIL, mode, Some(text(" "))));
            fields.push(sub_field(CONTACT, EMAIL, mode, None));
        }
        for mode in [Mode::Replace, Mode::Append] {
            fields.push(field("dc:subject", mode, Some(list(&[]))));
            fields.push(field("dc:subject", mode, Some(list(&["", " "]))));
        }
        for property in ["photoshop:City", "dc:subject", "dc:title", CONTACT] {
            fields.push(field(property, Mode::Keep, None));
            fields.push(field(property, Mode::Keep, Some(text("x"))));
        }
        fields.push(sub_field(CONTACT, EMAIL, Mode::Keep, Some(text("b@x"))));
        let mut metadata = original.clone();
        assert_eq!(apply(&template(fields), &mut metadata), Ok(()));
        assert!(changed(&metadata).is_empty());
        assert_eq!(metadata, original);
    }

    // --- Errors ---

    #[test]
    fn copyright_management_properties_cannot_be_cleared() {
        for property in [
            "dc:rights",
            "dc:creator",
            "photoshop:Credit",
            "xmpRights:UsageTerms",
            "xmpRights:WebStatement",
            "plus:CopyrightOwner",
            "plus:Licensor",
            "plus:ImageCreator",
        ] {
            let mut metadata = Metadata::new();
            assert_eq!(
                apply(
                    &template(vec![field(property, Mode::Clear, None)]),
                    &mut metadata
                ),
                Err(TemplateError::CannotClear(property.to_string())),
                "{property}"
            );
            assert!(changed(&metadata).is_empty(), "{property}");
        }
        // Nor their fields.
        let mut metadata = Metadata::new();
        assert_eq!(
            apply(
                &template(vec![sub_field(
                    "plus:ImageCreator",
                    "plus:ImageCreatorName",
                    Mode::Clear,
                    None
                )]),
                &mut metadata
            ),
            Err(TemplateError::CannotClear(
                "plus:ImageCreator/plus:ImageCreatorName".to_string()
            ))
        );
    }

    #[test]
    fn modes_that_are_not_allowed() {
        let date = match IptcDate::parse_xmp("2026-11-04") {
            Ok(date) => Value::Date(date),
            Err(error) => panic!("{error:?}"),
        };
        let rating = || Some(Value::Number("3".to_string()));
        let cases = [
            field("photoshop:DateCreated", Mode::Append, Some(date.clone())),
            field("photoshop:DateCreated", Mode::Prefix, Some(date)),
            field("xmp:Rating", Mode::Append, rating()),
            field("xmp:Rating", Mode::Prefix, rating()),
            field("dc:subject", Mode::Prefix, Some(list(&["hav"]))),
            field(
                CONTACT,
                Mode::Append,
                Some(structure(&[(EMAIL, text("a@example.com"))])),
            ),
        ];
        for case in cases {
            let mut metadata = Metadata::new();
            assert_eq!(
                apply(&template(vec![case.clone()]), &mut metadata),
                Err(TemplateError::ModeNotAllowed {
                    property: case.property.clone(),
                    mode: case.mode
                }),
                "{case:?}"
            );
            assert!(changed(&metadata).is_empty());
        }
    }

    #[test]
    fn a_field_of_a_list_of_structures_allows_only_keep() {
        let path = "plus:ImageCreator/plus:ImageCreatorName";
        let creator_name = |mode: Mode| {
            sub_field(
                "plus:ImageCreator",
                "plus:ImageCreatorName",
                mode,
                Some(text("Anna")),
            )
        };
        for mode in [Mode::Replace, Mode::Append, Mode::Prefix] {
            let mut metadata = Metadata::new();
            assert_eq!(
                apply(&template(vec![creator_name(mode)]), &mut metadata),
                Err(TemplateError::ModeNotAllowed {
                    property: path.to_string(),
                    mode
                }),
                "{mode:?}"
            );
        }
        let mut metadata = Metadata::new();
        assert_eq!(
            apply(&template(vec![creator_name(Mode::Keep)]), &mut metadata),
            Ok(())
        );
        // The same for Location Created, which is not a copyright-management property.
        assert_eq!(
            apply(
                &template(vec![sub_field(
                    "Iptc4xmpExt:LocationCreated",
                    "Iptc4xmpExt:City",
                    Mode::Clear,
                    None
                )]),
                &mut metadata
            ),
            Err(TemplateError::ModeNotAllowed {
                property: "Iptc4xmpExt:LocationCreated/Iptc4xmpExt:City".to_string(),
                mode: Mode::Clear
            })
        );
        assert!(changed(&metadata).is_empty());
    }

    #[test]
    fn four_image_creators_are_too_many() {
        let creator = |name: &str| StructValue {
            fields: vec![("plus:ImageCreatorName".to_string(), text(name))],
        };
        let four = Value::StructList(vec![creator("A"), creator("B"), creator("C"), creator("D")]);
        let mut metadata = Metadata::new();
        assert_eq!(
            apply(
                &template(vec![field("plus:ImageCreator", Mode::Replace, Some(four))]),
                &mut metadata
            ),
            Err(TemplateError::Value(ValueError::TooMany {
                property: "plus:ImageCreator".to_string(),
                max: 3
            }))
        );
    }

    #[test]
    fn unknown_properties_and_fields() {
        let mut metadata = Metadata::new();
        let replace = |property: &str, sub: Option<&str>| TemplateField {
            field: sub.map(str::to_string),
            ..field(property, Mode::Replace, Some(text("x")))
        };
        assert_eq!(
            apply(&template(vec![replace("foo:Bar", None)]), &mut metadata),
            Err(TemplateError::UnknownProperty("foo:Bar".to_string()))
        );
        assert_eq!(
            apply(
                &template(vec![replace(CONTACT, Some("foo:Bar"))]),
                &mut metadata
            ),
            Err(TemplateError::UnknownProperty(format!("{CONTACT}/foo:Bar")))
        );
        assert_eq!(
            apply(
                &template(vec![replace("photoshop:City", Some(EMAIL))]),
                &mut metadata
            ),
            Err(TemplateError::UnknownProperty(format!(
                "photoshop:City/{EMAIL}"
            )))
        );
        assert!(changed(&metadata).is_empty());
    }

    // --- A template that is already in the file ---

    #[test]
    fn a_template_equal_to_the_file_changes_nothing() {
        let original = read(&[
            ("dc:creator", list(&["Anna Exempelström"])),
            (
                "dc:rights",
                lang_alt(&[("x-default", "© 2026 Anna Exempelström"), ("sv", "©")]),
            ),
            ("photoshop:Credit", text("Anna Exempelström")),
            (
                CONTACT,
                structure(&[(EMAIL, text("a@example.com")), (PHONE, text("031-12"))]),
            ),
            ("xmpRights:Marked", text("True")),
        ]);
        let mut metadata = original.clone();
        let applied = apply(
            &template(vec![
                field(
                    "dc:creator",
                    Mode::Replace,
                    Some(list(&["Anna Exempelström"])),
                ),
                field(
                    "dc:rights",
                    Mode::Replace,
                    Some(lang_alt(&[("x-default", "© 2026 Anna Exempelström")])),
                ),
                field(
                    "photoshop:Credit",
                    Mode::Replace,
                    Some(text("Anna Exempelström")),
                ),
                sub_field(CONTACT, PHONE, Mode::Replace, Some(text("031-12"))),
                sub_field(CONTACT, EMAIL, Mode::Replace, Some(text("a@example.com"))),
                field("xmpRights:Marked", Mode::Replace, Some(text("True"))),
                field("dc:subject", Mode::Clear, None),
            ]),
            &mut metadata,
        );
        assert_eq!(applied, Ok(()));
        assert!(changed(&metadata).is_empty(), "{:?}", changed(&metadata));
        assert_eq!(metadata, original);
    }

    #[test]
    fn errors_are_shown_in_english() {
        assert_eq!(
            TemplateError::UnknownProperty("foo:Bar".to_string()).to_string(),
            "unknown property foo:Bar"
        );
        assert_eq!(
            TemplateError::CannotClear("dc:creator".to_string()).to_string(),
            "dc:creator is a copyright-management property and cannot be cleared by a template"
        );
        assert_eq!(
            TemplateError::ModeNotAllowed {
                property: "dc:subject".to_string(),
                mode: Mode::Prefix
            }
            .to_string(),
            "Prefix cannot be used on dc:subject"
        );
        assert_eq!(
            TemplateError::Value(ValueError::Empty("photoshop:City".to_string())).to_string(),
            "the value was not accepted: photoshop:City: the value is empty (clear the property \
             instead)"
        );
    }
}
