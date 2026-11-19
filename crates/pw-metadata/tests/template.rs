//! Integration tests of the ingest template with the modes of ADR 0007 §10 written through the
//! write plan and the safe write (I9b, T88) against a real ExifTool: `exiftool` on PATH, or
//! `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! The files are the small JPEG of `pw-thumbcache`'s fixture and a fake RAW (`TN0001.ARW` with a
//! few bytes, as in `tests/read.rs`), each in its own temporary folder, with a temporary journal
//! folder and `NoImageCheck`. These tests need ExifTool and fail with an explanation when it is
//! missing, instead of returning early and being counted as passed (docs/TESTING.md §4.5). The
//! safe write uses Windows file operations, so the file is built on Windows only.
#![cfg(windows)]

use std::path::{Path, PathBuf};

use pw_metadata::edit::{EditError, EditOptions};
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Journal;
use pw_metadata::read::MetadataRead;
use pw_metadata::reconcile::IimDigest;
use pw_metadata::safe_write::{NoImageCheck, WriteReport};
use pw_metadata::template::{Mode, Template, TemplateField};
use pw_metadata::value::{LangAlt, StructValue, Value};
use pw_metadata::{ExifTool, Iptc, LocalizedValue};

/// The small baseline JPEG of `crates/pw-thumbcache/src/jpeg_fixture.rs`, copied.
const SMALL: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x02, 0x01, 0x01, 0x01, 0x01, 0x01, 0x02,
    0x01, 0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x02, 0x04, 0x03, 0x02, 0x02, 0x02, 0x02, 0x05, 0x04,
    0x04, 0x03, 0x04, 0x06, 0x05, 0x06, 0x06, 0x06, 0x05, 0x06, 0x06, 0x06, 0x07, 0x09, 0x08, 0x06,
    0x07, 0x09, 0x07, 0x06, 0x06, 0x08, 0x0b, 0x08, 0x09, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x06, 0x08,
    0x0b, 0x0c, 0x0b, 0x0a, 0x0c, 0x09, 0x0a, 0x0a, 0x0a, 0xff, 0xdb, 0x00, 0x43, 0x01, 0x02, 0x02,
    0x02, 0x02, 0x02, 0x02, 0x05, 0x03, 0x03, 0x05, 0x0a, 0x07, 0x06, 0x07, 0x0a, 0x0a, 0x0a, 0x0a,
    0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a,
    0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a,
    0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0xff, 0xc0,
    0x00, 0x11, 0x08, 0x00, 0x08, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11,
    0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
    0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05,
    0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7d, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21,
    0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23,
    0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17,
    0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a,
    0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a,
    0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a,
    0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99,
    0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
    0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5,
    0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1,
    0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xff, 0xc4, 0x00, 0x1f, 0x01, 0x00, 0x03,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x11, 0x00,
    0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77, 0x00,
    0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13,
    0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15,
    0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27,
    0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88,
    0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6,
    0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4,
    0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9,
    0xfa, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00, 0xf9,
    0x6e, 0xbf, 0x9f, 0xcf, 0xf5, 0xf0, 0xf9, 0xce, 0xbf, 0xdf, 0xc3, 0xfc, 0x47, 0x3f, 0xff, 0xd9,
];

/// The content of the fake RAW file.
const FAKE_RAW: &[u8] = b"not really a raw file";

/// Starts ExifTool and checks that it answers. Panics (fails the test) if it is unavailable.
fn exiftool() -> ExifTool {
    let (started, source) = match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => (
            ExifTool::start_with("perl", &[&pl]),
            format!("perl {pl} (PW_EXIFTOOL_PL)"),
        ),
        Err(_) => (ExifTool::start(), "exiftool on PATH".to_string()),
    };
    let mut et = match started {
        Ok(et) => et,
        Err(e) => panic!(
            "ExifTool is required for this test but could not be started from {source}: {e}. \
             Install ExifTool or set PW_EXIFTOOL_PL (docs/TESTING.md §4.5)."
        ),
    };
    if let Err(e) = et.execute(&["-ver".to_string()]) {
        panic!("ExifTool from {source} does not answer: {e} (docs/TESTING.md §4.5)");
    }
    et
}

/// A new, empty temporary folder.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pw-metadata-template-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The folders of one test: the files, and the write journal in a folder of its own.
struct Scratch {
    dir: PathBuf,
    journal_dir: PathBuf,
    journal: Journal,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = temp_dir(name);
        let journal_dir = temp_dir(&format!("{name}-journal"));
        let journal = Journal::open(&journal_dir).unwrap();
        Scratch {
            dir,
            journal_dir,
            journal,
        }
    }

    /// A copy of the small JPEG in the folder.
    fn jpeg(&self) -> PathBuf {
        let file = self.dir.join("bild ä.jpg");
        std::fs::write(&file, SMALL).unwrap();
        file
    }

    /// The fake RAW in the folder, without a sidecar.
    fn raw(&self) -> PathBuf {
        let file = self.dir.join("TN0001.ARW");
        std::fs::write(&file, FAKE_RAW).unwrap();
        file
    }

    /// The names of the files in the folder, sorted.
    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// No `pwtmp`/`pwbak` file in the folder and an empty journal.
    fn assert_clean(&self) {
        let leftovers: Vec<String> = self
            .files()
            .into_iter()
            .filter(|name| name.contains(".pwtmp-") || name.contains(".pwbak-"))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new(), "files left over");
        let entries: Vec<PathBuf> = std::fs::read_dir(&self.journal_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(entries, Vec::<PathBuf>::new(), "journal entries left over");
    }

    /// Applies `template` with the default options and `NoImageCheck`.
    fn apply(
        &self,
        et: &mut ExifTool,
        image: &Path,
        template: &Template,
    ) -> Result<WriteReport, EditError> {
        et.apply_template_safely(
            image,
            template,
            &EditOptions::default(),
            &NoImageCheck,
            &self.journal,
        )
    }

    fn remove(self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.journal_dir);
    }
}

/// Writes `tags` to `file` in place, to set up a test.
fn write(et: &mut ExifTool, file: &Path, tags: &[&str]) {
    let mut args = vec!["-overwrite_original".to_string()];
    args.extend(tags.iter().map(|tag| tag.to_string()));
    args.push(file.to_string_lossy().into_owned());
    let out = et.execute(&args).unwrap();
    assert!(
        out.contains("1 image files updated") || out.contains("1 image files created"),
        "ExifTool did not write {}: {out}",
        file.display()
    );
}

/// One tag of `file` as ExifTool prints it (`-s3 -f`): `-` when it is missing.
fn tag(et: &mut ExifTool, file: &Path, name: &str) -> String {
    et.execute(&[
        "-s3".to_string(),
        "-f".to_string(),
        format!("-{name}"),
        file.to_string_lossy().into_owned(),
    ])
    .unwrap()
    .trim()
    .to_string()
}

fn read(et: &mut ExifTool, image: &Path) -> MetadataRead {
    et.read_metadata(image, Fallback::Windows1252).unwrap()
}

/// The value of `xmp_id` after reading.
fn value(read: &MetadataRead, xmp_id: &str) -> Option<Value> {
    read.reconciled
        .metadata
        .get(xmp_id)
        .map(|stored| stored.value.clone())
}

fn text(text: &str) -> Value {
    Value::Text(text.to_string())
}

fn list(entries: &[&str]) -> Value {
    Value::List(entries.iter().map(|entry| entry.to_string()).collect())
}

fn x_default(text: &str) -> Value {
    Value::LangAlt(LangAlt::from_entries(vec![(
        "x-default".to_string(),
        text.to_string(),
    )]))
}

fn one_field(property: &str, mode: Mode, value: Option<Value>) -> Template {
    Template {
        fields: vec![TemplateField {
            property: property.to_string(),
            field: None,
            mode,
            value,
        }],
    }
}

fn written(target: &Path) -> WriteReport {
    WriteReport::Written {
        target: target.to_path_buf(),
    }
}

/// Fails the test unless `result` is the report `expected`.
fn assert_report(result: Result<WriteReport, EditError>, expected: WriteReport) {
    match result {
        Ok(report) => assert_eq!(report, expected),
        Err(error) => panic!("expected {expected:?}, got the error {error}"),
    }
}

/// The template of an ingest as the import dialog fills it in.
fn ingest_template() -> Iptc {
    Iptc {
        creator: "Tobias Nykänen".into(),
        copyright: LocalizedValue::x_default("© 2026 Tobias Nykänen"),
        credit: "Nykänen Foto".into(),
        email: "foto@example.com".into(),
        phone: "+46 31 12 34 56".into(),
        url: "https://example.com".into(),
        ..Iptc::default()
    }
}

/// `Iptc::to_template` on a JPEG: Creator, Copyright Notice, Credit and the three contact fields
/// are read back; the same template again writes nothing.
#[test]
fn the_ingest_template_is_written_to_a_jpeg_and_read_back() {
    let mut et = exiftool();
    let s = Scratch::new("jpeg");
    let file = s.jpeg();
    let template = ingest_template().to_template();

    assert_report(s.apply(&mut et, &file, &template), written(&file));

    let after = read(&mut et, &file);
    assert_eq!(value(&after, "dc:creator"), Some(list(&["Tobias Nykänen"])));
    assert_eq!(
        value(&after, "dc:rights"),
        Some(x_default("© 2026 Tobias Nykänen"))
    );
    assert_eq!(
        value(&after, "photoshop:Credit"),
        Some(text("Nykänen Foto"))
    );
    assert_eq!(
        value(&after, "Iptc4xmpCore:CreatorContactInfo"),
        Some(Value::Struct(StructValue {
            fields: vec![
                (
                    "Iptc4xmpCore:CiEmailWork".to_string(),
                    text("foto@example.com")
                ),
                (
                    "Iptc4xmpCore:CiTelWork".to_string(),
                    text("+46 31 12 34 56")
                ),
                (
                    "Iptc4xmpCore:CiUrlWork".to_string(),
                    text("https://example.com")
                ),
            ]
        }))
    );
    assert_eq!(value(&after, "xmpRights:Marked"), Some(text("True")));
    assert_eq!(value(&after, "dc:title"), None);
    s.assert_clean();

    let bytes = std::fs::read(&file).unwrap();
    assert_report(s.apply(&mut et, &file, &template), WriteReport::Unchanged);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// A JPEG with Exif Copyright: the template's Copyright Notice updates it too (§6.3).
#[test]
fn the_exif_copyright_is_updated_too() {
    let mut et = exiftool();
    let s = Scratch::new("exif");
    let file = s.jpeg();
    write(&mut et, &file, &["-IFD0:Copyright=Old copyright"]);

    assert_report(
        s.apply(&mut et, &file, &ingest_template().to_template()),
        written(&file),
    );

    assert_eq!(
        tag(&mut et, &file, "IFD0:Copyright"),
        "© 2026 Tobias Nykänen"
    );
    assert_eq!(
        tag(&mut et, &file, "XMP-dc:Rights"),
        "© 2026 Tobias Nykänen"
    );
    s.assert_clean();
    s.remove();
}

/// Append of a keyword to a JPEG with two keywords gives three, in order.
#[test]
fn append_adds_a_keyword_at_the_end() {
    let mut et = exiftool();
    let s = Scratch::new("append");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &["-XMP-dc:Subject=hav", "-XMP-dc:Subject=båt"],
    );

    assert_report(
        s.apply(
            &mut et,
            &file,
            &one_field("dc:subject", Mode::Append, Some(list(&["sol"]))),
        ),
        written(&file),
    );

    let after = read(&mut et, &file);
    assert_eq!(
        value(&after, "dc:subject"),
        Some(list(&["hav", "båt", "sol"]))
    );
    s.assert_clean();
    s.remove();
}

/// Clear of City on a JPEG with IIM removes it from XMP and from IIM.
///
/// The file also has a keyword in XMP, so XMP is not empty after the Clear. With City as the only
/// XMP value the write plan (T83) writes no new `IPTCDigest`, the old one is left out of date and
/// the check of the copy refuses it ("IPTCDigest is not current"); that is reported to the owner
/// separately (owner's choice, 2026-11-12, during T88).
#[test]
fn clear_removes_city_from_xmp_and_iim() {
    let mut et = exiftool();
    let s = Scratch::new("clear");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-photoshop:City=Göteborg",
            "-XMP-dc:Subject=hamn",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Göteborg",
            "-IPTCDigest=new",
        ],
    );
    assert_eq!(tag(&mut et, &file, "IPTC:City"), "Göteborg", "precondition");

    assert_report(
        s.apply(
            &mut et,
            &file,
            &one_field("photoshop:City", Mode::Clear, None),
        ),
        written(&file),
    );

    assert_eq!(tag(&mut et, &file, "XMP-photoshop:City"), "-");
    assert_eq!(tag(&mut et, &file, "IPTC:City"), "-");
    let after = read(&mut et, &file);
    assert_eq!(value(&after, "photoshop:City"), None);
    assert_eq!(value(&after, "dc:subject"), Some(list(&["hamn"])));
    assert_eq!(after.state.digest, IimDigest::Matches);
    assert!(
        !after
            .state
            .iim_values
            .iter()
            .any(|(xmp_id, _)| xmp_id == "photoshop:City"),
        "{:?}",
        after.state.iim_values
    );
    s.assert_clean();
    s.remove();
}

/// Clear of City on a JPEG with IIM and a checksum where City is the only XMP value (no keyword):
/// no XMP is left, but the write plan writes a new `IPTCDigest` because the file has a stored one
/// (T93), so the copy check accepts it and the checksum is current (§5.2, §12).
#[test]
fn clear_of_the_only_xmp_value_keeps_the_checksum_current() {
    let mut et = exiftool();
    let s = Scratch::new("clear-only");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-photoshop:City=Göteborg",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Göteborg",
            "-IPTCDigest=new",
        ],
    );
    assert_eq!(tag(&mut et, &file, "IPTC:City"), "Göteborg", "precondition");
    assert_eq!(tag(&mut et, &file, "XMP-dc:Subject"), "-", "precondition");

    assert_report(
        s.apply(
            &mut et,
            &file,
            &one_field("photoshop:City", Mode::Clear, None),
        ),
        written(&file),
    );

    assert_eq!(tag(&mut et, &file, "XMP-photoshop:City"), "-");
    assert_eq!(tag(&mut et, &file, "IPTC:City"), "-");
    let stored = tag(&mut et, &file, "Photoshop:IPTCDigest");
    assert_ne!(stored, "-");
    assert_eq!(stored, tag(&mut et, &file, "CurrentIPTCDigest"));
    let after = read(&mut et, &file);
    assert_eq!(value(&after, "photoshop:City"), None);
    assert_eq!(after.state.digest, IimDigest::Matches);
    assert!(
        !after
            .state
            .iim_values
            .iter()
            .any(|(xmp_id, _)| xmp_id == "photoshop:City"),
        "{:?}",
        after.state.iim_values
    );
    s.assert_clean();
    s.remove();
}

/// The fake RAW: only its sidecar is written (created), the RAW's bytes are unchanged.
#[test]
fn a_raw_gets_only_its_sidecar_written() {
    let mut et = exiftool();
    let s = Scratch::new("raw");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");

    assert_report(
        s.apply(&mut et, &raw, &ingest_template().to_template()),
        written(&sidecar),
    );

    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    assert_eq!(s.files(), ["TN0001.ARW", "TN0001.xmp"]);
    assert_eq!(tag(&mut et, &sidecar, "XMP-dc:Creator"), "Tobias Nykänen");
    assert_eq!(
        tag(&mut et, &sidecar, "XMP-iptcCore:CreatorWorkEmail"),
        "foto@example.com"
    );
    let after = read(&mut et, &raw);
    assert_eq!(
        value(&after, "dc:rights"),
        Some(x_default("© 2026 Tobias Nykänen"))
    );
    s.assert_clean();
    s.remove();
}

/// A RAW whose sidecar already holds a rating of 4: the template is added to the sidecar and the
/// rating is kept; an empty template then writes nothing (moved from the in-place write's tests,
/// T211).
#[test]
fn a_template_keeps_an_earlier_rating_in_a_sidecar() {
    let mut et = exiftool();
    let s = Scratch::new("raw-rating");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");
    write(&mut et, &sidecar, &["-XMP-xmp:Rating=4"]);

    assert_report(
        s.apply(&mut et, &raw, &ingest_template().to_template()),
        written(&sidecar),
    );

    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    assert_eq!(tag(&mut et, &sidecar, "XMP-dc:Creator"), "Tobias Nykänen");
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw)).unwrap()[&raw].stars,
        Some(4)
    );

    let bytes = std::fs::read(&sidecar).unwrap();
    assert_report(
        s.apply(&mut et, &raw, &Iptc::default().to_template()),
        WriteReport::Unchanged,
    );
    assert_eq!(std::fs::read(&sidecar).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}
