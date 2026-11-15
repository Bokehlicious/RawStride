//! The histogram and the exposure statistics of the unpacked RAW data (ADR 0017 §2.6, U7; T142)
//! on the real test RAWs. Run with
//! `$env:PW_TESTDATA = "<folder>"; cargo test -p pw-libraw -- --ignored`.
//!
//! - Every test file gives statistics: 256 bins per channel that add up to the channel's pixels,
//!   the three channels of a mosaic add up to the image's pixel count, and every share is between
//!   0 and 100 %.
//! - A render before the statistics does not change them (they are read from the unpacked data).
//! - `TN006067.ARW` (a bird against a blown-out sky) has clipped pixels, `TN100426.ARW` (a pylon
//!   in front of a dark forest) has deep shadows, and `TN006331.ARW` has neither.
//! - A file cut short inside its RAW data gives an error, not a panic and not statistics.

use std::path::{Path, PathBuf};

use pw_libraw::{DAMAGED, Demosaic, RawStatistics, Rect, Session};

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

fn raws(dir: &Path) -> Vec<PathBuf> {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "could not read PW_TESTDATA folder {}: {error}",
            dir.display()
        )
    });
    let mut v: Vec<PathBuf> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!(
                        "could not read an entry in PW_TESTDATA folder {}: {error}",
                        dir.display()
                    )
                })
                .path()
        })
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    matches!(e.to_ascii_lowercase().as_str(), "arw" | "rw2" | "dng")
                })
        })
        .collect();
    v.sort();
    v
}

/// The test file `name`; it must exist.
fn named(name: &str) -> PathBuf {
    let path = testdata().join(name);
    assert!(
        path.is_file(),
        "the test file {} was not found",
        path.display()
    );
    path
}

fn statistics_of(path: &Path) -> RawStatistics {
    let mut session = Session::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    match session.statistics() {
        Ok(Some(stats)) => stats,
        other => panic!("{}: no statistics: {other:?}", path.display()),
    }
}

/// What must hold for the statistics of any Bayer file of `width` × `height` pixels.
fn check(path: &Path, width: u32, height: u32, stats: &RawStatistics) {
    let name = path.display();
    let all = u64::from(width) * u64::from(height);
    for c in 0..3 {
        assert_eq!(stats.histogram[c].len(), 256);
        let sum: u64 = stats.histogram[c].iter().sum();
        assert_eq!(sum, stats.pixels[c], "{name}: bins of channel {c}");
        assert!(stats.pixels[c] > 0, "{name}: channel {c} has no pixels");
        assert!(stats.clipped[c] <= stats.pixels[c], "{name}: channel {c}");
        let share = stats.clipped_share(c);
        assert!((0.0..=1.0).contains(&share), "{name}: clipped {share}");
        // a pixel at the clipping level is in the top bin
        assert!(
            stats.clipped[c] <= stats.histogram[c][255],
            "{name}: channel {c}: {} clipped, {} in the top bin",
            stats.clipped[c],
            stats.histogram[c][255]
        );
    }
    // a mosaic: every photosite is in exactly one channel, and half of them are green
    assert_eq!(stats.pixels.iter().sum::<u64>(), all, "{name}");
    assert_eq!(stats.pixels[1], stats.pixels[0] + stats.pixels[2], "{name}");
    // the deep shadows are judged in whole 2×2 cells
    let cells = u64::from(width / 2) * u64::from(height / 2);
    assert_eq!(stats.shadow_pixels, cells * 4, "{name}");
    assert!(stats.deep_shadows <= stats.shadow_pixels, "{name}");
    assert_eq!(stats.deep_shadows % 4, 0, "{name}");
    let share = stats.deep_shadow_share();
    assert!((0.0..=1.0).contains(&share), "{name}: deep shadows {share}");
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn every_test_file_has_statistics_that_add_up() {
    let dir = testdata();
    let files = raws(&dir);
    assert!(
        !files.is_empty(),
        "no RAW test files were found in PW_TESTDATA folder {}",
        dir.display()
    );
    for f in &files {
        let mut session = Session::open(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let stats = match session.statistics() {
            Ok(Some(stats)) => stats,
            other => panic!("{}: no statistics: {other:?}", f.display()),
        };
        eprintln!(
            "{:<28} {}x{} clipped R {:.4} % G {:.4} % B {:.4} %  deep shadows {:.4} %",
            f.file_name().unwrap().to_string_lossy(),
            session.width(),
            session.height(),
            stats.clipped_share(0) * 100.0,
            stats.clipped_share(1) * 100.0,
            stats.clipped_share(2) * 100.0,
            stats.deep_shadow_share() * 100.0
        );
        check(f, session.width(), session.height(), &stats);
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn the_statistics_are_the_same_before_and_after_renders() {
    for prefix in ["TN006331", "P1240231", "IMG_20261026_062851"] {
        let dir = testdata();
        let file = raws(&dir)
            .into_iter()
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("no test file {prefix}* in {}", dir.display()));
        let mut session = Session::open(&file).unwrap();
        let before = session.statistics().unwrap().unwrap();
        session.render_half().unwrap();
        let after_half = session.statistics().unwrap().unwrap();
        assert_eq!(after_half, before, "{}: half-size render", file.display());
        session
            .render_region_with(Rect::new(64, 64, 256, 128), 16, Demosaic::Ppg)
            .unwrap();
        let after_region = session.statistics().unwrap().unwrap();
        assert_eq!(after_region, before, "{}: region render", file.display());
        // and a session that rendered first gives the same as one that did not
        let mut rendered_first = Session::open(&file).unwrap();
        rendered_first.render_half().unwrap();
        assert_eq!(
            rendered_first.statistics().unwrap().unwrap(),
            before,
            "{}: rendered before the first statistics",
            file.display()
        );
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn an_overexposed_and_a_dark_scene_show_in_the_shares() {
    // A bird against a blown-out sky: the green channel clips first (before white balance).
    let bright = statistics_of(&named("TN006067.ARW"));
    assert!(
        bright.clipped_share(1) > 0.02,
        "TN006067.ARW: G clipped {}",
        bright.clipped_share(1)
    );
    assert!(bright.deep_shadow_share() < 0.01);
    // A pylon in front of a dark forest.
    let dark = statistics_of(&named("TN100426.ARW"));
    assert!(
        dark.deep_shadow_share() > 0.05,
        "TN100426.ARW: deep shadows {}",
        dark.deep_shadow_share()
    );
    for c in 0..3 {
        assert!(dark.clipped_share(c) < 0.001, "TN100426.ARW: channel {c}");
    }
    // An ordinary exposure has neither.
    let plain = statistics_of(&named("TN006331.ARW"));
    for c in 0..3 {
        assert_eq!(plain.clipped[c], 0, "TN006331.ARW: channel {c}");
    }
    assert!(plain.deep_shadow_share() < 0.001);
}

/// A scratch folder for one test (the tests run in parallel).
fn scratch(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "pw-libraw-statistics-{test}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_file_cut_short_gives_an_error_and_no_statistics() {
    let tmp = scratch("cut");
    for (name, part) in [("TN006331.ARW", 0.4), ("P1240231.RW2", 0.9)] {
        let src = named(name);
        let data = std::fs::read(&src).unwrap();
        let dst = tmp.join(name);
        std::fs::write(&dst, &data[..(data.len() as f64 * part) as usize]).unwrap();
        let result = Session::open(&dst).and_then(|mut session| session.statistics());
        let message = match result {
            Ok(stats) => panic!("{}: statistics of a damaged file: {stats:?}", dst.display()),
            Err(e) => e.to_string(),
        };
        assert!(message.contains(DAMAGED), "{}: {message}", dst.display());
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_file_that_is_no_raw_gives_an_error_and_no_statistics() {
    let tmp = scratch("garbage");
    let mut x: u64 = 7;
    let junk: Vec<u8> = (0..256 * 1024 + 5)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    let f = tmp.join("garbage.ARW");
    std::fs::write(&f, junk).unwrap();
    let result = Session::open(&f).and_then(|mut session| session.statistics());
    assert!(result.is_err(), "{result:?}");
    let _ = std::fs::remove_dir_all(&tmp);
}
