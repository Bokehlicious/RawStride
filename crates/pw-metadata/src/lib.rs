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

mod exiftool;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub use exiftool::ExifTool;

/// Errors from metadata operations.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// ExifTool ran but reported a problem.
    ExifTool(String),
    /// The file type cannot carry ratings yet (DNG, JPEG, …).
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::ExifTool(m) => write!(f, "ExifTool: {m}"),
            Error::Unsupported(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

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

/// IPTC fields applied to every image of an ingest (photographer, copyright, contact).
/// Written as IPTC Core in XMP, which Lightroom, Bridge and Photo Mechanic read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Iptc {
    pub creator: String,
    pub copyright: String,
    pub credit: String,
    pub email: String,
    pub phone: String,
    pub url: String,
}

impl Iptc {
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
        put("XMP-dc:Rights", &self.copyright);
        put("XMP-photoshop:Credit", &self.credit);
        put("XMP-iptcCore:CreatorWorkEmail", &self.email);
        put("XMP-iptcCore:CreatorWorkTelephone", &self.phone);
        put("XMP-iptcCore:CreatorWorkURL", &self.url);
        if !clean(&self.copyright).is_empty() {
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
        if out.contains("files created")
            || out.contains("files updated")
            || out.contains("unchanged")
        {
            Ok(())
        } else {
            Err(Error::ExifTool(out.trim().to_string()))
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
