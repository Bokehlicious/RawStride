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
//! - The clipping map (ADR 0017 U7b, T146) of the three named files: 2×2 cells on the grid of
//!   `Session::width()` × `height()`, only bits 0–3, the deep-shadow cells among the whole cells
//!   are the statistics' `deep_shadows`, the clipped bits agree with the statistics' clipped
//!   pixels, and a render before or after does not change it.
//! - Every Fujifilm X-Trans file (`.RAF`, at least one): statistics that add up and a clipping map
//!   of 3×3 cells that agrees with them; `_DSF4271.RAF` has clipped cells in R and G and none in
//!   B, the other four named files have none. (The every-file test above skips `.RAF`: its checks
//!   assume a Bayer pattern.)

use std::path::{Path, PathBuf};

use pw_libraw::{ClippingMap, DAMAGED, Demosaic, RawStatistics, Rect, Session};

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

/// The files T142's statistics tests name: with clipping, without, and dark.
const MAP_FILES: [&str; 3] = ["TN006067.ARW", "TN006331.ARW", "TN100426.ARW"];

/// The session's size, its statistics and its clipping map.
fn map_of(path: &Path) -> (u32, u32, RawStatistics, ClippingMap) {
    let mut session = Session::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let stats = match session.statistics() {
        Ok(Some(stats)) => stats,
        other => panic!("{}: no statistics: {other:?}", path.display()),
    };
    let map = match session.clipping_map() {
        Ok(Some(map)) => map,
        other => panic!("{}: no clipping map: {other:?}", path.display()),
    };
    (session.width(), session.height(), stats, map)
}

/// The number of cells with `bit`; with `whole_only`, only the cells not cut by the right or
/// bottom edge of an image of `width` × `height` photosites.
fn cells_with(map: &ClippingMap, bit: u8, width: u32, height: u32, whole_only: bool) -> u64 {
    let (cols, rows) = if whole_only {
        (width / map.cell, height / map.cell)
    } else {
        (map.width, map.height)
    };
    let mut n = 0;
    for y in 0..rows {
        for x in 0..cols {
            let cell = map.at(x, y).expect("a cell inside the map");
            if cell & bit != 0 {
                n += 1;
            }
        }
    }
    n
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn the_clipping_map_covers_the_image_and_agrees_with_the_statistics() {
    for name in MAP_FILES {
        let (width, height, stats, map) = map_of(&named(name));
        eprintln!(
            "{name:<14} {width}x{height} (divisible by 2: {}) map {map:?}",
            width % 2 == 0 && height % 2 == 0
        );
        // a Bayer file: 2×2 cells on the render's grid
        assert_eq!(map.cell, 2, "{name}");
        assert!(map.width * map.cell >= width, "{name}");
        assert!(map.width * map.cell < width + map.cell, "{name}");
        assert!(map.height * map.cell >= height, "{name}");
        assert!(map.height * map.cell < height + map.cell, "{name}");
        assert_eq!(
            map.data.len() as u64,
            u64::from(map.width) * u64::from(map.height),
            "{name}"
        );
        // only bits 0–3
        assert!(
            map.data.iter().all(|&b| b & !ClippingMap::USED_BITS == 0),
            "{name}: a bit above 3 is set"
        );
        // the deep shadows: the whole cells with the bit are what the statistics count
        let deep = cells_with(&map, ClippingMap::DEEP_SHADOWS, width, height, true);
        assert_eq!(deep * 4, stats.deep_shadows, "{name}: deep shadows");
        // the clipped bits: a cell has one only if it holds a clipped photosite of that colour,
        // and every clipped photosite is in a cell with the bit
        for c in 0..3 {
            let bit = ClippingMap::CLIPPED[c];
            let cells = cells_with(&map, bit, width, height, false);
            assert!(
                cells <= stats.clipped[c],
                "{name}: channel {c}: {cells} cells, {} clipped photosites",
                stats.clipped[c]
            );
            assert_eq!(
                cells > 0,
                stats.clipped[c] > 0,
                "{name}: channel {c}: {cells} cells, {} clipped photosites",
                stats.clipped[c]
            );
        }
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn the_clipping_map_marks_clipping_and_deep_shadows_where_the_files_have_them() {
    let any_clipped = ClippingMap::CLIPPED_R | ClippingMap::CLIPPED_G | ClippingMap::CLIPPED_B;
    // A bird against a blown-out sky.
    let (w, h, _, bright) = map_of(&named("TN006067.ARW"));
    let clipped = cells_with(&bright, any_clipped, w, h, false);
    assert!(clipped > 0, "TN006067.ARW: no clipped cells");
    assert!(cells_with(&bright, ClippingMap::CLIPPED_G, w, h, false) > 0);
    // An ordinary exposure.
    let (w, h, _, plain) = map_of(&named("TN006331.ARW"));
    assert_eq!(
        cells_with(&plain, any_clipped, w, h, false),
        0,
        "TN006331.ARW: clipped cells"
    );
    // A pylon in front of a dark forest.
    let (w, h, _, dark) = map_of(&named("TN100426.ARW"));
    assert!(
        cells_with(&dark, ClippingMap::DEEP_SHADOWS, w, h, true) > 0,
        "TN100426.ARW: no deep-shadow cells"
    );
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn the_clipping_map_is_the_same_before_and_after_renders() {
    for name in MAP_FILES {
        let file = named(name);
        let mut session = Session::open(&file).unwrap();
        let before = session.clipping_map().unwrap().unwrap();
        session.render_half().unwrap();
        assert_eq!(
            session.clipping_map().unwrap().unwrap(),
            before,
            "{name}: half-size render"
        );
        session
            .render_region_with(Rect::new(64, 64, 256, 128), 16, Demosaic::Ppg)
            .unwrap();
        assert_eq!(
            session.clipping_map().unwrap().unwrap(),
            before,
            "{name}: region render"
        );
        let mut rendered_first = Session::open(&file).unwrap();
        rendered_first.render_half().unwrap();
        assert_eq!(
            rendered_first.clipping_map().unwrap().unwrap(),
            before,
            "{name}: rendered before the first map"
        );
    }
}

/// The Fujifilm files (`.RAF`) in `PW_TESTDATA`; there must be at least one.
fn fujifilm_files() -> Vec<PathBuf> {
    let dir = testdata();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!(
            "could not read PW_TESTDATA folder {}: {error}",
            dir.display()
        )
    });
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("raf"))
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no Fujifilm (.RAF) test files were found in PW_TESTDATA folder {}",
        dir.display()
    );
    files
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn fujifilm_files_have_statistics_and_a_clipping_map_that_agree() {
    // The named files: with clipped highlights in R and G (none in B), and without any.
    let clipped_in_r_and_g = "_DSF4271.RAF";
    let mut not_clipped = vec![
        "_DSF4243.RAF",
        "_DSF4248.RAF",
        "_DSF4252.RAF",
        "_DSF4254.RAF",
    ];
    let mut found_clipped = false;
    for file in fujifilm_files() {
        let name = file.display();
        let file_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let (width, height, stats, map) = map_of(&file);
        eprintln!(
            "{file_name:<16} {width}x{height} (divisible by 3: {}) map {map:?} clipped R {} G {} \
             B {} deep shadows {} of {}",
            width % 3 == 0 && height % 3 == 0,
            stats.clipped[0],
            stats.clipped[1],
            stats.clipped[2],
            stats.deep_shadows,
            stats.shadow_pixels
        );
        // the statistics add up: every photosite of the mosaic in exactly one channel
        let all = u64::from(width) * u64::from(height);
        assert_eq!(stats.pixels.iter().sum::<u64>(), all, "{name}");
        for c in 0..3 {
            let sum: u64 = stats.histogram[c].iter().sum();
            assert_eq!(sum, stats.pixels[c], "{name}: bins of channel {c}");
            assert!(stats.pixels[c] > 0, "{name}: channel {c} has no pixels");
            let share = stats.clipped_share(c);
            assert!((0.0..=1.0).contains(&share), "{name}: clipped {share}");
        }
        // the deep shadows are judged in whole 3×3 cells
        let whole = u64::from(width / 3) * u64::from(height / 3);
        assert_eq!(stats.shadow_pixels, whole * 9, "{name}");
        let share = stats.deep_shadow_share();
        assert!((0.0..=1.0).contains(&share), "{name}: deep shadows {share}");
        // an X-Trans map: 3×3 cells, ceil(width / 3) × ceil(height / 3), only bits 0–3
        assert_eq!(map.cell, 3, "{name}: not judged as X-Trans");
        assert_eq!(map.width, width.div_ceil(3), "{name}");
        assert_eq!(map.height, height.div_ceil(3), "{name}");
        assert_eq!(
            map.data.len() as u64,
            u64::from(map.width) * u64::from(map.height),
            "{name}"
        );
        assert!(
            map.data.iter().all(|&b| b & !ClippingMap::USED_BITS == 0),
            "{name}: a bit above 3 is set"
        );
        // the whole cells with the deep-shadow bit are what the statistics count
        let deep = cells_with(&map, ClippingMap::DEEP_SHADOWS, width, height, true);
        assert_eq!(deep * 9, stats.deep_shadows, "{name}: deep shadows");
        // each channel has clipped cells exactly when it has clipped photosites, and no more
        let mut clipped_cells = [0u64; 3];
        for (c, clipped) in clipped_cells.iter_mut().enumerate() {
            let cells = cells_with(&map, ClippingMap::CLIPPED[c], width, height, false);
            *clipped = cells;
            assert!(
                cells <= stats.clipped[c],
                "{name}: channel {c}: {cells} cells, {} clipped photosites",
                stats.clipped[c]
            );
            assert_eq!(
                cells > 0,
                stats.clipped[c] > 0,
                "{name}: channel {c}: {cells} cells, {} clipped photosites",
                stats.clipped[c]
            );
        }
        if file_name == clipped_in_r_and_g {
            found_clipped = true;
            assert!(clipped_cells[0] > 0, "{name}: no cells clipped in R");
            assert!(clipped_cells[1] > 0, "{name}: no cells clipped in G");
            assert_eq!(clipped_cells[2], 0, "{name}: cells clipped in B");
        } else if let Some(i) = not_clipped.iter().position(|n| *n == file_name) {
            not_clipped.remove(i);
            assert_eq!(clipped_cells, [0, 0, 0], "{name}: clipped cells");
        }
        // the map is the same after a half-size render
        let mut session = Session::open(&file).unwrap();
        session.render_half().unwrap();
        assert_eq!(
            session.clipping_map().unwrap().unwrap(),
            map,
            "{name}: after a half-size render"
        );
    }
    assert!(
        found_clipped,
        "the test file {clipped_in_r_and_g} was not found"
    );
    assert!(
        not_clipped.is_empty(),
        "the test files {not_clipped:?} were not found"
    );
}
