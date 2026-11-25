//! The temporary exposure per LibRaw render and the camera's own brightness (ADR 0027 §3.4,
//! W5-Q2 to W5-Q5; T331) on the real test RAWs. Run with
//! `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`; the timing wants
//! `--release` and `--nocapture` (`timing_of_exposure`).
//!
//! - A render at `Exposure::NONE` gives the bytes of the plain render, and a render at `NONE` after
//!   an exposed one in the same session too: nothing of the exposure is left in the session.
//! - The region identity holds with an exposure: a region equals the same area of the whole image
//!   rendered with the same exposure.
//! - +1 EV is brighter and -1 EV darker than `NONE`.
//! - The statistics and the clipping map do not see an exposure.
//! - `camera_exposure()`: a DNG's `BaselineExposure`, a Fujifilm RAF's value as ExifTool's
//!   `RawExposureBias` (unrounded, `exiftool -n`), none for ARW and RW2.
//!
//! A half-size render of an X-Trans file (`.RAF`) is not deterministic in LibRaw (found in T327), so
//! there only the size is compared (`assert_same`, as in `tests/stop.rs`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pw_libraw::{CameraEv, Demosaic, Exposure, Rect, Rgb8, Session};

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

/// The files of `PW_TESTDATA` with one of the extensions (lower case), sorted; at least one.
fn files_with(extensions: &[&str]) -> Vec<PathBuf> {
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
                    .is_some_and(|e| extensions.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no {extensions:?} test files were found in PW_TESTDATA folder {}",
        dir.display()
    );
    files
}

fn arw_files() -> Vec<PathBuf> {
    files_with(&["arw"])
}

fn raf_files() -> Vec<PathBuf> {
    files_with(&["raf"])
}

fn dng_files() -> Vec<PathBuf> {
    files_with(&["dng"])
}

fn rw2_files() -> Vec<PathBuf> {
    files_with(&["rw2"])
}

/// The first ARW and the first RAF.
fn one_arw_and_one_raf() -> [PathBuf; 2] {
    [arw_files()[0].clone(), raf_files()[0].clone()]
}

fn open(path: &Path) -> Session {
    Session::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn ev(value: f32) -> Exposure {
    Exposure::from_ev(value).unwrap_or_else(|| panic!("{value} EV is outside the range"))
}

const NEVER: &(dyn Fn() -> bool + Sync) = &|| false;

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// The whole image at half size
    Half,
    /// A 600×400 px PPG region whose top left corner is at a third of the width and height
    /// (margin 8)
    RegionPpg,
}

/// The region of `Kind::RegionPpg`.
fn third_region(session: &Session) -> Rect {
    Rect::new(session.width() / 3, session.height() / 3, 600, 400)
}

/// The plain render of `kind` (no exposure parameter at all).
fn plain(session: &mut Session, kind: Kind) -> Rgb8 {
    let result = match kind {
        Kind::Half => session.render_half(),
        Kind::RegionPpg => {
            let r = third_region(session);
            session.render_region_with(r, 8, Demosaic::Ppg)
        }
    };
    result.unwrap_or_else(|e| panic!("{kind:?}: {e}"))
}

/// The render of `kind` at `exposure`.
fn exposed(session: &mut Session, kind: Kind, exposure: Exposure) -> Rgb8 {
    let result = match kind {
        Kind::Half => session.render_half_exposed(exposure, NEVER),
        Kind::RegionPpg => {
            let r = third_region(session);
            session.render_region_exposed(r, 8, Demosaic::Ppg, exposure, NEVER)
        }
    };
    result.unwrap_or_else(|e| panic!("{kind:?} at {} EV: {e}", exposure.ev()))
}

/// The whole image as one PPG region (margin 0) at `exposure`.
fn whole_ppg(session: &mut Session, exposure: Exposure) -> Rgb8 {
    let r = Rect::new(0, 0, session.width(), session.height());
    session
        .render_region_exposed(r, 0, Demosaic::Ppg, exposure, NEVER)
        .unwrap_or_else(|e| panic!("whole image at {} EV: {e}", exposure.ev()))
}

fn is_raf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("raf"))
}

/// Two renders of `kind` of `path` are the same image; for a half-size render of a RAF only the
/// size (see the head comment).
fn assert_same(path: &Path, kind: Kind, a: &Rgb8, b: &Rgb8, what: &str) {
    if is_raf(path) && matches!(kind, Kind::Half) {
        assert_eq!(
            (a.width, a.height, a.data.len()),
            (b.width, b.height, b.data.len()),
            "{}: {what}",
            path.display()
        );
    } else {
        assert!(
            a == b,
            "{}: {kind:?}: {what}: the bytes differ",
            path.display()
        );
    }
}

/// The mean of all bytes of the image.
fn mean(image: &Rgb8) -> f64 {
    let sum: u64 = image.data.iter().map(|&b| u64::from(b)).sum();
    sum as f64 / image.data.len().max(1) as f64
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_render_at_none_gives_the_plain_bytes() {
    for path in one_arw_and_one_raf() {
        for kind in [Kind::Half, Kind::RegionPpg] {
            let at_none = exposed(&mut open(&path), kind, Exposure::NONE);
            let fresh = plain(&mut open(&path), kind);
            assert_same(&path, kind, &at_none, &fresh, "a render at NONE");
        }
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn nothing_of_an_exposure_is_left_in_the_session() {
    for path in one_arw_and_one_raf() {
        for kind in [Kind::Half, Kind::RegionPpg] {
            let mut session = open(&path);
            let brighter = exposed(&mut session, kind, ev(1.0));
            let after = exposed(&mut session, kind, Exposure::NONE);
            let fresh = plain(&mut open(&path), kind);
            assert_same(&path, kind, &after, &fresh, "NONE after +1 EV");
            // the exposed render was a different image (else the test shows nothing)
            assert!(
                brighter != fresh,
                "{}: {kind:?}: +1 EV gave the plain bytes",
                path.display()
            );
        }
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_region_equals_the_whole_image_with_the_same_exposure() {
    for path in one_arw_and_one_raf() {
        let mut session = open(&path);
        let r = third_region(&session);
        for value in [1.0f32, -1.0] {
            let exposure = ev(value);
            let region = exposed(&mut session, Kind::RegionPpg, exposure);
            let whole = whole_ppg(&mut session, exposure);
            assert!(
                region == whole.crop(r),
                "{}: {value} EV: the region differs from the same area of the whole image",
                path.display()
            );
        }
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn plus_one_ev_is_brighter_and_minus_one_ev_darker() {
    for path in arw_files().into_iter().chain(raf_files()) {
        let mut session = open(&path);
        let none = mean(&exposed(&mut session, Kind::Half, Exposure::NONE));
        let up = mean(&exposed(&mut session, Kind::Half, ev(1.0)));
        let down = mean(&exposed(&mut session, Kind::Half, ev(-1.0)));
        println!(
            "{}: mean at -1 EV {down:.2}, at NONE {none:.2}, at +1 EV {up:.2}",
            path.display()
        );
        assert!(up > none, "{}: +1 EV is not brighter", path.display());
        assert!(down < none, "{}: -1 EV is not darker", path.display());
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn the_statistics_and_the_clipping_map_do_not_see_an_exposure() {
    for path in one_arw_and_one_raf() {
        let name = path.display();
        let mut session = open(&path);
        exposed(&mut session, Kind::Half, ev(2.0));
        exposed(&mut session, Kind::RegionPpg, ev(2.0));
        // darkening too: LibRaw scales its white level only then
        exposed(&mut session, Kind::Half, ev(-1.0));
        exposed(&mut session, Kind::RegionPpg, ev(-1.0));
        let stats = session
            .statistics()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let map = session
            .clipping_map()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut fresh = open(&path);
        let fresh_stats = fresh.statistics().unwrap_or_else(|e| panic!("{name}: {e}"));
        let fresh_map = fresh
            .clipping_map()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(stats.is_some(), "{name}: no statistics");
        assert!(map.is_some(), "{name}: no clipping map");
        assert!(stats == fresh_stats, "{name}: the statistics changed");
        assert!(map == fresh_map, "{name}: the clipping map changed");
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn the_camera_exposure_of_each_kind_of_file() {
    for path in arw_files().into_iter().chain(rw2_files()) {
        let camera = open(&path).camera_exposure();
        println!("{}: {camera:?}", path.display());
        assert_eq!(camera, None, "{}", path.display());
    }
    for path in raf_files() {
        let camera = open(&path).camera_exposure();
        println!("{}: {camera:?}", path.display());
        let camera = camera.unwrap_or_else(|| panic!("{}: no camera exposure", path.display()));
        assert_eq!(camera.from, CameraEv::Fujifilm, "{}", path.display());
        // ExifTool shows -0.7; its unrounded value (`exiftool -n`) is -0.72 (the owner, T331)
        assert!(
            (camera.ev - (-0.72)).abs() <= 0.01,
            "{}: {} EV, expected ExifTool's RawExposureBias -0.72",
            path.display(),
            camera.ev
        );
    }
    for path in dng_files() {
        let camera = open(&path).camera_exposure();
        println!("{}: {camera:?}", path.display());
        let name = file_name(&path);
        let expected = if name.starts_with("TN") {
            -0.51
        } else if name.starts_with("IMG_") {
            0.0
        } else {
            panic!("{}: a DNG of an unknown camera", path.display())
        };
        let camera = camera.unwrap_or_else(|| panic!("{}: no camera exposure", path.display()));
        assert_eq!(camera.from, CameraEv::DngBaseline, "{}", path.display());
        assert!(
            (camera.ev - expected).abs() <= 0.01,
            "{}: {} EV, expected {expected}",
            path.display(),
            camera.ev
        );
    }
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v.get(v.len() / 2).copied().unwrap_or_default()
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

#[test]
#[ignore = "a time measurement on real files; run with --release (ADR 0027 W5-Q4)"]
fn timing_of_exposure() {
    const RUNS: usize = 5;
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    for path in arw_files().into_iter().chain(raf_files()) {
        let name = path.display();
        let mut open_t = Vec::new();
        let mut stats_t = Vec::new();
        let mut step_t = Vec::new();
        let mut half_none = Vec::new();
        let mut half_up = Vec::new();
        let mut ppg_none = Vec::new();
        let mut ppg_up = Vec::new();
        let mut size = (0, 0);
        for _ in 0..RUNS {
            let (mut session, t) = timed(|| open(&path));
            open_t.push(t);
            size = (session.width(), session.height());
            let (stats, t) = timed(|| session.statistics());
            stats.unwrap_or_else(|e| panic!("{name}: {e}"));
            stats_t.push(t);
            // a step to an image that was not prefetched, with auto exposure on: open, statistics
            // and a half-size render, one after the other
            let (_, t) = timed(|| {
                let mut s = open(&path);
                s.statistics().unwrap_or_else(|e| panic!("{name}: {e}"));
                exposed(&mut s, Kind::Half, Exposure::NONE)
            });
            step_t.push(t);
            half_none.push(timed(|| exposed(&mut session, Kind::Half, Exposure::NONE)).1);
            half_up.push(timed(|| exposed(&mut session, Kind::Half, ev(1.0))).1);
            ppg_none.push(timed(|| whole_ppg(&mut session, Exposure::NONE)).1);
            ppg_up.push(timed(|| whole_ppg(&mut session, ev(1.0))).1);
        }
        println!(
            "{name} ({}x{}), median of {RUNS} in ms: open {:.0}, statistics {:.0}, open + \
             statistics + half NONE {:.0}; half NONE {:.0}, half +1 EV {:.0}; whole PPG NONE {:.0}, \
             whole PPG +1 EV {:.0}",
            size.0,
            size.1,
            ms(median(open_t)),
            ms(median(stats_t)),
            ms(median(step_t)),
            ms(median(half_none)),
            ms(median(half_up)),
            ms(median(ppg_none)),
            ms(median(ppg_up)),
        );
    }
}
