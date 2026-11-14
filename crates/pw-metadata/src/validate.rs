//! Validation of edited metadata (ADR 0007 §11): warnings, never refusals.
//!
//! [`validate`] looks at every value of a [`Metadata`] and returns a [`Warning`] for each value
//! that breaks a rule of §11; nothing is changed and nothing is refused. The editor shows the
//! warnings (T104, with the Swedish texts); the [`fmt::Display`] texts here are English.
//!
//! The rules checked (§11):
//! - **Lengths:** a value whose IIM form would be cut to the IIM byte limit (§5.2), only when IIM
//!   is written (the file has IIM, or the setting creates it); Alt Text (Accessibility) above 250
//!   characters [IPTC §7.1].
//! - **Country Code:** 2 or 3 ASCII letters (ISO 3166-1, the scheme from the length) [IPTC §7.5].
//!   Whether the code is in ISO 3166-1 is not checked.
//! - **Creator's Jobtitle** without a Creator [User Guide; §8].
//! - **Digital Source Type:** a URI of the IPTC NewsCodes vocabulary [IPTC §11.13].
//! - **PLUS choice lists** (I11, T128): Model Release Status, Property Release Status, Minor Model
//!   Age Disclosure and Data Mining, one of the codes of their PLUS vocabulary ([`vocabulary`],
//!   exact; ExifTool gives the code without the URI's prefix).
//! - **Image Registry Entry:** both Item Id and Organisation Id [IPTC §11.21].
//! - **Rating:** a number from −1 to 5; a real number inside the range is fine (§8).
//!
//! A blank value (only spaces and/or NUL, [`iim::is_blank`]) counts as missing (§5.1).
//!
//! Left out here:
//! - the controlled vocabularies other than Digital Source Type and the PLUS choice lists;
//! - dates: the editor checks a date when it turns a text into a date (T104).

use std::fmt;

use crate::iim;
use crate::iim_read;
use crate::properties::{self, IimDataset};
use crate::value::{Metadata, StructValue, Value};
use crate::vocabulary;

const ALT_TEXT: &str = "Iptc4xmpCore:AltTextAccessibility";
const COUNTRY_CODE: &str = "Iptc4xmpCore:CountryCode";
const CREATOR: &str = "dc:creator";
const JOBTITLE: &str = "photoshop:AuthorsPosition";
const DIGITAL_SOURCE_TYPE: &str = "Iptc4xmpExt:DigitalSourceType";
const REGISTRY: &str = "Iptc4xmpExt:RegistryId";
const REG_ITEM_ID: &str = "Iptc4xmpExt:RegItemId";
const REG_ORG_ID: &str = "Iptc4xmpExt:RegOrgId";
const RATING: &str = "xmp:Rating";

/// The most characters of Alt Text (Accessibility) without a warning [IPTC §7.1].
const ALT_TEXT_MAX_CHARACTERS: usize = 250;

/// The start of every Digital Source Type URI of the IPTC NewsCodes vocabulary [IPTC §11.13].
const DIGITAL_SOURCE_TYPE_PREFIX: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/";

/// The rating range (§8): −1 rejected, 0 unrated, 1–5 stars.
const RATING_MIN: f64 = -1.0;
const RATING_MAX: f64 = 5.0;

/// A rule of ADR 0007 §11 that a value breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The IIM form of a value of `property` (XMP id) would be cut to `max_bytes` (§5.2). For a
    /// Lang Alt the shown text is checked, for a list each entry; the property is named once.
    TooLongForIim { property: String, max_bytes: u16 },
    /// Alt Text (Accessibility) has more than 250 characters (the shown text) [IPTC §7.1].
    AltTextTooLong { characters: usize },
    /// The Country Code is not 2 or 3 ASCII letters [IPTC §7.5].
    CountryCodeForm { code: String },
    /// Creator's Jobtitle is set but there is no Creator [User Guide].
    JobtitleWithoutCreator,
    /// The Digital Source Type is not a URI of the IPTC NewsCodes vocabulary [IPTC §11.13].
    DigitalSourceTypeUnknown { uri: String },
    /// The Image Registry Entry at `index` (from 0) lacks its Item Id or its Organisation Id
    /// [IPTC §11.21].
    RegistryEntryIncomplete { index: usize },
    /// The rating is not a number from −1 to 5 (§8); `value` is the text as stored.
    RatingOutOfRange { value: String },
    /// The value of the PLUS choice-list property `property` (XMP id) is not one of the codes of
    /// its PLUS vocabulary (I11, T128); `value` is the text as read. Kept, never refused.
    VocabularyValueUnknown { property: String, value: String },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::TooLongForIim {
                property,
                max_bytes,
            } => write!(
                f,
                "{property}: longer than the IIM limit of {max_bytes} bytes; the IIM value will be cut"
            ),
            Warning::AltTextTooLong { characters } => write!(
                f,
                "Alt Text has {characters} characters; at most {ALT_TEXT_MAX_CHARACTERS} are recommended"
            ),
            Warning::CountryCodeForm { code } => write!(
                f,
                "Country Code \"{code}\" is not an ISO 3166-1 code of 2 or 3 letters"
            ),
            Warning::JobtitleWithoutCreator => {
                write!(f, "Creator's Jobtitle is set but there is no Creator")
            }
            Warning::DigitalSourceTypeUnknown { uri } => write!(
                f,
                "Digital Source Type \"{uri}\" is not in the IPTC Digital Source Type vocabulary"
            ),
            Warning::RegistryEntryIncomplete { index } => write!(
                f,
                "Image Registry Entry {} lacks its Item Id or its Organisation Id",
                index + 1
            ),
            Warning::RatingOutOfRange { value } => {
                write!(f, "Rating \"{value}\" is not a number from -1 to 5")
            }
            Warning::VocabularyValueUnknown { property, value } => write!(
                f,
                "{property}: \"{value}\" is not in the PLUS controlled vocabulary"
            ),
        }
    }
}

/// The warnings of every value in `metadata`, the properties in table order
/// ([`properties::properties`]). `TooLongForIim` only when `iim_written`: the file has IIM, or the
/// setting creates it.
pub fn validate(metadata: &Metadata, iim_written: bool) -> Vec<Warning> {
    let mut warnings = Vec::new();
    for def in properties::properties() {
        let Some(stored) = metadata.get(def.xmp_id) else {
            continue;
        };
        let value = &stored.value;
        if iim_written
            && let Some(IimDataset {
                max_bytes: Some(max_bytes),
                ..
            }) = def.iim
            && too_long_for_iim(def.xmp_id, value, max_bytes)
        {
            warnings.push(Warning::TooLongForIim {
                property: def.xmp_id.to_string(),
                max_bytes,
            });
        }
        let warning = match def.xmp_id {
            ALT_TEXT => alt_text_warning(value),
            COUNTRY_CODE => country_code_warning(value),
            JOBTITLE => jobtitle_warning(metadata, value),
            DIGITAL_SOURCE_TYPE => digital_source_type_warning(value),
            id if vocabulary::PLUS_PROPERTIES.contains(&id) => vocabulary_warning(id, value),
            RATING => match value {
                Value::Number(text) => rating_warning(text),
                _ => None,
            },
            REGISTRY => {
                if let Value::StructList(entries) = value {
                    warnings.extend(
                        entries
                            .iter()
                            .enumerate()
                            .filter(|(_, entry)| registry_entry_incomplete(entry))
                            .map(|(index, _)| Warning::RegistryEntryIncomplete { index }),
                    );
                }
                None
            }
            _ => None,
        };
        warnings.extend(warning);
    }
    warnings
}

/// The texts whose IIM form is checked: a text, the shown text of a Lang Alt, each entry of a
/// list. Other values have no IIM text here (dates are left out, see the module comment).
fn iim_texts(value: &Value) -> Vec<&str> {
    match value {
        Value::Text(text) => vec![text.as_str()],
        Value::LangAlt(lang_alt) => lang_alt
            .display()
            .map(|(text, _)| text)
            .into_iter()
            .collect(),
        Value::List(entries) => entries.iter().map(String::as_str).collect(),
        _ => Vec::new(),
    }
}

/// True when the IIM form of any text of `value` would be cut to `max_bytes`
/// ([`iim::predict`] with the property's [`iim_read::iim_format`]).
fn too_long_for_iim(xmp_id: &str, value: &Value, max_bytes: u16) -> bool {
    let format = iim_read::iim_format(xmp_id);
    iim_texts(value)
        .into_iter()
        .any(|text| iim::predict(text, format, Some(max_bytes)).truncated)
}

/// The text of a `Text` value that is not blank.
fn present_text(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) if !iim::is_blank(text) => Some(text),
        _ => None,
    }
}

fn alt_text_warning(value: &Value) -> Option<Warning> {
    let Value::LangAlt(lang_alt) = value else {
        return None;
    };
    let (text, _) = lang_alt.display()?;
    let characters = text.chars().count();
    (characters > ALT_TEXT_MAX_CHARACTERS).then_some(Warning::AltTextTooLong { characters })
}

fn country_code_warning(value: &Value) -> Option<Warning> {
    let code = present_text(value)?;
    let well_formed =
        matches!(code.len(), 2 | 3) && code.bytes().all(|byte| byte.is_ascii_alphabetic());
    (!well_formed).then(|| Warning::CountryCodeForm {
        code: code.to_string(),
    })
}

/// `JobtitleWithoutCreator` when the jobtitle is not blank and `dc:creator` is missing or has no
/// entry that is not blank.
fn jobtitle_warning(metadata: &Metadata, jobtitle: &Value) -> Option<Warning> {
    present_text(jobtitle)?;
    let has_creator = metadata
        .get(CREATOR)
        .is_some_and(|stored| match &stored.value {
            Value::List(entries) => entries.iter().any(|entry| !iim::is_blank(entry)),
            _ => false,
        });
    (!has_creator).then_some(Warning::JobtitleWithoutCreator)
}

/// `DigitalSourceTypeUnknown` unless the URI is the NewsCodes prefix followed by a code (one or
/// more ASCII letters or digits).
fn digital_source_type_warning(value: &Value) -> Option<Warning> {
    let uri = present_text(value)?;
    let known = uri
        .strip_prefix(DIGITAL_SOURCE_TYPE_PREFIX)
        .is_some_and(|code| {
            !code.is_empty() && code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    (!known).then(|| Warning::DigitalSourceTypeUnknown {
        uri: uri.to_string(),
    })
}

/// `VocabularyValueUnknown` when the text of the PLUS choice-list property `xmp_id` is not blank
/// and not in its vocabulary (exact comparison).
fn vocabulary_warning(xmp_id: &str, value: &Value) -> Option<Warning> {
    let text = present_text(value)?;
    (!vocabulary::is_choice(xmp_id, text)).then(|| Warning::VocabularyValueUnknown {
        property: xmp_id.to_string(),
        value: text.to_string(),
    })
}

/// True when the entry lacks `RegItemId` or `RegOrgId`; a blank text counts as missing.
fn registry_entry_incomplete(entry: &StructValue) -> bool {
    let has = |field_id: &str| {
        entry
            .fields
            .iter()
            .any(|(id, value)| id == field_id && present_text(value).is_some())
    };
    !(has(REG_ITEM_ID) && has(REG_ORG_ID))
}

/// `RatingOutOfRange` unless `text` is a finite number from −1 to 5 (§8: a real number inside is
/// fine).
fn rating_warning(text: &str) -> Option<Warning> {
    let in_range = text
        .trim()
        .parse::<f64>()
        .is_ok_and(|rating| rating.is_finite() && (RATING_MIN..=RATING_MAX).contains(&rating));
    (!in_range).then(|| Warning::RatingOutOfRange {
        value: text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{LangAlt, Origin, ValueError};

    const CITY: &str = "photoshop:City";
    const KEYWORDS: &str = "dc:subject";

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    fn list(entries: &[&str]) -> Value {
        Value::List(entries.iter().map(|entry| entry.to_string()).collect())
    }

    fn lang_alt(text: &str) -> Value {
        Value::LangAlt(LangAlt::from_entries(vec![(
            "x-default".to_string(),
            text.to_string(),
        )]))
    }

    fn registry_entry(item: Option<&str>, organisation: Option<&str>) -> StructValue {
        let mut fields = Vec::new();
        if let Some(item) = item {
            fields.push((REG_ITEM_ID.to_string(), text(item)));
        }
        if let Some(organisation) = organisation {
            fields.push((REG_ORG_ID.to_string(), text(organisation)));
        }
        StructValue { fields }
    }

    /// A `Metadata` holding `values` as read from embedded XMP.
    fn metadata(values: Vec<(&str, Value)>) -> Result<Metadata, ValueError> {
        let mut metadata = Metadata::new();
        for (xmp_id, value) in values {
            metadata.insert_read(xmp_id, value, Origin::XmpEmbedded)?;
        }
        Ok(metadata)
    }

    /// The warnings of one value, with and without IIM.
    fn warnings_of(
        xmp_id: &str,
        value: Value,
        iim_written: bool,
    ) -> Result<Vec<Warning>, ValueError> {
        Ok(validate(&metadata(vec![(xmp_id, value)])?, iim_written))
    }

    fn too_long(property: &str, max_bytes: u16) -> Warning {
        Warning::TooLongForIim {
            property: property.to_string(),
            max_bytes,
        }
    }

    /// The position of a property in the table.
    fn table_position(xmp_id: &str) -> Option<usize> {
        properties::properties().position(|def| def.xmp_id == xmp_id)
    }

    /// Every warning once, with its exact content, in table order. The values are inserted in
    /// another order so the result can only follow the table.
    #[test]
    fn every_warning_once_in_table_order() -> Result<(), ValueError> {
        let metadata = metadata(vec![
            (
                REGISTRY,
                Value::StructList(vec![registry_entry(Some("item"), None)]),
            ),
            (RATING, Value::Number("7".to_string())),
            (
                DIGITAL_SOURCE_TYPE,
                text("http://example.com/digitalCapture"),
            ),
            (JOBTITLE, text("Photographer")),
            (COUNTRY_CODE, text("Sverige")),
            (CITY, text("a".repeat(33).as_str())),
            (ALT_TEXT, lang_alt("x".repeat(251).as_str())),
        ])?;
        let expected = vec![
            Warning::AltTextTooLong { characters: 251 },
            too_long(CITY, 32),
            too_long(COUNTRY_CODE, 3),
            Warning::CountryCodeForm {
                code: "Sverige".to_string(),
            },
            Warning::JobtitleWithoutCreator,
            Warning::DigitalSourceTypeUnknown {
                uri: "http://example.com/digitalCapture".to_string(),
            },
            Warning::RatingOutOfRange {
                value: "7".to_string(),
            },
            Warning::RegistryEntryIncomplete { index: 0 },
        ];
        assert_eq!(validate(&metadata, true), expected);

        // The order above is the table's (the hand-written list must not drift from it).
        let order = [
            ALT_TEXT,
            CITY,
            COUNTRY_CODE,
            JOBTITLE,
            DIGITAL_SOURCE_TYPE,
            RATING,
            REGISTRY,
        ];
        let positions: Vec<Option<usize>> = order.iter().map(|id| table_position(id)).collect();
        assert!(positions.iter().all(Option::is_some), "{positions:?}");
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{positions:?}"
        );
        Ok(())
    }

    #[test]
    fn empty_metadata_gives_nothing() {
        assert_eq!(validate(&Metadata::new(), true), Vec::new());
        assert_eq!(validate(&Metadata::new(), false), Vec::new());
    }

    #[test]
    fn a_too_long_city_only_when_iim_is_written() -> Result<(), ValueError> {
        let city = "a".repeat(33);
        assert_eq!(
            warnings_of(CITY, text(&city), true)?,
            vec![too_long(CITY, 32)]
        );
        assert_eq!(warnings_of(CITY, text(&city), false)?, Vec::new());
        assert_eq!(warnings_of(CITY, text(&"a".repeat(32)), true)?, Vec::new());
        Ok(())
    }

    /// IIM limits count bytes: `ä` is 2 bytes in UTF-8.
    #[test]
    fn the_iim_limit_counts_bytes() -> Result<(), ValueError> {
        assert_eq!(warnings_of(CITY, text(&"ä".repeat(16)), true)?, Vec::new());
        assert_eq!(
            warnings_of(CITY, text(&"ä".repeat(17)), true)?,
            vec![too_long(CITY, 32)]
        );
        Ok(())
    }

    /// Each entry of a list is checked; the property is named once.
    #[test]
    fn a_keyword_list_with_one_long_entry_gives_one_warning() -> Result<(), ValueError> {
        let long = "k".repeat(65);
        assert_eq!(
            warnings_of(KEYWORDS, list(&["short", &long, "other"]), true)?,
            vec![too_long(KEYWORDS, 64)]
        );
        assert_eq!(
            warnings_of(KEYWORDS, list(&[&long, &long]), true)?,
            vec![too_long(KEYWORDS, 64)]
        );
        assert_eq!(
            warnings_of(KEYWORDS, list(&["short", &"k".repeat(64)]), true)?,
            Vec::new()
        );
        Ok(())
    }

    /// For a Lang Alt the shown text is checked (`dc:title`, 64 bytes).
    #[test]
    fn a_lang_alt_is_checked_on_its_shown_text() -> Result<(), ValueError> {
        let title = Value::LangAlt(LangAlt::from_entries(vec![
            ("x-default".to_string(), "short".to_string()),
            ("sv".to_string(), "t".repeat(65)),
        ]));
        assert_eq!(warnings_of("dc:title", title, true)?, Vec::new());
        assert_eq!(
            warnings_of("dc:title", lang_alt(&"t".repeat(65)), true)?,
            vec![too_long("dc:title", 64)]
        );
        Ok(())
    }

    /// Intellectual Genre's limit applies to the code only, not to the `000:` prefix (§5.3), as
    /// `iim::predict` gives it.
    #[test]
    fn the_iim_format_of_the_property_is_used() -> Result<(), ValueError> {
        let genre = "Iptc4xmpCore:IntellectualGenre";
        assert_eq!(warnings_of(genre, text(&"g".repeat(64)), true)?, Vec::new());
        assert_eq!(
            warnings_of(genre, text(&"g".repeat(65)), true)?,
            vec![too_long(genre, 64)]
        );
        Ok(())
    }

    #[test]
    fn alt_text_up_to_250_characters() -> Result<(), ValueError> {
        assert_eq!(
            warnings_of(ALT_TEXT, lang_alt(&"x".repeat(250)), false)?,
            Vec::new()
        );
        assert_eq!(
            warnings_of(ALT_TEXT, lang_alt(&"x".repeat(251)), false)?,
            vec![Warning::AltTextTooLong { characters: 251 }]
        );
        // Characters, not bytes: 250 × `ä` is 500 bytes and still fine.
        assert_eq!(
            warnings_of(ALT_TEXT, lang_alt(&"ä".repeat(250)), false)?,
            Vec::new()
        );
        Ok(())
    }

    #[test]
    fn country_codes_of_two_or_three_letters() -> Result<(), ValueError> {
        for code in ["SE", "SWE", "se"] {
            assert_eq!(
                warnings_of(COUNTRY_CODE, text(code), false)?,
                Vec::new(),
                "{code}"
            );
        }
        for code in ["S", "SVER", "S1", "Sverige"] {
            assert_eq!(
                warnings_of(COUNTRY_CODE, text(code), false)?,
                vec![Warning::CountryCodeForm {
                    code: code.to_string()
                }],
                "{code}"
            );
        }
        Ok(())
    }

    #[test]
    fn jobtitle_with_and_without_creator() -> Result<(), ValueError> {
        let with_creator = metadata(vec![
            (JOBTITLE, text("Photographer")),
            (CREATOR, list(&["Anna Andersson"])),
        ])?;
        assert_eq!(validate(&with_creator, false), Vec::new());
        assert_eq!(
            warnings_of(JOBTITLE, text("Photographer"), false)?,
            vec![Warning::JobtitleWithoutCreator]
        );
        // A Creator list of only blank entries is no Creator (§5.1).
        let blank_creator = metadata(vec![
            (JOBTITLE, text("Photographer")),
            (CREATOR, list(&[" "])),
        ])?;
        assert_eq!(
            validate(&blank_creator, false),
            vec![Warning::JobtitleWithoutCreator]
        );
        Ok(())
    }

    #[test]
    fn digital_source_type_from_the_newscodes() -> Result<(), ValueError> {
        let known = "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";
        assert_eq!(
            warnings_of(DIGITAL_SOURCE_TYPE, text(known), false)?,
            Vec::new()
        );
        for uri in [DIGITAL_SOURCE_TYPE_PREFIX, "digitalCapture"] {
            assert_eq!(
                warnings_of(DIGITAL_SOURCE_TYPE, text(uri), false)?,
                vec![Warning::DigitalSourceTypeUnknown {
                    uri: uri.to_string()
                }],
                "{uri}"
            );
        }
        Ok(())
    }

    /// The PLUS choice lists (I11, T128): an unknown value warns with its property and its text
    /// as read; a known code and a blank value give nothing.
    #[test]
    fn plus_choice_lists_warn_only_for_a_value_outside_the_vocabulary() -> Result<(), ValueError> {
        let cases = [
            (vocabulary::MODEL_RELEASE_STATUS, "MR-NAP"),
            (vocabulary::PROPERTY_RELEASE_STATUS, "PR-NON"),
            (vocabulary::MINOR_MODEL_AGE_DISCLOSURE, "AG-A25"),
            (vocabulary::DATA_MINING, "DMI-UNSPECIFIED"),
        ];
        for (property, known) in cases {
            assert_eq!(
                warnings_of(property, text(known), true)?,
                Vec::new(),
                "{property}"
            );
            for blank in ["", "  "] {
                assert_eq!(
                    warnings_of(property, text(blank), true)?,
                    Vec::new(),
                    "{property} {blank:?}"
                );
            }
            // An unknown code, another case, a trailing space, the full URI (ExifTool gives the
            // code alone, so a full URI in the read is not a known value).
            let unknown = [
                "XX-UNKNOWN".to_string(),
                known.to_lowercase(),
                format!("{known} "),
                format!("{}{known}", vocabulary::PLUS_PREFIX),
            ];
            for value in unknown {
                assert_eq!(
                    warnings_of(property, text(&value), true)?,
                    vec![Warning::VocabularyValueUnknown {
                        property: property.to_string(),
                        value: value.clone(),
                    }],
                    "{property} {value:?}"
                );
            }
        }
        // A code of another PLUS list is unknown here.
        assert_eq!(
            warnings_of(vocabulary::PROPERTY_RELEASE_STATUS, text("MR-NAP"), false)?,
            vec![Warning::VocabularyValueUnknown {
                property: vocabulary::PROPERTY_RELEASE_STATUS.to_string(),
                value: "MR-NAP".to_string(),
            }]
        );
        Ok(())
    }

    /// Digital Source Type keeps its own warning (T128 point 4): a retired code and a NewsCodes
    /// URI that is not in the list give nothing, and it never gives `VocabularyValueUnknown`.
    #[test]
    fn digital_source_type_keeps_its_own_warning() -> Result<(), ValueError> {
        for uri in [
            "http://cv.iptc.org/newscodes/digitalsourcetype/digitalArt",
            "http://cv.iptc.org/newscodes/digitalsourcetype/notACode",
        ] {
            assert_eq!(
                warnings_of(DIGITAL_SOURCE_TYPE, text(uri), false)?,
                Vec::new(),
                "{uri}"
            );
        }
        assert_eq!(
            warnings_of(DIGITAL_SOURCE_TYPE, text("http://example.com/x"), false)?,
            vec![Warning::DigitalSourceTypeUnknown {
                uri: "http://example.com/x".to_string()
            }]
        );
        Ok(())
    }

    #[test]
    fn a_registry_entry_without_organisation() -> Result<(), ValueError> {
        let entries = Value::StructList(vec![
            registry_entry(Some("item-1"), Some("org-1")),
            registry_entry(Some("item-2"), None),
        ]);
        assert_eq!(
            warnings_of(REGISTRY, entries, false)?,
            vec![Warning::RegistryEntryIncomplete { index: 1 }]
        );
        // A blank text counts as missing; so does a missing Item Id.
        let entries = Value::StructList(vec![
            registry_entry(Some("item-1"), Some(" ")),
            registry_entry(None, Some("org-2")),
            registry_entry(Some("item-3"), Some("org-3")),
        ]);
        assert_eq!(
            warnings_of(REGISTRY, entries, false)?,
            vec![
                Warning::RegistryEntryIncomplete { index: 0 },
                Warning::RegistryEntryIncomplete { index: 1 },
            ]
        );
        Ok(())
    }

    #[test]
    fn ratings_from_minus_one_to_five() -> Result<(), ValueError> {
        for rating in ["-1", "0", "2.5", "5"] {
            assert_eq!(
                warnings_of(RATING, Value::Number(rating.to_string()), false)?,
                Vec::new(),
                "{rating}"
            );
        }
        for rating in ["6", "-2"] {
            assert_eq!(
                warnings_of(RATING, Value::Number(rating.to_string()), false)?,
                vec![Warning::RatingOutOfRange {
                    value: rating.to_string()
                }],
                "{rating}"
            );
        }
        Ok(())
    }

    /// `abc` cannot be stored as `xmp:Rating` in a `Metadata` (its kind is Number, and
    /// `Metadata::insert_read` refuses it), so the check `validate` uses for a rating is tested
    /// directly (owner, 2026-11-12). Infinite and NaN values are not numbers in the range either.
    #[test]
    fn a_rating_that_is_not_a_number() {
        assert!(
            Metadata::new()
                .insert_read(
                    RATING,
                    Value::Number("abc".to_string()),
                    Origin::XmpEmbedded
                )
                .is_err()
        );
        for rating in ["abc", "inf", "NaN", ""] {
            assert_eq!(
                rating_warning(rating),
                Some(Warning::RatingOutOfRange {
                    value: rating.to_string()
                }),
                "{rating}"
            );
        }
        assert_eq!(rating_warning("2.5"), None);
    }

    #[test]
    fn the_english_texts() {
        let texts: Vec<String> = [
            too_long(CITY, 32),
            Warning::AltTextTooLong { characters: 251 },
            Warning::CountryCodeForm {
                code: "S1".to_string(),
            },
            Warning::JobtitleWithoutCreator,
            Warning::DigitalSourceTypeUnknown {
                uri: "digitalCapture".to_string(),
            },
            Warning::RegistryEntryIncomplete { index: 1 },
            Warning::RatingOutOfRange {
                value: "6".to_string(),
            },
        ]
        .iter()
        .map(Warning::to_string)
        .collect();
        assert_eq!(
            texts,
            [
                "photoshop:City: longer than the IIM limit of 32 bytes; the IIM value will be cut",
                "Alt Text has 251 characters; at most 250 are recommended",
                "Country Code \"S1\" is not an ISO 3166-1 code of 2 or 3 letters",
                "Creator's Jobtitle is set but there is no Creator",
                "Digital Source Type \"digitalCapture\" is not in the IPTC Digital Source Type vocabulary",
                "Image Registry Entry 2 lacks its Item Id or its Organisation Id",
                "Rating \"6\" is not a number from -1 to 5",
            ]
        );
    }

    #[test]
    fn the_english_text_of_an_unknown_vocabulary_value() {
        let warning = Warning::VocabularyValueUnknown {
            property: vocabulary::MODEL_RELEASE_STATUS.to_string(),
            value: "MR-XXX".to_string(),
        };
        assert_eq!(
            warning.to_string(),
            "plus:ModelReleaseStatus: \"MR-XXX\" is not in the PLUS controlled vocabulary"
        );
    }
}
