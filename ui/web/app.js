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
  zoom: $("status-zoom"), toast: $("toast"), anchor: $("status-anchor"),
  filterStars: $("filter-stars"), filterLabel: $("filter-label"), sel: $("status-sel"),
};

const state = {
  generation: 0,
  files: [],          // [{index, name, size}]
  info: new Map(),    // index -> details (from Rust)
  ratings: new Map(), // index -> {stars, label}
  current: 0,
  mode: "grid",       // "grid" | "loupe"
  visible: [],        // indexes shown under the current filter, in order
  pos: new Map(),     // index -> position in `visible`
  picked: new Set(),  // multi-selection (indexes); empty = only `current`
  anchor: 0,          // start of a Shift range
  filter: { stars: "all", label: "all" },
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
    if (window.rawFolder) rawFolder(listing.generation); // RAW rendering (raw.js)
    state.files = listing.files;
    state.info.clear();
    state.ratings.clear();
    state.current = 0;
    state.picked.clear();
    state.anchor = 0;
    preload.clear();
    el.folder.textContent = `${listing.folder} — ${listing.files.length} bilder`;
    el.folder.classList.remove("muted");
    el.viewBtn.disabled = listing.files.length === 0;
    try { localStorage.setItem("lastFolder", listing.folder); } catch (_) {}
    buildGrid();
    setMode("grid");
    if (listing.files.length === 0) toast("Inga bildfiler som stöds hittades i mappen.");
    await loadRatings(listing.generation);
    if (state.filter.stars !== "all" || state.filter.label !== "all") applyFilter(false);
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
    cell.innerHTML =
      `<div class="frame"></div><div class="label-bar"><span class="stars"></span></div>` +
      `<div class="name"></div>`;
    cell.querySelector(".name").textContent = f.name;
    cell.addEventListener("click", (e) => clickCell(f.index, e));
    cell.addEventListener("dblclick", () => { select(f.index); setMode("loupe"); });
    frag.appendChild(cell);
    observer.observe(cell);
  }
  el.grid.appendChild(frag);
  state.visible = state.files.map((f) => f.index);
  state.pos = new Map(state.visible.map((idx, p) => [idx, p]));
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
    [info.is_jpeg_file || info.is_tiff_file ? "Storlek" : "RAW-storlek", info.width ? `${info.width} × ${info.height}` : null],
    ["Orientering", `${info.orientation} (${orient(info.orientation).rot}°)`],
    ["Format", info.format],
    ["Betyg", ratingText(index)],
    ["Etikett", (() => { const l = (state.ratings.get(index) || {}).label; return l ? (LABEL_SV[l] || l) : null; })()],
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
  try { info = await getInfo(index); } catch (e) {
    if (gen === state.generation && index === state.current) showUnreadable(index, e);
    return;
  }
  if (gen !== state.generation || index !== state.current) return;
  view.index = index;
  view.loadingLarge = false;
  renderRating(index);
  Object.assign(view, orient(info.orientation));
  if (!info.medium) {
    view.level = null;
    el.img.removeAttribute("src");
    el.badge.hidden = false;
    el.badge.textContent = (window.rawFailedText && rawFailedText(index)) ||
      "Ingen inbäddad förhandsvisning – väntar på RAW-avkodning.";
    if (window.rawTilesRender) rawTilesRender(); // no tiles of the previous image stay
    if (window.rawWarnUpdate) rawWarnUpdate();
    if (window.rawShow) rawShow(index);
    return;
  }
  setSource(info, "medium", info.medium.width, info.medium.height);
  fitToWindow();
  for (const d of [1, -1, 2, -2]) preloadImage(index + d);
  // The embedded preview (M1) is shown first; RAW pixels replace it when ready (raw.js).
  if (window.rawShow) rawShow(index);
}

/// A file that cannot be read: the loupe is emptied (no image of the previous file stays on
/// screen) and says why. RAW work for the previous image stops.
function showUnreadable(index, error) {
  view.index = index;
  view.level = null;
  view.loadingLarge = false;
  el.img.removeAttribute("src");
  el.img.style.width = "0px";
  el.img.style.height = "0px";
  renderRating(index);
  el.badge.textContent = `Filen kan inte läsas: ${String(error)}`;
  el.badge.hidden = false;
  el.zoom.textContent = "";
  if (window.rawTilesRender) rawTilesRender(); // tiles of the previous image go
  if (window.rawWarnUpdate) rawWarnUpdate();
  if (window.rawShow) rawShow(index); // the backend stops working on the previous image
  document.dispatchEvent(new CustomEvent("pw-unreadable", { detail: { index, error: String(error) } }));
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
  if (window.rawTilesRender) rawTilesRender(); // RAW tiles follow the image (rawtiles.js)
  if (window.rawWarnUpdate) rawWarnUpdate();   // warning chip of the image on screen
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
  el.badge.textContent = info.is_tiff_file
    ? `Avkodar TIFF i full upplösning ${info.large.width} × ${info.large.height} …`
    : `Laddar inbäddad JPEG ${info.large.width} × ${info.large.height} …`;
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
/// Width of the full RAW image while a half-size RAW image is shown (twice its width if the
/// backend did not report it).
function rawhalfRef() {
  const w = typeof rawFullWidth === "function" ? rawFullWidth(view.index) : null;
  return w || view.w * 2;
}

/// Full RAW width when RAW rendering shows more than the embedded previews (null otherwise).
function rawRefWidth() {
  return typeof rawTileRef === "function" ? rawTileRef(view.index) : null;
}

function zoomTo100(cx, cy) {
  const info = state.info.get(view.index);
  if (!info) return;
  const fw = rawRefWidth();
  if (fw) { zoomTo(fw / view.w, cx, cy); return; } // 100 % of the RAW (tiles)
  if (view.level === "rawhalf") { zoomTo(rawhalfRef() / view.w, cx, cy); return; } // 100 % of the RAW
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
  if (view.level === "rawhalf") {
    // Half-size RAW, scaled to the screen: show both the shown size and the full RAW size.
    const fw = rawhalfRef();
    const full = fw !== view.w * 2 ? ` (av ${fw} px bredd)` : "";
    el.badge.textContent = `RAW-avkodning (LibRaw), skärmanpassad ${view.w} × ${view.h}${full}` +
      (window.rawTilesBadge ? rawTilesBadge() : "");
    el.badge.hidden = false;
    return;
  }
  const p = view.level === "large" ? info.large : info.medium;
  if (!p) return;
  const rawW = info.width;
  if (info.is_jpeg_file) { el.badge.hidden = true; return; }
  if (info.is_tiff_file) {
    el.badge.textContent = `TIFF avkodad till ${p.width} × ${p.height}` +
      (view.level === "large" ? " (full upplösning)" : " – zooma in för full upplösning");
    el.badge.hidden = false;
    return;
  }
  const parts = [`Inbäddad JPEG ${p.width} × ${p.height} (kamerans rendering, inte RAW-avkodning)`];
  const failed = window.rawFailedText && rawFailedText(view.index);
  if (rawW && !failed && (!info.large || info.large.width < rawW * 0.95)) {
    parts.push(window.rawTileRef && rawTileRef(view.index)
      ? `RAW är ${info.width} × ${info.height} – zooma in för RAW-rutor i full upplösning`
      : `RAW är ${info.width} × ${info.height} – full upplösning kräver RAW-avkodning (M2)`);
  }
  if (failed) parts.push(failed);
  el.badge.textContent = parts.join(" · ") + (window.rawTilesBadge ? rawTilesBadge() : "");
  el.badge.hidden = false;
}

function updateZoomStatus() {
  const info = state.info.get(view.index);
  if (!info || state.mode !== "loupe") { el.zoom.textContent = ""; return; }
  const ref = rawRefWidth() ||
    (view.level === "rawhalf" ? rawhalfRef() : (info.large && info.large.width) || view.w);
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
// ---------- zoom anchor: at the mouse cursor (default) or at the centre ----------

// Last mouse position over the loupe, in viewport coordinates (null = mouse elsewhere).
let mouseAt = null;
let anchorAtCursor = true;
try { anchorAtCursor = localStorage.getItem("zoomAnchor") !== "center"; } catch (_) {}

el.loupe.addEventListener("pointermove", (e) => {
  const r = el.viewport.getBoundingClientRect();
  mouseAt = [e.clientX - r.left, e.clientY - r.top];
});
el.loupe.addEventListener("pointerleave", () => { mouseAt = null; });

/// The point to zoom around: the given mouse point or the last known cursor position when
/// anchoring at the cursor, else the centre (undefined = centre in zoomTo).
function anchor(x, y) {
  if (!anchorAtCursor) return [];
  if (x != null) return [x, y];
  return mouseAt ? [...mouseAt] : [];
}

function renderAnchorToggle() {
  el.anchor.textContent = anchorAtCursor ? "Zoom mot pekaren" : "Zoom mot mitten";
  el.anchor.title = "Klicka för att växla vad zoomen förankras vid (A)";
}

function toggleAnchor() {
  anchorAtCursor = !anchorAtCursor;
  try { localStorage.setItem("zoomAnchor", anchorAtCursor ? "cursor" : "center"); } catch (_) {}
  renderAnchorToggle();
  toast(anchorAtCursor ? "Zoom förankras vid muspekaren." : "Zoom förankras vid bildens mitt.");
}
el.anchor.addEventListener("click", toggleAnchor);
renderAnchorToggle();

el.viewport.addEventListener("wheel", (e) => {
  e.preventDefault();
  const r = el.viewport.getBoundingClientRect();
  const k = Math.exp(-e.deltaY * 0.0015);
  zoomTo(view.scale * k, ...anchor(e.clientX - r.left, e.clientY - r.top));
}, { passive: false });
el.viewport.addEventListener("dblclick", (e) => {
  const r = el.viewport.getBoundingClientRect();
  toggleZoom(...anchor(e.clientX - r.left, e.clientY - r.top));
});
window.addEventListener("resize", () => { if (state.mode === "loupe") fitToWindow(); });

// ---------- modes, status, keyboard ----------

function setMode(mode) {
  if (state.visible.length === 0) mode = "grid";
  state.mode = mode;
  el.grid.hidden = mode !== "grid";
  el.loupe.hidden = mode !== "loupe";
  el.viewBtn.textContent = mode === "grid" ? "Loupe" : "Kontaktark";
  if (mode === "loupe") { showLoupe(state.current); el.loupe.focus(); }
  else {
    el.badge.hidden = true;
    if (window.rawLeave) rawLeave(); // no RAW work for the contact sheet
    select(state.current);
    el.grid.focus();
  }
  updateStatus();
}

function updateStatus() {
  const n = state.visible.length;
  const total = state.files.length;
  const p = state.pos.get(state.current);
  el.pos.textContent = n && p != null
    ? `${p + 1} / ${n}${n < total ? ` (av ${total})` : ""} · ${state.files[state.current].name}`
    : total ? `Inga bilder matchar filtret (${total} i mappen)` : "";
  el.sel.textContent = state.picked.size > 1 ? `${state.picked.size} markerade` : "";
  el.source.textContent = "";
  updateZoomStatus();
}

/// Moves `delta` positions among the visible images. With `extend` (Shift), the multi-selection
/// grows from the anchor; otherwise it is cleared.
function go(delta, extend = false) {
  // Images that stopped matching the filter (after a rating) disappear when you move on.
  applyFilter(true);
  if (!state.visible.length) return;
  const p = state.pos.has(state.current) ? state.pos.get(state.current) : 0;
  const np = Math.max(0, Math.min(state.visible.length - 1, p + delta));
  const index = state.visible[np];
  if (extend) {
    if (!state.picked.size) state.anchor = state.current;
    pickRange(state.anchor, index);
  } else if (state.picked.size) {
    clearPicked();
  }
  if (!extend) state.anchor = index;
  select(index);
  // Now drop the previous image if it no longer matches (the new current one is kept).
  applyFilter(true);
  if (state.mode === "loupe") showLoupe(state.current);
}

document.addEventListener("keydown", (e) => {
  if (e.ctrlKey && e.key.toLowerCase() === "o") { e.preventDefault(); pickFolder(); return; }
  if (e.target.tagName === "INPUT" || e.target.tagName === "SELECT") return;
  if (!state.files.length) return;
  const n = state.visible.length;
  if (!n) return;
  if (handleRatingKey(e)) { e.preventDefault(); return; }
  const key = e.key;
  const ext = e.shiftKey;
  if (e.ctrlKey && !e.altKey && key.toLowerCase() === "a") { e.preventDefault(); pickAll(); return; }
  if (state.mode === "loupe") {
    switch (key) {
      case "ArrowRight": case "ArrowDown": case "PageDown": go(1); break;
      case "ArrowLeft": case "ArrowUp": case "PageUp": go(-1); break;
      case "Home": go(-n); break;
      case "End": go(n); break;
      case "Escape": case "g": case "G": case "Enter": setMode("grid"); break;
      case "z": case "Z": toggleZoom(...anchor()); break;
      case "f": case "F": fitToWindow(); break;
      case "+": case "=": zoomTo(view.scale * 1.25, ...anchor()); break;
      case "-": zoomTo(view.scale / 1.25, ...anchor()); break;
      case "a": case "A": toggleAnchor(); break;
      case "i": case "I": toggleInfo(); break;
      default: return;
    }
  } else {
    const cols = gridColumns();
    switch (key) {
      case "ArrowRight": go(1, ext); break;
      case "ArrowLeft": go(-1, ext); break;
      case "ArrowDown": go(cols, ext); break;
      case "ArrowUp": go(-cols, ext); break;
      case "Home": go(-n, ext); break;
      case "End": go(n, ext); break;
      case "Escape": clearPicked(); updateStatus(); break;
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
// Lets raw.js and the measurement run (bench.js) know what is on screen.
el.img.addEventListener("load", () => {
  document.dispatchEvent(new CustomEvent("pw-shown", { detail: { index: view.index, level: view.level } }));
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


// ---------- ratings and colour labels (XMP sidecar via ExifTool) ----------

const LABELS = ["Red", "Yellow", "Green", "Blue", "Purple"];
const LABEL_COLORS = { Red: "#e5484d", Yellow: "#f5c400", Green: "#3fb950", Blue: "#4c8dff", Purple: "#a371f7" };
const LABEL_SV = { Red: "Röd", Yellow: "Gul", Green: "Grön", Blue: "Blå", Purple: "Lila" };

function ratingText(index) {
  const r = state.ratings.get(index);
  if (!r || r.stars == null) return null;
  return r.stars < 0 ? "Avvisad" : "★".repeat(r.stars) + "☆".repeat(5 - r.stars);
}

async function loadRatings(gen) {
  try {
    const list = await invoke("read_ratings", { generation: gen });
    if (gen !== state.generation) return;
    for (const r of list) {
      state.ratings.set(r.index, { stars: r.stars, label: r.label });
      renderRating(r.index);
    }
  } catch (e) {
    toast(`Kunde inte läsa betyg: ${e}`);
  }
}

/// The rating/label bar under a thumbnail (and in the loupe): a rounded bar in the label colour
/// with the stars inside; without a label only the stars show.
function paintLabelBar(bar, r) {
  const stars = bar.querySelector(".stars");
  if (r.stars == null) stars.textContent = "";
  else if (r.stars < 0) stars.textContent = "✕";
  else stars.innerHTML = "★".repeat(r.stars) + `<span class="empty">${"•".repeat(5 - r.stars)}</span>`;
  bar.classList.toggle("labelled", !!r.label);
  bar.classList.toggle("rejected", r.stars != null && r.stars < 0);
  bar.style.background = r.label ? (LABEL_COLORS[r.label] || "#888") : "";
  const sv = r.label ? (LABEL_SV[r.label] || r.label) : "";
  const st = r.stars == null ? "" : r.stars < 0 ? "Avvisad" : `${r.stars} stjärn${r.stars === 1 ? "a" : "or"}`;
  bar.title = [st, sv].filter(Boolean).join(" · ");
}

function renderRating(index) {
  const r = state.ratings.get(index) || {};
  const cell = cellOf(index);
  if (cell) {
    paintLabelBar(cell.querySelector(".label-bar"), r);
  }
  if (index === state.current) {
    const el2 = document.getElementById("loupe-rating");
    if (el2) {
      const bar = el2.querySelector(".label-bar") || el2.appendChild(Object.assign(
        document.createElement("div"), { className: "label-bar", innerHTML: '<span class="stars"></span>' }));
      paintLabelBar(bar, r);
      el2.hidden = r.stars == null && !r.label;
    }
    showInfo(index);
  }
}

async function setRating(index, change, quiet = false) {
  const f = state.files[index];
  if (!f) return;
  if (!f.ratable) {
    if (!quiet) toast("Betyg och etiketter kan inte sparas för den här filtypen.");
    return;
  }
  const before = { ...(state.ratings.get(index) || { stars: null, label: null }) };
  const after = { ...before, ...change };
  if (after.stars === 0) after.stars = null;
  state.ratings.set(index, after);
  renderRating(index);
  const gen = state.generation;
  // Writes go out one at a time, in key-press order, so the last key always wins.
  writeQueue = writeQueue.then(() =>
    invoke("set_rating", { generation: gen, index, stars: after.stars, label: after.label })
  ).catch((e) => {
    if (gen === state.generation) { state.ratings.set(index, before); renderRating(index); }
    toast(`Kunde inte spara betyget: ${e}`);
  });
}
let writeQueue = Promise.resolve();

/// 0–5 / Alt+0–5: stars. 6–9: Red/Yellow/Green/Blue. Ctrl+1–5: Red…Purple, Ctrl+0: no label.
/// Pressing the label an image already has removes it.
/// With a multi-selection, rating keys apply to every marked image.
function handleRatingKey(e) {
  const m = /^(?:Digit|Numpad)(\d)$/.exec(e.code);
  if (!m || e.shiftKey || e.metaKey) return false;
  const d = Number(m[1]);
  const targets = ratingTargets();
  const apply = (change) => {
    const multi = targets.length > 1;
    if (multi && !targets.some((i) => state.files[i].ratable)) {
      toast("Inga av de markerade filerna kan få betyg.");
      return;
    }
    for (const i of targets) setRating(i, change, multi);
  };
  // The same label again removes it (from all targets, if all of them have it).
  const toggle = (label) => {
    const ratable = targets.filter((i) => state.files[i].ratable);
    const all = ratable.length > 0 &&
      ratable.every((i) => (state.ratings.get(i) || {}).label === label);
    apply({ label: all ? null : label });
  };
  if (e.ctrlKey && !e.altKey) {
    if (d === 0) apply({ label: null });
    else if (d <= LABELS.length) toggle(LABELS[d - 1]);
    else return false;
    return true;
  }
  if (e.ctrlKey) return false; // Ctrl+Alt (AltGr) is used for characters on some layouts
  if (d <= 5) { apply({ stars: d }); return true; }
  if (!e.altKey && d <= 9) { toggle(LABELS[d - 6]); return true; }
  return false;
}

// ---------- multi-selection ----------

/// The images a rating key acts on: in the contact sheet, the marked ones (in grid order) if
/// the current image is among them; otherwise (and always in the loupe) the current image.
function ratingTargets() {
  if (state.mode === "grid" && state.picked.size > 1 && state.picked.has(state.current)) {
    return state.visible.filter((i) => state.picked.has(i));
  }
  return [state.current];
}

function markCell(index, on) {
  const c = cellOf(index);
  if (c) c.classList.toggle("picked", on);
}

function clearPicked() {
  for (const i of state.picked) markCell(i, false);
  state.picked.clear();
}

/// Marks every visible image between `a` and `b` (inclusive), replacing the marking.
function pickRange(a, b) {
  clearPicked();
  let pa = state.pos.get(a);
  let pb = state.pos.get(b);
  if (pa == null) pa = pb;
  if (pa > pb) [pa, pb] = [pb, pa];
  for (let p = pa; p <= pb; p++) {
    state.picked.add(state.visible[p]);
    markCell(state.visible[p], true);
  }
}

function pickAll() {
  if (state.mode !== "grid") return;
  clearPicked();
  for (const i of state.visible) { state.picked.add(i); markCell(i, true); }
  updateStatus();
}

/// Click: select. Ctrl+click: add/remove from the marking. Shift+click: mark a range.
function clickCell(index, e) {
  if (e.shiftKey) {
    if (!state.picked.size) state.anchor = state.current;
    pickRange(state.anchor, index);
  } else if (e.ctrlKey) {
    if (!state.picked.size) { state.picked.add(state.current); markCell(state.current, true); }
    if (state.picked.has(index) && state.picked.size > 1) {
      state.picked.delete(index);
      markCell(index, false);
    } else {
      state.picked.add(index);
      markCell(index, true);
    }
    state.anchor = index;
  } else {
    clearPicked();
    state.anchor = index;
  }
  select(index);
}

// ---------- filter by rating and label ----------

function matches(index) {
  const r = state.ratings.get(index) || {};
  const s = r.stars == null ? null : r.stars;
  const fs = state.filter.stars;
  const okStars = fs === "all" ? true
    : fs === "unrated" ? s == null
    : fs === "rejected" ? s != null && s < 0
    : s != null && s >= Number(fs);
  const fl = state.filter.label;
  const okLabel = fl === "all" ? true : fl === "none" ? !r.label : r.label === fl;
  return okStars && okLabel;
}

/// Recomputes which images are shown. `keepCurrent` keeps the current image visible even if
/// it no longer matches (so rating an image does not make it vanish under the cursor).
function applyFilter(keepCurrent) {
  if (!state.files.length) return;
  const vis = [];
  for (const f of state.files) {
    const show = matches(f.index) || (keepCurrent && f.index === state.current);
    const c = cellOf(f.index);
    if (c) c.hidden = !show;
    if (show) vis.push(f.index);
  }
  state.visible = vis;
  state.pos = new Map(vis.map((idx, p) => [idx, p]));
  for (const i of [...state.picked]) {
    if (!state.pos.has(i)) { state.picked.delete(i); markCell(i, false); }
  }
  el.grid.classList.toggle("empty", vis.length === 0);
  if (!vis.length) {
    if (state.mode === "loupe") setMode("grid");
    updateStatus();
    return;
  }
  if (!state.pos.has(state.current)) {
    // Move to the nearest following visible image.
    const next = vis.find((i) => i > state.current);
    select(next != null ? next : vis[vis.length - 1]);
    if (state.mode === "loupe") showLoupe(state.current);
  } else {
    updateStatus();
  }
}

/// An active filter is outlined so a folder that looks half-empty is never a mystery.
function markFilters() {
  el.filterStars.classList.toggle("active", state.filter.stars !== "all");
  el.filterLabel.classList.toggle("active", state.filter.label !== "all");
}

function setFilter(change) {
  Object.assign(state.filter, change);
  markFilters();
  try { localStorage.setItem("filter", JSON.stringify(state.filter)); } catch (_) {}
  applyFilter(false);
  updateStatus();
  (state.mode === "grid" ? el.grid : el.loupe).focus();
}

(() => {
  try {
    const saved = JSON.parse(localStorage.getItem("filter") || "null");
    if (saved && typeof saved === "object") Object.assign(state.filter, saved);
  } catch (_) {}
  el.filterStars.value = state.filter.stars;
  el.filterLabel.value = state.filter.label;
  if (el.filterStars.value !== state.filter.stars) state.filter.stars = el.filterStars.value = "all";
  if (el.filterLabel.value !== state.filter.label) state.filter.label = el.filterLabel.value = "all";
  markFilters();
  el.filterStars.addEventListener("change", () => setFilter({ stars: el.filterStars.value }));
  el.filterLabel.addEventListener("change", () => setFilter({ label: el.filterLabel.value }));
})();
