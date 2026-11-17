//! Edits of one image written through the write plan and the safe write (ADR 0007 §8, §9, §10,
//! §15; I9a, T87; I9b, T88).
//!
//! [`ExifTool::write_rating_safely`] writes a star rating and a colour label the way §15 asks for
//! embedded writes: no longer with `-overwrite_original_in_place`, but as a [`WritePlan`] applied
//! by [`ExifTool::write_safely`] (§9: a verified temporary copy replaces the original):
//!
//! 1. A file kind that cannot hold ratings, or stars outside −1…5, is
//!    [`EditError::Unsupported`] with the same text as the old [`ExifTool::write_rating`]; nothing
//!    is read or written.
//! 2. The file is read with [`ExifTool::read_metadata`], which also gives the [`FileState`] the plan
//!    needs (IIM, checksum, Exif fields).
//! 3. **Rating** (`xmp:Rating`, §8): a number is set, `None` clears it. It is written only when it
//!    changes: the same number as read is no change, and `None` is no change while the file has no
//!    Rating or a Rating of 0 (§8: a missing value counts as 0, so a `0` written by another program
//!    is not removed).
//! 4. **Label** (`xmp:Label`): a text is set, `None` clears it; the same text as read, or `None`
//!    while the file has no Label, is no change.
//! 5. [`write_plan::plan`] makes the plan with the options' [`WriteSettings`]; then
//!    [`ExifTool::write_safely`] writes it. An empty plan is [`WriteReport::Unchanged`] there, so a
//!    rating that is already in the file writes nothing, and a manufacturer RAW without a sidecar
//!    gets none for an empty rating. The RAW file itself is never written (§4.2); its sidecar is.
//!
//! [`ExifTool::apply_template_safely`] writes an ingest template the same way (§9.5, §10): a file
//! kind that cannot hold metadata is [`EditError::Unsupported`] with the same text as the old
//! [`ExifTool::write_iptc`]; then the file is read, [`template::apply`] applies the template's
//! modes to its metadata (an error is [`EditError::Template`]), and the plan is made and written as
//! above. A template that is already in the file gives an empty plan and
//! [`WriteReport::Unchanged`].
//!
//! [`ExifTool::write_edits_safely`] writes the editor's [`Edit`]s of single properties (I10d,
//! T97): a file kind that cannot hold metadata is [`EditError::Unsupported`] with the same text as
//! [`ExifTool::write_iptc`]; then the file is read, [`apply_edits`] applies the edits in order, and
//! the plan is made and written as above. The result is an [`EditReport`]: the safe write's report
//! and the plan's [`WriteNotice`]s, which the user is told about (§5.2: an IIM value cut to its
//! byte limit; §5.4: IIM converted to UTF-8; Q1: a conflict kept and no checksum written). They are
//! empty when nothing was written, except that an **Entries** edit left alone because of an `Open`
//! conflict (below) is always named in a [`WriteNotice::ConflictKept`], also when nothing was
//! written, and only once (ADR 0017 U12d-Q6).
//!
//! - **Set** stores the value with [`Metadata::set`]; a Lang Alt edits `x-default` only and keeps
//!   the other languages (§8, Q7). A value equal to the current one ([`same_value`]) is no change,
//!   **unless** the property has an unresolved (`Open`) conflict (§5.1, Q1): then the user has
//!   decided it by choosing a value, even the one already shown, and the plan writes both forms.
//! - **Clear** removes every representation of the property (§8: every language of a Lang Alt,
//!   IIM, and the Exif fields that exist). Unlike a template (§10), an edit may clear the
//!   copyright-management properties. Nothing to clear (no value and no `Open` conflict) is no
//!   change.
//! - **Entries** adds and removes entries of a list (a repeated property that is not a structure,
//!   e.g. `dc:subject`) in the file's own entries (ADR 0017 U12d-Q1): every entry equal to one in
//!   `remove` (exact text) is taken out, then each entry of `add` the list lacks is appended once,
//!   in order (blank ones are ignored); the other entries are kept. A property with an `Open`
//!   conflict that no earlier edit of the same call has decided (changed), or a result equal to
//!   the file's entries, is no change (the conflict is kept, U12d-Q6); an empty result, or one of
//!   only blank entries, clears the property; any other property is [`EditError::Unsupported`].
//! - **A field of a single structure** (`field`, e.g. `Iptc4xmpCore:CiEmailWork` of
//!   `Iptc4xmpCore:CreatorContactInfo`) is set in, or removed from, the structure's current value
//!   (or an empty one), the other fields kept and a new field placed in the order of the
//!   structure's fields; then the whole structure is set (with the rule above) or, when no field is
//!   left, cleared. A field of a list of structures, or a `field` on a property that is not a
//!   structure, is [`EditError::Unsupported`].
//!
//! The old `write_rating`, `write_iptc` and `write_tags` are unchanged; the Tauri app still uses
//! them (owner decision D7).
//!
//! [`FileState`]: crate::write_plan::FileState

use std::fmt;
use std::path::Path;

use crate::iim::Fallback;
use crate::journal::Journal;
use crate::properties::{self, Occurrence, ValueKind};
use crate::reconcile::same_value;
use crate::safe_write::{ImageCheck, SafeWriteError, WriteReport};
use crate::template::{self, Template, TemplateError};
use crate::value::{ConflictRule, Metadata, StructValue, Value, ValueError};
use crate::write_plan::{self, PlanError, WriteNotice, WriteSettings};
use crate::{Error, ExifTool, Rating, supports_rating};

/// The Rating property (§8).
const RATING: &str = "xmp:Rating";
/// The colour label (not IPTC; kept for Adobe and Photo Mechanic, §3).
const LABEL: &str = "xmp:Label";

/// The choices that change how an edit is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditOptions {
    /// What the write plan writes (create IIM or not); the settings screen is I10.
    pub settings: WriteSettings,
    /// The encoding for IIM and Exif text that is neither ASCII nor UTF-8 when the file is read.
    pub fallback: Fallback,
}

/// Why an edit was not written.
#[derive(Debug)]
pub enum EditError {
    /// The file kind cannot hold the edit, or the value is out of range; holds the reason.
    Unsupported(String),
    /// The file could not be read.
    Read(Error),
    /// The new value was not accepted.
    Value(ValueError),
    /// The write plan could not be made.
    Plan(PlanError),
    /// The safe write failed.
    Write(SafeWriteError),
    /// The template could not be applied.
    Template(TemplateError),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Unsupported(reason) => write!(f, "{reason}"),
            EditError::Read(error) => write!(f, "the metadata could not be read: {error}"),
            EditError::Value(error) => write!(f, "the value was not accepted: {error}"),
            EditError::Plan(error) => write!(f, "the write could not be planned: {error}"),
            EditError::Write(error) => write!(f, "{error}"),
            EditError::Template(error) => {
                write!(f, "the template could not be applied: {error}")
            }
        }
    }
}

impl std::error::Error for EditError {}

/// One edit of the editor: a property of the table set or cleared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Sets `value` on `property`, or on its field `field` when `property` is a single structure.
    Set {
        /// The XMP id of a property of the table.
        property: String,
        /// The XMP id of a field of a **single** structure (as [`template::TemplateField::field`]).
        field: Option<String>,
        value: Value,
    },
    /// Clears `property` (every representation, §8), or removes its field `field`.
    Clear {
        /// The XMP id of a property of the table.
        property: String,
        /// The XMP id of a field of a **single** structure.
        field: Option<String>,
    },
    /// Adds and removes entries of a list `property` (a `Multi` property that is not a
    /// structure, `Value::List`), applied to the file's own entries (ADR 0017 U12d-Q1).
    Entries {
        /// The XMP id of a property of the table.
        property: String,
        /// The entries to add, in order.
        add: Vec<String>,
        /// The entries to remove.
        remove: Vec<String>,
    },
}

/// The result of [`ExifTool::write_edits_safely`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditReport {
    /// What the safe write did.
    pub report: WriteReport,
    /// What the user is told about the write (§5.2, §5.4, Q1); empty when nothing was written,
    /// except for a [`WriteNotice::ConflictKept`] of each [`Edit::Entries`] left alone because of
    /// an `Open` conflict (U12d-Q6).
    pub notices: Vec<WriteNotice>,
}

/// Applies `edits` to `metadata`, edit by edit in order (see the module comment). On an error the
/// edits before it have been applied; the caller writes nothing then.
pub fn apply_edits(metadata: &mut Metadata, edits: &[Edit]) -> Result<(), EditError> {
    for edit in edits {
        apply_edit(metadata, edit)?;
    }
    Ok(())
}

/// Applies one edit.
fn apply_edit(metadata: &mut Metadata, edit: &Edit) -> Result<(), EditError> {
    let (property, field, value) = match edit {
        Edit::Set {
            property,
            field,
            value,
        } => (property.as_str(), field.as_deref(), Some(value)),
        Edit::Clear { property, field } => (property.as_str(), field.as_deref(), None),
        Edit::Entries {
            property,
            add,
            remove,
        } => return apply_entries(metadata, property, add, remove),
    };
    let def = properties::property(property)
        .ok_or_else(|| EditError::Value(ValueError::UnknownProperty(property.to_string())))?;
    match (field, value) {
        (None, Some(value)) => set_if_changed(metadata, property, value.clone()),
        (None, None) => {
            if metadata.get(property).is_some() || has_open_conflict(metadata, property) {
                metadata.clear(property).map_err(EditError::Value)?;
            }
            Ok(())
        }
        (Some(field), value) => {
            let unsupported = || {
                EditError::Unsupported(format!(
                    "{property}/{field} cannot be edited field by field"
                ))
            };
            let ValueKind::Struct(name) = def.kind else {
                return Err(unsupported());
            };
            if def.occurrence == Occurrence::Multi {
                return Err(unsupported());
            }
            let unknown_field =
                || EditError::Value(ValueError::UnknownProperty(format!("{property}/{field}")));
            let struct_def = properties::struct_def(name).ok_or_else(unknown_field)?;
            if !struct_def.fields.iter().any(|def| def.xmp_id == field) {
                return Err(unknown_field());
            }
            let current = match metadata.get(property).map(|stored| &stored.value) {
                Some(Value::Struct(fields)) => fields.clone(),
                _ => StructValue::default(),
            };
            let had_field = current.fields.iter().any(|(id, _)| id == field);
            let mut result = current;
            match value {
                Some(value) => {
                    let new_field = match (result.fields.iter().find(|(id, _)| id == field), value)
                    {
                        // Inside a structure `set` does not merge a Lang Alt, so do it here:
                        // `x-default` changes, the other languages stay (§8, Q7).
                        (Some((_, Value::LangAlt(old))), Value::LangAlt(new)) => {
                            match new.display() {
                                Some((text, _)) => {
                                    let mut merged = old.clone();
                                    merged.set_default(text).map_err(EditError::Value)?;
                                    Value::LangAlt(merged)
                                }
                                None => value.clone(),
                            }
                        }
                        _ => value.clone(),
                    };
                    match result.fields.iter_mut().find(|(id, _)| id == field) {
                        Some((_, old)) => *old = new_field,
                        None => result.fields.push((field.to_string(), new_field)),
                    }
                    // Fields in the order of the structure, as the reader gives them, so a
                    // structure that is already in the file compares equal (as T88).
                    let position = |id: &str| {
                        struct_def
                            .fields
                            .iter()
                            .position(|candidate| candidate.xmp_id == id)
                            .unwrap_or(usize::MAX)
                    };
                    result.fields.sort_by_key(|(id, _)| position(id));
                    set_if_changed(metadata, property, Value::Struct(result))
                }
                None => {
                    if !had_field {
                        return Ok(());
                    }
                    result.fields.retain(|(id, _)| id != field);
                    if result.fields.is_empty() {
                        return metadata.clear(property).map_err(EditError::Value);
                    }
                    match set_if_changed(metadata, property, Value::Struct(result)) {
                        // Only fields without content are left: no field is left to keep.
                        Err(EditError::Value(ValueError::Empty(_))) => {
                            metadata.clear(property).map_err(EditError::Value)
                        }
                        other => other,
                    }
                }
            }
        }
    }
}

/// Applies [`Edit::Entries`]: removes the entries of `remove` from the file's own entries of the
/// list `property`, then appends each entry of `add` it does not hold yet (see the module comment).
fn apply_entries(
    metadata: &mut Metadata,
    property: &str,
    add: &[String],
    remove: &[String],
) -> Result<(), EditError> {
    let def = properties::property(property)
        .ok_or_else(|| EditError::Value(ValueError::UnknownProperty(property.to_string())))?;
    if def.occurrence != Occurrence::Multi || matches!(def.kind, ValueKind::Struct(_)) {
        return Err(EditError::Unsupported(format!(
            "{property} is not a list of entries"
        )));
    }
    // The file's XMP and IIM entries differ and the user has not decided them: left alone
    // (ADR 0017 U12d-Q6). An earlier edit of the same call that changed the property has decided
    // the conflict, so the entries are applied to its value.
    if has_open_conflict(metadata, property) && !is_changed(metadata, property) {
        return Ok(());
    }
    let current = match metadata.get(property).map(|stored| &stored.value) {
        Some(Value::List(entries)) => entries.clone(),
        _ => Vec::new(),
    };
    let mut result: Vec<String> = current
        .iter()
        .filter(|entry| !remove.contains(entry))
        .cloned()
        .collect();
    for entry in add {
        if !entry.trim().is_empty() && !result.contains(entry) {
            result.push(entry.clone());
        }
    }
    if result == current {
        return Ok(());
    }
    if result.is_empty() {
        return metadata.clear(property).map_err(EditError::Value);
    }
    match set_if_changed(metadata, property, Value::List(result)) {
        // Only blank entries are left: nothing is left to keep.
        Err(EditError::Value(ValueError::Empty(_))) => {
            metadata.clear(property).map_err(EditError::Value)
        }
        other => other,
    }
}

/// Sets `value` on `property`, except when it equals the current value and the property has no
/// `Open` conflict (choosing the value already shown in an `Open` conflict decides it, Q1).
fn set_if_changed(metadata: &mut Metadata, property: &str, value: Value) -> Result<(), EditError> {
    let unchanged = metadata
        .get(property)
        .is_some_and(|stored| same_value(property, &stored.value, &value));
    if unchanged && !has_open_conflict(metadata, property) {
        return Ok(());
    }
    metadata.set(property, value).map_err(EditError::Value)
}

/// True when an edit has changed (set or cleared) `property`.
fn is_changed(metadata: &Metadata, property: &str) -> bool {
    metadata.changes().any(|(changed, _)| changed == property)
}

/// True when `property` has a conflict with the rule `Open` (§5.1, Q1).
fn has_open_conflict(metadata: &Metadata, property: &str) -> bool {
    metadata
        .conflicts()
        .iter()
        .any(|conflict| conflict.property == property && conflict.rule == ConflictRule::Open)
}

impl ExifTool {
    /// Writes `rating` (stars and label) to `image` through the write plan and the safe write (see
    /// the module comment). `checks` are the caller's image checks; `journal` is the app's write
    /// journal.
    pub fn write_rating_safely(
        &mut self,
        image: &Path,
        rating: &Rating,
        options: &EditOptions,
        checks: &dyn ImageCheck,
        journal: &Journal,
    ) -> Result<WriteReport, EditError> {
        if !supports_rating(image) {
            return Err(EditError::Unsupported(format!(
                "ratings are not supported for {}",
                image.display()
            )));
        }
        if let Some(stars) = rating.stars
            && !(-1..=5).contains(&stars)
        {
            return Err(EditError::Unsupported(format!(
                "rating {stars} is outside -1..5"
            )));
        }
        let read = self
            .read_metadata(image, options.fallback)
            .map_err(EditError::Read)?;
        let mut metadata = read.reconciled.metadata;

        let current_rating = metadata.get(RATING).map(|stored| stored.value.clone());
        let new_rating = rating.stars.map(|stars| Value::Number(stars.to_string()));
        if rating_changes(current_rating.as_ref(), new_rating.as_ref()) {
            apply(&mut metadata, RATING, new_rating)?;
        }

        let current_label = metadata.get(LABEL).map(|stored| stored.value.clone());
        let new_label = rating.label.clone().map(Value::Text);
        if label_changes(current_label.as_ref(), new_label.as_ref()) {
            apply(&mut metadata, LABEL, new_label)?;
        }

        let plan =
            write_plan::plan(&metadata, &read.state, options.settings).map_err(EditError::Plan)?;
        self.write_safely(image, &plan, checks, journal)
            .map_err(EditError::Write)
    }

    /// Applies `template` to `image` through the write plan and the safe write (see the module
    /// comment). `checks` are the caller's image checks; `journal` is the app's write journal.
    pub fn apply_template_safely(
        &mut self,
        image: &Path,
        template: &Template,
        options: &EditOptions,
        checks: &dyn ImageCheck,
        journal: &Journal,
    ) -> Result<WriteReport, EditError> {
        if !supports_rating(image) {
            return Err(EditError::Unsupported(format!(
                "metadata is not supported for {}",
                image.display()
            )));
        }
        let read = self
            .read_metadata(image, options.fallback)
            .map_err(EditError::Read)?;
        let mut metadata = read.reconciled.metadata;
        template::apply(template, &mut metadata).map_err(EditError::Template)?;
        let plan =
            write_plan::plan(&metadata, &read.state, options.settings).map_err(EditError::Plan)?;
        self.write_safely(image, &plan, checks, journal)
            .map_err(EditError::Write)
    }

    /// Writes the editor's `edits` to `image` through the write plan and the safe write, and
    /// returns the report with the plan's notices (see the module comment). An [`Edit::Entries`]
    /// left alone because its property has an `Open` conflict adds a
    /// [`WriteNotice::ConflictKept`] for that property, also when the report is
    /// [`WriteReport::Unchanged`], never twice (U12d-Q6). `checks` are the caller's image checks;
    /// `journal` is the app's write journal.
    pub fn write_edits_safely(
        &mut self,
        image: &Path,
        edits: &[Edit],
        options: &EditOptions,
        checks: &dyn ImageCheck,
        journal: &Journal,
    ) -> Result<EditReport, EditError> {
        if !supports_rating(image) {
            return Err(EditError::Unsupported(format!(
                "metadata is not supported for {}",
                image.display()
            )));
        }
        let read = self
            .read_metadata(image, options.fallback)
            .map_err(EditError::Read)?;
        let mut metadata = read.reconciled.metadata;
        apply_edits(&mut metadata, edits)?;
        let left_alone = entries_left_alone(&metadata, edits);
        let plan =
            write_plan::plan(&metadata, &read.state, options.settings).map_err(EditError::Plan)?;
        let report = self
            .write_safely(image, &plan, checks, journal)
            .map_err(EditError::Write)?;
        let mut notices = match report {
            WriteReport::Unchanged => Vec::new(),
            WriteReport::Written { .. } => plan.notices,
        };
        for property in left_alone {
            let notice = WriteNotice::ConflictKept { property };
            if !notices.contains(&notice) {
                notices.push(notice);
            }
        }
        Ok(EditReport { report, notices })
    }
}

/// The properties of the [`Edit::Entries`] in `edits` that were left alone because of an `Open`
/// conflict (U12d-Q6): those that still have one and are not changed after [`apply_edits`]. In the
/// order of `edits`, each once.
fn entries_left_alone(metadata: &Metadata, edits: &[Edit]) -> Vec<String> {
    let mut left_alone: Vec<String> = Vec::new();
    for edit in edits {
        let Edit::Entries { property, .. } = edit else {
            continue;
        };
        if has_open_conflict(metadata, property)
            && !is_changed(metadata, property)
            && !left_alone.contains(property)
        {
            left_alone.push(property.clone());
        }
    }
    left_alone
}

/// Sets `value` on `property`, or clears it for `None`.
fn apply(metadata: &mut Metadata, property: &str, value: Option<Value>) -> Result<(), EditError> {
    match value {
        Some(value) => metadata.set(property, value),
        None => metadata.clear(property),
    }
    .map_err(EditError::Value)
}

/// True when the Rating must be written: a number that differs from the one read, or `None` while
/// the file has a Rating other than 0 (§8: a missing value counts as 0).
fn rating_changes(current: Option<&Value>, new: Option<&Value>) -> bool {
    match (current, new) {
        (Some(current), Some(new)) => !same_value(RATING, current, new),
        (None, Some(_)) => true,
        (Some(current), None) => !is_zero(current),
        (None, None) => false,
    }
}

/// True when the Label must be written: a text that differs from the one read, or `None` while the
/// file has a Label.
fn label_changes(current: Option<&Value>, new: Option<&Value>) -> bool {
    match (current, new) {
        (Some(Value::Text(current)), Some(Value::Text(new))) => current != new,
        (_, Some(_)) => true,
        (Some(_), None) => true,
        (None, None) => false,
    }
}

/// True for a number whose value is 0 (`0`, `0.0`, `-0`, …).
fn is_zero(value: &Value) -> bool {
    match value {
        Value::Number(text) => text.trim().parse::<f64>().is_ok_and(|number| number == 0.0),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> Value {
        Value::Number(text.to_string())
    }

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    #[test]
    fn the_same_rating_is_no_change() {
        assert!(!rating_changes(Some(&number("3")), Some(&number("3"))));
        assert!(rating_changes(Some(&number("3")), Some(&number("4"))));
        assert!(rating_changes(None, Some(&number("2"))));
        assert!(rating_changes(Some(&number("2.5")), Some(&number("2"))));
    }

    #[test]
    fn no_rating_keeps_a_zero_and_clears_any_other_number() {
        assert!(!rating_changes(None, None));
        assert!(!rating_changes(Some(&number("0")), None));
        assert!(!rating_changes(Some(&number("0.0")), None));
        assert!(rating_changes(Some(&number("3")), None));
        assert!(rating_changes(Some(&number("-1")), None));
        assert!(rating_changes(Some(&number("high")), None));
    }

    #[test]
    fn the_same_label_is_no_change() {
        assert!(!label_changes(Some(&text("Red")), Some(&text("Red"))));
        assert!(label_changes(Some(&text("Red")), Some(&text("Blue"))));
        assert!(label_changes(Some(&text("Red")), Some(&text("red"))));
        assert!(label_changes(None, Some(&text("Red"))));
        assert!(label_changes(Some(&text("Red")), None));
        assert!(!label_changes(None, None));
    }

    const CONTACT: &str = "Iptc4xmpCore:CreatorContactInfo";
    const EMAIL: &str = "Iptc4xmpCore:CiEmailWork";
    const PHONE: &str = "Iptc4xmpCore:CiTelWork";

    fn lang_alt(entries: &[(&str, &str)]) -> Value {
        Value::LangAlt(crate::value::LangAlt::from_entries(
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

    fn set(property: &str, value: Value) -> Edit {
        Edit::Set {
            property: property.to_string(),
            field: None,
            value,
        }
    }

    fn set_field(property: &str, field: &str, value: Value) -> Edit {
        Edit::Set {
            property: property.to_string(),
            field: Some(field.to_string()),
            value,
        }
    }

    fn clear(property: &str) -> Edit {
        Edit::Clear {
            property: property.to_string(),
            field: None,
        }
    }

    fn clear_field(property: &str, field: &str) -> Edit {
        Edit::Clear {
            property: property.to_string(),
            field: Some(field.to_string()),
        }
    }

    /// Metadata as read from a file: the values, nothing changed.
    fn read(values: &[(&str, Value)]) -> Metadata {
        let mut metadata = Metadata::new();
        for (xmp_id, value) in values {
            if let Err(error) =
                metadata.insert_read(xmp_id, value.clone(), crate::value::Origin::XmpEmbedded)
            {
                panic!("{xmp_id}: {error}");
            }
        }
        metadata
    }

    fn edited(metadata: &mut Metadata, edits: &[Edit]) {
        if let Err(error) = apply_edits(metadata, edits) {
            panic!("the edits were not applied: {error}");
        }
    }

    fn changes(metadata: &Metadata) -> Vec<(String, Option<Value>)> {
        metadata
            .changes()
            .map(|(xmp_id, value)| (xmp_id.to_string(), value.cloned()))
            .collect()
    }

    #[test]
    fn a_text_is_set() {
        let mut metadata = read(&[("photoshop:Headline", text("Gammal"))]);
        edited(&mut metadata, &[set("photoshop:Headline", text("Ny"))]);
        assert_eq!(
            changes(&metadata),
            [("photoshop:Headline".to_string(), Some(text("Ny")))]
        );
    }

    #[test]
    fn the_same_text_is_no_change() {
        let mut metadata = read(&[("photoshop:Headline", text("Samma"))]);
        edited(&mut metadata, &[set("photoshop:Headline", text("Samma"))]);
        assert_eq!(changes(&metadata), []);
    }

    #[test]
    fn the_same_text_with_an_open_conflict_is_a_change() {
        let mut metadata = read(&[("photoshop:City", text("Lund"))]);
        metadata.add_conflict(crate::value::Conflict {
            property: "photoshop:City".to_string(),
            values: vec![
                (crate::value::Origin::XmpEmbedded, text("Lund")),
                (crate::value::Origin::Iim, text("Malmö")),
            ],
            rule: ConflictRule::Open,
        });
        edited(&mut metadata, &[set("photoshop:City", text("Lund"))]);
        assert_eq!(
            changes(&metadata),
            [("photoshop:City".to_string(), Some(text("Lund")))]
        );
    }

    #[test]
    fn a_lang_alt_keeps_its_other_languages() {
        let mut metadata = read(&[(
            "dc:title",
            lang_alt(&[("x-default", "Gammal"), ("sv", "Svensk")]),
        )]);
        edited(
            &mut metadata,
            &[set("dc:title", lang_alt(&[("x-default", "Ny")]))],
        );
        assert_eq!(
            changes(&metadata),
            [(
                "dc:title".to_string(),
                Some(lang_alt(&[("x-default", "Ny"), ("sv", "Svensk")]))
            )]
        );
    }

    #[test]
    fn a_copyright_notice_may_be_cleared_by_an_edit() {
        let mut metadata = read(&[("dc:rights", lang_alt(&[("x-default", "© A")]))]);
        edited(&mut metadata, &[clear("dc:rights")]);
        assert_eq!(changes(&metadata), [("dc:rights".to_string(), None)]);
        assert!(metadata.get("dc:rights").is_none());
    }

    #[test]
    fn clearing_an_absent_property_is_no_change() {
        let mut metadata = read(&[("photoshop:Headline", text("Kvar"))]);
        edited(&mut metadata, &[clear("photoshop:City")]);
        assert_eq!(changes(&metadata), []);
    }

    #[test]
    fn a_contact_field_is_set_and_the_other_fields_kept() {
        let mut metadata = read(&[(CONTACT, structure(&[(PHONE, text("123"))]))]);
        edited(&mut metadata, &[set_field(CONTACT, EMAIL, text("a@b.se"))]);
        assert_eq!(
            changes(&metadata),
            [(
                CONTACT.to_string(),
                Some(structure(&[(EMAIL, text("a@b.se")), (PHONE, text("123"))]))
            )]
        );
    }

    #[test]
    fn the_same_contact_field_is_no_change() {
        let mut metadata = read(&[(
            CONTACT,
            structure(&[(EMAIL, text("a@b.se")), (PHONE, text("123"))]),
        )]);
        edited(&mut metadata, &[set_field(CONTACT, EMAIL, text("a@b.se"))]);
        assert_eq!(changes(&metadata), []);
    }

    #[test]
    fn a_contact_field_is_cleared_and_the_last_field_clears_the_property() {
        let mut metadata = read(&[(
            CONTACT,
            structure(&[(EMAIL, text("a@b.se")), (PHONE, text("123"))]),
        )]);
        edited(&mut metadata, &[clear_field(CONTACT, EMAIL)]);
        assert_eq!(
            changes(&metadata),
            [(
                CONTACT.to_string(),
                Some(structure(&[(PHONE, text("123"))]))
            )]
        );

        edited(&mut metadata, &[clear_field(CONTACT, PHONE)]);
        assert_eq!(changes(&metadata), [(CONTACT.to_string(), None)]);
        assert!(metadata.get(CONTACT).is_none());
    }

    #[test]
    fn clearing_an_absent_contact_field_is_no_change() {
        let mut metadata = read(&[(CONTACT, structure(&[(PHONE, text("123"))]))]);
        edited(&mut metadata, &[clear_field(CONTACT, EMAIL)]);
        assert_eq!(changes(&metadata), []);
    }

    #[test]
    fn fields_of_a_list_of_structures_or_of_a_text_are_unsupported() {
        let mut metadata = Metadata::new();
        match apply_edits(
            &mut metadata,
            &[set_field(
                "plus:ImageCreator",
                "plus:ImageCreatorName",
                text("A"),
            )],
        ) {
            Err(EditError::Unsupported(reason)) => assert_eq!(
                reason,
                "plus:ImageCreator/plus:ImageCreatorName cannot be edited field by field"
            ),
            other => panic!("expected Unsupported, got {other:?}"),
        }
        match apply_edits(
            &mut metadata,
            &[clear_field("photoshop:City", "Iptc4xmpCore:CiAdrCity")],
        ) {
            Err(EditError::Unsupported(reason)) => assert_eq!(
                reason,
                "photoshop:City/Iptc4xmpCore:CiAdrCity cannot be edited field by field"
            ),
            other => panic!("expected Unsupported, got {other:?}"),
        }
        assert_eq!(changes(&metadata), []);
    }

    #[test]
    fn an_unknown_property_is_a_value_error() {
        let mut metadata = Metadata::new();
        match apply_edits(&mut metadata, &[set("xmp:Nonsense", text("A"))]) {
            Err(EditError::Value(ValueError::UnknownProperty(property))) => {
                assert_eq!(property, "xmp:Nonsense")
            }
            other => panic!("expected a Value error, got {other:?}"),
        }
        match apply_edits(&mut metadata, &[clear("xmp:Nonsense")]) {
            Err(EditError::Value(ValueError::UnknownProperty(property))) => {
                assert_eq!(property, "xmp:Nonsense")
            }
            other => panic!("expected a Value error, got {other:?}"),
        }
    }

    const SUBJECT: &str = "dc:subject";

    fn list(entries: &[&str]) -> Value {
        Value::List(entries.iter().map(|entry| entry.to_string()).collect())
    }

    fn entries(property: &str, add: &[&str], remove: &[&str]) -> Edit {
        Edit::Entries {
            property: property.to_string(),
            add: add.iter().map(|entry| entry.to_string()).collect(),
            remove: remove.iter().map(|entry| entry.to_string()).collect(),
        }
    }

    #[test]
    fn entries_are_added_at_the_end_and_the_old_ones_kept() {
        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &["vinter", "älg"], &[])]);
        assert_eq!(
            changes(&metadata),
            [(
                SUBJECT.to_string(),
                Some(list(&["sol", "hav", "vinter", "älg"]))
            )]
        );
    }

    #[test]
    fn an_entry_already_there_is_not_added_twice() {
        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        edited(
            &mut metadata,
            &[entries(SUBJECT, &["hav", "hav", "is"], &[])],
        );
        assert_eq!(
            changes(&metadata),
            [(SUBJECT.to_string(), Some(list(&["sol", "hav", "is"])))]
        );
    }

    #[test]
    fn entries_are_removed_and_the_others_kept() {
        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &[], &["sol"])]);
        assert_eq!(
            changes(&metadata),
            [(SUBJECT.to_string(), Some(list(&["hav"])))]
        );

        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &[], &["skog"])]);
        assert_eq!(changes(&metadata), []);
        assert_eq!(
            metadata.get(SUBJECT).map(|stored| stored.value.clone()),
            Some(list(&["sol", "hav"]))
        );
    }

    #[test]
    fn add_and_remove_in_one_edit() {
        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &["is"], &["sol"])]);
        assert_eq!(
            changes(&metadata),
            [(SUBJECT.to_string(), Some(list(&["hav", "is"])))]
        );
    }

    #[test]
    fn the_comparison_is_exact_text() {
        let mut metadata = read(&[(SUBJECT, list(&["sol"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &[], &["Sol"])]);
        assert_eq!(changes(&metadata), []);
        assert_eq!(
            metadata.get(SUBJECT).map(|stored| stored.value.clone()),
            Some(list(&["sol"]))
        );

        edited(&mut metadata, &[entries(SUBJECT, &["Sol"], &[])]);
        assert_eq!(
            changes(&metadata),
            [(SUBJECT.to_string(), Some(list(&["sol", "Sol"])))]
        );
    }

    #[test]
    fn removing_every_entry_clears_the_property() {
        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &[], &["hav", "sol"])]);
        assert_eq!(changes(&metadata), [(SUBJECT.to_string(), None)]);
        assert!(metadata.get(SUBJECT).is_none());
    }

    #[test]
    fn adding_to_an_absent_property_sets_it() {
        let mut metadata = read(&[("photoshop:Headline", text("Kvar"))]);
        edited(&mut metadata, &[entries(SUBJECT, &["is"], &[])]);
        assert_eq!(
            changes(&metadata),
            [(SUBJECT.to_string(), Some(list(&["is"])))]
        );

        // Nothing to add to an absent property is no change, never a clear.
        let mut metadata = read(&[("photoshop:Headline", text("Kvar"))]);
        edited(&mut metadata, &[entries(SUBJECT, &["", "  "], &["sol"])]);
        assert_eq!(changes(&metadata), []);
    }

    /// `dc:subject` `["sol", "hav"]` with an `Open` conflict against the IIM entries `["sol"]`.
    fn subject_with_an_open_conflict() -> Metadata {
        let mut metadata = read(&[(SUBJECT, list(&["sol", "hav"]))]);
        metadata.add_conflict(crate::value::Conflict {
            property: SUBJECT.to_string(),
            values: vec![
                (crate::value::Origin::XmpEmbedded, list(&["sol", "hav"])),
                (crate::value::Origin::Iim, list(&["sol"])),
            ],
            rule: ConflictRule::Open,
        });
        metadata
    }

    #[test]
    fn no_change_keeps_an_open_conflict() {
        let mut metadata = subject_with_an_open_conflict();
        edited(&mut metadata, &[entries(SUBJECT, &["hav"], &[])]);
        assert_eq!(changes(&metadata), []);
        assert!(has_open_conflict(&metadata, SUBJECT));
    }

    #[test]
    fn only_blank_entries_left_clears_the_property() {
        let mut metadata = read(&[(SUBJECT, list(&["", "sol"]))]);
        edited(&mut metadata, &[entries(SUBJECT, &[], &["sol"])]);
        assert_eq!(changes(&metadata), [(SUBJECT.to_string(), None)]);
        assert!(metadata.get(SUBJECT).is_none());
    }

    #[test]
    fn an_open_conflict_is_left_alone() {
        let mut metadata = subject_with_an_open_conflict();
        edited(&mut metadata, &[entries(SUBJECT, &["vinter"], &["sol"])]);
        assert_eq!(changes(&metadata), []);
        assert_eq!(
            metadata.get(SUBJECT).map(|stored| stored.value.clone()),
            Some(list(&["sol", "hav"]))
        );
        assert!(has_open_conflict(&metadata, SUBJECT));
    }

    #[test]
    fn entries_after_a_decided_conflict_are_applied() {
        let mut metadata = subject_with_an_open_conflict();
        edited(
            &mut metadata,
            &[
                set(SUBJECT, list(&["sol"])),
                entries(SUBJECT, &["vinter"], &[]),
            ],
        );
        assert_eq!(
            changes(&metadata),
            [(SUBJECT.to_string(), Some(list(&["sol", "vinter"])))]
        );
    }

    #[test]
    fn entries_on_a_text_or_a_structure_list_are_unsupported() {
        let mut metadata = Metadata::new();
        for property in ["photoshop:City", "Iptc4xmpExt:LocationCreated"] {
            match apply_edits(&mut metadata, &[entries(property, &["A"], &[])]) {
                Err(EditError::Unsupported(reason)) => {
                    assert_eq!(reason, format!("{property} is not a list of entries"))
                }
                other => panic!("{property}: expected Unsupported, got {other:?}"),
            }
        }
        match apply_edits(&mut metadata, &[entries("xmp:Nonsense", &["A"], &[])]) {
            Err(EditError::Value(ValueError::UnknownProperty(property))) => {
                assert_eq!(property, "xmp:Nonsense")
            }
            other => panic!("expected a Value error, got {other:?}"),
        }
        assert_eq!(changes(&metadata), []);
    }

    #[test]
    fn errors_are_shown_in_english() {
        let unsupported = EditError::Unsupported("rating 6 is outside -1..5".to_string());
        assert_eq!(unsupported.to_string(), "rating 6 is outside -1..5");
        let value = EditError::Value(ValueError::Empty("xmp:Label".to_string()));
        assert_eq!(
            value.to_string(),
            "the value was not accepted: xmp:Label: the value is empty (clear the property instead)"
        );
        let plan = EditError::Plan(PlanError::NotEditable("photoshop:Urgency".to_string()));
        assert_eq!(
            plan.to_string(),
            "the write could not be planned: photoshop:Urgency is a deprecated IIM property and \
             cannot be edited"
        );
        let template = EditError::Template(TemplateError::CannotClear("dc:rights".to_string()));
        assert_eq!(
            template.to_string(),
            "the template could not be applied: dc:rights is a copyright-management property and \
             cannot be cleared by a template"
        );
    }
}
