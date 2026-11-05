//! ADR 0008 P6: GPU demosaicing inside presenter B2 (section 13). Rust opens and unpacks the RAW
//! with LibRaw (CPU), copies the visible Bayer data (16 bit) into a WebView2 shared buffer and posts
//! it, together with the parameters dcraw_process would use (black levels, white balance, camera
//! matrix). The page uploads it as an integer texture and does scaling, demosaicing (bilinear as
//! LibRaw's "linear", and Malvar-He-Cutler), colour conversion and the output curve in WebGL2.
//!
//! - `--bench <out.json> <files…>`: per file, timings for image change and Z, and quality of the
//!   GPU result against LibRaw (AHD, PPG and linear) on the same region, with the phase 1 metrics.
//! - `--interactive <files…>`: RIGHT = next file, Z = fit / 100 % (latency.ps1).
//!
//! Not part of the app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../../../phase1/src/metrics.rs"]
#[allow(dead_code)]
mod metrics;

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use serde_json::{Value, json};
use tauri::{AppHandle, State, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, ICoreWebView2_2, ICoreWebView2_17, ICoreWebView2Environment12,
};
use windows_core::{Interface, PCWSTR};

// ---------- FFI: P6 shim ----------

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

// ---------- FFI: phase 1 shim (reference renders) ----------

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
    fn p6_open(path: *const u16, info: *mut P6Info) -> *mut c_void;
    fn p6_copy_raw(s: *mut c_void, dst: *mut u16) -> c_int;
    fn p6_close(s: *mut c_void);
    fn pp_open(path: *const u16, info: *mut PpInfo) -> *mut c_void;
    fn pp_render(s: *mut c_void, q: c_int, half: c_int, x: c_int, y: c_int, w: c_int, h: c_int, ri: *mut PpRenderInfo) -> *mut c_void;
    fn pp_image(img: *const c_void, data: *mut *const u8, size: *mut c_uint);
    fn pp_free(img: *mut c_void);
    fn pp_close(s: *mut c_void);
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

/// LibRaw reference render of a region (phase 1 shim): (rgb, w, h).
fn reference(path: &Path, quality: i32, r: [i32; 4]) -> Result<(Vec<u8>, usize, usize), String> {
    let w = wide(path);
    // SAFETY: as in phase 1; session closed below.
    unsafe {
        let mut info: PpInfo = std::mem::zeroed();
        let s = pp_open(w.as_ptr(), &mut info);
        if s.is_null() {
            return Err(c_str(&info.error));
        }
        let mut ri: PpRenderInfo = std::mem::zeroed();
        let img = pp_render(s, quality, 0, r[0], r[1], r[2], r[3], &mut ri);
        if img.is_null() {
            pp_close(s);
            return Err(c_str(&ri.error));
        }
        let (mut d, mut n) = (std::ptr::null(), 0u32);
        pp_image(img, &mut d, &mut n);
        let v = std::slice::from_raw_parts(d, n as usize).to_vec();
        pp_free(img);
        pp_close(s);
        Ok((v, ri.width as usize, ri.height as usize))
    }
}

struct App {
    files: Vec<PathBuf>,
    /// Reference renders per (file, quality, rect), cached for the quality runs.
    refs: Mutex<HashMap<(usize, i32, [i32; 4]), Arc<(Vec<u8>, usize, usize)>>>,
}

/// Opens + unpacks on a worker thread, then copies the Bayer data into a shared buffer and posts it
/// to the page on the UI thread. Returns timings and the parameters for the shader.
#[tauri::command]
async fn p6_load(win: WebviewWindow, app: State<'_, Arc<App>>, idx: usize) -> Result<Value, String> {
    let path = app.files.get(idx % app.files.len().max(1)).cloned().ok_or("no files")?;
    let t0 = Instant::now();
    let raw = tauri::async_runtime::spawn_blocking(move || open_raw(&path)).await.map_err(|e| e.to_string())??;
    let open_unpack_ms = ms(t0);
    let i = &raw.1;
    let meta = json!({
        "idx": idx,
        "w": i.width, "h": i.height, "flip": i.flip,
        "filters": i.filters, "colors": i.colors,
        "black": i.black, "scale": i.scale_mul, "maximum": i.maximum,
        "cam": [i.rgb_cam[0], i.rgb_cam[1], i.rgb_cam[2]],
        "open_ms": i.open_ms, "unpack_ms": i.unpack_ms,
    });
    let meta_s = meta.to_string();
    let (tx, rx) = mpsc::channel();
    win.with_webview(move |wv| {
        let r = (|| -> Result<Value, String> {
            let bytes = raw.1.width as u64 * raw.1.height as u64 * 2;
            // SAFETY: COM calls on the WebView2 UI thread; the buffer holds `bytes` bytes.
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
                p6_copy_raw(raw.0, p as *mut u16);
                let copy_ms = ms(t);
                drop(raw); // the unpacked LibRaw data is no longer needed
                let w: Vec<u16> = meta_s.encode_utf16().chain(std::iter::once(0)).collect();
                let t = Instant::now();
                core17
                    .PostSharedBufferToScript(&buf, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, PCWSTR(w.as_ptr()))
                    .map_err(|e| e.to_string())?;
                Ok(json!({ "create_ms": create_ms, "copy_ms": copy_ms, "post_ms": ms(t) }))
            }
        })();
        let _ = tx.send(r);
    })
    .map_err(|e| e.to_string())?;
    let mut r = rx.recv().map_err(|e| e.to_string())??;
    r["open_unpack_ms"] = json!(open_unpack_ms);
    r["meta"] = meta;
    Ok(r)
}

/// Quality: the page sends a GPU-rendered region (RGBA, top-down) as the raw body; headers carry
/// the file, rect and method. Compared with LibRaw AHD (reference), and, for "linear", with LibRaw's
/// own linear method (pipeline check).
#[tauri::command]
async fn p6_quality(app: State<'_, Arc<App>>, request: tauri::ipc::Request<'_>) -> Result<Value, String> {
    let tauri::ipc::InvokeBody::Raw(body) = request.body() else { return Err("expected raw body".into()) };
    let h = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(|s| s.to_string()).ok_or(format!("header {k}"));
    let idx: usize = h("p6-idx")?.parse().map_err(|_| "idx")?;
    let rect: Vec<i32> = h("p6-rect")?.split(',').filter_map(|v| v.parse().ok()).collect();
    let method = h("p6-method")?;
    let [x, y, w, hh] = [rect[0], rect[1], rect[2], rect[3]];
    let gpu_rgba = body.clone();
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Value, String> {
        let path = app.files[idx % app.files.len()].clone();
        // reference renders with a 16 px margin, cropped back (avoids LibRaw's border handling at
        // the region edge)
        const M: i32 = 16;
        let big = [x - M, y - M, w + 2 * M, hh + 2 * M];
        let get = |q: i32| -> Result<Arc<(Vec<u8>, usize, usize)>, String> {
            if let Some(r) = app.refs.lock().unwrap().get(&(idx, q, big)) {
                return Ok(r.clone());
            }
            let r = Arc::new(reference(&path, q, big)?);
            app.refs.lock().unwrap().insert((idx, q, big), r.clone());
            Ok(r)
        };
        let crop = |r: &(Vec<u8>, usize, usize)| -> Vec<u8> {
            let (bw, w, hh) = (r.1, w as usize, hh as usize);
            let mut o = Vec::with_capacity(w * hh * 3);
            for row in 0..hh {
                let s = ((row + M as usize) * bw + M as usize) * 3;
                o.extend_from_slice(&r.0[s..s + w * 3]);
            }
            o
        };
        let gpu: Vec<u8> = gpu_rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let (w, hh) = (w as usize, hh as usize);
        fn img(v: &[u8], w: usize, h: usize) -> metrics::Img<'_> {
            metrics::Img { w, h, rgb: v }
        }
        let rep = |r: metrics::Report| {
            json!({ "de00_mean": r.de00_mean, "de00_p99": r.de00_p99, "ssim": r.ssim, "ms_ssim": r.ms_ssim, "psnr_y": r.psnr_y,
                    "rise_ref_px": r.rise_ref_px, "rise_test_px": r.rise_test_px, "hf_ratio": r.hf_ratio,
                    "false_colour_ref": r.false_colour_ref, "false_colour_test": r.false_colour_test, "zipper_ratio": r.zipper_ratio })
        };
        let ahd = crop(&*get(3)?);
        let mut out = json!({ "idx": idx, "file": path.display().to_string(), "method": method, "rect": [x, y, w, hh],
                              "vs_ahd": rep(metrics::compare(img(&ahd, w, hh), img(&gpu, w, hh))) });
        if method == "linear" {
            let lin = crop(&*get(0)?);
            out["vs_libraw_linear"] = rep(metrics::compare(img(&lin, w, hh), img(&gpu, w, hh)));
        } else {
            let ppg = crop(&*get(2)?);
            out["ppg_vs_ahd"] = rep(metrics::compare(img(&ahd, w, hh), img(&ppg, w, hh)));
        }
        if let Some(dir) = arg_after("--bench").and_then(|p| Path::new(&p).parent().map(|d| d.join("crops"))) {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let _ = std::fs::create_dir_all(&dir);
            let _ = write_ppm(&dir.join(format!("{stem}-gpu-{method}.ppm")), w, hh, &gpu);
            let _ = write_ppm(&dir.join(format!("{stem}-libraw-ahd.ppm")), w, hh, &ahd);
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn write_ppm(p: &Path, w: usize, h: usize, rgb: &[u8]) -> std::io::Result<()> {
    let mut v = format!("P6\n{w} {h}\n255\n").into_bytes();
    v.extend_from_slice(rgb);
    std::fs::write(p, v)
}

#[tauri::command]
fn p6_info(app: State<'_, Arc<App>>) -> Value {
    json!({
        "files": app.files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>(),
        "bench": arg_after("--bench").is_some(),
        "interactive": std::env::args().any(|a| a == "--interactive"),
    })
}

#[tauri::command]
fn p6_mem() -> Value {
    sys::tree_private_mb()
}

#[tauri::command]
fn p6_write(json: String) -> Result<(), String> {
    let out = arg_after("--bench").ok_or("not in measurement mode")?;
    std::fs::write(&out, json).map_err(|e| format!("{out}: {e}"))
}

#[tauri::command]
fn p6_exit(app: AppHandle) {
    app.exit(0);
}

// ---------- memory of this process and its WebView2 processes (as in P2) ----------

mod sys {
    use serde_json::{Value, json};
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
        fn GetCurrentProcessId() -> u32;
    }

    pub fn tree_private_mb() -> Value {
        // SAFETY: plain Win32 calls with valid out-pointers; handles are closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(2, 0);
            if snap.is_null() || snap as isize == -1 {
                return json!({ "error": "snapshot" });
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
            let mut tree = vec![GetCurrentProcessId()];
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
            let mut total = 0.0;
            for pid in &tree {
                let h = OpenProcess(0x1000, 0, *pid);
                if h.is_null() {
                    continue;
                }
                let mut m = MemCountersEx { cb: std::mem::size_of::<MemCountersEx>() as u32, ..Default::default() };
                if K32GetProcessMemoryInfo(h, &mut m, m.cb) != 0 {
                    total += m.private_usage as f64 / 1048576.0;
                }
                CloseHandle(h);
            }
            json!({ "total_private_mb": (total * 10.0).round() / 10.0 })
        }
    }
}

fn main() {
    let files: Vec<PathBuf> = std::env::args()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file() && !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
        .collect();
    if files.is_empty() {
        eprintln!("p6: give RAW files");
        std::process::exit(2);
    }
    let app = Arc::new(App { files, refs: Mutex::new(HashMap::new()) });
    tauri::Builder::default()
        .manage(app)
        .invoke_handler(tauri::generate_handler![p6_info, p6_load, p6_quality, p6_mem, p6_write, p6_exit])
        .run(tauri::generate_context!())
        .expect("error while running p6");
}
