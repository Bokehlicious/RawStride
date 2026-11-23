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

use pw_metadata::edit::{EditError, EditOptions, EditReport};
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Journal;
use pw_metadata::read::MetadataRead;
use pw_metadata::reconcile::IimDigest;
use pw_metadata::safe_write::{NoImageCheck, WriteReport};
use pw_metadata::template::{Mode, Template, TemplateField};
use pw_metadata::template_file::{TemplateFile, write_exiftool_config};
use pw_metadata::value::{LangAlt, StructValue, Value};
use pw_metadata::variables::VariableError;
use pw_metadata::write_plan::WriteNotice;
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

/// Starts ExifTool with `-config` and the template config written in `dir` (T285), as `exiftool`
/// does otherwise. Panics (fails the test) if it is unavailable.
fn exiftool_with_config(dir: &Path) -> ExifTool {
    let config = write_exiftool_config(dir).unwrap();
    let config = config.to_string_lossy().into_owned();
    let (started, source) = match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => (
            ExifTool::start_with("perl", &[&pl, "-config", &config]),
            format!("perl {pl} (PW_EXIFTOOL_PL)"),
        ),
        Err(_) => (
            ExifTool::start_with("exiftool", &["-config", &config]),
            "exiftool on PATH".to_string(),
        ),
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

    /// Applies the template file `template` with its variables filled (T288), with the default
    /// options and `NoImageCheck`.
    fn apply_file(
        &self,
        et: &mut ExifTool,
        image: &Path,
        template: &TemplateFile,
    ) -> Result<EditReport, EditError> {
        et.apply_template_file_safely(
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

/// Fails the test unless `result` is an edit report with `report` and `notices`.
fn assert_edit_report(
    result: Result<EditReport, EditError>,
    report: WriteReport,
    notices: &[WriteNotice],
) {
    match result {
        Ok(got) => assert_eq!(
            got,
            EditReport {
                report,
                notices: notices.to_vec(),
            }
        ),
        Err(error) => panic!("expected {report:?}, got the error {error}"),
    }
}

/// The template of an ingest as the import dialog fills it in.
fn ingest_template() -> Iptc {
    Iptc {
        creator: "Anna Exempelström".into(),
        copyright: LocalizedValue::x_default("© 2026 Anna Exempelström"),
        credit: "Exempelström Foto".into(),
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
    assert_eq!(
        value(&after, "dc:creator"),
        Some(list(&["Anna Exempelström"]))
    );
    assert_eq!(
        value(&after, "dc:rights"),
        Some(x_default("© 2026 Anna Exempelström"))
    );
    assert_eq!(
        value(&after, "photoshop:Credit"),
        Some(text("Exempelström Foto"))
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
        "© 2026 Anna Exempelström"
    );
    assert_eq!(
        tag(&mut et, &file, "XMP-dc:Rights"),
        "© 2026 Anna Exempelström"
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
    assert_eq!(
        tag(&mut et, &sidecar, "XMP-dc:Creator"),
        "Anna Exempelström"
    );
    assert_eq!(
        tag(&mut et, &sidecar, "XMP-iptcCore:CreatorWorkEmail"),
        "foto@example.com"
    );
    let after = read(&mut et, &raw);
    assert_eq!(
        value(&after, "dc:rights"),
        Some(x_default("© 2026 Anna Exempelström"))
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
    assert_eq!(
        tag(&mut et, &sidecar, "XMP-dc:Creator"),
        "Anna Exempelström"
    );
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

/// The whole path of a template file (T285 point 5): saved with keyword `c` and its + box checked
/// and Headline `X`, read back, turned into a template and applied to a JPEG with keywords `a`,
/// `b` and Headline `Y`: the keyword is added after the image's and the Headline replaced.
#[test]
fn a_saved_template_file_is_read_back_and_applied() {
    let templates = temp_dir("whole-path-templates");
    let mut et = exiftool_with_config(&templates);
    let s = Scratch::new("whole-path");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-dc:Subject=a",
            "-XMP-dc:Subject=b",
            "-XMP-photoshop:Headline=Y",
        ],
    );

    let path = templates.join("mall.xmp");
    let saved = TemplateFile::new(
        vec![
            ("dc:subject".to_string(), list(&["c"])),
            ("photoshop:Headline".to_string(), text("X")),
        ],
        vec!["dc:subject".to_string()],
    );
    et.write_template_file(&path, &saved).unwrap();
    let read = et.read_template_file(&path).unwrap();
    assert_eq!(read.template, saved);

    assert_report(
        s.apply(&mut et, &file, &read.template.to_template()),
        written(&file),
    );
    let after = self::read(&mut et, &file);
    assert_eq!(value(&after, "dc:subject"), Some(list(&["a", "b", "c"])));
    assert_eq!(value(&after, "photoshop:Headline"), Some(text("X")));
    s.assert_clean();
    s.remove();
    let _ = std::fs::remove_dir_all(&templates);
}

// --- A template file with variables (T288) ---

/// The capture time, make and model the tests of variables set up.
const CAMERA: [&str; 3] = [
    "-ExifIFD:DateTimeOriginal=2026:11:22 14:30:05",
    "-IFD0:Make=SONY",
    "-IFD0:Model=ILCE-1",
];
const LOCATION_CREATED: &str = "Iptc4xmpExt:LocationCreated";
const CONTACT_INFO: &str = "Iptc4xmpCore:CreatorContactInfo";
const CITY: &str = "Iptc4xmpExt:City";
const EMAIL: &str = "Iptc4xmpCore:CiEmailWork";

/// The name of the folder `file` is in.
fn folder_name(file: &Path) -> String {
    file.parent()
        .and_then(Path::file_name)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// A structure of these fields.
fn structure(fields: &[(&str, Value)]) -> StructValue {
    StructValue {
        fields: fields
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect(),
    }
}

/// `TemplateFile::new` of `fields`, with the + box of each of `add`; every field must be kept.
fn template_file(fields: &[(&str, Value)], add: &[&str]) -> TemplateFile {
    let made = TemplateFile::new(
        fields
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect(),
        add.iter().map(|id| id.to_string()).collect(),
    );
    assert_eq!(made.fields.len(), fields.len(), "precondition: {made:?}");
    made
}

/// A template file with a variable in every kind of field: a text, a Lang Alt, a list with its +
/// box, a field of Location Created and a field of Creator's Contact Info.
fn every_kind_of_field() -> TemplateFile {
    template_file(
        &[
            ("photoshop:Headline", text("{date} {name}")),
            ("dc:description", x_default("Shot with {model}")),
            ("dc:subject", list(&["{year}", "{make}"])),
            (
                LOCATION_CREATED,
                Value::StructList(vec![structure(&[(CITY, text("{folder}"))])]),
            ),
            (
                CONTACT_INFO,
                Value::Struct(structure(&[(EMAIL, text("{name}@example.com"))])),
            ),
        ],
        &["dc:subject"],
    )
}

/// Every kind of field of a template file is filled with the JPEG's own values; the same template
/// again writes nothing.
#[test]
fn a_template_file_fills_every_kind_of_field() {
    let mut et = exiftool();
    let s = Scratch::new("variables-every-kind");
    let file = s.jpeg();
    let mut tags = CAMERA.to_vec();
    tags.push("-XMP-dc:Subject=a");
    write(&mut et, &file, &tags);
    let template = every_kind_of_field();

    assert_edit_report(s.apply_file(&mut et, &file, &template), written(&file), &[]);

    let after = read(&mut et, &file);
    assert_eq!(
        value(&after, "photoshop:Headline"),
        Some(text("2026-11-22 bild ä"))
    );
    assert_eq!(
        value(&after, "dc:description"),
        Some(x_default("Shot with ILCE-1"))
    );
    assert_eq!(
        value(&after, "dc:subject"),
        Some(list(&["a", "2026", "SONY"]))
    );
    assert_eq!(
        value(&after, LOCATION_CREATED),
        Some(Value::StructList(vec![structure(&[(
            CITY,
            text(&folder_name(&file))
        )])]))
    );
    assert_eq!(
        value(&after, CONTACT_INFO),
        Some(Value::Struct(structure(&[(
            EMAIL,
            text("bild ä@example.com")
        )])))
    );
    s.assert_clean();

    let bytes = std::fs::read(&file).unwrap();
    assert_edit_report(
        s.apply_file(&mut et, &file, &template),
        WriteReport::Unchanged,
        &[],
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// A date variable on a JPEG without a capture time stops the write: nothing is written.
#[test]
fn a_date_variable_without_a_capture_time_writes_nothing() {
    let mut et = exiftool();
    let s = Scratch::new("variables-no-date");
    let file = s.jpeg();
    write(&mut et, &file, &["-IFD0:Make=SONY", "-IFD0:Model=ILCE-1"]);
    let bytes = std::fs::read(&file).unwrap();

    match s.apply_file(&mut et, &file, &every_kind_of_field()) {
        Err(EditError::Variable(VariableError::NoCaptureTime(name))) => assert_eq!(name, "date"),
        other => panic!("expected NoCaptureTime, got {other:?}"),
    }
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// An unknown variable is an error and nothing is written.
#[test]
fn an_unknown_variable_in_a_template_file_writes_nothing() {
    let mut et = exiftool();
    let s = Scratch::new("variables-unknown");
    let file = s.jpeg();
    write(&mut et, &file, &CAMERA);
    let bytes = std::fs::read(&file).unwrap();
    let template = template_file(&[("photoshop:Headline", text("{job}"))], &[]);

    match s.apply_file(&mut et, &file, &template) {
        Err(EditError::Variable(VariableError::Unknown(name))) => assert_eq!(name, "job"),
        other => panic!("expected Unknown, got {other:?}"),
    }
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// A `{make}` on a JPEG without Make leaves the field without content, which changes nothing: the
/// Headline is kept and no keyword is added.
#[test]
fn a_field_empty_after_filling_changes_nothing() {
    let mut et = exiftool();
    let s = Scratch::new("variables-empty");
    let file = s.jpeg();
    write(&mut et, &file, &["-XMP-photoshop:Headline=Old"]);
    let bytes = std::fs::read(&file).unwrap();
    let template = template_file(
        &[
            ("photoshop:Headline", text("{make}")),
            ("dc:subject", list(&["{make}"])),
        ],
        &["dc:subject"],
    );

    assert_edit_report(
        s.apply_file(&mut et, &file, &template),
        WriteReport::Unchanged,
        &[],
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let after = read(&mut et, &file);
    assert_eq!(value(&after, "photoshop:Headline"), Some(text("Old")));
    assert_eq!(value(&after, "dc:subject"), None);
    s.assert_clean();
    s.remove();
}

/// The fake RAW: `{date}` (it has no capture time) makes no sidecar; `{name}` gives its file stem
/// in a new sidecar.
#[test]
fn a_template_file_on_a_raw_fills_its_sidecar() {
    let mut et = exiftool();
    let s = Scratch::new("variables-raw");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");

    let date = template_file(&[("photoshop:Headline", text("{date}"))], &[]);
    match s.apply_file(&mut et, &raw, &date) {
        Err(EditError::Variable(VariableError::NoCaptureTime(name))) => assert_eq!(name, "date"),
        other => panic!("expected NoCaptureTime, got {other:?}"),
    }
    assert_eq!(s.files(), ["TN0001.ARW"]);

    let name = template_file(&[("photoshop:Headline", text("{name}"))], &[]);
    assert_edit_report(s.apply_file(&mut et, &raw, &name), written(&sidecar), &[]);
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    assert_eq!(tag(&mut et, &sidecar, "XMP-photoshop:Headline"), "TN0001");
    s.assert_clean();
    s.remove();
}

/// `{{x}}` gives `{x}`.
#[test]
fn doubled_braces_in_a_template_file_give_one_brace() {
    let mut et = exiftool();
    let s = Scratch::new("variables-braces");
    let file = s.jpeg();
    let template = template_file(&[("photoshop:Headline", text("{{x}}"))], &[]);

    assert_edit_report(s.apply_file(&mut et, &file, &template), written(&file), &[]);
    let after = read(&mut et, &file);
    assert_eq!(value(&after, "photoshop:Headline"), Some(text("{x}")));
    s.assert_clean();
    s.remove();
}

/// A JPEG with IIM City and a matching checksum: a template file's literal City of 40 bytes gives
/// the notice that IIM was cut to 32 bytes (§5.2), as an edit does.
#[test]
fn a_template_file_gives_the_plans_notices() {
    let mut et = exiftool();
    let s = Scratch::new("variables-notices");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-photoshop:City=Lund",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Lund",
            "-IPTCDigest=new",
        ],
    );
    assert_eq!(
        read(&mut et, &file).state.digest,
        IimDigest::Matches,
        "precondition"
    );
    let long = "0123456789012345678901234567890123456789";
    assert_eq!(long.len(), 40);
    let template = template_file(&[("photoshop:City", text(long))], &[]);

    assert_edit_report(
        s.apply_file(&mut et, &file, &template),
        written(&file),
        &[WriteNotice::Truncated {
            property: "photoshop:City".to_string(),
        }],
    );
    assert_eq!(tag(&mut et, &file, "XMP-photoshop:City"), long);
    assert_eq!(tag(&mut et, &file, "IPTC:City"), &long[..32]);
    s.assert_clean();
    s.remove();
}
