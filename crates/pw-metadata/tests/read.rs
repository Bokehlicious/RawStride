//! Integration tests of the reader (ADR 0007 I6, T82) against a real ExifTool: `exiftool` on
//! PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! These tests need ExifTool and fail with an explanation when it is missing, instead of returning
//! early and being counted as passed (docs/TESTING.md §4.5), as `tests/sidecar.rs` does. The files
//! are written with `ExifTool::execute` (UTF-8 through `-stay_open`), never with a command line,
//! whose arguments Windows converts to the code page.
//!
//! The real-DNG test is ignored by default. Run it with
//! `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`.

use std::path::{Path, PathBuf};

use pw_metadata::ExifTool;
use pw_metadata::date::IptcDate;
use pw_metadata::iim::{CodedCharset, Fallback};
use pw_metadata::read::{FileNotice, MetadataRead};
use pw_metadata::reconcile::{FileKind, IimDigest};
use pw_metadata::value::{ConflictRule, LangAlt, Origin, StructValue, Value};
use pw_metadata::write_plan::ExifPresent;

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

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pw-metadata-read-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A copy of the small JPEG in a new temporary folder.
fn jpeg(name: &str) -> (PathBuf, PathBuf) {
    let dir = temp_dir(name);
    let file = dir.join("bild ä.jpg");
    std::fs::write(&file, SMALL).unwrap();
    (dir, file)
}

/// Writes `tags` to `file` (created if it is an `.xmp` that does not exist yet).
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

fn read_file(et: &mut ExifTool, file: &Path) -> MetadataRead {
    match et.read_metadata(file, Fallback::Windows1252) {
        Ok(read) => read,
        Err(e) => panic!("reading {} failed: {e}", file.display()),
    }
}

/// The value and origin of a property.
fn get(read: &MetadataRead, id: &str) -> Option<(Value, Origin)> {
    read.reconciled
        .metadata
        .get(id)
        .map(|stored| (stored.value.clone(), stored.origin))
}

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

fn structure(fields: Vec<(&str, Value)>) -> StructValue {
    StructValue {
        fields: fields
            .into_iter()
            .map(|(id, value)| (id.to_string(), value))
            .collect(),
    }
}

/// ExifTool's own view of the IIM checksum of `file`: (stored, current).
fn digests(et: &mut ExifTool, file: &Path) -> (String, String) {
    let out = et
        .execute(&[
            "-s3".into(),
            "-f".into(),
            "-Photoshop:IPTCDigest".into(),
            "-CurrentIPTCDigest".into(),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    let mut lines = out.lines().map(str::to_string);
    (
        lines.next().unwrap_or_default(),
        lines.next().unwrap_or_default(),
    )
}

/// XMP and IIM written together with `-IPTCDigest=new`: everything is read back exactly, the
/// checksum matches and there is no conflict. Then IIM City is changed without updating the
/// checksum: the checksum differs, so IIM wins with a conflict of rule `Iim`.
#[test]
fn a_jpeg_is_read_back_exactly_and_a_changed_iim_wins_by_its_checksum() {
    let mut et = exiftool();
    let (dir, file) = jpeg("jpeg");
    write(
        &mut et,
        &file,
        &[
            "-XMP-dc:Description=Hej",
            "-XMP-dc:Description-sv=Hej på svenska",
            "-XMP-dc:Creator=Anna Åberg",
            "-XMP-dc:Creator=Bertil",
            "-XMP-iptcCore:CreatorContactInfo={CiAdrCity=Göteborg,CiEmailWork=anna@example.com}",
            "-XMP-iptcExt:LocationCreated=[{City=Malmö,LocationName=Torget}]",
            "-XMP-xmp:Rating=2.5",
            "-XMP-photoshop:DateCreated=2026:11",
            "-XMP-photoshop:City=Göteborg",
            "-XMP-dc:Subject=k1",
            "-XMP-dc:Subject=k2",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Göteborg",
            "-IPTC:Keywords=k1",
            "-IPTC:Keywords=k2",
            "-IPTCDigest=new",
        ],
    );
    let (stored, current) = digests(&mut et, &file);
    assert_eq!(stored, current, "precondition: the checksum matches");

    let first = read_file(&mut et, &file);
    let embedded = |value: Value| Some((value, Origin::XmpEmbedded));
    assert_eq!(
        get(&first, "dc:description"),
        embedded(lang_alt(&[("x-default", "Hej"), ("sv", "Hej på svenska")]))
    );
    assert_eq!(
        get(&first, "dc:creator"),
        embedded(list(&["Anna Åberg", "Bertil"]))
    );
    assert_eq!(
        get(&first, "Iptc4xmpCore:CreatorContactInfo"),
        embedded(Value::Struct(structure(vec![
            ("Iptc4xmpCore:CiAdrCity", text("Göteborg")),
            ("Iptc4xmpCore:CiEmailWork", text("anna@example.com")),
        ])))
    );
    assert_eq!(
        get(&first, "Iptc4xmpExt:LocationCreated"),
        embedded(Value::StructList(vec![structure(vec![
            (
                "Iptc4xmpExt:LocationName",
                lang_alt(&[("x-default", "Torget")])
            ),
            ("Iptc4xmpExt:City", text("Malmö")),
        ])]))
    );
    assert_eq!(
        get(&first, "xmp:Rating"),
        embedded(Value::Number("2.5".to_string()))
    );
    assert_eq!(
        get(&first, "photoshop:DateCreated"),
        embedded(Value::Date(IptcDate::parse_xmp("2026-11").unwrap()))
    );
    assert_eq!(get(&first, "photoshop:City"), embedded(text("Göteborg")));
    assert_eq!(get(&first, "dc:subject"), embedded(list(&["k1", "k2"])));
    assert!(
        first.reconciled.metadata.conflicts().is_empty(),
        "{:?}",
        first.reconciled.metadata.conflicts()
    );
    assert!(
        first.reconciled.notices.is_empty(),
        "{:?}",
        first.reconciled.notices
    );
    assert!(first.file_notices.is_empty(), "{:?}", first.file_notices);
    assert_eq!(first.reconciled.suggested_date_created, None);

    // IIM City changed without -IPTCDigest=new: the stored checksum is now old.
    write(&mut et, &file, &["-IPTC:City=Lund"]);
    let (stored, current) = digests(&mut et, &file);
    assert_ne!(stored, current, "precondition: the checksum differs");

    let second = read_file(&mut et, &file);
    assert_eq!(
        get(&second, "photoshop:City"),
        Some((text("Lund"), Origin::Iim))
    );
    let conflicts = second.reconciled.metadata.conflicts();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].property, "photoshop:City");
    assert_eq!(conflicts[0].rule, ConflictRule::Iim);
    assert_eq!(
        conflicts[0].values,
        [
            (Origin::XmpEmbedded, text("Göteborg")),
            (Origin::Iim, text("Lund")),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// IIM without a checksum and a different XMP City: the conflict stays open and XMP is kept.
#[test]
fn iim_without_a_checksum_gives_an_open_conflict() {
    let mut et = exiftool();
    let (dir, file) = jpeg("missing-digest");
    write(
        &mut et,
        &file,
        &[
            "-XMP-photoshop:City=Göteborg",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Lund",
        ],
    );
    let (stored, _) = digests(&mut et, &file);
    assert_eq!(stored, "-", "precondition: no stored checksum");

    let read = read_file(&mut et, &file);
    assert_eq!(
        get(&read, "photoshop:City"),
        Some((text("Göteborg"), Origin::XmpEmbedded))
    );
    let conflicts = read.reconciled.metadata.conflicts();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].rule, ConflictRule::Open);
    assert_eq!(
        conflicts[0].values,
        [
            (Origin::XmpEmbedded, text("Göteborg")),
            (Origin::Iim, text("Lund")),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Only Exif Artist and ImageDescription: both fill the IPTC properties.
#[test]
fn exif_artist_and_image_description_fill_creator_and_description() {
    let mut et = exiftool();
    let (dir, file) = jpeg("exif");
    write(
        &mut et,
        &file,
        &[
            "-IFD0:Artist=Anna Åberg; Bertil",
            "-IFD0:ImageDescription=Solnedgång",
        ],
    );

    let read = read_file(&mut et, &file);
    assert_eq!(
        get(&read, "dc:creator"),
        Some((list(&["Anna Åberg", "Bertil"]), Origin::Exif))
    );
    assert_eq!(
        get(&read, "dc:description"),
        Some((lang_alt(&[("x-default", "Solnedgång")]), Origin::Exif))
    );
    assert!(read.reconciled.metadata.conflicts().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Exif ImageEditor fills an absent Contributor with one content editor (ADR 0007 §6.6, T192), and
/// the file state names the tag as present.
#[test]
fn exif_image_editor_fills_contributor() {
    let mut et = exiftool();
    let (dir, file) = jpeg("exif-image-editor");
    write(&mut et, &file, &["-ExifIFD:ImageEditor=Åsa Öberg"]);

    let read = read_file(&mut et, &file);
    assert_eq!(
        get(&read, "Iptc4xmpExt:Contributor"),
        Some((
            Value::StructList(vec![structure(vec![
                ("Iptc4xmpExt:Name", lang_alt(&[("x-default", "Åsa Öberg")])),
                (
                    "Iptc4xmpExt:Role",
                    list(&["https://cv.iptc.org/newscodes/contentprodpartyrole/contentEditor"])
                ),
            ])]),
            Origin::Exif
        ))
    );
    assert!(read.reconciled.metadata.conflicts().is_empty());
    assert!(read.state.exif_present.image_editor);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A manufacturer RAW reads its sidecar `<basename>.xmp` (also with the extension in another
/// case) and never `<basename>.<ext>.xmp`. The RAW file is not changed.
#[test]
fn a_raw_file_reads_its_sidecar_and_ignores_a_sidecar_with_the_extension() {
    let mut et = exiftool();
    let dir = temp_dir("raw");
    let raw = dir.join("TN0001.ARW");
    std::fs::write(&raw, b"not really a raw file").unwrap();
    write(
        &mut et,
        &dir.join("TN0001.xmp"),
        &["-XMP-photoshop:City=Göteborg", "-XMP-xmp:Rating=4"],
    );

    let read = read_file(&mut et, &raw);
    assert_eq!(
        get(&read, "photoshop:City"),
        Some((text("Göteborg"), Origin::XmpSidecar))
    );
    assert_eq!(
        get(&read, "xmp:Rating"),
        Some((Value::Number("4".to_string()), Origin::XmpSidecar))
    );
    assert!(read.file_notices.is_empty(), "{:?}", read.file_notices);
    assert_eq!(std::fs::read(&raw).unwrap(), b"not really a raw file");

    // The sidecar's name in another case is still the sidecar.
    let raw_upper = dir.join("TN0003.ARW");
    std::fs::write(&raw_upper, b"not really a raw file").unwrap();
    write(
        &mut et,
        &dir.join("TN0003.xmp"),
        &["-XMP-photoshop:City=Lund"],
    );
    std::fs::rename(dir.join("TN0003.xmp"), dir.join("tn0003.XMP")).unwrap();
    assert_eq!(
        get(&read_file(&mut et, &raw_upper), "photoshop:City"),
        Some((text("Lund"), Origin::XmpSidecar))
    );

    // `<basename>.<ext>.xmp` is reported and never read.
    let raw_ext = dir.join("TN0002.ARW");
    std::fs::write(&raw_ext, b"not really a raw file").unwrap();
    let ext_sidecar = dir.join("TN0002.ARW.xmp");
    write(&mut et, &ext_sidecar, &["-XMP-photoshop:City=Göteborg"]);

    let read = read_file(&mut et, &raw_ext);
    assert_eq!(get(&read, "photoshop:City"), None);
    assert_eq!(read.file_notices.len(), 1, "{:?}", read.file_notices);
    match &read.file_notices[0] {
        FileNotice::ExtSidecarIgnored(path) => {
            assert_eq!(path.file_name(), ext_sidecar.file_name());
        }
        other => panic!("expected ExtSidecarIgnored, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// An XMP packet whose `dc:subject` has one item with a qualifier (`ex:type`) and one plain item
/// (ADR 0007 §8; checked with ExifTool 13.59).
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

/// The qualified item of [`QUALIFIED_SUBJECT`] and the same item without its qualifier.
const QUALIFIED_ITEM: &str = "<rdf:li rdf:parseType='Resource'><rdf:value>San Francisco</rdf:value><ex:type>location</ex:type></rdf:li>";
const PLAIN_ITEM: &str = "<rdf:li>San Francisco</rdf:li>";

/// True when `read` marks `dc:subject` and has the notice `ListWithQualifiers("dc:subject")`.
fn subject_marked(read: &MetadataRead) -> (bool, bool) {
    (
        read.reconciled.metadata.has_qualifiers("dc:subject"),
        read.file_notices
            .contains(&FileNotice::ListWithQualifiers("dc:subject".to_string())),
    )
}

/// A sidecar keyword with a qualifier marks `dc:subject` and is still read as a list (ADR 0007
/// §8); the same sidecar without the qualifier marks nothing.
#[test]
fn a_sidecar_keyword_with_a_qualifier_is_marked() {
    let mut et = exiftool();
    let dir = temp_dir("qualified-sidecar");
    let raw = dir.join("TN0001.ARW");
    std::fs::write(&raw, b"not really a raw file").unwrap();
    let sidecar = dir.join("TN0001.xmp");
    std::fs::write(&sidecar, QUALIFIED_SUBJECT).unwrap();

    let read = read_file(&mut et, &raw);
    assert_eq!(
        get(&read, "dc:subject"),
        Some((list(&["San Francisco", "cat"]), Origin::XmpSidecar))
    );
    assert_eq!(
        subject_marked(&read),
        (true, true),
        "{:?}",
        read.file_notices
    );

    std::fs::write(
        &sidecar,
        QUALIFIED_SUBJECT.replace(QUALIFIED_ITEM, PLAIN_ITEM),
    )
    .unwrap();
    let read = read_file(&mut et, &raw);
    assert_eq!(
        get(&read, "dc:subject"),
        Some((list(&["San Francisco", "cat"]), Origin::XmpSidecar))
    );
    assert_eq!(
        subject_marked(&read),
        (false, false),
        "{:?}",
        read.file_notices
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An embedded keyword with a qualifier in a JPEG marks `dc:subject` the same way (ADR 0007 §8).
#[test]
fn an_embedded_keyword_with_a_qualifier_is_marked() {
    let mut et = exiftool();
    let (dir, file) = jpeg("qualified-jpeg");
    let packet = dir.join("packet.xmp");
    std::fs::write(&packet, QUALIFIED_SUBJECT).unwrap();
    write(
        &mut et,
        &file,
        &[&format!("-xmp<={}", packet.to_string_lossy())],
    );
    // The packet was copied as it is: the qualifier is in the file.
    let out = et
        .execute(&[
            "-j".to_string(),
            "-G1".to_string(),
            "-XMP-dc:all".to_string(),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    assert!(out.contains("\"XMP-dc:SubjectType\""), "{out}");

    let read = read_file(&mut et, &file);
    assert_eq!(
        get(&read, "dc:subject"),
        Some((list(&["San Francisco", "cat"]), Origin::XmpEmbedded))
    );
    assert_eq!(
        subject_marked(&read),
        (true, true),
        "{:?}",
        read.file_notices
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A missing file is the error ExifTool reports; an unknown extension is `Unsupported`.
#[test]
fn a_missing_file_and_an_unknown_kind_are_errors() {
    let mut et = exiftool();
    let dir = temp_dir("errors");
    match et.read_metadata(&dir.join("missing.jpg"), Fallback::Windows1252) {
        Err(pw_metadata::Error::ExifTool(message)) => {
            assert!(message.contains("not found"), "{message}");
        }
        other => panic!("expected an ExifTool error, got {other:?}"),
    }
    let png = dir.join("a.png");
    std::fs::write(&png, b"png").unwrap();
    assert!(matches!(
        et.read_metadata(&png, Fallback::Windows1252),
        Err(pw_metadata::Error::Unsupported(_))
    ));
    assert!(!et.is_broken());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The first DNG in `PW_TESTDATA`, copied, with a `<basename>.xmp` next to the copy: the sidecar
/// is reported and its value is not used (ADR 0007 §4.1, Q5).
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_sidecar_next_to_a_real_dng_is_ignored() {
    let Some(testdata) = std::env::var_os("PW_TESTDATA") else {
        panic!("PW_TESTDATA is not set; set it to the real camera test-data folder");
    };
    let testdata = PathBuf::from(testdata);
    let mut dngs: Vec<PathBuf> = std::fs::read_dir(&testdata)
        .unwrap_or_else(|e| panic!("cannot read PW_TESTDATA {}: {e}", testdata.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
        })
        .collect();
    dngs.sort();
    let Some(source) = dngs.first() else {
        panic!("no DNG in PW_TESTDATA {}", testdata.display());
    };

    let mut et = exiftool();
    let dir = temp_dir("dng");
    let dng = dir.join("copy.dng");
    std::fs::copy(source, &dng).unwrap();
    let sidecar = dir.join("copy.xmp");
    write(
        &mut et,
        &sidecar,
        &["-XMP-photoshop:City=Från en främmande sidecar"],
    );

    let read = read_file(&mut et, &dng);
    assert_eq!(read.file_notices.len(), 1, "{:?}", read.file_notices);
    match &read.file_notices[0] {
        FileNotice::ForeignSidecarIgnored(path) => {
            assert_eq!(path.file_name(), sidecar.file_name());
        }
        other => panic!("expected ForeignSidecarIgnored, got {other:?}"),
    }
    assert_ne!(
        get(&read, "photoshop:City").map(|(value, _)| value),
        Some(text("Från en främmande sidecar")),
        "the sidecar's value is not used"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The file state of a JPEG with IIM City and a written `IPTCDigest` (T87): IIM present, declared
/// UTF-8, the City among its values and the checksum matching.
#[test]
fn the_state_of_a_jpeg_with_iim_has_its_values_charset_and_digest() {
    let mut et = exiftool();
    let (dir, file) = jpeg("state-iim");
    write(
        &mut et,
        &file,
        &[
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Göteborg",
            "-IPTCDigest=new",
        ],
    );

    let state = read_file(&mut et, &file).state;
    assert_eq!(state.kind, FileKind::Jpeg);
    assert!(state.iim_present);
    assert_eq!(state.iim_charset, CodedCharset::Utf8);
    assert!(
        state
            .iim_values
            .contains(&("photoshop:City".to_string(), text("Göteborg"))),
        "{:?}",
        state.iim_values
    );
    assert_eq!(state.digest, IimDigest::Matches);
    assert_eq!(state.exif_present, ExifPresent::default());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The file state of a JPEG without IIM (T87): no IIM and no checksum.
#[test]
fn the_state_of_a_jpeg_without_iim_has_no_iim() {
    let mut et = exiftool();
    let (dir, file) = jpeg("state-no-iim");
    write(&mut et, &file, &["-XMP-photoshop:City=Göteborg"]);

    let state = read_file(&mut et, &file).state;
    assert_eq!(state.kind, FileKind::Jpeg);
    assert!(!state.iim_present);
    assert_eq!(state.iim_charset, CodedCharset::Absent);
    assert!(state.iim_values.is_empty(), "{:?}", state.iim_values);
    assert_eq!(state.digest, IimDigest::Missing);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The file state of a JPEG with Exif ImageDescription and Artist (T87): only those two fields are
/// present.
#[test]
fn the_state_of_a_jpeg_names_only_the_exif_fields_it_has() {
    let mut et = exiftool();
    let (dir, file) = jpeg("state-exif");
    write(
        &mut et,
        &file,
        &[
            "-IFD0:ImageDescription=Solnedgång",
            "-IFD0:Artist=Anna Åberg",
        ],
    );

    let state = read_file(&mut et, &file).state;
    assert_eq!(
        state.exif_present,
        ExifPresent {
            image_description: true,
            copyright: false,
            artist: true,
            photographer: false,
            image_editor: false,
        }
    );
    assert!(!state.iim_present);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The file state of a manufacturer RAW with a sidecar (T87): kind `Raw`, no IIM and no Exif
/// fields, because the plan never writes the RAW file.
#[test]
fn the_state_of_a_raw_has_no_iim_and_no_exif() {
    let mut et = exiftool();
    let dir = temp_dir("state-raw");
    let raw = dir.join("TN0001.ARW");
    std::fs::write(&raw, b"not really a raw file").unwrap();
    write(
        &mut et,
        &dir.join("TN0001.xmp"),
        &["-XMP-photoshop:City=Göteborg", "-XMP-xmp:Rating=4"],
    );

    let state = read_file(&mut et, &raw).state;
    assert_eq!(state.kind, FileKind::Raw);
    assert!(!state.iim_present);
    assert_eq!(state.iim_charset, CodedCharset::Absent);
    assert!(state.iim_values.is_empty(), "{:?}", state.iim_values);
    assert_eq!(state.digest, IimDigest::Missing);
    assert_eq!(state.exif_present, ExifPresent::default());
    assert_eq!(std::fs::read(&raw).unwrap(), b"not really a raw file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The sidecars are found whatever the case of their names, and the path reported is the name as
/// it is in the folder (T95): a DNG `img_1.dng` with `IMG_1.XMP` → `ForeignSidecarIgnored` naming
/// `IMG_1.XMP`.
#[test]
fn a_sidecar_in_another_case_next_to_a_dng_is_reported_by_its_name_in_the_folder() {
    let mut et = exiftool();
    let dir = temp_dir("case-dng");
    let dng = dir.join("img_1.dng");
    std::fs::write(&dng, b"not really a dng file").unwrap();
    write(
        &mut et,
        &dir.join("written.xmp"),
        &["-XMP-photoshop:City=Från en främmande sidecar"],
    );
    std::fs::rename(dir.join("written.xmp"), dir.join("IMG_1.XMP")).unwrap();

    let read = read_file(&mut et, &dng);
    assert_eq!(read.file_notices.len(), 1, "{:?}", read.file_notices);
    match &read.file_notices[0] {
        FileNotice::ForeignSidecarIgnored(path) => {
            assert_eq!(path.file_name(), Some(std::ffi::OsStr::new("IMG_1.XMP")));
            assert_eq!(path.parent(), Some(dir.as_path()));
        }
        other => panic!("expected ForeignSidecarIgnored, got {other:?}"),
    }
    assert_eq!(get(&read, "photoshop:City"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fake RAW `img_2.arw` reads its sidecar `IMG_2.xmp`; for `x.arw` the sidecar `x.arw.XMP` is
/// reported by its name in the folder and never read (T95).
#[test]
fn a_raw_finds_its_sidecars_whatever_the_case_of_their_names() {
    let mut et = exiftool();
    let dir = temp_dir("case-raw");
    let raw = dir.join("img_2.arw");
    std::fs::write(&raw, b"not really a raw file").unwrap();
    write(
        &mut et,
        &dir.join("written.xmp"),
        &["-XMP-photoshop:City=Göteborg"],
    );
    std::fs::rename(dir.join("written.xmp"), dir.join("IMG_2.xmp")).unwrap();

    let read = read_file(&mut et, &raw);
    assert_eq!(
        get(&read, "photoshop:City"),
        Some((text("Göteborg"), Origin::XmpSidecar))
    );
    assert!(read.file_notices.is_empty(), "{:?}", read.file_notices);

    let raw_ext = dir.join("x.arw");
    std::fs::write(&raw_ext, b"not really a raw file").unwrap();
    write(
        &mut et,
        &dir.join("written-ext.xmp"),
        &["-XMP-photoshop:City=Lund"],
    );
    std::fs::rename(dir.join("written-ext.xmp"), dir.join("x.arw.XMP")).unwrap();

    let read = read_file(&mut et, &raw_ext);
    assert_eq!(get(&read, "photoshop:City"), None);
    assert_eq!(read.file_notices.len(), 1, "{:?}", read.file_notices);
    match &read.file_notices[0] {
        FileNotice::ExtSidecarIgnored(path) => {
            assert_eq!(path.file_name(), Some(std::ffi::OsStr::new("x.arw.XMP")));
            assert_eq!(path.parent(), Some(dir.as_path()));
        }
        other => panic!("expected ExtSidecarIgnored, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The time of a read in a folder of 20 000 files (T95): a JPEG next to 20 000 empty files, read
/// ten times; the time per read is printed. Ignored by default (it writes 20 000 files). Run it
/// with `cargo test --workspace --test read -- --ignored --nocapture a_read_in_a_large_folder`.
#[test]
#[ignore = "timing: writes 20 000 files; run with --ignored --nocapture"]
fn a_read_in_a_large_folder() {
    let mut et = exiftool();
    let (dir, file) = jpeg("large-folder");
    for i in 0..20_000 {
        std::fs::write(dir.join(format!("empty-{i:05}.txt")), b"").unwrap();
    }
    write(&mut et, &file, &["-XMP-photoshop:City=Göteborg"]);
    let _ = read_file(&mut et, &file);

    let reads = 10;
    let start = std::time::Instant::now();
    for _ in 0..reads {
        let read = read_file(&mut et, &file);
        assert_eq!(
            get(&read, "photoshop:City"),
            Some((text("Göteborg"), Origin::XmpEmbedded))
        );
    }
    let per_read = start.elapsed() / reads;
    println!("read_metadata in a folder of 20 001 files: {per_read:?} per read ({reads} reads)");
    let _ = std::fs::remove_dir_all(&dir);
}
