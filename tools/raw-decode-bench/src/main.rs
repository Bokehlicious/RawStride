//! `raw-decode-bench` – LibRaw decode time, memory use and parallel throughput.
//!
//! Development tool for milestone 2, not application code. Every measurement runs in a fresh
//! child process, so that peak memory belongs to exactly one configuration.
//!
//! ```text
//! raw-decode-bench single   <file> [<file> ...] [--reps N] [--out results.csv]
//! raw-decode-bench parallel <file> [<file> ...] [--threads 1,2,4,8] [--config full_ahd] [--out results.csv]
//! raw-decode-bench tiles    <file> [<file> ...] [--reps N] [--out results.csv]
//! raw-decode-bench ptiles   <file> [--threads 1,2,4] [--out results.csv]
//! ```
//!
//! `single` runs, for each file: unpack only, full decode (linear and AHD demosaic),
//! half-size decode, and a centred 2048 × 2048 crop (one "tile") with AHD.
//! `parallel` decodes N files at the same time (one configuration) and reports throughput.
//! `tiles` opens and unpacks a file **once** and then processes 6 regions at different places,
//! then the first region again (to check that repeated processing gives identical pixels), for
//! several region sizes and both demosaics. "First visible" = open + unpack + first region.
//! `ptiles` splits one 2560 × 1440 viewport (AHD) into N strips decoded by N independent
//! LibRaw instances at the same time (each with its own unpack).

use std::ffi::{CStr, OsStr, c_char, c_int, c_uint};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct PwStats {
    open_ms: f64,
    unpack_ms: f64,
    process_ms: f64,
    mkimg_ms: f64,
    total_ms: f64,
    raw_width: c_int,
    raw_height: c_int,
    width: c_int,
    height: c_int,
    out_width: c_int,
    out_height: c_int,
    ok: c_int,
    error: [c_char; 160],
}

impl Default for PwStats {
    fn default() -> Self {
        // SAFETY: PwStats is plain old data; all-zero is a valid value.
        unsafe { std::mem::zeroed() }
    }
}

const PW_UNPACK_ONLY: c_int = 0;
const PW_FULL: c_int = 1;
const PW_HALF: c_int = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SessionInfo {
    open_ms: f64,
    unpack_ms: f64,
    width: c_int,
    height: c_int,
    error: [c_char; 160],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct TileStats {
    process_ms: f64,
    mkimg_ms: f64,
    out_width: c_int,
    out_height: c_int,
    hash: u64,
    error: [c_char; 160],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SeamResult {
    margin: c_int,
    out_w: c_int,
    out_h: c_int,
    dx: c_int,
    dy: c_int,
    diff_pct: f64,
    max_diff: c_int,
    mean_abs: f64,
    band_max: c_int,
    interior_max: c_int,
    process_ms: f64,
    hash: u64,
    error: [c_char; 160],
}

unsafe extern "C" {
    fn pw_session_open(
        path: *const u16,
        quality: c_int,
        info: *mut SessionInfo,
    ) -> *mut std::ffi::c_void;
    fn pw_session_tile(
        session: *mut std::ffi::c_void,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        st: *mut TileStats,
    ) -> c_int;
    fn pw_session_close(session: *mut std::ffi::c_void);
    fn pw_decode_hash(
        path: *const u16,
        quality: c_int,
        half: c_int,
        auto_bright: c_int,
        hash: *mut u64,
        w: *mut c_int,
        h: *mut c_int,
        error: *mut c_char,
    ) -> c_int;
    fn pw_set_adjust_maximum_thr(thr: f32);
    fn pw_probe_info(
        path: *const u16,
        filters: *mut c_uint,
        top: *mut c_int,
        left: *mut c_int,
        w: *mut c_int,
        h: *mut c_int,
        raw_w: *mut c_int,
        raw_h: *mut c_int,
        error: *mut c_char,
    ) -> c_int;
    fn pw_seam_check(
        path: *const u16,
        auto_bright: c_int,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        margins: *const c_int,
        n: c_int,
        out: *mut SeamResult,
        full_hash: *mut u64,
        error: *mut c_char,
    ) -> c_int;
    fn pw_libraw_version() -> *const c_char;
    fn pw_libraw_capabilities() -> c_uint;
    fn pw_decode(
        path: *const u16,
        mode: c_int,
        quality: c_int,
        crop: c_int,
        st: *mut PwStats,
    ) -> c_int;
}

/// A named decode configuration.
#[derive(Debug, Clone, Copy)]
struct Config {
    name: &'static str,
    mode: c_int,
    quality: c_int,
    crop: c_int,
}

const CONFIGS: &[Config] = &[
    Config {
        name: "unpack_only",
        mode: PW_UNPACK_ONLY,
        quality: 0,
        crop: 0,
    },
    Config {
        name: "half_size",
        mode: PW_HALF,
        quality: 0,
        crop: 0,
    },
    Config {
        name: "full_linear",
        mode: PW_FULL,
        quality: 0,
        crop: 0,
    },
    Config {
        name: "full_ahd",
        mode: PW_FULL,
        quality: 3,
        crop: 0,
    },
    Config {
        name: "crop2048_ahd",
        mode: PW_FULL,
        quality: 3,
        crop: 2048,
    },
];

fn config(name: &str) -> Option<Config> {
    CONFIGS.iter().copied().find(|c| c.name == name)
}

fn wide(path: &Path) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }
}

fn decode(path: &Path, c: Config) -> Result<PwStats, String> {
    let w = wide(path);
    let mut st = PwStats::default();
    // SAFETY: w is NUL-terminated UTF-16; st is a valid out-pointer.
    let ok = unsafe { pw_decode(w.as_ptr(), c.mode, c.quality, c.crop, &mut st) };
    if ok == 0 {
        // SAFETY: the shim always NUL-terminates `error`.
        let msg = unsafe { CStr::from_ptr(st.error.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        return Err(format!("{}: {msg}", path.display()));
    }
    Ok(st)
}

// ---------- tile sessions ----------

/// One LibRaw instance after open + unpack.
struct Session {
    ptr: *mut std::ffi::c_void,
    info: SessionInfo,
}

// SAFETY: a session is used by one thread at a time (moved, never shared); LibRaw's `raw_r`
// keeps all state in the instance.
unsafe impl Send for Session {}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("info", &self.info).finish()
    }
}

fn c_err(e: &[c_char; 160]) -> String {
    // SAFETY: the shim NUL-terminates (the buffer is zeroed and written with strncpy len-1).
    unsafe { CStr::from_ptr(e.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

impl Session {
    fn open(path: &Path, quality: c_int) -> Result<Session, String> {
        let w = wide(path);
        // SAFETY: plain old data; zero is valid.
        let mut info: SessionInfo = unsafe { std::mem::zeroed() };
        // SAFETY: NUL-terminated path, valid out-pointer.
        let ptr = unsafe { pw_session_open(w.as_ptr(), quality, &mut info) };
        if ptr.is_null() {
            return Err(format!("{}: {}", path.display(), c_err(&info.error)));
        }
        Ok(Session { ptr, info })
    }

    /// Processes region (x, y, w, h), clamped to the image.
    fn tile(&mut self, x: i32, y: i32, w: i32, h: i32) -> Result<TileStats, String> {
        let w = w.min(self.info.width);
        let h = h.min(self.info.height);
        let x = x.clamp(0, self.info.width - w);
        let y = y.clamp(0, self.info.height - h);
        // SAFETY: plain old data; zero is valid.
        let mut st: TileStats = unsafe { std::mem::zeroed() };
        // SAFETY: live session, valid out-pointer.
        let ok = unsafe { pw_session_tile(self.ptr, x, y, w, h, &mut st) };
        if ok == 0 {
            return Err(c_err(&st.error));
        }
        Ok(st)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: the pointer came from pw_session_open and is closed once.
        unsafe { pw_session_close(self.ptr) };
    }
}

/// Region sizes measured by `tiles` (the last two are a 100 % viewport on QHD and 4K screens).
const TILE_SIZES: &[(i32, i32)] = &[
    (512, 512),
    (1024, 1024),
    (2048, 2048),
    (2560, 1440),
    (3840, 2160),
];
/// Region centres (fractions of the image), in processing order.
const TILE_CENTRES: &[(f64, f64)] = &[
    (0.5, 0.5),
    (0.25, 0.25),
    (0.75, 0.25),
    (0.25, 0.75),
    (0.75, 0.75),
    (0.5, 0.2),
];

/// `--child-tiles <quality> <w> <h> <reps> <file>`
fn child_tiles(quality: c_int, w: i32, h: i32, reps: usize, file: &Path) -> Result<(), String> {
    for run in 0..reps {
        let mut s = Session::open(file, quality)?;
        println!("SES {run} {:.2} {:.2}", s.info.open_ms, s.info.unpack_ms);
        let (iw, ih) = (f64::from(s.info.width), f64::from(s.info.height));
        let mut first_hash = 0;
        for (i, (cx, cy)) in TILE_CENTRES.iter().chain(&TILE_CENTRES[..1]).enumerate() {
            let x = (cx * iw) as i32 - w / 2;
            let y = (cy * ih) as i32 - h / 2;
            let t = s.tile(x, y, w, h)?;
            if i == 0 {
                first_hash = t.hash;
            }
            let repeat = i == TILE_CENTRES.len();
            println!(
                "TILE {run} {i} {:.2} {:.2} {} {} {}",
                t.process_ms,
                t.mkimg_ms,
                t.out_width,
                t.out_height,
                if repeat {
                    u8::from(t.hash == first_hash)
                } else {
                    2
                }
            );
        }
    }
    let (ws, private) = sys::peak_memory();
    println!("MEM {ws} {private}");
    Ok(())
}

/// `--child-ptiles <n> <file>`: one 2560 × 1440 AHD viewport split into n vertical strips,
/// each decoded by its own LibRaw instance (own open + unpack), all at the same time.
fn child_ptiles(n: usize, file: &Path) -> Result<(), String> {
    std::fs::read(file).map_err(|e| e.to_string())?;
    let (vw, vh) = (2560i32, 1440i32);
    let cpu0 = sys::cpu_time();
    let t0 = Instant::now();
    let results: Vec<Result<f64, String>> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..n)
            .map(|i| {
                sc.spawn(move || -> Result<f64, String> {
                    let mut s = Session::open(file, 3)?;
                    let (iw, ih) = (s.info.width, s.info.height);
                    let x0 = iw / 2 - vw / 2;
                    let y0 = ih / 2 - vh / 2;
                    let sw = vw / n as i32;
                    s.tile(x0 + sw * i as i32, y0, sw, vh)?;
                    Ok(t0.elapsed().as_secs_f64() * 1000.0)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("panic".into())))
            .collect()
    });
    let wall = t0.elapsed().as_secs_f64() * 1000.0;
    let cpu = (sys::cpu_time() - cpu0).as_secs_f64() * 1000.0;
    let mut first = f64::MAX;
    for r in results {
        first = first.min(r?);
    }
    let (ws, private) = sys::peak_memory();
    println!("PT {n} {wall:.1} {first:.1} {cpu:.1} {ws} {private}");
    Ok(())
}

// ---------- process metrics (Windows) ----------

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    use std::time::Duration;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
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
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessTimes(
            p: *mut c_void,
            c: *mut FileTime,
            e: *mut FileTime,
            k: *mut FileTime,
            u: *mut FileTime,
        ) -> i32;
        fn K32GetProcessMemoryInfo(p: *mut c_void, m: *mut MemCounters, cb: u32) -> i32;
    }

    /// Total CPU time of this process (all threads).
    pub fn cpu_time() -> Duration {
        let (mut c, mut e, mut k, mut u) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        // SAFETY: pseudo-handle and valid out-pointers.
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
        let t = |f: FileTime| (u64::from(f.high) << 32) | u64::from(f.low);
        Duration::from_nanos((t(k) + t(u)) * 100)
    }

    /// (peak working set, peak private commit) in bytes.
    pub fn peak_memory() -> (u64, u64) {
        let mut m = MemCounters {
            cb: std::mem::size_of::<MemCounters>() as u32,
            ..Default::default()
        };
        // SAFETY: m is sized and initialised as the API requires.
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) };
        (m.peak_working_set as u64, m.peak_pagefile_usage as u64)
    }
}

#[cfg(not(windows))]
mod sys {
    use std::time::Duration;
    pub fn cpu_time() -> Duration {
        Duration::ZERO
    }
    pub fn peak_memory() -> (u64, u64) {
        (0, 0)
    }
}

// ---------- child processes ----------

/// `--child-single <config> <reps> <file>`: one configuration, `reps` runs, then peak memory.
fn child_single(cfg: &str, reps: usize, file: &Path) -> Result<(), String> {
    let c = config(cfg).ok_or("unknown config")?;
    for run in 0..reps {
        let cpu0 = sys::cpu_time();
        let st = decode(file, c)?;
        let cpu = (sys::cpu_time() - cpu0).as_secs_f64() * 1000.0;
        println!(
            "RUN {run} {:.2} {:.2} {:.2} {:.2} {:.2} {} {} {} {} {} {} {:.1}",
            st.open_ms,
            st.unpack_ms,
            st.process_ms,
            st.mkimg_ms,
            st.total_ms,
            st.raw_width,
            st.raw_height,
            st.width,
            st.height,
            st.out_width,
            st.out_height,
            cpu
        );
    }
    let (ws, private) = sys::peak_memory();
    println!("MEM {ws} {private}");
    Ok(())
}

/// `--child-parallel <config> <n> <file>...`: decode n files concurrently, one batch.
fn child_parallel(cfg: &str, n: usize, files: &[PathBuf]) -> Result<(), String> {
    let c = config(cfg).ok_or("unknown config")?;
    // Warm the OS file cache so all thread counts read from memory, not from disk.
    for f in files {
        std::fs::read(f).map_err(|e| e.to_string())?;
    }
    let cpu0 = sys::cpu_time();
    let t0 = Instant::now();
    let results: Vec<Result<PwStats, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..n)
            .map(|i| {
                let f = files[i % files.len()].clone();
                s.spawn(move || decode(&f, c))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("panic".into())))
            .collect()
    });
    let wall = t0.elapsed().as_secs_f64() * 1000.0;
    let cpu = (sys::cpu_time() - cpu0).as_secs_f64() * 1000.0;
    let mut per = Vec::new();
    for r in results {
        per.push(r?.total_ms);
    }
    let mean = per.iter().sum::<f64>() / per.len() as f64;
    let (ws, private) = sys::peak_memory();
    println!("PAR {n} {wall:.1} {mean:.1} {cpu:.1} {ws} {private}");
    Ok(())
}

// ---------- parent ----------

fn run_child(args: &[&OsStr]) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = Command::new(exe)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        return f64::NAN;
    }
    let m = v.len() / 2;
    if v.len() % 2 == 1 {
        v[m]
    } else {
        (v[m - 1] + v[m]) / 2.0
    }
}

fn mb(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

fn parent_single(files: &[PathBuf], reps: usize, csv: &mut impl Write) -> Result<(), String> {
    writeln!(csv, "kind,file,config,run,open_ms,unpack_ms,process_ms,mkimg_ms,total_ms,raw_w,raw_h,w,h,out_w,out_h,cpu_ms,peak_ws_mb,peak_private_mb")
        .map_err(|e| e.to_string())?;
    println!(
        "\n{:<14} {:<13} {:>8} {:>8} {:>9} {:>8} {:>9} {:>11} {:>6} {:>9} {:>9}",
        "file",
        "config",
        "open",
        "unpack",
        "process",
        "mkimg",
        "TOTAL",
        "output",
        "cpu/w",
        "peak WS",
        "peak priv"
    );
    for f in files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Read the file once so every configuration starts with a warm OS cache.
        std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
        for c in CONFIGS {
            let reps_s = reps.to_string();
            let out = run_child(&[
                OsStr::new("--child-single"),
                OsStr::new(c.name),
                OsStr::new(&reps_s),
                f.as_os_str(),
            ])?;
            let mut runs: Vec<Vec<f64>> = Vec::new();
            let (mut ws, mut private) = (0u64, 0u64);
            for line in out.lines() {
                let p: Vec<&str> = line.split_whitespace().collect();
                match p.first() {
                    Some(&"RUN") => runs.push(
                        p[2..]
                            .iter()
                            .map(|x| x.parse().unwrap_or(f64::NAN))
                            .collect(),
                    ),
                    Some(&"MEM") => {
                        ws = p[1].parse().unwrap_or(0);
                        private = p[2].parse().unwrap_or(0);
                    }
                    _ => {}
                }
            }
            for (i, r) in runs.iter().enumerate() {
                writeln!(
                    csv,
                    "single,{name},{},{i},{},{:.1},{:.1}",
                    c.name,
                    r.iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                    mb(ws),
                    mb(private)
                )
                .map_err(|e| e.to_string())?;
            }
            let col = |k: usize| median(runs.iter().map(|r| r[k]).collect());
            let (total, cpu) = (col(4), col(11));
            println!(
                "{:<14} {:<13} {:>8.1} {:>8.1} {:>9.1} {:>8.1} {:>9.1} {:>11} {:>6.2} {:>7.0}MB {:>7.0}MB",
                name,
                c.name,
                col(0),
                col(1),
                col(2),
                col(3),
                total,
                if col(9) > 0.0 {
                    format!("{}x{}", col(9), col(10))
                } else {
                    "-".into()
                },
                cpu / total,
                mb(ws),
                mb(private)
            );
        }
    }
    Ok(())
}

fn parent_parallel(
    files: &[PathBuf],
    threads: &[usize],
    cfg: &str,
    csv: &mut impl Write,
) -> Result<(), String> {
    config(cfg).ok_or("unknown --config")?;
    writeln!(
        csv,
        "kind,config,threads,wall_ms,mean_decode_ms,cpu_ms,images_per_s,peak_ws_mb,peak_private_mb"
    )
    .map_err(|e| e.to_string())?;
    println!("\nconfig {cfg}");
    println!(
        "{:>7} {:>10} {:>14} {:>9} {:>10} {:>9} {:>9}",
        "threads", "wall ms", "mean decode ms", "cpu/wall", "images/s", "peak WS", "peak priv"
    );
    for &n in threads {
        let n_s = n.to_string();
        let mut args: Vec<&OsStr> = vec![
            OsStr::new("--child-parallel"),
            OsStr::new(cfg),
            OsStr::new(&n_s),
        ];
        args.extend(files.iter().map(|f| f.as_os_str()));
        let out = run_child(&args)?;
        let line = out
            .lines()
            .find(|l| l.starts_with("PAR"))
            .ok_or("no PAR line")?;
        let p: Vec<f64> = line
            .split_whitespace()
            .skip(1)
            .map(|x| x.parse().unwrap_or(f64::NAN))
            .collect();
        let (wall, mean, cpu, ws, private) = (p[1], p[2], p[3], p[4] as u64, p[5] as u64);
        let ips = n as f64 / (wall / 1000.0);
        writeln!(
            csv,
            "parallel,{cfg},{n},{wall:.1},{mean:.1},{cpu:.1},{ips:.2},{:.1},{:.1}",
            mb(ws),
            mb(private)
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{n:>7} {wall:>10.1} {mean:>14.1} {:>9.2} {ips:>10.2} {:>7.0}MB {:>7.0}MB",
            cpu / wall,
            mb(ws),
            mb(private)
        );
    }
    Ok(())
}

fn parent_tiles(files: &[PathBuf], reps: usize, csv: &mut impl Write) -> Result<(), String> {
    writeln!(csv, "kind,file,demosaic,tile_w,tile_h,run,open_ms,unpack_ms,tile,process_ms,mkimg_ms,out_w,out_h,repeat_identical,peak_ws_mb,peak_private_mb")
        .map_err(|e| e.to_string())?;
    println!(
        "\n{:<14} {:<7} {:>10} {:>8} {:>13} {:>12} {:>12} {:>9} {:>9} {:>9}",
        "file",
        "demosaic",
        "region",
        "unpack",
        "FIRST VISIBLE",
        "next region",
        "7 regions",
        "identical",
        "peak WS",
        "peak priv"
    );
    for f in files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
        for (q, qname) in [(0, "linear"), (3, "AHD")] {
            for &(w, h) in TILE_SIZES {
                let (q_s, w_s, h_s, r_s) = (
                    q.to_string(),
                    w.to_string(),
                    h.to_string(),
                    reps.to_string(),
                );
                let out = run_child(&[
                    OsStr::new("--child-tiles"),
                    OsStr::new(&q_s),
                    OsStr::new(&w_s),
                    OsStr::new(&h_s),
                    OsStr::new(&r_s),
                    f.as_os_str(),
                ])?;
                let mut ses: Vec<(f64, f64)> = Vec::new();
                let mut tiles: Vec<(usize, usize, f64, f64, i64, i64, u8)> = Vec::new();
                let (mut ws, mut private) = (0u64, 0u64);
                for line in out.lines() {
                    let p: Vec<&str> = line.split_whitespace().collect();
                    let num = |i: usize| p[i].parse::<f64>().unwrap_or(f64::NAN);
                    match p.first() {
                        Some(&"SES") => ses.push((num(2), num(3))),
                        Some(&"TILE") => tiles.push((
                            p[1].parse().unwrap_or(0),
                            p[2].parse().unwrap_or(0),
                            num(3),
                            num(4),
                            p[5].parse().unwrap_or(0),
                            p[6].parse().unwrap_or(0),
                            p[7].parse().unwrap_or(2),
                        )),
                        Some(&"MEM") => {
                            ws = p[1].parse().unwrap_or(0);
                            private = p[2].parse().unwrap_or(0);
                        }
                        _ => {}
                    }
                }
                for &(run, i, pm, mm, ow, oh, same) in &tiles {
                    let (o, u) = ses.get(run).copied().unwrap_or((f64::NAN, f64::NAN));
                    writeln!(
                        csv,
                        "tiles,{name},{qname},{w},{h},{run},{o:.2},{u:.2},{i},{pm:.2},{mm:.2},{ow},{oh},{},{:.1},{:.1}",
                        match same { 1 => "yes", 0 => "NO", _ => "" },
                        mb(ws),
                        mb(private)
                    )
                    .map_err(|e| e.to_string())?;
                }
                let unpack = median(ses.iter().map(|s| s.1).collect());
                let first = median(
                    (0..ses.len())
                        .map(|r| {
                            let t = tiles.iter().find(|t| t.0 == r && t.1 == 0);
                            ses[r].0 + ses[r].1 + t.map_or(f64::NAN, |t| t.2 + t.3)
                        })
                        .collect(),
                );
                let next = median(
                    tiles
                        .iter()
                        .filter(|t| t.1 > 0)
                        .map(|t| t.2 + t.3)
                        .collect(),
                );
                let all7 = median(
                    (0..ses.len())
                        .map(|r| {
                            ses[r].0
                                + ses[r].1
                                + tiles
                                    .iter()
                                    .filter(|t| t.0 == r)
                                    .map(|t| t.2 + t.3)
                                    .sum::<f64>()
                        })
                        .collect(),
                );
                let identical = tiles.iter().filter(|t| t.6 != 2).all(|t| t.6 == 1);
                println!(
                    "{:<14} {:<7} {:>10} {:>8.1} {:>13.1} {:>12.1} {:>12.1} {:>9} {:>7.0}MB {:>7.0}MB",
                    name,
                    qname,
                    format!("{w}x{h}"),
                    unpack,
                    first,
                    next,
                    all7,
                    if identical { "yes" } else { "NO" },
                    mb(ws),
                    mb(private)
                );
            }
        }
    }
    Ok(())
}

fn parent_ptiles(file: &Path, threads: &[usize], csv: &mut impl Write) -> Result<(), String> {
    writeln!(
        csv,
        "kind,threads,wall_ms,first_strip_ms,cpu_ms,peak_ws_mb,peak_private_mb"
    )
    .map_err(|e| e.to_string())?;
    println!("\nviewport 2560x1440 AHD, split into N strips, N LibRaw instances in parallel");
    println!(
        "{:>7} {:>16} {:>14} {:>9} {:>9} {:>9}",
        "strips", "viewport done ms", "first strip ms", "cpu/wall", "peak WS", "peak priv"
    );
    for &n in threads {
        let n_s = n.to_string();
        let mut walls = Vec::new();
        let mut firsts = Vec::new();
        let mut cpus = Vec::new();
        let (mut ws, mut private) = (0u64, 0u64);
        for _ in 0..3 {
            let out = run_child(&[
                OsStr::new("--child-ptiles"),
                OsStr::new(&n_s),
                file.as_os_str(),
            ])?;
            let line = out
                .lines()
                .find(|l| l.starts_with("PT"))
                .ok_or("no PT line")?;
            let p: Vec<f64> = line
                .split_whitespace()
                .skip(1)
                .map(|x| x.parse().unwrap_or(f64::NAN))
                .collect();
            walls.push(p[1]);
            firsts.push(p[2]);
            cpus.push(p[3]);
            ws = ws.max(p[4] as u64);
            private = private.max(p[5] as u64);
        }
        let (wall, first, cpu) = (median(walls), median(firsts), median(cpus));
        writeln!(
            csv,
            "ptiles,{n},{wall:.1},{first:.1},{cpu:.1},{:.1},{:.1}",
            mb(ws),
            mb(private)
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{n:>7} {wall:>16.1} {first:>14.1} {:>9.2} {:>7.0}MB {:>7.0}MB",
            cpu / wall,
            mb(ws),
            mb(private)
        );
    }
    Ok(())
}

/// `verify`: hashes of whole-image decodes (to compare builds) and tile-vs-full comparisons
/// with several margins (to find the overlap that makes tiles seamless).
fn parent_verify(files: &[PathBuf], csv: &mut impl Write) -> Result<(), String> {
    writeln!(csv, "kind,file,what,auto_bright,margin,w,h,hash,dx,dy,diff_pct,max_diff,mean_abs,band_max,interior_max,ms")
        .map_err(|e| e.to_string())?;
    let margins: [c_int; 6] = [0, 2, 4, 8, 16, 32];
    for f in files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let wpath = wide(f);
        println!("\n{name}: whole-image decodes (sensor orientation, camera WB)");
        for (what, q, half, auto) in [
            ("full_ahd", 3, 0, 0),
            ("full_ahd", 3, 0, 1),
            ("full_linear", 0, 0, 0),
            ("half_size", 0, 1, 0),
            ("half_size", 0, 1, 1),
        ] {
            let (mut hash, mut w, mut h) = (0u64, 0, 0);
            let mut err = [0 as c_char; 160];
            let t = Instant::now();
            // SAFETY: NUL-terminated path; valid out-pointers; 160-byte error buffer.
            let ok = unsafe {
                pw_decode_hash(
                    wpath.as_ptr(),
                    q,
                    half,
                    auto,
                    &mut hash,
                    &mut w,
                    &mut h,
                    err.as_mut_ptr(),
                )
            };
            if ok == 0 {
                return Err(format!("{name} {what}: {}", c_err(&err)));
            }
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            println!("  {what:<12} auto_bright={auto}  {w}x{h}  hash {hash:016x}");
            writeln!(
                csv,
                "hash,{name},{what},{auto},,{w},{h},{hash:016x},,,,,,,,{ms:.0}"
            )
            .map_err(|e| e.to_string())?;
        }
        for auto in [0, 1] {
            // A 1024 x 1024 tile at an even position inside the image (as the viewer's grid).
            let (x, y, w, h) = (2048, 2048, 1024, 1024);
            // SAFETY: zeroed plain old data.
            let mut res: [SeamResult; 6] = unsafe { std::mem::zeroed() };
            let mut full_hash = 0u64;
            let mut err = [0 as c_char; 160];
            // SAFETY: as above; `res` has room for `margins.len()` results.
            let ok = unsafe {
                pw_seam_check(
                    wpath.as_ptr(),
                    auto,
                    x,
                    y,
                    w,
                    h,
                    margins.as_ptr(),
                    margins.len() as c_int,
                    res.as_mut_ptr(),
                    &mut full_hash,
                    err.as_mut_ptr(),
                )
            };
            if ok == 0 {
                return Err(format!("{name} seam: {}", c_err(&err)));
            }
            println!(
                "  tile 1024x1024 at (2048,2048) vs full AHD, auto_bright={auto} (full hash {full_hash:016x})"
            );
            println!(
                "  {:>6} {:>11} {:>7} {:>9} {:>8} {:>9} {:>9} {:>12} {:>8}",
                "margin",
                "processed",
                "offset",
                "differ %",
                "max",
                "mean",
                "edge max",
                "inside max",
                "ms"
            );
            for r in &res {
                let e = c_err(&r.error);
                if !e.is_empty() {
                    println!("  {:>6} error: {e}", r.margin);
                    continue;
                }
                println!(
                    "  {:>6} {:>11} {:>7} {:>9.3} {:>8} {:>9.4} {:>9} {:>12} {:>8.0}",
                    r.margin,
                    format!("{}x{}", r.out_w, r.out_h),
                    format!("{},{}", r.dx, r.dy),
                    r.diff_pct,
                    r.max_diff,
                    r.mean_abs,
                    r.band_max,
                    r.interior_max,
                    r.process_ms
                );
                writeln!(
                    csv,
                    "seam,{name},tile1024,{auto},{},{},{},{:016x},{},{},{:.4},{},{:.5},{},{},{:.0}",
                    r.margin,
                    r.out_w,
                    r.out_h,
                    r.hash,
                    r.dx,
                    r.dy,
                    r.diff_pct,
                    r.max_diff,
                    r.mean_abs,
                    r.band_max,
                    r.interior_max,
                    r.process_ms
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

thread_local! {
    static THR_TAG: std::cell::Cell<f32> = const { std::cell::Cell::new(0.75) };
}

fn set_thr_tag(thr: f32) {
    THR_TAG.with(|t| t.set(thr));
}

/// Suffix for the `instance` column: `/max0` when adjust_maximum is off.
fn thr_tag() -> &'static str {
    if THR_TAG.with(|t| t.get()) == 0.0 {
        "/max0"
    } else {
        ""
    }
}

/// `crops`: which crop positions reproduce the full decode exactly? Each case runs in a fresh
/// LibRaw instance (one full decode + one crop), so no state carries over between crops; the
/// last case repeats all margins in one instance for comparison.
fn parent_crops(files: &[PathBuf], csv: &mut impl Write) -> Result<(), String> {
    writeln!(csv, "kind,file,x,y,w,h,margin,instance,out_w,out_h,dx,dy,diff_pct,max_diff,band_max,interior_max")
        .map_err(|e| e.to_string())?;
    // (x, y, margin): vary the crop origin row and column around the earlier failures.
    let cases: &[(c_int, c_int, c_int)] = &[
        (2048, 2048, 8),
        (2048, 2048, 16),
        (2048, 2048, 32),
        (2048, 2056, 16),
        (2048, 2052, 16),
        (2048, 2050, 16),
        (2056, 2048, 16),
        (2048, 2040, 8),
        (2048, 2024, 8),
        (2048, 2016, 8),
        (4096, 3000, 8),
        (4096, 3000, 16),
        (1024, 512, 16),
    ];
    for f in files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let wpath = wide(f);
        let (mut filters, mut top, mut left, mut w, mut h, mut rw, mut rh) =
            (0u32, 0, 0, 0, 0, 0, 0);
        let mut err = [0 as c_char; 160];
        // SAFETY: NUL-terminated path; valid out-pointers.
        let ok = unsafe {
            pw_probe_info(
                wpath.as_ptr(),
                &mut filters,
                &mut top,
                &mut left,
                &mut w,
                &mut h,
                &mut rw,
                &mut rh,
                err.as_mut_ptr(),
            )
        };
        if ok == 0 {
            return Err(format!("{name}: {}", c_err(&err)));
        }
        println!(
            "\n{name}: filters 0x{filters:08x}, visible {w}x{h} at left {left} top {top}, raw {rw}x{rh}"
        );
        println!(
            "  {:>11} {:>6} {:>9} {:>11} {:>7} {:>9} {:>8} {:>9} {:>12}",
            "origin",
            "margin",
            "instance",
            "processed",
            "offset",
            "differ %",
            "max",
            "edge max",
            "inside max"
        );
        let mut run = |x: c_int, y: c_int, margins: &[c_int], inst: &str| -> Result<(), String> {
            let inst = format!("{inst}{}", thr_tag());
            // SAFETY: zeroed plain old data.
            let mut res: [SeamResult; 6] = unsafe { std::mem::zeroed() };
            let mut full_hash = 0u64;
            let mut err = [0 as c_char; 160];
            // SAFETY: valid pointers; `res` holds up to 6 results.
            let ok = unsafe {
                pw_seam_check(
                    wpath.as_ptr(),
                    0,
                    x,
                    y,
                    1024,
                    1024,
                    margins.as_ptr(),
                    margins.len() as c_int,
                    res.as_mut_ptr(),
                    &mut full_hash,
                    err.as_mut_ptr(),
                )
            };
            if ok == 0 {
                return Err(format!("{name}: {}", c_err(&err)));
            }
            for r in &res[..margins.len()] {
                let e = c_err(&r.error);
                if !e.is_empty() {
                    println!(
                        "  {:>11} {:>6} {inst:>9} error: {e}",
                        format!("{x},{y}"),
                        r.margin
                    );
                    continue;
                }
                println!(
                    "  {:>11} {:>6} {inst:>9} {:>11} {:>7} {:>9.3} {:>8} {:>9} {:>12}",
                    format!("{x},{y}"),
                    r.margin,
                    format!("{}x{}", r.out_w, r.out_h),
                    format!("{},{}", r.dx, r.dy),
                    r.diff_pct,
                    r.max_diff,
                    r.band_max,
                    r.interior_max
                );
                writeln!(
                    csv,
                    "crop,{name},{x},{y},1024,1024,{},{inst},{},{},{},{},{:.4},{},{},{}",
                    r.margin,
                    r.out_w,
                    r.out_h,
                    r.dx,
                    r.dy,
                    r.diff_pct,
                    r.max_diff,
                    r.band_max,
                    r.interior_max
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        for thr in [0.75f32, 0.0] {
            // SAFETY: sets a plain global used by later decodes on this thread.
            unsafe { pw_set_adjust_maximum_thr(thr) };
            set_thr_tag(thr);
            for &(x, y, m) in cases {
                run(x, y, &[m], "fresh")?;
            }
            run(2048, 2048, &[0, 2, 4, 8, 16, 32], "shared")?;
        }
        // SAFETY: as above (restore LibRaw's default).
        unsafe { pw_set_adjust_maximum_thr(0.75) };
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--child-single") => {
            let reps = args[2].parse().map_err(|_| "reps")?;
            return child_single(&args[1], reps, Path::new(&args[3]));
        }
        Some("--child-parallel") => {
            let n = args[2].parse().map_err(|_| "n")?;
            let files: Vec<PathBuf> = args[3..].iter().map(PathBuf::from).collect();
            return child_parallel(&args[1], n, &files);
        }
        Some("--child-tiles") => {
            let num = |i: usize| args[i].parse::<i32>().map_err(|_| "tiles args");
            let reps = args[4].parse().map_err(|_| "reps")?;
            return child_tiles(num(1)?, num(2)?, num(3)?, reps, Path::new(&args[5]));
        }
        Some("--child-ptiles") => {
            let n = args[1].parse().map_err(|_| "n")?;
            return child_ptiles(n, Path::new(&args[2]));
        }
        _ => {}
    }
    let usage = "usage: raw-decode-bench single|parallel|tiles|ptiles|verify|crops <file>... [--reps N] [--threads 1,2,4,8] [--config NAME] [--out results.csv]";
    let mode = args.first().ok_or(usage)?.clone();
    let mut files = Vec::new();
    let (mut reps, mut threads, mut out) = (3usize, vec![1usize, 2, 4, 8], None::<PathBuf>);
    let mut cfg = "full_ahd".to_string();
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--reps" => reps = it.next().and_then(|v| v.parse().ok()).ok_or("--reps N")?,
            "--threads" => {
                threads = it
                    .next()
                    .ok_or("--threads list")?
                    .split(',')
                    .map(|v| v.trim().parse().map_err(|_| "--threads list"))
                    .collect::<Result<_, _>>()?
            }
            "--out" => out = Some(it.next().ok_or("--out path")?.into()),
            "--config" => cfg = it.next().ok_or("--config NAME")?.clone(),
            f => files.push(PathBuf::from(f)),
        }
    }
    if files.is_empty() {
        return Err(usage.into());
    }
    // SAFETY: returns a static NUL-terminated string.
    let version = unsafe { CStr::from_ptr(pw_libraw_version()) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: plain query.
    let caps = unsafe { pw_libraw_capabilities() };
    println!(
        "LibRaw {version} (capabilities 0x{caps:x}), {} logical CPUs",
        std::thread::available_parallelism().map_or(0, |n| n.get())
    );
    let out = out.unwrap_or_else(|| PathBuf::from(format!("raw-decode-{mode}.csv")));
    let mut csv = std::io::BufWriter::new(std::fs::File::create(&out).map_err(|e| e.to_string())?);
    let t = Instant::now();
    match mode.as_str() {
        "single" => parent_single(&files, reps, &mut csv)?,
        "parallel" => parent_parallel(&files, &threads, &cfg, &mut csv)?,
        "tiles" => parent_tiles(&files, reps, &mut csv)?,
        "ptiles" => parent_ptiles(&files[0], &threads, &mut csv)?,
        "verify" => parent_verify(&files, &mut csv)?,
        "crops" => parent_crops(&files, &mut csv)?,
        _ => return Err(usage.into()),
    }
    csv.flush().map_err(|e| e.to_string())?;
    println!(
        "\nLibRaw {version}; results: {} ({:.0} s)",
        out.display(),
        Duration::as_secs_f64(&t.elapsed())
    );
    Ok(())
}
