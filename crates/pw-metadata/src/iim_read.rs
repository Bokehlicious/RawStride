//! Reading the IIM block (ADR 0007 §5.1, §5.3, §5.4, §5.5).
//!
//! The reader (I6) gets the raw IIM block from ExifTool (`-b -IPTC`). This module turns it into
//! values of the model ([`crate::value`]) for the reconciliation (I5b):
//! - [`parse_block`] splits the block into its datasets, in file order (tag marker `0x1C`, record,
//!   dataset, a 2-byte big-endian length or an extended length, then the data). Zero bytes after
//!   the last dataset are padding. Malformed bytes are an error, never a panic.
//! - [`iim_values`] decodes every record-2 dataset under the encoding declared by 1:90 (§5.4, with
//!   the fallback encoding marking the value as converted), skips blank values (§5.1: they are
//!   non-existent), removes the prefixes of 2:04 and 2:12 (§5.3), merges Date Created 2:55 with
//!   Time Created 2:60 (§5.3), and builds one value per property of the property table. The
//!   deprecated datasets 2:10, 2:15 and 2:20 are read like the others (§5.5). Every value is
//!   checked by the rules of [`Metadata::insert_read`]; a value that does not fit is reported as
//!   unreadable instead of being dropped silently.
//!
//! Nothing here runs ExifTool or reads a file.

use std::fmt;

use crate::date::IptcDate;
use crate::iim::{self, CodedCharset, Decoded, Fallback, IimFormat};
use crate::properties::{self, Occurrence, PropertyDef, ValueKind};
use crate::value::{LangAlt, Metadata, Origin, Value};

/// The byte that starts every dataset.
const TAG_MARKER: u8 = 0x1C;

/// The envelope record (1:xx) and the application record (2:xx).
const ENVELOPE_RECORD: u8 = 1;
const APPLICATION_RECORD: u8 = 2;

/// 1:90, Coded Character Set.
const CODED_CHARACTER_SET: u8 = 90;

/// 2:00, Record Version: not a property.
const RECORD_VERSION: u8 = 0;

/// 2:55, Date Created, and 2:60, Time Created; read together as one Date Created (§5.3).
const DATE_CREATED: u8 = 55;
const TIME_CREATED: u8 = 60;

/// The top bit of the 2-byte length marks an extended length.
const EXTENDED_LENGTH: u16 = 0x8000;

/// The most bytes an extended length may use.
const MAX_EXTENDED_LENGTH_BYTES: usize = 4;

/// The language of the one Lang Alt entry an IIM value gives.
const X_DEFAULT: &str = "x-default";

/// One dataset of an IIM block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dataset {
    pub record: u8,
    pub dataset: u8,
    pub data: Vec<u8>,
}

/// Why an IIM block could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IimError {
    /// The byte at `offset` is not what the block format allows there: no tag marker where one is
    /// expected, an incomplete dataset header, or a length (at the offset of its first byte) that
    /// is invalid or runs past the end of the block.
    Malformed { offset: usize },
}

impl fmt::Display for IimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IimError::Malformed { offset } => {
                write!(f, "IIM block is malformed at byte {offset}")
            }
        }
    }
}

impl std::error::Error for IimError {}

/// Splits a raw IIM block into its datasets, in file order.
///
/// Each dataset is the tag marker `0x1C`, the record number, the dataset number and a 2-byte
/// big-endian length, then the data. A length with the top bit set is an extended length: its low
/// 15 bits give how many of the following bytes (1–4) hold the real length, big-endian. Zero bytes
/// after the last complete dataset are padding and are ignored. An empty block has no datasets.
pub fn parse_block(bytes: &[u8]) -> Result<Vec<Dataset>, IimError> {
    let mut datasets = Vec::new();
    let mut pos = 0;
    while let Some(&marker) = bytes.get(pos) {
        if marker != TAG_MARKER {
            let rest = bytes.get(pos..).unwrap_or_default();
            if rest.iter().all(|&b| b == 0) {
                break;
            }
            return Err(IimError::Malformed { offset: pos });
        }
        let malformed_header = IimError::Malformed { offset: pos };
        let record = *bytes.get(pos + 1).ok_or(malformed_header)?;
        let dataset = *bytes.get(pos + 2).ok_or(malformed_header)?;

        let length_at = pos + 3;
        let malformed_length = IimError::Malformed { offset: length_at };
        let word = read_be(bytes, length_at, 2).ok_or(malformed_length)?;
        let (length, data_start) = if word & u32::from(EXTENDED_LENGTH) != 0 {
            let count = usize::try_from(word & u32::from(!EXTENDED_LENGTH))
                .map_err(|_| malformed_length)?;
            if !(1..=MAX_EXTENDED_LENGTH_BYTES).contains(&count) {
                return Err(malformed_length);
            }
            let length = read_be(bytes, length_at + 2, count).ok_or(malformed_length)?;
            (length, length_at + 2 + count)
        } else {
            (word, length_at + 2)
        };
        let length = usize::try_from(length).map_err(|_| malformed_length)?;
        let data_end = data_start.checked_add(length).ok_or(malformed_length)?;
        let data = bytes.get(data_start..data_end).ok_or(malformed_length)?;
        datasets.push(Dataset {
            record,
            dataset,
            data: data.to_vec(),
        });
        pos = data_end;
    }
    Ok(datasets)
}

/// `count` bytes (at most 4) at `at` as a big-endian number; `None` past the end.
fn read_be(bytes: &[u8], at: usize, count: usize) -> Option<u32> {
    let field = bytes.get(at..at.checked_add(count)?)?;
    Some(
        field
            .iter()
            .fold(0u32, |value, &byte| (value << 8) | u32::from(byte)),
    )
}

/// How a property is written as an IIM value (ADR 0007 §5.3): `Genre` for Intellectual Genre
/// (2:04), `SubjectCode` for Subject Code (2:12), `Plain` for every other property. Used for
/// reading here and for writing in the write plan (I7).
pub fn iim_format(xmp_id: &str) -> IimFormat {
    match xmp_id {
        "Iptc4xmpCore:IntellectualGenre" => IimFormat::Genre,
        "Iptc4xmpCore:SubjectCode" => IimFormat::SubjectCode,
        _ => IimFormat::Plain,
    }
}

/// Something the user is told about an IIM value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IimNotice {
    /// The value was decoded by the fallback encoding, under an encoding RawStride does not decode,
    /// or with invalid UTF-8 replaced (ADR 0007 §5.4). Once per property.
    Converted { property: String },
    /// A single-value property had more than one dataset; the first was used.
    Repeated { property: String },
    /// The value could not be turned into a value of the property; `text` is the text as read
    /// (for Date Created, 2:55 and 2:60 joined with a space). The property gets no value.
    Unreadable { property: String, text: String },
}

/// The values of an IIM block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IimValues {
    /// `(XMP id, value)`, ordered by the position of each property's first dataset.
    pub values: Vec<(String, Value)>,
    /// The encoding declared by the first 1:90, `Absent` without one.
    pub charset: CodedCharset,
    /// The notices, grouped by property in the order of the properties' first datasets.
    pub notices: Vec<IimNotice>,
}

/// The non-blank datasets of one property, decoded, in file order.
struct Group {
    def: &'static PropertyDef,
    texts: Vec<Decoded>,
}

/// Turns the datasets of an IIM block into values of the model (ADR 0007 §5.1, §5.3–§5.5).
///
/// - The encoding is declared by the first 1:90; every other record-1 dataset, 2:00 and every
///   other record are ignored.
/// - Every record-2 dataset is decoded under that encoding; a blank value (only spaces and/or NUL)
///   is non-existent and skipped, and so is a Date Created 2:55 of only zeros.
/// - A dataset without a property in the table is ignored (it stays in the file; RawStride never
///   writes it).
/// - The prefix of 2:04 and 2:12 is removed; a value that is blank without its prefix is
///   non-existent too.
/// - A `Multi` property gives one list of all its datasets in file order; a `Single` property takes
///   its first dataset and a repeated one gives [`IimNotice::Repeated`]. Date Created is 2:55 with
///   the first 2:60.
/// - A value that cannot be built, or that the rules of [`Metadata::insert_read`] refuse, gives
///   [`IimNotice::Unreadable`] and no value.
pub fn iim_values(datasets: &[Dataset], fallback: Fallback) -> IimValues {
    let charset = iim::coded_charset(
        datasets
            .iter()
            .find(|d| d.record == ENVELOPE_RECORD && d.dataset == CODED_CHARACTER_SET)
            .map(|d| d.data.as_slice()),
    );

    let mut groups: Vec<Group> = Vec::new();
    let mut times: Vec<Decoded> = Vec::new();
    for dataset in datasets
        .iter()
        .filter(|d| d.record == APPLICATION_RECORD && d.dataset != RECORD_VERSION)
    {
        let decoded = iim::decode(&dataset.data, &charset, fallback);
        if iim::is_blank(&decoded.text) {
            continue;
        }
        if dataset.dataset == TIME_CREATED {
            times.push(decoded);
            continue;
        }
        if dataset.dataset == DATE_CREATED && decoded.text.bytes().all(|b| b == b'0') {
            continue;
        }
        let Some(def) = properties::by_iim(APPLICATION_RECORD, dataset.dataset) else {
            continue;
        };
        if iim::is_blank(iim::strip_prefix(&decoded.text, iim_format(def.xmp_id))) {
            // Only a prefix: as blank as an empty value (§5.1).
            continue;
        }
        match groups.iter_mut().find(|g| g.def.xmp_id == def.xmp_id) {
            Some(group) => group.texts.push(decoded),
            None => groups.push(Group {
                def,
                texts: vec![decoded],
            }),
        }
    }

    let mut result = IimValues {
        values: Vec::new(),
        charset,
        notices: Vec::new(),
    };
    let mut check = Metadata::new();
    for group in &groups {
        let time = if is_date_created(group.def) {
            times.first()
        } else {
            None
        };
        let repeated = group.texts.len() > 1 || (time.is_some() && times.len() > 1);
        read_group(group, time, repeated, &mut check, &mut result);
    }

    // A 2:60 without a 2:55: there is no date to put the time on.
    if let Some(time) = times.first()
        && let Some(def) = properties::by_iim(APPLICATION_RECORD, DATE_CREATED)
        && !groups.iter().any(|g| g.def.xmp_id == def.xmp_id)
    {
        let property = def.xmp_id.to_string();
        if times.len() > 1 {
            result.notices.push(IimNotice::Repeated {
                property: property.clone(),
            });
        }
        if time.converted {
            result.notices.push(IimNotice::Converted {
                property: property.clone(),
            });
        }
        result.notices.push(IimNotice::Unreadable {
            property,
            text: time.text.clone(),
        });
    }
    result
}

/// True for the property stored in 2:55.
fn is_date_created(def: &PropertyDef) -> bool {
    def.iim
        .is_some_and(|iim| iim.record == APPLICATION_RECORD && iim.dataset == DATE_CREATED)
}

/// Builds the value of one property and adds it, or its notices, to `result`.
fn read_group(
    group: &Group,
    time: Option<&Decoded>,
    repeated: bool,
    check: &mut Metadata,
    result: &mut IimValues,
) {
    let def = group.def;
    let property = def.xmp_id.to_string();
    let format = iim_format(def.xmp_id);

    // The datasets the value is made of: all of a `Multi` property, the first of a `Single` one.
    let used: Vec<&Decoded> = match def.occurrence {
        Occurrence::Multi => group.texts.iter().collect(),
        Occurrence::Single => group.texts.iter().take(1).collect(),
    };
    let texts: Vec<&str> = used
        .iter()
        .map(|decoded| iim::strip_prefix(&decoded.text, format))
        .collect();

    if repeated && def.occurrence == Occurrence::Single {
        result.notices.push(IimNotice::Repeated {
            property: property.clone(),
        });
    }
    if used.iter().any(|decoded| decoded.converted) || time.is_some_and(|t| t.converted) {
        result.notices.push(IimNotice::Converted {
            property: property.clone(),
        });
    }

    let as_read = match (texts.first(), time) {
        (Some(date), Some(time)) if def.kind == ValueKind::Date => {
            format!("{date} {}", time.text)
        }
        _ => texts.join("; "),
    };
    let value = build_value(def, &texts, time);
    let accepted = match value {
        Some(value) => match check.insert_read(def.xmp_id, value.clone(), Origin::Iim) {
            Ok(()) => Some(value),
            Err(_) => None,
        },
        None => None,
    };
    match accepted {
        Some(value) => result.values.push((property, value)),
        None => result.notices.push(IimNotice::Unreadable {
            property,
            text: as_read,
        }),
    }
}

/// The value of a property from its (non-blank, prefix-free) texts; `None` when the property's
/// kind has no IIM form or the text does not parse.
fn build_value(def: &PropertyDef, texts: &[&str], time: Option<&Decoded>) -> Option<Value> {
    if def.occurrence == Occurrence::Multi {
        return match def.kind {
            ValueKind::Struct(_) | ValueKind::LangAlt => None,
            _ => Some(Value::List(
                texts.iter().map(|text| (*text).to_string()).collect(),
            )),
        };
    }
    let text = *texts.first()?;
    match def.kind {
        ValueKind::Text | ValueKind::Uri | ValueKind::Url | ValueKind::Any => {
            Some(Value::Text(text.to_string()))
        }
        ValueKind::LangAlt => Some(Value::LangAlt(LangAlt::from_entries(vec![(
            X_DEFAULT.to_string(),
            text.to_string(),
        )]))),
        ValueKind::Integer | ValueKind::Number => Some(Value::Number(text.to_string())),
        ValueKind::Date => IptcDate::from_iim(text, time.map(|t| t.text.as_str()))
            .ok()
            .map(Value::Date),
        ValueKind::Struct(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::{Time, Zone};

    /// Standard base64 (RFC 4648) without a dependency; padding and invalid characters are skipped.
    fn base64(text: &str) -> Vec<u8> {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut buffer = 0u32;
        let mut bits = 0;
        for byte in text.bytes() {
            let Some(value) = ALPHABET.iter().position(|&c| c == byte) else {
                continue;
            };
            buffer = (buffer << 6) | u32::try_from(value).unwrap();
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(u8::try_from((buffer >> bits) & 0xFF).unwrap());
            }
        }
        out
    }

    /// A block of datasets with standard 2-byte lengths.
    fn block(datasets: &[(u8, u8, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (record, dataset, data) in datasets {
            out.extend_from_slice(&[TAG_MARKER, *record, *dataset]);
            out.extend_from_slice(&u16::try_from(data.len()).unwrap().to_be_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    fn values_of(datasets: &[(u8, u8, &[u8])]) -> IimValues {
        let datasets = parse_block(&block(datasets)).unwrap();
        iim_values(&datasets, Fallback::Windows1252)
    }

    fn text(value: &str) -> Value {
        Value::Text(value.to_string())
    }

    fn list(values: &[&str]) -> Value {
        Value::List(values.iter().map(|v| v.to_string()).collect())
    }

    fn converted(property: &str) -> IimNotice {
        IimNotice::Converted {
            property: property.to_string(),
        }
    }

    fn unreadable(property: &str, text: &str) -> IimNotice {
        IimNotice::Unreadable {
            property: property.to_string(),
            text: text.to_string(),
        }
    }

    fn pairs(values: &IimValues) -> Vec<(&str, &Value)> {
        values
            .values
            .iter()
            .map(|(id, value)| (id.as_str(), value))
            .collect()
    }

    const EXIFTOOL_BLOCK: &str =
        "HAFaAAMbJUccAQAAAgAEHAIZAANldHQcAhkAA3R25RwCWgAIR/Z0ZWJvcmccAgAAAgAE";

    #[test]
    fn a_block_written_by_exiftool_declares_utf8_but_holds_latin1() {
        let datasets = parse_block(&base64(EXIFTOOL_BLOCK)).unwrap();
        let ids: Vec<(u8, u8)> = datasets.iter().map(|d| (d.record, d.dataset)).collect();
        assert_eq!(ids, [(1, 90), (1, 0), (2, 25), (2, 25), (2, 90), (2, 0)]);
        assert_eq!(datasets[0].data, b"\x1b%G");
        assert_eq!(datasets[3].data, b"tv\xe5");
        assert_eq!(datasets[4].data, b"G\xf6teborg");

        let values = iim_values(&datasets, Fallback::Windows1252);
        assert_eq!(values.charset, CodedCharset::Utf8);
        assert_eq!(
            pairs(&values),
            [
                ("dc:subject", &list(&["ett", "tv\u{FFFD}"])),
                ("photoshop:City", &text("G\u{FFFD}teborg")),
            ]
        );
        assert_eq!(
            values.notices,
            [converted("dc:subject"), converted("photoshop:City")]
        );
    }

    #[test]
    fn without_1_90_the_fallback_decodes_the_same_datasets() {
        let datasets: Vec<Dataset> = parse_block(&base64(EXIFTOOL_BLOCK))
            .unwrap()
            .into_iter()
            .filter(|d| !(d.record == 1 && d.dataset == 90))
            .collect();
        for fallback in [Fallback::Windows1252, Fallback::Latin1] {
            let values = iim_values(&datasets, fallback);
            assert_eq!(values.charset, CodedCharset::Absent);
            assert_eq!(
                pairs(&values),
                [
                    ("dc:subject", &list(&["ett", "två"])),
                    ("photoshop:City", &text("Göteborg")),
                ],
                "{fallback:?}"
            );
            assert_eq!(
                values.notices,
                [converted("dc:subject"), converted("photoshop:City")]
            );
        }
    }

    #[test]
    fn converted_is_given_once_per_property() {
        let values = values_of(&[(2, 25, b"\xe5"), (2, 25, b"\xf6"), (2, 25, b"abc")]);
        assert_eq!(pairs(&values), [("dc:subject", &list(&["å", "ö", "abc"]))]);
        assert_eq!(values.notices, [converted("dc:subject")]);
    }

    #[test]
    fn an_extended_length_is_read() {
        let caption = vec![b'a'; 40_000];
        let mut bytes = vec![TAG_MARKER, 2, 120, 0x80, 0x02];
        bytes.extend_from_slice(&40_000u16.to_be_bytes());
        bytes.extend_from_slice(&caption);
        bytes.extend_from_slice(&block(&[(2, 90, b"Lund")]));

        let datasets = parse_block(&bytes).unwrap();
        assert_eq!(datasets.len(), 2);
        assert_eq!((datasets[0].record, datasets[0].dataset), (2, 120));
        assert_eq!(datasets[0].data, caption);
        assert_eq!(datasets[1].data, b"Lund");

        let values = iim_values(&datasets, Fallback::Windows1252);
        let expected = Value::LangAlt(LangAlt::from_entries(vec![(
            "x-default".to_string(),
            "a".repeat(40_000),
        )]));
        assert_eq!(
            pairs(&values),
            [
                ("dc:description", &expected),
                ("photoshop:City", &text("Lund"))
            ]
        );

        // Four length bytes are the most allowed.
        let mut four = vec![TAG_MARKER, 2, 90, 0x80, 0x04, 0, 0, 0, 3];
        four.extend_from_slice(b"Ystad");
        // The length is 3, so `ad` is left where a tag marker is expected.
        assert_eq!(parse_block(&four), Err(IimError::Malformed { offset: 12 }));
        four.truncate(12);
        assert_eq!(parse_block(&four).unwrap()[0].data, b"Yst");
    }

    #[test]
    fn malformed_blocks_report_the_offset() {
        // A 5-byte extended length.
        let five = [TAG_MARKER, 2, 90, 0x80, 0x05, 0, 0, 0, 0, 1, b'x'];
        assert_eq!(parse_block(&five), Err(IimError::Malformed { offset: 3 }));
        // An extended length of 0 bytes.
        let zero = [TAG_MARKER, 2, 90, 0x80, 0x00];
        assert_eq!(parse_block(&zero), Err(IimError::Malformed { offset: 3 }));

        // A length past the end, in the second dataset.
        let mut past = block(&[(2, 90, b"Lund")]);
        past.extend_from_slice(&[TAG_MARKER, 2, 25, 0, 10, b'a', b'b']);
        assert_eq!(parse_block(&past), Err(IimError::Malformed { offset: 12 }));
        // Extended length bytes past the end.
        let cut = [TAG_MARKER, 2, 90, 0x80, 0x02, 0x01];
        assert_eq!(parse_block(&cut), Err(IimError::Malformed { offset: 3 }));
        // A length field cut in half, and a header cut before it.
        assert_eq!(
            parse_block(&[TAG_MARKER, 2, 90, 0]),
            Err(IimError::Malformed { offset: 3 })
        );
        assert_eq!(
            parse_block(&[TAG_MARKER, 2]),
            Err(IimError::Malformed { offset: 0 })
        );

        // A missing tag marker after a complete dataset.
        let mut missing = block(&[(2, 90, b"Lund")]);
        missing.extend_from_slice(&[0x1D, 2, 25, 0, 1, b'a']);
        assert_eq!(
            parse_block(&missing),
            Err(IimError::Malformed { offset: 9 })
        );

        // Three bytes of garbage.
        assert_eq!(parse_block(b"abc"), Err(IimError::Malformed { offset: 0 }));

        // Zero bytes followed by anything else are not padding.
        let mut not_padding = block(&[(2, 90, b"Lund")]);
        not_padding.extend_from_slice(&[0, 0, 1]);
        assert_eq!(
            parse_block(&not_padding),
            Err(IimError::Malformed { offset: 9 })
        );

        assert_eq!(
            IimError::Malformed { offset: 12 }.to_string(),
            "IIM block is malformed at byte 12"
        );
    }

    #[test]
    fn trailing_zero_bytes_are_padding_and_an_empty_block_is_empty() {
        let mut padded = block(&[(2, 90, b"Lund")]);
        padded.extend_from_slice(&[0, 0, 0]);
        let datasets = parse_block(&padded).unwrap();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].data, b"Lund");

        assert_eq!(parse_block(&[]), Ok(vec![]));
        assert_eq!(parse_block(&[0, 0]), Ok(vec![]));
        let empty = iim_values(&[], Fallback::Windows1252);
        assert!(empty.values.is_empty());
        assert!(empty.notices.is_empty());
        assert_eq!(empty.charset, CodedCharset::Absent);
    }

    /// xorshift64 with a fixed seed: repeatable "random" bytes without a dependency.
    struct Random(u64);

    impl Random {
        fn byte(&mut self) -> u8 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0.to_le_bytes()[0]
        }
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut random = Random(0x5EED_1234_ABCD_0042);
        for length in 0..=64 {
            for round in 0..200 {
                let mut bytes: Vec<u8> = (0..length).map(|_| random.byte()).collect();
                // Half of the strings start like a dataset, so the length paths are reached.
                if round % 2 == 0 {
                    for (index, byte) in [TAG_MARKER, 2].into_iter().enumerate() {
                        if let Some(slot) = bytes.get_mut(index) {
                            *slot = byte;
                        }
                    }
                }
                if let Ok(datasets) = parse_block(&bytes) {
                    let _ = iim_values(&datasets, Fallback::Windows1252);
                    let _ = iim_values(&datasets, Fallback::Latin1);
                }
            }
        }
    }

    #[test]
    fn blank_values_are_non_existent() {
        let values = values_of(&[
            (2, 90, b"   "),
            (2, 95, b"\0\0"),
            (2, 25, b" "),
            (2, 25, b"\0 \0"),
            (2, 55, b"00000000"),
        ]);
        assert!(values.values.is_empty(), "{:?}", values.values);
        assert!(values.notices.is_empty(), "{:?}", values.notices);

        // A blank first dataset of a single property is skipped, not repeated.
        let values = values_of(&[(2, 90, b"  "), (2, 90, b"Lund")]);
        assert_eq!(pairs(&values), [("photoshop:City", &text("Lund"))]);
        assert!(values.notices.is_empty());
    }

    #[test]
    fn prefixes_are_removed() {
        let values = values_of(&[(2, 4, b"000:Feature"), (2, 12, b"IPTC:04000000")]);
        assert_eq!(
            pairs(&values),
            [
                ("Iptc4xmpCore:IntellectualGenre", &text("Feature")),
                ("Iptc4xmpCore:SubjectCode", &list(&["04000000"])),
            ]
        );
        assert!(values.notices.is_empty());

        // Only the prefix is as blank as an empty value; the next dataset is the first.
        let values = values_of(&[(2, 4, b"000:"), (2, 4, b"001:News"), (2, 12, b"IPTC:")]);
        assert_eq!(
            pairs(&values),
            [("Iptc4xmpCore:IntellectualGenre", &text("News"))]
        );
        assert!(values.notices.is_empty());
    }

    #[test]
    fn values_follow_the_kind_and_occurrence() {
        let values = values_of(&[
            (2, 120, b"En bild"),
            (2, 80, b"Anna"),
            (2, 80, b"Bo"),
            (2, 90, b"Lund"),
            (2, 90, b"Malm\xc3\xb6"),
            (2, 10, b"5"),
        ]);
        let caption = Value::LangAlt(LangAlt::from_entries(vec![(
            "x-default".to_string(),
            "En bild".to_string(),
        )]));
        assert_eq!(
            pairs(&values),
            [
                ("dc:description", &caption),
                ("dc:creator", &list(&["Anna", "Bo"])),
                ("photoshop:City", &text("Lund")),
                ("photoshop:Urgency", &Value::Number("5".to_string())),
            ]
        );
        assert_eq!(
            values.notices,
            [IimNotice::Repeated {
                property: "photoshop:City".to_string()
            }]
        );

        let values = values_of(&[(2, 10, b"x")]);
        assert!(values.values.is_empty());
        assert_eq!(values.notices, [unreadable("photoshop:Urgency", "x")]);
    }

    #[test]
    fn values_are_ordered_by_the_first_dataset_of_each_property() {
        let values = values_of(&[
            (2, 25, b"ett"),
            (2, 90, b"Lund"),
            (2, 25, b"tv\xc3\xa5"),
            (2, 80, b"Anna"),
        ]);
        let ids: Vec<&str> = values.values.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["dc:subject", "photoshop:City", "dc:creator"]);
        assert_eq!(values.values[0].1, list(&["ett", "två"]));
    }

    #[test]
    fn date_created_merges_2_55_and_2_60() {
        let values = values_of(&[(2, 55, b"20261104"), (2, 60, b"101530+0100")]);
        let expected = IptcDate {
            year: 2026,
            month: Some(11),
            day: Some(4),
            time: Some(Time {
                hour: 10,
                minute: 15,
                second: Some(30),
                fraction: None,
                zone: Some(Zone::Offset(60)),
            }),
        };
        assert_eq!(
            pairs(&values),
            [("photoshop:DateCreated", &Value::Date(expected))]
        );
        assert!(values.notices.is_empty());

        let values = values_of(&[(2, 55, b"20261104")]);
        let date_only = IptcDate {
            year: 2026,
            month: Some(11),
            day: Some(4),
            time: None,
        };
        assert_eq!(
            pairs(&values),
            [("photoshop:DateCreated", &Value::Date(date_only))]
        );

        let values = values_of(&[(2, 55, b"20261304")]);
        assert!(values.values.is_empty());
        assert_eq!(
            values.notices,
            [unreadable("photoshop:DateCreated", "20261304")]
        );

        let values = values_of(&[(2, 55, b"20261304"), (2, 60, b"101530+0100")]);
        assert_eq!(
            values.notices,
            [unreadable("photoshop:DateCreated", "20261304 101530+0100")]
        );

        let values = values_of(&[(2, 60, b"101530+0100")]);
        assert!(values.values.is_empty());
        assert_eq!(
            values.notices,
            [unreadable("photoshop:DateCreated", "101530+0100")]
        );
    }

    #[test]
    fn unknown_datasets_and_record_versions_are_ignored() {
        let values = values_of(&[
            (1, 0, b"\x00\x04"),
            (2, 0, b"\x00\x04"),
            (2, 200, b"unknown"),
            (3, 90, b"other record"),
        ]);
        assert!(values.values.is_empty());
        assert!(values.notices.is_empty());
        assert_eq!(values.charset, CodedCharset::Absent);
    }

    #[test]
    fn iim_format_of_the_prefixed_properties() {
        assert_eq!(
            iim_format("Iptc4xmpCore:IntellectualGenre"),
            IimFormat::Genre
        );
        assert_eq!(
            iim_format("Iptc4xmpCore:SubjectCode"),
            IimFormat::SubjectCode
        );
        assert_eq!(iim_format("photoshop:City"), IimFormat::Plain);
    }
}
