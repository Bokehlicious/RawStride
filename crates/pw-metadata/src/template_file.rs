//! The metadata template file's model (ADR 0017 U13-Q1, Q4, Q5, Q12; ADR 0007 §10).
//!
//! A metadata template is a set of **checked fields**, each with its value ([`TemplateFile`]). A
//! list of plain text values can also have its **+ box** checked: its entries are then added after
//! the image's instead of replacing the list (U13-Q12). [`TemplateFile::new`] is the one way the
//! reader of a template file and the template window make a template, so both follow the same
//! rules; [`TemplateFile::to_template`] turns it into the [`Template`] that
//! [`crate::ExifTool::apply_template_safely`] applies through the write plan and the safe write.
//!
//! A template holds only properties the metadata window edits ([`in_template`], U13-Q3), so a file
//! from another program cannot bring in a field the template window cannot show.

use crate::properties::{self, Occurrence, ValueKind};
use crate::template::{
    LOCATION_CREATED, Mode, Template, TemplateField, has_content, sort_fields, x_default,
};
use crate::value::{self, Value};

/// Creator's Contact Info, the single structure whose checked fields are applied one by one.
const CONTACT_INFO: &str = "Iptc4xmpCore:CreatorContactInfo";

/// The language of the default entry of a Lang Alt value.
const X_DEFAULT: &str = "x-default";

/// The properties a template can hold (U13-Q3): those the metadata window edits, the eight
/// contact fields as the one structure Creator's Contact Info, without Rating and Label (U13-Q10)
/// and the deprecated IIM properties (never edited, ADR 0007 §5.5). The fields in the window's
/// order, then its lists of structures in form order. The window (`pw-loupe`) must agree with
/// this list (a test there, U13c).
const IN_TEMPLATE: [&str; 50] = [
    "dc:title",
    "photoshop:Headline",
    "dc:description",
    "Iptc4xmpCore:AltTextAccessibility",
    "Iptc4xmpCore:ExtDescrAccessibility",
    "photoshop:CaptionWriter",
    "dc:subject",
    "Iptc4xmpExt:PersonInImage",
    "Iptc4xmpExt:OrganisationInImageName",
    "Iptc4xmpExt:OrganisationInImageCode",
    "Iptc4xmpExt:Event",
    "dc:creator",
    "photoshop:AuthorsPosition",
    CONTACT_INFO,
    "dc:rights",
    "photoshop:Credit",
    "photoshop:Source",
    "xmpRights:UsageTerms",
    "xmpRights:WebStatement",
    "xmpRights:Marked",
    "photoshop:DateCreated",
    "Iptc4xmpCore:Location",
    "photoshop:City",
    "photoshop:State",
    "photoshop:Country",
    "Iptc4xmpCore:CountryCode",
    "photoshop:Instructions",
    "photoshop:TransmissionReference",
    "Iptc4xmpCore:SubjectCode",
    "Iptc4xmpCore:Scene",
    "plus:ModelReleaseStatus",
    "plus:ModelReleaseID",
    "plus:MinorModelAgeDisclosure",
    "plus:PropertyReleaseStatus",
    "plus:PropertyReleaseID",
    "plus:DataMining",
    "Iptc4xmpExt:DigitalSourceType",
    "Iptc4xmpExt:PersonInImageWDetails",
    "Iptc4xmpExt:ProductInImage",
    "Iptc4xmpExt:ArtworkOrObject",
    "Iptc4xmpExt:Contributor",
    LOCATION_CREATED,
    "Iptc4xmpExt:LocationShown",
    "Iptc4xmpExt:Genre",
    "Iptc4xmpExt:AboutCvTerm",
    "Iptc4xmpExt:RegistryId",
    "plus:ImageCreator",
    "plus:CopyrightOwner",
    "plus:Licensor",
    "plus:ImageSupplier",
];

/// A metadata template (U13-Q1): the checked fields with their values, and the fields whose + box
/// is checked. Made by [`TemplateFile::new`].
///
/// The shape of each kind of field in `fields`:
/// - a simple field (text, date, number, list of text): its value;
/// - a Lang Alt: a `Value::LangAlt` with only `x-default`;
/// - Location Created: a `Value::StructList` of **one** entry holding only the checked fields of
///   the structure (U13-Q4);
/// - Creator's Contact Info: a `Value::Struct` holding only the checked fields;
/// - any other list of structures (Location Shown, Image Creator, Copyright Owner, Licensor, …): a
///   `Value::StructList`, whole (U13-Q4: one check box for the list).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateFile {
    /// The checked fields with their values, keyed by the property's XMP id, in the order of the
    /// property table.
    pub fields: Vec<(String, Value)>,
    /// The XMP ids of the fields whose + box is checked ([`can_add`]), in the same order.
    pub add: Vec<String>,
}

/// True for a property the metadata window edits, which a template can hold (U13-Q3; see
/// `IN_TEMPLATE`).
pub fn in_template(xmp_id: &str) -> bool {
    IN_TEMPLATE.contains(&xmp_id)
}

/// True for a property whose + box can be checked (U13-Q12): one a template holds
/// ([`in_template`]) that is a list of plain text values (a list of text or URIs).
pub fn can_add(xmp_id: &str) -> bool {
    in_template(xmp_id)
        && properties::property(xmp_id).is_some_and(|def| {
            def.occurrence == Occurrence::Multi
                && matches!(def.kind, ValueKind::Text | ValueKind::Uri)
        })
}

impl TemplateFile {
    /// The template of `fields` and `add`, by the rules shared by the reader and the window:
    ///
    /// - a field that is not [`in_template`], or whose value does not fit its property
    ///   (`value::check_kind`, structure fields at any depth), is dropped; a field named twice
    ///   keeps the first;
    /// - every Lang Alt, at the top and inside a structure, is cut to its `x-default` (matched
    ///   without regard to case); one without it is dropped;
    /// - every list, at the top and inside a structure, loses its blank entries and its exact
    ///   duplicates (the first kept);
    /// - a value without content is dropped; inside every structure, at any depth, a field without
    ///   content is dropped and the fields are put in the structure's order, and an entry or
    ///   structure left with no field is dropped;
    /// - a list of structures with more entries than its limit (`properties::max_entries`: Image
    ///   Creator, Copyright Owner and Licensor three, Image Supplier one; ADR 0007 §8), counted
    ///   after the entries without content are dropped, is dropped, so applying the template never
    ///   fails on it (U13-Q5);
    /// - Location Created keeps only its first entry;
    /// - `fields` are put in the order of the property table;
    /// - `add` keeps the ids that are [`can_add`] and among the kept fields, each once, in the
    ///   same order.
    ///
    /// Running it again on its result changes nothing.
    pub fn new(fields: Vec<(String, Value)>, add: Vec<String>) -> TemplateFile {
        let mut seen: Vec<String> = Vec::new();
        let mut kept: Vec<(String, Value)> = Vec::new();
        for (xmp_id, value) in fields {
            if seen.contains(&xmp_id) {
                continue;
            }
            seen.push(xmp_id.clone());
            if !in_template(&xmp_id) {
                continue;
            }
            let Some(def) = properties::property(&xmp_id) else {
                continue;
            };
            if value::check_kind(&xmp_id, def.kind, def.occurrence, &value).is_err() {
                continue;
            }
            let Some(mut value) = clean(value, def.kind) else {
                continue;
            };
            if properties::max_entries(&xmp_id)
                .is_some_and(|max| matches!(&value, Value::StructList(list) if list.len() > max))
            {
                continue;
            }
            if xmp_id == LOCATION_CREATED
                && let Value::StructList(list) = &mut value
            {
                list.truncate(1);
            }
            kept.push((xmp_id, value));
        }
        kept.sort_by_key(|(xmp_id, _)| table_position(xmp_id));

        let mut checked: Vec<String> = Vec::new();
        for xmp_id in add {
            if can_add(&xmp_id)
                && kept.iter().any(|(kept_id, _)| *kept_id == xmp_id)
                && !checked.contains(&xmp_id)
            {
                checked.push(xmp_id);
            }
        }
        checked.sort_by_key(|xmp_id| table_position(xmp_id));
        TemplateFile {
            fields: kept,
            add: checked,
        }
    }

    /// The [`Template`] that applies this template: one field per checked field, in order, except
    /// Location Created and Creator's Contact Info, which give one Replace field per field of the
    /// structure (set in Location Created's first entry). A field in `add` is Append with its
    /// list; every other field Replace with its whole value. For a value made by
    /// [`TemplateFile::new`]; it is not checked again.
    pub fn to_template(&self) -> Template {
        let mut fields = Vec::new();
        for (xmp_id, value) in &self.fields {
            let sub_fields = match (xmp_id.as_str(), value) {
                (LOCATION_CREATED, Value::StructList(list)) => {
                    list.first().map(|entry| entry.fields.as_slice())
                }
                (CONTACT_INFO, Value::Struct(structure)) => Some(structure.fields.as_slice()),
                _ => None,
            };
            if let Some(sub_fields) = sub_fields {
                for (sub, sub_value) in sub_fields {
                    fields.push(TemplateField {
                        property: xmp_id.clone(),
                        field: Some(sub.clone()),
                        mode: Mode::Replace,
                        value: Some(sub_value.clone()),
                    });
                }
                continue;
            }
            let mode = if self.add.contains(xmp_id) {
                Mode::Append
            } else {
                Mode::Replace
            };
            fields.push(TemplateField {
                property: xmp_id.clone(),
                field: None,
                mode,
                value: Some(value.clone()),
            });
        }
        Template { fields }
    }
}

/// The place of a property in the property table (an unknown one last).
fn table_position(xmp_id: &str) -> usize {
    properties::properties()
        .position(|def| def.xmp_id == xmp_id)
        .unwrap_or(usize::MAX)
}

/// `value` (of a property or structure field of `kind`, already checked by `check_kind`) with the
/// rules of [`TemplateFile::new`] applied; `None` when nothing with content is left.
fn clean(value: Value, kind: ValueKind) -> Option<Value> {
    let value = match value {
        Value::LangAlt(lang_alt) => {
            let (_, text) = lang_alt
                .entries()
                .iter()
                .find(|(language, _)| language.eq_ignore_ascii_case(X_DEFAULT))?;
            x_default(text)
        }
        Value::List(entries) => {
            let mut kept: Vec<String> = Vec::new();
            for entry in entries {
                if !entry.trim().is_empty() && !kept.contains(&entry) {
                    kept.push(entry);
                }
            }
            Value::List(kept)
        }
        Value::Struct(fields) => Value::Struct(clean_struct(fields, kind)?),
        Value::StructList(list) => Value::StructList(
            list.into_iter()
                .filter_map(|fields| clean_struct(fields, kind))
                .collect(),
        ),
        other => other,
    };
    has_content(&value).then_some(value)
}

/// A structure value of `kind` (`ValueKind::Struct`) with each field cleaned ([`clean`]), a field
/// named twice keeping the first, the fields without content dropped and the rest in the
/// structure's order; `None` when no field is left.
fn clean_struct(fields: value::StructValue, kind: ValueKind) -> Option<value::StructValue> {
    let ValueKind::Struct(name) = kind else {
        return None;
    };
    let struct_def = properties::struct_def(name)?;
    let mut kept = value::StructValue::default();
    for (field_id, field_value) in fields.fields {
        if kept.fields.iter().any(|(seen, _)| *seen == field_id) {
            continue;
        }
        let Some(field_def) = struct_def
            .fields
            .iter()
            .find(|candidate| candidate.xmp_id == field_id)
        else {
            continue;
        };
        if let Some(field_value) = clean(field_value, field_def.kind) {
            kept.fields.push((field_id, field_value));
        }
    }
    if kept.fields.is_empty() {
        return None;
    }
    sort_fields(&mut kept, struct_def);
    Some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::IptcDate;
    use crate::template;
    use crate::value::{LangAlt, Metadata, Origin, StructValue};

    const LOCATION_SHOWN: &str = "Iptc4xmpExt:LocationShown";
    const CITY: &str = "Iptc4xmpExt:City";
    const SUBLOCATION: &str = "Iptc4xmpExt:Sublocation";
    const LOCATION_NAME: &str = "Iptc4xmpExt:LocationName";
    const EMAIL: &str = "Iptc4xmpCore:CiEmailWork";
    const PHONE: &str = "Iptc4xmpCore:CiTelWork";

    /// The nine lists of plain text values (U13-Q12).
    const ADDABLE: [&str; 9] = [
        "dc:subject",
        "dc:creator",
        "Iptc4xmpExt:PersonInImage",
        "Iptc4xmpCore:SubjectCode",
        "Iptc4xmpCore:Scene",
        "Iptc4xmpExt:OrganisationInImageName",
        "Iptc4xmpExt:OrganisationInImageCode",
        "plus:ModelReleaseID",
        "plus:PropertyReleaseID",
    ];

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

    fn date(text: &str) -> Value {
        match IptcDate::parse_xmp(text) {
            Ok(date) => Value::Date(date),
            Err(error) => panic!("{text}: {error:?}"),
        }
    }

    fn entry(fields: &[(&str, Value)]) -> StructValue {
        StructValue {
            fields: fields
                .iter()
                .map(|(id, value)| (id.to_string(), value.clone()))
                .collect(),
        }
    }

    fn fields(fields: &[(&str, Value)]) -> Vec<(String, Value)> {
        fields
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect()
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    /// `new` with these fields and no + box.
    fn made(given: &[(&str, Value)]) -> TemplateFile {
        TemplateFile::new(fields(given), Vec::new())
    }

    /// The kept value of `xmp_id` after `new` of that one field.
    fn kept(xmp_id: &str, value: Value) -> Option<Value> {
        made(&[(xmp_id, value)])
            .fields
            .into_iter()
            .find(|(id, _)| id == xmp_id)
            .map(|(_, value)| value)
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

    fn applied(template_file: &TemplateFile, metadata: &mut Metadata) {
        if let Err(error) = template::apply(&template_file.to_template(), metadata) {
            panic!("{error}");
        }
    }

    fn value_of<'a>(metadata: &'a Metadata, xmp_id: &str) -> Option<&'a Value> {
        metadata.get(xmp_id).map(|stored| &stored.value)
    }

    // --- can_add and in_template ---

    #[test]
    fn can_add_is_true_only_for_the_lists_of_plain_text_values() {
        for id in ADDABLE {
            assert!(can_add(id), "{id}");
        }
        for id in [
            "dc:title",
            "photoshop:City",
            "photoshop:DateCreated",
            "Iptc4xmpExt:ModelAge",
            "Iptc4xmpExt:EventId",
            LOCATION_SHOWN,
            "plus:Licensor",
            CONTACT_INFO,
            "photoshop:SupplementalCategories",
            "xx:Unknown",
        ] {
            assert!(!can_add(id), "{id}");
        }
        // The rule gives exactly the nine among the template's properties.
        let addable: Vec<&str> = IN_TEMPLATE.into_iter().filter(|id| can_add(id)).collect();
        let mut expected = ADDABLE.to_vec();
        expected.sort_by_key(|id| IN_TEMPLATE.iter().position(|candidate| candidate == id));
        assert_eq!(addable, expected);
    }

    #[test]
    fn in_template_is_the_metadata_windows_fifty_properties() {
        assert_eq!(IN_TEMPLATE.len(), 50);
        for (index, id) in IN_TEMPLATE.iter().enumerate() {
            assert!(properties::property(id).is_some(), "{id} is in the table");
            assert!(
                !IN_TEMPLATE[..index].contains(id),
                "{id} is named only once"
            );
            assert!(!properties::is_deprecated_iim(id), "{id}");
        }
        for id in [
            "xmp:Rating",
            "xmp:Label",
            "photoshop:Urgency",
            "photoshop:Category",
            "photoshop:SupplementalCategories",
            "Iptc4xmpCore:IntellectualGenre",
            "Iptc4xmpExt:ImageRegion",
            "Iptc4xmpExt:DigImageGUID",
            "xx:Unknown",
        ] {
            assert!(!in_template(id), "{id}");
        }
        for id in ["dc:subject", CONTACT_INFO, LOCATION_CREATED] {
            assert!(in_template(id), "{id}");
        }
    }

    // --- new ---

    #[test]
    fn new_drops_a_field_not_in_the_template() {
        assert_eq!(
            made(&[("xmp:Rating", Value::Number("3".to_string()))]),
            TemplateFile::default()
        );
        assert_eq!(
            made(&[("Iptc4xmpCore:IntellectualGenre", text("Feature"))]),
            TemplateFile::default()
        );
        assert_eq!(made(&[("xx:Unknown", text("a"))]), TemplateFile::default());
    }

    #[test]
    fn new_drops_a_value_of_the_wrong_kind() {
        assert_eq!(kept("dc:subject", text("a")), None);
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::Struct(entry(&[(CITY, text("Lund"))]))
            ),
            None
        );
        assert_eq!(kept("dc:title", text("Hamn")), None);
        // A field of a structure that is not a field of that structure.
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::StructList(vec![entry(&[
                    (CITY, text("Lund")),
                    ("xx:Unknown", text("a"))
                ])])
            ),
            None
        );
    }

    #[test]
    fn new_drops_a_value_without_content() {
        assert_eq!(kept("photoshop:City", text("  ")), None);
        assert_eq!(kept("dc:subject", list(&[])), None);
        assert_eq!(kept(CONTACT_INFO, Value::Struct(entry(&[]))), None);
    }

    #[test]
    fn new_drops_an_empty_sub_field() {
        assert_eq!(
            kept(
                CONTACT_INFO,
                Value::Struct(entry(&[(EMAIL, text("a@example.com")), (PHONE, text(" "))]))
            ),
            Some(Value::Struct(entry(&[(EMAIL, text("a@example.com"))])))
        );
        // Only empty sub-fields: the structure is dropped.
        assert_eq!(
            kept(
                CONTACT_INFO,
                Value::Struct(entry(&[(EMAIL, text("")), (PHONE, text(" "))]))
            ),
            None
        );
    }

    #[test]
    fn new_cuts_a_lang_alt_to_x_default() {
        assert_eq!(
            kept(
                "dc:title",
                lang_alt(&[("sv", "Hamn"), ("x-default", "Harbour")])
            ),
            Some(lang_alt(&[("x-default", "Harbour")]))
        );
        // Cut first, then checked: a blank x-default is dropped even with another language.
        assert_eq!(
            kept("dc:title", lang_alt(&[("x-default", " "), ("sv", "Hamn")])),
            None
        );
        // Without x-default.
        assert_eq!(kept("dc:title", lang_alt(&[("sv", "Hamn")])), None);
        // X-Default is x-default.
        assert_eq!(
            kept(
                "dc:title",
                lang_alt(&[("sv", "Hamn"), ("X-Default", "Harbour")])
            ),
            Some(lang_alt(&[("x-default", "Harbour")]))
        );
    }

    #[test]
    fn new_cuts_a_lang_alt_inside_location_created() {
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::StructList(vec![entry(&[(
                    LOCATION_NAME,
                    lang_alt(&[("sv", "Hamnen"), ("x-default", "The harbour")])
                )])])
            ),
            Some(Value::StructList(vec![entry(&[(
                LOCATION_NAME,
                lang_alt(&[("x-default", "The harbour")])
            )])]))
        );
    }

    #[test]
    fn new_sorts_the_fields_of_a_location_shown_entry_and_drops_an_empty_one() {
        assert_eq!(
            kept(
                LOCATION_SHOWN,
                Value::StructList(vec![
                    entry(&[
                        (CITY, text("Lund")),
                        ("Iptc4xmpExt:CountryName", text("")),
                        (SUBLOCATION, text("Hamnen")),
                    ]),
                    entry(&[(CITY, text(" "))]),
                    entry(&[(CITY, text("Malmö"))]),
                ])
            ),
            Some(Value::StructList(vec![
                entry(&[(SUBLOCATION, text("Hamnen")), (CITY, text("Lund"))]),
                entry(&[(CITY, text("Malmö"))]),
            ]))
        );
    }

    #[test]
    fn new_drops_blank_and_repeated_list_entries() {
        assert_eq!(
            kept("dc:subject", list(&["a", " ", "a", "b"])),
            Some(list(&["a", "b"]))
        );
        assert_eq!(kept("dc:subject", list(&[" ", "", "\t"])), None);
        // Inside a structure too; the list is otherwise kept as it is.
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::StructList(vec![entry(&[(
                    "Iptc4xmpExt:LocationId",
                    list(&["b", " ", "a", "b"])
                )])])
            ),
            Some(Value::StructList(vec![entry(&[(
                "Iptc4xmpExt:LocationId",
                list(&["b", "a"])
            )])]))
        );
    }

    #[test]
    fn new_checks_the_kind_of_a_location_created_field() {
        // City is a text: a text of digits is kept, a list is not.
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::StructList(vec![entry(&[(CITY, text("12"))])])
            ),
            Some(Value::StructList(vec![entry(&[(CITY, text("12"))])]))
        );
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::StructList(vec![entry(&[(CITY, list(&["Lund"]))])])
            ),
            None
        );
    }

    #[test]
    fn new_keeps_only_the_first_location_created_entry() {
        assert_eq!(
            kept(
                LOCATION_CREATED,
                Value::StructList(vec![
                    entry(&[(CITY, text("Lund"))]),
                    entry(&[(CITY, text("Malmö"))]),
                ])
            ),
            Some(Value::StructList(vec![entry(&[(CITY, text("Lund"))])]))
        );
    }

    #[test]
    fn new_drops_a_list_of_structures_over_its_limit() {
        let creators = |names: &[&str]| {
            Value::StructList(
                names
                    .iter()
                    .map(|name| entry(&[("plus:ImageCreatorName", text(name))]))
                    .collect(),
            )
        };
        assert_eq!(
            kept("plus:ImageCreator", creators(&["A", "B", "C", "D"])),
            None
        );
        assert_eq!(
            kept("plus:ImageCreator", creators(&["A", "B", "C"])),
            Some(creators(&["A", "B", "C"]))
        );
        // Counted after an entry without content is dropped.
        assert_eq!(
            kept("plus:ImageCreator", creators(&["A", " ", "B", "C"])),
            Some(creators(&["A", "B", "C"]))
        );
        let suppliers = Value::StructList(vec![
            entry(&[("plus:ImageSupplierName", text("A"))]),
            entry(&[("plus:ImageSupplierName", text("B"))]),
        ]);
        assert_eq!(kept("plus:ImageSupplier", suppliers), None);
    }

    #[test]
    fn new_keeps_the_first_of_a_field_named_twice() {
        assert_eq!(
            made(&[
                ("photoshop:City", text("Lund")),
                ("photoshop:City", text("Malmö")),
            ]),
            TemplateFile {
                fields: fields(&[("photoshop:City", text("Lund"))]),
                add: Vec::new(),
            }
        );
    }

    #[test]
    fn new_puts_the_fields_in_table_order() {
        let template_file = made(&[
            ("photoshop:City", text("Lund")),
            ("dc:subject", list(&["a"])),
            ("dc:title", lang_alt(&[("x-default", "Hamn")])),
            ("photoshop:DateCreated", date("2026-11-23")),
        ]);
        let order: Vec<&str> = template_file
            .fields
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        let mut expected = vec![
            "photoshop:City",
            "dc:subject",
            "dc:title",
            "photoshop:DateCreated",
        ];
        expected.sort_by_key(|id| table_position(id));
        assert_eq!(order, expected);
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn new_keeps_a_plus_box_only_on_a_checked_list_of_text_once() {
        let template_file = TemplateFile::new(
            fields(&[
                ("dc:title", lang_alt(&[("x-default", "Hamn")])),
                ("dc:subject", list(&["a"])),
                ("dc:creator", list(&["Anna"])),
            ]),
            ids(&[
                "dc:creator",
                "Iptc4xmpExt:PersonInImage",
                "dc:title",
                "dc:subject",
                "dc:creator",
            ]),
        );
        let mut expected = ids(&["dc:creator", "dc:subject"]);
        expected.sort_by_key(|id| table_position(id));
        assert_eq!(template_file.add, expected);
    }

    #[test]
    fn new_run_on_its_own_result_gives_the_same() {
        let template_file = TemplateFile::new(
            fields(&[
                ("dc:subject", list(&["a", " ", "a", "b"])),
                (
                    "dc:title",
                    lang_alt(&[("sv", "Hamn"), ("X-Default", "Harbour")]),
                ),
                (
                    LOCATION_CREATED,
                    Value::StructList(vec![
                        entry(&[(CITY, text("Lund")), (SUBLOCATION, text("Hamnen"))]),
                        entry(&[(CITY, text("Malmö"))]),
                    ]),
                ),
                (
                    CONTACT_INFO,
                    Value::Struct(entry(&[(PHONE, text("1")), (EMAIL, text(""))])),
                ),
                ("photoshop:City", text("Lund")),
            ]),
            ids(&["dc:subject", "dc:title"]),
        );
        assert_eq!(template_file.fields.len(), 5);
        assert_eq!(template_file.add, ids(&["dc:subject"]));
        let again = TemplateFile::new(template_file.fields.clone(), template_file.add.clone());
        assert_eq!(again, template_file);
    }

    // --- to_template ---

    #[test]
    fn to_template_appends_a_list_with_its_plus_box_and_replaces_it_without() {
        let keywords = || read(&[("dc:subject", list(&["a", "b"]))]);
        let with_add = TemplateFile::new(
            fields(&[("dc:subject", list(&["b", "c"]))]),
            ids(&["dc:subject"]),
        );
        let mut metadata = keywords();
        applied(&with_add, &mut metadata);
        assert_eq!(
            value_of(&metadata, "dc:subject"),
            Some(&list(&["a", "b", "c"]))
        );

        let without = made(&[("dc:subject", list(&["b", "c"]))]);
        let mut metadata = keywords();
        applied(&without, &mut metadata);
        assert_eq!(value_of(&metadata, "dc:subject"), Some(&list(&["b", "c"])));
    }

    #[test]
    fn to_template_replaces_the_city_of_location_created_and_keeps_the_sublocation() {
        let template_file = made(&[(
            LOCATION_CREATED,
            Value::StructList(vec![entry(&[(CITY, text("Lund"))])]),
        )]);
        assert_eq!(
            template_file.to_template(),
            Template {
                fields: vec![TemplateField {
                    property: LOCATION_CREATED.to_string(),
                    field: Some(CITY.to_string()),
                    mode: Mode::Replace,
                    value: Some(text("Lund")),
                }],
            }
        );
        let mut metadata = read(&[(
            LOCATION_CREATED,
            Value::StructList(vec![entry(&[
                (SUBLOCATION, text("Hamnen")),
                (CITY, text("Göteborg")),
            ])]),
        )]);
        applied(&template_file, &mut metadata);
        assert_eq!(
            value_of(&metadata, LOCATION_CREATED),
            Some(&Value::StructList(vec![entry(&[
                (SUBLOCATION, text("Hamnen")),
                (CITY, text("Lund")),
            ])]))
        );
    }

    #[test]
    fn to_template_sets_the_email_of_the_contact_info_and_keeps_the_phone() {
        let template_file = made(&[(
            CONTACT_INFO,
            Value::Struct(entry(&[(EMAIL, text("a@example.com"))])),
        )]);
        let mut metadata = read(&[(CONTACT_INFO, Value::Struct(entry(&[(PHONE, text("1"))])))]);
        applied(&template_file, &mut metadata);
        let Some(Value::Struct(contact)) = value_of(&metadata, CONTACT_INFO) else {
            panic!("no contact info");
        };
        assert_eq!(contact.fields.len(), 2);
        for (id, value) in [(EMAIL, text("a@example.com")), (PHONE, text("1"))] {
            assert!(contact.fields.contains(&(id.to_string(), value)), "{id}");
        }
    }

    #[test]
    fn to_template_replaces_a_location_shown_list_whole() {
        let shown = |cities: &[&str]| {
            Value::StructList(
                cities
                    .iter()
                    .map(|city| entry(&[(CITY, text(city))]))
                    .collect(),
            )
        };
        let template_file = made(&[(LOCATION_SHOWN, shown(&["Lund", "Malmö"]))]);
        let mut metadata = read(&[(LOCATION_SHOWN, shown(&["A", "B", "C"]))]);
        applied(&template_file, &mut metadata);
        assert_eq!(
            value_of(&metadata, LOCATION_SHOWN),
            Some(&shown(&["Lund", "Malmö"]))
        );
    }
}
