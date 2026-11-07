// ADR 0008 P6c: GPU PPG memory budget and texture-cache validation in presenter B2-res.
// Not part of the app.
//
// The PPG shader, the half-size render and the orientation mapping are copied unchanged from P6b.
// New in P6c:
// - a texture cache under a byte budget (Bayer R16UI + half-size RGBA8 per image), with the P5
//   rules: the current image always loads; prefetch (+1, +2 in the direction of travel, -1 behind,
//   after a 150 ms dwell) starts only if it fits after evicting what the plan does not want;
//   eviction is least recently used and never touches the current image or wanted images when
//   admitting prefetch;
// - two load slots as P5's two workers: an interactive slot for the current image only and a
//   background slot for the plan;
// - upload variants U1 (one texSubImage2D), U2 (pixel unpack buffer), U3 (16 strips) and U4
//   (texture pool: evicted textures are reused for an image of the same size);
// - H0: no stored half-size texture; the fit view is drawn straight from the Bayer texture;
// - the P5 scenarios (S1, S2, S4, S6, S5, NAT, MIX; RAW mode), with the same pauses, positions and
//   random sequence, logged in P5's event format;
// - memory tests (M0 overlap sweep) and stress tests (M1-M5) with readback hashes.
"use strict";

const { invoke } = window.__TAURI__.core;
const canvas = document.getElementById("gl");
const statusEl = document.getElementById("status");
const status = (s) => { statusEl.textContent = s; };
const now = () => performance.now();
const raf = () => new Promise((r) => requestAnimationFrame(r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const MB = 1048576;

let CFG = null; // from p6c_info

// ---------- shared buffers from Rust, matched by position ----------

const pending = new Map();
window.chrome.webview.addEventListener("sharedbufferreceived", (e) => {
  const t = now();
  const buf = e.getBuffer();
  const meta = e.additionalData;
  const p = pending.get(meta.idx);
  if (p) { pending.delete(meta.idx); p({ buf, meta, t }); }
  else { window.chrome.webview.releaseBuffer(buf); invoke("p6c_released", { pos: meta.idx }); }
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


// ---------- GL helpers (from P6b) ----------

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
  if (pr.u.vp) gl.uniform2i(pr.u.vp, vp[0], vp[1]);
  if (pr.u.outH) gl.uniform1i(pr.u.outH, outH);
  if (pr.u.flipY) gl.uniform1i(pr.u.flipY, flipY);
  gl.uniform4fv(pr.u.r, [-1, -1, 1, 1]);
}

// P6c: renders into `tex` when given (a pooled texture of the right size), else into a new one.
function renderHalf(raw, m, tex = null) {
  const hw = m.w >> 1, hh = m.h >> 1;
  if (!tex) {
    tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, hw, hh);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  }
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


function view100Rect(m) {
  const [ow, oh] = orientedSize(m);
  const w = Math.min(canvas.width, ow), h = Math.min(canvas.height, oh);
  let X = Math.round(view.cx * ow - w / 2), Y = Math.round(view.cy * oh - h / 2);
  X = Math.max(0, Math.min(ow - w, X)); Y = Math.max(0, Math.min(oh - h, Y));
  return { X, Y, w, h, vx: (canvas.width - w) >> 1, vy: (canvas.height - h) >> 1 };
}


// H0: the fit view straight from the Bayer texture (no stored half-size texture). Each output pixel
// takes the 2x2 superpixel under it, as the half-size render does, without the bilinear step.
const FS_FITDIRECT = COMMON + `
  uniform highp sampler2D lut;
  uniform vec3 cam0, cam1, cam2;
  uniform int flip;
  uniform vec4 fitRect;   // window rectangle of the fitted image (x, y, w, h), y bottom-up
  uniform vec2 osz;       // oriented image size
  out vec4 o;
  void main() {
    vec2 f = gl_FragCoord.xy - fitRect.xy;
    vec2 uv = vec2(f.x / fitRect.z, 1.0 - f.y / fitRect.w);
    ivec2 q = clamp(ivec2(floor(uv * osz)), ivec2(0), ivec2(osz) - 1);
    ivec2 s = q;
    if (flip == 3) s = ivec2(isize.x - 1 - q.x, isize.y - 1 - q.y);
    else if (flip == 6) s = ivec2(q.y, isize.y - 1 - q.x);
    else if (flip == 5) s = ivec2(isize.x - 1 - q.y, q.x);
    ivec2 p = (s / 2) * 2;
    vec3 sum = vec3(0.0), n = vec3(0.0);
    for (int dy = 0; dy <= 1; dy++) for (int dx = 0; dx <= 1; dx++) {
      ivec2 c2 = p + ivec2(dx, dy); int c = chan(c2); sum[c] += float(S(c2)); n[c] += 1.0; }
    vec3 fv = floor(sum / max(n, vec3(1.0)));
    vec3 v = clamp(floor(vec3(dot(cam0, fv), dot(cam1, fv), dot(cam2, fv))), 0.0, 65535.0);
    ivec3 i = ivec3(v);
    o = vec4(texelFetch(lut, ivec2(i.r & 255, i.r >> 8), 0).r, texelFetch(lut, ivec2(i.g & 255, i.g >> 8), 0).r,
             texelFetch(lut, ivec2(i.b & 255, i.b >> 8), 0).r, 1.0); }`;

// ---------- GL state ----------

let gl, progGreen, progFinal, progHalf, progBlit, progFit, lutTex, greenTex = null, greenFb = null, greenW = 0, greenH = 0;

function initGL() {
  gl = canvas.getContext("webgl2", { antialias: false, alpha: false, powerPreference: "high-performance" });
  progGreen = program(VS, FS_GREEN);
  progFinal = program(VS, FS_FINAL);
  progHalf = program(VS, FS_HALF);
  progBlit = program(VS, FS_BLIT);
  progFit = program(VS, FS_FITDIRECT);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.pixelStorei(gl.PACK_ALIGNMENT, 1);
  lutTex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, lutTex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, 256, 256, 0, gl.RED, gl.UNSIGNED_BYTE, gammaLut());
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
}

function rendererString() {
  const x = gl.getExtension("WEBGL_debug_renderer_info");
  return x ? gl.getParameter(x.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
}

// ---------- texture cache, pool and budget (L1 of the page) ----------

const cache = new Map(); // pos -> { meta, raw, half, bytes, used }
const pool = [];         // U4: { w, h, raw, half, bytes }
let cacheBytes = 0, poolBytes = 0, reserved = 0, budgetBytes = 0;
const st = {
  loads: 0, load_errors: 0, tier_hits: 0, late: 0, discarded: 0, evictions: 0, pool_reuse: 0, pool_deleted: 0,
  refused: 0, overshoot_max_mb: 0, peak_tex_mb: 0, gl_errors: 0, hash_checks: 0, hash_mismatch: 0, black_frames: 0,
  cur_missing_on_draw: 0,
};
const times = { unpack: [], to_page: [], upload: [], half: [] };
let CHECK = false;
let NFILES = 1;

const texBytes = (m) => m.w * m.h * 2 + (CFG.half ? (m.w >> 1) * (m.h >> 1) * 4 : 0);
const used = () => cacheBytes + poolBytes + reserved;

function reportTex() {
  const t = (cacheBytes + poolBytes) / MB;
  st.peak_tex_mb = Math.max(st.peak_tex_mb, t);
  st.overshoot_max_mb = Math.max(st.overshoot_max_mb, t - budgetBytes / MB);
  invoke("p6c_tex", { texMb: cacheBytes / MB, poolMb: poolBytes / MB });
}

function deleteTex(x) { gl.deleteTexture(x.raw); if (x.half) gl.deleteTexture(x.half); }

function dropEntry(k) {
  const e = cache.get(k);
  cache.delete(k); cacheBytes -= e.bytes; st.evictions++;
  deleteTex(e);
}

/// Frees room for `need` bytes: pool first (oldest), then the least recently used image that is not
/// protected. Returns whether it fits.
function makeRoom(need, protect) {
  while (used() + need > budgetBytes) {
    if (pool.length) { const p = pool.shift(); poolBytes -= p.bytes; deleteTex(p); st.pool_deleted++; continue; }
    let victim = null;
    for (const [k, e] of cache) if (!protect.has(k) && (!victim || e.used < victim[1].used)) victim = [k, e];
    if (!victim) return false;
    dropEntry(victim[0]);
  }
  return true;
}

/// Whether `need` bytes would fit after evicting everything evictable (no eviction is done). U4 uses
/// it for admission, so evicted textures can be reused at arrival instead of deleted now.
function canFit(need, protect) {
  let free = poolBytes;
  for (const [k, e] of cache) if (!protect.has(k)) free += e.bytes;
  return used() + need - free <= budgetBytes;
}

function newTextures(m) {
  const raw = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, raw);
  gl.texStorage2D(gl.TEXTURE_2D, 1, gl.R16UI, m.w, m.h);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  let half = null;
  if (CFG.half) {
    half = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, half);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, m.w >> 1, m.h >> 1);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  }
  return { raw, half };
}

/// Textures for a new image. `force` (the current image) proceeds even over the budget.
function acquire(m, protect, force) {
  const need = texBytes(m);
  if (CFG.upload === "U4") {
    const i = pool.findIndex((p) => p.w === m.w && p.h === m.h);
    if (i >= 0) { const p = pool.splice(i, 1)[0]; poolBytes -= p.bytes; st.pool_reuse++; return p; }
    if (used() + need > budgetBytes) {
      // reuse the least recently used evictable image of the same size directly
      let victim = null;
      for (const [k, e] of cache) if (!protect.has(k) && e.meta.w === m.w && e.meta.h === m.h && (!victim || e.used < victim[1].used)) victim = [k, e];
      if (victim) {
        const e = victim[1];
        cache.delete(victim[0]); cacheBytes -= e.bytes; st.evictions++; st.pool_reuse++;
        if (makeRoom(need, protect) || force) return { raw: e.raw, half: e.half };
        pool.push({ w: m.w, h: m.h, raw: e.raw, half: e.half, bytes: e.bytes }); poolBytes += e.bytes;
        return null;
      }
    }
  }
  if (!makeRoom(need, protect) && !force) return null;
  return newTextures(m);
}

function upload(raw, m, u16) {
  gl.bindTexture(gl.TEXTURE_2D, raw);
  if (CFG.upload === "U2") {
    const pbo = gl.createBuffer();
    gl.bindBuffer(gl.PIXEL_UNPACK_BUFFER, pbo);
    gl.bufferData(gl.PIXEL_UNPACK_BUFFER, u16, gl.STREAM_DRAW);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, m.w, m.h, gl.RED_INTEGER, gl.UNSIGNED_SHORT, 0);
    gl.bindBuffer(gl.PIXEL_UNPACK_BUFFER, null);
    gl.deleteBuffer(pbo);
  } else if (CFG.upload === "U3") {
    const rows = Math.ceil(m.h / 16);
    for (let y = 0; y < m.h; y += rows) {
      const h = Math.min(rows, m.h - y);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, y, m.w, h, gl.RED_INTEGER, gl.UNSIGNED_SHORT, u16.subarray(y * m.w, (y + h) * m.w));
    }
  } else {
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, m.w, m.h, gl.RED_INTEGER, gl.UNSIGNED_SHORT, u16);
  }
}

/// Empties the cache (a new folder). U4 keeps the textures in the pool.
function clearCache() {
  for (const [k, e] of [...cache]) {
    cache.delete(k); cacheBytes -= e.bytes;
    if (CFG.upload === "U4") { pool.push({ w: e.meta.w, h: e.meta.h, raw: e.raw, half: e.half, bytes: e.bytes }); poolBytes += e.bytes; }
    else deleteTex(e);
  }
  while (CFG.upload === "U4" && poolBytes > budgetBytes && pool.length) { const p = pool.shift(); poolBytes -= p.bytes; deleteTex(p); st.pool_deleted++; }
  reportTex();
}

function setBudget(mb) { budgetBytes = mb * MB; }

// ---------- scheduler (P5 rules) ----------

const S = { cur: null, dir: 1, zoomed: false, since: 0, gen: 0 };
const view = { cx: 0.5, cy: 0.5 };
const inflight = new Map(); // pos -> job
let slotI = null, slotB = null, lastBytes = 0, dwellTimer = null;
const everResident = new Set(), refusedSet = new Set();
const DWELL = 150;

function wanted(ignoreDwell) {
  if (S.cur === null) return [];
  const v = [S.cur];
  if (ignoreDwell || now() - S.since >= DWELL)
    for (const k of [S.cur + S.dir, S.cur + 2 * S.dir, S.cur - S.dir]) if (k >= 0 && !v.includes(k)) v.push(k);
  return v;
}

const needsLoad = (p) => !cache.has(p) && !inflight.has(p);

function pump() {
  if (S.cur === null) return;
  if (slotI === null && needsLoad(S.cur)) start(S.cur, "I");
  if (slotB === null) {
    for (const p of wanted(false)) {
      if (!needsLoad(p)) continue;
      if (p !== S.cur) {
        const est = lastBytes || 200 * MB;
        const protect = new Set(wanted(true));
        const ok = CFG.upload === "U4" ? canFit(est, protect) : makeRoom(est, protect);
        if (!ok) { st.refused++; refusedSet.add(`${S.gen}:${p}`); continue; }
      }
      start(p, "B");
      break;
    }
  }
}

async function start(p, slot) {
  const job = { slot, gen: S.gen, est: lastBytes || 200 * MB, t0: now() };
  inflight.set(p, job); reserved += job.est;
  if (slot === "I") slotI = p; else slotB = p;
  st.loads++;
  let g = null, r = null;
  try {
    const got = new Promise((res) => pending.set(p, res));
    r = await invoke("p6c_load", { pos: p });
    g = await got;
  } catch (e) {
    pending.delete(p); st.load_errors++; st.last_error = String(e);
  }
  inflight.delete(p); reserved -= job.est;
  if (slot === "I") slotI = null; else slotB = null;
  if (g) arrive(p, g, r, job);
  pump();
}

function arrive(p, g, r, job) {
  const m = g.meta;
  const release = () => { window.chrome.webview.releaseBuffer(g.buf); invoke("p6c_released", { pos: p }); };
  if (job.gen !== S.gen) { release(); st.discarded++; return; }
  lastBytes = texBytes(m);
  times.unpack.push(r.unpack_ms);
  if (r.tier_hit) st.tier_hits++;
  times.to_page.push(g.t - job.t0);
  const isCur = p === S.cur;
  const want = wanted(true);
  if (!isCur && !want.includes(p)) st.late++;
  const protect = new Set([S.cur, p, ...(isCur ? [] : want)]);
  const tex = acquire(m, protect, isCur);
  if (!tex) { release(); st.discarded++; return; }
  const t = now();
  upload(tex.raw, m, new Uint16Array(g.buf));
  release();
  const tu = now();
  const half = CFG.half ? renderHalf(tex.raw, m, tex.half) : null;
  times.upload.push(tu - t); times.half.push(now() - tu);
  if (CHECK) { const err = gl.getError(); if (err) { st.gl_errors++; st.last_gl_error = err; } }
  const e = { meta: m, raw: tex.raw, half, bytes: texBytes(m), used: now() };
  cache.set(p, e); cacheBytes += e.bytes;
  everResident.add(`${S.gen}:${p}`);
  reportTex();
  if (openRec && openRec.pos === p && openRec.ready === null) openRec.ready = now();
  if (isCur) present();
}

// ---------- drawing, frame checks ----------

let lastHash = null;
const expected = new Map();

function drawView(e) {
  const m = e.meta;
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.clearColor(0.125, 0.125, 0.125, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  if (S.zoomed) {
    const r = view100Rect(m);
    finalPass(e, "ppg", m.flip, r.X, r.Y, r.w, r.h, [r.vx, r.vy], 1);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(r.vx, r.vy, r.w, r.h);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    gl.viewport(0, 0, canvas.width, canvas.height);
    return;
  }
  const [ow, oh] = orientedSize(m);
  const s = Math.min(canvas.width / ow, canvas.height / oh);
  const qw = (ow * s) / canvas.width, qh = (oh * s) / canvas.height;
  if (e.half) {
    gl.useProgram(progBlit.p);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, e.half); gl.uniform1i(progBlit.u.t, 0);
    gl.uniform1i(progBlit.u.flip, m.flip);
    gl.uniform4fv(progBlit.u.r, [-qw, -qh, qw, qh]);
  } else {
    const fw = qw * canvas.width, fh = qh * canvas.height;
    setCommon(progFit, e, [0, 0], 0, 0);
    gl.uniform1i(progFit.u.flip, m.flip);
    gl.uniform4f(progFit.u.fitRect, (canvas.width - fw) / 2, (canvas.height - fh) / 2, fw, fh);
    gl.uniform2f(progFit.u.osz, ow, oh);
    gl.uniform4fv(progFit.u.r, [-qw, -qh, qw, qh]);
  }
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

/// Reads 64x64 pixels at the canvas centre right after drawing: hash, black-frame and GL-error checks.
/// The same file, zoom and position must always give the same pixels.
function checkFrame(pos) {
  const n = 64, x = (canvas.width >> 1) - 32, y = (canvas.height >> 1) - 32;
  const px = new Uint8Array(n * n * 4);
  gl.readPixels(x, y, n, n, gl.RGBA, gl.UNSIGNED_BYTE, px);
  let h = 0x811c9dc5, black = true;
  for (let i = 0; i < px.length; i++) {
    h = Math.imul(h ^ px[i], 0x01000193) >>> 0;
    if ((i & 3) !== 3 && Math.abs(px[i] - 32) > 1) black = false;
  }
  if (black) st.black_frames++;
  const err = gl.getError();
  if (err) { st.gl_errors++; st.last_gl_error = err; }
  const key = `${pos % NFILES}|${S.zoomed}|${view.cx.toFixed(3)}|${view.cy.toFixed(3)}`;
  st.hash_checks++;
  if (!expected.has(key)) expected.set(key, h);
  else if (expected.get(key) !== h) st.hash_mismatch++;
  lastHash = h;
  return h;
}

function present() {
  const e = cache.get(S.cur);
  if (!e) { st.cur_missing_on_draw++; return; }
  e.used = now();
  drawView(e);
  if (CHECK) checkFrame(S.cur);
  const r = openRec;
  if (r && r.pos === S.cur && r.drawn === null) raf().then(() => { if (r.drawn === null) r.drawn = now(); });
}

// ---------- the simulated user and its log (P5 event format) ----------

const recs = [];
let openRec = null, scen = "";

function request(label) {
  const t = now();
  if (openRec) { openRec.left = t; openRec = null; }
  const pos = S.cur, hit = cache.has(pos);
  const r = { gen: S.gen, scen, label, pos, view: S.zoomed ? "Zoom" : "Fit", t, hit, ready: hit ? t : null, drawn: null, left: null, miss: "" };
  if (!hit) {
    const k = `${S.gen}:${pos}`;
    r.miss = inflight.has(pos) ? "inflight" : everResident.has(k) ? "evicted" : refusedSet.has(k) ? "refused" : "not-started";
  }
  recs.push(r); openRec = r;
  if (hit) present();
}

function go(pos, label) {
  S.dir = S.cur !== null && pos < S.cur ? -1 : 1;
  S.cur = pos; S.since = now();
  request(label);
  pump();
  clearTimeout(dwellTimer);
  dwellTimer = setTimeout(pump, DWELL + 2);
}

function zoom(z, label) { S.zoomed = z; request(label); pump(); }

async function waitReady(max) {
  const end = now() + max;
  while (now() < end) {
    if (!openRec || openRec.ready !== null) return;
    await sleep(2);
  }
}

async function reset() {
  if (openRec) { openRec.left = now(); openRec = null; }
  S.gen++; S.cur = null; S.zoomed = false;
  while (inflight.size) await sleep(5);
  clearCache();
}

function eventsCsv() {
  let s = "group,strategy,scenario,label,pos,view,t_req,latency,shown,hit,on_screen,drawn,miss\n";
  const strat = `gpu-${CFG.upload}${CFG.half ? "" : "-H0"}${CFG.tier_mb > 0 ? "-K2" : "-K1"}@${CFG.budget_mb}`;
  for (const r of recs) {
    const shown = r.ready !== null && (r.left === null || r.ready <= r.left);
    const lat = shown ? (r.ready - r.t).toFixed(1) : "";
    const drawn = r.drawn !== null && (r.left === null || r.drawn <= r.left) ? (r.drawn - r.t).toFixed(1) : "";
    const onScreen = r.left !== null ? (r.left - r.t).toFixed(1) : "NaN";
    s += `${CFG.tag},${strat},${r.scen},${r.label},${r.pos},${r.view},${r.t.toFixed(1)},${lat},${shown ? 1 : 0},${r.hit ? 1 : 0},${onScreen},${drawn},${r.miss}\n`;
  }
  return s;
}

// ---------- P5 scenarios (RAW mode), as phase1 p4.rs run_all ----------

class Rng {
  constructor(seed) { this.s = BigInt.asUintN(64, seed); }
  next() {
    let x = this.s;
    x = BigInt.asUintN(64, x ^ (x << 13n));
    x = x ^ (x >> 7n);
    x = BigInt.asUintN(64, x ^ (x << 17n));
    this.s = x;
    return x;
  }
  range(lo, hi) { return lo + Number(this.next() % BigInt(hi - lo + 1)); }
}

const mark = (label) => invoke("p6c_mark", { label });

/// `natmixOnly`: S1-S5 are skipped, but their positions are still taken, so NAT and MIX use the
/// same positions (and files) as in P5.
async function runP5(natmixOnly) {
  const settle = 2000;
  let pos = 0;
  const fresh = (n) => { const p = pos; pos += n; return p; };
  if (natmixOnly) fresh(40 + 180 + 15 + 20 + 30);
  else {
    scen = "S1"; await mark("S1"); status("P6c: S1");
    await reset();
    let p = fresh(40);
    go(p, "start"); await waitReady(3000); await sleep(settle);
    for (const w of [0, 50, 150]) for (let i = 0; i < 3; i++) {
      p++;
      go(p, `raw fit change (z after ${w})`);
      await sleep(w);
      zoom(true, `raw zoom ${w} ms after change`);
      await waitReady(3000); await sleep(300);
      zoom(false, "raw zoom out"); await sleep(1000);
    }
    scen = "S2"; await mark("S2"); status("P6c: S2");
    for (const zoomed of [false, true]) for (const rate of [4, 7, 10]) {
      await reset();
      let q = fresh(30);
      go(q, "start");
      if (zoomed) zoom(true, "start");
      await waitReady(3000); await sleep(settle);
      const z = zoomed ? "100%" : "fit";
      for (let i = 0; i < 20; i++) { q++; go(q, `raw ${z} step ${rate}/s`); await sleep(Math.floor(1000 / rate)); }
      await waitReady(3000);
    }
    scen = "S4"; await mark("S4"); status("P6c: S4");
    for (let i = 0; i < 3; i++) {
      await reset();
      const q = fresh(5);
      go(q, "raw cold first view"); await waitReady(5000);
      zoom(true, "raw cold zoom after first view"); await waitReady(5000);
      await sleep(500);
    }
    scen = "S6"; await mark("S6"); status("P6c: S6");
    await reset();
    p = fresh(20);
    go(p, "start"); await waitReady(3000); await sleep(settle);
    for (const pause of [1000, 3000, 5000]) for (let i = 0; i < 2; i++) {
      p++;
      go(p, `raw fit change (z after ${pause})`);
      await sleep(pause);
      zoom(true, `raw zoom after ${pause / 1000} s pause`);
      await waitReady(3000); await sleep(300);
      zoom(false, "raw zoom out"); await sleep(300);
    }
    scen = "S5"; await mark("S5"); status("P6c: S5");
    await reset();
    const p0 = fresh(30);
    go(p0, "start"); await waitReady(3000); await sleep(settle);
    for (let k = 1; k <= 5; k++) {
      const base = S.cur;
      for (let i = 1; i <= k; i++) { go(base + i, "raw fit step"); await sleep(500); }
      for (let i = k - 1; i >= 0; i--) { go(base + i, i === 0 ? `raw fit back after ${k} steps` : "raw fit step back"); await sleep(500); }
      go(base + 1, "raw fit step"); await sleep(800);
    }
  }
  // the JPEG-mode scenarios and S3 of P5 are not run (GPU PPG is RAW mode); their positions are kept
  fresh(40 + 180 + 15 + 20 + 30); fresh(40);
  // NAT
  scen = "NAT"; await mark("NAT"); status("P6c: NAT");
  await reset();
  let p = fresh(100) + 5;
  go(p, "start"); await waitReady(3000); await sleep(settle);
  const rng = new Rng(0x9E3779B97F4A7C15n);
  for (let i = 0; i < 50; i++) {
    p = rng.range(0, 9) === 0 ? Math.max(0, p - 1) : p + 1;
    go(p, "raw change");
    const dwell = rng.range(150, 1500);
    if (rng.range(0, 4) === 0) {
      const before = rng.range(150, Math.max(dwell, 151));
      await sleep(before);
      zoom(true, "raw zoom");
      await sleep(rng.range(1000, 3000));
      zoom(false, "raw zoom out");
      await sleep(dwell - Math.min(before, dwell));
    } else await sleep(dwell);
  }
  // MIX: mode switches are logged but the page stays in RAW mode (only "mix raw ..." compares to P5)
  scen = "MIX"; await mark("MIX"); status("P6c: MIX");
  await reset();
  const lo = fresh(200) + 50, hi = lo + 11;
  go(lo, "start"); await waitReady(3000); await sleep(settle);
  p = lo;
  let dir = 1, mode = "raw";
  for (let i = 0; i < 150; i++) {
    const r = rng.range(0, 99);
    if (r < 60) {
      if ((dir > 0 && p >= hi) || (dir < 0 && p <= lo)) dir = -dir;
      p += dir;
      go(p, `mix ${mode} change`);
    } else if (r < 75) {
      const j = rng.range(0, 1) === 0 ? -3 : 3;
      p = Math.min(hi, Math.max(lo, p + j));
      go(p, `mix ${mode} jump`);
    } else if (r < 90) {
      zoom(true, `mix ${mode} zoom`);
      await sleep(rng.range(500, 2000));
      zoom(false, `mix ${mode} zoom out`);
    } else {
      mode = mode === "raw" ? "jpeg" : "raw";
      request(`mix to ${mode}`);
    }
    await sleep(rng.range(150, 1200));
  }
  await waitReady(3000);
}

// ---------- M0: what textures cost in each counter ----------

async function runM0(out) {
  status("P6c: M0");
  const got = new Promise((res) => pending.set(0, res));
  await invoke("p6c_load", { pos: 0 });
  const g = await got;
  const m = g.meta, u16 = new Uint16Array(g.buf);
  const one = (m.w * m.h * 2) / MB;
  const counts = [0, 2, 4, 8, 11, 16];
  const texs = [];
  const rows = [];
  const snap = async (phase, n, settle = 3000) => {
    gl.finish();
    const s = await invoke("p6c_snap", { label: `m0 ${phase} ${n}`, settleMs: settle });
    rows.push({ phase, n, tex_mb: n * one, ...s });
  };
  await snap("base", 0);
  for (const n of counts.slice(1)) {
    while (texs.length < n) {
      const t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.R16UI, m.w, m.h);
      texs.push(t);
    }
    await snap("alloc", n);
  }
  let up = 0;
  const ut = [];
  for (const n of counts.slice(1)) {
    while (up < n) { const t = now(); upload(texs[up], m, u16); gl.finish(); ut.push(now() - t); up++; }
    await snap("upload", n);
  }
  for (const n of [...counts].reverse().slice(1)) {
    while (texs.length > n) gl.deleteTexture(texs.pop());
    await snap("delete", n);
  }
  await snap("after", 0, 10000);
  window.chrome.webview.releaseBuffer(g.buf);
  invoke("p6c_released", { pos: 0 });
  await snap("buffer released", 0, 3000);
  out.m0 = { image: { w: m.w, h: m.h, mb: one }, upload_ms: ut, rows };
}

// ---------- stress: M1-M5 (with frame checks) ----------

async function runStress(out) {
  CHECK = true;
  const snap = async (label, settle = 2500) => out.snaps.push({ label, tex_mb: (cacheBytes + poolBytes) / MB, ...(await invoke("p6c_snap", { label, settleMs: settle })) });
  // M1: 100 steps forward at the budget limit
  scen = "M1"; await mark("M1"); status("P6c: M1");
  await reset();
  let p = 10000;
  go(p, "start"); await waitReady(5000);
  for (let i = 0; i < 100; i++) { go(++p, "m1 step"); await waitReady(3000); await sleep(600); }
  await snap("M1 end (cache at limit)");
  // M2: zoom to a point, step away until the image is evicted, come back
  scen = "M2"; await mark("M2"); status("P6c: M2");
  out.m2 = [];
  for (let rep = 0; rep < 5; rep++) {
    await reset();
    const base = 20000 + rep * 100;
    go(base, "start"); await waitReady(5000);
    view.cx = 0.3; view.cy = 0.4;
    zoom(true, "m2 zoom"); await sleep(300);
    present();
    const h0 = lastHash;
    let steps = 0;
    for (let k = 1; k <= 30; k++) {
      go(base + k, "m2 away"); await waitReady(3000); await sleep(400);
      steps = k;
      if (!cache.has(base)) break;
    }
    const evicted = !cache.has(base);
    const missBefore = st.cur_missing_on_draw;
    go(base, "m2 return");
    const r = openRec;
    await waitReady(5000);
    await sleep(200);
    out.m2.push({
      rep, steps_away: steps, evicted, hit: r.hit, latency_ms: r.ready !== null ? r.ready - r.t : null,
      drawn_ms: r.drawn !== null ? r.drawn - r.t : null, zoom_kept: S.zoomed === true,
      position_kept: view.cx === 0.3 && view.cy === 0.4, same_pixels: lastHash === h0,
      intermediate_draws: st.cur_missing_on_draw - missBefore,
    });
  }
  view.cx = view.cy = 0.5;
  // M3: tiny budget (about 2 A7R images), 10 images/s, fit and 100 %
  scen = "M3"; await mark("M3"); status("P6c: M3");
  out.m3 = [];
  for (const z of [false, true]) {
    setBudget((2.2 * (lastBytes || 175 * MB)) / MB);
    await reset();
    const peakBefore = st.peak_tex_mb, overBefore = st.overshoot_max_mb;
    st.peak_tex_mb = 0; st.overshoot_max_mb = -Infinity;
    const before = { ...st };
    let q = 30000 + (z ? 1000 : 0);
    go(q, "start"); if (z) zoom(true, "start");
    await waitReady(5000);
    for (let i = 0; i < 40; i++) { go(++q, `m3 10/s ${z ? "100%" : "fit"}`); await sleep(100); }
    await waitReady(5000); await sleep(1000);
    out.m3.push({ zoomed: z, budget_mb: budgetBytes / MB, peak_tex_mb: st.peak_tex_mb, overshoot_max_mb: st.overshoot_max_mb, gl_errors: st.gl_errors - before.gl_errors,
      black_frames: st.black_frames - before.black_frames, hash_mismatch: st.hash_mismatch - before.hash_mismatch,
      cur_missing_on_draw: st.cur_missing_on_draw - before.cur_missing_on_draw, discarded: st.discarded - before.discarded,
      shown: recs.filter((r) => r.scen === "M3" && r.ready !== null && (r.left === null || r.ready <= r.left)).length });
    st.peak_tex_mb = Math.max(st.peak_tex_mb, peakBefore); st.overshoot_max_mb = Math.max(st.overshoot_max_mb, overBefore);
    await reset();
    setBudget(CFG.budget_mb);
  }
  // M4: loads that finish after the user moved on
  scen = "M4"; await mark("M4"); status("P6c: M4");
  {
    const before = { ...st };
    await reset();
    let q = 40000;
    go(q, "start"); await waitReady(5000); await sleep(1000);
    for (let i = 0; i < 5; i++) { go(++q, "m4 skip"); await sleep(100); }
    await sleep(4000);
    out.m4 = { late: st.late - before.late, discarded: st.discarded - before.discarded, tex_mb: (cacheBytes + poolBytes) / MB,
      budget_mb: budgetBytes / MB, inflight: inflight.size };
  }
  // M5: 400 steps, every 500 ms, at the budget limit: drift and release
  scen = "M5"; await mark("M5"); status("P6c: M5");
  await reset();
  await snap("M5 start");
  p = 50000;
  go(p, "start"); await waitReady(5000);
  for (let i = 0; i < 400; i++) {
    go(++p, "m5 step"); await sleep(500);
    if (i % 50 === 49) await mark(`M5 step ${i + 1}`);
  }
  await waitReady(5000);
  await snap("M5 end (cache at limit)");
  await reset();
  for (const pp of [...pool]) { gl.deleteTexture(pp.raw); if (pp.half) gl.deleteTexture(pp.half); }
  pool.length = 0; poolBytes = 0; reportTex();
  await snap("M5 cleared +3 s", 3000);
  await snap("M5 cleared +13 s", 10000);
}

// ---------- interactive (latency.ps1): RIGHT/LEFT, Z, D/A ----------

function interactive() {
  scen = "interactive";
  S.zoomed = !!CFG.zoomed;
  window.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") go(S.cur + 1, "key next");
    else if (e.key === "ArrowLeft") go(Math.max(0, S.cur - 1), "key prev");
    else if (e.key === "z" || e.key === "Z") zoom(!S.zoomed, "key z");
    else if ("dDaA".includes(e.key) && S.zoomed) {
      const s = e.key === "d" || e.key === "D" ? 1 : -1;
      view.cx += s * 0.03; if (view.cx > 0.8 || view.cx < 0.2) view.cx = 0.5;
      present();
    }
    status(`P6c #${S.cur} ${S.zoomed ? "100 %" : "fit"} tex ${((cacheBytes + poolBytes) / MB).toFixed(0)} MB`);
  });
  go(0, "start");
}

// ---------- main ----------

function summary(v) {
  const s = v.filter((x) => x !== null && x !== undefined && !Number.isNaN(x)).sort((a, b) => a - b);
  if (!s.length) return null;
  const q = (p) => s[Math.min(s.length - 1, Math.max(0, Math.ceil(s.length * p) - 1))];
  return { n: s.length, median: q(0.5), p95: q(0.95), max: s[s.length - 1] };
}

(async () => {
  const out = { snaps: [] };
  try {
    CFG = await invoke("p6c_info");
    NFILES = CFG.files.length;
    setBudget(CFG.budget_mb);
    initGL();
    await sleep(1500);
    out.cfg = CFG;
    out.env = { renderer: rendererString(), canvas: [canvas.width, canvas.height], kinds: await invoke("p6c_kinds").catch((e) => String(e)) };
    if (CFG.suite === "interactive") { interactive(); return; }
    out.core_start = await invoke("p6c_core");
    out.snaps.push({ label: "start", ...(await invoke("p6c_snap", { label: "start", settleMs: 2500 })) });
    if (CFG.suite === "m0") await runM0(out);
    else if (CFG.suite === "p5") await runP5(false);
    else if (CFG.suite === "natmix") await runP5(true);
    else if (CFG.suite === "stress") await runStress(out);
    else throw new Error(`unknown suite ${CFG.suite}`);
    out.kinds_end = await invoke("p6c_kinds").catch((e) => String(e));
    out.snaps.push({ label: "end (cache as left)", tex_mb: (cacheBytes + poolBytes) / MB, ...(await invoke("p6c_snap", { label: "end", settleMs: 3000 })) });
    out.core_end = await invoke("p6c_core");
    out.stats = st;
    out.times = Object.fromEntries(Object.entries(times).map(([k, v]) => [k, summary(v)]));
    out.final = { tex_mb: cacheBytes / MB, pool_mb: poolBytes / MB, images: cache.size, pool_items: pool.length };
    await invoke("p6c_write", { name: `${CFG.tag}.json`, text: JSON.stringify(out, null, 1) });
    if (recs.length) await invoke("p6c_write", { name: `${CFG.tag}-events.csv`, text: eventsCsv() });
  } catch (e) {
    out.error = String((e && e.stack) || e);
    try { await invoke("p6c_write", { name: `${CFG ? CFG.tag : "p6c"}.json`, text: JSON.stringify(out, null, 1) }); } catch (_) {}
  }
  try { await invoke("p6c_dump"); } catch (_) {}
  await invoke("p6c_exit");
})();
