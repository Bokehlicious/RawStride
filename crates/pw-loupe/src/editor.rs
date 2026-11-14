//! The editor's form (ADR 0007 I10k, owner decision D12): the fields of one image, the choices of
//! its unresolved conflicts, the edits they give, the warnings and the texts of the write notices.
//! Pure functions, no egui; T105 draws the window.
//!
//! - **Fields** (D12): 22 fields in a fixed order, [`EditorField::ALL`]. Each is a property of the
//!   table, or (Email, Phone, Web URL) a field of `Iptc4xmpCore:CreatorContactInfo`. The value
//!   kind (text, Lang Alt, list, date) comes from the property table.
//! - **From the read** ([`EditorForm::from_read`]): one text per field. A Lang Alt shows its shown
//!   text; without `x-default` it gets a note with its language, because saving adds `x-default`
//!   (§8, Q7).
//! - **Conflicts** (§5.1, Q1): every `Open` conflict of the file, also on properties that are not
//!   fields, can be decided by choosing one of its values ([`EditorForm::choose`]).
//! - **Edits** ([`EditorForm::edits`]): a changed field is a `Set` of the field's kind, or a
//!   `Clear` when emptied (§8: every representation). A chosen conflict value is a `Set` of that
//!   value, even when the text did not change (the write plan then writes both forms). A field
//!   is changed when its text differs from its starting text: the text the read gave, or, after a
//!   choice, the chosen value's text. Dates are checked here (§7: truncated dates are fine; no
//!   zone is invented).
//! - **Warnings** (§11): the edits applied to a copy of the read metadata, then validated.
//!
//! Text rule (owner, 2026-11-12): every text the user sees from the editor is made here or in
//! `info_panel.rs`, each a whole sentence from one template with its values filled in; property
//! names come only from [`property_label`].

use pw_metadata::MIN_EXIFTOOL_VERSION;
use pw_metadata::date::IptcDate;
use pw_metadata::edit::{Edit, apply_edits};
use pw_metadata::properties::{self, Occurrence, ValueKind};
use pw_metadata::read::MetadataRead;
use pw_metadata::reconcile::FileKind;
use pw_metadata::validate::{Warning, validate};
use pw_metadata::value::{Conflict, ConflictRule, LangAlt, Value};
use pw_metadata::write_plan::WriteNotice;

use crate::info_panel::{
    ConflictValue, Language, origin_text, property_label, property_tooltip, value_text,
};

/// The language of the default entry of a Lang Alt value.
const X_DEFAULT: &str = "x-default";
/// The structure of the contact fields.
const CONTACT_INFO: &str = "Iptc4xmpCore:CreatorContactInfo";

/// A field of the editor (D12), in the order of the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorField {
    Title,
    Headline,
    Description,
    AltText,
    Keywords,
    Creator,
    CreatorJobtitle,
    Email,
    Phone,
    Web,
    Copyright,
    Credit,
    Source,
    UsageTerms,
    DateCreated,
    Sublocation,
    City,
    State,
    Country,
    CountryCode,
    Instructions,
    JobId,
}

/// The kind of value a field holds, from the property table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Text,
    LangAlt,
    List,
    Date,
}

impl EditorField {
    /// Every field in the order of D12.
    pub const ALL: [EditorField; 22] = [
        EditorField::Title,
        EditorField::Headline,
        EditorField::Description,
        EditorField::AltText,
        EditorField::Keywords,
        EditorField::Creator,
        EditorField::CreatorJobtitle,
        EditorField::Email,
        EditorField::Phone,
        EditorField::Web,
        EditorField::Copyright,
        EditorField::Credit,
        EditorField::Source,
        EditorField::UsageTerms,
        EditorField::DateCreated,
        EditorField::Sublocation,
        EditorField::City,
        EditorField::State,
        EditorField::Country,
        EditorField::CountryCode,
        EditorField::Instructions,
        EditorField::JobId,
    ];

    /// The XMP id of the property the field edits (the structure for a contact field).
    pub fn property(self) -> &'static str {
        match self {
            EditorField::Title => "dc:title",
            EditorField::Headline => "photoshop:Headline",
            EditorField::Description => "dc:description",
            EditorField::AltText => "Iptc4xmpCore:AltTextAccessibility",
            EditorField::Keywords => "dc:subject",
            EditorField::Creator => "dc:creator",
            EditorField::CreatorJobtitle => "photoshop:AuthorsPosition",
            EditorField::Email | EditorField::Phone | EditorField::Web => CONTACT_INFO,
            EditorField::Copyright => "dc:rights",
            EditorField::Credit => "photoshop:Credit",
            EditorField::Source => "photoshop:Source",
            EditorField::UsageTerms => "xmpRights:UsageTerms",
            EditorField::DateCreated => "photoshop:DateCreated",
            EditorField::Sublocation => "Iptc4xmpCore:Location",
            EditorField::City => "photoshop:City",
            EditorField::State => "photoshop:State",
            EditorField::Country => "photoshop:Country",
            EditorField::CountryCode => "Iptc4xmpCore:CountryCode",
            EditorField::Instructions => "photoshop:Instructions",
            EditorField::JobId => "photoshop:TransmissionReference",
        }
    }

    /// The XMP id of the structure field a contact field edits; `None` for the other fields.
    pub fn struct_field(self) -> Option<&'static str> {
        match self {
            EditorField::Email => Some("Iptc4xmpCore:CiEmailWork"),
            EditorField::Phone => Some("Iptc4xmpCore:CiTelWork"),
            EditorField::Web => Some("Iptc4xmpCore:CiUrlWork"),
            _ => None,
        }
    }

    /// The field's name ([`property_label`] in English).
    pub fn label(self) -> String {
        property_label(self.struct_field().unwrap_or(self.property()), Language::En)
    }

    /// The tooltip of the field's name: the full IPTC name when the name leaves part of it out
    /// (the legacy properties and the contact fields; [`property_tooltip`], T117).
    pub fn tooltip(self) -> Option<String> {
        property_tooltip(self.struct_field().unwrap_or(self.property()))
    }

    /// True for the fields shown as a box of several lines. Keywords and Creator are one line
    /// with their entries separated by commas (T108).
    pub fn multiline(self) -> bool {
        matches!(
            self,
            EditorField::Description
                | EditorField::AltText
                | EditorField::Instructions
                | EditorField::UsageTerms
        )
    }

    /// The kind of the field's value, from the property table (a text when it is not found).
    fn kind(self) -> FieldKind {
        let def = match self.struct_field() {
            None => properties::property(self.property()).map(|def| (def.kind, def.occurrence)),
            Some(field) => properties::property(self.property())
                .and_then(|def| match def.kind {
                    ValueKind::Struct(name) => properties::struct_def(name),
                    _ => None,
                })
                .and_then(|def| def.fields.iter().find(|def| def.xmp_id == field))
                .map(|def| (def.kind, def.occurrence)),
        };
        match def {
            Some((_, Occurrence::Multi)) => FieldKind::List,
            Some((ValueKind::LangAlt, _)) => FieldKind::LangAlt,
            Some((ValueKind::Date, _)) => FieldKind::Date,
            _ => FieldKind::Text,
        }
    }

    /// The text of this field for `value`, a value of the field's property.
    fn text_of(self, value: &Value) -> String {
        match self.struct_field() {
            None => form_text(value),
            Some(field) => match value {
                Value::Struct(fields) => fields
                    .fields
                    .iter()
                    .find(|(id, _)| id == field)
                    .map(|(_, value)| form_text(value))
                    .unwrap_or_default(),
                _ => String::new(),
            },
        }
    }
}

/// The separator of the entries of a list field on its one line (T108).
const LIST_SEPARATOR: &str = ", ";

/// The text of a value in the form: a text and a number as they are; a Lang Alt its shown text; a
/// list its entries joined with `", "` on one line (T108); a date in XMP form; anything else as
/// the info panel shows it.
fn form_text(value: &Value) -> String {
    match value {
        Value::Text(text) | Value::Number(text) => text.clone(),
        Value::LangAlt(lang_alt) => lang_alt
            .display()
            .map(|(text, _)| text.to_string())
            .unwrap_or_default(),
        Value::List(entries) => entries.join(LIST_SEPARATOR),
        Value::Date(date) => date.to_xmp(),
        Value::Struct(_) | Value::StructList(_) => value_text(value),
    }
}

/// The language of a Lang Alt shown without `x-default`.
fn lang_alt_language(value: &Value) -> Option<String> {
    match value {
        Value::LangAlt(lang_alt) => lang_alt
            .display()
            .and_then(|(_, language)| language.map(str::to_string)),
        _ => None,
    }
}

/// The note of a Lang Alt field whose text is shown from `language`, not from `x-default`.
fn lang_alt_note(language: &str) -> String {
    format!("({language})")
}

/// The error of a date that cannot be read.
fn date_error_text() -> String {
    "Write the date as YYYY, YYYY-MM, YYYY-MM-DD or YYYY-MM-DDThh:mm[:ss], with a time zone only \
     when it is known"
        .to_string()
}

/// One field of the form.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FieldState {
    field: EditorField,
    /// The text as the user has it.
    text: String,
    /// The text `from_read` gave.
    original: String,
    /// The language of a Lang Alt shown without `x-default`.
    language: Option<String>,
}

/// An unresolved conflict of the file, as the editor shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenConflict {
    /// The property's name.
    pub label: String,
    /// Each value with its origin.
    pub values: Vec<ConflictValue>,
    /// The index of the chosen value, if one is chosen.
    pub chosen: Option<usize>,
}

/// The editor's form for one image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorForm {
    fields: Vec<FieldState>,
    /// The `Open` conflicts of the file, in the order of the read.
    conflicts: Vec<Conflict>,
    /// The chosen value of each conflict.
    chosen: Vec<Option<usize>>,
}

impl EditorForm {
    /// The form of `read`: one text per field, and the file's `Open` conflicts, none chosen.
    pub fn from_read(read: &MetadataRead) -> EditorForm {
        let metadata = &read.reconciled.metadata;
        let fields = EditorField::ALL
            .iter()
            .map(|&field| {
                let value = metadata.get(field.property()).map(|stored| &stored.value);
                let text = value.map(|value| field.text_of(value)).unwrap_or_default();
                let language = match field.struct_field() {
                    None => value.and_then(lang_alt_language),
                    Some(_) => None,
                };
                FieldState {
                    field,
                    text: text.clone(),
                    original: text,
                    language,
                }
            })
            .collect();
        let conflicts: Vec<Conflict> = metadata
            .conflicts()
            .iter()
            .filter(|conflict| conflict.rule == ConflictRule::Open)
            .cloned()
            .collect();
        let chosen = vec![None; conflicts.len()];
        EditorForm {
            fields,
            conflicts,
            chosen,
        }
    }

    fn state(&self, field: EditorField) -> Option<&FieldState> {
        self.fields.iter().find(|state| state.field == field)
    }

    /// The text of `field` as the user has it.
    pub fn text(&self, field: EditorField) -> &str {
        self.state(field).map_or("", |state| state.text.as_str())
    }

    /// The text of `field`, to be edited.
    pub fn text_mut(&mut self, field: EditorField) -> Option<&mut String> {
        self.fields
            .iter_mut()
            .find(|state| state.field == field)
            .map(|state| &mut state.text)
    }

    /// The note of a Lang Alt field that has no `x-default`: its language, `(<language>)`; saving
    /// adds `x-default` (Q7).
    pub fn note(&self, field: EditorField) -> Option<String> {
        self.state(field)
            .and_then(|state| state.language.as_deref())
            .map(lang_alt_note)
    }

    /// Every `Open` conflict of the file (also on properties that are not fields): its name, each
    /// value with its origin, and the chosen value.
    pub fn open_conflicts(&self) -> Vec<OpenConflict> {
        self.conflicts
            .iter()
            .zip(&self.chosen)
            .map(|(conflict, chosen)| OpenConflict {
                label: property_label(&conflict.property, Language::En),
                values: conflict
                    .values
                    .iter()
                    .map(|(origin, value)| ConflictValue {
                        value: value_text(value),
                        origin: origin_text(*origin).to_string(),
                    })
                    .collect(),
                chosen: *chosen,
            })
            .collect()
    }

    /// Chooses value `value_index` of conflict `conflict_index`; the fields of its property get
    /// the value's text. False (and nothing changes) when either index is out of range.
    pub fn choose(&mut self, conflict_index: usize, value_index: usize) -> bool {
        let Some(conflict) = self.conflicts.get(conflict_index) else {
            return false;
        };
        let Some((_, value)) = conflict.values.get(value_index) else {
            return false;
        };
        for state in &mut self.fields {
            if state.field.property() == conflict.property {
                state.text = state.field.text_of(value);
            }
        }
        if let Some(chosen) = self.chosen.get_mut(conflict_index) {
            *chosen = Some(value_index);
        }
        true
    }

    /// The chosen value of a conflict on `property`, if any.
    fn chosen_value(&self, property: &str) -> Option<&Value> {
        self.conflicts
            .iter()
            .zip(&self.chosen)
            .filter(|(conflict, _)| conflict.property == property)
            .find_map(|(conflict, chosen)| {
                chosen
                    .and_then(|index| conflict.values.get(index))
                    .map(|(_, value)| value)
            })
    }

    /// The text a field is compared with: the chosen conflict value's text, else the text the read
    /// gave.
    fn start_text(&self, state: &FieldState) -> String {
        match self.chosen_value(state.field.property()) {
            Some(value) => state.field.text_of(value),
            None => state.original.clone(),
        }
    }

    /// The edits of the form and the errors of its fields (see [`EditorForm::edits`]).
    fn collect(&self) -> (Vec<Edit>, Vec<(EditorField, String)>) {
        let mut edits = Vec::new();
        let mut errors = Vec::new();
        // The chosen conflict values first, so a changed contact field is applied on top of a
        // chosen structure. A whole field whose text was changed after the choice is set from
        // its text instead.
        for (conflict, chosen) in self.conflicts.iter().zip(&self.chosen) {
            let Some((_, value)) = chosen.and_then(|index| conflict.values.get(index)) else {
                continue;
            };
            let changed_after_choice = self.fields.iter().any(|state| {
                state.field.property() == conflict.property
                    && state.field.struct_field().is_none()
                    && state.text != state.field.text_of(value)
            });
            if !changed_after_choice {
                edits.push(Edit::Set {
                    property: conflict.property.clone(),
                    field: None,
                    value: value.clone(),
                });
            }
        }
        for state in &self.fields {
            if state.text == self.start_text(state) {
                continue;
            }
            match field_edit(state.field, &state.text) {
                Ok(edit) => edits.push(edit),
                Err(error) => errors.push((state.field, error)),
            }
        }
        (edits, errors)
    }

    /// The edits of the form: a `Set` or `Clear` for each changed field (in field order, after
    /// the chosen conflict values), or every field error together.
    pub fn edits(&self) -> Result<Vec<Edit>, Vec<(EditorField, String)>> {
        let (edits, errors) = self.collect();
        if errors.is_empty() {
            Ok(edits)
        } else {
            Err(errors)
        }
    }

    /// The warnings (§11) of the read metadata with the form's edits applied (a field with an
    /// error is left out). IIM lengths count when the file has IIM, or `create_iim` and the file
    /// is not a manufacturer RAW.
    pub fn warnings(&self, read: &MetadataRead, create_iim: bool) -> Vec<String> {
        let (edits, _) = self.collect();
        let mut metadata = read.reconciled.metadata.clone();
        for edit in &edits {
            // An edit that is not accepted is reported when saving; the others are still checked.
            if apply_edits(&mut metadata, std::slice::from_ref(edit)).is_err() {
                continue;
            }
        }
        let iim_written =
            read.state.iim_present || (create_iim && read.state.kind != FileKind::Raw);
        validate(&metadata, iim_written)
            .iter()
            .map(warning_text)
            .collect()
    }
}

/// The entries of a list field's text: split at `,`, each part trimmed, empty parts dropped
/// (T108). An entry cannot hold a comma.
fn list_entries(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// The edit of a changed field with the text `text`, or the field's error.
fn field_edit(field: EditorField, text: &str) -> Result<Edit, String> {
    let property = field.property().to_string();
    let struct_field = field.struct_field().map(str::to_string);
    let clear = || Edit::Clear {
        property: property.clone(),
        field: struct_field.clone(),
    };
    if text.trim().is_empty() {
        return Ok(clear());
    }
    let value = match field.kind() {
        FieldKind::Text => Value::Text(text.to_string()),
        FieldKind::LangAlt => Value::LangAlt(LangAlt::from_entries(vec![(
            X_DEFAULT.to_string(),
            text.to_string(),
        )])),
        FieldKind::List => {
            let entries = list_entries(text);
            // Only spaces and commas: the list is emptied.
            if entries.is_empty() {
                return Ok(clear());
            }
            Value::List(entries)
        }
        FieldKind::Date => {
            Value::Date(IptcDate::parse_xmp(text.trim()).map_err(|_| date_error_text())?)
        }
    };
    Ok(Edit::Set {
        property,
        field: struct_field,
        value,
    })
}

/// The text of a warning (§11).
fn warning_text(warning: &Warning) -> String {
    match warning {
        Warning::TooLongForIim {
            property,
            max_bytes,
        } => {
            let field = property_label(property, Language::En);
            if *max_bytes == 1 {
                format!("{field} is truncated in IIM (at most 1 byte); XMP gets the whole text")
            } else {
                format!(
                    "{field} is truncated in IIM (at most {max_bytes} bytes); XMP gets the whole \
                     text"
                )
            }
        }
        Warning::AltTextTooLong { characters } => format!(
            "{field} is longer than 250 characters ({characters})",
            field = property_label("Iptc4xmpCore:AltTextAccessibility", Language::En)
        ),
        Warning::CountryCodeForm { code } => format!(
            "{field} {code} should be two or three letters (ISO 3166-1)",
            field = property_label("Iptc4xmpCore:CountryCode", Language::En)
        ),
        Warning::JobtitleWithoutCreator => format!(
            "{jobtitle} without {creator}",
            jobtitle = property_label("photoshop:AuthorsPosition", Language::En),
            creator = property_label("dc:creator", Language::En)
        ),
        Warning::DigitalSourceTypeUnknown { uri } => format!(
            "{field} {uri} is not in IPTC's list",
            field = property_label("Iptc4xmpExt:DigitalSourceType", Language::En)
        ),
        Warning::RegistryEntryIncomplete { index } => format!(
            "Registry entry {n} is missing an item ID or an organization ID",
            n = index + 1
        ),
        Warning::RatingOutOfRange { value } => {
            format!("Rating {value} is outside −1 to 5")
        }
    }
}

/// The text of a notice of an editor write (T105).
pub fn notice_text(notice: &WriteNotice) -> String {
    match notice {
        WriteNotice::Truncated { property } => format!(
            "{field} was truncated in IIM; XMP has the whole text",
            field = property_label(property, Language::En)
        ),
        WriteNotice::IimConvertedToUtf8 => "The IIM texts were rewritten as UTF-8".to_string(),
        WriteNotice::ConflictKept { property } => format!(
            "{field}: the conflict is unresolved and was not written",
            field = property_label(property, Language::En)
        ),
        WriteNotice::DigestNotWritten => {
            "No new IIM checksum was written, because a conflict is unresolved".to_string()
        }
    }
}

/// A fixed text of the metadata window (T105, T107), as [`PanelText`](crate::PanelText) for the
/// texts it shares with the former info panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorText {
    /// The window's title; `file` is the image's file name.
    WindowTitle { file: String },
    /// The heading of the unresolved conflicts.
    ConflictsHeading,
    /// One radio button of a conflict: a value and its origin.
    ConflictChoice { value: String, origin: String },
    /// The heading of the warnings.
    WarningsHeading,
    /// The heading of the read-only list of the properties that are not fields.
    OtherValuesHeading,
    /// The button that writes the edits and closes the window.
    Ok,
    /// Shown while the write runs.
    Saving,
    /// Shown after a write that succeeded.
    Saved,
    /// A write that failed; `reason` as `write_error_text` gives it for ratings.
    SaveFailed { file: String, reason: String },
    /// Asked when the window is to leave an image with unsaved edits: a move, Escape or the
    /// window's "Cancel" (ADR 0017 §2.3.1, U12-Q4, T112).
    SaveQuestion,
    /// The answer to `SaveQuestion` that writes the edits, then moves or closes.
    Save,
    /// The answer to `SaveQuestion` that drops the edits, then moves or closes.
    Discard,
    /// The button that closes the window without writing, and the answer to `SaveQuestion` that
    /// goes back to the window with the edits kept.
    Cancel,
    /// The button that moves the window to the previous image with metadata support (T112).
    Previous,
    /// The button that moves the window to the next image with metadata support (T112).
    Next,
    /// The button that writes the edits and moves to the next image (T112).
    SaveAndNext,
    /// The title of the message at program exit listing the rating and editor writes that were
    /// not saved (owner decision 2026-11-12: one message for both).
    UnsavedAtExitTitle,
    /// The last line of that message when it lists more files than it shows (moved from
    /// `metadata.rs`, T116).
    MoreUnsaved { more: usize },
    /// The status-line notice while an image's metadata is read (moved from `metadata.rs`, T116).
    ReadingMetadata,
    /// The status-line notice of a metadata read that failed (moved from `metadata.rs` and
    /// `main.rs`, T116).
    MetadataError { error: String },
}

/// The text of a fixed text of the editor window, each a whole sentence from one template.
pub fn editor_text(text: &EditorText) -> String {
    match text {
        EditorText::WindowTitle { file } => format!("Metadata – {file}"),
        EditorText::ConflictsHeading => "Unresolved Conflicts".to_string(),
        EditorText::ConflictChoice { value, origin } => format!("{value} ({origin})"),
        EditorText::WarningsHeading => "Warnings".to_string(),
        EditorText::OtherValuesHeading => "Other Metadata".to_string(),
        EditorText::Ok => "OK".to_string(),
        EditorText::Saving => "Saving …".to_string(),
        EditorText::Saved => "Saved.".to_string(),
        EditorText::SaveFailed { file, reason } => {
            format!("Could not save metadata for {file}: {reason}")
        }
        EditorText::SaveQuestion => "Save changes?".to_string(),
        EditorText::Save => "Save".to_string(),
        EditorText::Discard => "Discard".to_string(),
        EditorText::Cancel => "Cancel".to_string(),
        EditorText::Previous => "Previous".to_string(),
        EditorText::Next => "Next".to_string(),
        EditorText::SaveAndNext => "Save and Next".to_string(),
        EditorText::UnsavedAtExitTitle => "Unsaved Changes".to_string(),
        EditorText::MoreUnsaved { more } => format!("… and {more} more"),
        EditorText::ReadingMetadata => "Reading metadata…".to_string(),
        EditorText::MetadataError { error } => format!("Metadata error: {error}"),
    }
}

/// The reason in the message for a failed metadata write: a rating (`Could not save the rating for
/// <file>: `), the metadata window (`Could not save metadata for <file>: `) or the import's
/// template. Moved from `metadata.rs` and `import_worker.rs` (T116).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteReason {
    /// The file is read-only (T49).
    ReadOnly,
    /// Another program holds the file open (T108).
    Locked,
    /// ExifTool refused the write (T49).
    NotWritten,
    /// The write journal could not be opened at the start, so nothing is written (T91).
    NoJournal,
    /// The metadata worker refuses writes while ExifTool is too old or its version could not be
    /// read (ADR 0007 §11, D9, T101).
    ExifToolTooOld,
    /// The safe write failed and the file is unchanged (ADR 0007 §9.5).
    NotSafeIntact,
    /// The safe write failed and the original is now the backup file `name`.
    NotSafeAtBackup { name: String },
    /// The file was written but the final check failed; the earlier version is kept as a backup.
    NotSafeReplaced,
    /// The metadata worker's queue is full.
    QueueFull,
    /// The metadata worker has stopped.
    WorkerStopped,
}

/// The text of a write reason, each a whole sentence part from one template.
pub fn write_reason_text(reason: &WriteReason) -> String {
    match reason {
        WriteReason::ReadOnly => "the file is read-only".to_string(),
        WriteReason::Locked => "the file is open in another program".to_string(),
        WriteReason::NotWritten => "the file could not be written".to_string(),
        WriteReason::NoJournal => "the write log could not be opened".to_string(),
        WriteReason::ExifToolTooOld => {
            format!("ExifTool is too old (at least {MIN_EXIFTOOL_VERSION} is needed)")
        }
        WriteReason::NotSafeIntact => {
            "the metadata could not be written safely; the file is unchanged".to_string()
        }
        WriteReason::NotSafeAtBackup { name } => {
            format!("the metadata could not be written safely; the original is now {name}")
        }
        WriteReason::NotSafeReplaced => "the file was written but the final check failed; the \
                                         earlier version is kept as a backup"
            .to_string(),
        WriteReason::QueueFull => "the metadata queue is full".to_string(),
        WriteReason::WorkerStopped => "metadata writing has stopped".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pw_metadata::date::{Time, Zone};
    use pw_metadata::iim::CodedCharset;
    use pw_metadata::reconcile::{IimDigest, Reconciled};
    use pw_metadata::value::{Metadata, Origin, StructValue};
    use pw_metadata::write_plan::{ExifPresent, FileState};

    fn state(kind: FileKind, iim_present: bool) -> FileState {
        FileState {
            kind,
            iim_present,
            iim_charset: CodedCharset::Absent,
            iim_values: Vec::new(),
            digest: IimDigest::Missing,
            exif_present: ExifPresent::default(),
        }
    }

    fn read_of(metadata: Metadata) -> MetadataRead {
        MetadataRead {
            reconciled: Reconciled {
                metadata,
                notices: Vec::new(),
                suggested_date_created: None,
            },
            file_notices: Vec::new(),
            state: state(FileKind::Jpeg, false),
        }
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn lang_alt(entries: &[(&str, &str)]) -> Value {
        Value::LangAlt(LangAlt::from_entries(
            entries
                .iter()
                .map(|(l, t)| (l.to_string(), t.to_string()))
                .collect(),
        ))
    }

    fn list(entries: &[&str]) -> Value {
        Value::List(entries.iter().map(|s| s.to_string()).collect())
    }

    fn contact(fields: &[(&str, &str)]) -> Value {
        Value::Struct(StructValue {
            fields: fields
                .iter()
                .map(|(id, t)| (id.to_string(), text(t)))
                .collect(),
        })
    }

    fn insert(metadata: &mut Metadata, id: &str, value: Value) {
        metadata
            .insert_read(id, value, Origin::XmpEmbedded)
            .unwrap();
    }

    fn set(property: &str, value: Value) -> Edit {
        Edit::Set {
            property: property.to_string(),
            field: None,
            value,
        }
    }

    fn form_with(metadata: Metadata) -> EditorForm {
        EditorForm::from_read(&read_of(metadata))
    }

    fn type_into(form: &mut EditorForm, field: EditorField, text: &str) {
        *form.text_mut(field).unwrap() = text.to_string();
    }

    // --- fields ---

    #[test]
    fn fields_in_d12_order_with_labels() {
        let labels: Vec<String> = EditorField::ALL.iter().map(|f| f.label()).collect();
        assert_eq!(
            labels,
            [
                "Title",
                "Headline",
                "Description",
                "Alt Text (Accessibility)",
                "Keywords",
                "Creator",
                "Creator's jobtitle",
                "Email",
                "Phone",
                "Web URL",
                "Copyright Notice",
                "Credit Line",
                "Source (Supply Chain)",
                "Rights Usage Terms",
                "Date Created",
                "Sublocation",
                "City",
                "Province or State",
                "Country",
                "Country Code",
                "Instructions",
                "Job Id",
            ]
        );
    }

    #[test]
    fn multiline_fields() {
        let multiline: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|f| f.multiline())
            .collect();
        assert_eq!(
            multiline,
            [
                EditorField::Description,
                EditorField::AltText,
                EditorField::UsageTerms,
                EditorField::Instructions,
            ]
        );
    }

    #[test]
    fn keywords_and_creator_are_one_line() {
        assert!(!EditorField::Keywords.multiline());
        assert!(!EditorField::Creator.multiline());
    }

    #[test]
    fn field_kinds_come_from_the_property_table() {
        for field in EditorField::ALL {
            let expected = match field {
                EditorField::Title
                | EditorField::Description
                | EditorField::AltText
                | EditorField::Copyright
                | EditorField::UsageTerms => FieldKind::LangAlt,
                EditorField::Keywords | EditorField::Creator => FieldKind::List,
                EditorField::DateCreated => FieldKind::Date,
                _ => FieldKind::Text,
            };
            assert_eq!(field.kind(), expected, "{field:?}");
        }
    }

    // --- from_read ---

    #[test]
    fn from_read_of_every_field_kind() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Rubriken"));
        insert(
            &mut metadata,
            "dc:title",
            lang_alt(&[("en", "Hello"), ("x-default", "Hej")]),
        );
        insert(&mut metadata, "dc:subject", list(&["sol", "hav"]));
        insert(&mut metadata, "dc:creator", list(&["Anna", "Per"]));
        insert(
            &mut metadata,
            "photoshop:DateCreated",
            Value::Date(IptcDate {
                year: 2026,
                month: Some(11),
                day: Some(12),
                time: Some(Time {
                    hour: 10,
                    minute: 15,
                    second: None,
                    fraction: None,
                    zone: Some(Zone::Offset(60)),
                }),
            }),
        );
        insert(
            &mut metadata,
            CONTACT_INFO,
            contact(&[
                ("Iptc4xmpCore:CiEmailWork", "foto@example.com"),
                ("Iptc4xmpCore:CiTelWork", "+46 8 123"),
                ("Iptc4xmpCore:CiUrlWork", "https://example.com"),
            ]),
        );
        let form = form_with(metadata);
        assert_eq!(form.text(EditorField::Headline), "Rubriken");
        assert_eq!(form.text(EditorField::Title), "Hej");
        assert_eq!(form.note(EditorField::Title), None);
        assert_eq!(form.text(EditorField::Keywords), "sol, hav");
        assert_eq!(form.text(EditorField::Creator), "Anna, Per");
        assert_eq!(
            form.text(EditorField::DateCreated),
            "2026-11-12T10:15+01:00"
        );
        assert_eq!(form.text(EditorField::Email), "foto@example.com");
        assert_eq!(form.text(EditorField::Phone), "+46 8 123");
        assert_eq!(form.text(EditorField::Web), "https://example.com");
        assert_eq!(form.text(EditorField::City), "");
        assert!(form.open_conflicts().is_empty());
    }

    #[test]
    fn from_read_of_lang_alt_without_x_default_has_a_note() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "dc:description",
            lang_alt(&[("en-US", "Hello"), ("de", "Hallo")]),
        );
        let form = form_with(metadata);
        assert_eq!(form.text(EditorField::Description), "Hello");
        assert_eq!(
            form.note(EditorField::Description),
            Some("(en-US)".to_string())
        );
        // Shown untouched, it gives no edit.
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    // --- edits ---

    #[test]
    fn untouched_form_has_no_edits() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Umeå"));
        insert(&mut metadata, "dc:subject", list(&["sol"]));
        assert_eq!(form_with(metadata).edits(), Ok(Vec::new()));
        assert_eq!(form_with(Metadata::new()).edits(), Ok(Vec::new()));
    }

    #[test]
    fn changed_text_and_lang_alt() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Umeå"));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::City, "Luleå");
        type_into(&mut form, EditorField::Title, "Älven");
        assert_eq!(
            form.edits(),
            Ok(vec![
                set("dc:title", lang_alt(&[("x-default", "Älven")])),
                set("photoshop:City", text("Luleå")),
            ])
        );
    }

    #[test]
    fn emptied_field_is_a_clear() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "dc:description",
            lang_alt(&[("x-default", "A")]),
        );
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Description, "  \n ");
        assert_eq!(
            form.edits(),
            Ok(vec![Edit::Clear {
                property: "dc:description".into(),
                field: None,
            }])
        );
    }

    #[test]
    fn two_keywords_with_spaces_and_empty_parts() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Keywords, "sol, hav ,, ");
        assert_eq!(
            form.edits(),
            Ok(vec![set("dc:subject", list(&["sol", "hav"]))])
        );
    }

    #[test]
    fn creators_are_split_at_commas() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Creator, " Anna Berg,Per ");
        assert_eq!(
            form.edits(),
            Ok(vec![set("dc:creator", list(&["Anna Berg", "Per"]))])
        );
    }

    #[test]
    fn only_spaces_and_commas_clear_the_list() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:subject", list(&["sol", "hav"]));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Keywords, " , ,, ");
        assert_eq!(
            form.edits(),
            Ok(vec![Edit::Clear {
                property: "dc:subject".into(),
                field: None,
            }])
        );
    }

    #[test]
    fn an_unchanged_list_gives_no_edit() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "dc:subject",
            list(&["fågel", "fåglar", "test", "inte bra", "slaguggla"]),
        );
        insert(&mut metadata, "dc:creator", list(&["Anna", "Per"]));
        let mut form = form_with(metadata);
        assert_eq!(
            form.text(EditorField::Keywords),
            "fågel, fåglar, test, inte bra, slaguggla"
        );
        assert_eq!(form.edits(), Ok(Vec::new()));
        // Typed back as it was shown: still no edit.
        type_into(
            &mut form,
            EditorField::Keywords,
            "fågel, fåglar, test, inte bra, slaguggla",
        );
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn an_entry_with_a_comma_is_split_when_the_field_is_changed() {
        // Accepted by the owner (T108): a read keyword holding a comma is shown as it is and
        // gives no edit untouched, but becomes two keywords when the field is changed.
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:subject", list(&["Umeå, Sverige"]));
        let mut form = form_with(metadata);
        assert_eq!(form.text(EditorField::Keywords), "Umeå, Sverige");
        assert_eq!(form.edits(), Ok(Vec::new()));
        type_into(&mut form, EditorField::Keywords, "Umeå, Sverige, sol");
        assert_eq!(
            form.edits(),
            Ok(vec![set("dc:subject", list(&["Umeå", "Sverige", "sol"]))])
        );
    }

    #[test]
    fn truncated_date() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::DateCreated, "2026-11");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                "photoshop:DateCreated",
                Value::Date(IptcDate {
                    year: 2026,
                    month: Some(11),
                    day: None,
                    time: None,
                })
            )])
        );
    }

    #[test]
    fn bad_date_gives_the_error_text() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::DateCreated, "12/11/2026");
        assert_eq!(
            form.edits(),
            Err(vec![(
                EditorField::DateCreated,
                "Write the date as YYYY, YYYY-MM, YYYY-MM-DD or YYYY-MM-DDThh:mm[:ss], with a \
                 time zone only when it is known"
                    .to_string()
            )])
        );
    }

    #[test]
    fn an_error_is_returned_with_other_changed_fields() {
        // Of the D12 fields only the date can be wrong, so this is the most errors a form can
        // have: the valid changes around it do not hide it, and no edits are returned.
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::City, "Umeå");
        type_into(&mut form, EditorField::DateCreated, "2026-13");
        type_into(&mut form, EditorField::Keywords, "sol");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::DateCreated, date_error_text())])
        );
    }

    #[test]
    fn email_is_a_field_edit_of_the_contact_structure() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            CONTACT_INFO,
            contact(&[
                ("Iptc4xmpCore:CiEmailWork", "gammal@example.com"),
                ("Iptc4xmpCore:CiTelWork", "123"),
            ]),
        );
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Email, "ny@example.com");
        type_into(&mut form, EditorField::Phone, "");
        assert_eq!(
            form.edits(),
            Ok(vec![
                Edit::Set {
                    property: CONTACT_INFO.into(),
                    field: Some("Iptc4xmpCore:CiEmailWork".into()),
                    value: text("ny@example.com"),
                },
                Edit::Clear {
                    property: CONTACT_INFO.into(),
                    field: Some("Iptc4xmpCore:CiTelWork".into()),
                },
            ])
        );
    }

    // --- conflicts ---

    fn city_conflict(rule: ConflictRule) -> Conflict {
        Conflict {
            property: "photoshop:City".into(),
            values: vec![
                (Origin::XmpEmbedded, text("Göteborg")),
                (Origin::Iim, text("Malmö")),
            ],
            rule,
        }
    }

    fn metadata_with_city_conflict() -> Metadata {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Göteborg"));
        metadata.add_conflict(city_conflict(ConflictRule::Open));
        metadata
    }

    #[test]
    fn only_open_conflicts_are_listed() {
        let mut metadata = metadata_with_city_conflict();
        metadata.add_conflict(Conflict {
            property: "photoshop:State".into(),
            ..city_conflict(ConflictRule::Iim)
        });
        let form = form_with(metadata);
        assert_eq!(
            form.open_conflicts(),
            [OpenConflict {
                label: "City".into(),
                values: vec![
                    ConflictValue {
                        value: "Göteborg".into(),
                        origin: "XMP".into(),
                    },
                    ConflictValue {
                        value: "Malmö".into(),
                        origin: "IIM".into(),
                    },
                ],
                chosen: None,
            }]
        );
    }

    #[test]
    fn chosen_value_with_unchanged_text_is_a_set() {
        let mut form = form_with(metadata_with_city_conflict());
        // The value already shown: the text does not change, the choice still decides it.
        assert!(form.choose(0, 0));
        assert_eq!(form.text(EditorField::City), "Göteborg");
        assert_eq!(form.open_conflicts()[0].chosen, Some(0));
        assert_eq!(
            form.edits(),
            Ok(vec![set("photoshop:City", text("Göteborg"))])
        );
    }

    #[test]
    fn chosen_other_value_fills_the_field() {
        let mut form = form_with(metadata_with_city_conflict());
        assert!(form.choose(0, 1));
        assert_eq!(form.text(EditorField::City), "Malmö");
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Malmö"))]));
        // Typed over after the choice: the typed text is set instead.
        type_into(&mut form, EditorField::City, "Lund");
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Lund"))]));
    }

    #[test]
    fn choice_out_of_range_changes_nothing() {
        let mut form = form_with(metadata_with_city_conflict());
        assert!(!form.choose(1, 0));
        assert!(!form.choose(0, 2));
        assert_eq!(form.open_conflicts()[0].chosen, None);
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn conflict_on_a_property_that_is_not_a_field() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:CaptionWriter", text("Anna"));
        metadata.add_conflict(Conflict {
            property: "photoshop:CaptionWriter".into(),
            values: vec![
                (Origin::XmpEmbedded, text("Anna")),
                (Origin::Iim, text("Per")),
            ],
            rule: ConflictRule::Open,
        });
        let mut form = form_with(metadata);
        let conflicts = form.open_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].label, "Description Writer");
        assert!(form.choose(0, 1));
        assert_eq!(
            form.edits(),
            Ok(vec![set("photoshop:CaptionWriter", text("Per"))])
        );
    }

    // --- warnings ---

    fn warnings_of(metadata: Metadata) -> Vec<String> {
        let read = read_of(metadata);
        EditorForm::from_read(&read).warnings(&read, false)
    }

    #[test]
    fn too_long_for_iim_only_when_iim_is_written() {
        let long_city = "a".repeat(40);
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text(&long_city));
        let mut read = read_of(metadata);
        let form = EditorForm::from_read(&read);
        let expected = ["City is truncated in IIM (at most 32 bytes); XMP gets the whole text"];
        // A JPEG without IIM: only with create_iim.
        assert!(form.warnings(&read, false).is_empty());
        assert_eq!(form.warnings(&read, true), expected);
        // A JPEG with IIM.
        read.state = state(FileKind::Jpeg, true);
        assert_eq!(form.warnings(&read, false), expected);
        // A manufacturer RAW never gets IIM created.
        read.state = state(FileKind::Raw, false);
        assert!(form.warnings(&read, true).is_empty());
    }

    /// The byte limit of the IIM warning in singular and plural (T117).
    #[test]
    fn too_long_for_iim_has_singular_and_plural_bytes() {
        let warning = |max_bytes| {
            warning_text(&Warning::TooLongForIim {
                property: "photoshop:Urgency".into(),
                max_bytes,
            })
        };
        assert_eq!(
            warning(1),
            "Urgency is truncated in IIM (at most 1 byte); XMP gets the whole text"
        );
        assert_eq!(
            warning(2),
            "Urgency is truncated in IIM (at most 2 bytes); XMP gets the whole text"
        );
    }

    /// The tooltip of a field's name is the full IPTC name where the name leaves part of it out
    /// (T117, glossary G1).
    #[test]
    fn the_legacy_and_contact_fields_have_the_full_iptc_name_as_tooltip() {
        let tooltips: Vec<(EditorField, String)> = EditorField::ALL
            .into_iter()
            .filter_map(|field| field.tooltip().map(|tooltip| (field, tooltip)))
            .collect();
        let expected = [
            (EditorField::Email, "Email address(es)"),
            (EditorField::Phone, "Phone number(s)"),
            (EditorField::Web, "Web URL(s)"),
            (EditorField::Sublocation, "Sublocation (legacy)"),
            (EditorField::City, "City (legacy)"),
            (EditorField::State, "Province or State (legacy)"),
            (EditorField::Country, "Country (legacy)"),
            (EditorField::CountryCode, "Country Code (legacy)"),
        ]
        .map(|(field, tooltip)| (field, tooltip.to_string()));
        assert_eq!(tooltips, expected);
    }

    #[test]
    fn warnings_see_the_edits() {
        let read = read_of(Metadata::new());
        let mut form = EditorForm::from_read(&read);
        type_into(&mut form, EditorField::CountryCode, "S1");
        assert_eq!(
            form.warnings(&read, false),
            ["Country Code S1 should be two or three letters (ISO 3166-1)"]
        );
        type_into(&mut form, EditorField::CountryCode, "SWE");
        assert!(form.warnings(&read, false).is_empty());
    }

    #[test]
    fn alt_text_warning() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "Iptc4xmpCore:AltTextAccessibility",
            lang_alt(&[("x-default", &"b".repeat(251))]),
        );
        assert_eq!(
            warnings_of(metadata),
            ["Alt Text (Accessibility) is longer than 250 characters (251)"]
        );
    }

    #[test]
    fn jobtitle_without_creator_warning() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:AuthorsPosition", text("Fotograf"));
        assert_eq!(
            warnings_of(metadata),
            ["Creator's jobtitle without Creator"]
        );
    }

    #[test]
    fn digital_source_type_warning() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "Iptc4xmpExt:DigitalSourceType",
            text("http://example.com/kamera"),
        );
        assert_eq!(
            warnings_of(metadata),
            ["Digital Source Type http://example.com/kamera is not in IPTC's list"]
        );
    }

    #[test]
    fn registry_entry_warning() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "Iptc4xmpExt:RegistryId",
            Value::StructList(vec![
                StructValue {
                    fields: vec![
                        ("Iptc4xmpExt:RegItemId".into(), text("1")),
                        ("Iptc4xmpExt:RegOrgId".into(), text("org")),
                    ],
                },
                StructValue {
                    fields: vec![("Iptc4xmpExt:RegItemId".into(), text("2"))],
                },
            ]),
        );
        assert_eq!(
            warnings_of(metadata),
            ["Registry entry 2 is missing an item ID or an organization ID"]
        );
    }

    #[test]
    fn rating_warning() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "xmp:Rating", Value::Number("7".into()));
        assert_eq!(warnings_of(metadata), ["Rating 7 is outside −1 to 5"]);
    }

    // --- notices ---

    #[test]
    fn every_notice_text() {
        assert_eq!(
            notice_text(&WriteNotice::Truncated {
                property: "photoshop:Headline".into()
            }),
            "Headline was truncated in IIM; XMP has the whole text"
        );
        assert_eq!(
            notice_text(&WriteNotice::IimConvertedToUtf8),
            "The IIM texts were rewritten as UTF-8"
        );
        assert_eq!(
            notice_text(&WriteNotice::ConflictKept {
                property: "photoshop:City".into()
            }),
            "City: the conflict is unresolved and was not written"
        );
        assert_eq!(
            notice_text(&WriteNotice::DigestNotWritten),
            "No new IIM checksum was written, because a conflict is unresolved"
        );
    }

    // --- the window's texts (T105) ---

    #[test]
    fn every_editor_text() {
        let cases = [
            (
                EditorText::WindowTitle {
                    file: "bild.jpg".into(),
                },
                "Metadata – bild.jpg",
            ),
            (EditorText::ConflictsHeading, "Unresolved Conflicts"),
            (
                EditorText::ConflictChoice {
                    value: "Solnedgång".into(),
                    origin: "IIM".into(),
                },
                "Solnedgång (IIM)",
            ),
            (EditorText::WarningsHeading, "Warnings"),
            (EditorText::OtherValuesHeading, "Other Metadata"),
            (EditorText::Ok, "OK"),
            (EditorText::Saving, "Saving …"),
            (EditorText::Saved, "Saved."),
            (
                EditorText::SaveFailed {
                    file: "bild.ARW".into(),
                    reason: "filen är skrivskyddad".into(),
                },
                "Could not save metadata for bild.ARW: filen är skrivskyddad",
            ),
            (EditorText::SaveQuestion, "Save changes?"),
            (EditorText::Save, "Save"),
            (EditorText::Discard, "Discard"),
            (EditorText::Cancel, "Cancel"),
            (EditorText::Previous, "Previous"),
            (EditorText::Next, "Next"),
            (EditorText::SaveAndNext, "Save and Next"),
            (EditorText::UnsavedAtExitTitle, "Unsaved Changes"),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }

    /// The texts moved here from `metadata.rs` and `import_worker.rs` (T116) keep their wording.
    #[test]
    fn the_moved_metadata_texts_keep_their_wording() {
        assert_eq!(
            editor_text(&EditorText::MoreUnsaved { more: 3 }),
            "… and 3 more"
        );
        assert_eq!(
            editor_text(&EditorText::ReadingMetadata),
            "Reading metadata…"
        );
        assert_eq!(
            editor_text(&EditorText::MetadataError {
                error: "boom".into()
            }),
            "Metadata error: boom"
        );
        let reasons = [
            (WriteReason::ReadOnly, "the file is read-only"),
            (WriteReason::Locked, "the file is open in another program"),
            (WriteReason::NotWritten, "the file could not be written"),
            (WriteReason::NoJournal, "the write log could not be opened"),
            (
                WriteReason::ExifToolTooOld,
                "ExifTool is too old (at least 12.67 is needed)",
            ),
            (
                WriteReason::NotSafeIntact,
                "the metadata could not be written safely; the file is unchanged",
            ),
            (
                WriteReason::NotSafeAtBackup {
                    name: "bild.pwbak-1.jpg".into(),
                },
                "the metadata could not be written safely; the original is now bild.pwbak-1.jpg",
            ),
            (
                WriteReason::NotSafeReplaced,
                "the file was written but the final check failed; the earlier version is kept as \
                 a backup",
            ),
            (WriteReason::QueueFull, "the metadata queue is full"),
            (WriteReason::WorkerStopped, "metadata writing has stopped"),
        ];
        for (reason, expected) in reasons {
            assert_eq!(write_reason_text(&reason), expected, "{reason:?}");
        }
    }
}
