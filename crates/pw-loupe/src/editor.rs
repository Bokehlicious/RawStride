//! The editor's form (ADR 0007 I10k, owner decision D12): the fields of one image, the choices of
//! its unresolved conflicts, the edits they give, the warnings and the texts of the write notices.
//! Pure functions, no egui; T105 draws the window.
//!
//! - **Fields** (D12, ADR 0017 U10, T123): 40 fields in a fixed order and four sections,
//!   [`EditorField::ALL`], [`EditorSection`]. Each is a property of the table, or (the address,
//!   Email, Phone, Web URL) a field of `Iptc4xmpCore:CreatorContactInfo`. The value kind (text,
//!   Lang Alt, list, date) comes from the property table. Copyrighted is a check box (`True` or
//!   `False`, U10-Q3); Urgency, Category and Supplemental Categories are read-only.
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
use pw_metadata::{MIN_EXIFTOOL_VERSION, Rating};

use crate::filter::{LABEL_CHOICES, LabelChoice};
use crate::info_panel::{
    ConflictValue, Language, origin_text, property_label, property_tooltip, value_text,
};
use crate::rating_text::{label_choice_text, label_name, star_count_text};
use crate::ratings::RatingKey;

/// The language of the default entry of a Lang Alt value.
const X_DEFAULT: &str = "x-default";
/// The structure of the contact fields.
const CONTACT_INFO: &str = "Iptc4xmpCore:CreatorContactInfo";

/// A section of the metadata window's form (ADR 0017 §2.3.1, U10-Q1, U10-Q6), in the order of the
/// form; each is a collapsible heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorSection {
    Content,
    CreatorRights,
    TimePlace,
    Assignment,
    /// The rating and the label (T124, U10c-Q5): no fields of the form; its buttons
    /// ([`RatingButton`]) rate through the rating path, never through the form's edits.
    RatingLabel,
}

impl EditorSection {
    /// Every section in the order of the form.
    pub const ALL: [EditorSection; 5] = [
        EditorSection::Content,
        EditorSection::CreatorRights,
        EditorSection::TimePlace,
        EditorSection::Assignment,
        EditorSection::RatingLabel,
    ];
}

/// A button of the metadata window's Rating and Label section (ADR 0017 U10c-Q1, T124), for the
/// window's image only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatingButton {
    /// "No stars".
    NoStars,
    /// 1–5 stars.
    Stars(u8),
    Reject,
    /// A label by its XMP name ("Red", …).
    Label(&'static str),
    /// "No label".
    NoLabel,
}

impl RatingButton {
    /// The star buttons in the order drawn: no stars, 1–5, Reject.
    pub const STARS: [RatingButton; 7] = [
        RatingButton::NoStars,
        RatingButton::Stars(1),
        RatingButton::Stars(2),
        RatingButton::Stars(3),
        RatingButton::Stars(4),
        RatingButton::Stars(5),
        RatingButton::Reject,
    ];

    /// The label buttons in the order drawn: the choices of `LABEL_CHOICES` (the five labels,
    /// then no label), so the window and the filter list the same labels.
    pub const LABELS: [RatingButton; LABEL_CHOICES.len()] = {
        let mut buttons = [RatingButton::NoLabel; LABEL_CHOICES.len()];
        let mut index = 0;
        while index < LABEL_CHOICES.len() {
            buttons[index] = match LABEL_CHOICES[index] {
                LabelChoice::Label(label) => RatingButton::Label(label),
                LabelChoice::NoLabel => RatingButton::NoLabel,
            };
            index += 1;
        }
        buttons
    };

    /// The rating key the button gives: the one its key gives (Alt+0–5, `X`, Ctrl+0–5), so
    /// `apply`'s rules hold (a second press of the current label or of Reject clears it).
    pub fn key(self) -> RatingKey {
        match self {
            RatingButton::NoStars => RatingKey::Stars(0),
            RatingButton::Stars(stars) => RatingKey::Stars(stars),
            RatingButton::Reject => RatingKey::Reject,
            RatingButton::Label(label) => RatingKey::Label(label),
            RatingButton::NoLabel => RatingKey::ClearLabel,
        }
    }

    /// Whether the button shows the image's shown rating (`None`: not rated). An unknown label
    /// selects no label button.
    pub fn selected(self, rating: Option<&Rating>) -> bool {
        let stars = rating.and_then(|rating| rating.stars);
        let label = rating.and_then(|rating| rating.label.as_deref());
        match self {
            RatingButton::NoStars => matches!(stars, None | Some(0)),
            RatingButton::Stars(n) => i8::try_from(n).is_ok_and(|n| stars == Some(n)),
            RatingButton::Reject => stars == Some(-1),
            RatingButton::Label(name) => label == Some(name),
            RatingButton::NoLabel => label.is_none(),
        }
    }
}

/// A field of the editor (D12, ADR 0017 U10), in the order of the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorField {
    Title,
    Headline,
    Description,
    AltText,
    ExtDescription,
    DescriptionWriter,
    Keywords,
    PersonShown,
    OrganisationName,
    OrganisationCode,
    Event,
    Creator,
    CreatorJobtitle,
    Address,
    ContactCity,
    ContactRegion,
    PostalCode,
    ContactCountry,
    Email,
    Phone,
    Web,
    Copyright,
    Credit,
    Source,
    UsageTerms,
    WebStatement,
    Copyrighted,
    DateCreated,
    Sublocation,
    City,
    State,
    Country,
    CountryCode,
    Instructions,
    JobId,
    SubjectCode,
    SceneCode,
    Urgency,
    Category,
    SupplementalCategories,
}

/// The kind of value a field holds, from the property table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Text,
    LangAlt,
    List,
    Date,
}

/// The text of a checked check box (Copyrighted, U10-Q3); an unchecked box has an empty text.
const CHECKED: &str = "True";
/// The value an unchecked check box writes.
const UNCHECKED: &str = "False";

impl EditorField {
    /// Every field in the order of the form (D12, ADR 0017 U10-Q1, U10-Q4).
    pub const ALL: [EditorField; 40] = [
        EditorField::Title,
        EditorField::Headline,
        EditorField::Description,
        EditorField::AltText,
        EditorField::ExtDescription,
        EditorField::DescriptionWriter,
        EditorField::Keywords,
        EditorField::PersonShown,
        EditorField::OrganisationName,
        EditorField::OrganisationCode,
        EditorField::Event,
        EditorField::Creator,
        EditorField::CreatorJobtitle,
        EditorField::Address,
        EditorField::ContactCity,
        EditorField::ContactRegion,
        EditorField::PostalCode,
        EditorField::ContactCountry,
        EditorField::Email,
        EditorField::Phone,
        EditorField::Web,
        EditorField::Copyright,
        EditorField::Credit,
        EditorField::Source,
        EditorField::UsageTerms,
        EditorField::WebStatement,
        EditorField::Copyrighted,
        EditorField::DateCreated,
        EditorField::Sublocation,
        EditorField::City,
        EditorField::State,
        EditorField::Country,
        EditorField::CountryCode,
        EditorField::Instructions,
        EditorField::JobId,
        EditorField::SubjectCode,
        EditorField::SceneCode,
        EditorField::Urgency,
        EditorField::Category,
        EditorField::SupplementalCategories,
    ];

    /// The section the field is drawn in (U10-Q1).
    pub fn section(self) -> EditorSection {
        match self {
            EditorField::Title
            | EditorField::Headline
            | EditorField::Description
            | EditorField::AltText
            | EditorField::ExtDescription
            | EditorField::DescriptionWriter
            | EditorField::Keywords
            | EditorField::PersonShown
            | EditorField::OrganisationName
            | EditorField::OrganisationCode
            | EditorField::Event => EditorSection::Content,
            EditorField::Creator
            | EditorField::CreatorJobtitle
            | EditorField::Address
            | EditorField::ContactCity
            | EditorField::ContactRegion
            | EditorField::PostalCode
            | EditorField::ContactCountry
            | EditorField::Email
            | EditorField::Phone
            | EditorField::Web
            | EditorField::Copyright
            | EditorField::Credit
            | EditorField::Source
            | EditorField::UsageTerms
            | EditorField::WebStatement
            | EditorField::Copyrighted => EditorSection::CreatorRights,
            EditorField::DateCreated
            | EditorField::Sublocation
            | EditorField::City
            | EditorField::State
            | EditorField::Country
            | EditorField::CountryCode => EditorSection::TimePlace,
            EditorField::Instructions
            | EditorField::JobId
            | EditorField::SubjectCode
            | EditorField::SceneCode
            | EditorField::Urgency
            | EditorField::Category
            | EditorField::SupplementalCategories => EditorSection::Assignment,
        }
    }

    /// The XMP id of the property the field edits (the structure for a contact field).
    pub fn property(self) -> &'static str {
        match self {
            EditorField::Title => "dc:title",
            EditorField::Headline => "photoshop:Headline",
            EditorField::Description => "dc:description",
            EditorField::AltText => "Iptc4xmpCore:AltTextAccessibility",
            EditorField::ExtDescription => "Iptc4xmpCore:ExtDescrAccessibility",
            EditorField::DescriptionWriter => "photoshop:CaptionWriter",
            EditorField::Keywords => "dc:subject",
            EditorField::PersonShown => "Iptc4xmpExt:PersonInImage",
            EditorField::OrganisationName => "Iptc4xmpExt:OrganisationInImageName",
            EditorField::OrganisationCode => "Iptc4xmpExt:OrganisationInImageCode",
            EditorField::Event => "Iptc4xmpExt:Event",
            EditorField::Creator => "dc:creator",
            EditorField::CreatorJobtitle => "photoshop:AuthorsPosition",
            EditorField::Address
            | EditorField::ContactCity
            | EditorField::ContactRegion
            | EditorField::PostalCode
            | EditorField::ContactCountry
            | EditorField::Email
            | EditorField::Phone
            | EditorField::Web => CONTACT_INFO,
            EditorField::Copyright => "dc:rights",
            EditorField::Credit => "photoshop:Credit",
            EditorField::Source => "photoshop:Source",
            EditorField::UsageTerms => "xmpRights:UsageTerms",
            EditorField::WebStatement => "xmpRights:WebStatement",
            EditorField::Copyrighted => "xmpRights:Marked",
            EditorField::DateCreated => "photoshop:DateCreated",
            EditorField::Sublocation => "Iptc4xmpCore:Location",
            EditorField::City => "photoshop:City",
            EditorField::State => "photoshop:State",
            EditorField::Country => "photoshop:Country",
            EditorField::CountryCode => "Iptc4xmpCore:CountryCode",
            EditorField::Instructions => "photoshop:Instructions",
            EditorField::JobId => "photoshop:TransmissionReference",
            EditorField::SubjectCode => "Iptc4xmpCore:SubjectCode",
            EditorField::SceneCode => "Iptc4xmpCore:Scene",
            EditorField::Urgency => "photoshop:Urgency",
            EditorField::Category => "photoshop:Category",
            EditorField::SupplementalCategories => "photoshop:SupplementalCategories",
        }
    }

    /// The XMP id of the structure field a contact field edits; `None` for the other fields.
    pub fn struct_field(self) -> Option<&'static str> {
        match self {
            EditorField::Address => Some("Iptc4xmpCore:CiAdrExtadr"),
            EditorField::ContactCity => Some("Iptc4xmpCore:CiAdrCity"),
            EditorField::ContactRegion => Some("Iptc4xmpCore:CiAdrRegion"),
            EditorField::PostalCode => Some("Iptc4xmpCore:CiAdrPcode"),
            EditorField::ContactCountry => Some("Iptc4xmpCore:CiAdrCtry"),
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

    /// True for the fields shown as a box of several lines. Keywords, Creator and the other list
    /// fields are one line with their entries separated by commas (T108, U10-Q2).
    pub fn multiline(self) -> bool {
        matches!(
            self,
            EditorField::Description
                | EditorField::AltText
                | EditorField::ExtDescription
                | EditorField::Instructions
                | EditorField::UsageTerms
        )
    }

    /// True for a field shown as a check box (Copyrighted, U10-Q3): checked writes `True`,
    /// unchecked `False`.
    pub fn check_box(self) -> bool {
        matches!(self, EditorField::Copyrighted)
    }

    /// False for the deprecated IIM properties Urgency, Category and Supplemental Categories
    /// (ADR 0007 §5.5, ADR 0017 §2.3.1): shown read-only, only when the file has them, never an
    /// edit.
    pub fn editable(self) -> bool {
        !matches!(
            self,
            EditorField::Urgency | EditorField::Category | EditorField::SupplementalCategories
        )
    }

    /// True for a copyright-management field (ADR 0007 §8): Creator, Copyright Notice, Credit
    /// Line, Rights Usage Terms and Web Statement of Rights. "Apply to All Marked" empties such a
    /// field on the window's image only (U12c-Q3, T126).
    pub fn copyright_management(self) -> bool {
        matches!(
            self,
            EditorField::Creator
                | EditorField::Copyright
                | EditorField::Credit
                | EditorField::UsageTerms
                | EditorField::WebStatement
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

    /// The text of this field for `value`, a value of the field's property. A check box's text is
    /// `True` for `True` and empty for `False` (any case); any other value keeps its text and is
    /// shown read-only.
    fn text_of(self, value: &Value) -> String {
        match self.struct_field() {
            None if self.check_box() => {
                let text = form_text(value);
                if text.eq_ignore_ascii_case(CHECKED) {
                    CHECKED.to_string()
                } else if text.eq_ignore_ascii_case(UNCHECKED) {
                    String::new()
                } else {
                    text
                }
            }
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

/// The fields copied by "Copy All Fields" (ADR 0017 §2.3.1, U12b-Q1, T125): the source file's
/// name and the shown text of every editable field (the check box's `True` or empty). Kept by the
/// program until the next copy; not saved and not on the Windows clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedFields {
    file: String,
    texts: Vec<(EditorField, String)>,
}

impl CopiedFields {
    /// The name of the file the fields were copied from.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The copied text of `field`; `None` for a field that was not copied (read-only, deprecated).
    pub fn text(&self, field: EditorField) -> Option<&str> {
        self.texts
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, text)| text.as_str())
    }

    /// How many copied fields are not empty: the fields a paste puts in (U12b-Q2).
    pub fn count(&self) -> usize {
        self.texts
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .count()
    }
}

/// What "Apply to All Marked" writes (ADR 0017 §2.3.1, U12c-Q3, T126), from
/// [`EditorForm::batch_edits`].
#[derive(Clone, Debug, PartialEq)]
pub struct BatchEdits {
    /// The edits of the window's image: [`EditorForm::edits`], the chosen conflict values
    /// included.
    pub this_image: Vec<Edit>,
    /// The edits of every other marked image: only the changed fields, never a chosen conflict
    /// value, and no `Clear` of a copyright-management field.
    pub others: Vec<Edit>,
    /// The changed fields given a value, in form order.
    pub changed: Vec<EditorField>,
    /// The emptied fields that are emptied on every image, in form order.
    pub emptied: Vec<EditorField>,
    /// The emptied copyright-management fields, emptied on the window's image only.
    pub emptied_here_only: Vec<EditorField>,
}

impl BatchEdits {
    /// How many fields were changed: given a value, emptied, or emptied on this image only.
    pub fn field_count(&self) -> usize {
        self.changed.len() + self.emptied.len() + self.emptied_here_only.len()
    }
}

/// The lines of the confirmation of "Apply to All Marked" (U12c-Q4, T126): the question for
/// `images` images, then the changed, emptied and copyright-only fields by name and the `skipped`
/// marked files without metadata support, each line only when it has something to name.
pub fn apply_confirmation(batch: &BatchEdits, images: usize, skipped: usize) -> Vec<String> {
    let mut lines = vec![editor_text(&EditorText::ApplyQuestion {
        fields: batch.field_count(),
        images,
    })];
    if !batch.changed.is_empty() {
        lines.push(editor_text(&EditorText::ChangedFields(
            batch.changed.clone(),
        )));
    }
    if !batch.emptied.is_empty() {
        lines.push(editor_text(&EditorText::EmptiedFields(
            batch.emptied.clone(),
        )));
    }
    if !batch.emptied_here_only.is_empty() {
        lines.push(editor_text(&EditorText::EmptiedHereOnly(
            batch.emptied_here_only.clone(),
        )));
    }
    if skipped > 0 {
        lines.push(editor_text(&EditorText::SkippedFiles { files: skipped }));
    }
    lines
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

    /// The text of `field`, to be edited; `None` for a check box and a read-only field.
    pub fn text_mut(&mut self, field: EditorField) -> Option<&mut String> {
        if field.check_box() || !field.editable() {
            return None;
        }
        self.fields
            .iter_mut()
            .find(|state| state.field == field)
            .map(|state| &mut state.text)
    }

    /// The text of a field shown read-only: a deprecated field the file has (point 4 of T123),
    /// or a check box whose value is neither `True` nor `False` (U10-Q3). `None` for every
    /// other field, also a deprecated field the file does not have (it is not shown).
    pub fn read_only_text(&self, field: EditorField) -> Option<&str> {
        let state = self.state(field)?;
        let read_only = if field.check_box() {
            !state.original.is_empty() && state.original != CHECKED
        } else {
            !field.editable() && !state.original.is_empty()
        };
        read_only.then_some(state.original.as_str())
    }

    /// True when the check box `field` is checked.
    pub fn checked(&self, field: EditorField) -> bool {
        field.check_box() && self.text(field) == CHECKED
    }

    /// Checks or unchecks the check box `field`; nothing for another field or a check box shown
    /// read-only.
    pub fn set_checked(&mut self, field: EditorField, checked: bool) {
        if !field.check_box() || self.read_only_text(field).is_some() {
            return;
        }
        if let Some(state) = self.fields.iter_mut().find(|state| state.field == field) {
            state.text = if checked {
                CHECKED.to_string()
            } else {
                String::new()
            };
        }
    }

    /// "Copy All Fields" (U12b-Q1, T125): the text of every editable field as the form shows it,
    /// unsaved edits included, and the Copyrighted check box. Not the read-only fields (the
    /// deprecated ones, a check box with another value), the conflict choices or the errors. A
    /// Lang Alt gives its text, which is the `x-default` text the window edits (Q7).
    pub fn copy(&self, file: &str) -> CopiedFields {
        let texts = self
            .fields
            .iter()
            .filter(|state| state.field.editable() && self.read_only_text(state.field).is_none())
            .map(|state| (state.field, state.text.clone()))
            .collect();
        CopiedFields {
            file: file.to_string(),
            texts,
        }
    }

    /// "Paste All Fields" (U12b-Q2, T125): every copied text that is not empty goes into its field
    /// as if typed, an unsaved edit; the other fields keep their text, and a field shown read-only
    /// here is left as it is. The edits, errors and conflict decisions then follow as for typing.
    pub fn paste(&mut self, copied: &CopiedFields) {
        for (field, text) in &copied.texts {
            if text.is_empty() {
                continue;
            }
            if field.check_box() {
                self.set_checked(*field, text == CHECKED);
            } else if self.read_only_text(*field).is_none()
                && let Some(target) = self.text_mut(*field)
            {
                target.clone_from(text);
            }
        }
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
            // A read-only field never gives an edit (point 4, U10-Q3).
            if !state.field.editable() || self.read_only_text(state.field).is_some() {
                continue;
            }
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

    /// What "Apply to All Marked" writes (U12c-Q3, T126): for the window's image the edits of
    /// [`EditorForm::edits`]; for every other marked image only the edits of the changed fields
    /// (a field whose text differs from its start text), never a chosen conflict value, and no
    /// `Clear` of a copyright-management field (ADR 0007 §8), which is emptied on the window's
    /// image only. Every field error together, and no batch, when a field has one.
    pub fn batch_edits(&self) -> Result<BatchEdits, Vec<(EditorField, String)>> {
        let mut batch = BatchEdits {
            this_image: self.edits()?,
            others: Vec::new(),
            changed: Vec::new(),
            emptied: Vec::new(),
            emptied_here_only: Vec::new(),
        };
        for state in &self.fields {
            if !state.field.editable() || self.read_only_text(state.field).is_some() {
                continue;
            }
            if state.text == self.start_text(state) {
                continue;
            }
            // `edits` had no error, so neither has this field.
            let edit = field_edit(state.field, &state.text).map_err(|e| vec![(state.field, e)])?;
            let cleared = matches!(edit, Edit::Clear { .. });
            if cleared && state.field.copyright_management() {
                batch.emptied_here_only.push(state.field);
                continue;
            }
            if cleared {
                batch.emptied.push(state.field);
            } else {
                batch.changed.push(state.field);
            }
            batch.others.push(edit);
        }
        Ok(batch)
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
    if field.check_box() {
        let value = if text == CHECKED { CHECKED } else { UNCHECKED };
        return Ok(Edit::Set {
            property,
            field: struct_field,
            value: Value::Text(value.to_string()),
        });
    }
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
    /// The heading of a section of the form (U10-Q1).
    SectionHeading(EditorSection),
    /// The text of a button of the Rating and Label section (T124, U10c-Q1).
    RatingButton(RatingButton),
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
    /// The button that copies every editable field of the form (U12b-Q3, T125).
    CopyAllFields,
    /// The button that pastes the copied fields into the form (U12b-Q3, T125).
    PasteAllFields,
    /// The tooltip of "Paste All Fields": how many fields it holds and from which file.
    PasteTooltip { fields: usize, file: String },
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
    /// The line under the window's title with two or more images marked (U12c-Q1, T126).
    MarkedImages { images: usize },
    /// The button that writes the edits to every marked image (U12c-Q2, T126).
    ApplyToMarked { images: usize },
    /// The title of the confirmation of "Apply to All Marked" (U12c-Q4, T126).
    ApplyTitle,
    /// The confirmation's question: `fields` changed fields to `images` images.
    ApplyQuestion { fields: usize, images: usize },
    /// The confirmation's line naming the changed fields given a value.
    ChangedFields(Vec<EditorField>),
    /// The confirmation's line naming the fields emptied on every image.
    EmptiedFields(Vec<EditorField>),
    /// The confirmation's line naming the copyright fields emptied on the window's image only.
    EmptiedHereOnly(Vec<EditorField>),
    /// The confirmation's line counting the marked files without metadata support.
    SkippedFiles { files: usize },
    /// The confirmation's button that starts the writes.
    Write,
    /// Shown while the batch runs (U12c-Q5): the write `current` of `total`.
    Writing { current: usize, total: usize },
    /// The first line of the batch's summary (U12c-Q6).
    BatchWritten { written: usize, total: usize },
    /// A line of the summary: an unresolved conflict on `property` (its XMP id) kept in `file`.
    ConflictKept { file: String, property: String },
}

/// The names of `fields` in form order, separated by commas.
fn field_names(fields: &[EditorField]) -> String {
    fields
        .iter()
        .map(|field| field.label())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The text of a fixed text of the editor window, each a whole sentence from one template.
pub fn editor_text(text: &EditorText) -> String {
    match text {
        EditorText::WindowTitle { file } => format!("Metadata – {file}"),
        EditorText::SectionHeading(section) => match section {
            EditorSection::Content => "Content",
            EditorSection::CreatorRights => "Creator and Rights",
            EditorSection::TimePlace => "Time and Place",
            EditorSection::Assignment => "Assignment",
            EditorSection::RatingLabel => "Rating and Label",
        }
        .to_string(),
        EditorText::RatingButton(button) => match button {
            RatingButton::NoStars => "No stars".to_string(),
            RatingButton::Stars(stars) => star_count_text(*stars),
            RatingButton::Reject => "Reject".to_string(),
            RatingButton::Label(label) => label_name(label).to_string(),
            RatingButton::NoLabel => label_choice_text(LabelChoice::NoLabel).to_string(),
        },
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
        EditorText::CopyAllFields => "Copy All Fields".to_string(),
        EditorText::PasteAllFields => "Paste All Fields".to_string(),
        EditorText::PasteTooltip { fields, file } => {
            if *fields == 1 {
                format!("1 field from {file}")
            } else {
                format!("{fields} fields from {file}")
            }
        }
        EditorText::UnsavedAtExitTitle => "Unsaved Changes".to_string(),
        EditorText::MoreUnsaved { more } => format!("… and {more} more"),
        EditorText::ReadingMetadata => "Reading metadata…".to_string(),
        EditorText::MetadataError { error } => format!("Metadata error: {error}"),
        EditorText::MarkedImages { images } => {
            if *images == 1 {
                "1 image marked".to_string()
            } else {
                format!("{images} images marked")
            }
        }
        EditorText::ApplyToMarked { images } => format!("Apply to {images} Marked"),
        EditorText::ApplyTitle => "Apply to All Marked".to_string(),
        EditorText::ApplyQuestion { fields, images } => match (*fields == 1, *images == 1) {
            (true, true) => "Write 1 changed field to 1 image?".to_string(),
            (true, false) => format!("Write 1 changed field to {images} images?"),
            (false, true) => format!("Write {fields} changed fields to 1 image?"),
            (false, false) => format!("Write {fields} changed fields to {images} images?"),
        },
        EditorText::ChangedFields(fields) => format!("Changed: {}", field_names(fields)),
        EditorText::EmptiedFields(fields) => format!("Emptied: {}", field_names(fields)),
        EditorText::EmptiedHereOnly(fields) => {
            format!("Emptied only on this image: {}", field_names(fields))
        }
        EditorText::SkippedFiles { files } => {
            if *files == 1 {
                "Skipped: 1 file".to_string()
            } else {
                format!("Skipped: {files} files")
            }
        }
        EditorText::Write => "Write".to_string(),
        EditorText::Writing { current, total } => format!("Writing {current} of {total} …"),
        EditorText::BatchWritten { written, total } => {
            if *total == 1 {
                format!("Written to {written} of 1 image.")
            } else {
                format!("Written to {written} of {total} images.")
            }
        }
        EditorText::ConflictKept { file, property } => format!(
            "Unresolved conflict kept: {file} ({field})",
            field = property_label(property, Language::En)
        ),
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
                "Extended Description (Accessibility)",
                "Description Writer",
                "Keywords",
                "Person Shown in the Image",
                "Name of Organisation Featured in the Image",
                "Code of Organisation Featured in the Image",
                "Event Name",
                "Creator",
                "Creator's jobtitle",
                "Address",
                "City",
                "State/Province",
                "Postal Code",
                "Country",
                "Email",
                "Phone",
                "Web URL",
                "Copyright Notice",
                "Credit Line",
                "Source (Supply Chain)",
                "Rights Usage Terms",
                "Web Statement of Rights",
                "Copyrighted",
                "Date Created",
                "Sublocation",
                "City",
                "Province or State",
                "Country",
                "Country Code",
                "Instructions",
                "Job Id",
                "Subject Code",
                "Scene Code",
                "Urgency",
                "Category",
                "Supplemental Categories",
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
                EditorField::ExtDescription,
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
                | EditorField::ExtDescription
                | EditorField::Event
                | EditorField::Copyright
                | EditorField::UsageTerms => FieldKind::LangAlt,
                EditorField::Keywords
                | EditorField::Creator
                | EditorField::PersonShown
                | EditorField::OrganisationName
                | EditorField::OrganisationCode
                | EditorField::SubjectCode
                | EditorField::SceneCode
                | EditorField::SupplementalCategories => FieldKind::List,
                EditorField::DateCreated => FieldKind::Date,
                _ => FieldKind::Text,
            };
            assert_eq!(field.kind(), expected, "{field:?}");
        }
    }

    /// The fields of each section in the order of the form (T123, U10-Q1, U10-Q4).
    #[test]
    fn every_field_is_in_its_section_in_form_order() {
        use EditorField::*;
        let expected: [(EditorSection, &[EditorField]); 5] = [
            (
                EditorSection::Content,
                &[
                    Title,
                    Headline,
                    Description,
                    AltText,
                    ExtDescription,
                    DescriptionWriter,
                    Keywords,
                    PersonShown,
                    OrganisationName,
                    OrganisationCode,
                    Event,
                ],
            ),
            (
                EditorSection::CreatorRights,
                &[
                    Creator,
                    CreatorJobtitle,
                    Address,
                    ContactCity,
                    ContactRegion,
                    PostalCode,
                    ContactCountry,
                    Email,
                    Phone,
                    Web,
                    Copyright,
                    Credit,
                    Source,
                    UsageTerms,
                    WebStatement,
                    Copyrighted,
                ],
            ),
            (
                EditorSection::TimePlace,
                &[DateCreated, Sublocation, City, State, Country, CountryCode],
            ),
            (
                EditorSection::Assignment,
                &[
                    Instructions,
                    JobId,
                    SubjectCode,
                    SceneCode,
                    Urgency,
                    Category,
                    SupplementalCategories,
                ],
            ),
            // Last, without fields: the rating path's buttons (T124, U10c-Q5).
            (EditorSection::RatingLabel, &[]),
        ];
        assert_eq!(
            EditorSection::ALL.to_vec(),
            expected
                .iter()
                .map(|(section, _)| *section)
                .collect::<Vec<_>>()
        );
        // The sections follow each other in `ALL`: concatenated they are `ALL`.
        let concatenated: Vec<EditorField> = expected
            .iter()
            .flat_map(|(_, fields)| fields.iter().copied())
            .collect();
        assert_eq!(concatenated, EditorField::ALL.to_vec());
        for (section, fields) in expected {
            for field in fields {
                assert_eq!(field.section(), section, "{field:?}");
            }
        }
    }

    /// The property, structure field and kind of each field T123 adds.
    #[test]
    fn the_new_fields_property_structure_field_and_kind() {
        use EditorField::*;
        let cases = [
            (
                ExtDescription,
                "Iptc4xmpCore:ExtDescrAccessibility",
                None,
                FieldKind::LangAlt,
            ),
            (
                DescriptionWriter,
                "photoshop:CaptionWriter",
                None,
                FieldKind::Text,
            ),
            (
                PersonShown,
                "Iptc4xmpExt:PersonInImage",
                None,
                FieldKind::List,
            ),
            (
                OrganisationName,
                "Iptc4xmpExt:OrganisationInImageName",
                None,
                FieldKind::List,
            ),
            (
                OrganisationCode,
                "Iptc4xmpExt:OrganisationInImageCode",
                None,
                FieldKind::List,
            ),
            (Event, "Iptc4xmpExt:Event", None, FieldKind::LangAlt),
            (
                Address,
                CONTACT_INFO,
                Some("Iptc4xmpCore:CiAdrExtadr"),
                FieldKind::Text,
            ),
            (
                ContactCity,
                CONTACT_INFO,
                Some("Iptc4xmpCore:CiAdrCity"),
                FieldKind::Text,
            ),
            (
                ContactRegion,
                CONTACT_INFO,
                Some("Iptc4xmpCore:CiAdrRegion"),
                FieldKind::Text,
            ),
            (
                PostalCode,
                CONTACT_INFO,
                Some("Iptc4xmpCore:CiAdrPcode"),
                FieldKind::Text,
            ),
            (
                ContactCountry,
                CONTACT_INFO,
                Some("Iptc4xmpCore:CiAdrCtry"),
                FieldKind::Text,
            ),
            (
                WebStatement,
                "xmpRights:WebStatement",
                None,
                FieldKind::Text,
            ),
            (Copyrighted, "xmpRights:Marked", None, FieldKind::Text),
            (
                SubjectCode,
                "Iptc4xmpCore:SubjectCode",
                None,
                FieldKind::List,
            ),
            (SceneCode, "Iptc4xmpCore:Scene", None, FieldKind::List),
            (Urgency, "photoshop:Urgency", None, FieldKind::Text),
            (Category, "photoshop:Category", None, FieldKind::Text),
            (
                SupplementalCategories,
                "photoshop:SupplementalCategories",
                None,
                FieldKind::List,
            ),
        ];
        for (field, property, struct_field, kind) in cases {
            assert_eq!(field.property(), property, "{field:?}");
            assert_eq!(field.struct_field(), struct_field, "{field:?}");
            assert_eq!(field.kind(), kind, "{field:?}");
            // Every property is in the table, so the kind is not the fallback.
            assert!(properties::property(property).is_some(), "{property}");
        }
        // Only Copyrighted is a check box; only the three deprecated fields are read-only.
        let check_boxes: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|f| f.check_box())
            .collect();
        assert_eq!(check_boxes, [Copyrighted]);
        let read_only: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|f| !f.editable())
            .collect();
        assert_eq!(read_only, [Urgency, Category, SupplementalCategories]);
    }

    #[test]
    fn the_new_list_fields_are_one_line() {
        for field in [
            EditorField::PersonShown,
            EditorField::OrganisationName,
            EditorField::OrganisationCode,
            EditorField::SubjectCode,
            EditorField::SceneCode,
            EditorField::Event,
        ] {
            assert!(!field.multiline(), "{field:?}");
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

    // --- edits of the fields T123 adds ---

    /// A Lang Alt field (Event Name, Extended Description): unchanged → no edit, changed → `Set`
    /// of `x-default`, emptied → `Clear`.
    #[test]
    fn new_lang_alt_fields_unchanged_changed_and_emptied() {
        for (field, property) in [
            (EditorField::Event, "Iptc4xmpExt:Event"),
            (
                EditorField::ExtDescription,
                "Iptc4xmpCore:ExtDescrAccessibility",
            ),
        ] {
            let mut metadata = Metadata::new();
            insert(&mut metadata, property, lang_alt(&[("x-default", "Före")]));
            let mut form = form_with(metadata);
            assert_eq!(form.text(field), "Före", "{field:?}");
            assert_eq!(form.edits(), Ok(Vec::new()), "{field:?}");
            type_into(&mut form, field, "Efter");
            assert_eq!(
                form.edits(),
                Ok(vec![set(property, lang_alt(&[("x-default", "Efter")]))]),
                "{field:?}"
            );
            type_into(&mut form, field, " ");
            assert_eq!(
                form.edits(),
                Ok(vec![Edit::Clear {
                    property: property.into(),
                    field: None,
                }]),
                "{field:?}"
            );
        }
    }

    #[test]
    fn a_new_lang_alt_field_without_x_default_has_a_note() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "Iptc4xmpExt:Event",
            lang_alt(&[("sv", "Midsommar")]),
        );
        let form = form_with(metadata);
        assert_eq!(form.text(EditorField::Event), "Midsommar");
        assert_eq!(form.note(EditorField::Event), Some("(sv)".to_string()));
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    /// A list field: unchanged → no edit, changed → `Set` of the entries split at commas,
    /// emptied → `Clear`.
    #[test]
    fn new_list_fields_unchanged_changed_and_emptied() {
        for (field, property) in [
            (EditorField::PersonShown, "Iptc4xmpExt:PersonInImage"),
            (
                EditorField::OrganisationName,
                "Iptc4xmpExt:OrganisationInImageName",
            ),
            (
                EditorField::OrganisationCode,
                "Iptc4xmpExt:OrganisationInImageCode",
            ),
            (EditorField::SubjectCode, "Iptc4xmpCore:SubjectCode"),
            (EditorField::SceneCode, "Iptc4xmpCore:Scene"),
        ] {
            let mut metadata = Metadata::new();
            insert(&mut metadata, property, list(&["a1", "b2"]));
            let mut form = form_with(metadata);
            assert_eq!(form.text(field), "a1, b2", "{field:?}");
            assert_eq!(form.edits(), Ok(Vec::new()), "{field:?}");
            type_into(&mut form, field, "a1, b2,c3 ,");
            assert_eq!(
                form.edits(),
                Ok(vec![set(property, list(&["a1", "b2", "c3"]))]),
                "{field:?}"
            );
            type_into(&mut form, field, "");
            assert_eq!(
                form.edits(),
                Ok(vec![Edit::Clear {
                    property: property.into(),
                    field: None,
                }]),
                "{field:?}"
            );
        }
    }

    /// A text field (Description Writer, Web Statement of Rights).
    #[test]
    fn new_text_fields_unchanged_changed_and_emptied() {
        for (field, property) in [
            (EditorField::DescriptionWriter, "photoshop:CaptionWriter"),
            (EditorField::WebStatement, "xmpRights:WebStatement"),
        ] {
            let mut metadata = Metadata::new();
            insert(&mut metadata, property, text("https://example.com/a"));
            let mut form = form_with(metadata);
            assert_eq!(form.edits(), Ok(Vec::new()), "{field:?}");
            type_into(&mut form, field, "https://example.com/b");
            assert_eq!(
                form.edits(),
                Ok(vec![set(property, text("https://example.com/b"))]),
                "{field:?}"
            );
            type_into(&mut form, field, "");
            assert_eq!(
                form.edits(),
                Ok(vec![Edit::Clear {
                    property: property.into(),
                    field: None,
                }]),
                "{field:?}"
            );
        }
    }

    /// A contact address field is a `Set` or `Clear` of its structure field, as Email is.
    #[test]
    fn contact_address_fields_unchanged_changed_and_emptied() {
        let fields = [
            (EditorField::Address, "Iptc4xmpCore:CiAdrExtadr"),
            (EditorField::ContactCity, "Iptc4xmpCore:CiAdrCity"),
            (EditorField::ContactRegion, "Iptc4xmpCore:CiAdrRegion"),
            (EditorField::PostalCode, "Iptc4xmpCore:CiAdrPcode"),
            (EditorField::ContactCountry, "Iptc4xmpCore:CiAdrCtry"),
        ];
        for (field, struct_field) in fields {
            let mut metadata = Metadata::new();
            insert(
                &mut metadata,
                CONTACT_INFO,
                contact(&[
                    (struct_field, "Före"),
                    ("Iptc4xmpCore:CiEmailWork", "foto@example.com"),
                ]),
            );
            let mut form = form_with(metadata);
            assert_eq!(form.text(field), "Före", "{field:?}");
            assert_eq!(form.text(EditorField::Email), "foto@example.com");
            assert_eq!(form.edits(), Ok(Vec::new()), "{field:?}");
            type_into(&mut form, field, "Efter");
            assert_eq!(
                form.edits(),
                Ok(vec![Edit::Set {
                    property: CONTACT_INFO.into(),
                    field: Some(struct_field.into()),
                    value: text("Efter"),
                }]),
                "{field:?}"
            );
            type_into(&mut form, field, "");
            assert_eq!(
                form.edits(),
                Ok(vec![Edit::Clear {
                    property: CONTACT_INFO.into(),
                    field: Some(struct_field.into()),
                }]),
                "{field:?}"
            );
        }
    }

    /// Every editable new field's `Set` is accepted by `pw-metadata` as it is (no change there,
    /// point 2 of T123), with Copyrighted checked.
    #[test]
    fn the_new_fields_edits_are_accepted_by_apply_edits() {
        let mut form = form_with(Metadata::new());
        for field in [
            EditorField::ExtDescription,
            EditorField::DescriptionWriter,
            EditorField::PersonShown,
            EditorField::OrganisationName,
            EditorField::OrganisationCode,
            EditorField::Event,
            EditorField::Address,
            EditorField::ContactCity,
            EditorField::ContactRegion,
            EditorField::PostalCode,
            EditorField::ContactCountry,
            EditorField::WebStatement,
            EditorField::SubjectCode,
            EditorField::SceneCode,
        ] {
            type_into(&mut form, field, "https://example.com/x");
        }
        form.set_checked(EditorField::Copyrighted, true);
        let edits = form.edits().unwrap();
        assert_eq!(edits.len(), 15);
        let mut metadata = Metadata::new();
        let applied = apply_edits(&mut metadata, &edits);
        assert!(applied.is_ok(), "{applied:?}");
        assert_eq!(
            metadata
                .get("xmpRights:Marked")
                .map(|stored| stored.value.clone()),
            Some(text("True"))
        );
    }

    #[test]
    fn copyrighted_absent_is_unchecked_and_checking_sets_true() {
        let mut form = form_with(Metadata::new());
        assert!(!form.checked(EditorField::Copyrighted));
        assert_eq!(form.read_only_text(EditorField::Copyrighted), None);
        // A check box has no text box.
        assert!(form.text_mut(EditorField::Copyrighted).is_none());
        assert_eq!(form.edits(), Ok(Vec::new()));
        form.set_checked(EditorField::Copyrighted, true);
        assert!(form.checked(EditorField::Copyrighted));
        assert_eq!(
            form.edits(),
            Ok(vec![set("xmpRights:Marked", text("True"))])
        );
        // Unchecked again, as it was read: no edit.
        form.set_checked(EditorField::Copyrighted, false);
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn copyrighted_true_in_any_case_is_checked_and_unchecking_sets_false() {
        for value in ["True", "true", "TRUE"] {
            let mut metadata = Metadata::new();
            insert(&mut metadata, "xmpRights:Marked", text(value));
            let mut form = form_with(metadata);
            assert!(form.checked(EditorField::Copyrighted), "{value}");
            assert_eq!(form.read_only_text(EditorField::Copyrighted), None);
            assert_eq!(form.edits(), Ok(Vec::new()), "{value}");
            form.set_checked(EditorField::Copyrighted, false);
            assert_eq!(
                form.edits(),
                Ok(vec![set("xmpRights:Marked", text("False"))]),
                "{value}"
            );
        }
    }

    #[test]
    fn copyrighted_false_in_any_case_is_unchecked_and_checking_sets_true() {
        for value in ["False", "false"] {
            let mut metadata = Metadata::new();
            insert(&mut metadata, "xmpRights:Marked", text(value));
            let mut form = form_with(metadata);
            assert!(!form.checked(EditorField::Copyrighted), "{value}");
            assert_eq!(form.edits(), Ok(Vec::new()), "{value}");
            form.set_checked(EditorField::Copyrighted, true);
            assert_eq!(
                form.edits(),
                Ok(vec![set("xmpRights:Marked", text("True"))]),
                "{value}"
            );
        }
    }

    #[test]
    fn copyrighted_with_another_value_is_read_only_and_gives_no_edit() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "xmpRights:Marked", text("Ja"));
        let mut form = form_with(metadata);
        assert_eq!(form.read_only_text(EditorField::Copyrighted), Some("Ja"));
        assert!(!form.checked(EditorField::Copyrighted));
        // Neither checking nor unchecking changes it.
        form.set_checked(EditorField::Copyrighted, true);
        assert!(!form.checked(EditorField::Copyrighted));
        form.set_checked(EditorField::Copyrighted, false);
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn the_deprecated_fields_are_read_only_when_present_and_never_give_an_edit() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "photoshop:Urgency",
            Value::Number("5".into()),
        );
        insert(&mut metadata, "photoshop:Category", text("SPO"));
        insert(
            &mut metadata,
            "photoshop:SupplementalCategories",
            list(&["fotboll", "sport"]),
        );
        let mut form = form_with(metadata);
        assert_eq!(form.read_only_text(EditorField::Urgency), Some("5"));
        assert_eq!(form.read_only_text(EditorField::Category), Some("SPO"));
        assert_eq!(
            form.read_only_text(EditorField::SupplementalCategories),
            Some("fotboll, sport")
        );
        for field in [
            EditorField::Urgency,
            EditorField::Category,
            EditorField::SupplementalCategories,
        ] {
            assert!(form.text_mut(field).is_none(), "{field:?}");
            form.set_checked(field, true);
        }
        assert_eq!(form.edits(), Ok(Vec::new()));

        // Absent: not shown, and still no edit.
        let form = form_with(Metadata::new());
        for field in [
            EditorField::Urgency,
            EditorField::Category,
            EditorField::SupplementalCategories,
        ] {
            assert_eq!(form.read_only_text(field), None, "{field:?}");
        }
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn an_editable_field_is_never_read_only() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Umeå"));
        insert(&mut metadata, "Iptc4xmpExt:PersonInImage", list(&["Anna"]));
        let form = form_with(metadata);
        for field in EditorField::ALL {
            if field.editable() && !field.check_box() {
                assert_eq!(form.read_only_text(field), None, "{field:?}");
            }
        }
    }

    /// A conflict on a property that is not a field (T107), now that Description Writer is a
    /// field: Intellectual Genre stays under Other Metadata (U10-Q4).
    #[test]
    fn conflict_on_intellectual_genre_which_is_not_a_field() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "Iptc4xmpCore:IntellectualGenre", text("A"));
        metadata.add_conflict(Conflict {
            property: "Iptc4xmpCore:IntellectualGenre".into(),
            values: vec![(Origin::XmpEmbedded, text("A")), (Origin::Iim, text("B"))],
            rule: ConflictRule::Open,
        });
        let mut form = form_with(metadata);
        assert!(
            EditorField::ALL
                .iter()
                .all(|f| f.property() != "Iptc4xmpCore:IntellectualGenre")
        );
        assert!(form.choose(0, 1));
        assert_eq!(
            form.edits(),
            Ok(vec![set("Iptc4xmpCore:IntellectualGenre", text("B"))])
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

    // --- copy and paste all fields (T125) ---

    /// A form with a text, a Lang Alt with `x-default` and another language, a list, the
    /// Copyrighted check box, every deprecated field, a rating and a label.
    fn source_metadata() -> Metadata {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Göteborg"));
        insert(
            &mut metadata,
            "dc:title",
            lang_alt(&[("sv", "Solnedgång"), ("x-default", "Sunset")]),
        );
        insert(&mut metadata, "dc:subject", list(&["hav", "kväll"]));
        insert(&mut metadata, "xmpRights:Marked", text("True"));
        insert(
            &mut metadata,
            "photoshop:Urgency",
            Value::Number("5".into()),
        );
        insert(&mut metadata, "photoshop:Category", text("SPO"));
        insert(
            &mut metadata,
            "photoshop:SupplementalCategories",
            list(&["fotboll"]),
        );
        insert(&mut metadata, "xmp:Rating", Value::Number("4".into()));
        insert(&mut metadata, "xmp:Label", text("Red"));
        metadata
    }

    #[test]
    fn copy_takes_every_editable_fields_shown_text_and_nothing_read_only() {
        let mut form = form_with(source_metadata());
        // An unsaved edit is copied as the window shows it.
        type_into(&mut form, EditorField::Headline, "Kväll vid havet");
        let copied = form.copy("a.jpg");
        assert_eq!(copied.file(), "a.jpg");
        assert_eq!(copied.text(EditorField::City), Some("Göteborg"));
        assert_eq!(copied.text(EditorField::Headline), Some("Kväll vid havet"));
        // The Lang Alt's x-default text, the list on one line, the checked box.
        assert_eq!(copied.text(EditorField::Title), Some("Sunset"));
        assert_eq!(copied.text(EditorField::Keywords), Some("hav, kväll"));
        assert_eq!(copied.text(EditorField::Copyrighted), Some("True"));
        // An empty editable field is copied empty.
        assert_eq!(copied.text(EditorField::Credit), Some(""));
        // No deprecated field, and every editable field once, in form order.
        for field in [
            EditorField::Urgency,
            EditorField::Category,
            EditorField::SupplementalCategories,
        ] {
            assert_eq!(copied.text(field), None, "{field:?}");
        }
        let fields: Vec<EditorField> = copied.texts.iter().map(|(field, _)| *field).collect();
        let editable: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|field| field.editable())
            .collect();
        assert_eq!(fields, editable);
        // No rating or label: no copied field is one of their properties.
        assert!(
            copied
                .texts
                .iter()
                .all(|(field, _)| !["xmp:Rating", "xmp:Label"].contains(&field.property()))
        );
        // The non-empty fields: City, Headline, Title, Keywords, Copyrighted.
        assert_eq!(copied.count(), 5);
    }

    #[test]
    fn copy_leaves_out_a_copyrighted_box_shown_read_only() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "xmpRights:Marked", text("Ja"));
        let copied = form_with(metadata).copy("a.jpg");
        assert_eq!(copied.text(EditorField::Copyrighted), None);
    }

    #[test]
    fn paste_leaves_the_fields_empty_in_the_copy_as_they_were() {
        let mut source = Metadata::new();
        insert(&mut source, "photoshop:City", text("Lund"));
        let copied = form_with(source).copy("a.jpg");
        let mut target = Metadata::new();
        insert(&mut target, "photoshop:City", text("Malmö"));
        insert(&mut target, "photoshop:Credit", text("Anna"));
        insert(&mut target, "dc:title", lang_alt(&[("x-default", "Hamn")]));
        insert(&mut target, "xmpRights:Marked", text("True"));
        let mut form = form_with(target);
        form.paste(&copied);
        assert_eq!(form.text(EditorField::City), "Lund");
        // Empty in the copy (Credit, Title, an unchecked Copyrighted): not touched.
        assert_eq!(form.text(EditorField::Credit), "Anna");
        assert_eq!(form.text(EditorField::Title), "Hamn");
        assert!(form.checked(EditorField::Copyrighted));
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Lund"))]));
    }

    #[test]
    fn a_pasted_text_equal_to_the_start_text_gives_no_edit_and_a_different_one_a_set() {
        let mut source = Metadata::new();
        insert(&mut source, "photoshop:City", text("Göteborg"));
        insert(&mut source, "photoshop:Credit", text("Per"));
        insert(&mut source, "xmpRights:Marked", text("True"));
        let copied = form_with(source).copy("a.jpg");
        let mut target = Metadata::new();
        insert(&mut target, "photoshop:City", text("Göteborg"));
        insert(&mut target, "photoshop:Credit", text("Anna"));
        let mut form = form_with(target);
        form.paste(&copied);
        assert_eq!(
            form.edits(),
            Ok(vec![
                set("photoshop:Credit", text("Per")),
                set("xmpRights:Marked", text("True")),
            ])
        );
    }

    #[test]
    fn a_pasted_bad_date_gives_the_fields_error() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::DateCreated, "igår");
        let copied = form.copy("a.jpg");
        let mut target = form_with(Metadata::new());
        target.paste(&copied);
        assert_eq!(target.text(EditorField::DateCreated), "igår");
        assert_eq!(
            target.edits(),
            Err(vec![(EditorField::DateCreated, date_error_text())])
        );
    }

    #[test]
    fn a_field_shown_read_only_on_the_target_is_not_changed() {
        let copied = form_with(source_metadata()).copy("a.jpg");
        let mut target = Metadata::new();
        insert(&mut target, "xmpRights:Marked", text("Ja"));
        insert(&mut target, "photoshop:Category", text("NYH"));
        let mut form = form_with(target);
        form.paste(&copied);
        assert_eq!(form.read_only_text(EditorField::Copyrighted), Some("Ja"));
        assert!(!form.checked(EditorField::Copyrighted));
        assert_eq!(form.read_only_text(EditorField::Category), Some("NYH"));
        // Only the editable fields of the copy give edits.
        let edits = form.edits().unwrap();
        assert!(
            edits.iter().all(|edit| !matches!(
                edit,
                Edit::Set { property, .. } | Edit::Clear { property, .. }
                    if property == "xmpRights:Marked" || property == "photoshop:Category"
            )),
            "{edits:?}"
        );
        // City, Title and Keywords.
        assert_eq!(edits.len(), 3);
    }

    #[test]
    fn a_pasted_list_is_split_at_commas() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Keywords, "hav,  kväll ,, sol");
        let copied = form.copy("a.jpg");
        let mut target = form_with(Metadata::new());
        target.paste(&copied);
        assert_eq!(
            target.edits(),
            Ok(vec![set("dc:subject", list(&["hav", "kväll", "sol"]))])
        );
    }

    #[test]
    fn a_pasted_value_on_a_property_with_an_open_conflict_decides_it() {
        let mut source = Metadata::new();
        insert(&mut source, "photoshop:City", text("Lund"));
        let copied = form_with(source).copy("a.jpg");
        let mut form = form_with(metadata_with_city_conflict());
        form.paste(&copied);
        assert_eq!(form.text(EditorField::City), "Lund");
        // The Set on the conflicted property decides it, as typing does (T83, Q1).
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Lund"))]));
    }

    #[test]
    fn copy_then_paste_on_the_same_form_gives_no_edits() {
        let mut form = form_with(source_metadata());
        let copied = form.copy("a.jpg");
        form.paste(&copied);
        assert_eq!(form.edits(), Ok(Vec::new()));
        assert_eq!(form, form_with(source_metadata()));
    }

    #[test]
    fn the_copy_and_paste_texts() {
        assert_eq!(editor_text(&EditorText::CopyAllFields), "Copy All Fields");
        assert_eq!(editor_text(&EditorText::PasteAllFields), "Paste All Fields");
        assert_eq!(
            editor_text(&EditorText::PasteTooltip {
                fields: 1,
                file: "a.jpg".into()
            }),
            "1 field from a.jpg"
        );
        assert_eq!(
            editor_text(&EditorText::PasteTooltip {
                fields: 12,
                file: "DSC01234.ARW".into()
            }),
            "12 fields from DSC01234.ARW"
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
            (EditorField::SubjectCode, "Subject Code (legacy)"),
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

    /// The headings of the form's sections (T123, U10-Q1).
    #[test]
    fn the_section_headings() {
        let headings: Vec<String> = EditorSection::ALL
            .into_iter()
            .map(|section| editor_text(&EditorText::SectionHeading(section)))
            .collect();
        assert_eq!(
            headings,
            [
                "Content",
                "Creator and Rights",
                "Time and Place",
                "Assignment",
                "Rating and Label"
            ]
        );
        assert_eq!(EditorSection::ALL.last(), Some(&EditorSection::RatingLabel));
    }

    /// Each button of the Rating and Label section gives its key's `RatingKey` (T124, U10c-Q1).
    #[test]
    fn each_rating_button_gives_the_rating_key_of_its_key() {
        let stars: Vec<RatingKey> = RatingButton::STARS.iter().map(|b| b.key()).collect();
        assert_eq!(
            stars,
            [
                RatingKey::Stars(0),
                RatingKey::Stars(1),
                RatingKey::Stars(2),
                RatingKey::Stars(3),
                RatingKey::Stars(4),
                RatingKey::Stars(5),
                RatingKey::Reject,
            ]
        );
        let labels: Vec<RatingKey> = RatingButton::LABELS.iter().map(|b| b.key()).collect();
        assert_eq!(
            labels,
            [
                RatingKey::Label("Red"),
                RatingKey::Label("Yellow"),
                RatingKey::Label("Green"),
                RatingKey::Label("Blue"),
                RatingKey::Label("Purple"),
                RatingKey::ClearLabel,
            ]
        );
        // The same keys as Alt+0–5 and Ctrl+0–5 in the contact sheet, and `X` for Reject.
        use egui::Key as K;
        let digits = [K::Num0, K::Num1, K::Num2, K::Num3, K::Num4, K::Num5];
        for (button, key) in RatingButton::STARS.iter().zip(digits) {
            assert_eq!(
                Some(button.key()),
                crate::rating_key_action(key, false, false, true)
            );
        }
        assert_eq!(
            Some(RatingButton::Reject.key()),
            crate::rating_key_action(K::X, false, false, false)
        );
        let label_keys = [K::Num1, K::Num2, K::Num3, K::Num4, K::Num5, K::Num0];
        for (button, key) in RatingButton::LABELS.iter().zip(label_keys) {
            assert_eq!(
                Some(button.key()),
                crate::rating_key_action(key, true, false, false)
            );
        }
    }

    /// The button of the shown rating is selected; nothing else in its row (T124, U10c-Q1).
    #[test]
    fn the_buttons_of_the_shown_rating_are_selected() {
        let rating = |stars: Option<i8>, label: Option<&str>| Rating {
            stars,
            label: label.map(str::to_string),
        };
        let selected = |value: Option<&Rating>, buttons: &[RatingButton]| -> Vec<RatingButton> {
            buttons
                .iter()
                .copied()
                .filter(|button| button.selected(value))
                .collect()
        };
        let cases = [
            (None, RatingButton::NoStars, RatingButton::NoLabel),
            (
                Some(rating(None, None)),
                RatingButton::NoStars,
                RatingButton::NoLabel,
            ),
            (
                Some(rating(Some(3), Some("Red"))),
                RatingButton::Stars(3),
                RatingButton::Label("Red"),
            ),
            (
                Some(rating(Some(5), Some("Purple"))),
                RatingButton::Stars(5),
                RatingButton::Label("Purple"),
            ),
            (
                Some(rating(Some(-1), Some("Blue"))),
                RatingButton::Reject,
                RatingButton::Label("Blue"),
            ),
            (
                Some(rating(Some(1), None)),
                RatingButton::Stars(1),
                RatingButton::NoLabel,
            ),
        ];
        for (value, star, label) in cases {
            assert_eq!(
                selected(value.as_ref(), &RatingButton::STARS),
                [star],
                "{value:?}"
            );
            assert_eq!(
                selected(value.as_ref(), &RatingButton::LABELS),
                [label],
                "{value:?}"
            );
        }
        // A label RawStride does not offer selects no label button.
        let orange = rating(Some(2), Some("Orange"));
        assert!(selected(Some(&orange), &RatingButton::LABELS).is_empty());
    }

    /// The buttons' texts are the glossary's terms (§2), as the Image menu has them (T124).
    #[test]
    fn the_rating_button_texts() {
        let texts = |buttons: &[RatingButton]| -> Vec<String> {
            buttons
                .iter()
                .map(|button| editor_text(&EditorText::RatingButton(*button)))
                .collect()
        };
        assert_eq!(
            texts(&RatingButton::STARS),
            [
                "No stars", "1 star", "2 stars", "3 stars", "4 stars", "5 stars", "Reject"
            ]
        );
        assert_eq!(
            texts(&RatingButton::LABELS),
            ["Red", "Yellow", "Green", "Blue", "Purple", "No label"]
        );
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

    // --- "Apply to All Marked" (ADR 0017 §2.3.1, U12c, T126) ---

    fn clear(property: &str) -> Edit {
        Edit::Clear {
            property: property.to_string(),
            field: None,
        }
    }

    fn batch_of(form: &EditorForm) -> BatchEdits {
        match form.batch_edits() {
            Ok(batch) => batch,
            Err(errors) => panic!("no field error expected: {errors:?}"),
        }
    }

    #[test]
    fn the_copyright_management_fields_are_the_five_of_adr_0007_section_8() {
        let fields: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|field| field.copyright_management())
            .collect();
        assert_eq!(
            fields,
            [
                EditorField::Creator,
                EditorField::Copyright,
                EditorField::Credit,
                EditorField::UsageTerms,
                EditorField::WebStatement,
            ]
        );
    }

    #[test]
    fn an_unchanged_field_is_written_to_no_image() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Sol"));
        insert(&mut metadata, "photoshop:City", text("Lund"));
        let form = form_with(metadata);

        let batch = batch_of(&form);

        assert!(batch.this_image.is_empty());
        assert!(batch.others.is_empty());
        assert_eq!(batch.field_count(), 0);
    }

    #[test]
    fn a_changed_text_lang_alt_list_and_check_box_go_to_every_image() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Sol"));
        insert(&mut metadata, "photoshop:City", text("Lund"));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Title, "Kväll");
        type_into(&mut form, EditorField::Headline, "Hav");
        type_into(&mut form, EditorField::Keywords, "sol, hav");
        form.set_checked(EditorField::Copyrighted, true);

        let batch = batch_of(&form);

        let expected = vec![
            set("dc:title", lang_alt(&[("x-default", "Kväll")])),
            set("photoshop:Headline", text("Hav")),
            set("dc:subject", list(&["sol", "hav"])),
            set("xmpRights:Marked", text("True")),
        ];
        assert_eq!(batch.this_image, expected);
        assert_eq!(
            batch.others, expected,
            "City is unchanged and written nowhere"
        );
        assert_eq!(
            batch.changed,
            [
                EditorField::Title,
                EditorField::Headline,
                EditorField::Keywords,
                EditorField::Copyrighted,
            ]
        );
        assert!(batch.emptied.is_empty());
        assert!(batch.emptied_here_only.is_empty());
    }

    #[test]
    fn a_chosen_conflict_value_goes_only_to_the_windows_image() {
        let mut form = form_with(metadata_with_city_conflict());
        assert!(form.choose(0, 1));
        type_into(&mut form, EditorField::Headline, "Hav");

        let batch = batch_of(&form);

        assert_eq!(
            batch.this_image,
            [
                set("photoshop:City", text("Malmö")),
                set("photoshop:Headline", text("Hav")),
            ]
        );
        assert_eq!(batch.others, [set("photoshop:Headline", text("Hav"))]);
        assert_eq!(batch.changed, [EditorField::Headline]);

        // A text typed over the choice is a changed field: every image gets it, and it decides
        // the conflict there too.
        type_into(&mut form, EditorField::City, "Lund");
        let batch = batch_of(&form);
        assert_eq!(
            batch.others,
            [
                set("photoshop:Headline", text("Hav")),
                set("photoshop:City", text("Lund")),
            ]
        );
        assert!(
            batch
                .this_image
                .contains(&set("photoshop:City", text("Lund")))
        );
    }

    #[test]
    fn an_emptied_copyright_field_is_emptied_only_on_the_windows_image() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:creator", list(&["Anna"]));
        insert(
            &mut metadata,
            "dc:rights",
            lang_alt(&[("x-default", "© Anna")]),
        );
        insert(&mut metadata, "photoshop:Credit", text("Anna AB"));
        insert(
            &mut metadata,
            "xmpRights:UsageTerms",
            lang_alt(&[("x-default", "Ask first")]),
        );
        insert(
            &mut metadata,
            "xmpRights:WebStatement",
            text("https://example.org/rights"),
        );
        insert(&mut metadata, "photoshop:City", text("Lund"));
        let mut form = form_with(metadata);
        for field in [
            EditorField::Creator,
            EditorField::Copyright,
            EditorField::Credit,
            EditorField::UsageTerms,
            EditorField::WebStatement,
            EditorField::City,
        ] {
            type_into(&mut form, field, "");
        }

        let batch = batch_of(&form);

        assert_eq!(
            batch.this_image,
            [
                clear("dc:creator"),
                clear("dc:rights"),
                clear("photoshop:Credit"),
                clear("xmpRights:UsageTerms"),
                clear("xmpRights:WebStatement"),
                clear("photoshop:City"),
            ]
        );
        // An emptied other field (City) is emptied on every image.
        assert_eq!(batch.others, [clear("photoshop:City")]);
        assert_eq!(batch.emptied, [EditorField::City]);
        assert_eq!(
            batch.emptied_here_only,
            [
                EditorField::Creator,
                EditorField::Copyright,
                EditorField::Credit,
                EditorField::UsageTerms,
                EditorField::WebStatement,
            ]
        );
        assert!(batch.changed.is_empty());
        assert_eq!(batch.field_count(), 6);
    }

    #[test]
    fn a_copyright_field_given_a_value_goes_to_every_image() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Credit, "Anna AB");

        let batch = batch_of(&form);

        assert_eq!(batch.others, [set("photoshop:Credit", text("Anna AB"))]);
        assert_eq!(batch.changed, [EditorField::Credit]);
    }

    #[test]
    fn field_errors_give_no_batch() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Headline, "Hav");
        type_into(&mut form, EditorField::DateCreated, "igår");

        assert_eq!(
            form.batch_edits(),
            Err(vec![(EditorField::DateCreated, date_error_text())])
        );
    }

    #[test]
    fn the_confirmation_names_the_changed_emptied_and_copyright_only_fields_and_the_skipped_files()
    {
        let batch = BatchEdits {
            this_image: Vec::new(),
            others: Vec::new(),
            changed: vec![EditorField::Headline, EditorField::City],
            emptied: vec![EditorField::Country],
            emptied_here_only: vec![EditorField::Copyright, EditorField::Credit],
        };
        assert_eq!(
            apply_confirmation(&batch, 4, 1),
            [
                "Write 5 changed fields to 4 images?",
                "Changed: Headline, City",
                "Emptied: Country",
                "Emptied only on this image: Copyright Notice, Credit Line",
                "Skipped: 1 file",
            ]
        );

        // Only the lines with something to name.
        let batch = BatchEdits {
            changed: vec![EditorField::Headline],
            emptied: Vec::new(),
            emptied_here_only: Vec::new(),
            ..batch
        };
        assert_eq!(
            apply_confirmation(&batch, 2, 0),
            ["Write 1 changed field to 2 images?", "Changed: Headline"]
        );
        let batch = BatchEdits {
            changed: Vec::new(),
            emptied: vec![EditorField::City],
            ..batch
        };
        assert_eq!(
            apply_confirmation(&batch, 3, 2),
            [
                "Write 1 changed field to 3 images?",
                "Emptied: City",
                "Skipped: 2 files",
            ]
        );
    }

    #[test]
    fn every_apply_to_all_marked_text_in_singular_and_plural() {
        let cases = [
            (EditorText::MarkedImages { images: 1 }, "1 image marked"),
            (EditorText::MarkedImages { images: 4 }, "4 images marked"),
            (EditorText::ApplyToMarked { images: 1 }, "Apply to 1 Marked"),
            (EditorText::ApplyToMarked { images: 4 }, "Apply to 4 Marked"),
            (EditorText::ApplyTitle, "Apply to All Marked"),
            (
                EditorText::ApplyQuestion {
                    fields: 1,
                    images: 1,
                },
                "Write 1 changed field to 1 image?",
            ),
            (
                EditorText::ApplyQuestion {
                    fields: 1,
                    images: 4,
                },
                "Write 1 changed field to 4 images?",
            ),
            (
                EditorText::ApplyQuestion {
                    fields: 3,
                    images: 1,
                },
                "Write 3 changed fields to 1 image?",
            ),
            (
                EditorText::ApplyQuestion {
                    fields: 3,
                    images: 4,
                },
                "Write 3 changed fields to 4 images?",
            ),
            (
                EditorText::ChangedFields(vec![EditorField::Headline]),
                "Changed: Headline",
            ),
            (
                EditorText::EmptiedFields(vec![EditorField::City, EditorField::Country]),
                "Emptied: City, Country",
            ),
            (
                EditorText::EmptiedHereOnly(vec![EditorField::Copyright]),
                "Emptied only on this image: Copyright Notice",
            ),
            (EditorText::SkippedFiles { files: 1 }, "Skipped: 1 file"),
            (EditorText::SkippedFiles { files: 3 }, "Skipped: 3 files"),
            (EditorText::Write, "Write"),
            (
                EditorText::Writing {
                    current: 2,
                    total: 4,
                },
                "Writing 2 of 4 …",
            ),
            (
                EditorText::BatchWritten {
                    written: 1,
                    total: 1,
                },
                "Written to 1 of 1 image.",
            ),
            (
                EditorText::BatchWritten {
                    written: 3,
                    total: 4,
                },
                "Written to 3 of 4 images.",
            ),
            (
                EditorText::ConflictKept {
                    file: "b.jpg".into(),
                    property: "photoshop:City".into(),
                },
                "Unresolved conflict kept: b.jpg (City)",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }
}
