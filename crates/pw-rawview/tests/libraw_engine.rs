//! The engine with the real decoder (LibRaw + JPEG) on the test RAWs, on Windows:
//! `$env:PW_TESTDATA = "D:\PhotoWorkflowData\RAW"; cargo test --release -p pw-rawview --features libraw -- --ignored --nocapture --test-threads=1`
//!
//! Measures the time to the first tile (with and without the unpack done ahead) and checks that
//! process memory stays within the budget during fast switching with zooming, and comes back down
//! after a folder change.
#![cfg(all(feature = "libraw", windows))]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pw_rawview::libraw::LibRawBackend;
use pw_rawview::{Config, Engine, ImageKey, Lookup, Target, TileId};

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

fn raws() -> Vec<PathBuf> {
    let dir = testdata();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
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
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        matches!(ext.to_ascii_lowercase().as_str(), "arw" | "rw2" | "dng")
                    })
        })
        .collect();
    v.sort();
    assert!(
        !v.is_empty(),
        "no RAW test files were found in PW_TESTDATA folder {}",
        dir.display()
    );
    v
}

fn a7r_iva_index(files: &[PathBuf]) -> usize {
    files
        .iter()
        .position(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("TN1004"))
        })
        .unwrap_or_else(|| {
            panic!(
                "no A7R IVA test file with prefix TN1004 was found in PW_TESTDATA folder {}",
                testdata().display()
            )
        })
}

#[repr(C)]
#[derive(Default)]
struct MemCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged_pool: usize,
    quota_paged_pool: usize,
    quota_peak_non_paged_pool: usize,
    quota_non_paged_pool: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, m: *mut MemCounters, cb: u32) -> i32;
}

/// Private bytes (commit) of this process, in MB.
fn private_mb() -> f64 {
    let mut m = MemCounters {
        cb: std::mem::size_of::<MemCounters>() as u32,
        ..Default::default()
    };
    // SAFETY: pseudo-handle and a correctly sized struct.
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) };
    m.pagefile_usage as f64 / (1024.0 * 1024.0)
}

fn key(g: u64, i: usize) -> ImageKey {
    ImageKey {
        generation: g,
        index: i,
    }
}

fn target(g: u64, i: usize, files: &[PathBuf]) -> Target {
    Target {
        key: key(g, i),
        path: files[i % files.len()].clone(),
        want_half: true,
    }
}

fn wait_idle(e: &Engine<LibRawBackend>, max: Duration) {
    let deadline = Instant::now() + max;
    while Instant::now() < deadline {
        let s = e.stats();
        if s.queued == 0 && s.running == 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn first_tile_and_half_size_with_libraw() {
    let files = raws();
    let a7r = a7r_iva_index(&files);
    let e = Engine::new(LibRawBackend, Config::default());
    e.set_folder(1);
    // Cold: zoom right away (open + unpack + tile).
    e.set_current(target(1, a7r, &files), &[]);
    let t = Instant::now();
    let Lookup::Ready(tile) = e.tile(
        key(1, a7r),
        TileId { col: 4, row: 3 },
        Duration::from_secs(10),
    ) else {
        panic!("tile not ready");
    };
    let cold = t.elapsed();
    assert_eq!(&tile[..2], &[0xFF, 0xD8], "JPEG");
    // Next tile from the kept session.
    let t = Instant::now();
    assert!(matches!(
        e.tile(
            key(1, a7r),
            TileId { col: 5, row: 3 },
            Duration::from_secs(10)
        ),
        Lookup::Ready(_)
    ));
    let warm = t.elapsed();
    let t = Instant::now();
    let Lookup::Ready(half) = e.half(key(1, a7r), Duration::from_secs(20)) else {
        panic!("half not ready");
    };
    let half_t = t.elapsed();
    // Unpack ahead: show another image, let the dwell + unpack happen, then zoom.
    let other = (a7r + 1) % files.len();
    e.set_current(target(1, other, &files), &[]);
    wait_idle(&e, Duration::from_secs(10));
    let t = Instant::now();
    assert!(matches!(
        e.tile(
            key(1, other),
            TileId { col: 1, row: 1 },
            Duration::from_secs(10)
        ),
        Lookup::Ready(_)
    ));
    let preunpacked = t.elapsed();
    eprintln!(
        "A7R IVA: first tile cold {cold:?}, next tile {warm:?}, half-size {half_t:?} ({} KB JPEG, \
         tile {} KB); first tile after unpack-ahead ({}): {preunpacked:?}",
        half.len() / 1024,
        tile.len() / 1024,
        files[other].file_name().unwrap().to_string_lossy()
    );
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn memory_stays_within_budget_and_is_freed() {
    let files = raws();
    assert!(
        files.len() >= 2,
        "at least two RAW test files are required in PW_TESTDATA folder {}",
        testdata().display()
    );
    let base = private_mb();
    let e = Engine::new(LibRawBackend, Config::default());
    e.set_folder(1);
    let mut peak = base;
    let start = Instant::now();
    // 40 images (the test files repeated under different keys), back and forth, zooming on
    // every other one; the pauses vary from "flicking" to "looking".
    for step in 0..40usize {
        let i = (step * 7) % 25;
        e.set_current(
            target(1, i, &files),
            &[
                target(1, i + 1, &files),
                target(1, i.saturating_sub(1), &files),
            ],
        );
        if step % 2 == 0 {
            let tiles: Vec<_> = (0..6)
                .map(|n| TileId {
                    col: 2 + n % 3,
                    row: 2 + n / 3,
                })
                .collect();
            e.request_tiles(key(1, i), &tiles);
        }
        let pause = [20u64, 80, 250, 600][step % 4];
        let until = Instant::now() + Duration::from_millis(pause);
        while Instant::now() < until {
            peak = peak.max(private_mb());
            let s = e.stats();
            assert!(s.sessions_kept <= 2, "{s:?}");
            assert!(s.tile_bytes <= 192 * 1024 * 1024, "{s:?}");
            assert!(s.halves <= 6, "{s:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    wait_idle(&e, Duration::from_secs(30));
    peak = peak.max(private_mb());
    let loaded = private_mb();
    let stats = e.stats();
    e.set_folder(2);
    wait_idle(&e, Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(300));
    let after = private_mb();
    eprintln!(
        "memory (private MB): before {base:.0}, peak {peak:.0} (+{:.0}), with caches {loaded:.0}, \
         after folder change {after:.0}; {:.1} s; {stats:?}",
        peak - base,
        start.elapsed().as_secs_f64()
    );
    // Budget: 2 kept sessions + 1 temporary (≈ 3 × 130 MB) + LibRaw working memory of one tile
    // and one half-size render (≈ 210 + 290 MB) + 192 MB tiles + 6 half-size JPEGs.
    assert!(
        peak - base < 1300.0,
        "peak {peak:.0} MB over a base of {base:.0} MB"
    );
    assert!(
        after - base < 120.0,
        "{after:.0} MB after the folder change (base {base:.0} MB)"
    );
}

/// Step 4: the app's configuration (no unpack ahead). The first tile after the half-size image is
/// shown, with the half-size job's session reused and without; and on the first zoom action
/// (`prepare`) without a half-size image. Also the memory a kept session costs.
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn step4_first_tile_with_and_without_session_reuse() {
    let files = raws();
    let a7r = a7r_iva_index(&files);
    for reuse in [true, false] {
        let base = private_mb();
        let e = Engine::new(
            LibRawBackend,
            Config {
                unpack_ahead: false,
                reuse_half_session: reuse,
                ..Config::default()
            },
        );
        e.set_folder(1);
        e.set_current(target(1, a7r, &files), &[]);
        assert!(matches!(
            e.half(key(1, a7r), Duration::from_secs(20)),
            Lookup::Ready(_)
        ));
        wait_idle(&e, Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(200));
        let kept_mb = private_mb() - base;
        let t = Instant::now();
        assert!(matches!(
            e.tile(
                key(1, a7r),
                TileId { col: 4, row: 3 },
                Duration::from_secs(10)
            ),
            Lookup::Ready(_)
        ));
        let first = t.elapsed();
        let t = Instant::now();
        let ids: Vec<_> = (0..6)
            .map(|n| TileId {
                col: 3 + n % 3,
                row: 2 + n / 3,
            })
            .collect();
        for id in &ids {
            assert!(matches!(
                e.tile(key(1, a7r), *id, Duration::from_secs(10)),
                Lookup::Ready(_)
            ));
        }
        let six = t.elapsed();
        eprintln!(
            "A7R IVA, reuse {reuse}: after half-size, memory held {kept_mb:.0} MB, first tile \
             {first:?}, 5 more tiles {six:?}; {:?}",
            e.stats()
        );
    }
    // No half-size image (like the RW2 on a 1920 px screen): prepare on the first zoom action.
    let rw2 = files
        .iter()
        .position(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("rw2")))
        .unwrap_or(a7r);
    let e = Engine::new(
        LibRawBackend,
        Config {
            unpack_ahead: false,
            ..Config::default()
        },
    );
    e.set_folder(1);
    let mut t = target(1, rw2, &files);
    t.want_half = false;
    e.set_current(t.clone(), &[]);
    let t0 = Instant::now();
    e.prepare(key(1, rw2));
    while e.grid(key(1, rw2)).is_none() && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(2));
    }
    let unpack = t0.elapsed();
    let t1 = Instant::now();
    assert!(matches!(
        e.tile(
            key(1, rw2),
            TileId { col: 1, row: 1 },
            Duration::from_secs(10)
        ),
        Lookup::Ready(_)
    ));
    eprintln!(
        "{}: unpack after prepare {unpack:?}, then first tile {:?}",
        files[rw2].file_name().unwrap().to_string_lossy(),
        t1.elapsed()
    );
}
