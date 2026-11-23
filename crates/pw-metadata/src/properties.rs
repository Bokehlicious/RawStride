//! The IPTC properties RawStride knows (ADR 0007 §3).
//!
//! ADR 0007 §3 keys the internal metadata model by the IPTC property's XMP name (`dc:creator`,
//! `photoshop:City`, …); IIM datasets and Exif tags are only representations of it. This module
//! describes every property: its value kind, whether it may repeat, and its IIM dataset, Exif tag
//! and ExifTool tag names.
//!
//! - **The table** (`TOP` and `STRUCTS` in `iptc_table.rs`) is generated from the IPTC Photo
//!   Metadata TechReference 2025.1 JSON (`iptc-pmd-techreference_2025.1.json`, kept outside the
//!   repository; its source is in `docs/references.md`, its SHA-256 in the header of
//!   `iptc_table.rs` and in `D:\PhotoWorkflowData\references\README.md`). A new IPTC version is a table update: run
//!   `.\tools\iptc-table.ps1 -Json <path to the JSON>` and review the diff. Never edit
//!   `iptc_table.rs` by hand.
//! - **The rules the TechReference does not hold** are written here by hand, each naming its ADR
//!   section: Seq or Bag (`list_kind`), the cardinality limits (`max_entries`), and the extra
//!   entries `xmp:Label`, `xmpRights:Marked` and the deprecated IIM datasets (`properties`,
//!   `is_deprecated_iim`).

use crate::iptc_table::{STRUCTS, TOP};

/// The schema a property belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    /// IPTC Core (`ipmdschema` `IptcCore`).
    Core,
    /// IPTC Extension (`ipmdschema` `IptcExt`).
    Extension,
    /// Not an IPTC property: `xmp:Label`, `xmpRights:Marked` (D6) and the deprecated IIM datasets
    /// (ADR 0007 §3, §5.5).
    Other,
}

/// The kind of value of a property or structure field, from the JSON's `datatype` and
/// `dataformat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// `string` without `dataformat`.
    Text,
    /// `struct` + `AltLang`: the JSON marks a Lang Alt value as the structure `AltLang`.
    LangAlt,
    /// `string` + `date-time`.
    Date,
    /// `string` + `uri`.
    Uri,
    /// `string` + `url`.
    Url,
    /// `number` + `integer`.
    Integer,
    /// `number` without `dataformat`.
    Number,
    /// `any`: the "Other Metadata Property" of an image region.
    Any,
    /// `struct` + the name of an `ipmd_struct` entry (see [`struct_def`]).
    Struct(&'static str),
}

/// Whether a property holds one value or a list (`propoccurrence`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occurrence {
    Single,
    Multi,
}

/// The XMP array type of a `Multi` property (see [`list_kind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    /// Ordered list (`rdf:Seq`).
    Seq,
    /// Unordered list (`rdf:Bag`).
    Bag,
}

/// An IIM dataset (`IIMid` `"2:90"` → record 2, dataset 90) and its byte limit (`IIMmaxbytes`;
/// per entry for a `Multi` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IimDataset {
    pub record: u8,
    pub dataset: u8,
    pub max_bytes: Option<u16>,
}

/// A top-level property. The strings are the JSON values as they are; a missing JSON key is
/// `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyDef {
    /// The JSON key of the entry (e.g. `creatorNames`).
    pub key: &'static str,
    /// `name`.
    pub name: &'static str,
    /// `ipmdschema`.
    pub schema: Schema,
    /// `datatype` + `dataformat`.
    pub kind: ValueKind,
    /// `propoccurrence`.
    pub occurrence: Occurrence,
    /// `XMPid`: the key of the internal model.
    pub xmp_id: &'static str,
    /// `IIMid` + `IIMmaxbytes`.
    pub iim: Option<IimDataset>,
    /// `IIMname`.
    pub iim_name: Option<&'static str>,
    /// `EXIFid`, the Exif tag number as the JSON text.
    pub exif_id: Option<&'static str>,
    /// `etXMP`: the ExifTool tag name of the XMP representation.
    pub et_xmp: Option<&'static str>,
    /// `etIIM`: the ExifTool tag name(s) of the IIM representation.
    pub et_iim: Option<&'static str>,
    /// `etEXIF`: the ExifTool tag name(s) of the Exif representation.
    pub et_exif: Option<&'static str>,
}

/// A structure (`ipmd_struct` entry) and its fields in `sortorder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructDef {
    pub name: &'static str,
    pub fields: &'static [FieldDef],
}

/// A field of a structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDef {
    /// The JSON key of the field (e.g. `city`).
    pub key: &'static str,
    /// `name`.
    pub name: &'static str,
    /// `datatype` + `dataformat`.
    pub kind: ValueKind,
    /// `propoccurrence`.
    pub occurrence: Occurrence,
    /// `XMPid`.
    pub xmp_id: &'static str,
    /// `etTag`: the ExifTool name of the field inside its structure.
    pub et_tag: Option<&'static str>,
}

/// Properties that are not in the TechReference but belong to the model (written by hand).
///
/// - `xmp:Label`: not IPTC, kept for Adobe/Photo Mechanic compatibility (ADR 0007 §3).
/// - `xmpRights:Marked` ("Copyrighted" in Lightroom and Photo Mechanic): not IPTC; a metadata
///   template sets it with its Copyrighted box (ADR 0007 §10, §15; owner decision D6; ADR 0017
///   U13). Text, as ExifTool reads and writes it (`True`/`False`).
/// - The deprecated IIM datasets Urgency (2:10), Category (2:15) and Supplemental Categories
///   (2:20) with their `photoshop:` XMP properties: kept and synchronised, never offered for
///   editing (ADR 0007 §5.5). Their value kinds follow the Adobe XMP specification
///   (`photoshop:Urgency` Integer, the others Text); their IIM names are the IIM 4.2 dataset names.
static EXTRA: &[PropertyDef] = &[
    PropertyDef {
        key: "label",
        name: "Label",
        schema: Schema::Other,
        kind: ValueKind::Text,
        occurrence: Occurrence::Single,
        xmp_id: "xmp:Label",
        iim: None,
        iim_name: None,
        exif_id: None,
        et_xmp: Some("XMP-xmp:Label"),
        et_iim: None,
        et_exif: None,
    },
    PropertyDef {
        key: "marked",
        name: "Marked",
        schema: Schema::Other,
        kind: ValueKind::Text,
        occurrence: Occurrence::Single,
        xmp_id: "xmpRights:Marked",
        iim: None,
        iim_name: None,
        exif_id: None,
        et_xmp: Some("XMP-xmpRights:Marked"),
        et_iim: None,
        et_exif: None,
    },
    PropertyDef {
        key: "urgency",
        name: "Urgency",
        schema: Schema::Other,
        kind: ValueKind::Integer,
        occurrence: Occurrence::Single,
        xmp_id: "photoshop:Urgency",
        iim: Some(IimDataset {
            record: 2,
            dataset: 10,
            max_bytes: Some(1),
        }),
        iim_name: Some("Urgency"),
        exif_id: None,
        et_xmp: Some("XMP-photoshop:Urgency"),
        et_iim: Some("IPTC:Urgency"),
        et_exif: None,
    },
    PropertyDef {
        key: "category",
        name: "Category",
        schema: Schema::Other,
        kind: ValueKind::Text,
        occurrence: Occurrence::Single,
        xmp_id: "photoshop:Category",
        iim: Some(IimDataset {
            record: 2,
            dataset: 15,
            max_bytes: Some(3),
        }),
        iim_name: Some("Category"),
        exif_id: None,
        et_xmp: Some("XMP-photoshop:Category"),
        et_iim: Some("IPTC:Category"),
        et_exif: None,
    },
    PropertyDef {
        key: "supplementalCategories",
        name: "Supplemental Categories",
        schema: Schema::Other,
        kind: ValueKind::Text,
        occurrence: Occurrence::Multi,
        xmp_id: "photoshop:SupplementalCategories",
        iim: Some(IimDataset {
            record: 2,
            dataset: 20,
            max_bytes: Some(32),
        }),
        iim_name: Some("Supplemental Category"),
        exif_id: None,
        et_xmp: Some("XMP-photoshop:SupplementalCategories"),
        et_iim: Some("IPTC:SupplementalCategories"),
        et_exif: None,
    },
];

/// Every property: the generated table in `sortorder`, then the hand-written extra entries.
pub fn properties() -> impl Iterator<Item = &'static PropertyDef> {
    TOP.iter().chain(EXTRA.iter())
}

/// The property with this XMP id (e.g. `photoshop:City`).
pub fn property(xmp_id: &str) -> Option<&'static PropertyDef> {
    properties().find(|def| def.xmp_id == xmp_id)
}

/// The property stored in this IIM dataset (e.g. 2, 80 → `dc:creator`).
pub fn by_iim(record: u8, dataset: u8) -> Option<&'static PropertyDef> {
    properties().find(|def| {
        def.iim
            .is_some_and(|iim| iim.record == record && iim.dataset == dataset)
    })
}

/// The structure with this name (the `name` of `ValueKind::Struct`).
pub fn struct_def(name: &str) -> Option<&'static StructDef> {
    STRUCTS.iter().find(|def| def.name == name)
}

/// The XMP array type of a property: `None` for a `Single` (or unknown) property.
///
/// ADR 0007 §3 (value types follow IPTC §7 and §11): `Seq` for `dc:creator` and the PLUS lists
/// Image Creator, Image Supplier and Copyright Owner; every other `Multi` property is a `Bag`. The
/// TechReference only says `multi`, so the order rule is kept here. Checked on 2026-11-11 against
/// the "XMP Specs" value type of every `Multi` property in IPTC Photo Metadata 2025.1 (§7, §11):
/// exactly these four are `Seq`.
pub fn list_kind(xmp_id: &str) -> Option<ListKind> {
    let def = property(xmp_id)?;
    match def.occurrence {
        Occurrence::Single => None,
        Occurrence::Multi => match xmp_id {
            "dc:creator" | "plus:ImageCreator" | "plus:ImageSupplier" | "plus:CopyrightOwner" => {
                Some(ListKind::Seq)
            }
            _ => Some(ListKind::Bag),
        },
    }
}

/// The XMP array type of a structure field: `None` for an unknown structure or field and for a
/// `Single` field.
///
/// ADR 0007 §3 (value types follow IPTC §7 and §11): `Seq` for Creator and Creator ID of an
/// artwork or object (`ArtworkOrObject`) and the polygon vertices of a region boundary
/// (`RegionBoundary`); every other `Multi` field is a `Bag`. [`list_kind`] covers top-level
/// properties only. Checked on 2026-11-11 against the "XMP Specs" value type of every `Multi`
/// field of every structure in IPTC Photo Metadata 2025.1: exactly these three are `Seq`.
pub fn field_list_kind(struct_name: &str, field_xmp_id: &str) -> Option<ListKind> {
    let field = struct_def(struct_name)?
        .fields
        .iter()
        .find(|field| field.xmp_id == field_xmp_id)?;
    match field.occurrence {
        Occurrence::Single => None,
        Occurrence::Multi => match (struct_name, field_xmp_id) {
            ("ArtworkOrObject", "Iptc4xmpExt:AOCreator" | "Iptc4xmpExt:AOCreatorId")
            | ("RegionBoundary", "Iptc4xmpExt:rbVertices") => Some(ListKind::Seq),
            _ => Some(ListKind::Bag),
        },
    }
}

/// The most entries a list property may hold, where IPTC sets a limit (ADR 0007 §8,
/// "Cardinalities"; IPTC §11.9, §11.22, §11.24, §11.26): Image Creator, Copyright Owner and
/// Licensor 3, Image Supplier 1. `None` for every other property.
pub fn max_entries(xmp_id: &str) -> Option<usize> {
    match xmp_id {
        "plus:ImageCreator" | "plus:CopyrightOwner" | "plus:Licensor" => Some(3),
        "plus:ImageSupplier" => Some(1),
        _ => None,
    }
}

/// True for the deprecated IIM properties Urgency, Category and Supplemental Categories, which
/// are kept and synchronised but never offered for editing, created or removed (ADR 0007 §5.5).
pub fn is_deprecated_iim(xmp_id: &str) -> bool {
    matches!(
        xmp_id,
        "photoshop:Urgency" | "photoshop:Category" | "photoshop:SupplementalCategories"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn def(xmp_id: &str) -> &'static PropertyDef {
        match property(xmp_id) {
            Some(def) => def,
            None => panic!("{xmp_id} not found"),
        }
    }

    fn iim(record: u8, dataset: u8, max_bytes: Option<u16>) -> Option<IimDataset> {
        Some(IimDataset {
            record,
            dataset,
            max_bytes,
        })
    }

    #[test]
    fn table_has_the_counts_of_the_saved_json() {
        assert_eq!(TOP.len(), 66);
        assert_eq!(
            TOP.iter().filter(|def| def.schema == Schema::Core).count(),
            25
        );
        assert_eq!(
            TOP.iter()
                .filter(|def| def.schema == Schema::Extension)
                .count(),
            41
        );
        // The 19 `ipmd_struct` entries without `AltLang`.
        assert_eq!(STRUCTS.len(), 18);
        assert!(struct_def("AltLang").is_none());
    }

    #[test]
    fn lang_alt_is_read_from_struct_alt_lang() {
        for xmp_id in ["dc:description", "dc:rights", "dc:title"] {
            assert_eq!(def(xmp_id).kind, ValueKind::LangAlt, "{xmp_id}");
        }
        // The JSON marks Lang Alt 21 times: 8 properties and 13 structure fields.
        let top = TOP
            .iter()
            .filter(|def| def.kind == ValueKind::LangAlt)
            .count();
        let fields = STRUCTS
            .iter()
            .flat_map(|def| def.fields)
            .filter(|field| field.kind == ValueKind::LangAlt)
            .count();
        assert_eq!((top, fields), (8, 13));
    }

    #[test]
    fn all_iptc_core_properties_of_the_analysis() {
        use Occurrence::{Multi, Single};
        use ValueKind::{Date, LangAlt, Struct, Text};
        // Analysis 2026-11-04 §2: XMP id, kind, occurrence, IIM dataset and byte limit as in the
        // table. Creator's Contact Info has no IIM dataset in the TechReference (the analysis
        // names 2:118 from spec §8.1; the table is what is tested here).
        let expected: [(&str, ValueKind, Occurrence, Option<IimDataset>); 25] = [
            ("Iptc4xmpCore:AltTextAccessibility", LangAlt, Single, None),
            ("photoshop:City", Text, Single, iim(2, 90, Some(32))),
            ("dc:rights", LangAlt, Single, iim(2, 116, Some(128))),
            ("photoshop:Country", Text, Single, iim(2, 101, Some(64))),
            (
                "Iptc4xmpCore:CountryCode",
                Text,
                Single,
                iim(2, 100, Some(3)),
            ),
            ("dc:creator", Text, Multi, iim(2, 80, Some(32))),
            (
                "Iptc4xmpCore:CreatorContactInfo",
                Struct("CreatorContactInfo"),
                Single,
                None,
            ),
            (
                "photoshop:AuthorsPosition",
                Text,
                Single,
                iim(2, 85, Some(32)),
            ),
            ("photoshop:Credit", Text, Single, iim(2, 110, Some(32))),
            ("photoshop:DateCreated", Date, Single, iim(2, 55, None)),
            ("dc:description", LangAlt, Single, iim(2, 120, Some(2000))),
            (
                "photoshop:CaptionWriter",
                Text,
                Single,
                iim(2, 122, Some(32)),
            ),
            ("Iptc4xmpCore:ExtDescrAccessibility", LangAlt, Single, None),
            ("photoshop:Headline", Text, Single, iim(2, 105, Some(256))),
            (
                "photoshop:Instructions",
                Text,
                Single,
                iim(2, 40, Some(256)),
            ),
            (
                "Iptc4xmpCore:IntellectualGenre",
                Text,
                Single,
                iim(2, 4, Some(64)),
            ),
            (
                "photoshop:TransmissionReference",
                Text,
                Single,
                iim(2, 103, Some(32)),
            ),
            ("dc:subject", Text, Multi, iim(2, 25, Some(64))),
            ("photoshop:State", Text, Single, iim(2, 95, Some(32))),
            ("xmpRights:UsageTerms", LangAlt, Single, None),
            ("Iptc4xmpCore:Scene", Text, Multi, None),
            ("photoshop:Source", Text, Single, iim(2, 115, Some(32))),
            (
                "Iptc4xmpCore:SubjectCode",
                Text,
                Multi,
                iim(2, 12, Some(236)),
            ),
            ("Iptc4xmpCore:Location", Text, Single, iim(2, 92, Some(32))),
            ("dc:title", LangAlt, Single, iim(2, 5, Some(64))),
        ];
        for (xmp_id, kind, occurrence, iim) in expected {
            let def = def(xmp_id);
            assert_eq!(def.schema, Schema::Core, "{xmp_id}");
            assert_eq!(def.kind, kind, "{xmp_id}");
            assert_eq!(def.occurrence, occurrence, "{xmp_id}");
            assert_eq!(def.iim, iim, "{xmp_id}");
        }
        let core: HashSet<&str> = TOP
            .iter()
            .filter(|def| def.schema == Schema::Core)
            .map(|def| def.xmp_id)
            .collect();
        let listed: HashSet<&str> = expected.iter().map(|(xmp_id, ..)| *xmp_id).collect();
        assert_eq!(core, listed);
    }

    #[test]
    fn date_created_names_its_iim_and_exif_tags() {
        let def = def("photoshop:DateCreated");
        assert_eq!(def.kind, ValueKind::Date);
        assert_eq!(def.iim, iim(2, 55, None));
        assert_eq!(def.et_iim, Some("IPTC:DateCreated+IPTC:TimeCreated"));
        assert_eq!(def.exif_id, Some("36867"));
        assert!(
            def.et_exif
                .is_some_and(|tags| tags.contains("ExifIFD:DateTimeOriginal")),
            "{:?}",
            def.et_exif
        );
    }

    #[test]
    fn structures_creator_contact_info_and_location() {
        assert_eq!(
            def("Iptc4xmpCore:CreatorContactInfo").kind,
            ValueKind::Struct("CreatorContactInfo")
        );
        let Some(contact) = struct_def("CreatorContactInfo") else {
            panic!("CreatorContactInfo not found");
        };
        let mut fields: Vec<&str> = contact.fields.iter().map(|field| field.xmp_id).collect();
        fields.sort_unstable();
        let mut expected = vec![
            "Iptc4xmpCore:CiAdrExtadr",
            "Iptc4xmpCore:CiAdrCity",
            "Iptc4xmpCore:CiAdrRegion",
            "Iptc4xmpCore:CiAdrPcode",
            "Iptc4xmpCore:CiAdrCtry",
            "Iptc4xmpCore:CiEmailWork",
            "Iptc4xmpCore:CiTelWork",
            "Iptc4xmpCore:CiUrlWork",
        ];
        expected.sort_unstable();
        assert_eq!(fields, expected);

        let location = def("Iptc4xmpExt:LocationCreated");
        assert_eq!(location.kind, ValueKind::Struct("Location"));
        assert_eq!(location.occurrence, Occurrence::Multi);
    }

    #[test]
    fn every_struct_kind_names_an_existing_structure() {
        let top = TOP.iter().map(|def| (def.xmp_id, def.kind));
        let fields = STRUCTS
            .iter()
            .flat_map(|def| def.fields)
            .map(|field| (field.xmp_id, field.kind));
        for (xmp_id, kind) in top.chain(fields) {
            if let ValueKind::Struct(name) = kind {
                assert!(struct_def(name).is_some(), "{xmp_id}: {name}");
            }
        }
    }

    #[test]
    fn xmp_ids_and_iim_datasets_are_unique() {
        let mut xmp_ids = HashSet::new();
        let mut datasets = HashSet::new();
        for def in properties() {
            assert!(xmp_ids.insert(def.xmp_id), "XMP id twice: {}", def.xmp_id);
            if let Some(iim) = def.iim {
                assert!(
                    datasets.insert((iim.record, iim.dataset)),
                    "IIM dataset twice: {}",
                    def.xmp_id
                );
            }
        }
        // 66 from the TechReference; `xmp:Label`, `xmpRights:Marked` (D6) and the three
        // deprecated IIM properties by hand.
        assert_eq!(xmp_ids.len(), 66 + 5);
    }

    #[test]
    fn rating_and_label_are_found() {
        let rating = def("xmp:Rating");
        assert_eq!(rating.name, "Image Rating");
        assert_eq!(rating.schema, Schema::Extension);
        let label = def("xmp:Label");
        assert_eq!(label.schema, Schema::Other);
        assert_eq!(label.kind, ValueKind::Text);
        assert_eq!(label.occurrence, Occurrence::Single);
        assert_eq!(label.et_xmp, Some("XMP-xmp:Label"));
        assert_eq!(label.iim, None);
    }

    /// D6: `xmpRights:Marked` is a hand-written non-IPTC text, XMP only, like `xmp:Label`.
    #[test]
    fn rights_marked_is_found() {
        let marked = def("xmpRights:Marked");
        assert_eq!(marked.schema, Schema::Other);
        assert_eq!(marked.kind, ValueKind::Text);
        assert_eq!(marked.occurrence, Occurrence::Single);
        assert_eq!(marked.et_xmp, Some("XMP-xmpRights:Marked"));
        assert_eq!(marked.iim, None);
        assert_eq!(marked.et_iim, None);
        assert_eq!(marked.exif_id, None);
        assert_eq!(marked.et_exif, None);
        assert!(!is_deprecated_iim("xmpRights:Marked"));
        assert_eq!(max_entries("xmpRights:Marked"), None);
    }

    #[test]
    fn deprecated_iim_properties() {
        let expected = [
            (
                "photoshop:Urgency",
                iim(2, 10, Some(1)),
                Occurrence::Single,
                "XMP-photoshop:Urgency",
                "IPTC:Urgency",
            ),
            (
                "photoshop:Category",
                iim(2, 15, Some(3)),
                Occurrence::Single,
                "XMP-photoshop:Category",
                "IPTC:Category",
            ),
            (
                "photoshop:SupplementalCategories",
                iim(2, 20, Some(32)),
                Occurrence::Multi,
                "XMP-photoshop:SupplementalCategories",
                "IPTC:SupplementalCategories",
            ),
        ];
        for (xmp_id, iim, occurrence, et_xmp, et_iim) in expected {
            let def = def(xmp_id);
            assert_eq!(def.schema, Schema::Other, "{xmp_id}");
            assert_eq!(def.iim, iim, "{xmp_id}");
            assert_eq!(def.occurrence, occurrence, "{xmp_id}");
            assert_eq!(def.et_xmp, Some(et_xmp), "{xmp_id}");
            assert_eq!(def.et_iim, Some(et_iim), "{xmp_id}");
            assert!(is_deprecated_iim(xmp_id), "{xmp_id}");
        }
        assert_eq!(
            list_kind("photoshop:SupplementalCategories"),
            Some(ListKind::Bag)
        );
        assert!(!is_deprecated_iim("xmp:Label"));
        assert!(!is_deprecated_iim("photoshop:City"));
        assert_eq!(
            properties()
                .filter(|def| is_deprecated_iim(def.xmp_id))
                .count(),
            3
        );
    }

    #[test]
    fn list_kinds() {
        assert_eq!(list_kind("dc:creator"), Some(ListKind::Seq));
        assert_eq!(list_kind("dc:subject"), Some(ListKind::Bag));
        assert_eq!(list_kind("plus:Licensor"), Some(ListKind::Bag));
        assert_eq!(list_kind("photoshop:City"), None);
        for xmp_id in [
            "plus:ImageCreator",
            "plus:ImageSupplier",
            "plus:CopyrightOwner",
        ] {
            assert_eq!(list_kind(xmp_id), Some(ListKind::Seq), "{xmp_id}");
        }
        assert_eq!(list_kind("no:Such"), None);
        // Every `Multi` property has a list kind, every `Single` one none.
        for def in properties() {
            assert_eq!(
                list_kind(def.xmp_id).is_some(),
                def.occurrence == Occurrence::Multi,
                "{}",
                def.xmp_id
            );
        }
    }

    #[test]
    fn field_list_kinds() {
        assert_eq!(
            field_list_kind("ArtworkOrObject", "Iptc4xmpExt:AOCreator"),
            Some(ListKind::Seq)
        );
        assert_eq!(
            field_list_kind("ArtworkOrObject", "Iptc4xmpExt:AOCreatorId"),
            Some(ListKind::Seq)
        );
        assert_eq!(
            field_list_kind("RegionBoundary", "Iptc4xmpExt:rbVertices"),
            Some(ListKind::Seq)
        );
        assert_eq!(
            field_list_kind("PersonWDetails", "Iptc4xmpExt:PersonId"),
            Some(ListKind::Bag)
        );
        assert_eq!(field_list_kind("Location", "Iptc4xmpExt:City"), None);
        assert_eq!(field_list_kind("Location", "no:Such"), None);
        assert_eq!(field_list_kind("NoSuchStruct", "Iptc4xmpExt:City"), None);
        // Every `Multi` field of every structure has a list kind, every `Single` one none; exactly
        // three are `Seq`.
        let mut seq = 0;
        for def in STRUCTS {
            for field in def.fields {
                let kind = field_list_kind(def.name, field.xmp_id);
                assert_eq!(
                    kind.is_some(),
                    field.occurrence == Occurrence::Multi,
                    "{}/{}",
                    def.name,
                    field.xmp_id
                );
                if kind == Some(ListKind::Seq) {
                    seq += 1;
                }
            }
        }
        assert_eq!(seq, 3);
    }

    #[test]
    fn max_entries_and_iim_lookup() {
        assert_eq!(max_entries("plus:ImageCreator"), Some(3));
        assert_eq!(max_entries("plus:CopyrightOwner"), Some(3));
        assert_eq!(max_entries("plus:Licensor"), Some(3));
        assert_eq!(max_entries("plus:ImageSupplier"), Some(1));
        assert_eq!(max_entries("dc:creator"), None);
        assert_eq!(max_entries("Iptc4xmpExt:LocationCreated"), None);

        assert_eq!(by_iim(2, 80).map(|def| def.xmp_id), Some("dc:creator"));
        assert_eq!(
            by_iim(2, 10).map(|def| def.xmp_id),
            Some("photoshop:Urgency")
        );
        assert!(by_iim(2, 0).is_none());
    }
}
