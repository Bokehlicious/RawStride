//! Engine tests with a simulated decoder (no LibRaw): priorities, cancellation, concurrency and
//! the memory budget. The simulated decoder sleeps like LibRaw would, logs every call with a
//! timestamp, and counts how many sessions are alive at once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pw_rawview::{Backend, Config, Engine, Image, ImageKey, Lookup, Rect, Target, TileId};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Open(usize),
    HalfStart(usize),
    HalfEnd(usize),
    TileStart(usize, u32, u32),
    TileEnd(usize, u32, u32),
}

#[derive(Default)]
struct Shared {
    log: Mutex<Vec<(Instant, Call)>>,
    alive: AtomicUsize,
    max_alive: AtomicUsize,
}

impl Shared {
    fn calls(&self) -> Vec<(Instant, Call)> {
        self.log.lock().unwrap().clone()
    }
    fn push(&self, c: Call) {
        self.log.lock().unwrap().push((Instant::now(), c));
    }
}

#[derive(Clone)]
struct Fake {
    shared: Arc<Shared>,
    open_ms: u64,
    half_ms: u64,
    tile_ms: u64,
    /// Encoded bytes per tile.
    tile_bytes: usize,
}

struct FakeSession {
    index: usize,
    shared: Arc<Shared>,
}

impl Drop for FakeSession {
    fn drop(&mut self) {
        self.shared.alive.fetch_sub(1, Ordering::SeqCst);
    }
}

fn index_of(path: &Path) -> usize {
    path.file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.trim_start_matches("img").parse().ok())
        .unwrap_or(usize::MAX)
}

impl Backend for Fake {
    type Session = FakeSession;
    fn open(&self, path: &Path) -> Result<FakeSession, String> {
        if path.to_string_lossy().contains("bad") {
            return Err("not a RAW file".into());
        }
        let n = self.shared.alive.fetch_add(1, Ordering::SeqCst) + 1;
        self.shared.max_alive.fetch_max(n, Ordering::SeqCst);
        let index = index_of(path);
        self.shared.push(Call::Open(index));
        std::thread::sleep(Duration::from_millis(self.open_ms));
        Ok(FakeSession {
            index,
            shared: self.shared.clone(),
        })
    }
    fn size(&self, _s: &FakeSession) -> (u32, u32) {
        (9568, 6376)
    }
    fn render_half(&self, s: &mut FakeSession) -> Result<Image, String> {
        self.shared.push(Call::HalfStart(s.index));
        std::thread::sleep(Duration::from_millis(self.half_ms));
        self.shared.push(Call::HalfEnd(s.index));
        Ok(Image {
            width: 4,
            height: 2,
            data: vec![s.index as u8; 24],
        })
    }
    fn render_region(&self, s: &mut FakeSession, r: Rect, _margin: u32) -> Result<Image, String> {
        let (c, rr) = (r.x / 1024, r.y / 1024);
        self.shared.push(Call::TileStart(s.index, c, rr));
        std::thread::sleep(Duration::from_millis(self.tile_ms));
        self.shared.push(Call::TileEnd(s.index, c, rr));
        Ok(Image {
            width: r.w,
            height: r.h,
            data: vec![1; 3],
        })
    }
    fn encode(&self, img: &Image) -> Result<Vec<u8>, String> {
        let n = if img.width <= 4 {
            1000
        } else {
            self.tile_bytes
        };
        Ok(vec![img.data[0]; n])
    }
}

fn fake(open_ms: u64, half_ms: u64, tile_ms: u64) -> (Fake, Arc<Shared>) {
    let shared = Arc::new(Shared::default());
    (
        Fake {
            shared: shared.clone(),
            open_ms,
            half_ms,
            tile_ms,
            tile_bytes: 100_000,
        },
        shared,
    )
}

fn key(g: u64, i: usize) -> ImageKey {
    ImageKey {
        generation: g,
        index: i,
    }
}

fn target(g: u64, i: usize, half: bool) -> Target {
    Target {
        key: key(g, i),
        path: PathBuf::from(format!("img{i}.ARW")),
        want_half: half,
    }
}

fn tid(col: u32, row: u32) -> TileId {
    TileId { col, row }
}

fn cfg() -> Config {
    Config {
        dwell: Duration::from_millis(60),
        ..Config::default()
    }
}

/// Waits until the engine has nothing queued and nothing running (or panics after `ms`).
/// Jobs waiting for the dwell time count as queued.
fn settle<B: Backend>(e: &Engine<B>, _shared: &Shared, ms: u64) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    let mut idle_since: Option<Instant> = None;
    while Instant::now() < deadline {
        let s = e.stats();
        if s.queued == 0 && s.running == 0 {
            let since = *idle_since.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_millis(50) {
                return;
            }
        } else {
            idle_since = None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("engine did not settle: {:?}", e.stats());
}

fn first(calls: &[(Instant, Call)], c: &Call) -> Option<Instant> {
    calls.iter().find(|(_, x)| x == c).map(|(t, _)| *t)
}

// ---------- queue priority ----------

#[test]
fn zoom_tiles_go_before_queued_background_jobs() {
    let (b, shared) = fake(40, 300, 20);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(
        target(1, 0, true),
        &[target(1, 1, true), target(1, 2, true), target(1, 3, true)],
    );
    // Let the half-size of the current image start, then zoom.
    std::thread::sleep(Duration::from_millis(150));
    let tiles = [tid(2, 2), tid(3, 2), tid(2, 3), tid(3, 3)];
    e.request_tiles(key(1, 0), &tiles);
    let t0 = Instant::now();
    for t in tiles {
        assert!(matches!(
            e.tile(key(1, 0), t, Duration::from_secs(3)),
            Lookup::Ready(_)
        ));
    }
    let tiles_done = t0.elapsed();
    settle(&e, &shared, 3000);
    let calls = shared.calls();
    // The tiles did not wait for the running half-size render (300 ms) ...
    assert!(
        tiles_done < Duration::from_millis(250),
        "tiles took {tiles_done:?}"
    );
    // ... and no queued background job started before the last tile had started.
    let last_tile_start = calls
        .iter()
        .filter(|(_, c)| matches!(c, Call::TileStart(0, _, _)))
        .map(|(t, _)| *t)
        .max()
        .unwrap();
    for i in 1..=3 {
        let h = first(&calls, &Call::HalfStart(i)).expect("neighbour half-size ran");
        assert!(h > last_tile_start, "half of {i} started before the tiles");
    }
    // Requested order is kept.
    let order: Vec<_> = calls
        .iter()
        .filter_map(|(_, c)| match c {
            Call::TileStart(0, c, r) => Some(tid(*c, *r)),
            _ => None,
        })
        .collect();
    assert_eq!(order, tiles);
}

#[test]
fn a_tile_asked_for_directly_jumps_the_tile_queue() {
    let (b, shared) = fake(10, 50, 40);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    let queued: Vec<_> = (0..6).map(|c| tid(c, 0)).collect();
    e.request_tiles(key(1, 0), &queued);
    std::thread::sleep(Duration::from_millis(60));
    assert!(matches!(
        e.tile(key(1, 0), tid(5, 5), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 2000);
    let calls = shared.calls();
    let urgent = first(&calls, &Call::TileStart(0, 5, 5)).unwrap();
    let last_queued = first(&calls, &Call::TileStart(0, 5, 0)).unwrap();
    assert!(
        urgent < last_queued,
        "the directly requested tile was not moved to the front"
    );
}

#[test]
fn a_new_viewport_replaces_the_queued_tiles() {
    let (b, shared) = fake(10, 50, 50);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    e.request_tiles(key(1, 0), &(0..8).map(|c| tid(c, 0)).collect::<Vec<_>>());
    std::thread::sleep(Duration::from_millis(80));
    e.request_tiles(key(1, 0), &[tid(0, 4), tid(1, 4)]);
    settle(&e, &shared, 2000);
    let started: Vec<_> = shared
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            Call::TileStart(0, c, r) => Some((c, r)),
            _ => None,
        })
        .collect();
    assert!(started.contains(&(0, 4)) && started.contains(&(1, 4)));
    assert!(
        started.len() < 6,
        "old viewport kept rendering: {started:?}"
    );
    assert!(e.stats().jobs_dropped > 0);
}

// ---------- cancellation ----------

#[test]
fn flicking_through_images_starts_no_raw_work() {
    let (b, shared) = fake(30, 100, 20);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    for i in 0..30 {
        e.set_current(target(1, i, true), &[target(1, i + 1, true)]);
        std::thread::sleep(Duration::from_millis(15)); // faster than the 60 ms dwell
    }
    std::thread::sleep(Duration::from_millis(400));
    let opened: Vec<usize> = shared
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            Call::Open(i) => Some(i),
            _ => None,
        })
        .collect();
    assert!(
        opened.iter().all(|&i| i >= 29),
        "work started for images that were only passed: {opened:?}"
    );
    assert!(e.stats().jobs_dropped >= 28);
}

#[test]
fn queued_jobs_of_a_left_image_are_removed() {
    let (b, shared) = fake(30, 200, 30);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(
        target(1, 0, true),
        &[target(1, 1, true), target(1, 2, true)],
    );
    e.request_tiles(key(1, 0), &(0..6).map(|c| tid(c, 1)).collect::<Vec<_>>());
    std::thread::sleep(Duration::from_millis(100));
    e.set_current(target(1, 10, true), &[]);
    settle(&e, &shared, 3000);
    let calls = shared.calls();
    let tiles_of_0 = calls
        .iter()
        .filter(|(_, c)| matches!(c, Call::TileStart(0, _, _)))
        .count();
    assert!(tiles_of_0 < 6, "all tiles of the left image still ran");
    assert!(first(&calls, &Call::HalfStart(1)).is_none());
    assert!(first(&calls, &Call::HalfStart(2)).is_none());
    assert!(first(&calls, &Call::HalfStart(10)).is_some());
    assert_eq!(e.tile(key(1, 0), tid(0, 1), Duration::ZERO), Lookup::Gone);
}

#[test]
fn a_running_call_finishes_but_its_result_is_thrown_away() {
    let (b, shared) = fake(10, 50, 300);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    e.request_tiles(key(1, 0), &[tid(1, 1)]);
    // Wait until the tile render is running, then leave the image.
    let deadline = Instant::now() + Duration::from_secs(2);
    while first(&shared.calls(), &Call::TileStart(0, 1, 1)).is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    e.set_current(target(1, 1, false), &[]);
    // The new image is not blocked: its work starts as soon as the running call has ended.
    assert!(matches!(
        e.tile(key(1, 1), tid(0, 0), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 1000);
    let calls = shared.calls();
    let old_end = first(&calls, &Call::TileEnd(0, 1, 1)).unwrap();
    let new_open = first(&calls, &Call::Open(1)).unwrap();
    let gap = new_open.saturating_duration_since(old_end);
    assert!(
        gap < Duration::from_millis(30),
        "new image waited {gap:?} after the old call"
    );
    assert!(
        first(&shared.calls(), &Call::TileEnd(0, 1, 1)).is_some(),
        "call ran to the end"
    );
    assert_eq!(e.stats().results_discarded, 1);
    assert_eq!(e.tile(key(1, 0), tid(1, 1), Duration::ZERO), Lookup::Gone);
    assert!(e.stats().tiles == 1, "only the new image's tile is kept");
}

#[test]
fn a_finished_half_size_of_a_left_image_is_discarded() {
    let (b, shared) = fake(10, 200, 10);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[]);
    let deadline = Instant::now() + Duration::from_secs(2);
    while first(&shared.calls(), &Call::HalfStart(0)).is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    e.set_current(target(1, 5, false), &[]);
    settle(&e, &shared, 1500);
    assert_eq!(e.stats().halves, 0);
    assert_eq!(e.stats().results_discarded, 1);
    assert_eq!(e.half(key(1, 0), Duration::ZERO), Lookup::Gone);
}

// ---------- concurrency ----------

#[test]
fn tiles_and_half_size_run_at_the_same_time_and_tiles_are_rendered_once() {
    let (b, shared) = fake(20, 400, 30);
    let e = Arc::new(Engine::new(b, cfg()));
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[]);
    std::thread::sleep(Duration::from_millis(120)); // half-size running
    // Four viewers ask for the same tile at once.
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let e = e.clone();
            std::thread::spawn(move || e.tile(key(1, 0), tid(4, 4), Duration::from_secs(3)))
        })
        .collect();
    for h in handles {
        assert!(matches!(h.join().unwrap(), Lookup::Ready(_)));
    }
    settle(&e, &shared, 2000);
    let calls = shared.calls();
    let renders = calls
        .iter()
        .filter(|(_, c)| *c == Call::TileStart(0, 4, 4))
        .count();
    assert_eq!(renders, 1, "the same tile was rendered more than once");
    let half = (
        first(&calls, &Call::HalfStart(0)).unwrap(),
        first(&calls, &Call::HalfEnd(0)).unwrap(),
    );
    let tile = first(&calls, &Call::TileStart(0, 4, 4)).unwrap();
    assert!(
        half.0 < tile && tile < half.1,
        "the tile waited for the half-size render"
    );
}

#[test]
fn a_failing_file_reports_once_and_is_not_retried() {
    let (b, shared) = fake(10, 10, 10);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    let bad = Target {
        key: key(1, 0),
        path: PathBuf::from("bad.ARW"),
        want_half: true,
    };
    e.set_current(bad.clone(), &[]);
    assert!(matches!(
        e.tile(key(1, 0), tid(0, 0), Duration::from_secs(1)),
        Lookup::Failed(_)
    ));
    assert!(matches!(
        e.half(key(1, 0), Duration::from_secs(1)),
        Lookup::Failed(_)
    ));
    e.set_current(bad, &[]);
    e.request_tiles(key(1, 0), &[tid(1, 1)]);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(e.stats().sessions_kept, 0);
    assert!(
        shared.calls().is_empty(),
        "the simulated decoder never opened it"
    );
}

// ---------- memory budget ----------

#[test]
fn budget_holds_under_fast_switching_and_repeated_zooming() {
    let (mut b, shared) = fake(15, 40, 8);
    b.tile_bytes = 1_000_000;
    let config = Config {
        dwell: Duration::from_millis(20),
        max_sessions: 2,
        max_tile_bytes: 5_000_000, // five tiles
        max_halves: 3,
        ..Config::default()
    };
    let e = Engine::new(b, config.clone());
    e.set_folder(1);
    let mut worst = (0usize, 0usize, 0usize);
    for step in 0..60usize {
        let i = (step * 7) % 23; // jumps back and forth
        e.set_current(
            target(1, i, true),
            &[target(1, i + 1, true), target(1, i.saturating_sub(1), true)],
        );
        if step % 2 == 0 {
            let tiles: Vec<_> = (0..9).map(|n| tid(n % 3, n / 3)).collect();
            e.request_tiles(key(1, i), &tiles);
        }
        std::thread::sleep(Duration::from_millis(5 + (step as u64 * 13) % 70));
        let s = e.stats();
        worst.0 = worst.0.max(s.sessions_kept);
        worst.1 = worst.1.max(s.tile_bytes);
        worst.2 = worst.2.max(s.halves);
        assert!(s.sessions_kept <= config.max_sessions, "{s:?}");
        assert!(s.tile_bytes <= config.max_tile_bytes, "{s:?}");
        assert!(s.halves <= config.max_halves, "{s:?}");
    }
    settle(&e, &shared, 3000);
    let max_alive = shared.max_alive.load(Ordering::SeqCst);
    // Kept sessions + the background worker's temporary one.
    assert!(
        max_alive <= config.max_sessions + 1,
        "{max_alive} sessions alive at once"
    );
    eprintln!(
        "worst: sessions {} tile bytes {} halves {}; max alive {max_alive}; {:?}",
        worst.0,
        worst.1,
        worst.2,
        e.stats()
    );
}

#[test]
fn tile_budget_evicts_least_recently_used() {
    let (mut b, shared) = fake(5, 5, 5);
    b.tile_bytes = 1_000_000;
    let e = Engine::new(
        b,
        Config {
            max_tile_bytes: 3_000_000,
            ..cfg()
        },
    );
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    for c in 0..3 {
        assert!(matches!(
            e.tile(key(1, 0), tid(c, 0), Duration::from_secs(1)),
            Lookup::Ready(_)
        ));
    }
    // Use tile 0 again, then add a fourth: tile 1 (least recently used) goes.
    assert!(matches!(
        e.tile(key(1, 0), tid(0, 0), Duration::ZERO),
        Lookup::Ready(_)
    ));
    assert!(matches!(
        e.tile(key(1, 0), tid(3, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    let s = e.stats();
    assert_eq!((s.tiles, s.tile_bytes, s.tiles_evicted), (3, 3_000_000, 1));
    assert!(matches!(
        e.tile(key(1, 0), tid(0, 0), Duration::ZERO),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 500);
    // Tile 1 is rendered again on demand.
    assert!(matches!(
        e.tile(key(1, 0), tid(1, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
}

#[test]
fn sessions_are_kept_for_two_images_and_freed_when_left_further() {
    let (b, shared) = fake(5, 5, 5);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    for i in 0..5 {
        e.set_current(target(1, i, false), &[]);
        assert!(matches!(
            e.tile(key(1, i), tid(0, 0), Duration::from_secs(1)),
            Lookup::Ready(_)
        ));
        let s = e.stats();
        assert!(s.sessions_kept <= 2, "{s:?}");
        assert!(shared.alive.load(Ordering::SeqCst) <= 2);
    }
    assert_eq!(e.stats().sessions_evicted, 3);
    // Going back to the previous image reuses its kept session (no new open).
    let opens_before = shared
        .calls()
        .iter()
        .filter(|(_, c)| matches!(c, Call::Open(_)))
        .count();
    e.set_current(target(1, 3, false), &[]);
    assert!(matches!(
        e.tile(key(1, 3), tid(1, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    let opens_after = shared
        .calls()
        .iter()
        .filter(|(_, c)| matches!(c, Call::Open(_)))
        .count();
    assert_eq!(opens_before, opens_after);
}

#[test]
fn changing_folder_frees_everything() {
    let (b, shared) = fake(10, 30, 10);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[target(1, 1, true)]);
    e.request_tiles(key(1, 0), &[tid(0, 0), tid(1, 0)]);
    assert!(matches!(
        e.half(key(1, 1), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    assert!(matches!(
        e.tile(key(1, 0), tid(1, 0), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    assert!(e.stats().sessions_kept >= 1 && e.stats().tiles >= 1 && e.stats().halves >= 1);
    e.set_folder(2);
    settle(&e, &shared, 1000);
    let s = e.stats();
    assert_eq!(
        (s.sessions_kept, s.tiles, s.tile_bytes, s.halves, s.queued),
        (0, 0, 0, 0, 0),
        "{s:?}"
    );
    assert_eq!(
        shared.alive.load(Ordering::SeqCst),
        0,
        "a LibRaw session was not closed"
    );
    assert_eq!(e.tile(key(1, 0), tid(0, 0), Duration::ZERO), Lookup::Gone);
}

#[test]
fn folder_change_while_a_call_runs_closes_its_session_afterwards() {
    let (b, shared) = fake(10, 10, 250);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    e.request_tiles(key(1, 0), &[tid(0, 0)]);
    std::thread::sleep(Duration::from_millis(80)); // tile render running
    e.set_folder(2);
    settle(&e, &shared, 1500);
    assert_eq!(shared.alive.load(Ordering::SeqCst), 0);
    assert_eq!(e.stats().sessions_kept, 0);
    assert_eq!(e.stats().results_discarded, 1);
}

#[test]
fn dropping_the_engine_stops_the_workers() {
    let (b, shared) = fake(10, 300, 10);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[]);
    std::thread::sleep(Duration::from_millis(120));
    let t = Instant::now();
    drop(e);
    assert!(
        t.elapsed() < Duration::from_millis(600),
        "drop waited {:?}",
        t.elapsed()
    );
    assert_eq!(shared.alive.load(Ordering::SeqCst), 0);
}

#[test]
fn grid_and_border_tiles() {
    let (b, _shared) = fake(5, 5, 5);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    assert!(matches!(
        e.tile(key(1, 0), tid(9, 6), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    let g = e.grid(key(1, 0)).unwrap();
    assert_eq!((g.cols, g.rows), (10, 7)); // 9568 x 6376 in 1024 tiles
    assert_eq!(
        g.rect(tid(9, 6)),
        Some(Rect {
            x: 9216,
            y: 6144,
            w: 352,
            h: 232
        })
    );
    assert!(matches!(
        e.tile(key(1, 0), tid(10, 0), Duration::from_secs(1)),
        Lookup::Failed(_)
    ));
}

#[test]
fn clearing_the_current_image_stops_new_work_and_keeps_results() {
    let (b, shared) = fake(10, 100, 10);
    let e = Engine::new(b, cfg());
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[target(1, 1, true)]);
    assert!(matches!(
        e.half(key(1, 0), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    e.clear_current();
    settle(&e, &shared, 1000);
    let calls = shared.calls();
    // The neighbour's job may already have been taken by the background worker when we cleared
    // (it is allowed to finish and its result is thrown away); nothing else may run.
    let neighbour_runs = calls
        .iter()
        .filter(|(_, c)| *c == Call::HalfStart(1))
        .count();
    assert!(neighbour_runs <= 1);
    assert_eq!(e.stats().results_discarded, neighbour_runs as u64);
    assert_eq!(e.stats().queued, 0);
    assert_eq!(e.stats().halves, 1, "finished result of image 0 kept");
    // The cached result can still be read, and no new work is queued for an uncached image.
    assert!(matches!(
        e.half(key(1, 0), Duration::ZERO),
        Lookup::Ready(_)
    ));
    assert_eq!(e.half(key(1, 7), Duration::ZERO), Lookup::Gone);
    assert_eq!(e.stats().queued, 0);
    e.set_current(target(1, 0, true), &[]);
    assert!(matches!(
        e.half(key(1, 0), Duration::ZERO),
        Lookup::Ready(_)
    ));
}

#[test]
fn without_unpack_ahead_only_needed_work_opens_files_and_half_reports_full_size() {
    let (b, shared) = fake(10, 20, 5);
    let e = Engine::new(
        b,
        Config {
            unpack_ahead: false,
            ..cfg()
        },
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let ev = events.clone();
    e.set_listener(move |x| ev.lock().unwrap().push(x));
    e.set_folder(1);
    // Dwell on an image whose embedded preview is enough: nothing is opened.
    e.set_current(target(1, 0, false), &[target(1, 1, false)]);
    std::thread::sleep(Duration::from_millis(200));
    settle(&e, &shared, 1000);
    assert!(shared.calls().is_empty(), "{:?}", shared.calls());
    assert_eq!(e.stats().sessions_opened, 0);
    // An image that wants a half-size image: one open (the background worker's own session).
    e.set_current(target(1, 2, true), &[]);
    assert!(matches!(
        e.half(key(1, 2), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 1000);
    let opens = shared
        .calls()
        .iter()
        .filter(|(_, c)| matches!(c, Call::Open(_)))
        .count();
    assert_eq!(opens, 1);
    // The full-size grid is reported before the half-size image.
    let evs = events.lock().unwrap().clone();
    let gi = evs
        .iter()
        .position(|x| matches!(x, pw_rawview::Event::Grid(k, g) if *k == key(1, 2) && g.width == 9568 && g.height == 6376))
        .expect("grid event");
    let hi = evs
        .iter()
        .position(|x| matches!(x, pw_rawview::Event::Half(k) if *k == key(1, 2)))
        .expect("half event");
    assert!(gi < hi);
    assert_eq!(e.grid(key(1, 2)).map(|g| g.width), Some(9568));
    // Tiles still work: the session is opened on demand.
    assert!(matches!(
        e.tile(key(1, 2), tid(0, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
}

// ---------- step 4: unpack on demand and session reuse ----------

fn opens_of(shared: &Shared, i: usize) -> usize {
    shared
        .calls()
        .iter()
        .filter(|(_, c)| *c == Call::Open(i))
        .count()
}

#[test]
fn prepare_unpacks_at_once_and_tiles_use_that_session() {
    let (b, shared) = fake(40, 20, 5);
    let e = Engine::new(
        b,
        Config {
            unpack_ahead: false,
            dwell: Duration::from_millis(400),
            ..Config::default()
        },
    );
    e.set_folder(1);
    e.set_current(target(1, 0, false), &[]);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(opens_of(&shared, 0), 0, "no unpack without a zoom");
    let t0 = Instant::now();
    e.prepare(key(1, 0));
    while e.stats().sessions_kept == 0 || e.stats().running > 0 {
        assert!(
            t0.elapsed() < Duration::from_millis(300),
            "prepare waited for the dwell"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(e.grid(key(1, 0)).is_some(), "grid known after the unpack");
    for c in 0..3 {
        assert!(matches!(
            e.tile(key(1, 0), tid(c, 0), Duration::from_secs(1)),
            Lookup::Ready(_)
        ));
    }
    assert_eq!(opens_of(&shared, 0), 1, "tiles reuse the prepared session");
    // Asking again, or re-sending the same current image, does not unpack again.
    e.prepare(key(1, 0));
    e.set_current(target(1, 0, false), &[]);
    settle(&e, &shared, 1000);
    assert_eq!(opens_of(&shared, 0), 1);
    // A new image: the request does not carry over.
    e.set_current(target(1, 1, false), &[]);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(opens_of(&shared, 1), 0);
}

#[test]
fn the_half_size_session_of_the_current_image_is_kept_for_tiles() {
    let (b, shared) = fake(30, 30, 5);
    let e = Engine::new(
        b,
        Config {
            unpack_ahead: false,
            ..cfg()
        },
    );
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[target(1, 1, true)]);
    assert!(matches!(
        e.half(key(1, 0), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    assert!(matches!(
        e.half(key(1, 1), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 1000);
    let st = e.stats();
    assert_eq!(
        st.sessions_reused, 1,
        "only the current image's session: {st:?}"
    );
    assert_eq!(st.sessions_kept, 1);
    assert_eq!(e.grid(key(1, 0)).map(|g| g.width), Some(9568));
    assert!(matches!(
        e.tile(key(1, 0), tid(4, 3), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    assert_eq!(opens_of(&shared, 0), 1, "the tile did not unpack again");
    // The neighbour's session was closed: going there and zooming opens the file again.
    e.set_current(target(1, 1, true), &[]);
    assert!(matches!(
        e.tile(key(1, 1), tid(0, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    assert_eq!(opens_of(&shared, 1), 2);
    assert!(e.stats().sessions_kept <= 2);
}

#[test]
fn without_reuse_the_first_tile_unpacks_again() {
    let (b, shared) = fake(30, 30, 5);
    let e = Engine::new(
        b,
        Config {
            unpack_ahead: false,
            reuse_half_session: false,
            ..cfg()
        },
    );
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[]);
    assert!(matches!(
        e.half(key(1, 0), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 1000);
    assert_eq!(e.stats().sessions_kept, 0);
    assert!(matches!(
        e.tile(key(1, 0), tid(0, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    assert_eq!(opens_of(&shared, 0), 2);
    assert_eq!(e.stats().sessions_reused, 0);
}

#[test]
fn a_zoom_during_the_half_size_render_does_not_wait_for_it() {
    // Half-size render is slow (300 ms); a tile asked for meanwhile opens its own session and is
    // ready long before the half-size job ends. The half-size session is then closed, not kept.
    let (b, shared) = fake(20, 300, 5);
    let e = Engine::new(
        b,
        Config {
            unpack_ahead: false,
            ..cfg()
        },
    );
    e.set_folder(1);
    e.set_current(target(1, 0, true), &[]);
    std::thread::sleep(Duration::from_millis(120)); // dwell 60 ms + open: half is rendering
    let t0 = Instant::now();
    assert!(matches!(
        e.tile(key(1, 0), tid(0, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    assert!(
        t0.elapsed() < Duration::from_millis(150),
        "{:?}",
        t0.elapsed()
    );
    settle(&e, &shared, 2000);
    let st = e.stats();
    assert_eq!(st.sessions_reused, 0, "{st:?}");
    assert_eq!(st.sessions_kept, 1);
}

#[test]
fn rapid_zoom_and_pan_keeps_budget_and_priority() {
    // Viewports change every few ms; the newest request always comes first and the budgets hold.
    let (mut b, shared) = fake(15, 40, 6);
    b.tile_bytes = 1_000_000;
    let config = Config {
        unpack_ahead: false,
        max_tile_bytes: 8_000_000,
        ..cfg()
    };
    let e = Engine::new(b, config.clone());
    e.set_folder(1);
    for img in 0..4usize {
        e.set_current(target(1, img, true), &[target(1, img + 1, true)]);
        e.prepare(key(1, img));
        for step in 0..40u32 {
            let c0 = step % 8;
            let tiles: Vec<_> = (0..6)
                .map(|n| tid((c0 + n % 3) % 10, (step / 8 + n / 3) % 7))
                .collect();
            e.request_tiles(key(1, img), &tiles);
            std::thread::sleep(Duration::from_millis(2 + u64::from(step % 5) * 3));
            let s = e.stats();
            assert!(s.sessions_kept <= config.max_sessions, "{s:?}");
            assert!(s.tile_bytes <= config.max_tile_bytes, "{s:?}");
            assert!(s.halves <= config.max_halves, "{s:?}");
        }
        // The last viewport is completed, starting with its first tile.
        let last: Vec<_> = (0..6)
            .map(|n| tid((7 + n % 3) % 10, (4 + n / 3) % 7))
            .collect();
        e.request_tiles(key(1, img), &last);
        let since = Instant::now();
        for t in &last {
            assert!(matches!(
                e.tile(key(1, img), *t, Duration::from_secs(2)),
                Lookup::Ready(_)
            ));
        }
        assert!(since.elapsed() < Duration::from_millis(500));
    }
    settle(&e, &shared, 3000);
    let max_alive = shared.max_alive.load(Ordering::SeqCst);
    assert!(
        max_alive <= config.max_sessions + 1,
        "{max_alive} sessions alive"
    );
}

// ---------- error handling: a failing (damaged) file and later images ----------

#[test]
fn a_late_failure_of_a_left_image_does_not_touch_the_current_one() {
    // The damaged file takes a while to fail (unpack reads to the end first).
    let (mut b, shared) = fake(10, 20, 5);
    b.open_ms = 10;
    let e = Engine::new(
        b,
        Config {
            unpack_ahead: false,
            ..cfg()
        },
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let ev = events.clone();
    e.set_listener(move |x| ev.lock().unwrap().push(x));
    e.set_folder(1);
    let bad = Target {
        key: key(1, 5),
        path: PathBuf::from("bad5.ARW"),
        want_half: true,
    };
    e.set_current(bad.clone(), &[target(1, 6, true)]);
    std::thread::sleep(Duration::from_millis(80)); // dwell passed: the half-size job runs and fails
    e.set_current(target(1, 6, true), &[bad.clone()]);
    assert!(matches!(
        e.half(key(1, 6), Duration::from_secs(2)),
        Lookup::Ready(_)
    ));
    assert!(matches!(
        e.tile(key(1, 6), tid(0, 0), Duration::from_secs(1)),
        Lookup::Ready(_)
    ));
    settle(&e, &shared, 1000);
    let evs = events.lock().unwrap().clone();
    assert!(
        evs.iter()
            .all(|x| !matches!(x, pw_rawview::Event::Failed(k, _) if *k == key(1, 6))),
        "the good image never fails: {evs:?}"
    );
    assert!(
        evs.iter()
            .any(|x| matches!(x, pw_rawview::Event::Failed(k, _) if *k == key(1, 5)))
    );
    // Going back to the damaged file: it is not tried again, and asks for it answer Failed.
    let opens_before = shared.calls().len();
    e.set_current(bad, &[]);
    assert!(matches!(
        e.half(key(1, 5), Duration::from_millis(200)),
        Lookup::Failed(_)
    ));
    assert!(matches!(
        e.tile(key(1, 5), tid(0, 0), Duration::from_millis(200)),
        Lookup::Failed(_)
    ));
    settle(&e, &shared, 500);
    assert_eq!(
        shared.calls().len(),
        opens_before,
        "no new attempt on the damaged file"
    );
}
