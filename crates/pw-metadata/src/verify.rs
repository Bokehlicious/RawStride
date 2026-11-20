//! The file snapshot and the verification of a written copy (ADR 0007 §9.1 points 1 and 3, §9.3
//! points 1–3; I8a, T84).
//!
//! [`ExifTool::snapshot`] records the state of one file with a single ExifTool call: its type by
//! content, size and modification time, the hash of its image data, ExifTool's validation counts
//! and every other tag as text. [`check_file_type`] refuses a file whose content does not match
//! its kind (a JPEG renamed `.dng`). [`verify_copy`] compares the snapshot of the original with
//! the snapshot of the written copy: the type and the image data must be unchanged, there must be
//! no new errors or warnings (one more warning when the write creates the Photoshop block of a
//! TIFF or DNG, D8), and every tag that was added, removed or changed must be one the
//! write plan wrote or one a write is known to change (file data, pointers, tool data; the keys of
//! an Exif block that an `IFD0:` write creates, only added, ADR 0024 §2.6).
//!
//! Nothing here writes a file; the safe write (T86) uses these checks.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::SystemTime;

use serde_json::{Map, Value as Json};

use crate::reconcile::FileKind;
use crate::write_plan::WritePlan;
use crate::{Error, ExifTool, Result};

/// The line `ExifTool::execute` puts between standard output and standard error.
const STDERR_MARKER: &str = "--- stderr ---\n";

/// The ExifTool arguments of a snapshot, before the file name.
const SNAPSHOT_ARGS: &[&str] = &[
    "-j",
    "-a",
    "-G1",
    "-n",
    "-struct",
    "-api",
    "structformat=jsonq",
    "-api",
    "ImageHashType=SHA256",
    "-all",
    "-ImageDataHash",
    "-validate",
    "-warning",
];

const FILE_TYPE: &str = "File:FileType";
const IMAGE_DATA_HASH: &str = "File:ImageDataHash";
const VALIDATE: &str = "ExifTool:Validate";
/// Keys never put in the listing: the file name ExifTool was given, the warning (JSON keeps only
/// one of several; the count is in `validate`) and the validation counts.
const NOT_LISTED: &[&str] = &["SourceFile", "ExifTool:Warning", VALIDATE];

/// Groups whose changes are always allowed: file name, size and dates; tool data; values ExifTool
/// derives and does not store.
const ALLOWED_GROUPS: &[&str] = &["System", "ExifTool", "Composite"];
/// Single keys whose changes are always allowed.
const ALLOWED_KEYS: &[&str] = &[
    "File:CurrentIPTCDigest",
    "XMP-x:XMPToolkit",
    "XMP-xmp:MetadataDate",
    "XMP-xmp:ModifyDate",
];
/// Tag name endings of pointers a write moves (measured: `IFD0:TileOffsets`,
/// `SubIFD:PreviewImageStart` of a DNG).
const POINTER_ENDINGS: &[&str] = &["Offset", "Offsets", "Start"];
/// The plan argument that shows the plan writes IIM.
const IIM_UTF8: &str = "-IPTC:CodedCharacterSet=UTF8";
/// Keys ExifTool adds when it creates IIM.
const IIM_RECORD_VERSIONS: &[&str] = &[
    "IPTC:EnvelopeRecordVersion",
    "IPTC:ApplicationRecordVersion",
];
/// The key of the Exif byte order, which exists while the file has an Exif block.
const EXIF_BYTE_ORDER: &str = "File:ExifByteOrder";
/// The keys that go with the Exif block ExifTool drops (point 6 of T97, [`drops_exif_block`]).
/// Owner decision 2026-11-12: all five, because ExifTool also adds the resolution when it creates
/// Exif in a JFIF file (measured in `tests/edits.rs`).
const EXIF_BLOCK_KEYS: &[&str] = &[
    EXIF_BYTE_ORDER,
    "IFD0:XResolution",
    "IFD0:YResolution",
    "IFD0:ResolutionUnit",
    "IFD0:YCbCrPositioning",
];
/// The groups of a plan argument that clears an Exif tag.
const EXIF_CLEAR_GROUPS: &[&str] = &["IFD0", "ExifIFD"];
/// The groups of the keys of an Exif block.
const EXIF_GROUPS: &[&str] = &["IFD0", "ExifIFD", "IFD1", "GPS", "InteropIFD"];
/// The plan argument tag that writes the IIM checksum, and the key it changes.
const DIGEST_TAG: &str = "IPTCDigest";
const DIGEST_KEY: &str = "Photoshop:IPTCDigest";
/// The plan argument that writes a new IIM checksum (as `write_plan` makes it); also the argument
/// of the safe write's digest pass.
pub(crate) const NEW_DIGEST: &str = "-IPTCDigest=new";
/// The file types in which ExifTool stores the Photoshop block as a TIFF tag (IFD0 0x8649).
const PHOTOSHOP_TAG_TYPES: &[&str] = &["TIFF", "DNG"];

/// True if the plan writes a new IIM checksum (`-IPTCDigest=new`).
pub(crate) fn writes_new_digest(plan: &WritePlan) -> bool {
    plan.args.iter().any(|arg| arg == NEW_DIGEST)
}

/// ExifTool's validation counts (`ExifTool:Validate`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Validate {
    pub errors: u32,
    pub warnings: u32,
    pub minor: u32,
}

impl Validate {
    /// The counts from the text `-n` prints: three numbers (`"0 7 1"`: errors, warnings, minor
    /// warnings). Anything else is `None`.
    pub fn parse(text: &str) -> Option<Validate> {
        let mut parts = text.split_whitespace().map(str::parse::<u32>);
        let errors = parts.next()?.ok()?;
        let warnings = parts.next()?.ok()?;
        let minor = parts.next()?.ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Validate {
            errors,
            warnings,
            minor,
        })
    }
}

impl fmt::Display for Validate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} errors, {} warnings, {} minor warnings",
            self.errors, self.warnings, self.minor
        )
    }
}

/// The state of one file (§9.1 points 1 and 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSnapshot {
    /// `File:FileType`: `JPEG`, `TIFF`, `DNG`, `XMP`, …; ExifTool decides by content.
    pub file_type: String,
    pub size: u64,
    pub modified: SystemTime,
    /// `File:ImageDataHash` (SHA-256); none for XMP.
    pub image_data_hash: Option<String>,
    /// `ExifTool:Validate`; none when ExifTool did not print it.
    pub validate: Option<Validate>,
    /// Every other key with its value as text (a JSON string as it is, any other JSON value as its
    /// JSON text, so a structure is compared whole). `SourceFile`, `ExifTool:Warning` and
    /// `ExifTool:Validate` are left out.
    pub listing: BTreeMap<String, String>,
}

/// Why a file or a written copy is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The content of the file is not of the expected type (§9.1 point 1).
    WrongFileType { expected: FileKind, found: String },
    /// The copy's type differs from the original's.
    FileTypeChanged { before: String, after: String },
    /// The copy's image data hash differs from the original's.
    ImageDataChanged {
        before: Option<String>,
        after: Option<String>,
    },
    /// The copy has more validation errors or warnings than the original, or its validation counts
    /// are missing where the original's are not (or the reverse).
    NewWarnings {
        before: Option<Validate>,
        after: Option<Validate>,
    },
    /// A key was added, removed or changed that the plan did not write and a write is not known
    /// to change; the first such key in key order.
    Unexpected {
        key: String,
        before: Option<String>,
        after: Option<String>,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::WrongFileType { expected, found } => write!(
                f,
                "file type check failed: expected {}, the file content is {found}",
                expected_type(*expected)
            ),
            VerifyError::FileTypeChanged { before, after } => write!(
                f,
                "file type check failed: the original is {before}, the written copy is {after}"
            ),
            VerifyError::ImageDataChanged { before, after } => write!(
                f,
                "image data check failed: the image data hash changed from {} to {}",
                or_none(before.as_deref()),
                or_none(after.as_deref())
            ),
            VerifyError::NewWarnings { before, after } => write!(
                f,
                "validation check failed: the original has {}, the written copy has {}",
                validate_text(before.as_ref()),
                validate_text(after.as_ref())
            ),
            VerifyError::Unexpected { key, before, after } => write!(
                f,
                "metadata check failed: {key} changed unexpectedly from {} to {}",
                or_none(before.as_deref()),
                or_none(after.as_deref())
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

fn or_none(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{value}\""),
        None => "(none)".to_string(),
    }
}

fn validate_text(validate: Option<&Validate>) -> String {
    match validate {
        Some(validate) => validate.to_string(),
        None => "no validation counts".to_string(),
    }
}

/// The `File:FileType` a file of `kind` must have; for a manufacturer RAW the sidecar is written.
fn expected_type(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Jpeg => "JPEG",
        FileKind::Tiff => "TIFF",
        FileKind::Dng => "DNG",
        FileKind::Raw => "XMP",
    }
}

impl ExifTool {
    /// The snapshot of `file` (see the module comment): `size` and `modified` from the file
    /// system, everything else from one ExifTool call. A missing or unreadable file is the error
    /// ExifTool reports.
    pub fn snapshot(&mut self, file: &Path) -> Result<FileSnapshot> {
        let mut args: Vec<String> = SNAPSHOT_ARGS.iter().map(|arg| arg.to_string()).collect();
        args.push(file.to_string_lossy().into_owned());
        let answer = self.execute(&args)?;
        let object = json_object(&answer, file)?;
        let metadata = std::fs::metadata(file)?;
        snapshot_from_json(object, metadata.len(), metadata.modified()?)
    }
}

/// The object of the one file in ExifTool's JSON answer. No output means ExifTool could not read
/// the file; its message is the error.
fn json_object(answer: &str, file: &Path) -> Result<Map<String, Json>> {
    let (stdout, stderr) = answer.split_once(STDERR_MARKER).unwrap_or((answer, ""));
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

/// A JSON string as it is, any other JSON value as its JSON text.
fn json_text(json: &Json) -> String {
    match json {
        Json::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The snapshot from ExifTool's object for the file. A file type that is missing, or validation
/// counts that are not three numbers, are errors.
fn snapshot_from_json(
    object: Map<String, Json>,
    size: u64,
    modified: SystemTime,
) -> Result<FileSnapshot> {
    let file_type = object
        .get(FILE_TYPE)
        .map(json_text)
        .ok_or_else(|| Error::ExifTool(format!("ExifTool did not report {FILE_TYPE}")))?;
    let image_data_hash = object.get(IMAGE_DATA_HASH).map(json_text);
    let validate = match object.get(VALIDATE).map(json_text) {
        Some(text) => Some(
            Validate::parse(&text)
                .ok_or_else(|| Error::ExifTool(format!("unreadable {VALIDATE} \"{text}\"")))?,
        ),
        None => None,
    };
    let listing = object
        .iter()
        .filter(|(key, _)| !NOT_LISTED.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), json_text(value)))
        .collect();
    Ok(FileSnapshot {
        file_type,
        size,
        modified,
        image_data_hash,
        validate,
        listing,
    })
}

/// Checks that the file's content is of the type `expected` needs (§9.1 point 1): `Jpeg` needs
/// `JPEG`, `Tiff` `TIFF`, `Dng` `DNG`, and `Raw` (whose sidecar is written) `XMP`.
pub fn check_file_type(
    expected: FileKind,
    snapshot: &FileSnapshot,
) -> std::result::Result<(), VerifyError> {
    if snapshot.file_type == expected_type(expected) {
        Ok(())
    } else {
        Err(VerifyError::WrongFileType {
            expected,
            found: snapshot.file_type.clone(),
        })
    }
}

/// Checks the written copy against the original (§9.3 points 1–3; see the module comment).
pub fn verify_copy(
    original: &FileSnapshot,
    copy: &FileSnapshot,
    plan: &WritePlan,
) -> std::result::Result<(), VerifyError> {
    if original.file_type != copy.file_type {
        return Err(VerifyError::FileTypeChanged {
            before: original.file_type.clone(),
            after: copy.file_type.clone(),
        });
    }
    if original.image_data_hash != copy.image_data_hash {
        return Err(VerifyError::ImageDataChanged {
            before: original.image_data_hash.clone(),
            after: copy.image_data_hash.clone(),
        });
    }
    // D8 (owner, 2026-11-12; T92): when the write creates the Photoshop block of a TIFF or DNG to
    // store a new checksum, ExifTool 13.59 adds the validation warning "Non-standard format
    // (undef) for IFD0 0x8649 PhotoshopSettings" (measured on a Xiaomi DNG, `0 7 1` → `0 8 1`, and
    // a Ricoh DNG), so exactly one more warning (never an error) is allowed then.
    let extra_warnings = u32::from(creates_photoshop_tag(original, copy, plan));
    let more = match (original.validate, copy.validate) {
        (Some(before), Some(after)) => {
            after.errors > before.errors
                || after.warnings > before.warnings.saturating_add(extra_warnings)
        }
        (None, None) => false,
        _ => true,
    };
    if more {
        return Err(VerifyError::NewWarnings {
            before: original.validate,
            after: copy.validate,
        });
    }

    let allowed = Allowed::from_plan(plan);
    let exif_block_dropped = drops_exif_block(original, copy, plan);
    let exif_block_created = creates_exif_block(original, copy, plan);
    let mut keys: Vec<&String> = original.listing.keys().chain(copy.listing.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let before = original.listing.get(key);
        let after = copy.listing.get(key);
        let dropped_with_block = exif_block_dropped
            && EXIF_BLOCK_KEYS.contains(&key.as_str())
            && before.is_some()
            && after.is_none();
        let added_with_block = exif_block_created
            && EXIF_BLOCK_KEYS.contains(&key.as_str())
            && before.is_none()
            && after.is_some();
        if before != after && !allowed.allows(key) && !dropped_with_block && !added_with_block {
            return Err(VerifyError::Unexpected {
                key: key.clone(),
                before: before.cloned(),
                after: after.cloned(),
            });
        }
    }
    Ok(())
}

/// D8: the copy of a TIFF or DNG has the key `Photoshop:IPTCDigest`, the original has not, and the
/// plan writes `-IPTCDigest=new`, so the write created the Photoshop block.
fn creates_photoshop_tag(original: &FileSnapshot, copy: &FileSnapshot, plan: &WritePlan) -> bool {
    PHOTOSHOP_TAG_TYPES.contains(&original.file_type.as_str())
        && copy.listing.contains_key(DIGEST_KEY)
        && !original.listing.contains_key(DIGEST_KEY)
        && writes_new_digest(plan)
}

/// Point 6 of T97 (owner decision A, 2026-11-12): in a JPEG whose only Exif is `IFD0:Copyright`,
/// clearing it makes ExifTool 13.59 drop the whole Exif block, and with it `File:ExifByteOrder`
/// and `IFD0:YCbCrPositioning` (which ExifTool adds itself when it creates Exif), and in a JFIF
/// file also `IFD0:XResolution`, `IFD0:YResolution` and `IFD0:ResolutionUnit`, which ExifTool
/// adds from the JFIF density when it creates the Exif (measured with the JPEG of
/// `tests/edits.rs`; owner decision 2026-11-12 to allow all five). Measured by the reviewer: with
/// another Exif tag left (`IFD0:Make`, an `IFD0:XResolution` of its own such as 300,
/// `ExifIFD:ExifVersion`) the block stays, so the check below finds Exif in the copy. True when the plan clears an `IFD0:` or `ExifIFD:` tag (an empty
/// value), the original has `File:ExifByteOrder`, and the copy has no `File:ExifByteOrder` and no
/// key of an Exif group; then the removal of exactly [`EXIF_BLOCK_KEYS`] is allowed.
fn drops_exif_block(original: &FileSnapshot, copy: &FileSnapshot, plan: &WritePlan) -> bool {
    let clears_exif_tag = plan.args.iter().any(|arg| {
        arg.strip_prefix('-')
            .and_then(|rest| rest.strip_suffix('='))
            .and_then(|name| name.split_once(':'))
            .is_some_and(|(group, tag)| {
                EXIF_CLEAR_GROUPS.contains(&group) && !tag.is_empty() && !tag.contains('=')
            })
    });
    let no_exif_left = !copy.listing.contains_key(EXIF_BYTE_ORDER)
        && !copy.listing.keys().any(|key| {
            key.split_once(':')
                .is_some_and(|(group, _)| EXIF_GROUPS.contains(&group))
        });
    clears_exif_tag && original.listing.contains_key(EXIF_BYTE_ORDER) && no_exif_left
}

/// ADR 0024 §2.6 (T233): writing an `IFD0:` tag into a file without Exif makes ExifTool create
/// the Exif block. Measured by the reviewer with ExifTool 13.59 on the JFIF JPEG of
/// `tests/edits.rs`: `-IFD0:Orientation#=6` adds exactly `File:ExifByteOrder`, `IFD0:Orientation`,
/// `IFD0:XResolution`, `IFD0:YResolution`, `IFD0:ResolutionUnit` and `IFD0:YCbCrPositioning`, and
/// the validation counts do not change. True when the plan sets an `IFD0:` tag (an argument
/// `-IFD0:<tag>=<value>` or `-IFD0:<tag>#=<value>` with a non-empty value), the original has no
/// `File:ExifByteOrder` and the copy has it; then the addition of [`EXIF_BLOCK_KEYS`] that the
/// original lacks is allowed (only added, never changed or removed).
fn creates_exif_block(original: &FileSnapshot, copy: &FileSnapshot, plan: &WritePlan) -> bool {
    let sets_ifd0_tag = plan.args.iter().any(|arg| {
        arg.strip_prefix("-IFD0:")
            .and_then(|rest| rest.split_once('='))
            .is_some_and(|(tag, value)| {
                let tag = tag.strip_suffix('#').unwrap_or(tag);
                !tag.is_empty() && !value.is_empty()
            })
    });
    sets_ifd0_tag
        && !original.listing.contains_key(EXIF_BYTE_ORDER)
        && copy.listing.contains_key(EXIF_BYTE_ORDER)
}

/// The keys a plan may change besides the ones always allowed.
struct Allowed {
    /// `<Group>:<Tag>` for each tag argument: the key itself and `<Group>:<Tag>-<language>`.
    tags: Vec<String>,
    /// Single keys (`Photoshop:IPTCDigest`, the IIM record versions).
    keys: Vec<&'static str>,
}

impl Allowed {
    fn from_plan(plan: &WritePlan) -> Allowed {
        let mut allowed = Allowed {
            tags: Vec::new(),
            keys: Vec::new(),
        };
        for arg in &plan.args {
            let Some(name) = arg
                .strip_prefix('-')
                .and_then(|rest| rest.split_once('='))
                .map(|(name, _)| name)
            else {
                continue;
            };
            match name.split_once(':') {
                Some((group, tag)) => {
                    let tag = tag.strip_suffix('#').unwrap_or(tag);
                    // Only an XMP tag has a `-<language>` suffix; IIM names hold `-` themselves
                    // (`By-line`, `By-lineTitle`), so they are kept whole.
                    let tag = if group.starts_with("XMP") {
                        tag.split_once('-').map_or(tag, |(tag, _)| tag)
                    } else {
                        tag
                    };
                    if !group.is_empty() && !tag.is_empty() {
                        allowed.tags.push(format!("{group}:{tag}"));
                    }
                }
                None if name == DIGEST_TAG => allowed.keys.push(DIGEST_KEY),
                None => {}
            }
        }
        if plan.args.iter().any(|arg| arg == IIM_UTF8) {
            allowed.keys.extend_from_slice(IIM_RECORD_VERSIONS);
        }
        allowed
    }

    fn allows(&self, key: &str) -> bool {
        let (group, tag) = key.split_once(':').unwrap_or(("", key));
        ALLOWED_GROUPS.contains(&group)
            || ALLOWED_KEYS.contains(&key)
            || POINTER_ENDINGS.iter().any(|ending| tag.ends_with(ending))
            || self.keys.contains(&key)
            || self.tags.iter().any(|allowed| {
                key.strip_prefix(allowed.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "59787ca8a2313db314cf842afee022339062905e11fde41947e7032f3b44488c";

    fn snapshot(entries: &[(&str, &str)]) -> FileSnapshot {
        FileSnapshot {
            file_type: "JPEG".to_string(),
            size: 640,
            modified: SystemTime::UNIX_EPOCH,
            image_data_hash: Some(HASH.to_string()),
            validate: Some(Validate::default()),
            listing: entries
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        }
    }

    fn original() -> FileSnapshot {
        snapshot(&[
            ("System:FileName", "a.jpg"),
            ("File:FileType", "JPEG"),
            ("JFIF:JFIFVersion", "1 1"),
            ("XMP-photoshop:City", "Lund"),
        ])
    }

    /// The original with `changes` applied: `Some` sets a key, `None` removes it.
    fn changed(changes: &[(&str, Option<&str>)]) -> FileSnapshot {
        let mut copy = original();
        for (key, value) in changes {
            match value {
                Some(value) => copy.listing.insert(key.to_string(), value.to_string()),
                None => copy.listing.remove(*key),
            };
        }
        copy
    }

    fn plan(args: &[&str]) -> WritePlan {
        WritePlan {
            args: args.iter().map(|arg| arg.to_string()).collect(),
            ..WritePlan::default()
        }
    }

    fn passes(changes: &[(&str, Option<&str>)], args: &[&str]) {
        let copy = changed(changes);
        assert_eq!(
            verify_copy(&original(), &copy, &plan(args)),
            Ok(()),
            "{changes:?} with {args:?}"
        );
    }

    fn unexpected(
        changes: &[(&str, Option<&str>)],
        args: &[&str],
        key: &str,
        before: Option<&str>,
        after: Option<&str>,
    ) {
        let copy = changed(changes);
        assert_eq!(
            verify_copy(&original(), &copy, &plan(args)),
            Err(VerifyError::Unexpected {
                key: key.to_string(),
                before: before.map(str::to_string),
                after: after.map(str::to_string),
            }),
            "{changes:?} with {args:?}"
        );
    }

    fn validate(errors: u32, warnings: u32, minor: u32) -> Option<Validate> {
        Some(Validate {
            errors,
            warnings,
            minor,
        })
    }

    /// A JPEG whose only Exif is `IFD0:Copyright` (point 6 of T97), with `extra` keys.
    fn only_copyright_exif(extra: &[(&str, &str)]) -> FileSnapshot {
        let mut entries = vec![
            ("File:FileType", "JPEG"),
            ("File:ExifByteOrder", "Big-endian (Motorola, MM)"),
            ("IFD0:XResolution", "1"),
            ("IFD0:YResolution", "1"),
            ("IFD0:ResolutionUnit", "1"),
            ("IFD0:Copyright", "© Fotograf"),
            ("IFD0:YCbCrPositioning", "Centered"),
            ("XMP-dc:Rights", "© Fotograf"),
        ];
        entries.extend_from_slice(extra);
        snapshot(&entries)
    }

    /// The copy after ExifTool dropped the Exif block (and XMP Rights), with `extra` keys.
    fn exif_block_dropped(extra: &[(&str, &str)]) -> FileSnapshot {
        let mut entries = vec![("File:FileType", "JPEG")];
        entries.extend_from_slice(extra);
        snapshot(&entries)
    }

    const CLEAR_COPYRIGHT: &[&str] = &["-XMP-dc:Rights=", "-IFD0:Copyright="];

    #[test]
    fn the_exif_block_exiftool_drops_when_its_last_tag_is_cleared_passes() {
        assert_eq!(
            verify_copy(
                &only_copyright_exif(&[]),
                &exif_block_dropped(&[]),
                &plan(CLEAR_COPYRIGHT)
            ),
            Ok(())
        );
    }

    #[test]
    fn the_exif_byte_order_may_not_go_while_an_exif_key_is_left() {
        assert_eq!(
            verify_copy(
                &only_copyright_exif(&[("IFD0:Make", "Kamera")]),
                &exif_block_dropped(&[("IFD0:Make", "Kamera")]),
                &plan(CLEAR_COPYRIGHT)
            ),
            Err(VerifyError::Unexpected {
                key: "File:ExifByteOrder".to_string(),
                before: Some("Big-endian (Motorola, MM)".to_string()),
                after: None,
            })
        );
    }

    #[test]
    fn the_exif_byte_order_may_not_go_without_a_clearing_argument() {
        assert_eq!(
            verify_copy(
                &only_copyright_exif(&[]),
                &exif_block_dropped(&[]),
                &plan(&["-XMP-dc:Rights=", "-IFD0:Copyright=x"])
            ),
            Err(VerifyError::Unexpected {
                key: "File:ExifByteOrder".to_string(),
                before: Some("Big-endian (Motorola, MM)".to_string()),
                after: None,
            })
        );
    }

    #[test]
    fn a_changed_exif_byte_order_is_unexpected_even_with_a_clearing_argument() {
        let mut copy = only_copyright_exif(&[]);
        copy.listing.insert(
            "File:ExifByteOrder".to_string(),
            "Little-endian (Intel, II)".to_string(),
        );
        assert_eq!(
            verify_copy(&only_copyright_exif(&[]), &copy, &plan(CLEAR_COPYRIGHT)),
            Err(VerifyError::Unexpected {
                key: "File:ExifByteOrder".to_string(),
                before: Some("Big-endian (Motorola, MM)".to_string()),
                after: Some("Little-endian (Intel, II)".to_string()),
            })
        );
    }

    /// The JFIF JPEG without Exif (T233 point 3).
    fn without_exif() -> FileSnapshot {
        snapshot(&[("File:FileType", "JPEG"), ("JFIF:JFIFVersion", "1 1")])
    }

    /// The copy after `-IFD0:Orientation#=6` created the Exif block (measured, T233 point 3).
    fn exif_block_created() -> FileSnapshot {
        snapshot(&[
            ("File:FileType", "JPEG"),
            ("JFIF:JFIFVersion", "1 1"),
            ("File:ExifByteOrder", "MM"),
            ("IFD0:Orientation", "6"),
            ("IFD0:XResolution", "1"),
            ("IFD0:YResolution", "1"),
            ("IFD0:ResolutionUnit", "1"),
            ("IFD0:YCbCrPositioning", "1"),
        ])
    }

    #[test]
    fn creating_the_exif_block_with_an_ifd0_tag_is_allowed() {
        assert_eq!(
            verify_copy(
                &without_exif(),
                &exif_block_created(),
                &plan(&["-IFD0:Orientation#=6"])
            ),
            Ok(())
        );
    }

    #[test]
    fn the_exif_block_keys_need_an_ifd0_write() {
        assert!(matches!(
            verify_copy(
                &without_exif(),
                &exif_block_created(),
                &plan(&["-XMP-tiff:Orientation#=6"])
            ),
            Err(VerifyError::Unexpected { .. })
        ));
    }

    #[test]
    fn an_existing_exif_block_gets_no_free_keys() {
        let original = snapshot(&[
            ("File:FileType", "JPEG"),
            ("File:ExifByteOrder", "MM"),
            ("IFD0:XResolution", "72"),
        ]);
        let copy = snapshot(&[
            ("File:FileType", "JPEG"),
            ("File:ExifByteOrder", "MM"),
            ("IFD0:XResolution", "1"),
            ("IFD0:Orientation", "6"),
        ]);
        assert_eq!(
            verify_copy(&original, &copy, &plan(&["-IFD0:Orientation#=6"])),
            Err(VerifyError::Unexpected {
                key: "IFD0:XResolution".to_string(),
                before: Some("72".to_string()),
                after: Some("1".to_string()),
            })
        );
    }

    #[test]
    fn an_unchanged_copy_passes() {
        assert_eq!(verify_copy(&original(), &original(), &plan(&[])), Ok(()));
    }

    #[test]
    fn file_data_tool_data_and_derived_values_may_change() {
        passes(
            &[
                ("System:FileName", Some("a.pwtmp-1.jpg")),
                ("System:FileSize", Some("3820")),
                ("ExifTool:ExifToolVersion", Some("13.59")),
                ("Composite:Megapixels", Some("0.000128")),
            ],
            &[],
        );
    }

    #[test]
    fn the_current_iim_digest_may_change() {
        passes(&[("File:CurrentIPTCDigest", Some("31e3daf0"))], &[]);
    }

    #[test]
    fn pointers_may_move() {
        passes(
            &[
                ("SubIFD:PreviewImageStart", Some("9148")),
                ("IFD0:TileOffsets", Some("(Binary data 529 bytes)")),
                ("IFD1:ThumbnailOffset", Some("1234")),
            ],
            &[],
        );
    }

    #[test]
    fn the_xmp_toolkit_and_dates_may_change() {
        passes(
            &[
                ("XMP-x:XMPToolkit", Some("Image::ExifTool 13.59")),
                ("XMP-xmp:MetadataDate", Some("2026:11:12 10:00:00+01:00")),
                ("XMP-xmp:ModifyDate", Some("2026:11:12 10:00:00+01:00")),
            ],
            &[],
        );
    }

    #[test]
    fn a_lang_alt_written_as_x_default_allows_every_language() {
        passes(
            &[
                ("XMP-dc:Description", Some("Hej")),
                ("XMP-dc:Description-sv", Some("Svensk text")),
            ],
            &["-XMP-dc:Description-x-default=Hej"],
        );
    }

    #[test]
    fn a_structure_is_allowed_by_its_argument() {
        passes(
            &[(
                "XMP-iptcCore:CreatorContactInfo",
                Some(r#"{"CiAdrCity":"Lund"}"#),
            )],
            &["-XMP-iptcCore:CreatorContactInfo={CiAdrCity=Lund}"],
        );
    }

    #[test]
    fn a_hash_argument_allows_its_tag() {
        passes(
            &[("IPTC:DateCreated", Some("2026:11:04"))],
            &["-IPTC:DateCreated#=20261104"],
        );
    }

    #[test]
    fn the_iim_record_versions_are_allowed_only_when_the_plan_writes_iim() {
        let changes = [
            ("IPTC:CodedCharacterSet", Some("\u{1b}%G")),
            ("IPTC:EnvelopeRecordVersion", Some("4")),
            ("IPTC:City", Some("Lund")),
            ("IPTC:ApplicationRecordVersion", Some("4")),
        ];
        passes(&changes, &[IIM_UTF8, "-IPTC:City=Lund"]);
        unexpected(
            &changes,
            &["-IPTC:CodedCharacterSet=", "-IPTC:City=Lund"],
            "IPTC:ApplicationRecordVersion",
            None,
            Some("4"),
        );
    }

    #[test]
    fn the_digest_argument_allows_the_photoshop_digest() {
        passes(
            &[("Photoshop:IPTCDigest", Some("31e3daf0"))],
            &["-IPTCDigest=new"],
        );
        unexpected(
            &[("Photoshop:IPTCDigest", Some("31e3daf0"))],
            &[],
            "Photoshop:IPTCDigest",
            None,
            Some("31e3daf0"),
        );
    }

    #[test]
    fn a_key_outside_the_plan_is_unexpected_with_both_values() {
        let args = ["-XMP-dc:Description-x-default=Hej"];
        unexpected(
            &[("XMP-dc:Title", Some("Ny"))],
            &args,
            "XMP-dc:Title",
            None,
            Some("Ny"),
        );
        unexpected(
            &[("JFIF:JFIFVersion", None)],
            &args,
            "JFIF:JFIFVersion",
            Some("1 1"),
            None,
        );
        unexpected(
            &[("XMP-photoshop:City", Some("Malmö"))],
            &args,
            "XMP-photoshop:City",
            Some("Lund"),
            Some("Malmö"),
        );
    }

    #[test]
    fn the_first_unexpected_key_in_key_order_is_reported() {
        unexpected(
            &[("XMP-dc:Title", Some("Ny")), ("IPTC:Headline", Some("Ny"))],
            &[],
            "IPTC:Headline",
            None,
            Some("Ny"),
        );
    }

    #[test]
    fn an_argument_allows_a_whole_tag_and_not_a_prefix() {
        unexpected(
            &[
                ("XMP-photoshop:City", Some("Malmö")),
                ("XMP-photoshop:CityCode", Some("MMA")),
            ],
            &["-XMP-photoshop:City=Malmö"],
            "XMP-photoshop:CityCode",
            None,
            Some("MMA"),
        );
    }

    #[test]
    fn an_iim_name_with_a_hyphen_is_a_whole_tag() {
        // Reviewer's fix: `-IPTC:By-line=` must not allow `IPTC:By-lineTitle`.
        passes(&[("IPTC:By-line", Some("Åsa"))], &["-IPTC:By-line=Åsa"]);
        unexpected(
            &[
                ("IPTC:By-line", Some("Åsa")),
                ("IPTC:By-lineTitle", Some("Fotograf")),
            ],
            &["-IPTC:By-line=Åsa"],
            "IPTC:By-lineTitle",
            None,
            Some("Fotograf"),
        );
    }

    #[test]
    fn a_different_hash_is_changed_image_data() {
        let mut copy = original();
        copy.image_data_hash = Some("00".to_string());
        assert_eq!(
            verify_copy(&original(), &copy, &plan(&[])),
            Err(VerifyError::ImageDataChanged {
                before: Some(HASH.to_string()),
                after: Some("00".to_string()),
            })
        );
    }

    #[test]
    fn a_different_file_type_is_a_changed_type() {
        let mut copy = original();
        copy.file_type = "TIFF".to_string();
        assert_eq!(
            verify_copy(&original(), &copy, &plan(&[])),
            Err(VerifyError::FileTypeChanged {
                before: "JPEG".to_string(),
                after: "TIFF".to_string(),
            })
        );
    }

    #[test]
    fn more_warnings_or_errors_are_refused_and_fewer_warnings_pass() {
        let with = |before, after| {
            let mut original = original();
            original.validate = before;
            let mut copy = self::original();
            copy.validate = after;
            verify_copy(&original, &copy, &plan(&[]))
        };
        assert_eq!(
            with(validate(0, 7, 1), validate(0, 8, 1)),
            Err(VerifyError::NewWarnings {
                before: validate(0, 7, 1),
                after: validate(0, 8, 1),
            })
        );
        assert_eq!(
            with(validate(0, 7, 1), validate(1, 7, 1)),
            Err(VerifyError::NewWarnings {
                before: validate(0, 7, 1),
                after: validate(1, 7, 1),
            })
        );
        assert_eq!(with(validate(0, 7, 1), validate(0, 6, 1)), Ok(()));
        assert_eq!(with(validate(0, 7, 1), validate(0, 7, 2)), Ok(()));
        assert_eq!(with(None, None), Ok(()));
        assert_eq!(
            with(validate(0, 0, 0), None),
            Err(VerifyError::NewWarnings {
                before: validate(0, 0, 0),
                after: None,
            })
        );
    }

    /// D8: `verify_copy` of a `file_type` copy whose validation goes from `0 7 1` to `after`; the
    /// copy has a new `Photoshop:IPTCDigest`, and the original has one when `original_digest`.
    fn photoshop_block(
        file_type: &str,
        original_digest: bool,
        args: &[&str],
        after: Option<Validate>,
    ) -> std::result::Result<(), VerifyError> {
        let mut original = original();
        original.file_type = file_type.to_string();
        original.validate = validate(0, 7, 1);
        if original_digest {
            original
                .listing
                .insert(DIGEST_KEY.to_string(), "f379e478".to_string());
        }
        let mut copy = original.clone();
        copy.listing
            .insert(DIGEST_KEY.to_string(), "2c586d07".to_string());
        copy.validate = after;
        verify_copy(&original, &copy, &plan(args))
    }

    #[test]
    fn one_more_warning_passes_when_a_tiff_or_dng_gets_its_photoshop_block() {
        for file_type in ["TIFF", "DNG"] {
            assert_eq!(
                photoshop_block(file_type, false, &[NEW_DIGEST], validate(0, 8, 1)),
                Ok(()),
                "{file_type}"
            );
            // Fewer warnings or more minor warnings still pass.
            assert_eq!(
                photoshop_block(file_type, false, &[NEW_DIGEST], validate(0, 7, 2)),
                Ok(()),
                "{file_type}"
            );
        }
    }

    #[test]
    fn one_more_warning_is_refused_outside_the_photoshop_block_case() {
        let refused = |file_type, original_digest, args: &[&str], after| {
            assert_eq!(
                photoshop_block(file_type, original_digest, args, after),
                Err(VerifyError::NewWarnings {
                    before: validate(0, 7, 1),
                    after,
                }),
                "{file_type} {original_digest} {args:?} {after:?}"
            );
        };
        // A JPEG.
        refused("JPEG", false, &[NEW_DIGEST], validate(0, 8, 1));
        // The plan does not write the checksum.
        refused("TIFF", false, &["-IPTC:City=Lund"], validate(0, 8, 1));
        refused("DNG", false, &[], validate(0, 8, 1));
        // The original already had the Photoshop block.
        refused("TIFF", true, &[NEW_DIGEST], validate(0, 8, 1));
        refused("DNG", true, &[NEW_DIGEST], validate(0, 8, 1));
        // Two more warnings.
        refused("TIFF", false, &[NEW_DIGEST], validate(0, 9, 1));
        refused("DNG", false, &[NEW_DIGEST], validate(0, 9, 1));
        // One more error, with or without one more warning.
        refused("TIFF", false, &[NEW_DIGEST], validate(1, 7, 1));
        refused("DNG", false, &[NEW_DIGEST], validate(1, 8, 1));
        // Missing counts.
        refused("TIFF", false, &[NEW_DIGEST], None);
    }

    #[test]
    fn check_file_type_for_every_kind() {
        let cases = [
            (FileKind::Jpeg, "JPEG"),
            (FileKind::Tiff, "TIFF"),
            (FileKind::Dng, "DNG"),
            (FileKind::Raw, "XMP"),
        ];
        for (kind, file_type) in cases {
            for (_, found) in cases {
                let mut snapshot = original();
                snapshot.file_type = found.to_string();
                let result = check_file_type(kind, &snapshot);
                if found == file_type {
                    assert_eq!(result, Ok(()), "{kind:?} with {found}");
                } else {
                    assert_eq!(
                        result,
                        Err(VerifyError::WrongFileType {
                            expected: kind,
                            found: found.to_string(),
                        }),
                        "{kind:?} with {found}"
                    );
                }
            }
        }
        let mut renamed = original();
        renamed.file_type = "ARW".to_string();
        assert!(check_file_type(FileKind::Raw, &renamed).is_err());
    }

    fn object(json: &str) -> Map<String, Json> {
        match serde_json::from_str(json) {
            Ok(Json::Object(object)) => object,
            other => panic!("not an object: {other:?}"),
        }
    }

    #[test]
    fn the_snapshot_is_built_from_exiftool_json() {
        let object = object(
            r#"{
                "SourceFile": "a.dng",
                "System:FileName": "a.dng",
                "File:FileType": "DNG",
                "File:ImageDataHash": "abc",
                "ExifTool:Warning": "Wrong IFD for 0xa405",
                "ExifTool:Validate": "0 7 1",
                "XMP-dc:Subject": ["a", "b"],
                "XMP-iptcCore:CreatorContactInfo": {"CiAdrCity": "Lund"},
                "IFD0:ImageWidth": 16
            }"#,
        );
        let snapshot = snapshot_from_json(object, 11, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(snapshot.file_type, "DNG");
        assert_eq!(snapshot.size, 11);
        assert_eq!(snapshot.image_data_hash.as_deref(), Some("abc"));
        assert_eq!(snapshot.validate, validate(0, 7, 1));
        let listing: Vec<(&str, &str)> = snapshot
            .listing
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            listing,
            [
                ("File:FileType", "DNG"),
                ("File:ImageDataHash", "abc"),
                ("IFD0:ImageWidth", "16"),
                ("System:FileName", "a.dng"),
                ("XMP-dc:Subject", r#"["a","b"]"#),
                ("XMP-iptcCore:CreatorContactInfo", r#"{"CiAdrCity":"Lund"}"#),
            ]
        );
    }

    #[test]
    fn a_missing_validate_and_hash_are_none() {
        let object = object(r#"{"SourceFile": "s.xmp", "File:FileType": "XMP"}"#);
        let snapshot = snapshot_from_json(object, 1, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(snapshot.validate, None);
        assert_eq!(snapshot.image_data_hash, None);
    }

    #[test]
    fn the_validate_text() {
        assert_eq!(Validate::parse("0 7 1"), validate(0, 7, 1));
        assert_eq!(Validate::parse("1 Warning"), None);
        assert_eq!(Validate::parse("0 7"), None);
        assert_eq!(Validate::parse("0 7 1 2"), None);
        assert_eq!(Validate::parse(""), None);
        let object = object(r#"{"File:FileType": "JPEG", "ExifTool:Validate": "1 Warning"}"#);
        assert!(matches!(
            snapshot_from_json(object, 1, SystemTime::UNIX_EPOCH),
            Err(Error::ExifTool(m)) if m.contains("Validate")
        ));
    }

    #[test]
    fn a_missing_file_type_is_an_error() {
        let object = object(r#"{"SourceFile": "a.jpg"}"#);
        assert!(matches!(
            snapshot_from_json(object, 1, SystemTime::UNIX_EPOCH),
            Err(Error::ExifTool(m)) if m.contains("FileType")
        ));
    }

    #[test]
    fn the_error_texts_name_the_check_and_the_key() {
        let texts = [
            VerifyError::WrongFileType {
                expected: FileKind::Dng,
                found: "JPEG".to_string(),
            }
            .to_string(),
            VerifyError::ImageDataChanged {
                before: Some("a".to_string()),
                after: Some("b".to_string()),
            }
            .to_string(),
            VerifyError::NewWarnings {
                before: validate(0, 7, 1),
                after: validate(0, 8, 1),
            }
            .to_string(),
            VerifyError::Unexpected {
                key: "XMP-photoshop:City".to_string(),
                before: None,
                after: Some("Lund".to_string()),
            }
            .to_string(),
        ];
        assert_eq!(
            texts[0],
            "file type check failed: expected DNG, the file content is JPEG"
        );
        assert!(
            texts[1].starts_with("image data check failed"),
            "{}",
            texts[1]
        );
        assert!(texts[2].contains("0 errors, 8 warnings"), "{}", texts[2]);
        assert_eq!(
            texts[3],
            "metadata check failed: XMP-photoshop:City changed unexpectedly from (none) to \"Lund\""
        );
    }
}
