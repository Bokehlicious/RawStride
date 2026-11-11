//! IIM text rules (ADR 0007 §5.1–§5.4, §6.2).
//!
//! Plain functions on text and bytes; nothing here reads or writes a file:
//! - **Blank values** (§5.1, MWG §4.2.3.3): a text of only spaces and/or NUL characters is treated
//!   as non-existent ([`is_blank`]).
//! - **Byte limits** (§5.2, IPTC §15.1): IIM limits count bytes, not characters. A longer value is
//!   cut at a UTF-8 character boundary ([`truncate_utf8`]); the caller tells the user.
//! - **The predicted IIM value** (§5.1, §5.3): [`predict`] gives the IIM value RawStride would write
//!   from an XMP text, with the prefixes of Intellectual Genre 2:04 (`000:`, the 64-byte limit on
//!   the code only, IPTC §7.16) and Subject Code 2:12 (`IPTC:`, the limit on the whole value, IPTC
//!   §7.23). Equality between IIM and XMP is tested on that prediction ([`same_as_predicted`]), so
//!   a value that only differs because of IIM truncation is not a conflict. [`strip_prefix`]
//!   removes the prefixes when reading.
//! - **Character encodings** (§5.4, Q10; the same order for Exif text, §6.2): [`coded_charset`]
//!   reads dataset 1:90, [`decode`] turns IIM bytes into text: UTF-8 or ISO 8859-1 when 1:90
//!   declares it; otherwise ASCII, then valid UTF-8, then the fallback encoding (Windows-1252 by
//!   default) with the value marked as converted. A leading UTF-8 BOM is removed. IIM is always
//!   written as UTF-8 with 1:90 ([`UTF8_CODED_CHARSET`], MWG text encodings).
//!
//! Note: other declared IIM encodings (for example ISO 2022 sets) are decoded by the fallback order
//! above and marked converted. Decoding them properly would need a new dependency and an owner
//! decision.

/// The value of dataset 1:90 (Coded Character Set) that declares UTF-8: `ESC % G`. IIM is written
/// as UTF-8 with this 1:90 (ADR 0007 §5.4, MWG text encodings).
pub const UTF8_CODED_CHARSET: &[u8] = b"\x1b%G";

/// The ISO 2022 escape sequences that designate the ISO 8859-1 right half (`A`) as a 96-character
/// set: `ESC - A` into G1 (the usual form) and `ESC . A` into G2.
const LATIN1_DESIGNATIONS: [&[u8]; 2] = [b"\x1b-A", b"\x1b.A"];

/// `ESC ( B`: ASCII into G0. Allowed next to a Latin-1 designation (e.g. `ESC ( B ESC - A`).
const ASCII_DESIGNATION: &[u8] = b"\x1b(B";

/// The UTF-8 byte order mark, removed before decoding (ADR 0007 §5.4).
const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

/// True for an empty text and for a text of only spaces and/or NUL characters, which IIM and XMP
/// readers treat as non-existent (ADR 0007 §5.1, MWG §4.2.3.3). Other whitespace (tabs, line feeds)
/// is not blank.
pub fn is_blank(text: &str) -> bool {
    text.chars().all(|c| c == ' ' || c == '\0')
}

/// The longest prefix of `text` of at most `max_bytes` bytes that ends on a character boundary,
/// and whether the text was cut (ADR 0007 §5.2, IPTC §15.1: the limit counts bytes).
pub fn truncate_utf8(text: &str, max_bytes: usize) -> (&str, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text.get(..end).unwrap_or(""), true)
}

/// How a property is written as an IIM value (ADR 0007 §5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IimFormat {
    /// The text as it is.
    Plain,
    /// Intellectual Genre, 2:04: `000:` + the code; the byte limit applies to the code only
    /// (IPTC §7.16).
    Genre,
    /// Subject Code, 2:12: `IPTC:` + the code; the byte limit counts the whole value (IPTC §7.23).
    SubjectCode,
}

/// The prefix written before the code of Intellectual Genre 2:04 (IPTC §7.16).
const GENRE_PREFIX: &str = "000:";

/// The prefix written before the code of Subject Code 2:12 (IPTC §7.23).
const SUBJECT_CODE_PREFIX: &str = "IPTC:";

/// The IIM value RawStride would write for a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Predicted {
    pub value: String,
    /// True when the text was cut to the byte limit; the user is told (ADR 0007 §5.2).
    pub truncated: bool,
}

/// The IIM value RawStride would write for `text` in `format`, cut to `max_bytes` (`None`: no
/// limit) at a UTF-8 character boundary (ADR 0007 §5.1–§5.3).
pub fn predict(text: &str, format: IimFormat, max_bytes: Option<u16>) -> Predicted {
    let limit = |value: &str| match max_bytes {
        Some(max) => {
            let (cut, truncated) = truncate_utf8(value, usize::from(max));
            (cut.to_string(), truncated)
        }
        None => (value.to_string(), false),
    };
    let (value, truncated) = match format {
        IimFormat::Plain => limit(text),
        IimFormat::Genre => {
            // IPTC §7.16: "The 64-byte limit applies to the code only."
            let (code, truncated) = limit(text);
            (format!("{GENRE_PREFIX}{code}"), truncated)
        }
        IimFormat::SubjectCode => {
            // IPTC §7.23 gives 236 bytes for dataset 2:12 and, unlike §7.16, no exception for the
            // prefix, so the limit counts the whole value.
            limit(&format!("{SUBJECT_CODE_PREFIX}{text}"))
        }
    };
    Predicted { value, truncated }
}

/// The value read from IIM without the prefix of its format (ADR 0007 §5.3): `Genre` removes a
/// leading three digits and a colon (any digits, not only `000`), `SubjectCode` a leading `IPTC:`.
/// A value without the prefix, and every `Plain` value, is returned unchanged.
pub fn strip_prefix(value: &str, format: IimFormat) -> &str {
    match format {
        IimFormat::Plain => value,
        IimFormat::Genre => {
            let bytes = value.as_bytes();
            let has_prefix = matches!(
                bytes,
                [a, b, c, b':', ..] if a.is_ascii_digit() && b.is_ascii_digit() && c.is_ascii_digit()
            );
            if has_prefix {
                value.get(GENRE_PREFIX.len()..).unwrap_or(value)
            } else {
                value
            }
        }
        IimFormat::SubjectCode => value.strip_prefix(SUBJECT_CODE_PREFIX).unwrap_or(value),
    }
}

/// True when the IIM value read from a file (`actual`, already decoded) is what RawStride would
/// write: the texts are equal, or both are blank (ADR 0007 §5.1: a value that only differs because
/// of IIM truncation or encoding is not a conflict).
pub fn same_as_predicted(actual: &str, predicted: &Predicted) -> bool {
    actual == predicted.value || (is_blank(actual) && is_blank(&predicted.value))
}

/// The character encoding declared by dataset 1:90 (ADR 0007 §5.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodedCharset {
    /// `ESC % G`.
    Utf8,
    /// ISO 8859-1: `ESC - A` or `ESC . A` (see [`coded_charset`]).
    Latin1,
    /// Any other declaration, with its bytes. RawStride does not decode these encodings itself
    /// (see the module comment).
    Other(Vec<u8>),
    /// No 1:90 dataset.
    Absent,
}

/// The encoding declared by the value of dataset 1:90, `None` when the dataset is missing.
///
/// A 1:90 value is one or more ISO 2022 escape sequences, each starting with `ESC`:
/// - exactly `ESC % G` is `Utf8`;
/// - one or more Latin-1 designations (`ESC - A`, `ESC . A`), optionally with `ESC ( B` (ASCII in
///   G0), and nothing else, is `Latin1`;
/// - anything else (another set, UTF-8 mixed with other sequences, bytes that are not escape
///   sequences) is `Other`.
pub fn coded_charset(value: Option<&[u8]>) -> CodedCharset {
    let Some(bytes) = value else {
        return CodedCharset::Absent;
    };
    if bytes == UTF8_CODED_CHARSET {
        return CodedCharset::Utf8;
    }
    let sequences = escape_sequences(bytes);
    let is_latin1 = |seq: &&[u8]| LATIN1_DESIGNATIONS.contains(seq);
    if let Some(sequences) = sequences
        && sequences.iter().any(is_latin1)
        && sequences
            .iter()
            .all(|seq| is_latin1(seq) || *seq == ASCII_DESIGNATION)
    {
        return CodedCharset::Latin1;
    }
    CodedCharset::Other(bytes.to_vec())
}

/// Splits a 1:90 value into its escape sequences, each from one `ESC` up to the next. `None` when
/// the value is empty or does not start with `ESC`.
fn escape_sequences(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    if bytes.first() != Some(&0x1b) {
        return None;
    }
    let mut sequences = Vec::new();
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate().skip(1) {
        if byte == 0x1b {
            sequences.push(bytes.get(start..index)?);
            start = index;
        }
    }
    sequences.push(bytes.get(start..)?);
    Some(sequences)
}

/// The encoding used for text that is neither ASCII nor valid UTF-8 and has no usable declaration
/// (ADR 0007 §5.4, Q10: Windows-1252 by default, changeable in the settings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fallback {
    #[default]
    Windows1252,
    Latin1,
}

/// Decoded IIM text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub text: String,
    /// True when the text was not read as declared UTF-8, ISO 8859-1, ASCII or valid UTF-8: decoded
    /// by the fallback encoding, under an encoding RawStride does not decode, or with invalid UTF-8
    /// replaced. The user is told (ADR 0007 §5.4).
    pub converted: bool,
}

/// Decodes IIM bytes under the encoding declared by 1:90 (ADR 0007 §5.4, §6.2):
/// - a leading UTF-8 BOM is removed;
/// - `Utf8`: UTF-8; invalid sequences are replaced with U+FFFD and the text is marked converted;
/// - `Latin1`: ISO 8859-1;
/// - `Absent` and `Other`: all bytes below 128 as ASCII, otherwise valid UTF-8 as UTF-8, otherwise
///   the fallback encoding, marked converted. `Other` is always marked converted, because the
///   declared encoding is not one RawStride decodes.
pub fn decode(bytes: &[u8], charset: &CodedCharset, fallback: Fallback) -> Decoded {
    let bytes = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    match charset {
        CodedCharset::Utf8 => match std::str::from_utf8(bytes) {
            Ok(text) => Decoded {
                text: text.to_string(),
                converted: false,
            },
            Err(_) => Decoded {
                text: String::from_utf8_lossy(bytes).into_owned(),
                converted: true,
            },
        },
        CodedCharset::Latin1 => Decoded {
            text: decode_latin1(bytes),
            converted: false,
        },
        CodedCharset::Absent | CodedCharset::Other(_) => {
            let declared_other = matches!(charset, CodedCharset::Other(_));
            // ASCII is valid UTF-8, so one check covers steps 1 and 2 of §5.4.
            match std::str::from_utf8(bytes) {
                Ok(text) => Decoded {
                    text: text.to_string(),
                    converted: declared_other,
                },
                Err(_) => Decoded {
                    text: match fallback {
                        Fallback::Windows1252 => decode_windows1252(bytes),
                        Fallback::Latin1 => decode_latin1(bytes),
                    },
                    converted: true,
                },
            }
        }
    }
}

/// ISO 8859-1: every byte is the code point of the same number.
fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// Windows-1252 for 0x80–0x9F. The five unassigned bytes (0x81, 0x8D, 0x8F, 0x90, 0x9D) are the C1
/// control character of the same number, as Windows maps them. Every other byte is the ISO 8859-1
/// code point of the same number.
const WINDOWS1252_80_9F: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// Windows-1252 by the table above; no dependency.
fn decode_windows1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => WINDOWS1252_80_9F
                .get(usize::from(b - 0x80))
                .copied()
                .unwrap_or(char::from(b)),
            _ => char::from(b),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_is_only_spaces_and_nul() {
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(is_blank("\0\0"));
        assert!(is_blank(" \0 "));
        assert!(!is_blank("\t"));
        assert!(!is_blank("\n"));
        assert!(!is_blank(" a "));
    }

    #[test]
    fn truncate_counts_bytes_and_keeps_whole_characters() {
        assert_eq!(truncate_utf8("Åsa", 1), ("", true));
        assert_eq!(truncate_utf8("Åsa", 2), ("Å", true));
        assert_eq!(truncate_utf8("Åsa", 4), ("Åsa", false));
        assert_eq!(truncate_utf8("", 0), ("", false));

        let umlauts = "ä".repeat(20);
        assert_eq!(umlauts.len(), 40);
        let (cut, truncated) = truncate_utf8(&umlauts, 32);
        assert_eq!(cut, "ä".repeat(16));
        assert!(truncated);
        // 31 bytes would end inside an `ä`: never half a character.
        assert_eq!(truncate_utf8(&umlauts, 31), ("ä".repeat(15).as_str(), true));
    }

    #[test]
    fn plain_city_is_cut_to_32_bytes() {
        let city = "A".repeat(33);
        let predicted = predict(&city, IimFormat::Plain, Some(32));
        assert_eq!(predicted.value, "A".repeat(32));
        assert_eq!(predicted.value.len(), 32);
        assert!(predicted.truncated);

        let short = predict("Göteborg", IimFormat::Plain, Some(32));
        assert_eq!(short.value, "Göteborg");
        assert!(!short.truncated);

        let unlimited = predict(&city, IimFormat::Plain, None);
        assert_eq!(unlimited.value, city);
        assert!(!unlimited.truncated);
    }

    #[test]
    fn genre_gets_prefix_and_limit_on_the_code_only() {
        assert_eq!(
            predict("Portrait", IimFormat::Genre, Some(64)),
            Predicted {
                value: "000:Portrait".to_string(),
                truncated: false,
            }
        );

        let code = "g".repeat(70);
        let predicted = predict(&code, IimFormat::Genre, Some(64));
        assert_eq!(predicted.value, format!("000:{}", "g".repeat(64)));
        assert_eq!(predicted.value.len(), 68);
        assert!(predicted.truncated);
    }

    #[test]
    fn subject_code_gets_prefix_and_limit_on_the_whole_value() {
        assert_eq!(
            predict("04000000", IimFormat::SubjectCode, Some(236)),
            Predicted {
                value: "IPTC:04000000".to_string(),
                truncated: false,
            }
        );

        let code = "1".repeat(236);
        let predicted = predict(&code, IimFormat::SubjectCode, Some(236));
        assert_eq!(predicted.value.len(), 236);
        assert_eq!(predicted.value, format!("IPTC:{}", "1".repeat(231)));
        assert!(predicted.truncated);
    }

    #[test]
    fn prefixes_are_removed_when_reading() {
        assert_eq!(strip_prefix("012:Feature", IimFormat::Genre), "Feature");
        assert_eq!(strip_prefix("000:Portrait", IimFormat::Genre), "Portrait");
        assert_eq!(strip_prefix("Feature", IimFormat::Genre), "Feature");
        assert_eq!(strip_prefix("12:Feature", IimFormat::Genre), "12:Feature");
        assert_eq!(strip_prefix("ab0:Feature", IimFormat::Genre), "ab0:Feature");
        assert_eq!(
            strip_prefix("IPTC:04000000", IimFormat::SubjectCode),
            "04000000"
        );
        assert_eq!(strip_prefix("04000000", IimFormat::SubjectCode), "04000000");
        assert_eq!(strip_prefix("000:Plain", IimFormat::Plain), "000:Plain");
    }

    #[test]
    fn same_as_predicted_compares_with_the_iim_value() {
        let long = "B".repeat(40);
        let predicted = predict(&long, IimFormat::Plain, Some(32));
        assert!(same_as_predicted(&"B".repeat(32), &predicted));
        assert!(!same_as_predicted(&long, &predicted));

        assert!(same_as_predicted(
            "  ",
            &predict("", IimFormat::Plain, Some(32))
        ));
        assert!(same_as_predicted(
            "\0",
            &predict(" ", IimFormat::Plain, Some(32))
        ));

        assert!(!same_as_predicted(
            "Malmö",
            &predict("Göteborg", IimFormat::Plain, Some(32))
        ));
    }

    #[test]
    fn coded_charset_of_dataset_1_90() {
        assert_eq!(coded_charset(Some(b"\x1b%G")), CodedCharset::Utf8);
        assert_eq!(coded_charset(Some(UTF8_CODED_CHARSET)), CodedCharset::Utf8);
        assert_eq!(coded_charset(Some(b"\x1b.A")), CodedCharset::Latin1);
        assert_eq!(
            coded_charset(Some(b"\x1b(B")),
            CodedCharset::Other(b"\x1b(B".to_vec())
        );
        assert_eq!(coded_charset(None), CodedCharset::Absent);
        assert_eq!(Fallback::default(), Fallback::Windows1252);
    }

    #[test]
    fn latin1_is_declared_in_g1_or_g2_and_may_follow_ascii() {
        // ISO 2022: `ESC - A` designates ISO 8859-1 into G1 (the usual form), `ESC . A` into G2.
        for value in [
            &b"\x1b-A"[..],
            b"\x1b.A",
            b"\x1b(B\x1b-A",
            b"\x1b-A\x1b(B",
            b"\x1b(B\x1b-A\x1b.A",
        ] {
            assert_eq!(
                coded_charset(Some(value)),
                CodedCharset::Latin1,
                "{value:?}"
            );
        }
        assert_eq!(
            decode(
                b"G\xf6teborg",
                &coded_charset(Some(b"\x1b-A")),
                Fallback::Windows1252
            ),
            decoded("Göteborg", false)
        );
    }

    #[test]
    fn other_or_mixed_1_90_values_are_other() {
        for value in [
            &b""[..],
            b"\x1b(B",
            b"\x1b-B",
            b"\x1b%G\x1b-A",
            b"\x1b(B\x1b%G",
            b"\x1b-A\x1b-B",
            b"\x1b-AX",
            b"x\x1b-A",
            b"\x1b",
            b"\x1b%G ",
        ] {
            assert_eq!(
                coded_charset(Some(value)),
                CodedCharset::Other(value.to_vec()),
                "{value:?}"
            );
        }
    }

    fn decoded(text: &str, converted: bool) -> Decoded {
        Decoded {
            text: text.to_string(),
            converted,
        }
    }

    #[test]
    fn decode_without_1_90_tries_ascii_then_utf8_then_the_fallback() {
        let absent = CodedCharset::Absent;
        let w1252 = Fallback::Windows1252;
        assert_eq!(decode(b"abc", &absent, w1252), decoded("abc", false));
        assert_eq!(
            decode("Göteborg".as_bytes(), &absent, w1252),
            decoded("Göteborg", false)
        );
        assert_eq!(
            decode(b"G\xf6teborg", &absent, w1252),
            decoded("Göteborg", true)
        );
        assert_eq!(
            decode(b"G\xf6teborg", &absent, Fallback::Latin1),
            decoded("Göteborg", true)
        );
    }

    #[test]
    fn windows1252_and_latin1_differ_in_0x80_to_0x9f() {
        let absent = CodedCharset::Absent;
        assert_eq!(
            decode(b"\x80", &absent, Fallback::Windows1252),
            decoded("€", true)
        );
        assert_eq!(
            decode(b"\x80", &absent, Fallback::Latin1),
            decoded("\u{80}", true)
        );
        assert_eq!(
            decode(b"\x80", &CodedCharset::Latin1, Fallback::Windows1252),
            decoded("\u{80}", false)
        );
        // The five unassigned bytes are the C1 control character of the same number.
        assert_eq!(
            decode(b"\x81\x8d\x8f\x90\x9d", &absent, Fallback::Windows1252),
            decoded("\u{81}\u{8d}\u{8f}\u{90}\u{9d}", true)
        );
        assert_eq!(
            decode(b"\x8a\x9f\x99", &absent, Fallback::Windows1252),
            decoded("ŠŸ™", true)
        );
    }

    #[test]
    fn a_utf8_bom_is_removed() {
        let text = b"\xef\xbb\xbfG\xc3\xb6teborg";
        assert_eq!(
            decode(text, &CodedCharset::Utf8, Fallback::Windows1252),
            decoded("Göteborg", false)
        );
        assert_eq!(
            decode(text, &CodedCharset::Absent, Fallback::Windows1252),
            decoded("Göteborg", false)
        );
    }

    #[test]
    fn other_declared_encodings_are_marked_converted() {
        let other = CodedCharset::Other(b"\x1b(B".to_vec());
        assert_eq!(
            decode(b"abc", &other, Fallback::Windows1252),
            decoded("abc", true)
        );
        assert_eq!(
            decode(b"G\xf6teborg", &other, Fallback::Windows1252),
            decoded("Göteborg", true)
        );
    }

    #[test]
    fn invalid_utf8_under_declared_utf8_is_replaced() {
        assert_eq!(
            decode(b"G\xf6teborg", &CodedCharset::Utf8, Fallback::Windows1252),
            decoded("G\u{FFFD}teborg", true)
        );
        assert_eq!(
            decode(
                "Göteborg".as_bytes(),
                &CodedCharset::Utf8,
                Fallback::Windows1252
            ),
            decoded("Göteborg", false)
        );
    }
}
