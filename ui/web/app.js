// PhotoWorkflow – milestone 1 UI: contact sheet, loupe, zoom/pan, EXIF panel.
// Plain JavaScript; talks to Rust through window.__TAURI__ (withGlobalTauri).
"use strict";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Images come from the pw:// scheme, which WebView2 exposes as http://pw.localhost/.
// (window.PW_BASE lets the browser-based UI test point this at a local test server.)
const PW = window.PW_BASE ||
  (navigator.userAgent.includes("Windows") ? "http://pw.localhost/" : "pw://localhost/");

const $ = (id) => document.getElementById(id);
const el = {
  main: $("main"), grid: $("grid"), loupe: $("loupe"), viewport: $("viewport"),
  img: $("loupe-img"), badge: $("loupe-badge"), folder: $("folder"),
  openBtn: $("open-btn"), viewBtn: $("view-btn"), infoBtn: $("info-btn"),
  thumbSize: $("thumb-size"), infoName: $("info-name"), infoList: $("info-list"),
  infoPreviews: $("info-previews"), pos: $("status-pos"), source: $("status-source"),
  zoom: $("status-zoom"), toast: $("toast"),
};

const state = {
  generation: 0,
  files: [],          // [{index, name, size}]
  info: new Map(),    // index -> details (from Rust)
  current: 0,
  mode: "grid",       // "grid" | "loupe"
};

// ---------- helpers ----------

function url(index, size) {
  return `${PW}${state.generation}/${index}/${size}`;
}

function toast(msg) {
  el.toast.textContent = msg;
  el.toast.hidden = false;
  clearTimeout(toast.t);
  toast.t = setTimeout(() => (el.toast.hidden = true), 5000);
}

// EXIF orientation -> rotation (degrees clockwise) and horizontal flip.
function orient(o) {
  switch (o) {
    case 2: return { rot: 0, flip: true };
    case 3: return { rot: 180, flip: false };
    case 4: return { rot: 180, flip: true };
    case 5: return { rot: 90, flip: true };
    case 6: return { rot: 90, flip: false };
    case 7: return { rot: 270, flip: true };
    case 8: return { rot: 270, flip: false };
    default: return { rot: 0, flip: false };
  }
}

async function getInfo(index) {
  if (state.info.has(index)) return state.info.get(index);
  const gen = state.generation;
  const info = await invoke("image_info", { generation: gen, index });
  if (gen === state.generation) state.info.set(index, info);
  return info;
}

const fmtNum = (v, d = 1) => (v == null ? null : Number(v.toFixed(d)).toString());
function fmtDate(s) {
  if (!s) return null;
  const m = /^(\d{4}):(\d{2}):(\d{2}) (.*)$/.exec(s);
  return m ? `${m[1]}-${m[2]}-${m[3]} ${m[4]}` : s;
}

// ---------- folder ----------

async function openFolder(path) {
  try {
    const listing = await invoke("open_folder", { path });
    state.generation = listing.generation;
    state.files = listing.files;
    state.info.clear();
    state.current = 0;
    preload.clear();
    el.folder.textContent = `${listing.folder} — ${listing.files.length} bilder`;
    el.folder.classList.remove("muted");
    el.viewBtn.disabled = listing.files.length === 0;
    try { localStorage.setItem("lastFolder", listing.folder); } catch (_) {}
    buildGrid();
    setMode("grid");
    if (listing.files.length === 0) toast("Inga bildfiler som stöds hittades i mappen.");
  } catch (e) {
    toast(`Kunde inte öppna mappen: ${e}`);
  }
}

async function pickFolder() {
  const path = await invoke("pick_folder");
  if (path) openFolder(path);
}

// ---------- contact sheet ----------

let observer = null;

function buildGrid() {
  if (observer) observer.disconnect();
  el.grid.replaceChildren();
  observer = new IntersectionObserver(onCellsVisible, { root: el.grid, rootMargin: "800px 0px" });
  const frag = document.createDocumentFragment();
  for (const f of state.files) {
    const cell = document.createElement("div");
    cell.className = "cell";
    cell.dataset.index = f.index;
    cell.innerHTML = `<div class="frame"></div><div class="name"></div>`;
    cell.querySelector(".name").textContent = f.name;
    cell.addEventListener("click", () => select(f.index));
    cell.addEventListener("dblclick", () => { select(f.index); setMode("loupe"); });
    frag.appendChild(cell);
    observer.observe(cell);
  }
  el.grid.appendChild(frag);
  select(0);
}

function onCellsVisible(entries) {
  for (const entry of entries) {
    if (!entry.isIntersecting) continue;
    const cell = entry.target;
    observer.unobserve(cell);
    loadThumb(cell, Number(cell.dataset.index));
  }
}

async function loadThumb(cell, index) {
  const gen = state.generation;
  try {
    const info = await getInfo(index);
    if (gen !== state.generation) return;
    const frame = cell.querySelector(".frame");
    if (!info.small) { frame.textContent = "ingen förhandsvisning"; return; }
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.decoding = "async";
    const { rot, flip } = orient(info.orientation);
    img.style.transform = `rotate(${rot}deg) scaleX(${flip ? -1 : 1})`;
    img.onerror = () => { frame.textContent = "kunde inte läsas"; };
    img.src = url(index, "small");
    frame.replaceChildren(img);
  } catch (e) {
    cell.querySelector(".frame").textContent = "fel";
  }
}

function cellOf(index) {
  return el.grid.querySelector(`.cell[data-index="${index}"]`);
}

function select(index) {
  if (state.files.length === 0) return;
  index = Math.max(0, Math.min(state.files.length - 1, index));
  const prev = cellOf(state.current);
  if (prev) prev.classList.remove("selected");
  state.current = index;
  const cell = cellOf(index);
  if (cell) {
    cell.classList.add("selected");
    if (state.mode === "grid") cell.scrollIntoView({ block: "nearest" });
  }
  updateStatus();
  showInfo(index);
}

function gridColumns() {
  const cols = getComputedStyle(el.grid).gridTemplateColumns.split(" ").length;
  return Math.max(1, cols);
}

// ---------- info panel ----------

async function showInfo(index) {
  let info;
  try { info = await getInfo(index); } catch (e) { el.infoName.textContent = String(e); return; }
  if (index !== state.current) return;
  el.infoName.textContent = info.name;
  const rows = [
    ["Kamera", [info.make, info.model].filter(Boolean).join(" ")],
    ["Objektiv", info.lens],
    ["Tagen", fmtDate(info.date_time_original)],
    ["Exponering", info.exposure_time],
    ["Bländare", info.f_number != null ? `f/${fmtNum(info.f_number)}` : null],
    ["ISO", info.iso],
    ["Brännvidd", info.focal_length != null ? `${fmtNum(info.focal_length)} mm` : null],
    ["Exp.-komp.", info.exposure_bias != null ? `${info.exposure_bias > 0 ? "+" : ""}${fmtNum(info.exposure_bias, 2)} EV` : null],
    [info.is_jpeg_file ? "Storlek" : "RAW-storlek", info.width ? `${info.width} × ${info.height}` : null],
    ["Orientering", `${info.orientation} (${orient(info.orientation).rot}°)`],
    ["Format", info.format],
  ];
  el.infoList.replaceChildren();
  for (const [k, v] of rows) {
    if (v == null || v === "") continue;
    const dt = document.createElement("dt"); dt.textContent = k;
    const dd = document.createElement("dd"); dd.textContent = v;
    el.infoList.append(dt, dd);
  }
  el.infoPreviews.replaceChildren();
  for (const p of info.previews) {
    const li = document.createElement("li");
    li.textContent = `${p.width} × ${p.height} · ${Math.round(p.bytes / 1024)} KB`;
    el.infoPreviews.appendChild(li);
  }
  if (!info.previews.length) el.infoPreviews.innerHTML = "<li>inga</li>";
}

// ---------- loupe ----------

const view = {
  index: -1, level: null, w: 0, h: 0, rot: 0, flip: false,
  scale: 1, fit: 1, px: 0, py: 0, loadingLarge: false,
};

// Small cache of decoded neighbour images (medium size) for instant switching.
const preload = new Map(); // key "gen/index" -> HTMLImageElement
function preloadImage(index) {
  if (index < 0 || index >= state.files.length) return;
  const key = `${state.generation}/${index}`;
  if (preload.has(key)) return;
  const img = new Image();
  img.crossOrigin = "anonymous";
  img.src = url(index, "medium");
  img.decode().catch(() => {});
  preload.set(key, img);
  if (preload.size > 12) preload.delete(preload.keys().next().value);
}

async function showLoupe(index) {
  const gen = state.generation;
  let info;
  try { info = await getInfo(index); } catch (e) { toast(String(e)); return; }
  if (gen !== state.generation || index !== state.current) return;
  view.index = index;
  view.loadingLarge = false;
  Object.assign(view, orient(info.orientation));
  if (!info.medium) {
    el.img.removeAttribute("src");
    el.badge.hidden = false;
    el.badge.textContent = "Ingen inbäddad förhandsvisning – RAW-avkodning kommer i M2.";
    return;
  }
  setSource(info, "medium", info.medium.width, info.medium.height);
  fitToWindow();
  for (const d of [1, -1, 2, -2]) preloadImage(index + d);
}

function setSource(info, level, w, h) {
  view.level = level;
  view.w = w;
  view.h = h;
  el.img.style.width = `${w}px`;
  el.img.style.height = `${h}px`;
  el.img.src = url(view.index, level);
  updateBadge(info);
}

function rotatedSize() {
  return view.rot % 180 === 0 ? [view.w, view.h] : [view.h, view.w];
}

function fitToWindow() {
  const r = el.viewport.getBoundingClientRect();
  const [rw, rh] = rotatedSize();
  view.fit = Math.min(r.width / rw, r.height / rh);
  view.scale = view.fit;
  view.px = 0;
  view.py = 0;
  render();
}

function render() {
  const r = el.viewport.getBoundingClientRect();
  const x = r.width / 2 + view.px - view.w / 2;
  const y = r.height / 2 + view.py - view.h / 2;
  const sx = view.flip ? -view.scale : view.scale;
  el.img.style.transform =
    `translate(${x}px, ${y}px) rotate(${view.rot}deg) scale(${sx}, ${view.scale})`;
  updateZoomStatus();
}

/// Zoom to `newScale` keeping the viewport point (cx, cy) fixed.
function zoomTo(newScale, cx, cy) {
  const r = el.viewport.getBoundingClientRect();
  if (cx == null) { cx = r.width / 2; cy = r.height / 2; }
  const minScale = Math.min(view.fit, 1) * 0.5;
  newScale = Math.max(minScale, Math.min(newScale, 8));
  const vx = cx - r.width / 2 - view.px;
  const vy = cy - r.height / 2 - view.py;
  const k = newScale / view.scale;
  view.px = cx - r.width / 2 - vx * k;
  view.py = cy - r.height / 2 - vy * k;
  view.scale = newScale;
  maybeLoadLarge();
  render();
}

/// When zoomed in beyond the medium preview's resolution, switch to the largest embedded JPEG.
function maybeLoadLarge() {
  const info = state.info.get(view.index);
  if (!info || !info.large || view.level !== "medium" || view.loadingLarge) return;
  if (info.large.width <= info.medium.width || view.scale <= 1.05) return;
  view.loadingLarge = true;
  const index = view.index;
  const big = new Image();
  big.crossOrigin = "anonymous";
  big.src = url(index, "large");
  el.badge.hidden = false;
  el.badge.textContent = `Laddar inbäddad JPEG ${info.large.width} × ${info.large.height} …`;
  big.decode().then(() => {
    if (view.index !== index || view.level !== "medium") return;
    const k = view.w / info.large.width; // keep the same on-screen size
    view.scale *= k;
    view.fit *= k;
    setSource(info, "large", info.large.width, info.large.height);
    render();
  }).catch(() => {
    view.loadingLarge = false;
    toast("Kunde inte avkoda den stora förhandsvisningen.");
  });
}

/// 100 % = one pixel of the largest available embedded JPEG per screen pixel.
function zoomTo100(cx, cy) {
  const info = state.info.get(view.index);
  if (!info) return;
  if (info.large && view.level === "medium" && info.large.width > view.w) {
    // Express 100 % of the large image in medium-image pixels; maybeLoadLarge swaps the source.
    zoomTo(info.large.width / view.w, cx, cy);
  } else {
    zoomTo(1, cx, cy);
  }
}

function toggleZoom(cx, cy) {
  if (view.scale > view.fit * 1.01) fitToWindow();
  else zoomTo100(cx, cy);
}

function updateBadge(info) {
  const p = view.level === "large" ? info.large : info.medium;
  if (!p) return;
  const rawW = info.width;
  if (info.is_jpeg_file) { el.badge.hidden = true; return; }
  const parts = [`Inbäddad JPEG ${p.width} × ${p.height} (kamerans rendering, inte RAW-avkodning)`];
  if (rawW && (!info.large || info.large.width < rawW * 0.95)) {
    parts.push(`RAW är ${info.width} × ${info.height} – full upplösning kräver RAW-avkodning (M2)`);
  }
  el.badge.textContent = parts.join(" · ");
  el.badge.hidden = false;
}

function updateZoomStatus() {
  const info = state.info.get(view.index);
  if (!info || state.mode !== "loupe") { el.zoom.textContent = ""; return; }
  const ref = (info.large && info.large.width) || view.w;
  const pct = Math.round(view.scale * (view.w / ref) * 100);
  el.zoom.textContent = `Zoom ${pct} %${view.scale <= view.fit * 1.01 ? " (anpassad)" : ""}`;
}

// Pan with the mouse
let drag = null;
el.viewport.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  drag = { x: e.clientX, y: e.clientY, px: view.px, py: view.py };
  el.viewport.setPointerCapture(e.pointerId);
  el.viewport.classList.add("dragging");
});
el.viewport.addEventListener("pointermove", (e) => {
  if (!drag) return;
  view.px = drag.px + (e.clientX - drag.x);
  view.py = drag.py + (e.clientY - drag.y);
  render();
});
const endDrag = () => { drag = null; el.viewport.classList.remove("dragging"); };
el.viewport.addEventListener("pointerup", endDrag);
el.viewport.addEventListener("pointercancel", endDrag);
el.viewport.addEventListener("wheel", (e) => {
  e.preventDefault();
  const r = el.viewport.getBoundingClientRect();
  const k = Math.exp(-e.deltaY * 0.0015);
  zoomTo(view.scale * k, e.clientX - r.left, e.clientY - r.top);
}, { passive: false });
el.viewport.addEventListener("dblclick", (e) => {
  const r = el.viewport.getBoundingClientRect();
  toggleZoom(e.clientX - r.left, e.clientY - r.top);
});
window.addEventListener("resize", () => { if (state.mode === "loupe") fitToWindow(); });

// ---------- modes, status, keyboard ----------

function setMode(mode) {
  if (state.files.length === 0) mode = "grid";
  state.mode = mode;
  el.grid.hidden = mode !== "grid";
  el.loupe.hidden = mode !== "loupe";
  el.viewBtn.textContent = mode === "grid" ? "Loupe" : "Kontaktark";
  if (mode === "loupe") { showLoupe(state.current); el.loupe.focus(); }
  else { el.badge.hidden = true; select(state.current); el.grid.focus(); }
  updateStatus();
}

function updateStatus() {
  const n = state.files.length;
  el.pos.textContent = n ? `${state.current + 1} / ${n} · ${state.files[state.current].name}` : "";
  el.source.textContent = "";
  updateZoomStatus();
}

function go(delta) {
  select(state.current + delta);
  if (state.mode === "loupe") showLoupe(state.current);
}

document.addEventListener("keydown", (e) => {
  if (e.ctrlKey && e.key.toLowerCase() === "o") { e.preventDefault(); pickFolder(); return; }
  if (e.target.tagName === "INPUT") return;
  const n = state.files.length;
  if (!n) return;
  const key = e.key;
  if (state.mode === "loupe") {
    switch (key) {
      case "ArrowRight": case "ArrowDown": case "PageDown": go(1); break;
      case "ArrowLeft": case "ArrowUp": case "PageUp": go(-1); break;
      case "Home": go(-state.current); break;
      case "End": go(n - 1 - state.current); break;
      case "Escape": case "g": case "G": case "Enter": setMode("grid"); break;
      case "z": case "Z": toggleZoom(); break;
      case "0": fitToWindow(); break;
      case "1": zoomTo100(); break;
      case "+": zoomTo(view.scale * 1.25); break;
      case "-": zoomTo(view.scale / 1.25); break;
      case "i": case "I": toggleInfo(); break;
      default: return;
    }
  } else {
    const cols = gridColumns();
    switch (key) {
      case "ArrowRight": go(1); break;
      case "ArrowLeft": go(-1); break;
      case "ArrowDown": go(cols); break;
      case "ArrowUp": go(-cols); break;
      case "Home": go(-state.current); break;
      case "End": go(n - 1 - state.current); break;
      case "Enter": case "g": case "G": case " ": setMode("loupe"); break;
      case "i": case "I": toggleInfo(); break;
      default: return;
    }
  }
  e.preventDefault();
});

function toggleInfo() {
  el.main.classList.toggle("no-info");
  if (state.mode === "loupe") requestAnimationFrame(fitToWindow);
}

el.openBtn.addEventListener("click", pickFolder);
el.viewBtn.addEventListener("click", () => setMode(state.mode === "grid" ? "loupe" : "grid"));
el.infoBtn.addEventListener("click", toggleInfo);
el.thumbSize.addEventListener("input", () => {
  document.documentElement.style.setProperty("--thumb", `${el.thumbSize.value}px`);
});
el.img.addEventListener("error", () => {
  if (el.img.getAttribute("src")) toast("Kunde inte visa bilden.");
});

// Drag a folder onto the window to open it.
listen("tauri://drag-enter", () => document.body.classList.add("drop-target"));
listen("tauri://drag-leave", () => document.body.classList.remove("drop-target"));
listen("tauri://drag-drop", (ev) => {
  document.body.classList.remove("drop-target");
  const paths = (ev.payload && ev.payload.paths) || [];
  if (paths.length) openFolder(paths[0]);
});

// Start: folder from the command line, else the last folder.
(async () => {
  let start = await invoke("initial_folder");
  if (!start) { try { start = localStorage.getItem("lastFolder"); } catch (_) {} }
  if (start) openFolder(start);
})();
