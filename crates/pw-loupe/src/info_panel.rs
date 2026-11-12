//! The texts of the info panel (ADR 0007 I10i): the values, the conflicts and the notices of one
//! image's [`MetadataRead`], and the panel's fixed texts. Pure functions, no egui; T103 draws them.
//!
//! - **Values** (§5.1): every property with a value, in the order of the property table, with its
//!   shown text and where it came from.
//! - **Conflicts** (§5.1, "conflicts always visible"): every value with its origin and the rule
//!   that was applied, until the next write by RawStride.
//! - **Notices**: ignored sidecars (Q5, Q8), an unreadable IIM block, converted IIM text (§5.4),
//!   refused and unreadable values, values hidden by the user (§6.4), the suggested Date Created
//!   (§7) and the IIM checksum (§5.1).
//!
//! Text rule (owner, 2026-11-12): every text the user sees in the info panel is made here. Each
//! text is a whole sentence built from one template with its values filled in, so another language
//! can change the word order. Property names come only from [`property_label`], whose Swedish names
//! (owner decision D11) are in the one table [`SV_LABELS`].

use std::path::Path;

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
    /// English: IPTC's name from the property table.
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
    ("photoshop:AuthorsPosition", "Fotografens titel"),
    (
        "Iptc4xmpCore:CreatorContactInfo",
        "Fotografens kontaktuppgifter",
    ),
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

/// The name of a property or structure field (owner, 2026-11-12):
/// - `Sv`: the Swedish name of D11 for the properties and fields in its list, otherwise as `En`;
/// - `En`: IPTC's name from the property table (`name` of the property, or of the field in its
///   structure), otherwise the id.
pub fn property_label(xmp_id: &str, language: Language) -> String {
    if language == Language::Sv
        && let Some((_, label)) = SV_LABELS.iter().find(|(id, _)| *id == xmp_id)
    {
        return (*label).to_string();
    }
    if let Some(def) = properties::property(xmp_id) {
        return def.name.to_string();
    }
    match field_name(xmp_id) {
        Some(name) => name.to_string(),
        None => xmp_id.to_string(),
    }
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
/// ` (<språk>)` when it is not `x-default` and ` +N språk` when it has N other entries; a list its
/// entries joined with `; `; a date in XMP form; a structure `fält: värde` joined with `, `; a list
/// of structures one structure per line.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::Text(text) | Value::Number(text) => text.clone(),
        Value::LangAlt(lang_alt) => lang_alt_text(lang_alt),
        Value::List(entries) => entries.join("; "),
        Value::Date(date) => date.to_xmp(),
        Value::Struct(fields) => struct_text(fields),
        Value::StructList(list) => list.iter().map(struct_text).collect::<Vec<_>>().join("\n"),
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
        (None, n) => format!("{text} +{n} språk"),
        (Some(language), 0) => format!("{text} ({language})"),
        (Some(language), n) => format!("{text} ({language}) +{n} språk"),
    }
}

/// A structure as `fält: värde` joined with `, `.
fn struct_text(value: &StructValue) -> String {
    value
        .fields
        .iter()
        .map(|(id, field)| {
            format!(
                "{label}: {text}",
                label = property_label(id, Language::Sv),
                text = value_text(field)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Where a value came from.
pub fn origin_text(origin: Origin) -> &'static str {
    match origin {
        Origin::XmpEmbedded => "XMP",
        Origin::XmpSidecar => "XMP-sidofil",
        Origin::Iim => "IIM",
        Origin::Exif => "Exif",
        Origin::Edited => "Ändrad",
    }
}

/// The rule that decided a conflict (§5.1).
pub fn rule_text(rule: ConflictRule) -> &'static str {
    match rule {
        ConflictRule::Xmp => "XMP används",
        ConflictRule::Iim => "IIM används (kontrollsumman stämmer inte)",
        ConflictRule::Exif => "Exif används",
        ConflictRule::Open => "Olöst: välj värde i redigeraren",
    }
}

/// Why a value was refused, in Swedish.
fn refused_reason(error: &ValueError) -> &'static str {
    match error {
        ValueError::Kind { .. } => "fel form",
        ValueError::TooMany { .. } => "för många värden",
        ValueError::Empty(_) => "tomt värde",
        ValueError::UnknownProperty(_) => "okänt fält",
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
}

/// The texts of the info panel for one image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoPanel {
    pub values: Vec<ValueRow>,
    pub conflicts: Vec<ConflictRow>,
    pub notices: Vec<String>,
}

/// The info panel of one image: its values in table order, its conflicts and its notices.
pub fn info_panel(read: &MetadataRead) -> InfoPanel {
    let metadata = &read.reconciled.metadata;
    let values = properties::properties()
        .filter_map(|def| {
            metadata.get(def.xmp_id).map(|found| ValueRow {
                label: property_label(def.xmp_id, Language::Sv),
                value: value_text(&found.value),
                origin: origin_text(found.origin).to_string(),
            })
        })
        .collect();
    let conflicts = metadata
        .conflicts()
        .iter()
        .map(|conflict| ConflictRow {
            label: property_label(&conflict.property, Language::Sv),
            values: conflict
                .values
                .iter()
                .map(|(origin, value)| ConflictValue {
                    value: value_text(value),
                    origin: origin_text(*origin).to_string(),
                })
                .collect(),
            rule: rule_text(conflict.rule).to_string(),
        })
        .collect();
    InfoPanel {
        values,
        conflicts,
        notices: notices(read),
    }
}

/// The notices, in this order: the file notices, the IIM notices, the refused values, the values
/// hidden by the user, the unreadable values, the suggested Date Created, the IIM checksum.
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
            "{field} från {origin} lästes inte: {reason}",
            field = property_label(property, Language::Sv),
            origin = origin_text(*origin),
            reason = refused_reason(error)
        )),
        _ => None,
    }));
    out.extend(reconciled.notices.iter().filter_map(|notice| match notice {
        Notice::HiddenByUser {
            property, value, ..
        } => Some(format!(
            "{field}: kamerans värde {value} är dolt av användaren",
            field = property_label(property, Language::Sv),
            value = value_text(value)
        )),
        _ => None,
    }));
    out.extend(reconciled.notices.iter().filter_map(|notice| match notice {
        Notice::Unreadable {
            property,
            origin,
            text,
        } => Some(format!(
            "{field} från {origin} kunde inte läsas ({text})",
            field = property_label(property, Language::Sv),
            origin = origin_text(*origin)
        )),
        _ => None,
    }));
    if let Some(date) = &reconciled.suggested_date_created {
        out.push(format!(
            "Förslag till Skapad från kameran: {date}",
            date = date.to_xmp()
        ));
    }
    if read.state.iim_present {
        match read.state.digest {
            IimDigest::Matches => {}
            IimDigest::Differs => out.push(
                "IIM har ändrats av ett annat program efter XMP (kontrollsumman stämmer inte)"
                    .to_string(),
            ),
            IimDigest::Missing => out.push("IIM saknar kontrollsumma".to_string()),
        }
    }
    out
}

/// The text of a notice about the files next to the image or its IIM block.
fn file_notice_text(notice: &FileNotice) -> String {
    match notice {
        FileNotice::ForeignSidecarIgnored(path) => format!(
            "{file} bredvid DNG-filen används inte (skriven av ett annat program)",
            file = file_name(path)
        ),
        FileNotice::ExtSidecarIgnored(path) => format!(
            "{file} läses inte (RawStride använder {base}.xmp)",
            file = file_name(path),
            base = base_name(path)
        ),
        FileNotice::IimBlockUnreadable(reason) => {
            format!("IIM-blocket kunde inte läsas: {reason}")
        }
    }
}

/// The text of a notice about an IIM value.
fn iim_notice_text(notice: &IimNotice) -> String {
    match notice {
        IimNotice::Converted { property } => format!(
            "{field}: IIM-texten hade okänd teckenkodning och har konverterats",
            field = property_label(property, Language::Sv)
        ),
        IimNotice::Repeated { property } => format!(
            "{field}: fanns flera gånger i IIM, det första värdet används",
            field = property_label(property, Language::Sv)
        ),
        IimNotice::Unreadable { property, text } => format!(
            "{field}: kunde inte läsas ({text})",
            field = property_label(property, Language::Sv)
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
        PanelText::Selected(1) => "1 bild markerad".to_string(),
        PanelText::Selected(n) => format!("{n} bilder markerade"),
        PanelText::NoneSelected => "Ingen bild vald".to_string(),
        PanelText::Loading => "Läser metadata …".to_string(),
        PanelText::ReadError(error) => format!("Metadata kunde inte läsas: {error}"),
        PanelText::Values => "Metadata".to_string(),
        PanelText::Conflicts => "Konflikter".to_string(),
        PanelText::Notices => "Meddelanden".to_string(),
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
            "Email address(es)"
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
            "Hello (en-US) +1 språk"
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
            "Hej +2 språk"
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
            "E-post: foto@example.com, Telefon: +46 8 123, City: Umeå"
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

    // --- origin and rule texts ---

    #[test]
    fn every_origin_text() {
        assert_eq!(origin_text(Origin::XmpEmbedded), "XMP");
        assert_eq!(origin_text(Origin::XmpSidecar), "XMP-sidofil");
        assert_eq!(origin_text(Origin::Iim), "IIM");
        assert_eq!(origin_text(Origin::Exif), "Exif");
        assert_eq!(origin_text(Origin::Edited), "Ändrad");
    }

    #[test]
    fn every_rule_text() {
        assert_eq!(rule_text(ConflictRule::Xmp), "XMP används");
        assert_eq!(
            rule_text(ConflictRule::Iim),
            "IIM används (kontrollsumman stämmer inte)"
        );
        assert_eq!(rule_text(ConflictRule::Exif), "Exif används");
        assert_eq!(
            rule_text(ConflictRule::Open),
            "Olöst: välj värde i redigeraren"
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
                "dc:title" => "Titel",
                "photoshop:City" => "Stad",
                _ => "Fotograf",
            })
            .collect();
        let labels: Vec<&str> = panel.values.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, expected_order);
        assert_eq!(labels, ["Stad", "Fotograf", "Titel"]);
        assert_eq!(
            panel.values,
            [
                ValueRow {
                    label: "Stad".into(),
                    value: "Umeå".into(),
                    origin: "IIM".into(),
                },
                ValueRow {
                    label: "Fotograf".into(),
                    value: "Anna".into(),
                    origin: "XMP-sidofil".into(),
                },
                ValueRow {
                    label: "Titel".into(),
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
                    label: "Stad".into(),
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
                    rule: "IIM används (kontrollsumman stämmer inte)".into(),
                },
                ConflictRow {
                    label: "Beskrivning".into(),
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
                    rule: "Olöst: välj värde i redigeraren".into(),
                },
            ]
        );
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
            Notice::HiddenByUser {
                property: "dc:creator".into(),
                value: Value::List(vec!["Kamera Ägare".into()]),
                origin: Origin::XmpEmbedded,
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
                "IMG_1.xmp bredvid DNG-filen används inte (skriven av ett annat program)",
                "DSC01234.ARW.xmp läses inte (RawStride använder DSC01234.xmp)",
                "IIM-blocket kunde inte läsas: datasetet är för kort",
                "Stad: IIM-texten hade okänd teckenkodning och har konverterats",
                "Rubrik: fanns flera gånger i IIM, det första värdet används",
                "Prioritet: kunde inte läsas (x)",
                "Stad från XMP-sidofil lästes inte: fel form",
                "Licensgivare från XMP lästes inte: för många värden",
                "Titel från IIM lästes inte: tomt värde",
                "foo:Bar från Exif lästes inte: okänt fält",
                "Fotograf: kamerans värde Kamera Ägare är dolt av användaren",
                "Skapad från Exif kunde inte läsas (2026:13:04 10:15:30)",
                "Förslag till Skapad från kameran: 2026-11-12T10:15:30",
                "IIM har ändrats av ett annat program efter XMP (kontrollsumman stämmer inte)",
            ]
        );
    }

    #[test]
    fn iim_checksum_notices() {
        let mut read = empty_read();
        read.state = state(true, IimDigest::Matches);
        assert!(info_panel(&read).notices.is_empty());
        read.state = state(true, IimDigest::Missing);
        assert_eq!(info_panel(&read).notices, ["IIM saknar kontrollsumma"]);
        read.state = state(true, IimDigest::Differs);
        assert_eq!(
            info_panel(&read).notices,
            ["IIM har ändrats av ett annat program efter XMP (kontrollsumman stämmer inte)"]
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
        assert_eq!(panel_text(&PanelText::Selected(3)), "3 bilder markerade");
        assert_eq!(panel_text(&PanelText::Selected(1)), "1 bild markerad");
        assert_eq!(panel_text(&PanelText::NoneSelected), "Ingen bild vald");
        assert_eq!(panel_text(&PanelText::Loading), "Läser metadata …");
        assert_eq!(
            panel_text(&PanelText::ReadError("ExifTool svarade inte".into())),
            "Metadata kunde inte läsas: ExifTool svarade inte"
        );
        assert_eq!(panel_text(&PanelText::Values), "Metadata");
        assert_eq!(panel_text(&PanelText::Conflicts), "Konflikter");
        assert_eq!(panel_text(&PanelText::Notices), "Meddelanden");
    }
}
