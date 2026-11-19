//! Integration tests of ratings and labels written through the write plan and the safe write
//! (ADR 0007 I9a, T87) against a real ExifTool: `exiftool` on PATH, or
//! `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! The files are the small JPEG of `pw-thumbcache`'s fixture and a fake RAW (`TN0001.ARW` with a
//! few bytes, as in `tests/read.rs`), each in its own temporary folder, with a temporary journal
//! folder and `NoImageCheck`. These tests need ExifTool and fail with an explanation when it is
//! missing, instead of returning early and being counted as passed (docs/TESTING.md §4.5). The
//! safe write uses Windows file operations, so the file is built on Windows only.
//!
//! The real-file test is ignored by default. Run it with
//! `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pw_metadata::edit::{Edit, EditError, EditOptions};
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Journal;
use pw_metadata::reconcile::IimDigest;
use pw_metadata::safe_write::{NoImageCheck, Stage, WriteReport};
use pw_metadata::value::{ConflictRule, Value};
use pw_metadata::write_plan::WriteNotice;
use pw_metadata::{ExifTool, Rating, WriteError};

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
    let dir = std::env::temp_dir().join(format!("pw-metadata-edit-{name}-{}", std::process::id()));
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

    /// Writes `rating` with the default options and `NoImageCheck`.
    fn rate(
        &self,
        et: &mut ExifTool,
        image: &Path,
        rating: &Rating,
    ) -> Result<WriteReport, EditError> {
        et.write_rating_safely(
            image,
            rating,
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

fn rating(stars: Option<i8>, label: Option<&str>) -> Rating {
    Rating {
        stars,
        label: label.map(str::to_string),
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

/// The rating `read_ratings` finds for `image` (`None` when it finds nothing).
fn read_rating(et: &mut ExifTool, image: &Path) -> Option<Rating> {
    et.read_ratings(&[image.to_path_buf()])
        .unwrap()
        .remove(image)
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

/// 3 stars and Red on a JPEG: written, read back by `read_ratings`, the modification time the time
/// of the write, and no `pwtmp`/`pwbak` file or journal entry left.
#[test]
fn a_rating_and_label_are_written_safely_to_a_jpeg() {
    let mut et = exiftool();
    let s = Scratch::new("jpeg");
    let file = s.jpeg();
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_600_000_000))
        .unwrap();

    let before = SystemTime::now();
    assert_report(
        s.rate(&mut et, &file, &rating(Some(3), Some("Red"))),
        written(&file),
    );

    assert_eq!(
        read_rating(&mut et, &file),
        Some(rating(Some(3), Some("Red")))
    );
    assert!(
        std::fs::metadata(&file).unwrap().modified().unwrap() >= before - Duration::from_secs(2),
        "the modification time is the time of the write"
    );
    s.assert_clean();
    s.remove();
}

/// The rating that is already in the file writes nothing.
#[test]
fn the_same_rating_again_is_unchanged() {
    let mut et = exiftool();
    let s = Scratch::new("same");
    let file = s.jpeg();
    let three_red = rating(Some(3), Some("Red"));
    assert_report(s.rate(&mut et, &file, &three_red), written(&file));
    let bytes = std::fs::read(&file).unwrap();

    assert_report(s.rate(&mut et, &file, &three_red), WriteReport::Unchanged);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    s.assert_clean();
    s.remove();
}

/// An empty rating on a rated JPEG removes the rating and the label.
#[test]
fn an_empty_rating_clears_a_rated_jpeg() {
    let mut et = exiftool();
    let s = Scratch::new("clear");
    let file = s.jpeg();
    assert_report(
        s.rate(&mut et, &file, &rating(Some(3), Some("Red"))),
        written(&file),
    );

    assert_report(s.rate(&mut et, &file, &Rating::default()), written(&file));
    assert_eq!(read_rating(&mut et, &file), None);
    assert_eq!(tag(&mut et, &file, "XMP-xmp:Rating"), "-");
    assert_eq!(tag(&mut et, &file, "XMP-xmp:Label"), "-");
    s.assert_clean();
    s.remove();
}

/// A Rating of 0 written by another program counts as no rating (§8): an empty rating leaves it.
#[test]
fn an_empty_rating_keeps_a_zero_written_by_another_program() {
    let mut et = exiftool();
    let s = Scratch::new("zero");
    let file = s.jpeg();
    write(&mut et, &file, &["-XMP-xmp:Rating=0"]);
    let bytes = std::fs::read(&file).unwrap();

    assert_report(
        s.rate(&mut et, &file, &Rating::default()),
        WriteReport::Unchanged,
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert_eq!(tag(&mut et, &file, "XMP-xmp:Rating"), "0");
    s.assert_clean();
    s.remove();
}

/// Rejected (−1) is written and read back as −1.
#[test]
fn rejected_is_written_and_read_back() {
    let mut et = exiftool();
    let s = Scratch::new("rejected");
    let file = s.jpeg();

    assert_report(
        s.rate(&mut et, &file, &rating(Some(-1), None)),
        written(&file),
    );
    assert_eq!(read_rating(&mut et, &file), Some(rating(Some(-1), None)));
    s.assert_clean();
    s.remove();
}

/// 6 stars are refused with the old text before anything is read or written.
#[test]
fn six_stars_are_unsupported() {
    let mut et = exiftool();
    let s = Scratch::new("six");
    let file = s.jpeg();

    match s.rate(&mut et, &file, &rating(Some(6), None)) {
        Err(EditError::Unsupported(reason)) => assert_eq!(reason, "rating 6 is outside -1..5"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// A JPEG with IIM City and a matching checksum: after a rating the checksum still matches and
/// City is unchanged in IIM and XMP.
#[test]
fn a_rating_keeps_iim_and_its_checksum() {
    let mut et = exiftool();
    let s = Scratch::new("iim");
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
    let before = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(before.state.digest, IimDigest::Matches, "precondition");

    assert_report(
        s.rate(&mut et, &file, &rating(Some(4), None)),
        written(&file),
    );

    let after = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    assert_eq!(after.state.digest, IimDigest::Matches);
    let city = (
        "photoshop:City".to_string(),
        Value::Text("Göteborg".to_string()),
    );
    assert!(
        after.state.iim_values.contains(&city),
        "{:?}",
        after.state.iim_values
    );
    assert_eq!(
        after
            .reconciled
            .metadata
            .get("photoshop:City")
            .map(|stored| stored.value.clone()),
        Some(city.1.clone())
    );
    assert_eq!(tag(&mut et, &file, "XMP-photoshop:City"), "Göteborg");
    assert_eq!(tag(&mut et, &file, "IPTC:City"), "Göteborg");
    assert!(after.reconciled.metadata.conflicts().is_empty());
    assert_eq!(read_rating(&mut et, &file), Some(rating(Some(4), None)));
    s.assert_clean();
    s.remove();
}

/// A RAW without a sidecar: an empty rating makes no sidecar; 2 stars create one holding Rating 2,
/// and the RAW is not changed.
#[test]
fn a_raw_gets_a_sidecar_only_for_a_rating() {
    let mut et = exiftool();
    let s = Scratch::new("raw");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");

    assert_report(
        s.rate(&mut et, &raw, &Rating::default()),
        WriteReport::Unchanged,
    );
    assert!(!sidecar.exists());

    assert_report(
        s.rate(&mut et, &raw, &rating(Some(2), None)),
        written(&sidecar),
    );
    assert_eq!(tag(&mut et, &sidecar, "XMP-xmp:Rating"), "2");
    assert_eq!(read_rating(&mut et, &raw), Some(rating(Some(2), None)));
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// A RAW whose sidecar already exists: a rating writes the sidecar, which gets the time of the
/// write as its modification time (§9.4, Q14); the RAW is not written (§4.2).
#[test]
fn an_existing_sidecar_gets_the_time_of_the_write() {
    let mut et = exiftool();
    let s = Scratch::new("sidecar-time");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");
    write(&mut et, &sidecar, &["-XMP-xmp:Label=Red"]);
    std::fs::File::options()
        .write(true)
        .open(&sidecar)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_600_000_000))
        .unwrap();
    let raw_modified = std::fs::metadata(&raw).unwrap().modified().unwrap();

    let before = SystemTime::now();
    assert_report(
        s.rate(&mut et, &raw, &rating(Some(2), None)),
        written(&sidecar),
    );

    assert_eq!(tag(&mut et, &sidecar, "XMP-xmp:Rating"), "2");
    assert!(
        std::fs::metadata(&sidecar).unwrap().modified().unwrap() >= before - Duration::from_secs(2),
        "the sidecar's modification time is the time of the write"
    );
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    assert_eq!(
        std::fs::metadata(&raw).unwrap().modified().unwrap(),
        raw_modified
    );
    s.assert_clean();
    s.remove();
}

/// The rating fields that follow a rating when they exist (§6.3, Q16).
const RATING_FIELDS: [&str; 3] = ["IFD0:Rating", "IFD0:RatingPercent", "XMP-xmp:RatingPercent"];

/// A JPEG with Exif Rating and RatingPercent and `xmp:RatingPercent` (as another program writes
/// them): 2 stars bring all three to 2 and 25; a reject then removes them and writes −1 (§6.3,
/// Q16).
#[test]
fn a_rating_brings_existing_percentages_and_exif_rating_along() {
    let mut et = exiftool();
    let s = Scratch::new("percentages");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-IFD0:Rating=5",
            "-IFD0:RatingPercent=99",
            "-XMP-xmp:Rating=5",
            "-XMP-xmp:RatingPercent=99",
        ],
    );

    assert_report(
        s.rate(&mut et, &file, &rating(Some(2), None)),
        written(&file),
    );
    assert_eq!(tag(&mut et, &file, "IFD0:Rating"), "2");
    assert_eq!(tag(&mut et, &file, "IFD0:RatingPercent"), "25");
    assert_eq!(tag(&mut et, &file, "XMP-xmp:RatingPercent"), "25");

    assert_report(
        s.rate(&mut et, &file, &rating(Some(-1), None)),
        written(&file),
    );
    for name in RATING_FIELDS {
        assert_eq!(tag(&mut et, &file, name), "-", "{name} is removed");
    }
    assert_eq!(tag(&mut et, &file, "XMP-xmp:Rating"), "-1");
    s.assert_clean();
    s.remove();
}

/// A rating never creates the percentages or Exif Rating (§6.3, Q16).
#[test]
fn a_rating_creates_no_percentage() {
    let mut et = exiftool();
    let s = Scratch::new("no-percentage");
    let file = s.jpeg();

    assert_report(
        s.rate(&mut et, &file, &rating(Some(4), None)),
        written(&file),
    );
    assert_eq!(tag(&mut et, &file, "XMP-xmp:Rating"), "4");
    for name in RATING_FIELDS {
        assert_eq!(tag(&mut et, &file, name), "-", "{name} is not created");
    }
    s.assert_clean();
    s.remove();
}

/// A RAW whose sidecar has `xmp:RatingPercent`: it follows the rating in the sidecar; the RAW is
/// not written (§4.2, §6.3).
#[test]
fn a_raw_sidecar_percentage_follows_the_rating() {
    let mut et = exiftool();
    let s = Scratch::new("raw-percentage");
    let raw = s.raw();
    let sidecar = s.dir.join("TN0001.xmp");
    write(
        &mut et,
        &sidecar,
        &["-XMP-xmp:Rating=3", "-XMP-xmp:RatingPercent=50"],
    );

    assert_report(
        s.rate(&mut et, &raw, &rating(Some(5), None)),
        written(&sidecar),
    );
    assert_eq!(tag(&mut et, &sidecar, "XMP-xmp:Rating"), "5");
    assert_eq!(tag(&mut et, &sidecar, "XMP-xmp:RatingPercent"), "99");
    assert_eq!(std::fs::read(&raw).unwrap(), FAKE_RAW);
    s.assert_clean();
    s.remove();
}

/// A read-only JPEG: the safe write fails in its check before writing; the file is unchanged.
#[test]
fn a_read_only_jpeg_fails_in_precheck() {
    let mut et = exiftool();
    let s = Scratch::new("read-only");
    let file = s.jpeg();
    let writable = std::fs::metadata(&file).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&file, read_only).unwrap();

    let result = s.rate(&mut et, &file, &rating(Some(3), None));
    std::fs::set_permissions(&file, writable).unwrap();

    match result {
        Err(EditError::Write(error)) => assert_eq!(error.stage, Stage::Precheck, "{error}"),
        other => panic!("expected a write error in Precheck, got {other:?}"),
    }
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// A JPEG that another program holds open for reading, sharing reading and writing but not
/// deleting (as Photo Mechanic did in the owner's check of T107, T108): the metadata is still
/// read, the editor's write is refused before writing, the error is `WriteError::Locked`, and the
/// file is unchanged. (Held open for writing as well, ExifTool cannot even read the file.)
#[test]
fn edits_to_a_jpeg_held_open_by_another_program_are_locked() {
    use std::os::windows::fs::OpenOptionsExt;
    /// `FILE_SHARE_READ | FILE_SHARE_WRITE` from `winnt.h`: no `FILE_SHARE_DELETE`.
    const SHARE_READ_WRITE: u32 = 0x1 | 0x2;

    let mut et = exiftool();
    let s = Scratch::new("locked");
    let file = s.jpeg();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ_WRITE)
        .open(&file)
        .unwrap();
    let edits = [Edit::Set {
        property: "photoshop:City".to_string(),
        field: None,
        value: Value::Text("Umeå".to_string()),
    }];

    let result = et.write_edits_safely(
        &file,
        &edits,
        &EditOptions::default(),
        &NoImageCheck,
        &s.journal,
    );
    drop(held);

    match result {
        Err(error) => {
            let EditError::Write(write_error) = &error else {
                panic!("expected a write error, got {error}");
            };
            assert_eq!(write_error.stage, Stage::Precheck, "{write_error}");
            assert_eq!(WriteError::from(error), WriteError::Locked);
        }
        Ok(report) => panic!("expected Locked, the file was written: {report:?}"),
    }
    assert_eq!(std::fs::read(&file).unwrap(), SMALL);
    s.assert_clean();
    s.remove();
}

/// Keywords `sol` and `hav` in a JPEG; an `Entries` edit that adds `vinter` and removes `sol` is
/// applied to the file's own keywords: `hav` is kept and `vinter` appended (ADR 0017 U12d-Q1).
#[test]
fn keywords_are_added_to_a_jpeg_and_its_own_kept() {
    let mut et = exiftool();
    let s = Scratch::new("entries");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &["-XMP-dc:Subject=sol", "-XMP-dc:Subject=hav"],
    );
    assert_eq!(
        tag(&mut et, &file, "XMP-dc:Subject"),
        "sol, hav",
        "precondition"
    );
    let edits = [Edit::Entries {
        property: "dc:subject".to_string(),
        add: vec!["vinter".to_string()],
        remove: vec!["sol".to_string()],
    }];

    let result = et.write_edits_safely(
        &file,
        &edits,
        &EditOptions::default(),
        &NoImageCheck,
        &s.journal,
    );

    match result {
        Ok(report) => assert_eq!(report.report, written(&file)),
        Err(error) => panic!("expected the file to be written, got the error {error}"),
    }
    assert_eq!(tag(&mut et, &file, "XMP-dc:Subject"), "hav, vinter");
    s.assert_clean();
    s.remove();
}

/// XMP keywords `sol`, `hav` and IIM Keywords `sol`, `is` without an IIM checksum: an `Open`
/// conflict on `dc:subject`. An `Entries` edit leaves it alone and the report names it once, both
/// when nothing is written and when another edit is (ADR 0017 U12d-Q6).
#[test]
fn an_open_keyword_conflict_is_left_alone_and_reported() {
    let mut et = exiftool();
    let s = Scratch::new("entries-conflict");
    let file = s.jpeg();
    write(
        &mut et,
        &file,
        &[
            "-XMP-dc:Subject=sol",
            "-XMP-dc:Subject=hav",
            "-IPTC:CodedCharacterSet=UTF8",
            "-IPTC:Keywords=sol",
            "-IPTC:Keywords=is",
        ],
    );
    let before = et.read_metadata(&file, Fallback::Windows1252).unwrap();
    let rules: Vec<_> = before
        .reconciled
        .metadata
        .conflicts()
        .iter()
        .map(|c| (c.property.as_str(), c.rule))
        .collect();
    assert_eq!(rules, [("dc:subject", ConflictRule::Open)], "precondition");
    let add_vinter = Edit::Entries {
        property: "dc:subject".to_string(),
        add: vec!["vinter".to_string()],
        remove: Vec::new(),
    };
    let kept = WriteNotice::ConflictKept {
        property: "dc:subject".to_string(),
    };

    // (a) Only the `Entries`: nothing is written, the conflict is still named.
    let bytes = std::fs::read(&file).unwrap();
    match et.write_edits_safely(
        &file,
        std::slice::from_ref(&add_vinter),
        &EditOptions::default(),
        &NoImageCheck,
        &s.journal,
    ) {
        Ok(report) => {
            assert_eq!(report.report, WriteReport::Unchanged);
            assert_eq!(report.notices, std::slice::from_ref(&kept));
        }
        Err(error) => panic!("expected Unchanged, got the error {error}"),
    }
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert_eq!(tag(&mut et, &file, "XMP-dc:Subject"), "sol, hav");
    assert_eq!(tag(&mut et, &file, "IPTC:Keywords"), "sol, is");

    // (b) With a Headline: written, the conflict named once, both keyword forms unchanged.
    let edits = [
        add_vinter,
        Edit::Set {
            property: "photoshop:Headline".to_string(),
            field: None,
            value: Value::Text("Rubrik".to_string()),
        },
    ];
    match et.write_edits_safely(
        &file,
        &edits,
        &EditOptions::default(),
        &NoImageCheck,
        &s.journal,
    ) {
        Ok(report) => {
            assert_eq!(report.report, written(&file));
            let named = report
                .notices
                .iter()
                .filter(|notice| **notice == kept)
                .count();
            assert_eq!(named, 1, "{:?}", report.notices);
        }
        Err(error) => panic!("expected the file to be written, got the error {error}"),
    }
    assert_eq!(tag(&mut et, &file, "XMP-photoshop:Headline"), "Rubrik");
    assert_eq!(tag(&mut et, &file, "XMP-dc:Subject"), "sol, hav");
    assert_eq!(tag(&mut et, &file, "IPTC:Keywords"), "sol, is");
    s.assert_clean();
    s.remove();
}

/// Every DNG, JPEG and TIFF in `PW_TESTDATA`, copied to a temporary folder: 3 stars and Red are
/// written, the image data hash is unchanged and the rating is read back. Prints the result and
/// the time of each file (run with `--nocapture` to see it).
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn every_real_dng_jpeg_and_tiff_is_rated_safely() {
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
                    ["dng", "jpg", "jpeg", "tif", "tiff"]
                        .iter()
                        .any(|ext| e.eq_ignore_ascii_case(ext))
                })
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no DNG, JPEG or TIFF in PW_TESTDATA {}",
        testdata.display()
    );

    let three_red = rating(Some(3), Some("Red"));
    let mut et = exiftool();
    let s = Scratch::new("real");
    let mut failures = Vec::new();
    for source in &files {
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let file = s.dir.join(&name);
        std::fs::copy(source, &file).unwrap();
        let before = et.snapshot(&file).unwrap();

        let started = Instant::now();
        let result = s.rate(&mut et, &file, &three_red);
        let elapsed = started.elapsed();
        let outcome = match &result {
            Ok(WriteReport::Written { target }) if *target == file => {
                let after = et.snapshot(&file).unwrap();
                if after.image_data_hash != before.image_data_hash {
                    Err("the image data hash changed".to_string())
                } else {
                    match read_rating(&mut et, &file) {
                        Some(read) if read == three_red => Ok(()),
                        other => Err(format!("read back as {other:?}")),
                    }
                }
            }
            other => Err(format!("{other:?}")),
        };
        println!(
            "{name}: {} bytes, {} ms; {}",
            before.size,
            elapsed.as_millis(),
            match &outcome {
                Ok(()) => "written, hash unchanged, rating read back".to_string(),
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
