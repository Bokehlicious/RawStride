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
//! The old `write_rating`, `write_iptc` and `write_tags` are unchanged; the Tauri app still uses
//! them (owner decision D7).
//!
//! [`FileState`]: crate::write_plan::FileState

use std::fmt;
use std::path::Path;

use crate::iim::Fallback;
use crate::journal::Journal;
use crate::reconcile::same_value;
use crate::safe_write::{ImageCheck, SafeWriteError, WriteReport};
use crate::template::{self, Template, TemplateError};
use crate::value::{Metadata, Value, ValueError};
use crate::write_plan::{self, PlanError, WriteSettings};
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
