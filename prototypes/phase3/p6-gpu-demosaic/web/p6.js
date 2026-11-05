// ADR 0008 P6: GPU demosaicing in presenter B2 (WebGL2). The Bayer data (16 bit) arrives through a
// WebView2 shared buffer and is kept as an integer texture. The shaders reproduce dcraw_process's
// steps for ADR 0006 settings:
//   black subtraction -> camera WB scale (clip) -> demosaic -> camera->sRGB matrix (clip)
//   -> dcraw output curve (BT.709-like, gamma 0.45, slope 4.5) -> 8 bit
// Demosaic: "linear" (as LibRaw's linear, to check the pipeline) and "mhc" (Malvar-He-Cutler).
// Fit view: half-size superpixels (as LibRaw half_size), drawn scaled.
// Not part of the app.
"use strict";

const { invoke } = window.__TAURI__.core;
const canvas = document.getElementById("gl");
const statusEl = document.getElementById("status");
const status = (s) => { statusEl.textContent = s; };
const now = () => performance.now();
const raf = () => new Promise((r) => requestAnimationFrame(r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let pendingShared = null;
window.chrome.webview.addEventListener("sharedbufferreceived", (e) => {
  const t = now();
  const buf = e.getBuffer();
  if (pendingShared) { const p = pendingShared; pendingShared = null; p({ buf, meta: e.additionalData, t }); }
  else window.chrome.webview.releaseBuffer(buf);
});

function fitCanvas() {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(window.innerWidth * dpr);
  canvas.height = Math.round(window.innerHeight * dpr);
}
fitCanvas();
const gl = canvas.getContext("webgl2", { antialias: false, alpha: false, powerPreference: "high-performance", preserveDrawingBuffer: false });

// ---------- dcraw output curve (gamma_curve(0.45, 4.5, 2, 0x10000)), as an 8-bit LUT ----------

function gammaLut(pwr = 0.45, ts = 4.5, imax = 0x10000) {
  const g = [pwr, ts, 0, 0, 0, 0], bnd = [0, 0];
  bnd[g[1] >= 1 ? 1 : 0] = 1;
  if (g[1] && (g[1] - 1) * (g[0] - 1) <= 0) {
    for (let i = 0; i < 48; i++) {
      g[2] = (bnd[0] + bnd[1]) / 2;
      if (g[0]) bnd[((Math.pow(g[2] / g[1], -g[0]) - 1) / g[0] - 1 / g[2] > -1) ? 1 : 0] = g[2];
      else bnd[(g[2] / Math.exp(1 - 1 / g[2]) < g[1]) ? 1 : 0] = g[2];
    }
    g[3] = g[2] / g[1];
    if (g[0]) g[4] = g[2] * (1 / g[0] - 1);
  }
  const lut = new Uint8Array(65536);
  for (let i = 0; i < 65536; i++) {
    let c = 0xffff;
    const r = i / imax;
    if (r < 1) c = Math.floor(0x10000 * (r < g[3] ? r * g[1] : (g[0] ? Math.pow(r, g[0]) * (1 + g[4]) - g[4] : Math.log(r) * g[2] + 1)));
    lut[i] = Math.min(0xffff, c) >> 8;
  }
  return lut;
}

// ---------- shaders ----------

const VS = `#version 300 es
  uniform vec4 r; out vec2 uv;
  void main() {
    vec2 p = vec2(gl_VertexID & 1, gl_VertexID >> 1);
    uv = vec2(p.x, 1.0 - p.y);
    gl_Position = vec4(mix(r.xy, r.zw, p), 0.0, 1.0);
  }`;

const COMMON = `#version 300 es
  precision highp float; precision highp int; precision highp usampler2D;
  uniform highp usampler2D raw;
  uniform highp sampler2D lut;
  uniform uint filters;
  uniform vec4 blk;
  uniform vec4 scl;
  uniform vec3 cam0, cam1, cam2;
  uniform ivec2 isize;
  uniform ivec2 origin;   // image position of output (0,0) = top-left
  uniform ivec2 vp;       // viewport origin in window coordinates
  uniform int outH;       // output height
  uniform int flipY;      // 1: window y is bottom-up (canvas), 0: row 0 = top (texture)
  out vec4 o;
  uint fcIdx(ivec2 p) { uint row = uint(p.y), col = uint(p.x);
    return (filters >> ((((row << 1u) & 14u) | (col & 1u)) << 1u)) & 3u; }
  int chan(ivec2 p) { uint c = fcIdx(p); return c == 3u ? 1 : int(c); }
  ivec2 reflect2(ivec2 p) {  // keeps the CFA phase at the borders
    if (p.x < 0) p.x += 2 * ((1 - p.x) / 2);
    if (p.y < 0) p.y += 2 * ((1 - p.y) / 2);
    if (p.x >= isize.x) p.x -= 2 * ((p.x - isize.x) / 2 + 1);
    if (p.y >= isize.y) p.y -= 2 * ((p.y - isize.y) / 2 + 1);
    return p; }
  float S(ivec2 p) { p = reflect2(p); uint c = fcIdx(p);
    float v = floor((float(texelFetch(raw, p, 0).r) - blk[c]) * scl[c]);
    return clamp(v, 0.0, 65535.0); }
  vec3 outColor(vec3 c) {
    vec3 v = clamp(floor(vec3(dot(cam0, c), dot(cam1, c), dot(cam2, c))), 0.0, 65535.0);
    ivec3 i = ivec3(v);
    return vec3(texelFetch(lut, ivec2(i.r & 255, i.r >> 8), 0).r,
                texelFetch(lut, ivec2(i.g & 255, i.g >> 8), 0).r,
                texelFetch(lut, ivec2(i.b & 255, i.b >> 8), 0).r); }
  ivec2 outPos() { ivec2 f = ivec2(gl_FragCoord.xy) - vp; if (flipY == 1) f.y = outH - 1 - f.y; return f; }
`;

const FS_DEMOSAIC = COMMON + `
  uniform int method;  // 0 linear, 1 MHC
  #define s(x, y) S(p + ivec2(x, y))
  vec3 linear(ivec2 p) {
    int own = chan(p); vec3 sum = vec3(0.0), ws = vec3(0.0);
    for (int dy = -1; dy <= 1; dy++) for (int dx = -1; dx <= 1; dx++) {
      if (dx == 0 && dy == 0) continue;
      ivec2 q = p + ivec2(dx, dy); int c = chan(q);
      if (c == own) continue;
      float w = (dx == 0 || dy == 0) ? 2.0 : 1.0;
      sum[c] += w * S(q); ws[c] += w;
    }
    vec3 r = floor(sum / max(ws, vec3(1e-6)));
    r[own] = S(p);
    return r; }
  vec3 mhc(ivec2 p) {
    float c0 = S(p); int own = chan(p); vec3 rgb;
    if (own == 1) {
      float dg = s(-1,-1) + s(1,-1) + s(-1,1) + s(1,1);
      float hor = (5.0*c0 - s(-2,0) - s(2,0) - dg + 4.0*(s(-1,0) + s(1,0)) + 0.5*(s(0,-2) + s(0,2))) / 8.0;
      float ver = (5.0*c0 - s(0,-2) - s(0,2) - dg + 4.0*(s(0,-1) + s(0,1)) + 0.5*(s(-2,0) + s(2,0))) / 8.0;
      rgb.g = c0;
      if (chan(p + ivec2(1, 0)) == 0) { rgb.r = hor; rgb.b = ver; } else { rgb.b = hor; rgb.r = ver; }
    } else {
      float ax = s(-2,0) + s(2,0) + s(0,-2) + s(0,2);
      float g = (4.0*c0 - ax + 2.0*(s(-1,0) + s(1,0) + s(0,-1) + s(0,1))) / 8.0;
      float d = (6.0*c0 - 1.5*ax + 2.0*(s(-1,-1) + s(1,-1) + s(-1,1) + s(1,1))) / 8.0;
      rgb.g = g;
      if (own == 0) { rgb.r = c0; rgb.b = d; } else { rgb.b = c0; rgb.r = d; }
    }
    return clamp(floor(rgb), 0.0, 65535.0); }
  void main() {
    ivec2 p = origin + outPos();
    o = vec4(outColor(method == 0 ? linear(p) : mhc(p)), 1.0); }`;

const FS_HALF = COMMON + `
  void main() {
    ivec2 p = outPos() * 2;
    vec3 sum = vec3(0.0), n = vec3(0.0);
    for (int dy = 0; dy <= 1; dy++) for (int dx = 0; dx <= 1; dx++) {
      ivec2 q = p + ivec2(dx, dy); int c = chan(q); sum[c] += S(q); n[c] += 1.0; }
    o = vec4(outColor(floor(sum / max(n, vec3(1.0)))), 1.0); }`;

const FS_BLIT = `#version 300 es
  precision mediump float; uniform sampler2D t; in vec2 uv; out vec4 o;
  void main() { o = texture(t, uv); }`;

function program(vs, fs) {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) { const a = gl.getActiveUniform(p, i); u[a.name.replace("[0]", "")] = gl.getUniformLocation(p, a.name); }
  return { p, u };
}

const progDemosaic = program(VS, FS_DEMOSAIC);
const progHalf = program(VS, FS_HALF);
const progBlit = program(VS, FS_BLIT);
gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
gl.pixelStorei(gl.PACK_ALIGNMENT, 1);

// LUT texture (R8, 256 x 256 = 65536 entries)
const lutTex = gl.createTexture();
gl.activeTexture(gl.TEXTURE1);
gl.bindTexture(gl.TEXTURE_2D, lutTex);
gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, 256, 256, 0, gl.RED, gl.UNSIGNED_BYTE, gammaLut());
gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
gl.activeTexture(gl.TEXTURE0);

// ---------- raw texture, half texture, readback target ----------

let rawTex = null, rawW = 0, rawH = 0, meta = null;
let halfTex = null, halfFb = null, halfW = 0, halfH = 0;

function uploadRaw(ab, m) {
  if (!rawTex || rawW !== m.w || rawH !== m.h) {
    if (rawTex) gl.deleteTexture(rawTex);
    rawTex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, rawTex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.R16UI, m.w, m.h);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    rawW = m.w; rawH = m.h;
  }
  gl.bindTexture(gl.TEXTURE_2D, rawTex);
  gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, m.w, m.h, gl.RED_INTEGER, gl.UNSIGNED_SHORT, new Uint16Array(ab));
  meta = m;
}

function setCommon(pr, origin, vp, outH, flipY) {
  gl.useProgram(pr.p);
  gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, rawTex); gl.uniform1i(pr.u.raw, 0);
  gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, lutTex); gl.uniform1i(pr.u.lut, 1);
  gl.activeTexture(gl.TEXTURE0);
  gl.uniform1ui(pr.u.filters, meta.filters >>> 0);
  gl.uniform4fv(pr.u.blk, meta.black);
  gl.uniform4fv(pr.u.scl, meta.scale);
  gl.uniform3fv(pr.u.cam0, meta.cam[0]); gl.uniform3fv(pr.u.cam1, meta.cam[1]); gl.uniform3fv(pr.u.cam2, meta.cam[2]);
  gl.uniform2i(pr.u.isize, meta.w, meta.h);
  gl.uniform2i(pr.u.origin, origin[0], origin[1]);
  gl.uniform2i(pr.u.vp, vp[0], vp[1]);
  gl.uniform1i(pr.u.outH, outH);
  gl.uniform1i(pr.u.flipY, flipY);
  gl.uniform4fv(pr.u.r, [-1, -1, 1, 1]);
}

/// 100 % view: the visible region (centred), demosaiced straight into the canvas.
function region100() {
  const w = Math.min(canvas.width, meta.w), h = Math.min(canvas.height, meta.h);
  const x = ((meta.w - w) >> 1) & ~1, y = ((meta.h - h) >> 1) & ~1;
  return { x, y, w, h, vx: (canvas.width - w) >> 1, vy: (canvas.height - h) >> 1 };
}

function draw100(method) {
  const r = region100();
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.clearColor(0.125, 0.125, 0.125, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  gl.viewport(r.vx, r.vy, r.w, r.h);
  setCommon(progDemosaic, [r.x, r.y], [r.vx, r.vy], r.h, 1);
  gl.uniform1i(progDemosaic.u.method, method === "linear" ? 0 : 1);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

/// Fit view: half-size superpixels into a texture, then drawn scaled into the canvas.
function drawFit() {
  const hw = meta.w >> 1, hh = meta.h >> 1;
  if (!halfTex || halfW !== hw || halfH !== hh) {
    if (halfTex) { gl.deleteTexture(halfTex); gl.deleteFramebuffer(halfFb); }
    halfTex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, halfTex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, hw, hh);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    halfFb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, halfFb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, halfTex, 0);
    halfW = hw; halfH = hh;
  }
  gl.bindFramebuffer(gl.FRAMEBUFFER, halfFb);
  gl.viewport(0, 0, hw, hh);
  setCommon(progHalf, [0, 0], [0, 0], hh, 0);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.clearColor(0.125, 0.125, 0.125, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  gl.useProgram(progBlit.p);
  gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, halfTex); gl.uniform1i(progBlit.u.t, 0);
  const s = Math.min(canvas.width / hw, canvas.height / hh);
  const qw = (hw * s) / canvas.width, qh = (hh * s) / canvas.height;
  // the half texture's row 0 is the image top; the VS maps uv.y = 0 to the top edge
  gl.uniform4fv(progBlit.u.r, [-qw, -qh, qw, qh]);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

/// Renders a region off-screen and reads it back top-down (RGBA).
function readRegion(method, r) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, r.w, r.h);
  const fb = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  gl.viewport(0, 0, r.w, r.h);
  setCommon(progDemosaic, [r.x, r.y], [0, 0], r.h, 0);
  gl.uniform1i(progDemosaic.u.method, method === "linear" ? 0 : 1);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  const px = new Uint8Array(r.w * r.h * 4);
  gl.readPixels(0, 0, r.w, r.h, gl.RGBA, gl.UNSIGNED_BYTE, px);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.deleteFramebuffer(fb); gl.deleteTexture(tex);
  return px;
}

// ---------- one image change ----------

async function load(idx) {
  const t0 = now();
  const got = new Promise((res) => { pendingShared = res; });
  const rust = await invoke("p6_load", { idx });
  const g = await got;
  const tu = now();
  uploadRaw(g.buf, g.meta);
  gl.finish();
  const upload = now() - tu;
  window.chrome.webview.releaseBuffer(g.buf);
  const tf = now();
  drawFit();
  gl.finish();
  const fitGpu = now() - tf;
  await raf();
  return { idx, w: g.meta.w, h: g.meta.h, total_ms: now() - t0, to_page_ms: g.t - t0, upload_ms: upload, fit_gpu_ms: fitGpu, rust };
}

async function zoom(method) {
  const t = now();
  draw100(method);
  gl.finish();
  const gpu = now() - t;
  await raf();
  return { gpu_ms: gpu, total_ms: now() - t };
}

function stats(v) {
  const s = [...v].sort((a, b) => a - b);
  const q = (p) => s[Math.min(s.length - 1, Math.max(0, Math.ceil(s.length * p) - 1))];
  return { n: s.length, median: q(0.5), p95: q(0.95), max: s[s.length - 1] };
}

// ---------- measurement ----------

async function bench(info) {
  const out = { env: {}, files: [], mem: [] };
  const dbg = gl.getExtension("WEBGL_debug_renderer_info");
  out.env = {
    ua: navigator.userAgent, canvas: [canvas.width, canvas.height],
    webgl_renderer: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    max_texture: gl.getParameter(gl.MAX_TEXTURE_SIZE),
  };
  out.mem.push({ label: "start", ...(await invoke("p6_mem")) });
  for (let i = 0; i < info.files.length; i++) {
    status(`P6: ${info.files[i]} (${i + 1}/${info.files.length})`);
    const changes = [];
    for (let k = 0; k < 3; k++) { changes.push(await load(i)); await sleep(300); }
    const zm = [], zl = [];
    for (let k = 0; k < 10; k++) { zm.push(await zoom("mhc")); await sleep(100); zl.push(await zoom("linear")); await sleep(100); }
    const r = region100();
    const quality = [];
    for (const m of ["linear", "mhc"]) {
      const px = readRegion(m, r);
      quality.push(await invoke("p6_quality", px, { headers: { "p6-idx": String(i), "p6-rect": `${r.x},${r.y},${r.w},${r.h}`, "p6-method": m } }));
    }
    const f = changes[0];
    out.files.push({
      file: info.files[i], w: f.w, h: f.h,
      change: {
        total: stats(changes.map((c) => c.total_ms)),
        open_unpack: stats(changes.map((c) => c.rust.open_unpack_ms)),
        copy: stats(changes.map((c) => c.rust.copy_ms + c.rust.create_ms)),
        post: stats(changes.map((c) => c.rust.post_ms)),
        to_page: stats(changes.map((c) => c.to_page_ms)),
        upload: stats(changes.map((c) => c.upload_ms)),
        fit_gpu: stats(changes.map((c) => c.fit_gpu_ms)),
      },
      zoom_mhc: { gpu: stats(zm.map((z) => z.gpu_ms)), total: stats(zm.map((z) => z.total_ms)) },
      zoom_linear: { gpu: stats(zl.map((z) => z.gpu_ms)), total: stats(zl.map((z) => z.total_ms)) },
      region: r,
      quality,
    });
    out.mem.push({ label: `after ${info.files[i]}`, ...(await invoke("p6_mem")) });
  }
  await invoke("p6_write", { json: JSON.stringify(out, null, 1) });
  await invoke("p6_exit");
}

// ---------- interactive (latency.ps1: RIGHT = next file, Z = fit / 100 %) ----------

async function interactive(info) {
  let idx = 0, zoomed = false, busy = false;
  const run = async (f) => { if (busy) return; busy = true; try { await f(); } catch (e) { status(`P6 error: ${e}`); } busy = false; };
  window.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") run(async () => { idx = (idx + 1) % info.files.length; zoomed = false; const r = await load(idx); status(`P6 #${idx} change ${r.total_ms.toFixed(0)} ms`); });
    if (e.key === "z" || e.key === "Z") run(async () => {
      zoomed = !zoomed;
      if (zoomed) { const r = await zoom("mhc"); status(`P6 #${idx} 100 % (GPU) ${r.total_ms.toFixed(1)} ms`); }
      else { drawFit(); await raf(); status(`P6 #${idx} fit`); }
    });
  });
  await run(async () => { await load(0); status("P6 #0 fit"); });
}

(async () => {
  try {
    const info = await invoke("p6_info");
    if (info.bench) { await sleep(1500); await bench(info); }
    else if (info.interactive) await interactive(info);
    else status("P6 ready. Start with --bench or --interactive.");
  } catch (e) {
    status(`P6 error: ${e}`);
    try { await invoke("p6_write", { json: JSON.stringify({ error: String((e && e.stack) || e) }) }); } catch (_) {}
  }
})();
