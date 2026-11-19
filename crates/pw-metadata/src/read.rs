//! The reader: ExifTool's output for one image turned into [`Readings`] and reconciled (ADR 0007
//! §4, §5.4, §6.2, §6.5, §12).
//!
//! [`ExifTool::read_metadata`] asks ExifTool for the file's XMP, its raw IIM block with both IIM
//! checksums and the Exif fields the reconciliation uses, as JSON (`-j -n -b -struct -G1`):
//! - **Where the metadata is** (§4): embedded in a JPEG, TIFF or DNG; for a manufacturer RAW the
//!   sidecar `<basename>.xmp` holds the primary XMP and the XMP inside the RAW file is camera data.
//!   A `<basename>.xmp` next to a DNG is never read (§4.1, Q5) and a `<basename>.<ext>.xmp` is
//!   never read for any file (§4.2, Q8); both are reported. Names are compared without regard to
//!   case. On Windows the two names are looked up directly instead of listing the folder (T95):
//!   the info panel reads one image at each step, and listing a folder of thousands of files at
//!   every step is slow on a memory card or a network share. NTFS and FAT/exFAT compare names
//!   without regard to case, so `IMG_1.XMP` is found for `img_1.dng`; the path reported holds the
//!   name as it is in the folder (from `std::fs::canonicalize`). Other systems list the folder,
//!   because their file systems may compare names with regard to case.
//! - **XMP** is mapped by the property table's ExifTool names (`et_xmp`, and `et_tag` inside a
//!   structure, at any depth); keys the table does not know are ignored and stay in the file. A Lang
//!   Alt keeps the order of the JSON object. A date in ExifTool's form is read with
//!   [`IptcDate::parse_exiftool`]; one that cannot be read is reported as
//!   [`Notice::Unreadable`] with its text.
//! - **IIM**: the raw block (`base64:…`) is split into datasets and turned into values (T80); the
//!   checksum compares `IPTCDigest` with `CurrentIPTCDigest`.
//! - **Exif** text that is not UTF-8 arrives as `base64:…` and is decoded ASCII → UTF-8 → fallback
//!   (§6.2); the two parts of Exif Copyright are joined with a line feed (§6.5).
//!
//! ExifTool prints every value that looks like a number as a bare JSON number, whatever the
//! property's type (so a Headline `1984` or a City `2.50` would arrive as numbers, and `2.50` would
//! lose its last zero). The reader therefore asks for quoted values (`-api structformat=jsonq`,
//! owner's decision during T82) and builds each value by the kind of its property. A bare JSON
//! number still becomes a `Number`.
//!
//! [`MetadataRead::state`] is the [`FileState`] the write plan needs (T87), filled from the same
//! readings before the reconciliation takes them: for a JPEG, TIFF or DNG whether the file has a
//! readable IIM block (an unreadable one counts as none), its declared encoding, its values and its
//! checksum, and which Exif fields the plan may update exist (the "both ways" fields and the
//! Microsoft rating fields), and from the primary XMP whether it has `xmp:RatingPercent` and a flat
//! `lr:hierarchicalSubject`; for a manufacturer RAW no IIM and no Exif, because the plan never
//! writes the RAW file and its sidecar holds neither (§4.2).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value as Json};

use crate::date::{DateError, IptcDate};
use crate::iim::{self, CodedCharset, Fallback};
use crate::iim_read::{iim_values, parse_block};
use crate::properties::{self, Occurrence, ValueKind};
use crate::reconcile::{
    ExifFields, FileKind, IimDigest, IimRead, Notice, Readings, Reconciled, reconcile,
};
use crate::value::{LangAlt, Origin, StructValue, Value};
use crate::write_plan::{ExifPresent, FileState, XmpPresent};
use crate::{Error, ExifTool, Result};

/// The language of the Lang Alt entry ExifTool names without a language suffix.
const X_DEFAULT: &str = "x-default";
/// The marker ExifTool puts before binary data encoded in base64 (`-b`).
const BASE64_PREFIX: &str = "base64:";
/// The line `ExifTool::execute` puts between standard output and standard error.
const STDERR_MARKER: &str = "--- stderr ---\n";

/// The options shared by both ExifTool calls: JSON, no print conversion, binary data as base64,
/// structures as JSON objects, every value quoted, group 1 names.
const JSON_OPTIONS: &[&str] = &[
    "-j",
    "-n",
    "-b",
    "-struct",
    "-api",
    "structformat=jsonq",
    "-G1",
    "-XMP:all",
];

/// The tags read from the image file besides its XMP.
const IMAGE_TAGS: &[&str] = &[
    "-IPTC",
    "-Photoshop:IPTCDigest",
    "-CurrentIPTCDigest",
    "-IFD0:ImageDescription",
    "-IFD0:Artist",
    "-IFD0:Copyright",
    "-ExifIFD:Photographer",
    "-ExifIFD:ImageEditor",
    "-IFD0:Rating",
    "-IFD0:RatingPercent",
    "-ExifIFD:ImageUniqueID",
    "-ExifIFD:DateTimeOriginal",
    "-ExifIFD:SubSecTimeOriginal",
    "-ExifIFD:OffsetTimeOriginal",
    "-GPS:GPSLatitudeRef",
    "-GPS:GPSLatitude",
    "-GPS:GPSLongitudeRef",
    "-GPS:GPSLongitude",
    "-GPS:GPSAltitudeRef",
    "-GPS:GPSAltitude",
];

/// What [`ExifTool::read_metadata`] found in one image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataRead {
    pub reconciled: Reconciled,
    pub file_notices: Vec<FileNotice>,
    /// What the write plan needs to know about the file as it is now (see the module comment).
    pub state: FileState,
}

/// Something about the files next to the image, or the image's IIM block, the user is told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileNotice {
    /// A `<basename>.xmp` next to a DNG: written by another program, never read (§4.1, Q5).
    ForeignSidecarIgnored(PathBuf),
    /// A `<basename>.<ext>.xmp` (e.g. `DSC01234.ARW.xmp`): never read (§4.2, Q8).
    ExtSidecarIgnored(PathBuf),
    /// The IIM block could not be split into datasets; holds the reason. The file has no IIM
    /// values then.
    IimBlockUnreadable(String),
    /// A list property whose items have qualifiers RawStride cannot keep when it writes the list;
    /// shown, never written (ADR 0007 §8). Holds the XMP id.
    ListWithQualifiers(String),
}

impl ExifTool {
    /// Reads and reconciles the metadata of one image (see the module comment). `fallback` is the
    /// encoding for IIM and Exif text that is neither ASCII nor UTF-8 (§5.4, §6.2).
    ///
    /// The file kind comes from the extension: a manufacturer RAW (with a sidecar), DNG, JPEG or
    /// TIFF; anything else is [`Error::Unsupported`]. A missing or unreadable file is the error
    /// ExifTool reports.
    pub fn read_metadata(&mut self, image: &Path, fallback: Fallback) -> Result<MetadataRead> {
        let kind = file_kind(image)?;
        let file = self.read_json(&image_args(image), image)?;
        let sidecars = find_sidecars(image)?;

        let mut file_notices = Vec::new();
        if let Some(path) = sidecars.with_extension {
            file_notices.push(FileNotice::ExtSidecarIgnored(path));
        }
        let file_xmp = xmp_values(&file);
        // The lists with qualifiers, `xmp:RatingPercent` and a flat `lr:hierarchicalSubject` are
        // looked for in the primary XMP only, never in the camera XMP inside a RAW (ADR 0007 §6.3,
        // §8).
        let (primary, camera, qualified, xmp_present) = match kind {
            FileKind::Raw => match sidecars.plain {
                Some(path) => {
                    let sidecar = self.read_json(&sidecar_args(&path), &path)?;
                    (
                        xmp_values(&sidecar),
                        file_xmp,
                        qualified_lists(&sidecar),
                        xmp_present(&sidecar),
                    )
                }
                None => (
                    XmpRead::default(),
                    file_xmp,
                    Vec::new(),
                    XmpPresent::default(),
                ),
            },
            FileKind::Dng => {
                if let Some(path) = sidecars.plain {
                    file_notices.push(FileNotice::ForeignSidecarIgnored(path));
                }
                (
                    file_xmp,
                    XmpRead::default(),
                    qualified_lists(&file),
                    xmp_present(&file),
                )
            }
            FileKind::Jpeg | FileKind::Tiff => (
                file_xmp,
                XmpRead::default(),
                qualified_lists(&file),
                xmp_present(&file),
            ),
        };
        let (iim, iim_notice) = iim_read(&file, fallback);
        file_notices.extend(iim_notice);

        let primary_origin = if kind == FileKind::Raw {
            Origin::XmpSidecar
        } else {
            Origin::XmpEmbedded
        };
        let unreadable: Vec<Notice> = primary
            .unreadable
            .into_iter()
            .map(|(property, text)| (property, text, primary_origin))
            .chain(
                camera
                    .unreadable
                    .into_iter()
                    .map(|(property, text)| (property, text, Origin::XmpEmbedded)),
            )
            .map(|(property, text, origin)| Notice::Unreadable {
                property,
                origin,
                text,
            })
            .collect();

        let exif = exif_fields(&file, fallback);
        let state = file_state(kind, iim.as_ref(), &exif, &file, xmp_present);
        let mut reconciled = reconcile(Readings {
            kind,
            xmp: primary.values,
            camera_xmp: camera.values,
            iim,
            exif,
        });
        reconciled.notices.extend(unreadable);
        for id in qualified {
            reconciled.metadata.mark_qualified(id);
            file_notices.push(FileNotice::ListWithQualifiers(id.to_string()));
        }
        Ok(MetadataRead {
            reconciled,
            file_notices,
            state,
        })
    }

    /// Runs one JSON read and returns the object of its one file. No output means ExifTool could
    /// not read the file; its message is the error.
    fn read_json(&mut self, args: &[String], file: &Path) -> Result<Map<String, Json>> {
        let answer = self.execute(args)?;
        let (stdout, stderr) = answer
            .split_once(STDERR_MARKER)
            .unwrap_or((answer.as_str(), ""));
        if stdout.trim().is_empty() {
            let message = stderr.trim();
            return Err(Error::ExifTool(if message.is_empty() {
                format!("no answer for {}", file.display())
            } else {
                message.to_string()
            }));
        }
        let unreadable = |reason: String| {
            Error::ExifTool(format!(
                "could not read the answer for {}: {reason}",
                file.display()
            ))
        };
        let json: Json = serde_json::from_str(stdout).map_err(|e| unreadable(e.to_string()))?;
        match json {
            Json::Array(items) => match items.into_iter().next() {
                Some(Json::Object(object)) => Ok(object),
                _ => Err(unreadable("no object for the file".to_string())),
            },
            _ => Err(unreadable("not a JSON array".to_string())),
        }
    }
}

/// The [`FileState`] of a file of `kind` with the IIM `iim` (`None` when it has no readable IIM
/// block), the Exif fields `exif`, the image's JSON object `file` (for the Exif rating fields; a
/// manufacturer RAW ignores it) and what its primary XMP has, `xmp_present` (see the module
/// comment).
fn file_state(
    kind: FileKind,
    iim: Option<&IimRead>,
    exif: &ExifFields,
    file: &Map<String, Json>,
    xmp_present: XmpPresent,
) -> FileState {
    match (kind, iim) {
        (FileKind::Raw, _) => FileState {
            kind,
            iim_present: false,
            iim_charset: CodedCharset::Absent,
            iim_values: Vec::new(),
            digest: IimDigest::Missing,
            exif_present: ExifPresent::default(),
            xmp_present,
        },
        (_, iim) => FileState {
            kind,
            iim_present: iim.is_some(),
            iim_charset: iim.map_or(CodedCharset::Absent, |read| read.values.charset.clone()),
            iim_values: iim.map_or_else(Vec::new, |read| read.values.values.clone()),
            digest: iim.map_or(IimDigest::Missing, |read| read.digest),
            exif_present: ExifPresent {
                image_description: exif.image_description.is_some(),
                copyright: exif.copyright.is_some(),
                artist: exif.artist.is_some(),
                photographer: exif.photographer.is_some(),
                image_editor: exif.image_editor.is_some(),
                // Present whatever the value: a stale 0 is still updated (§6.3, Q16).
                rating: file.contains_key("IFD0:Rating"),
                rating_percent: file.contains_key("IFD0:RatingPercent"),
            },
            xmp_present,
        },
    }
}

/// The kind of file, by its extension (the lists in `lib.rs`).
fn file_kind(image: &Path) -> Result<FileKind> {
    if crate::supports_sidecar(image) {
        return Ok(FileKind::Raw);
    }
    let extension = image
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("dng") => Ok(FileKind::Dng),
        Some("jpg" | "jpeg") => Ok(FileKind::Jpeg),
        Some("tif" | "tiff") => Ok(FileKind::Tiff),
        _ => Err(Error::Unsupported(format!(
            "metadata cannot be read from {}",
            image.display()
        ))),
    }
}

fn image_args(image: &Path) -> Vec<String> {
    JSON_OPTIONS
        .iter()
        .chain(IMAGE_TAGS)
        .map(|arg| arg.to_string())
        .chain([image.to_string_lossy().into_owned()])
        .collect()
}

fn sidecar_args(sidecar: &Path) -> Vec<String> {
    JSON_OPTIONS
        .iter()
        .map(|arg| arg.to_string())
        .chain([sidecar.to_string_lossy().into_owned()])
        .collect()
}

/// The XMP files next to an image.
#[derive(Debug, Default)]
struct Sidecars {
    /// `<basename>.xmp`.
    plain: Option<PathBuf>,
    /// `<basename>.<ext>.xmp`.
    with_extension: Option<PathBuf>,
}

/// The `<basename>.xmp` sidecar of `image` with its name as stored in the folder (found without
/// regard to case, as [`find_sidecars`]); `None` when there is none. A folder that cannot be read
/// is the error. Used by the safe write to keep the sidecar's name (T132).
pub(crate) fn plain_sidecar(image: &Path) -> Result<Option<PathBuf>> {
    Ok(find_sidecars(image)?.plain)
}

/// Finds the sidecars of `image` in its folder, comparing names without regard to case. On
/// Windows the two names are looked up directly; the path returned holds the name as it is in the
/// folder (see the module comment).
#[cfg(windows)]
fn find_sidecars(image: &Path) -> Result<Sidecars> {
    let mut found = Sidecars::default();
    let (Some(stem), Some(name)) = (image.file_stem(), image.file_name()) else {
        return Ok(found);
    };
    let mut plain = stem.to_os_string();
    plain.push(".xmp");
    let mut with_extension = name.to_os_string();
    with_extension.push(".xmp");
    let folder = match image.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    found.plain = sidecar_as_stored(folder, &plain)?;
    if !with_extension.eq_ignore_ascii_case(&plain) {
        found.with_extension = sidecar_as_stored(folder, &with_extension)?;
    }
    Ok(found)
}

/// The file `name` in `folder` (looked up without regard to case by the file system), as
/// `folder.join(<its name as stored>)`; `None` when there is no such file. Only a missing file is
/// `None`; any other failure (a folder that cannot be read) is the error.
#[cfg(windows)]
fn sidecar_as_stored(folder: &Path, name: &std::ffi::OsStr) -> Result<Option<PathBuf>> {
    let path = folder.join(name);
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let canonical = std::fs::canonicalize(&path)?;
    Ok(Some(match canonical.file_name() {
        Some(stored) => folder.join(stored),
        None => path,
    }))
}

/// Finds the sidecars of `image` in its folder, comparing names without regard to case. The folder
/// is listed, because the file system may compare names with regard to case (see the module
/// comment).
#[cfg(not(windows))]
fn find_sidecars(image: &Path) -> Result<Sidecars> {
    let mut found = Sidecars::default();
    let (Some(stem), Some(name)) = (image.file_stem(), image.file_name()) else {
        return Ok(found);
    };
    let plain = format!("{}.xmp", stem.to_string_lossy()).to_lowercase();
    let with_extension = format!("{}.xmp", name.to_string_lossy()).to_lowercase();
    let folder = match image.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let entry_name = entry.file_name().to_string_lossy().to_lowercase();
        let slot = if entry_name == plain {
            &mut found.plain
        } else if entry_name == with_extension {
            &mut found.with_extension
        } else {
            continue;
        };
        if slot.is_none() && entry.path().is_file() {
            *slot = Some(entry.path());
        }
    }
    Ok(found)
}

/// The XMP values of one JSON object, and the dates that could not be read as `(property path,
/// text)`.
#[derive(Debug, Default)]
struct XmpRead {
    values: Vec<(String, Value)>,
    unreadable: Vec<(String, String)>,
}

/// The XMP of one ExifTool JSON object, keyed by the table's `et_xmp`, in table order.
fn xmp_values(object: &Map<String, Json>) -> XmpRead {
    let mut read = XmpRead::default();
    for def in properties::properties() {
        let Some(et_xmp) = def.et_xmp else {
            continue;
        };
        if let Some(value) = member(
            object,
            et_xmp,
            def.xmp_id,
            def.kind,
            def.occurrence,
            &mut read.unreadable,
        ) {
            read.values.push((def.xmp_id.to_string(), value));
        }
    }
    read
}

/// The XMP ids of the list properties (`et_xmp` set, `Multi`, not a structure) that have at least
/// one qualifier key in `object`, in table order (ADR 0007 §8). A qualifier key of a list named `N`
/// starts with `N`, goes on with an ASCII upper-case letter (ExifTool names a qualifier as the
/// property's tag name followed by the qualifier's name, e.g. `XMP-dc:SubjectType`) and is not the
/// `et_xmp` of any property of the table (so `XMP-iptcExt:PersonInImageWDetails` is not one).
fn qualified_lists(object: &Map<String, Json>) -> Vec<&'static str> {
    let is_property = |key: &str| properties::properties().any(|def| def.et_xmp == Some(key));
    properties::properties()
        .filter(|def| {
            def.occurrence == Occurrence::Multi && !matches!(def.kind, ValueKind::Struct(_))
        })
        .filter_map(|def| def.et_xmp.map(|et_xmp| (def.xmp_id, et_xmp)))
        .filter(|(_, et_xmp)| {
            object.keys().any(|key| {
                key.strip_prefix(et_xmp)
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|next| next.is_ascii_uppercase())
                    && !is_property(key)
            })
        })
        .map(|(xmp_id, _)| xmp_id)
        .collect()
}

/// What the primary XMP `object` has outside the property table (ADR 0007 §6.3 Q16, §8 Q15):
/// `xmp:RatingPercent`, and an `lr:hierarchicalSubject` that is flat: one text or a list of texts,
/// at least one entry, none holding `|`, and no qualifier key (a key that starts with its name and
/// goes on with an ASCII upper-case letter, as in [`qualified_lists`]).
fn xmp_present(object: &Map<String, Json>) -> XmpPresent {
    const HIERARCHY: &str = "XMP-lr:HierarchicalSubject";
    let is_flat = |entry: &Json| matches!(entry, Json::String(text) if !text.contains('|'));
    let entries_flat = match object.get(HIERARCHY) {
        Some(Json::Array(entries)) => !entries.is_empty() && entries.iter().all(is_flat),
        Some(entry @ Json::String(_)) => is_flat(entry),
        _ => false,
    };
    let has_qualifier = object.keys().any(|key| {
        key.strip_prefix(HIERARCHY)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|next| next.is_ascii_uppercase())
    });
    XmpPresent {
        rating_percent: object.contains_key("XMP-xmp:RatingPercent"),
        flat_hierarchical_subject: entries_flat && !has_qualifier,
    }
}

/// The value of the property or field named `et_name` in `object`; `None` when it is absent or
/// gives no value. A single Lang Alt is gathered from `et_name` (`x-default`) and every
/// `et_name-<language>` key, in the order of the object.
fn member(
    object: &Map<String, Json>,
    et_name: &str,
    path: &str,
    kind: ValueKind,
    occurrence: Occurrence,
    unreadable: &mut Vec<(String, String)>,
) -> Option<Value> {
    if kind == ValueKind::LangAlt && occurrence == Occurrence::Single {
        return lang_alt(object, et_name);
    }
    build(path, kind, occurrence, object.get(et_name)?, unreadable)
}

/// A Lang Alt from `et_name` and its `et_name-<language>` keys. A language may hold `-`
/// (`en-US`). An entry that is not a text cannot be built: the whole value is passed on as text
/// and refused by the reconciliation.
fn lang_alt(object: &Map<String, Json>, et_name: &str) -> Option<Value> {
    let mut entries = Vec::new();
    for (key, json) in object {
        let language = if key == et_name {
            X_DEFAULT
        } else if let Some(language) = key
            .strip_prefix(et_name)
            .and_then(|rest| rest.strip_prefix('-'))
            .filter(|language| !language.is_empty())
        {
            language
        } else {
            continue;
        };
        match scalar_text(json) {
            Some(text) => entries.push((language.to_string(), text)),
            None => return Some(unbuildable(json)),
        }
    }
    if entries.is_empty() {
        None
    } else {
        Some(Value::LangAlt(LangAlt::from_entries(entries)))
    }
}

/// Builds a value of `kind` from JSON (see the module comment). `None` for a JSON `null`, an empty
/// date, and a date that cannot be read (recorded in `unreadable`).
fn build(
    path: &str,
    kind: ValueKind,
    occurrence: Occurrence,
    json: &Json,
    unreadable: &mut Vec<(String, String)>,
) -> Option<Value> {
    match json {
        Json::Null => None,
        Json::Array(items) => {
            if let ValueKind::Struct(name) = kind
                && items.iter().all(Json::is_object)
            {
                let list = items
                    .iter()
                    .filter_map(Json::as_object)
                    .map(|object| build_struct(path, name, object, unreadable))
                    .collect();
                return Some(Value::StructList(list));
            }
            let texts: Option<Vec<String>> = items.iter().map(scalar_text).collect();
            Some(texts.map_or_else(|| unbuildable(json), Value::List))
        }
        Json::Object(object) => match kind {
            ValueKind::Struct(name) => {
                Some(Value::Struct(build_struct(path, name, object, unreadable)))
            }
            _ => Some(unbuildable(json)),
        },
        Json::Number(number) => Some(Value::Number(number.to_string())),
        Json::String(_) | Json::Bool(_) => {
            let text = scalar_text(json)?;
            match (kind, occurrence) {
                (ValueKind::Date, Occurrence::Single) => match IptcDate::parse_exiftool(&text) {
                    Ok(date) => Some(Value::Date(date)),
                    Err(DateError::Empty) => None,
                    Err(DateError::Invalid(_)) => {
                        unreadable.push((path.to_string(), text));
                        None
                    }
                },
                (ValueKind::Integer | ValueKind::Number, Occurrence::Single) => {
                    Some(Value::Number(text))
                }
                _ => Some(Value::Text(text)),
            }
        }
    }
}

/// The fields of a structure, by each field's `et_tag`, in the order of the structure's fields.
/// Unknown keys are ignored. `path` grows by `/<field XMP id>` for every level.
fn build_struct(
    path: &str,
    name: &str,
    object: &Map<String, Json>,
    unreadable: &mut Vec<(String, String)>,
) -> StructValue {
    let mut value = StructValue::default();
    let Some(def) = properties::struct_def(name) else {
        return value;
    };
    for field in def.fields {
        let Some(et_tag) = field.et_tag else {
            continue;
        };
        let field_path = format!("{path}/{}", field.xmp_id);
        if let Some(field_value) = member(
            object,
            et_tag,
            &field_path,
            field.kind,
            field.occurrence,
            unreadable,
        ) {
            value.fields.push((field.xmp_id.to_string(), field_value));
        }
    }
    value
}

/// The text of a JSON string, number or boolean; `None` for anything else.
fn scalar_text(json: &Json) -> Option<String> {
    match json {
        Json::String(text) => Some(text.clone()),
        Json::Number(number) => Some(number.to_string()),
        Json::Bool(true) => Some("True".to_string()),
        Json::Bool(false) => Some("False".to_string()),
        Json::Null | Json::Array(_) | Json::Object(_) => None,
    }
}

/// A JSON value that does not fit its property, passed on so the reconciliation refuses and
/// reports it (`Notice::Refused`): a structure whose one field has no name, holding the JSON text.
/// The model refuses it for every kind: a structure for a property that is not one, and a field
/// without a name for every structure. (A text would be accepted by a text property.)
fn unbuildable(json: &Json) -> Value {
    Value::Struct(StructValue {
        fields: vec![(String::new(), Value::Text(json.to_string()))],
    })
}

/// The value of a tag outside XMP, by its name without the group (`IPTCDigest` for
/// `Photoshop:IPTCDigest`).
fn tag<'a>(object: &'a Map<String, Json>, name: &str) -> Option<&'a Json> {
    object
        .iter()
        .find(|(key, _)| {
            key.split_once(':')
                .is_some_and(|(group, tag)| tag == name && !group.starts_with("XMP"))
        })
        .map(|(_, json)| json)
}

/// The IIM of the image: its values and checksum, or a notice when the block cannot be read. No
/// `IPTC` tag means no IIM.
fn iim_read(
    object: &Map<String, Json>,
    fallback: Fallback,
) -> (Option<IimRead>, Option<FileNotice>) {
    let Some(block) = tag(object, "IPTC").and_then(scalar_text) else {
        return (None, None);
    };
    let bytes = match block.strip_prefix(BASE64_PREFIX) {
        Some(encoded) => match decode_base64(encoded) {
            Ok(bytes) => bytes,
            Err(InvalidBase64) => {
                return (
                    None,
                    Some(FileNotice::IimBlockUnreadable(
                        "the IIM block is not valid base64".to_string(),
                    )),
                );
            }
        },
        None => block.into_bytes(),
    };
    match parse_block(&bytes) {
        Ok(datasets) => {
            let stored = tag(object, "IPTCDigest").and_then(scalar_text);
            let current = tag(object, "CurrentIPTCDigest").and_then(scalar_text);
            let read = IimRead {
                values: iim_values(&datasets, fallback),
                digest: iim_digest(stored.as_deref(), current.as_deref()),
            };
            (Some(read), None)
        }
        Err(error) => (
            None,
            Some(FileNotice::IimBlockUnreadable(error.to_string())),
        ),
    }
}

/// The IIM checksum (§5.1): `Missing` without a stored `IPTCDigest`, `Matches` when it equals
/// `CurrentIPTCDigest`, otherwise `Differs`. (A stored digest without a current one cannot happen
/// with an IIM block, because ExifTool computes the current digest from it; it would not match
/// anything, so it is `Differs`.)
fn iim_digest(stored: Option<&str>, current: Option<&str>) -> IimDigest {
    match (stored, current) {
        (None, _) => IimDigest::Missing,
        (Some(stored), Some(current)) if stored == current => IimDigest::Matches,
        (Some(_), _) => IimDigest::Differs,
    }
}

/// The Exif fields of the image (§6.2, §6.5). Blank values are `None`.
fn exif_fields(object: &Map<String, Json>, fallback: Fallback) -> ExifFields {
    let field = |name: &str| {
        tag(object, name)
            .and_then(|json| exif_text(json, fallback))
            .filter(|text| !iim::is_blank(text))
    };
    ExifFields {
        image_description: field("ImageDescription"),
        copyright: field("Copyright")
            .map(|text| join_copyright(&text))
            .filter(|text| !iim::is_blank(text)),
        artist: field("Artist"),
        photographer: field("Photographer"),
        image_editor: field("ImageEditor"),
        image_unique_id: field("ImageUniqueID"),
        date_time_original: field("DateTimeOriginal"),
        sub_sec_time_original: field("SubSecTimeOriginal"),
        offset_time_original: field("OffsetTimeOriginal"),
        gps_latitude: field("GPSLatitude"),
        gps_latitude_ref: field("GPSLatitudeRef"),
        gps_longitude: field("GPSLongitude"),
        gps_longitude_ref: field("GPSLongitudeRef"),
        gps_altitude: field("GPSAltitude"),
        gps_altitude_ref: field("GPSAltitudeRef"),
    }
}

/// The text of an Exif value. Bytes that are not UTF-8 arrive as `base64:…` and are decoded
/// ASCII → UTF-8 → fallback (§6.2). Text after a `base64:` that is not valid base64 is kept as it
/// is.
fn exif_text(json: &Json, fallback: Fallback) -> Option<String> {
    let text = scalar_text(json)?;
    if let Some(encoded) = text.strip_prefix(BASE64_PREFIX)
        && let Ok(bytes) = decode_base64(encoded)
    {
        return Some(iim::decode(&bytes, &CodedCharset::Absent, fallback).text);
    }
    Some(text)
}

/// Exif Copyright (§6.5): trailing NULs and spaces are dropped, and the parts separated by NUL
/// (photographer, editor) are joined with a line feed.
fn join_copyright(text: &str) -> String {
    text.trim_end_matches(['\0', ' ']).replace('\0', "\n")
}

/// Base64 that is not RFC 4648 §4 (standard alphabet, padded to a multiple of four).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InvalidBase64;

/// Decodes base64 (RFC 4648 §4) as ExifTool writes it: the standard alphabet, padded with `=` to
/// a multiple of four characters. Anything else is an error.
fn decode_base64(text: &str) -> std::result::Result<Vec<u8>, InvalidBase64> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(InvalidBase64);
    }
    let (groups, _) = bytes.as_chunks::<4>();
    let mut out = Vec::with_capacity(groups.len() * 3);
    for (index, group) in groups.iter().enumerate() {
        let padding = group.iter().rev().take_while(|&&b| b == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != groups.len()) {
            return Err(InvalidBase64);
        }
        // Every character before the trailing `=` must be in the alphabet (a `=` there is an
        // error); each `=` stands for six zero bits.
        let mut word = 0u32;
        for (position, &byte) in group.iter().enumerate() {
            let bits = if position < 4 - padding {
                sextet(byte)?
            } else {
                0
            };
            word = (word << 6) | u32::from(bits);
        }
        let [_, first, second, third] = word.to_be_bytes();
        out.push(first);
        if padding < 2 {
            out.push(second);
        }
        if padding < 1 {
            out.push(third);
        }
    }
    Ok(out)
}

/// The value of one base64 character.
fn sextet(byte: u8) -> std::result::Result<u8, InvalidBase64> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(InvalidBase64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::{Time, Zone};
    use crate::iim_read::IimNotice;
    use crate::value::ValueError;

    fn object(json: &str) -> Map<String, Json> {
        match serde_json::from_str::<Json>(json) {
            Ok(Json::Object(object)) => object,
            other => panic!("not a JSON object: {other:?}"),
        }
    }

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    fn lang_alt(entries: &[(&str, &str)]) -> Value {
        Value::LangAlt(LangAlt::from_entries(
            entries
                .iter()
                .map(|(language, text)| (language.to_string(), text.to_string()))
                .collect(),
        ))
    }

    fn structure(fields: Vec<(&str, Value)>) -> StructValue {
        StructValue {
            fields: fields
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect(),
        }
    }

    fn value_of<'a>(read: &'a XmpRead, id: &str) -> Option<&'a Value> {
        read.values
            .iter()
            .find(|(property, _)| property == id)
            .map(|(_, value)| value)
    }

    #[test]
    fn qualified_lists_finds_a_qualifier_key() {
        let object = object(
            r#"{"XMP-dc:Subject": ["San Francisco", "cat"], "XMP-dc:SubjectType": "location"}"#,
        );
        assert_eq!(qualified_lists(&object), ["dc:subject"]);
    }

    #[test]
    fn qualified_lists_ignores_other_properties_and_languages() {
        let object = object(
            r#"{"XMP-iptcExt:PersonInImage": ["A"],
                "XMP-iptcExt:PersonInImageWDetails": [{"PersonName": "A"}],
                "XMP-dc:Title-sv": "Titel",
                "XMP-dc:Subject": ["cat"]}"#,
        );
        assert_eq!(qualified_lists(&object), Vec::<&str>::new());
    }

    #[test]
    fn xmp_present_finds_a_rating_percentage_and_a_flat_hierarchy() {
        let present = xmp_present(&object(r#"{"XMP-xmp:RatingPercent": "50"}"#));
        assert!(present.rating_percent);
        assert!(!present.flat_hierarchical_subject);
        let flat = |json: &str| xmp_present(&object(json)).flat_hierarchical_subject;
        assert!(flat(r#"{"XMP-lr:HierarchicalSubject": ["a", "b"]}"#));
        assert!(flat(r#"{"XMP-lr:HierarchicalSubject": "a"}"#));
        assert!(!flat(r#"{"XMP-lr:HierarchicalSubject": ["a", "x|y"]}"#));
        assert!(!flat(r#"{"XMP-lr:HierarchicalSubject": []}"#));
        assert!(!flat(
            r#"{"XMP-lr:HierarchicalSubject": ["a"], "XMP-lr:HierarchicalSubjectLang": "sv"}"#
        ));
        assert!(!xmp_present(&object(r#"{"XMP-lr:HierarchicalSubject": ["a"]}"#)).rating_percent);
        assert_eq!(xmp_present(&object("{}")), XmpPresent::default());
    }

    #[test]
    fn base64_decodes_the_rfc_4648_vectors() {
        for (encoded, decoded) in [
            ("", ""),
            ("Zg==", "f"),
            ("Zm8=", "fo"),
            ("Zm9v", "foo"),
            ("Zm9vYg==", "foob"),
            ("Zm9vYmE=", "fooba"),
            ("Zm9vYmFy", "foobar"),
        ] {
            assert_eq!(
                decode_base64(encoded),
                Ok(decoded.as_bytes().to_vec()),
                "{encoded:?}"
            );
        }
        // The whole alphabet, including `+` and `/`.
        assert_eq!(decode_base64("+/+/"), Ok(vec![0xfb, 0xff, 0xbf]));
        assert_eq!(
            decode_base64("AAECAwQFBgcICQoLDA0ODw=="),
            Ok((0u8..16).collect())
        );
    }

    #[test]
    fn invalid_base64_is_an_error_and_never_a_panic() {
        for encoded in [
            "Z", "Zg", "Zg=", "Zm9vY", "Z===", "====", "Zg==Zg==", "Z=g=", "=Zg=", "Zm9v!A==",
            "Zm 9", "Zm-_", "äbc", "Zm9v\n",
        ] {
            assert_eq!(decode_base64(encoded), Err(InvalidBase64), "{encoded:?}");
        }
    }

    /// A hand-written ExifTool answer: a Lang Alt in three languages (not in alphabetical order),
    /// a list, a structure, a list of structures with a Lang Alt field, a date, a number, a text
    /// of digits, and keys the table does not know.
    const XMP_JSON: &str = r#"{
        "SourceFile": "a.jpg",
        "XMP-x:XMPToolkit": "Image::ExifTool 13.59",
        "XMP-dc:Description-sv": "Hej",
        "XMP-dc:Description": "Hello",
        "XMP-dc:Description-en-US": "Hi",
        "XMP-dc:Subject": ["k2", "k1"],
        "XMP-iptcCore:CreatorContactInfo": {
            "CiEmailWork": "a@b.se",
            "CiAdrCity": "Göteborg",
            "CiUnknown": "kept in the file"
        },
        "XMP-iptcExt:LocationCreated": [{
            "City": "Malmö",
            "LocationName": "Torget",
            "LocationName-sv": "Torget sv"
        }, {
            "City": "Lund"
        }],
        "XMP-photoshop:DateCreated": "2026:11:04 10:15:30.5+01:00",
        "XMP-photoshop:Headline": "1984",
        "XMP-xmp:Rating": "2.5",
        "XMP-foo:Bar": "unknown"
    }"#;

    #[test]
    fn json_becomes_values_of_the_model() {
        let read = xmp_values(&object(XMP_JSON));

        assert_eq!(
            value_of(&read, "dc:description"),
            Some(&lang_alt(&[
                ("sv", "Hej"),
                ("x-default", "Hello"),
                ("en-US", "Hi")
            ])),
            "the order of the JSON object is kept"
        );
        assert_eq!(
            value_of(&read, "dc:subject"),
            Some(&Value::List(vec!["k2".to_string(), "k1".to_string()]))
        );
        assert_eq!(
            value_of(&read, "Iptc4xmpCore:CreatorContactInfo"),
            Some(&Value::Struct(structure(vec![
                ("Iptc4xmpCore:CiAdrCity", text("Göteborg")),
                ("Iptc4xmpCore:CiEmailWork", text("a@b.se")),
            ])))
        );
        assert_eq!(
            value_of(&read, "Iptc4xmpExt:LocationCreated"),
            Some(&Value::StructList(vec![
                structure(vec![
                    (
                        "Iptc4xmpExt:LocationName",
                        lang_alt(&[("x-default", "Torget"), ("sv", "Torget sv")])
                    ),
                    ("Iptc4xmpExt:City", text("Malmö")),
                ]),
                structure(vec![("Iptc4xmpExt:City", text("Lund"))]),
            ]))
        );
        assert_eq!(
            value_of(&read, "photoshop:DateCreated"),
            Some(&Value::Date(IptcDate {
                year: 2026,
                month: Some(11),
                day: Some(4),
                time: Some(Time {
                    hour: 10,
                    minute: 15,
                    second: Some(30),
                    fraction: Some("5".to_string()),
                    zone: Some(Zone::Offset(60)),
                }),
            }))
        );
        assert_eq!(
            value_of(&read, "photoshop:Headline"),
            Some(&text("1984")),
            "a quoted number for a text property stays text"
        );
        assert_eq!(
            value_of(&read, "xmp:Rating"),
            Some(&Value::Number("2.5".to_string()))
        );
        assert_eq!(read.values.len(), 7, "unknown keys are ignored: {read:?}");
        assert!(read.unreadable.is_empty());
    }

    #[test]
    fn a_bare_json_number_becomes_a_number_with_its_text() {
        let read = xmp_values(&object(r#"{"XMP-xmp:Rating": 2.5, "XMP-dc:Subject": [7]}"#));
        assert_eq!(
            value_of(&read, "xmp:Rating"),
            Some(&Value::Number("2.5".to_string()))
        );
        assert_eq!(
            value_of(&read, "dc:subject"),
            Some(&Value::List(vec!["7".to_string()]))
        );
    }

    #[test]
    fn an_unreadable_date_is_reported_with_its_text_and_left_out() {
        let read = xmp_values(&object(
            r#"{
                "XMP-photoshop:DateCreated": "2026-11-04",
                "XMP-iptcExt:ArtworkOrObject": [{"AODateCreated": "yesterday", "AOTitle": "T"}]
            }"#,
        ));
        assert_eq!(value_of(&read, "photoshop:DateCreated"), None);
        assert_eq!(
            value_of(&read, "Iptc4xmpExt:ArtworkOrObject"),
            Some(&Value::StructList(vec![structure(vec![(
                "Iptc4xmpExt:AOTitle",
                lang_alt(&[("x-default", "T")])
            )])]))
        );
        // In the order of the property table, where Artwork or Object comes first.
        assert_eq!(
            read.unreadable,
            [
                (
                    "Iptc4xmpExt:ArtworkOrObject/Iptc4xmpExt:AODateCreated".to_string(),
                    "yesterday".to_string()
                ),
                (
                    "photoshop:DateCreated".to_string(),
                    "2026-11-04".to_string()
                ),
            ]
        );
    }

    #[test]
    fn an_empty_date_is_absent_and_not_reported() {
        let read = xmp_values(&object(r#"{"XMP-photoshop:DateCreated": " "}"#));
        assert!(read.values.is_empty());
        assert!(read.unreadable.is_empty());
    }

    #[test]
    fn a_value_that_cannot_be_built_is_refused_by_the_reconciliation() {
        let read = xmp_values(&object(
            r#"{
                "XMP-photoshop:City": {"x": "y"},
                "XMP-photoshop:Headline": ["a", {"b": "c"}],
                "XMP-dc:Subject": "one",
                "XMP-xmp:Rating": "high",
                "XMP-iptcCore:CreatorContactInfo": "text",
                "XMP-iptcExt:LocationCreated": [1, {"City": "Lund"}],
                "XMP-dc:Title": {"x-default": "not a Lang Alt in this form"}
            }"#,
        ));
        let reconciled = reconcile(Readings {
            kind: FileKind::Jpeg,
            xmp: read.values,
            camera_xmp: Vec::new(),
            iim: None,
            exif: ExifFields::default(),
        });
        let mut refused: Vec<&str> = reconciled
            .notices
            .iter()
            .filter_map(|notice| match notice {
                Notice::Refused {
                    property,
                    origin: Origin::XmpEmbedded,
                    error: ValueError::Kind { .. },
                } => Some(property.as_str()),
                _ => None,
            })
            .collect();
        refused.sort_unstable();
        assert_eq!(
            refused,
            [
                "Iptc4xmpCore:CreatorContactInfo",
                "Iptc4xmpExt:LocationCreated",
                "dc:subject",
                "dc:title",
                "photoshop:City",
                "photoshop:Headline",
                "xmp:Rating",
            ]
        );
        assert_eq!(reconciled.metadata, crate::value::Metadata::new());
    }

    #[test]
    fn the_digest_cases() {
        assert_eq!(iim_digest(Some("ab12"), Some("ab12")), IimDigest::Matches);
        assert_eq!(iim_digest(Some("ab12"), Some("cd34")), IimDigest::Differs);
        assert_eq!(iim_digest(None, Some("cd34")), IimDigest::Missing);
        assert_eq!(iim_digest(None, None), IimDigest::Missing);
        assert_eq!(iim_digest(Some("ab12"), None), IimDigest::Differs);
    }

    /// The IIM block ExifTool 13.59 wrote for City "Göteborg" and Keywords k1, k2 with
    /// `CodedCharacterSet=UTF8`.
    const IIM_BLOCK: &str =
        "base64:HAFaAAMbJUccAQAAAgAEHAIZAAJrMRwCGQACazIcAloACUfDtnRlYm9yZxwCAAACAAQ=";

    #[test]
    fn the_iim_block_is_decoded_into_values_with_its_digest() {
        let json = format!(
            r#"{{"IPTC:IPTC": "{IIM_BLOCK}",
                "Photoshop:IPTCDigest": "bf58d828d4da0bcfdd224dd4b9a1a136",
                "File:CurrentIPTCDigest": "bf58d828d4da0bcfdd224dd4b9a1a136"}}"#
        );
        let (read, notice) = iim_read(&object(&json), Fallback::Windows1252);
        assert_eq!(notice, None);
        let Some(read) = read else {
            panic!("no IIM read");
        };
        assert_eq!(read.digest, IimDigest::Matches);
        assert_eq!(read.values.charset, CodedCharset::Utf8);
        assert_eq!(
            read.values.values,
            [
                (
                    "dc:subject".to_string(),
                    Value::List(vec!["k1".to_string(), "k2".to_string()])
                ),
                ("photoshop:City".to_string(), text("Göteborg")),
            ]
        );
    }

    #[test]
    fn an_unreadable_iim_block_is_a_file_notice_and_no_iim() {
        // A dataset that claims 9 bytes where only 2 follow.
        let (read, notice) = iim_read(
            &object(r#"{"IPTC:IPTC": "base64:HAJaAAlhYg=="}"#),
            Fallback::Windows1252,
        );
        assert_eq!(read, None);
        assert!(
            matches!(notice, Some(FileNotice::IimBlockUnreadable(_))),
            "{notice:?}"
        );
        let (read, notice) = iim_read(
            &object(r#"{"IPTC:IPTC": "base64:not base64"}"#),
            Fallback::Windows1252,
        );
        assert_eq!(read, None);
        assert!(
            matches!(notice, Some(FileNotice::IimBlockUnreadable(_))),
            "{notice:?}"
        );
        assert_eq!(
            iim_read(&object(r#"{"XMP-dc:Title": "x"}"#), Fallback::Windows1252),
            (None, None)
        );
    }

    #[test]
    fn iim_text_without_a_declared_encoding_uses_the_fallback() {
        // City "Malmö" in Windows-1252 without 1:90: decoded by the fallback and marked converted.
        let json = r#"{"IPTC:IPTC": "base64:HAJaAAVNYWxt9g=="}"#;
        let (read, _) = iim_read(&object(json), Fallback::Windows1252);
        let Some(read) = read else {
            panic!("no IIM read");
        };
        assert_eq!(read.digest, IimDigest::Missing);
        assert_eq!(
            read.values.values,
            [("photoshop:City".to_string(), text("Malmö"))]
        );
        assert_eq!(
            read.values.notices,
            [IimNotice::Converted {
                property: "photoshop:City".to_string()
            }]
        );
    }

    #[test]
    fn copyright_parts_are_joined_with_a_line_feed() {
        assert_eq!(
            join_copyright("Photographer\0Editor\0"),
            "Photographer\nEditor"
        );
        assert_eq!(join_copyright("© A\0\0  \0"), "© A");
        assert_eq!(join_copyright("© A"), "© A");
        assert_eq!(join_copyright(" \0Editor"), " \nEditor");

        let exif = exif_fields(
            &object(r#"{"IFD0:Copyright": "© A\u0000© B\u0000", "IFD0:Artist": " \u0000"}"#),
            Fallback::Windows1252,
        );
        assert_eq!(exif.copyright.as_deref(), Some("© A\n© B"));
        assert_eq!(exif.artist, None, "a blank value is None");
        let blank = exif_fields(
            &object(r#"{"IFD0:Copyright": "\u0000 \u0000"}"#),
            Fallback::Windows1252,
        );
        assert_eq!(blank.copyright, None);
    }

    #[test]
    fn exif_bytes_that_are_not_utf8_are_decoded_by_the_fallback() {
        // "Café €" in Windows-1252: 43 61 66 E9 20 80.
        let json = object(
            r#"{
                "IFD0:Artist": "base64:Q2Fm6SCA",
                "IFD0:ImageDescription": "Plain",
                "ExifIFD:Photographer": "Åsa",
                "ExifIFD:ImageUniqueID": "0123abcd",
                "ExifIFD:DateTimeOriginal": "2026:11:04 10:15:30",
                "ExifIFD:SubSecTimeOriginal": "050",
                "ExifIFD:OffsetTimeOriginal": "+01:00",
                "XMP-tiff:Artist": "not Exif"
            }"#,
        );
        let exif = exif_fields(&json, Fallback::Windows1252);
        assert_eq!(
            exif,
            ExifFields {
                image_description: Some("Plain".to_string()),
                copyright: None,
                artist: Some("Café €".to_string()),
                photographer: Some("Åsa".to_string()),
                image_editor: None,
                image_unique_id: Some("0123abcd".to_string()),
                date_time_original: Some("2026:11:04 10:15:30".to_string()),
                sub_sec_time_original: Some("050".to_string()),
                offset_time_original: Some("+01:00".to_string()),
                gps_latitude: None,
                gps_latitude_ref: None,
                gps_longitude: None,
                gps_longitude_ref: None,
                gps_altitude: None,
                gps_altitude_ref: None,
            }
        );
        let latin1 = exif_fields(&json, Fallback::Latin1);
        assert_eq!(latin1.artist.as_deref(), Some("Café \u{80}"));
        // UTF-8 bytes in base64 are read as UTF-8, whatever the fallback.
        let utf8 = exif_fields(
            &object(r#"{"IFD0:Artist": "base64:w4VzYQ=="}"#),
            Fallback::Latin1,
        );
        assert_eq!(utf8.artist.as_deref(), Some("Åsa"));
    }

    #[test]
    fn the_file_kind_comes_from_the_extension() {
        for (name, kind) in [
            ("a.ARW", FileKind::Raw),
            ("a.rw2", FileKind::Raw),
            ("a.Dng", FileKind::Dng),
            ("a.jpg", FileKind::Jpeg),
            ("a.JPEG", FileKind::Jpeg),
            ("a.tif", FileKind::Tiff),
            ("a.TIFF", FileKind::Tiff),
        ] {
            assert_eq!(file_kind(Path::new(name)).ok(), Some(kind), "{name}");
        }
        for name in ["a.png", "a.xmp", "a"] {
            assert!(
                matches!(file_kind(Path::new(name)), Err(Error::Unsupported(_))),
                "{name}"
            );
        }
    }
}
