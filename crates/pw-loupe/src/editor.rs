//! The editor's form (ADR 0007 I10k, owner decision D12): the fields of one image, the choices of
//! its unresolved conflicts, the edits they give, the warnings and the texts of the write notices.
//! Pure functions, no egui; T105 draws the window.
//!
//! - **Fields** (D12, ADR 0017 U10, T123, T128): 47 fields in a fixed order and five sections,
//!   [`EditorField::ALL`], [`EditorSection`]. Each is a property of the table, or (the address,
//!   Email, Phone, Web URL) a field of `Iptc4xmpCore:CreatorContactInfo`. The value kind (text,
//!   Lang Alt, list, date) comes from the property table. Copyrighted is a check box (`True` or
//!   `False`, U10-Q3); Urgency, Category and Supplemental Categories are read-only.
//! - **Choice lists** (U10d, T128): the PLUS release statuses, Minor Model Age Disclosure, Data
//!   Mining and Digital Source Type hold one value of their vocabulary
//!   ([`pw_metadata::vocabulary`]: the code for PLUS, the full URI for Digital Source Type),
//!   chosen from [`EditorForm::choice_entries`]. A value of the file outside the vocabulary is an
//!   extra entry, kept until another entry is chosen.
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
use pw_metadata::template_file::TemplateFile;
use pw_metadata::validate::{Warning, validate};
use pw_metadata::value::{Conflict, ConflictRule, LangAlt, StructValue, Value};
use pw_metadata::variables::{self, VariableError};
use pw_metadata::vocabulary;
use pw_metadata::write_plan::WriteNotice;
use pw_metadata::{MIN_EXIFTOOL_VERSION, Rating};

use crate::editor_layout::{EditorItem, EditorLayout};
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
    /// The PLUS release fields, Data Mining and Digital Source Type (U10d, T128).
    Licence,
    /// The rating and the label (T124, U10c-Q5): no fields of the form; its buttons
    /// ([`RatingButton`]) rate through the rating path, never through the form's edits.
    RatingLabel,
}

impl EditorSection {
    /// Every section in the order of the form.
    pub const ALL: [EditorSection; 6] = [
        EditorSection::Content,
        EditorSection::CreatorRights,
        EditorSection::TimePlace,
        EditorSection::Assignment,
        EditorSection::Licence,
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
    ModelReleaseStatus,
    ModelReleaseId,
    MinorModelAge,
    PropertyReleaseStatus,
    PropertyReleaseId,
    DataMining,
    DigitalSourceType,
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
    /// Every field in the order of the form (D12, ADR 0017 U10-Q1, U10-Q4, U10d).
    pub const ALL: [EditorField; 47] = [
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
        EditorField::ModelReleaseStatus,
        EditorField::ModelReleaseId,
        EditorField::MinorModelAge,
        EditorField::PropertyReleaseStatus,
        EditorField::PropertyReleaseId,
        EditorField::DataMining,
        EditorField::DigitalSourceType,
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
            EditorField::ModelReleaseStatus
            | EditorField::ModelReleaseId
            | EditorField::MinorModelAge
            | EditorField::PropertyReleaseStatus
            | EditorField::PropertyReleaseId
            | EditorField::DataMining
            | EditorField::DigitalSourceType => EditorSection::Licence,
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
            EditorField::ModelReleaseStatus => vocabulary::MODEL_RELEASE_STATUS,
            EditorField::ModelReleaseId => "plus:ModelReleaseID",
            EditorField::MinorModelAge => vocabulary::MINOR_MODEL_AGE_DISCLOSURE,
            EditorField::PropertyReleaseStatus => vocabulary::PROPERTY_RELEASE_STATUS,
            EditorField::PropertyReleaseId => "plus:PropertyReleaseID",
            EditorField::DataMining => vocabulary::DATA_MINING,
            EditorField::DigitalSourceType => vocabulary::DIGITAL_SOURCE_TYPE,
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

    /// The values of a choice-list field (U10d-Q3, T128) in the vocabulary's order: the codes
    /// for the PLUS fields, the full URIs (the current codes) for Digital Source Type; `None`
    /// for every other field.
    pub fn choice_list(self) -> Option<&'static [&'static str]> {
        match self.struct_field() {
            None => vocabulary::choices(self.property()),
            Some(_) => None,
        }
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

    /// True for a field with a "▼" of recent and own values (ADR 0017 U12a-Q1, T141): every
    /// editable one-line text field and list field, the contact fields included; not the
    /// multi-line fields, Date Created, the check box, the choice lists, nor the deprecated
    /// read-only fields. (The structure lists' sub-fields are not `EditorField`s.)
    pub fn has_values(self) -> bool {
        self.editable()
            && !self.multiline()
            && !self.check_box()
            && self.choice_list().is_none()
            && self.kind() != FieldKind::Date
    }

    /// True for a list field (Keywords, Person Shown, Creator …): one line with its entries
    /// separated by commas (T108).
    pub fn is_list(self) -> bool {
        self.kind() == FieldKind::List
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

/// The error of a text whose variables cannot be read (ADR 0017 U13-Q9, T289): an unknown name, a
/// `{` that is not closed or a `}` that is not opened; a field's or a structure list's sub-field's
/// error, found before anything is written. `NoCaptureTime` is found only when an image is written
/// (T288), but has a text too.
pub(crate) fn variable_error_text(error: &VariableError) -> String {
    match error {
        VariableError::Unknown(name) => format!(
            "{{{name}}} is not a variable. Variables: {{date}} {{year}} {{month}} {{day}} {{time}} \
             {{make}} {{model}} {{name}} {{folder}}. Write {{{{ or }}}} for a brace."
        ),
        VariableError::Unclosed(_) => "A { is not closed. Write {{ for a brace.".to_string(),
        VariableError::UnexpectedClose => "A } is not opened. Write }} for a brace.".to_string(),
        VariableError::NoCaptureTime(name) => {
            format!("The image has no capture time for {{{name}}}.")
        }
    }
}

/// `value` with every text escaped ([`variables::escape`], T289): a text, every language of a
/// Lang Alt, every list entry and every field of a structure at any depth; dates and numbers are
/// not texts and stay as they are. For a value the user did not type (a chosen conflict value, the
/// unchanged parts of a structure list), so T288's filling gives it back exactly.
fn escaped_value(value: &Value) -> Value {
    match value {
        Value::Text(text) => Value::Text(variables::escape(text)),
        Value::LangAlt(lang_alt) => Value::LangAlt(LangAlt::from_entries(
            lang_alt
                .entries()
                .iter()
                .map(|(language, text)| (language.clone(), variables::escape(text)))
                .collect(),
        )),
        Value::List(entries) => Value::List(
            entries
                .iter()
                .map(|entry| variables::escape(entry))
                .collect(),
        ),
        Value::Date(_) | Value::Number(_) => value.clone(),
        Value::Struct(structure) => Value::Struct(escaped_struct(structure)),
        Value::StructList(entries) => {
            Value::StructList(entries.iter().map(escaped_struct).collect())
        }
    }
}

/// [`escaped_value`] on every field of a structure.
fn escaped_struct(structure: &StructValue) -> StructValue {
    StructValue {
        fields: structure
            .fields
            .iter()
            .map(|(id, value)| (id.clone(), escaped_value(value)))
            .collect(),
    }
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

/// One entry of a choice-list field's drop-down (U10d-Q3, T128), from
/// [`EditorForm::choice_entries`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceEntry {
    /// The value the entry gives the field: empty for the empty entry, else the code (PLUS) or
    /// the full URI (Digital Source Type), or the file's value as read.
    pub value: String,
    /// The entry's text: empty for the empty entry, the vocabulary's English label, or
    /// "Unknown value: …" for a value outside the vocabulary.
    pub text: String,
    /// The entry's tooltip: the full URI of a vocabulary value; `None` for the empty entry and an
    /// unknown value.
    pub tooltip: Option<String>,
}

/// Location Created (ADR 0017 U11a, T136): where the image was taken.
const LOCATION_CREATED: &str = "Iptc4xmpExt:LocationCreated";
/// Location Shown (ADR 0017 U11a, T136): what the image shows.
const LOCATION_SHOWN: &str = "Iptc4xmpExt:LocationShown";
/// The sub-field of a location that is a list of URIs, one line separated by commas (U10-Q2).
const LOCATION_ID: &str = "Iptc4xmpExt:LocationId";
/// The shown sub-fields of a location in the order of the form (U11a-Q2).
const LOCATION_FIELDS: [&str; 7] = [
    "Iptc4xmpExt:Sublocation",
    "Iptc4xmpExt:City",
    "Iptc4xmpExt:ProvinceState",
    "Iptc4xmpExt:CountryName",
    "Iptc4xmpExt:CountryCode",
    "Iptc4xmpExt:WorldRegion",
    LOCATION_ID,
];
/// The field after which the location lists are drawn and their edits made (U11a-Q2).
const LOCATIONS_AFTER: EditorField = EditorField::CountryCode;

/// Genre (ADR 0017 U11, T139): a list of CV terms.
const GENRE: &str = "Iptc4xmpExt:Genre";
/// Image Creator (PLUS, T139).
const IMAGE_CREATOR: &str = "plus:ImageCreator";
/// The name of an Image Creator entry.
const IMAGE_CREATOR_NAME: &str = "plus:ImageCreatorName";
/// Copyright Owner (PLUS, T139).
const COPYRIGHT_OWNER: &str = "plus:CopyrightOwner";
/// Licensor (PLUS, T139).
const LICENSOR: &str = "plus:Licensor";
/// Image Supplier (PLUS, T139).
const IMAGE_SUPPLIER: &str = "plus:ImageSupplier";
/// Person Shown in the Image with Details (ADR 0017 U11, T143).
const PERSON_DETAILS: &str = "Iptc4xmpExt:PersonInImageWDetails";
/// The name of a Person Shown with Details entry (a Lang Alt).
const PERSON_NAME: &str = "Iptc4xmpExt:PersonName";
/// Product Shown in the Image (T143).
const PRODUCT: &str = "Iptc4xmpExt:ProductInImage";
/// Artwork or Object in the Image (T143).
const ARTWORK: &str = "Iptc4xmpExt:ArtworkOrObject";
/// Contributor (T143).
const CONTRIBUTOR: &str = "Iptc4xmpExt:Contributor";
/// CV-Term About Image (T143): CV terms as Genre's.
const ABOUT_CV_TERM: &str = "Iptc4xmpExt:AboutCvTerm";
/// Image Registry Entry (T143).
const REGISTRY: &str = "Iptc4xmpExt:RegistryId";

/// One kind of list of structures in the form (U11b-Q1, T139): the list's XMP id, its shown
/// sub-fields in the order of the form and where it is drawn. Its most entries come from
/// [`properties::max_entries`]. Location Created and Location Shown (T136) are two of them.
#[derive(Debug, PartialEq, Eq)]
struct ListSpec {
    /// The XMP id of the list.
    property: &'static str,
    /// The XMP ids of the shown sub-fields in the order of the form; names from
    /// [`property_label`].
    fields: &'static [&'static str],
    /// How many of `fields` are shown open in each entry; the rest are under a collapsed line:
    /// "Address" for a Licensor (U11b-Q3), "More" for an Artwork or Object (U11c-Q3, T143).
    open: usize,
    /// The section the list is drawn in.
    section: EditorSection,
    /// The field after which the list is drawn; `None` at the start of its section. For a list
    /// with `after_list`, the anchor list's field (it gives the place of the edits).
    after: Option<EditorField>,
    /// The list right after which this list is drawn, instead of right after the field `after`
    /// (CV-Term About Image after Genre, U11c-Q2, T143); `None` for every other list.
    after_list: Option<&'static str>,
}

/// The sub-fields of a CV term as Genre shows them (U11b-Q2).
const GENRE_FIELDS: [&str; 3] = [
    "Iptc4xmpExt:CvTermName",
    "Iptc4xmpExt:CvTermId",
    "Iptc4xmpExt:CvId",
];
/// The sub-fields of an Image Creator entry.
const IMAGE_CREATOR_FIELDS: [&str; 2] = [IMAGE_CREATOR_NAME, "plus:ImageCreatorID"];
/// The sub-fields of a Copyright Owner entry.
const COPYRIGHT_OWNER_FIELDS: [&str; 2] = ["plus:CopyrightOwnerName", "plus:CopyrightOwnerID"];
/// The sub-fields of a Licensor entry (U11b-Q3): eight open, then the six of the address.
const LICENSOR_FIELDS: [&str; 14] = [
    "plus:LicensorName",
    "plus:LicensorID",
    "plus:LicensorEmail",
    "plus:LicensorURL",
    "plus:LicensorTelephone1",
    vocabulary::LICENSOR_TELEPHONE_TYPE_1,
    "plus:LicensorTelephone2",
    vocabulary::LICENSOR_TELEPHONE_TYPE_2,
    "plus:LicensorStreetAddress",
    "plus:LicensorExtendedAddress",
    "plus:LicensorCity",
    "plus:LicensorRegion",
    "plus:LicensorPostalCode",
    "plus:LicensorCountry",
];
/// How many of a Licensor entry's sub-fields are shown open (U11b-Q3).
const LICENSOR_OPEN: usize = 8;
/// The sub-fields of an Image Supplier entry.
const IMAGE_SUPPLIER_FIELDS: [&str; 2] = ["plus:ImageSupplierName", "plus:ImageSupplierID"];
/// The sub-fields of a Person Shown with Details entry (U11c-Q1). Its Characteristics (a list of
/// CV terms) are not shown and kept unchanged (U11c-Q5).
const PERSON_DETAILS_FIELDS: [&str; 3] = [
    PERSON_NAME,
    "Iptc4xmpExt:PersonId",
    "Iptc4xmpExt:PersonDescription",
];
/// The sub-fields of a Product Shown entry (U11c-Q1).
const PRODUCT_FIELDS: [&str; 4] = [
    "Iptc4xmpExt:ProductName",
    "Iptc4xmpExt:ProductGTIN",
    "Iptc4xmpExt:ProductId",
    "Iptc4xmpExt:ProductDescription",
];
/// The 17 sub-fields of an Artwork or Object entry (U11c-Q3): six open, then the eleven under
/// "More".
const ARTWORK_FIELDS: [&str; 17] = [
    "Iptc4xmpExt:AOTitle",
    "Iptc4xmpExt:AOCreator",
    "Iptc4xmpExt:AODateCreated",
    "Iptc4xmpExt:AOSource",
    "Iptc4xmpExt:AOSourceInvNo",
    "Iptc4xmpExt:AOCopyrightNotice",
    "Iptc4xmpExt:AOCircaDateCreated",
    "Iptc4xmpExt:AOStylePeriod",
    "Iptc4xmpExt:AOContentDescription",
    "Iptc4xmpExt:AOContributionDescription",
    "Iptc4xmpExt:AOPhysicalDescription",
    "Iptc4xmpExt:AOCreatorId",
    "Iptc4xmpExt:AOCurrentCopyrightOwnerName",
    "Iptc4xmpExt:AOCurrentCopyrightOwnerId",
    "Iptc4xmpExt:AOCurrentLicensorName",
    "Iptc4xmpExt:AOCurrentLicensorId",
    "Iptc4xmpExt:AOSourceInvURL",
];
/// How many of an Artwork or Object entry's sub-fields are shown open (U11c-Q3).
const ARTWORK_OPEN: usize = 6;
/// The sub-fields of a Contributor entry (U11c-Q1).
const CONTRIBUTOR_FIELDS: [&str; 3] = ["Iptc4xmpExt:Name", "Iptc4xmpExt:Role", "xmp:Identifier"];
/// The sub-fields of an Image Registry Entry (U11c-Q1), IPTC's names.
const REGISTRY_FIELDS: [&str; 3] = [
    "Iptc4xmpExt:RegOrgId",
    "Iptc4xmpExt:RegItemId",
    "Iptc4xmpExt:RegEntryRole",
];

/// Every structure list of the form in form order (U11b-Q1, U11c-Q2): Person Shown with Details,
/// Product Shown and Artwork or Object after Person Shown (T143), Contributor at the end of
/// Creator and Rights (T143), the locations at the end of Time and Place (U11a-Q2), Genre and
/// CV-Term About Image after Scene Code, Image Registry Entry at the end of Assignment (T143), and
/// the four PLUS lists at the start of Licence (PLUS), before the release fields (T139, point 2).
/// Lists with the same place give their edits in this order.
static LIST_SPECS: [ListSpec; 13] = [
    ListSpec {
        property: PERSON_DETAILS,
        fields: &PERSON_DETAILS_FIELDS,
        open: PERSON_DETAILS_FIELDS.len(),
        section: EditorSection::Content,
        after: Some(EditorField::PersonShown),
        after_list: None,
    },
    ListSpec {
        property: PRODUCT,
        fields: &PRODUCT_FIELDS,
        open: PRODUCT_FIELDS.len(),
        section: EditorSection::Content,
        after: Some(EditorField::PersonShown),
        after_list: None,
    },
    ListSpec {
        property: ARTWORK,
        fields: &ARTWORK_FIELDS,
        open: ARTWORK_OPEN,
        section: EditorSection::Content,
        after: Some(EditorField::PersonShown),
        after_list: None,
    },
    ListSpec {
        property: CONTRIBUTOR,
        fields: &CONTRIBUTOR_FIELDS,
        open: CONTRIBUTOR_FIELDS.len(),
        section: EditorSection::CreatorRights,
        after: Some(EditorField::Copyrighted),
        after_list: None,
    },
    ListSpec {
        property: LOCATION_CREATED,
        fields: &LOCATION_FIELDS,
        open: LOCATION_FIELDS.len(),
        section: EditorSection::TimePlace,
        after: Some(LOCATIONS_AFTER),
        after_list: None,
    },
    ListSpec {
        property: LOCATION_SHOWN,
        fields: &LOCATION_FIELDS,
        open: LOCATION_FIELDS.len(),
        section: EditorSection::TimePlace,
        after: Some(LOCATIONS_AFTER),
        after_list: None,
    },
    ListSpec {
        property: GENRE,
        fields: &GENRE_FIELDS,
        open: GENRE_FIELDS.len(),
        section: EditorSection::Assignment,
        after: Some(EditorField::SceneCode),
        after_list: None,
    },
    ListSpec {
        property: ABOUT_CV_TERM,
        fields: &GENRE_FIELDS,
        open: GENRE_FIELDS.len(),
        section: EditorSection::Assignment,
        after: Some(EditorField::SceneCode),
        after_list: Some(GENRE),
    },
    ListSpec {
        property: REGISTRY,
        fields: &REGISTRY_FIELDS,
        open: REGISTRY_FIELDS.len(),
        section: EditorSection::Assignment,
        after: Some(EditorField::SupplementalCategories),
        after_list: None,
    },
    ListSpec {
        property: IMAGE_CREATOR,
        fields: &IMAGE_CREATOR_FIELDS,
        open: IMAGE_CREATOR_FIELDS.len(),
        section: EditorSection::Licence,
        after: None,
        after_list: None,
    },
    ListSpec {
        property: COPYRIGHT_OWNER,
        fields: &COPYRIGHT_OWNER_FIELDS,
        open: COPYRIGHT_OWNER_FIELDS.len(),
        section: EditorSection::Licence,
        after: None,
        after_list: None,
    },
    ListSpec {
        property: LICENSOR,
        fields: &LICENSOR_FIELDS,
        open: LICENSOR_OPEN,
        section: EditorSection::Licence,
        after: None,
        after_list: None,
    },
    ListSpec {
        property: IMAGE_SUPPLIER,
        fields: &IMAGE_SUPPLIER_FIELDS,
        open: IMAGE_SUPPLIER_FIELDS.len(),
        section: EditorSection::Licence,
        after: None,
        after_list: None,
    },
];

/// The spec of the structure list `list`.
fn list_spec(list: &str) -> Option<&'static ListSpec> {
    LIST_SPECS.iter().find(|spec| spec.property == list)
}

/// The section the structure list `list` is drawn in; `None` for a property that is not one of
/// [`EditorForm::ALL_LISTS`] (the metadata window's layout, U10b-Q5).
pub(crate) fn list_section(list: &str) -> Option<EditorSection> {
    list_spec(list).map(|spec| spec.section)
}

/// The place of a list among the fields: the index in [`EditorField::ALL`] of the first field
/// drawn after it. Its edits come just before that field's (U11a-Q2).
fn list_slot(spec: &ListSpec) -> usize {
    let index = |field: EditorField| {
        EditorField::ALL
            .iter()
            .position(|f| *f == field)
            .unwrap_or(EditorField::ALL.len())
    };
    match spec.after {
        Some(field) => index(field) + 1,
        None => EditorField::ALL
            .iter()
            .position(|field| field.section() == spec.section)
            .unwrap_or(EditorField::ALL.len()),
    }
}

/// True for a copyright-management list (ADR 0007 §8, U11b-Q5): Image Creator, Copyright Owner
/// and Licensor. "Apply to All Marked" empties such a list on the window's image only.
fn list_copyright_management(list: &str) -> bool {
    matches!(list, IMAGE_CREATOR | COPYRIGHT_OWNER | LICENSOR)
}

/// How a sub-field of a structure list is edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubFieldKind {
    /// One line of text.
    Text,
    /// A list on one line, split at commas (Location Identifier, U10-Q2).
    List,
    /// A Lang Alt: the line edits `x-default` and keeps the other languages (ADR 0007 §8).
    LangAlt,
    /// A choice from a vocabulary (the telephone types of a Licensor entry, U11b-Q3).
    Choice,
    /// An IPTC date, checked as the form's Date Created (an Artwork or Object's Date Created,
    /// U11c-Q4, T143).
    Date,
}

/// The kind of the sub-field `field` of the list `list`, from the vocabulary and the property
/// table (a text when it is not found).
fn sub_field_kind(list: &str, field: &str) -> SubFieldKind {
    if vocabulary::choices(field).is_some() {
        return SubFieldKind::Choice;
    }
    let def = properties::property(list)
        .and_then(|def| match def.kind {
            ValueKind::Struct(name) => properties::struct_def(name),
            _ => None,
        })
        .and_then(|def| def.fields.iter().find(|def| def.xmp_id == field));
    match def.map(|def| (def.kind, def.occurrence)) {
        Some((_, Occurrence::Multi)) => SubFieldKind::List,
        Some((ValueKind::LangAlt, _)) => SubFieldKind::LangAlt,
        Some((ValueKind::Date, _)) => SubFieldKind::Date,
        _ => SubFieldKind::Text,
    }
}

/// One entry of a structure list (U11a, T136; U11b-Q1, T139): the texts of its shown
/// sub-fields and the structure it started from, which keeps the sub-fields the form does not
/// show and the other languages of a Lang Alt (ADR 0007 §8).
#[derive(Clone, Debug, PartialEq, Eq)]
struct ListEntry {
    /// The list's spec.
    spec: &'static ListSpec,
    /// The texts in the order of the spec's fields (a choice's code as it is stored).
    texts: Vec<String>,
    /// The entry as read, pasted or copied; empty for an added entry.
    base: StructValue,
    /// True for an entry put in by "Paste All Fields" (ADR 0017 U13-Q9 amended, T289's review
    /// fix): its shown text, list and Lang Alt sub-fields count as typed
    /// ([`ListEntry::typed`]). Kept until the form is made anew from a read.
    pasted: bool,
}

impl ListEntry {
    /// The entry of the structure `base`: each shown sub-field's text as the form shows it; not
    /// pasted.
    fn of(spec: &'static ListSpec, base: &StructValue) -> ListEntry {
        ListEntry {
            spec,
            texts: spec
                .fields
                .iter()
                .map(|field| sub_field_text(base, field))
                .collect(),
            base: base.clone(),
            pasted: false,
        }
    }

    /// The entry of the pasted structure `base` ([`EditorForm::paste_shown`]).
    fn pasted(spec: &'static ListSpec, base: &StructValue) -> ListEntry {
        ListEntry {
            pasted: true,
            ..ListEntry::of(spec, base)
        }
    }

    /// True when the sub-field `field` with the text `text` counts as typed by the user whether
    /// or not it differs from the base (T289's review fix): a shown text, list or Lang Alt
    /// sub-field of a pasted entry whose text is not empty. It is then checked
    /// ([`ListEntry::error`]) and written as it is ([`ListEntry::written`]), so T288 fills its
    /// variables.
    fn typed(&self, field: &str, text: &str) -> bool {
        self.pasted
            && !text.trim().is_empty()
            && matches!(
                sub_field_kind(self.spec.property, field),
                SubFieldKind::Text | SubFieldKind::List | SubFieldKind::LangAlt
            )
    }

    /// The error of the sub-field `field`, only for a text that was changed (an unchanged text
    /// never has one, except one that counts as typed in a pasted entry, [`ListEntry::typed`])
    /// and is not empty: a date sub-field that is not an IPTC date gives the error of the form's
    /// Date Created (U11c-Q4, T143); a text, list or Lang Alt sub-field that
    /// [`variables::check`] refuses gives [`variable_error_text`] (U13-Q9, T289). `None` for a
    /// choice and for every other text.
    fn error(&self, field: &str) -> Option<String> {
        let index = self.spec.fields.iter().position(|id| *id == field)?;
        let text = self.texts.get(index)?;
        let unchanged = sub_field_text(&self.base, field) == *text && !self.typed(field, text);
        if unchanged || text.trim().is_empty() {
            return None;
        }
        match sub_field_kind(self.spec.property, field) {
            SubFieldKind::Date => IptcDate::parse_xmp(text.trim())
                .is_err()
                .then(date_error_text),
            SubFieldKind::Text | SubFieldKind::List | SubFieldKind::LangAlt => {
                variables::check(text)
                    .err()
                    .map(|error| variable_error_text(&error))
            }
            SubFieldKind::Choice => None,
        }
    }

    /// True when a shown sub-field of the entry has an error ([`ListEntry::error`]).
    fn has_error(&self) -> bool {
        self.spec
            .fields
            .iter()
            .any(|field| self.error(field).is_some())
    }

    /// The structure the entry writes: its base with each shown sub-field from its text. A text
    /// equal to the base's keeps the base's value; an empty text removes the sub-field; a list
    /// sub-field is split at commas; a Lang Alt gets its `x-default` set and keeps its other
    /// languages; a date is read as an IPTC date (a date with an error keeps the base's value,
    /// and the list gives no edit while it has one, T143). `None` when no sub-field is left
    /// (U11a-Q3).
    fn value(&self) -> Option<StructValue> {
        self.build(false)
    }

    /// The structure the entry writes, as [`ListEntry::value`], with every text of its base the
    /// user did not change escaped ([`escaped_struct`], T289): an unchanged sub-field, the other
    /// languages of a Lang Alt sub-field and the sub-fields the form does not show, at any depth.
    /// A sub-field the user changed keeps its text as typed, so T288 fills its variables; so does
    /// one that counts as typed in a pasted entry ([`ListEntry::typed`]: for a Lang Alt its
    /// `x-default`, the other languages escaped). [`ListEntry::value`] is not changed by a paste,
    /// so Copy All Fields copies a pasted entry's texts as shown.
    fn written(&self) -> Option<StructValue> {
        self.build(true)
    }

    /// [`ListEntry::value`], or with `escaped` [`ListEntry::written`].
    fn build(&self, escaped: bool) -> Option<StructValue> {
        let mut fields = if escaped {
            escaped_struct(&self.base).fields
        } else {
            self.base.fields.clone()
        };
        for (field, text) in self.spec.fields.iter().zip(&self.texts) {
            let position = fields.iter().position(|(id, _)| id == field);
            let old = position
                .and_then(|index| fields.get(index))
                .map(|(_, value)| value);
            // Compared with the base as read, not with its escaped texts. When escaping, a pasted
            // sub-field that counts as typed is written from its text, not escaped.
            let unchanged = position.is_some()
                && sub_field_text(&self.base, field) == *text
                && !(escaped && self.typed(field, text));
            let value = if text.trim().is_empty() {
                None
            } else {
                match sub_field_kind(self.spec.property, field) {
                    SubFieldKind::List => {
                        let entries = list_entries(text);
                        (!entries.is_empty()).then_some(Value::List(entries))
                    }
                    SubFieldKind::LangAlt => {
                        let mut lang_alt = match old {
                            Some(Value::LangAlt(lang_alt)) => lang_alt.clone(),
                            _ => LangAlt::default(),
                        };
                        // Not empty (checked above), so `set_default` accepts it.
                        lang_alt
                            .set_default(text)
                            .ok()
                            .map(|()| Value::LangAlt(lang_alt))
                    }
                    SubFieldKind::Date => match IptcDate::parse_xmp(text.trim()) {
                        Ok(date) => Some(Value::Date(date)),
                        Err(_) => old.cloned(),
                    },
                    SubFieldKind::Text | SubFieldKind::Choice => Some(Value::Text(text.clone())),
                }
            };
            match (position, value) {
                (Some(_), Some(_)) if unchanged => {}
                (Some(index), Some(value)) => {
                    if let Some(slot) = fields.get_mut(index) {
                        slot.1 = value;
                    }
                }
                (Some(index), None) => {
                    fields.remove(index);
                }
                (None, Some(value)) => fields.push(((*field).to_string(), value)),
                (None, None) => {}
            }
        }
        (!fields.is_empty()).then_some(StructValue { fields })
    }
}

/// The text of the sub-field `field` of `base` as the form shows it; empty when it has none.
fn sub_field_text(base: &StructValue, field: &str) -> String {
    base.fields
        .iter()
        .find(|(id, _)| id == field)
        .map(|(_, value)| form_text(value))
        .unwrap_or_default()
}

/// The entries of a structure list's value; `None` for a value that is not a structure.
fn list_entries_of(spec: &'static ListSpec, value: &Value) -> Option<Vec<ListEntry>> {
    match value {
        Value::StructList(list) => Some(list.iter().map(|s| ListEntry::of(spec, s)).collect()),
        Value::Struct(fields) => Some(vec![ListEntry::of(spec, fields)]),
        _ => None,
    }
}

/// The structures `entries` write, the empty ones dropped (U11a-Q3).
fn list_values(entries: &[ListEntry]) -> Vec<StructValue> {
    entries.iter().filter_map(ListEntry::value).collect()
}

/// One structure list of the form (U11a, T136; U11b-Q1, T139).
#[derive(Clone, Debug, PartialEq, Eq)]
struct ListState {
    /// The list's spec.
    spec: &'static ListSpec,
    /// The entries as the user has them.
    entries: Vec<ListEntry>,
    /// The entries `from_read` gave.
    original: Vec<ListEntry>,
    /// The text of the read value; shown when the list is read-only.
    read_text: String,
    /// True when the read value is not a structure, so the form cannot edit it.
    unsupported: bool,
}

impl ListState {
    /// The XMP id of the list.
    fn property(&self) -> &'static str {
        self.spec.property
    }
}

/// The fields copied by "Copy All Fields" (ADR 0017 §2.3.1, U12b-Q1, T125): the source file's
/// name and the shown text of every editable field (the check box's `True` or empty), and the
/// entries of each structure list that is not read-only and has an entry (U11a-Q5, T136;
/// U11b-Q5, T139). Kept by the program until the next copy; not saved and not on the Windows
/// clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedFields {
    file: String,
    texts: Vec<(EditorField, String)>,
    /// Each structure list with the structures it writes (the empty entries dropped), in form
    /// order; a list without an entry is left out, as it is not pasted (U12b-Q2). The name is
    /// T136's, from when the location lists were the only structure lists.
    locations: Vec<(&'static str, Vec<StructValue>)>,
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

    /// How many copied fields are not empty: the fields a paste puts in (U12b-Q2), each structure
    /// list with an entry counted as one field (U11a-Q5, U11b-Q5).
    pub fn count(&self) -> usize {
        self.texts
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .count()
            + self
                .locations
                .iter()
                .filter(|(_, entries)| !entries.is_empty())
                .count()
    }
}

/// What "Apply to {n} Marked" does to one list field (ADR 0017 U12d-Q1–U12d-Q3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListChange {
    pub field: EditorField,
    /// Added on every marked image that lacks them.
    pub adds: Vec<String>,
    /// Removed on every marked image that has them.
    pub removes: Vec<String>,
    /// Removed on the window's image only (Creator).
    pub removes_here: Vec<String>,
}

/// What "Apply to All Marked" writes (ADR 0017 §2.3.1, U12c-Q3, T126; U12d, T180), from
/// [`EditorForm::batch_edits`].
#[derive(Clone, Debug, PartialEq)]
pub struct BatchEdits {
    /// The edits of the window's image: [`EditorForm::edits`], the chosen conflict values
    /// included (a changed list field as its whole list).
    pub this_image: Vec<Edit>,
    /// The edits of every other marked image: only the changed fields, never a chosen conflict
    /// value, and no `Clear` of a copyright-management field; a changed list field (Keywords,
    /// Creator …) only as an `Edit::Entries` of its added and removed entries (U12d-Q1), never
    /// a removal of Creator entries (U12d-Q2).
    pub others: Vec<Edit>,
    /// The changed fields given a value, in form order; never a list field (see `list_changes`).
    pub changed: Vec<EditorField>,
    /// The emptied fields that are emptied on every image, in form order; never a list field.
    pub emptied: Vec<EditorField>,
    /// The emptied copyright-management fields, emptied on the window's image only; never a
    /// list field (an emptied Creator is in `list_changes`, its entries in `removes_here`).
    pub emptied_here_only: Vec<EditorField>,
    /// What each changed list field adds and removes (U12d-Q1–U12d-Q3), in form order: one per
    /// list field whose text differs from its start text and that adds or removes an entry
    /// (only a new order or new spaces give none). For Creator `removes` is always empty and
    /// its removed entries are in `removes_here`; for every other field `removes_here` is
    /// always empty.
    pub list_changes: Vec<ListChange>,
}

/// The confirmation's lines of one list field (U12d-Q3): what it adds and removes on every image
/// (none when neither), then what it removes on the window's image only.
fn list_change_lines(change: &ListChange) -> Vec<String> {
    let field = change.field;
    let mut texts = Vec::new();
    match (change.adds.is_empty(), change.removes.is_empty()) {
        (false, true) => texts.push(EditorText::ListAdds {
            field,
            entries: change.adds.clone(),
        }),
        (true, false) => texts.push(EditorText::ListRemoves {
            field,
            entries: change.removes.clone(),
        }),
        (false, false) => texts.push(EditorText::ListAddsRemoves {
            field,
            adds: change.adds.clone(),
            removes: change.removes.clone(),
        }),
        (true, true) => {}
    }
    if !change.removes_here.is_empty() {
        texts.push(EditorText::ListRemovesHere {
            field,
            entries: change.removes_here.clone(),
        });
    }
    texts.iter().map(editor_text).collect()
}

/// The structure lists of a batch by what happens to them (U11a-Q5, U11b-Q5), each in form
/// order.
struct BatchLists {
    /// Given entries on every image.
    changed: Vec<&'static str>,
    /// Emptied on every image.
    emptied: Vec<&'static str>,
    /// Copyright-management lists emptied on the window's image only.
    emptied_here_only: Vec<&'static str>,
}

impl BatchEdits {
    /// How many fields were changed: given a value, emptied, or emptied on this image only; each
    /// changed structure list counts as one field (U11a-Q5, U11b-Q5), and so does each list
    /// field's [`ListChange`] (U12d, T180).
    pub fn field_count(&self) -> usize {
        let lists = self.lists();
        self.changed.len()
            + self.emptied.len()
            + self.emptied_here_only.len()
            + self.list_changes.len()
            + lists.changed.len()
            + lists.emptied.len()
            + lists.emptied_here_only.len()
    }

    /// The structure lists of the batch (U11a-Q5, T136; U11b-Q5, T139): those given entries and
    /// those emptied, each from its edit in `others`, and the copyright-management lists emptied
    /// on the window's image only: a `Clear` in `this_image` that `others` does not have.
    fn lists(&self) -> BatchLists {
        let mut lists = BatchLists {
            changed: Vec::new(),
            emptied: Vec::new(),
            emptied_here_only: Vec::new(),
        };
        let has = |edits: &[Edit], list: &str, clear: bool| {
            edits.iter().any(|edit| match edit {
                Edit::Set { property, .. } => !clear && property == list,
                Edit::Clear { property, .. } => clear && property == list,
                // `Entries` is never sent for a structure list (`ALL_LISTS`); the list fields are
                // counted through `list_changes`.
                Edit::Entries { .. } => false,
            })
        };
        for list in EditorForm::ALL_LISTS {
            if has(&self.others, list, false) {
                lists.changed.push(list);
            }
            if has(&self.others, list, true) {
                lists.emptied.push(list);
            } else if list_copyright_management(list) && has(&self.this_image, list, true) {
                lists.emptied_here_only.push(list);
            }
        }
        lists
    }
}

/// The lines of the confirmation of "Apply to All Marked" (U12c-Q4, T126): the question for
/// `images` images, then the changed fields by name (the structure lists among them, U11a-Q5,
/// U11b-Q5), what each list field adds and removes (U12d-Q3, T180), the emptied and
/// copyright-only fields by name and the `skipped` marked files without metadata support, each
/// line only when it has something to name.
pub fn apply_confirmation(batch: &BatchEdits, images: usize, skipped: usize) -> Vec<String> {
    let mut lines = vec![editor_text(&EditorText::ApplyQuestion {
        fields: batch.field_count(),
        images,
    })];
    let lists = batch.lists();
    if !batch.changed.is_empty() || !lists.changed.is_empty() {
        lines.push(editor_text(&EditorText::Changed {
            fields: batch.changed.clone(),
            lists: lists.changed,
        }));
    }
    for change in &batch.list_changes {
        lines.extend(list_change_lines(change));
    }
    if !batch.emptied.is_empty() || !lists.emptied.is_empty() {
        lines.push(editor_text(&EditorText::Emptied {
            fields: batch.emptied.clone(),
            lists: lists.emptied,
        }));
    }
    if !batch.emptied_here_only.is_empty() || !lists.emptied_here_only.is_empty() {
        lines.push(editor_text(&EditorText::EmptiedHere {
            fields: batch.emptied_here_only.clone(),
            lists: lists.emptied_here_only,
        }));
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
    /// The structure lists (U11a, T136; U11b-Q1, T139; U11c-Q1, T143) in the order of
    /// [`EditorForm::ALL_LISTS`].
    lists: Vec<ListState>,
}

impl EditorForm {
    /// The XMP id of Location Created (ADR 0017 U11a, T136).
    pub const LOCATION_CREATED: &'static str = LOCATION_CREATED;
    /// The XMP id of Location Shown (ADR 0017 U11a, T136).
    pub const LOCATION_SHOWN: &'static str = LOCATION_SHOWN;
    /// The XMP id of Image Creator (ADR 0017 U11, T139): "Copy from Creator" is drawn under it.
    pub const IMAGE_CREATOR: &'static str = IMAGE_CREATOR;
    /// The location lists in the order of the form, drawn in Time and Place after Country Code
    /// (U11a-Q2).
    pub const LOCATION_LISTS: [&'static str; 2] = [LOCATION_CREATED, LOCATION_SHOWN];
    /// The XMP ids of a location entry's seven shown sub-fields in the order of the form
    /// (U11a-Q2); their names come from `property_label`. Location Identifier is one line with
    /// its URIs separated by commas.
    pub const LOCATION_FIELDS: [&'static str; 7] = LOCATION_FIELDS;
    /// The structure lists of T136 and T139 in form order (U11b-Q1): Location Created, Location
    /// Shown, Genre, Image Creator, Copyright Owner, Licensor, Image Supplier. Every list,
    /// T143's included, is in [`EditorForm::ALL_LISTS`].
    pub const STRUCTURE_LISTS: [&'static str; 7] = [
        LOCATION_CREATED,
        LOCATION_SHOWN,
        GENRE,
        IMAGE_CREATOR,
        COPYRIGHT_OWNER,
        LICENSOR,
        IMAGE_SUPPLIER,
    ];
    /// The XMP id of Person Shown in the Image with Details (ADR 0017 U11c, T143): "Copy from
    /// Person Shown" is drawn under it.
    pub const PERSON_DETAILS: &'static str = PERSON_DETAILS;
    /// Every structure list in form order (U11c-Q2, T143): T139's seven
    /// ([`EditorForm::STRUCTURE_LISTS`]) and Person Shown with Details, Product Shown, Artwork or
    /// Object, Contributor, CV-Term About Image and Image Registry Entry, each in its place.
    pub const ALL_LISTS: [&'static str; 13] = [
        PERSON_DETAILS,
        PRODUCT,
        ARTWORK,
        CONTRIBUTOR,
        LOCATION_CREATED,
        LOCATION_SHOWN,
        GENRE,
        ABOUT_CV_TERM,
        REGISTRY,
        IMAGE_CREATOR,
        COPYRIGHT_OWNER,
        LICENSOR,
        IMAGE_SUPPLIER,
    ];

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
        let lists = LIST_SPECS
            .iter()
            .map(|spec| {
                let value = metadata.get(spec.property).map(|stored| &stored.value);
                let entries = match value {
                    Some(value) => list_entries_of(spec, value),
                    None => Some(Vec::new()),
                };
                ListState {
                    spec,
                    entries: entries.clone().unwrap_or_default(),
                    original: entries.clone().unwrap_or_default(),
                    read_text: value.map(value_text).unwrap_or_default(),
                    unsupported: entries.is_none(),
                }
            })
            .collect();
        EditorForm {
            fields,
            conflicts,
            chosen,
            lists,
        }
    }

    /// The structure lists drawn in `section` right after the field `after`, or at the start of
    /// the section for `None` (U11a-Q2, T139 point 2), in form order. A list drawn after another
    /// list is not among them ([`EditorForm::lists_after_list`]).
    pub fn lists_at(section: EditorSection, after: Option<EditorField>) -> Vec<&'static str> {
        LIST_SPECS
            .iter()
            .filter(|spec| {
                spec.section == section && spec.after == after && spec.after_list.is_none()
            })
            .map(|spec| spec.property)
            .collect()
    }

    /// The structure lists drawn right after the list `list` (CV-Term About Image after Genre,
    /// U11c-Q2, T143), in form order; empty for every other list.
    pub fn lists_after_list(list: &str) -> Vec<&'static str> {
        LIST_SPECS
            .iter()
            .filter(|spec| spec.after_list == Some(list))
            .map(|spec| spec.property)
            .collect()
    }

    /// The sub-fields of the list `list` shown open in each entry, in the order of the form;
    /// empty for another property.
    pub fn list_fields(list: &str) -> &'static [&'static str] {
        list_spec(list)
            .and_then(|spec| spec.fields.get(..spec.open))
            .unwrap_or(&[])
    }

    /// The sub-fields of the list `list` under the collapsed "Address" line of each entry (a
    /// Licensor's address, U11b-Q3); empty for every other list.
    pub fn list_address_fields(list: &str) -> &'static [&'static str] {
        if list != LICENSOR {
            return &[];
        }
        Self::collapsed_fields(list)
    }

    /// The sub-fields of the list `list` under the collapsed "More" line of each entry (the
    /// eleven of an Artwork or Object, U11c-Q3, T143); empty for every other list.
    pub fn list_more_fields(list: &str) -> &'static [&'static str] {
        if list == LICENSOR {
            return &[];
        }
        Self::collapsed_fields(list)
    }

    /// The sub-fields of the list `list` after those shown open.
    fn collapsed_fields(list: &str) -> &'static [&'static str] {
        list_spec(list)
            .and_then(|spec| spec.fields.get(spec.open..))
            .unwrap_or(&[])
    }

    /// The most entries the list `list` may hold (IPTC's limits, ADR 0007 §8): 3 for Image
    /// Creator, Copyright Owner and Licensor, 1 for Image Supplier; `None` without a limit.
    pub fn max_entries(list: &str) -> Option<usize> {
        properties::max_entries(list)
    }

    fn list(&self, list: &str) -> Option<&ListState> {
        self.lists.iter().find(|state| state.property() == list)
    }

    /// The list `list` when it can be edited: not a value the form cannot show and no open
    /// conflict that is not decided yet.
    fn list_editable(&mut self, list: &str) -> Option<&mut ListState> {
        if self.list_read_only(list).is_some() {
            return None;
        }
        self.lists.iter_mut().find(|state| state.property() == list)
    }

    /// The text of the structure list `list` (an XMP id of [`EditorForm::ALL_LISTS`]) when
    /// it is shown read-only (U11a, point 6 of T136): while an open conflict on it is not decided,
    /// or when its value is not a structure. `None` when its entries can be edited.
    pub fn list_read_only(&self, list: &str) -> Option<&str> {
        let state = self.list(list)?;
        let undecided = self
            .conflicts
            .iter()
            .zip(&self.chosen)
            .any(|(conflict, chosen)| conflict.property == list && chosen.is_none());
        (state.unsupported || undecided).then_some(state.read_text.as_str())
    }

    /// How many entries the list `list` has, the empty ones included.
    pub fn list_count(&self, list: &str) -> usize {
        self.list(list).map_or(0, |state| state.entries.len())
    }

    /// The text of the sub-field `field` of entry `entry` of the list `list` (a choice's code as
    /// it is stored); empty when there is no such entry or field.
    pub fn list_text(&self, list: &str, entry: usize, field: &str) -> &str {
        let Some(state) = self.list(list) else {
            return "";
        };
        let index = state.spec.fields.iter().position(|id| *id == field);
        state
            .entries
            .get(entry)
            .zip(index)
            .and_then(|(entry, index)| entry.texts.get(index))
            .map_or("", String::as_str)
    }

    /// The error of the sub-field `field` of entry `entry` of the list `list`, shown while the
    /// changed text is wrong; [`EditorForm::edits`] then writes nothing: an Artwork or Object's
    /// Date Created that is not an IPTC date gets the error of the form's Date Created (U11c-Q4,
    /// T143); a text, list or Lang Alt sub-field with an unknown variable or a brace that is not
    /// closed gets [`variable_error_text`] (U13-Q9, T289). `None` for a right or unchanged text, a
    /// choice, a read-only list or an entry that does not exist.
    pub fn list_error(&self, list: &str, entry: usize, field: &str) -> Option<String> {
        if self.list_read_only(list).is_some() {
            return None;
        }
        self.list(list)?.entries.get(entry)?.error(field)
    }

    /// True when an entry of an editable structure list has a sub-field with an error.
    fn has_list_error(&self) -> bool {
        !self.lists_with_errors().is_empty()
    }

    /// The XMP ids of the editable structure lists with an entry that has a sub-field with an
    /// error ([`EditorForm::list_error`]), each once, in form order (U11c-Q9, T181).
    pub fn lists_with_errors(&self) -> Vec<&'static str> {
        self.lists
            .iter()
            .filter(|state| {
                self.list_read_only(state.property()).is_none()
                    && state.entries.iter().any(ListEntry::has_error)
            })
            .map(ListState::property)
            .collect()
    }

    /// The text of a sub-field, to be edited; `None` when there is no such entry or field, the
    /// sub-field is a choice ([`EditorForm::set_list_choice`]), or the list is read-only.
    pub fn list_text_mut(&mut self, list: &str, entry: usize, field: &str) -> Option<&mut String> {
        if sub_field_kind(list, field) == SubFieldKind::Choice {
            return None;
        }
        let state = self.list_editable(list)?;
        let index = state.spec.fields.iter().position(|id| *id == field)?;
        state.entries.get_mut(entry)?.texts.get_mut(index)
    }

    /// The entries of the drop-down of the choice sub-field `field` of entry `entry` of the list
    /// `list` (a Licensor's telephone types, U11b-Q3), as [`EditorForm::choice_entries`]: the
    /// empty entry, the vocabulary's values with their labels and full URIs, then, when it is not
    /// one of them, the entry's value as read and its present value, each "Unknown value: {uri}"
    /// (U10d-Q4). Empty for a sub-field that is not a choice or an entry that does not exist.
    pub fn list_choice_entries(&self, list: &str, entry: usize, field: &str) -> Vec<ChoiceEntry> {
        let Some(values) = vocabulary::choices(field) else {
            return Vec::new();
        };
        let Some(list_entry) = self.list(list).and_then(|state| state.entries.get(entry)) else {
            return Vec::new();
        };
        let mut entries = vec![ChoiceEntry {
            value: String::new(),
            text: String::new(),
            tooltip: None,
        }];
        for value in values {
            entries.push(ChoiceEntry {
                value: (*value).to_string(),
                text: choice_label(value).unwrap_or_default().to_string(),
                tooltip: vocabulary::full_uri(field, value),
            });
        }
        let original = sub_field_text(&list_entry.base, field);
        let present = self.list_text(list, entry, field).to_string();
        for extra in [original, present] {
            if extra.is_empty() || entries.iter().any(|entry| entry.value == extra) {
                continue;
            }
            entries.push(ChoiceEntry {
                text: editor_text(&EditorText::UnknownValue {
                    value: extra.clone(),
                }),
                value: extra,
                tooltip: None,
            });
        }
        entries
    }

    /// Chooses `value` (empty for the empty entry) for the choice sub-field `field` of entry
    /// `entry` of the list `list`; nothing for another sub-field or a read-only list.
    pub fn set_list_choice(&mut self, list: &str, entry: usize, field: &str, value: &str) {
        if sub_field_kind(list, field) != SubFieldKind::Choice {
            return;
        }
        let Some(state) = self.list_editable(list) else {
            return;
        };
        let Some(index) = state.spec.fields.iter().position(|id| *id == field) else {
            return;
        };
        if let Some(text) = state
            .entries
            .get_mut(entry)
            .and_then(|entry| entry.texts.get_mut(index))
        {
            value.clone_into(text);
        }
    }

    /// Whether "Add" can be used on the list `list`: it is not read-only and has fewer entries
    /// than its limit ([`EditorForm::max_entries`], T139 point 2).
    pub fn can_add_entry(&self, list: &str) -> bool {
        self.list_read_only(list).is_none()
            && self.list(list).is_some()
            && Self::max_entries(list).is_none_or(|max| self.list_count(list) < max)
    }

    /// "Add" (U11a-Q2): a new empty entry at the end of the list; nothing when
    /// [`EditorForm::can_add_entry`] is false. An entry left empty is not written (U11a-Q3).
    pub fn add_entry(&mut self, list: &str) {
        if !self.can_add_entry(list) {
            return;
        }
        if let Some(state) = self.list_editable(list) {
            state
                .entries
                .push(ListEntry::of(state.spec, &StructValue::default()));
        }
    }

    /// "Remove" (U11a-Q2): removes entry `entry`; the others keep their order. Nothing when the
    /// list is read-only or has no such entry.
    pub fn remove_entry(&mut self, list: &str, entry: usize) {
        if let Some(state) = self.list_editable(list)
            && entry < state.entries.len()
        {
            state.entries.remove(entry);
        }
    }

    /// [`EditorForm::list_read_only`] for a location list (T136's name).
    pub fn location_read_only(&self, list: &str) -> Option<&str> {
        self.list_read_only(list)
    }

    /// [`EditorForm::list_count`] for a location list (T136's name).
    pub fn location_count(&self, list: &str) -> usize {
        self.list_count(list)
    }

    /// [`EditorForm::list_text`] for a location list (T136's name).
    pub fn location_text(&self, list: &str, entry: usize, field: &str) -> &str {
        self.list_text(list, entry, field)
    }

    /// [`EditorForm::list_text_mut`] for a location list (T136's name).
    pub fn location_text_mut(
        &mut self,
        list: &str,
        entry: usize,
        field: &str,
    ) -> Option<&mut String> {
        self.list_text_mut(list, entry, field)
    }

    /// [`EditorForm::add_entry`] for a location list (T136's name).
    pub fn add_location(&mut self, list: &str) {
        self.add_entry(list);
    }

    /// [`EditorForm::remove_entry`] for a location list (T136's name).
    pub fn remove_location(&mut self, list: &str, entry: usize) {
        self.remove_entry(list, entry);
    }

    /// The first entry of Location Created that writes something, as it would be written.
    fn first_location_created(&self) -> Option<StructValue> {
        self.list(LOCATION_CREATED)?
            .entries
            .iter()
            .find_map(ListEntry::value)
    }

    /// Whether "Copy from Location Created" can be used (U11a-Q4): Location Created has an entry
    /// that is not empty and Location Shown is not read-only.
    pub fn can_copy_location_created(&self) -> bool {
        self.list_read_only(LOCATION_SHOWN).is_none() && self.first_location_created().is_some()
    }

    /// "Copy from Location Created" (U11a-Q4): adds one new Location Shown entry with the fields
    /// of the first Location Created entry that is not empty; no existing entry changes. Nothing
    /// when [`EditorForm::can_copy_location_created`] is false.
    pub fn copy_location_created(&mut self) {
        let Some(first) = self.first_location_created() else {
            return;
        };
        if let Some(state) = self.list_editable(LOCATION_SHOWN) {
            state.entries.push(ListEntry::of(state.spec, &first));
        }
    }

    /// The names of Creator (`dc:creator`) as the form has them, unsaved edits included.
    fn creator_names(&self) -> Vec<String> {
        list_entries(self.text(EditorField::Creator))
    }

    /// Whether "Copy from Creator" can be used (U11b-Q4): Creator has a name and Image Creator
    /// is not read-only and has room (fewer than 3 entries).
    pub fn can_copy_creator(&self) -> bool {
        !self.creator_names().is_empty() && self.can_add_entry(IMAGE_CREATOR)
    }

    /// "Copy from Creator" (U11b-Q4, ADR 0007 §8 "synchronisation is offered"): adds one Image
    /// Creator entry per Creator name that is not already an Image Creator Name, in Creator's
    /// order, up to the limit; never changes an existing entry. Nothing when
    /// [`EditorForm::can_copy_creator`] is false. Nothing is synchronised automatically.
    pub fn copy_creator(&mut self) {
        if !self.can_copy_creator() {
            return;
        }
        let names = self.creator_names();
        let limit = Self::max_entries(IMAGE_CREATOR);
        let Some(state) = self.list_editable(IMAGE_CREATOR) else {
            return;
        };
        let index = state
            .spec
            .fields
            .iter()
            .position(|id| *id == IMAGE_CREATOR_NAME);
        for name in names {
            if limit.is_some_and(|max| state.entries.len() >= max) {
                break;
            }
            let present = state.entries.iter().any(|entry| {
                index
                    .and_then(|index| entry.texts.get(index))
                    .is_some_and(|text| text.trim() == name)
            });
            if present {
                continue;
            }
            let base = StructValue {
                fields: vec![(IMAGE_CREATOR_NAME.to_string(), Value::Text(name))],
            };
            state.entries.push(ListEntry::of(state.spec, &base));
        }
    }

    /// The names of Person Shown in the Image (`Iptc4xmpExt:PersonInImage`) as the form has
    /// them, unsaved edits included.
    fn person_shown_names(&self) -> Vec<String> {
        list_entries(self.text(EditorField::PersonShown))
    }

    /// Whether "Copy from Person Shown" can be used (U11c-Q6, T143): Person Shown has a name and
    /// Person Shown with Details is not read-only.
    pub fn can_copy_person_shown(&self) -> bool {
        !self.person_shown_names().is_empty() && self.can_add_entry(PERSON_DETAILS)
    }

    /// "Copy from Person Shown" (U11c-Q6, T143), as "Copy from Creator": adds one Person Shown
    /// with Details entry per Person Shown name that is not already a Name there (its `x-default`
    /// text), in Person Shown's order; never changes an existing entry. Nothing when
    /// [`EditorForm::can_copy_person_shown`] is false. Nothing is synchronised automatically.
    pub fn copy_person_shown(&mut self) {
        if !self.can_copy_person_shown() {
            return;
        }
        let names = self.person_shown_names();
        let Some(state) = self.list_editable(PERSON_DETAILS) else {
            return;
        };
        let index = state.spec.fields.iter().position(|id| *id == PERSON_NAME);
        for name in names {
            let present = state.entries.iter().any(|entry| {
                index
                    .and_then(|index| entry.texts.get(index))
                    .is_some_and(|text| text.trim() == name)
            });
            if present {
                continue;
            }
            let base = StructValue {
                fields: vec![(
                    PERSON_NAME.to_string(),
                    Value::LangAlt(LangAlt::from_entries(vec![(X_DEFAULT.to_string(), name)])),
                )],
            };
            state.entries.push(ListEntry::of(state.spec, &base));
        }
    }

    /// The entries a list is compared with: the chosen conflict value's, else those the read
    /// gave.
    fn list_start(&self, state: &ListState) -> Vec<StructValue> {
        match self
            .chosen_value(state.property())
            .and_then(|value| list_entries_of(state.spec, value))
        {
            Some(entries) => list_values(&entries),
            None => list_values(&state.original),
        }
    }

    /// The edit of a structure list (U11a-Q3, point 6 of T136): `None` when it is read-only or
    /// writes the same structures as at the start, or (T143, T289) while a sub-field has an error
    /// (a wrong date or a text with a variable that cannot be read); a `Clear` when no entry
    /// writes anything; else a `Set` of the whole list, the texts the user did not change escaped
    /// and those typed or pasted (a pasted entry's shown text, list and Lang Alt sub-fields) as
    /// they are ([`ListEntry::written`], T289).
    fn list_edit(&self, state: &ListState) -> Option<Edit> {
        if self.list_read_only(state.property()).is_some() {
            return None;
        }
        if state.entries.iter().any(ListEntry::has_error) {
            return None;
        }
        let values = list_values(&state.entries);
        if values == self.list_start(state) {
            return None;
        }
        let property = state.property().to_string();
        Some(if values.is_empty() {
            Edit::Clear {
                property,
                field: None,
            }
        } else {
            Edit::Set {
                property,
                field: None,
                value: Value::StructList(
                    state
                        .entries
                        .iter()
                        .filter_map(ListEntry::written)
                        .collect(),
                ),
            }
        })
    }

    /// The edits of the structure lists placed just before the field with index `slot` in
    /// [`EditorField::ALL`] (or after the last field for its length), in form order.
    fn list_edits_at(&self, slot: usize) -> Vec<Edit> {
        self.lists
            .iter()
            .filter(|state| list_slot(state.spec) == slot)
            .filter_map(|state| self.list_edit(state))
            .collect()
    }

    fn state(&self, field: EditorField) -> Option<&FieldState> {
        self.fields.iter().find(|state| state.field == field)
    }

    /// The text of `field` as the user has it.
    pub fn text(&self, field: EditorField) -> &str {
        self.state(field).map_or("", |state| state.text.as_str())
    }

    /// The text of `field`, to be edited; `None` for a check box, a choice list
    /// ([`EditorForm::set_choice`]) and a read-only field.
    pub fn text_mut(&mut self, field: EditorField) -> Option<&mut String> {
        if field.check_box() || field.choice_list().is_some() || !field.editable() {
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

    /// True when `field` shows a "▼" (U12a-Q1, T141): a field with values
    /// ([`EditorField::has_values`]) that is not shown read-only.
    pub fn values_shown(&self, field: EditorField) -> bool {
        field.has_values() && self.read_only_text(field).is_none()
    }

    /// The values of `field`'s present text for "Add to My Values" (U12a-Q3): each entry on its
    /// own for a list field, else the text; nothing when it is empty or only spaces.
    pub fn present_values(&self, field: EditorField) -> Vec<String> {
        text_values(field, self.text(field))
    }

    /// A value chosen in the "▼" list of `field` (U12a-Q4, T141): it replaces a text field's
    /// text; for a list field it is added as a new entry at the end when it is not already there.
    /// The field is changed as after typing; nothing is written until the window's write. Nothing
    /// for a field without a "▼" or shown read-only.
    pub fn use_value(&mut self, field: EditorField, value: &str) {
        if !self.values_shown(field) {
            return;
        }
        let Some(text) = self.text_mut(field) else {
            return;
        };
        if !field.is_list() {
            *text = value.to_string();
            return;
        }
        let value = value.trim();
        let mut entries = list_entries(text);
        if value.is_empty() || entries.iter().any(|entry| entry == value) {
            return;
        }
        entries.push(value.to_string());
        *text = entries.join(LIST_SEPARATOR);
    }

    /// What a write of the form puts in the fields with a "▼" (U12a-Q2, T141), for their recent
    /// values once the write has succeeded: each changed field (its text differs from its start
    /// text) with its values; for a list field only the entries the write adds (not in its start
    /// text), each on its own, so a keyword added to a long list is not pushed out of the recent
    /// values by the entries the image already had. An emptied field gives nothing. The values
    /// are the texts as typed, with their variables (`{date} {name}`, not the filled values,
    /// ADR 0017 U13-Q9, T289), so a variable can be chosen again.
    pub fn written_values(&self) -> Vec<(EditorField, Vec<String>)> {
        self.fields
            .iter()
            .filter(|state| self.values_shown(state.field))
            .filter(|state| state.text != self.start_text(state))
            .map(|state| {
                let mut values = text_values(state.field, &state.text);
                if state.field.is_list() {
                    let start = list_entries(&self.start_text(state));
                    values.retain(|value| !start.contains(value));
                }
                (state.field, values)
            })
            .filter(|(_, values)| !values.is_empty())
            .collect()
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

    /// The entries of the choice-list field `field` (U10d-Q3, U10d-Q4, T128), in order: the empty
    /// entry, the vocabulary's values, then, when it is not one of them, the file's value and the
    /// field's present value (each once, not empty). Such an extra entry is a retired Digital
    /// Source Type with its own label, or else "Unknown value: {value}". Empty for another field.
    pub fn choice_entries(&self, field: EditorField) -> Vec<ChoiceEntry> {
        let (Some(values), Some(state)) = (field.choice_list(), self.state(field)) else {
            return Vec::new();
        };
        let property = field.property();
        let mut entries = vec![ChoiceEntry {
            value: String::new(),
            text: String::new(),
            tooltip: None,
        }];
        for value in values {
            entries.push(ChoiceEntry {
                value: (*value).to_string(),
                text: choice_label(value).unwrap_or_default().to_string(),
                tooltip: vocabulary::full_uri(property, value),
            });
        }
        for extra in [&state.original, &state.text] {
            if extra.is_empty() || entries.iter().any(|entry| entry.value == *extra) {
                continue;
            }
            let retired = field == EditorField::DigitalSourceType
                && vocabulary::is_retired_digital_source_type(extra);
            let text = match choice_label(extra) {
                Some(label) if retired => label.to_string(),
                _ => editor_text(&EditorText::UnknownValue {
                    value: extra.clone(),
                }),
            };
            let tooltip = if retired {
                vocabulary::full_uri(property, extra)
            } else {
                None
            };
            entries.push(ChoiceEntry {
                value: extra.clone(),
                text,
                tooltip,
            });
        }
        entries
    }

    /// Chooses the entry with `value` of the choice-list field `field` (an empty `value` is the
    /// empty entry); nothing for another field.
    pub fn set_choice(&mut self, field: EditorField, value: &str) {
        if field.choice_list().is_none() {
            return;
        }
        if let Some(state) = self.fields.iter_mut().find(|state| state.field == field) {
            value.clone_into(&mut state.text);
        }
    }

    /// "Copy All Fields" (U12b-Q1, T125): the text of every editable field as the form shows it,
    /// unsaved edits included, and the Copyrighted check box. Not the read-only fields (the
    /// deprecated ones, a check box with another value), the conflict choices or the errors. A
    /// Lang Alt gives its text, which is the `x-default` text the window edits (Q7).
    pub fn copy(&self, file: &str) -> CopiedFields {
        self.copy_shown(file, &EditorLayout::default())
    }

    /// "Copy All Fields" with the metadata window's layout (ADR 0017 U10b-Q4, T277): as
    /// [`EditorForm::copy`], but only the fields and structure lists whose item `layout` shows
    /// ([`EditorLayout::is_shown`]), so [`CopiedFields::count`] counts only those.
    pub fn copy_shown(&self, file: &str, layout: &EditorLayout) -> CopiedFields {
        let texts = self
            .fields
            .iter()
            .filter(|state| {
                state.field.editable()
                    && self.read_only_text(state.field).is_none()
                    && layout.is_shown(EditorItem::of_field(state.field))
            })
            .map(|state| (state.field, state.text.clone()))
            .collect();
        // Each structure list with an entry (U11a-Q5, U11b-Q5); an empty one is not pasted.
        let locations = self
            .lists
            .iter()
            .filter(|state| {
                self.list_read_only(state.property()).is_none()
                    && layout.is_shown(EditorItem::List(state.property()))
            })
            .map(|state| (state.property(), list_values(&state.entries)))
            .filter(|(_, values)| !values.is_empty())
            .collect();
        CopiedFields {
            file: file.to_string(),
            texts,
            locations,
        }
    }

    /// "Paste All Fields" (U12b-Q2, T125): every copied text that is not empty goes into its field
    /// as if typed, an unsaved edit; the other fields keep their text, and a field shown read-only
    /// here is left as it is. The edits, errors and conflict decisions then follow as for typing.
    pub fn paste(&mut self, copied: &CopiedFields) {
        self.paste_shown(copied, &EditorLayout::default());
    }

    /// "Paste All Fields" with the metadata window's layout (ADR 0017 U10b-Q4, T277): as
    /// [`EditorForm::paste`], but a field or structure list whose item `layout` does not show
    /// ([`EditorLayout::is_shown`]) is left as it is, also when `copied` holds it (the layout may
    /// have changed between the copy and the paste). A hidden item cannot be edited in the
    /// window either, so its text stays its start text and it never has a field error (errors
    /// come only from changed text, and a conflict choice moves the start text along); the
    /// edits ([`EditorForm::batch_edits`]) need no layout. An item shown again after the copy was
    /// not copied, so it is not pasted.
    pub fn paste_shown(&mut self, copied: &CopiedFields, layout: &EditorLayout) {
        for (field, text) in &copied.texts {
            if text.is_empty() || !layout.is_shown(EditorItem::of_field(*field)) {
                continue;
            }
            if field.check_box() {
                self.set_checked(*field, text == CHECKED);
            } else if field.choice_list().is_some() {
                self.set_choice(*field, text);
            } else if self.read_only_text(*field).is_none()
                && let Some(target) = self.text_mut(*field)
            {
                target.clone_from(text);
            }
        }
        // A copied structure list with entries replaces the whole list (U11a-Q5, U11b-Q5); an
        // empty one is not pasted (U12b-Q2), and a list shown read-only here is left as it is.
        // Its entries are pasted ones (ADR 0017 U13-Q9 amended, T289's review fix): their shown
        // text, list and Lang Alt sub-fields count as typed, so they are checked for variables at
        // once and written with their variables to be filled, as a pasted field is.
        for (list, values) in &copied.locations {
            if values.is_empty() || !layout.is_shown(EditorItem::List(list)) {
                continue;
            }
            if let Some(state) = self.list_editable(list) {
                state.entries = values
                    .iter()
                    .map(|s| ListEntry::pasted(state.spec, s))
                    .collect();
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
        // A structure list gets the chosen value's entries (U11a, point 6 of T136).
        for state in &mut self.lists {
            if state.property() == conflict.property
                && let Some(entries) = list_entries_of(state.spec, value)
            {
                state.entries = entries;
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
            }) || self.lists.iter().any(|state| {
                state.property() == conflict.property && self.list_edit(state).is_some()
            });
            if !changed_after_choice {
                // Not typed by the user: escaped, so the write gives it back exactly (T289).
                edits.push(Edit::Set {
                    property: conflict.property.clone(),
                    field: None,
                    value: escaped_value(value),
                });
            }
        }
        for (index, state) in self.fields.iter().enumerate() {
            // The structure lists drawn before this field (U11a-Q2, T139 point 2).
            edits.extend(self.list_edits_at(index));
            // A read-only field never gives an edit (point 4, U10-Q3).
            let read_only = !state.field.editable() || self.read_only_text(state.field).is_some();
            let start_text = self.start_text(state);
            if !read_only && state.text != start_text {
                match field_edit(state.field, &start_text, &state.text) {
                    Ok(edit) => edits.push(edit),
                    Err(error) => errors.push((state.field, error)),
                }
            }
        }
        edits.extend(self.list_edits_at(self.fields.len()));
        (edits, errors)
    }

    /// The edits of the form: a `Set` or `Clear` for each changed field (in field order, after
    /// the chosen conflict values; a changed structure list in its place among the fields, as one
    /// `Set` of the whole list, or a `Clear` when no entry is left, U11a-Q3), or every field error
    /// together (a wrong date, or a text with an unknown variable or a brace that is not closed,
    /// T289). A structure list's sub-field with an error (a wrong date, T143, or a variable that
    /// cannot be read, T289) also gives `Err`, so nothing is written; its error is not a field's,
    /// so it is not in the list (it may then be empty) but comes from
    /// [`EditorForm::list_error`]. The texts the user did not type are escaped (a chosen conflict
    /// value, a list field's entries of its start text, the unchanged parts of a structure list),
    /// those typed are kept with their variables, which T288 fills for each image.
    pub fn edits(&self) -> Result<Vec<Edit>, Vec<(EditorField, String)>> {
        let (edits, errors) = self.collect();
        if errors.is_empty() && !self.has_list_error() {
            Ok(edits)
        } else {
            Err(errors)
        }
    }

    /// What "Apply to All Marked" writes (U12c-Q3, T126): for the window's image the edits of
    /// [`EditorForm::edits`]; for every other marked image only the edits of the changed fields
    /// (a field whose text differs from its start text), never a chosen conflict value, and no
    /// `Clear` of a copyright-management field (ADR 0007 §8), which is emptied on the window's
    /// image only. A changed list field gives the other images only its added and removed
    /// entries, Creator only its added ones (U12d-Q1, U12d-Q2, T180). Every field error together,
    /// and no batch, when a field has one.
    pub fn batch_edits(&self) -> Result<BatchEdits, Vec<(EditorField, String)>> {
        let mut batch = BatchEdits {
            this_image: self.edits()?,
            others: Vec::new(),
            changed: Vec::new(),
            emptied: Vec::new(),
            emptied_here_only: Vec::new(),
            list_changes: Vec::new(),
        };
        for (index, state) in self.fields.iter().enumerate() {
            self.batch_lists(index, &mut batch);
            self.batch_field(state, &mut batch)?;
        }
        self.batch_lists(self.fields.len(), &mut batch);
        Ok(batch)
    }

    /// The part of `batch` the structure lists placed before the field with index `slot` give: a
    /// changed list replaces the whole list on every image, and an emptied one is emptied on
    /// every image (U11a-Q5), except a copyright-management list (Image Creator, Copyright Owner,
    /// Licensor), which is emptied on the window's image only (U11b-Q5, U12c-Q3);
    /// `apply_confirmation` names them.
    fn batch_lists(&self, slot: usize, batch: &mut BatchEdits) {
        for edit in self.list_edits_at(slot) {
            let here_only = match &edit {
                Edit::Clear { property, .. } => list_copyright_management(property),
                Edit::Set { .. } => false,
                Edit::Entries { .. } => false,
            };
            if !here_only {
                batch.others.push(edit);
            }
        }
    }

    /// The part of `batch` that one field gives (see [`EditorForm::batch_edits`]).
    fn batch_field(
        &self,
        state: &FieldState,
        batch: &mut BatchEdits,
    ) -> Result<(), Vec<(EditorField, String)>> {
        if !state.field.editable() || self.read_only_text(state.field).is_some() {
            return Ok(());
        }
        let start_text = self.start_text(state);
        if state.text == start_text {
            return Ok(());
        }
        if state.field.is_list() && state.field.struct_field().is_none() {
            batch_list_field(state.field, &start_text, &state.text, batch);
            return Ok(());
        }
        // `edits` had no error, so neither has this field.
        let edit = field_edit(state.field, &start_text, &state.text)
            .map_err(|e| vec![(state.field, e)])?;
        let cleared = matches!(edit, Edit::Clear { .. });
        if cleared && state.field.copyright_management() {
            batch.emptied_here_only.push(state.field);
            return Ok(());
        }
        if cleared {
            batch.emptied.push(state.field);
        } else {
            batch.changed.push(state.field);
        }
        batch.others.push(edit);
        Ok(())
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

/// The values of the template window's form (ADR 0017 U13c, T291), from
/// [`EditorForm::template_values`]: everything as typed, nothing escaped. Also the values of the
/// metadata window's Save as Template… (U13-Q7, U13-Q25, T300), from
/// [`EditorForm::window_template_values`]: only the items shown, what was read from the image
/// escaped.
#[derive(Clone, Debug)]
pub(crate) struct TemplateValues {
    /// Each editable field in [`EditorField::ALL`] order with its value (`None` when it gives
    /// none, for example an empty text) or its error's text.
    pub(crate) fields: Vec<(EditorField, Result<Option<Value>, String>)>,
    /// Each structure list in [`EditorForm::ALL_LISTS`] order.
    pub(crate) lists: Vec<TemplateListValues>,
}

/// One structure list of [`TemplateValues`].
#[derive(Clone, Debug)]
pub(crate) struct TemplateListValues {
    /// The XMP id of the list.
    pub(crate) list: &'static str,
    /// The entries as [`ListEntry::value`] gives them (for the metadata window
    /// [`ListEntry::written`]), the empty ones left out.
    pub(crate) entries: Vec<StructValue>,
    /// The error of each sub-field that has one: the entry's index (among all the list's
    /// entries, the empty ones included), the sub-field's XMP id and the error's text.
    pub(crate) errors: Vec<(usize, &'static str, String)>,
}

impl TemplateValues {
    /// The value or error of `field`; `None` for a field that is not editable.
    pub(crate) fn field(&self, field: EditorField) -> Option<&Result<Option<Value>, String>> {
        self.fields
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, value)| value)
    }

    /// The entries and errors of the structure list `list`.
    pub(crate) fn list(&self, list: &str) -> Option<&TemplateListValues> {
        self.lists.iter().find(|values| values.list == list)
    }
}

/// The template window's form (ADR 0017 U13c, T291): the same form as the metadata window's, with
/// no image behind it, so every start text is empty and whatever is in it counts as typed.
impl EditorForm {
    /// The empty form: no value, no conflict, every structure list empty; the same as
    /// [`EditorForm::from_read`] of a read with no metadata.
    pub(crate) fn empty() -> EditorForm {
        EditorForm {
            fields: EditorField::ALL
                .iter()
                .map(|&field| FieldState {
                    field,
                    text: String::new(),
                    original: String::new(),
                    language: None,
                })
                .collect(),
            conflicts: Vec::new(),
            chosen: Vec::new(),
            lists: LIST_SPECS
                .iter()
                .map(|spec| ListState {
                    spec,
                    entries: Vec::new(),
                    original: Vec::new(),
                    read_text: String::new(),
                    unsupported: false,
                })
                .collect(),
        }
    }

    /// The values of the template form as typed, nothing escaped (a template is not read from an
    /// image, so it has no text to keep exactly): each editable field's value from [`field_edit`]
    /// with an empty start text (a `Set` gives its value, a `Clear` none, an error its text; a
    /// contact field's value is its sub-field's), and each structure list's entries
    /// ([`ListEntry::value`], the empty ones left out) with the error of each sub-field
    /// ([`ListEntry::error`]).
    pub(crate) fn template_values(&self) -> TemplateValues {
        let fields = self
            .fields
            .iter()
            .filter(|state| state.field.editable())
            .map(|state| {
                let value = match field_edit(state.field, "", &state.text) {
                    Ok(Edit::Set { value, .. }) => Ok(Some(value)),
                    Ok(_) => Ok(None),
                    Err(error) => Err(error),
                };
                (state.field, value)
            })
            .collect();
        let lists = self
            .lists
            .iter()
            .map(|state| TemplateListValues {
                list: state.property(),
                entries: list_values(&state.entries),
                errors: entry_errors(&state.entries),
            })
            .collect();
        TemplateValues { fields, lists }
    }

    /// The values of the metadata window's Save as Template… (ADR 0017 U13-Q7, U13-Q25, T300):
    /// the items Copy All Fields copies ([`EditorForm::copy_shown`]: the editable fields `layout`
    /// shows that are not read-only, and the shown structure lists that are not read-only).
    ///
    /// What was read from the image and left unchanged is escaped ([`variables::escape`]), so the
    /// image's own braces stay text; what was typed or pasted is kept as it stands, its variables
    /// with it. A field's value is made by [`field_edit`] with an empty start text from its text,
    /// escaped as a whole when it equals its start text; in a list field each entry that is also
    /// an entry of the start text is escaped and the others are not. Copyrighted gives `True`
    /// when its box is ticked and nothing when not (the form cannot tell an image's `False` from
    /// no value). A structure list's entries are [`ListEntry::written`] (the empty ones left
    /// out), with the error of each sub-field ([`ListEntry::error`]).
    pub(crate) fn window_template_values(&self, layout: &EditorLayout) -> TemplateValues {
        let fields = self
            .fields
            .iter()
            .filter(|state| {
                state.field.editable()
                    && self.read_only_text(state.field).is_none()
                    && layout.is_shown(EditorItem::of_field(state.field))
            })
            .map(|state| (state.field, self.window_field_value(state)))
            .collect();
        let lists = self
            .lists
            .iter()
            .filter(|state| {
                self.list_read_only(state.property()).is_none()
                    && layout.is_shown(EditorItem::List(state.property()))
            })
            .map(|state| TemplateListValues {
                list: state.property(),
                entries: state
                    .entries
                    .iter()
                    .filter_map(ListEntry::written)
                    .collect(),
                errors: entry_errors(&state.entries),
            })
            .collect();
        TemplateValues { fields, lists }
    }

    /// The value of one field for [`EditorForm::window_template_values`].
    fn window_field_value(&self, state: &FieldState) -> Result<Option<Value>, String> {
        if state.field.check_box() {
            return Ok((state.text == CHECKED).then(|| Value::Text(CHECKED.to_string())));
        }
        let start = self.start_text(state);
        let text = if state.field.is_list() {
            let start = list_entries(&start);
            list_entries(&state.text)
                .into_iter()
                .map(|entry| {
                    if start.contains(&entry) {
                        variables::escape(&entry)
                    } else {
                        entry
                    }
                })
                .collect::<Vec<String>>()
                .join(LIST_SEPARATOR)
        } else if state.text == start {
            variables::escape(&state.text)
        } else {
            state.text.clone()
        };
        match field_edit(state.field, "", &text) {
            Ok(Edit::Set { value, .. }) => Ok(Some(value)),
            Ok(_) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Puts a template's fields (`(XMP id, value)` as `TemplateFile::fields` holds them) in the
    /// form as typed; the start texts stay empty. A field gets its text; Copyrighted is ticked
    /// for `True` and left unticked for `False`; Creator's Contact Info fills its contact fields;
    /// a structure list's entries replace the list's, each as pasted (its shown sub-fields typed,
    /// the others kept in its base); Location Created gets its first entry as its only entry.
    ///
    /// Returns the XMP ids (`property` or `property/sub-field`) of what was not put in the form: a
    /// property the form has no editable field or list for, a Copyrighted that is neither `True`
    /// nor `False`, a value of a shape the field cannot take, and a sub-field of Creator's Contact
    /// Info or Location Created that the form has no field for (Location Created's Location Name
    /// and GPS fields, for example; they are not kept).
    pub(crate) fn put_template_fields(&mut self, fields: &[(String, Value)]) -> Vec<String> {
        let mut not_loaded = Vec::new();
        for (property, value) in fields {
            if property == CONTACT_INFO {
                let Value::Struct(structure) = value else {
                    not_loaded.push(property.clone());
                    continue;
                };
                for (sub, sub_value) in &structure.fields {
                    let state = self
                        .fields
                        .iter_mut()
                        .find(|state| state.field.struct_field() == Some(sub.as_str()));
                    match state {
                        Some(state) => state.text = form_text(sub_value),
                        None => not_loaded.push(format!("{property}/{sub}")),
                    }
                }
                continue;
            }
            if let Some(state) = self
                .lists
                .iter_mut()
                .find(|state| state.property() == property)
            {
                let entries = match value {
                    Value::StructList(entries) => entries.clone(),
                    Value::Struct(entry) => vec![entry.clone()],
                    _ => {
                        not_loaded.push(property.clone());
                        continue;
                    }
                };
                if property == LOCATION_CREATED {
                    let mut first = entries.into_iter().next().unwrap_or_default();
                    first.fields.retain(|(sub, _)| {
                        let shown = LOCATION_FIELDS.contains(&sub.as_str());
                        if !shown {
                            not_loaded.push(format!("{property}/{sub}"));
                        }
                        shown
                    });
                    state.entries = vec![ListEntry::pasted(state.spec, &first)];
                } else {
                    state.entries = entries
                        .iter()
                        .map(|entry| ListEntry::pasted(state.spec, entry))
                        .collect();
                }
                continue;
            }
            let state = self.fields.iter_mut().find(|state| {
                state.field.struct_field().is_none()
                    && state.field.editable()
                    && state.field.property() == property
            });
            let Some(state) = state else {
                not_loaded.push(property.clone());
                continue;
            };
            match template_field_text(state.field, value) {
                Some(text) => state.text = text,
                None => not_loaded.push(property.clone()),
            }
        }
        not_loaded
    }

    /// Load Template… in the metadata window (ADR 0017 U13-Q7, U13-Q12, U13-Q23, T300): each field
    /// of `template` goes into the form as typed, an unsaved edit, as Paste All Fields puts a
    /// copied text, so its variables are checked and filled per image (U13-Q9). The start texts
    /// are not changed, so what was loaded counts as changed.
    ///
    /// - a text, Lang Alt (`x-default`), date, number or choice field gets the template's value;
    ///   Copyrighted is ticked for `True` and unticked for `False`;
    /// - a list field without its id in the template's `add` gets the template's entries in place
    ///   of the form's; with it, the form's own entries are kept and the template's added after
    ///   them, an entry the form already holds (exact text) not added again. The entries are split
    ///   and joined as the form's own list text is, so an entry holding a comma becomes two;
    /// - Creator's Contact Info: each sub-field in the template replaces that contact field's
    ///   text, the others are kept;
    /// - Location Created: each sub-field in the template replaces that sub-field of the form's
    ///   first entry, the others are kept (an entry is added when the form has none);
    /// - any other structure list: the template's entries replace the form's, each as pasted.
    ///
    /// Returns, in the template's order, the XMP ids (`property` or `property/sub-field`) of what
    /// was not loaded: a field or list `layout` hides, one shown read-only, a property the form has
    /// no editable field or list for, a sub-field of Location Created the form has no field for,
    /// a value of a shape the field cannot take, and a Copyrighted that is neither `True` nor
    /// `False`. A hidden contact field is named as `Iptc4xmpCore:CreatorContactInfo/{sub-field}`.
    pub fn load_template(&mut self, template: &TemplateFile, layout: &EditorLayout) -> Vec<String> {
        let mut not_loaded = Vec::new();
        for (property, value) in &template.fields {
            if property == CONTACT_INFO {
                let Value::Struct(structure) = value else {
                    not_loaded.push(property.clone());
                    continue;
                };
                for (sub, sub_value) in &structure.fields {
                    let field = EditorField::ALL
                        .into_iter()
                        .find(|field| field.struct_field() == Some(sub.as_str()))
                        .filter(|field| self.field_loads(*field, layout));
                    let text = match sub_value {
                        Value::Struct(_) | Value::StructList(_) => None,
                        _ => Some(form_text(sub_value)),
                    };
                    match field.zip(text) {
                        Some((field, text)) => self.set_text(field, text),
                        None => not_loaded.push(format!("{property}/{sub}")),
                    }
                }
                continue;
            }
            if list_spec(property).is_some() {
                self.load_template_list(property, value, layout, &mut not_loaded);
                continue;
            }
            let loaded = EditorField::ALL
                .into_iter()
                .find(|field| field.struct_field().is_none() && field.property() == property)
                .filter(|field| self.field_loads(*field, layout))
                .and_then(|field| template_field_text(field, value).map(|text| (field, text)));
            let Some((field, text)) = loaded else {
                not_loaded.push(property.clone());
                continue;
            };
            let text = if field.is_list() && template.add.contains(property) {
                let mut entries = list_entries(self.text(field));
                let before = entries.len();
                for entry in list_entries(&text) {
                    if !entries.contains(&entry) {
                        entries.push(entry);
                    }
                }
                if entries.len() == before {
                    continue;
                }
                entries.join(LIST_SEPARATOR)
            } else {
                text
            };
            self.set_text(field, text);
        }
        not_loaded
    }

    /// True when Load Template… may put a value in `field` (U13-Q23): it is editable, `layout`
    /// shows it and it is not shown read-only.
    fn field_loads(&self, field: EditorField, layout: &EditorLayout) -> bool {
        field.editable()
            && layout.is_shown(EditorItem::of_field(field))
            && self.read_only_text(field).is_none()
    }

    /// Sets the text of `field` (a check box's `True` or empty, a choice's value).
    fn set_text(&mut self, field: EditorField, text: String) {
        if let Some(state) = self.fields.iter_mut().find(|state| state.field == field) {
            state.text = text;
        }
    }

    /// The structure list `list` of [`EditorForm::load_template`]; what is not loaded is pushed on
    /// `not_loaded`.
    fn load_template_list(
        &mut self,
        list: &str,
        value: &Value,
        layout: &EditorLayout,
        not_loaded: &mut Vec<String>,
    ) {
        let entries = match value {
            Value::StructList(entries) => Some(entries.clone()),
            Value::Struct(entry) => Some(vec![entry.clone()]),
            _ => None,
        };
        let shown =
            list_spec(list).is_some_and(|spec| layout.is_shown(EditorItem::List(spec.property)));
        let state = if shown {
            self.list_editable(list)
        } else {
            None
        };
        let (Some(entries), Some(state)) = (entries, state) else {
            not_loaded.push(list.to_string());
            return;
        };
        if list != LOCATION_CREATED {
            state.entries = entries
                .iter()
                .map(|entry| ListEntry::pasted(state.spec, entry))
                .collect();
            return;
        }
        let first = entries.into_iter().next().unwrap_or_default();
        let mut texts = Vec::new();
        for (sub, sub_value) in &first.fields {
            let index = state.spec.fields.iter().position(|field| field == sub);
            let text = match sub_value {
                Value::Struct(_) | Value::StructList(_) => None,
                _ => Some(form_text(sub_value)),
            };
            match index.zip(text) {
                Some(found) => texts.push(found),
                None => not_loaded.push(format!("{list}/{sub}")),
            }
        }
        if texts.is_empty() {
            return;
        }
        if state.entries.is_empty() {
            state
                .entries
                .push(ListEntry::of(state.spec, &StructValue::default()));
        }
        if let Some(entry) = state.entries.first_mut() {
            for (index, text) in texts {
                if let Some(slot) = entry.texts.get_mut(index) {
                    *slot = text;
                }
            }
        }
    }
}

/// The text a template's `value` gives the field `field` (not a contact field): a check box's
/// `True` for `True` and empty for `False` (any case); [`EditorField::text_of`] for any other
/// field. `None` for a check box with another value and for a structure.
fn template_field_text(field: EditorField, value: &Value) -> Option<String> {
    if field.check_box() {
        let text = form_text(value);
        return if text.eq_ignore_ascii_case(CHECKED) {
            Some(CHECKED.to_string())
        } else if text.eq_ignore_ascii_case(UNCHECKED) {
            Some(String::new())
        } else {
            None
        };
    }
    match value {
        Value::Struct(_) | Value::StructList(_) => None,
        _ => Some(field.text_of(value)),
    }
}

/// The error of each sub-field of `entries` that has one ([`ListEntry::error`]): the entry's
/// index, the sub-field's XMP id and the error's text.
fn entry_errors(entries: &[ListEntry]) -> Vec<(usize, &'static str, String)> {
    entries
        .iter()
        .enumerate()
        .flat_map(|(index, entry)| {
            entry
                .spec
                .fields
                .iter()
                .filter_map(move |field| entry.error(field).map(|error| (index, *field, error)))
        })
        .collect()
}

/// The part of `batch` a changed list field `field` gives (U12d-Q1, U12d-Q2, T180): the entries
/// of `text` not in `start` are added on every other image, those of `start` not in `text`
/// removed there (exact text, each once); Creator's removed entries are removed on the window's
/// image only. The other images get an `Edit::Entries` only when it adds or removes something.
fn batch_list_field(field: EditorField, start: &str, text: &str, batch: &mut BatchEdits) {
    let start = list_entries(start);
    let now = list_entries(text);
    let adds = entries_not_in(&now, &start);
    let removed = entries_not_in(&start, &now);
    let (removes, removes_here) = if field.copyright_management() {
        (Vec::new(), removed)
    } else {
        (removed, Vec::new())
    };
    if !adds.is_empty() || !removes.is_empty() {
        batch.others.push(Edit::Entries {
            property: field.property().to_string(),
            add: adds.clone(),
            remove: removes.clone(),
        });
    }
    if !adds.is_empty() || !removes.is_empty() || !removes_here.is_empty() {
        batch.list_changes.push(ListChange {
            field,
            adds,
            removes,
            removes_here,
        });
    }
}

/// The entries of `entries` not in `other` (exact text), in `entries`' order, each once.
fn entries_not_in(entries: &[String], other: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for entry in entries {
        if !other.contains(entry) && !found.contains(entry) {
            found.push(entry.clone());
        }
    }
    found
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

/// The values of `text` in `field` (U12a, T141): a list field's entries, else the text itself;
/// nothing for a text that is empty or only spaces.
fn text_values(field: EditorField, text: &str) -> Vec<String> {
    if field.is_list() {
        list_entries(text)
    } else if text.trim().is_empty() {
        Vec::new()
    } else {
        vec![text.to_string()]
    }
}

/// The edit of a changed field with the text `text`, or the field's error; `start` is the text the
/// field is compared with ([`EditorForm::start_text`]).
///
/// Variables (ADR 0017 U13-Q9, T289): a text, Lang Alt or list field that is not a choice list is
/// checked with [`variables::check`], so an unknown variable or a brace that is not closed is the
/// field's error ([`variable_error_text`]) before anything is written; a date is not checked. In a
/// list field only the entries the user typed (not among the entries of `start`, exact text) are
/// checked and kept as typed, so T288 fills their variables; the entries of `start` are escaped
/// ([`variables::escape`]), so the write gives them back exactly.
fn field_edit(field: EditorField, start: &str, text: &str) -> Result<Edit, String> {
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
    let checked = field.choice_list().is_none();
    let check = |text: &str| -> Result<(), String> {
        if checked {
            variables::check(text).map_err(|error| variable_error_text(&error))
        } else {
            Ok(())
        }
    };
    let value = match field.kind() {
        FieldKind::Text => {
            check(text)?;
            Value::Text(text.to_string())
        }
        FieldKind::LangAlt => {
            check(text)?;
            Value::LangAlt(LangAlt::from_entries(vec![(
                X_DEFAULT.to_string(),
                text.to_string(),
            )]))
        }
        FieldKind::List => {
            let start = list_entries(start);
            let mut entries = list_entries(text);
            // Only spaces and commas: the list is emptied.
            if entries.is_empty() {
                return Ok(clear());
            }
            for entry in &mut entries {
                if start.contains(entry) {
                    *entry = variables::escape(entry);
                } else {
                    check(entry)?;
                }
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
        Warning::VocabularyValueUnknown { property, value } => format!(
            "{field} {value} is not in the PLUS list",
            field = property_label(property, Language::En)
        ),
        Warning::NewsCodeUnknown { property, value } => format!(
            "{field} {value} is not in IPTC's list",
            field = property_label(property, Language::En)
        ),
        Warning::SubjectCodeForm { code } => format!(
            "{field} {code} should be eight digits",
            field = property_label("Iptc4xmpCore:SubjectCode", Language::En)
        ),
    }
}

/// The English label of a value of a choice list (U10d-Q3, T128; the Licensor telephone types,
/// T139), the vocabulary's own: a PLUS code or a full Digital Source Type URI ([`vocabulary`]),
/// the retired Digital Source Types included. `None` for any other value.
fn choice_label(value: &str) -> Option<&'static str> {
    if let Some(code) = value.strip_prefix(vocabulary::DIGITAL_SOURCE_TYPE_PREFIX) {
        return digital_source_type_label(code);
    }
    let label = match value {
        "MR-NON" | "PR-NON" => "None",
        "MR-NAP" | "PR-NAP" => "Not Applicable",
        "MR-UMR" => "Unlimited Model Releases",
        "MR-LMR" => "Limited or Incomplete Model Releases",
        "PR-UPR" => "Unlimited Property Releases",
        "PR-LPR" => "Limited or Incomplete Property Releases",
        "AG-UNK" => "Age Unknown",
        "AG-A25" => "Age 25 or Over",
        "AG-A24" => "Age 24",
        "AG-A23" => "Age 23",
        "AG-A22" => "Age 22",
        "AG-A21" => "Age 21",
        "AG-A20" => "Age 20",
        "AG-A19" => "Age 19",
        "AG-A18" => "Age 18",
        "AG-A17" => "Age 17",
        "AG-A16" => "Age 16",
        "AG-A15" => "Age 15",
        "AG-U14" => "Age 14 or Under",
        // The telephone types of a Licensor entry (T139): the LDF file's labels, capitalised.
        "work" => "Work",
        "cell" => "Cell",
        "fax" => "Fax",
        "home" => "Home",
        "pager" => "Pager",
        "DMI-UNSPECIFIED" => "Unspecified - no prohibition defined",
        "DMI-ALLOWED" => "Allowed",
        "DMI-PROHIBITED-AIMLTRAINING" => "Prohibited for AI/ML training",
        "DMI-PROHIBITED-GENAIMLTRAINING" => "Prohibited for Generative AI/ML training",
        "DMI-PROHIBITED-EXCEPTSEARCHENGINEINDEXING" => {
            "Prohibited except for search engine indexing"
        }
        "DMI-PROHIBITED" => "Prohibited",
        "DMI-PROHIBITED-SEECONSTRAINT" => "Prohibited, see Other Constraints property",
        "DMI-PROHIBITED-SEEEMBEDDEDRIGHTSEXPR" => {
            "Prohibited, see Embedded Encoded Rights Expression property"
        }
        "DMI-PROHIBITED-SEELINKEDRIGHTSEXPR" => {
            "Prohibited, see Linked Encoded Rights Expression property"
        }
        _ => return None,
    };
    Some(label)
}

/// The English label of a Digital Source Type code (the part after the prefix), current or
/// retired.
fn digital_source_type_label(code: &str) -> Option<&'static str> {
    let label = match code {
        "digitalCapture" => "Digital capture sampled from real life",
        "computationalCapture" => "Multi-frame computational capture sampled from real life",
        "negativeFilm" => "Digitised from a transparent negative",
        "positiveFilm" => "Digitised from a transparent positive",
        "print" => "Digitised from a non-transparent medium",
        "humanEdits" => "Human-edited media",
        "compositeWithTrainedAlgorithmicMedia" => "Edited using Generative AI",
        "algorithmicallyEnhanced" => "Algorithmically-altered media",
        "digitalCreation" => "Digital creation",
        "dataDrivenMedia" => "Data-driven media",
        "trainedAlgorithmicMedia" => "Created using Generative AI",
        "algorithmicMedia" => "Pure algorithmic media",
        "screenCapture" => "Screen capture",
        "virtualRecording" => "Virtual event recording",
        "composite" => "Composite of elements",
        "compositeCapture" => "Composite of captured elements",
        "compositeSynthetic" => "Composite including generative AI elements",
        // Retired: still valid in a file, not offered.
        "minorHumanEdits" => "Original media with minor human edits",
        "softwareImage" => "Created by software",
        "digitalArt" => "Digital art",
        _ => return None,
    };
    Some(label)
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
            "No new IIM checksum was written, because IIM and XMP still differ".to_string()
        }
        WriteNotice::QualifiersKept { property } => format!(
            "{field} was not written: it has qualifiers RawStride cannot keep",
            field = property_label(property, Language::En)
        ),
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
    /// The entry of a choice list for a value outside its vocabulary (U10d-Q4, T128); `value` as
    /// read.
    UnknownValue { value: String },
    /// The button that writes the edits and closes the window.
    Ok,
    /// Shown while the write runs.
    Saving,
    /// Shown after a write that succeeded.
    Saved,
    /// A write that failed; `reason` as `write_error_text` gives it for ratings.
    SaveFailed { file: String, reason: String },
    /// The window's status line when OK, Save and Next, Save or "Apply to {n} Marked" wrote
    /// nothing because of field errors (U11c-Q9, T181): the fields and the structure lists
    /// (XMP ids of [`EditorForm::ALL_LISTS`]) that have an error.
    NotSaved {
        fields: Vec<EditorField>,
        lists: Vec<&'static str>,
    },
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
    /// The line under `MarkedImages`: OK and Save write the window's image only (U12d-Q4, T180).
    ThisImageOnly,
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
    /// The confirmation's line naming the changed fields and structure lists (XMP ids of
    /// [`EditorForm::ALL_LISTS`]) given a value, in form order (U11a-Q5, T136; U11b-Q5).
    Changed {
        fields: Vec<EditorField>,
        lists: Vec<&'static str>,
    },
    /// The confirmation's line naming the fields and structure lists emptied on every image.
    Emptied {
        fields: Vec<EditorField>,
        lists: Vec<&'static str>,
    },
    /// The confirmation's line naming the copyright-management fields and lists emptied on the
    /// window's image only (U12c-Q4; Image Creator, Copyright Owner and Licensor, U11b-Q5, T139).
    EmptiedHere {
        fields: Vec<EditorField>,
        lists: Vec<&'static str>,
    },
    /// The button under a location list that adds an empty entry (U11a-Q2, T136).
    AddLocation,
    /// The button of a location entry that removes it (U11a-Q2, T136).
    RemoveLocation,
    /// The button under Location Shown that adds a copy of the first Location Created entry
    /// (U11a-Q4, T136).
    CopyFromLocationCreated,
    /// The button under a structure list other than the locations that adds an empty entry
    /// (T139 point 2); `list` is its XMP id.
    AddEntry { list: &'static str },
    /// The tooltip of a disabled "Add" at the list's limit (T139 point 2).
    AtMostEntries { max: usize },
    /// The button under Image Creator that adds an entry per Creator name (U11b-Q4, T139).
    CopyFromCreator,
    /// The collapsed line over a Licensor entry's address fields (U11b-Q3, T139).
    Address,
    /// The collapsed line over the eleven further sub-fields of an Artwork or Object entry
    /// (U11c-Q3, T143).
    More,
    /// The button under Person Shown with Details that adds an entry per Person Shown name
    /// (U11c-Q6, T143).
    CopyFromPersonShown,
    /// The confirmation's line of a list field that adds `entries` on every image (U12d-Q3,
    /// T180).
    ListAdds {
        field: EditorField,
        entries: Vec<String>,
    },
    /// The confirmation's line of a list field that removes `entries` on every image.
    ListRemoves {
        field: EditorField,
        entries: Vec<String>,
    },
    /// The confirmation's line of a list field that adds `adds` and removes `removes` on every
    /// image.
    ListAddsRemoves {
        field: EditorField,
        adds: Vec<String>,
        removes: Vec<String>,
    },
    /// The confirmation's line of Creator's entries removed on the window's image only
    /// (U12d-Q2).
    ListRemovesHere {
        field: EditorField,
        entries: Vec<String>,
    },
    /// The confirmation's line counting the marked files without metadata support.
    SkippedFiles { files: usize },
    /// The confirmation's button that starts the writes.
    Write,
    /// Shown while the batch runs (U12c-Q5): the write `current` of `total`.
    Writing { current: usize, total: usize },
    /// The first line of the batch's summary (U12c-Q6).
    BatchWritten { written: usize, total: usize },
    /// A line of the summary: an unresolved conflict on `property` (its XMP id) kept in `file`.
    /// The summary's lines name every write notice of the other images (ADR 0017 U12d-Q10).
    ConflictKept { file: String, property: String },
    /// A line of the summary: a changed list with qualifiers on `property` (its XMP id) put back
    /// instead of written in `file` (ADR 0007 §8).
    QualifiersKept { file: String, property: String },
    /// A line of the summary: the IIM value of `property` (its XMP id) truncated in `file`
    /// (ADR 0007 §5.2).
    TruncatedInIim { file: String, property: String },
    /// A line of the summary: the IIM texts of `file` written again as UTF-8 (ADR 0007 §5.4).
    IimRewrittenAsUtf8 { file: String },
    /// A line of the summary: no new IIM checksum written in `file` (ADR 0007 §5.2 Q1, §8).
    NoDigestWritten { file: String },
    /// The tooltip of the "▼" button right of a field that opens its list of values (U12a-Q5,
    /// U12a-Q7, T141; the triangle is painted, not a text).
    ValuesTooltip,
    /// The heading of a field's recent values in its list (U12a-Q2).
    Recent,
    /// The heading of a field's own values in its list (U12a-Q3).
    MyValues,
    /// The entry of the list that adds the field's present text to its own values (U12a-Q3).
    AddToMyValues,
    /// The list's line when the field has neither recent nor own values (U12a-Q5).
    NoValuesYet,
    /// The button beside an own value that removes it (U12a-Q3).
    RemoveValue,
}

/// The names of `fields` in form order, separated by commas.
fn field_names(fields: &[EditorField]) -> String {
    fields
        .iter()
        .map(|field| field.label())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The names of `fields` and of the structure lists `lists` (XMP ids) in form order, separated
/// by commas: each list in its place among the fields (the locations after Country Code,
/// U11a-Q2; Genre after Scene Code; the PLUS lists before the release fields, T139), each named by
/// [`property_label`].
fn item_names(fields: &[EditorField], lists: &[&'static str]) -> String {
    if lists.is_empty() {
        return field_names(fields);
    }
    let position = |field: &EditorField| {
        EditorField::ALL
            .iter()
            .position(|f| f == field)
            .unwrap_or(EditorField::ALL.len())
    };
    // A field at index i sorts as 2i + 1, a list placed before field i as 2i: lists before
    // their field, fields and lists each in the order given (a stable sort).
    let mut items: Vec<(usize, String)> = fields
        .iter()
        .map(|field| (2 * position(field) + 1, field.label()))
        .chain(lists.iter().map(|list| {
            let slot = list_spec(list).map_or(EditorField::ALL.len(), list_slot);
            (2 * slot, property_label(list, Language::En))
        }))
        .collect();
    items.sort_by_key(|(key, _)| *key);
    items
        .into_iter()
        .map(|(_, name)| name)
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
            EditorSection::Licence => "Licence (PLUS)",
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
        EditorText::UnknownValue { value } => format!("Unknown value: {value}"),
        EditorText::Ok => "OK".to_string(),
        EditorText::Saving => "Saving …".to_string(),
        EditorText::Saved => "Saved.".to_string(),
        EditorText::SaveFailed { file, reason } => {
            format!("Could not save metadata for {file}: {reason}")
        }
        EditorText::NotSaved { fields, lists } => match fields.len() + lists.len() {
            0 => "Nothing was saved. Correct the errors in the form.".to_string(),
            1 => format!(
                "Nothing was saved. Correct the error in {}.",
                item_names(fields, lists)
            ),
            _ => format!(
                "Nothing was saved. Correct the errors in {}.",
                item_names(fields, lists)
            ),
        },
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
        EditorText::ThisImageOnly => "OK and Save write this image only.".to_string(),
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
        EditorText::Changed { fields, lists } => {
            format!("Changed: {}", item_names(fields, lists))
        }
        EditorText::Emptied { fields, lists } => {
            format!("Emptied: {}", item_names(fields, lists))
        }
        EditorText::AddLocation => "Add Location".to_string(),
        EditorText::RemoveLocation => "Remove".to_string(),
        EditorText::CopyFromLocationCreated => format!(
            "Copy from {list}",
            list = property_label(LOCATION_CREATED, Language::En)
        ),
        EditorText::EmptiedHere { fields, lists } => {
            format!("Emptied only on this image: {}", item_names(fields, lists))
        }
        EditorText::AddEntry { list } => {
            format!("Add {list}", list = property_label(list, Language::En))
        }
        EditorText::AtMostEntries { max } => {
            if *max == 1 {
                "At most 1 entry".to_string()
            } else {
                format!("At most {max} entries")
            }
        }
        EditorText::CopyFromCreator => format!(
            "Copy from {field}",
            field = property_label(EditorField::Creator.property(), Language::En)
        ),
        EditorText::Address => "Address".to_string(),
        EditorText::More => "More".to_string(),
        EditorText::CopyFromPersonShown => format!(
            "Copy from {field}",
            field = property_label(EditorField::PersonShown.property(), Language::En)
        ),
        EditorText::ListAdds { field, entries } => format!(
            "{field}: adds {entries}",
            field = field.label(),
            entries = entries.join(", ")
        ),
        EditorText::ListRemoves { field, entries } => format!(
            "{field}: removes {entries}",
            field = field.label(),
            entries = entries.join(", ")
        ),
        EditorText::ListAddsRemoves {
            field,
            adds,
            removes,
        } => format!(
            "{field}: adds {adds}; removes {removes}",
            field = field.label(),
            adds = adds.join(", "),
            removes = removes.join(", ")
        ),
        EditorText::ListRemovesHere { field, entries } => format!(
            "{field}: removes {entries} on this image only",
            field = field.label(),
            entries = entries.join(", ")
        ),
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
        EditorText::QualifiersKept { file, property } => format!(
            "List with qualifiers not written: {file} ({field})",
            field = property_label(property, Language::En)
        ),
        EditorText::TruncatedInIim { file, property } => format!(
            "Truncated in IIM: {file} ({field})",
            field = property_label(property, Language::En)
        ),
        EditorText::IimRewrittenAsUtf8 { file } => format!("IIM texts rewritten as UTF-8: {file}"),
        EditorText::NoDigestWritten { file } => format!("No new IIM checksum written: {file}"),
        EditorText::ValuesTooltip => "Recent and saved values".to_string(),
        EditorText::Recent => "Recent".to_string(),
        EditorText::MyValues => "My Values".to_string(),
        EditorText::AddToMyValues => "Add to My Values".to_string(),
        EditorText::NoValuesYet => "No values yet".to_string(),
        EditorText::RemoveValue => "×".to_string(),
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
    use pw_metadata::write_plan::{ExifPresent, FileState, XmpPresent};

    fn state(kind: FileKind, iim_present: bool) -> FileState {
        FileState {
            kind,
            iim_present,
            iim_charset: CodedCharset::Absent,
            iim_values: Vec::new(),
            digest: IimDigest::Missing,
            exif_present: ExifPresent::default(),
            xmp_present: XmpPresent::default(),
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
                "Model Release Status",
                "Model Release Id",
                "Minor Model Age Disclosure",
                "Property Release Status",
                "Property Release Id",
                "Data Mining",
                "Digital Source Type",
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
                | EditorField::SupplementalCategories
                | EditorField::ModelReleaseId
                | EditorField::PropertyReleaseId => FieldKind::List,
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
        let expected: [(EditorSection, &[EditorField]); 6] = [
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
            (
                EditorSection::Licence,
                &[
                    ModelReleaseStatus,
                    ModelReleaseId,
                    MinorModelAge,
                    PropertyReleaseStatus,
                    PropertyReleaseId,
                    DataMining,
                    DigitalSourceType,
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
            "No new IIM checksum was written, because IIM and XMP still differ"
        );
    }

    #[test]
    fn the_qualifiers_kept_notice_text() {
        assert_eq!(
            notice_text(&WriteNotice::QualifiersKept {
                property: "dc:subject".into()
            }),
            "Keywords was not written: it has qualifiers RawStride cannot keep"
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
            (
                EditorText::ThisImageOnly,
                "OK and Save write this image only.",
            ),
            (
                EditorText::ListAdds {
                    field: EditorField::Keywords,
                    entries: strings(&["vinter", "älg"]),
                },
                "Keywords: adds vinter, älg",
            ),
            (
                EditorText::ListRemoves {
                    field: EditorField::Keywords,
                    entries: strings(&["skog"]),
                },
                "Keywords: removes skog",
            ),
            (
                EditorText::ListAddsRemoves {
                    field: EditorField::Keywords,
                    adds: strings(&["vinter", "älg"]),
                    removes: strings(&["skog"]),
                },
                "Keywords: adds vinter, älg; removes skog",
            ),
            (
                EditorText::ListRemovesHere {
                    field: EditorField::Creator,
                    entries: strings(&["Bo"]),
                },
                "Creator: removes Bo on this image only",
            ),
            (
                EditorText::NotSaved {
                    fields: vec![EditorField::Keywords],
                    lists: Vec::new(),
                },
                "Nothing was saved. Correct the error in Keywords.",
            ),
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
                "Licence (PLUS)",
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
                "ExifTool is too old (at least 13.64 is needed)",
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

    fn entries(property: &str, add: &[&str], remove: &[&str]) -> Edit {
        Edit::Entries {
            property: property.to_string(),
            add: add.iter().map(|entry| entry.to_string()).collect(),
            remove: remove.iter().map(|entry| entry.to_string()).collect(),
        }
    }

    fn strings(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|entry| entry.to_string()).collect()
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
        // The list field goes to the other images as the entries it adds (U12d-Q1, T180).
        assert_eq!(
            batch.others,
            [
                set("dc:title", lang_alt(&[("x-default", "Kväll")])),
                set("photoshop:Headline", text("Hav")),
                entries("dc:subject", &["sol", "hav"], &[]),
                set("xmpRights:Marked", text("True")),
            ],
            "City is unchanged and written nowhere"
        );
        assert_eq!(
            batch.changed,
            [
                EditorField::Title,
                EditorField::Headline,
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
                EditorField::Copyright,
                EditorField::Credit,
                EditorField::UsageTerms,
                EditorField::WebStatement,
            ]
        );
        // Creator is a list field: its removed entry stays on the window's image (U12d-Q2).
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Creator,
                adds: Vec::new(),
                removes: Vec::new(),
                removes_here: vec!["Anna".to_string()],
            }]
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

    // --- "Apply to {n} Marked" adds and removes list entries (ADR 0017 U12d, T180) ---

    fn keywords_form(start: &[&str]) -> EditorForm {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:subject", list(start));
        form_with(metadata)
    }

    #[test]
    fn an_added_keyword_is_added_to_the_other_images() {
        let mut form = keywords_form(&["sol", "hav"]);
        type_into(&mut form, EditorField::Keywords, "sol, hav, vinter");

        let batch = batch_of(&form);

        assert_eq!(batch.others, [entries("dc:subject", &["vinter"], &[])]);
        assert_eq!(
            batch.this_image,
            [set("dc:subject", list(&["sol", "hav", "vinter"]))]
        );
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Keywords,
                adds: strings(&["vinter"]),
                removes: Vec::new(),
                removes_here: Vec::new(),
            }]
        );
        assert!(batch.changed.is_empty());
        assert_eq!(batch.field_count(), 1);
    }

    #[test]
    fn a_removed_keyword_is_removed_from_the_other_images() {
        let mut form = keywords_form(&["sol", "hav"]);
        type_into(&mut form, EditorField::Keywords, "hav");

        let batch = batch_of(&form);

        assert_eq!(batch.others, [entries("dc:subject", &[], &["sol"])]);
        assert_eq!(batch.this_image, [set("dc:subject", list(&["hav"]))]);
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Keywords,
                adds: Vec::new(),
                removes: strings(&["sol"]),
                removes_here: Vec::new(),
            }]
        );
    }

    #[test]
    fn added_and_removed_keywords_in_one_edit() {
        let mut form = keywords_form(&["sol", "hav"]);
        type_into(&mut form, EditorField::Keywords, "hav, is");

        let batch = batch_of(&form);

        assert_eq!(batch.others, [entries("dc:subject", &["is"], &["sol"])]);
        assert_eq!(batch.this_image, [set("dc:subject", list(&["hav", "is"]))]);
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Keywords,
                adds: strings(&["is"]),
                removes: strings(&["sol"]),
                removes_here: Vec::new(),
            }]
        );
        assert_eq!(batch.field_count(), 1);
    }

    #[test]
    fn an_emptied_keyword_list_removes_only_its_own_entries() {
        let mut form = keywords_form(&["sol", "hav"]);
        type_into(&mut form, EditorField::Keywords, "");

        let batch = batch_of(&form);

        // Not a `Clear`: the other images keep their own other keywords.
        assert_eq!(batch.others, [entries("dc:subject", &[], &["sol", "hav"])]);
        assert_eq!(batch.this_image, [clear("dc:subject")]);
        assert!(batch.emptied.is_empty());
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Keywords,
                adds: Vec::new(),
                removes: strings(&["sol", "hav"]),
                removes_here: Vec::new(),
            }]
        );
    }

    #[test]
    fn only_the_order_changed_sends_nothing_to_the_other_images() {
        let mut form = keywords_form(&["sol", "hav"]);
        type_into(&mut form, EditorField::Keywords, "hav,  sol");

        let batch = batch_of(&form);

        assert!(batch.others.is_empty());
        assert!(batch.list_changes.is_empty());
        assert_eq!(batch.field_count(), 0);
        assert_eq!(batch.this_image, [set("dc:subject", list(&["hav", "sol"]))]);
    }

    #[test]
    fn a_removed_creator_stays_on_the_windows_image() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:creator", list(&["Anna", "Bo"]));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Creator, "Anna, Cecilia");

        let batch = batch_of(&form);

        assert_eq!(batch.others, [entries("dc:creator", &["Cecilia"], &[])]);
        assert_eq!(
            batch.this_image,
            [set("dc:creator", list(&["Anna", "Cecilia"]))]
        );
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Creator,
                adds: strings(&["Cecilia"]),
                removes: Vec::new(),
                removes_here: strings(&["Bo"]),
            }]
        );
        assert!(batch.emptied_here_only.is_empty());
        assert_eq!(batch.field_count(), 1);
    }

    #[test]
    fn the_confirmation_names_what_each_list_adds_and_removes() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Sol"));
        insert(&mut metadata, "dc:subject", list(&["skog", "sjö"]));
        insert(&mut metadata, "Iptc4xmpExt:PersonInImage", list(&["Anna"]));
        insert(&mut metadata, "dc:creator", list(&["Anna", "Bo"]));
        insert(&mut metadata, "plus:ModelReleaseID", list(&["M1", "M2"]));
        insert(&mut metadata, "photoshop:City", text("Lund"));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Headline, "Hav");
        type_into(&mut form, EditorField::Keywords, "sjö, vinter, älg");
        type_into(&mut form, EditorField::PersonShown, "Anna, Bo");
        type_into(&mut form, EditorField::Creator, "Cecilia");
        type_into(&mut form, EditorField::ModelReleaseId, "M2");
        type_into(&mut form, EditorField::City, "");

        let batch = batch_of(&form);

        assert_eq!(
            apply_confirmation(&batch, 3, 0),
            [
                "Write 6 changed fields to 3 images?",
                "Changed: Headline",
                "Keywords: adds vinter, älg; removes skog",
                "Person Shown in the Image: adds Bo",
                "Creator: adds Cecilia",
                "Creator: removes Anna, Bo on this image only",
                &format!("{}: removes M1", EditorField::ModelReleaseId.label()),
                "Emptied: City",
            ]
        );

        // Only list lines: they stand where the "Changed:" line would.
        let mut form = keywords_form(&["sol", "hav"]);
        type_into(&mut form, EditorField::Keywords, "hav");
        assert_eq!(
            apply_confirmation(&batch_of(&form), 2, 0),
            [
                "Write 1 changed field to 2 images?",
                "Keywords: removes sol",
            ]
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
            list_changes: Vec::new(),
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
            (
                EditorText::QualifiersKept {
                    file: "c.jpg".into(),
                    property: "dc:subject".into(),
                },
                "List with qualifiers not written: c.jpg (Keywords)",
            ),
            (
                EditorText::TruncatedInIim {
                    file: "d.jpg".into(),
                    property: "photoshop:Headline".into(),
                },
                "Truncated in IIM: d.jpg (Headline)",
            ),
            (
                EditorText::IimRewrittenAsUtf8 {
                    file: "e.jpg".into(),
                },
                "IIM texts rewritten as UTF-8: e.jpg",
            ),
            (
                EditorText::NoDigestWritten {
                    file: "f.jpg".into(),
                },
                "No new IIM checksum written: f.jpg",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }

    // --- the Licence (PLUS) section and its choice lists (T128, U10d) ---

    const DST: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/";

    /// The choice-list fields and their XMP ids.
    const CHOICE_FIELDS: [(EditorField, &str); 5] = [
        (EditorField::ModelReleaseStatus, "plus:ModelReleaseStatus"),
        (EditorField::MinorModelAge, "plus:MinorModelAgeDisclosure"),
        (
            EditorField::PropertyReleaseStatus,
            "plus:PropertyReleaseStatus",
        ),
        (EditorField::DataMining, "plus:DataMining"),
        (
            EditorField::DigitalSourceType,
            "Iptc4xmpExt:DigitalSourceType",
        ),
    ];

    /// The level-C sample `PM-jpg.JPG`'s PLUS values as ExifTool reads them (codes), and a
    /// Digital Source Type.
    fn licence_metadata() -> Metadata {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "plus:ModelReleaseStatus", text("MR-NAP"));
        insert(&mut metadata, "plus:PropertyReleaseStatus", text("PR-NON"));
        insert(
            &mut metadata,
            "plus:MinorModelAgeDisclosure",
            text("AG-A25"),
        );
        insert(&mut metadata, "plus:DataMining", text("DMI-UNSPECIFIED"));
        insert(
            &mut metadata,
            "Iptc4xmpExt:DigitalSourceType",
            text(&format!("{DST}digitalCapture")),
        );
        metadata
    }

    #[test]
    fn the_licence_fields_property_kind_and_choice_list() {
        use EditorField::*;
        let cases = [
            (
                ModelReleaseStatus,
                "plus:ModelReleaseStatus",
                FieldKind::Text,
                true,
            ),
            (
                ModelReleaseId,
                "plus:ModelReleaseID",
                FieldKind::List,
                false,
            ),
            (
                MinorModelAge,
                "plus:MinorModelAgeDisclosure",
                FieldKind::Text,
                true,
            ),
            (
                PropertyReleaseStatus,
                "plus:PropertyReleaseStatus",
                FieldKind::Text,
                true,
            ),
            (
                PropertyReleaseId,
                "plus:PropertyReleaseID",
                FieldKind::List,
                false,
            ),
            (DataMining, "plus:DataMining", FieldKind::Text, true),
            (
                DigitalSourceType,
                "Iptc4xmpExt:DigitalSourceType",
                FieldKind::Text,
                true,
            ),
        ];
        for (field, property, kind, choice) in cases {
            assert_eq!(field.property(), property, "{field:?}");
            assert_eq!(field.struct_field(), None, "{field:?}");
            assert_eq!(field.kind(), kind, "{field:?}");
            assert_eq!(field.section(), EditorSection::Licence, "{field:?}");
            assert_eq!(field.choice_list().is_some(), choice, "{field:?}");
            assert!(field.editable(), "{field:?}");
            assert!(!field.multiline(), "{field:?}");
            assert!(!field.check_box(), "{field:?}");
            assert!(!field.copyright_management(), "{field:?}");
        }
        // Only these five fields are choice lists; their lists are the vocabulary's.
        let choice_fields: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|field| field.choice_list().is_some())
            .collect();
        assert_eq!(
            choice_fields,
            CHOICE_FIELDS.map(|(field, _)| field).to_vec()
        );
        for (field, property) in CHOICE_FIELDS {
            assert_eq!(
                field.choice_list(),
                vocabulary::choices(property),
                "{field:?}"
            );
        }
    }

    #[test]
    fn a_choice_list_has_the_empty_entry_then_the_vocabulary_with_labels_and_full_uris() {
        let form = form_with(Metadata::new());
        for (field, property) in CHOICE_FIELDS {
            let entries = form.choice_entries(field);
            let values = vocabulary::choices(property).unwrap();
            assert_eq!(entries.len(), values.len() + 1, "{field:?}");
            assert_eq!(
                entries[0],
                ChoiceEntry {
                    value: String::new(),
                    text: String::new(),
                    tooltip: None
                },
                "{field:?}"
            );
            for (entry, value) in entries[1..].iter().zip(values) {
                assert_eq!(entry.value, *value, "{field:?}");
                assert_eq!(Some(entry.text.as_str()), choice_label(value), "{value}");
                let uri = if field == EditorField::DigitalSourceType {
                    value.to_string()
                } else {
                    format!("http://ns.useplus.org/ldf/vocab/{value}")
                };
                assert_eq!(entry.tooltip.as_deref(), Some(uri.as_str()), "{value}");
            }
        }
        // Not a choice list: no entries.
        assert!(form.choice_entries(EditorField::City).is_empty());
        assert!(form.choice_entries(EditorField::ModelReleaseId).is_empty());
    }

    /// The level-C sample's values (codes, as ExifTool reads them) are the selected entries, with
    /// no extra entry and no warning.
    #[test]
    fn the_files_codes_are_selected_without_an_extra_entry() {
        let read = read_of(licence_metadata());
        let form = EditorForm::from_read(&read);
        let digital_capture = format!("{DST}digitalCapture");
        for (field, value, label) in [
            (EditorField::ModelReleaseStatus, "MR-NAP", "Not Applicable"),
            (EditorField::PropertyReleaseStatus, "PR-NON", "None"),
            (EditorField::MinorModelAge, "AG-A25", "Age 25 or Over"),
            (
                EditorField::DataMining,
                "DMI-UNSPECIFIED",
                "Unspecified - no prohibition defined",
            ),
            (
                EditorField::DigitalSourceType,
                digital_capture.as_str(),
                "Digital capture sampled from real life",
            ),
        ] {
            assert_eq!(form.text(field), value, "{field:?}");
            let entries = form.choice_entries(field);
            let selected: Vec<&ChoiceEntry> = entries
                .iter()
                .filter(|entry| entry.value == value)
                .collect();
            assert_eq!(selected.len(), 1, "{field:?}");
            assert_eq!(selected[0].text, label, "{field:?}");
            assert_eq!(
                entries.len(),
                field.choice_list().unwrap().len() + 1,
                "{field:?}"
            );
        }
        assert_eq!(form.edits(), Ok(Vec::new()));
        assert!(form.warnings(&read, false).is_empty());
    }

    #[test]
    fn a_choice_list_is_not_typed_into() {
        let mut form = form_with(licence_metadata());
        for (field, _) in CHOICE_FIELDS {
            assert!(form.text_mut(field).is_none(), "{field:?}");
        }
        assert!(form.text_mut(EditorField::ModelReleaseId).is_some());
        // `set_choice` changes nothing on another field.
        form.set_choice(EditorField::City, "MR-NAP");
        assert_eq!(form.text(EditorField::City), "");
    }

    #[test]
    fn another_entry_is_a_set_of_its_exact_value_and_the_empty_entry_a_clear() {
        let mut form = form_with(licence_metadata());
        form.set_choice(EditorField::ModelReleaseStatus, "MR-UMR");
        form.set_choice(EditorField::MinorModelAge, "");
        form.set_choice(EditorField::DigitalSourceType, &format!("{DST}humanEdits"));
        // Chosen again as it was: no edit.
        form.set_choice(EditorField::DataMining, "DMI-UNSPECIFIED");
        assert_eq!(
            form.edits(),
            Ok(vec![
                // The PLUS code alone (ExifTool adds the prefix when it writes).
                set("plus:ModelReleaseStatus", text("MR-UMR")),
                Edit::Clear {
                    property: "plus:MinorModelAgeDisclosure".into(),
                    field: None,
                },
                // Digital Source Type: the full URI.
                set(
                    "Iptc4xmpExt:DigitalSourceType",
                    text(&format!("{DST}humanEdits"))
                ),
            ])
        );
        // An empty field given a value.
        let mut empty = form_with(Metadata::new());
        empty.set_choice(EditorField::PropertyReleaseStatus, "PR-LPR");
        assert_eq!(
            empty.edits(),
            Ok(vec![set("plus:PropertyReleaseStatus", text("PR-LPR"))])
        );
    }

    /// A value outside the vocabulary (ADR 0007 §11, U10d-Q4): its own entry, selected; kept when
    /// untouched (also while another field is edited); replaced only by another entry; a warning.
    #[test]
    fn an_unknown_value_is_its_own_entry_kept_until_replaced() {
        let mut metadata = Metadata::new();
        // The full URI is not what ExifTool reads, so it is unknown too.
        let full = "http://ns.useplus.org/ldf/vocab/MR-NAP";
        insert(&mut metadata, "plus:ModelReleaseStatus", text(full));
        insert(&mut metadata, "plus:DataMining", text("DMI-SOMETHING"));
        let read = read_of(metadata);
        let mut form = EditorForm::from_read(&read);

        let entries = form.choice_entries(EditorField::DataMining);
        assert_eq!(entries.len(), 9 + 2);
        assert_eq!(
            entries.last(),
            Some(&ChoiceEntry {
                value: "DMI-SOMETHING".into(),
                text: "Unknown value: DMI-SOMETHING".into(),
                tooltip: None,
            })
        );
        assert_eq!(form.text(EditorField::DataMining), "DMI-SOMETHING");
        assert_eq!(
            form.choice_entries(EditorField::ModelReleaseStatus)
                .last()
                .map(|entry| entry.text.clone()),
            Some(format!("Unknown value: {full}"))
        );
        // Untouched: no edit, and the warnings name both values (in table order).
        type_into(&mut form, EditorField::City, "Lund");
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Lund"))]));
        assert_eq!(
            form.warnings(&read, false),
            [
                "Data Mining DMI-SOMETHING is not in the PLUS list".to_string(),
                format!("Model Release Status {full} is not in the PLUS list"),
            ]
        );
        // Replaced by another entry: a `Set`; the unknown entry stays in the list to go back to.
        form.set_choice(EditorField::DataMining, "DMI-PROHIBITED-AIMLTRAINING");
        assert_eq!(
            form.edits(),
            Ok(vec![
                set("photoshop:City", text("Lund")),
                set("plus:DataMining", text("DMI-PROHIBITED-AIMLTRAINING")),
            ])
        );
        assert!(
            form.choice_entries(EditorField::DataMining)
                .iter()
                .any(|entry| entry.value == "DMI-SOMETHING")
        );
        assert_eq!(
            form.warnings(&read, false),
            [format!(
                "Model Release Status {full} is not in the PLUS list"
            )]
        );
        // Chosen back: no edit.
        form.set_choice(EditorField::DataMining, "DMI-SOMETHING");
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Lund"))]));
    }

    /// A retired Digital Source Type in a file: its own entry with its label and full URI,
    /// selected, no warning, kept; never offered otherwise.
    #[test]
    fn a_retired_digital_source_type_is_shown_with_its_label_and_kept() {
        let retired = format!("{DST}digitalArt");
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "Iptc4xmpExt:DigitalSourceType",
            text(&retired),
        );
        let read = read_of(metadata);
        let mut form = EditorForm::from_read(&read);
        let entries = form.choice_entries(EditorField::DigitalSourceType);
        assert_eq!(entries.len(), 17 + 2);
        assert_eq!(
            entries.last(),
            Some(&ChoiceEntry {
                value: retired.clone(),
                text: "Digital art".into(),
                tooltip: Some(retired.clone()),
            })
        );
        assert_eq!(form.edits(), Ok(Vec::new()));
        assert!(form.warnings(&read, false).is_empty());
        // Another file: no retired entry.
        let fresh = form_with(Metadata::new());
        for code in ["minorHumanEdits", "softwareImage", "digitalArt"] {
            assert!(
                fresh
                    .choice_entries(EditorField::DigitalSourceType)
                    .iter()
                    .all(|entry| entry.value != format!("{DST}{code}")),
                "{code}"
            );
        }
        // Replaced: a `Set` of the new URI.
        form.set_choice(
            EditorField::DigitalSourceType,
            &format!("{DST}digitalCreation"),
        );
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                "Iptc4xmpExt:DigitalSourceType",
                text(&format!("{DST}digitalCreation"))
            )])
        );
    }

    /// A retired Digital Source Type code is not a retired value of a PLUS field: there it is
    /// unknown.
    #[test]
    fn a_digital_source_type_value_in_a_plus_field_is_unknown() {
        let retired = format!("{DST}digitalArt");
        let mut metadata = Metadata::new();
        insert(&mut metadata, "plus:ModelReleaseStatus", text(&retired));
        let form = form_with(metadata);
        assert_eq!(
            form.choice_entries(EditorField::ModelReleaseStatus)
                .last()
                .map(|entry| (entry.text.clone(), entry.tooltip.clone())),
            Some((format!("Unknown value: {retired}"), None))
        );
    }

    #[test]
    fn the_release_id_lists_unchanged_changed_and_emptied() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "plus:ModelReleaseID",
            list(&["MR-1", "MR-2"]),
        );
        insert(&mut metadata, "plus:PropertyReleaseID", list(&["PR-1"]));
        let mut form = form_with(metadata);
        assert_eq!(form.text(EditorField::ModelReleaseId), "MR-1, MR-2");
        assert_eq!(form.text(EditorField::PropertyReleaseId), "PR-1");
        assert_eq!(form.edits(), Ok(Vec::new()));
        type_into(&mut form, EditorField::ModelReleaseId, "MR-1, MR-3 ,, ");
        type_into(&mut form, EditorField::PropertyReleaseId, " , ");
        assert_eq!(
            form.edits(),
            Ok(vec![
                set("plus:ModelReleaseID", list(&["MR-1", "MR-3"])),
                Edit::Clear {
                    property: "plus:PropertyReleaseID".into(),
                    field: None,
                },
            ])
        );
        let mut metadata = licence_metadata();
        apply_edits(&mut metadata, &form.edits().unwrap()).unwrap();
    }

    #[test]
    fn the_licence_edits_are_accepted_by_apply_edits() {
        let mut form = form_with(licence_metadata());
        form.set_choice(EditorField::ModelReleaseStatus, "MR-LMR");
        form.set_choice(EditorField::DigitalSourceType, "");
        type_into(&mut form, EditorField::PropertyReleaseId, "PR-9");
        let mut metadata = licence_metadata();
        apply_edits(&mut metadata, &form.edits().unwrap()).unwrap();
        assert_eq!(
            metadata.get("plus:ModelReleaseStatus").map(|s| &s.value),
            Some(&text("MR-LMR"))
        );
        assert_eq!(metadata.get("Iptc4xmpExt:DigitalSourceType"), None);
        assert_eq!(
            metadata.get("plus:PropertyReleaseID").map(|s| &s.value),
            Some(&list(&["PR-9"]))
        );
    }

    #[test]
    fn a_conflict_on_a_choice_field_is_decided_by_choosing_its_value() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "plus:ModelReleaseStatus", text("MR-NAP"));
        metadata.add_conflict(Conflict {
            property: "plus:ModelReleaseStatus".into(),
            values: vec![
                (Origin::XmpEmbedded, text("MR-NAP")),
                (Origin::XmpSidecar, text("MR-UMR")),
            ],
            rule: ConflictRule::Open,
        });
        let mut form = form_with(metadata);
        assert!(form.choose(0, 1));
        assert_eq!(form.text(EditorField::ModelReleaseStatus), "MR-UMR");
        assert_eq!(
            form.edits(),
            Ok(vec![set("plus:ModelReleaseStatus", text("MR-UMR"))])
        );
    }

    #[test]
    fn copy_and_paste_carry_a_changed_choice_and_the_id_lists() {
        let mut source = form_with(licence_metadata());
        source.set_choice(EditorField::ModelReleaseStatus, "MR-UMR");
        type_into(&mut source, EditorField::ModelReleaseId, "MR-7");
        let copied = source.copy("a.jpg");
        assert_eq!(copied.text(EditorField::ModelReleaseStatus), Some("MR-UMR"));
        assert_eq!(copied.text(EditorField::ModelReleaseId), Some("MR-7"));
        assert_eq!(
            copied.text(EditorField::DigitalSourceType),
            Some(format!("{DST}digitalCapture").as_str())
        );

        let mut target_metadata = Metadata::new();
        insert(
            &mut target_metadata,
            "plus:ModelReleaseStatus",
            text("MR-NON"),
        );
        insert(
            &mut target_metadata,
            "plus:PropertyReleaseStatus",
            text("PR-NAP"),
        );
        let mut target = form_with(target_metadata);
        // The copy's empty Property Release Id leaves nothing; its PR-NON replaces PR-NAP.
        target.paste(&copied);
        assert_eq!(target.text(EditorField::ModelReleaseStatus), "MR-UMR");
        assert_eq!(target.text(EditorField::PropertyReleaseStatus), "PR-NON");
        assert_eq!(
            target.edits(),
            Ok(vec![
                set("plus:ModelReleaseStatus", text("MR-UMR")),
                set("plus:ModelReleaseID", list(&["MR-7"])),
                set("plus:MinorModelAgeDisclosure", text("AG-A25")),
                set("plus:PropertyReleaseStatus", text("PR-NON")),
                set("plus:DataMining", text("DMI-UNSPECIFIED")),
                set(
                    "Iptc4xmpExt:DigitalSourceType",
                    text(&format!("{DST}digitalCapture"))
                ),
            ])
        );
    }

    /// Paste never empties a choice: an empty copied choice leaves the target's value.
    #[test]
    fn an_empty_copied_choice_leaves_the_targets_value() {
        let copied = form_with(Metadata::new()).copy("a.jpg");
        let mut target = form_with(licence_metadata());
        target.paste(&copied);
        assert_eq!(target.text(EditorField::ModelReleaseStatus), "MR-NAP");
        assert_eq!(target.edits(), Ok(Vec::new()));
    }

    #[test]
    fn apply_to_all_marked_carries_only_a_changed_choice() {
        let mut form = form_with(licence_metadata());
        form.set_choice(EditorField::DataMining, "DMI-PROHIBITED");
        form.set_choice(EditorField::ModelReleaseStatus, "");
        let batch = batch_of(&form);
        let expected = vec![
            Edit::Clear {
                property: "plus:ModelReleaseStatus".into(),
                field: None,
            },
            set("plus:DataMining", text("DMI-PROHIBITED")),
        ];
        assert_eq!(batch.this_image, expected);
        // Not a copyright-management field: the emptied choice is emptied on every image.
        assert_eq!(batch.others, expected);
        assert_eq!(batch.changed, [EditorField::DataMining]);
        assert_eq!(batch.emptied, [EditorField::ModelReleaseStatus]);
        assert!(batch.emptied_here_only.is_empty());
    }

    /// Every value of every list, the retired Digital Source Types included, has a label, none
    /// empty; the labels of point 5 of T128 are spot-checked.
    #[test]
    fn every_vocabulary_value_has_a_label() {
        for (field, property) in CHOICE_FIELDS {
            for value in vocabulary::choices(property).unwrap() {
                let label = choice_label(value);
                assert!(label.is_some_and(|l| !l.is_empty()), "{field:?} {value}");
            }
        }
        for uri in vocabulary::RETIRED_DIGITAL_SOURCE_TYPE_URIS {
            assert!(choice_label(uri).is_some_and(|l| !l.is_empty()), "{uri}");
        }
        for (value, label) in [
            ("MR-LMR", "Limited or Incomplete Model Releases"),
            ("PR-UPR", "Unlimited Property Releases"),
            ("AG-A19", "Age 19"),
            ("AG-U14", "Age 14 or Under"),
            (
                "DMI-PROHIBITED-SEEEMBEDDEDRIGHTSEXPR",
                "Prohibited, see Embedded Encoded Rights Expression property",
            ),
            (
                "DMI-PROHIBITED-GENAIMLTRAINING",
                "Prohibited for Generative AI/ML training",
            ),
        ] {
            assert_eq!(choice_label(value), Some(label), "{value}");
        }
        for (code, label) in [
            (
                "compositeWithTrainedAlgorithmicMedia",
                "Edited using Generative AI",
            ),
            ("print", "Digitised from a non-transparent medium"),
            ("minorHumanEdits", "Original media with minor human edits"),
            ("softwareImage", "Created by software"),
        ] {
            assert_eq!(choice_label(&format!("{DST}{code}")), Some(label), "{code}");
        }
        // Not a vocabulary value: no label (a full PLUS URI, a bare Digital Source Type code).
        for value in [
            "",
            "MR-XXX",
            "http://ns.useplus.org/ldf/vocab/MR-NAP",
            "digitalCapture",
            "mr-nap",
        ] {
            assert_eq!(choice_label(value), None, "{value:?}");
        }
    }

    #[test]
    fn the_unknown_value_and_warning_texts() {
        assert_eq!(
            editor_text(&EditorText::UnknownValue {
                value: "MR-XXX".into()
            }),
            "Unknown value: MR-XXX"
        );
        assert_eq!(
            warning_text(&Warning::VocabularyValueUnknown {
                property: "plus:MinorModelAgeDisclosure".into(),
                value: "AG-99".into(),
            }),
            "Minor Model Age Disclosure AG-99 is not in the PLUS list"
        );
        assert_eq!(
            editor_text(&EditorText::SectionHeading(EditorSection::Licence)),
            "Licence (PLUS)"
        );
    }

    #[test]
    fn the_newscodes_warning_texts() {
        assert_eq!(
            warning_text(&Warning::NewsCodeUnknown {
                property: "Iptc4xmpCore:Scene".into(),
                value: "999999".into(),
            }),
            format!(
                "{} 999999 is not in IPTC's list",
                property_label("Iptc4xmpCore:Scene", Language::En)
            )
        );
        assert_eq!(
            warning_text(&Warning::SubjectCodeForm {
                code: "4000000".into()
            }),
            format!(
                "{} 4000000 should be eight digits",
                property_label("Iptc4xmpCore:SubjectCode", Language::En)
            )
        );
    }

    // --- Location Created and Location Shown (ADR 0017 U11a, T136) ---

    const CREATED: &str = "Iptc4xmpExt:LocationCreated";
    const SHOWN: &str = "Iptc4xmpExt:LocationShown";
    const SUBLOCATION: &str = "Iptc4xmpExt:Sublocation";
    const CITY: &str = "Iptc4xmpExt:City";
    const STATE: &str = "Iptc4xmpExt:ProvinceState";
    const COUNTRY: &str = "Iptc4xmpExt:CountryName";
    const CODE: &str = "Iptc4xmpExt:CountryCode";
    const REGION: &str = "Iptc4xmpExt:WorldRegion";
    const ID: &str = "Iptc4xmpExt:LocationId";
    const NAME: &str = "Iptc4xmpExt:LocationName";

    /// A structure of text sub-fields.
    fn place(fields: &[(&str, &str)]) -> StructValue {
        StructValue {
            fields: fields
                .iter()
                .map(|(id, t)| (id.to_string(), text(t)))
                .collect(),
        }
    }

    fn places(entries: Vec<StructValue>) -> Value {
        Value::StructList(entries)
    }

    /// Umeå with every shown sub-field (Location Identifier a list of two URIs).
    fn umea() -> StructValue {
        let mut umea = place(&[
            (SUBLOCATION, "Rådhustorget"),
            (CITY, "Umeå"),
            (STATE, "Västerbotten"),
            (COUNTRY, "Sverige"),
            (CODE, "SE"),
            (REGION, "Europa"),
        ]);
        umea.fields.push((
            ID.to_string(),
            list(&["https://example.org/umea", "https://example.org/se"]),
        ));
        umea
    }

    /// Oslo with a city and a country.
    fn oslo() -> StructValue {
        place(&[(CITY, "Oslo"), (COUNTRY, "Norge")])
    }

    fn located(created: Vec<StructValue>, shown: Vec<StructValue>) -> Metadata {
        let mut metadata = Metadata::new();
        if !created.is_empty() {
            insert(&mut metadata, CREATED, places(created));
        }
        if !shown.is_empty() {
            insert(&mut metadata, SHOWN, places(shown));
        }
        metadata
    }

    fn type_location(form: &mut EditorForm, list: &str, entry: usize, field: &str, text: &str) {
        *form.location_text_mut(list, entry, field).unwrap() = text.to_string();
    }

    #[test]
    fn the_location_lists_and_their_seven_fields_with_names() {
        assert_eq!(EditorForm::LOCATION_LISTS, [CREATED, SHOWN]);
        assert_eq!(EditorForm::LOCATION_CREATED, CREATED);
        assert_eq!(EditorForm::LOCATION_SHOWN, SHOWN);
        assert_eq!(
            EditorForm::LOCATION_FIELDS,
            [SUBLOCATION, CITY, STATE, COUNTRY, CODE, REGION, ID]
        );
        // Names from `property_label` (IPTC's names from the property table, U11a-Q6).
        let names: Vec<String> = EditorForm::LOCATION_LISTS
            .iter()
            .chain(EditorForm::LOCATION_FIELDS.iter())
            .map(|id| property_label(id, Language::En))
            .collect();
        assert_eq!(
            names,
            [
                "Location Created",
                "Location Shown in the Image",
                "Sublocation",
                "City",
                "Province or State",
                "Country Name",
                "Country ISO-Code",
                "World Region",
                "Location Identifier",
            ]
        );
        // The lists come after Country Code, the last of the older fields (U11a-Q2).
        assert_eq!(
            EditorField::ALL
                .into_iter()
                .rfind(|field| field.section() == EditorSection::TimePlace),
            Some(LOCATIONS_AFTER)
        );
    }

    #[test]
    fn a_read_list_is_shown_as_its_entries_with_the_seven_fields() {
        let form = form_with(located(vec![umea(), oslo()], Vec::new()));
        assert_eq!(form.location_count(CREATED), 2);
        assert_eq!(form.location_count(SHOWN), 0);
        let texts: Vec<&str> = EditorForm::LOCATION_FIELDS
            .iter()
            .map(|field| form.location_text(CREATED, 0, field))
            .collect();
        assert_eq!(
            texts,
            [
                "Rådhustorget",
                "Umeå",
                "Västerbotten",
                "Sverige",
                "SE",
                "Europa",
                // Location Identifier on one line, separated by commas (U10-Q2).
                "https://example.org/umea, https://example.org/se",
            ]
        );
        assert_eq!(form.location_text(CREATED, 1, CITY), "Oslo");
        assert_eq!(form.location_text(CREATED, 1, SUBLOCATION), "");
        // No such entry or field.
        assert_eq!(form.location_text(CREATED, 2, CITY), "");
        assert_eq!(form.location_text(CREATED, 0, NAME), "");
        assert_eq!(form.location_read_only(CREATED), None);
        // Shown untouched, the lists give no edit.
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn an_edited_field_gives_one_struct_list_edit_with_the_other_entries_unchanged() {
        let mut form = form_with(located(vec![umea(), oslo()], vec![oslo()]));
        type_location(&mut form, CREATED, 1, CITY, "Bergen");
        type_location(&mut form, CREATED, 1, REGION, "Europa");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                CREATED,
                places(vec![
                    umea(),
                    place(&[(CITY, "Bergen"), (COUNTRY, "Norge"), (REGION, "Europa")]),
                ])
            )])
        );
    }

    #[test]
    fn location_identifier_is_split_at_commas_and_emptied_by_commas_only() {
        let mut form = form_with(located(vec![umea()], Vec::new()));
        type_location(
            &mut form,
            CREATED,
            0,
            ID,
            " https://a.example ,, https://b.example ",
        );
        let mut expected = umea();
        if let Some(slot) = expected.fields.iter_mut().find(|(id, _)| id == ID) {
            slot.1 = list(&["https://a.example", "https://b.example"]);
        }
        assert_eq!(form.edits(), Ok(vec![set(CREATED, places(vec![expected]))]));

        type_location(&mut form, CREATED, 0, ID, " , ,");
        let mut expected = umea();
        expected.fields.retain(|(id, _)| id != ID);
        assert_eq!(form.edits(), Ok(vec![set(CREATED, places(vec![expected]))]));
    }

    #[test]
    fn add_then_ok_with_nothing_typed_gives_no_edit() {
        let mut form = form_with(Metadata::new());
        form.add_location(CREATED);
        form.add_location(SHOWN);
        assert_eq!(form.location_count(CREATED), 1);
        assert_eq!(form.location_text(CREATED, 0, CITY), "");
        assert_eq!(form.edits(), Ok(Vec::new()));
        // Spaces only are empty too.
        type_location(&mut form, SHOWN, 0, CITY, "   ");
        assert_eq!(form.edits(), Ok(Vec::new()));

        // An empty entry added to a read list is dropped when the edits are made.
        let mut form = form_with(located(vec![oslo()], Vec::new()));
        form.add_location(CREATED);
        assert_eq!(form.edits(), Ok(Vec::new()));
        type_location(&mut form, CREATED, 1, CITY, "Lund");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                CREATED,
                places(vec![oslo(), place(&[(CITY, "Lund")])])
            )])
        );
    }

    #[test]
    fn an_added_entry_on_an_empty_list_is_written() {
        let mut form = form_with(Metadata::new());
        form.add_location(SHOWN);
        type_location(&mut form, SHOWN, 0, CITY, "Lund");
        type_location(&mut form, SHOWN, 0, CODE, "SE");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                SHOWN,
                places(vec![place(&[(CITY, "Lund"), (CODE, "SE")])])
            )])
        );
    }

    #[test]
    fn removing_every_entry_clears_and_removing_one_keeps_the_order() {
        let stockholm = place(&[(CITY, "Stockholm")]);
        let mut form = form_with(located(vec![umea(), oslo(), stockholm.clone()], Vec::new()));
        form.remove_location(CREATED, 1);
        assert_eq!(form.location_count(CREATED), 2);
        assert_eq!(
            form.edits(),
            Ok(vec![set(CREATED, places(vec![umea(), stockholm]))])
        );
        // No such entry: nothing changes.
        form.remove_location(CREATED, 5);
        assert_eq!(form.location_count(CREATED), 2);
        form.remove_location(CREATED, 0);
        form.remove_location(CREATED, 0);
        assert_eq!(form.location_count(CREATED), 0);
        assert_eq!(form.edits(), Ok(vec![clear(CREATED)]));

        // Emptying every field of every entry clears too.
        let mut form = form_with(located(vec![oslo()], Vec::new()));
        type_location(&mut form, CREATED, 0, CITY, "");
        type_location(&mut form, CREATED, 0, COUNTRY, " ");
        assert_eq!(form.edits(), Ok(vec![clear(CREATED)]));
    }

    #[test]
    fn unknown_sub_fields_are_kept_through_an_edit() {
        let mut entry = place(&[(CITY, "Umeå")]);
        entry.fields.insert(
            0,
            (NAME.to_string(), lang_alt(&[("x-default", "Rådhuset")])),
        );
        entry
            .fields
            .push(("exif:GPSAltitude".to_string(), Value::Number("12".into())));
        let mut form = form_with(located(vec![entry.clone()], Vec::new()));
        type_location(&mut form, CREATED, 0, CITY, "Luleå");
        type_location(&mut form, CREATED, 0, COUNTRY, "Sverige");
        let mut expected = entry.clone();
        expected.fields[1].1 = text("Luleå");
        expected.fields.push((COUNTRY.to_string(), text("Sverige")));
        assert_eq!(form.edits(), Ok(vec![set(CREATED, places(vec![expected]))]));

        // An entry with such fields is never dropped, also with every shown field emptied.
        type_location(&mut form, CREATED, 0, CITY, "");
        type_location(&mut form, CREATED, 0, COUNTRY, "");
        let mut expected = entry;
        expected.fields.remove(1);
        assert_eq!(form.edits(), Ok(vec![set(CREATED, places(vec![expected]))]));
    }

    #[test]
    fn copy_from_location_created_adds_an_entry_and_changes_no_other() {
        // Not enabled without a Location Created entry that is not empty.
        let mut form = form_with(Metadata::new());
        assert!(!form.can_copy_location_created());
        form.add_location(CREATED);
        assert!(!form.can_copy_location_created());
        form.copy_location_created();
        assert_eq!(form.location_count(SHOWN), 0);

        let mut form = form_with(located(vec![umea(), oslo()], vec![oslo()]));
        assert!(form.can_copy_location_created());
        form.copy_location_created();
        assert_eq!(form.location_count(SHOWN), 2);
        for field in EditorForm::LOCATION_FIELDS {
            assert_eq!(
                form.location_text(SHOWN, 1, field),
                form.location_text(CREATED, 0, field),
                "{field}"
            );
        }
        // The existing Location Shown entry and Location Created are unchanged.
        assert_eq!(
            form.edits(),
            Ok(vec![set(SHOWN, places(vec![oslo(), umea()]))])
        );

        // An unsaved entry is copied as the form shows it; an empty first entry is skipped.
        let mut form = form_with(Metadata::new());
        form.add_location(CREATED);
        form.add_location(CREATED);
        type_location(&mut form, CREATED, 1, CITY, "Lund");
        assert!(form.can_copy_location_created());
        form.copy_location_created();
        assert_eq!(form.location_count(SHOWN), 1);
        assert_eq!(form.location_text(SHOWN, 0, CITY), "Lund");
    }

    fn created_conflict() -> Metadata {
        let mut metadata = located(vec![umea()], Vec::new());
        metadata.add_conflict(Conflict {
            property: CREATED.into(),
            values: vec![
                (Origin::XmpEmbedded, places(vec![umea()])),
                (Origin::XmpSidecar, places(vec![oslo()])),
            ],
            rule: ConflictRule::Open,
        });
        metadata
    }

    #[test]
    fn a_list_with_an_open_conflict_is_read_only_until_it_is_decided() {
        let mut form = form_with(created_conflict());
        let read_text = value_text(&places(vec![umea()]));
        assert_eq!(form.location_read_only(CREATED), Some(read_text.as_str()));
        assert_eq!(form.location_read_only(SHOWN), None);
        assert!(form.location_text_mut(CREATED, 0, CITY).is_none());
        form.add_location(CREATED);
        form.remove_location(CREATED, 0);
        assert_eq!(form.location_count(CREATED), 1);
        assert_eq!(form.edits(), Ok(Vec::new()));
        // Not copied either, so it is not pasted elsewhere.
        assert!(
            form.copy("a.jpg")
                .locations
                .iter()
                .all(|(list, _)| *list != CREATED)
        );

        // Choosing a value shows its entries and writes it.
        assert!(form.choose(0, 1));
        assert_eq!(form.location_read_only(CREATED), None);
        assert_eq!(form.location_text(CREATED, 0, CITY), "Oslo");
        assert_eq!(form.edits(), Ok(vec![set(CREATED, places(vec![oslo()]))]));
        // An edit after the choice gives the list's edit instead of the chosen value.
        type_location(&mut form, CREATED, 0, CITY, "Bergen");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                CREATED,
                places(vec![place(&[(CITY, "Bergen"), (COUNTRY, "Norge")])])
            )])
        );
    }

    #[test]
    fn copy_and_paste_carry_each_list_as_one_field_that_replaces_the_whole_list() {
        let mut source = form_with(located(vec![umea()], Vec::new()));
        // An unsaved entry is copied; an empty one is dropped.
        source.add_location(SHOWN);
        source.add_location(SHOWN);
        type_location(&mut source, SHOWN, 1, CITY, "Lund");
        let copied = source.copy("a.jpg");
        assert_eq!(
            copied.locations,
            [
                (CREATED, vec![umea()]),
                (SHOWN, vec![place(&[(CITY, "Lund")])]),
            ]
        );
        // Each list with an entry is one field.
        assert_eq!(copied.count(), 2);

        let mut target = form_with(located(vec![oslo(), oslo()], vec![oslo()]));
        target.paste(&copied);
        assert_eq!(target.location_count(CREATED), 1);
        assert_eq!(target.location_text(CREATED, 0, CITY), "Umeå");
        assert_eq!(
            target.edits(),
            Ok(vec![
                set(CREATED, places(vec![umea()])),
                set(SHOWN, places(vec![place(&[(CITY, "Lund")])])),
            ])
        );

        // An empty list is not pasted (U12b-Q2): the target keeps its list.
        let copied = form_with(Metadata::new()).copy("b.jpg");
        assert_eq!(copied.count(), 0);
        let mut target = form_with(located(vec![oslo()], Vec::new()));
        target.paste(&copied);
        assert_eq!(target.location_count(CREATED), 1);
        assert_eq!(target.edits(), Ok(Vec::new()));

        // A list shown read-only on the target is left as it is.
        let copied = form_with(located(vec![oslo()], Vec::new())).copy("c.jpg");
        let mut target = form_with(created_conflict());
        target.paste(&copied);
        assert_eq!(target.edits(), Ok(Vec::new()));
    }

    #[test]
    fn copy_shown_leaves_out_a_hidden_field_and_a_hidden_list() {
        let mut source = form_with(located(vec![umea()], vec![oslo()]));
        type_into(&mut source, EditorField::City, "Lund");
        type_into(&mut source, EditorField::Credit, "Anna");
        let mut layout = EditorLayout::default();
        layout.set_item_shown(EditorItem::Field(EditorField::City), false);
        layout.set_item_shown(EditorItem::List(SHOWN), false);

        let copied = source.copy_shown("a.jpg", &layout);
        assert_eq!(copied.text(EditorField::City), None);
        assert_eq!(copied.text(EditorField::Credit), Some("Anna"));
        assert_eq!(copied.locations, [(CREATED, vec![umea()])]);
        // Credit and Location Created; City and Location Shown are not counted.
        assert_eq!(copied.count(), 2);
        // With every item shown, it is `copy`.
        assert_eq!(
            source.copy_shown("a.jpg", &EditorLayout::default()),
            source.copy("a.jpg")
        );
        assert_eq!(source.copy("a.jpg").count(), 4);
    }

    #[test]
    fn paste_shown_leaves_a_hidden_field_and_a_hidden_list_as_they_are() {
        let mut source = form_with(located(vec![umea()], vec![umea()]));
        type_into(&mut source, EditorField::City, "Lund");
        type_into(&mut source, EditorField::Credit, "Anna");
        // Made with every item shown, so it holds City and Location Shown.
        let copied = source.copy("a.jpg");
        assert_eq!(copied.text(EditorField::City), Some("Lund"));

        let mut layout = EditorLayout::default();
        layout.set_item_shown(EditorItem::Field(EditorField::City), false);
        layout.set_item_shown(EditorItem::List(SHOWN), false);
        let mut target_metadata = located(vec![oslo()], vec![oslo()]);
        insert(&mut target_metadata, "photoshop:City", text("Malmö"));
        let mut target = form_with(target_metadata);
        target.paste_shown(&copied, &layout);
        // The hidden ones as they were.
        assert_eq!(target.text(EditorField::City), "Malmö");
        assert_eq!(target.location_text(SHOWN, 0, CITY), "Oslo");
        // The shown ones pasted.
        assert_eq!(target.text(EditorField::Credit), "Anna");
        assert_eq!(target.location_text(CREATED, 0, CITY), "Umeå");
        assert_eq!(
            target.edits(),
            Ok(vec![
                set("photoshop:Credit", text("Anna")),
                set(CREATED, places(vec![umea()])),
            ])
        );
    }

    #[test]
    fn paste_shown_leaves_a_hidden_sections_fields_as_they_are() {
        let mut source = form_with(located(vec![umea()], Vec::new()));
        type_into(&mut source, EditorField::City, "Lund");
        type_into(&mut source, EditorField::Credit, "Anna");
        let copied = source.copy("a.jpg");

        let mut layout = EditorLayout::default();
        layout.set_section_shown(EditorSection::TimePlace, false);
        let mut target = form_with(Metadata::new());
        target.paste_shown(&copied, &layout);
        // City and Location Created are in Time and Place.
        assert_eq!(target.text(EditorField::City), "");
        assert_eq!(target.location_count(CREATED), 0);
        assert_eq!(target.text(EditorField::Credit), "Anna");
        assert_eq!(
            target.edits(),
            Ok(vec![set("photoshop:Credit", text("Anna"))])
        );
        // And a copy with the section hidden leaves them out.
        let copied = source.copy_shown("a.jpg", &layout);
        assert_eq!(copied.text(EditorField::City), None);
        assert!(copied.locations.is_empty());
        assert_eq!(copied.count(), 1);
    }

    #[test]
    fn copy_then_paste_of_lists_on_the_same_form_gives_no_edits() {
        let mut form = form_with(located(vec![umea(), oslo()], vec![oslo()]));
        let copied = form.copy("a.jpg");
        form.paste(&copied);
        assert_eq!(form.edits(), Ok(Vec::new()));
    }

    #[test]
    fn apply_to_all_marked_replaces_a_changed_list_and_empties_an_emptied_one() {
        let mut form = form_with(located(vec![umea(), oslo()], vec![oslo()]));
        type_into(&mut form, EditorField::Headline, "Hav");
        type_into(&mut form, EditorField::Instructions, "Embargo");
        type_location(&mut form, CREATED, 1, CITY, "Bergen");
        form.remove_location(SHOWN, 0);

        let batch = batch_of(&form);

        let created = places(vec![umea(), place(&[(CITY, "Bergen"), (COUNTRY, "Norge")])]);
        let expected = vec![
            set("photoshop:Headline", text("Hav")),
            set(CREATED, created),
            clear(SHOWN),
            set("photoshop:Instructions", text("Embargo")),
        ];
        assert_eq!(batch.this_image, expected);
        // The whole list goes to every image, and the emptied list is emptied on every image.
        assert_eq!(batch.others, expected);
        assert_eq!(batch.field_count(), 4);
        assert_eq!(
            apply_confirmation(&batch, 3, 0),
            [
                "Write 4 changed fields to 3 images?",
                "Changed: Headline, Location Created, Instructions",
                "Emptied: Location Shown in the Image",
            ]
        );

        // An added entry left empty changes nothing anywhere.
        let mut form = form_with(Metadata::new());
        form.add_location(CREATED);
        let batch = batch_of(&form);
        assert!(batch.others.is_empty());
        assert_eq!(batch.field_count(), 0);
    }

    #[test]
    fn a_chosen_location_conflict_goes_only_to_the_windows_image() {
        let mut form = form_with(created_conflict());
        assert!(form.choose(0, 1));
        let batch = batch_of(&form);
        assert_eq!(batch.this_image, [set(CREATED, places(vec![oslo()]))]);
        assert!(batch.others.is_empty());
        assert_eq!(batch.field_count(), 0);
    }

    #[test]
    fn the_location_edits_are_accepted_by_apply_edits() {
        let mut form = form_with(located(vec![umea()], vec![oslo()]));
        type_location(&mut form, CREATED, 0, CITY, "Luleå");
        form.remove_location(SHOWN, 0);
        let edits = form.edits().unwrap();
        let mut metadata = located(vec![umea()], vec![oslo()]);
        assert!(apply_edits(&mut metadata, &edits).is_ok());
        let mut expected = umea();
        expected.fields[1].1 = text("Luleå");
        assert_eq!(
            metadata.get(CREATED).map(|stored| &stored.value),
            Some(&places(vec![expected]))
        );
        assert!(metadata.get(SHOWN).is_none());
    }

    #[test]
    fn the_location_texts() {
        assert_eq!(editor_text(&EditorText::AddLocation), "Add Location");
        assert_eq!(editor_text(&EditorText::RemoveLocation), "Remove");
        assert_eq!(
            editor_text(&EditorText::CopyFromLocationCreated),
            "Copy from Location Created"
        );
        let cases = [
            (
                EditorText::Changed {
                    fields: vec![EditorField::Title, EditorField::Instructions],
                    lists: vec![CREATED, SHOWN],
                },
                "Changed: Title, Location Created, Location Shown in the Image, Instructions",
            ),
            (
                EditorText::Changed {
                    fields: vec![EditorField::Headline],
                    lists: Vec::new(),
                },
                "Changed: Headline",
            ),
            (
                EditorText::Emptied {
                    fields: Vec::new(),
                    lists: vec![SHOWN],
                },
                "Emptied: Location Shown in the Image",
            ),
            (
                EditorText::Emptied {
                    fields: vec![EditorField::City, EditorField::Country],
                    lists: Vec::new(),
                },
                "Emptied: City, Country",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }

    // --- Genre and the PLUS structures (ADR 0017 U11b, T139) ---

    const GENRE_LIST: &str = "Iptc4xmpExt:Genre";
    const CV_NAME: &str = "Iptc4xmpExt:CvTermName";
    const CV_ID: &str = "Iptc4xmpExt:CvTermId";
    const CV_CV_ID: &str = "Iptc4xmpExt:CvId";
    const CREATOR_LIST: &str = "plus:ImageCreator";
    const CREATOR_NAME: &str = "plus:ImageCreatorName";
    const CREATOR_ID: &str = "plus:ImageCreatorID";
    const OWNER_LIST: &str = "plus:CopyrightOwner";
    const OWNER_NAME: &str = "plus:CopyrightOwnerName";
    const OWNER_ID: &str = "plus:CopyrightOwnerID";
    const LICENSOR_LIST: &str = "plus:Licensor";
    const LICENSOR_NAME: &str = "plus:LicensorName";
    const LICENSOR_EMAIL: &str = "plus:LicensorEmail";
    const TEL_1: &str = "plus:LicensorTelephone1";
    const TEL_TYPE_1: &str = "plus:LicensorTelephoneType1";
    const TEL_2: &str = "plus:LicensorTelephone2";
    const TEL_TYPE_2: &str = "plus:LicensorTelephoneType2";
    const STREET: &str = "plus:LicensorStreetAddress";
    const LICENSOR_CITY: &str = "plus:LicensorCity";
    const POSTAL_CODE: &str = "plus:LicensorPostalCode";
    const SUPPLIER_LIST: &str = "plus:ImageSupplier";
    const SUPPLIER_NAME: &str = "plus:ImageSupplierName";
    const SUPPLIER_ID: &str = "plus:ImageSupplierID";

    /// A Genre entry: a name in `x-default` and Swedish, a term ID and a CV ID.
    fn sport() -> StructValue {
        StructValue {
            fields: vec![
                (
                    CV_NAME.to_string(),
                    lang_alt(&[("x-default", "Sport"), ("sv", "Idrott")]),
                ),
                (
                    CV_ID.to_string(),
                    text("http://cv.iptc.org/newscodes/genre/Sport"),
                ),
                (
                    CV_CV_ID.to_string(),
                    text("http://cv.iptc.org/newscodes/genre/"),
                ),
            ],
        }
    }

    /// A Licensor with a telephone and its type, an unknown second type and an address.
    fn licensor() -> StructValue {
        place(&[
            (LICENSOR_NAME, "Bildbyrån AB"),
            (TEL_1, "+46 90 12 34 56"),
            (TEL_TYPE_1, "cell"),
            (TEL_TYPE_2, "mobile"),
            (STREET, "Storgatan 1"),
            (LICENSOR_CITY, "Umeå"),
            (POSTAL_CODE, "903 26"),
        ])
    }

    fn with_lists(lists: &[(&str, Vec<StructValue>)]) -> Metadata {
        let mut metadata = Metadata::new();
        for (list, entries) in lists {
            insert(&mut metadata, list, places(entries.clone()));
        }
        metadata
    }

    fn type_entry(form: &mut EditorForm, list: &str, entry: usize, field: &str, text: &str) {
        *form.list_text_mut(list, entry, field).unwrap() = text.to_string();
    }

    #[test]
    fn the_seven_structure_lists_their_places_and_sub_fields_with_names() {
        assert_eq!(
            EditorForm::STRUCTURE_LISTS,
            [
                CREATED,
                SHOWN,
                GENRE_LIST,
                CREATOR_LIST,
                OWNER_LIST,
                LICENSOR_LIST,
                SUPPLIER_LIST
            ]
        );
        assert_eq!(EditorForm::IMAGE_CREATOR, CREATOR_LIST);
        // The locations after Country Code, Genre after Scene Code, the PLUS lists at the start of
        // Licence (PLUS), before the release fields (point 2).
        assert_eq!(
            EditorForm::lists_at(EditorSection::TimePlace, Some(EditorField::CountryCode)),
            [CREATED, SHOWN]
        );
        assert_eq!(
            EditorForm::lists_at(EditorSection::Assignment, Some(EditorField::SceneCode)),
            [GENRE_LIST]
        );
        assert_eq!(
            EditorForm::lists_at(EditorSection::Licence, None),
            [CREATOR_LIST, OWNER_LIST, LICENSOR_LIST, SUPPLIER_LIST]
        );
        assert!(EditorForm::lists_at(EditorSection::Content, None).is_empty());
        assert!(
            EditorForm::lists_at(
                EditorSection::Licence,
                Some(EditorField::ModelReleaseStatus)
            )
            .is_empty()
        );
        assert_eq!(
            EditorField::ALL
                .into_iter()
                .find(|field| field.section() == EditorSection::Licence),
            Some(EditorField::ModelReleaseStatus)
        );

        // The sub-fields in the order of the form, names from `property_label` (IPTC's names).
        let names = |fields: &[&str]| -> Vec<String> {
            fields
                .iter()
                .map(|id| property_label(id, Language::En))
                .collect()
        };
        assert_eq!(
            EditorForm::list_fields(CREATED),
            EditorForm::LOCATION_FIELDS
        );
        assert_eq!(
            names(EditorForm::list_fields(GENRE_LIST)),
            ["CV-Term name", "CV-Term ID", "CV-Term CV ID"]
        );
        assert_eq!(
            names(EditorForm::list_fields(CREATOR_LIST)),
            ["Image Creator Name", "Image Creator ID"]
        );
        assert_eq!(
            names(EditorForm::list_fields(OWNER_LIST)),
            ["Copyright Owner Name", "Copyright Owner ID"]
        );
        assert_eq!(
            names(EditorForm::list_fields(SUPPLIER_LIST)),
            ["Image Supplier Name", "Image Supplier ID"]
        );
        // Licensor (U11b-Q3): eight open, the six of the address under "Address".
        assert_eq!(
            EditorForm::list_fields(LICENSOR_LIST),
            [
                LICENSOR_NAME,
                "plus:LicensorID",
                LICENSOR_EMAIL,
                "plus:LicensorURL",
                TEL_1,
                TEL_TYPE_1,
                TEL_2,
                TEL_TYPE_2,
            ]
        );
        assert_eq!(
            EditorForm::list_address_fields(LICENSOR_LIST),
            [
                STREET,
                "plus:LicensorExtendedAddress",
                LICENSOR_CITY,
                "plus:LicensorRegion",
                POSTAL_CODE,
                "plus:LicensorCountry",
            ]
        );
        for list in EditorForm::STRUCTURE_LISTS {
            if list != LICENSOR_LIST {
                assert!(EditorForm::list_address_fields(list).is_empty(), "{list}");
            }
        }
        // The list names.
        assert_eq!(
            names(&EditorForm::STRUCTURE_LISTS[2..]),
            [
                "Genre",
                "Image Creator",
                "Copyright Owner",
                "Licensor",
                "Image Supplier"
            ]
        );
        // Another property has no sub-fields.
        assert!(EditorForm::list_fields("dc:title").is_empty());
    }

    #[test]
    fn each_new_list_is_read_edited_and_written_as_one_struct_list_edit() {
        let anna = place(&[(CREATOR_NAME, "Anna Berg"), (CREATOR_ID, "urn:anna")]);
        let owner = place(&[(OWNER_NAME, "Anna Berg AB")]);
        let supplier = place(&[(SUPPLIER_NAME, "Bildbyrån"), (SUPPLIER_ID, "B-1")]);
        let metadata = with_lists(&[
            (GENRE_LIST, vec![sport()]),
            (CREATOR_LIST, vec![anna.clone()]),
            (OWNER_LIST, vec![owner.clone()]),
            (LICENSOR_LIST, vec![licensor()]),
            (SUPPLIER_LIST, vec![supplier.clone()]),
        ]);
        let mut form = form_with(metadata);
        for list in &EditorForm::STRUCTURE_LISTS[2..] {
            assert_eq!(form.list_count(list), 1, "{list}");
            assert_eq!(form.list_read_only(list), None, "{list}");
        }
        assert_eq!(form.list_text(GENRE_LIST, 0, CV_NAME), "Sport");
        assert_eq!(
            form.list_text(GENRE_LIST, 0, CV_CV_ID),
            "http://cv.iptc.org/newscodes/genre/"
        );
        assert_eq!(form.list_text(CREATOR_LIST, 0, CREATOR_ID), "urn:anna");
        assert_eq!(form.list_text(OWNER_LIST, 0, OWNER_ID), "");
        assert_eq!(form.list_text(LICENSOR_LIST, 0, LICENSOR_CITY), "Umeå");
        assert_eq!(form.list_text(SUPPLIER_LIST, 0, SUPPLIER_ID), "B-1");
        // Shown untouched, the lists give no edit.
        assert_eq!(form.edits(), Ok(Vec::new()));

        type_entry(
            &mut form,
            GENRE_LIST,
            0,
            CV_ID,
            "http://example.org/genre/x",
        );
        type_entry(&mut form, CREATOR_LIST, 0, CREATOR_NAME, "Anna Berg-Ek");
        type_entry(&mut form, OWNER_LIST, 0, OWNER_ID, "urn:owner");
        type_entry(&mut form, LICENSOR_LIST, 0, LICENSOR_NAME, "Bildbyrån");
        type_entry(&mut form, SUPPLIER_LIST, 0, SUPPLIER_ID, "");

        let mut genre = sport();
        genre.fields[1].1 = text("http://example.org/genre/x");
        let mut creator = anna;
        creator.fields[0].1 = text("Anna Berg-Ek");
        let mut owner = owner;
        owner.fields.push((OWNER_ID.to_string(), text("urn:owner")));
        let mut licensor = licensor();
        licensor.fields[0].1 = text("Bildbyrån");
        let supplier = place(&[(SUPPLIER_NAME, "Bildbyrån")]);
        // One `Set` of the whole list each, Genre in Assignment before the PLUS lists.
        assert_eq!(
            form.edits(),
            Ok(vec![
                set(GENRE_LIST, places(vec![genre])),
                set(CREATOR_LIST, places(vec![creator])),
                set(OWNER_LIST, places(vec![owner])),
                set(LICENSOR_LIST, places(vec![licensor])),
                set(SUPPLIER_LIST, places(vec![supplier])),
            ])
        );
    }

    #[test]
    fn the_new_list_edits_are_accepted_by_apply_edits() {
        let mut form = form_with(Metadata::new());
        for list in &EditorForm::STRUCTURE_LISTS[2..] {
            form.add_entry(list);
            let field = EditorForm::list_fields(list)[0];
            type_entry(&mut form, list, 0, field, "Namn");
        }
        let edits = form.edits().unwrap();
        assert_eq!(edits.len(), 5);
        let mut metadata = Metadata::new();
        assert!(apply_edits(&mut metadata, &edits).is_ok());
        assert_eq!(
            metadata.get(GENRE_LIST).map(|stored| &stored.value),
            Some(&places(vec![StructValue {
                fields: vec![(CV_NAME.to_string(), lang_alt(&[("x-default", "Namn")]))],
            }]))
        );
        assert_eq!(
            metadata.get(SUPPLIER_LIST).map(|stored| &stored.value),
            Some(&places(vec![place(&[(SUPPLIER_NAME, "Namn")])]))
        );
    }

    #[test]
    fn add_is_disabled_at_the_limit_of_3_and_of_1() {
        assert_eq!(EditorForm::max_entries(CREATOR_LIST), Some(3));
        assert_eq!(EditorForm::max_entries(OWNER_LIST), Some(3));
        assert_eq!(EditorForm::max_entries(LICENSOR_LIST), Some(3));
        assert_eq!(EditorForm::max_entries(SUPPLIER_LIST), Some(1));
        assert_eq!(EditorForm::max_entries(GENRE_LIST), None);
        assert_eq!(EditorForm::max_entries(CREATED), None);

        let one = |name: &str| place(&[(CREATOR_NAME, name)]);
        let mut form = form_with(with_lists(&[(CREATOR_LIST, vec![one("A"), one("B")])]));
        assert!(form.can_add_entry(CREATOR_LIST));
        form.add_entry(CREATOR_LIST);
        assert_eq!(form.list_count(CREATOR_LIST), 3);
        // At 3, also with the third entry empty: Add does nothing.
        assert!(!form.can_add_entry(CREATOR_LIST));
        form.add_entry(CREATOR_LIST);
        assert_eq!(form.list_count(CREATOR_LIST), 3);
        // Removing one makes room again.
        form.remove_entry(CREATOR_LIST, 2);
        assert!(form.can_add_entry(CREATOR_LIST));

        // Licensor and Copyright Owner alike.
        let mut form = form_with(Metadata::new());
        for list in [OWNER_LIST, LICENSOR_LIST] {
            for _ in 0..5 {
                form.add_entry(list);
            }
            assert_eq!(form.list_count(list), 3, "{list}");
            assert!(!form.can_add_entry(list), "{list}");
        }

        // Image Supplier: at most 1.
        let mut form = form_with(Metadata::new());
        assert!(form.can_add_entry(SUPPLIER_LIST));
        form.add_entry(SUPPLIER_LIST);
        form.add_entry(SUPPLIER_LIST);
        assert_eq!(form.list_count(SUPPLIER_LIST), 1);
        assert!(!form.can_add_entry(SUPPLIER_LIST));
        let form = form_with(with_lists(&[(
            SUPPLIER_LIST,
            vec![place(&[(SUPPLIER_NAME, "Bildbyrån")])],
        )]));
        assert!(!form.can_add_entry(SUPPLIER_LIST));

        // Genre and the locations have no limit.
        let mut form = form_with(Metadata::new());
        for _ in 0..5 {
            form.add_entry(GENRE_LIST);
            form.add_entry(CREATED);
        }
        assert_eq!(form.list_count(GENRE_LIST), 5);
        assert!(form.can_add_entry(GENRE_LIST));
        assert!(form.can_add_entry(CREATED));
        // Another property is no list.
        assert!(!form.can_add_entry("dc:title"));
    }

    #[test]
    fn genres_name_edits_only_x_default_and_keeps_the_other_languages() {
        let mut form = form_with(with_lists(&[(GENRE_LIST, vec![sport()])]));
        type_entry(&mut form, GENRE_LIST, 0, CV_NAME, "Sports");
        let mut expected = sport();
        expected.fields[0].1 = lang_alt(&[("x-default", "Sports"), ("sv", "Idrott")]);
        assert_eq!(
            form.edits(),
            Ok(vec![set(GENRE_LIST, places(vec![expected]))])
        );

        // Without `x-default` the first language is shown; an edit adds `x-default` first.
        let swedish = StructValue {
            fields: vec![(CV_NAME.to_string(), lang_alt(&[("sv", "Idrott")]))],
        };
        let mut form = form_with(with_lists(&[(GENRE_LIST, vec![swedish])]));
        assert_eq!(form.list_text(GENRE_LIST, 0, CV_NAME), "Idrott");
        assert_eq!(form.edits(), Ok(Vec::new()));
        type_entry(&mut form, GENRE_LIST, 0, CV_NAME, "Sport");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                GENRE_LIST,
                places(vec![StructValue {
                    fields: vec![(
                        CV_NAME.to_string(),
                        lang_alt(&[("x-default", "Sport"), ("sv", "Idrott")])
                    )],
                }])
            )])
        );

        // Emptied, the name is removed as a whole; the entry keeps its IDs.
        let mut form = form_with(with_lists(&[(GENRE_LIST, vec![sport()])]));
        type_entry(&mut form, GENRE_LIST, 0, CV_NAME, " ");
        let mut expected = sport();
        expected.fields.remove(0);
        assert_eq!(
            form.edits(),
            Ok(vec![set(GENRE_LIST, places(vec![expected]))])
        );
    }

    #[test]
    fn a_licensor_with_an_address_and_telephone_types_is_written_back_unchanged() {
        let mut form = form_with(with_lists(&[(LICENSOR_LIST, vec![licensor()])]));
        assert_eq!(form.list_text(LICENSOR_LIST, 0, TEL_TYPE_1), "cell");
        assert_eq!(form.list_text(LICENSOR_LIST, 0, TEL_TYPE_2), "mobile");
        assert_eq!(form.list_text(LICENSOR_LIST, 0, STREET), "Storgatan 1");
        // A choice is not typed into.
        assert!(form.list_text_mut(LICENSOR_LIST, 0, TEL_TYPE_1).is_none());
        type_entry(
            &mut form,
            LICENSOR_LIST,
            0,
            LICENSOR_EMAIL,
            "info@example.org",
        );
        let mut expected = licensor();
        expected
            .fields
            .push((LICENSOR_EMAIL.to_string(), text("info@example.org")));
        // Every other sub-field (address, telephones, the unknown type) as read.
        assert_eq!(
            form.edits(),
            Ok(vec![set(LICENSOR_LIST, places(vec![expected]))])
        );
    }

    #[test]
    fn the_telephone_types_are_choices_and_an_unknown_type_is_kept() {
        let mut form = form_with(with_lists(&[(LICENSOR_LIST, vec![licensor()])]));
        let entries = form.list_choice_entries(LICENSOR_LIST, 0, TEL_TYPE_2);
        let values: Vec<&str> = entries.iter().map(|entry| entry.value.as_str()).collect();
        assert_eq!(
            values,
            ["", "work", "cell", "fax", "home", "pager", "mobile"]
        );
        let texts: Vec<&str> = entries.iter().map(|entry| entry.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "",
                "Work",
                "Cell",
                "Fax",
                "Home",
                "Pager",
                "Unknown value: mobile"
            ]
        );
        assert_eq!(
            entries.get(1).and_then(|entry| entry.tooltip.as_deref()),
            Some("http://ns.useplus.org/ldf/vocab/work")
        );
        assert_eq!(entries.last().and_then(|entry| entry.tooltip.clone()), None);
        // A known value has no extra entry.
        assert_eq!(
            form.list_choice_entries(LICENSOR_LIST, 0, TEL_TYPE_1).len(),
            6
        );
        // A sub-field that is not a choice, or an entry that does not exist: none.
        assert!(
            form.list_choice_entries(LICENSOR_LIST, 0, LICENSOR_CITY)
                .is_empty()
        );
        assert!(
            form.list_choice_entries(LICENSOR_LIST, 3, TEL_TYPE_1)
                .is_empty()
        );

        // Another type chosen, then the unknown one chosen back: written as read.
        form.set_list_choice(LICENSOR_LIST, 0, TEL_TYPE_2, "fax");
        assert_eq!(form.list_text(LICENSOR_LIST, 0, TEL_TYPE_2), "fax");
        // The unknown value stays in the list until the form is left.
        assert_eq!(
            form.list_choice_entries(LICENSOR_LIST, 0, TEL_TYPE_2).len(),
            7
        );
        form.set_list_choice(LICENSOR_LIST, 0, TEL_TYPE_2, "mobile");
        assert_eq!(form.edits(), Ok(Vec::new()));

        // The code is written; the empty entry removes the type.
        form.set_list_choice(LICENSOR_LIST, 0, TEL_TYPE_1, "work");
        form.set_list_choice(LICENSOR_LIST, 0, TEL_TYPE_2, "");
        let mut expected = licensor();
        expected.fields[2].1 = text("work");
        expected.fields.remove(3);
        assert_eq!(
            form.edits(),
            Ok(vec![set(LICENSOR_LIST, places(vec![expected]))])
        );
        // `set_list_choice` does nothing on a sub-field that is not a choice.
        form.set_list_choice(LICENSOR_LIST, 0, LICENSOR_CITY, "work");
        assert_eq!(form.list_text(LICENSOR_LIST, 0, LICENSOR_CITY), "Umeå");
    }

    #[test]
    fn copy_from_creator_adds_the_missing_names_up_to_the_limit() {
        // Not enabled without a Creator name.
        let mut form = form_with(Metadata::new());
        assert!(!form.can_copy_creator());
        form.copy_creator();
        assert_eq!(form.list_count(CREATOR_LIST), 0);

        // Two creators, one already an Image Creator (with an ID, which stays).
        let per = place(&[(CREATOR_NAME, "Per Ek"), (CREATOR_ID, "urn:per")]);
        let mut metadata = with_lists(&[(CREATOR_LIST, vec![per.clone()])]);
        insert(&mut metadata, "dc:creator", list(&["Anna Berg", "Per Ek"]));
        let mut form = form_with(metadata);
        assert!(form.can_copy_creator());
        form.copy_creator();
        assert_eq!(form.list_count(CREATOR_LIST), 2);
        assert_eq!(form.list_text(CREATOR_LIST, 1, CREATOR_NAME), "Anna Berg");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                CREATOR_LIST,
                places(vec![per, place(&[(CREATOR_NAME, "Anna Berg")])])
            )])
        );
        // A second press adds nothing: both names are there.
        form.copy_creator();
        assert_eq!(form.list_count(CREATOR_LIST), 2);

        // The limit: two entries and two new names give one new entry.
        let one = |name: &str| place(&[(CREATOR_NAME, name)]);
        let mut form = form_with(with_lists(&[(CREATOR_LIST, vec![one("A"), one("B")])]));
        // An unsaved Creator text counts.
        type_into(&mut form, EditorField::Creator, "C, D");
        assert!(form.can_copy_creator());
        form.copy_creator();
        assert_eq!(form.list_count(CREATOR_LIST), 3);
        assert_eq!(form.list_text(CREATOR_LIST, 2, CREATOR_NAME), "C");
        // Full: not enabled.
        assert!(!form.can_copy_creator());
        form.copy_creator();
        assert_eq!(form.list_count(CREATOR_LIST), 3);
        // Creator itself is unchanged.
        assert_eq!(form.text(EditorField::Creator), "C, D");
    }

    #[test]
    fn an_emptied_image_creator_is_emptied_on_the_windows_image_only() {
        let anna = place(&[(CREATOR_NAME, "Anna Berg")]);
        let mut metadata = with_lists(&[
            (GENRE_LIST, vec![sport()]),
            (CREATOR_LIST, vec![anna]),
            (OWNER_LIST, vec![place(&[(OWNER_NAME, "Anna Berg AB")])]),
        ]);
        insert(
            &mut metadata,
            "dc:rights",
            lang_alt(&[("x-default", "© Anna")]),
        );
        let mut form = form_with(metadata);
        form.remove_entry(GENRE_LIST, 0);
        form.remove_entry(CREATOR_LIST, 0);
        type_entry(&mut form, OWNER_LIST, 0, OWNER_ID, "urn:owner");
        type_into(&mut form, EditorField::Copyright, "");

        let batch = batch_of(&form);
        let owner = places(vec![place(&[
            (OWNER_NAME, "Anna Berg AB"),
            (OWNER_ID, "urn:owner"),
        ])]);
        assert_eq!(
            batch.this_image,
            [
                clear("dc:rights"),
                clear(GENRE_LIST),
                clear(CREATOR_LIST),
                set(OWNER_LIST, owner.clone()),
            ]
        );
        // The emptied Image Creator (and Copyright Notice) stay on the window's image; the
        // emptied Genre and the changed Copyright Owner go to every image.
        assert_eq!(batch.others, [clear(GENRE_LIST), set(OWNER_LIST, owner)]);
        assert_eq!(batch.emptied_here_only, [EditorField::Copyright]);
        assert_eq!(batch.field_count(), 4);
        assert_eq!(
            apply_confirmation(&batch, 3, 0),
            [
                "Write 4 changed fields to 3 images?",
                "Changed: Copyright Owner",
                "Emptied: Genre",
                "Emptied only on this image: Copyright Notice, Image Creator",
            ]
        );

        // Copyright Owner and Licensor alike.
        let mut form = form_with(with_lists(&[
            (OWNER_LIST, vec![place(&[(OWNER_NAME, "Anna Berg AB")])]),
            (LICENSOR_LIST, vec![licensor()]),
        ]));
        form.remove_entry(OWNER_LIST, 0);
        form.remove_entry(LICENSOR_LIST, 0);
        let batch = batch_of(&form);
        assert_eq!(batch.this_image, [clear(OWNER_LIST), clear(LICENSOR_LIST)]);
        assert!(batch.others.is_empty());
        assert_eq!(
            apply_confirmation(&batch, 2, 0),
            [
                "Write 2 changed fields to 2 images?",
                "Emptied only on this image: Copyright Owner, Licensor",
            ]
        );

        // An emptied Image Supplier is not a copyright-management field: emptied everywhere.
        let mut form = form_with(with_lists(&[(
            SUPPLIER_LIST,
            vec![place(&[(SUPPLIER_NAME, "Bildbyrån")])],
        )]));
        form.remove_entry(SUPPLIER_LIST, 0);
        let batch = batch_of(&form);
        assert_eq!(batch.others, [clear(SUPPLIER_LIST)]);
    }

    #[test]
    fn copy_and_paste_carry_the_new_lists_and_leave_out_the_empty_ones() {
        let mut source = form_with(with_lists(&[(LICENSOR_LIST, vec![licensor()])]));
        source.add_entry(SUPPLIER_LIST);
        type_entry(&mut source, SUPPLIER_LIST, 0, SUPPLIER_NAME, "Bildbyrån");
        source.add_entry(GENRE_LIST);
        let copied = source.copy("a.jpg");
        // The empty Genre entry is dropped and the lists without an entry left out.
        assert_eq!(
            copied.locations,
            [
                (LICENSOR_LIST, vec![licensor()]),
                (SUPPLIER_LIST, vec![place(&[(SUPPLIER_NAME, "Bildbyrån")])]),
            ]
        );
        assert_eq!(copied.count(), 2);

        let mut target = form_with(with_lists(&[(GENRE_LIST, vec![sport()])]));
        target.paste(&copied);
        assert_eq!(target.list_count(LICENSOR_LIST), 1);
        assert_eq!(target.list_text(LICENSOR_LIST, 0, TEL_TYPE_2), "mobile");
        // Genre was not copied, so it keeps its entry.
        assert_eq!(target.list_count(GENRE_LIST), 1);
        assert_eq!(
            target.edits(),
            Ok(vec![
                set(LICENSOR_LIST, places(vec![licensor()])),
                set(
                    SUPPLIER_LIST,
                    places(vec![place(&[(SUPPLIER_NAME, "Bildbyrån")])])
                ),
            ])
        );
    }

    #[test]
    fn a_new_list_with_an_open_conflict_is_read_only_until_it_is_decided() {
        let anna = place(&[(CREATOR_NAME, "Anna Berg")]);
        let per = place(&[(CREATOR_NAME, "Per Ek")]);
        let mut metadata = with_lists(&[(CREATOR_LIST, vec![anna.clone()])]);
        insert(&mut metadata, "dc:creator", list(&["Lisa"]));
        metadata.add_conflict(Conflict {
            property: CREATOR_LIST.into(),
            values: vec![
                (Origin::XmpEmbedded, places(vec![anna])),
                (Origin::XmpSidecar, places(vec![per.clone()])),
            ],
            rule: ConflictRule::Open,
        });
        let mut form = form_with(metadata);
        assert!(form.list_read_only(CREATOR_LIST).is_some());
        assert!(!form.can_add_entry(CREATOR_LIST));
        assert!(!form.can_copy_creator());
        assert!(form.list_text_mut(CREATOR_LIST, 0, CREATOR_NAME).is_none());
        assert!(form.choose(0, 1));
        assert_eq!(form.list_text(CREATOR_LIST, 0, CREATOR_NAME), "Per Ek");
        assert_eq!(form.edits(), Ok(vec![set(CREATOR_LIST, places(vec![per]))]));
        assert!(form.can_copy_creator());
    }

    #[test]
    fn the_texts_of_the_new_lists() {
        let cases = [
            (EditorText::AddEntry { list: GENRE_LIST }, "Add Genre"),
            (
                EditorText::AddEntry { list: CREATOR_LIST },
                "Add Image Creator",
            ),
            (
                EditorText::AddEntry { list: OWNER_LIST },
                "Add Copyright Owner",
            ),
            (
                EditorText::AddEntry {
                    list: LICENSOR_LIST,
                },
                "Add Licensor",
            ),
            (
                EditorText::AddEntry {
                    list: SUPPLIER_LIST,
                },
                "Add Image Supplier",
            ),
            (EditorText::AtMostEntries { max: 3 }, "At most 3 entries"),
            (EditorText::AtMostEntries { max: 1 }, "At most 1 entry"),
            (EditorText::CopyFromCreator, "Copy from Creator"),
            (EditorText::Address, "Address"),
            (
                EditorText::EmptiedHere {
                    fields: vec![EditorField::Creator],
                    lists: vec![CREATOR_LIST, LICENSOR_LIST],
                },
                "Emptied only on this image: Creator, Image Creator, Licensor",
            ),
            (
                EditorText::EmptiedHere {
                    fields: vec![EditorField::Copyright],
                    lists: Vec::new(),
                },
                "Emptied only on this image: Copyright Notice",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
        // Each list in its place among the fields.
        assert_eq!(
            editor_text(&EditorText::Changed {
                fields: vec![
                    EditorField::Title,
                    EditorField::Instructions,
                    EditorField::ModelReleaseStatus
                ],
                lists: vec![CREATED, GENRE_LIST, CREATOR_LIST, SUPPLIER_LIST],
            }),
            format!(
                "Changed: Title, Location Created, Instructions, Genre, Image Creator, Image \
                 Supplier, {}",
                EditorField::ModelReleaseStatus.label()
            )
        );
    }

    // --- recent and own values (U12a, T141) ---

    #[test]
    fn the_fields_with_a_list_of_values() {
        use EditorField::*;
        let with: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|field| field.has_values())
            .collect();
        assert_eq!(
            with,
            [
                Title,
                Headline,
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
                WebStatement,
                Sublocation,
                City,
                State,
                Country,
                CountryCode,
                JobId,
                SubjectCode,
                SceneCode,
                ModelReleaseId,
                PropertyReleaseId,
            ]
        );
        // Not the multi-line fields, the date, the check box, the choice lists, nor the
        // deprecated read-only fields.
        for field in [
            Description,
            AltText,
            ExtDescription,
            Instructions,
            UsageTerms,
            DateCreated,
            Copyrighted,
            ModelReleaseStatus,
            MinorModelAge,
            PropertyReleaseStatus,
            DataMining,
            DigitalSourceType,
            Urgency,
            Category,
            SupplementalCategories,
        ] {
            assert!(!field.has_values(), "{field:?}");
        }
        let form = form_with(Metadata::new());
        assert!(form.values_shown(City));
        assert!(!form.values_shown(Description));
    }

    #[test]
    fn a_read_only_field_shows_no_list() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Category", text("SPO"));
        let form = form_with(metadata);
        assert_eq!(form.read_only_text(EditorField::Category), Some("SPO"));
        assert!(!form.values_shown(EditorField::Category));
    }

    #[test]
    fn a_chosen_value_replaces_a_texts_text() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Oslo"));
        let mut form = form_with(metadata);
        form.use_value(EditorField::City, "Bergen");
        assert_eq!(form.text(EditorField::City), "Bergen");
        // Changed as after typing: an edit, not yet written.
        assert_eq!(
            form.edits().unwrap(),
            [set("photoshop:City", text("Bergen"))]
        );
    }

    #[test]
    fn a_chosen_value_is_appended_to_a_list_once() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:subject", list(&["sky", "sea"]));
        let mut form = form_with(metadata);
        form.use_value(EditorField::Keywords, "boat");
        assert_eq!(form.text(EditorField::Keywords), "sky, sea, boat");
        form.use_value(EditorField::Keywords, "sky");
        assert_eq!(form.text(EditorField::Keywords), "sky, sea, boat");
        let mut empty = form_with(Metadata::new());
        empty.use_value(EditorField::PersonShown, "Anna");
        assert_eq!(empty.text(EditorField::PersonShown), "Anna");
    }

    #[test]
    fn a_value_is_not_chosen_into_a_field_without_a_list() {
        let mut form = form_with(Metadata::new());
        form.use_value(EditorField::Description, "text");
        form.use_value(EditorField::DateCreated, "2026");
        assert_eq!(form.text(EditorField::Description), "");
        assert_eq!(form.text(EditorField::DateCreated), "");
    }

    #[test]
    fn the_present_values_of_a_text_and_a_list_field() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::City, "Oslo");
        type_into(&mut form, EditorField::Keywords, "sky, , sea");
        assert_eq!(form.present_values(EditorField::City), ["Oslo"]);
        assert_eq!(form.present_values(EditorField::Keywords), ["sky", "sea"]);
        type_into(&mut form, EditorField::City, "   ");
        assert!(form.present_values(EditorField::City).is_empty());
    }

    #[test]
    fn the_written_values_are_the_changed_fields_with_a_list() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("Oslo"));
        insert(&mut metadata, "photoshop:Headline", text("Old"));
        insert(&mut metadata, "photoshop:Country", text("Norway"));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Keywords, "sky, sea");
        type_into(&mut form, EditorField::Headline, "New");
        type_into(&mut form, EditorField::Description, "A caption");
        // Emptied: nothing.
        type_into(&mut form, EditorField::Country, "");
        assert_eq!(
            form.written_values(),
            [
                (EditorField::Headline, vec!["New".to_string()]),
                (
                    EditorField::Keywords,
                    vec!["sky".to_string(), "sea".to_string()]
                ),
            ]
        );
        assert!(form_with(Metadata::new()).written_values().is_empty());

        // A list field gives only the entries the write adds; one only removed gives nothing.
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "dc:subject",
            Value::List(vec!["sky".into(), "sea".into()]),
        );
        let mut form = form_with(metadata.clone());
        type_into(&mut form, EditorField::Keywords, "sky, sea, gull");
        assert_eq!(
            form.written_values(),
            [(EditorField::Keywords, vec!["gull".to_string()])]
        );
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Keywords, "sky");
        assert!(form.written_values().is_empty());
    }

    #[test]
    fn the_texts_of_the_list_of_values() {
        let cases = [
            (EditorText::ValuesTooltip, "Recent and saved values"),
            (EditorText::Recent, "Recent"),
            (EditorText::MyValues, "My Values"),
            (EditorText::AddToMyValues, "Add to My Values"),
            (EditorText::NoValuesYet, "No values yet"),
            (EditorText::RemoveValue, "×"),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }

    // --- persons with details, products, artwork, contributors, CV terms, registry (U11c, T143) ---

    const PERSON_LIST: &str = "Iptc4xmpExt:PersonInImageWDetails";
    const P_NAME: &str = "Iptc4xmpExt:PersonName";
    const P_ID: &str = "Iptc4xmpExt:PersonId";
    const P_DESCRIPTION: &str = "Iptc4xmpExt:PersonDescription";
    const P_CHARACTERISTIC: &str = "Iptc4xmpExt:PersonCharacteristic";
    const PRODUCT_LIST: &str = "Iptc4xmpExt:ProductInImage";
    const PR_NAME: &str = "Iptc4xmpExt:ProductName";
    const PR_GTIN: &str = "Iptc4xmpExt:ProductGTIN";
    const PR_DESCRIPTION: &str = "Iptc4xmpExt:ProductDescription";
    const ARTWORK_LIST: &str = "Iptc4xmpExt:ArtworkOrObject";
    const AO_TITLE: &str = "Iptc4xmpExt:AOTitle";
    const AO_CREATOR: &str = "Iptc4xmpExt:AOCreator";
    const AO_DATE: &str = "Iptc4xmpExt:AODateCreated";
    const AO_CIRCA: &str = "Iptc4xmpExt:AOCircaDateCreated";
    const AO_SOURCE: &str = "Iptc4xmpExt:AOSource";
    const AO_LICENSOR_NAME: &str = "Iptc4xmpExt:AOCurrentLicensorName";
    const CONTRIBUTOR_LIST: &str = "Iptc4xmpExt:Contributor";
    const C_NAME: &str = "Iptc4xmpExt:Name";
    const C_ROLE: &str = "Iptc4xmpExt:Role";
    const C_IDENTIFIER: &str = "xmp:Identifier";
    const ABOUT_LIST: &str = "Iptc4xmpExt:AboutCvTerm";
    const REGISTRY_LIST: &str = "Iptc4xmpExt:RegistryId";
    const R_ORG: &str = "Iptc4xmpExt:RegOrgId";
    const R_ITEM: &str = "Iptc4xmpExt:RegItemId";
    /// The six lists of T143 in form order.
    const T143_LISTS: [&str; 6] = [
        PERSON_LIST,
        PRODUCT_LIST,
        ARTWORK_LIST,
        CONTRIBUTOR_LIST,
        ABOUT_LIST,
        REGISTRY_LIST,
    ];

    fn date(xmp: &str) -> Value {
        Value::Date(IptcDate::parse_xmp(xmp).unwrap())
    }

    /// A structure of the given sub-fields and values.
    fn entry(fields: Vec<(&str, Value)>) -> StructValue {
        StructValue {
            fields: fields
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect(),
        }
    }

    /// A person with a name in `x-default` and Swedish, an identifier and one characteristic.
    fn anna_person() -> StructValue {
        entry(vec![
            (P_NAME, lang_alt(&[("x-default", "Anna"), ("sv", "Anna S")])),
            (P_ID, list(&["urn:anna"])),
            (
                P_CHARACTERISTIC,
                places(vec![entry(vec![(
                    "Iptc4xmpExt:CvTermName",
                    lang_alt(&[("x-default", "adult")]),
                )])]),
            ),
        ])
    }

    /// A painting with a title, two creators, a date and a current licensor (under "More").
    fn painting() -> StructValue {
        entry(vec![
            (
                AO_TITLE,
                lang_alt(&[("x-default", "Skriet"), ("no", "Skrik")]),
            ),
            (AO_CREATOR, list(&["Edvard Munch"])),
            (AO_DATE, date("1893")),
            (AO_LICENSOR_NAME, text("Museet")),
        ])
    }

    /// One entry of each of the six lists of T143.
    fn t143_metadata() -> Metadata {
        with_lists(&[
            (PERSON_LIST, vec![anna_person()]),
            (
                PRODUCT_LIST,
                vec![entry(vec![
                    (PR_NAME, lang_alt(&[("x-default", "Cykel")])),
                    (PR_GTIN, text("7310000000001")),
                ])],
            ),
            (ARTWORK_LIST, vec![painting()]),
            (
                CONTRIBUTOR_LIST,
                vec![entry(vec![
                    (C_NAME, lang_alt(&[("x-default", "Per")])),
                    (C_ROLE, list(&["editor"])),
                ])],
            ),
            (ABOUT_LIST, vec![sport()]),
            (
                REGISTRY_LIST,
                vec![entry(vec![(R_ORG, text("org")), (R_ITEM, text("1"))])],
            ),
        ])
    }

    #[test]
    fn the_six_new_lists_their_places_and_sub_fields_with_names() {
        // Every list in form order: T139's seven unchanged, the six of T143 in their places.
        assert_eq!(
            EditorForm::ALL_LISTS,
            [
                PERSON_LIST,
                PRODUCT_LIST,
                ARTWORK_LIST,
                CONTRIBUTOR_LIST,
                CREATED,
                SHOWN,
                GENRE_LIST,
                ABOUT_LIST,
                REGISTRY_LIST,
                CREATOR_LIST,
                OWNER_LIST,
                LICENSOR_LIST,
                SUPPLIER_LIST,
            ]
        );
        assert_eq!(EditorForm::PERSON_DETAILS, PERSON_LIST);
        // Content after Person Shown; Creator and Rights at the end (after Copyrighted, its last
        // field); Assignment after Genre (after Scene Code) and at the end (after Supplemental
        // Categories, its last field) (U11c-Q2).
        assert_eq!(
            EditorForm::lists_at(EditorSection::Content, Some(EditorField::PersonShown)),
            [PERSON_LIST, PRODUCT_LIST, ARTWORK_LIST]
        );
        assert_eq!(
            EditorForm::lists_at(EditorSection::CreatorRights, Some(EditorField::Copyrighted)),
            [CONTRIBUTOR_LIST]
        );
        // CV-Term About Image is drawn right after the list Genre, so Genre alone follows Scene
        // Code (T139's place unchanged).
        assert_eq!(
            EditorForm::lists_at(EditorSection::Assignment, Some(EditorField::SceneCode)),
            [GENRE_LIST]
        );
        assert_eq!(EditorForm::lists_after_list(GENRE_LIST), [ABOUT_LIST]);
        for list in EditorForm::ALL_LISTS {
            if list != GENRE_LIST {
                assert!(EditorForm::lists_after_list(list).is_empty(), "{list}");
            }
        }
        assert_eq!(
            EditorForm::lists_at(
                EditorSection::Assignment,
                Some(EditorField::SupplementalCategories)
            ),
            [REGISTRY_LIST]
        );
        let last = |section: EditorSection| {
            EditorField::ALL
                .into_iter()
                .rfind(|field| field.section() == section)
        };
        assert_eq!(
            last(EditorSection::CreatorRights),
            Some(EditorField::Copyrighted)
        );
        assert_eq!(
            last(EditorSection::Assignment),
            Some(EditorField::SupplementalCategories)
        );

        // The list names and the sub-fields' names (IPTC's, from `property_label`).
        let names = |fields: &[&str]| -> Vec<String> {
            fields
                .iter()
                .map(|id| property_label(id, Language::En))
                .collect()
        };
        assert_eq!(
            names(&T143_LISTS),
            [
                "Person Shown in the Image with Details",
                "Product Shown in the Image",
                "Artwork or Object in the Image",
                "Contributor",
                "CV-Term About Image",
                "Image Registry Entry",
            ]
        );
        assert_eq!(
            EditorForm::list_fields(PERSON_LIST),
            [P_NAME, P_ID, P_DESCRIPTION]
        );
        assert_eq!(
            names(EditorForm::list_fields(PERSON_LIST)),
            ["Name", "Identifier", "Description"]
        );
        assert_eq!(
            names(EditorForm::list_fields(PRODUCT_LIST)),
            ["Name", "GTIN", "Identifier", "Description"]
        );
        assert_eq!(
            EditorForm::list_fields(CONTRIBUTOR_LIST),
            [C_NAME, C_ROLE, C_IDENTIFIER]
        );
        assert_eq!(
            names(EditorForm::list_fields(CONTRIBUTOR_LIST)),
            ["Name", "Role", "Identifier"]
        );
        assert_eq!(
            EditorForm::list_fields(ABOUT_LIST),
            EditorForm::list_fields(GENRE_LIST)
        );
        assert_eq!(
            names(EditorForm::list_fields(REGISTRY_LIST)),
            ["Organisation Id", "Item Id", "Role"]
        );
        // Artwork or Object (U11c-Q3): six open, eleven under "More".
        assert_eq!(
            names(EditorForm::list_fields(ARTWORK_LIST)),
            [
                "Title",
                "Creator",
                "Date Created",
                "Source",
                "Source Inventory Number",
                "Copyright Notice",
            ]
        );
        assert_eq!(
            names(EditorForm::list_more_fields(ARTWORK_LIST)),
            [
                "Circa Date Created",
                "Style Period",
                "Content Description",
                "Contribution Description",
                "Physical Description",
                "Creator ID",
                "Current Copyright Owner Name",
                "Current Copyright Owner ID",
                "Current Licensor Name",
                "Current Licensor ID",
                "Source Inventory URL",
            ]
        );
        // "More" only for Artwork, "Address" only for Licensor.
        assert!(EditorForm::list_address_fields(ARTWORK_LIST).is_empty());
        assert!(EditorForm::list_more_fields(LICENSOR_LIST).is_empty());
        for list in EditorForm::ALL_LISTS {
            if list != ARTWORK_LIST {
                assert!(EditorForm::list_more_fields(list).is_empty(), "{list}");
            }
            if list != LICENSOR_LIST {
                assert!(EditorForm::list_address_fields(list).is_empty(), "{list}");
            }
        }
        // No IPTC limit on any of the six.
        for list in T143_LISTS {
            assert_eq!(EditorForm::max_entries(list), None, "{list}");
        }
    }

    #[test]
    fn each_t143_list_is_read_edited_and_written_as_one_struct_list_edit() {
        let mut form = form_with(t143_metadata());
        for list in T143_LISTS {
            assert_eq!(form.list_count(list), 1, "{list}");
            assert_eq!(form.list_read_only(list), None, "{list}");
        }
        assert_eq!(form.list_text(PERSON_LIST, 0, P_NAME), "Anna");
        assert_eq!(form.list_text(PERSON_LIST, 0, P_ID), "urn:anna");
        assert_eq!(form.list_text(PRODUCT_LIST, 0, PR_GTIN), "7310000000001");
        assert_eq!(form.list_text(ARTWORK_LIST, 0, AO_DATE), "1893");
        assert_eq!(form.list_text(ARTWORK_LIST, 0, AO_LICENSOR_NAME), "Museet");
        assert_eq!(form.list_text(CONTRIBUTOR_LIST, 0, C_ROLE), "editor");
        assert_eq!(form.list_text(ABOUT_LIST, 0, CV_NAME), "Sport");
        assert_eq!(form.list_text(REGISTRY_LIST, 0, R_ITEM), "1");
        // Shown untouched, the lists give no edit.
        assert_eq!(form.edits(), Ok(Vec::new()));

        type_entry(&mut form, PERSON_LIST, 0, P_DESCRIPTION, "Till vänster");
        type_entry(&mut form, PRODUCT_LIST, 0, PR_GTIN, "7310000000002");
        // A sub-field under "More".
        type_entry(&mut form, ARTWORK_LIST, 0, AO_LICENSOR_NAME, "Galleriet");
        type_entry(&mut form, CONTRIBUTOR_LIST, 0, C_NAME, "Per Ek");
        type_entry(&mut form, ABOUT_LIST, 0, CV_ID, "http://example.org/x");
        type_entry(&mut form, REGISTRY_LIST, 0, R_ORG, "");
        type_into(&mut form, EditorField::Keywords, "sky");
        type_into(&mut form, EditorField::Event, "Vernissage");
        type_into(&mut form, EditorField::DateCreated, "2026-11-15");
        type_into(&mut form, EditorField::Instructions, "Embargo");

        let mut person = anna_person();
        person.fields.push((
            P_DESCRIPTION.to_string(),
            lang_alt(&[("x-default", "Till vänster")]),
        ));
        let product = entry(vec![
            (PR_NAME, lang_alt(&[("x-default", "Cykel")])),
            (PR_GTIN, text("7310000000002")),
        ]);
        let mut artwork = painting();
        artwork.fields[3].1 = text("Galleriet");
        let contributor = entry(vec![
            (C_NAME, lang_alt(&[("x-default", "Per Ek")])),
            (C_ROLE, list(&["editor"])),
        ]);
        let mut about = sport();
        about.fields[1].1 = text("http://example.org/x");
        let registry = entry(vec![(R_ITEM, text("1"))]);
        let edits = form.edits().unwrap();
        // One `Set` of the whole list each, in its place among the fields.
        assert_eq!(
            edits,
            vec![
                set("dc:subject", list(&["sky"])),
                set(PERSON_LIST, places(vec![person])),
                set(PRODUCT_LIST, places(vec![product])),
                set(ARTWORK_LIST, places(vec![artwork])),
                set(
                    "Iptc4xmpExt:Event",
                    lang_alt(&[("x-default", "Vernissage")])
                ),
                set(CONTRIBUTOR_LIST, places(vec![contributor])),
                set("photoshop:DateCreated", date("2026-11-15")),
                set("photoshop:Instructions", text("Embargo")),
                set(ABOUT_LIST, places(vec![about])),
                set(REGISTRY_LIST, places(vec![registry])),
            ]
        );
        // Accepted by `apply_edits`, the Characteristics (a nested list) included.
        let mut metadata = t143_metadata();
        assert!(apply_edits(&mut metadata, &edits).is_ok());

        // The confirmation names each list in its place.
        let batch = batch_of(&form);
        // Keywords go to the other images as the entry they add (U12d-Q1, T180).
        let mut others = edits.clone();
        others[0] = entries("dc:subject", &["sky"], &[]);
        assert_eq!(batch.others, others);
        assert_eq!(
            apply_confirmation(&batch, 2, 0),
            [
                "Write 10 changed fields to 2 images?".to_string(),
                format!(
                    "Changed: Person Shown in the Image with Details, Product Shown in the \
                     Image, Artwork or Object in the Image, {}, Contributor, {}, {}, CV-Term About \
                     Image, Image Registry Entry",
                    EditorField::Event.label(),
                    EditorField::DateCreated.label(),
                    EditorField::Instructions.label(),
                ),
                format!("{}: adds sky", EditorField::Keywords.label()),
            ]
        );
    }

    #[test]
    fn an_emptied_t143_list_is_emptied_on_every_image_and_named() {
        let mut form = form_with(t143_metadata());
        for list in T143_LISTS {
            form.remove_entry(list, 0);
        }
        let batch = batch_of(&form);
        let clears: Vec<Edit> = T143_LISTS.iter().map(|list| clear(list)).collect();
        assert_eq!(batch.this_image, clears);
        // None is a copyright-management field (U11c-Q7): emptied on every image.
        assert_eq!(batch.others, clears);
        assert_eq!(
            apply_confirmation(&batch, 3, 0),
            [
                "Write 6 changed fields to 3 images?",
                "Emptied: Person Shown in the Image with Details, Product Shown in the Image, \
                 Artwork or Object in the Image, Contributor, CV-Term About Image, Image Registry \
                 Entry",
            ]
        );
    }

    #[test]
    fn add_then_ok_with_nothing_typed_gives_no_edit_on_a_t143_list() {
        let mut form = form_with(Metadata::new());
        for list in T143_LISTS {
            assert!(form.can_add_entry(list), "{list}");
            form.add_entry(list);
            assert_eq!(form.list_count(list), 1, "{list}");
        }
        assert_eq!(form.edits(), Ok(Vec::new()));
        // A first sub-field typed in each gives a written list, accepted by `apply_edits`.
        for list in T143_LISTS {
            let field = EditorForm::list_fields(list)[0];
            type_entry(&mut form, list, 0, field, "Namn");
        }
        let edits = form.edits().unwrap();
        assert_eq!(edits.len(), 6);
        let mut metadata = Metadata::new();
        assert!(apply_edits(&mut metadata, &edits).is_ok());
    }

    #[test]
    fn a_lang_alt_sub_field_edits_only_x_default() {
        let mut form = form_with(t143_metadata());
        type_entry(&mut form, PERSON_LIST, 0, P_NAME, "Anna Svensson");
        type_entry(&mut form, ARTWORK_LIST, 0, AO_TITLE, "Skriet II");
        let edits = form.edits().unwrap();
        let mut person = anna_person();
        person.fields[0].1 = lang_alt(&[("x-default", "Anna Svensson"), ("sv", "Anna S")]);
        let mut artwork = painting();
        artwork.fields[0].1 = lang_alt(&[("x-default", "Skriet II"), ("no", "Skrik")]);
        assert_eq!(
            edits,
            [
                set(PERSON_LIST, places(vec![person])),
                set(ARTWORK_LIST, places(vec![artwork])),
            ]
        );
        // A new Lang Alt sub-field gets only `x-default`.
        let mut form = form_with(Metadata::new());
        form.add_entry(PRODUCT_LIST);
        type_entry(&mut form, PRODUCT_LIST, 0, PR_DESCRIPTION, "Röd");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                PRODUCT_LIST,
                places(vec![entry(vec![(
                    PR_DESCRIPTION,
                    lang_alt(&[("x-default", "Röd")])
                )])])
            )])
        );
    }

    #[test]
    fn a_multi_value_sub_field_is_split_at_commas() {
        let mut form = form_with(t143_metadata());
        type_entry(
            &mut form,
            ARTWORK_LIST,
            0,
            AO_CREATOR,
            "Edvard Munch, , Okänd ,",
        );
        type_entry(&mut form, CONTRIBUTOR_LIST, 0, C_ROLE, "editor, translator");
        type_entry(&mut form, CONTRIBUTOR_LIST, 0, C_IDENTIFIER, "urn:a,urn:b");
        type_entry(&mut form, PERSON_LIST, 0, P_ID, " , ");
        let mut artwork = painting();
        artwork.fields[1].1 = list(&["Edvard Munch", "Okänd"]);
        let contributor = entry(vec![
            (C_NAME, lang_alt(&[("x-default", "Per")])),
            (C_ROLE, list(&["editor", "translator"])),
            (C_IDENTIFIER, list(&["urn:a", "urn:b"])),
        ]);
        // Only commas and spaces remove the sub-field.
        let mut person = anna_person();
        person.fields.remove(1);
        assert_eq!(
            form.edits(),
            Ok(vec![
                set(PERSON_LIST, places(vec![person])),
                set(ARTWORK_LIST, places(vec![artwork])),
                set(CONTRIBUTOR_LIST, places(vec![contributor])),
            ])
        );
    }

    #[test]
    fn a_wrong_artwork_date_created_is_an_error_and_gives_no_edit() {
        let mut form = form_with(t143_metadata());
        // The read date is no error.
        assert_eq!(form.list_error(ARTWORK_LIST, 0, AO_DATE), None);
        type_entry(&mut form, ARTWORK_LIST, 0, AO_DATE, "förra året");
        assert_eq!(
            form.list_error(ARTWORK_LIST, 0, AO_DATE),
            Some(date_error_text())
        );
        // Nothing is written: no field error, but no edits either, and no batch.
        assert_eq!(form.edits(), Err(Vec::new()));
        assert!(form.batch_edits().is_err());
        // With a field error too, the field's error is returned.
        type_into(&mut form, EditorField::DateCreated, "igår");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::DateCreated, date_error_text())])
        );
        type_into(&mut form, EditorField::DateCreated, "");
        // Another sub-field has no error; Circa Date Created is free text.
        type_entry(&mut form, ARTWORK_LIST, 0, AO_CIRCA, "omkring 1890");
        assert_eq!(form.list_error(ARTWORK_LIST, 0, AO_CIRCA), None);
        assert_eq!(form.list_error(ARTWORK_LIST, 0, AO_SOURCE), None);

        // A right date (a truncated one) is written as a date.
        type_entry(&mut form, ARTWORK_LIST, 0, AO_DATE, "1893-05");
        assert_eq!(form.list_error(ARTWORK_LIST, 0, AO_DATE), None);
        let mut artwork = painting();
        artwork.fields[2].1 = date("1893-05");
        artwork
            .fields
            .push((AO_CIRCA.to_string(), text("omkring 1890")));
        assert_eq!(
            form.edits(),
            Ok(vec![set(ARTWORK_LIST, places(vec![artwork]))])
        );
        // An emptied date removes the sub-field.
        type_entry(&mut form, ARTWORK_LIST, 0, AO_DATE, "");
        assert_eq!(form.list_error(ARTWORK_LIST, 0, AO_DATE), None);
        let mut artwork = painting();
        artwork.fields.remove(2);
        artwork
            .fields
            .push((AO_CIRCA.to_string(), text("omkring 1890")));
        assert_eq!(
            form.edits(),
            Ok(vec![set(ARTWORK_LIST, places(vec![artwork]))])
        );
        // No error for an entry or a list that does not exist.
        assert_eq!(form.list_error(ARTWORK_LIST, 5, AO_DATE), None);
        assert_eq!(form.list_error("dc:title", 0, AO_DATE), None);
    }

    #[test]
    fn lists_with_errors_names_each_list_with_a_wrong_date_once() {
        let mut form = form_with(t143_metadata());
        assert!(form.lists_with_errors().is_empty());
        type_entry(&mut form, ARTWORK_LIST, 0, AO_DATE, "förra året");
        assert_eq!(form.lists_with_errors(), vec![ARTWORK_LIST]);
        // A second entry with a wrong date too: the list is still named once.
        form.add_entry(ARTWORK_LIST);
        type_entry(&mut form, ARTWORK_LIST, 1, AO_DATE, "nästa år");
        assert!(form.list_error(ARTWORK_LIST, 1, AO_DATE).is_some());
        assert_eq!(form.lists_with_errors(), vec![ARTWORK_LIST]);
        // The first date back to the read text, the second emptied: no list has an error.
        type_entry(&mut form, ARTWORK_LIST, 0, AO_DATE, "1893");
        type_entry(&mut form, ARTWORK_LIST, 1, AO_DATE, "");
        assert!(form.lists_with_errors().is_empty());
    }

    #[test]
    fn the_not_saved_texts() {
        let one_field = EditorText::NotSaved {
            fields: vec![EditorField::DateCreated],
            lists: Vec::new(),
        };
        assert_eq!(
            editor_text(&one_field),
            format!(
                "Nothing was saved. Correct the error in {}.",
                EditorField::DateCreated.label()
            )
        );
        let one_list = EditorText::NotSaved {
            fields: Vec::new(),
            lists: vec![ARTWORK_LIST],
        };
        assert_eq!(
            editor_text(&one_list),
            "Nothing was saved. Correct the error in Artwork or Object in the Image."
        );
        // The Artwork list sits after Person Shown, before Date Created.
        let both = EditorText::NotSaved {
            fields: vec![EditorField::DateCreated],
            lists: vec![ARTWORK_LIST],
        };
        assert_eq!(
            editor_text(&both),
            format!(
                "Nothing was saved. Correct the errors in Artwork or Object in the Image, {}.",
                EditorField::DateCreated.label()
            )
        );
        let neither = EditorText::NotSaved {
            fields: Vec::new(),
            lists: Vec::new(),
        };
        assert_eq!(
            editor_text(&neither),
            "Nothing was saved. Correct the errors in the form."
        );
    }

    #[test]
    fn person_characteristics_are_kept_when_the_name_is_edited() {
        let mut form = form_with(t143_metadata());
        // Not a shown sub-field.
        assert!(!EditorForm::list_fields(PERSON_LIST).contains(&P_CHARACTERISTIC));
        assert!(
            form.list_text_mut(PERSON_LIST, 0, P_CHARACTERISTIC)
                .is_none()
        );
        type_entry(&mut form, PERSON_LIST, 0, P_NAME, "Anna Berg");
        let Ok(edits) = form.edits() else {
            panic!("no error expected");
        };
        let Some(Edit::Set {
            value: Value::StructList(entries),
            ..
        }) = edits.first()
        else {
            panic!("a Set of the list expected: {edits:?}");
        };
        let characteristic = anna_person().fields[2].clone();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].fields.contains(&characteristic));
        assert_eq!(
            entries[0].fields[0].1,
            lang_alt(&[("x-default", "Anna Berg"), ("sv", "Anna S")])
        );
    }

    #[test]
    fn copy_from_person_shown_adds_the_missing_names() {
        // Not enabled without a Person Shown name.
        let mut form = form_with(Metadata::new());
        assert!(!form.can_copy_person_shown());
        form.copy_person_shown();
        assert_eq!(form.list_count(PERSON_LIST), 0);

        // Two names, one already a Name there (its entry stays as it is).
        let mut metadata = with_lists(&[(PERSON_LIST, vec![anna_person()])]);
        insert(
            &mut metadata,
            "Iptc4xmpExt:PersonInImage",
            list(&["Anna", "Per Ek"]),
        );
        let mut form = form_with(metadata);
        assert!(form.can_copy_person_shown());
        form.copy_person_shown();
        assert_eq!(form.list_count(PERSON_LIST), 2);
        assert_eq!(form.list_text(PERSON_LIST, 1, P_NAME), "Per Ek");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                PERSON_LIST,
                places(vec![
                    anna_person(),
                    entry(vec![(P_NAME, lang_alt(&[("x-default", "Per Ek")]))]),
                ])
            )])
        );
        // A second press adds nothing.
        form.copy_person_shown();
        assert_eq!(form.list_count(PERSON_LIST), 2);
        // An unsaved Person Shown text counts; Person Shown itself is unchanged.
        type_into(&mut form, EditorField::PersonShown, "Lisa");
        form.copy_person_shown();
        assert_eq!(form.list_count(PERSON_LIST), 3);
        assert_eq!(form.list_text(PERSON_LIST, 2, P_NAME), "Lisa");
        assert_eq!(form.text(EditorField::PersonShown), "Lisa");
    }

    #[test]
    fn a_t143_list_with_an_open_conflict_is_read_only_and_copy_and_paste_carry_it() {
        let lisa = entry(vec![(P_NAME, lang_alt(&[("x-default", "Lisa")]))]);
        let mut metadata = with_lists(&[(PERSON_LIST, vec![anna_person()])]);
        insert(&mut metadata, "Iptc4xmpExt:PersonInImage", list(&["Per"]));
        metadata.add_conflict(Conflict {
            property: PERSON_LIST.into(),
            values: vec![
                (Origin::XmpEmbedded, places(vec![anna_person()])),
                (Origin::XmpSidecar, places(vec![lisa.clone()])),
            ],
            rule: ConflictRule::Open,
        });
        let mut form = form_with(metadata);
        assert!(form.list_read_only(PERSON_LIST).is_some());
        assert!(!form.can_add_entry(PERSON_LIST));
        assert!(!form.can_copy_person_shown());
        assert!(form.list_text_mut(PERSON_LIST, 0, P_NAME).is_none());
        // A read-only list is not copied.
        assert!(form.copy("a.jpg").locations.is_empty());
        assert!(form.choose(0, 1));
        assert_eq!(form.list_text(PERSON_LIST, 0, P_NAME), "Lisa");
        assert_eq!(form.edits(), Ok(vec![set(PERSON_LIST, places(vec![lisa]))]));

        // Copy and paste carry each list as one field that replaces the whole list.
        let source = form_with(t143_metadata());
        let copied = source.copy("a.jpg");
        assert_eq!(copied.count(), 6);
        let mut target = form_with(with_lists(&[(
            CONTRIBUTOR_LIST,
            vec![entry(vec![(C_NAME, lang_alt(&[("x-default", "Ola")]))])],
        )]));
        target.paste(&copied);
        assert_eq!(target.list_text(CONTRIBUTOR_LIST, 0, C_NAME), "Per");
        assert_eq!(target.list_count(CONTRIBUTOR_LIST), 1);
        let edits = target.edits().unwrap();
        assert_eq!(edits.len(), 6);
        assert!(edits.contains(&set(ARTWORK_LIST, places(vec![painting()]))));
        // Copy then paste on the same form gives no edits.
        let mut same = form_with(t143_metadata());
        same.paste(&copied);
        assert_eq!(same.edits(), Ok(Vec::new()));
    }

    #[test]
    fn the_texts_of_the_t143_lists() {
        let cases = [
            (
                EditorText::AddEntry { list: PERSON_LIST },
                "Add Person Shown in the Image with Details",
            ),
            (
                EditorText::AddEntry { list: PRODUCT_LIST },
                "Add Product Shown in the Image",
            ),
            (
                EditorText::AddEntry { list: ARTWORK_LIST },
                "Add Artwork or Object in the Image",
            ),
            (
                EditorText::AddEntry {
                    list: CONTRIBUTOR_LIST,
                },
                "Add Contributor",
            ),
            (
                EditorText::AddEntry { list: ABOUT_LIST },
                "Add CV-Term About Image",
            ),
            (
                EditorText::AddEntry {
                    list: REGISTRY_LIST,
                },
                "Add Image Registry Entry",
            ),
            (EditorText::More, "More"),
            (
                EditorText::CopyFromPersonShown,
                "Copy from Person Shown in the Image",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(editor_text(&text), expected, "{text:?}");
        }
    }

    // --- the variables (ADR 0017 U13-Q9, T289) ---

    const UNKNOWN_JOB: &str = "{job} is not a variable. Variables: {date} {year} {month} {day} \
                               {time} {make} {model} {name} {folder}. Write {{ or }} for a brace.";
    const UNCLOSED: &str = "A { is not closed. Write {{ for a brace.";
    const NOT_OPENED: &str = "A } is not opened. Write }} for a brace.";

    #[test]
    fn the_variable_error_texts() {
        assert_eq!(
            variable_error_text(&VariableError::Unknown("job".into())),
            UNKNOWN_JOB
        );
        assert_eq!(
            variable_error_text(&VariableError::Unclosed("{date".into())),
            UNCLOSED
        );
        assert_eq!(
            variable_error_text(&VariableError::UnexpectedClose),
            NOT_OPENED
        );
        assert_eq!(
            variable_error_text(&VariableError::NoCaptureTime("date".into())),
            "The image has no capture time for {date}."
        );
    }

    #[test]
    fn an_unknown_variable_is_the_fields_error_and_nothing_is_written() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Headline, "{job}");
        let error = vec![(EditorField::Headline, UNKNOWN_JOB.to_string())];
        assert_eq!(form.edits(), Err(error.clone()));
        assert_eq!(form.batch_edits().err(), Some(error));

        type_into(&mut form, EditorField::Headline, "{date} {name}");
        assert_eq!(
            form.edits(),
            Ok(vec![set("photoshop:Headline", text("{date} {name}"))])
        );
        assert!(form.batch_edits().is_ok());
        type_into(&mut form, EditorField::Headline, "{{x}}");
        assert_eq!(
            form.edits(),
            Ok(vec![set("photoshop:Headline", text("{{x}}"))])
        );
    }

    #[test]
    fn an_unclosed_and_an_unopened_brace_are_the_fields_error() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Headline, "{date");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::Headline, UNCLOSED.to_string())])
        );
        type_into(&mut form, EditorField::Headline, "a}b");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::Headline, NOT_OPENED.to_string())])
        );
    }

    #[test]
    fn a_list_and_a_lang_alt_field_are_checked() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Keywords, "a, {job}");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::Keywords, UNKNOWN_JOB.to_string())])
        );
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Description, "{job}");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::Description, UNKNOWN_JOB.to_string())])
        );
    }

    #[test]
    fn a_structure_lists_text_sub_field_is_checked_when_changed() {
        let mut form = form_with(located(Vec::new(), vec![oslo()]));
        assert_eq!(form.list_error(SHOWN, 0, CITY), None);
        type_entry(&mut form, SHOWN, 0, CITY, "{job}");
        assert_eq!(
            form.list_error(SHOWN, 0, CITY),
            Some(UNKNOWN_JOB.to_string())
        );
        assert_eq!(form.lists_with_errors(), vec![SHOWN]);
        assert_eq!(form.edits(), Err(Vec::new()));
        assert!(form.batch_edits().is_err());

        type_entry(&mut form, SHOWN, 0, CITY, "{folder}");
        assert_eq!(form.list_error(SHOWN, 0, CITY), None);
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                SHOWN,
                places(vec![place(&[(CITY, "{folder}"), (COUNTRY, "Norge")])])
            )])
        );

        // An added entry is checked too.
        form.add_entry(SHOWN);
        type_entry(&mut form, SHOWN, 1, CITY, "{job}");
        assert_eq!(
            form.list_error(SHOWN, 1, CITY),
            Some(UNKNOWN_JOB.to_string())
        );
        assert_eq!(form.edits(), Err(Vec::new()));
    }

    #[test]
    fn another_programs_brace_in_an_unchanged_field_is_no_error() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("{x}"));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Title, "T");
        assert_eq!(
            form.edits(),
            Ok(vec![set("dc:title", lang_alt(&[("x-default", "T")]))])
        );
    }

    #[test]
    fn a_contact_field_is_checked_and_a_choice_list_is_not() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Email, "{job}");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::Email, UNKNOWN_JOB.to_string())])
        );
        // A choice list is never typed into; its value is not checked.
        for field in EditorField::ALL {
            if field.choice_list().is_some() {
                assert_eq!(
                    field_edit(field, "", "{job}"),
                    Ok(set(field.property(), text("{job}"))),
                    "{field:?}"
                );
            }
        }
    }

    #[test]
    fn a_date_with_a_variable_is_a_date_error() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::DateCreated, "{date}");
        assert_eq!(
            form.edits(),
            Err(vec![(EditorField::DateCreated, date_error_text())])
        );
    }

    #[test]
    fn the_entries_of_a_list_field_the_user_did_not_type_are_escaped() {
        let mut form = keywords_form(&["{draft}", "a"]);
        type_into(&mut form, EditorField::Keywords, "{draft}, a, {year}");
        assert_eq!(
            form.edits(),
            Ok(vec![set("dc:subject", list(&["{{draft}}", "a", "{year}"]))])
        );
        // The other images get only the typed entry, as typed.
        assert_eq!(
            batch_of(&form).others,
            vec![entries("dc:subject", &["{year}"], &[])]
        );
    }

    #[test]
    fn a_chosen_conflict_value_is_escaped() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:City", text("{x}"));
        metadata.add_conflict(Conflict {
            property: "photoshop:City".into(),
            values: vec![
                (Origin::XmpEmbedded, text("{x}")),
                (Origin::Iim, text("Malmö")),
            ],
            rule: ConflictRule::Open,
        });
        let mut form = form_with(metadata);
        assert!(form.choose(0, 0));
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("{{x}}"))]));
    }

    #[test]
    fn the_texts_of_a_structure_list_the_user_did_not_change_are_escaped() {
        let mut second = oslo();
        second
            .fields
            .push((NAME.to_string(), lang_alt(&[("x-default", "{n}")])));
        let mut form = form_with(located(Vec::new(), vec![place(&[(CITY, "{x}")]), second]));
        type_entry(&mut form, SHOWN, 1, CITY, "{folder}");
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                SHOWN,
                places(vec![
                    place(&[(CITY, "{{x}}")]),
                    entry(vec![
                        (CITY, text("{folder}")),
                        (COUNTRY, text("Norge")),
                        (NAME, lang_alt(&[("x-default", "{{n}}")])),
                    ]),
                ])
            )])
        );

        // A Lang Alt sub-field: the typed x-default as typed, the other language escaped.
        let person = entry(vec![(
            P_NAME,
            lang_alt(&[("x-default", "Anna"), ("sv", "{Anna}")]),
        )]);
        let mut form = form_with(with_lists(&[(PERSON_LIST, vec![person])]));
        type_entry(&mut form, PERSON_LIST, 0, P_NAME, "{name}");
        assert_eq!(form.list_error(PERSON_LIST, 0, P_NAME), None);
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                PERSON_LIST,
                places(vec![entry(vec![(
                    P_NAME,
                    lang_alt(&[("x-default", "{name}"), ("sv", "{{Anna}}")]),
                )])])
            )])
        );
    }

    #[test]
    fn a_pasted_structure_lists_typed_variable_is_written_to_be_filled() {
        let mut source = form_with(located(Vec::new(), vec![oslo()]));
        type_entry(&mut source, SHOWN, 0, CITY, "{folder}");
        let copied = source.copy("a.jpg");
        let mut form = form_with(Metadata::new());
        form.paste(&copied);
        assert_eq!(form.list_error(SHOWN, 0, CITY), None);
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                SHOWN,
                places(vec![place(&[(CITY, "{folder}"), (COUNTRY, "Norge")])])
            )])
        );
    }

    #[test]
    fn a_pasted_structure_lists_unknown_variable_is_an_error_at_once() {
        let mut source = form_with(located(Vec::new(), vec![oslo()]));
        type_entry(&mut source, SHOWN, 0, CITY, "{job}");
        // The source shows the error too, but Copy All Fields copies unsaved edits.
        let copied = source.copy("a.jpg");
        let mut form = form_with(Metadata::new());
        form.paste(&copied);
        assert_eq!(
            form.list_error(SHOWN, 0, CITY),
            Some(UNKNOWN_JOB.to_string())
        );
        assert_eq!(form.lists_with_errors(), vec![SHOWN]);
        assert_eq!(form.edits(), Err(Vec::new()));
    }

    #[test]
    fn a_pasted_entrys_unshown_texts_and_other_languages_are_escaped() {
        // Person Characteristics is not shown in the form.
        let characteristic = |name: &str| {
            places(vec![entry(vec![(
                "Iptc4xmpExt:CvTermName",
                lang_alt(&[("x-default", name)]),
            )])])
        };
        let person = entry(vec![
            (
                P_NAME,
                lang_alt(&[("x-default", "Anna {name}"), ("sv", "{Anna}")]),
            ),
            (P_CHARACTERISTIC, characteristic("{x}")),
        ]);
        let source = form_with(with_lists(&[(PERSON_LIST, vec![person])]));
        // Read from the file, not pasted: escaped and no error.
        assert_eq!(source.list_error(PERSON_LIST, 0, P_NAME), None);
        let copied = source.copy("a.jpg");
        let mut form = form_with(Metadata::new());
        form.paste(&copied);
        assert_eq!(form.list_error(PERSON_LIST, 0, P_NAME), None);
        assert_eq!(
            form.edits(),
            Ok(vec![set(
                PERSON_LIST,
                places(vec![entry(vec![
                    (
                        P_NAME,
                        lang_alt(&[("x-default", "Anna {name}"), ("sv", "{{Anna}}")]),
                    ),
                    (P_CHARACTERISTIC, characteristic("{{x}}")),
                ])])
            )])
        );
    }

    #[test]
    fn the_recent_values_keep_the_variables() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Headline, "{date}");
        assert!(form.edits().is_ok());
        assert_eq!(
            form.written_values(),
            [(EditorField::Headline, vec!["{date}".to_string()])]
        );
    }

    // --- The template form (ADR 0017 U13c, T291) ---

    #[test]
    fn the_empty_form_is_the_form_of_a_read_without_metadata() {
        assert_eq!(EditorForm::empty(), form_with(Metadata::new()));
    }

    #[test]
    fn the_template_values_are_as_typed() {
        let mut form = EditorForm::empty();
        type_into(&mut form, EditorField::Headline, "{date} {{x}}");
        type_into(&mut form, EditorField::Keywords, "a, {name}");
        type_into(&mut form, EditorField::Email, "a@example.com");
        type_into(&mut form, EditorField::DateCreated, "abc");
        type_into(&mut form, EditorField::Title, "{job}");
        form.add_entry(LOCATION_SHOWN);
        form.add_entry(LOCATION_SHOWN);
        *form
            .list_text_mut(LOCATION_SHOWN, 1, "Iptc4xmpExt:City")
            .unwrap() = "{folder}".to_string();
        *form
            .list_text_mut(LOCATION_SHOWN, 1, "Iptc4xmpExt:CountryName")
            .unwrap() = "{x".to_string();
        let values = form.template_values();
        assert_eq!(
            values.field(EditorField::Headline),
            Some(&Ok(Some(text("{date} {{x}}"))))
        );
        assert_eq!(
            values.field(EditorField::Keywords),
            Some(&Ok(Some(list(&["a", "{name}"]))))
        );
        assert_eq!(
            values.field(EditorField::Email),
            Some(&Ok(Some(text("a@example.com"))))
        );
        assert_eq!(
            values.field(EditorField::DateCreated),
            Some(&Err(date_error_text()))
        );
        assert_eq!(
            values.field(EditorField::Title),
            Some(&Err(variable_error_text(&VariableError::Unknown(
                "job".to_string()
            ))))
        );
        assert_eq!(values.field(EditorField::Description), Some(&Ok(None)));
        // Copyrighted always has a value.
        assert_eq!(
            values.field(EditorField::Copyrighted),
            Some(&Ok(Some(text("False"))))
        );
        // Not editable: no value at all.
        assert_eq!(values.field(EditorField::Urgency), None);
        let shown = values.list(LOCATION_SHOWN).unwrap();
        assert_eq!(
            shown.entries,
            [StructValue {
                fields: vec![
                    ("Iptc4xmpExt:City".to_string(), text("{folder}")),
                    ("Iptc4xmpExt:CountryName".to_string(), text("{x")),
                ]
            }]
        );
        assert_eq!(
            shown.errors,
            [(
                1,
                "Iptc4xmpExt:CountryName",
                variable_error_text(&VariableError::Unclosed("x".to_string()))
            )]
        );
    }

    #[test]
    fn template_fields_are_put_in_the_form_as_typed() {
        let mut form = EditorForm::empty();
        let not_loaded = form.put_template_fields(&[
            ("photoshop:Headline".to_string(), text("{date}")),
            ("dc:subject".to_string(), list(&["a", "b"])),
            ("dc:title".to_string(), lang_alt(&[("x-default", "T")])),
            ("xmpRights:Marked".to_string(), text("true")),
            (vocabulary::MODEL_RELEASE_STATUS.to_string(), text("MR-NON")),
            (
                CONTACT_INFO.to_string(),
                contact(&[
                    ("Iptc4xmpCore:CiEmailWork", "a@example.com"),
                    ("Iptc4xmpCore:CiOther", "x"),
                ]),
            ),
            (
                LOCATION_CREATED.to_string(),
                Value::StructList(vec![
                    StructValue {
                        fields: vec![
                            ("Iptc4xmpExt:City".to_string(), text("Lund")),
                            ("Iptc4xmpExt:LocationName".to_string(), text("Home")),
                        ],
                    },
                    StructValue {
                        fields: vec![("Iptc4xmpExt:City".to_string(), text("Malmö"))],
                    },
                ]),
            ),
            (
                LOCATION_SHOWN.to_string(),
                Value::StructList(vec![StructValue {
                    fields: vec![
                        ("Iptc4xmpExt:City".to_string(), text("{folder}")),
                        ("Iptc4xmpExt:LocationName".to_string(), text("{name}")),
                    ],
                }]),
            ),
            ("photoshop:Urgency".to_string(), text("5")),
            ("xmp:Nothing".to_string(), text("x")),
        ]);
        assert_eq!(
            not_loaded,
            [
                "Iptc4xmpCore:CreatorContactInfo/Iptc4xmpCore:CiOther",
                "Iptc4xmpExt:LocationCreated/Iptc4xmpExt:LocationName",
                "photoshop:Urgency",
                "xmp:Nothing",
            ]
        );
        assert_eq!(form.text(EditorField::Headline), "{date}");
        assert_eq!(form.text(EditorField::Keywords), "a, b");
        assert_eq!(form.text(EditorField::Title), "T");
        assert!(form.checked(EditorField::Copyrighted));
        assert_eq!(form.text(EditorField::ModelReleaseStatus), "MR-NON");
        assert_eq!(form.text(EditorField::Email), "a@example.com");
        assert_eq!(form.list_count(LOCATION_CREATED), 1);
        assert_eq!(
            form.list_text(LOCATION_CREATED, 0, "Iptc4xmpExt:City"),
            "Lund"
        );
        assert_eq!(
            form.list_text(LOCATION_SHOWN, 0, "Iptc4xmpExt:City"),
            "{folder}"
        );
        // Counts as typed: checked at once, and the hidden sub-field kept in the entry.
        let values = form.template_values();
        assert_eq!(
            values.list(LOCATION_SHOWN).unwrap().entries,
            [StructValue {
                fields: vec![
                    ("Iptc4xmpExt:City".to_string(), text("{folder}")),
                    ("Iptc4xmpExt:LocationName".to_string(), text("{name}")),
                ],
            }]
        );
        assert_eq!(
            values.list(LOCATION_CREATED).unwrap().entries,
            [StructValue {
                fields: vec![("Iptc4xmpExt:City".to_string(), text("Lund"))],
            }]
        );
        // Nothing escaped, nothing compared with a start text.
        assert_eq!(form.edits().map(|edits| edits.len()), Ok(8));

        // False leaves the box unticked; another value is not loaded.
        let mut form = EditorForm::empty();
        assert!(
            form.put_template_fields(&[("xmpRights:Marked".to_string(), text("False"))])
                .is_empty()
        );
        assert!(!form.checked(EditorField::Copyrighted));
        assert_eq!(
            form.put_template_fields(&[("xmpRights:Marked".to_string(), text("Maybe"))]),
            ["xmpRights:Marked"]
        );
        assert!(!form.checked(EditorField::Copyrighted));
    }

    // --- Templates in the metadata window (ADR 0017 U13-Q7, U13-Q23–U13-Q25, T300) ---

    use crate::template_form::{
        TemplateFieldError, TemplateItem, TemplateRefused, window_template,
    };

    const C_EMAIL: &str = "Iptc4xmpCore:CiEmailWork";
    const C_CITY: &str = "Iptc4xmpCore:CiAdrCity";
    const C_PHONE: &str = "Iptc4xmpCore:CiTelWork";

    /// A template of `fields` and `add` as given, not cleaned by `TemplateFile::new` (so it can
    /// hold a property no template holds).
    fn template_of(fields: Vec<(&str, Value)>, add: &[&str]) -> TemplateFile {
        TemplateFile {
            fields: fields
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect(),
            add: add.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn template_field(template: &TemplateFile, property: &str) -> Option<Value> {
        template
            .fields
            .iter()
            .find(|(id, _)| id == property)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn load_template_puts_a_text_a_lang_alt_a_date_and_copyrighted_in_the_form() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Old"));
        insert(
            &mut metadata,
            "dc:title",
            lang_alt(&[("sv", "Gammal"), ("x-default", "Old")]),
        );
        insert(&mut metadata, "xmpRights:Marked", text("True"));
        let mut form = form_with(metadata);
        let template = template_of(
            vec![
                ("photoshop:Headline", text("New")),
                ("dc:title", lang_alt(&[("x-default", "Title")])),
                ("photoshop:DateCreated", date("2024-05-01")),
                ("xmpRights:Marked", text("False")),
            ],
            &[],
        );
        let not_loaded = form.load_template(&template, &EditorLayout::default());
        assert_eq!(not_loaded, Vec::<String>::new());
        assert_eq!(form.text(EditorField::Headline), "New");
        assert_eq!(form.text(EditorField::Title), "Title");
        assert_eq!(form.text(EditorField::DateCreated), "2024-05-01");
        assert!(!form.checked(EditorField::Copyrighted));

        // `True` ticks it.
        let mut form = form_with(Metadata::new());
        form.load_template(
            &template_of(vec![("xmpRights:Marked", text("true"))], &[]),
            &EditorLayout::default(),
        );
        assert!(form.checked(EditorField::Copyrighted));
    }

    #[test]
    fn load_template_replaces_keywords_without_plus_and_adds_after_them_with_it() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "dc:subject", list(&["a", "b"]));
        let keywords = || vec![("dc:subject", list(&["c", "a"]))];

        let mut form = form_with(metadata.clone());
        form.load_template(&template_of(keywords(), &[]), &EditorLayout::default());
        assert_eq!(form.text(EditorField::Keywords), "c, a");

        // With +: the form's own first, the template's after them, "a" not again.
        let mut form = form_with(metadata);
        form.load_template(
            &template_of(keywords(), &["dc:subject"]),
            &EditorLayout::default(),
        );
        assert_eq!(form.text(EditorField::Keywords), "a, b, c");
    }

    #[test]
    fn load_template_replaces_only_the_contact_fields_it_holds() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            CONTACT_INFO,
            contact(&[
                (C_EMAIL, "old@example.com"),
                (C_CITY, "Lund"),
                (C_PHONE, "1"),
            ]),
        );
        let mut form = form_with(metadata);
        let template = template_of(
            vec![(CONTACT_INFO, contact(&[(C_CITY, "Malmö"), (C_PHONE, "2")]))],
            &[],
        );
        assert!(
            form.load_template(&template, &EditorLayout::default())
                .is_empty()
        );
        assert_eq!(form.text(EditorField::Email), "old@example.com");
        assert_eq!(form.text(EditorField::ContactCity), "Malmö");
        assert_eq!(form.text(EditorField::Phone), "2");
    }

    #[test]
    fn load_template_sets_a_location_created_city_and_replaces_location_shown() {
        let mut form = form_with(located(vec![umea(), oslo()], vec![umea(), oslo()]));
        let template = template_of(
            vec![
                (CREATED, places(vec![place(&[(CITY, "Lund")])])),
                (SHOWN, places(vec![place(&[(CITY, "Bergen")])])),
            ],
            &[],
        );
        assert!(
            form.load_template(&template, &EditorLayout::default())
                .is_empty()
        );
        // Only City of the first entry; its other sub-fields and the second entry are kept.
        assert_eq!(form.location_count(CREATED), 2);
        assert_eq!(form.location_text(CREATED, 0, CITY), "Lund");
        assert_eq!(form.location_text(CREATED, 0, COUNTRY), "Sverige");
        assert_eq!(form.location_text(CREATED, 1, CITY), "Oslo");
        // Location Shown's entries are replaced.
        assert_eq!(form.location_count(SHOWN), 1);
        assert_eq!(form.location_text(SHOWN, 0, CITY), "Bergen");
        assert_eq!(form.location_text(SHOWN, 0, COUNTRY), "");

        // A form without a Location Created entry gets one.
        let mut form = form_with(Metadata::new());
        form.load_template(&template, &EditorLayout::default());
        assert_eq!(form.location_count(CREATED), 1);
        assert_eq!(form.location_text(CREATED, 0, CITY), "Lund");
    }

    #[test]
    fn load_template_leaves_out_what_is_hidden_read_only_or_unknown() {
        let mut metadata = located(vec![oslo()], vec![umea()]);
        insert(&mut metadata, "photoshop:Headline", text("Old"));
        insert(&mut metadata, "xmpRights:Marked", text("Ja"));
        insert(
            &mut metadata,
            CONTACT_INFO,
            contact(&[(C_EMAIL, "old@example.com")]),
        );
        let mut form = form_with(metadata);
        let mut layout = EditorLayout::default();
        layout.set_item_shown(EditorItem::Field(EditorField::Headline), false);
        layout.set_item_shown(EditorItem::ContactInfo, false);
        layout.set_item_shown(EditorItem::List(SHOWN), false);
        let template = template_of(
            vec![
                ("photoshop:Headline", text("New")),
                ("Iptc4xmpExt:Unknown", text("x")),
                ("xmpRights:Marked", text("True")),
                (CONTACT_INFO, contact(&[(C_EMAIL, "new@example.com")])),
                (
                    CREATED,
                    places(vec![entry(vec![(
                        NAME,
                        lang_alt(&[("x-default", "Home")]),
                    )])]),
                ),
                (SHOWN, places(vec![oslo()])),
                ("photoshop:City", text("Lund")),
            ],
            &[],
        );
        assert_eq!(
            form.load_template(&template, &layout),
            [
                "photoshop:Headline".to_string(),
                "Iptc4xmpExt:Unknown".to_string(),
                "xmpRights:Marked".to_string(),
                format!("{CONTACT_INFO}/{C_EMAIL}"),
                format!("{CREATED}/{NAME}"),
                SHOWN.to_string(),
            ]
        );
        // They are left as they were; only City was loaded.
        assert_eq!(form.text(EditorField::Headline), "Old");
        assert_eq!(form.read_only_text(EditorField::Copyrighted), Some("Ja"));
        assert_eq!(form.text(EditorField::Email), "old@example.com");
        assert_eq!(form.location_count(CREATED), 1);
        assert_eq!(form.location_text(CREATED, 0, CITY), "Oslo");
        assert_eq!(form.location_text(SHOWN, 0, CITY), "Umeå");
        assert_eq!(form.edits(), Ok(vec![set("photoshop:City", text("Lund"))]));
    }

    #[test]
    fn a_loaded_template_counts_as_changed_in_the_batch() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Old"));
        insert(&mut metadata, "dc:subject", list(&["a"]));
        let mut form = form_with(metadata);
        form.load_template(
            &template_of(
                vec![
                    ("photoshop:Headline", text("New")),
                    ("dc:subject", list(&["b"])),
                ],
                &["dc:subject"],
            ),
            &EditorLayout::default(),
        );
        let batch = form.batch_edits().unwrap();
        assert_eq!(batch.changed, [EditorField::Headline]);
        assert_eq!(
            batch.list_changes,
            [ListChange {
                field: EditorField::Keywords,
                adds: vec!["b".to_string()],
                removes: Vec::new(),
                removes_here: Vec::new(),
            }]
        );
    }

    #[test]
    fn a_loaded_variable_is_written_as_typed() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("Old"));
        let mut form = form_with(metadata);
        form.load_template(
            &template_of(vec![("photoshop:Headline", text("{date}"))], &[]),
            &EditorLayout::default(),
        );
        assert_eq!(
            form.edits(),
            Ok(vec![set("photoshop:Headline", text("{date}"))])
        );
    }

    #[test]
    fn window_template_escapes_what_was_read_and_keeps_what_was_typed() {
        let mut metadata = located(Vec::new(), vec![place(&[(CITY, "{c}")])]);
        insert(&mut metadata, "photoshop:Headline", text("a {b}"));
        insert(&mut metadata, "dc:subject", list(&["x{1}"]));
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Keywords, "x{1}, {date}");
        let template = window_template(&form, &EditorLayout::default()).unwrap();
        assert_eq!(
            template_field(&template, "photoshop:Headline"),
            Some(text("a {{b}}"))
        );
        assert_eq!(
            template_field(&template, "dc:subject"),
            Some(list(&["x{{1}}", "{date}"]))
        );
        assert_eq!(
            template_field(&template, SHOWN),
            Some(places(vec![place(&[(CITY, "{{c}}")])]))
        );
        assert!(template.add.is_empty());

        // The same Headline typed as a variable is kept.
        type_into(&mut form, EditorField::Headline, "{date}");
        let template = window_template(&form, &EditorLayout::default()).unwrap();
        assert_eq!(
            template_field(&template, "photoshop:Headline"),
            Some(text("{date}"))
        );
    }

    #[test]
    fn window_template_gives_copyrighted_true_or_nothing() {
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("h"));
        insert(&mut metadata, "xmpRights:Marked", text("True"));
        let mut form = form_with(metadata);
        let template = window_template(&form, &EditorLayout::default()).unwrap();
        assert_eq!(
            template_field(&template, "xmpRights:Marked"),
            Some(text("True"))
        );
        form.set_checked(EditorField::Copyrighted, false);
        let template = window_template(&form, &EditorLayout::default()).unwrap();
        assert_eq!(template_field(&template, "xmpRights:Marked"), None);
    }

    #[test]
    fn window_template_leaves_out_hidden_and_read_only_items() {
        let mut metadata = located(Vec::new(), vec![oslo()]);
        insert(&mut metadata, "photoshop:Headline", text("h"));
        insert(&mut metadata, "photoshop:Credit", text("c"));
        insert(&mut metadata, "xmpRights:Marked", text("Ja"));
        insert(&mut metadata, "photoshop:Category", text("SPO"));
        let form = form_with(metadata);
        let mut layout = EditorLayout::default();
        layout.set_item_shown(EditorItem::Field(EditorField::Headline), false);
        layout.set_item_shown(EditorItem::List(SHOWN), false);
        let template = window_template(&form, &layout).unwrap();
        assert_eq!(
            template.fields,
            [("photoshop:Credit".to_string(), text("c"))]
        );
    }

    #[test]
    fn window_template_of_an_empty_form_is_refused() {
        assert_eq!(
            window_template(&form_with(Metadata::new()), &EditorLayout::default()),
            Err(TemplateRefused::Empty)
        );
    }

    #[test]
    fn a_shown_item_with_an_error_refuses_the_window_template() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Headline, "{nope}");
        type_into(&mut form, EditorField::City, "Lund");
        form.add_location(SHOWN);
        type_location(&mut form, SHOWN, 0, CITY, "{x");
        let headline_error = field_edit(EditorField::Headline, "", "{nope}").unwrap_err();
        let city_error = form.list_error(SHOWN, 0, CITY).unwrap();
        assert_eq!(
            window_template(&form, &EditorLayout::default()),
            Err(TemplateRefused::FieldErrors(vec![
                TemplateFieldError {
                    item: TemplateItem::Field(EditorField::Headline),
                    entry: None,
                    text: headline_error,
                },
                TemplateFieldError {
                    item: TemplateItem::List(SHOWN),
                    entry: Some((0, CITY)),
                    text: city_error,
                },
            ]))
        );

        // Hidden, their errors do not refuse it.
        let mut layout = EditorLayout::default();
        layout.set_item_shown(EditorItem::Field(EditorField::Headline), false);
        layout.set_item_shown(EditorItem::List(SHOWN), false);
        assert_eq!(
            window_template(&form, &layout).map(|template| template.fields),
            Ok(vec![("photoshop:City".to_string(), text("Lund"))])
        );
    }

    #[test]
    fn window_template_gives_one_contact_structure_and_one_location_created_entry() {
        let mut metadata = located(
            vec![
                entry(vec![
                    (CITY, text("Lund")),
                    (NAME, lang_alt(&[("x-default", "Home")])),
                ]),
                oslo(),
            ],
            Vec::new(),
        );
        insert(
            &mut metadata,
            CONTACT_INFO,
            contact(&[(C_EMAIL, "a@example.com"), (C_CITY, "Lund")]),
        );
        let mut form = form_with(metadata);
        type_into(&mut form, EditorField::Phone, "2");
        let template = window_template(&form, &EditorLayout::default()).unwrap();
        let Some(Value::Struct(contact_info)) = template_field(&template, CONTACT_INFO) else {
            panic!("no Creator's Contact Info structure");
        };
        let mut sub_fields = contact_info.fields.clone();
        sub_fields.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            sub_fields,
            [
                (C_CITY.to_string(), text("Lund")),
                (C_EMAIL.to_string(), text("a@example.com")),
                (C_PHONE.to_string(), text("2")),
            ]
        );
        // The first entry, only its sub-fields the form shows (not Location Name).
        assert_eq!(
            template_field(&template, CREATED),
            Some(places(vec![place(&[(CITY, "Lund")])]))
        );
    }

    #[test]
    fn window_template_keeps_x_default_and_leaves_out_a_list_over_its_limit() {
        let mut metadata = Metadata::new();
        insert(
            &mut metadata,
            "dc:title",
            lang_alt(&[("sv", "Solnedgång"), ("x-default", "Sunset")]),
        );
        insert(
            &mut metadata,
            "plus:ImageSupplier",
            places(vec![
                place(&[(SUPPLIER_NAME, "A")]),
                place(&[(SUPPLIER_NAME, "B")]),
            ]),
        );
        let template = window_template(&form_with(metadata), &EditorLayout::default()).unwrap();
        assert_eq!(
            template.fields,
            [("dc:title".to_string(), lang_alt(&[("x-default", "Sunset")]))]
        );
    }

    #[test]
    fn window_template_loads_back_into_an_empty_form() {
        let mut form = form_with(Metadata::new());
        type_into(&mut form, EditorField::Headline, "Head");
        type_into(&mut form, EditorField::Title, "Title");
        type_into(&mut form, EditorField::Keywords, "a, b");
        type_into(&mut form, EditorField::DateCreated, "2024-05-01");
        type_into(&mut form, EditorField::Email, "a@example.com");
        type_into(&mut form, EditorField::ContactCity, "Lund");
        form.set_checked(EditorField::Copyrighted, true);
        let status = EditorField::ModelReleaseStatus.choice_list().unwrap()[0];
        form.set_choice(EditorField::ModelReleaseStatus, status);
        form.add_location(CREATED);
        type_location(&mut form, CREATED, 0, CITY, "Umeå");
        type_location(
            &mut form,
            CREATED,
            0,
            ID,
            "https://example.org/a, https://example.org/b",
        );
        form.add_location(SHOWN);
        type_location(&mut form, SHOWN, 0, CITY, "Oslo");
        type_location(&mut form, SHOWN, 0, COUNTRY, "Norge");

        let template = window_template(&form, &EditorLayout::default()).unwrap();
        let mut loaded = form_with(Metadata::new());
        assert!(
            loaded
                .load_template(&template, &EditorLayout::default())
                .is_empty()
        );
        for field in EditorField::ALL {
            assert_eq!(loaded.text(field), form.text(field), "{field:?}");
        }
        for list in EditorForm::ALL_LISTS {
            assert_eq!(loaded.list_count(list), form.list_count(list), "{list}");
            for index in 0..form.list_count(list) {
                for field in EditorForm::list_fields(list) {
                    assert_eq!(
                        loaded.list_text(list, index, field),
                        form.list_text(list, index, field),
                        "{list} {field}"
                    );
                }
            }
        }

        // A text with braces read from an image comes back doubled, which writes `{b}`.
        let mut metadata = Metadata::new();
        insert(&mut metadata, "photoshop:Headline", text("a {b}"));
        let template = window_template(&form_with(metadata), &EditorLayout::default()).unwrap();
        let mut loaded = form_with(Metadata::new());
        loaded.load_template(&template, &EditorLayout::default());
        assert_eq!(loaded.text(EditorField::Headline), "a {{b}}");
        assert_eq!(
            loaded.edits(),
            Ok(vec![set("photoshop:Headline", text("a {{b}}"))])
        );
    }
}
