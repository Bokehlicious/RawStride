// ADR 0008 phase 2, P2: presenter B2 (WebView2 shared buffer) against B1 (custom scheme fetch),
// drawn with WebGL2, and with WebGPU when the runtime has it. Not part of the app.
"use strict";

const { invoke } = window.__TAURI__.core;
const B1_BASE = navigator.userAgent.includes("Windows") ? "http://p2.localhost/" : "p2://localhost/";
const statusEl = document.getElementById("status");
const glCanvas = document.getElementById("gl");
const gpuCanvas = document.getElementById("gpu");
const now = () => performance.now();
const raf = () => new Promise((r) => requestAnimationFrame(r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const status = (s) => { statusEl.textContent = s; };

// ---------- shared buffers from Rust ----------

let pendingShared = null;
window.chrome.webview.addEventListener("sharedbufferreceived", (e) => {
  const t = now();
  const buf = e.getBuffer();
  const meta = e.additionalData;
  if (pendingShared) {
    const p = pendingShared;
    pendingShared = null;
    p({ buf, meta, t });
  } else {
    window.chrome.webview.releaseBuffer(buf);
  }
});

// ---------- canvas size (device pixels, so 100 % is 1:1) ----------

function fitCanvas(c) {
  const dpr = window.devicePixelRatio || 1;
  c.width = Math.round(window.innerWidth * dpr);
  c.height = Math.round(window.innerHeight * dpr);
}

/// Quad in clip space: a view at 1:1 in the centre, a whole image fitted.
function quad(kind, w, h, cw, ch) {
  const s = kind === "full" ? Math.min(cw / w, ch / h) : 1;
  const qw = (w * s) / cw, qh = (h * s) / ch;
  return [-qw, -qh, qw, qh];
}

// ---------- WebGL2 ----------

const gl = glCanvas.getContext("webgl2", { antialias: false, alpha: false, powerPreference: "high-performance" });
let glProg, glRect, glTextures = {};

function initGL() {
  fitCanvas(glCanvas);
  const vs = `#version 300 es
    uniform vec4 r; out vec2 uv;
    void main() {
      vec2 p = vec2(gl_VertexID & 1, gl_VertexID >> 1);
      uv = vec2(p.x, 1.0 - p.y);
      gl_Position = vec4(mix(r.xy, r.zw, p), 0.0, 1.0);
    }`;
  const fs = `#version 300 es
    precision mediump float; uniform sampler2D t; in vec2 uv; out vec4 o;
    void main() { o = texture(t, uv); }`;
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  glProg = gl.createProgram();
  gl.attachShader(glProg, sh(gl.VERTEX_SHADER, vs));
  gl.attachShader(glProg, sh(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(glProg);
  if (!gl.getProgramParameter(glProg, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(glProg));
  glRect = gl.getUniformLocation(glProg, "r");
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
}

function glTexture(kind, w, h, mip) {
  const key = `${kind}${mip ? "-mip" : ""}`;
  let t = glTextures[key];
  if (t && t.w === w && t.h === h) return t;
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  const levels = mip ? Math.floor(Math.log2(Math.max(w, h))) + 1 : 1;
  gl.texStorage2D(gl.TEXTURE_2D, levels, gl.RGBA8, w, h);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, mip ? gl.LINEAR_MIPMAP_LINEAR : gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, kind === "full" ? gl.LINEAR : gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  t = glTextures[key] = { tex, w, h };
  return t;
}

/// "b2-res": one texture per image, kept on the GPU (a VRAM cache). Filled once, then only drawn.
const resTextures = {};

async function drawGL(kind, t) {
  gl.viewport(0, 0, glCanvas.width, glCanvas.height);
  gl.clearColor(0.125, 0.125, 0.125, 1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.useProgram(glProg);
  gl.bindTexture(gl.TEXTURE_2D, t.tex);
  gl.uniform4fv(glRect, quad(kind, t.w, t.h, glCanvas.width, glCanvas.height));
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  gl.finish();
  const done = now();
  await raf();
  return { up: 0, done, frame: now() };
}

async function presentGL(kind, w, h, ab, mip) {
  const t = glTexture(kind, w, h, mip);
  const tu = now();
  gl.bindTexture(gl.TEXTURE_2D, t.tex);
  gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(ab));
  if (mip) gl.generateMipmap(gl.TEXTURE_2D);
  const up = now() - tu;
  gl.viewport(0, 0, glCanvas.width, glCanvas.height);
  gl.clearColor(0.125, 0.125, 0.125, 1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.useProgram(glProg);
  gl.uniform4fv(glRect, quad(kind, w, h, glCanvas.width, glCanvas.height));
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  gl.finish();
  const done = now();
  await raf();
  return { up, done, frame: now() };
}

// ---------- WebGPU (optional) ----------

let gpu = null;

async function initGPU() {
  if (!navigator.gpu) return null;
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) return null;
  const device = await adapter.requestDevice({
    requiredLimits: { maxTextureDimension2D: adapter.limits.maxTextureDimension2D },
  });
  fitCanvas(gpuCanvas);
  const ctx = gpuCanvas.getContext("webgpu");
  const format = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format, alphaMode: "opaque" });
  const module = device.createShaderModule({
    code: `
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
      @fragment fn fs(i: O) -> @location(0) vec4f { return textureSample(t, s, i.uv); }`,
  });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format }] },
    primitive: { topology: "triangle-strip" },
  });
  const ubuf = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
  let info = {};
  try {
    const i = adapter.info || (adapter.requestAdapterInfo ? await adapter.requestAdapterInfo() : {});
    info = { vendor: i.vendor, architecture: i.architecture, description: i.description };
  } catch (_) {}
  return { device, ctx, pipeline, ubuf, sampler, textures: {}, maxTex: adapter.limits.maxTextureDimension2D, info };
}

async function presentGPU(kind, w, h, ab) {
  const g = gpu;
  let t = g.textures[kind];
  if (!t || t.w !== w || t.h !== h) {
    const tex = g.device.createTexture({
      size: [w, h],
      format: "rgba8unorm",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    const bind = g.device.createBindGroup({
      layout: g.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: g.ubuf } },
        { binding: 1, resource: g.sampler },
        { binding: 2, resource: tex.createView() },
      ],
    });
    t = g.textures[kind] = { tex, bind, w, h };
  }
  const tu = now();
  g.device.queue.writeTexture({ texture: t.tex }, ab, { bytesPerRow: w * 4 }, [w, h]);
  const up = now() - tu;
  g.device.queue.writeBuffer(g.ubuf, 0, new Float32Array(quad(kind, w, h, gpuCanvas.width, gpuCanvas.height)));
  const enc = g.device.createCommandEncoder();
  const pass = enc.beginRenderPass({
    colorAttachments: [{ view: g.ctx.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: [0.125, 0.125, 0.125, 1] }],
  });
  pass.setPipeline(g.pipeline);
  pass.setBindGroup(0, t.bind);
  pass.draw(4);
  pass.end();
  g.device.queue.submit([enc.finish()]);
  await g.device.queue.onSubmittedWorkDone();
  const done = now();
  await raf();
  return { up, done, frame: now() };
}

function useApi(api) {
  glCanvas.style.display = api === "webgl" ? "block" : "none";
  gpuCanvas.style.display = api === "webgpu" ? "block" : "none";
}

// ---------- one swap ----------

async function fetchB1(kind, idx) {
  const r = await fetch(`${B1_BASE}${kind}/${idx}`);
  if (!r.ok) throw new Error(`B1 ${r.status}`);
  const ab = await r.arrayBuffer();
  return { ab, w: +r.headers.get("X-Width"), h: +r.headers.get("X-Height") };
}

async function swap(cfg, kind, idx) {
  const t0 = now();
  let ab, w, h, t1, rust = null;
  if (cfg.path === "b2-res") {
    const key = `${kind}-${idx}`;
    let t = resTextures[key];
    if (!t) {
      // first time: the image arrives through a pooled shared buffer and becomes its own texture
      const got = new Promise((res) => { pendingShared = res; });
      rust = await invoke("p2_post", { kind, idx, pool: true });
      const g = await got;
      const tex = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, g.meta.w, g.meta.h);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, kind === "full" ? gl.LINEAR : gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, g.meta.w, g.meta.h, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(g.buf));
      window.chrome.webview.releaseBuffer(g.buf);
      t = resTextures[key] = { tex, w: g.meta.w, h: g.meta.h };
    }
    const r = await drawGL(kind, t);
    return { total_ms: r.frame - t0, to_page_ms: 0, upload_ms: 0, gpu_done_ms: r.done - t0, rust, first: !!rust };
  }
  if (cfg.path === "b1") {
    ({ ab, w, h } = await fetchB1(kind, idx));
    t1 = now();
  } else {
    const got = new Promise((res) => { pendingShared = res; });
    rust = await invoke("p2_post", { kind, idx, pool: cfg.path === "b2-pool" });
    const g = await got;
    ab = g.buf; w = g.meta.w; h = g.meta.h; t1 = g.t;
  }
  const r = cfg.api === "webgpu" ? await presentGPU(kind, w, h, ab) : await presentGL(kind, w, h, ab, !!cfg.mip);
  if (cfg.path !== "b1") window.chrome.webview.releaseBuffer(ab);
  return {
    total_ms: r.frame - t0,
    to_page_ms: t1 - t0,
    upload_ms: r.up,
    gpu_done_ms: r.done - t0,
    rust,
  };
}

// ---------- measurement run ----------

function stats(v) {
  const s = [...v].sort((a, b) => a - b);
  const q = (p) => s[Math.min(s.length - 1, Math.max(0, Math.ceil(s.length * p) - 1))];
  return { n: s.length, median: q(0.5), p95: q(0.95), max: s[s.length - 1] };
}

async function bench(info) {
  const out = { env: {}, runs: [], mem: [], lifetime: {} };
  const dbg = gl.getExtension("WEBGL_debug_renderer_info");
  out.env = {
    ua: navigator.userAgent,
    dpr: window.devicePixelRatio,
    canvas: [glCanvas.width, glCanvas.height],
    webgl_renderer: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    webgl_max_texture: gl.getParameter(gl.MAX_TEXTURE_SIZE),
    webgpu: gpu ? { maxTexture: gpu.maxTex, ...gpu.info } : null,
    kinds: info.kinds,
    sources: info.sources,
  };
  const mem = async (label) => out.mem.push({ label, t: Math.round(now()), ...(await invoke("p2_mem")) });
  await mem("start");

  const configs = [];
  for (const kind of ["v1080", "v1440", "full"]) {
    configs.push({ path: "b1", api: "webgl", kind });
    configs.push({ path: "b2-new", api: "webgl", kind });
    configs.push({ path: "b2-pool", api: "webgl", kind });
    configs.push({ path: "b2-res", api: "webgl", kind });
    if (gpu && info.kinds[kind].w <= gpu.maxTex && info.kinds[kind].h <= gpu.maxTex) {
      configs.push({ path: "b2-pool", api: "webgpu", kind });
    }
  }
  configs.push({ path: "b2-pool", api: "webgl", kind: "full", mip: true });

  status("P2: filling the shared-buffer pool...");
  const fill = await invoke("p2_fill_pool");
  await mem(`pool filled (${fill.buffers} buffers)`);

  const WARM = 3, N = 20;
  for (const cfg of configs) {
    const name = `${cfg.path} ${cfg.api} ${cfg.kind}${cfg.mip ? " mip" : ""}`;
    useApi(cfg.api);
    await raf();
    const count = info.kinds[cfg.kind].count;
    const samples = [];
    // b2-res: every image has been made a texture during the warm-up, so the timed swaps only draw
    const warm = cfg.path === "b2-res" ? Math.max(WARM, count) : WARM;
    for (let i = 0; i < warm + N; i++) {
      status(`P2: ${name} ${i + 1}/${warm + N}`);
      const r = await swap(cfg, cfg.kind, i % count);
      if (i >= warm) samples.push(r);
      await sleep(150);
    }
    const pick = (k) => stats(samples.map((s) => s[k]));
    out.runs.push({
      ...cfg,
      total: pick("total_ms"),
      to_page: pick("to_page_ms"),
      upload: pick("upload_ms"),
      gpu_done: pick("gpu_done_ms"),
      rust_copy: cfg.path !== "b2-new" && cfg.path !== "b2-pool" ? null : stats(samples.map((s) => s.rust.copy_ms + s.rust.create_ms)),
      rust_post: cfg.path !== "b2-new" && cfg.path !== "b2-pool" ? null : stats(samples.map((s) => s.rust.post_ms)),
      samples,
    });
    await mem(`after ${name}`);
  }
  useApi("webgl");

  // Buffer lifetime: full images posted as new buffers, released at once / kept / released later.
  status("P2: buffer lifetime...");
  for (let i = 0; i < 6; i++) await swap({ path: "b2-new", api: "webgl" }, "full", i % 2);
  await sleep(1000);
  await mem("lifetime: 6 new full buffers, each released after upload");
  const kept = [];
  for (let i = 0; i < 6; i++) {
    const got = new Promise((res) => { pendingShared = res; });
    await invoke("p2_post", { kind: "full", idx: i % 2, pool: false });
    kept.push((await got).buf);
  }
  await sleep(1000);
  await mem("lifetime: 6 new full buffers kept by the page (not released)");
  for (const b of kept) window.chrome.webview.releaseBuffer(b);
  await sleep(1000);
  await mem("lifetime: the 6 kept buffers released");
  await invoke("p2_clear_pool");
  await sleep(1000);
  await mem("lifetime: pool closed");

  status("P2: writing results...");
  await invoke("p2_write", { json: JSON.stringify(out, null, 1) });
  await invoke("p2_exit");
}

// ---------- interactive (on-screen latency with latency.ps1) ----------

async function interactive(info, path, kind) {
  const cfg = { path, api: "webgl" };
  if (path === "b2-pool" || path === "b2-res") await invoke("p2_fill_pool");
  const count = info.kinds[kind].count;
  let idx = 0, busy = false, want = null;
  const show = async (i) => {
    if (busy) { want = i; return; }
    busy = true;
    try {
      const r = await swap(cfg, kind, i);
      status(`P2 ${path} ${kind} #${i}  total ${r.total_ms.toFixed(1)} ms`);
    } finally {
      busy = false;
    }
    if (want !== null) { const n = want; want = null; show(n); }
  };
  window.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") { idx = (idx + 1) % count; show(idx); }
    if (e.key === "ArrowLeft") { idx = (idx + count - 1) % count; show(idx); }
  });
  if (path === "b2-res") {
    // make every image a texture before the measurement starts
    for (let i = count - 1; i >= 0; i--) await swap(cfg, kind, i);
  }
  await show(0);
}

// ---------- start ----------

(async () => {
  try {
    initGL();
    gpu = await initGPU().catch(() => null);
    const info = await invoke("p2_info");
    if (info.interactive) {
      await interactive(info, info.interactive.path, info.interactive.kind);
    } else if (info.bench) {
      await sleep(1500); // let the window settle
      await bench(info);
    } else {
      status(`P2 ready. WebGPU: ${gpu ? "yes" : "no"}. Start with --bench or --interactive.`);
    }
  } catch (e) {
    status(`P2 error: ${e}`);
    try { await invoke("p2_write", { json: JSON.stringify({ error: String(e && e.stack || e) }) }); } catch (_) {}
  }
})();
