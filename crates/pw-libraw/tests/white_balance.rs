//! Per-render white balance, RAW channel means and white balance presets (ADR 0027 W6).
//! Run with `PW_TESTDATA` set and `--ignored --nocapture`. Use `--release` for timing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pw_libraw::{Adjust, ChannelMeans, Demosaic, Multipliers, Rect, Rgb8, Session, WhiteBalance};

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

const NEVER: &(dyn Fn() -> bool + Sync) = &|| false;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Half,
    Region,
}

fn third_region(session: &Session) -> Rect {
    Rect::new(session.width() / 3, session.height() / 3, 600, 400)
}

fn whole(session: &Session) -> Rect {
    Rect::new(0, 0, session.width(), session.height())
}

fn adjusted(session: &mut Session, kind: Kind, adjust: Adjust) -> Rgb8 {
    match kind {
        Kind::Half => session.render_half_adjusted(adjust, NEVER),
        Kind::Region => {
            session.render_region_adjusted(third_region(session), 8, Demosaic::Ppg, adjust, NEVER)
        }
    }
    .unwrap()
}

fn whole_ppg(session: &mut Session, adjust: Adjust) -> Rgb8 {
    session
        .render_region_adjusted(whole(session), 0, Demosaic::Ppg, adjust, NEVER)
        .unwrap()
}

fn plain(session: &mut Session, kind: Kind) -> Rgb8 {
    match kind {
        Kind::Half => session.render_half(),
        Kind::Region => session.render_region_with(third_region(session), 8, Demosaic::Ppg),
    }
    .unwrap()
}

fn is_raf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("raf"))
}

fn assert_same(path: &Path, kind: Kind, a: &Rgb8, b: &Rgb8, what: &str) {
    if is_raf(path) && matches!(kind, Kind::Half) {
        // Half-size X-Trans pixels are nondeterministic (T327).
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

fn balance(mul: Multipliers) -> Adjust {
    Adjust {
        white_balance: WhiteBalance::Multipliers(mul),
        ..Adjust::NONE
    }
}

fn camera(session: &Session) -> Multipliers {
    session.camera_multipliers().expect("no camera multipliers")
}

fn scaled(session: &Session, red: f32, blue: f32) -> Adjust {
    let [r, g, b] = camera(session).get();
    balance(Multipliers::new(r * red, g, b * blue).unwrap())
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn as_shot_gives_the_plain_render_before_and_after_a_balance() {
    for path in one_arw_and_one_raf() {
        for kind in [Kind::Half, Kind::Region] {
            let mut session = open(&path);
            let fresh = plain(&mut open(&path), kind);
            let before = adjusted(&mut session, kind, Adjust::NONE);
            assert_same(&path, kind, &before, &fresh, "as shot before a balance");
            let change = scaled(&session, 1.3, 0.8);
            adjusted(&mut session, kind, change);
            let after = adjusted(&mut session, kind, Adjust::NONE);
            assert_same(&path, kind, &after, &fresh, "as shot after a balance");
        }
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn camera_multipliers_give_the_as_shot_bytes() {
    for (path, kind) in [
        (arw_files().remove(0), Kind::Half),
        (rw2_files().remove(0), Kind::Half),
        (dng_files().remove(0), Kind::Half),
        (raf_files().remove(0), Kind::Region),
    ] {
        let mut session = open(&path);
        let mul = camera(&session);
        let given = adjusted(&mut session, kind, balance(mul));
        let as_shot = adjusted(&mut open(&path), kind, Adjust::NONE);
        if given != as_shot {
            let diag = session.white_balance_diagnostics();
            println!(
                "{}: cam_mul[3] = {}, cam_mul = {:?}",
                path.display(),
                diag.cam_mul[3],
                diag.cam_mul
            );
        }
        assert_same(&path, kind, &given, &as_shot, "camera multipliers");
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_balanced_region_equals_the_same_area_in_the_whole_render() {
    for path in one_arw_and_one_raf() {
        let mut session = open(&path);
        let rect = third_region(&session);
        let balance = scaled(&session, 1.3, 0.8);
        let region = adjusted(&mut session, Kind::Region, balance);
        let full = whole_ppg(&mut session, balance);
        assert_same(
            &path,
            Kind::Region,
            &region,
            &full.crop(rect),
            "region identity",
        );
    }
}

fn mean(image: &Rgb8, channel: usize) -> f64 {
    let (pixels, _) = image.data.as_chunks::<3>();
    let sum: u64 = pixels.iter().map(|rgb| u64::from(rgb[channel])).sum();
    sum as f64 / pixels.len() as f64
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn increasing_red_raises_red_and_decreasing_blue_lowers_blue() {
    for path in arw_files().into_iter().chain(raf_files()) {
        let mut session = open(&path);
        let red = scaled(&session, 1.3, 1.0);
        let blue = scaled(&session, 1.0, 0.7);
        let camera_balance = balance(camera(&session));
        let baseline = adjusted(&mut session, Kind::Half, camera_balance);
        let red_up = mean(&adjusted(&mut session, Kind::Half, red), 0);
        let blue_down = mean(&adjusted(&mut session, Kind::Half, blue), 2);
        let red_base = mean(&baseline, 0);
        let blue_base = mean(&baseline, 2);
        println!(
            "{}: red {red_base:.2} to {red_up:.2}, blue {blue_base:.2} to {blue_down:.2}",
            path.display()
        );
        assert!(red_up > red_base, "{}: red did not rise", path.display());
        assert!(
            blue_down < blue_base,
            "{}: blue did not fall",
            path.display()
        );
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn statistics_and_clipping_map_stay_as_shot() {
    for path in one_arw_and_one_raf() {
        let mut session = open(&path);
        let change = scaled(&session, 1.3, 0.8);
        adjusted(&mut session, Kind::Half, change);
        adjusted(&mut session, Kind::Region, change);
        let mut fresh = open(&path);
        let stats = session.statistics().unwrap().expect("no statistics");
        let map = session.clipping_map().unwrap().expect("no clipping map");
        assert_eq!(
            Some(stats),
            fresh.statistics().unwrap(),
            "{}",
            path.display()
        );
        assert_eq!(
            Some(map),
            fresh.clipping_map().unwrap(),
            "{}",
            path.display()
        );
    }
}

fn all_raw_files() -> impl Iterator<Item = PathBuf> {
    arw_files()
        .into_iter()
        .chain(raf_files())
        .chain(rw2_files())
        .chain(dng_files())
}

fn means(session: &mut Session, rect: Rect) -> ChannelMeans {
    session
        .channel_means(rect)
        .unwrap()
        .expect("no channel means")
}

fn assert_additive(path: &Path, full: ChannelMeans, left: ChannelMeans, right: ChannelMeans) {
    for c in 0..3 {
        assert_eq!(
            full.total[c],
            left.total[c] + right.total[c],
            "{}",
            path.display()
        );
        assert_eq!(
            full.used[c],
            left.used[c] + right.used[c],
            "{}",
            path.display()
        );
        let weighted = if full.used[c] == 0 {
            0.0
        } else {
            (left.mean[c] * left.used[c] as f64 + right.mean[c] * right.used[c] as f64)
                / full.used[c] as f64
        };
        assert!(
            (weighted - full.mean[c]).abs() < 1e-9,
            "{}: channel {c}",
            path.display()
        );
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn channel_means_count_complete_cells_and_leave_statistics_unchanged() {
    for path in all_raw_files() {
        let mut session = open(&path);
        let stats = session.statistics().unwrap().expect("no statistics");
        assert!(session.can_measure(), "{}", path.display());
        let full = whole(&session);
        let all = means(&mut session, full);
        for c in 0..3 {
            assert!(
                all.used[c] <= all.total[c] && all.mean[c].is_finite(),
                "{}",
                path.display()
            );
        }
        let mul = all.multipliers();
        println!(
            "{}: whole means {all:?}, multipliers {mul:?}",
            path.display()
        );
        if (0..3).all(|c| all.used[c] * 2 >= all.total[c]) {
            assert!(
                mul.is_some_and(|m| m.get().into_iter().all(|v| (0.2..=8.0).contains(&v))),
                "{}: unexpected whole-image multipliers {mul:?}",
                path.display()
            );
        }
        // A multiple of six is on a cell border for both mosaics.
        let x = full.w / 2 / 6 * 6;
        let y = full.h / 2 / 6 * 6;
        let square = Rect::new(x, y, 12, 12);
        let sample = means(&mut session, square);
        assert_eq!(sample, means(&mut session, square), "{}", path.display());
        let kind = if is_raf(&path) {
            assert!(
                sample.total.into_iter().all(|n| n >= 9),
                "{}",
                path.display()
            );
            "X-Trans"
        } else if stats.pixels == [u64::from(full.w) * u64::from(full.h); 3] {
            assert_eq!(sample.total, [144; 3], "{}", path.display());
            "linear"
        } else {
            assert_eq!(sample.total, [36, 72, 36], "{}", path.display());
            "Bayer"
        };
        println!("{}: {kind} 12x12 {sample:?}", path.display());
        let left = means(&mut session, Rect::new(x, y, 6, 12));
        let right = means(&mut session, Rect::new(x + 6, y, 6, 12));
        assert_additive(&path, sample, left, right);
        for invalid in [
            Rect::new(full.w, 0, 1, 1),
            Rect::new(0, 0, 0, 12),
            Rect::new(u32::MAX, 0, 2, 12),
        ] {
            assert!(
                session.channel_means(invalid).is_err(),
                "{}",
                path.display()
            );
        }
        assert_eq!(
            session.statistics().unwrap(),
            Some(stats),
            "{}",
            path.display()
        );
    }
}

fn daylight_preset(session: &Session) -> Option<Multipliers> {
    session
        .white_balance_presets()
        .iter()
        .find(|p| p.name == "Daylight")
        .map(|p| p.multipliers)
}

fn ratios(mul: Multipliers) -> [f32; 2] {
    let [r, g, b] = mul.get();
    [r / g, b / g]
}

fn check_ratios(path: &Path, actual: Multipliers, expected: [f32; 2], tolerance: f32) {
    for (actual, expected) in ratios(actual).into_iter().zip(expected) {
        assert!(
            (actual / expected - 1.0).abs() <= tolerance,
            "{}: {actual} differs from {expected} by more than {}%",
            path.display(),
            tolerance * 100.0
        );
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn named_presets_and_camera_multipliers() {
    for path in all_raw_files() {
        let session = open(&path);
        let presets = session.white_balance_presets();
        let diag = session.white_balance_diagnostics();
        println!(
            "{}: camera {:?}, {} presets {presets:?}, other IDs {:?}{}",
            path.display(),
            session.camera_multipliers(),
            presets.len(),
            diag.other_ids,
            if presets.len() == 96 {
                " (preset limit reached)"
            } else {
                ""
            }
        );
        assert!(presets.len() <= 96, "{}", path.display());
        let mut names = HashSet::new();
        for preset in presets {
            assert!(
                names.insert(&preset.name),
                "{}: {}",
                path.display(),
                preset.name
            );
            let [r, g, b] = preset.multipliers.get();
            assert_eq!(Multipliers::new(r, g, b), Some(preset.multipliers));
        }
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rw2"))
        {
            assert!(!presets.is_empty(), "{}: no presets", path.display());
        }
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("arw"))
            && daylight_preset(&session).is_none()
        {
            println!("{}: no Daylight preset", path.display());
        }
    }
    let path = testdata().join("level-c").join("PM-raw.ARW");
    if path.is_file() {
        let session = open(&path);
        let actual = daylight_preset(&session).expect("PM-raw.ARW has no Daylight preset");
        check_ratios(&path, actual, [2473.0 / 1024.0, 1606.0 / 1024.0], 0.01);
    } else {
        println!("SKIPPED: {} is missing", path.display());
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn daylight() {
    for path in all_raw_files() {
        let session = open(&path);
        let diag = session.white_balance_diagnostics();
        let actual = session.daylight();
        let preset = daylight_preset(&session);
        let difference = actual.zip(preset).map(|(actual, preset)| {
            let [ar, ab] = ratios(actual);
            let [pr, pb] = ratios(preset);
            [(ar / pr - 1.0) * 100.0, (ab / pb - 1.0) * 100.0]
        });
        println!(
            "{}: cam_mul {:?}, pre_mul {:?}, matrix_mul {:?}, daylight source {}, daylight {actual:?}, Daylight preset {preset:?}, R/G and B/G difference (%) {difference:?}",
            path.display(),
            diag.cam_mul,
            diag.pre_mul,
            diag.matrix_mul,
            diag.daylight_from
        );
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("dng"))
        {
            continue;
        }
        let actual =
            actual.unwrap_or_else(|| panic!("{}: no LibRaw daylight balance", path.display()));
        assert!(
            matches!(diag.daylight_from, 1 | 2),
            "{}: {diag:?}",
            path.display()
        );
        if diag.daylight_from == 1 {
            let [r, g, b, _] = diag.pre_mul;
            assert_eq!(actual.get(), [r, g, b], "{}", path.display());
            let [r, g, b, _] = diag.matrix_mul;
            check_ratios(&path, actual, [r / g, b / g], 0.005);
        } else {
            let [r, g, b, _] = diag.matrix_mul;
            assert_eq!(actual.get(), [r, g, b], "{}", path.display());
        }
        assert!(
            ratios(actual)
                .into_iter()
                .zip(ratios(camera(&session)))
                .any(|(actual, camera)| (actual / camera - 1.0).abs() > 0.005),
            "{}: daylight {actual:?} is within 0.5% of camera balance {:?}",
            path.display(),
            diag.cam_mul
        );
        if let Some(preset) = preset {
            check_ratios(&path, actual, ratios(preset), 0.30);
        }
    }
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

fn median_ms(mut samples: Vec<Duration>) -> f64 {
    samples.sort_unstable();
    samples[samples.len() / 2].as_secs_f64() * 1000.0
}

fn auto_adjust(session: &mut Session) -> Adjust {
    let rect = whole(session);
    means(session, rect)
        .multipliers()
        .map_or(Adjust::NONE, balance)
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored (use --release for timing)"]
fn timing_of_white_balance() {
    const RUNS: usize = 5;
    for path in arw_files().into_iter().chain(raf_files()) {
        let mut samples: [Vec<Duration>; 10] = std::array::from_fn(|_| Vec::with_capacity(RUNS));
        let mut dimensions = (0, 0);
        for _ in 0..RUNS {
            let (mut session, open_time) = timed(|| open(&path));
            dimensions = (session.width(), session.height());
            let presets = Duration::from_nanos(session.white_balance_diagnostics().presets_ns);
            let (_, stats) = timed(|| session.statistics().unwrap().expect("no statistics"));
            let (_, means_time) = timed(|| auto_adjust(&mut session));
            let step = |with_stats| {
                timed(|| {
                    let mut session = open(&path);
                    if with_stats {
                        session.statistics().unwrap().expect("no statistics");
                    }
                    let auto = auto_adjust(&mut session);
                    adjusted(&mut session, Kind::Half, auto)
                })
                .1
            };
            let change = scaled(&session, 1.3, 0.8);
            let durations = [
                open_time,
                stats,
                means_time,
                step(false),
                step(true),
                timed(|| adjusted(&mut session, Kind::Half, Adjust::NONE)).1,
                timed(|| adjusted(&mut session, Kind::Half, change)).1,
                timed(|| whole_ppg(&mut session, Adjust::NONE)).1,
                timed(|| whole_ppg(&mut session, change)).1,
                presets,
            ];
            for (series, duration) in samples.iter_mut().zip(durations) {
                series.push(duration);
            }
        }
        let [
            open,
            stats,
            means,
            step,
            both,
            half,
            half_wb,
            ppg,
            ppg_wb,
            presets,
        ] = samples.map(median_ms);
        println!(
            "{} ({}x{}), median of {RUNS} in ms: open {open:.2}, statistics {stats:.2}, means {means:.2}, open+means+half {step:.2}, open+statistics+means+half {both:.2}, half as shot {half:.2}, half balanced {half_wb:.2}, PPG as shot {ppg:.2}, PPG balanced {ppg_wb:.2}, presets {:.1} us",
            path.display(),
            dimensions.0,
            dimensions.1,
            presets * 1000.0
        );
    }
}
