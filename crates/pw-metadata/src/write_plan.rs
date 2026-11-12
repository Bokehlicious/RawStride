//! The write plan: which ExifTool arguments bring a file in line with an edited [`Metadata`] (ADR
//! 0007 §5.2, §5.4, §5.5, §6.3, §6.5, §7, §8).
//!
//! [`plan`] turns the changed properties ([`Metadata::changes`]) into ExifTool tag arguments
//! (`-TAG=value`). Nothing here runs ExifTool or touches a file; applying the plan safely (§9) is
//! I8, so the plan holds no file name and no options such as `-o` or `-P`.
//! - **XMP** (§8): every changed property by its `et_xmp`. A Lang Alt writes only `x-default` (Q7),
//!   named as `-<et_xmp>-x-default=` so the other languages are kept; a list is written as a whole;
//!   a structure in ExifTool's structure syntax. Clearing removes the
//!   property, for a Lang Alt every language (`-<et_xmp>=` does that in ExifTool 13.59, checked in
//!   `tests/write_plan.rs`).
//! - **IIM** (§5.2–§5.5; JPEG, TIFF and DNG, never a manufacturer RAW): written when the file has
//!   IIM, or when the setting "create IIM" is on. Values are the predicted IIM values
//!   ([`iim::predict`]), cut to the byte limit; always UTF-8 with 1:90. Existing IIM in another
//!   encoding is converted on the first write. The checksum is written when IIM and XMP both exist
//!   after the write, unless an unresolved (`Open`) conflict remains (Q1). Every write also brings
//!   both forms in line (§5.1, §12): unchanged properties whose IIM value won (rule `Iim`) or that
//!   only IIM held are written to XMP, and those whose XMP value won (rule `Xmp`) to IIM, so the
//!   new checksum never hands a property back to a stale form.
//! - **Exif** (§6.3, §6.5; JPEG, TIFF and DNG): only the "both ways" fields that already exist are
//!   updated or removed, never created. DateTimeOriginal and its sub-fields, ImageUniqueID and GPS
//!   are never written.
//! - **Line breaks** (`-stay_open` takes one argument per line): when a value holds one, the plan
//!   asks for ExifTool's `-ec` option and writes a line feed as `\n`, a carriage return as `\r` and
//!   `\` as `\\` in every argument.

use std::collections::BTreeSet;
use std::fmt;

use crate::iim::{self, CodedCharset};
use crate::iim_read::iim_format;
use crate::properties::{self, PropertyDef, ValueKind};
use crate::reconcile::{FileKind, IimDigest};
use crate::value::{ConflictRule, LangAlt, Metadata, Origin, StructValue, Value};

/// The language of the Lang Alt entry ExifTool names without a language suffix.
const X_DEFAULT: &str = "x-default";
/// Description: synced with Exif ImageDescription.
const DESCRIPTION: &str = "dc:description";
/// Copyright Notice: synced with Exif Copyright.
const COPYRIGHT: &str = "dc:rights";
/// Creator: the first creator is synced with Exif Artist and Photographer (§6.5).
const CREATOR: &str = "dc:creator";
/// The ExifTool option that turns C-style escapes in values into characters.
const ESCAPE_C: &str = "-ec";
/// Declares the IIM as UTF-8 (1:90 `ESC % G`, MWG text encodings).
const IIM_UTF8: &str = "-IPTC:CodedCharacterSet=UTF8";
/// Writes a new IIM checksum computed over the IIM block after the write.
const NEW_DIGEST: &str = "-IPTCDigest=new";

/// Which Exif "both ways" fields exist in the file (§6.3: only these are written).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExifPresent {
    /// `IFD0:ImageDescription`.
    pub image_description: bool,
    /// `IFD0:Copyright`.
    pub copyright: bool,
    /// `IFD0:Artist`.
    pub artist: bool,
    /// `ExifIFD:Photographer` (Exif 3.0).
    pub photographer: bool,
}

/// What the plan needs to know about the file as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileState {
    pub kind: FileKind,
    /// The file has an IIM block.
    pub iim_present: bool,
    /// The encoding the file's 1:90 declares (`Absent` without one).
    pub iim_charset: CodedCharset,
    /// The values read from the file's IIM (T80), as text.
    pub iim_values: Vec<(String, Value)>,
    /// The IIM checksum as read.
    pub digest: IimDigest,
    pub exif_present: ExifPresent,
}

/// The settings that change what is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WriteSettings {
    /// "Skapa IPTC-IIM för kompatibilitet med äldre system" (§5.2): create IIM in a file without
    /// one. Off by default.
    pub create_iim: bool,
}

/// The ExifTool arguments of one write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WritePlan {
    /// ExifTool options: only `-ec`, when a value holds a line break.
    pub options: Vec<String>,
    /// ExifTool tag arguments (`-TAG=value`), in the order they are given.
    pub args: Vec<String>,
    pub notices: Vec<WriteNotice>,
}

/// Something the user is told about a write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteNotice {
    /// The IIM value of the property was cut to its byte limit (§5.2); XMP keeps it in full.
    Truncated { property: String },
    /// Existing IIM in another encoding was written again as UTF-8 (§5.4). Once per plan.
    IimConvertedToUtf8,
    /// The property has an unresolved conflict the user did not decide; neither form is written
    /// (Q1).
    ConflictKept { property: String },
    /// No checksum is written, because an unresolved conflict remains (Q1).
    DigestNotWritten,
}

/// Why a plan cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// A deprecated IIM property was changed (§5.5: never edited); holds its XMP id.
    NotEditable(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::NotEditable(property) => {
                write!(
                    f,
                    "{property} is a deprecated IIM property and cannot be edited"
                )
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// The plan that writes the changes of `metadata` to a file in `state` (see the module comment).
///
/// Without changes the plan is empty. A changed deprecated IIM property (Urgency, Category,
/// Supplemental Categories) is [`PlanError::NotEditable`].
pub fn plan(
    metadata: &Metadata,
    state: &FileState,
    settings: WriteSettings,
) -> Result<WritePlan, PlanError> {
    let changes: Vec<(&'static PropertyDef, Option<&Value>)> = metadata
        .changes()
        .filter_map(|(xmp_id, value)| properties::property(xmp_id).map(|def| (def, value)))
        .collect();
    if let Some((def, _)) = changes
        .iter()
        .find(|(def, _)| properties::is_deprecated_iim(def.xmp_id))
    {
        return Err(PlanError::NotEditable(def.xmp_id.to_string()));
    }
    let mut out = WritePlan::default();
    if changes.is_empty() {
        return Ok(out);
    }
    let changed: BTreeSet<&str> = changes.iter().map(|(def, _)| def.xmp_id).collect();

    // Q1: an unresolved conflict the user did not decide keeps both forms as they are.
    let mut kept: Vec<&str> = Vec::new();
    for conflict in metadata.conflicts() {
        let property = conflict.property.as_str();
        if conflict.rule == ConflictRule::Open
            && !changed.contains(property)
            && !kept.contains(&property)
        {
            kept.push(property);
        }
    }
    out.notices
        .extend(kept.iter().map(|property| WriteNotice::ConflictKept {
            property: (*property).to_string(),
        }));

    // XMP (§8).
    let mut writes_xmp_value = false;
    for (def, value) in &changes {
        let Some(et_xmp) = def.et_xmp else {
            continue;
        };
        match value {
            Some(value) => {
                writes_xmp_value = true;
                xmp_args(et_xmp, def.kind, value, &mut out.args);
            }
            None => out.args.push(format!("-{et_xmp}=")),
        }
    }

    // Bring both forms in line (§5.1, §12): the unchanged properties whose XMP and IIM differ.
    let in_line = if state.kind != FileKind::Raw && state.iim_present {
        bring_in_line(metadata, &changed)
    } else {
        InLine::default()
    };
    for (def, value) in &in_line.to_xmp {
        if let Some(et_xmp) = def.et_xmp {
            writes_xmp_value = true;
            xmp_args(et_xmp, def.kind, value, &mut out.args);
        }
    }

    if state.kind != FileKind::Raw {
        // IIM (§5.2–§5.5).
        let mut iim = IimArgs::default();
        for (def, value) in &changes {
            match value {
                Some(value) => iim.set(def, value),
                // Clearing a dataset of a file without IIM would only create an empty block.
                None if state.iim_present => iim.clear(def),
                None => {}
            }
        }
        for (def, value) in &in_line.to_iim {
            iim.set(def, value);
        }
        let has_sets = iim.sets > 0;
        let converts = state.iim_present
            && state.iim_charset != CodedCharset::Utf8
            && state
                .iim_values
                .iter()
                .any(|(_, value)| !value_is_ascii(value));
        if converts {
            // §5.4: every unchanged value is written again as UTF-8. This includes the IIM side of
            // an undecided conflict: its text is written again as it was read, only re-encoded,
            // so the conflict is not decided (otherwise the old bytes would be read as UTF-8).
            for (xmp_id, value) in &state.iim_values {
                // A value just written from XMP is not written again with its old IIM text.
                if changed.contains(xmp_id.as_str())
                    || in_line.to_iim.iter().any(|(def, _)| def.xmp_id == xmp_id)
                {
                    continue;
                }
                if let Some(def) = properties::property(xmp_id) {
                    iim.set(def, value);
                }
            }
        }
        let writes_iim =
            (state.iim_present || (settings.create_iim && has_sets)) && !iim.args.is_empty();
        if writes_iim {
            out.args.push(IIM_UTF8.to_string());
            out.args.append(&mut iim.args);
            out.notices.append(&mut iim.notices);
            if converts {
                out.notices.push(WriteNotice::IimConvertedToUtf8);
            }
        }

        // The checksum (§5.2, Q1).
        let iim_after = state.iim_present || writes_iim;
        let xmp_after = writes_xmp_value || has_xmp(metadata);
        if iim_after && xmp_after {
            if kept.is_empty() {
                out.args.push(NEW_DIGEST.to_string());
            } else {
                out.notices.push(WriteNotice::DigestNotWritten);
            }
        }

        // Exif (§6.3, §6.5).
        for (def, value) in &changes {
            exif_args(def.xmp_id, *value, state.exif_present, &mut out.args);
        }
    }

    if out.args.iter().any(|arg| arg.contains(['\n', '\r'])) {
        out.options.push(ESCAPE_C.to_string());
        out.args = out.args.iter().map(|arg| escape_c(arg)).collect();
    }
    Ok(out)
}

/// The XMP arguments for a new value of a property.
fn xmp_args(et_xmp: &str, kind: ValueKind, value: &Value, args: &mut Vec<String>) {
    match value {
        Value::Text(text) | Value::Number(text) => args.push(format!("-{et_xmp}={text}")),
        Value::Date(date) => args.push(format!("-{et_xmp}={}", date.to_xmp())),
        Value::LangAlt(lang_alt) => {
            // Q7: only `x-default`. The language must be named: ExifTool 13.59 replaces the whole
            // Lang Alt for `-<et_xmp>=<text>`, but `-<et_xmp>-x-default=<text>` keeps the other
            // languages and inserts `x-default` first when it is missing (owner's decision during
            // T83). Without `x-default` (not made by `Metadata::set`), the shown entry.
            match default_entry(lang_alt) {
                Some((None, text)) => args.push(format!("-{et_xmp}-{X_DEFAULT}={text}")),
                Some((Some(language), text)) => args.push(format!("-{et_xmp}-{language}={text}")),
                None => {}
            }
        }
        Value::List(items) => {
            args.push(format!("-{et_xmp}="));
            args.extend(items.iter().map(|item| format!("-{et_xmp}={item}")));
        }
        Value::Struct(fields) => {
            if let ValueKind::Struct(name) = kind {
                args.push(format!("-{et_xmp}={}", struct_text(name, fields)));
            }
        }
        Value::StructList(list) => {
            if let ValueKind::Struct(name) = kind {
                let items: Vec<String> = list
                    .iter()
                    .map(|fields| struct_text(name, fields))
                    .collect();
                args.push(format!("-{et_xmp}=[{}]", items.join(",")));
            }
        }
    }
}

/// The entry of a Lang Alt that is written: `x-default` as `(None, text)`, or without one the
/// shown entry with its language.
fn default_entry(lang_alt: &LangAlt) -> Option<(Option<&str>, &str)> {
    lang_alt.display().map(|(text, language)| (language, text))
}

/// A structure in ExifTool's structure syntax: `{Field=value,Field2=value}`, fields by their
/// `et_tag`. A field the structure does not know, or one without an ExifTool name, is left out
/// (the model only holds known fields).
fn struct_text(name: &str, fields: &StructValue) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(def) = properties::struct_def(name) {
        for (field_id, value) in &fields.fields {
            let Some(field) = def.fields.iter().find(|field| field.xmp_id == field_id) else {
                continue;
            };
            let Some(et_tag) = field.et_tag else {
                continue;
            };
            field_parts(et_tag, field.kind, value, &mut parts);
        }
    }
    format!("{{{}}}", parts.join(","))
}

/// The `Field=value` parts of one structure field. A Lang Alt field gives one part per language
/// (`Field` for `x-default`, `Field-<language>` for the others), because the whole structure is
/// written.
fn field_parts(et_tag: &str, kind: ValueKind, value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Text(text) | Value::Number(text) => {
            parts.push(format!("{et_tag}={}", escape_struct(text)));
        }
        Value::Date(date) => parts.push(format!("{et_tag}={}", escape_struct(&date.to_xmp()))),
        Value::LangAlt(lang_alt) => {
            for (language, text) in lang_alt.entries() {
                if language.eq_ignore_ascii_case(X_DEFAULT) {
                    parts.push(format!("{et_tag}={}", escape_struct(text)));
                } else {
                    parts.push(format!("{et_tag}-{language}={}", escape_struct(text)));
                }
            }
        }
        Value::List(items) => {
            let items: Vec<String> = items.iter().map(|item| escape_struct(item)).collect();
            parts.push(format!("{et_tag}=[{}]", items.join(",")));
        }
        Value::Struct(fields) => {
            if let ValueKind::Struct(name) = kind {
                parts.push(format!("{et_tag}={}", struct_text(name, fields)));
            }
        }
        Value::StructList(list) => {
            if let ValueKind::Struct(name) = kind {
                let items: Vec<String> = list
                    .iter()
                    .map(|fields| struct_text(name, fields))
                    .collect();
                parts.push(format!("{et_tag}=[{}]", items.join(",")));
            }
        }
    }
}

/// A value inside ExifTool's structure syntax: `|` before `,`, `}`, `]` and `|`, and before a
/// leading `[`, `{` or white space (ExifTool would otherwise read a structure or list, or drop the
/// leading space; checked with ExifTool 13.59).
fn escape_struct(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, c) in text.chars().enumerate() {
        let leading = index == 0 && (c == '[' || c == '{' || c.is_whitespace());
        if leading || matches!(c, ',' | '}' | ']' | '|') {
            out.push('|');
        }
        out.push(c);
    }
    out
}

/// C-style escapes for ExifTool's `-ec` option: `\` as `\\`, a line feed as `\n`, a carriage
/// return as `\r`.
fn escape_c(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len());
    for c in arg.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// The IIM arguments of a plan while it is made.
#[derive(Default)]
struct IimArgs {
    args: Vec<String>,
    notices: Vec<WriteNotice>,
    /// How many properties got a value (not a removal).
    sets: usize,
}

impl IimArgs {
    /// The IIM arguments for a value of `def` (§5.2, §5.3): the predicted value of each text, cut
    /// to the byte limit (a cut value gives `Truncated`, once per property). A list is rewritten as
    /// a whole, item by item; a Lang Alt by its `x-default`. Date Created is 2:55 and, only when
    /// [`crate::date::IptcDate::to_iim`] gives one, 2:60; otherwise 2:60 is removed (Q3). A
    /// property without an IIM dataset gives nothing.
    fn set(&mut self, def: &PropertyDef, value: &Value) {
        let (Some(dataset), Some(et_iim)) = (def.iim, def.et_iim) else {
            return;
        };
        let format = iim_format(def.xmp_id);
        let mut truncated = false;
        let mut predict = |text: &str| {
            let predicted = iim::predict(text, format, dataset.max_bytes);
            truncated |= predicted.truncated;
            predicted.value
        };
        let before = self.args.len();
        match value {
            Value::Text(text) | Value::Number(text) => {
                let text = predict(text);
                self.args.push(format!("-{et_iim}={text}"));
            }
            Value::LangAlt(lang_alt) => {
                if let Some((text, _)) = lang_alt.display() {
                    let text = predict(text);
                    self.args.push(format!("-{et_iim}={text}"));
                }
            }
            Value::List(items) => {
                self.args.push(format!("-{et_iim}="));
                for item in items {
                    let item = predict(item);
                    self.args.push(format!("-{et_iim}={item}"));
                }
            }
            Value::Date(date) => {
                let mut tags = et_iim.split('+');
                if let (Some(date_tag), Some(time_tag)) = (tags.next(), tags.next()) {
                    // `#=`: the value as it is, without ExifTool's conversion (checked with
                    // ExifTool 13.59: the dataset holds exactly this text).
                    let (date_text, time_text) = date.to_iim();
                    self.args.push(format!("-{date_tag}#={date_text}"));
                    match time_text {
                        Some(time_text) => self.args.push(format!("-{time_tag}#={time_text}")),
                        None => self.args.push(format!("-{time_tag}=")),
                    }
                }
            }
            Value::Struct(_) | Value::StructList(_) => {}
        }
        if self.args.len() > before {
            self.sets += 1;
        }
        if truncated {
            self.notices.push(WriteNotice::Truncated {
                property: def.xmp_id.to_string(),
            });
        }
    }

    /// Removes every dataset of `def` (Date Created: 2:55 and 2:60).
    fn clear(&mut self, def: &PropertyDef) {
        let (Some(_), Some(et_iim)) = (def.iim, def.et_iim) else {
            return;
        };
        self.args
            .extend(et_iim.split('+').map(|tag| format!("-{tag}=")));
    }
}

/// The unchanged properties a write brings in line (§5.1, §12), with their stored values.
#[derive(Default)]
struct InLine<'a> {
    /// Written to XMP: a conflict with rule `Iim`, or a value with origin `Iim` and no conflict.
    to_xmp: Vec<(&'static PropertyDef, &'a Value)>,
    /// Written to IIM: a conflict with rule `Xmp` between XMP and IIM.
    to_iim: Vec<(&'static PropertyDef, &'a Value)>,
}

/// Finds the properties the user did not change whose XMP and IIM differ, in table order (only
/// called for a JPEG, TIFF or DNG with IIM). Without this, the new checksum would make XMP win at
/// the next read and silently undo an IIM edit made by another program. Left out: conflicts with
/// rule `Open` (Q1) and `Exif`, conflicts between XMP and Exif only, and the deprecated IIM
/// properties (§5.5).
fn bring_in_line<'a>(metadata: &'a Metadata, changed: &BTreeSet<&str>) -> InLine<'a> {
    let mut in_line = InLine::default();
    for def in properties::properties() {
        let id = def.xmp_id;
        if changed.contains(id) || properties::is_deprecated_iim(id) {
            continue;
        }
        let Some(stored) = metadata.get(id) else {
            continue;
        };
        let mut conflicts = metadata
            .conflicts()
            .iter()
            .filter(|conflict| conflict.property == id)
            .peekable();
        if conflicts.peek().is_none() {
            if stored.origin == Origin::Iim {
                in_line.to_xmp.push((def, &stored.value));
            }
            continue;
        }
        let rules: Vec<ConflictRule> = conflicts
            .filter(|conflict| {
                conflict
                    .values
                    .iter()
                    .any(|(origin, _)| *origin == Origin::Iim)
            })
            .map(|conflict| conflict.rule)
            .collect();
        if rules.contains(&ConflictRule::Open) {
            continue;
        }
        if rules.contains(&ConflictRule::Iim) {
            in_line.to_xmp.push((def, &stored.value));
        } else if rules.contains(&ConflictRule::Xmp) {
            in_line.to_iim.push((def, &stored.value));
        }
    }
    in_line
}

/// True when every text of the value is ASCII.
fn value_is_ascii(value: &Value) -> bool {
    match value {
        Value::Text(text) | Value::Number(text) => text.is_ascii(),
        Value::LangAlt(lang_alt) => lang_alt.entries().iter().all(|(_, text)| text.is_ascii()),
        Value::List(items) => items.iter().all(|item| item.is_ascii()),
        Value::Date(_) => true,
        Value::Struct(fields) => fields.fields.iter().all(|(_, value)| value_is_ascii(value)),
        Value::StructList(list) => list
            .iter()
            .all(|fields| fields.fields.iter().all(|(_, value)| value_is_ascii(value))),
    }
}

/// True when the file has embedded XMP as far as the metadata shows: a value read from it, or a
/// conflict with such a value.
fn has_xmp(metadata: &Metadata) -> bool {
    let embedded = |origin: Origin| origin == Origin::XmpEmbedded;
    properties::properties().any(|def| {
        metadata
            .get(def.xmp_id)
            .is_some_and(|stored| embedded(stored.origin))
    }) || metadata
        .conflicts()
        .iter()
        .any(|conflict| conflict.values.iter().any(|(origin, _)| embedded(*origin)))
}

/// The Exif arguments for a changed property (§6.3, §6.5): Description → ImageDescription (the
/// shown text), Copyright Notice → Copyright, Creator → the first creator to Artist and
/// Photographer; each only when the field exists, and removed when the property was cleared.
/// Every other property gives nothing (DateTimeOriginal, ImageUniqueID and GPS are never written).
fn exif_args(xmp_id: &str, value: Option<&Value>, present: ExifPresent, args: &mut Vec<String>) {
    let tags: &[(&str, bool)] = match xmp_id {
        DESCRIPTION => &[("IFD0:ImageDescription", present.image_description)],
        COPYRIGHT => &[("IFD0:Copyright", present.copyright)],
        CREATOR => &[
            ("IFD0:Artist", present.artist),
            ("ExifIFD:Photographer", present.photographer),
        ],
        _ => return,
    };
    let text = match value {
        Some(value) => match exif_text(value) {
            Some(text) => text,
            None => return,
        },
        None => "",
    };
    for (tag, exists) in tags {
        if *exists {
            args.push(format!("-{tag}={text}"));
        }
    }
}

/// The text an Exif field gets: the shown text of a Lang Alt, the first item of a list.
fn exif_text(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) => Some(text),
        Value::LangAlt(lang_alt) => lang_alt.display().map(|(text, _)| text),
        Value::List(items) => items.first().map(String::as_str),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::IptcDate;
    use crate::value::Conflict;

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    fn list(items: &[&str]) -> Value {
        Value::List(items.iter().map(|item| item.to_string()).collect())
    }

    fn lang_alt(entries: &[(&str, &str)]) -> Value {
        Value::LangAlt(LangAlt::from_entries(
            entries
                .iter()
                .map(|(language, text)| (language.to_string(), text.to_string()))
                .collect(),
        ))
    }

    fn date(xmp: &str) -> Value {
        Value::Date(IptcDate::parse_xmp(xmp).unwrap())
    }

    fn fields(fields: Vec<(&str, Value)>) -> StructValue {
        StructValue {
            fields: fields
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect(),
        }
    }

    /// A JPEG without IIM and without Exif fields.
    fn jpeg() -> FileState {
        FileState {
            kind: FileKind::Jpeg,
            iim_present: false,
            iim_charset: CodedCharset::Absent,
            iim_values: Vec::new(),
            digest: IimDigest::Missing,
            exif_present: ExifPresent::default(),
        }
    }

    /// A JPEG with UTF-8 IIM and a matching checksum.
    fn jpeg_with_iim() -> FileState {
        FileState {
            iim_present: true,
            iim_charset: CodedCharset::Utf8,
            digest: IimDigest::Matches,
            ..jpeg()
        }
    }

    fn all_exif() -> ExifPresent {
        ExifPresent {
            image_description: true,
            copyright: true,
            artist: true,
            photographer: true,
        }
    }

    /// Metadata with one value read from embedded XMP, then the given edits.
    fn edited(edits: &[(&str, Value)]) -> Metadata {
        let mut metadata = Metadata::new();
        metadata
            .insert_read("xmp:Label", text("Red"), Origin::XmpEmbedded)
            .unwrap();
        for (id, value) in edits {
            metadata.set(id, value.clone()).unwrap();
        }
        metadata
    }

    fn cleared(ids: &[&str]) -> Metadata {
        let mut metadata = edited(&[]);
        for id in ids {
            metadata.clear(id).unwrap();
        }
        metadata
    }

    fn run(metadata: &Metadata, state: &FileState) -> WritePlan {
        plan(metadata, state, WriteSettings::default()).unwrap()
    }

    fn args(plan: &WritePlan) -> Vec<&str> {
        plan.args.iter().map(String::as_str).collect()
    }

    #[test]
    fn no_change_gives_an_empty_plan() {
        let plan = run(&edited(&[]), &jpeg_with_iim());
        assert_eq!(plan, WritePlan::default());
    }

    #[test]
    fn a_text_is_one_xmp_argument() {
        let plan = run(&edited(&[("photoshop:City", text("Lund"))]), &jpeg());
        assert_eq!(args(&plan), ["-XMP-photoshop:City=Lund"]);
        assert!(plan.options.is_empty());
        assert!(plan.notices.is_empty());
    }

    #[test]
    fn a_number_and_a_uri_are_written_as_their_text() {
        let plan = run(
            &edited(&[
                ("xmp:Rating", Value::Number("2.5".into())),
                ("xmpRights:WebStatement", text("https://example.com/r")),
            ]),
            &jpeg(),
        );
        assert_eq!(
            args(&plan),
            [
                "-XMP-xmp:Rating=2.5",
                "-XMP-xmpRights:WebStatement=https://example.com/r",
            ]
        );
    }

    #[test]
    fn a_lang_alt_writes_only_x_default() {
        let mut metadata = Metadata::new();
        metadata
            .insert_read(
                "dc:title",
                lang_alt(&[
                    ("x-default", "Gammal"),
                    ("sv", "Svensk"),
                    ("en-US", "English"),
                ]),
                Origin::XmpEmbedded,
            )
            .unwrap();
        metadata
            .set("dc:title", lang_alt(&[("x-default", "Ny titel")]))
            .unwrap();
        let plan = run(&metadata, &jpeg());
        assert_eq!(args(&plan), ["-XMP-dc:Title-x-default=Ny titel"]);
    }

    #[test]
    fn a_list_is_written_as_a_whole() {
        let plan = run(&edited(&[("dc:subject", list(&["a, b", "c"]))]), &jpeg());
        assert_eq!(
            args(&plan),
            [
                "-XMP-dc:Subject=",
                "-XMP-dc:Subject=a, b",
                "-XMP-dc:Subject=c"
            ]
        );
    }

    #[test]
    fn a_structure_escapes_its_special_characters() {
        let contact = fields(vec![
            ("Iptc4xmpCore:CiAdrCity", text("A,B}C]D|E")),
            ("Iptc4xmpCore:CiAdrExtadr", text("[ruta]")),
            ("Iptc4xmpCore:CiEmailWork", text("{a@b}")),
            ("Iptc4xmpCore:CiAdrRegion", text(" två")),
        ]);
        let plan = run(
            &edited(&[("Iptc4xmpCore:CreatorContactInfo", Value::Struct(contact))]),
            &jpeg(),
        );
        assert_eq!(
            args(&plan),
            ["-XMP-iptcCore:CreatorContactInfo={CiAdrCity=A|,B|}C|]D||E,\
              CiAdrExtadr=|[ruta|],CiEmailWork=|{a@b|},CiAdrRegion=| två}"]
        );
    }

    #[test]
    fn a_list_of_structures_with_a_lang_alt_field_writes_every_language() {
        let place = fields(vec![
            (
                "Iptc4xmpExt:LocationName",
                lang_alt(&[("x-default", "Torget"), ("sv", "Torget, Malmö")]),
            ),
            ("Iptc4xmpExt:City", text("Malmö")),
        ]);
        let second = fields(vec![("Iptc4xmpExt:City", text("Lund"))]);
        let plan = run(
            &edited(&[(
                "Iptc4xmpExt:LocationCreated",
                Value::StructList(vec![place, second]),
            )]),
            &jpeg(),
        );
        assert_eq!(
            args(&plan),
            ["-XMP-iptcExt:LocationCreated=[{LocationName=Torget,\
              LocationName-sv=Torget|, Malmö,City=Malmö},{City=Lund}]"]
        );
    }

    #[test]
    fn nested_lists_and_dates_in_a_structure() {
        let artwork = fields(vec![
            ("Iptc4xmpExt:AOCreator", list(&["Leo, da Vinci", "Okänd"])),
            ("Iptc4xmpExt:AODateCreated", date("1503-10")),
        ]);
        let plan = run(
            &edited(&[(
                "Iptc4xmpExt:ArtworkOrObject",
                Value::StructList(vec![artwork]),
            )]),
            &jpeg(),
        );
        assert_eq!(
            args(&plan),
            [
                "-XMP-iptcExt:ArtworkOrObject=[{AOCreator=[Leo|, da Vinci,Okänd],\
              AODateCreated=1503-10}]"
            ]
        );
    }

    #[test]
    fn a_date_with_a_zone_writes_time_created() {
        let plan = run(
            &edited(&[("photoshop:DateCreated", date("2026-11-04T10:15:30.5+01:00"))]),
            &jpeg_with_iim(),
        );
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:DateCreated=2026-11-04T10:15:30.5+01:00",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:DateCreated#=20261104",
                "-IPTC:TimeCreated#=101530+0100",
                "-IPTCDigest=new",
            ]
        );
    }

    #[test]
    fn a_date_without_a_zone_removes_time_created() {
        for xmp in ["2026-11-04T10:15", "2026-11"] {
            let plan = run(
                &edited(&[("photoshop:DateCreated", date(xmp))]),
                &jpeg_with_iim(),
            );
            let (date_iim, _) = IptcDate::parse_xmp(xmp).unwrap().to_iim();
            assert_eq!(
                args(&plan),
                [
                    format!("-XMP-photoshop:DateCreated={xmp}").as_str(),
                    "-IPTC:CodedCharacterSet=UTF8",
                    format!("-IPTC:DateCreated#={date_iim}").as_str(),
                    "-IPTC:TimeCreated=",
                    "-IPTCDigest=new",
                ],
                "{xmp}"
            );
        }
    }

    #[test]
    fn a_clear_removes_every_representation() {
        let state = FileState {
            exif_present: all_exif(),
            ..jpeg_with_iim()
        };
        let plan = run(
            &cleared(&[
                "photoshop:City",
                "dc:description",
                "dc:subject",
                "dc:creator",
                "photoshop:DateCreated",
                "Iptc4xmpCore:CreatorContactInfo",
                "Iptc4xmpExt:LocationCreated",
            ]),
            &state,
        );
        // `changes()` is ordered by XMP id.
        assert_eq!(
            args(&plan),
            [
                "-XMP-iptcCore:CreatorContactInfo=",
                "-XMP-iptcExt:LocationCreated=",
                "-XMP-dc:Creator=",
                "-XMP-dc:Description=",
                "-XMP-dc:Subject=",
                "-XMP-photoshop:City=",
                "-XMP-photoshop:DateCreated=",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:By-line=",
                "-IPTC:Caption-Abstract=",
                "-IPTC:Keywords=",
                "-IPTC:City=",
                "-IPTC:DateCreated=",
                "-IPTC:TimeCreated=",
                "-IPTCDigest=new",
                "-IFD0:Artist=",
                "-ExifIFD:Photographer=",
                "-IFD0:ImageDescription=",
            ]
        );
    }

    #[test]
    fn a_clear_of_a_lang_alt_removes_all_languages_with_one_argument() {
        let mut metadata = Metadata::new();
        metadata
            .insert_read(
                "dc:title",
                lang_alt(&[("x-default", "T"), ("de", "Deutsch")]),
                Origin::XmpEmbedded,
            )
            .unwrap();
        metadata.clear("dc:title").unwrap();
        let plan = run(&metadata, &jpeg());
        assert_eq!(args(&plan), ["-XMP-dc:Title="]);
    }

    #[test]
    fn iim_is_not_written_without_iim_and_without_the_setting() {
        let metadata = edited(&[("photoshop:City", text("Lund"))]);
        let plan = run(&metadata, &jpeg());
        assert_eq!(args(&plan), ["-XMP-photoshop:City=Lund"]);

        // A clear does not create IIM either, with or without the setting.
        let plan = plan_with(&cleared(&["photoshop:City"]), &jpeg(), true);
        assert_eq!(args(&plan), ["-XMP-photoshop:City="]);
    }

    fn plan_with(metadata: &Metadata, state: &FileState, create_iim: bool) -> WritePlan {
        plan(metadata, state, WriteSettings { create_iim }).unwrap()
    }

    #[test]
    fn iim_is_written_with_the_setting() {
        let metadata = edited(&[("photoshop:City", text("Lund"))]);
        let plan = plan_with(&metadata, &jpeg(), true);
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:City=Lund",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:City=Lund",
                "-IPTCDigest=new",
            ]
        );
        // A property without an IIM dataset does not create IIM.
        let metadata = edited(&[("Iptc4xmpExt:DigImageGUID", text("guid"))]);
        let plan = plan_with(&metadata, &jpeg(), true);
        assert_eq!(args(&plan), ["-XMP-iptcExt:DigitalImageGUID=guid"]);
    }

    #[test]
    fn a_city_over_32_bytes_is_truncated_in_iim_only() {
        // 40 bytes; `ö` takes bytes 31–32, so the cut at 32 bytes falls inside it.
        let city = format!("{}ö{}", "a".repeat(31), "b".repeat(7));
        assert_eq!(city.len(), 40);
        let plan = run(
            &edited(&[("photoshop:City", text(&city))]),
            &jpeg_with_iim(),
        );
        let cut = "a".repeat(31);
        assert_eq!(
            args(&plan),
            [
                format!("-XMP-photoshop:City={city}").as_str(),
                "-IPTC:CodedCharacterSet=UTF8",
                format!("-IPTC:City={cut}").as_str(),
                "-IPTCDigest=new",
            ]
        );
        assert_eq!(
            plan.notices,
            [WriteNotice::Truncated {
                property: "photoshop:City".into()
            }]
        );
    }

    #[test]
    fn a_40_byte_ascii_city_is_cut_at_32_bytes() {
        let city = "a".repeat(40);
        let plan = run(
            &edited(&[("photoshop:City", text(&city))]),
            &jpeg_with_iim(),
        );
        assert!(args(&plan).contains(&format!("-IPTC:City={}", "a".repeat(32)).as_str()));
        assert!(plan.notices.contains(&WriteNotice::Truncated {
            property: "photoshop:City".into()
        }));
    }

    #[test]
    fn genre_and_subject_code_get_their_prefixes() {
        let plan = run(
            &edited(&[
                ("Iptc4xmpCore:IntellectualGenre", text("Profile")),
                ("Iptc4xmpCore:SubjectCode", list(&["01001000", "04000000"])),
            ]),
            &jpeg_with_iim(),
        );
        assert_eq!(
            args(&plan),
            [
                "-XMP-iptcCore:IntellectualGenre=Profile",
                "-XMP-iptcCore:SubjectCode=",
                "-XMP-iptcCore:SubjectCode=01001000",
                "-XMP-iptcCore:SubjectCode=04000000",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:ObjectAttributeReference=000:Profile",
                "-IPTC:SubjectReference=",
                "-IPTC:SubjectReference=IPTC:01001000",
                "-IPTC:SubjectReference=IPTC:04000000",
                "-IPTCDigest=new",
            ]
        );
    }

    #[test]
    fn unchanged_latin1_values_are_written_again_as_utf8() {
        let state = FileState {
            iim_charset: CodedCharset::Latin1,
            iim_values: vec![
                ("photoshop:City".into(), text("Göteborg")),
                ("photoshop:Headline".into(), text("Åsa")),
                ("dc:subject".into(), list(&["fjäll", "sjö"])),
                ("dc:title".into(), lang_alt(&[("x-default", "Älv")])),
                ("photoshop:DateCreated".into(), date("2026-11-04")),
            ],
            ..jpeg_with_iim()
        };
        let plan = run(&edited(&[("photoshop:City", text("Malmö"))]), &state);
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:City=Malmö",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:City=Malmö",
                "-IPTC:Headline=Åsa",
                "-IPTC:Keywords=",
                "-IPTC:Keywords=fjäll",
                "-IPTC:Keywords=sjö",
                "-IPTC:ObjectName=Älv",
                "-IPTC:DateCreated#=20261104",
                "-IPTC:TimeCreated=",
                "-IPTCDigest=new",
            ]
        );
        assert_eq!(plan.notices, [WriteNotice::IimConvertedToUtf8]);
    }

    #[test]
    fn existing_iim_is_converted_on_any_write_but_not_when_it_is_ascii_or_utf8() {
        let latin1 = FileState {
            iim_charset: CodedCharset::Absent,
            iim_values: vec![("photoshop:Headline".into(), text("Åsa"))],
            ..jpeg_with_iim()
        };
        // A change without an IIM dataset still converts the existing IIM.
        let plan = run(
            &edited(&[("xmp:Rating", Value::Number("3".into()))]),
            &latin1,
        );
        assert_eq!(
            args(&plan),
            [
                "-XMP-xmp:Rating=3",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:Headline=Åsa",
                "-IPTCDigest=new",
            ]
        );
        assert_eq!(plan.notices, [WriteNotice::IimConvertedToUtf8]);

        let ascii = FileState {
            iim_values: vec![("photoshop:Headline".into(), text("Asa"))],
            ..latin1.clone()
        };
        let plan = run(
            &edited(&[("xmp:Rating", Value::Number("3".into()))]),
            &ascii,
        );
        assert_eq!(args(&plan), ["-XMP-xmp:Rating=3", "-IPTCDigest=new"]);
        assert!(plan.notices.is_empty());

        let utf8 = FileState {
            iim_charset: CodedCharset::Utf8,
            ..latin1
        };
        let plan = run(&edited(&[("xmp:Rating", Value::Number("3".into()))]), &utf8);
        assert_eq!(args(&plan), ["-XMP-xmp:Rating=3", "-IPTCDigest=new"]);
    }

    /// City read from XMP with a different IIM value and no checksum: an `Open` conflict.
    fn with_open_conflict() -> Metadata {
        let mut metadata = Metadata::new();
        metadata
            .insert_read("photoshop:City", text("Lund"), Origin::XmpEmbedded)
            .unwrap();
        metadata.add_conflict(Conflict {
            property: "photoshop:City".into(),
            values: vec![
                (Origin::XmpEmbedded, text("Lund")),
                (Origin::Iim, text("Malmö")),
            ],
            rule: ConflictRule::Open,
        });
        metadata
    }

    fn missing_digest() -> FileState {
        FileState {
            digest: IimDigest::Missing,
            ..jpeg_with_iim()
        }
    }

    #[test]
    fn the_digest_is_written_without_an_open_conflict() {
        let plan = run(
            &edited(&[("photoshop:Headline", text("Ny"))]),
            &missing_digest(),
        );
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:Headline=Ny",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:Headline=Ny",
                "-IPTCDigest=new",
            ]
        );
        assert!(plan.notices.is_empty());
    }

    #[test]
    fn an_open_conflict_keeps_both_forms_and_the_digest_is_not_written() {
        let mut metadata = with_open_conflict();
        metadata.set("photoshop:Headline", text("Ny")).unwrap();
        let plan = run(&metadata, &missing_digest());
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:Headline=Ny",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:Headline=Ny",
            ]
        );
        assert_eq!(
            plan.notices,
            [
                WriteNotice::ConflictKept {
                    property: "photoshop:City".into()
                },
                WriteNotice::DigestNotWritten,
            ]
        );
    }

    #[test]
    fn a_changed_property_with_an_open_conflict_is_written_in_both_forms() {
        let mut metadata = with_open_conflict();
        metadata.set("photoshop:City", text("Göteborg")).unwrap();
        let plan = run(&metadata, &missing_digest());
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:City=Göteborg",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:City=Göteborg",
                "-IPTCDigest=new",
            ]
        );
        assert!(plan.notices.is_empty());
    }

    #[test]
    fn a_conversion_rewrites_the_iim_text_of_an_open_conflict_unchanged() {
        let mut metadata = with_open_conflict();
        metadata.set("photoshop:Headline", text("Ny")).unwrap();
        let state = FileState {
            iim_charset: CodedCharset::Absent,
            iim_values: vec![("photoshop:City".into(), text("Malmö"))],
            ..missing_digest()
        };
        let plan = run(&metadata, &state);
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:Headline=Ny",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:Headline=Ny",
                "-IPTC:City=Malmö",
            ]
        );
        assert!(plan.notices.contains(&WriteNotice::DigestNotWritten));
        assert!(plan.notices.contains(&WriteNotice::IimConvertedToUtf8));
    }

    /// A conflict between XMP and IIM with `rule`; the stored value is the one the rule chose.
    fn iim_conflict(metadata: &mut Metadata, id: &str, xmp: Value, iim: Value, rule: ConflictRule) {
        let (stored, origin) = match rule {
            ConflictRule::Iim => (iim.clone(), Origin::Iim),
            _ => (xmp.clone(), Origin::XmpEmbedded),
        };
        metadata.insert_read(id, stored, origin).unwrap();
        metadata.add_conflict(Conflict {
            property: id.into(),
            values: vec![(Origin::XmpEmbedded, xmp), (Origin::Iim, iim)],
            rule,
        });
    }

    fn differs() -> FileState {
        FileState {
            digest: IimDigest::Differs,
            exif_present: all_exif(),
            ..jpeg_with_iim()
        }
    }

    fn rating() -> (&'static str, Value) {
        ("xmp:Rating", Value::Number("3".into()))
    }

    #[test]
    fn an_iim_value_that_won_by_the_checksum_is_written_to_xmp() {
        let mut metadata = edited(&[rating()]);
        iim_conflict(
            &mut metadata,
            "photoshop:City",
            text("Lund"),
            text("Malmö"),
            ConflictRule::Iim,
        );
        let plan = run(&metadata, &differs());
        assert_eq!(
            args(&plan),
            [
                "-XMP-xmp:Rating=3",
                "-XMP-photoshop:City=Malmö",
                "-IPTCDigest=new"
            ]
        );
        assert!(plan.notices.is_empty());
    }

    #[test]
    fn an_xmp_value_that_won_by_the_checksum_is_written_to_iim() {
        let mut metadata = edited(&[rating()]);
        let city = format!("{}ö", "a".repeat(31)); // 33 bytes: cut to 31 in IIM.
        iim_conflict(
            &mut metadata,
            "photoshop:City",
            text(&city),
            text("Malmö"),
            ConflictRule::Xmp,
        );
        let state = FileState {
            digest: IimDigest::Matches,
            ..differs()
        };
        let plan = run(&metadata, &state);
        assert_eq!(
            args(&plan),
            [
                "-XMP-xmp:Rating=3",
                "-IPTC:CodedCharacterSet=UTF8",
                format!("-IPTC:City={}", "a".repeat(31)).as_str(),
                "-IPTCDigest=new",
            ]
        );
        assert_eq!(
            plan.notices,
            [WriteNotice::Truncated {
                property: "photoshop:City".into()
            }]
        );
    }

    #[test]
    fn an_iim_only_value_is_written_to_xmp() {
        let mut metadata = edited(&[rating()]);
        metadata
            .insert_read("photoshop:Headline", text("Rubrik"), Origin::Iim)
            .unwrap();
        let plan = run(&metadata, &jpeg_with_iim());
        assert_eq!(
            args(&plan),
            [
                "-XMP-xmp:Rating=3",
                "-XMP-photoshop:Headline=Rubrik",
                "-IPTCDigest=new"
            ]
        );
    }

    #[test]
    fn a_conversion_does_not_write_the_old_iim_text_of_a_value_brought_in_line() {
        let mut metadata = edited(&[rating()]);
        iim_conflict(
            &mut metadata,
            "photoshop:City",
            text("Lund"),
            text("Malmö"),
            ConflictRule::Xmp,
        );
        let state = FileState {
            digest: IimDigest::Matches,
            iim_charset: CodedCharset::Absent,
            iim_values: vec![("photoshop:City".into(), text("Malmö"))],
            ..jpeg_with_iim()
        };
        let plan = run(&metadata, &state);
        assert_eq!(
            args(&plan),
            [
                "-XMP-xmp:Rating=3",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:City=Lund",
                "-IPTCDigest=new",
            ]
        );
        assert_eq!(plan.notices, [WriteNotice::IimConvertedToUtf8]);
    }

    #[test]
    fn an_open_conflict_is_still_untouched_while_others_are_brought_in_line() {
        let mut metadata = edited(&[rating()]);
        iim_conflict(
            &mut metadata,
            "photoshop:City",
            text("Lund"),
            text("Malmö"),
            ConflictRule::Open,
        );
        metadata
            .insert_read("photoshop:Headline", text("Rubrik"), Origin::Iim)
            .unwrap();
        let plan = run(&metadata, &missing_digest());
        assert_eq!(
            args(&plan),
            ["-XMP-xmp:Rating=3", "-XMP-photoshop:Headline=Rubrik"]
        );
        assert_eq!(
            plan.notices,
            [
                WriteNotice::ConflictKept {
                    property: "photoshop:City".into()
                },
                WriteNotice::DigestNotWritten,
            ]
        );
    }

    #[test]
    fn exif_conflicts_deprecated_properties_and_changed_properties_are_not_brought_in_line() {
        let mut metadata = edited(&[rating(), ("photoshop:Headline", text("Ny"))]);
        // Headline was changed: written once, with the new value.
        iim_conflict(
            &mut metadata,
            "photoshop:Headline",
            text("Gammal"),
            text("Annan"),
            ConflictRule::Iim,
        );
        metadata.set("photoshop:Headline", text("Ny")).unwrap();
        // A deprecated IIM property read from IIM only.
        metadata
            .insert_read("photoshop:Category", text("SPO"), Origin::Iim)
            .unwrap();
        // Description from Exif against IIM (rule `Exif`), and Creator from XMP against Exif.
        metadata
            .insert_read(
                "dc:description",
                lang_alt(&[("x-default", "Exif")]),
                Origin::Exif,
            )
            .unwrap();
        metadata.add_conflict(Conflict {
            property: "dc:description".into(),
            values: vec![
                (Origin::Iim, lang_alt(&[("x-default", "IIM")])),
                (Origin::Exif, lang_alt(&[("x-default", "Exif")])),
            ],
            rule: ConflictRule::Exif,
        });
        metadata
            .insert_read("dc:creator", list(&["Anna"]), Origin::XmpEmbedded)
            .unwrap();
        metadata.add_conflict(Conflict {
            property: "dc:creator".into(),
            values: vec![
                (Origin::XmpEmbedded, list(&["Anna"])),
                (Origin::Exif, list(&["Bertil"])),
            ],
            rule: ConflictRule::Xmp,
        });
        let state = FileState {
            exif_present: ExifPresent::default(),
            ..differs()
        };
        let plan = run(&metadata, &state);
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:Headline=Ny",
                "-XMP-xmp:Rating=3",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:Headline=Ny",
                "-IPTCDigest=new",
            ]
        );
    }

    #[test]
    fn nothing_is_brought_in_line_without_iim_or_for_a_raw_file() {
        let mut metadata = edited(&[rating()]);
        iim_conflict(
            &mut metadata,
            "photoshop:City",
            text("Lund"),
            text("Malmö"),
            ConflictRule::Iim,
        );
        let raw = FileState {
            kind: FileKind::Raw,
            ..differs()
        };
        assert_eq!(args(&run(&metadata, &raw)), ["-XMP-xmp:Rating=3"]);
        let no_iim = FileState {
            iim_present: false,
            ..differs()
        };
        assert_eq!(args(&run(&metadata, &no_iim)), ["-XMP-xmp:Rating=3"]);
    }

    #[test]
    fn exif_fields_are_written_only_when_present() {
        let metadata = edited(&[
            ("dc:description", lang_alt(&[("x-default", "Beskrivning")])),
            ("dc:rights", lang_alt(&[("x-default", "© 2026")])),
            ("dc:creator", list(&["Anna", "Bertil"])),
        ]);
        let plan = run(&metadata, &jpeg());
        assert_eq!(
            args(&plan),
            [
                "-XMP-dc:Creator=",
                "-XMP-dc:Creator=Anna",
                "-XMP-dc:Creator=Bertil",
                "-XMP-dc:Description-x-default=Beskrivning",
                "-XMP-dc:Rights-x-default=© 2026",
            ]
        );

        let state = FileState {
            exif_present: all_exif(),
            ..jpeg()
        };
        let plan = run(&metadata, &state);
        assert_eq!(
            args(&plan)[5..],
            [
                "-IFD0:Artist=Anna",
                "-ExifIFD:Photographer=Anna",
                "-IFD0:ImageDescription=Beskrivning",
                "-IFD0:Copyright=© 2026",
            ]
        );

        let only_artist = FileState {
            exif_present: ExifPresent {
                artist: true,
                ..ExifPresent::default()
            },
            ..jpeg()
        };
        let plan = run(&metadata, &only_artist);
        assert_eq!(args(&plan)[5..], ["-IFD0:Artist=Anna"]);
    }

    #[test]
    fn date_time_original_image_unique_id_and_gps_are_never_written() {
        let state = FileState {
            exif_present: all_exif(),
            ..jpeg()
        };
        let metadata = edited(&[
            ("photoshop:DateCreated", date("2026-11-04T10:15:30+01:00")),
            ("Iptc4xmpExt:DigImageGUID", text("guid")),
            (
                "Iptc4xmpExt:LocationCreated",
                Value::StructList(vec![fields(vec![("Iptc4xmpExt:City", text("Lund"))])]),
            ),
        ]);
        let plan = run(&metadata, &state);
        assert_eq!(
            args(&plan),
            [
                "-XMP-iptcExt:DigitalImageGUID=guid",
                "-XMP-iptcExt:LocationCreated=[{City=Lund}]",
                "-XMP-photoshop:DateCreated=2026-11-04T10:15:30+01:00",
            ]
        );
        assert!(plan.args.iter().all(|arg| !arg.contains("ExifIFD")
            && !arg.contains("GPS")
            && !arg.contains("ImageUniqueID")));
    }

    #[test]
    fn a_deprecated_iim_property_is_not_editable() {
        let mut metadata = edited(&[("photoshop:City", text("Lund"))]);
        metadata.set("photoshop:Category", text("SPO")).unwrap();
        assert_eq!(
            plan(&metadata, &jpeg_with_iim(), WriteSettings::default()),
            Err(PlanError::NotEditable("photoshop:Category".into()))
        );
        let metadata = cleared(&["photoshop:SupplementalCategories"]);
        assert_eq!(
            plan(&metadata, &jpeg_with_iim(), WriteSettings::default()),
            Err(PlanError::NotEditable(
                "photoshop:SupplementalCategories".into()
            ))
        );
    }

    #[test]
    fn a_raw_file_gets_xmp_arguments_only() {
        let state = FileState {
            kind: FileKind::Raw,
            iim_present: true,
            iim_charset: CodedCharset::Absent,
            iim_values: vec![("photoshop:City".into(), text("Göteborg"))],
            digest: IimDigest::Missing,
            exif_present: all_exif(),
        };
        let mut metadata = edited(&[
            ("photoshop:City", text("Lund")),
            ("dc:creator", list(&["Anna"])),
            ("photoshop:DateCreated", date("2026-11-04")),
        ]);
        metadata.clear("dc:description").unwrap();
        let plan = plan_with(&metadata, &state, true);
        assert_eq!(
            args(&plan),
            [
                "-XMP-dc:Creator=",
                "-XMP-dc:Creator=Anna",
                "-XMP-dc:Description=",
                "-XMP-photoshop:City=Lund",
                "-XMP-photoshop:DateCreated=2026-11-04",
            ]
        );
        assert!(plan.notices.is_empty());
    }

    #[test]
    fn a_line_break_asks_for_ec_and_escapes_every_argument() {
        let state = FileState {
            exif_present: all_exif(),
            ..jpeg_with_iim()
        };
        let metadata = edited(&[
            (
                "dc:description",
                lang_alt(&[("x-default", "Rad 1\nRad 2\r\nC:\\bild")]),
            ),
            ("photoshop:City", text("a\\b")),
        ]);
        let plan = run(&metadata, &state);
        assert_eq!(plan.options, ["-ec"]);
        assert_eq!(
            args(&plan),
            [
                "-XMP-dc:Description-x-default=Rad 1\\nRad 2\\r\\nC:\\\\bild",
                "-XMP-photoshop:City=a\\\\b",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:Caption-Abstract=Rad 1\\nRad 2\\r\\nC:\\\\bild",
                "-IPTC:City=a\\\\b",
                "-IPTCDigest=new",
                "-IFD0:ImageDescription=Rad 1\\nRad 2\\r\\nC:\\\\bild",
            ]
        );
        assert!(plan.args.iter().all(|arg| !arg.contains(['\n', '\r'])));
    }

    #[test]
    fn without_a_line_break_backslashes_are_not_escaped() {
        let plan = run(&edited(&[("photoshop:City", text("a\\b"))]), &jpeg());
        assert!(plan.options.is_empty());
        assert_eq!(args(&plan), ["-XMP-photoshop:City=a\\b"]);
    }

    #[test]
    fn the_digest_needs_xmp_after_the_write() {
        // Only clears and no XMP value left: IIM without XMP gets no checksum.
        let mut metadata = Metadata::new();
        metadata
            .insert_read("photoshop:City", text("Lund"), Origin::Iim)
            .unwrap();
        metadata.clear("photoshop:City").unwrap();
        let plan = run(&metadata, &jpeg_with_iim());
        assert_eq!(
            args(&plan),
            [
                "-XMP-photoshop:City=",
                "-IPTC:CodedCharacterSet=UTF8",
                "-IPTC:City=",
            ]
        );
    }
}
