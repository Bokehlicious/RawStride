// ADR 0008 P6b: PPG demosaicing as a WebGL2 shader in presenter B2. Not part of the app.
//
// Pipeline (as dcraw_process with the ADR 0006 settings, verified exactly in P6 with "linear"):
//   black subtraction -> camera WB scale (clip) -> demosaic -> camera->sRGB matrix (clip)
//   -> dcraw output curve (gamma 0.45, slope 4.5) -> 8 bit -> rotation for display (LibRaw flip)
// PPG (Patterned Pixel Grouping) in two passes:
//   1. green at red/blue sites (gradient-directed, clamped between the neighbours), into an R16UI
//      texture covering the needed sensor rectangle plus a 1-pixel margin;
//   2. red/blue at green sites, then blue/red at red/blue sites (diagonal gradients), colour
//      conversion, output curve, written straight to the screen pixel (rotation = coordinate mapping).
// Demosaicing always works on the sensor grid; orientation only maps screen pixels to sensor pixels.
"use strict";

const { invoke } = window.__TAURI__.core;
const canvas = document.getElementById("gl");
const statusEl = document.getElementById("status");
const status = (s) => { statusEl.textContent = s; };
const now = () => performance.now();
const raf = () => new Promise((r) => requestAnimationFrame(r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const GPU_BUDGET_MB = 2048; // texture cache budget for this prototype (Bayer + half-size)

// ---------- shared buffers from Rust, matched by position ----------

const pending = new Map();
window.chrome.webview.addEventListener("sharedbufferreceived", (e) => {
  const t = now();
  const buf = e.getBuffer();
  const meta = e.additionalData;
  const p = pending.get(meta.idx);
  if (p) { pending.delete(meta.idx); p({ buf, meta, t }); }
  else window.chrome.webview.releaseBuffer(buf);
});

function fitCanvas() {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(window.innerWidth * dpr);
  canvas.height = Math.round(window.innerHeight * dpr);
}
fitCanvas();

// ---------- dcraw output curve as an 8-bit LUT ----------

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
  uniform uint filters;
  uniform vec4 blk;
  uniform vec4 scl;
  uniform ivec2 isize;      // sensor (visible) size
  uniform ivec2 vp;         // viewport origin in window coordinates
  uniform int outH;         // output height
  uniform int flipY;        // 1: window y is bottom-up (canvas); 0: row 0 = top (texture)
  uint fcIdx(ivec2 p) { uint row = uint(p.y), col = uint(p.x);
    return (filters >> ((((row << 1u) & 14u) | (col & 1u)) << 1u)) & 3u; }
  int chan(ivec2 p) { uint c = fcIdx(p); return c == 3u ? 1 : int(c); }
  ivec2 reflect2(ivec2 p) {
    if (p.x < 0) p.x += 2 * ((1 - p.x) / 2);
    if (p.y < 0) p.y += 2 * ((1 - p.y) / 2);
    if (p.x >= isize.x) p.x -= 2 * ((p.x - isize.x) / 2 + 1);
    if (p.y >= isize.y) p.y -= 2 * ((p.y - isize.y) / 2 + 1);
    return p; }
  // scaled sensor value, as dcraw's scale_colors: CLIP((int)((raw - black) * scale))
  int S(ivec2 p) { p = reflect2(p); uint c = fcIdx(p);
    float v = floor((float(texelFetch(raw, p, 0).r) - blk[c]) * scl[c]);
    return int(clamp(v, 0.0, 65535.0)); }
  ivec2 outPos() { ivec2 f = ivec2(gl_FragCoord.xy) - vp; if (flipY == 1) f.y = outH - 1 - f.y; return f; }
`;

// Pass 1: green everywhere on a sensor rectangle (origin gOrigin), PPG at red/blue sites.
const FS_GREEN = COMMON + `
  uniform ivec2 gOrigin;
  out uvec4 o;
  int ulim(int x, int y, int z) { return y < z ? clamp(x, y, z) : clamp(x, z, y); }
  void main() {
    ivec2 p = gOrigin + outPos();
    if (chan(p) == 1) { o = uvec4(uint(S(p)), 0u, 0u, 0u); return; }
    int c0 = S(p);
    int guess[2]; int diff[2];
    for (int i = 0; i < 2; i++) {
      ivec2 d = i == 0 ? ivec2(1, 0) : ivec2(0, 1);
      int gm = S(p - d), gp = S(p + d);
      int cm2 = S(p - 2 * d), cp2 = S(p + 2 * d);
      guess[i] = (gm + c0 + gp) * 2 - cm2 - cp2;
      diff[i] = (abs(cm2 - c0) + abs(cp2 - c0) + abs(gm - gp)) * 3
              + (abs(S(p + 3 * d) - gp) + abs(S(p - 3 * d) - gm)) * 2;
    }
    int i = diff[0] > diff[1] ? 1 : 0;
    ivec2 d = i == 0 ? ivec2(1, 0) : ivec2(0, 1);
    o = uvec4(uint(ulim(guess[i] >> 2, S(p + d), S(p - d))), 0u, 0u, 0u);
  }`;

// Pass 2: final colour per screen pixel. Screen -> oriented image -> sensor (LibRaw flip).
const FS_FINAL = COMMON + `
  uniform highp usampler2D green;
  uniform ivec2 gOrigin;
  uniform highp sampler2D lut;
  uniform vec3 cam0, cam1, cam2;
  uniform int flip;           // LibRaw flip: 0, 3, 5, 6 (forced 0 for sensor-orientation output)
  uniform ivec2 dispOrigin;   // oriented-image position of output (0,0)
  uniform int method;         // 0 linear (pipeline check), 2 PPG
  out vec4 o;
  int G(ivec2 p) { return int(texelFetch(green, reflect2(p) - gOrigin, 0).r); }
  int clip(int v) { return clamp(v, 0, 65535); }
  ivec2 toSensor(ivec2 q) {
    if (flip == 3) return ivec2(isize.x - 1 - q.x, isize.y - 1 - q.y);
    if (flip == 6) return ivec2(q.y, isize.y - 1 - q.x);
    if (flip == 5) return ivec2(isize.x - 1 - q.y, q.x);
    return q; }
  ivec3 linear(ivec2 p) {
    int own = chan(p); vec3 sum = vec3(0.0), ws = vec3(0.0);
    for (int dy = -1; dy <= 1; dy++) for (int dx = -1; dx <= 1; dx++) {
      if (dx == 0 && dy == 0) continue;
      ivec2 q = p + ivec2(dx, dy); int c = chan(q);
      if (c == own) continue;
      float w = (dx == 0 || dy == 0) ? 2.0 : 1.0;
      sum[c] += w * float(S(q)); ws[c] += w;
    }
    ivec3 r = ivec3(floor(sum / max(ws, vec3(1e-6))));
    r[own] = S(p);
    return r; }
  ivec3 ppg(ivec2 p) {
    int own = chan(p); ivec3 r;
    if (own == 1) {
      int g0 = S(p); r.g = g0;
      int ch = chan(p + ivec2(1, 0));          // colour of the horizontal neighbours
      int hv = clip((S(p - ivec2(1, 0)) + S(p + ivec2(1, 0)) + 2 * g0 - G(p - ivec2(1, 0)) - G(p + ivec2(1, 0))) >> 1);
      int vv = clip((S(p - ivec2(0, 1)) + S(p + ivec2(0, 1)) + 2 * g0 - G(p - ivec2(0, 1)) - G(p + ivec2(0, 1))) >> 1);
      if (ch == 0) { r.r = hv; r.b = vv; } else { r.b = hv; r.r = vv; }
    } else {
      int g0 = G(p); r.g = g0;
      int guess[2]; int diff[2];
      for (int i = 0; i < 2; i++) {
        ivec2 d = i == 0 ? ivec2(1, 1) : ivec2(-1, 1);
        int cm = S(p - d), cp = S(p + d), gm = G(p - d), gp = G(p + d);
        diff[i] = abs(cm - cp) + abs(gm - g0) + abs(gp - g0);
        guess[i] = cm + cp + 2 * g0 - gm - gp;
      }
      int other = diff[0] != diff[1] ? clip(guess[diff[0] > diff[1] ? 1 : 0] >> 1) : clip((guess[0] + guess[1]) >> 2);
      if (own == 0) { r.r = S(p); r.b = other; } else { r.b = S(p); r.r = other; }
    }
    return r; }
  vec3 outColor(ivec3 c) {
    vec3 f = vec3(c);
    vec3 v = clamp(floor(vec3(dot(cam0, f), dot(cam1, f), dot(cam2, f))), 0.0, 65535.0);
    ivec3 i = ivec3(v);
    return vec3(texelFetch(lut, ivec2(i.r & 255, i.r >> 8), 0).r,
                texelFetch(lut, ivec2(i.g & 255, i.g >> 8), 0).r,
                texelFetch(lut, ivec2(i.b & 255, i.b >> 8), 0).r); }
  void main() {
    ivec2 p = toSensor(dispOrigin + outPos());
    o = vec4(outColor(method == 0 ? linear(p) : ppg(p)), 1.0); }`;

// Half-size superpixels (as LibRaw half_size) in sensor orientation, for the fit view.
const FS_HALF = COMMON + `
  uniform highp sampler2D lut;
  uniform vec3 cam0, cam1, cam2;
  out vec4 o;
  void main() {
    ivec2 p = outPos() * 2;
    vec3 sum = vec3(0.0), n = vec3(0.0);
    for (int dy = 0; dy <= 1; dy++) for (int dx = 0; dx <= 1; dx++) {
      ivec2 q = p + ivec2(dx, dy); int c = chan(q); sum[c] += float(S(q)); n[c] += 1.0; }
    vec3 f = floor(sum / max(n, vec3(1.0)));
    vec3 v = clamp(floor(vec3(dot(cam0, f), dot(cam1, f), dot(cam2, f))), 0.0, 65535.0);
    ivec3 i = ivec3(v);
    o = vec4(texelFetch(lut, ivec2(i.r & 255, i.r >> 8), 0).r, texelFetch(lut, ivec2(i.g & 255, i.g >> 8), 0).r,
             texelFetch(lut, ivec2(i.b & 255, i.b >> 8), 0).r, 1.0); }`;

// Fit view: the half-size texture drawn scaled, rotated by flip (uv is in oriented space).
const FS_BLIT = `#version 300 es
  precision mediump float; uniform sampler2D t; uniform int flip; in vec2 uv; out vec4 o;
  void main() {
    vec2 s = uv;
    if (flip == 3) s = vec2(1.0 - uv.x, 1.0 - uv.y);
    else if (flip == 6) s = vec2(uv.y, 1.0 - uv.x);
    else if (flip == 5) s = vec2(1.0 - uv.y, uv.x);
    o = texture(t, s); }`;

// ---------- GL state (rebuilt after a context loss) ----------

let gl, progGreen, progFinal, progHalf, progBlit, lutTex, greenTex = null, greenFb = null, greenW = 0, greenH = 0;
const cache = new Map(); // position -> { meta, raw, half, bytes, used }
let cacheBytes = 0;
let contextLost = false;

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

function initGL() {
  gl = canvas.getContext("webgl2", { antialias: false, alpha: false, powerPreference: "high-performance" });
  progGreen = program(VS, FS_GREEN);
  progFinal = program(VS, FS_FINAL);
  progHalf = program(VS, FS_HALF);
  progBlit = program(VS, FS_BLIT);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.pixelStorei(gl.PACK_ALIGNMENT, 1);
  lutTex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, lutTex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, 256, 256, 0, gl.RED, gl.UNSIGNED_BYTE, gammaLut());
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  greenTex = null; greenFb = null; greenW = greenH = 0;
}

canvas.addEventListener("webglcontextlost", (e) => { e.preventDefault(); contextLost = true; });

// ---------- texture cache ----------

function evict(needBytes, keep) {
  const budget = GPU_BUDGET_MB * 1048576;
  const entries = [...cache.entries()].filter(([k]) => !keep.includes(k)).sort((a, b) => a[1].used - b[1].used);
  let evicted = 0;
  while (cacheBytes + needBytes > budget && entries.length) {
    const [k, e] = entries.shift();
    gl.deleteTexture(e.raw); gl.deleteTexture(e.half);
    cacheBytes -= e.bytes; cache.delete(k); evicted++;
  }
  stats.evictions += evicted;
}

/// Loads position `pos` (unpack in Rust, upload, half-size render) unless cached.
async function ensure(pos, keep = []) {
  const c = cache.get(pos);
  if (c) { c.used = now(); return { cached: true, entry: c }; }
  if (ensure.inflight.has(pos)) return ensure.inflight.get(pos);
  const job = (async () => {
    const t0 = now();
    const got = new Promise((res) => pending.set(pos, res));
    const rust = await invoke("p6_load", { idx: pos });
    const g = await got;
    const m = g.meta;
    const bytes = m.w * m.h * 2 + (m.w >> 1) * (m.h >> 1) * 4;
    evict(bytes, [pos, ...keep]);
    const tu = now();
    const raw = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, raw);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.R16UI, m.w, m.h);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, m.w, m.h, gl.RED_INTEGER, gl.UNSIGNED_SHORT, new Uint16Array(g.buf));
    window.chrome.webview.releaseBuffer(g.buf);
    const entry = { meta: m, raw, half: renderHalf(raw, m), bytes, used: now() };
    gl.finish();
    const upload = now() - tu;
    cache.set(pos, entry); cacheBytes += bytes;
    return { cached: false, entry, rust, to_page_ms: g.t - t0, upload_ms: upload };
  })();
  ensure.inflight.set(pos, job);
  try { return await job; } finally { ensure.inflight.delete(pos); }
}
ensure.inflight = new Map();

function setCommon(pr, e, vp, outH, flipY) {
  const m = e.meta;
  gl.useProgram(pr.p);
  gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, e.raw); gl.uniform1i(pr.u.raw, 0);
  if (pr.u.lut) { gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, lutTex); gl.uniform1i(pr.u.lut, 1); }
  gl.activeTexture(gl.TEXTURE0);
  gl.uniform1ui(pr.u.filters, m.filters >>> 0);
  gl.uniform4fv(pr.u.blk, m.black);
  gl.uniform4fv(pr.u.scl, m.scale);
  if (pr.u.cam0) { gl.uniform3fv(pr.u.cam0, m.cam[0]); gl.uniform3fv(pr.u.cam1, m.cam[1]); gl.uniform3fv(pr.u.cam2, m.cam[2]); }
  gl.uniform2i(pr.u.isize, m.w, m.h);
  gl.uniform2i(pr.u.vp, vp[0], vp[1]);
  gl.uniform1i(pr.u.outH, outH);
  gl.uniform1i(pr.u.flipY, flipY);
  gl.uniform4fv(pr.u.r, [-1, -1, 1, 1]);
}

function renderHalf(raw, m) {
  const hw = m.w >> 1, hh = m.h >> 1;
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, hw, hh);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  const fb = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  gl.viewport(0, 0, hw, hh);
  setCommon(progHalf, { meta: m, raw }, [0, 0], hh, 0);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.deleteFramebuffer(fb);
  return tex;
}

const orientedSize = (m) => (m.flip === 5 || m.flip === 6 ? [m.h, m.w] : [m.w, m.h]);

/// Sensor bounding box of an oriented rectangle.
function sensorBox(m, X, Y, w, h) {
  const map = (x, y) => {
    if (m.flip === 3) return [m.w - 1 - x, m.h - 1 - y];
    if (m.flip === 6) return [y, m.h - 1 - x];
    if (m.flip === 5) return [m.w - 1 - y, x];
    return [x, y];
  };
  const pts = [map(X, Y), map(X + w - 1, Y), map(X, Y + h - 1), map(X + w - 1, Y + h - 1)];
  const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
  const x0 = Math.min(...xs), y0 = Math.min(...ys);
  return { x: x0, y: y0, w: Math.max(...xs) - x0 + 1, h: Math.max(...ys) - y0 + 1 };
}

/// Pass 1 into the green texture for sensor box b (plus 1-pixel margin).
function greenPass(e, b) {
  const gx = b.x - 1, gy = b.y - 1, gw = b.w + 2, gh = b.h + 2;
  if (!greenTex || greenW < gw || greenH < gh) {
    if (greenTex) { gl.deleteTexture(greenTex); gl.deleteFramebuffer(greenFb); }
    greenW = Math.max(gw, greenW); greenH = Math.max(gh, greenH);
    greenTex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, greenTex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.R16UI, greenW, greenH);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    greenFb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, greenFb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, greenTex, 0);
  }
  gl.bindFramebuffer(gl.FRAMEBUFFER, greenFb);
  gl.viewport(0, 0, gw, gh);
  setCommon(progGreen, e, [0, 0], gh, 0);
  gl.uniform2i(progGreen.u.gOrigin, gx, gy);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  return [gx, gy];
}

/// Renders the oriented rectangle (X, Y, w, h) with `method` into the bound framebuffer area.
function finalPass(e, method, flip, X, Y, w, h, vp, flipY) {
  const m = e.meta;
  const b = flip === 0 ? { x: X, y: Y, w, h } : sensorBox(m, X, Y, w, h);
  const g = method === "ppg" ? greenPass(e, b) : [0, 0];
  setCommon(progFinal, e, vp, h, flipY);
  gl.activeTexture(gl.TEXTURE2); gl.bindTexture(gl.TEXTURE_2D, greenTex); gl.uniform1i(progFinal.u.green, 2);
  gl.activeTexture(gl.TEXTURE0);
  gl.uniform2i(progFinal.u.gOrigin, g[0], g[1]);
  gl.uniform1i(progFinal.u.flip, flip);
  gl.uniform2i(progFinal.u.dispOrigin, X, Y);
  gl.uniform1i(progFinal.u.method, method === "linear" ? 0 : 2);
  return b;
}

// ---------- view: fit or 100 % with pan, upright ----------

const view = { pos: 0, zoomed: false, cx: 0.5, cy: 0.5 };

function view100Rect(m) {
  const [ow, oh] = orientedSize(m);
  const w = Math.min(canvas.width, ow), h = Math.min(canvas.height, oh);
  let X = Math.round(view.cx * ow - w / 2), Y = Math.round(view.cy * oh - h / 2);
  X = Math.max(0, Math.min(ow - w, X)); Y = Math.max(0, Math.min(oh - h, Y));
  return { X, Y, w, h, vx: (canvas.width - w) >> 1, vy: (canvas.height - h) >> 1 };
}

function drawView(e) {
  const m = e.meta;
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.clearColor(0.125, 0.125, 0.125, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  if (view.zoomed) {
    const r = view100Rect(m);
    finalPass(e, "ppg", m.flip, r.X, r.Y, r.w, r.h, [r.vx, r.vy], 1);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(r.vx, r.vy, r.w, r.h);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  } else {
    const [ow, oh] = orientedSize(m);
    const s = Math.min(canvas.width / ow, canvas.height / oh);
    const qw = (ow * s) / canvas.width, qh = (oh * s) / canvas.height;
    gl.useProgram(progBlit.p);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, e.half); gl.uniform1i(progBlit.u.t, 0);
    gl.uniform1i(progBlit.u.flip, m.flip);
    gl.uniform4fv(progBlit.u.r, [-qw, -qh, qw, qh]);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }
}

/// Shows the current view; returns ms until the next animation frame.
async function show() {
  const t = now();
  const e = cache.get(view.pos);
  if (!e) return null;
  e.used = now();
  drawView(e);
  await raf();
  return now() - t;
}

// ---------- readback for quality ----------

function readback(e, method, flip, X, Y, w, h) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, w, h);
  const fb = gl.createFramebuffer();
  const b = finalPass(e, method, flip, X, Y, w, h, [0, 0], 0);
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  gl.viewport(0, 0, w, h);
  gl.useProgram(progFinal.p);
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  const px = new Uint8Array(w * h * 4);
  gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.deleteFramebuffer(fb); gl.deleteTexture(tex);
  return { px, box: b };
}

async function quality(pos, e, method, oriented) {
  const m = e.meta;
  // 1920 x 1080 in the orientation the comparison uses, centred
  const flip = oriented ? m.flip : 0;
  const [ow, oh] = oriented ? orientedSize(m) : [m.w, m.h];
  const w = Math.min(1920, ow - 64), h = Math.min(1080, oh - 64);
  const X = ((ow - w) >> 1) & ~1, Y = ((oh - h) >> 1) & ~1;
  const { px, box } = readback(e, method, flip, X, Y, w, h);
  const name = oriented ? `${method}-oriented` : method;
  return invoke("p6_quality", px, { headers: { "p6-idx": String(pos), "p6-rect": `${box.x},${box.y},${box.w},${box.h}`, "p6-method": name, "p6-out-w": String(w), "p6-out-h": String(h) } });
}

// ---------- measurement ----------

const stats = { evictions: 0 };

function summary(v) {
  const s = v.filter((x) => x !== null && x !== undefined).sort((a, b) => a - b);
  if (!s.length) return null;
  const q = (p) => s[Math.min(s.length - 1, Math.max(0, Math.ceil(s.length * p) - 1))];
  return { n: s.length, median: q(0.5), p95: q(0.95), max: s[s.length - 1] };
}

function clearCache() {
  for (const e of cache.values()) { gl.deleteTexture(e.raw); gl.deleteTexture(e.half); }
  cache.clear(); cacheBytes = 0;
}

async function mem(out, label) { out.mem.push({ label, cache_mb: Math.round(cacheBytes / 1048576), ...(await invoke("p6_mem")) }); }

/// Stepping over a virtual folder: positions base..base+n, each its own image (as P5).
async function stepping(out, label, files, steps, prefetch) {
  clearCache();
  const res = [];
  let pos = steps[0].pos;
  view.pos = pos; view.zoomed = false;
  await ensure(pos);
  await show();
  await sleep(1500);
  const prefetchFrom = async (p, dir) => {
    for (const k of [p + dir, p + 2 * dir, p - dir]) {
      if (k < 0 || view.pos !== p) continue;
      try { await ensure(k, [p]); } catch (_) {}
    }
  };
  for (const st of steps) {
    const t0 = now();
    const dir = st.pos >= view.pos ? 1 : -1;
    view.pos = st.pos;
    const hit = cache.has(st.pos);
    let shownAt = null;
    const done = (async () => {
      await ensure(st.pos);
      if (view.pos !== st.pos) return;           // the user moved on first
      await show();
      shownAt = now();
    })();
    if (prefetch) done.then(() => prefetchFrom(st.pos, dir));
    await sleep(st.wait);
    if (shownAt === null && st.last) { await done; }
    res.push({ pos: st.pos, hit, shown: shownAt !== null, ms: shownAt !== null ? shownAt - t0 : null });
  }
  await Promise.allSettled([...ensure.inflight.values()]);
  const shown = res.filter((r) => r.shown);
  out.stepping.push({
    label, steps: res.length,
    shown: shown.length, le30: res.filter((r) => r.shown && r.ms <= 30).length,
    hits: res.filter((r) => r.hit).length,
    time: summary(shown.map((r) => r.ms)),
    evictions: stats.evictions,
  });
  await mem(out, `after stepping ${label}`);
}

async function bench(info) {
  const out = { env: {}, files: [], stepping: [], mem: [], context_loss: null, budget_mb: GPU_BUDGET_MB };
  const dbg = gl.getExtension("WEBGL_debug_renderer_info");
  out.env = { ua: navigator.userAgent, canvas: [canvas.width, canvas.height],
              renderer: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER) };
  await mem(out, "start");
  const n = info.files.length;
  // per file: cold change, zoom, pan, quality
  for (let i = 0; i < n; i++) {
    status(`P6b: ${info.files[i]} (${i + 1}/${n})`);
    const changes = [];
    for (let k = 0; k < 3; k++) {
      clearCache();
      view.pos = i; view.zoomed = false; view.cx = view.cy = 0.5;
      const t0 = now();
      const r = await ensure(i);
      await show();
      changes.push({ total_ms: now() - t0, open_unpack_ms: r.rust.open_unpack_ms, copy_ms: r.rust.copy_ms + r.rust.create_ms,
                     to_page_ms: r.to_page_ms, upload_ms: r.upload_ms });
      await sleep(200);
    }
    const e = cache.get(i);
    const zoom = [], pan = [];
    for (let k = 0; k < 10; k++) {
      view.zoomed = true; view.cx = view.cy = 0.5;
      zoom.push(await show()); await sleep(100);
      view.cx = 0.3 + 0.04 * k; pan.push(await show()); await sleep(100);
      view.zoomed = false; await show(); await sleep(100);
    }
    const q = [];
    try { q.push(await quality(i, e, "linear", false)); } catch (err) { q.push({ method: "linear", error: String(err) }); }
    try { q.push(await quality(i, e, "ppg", false)); } catch (err) { q.push({ method: "ppg", error: String(err) }); }
    try { q.push(await quality(i, e, "ppg", true)); } catch (err) { q.push({ method: "ppg-oriented", error: String(err) }); }
    const pick = (k) => summary(changes.map((c) => c[k]));
    out.files.push({
      file: info.files[i], w: e.meta.w, h: e.meta.h, flip: e.meta.flip,
      change: { total: pick("total_ms"), open_unpack: pick("open_unpack_ms"), copy: pick("copy_ms"), to_page: pick("to_page_ms"), upload: pick("upload_ms") },
      zoom_frame: summary(zoom), pan_frame: summary(pan),
      quality: q,
    });
    await mem(out, `after ${info.files[i]}`);
  }
  // stepping on the A7R set (the first files given): positions are distinct images
  const a7r = Math.min(5, n);
  const rnd = (() => { let s = 12345; return () => ((s = (s * 1103515245 + 12345) >>> 0) / 4294967296); })();
  const natural = [], back = [], fast4 = [], fast7 = [];
  let p = 100;
  for (let k = 0; k < 25; k++) { p += rnd() < 0.15 ? -1 : 1; natural.push({ pos: p, wait: 300 + Math.floor(rnd() * 1200) }); }
  p = 200;
  for (const kk of [1, 2, 3]) { for (let k = 0; k < kk; k++) back.push({ pos: ++p, wait: 800 }); for (let k = 0; k < kk; k++) back.push({ pos: --p, wait: 800 }); }
  p = 300; for (let k = 0; k < 20; k++) fast4.push({ pos: ++p, wait: 250 });
  p = 400; for (let k = 0; k < 20; k++) fast7.push({ pos: ++p, wait: 143 });
  fast4[fast4.length - 1].last = fast7[fast7.length - 1].last = true;
  const files = info.files.slice(0, a7r);
  status("P6b: stepping");
  await stepping(out, "natural 300-1500 ms, prefetch", files, natural, true);
  await stepping(out, "back and forth 1-3 steps, 800 ms, prefetch", files, back, true);
  await stepping(out, "continuous 4/s, prefetch", files, fast4, true);
  await stepping(out, "continuous 7/s, prefetch", files, fast7, true);
  await stepping(out, "natural 300-1500 ms, no prefetch", files, natural.map((s) => ({ ...s, pos: s.pos + 1000 })), false);
  // WebGL context loss and recovery (zoomed view of a vertical file)
  status("P6b: context loss");
  clearCache();
  view.pos = 0; view.zoomed = true; view.cx = view.cy = 0.5;
  await ensure(0); await show(); await sleep(500);
  const ext = gl.getExtension("WEBGL_lose_context");
  if (ext) {
    const restored = new Promise((res) => canvas.addEventListener("webglcontextrestored", res, { once: true }));
    ext.loseContext();
    await sleep(300);
    const t0 = now();
    ext.restoreContext();
    await restored;
    contextLost = false;
    initGL(); cache.clear(); cacheBytes = 0;
    const tr = now();
    await ensure(0); await show();
    out.context_loss = { lost: true, restore_event_ms: tr - t0, reload_and_show_ms: now() - tr, total_ms: now() - t0, view_back: cache.has(0) };
  } else out.context_loss = { lost: false, reason: "WEBGL_lose_context not available" };
  await mem(out, "end");
  await invoke("p6_write", { json: JSON.stringify(out, null, 1) });
  await invoke("p6_exit");
}

// ---------- interactive: RIGHT/LEFT, Z, D/A (latency.ps1) ----------

async function interactive(info) {
  let busy = false;
  view.zoomed = !!info.zoomed;
  const run = async (f) => { if (busy) return; busy = true; try { await f(); } catch (e) { status(`P6b error: ${e}`); } busy = false; };
  const step = async (d) => {
    const dir = d;
    view.pos = Math.max(0, view.pos + d);
    const r = await ensure(view.pos);
    const ms = await show();
    status(`P6b #${view.pos} ${r.cached ? "cached" : "loaded"} ${ms ? ms.toFixed(1) : "-"} ms`);
    for (const k of [view.pos + dir, view.pos + 2 * dir, view.pos - dir]) if (k >= 0) ensure(k, [view.pos]).catch(() => {});
  };
  window.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") run(() => step(1));
    if (e.key === "ArrowLeft") run(() => step(-1));
    if (e.key === "z" || e.key === "Z") run(async () => { view.zoomed = !view.zoomed; const ms = await show(); status(`P6b #${view.pos} ${view.zoomed ? "100 %" : "fit"} ${ms.toFixed(1)} ms`); });
    if (e.key === "d" || e.key === "D" || e.key === "a" || e.key === "A") run(async () => {
      if (!view.zoomed) return;
      const s = (e.key === "d" || e.key === "D") ? 1 : -1;
      view.cx += s * 0.03; if (view.cx > 0.8 || view.cx < 0.2) view.cx = 0.5;
      const ms = await show(); status(`P6b #${view.pos} pan ${ms.toFixed(1)} ms`);
    });
  });
  await run(async () => { await ensure(0); await show(); status(`P6b #0 ${view.zoomed ? "100 %" : "fit"}`); });
  for (const k of [1, 2]) ensure(k, [0]).catch(() => {});
}

(async () => {
  try {
    initGL();
    const info = await invoke("p6_info");
    if (info.bench) { await sleep(1500); await bench(info); }
    else if (info.interactive) await interactive(info);
    else status("P6b ready. Start with --bench or --interactive.");
  } catch (e) {
    status(`P6b error: ${e}`);
    try { await invoke("p6_write", { json: JSON.stringify({ error: String((e && e.stack) || e) }) }); } catch (_) {}
  }
})();
