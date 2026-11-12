//! Integration tests of the safe write transaction (ADR 0007 I8c, T86) against a real ExifTool:
//! `exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! The files are the small JPEG of `pw-thumbcache`'s fixture and a fake RAW (`TN0001.ARW` with a
//! few bytes, as in `tests/read.rs`), each in its own temporary folder, with a temporary journal
//! folder. These tests need ExifTool and fail with an explanation when it is missing, instead of
//! returning early and being counted as passed (docs/TESTING.md §4.5). Values are written through
//! `-stay_open` (UTF-8), never with a command line, whose arguments Windows converts to the code
//! page. The transaction uses Windows file operations (`ReplaceFileW`, `MoveFileExW`, locks), so
//! the file is built on Windows only.
//!
//! The real-DNG test is ignored by default. Run it with
//! `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`.
#![cfg(windows)]

use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pw_metadata::ExifTool;
use pw_metadata::iim::{CodedCharset, Fallback};
use pw_metadata::journal::Journal;
use pw_metadata::read::MetadataRead;
use pw_metadata::reconcile::{FileKind, IimDigest};
use pw_metadata::safe_write::{
    ImageCheck, NoImageCheck, OriginalState, SafeWriteError, Stage, WriteReport,
};
use pw_metadata::value::{LangAlt, Metadata, Value};
use pw_metadata::write_plan::{ExifPresent, FileState, WritePlan, WriteSettings, plan};

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

/// A minimal uncompressed little-endian TIFF built here (T92): the header and one IFD with a
/// 1 × 1 grey strip, enough for ExifTool to write it.
fn tiny_tiff() -> Vec<u8> {
    tiny_tiff_with(None)
}

/// [`tiny_tiff`], with `photoshop` (a Photoshop block) as IFD0 0x8649 when it is given.
fn tiny_tiff_with(photoshop: Option<&[u8]>) -> Vec<u8> {
    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    const UNDEFINED: u16 = 7;
    let count: u32 = if photoshop.is_some() { 10 } else { 9 };
    // The header (8 bytes), the IFD (entry count, the entries, next-IFD offset), the strip (one
    // byte and one byte of padding), then the Photoshop block.
    let strip: u32 = 8 + 2 + count * 12 + 4;
    let block = strip + 2;
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, SHORT, 1, 1),    // ImageWidth
        (257, SHORT, 1, 1),    // ImageLength
        (258, SHORT, 1, 8),    // BitsPerSample
        (259, SHORT, 1, 1),    // Compression: none
        (262, SHORT, 1, 1),    // PhotometricInterpretation: BlackIsZero
        (273, LONG, 1, strip), // StripOffsets
        (277, SHORT, 1, 1),    // SamplesPerPixel
        (278, SHORT, 1, 1),    // RowsPerStrip
        (279, LONG, 1, 1),     // StripByteCounts
    ];
    if let Some(photoshop) = photoshop {
        let length = u32::try_from(photoshop.len()).unwrap();
        entries.push((0x8649, UNDEFINED, length, block)); // PhotoshopSettings
    }
    let mut tiff = b"II*\0".to_vec();
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
    for (tag, kind, count, value) in entries {
        tiff.extend_from_slice(&tag.to_le_bytes());
        tiff.extend_from_slice(&kind.to_le_bytes());
        tiff.extend_from_slice(&count.to_le_bytes());
        if kind == SHORT {
            tiff.extend_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
            tiff.extend_from_slice(&[0, 0]);
        } else {
            tiff.extend_from_slice(&value.to_le_bytes());
        }
    }
    tiff.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(tiff.len(), strip as usize);
    tiff.extend_from_slice(&[0x80, 0]);
    if let Some(photoshop) = photoshop {
        tiff.extend_from_slice(photoshop);
    }
    tiff
}

/// One Photoshop resource: signature, id, an empty name and the data, padded to an even length.
fn photoshop_resource(signature: &[u8; 4], id: u16, data: &[u8]) -> Vec<u8> {
    let mut resource = signature.to_vec();
    resource.extend_from_slice(&id.to_be_bytes());
    resource.extend_from_slice(&[0, 0]);
    resource.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
    resource.extend_from_slice(data);
    if data.len() % 2 == 1 {
        resource.push(0);
    }
    resource
}

/// The Photoshop block that makes ExifTool 13.59 misread (T92, measured): an `IPTCDigest`
/// (`8BIM` 0x0425) followed by an `AgHg` resource 0x1f40 of 4 bytes, as at the end of the block
/// of `TN002683.tif`. After a process has written `-IPTCDigest=new` to such a TIFF, it reads the
/// block of every such TIFF with two more warnings ("Non-standard Photoshop at
/// TIFF-IFD0-Photoshop-IPTCDigest", "Bad Photoshop IRB resource").
fn ag_hg_block() -> Vec<u8> {
    let digest: Vec<u8> = (0u8..16).collect();
    let mut block = photoshop_resource(b"8BIM", 0x0425, &digest);
    block.extend(photoshop_resource(b"AgHg", 0x1f40, &[0, 0, 0, 1]));
    block
}

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
        "pw-metadata-safe-write-{name}-{}",
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

    /// A minimal TIFF in the folder (see [`tiny_tiff`]).
    fn tiff(&self) -> PathBuf {
        let file = self.dir.join("bild ä.tif");
        std::fs::write(&file, tiny_tiff()).unwrap();
        file
    }

    /// The `pwtmp`/`pwbak` files left in the folder.
    fn leftovers(&self) -> Vec<String> {
        std::fs::read_dir(&self.dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".pwtmp-") || name.contains(".pwbak-"))
            .collect()
    }

    /// The entries left in the journal folder.
    fn journal_entries(&self) -> Vec<PathBuf> {
        std::fs::read_dir(&self.journal_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }

    /// No `pwtmp`/`pwbak` file and an empty journal.
    fn assert_clean(&self) {
        assert_eq!(self.leftovers(), Vec::<String>::new(), "files left over");
        assert_eq!(
            self.journal_entries(),
            Vec::<PathBuf>::new(),
            "journal entries left over"
        );
    }

    fn remove(self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.journal_dir);
    }
}

/// Writes `tags` to `file` in place (created if it is an `.xmp` that does not exist yet), to set
/// up a test.
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

/// The value of a property.
fn value(read: &MetadataRead, id: &str) -> Option<Value> {
    read.reconciled
        .metadata
        .get(id)
        .map(|stored| stored.value.clone())
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
    }
}

/// The plan that writes City for a file of `kind`.
fn city_plan(kind: FileKind, city: &str) -> WritePlan {
    let mut metadata = Metadata::new();
    metadata.set("photoshop:City", text(city)).unwrap();
    plan(&metadata, &bare_state(kind), WriteSettings::default()).unwrap()
}

/// A plan of hand-written arguments.
fn args_plan(args: &[&str]) -> WritePlan {
    WritePlan {
        args: args.iter().map(|arg| arg.to_string()).collect(),
        ..WritePlan::default()
    }
}

fn written(target: &Path) -> Result<WriteReport, SafeWriteError> {
    Ok(WriteReport::Written {
        target: target.to_path_buf(),
    })
}

/// Fails the test unless `result` is an error of `stage` with the original intact.
fn assert_failed(result: Result<WriteReport, SafeWriteError>, stage: Stage) -> SafeWriteError {
    match result {
        Err(error) => {
            assert_eq!(error.stage, stage, "{error}");
            assert_eq!(error.original, OriginalState::Intact, "{error}");
            error
        }
        Ok(report) => panic!("expected a failure in {stage:?}, got {report:?}"),
    }
}

/// An [`ImageCheck`] whose check of the copy fails.
struct FailingCheck;

impl ImageCheck for FailingCheck {
    fn release(&self, _image: &Path) {}

    fn check_copy(&self, _original: &Path, _copy: &Path, _kind: FileKind) -> Result<(), String> {
        Err("the previews of the copy differ".to_string())
    }
}

/// An [`ImageCheck`] that opens the original without sharing in its check of the copy and keeps
/// it open, so the replace finds the original locked.
#[derive(Default)]
struct LockingCheck {
    held: Mutex<Option<std::fs::File>>,
}

impl ImageCheck for LockingCheck {
    fn release(&self, _image: &Path) {}

    fn check_copy(&self, original: &Path, _copy: &Path, _kind: FileKind) -> Result<(), String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(original)
            .map_err(|e| e.to_string())?;
        *self.held.lock().unwrap() = Some(file);
        Ok(())
    }
}

/// A JPEG written with City, a two-line Description, keywords and IIM created: the values are
/// read back, the modification time is the original's, and no `pwtmp`/`pwbak` file and no journal
/// entry is left.
#[test]
fn a_jpeg_is_written_safely_and_its_values_read_back() {
    let mut et = exiftool();
    let s = Scratch::new("jpeg");
    let file = s.jpeg();
    let modified = std::fs::metadata(&file).unwrap().modified().unwrap();

    let mut metadata = Metadata::new();
    metadata.set("photoshop:City", text("Malmö")).unwrap();
    metadata
        .set("dc:description", lang_alt(&[("x-default", "Rad 1\nRad 2")]))
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
    assert!(plan.options.iter().any(|o| o == "-ec"), "{plan:?}");
    assert!(
        plan.args
            .iter()
            .any(|arg| arg == "-IPTC:CodedCharacterSet=UTF8"),
        "IIM is created: {plan:?}"
    );

    assert_eq!(
        et.write_safely(&file, &plan, &NoImageCheck, &s.journal),
        written(&file)
    );

    let read = read_file(&mut et, &file);
    assert_eq!(value(&read, "photoshop:City"), Some(text("Malmö")));
    assert_eq!(
        value(&read, "dc:description"),
        Some(lang_alt(&[("x-default", "Rad 1\nRad 2")]))
    );
    assert_eq!(
        value(&read, "dc:subject"),
        Some(list(&["fjäll", "sjö, strand"]))
    );
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        modified
    );
    s.assert_clean();
    s.remove();
}

/// An empty plan writes nothing.
#[test]
fn an_empty_plan_leaves_the_file_unchanged() {
    let mut et = exiftool();
    let s = Scratch::new("empty");
    let file = s.jpeg();

    assert_eq!(
        et.write_safely(&file, &WritePlan::default(), &NoImageCheck, &s.journal),
        Ok(WriteReport::Unchanged)
    );
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// A RAW without a sidecar: the sidecar is created with only the plan's values (no `XMP-exif`
/// keys) and the RAW is not changed.
#[test]
fn a_raw_without_a_sidecar_gets_a_sidecar_with_only_the_plans_values() {
    let mut et = exiftool();
    let s = Scratch::new("raw-new");
    let raw = s.dir.join("TN0001.ARW");
    std::fs::write(&raw, FAKE_RAW).unwrap();
    let sidecar = s.dir.join("TN0001.xmp");

    assert_eq!(
        et.write_safely(
            &raw,
            &city_plan(FileKind::Raw, "Malmö"),
            &NoImageCheck,
            &s.journal
        ),
        written(&sidecar)
    );

    let snapshot = et.snapshot(&sidecar).unwrap();
    assert_eq!(snapshot.file_type, "XMP");
    let xmp_keys: Vec<&str> = snapshot
        .listing
        .keys()
        .map(String::as_str)
        .filter(|key| key.starts_with("XMP"))
        .collect();
    assert_eq!(
        xmp_keys,
        ["XMP-photoshop:City", "XMP-x:XMPToolkit"],
        "{snapshot:?}"
    );
    assert!(
        !snapshot
            .listing
            .keys()
            .any(|key| key.starts_with("XMP-exif")),
        "{snapshot:?}"
    );
    assert_eq!(
        value(&read_file(&mut et, &raw), "photoshop:City"),
        Some(text("Malmö"))
    );
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// A RAW with a sidecar: the sidecar is updated and keeps its other values; the RAW is not
/// changed.
#[test]
fn a_raw_with_a_sidecar_has_the_sidecar_updated_and_its_other_values_kept() {
    let mut et = exiftool();
    let s = Scratch::new("raw-existing");
    let raw = s.dir.join("TN0001.ARW");
    std::fs::write(&raw, FAKE_RAW).unwrap();
    let sidecar = s.dir.join("TN0001.xmp");
    write(
        &mut et,
        &sidecar,
        &["-XMP-photoshop:City=Göteborg", "-XMP-xmp:Rating=4"],
    );

    assert_eq!(
        et.write_safely(
            &raw,
            &city_plan(FileKind::Raw, "Malmö"),
            &NoImageCheck,
            &s.journal
        ),
        written(&sidecar)
    );

    let read = read_file(&mut et, &raw);
    assert_eq!(value(&read, "photoshop:City"), Some(text("Malmö")));
    assert_eq!(
        value(&read, "xmp:Rating"),
        Some(Value::Number("4".to_string()))
    );
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// The image check is never called for a sidecar (reviewer's fix): a failing check does not stop
/// the update of an existing sidecar.
#[test]
fn the_image_check_is_not_called_for_an_existing_sidecar() {
    let mut et = exiftool();
    let s = Scratch::new("raw-check");
    let raw = s.dir.join("TN0001.ARW");
    std::fs::write(&raw, FAKE_RAW).unwrap();
    let sidecar = s.dir.join("TN0001.xmp");
    write(&mut et, &sidecar, &["-XMP-photoshop:City=Göteborg"]);

    assert_eq!(
        et.write_safely(
            &raw,
            &city_plan(FileKind::Raw, "Malmö"),
            &FailingCheck,
            &s.journal
        ),
        written(&sidecar)
    );
    assert_eq!(
        value(&read_file(&mut et, &raw), "photoshop:City"),
        Some(text("Malmö"))
    );
    s.assert_clean();
    s.remove();
}

/// A tag ExifTool does not know: ExifTool copies the file with a warning (measured), so the copy
/// is refused in `WriteCopy`.
#[test]
fn an_argument_exiftool_refuses_fails_in_write_copy() {
    let mut et = exiftool();
    let s = Scratch::new("refused");
    let file = s.jpeg();

    let result = et.write_safely(
        &file,
        &args_plan(&["-XMP-photoshop:NoSuchTag=x"]),
        &NoImageCheck,
        &s.journal,
    );

    assert_failed(result, Stage::WriteCopy);
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// `-XMP:all=` removes `XMP-photoshop:City`, which the plan does not name: `Verify`.
#[test]
fn removing_all_xmp_fails_in_verify() {
    let mut et = exiftool();
    let s = Scratch::new("verify");
    let file = s.jpeg();
    write(&mut et, &file, &["-XMP-photoshop:City=Lund"]);
    let bytes = std::fs::read(&file).unwrap();

    let result = et.write_safely(&file, &args_plan(&["-XMP:all="]), &NoImageCheck, &s.journal);

    let error = assert_failed(result, Stage::Verify);
    assert!(error.reason.contains("XMP-photoshop:City"), "{error}");
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// An image check that fails: `ImageCheck`, with its reason.
#[test]
fn a_failing_image_check_fails_in_image_check() {
    let mut et = exiftool();
    let s = Scratch::new("image-check");
    let file = s.jpeg();

    let result = et.write_safely(
        &file,
        &city_plan(FileKind::Jpeg, "Malmö"),
        &FailingCheck,
        &s.journal,
    );

    let error = assert_failed(result, Stage::ImageCheck);
    assert_eq!(error.reason, "the previews of the copy differ");
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// The original locked by another program after the checks (here by the image check): the replace
/// fails with the original intact.
#[test]
fn an_original_locked_before_the_replace_fails_in_replace_intact() {
    let mut et = exiftool();
    let s = Scratch::new("replace");
    let file = s.jpeg();
    let check = LockingCheck::default();

    let result = et.write_safely(
        &file,
        &city_plan(FileKind::Jpeg, "Malmö"),
        &check,
        &s.journal,
    );
    drop(check.held.lock().unwrap().take());

    assert_failed(result, Stage::Replace);
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// The original held open without sharing before the call: `Precheck`.
#[test]
fn an_original_locked_before_the_call_fails_in_precheck() {
    let mut et = exiftool();
    let s = Scratch::new("locked");
    let file = s.jpeg();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&file)
        .unwrap();

    let result = et.write_safely(
        &file,
        &city_plan(FileKind::Jpeg, "Malmö"),
        &NoImageCheck,
        &s.journal,
    );
    drop(held);

    assert_failed(result, Stage::Precheck);
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// A read-only original: `Precheck`.
#[test]
fn a_read_only_original_fails_in_precheck() {
    let mut et = exiftool();
    let s = Scratch::new("read-only");
    let file = s.jpeg();
    let writable = std::fs::metadata(&file).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&file, read_only).unwrap();

    let result = et.write_safely(
        &file,
        &city_plan(FileKind::Jpeg, "Malmö"),
        &NoImageCheck,
        &s.journal,
    );
    std::fs::set_permissions(&file, writable).unwrap();

    assert_failed(result, Stage::Precheck);
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// The stored IIM checksum (`Photoshop:IPTCDigest`) of `file`, or why it is not current: it must
/// exist and equal `File:CurrentIPTCDigest` (T92).
fn digest_state(et: &mut ExifTool, file: &Path) -> Result<String, String> {
    let snapshot = et.snapshot(file).map_err(|e| e.to_string())?;
    let stored = snapshot.listing.get("Photoshop:IPTCDigest");
    let current = snapshot.listing.get("File:CurrentIPTCDigest");
    match (stored, current) {
        (Some(stored), Some(current)) if stored == current => Ok(stored.clone()),
        _ => Err(format!(
            "IPTCDigest {stored:?} is not CurrentIPTCDigest {current:?}"
        )),
    }
}

fn assert_digest_current(et: &mut ExifTool, file: &Path) {
    if let Err(e) = digest_state(et, file) {
        panic!("{}: {e}", file.display());
    }
}

/// The plan that sets City in `read`'s metadata, for the file as read (T87's `state`).
fn read_city_plan(read: &MetadataRead, city: &str, settings: WriteSettings) -> WritePlan {
    let mut metadata = read.reconciled.metadata.clone();
    metadata.set("photoshop:City", text(city)).unwrap();
    plan(&metadata, &read.state, settings).unwrap()
}

/// A TIFF with IIM and a current checksum (made with two ExifTool calls): a City write leaves a
/// current checksum, which the reader reports as `Matches` (T92: in a TIFF, `-IPTCDigest=new` in
/// the same call as the IIM write leaves it out of date, measured with ExifTool 13.59).
#[test]
fn a_tiff_with_iim_keeps_a_current_digest() {
    let mut et = exiftool();
    let s = Scratch::new("tiff-iim");
    let file = s.tiff();
    write(
        &mut et,
        &file,
        &[
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:City=Lund",
            "-XMP-photoshop:City=Lund",
            "-IPTCDigest=new",
        ],
    );
    write(&mut et, &file, &["-IPTCDigest=new"]);
    assert_digest_current(&mut et, &file);
    let read = read_file(&mut et, &file);
    assert!(read.state.iim_present, "{:?}", read.state);
    assert_eq!(read.state.digest, IimDigest::Matches);

    let plan = read_city_plan(&read, "Malmö", WriteSettings::default());
    assert!(
        plan.args.iter().any(|arg| arg == "-IPTC:City=Malmö"),
        "{plan:?}"
    );
    assert!(
        plan.args.iter().any(|arg| arg == "-IPTCDigest=new"),
        "{plan:?}"
    );

    assert_eq!(
        et.write_safely(&file, &plan, &NoImageCheck, &s.journal),
        written(&file)
    );

    assert_digest_current(&mut et, &file);
    let read = read_file(&mut et, &file);
    assert_eq!(read.state.digest, IimDigest::Matches);
    assert_eq!(value(&read, "photoshop:City"), Some(text("Malmö")));
    s.assert_clean();
    s.remove();
}

/// A TIFF without IIM and the setting "create IIM": the IIM and the Photoshop block are created,
/// with one more validation warning (D8), and the checksum is current.
#[test]
fn a_tiff_without_iim_gets_iim_and_a_current_digest() {
    let mut et = exiftool();
    let s = Scratch::new("tiff-new-iim");
    let file = s.tiff();
    let read = read_file(&mut et, &file);
    assert!(!read.state.iim_present, "{:?}", read.state);
    assert_eq!(read.state.digest, IimDigest::Missing);
    let before = et.snapshot(&file).unwrap();
    assert!(
        !before.listing.contains_key("Photoshop:IPTCDigest"),
        "{before:?}"
    );

    let plan = read_city_plan(&read, "Malmö", WriteSettings { create_iim: true });
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

    assert_eq!(
        et.write_safely(&file, &plan, &NoImageCheck, &s.journal),
        written(&file)
    );

    assert_digest_current(&mut et, &file);
    let read = read_file(&mut et, &file);
    assert!(read.state.iim_present, "{:?}", read.state);
    assert_eq!(read.state.digest, IimDigest::Matches);
    assert_eq!(value(&read, "photoshop:City"), Some(text("Malmö")));
    s.assert_clean();
    s.remove();
}

/// A TIFF whose Photoshop block holds an `IPTCDigest` and an `AgHg` resource (as `TN002683.tif`,
/// see [`ag_hg_block`]), written with "create IIM": `Written` with a current checksum, and the same
/// `ExifTool` afterwards reads an untouched copy of the file as it did before the write (T92: the
/// safe write restarts ExifTool after a TIFF write of the checksum).
#[test]
fn a_tiff_with_an_aghg_resource_is_written_and_read_correctly_afterwards() {
    let mut et = exiftool();
    let s = Scratch::new("tiff-aghg");
    let bytes = tiny_tiff_with(Some(&ag_hg_block()));
    let file = s.dir.join("bild ä.tif");
    let untouched = s.dir.join("orörd.tif");
    std::fs::write(&file, &bytes).unwrap();
    std::fs::write(&untouched, &bytes).unwrap();
    let before = et.snapshot(&untouched).unwrap();
    assert!(
        before.listing.contains_key("Photoshop:IPTCDigest"),
        "{before:?}"
    );

    let plan = read_city_plan(
        &read_file(&mut et, &file),
        "Malmö",
        WriteSettings { create_iim: true },
    );
    assert!(
        plan.args.iter().any(|arg| arg == "-IPTCDigest=new"),
        "{plan:?}"
    );

    assert_eq!(
        et.write_safely(&file, &plan, &NoImageCheck, &s.journal),
        written(&file)
    );

    assert_digest_current(&mut et, &file);
    assert_eq!(
        value(&read_file(&mut et, &file), "photoshop:City"),
        Some(text("Malmö"))
    );
    let after = et.snapshot(&untouched).unwrap();
    assert_eq!(after.validate, before.validate);
    assert_eq!(
        after.listing.get("Photoshop:IPTCDigest"),
        before.listing.get("Photoshop:IPTCDigest")
    );
    s.assert_clean();
    s.remove();
}

/// Every DNG in `PW_TESTDATA`, copied to a temporary folder, written with a City and Description
/// plan and `NoImageCheck`: `Written`, the image data hash unchanged and the values read back.
/// Prints the result for each file (run with `--nocapture` to see it).
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn every_real_dng_is_written_safely() {
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
    metadata
        .set(
            "dc:description",
            lang_alt(&[("x-default", "Utsikt mot sjön")]),
        )
        .unwrap();
    let plan = plan(
        &metadata,
        &bare_state(FileKind::Dng),
        WriteSettings::default(),
    )
    .unwrap();

    let mut et = exiftool();
    let s = Scratch::new("dng");
    let mut failures = Vec::new();
    for source in &dngs {
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let file = s.dir.join(&name);
        std::fs::copy(source, &file).unwrap();
        let before = et.snapshot(&file).unwrap();

        let result = et.write_safely(&file, &plan, &NoImageCheck, &s.journal);
        let outcome = match &result {
            Ok(WriteReport::Written { target }) if *target == file => {
                let after = et.snapshot(&file).unwrap();
                let read = read_file(&mut et, &file);
                if after.image_data_hash != before.image_data_hash {
                    Err("the image data hash changed".to_string())
                } else if value(&read, "photoshop:City") != Some(text("Malmö")) {
                    Err(format!(
                        "City read back as {:?}",
                        value(&read, "photoshop:City")
                    ))
                } else if value(&read, "dc:description")
                    != Some(lang_alt(&[("x-default", "Utsikt mot sjön")]))
                {
                    Err(format!(
                        "Description read back as {:?}",
                        value(&read, "dc:description")
                    ))
                } else {
                    Ok(())
                }
            }
            other => Err(format!("{other:?}")),
        };
        println!(
            "{name}: {} bytes -> {} bytes; {}",
            before.size,
            std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0),
            match &outcome {
                Ok(()) => "written, hash unchanged, values read back".to_string(),
                Err(e) => e.clone(),
            }
        );
        if let Err(e) = outcome {
            failures.push(format!("{name}: {e}"));
        }
        std::fs::remove_file(&file).unwrap();
    }
    s.assert_clean();
    s.remove();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Every TIFF and DNG in `PW_TESTDATA`, copied to a temporary folder, written with a City plan
/// for the file as read and the setting "create IIM" through `write_safely` (T92): `Written`, the
/// image data hash unchanged, City read back and a current IIM checksum (`Matches`). Prints the
/// result for each file (run with `--nocapture` to see it).
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn every_real_tiff_and_dng_gets_a_current_digest() {
    let Some(testdata) = std::env::var_os("PW_TESTDATA") else {
        panic!("PW_TESTDATA is not set; set it to the real camera test-data folder");
    };
    let testdata = PathBuf::from(testdata);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&testdata)
        .unwrap_or_else(|e| panic!("cannot read PW_TESTDATA {}: {e}", testdata.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|e| {
                    ["dng", "tif", "tiff"]
                        .iter()
                        .any(|ext| e.eq_ignore_ascii_case(ext))
                })
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no TIFF or DNG in PW_TESTDATA {}",
        testdata.display()
    );

    let mut et = exiftool();
    let s = Scratch::new("real-digest");
    let mut failures = Vec::new();
    for source in &files {
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let file = s.dir.join(&name);
        std::fs::copy(source, &file).unwrap();
        let before = et.snapshot(&file).unwrap();
        let plan = read_city_plan(
            &read_file(&mut et, &file),
            "Malmö",
            WriteSettings { create_iim: true },
        );

        let result = et.write_safely(&file, &plan, &NoImageCheck, &s.journal);
        let outcome = match &result {
            Ok(WriteReport::Written { target }) if *target == file => {
                let after = et.snapshot(&file).unwrap();
                let read = read_file(&mut et, &file);
                if after.image_data_hash != before.image_data_hash {
                    Err("the image data hash changed".to_string())
                } else if value(&read, "photoshop:City") != Some(text("Malmö")) {
                    Err(format!(
                        "City read back as {:?}",
                        value(&read, "photoshop:City")
                    ))
                } else if read.state.digest != IimDigest::Matches {
                    Err(format!("the checksum is read as {:?}", read.state.digest))
                } else {
                    digest_state(&mut et, &file).map(|_| ())
                }
            }
            other => Err(format!("{other:?}")),
        };
        println!(
            "{name}: {} bytes, validation {:?} -> {:?}; {}",
            before.size,
            before.validate,
            et.snapshot(&file).ok().and_then(|after| after.validate),
            match &outcome {
                Ok(()) => "written, hash unchanged, City read back, digest current".to_string(),
                Err(e) => e.clone(),
            }
        );
        if let Err(e) = outcome {
            failures.push(format!("{name}: {e}"));
        }
        std::fs::remove_file(&file).unwrap();
    }
    s.assert_clean();
    s.remove();
    assert!(failures.is_empty(), "{failures:#?}");
}
