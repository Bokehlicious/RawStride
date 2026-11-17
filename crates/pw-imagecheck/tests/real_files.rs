//! The real image check (ADR 0007 I9d, T90) on every DNG in `PW_TESTDATA`, copied to temporary
//! folders. All tests are ignored by default. Run them with
//! `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`.
//!
//! The write test needs ExifTool (`exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` run
//! through `perl`) and fails with an explanation when it is missing (docs/TESTING.md §4.5). The
//! safe write uses Windows file operations (`ReplaceFileW`), so the file is built on Windows only.
//! The time of each `check_copy` is printed (`-- --nocapture`).
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pw_imagecheck::RawImageCheck;
use pw_libraw::DAMAGED;
use pw_metadata::ExifTool;
use pw_metadata::iim::CodedCharset;
use pw_metadata::journal::Journal;
use pw_metadata::reconcile::{FileKind, IimDigest};
use pw_metadata::safe_write::{ImageCheck, WriteReport};
use pw_metadata::value::{Metadata, Value};
use pw_metadata::write_plan::{ExifPresent, FileState, WriteSettings, plan};

fn testdata() -> PathBuf {
    let Some(dir) = std::env::var_os("PW_TESTDATA") else {
        panic!("PW_TESTDATA is not set; set it to the real camera test-data folder");
    };
    let dir = PathBuf::from(dir);
    assert!(
        dir.is_dir(),
        "PW_TESTDATA folder does not exist: {}",
        dir.display()
    );
    dir
}

/// Every DNG in `PW_TESTDATA`, sorted by name.
fn dngs() -> Vec<PathBuf> {
    let dir = testdata();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read PW_TESTDATA {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no DNG in PW_TESTDATA {}", dir.display());
    files
}

/// A new, empty temporary folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("pw-imagecheck-real-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

/// A copy of `source` in `dir` with only the first `keep` bytes.
fn cut(source: &Path, keep: u64, target: &Path) {
    let data = std::fs::read(source).unwrap();
    std::fs::write(target, &data[..(keep as usize).min(data.len())]).unwrap();
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

/// [`RawImageCheck`] that records how long each `check_copy` took.
struct Timed {
    check: RawImageCheck,
    times: Mutex<Vec<Duration>>,
}

impl ImageCheck for Timed {
    fn release(&self, image: &Path) {
        self.check.release(image);
    }

    fn release_for_move(&self, image: &Path) {
        self.check.release_for_move(image);
    }

    fn check_copy(&self, original: &Path, copy: &Path, kind: FileKind) -> Result<(), String> {
        let start = Instant::now();
        let result = self.check.check_copy(original, copy, kind);
        self.times.lock().unwrap().push(start.elapsed());
        result
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn every_dng_is_written_safely_with_the_real_check() {
    let mut metadata = Metadata::new();
    metadata
        .set("photoshop:City", Value::Text("Malmö".to_string()))
        .unwrap();
    let state = FileState {
        kind: FileKind::Dng,
        iim_present: false,
        iim_charset: CodedCharset::Absent,
        iim_values: Vec::new(),
        digest: IimDigest::Missing,
        exif_present: ExifPresent::default(),
    };
    let plan = plan(&metadata, &state, WriteSettings::default()).unwrap();

    let mut et = exiftool();
    let dir = temp_dir("write");
    let journal_dir = temp_dir("write-journal");
    let journal = Journal::open(&journal_dir).unwrap();
    let mut failures = Vec::new();
    for source in dngs() {
        let file = dir.join(name(&source));
        std::fs::copy(&source, &file).unwrap();
        let check = Timed {
            check: RawImageCheck::without_release(),
            times: Mutex::new(Vec::new()),
        };
        let result = et.write_safely(&file, &plan, &check, &journal);
        let times = check.times.lock().unwrap().clone();
        println!(
            "{}: {:?}; check_copy {}",
            name(&source),
            result,
            times
                .iter()
                .map(|t| format!("{} ms", t.as_millis()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        match result {
            Ok(WriteReport::Written { target }) if target == file && times.len() == 1 => {}
            other => failures.push(format!(
                "{}: {other:?}, check_copy called {} times",
                name(&source),
                times.len()
            )),
        }
        std::fs::remove_file(&file).unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&journal_dir);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_dng_copy_cut_inside_its_raw_data_is_refused_as_damaged() {
    let dir = temp_dir("cut");
    let check = RawImageCheck::without_release();
    let mut failures = Vec::new();
    for source in dngs() {
        // Cut in the middle, inside the RAW data (as `pw-libraw/tests/damaged.rs` cuts a DNG).
        let len = std::fs::metadata(&source).unwrap().len();
        let original = dir.join(name(&source));
        std::fs::copy(&source, &original).unwrap();
        let copy = dir.join(format!("cut-{}", name(&source)));
        cut(&source, len / 2, &copy);

        let start = Instant::now();
        let result = check.check_copy(&original, &copy, FileKind::Dng);
        println!(
            "{} cut to {} bytes: {:?} ({} ms)",
            name(&source),
            len / 2,
            result,
            start.elapsed().as_millis()
        );
        match &result {
            Err(error) if error.starts_with("LibRaw: ") && error.contains(DAMAGED) => {}
            other => failures.push(format!("{}: {other:?}", name(&source))),
        }

        // The cut copy against a cut original: the write did not make it worse.
        let cut_original = dir.join(format!("cut-original-{}", name(&source)));
        cut(&source, len / 2, &cut_original);
        let result = check.check_copy(&cut_original, &copy, FileKind::Dng);
        println!("{} cut against cut: {:?}", name(&source), result);
        if result.is_err() {
            failures.push(format!("{} cut against cut: {result:?}", name(&source)));
        }
        for file in [&original, &copy, &cut_original] {
            std::fs::remove_file(file).unwrap();
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_dng_copy_with_one_byte_changed_in_its_largest_preview_is_refused() {
    let dir = temp_dir("preview");
    let check = RawImageCheck::without_release();
    let mut failures = Vec::new();
    for source in dngs() {
        let original = dir.join(name(&source));
        std::fs::copy(&source, &original).unwrap();
        let info = pw_raw::inspect(&original).unwrap();
        let count = info.previews.len();
        let Some(largest) = info.previews.last() else {
            failures.push(format!("{}: no preview", name(&source)));
            continue;
        };
        let mut bytes = std::fs::read(&original).unwrap();
        // A byte in the middle of the preview's entropy-coded data.
        let at = (largest.offset + largest.length / 2) as usize;
        bytes[at] ^= 0x01;
        let copy = dir.join(format!("changed-{}", name(&source)));
        std::fs::write(&copy, &bytes).unwrap();

        let result = check.check_copy(&original, &copy, FileKind::Dng);
        println!(
            "{}: preview {count} of {count} ({} × {}) changed: {:?}",
            name(&source),
            largest.width,
            largest.height,
            result
        );
        let expected = format!("preview {count} differs");
        if result != Err(expected) {
            failures.push(format!("{}: {result:?}", name(&source)));
        }
        std::fs::remove_file(&original).unwrap();
        std::fs::remove_file(&copy).unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}
