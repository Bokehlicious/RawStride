//! ADR 0008 phase 2, P3: presenter A. A wgpu surface draws the image area of a Tauri window, and
//! the UI is a child webview, either on top of the whole window with a transparent background
//! (`--layout overlay`) or as a toolbar strip above the image area (`--layout strip`).
//!
//! The images are the same as in P2: 100 % views (`v1080`, `v1440`) and a whole A1 preview
//! (`full`, 8640×5760 RGBA). A swap is either:
//! - **upload**: the RGBA bytes are uploaded to a texture, then drawn (finished pixels in RAM, as
//!   from the phase 1 core); or
//! - **resident**: the image is already a GPU texture (a VRAM cache); only drawn.
//!
//! Modes:
//! - `--bench <out.json>`: timed swaps in Rust for every image size, swap kind and present mode;
//!   writes the results and exits.
//! - `--interactive <upload|resident> <kind>`: the arrow keys (in the webview) step through
//!   images, for the on-screen latency with latency.ps1.
//!
//! Not part of the app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tauri::{AppHandle, LogicalPosition, LogicalSize, State, WebviewUrl, WindowEvent};

// ---------- test images (as in P2) ----------

struct Img {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

fn arg_after(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn load_preview(path: &Path) -> Result<Img, String> {
    let info = pw_raw::inspect(path).map_err(|e| e.to_string())?;
    let p = info.pick(pw_raw::PreviewSize::Large).cloned().ok_or("no preview")?;
    let data = pw_raw::read_preview(path, &p).map_err(|e| e.to_string())?;
    let mut d = zune_jpeg::JpegDecoder::new(&data);
    let rgb = d.decode().map_err(|e| format!("{e:?}"))?;
    let (w, h) = d.dimensions().ok_or("no size")?;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in rgb.chunks_exact(3) {
        rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    Ok(Img { w: w as u32, h: h as u32, rgba })
}

fn crop(src: &Img, x: u32, y: u32, w: u32, h: u32) -> Img {
    let mut o = Vec::with_capacity((w * h * 4) as usize);
    for r in y..y + h {
        let s = ((r * src.w + x) * 4) as usize;
        o.extend_from_slice(&src.rgba[s..s + (w * 4) as usize]);
    }
    Img { w, h, rgba: o }
}

fn prepare(files: &[String]) -> Result<HashMap<&'static str, Vec<Img>>, String> {
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
            let fx = [0.15, 0.5, 0.8][i / full.len() % 3];
            let fy = [0.3, 0.6, 0.45][i / full.len() % 3];
            v.push(crop(s, ((s.w - w) as f64 * fx) as u32, ((s.h - h) as f64 * fy) as u32, w, h));
        }
        imgs.insert(kind, v);
    }
    imgs.insert("full", full);
    Ok(imgs)
}

// ---------- wgpu presenter ----------

const SHADER: &str = r#"
struct U { r: vec4f };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var s: sampler;
@group(0) @binding(2) var t: texture_2d<f32>;
struct O { @builtin(position) p: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) i: u32) -> O {
  let q = vec2f(f32(i & 1u), f32(i >> 1u));
  var o: O;
  o.p = vec4f(mix(u.r.xy, u.r.zw, q), 0.0, 1.0);
  o.uv = vec2f(q.x, 1.0 - q.y);
  return o;
}
@fragment fn fs(i: O) -> @location(0) vec4f { return textureSample(t, s, i.uv); }
"#;

struct Tex {
    tex: wgpu::Texture,
    bind: wgpu::BindGroup,
    w: u32,
    h: u32,
}

struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler_lin: wgpu::Sampler,
    sampler_near: wgpu::Sampler,
    ubuf: wgpu::Buffer,
    tex_format: wgpu::TextureFormat,
    upload: HashMap<&'static str, Tex>,
    resident: HashMap<(&'static str, usize), Tex>,
    /// Physical pixels at the top that belong to the webview strip (0 for the overlay layout).
    top_px: u32,
    adapter_info: String,
    present_modes: Vec<wgpu::PresentMode>,
}

impl Gpu {
    fn new(window: &tauri::Window, top_px: u32) -> Result<Gpu, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("surface: {e}"))?;
        let adapter = tauri::async_runtime::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| format!("adapter: {e}"))?;
        let info = adapter.get_info();
        let adapter_info = format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type);
        let (device, queue) = tauri::async_runtime::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("p3"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("device: {e}"))?;
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let tex_format = if format.is_srgb() { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = |f: wgpu::FilterMode| {
            device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: f, min_filter: f, ..Default::default() })
        };
        let sampler_lin = sampler(wgpu::FilterMode::Linear);
        let sampler_near = sampler(wgpu::FilterMode::Nearest);
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Gpu {
            surface,
            device,
            queue,
            config,
            pipeline,
            layout,
            sampler_lin,
            sampler_near,
            ubuf,
            tex_format,
            upload: HashMap::new(),
            resident: HashMap::new(),
            top_px,
            adapter_info,
            present_modes: caps.present_modes,
        })
    }

    fn resize(&mut self, w: u32, h: u32) {
        self.config.width = w.max(1);
        self.config.height = h.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn set_present_mode(&mut self, m: wgpu::PresentMode) {
        self.config.present_mode = m;
        self.surface.configure(&self.device, &self.config);
    }

    fn make_tex(&self, kind: &str, w: u32, h: u32) -> Tex {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.tex_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let sampler = if kind == "full" { &self.sampler_lin } else { &self.sampler_near };
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        });
        Tex { tex, bind, w, h }
    }

    fn write(&self, t: &Tex, img: &Img) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &t.tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &img.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(img.w * 4), rows_per_image: Some(img.h) },
            wgpu::Extent3d { width: img.w, height: img.h, depth_or_array_layers: 1 },
        );
    }

    /// Clip-space quad of the image inside the image area (below the strip): 100 % views 1:1 in the
    /// centre, the whole image fitted.
    fn quad(&self, kind: &str, w: u32, h: u32) -> [f32; 4] {
        let (cw, ch) = (self.config.width as f32, self.config.height as f32);
        let (aw, ah) = (cw, (ch - self.top_px as f32).max(1.0));
        let s = if kind == "full" { (aw / w as f32).min(ah / h as f32) } else { 1.0 };
        let (qw, qh) = (w as f32 * s, h as f32 * s);
        let x0 = (aw - qw) / 2.0;
        let y0 = self.top_px as f32 + (ah - qh) / 2.0;
        let nx = |x: f32| x / cw * 2.0 - 1.0;
        let ny = |y: f32| 1.0 - y / ch * 2.0;
        [nx(x0), ny(y0 + qh), nx(x0 + qw), ny(y0)]
    }

    /// Draws `t` and presents. Returns (frame acquired ms, submitted+presented ms).
    fn draw(&mut self, kind: &str, t_key: Option<(&'static str, usize)>) -> Result<(f64, f64), String> {
        let t0 = Instant::now();
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                self.surface.get_current_texture().map_err(|e| format!("frame: {e}"))?
            }
        };
        let acquire = ms(t0);
        let t = match t_key {
            Some(k) => self.resident.get(&k),
            None => self.upload.get(kind),
        }
        .ok_or("no texture")?;
        let q = self.quad(kind, t.w, t.h);
        self.queue.write_buffer(&self.ubuf, 0, &q.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.015, g: 0.015, b: 0.015, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &t.bind, &[]);
            pass.draw(0..4, 0..1);
        }
        self.queue.submit([enc.finish()]);
        frame.present();
        Ok((acquire, ms(t0)))
    }

    /// Waits until the GPU has finished the submitted work; returns the wait in ms.
    fn wait_gpu(&self) -> f64 {
        let t = Instant::now();
        let (tx, rx) = mpsc::channel();
        self.queue.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        loop {
            let _ = self.device.poll(wgpu::PollType::Poll);
            if rx.try_recv().is_ok() || t.elapsed() > Duration::from_secs(5) {
                break;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        ms(t)
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

struct App {
    imgs: HashMap<&'static str, Vec<Img>>,
    gpu: Mutex<Option<Gpu>>,
    cur: Mutex<usize>,
}

/// One swap: upload (unless resident) and draw. Returns timings.
fn swap(gpu: &mut Gpu, imgs: &HashMap<&'static str, Vec<Img>>, kind: &'static str, idx: usize, resident: bool) -> Result<Value, String> {
    let img = &imgs[kind][idx];
    let t0 = Instant::now();
    let key = if resident {
        if !gpu.resident.contains_key(&(kind, idx)) {
            let t = gpu.make_tex(kind, img.w, img.h);
            gpu.write(&t, img);
            gpu.resident.insert((kind, idx), t);
        }
        Some((kind, idx))
    } else {
        if gpu.upload.get(kind).is_none_or(|t| t.w != img.w || t.h != img.h) {
            let t = gpu.make_tex(kind, img.w, img.h);
            gpu.upload.insert(kind, t);
        }
        gpu.write(&gpu.upload[kind], img);
        None
    };
    let upload = ms(t0);
    let (acquire, present) = gpu.draw(kind, key)?;
    let presented = ms(t0);
    let gpu_wait = gpu.wait_gpu();
    Ok(json!({ "upload_ms": upload, "acquire_ms": acquire, "draw_present_ms": present, "to_present_ms": presented, "gpu_done_ms": presented + gpu_wait }))
}

fn kind_of(s: &str) -> &'static str {
    match s {
        "v1080" => "v1080",
        "full" => "full",
        _ => "v1440",
    }
}

#[tauri::command]
fn p3_show(app: State<'_, Arc<App>>, delta: i64) -> Result<Value, String> {
    let kind = kind_of(&arg_after("--interactive").and_then(|m| arg_after(&m)).unwrap_or_default());
    let resident = arg_after("--interactive").as_deref() == Some("resident");
    let n = app.imgs[kind].len() as i64;
    let idx = {
        let mut c = app.cur.lock().unwrap();
        *c = ((*c as i64 + delta).rem_euclid(n)) as usize;
        *c
    };
    let mut g = app.gpu.lock().unwrap();
    let gpu = g.as_mut().ok_or("gpu not ready")?;
    let mut r = swap(gpu, &app.imgs, kind, idx, resident)?;
    r["idx"] = json!(idx);
    Ok(r)
}

#[tauri::command]
fn p3_info() -> Value {
    json!({ "interactive": arg_after("--interactive"), "layout": arg_after("--layout").unwrap_or_else(|| "overlay".into()) })
}

// ---------- measurement ----------

fn stats(v: &[f64]) -> Value {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    if s.is_empty() {
        return Value::Null;
    }
    let q = |p: f64| s[((s.len() as f64 * p).ceil() as usize).clamp(1, s.len()) - 1];
    json!({ "n": s.len(), "median": q(0.5), "p95": q(0.95), "max": s[s.len() - 1] })
}

fn bench(handle: AppHandle, app: Arc<App>, out: String) {
    std::thread::sleep(Duration::from_millis(2500));
    let mut runs = Vec::new();
    let mut mem = vec![json!({ "label": "start", "tree": sys::tree_private_mb() })];
    let (info, modes) = {
        let g = app.gpu.lock().unwrap();
        match g.as_ref() {
            Some(gpu) => (gpu.adapter_info.clone(), gpu.present_modes.clone()),
            None => {
                let _ = std::fs::write(&out, json!({ "error": "no gpu" }).to_string());
                handle.exit(1);
                return;
            }
        }
    };
    let mut want = vec![wgpu::PresentMode::Fifo];
    for m in [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate] {
        if modes.contains(&m) {
            want.push(m);
            break;
        }
    }
    for pm in &want {
        app.gpu.lock().unwrap().as_mut().unwrap().set_present_mode(*pm);
        for kind in ["v1080", "v1440", "full"] {
            for resident in [false, true] {
                let n = app.imgs[kind].len();
                let mut samples: Vec<Value> = Vec::new();
                for i in 0..23 {
                    let r = {
                        let mut g = app.gpu.lock().unwrap();
                        swap(g.as_mut().unwrap(), &app.imgs, kind, i % n, resident)
                    };
                    match r {
                        Ok(v) if i >= 3 => samples.push(v),
                        Ok(_) => {}
                        Err(e) => samples.push(json!({ "error": e })),
                    }
                    std::thread::sleep(Duration::from_millis(150));
                }
                let col = |k: &str| samples.iter().filter_map(|s| s[k].as_f64()).collect::<Vec<f64>>();
                runs.push(json!({
                    "present_mode": format!("{pm:?}"),
                    "kind": kind,
                    "swap": if resident { "resident" } else { "upload" },
                    "upload": stats(&col("upload_ms")),
                    "acquire": stats(&col("acquire_ms")),
                    "to_present": stats(&col("to_present_ms")),
                    "gpu_done": stats(&col("gpu_done_ms")),
                    "samples": samples,
                }));
                mem.push(json!({ "label": format!("after {pm:?} {kind} {}", if resident { "resident" } else { "upload" }), "tree": sys::tree_private_mb() }));
            }
        }
    }
    let vram = app.gpu.lock().unwrap().as_ref().and_then(|g| g.device.generate_allocator_report()).map(|r| {
        json!({ "total_allocated_mb": r.total_allocated_bytes as f64 / 1048576.0, "total_reserved_mb": r.total_reserved_bytes as f64 / 1048576.0 })
    });
    let result = json!({
        "adapter": info,
        "present_modes_available": modes.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>(),
        "layout": arg_after("--layout").unwrap_or_else(|| "overlay".into()),
        "vram_after_all": vram,
        "runs": runs,
        "mem": mem,
    });
    let _ = std::fs::write(&out, serde_json::to_string_pretty(&result).unwrap_or_default());
    handle.exit(0);
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
            let (mut total, mut own) = (0.0, 0.0);
            for (n, pid) in tree.iter().enumerate() {
                let h = OpenProcess(0x1000, 0, *pid);
                if h.is_null() {
                    continue;
                }
                let mut m = MemCountersEx { cb: std::mem::size_of::<MemCountersEx>() as u32, ..Default::default() };
                if K32GetProcessMemoryInfo(h, &mut m, m.cb) != 0 {
                    let mb = m.private_usage as f64 / 1048576.0;
                    total += mb;
                    if n == 0 {
                        own = mb;
                    }
                }
                CloseHandle(h);
            }
            json!({ "total_private_mb": (total * 10.0).round() / 10.0, "app_private_mb": (own * 10.0).round() / 10.0, "processes": tree.len() })
        }
    }
}

// ---------- start ----------

/// Makes the webview's own background transparent (overlay layout), through WebView2.
fn transparent_background(wv: &tauri::Webview) {
    let _ = wv.with_webview(|p| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_COLOR, ICoreWebView2Controller2};
        use windows_core::Interface;
        // SAFETY: COM call on the UI thread (with_webview runs there).
        unsafe {
            if let Ok(c2) = p.controller().cast::<ICoreWebView2Controller2>() {
                let _ = c2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR { A: 0, R: 0, G: 0, B: 0 });
            }
        }
    });
}

fn main() {
    let files: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| {
            let p = Path::new(a);
            p.is_file() && !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .collect();
    let imgs = match prepare(&files) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("p3: {e} (give two A1 RAW files)");
            std::process::exit(2);
        }
    };
    let app_state = Arc::new(App { imgs, gpu: Mutex::new(None), cur: Mutex::new(0) });
    let strip = arg_after("--layout").as_deref() == Some("strip");
    const STRIP_H: f64 = 48.0;
    let st = app_state.clone();
    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            let window = tauri::window::WindowBuilder::new(app, "main")
                .title("P3 wgpu")
                .inner_size(1600.0, 1000.0)
                .maximized(true)
                .build()?;
            let scale = window.scale_factor()?;
            let size = window.inner_size()?.to_logical::<f64>(scale);
            let top_px = if strip { (STRIP_H * scale).round() as u32 } else { 0 };
            let gpu = Gpu::new(&window, top_px).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            *st.gpu.lock().unwrap() = Some(gpu);
            let wb = tauri::webview::WebviewBuilder::new("ui", WebviewUrl::App("index.html".into())).transparent(!strip);
            let wv = if strip {
                window.add_child(wb, LogicalPosition::new(0.0, 0.0), LogicalSize::new(size.width, STRIP_H))?
            } else {
                window.add_child(wb.auto_resize(), LogicalPosition::new(0.0, 0.0), size)?
            };
            if !strip {
                transparent_background(&wv);
            }
            let _ = wv.set_focus();
            // keep the surface (and the strip) in step with the window
            let st2 = st.clone();
            let wv2 = wv.clone();
            window.on_window_event(move |e| match e {
                WindowEvent::Resized(s) => {
                    if let Some(g) = st2.gpu.lock().unwrap().as_mut() {
                        g.resize(s.width, s.height);
                    }
                    if strip {
                        if let Ok(sc) = wv2.window().scale_factor() {
                            let _ = wv2.set_size(LogicalSize::new(s.width as f64 / sc, STRIP_H));
                        }
                    }
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, scale_factor, .. } => {
                    if let Some(g) = st2.gpu.lock().unwrap().as_mut() {
                        g.top_px = if strip { (STRIP_H * scale_factor).round() as u32 } else { 0 };
                        g.resize(new_inner_size.width, new_inner_size.height);
                    }
                }
                _ => {}
            });
            if let Some(out) = arg_after("--bench") {
                let handle = app.handle().clone();
                let st3 = st.clone();
                std::thread::spawn(move || bench(handle, st3, out));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![p3_show, p3_info])
        .run(tauri::generate_context!())
        .expect("error while running p3");
}
