// PhotoWorkflow – RAW rendering in the loupe (ADR 0006, step 3: half-size images; tiles are in
// rawtiles.js).
// The embedded preview (M1) is always shown first; when the backend reports that the half-size
// RAW image is ready ("raw-ready"), it replaces the preview without changing the framing.
// "RAW-rendering" off = M1 only. Uses globals from app.js (state, view, el, url, setSource, …).
"use strict";

const raw = {
  enabled: true,
  ready: new Set(),      // indexes with a half-size image ready (current folder)
  failed: new Map(),     // index -> message
  wanted: new Map(),     // index -> backend said a half-size image is wanted
  full: new Map(),       // index -> [width, height] of the full RAW image (100 % zoom)
  btn: null,
  warn: null,            // the warning chip in the loupe (RAW failed, preview shown)
  seq: 0,                // order of raw_view calls (the backend ignores older ones)
};
try { raw.enabled = localStorage.getItem("rawRendering") !== "off"; } catch (_) {}

/// Screen long edge in physical pixels (decides whether the embedded preview is enough).
function rawScreenEdge() {
  return Math.round(Math.max(screen.width, screen.height) * (window.devicePixelRatio || 1));
}

function rawNeighbours(index) {
  const p = state.pos.get(index);
  if (p == null) return [];
  return [p + 1, p - 1].filter((q) => q >= 0 && q < state.visible.length).map((q) => state.visible[q]);
}

async function rawShow(index) {
  if (!raw.enabled) return;
  const gen = state.generation;
  let r;
  try {
    r = await invoke("raw_view", {
      generation: gen, index, neighbours: rawNeighbours(index), screenEdge: rawScreenEdge(),
      seq: ++raw.seq,
    });
  } catch (_) { return; }
  if (gen !== state.generation || !r) return;
  raw.wanted.set(index, !!r.want_half);
  raw.canTile.set(index, !!r.can_tile);
  if (r.grid) { raw.full.set(index, [r.grid[0], r.grid[1]]); raw.tileSize = r.grid[2]; }
  document.dispatchEvent(new CustomEvent("pw-raw-wanted", { detail: { index, want: !!r.want_half } }));
  if (r.want_half && raw.ready.has(index)) rawSwapToHalf(index);
  if (view.index === index && state.mode === "loupe") rawTilesRender(); // zoomed before the answer
}

function rawLeave() {
  if (!raw.enabled) return;
  invoke("raw_view", {
    generation: state.generation, index: null, neighbours: [], screenEdge: rawScreenEdge(),
    seq: ++raw.seq,
  }).catch(() => {});
}

function rawFolder(gen) {
  raw.ready.clear();
  raw.failed.clear();
  raw.wanted.clear();
  raw.full.clear();
  raw.canTile.clear();
  invoke("raw_set_enabled", { enabled: raw.enabled, generation: gen }).catch(() => {});
}

/// Width of the full RAW image behind the half-size image shown for `index` (null if unknown).
/// The half-size image is scaled to the screen, so 100 % is not simply twice its width.
function rawFullWidth(index) {
  const f = raw.full.get(index);
  return f ? f[0] : null;
}

/// "damaged" when the RAW data is missing or corrupt (truncated file), else "failed".
function rawFailureKind(message) {
  return /damaged:/.test(message || "") ? "damaged" : "failed";
}

function rawFailedText(index) {
  const m = raw.failed.get(index);
  if (!m) return null;
  return rawFailureKind(m) === "damaged"
    ? `RAW-data skadad eller ofullständig – visar kamerans inbäddade bild (${m})`
    : `RAW kunde inte avkodas – visar kamerans inbäddade bild (${m})`;
}

/// Shows the warning chip while the loupe shows the fallback image of a RAW that failed.
/// Called on every render, so it always follows the image on screen.
function rawWarnUpdate() {
  if (!raw.warn) return;
  const m = raw.enabled && state.mode === "loupe" && view.index >= 0 ? raw.failed.get(view.index) : null;
  if (!m) { raw.warn.hidden = true; return; }
  const damaged = rawFailureKind(m) === "damaged";
  // The fallback is the embedded preview; some damaged files have none left (it was cut off too).
  const shown = el.img.getAttribute("src") ? "visar inbäddad bild" : "ingen inbäddad bild att visa";
  raw.warn.textContent = damaged ? `⚠ Skadad RAW – ${shown}` : `⚠ RAW kunde inte avkodas – ${shown}`;
  raw.warn.title = rawFailedText(view.index);
  raw.warn.dataset.kind = damaged ? "damaged" : "failed";
  raw.warn.hidden = false;
}

async function rawSwapToHalf(index) {
  const info = state.info.get(index);
  if (!info || !raw.enabled || state.mode !== "loupe" || view.index !== index) return;
  if (view.level === "rawhalf" || raw.failed.has(index)) return;
  const gen = state.generation;
  const t0 = performance.now();
  const img = new Image();
  img.crossOrigin = "anonymous";
  img.src = url(index, "rawhalf");
  try { await img.decode(); } catch (_) {
    // Evicted from the backend cache meanwhile: it is queued again (raw_view made it current)
    // and a new "raw-ready" follows. The embedded preview stays until then.
    raw.ready.delete(index);
    rawShow(index);
    return;
  }
  const decodeMs = performance.now() - t0;
  if (gen !== state.generation || view.index !== index || state.mode !== "loupe" || !raw.enabled ||
      raw.failed.has(index)) return;
  const w = img.naturalWidth, h = img.naturalHeight;
  // Keep the framing: at "fit" refit; otherwise keep the same on-screen size and position.
  const atFit = view.scale <= view.fit * 1.01;
  const k = view.w / w;
  setSource(info, "rawhalf", w, h);
  if (atFit) fitToWindow();
  else { view.scale *= k; view.fit *= k; render(); }
  document.dispatchEvent(new CustomEvent("pw-raw-shown", { detail: { index, decodeMs, w, h } }));
}

listen("raw-ready", (ev) => {
  const { generation, index, full_width, full_height } = ev.payload;
  if (generation !== state.generation) return;
  if (raw.failed.has(index)) return; // a failed RAW never replaces the preview
  if (full_width && full_height) raw.full.set(index, [full_width, full_height]);
  raw.ready.add(index);
  if (raw.wanted.get(index)) rawSwapToHalf(index);
});

listen("raw-failed", (ev) => {
  const { generation, index, message } = ev.payload;
  if (generation !== state.generation) return;
  raw.failed.set(index, message || "okänt fel");
  raw.ready.delete(index);
  document.dispatchEvent(new CustomEvent("pw-raw-failed", { detail: { index, message } }));
  // Only the image on screen is touched; a late failure of another image changes nothing here.
  const info = state.info.get(index);
  if (info && view.index === index && state.mode === "loupe") {
    if (view.level === "rawhalf") showLoupe(index); // never keep RAW pixels of a failed file
    else if (!el.img.getAttribute("src")) { el.badge.textContent = rawFailedText(index); el.badge.hidden = false; }
    else updateBadge(info);
    rawWarnUpdate();
    rawTilesRender();
  }
});

// ---------- the "RAW-rendering" setting ----------

function rawRenderToggle() {
  raw.btn.textContent = raw.enabled ? "RAW-rendering: på" : "RAW-rendering: av";
  raw.btn.title = "Avkoda RAW-filer när den inbäddade bilden är för liten för skärmen. " +
    "Av = bara inbäddade bilder (som M1).";
}

/// Turns RAW rendering on or off (persist = remember the choice).
function rawSetEnabled(on, persist = true) {
  raw.enabled = on;
  if (persist) { try { localStorage.setItem("rawRendering", on ? "on" : "off"); } catch (_) {} }
  rawRenderToggle();
  raw.ready.clear();
  raw.failed.clear();
  rawWarnUpdate();
  if (window.rtReset) rtReset(); // tiles go with the engine's caches
  invoke("raw_set_enabled", { enabled: on, generation: state.generation }).catch(() => {});
  if (state.mode === "loupe" && state.visible.length) {
    if (!on && view.level === "rawhalf") showLoupe(state.current); // back to the embedded preview
    else if (on) rawShow(state.current);
  }
}

(() => {
  raw.btn = document.createElement("button");
  raw.btn.id = "status-raw";
  raw.btn.className = "link";
  document.getElementById("status").appendChild(raw.btn);
  raw.btn.addEventListener("click", () => rawSetEnabled(!raw.enabled));
  rawRenderToggle();
  raw.warn = document.createElement("div");
  raw.warn.id = "loupe-rawwarn";
  raw.warn.hidden = true;
  raw.warn.setAttribute("role", "status");
  document.getElementById("loupe").appendChild(raw.warn);
  // The first folder may have been opened before this script ran: tell the backend the setting.
  invoke("raw_set_enabled", { enabled: raw.enabled, generation: state.generation }).catch(() => {});
})();
