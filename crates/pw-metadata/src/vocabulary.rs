//! The controlled vocabularies of the editor's choice lists (ADR 0007 §11, I11; ADR 0017 U10d,
//! T128): for each choice-list property its XMP id and its allowed values, in the vocabulary's
//! order. Values only; the labels the user sees are made in the editor (rule 13).
//!
//! **The values are the form RawStride reads and writes through ExifTool:** for the four PLUS
//! properties the code alone (`MR-NAP`, `AG-A25`, …). The file holds [`PLUS_PREFIX`] followed by
//! the code, but ExifTool removes that prefix when it reads and adds it when it writes, so an edit
//! with the full URI would store the prefix twice. For Digital Source Type the full URI
//! ([`DIGITAL_SOURCE_TYPE_PREFIX`] followed by the code), which ExifTool does not convert.
//!
//! Written by hand from the reference copies in `D:\PhotoWorkflowData\references\` (R1, R2):
//! - `LDFXMP-1_2_0-ControlledVocabulary.xml` (SHA-256
//!   `26961ebed377caf0b347ed6a703f7a0ce80c10974c017490f34ac3bd08c92fa2`): Model Release Status,
//!   Property Release Status, Minor Model Age Disclosure;
//! - `README.md` of that folder, section "PLUS controlled vocabulary": Data Mining, which is not
//!   in the LDF file (it came with PLUS 2.0.1; taken from the PLUS XMP specification 2.0.1 page);
//! - `IPTC-NewsCodes-digitalsourcetype.json` (SHA-256
//!   `1caaa5bb729d5b40fdfef599bc650bf1288e7cca4fe52b9bb4d65701da4fb1d6`): Digital Source Type, the
//!   17 current codes and, apart, the 3 retired ones.
//!
//! The comparison of a value with a list is exact: no case folding, no trimming.

/// The start of every PLUS vocabulary URI; ExifTool removes it on read and adds it on write.
pub const PLUS_PREFIX: &str = "http://ns.useplus.org/ldf/vocab/";
/// The start of every Digital Source Type URI of the IPTC NewsCodes vocabulary.
pub const DIGITAL_SOURCE_TYPE_PREFIX: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/";

/// The XMP id of Model Release Status.
pub const MODEL_RELEASE_STATUS: &str = "plus:ModelReleaseStatus";
/// The XMP id of Property Release Status.
pub const PROPERTY_RELEASE_STATUS: &str = "plus:PropertyReleaseStatus";
/// The XMP id of Minor Model Age Disclosure.
pub const MINOR_MODEL_AGE_DISCLOSURE: &str = "plus:MinorModelAgeDisclosure";
/// The XMP id of Data Mining.
pub const DATA_MINING: &str = "plus:DataMining";
/// The XMP id of Digital Source Type.
pub const DIGITAL_SOURCE_TYPE: &str = "Iptc4xmpExt:DigitalSourceType";

/// The PLUS choice-list properties (the ones [`crate::validate`] checks against their list).
pub const PLUS_PROPERTIES: [&str; 4] = [
    MODEL_RELEASE_STATUS,
    PROPERTY_RELEASE_STATUS,
    MINOR_MODEL_AGE_DISCLOSURE,
    DATA_MINING,
];

/// Model Release Status (LDF file).
const MODEL_RELEASE_STATUS_CODES: [&str; 4] = ["MR-NON", "MR-NAP", "MR-UMR", "MR-LMR"];

/// Property Release Status (LDF file).
const PROPERTY_RELEASE_STATUS_CODES: [&str; 4] = ["PR-NON", "PR-NAP", "PR-UPR", "PR-LPR"];

/// Minor Model Age Disclosure (LDF file).
const MINOR_MODEL_AGE_DISCLOSURE_CODES: [&str; 13] = [
    "AG-UNK", "AG-A25", "AG-A24", "AG-A23", "AG-A22", "AG-A21", "AG-A20", "AG-A19", "AG-A18",
    "AG-A17", "AG-A16", "AG-A15", "AG-U14",
];

/// Data Mining (the references folder's `README.md`).
const DATA_MINING_CODES: [&str; 9] = [
    "DMI-UNSPECIFIED",
    "DMI-ALLOWED",
    "DMI-PROHIBITED-AIMLTRAINING",
    "DMI-PROHIBITED-GENAIMLTRAINING",
    "DMI-PROHIBITED-EXCEPTSEARCHENGINEINDEXING",
    "DMI-PROHIBITED",
    "DMI-PROHIBITED-SEECONSTRAINT",
    "DMI-PROHIBITED-SEEEMBEDDEDRIGHTSEXPR",
    "DMI-PROHIBITED-SEELINKEDRIGHTSEXPR",
];

/// Digital Source Type, the current codes in the file's order (NewsCodes file).
const DIGITAL_SOURCE_TYPE_URIS: [&str; 17] = [
    "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture",
    "http://cv.iptc.org/newscodes/digitalsourcetype/computationalCapture",
    "http://cv.iptc.org/newscodes/digitalsourcetype/negativeFilm",
    "http://cv.iptc.org/newscodes/digitalsourcetype/positiveFilm",
    "http://cv.iptc.org/newscodes/digitalsourcetype/print",
    "http://cv.iptc.org/newscodes/digitalsourcetype/humanEdits",
    "http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia",
    "http://cv.iptc.org/newscodes/digitalsourcetype/algorithmicallyEnhanced",
    "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCreation",
    "http://cv.iptc.org/newscodes/digitalsourcetype/dataDrivenMedia",
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia",
    "http://cv.iptc.org/newscodes/digitalsourcetype/algorithmicMedia",
    "http://cv.iptc.org/newscodes/digitalsourcetype/screenCapture",
    "http://cv.iptc.org/newscodes/digitalsourcetype/virtualRecording",
    "http://cv.iptc.org/newscodes/digitalsourcetype/composite",
    "http://cv.iptc.org/newscodes/digitalsourcetype/compositeCapture",
    "http://cv.iptc.org/newscodes/digitalsourcetype/compositeSynthetic",
];

/// The retired Digital Source Type codes (NewsCodes file): still valid in a file, not offered.
pub const RETIRED_DIGITAL_SOURCE_TYPE_URIS: [&str; 3] = [
    "http://cv.iptc.org/newscodes/digitalsourcetype/minorHumanEdits",
    "http://cv.iptc.org/newscodes/digitalsourcetype/softwareImage",
    "http://cv.iptc.org/newscodes/digitalsourcetype/digitalArt",
];

/// The allowed values of the choice-list property `xmp_id`, in the vocabulary's order: codes for
/// PLUS, full URIs for Digital Source Type (the current codes only); `None` for any other
/// property.
pub fn choices(xmp_id: &str) -> Option<&'static [&'static str]> {
    match xmp_id {
        MODEL_RELEASE_STATUS => Some(&MODEL_RELEASE_STATUS_CODES),
        PROPERTY_RELEASE_STATUS => Some(&PROPERTY_RELEASE_STATUS_CODES),
        MINOR_MODEL_AGE_DISCLOSURE => Some(&MINOR_MODEL_AGE_DISCLOSURE_CODES),
        DATA_MINING => Some(&DATA_MINING_CODES),
        DIGITAL_SOURCE_TYPE => Some(&DIGITAL_SOURCE_TYPE_URIS),
        _ => None,
    }
}

/// True when `value` is in the list of `xmp_id` ([`choices`]), compared exactly; false for a
/// property without a list.
pub fn is_choice(xmp_id: &str, value: &str) -> bool {
    choices(xmp_id).is_some_and(|values| values.contains(&value))
}

/// The full URI of `value`, a value of the choice-list property `xmp_id` (from [`choices`], or a
/// retired Digital Source Type): [`PLUS_PREFIX`] and the code for PLUS, the value itself for
/// Digital Source Type. `None` for a value that is in neither list.
pub fn full_uri(xmp_id: &str, value: &str) -> Option<String> {
    if is_choice(xmp_id, value) && PLUS_PROPERTIES.contains(&xmp_id) {
        return Some(format!("{PLUS_PREFIX}{value}"));
    }
    let digital_source_type = xmp_id == DIGITAL_SOURCE_TYPE
        && (is_choice(xmp_id, value) || is_retired_digital_source_type(value));
    digital_source_type.then(|| value.to_string())
}

/// True when `uri` is a retired Digital Source Type code, compared exactly.
pub fn is_retired_digital_source_type(uri: &str) -> bool {
    RETIRED_DIGITAL_SOURCE_TYPE_URIS.contains(&uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DST: &str = DIGITAL_SOURCE_TYPE_PREFIX;

    /// Each list: its count, its first and last value, and no duplicate; the PLUS lists hold codes
    /// without a prefix, the Digital Source Type list full URIs.
    #[test]
    fn each_list_has_its_count_ends_and_no_duplicate() {
        let cases: [(&str, usize, String, String); 5] = [
            (MODEL_RELEASE_STATUS, 4, "MR-NON".into(), "MR-LMR".into()),
            (PROPERTY_RELEASE_STATUS, 4, "PR-NON".into(), "PR-LPR".into()),
            (
                MINOR_MODEL_AGE_DISCLOSURE,
                13,
                "AG-UNK".into(),
                "AG-U14".into(),
            ),
            (
                DATA_MINING,
                9,
                "DMI-UNSPECIFIED".into(),
                "DMI-PROHIBITED-SEELINKEDRIGHTSEXPR".into(),
            ),
            (
                DIGITAL_SOURCE_TYPE,
                17,
                format!("{DST}digitalCapture"),
                format!("{DST}compositeSynthetic"),
            ),
        ];
        for (xmp_id, count, first, last) in cases {
            let values = choices(xmp_id).unwrap();
            assert_eq!(values.len(), count, "{xmp_id}");
            assert_eq!(values.first().copied(), Some(first.as_str()), "{xmp_id}");
            assert_eq!(values.last().copied(), Some(last.as_str()), "{xmp_id}");
            let mut unique = values.to_vec();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), values.len(), "{xmp_id}");
            if xmp_id == DIGITAL_SOURCE_TYPE {
                assert!(values.iter().all(|v| v.starts_with(DST)), "{xmp_id}");
            } else {
                assert!(
                    values
                        .iter()
                        .all(|v| !v.contains(':') && !v.contains('/') && !v.is_empty()),
                    "{xmp_id}"
                );
            }
        }
    }

    #[test]
    fn the_age_list_runs_from_25_down_to_15_between_unknown_and_14() {
        let values = choices(MINOR_MODEL_AGE_DISCLOSURE).unwrap();
        let mut expected = vec!["AG-UNK".to_string()];
        expected.extend((15..=25).rev().map(|age| format!("AG-A{age}")));
        expected.push("AG-U14".to_string());
        assert_eq!(values, expected);
    }

    #[test]
    fn the_retired_digital_source_types_are_apart_and_not_offered() {
        assert_eq!(RETIRED_DIGITAL_SOURCE_TYPE_URIS.len(), 3);
        for uri in RETIRED_DIGITAL_SOURCE_TYPE_URIS {
            assert!(uri.starts_with(DST), "{uri}");
            assert!(is_retired_digital_source_type(uri), "{uri}");
            assert!(!is_choice(DIGITAL_SOURCE_TYPE, uri), "{uri}");
        }
        assert!(!is_retired_digital_source_type(&format!(
            "{DST}digitalCapture"
        )));
        assert!(!is_retired_digital_source_type(&format!("{DST}DigitalArt")));
    }

    #[test]
    fn membership_is_exact() {
        assert!(is_choice(MODEL_RELEASE_STATUS, "MR-NAP"));
        // Another case, a trailing or leading space, the full PLUS URI: not in the list.
        for value in [
            "mr-nap",
            "MR-NAP ",
            " MR-NAP",
            "http://ns.useplus.org/ldf/vocab/MR-NAP",
            "",
        ] {
            assert!(!is_choice(MODEL_RELEASE_STATUS, value), "{value:?}");
        }
        // A value of another list.
        assert!(!is_choice(PROPERTY_RELEASE_STATUS, "MR-NAP"));
        // Digital Source Type: the full URI only, exact.
        assert!(is_choice(
            DIGITAL_SOURCE_TYPE,
            &format!("{DST}digitalCapture")
        ));
        for value in [
            "https://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture".to_string(),
            format!("{DST}digitalcapture"),
            format!("{DST}digitalCapture "),
            "digitalCapture".to_string(),
        ] {
            assert!(!is_choice(DIGITAL_SOURCE_TYPE, &value), "{value:?}");
        }
    }

    #[test]
    fn the_full_uri_of_a_value() {
        assert_eq!(
            full_uri(MODEL_RELEASE_STATUS, "MR-NAP").as_deref(),
            Some("http://ns.useplus.org/ldf/vocab/MR-NAP")
        );
        assert_eq!(
            full_uri(DATA_MINING, "DMI-PROHIBITED-AIMLTRAINING").as_deref(),
            Some("http://ns.useplus.org/ldf/vocab/DMI-PROHIBITED-AIMLTRAINING")
        );
        let capture = format!("{DST}digitalCapture");
        assert_eq!(
            full_uri(DIGITAL_SOURCE_TYPE, &capture).as_deref(),
            Some(capture.as_str())
        );
        let retired = format!("{DST}digitalArt");
        assert_eq!(
            full_uri(DIGITAL_SOURCE_TYPE, &retired).as_deref(),
            Some(retired.as_str())
        );
        // Every value of every list has one.
        for xmp_id in PLUS_PROPERTIES
            .into_iter()
            .chain(std::iter::once(DIGITAL_SOURCE_TYPE))
        {
            for value in choices(xmp_id).unwrap() {
                assert!(full_uri(xmp_id, value).is_some(), "{xmp_id} {value}");
            }
        }
        // A value outside the list, or of another list, has none.
        assert_eq!(full_uri(MODEL_RELEASE_STATUS, "XX"), None);
        assert_eq!(full_uri(PROPERTY_RELEASE_STATUS, "MR-NAP"), None);
        assert_eq!(full_uri(MINOR_MODEL_AGE_DISCLOSURE, &retired), None);
        assert_eq!(full_uri("photoshop:City", "MR-NAP"), None);
    }

    #[test]
    fn a_property_without_a_list_has_none() {
        assert_eq!(choices("plus:ModelReleaseID"), None);
        assert_eq!(choices("photoshop:City"), None);
        assert!(!is_choice("photoshop:City", "MR-NAP"));
    }

    #[test]
    fn the_choice_list_properties_are_in_the_property_table() {
        for xmp_id in PLUS_PROPERTIES
            .into_iter()
            .chain(std::iter::once(DIGITAL_SOURCE_TYPE))
        {
            assert!(choices(xmp_id).is_some(), "{xmp_id}");
            assert!(crate::properties::property(xmp_id).is_some(), "{xmp_id}");
        }
    }
}
