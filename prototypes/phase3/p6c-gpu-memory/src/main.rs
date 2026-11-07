//! ADR 0008 P6c: GPU PPG memory budget and texture-cache validation, in presenter B2-res.
//!
//! Built on P6b (same LibRaw shim, same PPG shader in the page). Rust is the core: it unpacks RAW
//! files on request (`p6c_load`), optionally keeps the unpacked Bayer data in a CPU tier (K2), and
//! posts it to the page through a WebView2 shared buffer. The page runs the P5 scheduling rules
//! with a GPU texture cache under a byte budget and drives the scenarios.
//!
//! Memory is measured in separate layers that are never added up blindly:
//! - L1: bytes the core and the page account for themselves (tier, in-flight unpacks, shared
//!   buffers; textures and pool, reported by the page);
//! - L2: private bytes and working set per process, by WebView2 process kind (GetProcessInfos);
//! - L3: GPU memory per process and per adapter (PDH "GPU Process Memory", "GPU Adapter Memory");
//! - L4: system commit and available memory (PDH "Memory"), only to check for double counting.
//!
//! A sampler thread records all layers every 250 ms; the page adds markers and settled snapshots.
//!
//! Usage: `p6c-gpu-memory --suite <m0|p5|nat-mix|stress|interactive> --out <dir> --tag <name>
//!   [--budget <MB>] [--upload U1|U2|U3|U4] [--half 1|0] [--tier <MB>] [--zoomed] <files…>`
//!
//! Not part of the app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, c_int, c_uint, c_void};
use std::fmt::Write as _;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_KIND, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, ICoreWebView2_2, ICoreWebView2_17,
    ICoreWebView2Environment8, ICoreWebView2Environment12,
};
use windows_core::{Interface, PCWSTR};

// ---------- FFI: P6 shim (unchanged) ----------

#[repr(C)]
struct P6Info {
    open_ms: f64,
    unpack_ms: f64,
    width: c_int,
    height: c_int,
    raw_width: c_int,
    raw_height: c_int,
    top_margin: c_int,
    left_margin: c_int,
    flip: c_int,
    filters: c_uint,
    colors: c_int,
    black: [f32; 4],
    scale_mul: [f32; 4],
    rgb_cam: [[f32; 3]; 3],
    maximum: f32,
    error: [c_char; 200],
}

unsafe extern "C" {
    fn p6_open(path: *const u16, info: *mut P6Info) -> *mut c_void;
    fn p6_copy_raw(s: *mut c_void, dst: *mut u16) -> c_int;
    fn p6_close(s: *mut c_void);
}

fn c_str(e: &[c_char]) -> String {
    let b: Vec<u8> = e.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&b).into_owned()
}

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn arg_after(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

const MB: f64 = 1048576.0;

/// An opened and unpacked RAW (P6 shim).
struct Raw(*mut c_void, P6Info);
// SAFETY: one LibRaw instance, used by one thread at a time (moved, never shared).
unsafe impl Send for Raw {}
impl Drop for Raw {
    fn drop(&mut self) {
        // SAFETY: opened by p6_open, closed once.
        unsafe { p6_close(self.0) };
    }
}

fn open_raw(path: &Path) -> Result<Raw, String> {
    let w = wide(path);
    // SAFETY: zeroed POD out-struct; valid wide path.
    let mut info: P6Info = unsafe { std::mem::zeroed() };
    let s = unsafe { p6_open(w.as_ptr(), &mut info) };
    if s.is_null() {
        return Err(format!("{}: {}", path.display(), c_str(&info.error)));
    }
    Ok(Raw(s, info))
}

fn meta_of(pos: usize, i: &P6Info) -> Value {
    json!({
        "idx": pos,
        "w": i.width, "h": i.height, "flip": i.flip,
        "filters": i.filters, "colors": i.colors,
        "black": i.black, "scale": i.scale_mul, "maximum": i.maximum,
        "cam": [i.rgb_cam[0], i.rgb_cam[1], i.rgb_cam[2]],
        "open_ms": i.open_ms, "unpack_ms": i.unpack_ms,
    })
}

// ---------- configuration ----------

#[derive(Clone)]
struct Cfg {
    files: Vec<PathBuf>,
    out: PathBuf,
    suite: String,
    tag: String,
    budget_mb: f64,
    upload: String,
    half: bool,
    tier_mb: f64,
    zoomed: bool,
}

impl Cfg {
    fn from_args() -> Cfg {
        let a: Vec<String> = std::env::args().collect();
        // values that follow an option are not files
        let mut skip = std::collections::HashSet::new();
        for (i, x) in a.iter().enumerate() {
            if ["--suite", "--out", "--tag", "--budget", "--upload", "--half", "--tier"].contains(&x.as_str()) {
                skip.insert(i + 1);
            }
        }
        let files = a
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(i, x)| !skip.contains(i) && !x.starts_with("--"))
            .map(|(_, x)| PathBuf::from(x))
            .filter(|p| p.is_file())
            .collect();
        Cfg {
            files,
            out: PathBuf::from(arg_after("--out").unwrap_or_else(|| ".".into())),
            suite: arg_after("--suite").unwrap_or_else(|| "interactive".into()),
            tag: arg_after("--tag").unwrap_or_else(|| "run".into()),
            budget_mb: arg_after("--budget").and_then(|v| v.parse().ok()).unwrap_or(1400.0),
            upload: arg_after("--upload").unwrap_or_else(|| "U1".into()),
            half: arg_after("--half").map(|v| v != "0").unwrap_or(true),
            tier_mb: arg_after("--tier").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            zoomed: std::env::args().any(|a| a == "--zoomed"),
        }
    }
}

// ---------- core state (L1) ----------

/// K2: unpacked Bayer data kept on the CPU (LRU, byte budget), keyed by position.
#[derive(Default)]
struct Tier {
    map: HashMap<usize, (Value, Arc<Vec<u16>>)>,
    lru: VecDeque<usize>,
    bytes: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
}

#[derive(Default)]
struct Core {
    tier: Tier,
    /// estimated bytes of unpacks in progress (LibRaw's raw buffer, w × h × 2)
    inflight: usize,
    last_raw_bytes: usize,
    /// shared buffers created and not yet released by the page
    shared: HashMap<usize, usize>,
    unpacks: usize,
}

struct App {
    cfg: Cfg,
    core: Mutex<Core>,
    /// pid -> WebView2 process kind (0 browser, 1 renderer, 2 utility, 3 sandbox helper, 4 GPU, ...)
    kinds: Mutex<HashMap<u32, i32>>,
    /// (texture MB, pool MB) as reported by the page
    page: Mutex<(f64, f64)>,
    mark: Mutex<String>,
    rows: Mutex<Vec<String>>,
    last: Mutex<Value>,
    t0: Instant,
}

impl App {
    fn core_mb(&self) -> (f64, f64, f64) {
        let c = self.core.lock().unwrap();
        (c.tier.bytes as f64 / MB, c.inflight as f64 / MB, c.shared.values().sum::<usize>() as f64 / MB)
    }
}

/// Unpacks position `pos` (or takes it from the CPU tier), copies the Bayer data into a shared
/// buffer and posts it to the page. Returns timings and the shader parameters.
#[tauri::command]
async fn p6c_load(win: WebviewWindow, app: State<'_, Arc<App>>, pos: usize) -> Result<Value, String> {
    let app = app.inner().clone();
    let path = app.cfg.files.get(pos % app.cfg.files.len().max(1)).cloned().ok_or("no files")?;
    let t0 = Instant::now();
    let tier_on = app.cfg.tier_mb > 0.0;
    // the source of the Bayer data: a fresh LibRaw unpack or the CPU tier
    enum Src {
        Raw(Raw),
        Tier(Arc<Vec<u16>>),
    }
    let cached = if tier_on {
        let mut c = app.core.lock().unwrap();
        let hit = c.tier.map.get(&pos).cloned();
        if hit.is_some() {
            c.tier.hits += 1;
            c.tier.lru.retain(|x| *x != pos);
            c.tier.lru.push_back(pos);
        } else {
            c.tier.misses += 1;
        }
        hit
    } else {
        None
    };
    let (meta, src, unpack_ms) = match cached {
        Some((meta, data)) => (meta, Src::Tier(data), 0.0),
        None => {
            let est = {
                let mut c = app.core.lock().unwrap();
                let e = if c.last_raw_bytes > 0 { c.last_raw_bytes } else { 128 << 20 };
                c.inflight += e;
                c.unpacks += 1;
                e
            };
            let a2 = app.clone();
            let r = tauri::async_runtime::spawn_blocking(move || -> Result<(Value, Src), String> {
                let raw = open_raw(&path)?;
                let meta = meta_of(pos, &raw.1);
                let n = raw.1.width as usize * raw.1.height as usize;
                if a2.cfg.tier_mb > 0.0 {
                    let mut v = vec![0u16; n];
                    // SAFETY: v holds width × height values, as p6_copy_raw writes.
                    unsafe { p6_copy_raw(raw.0, v.as_mut_ptr()) };
                    drop(raw);
                    let data = Arc::new(v);
                    let mut c = a2.core.lock().unwrap();
                    let budget = (a2.cfg.tier_mb * MB) as usize;
                    while c.tier.bytes + n * 2 > budget {
                        let Some(k) = c.tier.lru.pop_front() else { break };
                        if let Some((_, d)) = c.tier.map.remove(&k) {
                            c.tier.bytes -= d.len() * 2;
                            c.tier.evictions += 1;
                        }
                    }
                    if c.tier.bytes + n * 2 <= budget {
                        c.tier.map.insert(pos, (meta.clone(), data.clone()));
                        c.tier.lru.push_back(pos);
                        c.tier.bytes += n * 2;
                    }
                    Ok((meta, Src::Tier(data)))
                } else {
                    Ok((meta, Src::Raw(raw)))
                }
            })
            .await
            .map_err(|e| e.to_string());
            let mut c = app.core.lock().unwrap();
            c.inflight = c.inflight.saturating_sub(est);
            let (meta, src) = r??;
            c.last_raw_bytes = meta["w"].as_u64().unwrap_or(0) as usize * meta["h"].as_u64().unwrap_or(0) as usize * 2;
            (meta, src, ms(t0))
        }
    };
    let mut meta = meta;
    meta["idx"] = json!(pos);
    let meta_s = meta.to_string();
    let bytes = meta["w"].as_u64().unwrap_or(0) * meta["h"].as_u64().unwrap_or(0) * 2;
    let (tx, rx) = mpsc::channel();
    let a3 = app.clone();
    win.with_webview(move |wv| {
        let r = (|| -> Result<Value, String> {
            // SAFETY: COM calls on the WebView2 UI thread; the buffer holds `bytes` bytes and the
            // copy writes exactly width × height u16 values.
            unsafe {
                let core = wv.controller().CoreWebView2().map_err(|e| e.to_string())?;
                let core17: ICoreWebView2_17 = core.cast().map_err(|e| e.to_string())?;
                let env: ICoreWebView2Environment12 = core
                    .cast::<ICoreWebView2_2>()
                    .and_then(|c| c.Environment())
                    .and_then(|e| e.cast())
                    .map_err(|e| e.to_string())?;
                let t = Instant::now();
                let buf = env.CreateSharedBuffer(bytes).map_err(|e| format!("CreateSharedBuffer: {e}"))?;
                let create_ms = ms(t);
                let mut p: *mut u8 = std::ptr::null_mut();
                buf.Buffer(&mut p).map_err(|e| e.to_string())?;
                let t = Instant::now();
                match src {
                    Src::Raw(raw) => {
                        p6_copy_raw(raw.0, p as *mut u16);
                        drop(raw);
                    }
                    Src::Tier(d) => std::ptr::copy_nonoverlapping(d.as_ptr(), p as *mut u16, d.len()),
                }
                let copy_ms = ms(t);
                a3.core.lock().unwrap().shared.insert(pos, bytes as usize);
                let w: Vec<u16> = meta_s.encode_utf16().chain(std::iter::once(0)).collect();
                core17
                    .PostSharedBufferToScript(&buf, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, PCWSTR(w.as_ptr()))
                    .map_err(|e| e.to_string())?;
                Ok(json!({ "create_ms": create_ms, "copy_ms": copy_ms }))
            }
        })();
        let _ = tx.send(r);
    })
    .map_err(|e| e.to_string())?;
    let mut r = rx.recv().map_err(|e| e.to_string())??;
    r["unpack_ms"] = json!(unpack_ms);
    r["tier_hit"] = json!(unpack_ms == 0.0 && tier_on);
    r["meta"] = meta;
    Ok(r)
}

/// The page released the shared buffer of `pos`.
#[tauri::command]
fn p6c_released(app: State<'_, Arc<App>>, pos: usize) {
    app.core.lock().unwrap().shared.remove(&pos);
}

/// The page reports its texture and pool bytes (L1).
#[tauri::command]
fn p6c_tex(app: State<'_, Arc<App>>, tex_mb: f64, pool_mb: f64) {
    *app.page.lock().unwrap() = (tex_mb, pool_mb);
}

#[tauri::command]
fn p6c_mark(app: State<'_, Arc<App>>, label: String) {
    *app.mark.lock().unwrap() = label;
}

/// Sets a marker, waits `settle_ms` (PDH updates about once a second) and returns the latest sample.
#[tauri::command]
async fn p6c_snap(app: State<'_, Arc<App>>, label: String, settle_ms: u64) -> Result<Value, String> {
    *app.mark.lock().unwrap() = label;
    let a = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(Duration::from_millis(settle_ms.max(300)));
        a.last.lock().unwrap().clone()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Process kinds from WebView2 (GetProcessInfos); refreshed on request.
#[tauri::command]
async fn p6c_kinds(win: WebviewWindow, app: State<'_, Arc<App>>) -> Result<Value, String> {
    let (tx, rx) = mpsc::channel();
    win.with_webview(move |wv| {
        let r = (|| -> Result<Vec<(u32, i32)>, String> {
            // SAFETY: COM calls on the WebView2 UI thread with valid out-pointers.
            unsafe {
                let core = wv.controller().CoreWebView2().map_err(|e| e.to_string())?;
                let env: ICoreWebView2Environment8 = core
                    .cast::<ICoreWebView2_2>()
                    .and_then(|c| c.Environment())
                    .and_then(|e| e.cast())
                    .map_err(|e| e.to_string())?;
                let coll = env.GetProcessInfos().map_err(|e| e.to_string())?;
                let mut n = 0u32;
                coll.Count(&mut n).map_err(|e| e.to_string())?;
                let mut v = Vec::new();
                for i in 0..n {
                    let pi = coll.GetValueAtIndex(i).map_err(|e| e.to_string())?;
                    let mut pid = 0i32;
                    pi.ProcessId(&mut pid).map_err(|e| e.to_string())?;
                    let mut k = COREWEBVIEW2_PROCESS_KIND::default();
                    pi.Kind(&mut k).map_err(|e| e.to_string())?;
                    v.push((pid as u32, k.0));
                }
                Ok(v)
            }
        })();
        let _ = tx.send(r);
    })
    .map_err(|e| e.to_string())?;
    let v = rx.recv().map_err(|e| e.to_string())??;
    let mut k = app.kinds.lock().unwrap();
    k.clear();
    for (pid, kind) in &v {
        k.insert(*pid, *kind);
    }
    Ok(json!(v.iter().map(|(p, k)| json!({ "pid": p, "kind": k })).collect::<Vec<_>>()))
}

#[tauri::command]
fn p6c_info(app: State<'_, Arc<App>>) -> Value {
    let c = &app.cfg;
    json!({
        "files": c.files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>(),
        "suite": c.suite, "tag": c.tag, "budget_mb": c.budget_mb, "upload": c.upload,
        "half": c.half, "tier_mb": c.tier_mb, "zoomed": c.zoomed,
    })
}

/// Core counters (L1) for the result file.
#[tauri::command]
fn p6c_core(app: State<'_, Arc<App>>) -> Value {
    let c = app.core.lock().unwrap();
    json!({
        "tier_mb": c.tier.bytes as f64 / MB, "tier_hits": c.tier.hits, "tier_misses": c.tier.misses,
        "tier_evictions": c.tier.evictions, "unpacks": c.unpacks,
        "cpu_app_ms": sys::cpu_ms(&[sys::own_pid()]), "cpu_tree_ms": sys::cpu_ms(&sys::tree_pids()),
        "t_ms": ms(app.t0),
    })
}

#[tauri::command]
fn p6c_write(app: State<'_, Arc<App>>, name: String, text: String) -> Result<(), String> {
    let p = app.cfg.out.join(name);
    std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
}

/// Writes the sampled memory series as CSV.
#[tauri::command]
fn p6c_dump(app: State<'_, Arc<App>>) -> Result<(), String> {
    let rows = app.rows.lock().unwrap();
    let mut s = String::from(sampler::HEADER);
    s.push('\n');
    for r in rows.iter() {
        s.push_str(r);
        s.push('\n');
    }
    let p = app.cfg.out.join(format!("{}-mem.csv", app.cfg.tag));
    std::fs::write(&p, s).map_err(|e| format!("{}: {e}", p.display()))
}

#[tauri::command]
fn p6c_exit(app: AppHandle) {
    app.exit(0);
}

// ---------- sampler: L2, L3, L4 every 250 ms ----------

mod sampler {
    use super::*;

    pub const HEADER: &str = "t_ms,mark,app_priv,app_ws,browser_priv,renderer_priv,renderer_ws,gpuproc_priv,gpuproc_ws,\
utility_priv,other_priv,tree_priv,gpuproc_ded,gpuproc_shared,gpuproc_commit,tree_ded,tree_shared,tree_commit,\
adapter_ded,adapter_shared,sys_commit,sys_avail,core_tier,core_inflight,core_shared,page_tex,page_pool,n_procs";

    pub fn run(app: Arc<App>) {
        let mut q = gpu::Query::new();
        loop {
            let t = Instant::now();
            let own = sys::own_pid();
            let pids = sys::tree_pids();
            let kinds = app.kinds.lock().unwrap().clone();
            // L2 per kind: (private, working set)
            let mut by: HashMap<&str, (f64, f64)> = HashMap::new();
            let kind_of = |pid: u32| -> &'static str {
                if pid == own {
                    return "app";
                }
                match kinds.get(&pid) {
                    Some(0) => "browser",
                    Some(1) => "renderer",
                    Some(4) => "gpuproc",
                    Some(2) | Some(3) => "utility",
                    _ => "other",
                }
            };
            let mut tree_priv = 0.0;
            for pid in &pids {
                if let Some((p, w)) = sys::mem_mb(*pid) {
                    let e = by.entry(kind_of(*pid)).or_default();
                    e.0 += p;
                    e.1 += w;
                    tree_priv += p;
                }
            }
            // L3 / L4
            let g = q.collect();
            let sum_pid = |m: &HashMap<u32, f64>, f: &dyn Fn(u32) -> bool| -> f64 { m.iter().filter(|(p, _)| f(**p)).map(|(_, v)| *v).sum() };
            let gpu_pids: Vec<u32> = pids.iter().copied().filter(|p| kind_of(*p) == "gpuproc").collect();
            let in_tree = |p: u32| pids.contains(&p);
            let is_gpu = |p: u32| gpu_pids.contains(&p);
            let (tier, inflight, shared) = app.core_mb();
            let (tex, pool) = *app.page.lock().unwrap();
            let mark = app.mark.lock().unwrap().replace(',', ";");
            let get = |k: &str| by.get(k).copied().unwrap_or_default();
            let vals = json!({
                "t_ms": ms(app.t0), "mark": mark,
                "app_priv": get("app").0, "app_ws": get("app").1,
                "browser_priv": get("browser").0,
                "renderer_priv": get("renderer").0, "renderer_ws": get("renderer").1,
                "gpuproc_priv": get("gpuproc").0, "gpuproc_ws": get("gpuproc").1,
                "utility_priv": get("utility").0, "other_priv": get("other").0,
                "tree_priv": tree_priv,
                "gpuproc_ded": sum_pid(&g.ded, &is_gpu), "gpuproc_shared": sum_pid(&g.shared, &is_gpu),
                "gpuproc_commit": sum_pid(&g.commit, &is_gpu),
                "tree_ded": sum_pid(&g.ded, &in_tree), "tree_shared": sum_pid(&g.shared, &in_tree),
                "tree_commit": sum_pid(&g.commit, &in_tree),
                "adapter_ded": g.adapter_ded, "adapter_shared": g.adapter_shared,
                "sys_commit": g.sys_commit, "sys_avail": g.sys_avail,
                "core_tier": tier, "core_inflight": inflight, "core_shared": shared,
                "page_tex": tex, "page_pool": pool, "n_procs": pids.len(),
            });
            let mut row = String::new();
            for (i, k) in HEADER.split(',').enumerate() {
                if i > 0 {
                    row.push(',');
                }
                match &vals[k] {
                    Value::String(s) => row.push_str(s),
                    Value::Number(n) => {
                        let f = n.as_f64().unwrap_or(f64::NAN);
                        let _ = write!(row, "{f:.1}");
                    }
                    _ => {}
                }
            }
            app.rows.lock().unwrap().push(row);
            *app.last.lock().unwrap() = vals;
            let spent = t.elapsed();
            std::thread::sleep(Duration::from_millis(250).saturating_sub(spent));
        }
    }
}

// ---------- processes: tree, memory, CPU time ----------

mod sys {
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessEntry {
        size: u32,
        usage: u32,
        pid: u32,
        heap_id: usize,
        module_id: u32,
        threads: u32,
        parent: u32,
        pri: i32,
        flags: u32,
        exe: [u16; 260],
    }
    #[repr(C)]
    #[derive(Default)]
    struct MemCountersEx {
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
        private_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Process32FirstW(snap: *mut c_void, e: *mut ProcessEntry) -> i32;
        fn Process32NextW(snap: *mut c_void, e: *mut ProcessEntry) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(h: *mut c_void) -> i32;
        fn K32GetProcessMemoryInfo(p: *mut c_void, m: *mut MemCountersEx, cb: u32) -> i32;
        fn GetProcessTimes(p: *mut c_void, c: *mut u64, e: *mut u64, k: *mut u64, u: *mut u64) -> i32;
        fn GetCurrentProcessId() -> u32;
    }

    pub fn own_pid() -> u32 {
        // SAFETY: no arguments.
        unsafe { GetCurrentProcessId() }
    }

    /// This process and all its descendants (the WebView2 processes).
    pub fn tree_pids() -> Vec<u32> {
        // SAFETY: plain Win32 calls with valid out-pointers; handles are closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(2, 0);
            if snap.is_null() || snap as isize == -1 {
                return vec![own_pid()];
            }
            let mut all: Vec<(u32, u32)> = Vec::new();
            let mut e: ProcessEntry = std::mem::zeroed();
            e.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                all.push((e.pid, e.parent));
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
            let mut tree = vec![own_pid()];
            let mut i = 0;
            while i < tree.len() {
                let p = tree[i];
                for (pid, parent) in &all {
                    if *parent == p && !tree.contains(pid) {
                        tree.push(*pid);
                    }
                }
                i += 1;
            }
            tree
        }
    }

    /// (private bytes, working set) in MB.
    pub fn mem_mb(pid: u32) -> Option<(f64, f64)> {
        // SAFETY: plain Win32 calls with a valid out-struct; the handle is closed.
        unsafe {
            let h = OpenProcess(0x1000, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut m = MemCountersEx { cb: std::mem::size_of::<MemCountersEx>() as u32, ..Default::default() };
            let ok = K32GetProcessMemoryInfo(h, &mut m, m.cb) != 0;
            CloseHandle(h);
            ok.then(|| (m.private_usage as f64 / super::MB, m.working_set as f64 / super::MB))
        }
    }

    /// Kernel + user CPU time of the given processes, in ms.
    pub fn cpu_ms(pids: &[u32]) -> f64 {
        let mut total = 0.0;
        for pid in pids {
            // SAFETY: plain Win32 calls with valid out-pointers; the handle is closed.
            unsafe {
                let h = OpenProcess(0x1000, 0, *pid);
                if h.is_null() {
                    continue;
                }
                let (mut c, mut e, mut k, mut u) = (0u64, 0u64, 0u64, 0u64);
                if GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0 {
                    total += (k + u) as f64 / 10_000.0;
                }
                CloseHandle(h);
            }
        }
        total
    }
}

// ---------- PDH: GPU memory per process and adapter, system memory ----------

mod gpu {
    use std::collections::HashMap;

    #[link(name = "pdh")]
    unsafe extern "system" {
        fn PdhOpenQueryW(src: *const u16, user: usize, q: *mut isize) -> u32;
        fn PdhAddEnglishCounterW(q: isize, path: *const u16, user: usize, c: *mut isize) -> u32;
        fn PdhCollectQueryData(q: isize) -> u32;
        fn PdhGetFormattedCounterArrayW(c: isize, fmt: u32, size: *mut u32, count: *mut u32, buf: *mut u8) -> u32;
        fn PdhGetFormattedCounterValue(c: isize, fmt: u32, typ: *mut u32, v: *mut FmtValue) -> u32;
    }
    #[repr(C)]
    struct Item {
        name: *const u16,
        status: u32,
        _pad: u32,
        value: i64,
    }
    #[repr(C)]
    struct FmtValue {
        status: u32,
        _pad: u32,
        value: i64,
    }
    const PDH_FMT_LARGE: u32 = 0x400;
    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[derive(Default)]
    pub struct Sample {
        pub ded: HashMap<u32, f64>,
        pub shared: HashMap<u32, f64>,
        pub commit: HashMap<u32, f64>,
        pub adapter_ded: f64,
        pub adapter_shared: f64,
        pub sys_commit: f64,
        pub sys_avail: f64,
    }

    /// A persistent PDH query; wildcard instances are expanded on every collection.
    pub struct Query {
        q: isize,
        c: [isize; 7],
    }

    impl Query {
        pub fn new() -> Query {
            let mut q = Query { q: 0, c: [0; 7] };
            let paths = [
                "\\GPU Process Memory(*)\\Dedicated Usage",
                "\\GPU Process Memory(*)\\Shared Usage",
                "\\GPU Process Memory(*)\\Total Committed",
                "\\GPU Adapter Memory(*)\\Dedicated Usage",
                "\\GPU Adapter Memory(*)\\Shared Usage",
                "\\Memory\\Committed Bytes",
                "\\Memory\\Available MBytes",
            ];
            // SAFETY: PDH calls with valid out-pointers; the query lives as long as the sampler.
            unsafe {
                if PdhOpenQueryW(std::ptr::null(), 0, &mut q.q) != 0 {
                    q.q = 0;
                    return q;
                }
                for (i, p) in paths.iter().enumerate() {
                    let w = wide(p);
                    PdhAddEnglishCounterW(q.q, w.as_ptr(), 0, &mut q.c[i]);
                }
            }
            q
        }

        fn array(&self, c: isize) -> Vec<(String, f64)> {
            let mut out = Vec::new();
            // SAFETY: buffer sized as PDH asks; item names point into it.
            unsafe {
                let (mut size, mut count) = (0u32, 0u32);
                if PdhGetFormattedCounterArrayW(c, PDH_FMT_LARGE, &mut size, &mut count, std::ptr::null_mut()) != PDH_MORE_DATA {
                    return out;
                }
                let mut buf = vec![0u8; size as usize + 64];
                if PdhGetFormattedCounterArrayW(c, PDH_FMT_LARGE, &mut size, &mut count, buf.as_mut_ptr()) != 0 {
                    return out;
                }
                let items = std::slice::from_raw_parts(buf.as_ptr() as *const Item, count as usize);
                for it in items {
                    let mut n = 0;
                    while *it.name.add(n) != 0 {
                        n += 1;
                    }
                    out.push((String::from_utf16_lossy(std::slice::from_raw_parts(it.name, n)), it.value as f64));
                }
            }
            out
        }

        fn single(&self, c: isize) -> f64 {
            let mut v = FmtValue { status: 0, _pad: 0, value: 0 };
            let mut t = 0u32;
            // SAFETY: valid counter handle and out-pointers.
            if unsafe { PdhGetFormattedCounterValue(c, PDH_FMT_LARGE, &mut t, &mut v) } != 0 {
                return f64::NAN;
            }
            v.value as f64
        }

        pub fn collect(&mut self) -> Sample {
            let mut s = Sample::default();
            if self.q == 0 {
                return s;
            }
            // SAFETY: valid query handle.
            unsafe { PdhCollectQueryData(self.q) };
            // instance names look like "pid_1234_luid_0x..._phys_0"
            let per_pid = |v: Vec<(String, f64)>| -> HashMap<u32, f64> {
                let mut m = HashMap::new();
                for (name, val) in v {
                    if let Some(pid) = name.strip_prefix("pid_").and_then(|r| r.split('_').next()).and_then(|p| p.parse::<u32>().ok()) {
                        *m.entry(pid).or_insert(0.0) += val / super::MB;
                    }
                }
                m
            };
            s.ded = per_pid(self.array(self.c[0]));
            s.shared = per_pid(self.array(self.c[1]));
            s.commit = per_pid(self.array(self.c[2]));
            s.adapter_ded = self.array(self.c[3]).iter().map(|x| x.1).sum::<f64>() / super::MB;
            s.adapter_shared = self.array(self.c[4]).iter().map(|x| x.1).sum::<f64>() / super::MB;
            s.sys_commit = self.single(self.c[5]) / super::MB;
            s.sys_avail = self.single(self.c[6]);
            s
        }
    }
}

fn main() {
    let cfg = Cfg::from_args();
    if cfg.files.is_empty() {
        eprintln!("p6c: give RAW files");
        std::process::exit(2);
    }
    let _ = std::fs::create_dir_all(&cfg.out);
    let app = Arc::new(App {
        cfg,
        core: Mutex::new(Core::default()),
        kinds: Mutex::new(HashMap::new()),
        page: Mutex::new((0.0, 0.0)),
        mark: Mutex::new("start".into()),
        rows: Mutex::new(Vec::new()),
        last: Mutex::new(json!({})),
        t0: Instant::now(),
    });
    let a2 = app.clone();
    std::thread::spawn(move || sampler::run(a2));
    tauri::Builder::default()
        .manage(app)
        .setup(|a| {
            let _ = a.get_webview_window("main");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            p6c_info, p6c_load, p6c_released, p6c_tex, p6c_mark, p6c_snap, p6c_kinds, p6c_core, p6c_write, p6c_dump, p6c_exit
        ])
        .run(tauri::generate_context!())
        .expect("error while running p6c");
}
