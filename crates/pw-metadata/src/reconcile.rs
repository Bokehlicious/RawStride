//! Reconciliation of the representations of one image (ADR 0007 §4.2, §5.1, §6.2, §6.4, §6.5, §7).
//!
//! [`reconcile`] turns what the reader (I6) found in a file into one [`Metadata`]:
//! 1. **Primary XMP** (embedded for JPEG, TIFF and DNG; the sidecar for a manufacturer RAW) is read
//!    first. A value the model refuses is reported, not dropped silently; a blank value counts as
//!    absent (§5.1).
//! 2. **IIM** (§5.1): fills a property XMP lacks. A difference is always a [`Conflict`] with both
//!    values and the rule applied by the IIM checksum: matching → XMP, differing → IIM, missing →
//!    open (the user decides). In a RAW file the IIM is camera data and only fills (§4.2, §6.4).
//! 3. **Camera XMP** inside a RAW file only fills (§4.2). Its `xmp:Rating` 0 counts as no rating:
//!    it neither fills nor gives a conflict (§4.2).
//! 4. **Exif** (§6.1, §6.2, §6.5): Description, Copyright Notice and Creator fill a property XMP and
//!    IIM lack; a difference is a conflict. Against an IIM value the checksum decides (§6.2). The
//!    Digital Image GUID only fills from ImageUniqueID.
//! 5. **`Cleared`** (§6.4): a property the user cleared in a RAW file's sidecar is never filled from
//!    the RAW file; the value it would have got is reported as hidden by the user.
//! 6. **Date Created** (§7) is only *suggested* from DateTimeOriginal, never inserted.
//!
//! Equality ([`same_value`]) is tested on the predicted IIM value (§5.1) "or the whole XMP text, as
//! a program that ignores the IIM limit writes it" (§5.1, T129) and, against Exif, on the shown text
//! and the first creator (§6.5). Nothing here runs ExifTool or reads a file.

use std::collections::BTreeSet;

use crate::date::{DateError, IptcDate};
use crate::iim;
use crate::iim_read::{IimNotice, IimValues, iim_format};
use crate::properties;
use crate::value::{Conflict, ConflictRule, LangAlt, Metadata, Origin, Value, ValueError};

/// Description, filled from and compared with Exif ImageDescription.
const DESCRIPTION: &str = "dc:description";
/// Copyright Notice, filled from and compared with Exif Copyright.
const COPYRIGHT: &str = "dc:rights";
/// Creator, filled from and compared with Exif Photographer or Artist (§6.5).
const CREATOR: &str = "dc:creator";
/// Digital Image GUID, only filled from Exif ImageUniqueID (§6.1).
const DIGITAL_IMAGE_GUID: &str = "Iptc4xmpExt:DigImageGUID";
/// Date Created, only suggested from Exif (§7).
const DATE_CREATED: &str = "photoshop:DateCreated";
/// Rating; a camera rating of 0 inside a RAW file counts as no rating (§4.2).
const RATING: &str = "xmp:Rating";
/// The language of the one Lang Alt entry an Exif value gives.
const X_DEFAULT: &str = "x-default";
/// The separator between the names of an Exif Artist (MWG §5.7).
const ARTIST_SEPARATOR: &str = "; ";

/// The kind of file the readings come from (ADR 0007 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Jpeg,
    Tiff,
    Dng,
    /// A manufacturer RAW (ARW, RW2, …) with an XMP sidecar.
    Raw,
}

/// The IIM checksum: ExifTool's `IPTCDigest` (stored in the file) against `CurrentIPTCDigest`
/// (computed from the IIM block) (ADR 0007 §5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IimDigest {
    Matches,
    Differs,
    Missing,
}

/// The IIM of a file: its values (T80) and its checksum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IimRead {
    pub values: IimValues,
    pub digest: IimDigest,
}

/// The Exif fields the reconciliation uses, already decoded by the reader (I6). `None` when the
/// tag is missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExifFields {
    pub image_description: Option<String>,
    pub copyright: Option<String>,
    pub artist: Option<String>,
    pub photographer: Option<String>,
    pub image_unique_id: Option<String>,
    pub date_time_original: Option<String>,
    pub sub_sec_time_original: Option<String>,
    pub offset_time_original: Option<String>,
}

/// Everything the reader found in one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readings {
    pub kind: FileKind,
    /// The primary XMP: embedded for JPEG, TIFF and DNG, the sidecar for `Raw`.
    pub xmp: Vec<(String, Value)>,
    /// The XMP inside a `Raw` file (written by the camera); empty otherwise.
    pub camera_xmp: Vec<(String, Value)>,
    pub iim: Option<IimRead>,
    pub exif: ExifFields,
    /// The sidecar's `Cleared` list (only for `Raw`; empty until owner decision D1).
    pub cleared: BTreeSet<String>,
}

/// Something the user is told about the reconciliation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A notice of the IIM block (T80).
    Iim(IimNotice),
    /// A value the model refused; it was left out.
    Refused {
        property: String,
        origin: Origin,
        error: ValueError,
    },
    /// A cleared property of a RAW file that this value from the RAW file would have filled
    /// (§6.4, shown as "dold av användaren"). Once per property.
    HiddenByUser {
        property: String,
        value: Value,
        origin: Origin,
    },
    /// A value that could not be read; `text` is the text as read.
    Unreadable {
        property: String,
        origin: Origin,
        text: String,
    },
}

/// The result of [`reconcile`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconciled {
    /// The values and the conflicts.
    pub metadata: Metadata,
    pub notices: Vec<Notice>,
    /// Date Created from Exif DateTimeOriginal when the file has none (§7). Never in `metadata`.
    pub suggested_date_created: Option<IptcDate>,
}

/// The state of the reconciliation while it runs.
struct Work<'a> {
    metadata: Metadata,
    notices: Vec<Notice>,
    raw: bool,
    cleared: &'a BTreeSet<String>,
    /// The cleared properties that already gave `HiddenByUser`.
    hidden: BTreeSet<String>,
}

impl Work<'_> {
    /// Stores a value read from the file; a refused value gives `Notice::Refused`.
    fn insert(&mut self, property: &str, value: Value, origin: Origin) {
        if let Err(error) = self.metadata.insert_read(property, value, origin) {
            self.notices.push(Notice::Refused {
                property: property.to_string(),
                origin,
                error,
            });
        }
    }

    /// Fills an absent property from the RAW file or another secondary representation. A property
    /// the user cleared in a RAW file's sidecar is not filled (§6.4); the first value that would
    /// have filled it gives `HiddenByUser`.
    fn fill(&mut self, property: &str, value: Value, origin: Origin) {
        if self.raw && self.cleared.contains(property) {
            if self.hidden.insert(property.to_string()) {
                self.notices.push(Notice::HiddenByUser {
                    property: property.to_string(),
                    value,
                    origin,
                });
            }
            return;
        }
        self.insert(property, value, origin);
    }

    /// The current value and origin of a property.
    fn current(&self, property: &str) -> Option<(Value, Origin)> {
        self.metadata
            .get(property)
            .map(|stored| (stored.value.clone(), stored.origin))
    }

    fn conflict(&mut self, property: &str, values: Vec<(Origin, Value)>, rule: ConflictRule) {
        self.metadata.add_conflict(Conflict {
            property: property.to_string(),
            values,
            rule,
        });
    }
}

/// Turns the readings of one file into one [`Metadata`] (see the module comment for the order and
/// the rules).
pub fn reconcile(readings: Readings) -> Reconciled {
    let Readings {
        kind,
        xmp,
        camera_xmp,
        iim,
        exif,
        cleared,
    } = readings;
    let raw = kind == FileKind::Raw;
    let mut work = Work {
        metadata: Metadata::new(),
        notices: Vec::new(),
        raw,
        cleared: &cleared,
        hidden: BTreeSet::new(),
    };

    // 1. Primary XMP.
    let xmp_origin = if raw {
        Origin::XmpSidecar
    } else {
        Origin::XmpEmbedded
    };
    for (property, value) in xmp {
        if !is_blank_value(&value) {
            work.insert(&property, value, xmp_origin);
        }
    }

    // 2. IIM (§5.1).
    let digest = iim.as_ref().map(|read| read.digest);
    if let Some(read) = iim {
        work.notices
            .extend(read.values.notices.into_iter().map(Notice::Iim));
        for (property, value) in read.values.values {
            reconcile_iim(&mut work, &property, value, read.digest);
        }
    }

    // 3. Camera XMP (`Raw` only, §4.2).
    if raw {
        for (property, value) in camera_xmp {
            if !is_blank_value(&value) && !is_camera_no_rating(&property, &value) {
                reconcile_camera_xmp(&mut work, &property, value);
            }
        }
    }

    // 4. Exif (§6.1, §6.2, §6.5). ImageEditor ↔ Contributor and GPS → Location Created are not
    // part of this step yet; they get a task of their own later.
    reconcile_exif(&mut work, &exif, digest);

    // 5. `Cleared` (§6.4) is applied by `Work::fill` in steps 2–4.

    // 6. Date Created (§7): only a suggestion.
    let suggested_date_created = if work.metadata.get(DATE_CREATED).is_none() {
        suggest_date_created(&mut work, &exif)
    } else {
        None
    };

    Reconciled {
        metadata: work.metadata,
        notices: work.notices,
        suggested_date_created,
    }
}

/// One IIM value against the value XMP gave (§5.1; for `Raw` §4.2, §6.4).
fn reconcile_iim(work: &mut Work<'_>, property: &str, value: Value, digest: IimDigest) {
    let Some((current, origin)) = work.current(property) else {
        work.fill(property, value, Origin::Iim);
        return;
    };
    if same_value(property, &current, &value) {
        return;
    }
    let rule = if work.raw {
        ConflictRule::Xmp
    } else {
        match digest {
            IimDigest::Matches => ConflictRule::Xmp,
            IimDigest::Differs => ConflictRule::Iim,
            IimDigest::Missing => ConflictRule::Open,
        }
    };
    work.conflict(
        property,
        vec![(origin, current), (Origin::Iim, value.clone())],
        rule,
    );
    if rule == ConflictRule::Iim {
        work.insert(property, value, Origin::Iim);
    }
}

/// One value of the XMP inside a RAW file (§4.2): it only fills. A difference to the sidecar's
/// value is a conflict with rule `Xmp`, the sidecar's value kept. A value that was filled from the
/// RAW file's IIM is replaced, because XMP has precedence over IIM (§5.1); that is a conflict with
/// rule `Xmp` too.
fn reconcile_camera_xmp(work: &mut Work<'_>, property: &str, value: Value) {
    let Some((current, origin)) = work.current(property) else {
        work.fill(property, value, Origin::XmpEmbedded);
        return;
    };
    let same = match origin {
        Origin::Iim => same_value(property, &value, &current),
        _ => current == value,
    };
    if same {
        return;
    }
    work.conflict(
        property,
        vec![(origin, current), (Origin::XmpEmbedded, value.clone())],
        ConflictRule::Xmp,
    );
    if origin == Origin::Iim {
        work.insert(property, value, Origin::XmpEmbedded);
    }
}

/// True for a camera `xmp:Rating` that is a number equal to zero (`0`, `0.0`, ` 0 `): it counts as
/// no rating, so it neither fills nor gives a conflict (§4.2; owner, 2026-11-13: Sony writes 0 for
/// an unrated image). A value that is not a number is not affected.
fn is_camera_no_rating(property: &str, value: &Value) -> bool {
    property == RATING
        && matches!(value, Value::Number(text) if text.trim().parse::<f64>() == Ok(0.0))
}

/// The Exif fields (§6.1, §6.2, §6.5).
fn reconcile_exif(work: &mut Work<'_>, exif: &ExifFields, digest: Option<IimDigest>) {
    let lang_alt = |text: &str| {
        Value::LangAlt(LangAlt::from_entries(vec![(
            X_DEFAULT.to_string(),
            text.to_string(),
        )]))
    };
    if let Some(text) = present(&exif.image_description) {
        reconcile_exif_field(work, DESCRIPTION, lang_alt(text), text, digest);
    }
    if let Some(text) = present(&exif.copyright) {
        reconcile_exif_field(work, COPYRIGHT, lang_alt(text), text, digest);
    }
    // Photographer if present, else Artist (§6.5).
    let creators = match present(&exif.photographer) {
        Some(photographer) => vec![photographer.to_string()],
        None => present(&exif.artist).map(split_artist).unwrap_or_default(),
    };
    if let Some(first) = creators.first().cloned() {
        reconcile_exif_field(work, CREATOR, Value::List(creators), &first, digest);
    }
    // Digital Image GUID: Exif only fills; a different IPTC value is kept and is not a conflict
    // (§6.1).
    if let Some(guid) = present(&exif.image_unique_id)
        && work.metadata.get(DIGITAL_IMAGE_GUID).is_none()
    {
        work.fill(
            DIGITAL_IMAGE_GUID,
            Value::Text(guid.to_string()),
            Origin::Exif,
        );
    }
}

/// One "both ways" Exif field: `value` fills an absent property; `compared` is what the current
/// value is compared with ([`same_value`] against Exif: the shown text, or the first creator).
fn reconcile_exif_field(
    work: &mut Work<'_>,
    property: &str,
    value: Value,
    compared: &str,
    digest: Option<IimDigest>,
) {
    let Some((current, origin)) = work.current(property) else {
        work.fill(property, value, Origin::Exif);
        return;
    };
    if same_value(property, &current, &Value::Text(compared.to_string())) {
        return;
    }
    // XMP has precedence (§6.2); in a RAW file Exif is camera data (§6.4). Against IIM the
    // checksum decides: differing → IIM, matching or missing → Exif (§6.2, MWG §4.2.3.3).
    let rule = match (work.raw, origin) {
        (false, Origin::Iim) => match digest {
            Some(IimDigest::Differs) => ConflictRule::Iim,
            _ => ConflictRule::Exif,
        },
        _ => ConflictRule::Xmp,
    };
    work.conflict(
        property,
        vec![(origin, current), (Origin::Exif, value.clone())],
        rule,
    );
    if rule == ConflictRule::Exif {
        work.insert(property, value, Origin::Exif);
    }
}

/// Date Created from DateTimeOriginal, SubSecTimeOriginal and OffsetTimeOriginal (§7). An empty
/// date gives `None`; an invalid one gives `Notice::Unreadable` with the three texts as read,
/// joined with a space.
fn suggest_date_created(work: &mut Work<'_>, exif: &ExifFields) -> Option<IptcDate> {
    let date_time = exif.date_time_original.as_deref()?;
    let subsec = exif.sub_sec_time_original.as_deref();
    let offset = exif.offset_time_original.as_deref();
    match IptcDate::from_exif(date_time, subsec, offset) {
        Ok(date) => Some(date),
        Err(DateError::Empty) => None,
        Err(DateError::Invalid(_)) => {
            let text = [Some(date_time), subsec, offset]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            work.notices.push(Notice::Unreadable {
                property: DATE_CREATED.to_string(),
                origin: Origin::Exif,
                text,
            });
            None
        }
    }
}

/// The text of an Exif field, `None` when it is missing or blank (§5.1, T79).
fn present(field: &Option<String>) -> Option<&str> {
    field.as_deref().filter(|text| !iim::is_blank(text))
}

/// True for a value without content (§5.1, T79 `is_blank`): a blank text or number, a Lang Alt
/// whose entries are all blank, a list whose items are all blank. Such a value counts as absent.
fn is_blank_value(value: &Value) -> bool {
    match value {
        Value::Text(text) | Value::Number(text) => iim::is_blank(text),
        Value::LangAlt(lang_alt) => lang_alt
            .entries()
            .iter()
            .all(|(_, text)| iim::is_blank(text)),
        Value::List(items) => items.iter().all(|item| iim::is_blank(item)),
        Value::Date(_) | Value::Struct(_) | Value::StructList(_) => false,
    }
}

/// True when two values of the property `xmp_id` are the same for the reconciliation. `a` is the
/// value in the model (XMP, or IIM when XMP lacks it), `b` the value it is compared with.
///
/// **Against IIM** (ADR 0007 §5.1), `b` is the value read from IIM and the comparison is on the
/// IIM value RawStride would write from `a`:
/// - `Text` and `Number`: [`iim::predict`] of `a` with [`iim_format`] and the property's byte limit,
///   against `b` ([`iim::same_as_predicted`]). The prefix of 2:04 and 2:12 is removed from the
///   prediction, because the IIM value was read without it (T80). `b` is also the same when it
///   equals the prediction without the byte limit: "or the whole XMP text, as a program that
///   ignores the IIM limit writes it" (§5.1, T129; Photo Mechanic writes a whole 2191-byte
///   Description to 2:120). A text cut anywhere else is still different;
/// - Lang Alt: by the shown text ([`LangAlt::display`]) through the same prediction as `Text`, so
///   a Title cut to 64 bytes in 2:05 is not a conflict;
/// - `List`: item by item in order, as `Text`; the lengths must be equal;
/// - `Date`: by [`IptcDate::to_iim`] (2:55 and 2:60 both equal).
///
/// **Against Exif** (§6.5), `b` is a `Text`: the Exif text for Description and Copyright Notice, the
/// first name of Photographer or Artist for Creator. A Lang Alt is compared by its shown text, a
/// list by its **first** item (RawStride writes only the first creator, so `["A", "B"]` against
/// Artist `A` is the same).
///
/// Text equality is exact: no trimming and no case folding. Any other pair is compared as it is.
pub fn same_value(xmp_id: &str, a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Text(a), Value::Text(b)) | (Value::Number(a), Value::Number(b)) => {
            same_on_prediction(xmp_id, a, b)
        }
        (Value::LangAlt(a), Value::LangAlt(b)) => {
            same_on_prediction(xmp_id, shown_text(a), shown_text(b))
        }
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| same_on_prediction(xmp_id, a, b))
        }
        (Value::Date(a), Value::Date(b)) => a.to_iim() == b.to_iim(),
        // Against Exif.
        (Value::LangAlt(a), Value::Text(b)) => shown_text(a) == b.as_str(),
        (Value::List(a), Value::Text(b)) => a.first() == Some(b),
        _ => a == b,
    }
}

/// The text a Lang Alt shows, `""` without entries.
fn shown_text(lang_alt: &LangAlt) -> &str {
    lang_alt.display().map_or("", |(text, _)| text)
}

/// True when the IIM value `actual` (read without its prefix) is what RawStride would write from
/// the XMP text `xmp` (§5.1–§5.3), or the whole XMP text as a program that ignores the IIM byte
/// limit writes it (§5.1, T129): the same prediction without `max_bytes`.
fn same_on_prediction(xmp_id: &str, xmp: &str, actual: &str) -> bool {
    let format = iim_format(xmp_id);
    let max_bytes = properties::property(xmp_id)
        .and_then(|def| def.iim)
        .and_then(|dataset| dataset.max_bytes);
    let same = |max_bytes: Option<u16>| {
        let predicted = iim::predict(xmp, format, max_bytes);
        let without_prefix = iim::Predicted {
            value: iim::strip_prefix(&predicted.value, format).to_string(),
            truncated: predicted.truncated,
        };
        iim::same_as_predicted(actual, &without_prefix)
    };
    same(max_bytes) || (max_bytes.is_some() && same(None))
}

/// Splits an Exif Artist into names (MWG §5.7): names are separated by `"; "`; a name in double
/// quotes may contain `"; "`, and inside the quotes a doubled quote `""` stands for one quote. A
/// name that does not start with a quote is taken as it is up to the next `"; "` (it may contain
/// `;` and quotes). Blank names are dropped.
pub fn split_artist(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    loop {
        let (name, next) = match rest.strip_prefix('"') {
            Some(quoted) => read_quoted(quoted),
            None => match rest.split_once(ARTIST_SEPARATOR) {
                Some((name, next)) => (name.to_string(), Some(next)),
                None => (rest.to_string(), None),
            },
        };
        if !iim::is_blank(&name) {
            names.push(name);
        }
        match next {
            Some(next) => rest = next,
            None => break,
        }
    }
    names
}

/// Reads a quoted name (the text after its opening quote) up to the closing quote and the next
/// separator. Returns the name and the text after the separator (`None` at the end). Text between
/// the closing quote and the separator is kept as it is; without a closing quote the name runs to
/// the end.
fn read_quoted(text: &str) -> (String, Option<&str>) {
    let mut name = String::new();
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if c != '"' {
            name.push(c);
            continue;
        }
        if chars.next_if(|&(_, next)| next == '"').is_some() {
            name.push('"');
            continue;
        }
        let after = text.get(index + c.len_utf8()..).unwrap_or("");
        return match after.split_once(ARTIST_SEPARATOR) {
            Some((tail, next)) => {
                name.push_str(tail);
                (name, Some(next))
            }
            None => {
                name.push_str(after);
                (name, None)
            }
        };
    }
    (name, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::{Time, Zone};
    use crate::iim::CodedCharset;

    const CITY: &str = "photoshop:City";

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn list(items: &[&str]) -> Value {
        Value::List(items.iter().map(|s| s.to_string()).collect())
    }

    fn lang_alt(entries: &[(&str, &str)]) -> Value {
        Value::LangAlt(LangAlt::from_entries(
            entries
                .iter()
                .map(|(l, t)| (l.to_string(), t.to_string()))
                .collect(),
        ))
    }

    fn x_default(s: &str) -> Value {
        lang_alt(&[("x-default", s)])
    }

    fn values(entries: &[(&str, Value)]) -> Vec<(String, Value)> {
        entries
            .iter()
            .map(|(id, v)| (id.to_string(), v.clone()))
            .collect()
    }

    fn iim(entries: &[(&str, Value)], digest: IimDigest) -> Option<IimRead> {
        Some(IimRead {
            values: IimValues {
                values: values(entries),
                charset: CodedCharset::Utf8,
                notices: Vec::new(),
            },
            digest,
        })
    }

    fn readings(kind: FileKind) -> Readings {
        Readings {
            kind,
            xmp: Vec::new(),
            camera_xmp: Vec::new(),
            iim: None,
            exif: ExifFields::default(),
            cleared: BTreeSet::new(),
        }
    }

    fn jpeg(xmp: &[(&str, Value)], iim_values: Option<IimRead>) -> Readings {
        Readings {
            xmp: values(xmp),
            iim: iim_values,
            ..readings(FileKind::Jpeg)
        }
    }

    fn value_of(result: &Reconciled, id: &str) -> Option<(Value, Origin)> {
        result
            .metadata
            .get(id)
            .map(|stored| (stored.value.clone(), stored.origin))
    }

    fn conflicts_of<'a>(result: &'a Reconciled, id: &str) -> Vec<&'a Conflict> {
        result
            .metadata
            .conflicts()
            .iter()
            .filter(|c| c.property == id)
            .collect()
    }

    fn date(year: u16, month: u8, day: u8, time: Option<Time>) -> IptcDate {
        IptcDate {
            year,
            month: Some(month),
            day: Some(day),
            time,
        }
    }

    fn time(zone: Option<Zone>) -> Time {
        Time {
            hour: 10,
            minute: 15,
            second: Some(30),
            fraction: None,
            zone,
        }
    }

    // --- The three checksum cases ---

    fn city_case(digest: IimDigest) -> Reconciled {
        reconcile(jpeg(
            &[(CITY, text("Göteborg"))],
            iim(&[(CITY, text("Malmö"))], digest),
        ))
    }

    #[test]
    fn matching_checksum_keeps_xmp_with_a_conflict_of_rule_xmp() {
        let result = city_case(IimDigest::Matches);
        assert_eq!(
            value_of(&result, CITY),
            Some((text("Göteborg"), Origin::XmpEmbedded))
        );
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: CITY.into(),
                values: vec![
                    (Origin::XmpEmbedded, text("Göteborg")),
                    (Origin::Iim, text("Malmö"))
                ],
                rule: ConflictRule::Xmp,
            }]
        );
    }

    #[test]
    fn differing_checksum_uses_iim_with_a_conflict_of_rule_iim() {
        let result = city_case(IimDigest::Differs);
        assert_eq!(value_of(&result, CITY), Some((text("Malmö"), Origin::Iim)));
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: CITY.into(),
                values: vec![
                    (Origin::XmpEmbedded, text("Göteborg")),
                    (Origin::Iim, text("Malmö"))
                ],
                rule: ConflictRule::Iim,
            }]
        );
    }

    #[test]
    fn missing_checksum_keeps_xmp_with_an_open_conflict() {
        let result = city_case(IimDigest::Missing);
        assert_eq!(
            value_of(&result, CITY),
            Some((text("Göteborg"), Origin::XmpEmbedded))
        );
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: CITY.into(),
                values: vec![
                    (Origin::XmpEmbedded, text("Göteborg")),
                    (Origin::Iim, text("Malmö"))
                ],
                rule: ConflictRule::Open,
            }]
        );
    }

    // --- Equality on the prediction ---

    fn no_conflict(xmp: (&str, Value), iim_value: Value) -> bool {
        let result = reconcile(jpeg(
            &[(xmp.0, xmp.1.clone())],
            iim(&[(xmp.0, iim_value)], IimDigest::Missing),
        ));
        assert_eq!(value_of(&result, xmp.0).map(|(v, _)| v), Some(xmp.1));
        result.metadata.conflicts().is_empty()
    }

    #[test]
    fn a_city_cut_to_the_iim_limit_is_not_a_conflict() {
        let long = "Abcdefghijklmnopqrstuvwxyzåäöabcdefgh"; // 40 bytes
        assert_eq!(long.len(), 40);
        let (cut, truncated) = iim::truncate_utf8(long, 32);
        assert!(truncated);
        assert!(no_conflict((CITY, text(long)), text(cut)));
    }

    #[test]
    fn a_genre_read_without_its_prefix_equals_the_xmp_genre() {
        assert!(no_conflict(
            ("Iptc4xmpCore:IntellectualGenre", text("Feature")),
            text("Feature"),
        ));
    }

    #[test]
    fn a_description_is_compared_by_its_shown_text() {
        assert!(no_conflict(
            (DESCRIPTION, lang_alt(&[("x-default", "A"), ("sv", "B")])),
            x_default("A"),
        ));
    }

    const TITLE: &str = "dc:title";

    /// 63 ASCII bytes, `å` (bytes 63–64) and 5 more: 70 bytes, so the 64-byte limit of 2:05 cuts
    /// before `å`, not at byte 64.
    fn long_title() -> String {
        format!("{}å{}", "a".repeat(63), "b".repeat(5))
    }

    #[test]
    fn a_title_cut_to_the_iim_limit_is_not_a_conflict_for_any_checksum() {
        let long = long_title();
        assert_eq!(long.len(), 70);
        let (cut, truncated) = iim::truncate_utf8(&long, 64);
        assert!(truncated);
        assert_eq!(cut.len(), 63);
        for digest in [IimDigest::Matches, IimDigest::Differs, IimDigest::Missing] {
            let result = reconcile(jpeg(
                &[(TITLE, x_default(&long))],
                iim(&[(TITLE, x_default(cut))], digest),
            ));
            assert!(result.metadata.conflicts().is_empty(), "{digest:?}");
            assert_eq!(
                value_of(&result, TITLE),
                Some((x_default(&long), Origin::XmpEmbedded)),
                "{digest:?}"
            );
        }
    }

    #[test]
    fn a_description_cut_to_the_iim_limit_is_not_a_conflict() {
        let long = "ä".repeat(1001); // 2002 bytes
        let (cut, truncated) = iim::truncate_utf8(&long, 2000);
        assert!(truncated);
        assert!(no_conflict((DESCRIPTION, x_default(&long)), x_default(cut)));
    }

    #[test]
    fn a_really_different_title_is_still_a_conflict() {
        assert!(!no_conflict(
            (TITLE, x_default("Solnedgång")),
            x_default("Soluppgång"),
        ));
        let long = long_title();
        assert!(!no_conflict(
            (TITLE, x_default(&long)),
            x_default(&"c".repeat(63)),
        ));
    }

    #[test]
    fn keywords_in_another_order_are_a_conflict() {
        assert!(!no_conflict(
            ("dc:subject", list(&["ett", "två"])),
            list(&["två", "ett"]),
        ));
    }

    #[test]
    fn a_zone_less_xmp_date_equals_iim_without_time_created() {
        let xmp = Value::Date(date(2026, 11, 4, Some(time(None))));
        let iim_date = Value::Date(date(2026, 11, 4, None));
        assert!(no_conflict((DATE_CREATED, xmp), iim_date));
    }

    #[test]
    fn an_xmp_date_with_a_known_zone_differs_from_iim_without_time_created() {
        let xmp = Value::Date(date(2026, 11, 4, Some(time(Some(Zone::Offset(60))))));
        let iim_date = Value::Date(date(2026, 11, 4, None));
        assert!(!no_conflict((DATE_CREATED, xmp), iim_date));
    }

    #[test]
    fn subject_codes_read_without_their_prefix_equal_the_xmp_codes() {
        assert!(same_value(
            "Iptc4xmpCore:SubjectCode",
            &list(&["04000000"]),
            &list(&["04000000"]),
        ));
    }

    #[test]
    fn text_equality_is_exact() {
        assert!(!same_value(CITY, &text("Lund"), &text("lund")));
        assert!(!same_value(CITY, &text("Lund"), &text("Lund ")));
    }

    // --- An IIM value equal to the whole XMP text (§5.1, T129) ---

    /// A text of exactly `bytes` bytes with Swedish letters, longer than the IIM limit it is used
    /// with.
    fn long_swedish(bytes: usize) -> String {
        let mut text = String::new();
        while text.len() + "Båtar vid ön, ängen på Åland. ".len() <= bytes {
            text.push_str("Båtar vid ön, ängen på Åland. ");
        }
        while text.len() < bytes {
            text.push('x');
        }
        text
    }

    #[test]
    fn a_whole_over_limit_description_in_iim_is_not_a_conflict_for_any_checksum() {
        // As Photo Mechanic writes it in the level-C sample PM-jpg.JPG (T127).
        let long = long_swedish(2191);
        assert_eq!(long.len(), 2191);
        assert!(long.contains('å') && long.contains('ä') && long.contains('ö'));
        for digest in [IimDigest::Missing, IimDigest::Matches, IimDigest::Differs] {
            let result = reconcile(jpeg(
                &[(DESCRIPTION, x_default(&long))],
                iim(&[(DESCRIPTION, x_default(&long))], digest),
            ));
            assert!(result.metadata.conflicts().is_empty(), "{digest:?}");
            assert_eq!(
                value_of(&result, DESCRIPTION),
                Some((x_default(&long), Origin::XmpEmbedded)),
                "{digest:?}"
            );
        }
    }

    #[test]
    fn a_whole_over_limit_city_in_iim_is_not_a_conflict() {
        let long = long_swedish(40);
        assert!(long.len() > 32);
        assert!(no_conflict((CITY, text(&long)), text(&long)));
    }

    #[test]
    fn a_whole_over_limit_keyword_in_iim_is_not_a_conflict() {
        let long = long_swedish(70);
        assert!(long.len() > 64);
        assert!(no_conflict(
            ("dc:subject", list(&["hav", long.as_str()])),
            list(&["hav", long.as_str()]),
        ));
    }

    #[test]
    fn an_over_limit_description_cut_elsewhere_or_changed_after_the_limit_is_a_conflict() {
        let long = long_swedish(2191);
        // Cut neither at the limit (2000) nor whole.
        let (cut, truncated) = iim::truncate_utf8(&long, 2100);
        assert!(truncated);
        assert!(cut.len() > 2000);
        assert!(!no_conflict(
            (DESCRIPTION, x_default(&long)),
            x_default(cut)
        ));
        // One character changed after byte 2000.
        let mut changed = long.clone();
        changed.pop();
        changed.push('y');
        assert_ne!(changed, long);
        assert_eq!(changed.get(..2000), long.get(..2000));
        let result = reconcile(jpeg(
            &[(DESCRIPTION, x_default(&long))],
            iim(&[(DESCRIPTION, x_default(&changed))], IimDigest::Missing),
        ));
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: DESCRIPTION.into(),
                values: vec![
                    (Origin::XmpEmbedded, x_default(&long)),
                    (Origin::Iim, x_default(&changed))
                ],
                rule: ConflictRule::Open,
            }]
        );
    }

    #[test]
    fn same_value_of_over_limit_texts_is_exact_after_the_limit() {
        let long = long_swedish(2191);
        let mut changed = long.clone();
        changed.pop();
        changed.push('y');
        assert!(same_value(
            DESCRIPTION,
            &x_default(&long),
            &x_default(&long)
        ));
        assert!(!same_value(
            DESCRIPTION,
            &x_default(&long),
            &x_default(&changed)
        ));
        let city = long_swedish(40);
        let mut other_city = city.clone();
        other_city.pop();
        other_city.push('y');
        assert!(same_value(CITY, &text(&city), &text(&city)));
        assert!(!same_value(CITY, &text(&city), &text(&other_city)));
    }

    // --- IIM fills ---

    #[test]
    fn iim_fills_a_property_xmp_lacks_without_a_conflict() {
        let result = reconcile(jpeg(&[], iim(&[(CITY, text("Malmö"))], IimDigest::Matches)));
        assert_eq!(value_of(&result, CITY), Some((text("Malmö"), Origin::Iim)));
        assert!(result.metadata.conflicts().is_empty());
    }

    #[test]
    fn a_blank_xmp_text_counts_as_absent() {
        let result = reconcile(jpeg(
            &[(CITY, text("  \0"))],
            iim(&[(CITY, text("Malmö"))], IimDigest::Missing),
        ));
        assert_eq!(value_of(&result, CITY), Some((text("Malmö"), Origin::Iim)));
        assert!(result.metadata.conflicts().is_empty());
    }

    // --- Exif ---

    #[test]
    fn exif_fills_description_copyright_and_creator() {
        let result = reconcile(Readings {
            exif: ExifFields {
                image_description: Some("Solnedgång".into()),
                copyright: Some("© 2026".into()),
                artist: Some("A; B".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(
            value_of(&result, DESCRIPTION),
            Some((x_default("Solnedgång"), Origin::Exif))
        );
        assert_eq!(
            value_of(&result, COPYRIGHT),
            Some((x_default("© 2026"), Origin::Exif))
        );
        assert_eq!(
            value_of(&result, CREATOR),
            Some((list(&["A", "B"]), Origin::Exif))
        );
        assert!(result.metadata.conflicts().is_empty());
    }

    #[test]
    fn photographer_wins_over_artist() {
        let result = reconcile(Readings {
            exif: ExifFields {
                artist: Some("A; B".into()),
                photographer: Some("P; Q".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(
            value_of(&result, CREATOR),
            Some((list(&["P; Q"]), Origin::Exif))
        );
    }

    #[test]
    fn blank_exif_fields_count_as_absent() {
        let result = reconcile(Readings {
            exif: ExifFields {
                image_description: Some("   ".into()),
                photographer: Some(" ".into()),
                artist: Some("A".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(value_of(&result, DESCRIPTION), None);
        assert_eq!(
            value_of(&result, CREATOR),
            Some((list(&["A"]), Origin::Exif))
        );
    }

    #[test]
    fn split_artist_follows_mwg_5_7() {
        assert_eq!(split_artist("A; B"), ["A", "B"]);
        assert_eq!(split_artist("\"Smith; J\"; B"), ["Smith; J", "B"]);
        assert_eq!(split_artist("\"Say \"\"hi\"\"\"; B"), ["Say \"hi\"", "B"]);
    }

    #[test]
    fn split_artist_reads_the_mwg_examples() {
        assert_eq!(
            split_artist(
                "Camera owner, John Smith; Photographer, Michael Brown; Image creator, Ken James"
            ),
            [
                "Camera owner, John Smith",
                "Photographer, Michael Brown",
                "Image creator, Ken James"
            ]
        );
        assert_eq!(
            split_artist(
                "first; with;semicolon; \"with; semicolon-space\"; \"with; semicolon-space and \
                 \"\"quotes\"\"\"; non-leading \"quotes\"; \"\"\"leading\"\" and non-leading \
                 \"\"quotes\"\"\"; last"
            ),
            [
                "first",
                "with;semicolon",
                "with; semicolon-space",
                "with; semicolon-space and \"quotes\"",
                "non-leading \"quotes\"",
                "\"leading\" and non-leading \"quotes\"",
                "last"
            ]
        );
    }

    #[test]
    fn split_artist_drops_blank_names_and_never_panics() {
        assert_eq!(split_artist(""), Vec::<String>::new());
        assert_eq!(split_artist("A; ; B; "), ["A", "B"]);
        assert_eq!(split_artist("\"unterminated; x"), ["unterminated; x"]);
        assert_eq!(split_artist("\"a\"b; c"), ["ab", "c"]);
        assert_eq!(split_artist("\"å\"; \"\""), ["å"]);
    }

    fn creator_against_artist(xmp_creator: Value, artist: &str) -> Reconciled {
        reconcile(Readings {
            xmp: values(&[(CREATOR, xmp_creator)]),
            exif: ExifFields {
                artist: Some(artist.into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        })
    }

    #[test]
    fn only_the_first_creator_is_compared_with_artist() {
        let result = creator_against_artist(list(&["A", "B"]), "A");
        assert!(result.metadata.conflicts().is_empty());
        assert_eq!(
            value_of(&result, CREATOR),
            Some((list(&["A", "B"]), Origin::XmpEmbedded))
        );
    }

    #[test]
    fn a_different_artist_is_a_conflict_and_xmp_is_kept() {
        let result = creator_against_artist(list(&["A", "B"]), "C");
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: CREATOR.into(),
                values: vec![
                    (Origin::XmpEmbedded, list(&["A", "B"])),
                    (Origin::Exif, list(&["C"]))
                ],
                rule: ConflictRule::Xmp,
            }]
        );
        assert_eq!(
            value_of(&result, CREATOR),
            Some((list(&["A", "B"]), Origin::XmpEmbedded))
        );
    }

    fn iim_creator_against_artist(digest: IimDigest) -> Reconciled {
        reconcile(Readings {
            iim: iim(&[(CREATOR, list(&["I"]))], digest),
            exif: ExifFields {
                artist: Some("E".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        })
    }

    #[test]
    fn exif_wins_over_iim_when_the_checksum_matches_or_is_missing() {
        for digest in [IimDigest::Matches, IimDigest::Missing] {
            let result = iim_creator_against_artist(digest);
            assert_eq!(
                value_of(&result, CREATOR),
                Some((list(&["E"]), Origin::Exif)),
                "{digest:?}"
            );
            assert_eq!(
                result.metadata.conflicts(),
                [Conflict {
                    property: CREATOR.into(),
                    values: vec![(Origin::Iim, list(&["I"])), (Origin::Exif, list(&["E"]))],
                    rule: ConflictRule::Exif,
                }],
                "{digest:?}"
            );
        }
    }

    #[test]
    fn iim_is_kept_against_exif_when_the_checksum_differs() {
        let result = iim_creator_against_artist(IimDigest::Differs);
        assert_eq!(
            value_of(&result, CREATOR),
            Some((list(&["I"]), Origin::Iim))
        );
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: CREATOR.into(),
                values: vec![(Origin::Iim, list(&["I"])), (Origin::Exif, list(&["E"]))],
                rule: ConflictRule::Iim,
            }]
        );
    }

    #[test]
    fn a_description_from_xmp_against_exif_is_compared_by_its_shown_text() {
        let same = reconcile(Readings {
            xmp: values(&[(DESCRIPTION, lang_alt(&[("x-default", "A"), ("sv", "B")]))]),
            exif: ExifFields {
                image_description: Some("A".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert!(same.metadata.conflicts().is_empty());

        let different = reconcile(Readings {
            xmp: values(&[(DESCRIPTION, x_default("A"))]),
            exif: ExifFields {
                image_description: Some("B".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(
            different.metadata.conflicts(),
            [Conflict {
                property: DESCRIPTION.into(),
                values: vec![
                    (Origin::XmpEmbedded, x_default("A")),
                    (Origin::Exif, x_default("B"))
                ],
                rule: ConflictRule::Xmp,
            }]
        );
    }

    #[test]
    fn the_guid_fills_from_image_unique_id() {
        let result = reconcile(Readings {
            exif: ExifFields {
                image_unique_id: Some("0123456789abcdef".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(
            value_of(&result, DIGITAL_IMAGE_GUID),
            Some((text("0123456789abcdef"), Origin::Exif))
        );
    }

    #[test]
    fn a_different_xmp_guid_is_kept_without_a_conflict() {
        let result = reconcile(Readings {
            xmp: values(&[(DIGITAL_IMAGE_GUID, text("xmp-guid"))]),
            exif: ExifFields {
                image_unique_id: Some("exif-guid".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(
            value_of(&result, DIGITAL_IMAGE_GUID),
            Some((text("xmp-guid"), Origin::XmpEmbedded))
        );
        assert!(result.metadata.conflicts().is_empty());
    }

    // --- RAW ---

    #[test]
    fn the_sidecar_wins_over_camera_xmp_with_a_conflict() {
        let result = reconcile(Readings {
            xmp: values(&[(CITY, text("Göteborg"))]),
            camera_xmp: values(&[(CITY, text("Malmö"))]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(
            value_of(&result, CITY),
            Some((text("Göteborg"), Origin::XmpSidecar))
        );
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: CITY.into(),
                values: vec![
                    (Origin::XmpSidecar, text("Göteborg")),
                    (Origin::XmpEmbedded, text("Malmö"))
                ],
                rule: ConflictRule::Xmp,
            }]
        );
    }

    #[test]
    fn camera_xmp_fills_a_property_the_sidecar_lacks() {
        let result = reconcile(Readings {
            camera_xmp: values(&[(CITY, text("Malmö"))]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(
            value_of(&result, CITY),
            Some((text("Malmö"), Origin::XmpEmbedded))
        );
        assert!(result.metadata.conflicts().is_empty());
    }

    #[test]
    fn camera_xmp_is_ignored_for_other_files() {
        let result = reconcile(Readings {
            camera_xmp: values(&[(CITY, text("Malmö"))]),
            ..readings(FileKind::Dng)
        });
        assert_eq!(value_of(&result, CITY), None);
    }

    #[test]
    fn raw_iim_only_fills_and_a_difference_gets_rule_xmp() {
        let result = reconcile(Readings {
            xmp: values(&[(CITY, text("Göteborg"))]),
            iim: iim(
                &[(CITY, text("Malmö")), ("photoshop:State", text("Skåne"))],
                IimDigest::Differs,
            ),
            ..readings(FileKind::Raw)
        });
        assert_eq!(
            value_of(&result, CITY),
            Some((text("Göteborg"), Origin::XmpSidecar))
        );
        assert_eq!(
            value_of(&result, "photoshop:State"),
            Some((text("Skåne"), Origin::Iim))
        );
        assert_eq!(
            conflicts_of(&result, CITY)
                .iter()
                .map(|c| c.rule)
                .collect::<Vec<_>>(),
            [ConflictRule::Xmp]
        );
    }

    #[test]
    fn raw_exif_against_a_raw_iim_value_gets_rule_xmp() {
        let result = reconcile(Readings {
            iim: iim(&[(COPYRIGHT, x_default("I"))], IimDigest::Matches),
            exif: ExifFields {
                copyright: Some("E".into()),
                ..ExifFields::default()
            },
            ..readings(FileKind::Raw)
        });
        assert_eq!(
            value_of(&result, COPYRIGHT),
            Some((x_default("I"), Origin::Iim))
        );
        assert_eq!(
            conflicts_of(&result, COPYRIGHT)
                .iter()
                .map(|c| c.rule)
                .collect::<Vec<_>>(),
            [ConflictRule::Xmp]
        );
    }

    #[test]
    fn a_cleared_property_is_not_filled_from_the_raw_file() {
        let result = reconcile(Readings {
            camera_xmp: values(&[(DESCRIPTION, x_default("Kamera"))]),
            iim: iim(&[(DESCRIPTION, x_default("IIM"))], IimDigest::Missing),
            exif: ExifFields {
                image_description: Some("Exif".into()),
                ..ExifFields::default()
            },
            cleared: BTreeSet::from([DESCRIPTION.to_string()]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(value_of(&result, DESCRIPTION), None);
        assert!(result.metadata.conflicts().is_empty());
        // IIM is read before camera XMP and Exif, so its value is the first that would have filled.
        assert_eq!(
            result.notices,
            [Notice::HiddenByUser {
                property: DESCRIPTION.into(),
                value: x_default("IIM"),
                origin: Origin::Iim,
            }]
        );
    }

    #[test]
    fn a_cleared_property_hides_the_camera_xmp_value() {
        let result = reconcile(Readings {
            camera_xmp: values(&[(DESCRIPTION, x_default("Kamera"))]),
            exif: ExifFields {
                image_description: Some("Exif".into()),
                ..ExifFields::default()
            },
            cleared: BTreeSet::from([DESCRIPTION.to_string()]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(value_of(&result, DESCRIPTION), None);
        assert_eq!(
            result.notices,
            [Notice::HiddenByUser {
                property: DESCRIPTION.into(),
                value: x_default("Kamera"),
                origin: Origin::XmpEmbedded,
            }]
        );
    }

    #[test]
    fn a_cleared_property_is_not_filled_from_exif() {
        let result = reconcile(Readings {
            exif: ExifFields {
                artist: Some("A".into()),
                ..ExifFields::default()
            },
            cleared: BTreeSet::from([CREATOR.to_string()]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(value_of(&result, CREATOR), None);
        assert_eq!(
            result.notices,
            [Notice::HiddenByUser {
                property: CREATOR.into(),
                value: list(&["A"]),
                origin: Origin::Exif,
            }]
        );
    }

    #[test]
    fn a_sidecar_value_for_a_cleared_property_is_used() {
        let result = reconcile(Readings {
            xmp: values(&[(DESCRIPTION, x_default("Sidecar"))]),
            exif: ExifFields {
                image_description: Some("Exif".into()),
                ..ExifFields::default()
            },
            cleared: BTreeSet::from([DESCRIPTION.to_string()]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(
            value_of(&result, DESCRIPTION),
            Some((x_default("Sidecar"), Origin::XmpSidecar))
        );
        assert!(
            !result
                .notices
                .iter()
                .any(|n| matches!(n, Notice::HiddenByUser { .. }))
        );
    }

    // --- Camera rating 0 (§4.2, T113) ---

    fn number(s: &str) -> Value {
        Value::Number(s.to_string())
    }

    fn raw_rating(sidecar: Option<&str>, camera: &str) -> Reconciled {
        reconcile(Readings {
            xmp: sidecar.map_or_else(Vec::new, |r| values(&[(RATING, number(r))])),
            camera_xmp: values(&[(RATING, number(camera))]),
            ..readings(FileKind::Raw)
        })
    }

    #[test]
    fn a_camera_rating_0_gives_no_conflict_with_the_sidecar_rating() {
        let result = raw_rating(Some("1"), "0");
        assert_eq!(
            value_of(&result, RATING),
            Some((number("1"), Origin::XmpSidecar))
        );
        assert!(result.metadata.conflicts().is_empty());
    }

    #[test]
    fn a_camera_rating_0_does_not_fill() {
        let result = raw_rating(None, "0");
        assert_eq!(value_of(&result, RATING), None);
        assert!(result.metadata.conflicts().is_empty());
    }

    #[test]
    fn a_camera_rating_0_0_or_spaced_0_is_the_same_as_0() {
        for camera in ["0.0", " 0 "] {
            let result = raw_rating(Some("1"), camera);
            assert_eq!(
                value_of(&result, RATING),
                Some((number("1"), Origin::XmpSidecar)),
                "{camera:?}"
            );
            assert!(result.metadata.conflicts().is_empty(), "{camera:?}");
            let result = raw_rating(None, camera);
            assert_eq!(value_of(&result, RATING), None, "{camera:?}");
            assert!(result.metadata.conflicts().is_empty(), "{camera:?}");
        }
    }

    #[test]
    fn a_different_nonzero_camera_rating_is_still_a_conflict_of_rule_xmp() {
        let result = raw_rating(Some("1"), "3");
        assert_eq!(
            value_of(&result, RATING),
            Some((number("1"), Origin::XmpSidecar))
        );
        assert_eq!(
            result.metadata.conflicts(),
            [Conflict {
                property: RATING.into(),
                values: vec![
                    (Origin::XmpSidecar, number("1")),
                    (Origin::XmpEmbedded, number("3"))
                ],
                rule: ConflictRule::Xmp,
            }]
        );
    }

    #[test]
    fn a_camera_rating_minus_1_fills() {
        let result = raw_rating(None, "-1");
        assert_eq!(
            value_of(&result, RATING),
            Some((number("-1"), Origin::XmpEmbedded))
        );
        assert!(result.metadata.conflicts().is_empty());
    }

    #[test]
    fn a_cleared_rating_and_a_camera_rating_0_give_no_hidden_notice() {
        let result = reconcile(Readings {
            camera_xmp: values(&[(RATING, number("0"))]),
            cleared: BTreeSet::from([RATING.to_string()]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(value_of(&result, RATING), None);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| matches!(n, Notice::HiddenByUser { .. }))
        );
    }

    #[test]
    fn a_rating_0_in_a_jpeg_primary_xmp_is_still_a_value() {
        let result = reconcile(jpeg(&[(RATING, number("0"))], None));
        assert_eq!(
            value_of(&result, RATING),
            Some((number("0"), Origin::XmpEmbedded))
        );
    }

    // --- Date Created ---

    fn with_date_time_original(
        xmp: &[(&str, Value)],
        date_time: &str,
        offset: Option<&str>,
    ) -> Reconciled {
        reconcile(Readings {
            xmp: values(xmp),
            exif: ExifFields {
                date_time_original: Some(date_time.into()),
                sub_sec_time_original: Some("25".into()),
                offset_time_original: offset.map(String::from),
                ..ExifFields::default()
            },
            ..readings(FileKind::Jpeg)
        })
    }

    #[test]
    fn date_created_is_suggested_from_exif_and_not_inserted() {
        let with_offset = with_date_time_original(&[], "2026:11:04 10:15:30", Some("+01:00"));
        assert_eq!(
            with_offset.suggested_date_created,
            Some(date(
                2026,
                11,
                4,
                Some(Time {
                    fraction: Some("25".into()),
                    ..time(Some(Zone::Offset(60)))
                })
            ))
        );
        assert_eq!(value_of(&with_offset, DATE_CREATED), None);

        let without_offset = with_date_time_original(&[], "2026:11:04 10:15:30", None);
        assert_eq!(
            without_offset.suggested_date_created,
            Some(date(
                2026,
                11,
                4,
                Some(Time {
                    fraction: Some("25".into()),
                    ..time(None)
                })
            ))
        );
        assert_eq!(value_of(&without_offset, DATE_CREATED), None);
    }

    #[test]
    fn no_suggestion_when_date_created_is_present() {
        let present = Value::Date(date(2025, 1, 2, None));
        let result = with_date_time_original(
            &[(DATE_CREATED, present.clone())],
            "2026:11:04 10:15:30",
            None,
        );
        assert_eq!(result.suggested_date_created, None);
        assert_eq!(
            value_of(&result, DATE_CREATED),
            Some((present, Origin::XmpEmbedded))
        );
    }

    #[test]
    fn an_invalid_date_time_original_is_unreadable() {
        let result = with_date_time_original(&[], "2026:13:04 10:15:30", Some("+01:00"));
        assert_eq!(result.suggested_date_created, None);
        assert_eq!(
            result.notices,
            [Notice::Unreadable {
                property: DATE_CREATED.into(),
                origin: Origin::Exif,
                text: "2026:13:04 10:15:30 25 +01:00".into(),
            }]
        );
    }

    #[test]
    fn an_empty_date_time_original_gives_no_suggestion_and_no_notice() {
        let result = with_date_time_original(&[], "0000:00:00 00:00:00", None);
        assert_eq!(result.suggested_date_created, None);
        assert!(result.notices.is_empty());
    }

    // --- Refused values and T80 notices ---

    #[test]
    fn a_refused_xmp_value_is_reported_and_iim_fills() {
        let result = reconcile(jpeg(
            &[("dc:subject", text("ett"))],
            iim(&[("dc:subject", list(&["ett"]))], IimDigest::Matches),
        ));
        assert_eq!(
            result.notices,
            [Notice::Refused {
                property: "dc:subject".into(),
                origin: Origin::XmpEmbedded,
                error: ValueError::Kind {
                    property: "dc:subject".into(),
                    expected: "a list".into(),
                },
            }]
        );
        assert_eq!(
            value_of(&result, "dc:subject"),
            Some((list(&["ett"]), Origin::Iim))
        );
    }

    #[test]
    fn a_refused_sidecar_value_has_origin_sidecar() {
        let result = reconcile(Readings {
            xmp: values(&[("no:Such", text("x"))]),
            ..readings(FileKind::Raw)
        });
        assert_eq!(
            result.notices,
            [Notice::Refused {
                property: "no:Such".into(),
                origin: Origin::XmpSidecar,
                error: ValueError::UnknownProperty("no:Such".into()),
            }]
        );
    }

    #[test]
    fn iim_notices_arrive_as_notices() {
        let iim_notices = vec![
            IimNotice::Converted {
                property: CITY.into(),
            },
            IimNotice::Unreadable {
                property: "photoshop:Urgency".into(),
                text: "x".into(),
            },
        ];
        let result = reconcile(Readings {
            iim: Some(IimRead {
                values: IimValues {
                    values: values(&[(CITY, text("G\u{FFFD}teborg"))]),
                    charset: CodedCharset::Utf8,
                    notices: iim_notices.clone(),
                },
                digest: IimDigest::Missing,
            }),
            ..readings(FileKind::Jpeg)
        });
        assert_eq!(
            result.notices,
            iim_notices.into_iter().map(Notice::Iim).collect::<Vec<_>>()
        );
    }
}
