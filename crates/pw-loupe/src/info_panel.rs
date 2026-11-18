//! The texts of the info panel (ADR 0007 I10i): the values, the conflicts and the notices of one
//! image's [`MetadataRead`], and the panel's fixed texts. Pure functions, no egui; T103 draws them.
//!
//! - **Values** (§5.1): every property with a value, in the order of the property table, with its
//!   shown text and where it came from.
//! - **Conflicts** (§5.1, "conflicts always visible"): every value with its origin and the rule
//!   that was applied, until the next write by RawStride.
//! - **Notices**: ignored sidecars (Q5, Q8), an unreadable IIM block, converted IIM text (§5.4),
//!   refused and unreadable values, the suggested Date Created (§7) and the IIM checksum (§5.1).
//!
//! Text rule (owner, 2026-11-12): every text the user sees in the info panel is made here. Each
//! text is a whole sentence built from one template with its values filled in, so another language
//! can change the word order. Property names come only from [`property_label`]: English (glossary
//! G1, G2; T117), with its exceptions in the one table [`EN_LABELS`]; the Swedish names of owner
//! decision D11 stay in the one table [`SV_LABELS`] for a second language later.

use std::path::Path;

use pw_metadata::iim::is_blank;
use pw_metadata::iim_read::IimNotice;
use pw_metadata::properties::{self, ValueKind};
use pw_metadata::read::{FileNotice, MetadataRead};
use pw_metadata::reconcile::{IimDigest, Notice};
use pw_metadata::value::{ConflictRule, LangAlt, Origin, StructValue, Value, ValueError};

/// The language of a property name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    /// Swedish: the names of D11, IPTC's English name for every other property.
    Sv,
    /// English: IPTC's name from the property table, with the exceptions of glossary G1 and G2.
    En,
}

/// The Swedish names of the properties and structure fields of owner decision D11, keyed by XMP
/// id. No other code holds property names.
const SV_LABELS: &[(&str, &str)] = &[
    ("dc:title", "Titel"),
    ("photoshop:Headline", "Rubrik"),
    ("dc:description", "Beskrivning"),
    ("Iptc4xmpCore:AltTextAccessibility", "Alternativtext"),
    ("dc:subject", "Nyckelord"),
    ("dc:creator", "Fotograf"),
    ("photoshop:AuthorsPosition", "Yrkestitel"),
    ("Iptc4xmpCore:CreatorContactInfo", "Kontaktuppgifter"),
    ("Iptc4xmpCore:CiEmailWork", "E-post"),
    ("Iptc4xmpCore:CiTelWork", "Telefon"),
    ("Iptc4xmpCore:CiUrlWork", "Webb"),
    ("dc:rights", "Copyright"),
    ("photoshop:Credit", "Credit"),
    ("photoshop:Source", "Källa"),
    ("xmpRights:UsageTerms", "Användarvillkor"),
    ("xmpRights:WebStatement", "Webbadress för rättigheter"),
    ("photoshop:DateCreated", "Skapad"),
    ("Iptc4xmpCore:Location", "Plats"),
    ("photoshop:City", "Stad"),
    ("photoshop:State", "Län/delstat"),
    ("photoshop:Country", "Land"),
    ("Iptc4xmpCore:CountryCode", "Landskod"),
    ("photoshop:Instructions", "Instruktioner"),
    ("photoshop:TransmissionReference", "Jobb-id"),
    ("photoshop:CaptionWriter", "Beskrivningens författare"),
    ("xmp:Rating", "Betyg"),
    ("xmp:Label", "Etikett"),
    ("xmpRights:Marked", "Upphovsrättsskyddad"),
    ("Iptc4xmpExt:LocationCreated", "Plats där bilden togs"),
    ("Iptc4xmpExt:LocationShown", "Plats som visas"),
    ("plus:ImageCreator", "Bildskapare"),
    ("plus:CopyrightOwner", "Upphovsrättsinnehavare"),
    ("plus:Licensor", "Licensgivare"),
    ("Iptc4xmpExt:PersonInImage", "Person i bilden"),
    ("Iptc4xmpExt:Event", "Händelse"),
    ("Iptc4xmpExt:DigitalSourceType", "Digital källtyp"),
    ("photoshop:Urgency", "Prioritet"),
    ("photoshop:Category", "Kategori"),
    ("photoshop:SupplementalCategories", "Tilläggskategorier"),
];

/// The English names that differ from the property table's name (glossary G1, G2; T117): the
/// legacy properties without " (legacy)", the short contact fields, and the names of the
/// properties that are not IPTC's or are deprecated. Keyed by XMP id.
const EN_LABELS: &[(&str, &str)] = &[
    ("Iptc4xmpCore:Location", "Sublocation"),
    ("photoshop:City", "City"),
    ("photoshop:State", "Province or State"),
    ("photoshop:Country", "Country"),
    ("Iptc4xmpCore:CountryCode", "Country Code"),
    ("Iptc4xmpCore:IntellectualGenre", "Intellectual Genre"),
    ("Iptc4xmpCore:SubjectCode", "Subject Code"),
    ("Iptc4xmpCore:CiEmailWork", "Email"),
    ("Iptc4xmpCore:CiTelWork", "Phone"),
    ("Iptc4xmpCore:CiUrlWork", "Web URL"),
    ("xmp:Rating", "Rating"),
    ("xmpRights:Marked", "Copyrighted"),
    ("xmp:Label", "Label"),
    ("photoshop:Urgency", "Urgency"),
    ("photoshop:Category", "Category"),
    (
        "photoshop:SupplementalCategories",
        "Supplemental Categories",
    ),
];

/// The properties and fields whose full IPTC name is shown as the tooltip of their shorter label
/// (T117): the legacy properties and the contact fields.
const FULL_NAME_TOOLTIPS: &[&str] = &[
    "Iptc4xmpCore:Location",
    "photoshop:City",
    "photoshop:State",
    "photoshop:Country",
    "Iptc4xmpCore:CountryCode",
    "Iptc4xmpCore:IntellectualGenre",
    "Iptc4xmpCore:SubjectCode",
    "Iptc4xmpCore:CiEmailWork",
    "Iptc4xmpCore:CiTelWork",
    "Iptc4xmpCore:CiUrlWork",
];

/// The name of a property or structure field (owner, 2026-11-12; T117):
/// - `Sv`: the Swedish name of D11 for the properties and fields in its list, otherwise IPTC's
///   name from the property table;
/// - `En`: the name of [`EN_LABELS`] for the properties and fields in its list, otherwise IPTC's
///   name from the property table;
/// - IPTC's name is the `name` of the property, or of the field in its structure; an unknown id is
///   its own name.
pub fn property_label(xmp_id: &str, language: Language) -> String {
    let labels = match language {
        Language::Sv => SV_LABELS,
        Language::En => EN_LABELS,
    };
    if let Some((_, label)) = labels.iter().find(|(id, _)| *id == xmp_id) {
        return (*label).to_string();
    }
    iptc_name(xmp_id)
}

/// IPTC's name of a property or structure field from the property table, otherwise the id.
fn iptc_name(xmp_id: &str) -> String {
    if let Some(def) = properties::property(xmp_id) {
        return def.name.to_string();
    }
    match field_name(xmp_id) {
        Some(name) => name.to_string(),
        None => xmp_id.to_string(),
    }
}

/// The tooltip of a property's English label in the metadata window: the full IPTC name of a
/// legacy property or a contact field, whose label leaves part of it out (glossary G1; T117).
/// `None` for every other property and field.
pub fn property_tooltip(xmp_id: &str) -> Option<String> {
    if !FULL_NAME_TOOLTIPS.contains(&xmp_id) {
        return None;
    }
    let full = iptc_name(xmp_id);
    (full != property_label(xmp_id, Language::En)).then_some(full)
}

/// The IPTC name of the structure field `xmp_id`, from the first structure (reached from the
/// properties in table order, then through structure fields) that has it.
fn field_name(xmp_id: &str) -> Option<&'static str> {
    let mut pending: Vec<&'static str> = properties::properties()
        .filter_map(|def| match def.kind {
            ValueKind::Struct(name) => Some(name),
            _ => None,
        })
        .collect();
    let mut seen: Vec<&'static str> = Vec::new();
    let mut next = 0;
    while let Some(&name) = pending.get(next) {
        next += 1;
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let Some(def) = properties::struct_def(name) else {
            continue;
        };
        for field in def.fields {
            if field.xmp_id == xmp_id {
                return Some(field.name);
            }
            if let ValueKind::Struct(inner) = field.kind {
                pending.push(inner);
            }
        }
    }
    None
}

/// The text of a value: `Text` and `Number` as they are; a Lang Alt its shown text, with
/// ` (<language>)` when it is not `x-default` and ` +N languages` when it has N other entries; a
/// list its entries joined with `; `; a date in XMP form; a structure `field: value` joined with
/// `, `, without its blank fields; a list
/// of structures one structure per line, without lines for structures that have no field left.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::Text(text) | Value::Number(text) => text.clone(),
        Value::LangAlt(lang_alt) => lang_alt_text(lang_alt),
        Value::List(entries) => entries.join("; "),
        Value::Date(date) => date.to_xmp(),
        Value::Struct(fields) => struct_text(fields),
        Value::StructList(list) => list
            .iter()
            .map(struct_text)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// The shown text of a Lang Alt (§8 reading rule), its language when it is not `x-default`, and
/// the number of other entries.
fn lang_alt_text(lang_alt: &LangAlt) -> String {
    let Some((text, language)) = lang_alt.display() else {
        return String::new();
    };
    let others = lang_alt.entries().len().saturating_sub(1);
    match (language, others) {
        (None, 0) => text.to_string(),
        (None, 1) => format!("{text} +1 language"),
        (None, n) => format!("{text} +{n} languages"),
        (Some(language), 0) => format!("{text} ({language})"),
        (Some(language), 1) => format!("{text} ({language}) +1 language"),
        (Some(language), n) => format!("{text} ({language}) +{n} languages"),
    }
}

/// A structure as `field: value` joined with `, `, leaving out the blank fields (T130; a blank
/// value counts as missing, ADR 0007 §5.1). A structure with no field left gives an empty text.
fn struct_text(value: &StructValue) -> String {
    value
        .fields
        .iter()
        .filter(|(_, field)| !is_blank_field(field))
        .map(|(id, field)| {
            format!(
                "{label}: {text}",
                label = property_label(id, Language::En),
                text = value_text(field)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether a structure's field is blank and so left out of its text (T130): a `Text` or `Number`
/// that is empty or only spaces and/or NUL ([`is_blank`]), a Lang Alt whose shown text is blank,
/// an empty list, and a structure or list of structures whose own text is empty.
fn is_blank_field(field: &Value) -> bool {
    match field {
        Value::Text(text) | Value::Number(text) => is_blank(text),
        Value::LangAlt(lang_alt) => lang_alt.display().is_none_or(|(text, _)| is_blank(text)),
        Value::List(entries) => entries.is_empty(),
        Value::Date(_) => false,
        Value::Struct(_) | Value::StructList(_) => value_text(field).is_empty(),
    }
}

/// Where a value came from.
pub fn origin_text(origin: Origin) -> &'static str {
    match origin {
        Origin::XmpEmbedded => "XMP",
        Origin::XmpSidecar => "XMP sidecar",
        Origin::Iim => "IIM",
        Origin::Exif => "Exif",
        Origin::Edited => "Changed",
    }
}

/// The rule that decided a conflict (§5.1).
pub fn rule_text(rule: ConflictRule) -> &'static str {
    match rule {
        ConflictRule::Xmp => "XMP is used",
        ConflictRule::Iim => "IIM is used (the checksum does not match)",
        ConflictRule::Exif => "Exif is used",
        ConflictRule::Open => "Unresolved: choose a value in the editor",
    }
}

/// Why a value was refused.
fn refused_reason(error: &ValueError) -> &'static str {
    match error {
        ValueError::Kind { .. } => "wrong form",
        ValueError::TooMany { .. } => "too many values",
        ValueError::Empty(_) => "empty value",
        ValueError::UnknownProperty(_) => "unknown field",
    }
}

/// One property with a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueRow {
    pub label: String,
    pub value: String,
    pub origin: String,
}

/// One value of a conflict and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictValue {
    pub value: String,
    pub origin: String,
}

/// One conflict: the property, each value with its origin, and the rule applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictRow {
    pub label: String,
    pub values: Vec<ConflictValue>,
    pub rule: String,
    /// The rule is `Open`: no rule decided it, the user chooses (D16, T107).
    pub open: bool,
}

/// The texts of the info panel for one image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoPanel {
    pub values: Vec<ValueRow>,
    pub conflicts: Vec<ConflictRow>,
    pub notices: Vec<String>,
}

/// The rows of the properties with a value, in table order, for the properties `keep` accepts.
fn value_rows(read: &MetadataRead, keep: impl Fn(&str) -> bool) -> Vec<ValueRow> {
    let metadata = &read.reconciled.metadata;
    properties::properties()
        .filter(|def| keep(def.xmp_id))
        .filter_map(|def| {
            metadata.get(def.xmp_id).map(|found| ValueRow {
                label: property_label(def.xmp_id, Language::En),
                value: value_text(&found.value),
                origin: origin_text(found.origin).to_string(),
            })
        })
        .collect()
}

/// The `values` of [`info_panel`] without the properties in `editor_properties`: the other
/// metadata of the metadata window (D16, T107), in table order.
pub fn other_values(read: &MetadataRead, editor_properties: &[&str]) -> Vec<ValueRow> {
    value_rows(read, |id| !editor_properties.contains(&id))
}

/// The info panel of one image: its values in table order, its conflicts and its notices.
pub fn info_panel(read: &MetadataRead) -> InfoPanel {
    let metadata = &read.reconciled.metadata;
    let values = value_rows(read, |_| true);
    let conflicts = metadata
        .conflicts()
        .iter()
        .map(|conflict| ConflictRow {
            label: property_label(&conflict.property, Language::En),
            values: conflict
                .values
                .iter()
                .map(|(origin, value)| ConflictValue {
                    value: value_text(value),
                    origin: origin_text(*origin).to_string(),
                })
                .collect(),
            rule: rule_text(conflict.rule).to_string(),
            open: matches!(conflict.rule, ConflictRule::Open),
        })
        .collect();
    InfoPanel {
        values,
        conflicts,
        notices: notices(read),
    }
}

/// The notices, in this order: the file notices, the IIM notices, the refused values, the
/// unreadable values, the suggested Date Created, the IIM checksum.
fn notices(read: &MetadataRead) -> Vec<String> {
    let reconciled = &read.reconciled;
    let mut out: Vec<String> = read.file_notices.iter().map(file_notice_text).collect();
    out.extend(reconciled.notices.iter().filter_map(|notice| match notice {
        Notice::Iim(iim) => Some(iim_notice_text(iim)),
        _ => None,
    }));
    out.extend(reconciled.notices.iter().filter_map(|notice| match notice {
        Notice::Refused {
            property,
            origin,
            error,
        } => Some(format!(
            "{field} from {origin} was not read: {reason}",
            field = property_label(property, Language::En),
            origin = origin_text(*origin),
            reason = refused_reason(error)
        )),
        _ => None,
    }));
    out.extend(reconciled.notices.iter().filter_map(|notice| match notice {
        Notice::Unreadable {
            property,
            origin,
            text,
        } => Some(format!(
            "{field} from {origin} could not be read ({text})",
            field = property_label(property, Language::En),
            origin = origin_text(*origin)
        )),
        _ => None,
    }));
    if let Some(date) = &reconciled.suggested_date_created {
        out.push(format!(
            "Suggested {field} from the camera: {date}",
            field = property_label("photoshop:DateCreated", Language::En),
            date = date.to_xmp()
        ));
    }
    if read.state.iim_present {
        match read.state.digest {
            IimDigest::Matches => {}
            IimDigest::Differs => out.push(
                "IIM has been changed by another program after XMP (the checksum does not match)"
                    .to_string(),
            ),
            IimDigest::Missing => out.push("IIM has no checksum".to_string()),
        }
    }
    out
}

/// The text of a notice about the files next to the image or its IIM block.
fn file_notice_text(notice: &FileNotice) -> String {
    match notice {
        FileNotice::ForeignSidecarIgnored(path) => format!(
            "{file} next to the DNG file is not used (written by another program)",
            file = file_name(path)
        ),
        FileNotice::ExtSidecarIgnored(path) => format!(
            "{file} is not read (RawStride uses {base}.xmp)",
            file = file_name(path),
            base = base_name(path)
        ),
        FileNotice::IimBlockUnreadable(reason) => {
            format!("The IIM block could not be read: {reason}")
        }
        FileNotice::ListWithQualifiers(property) => format!(
            "{field} has qualifiers RawStride cannot keep, so it is shown but never changed",
            field = property_label(property, Language::En)
        ),
    }
}

/// The text of a notice about an IIM value.
fn iim_notice_text(notice: &IimNotice) -> String {
    match notice {
        IimNotice::Converted { property } => format!(
            "{field}: the IIM text had an unknown character encoding and has been converted",
            field = property_label(property, Language::En)
        ),
        IimNotice::Repeated { property } => format!(
            "{field}: appeared more than once in IIM; the first value is used",
            field = property_label(property, Language::En)
        ),
        IimNotice::Unreadable { property, text } => format!(
            "{field}: could not be read ({text})",
            field = property_label(property, Language::En)
        ),
    }
}

/// The file name of `path`, or the whole path when it has none.
fn file_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

/// The image's base name from a `<basename>.<ext>.xmp` sidecar (`DSC01234` from
/// `DSC01234.ARW.xmp`).
fn base_name(path: &Path) -> String {
    let image = path.file_stem().map(Path::new);
    match image.and_then(Path::file_stem) {
        Some(base) => base.to_string_lossy().into_owned(),
        None => file_name(path),
    }
}

/// A fixed text of the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelText {
    /// More than one image is selected.
    Selected(usize),
    NoneSelected,
    Loading,
    /// The metadata could not be read; holds the reason.
    ReadError(String),
    /// The heading of the values.
    Values,
    /// The heading of the conflicts.
    Conflicts,
    /// The heading of the notices.
    Notices,
}

/// The text of a fixed text of the panel.
pub fn panel_text(text: &PanelText) -> String {
    match text {
        PanelText::Selected(1) => "1 image marked".to_string(),
        PanelText::Selected(n) => format!("{n} images marked"),
        PanelText::NoneSelected => "No image selected".to_string(),
        PanelText::Loading => "Reading metadata …".to_string(),
        PanelText::ReadError(error) => format!("The metadata could not be read: {error}"),
        PanelText::Values => "Metadata".to_string(),
        PanelText::Conflicts => "Conflicts".to_string(),
        PanelText::Notices => "Notices".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use pw_metadata::date::{IptcDate, Time, Zone};
    use pw_metadata::iim::CodedCharset;
    use pw_metadata::reconcile::{FileKind, Reconciled};
    use pw_metadata::value::{Conflict, Metadata};
    use pw_metadata::write_plan::{ExifPresent, FileState};

    fn state(iim_present: bool, digest: IimDigest) -> FileState {
        FileState {
            kind: FileKind::Jpeg,
            iim_present,
            iim_charset: CodedCharset::Absent,
            iim_values: Vec::new(),
            digest,
            exif_present: ExifPresent::default(),
        }
    }

    fn read_with(
        metadata: Metadata,
        notices: Vec<Notice>,
        file_notices: Vec<FileNotice>,
    ) -> MetadataRead {
        MetadataRead {
            reconciled: Reconciled {
                metadata,
                notices,
                suggested_date_created: None,
            },
            file_notices,
            state: state(false, IimDigest::Missing),
        }
    }

    fn empty_read() -> MetadataRead {
        read_with(Metadata::new(), Vec::new(), Vec::new())
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

    fn date(zone: Option<Zone>) -> IptcDate {
        IptcDate {
            year: 2026,
            month: Some(11),
            day: Some(12),
            time: Some(Time {
                hour: 10,
                minute: 15,
                second: Some(30),
                fraction: None,
                zone,
            }),
        }
    }

    fn location(city: &str, country: &str) -> StructValue {
        StructValue {
            fields: vec![
                ("Iptc4xmpExt:City".to_string(), text(city)),
                ("Iptc4xmpExt:CountryName".to_string(), text(country)),
            ],
        }
    }

    // --- property_label ---

    #[test]
    fn label_of_listed_property() {
        assert_eq!(property_label("dc:creator", Language::Sv), "Fotograf");
        assert_eq!(property_label("dc:creator", Language::En), "Creator");
    }

    #[test]
    fn label_of_jobtitle_and_contact_info() {
        assert_eq!(
            property_label("photoshop:AuthorsPosition", Language::Sv),
            "Yrkestitel"
        );
        assert_eq!(
            property_label("Iptc4xmpCore:CreatorContactInfo", Language::Sv),
            "Kontaktuppgifter"
        );
        // The English names are IPTC's, unchanged.
        assert_eq!(
            property_label("photoshop:AuthorsPosition", Language::En),
            "Creator's jobtitle"
        );
        assert_eq!(
            property_label("Iptc4xmpCore:CreatorContactInfo", Language::En),
            "Creator's Contact Info"
        );
    }

    #[test]
    fn label_of_unlisted_property_is_iptc_name_in_both_languages() {
        assert_eq!(property_label("Iptc4xmpExt:Genre", Language::Sv), "Genre");
        assert_eq!(property_label("Iptc4xmpExt:Genre", Language::En), "Genre");
    }

    #[test]
    fn label_of_unknown_id_is_the_id() {
        assert_eq!(property_label("foo:Bar", Language::Sv), "foo:Bar");
        assert_eq!(property_label("foo:Bar", Language::En), "foo:Bar");
    }

    #[test]
    fn label_of_contact_field() {
        assert_eq!(
            property_label("Iptc4xmpCore:CiEmailWork", Language::Sv),
            "E-post"
        );
        assert_eq!(
            property_label("Iptc4xmpCore:CiEmailWork", Language::En),
            "Email"
        );
    }

    #[test]
    fn label_of_unlisted_structure_field_is_its_iptc_name() {
        assert_eq!(property_label("Iptc4xmpExt:City", Language::Sv), "City");
        assert_eq!(
            property_label("Iptc4xmpExt:CountryName", Language::En),
            "Country Name"
        );
    }

    #[test]
    fn every_swedish_label_is_a_known_property_or_field() {
        for (id, _) in SV_LABELS {
            assert!(
                properties::property(id).is_some() || field_name(id).is_some(),
                "{id} is not in the property table"
            );
        }
    }

    // --- value_text ---

    #[test]
    fn value_text_of_text_and_number() {
        assert_eq!(value_text(&text("Stockholm")), "Stockholm");
        assert_eq!(value_text(&Value::Number("2.50".into())), "2.50");
    }

    #[test]
    fn value_text_of_lang_alt_with_x_default_only() {
        assert_eq!(value_text(&lang_alt(&[("x-default", "Hej")])), "Hej");
    }

    #[test]
    fn value_text_of_lang_alt_without_x_default() {
        assert_eq!(
            value_text(&lang_alt(&[("en-US", "Hello")])),
            "Hello (en-US)"
        );
        assert_eq!(
            value_text(&lang_alt(&[("en-US", "Hello"), ("de", "Hallo")])),
            "Hello (en-US) +1 language"
        );
    }

    #[test]
    fn value_text_of_lang_alt_with_two_other_languages() {
        assert_eq!(
            value_text(&lang_alt(&[
                ("en", "Hello"),
                ("x-default", "Hej"),
                ("de", "Hallo")
            ])),
            "Hej +2 languages"
        );
    }

    #[test]
    fn value_text_of_list() {
        assert_eq!(
            value_text(&Value::List(vec![
                "sol".into(),
                "hav".into(),
                "strand".into()
            ])),
            "sol; hav; strand"
        );
    }

    #[test]
    fn value_text_of_date_with_and_without_zone() {
        assert_eq!(value_text(&Value::Date(date(None))), "2026-11-12T10:15:30");
        assert_eq!(
            value_text(&Value::Date(date(Some(Zone::Offset(60))))),
            "2026-11-12T10:15:30+01:00"
        );
    }

    #[test]
    fn value_text_of_contact_structure() {
        let contact = Value::Struct(StructValue {
            fields: vec![
                (
                    "Iptc4xmpCore:CiEmailWork".to_string(),
                    text("foto@example.com"),
                ),
                ("Iptc4xmpCore:CiTelWork".to_string(), text("+46 8 123")),
                ("Iptc4xmpCore:CiAdrCity".to_string(), text("Umeå")),
            ],
        });
        assert_eq!(
            value_text(&contact),
            "Email: foto@example.com, Phone: +46 8 123, City: Umeå"
        );
    }

    #[test]
    fn value_text_of_two_locations_created() {
        let locations =
            Value::StructList(vec![location("Umeå", "Sverige"), location("Oslo", "Norge")]);
        assert_eq!(
            value_text(&locations),
            "City: Umeå, Country Name: Sverige\nCity: Oslo, Country Name: Norge"
        );
    }

    /// A Location Created with every field blank (empty, only spaces, only NUL), as Photo
    /// Mechanic writes the empty sub-fields.
    fn blank_location() -> StructValue {
        StructValue {
            fields: vec![
                ("Iptc4xmpExt:Sublocation".to_string(), text("")),
                ("Iptc4xmpExt:City".to_string(), text("   ")),
                ("Iptc4xmpExt:ProvinceState".to_string(), text("")),
                ("Iptc4xmpExt:CountryName".to_string(), text("\0\0")),
                ("Iptc4xmpExt:WorldRegion".to_string(), text("")),
            ],
        }
    }

    #[test]
    fn value_text_of_location_leaves_out_blank_fields() {
        let location = Value::Struct(StructValue {
            fields: vec![
                ("Iptc4xmpExt:Sublocation".to_string(), text("")),
                ("Iptc4xmpExt:City".to_string(), text("Exempelstad")),
                ("Iptc4xmpExt:ProvinceState".to_string(), text("")),
                ("Iptc4xmpExt:CountryName".to_string(), text("Sverige")),
                ("Iptc4xmpExt:WorldRegion".to_string(), text("")),
                ("Iptc4xmpExt:CountryCode".to_string(), text("  ")),
                ("Iptc4xmpExt:LocationId".to_string(), text("\0")),
            ],
        });
        assert_eq!(
            value_text(&location),
            "City: Exempelstad, Country Name: Sverige"
        );
    }

    #[test]
    fn value_text_of_all_blank_structure_is_empty() {
        assert_eq!(value_text(&Value::Struct(blank_location())), "");
    }

    #[test]
    fn value_text_of_two_locations_where_the_first_is_blank() {
        let locations = Value::StructList(vec![blank_location(), location("Oslo", "Norge")]);
        assert_eq!(value_text(&locations), "City: Oslo, Country Name: Norge");
    }

    #[test]
    fn value_text_of_contact_structure_leaves_out_an_empty_field() {
        let contact = StructValue {
            fields: vec![
                (
                    "Iptc4xmpCore:CiEmailWork".to_string(),
                    text("foto@example.com"),
                ),
                ("Iptc4xmpCore:CiTelWork".to_string(), text("")),
                ("Iptc4xmpCore:CiAdrCity".to_string(), text("Umeå")),
            ],
        };
        assert_eq!(
            value_text(&Value::Struct(contact.clone())),
            "Email: foto@example.com, City: Umeå"
        );
        // Nested in another structure: the empty field is left out there too, and a nested
        // structure, Lang Alt or list that is blank is left out as a whole.
        let outer = Value::Struct(StructValue {
            fields: vec![
                (
                    "Iptc4xmpCore:CreatorContactInfo".to_string(),
                    Value::Struct(contact),
                ),
                (
                    "Iptc4xmpExt:LocationCreated".to_string(),
                    Value::Struct(blank_location()),
                ),
                (
                    "Iptc4xmpExt:LocationShown".to_string(),
                    Value::StructList(vec![blank_location()]),
                ),
                ("dc:title".to_string(), lang_alt(&[("x-default", " \0")])),
                ("dc:subject".to_string(), Value::List(Vec::new())),
            ],
        });
        assert_eq!(
            value_text(&outer),
            format!(
                "{}: Email: foto@example.com, City: Umeå",
                property_label("Iptc4xmpCore:CreatorContactInfo", Language::En)
            )
        );
    }

    // --- origin and rule texts ---

    #[test]
    fn every_origin_text() {
        assert_eq!(origin_text(Origin::XmpEmbedded), "XMP");
        assert_eq!(origin_text(Origin::XmpSidecar), "XMP sidecar");
        assert_eq!(origin_text(Origin::Iim), "IIM");
        assert_eq!(origin_text(Origin::Exif), "Exif");
        assert_eq!(origin_text(Origin::Edited), "Changed");
    }

    #[test]
    fn every_rule_text() {
        assert_eq!(rule_text(ConflictRule::Xmp), "XMP is used");
        assert_eq!(
            rule_text(ConflictRule::Iim),
            "IIM is used (the checksum does not match)"
        );
        assert_eq!(rule_text(ConflictRule::Exif), "Exif is used");
        assert_eq!(
            rule_text(ConflictRule::Open),
            "Unresolved: choose a value in the editor"
        );
    }

    // --- info_panel ---

    #[test]
    fn values_follow_the_table_order() {
        let mut metadata = Metadata::new();
        // Inserted out of table order (the table is sorted by IPTC name).
        metadata
            .insert_read(
                "dc:title",
                lang_alt(&[("x-default", "Älv")]),
                Origin::XmpEmbedded,
            )
            .unwrap();
        metadata
            .insert_read("photoshop:City", text("Umeå"), Origin::Iim)
            .unwrap();
        metadata
            .insert_read(
                "dc:creator",
                Value::List(vec!["Anna".into()]),
                Origin::XmpSidecar,
            )
            .unwrap();
        let panel = info_panel(&read_with(metadata, Vec::new(), Vec::new()));
        let expected_order: Vec<&str> = properties::properties()
            .map(|def| def.xmp_id)
            .filter(|id| ["dc:title", "photoshop:City", "dc:creator"].contains(id))
            .map(|id| match id {
                "dc:title" => "Title",
                "photoshop:City" => "City",
                _ => "Creator",
            })
            .collect();
        let labels: Vec<&str> = panel.values.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, expected_order);
        assert_eq!(labels, ["City", "Creator", "Title"]);
        assert_eq!(
            panel.values,
            [
                ValueRow {
                    label: "City".into(),
                    value: "Umeå".into(),
                    origin: "IIM".into(),
                },
                ValueRow {
                    label: "Creator".into(),
                    value: "Anna".into(),
                    origin: "XMP sidecar".into(),
                },
                ValueRow {
                    label: "Title".into(),
                    value: "Älv".into(),
                    origin: "XMP".into(),
                },
            ]
        );
        assert!(panel.conflicts.is_empty());
        assert!(panel.notices.is_empty());
    }

    #[test]
    fn conflict_rows_hold_values_origins_and_rule() {
        let mut metadata = Metadata::new();
        metadata.add_conflict(Conflict {
            property: "photoshop:City".into(),
            values: vec![
                (Origin::XmpEmbedded, text("Umeå")),
                (Origin::Iim, text("Umea")),
            ],
            rule: ConflictRule::Iim,
        });
        metadata.add_conflict(Conflict {
            property: "dc:description".into(),
            values: vec![
                (Origin::XmpEmbedded, lang_alt(&[("x-default", "A")])),
                (Origin::Exif, text("B")),
            ],
            rule: ConflictRule::Open,
        });
        let panel = info_panel(&read_with(metadata, Vec::new(), Vec::new()));
        assert_eq!(
            panel.conflicts,
            [
                ConflictRow {
                    label: "City".into(),
                    values: vec![
                        ConflictValue {
                            value: "Umeå".into(),
                            origin: "XMP".into(),
                        },
                        ConflictValue {
                            value: "Umea".into(),
                            origin: "IIM".into(),
                        },
                    ],
                    rule: "IIM is used (the checksum does not match)".into(),
                    open: false,
                },
                ConflictRow {
                    label: "Description".into(),
                    values: vec![
                        ConflictValue {
                            value: "A".into(),
                            origin: "XMP".into(),
                        },
                        ConflictValue {
                            value: "B".into(),
                            origin: "Exif".into(),
                        },
                    ],
                    rule: "Unresolved: choose a value in the editor".into(),
                    open: true,
                },
            ]
        );
    }

    #[test]
    fn conflict_row_is_open_only_for_the_open_rule() {
        for (rule, open) in [
            (ConflictRule::Xmp, false),
            (ConflictRule::Iim, false),
            (ConflictRule::Exif, false),
            (ConflictRule::Open, true),
        ] {
            let mut metadata = Metadata::new();
            metadata.add_conflict(Conflict {
                property: "photoshop:City".into(),
                values: vec![
                    (Origin::XmpEmbedded, text("Umeå")),
                    (Origin::Iim, text("Umea")),
                ],
                rule,
            });
            let panel = info_panel(&read_with(metadata, Vec::new(), Vec::new()));
            assert_eq!(panel.conflicts.len(), 1);
            assert_eq!(panel.conflicts[0].open, open, "{rule:?}");
        }
    }

    // --- other_values ---

    /// A read with a title, a city, a creator, contact info and a headline, inserted out of table
    /// order.
    fn read_for_other_values() -> MetadataRead {
        let mut metadata = Metadata::new();
        metadata
            .insert_read(
                "dc:title",
                lang_alt(&[("x-default", "Älv")]),
                Origin::XmpEmbedded,
            )
            .unwrap();
        metadata
            .insert_read("photoshop:City", text("Umeå"), Origin::Iim)
            .unwrap();
        metadata
            .insert_read(
                "Iptc4xmpCore:CreatorContactInfo",
                Value::Struct(StructValue {
                    fields: vec![
                        ("Iptc4xmpCore:CiAdrCity".to_string(), text("Umeå")),
                        (
                            "Iptc4xmpCore:CiEmailWork".to_string(),
                            text("anna@example.com"),
                        ),
                    ],
                }),
                Origin::XmpEmbedded,
            )
            .unwrap();
        metadata
            .insert_read(
                "dc:creator",
                Value::List(vec!["Anna".into()]),
                Origin::XmpSidecar,
            )
            .unwrap();
        metadata
            .insert_read("photoshop:Headline", text("Rubrik"), Origin::Iim)
            .unwrap();
        read_with(metadata, Vec::new(), Vec::new())
    }

    #[test]
    fn other_values_leave_out_the_editor_properties_in_table_order() {
        let read = read_for_other_values();
        let all = info_panel(&read).values;

        let rows = other_values(&read, &["dc:title", "dc:creator"]);

        // The rows of `info_panel` in the same (table) order, without the two properties.
        let expected: Vec<ValueRow> = all
            .iter()
            .filter(|row| row.label != "Title" && row.label != "Creator")
            .cloned()
            .collect();
        assert_eq!(rows, expected);
        assert_eq!(rows.len(), all.len() - 2);
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert!(!labels.contains(&"Title"));
        assert!(!labels.contains(&"Creator"));
        assert!(labels.contains(&"City"));
        assert!(labels.contains(&"Headline"));
        // Nothing left out: the same as the panel's values.
        assert_eq!(other_values(&read, &[]), all);
    }

    #[test]
    fn other_values_keep_the_contact_info_when_it_is_not_left_out() {
        let read = read_for_other_values();
        let contact = property_label("Iptc4xmpCore:CreatorContactInfo", Language::En);

        // The window leaves out the editor's properties but not the contact structure, whose
        // address fields are not editor fields.
        let rows = other_values(&read, &["dc:title", "photoshop:City", "dc:creator"]);
        let kept: Vec<&ValueRow> = rows.iter().filter(|row| row.label == contact).collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].origin, "XMP");
        assert!(kept[0].value.contains("anna@example.com"));

        let rows = other_values(&read, &["Iptc4xmpCore:CreatorContactInfo"]);
        assert!(rows.iter().all(|row| row.label != contact));
    }

    #[test]
    fn every_notice_text_in_order() {
        // Given out of the panel's order, to show that the panel orders them.
        let notices = vec![
            Notice::Unreadable {
                property: "photoshop:DateCreated".into(),
                origin: Origin::Exif,
                text: "2026:13:04 10:15:30".into(),
            },
            Notice::Refused {
                property: "photoshop:City".into(),
                origin: Origin::XmpSidecar,
                error: ValueError::Kind {
                    property: "photoshop:City".into(),
                    expected: "text".into(),
                },
            },
            Notice::Refused {
                property: "plus:Licensor".into(),
                origin: Origin::XmpEmbedded,
                error: ValueError::TooMany {
                    property: "plus:Licensor".into(),
                    max: 3,
                },
            },
            Notice::Refused {
                property: "dc:title".into(),
                origin: Origin::Iim,
                error: ValueError::Empty("dc:title".into()),
            },
            Notice::Refused {
                property: "foo:Bar".into(),
                origin: Origin::Exif,
                error: ValueError::UnknownProperty("foo:Bar".into()),
            },
            Notice::Iim(IimNotice::Converted {
                property: "photoshop:City".into(),
            }),
            Notice::Iim(IimNotice::Repeated {
                property: "photoshop:Headline".into(),
            }),
            Notice::Iim(IimNotice::Unreadable {
                property: "photoshop:Urgency".into(),
                text: "x".into(),
            }),
        ];
        let file_notices = vec![
            FileNotice::ForeignSidecarIgnored(PathBuf::from("bilder").join("IMG_1.xmp")),
            FileNotice::ExtSidecarIgnored(PathBuf::from("bilder").join("DSC01234.ARW.xmp")),
            FileNotice::IimBlockUnreadable("datasetet är för kort".into()),
        ];
        let mut read = read_with(Metadata::new(), notices, file_notices);
        read.reconciled.suggested_date_created = Some(date(None));
        read.state = state(true, IimDigest::Differs);
        assert_eq!(
            info_panel(&read).notices,
            [
                "IMG_1.xmp next to the DNG file is not used (written by another program)",
                "DSC01234.ARW.xmp is not read (RawStride uses DSC01234.xmp)",
                "The IIM block could not be read: datasetet är för kort",
                "City: the IIM text had an unknown character encoding and has been converted",
                "Headline: appeared more than once in IIM; the first value is used",
                "Urgency: could not be read (x)",
                "City from XMP sidecar was not read: wrong form",
                "Licensor from XMP was not read: too many values",
                "Title from IIM was not read: empty value",
                "foo:Bar from Exif was not read: unknown field",
                "Date Created from Exif could not be read (2026:13:04 10:15:30)",
                "Suggested Date Created from the camera: 2026-11-12T10:15:30",
                "IIM has been changed by another program after XMP (the checksum does not match)",
            ]
        );
    }

    #[test]
    fn a_list_with_qualifiers_has_a_notice() {
        let read = read_with(
            Metadata::new(),
            Vec::new(),
            vec![FileNotice::ListWithQualifiers("dc:subject".into())],
        );
        assert_eq!(
            info_panel(&read).notices,
            ["Keywords has qualifiers RawStride cannot keep, so it is shown but never changed"]
        );
    }

    #[test]
    fn iim_checksum_notices() {
        let mut read = empty_read();
        read.state = state(true, IimDigest::Matches);
        assert!(info_panel(&read).notices.is_empty());
        read.state = state(true, IimDigest::Missing);
        assert_eq!(info_panel(&read).notices, ["IIM has no checksum"]);
        read.state = state(true, IimDigest::Differs);
        assert_eq!(
            info_panel(&read).notices,
            ["IIM has been changed by another program after XMP (the checksum does not match)"]
        );
    }

    #[test]
    fn no_checksum_notice_without_iim() {
        let mut read = empty_read();
        read.state = state(false, IimDigest::Missing);
        assert!(info_panel(&read).notices.is_empty());
        read.state = state(false, IimDigest::Differs);
        assert!(info_panel(&read).notices.is_empty());
    }

    // --- panel_text ---

    #[test]
    fn every_panel_text() {
        assert_eq!(panel_text(&PanelText::Selected(3)), "3 images marked");
        assert_eq!(panel_text(&PanelText::Selected(1)), "1 image marked");
        assert_eq!(panel_text(&PanelText::NoneSelected), "No image selected");
        assert_eq!(panel_text(&PanelText::Loading), "Reading metadata …");
        assert_eq!(
            panel_text(&PanelText::ReadError("ExifTool svarade inte".into())),
            "The metadata could not be read: ExifTool svarade inte"
        );
        assert_eq!(panel_text(&PanelText::Values), "Metadata");
        assert_eq!(panel_text(&PanelText::Conflicts), "Conflicts");
        assert_eq!(panel_text(&PanelText::Notices), "Notices");
    }

    // --- English property names (T117, glossary G1 and G2) ---

    #[test]
    fn english_label_of_a_plain_iptc_property_is_its_iptc_name() {
        assert_eq!(property_label("dc:title", Language::En), "Title");
    }

    #[test]
    fn english_label_of_sublocation_leaves_out_legacy() {
        assert_eq!(
            property_label("Iptc4xmpCore:Location", Language::En),
            "Sublocation"
        );
    }

    #[test]
    fn english_label_of_city_leaves_out_legacy() {
        assert_eq!(property_label("photoshop:City", Language::En), "City");
    }

    #[test]
    fn english_label_of_province_or_state_leaves_out_legacy() {
        assert_eq!(
            property_label("photoshop:State", Language::En),
            "Province or State"
        );
    }

    #[test]
    fn english_label_of_country_leaves_out_legacy() {
        assert_eq!(property_label("photoshop:Country", Language::En), "Country");
    }

    #[test]
    fn english_label_of_country_code_leaves_out_legacy() {
        assert_eq!(
            property_label("Iptc4xmpCore:CountryCode", Language::En),
            "Country Code"
        );
    }

    #[test]
    fn english_label_of_the_email_field_is_email() {
        assert_eq!(
            property_label("Iptc4xmpCore:CiEmailWork", Language::En),
            "Email"
        );
    }

    #[test]
    fn english_label_of_the_phone_field_is_phone() {
        assert_eq!(
            property_label("Iptc4xmpCore:CiTelWork", Language::En),
            "Phone"
        );
    }

    #[test]
    fn english_label_of_the_web_field_is_web_url() {
        assert_eq!(
            property_label("Iptc4xmpCore:CiUrlWork", Language::En),
            "Web URL"
        );
    }

    #[test]
    fn english_label_of_xmp_rating_is_rating() {
        assert_eq!(property_label("xmp:Rating", Language::En), "Rating");
    }

    #[test]
    fn english_label_of_xmp_rights_marked_is_copyrighted() {
        assert_eq!(
            property_label("xmpRights:Marked", Language::En),
            "Copyrighted"
        );
    }

    #[test]
    fn english_label_of_xmp_label_is_label() {
        assert_eq!(property_label("xmp:Label", Language::En), "Label");
    }

    #[test]
    fn english_label_of_urgency_is_urgency() {
        assert_eq!(property_label("photoshop:Urgency", Language::En), "Urgency");
    }

    #[test]
    fn english_label_of_category_is_category() {
        assert_eq!(
            property_label("photoshop:Category", Language::En),
            "Category"
        );
    }

    #[test]
    fn english_label_of_supplemental_categories_is_supplemental_categories() {
        assert_eq!(
            property_label("photoshop:SupplementalCategories", Language::En),
            "Supplemental Categories"
        );
    }

    #[test]
    fn the_swedish_labels_of_the_english_exceptions_are_unchanged() {
        for (id, swedish) in [
            ("Iptc4xmpCore:Location", "Plats"),
            ("photoshop:City", "Stad"),
            ("Iptc4xmpCore:CiEmailWork", "E-post"),
            ("xmp:Rating", "Betyg"),
            ("xmpRights:Marked", "Upphovsrättsskyddad"),
            ("photoshop:Urgency", "Prioritet"),
        ] {
            assert_eq!(property_label(id, Language::Sv), swedish, "{id}");
        }
    }

    #[test]
    fn the_tooltip_is_the_full_iptc_name_of_a_legacy_property_or_a_contact_field() {
        for (id, full) in [
            ("Iptc4xmpCore:Location", "Sublocation (legacy)"),
            ("photoshop:City", "City (legacy)"),
            ("photoshop:State", "Province or State (legacy)"),
            ("photoshop:Country", "Country (legacy)"),
            ("Iptc4xmpCore:CountryCode", "Country Code (legacy)"),
            ("Iptc4xmpCore:CiEmailWork", "Email address(es)"),
            ("Iptc4xmpCore:CiTelWork", "Phone number(s)"),
            ("Iptc4xmpCore:CiUrlWork", "Web URL(s)"),
        ] {
            assert_eq!(property_tooltip(id).as_deref(), Some(full), "{id}");
        }
        for id in ["dc:title", "xmp:Rating", "xmpRights:Marked", "foo:Bar"] {
            assert_eq!(property_tooltip(id), None, "{id}");
        }
    }

    /// Glossary G1 leaves " (legacy)" out of every English label, also of the two legacy
    /// properties T117's task text did not name (reviewer, 2026-11-14); the full name is the
    /// tooltip.
    #[test]
    fn no_english_label_says_legacy() {
        for def in properties::properties() {
            let label = property_label(def.xmp_id, Language::En);
            assert!(!label.contains("(legacy)"), "{}: {label}", def.xmp_id);
        }
        for (id, label, full) in [
            (
                "Iptc4xmpCore:IntellectualGenre",
                "Intellectual Genre",
                "Intellectual Genre (legacy)",
            ),
            (
                "Iptc4xmpCore:SubjectCode",
                "Subject Code",
                "Subject Code (legacy)",
            ),
        ] {
            assert_eq!(property_label(id, Language::En), label, "{id}");
            assert_eq!(property_tooltip(id).as_deref(), Some(full), "{id}");
        }
    }

    #[test]
    fn every_english_label_is_a_known_property_or_field() {
        for (id, _) in EN_LABELS {
            assert!(
                properties::property(id).is_some() || field_name(id).is_some(),
                "{id} is not in the property table"
            );
        }
    }

    // --- no Swedish letters in the workspace's non-test string literals (T117, T118, T121) ---

    /// The letters that only a Swedish text has.
    const SWEDISH_LETTERS: [char; 6] = ['å', 'ä', 'ö', 'Å', 'Ä', 'Ö'];

    /// The one file whose table `SV_LABELS` may hold Swedish letters, as the check names it.
    const SV_LABELS_FILE: &str = "pw-loupe/src/info_panel.rs";

    /// `source` with every top-level `#[cfg(test)]` item blanked (its lines kept, empty, so the
    /// line numbers stay), and the files those items declare (`mod name;` or `#[path = "…"]`).
    fn non_test_code(source: &str) -> (String, Vec<String>) {
        let source = source.replace("\r\n", "\n");
        let lines: Vec<&str> = source.lines().collect();
        let mut kept = Vec::with_capacity(lines.len());
        let mut declared = Vec::new();
        let mut at = 0;
        while at < lines.len() {
            if lines[at] != "#[cfg(test)]" {
                kept.push(lines[at]);
                at += 1;
                continue;
            }
            let start = at;
            at += 1;
            let mut path = None;
            while at < lines.len() && lines[at].starts_with("#[") {
                if let Some(file) = lines[at]
                    .strip_prefix("#[path = \"")
                    .and_then(|rest| rest.strip_suffix("\"]"))
                {
                    path = Some(file.rsplit('/').next().unwrap_or(file).to_string());
                }
                at += 1;
            }
            let item = lines.get(at).copied().unwrap_or("");
            if let Some(name) = item.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
                declared.push(path.unwrap_or_else(|| format!("{name}.rs")));
            } else if item.ends_with('{') {
                while at < lines.len() && lines[at] != "}" {
                    at += 1;
                }
            }
            at += 1;
            kept.extend(std::iter::repeat_n("", at.min(lines.len()) - start));
        }
        (kept.join("\n"), declared)
    }

    /// The places in `code` where a string literal holds a Swedish letter, as "file:line";
    /// comments, character literals and lifetimes are skipped, and so are the lines of
    /// `SV_LABELS` when `file` is [`SV_LABELS_FILE`].
    fn swedish_letters_in_literals(file: &str, code: &str) -> Vec<String> {
        // The lines of the table `SV_LABELS`, from its first line to its closing `];`.
        let mut skipped = std::ops::Range::default();
        if file == SV_LABELS_FILE {
            let lines: Vec<&str> = code.lines().collect();
            if let Some(start) = lines
                .iter()
                .position(|line| line.starts_with("const SV_LABELS"))
                && let Some(length) = lines[start..].iter().position(|line| *line == "];")
            {
                skipped = start + 1..start + length + 2;
            }
        }
        let chars: Vec<char> = code.chars().collect();
        let mut found = Vec::new();
        let mut line = 1;
        let mut at = 0;
        // Moves past `chars[at]`, counting lines.
        let step = |at: &mut usize, line: &mut usize| {
            if chars[*at] == '\n' {
                *line += 1;
            }
            *at += 1;
        };
        while at < chars.len() {
            match chars[at] {
                '/' if chars.get(at + 1) == Some(&'/') => {
                    while at < chars.len() && chars[at] != '\n' {
                        step(&mut at, &mut line);
                    }
                }
                '/' if chars.get(at + 1) == Some(&'*') => {
                    while at < chars.len() && !(chars[at] == '*' && chars.get(at + 1) == Some(&'/'))
                    {
                        step(&mut at, &mut line);
                    }
                    at += 2;
                }
                '\'' if chars.get(at + 1) == Some(&'\\') => {
                    // An escaped character literal: up to its closing quote.
                    at += 2;
                    while at < chars.len() && chars[at] != '\'' {
                        at += 1;
                    }
                    at += 1;
                }
                '\'' if chars.get(at + 2) == Some(&'\'') => at += 3,
                // A raw string `r"…"` or `r#"…"#`: no escapes inside.
                'r' if matches!(chars.get(at + 1), Some('"') | Some('#'))
                    && !chars
                        .get(at.wrapping_sub(1))
                        .is_some_and(|c| c.is_alphanumeric() || *c == '_') =>
                {
                    let mut hashes = 0;
                    at += 1;
                    while chars.get(at) == Some(&'#') {
                        hashes += 1;
                        at += 1;
                    }
                    if chars.get(at) != Some(&'"') {
                        continue;
                    }
                    at += 1;
                    loop {
                        if at >= chars.len() {
                            break;
                        }
                        if chars[at] == '"' && (1..=hashes).all(|n| chars.get(at + n) == Some(&'#'))
                        {
                            at += 1 + hashes;
                            break;
                        }
                        if SWEDISH_LETTERS.contains(&chars[at]) && !skipped.contains(&line) {
                            found.push(format!("{file}:{line}"));
                        }
                        step(&mut at, &mut line);
                    }
                }
                '"' => {
                    at += 1;
                    while at < chars.len() && chars[at] != '"' {
                        if chars[at] == '\\' {
                            step(&mut at, &mut line);
                        } else if SWEDISH_LETTERS.contains(&chars[at]) && !skipped.contains(&line) {
                            found.push(format!("{file}:{line}"));
                        }
                        if at < chars.len() {
                            step(&mut at, &mut line);
                        }
                    }
                    at += 1;
                }
                _ => step(&mut at, &mut line),
            }
        }
        found.dedup();
        found
    }

    /// Adds the `.rs` files under `folder`, with their sources, to `out`, sorted by path.
    fn rust_sources(folder: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(folder)
            .unwrap_or_else(|error| panic!("read {}: {error}", folder.display()))
            .map(|entry| entry.unwrap_or_else(|error| panic!("{error}")).path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                rust_sources(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                out.push((path, source));
            }
        }
    }

    #[test]
    fn the_check_finds_a_swedish_letter_in_a_non_test_string_literal() {
        let source = "// \"Stäng\" in a comment\n\
                      #[cfg(test)]\n\
                      mod test_support;\n\
                      fn a() -> &'static str {\n\
                      let c = 'ä';\n\
                      \"Close\"\n\
                      }\n\
                      fn b() -> String { format!(\"Öppna {x}\") }\n\
                      #[cfg(test)]\n\
                      #[path = \"fixture_file.rs\"]\n\
                      mod fixture;\n\
                      const R: &str = r#\"Mål\"#;\n\
                      #[cfg(test)]\n\
                      mod tests {\n    \"Avbryt\";\n}\n\
                      const S: &str = \"Källa\";\n";
        let (code, declared) = non_test_code(source);
        assert_eq!(declared, ["test_support.rs", "fixture_file.rs"]);
        assert_eq!(
            swedish_letters_in_literals("x.rs", &code),
            ["x.rs:8", "x.rs:12", "x.rs:17"]
        );
        let table = "const SV_LABELS: &[(&str, &str)] = &[\n    (\"a\", \"Källa\"),\n];\n\
                     const B: &str = \"Källa\";\n";
        assert_eq!(
            swedish_letters_in_literals(SV_LABELS_FILE, table),
            [format!("{SV_LABELS_FILE}:4")]
        );
        assert_eq!(
            swedish_letters_in_literals("pw-other/src/info_panel.rs", table),
            [
                "pw-other/src/info_panel.rs:2",
                "pw-other/src/info_panel.rs:4"
            ]
        );
    }

    #[test]
    fn the_workspace_has_no_swedish_letters_in_its_non_test_string_literals() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| panic!("no folder above {}", env!("CARGO_MANIFEST_DIR")));
        let mut folders: Vec<std::path::PathBuf> = std::fs::read_dir(crates)
            .unwrap_or_else(|error| panic!("read {}: {error}", crates.display()))
            .map(|entry| entry.unwrap_or_else(|error| panic!("{error}")).path())
            .filter(|path| path.join("src").is_dir())
            .collect();
        folders.sort();
        assert!(folders.len() > 1, "no crates found in {}", crates.display());
        let mut found = Vec::new();
        for folder in folders {
            let mut files = Vec::new();
            rust_sources(&folder.join("src"), &mut files);
            // The files a crate's `#[cfg(test)]` items declare are test code, matched by name
            // within that crate.
            let mut test_only = vec!["tests.rs".to_string()];
            let code: Vec<(String, String)> = files
                .iter()
                .map(|(path, source)| {
                    let (code, declared) = non_test_code(source);
                    test_only.extend(declared);
                    let name = path.strip_prefix(crates).unwrap_or(path);
                    (name.display().to_string().replace('\\', "/"), code)
                })
                .collect();
            found.extend(
                code.iter()
                    .filter(|(name, _)| {
                        let file = name.rsplit('/').next().unwrap_or(name);
                        !test_only.iter().any(|test| test == file)
                    })
                    .flat_map(|(name, code)| swedish_letters_in_literals(name, code)),
            );
        }
        assert!(
            found.is_empty(),
            "string literals with a Swedish letter: {found:?}"
        );
    }
}
