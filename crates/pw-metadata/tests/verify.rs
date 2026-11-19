//! Integration tests of the file snapshot and the verification of a written copy (ADR 0007 I8a,
//! T84) against a real ExifTool: `exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run
//! through `perl`).
//!
//! A plan is written with `ExifTool::execute` and `-o` to a new file next to the original (the
//! small JPEG of `pw-thumbcache`'s fixture in a temporary folder), and the snapshots of both are
//! compared. These tests need ExifTool and fail with an explanation when it is missing, instead of
//! returning early and being counted as passed (docs/TESTING.md §4.5), as `tests/read.rs` does.
//! Files are written through `-stay_open` (UTF-8), never with a command line, whose arguments
//! Windows converts to the code page.
//!
//! The real-DNG test is ignored by default. Run it with
//! `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pw_metadata::ExifTool;
use pw_metadata::iim::CodedCharset;
use pw_metadata::reconcile::{FileKind, IimDigest};
use pw_metadata::value::{LangAlt, Metadata, Value};
use pw_metadata::verify::{FileSnapshot, VerifyError, check_file_type, verify_copy};
use pw_metadata::write_plan::{ExifPresent, FileState, WritePlan, WriteSettings, XmpPresent, plan};

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

/// A new, empty temporary folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("pw-metadata-verify-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `plan` to `file` with `-o copy`: the original is left as it is.
fn write_copy(et: &mut ExifTool, file: &Path, copy: &Path, plan: &WritePlan) {
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
}

fn take_snapshot(et: &mut ExifTool, file: &Path) -> FileSnapshot {
    match et.snapshot(file) {
        Ok(snapshot) => snapshot,
        Err(e) => panic!("snapshot of {} failed: {e}", file.display()),
    }
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

/// A file of `kind` without IIM or Exif fields.
fn bare_state(kind: FileKind) -> FileState {
    FileState {
        kind,
        iim_present: false,
        iim_charset: CodedCharset::Absent,
        iim_values: Vec::new(),
        digest: IimDigest::Missing,
        exif_present: ExifPresent::default(),
        xmp_present: XmpPresent::default(),
    }
}

/// The plan without the arguments that start with one of `prefixes`.
fn without(plan: &WritePlan, prefixes: &[&str]) -> WritePlan {
    let mut plan = plan.clone();
    plan.args
        .retain(|arg| !prefixes.iter().any(|prefix| arg.starts_with(prefix)));
    plan
}

/// City, a Description in two languages and keywords, with IIM created by the setting, written to
/// a copy: the copy passes and its image data hash is the original's. The same copy against the
/// plan without the XMP City argument is `Unexpected` for `XMP-photoshop:City`; without both City
/// arguments (XMP and IIM) the first unexpected key in key order is `IPTC:City`.
#[test]
fn a_copy_written_by_its_plan_passes_and_a_change_outside_the_plan_does_not() {
    let mut et = exiftool();
    let dir = temp_dir("jpeg");
    let file = dir.join("bild ä.jpg");
    std::fs::write(&file, SMALL).unwrap();
    let copy = dir.join("bild ä.pwtmp-1.jpg");

    let mut metadata = Metadata::new();
    metadata.set("photoshop:City", text("Malmö")).unwrap();
    metadata
        .set(
            "dc:description",
            lang_alt(&[("x-default", "Utsikt"), ("sv", "Utsikt på svenska")]),
        )
        .unwrap();
    metadata
        .set("dc:subject", list(&["fjäll", "sjö, strand"]))
        .unwrap();
    let plan = plan(
        &metadata,
        &bare_state(FileKind::Jpeg),
        WriteSettings { create_iim: true },
    )
    .unwrap();
    assert!(
        plan.args
            .iter()
            .any(|arg| arg == "-IPTC:CodedCharacterSet=UTF8"),
        "IIM is created: {plan:?}"
    );
    assert!(
        plan.args.iter().any(|arg| arg == "-IPTCDigest=new"),
        "{plan:?}"
    );
    write_copy(&mut et, &file, &copy, &plan);

    let original = take_snapshot(&mut et, &file);
    let written = take_snapshot(&mut et, &copy);
    assert_eq!(original.file_type, "JPEG");
    assert!(original.image_data_hash.is_some(), "{original:?}");
    assert_eq!(original.image_data_hash, written.image_data_hash);
    assert_eq!(original.size, SMALL.len() as u64);
    assert!(written.size > original.size);
    assert_eq!(
        written
            .listing
            .get("XMP-photoshop:City")
            .map(String::as_str),
        Some("Malmö")
    );
    assert_eq!(verify_copy(&original, &written, &plan), Ok(()));

    assert_eq!(
        verify_copy(
            &original,
            &written,
            &without(&plan, &["-XMP-photoshop:City="])
        ),
        Err(VerifyError::Unexpected {
            key: "XMP-photoshop:City".to_string(),
            before: None,
            after: Some("Malmö".to_string()),
        })
    );
    assert_eq!(
        verify_copy(
            &original,
            &written,
            &without(&plan, &["-XMP-photoshop:City=", "-IPTC:City="])
        ),
        Err(VerifyError::Unexpected {
            key: "IPTC:City".to_string(),
            before: None,
            after: Some("Malmö".to_string()),
        })
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A JPEG renamed `.dng` is refused by its content.
#[test]
fn a_jpeg_renamed_dng_is_the_wrong_file_type() {
    let mut et = exiftool();
    let dir = temp_dir("renamed");
    let file = dir.join("bild.dng");
    std::fs::write(&file, SMALL).unwrap();
    let snapshot = take_snapshot(&mut et, &file);
    assert_eq!(
        check_file_type(FileKind::Dng, &snapshot),
        Err(VerifyError::WrongFileType {
            expected: FileKind::Dng,
            found: "JPEG".to_string(),
        })
    );
    assert_eq!(check_file_type(FileKind::Jpeg, &snapshot), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A missing file is ExifTool's error.
#[test]
fn the_snapshot_of_a_missing_file_is_exiftools_error() {
    let mut et = exiftool();
    let dir = temp_dir("missing");
    match et.snapshot(&dir.join("saknas.jpg")) {
        Err(pw_metadata::Error::ExifTool(message)) => {
            assert!(message.contains("not found"), "{message}");
        }
        other => panic!("expected ExifTool's error, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every DNG in `PW_TESTDATA`, copied to a temporary folder, written with a City plan and `-o`:
/// the copy passes `verify_copy`. Prints the `Validate` counts and the keys that changed for each
/// file (run with `--nocapture` to see them).
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn every_real_dng_written_with_a_city_plan_passes() {
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
    assert!(
        !dngs.is_empty(),
        "no DNG in PW_TESTDATA {}",
        testdata.display()
    );

    let mut metadata = Metadata::new();
    metadata.set("photoshop:City", text("Malmö")).unwrap();
    let plan = plan(
        &metadata,
        &bare_state(FileKind::Dng),
        WriteSettings::default(),
    )
    .unwrap();

    let mut et = exiftool();
    let dir = temp_dir("dng");
    let mut failures = Vec::new();
    for source in &dngs {
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let file = dir.join(&name);
        std::fs::copy(source, &file).unwrap();
        let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
        let copy = dir.join(format!("{stem}.pwtmp-1.dng"));
        write_copy(&mut et, &file, &copy, &plan);

        let original = take_snapshot(&mut et, &file);
        let written = take_snapshot(&mut et, &copy);
        let changed: BTreeSet<&str> = original
            .listing
            .keys()
            .chain(written.listing.keys())
            .filter(|key| original.listing.get(*key) != written.listing.get(*key))
            .map(String::as_str)
            .collect();
        let result = check_file_type(FileKind::Dng, &original)
            .and_then(|()| verify_copy(&original, &written, &plan));
        println!(
            "{name}: Validate {:?} -> {:?}; changed: {}; {}",
            original.validate,
            written.validate,
            changed.into_iter().collect::<Vec<_>>().join(", "),
            match &result {
                Ok(()) => "passes".to_string(),
                Err(e) => e.to_string(),
            }
        );
        if let Err(e) = result {
            failures.push(format!("{name}: {e}"));
        }
        std::fs::remove_file(&copy).unwrap();
        std::fs::remove_file(&file).unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}
