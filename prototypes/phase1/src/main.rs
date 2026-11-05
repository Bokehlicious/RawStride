//! ADR 0008 phase 1 prototypes (separate from the app; ADR 0008 §8.1).
//!
//! * `p0 <out.csv> <files…>` – P0: JPEG decode of the largest embedded preview with zune-jpeg,
//!   jpeg-decoder and libjpeg-turbo (feature `turbojpeg`, default; full size and 1/2 scale), the
//!   decoders of tools/preview-bench; single image and N threads.
//! * `p4b <out-dir> <threads-list> <files…>` – P4b: demosaic methods against AHD on the same file.
//!   Each (file, method, threads) runs in its own child process, so peak memory and CPU time belong
//!   to that one method. Writes timings, quality metrics (8.3) and blind 100 % crops.
//! * `p4 <out-dir> <group> <strategies,list> <files…>` – P4: scheduling without UI (src/p4.rs). Each
//!   strategy runs in its own child process; events go to `p4-events.csv`, totals to `p4-runs.csv`.
//! * `p4sum <out-dir>` – aggregates `p4-events.csv` into `p4-summary.csv`.
//! * `child …`, `p4child …` – internal.

mod bmp;
mod metrics;
mod p4;
#[cfg(feature = "turbojpeg")]
mod turbo;

use std::ffi::{OsStr, OsString, c_char, c_int, c_uint, c_void};
use std::fmt::Write as _;
use std::io::Write as _;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// ---------- LibRaw prototype shim ----------

#[repr(C)]
struct PpInfo {
    open_ms: f64,
    unpack_ms: f64,
    width: c_int,
    height: c_int,
    flip: c_int,
    error: [c_char; 200],
}

#[repr(C)]
struct PpRenderInfo {
    process_ms: f64,
    mkimg_ms: f64,
    width: c_int,
    height: c_int,
    error: [c_char; 200],
}

unsafe extern "C" {
    fn pp_version() -> *const c_char;
    fn pp_open(path: *const u16, info: *mut PpInfo) -> *mut c_void;
    fn pp_render(
        s: *mut c_void,
        quality: c_int,
        half: c_int,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        ri: *mut PpRenderInfo,
    ) -> *mut c_void;
    fn pp_image(img: *const c_void, data: *mut *const u8, size: *mut c_uint);
    fn pp_free(img: *mut c_void);
    fn pp_close(s: *mut c_void);
}

fn c_str(e: &[c_char]) -> String {
    let bytes: Vec<u8> = e.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

struct Session(*mut c_void, PpInfo);

impl Session {
    fn open(path: &Path) -> Result<Session, String> {
        let w = wide(path);
        let mut info: PpInfo = unsafe { std::mem::zeroed() };
        // SAFETY: valid NUL-terminated wide path and out-pointer.
        let s = unsafe { pp_open(w.as_ptr(), &mut info) };
        if s.is_null() {
            return Err(format!("open: {}", c_str(&info.error)));
        }
        Ok(Session(s, info))
    }

    /// Renders and returns (rgb, width, height, process_ms, mkimg_ms).
    fn render(&self, q: i32, half: bool, region: Option<[i32; 4]>) -> Result<(Vec<u8>, usize, usize, f64, f64), String> {
        let [x, y, w, h] = region.unwrap_or([0, 0, 0, 0]);
        let mut ri: PpRenderInfo = unsafe { std::mem::zeroed() };
        // SAFETY: session is open; out-pointer valid.
        let img = unsafe { pp_render(self.0, q, half as c_int, x, y, w, h, &mut ri) };
        if img.is_null() {
            return Err(format!("render q{q}: {}", c_str(&ri.error)));
        }
        let (mut data, mut size) = (std::ptr::null(), 0u32);
        // SAFETY: img is a valid processed image until pp_free.
        let rgb = unsafe {
            pp_image(img, &mut data, &mut size);
            let v = std::slice::from_raw_parts(data, size as usize).to_vec();
            pp_free(img);
            v
        };
        Ok((rgb, ri.width as usize, ri.height as usize, ri.process_ms, ri.mkimg_ms))
    }
}

// SAFETY: a LibRaw instance may be used from any thread, as long as only one thread uses it at a
// time; the P4 scheduler hands each session to exactly one worker at a time.
unsafe impl Send for Session {}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: opened by pp_open, closed once.
        unsafe { pp_close(self.0) };
    }
}

// ---------- process counters (Windows) ----------

mod sys {
    use std::ffi::c_void;
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
        fn GetProcessTimes(p: *mut c_void, c: *mut FileTime, e: *mut FileTime, k: *mut FileTime, u: *mut FileTime) -> i32;
        fn K32GetProcessMemoryInfo(p: *mut c_void, m: *mut MemCounters, cb: u32) -> i32;
    }
    /// CPU time of this process (all threads), ms.
    pub fn cpu_ms() -> f64 {
        let (mut c, mut e, mut k, mut u) = Default::default();
        // SAFETY: pseudo-handle, valid out-pointers.
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
        let t = |f: FileTime| (u64::from(f.high) << 32) | u64::from(f.low);
        (t(k) + t(u)) as f64 / 10_000.0
    }
    /// (current private bytes, peak private bytes) in MB.
    pub fn private_mb() -> (f64, f64) {
        let mut m = MemCounters { cb: std::mem::size_of::<MemCounters>() as u32, ..Default::default() };
        // SAFETY: sized struct.
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) };
        (m.pagefile_usage as f64 / 1048576.0, m.peak_pagefile_usage as f64 / 1048576.0)
    }
}

// ---------- P0: JPEG decoders ----------

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() { f64::NAN } else { v[v.len() / 2] }
}

fn decode_zune(data: &[u8]) -> Result<usize, String> {
    let mut d = zune_jpeg::JpegDecoder::new(data);
    let px = d.decode().map_err(|e| format!("zune-jpeg: {e:?}"))?;
    Ok(px.len())
}

fn decode_jd(data: &[u8]) -> Result<usize, String> {
    let mut d = jpeg_decoder::Decoder::new(std::io::Cursor::new(data));
    let px = d.decode().map_err(|e| format!("jpeg-decoder: {e}"))?;
    Ok(px.len())
}

/// libjpeg-turbo, one decode at scale 1/`div`; `None` when built without the feature.
#[cfg(feature = "turbojpeg")]
fn decode_tj(data: &[u8], div: u32) -> Result<Option<usize>, String> {
    let mut d = turbo::Decompressor::new()?;
    Ok(Some(d.decode(data, div)?.2.len()))
}
#[cfg(not(feature = "turbojpeg"))]
fn decode_tj(_: &[u8], _: u32) -> Result<Option<usize>, String> {
    Ok(None)
}

/// Median of 5 timed runs of `f` (NaN when `f` reports "not available").
fn time5(mut f: impl FnMut() -> Result<Option<usize>, String>) -> Result<f64, String> {
    let mut v = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        if f()?.is_none() {
            return Ok(f64::NAN);
        }
        v.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(median(v))
}

fn p0(out: &Path, files: &[PathBuf]) -> Result<(), String> {
    let mut csv = String::from("file,preview_w,preview_h,bytes,read_ms,zune_ms,jpegdecoder_ms,turbo_ms,turbo_half_ms\n");
    let mut all: Vec<Vec<u8>> = Vec::new();
    for f in files {
        let info = match pw_raw::inspect(f) {
            Ok(i) => i,
            Err(e) => { eprintln!("{}: {e}", f.display()); continue; }
        };
        let Some(p) = info.pick(pw_raw::PreviewSize::Large).cloned() else { continue };
        let t = Instant::now();
        let data = pw_raw::read_preview(f, &p).map_err(|e| e.to_string())?;
        let read_ms = t.elapsed().as_secs_f64() * 1000.0;
        let zm = time5(|| decode_zune(&data).map(Some))?;
        let jm = time5(|| decode_jd(&data).map(Some))?;
        let tm = time5(|| decode_tj(&data, 1))?;
        let th = time5(|| decode_tj(&data, 2))?;
        let name = f.file_name().unwrap().to_string_lossy();
        let _ = writeln!(csv, "{name},{},{},{},{read_ms:.2},{zm:.2},{jm:.2},{tm:.2},{th:.2}", p.width, p.height, data.len());
        println!("P0 {name}: {}x{} zune {zm:.1} ms, jpeg-decoder {jm:.1} ms, turbo {tm:.1} ms, turbo 1/2 {th:.1} ms", p.width, p.height);
        if p.width >= 4000 {
            all.push(data);
        }
    }
    // Parallel throughput on the large previews (≥ 4000 px): several images at once.
    csv.push_str("\ndecoder,parallel_threads,images,wall_ms,images_per_s\n");
    if !all.is_empty() {
        let decoders: [(&str, fn(&[u8]) -> Result<Option<usize>, String>); 2] =
            [("zune", |d| decode_zune(d).map(Some)), ("turbo", |d| decode_tj(d, 1))];
        for (dname, dec) in decoders {
            if dec(&all[0])?.is_none() {
                continue;
            }
            for threads in [1usize, 2, 4, 8] {
                let n = all.len() * 2;
                let next = AtomicUsize::new(0);
                let t = Instant::now();
                std::thread::scope(|s| {
                    for _ in 0..threads {
                        s.spawn(|| loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            if i >= n { break; }
                            let _ = dec(&all[i % all.len()]);
                        });
                    }
                });
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let _ = writeln!(csv, "{dname},{threads},{n},{ms:.1},{:.2}", n as f64 / (ms / 1000.0));
            }
        }
    }
    std::fs::write(out, csv).map_err(|e| e.to_string())
}

// ---------- P4: scheduling ----------

/// The real decoder for P4: the prototype LibRaw shim, and libjpeg-turbo (or zune-jpeg) for previews.
struct LibRawP4;

impl p4::Backend for LibRawP4 {
    type S = Session;
    fn open(&self, path: &Path) -> Result<(Session, u32, u32, i32), String> {
        let s = Session::open(path)?;
        let (w, h, f) = (s.1.width as u32, s.1.height as u32, s.1.flip);
        Ok((s, w, h, f))
    }
    fn half(&self, s: &mut Session, q: i32) -> Result<Vec<u8>, String> {
        Ok(s.render(q, true, None)?.0)
    }
    fn region(&self, s: &mut Session, q: i32, r: [u32; 4]) -> Result<Vec<u8>, String> {
        Ok(s.render(q, false, Some([r[0] as i32, r[1] as i32, r[2] as i32, r[3] as i32]))?.0)
    }
    fn jpeg(&self, path: &Path) -> Result<Vec<u8>, String> {
        let info = pw_raw::inspect(path).map_err(|e| e.to_string())?;
        let p = info.pick(pw_raw::PreviewSize::Large).cloned().ok_or("no embedded preview")?;
        let data = pw_raw::read_preview(path, &p).map_err(|e| e.to_string())?;
        #[cfg(feature = "turbojpeg")]
        {
            Ok(turbo::Decompressor::new()?.decode(&data, 1)?.2)
        }
        #[cfg(not(feature = "turbojpeg"))]
        {
            zune_jpeg::JpegDecoder::new(&data).decode().map_err(|e| format!("zune-jpeg: {e:?}"))
        }
    }
}

const P4_VIEW: (u32, u32) = (1920, 1080);
const P4_BUDGET_MB: usize = 4096;
const P4_FOLDER_LEN: usize = 1000;

fn p4child(group: &str, strat: &str, files: &[PathBuf]) -> Result<(), String> {
    let st = p4::strategy(strat).ok_or_else(|| format!("unknown strategy {strat}"))?;
    let budget = st.budget_mb.unwrap_or(P4_BUDGET_MB);
    let cfg = p4::Config { view_w: P4_VIEW.0, view_h: P4_VIEW.1, budget_bytes: budget << 20, tile: 1024 };
    let mut d = p4::Driver::new(LibRawP4, files.to_vec(), P4_FOLDER_LEN, st, cfg);
    p4::run_all(&mut d, 1.0);
    let (rows, sum) = d.finish(group);
    print!("{rows}");
    println!("{sum}");
    Ok(())
}

fn p4(out: &Path, group: &str, strategies: &str, files: &[PathBuf]) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let ev_path = out.join("p4-events.csv");
    let run_path = out.join("p4-runs.csv");
    if !ev_path.exists() {
        std::fs::write(&ev_path, "group,strategy,scenario,label,pos,view,t_req_ms,latency_ms,shown,hit,on_screen_ms\n").map_err(|e| e.to_string())?;
    }
    if !run_path.exists() {
        std::fs::write(&run_path, "group,strategy,wall_ms,cpu_ms,avg_cores,peak_private_mb,jobs,job_ms,wasted_jobs,wasted_ms,errors,evicted_views,evicted_sessions,peak_tracked_mb\n").map_err(|e| e.to_string())?;
    }
    for st in strategies.split(',') {
        println!("P4 {group} {st}: running (about 6-8 minutes)");
        let mut args: Vec<OsString> = vec!["p4child".into(), group.into(), st.into()];
        args.extend(files.iter().map(|f| f.as_os_str().to_owned()));
        let a: Vec<&OsStr> = args.iter().map(|x| x.as_os_str()).collect();
        let o = run_child(&exe, &a, "all")?;
        let mut ev = String::new();
        let mut run = String::new();
        for line in o.lines() {
            if let Some(r) = line.strip_prefix("EV,") {
                ev.push_str(r);
                ev.push('\n');
            } else if let Some(r) = line.strip_prefix("SUM,") {
                run.push_str(r);
                run.push('\n');
                println!("P4 {group} {st}: {r}");
            }
        }
        let append = |p: &Path, s: &str| -> Result<(), String> {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new().append(true).open(p).map_err(|e| e.to_string())?;
            f.write_all(s.as_bytes()).map_err(|e| e.to_string())
        };
        append(&ev_path, &ev)?;
        append(&run_path, &run)?;
    }
    Ok(())
}

fn p4sum(out: &Path) -> Result<(), String> {
    let ev = std::fs::read_to_string(out.join("p4-events.csv")).map_err(|e| e.to_string())?;
    std::fs::write(out.join("p4-summary.csv"), p4::summarize(&ev)).map_err(|e| e.to_string())
}

// ---------- P4b: demosaic methods ----------

const METHODS: &[(&str, i32, bool)] = &[
    ("AHD", 3, false),
    ("linear", 0, false),
    ("VNG", 1, false),
    ("PPG", 2, false),
    ("DCB", 4, false),
    ("DHT", 11, false),
    ("AAHD", 12, false),
    ("half-AHD", 3, true),
];

const REGION_W: i32 = 1920;
const REGION_H: i32 = 1080;

/// Picks the 1920×1080 window with the most detail, from a half-size AHD render.
fn pick_region(s: &Session) -> Result<[i32; 4], String> {
    let (rgb, w, h, ..) = s.render(3, true, None)?;
    let l: Vec<f64> = rgb.chunks_exact(3).map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2])).collect();
    let (ww, wh) = ((REGION_W / 2) as usize, (REGION_H / 2) as usize);
    if w <= ww || h <= wh {
        return Ok([0, 0, (w * 2) as i32, (h * 2) as i32]);
    }
    // gradient energy, integral image
    let mut ii = vec![0.0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0.0;
        for x in 0..w {
            let g = if x + 1 < w && y + 1 < h {
                (l[y * w + x + 1] - l[y * w + x]).abs() + (l[(y + 1) * w + x] - l[y * w + x]).abs()
            } else { 0.0 };
            row += g;
            ii[(y + 1) * (w + 1) + x + 1] = ii[y * (w + 1) + x + 1] + row;
        }
    }
    let sum = |x0: usize, y0: usize| {
        let (x1, y1) = (x0 + ww, y0 + wh);
        ii[y1 * (w + 1) + x1] - ii[y0 * (w + 1) + x1] - ii[y1 * (w + 1) + x0] + ii[y0 * (w + 1) + x0]
    };
    let mut best = (f64::MIN, 0, 0);
    for y0 in (0..h - wh).step_by(60) {
        for x0 in (0..w - ww).step_by(60) {
            let e = sum(x0, y0);
            if e > best.0 { best = (e, x0, y0); }
        }
    }
    Ok([(best.1 * 2) as i32, (best.2 * 2) as i32, REGION_W, REGION_H])
}

/// `child <file> <method> <out-dir> <write-images 0/1>`; prints one CSV line.
fn child(file: &Path, method: &str, out: &Path, write_images: bool) -> Result<(), String> {
    let &(name, q, half) = METHODS.iter().find(|m| m.0 == method).ok_or("unknown method")?;
    let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
    let cpu0 = sys::cpu_ms();
    let t0 = Instant::now();
    let s = Session::open(file)?;
    let (priv_after_unpack, _) = sys::private_mb();
    let region_file = out.join(format!("{stem}.region.txt"));
    let region: [i32; 4] = if let Ok(t) = std::fs::read_to_string(&region_file) {
        let v: Vec<i32> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        [v[0], v[1], v[2], v[3]]
    } else {
        let r = pick_region(&s)?;
        std::fs::write(&region_file, format!("{} {} {} {}", r[0], r[1], r[2], r[3])).map_err(|e| e.to_string())?;
        r
    };
    // whole image
    let tw = Instant::now();
    let (_, ow, oh, whole_proc, whole_mk) = s.render(q, half, None)?;
    let whole_ms = tw.elapsed().as_secs_f64() * 1000.0;
    // region (not for half size), 3 runs
    let (mut reg_ms, mut reg_proc) = (Vec::new(), Vec::new());
    let mut region_rgb = None;
    if !half {
        for _ in 0..3 {
            let t = Instant::now();
            let (rgb, rw, rh, p, _) = s.render(q, false, Some(region))?;
            reg_ms.push(t.elapsed().as_secs_f64() * 1000.0);
            reg_proc.push(p);
            region_rgb = Some((rgb, rw, rh));
        }
    }
    let cpu = sys::cpu_ms() - cpu0;
    let (_, peak) = sys::private_mb();
    let total = t0.elapsed().as_secs_f64() * 1000.0;
    if write_images {
        if let Some((rgb, rw, rh)) = &region_rgb {
            bmp::write(&out.join("regions").join(format!("{stem}.{name}.bmp")), *rw, *rh, rgb, s.1.flip)?;
        }
    }
    println!(
        "ROW,{stem},{name},{},{},{:.1},{:.1},{ow},{oh},{whole_ms:.1},{whole_proc:.1},{whole_mk:.1},{:.1},{:.1},{cpu:.0},{total:.0},{priv_after_unpack:.0},{peak:.0},{}",
        s.1.width, s.1.height, s.1.open_ms, s.1.unpack_ms, median(reg_ms), median(reg_proc),
        std::env::var("OMP_NUM_THREADS").unwrap_or_else(|_| "all".into())
    );
    Ok(())
}

fn run_child(exe: &Path, args: &[&OsStr], threads: &str) -> Result<String, String> {
    let mut c = Command::new(exe);
    c.args(args);
    if threads != "all" {
        c.env("OMP_NUM_THREADS", threads);
    } else {
        c.env_remove("OMP_NUM_THREADS");
    }
    let o = c.output().map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    if !o.status.success() {
        return Err(format!("child failed: {} {}", out, String::from_utf8_lossy(&o.stderr)));
    }
    Ok(out)
}

/// Deterministic shuffle of the method codes per file, so crops can be reviewed blind.
fn codes_for(stem: &str, n: usize) -> Vec<char> {
    let mut seed: u64 = stem.bytes().fold(1469598103934665603, |h, b| (h ^ u64::from(b)).wrapping_mul(1099511628211));
    let mut idx: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        idx.swap(i, (seed % (i as u64 + 1)) as usize);
    }
    idx.iter().map(|i| (b'A' + *i as u8) as char).collect()
}

fn p4b(out: &Path, threads_list: &str, files: &[PathBuf]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out.join("regions")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out.join("crops")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out.join("key-open-after-review")).map_err(|e| e.to_string())?;
    let mut timing = String::from(
        "file,method,width,height,open_ms,unpack_ms,out_w,out_h,whole_ms,whole_process_ms,whole_mkimg_ms,region_ms,region_process_ms,cpu_ms,child_total_ms,private_after_unpack_mb,peak_private_mb,omp_threads\n",
    );
    let threads: Vec<&str> = threads_list.split(',').collect();
    for f in files {
        for (ti, th) in threads.iter().enumerate() {
            for (name, ..) in METHODS {
                let write = ti == 0; // images only once (identical for every thread count)
                let args: Vec<OsString> = vec!["child".into(), f.into(), (*name).into(), out.into(), if write { "1" } else { "0" }.into()];
                let a: Vec<&OsStr> = args.iter().map(|x| x.as_os_str()).collect();
                match run_child(&exe, &a, th) {
                    Ok(o) => {
                        for line in o.lines().filter(|l| l.starts_with("ROW,")) {
                            timing.push_str(&line[4..]);
                            timing.push('\n');
                            println!("{line}");
                        }
                    }
                    Err(e) => println!("ERROR {} {name} threads {th}: {e}", f.display()),
                }
            }
        }
    }
    std::fs::write(out.join("p4b-timing.csv"), &timing).map_err(|e| e.to_string())?;

    // Quality: every method's region against the AHD region of the same file.
    let mut q = String::from(
        "file,method,de00_mean,de00_p99,ssim,ms_ssim,psnr_y,rise_ref_px,rise_test_px,hf_ratio,false_colour_ref,false_colour_test,zipper_ratio,edge_points\n",
    );
    let mut key = String::from("file,crop_code,method\n");
    for f in files {
        let stem = f.file_stem().unwrap().to_string_lossy().into_owned();
        let Ok(reference) = bmp::read(&out.join("regions").join(format!("{stem}.AHD.bmp"))) else { continue };
        let methods: Vec<&str> = METHODS.iter().filter(|m| !m.2).map(|m| m.0).collect();
        let codes = codes_for(&stem, methods.len());
        // two crops of 640×480 with the most detail in the reference region (non-overlapping)
        let crops = bmp::detail_crops(&reference, 640, 480, 2);
        for (mi, m) in methods.iter().enumerate() {
            let Ok(test) = bmp::read(&out.join("regions").join(format!("{stem}.{m}.bmp"))) else { continue };
            if (test.w, test.h) != (reference.w, reference.h) { continue; }
            let r = metrics::compare(
                metrics::Img { w: reference.w, h: reference.h, rgb: &reference.rgb },
                metrics::Img { w: test.w, h: test.h, rgb: &test.rgb },
            );
            let _ = writeln!(q, "{stem},{m},{:.3},{:.3},{:.5},{:.5},{:.2},{:.3},{:.3},{:.4},{:.3},{:.3},{:.4},{}",
                r.de00_mean, r.de00_p99, r.ssim, r.ms_ssim, r.psnr_y, r.rise_ref_px, r.rise_test_px, r.hf_ratio,
                r.false_colour_ref, r.false_colour_test, r.zipper_ratio, r.edge_points);
            for (ci, c) in crops.iter().enumerate() {
                let name = format!("{stem}-crop{}-{}.bmp", ci + 1, codes[mi]);
                bmp::write_crop(&out.join("crops").join(name), &test, *c)?;
            }
            let _ = writeln!(key, "{stem},{},{m}", codes[mi]);
        }
    }
    std::fs::write(out.join("p4b-quality.csv"), q).map_err(|e| e.to_string())?;
    std::fs::write(out.join("key-open-after-review").join("crop-key.csv"), key).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let s = |i: usize| args.get(i).map(|a| a.to_string_lossy().into_owned()).unwrap_or_default();
    let r = match s(0).as_str() {
        "p0" => p0(Path::new(&args[1]), &args[2..].iter().map(PathBuf::from).collect::<Vec<_>>()),
        "p4b" => p4b(Path::new(&args[1]), &s(2), &args[3..].iter().map(PathBuf::from).collect::<Vec<_>>()),
        "p4" => p4(Path::new(&args[1]), &s(2), &s(3), &args[4..].iter().map(PathBuf::from).collect::<Vec<_>>()),
        "p4child" => p4child(&s(1), &s(2), &args[3..].iter().map(PathBuf::from).collect::<Vec<_>>()),
        "p4sum" => p4sum(Path::new(&args[1])),
        "child" => child(Path::new(&args[1]), &s(2), Path::new(&args[3]), s(4) == "1"),
        "version" => {
            // SAFETY: static C string from LibRaw.
            let v = unsafe { std::ffi::CStr::from_ptr(pp_version()) };
            println!("LibRaw {}", v.to_string_lossy());
            Ok(())
        }
        _ => Err("usage: proto-phase1 p0 <out.csv> <files…> | p4b <out-dir> <threads,list> <files…> | p4 <out-dir> <group> <strategies,list> <files…> | p4sum <out-dir> | version".into()),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        let _ = std::io::stderr().flush();
        std::process::exit(1);
    }
}
