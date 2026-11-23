//! Integration tests of the template file (ADR 0017 U13a, T285): saving a template as a plain XMP
//! file and reading one back, also a file another program wrote, against a real ExifTool: the
//! `exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`), started with
//! `-config` and the config [`write_exiftool_config`] writes in the test's temporary folder.
//!
//! These tests need ExifTool and fail with an explanation when it is missing, instead of returning
//! early and being counted as passed (docs/TESTING.md §4.5). Each test has a temporary folder of
//! its own.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pw_metadata::ExifTool;
use pw_metadata::date::IptcDate;
use pw_metadata::template_file::{
    EXIFTOOL_CONFIG, RAWSTRIDE_NAMESPACE, TemplateFile, TemplateFileError, TemplateRead,
    write_exiftool_config,
};
use pw_metadata::value::{LangAlt, StructValue, Value};

/// The small baseline JPEG of `crates/pw-thumbcache/src/jpeg_fixture.rs`, copied (as in
/// `tests/template.rs`).
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

const LOCATION_CREATED: &str = "Iptc4xmpExt:LocationCreated";
const LOCATION_SHOWN: &str = "Iptc4xmpExt:LocationShown";
const CONTACT_INFO: &str = "Iptc4xmpCore:CreatorContactInfo";
const CITY: &str = "Iptc4xmpExt:City";
const SUBLOCATION: &str = "Iptc4xmpExt:Sublocation";
const LATITUDE: &str = "exif:GPSLatitude";
const EMAIL: &str = "Iptc4xmpCore:CiEmailWork";

/// Starts ExifTool with `pre_args` after the program (and after the script with
/// `PW_EXIFTOOL_PL`) and checks that it answers. Panics (fails the test) if it is unavailable.
fn start(pre_args: &[&str]) -> ExifTool {
    let (started, source) = match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => {
            let mut args = vec![pl.as_str()];
            args.extend_from_slice(pre_args);
            (
                ExifTool::start_with("perl", &args),
                format!("perl {pl} (PW_EXIFTOOL_PL)"),
            )
        }
        Err(_) => (
            ExifTool::start_with("exiftool", pre_args),
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

/// ExifTool started with `-config` and the config written in `dir` (as U13c starts it).
fn exiftool_with_config(dir: &Path) -> ExifTool {
    let config = write_exiftool_config(dir).unwrap();
    let config = config.to_string_lossy().into_owned();
    start(&["-config", &config])
}

/// ExifTool started without the config, as another program would use it.
fn exiftool_without_config() -> ExifTool {
    start(&[])
}

/// A new, empty temporary folder.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pw-metadata-template-file-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The names of the entries of `dir`, sorted.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Writes `tags` to a new file `file` with plain ExifTool, as another program would.
fn write_other(et: &mut ExifTool, file: &Path, tags: &[&str]) {
    let mut args: Vec<String> = tags.iter().map(|tag| tag.to_string()).collect();
    args.push("-o".to_string());
    args.push(file.to_string_lossy().into_owned());
    let out = et.execute(&args).unwrap();
    assert!(
        out.contains("1 image files created"),
        "ExifTool did not write {}: {out}",
        file.display()
    );
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

fn date(text: &str) -> Value {
    Value::Date(IptcDate::parse_xmp(text).unwrap())
}

fn entry(fields: &[(&str, Value)]) -> StructValue {
    StructValue {
        fields: fields
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect(),
    }
}

fn template(fields: &[(&str, Value)], add: &[&str]) -> TemplateFile {
    TemplateFile {
        fields: fields
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect(),
        add: add.iter().map(|id| id.to_string()).collect(),
    }
}

/// `template` as `TemplateFile::new` makes it.
fn made(template: &TemplateFile) -> TemplateFile {
    TemplateFile::new(template.fields.clone(), template.add.clone())
}

fn save(et: &mut ExifTool, path: &Path, template: &TemplateFile) {
    if let Err(error) = et.write_template_file(path, template) {
        panic!("{} was not saved: {error}", path.display());
    }
}

fn load(et: &mut ExifTool, path: &Path) -> TemplateRead {
    match et.read_template_file(path) {
        Ok(read) => read,
        Err(error) => panic!("{} was not read: {error}", path.display()),
    }
}

/// True when the values are equal, a number (also inside a structure) compared as a number within
/// 1e-9 of the larger, as T285 point 3 says.
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
            (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
        }
        (Value::Struct(a), Value::Struct(b)) => same_struct(a, b),
        (Value::StructList(a), Value::StructList(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_struct(a, b))
        }
        (a, b) => a == b,
    }
}

fn same_struct(a: &StructValue, b: &StructValue) -> bool {
    a.fields.len() == b.fields.len()
        && a.fields
            .iter()
            .zip(&b.fields)
            .all(|((id_a, a), (id_b, b))| id_a == id_b && same_value(a, b))
}

/// Fails unless `read` holds `expected` (compared by [`same_value`]) and nothing was skipped,
/// dropped or unreadable.
fn assert_read_as(read: &TemplateRead, expected: &TemplateFile) {
    let ids = |template: &TemplateFile| -> Vec<String> {
        template.fields.iter().map(|(id, _)| id.clone()).collect()
    };
    assert_eq!(ids(&read.template), ids(expected));
    for ((id, got), (_, want)) in read.template.fields.iter().zip(&expected.fields) {
        assert!(
            same_value(got, want),
            "{id}: read {got:?}, expected {want:?}"
        );
    }
    assert_eq!(read.template.add, expected.add);
    assert_eq!(read.skipped, Vec::<String>::new());
    assert_eq!(read.dropped, Vec::<String>::new());
    assert_eq!(read.unreadable, Vec::<(String, String)>::new());
}

/// The namespace prefixes the XMP text declares (`xmlns:<prefix>=`), sorted, each once.
fn declared_prefixes(xmp: &str) -> Vec<String> {
    let mut prefixes: Vec<String> = xmp
        .split("xmlns:")
        .skip(1)
        .filter_map(|rest| rest.split_once('=').map(|(prefix, _)| prefix.to_string()))
        .collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes
}

/// The `rdf:li` entries of `rawstride:AddTo` in the XMP text.
fn add_to_entries(xmp: &str) -> Vec<String> {
    let Some((_, rest)) = xmp.split_once("<rawstride:AddTo>") else {
        return Vec::new();
    };
    let Some((bag, _)) = rest.split_once("</rawstride:AddTo>") else {
        return Vec::new();
    };
    bag.split("<rdf:li>")
        .skip(1)
        .filter_map(|rest| rest.split_once("</rdf:li>").map(|(id, _)| id.to_string()))
        .collect()
}

/// The template of the round trip (T285, "Round trip").
fn round_trip_template() -> TemplateFile {
    template(
        &[
            ("photoshop:Headline", text("Hamn å ä ö\nandra raden")),
            ("dc:description", x_default("Båtar i hamnen")),
            ("dc:creator", list(&["Anna", "Bo"])),
            ("dc:subject", list(&["hav", "båt"])),
            ("photoshop:DateCreated", date("2026-11-23T14:30:00+01:00")),
            (
                LOCATION_CREATED,
                Value::StructList(vec![entry(&[
                    (CITY, text("Lund")),
                    (SUBLOCATION, text("Hamnen")),
                ])]),
            ),
            (
                CONTACT_INFO,
                Value::Struct(entry(&[(EMAIL, text("foto@example.com"))])),
            ),
            (
                LOCATION_SHOWN,
                Value::StructList(vec![
                    entry(&[
                        (LATITUDE, Value::Number("59.3293".to_string())),
                        (CITY, text("{folder}")),
                    ]),
                    entry(&[(CITY, text("Malmö"))]),
                ]),
            ),
            ("photoshop:Instructions", text("{date} {name}")),
        ],
        &["dc:subject"],
    )
}

/// Every kind of field the template window has, saved and read back: the same template, nothing
/// skipped, dropped or unreadable. A plain date too, in a second file.
#[test]
fn a_template_is_saved_and_read_back_the_same() {
    let dir = temp_dir("round-trip");
    let mut et = exiftool_with_config(&dir);
    let path = dir.join("Hamnen å.xmp");
    let saved = round_trip_template();
    save(&mut et, &path, &saved);
    let expected = made(&saved);
    assert_eq!(expected.fields.len(), 9);
    assert_read_as(&load(&mut et, &path), &expected);

    let plain_date = template(&[("photoshop:DateCreated", date("2026-11-23"))], &[]);
    let path = dir.join("datum.xmp");
    save(&mut et, &path, &plain_date);
    assert_read_as(&load(&mut et, &path), &made(&plain_date));
    assert_eq!(
        entries(&dir),
        ["Hamnen å.xmp", "datum.xmp", "rawstride-exiftool.config"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Another program's file whose description has only `en-US`: named in `dropped`; its
/// Intellectual Genre (not in a template: the template window does not show it) in `skipped`.
#[test]
fn a_lang_alt_without_x_default_is_dropped_and_a_property_not_in_a_template_skipped() {
    let dir = temp_dir("dropped");
    let mut other = exiftool_without_config();
    let path = dir.join("other.xmp");
    write_other(
        &mut other,
        &path,
        &[
            "-XMP-dc:Description-en-US=Boats",
            "-XMP-iptcCore:IntellectualGenre=Feature",
            "-XMP-photoshop:Headline=Hamn",
        ],
    );
    let mut et = exiftool_with_config(&dir);
    let read = load(&mut et, &path);
    assert_eq!(
        read.template,
        template(&[("photoshop:Headline", text("Hamn"))], &[])
    );
    assert_eq!(read.dropped, ["dc:description"]);
    assert_eq!(read.skipped, ["Iptc4xmpCore:IntellectualGenre"]);
    assert_eq!(read.unreadable, Vec::<(String, String)>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A text file and a JPEG named `.xmp` are not template files.
#[test]
fn a_file_that_is_not_xmp_is_the_error() {
    let dir = temp_dir("not-xmp");
    let mut et = exiftool_with_config(&dir);
    let text_file = dir.join("text.xmp");
    std::fs::write(&text_file, "just some text\n").unwrap();
    let jpeg = dir.join("bild.xmp");
    std::fs::write(&jpeg, SMALL).unwrap();
    for path in [&text_file, &jpeg] {
        match et.read_template_file(path) {
            Err(TemplateFileError::NotXmp { .. } | TemplateFileError::Unreadable { .. }) => {}
            other => panic!("{}: {other:?}", path.display()),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The saved file declares only `x`, `rdf`, the namespaces of the fields written and `rawstride`,
/// whose `AddTo` holds `dc:subject` only; without + boxes there is no `rawstride` namespace.
#[test]
fn the_file_holds_only_standard_xmp_and_the_one_property() {
    let dir = temp_dir("standard");
    let mut et = exiftool_with_config(&dir);
    let path = dir.join("standard.xmp");
    save(
        &mut et,
        &path,
        &template(
            &[
                ("photoshop:Headline", text("Hamn")),
                ("dc:creator", list(&["Anna"])),
                ("dc:subject", list(&["hav"])),
            ],
            &["dc:subject"],
        ),
    );
    let xmp = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        declared_prefixes(&xmp),
        ["dc", "photoshop", "rawstride", "rdf", "x"]
    );
    assert!(xmp.contains(RAWSTRIDE_NAMESPACE), "{xmp}");
    assert_eq!(add_to_entries(&xmp), ["dc:subject"]);

    let path = dir.join("no-plus.xmp");
    save(
        &mut et,
        &path,
        &template(&[("dc:subject", list(&["hav"]))], &[]),
    );
    let xmp = std::fs::read_to_string(&path).unwrap();
    assert_eq!(declared_prefixes(&xmp), ["dc", "rdf", "x"]);
    assert!(!xmp.contains("rawstride"), "{xmp}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A checked field without content and a + box on an unchecked field are not saved; an empty
/// template is the error and makes no file.
#[test]
fn what_is_not_saved() {
    let dir = temp_dir("not-saved");
    let mut et = exiftool_with_config(&dir);
    let path = dir.join("some.xmp");
    save(
        &mut et,
        &path,
        &template(
            &[
                ("photoshop:City", text("  ")),
                ("photoshop:Headline", text("Hamn")),
                ("dc:creator", list(&["Anna"])),
            ],
            &["dc:subject"],
        ),
    );
    let xmp = std::fs::read_to_string(&path).unwrap();
    assert!(!xmp.contains("photoshop:City"), "{xmp}");
    assert!(!xmp.contains("dc:subject"), "{xmp}");
    assert!(!xmp.contains("rawstride"), "{xmp}");
    assert_read_as(
        &load(&mut et, &path),
        &made(&template(
            &[
                ("photoshop:Headline", text("Hamn")),
                ("dc:creator", list(&["Anna"])),
            ],
            &[],
        )),
    );

    let empty = dir.join("empty.xmp");
    assert_eq!(
        et.write_template_file(
            &empty,
            &template(&[("photoshop:City", text(""))], &["dc:subject"])
        ),
        Err(TemplateFileError::Empty)
    );
    assert!(!empty.exists());
    assert_eq!(entries(&dir), ["rawstride-exiftool.config", "some.xmp"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file plain ExifTool wrote with keywords, Rating and Label: the keywords as a checked field
/// without + box; Rating and Label skipped.
#[test]
fn another_programs_file_loads_with_every_plus_unchecked() {
    let dir = temp_dir("other");
    let mut other = exiftool_without_config();
    let path = dir.join("other.xmp");
    write_other(
        &mut other,
        &path,
        &[
            "-XMP-dc:Subject=hav",
            "-XMP-dc:Subject=båt",
            "-XMP-xmp:Rating=3",
            "-XMP-xmp:Label=Red",
        ],
    );
    let mut et = exiftool_with_config(&dir);
    let read = load(&mut et, &path);
    assert_eq!(
        read.template,
        template(&[("dc:subject", list(&["hav", "båt"]))], &[])
    );
    let mut skipped = read.skipped.clone();
    skipped.sort();
    assert_eq!(skipped, ["xmp:Label", "xmp:Rating"]);
    assert_eq!(read.dropped, Vec::<String>::new());
    assert_eq!(read.unreadable, Vec::<(String, String)>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Saving over a file replaces it; two saves one after the other to the same path both succeed
/// and the second's content is the file's; no `.saving` folder is left.
#[test]
fn saving_over_a_file_replaces_it() {
    let dir = temp_dir("over");
    let mut other = exiftool_without_config();
    let path = dir.join("mall.xmp");
    write_other(&mut other, &path, &["-XMP-photoshop:Headline=Gammal"]);
    let mut et = exiftool_with_config(&dir);

    let first = template(&[("photoshop:Headline", text("Ny"))], &[]);
    save(&mut et, &path, &first);
    assert_read_as(&load(&mut et, &path), &made(&first));

    let second = template(&[("dc:subject", list(&["hav"]))], &["dc:subject"]);
    save(&mut et, &path, &second);
    assert_read_as(&load(&mut et, &path), &made(&second));
    assert_eq!(entries(&dir), ["mall.xmp", "rawstride-exiftool.config"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A save that fails (ExifTool without the config cannot write `AddTo`) leaves the existing file
/// byte for byte as it was and no `.saving` folder.
#[test]
fn a_failed_save_leaves_the_file_as_it_was() {
    let dir = temp_dir("failed");
    let mut other = exiftool_without_config();
    let path = dir.join("mall.xmp");
    write_other(&mut other, &path, &["-XMP-photoshop:Headline=Gammal"]);
    let before = std::fs::read(&path).unwrap();

    let result = other.write_template_file(
        &path,
        &template(&[("dc:subject", list(&["hav"]))], &["dc:subject"]),
    );
    assert!(
        matches!(result, Err(TemplateFileError::ExifTool(_))),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(entries(&dir), ["mall.xmp"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A name that does not end in `.xmp` is the error and makes no file; `.XMP` is an `.xmp`.
#[test]
fn a_name_not_ending_in_xmp_is_the_error() {
    let dir = temp_dir("name");
    let mut et = exiftool_with_config(&dir);
    let headline = template(&[("photoshop:Headline", text("Hamn"))], &[]);
    let txt = dir.join("mall.txt");
    assert_eq!(
        et.write_template_file(&txt, &headline),
        Err(TemplateFileError::NotXmpName(txt.clone()))
    );
    save(&mut et, &dir.join("STOR.XMP"), &headline);
    assert_eq!(entries(&dir), ["STOR.XMP", "rawstride-exiftool.config"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `write_exiftool_config` makes the folder and the file, leaves a file with the same text alone
/// (its modification time unchanged) and writes it again when it differs.
#[test]
fn the_config_file_is_written_only_when_needed() {
    let dir = temp_dir("config");
    let folder = dir.join("settings").join("exiftool");
    let path = write_exiftool_config(&folder).unwrap();
    assert_eq!(path, folder.join("rawstride-exiftool.config"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), EXIFTOOL_CONFIG);

    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();
    assert_eq!(write_exiftool_config(&folder).unwrap(), path);
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), old);

    std::fs::write(&path, "# changed\n").unwrap();
    write_exiftool_config(&folder).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), EXIFTOOL_CONFIG);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A `%` in the template's name is saved as it is (ExifTool's `-o` never sees the name); a folder
/// whose path holds a `%` is the error, before any folder or file is made.
#[test]
fn a_percent_sign_in_the_name_is_saved_and_in_the_folder_is_the_error() {
    let dir = temp_dir("percent");
    let mut et = exiftool_with_config(&dir);
    let headline = template(&[("photoshop:Headline", text("Hamn"))], &[]);
    let path = dir.join("50%fun %d.xmp");
    save(&mut et, &path, &headline);
    assert_read_as(&load(&mut et, &path), &made(&headline));
    assert_eq!(
        entries(&dir),
        ["50%fun %d.xmp", "rawstride-exiftool.config"]
    );

    let folder = dir.join("Foto%d");
    std::fs::create_dir_all(&folder).unwrap();
    let in_folder = folder.join("mall.xmp");
    assert_eq!(
        et.write_template_file(&in_folder, &headline),
        Err(TemplateFileError::PercentInFolder(folder.clone()))
    );
    assert_eq!(entries(&folder), Vec::<String>::new());
    assert_eq!(
        entries(&dir),
        ["50%fun %d.xmp", "Foto%d", "rawstride-exiftool.config"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Another program's file with four Image Creators: over the limit of three, named in `dropped`
/// (not `unreadable`); with one of the four without content, the three others are kept.
#[test]
fn image_creators_over_the_limit_are_dropped_after_the_blank_ones() {
    let dir = temp_dir("too-many");
    let mut other = exiftool_without_config();
    let four = dir.join("four.xmp");
    write_other(
        &mut other,
        &four,
        &[
            "-XMP-plus:ImageCreator=[{ImageCreatorName=A},{ImageCreatorName=B},{ImageCreatorName=C},{ImageCreatorName=D}]",
            "-XMP-photoshop:Headline=Hamn",
        ],
    );
    let blank = dir.join("blank.xmp");
    write_other(
        &mut other,
        &blank,
        &[
            "-XMP-plus:ImageCreator=[{ImageCreatorName=A},{ImageCreatorID= },{ImageCreatorName=B},{ImageCreatorName=C}]",
        ],
    );
    let mut et = exiftool_with_config(&dir);

    let read = load(&mut et, &four);
    assert_eq!(
        read.template,
        template(&[("photoshop:Headline", text("Hamn"))], &[])
    );
    assert_eq!(read.dropped, ["plus:ImageCreator"]);
    assert_eq!(read.unreadable, Vec::<(String, String)>::new());

    let read = load(&mut et, &blank);
    let creators = |names: &[&str]| {
        Value::StructList(
            names
                .iter()
                .map(|name| entry(&[("plus:ImageCreatorName", text(name))]))
                .collect(),
        )
    };
    assert_eq!(
        read.template,
        template(&[("plus:ImageCreator", creators(&["A", "B", "C"]))], &[])
    );
    assert_eq!(read.dropped, Vec::<String>::new());
    assert_eq!(read.unreadable, Vec::<(String, String)>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file that does not exist is the error.
#[test]
fn a_missing_file_is_the_error() {
    let dir = temp_dir("missing");
    let mut et = exiftool_with_config(&dir);
    let path = dir.join("none.xmp");
    assert_eq!(
        et.read_template_file(&path),
        Err(TemplateFileError::NotFound(path.clone()))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
