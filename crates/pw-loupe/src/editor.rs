//! The editor's form (ADR 0007 I10k, owner decision D12): the fields of one image, the choices of
//! its unresolved conflicts, the edits they give, the warnings and the texts of the write notices.
//! Pure functions, no egui; T105 draws the window.
//!
//! - **Fields** (D12): 22 fields in a fixed order, [`EditorField::ALL`]. Each is a property of the
//!   table, or (E-post, Telefon, Webb) a field of `Iptc4xmpCore:CreatorContactInfo`. The value
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

use pw_metadata::date::IptcDate;
use pw_metadata::edit::{Edit, apply_edits};
use pw_metadata::properties::{self, Occurrence, ValueKind};
use pw_metadata::read::MetadataRead;
use pw_metadata::reconcile::FileKind;
use pw_metadata::validate::{Warning, validate};
use pw_metadata::value::{Conflict, ConflictRule, LangAlt, Value};
use pw_metadata::write_plan::WriteNotice;

use crate::info_panel::{ConflictValue, Language, origin_text, property_label, value_text};

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

    /// The field's name ([`property_label`] in Swedish).
    pub fn label(self) -> String {
        property_label(self.struct_field().unwrap_or(self.property()), Language::Sv)
    }

    /// True for the fields shown as a box of several lines.
    pub fn multiline(self) -> bool {
        matches!(
            self,
            EditorField::Description
                | EditorField::AltText
                | EditorField::Instructions
                | EditorField::UsageTerms
                | EditorField::Keywords
                | EditorField::Creator
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

/// The text of a value in the form: a text and a number as they are; a Lang Alt its shown text; a
/// list one entry per line; a date in XMP form; anything else as the info panel shows it.
fn form_text(value: &Value) -> String {
    match value {
        Value::Text(text) | Value::Number(text) => text.clone(),
        Value::LangAlt(lang_alt) => lang_alt
            .display()
            .map(|(text, _)| text.to_string())
            .unwrap_or_default(),
        Value::List(entries) => entries.join("\n"),
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
    "Skriv datumet som ÅÅÅÅ, ÅÅÅÅ-MM, ÅÅÅÅ-MM-DD eller ÅÅÅÅ-MM-DDTtt:mm[:ss], med tidszon bara om \
     den är känd"
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

    /// The note of a Lang Alt field that has no `x-default`: its language, `(<språk>)`; saving
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
                label: property_label(&conflict.property, Language::Sv),
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

/// The edit of a changed field with the text `text`, or the field's error.
fn field_edit(field: EditorField, text: &str) -> Result<Edit, String> {
    let property = field.property().to_string();
    let struct_field = field.struct_field().map(str::to_string);
    if text.trim().is_empty() {
        return Ok(Edit::Clear {
            property,
            field: struct_field,
        });
    }
    let value = match field.kind() {
        FieldKind::Text => Value::Text(text.to_string()),
        FieldKind::LangAlt => Value::LangAlt(LangAlt::from_entries(vec![(
            X_DEFAULT.to_string(),
            text.to_string(),
        )])),
        FieldKind::List => Value::List(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect(),
        ),
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

/// The Swedish text of a warning (§11).
fn warning_text(warning: &Warning) -> String {
    match warning {
        Warning::TooLongForIim {
            property,
            max_bytes,
        } => format!(
            "{field} blir avkortat i IIM (högst {max_bytes} byte); XMP får hela texten",
            field = property_label(property, Language::Sv)
        ),
        Warning::AltTextTooLong { characters } => {
            format!("Alternativtexten är längre än 250 tecken ({characters})")
        }
        Warning::CountryCodeForm { code } => {
            format!("Landskoden {code} ska vara två eller tre bokstäver (ISO 3166-1)")
        }
        Warning::JobtitleWithoutCreator => format!(
            "{jobtitle} utan {creator}",
            jobtitle = property_label("photoshop:AuthorsPosition", Language::Sv),
            creator = property_label("dc:creator", Language::Sv)
        ),
        Warning::DigitalSourceTypeUnknown { uri } => format!(
            "{field} {uri} finns inte i IPTC:s lista",
            field = property_label("Iptc4xmpExt:DigitalSourceType", Language::Sv)
        ),
        Warning::RegistryEntryIncomplete { index } => format!(
            "Registerpost {n} saknar post-id eller organisation",
            n = index + 1
        ),
        Warning::RatingOutOfRange { value } => {
            format!("Betyget {value} ligger utanför −1 till 5")
        }
    }
}

/// The text of a notice of an editor write (T105).
pub fn notice_text(notice: &WriteNotice) -> String {
    match notice {
        WriteNotice::Truncated { property } => format!(
            "{field} kortades i IIM; XMP har hela texten",
            field = property_label(property, Language::Sv)
        ),
        WriteNotice::IimConvertedToUtf8 => "IIM-texterna skrevs om som UTF-8".to_string(),
        WriteNotice::ConflictKept { property } => format!(
            "{field}: konflikten är olöst och skrevs inte",
            field = property_label(property, Language::Sv)
        ),
        WriteNotice::DigestNotWritten => {
            "Ingen ny IIM-kontrollsumma skrevs, eftersom en konflikt är olöst".to_string()
        }
    }
}

/// A fixed text of the editor window (T105), as [`PanelText`](crate::PanelText) for the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorText {
    /// The button at the top of the info panel that opens the editor.
    EditButton,
    /// The window's title; `file` is the image's file name.
    WindowTitle {
        file: String,
    },
    /// The heading of the unresolved conflicts.
    ConflictsHeading,
    /// One radio button of a conflict: a value and its origin.
    ConflictChoice {
        value: String,
        origin: String,
    },
    /// The heading of the warnings.
    WarningsHeading,
    Save,
    Close,
    /// Shown while the write runs.
    Saving,
    /// Shown after a write that succeeded.
    Saved,
    /// A write that failed; `reason` as `write_error_text` gives it for ratings.
    SaveFailed {
        file: String,
        reason: String,
    },
    /// Asked when the window is closed with unsaved edits.
    UnsavedQuestion,
    Discard,
    Cancel,
    /// The title of the message at program exit listing the rating and editor writes that were
    /// not saved (owner decision 2026-11-12: one message for both).
    UnsavedAtExitTitle,
}

/// The text of a fixed text of the editor window, each a whole sentence from one template.
pub fn editor_text(text: &EditorText) -> String {
    match text {
        EditorText::EditButton => "Redigera".to_string(),
        EditorText::WindowTitle { file } => format!("Redigera metadata – {file}"),
        EditorText::ConflictsHeading => "Olösta konflikter".to_string(),
        EditorText::ConflictChoice { value, origin } => format!("{value} ({origin})"),
        EditorText::WarningsHeading => "Varningar".to_string(),
        EditorText::Save => "Spara".to_string(),
        EditorText::Close => "Stäng".to_string(),
        EditorText::Saving => "Sparar …".to_string(),
        EditorText::Saved => "Sparat.".to_string(),
        EditorText::SaveFailed { file, reason } => {
            format!("Kunde inte spara metadata för {file}: {reason}")
        }
        EditorText::UnsavedQuestion => "Spara ändringarna?".to_string(),
        EditorText::Discard => "Släng".to_string(),
        EditorText::Cancel => "Avbryt".to_string(),
        EditorText::UnsavedAtExitTitle => "Ändringar som inte sparades".to_string(),
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
                "Titel",
                "Rubrik",
                "Beskrivning",
                "Alternativtext",
                "Nyckelord",
                "Fotograf",
                "Fotografens titel",
                "E-post",
                "Telefon",
                "Webb",
                "Copyright",
                "Credit",
                "Källa",
                "Användarvillkor",
                "Skapad",
                "Plats",
                "Stad",
                "Län/delstat",
                "Land",
                "Landskod",
                "Instruktioner",
                "Jobb-id",
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
                EditorField::Keywords,
                EditorField::Creator,
                EditorField::UsageTerms,
                EditorField::Instructions,
            ]
        );
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
        assert_eq!(form.text(EditorField::Keywords), "sol\nhav");
        assert_eq!(form.text(EditorField::Creator), "Anna\nPer");
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
    fn two_keywords_on_three_lines_with_one_blank() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Keywords, " sol \n\n hav");
        assert_eq!(
            form.edits(),
            Ok(vec![set("dc:subject", list(&["sol", "hav"]))])
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
                "Skriv datumet som ÅÅÅÅ, ÅÅÅÅ-MM, ÅÅÅÅ-MM-DD eller ÅÅÅÅ-MM-DDTtt:mm[:ss], med \
                 tidszon bara om den är känd"
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
                label: "Stad".into(),
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
        assert_eq!(conflicts[0].label, "Beskrivningens författare");
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
        let expected = ["Stad blir avkortat i IIM (högst 32 byte); XMP får hela texten"];
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

    #[test]
    fn warnings_see_the_edits() {
        let read = read_of(Metadata::new());
        let mut form = EditorForm::from_read(&read);
        type_into(&mut form, EditorField::CountryCode, "S1");
        assert_eq!(
            form.warnings(&read, false),
            ["Landskoden S1 ska vara två eller tre bokstäver (ISO 3166-1)"]
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
            ["Alternativtexten är längre än 250 tecken (251)"]
        );
    }

    #[test]
    fn jobtitle_without_creator_warning() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:AuthorsPosition", text("Fotograf"));
        assert_eq!(warnings_of(metadata), ["Fotografens titel utan Fotograf"]);
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
            ["Digital källtyp http://example.com/kamera finns inte i IPTC:s lista"]
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
            ["Registerpost 2 saknar post-id eller organisation"]
        );
    }

    #[test]
    fn rating_warning() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "xmp:Rating", Value::Number("7".into()));
        assert_eq!(
            warnings_of(metadata),
            ["Betyget 7 ligger utanför −1 till 5"]
        );
    }

    // --- notices ---

    #[test]
    fn every_notice_text() {
        assert_eq!(
            notice_text(&WriteNotice::Truncated {
                property: "photoshop:Headline".into()
            }),
            "Rubrik kortades i IIM; XMP har hela texten"
        );
        assert_eq!(
            notice_text(&WriteNotice::IimConvertedToUtf8),
            "IIM-texterna skrevs om som UTF-8"
        );
        assert_eq!(
            notice_text(&WriteNotice::ConflictKept {
                property: "photoshop:City".into()
            }),
            "Stad: konflikten är olöst och skrevs inte"
        );
        assert_eq!(
            notice_text(&WriteNotice::DigestNotWritten),
            "Ingen ny IIM-kontrollsumma skrevs, eftersom en konflikt är olöst"
        );
    }

    // --- the window's texts (T105) ---

    #[test]
    fn every_editor_text() {
        let cases = [
            (EditorText::EditButton, "Redigera"),
            (
                EditorText::WindowTitle {
                    file: "bild.jpg".into(),
                },
                "Redigera metadata – bild.jpg",
            ),
            (EditorText::ConflictsHeading, "Olösta konflikter"),
            (
                EditorText::ConflictChoice {
                    value: "Solnedgång".into(),
                    origin: "IIM".into(),
                },
                "Solnedgång (IIM)",
            ),
            (EditorText::WarningsHeading, "Varningar"),
            (EditorText::Save, "Spara"),
            (EditorText::Close, "Stäng"),
            (EditorText::Saving, "Sparar …"),
            (EditorText::Saved, "Sparat."),
            (
                EditorText::SaveFailed {
                    file: "bild.ARW".into(),
                    reason: "filen är skrivskyddad".into(),
                },
                "Kunde inte spara metadata för bild.ARW: filen är skrivskyddad",
            ),
            (EditorText::UnsavedQuestion, "Spara ändringarna?"),
            (EditorText::Discard, "Släng"),
            (EditorText::Cancel, "Avbryt"),
            (
                EditorText::UnsavedAtExitTitle,
                "Ändringar som inte sparades",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }
}
