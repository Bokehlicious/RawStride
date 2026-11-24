//! The core with its workers, driven like the presentation drives it, against a simulated decoder
//! (no native libraries). Real files are checked in `native_files.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pw_viewcore::{
    Backend, Config, DecodeError, Frame, ImageSize, Rect, Rgb, Source, Status, ViewCore, ViewKind,
    ViewOut, Zoom,
};

const VIEW: [u32; 2] = [1000, 800];

/// Simulated files. RAW: "bad*" fails as damaged, the rest are 4000×3000 (flip 6 for "v*").
/// JPEG: "nojpeg*" has no embedded JPEG, the rest have a 1600×1200 one. Every pixel encodes its
/// position, so regions can be checked.
#[derive(Clone, Default)]
struct Fake {
    opens: Arc<AtomicUsize>,
    renders: Arc<Mutex<Vec<(ViewKind, Rect)>>>,
    delay: Duration,
}

fn pixel(x: u32, y: u32) -> [u8; 3] {
    [(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8]
}

impl Backend for Fake {
    type Session = (u32, u32);

    fn open(&self, path: &Path, source: Source) -> Result<((u32, u32), ImageSize), DecodeError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let flip = if name.starts_with('v') { 6 } else { 0 };
        let (w, h) = match source {
            Source::Raw if name.starts_with("bad") => {
                return Err(DecodeError::damaged("damaged: RAW data incomplete"));
            }
            Source::Jpeg if name.starts_with("nojpeg") => {
                return Err(DecodeError::new("the file has no embedded JPEG"));
            }
            Source::Raw => (4000, 3000),
            Source::Jpeg => (1600, 1200),
        };
        Ok(((w, h), ImageSize { w, h, flip }))
    }

    fn render_half(&self, s: &mut (u32, u32)) -> Result<Rgb, DecodeError> {
        std::thread::sleep(self.delay);
        self.renders
            .lock()
            .map_err(|_| DecodeError::new("lock"))?
            .push((ViewKind::Half, Rect::new(0, 0, s.0, s.1)));
        let (w, h) = (s.0 / 2, s.1 / 2);
        let mut data = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&pixel(x * 2, y * 2));
            }
        }
        Ok(Rgb {
            width: w,
            height: h,
            data,
        })
    }

    fn render_region(&self, _s: &mut (u32, u32), r: Rect) -> Result<Rgb, DecodeError> {
        std::thread::sleep(self.delay);
        self.renders
            .lock()
            .map_err(|_| DecodeError::new("lock"))?
            .push((ViewKind::Zoom, r));
        let mut data = Vec::with_capacity((r.w * r.h * 3) as usize);
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                data.extend_from_slice(&pixel(x, y));
            }
        }
        Ok(Rgb {
            width: r.w,
            height: r.h,
            data,
        })
    }
}

/// The presentation's side: uploads finished views into a map of "textures".
struct Screen<'a, B: Backend> {
    core: &'a ViewCore<B>,
    textures: HashMap<u64, ViewOut>,
}

impl<'a, B: Backend> Screen<'a, B> {
    fn new(core: &'a ViewCore<B>) -> Self {
        Screen {
            core,
            textures: HashMap::new(),
        }
    }

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
        let f = self.core.frame(VIEW);
        if let Some(d) = f.draw {
            assert!(
                self.textures.contains_key(&d.id),
                "drawn texture {} is resident",
                d.id
            );
        }
        f
    }

    /// Frames until `done` or the timeout; returns the last frame.
    fn until(&mut self, max: Duration, done: impl Fn(&Frame) -> bool) -> Frame {
        let end = Instant::now() + max;
        loop {
            let f = self.frame();
            if done(&f) || Instant::now() > end {
                return f;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn files(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

fn ready(f: &Frame) -> bool {
    f.status == Status::Ready
}

const WAIT: Duration = Duration::from_secs(10);

fn start(fake: Fake, config: Config) -> ViewCore<Fake> {
    match ViewCore::start(fake, config) {
        Ok(c) => c,
        Err(e) => panic!("start: {e}"),
    }
}

#[test]
fn the_first_image_appears_and_neighbours_are_prefetched() {
    let fake = Fake::default();
    let core = start(fake.clone(), Config::default());
    core.open_folder(files(&["a", "b", "c", "d", "e"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, ready);
    assert_eq!(f.status, Status::Ready);
    assert_eq!(f.pos, Some(0));
    // after the dwell the next two images are unpacked and their fit views prepared
    screen.until(WAIT, |_| fake.opens.load(Ordering::SeqCst) >= 3);
    std::thread::sleep(Duration::from_millis(300));
    screen.frame();
    core.step(1);
    let f = screen.frame();
    assert_eq!(f.status, Status::Ready, "image 1 was prefetched");
    assert_eq!(f.pos, Some(1));
}

#[test]
fn zoom_to_100_percent_draws_the_exact_region_with_correct_pixels() {
    let fake = Fake::default();
    let core = start(fake.clone(), Config::default());
    core.open_folder(files(&["a"]));
    let mut screen = Screen::new(&core);
    screen.until(WAIT, ready);
    core.toggle_zoom(None);
    let f = screen.until(WAIT, ready);
    assert_eq!(f.zoom, Zoom::Scale(1.0));
    let d = f.draw.unwrap_or_else(|| panic!("nothing drawn"));
    let tex = &screen.textures[&d.id];
    assert!(matches!(tex.kind, ViewKind::Zoom | ViewKind::Ring));
    // the top-left corner of the view shows sensor pixel (1500, 1100) of a centred 1000x800 view
    let (u, v) = (d.uv[0][0] as f64, d.uv[0][1] as f64);
    let x = tex.region.x as f64 + u * tex.region.w as f64;
    let y = tex.region.y as f64 + v * tex.region.h as f64;
    assert!(
        (x - 1500.0).abs() < 0.5 && (y - 1100.0).abs() < 0.5,
        "({x}, {y})"
    );
    let i = (((1100 - tex.region.y) * tex.image.width + (1500 - tex.region.x)) * 3) as usize;
    assert_eq!(tex.image.data[i..i + 3], pixel(1500, 1100));
}

#[test]
fn a_free_zoom_level_is_rendered_scaled_and_sharp() {
    let fake = Fake::default();
    let core = start(fake.clone(), Config::default());
    core.open_folder(files(&["a"]));
    let mut screen = Screen::new(&core);
    screen.until(WAIT, ready);
    // let the prefetch finish (the 100 % target and the ring around the centre)
    std::thread::sleep(Duration::from_millis(400));
    screen.frame();
    // zoom from fit (25 %) to 70 % at the image's top-left corner: outside the prefetched ring,
    // so a scaled view is rendered
    core.zoom_by(2.8, Some([1.0, 26.0]));
    let f = screen.until(WAIT, |f| ready(f) && f.zoom == Zoom::Scale(0.7));
    assert_eq!(f.status, Status::Ready);
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    assert_eq!(tex.kind, ViewKind::Scaled);
    assert!(tex.density >= 0.7);
    // the texture is about the size of the view, not of the sensor region
    assert!(
        tex.image.width <= 1001 && tex.image.height <= 801,
        "{}x{}",
        tex.image.width,
        tex.image.height
    );
    assert!(tex.region.x <= 4 && tex.region.y <= 4, "{:?}", tex.region);
}

#[test]
fn the_ring_serves_free_levels_around_the_centre_at_once() {
    let core = start(Fake::default(), Config::default());
    core.open_folder(files(&["a"]));
    let mut screen = Screen::new(&core);
    screen.until(WAIT, ready);
    core.toggle_zoom(None);
    let end = Instant::now() + WAIT;
    while !screen.textures.values().any(|v| v.kind == ViewKind::Ring) && Instant::now() < end {
        screen.frame();
        std::thread::sleep(Duration::from_millis(2));
    }
    core.zoom_by(0.7, None);
    let f = screen.frame();
    assert_eq!(
        f.status,
        Status::Ready,
        "70 % at the centre is inside the ring at 100 %"
    );
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    assert_eq!(tex.kind, ViewKind::Ring);
}

#[test]
fn a_damaged_file_shows_its_error_and_stepping_on_works() {
    let core = start(Fake::default(), Config::default());
    core.open_folder(files(&["a", "bad", "c"]));
    let mut screen = Screen::new(&core);
    screen.until(WAIT, ready);
    core.step(1);
    let f = screen.until(WAIT, |f| matches!(f.status, Status::Error(_)));
    assert!(matches!(f.status, Status::Error(ref e) if e.damaged));
    assert!(f.draw.is_none());
    core.step(1);
    let f = screen.until(WAIT, ready);
    assert_eq!(f.pos, Some(2));
}

#[test]
fn the_texture_budget_holds_while_stepping_through_a_folder() {
    // about three fit views of 4000x3000 (reduced to 1000x750, ADR 0027 W1) with their 100 %
    // views (1000x800)
    let budget = 20 << 20;
    let config = Config {
        texture_budget: budget,
        ..Config::default()
    };
    let core = start(Fake::default(), config);
    let names: Vec<String> = (0..12).map(|i| format!("img{i}")).collect();
    core.open_folder(names.iter().map(PathBuf::from).collect());
    let mut screen = Screen::new(&core);
    for _ in 0..11 {
        screen.until(WAIT, ready);
        let (_, tex) = core.memory();
        assert!(tex <= budget + (12 << 20), "texture bytes {tex}");
        core.step(1);
    }
    let f = screen.until(WAIT, ready);
    assert_eq!(f.pos, Some(11));
    assert!(core.stats().evicted_textures > 0);
}

#[test]
fn a_vertical_image_is_drawn_rotated() {
    let core = start(Fake::default(), Config::default());
    core.open_folder(files(&["v1"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, ready);
    assert_eq!(f.size.map(|s| s.oriented()), Some((3000, 4000)));
    let d = f.draw.unwrap_or_else(|| panic!("nothing drawn"));
    // flip 6: the upright top-left corner is the sensor's bottom-left corner
    assert_eq!(d.uv[0], [0.0, 1.0]);
    // portrait in a landscape view: full height, centred horizontally
    assert!((d.dest[3] - d.dest[1] - 800.0).abs() < 0.01);
    assert!((d.dest[0] - (1000.0 - 600.0) / 2.0).abs() < 0.01);
}

#[test]
fn a_new_folder_drops_the_old_textures() {
    let core = start(Fake::default(), Config::default());
    core.open_folder(files(&["a", "b"]));
    let mut screen = Screen::new(&core);
    screen.until(WAIT, ready);
    core.open_folder(files(&["c"]));
    let f = screen.until(WAIT, ready);
    assert_eq!(f.path, Some(PathBuf::from("c")));
    assert!(screen.textures.values().all(|v| v.pos == 0));
}

#[test]
fn dropping_the_core_stops_the_workers_during_a_slow_decode() {
    let fake = Fake {
        delay: Duration::from_millis(200),
        ..Fake::default()
    };
    let core = start(fake, Config::default());
    core.open_folder(files(&["a", "b", "c"]));
    std::thread::sleep(Duration::from_millis(50));
    let t = Instant::now();
    drop(core);
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn every_action_returns_at_once_while_the_workers_are_busy() {
    let fake = Fake {
        delay: Duration::from_millis(300),
        ..Fake::default()
    };
    let core = start(fake, Config::default());
    core.open_folder(files(&["a", "b", "c", "d"]));
    std::thread::sleep(Duration::from_millis(20));
    let t = Instant::now();
    core.step(1);
    core.toggle_zoom(Some([10.0, 10.0]));
    core.pan([5.0, 5.0]);
    core.zoom_steps(2, None);
    core.fit();
    let _ = core.frame(VIEW);
    assert!(t.elapsed() < Duration::from_millis(50), "{:?}", t.elapsed());
}

#[test]
fn r_switches_to_the_embedded_jpeg_and_back() {
    let core = start(Fake::default(), Config::default());
    core.open_folder(files(&["a", "bad", "nojpeg"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, ready);
    assert_eq!((f.mode, f.source), (Source::Raw, Source::Raw));
    assert_eq!(f.size.map(|s| (s.w, s.h)), Some((4000, 3000)));
    core.toggle_mode();
    let f = screen.until(WAIT, |f| ready(f) && f.source == Source::Jpeg);
    assert_eq!(f.size.map(|s| (s.w, s.h)), Some((1600, 1200)));
    // 100 % is 100 % of the JPEG
    core.toggle_zoom(None);
    let f = screen.until(WAIT, |f| ready(f) && f.zoom == Zoom::Scale(1.0));
    let tex = &screen.textures[&f.draw.map_or(0, |d| d.id)];
    assert_eq!(tex.source, Source::Jpeg);
    assert!(tex.region.x + tex.region.w <= 1600 && tex.region.y + tex.region.h <= 1200);
    // a damaged RAW still shows its embedded JPEG (G3)
    core.fit();
    core.step(1);
    let f = screen.until(WAIT, |f| ready(f) && f.pos == Some(1));
    assert_eq!(f.status, Status::Ready);
    // no embedded JPEG: a message, not the RAW rendering (G4)
    core.step(1);
    let f = screen.until(WAIT, |f| matches!(f.status, Status::Error(_)));
    assert!(matches!(f.status, Status::Error(ref e) if e.message.contains("no embedded JPEG")));
    assert!(f.draw.is_none());
    // back to RAW: the mode is the user's, and stays until changed
    core.toggle_mode();
    let f = screen.until(WAIT, ready);
    assert_eq!((f.mode, f.source), (Source::Raw, Source::Raw));
}

#[test]
fn a_plain_jpeg_file_is_shown_in_raw_mode_too() {
    let core = start(Fake::default(), Config::default());
    core.open_folder(files(&["photo.jpg"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, ready);
    assert_eq!((f.mode, f.source), (Source::Raw, Source::Jpeg));
}

// ---------- statistics (ADR 0017 §2.6, U7-Q6, T142) ----------

use pw_viewcore::{ImageStatistics, Statistics, StatisticsOrigin};

/// A decoder that records the order of its calls and has statistics for every file whose name
/// does not start with "none". A session is the file's name and display mode.
#[derive(Clone, Default)]
struct Recorder {
    calls: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
    fn record(&self, call: String) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    fn count(&self, call: &str) -> usize {
        self.calls().iter().filter(|c| *c == call).count()
    }

    fn position(&self, call: &str) -> usize {
        self.calls()
            .iter()
            .position(|c| c == call)
            .unwrap_or_else(|| panic!("no call {call:?} in {:?}", self.calls()))
    }
}

/// The statistics the recorder gives a session: its name and mode can be read from them.
fn recorded_statistics(session: &str) -> Statistics {
    let mut histogram = [[0u64; 256]; 3];
    histogram[0][0] = session.len() as u64;
    histogram[1][255] = u64::from(session.as_bytes()[0]);
    Statistics {
        origin: if session.ends_with("Raw") {
            StatisticsOrigin::RawData
        } else {
            StatisticsOrigin::EmbeddedJpeg
        },
        histogram,
        clipped: [0.5, 0.25, 0.0],
        deep_shadows: 0.125,
    }
}

impl Backend for Recorder {
    type Session = String;

    fn open(&self, path: &Path, source: Source) -> Result<(String, ImageSize), DecodeError> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let session = format!("{name} {source:?}");
        self.record(format!("open {session}"));
        Ok((
            session,
            ImageSize {
                w: 4000,
                h: 3000,
                flip: 0,
            },
        ))
    }

    fn render_half(&self, session: &mut String) -> Result<Rgb, DecodeError> {
        self.record(format!("half {session}"));
        Ok(Rgb {
            width: 2000,
            height: 1500,
            data: vec![0; 2000 * 1500 * 3],
        })
    }

    fn render_region(&self, session: &mut String, r: Rect) -> Result<Rgb, DecodeError> {
        self.record(format!("region {session}"));
        Ok(Rgb {
            width: r.w,
            height: r.h,
            data: vec![0; (r.w * r.h * 3) as usize],
        })
    }

    fn statistics(&self, session: &mut String) -> Option<Statistics> {
        self.record(format!("statistics {session}"));
        (!session.starts_with("none")).then(|| recorded_statistics(session))
    }
}

fn start_recorder(recorder: &Recorder) -> ViewCore<Recorder> {
    match ViewCore::start(recorder.clone(), Config::default()) {
        Ok(c) => c,
        Err(e) => panic!("start: {e}"),
    }
}

fn has_statistics(f: &Frame) -> bool {
    f.statistics.is_some()
}

fn available(session: &str) -> Option<ImageStatistics> {
    Some(ImageStatistics::Available(Arc::new(recorded_statistics(
        session,
    ))))
}

#[test]
fn statistics_are_asked_once_per_opened_image_after_its_first_view() {
    let recorder = Recorder::default();
    let core = start_recorder(&recorder);
    core.open_folder(files(&["a", "b", "c"]));
    let mut screen = Screen::new(&core);
    // the first image is shown without its statistics
    let f = screen.until(WAIT, ready);
    assert_eq!(f.status, Status::Ready);
    assert_eq!(f.statistics, None);
    assert_eq!(recorder.count("statistics a Raw"), 0);
    // then they come, in the frame of that image
    let f = screen.until(WAIT, has_statistics);
    assert_eq!(f.pos, Some(0));
    assert_eq!(f.statistics, available("a Raw"));
    // asked after the open and after the first view was rendered
    assert!(recorder.position("open a Raw") < recorder.position("half a Raw"));
    assert!(recorder.position("half a Raw") < recorder.position("statistics a Raw"));

    // the neighbours are opened ahead, but only the current image's statistics are asked
    screen.until(WAIT, |_| recorder.count("open b Raw") == 1);
    std::thread::sleep(Duration::from_millis(300));
    screen.frame();
    assert_eq!(recorder.count("statistics b Raw"), 0);
    assert_eq!(recorder.count("statistics a Raw"), 1);

    // the next image: its frame has no statistics until its own are ready
    core.step(1);
    let f = screen.frame();
    assert_eq!(f.pos, Some(1));
    assert_eq!(f.statistics, None);
    let f = screen.until(WAIT, has_statistics);
    assert_eq!(f.pos, Some(1));
    assert_eq!(f.statistics, available("b Raw"));

    // back: the first image's statistics are there at once and are not asked again
    core.step(-1);
    let f = screen.frame();
    assert_eq!(f.pos, Some(0));
    assert_eq!(f.statistics, available("a Raw"));
    std::thread::sleep(Duration::from_millis(400));
    screen.frame();
    assert_eq!(recorder.count("statistics a Raw"), 1);
    assert_eq!(recorder.count("statistics b Raw"), 1);
    assert_eq!(core.stats().statistics, 2);
}

#[test]
fn the_other_display_mode_has_its_own_statistics() {
    let recorder = Recorder::default();
    let core = start_recorder(&recorder);
    core.open_folder(files(&["a"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, has_statistics);
    assert_eq!(f.statistics, available("a Raw"));
    core.toggle_mode();
    let f = screen.until(WAIT, |f| f.source == Source::Jpeg && has_statistics(f));
    assert_eq!(f.statistics, available("a Jpeg"));
    // back in RAW mode: the first ones, not asked again
    core.toggle_mode();
    let f = screen.frame();
    assert_eq!(f.statistics, available("a Raw"));
    std::thread::sleep(Duration::from_millis(400));
    screen.frame();
    assert_eq!(recorder.count("statistics a Raw"), 1);
    assert_eq!(recorder.count("statistics a Jpeg"), 1);
}

#[test]
fn a_file_without_statistics_says_so_and_is_asked_once() {
    let recorder = Recorder::default();
    let core = start_recorder(&recorder);
    core.open_folder(files(&["none"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, has_statistics);
    assert_eq!(f.statistics, Some(ImageStatistics::Unavailable));
    std::thread::sleep(Duration::from_millis(400));
    screen.frame();
    assert_eq!(recorder.count("statistics none Raw"), 1);
}

// ---------- a view set from outside (ADR 0017 U6, T150) ----------

#[test]
fn set_view_gives_its_zoom_and_centre_in_the_frame_and_is_not_recorded() {
    let config = Config {
        record: true,
        ..Config::default()
    };
    let core = start(Fake::default(), config);
    core.open_folder(files(&["a"]));
    let mut screen = Screen::new(&core);
    let f = screen.until(WAIT, ready);
    assert_eq!(f.zoom, Zoom::Fit);
    assert_eq!(f.centre, [0.5, 0.5]);
    // the records so far (the opened folder)
    assert!(!core.take_records().is_empty());

    core.set_view(Zoom::Scale(1.0), [0.3, 0.6]);
    let f = screen.frame();
    assert_eq!(f.zoom, Zoom::Scale(1.0));
    assert_eq!(f.centre, [0.3, 0.6]);
    // the same values again, as the shell does every frame: the exact view comes after the pause
    core.set_view(Zoom::Scale(1.0), [0.3, 0.6]);
    let f = screen.until(WAIT, ready);
    assert_eq!(f.status, Status::Ready);
    assert_eq!(f.zoom, Zoom::Scale(1.0));
    assert_eq!(f.centre, [0.3, 0.6]);
    // the view's top-left corner shows sensor pixel (700, 1400): 0.3 × 4000 − 500, 0.6 × 3000 − 400
    let d = f.draw.unwrap_or_else(|| panic!("nothing drawn"));
    let tex = &screen.textures[&d.id];
    let x = tex.region.x as f64 + d.uv[0][0] as f64 * tex.region.w as f64;
    let y = tex.region.y as f64 + d.uv[0][1] as f64 * tex.region.h as f64;
    assert!(
        (x - 700.0).abs() < 0.5 && (y - 1400.0).abs() < 0.5,
        "({x}, {y})"
    );
    assert!(core.take_records().is_empty(), "set_view is not recorded");
}
