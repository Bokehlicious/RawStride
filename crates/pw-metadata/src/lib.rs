//! Metadata handling for PhotoWorkflow.
//!
//! Milestone 3 (first part): star ratings and colour labels in XMP sidecar files, written and
//! read through a persistent ExifTool process (`-stay_open`), so there is one process for the
//! whole session instead of one per file.
//!
//! Scope decided with the project owner (2026-11-03):
//! - Proprietary RAW (ARW, RW2, …): an XMP sidecar next to the file (`<stem>.xmp`), as
//!   `xmp:Rating` and `xmp:Label`. The RAW file is never modified.
//! - DNG, JPEG and TIFF: written *into the file* (embedded XMP), like Photo Mechanic does for DNG
//!   and as Lightroom expects for these formats. Only the metadata is rewritten, never the image
//!   data; the file modification date is preserved (`-P`) and the file is replaced in place.
//! - ExifTool edits existing metadata in place, so everything else is preserved.
//! - `properties`: the IPTC property table generated from the TechReference (ADR 0007 §3).

pub mod date;
mod exiftool;
mod iptc_table;
pub mod properties;
#[cfg(test)]
mod test_support;
mod worker;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

pub use exiftool::ExifTool;
pub use worker::{Event as WorkerEvent, MetadataWorker, RequestId, SubmitError};

/// Errors from metadata operations.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// ExifTool ran but reported a problem.
    ExifTool(String),
    /// The file type cannot carry ratings yet (DNG, JPEG, …).
    Unsupported(String),
    /// ExifTool refused a write because the file that holds the metadata is read-only.
    ReadOnly(PathBuf),
    /// ExifTool refused a write for another reason; holds ExifTool's answer.
    NotWritten(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::ExifTool(m) => write!(f, "ExifTool: {m}"),
            Error::Unsupported(m) => write!(f, "{m}"),
            Error::ReadOnly(path) => write!(f, "{} is read-only", path.display()),
            Error::NotWritten(m) => write!(f, "ExifTool did not write the file: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Why a rating write from the worker failed, kept as a kind so a program can show its own text
/// (T49).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The file that holds the rating is read-only.
    ReadOnly,
    /// ExifTool refused the write for another reason; holds ExifTool's answer.
    NotWritten(String),
    /// Anything else (ExifTool could not start or crashed, I/O); holds the error text.
    Other(String),
}

impl From<Error> for WriteError {
    fn from(error: Error) -> Self {
        match error {
            Error::ReadOnly(_) => WriteError::ReadOnly,
            Error::NotWritten(answer) => WriteError::NotWritten(answer),
            other => WriteError::Other(other.to_string()),
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::ReadOnly => write!(f, "the file is read-only"),
            WriteError::NotWritten(m) => write!(f, "ExifTool did not write the file: {m}"),
            WriteError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Colour labels, written as the Adobe/Lightroom label names.
pub const LABELS: &[&str] = &["Red", "Yellow", "Green", "Blue", "Purple"];

/// Rating and colour label of one image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rating {
    /// 1–5 stars, -1 = rejected (Lightroom convention), `None` = unrated.
    pub stars: Option<i8>,
    /// Label text, e.g. "Red". Any text found in an existing sidecar is kept as-is.
    pub label: Option<String>,
}

/// Text values indexed by BCP 47 language tag (an XMP Lang Alt value).
///
/// The map is ordered by language tag so iteration and serialized plans are deterministic.
///
/// Only the `x-default` entry is written (ADR 0007, Q7: PhotoWorkflow edits `x-default` only and
/// preserves all other language entries in the file). Other entries are kept in the model for
/// reading, but they are not written and do not make a template non-empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalizedValue {
    pub values: BTreeMap<String, String>,
}

impl LocalizedValue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        language_tag: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<String> {
        self.values.insert(language_tag.into(), value.into())
    }

    pub fn get(&self, language_tag: &str) -> Option<&str> {
        self.values.get(language_tag).map(String::as_str)
    }

    /// Empty when no locale has a non-whitespace value.
    pub fn is_empty(&self) -> bool {
        self.values.values().all(|value| value.trim().is_empty())
    }

    /// A value with only the `x-default` entry, the one PhotoWorkflow writes.
    pub fn x_default(value: impl Into<String>) -> Self {
        let mut out = Self::new();
        out.insert(X_DEFAULT, value);
        out
    }

    /// The `x-default` entry, or `""` if there is none.
    pub fn default_text(&self) -> &str {
        self.get(X_DEFAULT).unwrap_or("")
    }
}

/// The Lang Alt entry PhotoWorkflow reads first and writes (ADR 0007 §8, Q7).
pub const X_DEFAULT: &str = "x-default";

/// IPTC fields applied to every image of an ingest (photographer, copyright, contact).
/// Written as IPTC Core in XMP, which Lightroom, Bridge and Photo Mechanic read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Iptc {
    pub creator: String,
    pub copyright: LocalizedValue,
    pub title: LocalizedValue,
    pub description: LocalizedValue,
    pub credit: String,
    pub email: String,
    pub phone: String,
    pub url: String,
}

impl Iptc {
    /// True when applying the template would write nothing. Defined through [`Iptc::tags`], so
    /// "empty" and "writes nothing" can never disagree (a template with only non-`x-default`
    /// languages is empty, because those are not written).
    pub fn is_empty(&self) -> bool {
        self.tags().is_empty()
    }

    /// ExifTool arguments for the non-empty fields. Line breaks become spaces (one argument per
    /// line in the `-stay_open` protocol).
    fn tags(&self) -> Vec<String> {
        let clean = |v: &str| v.replace(['\r', '\n'], " ").trim().to_string();
        let mut out = Vec::new();
        let mut put = |tag: &str, v: &str| {
            let v = clean(v);
            if !v.is_empty() {
                out.push(format!("-{tag}={v}"));
            }
        };
        put("XMP-dc:Creator", &self.creator);
        put("XMP-dc:Title", self.title.default_text());
        put("XMP-dc:Description", self.description.default_text());
        put("XMP-dc:Rights", self.copyright.default_text());
        put("XMP-photoshop:Credit", &self.credit);
        put("XMP-iptcCore:CreatorWorkEmail", &self.email);
        put("XMP-iptcCore:CreatorWorkTelephone", &self.phone);
        put("XMP-iptcCore:CreatorWorkURL", &self.url);
        if !clean(self.copyright.default_text()).is_empty() {
            out.push("-XMP-xmpRights:Marked=True".to_string());
        }
        out
    }
}

impl Rating {
    pub fn is_empty(&self) -> bool {
        self.stars.is_none() && self.label.is_none()
    }
}

/// RAW extensions (lower case) whose ratings go to an XMP sidecar.
const SIDECAR_EXTENSIONS: &[&str] = &["arw", "rw2", "nef", "cr2", "orf", "pef", "srw"];
/// Extensions whose ratings are embedded in the file itself.
const EMBEDDED_EXTENSIONS: &[&str] = &["dng", "jpg", "jpeg", "tif", "tiff"];

fn ext_in(path: &Path, list: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| list.contains(&e.to_ascii_lowercase().as_str()))
}

/// True if ratings for `path` are written to an XMP sidecar (proprietary RAW).
pub fn supports_sidecar(path: &Path) -> bool {
    ext_in(path, SIDECAR_EXTENSIONS)
}

/// True if ratings for `path` are written into the file itself (DNG, JPEG, TIFF).
pub fn supports_embedded(path: &Path) -> bool {
    ext_in(path, EMBEDDED_EXTENSIONS)
}

/// True if ratings can be stored for `path` at all.
pub fn supports_rating(path: &Path) -> bool {
    supports_sidecar(path) || supports_embedded(path)
}

/// The file that holds the rating of `image`: its sidecar, or the image itself.
pub fn rating_file(image: &Path) -> PathBuf {
    if supports_embedded(image) {
        image.to_path_buf()
    } else {
        sidecar_path(image)
    }
}

/// True if ExifTool's answer to a write says that at least one file was created, updated or left
/// unchanged and that no file failed (T48). The summary lines are read as numbers, because a
/// refused write also prints "0 image files updated" next to "1 files weren't updated due to
/// errors".
fn write_succeeded(out: &str) -> bool {
    let mut written = 0u64;
    let mut failed = 0u64;
    for line in out.lines() {
        let Some((count, rest)) = line.trim().split_once(' ') else {
            continue;
        };
        let Ok(count) = count.parse::<u64>() else {
            continue;
        };
        if matches!(
            rest,
            "image files created" | "image files updated" | "image files unchanged"
        ) {
            written = written.saturating_add(count);
        } else if rest.starts_with("files weren't ") && rest.ends_with(" due to errors") {
            failed = failed.saturating_add(count);
        }
    }
    written > 0 && failed == 0
}

/// The error for a write that ExifTool refused (T49): `ReadOnly` when `target` exists and is
/// read-only, otherwise `NotWritten` with ExifTool's answer. ExifTool has already decided that the
/// write failed; this only names the reason.
fn refused_write_error(target: &Path, out: &str) -> Error {
    if std::fs::metadata(target).is_ok_and(|meta| meta.permissions().readonly()) {
        Error::ReadOnly(target.to_path_buf())
    } else {
        Error::NotWritten(out.trim().to_string())
    }
}

/// The sidecar path for an image: same folder and stem, extension `.xmp`.
pub fn sidecar_path(image: &Path) -> PathBuf {
    image.with_extension("xmp")
}

impl ExifTool {
    /// Writes (or clears) rating and label: in the XMP sidecar for proprietary RAW files,
    /// embedded in the file for DNG, JPEG and TIFF.
    ///
    /// Creates a sidecar if needed; existing metadata keeps everything else. Clearing both values
    /// on a RAW file without a sidecar does nothing.
    pub fn write_rating(&mut self, image: &Path, rating: &Rating) -> Result<()> {
        if !supports_rating(image) {
            return Err(Error::Unsupported(format!(
                "ratings are not supported for {}",
                image.display()
            )));
        }
        let embedded = supports_embedded(image);
        let target = rating_file(image);
        if rating.is_empty() && !embedded && !target.exists() {
            return Ok(());
        }
        if let Some(s) = rating.stars
            && !(-1..=5).contains(&s)
        {
            return Err(Error::Unsupported(format!("rating {s} is outside -1..5")));
        }
        self.write_tags(
            image,
            vec![
                format!(
                    "-XMP-xmp:Rating={}",
                    rating.stars.map(|s| s.to_string()).unwrap_or_default()
                ),
                format!(
                    "-XMP-xmp:Label={}",
                    rating.label.clone().unwrap_or_default()
                ),
            ],
        )
    }

    /// Writes `tags` (ExifTool `-TAG=value` arguments) to the file that holds `image`'s
    /// metadata: the sidecar (created if needed) or, for DNG/JPEG/TIFF, the image itself.
    fn write_tags(&mut self, image: &Path, tags: Vec<String>) -> Result<()> {
        let mut args = if supports_embedded(image) {
            // Replace the file's contents in place (keeps creation date, attributes and links)
            // and keep the modification date.
            vec!["-overwrite_original_in_place".to_string(), "-P".to_string()]
        } else {
            vec!["-overwrite_original".to_string()]
        };
        args.extend(tags);
        args.push(rating_file(image).to_string_lossy().into_owned());
        let out = self.execute(&args)?;
        if write_succeeded(&out) {
            Ok(())
        } else {
            Err(refused_write_error(&rating_file(image), &out))
        }
    }

    /// Applies an IPTC template (ADR 0005): only the non-empty fields are written, so existing
    /// values for the others are kept. Same storage rules as ratings.
    pub fn write_iptc(&mut self, image: &Path, iptc: &Iptc) -> Result<()> {
        if !supports_rating(image) {
            return Err(Error::Unsupported(format!(
                "metadata is not supported for {}",
                image.display()
            )));
        }
        let tags = iptc.tags();
        if tags.is_empty() {
            return Ok(());
        }
        self.write_tags(image, tags)
    }

    /// Reads rating and label for the given images (from sidecars, or from the files themselves
    /// for DNG, JPEG and TIFF). Returns a map keyed by image path; unrated images are absent.
    pub fn read_ratings(&mut self, images: &[PathBuf]) -> Result<HashMap<PathBuf, Rating>> {
        let mut by_sidecar: HashMap<String, PathBuf> = HashMap::new();
        let mut args = vec![
            "-T".to_string(),
            "-f".to_string(),
            "-FileName".to_string(),
            "-XMP-xmp:Rating".to_string(),
            "-XMP-xmp:Label".to_string(),
        ];
        for img in images.iter().filter(|p| supports_rating(p)) {
            let sc = rating_file(img);
            if sc.exists() {
                let name = sc.file_name().map(|n| n.to_string_lossy().to_lowercase());
                if let Some(name) = name {
                    by_sidecar.insert(name, img.clone());
                    args.push(sc.to_string_lossy().into_owned());
                }
            }
        }
        let mut result = HashMap::new();
        if by_sidecar.is_empty() {
            return Ok(result);
        }
        let out = self.execute(&args)?;
        for line in out.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                continue;
            }
            let Some(img) = by_sidecar.get(&cols[0].to_lowercase()) else {
                continue;
            };
            let stars = cols[1].trim().parse::<i8>().ok().filter(|s| *s != 0);
            let label = Some(cols[2].trim())
                .filter(|l| !l.is_empty() && *l != "-")
                .map(String::from);
            let r = Rating { stars, label };
            if !r.is_empty() {
                result.insert(img.clone(), r);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod write_result_tests {
    use super::write_succeeded;

    #[test]
    fn created_updated_and_unchanged_answers_are_success() {
        assert!(write_succeeded("    1 image files created\n"));
        assert!(write_succeeded("    1 image files updated\n"));
        assert!(write_succeeded("    1 image files unchanged\n"));
    }

    #[test]
    fn a_refused_write_on_a_read_only_file_is_a_failure() {
        let sidecar = "    0 image files updated\n    1 files weren't updated due to errors\n\
                       --- stderr ---\nError renaming temporary file to C:/temp/test.xmp\n";
        let jpeg = "    0 image files updated\n    1 files weren't updated due to errors\n\
                    --- stderr ---\nError opening C:/temp/test.jpg for writing\n";

        assert!(!write_succeeded(sidecar));
        assert!(!write_succeeded(jpeg));
    }

    #[test]
    fn zero_files_updated_alone_or_an_empty_answer_is_a_failure() {
        assert!(!write_succeeded("    0 image files updated\n"));
        assert!(!write_succeeded(""));
    }
}

#[cfg(test)]
mod refused_write_tests {
    use super::{Error, WriteError, refused_write_error};

    const REFUSED: &str = "    0 image files updated\n    1 files weren't updated due to errors\n";

    fn temp_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pw-metadata-refused-{name}-{}.xmp",
            std::process::id()
        ));
        std::fs::write(&path, b"<x:xmpmeta/>").unwrap();
        path
    }

    #[test]
    fn a_read_only_target_is_read_only() {
        let path = temp_file("read-only");
        let original = std::fs::metadata(&path).unwrap().permissions();
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        std::fs::set_permissions(&path, read_only).unwrap();

        let error = refused_write_error(&path, REFUSED);

        std::fs::set_permissions(&path, original).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(&error, Error::ReadOnly(target) if *target == path),
            "{error:?}"
        );
    }

    #[test]
    fn a_writable_target_is_not_written_with_the_answer() {
        let path = temp_file("writable");

        let error = refused_write_error(&path, REFUSED);

        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(&error, Error::NotWritten(answer) if answer == REFUSED.trim()),
            "{error:?}"
        );
    }

    #[test]
    fn a_missing_target_is_not_written() {
        let path = std::env::temp_dir().join(format!(
            "pw-metadata-refused-missing-{}.xmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let error = refused_write_error(&path, REFUSED);

        assert!(matches!(error, Error::NotWritten(_)), "{error:?}");
    }

    #[test]
    fn write_error_keeps_the_kind_of_the_error() {
        assert_eq!(
            WriteError::from(Error::ReadOnly("a.xmp".into())),
            WriteError::ReadOnly
        );
        assert_eq!(
            WriteError::from(Error::NotWritten("0 image files updated".into())),
            WriteError::NotWritten("0 image files updated".into())
        );
        assert_eq!(
            WriteError::from(Error::ExifTool("ExifTool exited unexpectedly".into())),
            WriteError::Other("ExifTool: ExifTool exited unexpectedly".into())
        );
    }
}

#[cfg(test)]
mod localized_iptc_tests {
    use super::{Iptc, LocalizedValue};

    #[test]
    fn localized_values_are_ordered_by_language_tag() {
        let mut value = LocalizedValue::new();
        value.insert("sv-SE", "Rättigheter");
        value.insert("en-US", "Rights");
        value.insert("fr-FR", "Droits");

        assert_eq!(
            value.values.keys().map(String::as_str).collect::<Vec<_>>(),
            ["en-US", "fr-FR", "sv-SE"]
        );
        assert_eq!(value.get("sv-SE"), Some("Rättigheter"));
        assert_eq!(value.get("de-DE"), None);
        assert!(!value.is_empty());
    }

    #[test]
    fn localized_values_are_empty_when_unset_or_whitespace_only() {
        assert!(LocalizedValue::default().is_empty());

        let mut value = LocalizedValue::new();
        value.insert("en-US", "");
        value.insert("sv-SE", " \t\n ");
        assert!(value.is_empty());

        value.insert("fr-FR", "Droits");
        assert!(!value.is_empty());
    }

    #[test]
    fn iptc_is_empty_for_default_and_whitespace_only_templates() {
        assert!(Iptc::default().is_empty());
        assert!(
            Iptc {
                creator: "  ".into(),
                credit: "\n".into(),
                email: "\t".into(),
                phone: " ".into(),
                url: " \r\n".into(),
                ..Iptc::default()
            }
            .is_empty()
        );
    }

    #[test]
    fn iptc_is_not_empty_for_x_default_values() {
        for iptc in [
            Iptc {
                copyright: LocalizedValue::x_default("Copyright"),
                ..Iptc::default()
            },
            Iptc {
                title: LocalizedValue::x_default("Titel"),
                ..Iptc::default()
            },
            Iptc {
                description: LocalizedValue::x_default("Description"),
                ..Iptc::default()
            },
        ] {
            assert!(!iptc.is_empty(), "{iptc:?}");
        }
    }

    /// Only `x-default` is written (ADR 0007 Q7), so a template whose localized fields hold other
    /// languages only writes nothing and must count as empty; otherwise ingest would run ExifTool
    /// with no tags and report an error for every file.
    #[test]
    fn iptc_with_only_other_languages_is_empty_and_writes_nothing() {
        let mut copyright = LocalizedValue::new();
        copyright.insert("en-US", "Copyright");
        let mut title = LocalizedValue::new();
        title.insert("sv-SE", "Titel");
        let iptc = Iptc {
            copyright,
            title,
            ..Iptc::default()
        };
        assert!(iptc.tags().is_empty());
        assert!(iptc.is_empty());
    }

    #[test]
    fn localized_fields_write_x_default_and_rights_marked() {
        let iptc = Iptc {
            title: LocalizedValue::x_default("Solnedgång"),
            description: LocalizedValue::x_default("Rad 1\nRad 2"),
            copyright: LocalizedValue::x_default("© 2026 Tobias Nykänen"),
            ..Iptc::default()
        };
        assert_eq!(
            iptc.tags(),
            [
                "-XMP-dc:Title=Solnedgång",
                "-XMP-dc:Description=Rad 1 Rad 2",
                "-XMP-dc:Rights=© 2026 Tobias Nykänen",
                "-XMP-xmpRights:Marked=True",
            ]
        );
    }
}
