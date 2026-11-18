//! Integration tests of the editor's edits written through the write plan and the safe write, with
//! the write notices (ADR 0007 I10d, T97), against a real ExifTool: `exiftool` on PATH, or
//! `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! The files are the small JPEG of `pw-thumbcache`'s fixture and a fake RAW (`TN0001.ARW` with a
//! few bytes, as in `tests/edit.rs`), each in its own temporary folder, with a temporary journal
//! folder and `NoImageCheck`. These tests need ExifTool and fail with an explanation when it is
//! missing, instead of returning early and being counted as passed (docs/TESTING.md §4.5). The
//! safe write uses Windows file operations, so the file is built on Windows only.
#![cfg(windows)]

use std::path::{Path, PathBuf};

use pw_metadata::ExifTool;
use pw_metadata::edit::{Edit, EditError, EditOptions, EditReport};
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Journal;
use pw_metadata::reconcile::IimDigest;
use pw_metadata::safe_write::{NoImageCheck, WriteReport};
use pw_metadata::value::{ConflictRule, LangAlt, Value};
use pw_metadata::write_plan::WriteNotice;

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
    let dir = std::env::temp_dir().join(format!("pw-metadata-edits-{name}-{}", std::process::id()));
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

    /// No `pwtmp`/`pwbak` file in the folder and an empty journal.
    fn assert_clean(&self) {
        let leftovers: Vec<String> = std::fs::read_dir(&self.dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".pwtmp-") || name.contains(".pwbak-"))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new(), "files left over");
        let entries: Vec<PathBuf> = std::fs::read_dir(&self.journal_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(entries, Vec::<PathBuf>::new(), "journal entries left over");
    }

    /// Writes `edits` with the default options and `NoImageCheck`.
    fn edit(
        &self,
        et: &mut ExifTool,
        image: &Path,
        edits: &[Edit],
    ) -> Result<EditReport, EditError> {
        et.write_edits_safely(
            image,
            edits,
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

fn text(text: &str) -> Value {
    Value::Text(text.to_string())
}

fn x_default(text: &str) -> Value {
    Value::LangAlt(LangAlt::from_entries(vec![(
        "x-default".to_string(),
        text.to_string(),
    )]))
}

fn set(property: &str, value: Value) -> Edit {
    Edit::Set {
        property: property.to_string(),
        field: None,
        value,
    }
}

fn clear(property: &str) -> Edit {
    Edit::Clear {
        property: property.to_string(),
        field: None,
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

/// The value `read_metadata` gives for `property` of `image`.
fn read_value(et: &mut ExifTool, image: &Path, property: &str) -> Option<Value> {
    et.read_metadata(image, Fallback::Windows1252)
        .unwrap()
        .reconciled
        .metadata
        .get(property)
        .map(|stored| stored.value.clone())
}

/// Fails the test unless `result` is an edit report with `report` and `notices`.
fn assert_edit(
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

fn written(target: &Path) -> WriteReport {
    WriteReport::Written {
        target: target.to_path_buf(),
    }
}

/// Headline and City set on a JPEG: written and read back with `read_metadata`; the same edits
/// again are unchanged and write nothing.
#[test]
fn headline_and_city_are_written_and_the_same_again_is_unchanged() {
    let mut et = exiftool();
    let s = Scratch::new("headline");
    let file = s.jpeg();
    let edits = [
        set("photoshop:Headline", text("Rubrik")),
        set("photoshop:City", text("Göteborg")),
    ];

    assert_edit(s.edit(&mut et, &file, &edits), written(&file), &[]);
    assert_eq!(
        read_value(&mut et, &file, "photoshop:Headline"),
        Some(text("Rubrik"))
    );
    assert_eq!(
        read_value(&mut et, &file, "photoshop:City"),
        Some(text("Göteborg"))
    );
    let bytes = std::fs::read(&file).unwrap();

    assert_edit(s.edit(&mut et, &file, &edits), WriteReport::Unchanged, &[]);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// The XMP packet of `file` as ExifTool gives it (`-b -XMP`). The packet does not end with a line
/// break, so `-echo3` prints a line after it; otherwise ExifTool's `{ready}` marker would follow on
/// the same line and `execute` would wait for it forever.
fn xmp_packet(et: &mut ExifTool, file: &Path) -> String {
    et.execute(&[
        "-b".to_string(),
        "-XMP".to_string(),
        "-echo3".to_string(),
        "--end of XMP--".to_string(),
        file.to_string_lossy().into_owned(),
    ])
    .unwrap()
}

/// The choice lists' values through ExifTool (T128, point 8): a PLUS edit holds the code alone,
/// ExifTool adds the vocabulary's prefix once and removes it on read; Digital Source Type is the
/// full URI, stored once and read back unchanged.
#[test]
fn a_plus_code_and_a_digital_source_type_uri_are_stored_once_and_read_back() {
    const PLUS_PREFIX: &str = "http://ns.useplus.org/ldf/vocab/";
    const DIGITAL_CAPTURE: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";
    let mut et = exiftool();
    let s = Scratch::new("plus-codes");
    let file = s.jpeg();
    let edits = [
        set("plus:ModelReleaseStatus", text("MR-UMR")),
        set("Iptc4xmpExt:DigitalSourceType", text(DIGITAL_CAPTURE)),
    ];

    assert_edit(s.edit(&mut et, &file, &edits), written(&file), &[]);
    let xmp = xmp_packet(&mut et, &file);
    assert_eq!(
        xmp.matches(&format!("{PLUS_PREFIX}MR-UMR")).count(),
        1,
        "{xmp}"
    );
    assert!(
        !xmp.contains(&format!("{PLUS_PREFIX}{PLUS_PREFIX}")),
        "the PLUS prefix twice: {xmp}"
    );
    assert_eq!(xmp.matches(DIGITAL_CAPTURE).count(), 1, "{xmp}");
    assert!(
        !xmp.contains("digitalsourcetype/http://"),
        "the Digital Source Type prefix twice: {xmp}"
    );
    assert_eq!(
        read_value(&mut et, &file, "plus:ModelReleaseStatus"),
        Some(text("MR-UMR"))
    );
    assert_eq!(
        read_value(&mut et, &file, "Iptc4xmpExt:DigitalSourceType"),
        Some(text(DIGITAL_CAPTURE))
    );
    s.assert_clean();
    s.remove();
}

/// A JPEG with IIM City and a matching checksum: a City of 40 bytes is written with the notice
/// that IIM was cut to 32 bytes (§5.2); XMP holds all 40.
#[test]
fn a_long_city_is_cut_in_iim_with_a_notice() {
    let mut et = exiftool();
    let s = Scratch::new("truncated");
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
    let before = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(before.state.digest, IimDigest::Matches, "precondition");
    let long = "0123456789012345678901234567890123456789";
    assert_eq!(long.len(), 40);

    assert_edit(
        s.edit(&mut et, &file, &[set("photoshop:City", text(long))]),
        written(&file),
        &[WriteNotice::Truncated {
            property: "photoshop:City".to_string(),
        }],
    );
    assert_eq!(tag(&mut et, &file, "XMP-photoshop:City"), long);
    assert_eq!(tag(&mut et, &file, "IPTC:City"), &long[..32]);
    let after = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(after.state.digest, IimDigest::Matches);
    s.assert_clean();
    s.remove();
}

/// A JPEG with City in IIM and another City in XMP and no checksum is an `Open` conflict (Q1):
/// setting the XMP value again decides it, both forms are written and no conflict is left.
#[test]
fn choosing_the_shown_value_decides_an_open_conflict() {
    let mut et = exiftool();
    let s = Scratch::new("open");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-photoshop:City=Lund",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Malmö",
        ],
    );
    let before = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(before.state.digest, IimDigest::Missing, "precondition");
    assert!(
        before
            .reconciled
            .metadata
            .conflicts()
            .iter()
            .any(|conflict| conflict.property == "photoshop:City"
                && conflict.rule == ConflictRule::Open),
        "precondition: {:?}",
        before.reconciled.metadata.conflicts()
    );
    assert_eq!(
        before
            .reconciled
            .metadata
            .get("photoshop:City")
            .map(|stored| stored.value.clone()),
        Some(text("Lund")),
        "precondition"
    );

    assert_edit(
        s.edit(&mut et, &file, &[set("photoshop:City", text("Lund"))]),
        written(&file),
        &[],
    );
    assert_eq!(tag(&mut et, &file, "XMP-photoshop:City"), "Lund");
    assert_eq!(tag(&mut et, &file, "IPTC:City"), "Lund");
    let after = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert!(
        after.reconciled.metadata.conflicts().is_empty(),
        "{:?}",
        after.reconciled.metadata.conflicts()
    );
    s.assert_clean();
    s.remove();
}

/// Clearing the Copyright Notice of a JPEG that also has `IFD0:Copyright` removes it from XMP and
/// Exif (§8: every representation; an edit may clear it, unlike a template).
#[test]
fn clearing_the_copyright_notice_removes_it_from_xmp_and_exif() {
    let mut et = exiftool();
    let s = Scratch::new("rights");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &["-XMP-dc:Rights=© Fotograf", "-IFD0:Copyright=© Fotograf"],
    );
    assert_eq!(
        read_value(&mut et, &file, "dc:rights"),
        Some(x_default("© Fotograf")),
        "precondition"
    );

    assert_edit(
        s.edit(&mut et, &file, &[clear("dc:rights")]),
        written(&file),
        &[],
    );
    assert_eq!(tag(&mut et, &file, "XMP-dc:Rights"), "-");
    assert_eq!(tag(&mut et, &file, "IFD0:Copyright"), "-");
    assert_eq!(read_value(&mut et, &file, "dc:rights"), None);
    s.assert_clean();
    s.remove();
}

/// A Title on the fake RAW: only the sidecar is written; the RAW's bytes are unchanged (§4.2).
#[test]
fn a_title_on_a_raw_writes_only_the_sidecar() {
    let mut et = exiftool();
    let s = Scratch::new("raw");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");

    assert_edit(
        s.edit(&mut et, &raw, &[set("dc:title", x_default("Titel"))]),
        written(&sidecar),
        &[],
    );
    assert_eq!(tag(&mut et, &sidecar, "XMP-dc:Title"), "Titel");
    assert_eq!(
        read_value(&mut et, &raw, "dc:title"),
        Some(x_default("Titel"))
    );
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// ExifTool's JSON output (`-j` and `tags`) for `file`, less the `SourceFile` key, so the same
/// output of a file before and after a write can be compared.
fn json_without_source_file(et: &mut ExifTool, file: &Path, tags: &[&str]) -> serde_json::Value {
    let mut args = vec!["-j".to_string()];
    args.extend(tags.iter().map(|tag| tag.to_string()));
    args.push(file.to_string_lossy().into_owned());
    let out = et.execute(&args).unwrap();
    let mut json: serde_json::Value = match serde_json::from_str(&out) {
        Ok(json) => json,
        Err(e) => panic!("ExifTool's -j output is not JSON ({e}): {out}"),
    };
    let objects = json.as_array_mut().unwrap();
    assert_eq!(objects.len(), 1, "{out}");
    let object = objects[0].as_object_mut().unwrap();
    assert!(object.remove("SourceFile").is_some(), "{out}");
    serde_json::Value::Object(object.clone())
}

fn list(entries: &[&str]) -> Value {
    Value::List(entries.iter().map(|entry| entry.to_string()).collect())
}

/// Hierarchical keywords in a JPEG (`lr:hierarchicalSubject` and an `mwg-kw:KeywordInfo` structure of
/// three levels) are kept unchanged when the flat keywords (`dc:subject`) are edited: ADR 0007 §12,
/// §8 (MWG §5.10 "MUST preserve all nested information").
#[test]
fn hierarchical_keywords_in_a_jpeg_are_kept_when_the_keywords_are_edited() {
    const HIERARCHY: &[&str] = &["-struct", "-XMP-lr:all", "-XMP-mwg-kw:all"];
    let mut et = exiftool();
    let s = Scratch::new("hierarchical-jpeg");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-dc:Subject=Göteborg",
            "-XMP-dc:Subject=Fåglar",
            "-XMP-lr:HierarchicalSubject=Platser|Sverige|Göteborg",
            "-XMP-lr:HierarchicalSubject=Djur|Fåglar",
            "-XMP-mwg-kw:KeywordInfo={Hierarchy=[{Keyword=Platser,Children=[{Keyword=Sverige,\
             Children=[{Keyword=Göteborg}]}]}]}",
        ],
    );
    let before = json_without_source_file(&mut et, &file, HIERARCHY);
    let text_before = before.to_string();
    assert!(
        text_before.contains("Platser|Sverige|Göteborg") && text_before.contains("Djur|Fåglar"),
        "precondition: {before}"
    );
    assert!(
        text_before.contains("\"Keyword\":\"Göteborg\""),
        "precondition: {before}"
    );

    assert_edit(
        s.edit(
            &mut et,
            &file,
            &[set("dc:subject", list(&["Göteborg", "Fåglar", "Hamn"]))],
        ),
        written(&file),
        &[],
    );
    assert_eq!(
        read_value(&mut et, &file, "dc:subject"),
        Some(list(&["Göteborg", "Fåglar", "Hamn"]))
    );
    assert_eq!(json_without_source_file(&mut et, &file, HIERARCHY), before);
    s.assert_clean();
    s.remove();
}

/// Hierarchical keywords (`lr:hierarchicalSubject`) in a RAW's sidecar are kept unchanged when
/// another field (the Title) is edited; the RAW's bytes are unchanged: ADR 0007 §12, §8.
#[test]
fn hierarchical_keywords_in_a_sidecar_are_kept_when_the_title_is_edited() {
    const HIERARCHY: &[&str] = &["-struct", "-XMP-lr:all"];
    let mut et = exiftool();
    let s = Scratch::new("hierarchical-sidecar");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");
    write(
        &mut et,
        &sidecar,
        &[
            "-XMP-dc:Subject=Göteborg",
            "-XMP-dc:Subject=Fåglar",
            "-XMP-lr:HierarchicalSubject=Platser|Sverige|Göteborg",
            "-XMP-lr:HierarchicalSubject=Djur|Fåglar",
        ],
    );
    let before = json_without_source_file(&mut et, &sidecar, HIERARCHY);
    let text_before = before.to_string();
    assert!(
        text_before.contains("Platser|Sverige|Göteborg") && text_before.contains("Djur|Fåglar"),
        "precondition: {before}"
    );

    assert_edit(
        s.edit(&mut et, &raw, &[set("dc:title", x_default("Titel"))]),
        written(&sidecar),
        &[],
    );
    assert_eq!(tag(&mut et, &sidecar, "XMP-dc:Title"), "Titel");
    assert_eq!(
        read_value(&mut et, &raw, "dc:title"),
        Some(x_default("Titel"))
    );
    assert_eq!(
        json_without_source_file(&mut et, &sidecar, HIERARCHY),
        before
    );
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// The deprecated IIM datasets Urgency, Category and Supplemental Categories, in both IIM and
/// XMP with a matching checksum, are kept unchanged when another field (the Headline) is edited and
/// both forms are written; the checksum still matches: ADR 0007 §12, §5.5.
#[test]
fn deprecated_iim_datasets_are_kept_when_the_headline_is_edited() {
    // `-G1 -a`: the IIM and XMP tags have the same names; without them ExifTool prints one of each.
    const DEPRECATED: &[&str] = &[
        "-G1",
        "-a",
        "-IPTC:Urgency",
        "-IPTC:Category",
        "-IPTC:SupplementalCategories",
        "-XMP-photoshop:Urgency",
        "-XMP-photoshop:Category",
        "-XMP-photoshop:SupplementalCategories",
    ];
    let mut et = exiftool();
    let s = Scratch::new("deprecated-iim");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:Urgency=5",
            "-IPTC:Category=SPO",
            "-IPTC:SupplementalCategories=Fotboll",
            "-IPTC:SupplementalCategories=Allsvenskan",
            "-XMP-photoshop:Urgency=5",
            "-XMP-photoshop:Category=SPO",
            "-XMP-photoshop:SupplementalCategories=Fotboll",
            "-XMP-photoshop:SupplementalCategories=Allsvenskan",
            "-IPTCDigest=new",
        ],
    );
    let read_before = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(read_before.state.digest, IimDigest::Matches, "precondition");
    let before = json_without_source_file(&mut et, &file, DEPRECATED);
    assert_eq!(
        before.as_object().map(|object| object.len()),
        Some(6),
        "precondition: {before}"
    );

    assert_edit(
        s.edit(&mut et, &file, &[set("photoshop:Headline", text("Rubrik"))]),
        written(&file),
        &[],
    );
    assert_eq!(tag(&mut et, &file, "XMP-photoshop:Headline"), "Rubrik");
    assert_eq!(tag(&mut et, &file, "IPTC:Headline"), "Rubrik");
    assert_eq!(json_without_source_file(&mut et, &file, DEPRECATED), before);
    let read_after = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(read_after.state.digest, IimDigest::Matches);
    s.assert_clean();
    s.remove();
}

/// An XMP packet whose `dc:subject` has one item with a qualifier (`ex:type`) and one plain item
/// (ADR 0007 §8; checked with ExifTool 13.59). Written with `std::fs::write`, because `write`
/// cannot make a qualifier.
const QUALIFIED_SUBJECT: &str = "<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x='adobe:ns:meta/'>
<rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>
 <rdf:Description rdf:about='' xmlns:dc='http://purl.org/dc/elements/1.1/' xmlns:ex='http://example.com/ns/'>
  <dc:subject><rdf:Bag>
   <rdf:li rdf:parseType='Resource'><rdf:value>San Francisco</rdf:value><ex:type>location</ex:type></rdf:li>
   <rdf:li>cat</rdf:li>
  </rdf:Bag></dc:subject>
 </rdf:Description>
</rdf:RDF>
</x:xmpmeta>
<?xpacket end='w'?>
";

/// The fake RAW with the sidecar [`QUALIFIED_SUBJECT`]; returns the RAW and the sidecar.
fn raw_with_qualified_subject(s: &Scratch) -> (PathBuf, PathBuf) {
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");
    std::fs::write(&sidecar, QUALIFIED_SUBJECT).unwrap();
    (raw, sidecar)
}

fn qualifiers_kept() -> WriteNotice {
    WriteNotice::QualifiersKept {
        property: "dc:subject".to_string(),
    }
}

/// A keyword edit of a list with qualifiers writes nothing and says so (ADR 0007 §8): the sidecar
/// keeps its bytes.
#[test]
fn a_keyword_edit_of_a_qualified_list_is_not_written_and_says_so() {
    let mut et = exiftool();
    let s = Scratch::new("qualified-only");
    let (raw, sidecar) = raw_with_qualified_subject(&s);
    let before = std::fs::read(&sidecar).unwrap();

    assert_edit(
        s.edit(&mut et, &raw, &[set("dc:subject", list(&["dog"]))]),
        WriteReport::Unchanged,
        &[qualifiers_kept()],
    );
    assert_eq!(std::fs::read(&sidecar).unwrap(), before);
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// A Headline edit beside a keyword edit of a list with qualifiers: the Headline is written, the
/// keywords and their qualifier stay as they were (no second "San Francisco"), and the notice is
/// given once (ADR 0007 §8).
#[test]
fn the_other_edits_are_written_beside_a_qualified_list() {
    let mut et = exiftool();
    let s = Scratch::new("qualified-beside");
    let (raw, sidecar) = raw_with_qualified_subject(&s);

    assert_edit(
        s.edit(
            &mut et,
            &raw,
            &[
                set("dc:subject", list(&["San Francisco", "cat", "dog"])),
                set("photoshop:Headline", text("Rubrik")),
            ],
        ),
        written(&sidecar),
        &[qualifiers_kept()],
    );
    assert_eq!(tag(&mut et, &sidecar, "XMP-photoshop:Headline"), "Rubrik");
    assert_eq!(
        read_value(&mut et, &raw, "dc:subject"),
        Some(list(&["San Francisco", "cat"]))
    );
    assert_eq!(tag(&mut et, &sidecar, "XMP-dc:SubjectType"), "location");
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}
