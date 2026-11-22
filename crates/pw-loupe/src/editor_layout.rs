//! The metadata window's layout (ADR 0017 §2.3.1, step U10b, U10b-Q1–U10b-Q9): which sections and
//! which items of each section are shown, and in which order. Pure functions, no egui.
//!
//! - **Items** ([`EditorItem`], U10b-Q5): a field, the eight contact fields together ("Creator's
//!   Contact Info") or a structure list. Each belongs to one section and never moves to another
//!   (U10b-Q2).
//! - **The default** ([`EditorLayout::default`]): every section and item shown, in today's form
//!   order, so nothing changes for a user who never opens the Settings page.
//! - **What is left out** ([`EditorLayout::shown_properties`]): a property no shown item edits is
//!   shown read-only under Other Metadata (U10b-Q3); the rating and the label are always edited
//!   (U10b-Q9).
//! - **Names** ([`EditorItem::name`], [`section_name`], U10b-Q7): the fixed lower-case names
//!   `settings.txt` keeps the layout by.

use crate::editor::{EditorField, EditorForm, EditorSection, list_section};
use crate::info_panel::{Language, property_label};

/// The properties `shown_properties` always holds, whether Rating and Label is shown or not
/// (U10b-Q9).
const RATING_PROPERTIES: [&str; 2] = ["xmp:Rating", "xmp:Label"];

/// The names of the structure lists in `settings.txt` (U10b-Q7), in the order of
/// [`EditorForm::ALL_LISTS`].
const LIST_NAMES: [&str; 13] = [
    "person_details",
    "product_shown",
    "artwork",
    "contributor",
    "location_created",
    "location_shown",
    "genre",
    "about_cv_term",
    "registry_entry",
    "image_creator",
    "copyright_owner",
    "licensor",
    "image_supplier",
];

/// The name of the contact fields together in `settings.txt` (U10b-Q7).
const CONTACT_INFO_NAME: &str = "contact_info";

/// One item of a section's list in the metadata window's layout (U10b-Q5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorItem {
    /// A field that is not a contact field.
    Field(EditorField),
    /// The eight contact fields together, "Creator's Contact Info".
    ContactInfo,
    /// A structure list by its XMP id, one of `EditorForm::ALL_LISTS`.
    List(&'static str),
}

impl EditorItem {
    /// The item that holds `field`: [`EditorItem::ContactInfo`] for a contact field, else the
    /// field itself.
    pub fn of_field(field: EditorField) -> EditorItem {
        if field.struct_field().is_some() {
            EditorItem::ContactInfo
        } else {
            EditorItem::Field(field)
        }
    }

    /// The section the item belongs to. A list not in `EditorForm::ALL_LISTS` (only code can
    /// make one) gives Content.
    pub fn section(self) -> EditorSection {
        match self {
            EditorItem::Field(field) => field.section(),
            EditorItem::ContactInfo => EditorField::Address.section(),
            EditorItem::List(list) => list_section(list).unwrap_or(EditorSection::Content),
        }
    }

    /// The fields the item edits: the field; the eight contact fields in `EditorField::ALL`
    /// order; none for a list.
    pub fn fields(self) -> Vec<EditorField> {
        match self {
            EditorItem::Field(field) => vec![field],
            EditorItem::ContactInfo => EditorField::ALL
                .into_iter()
                .filter(|field| field.struct_field().is_some())
                .collect(),
            EditorItem::List(_) => Vec::new(),
        }
    }

    /// The XMP id of the property the item edits: the field's,
    /// `Iptc4xmpCore:CreatorContactInfo`, or the list's.
    pub fn property(self) -> &'static str {
        match self {
            EditorItem::Field(field) => field.property(),
            EditorItem::ContactInfo => EditorField::Address.property(),
            EditorItem::List(list) => list,
        }
    }

    /// The item's name as the window draws it: the field's name, or [`property_label`] of the
    /// contact structure or of the list.
    pub fn label(self) -> String {
        match self {
            EditorItem::Field(field) => field.label(),
            EditorItem::ContactInfo | EditorItem::List(_) => {
                property_label(self.property(), Language::En)
            }
        }
    }

    /// The item's name in `settings.txt` (U10b-Q7). An item outside the valid ones (only code can
    /// make one) gives an XMP id, which [`EditorItem::from_name`] never takes: the list's id, or
    /// the contact field's own structure field id.
    pub fn name(self) -> &'static str {
        match self {
            EditorItem::Field(field) => field_name(field),
            EditorItem::ContactInfo => CONTACT_INFO_NAME,
            EditorItem::List(list) => EditorForm::ALL_LISTS
                .iter()
                .position(|id| *id == list)
                .and_then(|index| LIST_NAMES.get(index))
                .copied()
                .unwrap_or(list),
        }
    }

    /// The item named `name` in `settings.txt`; `None` for an unknown name. Only valid items are
    /// given.
    pub fn from_name(name: &str) -> Option<EditorItem> {
        EditorSection::ALL
            .into_iter()
            .flat_map(EditorItem::defaults)
            .find(|item| item.name() == name)
    }

    /// The items of `section` in today's form order: the lists at the start of the section, then
    /// each field followed by the lists drawn after it, each list followed by the lists drawn
    /// after that list; the contact fields once, as [`EditorItem::ContactInfo`] in the place of
    /// the first (Address). Rating and Label has none.
    pub fn defaults(section: EditorSection) -> Vec<EditorItem> {
        let mut items = Vec::new();
        let push_lists = |items: &mut Vec<EditorItem>, after: Option<EditorField>| {
            for list in EditorForm::lists_at(section, after) {
                items.push(EditorItem::List(list));
                items.extend(
                    EditorForm::lists_after_list(list)
                        .into_iter()
                        .map(EditorItem::List),
                );
            }
        };
        push_lists(&mut items, None);
        for field in EditorField::ALL
            .into_iter()
            .filter(|field| field.section() == section)
        {
            let item = EditorItem::of_field(field);
            if !items.contains(&item) {
                items.push(item);
            }
            push_lists(&mut items, Some(field));
        }
        items
    }
}

/// The name of a field that is not a contact field in `settings.txt` (U10b-Q7); a contact field
/// gives its structure field's XMP id (it is never an item of its own).
fn field_name(field: EditorField) -> &'static str {
    match field {
        EditorField::Title => "title",
        EditorField::Headline => "headline",
        EditorField::Description => "description",
        EditorField::AltText => "alt_text",
        EditorField::ExtDescription => "ext_description",
        EditorField::DescriptionWriter => "description_writer",
        EditorField::Keywords => "keywords",
        EditorField::PersonShown => "person_shown",
        EditorField::OrganisationName => "organisation_name",
        EditorField::OrganisationCode => "organisation_code",
        EditorField::Event => "event",
        EditorField::Creator => "creator",
        EditorField::CreatorJobtitle => "creator_jobtitle",
        EditorField::Copyright => "copyright",
        EditorField::Credit => "credit",
        EditorField::Source => "source",
        EditorField::UsageTerms => "usage_terms",
        EditorField::WebStatement => "web_statement",
        EditorField::Copyrighted => "copyrighted",
        EditorField::DateCreated => "date_created",
        EditorField::Sublocation => "sublocation",
        EditorField::City => "city",
        EditorField::State => "state",
        EditorField::Country => "country",
        EditorField::CountryCode => "country_code",
        EditorField::Instructions => "instructions",
        EditorField::JobId => "job_id",
        EditorField::SubjectCode => "subject_code",
        EditorField::SceneCode => "scene_code",
        EditorField::Urgency => "urgency",
        EditorField::Category => "category",
        EditorField::SupplementalCategories => "supplemental_categories",
        EditorField::ModelReleaseStatus => "model_release_status",
        EditorField::ModelReleaseId => "model_release_id",
        EditorField::MinorModelAge => "minor_model_age",
        EditorField::PropertyReleaseStatus => "property_release_status",
        EditorField::PropertyReleaseId => "property_release_id",
        EditorField::DataMining => "data_mining",
        EditorField::DigitalSourceType => "digital_source_type",
        EditorField::Address
        | EditorField::ContactCity
        | EditorField::ContactRegion
        | EditorField::PostalCode
        | EditorField::ContactCountry
        | EditorField::Email
        | EditorField::Phone
        | EditorField::Web => field.struct_field().unwrap_or(field.property()),
    }
}

/// The name of a section in `settings.txt` (U10b-Q7), as in its `editor_*` key.
pub(crate) fn section_name(section: EditorSection) -> &'static str {
    match section {
        EditorSection::Content => "content",
        EditorSection::CreatorRights => "creator_rights",
        EditorSection::TimePlace => "time_place",
        EditorSection::Assignment => "assignment",
        EditorSection::Licence => "licence",
        EditorSection::RatingLabel => "rating_label",
    }
}

/// The section named `name` in `settings.txt`; `None` for an unknown name.
pub(crate) fn section_from_name(name: &str) -> Option<EditorSection> {
    EditorSection::ALL
        .into_iter()
        .find(|section| section_name(*section) == name)
}

/// The order `given` names, each entry once at its first place and only the entries of
/// `default`, with every entry of `default` it leaves out put back in its default place: right
/// after the entry before it in `default`, or first when it is the first (U10b-Q7).
fn merged_order<T: Copy + PartialEq>(given: &[T], default: &[T]) -> Vec<T> {
    let mut order: Vec<T> = Vec::new();
    for entry in given {
        if default.contains(entry) && !order.contains(entry) {
            order.push(*entry);
        }
    }
    let mut previous: Option<T> = None;
    for entry in default {
        if !order.contains(entry) {
            let place = previous
                .and_then(|before| order.iter().position(|e| *e == before))
                .map_or(0, |index| index + 1);
            order.insert(place, *entry);
        }
        previous = Some(*entry);
    }
    order
}

/// The metadata window's layout (U10b-Q2): the sections in their order, each shown or hidden,
/// and every section's items in their order, each shown or hidden. An item is drawn only when it
/// and its section are shown.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorLayout {
    /// Every section once, in the layout's order, with whether it is shown.
    sections: Vec<(EditorSection, bool)>,
    /// Every item once with whether it is shown, grouped by section in `EditorSection::ALL`
    /// order, each section's items in the layout's order.
    items: Vec<(EditorItem, bool)>,
}

impl Default for EditorLayout {
    /// Every section in `EditorSection::ALL` order and every section's items in today's form
    /// order, all shown.
    fn default() -> Self {
        EditorLayout {
            sections: EditorSection::ALL
                .into_iter()
                .map(|section| (section, true))
                .collect(),
            items: EditorSection::ALL
                .into_iter()
                .flat_map(EditorItem::defaults)
                .map(|item| (item, true))
                .collect(),
        }
    }
}

impl EditorLayout {
    /// Every section in the layout's order with whether it is shown.
    pub fn sections(&self) -> Vec<(EditorSection, bool)> {
        self.sections.clone()
    }

    /// Every item of `section` in the layout's order with whether the item itself is shown (not
    /// looking at the section).
    pub fn items(&self, section: EditorSection) -> Vec<(EditorItem, bool)> {
        self.items
            .iter()
            .filter(|(item, _)| item.section() == section)
            .copied()
            .collect()
    }

    /// The shown sections in the layout's order.
    pub fn shown_sections(&self) -> Vec<EditorSection> {
        self.sections
            .iter()
            .filter(|(_, shown)| *shown)
            .map(|(section, _)| *section)
            .collect()
    }

    /// The shown items of `section` in the layout's order, whether or not the section is shown.
    pub fn shown_items(&self, section: EditorSection) -> Vec<EditorItem> {
        self.items(section)
            .into_iter()
            .filter(|(_, shown)| *shown)
            .map(|(item, _)| item)
            .collect()
    }

    /// Whether `section` is shown.
    fn section_shown(&self, section: EditorSection) -> bool {
        self.sections
            .iter()
            .any(|(s, shown)| *s == section && *shown)
    }

    /// True when `item` is shown and so is its section. An item the layout does not hold (only
    /// code can make one) is not shown.
    pub fn is_shown(&self, item: EditorItem) -> bool {
        self.items.iter().any(|(i, shown)| *i == item && *shown)
            && self.section_shown(item.section())
    }

    /// Shows or hides `section`.
    pub fn set_section_shown(&mut self, section: EditorSection, shown: bool) {
        if let Some(entry) = self.sections.iter_mut().find(|(s, _)| *s == section) {
            entry.1 = shown;
        }
    }

    /// Shows or hides `item`; nothing for an item the layout does not hold.
    pub fn set_item_shown(&mut self, item: EditorItem, shown: bool) {
        if let Some(entry) = self.items.iter_mut().find(|(i, _)| *i == item) {
            entry.1 = shown;
        }
    }

    /// Moves `section` one place up; nothing when it is first.
    pub fn move_section_up(&mut self, section: EditorSection) {
        if let Some(index) = self.sections.iter().position(|(s, _)| *s == section)
            && index > 0
        {
            self.sections.swap(index - 1, index);
        }
    }

    /// Moves `item` one place up inside its own section; nothing when it is the section's first,
    /// or for an item the layout does not hold (U10b-Q2: an item never moves to another
    /// section).
    pub fn move_item_up(&mut self, item: EditorItem) {
        let Some(index) = self.items.iter().position(|(i, _)| *i == item) else {
            return;
        };
        let section = item.section();
        if let Some(before) = self
            .items
            .get(..index)
            .and_then(|items| items.iter().rposition(|(i, _)| i.section() == section))
        {
            self.items.swap(before, index);
        }
    }

    /// The property of every item [`EditorLayout::is_shown`] gives, each once, sections and items
    /// in the layout's order, then `xmp:Rating` and `xmp:Label` always, whether Rating and Label
    /// is shown or not (U10b-Q9). What it leaves out is shown read-only under Other Metadata
    /// (U10b-Q3).
    pub fn shown_properties(&self) -> Vec<&'static str> {
        let mut properties: Vec<&'static str> = Vec::new();
        for section in self.shown_sections() {
            for item in self.shown_items(section) {
                let property = item.property();
                if !properties.contains(&property) {
                    properties.push(property);
                }
            }
        }
        for property in RATING_PROPERTIES {
            if !properties.contains(&property) {
                properties.push(property);
            }
        }
        properties
    }

    /// True when the layout is the default one.
    pub fn is_default(&self) -> bool {
        *self == EditorLayout::default()
    }

    /// Puts the sections in the order `names` gives; a repeated entry counts once, at its first
    /// place, and a section it leaves out goes back to its default place (right after the one
    /// before it in `EditorSection::ALL`, or first). Every section keeps its shown flag.
    pub fn set_section_order(&mut self, names: &[EditorSection]) {
        let order = merged_order(names, &EditorSection::ALL);
        let flag = |section: EditorSection| {
            self.sections
                .iter()
                .find(|(s, _)| *s == section)
                .is_none_or(|(_, shown)| *shown)
        };
        self.sections = order
            .into_iter()
            .map(|section| (section, flag(section)))
            .collect();
    }

    /// Puts every section's items in the order `names` gives them; a repeated entry counts once,
    /// at its first place, an item it leaves out goes back to its default place in its section
    /// (right after the one before it in [`EditorItem::defaults`], or first), and an item stays
    /// in its own section wherever `names` has it. Every item keeps its shown flag.
    pub fn set_item_order(&mut self, names: &[EditorItem]) {
        let flag = |item: EditorItem| {
            self.items
                .iter()
                .find(|(i, _)| *i == item)
                .is_none_or(|(_, shown)| *shown)
        };
        let items: Vec<(EditorItem, bool)> = EditorSection::ALL
            .into_iter()
            .flat_map(|section| {
                let given: Vec<EditorItem> = names
                    .iter()
                    .copied()
                    .filter(|item| item.section() == section)
                    .collect();
                merged_order(&given, &EditorItem::defaults(section))
            })
            .map(|item| (item, flag(item)))
            .collect();
        self.items = items;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    use EditorSection::{Assignment, Content, CreatorRights, Licence, RatingLabel, TimePlace};

    /// The names of `items`.
    fn names(items: &[EditorItem]) -> Vec<&'static str> {
        items.iter().map(|item| item.name()).collect()
    }

    /// The items named `names`.
    fn items_named(names: &[&str]) -> Vec<EditorItem> {
        names
            .iter()
            .map(|name| EditorItem::from_name(name).unwrap())
            .collect()
    }

    fn item(name: &str) -> EditorItem {
        EditorItem::from_name(name).unwrap()
    }

    /// The names of the items of `section` in `layout`'s order.
    fn order(layout: &EditorLayout, section: EditorSection) -> Vec<&'static str> {
        layout
            .items(section)
            .iter()
            .map(|(item, _)| item.name())
            .collect()
    }

    #[test]
    fn the_default_sections_are_every_section_in_form_order_all_shown() {
        let layout = EditorLayout::default();
        let expected: Vec<(EditorSection, bool)> = EditorSection::ALL
            .into_iter()
            .map(|section| (section, true))
            .collect();
        assert_eq!(layout.sections(), expected);
        assert_eq!(layout.shown_sections(), EditorSection::ALL.to_vec());
        assert!(layout.is_default());
    }

    #[test]
    fn the_default_items_are_todays_form_order() {
        let cases: [(EditorSection, &[&str]); 6] = [
            (
                Content,
                &[
                    "title",
                    "headline",
                    "description",
                    "alt_text",
                    "ext_description",
                    "description_writer",
                    "keywords",
                    "person_shown",
                    "person_details",
                    "product_shown",
                    "artwork",
                    "organisation_name",
                    "organisation_code",
                    "event",
                ],
            ),
            (
                CreatorRights,
                &[
                    "creator",
                    "creator_jobtitle",
                    "contact_info",
                    "copyright",
                    "credit",
                    "source",
                    "usage_terms",
                    "web_statement",
                    "copyrighted",
                    "contributor",
                ],
            ),
            (
                TimePlace,
                &[
                    "date_created",
                    "sublocation",
                    "city",
                    "state",
                    "country",
                    "country_code",
                    "location_created",
                    "location_shown",
                ],
            ),
            (
                Assignment,
                &[
                    "instructions",
                    "job_id",
                    "subject_code",
                    "scene_code",
                    "genre",
                    "about_cv_term",
                    "urgency",
                    "category",
                    "supplemental_categories",
                    "registry_entry",
                ],
            ),
            (
                Licence,
                &[
                    "image_creator",
                    "copyright_owner",
                    "licensor",
                    "image_supplier",
                    "model_release_status",
                    "model_release_id",
                    "minor_model_age",
                    "property_release_status",
                    "property_release_id",
                    "data_mining",
                    "digital_source_type",
                ],
            ),
            (RatingLabel, &[]),
        ];
        let layout = EditorLayout::default();
        for (section, expected) in cases {
            assert_eq!(
                names(&EditorItem::defaults(section)),
                expected,
                "{section:?}"
            );
            assert_eq!(order(&layout, section), expected, "{section:?}");
            assert!(
                layout.items(section).iter().all(|(_, shown)| *shown),
                "{section:?}"
            );
        }
    }

    #[test]
    fn every_field_and_every_list_is_in_exactly_one_default_item() {
        let items: Vec<EditorItem> = EditorSection::ALL
            .into_iter()
            .flat_map(EditorItem::defaults)
            .collect();
        for field in EditorField::ALL {
            let holding = items
                .iter()
                .filter(|item| item.fields().contains(&field))
                .count();
            assert_eq!(holding, 1, "{field:?}");
        }
        for list in EditorForm::ALL_LISTS {
            let holding = items
                .iter()
                .filter(|item| **item == EditorItem::List(list))
                .count();
            assert_eq!(holding, 1, "{list}");
        }
        // Each item in the section it says.
        for section in EditorSection::ALL {
            for item in EditorItem::defaults(section) {
                assert_eq!(item.section(), section, "{item:?}");
            }
        }
    }

    #[test]
    fn every_name_is_fixed_and_round_trips() {
        let fields = [
            (EditorField::Title, "title"),
            (EditorField::Headline, "headline"),
            (EditorField::Description, "description"),
            (EditorField::AltText, "alt_text"),
            (EditorField::ExtDescription, "ext_description"),
            (EditorField::DescriptionWriter, "description_writer"),
            (EditorField::Keywords, "keywords"),
            (EditorField::PersonShown, "person_shown"),
            (EditorField::OrganisationName, "organisation_name"),
            (EditorField::OrganisationCode, "organisation_code"),
            (EditorField::Event, "event"),
            (EditorField::Creator, "creator"),
            (EditorField::CreatorJobtitle, "creator_jobtitle"),
            (EditorField::Copyright, "copyright"),
            (EditorField::Credit, "credit"),
            (EditorField::Source, "source"),
            (EditorField::UsageTerms, "usage_terms"),
            (EditorField::WebStatement, "web_statement"),
            (EditorField::Copyrighted, "copyrighted"),
            (EditorField::DateCreated, "date_created"),
            (EditorField::Sublocation, "sublocation"),
            (EditorField::City, "city"),
            (EditorField::State, "state"),
            (EditorField::Country, "country"),
            (EditorField::CountryCode, "country_code"),
            (EditorField::Instructions, "instructions"),
            (EditorField::JobId, "job_id"),
            (EditorField::SubjectCode, "subject_code"),
            (EditorField::SceneCode, "scene_code"),
            (EditorField::Urgency, "urgency"),
            (EditorField::Category, "category"),
            (
                EditorField::SupplementalCategories,
                "supplemental_categories",
            ),
            (EditorField::ModelReleaseStatus, "model_release_status"),
            (EditorField::ModelReleaseId, "model_release_id"),
            (EditorField::MinorModelAge, "minor_model_age"),
            (
                EditorField::PropertyReleaseStatus,
                "property_release_status",
            ),
            (EditorField::PropertyReleaseId, "property_release_id"),
            (EditorField::DataMining, "data_mining"),
            (EditorField::DigitalSourceType, "digital_source_type"),
        ];
        let lists = [
            ("Iptc4xmpExt:PersonInImageWDetails", "person_details"),
            ("Iptc4xmpExt:ProductInImage", "product_shown"),
            ("Iptc4xmpExt:ArtworkOrObject", "artwork"),
            ("Iptc4xmpExt:Contributor", "contributor"),
            ("Iptc4xmpExt:LocationCreated", "location_created"),
            ("Iptc4xmpExt:LocationShown", "location_shown"),
            ("Iptc4xmpExt:Genre", "genre"),
            ("Iptc4xmpExt:AboutCvTerm", "about_cv_term"),
            ("Iptc4xmpExt:RegistryId", "registry_entry"),
            ("plus:ImageCreator", "image_creator"),
            ("plus:CopyrightOwner", "copyright_owner"),
            ("plus:Licensor", "licensor"),
            ("plus:ImageSupplier", "image_supplier"),
        ];
        let mut expected: Vec<(EditorItem, &str)> = fields
            .into_iter()
            .map(|(field, name)| (EditorItem::Field(field), name))
            .collect();
        expected.push((EditorItem::ContactInfo, "contact_info"));
        expected.extend(
            lists
                .into_iter()
                .map(|(list, name)| (EditorItem::List(list), name)),
        );
        for (item, name) in &expected {
            assert_eq!(item.name(), *name, "{item:?}");
            assert_eq!(EditorItem::from_name(name), Some(*item), "{name}");
        }
        // The table holds every item, each name once.
        let all: Vec<EditorItem> = EditorSection::ALL
            .into_iter()
            .flat_map(EditorItem::defaults)
            .collect();
        assert_eq!(all.len(), expected.len());
        for item in &all {
            assert!(expected.iter().any(|(i, _)| i == item), "{item:?}");
        }
        let distinct: HashSet<&str> = expected.iter().map(|(_, name)| *name).collect();
        assert_eq!(distinct.len(), expected.len());
        assert_eq!(EditorItem::from_name("no_such_item"), None);
        assert_eq!(EditorItem::from_name(""), None);

        let sections = [
            (Content, "content"),
            (CreatorRights, "creator_rights"),
            (TimePlace, "time_place"),
            (Assignment, "assignment"),
            (Licence, "licence"),
            (RatingLabel, "rating_label"),
        ];
        for (section, name) in sections {
            assert_eq!(section_name(section), name, "{section:?}");
            assert_eq!(section_from_name(name), Some(section), "{name}");
        }
        assert_eq!(section_from_name("rating"), None);
    }

    #[test]
    fn each_contact_field_is_the_contact_info_item() {
        let contact: Vec<EditorField> = EditorField::ALL
            .into_iter()
            .filter(|field| field.struct_field().is_some())
            .collect();
        assert_eq!(contact.len(), 8);
        for field in &contact {
            assert_eq!(
                EditorItem::of_field(*field),
                EditorItem::ContactInfo,
                "{field:?}"
            );
        }
        assert_eq!(EditorItem::ContactInfo.fields(), contact);
        assert_eq!(
            EditorItem::ContactInfo.property(),
            "Iptc4xmpCore:CreatorContactInfo"
        );
        assert_eq!(EditorItem::ContactInfo.section(), CreatorRights);
        assert_eq!(
            EditorItem::ContactInfo.label(),
            property_label("Iptc4xmpCore:CreatorContactInfo", Language::En)
        );
        // Another field is its own item, with its own property and name.
        let city = EditorItem::of_field(EditorField::City);
        assert_eq!(city, EditorItem::Field(EditorField::City));
        assert_eq!(city.fields(), vec![EditorField::City]);
        assert_eq!(city.property(), "photoshop:City");
        assert_eq!(city.label(), EditorField::City.label());
        // A list edits no field; its property is its id, its name the list's.
        let genre = EditorItem::List(EditorForm::ALL_LISTS[6]);
        assert!(genre.fields().is_empty());
        assert_eq!(genre.property(), "Iptc4xmpExt:Genre");
        assert_eq!(genre.section(), Assignment);
        assert_eq!(
            genre.label(),
            property_label("Iptc4xmpExt:Genre", Language::En)
        );
    }

    #[test]
    fn an_item_outside_the_valid_ones_never_panics_and_is_never_shown() {
        let unknown_list = EditorItem::List("Iptc4xmpExt:NoSuchList");
        let contact_field = EditorItem::Field(EditorField::Email);
        assert_eq!(unknown_list.section(), Content);
        assert_eq!(contact_field.section(), CreatorRights);
        assert_eq!(unknown_list.name(), "Iptc4xmpExt:NoSuchList");
        assert_eq!(contact_field.name(), "Iptc4xmpCore:CiEmailWork");
        assert_eq!(EditorItem::from_name(unknown_list.name()), None);
        assert_eq!(EditorItem::from_name(contact_field.name()), None);

        let mut layout = EditorLayout::default();
        for invalid in [unknown_list, contact_field] {
            assert!(!layout.is_shown(invalid), "{invalid:?}");
            layout.set_item_shown(invalid, true);
            layout.move_item_up(invalid);
            assert!(!layout.is_shown(invalid), "{invalid:?}");
            assert_eq!(layout, EditorLayout::default(), "{invalid:?}");
        }
    }

    #[test]
    fn the_default_shown_properties_are_the_55_the_window_edits() {
        let properties = EditorLayout::default().shown_properties();
        // 39 fields, the contact structure, 13 lists and the rating and the label.
        assert_eq!(properties.len(), 55);
        let distinct: HashSet<&str> = properties.iter().copied().collect();
        assert_eq!(distinct.len(), 55);
        for field in EditorField::ALL {
            assert!(properties.contains(&field.property()), "{field:?}");
        }
        for list in EditorForm::ALL_LISTS {
            assert!(properties.contains(&list), "{list}");
        }
        assert!(properties.contains(&"xmp:Rating"));
        assert!(properties.contains(&"xmp:Label"));
    }

    #[test]
    fn a_hidden_item_is_not_shown_and_its_property_is_left_out() {
        let mut layout = EditorLayout::default();
        let city = item("city");
        let genre = item("genre");
        layout.set_item_shown(city, false);
        layout.set_item_shown(genre, false);
        assert!(!layout.is_shown(city));
        assert!(!layout.is_shown(genre));
        assert!(layout.is_shown(item("state")));
        let properties = layout.shown_properties();
        assert!(!properties.contains(&"photoshop:City"));
        assert!(!properties.contains(&"Iptc4xmpExt:Genre"));
        assert_eq!(properties.len(), 53);
        // Still listed, as hidden, in its place.
        assert!(layout.items(TimePlace).contains(&(city, false)));
        assert!(!layout.shown_items(TimePlace).contains(&city));
        assert!(!layout.is_default());
        // Shown again, the layout is the default one.
        layout.set_item_shown(city, true);
        layout.set_item_shown(genre, true);
        assert!(layout.is_default());
    }

    #[test]
    fn a_hidden_section_hides_its_items_properties_but_keeps_their_own_flags() {
        let mut layout = EditorLayout::default();
        layout.set_item_shown(item("city"), false);
        layout.set_section_shown(TimePlace, false);
        assert_eq!(
            layout.shown_sections(),
            vec![Content, CreatorRights, Assignment, Licence, RatingLabel]
        );
        assert!(layout.sections().contains(&(TimePlace, false)));
        for (item, _) in layout.items(TimePlace) {
            assert!(!layout.is_shown(item), "{item:?}");
            assert!(!layout.shown_properties().contains(&item.property()));
        }
        // Each item's own flag is still reported.
        assert!(layout.items(TimePlace).contains(&(item("city"), false)));
        assert!(layout.items(TimePlace).contains(&(item("state"), true)));
        assert!(layout.shown_items(TimePlace).contains(&item("state")));
        // 55 less the eight items of Time and Place.
        assert_eq!(layout.shown_properties().len(), 47);
        // Shown again, its items come back, City still hidden.
        layout.set_section_shown(TimePlace, true);
        assert!(layout.is_shown(item("state")));
        assert!(!layout.is_shown(item("city")));
    }

    #[test]
    fn the_rating_and_the_label_are_always_in_the_shown_properties() {
        let mut layout = EditorLayout::default();
        layout.set_section_shown(RatingLabel, false);
        let properties = layout.shown_properties();
        assert!(properties.contains(&"xmp:Rating"));
        assert!(properties.contains(&"xmp:Label"));
        assert_eq!(properties.len(), 55);
        layout.set_section_shown(RatingLabel, true);
        assert_eq!(layout.shown_properties().len(), 55);
        assert!(layout.is_default());
    }

    #[test]
    fn moving_up_stops_at_the_first_place_and_stays_in_the_section() {
        let mut layout = EditorLayout::default();
        // The first section and the first item of a section do not move.
        layout.move_section_up(Content);
        layout.move_item_up(item("title"));
        // Date Created is the first of Time and Place: it never moves into Creator and Rights.
        layout.move_item_up(item("date_created"));
        layout.move_item_up(item("image_creator"));
        assert_eq!(layout, EditorLayout::default());

        layout.move_section_up(Licence);
        assert_eq!(
            layout
                .sections()
                .iter()
                .map(|(s, _)| *s)
                .collect::<Vec<_>>(),
            vec![
                Content,
                CreatorRights,
                TimePlace,
                Licence,
                Assignment,
                RatingLabel
            ]
        );
        layout.move_item_up(item("contact_info"));
        layout.move_item_up(item("contact_info"));
        layout.move_item_up(item("contact_info"));
        assert_eq!(
            order(&layout, CreatorRights),
            [
                "contact_info",
                "creator",
                "creator_jobtitle",
                "copyright",
                "credit",
                "source",
                "usage_terms",
                "web_statement",
                "copyrighted",
                "contributor",
            ]
        );
        // The other sections are as they were.
        for section in [Content, TimePlace, Assignment, Licence] {
            assert_eq!(
                order(&layout, section),
                names(&EditorItem::defaults(section)),
                "{section:?}"
            );
        }
        // A hidden item moves with its flag.
        layout.set_item_shown(item("city"), false);
        layout.move_item_up(item("city"));
        assert_eq!(layout.items(TimePlace).get(1), Some(&(item("city"), false)));
    }

    #[test]
    fn an_item_order_leaving_one_out_in_the_middle_puts_it_back_after_its_default_neighbour() {
        let mut layout = EditorLayout::default();
        // Time and Place reversed, City left out: City goes right after Sublocation.
        layout.set_item_order(&items_named(&[
            "location_shown",
            "location_created",
            "country_code",
            "country",
            "state",
            "sublocation",
            "date_created",
        ]));
        assert_eq!(
            order(&layout, TimePlace),
            [
                "location_shown",
                "location_created",
                "country_code",
                "country",
                "state",
                "sublocation",
                "city",
                "date_created",
            ]
        );
        // The sections the order does not name keep their default order.
        assert_eq!(
            order(&layout, Content),
            names(&EditorItem::defaults(Content))
        );
    }

    #[test]
    fn an_item_order_leaving_out_the_first_puts_it_first() {
        let mut layout = EditorLayout::default();
        // Time and Place reversed, Date Created left out: it goes first.
        layout.set_item_order(&items_named(&[
            "location_shown",
            "location_created",
            "country_code",
            "country",
            "state",
            "city",
            "sublocation",
        ]));
        assert_eq!(
            order(&layout, TimePlace),
            [
                "date_created",
                "location_shown",
                "location_created",
                "country_code",
                "country",
                "state",
                "city",
                "sublocation",
            ]
        );
        // Licence (PLUS)'s first item, a list, likewise.
        let mut licence: Vec<&str> = names(&EditorItem::defaults(Licence));
        licence.remove(0);
        licence.reverse();
        layout.set_item_order(&items_named(&licence));
        let mut expected = vec!["image_creator"];
        expected.extend(licence);
        assert_eq!(order(&layout, Licence), expected);
    }

    #[test]
    fn a_repeated_name_counts_once_at_its_first_place() {
        let mut layout = EditorLayout::default();
        layout.set_item_order(&items_named(&[
            "state",
            "date_created",
            "state",
            "sublocation",
            "city",
            "country",
            "country_code",
            "location_created",
            "location_shown",
            "date_created",
        ]));
        assert_eq!(
            order(&layout, TimePlace),
            [
                "state",
                "date_created",
                "sublocation",
                "city",
                "country",
                "country_code",
                "location_created",
                "location_shown",
            ]
        );
        layout.set_section_order(&[
            Licence,
            Content,
            Licence,
            CreatorRights,
            TimePlace,
            Assignment,
            RatingLabel,
        ]);
        assert_eq!(
            layout
                .sections()
                .iter()
                .map(|(s, _)| *s)
                .collect::<Vec<_>>(),
            vec![
                Licence,
                Content,
                CreatorRights,
                TimePlace,
                Assignment,
                RatingLabel
            ]
        );
    }

    #[test]
    fn an_item_order_mixing_sections_keeps_each_item_in_its_own_section() {
        let mut layout = EditorLayout::default();
        // Time and Place with City first, Content's Event and Creator and Rights' Credit among
        // its items.
        layout.set_item_order(&items_named(&[
            "city",
            "event",
            "date_created",
            "sublocation",
            "credit",
            "state",
            "country",
            "country_code",
            "location_created",
            "location_shown",
        ]));
        assert_eq!(
            order(&layout, TimePlace),
            [
                "city",
                "date_created",
                "sublocation",
                "state",
                "country",
                "country_code",
                "location_created",
                "location_shown",
            ]
        );
        assert_eq!(order(&layout, Content).last(), Some(&"event"));
        assert!(order(&layout, CreatorRights).contains(&"credit"));
        for section in EditorSection::ALL {
            for (item, _) in layout.items(section) {
                assert_eq!(item.section(), section, "{item:?}");
            }
        }
    }

    #[test]
    fn the_order_setters_keep_every_shown_flag() {
        let mut layout = EditorLayout::default();
        layout.set_item_shown(item("city"), false);
        layout.set_section_shown(Assignment, false);
        layout.set_item_order(&items_named(&["country", "city"]));
        layout.set_section_order(&[
            Assignment,
            Content,
            CreatorRights,
            TimePlace,
            Licence,
            RatingLabel,
        ]);
        assert!(layout.items(TimePlace).contains(&(item("city"), false)));
        assert!(layout.items(TimePlace).contains(&(item("country"), true)));
        assert_eq!(layout.sections().first(), Some(&(Assignment, false)));
        assert!(
            layout
                .sections()
                .iter()
                .filter(|(s, _)| *s != Assignment)
                .all(|(_, shown)| *shown)
        );
        // An empty order is the default order, the flags still kept.
        layout.set_item_order(&[]);
        layout.set_section_order(&[]);
        let mut expected = EditorLayout::default();
        expected.set_item_shown(item("city"), false);
        expected.set_section_shown(Assignment, false);
        assert_eq!(layout, expected);
    }
}
