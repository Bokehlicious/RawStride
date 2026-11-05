//! ADR 0008 phase 2, P2: presenter B2. Finished pixels go from Rust to the page through a WebView2
//! shared buffer (`CreateSharedBuffer` + `PostSharedBufferToScript`) and are uploaded to WebGL2
//! (and WebGPU when the runtime has it). For comparison, B1 serves the same raw RGBA bytes over a
//! custom URI scheme (`fetch`), like today's `pw://`.
//!
//! The images stand in for the finished views of the phase 1 core:
//! - `v1080` / `v1440`: 100 % views of 1920×1080 and 2560×1440 RGBA (8 and 15 MB);
//! - `full`: a whole A1 preview, 8640×5760 RGBA (199 MB).
//!
//! Modes (from the command line):
//! - `--bench <out.json>`: the page runs the fixed measurement (web/p2.js), writes it and exits;
//! - `--interactive <path> <kind>`: arrow keys step through images over one path, for the
//!   on-screen latency measurement with latency.ps1.
//!
//! Not part of the app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use serde_json::{Value, json};
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, ICoreWebView2_2, ICoreWebView2_17, ICoreWebView2Environment12,
    ICoreWebView2SharedBuffer,
};
use windows_core::{Interface, PCWSTR};

struct Img {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

struct Data {
    imgs: HashMap<&'static str, Vec<Img>>,
    sources: Vec<String>,
}

fn arg_after(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

// ---------- test images ----------

fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(rgb.len() / 3 * 4);
    for p in rgb.chunks_exact(3) {
        o.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    o
}

fn load_preview(path: &Path) -> Result<Img, String> {
    let info = pw_raw::inspect(path).map_err(|e| e.to_string())?;
    let p = info.pick(pw_raw::PreviewSize::Large).cloned().ok_or("no preview")?;
    let data = pw_raw::read_preview(path, &p).map_err(|e| e.to_string())?;
    let mut d = zune_jpeg::JpegDecoder::new(&data);
    let rgb = d.decode().map_err(|e| format!("{e:?}"))?;
    let (w, h) = d.dimensions().ok_or("no size")?;
    if rgb.len() != w * h * 3 {
        return Err(format!("{}: unexpected decoder output", path.display()));
    }
    Ok(Img { w: w as u32, h: h as u32, rgba: rgb_to_rgba(&rgb) })
}

fn crop(src: &Img, x: u32, y: u32, w: u32, h: u32) -> Img {
    let mut o = Vec::with_capacity((w * h * 4) as usize);
    for r in y..y + h {
        let s = ((r * src.w + x) * 4) as usize;
        o.extend_from_slice(&src.rgba[s..s + (w * 4) as usize]);
    }
    Img { w, h, rgba: o }
}

/// Two full A1 previews, and 6 different 100 % crops of each view size taken from them.
fn prepare(files: &[String]) -> Result<Data, String> {
    let mut full = Vec::new();
    for f in files.iter().take(2) {
        full.push(load_preview(Path::new(f))?);
    }
    if full.is_empty() {
        return Err("no input files".into());
    }
    let mut imgs: HashMap<&'static str, Vec<Img>> = HashMap::new();
    for (kind, w, h) in [("v1080", 1920u32, 1080u32), ("v1440", 2560, 1440)] {
        let mut v = Vec::new();
        for i in 0..6 {
            let s = &full[i % full.len()];
            let (w, h) = (w.min(s.w), h.min(s.h));
            // spread the crops over the image so consecutive views differ clearly
            let fx = [0.15, 0.5, 0.8][i / full.len() % 3];
            let fy = [0.3, 0.6, 0.45][i / full.len() % 3];
            let x = ((s.w - w) as f64 * fx) as u32;
            let y = ((s.h - h) as f64 * fy) as u32;
            v.push(crop(s, x, y, w, h));
        }
        imgs.insert(kind, v);
    }
    imgs.insert("full", full);
    Ok(Data { imgs, sources: files.iter().take(2).cloned().collect() })
}

// ---------- B2: shared buffer ----------

thread_local! {
    /// Pre-filled shared buffers ("pool"), created on the UI thread and only used there.
    static POOL: RefCell<HashMap<(String, usize), ICoreWebView2SharedBuffer>> = RefCell::new(HashMap::new());
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Creates a shared buffer and copies the image into it. UI thread only.
fn make_buffer(env: &ICoreWebView2Environment12, img: &Img) -> Result<(ICoreWebView2SharedBuffer, f64, f64), String> {
    let t = Instant::now();
    // SAFETY: COM calls on the WebView2 UI thread; the buffer is at least `len` bytes.
    unsafe {
        let buf = env.CreateSharedBuffer(img.rgba.len() as u64).map_err(|e| format!("CreateSharedBuffer: {e}"))?;
        let create = ms(t);
        let t = Instant::now();
        let mut p: *mut u8 = std::ptr::null_mut();
        buf.Buffer(&mut p).map_err(|e| format!("Buffer: {e}"))?;
        if p.is_null() {
            return Err("Buffer: null".into());
        }
        std::ptr::copy_nonoverlapping(img.rgba.as_ptr(), p, img.rgba.len());
        Ok((buf, create, ms(t)))
    }
}

fn post_on_ui(wv: &tauri::webview::PlatformWebview, data: &Data, kind: &str, idx: usize, pool: bool) -> Result<Value, String> {
    let t0 = Instant::now();
    let img = data.imgs.get(kind).and_then(|v| v.get(idx)).ok_or("no such image")?;
    // SAFETY: COM calls on the WebView2 UI thread (with_webview runs there).
    unsafe {
        let core = wv.controller().CoreWebView2().map_err(|e| format!("CoreWebView2: {e}"))?;
        let core17: ICoreWebView2_17 = core.cast().map_err(|e| format!("ICoreWebView2_17: {e}"))?;
        let env: ICoreWebView2Environment12 = core
            .cast::<ICoreWebView2_2>()
            .and_then(|c| c.Environment())
            .and_then(|e| e.cast())
            .map_err(|e| format!("ICoreWebView2Environment12: {e}"))?;
        let key = (kind.to_string(), idx);
        let (buf, create_ms, copy_ms) = if pool {
            match POOL.with(|p| p.borrow().get(&key).cloned()) {
                Some(b) => (b, 0.0, 0.0),
                None => {
                    let r = make_buffer(&env, img)?;
                    POOL.with(|p| p.borrow_mut().insert(key, r.0.clone()));
                    r
                }
            }
        } else {
            make_buffer(&env, img)?
        };
        let meta = wide(&json!({ "kind": kind, "idx": idx, "w": img.w, "h": img.h }).to_string());
        let t = Instant::now();
        core17
            .PostSharedBufferToScript(&buf, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, PCWSTR(meta.as_ptr()))
            .map_err(|e| format!("PostSharedBufferToScript: {e}"))?;
        let post_ms = ms(t);
        // "new": our reference goes now; the memory is freed when the page releases its ArrayBuffer.
        drop(buf);
        Ok(json!({ "create_ms": create_ms, "copy_ms": copy_ms, "post_ms": post_ms, "rust_ms": ms(t0) }))
    }
}

/// Runs `f` on the UI thread with the platform webview and waits for its result.
fn on_ui<T: Send + 'static>(
    win: &WebviewWindow,
    f: impl FnOnce(&tauri::webview::PlatformWebview) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    win.with_webview(move |wv| {
        let _ = tx.send(f(&wv));
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

#[tauri::command]
async fn p2_post(win: WebviewWindow, data: State<'_, Arc<Data>>, kind: String, idx: usize, pool: bool) -> Result<Value, String> {
    let d = data.inner().clone();
    on_ui(&win, move |wv| post_on_ui(wv, &d, &kind, idx, pool))
}

/// Fills the pool for every image (done before the timed "pool" runs).
#[tauri::command]
async fn p2_fill_pool(win: WebviewWindow, data: State<'_, Arc<Data>>) -> Result<Value, String> {
    let d = data.inner().clone();
    on_ui(&win, move |wv| {
        // SAFETY: COM calls on the UI thread.
        let env: ICoreWebView2Environment12 = unsafe {
            wv.controller()
                .CoreWebView2()
                .and_then(|c| c.cast::<ICoreWebView2_2>())
                .and_then(|c| c.Environment())
                .and_then(|e| e.cast())
                .map_err(|e| e.to_string())?
        };
        let mut n = 0;
        for (kind, v) in &d.imgs {
            for (i, img) in v.iter().enumerate() {
                let (b, ..) = make_buffer(&env, img)?;
                POOL.with(|p| p.borrow_mut().insert((kind.to_string(), i), b));
                n += 1;
            }
        }
        Ok(json!({ "buffers": n }))
    })
}

/// Closes the pool's buffers (frees the shared memory once the page has released its views).
#[tauri::command]
async fn p2_clear_pool(win: WebviewWindow) -> Result<Value, String> {
    on_ui(&win, move |_| {
        let n = POOL.with(|p| {
            let mut p = p.borrow_mut();
            let n = p.len();
            for b in p.values() {
                // SAFETY: UI thread.
                let _ = unsafe { b.Close() };
            }
            p.clear();
            n
        });
        Ok(json!({ "closed": n }))
    })
}

#[tauri::command]
fn p2_info(data: State<'_, Arc<Data>>) -> Value {
    let kinds: serde_json::Map<String, Value> = data
        .imgs
        .iter()
        .map(|(k, v)| (k.to_string(), json!({ "w": v[0].w, "h": v[0].h, "count": v.len() })))
        .collect();
    json!({
        "kinds": kinds,
        "sources": data.sources,
        "bench": arg_after("--bench").is_some(),
        "interactive": arg_after("--interactive").map(|p| json!({ "path": p, "kind": arg_after(&p).unwrap_or_else(|| "v1440".into()) })),
    })
}

#[tauri::command]
fn p2_mem() -> Value {
    sys::tree_private_mb()
}

#[tauri::command]
fn p2_write(json: String) -> Result<(), String> {
    let out = arg_after("--bench").ok_or("not in measurement mode")?;
    std::fs::write(&out, json).map_err(|e| format!("{out}: {e}"))
}

#[tauri::command]
fn p2_exit(app: AppHandle) {
    app.exit(0);
}

// ---------- B1: custom scheme ----------

fn serve(data: &Data, req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    // /<kind>/<idx>
    let parts: Vec<&str> = req.uri().path().trim_matches('/').split('/').collect();
    let img = match parts.as_slice() {
        [k, i] => i.parse::<usize>().ok().and_then(|i| data.imgs.get(*k).and_then(|v| v.get(i))),
        _ => None,
    };
    match img {
        Some(img) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/octet-stream")
            .header("Access-Control-Allow-Origin", "*")
            .header("X-Width", img.w.to_string())
            .header("X-Height", img.h.to_string())
            .header("Access-Control-Expose-Headers", "X-Width, X-Height")
            .body(img.rgba.clone()) // the copy is part of what B1 costs
            .unwrap_or_else(|_| Response::new(Vec::new())),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap_or_else(|_| Response::new(Vec::new())),
    }
}

// ---------- memory of this process and its WebView2 processes ----------

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
    const TH32CS_SNAPPROCESS: u32 = 2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    /// Private memory of this process and all its descendants (the WebView2 browser, renderer
    /// and GPU processes), in MB.
    pub fn tree_private_mb() -> Value {
        // SAFETY: plain Win32 calls with valid out-pointers; handles are closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap as isize == -1 {
                return json!({ "error": "snapshot" });
            }
            let mut all: Vec<(u32, u32, String)> = Vec::new();
            let mut e: ProcessEntry = std::mem::zeroed();
            e.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                let n = e.exe.iter().position(|c| *c == 0).unwrap_or(260);
                all.push((e.pid, e.parent, String::from_utf16_lossy(&e.exe[..n])));
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
            let me = GetCurrentProcessId();
            let mut tree = vec![me];
            let mut i = 0;
            while i < tree.len() {
                let p = tree[i];
                for (pid, parent, _) in &all {
                    if *parent == p && !tree.contains(pid) {
                        tree.push(*pid);
                    }
                }
                i += 1;
            }
            let mut total = 0.0;
            let mut list = Vec::new();
            for pid in &tree {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, *pid);
                if h.is_null() {
                    continue;
                }
                let mut m = MemCountersEx { cb: std::mem::size_of::<MemCountersEx>() as u32, ..Default::default() };
                if K32GetProcessMemoryInfo(h, &mut m, m.cb) != 0 {
                    let mb = m.private_usage as f64 / 1048576.0;
                    total += mb;
                    let name = all.iter().find(|x| x.0 == *pid).map(|x| x.2.clone()).unwrap_or_default();
                    list.push(json!({ "pid": pid, "exe": name, "private_mb": (mb * 10.0).round() / 10.0 }));
                }
                CloseHandle(h);
            }
            json!({ "total_private_mb": (total * 10.0).round() / 10.0, "processes": list })
        }
    }
}

fn main() {
    let files: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| {
            let p = Path::new(a);
            p.is_file() && !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .collect();
    let data = match prepare(&files) {
        Ok(d) => Arc::new(d),
        Err(e) => {
            eprintln!("p2: {e} (give two A1 RAW files)");
            std::process::exit(2);
        }
    };
    let served = data.clone();
    tauri::Builder::default()
        .manage(data)
        .register_asynchronous_uri_scheme_protocol("p2", move |_ctx, request, responder| {
            let d = served.clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(serve(&d, &request)));
        })
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![p2_info, p2_post, p2_fill_pool, p2_clear_pool, p2_mem, p2_write, p2_exit])
        .run(tauri::generate_context!())
        .expect("error while running p2");
}
