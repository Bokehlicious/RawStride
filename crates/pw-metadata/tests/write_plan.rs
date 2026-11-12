//! Integration tests of the write plan (ADR 0007 I7, T83) against a real ExifTool: `exiftool` on
//! PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! Each test writes a small JPEG in a temporary folder, applies a plan to it with
//! `ExifTool::execute` (with `-overwrite_original`, for the test only; the safe write procedure is
//! I8) and reads it back with `read_metadata`. These tests need ExifTool and fail with an
//! explanation when it is missing, instead of returning early and being counted as passed
//! (docs/TESTING.md §4.5), as `tests/read.rs` does. Files are written through `-stay_open` (UTF-8),
//! never with a command line, whose arguments Windows converts to the code page.

use std::path::{Path, PathBuf};

use pw_metadata::ExifTool;
use pw_metadata::date::IptcDate;
use pw_metadata::iim::{CodedCharset, Fallback, UTF8_CODED_CHARSET};
use pw_metadata::iim_read::{Dataset, parse_block};
use pw_metadata::read::MetadataRead;
use pw_metadata::reconcile::{FileKind, IimDigest};
use pw_metadata::value::{ConflictRule, LangAlt, Metadata, Origin, Value};
use pw_metadata::write_plan::{
    ExifPresent, FileState, WriteNotice, WritePlan, WriteSettings, plan,
};

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

/// A copy of the small JPEG in a new temporary folder.
fn jpeg(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "pw-metadata-write-plan-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bild ä.jpg");
    std::fs::write(&file, SMALL).unwrap();
    (dir, file)
}

/// Writes `tags` to `file` to set up a test.
fn write(et: &mut ExifTool, file: &Path, tags: &[&str]) {
    let mut args = vec!["-overwrite_original".to_string()];
    args.extend(tags.iter().map(|tag| tag.to_string()));
    args.push(file.to_string_lossy().into_owned());
    let out = et.execute(&args).unwrap();
    assert!(
        out.contains("1 image files updated"),
        "ExifTool did not write {}: {out}",
        file.display()
    );
}

/// Applies a plan: `-overwrite_original` (test only), the plan's options and arguments, the file.
fn apply(et: &mut ExifTool, file: &Path, plan: &WritePlan) {
    let mut args = vec!["-overwrite_original".to_string()];
    args.extend(plan.options.iter().cloned());
    args.extend(plan.args.iter().cloned());
    args.push(file.to_string_lossy().into_owned());
    let out = et.execute(&args).unwrap();
    assert!(
        out.contains("1 image files updated"),
        "ExifTool did not apply {plan:?} to {}: {out}",
        file.display()
    );
}

fn read_file(et: &mut ExifTool, file: &Path) -> MetadataRead {
    match et.read_metadata(file, Fallback::Windows1252) {
        Ok(read) => read,
        Err(e) => panic!("reading {} failed: {e}", file.display()),
    }
}

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

fn date(xmp: &str) -> Value {
    Value::Date(IptcDate::parse_xmp(xmp).unwrap())
}

/// One tag as ExifTool prints it (`-s3 -f`: `-` when it is missing).
fn tag(et: &mut ExifTool, file: &Path, name: &str) -> String {
    let out = et
        .execute(&[
            "-s3".into(),
            "-f".into(),
            format!("-{name}"),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    out.lines().next().unwrap_or_default().to_string()
}

/// ExifTool's own view of the IIM checksum of `file`: (stored, current).
fn digests(et: &mut ExifTool, file: &Path) -> (String, String) {
    (
        tag(et, file, "Photoshop:IPTCDigest"),
        tag(et, file, "CurrentIPTCDigest"),
    )
}

/// The datasets of the raw IIM block (`-b -IPTC`), empty without IIM.
fn iim_block(et: &mut ExifTool, file: &Path) -> Vec<Dataset> {
    let out = et
        .execute(&[
            "-j".into(),
            "-b".into(),
            "-IPTC".into(),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    let stdout = out.split("--- stderr ---").next().unwrap();
    let json: serde_json::Value = serde_json::from_str(stdout).unwrap();
    match json[0]["IPTC"].as_str() {
        Some(block) => parse_block(&base64(block.strip_prefix("base64:").unwrap())).unwrap(),
        None => Vec::new(),
    }
}

/// Decodes padded standard base64 (test helper).
fn base64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut bits, mut count) = (0u32, 0u32);
    for &c in text.as_bytes().iter().take_while(|&&c| c != b'=') {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("not base64: {c}"),
        };
        bits = ((bits << 6) | u32::from(value)) & 0xffff;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push(((bits >> count) & 0xff) as u8);
        }
    }
    out
}

/// The data of every dataset `record:dataset`, in file order.
fn datasets(block: &[Dataset], record: u8, dataset: u8) -> Vec<Vec<u8>> {
    block
        .iter()
        .filter(|d| d.record == record && d.dataset == dataset)
        .map(|d| d.data.clone())
        .collect()
}

fn bytes(texts: &[&str]) -> Vec<Vec<u8>> {
    texts.iter().map(|text| text.as_bytes().to_vec()).collect()
}

/// A JPEG with UTF-8 IIM, as written by the tests; `exif_present` as given.
fn state(digest: IimDigest, exif_present: ExifPresent) -> FileState {
    FileState {
        kind: FileKind::Jpeg,
        iim_present: true,
        iim_charset: CodedCharset::Utf8,
        // The IIM is UTF-8, so its values are only needed for a conversion (§5.4), which does not
        // happen here.
        iim_values: Vec::new(),
        digest,
        exif_present,
    }
}

fn make_plan(metadata: &Metadata, state: &FileState) -> WritePlan {
    plan(metadata, state, WriteSettings::default()).unwrap()
}

/// XMP, IIM with a checksum and Exif Artist are changed together: the changed values are there,
/// the other XMP language is kept, a two-line Description keeps its line break, IIM equals XMP
/// (no conflict on reading) and the checksum matches; the existing Artist gets the first creator
/// and the missing Photographer is not created; the IIM is UTF-8 with 1:90 and a Date Created
/// without a zone has no 2:60.
#[test]
fn a_plan_brings_xmp_iim_and_exif_in_line() {
    let mut et = exiftool();
    let (dir, file) = jpeg("in-line");
    write(
        &mut et,
        &file,
        &[
            "-XMP-dc:Description=Gammal",
            "-XMP-dc:Description-sv=Svensk text",
            "-XMP-photoshop:City=Lund",
            "-XMP-dc:Subject=k1",
            "-XMP-dc:Creator=Gammal Fotograf",
            "-IFD0:Artist=Gammal Fotograf",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:Caption-Abstract=Gammal",
            "-IPTC:City=Lund",
            "-IPTC:Keywords=k1",
            "-IPTC:By-line=Gammal Fotograf",
            "-IPTCDigest=new",
        ],
    );
    let before = read_file(&mut et, &file);
    assert!(before.reconciled.metadata.conflicts().is_empty());

    let mut metadata = before.reconciled.metadata.clone();
    metadata
        .set("dc:description", lang_alt(&[("x-default", "Rad 1\nRad 2")]))
        .unwrap();
    metadata.set("photoshop:City", text("Malmö")).unwrap();
    metadata
        .set("dc:subject", list(&["fjäll", "sjö, strand"]))
        .unwrap();
    metadata
        .set("dc:creator", list(&["Ny Fotograf", "Bertil"]))
        .unwrap();
    metadata
        .set("photoshop:DateCreated", date("2026-11-04T10:15"))
        .unwrap();
    let exif = ExifPresent {
        artist: true,
        ..ExifPresent::default()
    };
    let plan = make_plan(&metadata, &state(IimDigest::Matches, exif));
    assert_eq!(plan.options, ["-ec"]);
    assert!(plan.notices.is_empty(), "{:?}", plan.notices);
    apply(&mut et, &file, &plan);

    let after = read_file(&mut et, &file);
    let description = get(&after, "dc:description").unwrap().0;
    let Value::LangAlt(description) = description else {
        panic!("{description:?}");
    };
    assert_eq!(description.display(), Some(("Rad 1\nRad 2", None)));
    assert!(
        description
            .entries()
            .contains(&("sv".to_string(), "Svensk text".to_string()))
    );
    assert_eq!(
        get(&after, "photoshop:City"),
        Some((text("Malmö"), Origin::XmpEmbedded))
    );
    assert_eq!(
        get(&after, "dc:subject").unwrap().0,
        list(&["fjäll", "sjö, strand"])
    );
    assert_eq!(
        get(&after, "dc:creator").unwrap().0,
        list(&["Ny Fotograf", "Bertil"])
    );
    assert_eq!(
        get(&after, "photoshop:DateCreated").unwrap().0,
        date("2026-11-04T10:15")
    );
    // IIM equals XMP and Artist equals the first creator: no conflict.
    assert!(
        after.reconciled.metadata.conflicts().is_empty(),
        "{:?}",
        after.reconciled.metadata.conflicts()
    );
    let (stored, current) = digests(&mut et, &file);
    assert_ne!(stored, "-");
    assert_eq!(stored, current);

    assert_eq!(tag(&mut et, &file, "IFD0:Artist"), "Ny Fotograf");
    assert_eq!(tag(&mut et, &file, "ExifIFD:Photographer"), "-");
    assert_eq!(tag(&mut et, &file, "IFD0:ImageDescription"), "-");

    let block = iim_block(&mut et, &file);
    assert_eq!(datasets(&block, 1, 90), [UTF8_CODED_CHARSET.to_vec()]);
    assert_eq!(datasets(&block, 2, 90), bytes(&["Malmö"]));
    assert_eq!(datasets(&block, 2, 25), bytes(&["fjäll", "sjö, strand"]));
    assert_eq!(datasets(&block, 2, 80), bytes(&["Ny Fotograf", "Bertil"]));
    assert_eq!(datasets(&block, 2, 120), bytes(&["Rad 1\nRad 2"]));
    assert_eq!(datasets(&block, 2, 55), bytes(&["20261104"]));
    assert!(datasets(&block, 2, 60).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// Clearing a Lang Alt removes every language from XMP (one `-<tag>=` argument) and its IIM
/// dataset; the checksum is updated.
#[test]
fn a_cleared_lang_alt_has_no_language_left() {
    let mut et = exiftool();
    let (dir, file) = jpeg("clear");
    write(
        &mut et,
        &file,
        &[
            "-XMP-dc:Title=Titel",
            "-XMP-dc:Title-de=Deutsch",
            "-XMP-dc:Title-sv=Svensk",
            "-XMP-photoshop:City=Lund",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:ObjectName=Titel",
            "-IPTC:City=Lund",
            "-IPTCDigest=new",
        ],
    );
    let mut metadata = read_file(&mut et, &file).reconciled.metadata;
    assert!(metadata.get("dc:title").is_some());
    metadata.clear("dc:title").unwrap();
    let plan = make_plan(
        &metadata,
        &state(IimDigest::Matches, ExifPresent::default()),
    );
    apply(&mut et, &file, &plan);

    let after = read_file(&mut et, &file);
    assert_eq!(get(&after, "dc:title"), None);
    let out = et
        .execute(&[
            "-j".into(),
            "-G1".into(),
            "-XMP:all".into(),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    assert!(!out.contains("XMP-dc:Title"), "{out}");
    let block = iim_block(&mut et, &file);
    assert!(datasets(&block, 2, 5).is_empty());
    assert_eq!(datasets(&block, 2, 90), bytes(&["Lund"]));
    let (stored, current) = digests(&mut et, &file);
    assert_eq!(stored, current);
    let _ = std::fs::remove_dir_all(dir);
}

/// IIM without a checksum and a different XMP City: an `Open` conflict. Changing another property
/// keeps both forms of City and writes no checksum. Changing City then decides the conflict: both
/// forms are written and the checksum matches.
#[test]
fn an_open_conflict_keeps_the_digest_unchanged_until_it_is_decided() {
    let mut et = exiftool();
    let (dir, file) = jpeg("open");
    write(
        &mut et,
        &file,
        &[
            "-XMP-photoshop:City=Lund",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Malmö",
        ],
    );
    let before = read_file(&mut et, &file);
    let rules: Vec<_> = before
        .reconciled
        .metadata
        .conflicts()
        .iter()
        .map(|c| (c.property.as_str(), c.rule))
        .collect();
    assert_eq!(rules, [("photoshop:City", ConflictRule::Open)]);
    assert_eq!(digests(&mut et, &file).0, "-");

    let mut metadata = before.reconciled.metadata.clone();
    metadata.set("photoshop:Headline", text("Rubrik")).unwrap();
    let plan = make_plan(
        &metadata,
        &state(IimDigest::Missing, ExifPresent::default()),
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
    apply(&mut et, &file, &plan);

    assert_eq!(digests(&mut et, &file).0, "-");
    let after = read_file(&mut et, &file);
    assert_eq!(
        get(&after, "photoshop:Headline"),
        Some((text("Rubrik"), Origin::XmpEmbedded))
    );
    let conflicts = after.reconciled.metadata.conflicts();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].rule, ConflictRule::Open);
    assert_eq!(
        conflicts[0].values,
        [
            (Origin::XmpEmbedded, text("Lund")),
            (Origin::Iim, text("Malmö"))
        ]
    );

    let mut metadata = after.reconciled.metadata.clone();
    metadata.set("photoshop:City", text("Göteborg")).unwrap();
    let plan = make_plan(
        &metadata,
        &state(IimDigest::Missing, ExifPresent::default()),
    );
    assert!(plan.notices.is_empty(), "{:?}", plan.notices);
    apply(&mut et, &file, &plan);

    let decided = read_file(&mut et, &file);
    assert!(decided.reconciled.metadata.conflicts().is_empty());
    assert_eq!(
        get(&decided, "photoshop:City"),
        Some((text("Göteborg"), Origin::XmpEmbedded))
    );
    let block = iim_block(&mut et, &file);
    assert_eq!(datasets(&block, 2, 90), bytes(&["Göteborg"]));
    let (stored, current) = digests(&mut et, &file);
    assert_ne!(stored, "-");
    assert_eq!(stored, current);
    let _ = std::fs::remove_dir_all(dir);
}

/// IIM City changed without updating the checksum: City comes from IIM (rule `Iim`). Editing only
/// the Rating brings both forms in line: the IIM City is written to XMP too, so after the new
/// checksum City is still the IIM value and there is no conflict (§5.1, §12).
#[test]
fn an_iim_value_that_won_by_the_checksum_stays_after_the_write() {
    let mut et = exiftool();
    let (dir, file) = jpeg("differs");
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
    write(&mut et, &file, &["-IPTC:City=Malmö"]);
    let (stored, current) = digests(&mut et, &file);
    assert_ne!(stored, current);
    let before = read_file(&mut et, &file);
    let rules: Vec<_> = before
        .reconciled
        .metadata
        .conflicts()
        .iter()
        .map(|c| (c.property.as_str(), c.rule))
        .collect();
    assert_eq!(rules, [("photoshop:City", ConflictRule::Iim)]);
    assert_eq!(
        get(&before, "photoshop:City"),
        Some((text("Malmö"), Origin::Iim))
    );

    let mut metadata = before.reconciled.metadata.clone();
    metadata
        .set("xmp:Rating", Value::Number("3".into()))
        .unwrap();
    let plan = make_plan(
        &metadata,
        &state(IimDigest::Differs, ExifPresent::default()),
    );
    assert!(plan.args.contains(&"-XMP-photoshop:City=Malmö".to_string()));
    apply(&mut et, &file, &plan);

    let after = read_file(&mut et, &file);
    assert!(
        after.reconciled.metadata.conflicts().is_empty(),
        "{:?}",
        after.reconciled.metadata.conflicts()
    );
    assert_eq!(
        get(&after, "photoshop:City"),
        Some((text("Malmö"), Origin::XmpEmbedded))
    );
    assert_eq!(
        get(&after, "xmp:Rating"),
        Some((Value::Number("3".into()), Origin::XmpEmbedded))
    );
    let (stored, current) = digests(&mut et, &file);
    assert_eq!(stored, current);
    let _ = std::fs::remove_dir_all(dir);
}

/// A file without Artist and Photographer: a new creator does not create them.
#[test]
fn a_missing_artist_is_not_created() {
    let mut et = exiftool();
    let (dir, file) = jpeg("no-artist");
    write(&mut et, &file, &["-XMP-dc:Creator=Gammal"]);
    let mut metadata = read_file(&mut et, &file).reconciled.metadata;
    metadata.set("dc:creator", list(&["Ny"])).unwrap();
    let state = FileState {
        iim_present: false,
        iim_charset: CodedCharset::Absent,
        ..state(IimDigest::Missing, ExifPresent::default())
    };
    let plan = make_plan(&metadata, &state);
    apply(&mut et, &file, &plan);

    assert_eq!(
        get(&read_file(&mut et, &file), "dc:creator").unwrap().0,
        list(&["Ny"])
    );
    assert_eq!(tag(&mut et, &file, "IFD0:Artist"), "-");
    assert_eq!(tag(&mut et, &file, "ExifIFD:Photographer"), "-");
    assert!(iim_block(&mut et, &file).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// Date Created with a zone gives 2:55 and 2:60 exactly as `to_iim()`; written again without a
/// zone, 2:60 is removed (Q3).
#[test]
fn time_created_is_written_only_with_a_zone() {
    let mut et = exiftool();
    let (dir, file) = jpeg("date");
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
    let state = state(IimDigest::Matches, ExifPresent::default());

    let mut metadata = read_file(&mut et, &file).reconciled.metadata;
    metadata
        .set("photoshop:DateCreated", date("2026-11-04T10:15:30.5-05:30"))
        .unwrap();
    apply(&mut et, &file, &make_plan(&metadata, &state));
    let block = iim_block(&mut et, &file);
    assert_eq!(datasets(&block, 2, 55), bytes(&["20261104"]));
    assert_eq!(datasets(&block, 2, 60), bytes(&["101530-0530"]));
    let read = read_file(&mut et, &file);
    assert_eq!(
        get(&read, "photoshop:DateCreated").unwrap().0,
        date("2026-11-04T10:15:30.5-05:30")
    );
    assert!(read.reconciled.metadata.conflicts().is_empty());

    let mut metadata = read.reconciled.metadata;
    metadata
        .set("photoshop:DateCreated", date("2026-11"))
        .unwrap();
    apply(&mut et, &file, &make_plan(&metadata, &state));
    let block = iim_block(&mut et, &file);
    assert_eq!(datasets(&block, 2, 55), bytes(&["20261100"]));
    assert!(datasets(&block, 2, 60).is_empty());
    let read = read_file(&mut et, &file);
    assert_eq!(
        get(&read, "photoshop:DateCreated").unwrap().0,
        date("2026-11")
    );
    assert!(read.reconciled.metadata.conflicts().is_empty());
    let (stored, current) = digests(&mut et, &file);
    assert_eq!(stored, current);
    let _ = std::fs::remove_dir_all(dir);
}

/// IIM written by ExifTool without 1:90 holds Latin-1 bytes. The first write converts the
/// unchanged values to UTF-8 and declares 1:90 (§5.4).
#[test]
fn latin1_iim_is_converted_to_utf8_on_the_first_write() {
    let mut et = exiftool();
    let (dir, file) = jpeg("latin1");
    write(
        &mut et,
        &file,
        &["-IPTC:City=Göteborg", "-IPTC:Headline=Åsa"],
    );
    let block = iim_block(&mut et, &file);
    assert!(datasets(&block, 1, 90).is_empty());
    assert_eq!(datasets(&block, 2, 105), [b"\xc5sa".to_vec()]);

    let before = read_file(&mut et, &file);
    let mut metadata = before.reconciled.metadata.clone();
    // What T80 read from the IIM: decoded by the fallback encoding.
    let iim_values = vec![
        ("photoshop:City".to_string(), text("Göteborg")),
        ("photoshop:Headline".to_string(), text("Åsa")),
    ];
    for (id, value) in &iim_values {
        assert_eq!(get(&before, id), Some((value.clone(), Origin::Iim)));
    }
    metadata.set("photoshop:City", text("Malmö")).unwrap();
    let state = FileState {
        iim_charset: CodedCharset::Absent,
        iim_values,
        ..state(IimDigest::Missing, ExifPresent::default())
    };
    let plan = make_plan(&metadata, &state);
    assert!(plan.notices.contains(&WriteNotice::IimConvertedToUtf8));
    apply(&mut et, &file, &plan);

    let block = iim_block(&mut et, &file);
    assert_eq!(datasets(&block, 1, 90), [UTF8_CODED_CHARSET.to_vec()]);
    assert_eq!(datasets(&block, 2, 90), bytes(&["Malmö"]));
    assert_eq!(datasets(&block, 2, 105), bytes(&["Åsa"]));
    let after = read_file(&mut et, &file);
    assert_eq!(get(&after, "photoshop:Headline").unwrap().0, text("Åsa"));
    assert!(after.reconciled.metadata.conflicts().is_empty());
    let (stored, current) = digests(&mut et, &file);
    assert_eq!(stored, current);
    let _ = std::fs::remove_dir_all(dir);
}

/// City is the only value, in XMP and in IIM, with a checksum. Clearing it leaves no XMP, but the
/// file has a stored checksum, so the plan writes a new one (T93): applied with `-o`, the stored
/// and the current checksum of the copy are equal and City is gone (§5.2, §12).
#[test]
fn clearing_the_only_xmp_value_keeps_the_stored_digest_current() {
    let mut et = exiftool();
    let (dir, file) = jpeg("clear-only-value");
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
    let before = read_file(&mut et, &file);
    assert_eq!(before.state.digest, IimDigest::Matches, "precondition");
    assert_eq!(
        get(&before, "photoshop:City"),
        Some((text("Lund"), Origin::XmpEmbedded)),
        "precondition"
    );

    let mut metadata = before.reconciled.metadata.clone();
    metadata.clear("photoshop:City").unwrap();
    let plan = make_plan(&metadata, &before.state);
    for arg in ["-XMP-photoshop:City=", "-IPTC:City=", "-IPTCDigest=new"] {
        assert!(plan.args.contains(&arg.to_string()), "{arg} in {plan:?}");
    }

    let copy = dir.join("kopia ä.jpg");
    let mut args = plan.options.clone();
    args.extend(plan.args.iter().cloned());
    args.push("-o".to_string());
    args.push(copy.to_string_lossy().into_owned());
    args.push(file.to_string_lossy().into_owned());
    let out = et.execute(&args).unwrap();
    assert!(
        out.contains("1 image files created") && copy.is_file(),
        "ExifTool did not write {} from {}: {out}",
        copy.display(),
        file.display()
    );

    let (stored, current) = digests(&mut et, &copy);
    assert_ne!(stored, "-");
    assert_eq!(stored, current);
    let after = read_file(&mut et, &copy);
    assert_eq!(get(&after, "photoshop:City"), None);
    assert_eq!(after.state.digest, IimDigest::Matches);
    let _ = std::fs::remove_dir_all(dir);
}
