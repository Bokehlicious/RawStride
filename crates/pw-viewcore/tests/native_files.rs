//! The core with LibRaw (PPG) and libjpeg-turbo on the real test RAWs. These tests are ignored by
//! default; run them with
//! `$env:PW_TESTDATA = "<folder>"; cargo test -p pw-viewcore --features native -- --ignored`.
#![cfg(feature = "native")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use pw_colour::{Profile, TransformCache, jpeg_source_profile};
use pw_viewcore::native::NativeBackend;
use pw_viewcore::{Backend, Config, Frame, Rgb, Source, Status, ViewCore, ViewKind, ViewOut, Zoom};

const VIEW: [u32; 2] = [1920, 1080];
const WAIT: Duration = Duration::from_secs(30);

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

fn test_files(prefix: &str) -> Vec<PathBuf> {
    let dir = testdata();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!(
            "could not read PW_TESTDATA folder {}: {error}",
            dir.display()
        )
    });
    let mut files: Vec<PathBuf> = entries
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
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        matches!(ext.to_ascii_lowercase().as_str(), "arw" | "rw2" | "dng")
                    })
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no RAW test file with prefix {prefix} was found in PW_TESTDATA folder {}",
        dir.display()
    );
    files
}

/// Every `.tif` / `.tiff` file in `PW_TESTDATA`, sorted; at least one must exist.
fn tiff_files() -> Vec<PathBuf> {
    let dir = testdata();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!(
            "could not read PW_TESTDATA folder {}: {error}",
            dir.display()
        )
    });
    let mut files: Vec<PathBuf> = entries
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
        .filter(|path| path.is_file() && pw_raw::is_tiff_file(path))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no TIFF test file was found in PW_TESTDATA folder {}",
        dir.display()
    );
    files
}

struct Screen<'a> {
    core: &'a ViewCore<NativeBackend>,
    textures: HashMap<u64, ViewOut>,
}

impl Screen<'_> {
    fn frame(&mut self) -> Frame {
        for v in self.core.take_views() {
            let bytes = u64::from(v.image.width) * u64::from(v.image.height) * 4;
            if let Some(id) = self.core.accept(&v, bytes) {
                self.textures.insert(id, v);
            }
        }
        for id in self.core.take_dropped() {
            self.textures.remove(&id);
        }
        self.core.frame(VIEW)
    }

    fn until_ready(&mut self) -> Frame {
        let end = Instant::now() + WAIT;
        loop {
            let f = self.frame();
            if f.status == Status::Ready
                || matches!(f.status, Status::Error(_))
                || Instant::now() > end
            {
                return f;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn start() -> ViewCore<NativeBackend> {
    match ViewCore::start(NativeBackend::default(), Config::default()) {
        Ok(c) => c,
        Err(e) => panic!("start: {e}"),
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a7r_vertical_images_fit_and_zoom_with_correct_sizes() {
    let files = test_files("TN1004");
    let core = start();
    core.open_folder(files);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready);
    let size = f.size.unwrap_or_else(|| panic!("no size"));
    assert!(
        size.flip == 5 || size.flip == 6,
        "A7R test files are vertical: {size:?}"
    );
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    // the half-size render reduced to the fit scale (ADR 0027 W1), at the job's density (the
    // fit scale rounded up to millionths)
    assert_eq!(tex.kind, ViewKind::Reduced);
    let density = (f.scale.unwrap_or_else(|| panic!("no scale")) * 1e6).ceil() / 1e6;
    assert_eq!(
        (tex.image.width, tex.image.height),
        pw_viewcore::scale::scaled_size(tex.region.w, tex.region.h, density)
    );
    // 100 %: a sensor region of the view's size, swapped for the vertical image
    core.toggle_zoom(None);
    let f = screen.until_ready();
    assert_eq!(f.zoom, Zoom::Scale(1.0));
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    assert!(matches!(tex.kind, ViewKind::Zoom | ViewKind::Ring));
    assert!(
        tex.region.w >= 1080 && tex.region.h >= 1920,
        "{:?}",
        tex.region
    );
    assert_eq!(
        (tex.image.width, tex.image.height),
        (tex.region.w, tex.region.h)
    );
    // the next image at 100 % (V6 keeps the level)
    core.step(1);
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready);
    assert_eq!(f.zoom, Zoom::Scale(1.0));
}

#[test]
fn a_file_that_is_not_raw_shows_an_error() {
    let tmp = std::env::temp_dir().join(format!("pw-viewcore-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let f = tmp.join("not-raw.ARW");
    let _ = std::fs::write(&f, b"this is not a raw file");
    let core = start();
    core.open_folder(vec![f]);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let frame = screen.until_ready();
    assert!(
        matches!(frame.status, Status::Error(_)),
        "{:?}",
        frame.status
    );
    assert!(frame.draw.is_none());
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn tiff_files_are_shown_as_themselves_in_both_modes() {
    for file in tiff_files() {
        let (w, h) = pw_tiff::dimensions(&file)
            .unwrap_or_else(|e| panic!("{}: dimensions: {e}", file.display()));
        for mode in [Source::Raw, Source::Jpeg] {
            let core = start();
            core.set_mode(mode);
            core.open_folder(vec![file.clone()]);
            let mut screen = Screen {
                core: &core,
                textures: HashMap::new(),
            };
            let f = screen.until_ready();
            assert_eq!(f.status, Status::Ready, "{} in {mode:?}", file.display());
            assert_eq!(f.zoom, Zoom::Fit, "{} in {mode:?}", file.display());
            assert_eq!(f.source, Source::Jpeg, "{} in {mode:?}", file.display());
            let size = f
                .size
                .unwrap_or_else(|| panic!("{} in {mode:?}: no size", file.display()));
            assert_eq!((size.w, size.h), (w, h), "{} in {mode:?}", file.display());
            let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
            assert_eq!(tex.source, Source::Jpeg, "{} in {mode:?}", file.display());
        }
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn jpeg_mode_shows_the_embedded_jpeg_upright_at_its_own_size() {
    for prefix in ["TN1004", "TN0059", "P124"] {
        let files = test_files(prefix);
        let core = start();
        core.set_mode(Source::Jpeg);
        core.open_folder(files.clone());
        let mut screen = Screen {
            core: &core,
            textures: HashMap::new(),
        };
        let f = screen.until_ready();
        assert_eq!(f.status, Status::Ready, "{prefix}");
        assert_eq!(f.source, Source::Jpeg);
        let size = f.size.unwrap_or_else(|| panic!("no size"));
        let info = pw_raw::inspect(&files[0]).unwrap_or_else(|e| panic!("{e}"));
        let large = info
            .pick(pw_raw::PreviewSize::Large)
            .unwrap_or_else(|| panic!("{prefix}: no preview"));
        assert_eq!((size.w, size.h), (large.width, large.height), "{prefix}");
        // the RAW file's orientation tag decides the rotation
        let expected = match info.orientation {
            3 => 3,
            6 => 6,
            8 => 5,
            _ => 0,
        };
        assert_eq!(size.flip, expected, "{prefix}");
        // 100 % of the JPEG
        core.toggle_zoom(None);
        let f = screen.until_ready();
        assert_eq!(f.status, Status::Ready, "{prefix} at 100 %");
        let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
        assert_eq!(tex.source, Source::Jpeg);
        assert_eq!(
            (tex.image.width, tex.image.height),
            (tex.region.w, tex.region.h)
        );
    }
}

/// `render_half` of `path` opened as `mode` with `backend`, converted to the display with
/// `to_display` as the core shows it (ADR 0027 W1-Q4), and how long both took.
fn timed_half(backend: &NativeBackend, path: &std::path::Path, mode: Source) -> (Rgb, Duration) {
    let (mut session, _) = backend
        .open(path, mode)
        .unwrap_or_else(|e| panic!("{} in {mode:?}: open: {}", path.display(), e.message));
    let start = Instant::now();
    let half = backend
        .render_half(&mut session)
        .unwrap_or_else(|e| panic!("{} in {mode:?}: render_half: {}", path.display(), e.message));
    let half = backend
        .to_display(&session, half)
        .unwrap_or_else(|e| panic!("{} in {mode:?}: to_display: {}", path.display(), e.message));
    (half, start.elapsed())
}

/// The source profile of the large embedded JPEG of `path`, found as the backend finds it
/// (ADR 0016 §3.1), and that JPEG decoded at half size without conversion.
fn embedded_jpeg_source(path: &std::path::Path) -> (Profile, Rgb) {
    let info = pw_raw::inspect(path).unwrap_or_else(|e| panic!("{e}"));
    let preview = info
        .pick(pw_raw::PreviewSize::Large)
        .unwrap_or_else(|| panic!("{}: no preview", path.display()));
    let data = pw_raw::read_preview(path, preview).unwrap_or_else(|e| panic!("{e}"));
    let mut decoder = pw_libjpeg::Decoder::new().unwrap_or_else(|e| panic!("{e}"));
    let icc = decoder.icc_profile(&data).unwrap_or_else(|e| panic!("{e}"));
    let source = jpeg_source_profile(
        icc.as_deref(),
        info.colour_space == pw_raw::ExifColourSpace::AdobeRgb,
    );
    let img = decoder
        .decode(&data, pw_libjpeg::Scale::Half)
        .unwrap_or_else(|e| panic!("{e}"));
    let plain = Rgb {
        width: img.width,
        height: img.height,
        data: img.data,
    };
    (source, plain)
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_camera_raw_is_converted_to_the_monitor_profile_in_both_modes() {
    let file = test_files("TN1004")[0].clone();
    let srgb_backend = NativeBackend::default();
    let adobe = Profile::adobe_rgb();
    let adobe_backend = NativeBackend::default().with_monitor_profile(adobe.clone());
    let cache = TransformCache::default();

    for mode in [Source::Raw, Source::Jpeg] {
        let (srgb_half, srgb_time) = timed_half(&srgb_backend, &file, mode);
        let (adobe_half, adobe_time) = timed_half(&adobe_backend, &file, mode);
        // RAW: LibRaw renders sRGB, and the sRGB backend leaves it unchanged (F3). JPEG: the
        // embedded JPEG converted from its own source profile.
        let (source, plain) = match mode {
            Source::Raw => (Profile::srgb(), srgb_half.clone()),
            Source::Jpeg => embedded_jpeg_source(&file),
        };
        let mut expected = plain.data;
        cache
            .get(&source, &adobe)
            .and_then(|t| t.convert_rgb8(&mut expected))
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            (adobe_half.width, adobe_half.height),
            (plain.width, plain.height),
            "{} in {mode:?}",
            file.display()
        );
        assert!(
            adobe_half.data == expected,
            "{} in {mode:?}: the Adobe RGB render is not the converted render",
            file.display()
        );
        println!(
            "{} in {mode:?}: source {source:?}; render_half {}x{} destination sRGB {:.1} ms, \
             destination Adobe RGB {:.1} ms",
            file.display(),
            adobe_half.width,
            adobe_half.height,
            srgb_time.as_secs_f64() * 1000.0,
            adobe_time.as_secs_f64() * 1000.0,
        );
    }
}

/// ADR 0007 §9.1 point 2 (T89): a LibRaw session keeps its file open, so a DNG shown in RAW mode
/// cannot be opened exclusively (as a metadata write needs) until the core releases it.
#[cfg(windows)]
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn release_closes_the_libraw_session_of_a_dng_so_it_can_be_opened_exclusively() {
    use std::os::windows::fs::OpenOptionsExt;
    let exclusive =
        |p: &std::path::Path| std::fs::OpenOptions::new().read(true).share_mode(0).open(p);
    let Some(dng) = test_files("").into_iter().find(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
    }) else {
        panic!("no DNG test file was found in PW_TESTDATA");
    };
    let core = start();
    core.set_mode(Source::Raw);
    core.open_folder(vec![dng.clone()]);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready, "{}", dng.display());
    assert_eq!(f.source, Source::Raw);
    // let the plan settle (every wanted view rendered), so nothing opens the file again
    let end = Instant::now() + WAIT;
    let mut last = (core.stats().unpacks, core.stats().renders);
    let mut since = Instant::now();
    while since.elapsed() < Duration::from_secs(1) {
        assert!(Instant::now() < end, "the plan did not settle");
        screen.frame();
        let now = (core.stats().unpacks, core.stats().renders);
        if now != last {
            last = now;
            since = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(core.memory().0 > 0, "a session of the DNG exists");
    assert!(
        exclusive(&dng).is_err(),
        "the DNG could be opened exclusively while a LibRaw session holds it"
    );
    assert!(core.releaser().release(&dng, Duration::from_secs(5)));
    let opened = exclusive(&dng);
    assert!(
        opened.is_ok(),
        "the DNG could not be opened exclusively after release: {:?}",
        opened.err()
    );
}

/// Frames until the current image's statistics are in the frame (or the timeout).
fn until_statistics(screen: &mut Screen<'_>) -> Frame {
    let end = Instant::now() + WAIT;
    loop {
        let f = screen.frame();
        if f.statistics.is_some() || Instant::now() > end {
            return f;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// ADR 0017 §2.6, U7 (T142): the core gives the statistics of the current image after its first
/// view: in RAW mode those of the unpacked RAW data, as `pw-libraw` computes them; in JPEG mode
/// those of the embedded JPEG.
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_camera_raw_gets_its_statistics_in_both_modes() {
    use pw_viewcore::{ImageStatistics, StatisticsOrigin};

    let file = test_files("TN006067").remove(0);
    let raw = pw_libraw::Session::open(&file)
        .unwrap()
        .statistics()
        .unwrap()
        .unwrap();

    let core = start();
    core.open_folder(vec![file.clone()]);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready);
    let f = until_statistics(&mut screen);
    let Some(ImageStatistics::Available(stats)) = f.statistics else {
        panic!("no RAW statistics: {:?}", f.statistics);
    };
    assert_eq!(stats.origin, StatisticsOrigin::RawData);
    assert_eq!(stats.histogram, raw.histogram);
    for c in 0..3 {
        assert_eq!(stats.clipped[c], raw.clipped_share(c));
    }
    assert_eq!(stats.deep_shadows, raw.deep_shadow_share());
    // the blown-out sky clips the green channel
    assert!(stats.clipped[1] > 0.02, "{:?}", stats.clipped);

    core.toggle_mode();
    let f = screen.until_ready();
    assert_eq!(f.source, Source::Jpeg);
    let f = until_statistics(&mut screen);
    let Some(ImageStatistics::Available(stats)) = f.statistics else {
        panic!("no JPEG statistics: {:?}", f.statistics);
    };
    assert_eq!(stats.origin, StatisticsOrigin::EmbeddedJpeg);
    let size = f.size.unwrap();
    let pixels = u64::from(size.w) * u64::from(size.h);
    for channel in &stats.histogram {
        assert_eq!(channel.iter().sum::<u64>(), pixels);
    }
    assert!(stats.clipped[1] > 0.02, "{:?}", stats.clipped);
    eprintln!("statistics ms: {:?}", core.stats().statistics_ms);
    assert_eq!(core.stats().statistics, 2);
}

/// Frames until the current image's clipping state is no longer "reading" (or the timeout).
fn until_clipping(screen: &mut Screen<'_>) -> Frame {
    let end = Instant::now() + WAIT;
    loop {
        let f = screen.frame();
        if f.clipping != pw_viewcore::ClippingState::Reading || Instant::now() > end {
            return f;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// ADR 0017 §2.6, U7b (T147): with the wish on, the core gives the clipping map of the current
/// image after its statistics: in RAW mode the map `pw-libraw` makes of the unpacked RAW data
/// (T146), with its size; in JPEG mode one of the embedded JPEG in cells of 2 × 2 pixels. Without
/// the wish none is asked for.
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_camera_raw_gets_its_clipping_map_in_both_modes_when_it_is_wanted() {
    use pw_viewcore::{ClippingMap, ClippingState};

    let file = test_files("TN006067").remove(0);
    let (raw, raw_width, raw_height) = {
        let mut session = pw_libraw::Session::open(&file).unwrap();
        let map = session.clipping_map().unwrap().unwrap();
        (map, session.width(), session.height())
    };
    assert_eq!(raw.width, raw_width.div_ceil(raw.cell));
    assert_eq!(raw.height, raw_height.div_ceil(raw.cell));

    // straight from the backend: T146's map, with its size
    let backend = NativeBackend::default();
    let (mut session, size) = backend.open(&file, Source::Raw).unwrap();
    let map = backend.clipping_map(&mut session).unwrap();
    assert_eq!((size.w, size.h), (raw_width, raw_height));
    assert_eq!(
        (map.cell, map.width, map.height),
        (raw.cell, raw.width, raw.height)
    );
    assert_eq!(map.data, raw.data);
    drop(session);

    let core = start();
    core.open_folder(vec![file.clone()]);
    let mut screen = Screen {
        core: &core,
        textures: HashMap::new(),
    };
    let f = screen.until_ready();
    assert_eq!(f.status, Status::Ready);
    // not wanted: the statistics come, no map is asked for
    let f = until_statistics(&mut screen);
    assert!(f.statistics.is_some());
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(screen.frame().clipping, ClippingState::NotWanted);
    assert_eq!(core.stats().clipping_maps, 0);

    core.set_clipping_wanted(true);
    let f = until_clipping(&mut screen);
    let ClippingState::Ready { map, uv } = &f.clipping else {
        panic!("no RAW clipping map: {:?}", f.clipping);
    };
    assert_eq!(
        (map.cell, map.width, map.height),
        (raw.cell, raw.width, raw.height)
    );
    assert_eq!(map.data, raw.data);
    // the blown-out sky clips the green channel
    assert!(map.data.iter().any(|c| c & ClippingMap::CLIPPED_G != 0));
    // at fit the whole image is drawn: every corner of the map's grid, within a cell
    for corner in uv {
        for c in corner {
            assert!(*c == 0.0 || (0.999..=1.0).contains(c), "{uv:?}");
        }
    }
    assert_eq!(core.stats().clipping_maps, 1);

    core.toggle_mode();
    let f = screen.until_ready();
    assert_eq!(f.source, Source::Jpeg);
    let f = until_clipping(&mut screen);
    let ClippingState::Ready { map, .. } = &f.clipping else {
        panic!("no JPEG clipping map: {:?}", f.clipping);
    };
    let size = f.size.unwrap();
    assert_eq!(
        (map.cell, map.width, map.height),
        (2, size.w.div_ceil(2), size.h.div_ceil(2))
    );
    assert_eq!(
        map.data.len() as u64,
        u64::from(map.width) * u64::from(map.height)
    );
    assert!(map.data.iter().any(|c| c & ClippingMap::CLIPPED_G != 0));
    eprintln!("clipping map ms: {:?}", core.stats().clipping_ms);
    assert_eq!(core.stats().clipping_maps, 2);

    // back in RAW mode the kept map is given again without being asked again
    core.toggle_mode();
    let f = screen.until_ready();
    assert_eq!(f.source, Source::Raw);
    let f = until_clipping(&mut screen);
    let ClippingState::Ready { map, .. } = &f.clipping else {
        panic!("the RAW clipping map was not kept: {:?}", f.clipping);
    };
    assert_eq!(map.data, raw.data);
    assert_eq!(core.stats().clipping_maps, 2);
}

/// Frames until the current image's focus state is no longer "reading" (or the timeout).
fn until_focus(screen: &mut Screen<'_>) -> Frame {
    let end = Instant::now() + WAIT;
    loop {
        let f = screen.frame();
        if f.focus != pw_viewcore::FocusState::Reading || Instant::now() > end {
            return f;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Frames until `done` holds for a frame; an error frame or a wait past `WAIT` fails the test
/// with `what` and the file's name.
fn until_frame(
    screen: &mut Screen<'_>,
    file: &std::path::Path,
    what: &str,
    done: impl Fn(&Frame) -> bool,
) -> Frame {
    let end = Instant::now() + WAIT;
    loop {
        let f = screen.frame();
        if let Status::Error(e) = &f.status {
            panic!("{}: {what}: error {e:?}", file.display());
        }
        if done(&f) {
            return f;
        }
        assert!(
            Instant::now() <= end,
            "{}: {what}: not done within {WAIT:?} ({:?})",
            file.display(),
            f.status
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The median and the largest of `values` (0 when there are none).
fn median_and_max(values: &[f64]) -> (f64, f64) {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let median = match v.len() {
        0 => 0.0,
        n if n % 2 == 1 => v[n / 2],
        n => (v[n / 2 - 1] + v[n / 2]) / 2.0,
    };
    (median, v.last().copied().unwrap_or(0.0))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// How long an image stays on the screen before the next step or Z in the passes "at 200 ms"
/// (T176): past the core's `Config::dwell` (150 ms), so the focus map has been started.
const SHOWN_FOR: Duration = Duration::from_millis(200);

/// Keeps drawing frames until `SHOWN_FOR` has passed since `from` (the image's first `Ready`
/// frame) and gives the last frame.
fn shown_for(screen: &mut Screen<'_>, from: Instant) -> Frame {
    loop {
        let f = screen.frame();
        if from.elapsed() >= SHOWN_FOR {
            return f;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// ADR 0017 U9a (T176): a measurement, not a check of times. On every TN RAW file in RAW mode,
/// once with the focus marking off and once with it on (each on a fresh core): the time from
/// `step(1)` to the next image shown, stepping as soon as the image before is shown, and the time
/// of Z (fit → 100 %) once the image's statistics (and, with the marking on, its focus map) are
/// there. Then the same stepping and Z with each image shown for 200 ms first (past the core's
/// dwell, so the focus map is being made), counting those where the last frame before had the
/// focus map `Reading`. Prints the median and the largest of each, and of the focus maps' own
/// times.
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn focus_map_time_and_stepping_and_zoom_with_and_without_the_marking() {
    use pw_viewcore::FocusState;

    let files = test_files("TN");
    for marking in [false, true] {
        let label = if marking { "marking on" } else { "marking off" };
        let core = start();
        core.set_mode(Source::Raw);
        if marking {
            core.set_focus_wanted(true);
        }
        core.open_folder(files.clone());
        let mut screen = Screen {
            core: &core,
            textures: HashMap::new(),
        };
        let shown = |pos: usize| move |f: &Frame| f.pos == Some(pos) && f.status == Status::Ready;
        until_frame(&mut screen, &files[0], "open", shown(0));

        // stepping: step as soon as the image before is shown
        let mut steps = Vec::new();
        for (pos, file) in files.iter().enumerate().skip(1) {
            let at = Instant::now();
            core.step(1);
            until_frame(&mut screen, file, "step", shown(pos));
            steps.push(ms(at.elapsed()));
        }

        // Z: back to the first file; each file's statistics (and focus map) first
        core.go(0);
        until_frame(&mut screen, &files[0], "go to the first", shown(0));
        let mut zooms = Vec::new();
        for (pos, file) in files.iter().enumerate() {
            let f = until_statistics(&mut screen);
            assert!(f.statistics.is_some(), "{}: no statistics", file.display());
            if marking {
                let f = until_focus(&mut screen);
                assert!(
                    matches!(f.focus, FocusState::Ready { .. }),
                    "{}: the focus map is not ready: {:?}",
                    file.display(),
                    f.focus
                );
            }
            let at = Instant::now();
            core.toggle_zoom(None);
            until_frame(&mut screen, file, "Z", |f| {
                f.pos == Some(pos) && f.zoom != Zoom::Fit && f.status == Status::Ready
            });
            zooms.push(ms(at.elapsed()));
            core.fit();
            until_frame(&mut screen, file, "fit", |f| {
                f.pos == Some(pos) && f.zoom == Zoom::Fit && f.status == Status::Ready
            });
            if pos + 1 < files.len() {
                core.step(1);
                until_frame(&mut screen, &files[pos + 1], "step", shown(pos + 1));
            }
        }

        // (a) stepping at 200 ms: each image shown for `SHOWN_FOR` before the timed step
        core.go(0);
        until_frame(&mut screen, &files[0], "go to the first", shown(0));
        let mut from = Instant::now();
        let mut late_steps = Vec::new();
        let mut late_steps_reading = 0;
        for (pos, file) in files.iter().enumerate().skip(1) {
            if shown_for(&mut screen, from).focus == FocusState::Reading {
                late_steps_reading += 1;
            }
            let at = Instant::now();
            core.step(1);
            until_frame(&mut screen, file, "step at 200 ms", shown(pos));
            from = Instant::now();
            late_steps.push(ms(at.elapsed()));
        }

        // (b) Z at 200 ms: each image shown for `SHOWN_FOR` before the timed Z
        core.go(0);
        until_frame(&mut screen, &files[0], "go to the first", shown(0));
        let mut from = Instant::now();
        let mut late_zooms = Vec::new();
        let mut late_zooms_reading = 0;
        for (pos, file) in files.iter().enumerate() {
            if shown_for(&mut screen, from).focus == FocusState::Reading {
                late_zooms_reading += 1;
            }
            let at = Instant::now();
            core.toggle_zoom(None);
            until_frame(&mut screen, file, "Z at 200 ms", |f| {
                f.pos == Some(pos) && f.zoom != Zoom::Fit && f.status == Status::Ready
            });
            late_zooms.push(ms(at.elapsed()));
            core.fit();
            until_frame(&mut screen, file, "fit", |f| {
                f.pos == Some(pos) && f.zoom == Zoom::Fit && f.status == Status::Ready
            });
            if pos + 1 < files.len() {
                core.step(1);
                until_frame(&mut screen, &files[pos + 1], "step", shown(pos + 1));
                from = Instant::now();
            }
        }

        let (step_median, step_max) = median_and_max(&steps);
        let (zoom_median, zoom_max) = median_and_max(&zooms);
        eprintln!(
            "{label}: {} files; stepping median {step_median:.1} ms, largest {step_max:.1} ms \
             ({} steps); Z median {zoom_median:.1} ms, largest {zoom_max:.1} ms ({} Z)",
            files.len(),
            steps.len(),
            zooms.len(),
        );
        let (late_step_median, late_step_max) = median_and_max(&late_steps);
        eprintln!(
            "{label}: stepping at 200 ms median {late_step_median:.1} ms, largest \
             {late_step_max:.1} ms ({} steps, {late_steps_reading} with focus Reading)",
            late_steps.len(),
        );
        let (late_zoom_median, late_zoom_max) = median_and_max(&late_zooms);
        eprintln!(
            "{label}: Z at 200 ms median {late_zoom_median:.1} ms, largest {late_zoom_max:.1} ms \
             ({} Z, {late_zooms_reading} with focus Reading)",
            late_zooms.len(),
        );
        let stats = core.stats();
        if marking {
            let (focus_median, focus_max) = median_and_max(&stats.focus_ms);
            eprintln!(
                "{label}: focus_maps {}; focus_ms median {focus_median:.1} ms, largest \
                 {focus_max:.1} ms",
                stats.focus_maps
            );
            assert!(
                stats.focus_maps >= files.len() as u64,
                "{} focus maps for {} files",
                stats.focus_maps,
                files.len()
            );
        } else {
            assert_eq!(stats.focus_maps, 0);
        }
    }
}

/// The screen a fine view is measured for: the owner's 4K screen, as T307's timing (ADR 0027 W2).
const SCREEN_4K: [u32; 2] = [3840, 2160];

/// ADR 0027 W2-Q1 (T312): a measurement, not a check of times. For one RAW file of every distinct
/// upright size among the TN and P files: the fine view's stages, (a) `render_region` of the whole
/// image (PPG), (b) `scale::reduce` of it to the fit size on a 4K screen and (c) `to_display`, and
/// for comparison (d) today's reduced view (`render_half`, `scale::reduce` to the same size,
/// `to_display`) when the fit scale is below 0.5. Three runs each, a new session every run (the
/// open is not timed); prints the median of each stage.
#[test]
#[ignore = "a time measurement on real files; run with --release (ADR 0027 W2-Q1)"]
fn timing_of_a_fine_view() {
    use pw_viewcore::{geometry, scale};

    const RUNS: usize = 3;

    let mut files = test_files("TN");
    files.extend(test_files("P"));
    let backend = NativeBackend::default();
    let open = |file: &std::path::Path| {
        backend
            .open(file, Source::Raw)
            .unwrap_or_else(|e| panic!("{}: open: {}", file.display(), e.message))
    };

    // the first file of every distinct upright size (a portrait and a landscape image differ)
    let mut kept = Vec::new();
    let mut uprights = Vec::new();
    for file in &files {
        let (_, size) = open(file);
        let upright = size.oriented();
        if !uprights.contains(&upright) {
            uprights.push(upright);
            kept.push((file.clone(), size));
        }
    }

    for (file, size) in kept {
        let s = geometry::fit_scale(size, SCREEN_4K);
        let (out_w, out_h) = scale::scaled_size(size.w, size.h, s);
        let fails =
            |stage: &str, message: &str| -> ! { panic!("{}: {stage}: {message}", file.display()) };
        let (mut render, mut reduce, mut display, mut half) = (vec![], vec![], vec![], vec![]);
        for _ in 0..RUNS {
            let (mut session, _) = open(&file);
            let at = Instant::now();
            let full = backend
                .render_region(&mut session, geometry::whole(size))
                .unwrap_or_else(|e| fails("(a) render_region", &e.message));
            render.push(ms(at.elapsed()));
            let at = Instant::now();
            let reduced = scale::reduce(&full, out_w, out_h)
                .unwrap_or_else(|| fails("(b) scale::reduce", "None"));
            reduce.push(ms(at.elapsed()));
            assert_eq!(
                (reduced.width, reduced.height),
                (out_w, out_h),
                "{}: (b) the reduced image's size",
                file.display()
            );
            let at = Instant::now();
            backend
                .to_display(&session, reduced)
                .unwrap_or_else(|e| fails("(c) to_display", &e.message));
            display.push(ms(at.elapsed()));
            drop(session);

            if s < 0.5 {
                let (mut session, _) = open(&file);
                let at = Instant::now();
                let img = backend
                    .render_half(&mut session)
                    .unwrap_or_else(|e| fails("(d) render_half", &e.message));
                let img = scale::reduce(&img, out_w, out_h)
                    .unwrap_or_else(|| fails("(d) scale::reduce", "None"));
                backend
                    .to_display(&session, img)
                    .unwrap_or_else(|e| fails("(d) to_display", &e.message));
                half.push(ms(at.elapsed()));
            }
        }
        let (upright_w, upright_h) = size.oriented();
        let mpix = f64::from(size.w) * f64::from(size.h) / 1e6;
        let median = |v: &[f64]| median_and_max(v).0;
        let half = if half.is_empty() {
            "n/a (fit scale ≥ 0.5)".to_owned()
        } else {
            format!("{:.1} ms", median(&half))
        };
        println!(
            "{}: sensor {}x{}, upright {upright_w}x{upright_h}, {mpix:.1} Mpix, s {s:.4} -> \
             {out_w}x{out_h}; median of {RUNS}: (a) render_region {:.1} ms, (b) reduce {:.1} ms, \
             (c) to_display {:.1} ms, fine view (a+b+c) {:.1} ms; (d) half-size view {half}",
            file.display(),
            size.w,
            size.h,
            median(&render),
            median(&reduce),
            median(&display),
            median(&render) + median(&reduce) + median(&display),
        );
    }
}
