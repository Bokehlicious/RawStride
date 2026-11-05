// PhotoWorkflow – RAW tiles in the loupe (ADR 0006, step 4).
// When the image on screen (embedded preview or the screen-sized half-size RAW image) has too
// little detail for the zoom level, full-resolution RAW tiles are laid over it:
// - the file is unpacked on the first zoom action (raw_prepare) unless the half-size job's
//   session is already kept;
// - the visible tiles are requested with the one under the mouse pointer first (raw_tiles);
// - a tile is added only once it is decoded, over the image that is already shown, so nothing
//   goes black or blinks; tiles are positioned with the same transform as the image.
// Files whose embedded preview already has the RAW's resolution (A1) never get tiles.
// "RAW-rendering" off = no tiles (M1). Uses globals from app.js and raw.js.
"use strict";

const rt = {
  layer: null,
  index: -1, gen: -1,
  els: new Map(),       // "c,r" -> <img> shown
  loading: new Set(),   // "c,r" fetched/decoding
  lastReq: null,        // last request sent (string), null = none
  prepared: false,      // raw_prepare sent for this image
  need: false,          // tiles needed at the current zoom
  covered: true,        // every visible tile is shown
  visible: [],          // keys of the tiles in the viewport, most important first
};
raw.canTile = new Map(); // index -> LibRaw can render tiles of the file
raw.tileSize = 1024;

function rtLayer() {
  if (!rt.layer) {
    rt.layer = document.createElement("div");
    rt.layer.id = "loupe-tiles";
    el.img.after(rt.layer);
  }
  return rt.layer;
}

function rtReset() {
  for (const e of rt.els.values()) e.remove();
  rt.els.clear();
  rt.loading.clear();
  rt.lastReq = null;
  rt.prepared = false;
  rt.need = false;
  rt.covered = true;
  rt.visible = [];
  rt.index = -1;
}

/// Full RAW width when RAW rendering can show more than the embedded previews (the reference
/// for 100 % zoom), else null. Uses LibRaw's size once known, the EXIF size before that.
function rawTileRef(index) {
  // A file whose RAW failed (damaged or unsupported) keeps the M1 behaviour.
  if (!raw.enabled || !raw.canTile.get(index) || raw.failed.has(index)) return null;
  const info = state.info.get(index);
  if (!info) return null;
  const f = raw.full.get(index);
  const fw = f ? f[0] : info.width;
  if (!fw) return null;
  const best = Math.max(info.large ? info.large.width : 0, info.medium ? info.medium.width : 0);
  return fw > best * 1.05 ? fw : null;
}

/// Width in source pixels of the most detailed image the loupe has (or is loading) without
/// tiles.
function rtAvailable(info) {
  if (view.level === "rawhalf") return view.w;
  return Math.max(view.w, info.large ? info.large.width : 0);
}

function rtGeom(index) {
  const f = raw.full.get(index);
  if (!f) return null;
  const T = raw.tileSize;
  return { fw: f[0], fh: f[1], T, cols: Math.ceil(f[0] / T), rows: Math.ceil(f[1] / T) };
}

/// Image transform pieces, as in app.js render(): screen = T + C + R·S·(p − C).
function rtXf() {
  const r = el.viewport.getBoundingClientRect();
  const a = (view.rot * Math.PI) / 180;
  return {
    W: r.width, H: r.height,
    x: r.width / 2 + view.px - view.w / 2,
    y: r.height / 2 + view.py - view.h / 2,
    cos: Math.cos(a), sin: Math.sin(a),
    sx: view.flip ? -view.scale : view.scale, sy: view.scale,
  };
}

/// Viewport point -> full-image pixel.
function rtToFull(xf, g, X, Y) {
  const dx = X - xf.x - view.w / 2, dy = Y - xf.y - view.h / 2;
  const u = (xf.cos * dx + xf.sin * dy) / xf.sx + view.w / 2;
  const v = (-xf.sin * dx + xf.cos * dy) / xf.sy + view.h / 2;
  return [(u * g.fw) / view.w, (v * g.fh) / view.h];
}

/// Positions a tile with the image's transform (tile pixels -> screen).
function rtPlace(img, col, row, xf, g) {
  const X0 = col * g.T, Y0 = row * g.T;
  const kx = view.w / g.fw, ky = view.h / g.fh;
  const a = xf.cos * xf.sx * kx, b = xf.sin * xf.sx * kx;
  const c = -xf.sin * xf.sy * ky, d = xf.cos * xf.sy * ky;
  const qx = xf.sx * (kx * X0 - view.w / 2), qy = xf.sy * (ky * Y0 - view.h / 2);
  const e = xf.x + view.w / 2 + xf.cos * qx - xf.sin * qy;
  const f = xf.y + view.h / 2 + xf.sin * qx + xf.cos * qy;
  img.style.width = `${Math.min(g.T, g.fw - X0)}px`;
  img.style.height = `${Math.min(g.T, g.fh - Y0)}px`;
  img.style.transform = `matrix(${a}, ${b}, ${c}, ${d}, ${e}, ${f})`;
}

/// Tiles in the viewport, the one under the pointer (or the centre) first, then by distance.
function rtVisibleTiles(xf, g) {
  const pts = [[0, 0], [xf.W, 0], [0, xf.H], [xf.W, xf.H]].map(([X, Y]) => rtToFull(xf, g, X, Y));
  const x0 = Math.max(0, Math.min(...pts.map((p) => p[0])));
  const x1 = Math.min(g.fw, Math.max(...pts.map((p) => p[0])));
  const y0 = Math.max(0, Math.min(...pts.map((p) => p[1])));
  const y1 = Math.min(g.fh, Math.max(...pts.map((p) => p[1])));
  if (x1 <= x0 || y1 <= y0) return [];
  const focusAt = typeof mouseAt !== "undefined" && mouseAt ? mouseAt : [xf.W / 2, xf.H / 2];
  const [fx, fy] = rtToFull(xf, g, focusAt[0], focusAt[1]);
  const out = [];
  for (let r = Math.floor(y0 / g.T); r <= Math.min(g.rows - 1, Math.floor((y1 - 1e-6) / g.T)); r++) {
    for (let c = Math.floor(x0 / g.T); c <= Math.min(g.cols - 1, Math.floor((x1 - 1e-6) / g.T)); c++) {
      const cx0 = c * g.T, cy0 = r * g.T;
      const inside = fx >= cx0 && fx < cx0 + g.T && fy >= cy0 && fy < cy0 + g.T;
      const d = inside ? -1 : Math.hypot(cx0 + g.T / 2 - fx, cy0 + g.T / 2 - fy);
      out.push([d, c, r]);
    }
  }
  out.sort((p, q) => p[0] - q[0]);
  return out.map((p) => [p[1], p[2]]);
}

function rtNeedTiles(info) {
  const ref = rawTileRef(view.index);
  if (!ref) return false;
  const avail = rtAvailable(info);
  const onScreen = view.scale * view.w * (window.devicePixelRatio || 1);
  // Never at "fit": that is what the (screen-sized) half-size image is for.
  const zoomed = view.scale > view.fit * 1.01;
  return zoomed && ref > avail * 1.05 && onScreen > avail * 1.05;
}

function rtSend(tiles) {
  const key = tiles.map((t) => t.join(",")).join(";");
  if (key === rt.lastReq) return;
  rt.lastReq = key;
  invoke("raw_tiles", { generation: state.generation, index: view.index, tiles }).catch(() => {});
}

/// Decides what is needed after the view changed: unpack, tile requests, coverage.
function rtUpdate() {
  if (!raw.enabled || state.mode !== "loupe" || view.index !== rt.index) return;
  const info = state.info.get(view.index);
  if (!info || !raw.canTile.get(view.index)) return;
  if (raw.failed.has(view.index)) { // RAW failed: no unpack, no tiles, drop queued ones
    rt.need = false; rt.covered = true; rt.visible = [];
    if (rt.lastReq) rtSend([]);
    return;
  }
  const zoomed = view.scale > view.fit * 1.01;
  const mayTile = rawTileRef(view.index) != null || (!raw.full.has(view.index) && !info.width);
  if (zoomed && mayTile && !rt.prepared) {
    rt.prepared = true;
    document.dispatchEvent(new CustomEvent("pw-raw-prepare", { detail: { index: view.index } }));
    invoke("raw_prepare", { generation: state.generation, index: view.index }).catch(() => {});
  }
  rt.need = rtNeedTiles(info);
  const g = rtGeom(view.index);
  if (!rt.need || !g) {
    rt.visible = [];
    if (rt.lastReq) rtSend([]); // drop queued tiles nobody needs any more
    rt.covered = !rt.need;      // without the grid (still unpacking) not covered yet
    return;
  }
  const tiles = rtVisibleTiles(rtXf(), g);
  rt.visible = tiles.map((t) => t.join(","));
  const missing = tiles.filter((t) => !rt.els.has(t.join(",")) && !rt.loading.has(t.join(",")));
  rtSend(missing);
  const covered = rt.visible.every((k) => rt.els.has(k));
  if (covered && !rt.covered) {
    document.dispatchEvent(new CustomEvent("pw-tiles-covered",
      { detail: { index: view.index, tiles: rt.visible.length } }));
  }
  rt.covered = covered;
}

/// Called by app.js render(): keeps the tiles on the image and re-evaluates what is needed.
function rawTilesRender() {
  rtLayer();
  if (!raw.enabled || state.mode !== "loupe" || view.index < 0) {
    if (rt.index !== -1) rtReset();
    return;
  }
  if (rt.index !== view.index || rt.gen !== state.generation) {
    rtReset();
    rt.index = view.index;
    rt.gen = state.generation;
  }
  const g = rtGeom(view.index);
  if (g && rt.els.size) {
    const xf = rtXf();
    for (const [k, img] of rt.els) {
      const [c, r] = k.split(",").map(Number);
      rtPlace(img, c, r, xf, g);
    }
  }
  rtUpdate();
}

/// State for the measurement run and tests.
function rawTilesState() {
  return { index: rt.index, need: rt.need, covered: rt.covered, visible: rt.visible.length,
    shown: rt.els.size, loading: rt.loading.size, prepared: rt.prepared };
}

/// Text for the badge while tiles are shown.
function rawTilesBadge() {
  return rt.els.size && rt.index === view.index ? ` · RAW-rutor i full upplösning: ${rt.els.size}` : "";
}

/// Keeps the number of tile elements bounded: drops tiles outside the viewport, oldest first.
function rtEvict() {
  const max = 120;
  if (rt.els.size <= max) return;
  const vis = new Set(rt.visible);
  for (const [k, img] of rt.els) {
    if (rt.els.size <= max - 20) break;
    if (vis.has(k)) continue;
    img.remove();
    rt.els.delete(k);
  }
}

listen("raw-tile", (ev) => {
  const { generation, index, col, row } = ev.payload;
  if (generation !== state.generation || index !== rt.index || !raw.enabled) return;
  const k = `${col},${row}`;
  if (rt.els.has(k) || rt.loading.has(k)) return;
  rt.loading.add(k);
  const img = new Image();
  img.crossOrigin = "anonymous";
  img.className = "raw-tile";
  img.draggable = false;
  img.alt = "";
  img.src = url(index, `tile/${col}/${row}`);
  img.decode().then(() => {
    if (rt.index !== index || rt.gen !== generation || !rt.loading.has(k)) return;
    rt.loading.delete(k);
    const g = rtGeom(index);
    if (!g) return;
    rtPlace(img, col, row, rtXf(), g);
    rtLayer().appendChild(img);   // over the image that is already shown: no gap, no blink
    rt.els.set(k, img);
    document.dispatchEvent(new CustomEvent("pw-tile-shown", { detail: { index, col, row } }));
    const info = state.info.get(index);
    if (info) updateBadge(info);
    rtEvict();
    rtUpdate();
  }).catch(() => {
    // Evicted from the backend cache meanwhile: ask again on the next update.
    rt.loading.delete(k);
    rt.lastReq = null;
    setTimeout(rtUpdate, 50);
  });
});

listen("raw-grid", (ev) => {
  const { generation, index, width, height, tile } = ev.payload;
  if (generation !== state.generation) return;
  raw.full.set(index, [width, height]);
  if (tile) raw.tileSize = tile;
  document.dispatchEvent(new CustomEvent("pw-raw-grid", { detail: { index } }));
  if (index === view.index && state.mode === "loupe") rawTilesRender();
});
