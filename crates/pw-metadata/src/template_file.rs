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
//!
//! **The file** (U13-Q1, Q2, Q12; ADR 0007 §10, §17.1): a template is saved as a plain XMP file
//! with only standard values, plus one RawStride property, `rawstride:AddTo` (an `rdf:Bag` of the
//! XMP ids whose + box is checked), in the namespace `rawstride` =
//! [`RAWSTRIDE_NAMESPACE`] (a name only, never fetched, never changed).
//! [`ExifTool::write_template_file`] saves one and [`ExifTool::read_template_file`] reads one back,
//! also a file another program wrote. ExifTool reads `rawstride:AddTo` without any setting but
//! writes it only with the user-defined tag of [`EXIFTOOL_CONFIG`]: an [`ExifTool`] used for
//! templates must be started with `-config <path>` as its first arguments, the path that
//! [`write_exiftool_config`] gives (`ExifTool::start_with(program, &["-config", path])`; with
//! `perl`, after the script). An ExifTool started without it cannot save a template (the save
//! fails on ExifTool's warning); reading works either way.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value as Json;

use crate::ExifTool;
use crate::iim::CodedCharset;
use crate::properties::{self, Occurrence, ValueKind};
use crate::read::{JSON_OPTIONS, xmp_values};
use crate::reconcile::{FileKind, IimDigest};
use crate::template::{
    LOCATION_CREATED, Mode, Template, TemplateField, has_content, sort_fields, x_default,
};
use crate::value::{self, Metadata, Value, ValueError};
use crate::write_plan::{self, ExifPresent, FileState, PlanError, WriteSettings, XmpPresent};

/// The URI of RawStride's template namespace (prefix `rawstride`; owner, 2026-11-23, ADR 0007
/// §10). An identifier only, never fetched; never changed, or older template files lose their +
/// boxes.
pub const RAWSTRIDE_NAMESPACE: &str = "https://github.com/Bokehlicious/RawStride/ns/template/1.0/";

/// The ExifTool config that defines `rawstride:AddTo` (group `XMP-rawstride`, an `rdf:Bag` of
/// strings), so ExifTool can write it. Given with `-config` (see the module comment).
pub const EXIFTOOL_CONFIG: &str = "\
# RawStride's metadata template namespace (written by RawStride; do not edit).
%Image::ExifTool::UserDefined = (
    'Image::ExifTool::XMP::Main' => {
        rawstride => {
            SubDirectory => {
                TagTable => 'Image::ExifTool::UserDefined::rawstride',
            },
        },
    },
);
%Image::ExifTool::UserDefined::rawstride = (
    GROUPS => { 0 => 'XMP', 1 => 'XMP-rawstride', 2 => 'Image' },
    NAMESPACE => { 'rawstride' => 'https://github.com/Bokehlicious/RawStride/ns/template/1.0/' },
    WRITABLE => 'string',
    AddTo => { List => 'Bag' },
);
1;
";

/// The name of the config file [`write_exiftool_config`] writes.
const CONFIG_FILE_NAME: &str = "rawstride-exiftool.config";

/// `rawstride:AddTo` by its ExifTool name.
const ADD_TO: &str = "XMP-rawstride:AddTo";

/// The subfolder of the template's folder where a new file is written before it replaces the
/// template (see [`ExifTool::write_template_file`]).
const SAVING_FOLDER: &str = ".saving";

/// Two numbers compared as numbers are equal within this part of the larger.
const NUMBER_TOLERANCE: f64 = 1e-9;

/// Makes the names of new files unique within this process.
static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Writes [`EXIFTOOL_CONFIG`] to `rawstride-exiftool.config` in `folder` (made when needed) and
/// returns the file's path. The file is written only when it is missing or its content differs,
/// so its modification time stays as it is otherwise. An [`ExifTool`] used for templates is
/// started with `-config <that path>` as its first arguments (see the module comment).
pub fn write_exiftool_config(folder: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(folder)?;
    let path = folder.join(CONFIG_FILE_NAME);
    match std::fs::read(&path) {
        Ok(content) if content == EXIFTOOL_CONFIG.as_bytes() => {}
        Ok(_) => std::fs::write(&path, EXIFTOOL_CONFIG)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::write(&path, EXIFTOOL_CONFIG)?
        }
        Err(error) => return Err(error),
    }
    Ok(path)
}

/// Why a template file could not be saved or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateFileError {
    /// The file's name does not end in `.xmp` (any case).
    NotXmpName(PathBuf),
    /// The template has no field once [`TemplateFile::new`] has run: an empty template is not
    /// saved.
    Empty,
    /// A value `Metadata::set` refuses (for example more than three Image Creators).
    Value(ValueError),
    /// The write plan refused the values.
    Plan(PlanError),
    /// ExifTool failed, did not answer or did not write the file; holds its text.
    ExifTool(String),
    /// A file operation failed: the path and the error's text.
    File { path: PathBuf, reason: String },
    /// The file read back after saving differs from the template; holds what differs.
    Differs(String),
    /// The file to read does not exist.
    NotFound(PathBuf),
    /// ExifTool could not read the file (`ExifTool:Error`); holds its text.
    Unreadable { path: PathBuf, reason: String },
    /// The file is not XMP (a text file or a JPEG named `.xmp`); holds the type ExifTool found.
    NotXmp { path: PathBuf, file_type: String },
    /// The template's folder has a `%` in its path, which ExifTool's `-o` reads as a format code
    /// (`%d`, `%f`, …) with no way to escape it; holds the folder.
    PercentInFolder(PathBuf),
}

impl fmt::Display for TemplateFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateFileError::NotXmpName(path) => {
                write!(f, "{} is not the name of an .xmp file", path.display())
            }
            TemplateFileError::Empty => write!(f, "the template has no field to save"),
            TemplateFileError::Value(error) => write!(f, "{error}"),
            TemplateFileError::Plan(error) => write!(f, "{error}"),
            TemplateFileError::ExifTool(text) => write!(f, "ExifTool: {text}"),
            TemplateFileError::File { path, reason } => {
                write!(f, "{}: {reason}", path.display())
            }
            TemplateFileError::Differs(what) => {
                write!(f, "the saved file differs from the template: {what}")
            }
            TemplateFileError::NotFound(path) => write!(f, "{} does not exist", path.display()),
            TemplateFileError::Unreadable { path, reason } => {
                write!(f, "{} could not be read: {reason}", path.display())
            }
            TemplateFileError::NotXmp { path, file_type } => {
                write!(f, "{} is not an XMP file ({file_type})", path.display())
            }
            TemplateFileError::PercentInFolder(folder) => write!(
                f,
                "a template cannot be saved in {}, as its path holds a % sign",
                folder.display()
            ),
        }
    }
}

impl std::error::Error for TemplateFileError {}

/// What [`ExifTool::read_template_file`] read, so the template window can tell the user which
/// fields of the file were not loaded (U13c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRead {
    /// The template, made by [`TemplateFile::new`].
    pub template: TemplateFile,
    /// The XMP ids of the file's properties of the table a template cannot hold ([`in_template`]:
    /// Rating, Label, a deprecated IIM property), never applied.
    pub skipped: Vec<String>,
    /// The XMP ids of the properties [`TemplateFile::new`] dropped as a whole (for example a Lang
    /// Alt with only `en-US`). A part it drops (a second Location Created entry, a blank list
    /// entry) is not named.
    pub dropped: Vec<String>,
    /// The values that could not be read as `(property path, text)`: a date ExifTool gives in a
    /// form that cannot be read, and a value `Metadata::set` refuses for its kind (with the
    /// error's text).
    pub unreadable: Vec<(String, String)>,
}

impl ExifTool {
    /// Saves `template` as a plain XMP file at `path` (see the module comment).
    ///
    /// The template is passed through [`TemplateFile::new`] first, so only checked fields with
    /// content and their + boxes are written; one with no field left is
    /// [`TemplateFileError::Empty`]. The values go through `Metadata::set` and the write plan (XMP
    /// only, as for a sidecar), plus one `rawstride:AddTo` entry per + box. ExifTool writes them
    /// with `-o` to a new file with a name of its own (`<process id>-<counter>.xmp`) in the
    /// subfolder `.saving` of `path`'s folder, so two saves at once never share a file and a file
    /// left by a crash is not among the templates. ExifTool reads a `%` after `-o` as a format
    /// code that cannot be escaped, so the new file's name never holds the template's name (which
    /// may have a `%`; only the rename uses it), and a folder whose path holds a `%` is
    /// [`TemplateFileError::PercentInFolder`]. The write counts only when ExifTool created
    /// exactly one file and printed no warning and no error (an ExifTool started without the
    /// config only warns that `AddTo` is not defined). The new file is then read back and compared
    /// with the template (a number as a number, as XMP stores coordinates in another form).
    ///
    /// Only then the new file replaces `path` (an existing file too; asking first is the window's,
    /// U13-Q2). The rename fails while another program holds `path` open; that is the error. On
    /// every failure after ExifTool was asked to write, the new file is removed, `path` is not
    /// touched, and the `.saving` folder is removed when it is left empty; so it is after a save.
    pub fn write_template_file(
        &mut self,
        path: &Path,
        template: &TemplateFile,
    ) -> Result<(), TemplateFileError> {
        if !has_xmp_extension(path) {
            return Err(TemplateFileError::NotXmpName(path.to_path_buf()));
        }
        let template = TemplateFile::new(template.fields.clone(), template.add.clone());
        if template.fields.is_empty() {
            return Err(TemplateFileError::Empty);
        }
        let mut metadata = Metadata::new();
        for (xmp_id, value) in &template.fields {
            metadata
                .set(xmp_id, value.clone())
                .map_err(TemplateFileError::Value)?;
        }
        let state = FileState {
            kind: FileKind::Raw,
            iim_present: false,
            iim_charset: CodedCharset::Absent,
            iim_values: Vec::new(),
            digest: IimDigest::Missing,
            exif_present: ExifPresent::default(),
            xmp_present: XmpPresent::default(),
        };
        let plan = write_plan::plan(&metadata, &state, WriteSettings::default())
            .map_err(TemplateFileError::Plan)?;

        let folder = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        if folder.to_string_lossy().contains('%') {
            return Err(TemplateFileError::PercentInFolder(folder.to_path_buf()));
        }
        let saving = folder.join(SAVING_FOLDER);
        std::fs::create_dir_all(&saving).map_err(|error| file_error(&saving, &error))?;
        let new_file = saving.join(new_file_name());
        if let Err(error) = remove_file_if_present(&new_file) {
            remove_dir_if_empty(&saving);
            return Err(file_error(&new_file, &error));
        }

        let mut args = plan.options.clone();
        args.extend(plan.args.iter().cloned());
        args.extend(
            template
                .add
                .iter()
                .map(|xmp_id| format!("-{ADD_TO}={xmp_id}")),
        );
        args.push("-o".to_string());
        args.push(new_file.to_string_lossy().into_owned());

        let saved = self
            .execute(&args)
            .map_err(|error| TemplateFileError::ExifTool(error.to_string()))
            .and_then(|answer| {
                if template_written(&answer) && new_file.is_file() {
                    Ok(())
                } else {
                    Err(TemplateFileError::ExifTool(answer.trim().to_string()))
                }
            })
            .and_then(|()| self.read_template_file(&new_file))
            .and_then(|read| compare(&template, &read))
            .and_then(|()| {
                std::fs::rename(&new_file, path).map_err(|error| file_error(path, &error))
            });
        if saved.is_err() {
            // The error being returned is the one that counts; a file left here is in `.saving`,
            // which is never listed among the templates.
            let _ = remove_file_if_present(&new_file);
        }
        remove_dir_if_empty(&saving);
        saved
    }

    /// Reads the template file at `path`, also one another program wrote (see the module
    /// comment).
    ///
    /// A missing file, a file ExifTool cannot read (`ExifTool:Error`) and a file that is not XMP
    /// are the error. Each property of the table that a template holds ([`in_template`]) becomes
    /// a checked field; one it cannot hold is named in [`TemplateRead::skipped`]; a value
    /// `Metadata::set` refuses for its kind is added to [`TemplateRead::unreadable`] and left out
    /// (`set` is a check only: the value passed on is the one read); an empty value and a list over
    /// its limit are left to [`TemplateFile::new`]. `rawstride:AddTo` gives the + boxes; a
    /// file without it loads with every + unchecked. The result is [`TemplateFile::new`] of these,
    /// and what it dropped as a whole is named in [`TemplateRead::dropped`]. Variables in a text
    /// (`{date}` and the others) are kept as written.
    pub fn read_template_file(&mut self, path: &Path) -> Result<TemplateRead, TemplateFileError> {
        if !path.is_file() {
            return Err(TemplateFileError::NotFound(path.to_path_buf()));
        }
        let args: Vec<String> = JSON_OPTIONS
            .iter()
            .chain(&["-ExifTool:Error", "-File:FileType"])
            .map(|arg| arg.to_string())
            .chain([path.to_string_lossy().into_owned()])
            .collect();
        let object = self
            .read_json(&args, path)
            .map_err(|error| TemplateFileError::ExifTool(error.to_string()))?;
        if let Some(error) = object.get("ExifTool:Error") {
            return Err(TemplateFileError::Unreadable {
                path: path.to_path_buf(),
                reason: json_text(error),
            });
        }
        match object.get("File:FileType") {
            Some(Json::String(file_type)) if file_type == "XMP" => {}
            other => {
                return Err(TemplateFileError::NotXmp {
                    path: path.to_path_buf(),
                    file_type: other.map_or_else(|| "unknown".to_string(), json_text),
                });
            }
        }

        let read = xmp_values(&object);
        let mut unreadable = read.unreadable;
        let mut skipped = Vec::new();
        let mut fields = Vec::new();
        for (xmp_id, value) in read.values {
            if !in_template(&xmp_id) {
                skipped.push(xmp_id);
                continue;
            }
            match Metadata::new().set(&xmp_id, value.clone()) {
                // An empty value, and a list over its limit, are left to `new`: it drops the
                // empty value, and the list once its entries without content are dropped if it
                // is still over the limit (named in `dropped`).
                Ok(()) | Err(ValueError::Empty(_) | ValueError::TooMany { .. }) => {
                    fields.push((xmp_id, value))
                }
                Err(error) => unreadable.push((xmp_id, error.to_string())),
            }
        }
        let add = match object.get(ADD_TO) {
            Some(Json::Array(entries)) => entries.iter().map(json_text).collect(),
            Some(entry @ Json::String(_)) => vec![json_text(entry)],
            _ => Vec::new(),
        };
        let given: Vec<String> = fields.iter().map(|(xmp_id, _)| xmp_id.clone()).collect();
        let template = TemplateFile::new(fields, add);
        let dropped = given
            .into_iter()
            .filter(|xmp_id| !template.fields.iter().any(|(kept, _)| kept == xmp_id))
            .collect();
        Ok(TemplateRead {
            template,
            skipped,
            dropped,
            unreadable,
        })
    }
}

/// True when `path`'s name ends in `.xmp`, in any case.
fn has_xmp_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xmp"))
        && path.file_stem().is_some_and(|stem| !stem.is_empty())
}

/// The name of a new file in `.saving`: `<process id>-<counter>.xmp`, digits only, so ExifTool's
/// `-o` finds no `%` in it.
fn new_file_name() -> String {
    let counter = SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{counter}.xmp", std::process::id())
}

/// True if ExifTool's answer to the write of a template file reports exactly one file created, no
/// failed file, and no line of a warning or an error.
fn template_written(answer: &str) -> bool {
    let created: u64 = answer
        .lines()
        .filter_map(|line| line.trim().strip_suffix(" image files created"))
        .filter_map(|count| count.trim().parse::<u64>().ok())
        .sum();
    let warned = answer
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("Warning") || line.starts_with("Error"));
    crate::write_succeeded(answer) && created == 1 && !warned
}

/// `Ok` when the file read back, `read`, holds exactly `template` (already passed through
/// [`TemplateFile::new`]), each value compared by [`values_match`]; otherwise what differs.
fn compare(template: &TemplateFile, read: &TemplateRead) -> Result<(), TemplateFileError> {
    let differs = |what: String| Err(TemplateFileError::Differs(what));
    if let Some((property, text)) = read.unreadable.first() {
        return differs(format!("{property} could not be read back ({text})"));
    }
    if let Some(property) = read.skipped.first().or(read.dropped.first()) {
        return differs(format!("{property} was not expected"));
    }
    let ids = |fields: &[(String, Value)]| -> Vec<String> {
        fields.iter().map(|(xmp_id, _)| xmp_id.clone()).collect()
    };
    if ids(&template.fields) != ids(&read.template.fields) {
        return differs(format!(
            "the fields {:?} were read back as {:?}",
            ids(&template.fields),
            ids(&read.template.fields)
        ));
    }
    for ((xmp_id, written), (_, read_back)) in template.fields.iter().zip(&read.template.fields) {
        if !values_match(written, read_back) {
            return differs(format!("{xmp_id} was read back as {read_back:?}"));
        }
    }
    if template.add != read.template.add {
        return differs(format!(
            "the + boxes {:?} were read back as {:?}",
            template.add, read.template.add
        ));
    }
    Ok(())
}

/// True when `written` and `read_back` are equal, a [`Value::Number`] (also inside a structure)
/// compared as a number ([`numbers_match`]).
fn values_match(written: &Value, read_back: &Value) -> bool {
    match (written, read_back) {
        (Value::Number(a), Value::Number(b)) => numbers_match(a, b),
        (Value::Struct(a), Value::Struct(b)) => structs_match(a, b),
        (Value::StructList(a), Value::StructList(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| structs_match(a, b))
        }
        (a, b) => a == b,
    }
}

/// True when the two structures have the same fields in the same order, each value matching by
/// [`values_match`].
fn structs_match(a: &value::StructValue, b: &value::StructValue) -> bool {
    a.fields.len() == b.fields.len()
        && a.fields
            .iter()
            .zip(&b.fields)
            .all(|((id_a, a), (id_b, b))| id_a == id_b && values_match(a, b))
}

/// True when the texts are equal, or both read as `f64` and are equal within
/// [`NUMBER_TOLERANCE`] of the larger.
fn numbers_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => {
            (a - b).abs() <= NUMBER_TOLERANCE * a.abs().max(b.abs())
        }
        _ => false,
    }
}

/// The text of a JSON value: a string as it is, anything else as JSON.
fn json_text(json: &Json) -> String {
    match json {
        Json::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// A [`TemplateFileError::File`] for `path`.
fn file_error(path: &Path, error: &std::io::Error) -> TemplateFileError {
    TemplateFileError::File {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

/// Removes the file at `path` if it exists.
fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Removes the folder at `path` when it is empty; a folder that is not empty, or any failure, is
/// left as it is (another save may be using it).
fn remove_dir_if_empty(path: &Path) {
    let _ = std::fs::remove_dir(path);
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

    // --- the file (T285) ---

    #[test]
    fn the_config_defines_the_namespace_and_its_one_property() {
        assert!(EXIFTOOL_CONFIG.contains(&format!("'rawstride' => '{RAWSTRIDE_NAMESPACE}'")));
        assert!(EXIFTOOL_CONFIG.contains("1 => 'XMP-rawstride'"));
        assert!(EXIFTOOL_CONFIG.contains("AddTo => { List => 'Bag' }"));
    }

    #[test]
    fn numbers_match_as_numbers_within_the_tolerance() {
        assert!(numbers_match("59.3293", "59.3293"));
        assert!(numbers_match("59.3293", "59.32930000000001"));
        assert!(numbers_match("1", "1.0"));
        assert!(!numbers_match("59.3293", "59.3294"));
        assert!(!numbers_match("59.3293", "north"));
        assert!(!numbers_match("NaN", "1"));
        assert!(!numbers_match("inf", "1e308"));
        assert!(values_match(
            &Value::StructList(vec![entry(&[(
                "exif:GPSLatitude",
                Value::Number("59.3293".to_string())
            )])]),
            &Value::StructList(vec![entry(&[(
                "exif:GPSLatitude",
                Value::Number("59.329300000000003".to_string())
            )])]),
        ));
        // A text is compared as it is.
        assert!(!values_match(&text("1"), &text("1.0")));
    }

    #[test]
    fn a_template_write_counts_only_with_one_file_and_no_warning() {
        assert!(template_written("    1 image files created\n"));
        assert!(template_written("    1 image files created\r\n"));
        assert!(!template_written(
            "    1 image files created\n--- stderr ---\nWarning: Tag 'XMP-rawstride:AddTo' is not defined\n"
        ));
        assert!(!template_written(
            "    0 image files created\n    1 files weren't created due to errors\n--- stderr ---\nError: 'a.xmp' already exists - \n"
        ));
        assert!(!template_written("    2 image files created\n"));
        assert!(!template_written(""));
    }

    #[test]
    fn the_name_must_end_in_xmp() {
        for name in ["a.xmp", "a.XMP", "dir/b.Xmp"] {
            assert!(has_xmp_extension(std::path::Path::new(name)), "{name}");
        }
        for name in ["a.txt", "a", "a.xmp.txt", ".xmp"] {
            assert!(!has_xmp_extension(std::path::Path::new(name)), "{name}");
        }
    }

    #[test]
    fn new_file_names_differ_and_hold_only_digits() {
        let (a, b) = (new_file_name(), new_file_name());
        assert_ne!(a, b);
        let prefix = format!("{}-", std::process::id());
        assert!(a.starts_with(&prefix) && a.ends_with(".xmp"), "{a}");
        // No `%` (nor anything else ExifTool's `-o` could read as a format code).
        let stem = a.trim_end_matches(".xmp");
        assert!(stem.chars().all(|c| c.is_ascii_digit() || c == '-'), "{a}");
    }

    #[test]
    fn compare_names_what_differs() {
        let saved = made(&[("photoshop:City", text("Lund"))]);
        let read = |template: TemplateFile| TemplateRead {
            template,
            skipped: Vec::new(),
            dropped: Vec::new(),
            unreadable: Vec::new(),
        };
        assert_eq!(compare(&saved, &read(saved.clone())), Ok(()));
        assert!(matches!(
            compare(&saved, &read(made(&[("photoshop:City", text("Malmö"))]))),
            Err(TemplateFileError::Differs(_))
        ));
        assert!(matches!(
            compare(&saved, &read(TemplateFile::default())),
            Err(TemplateFileError::Differs(_))
        ));
        let mut unreadable = read(saved.clone());
        unreadable.unreadable = vec![("photoshop:DateCreated".to_string(), "x".to_string())];
        assert!(matches!(
            compare(&saved, &unreadable),
            Err(TemplateFileError::Differs(_))
        ));
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
